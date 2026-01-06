//! Code generation logic for the Builder pattern.
//!
//! This module uses the `quote` crate to generate the builder struct,
//! setter methods, and build method.

use crate::field::FieldInfo;
use crate::parse::{extract_fields, validate_struct};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

/// Generates the complete builder implementation for a struct.
pub fn impl_builder(input: &DeriveInput) -> syn::Result<TokenStream> {
    // Validate that input is a struct with named fields
    validate_struct(input)?;

    // Extract field information
    let fields = extract_fields(input)?;
    let field_infos: Result<Vec<_>, _> = fields.iter().map(FieldInfo::from_field).collect();
    let field_infos = field_infos?;

    // Get struct name and visibility
    let struct_name = &input.ident;
    let builder_name = quote::format_ident!("{}Builder", struct_name);
    let vis = &input.vis;

    // Generate builder struct
    let builder_struct = generate_builder_struct(&builder_name, &field_infos, vis);

    // Generate builder() constructor method
    let builder_constructor =
        generate_builder_constructor(struct_name, &builder_name, &field_infos, vis);

    // Generate setter methods
    let setter_methods = generate_setter_methods(&field_infos);

    // Generate build() method
    let build_method = generate_build_method(struct_name, &field_infos);

    // Combine everything
    Ok(quote! {
        #builder_struct

        #builder_constructor

        impl #builder_name {
            #setter_methods
            #build_method
        }
    })
}

/// Generates the builder struct definition.
fn generate_builder_struct(
    builder_name: &syn::Ident,
    field_infos: &[FieldInfo],
    vis: &syn::Visibility,
) -> TokenStream {
    let builder_fields = field_infos.iter().map(|field| {
        let name = &field.name;
        let builder_ty = field.builder_field_type();
        quote! { #name: #builder_ty }
    });

    quote! {
        #vis struct #builder_name {
            #(#builder_fields,)*
        }
    }
}

/// Generates the builder() constructor method on the original struct.
fn generate_builder_constructor(
    struct_name: &syn::Ident,
    builder_name: &syn::Ident,
    field_infos: &[FieldInfo],
    vis: &syn::Visibility,
) -> TokenStream {
    let field_initializers = field_infos.iter().map(|field| {
        let name = &field.name;
        quote! { #name: ::std::option::Option::None }
    });

    quote! {
        impl #struct_name {
            #vis fn builder() -> #builder_name {
                #builder_name {
                    #(#field_initializers,)*
                }
            }
        }
    }
}

/// Generates setter methods for each field.
fn generate_setter_methods(field_infos: &[FieldInfo]) -> TokenStream {
    let setters = field_infos.iter().map(|field| {
        let name = &field.name;
        let param_ty = field.setter_param_type();

        quote! {
            pub fn #name(mut self, value: #param_ty) -> Self {
                self.#name = ::std::option::Option::Some(value);
                self
            }
        }
    });

    quote! {
        #(#setters)*
    }
}

/// Generates the build() method that constructs the original struct.
fn generate_build_method(struct_name: &syn::Ident, field_infos: &[FieldInfo]) -> TokenStream {
    let field_assignments = field_infos.iter().map(|field| {
        let name = &field.name;
        let field_name_str = name.to_string();

        if field.is_optional {
            // Optional fields: pass through as-is (already Option<T>)
            quote! {
                #name: self.#name
            }
        } else if field.is_vec {
            // Vec fields: default to empty vector if not set
            quote! {
                #name: self.#name.unwrap_or_default()
            }
        } else {
            // Required fields: return error if not set
            quote! {
                #name: self.#name.ok_or_else(|| format!("{} is required", #field_name_str))?
            }
        }
    });

    quote! {
        pub fn build(self) -> ::std::result::Result<#struct_name, ::std::string::String> {
            ::std::result::Result::Ok(#struct_name {
                #(#field_assignments,)*
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_impl_builder_generates_code() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct TestStruct {
                pub field1: String,
                pub field2: i32,
            }
        })
        .unwrap();

        let result = impl_builder(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_impl_builder_with_optional_fields() {
        let input: DeriveInput = syn::parse2(quote! {
            pub struct TestStruct {
                pub required: String,
                pub optional: Option<i32>,
            }
        })
        .unwrap();

        let result = impl_builder(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_impl_builder_rejects_enum() {
        let input: DeriveInput = syn::parse2(quote! {
            pub enum TestEnum {
                Variant1,
                Variant2,
            }
        })
        .unwrap();

        let result = impl_builder(&input);
        assert!(result.is_err());
    }
}
