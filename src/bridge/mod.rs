use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

pub(crate) use self::message::{BridgeMessage, WriterMessage};
pub(crate) use self::net::connect;
use crate::error::Result;
use crate::router::{Expiration, RouterCtx, Session};
use crate::SessionKey;

mod message;
mod net;

pub(crate) struct Bridge<T> {
    listener: T,
    heartbeat: Option<Duration>,
    router_ctx: RouterCtx,
}

impl<T> Bridge<T>
where
    T: Listener,
{
    pub(crate) fn new(listener: T, router_ctx: RouterCtx, heartbeat: Option<Duration>) -> Self {
        Self {
            listener,
            heartbeat,
            router_ctx,
        }
    }

    pub(crate) async fn run(mut self) -> Result<()> {
        while let Ok((mut reader, mut writer)) = self.listener.accept().await {
            let serializer = self.router_ctx.serializer() as u8;
            if let Err(_) = writer.write_u8(serializer).await {
                continue;
            }

            // TODO negotiate heartbeat (see Agent::connect for the same comment)
            // TODO read capacity or configure this via the server?

            let writer_agent = self
                .router_ctx
                .new_bridge_agent::<()>(None, None, self.router_ctx.serializer())
                .await?;

            // -----------------------------------------------------------------------------
            //   - Read / Write session info -
            //   Every connection has a session. This is written as:
            //   Step 1: There is a session (sending 1)
            //   Step 2: Write the session key as be_bytes
            // -----------------------------------------------------------------------------

            // TODO
            // Expiration should be configured as part of the
            // connection setup. For now, it's hard coded to five minutes
            let expiration = Expiration::after(Duration::from_secs(60 * 5));
            let session_key = match reader.read_u8().await? == 1 {
                true => {
                    let session_key = SessionKey::from(reader.read_u64().await?);

                    // Flow:
                    // 1. Check if session is active:
                    //  -> yes all is well
                    //  -> no, make new session and reply with a new session key

                    let session = self.router_ctx.get_or_create_session(session_key).await?;
                    if session.key() == session_key {
                        writer.write_u8(1).await?;
                        // they are the same session, so no need to do anything but associate
                        // the new writer with the given session
                        self.router_ctx.session_track_writer(session.key(), writer_agent.key()).await;
                        session_key
                    } else {
                        // they are **NOT** the same
                        writer.write_u8(0).await?;

                        let session_agent = self
                            .router_ctx
                            .new_bridge_agent::<()>(None, None, self.router_ctx.serializer())
                            .await?;

                        session_agent.track(writer_agent.key()).await;

                        let raw_session_key = u64::from(session_agent.key()).to_be_bytes();
                        writer.write_all(&raw_session_key).await?;
                        let session = Session::new(expiration, session_agent, writer_agent.key(), 100);
                        let key = session.key();
                        tokio::spawn(session.run());
                        key
                    }
                }
                false => {
                    let session_agent = self
                        .router_ctx
                        .new_bridge_agent::<()>(None, None, self.router_ctx.serializer())
                        .await?;

                    session_agent.track(writer_agent.key()).await;

                    let raw_session_key = u64::from(session_agent.key()).to_be_bytes();
                    writer.write_all(&raw_session_key).await?;
                    let session = Session::new(expiration, session_agent, writer_agent.key(), 100);
                    let key = session.key();
                    tokio::spawn(session.run());
                    key
                }
            };

            tokio::spawn(net::read(
                self.router_ctx.clone(),
                reader,
                self.heartbeat,
                session_key,
            ));

            tokio::spawn(net::write(writer, writer_agent));
        }

        Ok(())
    }
}

/// Implement the `Stream` trait for any type that should be used
/// with the `Agent::connect` function to pass data to other `Router`s.
pub trait Stream: Send + 'static {
    /// Split the stream into a read / write half
    fn split(
        self,
    ) -> (
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    );
}

impl Stream for TcpStream {
    fn split(
        self,
    ) -> (
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    ) {
        self.into_split()
    }
}

impl Stream for UnixStream {
    fn split(
        self,
    ) -> (
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    ) {
        self.into_split()
    }
}

/// A listener that returns an async read / write half
/// ```no_run
/// # async fn run_async() {
/// # use tokio::net::TcpListener;
/// let mut listener = TcpListener::bind("127.0.0.1:1234").await.unwrap();
/// let (read, write) = listener.accept().await.unwrap();
/// # }
/// ```
pub trait Listener: Unpin + Send + 'static {
    /// Accept and split incoming streams
    fn accept(
        &mut self,
    ) -> impl std::future::Future<
        Output = Result<(
            impl AsyncReadExt + Unpin + Send + 'static,
            impl AsyncWriteExt + Unpin + Send + 'static,
        )>,
    > + Send;
}

impl Listener for TcpListener {
    async fn accept(
        &mut self,
    ) -> Result<(
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    )> {
        let (output, addr) = TcpListener::accept(self).await?;
        Ok(output.into_split())
    }
}

impl Listener for UnixListener {
    async fn accept(
        &mut self,
    ) -> Result<(
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    )> {
        let (output, addr) = UnixListener::accept(self).await?;
        Ok(output.into_split())
    }
}
