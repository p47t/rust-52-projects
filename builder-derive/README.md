# builder-derive

A procedural macro for automatically generating the Builder pattern in Rust.

## Overview

`builder-derive` provides a `#[derive(Builder)]` macro that automatically generates builder pattern code for your structs. This eliminates boilerplate and provides a clean, fluent API for constructing complex objects.

## Features

- Automatic builder struct generation
- Method chaining for ergonomic API
- Smart handling of `Option<T>` fields (automatically optional)
- Smart handling of `Vec<T>` fields (default to empty)
- Compile-time validation of struct compatibility
- Runtime validation of required fields
- Clear error messages

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
builder-derive = { path = "../builder-derive" }
```

Then use the `Builder` derive macro:

```rust
use builder_derive::Builder;

#[derive(Builder, Debug)]
struct User {
    username: String,
    email: String,
    age: Option<u32>,
}

fn main() {
    let user = User::builder()
        .username("alice".to_string())
        .email("alice@example.com".to_string())
        .age(30)
        .build()
        .expect("Failed to build user");

    println!("{:?}", user);
}
```

## How It Works

For a struct like this:

```rust
#[derive(Builder)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub timeout: Option<u64>,
    pub features: Vec<String>,
}
```

The macro generates:

1. A builder struct (`ConfigBuilder`) with all fields as `Option<T>`
2. A `builder()` constructor method
3. Setter methods for each field that enable method chaining
4. A `build()` method that validates and constructs the original struct

Usage:

```rust
let config = Config::builder()
    .host("localhost".to_string())
    .port(8080)
    .build()
    .expect("Missing required fields");
```

## Field Types

### Required Fields

Non-`Option` fields are required and must be set before calling `build()`:

```rust
#[derive(Builder)]
struct User {
    username: String,  // Required
    email: String,     // Required
}

// This will fail at runtime:
User::builder().username("alice".to_string()).build(); // Error: "email is required"
```

### Optional Fields

`Option<T>` fields are automatically optional:

```rust
#[derive(Builder)]
struct Profile {
    username: String,
    bio: Option<String>,  // Optional
}

// This works fine:
Profile::builder().username("alice".to_string()).build(); // bio will be None
```

### Collection Fields

`Vec<T>` fields default to empty vectors if not set:

```rust
#[derive(Builder)]
struct Config {
    host: String,
    features: Vec<String>,  // Defaults to empty vec
}

// This works fine:
Config::builder().host("localhost".to_string()).build(); // features will be []
```

## Error Handling

The `build()` method returns `Result<T, String>`:

```rust
match User::builder().username("alice".to_string()).build() {
    Ok(user) => println!("Created: {:?}", user),
    Err(e) => println!("Error: {}", e),  // "email is required"
}
```

## Examples

See the `examples/` directory for complete examples:

- `basic_usage.rs` - Simple struct demonstration
- `optional_fields.rs` - Working with `Option<T>` fields
- `complex_types.rs` - `Vec<T>` and other complex types
- `error_handling.rs` - Error handling patterns

Run an example:

```bash
cargo run --example basic_usage
```

## Limitations

Currently, the Builder macro:

- Only works with structs that have named fields
- Does not support tuple structs or unit structs
- Does not support enums or unions
- Does not support custom attributes for field configuration
- Does not support generics (planned for future versions)

## For More Details

See [Claude.md](Claude.md) for detailed documentation including:
- Architecture and implementation details
- How procedural macros work
- Code generation examples
- Future enhancements

## License

MIT OR Apache-2.0
