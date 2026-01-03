# async-job-queue

A distributed asynchronous job queue system built with Rust and Tokio.

## Overview

This project implements a persistent job queue with the following features:

- **Asynchronous processing** using Tokio runtime
- **SQLite persistence** - jobs survive process restarts
- **Priority-based scheduling** - critical, high, normal, low
- **Automatic retry logic** with exponential backoff
- **Dead letter queue** for permanently failed jobs
- **Worker pool** with configurable concurrency
- **CLI tools** for job submission and monitoring

## Architecture

### Core Components

- **Job** (`src/job.rs`) - Job data structure with priority, status, and retry metadata
- **Storage** (`src/storage.rs`) - SQLite-backed persistent storage with thread-safe operations
- **WorkerPool** (`src/worker.rs`) - Manages multiple concurrent workers processing jobs
- **JobHandler** trait - Defines how jobs are processed (implement custom handlers)

### Binaries

- **producer** (`src/bin/producer.rs`) - CLI to submit jobs and query status
- **worker** (`src/bin/worker.rs`) - Worker daemon that processes jobs

## Building

```bash
cd async-job-queue
cargo build --release
```

## Usage

### Start the Worker

```bash
# Start with 4 workers (default)
cargo run --bin worker

# Start with 8 workers
cargo run --bin worker -- --workers 8

# Use a custom database
cargo run --bin worker -- --database /path/to/jobs.db
```

### Submit Jobs

```bash
# Submit a normal priority job
cargo run --bin producer -- submit --payload "Process this data"

# Submit a high priority job with custom retries
cargo run --bin producer -- submit \
  --payload "Important task" \
  --priority high \
  --max-retries 5

# Submit a job that will fail (for testing)
cargo run --bin producer -- submit --payload "fail this job"
```

### Monitor Jobs

```bash
# View overall statistics
cargo run --bin producer -- stats

# Check specific job status
cargo run --bin producer -- status --job-id <uuid>
```

## Job Priorities

Jobs are processed in priority order:

1. **Critical** - Highest priority, processed first
2. **High** - Important jobs
3. **Normal** - Default priority
4. **Low** - Background tasks

Within the same priority, jobs are processed FIFO (first in, first out).

## Retry Logic

- Failed jobs are automatically retried up to `max_retries`
- Each retry has exponential backoff: 2^retry_count seconds (capped at 32s)
- Jobs exceeding max retries move to the dead letter queue
- Dead letter jobs require manual intervention

## Job States

- **Pending** - Waiting to be processed
- **Running** - Currently being processed by a worker
- **Completed** - Successfully finished
- **Failed** - Failed but will retry
- **DeadLetter** - Failed permanently, no more retries

## Customizing Job Handlers

The default worker uses `EchoHandler` which prints payloads. To implement custom logic:

```rust
use async_job_queue::JobHandler;

struct MyHandler;

impl JobHandler for MyHandler {
    fn handle(&self, payload: &[u8]) -> Result<(), String> {
        // Your custom processing logic
        let data = String::from_utf8_lossy(payload);
        
        // Do work...
        
        Ok(())
    }
}

// In worker main:
let handler = Arc::new(MyHandler);
let pool = WorkerPool::new(storage, handler, num_workers);
```

## Database Schema

The system uses SQLite with a single `jobs` table:

```sql
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    payload BLOB NOT NULL,
    priority INTEGER NOT NULL,
    status INTEGER NOT NULL,
    retry_count INTEGER NOT NULL,
    max_retries INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    error_message TEXT
)
```

Indexed on `(status, priority DESC, created_at ASC)` for efficient job retrieval.

## Concepts Demonstrated

### Rust Concepts

- **Async/await** with Tokio runtime
- **Arc and Mutex** for thread-safe shared state
- **Trait objects** for polymorphic job handlers
- **Error handling** with thiserror and Result types
- **Serialization** with serde and bincode
- **CLI parsing** with clap

### System Design

- **Producer-consumer pattern** with persistent queue
- **Worker pool** for parallel processing
- **Retry strategies** with exponential backoff
- **Priority scheduling** for job ordering
- **Graceful degradation** to dead letter queue

## Future Enhancements

Potential improvements to explore:

- [ ] Network protocol for remote job submission (Redis-compatible?)
- [ ] Job scheduling - delay jobs until a specific time
- [ ] Job dependencies - wait for other jobs to complete
- [ ] Metrics and monitoring - Prometheus endpoints
- [ ] Job timeouts - cancel long-running jobs
- [ ] Multiple queues - separate queues for different job types
- [ ] Distributed workers - multiple machines processing the same queue

## Learning Resources

- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Async Book](https://rust-lang.github.io/async-book/)
- [SQLite Rust](https://docs.rs/rusqlite/)
- [Job Queue Patterns](https://en.wikipedia.org/wiki/Job_queue)

## License

MIT
