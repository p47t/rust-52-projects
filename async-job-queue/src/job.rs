use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Normal => write!(f, "normal"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    DeadLetter,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::DeadLetter => write!(f, "dead_letter"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub payload: Vec<u8>,
    pub priority: Priority,
    pub status: JobStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub error_message: Option<String>,
}

impl Job {
    pub fn new(payload: Vec<u8>, priority: Priority, max_retries: u32) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            payload,
            priority,
            status: JobStatus::Pending,
            retry_count: 0,
            max_retries,
            created_at: now,
            updated_at: now,
            error_message: None,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn mark_failed(&mut self, error: String) {
        self.status = if self.can_retry() {
            self.retry_count += 1;
            JobStatus::Pending
        } else {
            JobStatus::DeadLetter
        };
        self.error_message = Some(error);
        self.updated_at = Utc::now();
    }

    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.error_message = None;
        self.updated_at = Utc::now();
    }

    pub fn mark_running(&mut self) {
        self.status = JobStatus::Running;
        self.updated_at = Utc::now();
    }
}

pub trait JobHandler: Send + Sync {
    fn handle(&self, payload: &[u8]) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(Priority::Low.to_string(), "low");
        assert_eq!(Priority::Normal.to_string(), "normal");
        assert_eq!(Priority::High.to_string(), "high");
        assert_eq!(Priority::Critical.to_string(), "critical");
    }

    #[test]
    fn test_job_status_display() {
        assert_eq!(JobStatus::Pending.to_string(), "pending");
        assert_eq!(JobStatus::Running.to_string(), "running");
        assert_eq!(JobStatus::Completed.to_string(), "completed");
        assert_eq!(JobStatus::Failed.to_string(), "failed");
        assert_eq!(JobStatus::DeadLetter.to_string(), "dead_letter");
    }

    #[test]
    fn test_job_creation() {
        let payload = b"test payload".to_vec();
        let job = Job::new(payload.clone(), Priority::High, 3);

        assert_eq!(job.payload, payload);
        assert_eq!(job.priority, Priority::High);
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.retry_count, 0);
        assert_eq!(job.max_retries, 3);
        assert_eq!(job.error_message, None);
        assert!(job.created_at <= Utc::now());
        assert!(job.updated_at <= Utc::now());
    }

    #[test]
    fn test_can_retry_with_retries_remaining() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.retry_count = 2;

        assert!(job.can_retry());
    }

    #[test]
    fn test_can_retry_at_max_retries() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.retry_count = 3;

        assert!(!job.can_retry());
    }

    #[test]
    fn test_can_retry_over_max_retries() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.retry_count = 5;

        assert!(!job.can_retry());
    }

    #[test]
    fn test_mark_failed_with_retries_remaining() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        let initial_retry_count = job.retry_count;
        let initial_updated_at = job.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        job.mark_failed("Test error".to_string());

        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.retry_count, initial_retry_count + 1);
        assert_eq!(job.error_message, Some("Test error".to_string()));
        assert!(job.updated_at > initial_updated_at);
    }

    #[test]
    fn test_mark_failed_at_max_retries() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.retry_count = 3;

        job.mark_failed("Final error".to_string());

        assert_eq!(job.status, JobStatus::DeadLetter);
        assert_eq!(job.retry_count, 3); // Should not increment
        assert_eq!(job.error_message, Some("Final error".to_string()));
    }

    #[test]
    fn test_mark_failed_increments_retry_count() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 5);

        job.mark_failed("Error 1".to_string());
        assert_eq!(job.retry_count, 1);
        assert_eq!(job.status, JobStatus::Pending);

        job.mark_failed("Error 2".to_string());
        assert_eq!(job.retry_count, 2);
        assert_eq!(job.status, JobStatus::Pending);

        job.mark_failed("Error 3".to_string());
        assert_eq!(job.retry_count, 3);
        assert_eq!(job.status, JobStatus::Pending);
    }

    #[test]
    fn test_mark_completed() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.error_message = Some("Previous error".to_string());
        let initial_updated_at = job.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        job.mark_completed();

        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.error_message, None);
        assert!(job.updated_at > initial_updated_at);
    }

    #[test]
    fn test_mark_running() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        let initial_updated_at = job.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        job.mark_running();

        assert_eq!(job.status, JobStatus::Running);
        assert!(job.updated_at > initial_updated_at);
    }

    #[test]
    fn test_job_state_transitions() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 2);

        // Pending -> Running
        assert_eq!(job.status, JobStatus::Pending);
        job.mark_running();
        assert_eq!(job.status, JobStatus::Running);

        // Running -> Failed (with retry) -> Pending
        job.mark_failed("First failure".to_string());
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.retry_count, 1);

        // Pending -> Running
        job.mark_running();
        assert_eq!(job.status, JobStatus::Running);

        // Running -> Failed (with retry) -> Pending
        job.mark_failed("Second failure".to_string());
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.retry_count, 2);

        // Pending -> Running
        job.mark_running();
        assert_eq!(job.status, JobStatus::Running);

        // Running -> Failed (max retries) -> DeadLetter
        job.mark_failed("Final failure".to_string());
        assert_eq!(job.status, JobStatus::DeadLetter);
        assert_eq!(job.retry_count, 2);
    }

    #[test]
    fn test_job_successful_completion() {
        let mut job = Job::new(b"test".to_vec(), Priority::High, 3);

        // Pending -> Running
        job.mark_running();
        assert_eq!(job.status, JobStatus::Running);

        // Running -> Completed
        job.mark_completed();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.error_message, None);
    }

    #[test]
    fn test_job_with_zero_max_retries() {
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 0);

        assert!(!job.can_retry());

        job.mark_failed("Error".to_string());
        assert_eq!(job.status, JobStatus::DeadLetter);
        assert_eq!(job.retry_count, 0);
    }

    #[test]
    fn test_job_serialization() {
        let job = Job::new(b"test payload".to_vec(), Priority::Critical, 5);

        // Test that Job can be serialized and deserialized
        let serialized = bincode::serialize(&job).expect("Failed to serialize");
        let deserialized: Job = bincode::deserialize(&serialized).expect("Failed to deserialize");

        assert_eq!(job.id, deserialized.id);
        assert_eq!(job.payload, deserialized.payload);
        assert_eq!(job.priority, deserialized.priority);
        assert_eq!(job.status, deserialized.status);
        assert_eq!(job.retry_count, deserialized.retry_count);
        assert_eq!(job.max_retries, deserialized.max_retries);
    }

    // Test that JobHandler trait is object-safe and works with Arc
    struct TestHandler;

    impl JobHandler for TestHandler {
        fn handle(&self, payload: &[u8]) -> Result<(), String> {
            if payload.is_empty() {
                Err("Empty payload".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_job_handler_trait() {
        let handler = TestHandler;

        assert!(handler.handle(b"test").is_ok());
        assert!(handler.handle(b"").is_err());
    }

    #[test]
    fn test_job_handler_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestHandler>();
    }

    #[test]
    fn test_job_handler_with_arc() {
        use std::sync::Arc;

        let handler: Arc<dyn JobHandler> = Arc::new(TestHandler);

        assert!(handler.handle(b"valid").is_ok());
        assert!(handler.handle(b"").is_err());
    }
}
