use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

pub(crate) use self::message::{SessionMessage, WriterMessage};
use crate::error::Result;
pub(crate) use crate::net::connect;
use crate::router::session::SessionNegotiation;
use crate::router::{Expiration, RouterCtx, Session};
use crate::storage::{Key, KeyKind};

pub(crate) mod message;

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

    async fn new_session(&self, writer_agent_key: Key) -> Result<Key> {
        // ... create a new session agent
        let session_agent = self
            .router_ctx
            .new_session_agent::<()>(None, None, self.router_ctx.serializer())
            .await?;

        // ... track the writer agent
        session_agent.track(writer_agent_key).await;

        // ... spawn a new session
        let expiration = Expiration::after(Duration::from_secs(60 * 5));
        let session = Session::new(expiration, session_agent, writer_agent_key, 100);
        let key = session.key();
        tokio::spawn(session.run());
        Ok(key)
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
                .new_writer_agent::<()>(None, None, self.router_ctx.serializer())
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

            let session_negotiation = reader.read_u8().await?.into();
            let session_key = match session_negotiation {
                SessionNegotiation::HasExistingSession => {
                    // Read the session key from the connection
                    let session_key = Key::new(reader.read_u64().await?, KeyKind::Session);

                    // Check if the router has an agent with the session key
                    let session_exists = self.router_ctx.session_exists(session_key).await;

                    match session_exists {
                        // If the session that was read from the connection exists then notify the
                        // connection
                        true => {
                            writer
                                .write_u8(SessionNegotiation::ValidSession as u8)
                                .await?;
                            session_key
                        }
                        // If the session is no longer valid then notify the connection and...
                        false => {
                            writer
                                .write_u8(SessionNegotiation::InvalidSession as u8)
                                .await?;
                            let new_session_key = self.new_session(writer_agent.key()).await?;
                            writer.write_u64(new_session_key.raw()).await?;
                            new_session_key
                        }
                    }
                }
                SessionNegotiation::RequestNewSession => {
                    let new_session_key = self.new_session(writer_agent.key()).await?;
                    writer.write_u64(new_session_key.raw()).await?;
                    new_session_key
                }
                _ => panic!("invalid session data"),
            };

            tokio::spawn(crate::net::read(
                self.router_ctx.clone(),
                reader,
                self.heartbeat,
                session_key,
                writer_agent.key(),
            ));

            tokio::spawn(crate::net::write(writer, writer_agent));
        }

        Ok(())
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
