//! Parsing and validation logic for the Builder derive macro.
//!
//! This module handles parsing the input struct using syn and validating
//! that it's suitable for builder pattern generation.

use syn::{Data, DeriveInput, Fields};

/// Validates that the input is a struct with named fields.
///
/// Returns an error if the input is:
/// - An enum
/// - A union
/// - A tuple struct
/// - A unit struct
pub fn validate_struct(input: &DeriveInput) -> syn::Result<()> {
    match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(_) => Ok(()),
            Fields::Unnamed(_) => Err(syn::Error::new_spanned(
                input,
                "Builder can only be derived for structs with named fields, not tuple structs",
            )),
            Fields::Unit => Err(syn::Error::new_spanned(
                input,
                "Builder cannot be derived for unit structs",
            )),
        },
        Data::Enum(_) => Err(syn::Error::new_spanned(
            input,
            "Builder can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            input,
            "Builder can only be derived for structs, not unions",
        )),
    }
}

/// Extracts the named fields from a struct.
///
/// Assumes the input has already been validated with `validate_struct()`.
pub fn extract_fields(
    input: &DeriveInput,
) -> syn::Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>> {
    match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => Ok(&fields_named.named),
            _ => Err(syn::Error::new_spanned(input, "Expected named fields")),
        },
        _ => Err(syn::Error::new_spanned(input, "Expected a struct")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_validate_struct_accepts_named_fields() {
        let input: DeriveInput = syn::parse2(quote! {
            struct TestStruct {
                field1: String,
                field2: i32,
            }
        })
        .unwrap();

        assert!(validate_struct(&input).is_ok());
    }

    #[test]
    fn test_validate_struct_rejects_tuple_struct() {
        let input: DeriveInput = syn::parse2(quote! {
            struct TestStruct(String, i32);
        })
        .unwrap();

        assert!(validate_struct(&input).is_err());
    }

    #[test]
    fn test_validate_struct_rejects_unit_struct() {
        let input: DeriveInput = syn::parse2(quote! {
            struct TestStruct;
        })
        .unwrap();

        assert!(validate_struct(&input).is_err());
    }

    #[test]
    fn test_validate_struct_rejects_enum() {
        let input: DeriveInput = syn::parse2(quote! {
            enum TestEnum {
                Variant1,
                Variant2,
            }
        })
        .unwrap();

        assert!(validate_struct(&input).is_err());
    }
}
