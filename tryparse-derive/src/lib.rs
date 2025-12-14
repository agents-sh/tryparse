//! Derive macros for tryparse
//!
//! This crate provides the `LlmDeserialize` derive macro for automatically
//! generating fuzzy deserialization logic from Rust types.

mod attributes;
mod enum_gen;
mod struct_gen;
mod union_gen;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Ident};

use attributes::has_union_attribute;
use enum_gen::generate_enum_deserialize;
use struct_gen::generate_struct_deserialize;
use union_gen::generate_union_deserialize;

/// Finds the path to the tryparse crate, checking both direct dependency
/// and re-exports through other crates (like radkit).
fn get_tryparse_crate() -> TokenStream2 {
    // First, try to find tryparse directly
    if let Ok(found) = crate_name("tryparse") {
        return match found {
            // FoundCrate::Itself means we're compiling within the tryparse crate itself.
            // However, doc tests can't use `crate::` paths - they need fully qualified paths.
            // So we use `::tryparse` which works for both regular builds and doc tests.
            FoundCrate::Itself => quote!(::tryparse),
            FoundCrate::Name(name) => {
                let ident = Ident::new(&name, Span::call_site());
                quote!(::#ident)
            }
        };
    }

    // If not found directly, look for radkit which re-exports tryparse
    if let Ok(found) = crate_name("radkit") {
        match found {
            FoundCrate::Itself => return quote!(crate::__private_tryparse),
            FoundCrate::Name(name) => {
                let ident = Ident::new(&name, Span::call_site());
                return quote!(::#ident::__private_tryparse);
            }
        }
    }

    // Fallback to direct path (will fail at compile time with helpful error)
    quote!(::tryparse)
}

/// Derives the `LlmDeserialize` trait for structs and enums.
///
/// This macro generates a custom deserialization implementation using BAML's
/// algorithms for fuzzy field matching and type coercion.
///
/// # Features
///
/// - **Fuzzy field matching**: Handles different naming conventions (userName ↔ user_name)
/// - **Fuzzy enum matching**: Case-insensitive, substring, and edit-distance matching for variants
/// - **Union types**: Score-based variant selection with `#[llm(union)]`
/// - **Optional fields**: Automatic handling of `Option<T>` fields
/// - **Transformation tracking**: Records all coercions applied during parsing
///
/// # Example
///
/// ```ignore
/// use tryparse::deserializer::LlmDeserialize;
///
/// #[derive(LlmDeserialize)]
/// struct User {
///     name: String,
///     age: u32,
///     email: Option<String>, // Optional field
/// }
///
/// // Handles messy input like:
/// // {"userName": "Alice", "age": "30"}  // camelCase + string number
/// ```
///
/// # Union Types
///
/// ```ignore
/// #[derive(LlmDeserialize)]
/// #[llm(union)]
/// enum Value {
///     Number(i64),
///     Text(String),
/// }
///
/// // Automatically picks the best matching variant
/// ```
#[proc_macro_derive(LlmDeserialize, attributes(llm))]
pub fn derive_llm_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let tryparse_crate = get_tryparse_crate();

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match &input.data {
        Data::Struct(data_struct) => {
            let deserialize_impl = generate_struct_deserialize(name, data_struct, &tryparse_crate);
            let name_str = name.to_string();

            let expanded = quote! {
                // Compile-time check: LlmDeserialize requires serde::Deserialize
                const _: () = {
                    fn __assert_deserialize_impl<__T: ::serde::de::DeserializeOwned>() {}
                    fn __check_deserialize_bound() {
                        __assert_deserialize_impl::<#name #ty_generics>();
                    }

                    // Provide a helpful error message
                    #[doc = concat!(
                        "LlmDeserialize requires serde::Deserialize. ",
                        "Add `#[derive(serde::Deserialize)]` to `", #name_str, "`."
                    )]
                    const __LLMDESERIALIZE_REQUIRES_SERDE: () = ();
                };

                impl #impl_generics #tryparse_crate::deserializer::LlmDeserialize for #name #ty_generics #where_clause {
                    #deserialize_impl
                }
            };

            TokenStream::from(expanded)
        }
        Data::Enum(data_enum) => {
            // Check if this is a union enum (has #[llm(union)] attribute)
            let is_union = has_union_attribute(&input.attrs);

            let deserialize_impl = if is_union {
                generate_union_deserialize(name, data_enum, &input.attrs, &tryparse_crate)
            } else {
                generate_enum_deserialize(name, data_enum, &input.attrs, &tryparse_crate)
            };

            let name_str = name.to_string();

            let expanded = quote! {
                // Compile-time check: LlmDeserialize requires serde::Deserialize
                const _: () = {
                    fn __assert_deserialize_impl<__T: ::serde::de::DeserializeOwned>() {}
                    fn __check_deserialize_bound() {
                        __assert_deserialize_impl::<#name #ty_generics>();
                    }

                    // Provide a helpful error message
                    #[doc = concat!(
                        "LlmDeserialize requires serde::Deserialize. ",
                        "Add `#[derive(serde::Deserialize)]` to `", #name_str, "`."
                    )]
                    const __LLMDESERIALIZE_REQUIRES_SERDE: () = ();
                };

                impl #impl_generics #tryparse_crate::deserializer::LlmDeserialize for #name #ty_generics #where_clause {
                    #deserialize_impl
                }
            };

            TokenStream::from(expanded)
        }
        Data::Union(_) => {
            syn::Error::new_spanned(input, "LlmDeserialize cannot be derived for unions")
                .to_compile_error()
                .into()
        }
    }
}
