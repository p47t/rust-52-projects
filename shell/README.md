A shell implementation in Rust based on [Shell From Scratch](https://www.destroyallsoftware.com/screencasts/catalog/shell-from-scratch) of Destroy All Software.

Things I learned:

- Use `rustyline` for REPL
- Use `nom` to implement simple PEG parser
- Use `Box<dyn Trait>` to build recursive data structure
- Use `std::process::Command` to create processes and setup pipe between them
- Use `Option::take` to take ownership of a optional field from a mutable struct.