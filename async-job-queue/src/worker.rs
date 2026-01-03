use crate::job::{JobHandler, JobStatus};
use crate::storage::Storage;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct WorkerPool {
    storage: Arc<Storage>,
    handler: Arc<dyn JobHandler>,
    num_workers: usize,
    poll_interval: Duration,
}

impl WorkerPool {
    pub fn new(storage: Arc<Storage>, handler: Arc<dyn JobHandler>, num_workers: usize) -> Self {
        Self {
            storage,
            handler,
            num_workers,
            poll_interval: Duration::from_secs(1),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting worker pool with {} workers", self.num_workers);

        let mut handles = vec![];

        for worker_id in 0..self.num_workers {
            let storage = Arc::clone(&self.storage);
            let handler = Arc::clone(&self.handler);
            let poll_interval = self.poll_interval;

            let handle = tokio::spawn(async move {
                worker_loop(worker_id, storage, handler, poll_interval).await;
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.await?;
        }

        Ok(())
    }
}

async fn worker_loop(
    worker_id: usize,
    storage: Arc<Storage>,
    handler: Arc<dyn JobHandler>,
    poll_interval: Duration,
) {
    info!("Worker {} started", worker_id);

    loop {
        match storage.get_next_pending() {
            Ok(Some(mut job)) => {
                info!("Worker {} processing job {}", worker_id, job.id);

                // Job is already marked as Running by get_next_pending()
                match handler.handle(&job.payload) {
                    Ok(()) => {
                        job.mark_completed();
                        info!("Worker {} completed job {}", worker_id, job.id);
                    }
                    Err(e) => {
                        warn!(
                            "Worker {} job {} failed (retry {}/{}): {}",
                            worker_id, job.id, job.retry_count, job.max_retries, e
                        );
                        job.mark_failed(e);

                        if job.status == JobStatus::DeadLetter {
                            error!("Job {} moved to dead letter queue", job.id);
                        }
                    }
                }

                if let Err(e) = storage.update(&job) {
                    error!("Worker {} failed to update job: {}", worker_id, e);
                }

                // Add exponential backoff for retried jobs
                if job.retry_count > 0 {
                    let backoff = Duration::from_secs(2_u64.pow(job.retry_count.min(5)));
                    sleep(backoff).await;
                }
            }
            Ok(None) => {
                // No jobs available, wait before polling again
                sleep(poll_interval).await;
            }
            Err(e) => {
                error!("Worker {} error fetching job: {}", worker_id, e);
                sleep(poll_interval).await;
            }
        }
    }
}
