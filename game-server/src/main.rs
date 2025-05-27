use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc, // Using std Arc, but with tokio::sync::Mutex
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, mpsc}, // Tokio's Mutex and MPSC
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error as TungsteniteError, Message as TungsteniteMessage},
};
use futures_util::{StreamExt, SinkExt}; // For .split(), .next(), .send()
use rand::Rng; // For generating client IDs

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Client {
    id: usize,
    position_x: i32,
    position_y: i32,
    #[serde(skip)] // Avoid serializing the sender
    // Sender for messages to be sent to this client's WebSocket stream
    sender: Option<mpsc::UnboundedSender<Result<TungsteniteMessage, TungsteniteError>>>,
}

type GameState = Arc<Mutex<HashMap<usize, Client>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_addr = "127.0.0.1:9002";
    let game_state = GameState::new(Mutex::new(HashMap::new()));

    let listener = TcpListener::bind(&server_addr).await?;
    println!("Game server listening on: {}", server_addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection from: {}", addr);
                let game_state_clone = game_state.clone();
                tokio::spawn(async move { // Spawn an async block to handle connection
                    if let Err(e) = handle_connection(game_state_clone, stream, addr).await {
                        eprintln!("Connection handler for {} error: {:?}", addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
            }
        }
    }
    // Ok(()) // main loop is infinite, so Ok(()) is unreachable
}

// Updated handle_connection function for Part 2a
async fn handle_connection(
    state: GameState,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake error for {}: {}", addr, e);
            return Ok(()); // Connection closed due to handshake error
        }
    };
    println!("WebSocket connection established: {}", addr);

    // Ensure rand::Rng is in scope (e.g., `use rand::Rng;` at the top of the file)
    let client_id: usize = rand::thread_rng().gen_range(1000..10000); 

    // Create an MPSC channel for sending messages to this client
    let (tx, _rx) = mpsc::unbounded_channel::<Result<TungsteniteMessage, TungsteniteError>>(); // _rx will be used in Part 2b
    
    let new_client = Client {
        id: client_id,
        position_x: 0, // Initial position
        position_y: 0,
        sender: Some(tx.clone()), // Store the sender
    };

    state.lock().await.insert(client_id, new_client.clone());
    println!("Client {} ({}) registered.", client_id, addr);

    let (_ws_sender, _ws_receiver) = ws_stream.split(); // _ws_sender and _ws_receiver will be used in later parts

    // TODO (Part 2b): Spawn task for sending messages (reading from _rx -> _ws_sender)
    // TODO (Part 2c): Loop for receiving messages (reading from _ws_receiver)
    
    // For now, just keep the connection open until explicitly closed or error.
    // A real implementation would await on send/receive tasks.
    // We'll simulate keeping it alive by a placeholder if needed, or just let it drop for now.
    // For this part, we are just testing registration.
    // The function will return, and the connection will drop if nothing holds it.
    // In later steps, loops will keep it alive.

    println!("Client {} ({}) connection handler part 2a finished. Placeholders for loops.", client_id, addr);
    
    Ok(())
}


// Placeholder for periodic broadcast - to be implemented in Part 3
// async fn periodic_broadcast(state: GameState) {
//     // TODO
// }
