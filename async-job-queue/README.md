# async-job-queue

A distributed asynchronous job queue system with persistent storage, priority scheduling, and automatic retry logic.

## Quick Start

```bash
# Build the project
cargo build --release

# Terminal 1 - Start the worker
cargo run --bin worker

# Terminal 2 - Submit some jobs
cargo run --bin producer -- submit --payload "Hello, queue!"
cargo run --bin producer -- submit --payload "High priority task" --priority high
cargo run --bin producer -- stats
```

See [Claude.md](Claude.md) for detailed documentation.
