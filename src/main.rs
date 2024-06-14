mod a {
    use binky::{Agent, Router, TcpListener};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub enum AddressA {
        A,
        Server,
    }

    pub async fn run_a() {
        let mut router = Router::new();
        let listener = TcpListener::bind("127.0.0.1:8000").await.unwrap();
        router.listen(listener, AddressA::Server);

        let agent = router.agent(AddressA::A);
        tokio::spawn(incoming_msg(agent));

        router.run().await;
    }

    pub async fn incoming_msg(mut agent: Agent) {
        while let Ok(msg) = agent.recv::<String>().await {
            match msg {
                binky::AgentMessage::Value { value, sender } => {
                    // eprintln!("{value}");
                    agent.send(&sender, value).await;
                }
                binky::AgentMessage::Request { request, sender } => {
                    panic!("if you can read this, then things are actually quite good");
                    // let (a, b) = request.read::<(u32, u32)>().unwrap();
                }
                binky::AgentMessage::AgentRemoved(_) => todo!(),
            }
        }
    }
}

mod b {
    use binky::{Agent, Router, TcpStream};
    use serde::{Deserialize, Serialize};

    use crate::a::AddressA;

    #[derive(Debug, Serialize, Deserialize)]
    enum AddressB {
        B,
        C,
        Connection,
    }

    pub async fn run_b() {
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let mut router = Router::new();
        let stream = TcpStream::connect("127.0.0.1:8000").await.unwrap();
        router.connect(stream, AddressB::Connection);
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;

        let agent_b = router.agent(AddressB::B);
        let agent_c = router.agent(AddressB::C);
        tokio::spawn(outgoing_msg(agent_b));
        tokio::spawn(crate::a::incoming_msg(agent_c));

        router.run().await;
    }

    async fn outgoing_msg(mut agent: Agent) {
        let Ok(bridge) = agent.resolve(AddressB::Connection).await else { panic!() };

        let Ok(remote) = agent.resolve_remote(bridge, AddressA::A).await else { panic!() };
        // let Ok(local) = agent.resolve(AddressB::C).await else { panic!() };

        let result: u32 = agent.request(&remote, (1u32, 2u32)).await.unwrap();

        // agent.send(local, "hello world".to_string()).await;

        while let Ok(msg) = agent.recv::<String>().await {
            match msg {
                binky::AgentMessage::Value { value, sender } => {
                    eprintln!("{value}");
                }
                binky::AgentMessage::Request { request, sender } => todo!(),
                binky::AgentMessage::AgentRemoved(_) => todo!(),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let handle_a = tokio::spawn(a::run_a());
    let handle_b = tokio::spawn(b::run_b());

    handle_a.await;
    handle_b.await;
}
