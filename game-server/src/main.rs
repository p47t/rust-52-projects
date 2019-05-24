use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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
}

fn main() -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new().build()?;
    let executor = runtime.executor();
    let server = Server::bind("127.0.0.1:8080", &tokio::reactor::Handle::default())?;

    let connections = Arc::new(RwLock::new(HashMap::new()));
    let entities = Arc::new(RwLock::new(HashMap::new()));
    let counter = Arc::new(RwLock::new(0));

    let conn_handler = {
        let connections = connections.clone();
        let entities = entities.clone();
        let counter = counter.clone();
        let executor = executor.clone();

        server.incoming()
            .map_err(|InvalidConnection { error, .. }| error)
            .for_each(move |(upgrade, _addr)| {
                let accept = {
                    let connections = connections.clone();
                    let entities = entities.clone();
                    let counter = counter.clone();
                    let executor = executor.clone();

                    upgrade.accept().and_then(move |(framed, _)| {
                        let (sink, stream) = framed.split();

                        // generate an ID for the client
                        {
                            let mut c = counter.write().unwrap();
                            *c += 1;
                        }
                        let id = *counter.read().unwrap();

                        // add id to Sink mapping
                        connections.write().unwrap().insert(id, sink);
                        // add id to Entity mapping
                        entities.write().unwrap().insert(id, Entity::default());

                        // spawn a task to handle message from this client
                        let fut = stream.for_each(move |msg| {
                            process_message(id, &msg, entities.clone());
                            Ok(())
                        }).map_err(|_| ());
                        executor.spawn(fut);

                        Ok(())
                    }).map_err(|_| ())
                };
                executor.spawn(accept);
                Ok(())
            }).map_err(|_| ())
    };

    let send_handler = {
        let connections = connections.clone();
        let entities = entities.clone();
        let executor = executor.clone();

        futures::future::loop_fn((), move |_| {
            let connections = connections.clone();
            let entities = entities.clone();
            let executor = executor.clone();

            tokio::timer::Delay::new(Instant::now() + Duration::from_millis(100))
                .map_err(|_| ())
                .and_then(move |_| {
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
                        executor.spawn(fut);
                    }

                    Ok(Loop::Continue(()))
                })
        })
    };

    runtime.block_on_all(conn_handler.select(send_handler))
        .map_err(|_| println!("Error while running core loop"))
        .unwrap();

    Ok(())
}

fn process_message(_id: u32, _msg: &OwnedMessage, _entities: Arc<RwLock<HashMap<u32, Entity>>>) {}