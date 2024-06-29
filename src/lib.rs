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
pub use net::{Listener, Connection, Stream, TcpConnection, UdsConnection};
pub use retry::{timeout, Timeout};
pub use router::Router;

// -----------------------------------------------------------------------------
//   - Re-exports -
// -----------------------------------------------------------------------------
pub use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
