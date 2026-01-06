//! Field analysis and type introspection.
//!
//! This module provides utilities for analyzing struct fields to determine
//! their characteristics (optional, collection, etc.) for builder generation.

use syn::{Field, GenericArgument, PathArguments, Type};

/// Information about a field extracted for builder generation.
#[derive(Clone)]
pub struct FieldInfo {
    /// The field's identifier
    pub name: syn::Ident,
    /// The field's type as declared
    pub ty: Type,
    /// Whether this field is wrapped in Option<T>
    pub is_optional: bool,
    /// The inner type T if this is Option<T>, otherwise None
    pub inner_type: Option<Type>,
    /// Whether this field is a Vec<T>
    pub is_vec: bool,
}

impl FieldInfo {
    /// Analyzes a field and extracts information needed for builder generation.
    pub fn from_field(field: &Field) -> syn::Result<Self> {
        let name = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new_spanned(field, "Field must have a name"))?;

        let ty = field.ty.clone();
        let (is_optional, inner_type) = extract_option_inner_type(&ty);
        let is_vec = is_vec_type(&ty);

        Ok(FieldInfo {
            name,
            ty,
            is_optional,
            inner_type,
            is_vec,
        })
    }

    /// Gets the type to use for the setter method parameter.
    ///
    /// For Option<T> fields, this returns T (unwrapped).
    /// For other fields, this returns the original type.
    pub fn setter_param_type(&self) -> &Type {
        if let Some(inner) = &self.inner_type {
            inner
        } else {
            &self.ty
        }
    }

    /// Gets the type to use in the builder struct.
    ///
    /// All builder fields are wrapped in Option<T> to track whether they've been set.
    pub fn builder_field_type(&self) -> Type {
        let inner = self.setter_param_type();
        syn::parse_quote! { ::std::option::Option<#inner> }
    }
}

/// Checks if a type is `Option<T>` and extracts the inner type T.
///
/// Returns (is_option, inner_type) where:
/// - is_option is true if the type is Option<T>
/// - inner_type is Some(T) if the type is Option<T>, None otherwise
fn extract_option_inner_type(ty: &Type) -> (bool, Option<Type>) {
    if let Type::Path(type_path) = ty {
        if type_path.qself.is_none() {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Option" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                            return (true, Some(inner_ty.clone()));
                        }
                    }
                }
            }
        }
    }
    (false, None)
}

/// Checks if a type is `Vec<T>`.
fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.qself.is_none() {
            if let Some(segment) = type_path.path.segments.last() {
                return segment.ident == "Vec";
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_detect_option_type() {
        let ty: Type = syn::parse2(quote! { Option<String> }).unwrap();
        let (is_option, inner) = extract_option_inner_type(&ty);
        assert!(is_option);
        assert!(inner.is_some());
    }

    #[test]
    fn test_detect_non_option_type() {
        let ty: Type = syn::parse2(quote! { String }).unwrap();
        let (is_option, inner) = extract_option_inner_type(&ty);
        assert!(!is_option);
        assert!(inner.is_none());
    }

    #[test]
    fn test_detect_vec_type() {
        let ty: Type = syn::parse2(quote! { Vec<String> }).unwrap();
        assert!(is_vec_type(&ty));
    }

    #[test]
    fn test_detect_non_vec_type() {
        let ty: Type = syn::parse2(quote! { String }).unwrap();
        assert!(!is_vec_type(&ty));
    }

    #[test]
    fn test_field_info_from_optional_field() {
        // Parse a complete struct to get a field
        let input: syn::DeriveInput = syn::parse2(quote! {
            struct Test {
                pub name: Option<String>
            }
        })
        .unwrap();

        if let syn::Data::Struct(data) = input.data {
            if let syn::Fields::Named(fields) = data.fields {
                let field = fields.named.first().unwrap();
                let info = FieldInfo::from_field(field).unwrap();
                assert_eq!(info.name, "name");
                assert!(info.is_optional);
                assert!(info.inner_type.is_some());
            }
        }
    }

    #[test]
    fn test_field_info_from_required_field() {
        // Parse a complete struct to get a field
        let input: syn::DeriveInput = syn::parse2(quote! {
            struct Test {
                pub name: String
            }
        })
        .unwrap();

        if let syn::Data::Struct(data) = input.data {
            if let syn::Fields::Named(fields) = data.fields {
                let field = fields.named.first().unwrap();
                let info = FieldInfo::from_field(field).unwrap();
                assert_eq!(info.name, "name");
                assert!(!info.is_optional);
                assert!(info.inner_type.is_none());
            }
        }
    }
}
