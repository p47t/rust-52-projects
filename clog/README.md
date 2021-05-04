A utility to output colored log.

## Converting Option<T> to Result<T, CustomError> with a trait

Generally you can extract a named match by:

```rust
cap.name("time0").unwrap().as_str()
```

or 

```rust
cap.name("time0").ok_or(ParseError::MissingField).as_str()
```

I want to simplify it to:

```rust
cap.field("time0")?
```
