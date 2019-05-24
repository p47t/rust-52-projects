use failure::{err_msg, Error};

use crate::client::Client;
use crate::server::Server;

mod server;
mod client;
mod tftp;

const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:34254";

fn main() -> Result<(), Error> {
    let command = std::env::args().nth(1).ok_or(err_msg("No command"))?;
    let filename = std::env::args().nth(2).ok_or(err_msg("No filename"))?;
    match command.as_ref() {
        "upload" => {
            Client::new().upload(filename.as_ref(), DEFAULT_SERVER_ADDR)?;
        }
        "download" => {
            Client::new().download(filename.as_ref(), DEFAULT_SERVER_ADDR)?;
        }
        "send" => {
            Server::new().send(filename.as_ref(), DEFAULT_SERVER_ADDR)?;
        }
        "recv" => {
            Server::new().recv(filename.as_ref(), DEFAULT_SERVER_ADDR)?;
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

#[cfg(test)]
mod tests {
    use crate::client::Client;
    use crate::DEFAULT_SERVER_ADDR;
    use crate::server::Server;

    #[test]
    #[ignore]
    fn test_download() {
        let server = std::thread::spawn(|| {
            let server = Server::new();
            let _ = server.send("README.md", DEFAULT_SERVER_ADDR);
        });
        let client = Client::new();
        let _ = client.download("README-downloaded.md", DEFAULT_SERVER_ADDR);
        let _ = server.join();
    }

    #[test]
    #[ignore]
    fn test_upload() {
        let server = std::thread::spawn(|| {
            let server = Server::new();
            let _ = server.recv("README-uploaded.md", DEFAULT_SERVER_ADDR);
        });
        let client = Client::new();
        let _ = client.upload("README.md", DEFAULT_SERVER_ADDR);
        let _ = server.join();
    }
}