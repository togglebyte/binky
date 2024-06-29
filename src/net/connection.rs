use std::future::Future;
use std::io::Cursor;
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
    fn connect(&mut self) -> impl Future<Output = Result<impl Stream + Send + 'static>> + Send;

    /// Sleep between reconnect attempts
    async fn sleep(&mut self) -> Result<()>;
}

pub(crate) struct DummyConnection {
    pub(crate) stream: DummyStream,
    pub(crate) timeout: Timeout,
}

impl Connection for DummyConnection {
    fn connect(&mut self) -> impl Future<Output = Result<impl Stream + Send + 'static>> + Send {
        async move { Ok(DummyStream::new(vec![])) }
    }

    async fn sleep(&mut self) -> Result<()> {
        self.timeout.sleep().await
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

impl<A: ToSocketAddrs + Send + Sync + 'static> Connection for TcpConnection<A> {
    fn connect(&mut self) -> impl Future<Output = Result<impl Stream + Send + 'static>> + Send {
        async move { Ok(TcpStream::connect(&self.0).await?) }
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

pub(crate) struct DummyStream {
    read: Cursor<Vec<u8>>,
    write: Vec<u8>,
}

impl DummyStream {
    pub fn new(read: Vec<u8>) -> Self {
        Self {
            read: Cursor::new(read),
            write: vec![],
        }
    }
}

impl Stream for DummyStream {
    fn split(
        self,
    ) -> (
        impl AsyncReadExt + Unpin + Send + 'static,
        impl AsyncWriteExt + Unpin + Send + 'static,
    ) {
        (self.read, self.write)
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
