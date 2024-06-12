#![deny(missing_docs)]
//! Binky is a message passing library

mod address;
mod agent;
mod bridge;
mod error;
mod frame;
mod request;
mod router;
mod serializer;
mod slab;
mod value;

pub use agent::{AgentMessage, Agent};
pub use bridge::Stream;
pub use router::Router;

// -----------------------------------------------------------------------------
//   - Re-exports -
// -----------------------------------------------------------------------------
pub use tokio::net::{TcpListener, TcpStream, UnixStream, UnixListener};
