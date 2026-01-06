//! # builder-derive
//!
//! A procedural macro for automatically generating the Builder pattern.
//!
//! ## Example
//!
//! ```rust
//! use builder_derive::Builder;
//!
//! #[derive(Builder)]
//! pub struct User {
//!     pub username: String,
//!     pub email: String,
//!     pub age: Option<u32>,
//! }
//!
//! let user = User::builder()
//!     .username("alice".to_string())
//!     .email("alice@example.com".to_string())
//!     .age(30)
//!     .build()
//!     .unwrap();
//! ```

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod field;
mod generate;
mod parse;

/// Derives a Builder pattern for the annotated struct.
///
/// This macro generates:
/// - A builder struct named `{StructName}Builder`
/// - A `builder()` constructor method on the original struct
/// - Setter methods for each field that enable method chaining
/// - A `build()` method that validates required fields and constructs the original struct
///
/// ## Field Handling
///
/// - **Required fields**: Non-`Option<T>` fields must be set before calling `build()`
/// - **Optional fields**: `Option<T>` fields can be omitted (default to `None`)
/// - **Collections**: `Vec<T>` fields default to empty vectors if not set
///
/// ## Example
///
/// ```rust
/// use builder_derive::Builder;
///
/// #[derive(Builder)]
/// pub struct Config {
///     pub host: String,
///     pub port: u16,
///     pub timeout: Option<u64>,
///     pub features: Vec<String>,
/// }
///
/// let config = Config::builder()
///     .host("localhost".to_string())
///     .port(8080)
///     .build()
///     .expect("Failed to build config");
/// ```
#[proc_macro_derive(Builder)]
pub fn derive_builder(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate::impl_builder(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
