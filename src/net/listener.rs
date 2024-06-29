use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UnixListener};

use crate::error::Result;
use crate::Stream;

use super::connection::DummyStream;

/// A listener that returns an async read / write half
/// ```no_run
/// # async fn run_async() {
/// # use tokio::net::TcpListener;
/// let mut listener = TcpListener::bind("127.0.0.1:1234").await.unwrap();
/// let (read, write) = listener.accept().await.unwrap();
/// # }
/// ```
pub trait Listener: Unpin + Send + Sync + 'static {
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
        let (output, _addr) = TcpListener::accept(self).await?;
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
        let (output, _addr) = UnixListener::accept(self).await?;
        Ok(output.into_split())
    }
}

pub(crate) struct DummyListener {
    streams: Vec<DummyStream>,
}

impl DummyListener {
    pub(crate) fn new(streams: Vec<DummyStream>) -> Self {
        Self {
            streams,
        }
    }
}

impl Listener for DummyListener {
    async fn accept(
        &mut self,
    ) -> Result<(
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    )> {
        let stream  = self.streams.remove(0);
        Ok(stream.split())
    }
}
