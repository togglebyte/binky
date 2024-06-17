use std::any::Any;

use flume::Receiver;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::AsyncReadExt;

use super::{AgentMessage, AnyMessage};
use crate::address::Address;
use crate::error::{Error, Result};
use crate::retry::Timeout;
use crate::router::{RouterCtx, RouterMessage};
use crate::serializer::Serializer;
use crate::slab::{AgentKey, BridgeKey};
use crate::value::{AnyValue, Initial, RemoteVal};
use crate::Stream;

/// Agent..
pub struct Agent {
    key: AgentKey,
    router_ctx: RouterCtx,
    pub(super) rx: Receiver<AnyMessage>,
    // This is the local serilizer that is used by the router
    // that spawned this agent.
    pub(crate) serializer: Serializer,
}

/// An agent
impl Agent {
    pub(crate) fn new(
        key: AgentKey,
        router_ctx: RouterCtx,
        rx: Receiver<AnyMessage>,
        serializer: Serializer,
    ) -> Self {
        Self {
            key,
            router_ctx,
            rx,
            serializer,
        }
    }

    /// Get the local address to this agent
    pub fn address(&self) -> Address {
        Address::Local(self.key)
    }

    /// Get the key for the agent
    pub fn key(&self) -> AgentKey {
        self.key
    }

    /// Send a message to an address
    /// ```
    /// # use binky::{Agent, Router};
    /// # async fn run() {
    /// let mut router = Router::new();
    /// let agent_a = router.agent(None::<()>);
    /// let mut agent_b = router.agent(Some(1));
    ///
    /// let b_recipient = agent_a.resolve(1).await.unwrap();
    /// agent_a.send(&b_recipient, "hi".to_string()).await;
    /// agent_b.recv::<String>().await;
    /// # }
    /// ```
    pub async fn send<U: Serialize + Send + 'static>(
        &self,
        recipient: &Address,
        value: U,
    ) -> Result<()> {
        match recipient {
            Address::Local(agent_key) => {
                let value: AnyValue = Box::new(value);
                let message = RouterMessage::value(self.key, *agent_key, value);
                self.router_ctx.send(message).await?;
            }
            Address::Remote {
                local_bridge_key,
                remote_address: recipient,
                remote_serializer,
            } => {
                let payload = remote_serializer.serialize(&value)?;
                let value = RemoteVal::new(payload, self.key, *local_bridge_key, recipient.clone());
                let message = RouterMessage::remote_value(value);
                self.router_ctx.send(message).await?;
            }
        }
        Ok(())
    }

    /// Sync version of `Self::send`
    pub fn send_sync<U: Serialize + Send + 'static>(
        &self,
        recipient: &Address,
        value: U,
    ) -> Result<()> {
        match recipient {
            Address::Local(agent_key) => {
                let value: AnyValue = Box::new(value);
                let message = RouterMessage::value(self.key, *agent_key, value);
                self.router_ctx.send_sync(message)?;
            }
            Address::Remote {
                local_bridge_key,
                remote_address: recipient,
                remote_serializer,
            } => {
                let payload = remote_serializer.serialize(&value)?;
                let value = RemoteVal::new(payload, self.key, *local_bridge_key, recipient.clone());
                let message = RouterMessage::remote_value(value);
                self.router_ctx.send_sync(message)?;
            }
        }
        Ok(())
    }

    /// Send a local message to an address
    /// ```
    /// # use binky::{Agent, Router};
    /// # async fn run() {
    /// let mut router = Router::new();
    /// let agent_a = router.agent(None::<()>);
    /// let mut agent_b = router.agent(Some(1));
    ///
    /// let b_recipient = agent_a.resolve(1).await.unwrap();
    /// agent_a.send_local(&b_recipient, "hi".to_string()).await;
    /// agent_b.recv::<String>().await;
    /// # }
    /// ```
    pub async fn send_local<U: Send + 'static>(&self, recipient: &Address, value: U) -> Result<()> {
        match recipient {
            Address::Local(agent_key) => {
                let value: AnyValue = Box::new(value);
                let message = RouterMessage::value(self.key, *agent_key, value);
                self.router_ctx.send(message).await?;
            }
            Address::Remote { .. } => return Err(Error::RemoteActionOnLocal),
        }
        Ok(())
    }

    /// Track a local agent
    pub async fn track(&self, address: &Address) -> Result<()> {
        match address {
            Address::Local(key) => self.router_ctx.track(self.key(), *key).await,
            Address::Remote { .. } => Err(Error::LocalOnly),
        }
    }

    /// Remove self
    pub async fn remove_self(&self) -> Result<()> {
        self.router_ctx.remove(self.key()).await
    }

    /// Receive a message that was sent to this agent's address.
    /// The message can be either local or remote.
    /// For local only messages see `Agent::recv_local`.
    /// ```
    /// # use binky::Agent;
    /// # async fn run(mut agent: Agent) {
    /// let msg = agent.recv::<()>().await;
    /// # }
    /// ```
    pub async fn recv<T: DeserializeOwned + Any>(&mut self) -> Result<AgentMessage<T>> {
        let any_message = self.rx.recv_async().await?;
        let msg = any_message.to_agent_message(self.serializer)?;
        Ok(msg)
    }

    /// Sync version of `Self::recv`
    pub fn recv_sync<T: DeserializeOwned + Any>(&mut self) -> Result<AgentMessage<T>> {
        let any_message = self.rx.recv()?;
        let msg = any_message.to_agent_message(self.serializer)?;
        Ok(msg)
    }

    /// Receive a local message that was sent to this agent's address.
    /// ```
    /// # use binky::Agent;
    /// # async fn run(mut agent: Agent) {
    /// let msg = agent.recv::<()>().await;
    /// # }
    /// ```
    pub async fn recv_local<T: Any>(&mut self) -> Result<AgentMessage<T>> {
        let any_message = self.rx.recv_async().await?;
        let msg = any_message.to_local_agent_message()?;
        Ok(msg)
    }

    /// Make a request to another agent
    /// ```
    /// # use binky::Agent;
    /// # async fn async_run(mut agent: Agent) {
    /// # let address = agent.resolve(()).await.unwrap();
    /// let request = agent.request::<String>(&address, 123).await.unwrap();
    /// # }
    /// ```
    pub async fn request<T: DeserializeOwned + Any>(
        &self,
        recipient: &Address,
        value: impl Send + 'static,
    ) -> Result<T> {
        match recipient {
            Address::Local(recipient) => {
                let value = Box::new(value);
                let (response, msg) = RouterMessage::local_request(self.key, *recipient, value);
                self.router_ctx.send(msg).await?;
                let response = response.consume().await?;
                response
                    .downcast::<T>()
                    .map_err(|e| Error::InvalidValueType)
                    .map(|val| *val)
            }
            Address::Remote {
                local_bridge_key,
                remote_address,
                remote_serializer,
            } => {
                panic!()
            }
        }
    }

    /// Shut down the local router.
    /// This does not have any impact on remote routers other than
    /// disconnecting from said routers.
    pub async fn shutdown(self) -> Result<()> {
        let message = RouterMessage::shutdown();
        self.router_ctx.send(message).await?;
        Ok(())
    }

    /// Resolve an address to a local agent
    pub async fn resolve(&self, address: impl Serialize) -> Result<Address> {
        let address = self.router_ctx.serialize(&address)?;
        let key = self.router_ctx.lookup_address(address).await?;
        Ok(Address::Local(key))
    }

    /// Resolve an address to a local agent with a retry
    pub async fn resolve_with_retry(
        &self,
        address: impl Serialize,
        mut retry: Timeout,
    ) -> Result<Address> {
        let address = self.router_ctx.serialize(&address)?;
        let key = loop {
            match self
                .router_ctx
                .lookup_address(address.clone())
                .await
                .map(Address::Local)
            {
                Ok(key) => break key,
                Err(Error::AddressNotFound) => {
                    retry.sleep().await?;
                    continue;
                }
                e @ Err(_) => return e,
            }
        };

        Ok(key)
    }

    /// Resolve an address to a remote agent using a local bridge
    ///
    /// # Panics
    ///
    /// Panics if the bridge address is a remote address
    pub async fn resolve_remote(
        &self,
        bridge: Address,
        address: impl Serialize,
    ) -> Result<Address> {
        let bridge: BridgeKey = match bridge {
            Address::Local(bridge) => bridge.into(),
            Address::Remote { .. } => panic!("can not use a remote address as the bridge"),
        };

        let (rx, msg) = RouterMessage::get_serializer(bridge);
        self.router_ctx.send(msg).await?;
        let remote_serializer = rx.recv_async().await??;

        // Now that we have the serializer we can make the request:
        let address = remote_serializer.serialize(&address)?;
        let (rx, msg) = RouterMessage::resolve_remote(self.key(), bridge, address);

        self.router_ctx.send(msg).await?;
        let remote_address = rx.recv_async().await??;
        Ok(Address::Remote {
            local_bridge_key: bridge,
            remote_address,
            remote_serializer,
        })
    }

    async fn new_agent(&self, address: impl Serialize, cap: Option<usize>) -> Result<Agent> {
        let address = self.router_ctx.serialize(&address)?;
        let (rx, msg) = RouterMessage::new_agent(Some(address), cap, self.serializer);
        self.router_ctx.send(msg).await?;
        Ok(rx.recv_async().await?)
    }

    /// Create a new agent with a given address and no capacity
    pub async fn agent(&self, address: impl Serialize) -> Result<Agent> {
        self.new_agent(address, None).await
    }

    /// Create a new agent with a given address and capacity
    pub async fn agent_with_capacity(&self, address: impl Serialize, cap: usize) -> Result<Agent> {
        self.new_agent(address, Some(cap)).await
    }

    /// Remove a local agent
    pub async fn remove_agent(&self, key: AgentKey) -> Result<()> {
        let msg = RouterMessage::remove_agent(key);
        self.router_ctx.send(msg).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Router;

    #[derive(Debug, Serialize)]
    enum Address {
        Origin,
        NewAgent,
    }

    #[tokio::test]
    async fn new_agent_no_capacity() {
        let mut router = Router::new();
        let a = router.agent(Address::Origin);
        tokio::spawn(router.run());

        let b = a.agent(Address::NewAgent).await.unwrap();
    }

    #[tokio::test]
    async fn new_agent_with_capacity() {
        let mut router = Router::new();
        let a = router.agent(Address::Origin);
        tokio::spawn(router.run());

        let b = a.agent_with_capacity(Address::NewAgent, 12).await.unwrap();
    }
}
