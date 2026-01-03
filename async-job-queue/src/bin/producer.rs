use async_job_queue::{Job, Priority, Storage};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "producer")]
#[command(about = "Job queue producer - submit and manage jobs")]
struct Cli {
    #[arg(short, long, default_value = "jobs.db")]
    database: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Submit {
        #[arg(short, long)]
        payload: String,

        #[arg(short = 'r', long, default_value = "normal")]
        priority: String,

        #[arg(short, long, default_value = "3")]
        max_retries: u32,
    },
    Status {
        #[arg(short, long)]
        job_id: String,
    },
    Stats,
}

fn parse_priority(s: &str) -> Priority {
    match s.to_lowercase().as_str() {
        "low" => Priority::Low,
        "normal" => Priority::Normal,
        "high" => Priority::High,
        "critical" => Priority::Critical,
        _ => Priority::Normal,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let storage = Arc::new(Storage::new(&cli.database)?);

    match cli.command {
        Commands::Submit {
            payload,
            priority,
            max_retries,
        } => {
            let priority = parse_priority(&priority);
            let job = Job::new(payload.into_bytes(), priority, max_retries);

            println!("Submitting job {} with priority {}", job.id, priority);
            storage.insert(&job)?;
            println!("Job submitted successfully!");
            println!("Job ID: {}", job.id);
        }
        Commands::Status { job_id } => {
            let uuid = Uuid::parse_str(&job_id)?;
            match storage.get_by_id(uuid)? {
                Some(job) => {
                    println!("Job ID: {}", job.id);
                    println!("Status: {}", job.status);
                    println!("Priority: {}", job.priority);
                    println!("Retries: {}/{}", job.retry_count, job.max_retries);
                    println!("Created: {}", job.created_at);
                    println!("Updated: {}", job.updated_at);
                    if let Some(error) = &job.error_message {
                        println!("Error: {}", error);
                    }
                    println!("Payload: {}", String::from_utf8_lossy(&job.payload));
                }
                None => {
                    println!("Job not found: {}", job_id);
                }
            }
        }
        Commands::Stats => {
            use async_job_queue::JobStatus;

            println!("Job Queue Statistics:");
            println!(
                "  Pending: {}",
                storage.count_by_status(JobStatus::Pending)?
            );
            println!(
                "  Running: {}",
                storage.count_by_status(JobStatus::Running)?
            );
            println!(
                "  Completed: {}",
                storage.count_by_status(JobStatus::Completed)?
            );
            println!("  Failed: {}", storage.count_by_status(JobStatus::Failed)?);
            println!(
                "  Dead Letter: {}",
                storage.count_by_status(JobStatus::DeadLetter)?
            );
        }
    }

    Ok(())
}
