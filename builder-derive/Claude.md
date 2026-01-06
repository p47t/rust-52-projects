# builder-derive - Detailed Documentation

## Overview

`builder-derive` is a procedural macro library that automatically generates the Builder pattern for Rust structs. This project demonstrates advanced Rust metaprogramming concepts including procedural macros, TokenStream manipulation, and the `syn` and `quote` crates.

## Table of Contents

- [Architecture](#architecture)
- [How It Works](#how-it-works)
- [Implementation Details](#implementation-details)
- [Generated Code](#generated-code)
- [Supported Field Types](#supported-field-types)
- [Error Handling](#error-handling)
- [Testing Strategy](#testing-strategy)
- [Rust Concepts Demonstrated](#rust-concepts-demonstrated)
- [Future Enhancements](#future-enhancements)
- [Learning Resources](#learning-resources)

## Architecture

The project is organized into several modules, each with a specific responsibility:

### Core Components

```
builder-derive/
├── src/
│   ├── lib.rs       - Proc-macro entry point and public API
│   ├── parse.rs     - Struct validation and field extraction
│   ├── field.rs     - Field analysis and type introspection
│   └── generate.rs  - Code generation using quote
```

#### lib.rs
- Defines the `#[proc_macro_derive(Builder)]` attribute
- Parses input TokenStream into `syn::DeriveInput`
- Coordinates parsing and code generation
- Handles errors and converts them to compiler errors

#### parse.rs
- Validates that the input is a struct with named fields
- Rejects enums, unions, tuple structs, and unit structs
- Extracts the field list for further processing
- Provides helpful error messages with proper spans

#### field.rs
- Analyzes individual fields to extract metadata
- Detects `Option<T>` types (for optional fields)
- Detects `Vec<T>` types (for default handling)
- Extracts inner types from generic wrappers
- Determines the appropriate types for builder fields and setter parameters

#### generate.rs
- Generates the builder struct definition
- Generates the `builder()` constructor method
- Generates setter methods for each field
- Generates the `build()` method with validation logic
- Uses the `quote!` macro for code generation

## How It Works

### Processing Pipeline

```
Input TokenStream (Rust code with #[derive(Builder)])
           ↓
   syn::parse_macro_input!
           ↓
      DeriveInput (AST)
           ↓
   validate_struct() - Ensure it's a valid struct
           ↓
   extract_fields() - Get field list
           ↓
   FieldInfo::from_field() - Analyze each field
           ↓
   generate_builder_struct()
   generate_builder_constructor()
   generate_setter_methods()
   generate_build_method()
           ↓
      quote! { ... } - Generate TokenStream
           ↓
  Output TokenStream (Generated Rust code)
```

### Field Analysis

For each field, the macro determines:

1. **Field name** - Used for setter method names and error messages
2. **Field type** - The original type as declared
3. **Is optional?** - Does the type match `Option<T>`?
4. **Inner type** - If `Option<T>`, extract `T`
5. **Is collection?** - Does the type match `Vec<T>`?

This analysis determines how the field is handled:

- **Required field** (`String`, `i32`, etc.) - Must be set before `build()`
- **Optional field** (`Option<T>`) - Can be omitted, defaults to `None`
- **Collection field** (`Vec<T>`) - Can be omitted, defaults to empty vector

### Type Detection

The macro uses pattern matching on `syn::Type` to detect special types:

```rust
fn extract_option_inner_type(ty: &Type) -> (bool, Option<Type>) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                // Extract T from Option<T>
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return (true, Some(inner_ty.clone()));
                    }
                }
            }
        }
    }
    (false, None)
}
```

This works by:
1. Matching on `Type::Path` (a type like `Option`, `String`, `Vec`)
2. Getting the last segment of the path (`Option` from `std::option::Option`)
3. Checking if the identifier is `"Option"`
4. Extracting the generic argument `T` from `Option<T>`

Similar logic applies to `Vec<T>` detection.

## Implementation Details

### Builder Struct Generation

For each field in the original struct, the builder has a corresponding field wrapped in `Option`:

```rust
// Original struct
struct User {
    username: String,      // Required
    age: Option<u32>,      // Optional
}

// Generated builder
struct UserBuilder {
    username: Option<String>,   // Tracks if set
    age: Option<Option<u32>>,   // Double Option!
}
```

Note: Optional fields become `Option<Option<T>>` in the builder. This allows the builder to distinguish between "not set" (`None`) and "set to None" (`Some(None)`).

### Setter Method Generation

Each setter method:
1. Takes `self` by value (enables method chaining)
2. Takes the unwrapped type as parameter (for `Option<T>` fields, takes `T`)
3. Sets the field to `Some(value)`
4. Returns `Self`

```rust
// For field: username: String
pub fn username(mut self, value: String) -> Self {
    self.username = Some(value);
    self
}

// For field: age: Option<u32>
pub fn age(mut self, value: u32) -> Self {  // Note: takes u32, not Option<u32>
    self.age = Some(Some(value));           // Double wrapping
    self
}
```

### Build Method Generation

The `build()` method:
1. Takes `self` by value (consumes the builder)
2. Returns `Result<OriginalStruct, String>`
3. Validates required fields
4. Unwraps optional fields
5. Provides defaults for collections

```rust
pub fn build(self) -> Result<User, String> {
    Ok(User {
        // Required field: error if None
        username: self.username
            .ok_or_else(|| "username is required".to_string())?,

        // Optional field: unwrap outer Option, keep inner Option
        age: self.age.unwrap_or(None),

        // Vec field: default to empty
        tags: self.tags.unwrap_or_default(),
    })
}
```

## Generated Code

### Complete Example

Input:

```rust
#[derive(Builder)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub timeout: Option<u64>,
    pub features: Vec<String>,
}
```

Generated output:

```rust
pub struct ConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    timeout: Option<Option<u64>>,
    features: Option<Vec<String>>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            host: None,
            port: None,
            timeout: None,
            features: None,
        }
    }
}

impl ConfigBuilder {
    pub fn host(mut self, value: String) -> Self {
        self.host = Some(value);
        self
    }

    pub fn port(mut self, value: u16) -> Self {
        self.port = Some(value);
        self
    }

    pub fn timeout(mut self, value: u64) -> Self {
        self.timeout = Some(Some(value));
        self
    }

    pub fn features(mut self, value: Vec<String>) -> Self {
        self.features = Some(value);
        self
    }

    pub fn build(self) -> Result<Config, String> {
        Ok(Config {
            host: self.host
                .ok_or_else(|| "host is required".to_string())?,
            port: self.port
                .ok_or_else(|| "port is required".to_string())?,
            timeout: self.timeout.unwrap_or(None),
            features: self.features.unwrap_or_default(),
        })
    }
}
```

## Supported Field Types

### Primitive Types
- `bool`, `char`
- `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
- `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
- `f32`, `f64`

### Standard Library Types
- `String`
- `Option<T>` (automatically detected as optional)
- `Vec<T>` (defaults to empty)
- Any other type that implements the necessary traits

### Custom Types
Any custom type can be used as long as it can be moved into the builder:

```rust
#[derive(Builder)]
struct Container {
    custom: MyCustomType,
}
```

### Nested Builders
You can use builder-derived types as fields:

```rust
#[derive(Builder)]
struct Address {
    street: String,
    city: String,
}

#[derive(Builder)]
struct Person {
    name: String,
    address: Address,  // Can be built separately
}
```

## Error Handling

### Compile-Time Errors

The macro validates input at compile time and produces helpful errors:

```rust
#[derive(Builder)]
enum NotSupported {  // ERROR: Builder can only be derived for structs, not enums
    Variant,
}

#[derive(Builder)]
struct NotSupported(String);  // ERROR: Builder can only be derived for structs with named fields
```

Errors include:
- Span information pointing to the problem
- Clear explanation of what went wrong
- Suggestions when applicable

### Runtime Errors

The `build()` method can fail if required fields are missing:

```rust
let result = User::builder()
    .username("alice".to_string())
    .build();  // Error: "email is required"

match result {
    Ok(user) => { /* use user */ },
    Err(e) => eprintln!("Build failed: {}", e),
}
```

Error messages include the field name for easy debugging.

## Testing Strategy

### Unit Tests

Located in the same files as the code they test:

```rust
// In field.rs
#[cfg(test)]
mod tests {
    #[test]
    fn test_detect_option_type() {
        // Test Option<T> detection
    }
}
```

Tests cover:
- Field type detection (`Option<T>`, `Vec<T>`)
- Struct validation logic
- Code generation building blocks

### Integration Tests

Located in `tests/integration_tests.rs`:

```rust
#[derive(Builder)]
struct TestStruct { /* ... */ }

#[test]
fn test_builder_all_fields_set() {
    let result = TestStruct::builder()
        .field1(value1)
        .field2(value2)
        .build();
    assert!(result.is_ok());
}
```

Tests cover:
- Successful builds
- Missing required fields
- Optional field handling
- Vec default handling
- Method chaining
- Complex scenarios

### Compile-Fail Tests

Located in `tests/compile_fail/`:

Uses the `trybuild` crate to verify that invalid usage produces compile errors:

```rust
// tests/compile_fail/enum_not_supported.rs
#[derive(Builder)]
enum NotSupported {  // Should not compile
    Variant,
}
```

Tests ensure:
- Enums are rejected
- Tuple structs are rejected
- Unit structs are rejected

### Examples

Located in `examples/`:

Real-world usage examples that also serve as documentation and manual tests:
- `basic_usage.rs`
- `optional_fields.rs`
- `complex_types.rs`
- `error_handling.rs`

Run with: `cargo run --example basic_usage`

## Rust Concepts Demonstrated

### Procedural Macros
- `#[proc_macro_derive]` attribute
- `proc-macro = true` in Cargo.toml
- TokenStream manipulation
- Compiler integration

### syn Crate
- Parsing Rust syntax into AST
- `DeriveInput`, `Field`, `Type` structures
- Pattern matching on AST nodes
- Error generation with spans

### quote Crate
- Code generation with `quote!` macro
- Interpolation with `#variable`
- Repetition with `#()*`
- Hygiene and scope handling

### Type System
- Generic type extraction
- Pattern matching on types
- Type path analysis
- Generic arguments

### Error Handling
- Compile-time vs runtime errors
- `syn::Result` and `syn::Error`
- Error message formatting
- Span preservation

### API Design
- Builder pattern
- Method chaining
- Consuming builders
- Result types

## Future Enhancements

### Custom Attributes
Allow field-level configuration:

```rust
#[derive(Builder)]
struct User {
    #[builder(default = "localhost")]
    host: String,

    #[builder(setter(into))]
    name: String,

    #[builder(skip)]
    internal: String,
}
```

### Validation
Add validation logic to setters or build:

```rust
#[derive(Builder)]
struct User {
    #[builder(validate = "validate_email")]
    email: String,
}
```

### Into Conversions
Accept anything that can convert into the target type:

```rust
// Instead of: .name("Alice".to_string())
// Allow:      .name("Alice")
```

### Custom Error Types
Replace `String` errors with structured error types:

```rust
pub enum BuildError {
    MissingField { field_name: &'static str },
    ValidationError { field_name: &'static str, message: String },
}
```

### Generic Support
Support structs with generic parameters:

```rust
#[derive(Builder)]
struct Container<T> {
    items: Vec<T>,
}
```

### Rename Builder
Allow customizing the builder name:

```rust
#[derive(Builder)]
#[builder(name = "CustomBuilder")]
struct MyStruct { /* ... */ }
```

## Learning Resources

### Official Documentation
- [The Rust Programming Language - Macros](https://doc.rust-lang.org/book/ch19-06-macros.html)
- [Procedural Macros Workshop](https://github.com/dtolnay/proc-macro-workshop)
- [syn Documentation](https://docs.rs/syn/)
- [quote Documentation](https://docs.rs/quote/)

### Related Projects
- [derive_builder](https://github.com/colin-kiegel/rust-derive-builder) - Full-featured builder macro
- [typed-builder](https://github.com/idanarye/rust-typed-builder) - Type-state builder macro
- [bon](https://github.com/elastio/bon) - Modern builder macro

### Articles and Tutorials
- [Procedural Macros in Rust](https://blog.logrocket.com/procedural-macros-in-rust/)
- [Writing a Procedural Macro](https://developerlife.com/2022/03/30/rust-proc-macro/)

## Performance Notes

### Compile Time
Procedural macros add to compile time because they:
- Parse and analyze code at compile time
- Generate additional code
- Run during macro expansion phase

For this simple builder macro, the impact is minimal.

### Runtime
Generated builders have zero runtime overhead:
- All validation happens when you call `build()`
- Method chaining is optimized by the compiler
- No vtables or dynamic dispatch
- Identical to hand-written builder code

## Comparison with Alternatives

### Hand-Written Builder
**Pros**: Full control, no macro magic
**Cons**: Lots of boilerplate, error-prone, tedious to maintain

### derive_builder Crate
**Pros**: More features, battle-tested, widely used
**Cons**: More complex, steeper learning curve

### builder-derive (This Project)
**Pros**: Simple, educational, easy to understand
**Cons**: Fewer features, not production-ready

## Contributing

This is an educational project. Potential improvements:
- Add support for custom attributes
- Implement generic type support
- Add validation capabilities
- Improve error messages
- Add more compile-fail tests
- Optimize generated code

## License

MIT OR Apache-2.0
