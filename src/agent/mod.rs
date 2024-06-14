use std::ops::Deref;

use flume::Sender;
use serde::de::DeserializeOwned;

pub(crate) use self::bridge::BridgeAgent;
pub use self::local::Agent;
use crate::address::Address;
use crate::bridge::{ReaderMessage, WriterMessage};
use crate::error::{Error, Result};
use crate::request::{Pending, Request, Serializable};
use crate::serializer::Serializer;
use crate::slab::AgentKey;
use crate::value::{AnyValue, Initial, RemoteVal};

mod bridge;
mod local;

pub(crate) enum AnyMessage {
    Value {
        value: AnyValue,
        sender: Address,
    },
    RemoteValue {
        value: Box<[u8]>,
        sender: Address,
    },
    /// This value is sent to a bridge agent and should be ignored
    /// by regular agents
    Bridge(WriterMessage),
    LocalRequest {
        request: Request<Pending>,
        sender: Address,
    },
    AgentRemoved(AgentKey),
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
                    sender,
                }),
                Err(_) => Err(Error::InvalidValueType),
            },
            AnyMessage::RemoteValue { value, sender } => {
                let value = serializer.deserialize(value)?;
                Ok(AgentMessage::Value { value, sender })
            }
            AnyMessage::Bridge(_) => {
                unreachable!("this should be handled directly by the bridge")
            }
            AnyMessage::LocalRequest { request, sender } => {
                Ok(AgentMessage::Request { request, sender })
            }
            AnyMessage::AgentRemoved(key) => Ok(AgentMessage::AgentRemoved(Address::Local(key))),
        }
    }

    fn to_local_agent_message<T: 'static>(self) -> Result<AgentMessage<T>> {
        match self {
            AnyMessage::Value { value, sender } => match value.downcast::<T>() {
                Ok(val) => Ok(AgentMessage::Value {
                    value: *val,
                    sender,
                }),
                Err(_) => Err(Error::InvalidValueType),
            },
            AnyMessage::RemoteValue { value, sender } => Err(Error::RemoteActionOnLocal),
            AnyMessage::Bridge(_) => {
                unreachable!("this should be handled directly by the bridge")
            }
            AnyMessage::LocalRequest { request, sender } => {
                Ok(AgentMessage::Request { request, sender })
            }
            AnyMessage::AgentRemoved(key) => Ok(AgentMessage::AgentRemoved(Address::Local(key))),
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
}
