use serde::Serialize;
use tracing::{error, info};

use crate::address::InternalAddress;
use crate::bridge::WriterMessage;
use crate::error::Result;
use crate::serializer::Serializer;
use crate::slab::WriterKey;
use crate::Agent;

#[derive(Debug)]
pub(crate) struct WriterAgent(Agent);

impl WriterAgent {
    pub(crate) fn new(agent: Agent) -> Self {
        Self(agent)
    }

    pub(crate) fn key(&self) -> WriterKey {
        self.0.key().into()
    }

    #[tracing::instrument]
    pub(crate) async fn recv(&self) -> Result<WriterMessage> {
        let msg = self.0.rx.recv_async().await?;
        match msg {
            super::AnyMessage::Writer(value) => {
                info!("received a message");
                Ok(value)
            }
            // TODO what should be done here?
            // The bridge should perhaps just ignore
            // agent messages? Maybe a log entry?
            super::AnyMessage::RemoteValue { .. } => {
                unreachable!("bridge agent should never get a remote value")
            }
            super::AnyMessage::Value { value, .. } => {
                let val = value
                    .downcast::<WriterMessage>();
                
                info!("received a message (downcast)");
                let is_err = val.is_err();

                let val = *val
                    .expect("only `Writer` messages can be sent to the writer");
                Ok(val)
            }
            super::AnyMessage::LocalRequest { .. } => todo!("local request"),
            super::AnyMessage::AgentRemoved(_) => todo!(), // TODO this is no longer relevant? -tb 2024-06-21  Ok(SessionMessage::AgentRemoved(key.into())),
            super::AnyMessage::Session(sess_msg) => {
                error!("received a session message (it shouldn't): {sess_msg:?}");
                todo!()
            }
        }
    }

    pub(crate) fn serializer(&self) -> Serializer {
        self.0.serializer
    }

    pub(crate) fn serialize(&self, value: &impl Serialize) -> Result<Box<[u8]>> {
        self.0.serializer.serialize(value)
    }

    pub(crate) async fn track(&self, key: WriterKey) -> Result<()> {
        let addr = InternalAddress::Local(key.into()).into();
        self.0.track(&addr).await
    }

    pub(crate) async fn remove_self(self) {
        let _ = self.0.remove_self().await;
    }
}
