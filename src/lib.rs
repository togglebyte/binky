// TODO remove this nonsense
#![allow(unused)]
#![allow(private_interfaces)]

// 
#![allow(async_fn_in_trait)]
#![deny(missing_docs)]
//! Binky is a message passing library

use storage::Key;

pub(crate) mod address;
mod agent;
mod bridge;
pub mod error;
mod frame;
mod net;
mod queue;
mod request;
mod retry;
mod router;
mod serializer;
mod storage;
mod value;

pub use address::Address;
pub use agent::{Agent, AgentMessage};
pub use bridge::Listener;
pub use net::{Connection, Stream, TcpConnection, UdsConnection};
pub use retry::{timeout, Timeout};
pub use router::Router;

/// A session key is returned when calling [`Agent::connect`].
/// The key might change upon reconnect and should be passed to the `connect` function upon
/// reconnect.
///
/// ```no_run
/// let mut session = None::<SessionKey>;
/// session = agent.connect(stream, session).await;
/// ```
pub type SessionKey = Key;

// -----------------------------------------------------------------------------
//   - Re-exports -
// -----------------------------------------------------------------------------
pub use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
