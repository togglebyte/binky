// use binky::*;
// use serde::{Deserialize, Serialize};

// #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
// enum A {
//     Client,
//     Server,
// }

// async fn second() {
//     timeout().duration_ms(100).sleep().await;
//     let mut router = Router::new();
//     let stream = TcpStream::connect("127.0.0.1:12345").await.unwrap();
//     let mut agent = router.agent("idontcare");

//     let router_task = tokio::spawn(router.run());

//     let session = agent.connect(stream, A::Client, None).await.unwrap();

//     let bridge = agent.resolve(A::Client).await.unwrap();
//     let remote_addr = agent.resolve_remote(bridge, A::Server).await.unwrap();

//     let payload = String::from("hello world");
//     eprintln!("sending message");
//     agent.send(&remote_addr, payload).await;

//     {
//         eprintln!("disconnecting");
//         let connection_addr = agent.resolve(A::Client).await.unwrap();
//         agent.remove_agent(connection_addr).await;
//         eprintln!("disconnected");
//     }

//     timeout().duration_ms(100).sleep().await;
//     eprintln!("slept before reconnecting");
//     let stream = TcpStream::connect("127.0.0.1:12345").await.unwrap();
//     eprintln!("reconnecting using another agent");
//     let session = agent.connect(stream, A::Client, Some(session)).await;

//     let msg = agent.recv::<String>().await;
//     eprintln!("{msg:?}");

//     router_task.await;
// }

// // NOTE this is the trouble maker
// async fn first() {
//     let mut router = Router::new();
//     let listener = TcpListener::bind("127.0.0.1:12345").await.unwrap();
//     router.listen(listener);

//     let mut agent = router.agent(A::Server);
//     let router_task = tokio::spawn(router.run());

//     let Ok(AgentMessage::Value { value: msg, sender }) = agent.recv::<String>().await else {
//         panic!("all is lost!")
//     };
//     eprintln!("message was: {msg}");

//     timeout().duration_ms(100).sleep().await;

//     eprintln!("sending reply");
//     agent.send(&sender, "You love cats".to_string()).await;

//     router_task.await.unwrap();
// }

#[tokio::main]
async fn main() {
//     let handle = tokio::spawn(first());
//     second().await;
//     handle.await;
}
