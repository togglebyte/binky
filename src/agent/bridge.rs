use serde::Serialize;

use crate::address::InternalAddress;
use crate::bridge::{BridgeMessage, WriterMessage};
use crate::error::Result;
use crate::serializer::Serializer;
use crate::slab::BridgeKey;
use crate::{Address, Agent};

pub(crate) struct BridgeAgent(Agent);

impl BridgeAgent {
    pub(crate) fn new(agent: Agent) -> Self {
        Self(agent)
    }

    pub(crate) fn key(&self) -> BridgeKey {
        self.0.key().into()
    }

    pub(crate) async fn recv(&self) -> Result<BridgeMessage> {
        let msg = self.0.rx.recv_async().await?;
        match msg {
            super::AnyMessage::Bridge(value) => Ok(value),
            // TODO uwat should be done here?
            // The bridge should perhaps just ignore
            // agent messages? Maybe a log entry?
            super::AnyMessage::RemoteValue { value, sender } => {
                eprintln!("remote value");
                panic!()
            }
            super::AnyMessage::Value { sender, .. } => {
                eprintln!("value | sender: {sender:?}");
                panic!()
            }
            super::AnyMessage::LocalRequest { .. } => todo!("local request"),
            super::AnyMessage::AgentRemoved(key) => Ok(BridgeMessage::AgentRemoved(key.into())),
        }
    }

    pub(crate) async fn send(&self, recipient: BridgeKey, msg: BridgeMessage) -> Result<()> {
        let address = InternalAddress::Local(recipient.consume().into()).into();
        self.0.send_local(&address, msg).await
    }

    pub(crate) fn serializer(&self) -> Serializer {
        self.0.serializer
    }

    pub(crate) fn serialize(&self, value: &impl Serialize) -> Result<Box<[u8]>> {
        self.0.serializer.serialize(value)
    }

    pub(crate) async fn track(&self, key: BridgeKey) -> Result<()> {
        let addr = InternalAddress::Local(key.into()).into();
        self.0.track(&addr).await
    }

    pub(crate) async fn remove_self(self) {
        let _ = self.0.remove_self().await;
    }
}
