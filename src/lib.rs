#![deny(missing_docs)]
//! Binky is a message passing library

pub(crate) mod address;
mod agent;
mod bridge;
pub mod error;
mod frame;
mod queue;
mod request;
mod retry;
mod router;
mod serializer;
mod slab;
mod value;

pub use address::Address;
pub use agent::{Agent, AgentMessage};
pub use bridge::{Listener, Stream};
pub use retry::{timeout, Timeout};
pub use router::Router;
pub use slab::SessionKey;
// -----------------------------------------------------------------------------
//   - Re-exports -
// -----------------------------------------------------------------------------
pub use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
