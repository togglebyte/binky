# Router
* Route internal messages to agents
* Route external messages to internal agents
* Route external messages to external router
* Listen for incoming messages (support multiple endpoints)

# Agent
* Send messages to an address (has to be resolved first)
* Reply to requests
* Connect to a remote router
* Can only receive remote messages not send
* Connecting to a remote creates a send handle for sending remote messages
* Resolve / Resolve remote

# Remote Agent (might not be required)
* Send messages to a address on a remote router
* Can only be connected to one router at a time

# Server / Listener
* Split into reader / writer halves
* Configurable max cap
* Configurable timeout (heartbeat)
* Associate an agent with each write half
* `read` function + router tx for each read half
    * build up message and send to address via router tx
    * the `read`er has to know the `write`r agents address
    * Reader should `select!` over timeout / `msg recv`
    * If timeout happens then remote shutdown the writer agent
    * `fn read(reader: ReadHalf, router_tx: RouterTx, writer_key: Key)`

# Message framing
* Read header
* Header will dictate
    * Message size
* First read header into a `Header`
* Second allocate size for the bytes

```rust
router.listen(TcpListener::bind(..));

```

```rust
let remote = RemoteAgent::connect(connection, heartbeat);
remote.send(msg).await;
```

```rust
let router = mut Router::new();
let tcp_listener = TcpListener::bind("123....");
router.listen(tcp_listener, MessageFormat::Text, Some(MAX_CON));

let agent: Agent<Address, T> = router.new_agent(Address);
let remote_agent = RemoteAgent::connect("remote router a", heartbeat, Reconnect::Never).await;

async fn use_agent(agent: Agent<Address, T>) {
    let remote_agent = RemoteAgent::connect("..", hb, recon).await;
    
    remote_agent.recv().await,
    agent.recv().await
    
    let tcp_adapter = TcpAdapter::connect("<some remote address>" heartbeat, reconnect_strat).await;
    let uds_adapter = UnixAdapter::connect("/some/path", heartbeat, reconnect_strat).await;
    let tcp = agent.connect(tcp_adapter).await; 
    let uds = agent.connect(uds_adapter).await;
    
    let address = agent.resolve(Address::A).await.unwrap();
    let remote_address = agent.remote_resolve(tcp, other_lieberry::Address::A).await.unwrap();
    
    agent.send(address, "hello world").await;
    agent.send(remote_address, "hello world").await;
    
    while let Ok(msg) = agent.recv().await {
        match msg {
            Message::Whatever(data) => { }
        }
    }
}
```

# Questions
Q: How do we agree upon serialization
A: `Message` trait that has to/from bytes

Q: What happens when a remote agent loses connectivity
A: Reconnect strategy that is supplied to the remote agent upon creation

Q: What if we want to receive from a remote agent and a local agent?
A: An agent receives messages, the origin is in the sender

Q: How do we decide how to ser/deser?
A: The listener associated with the router is in charge of that

Q: What is life cycle of a remote message?
A: Adapter receives message (as bytes), message is passed to the agent.
   The agent knows the message type and can try to deserialize the message
   before invoking the recv
