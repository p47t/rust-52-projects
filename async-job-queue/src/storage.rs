use crate::job::{Job, JobStatus, Priority};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Job not found: {0}")]
    NotFound(Uuid),
    #[error("Mutex lock failed: internal state may be corrupted")]
    MutexPoisoned,
}

pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    pub fn new(db_path: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                payload BLOB NOT NULL,
                priority INTEGER NOT NULL,
                status INTEGER NOT NULL,
                retry_count INTEGER NOT NULL,
                max_retries INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                error_message TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_status_priority
             ON jobs(status, priority DESC, created_at ASC)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert(&self, job: &Job) -> Result<(), StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::MutexPoisoned)?;
        conn.execute(
            "INSERT INTO jobs (id, payload, priority, status, retry_count, max_retries,
                               created_at, updated_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                job.id.to_string(),
                job.payload,
                job.priority as i32,
                job.status as i32,
                job.retry_count,
                job.max_retries,
                job.created_at.to_rfc3339(),
                job.updated_at.to_rfc3339(),
                job.error_message,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, job: &Job) -> Result<(), StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::MutexPoisoned)?;
        let rows_affected = conn.execute(
            "UPDATE jobs SET payload = ?2, priority = ?3, status = ?4, retry_count = ?5,
                            max_retries = ?6, updated_at = ?7, error_message = ?8
             WHERE id = ?1",
            params![
                job.id.to_string(),
                job.payload,
                job.priority as i32,
                job.status as i32,
                job.retry_count,
                job.max_retries,
                job.updated_at.to_rfc3339(),
                job.error_message,
            ],
        )?;

        if rows_affected == 0 {
            return Err(StorageError::NotFound(job.id));
        }

        Ok(())
    }

    pub fn get_next_pending(&self) -> Result<Option<Job>, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::MutexPoisoned)?;

        // Atomically claim the job by updating it to Running status
        // SQLite's RETURNING clause allows us to get the updated row
        let mut stmt = conn.prepare(
            "UPDATE jobs
             SET status = ?1, updated_at = ?2
             WHERE id = (
                 SELECT id FROM jobs
                 WHERE status = ?3
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 1
             )
             RETURNING id, payload, priority, status, retry_count, max_retries,
                       created_at, updated_at, error_message",
        )?;

        let now = Utc::now().to_rfc3339();
        let job = stmt
            .query_row(
                params![JobStatus::Running as i32, now, JobStatus::Pending as i32],
                |row| Ok(self.row_to_job(row)?),
            )
            .optional()?;

        Ok(job)
    }

    pub fn get_by_id(&self, id: Uuid) -> Result<Option<Job>, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::MutexPoisoned)?;
        let mut stmt = conn.prepare(
            "SELECT id, payload, priority, status, retry_count, max_retries,
                    created_at, updated_at, error_message
             FROM jobs WHERE id = ?1",
        )?;

        let job = stmt
            .query_row(params![id.to_string()], |row| Ok(self.row_to_job(row)?))
            .optional()?;

        Ok(job)
    }

    pub fn count_by_status(&self, status: JobStatus) -> Result<usize, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::MutexPoisoned)?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE status = ?1",
            params![status as i32],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn row_to_job(&self, row: &rusqlite::Row) -> SqlResult<Job> {
        let id_str: String = row.get(0)?;
        let priority_val: i32 = row.get(2)?;
        let status_val: i32 = row.get(3)?;
        let created_str: String = row.get(6)?;
        let updated_str: String = row.get(7)?;

        Ok(Job {
            id: Uuid::parse_str(&id_str).unwrap(),
            payload: row.get(1)?,
            priority: match priority_val {
                0 => Priority::Low,
                1 => Priority::Normal,
                2 => Priority::High,
                3 => Priority::Critical,
                _ => Priority::Normal,
            },
            status: match status_val {
                0 => JobStatus::Pending,
                1 => JobStatus::Running,
                2 => JobStatus::Completed,
                3 => JobStatus::Failed,
                4 => JobStatus::DeadLetter,
                _ => JobStatus::Pending,
            },
            retry_count: row.get(4)?,
            max_retries: row.get(5)?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .unwrap()
                .with_timezone(&Utc),
            error_message: row.get(8)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_storage() -> (Storage, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let storage =
            Storage::new(temp_file.path().to_str().unwrap()).expect("Failed to create storage");
        (storage, temp_file)
    }

    #[test]
    fn test_storage_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let result = Storage::new(temp_file.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_job() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test payload".to_vec(), Priority::Normal, 3);

        let result = storage.insert(&job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_insert_and_retrieve_job() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test payload".to_vec(), Priority::High, 5);

        storage.insert(&job).unwrap();

        let retrieved = storage.get_by_id(job.id).unwrap();
        assert!(retrieved.is_some());

        let retrieved_job = retrieved.unwrap();
        assert_eq!(retrieved_job.id, job.id);
        assert_eq!(retrieved_job.payload, job.payload);
        assert_eq!(retrieved_job.priority, job.priority);
        assert_eq!(retrieved_job.status, job.status);
        assert_eq!(retrieved_job.retry_count, job.retry_count);
        assert_eq!(retrieved_job.max_retries, job.max_retries);
    }

    #[test]
    fn test_get_nonexistent_job() {
        let (storage, _temp) = create_test_storage();
        let random_id = Uuid::new_v4();

        let result = storage.get_by_id(random_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_job() {
        let (storage, _temp) = create_test_storage();
        let mut job = Job::new(b"original".to_vec(), Priority::Low, 2);

        storage.insert(&job).unwrap();

        // Modify the job
        job.payload = b"updated".to_vec();
        job.priority = Priority::Critical;
        job.status = JobStatus::Running;

        storage.update(&job).unwrap();

        let retrieved = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(retrieved.payload, b"updated");
        assert_eq!(retrieved.priority, Priority::Critical);
        assert_eq!(retrieved.status, JobStatus::Running);
    }

    #[test]
    fn test_update_nonexistent_job() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test".to_vec(), Priority::Normal, 3);

        let result = storage.update(&job);
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_count_by_status() {
        let (storage, _temp) = create_test_storage();

        // Initially should be zero
        assert_eq!(storage.count_by_status(JobStatus::Pending).unwrap(), 0);

        // Insert some jobs with different statuses
        let job1 = Job::new(b"job1".to_vec(), Priority::Normal, 3);
        let mut job2 = Job::new(b"job2".to_vec(), Priority::Normal, 3);
        job2.status = JobStatus::Running;
        let mut job3 = Job::new(b"job3".to_vec(), Priority::Normal, 3);
        job3.status = JobStatus::Pending;

        storage.insert(&job1).unwrap();
        storage.insert(&job2).unwrap();
        storage.insert(&job3).unwrap();

        assert_eq!(storage.count_by_status(JobStatus::Pending).unwrap(), 2);
        assert_eq!(storage.count_by_status(JobStatus::Running).unwrap(), 1);
        assert_eq!(storage.count_by_status(JobStatus::Completed).unwrap(), 0);
    }

    #[test]
    fn test_get_next_pending_returns_none_when_empty() {
        let (storage, _temp) = create_test_storage();

        let result = storage.get_next_pending().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_next_pending_returns_pending_job() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test".to_vec(), Priority::Normal, 3);

        storage.insert(&job).unwrap();

        let result = storage.get_next_pending().unwrap();
        assert!(result.is_some());

        let fetched = result.unwrap();
        assert_eq!(fetched.id, job.id);
        assert_eq!(fetched.status, JobStatus::Running); // Should be marked as Running
    }

    #[test]
    fn test_get_next_pending_respects_priority() {
        let (storage, _temp) = create_test_storage();

        let low = Job::new(b"low".to_vec(), Priority::Low, 3);
        let high = Job::new(b"high".to_vec(), Priority::High, 3);
        let normal = Job::new(b"normal".to_vec(), Priority::Normal, 3);

        // Insert in random order
        storage.insert(&normal).unwrap();
        storage.insert(&low).unwrap();
        storage.insert(&high).unwrap();

        // Should get high priority first
        let result = storage.get_next_pending().unwrap().unwrap();
        assert_eq!(result.payload, b"high");
        assert_eq!(result.status, JobStatus::Running);
    }

    #[test]
    fn test_get_next_pending_fifo_for_same_priority() {
        let (storage, _temp) = create_test_storage();

        let job1 = Job::new(b"first".to_vec(), Priority::Normal, 3);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let job2 = Job::new(b"second".to_vec(), Priority::Normal, 3);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let job3 = Job::new(b"third".to_vec(), Priority::Normal, 3);

        storage.insert(&job1).unwrap();
        storage.insert(&job2).unwrap();
        storage.insert(&job3).unwrap();

        // Should get jobs in FIFO order
        let first = storage.get_next_pending().unwrap().unwrap();
        assert_eq!(first.payload, b"first");

        let second = storage.get_next_pending().unwrap().unwrap();
        assert_eq!(second.payload, b"second");

        let third = storage.get_next_pending().unwrap().unwrap();
        assert_eq!(third.payload, b"third");
    }

    #[test]
    fn test_get_next_pending_atomic_claim() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test".to_vec(), Priority::Normal, 3);

        storage.insert(&job).unwrap();

        // First call should return the job and mark it as Running
        let first = storage.get_next_pending().unwrap();
        assert!(first.is_some());
        assert_eq!(first.unwrap().status, JobStatus::Running);

        // Second call should return None (no more pending jobs)
        let second = storage.get_next_pending().unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn test_get_next_pending_skips_non_pending() {
        let (storage, _temp) = create_test_storage();

        let mut running = Job::new(b"running".to_vec(), Priority::High, 3);
        running.status = JobStatus::Running;

        let mut completed = Job::new(b"completed".to_vec(), Priority::High, 3);
        completed.status = JobStatus::Completed;

        let pending = Job::new(b"pending".to_vec(), Priority::Low, 3);

        storage.insert(&running).unwrap();
        storage.insert(&completed).unwrap();
        storage.insert(&pending).unwrap();

        // Should get the pending job despite lower priority
        let result = storage.get_next_pending().unwrap().unwrap();
        assert_eq!(result.payload, b"pending");
    }

    #[test]
    fn test_insert_multiple_jobs() {
        let (storage, _temp) = create_test_storage();

        for i in 0..10 {
            let job = Job::new(format!("job{}", i).into_bytes(), Priority::Normal, 3);
            storage.insert(&job).unwrap();
        }

        assert_eq!(storage.count_by_status(JobStatus::Pending).unwrap(), 10);
    }

    #[test]
    fn test_job_with_error_message() {
        let (storage, _temp) = create_test_storage();
        let mut job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        job.error_message = Some("Test error message".to_string());

        storage.insert(&job).unwrap();

        let retrieved = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(
            retrieved.error_message,
            Some("Test error message".to_string())
        );
    }

    #[test]
    fn test_storage_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Storage>();
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let (storage, _temp) = create_test_storage();
        let storage = Arc::new(storage);

        let mut handles = vec![];

        // Spawn multiple threads inserting jobs
        for i in 0..5 {
            let storage_clone = Arc::clone(&storage);
            let handle = thread::spawn(move || {
                let job = Job::new(format!("job{}", i).into_bytes(), Priority::Normal, 3);
                storage_clone.insert(&job).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(storage.count_by_status(JobStatus::Pending).unwrap(), 5);
    }

    #[test]
    fn test_priority_values_match_enum() {
        let (storage, _temp) = create_test_storage();

        let jobs = vec![
            Job::new(b"low".to_vec(), Priority::Low, 3),
            Job::new(b"normal".to_vec(), Priority::Normal, 3),
            Job::new(b"high".to_vec(), Priority::High, 3),
            Job::new(b"critical".to_vec(), Priority::Critical, 3),
        ];

        for job in &jobs {
            storage.insert(job).unwrap();
        }

        // Verify all priorities are stored and retrieved correctly
        for job in &jobs {
            let retrieved = storage.get_by_id(job.id).unwrap().unwrap();
            assert_eq!(retrieved.priority, job.priority);
        }
    }

    #[test]
    fn test_all_job_statuses() {
        let (storage, _temp) = create_test_storage();

        let statuses = vec![
            JobStatus::Pending,
            JobStatus::Running,
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::DeadLetter,
        ];

        for (i, status) in statuses.iter().enumerate() {
            let mut job = Job::new(format!("job{}", i).into_bytes(), Priority::Normal, 3);
            job.status = *status;
            storage.insert(&job).unwrap();

            let retrieved = storage.get_by_id(job.id).unwrap().unwrap();
            assert_eq!(retrieved.status, *status);
        }
    }

    #[test]
    fn test_update_preserves_created_at() {
        let (storage, _temp) = create_test_storage();
        let job = Job::new(b"test".to_vec(), Priority::Normal, 3);
        let original_created_at = job.created_at;

        storage.insert(&job).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut updated_job = storage.get_by_id(job.id).unwrap().unwrap();
        updated_job.payload = b"updated".to_vec();
        storage.update(&updated_job).unwrap();

        let final_job = storage.get_by_id(job.id).unwrap().unwrap();
        assert_eq!(final_job.created_at, original_created_at);
        assert_eq!(final_job.payload, b"updated");
    }
}
