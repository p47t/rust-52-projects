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
