use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use tracing::info;

pub use self::connection::{Connection, Stream, TcpConnection, UdsConnection};
pub(crate) use self::listener::DummyListener;
pub use self::listener::Listener;
use crate::address::InternalAddress;
use crate::agent::WriterAgent;
use crate::bridge::message::ReaderMessage;
use crate::bridge::{SessionMessage, WriterMessage};
use crate::error::{Error, Result};
use crate::frame::{Frame, FrameOutput};
use crate::router::session::SessionNegotiation;
use crate::router::{RouterCtx, RouterMessage};
use crate::serializer::Serializer;
use crate::storage::{Key, KeyKind};
use crate::value::Outgoing;

mod connection;
mod listener;

pub(crate) async fn connect<T: Serialize + Clone>(
    mut connection: impl Connection,
    router_ctx: RouterCtx,
    heartbeat: Option<Duration>,
    address: Option<T>,
) -> Result<()> {
    let mut session_key = None::<Key>;

    loop {
        let stream = connection.connect().await?;
        let (mut reader, mut writer) = stream.split();

        // Get the serializer
        let serializer: Serializer = reader.read_u8().await?.try_into()?;

        // Get the session key
        let key = match session_key {
            Some(session_key) => {
                // Tell the receiving end that we have a session
                writer.write_u8(SessionNegotiation::HasExistingSession as u8).await?;
                writer.write_u64(session_key.raw()).await?;

                match SessionNegotiation::from(reader.read_u8().await?) {
                    SessionNegotiation::ValidSession => session_key,
                    SessionNegotiation::InvalidSession => Key::new(reader.read_u64().await?, KeyKind::Session),
                    _ => todo!("return an error about invalid session"),
                }
            }
            None => {
                // Tell the receiving end that we don't have a session
                writer.write_u8(SessionNegotiation::RequestNewSession as u8).await?;
                Key::new(reader.read_u64().await?, KeyKind::Session)
            }
        };
        session_key = Some(key);

        let writer_agent = router_ctx
            .new_writer_agent::<T>(address.clone(), None, serializer)
            .await?;

        tokio::spawn(read(router_ctx.clone(), reader, heartbeat, key, writer_agent.key()));

        match write(writer, writer_agent).await? {
            WriterState::Reconnect => continue,
            WriterState::Stop => break Ok(()),
        }

        // Wait between reconnects
        match connection.sleep().await {
            Ok(_) => continue,
            Err(Error::NoRetry) => break Ok(()),
            Err(_) => unreachable!(),
        }
    }
}

pub(crate) enum WriterState {
    Reconnect,
    Stop,
}

// -----------------------------------------------------------------------------
//   - Socket writer -
// -----------------------------------------------------------------------------
struct Writer<W> {
    writer: W,
    agent: WriterAgent,
}

impl<W> Writer<W>
where
    W: AsyncWriteExt + Unpin,
{
    async fn run(mut self) -> Result<WriterState> {
        while let Ok(msg) = self.agent.recv().await {
            match msg {
                WriterMessage::Value(value) => {
                    let msg = value.next(self.agent.serializer());
                    let msg = ReaderMessage::Value(msg.0);
                    self.write_msg(msg).await?;
                }
                WriterMessage::AddressRequest { callback, address } => {
                    let msg = ReaderMessage::AddressRequest { callback, address };
                    self.write_msg(msg).await?;
                }
                WriterMessage::AddressResponse { callback, address } => {
                    let msg = ReaderMessage::AddressResponse { callback, address };
                    self.write_msg(msg).await?;
                }
                WriterMessage::Shutdown => return Ok(WriterState::Stop),
            }
        }

        Ok(WriterState::Reconnect)
    }

    async fn write_msg(&mut self, msg: ReaderMessage) -> Result<()> {
        // NOTE if the value can't be serialized there isn't much
        // that can be done here, so just dispose of it and return Ok (even though it's not)
        let Ok(raw_bytes) = self.agent.serialize(&msg) else {
            return Ok(());
        };
        let payload = FrameOutput::frame_message(&raw_bytes);
        self.writer.write(payload.as_ref()).await?;
        Ok(())
    }
}

pub(crate) async fn write(writer: impl AsyncWriteExt + Unpin, agent: WriterAgent) -> Result<WriterState> {
    Writer { writer, agent }.run().await
}

// -----------------------------------------------------------------------------
//   - Socket reader -
// -----------------------------------------------------------------------------
pub(crate) async fn read(
    router_ctx: RouterCtx,
    mut reader: impl AsyncReadExt + Unpin,
    heartbeat: Option<Duration>,
    session_key: Key,
    writer_key: Key,
) {
    let mut frame = Frame::empty();

    let timeout = match heartbeat {
        Some(dur) => dur,
        None => Duration::MAX,
    };

    'read: loop {
        let timeout = sleep(timeout);
        let should_break = tokio::select! {
            _ = timeout => true,
            res = frame.read_async(&mut reader) => {
                match res {
                    Ok(0) => break 'read,
                    Ok(_byte_count) => 'msg: loop {
                        match frame.try_msg() {
                            Ok(Some(msg)) => {
                                match msg {
                                    FrameOutput::Bytes(bytes) => {
                                        let Ok(msg) = router_ctx.deserialize::<ReaderMessage>(bytes) else { continue };
                                        match msg {
                                            ReaderMessage::Value(Outgoing { value, recipient, sender, reply_serializer }) => {
                                                let sender = InternalAddress::Remote {
                                                    local_session_key: session_key,
                                                    remote_address: sender,
                                                    remote_serializer: reply_serializer.try_into().expect("this is always created from a serializer"),
                                                };
                                                let msg = RouterMessage::incoming(value, sender, recipient);
                                                router_ctx.send(msg).await;
                                            }
                                            ReaderMessage::AddressRequest { callback, address } => {
                                                let address = router_ctx.lookup_address(address).await.ok();
                                                let msg = RouterMessage::respond_resolve_remote(callback, address, session_key);
                                                router_ctx.send(msg).await;
                                            }
                                            ReaderMessage::AddressResponse { callback, address } => {
                                                let address = address.ok_or(Error::AddressNotFound);
                                                // TODO
                                                // What do we do if the callback fails?
                                                // This can onlyh happen if the Receiver in the
                                                // router has closed down, meaning the router
                                                // is inaccessible, meaning there is no reason to
                                                // continue the program
                                                router_ctx.callback(callback, address.into()).await;
                                            }
                                            //RemoteMessage::AddressRequest { address, callback_id } => {
                                            //    let msg = RouterMessage::something(address, callback_id, writer);
                                            //    router_ctx.send(msg).await;
                                            //}
                                            //RemoteMessage::ResolveResponse { address, callback_id } => todo!(),
                                        }
                                    }
                                    FrameOutput::Heartbeat => continue,
                                }
                            }
                            Ok(None) => continue 'read,
                            Err(Error::MalformedHeader) => {
                                frame = Frame::empty();
                                break 'msg;
                            }
                            Err(_) => unreachable!("try_msg can only fail with a malformed header"),
                        }
                    },
                    Err(e) => {
                        log::error!("{e}");
                        break;
                    }
                }
                false
            }
        };

        if should_break {
            break;
        }
    }

    info!("reader closed, removing writer");
    router_ctx.remove_writer(writer_key).await;
}

#[cfg(test)]
mod test {
    use connection::{DummyConnection, DummyStream};

    use super::*;
    use crate::{timeout, Router};

    #[tokio::test]
    async fn connect_to_router() {
        let mut router = Router::new();
        let listener = DummyListener::new(vec![DummyStream::new(vec![
            SessionNegotiation::RequestNewSession as u8,
        ])]);
        router.listen(listener);

        let mut agent = router.agent("agent 1");

        let connection = DummyConnection {
            stream: DummyStream::new(vec![Serializer::Json as u8]),
            timeout: timeout(),
        };

        agent.connect(connection, "connection").await;

        agent.recv::<()>().await;
    }
}
