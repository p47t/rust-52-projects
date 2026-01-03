use async_job_queue::{JobHandler, Storage, WorkerPool};
use clap::Parser;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(name = "worker")]
#[command(about = "Job queue worker - process jobs from the queue")]
struct Cli {
    #[arg(short, long, default_value = "jobs.db")]
    database: String,

    #[arg(short, long, default_value = "4")]
    workers: usize,
}

struct EchoHandler;

impl JobHandler for EchoHandler {
    fn handle(&self, payload: &[u8]) -> Result<(), String> {
        let message = String::from_utf8_lossy(payload);
        info!("Processing job with payload: {}", message);

        // Simulate work
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Simulate occasional failures for testing retry logic
        if message.contains("fail") {
            return Err("Simulated failure".to_string());
        }

        println!("Processed: {}", message);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    info!("Starting worker with database: {}", cli.database);

    let storage = Arc::new(Storage::new(&cli.database)?);
    let handler = Arc::new(EchoHandler);

    let pool = WorkerPool::new(storage, handler, cli.workers);

    info!("Worker pool initialized with {} workers", cli.workers);

    pool.run().await?;

    Ok(())
}
