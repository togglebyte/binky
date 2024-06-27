use serde::de::DeserializeOwned;

pub use self::local::Agent;
pub(crate) use self::session::SessionAgent;
pub(crate) use self::writer::WriterAgent;
use crate::address::InternalAddress;
use crate::bridge::{SessionMessage, WriterMessage};
use crate::error::{Error, Result};
use crate::request::{Pending, Request};
use crate::serializer::Serializer;
use crate::storage::Key;
use crate::value::AnyValue;
use crate::Address;

mod local;
mod session;
mod writer;

pub(crate) enum AnyMessage {
    Value {
        value: AnyValue,
        sender: InternalAddress,
    },
    RemoteValue {
        value: Box<[u8]>,
        sender: InternalAddress,
    },
    /// This value is sent to a bridge agent and should be ignored by regular agents
    Session(SessionMessage),
    /// This value is sent to a writer agent and should be ignored by regular agents
    Writer(WriterMessage),
    LocalRequest {
        request: Request<Pending>,
        sender: InternalAddress,
    },
    AgentRemoved(Key),
}

impl AnyMessage {
    fn to_agent_message<T: DeserializeOwned + 'static>(
        self,
        serializer: Serializer,
    ) -> Result<AgentMessage<T>> {
        match self {
            AnyMessage::Value { value, sender } => match value.downcast::<T>() {
                Ok(val) => Ok(AgentMessage::Value {
                    value: *val,
                    sender: Address(sender),
                }),
                Err(_) => Err(Error::InvalidValueType),
            },
            AnyMessage::RemoteValue { value, sender } => {
                let value = serializer.deserialize(value)?;
                Ok(AgentMessage::Value {
                    value,
                    sender: sender.into(),
                })
            }
            AnyMessage::LocalRequest { request, sender } => Ok(AgentMessage::Request {
                request,
                sender: sender.into(),
            }),
            AnyMessage::AgentRemoved(key) => Ok(AgentMessage::AgentRemoved(
                InternalAddress::Local(key).into(),
            )),
            AnyMessage::Session(_) => {
                unreachable!("this should be handled directly by the session")
            }
            AnyMessage::Writer(_) => {
                unreachable!("this should be handled directly by the writer")
            }
        }
    }

    fn to_local_agent_message<T: 'static>(self) -> Result<AgentMessage<T>> {
        match self {
            AnyMessage::Value { value, sender } => match value.downcast::<T>() {
                Ok(val) => Ok(AgentMessage::Value {
                    value: *val,
                    sender: sender.into(),
                }),
                Err(_) => Err(Error::InvalidValueType),
            },
            AnyMessage::RemoteValue { .. } => Err(Error::RemoteActionOnLocal),
            AnyMessage::LocalRequest { request, sender } => Ok(AgentMessage::Request {
                request,
                sender: sender.into(),
            }),
            AnyMessage::AgentRemoved(key) => Ok(AgentMessage::AgentRemoved(
                InternalAddress::Local(key).into(),
            )),
            AnyMessage::Session(_) => {
                unreachable!("this should be handled directly by the session")
            }
            AnyMessage::Writer(_) => {
                unreachable!("this should be handled directly by the writer")
            }
        }
    }
}

/// A message received by an agent.
/// ```no_run
/// use binky::{Agent, AgentMessage};
///
/// # async fn async_run(mut agent: Agent) {
/// while let Ok(msg) = agent.recv::<()>().await {
///     match msg {
///         AgentMessage::Value { value, sender } => {}
///         AgentMessage::Request { request, sender } => {
///             let request = request.read::<(u8, u8)>().unwrap();
///             let (a, b) = *request;
///             request.reply(a + b).await;
///         }
///         AgentMessage::AgentRemoved(key) => {}
///     }
/// }
/// # }
/// ```
#[derive(Debug)]
pub enum AgentMessage<T> {
    /// A value sent to the agent
    Value {
        /// The actual value
        value: T,
        /// The sender of the value
        sender: Address,
    },
    /// A request that can be replied to
    Request {
        /// The request
        request: Request<Pending>,
        /// The sender of the request
        sender: Address,
    },
    /// An agent with a given address was removed
    AgentRemoved(Address),
    /// A connection associated with the Agent was disconnected
    Disconnected,
    /// Agent connected to a remote router
    Connected,
}
