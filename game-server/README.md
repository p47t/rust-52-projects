This is a revised version of [Game Server in 150 lines of Rust](https://medium.com/@buterajay/game-server-in-150-lines-of-rust-ce1782199907)

### What I Learned

- The design of futures-0.1 crate and how to represent asynchronous operations in `Future`s and `Stream`s.
- Using `Tokio` to execute futures
- Using nested block to prevent the awful renaming for cloned variables that need to be moved to closure called by another thread.
