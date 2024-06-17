use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

pub(crate) use self::message::{ReaderMessage, WriterMessage};
pub(crate) use self::net::connect;
use crate::error::Result;
use crate::router::{Expiration, RouterCtx, Session};
use crate::serializer::Serializer;

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

    pub(crate) async fn run(mut self, address: impl Serialize) -> Result<()> {
        while let Ok((reader, mut writer)) = self.listener.accept().await {
            let serializer = self.router_ctx.serializer() as u8;
            if let Err(_) = writer.write_u8(serializer).await {
                continue;
            }

            // TODO
            // * Create a session
            // * Send session to router
            // * Writing messages to a session rather than a writer directly

            // TODO negotiate heartbeat (see Agent::connect for the same comment)
            // TODO read capacity or configure this via the server?

            let session_agent = self
                .router_ctx
                .new_bridge_agent::<()>(None, None, self.router_ctx.serializer())
                .await?;

            let writer_agent = self
                .router_ctx
                .new_bridge_agent::<()>(None, None, self.router_ctx.serializer())
                .await?;

            session_agent.track(&writer_agent.address()).await;

            tokio::spawn(net::read(
                self.router_ctx.clone(),
                reader,
                self.heartbeat,
                session_agent.key(),
            ));

            // TODO
            // Expiration should be configured as part of the
            // connection setup. For now, it's hard coded to five minutes
            let expiration = Expiration::after(Duration::from_secs(60 * 5));
            let session = Session::new(expiration, session_agent);

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

pub(crate) trait Listener: Unpin + Send + 'static {
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
