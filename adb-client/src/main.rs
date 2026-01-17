use adb_client::AdbClient;
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "adb-client")]
#[command(about = "ADB client - Android Debug Bridge protocol implementation in Rust")]
struct Cli {
    /// ADB server host address.
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// ADB server port.
    #[arg(short = 'P', long, default_value_t = 5037)]
    port: u16,

    /// Target device serial number.
    #[arg(short, long)]
    serial: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Get ADB server version.
    Version,

    /// List connected devices.
    Devices,

    /// Run a shell command on the device.
    Shell {
        /// Shell command to execute.
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Push a local file to the device.
    Push {
        /// Local file path.
        local: PathBuf,
        /// Remote file path on device.
        remote: String,
    },

    /// Pull a file from the device.
    Pull {
        /// Remote file path on device.
        remote: String,
        /// Local file path.
        local: PathBuf,
    },

    /// Stream device logs (logcat).
    Logcat,

    /// Stat a remote file on the device.
    Stat {
        /// Remote path on device.
        path: String,
    },

    /// List a remote directory on the device.
    Ls {
        /// Remote directory path on device.
        path: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            Cli::command().print_help()?;
            println!();
            return Ok(());
        }
    };

    let client = AdbClient::with_address(&cli.host, cli.port);

    match command {
        Commands::Version => {
            let version = client.server_version().await?;
            println!("ADB server version: {}", version);
        }
        Commands::Devices => {
            let devices = client.list_devices().await?;
            if devices.is_empty() {
                println!("No devices connected.");
            } else {
                println!("{:<24} State", "Serial");
                for d in &devices {
                    println!("{:<24} {}", d.serial, d.state);
                }
            }
        }
        Commands::Shell { command } => {
            if command.is_empty() {
                eprintln!("Error: no shell command specified");
                std::process::exit(1);
            }
            let cmd = command.join(" ");
            let output = client.shell(cli.serial.as_deref(), &cmd).await?;
            print!("{}", output);
        }
        Commands::Push { local, remote } => {
            client.push(cli.serial.as_deref(), &local, &remote).await?;
            println!("Pushed {} -> {}", local.display(), remote);
        }
        Commands::Pull { remote, local } => {
            client.pull(cli.serial.as_deref(), &remote, &local).await?;
            println!("Pulled {} -> {}", remote, local.display());
        }
        Commands::Logcat => {
            let mut stream = client.logcat(cli.serial.as_deref()).await?;
            let mut stdout = tokio::io::stdout();
            tokio::io::copy(&mut stream, &mut stdout).await?;
        }
        Commands::Stat { path } => {
            let stat = client.stat(cli.serial.as_deref(), &path).await?;
            println!("Mode:     {:o}", stat.mode);
            println!("Size:     {} bytes", stat.size);
            println!("Modified: {} (unix timestamp)", stat.mtime);
            if stat.is_file() {
                println!("Type:     regular file");
            } else if stat.is_directory() {
                println!("Type:     directory");
            }
        }
        Commands::Ls { path } => {
            let entries = client.list_dir(cli.serial.as_deref(), &path).await?;
            if entries.is_empty() {
                println!("(empty)");
            } else {
                for entry in &entries {
                    let type_char = if (entry.mode & 0o170000) == 0o040000 {
                        'd'
                    } else {
                        '-'
                    };
                    println!(
                        "{}{:o}  {:>8}  {}",
                        type_char,
                        entry.mode & 0o7777,
                        entry.size,
                        entry.name
                    );
                }
            }
        }
    }

    Ok(())
}
