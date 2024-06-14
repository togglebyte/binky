#![deny(missing_docs)]
//! Binky is a message passing library

mod address;
mod agent;
mod bridge;
pub mod error;
mod frame;
mod request;
mod retry;
mod router;
mod serializer;
mod slab;
mod value;

pub use address::Address;
pub use agent::{Agent, AgentMessage};
pub use bridge::Stream;
pub use retry::{timeout, Timeout};
pub use router::Router;

// -----------------------------------------------------------------------------
//   - Re-exports -
// -----------------------------------------------------------------------------
pub use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
