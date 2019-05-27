use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use failure::Error;
use futures::{Future, Stream};
use futures::future::Loop;
use futures::sink::Sink;
use websocket::OwnedMessage;
use websocket::server::InvalidConnection;
use websocket::server::r#async::Server;

#[derive(Default)]
pub struct Entity {
    pub id: u32,
    pub pos: (i32, i32),
}

impl Entity {
    pub fn to_json(&self) -> String {
        format!("{{\"position\":{{\"x\":{},\"y\":{}}}, \"id\":{}}}", self.pos.0, self.pos.1, self.id)
    }

    fn process_message(&mut self, txt: &str) {
        match txt {
            "R" => { self.pos.0 += 10; }
            "L" => { self.pos.0 -= 10; }
            "D" => { self.pos.1 += 10; }
            "U" => { self.pos.1 -= 10; }
            _ => {}
        }
    }
}

fn main() -> Result<(), Error> {
    let server = Server::bind("127.0.0.1:8080", &tokio::reactor::Handle::default())?;

    let connections = Arc::new(RwLock::new(HashMap::new()));
    let entities = Arc::new(RwLock::new(HashMap::new()));
    let counter = Arc::new(RwLock::new(0));

    let conn_handler = {
        let connections = connections.clone();
        let entities = entities.clone();
        let counter = counter.clone();

        server.incoming()
            .map_err(|InvalidConnection { error, .. }| error)
            .for_each(move |(upgrade, addr)| {
                println!("client addr: {}", addr);

                let accept = {
                    let connections = connections.clone();
                    let entities = entities.clone();
                    let counter = counter.clone();

                    upgrade.accept().and_then(move |(framed, _)| {
                        let (sink, stream) = framed.split();

                        // generate an ID for the client
                        {
                            let mut c = counter.write().unwrap();
                            *c += 1;
                        }
                        let id = *counter.read().unwrap();
                        println!("new client {}", id);

                        // add id to Sink mapping
                        connections.write().unwrap().insert(id, sink);
                        // add id to Entity mapping
                        entities.write().unwrap().insert(id, Entity { id, ..Default::default() });

                        // spawn a task to handle message from this client
                        let fut = stream.for_each(move |msg| {
                            println!("message for client {}", id);
                            process_message(id, &msg, entities.clone());
                            Ok(())
                        }).map_err(|_| ());
                        tokio::spawn(fut);

                        Ok(())
                    }).map_err(|_| ())
                };
                tokio::spawn(accept);
                Ok(())
            }).map_err(|_| ())
    };

    let send_handler = {
        let connections = connections.clone();
        let entities = entities.clone();

        futures::future::loop_fn((), move |_| {
            let connections = connections.clone();
            let entities = entities.clone();

            tokio::timer::Delay::new(Instant::now() + Duration::from_millis(100))
                .map_err(|_| ())
                .and_then(move |_| {
                    println!("thread {:?}: broadcast game state", thread::current().id());
                    let mut conn = connections.write().unwrap();

                    let ids = conn.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
                    for id in ids.iter() {
                        let sink = conn.remove(id).unwrap();
                        let entities = entities.read().unwrap();

                        // generate JSON for all entities
                        let first = match entities.iter().take(1).next() {
                            Some((_, e)) => e,
                            None => return Ok(Loop::Continue(())),
                        };
                        let entities_json = format!("[{}]", entities.iter().skip(1)
                            .map(|(_, e)| e.to_json())
                            .fold(first.to_json(), |acc, s| format!("{},{}", s, acc)));

                        // spawn a task to send the game state to the client
                        let fut = {
                            let connections = connections.clone();
                            let id = id.clone();

                            sink.send(OwnedMessage::Text(entities_json))
                                .and_then(move |sink| {
                                    // Re-insert the entry to the connections map
                                    connections.write().unwrap().insert(id.clone(), sink);
                                    Ok(())
                                })
                                .map_err(|_| ())
                        };
                        tokio::spawn(fut);
                    }

                    Ok(Loop::Continue(()))
                })
        })
    };

    tokio::runtime::current_thread::block_on_all(conn_handler.select(send_handler))
        .map_err(|_| println!("Error while running core loop"))
        .unwrap();

    Ok(())
}

fn process_message(id: u32, msg: &OwnedMessage, entities: Arc<RwLock<HashMap<u32, Entity>>>) {
    if let OwnedMessage::Text(ref txt) = *msg {
        entities.write().unwrap().entry(id).and_modify(|e| e.process_message(txt));
    }
}