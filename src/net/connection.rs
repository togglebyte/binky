use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{empty, repeat, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Empty, Repeat};
use tokio::net::{TcpStream, ToSocketAddrs, UnixStream};

use crate::error::Result;
use crate::Timeout;

/// Connection handling.
/// Implementation of this trait is in charge of connecting, reconecting and
/// disconnecting.
pub trait Connection: Send + 'static {
    /// Create a stream
    async fn connect(&mut self) -> Result<impl Stream + Send + 'static>;

    /// Sleep between reconnect attempts
    async fn sleep(&mut self) -> Result<()>;
}

pub(crate) struct DummyConnection(Timeout);

impl Connection for DummyConnection {
    async fn connect(&mut self) -> Result<impl Stream> {
        Ok(DummyStream::empty())
    }

    async fn sleep(&mut self) -> Result<()> {
        self.0.sleep().await
    }
}

/// Connection handling for a tcp socket
pub struct TcpConnection<A>(A, Timeout);

impl<A> TcpConnection<A> {
    /// Create a new instance of a tcp connection
    pub fn new(addr: A, timeout: Timeout) -> Self {
        Self(addr, timeout)
    }
}

impl<A: ToSocketAddrs + Send + 'static> Connection for TcpConnection<A> {
    async fn connect(&mut self) -> Result<impl Stream> {
        Ok(TcpStream::connect(&self.0).await?)
    }

    async fn sleep(&mut self) -> Result<()> {
        self.1.sleep().await
    }
}

/// Connection handling for a unix domain socket.
pub struct UdsConnection(PathBuf, Timeout);

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

pub(crate) struct DummyStream<R, W>(R, W);

impl DummyStream<Empty, Vec<u8>> {
    pub fn empty() -> DummyStream<Empty, Vec<u8>> {
        Self(empty(), vec![])
    }
}

impl DummyStream<Repeat, Vec<u8>> {
    pub fn repeat(byte: u8) -> Self {
        Self(repeat(byte), vec![])
    }
}

impl DummyStream<Vec<u8>, Vec<u8>> {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data, vec![])
    }
}

impl<R: AsyncRead + Unpin + Send + 'static, W: AsyncWrite + Unpin + Send + 'static> Stream
    for DummyStream<R, W>
{
    fn split(
        self,
    ) -> (
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    ) {
        (self.0, self.1)
    }
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
