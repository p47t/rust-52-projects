use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
pub struct Entity {
    pub id: u32,
    pub pos: (i32, i32),
}

impl Entity {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"position\":{{\"x\":{},\"y\":{}}}, \"id\":{}}}",
            self.pos.0, self.pos.1, self.id
        )
    }

    fn process_message(&mut self, txt: &str) {
        match txt {
            "R" => {
                self.pos.0 += 10;
            }
            "L" => {
                self.pos.0 -= 10;
            }
            "D" => {
                self.pos.1 += 10;
            }
            "U" => {
                self.pos.1 -= 10;
            }
            _ => {}
        }
    }
}

type Tx = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    Message,
>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Listening on 127.0.0.1:8080");

    let connections: Arc<RwLock<HashMap<u32, Tx>>> = Arc::new(RwLock::new(HashMap::new()));
    let entities: Arc<RwLock<HashMap<u32, Entity>>> = Arc::new(RwLock::new(HashMap::new()));
    let counter: Arc<RwLock<u32>> = Arc::new(RwLock::new(0));

    // Spawn the broadcast loop
    {
        let connections = connections.clone();
        let entities = entities.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                println!(
                    "thread {:?}: broadcast game state",
                    std::thread::current().id()
                );

                let entities_json = {
                    let entities = entities.read().await;
                    if entities.is_empty() {
                        continue;
                    }
                    let json_parts: Vec<String> = entities.values().map(|e| e.to_json()).collect();
                    format!("[{}]", json_parts.join(","))
                };

                let mut conns = connections.write().await;
                let ids: Vec<u32> = conns.keys().copied().collect();
                for id in ids {
                    if let Some(sink) = conns.get_mut(&id) {
                        if sink
                            .send(Message::Text(entities_json.clone().into()))
                            .await
                            .is_err()
                        {
                            conns.remove(&id);
                        }
                    }
                }
            }
        });
    }

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
        println!("client addr: {}", addr);

        let connections = connections.clone();
        let entities = entities.clone();
        let counter = counter.clone();

        tokio::spawn(async move {
            let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake error: {}", e);
                    return;
                }
            };

            let id = {
                let mut c = counter.write().await;
                *c += 1;
                *c
            };
            println!("new client {}", id);

            let (sink, mut stream) = ws_stream.split();

            connections.write().await.insert(id, sink);
            entities.write().await.insert(
                id,
                Entity {
                    id,
                    ..Default::default()
                },
            );

            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(txt) = msg {
                    println!("message for client {}", id);
                    entities
                        .write()
                        .await
                        .entry(id)
                        .and_modify(|e| e.process_message(&txt));
                }
            }

            // Client disconnected
            connections.write().await.remove(&id);
            entities.write().await.remove(&id);
            println!("client {} disconnected", id);
        });
    }
}
