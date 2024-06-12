// use std::time::Duration;

// use binky::{Agent, Router};
// use serde::{Deserialize, Serialize};
// use tokio::net::{TcpListener, TcpStream};

// #[derive(Debug, Serialize)]
// enum A {
//     A,
//     Other,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct OurMessage {
//     greeting: String,
// }

// async fn send_it_all(mut agent: Agent) {
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     let connection_key = agent.connect(TcpStream::connect("127.0.0.1:8000").await.unwrap()).await.unwrap();
//     let message = OurMessage { greeting: "hello you wonderful gherkins you !".to_string() };
//     let address_b = agent.resolve_remote(connection_key, B::B).await.unwrap();
//     // let address_other = agent.resolve(A::Other).await;
//     agent.send(address_b, message.clone()).await;
//     // agent.send(address_other, message);
// }

// async fn a() {
//     let mut router = Router::new();
//     let agent = router.agent(Some(A::A));
//     let handle = tokio::spawn(send_it_all(agent));
//     router.run().await;
//     handle.await.unwrap();
// }

// #[derive(Debug, Serialize)]
// enum B {
//     B
// }

// async fn b() {
//     let mut router = Router::new();
//     let mut agent_b = router.agent(Some(B::B));
//     let listener = TcpListener::bind("127.0.0.1:8000").await.unwrap();
//     router.listen(listener, None);
//     tokio::spawn(async move {
//         let Ok(msg) = agent_b.recv::<OurMessage>().await else { panic!("not good!") };
//         panic!("{msg:#?}");
//     });
//     router.run().await;
// }

// async fn run() {
//     let handle_1 = tokio::spawn(a());
//     let handle_2 = tokio::spawn(b());
//     handle_1.await.unwrap();
//     handle_2.await.unwrap();
// }

// #[test]
// fn listener() {
//     tokio::runtime::Builder::new_multi_thread()
//         .enable_all()
//         .build()
//         .unwrap()
//         .block_on(run());
// }
