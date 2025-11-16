//! Derive macros for tryparse
//!
//! This crate provides the `LlmDeserialize` derive macro for automatically
//! generating fuzzy deserialization logic from Rust types.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

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

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match &input.data {
        Data::Struct(data_struct) => {
            let deserialize_impl = generate_struct_deserialize(name, data_struct);

            let expanded = quote! {
                impl #impl_generics ::tryparse::deserializer::LlmDeserialize for #name #ty_generics #where_clause {
                    #deserialize_impl
                }
            };

            TokenStream::from(expanded)
        }
        Data::Enum(data_enum) => {
            // Check if this is a union enum (has #[llm(union)] attribute)
            let is_union = has_union_attribute(&input.attrs);

            let deserialize_impl = if is_union {
                generate_union_deserialize(name, data_enum, &input.attrs)
            } else {
                generate_enum_deserialize(name, data_enum, &input.attrs)
            };

            let expanded = quote! {
                impl #impl_generics ::tryparse::deserializer::LlmDeserialize for #name #ty_generics #where_clause {
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

fn generate_struct_deserialize(
    name: &syn::Ident,
    data: &syn::DataStruct,
) -> proc_macro2::TokenStream {
    match &data.fields {
        Fields::Named(fields) => {
            let field_names: Vec<_> = fields.named.iter().map(|f| &f.ident).collect();
            let field_types: Vec<_> = fields.named.iter().map(|f| &f.ty).collect();
            let field_name_strs: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();

            // Check if each field is Option<T>
            let is_optional: Vec<_> = field_types.iter().map(|ty| is_option_type(ty)).collect();

            // Extract inner type for Option<T> fields
            let inner_types: Vec<_> = field_types
                .iter()
                .zip(&is_optional)
                .map(|(ty, opt)| {
                    if *opt {
                        extract_option_inner(ty)
                    } else {
                        (*ty).clone()
                    }
                })
                .collect();

            let name_str = name.to_string();

            // Generate field descriptor setup (collect to Vec for reuse)
            let field_descriptors: Vec<_> = field_name_strs
                .iter()
                .zip(&field_types)
                .zip(&is_optional)
                .map(|((name, ty), opt)| {
                    let type_name = quote!(stringify!(#ty)).to_string();
                    quote! {
                        .field(::tryparse::deserializer::FieldDescriptor::new(
                            #name,
                            #type_name,
                            #opt
                        ))
                    }
                })
                .collect();

            // Generate field extraction for try_deserialize (returns Option)
            let field_extractions_strict: Vec<_> = field_names
                .iter()
                .zip(&inner_types)
                .zip(&is_optional)
                .map(|((field_name, inner_ty), opt)| {
                    let field_name_str = field_name.as_ref().unwrap().to_string();
                    if *opt {
                        // Optional field
                        quote! {
                            let #field_name = fields.get(#field_name_str)
                                .and_then(|v| v.downcast_ref::<#inner_ty>())
                                .cloned();
                        }
                    } else {
                        // Required field - return None if missing
                        quote! {
                            let #field_name = fields.get(#field_name_str)
                                .and_then(|v| v.downcast_ref::<#inner_ty>())
                                .cloned()?;
                        }
                    }
                })
                .collect();

            // Generate field extraction for deserialize (returns Result)
            let field_extractions_lenient: Vec<_> = field_names.iter().zip(&inner_types).zip(&is_optional).map(|((field_name, inner_ty), opt)| {
                let field_name_str = field_name.as_ref().unwrap().to_string();
                if *opt {
                    // Optional field
                    quote! {
                        let #field_name = fields.get(#field_name_str)
                            .and_then(|v| v.downcast_ref::<#inner_ty>())
                            .cloned();
                    }
                } else {
                    // Required field
                    quote! {
                        let #field_name = fields.get(#field_name_str)
                            .and_then(|v| v.downcast_ref::<#inner_ty>())
                            .cloned()
                            .ok_or_else(|| ::tryparse::error::ParseError::DeserializeFailed(
                                ::tryparse::error::DeserializeError::missing_field(#field_name_str)
                            ))?;
                    }
                }
            }).collect();

            quote! {
                fn try_deserialize(
                    value: &::tryparse::value::FlexValue,
                    ctx: &mut ::tryparse::deserializer::CoercionContext,
                ) -> Option<Self> {
                    use std::any::Any;

                    let mut deserializer = ::tryparse::deserializer::StructDeserializer::new()
                        #(#field_descriptors)*;

                    let fields = deserializer.try_deserialize(
                        value,
                        ctx,
                        #name_str,
                        |field_name, field_value, field_ctx| {
                            // Dispatch to the appropriate field type's LlmDeserialize impl (strict mode only)
                            match field_name {
                                #(
                                    #field_name_strs => {
                                        // Try strict deserialization
                                        <#inner_types as ::tryparse::deserializer::LlmDeserialize>::try_deserialize(field_value, field_ctx)
                                            .map(|v| Box::new(v) as Box<dyn Any>)
                                    }
                                )*
                                _ => None
                            }
                        }
                    ).ok()?;

                    // Extract fields from Box<dyn Any> (strict mode - return None on failure)
                    #(#field_extractions_strict)*

                    Some(Self {
                        #(#field_names),*
                    })
                }

                fn deserialize(
                    value: &::tryparse::value::FlexValue,
                    ctx: &mut ::tryparse::deserializer::CoercionContext,
                ) -> ::tryparse::error::Result<Self> {
                    use std::any::Any;

                    let mut deserializer = ::tryparse::deserializer::StructDeserializer::new()
                        #(#field_descriptors)*;

                    let fields = deserializer.deserialize(
                        value,
                        ctx,
                        #name_str,
                        |field_name, field_value, field_ctx, strict| {
                            // Dispatch to the appropriate field type's LlmDeserialize impl
                            match field_name {
                                #(
                                    #field_name_strs => {
                                        if strict {
                                            // Try strict deserialization
                                            if let Some(v) = <#inner_types as ::tryparse::deserializer::LlmDeserialize>::try_deserialize(field_value, field_ctx) {
                                                Ok(Box::new(v) as Box<dyn Any>)
                                            } else {
                                                Err(::tryparse::error::ParseError::DeserializeFailed(
                                                    ::tryparse::error::DeserializeError::type_mismatch(
                                                        stringify!(#inner_types),
                                                        "value"
                                                    )
                                                ))
                                            }
                                        } else {
                                            // Lenient deserialization
                                            let v = <#inner_types as ::tryparse::deserializer::LlmDeserialize>::deserialize(field_value, field_ctx)?;
                                            Ok(Box::new(v) as Box<dyn Any>)
                                        }
                                    }
                                )*
                                _ => Err(::tryparse::error::ParseError::DeserializeFailed(
                                    ::tryparse::error::DeserializeError::Custom(
                                        format!("Unknown field: {}", field_name)
                                    )
                                ))
                            }
                        }
                    )?;

                    // Extract fields from Box<dyn Any> (lenient mode - return error on failure)
                    #(#field_extractions_lenient)*

                    Ok(Self {
                        #(#field_names),*
                    })
                }
            }
        }
        Fields::Unnamed(_) => syn::Error::new_spanned(
            data.fields.clone(),
            "LlmDeserialize does not support tuple structs yet",
        )
        .to_compile_error(),
        Fields::Unit => syn::Error::new_spanned(
            data.fields.clone(),
            "LlmDeserialize does not support unit structs",
        )
        .to_compile_error(),
    }
}

/// Check if a type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Extract the inner type T from Option<T>
fn extract_option_inner(ty: &Type) -> Type {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return inner.clone();
                    }
                }
            }
        }
    }
    // Fallback: return the original type
    ty.clone()
}

fn generate_enum_deserialize(
    name: &syn::Ident,
    data: &syn::DataEnum,
    attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
    // Check if this is an internally-tagged enum
    if let Some(tag_info) = extract_tag_info(attrs) {
        return generate_tagged_enum_deserialize(name, data, tag_info);
    }

    let name_str = name.to_string();

    // Build EnumMatcher setup with all variants
    let matcher_setup = data.variants.iter().map(|v| {
        let variant_name = v.ident.to_string();
        quote! {
            .variant(::tryparse::deserializer::enum_coercer::EnumVariant::new(#variant_name))
        }
    });

    // Build match arms for each variant
    let match_arms = data.variants.iter().map(|v| {
        let variant_ident = &v.ident;
        let variant_name = v.ident.to_string();

        match &v.fields {
            Fields::Unit => {
                // Simple unit variant (e.g., Status::Active)
                quote! {
                    #variant_name => Ok(Self::#variant_ident),
                }
            }
            Fields::Named(_) | Fields::Unnamed(_) => {
                // Complex variants with fields - not yet supported in derive macro
                // Users can implement LlmDeserialize manually for these cases
                quote! {
                    #variant_name => Err(::tryparse::error::ParseError::DeserializeFailed(
                        ::tryparse::error::DeserializeError::Custom(
                            format!("Enum variant '{}' has fields - derive macro only supports unit variants", #variant_name)
                        )
                    )),
                }
            }
        }
    });

    quote! {
        fn deserialize(
            value: &::tryparse::value::FlexValue,
            _ctx: &mut ::tryparse::deserializer::CoercionContext,
        ) -> ::tryparse::error::Result<Self> {
            // Build matcher with all enum variants
            let matcher = ::tryparse::deserializer::enum_coercer::EnumMatcher::new()
                #(#matcher_setup)*;

            // Use BAML's fuzzy matching to find the best variant
            let matched_variant = ::tryparse::deserializer::enum_coercer::match_enum_variant(
                value,
                &matcher
            )?;

            // Construct the matched variant
            match matched_variant.as_str() {
                #(#match_arms)*
                _ => Err(::tryparse::error::ParseError::DeserializeFailed(
                    ::tryparse::error::DeserializeError::UnknownVariant {
                        enum_name: #name_str.to_string(),
                        variant: matched_variant,
                    }
                )),
            }
        }
    }
}

/// Generate deserialization code for internally-tagged enums.
///
/// Handles #[serde(tag = "type", rename_all = "snake_case")] enums with fuzzy tag matching.
fn generate_tagged_enum_deserialize(
    _name: &syn::Ident,
    data: &syn::DataEnum,
    tag_info: TaggedEnumInfo,
) -> proc_macro2::TokenStream {
    let tag_field = &tag_info.tag_field;
    let rename_all = tag_info.rename_all.as_deref().unwrap_or("none");

    // Build a map of variant names after applying rename_all transformation
    let variant_names: Vec<_> = data.variants.iter().map(|v| v.ident.to_string()).collect();

    // Build EnumMatcher setup with all variants
    let matcher_setup = variant_names.iter().map(|variant_name| {
        quote! {
            .variant(::tryparse::deserializer::enum_coercer::EnumVariant::new(#variant_name))
        }
    });

    quote! {
        fn deserialize(
            value: &::tryparse::value::FlexValue,
            ctx: &mut ::tryparse::deserializer::CoercionContext,
        ) -> ::tryparse::error::Result<Self> {
            use serde::Deserialize;
            use serde_json::Value;

            // Must be an object for internally-tagged enums
            let obj = match &value.value {
                Value::Object(obj) => obj,
                _ => {
                    return Err(::tryparse::error::ParseError::DeserializeFailed(
                        ::tryparse::error::DeserializeError::type_mismatch(
                            "object (internally-tagged enum)",
                            "non-object"
                        )
                    ));
                }
            };

            // Extract the tag field value
            let tag_value = obj.get(#tag_field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ::tryparse::error::ParseError::DeserializeFailed(
                        ::tryparse::error::DeserializeError::Custom(
                            format!("Missing or non-string tag field '{}'", #tag_field)
                        )
                    )
                })?;

            // Build matcher with all enum variants
            let matcher = ::tryparse::deserializer::enum_coercer::EnumMatcher::new()
                #(#matcher_setup)*;

            // Use fuzzy matching to find the best variant
            let matched_variant = matcher.match_string(tag_value)
                .map_err(|_| {
                    ::tryparse::error::ParseError::DeserializeFailed(
                        ::tryparse::error::DeserializeError::Custom(
                            format!("Unknown variant '{}' for tag field '{}'", tag_value, #tag_field)
                        )
                    )
                })?;

            // Apply rename_all transformation to the matched variant
            let normalized_variant = match #rename_all {
                "snake_case" => {
                    // Convert PascalCase or camelCase to snake_case
                    let mut result = String::new();
                    for ch in matched_variant.chars() {
                        if ch.is_uppercase() {
                            if !result.is_empty() {
                                result.push('_');
                            }
                            result.push(ch.to_ascii_lowercase());
                        } else {
                            result.push(ch);
                        }
                    }
                    result
                }
                "camelCase" => {
                    // Convert to camelCase (first char lowercase)
                    let mut chars = matched_variant.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
                    }
                }
                "PascalCase" => {
                    // Convert to PascalCase (first char uppercase)
                    let mut chars = matched_variant.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                }
                "SCREAMING_SNAKE_CASE" => {
                    // Convert to SCREAMING_SNAKE_CASE
                    let mut result = String::new();
                    for ch in matched_variant.chars() {
                        if ch.is_uppercase() && !result.is_empty() {
                            result.push('_');
                        }
                        result.push(ch.to_ascii_uppercase());
                    }
                    result
                }
                "kebab-case" => {
                    // Convert to kebab-case
                    let mut result = String::new();
                    for ch in matched_variant.chars() {
                        if ch.is_uppercase() {
                            if !result.is_empty() {
                                result.push('-');
                            }
                            result.push(ch.to_ascii_lowercase());
                        } else {
                            result.push(ch);
                        }
                    }
                    result
                }
                _ => matched_variant.clone(),
            };

            // Track transformation if tag value was normalized
            if tag_value != normalized_variant {
                ctx.add_transformation(::tryparse::value::Transformation::FieldNameCaseChanged {
                    from: tag_value.to_string(),
                    to: normalized_variant.clone(),
                });
            }

            // Clone the object and update the tag field with the normalized variant
            let mut normalized_obj = obj.clone();
            normalized_obj.insert(#tag_field.to_string(), Value::String(normalized_variant));

            // Deserialize using serde's standard Deserialize trait
            let normalized_value = Value::Object(normalized_obj);
            <Self as ::serde::Deserialize>::deserialize(normalized_value)
                .map_err(|e| {
                    ::tryparse::error::ParseError::DeserializeFailed(
                        ::tryparse::error::DeserializeError::Custom(
                            format!("Failed to deserialize internally-tagged enum: {}", e)
                        )
                    )
                })
        }
    }
}

/// Check if enum has #[llm(union)] attribute.
fn has_union_attribute(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("llm") {
            // Parse as #[llm(union)]
            if let Ok(meta_list) = attr.meta.require_list() {
                // Check if any nested item is "union"
                return meta_list.tokens.to_string().trim() == "union";
            }
        }
        false
    })
}

/// Metadata about an internally-tagged enum.
#[derive(Debug, Clone)]
struct TaggedEnumInfo {
    /// The tag field name (e.g., "type")
    tag_field: String,
    /// The rename_all rule (e.g., "snake_case")
    rename_all: Option<String>,
}

/// Extract serde tag information from enum attributes.
///
/// Looks for #[serde(tag = "type")] and #[serde(rename_all = "snake_case")].
fn extract_tag_info(attrs: &[syn::Attribute]) -> Option<TaggedEnumInfo> {
    let mut tag_field: Option<String> = None;
    let mut rename_all: Option<String> = None;

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }

        // Use parse_nested_meta for syn 2.0
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                // Parse tag = "value"
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                tag_field = Some(lit.value());
            } else if meta.path.is_ident("rename_all") {
                // Parse rename_all = "value"
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                rename_all = Some(lit.value());
            }
            Ok(())
        });
    }

    tag_field.map(|tag| TaggedEnumInfo {
        tag_field: tag,
        rename_all,
    })
}

/// Generate union deserialization code for enums with #[llm(union)].
fn generate_union_deserialize(
    name: &syn::Ident,
    data: &syn::DataEnum,
    _attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
    if data.variants.len() != 2 {
        return syn::Error::new_spanned(name, "Union enums must have exactly 2 variants")
            .to_compile_error();
    }

    let variants: Vec<_> = data.variants.iter().collect();
    let variant1 = &variants[0];
    let variant2 = &variants[1];

    // Extract variant types
    let (variant1_ident, variant1_type) = match &variant1.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            (&variant1.ident, &fields.unnamed[0].ty)
        }
        _ => {
            return syn::Error::new_spanned(
                variant1,
                "Union variants must have exactly one unnamed field",
            )
            .to_compile_error();
        }
    };

    let (variant2_ident, variant2_type) = match &variant2.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            (&variant2.ident, &fields.unnamed[0].ty)
        }
        _ => {
            return syn::Error::new_spanned(
                variant2,
                "Union variants must have exactly one unnamed field",
            )
            .to_compile_error();
        }
    };

    quote! {
        fn deserialize(
            value: &::tryparse::value::FlexValue,
            ctx: &mut ::tryparse::deserializer::CoercionContext,
        ) -> ::tryparse::error::Result<Self> {
            use ::tryparse::deserializer::LlmDeserialize;

            // BAML ALGORITHM: Try strict matching first (try_cast)
            if let Some(v1) = <#variant1_type as LlmDeserialize>::try_deserialize(value, ctx) {
                // Add UnionMatch transformation for strict match
                ctx.add_transformation(::tryparse::value::Transformation::UnionMatch {
                    index: 0,
                    candidates: vec![
                        stringify!(#variant1_type).to_string(),
                        stringify!(#variant2_type).to_string(),
                    ],
                });
                return Ok(Self::#variant1_ident(v1));
            }

            if let Some(v2) = <#variant2_type as LlmDeserialize>::try_deserialize(value, ctx) {
                // Add UnionMatch transformation for strict match
                ctx.add_transformation(::tryparse::value::Transformation::UnionMatch {
                    index: 1,
                    candidates: vec![
                        stringify!(#variant1_type).to_string(),
                        stringify!(#variant2_type).to_string(),
                    ],
                });
                return Ok(Self::#variant2_ident(v2));
            }

            // BAML ALGORITHM: Try lenient matching with scoring (coerce)
            struct MatchResult {
                variant: u8,  // 1 or 2
                score: u32,
            }

            let mut matches = Vec::new();

            // Try variant 1 with separate FlexValue to track transformations
            let value1 = value.clone();
            if let Ok(_) = <#variant1_type as LlmDeserialize>::deserialize(&value1, ctx) {
                let score: u32 = value1.transformations().iter().map(|t| t.penalty()).sum();
                matches.push(MatchResult { variant: 1, score });
            }

            // Try variant 2 with separate FlexValue to track transformations
            let value2 = value.clone();
            if let Ok(_) = <#variant2_type as LlmDeserialize>::deserialize(&value2, ctx) {
                let score: u32 = value2.transformations().iter().map(|t| t.penalty()).sum();
                matches.push(MatchResult { variant: 2, score });
            }

            if matches.is_empty() {
                return Err(::tryparse::error::ParseError::DeserializeFailed(
                    ::tryparse::error::DeserializeError::Custom(
                        "No union variant matched".to_string()
                    )
                ));
            }

            // Sort by score (lower is better)
            matches.sort_by_key(|m| m.score);

            // Add UnionMatch transformation to track which variant was selected
            let variant_index = (matches[0].variant - 1) as usize;
            ctx.add_transformation(::tryparse::value::Transformation::UnionMatch {
                index: variant_index,
                candidates: vec![
                    stringify!(#variant1_type).to_string(),
                    stringify!(#variant2_type).to_string(),
                ],
            });

            // Deserialize the best match
            match matches[0].variant {
                1 => {
                    let v1 = <#variant1_type as LlmDeserialize>::deserialize(value, ctx)?;
                    Ok(Self::#variant1_ident(v1))
                }
                2 => {
                    let v2 = <#variant2_type as LlmDeserialize>::deserialize(value, ctx)?;
                    Ok(Self::#variant2_ident(v2))
                }
                _ => unreachable!(),
            }
        }
    }
}
