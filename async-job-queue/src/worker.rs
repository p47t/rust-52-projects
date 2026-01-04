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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Job, Priority};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use tempfile::NamedTempFile;
    use tokio::time::timeout;

    // Test handler that always succeeds
    struct SuccessHandler {
        call_count: Arc<AtomicUsize>,
    }

    impl JobHandler for SuccessHandler {
        fn handle(&self, _payload: &[u8]) -> Result<(), String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    // Test handler that always fails
    struct FailHandler {
        call_count: Arc<AtomicUsize>,
    }

    impl JobHandler for FailHandler {
        fn handle(&self, _payload: &[u8]) -> Result<(), String> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Err("Test failure".to_string())
        }
    }

    // Test handler that fails N times then succeeds
    struct FailNTimesHandler {
        call_count: Arc<AtomicUsize>,
        fail_times: usize,
    }

    impl JobHandler for FailNTimesHandler {
        fn handle(&self, _payload: &[u8]) -> Result<(), String> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            if count < self.fail_times {
                Err(format!("Failure {}", count + 1))
            } else {
                Ok(())
            }
        }
    }

    // Test handler that tracks which payloads were processed
    struct TrackingHandler {
        processed: Arc<StdMutex<Vec<Vec<u8>>>>,
    }

    impl JobHandler for TrackingHandler {
        fn handle(&self, payload: &[u8]) -> Result<(), String> {
            self.processed.lock().unwrap().push(payload.to_vec());
            Ok(())
        }
    }

    fn create_test_storage() -> (Arc<Storage>, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let storage = Arc::new(
            Storage::new(temp_file.path().to_str().unwrap()).expect("Failed to create storage"),
        );
        (storage, temp_file)
    }

    #[test]
    fn test_worker_pool_creation() {
        let (storage, _temp) = create_test_storage();
        let handler = Arc::new(SuccessHandler {
            call_count: Arc::new(AtomicUsize::new(0)),
        });

        let pool = WorkerPool::new(storage, handler, 4);
        assert_eq!(pool.num_workers, 4);
        assert_eq!(pool.poll_interval, Duration::from_secs(1));
    }

    #[test]
    fn test_worker_pool_with_custom_poll_interval() {
        let (storage, _temp) = create_test_storage();
        let handler = Arc::new(SuccessHandler {
            call_count: Arc::new(AtomicUsize::new(0)),
        });

        let pool =
            WorkerPool::new(storage, handler, 2).with_poll_interval(Duration::from_millis(100));

        assert_eq!(pool.poll_interval, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_single_worker_processes_job() {
        let (storage, _temp) = create_test_storage();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(SuccessHandler {
            call_count: Arc::clone(&call_count),
        });

        let job = Job::new(b"test job".to_vec(), Priority::Normal, 3);
        storage.insert(&job).unwrap();

        // Run worker in background with timeout
        let storage_clone = Arc::clone(&storage);
        let handler_clone = Arc::clone(&handler);
        let worker_task = tokio::spawn(async move {
            worker_loop(0, storage_clone, handler_clone, Duration::from_millis(10)).await;
        });

        // Wait for job to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify job was processed
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        let processed_job = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(processed_job.status, JobStatus::Completed);

        worker_task.abort();
    }

    #[tokio::test]
    async fn test_worker_handles_job_failure() {
        let (storage, _temp) = create_test_storage();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(FailHandler {
            call_count: Arc::clone(&call_count),
        });

        let job = Job::new(b"failing job".to_vec(), Priority::Normal, 2);
        storage.insert(&job).unwrap();

        let storage_clone = Arc::clone(&storage);
        let handler_clone = Arc::clone(&handler);
        let worker_task = tokio::spawn(async move {
            worker_loop(0, storage_clone, handler_clone, Duration::from_millis(10)).await;
        });

        // Wait for retries to complete (initial + 2s backoff + 4s backoff + processing)
        tokio::time::sleep(Duration::from_secs(7)).await;

        // Should be called 3 times: initial + 2 retries
        let calls = call_count.load(Ordering::SeqCst);
        assert!(calls >= 3, "Expected at least 3 calls, got {}", calls);

        let final_job = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(final_job.status, JobStatus::DeadLetter);
        assert!(final_job.error_message.is_some());

        worker_task.abort();
    }

    #[tokio::test]
    async fn test_worker_retry_logic() {
        let (storage, _temp) = create_test_storage();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(FailNTimesHandler {
            call_count: Arc::clone(&call_count),
            fail_times: 2, // Fail twice, then succeed
        });

        let job = Job::new(b"retry job".to_vec(), Priority::Normal, 5);
        storage.insert(&job).unwrap();

        let storage_clone = Arc::clone(&storage);
        let handler_clone = Arc::clone(&handler);
        let worker_task = tokio::spawn(async move {
            worker_loop(0, storage_clone, handler_clone, Duration::from_millis(10)).await;
        });

        // Wait for job to be processed through retries (initial + 2s backoff + 4s backoff)
        tokio::time::sleep(Duration::from_secs(7)).await;

        // Should be called 3 times: 2 failures + 1 success
        assert_eq!(call_count.load(Ordering::SeqCst), 3);

        let final_job = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(final_job.status, JobStatus::Completed);

        worker_task.abort();
    }

    #[tokio::test]
    async fn test_multiple_workers_no_duplicate_processing() {
        let (storage, _temp) = create_test_storage();
        let processed = Arc::new(StdMutex::new(Vec::new()));
        let handler = Arc::new(TrackingHandler {
            processed: Arc::clone(&processed),
        });

        // Insert 10 jobs
        for i in 0..10 {
            let job = Job::new(format!("job{}", i).into_bytes(), Priority::Normal, 3);
            storage.insert(&job).unwrap();
        }

        // Start 3 workers
        let mut worker_tasks = vec![];
        for worker_id in 0..3 {
            let storage_clone = Arc::clone(&storage);
            let handler_clone = Arc::clone(&handler);
            let task = tokio::spawn(async move {
                worker_loop(
                    worker_id,
                    storage_clone,
                    handler_clone,
                    Duration::from_millis(10),
                )
                .await;
            });
            worker_tasks.push(task);
        }

        // Wait for all jobs to be processed
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Verify exactly 10 jobs were processed (no duplicates)
        let processed_jobs = processed.lock().unwrap();
        assert_eq!(processed_jobs.len(), 10);

        // Verify all jobs are completed
        assert_eq!(storage.count_by_status(JobStatus::Completed).unwrap(), 10);
        assert_eq!(storage.count_by_status(JobStatus::Pending).unwrap(), 0);

        for task in worker_tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn test_worker_respects_priority() {
        let (storage, _temp) = create_test_storage();
        let processed = Arc::new(StdMutex::new(Vec::new()));
        let handler = Arc::new(TrackingHandler {
            processed: Arc::clone(&processed),
        });

        // Insert jobs with different priorities
        let low = Job::new(b"low".to_vec(), Priority::Low, 3);
        let high = Job::new(b"high".to_vec(), Priority::High, 3);
        let critical = Job::new(b"critical".to_vec(), Priority::Critical, 3);

        storage.insert(&low).unwrap();
        storage.insert(&high).unwrap();
        storage.insert(&critical).unwrap();

        let storage_clone = Arc::clone(&storage);
        let handler_clone = Arc::clone(&handler);
        let worker_task = tokio::spawn(async move {
            worker_loop(0, storage_clone, handler_clone, Duration::from_millis(10)).await;
        });

        // Wait for all jobs to be processed
        tokio::time::sleep(Duration::from_millis(200)).await;

        let processed_jobs = processed.lock().unwrap();
        assert_eq!(processed_jobs.len(), 3);

        // Verify processing order: critical, high, low
        assert_eq!(processed_jobs[0], b"critical");
        assert_eq!(processed_jobs[1], b"high");
        assert_eq!(processed_jobs[2], b"low");

        worker_task.abort();
    }

    #[tokio::test]
    async fn test_worker_exponential_backoff() {
        let (storage, _temp) = create_test_storage();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(FailHandler {
            call_count: Arc::clone(&call_count),
        });

        let job = Job::new(b"backoff test".to_vec(), Priority::Normal, 3);
        storage.insert(&job).unwrap();

        let start = std::time::Instant::now();

        let storage_clone = Arc::clone(&storage);
        let handler_clone = Arc::clone(&handler);
        let worker_task = tokio::spawn(async move {
            worker_loop(0, storage_clone, handler_clone, Duration::from_millis(10)).await;
        });

        // Wait for all retries (1st: immediate, 2nd: +2s, 3rd: +4s, 4th: +8s)
        tokio::time::sleep(Duration::from_secs(15)).await;

        let elapsed = start.elapsed();

        // Should have been called 4 times (initial + 3 retries)
        assert_eq!(call_count.load(Ordering::SeqCst), 4);

        // Total backoff time should be at least 2+4+8 = 14 seconds
        assert!(
            elapsed >= Duration::from_secs(14),
            "Expected at least 14s, got {:?}",
            elapsed
        );

        worker_task.abort();
    }

    #[tokio::test]
    async fn test_worker_pool_spawns_multiple_workers() {
        let (storage, _temp) = create_test_storage();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(SuccessHandler {
            call_count: Arc::clone(&call_count),
        });

        // Insert 5 jobs
        for i in 0..5 {
            let job = Job::new(format!("job{}", i).into_bytes(), Priority::Normal, 3);
            storage.insert(&job).unwrap();
        }

        let pool = WorkerPool::new(Arc::clone(&storage), handler, 3)
            .with_poll_interval(Duration::from_millis(10));

        // Run pool in background with timeout
        let pool_task = tokio::spawn(async move {
            let _ = timeout(Duration::from_millis(200), pool.run()).await;
        });

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(150)).await;

        // All jobs should be processed
        assert_eq!(storage.count_by_status(JobStatus::Completed).unwrap(), 5);

        pool_task.abort();
    }

    #[test]
    fn test_worker_pool_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WorkerPool>();
    }
}
