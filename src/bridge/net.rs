use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use tracing::info;

use super::SessionMessage;
use crate::address::InternalAddress;
use crate::agent::WriterAgent;
use crate::bridge::message::ReaderMessage;
use crate::bridge::WriterMessage;
use crate::error::{Error, Result};
use crate::frame::{Frame, FrameOutput};
use crate::router::session::SessionNegotiation;
use crate::router::{RouterCtx, RouterMessage};
use crate::serializer::Serializer;
use crate::slab::WriterKey;
use crate::value::Outgoing;
use crate::{SessionKey, Stream};

pub(crate) async fn connect<T: Serialize>(
    stream: impl Stream,
    router_ctx: RouterCtx,
    heartbeat: Option<Duration>,
    address: Option<T>,
    session: Option<SessionKey>,
) -> Result<SessionKey> {
    let (mut reader, mut writer) = stream.split();

    let serializer: Serializer = reader.read_u8().await?.try_into()?;

    let session_key = match session {
        Some(session_key) => {
            // Tell the receiving end that we have a session
            writer.write_u8(SessionNegotiation::HasExistingSession as u8).await?;
            writer.write_u64(session_key.into()).await?;

            match SessionNegotiation::from(reader.read_u8().await?) {
                SessionNegotiation::ValidSession => session_key,
                SessionNegotiation::InvalidSession => SessionKey::from(reader.read_u64().await?),
                _ => todo!("return an error about invalid session"),
            }
        }
        None => {
            // Tell the receiving end that we don't have a session
            writer.write_u8(SessionNegotiation::RequestNewSession as u8).await?;
            SessionKey::from(reader.read_u64().await?)
        }
    }.into();

    let agent = router_ctx
        .new_writer_agent::<T>(address, None, serializer)
        .await?;

    The session key is only needed here for reconnection.
    Instead of always having a session, let's make the session a proxy for the writer
    but only apply that to the server side, on the client side the reader gets the writer key

    tokio::spawn(read(router_ctx.clone(), reader, heartbeat, session_key, agent.key()));
    tokio::spawn(write(writer, agent));

    Ok(session_key)
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
    async fn run(mut self) -> Result<()> {
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
                WriterMessage::Shutdown => break,
            }
        }

        Ok(())
    }

    async fn write_msg(&mut self, msg: ReaderMessage) -> Result<()> {
        // NOTE if the value can't be serialized there isn't much
        // that can be done here, so just dispose of it and return Ok (even though it's not)
        let Ok(raw_bytes) = self.agent.serialize(&msg) else { return Ok(()) };
        let payload = FrameOutput::frame_message(&raw_bytes);
        self.writer.write(payload.as_ref()).await?;
        Ok(())
    }
}

pub(crate) async fn write(writer: impl AsyncWriteExt + Unpin, agent: WriterAgent) -> Result<()> {
    Writer { writer, agent }.run().await
}

// -----------------------------------------------------------------------------
//   - Socket reader -
// -----------------------------------------------------------------------------
pub(crate) async fn read(
    router_ctx: RouterCtx,
    mut reader: impl AsyncReadExt + Unpin,
    heartbeat: Option<Duration>,
    session_key: SessionKey,
    writer_key: WriterKey,
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
    // info!("here");
}
