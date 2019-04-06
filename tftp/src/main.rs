use crate::client::Client;
use crate::server::Server;

mod server;
mod client;
mod tftp;

const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:34254";

fn main() -> std::io::Result<()> {
    match std::env::args().nth(1).unwrap_or(String::new()).as_ref() {
        "upload" => {
            Client::new().upload("", DEFAULT_SERVER_ADDR)?;
        }
        "download" => {
            Client::new().download("", DEFAULT_SERVER_ADDR)?;
        }
        "send" => {
            Server::new().send("", DEFAULT_SERVER_ADDR)?;
        }
        "recv" => {
            Server::new().recv("", DEFAULT_SERVER_ADDR)?;
        }
        "" => {
            println!("no command is given.");
        }
        s => {
            println!("invalid command: {}", s);
        }
    }
    Ok(())
}
