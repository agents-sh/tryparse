//! Enum code generation for LlmDeserialize derive macro.
//!
//! Handles regular enums, internally-tagged enums, and untagged enums.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Fields;

use crate::attributes::{
    apply_rename_all_at_compile_time, extract_tag_info, has_untagged_attribute,
    validate_variant_rename_all, TaggedEnumInfo,
};

/// Generate deserialization code for enums.
pub fn generate_enum_deserialize(
    name: &syn::Ident,
    data: &syn::DataEnum,
    attrs: &[syn::Attribute],
    tryparse_crate: &TokenStream,
) -> TokenStream {
    // Check if this is an untagged enum
    if has_untagged_attribute(attrs) {
        return generate_untagged_enum_deserialize(name, data, tryparse_crate);
    }

    // Check if this is an internally-tagged enum
    match extract_tag_info(attrs) {
        Ok(Some(tag_info)) => return generate_tagged_enum_deserialize(name, data, tag_info, tryparse_crate),
        Ok(None) => {}          // Continue with regular enum deserialization
        Err(err) => return err, // Return compile error
    }

    let name_str = name.to_string();

    // Build EnumMatcher setup with all variants
    let matcher_setup = data.variants.iter().map(|v| {
        let variant_name = v.ident.to_string();
        quote! {
            .variant(#tryparse_crate::deserializer::enum_coercer::EnumVariant::new(#variant_name))
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
                    #variant_name => Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                        #tryparse_crate::error::DeserializeError::Custom(
                            format!("Enum variant '{}' has fields - derive macro only supports unit variants", #variant_name)
                        )
                    )),
                }
            }
        }
    });

    quote! {
        fn deserialize(
            value: &#tryparse_crate::value::FlexValue,
            _ctx: &mut #tryparse_crate::deserializer::CoercionContext,
        ) -> #tryparse_crate::error::Result<Self> {
            // Build matcher with all enum variants
            let matcher = #tryparse_crate::deserializer::enum_coercer::EnumMatcher::new()
                #(#matcher_setup)*;

            // Use BAML's fuzzy matching to find the best variant
            let matched_variant = #tryparse_crate::deserializer::enum_coercer::match_enum_variant(
                value,
                &matcher
            )?;

            // Construct the matched variant
            match matched_variant.as_str() {
                #(#match_arms)*
                _ => Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                    #tryparse_crate::error::DeserializeError::UnknownVariant {
                        enum_name: #name_str.to_string(),
                        variant: matched_variant,
                        suggestion: None, // No suggestion at derive level
                    }
                )),
            }
        }
    }
}

/// Generate deserialization code for internally-tagged enums.
///
/// Handles both internally-tagged: #[serde(tag = "type")]
/// and adjacently-tagged: #[serde(tag = "type", content = "data")] enums.
fn generate_tagged_enum_deserialize(
    _name: &syn::Ident,
    data: &syn::DataEnum,
    tag_info: TaggedEnumInfo,
    tryparse_crate: &TokenStream,
) -> TokenStream {
    let tag_field = &tag_info.tag_field;
    let content_field = &tag_info.content_field;
    let rename_all = tag_info.rename_all.as_deref().unwrap_or("none");

    // Validate all variant-level rename_all attributes first
    for v in &data.variants {
        if let Err(err) = validate_variant_rename_all(v) {
            return err;
        }
    }

    // Build a map of variant names after applying rename_all transformation
    let variant_names: Vec<_> = data.variants.iter().map(|v| v.ident.to_string()).collect();

    // Extract field names from each variant for fuzzy matching
    let variant_fields: Vec<Vec<String>> = data
        .variants
        .iter()
        .map(|v| match &v.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect(),
            _ => vec![],
        })
        .collect();

    // Manually apply rename_all transformation at compile time for matching
    let variant_names_normalized: Vec<String> = variant_names
        .iter()
        .map(|name| apply_rename_all_at_compile_time(name, rename_all))
        .collect();

    // Build EnumMatcher setup with all variants
    let matcher_setup = variant_names.iter().map(|variant_name| {
        quote! {
            .variant(#tryparse_crate::deserializer::enum_coercer::EnumVariant::new(#variant_name))
        }
    });

    // Generate deserialization code based on whether this is internally or adjacently tagged
    let deserialization_code = if let Some(content_field_name) = content_field {
        // Adjacently-tagged enum
        quote! {
            {
                // Construct object with tag and content fields only
                let mut adjacently_tagged_obj = serde_json::Map::new();
                adjacently_tagged_obj.insert(#tag_field.to_string(), Value::String(normalized_variant.clone()));

                // Extract content from original object
                if let Some(content_value) = obj.get(#content_field_name) {
                    adjacently_tagged_obj.insert(#content_field_name.to_string(), content_value.clone());
                }

                let normalized_value = Value::Object(adjacently_tagged_obj);
                <Self as ::serde::Deserialize>::deserialize(normalized_value)
            }
        }
    } else {
        // Internally-tagged enum
        quote! {
            {
                let normalized_value = Value::Object(normalized_obj);
                <Self as ::serde::Deserialize>::deserialize(normalized_value)
            }
        }
    };

    quote! {
        fn deserialize(
            value: &#tryparse_crate::value::FlexValue,
            ctx: &mut #tryparse_crate::deserializer::CoercionContext,
        ) -> #tryparse_crate::error::Result<Self> {
            use serde::Deserialize;
            use serde_json::Value;

            // Must be an object for internally-tagged enums
            let obj = match &value.value {
                Value::Object(obj) => obj,
                _ => {
                    return Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                        #tryparse_crate::error::DeserializeError::type_mismatch(
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
                    #tryparse_crate::error::ParseError::DeserializeFailed(
                        #tryparse_crate::error::DeserializeError::Custom(
                            format!("Missing or non-string tag field '{}'", #tag_field)
                        )
                    )
                })?;

            // Build matcher with all enum variants
            let matcher = #tryparse_crate::deserializer::enum_coercer::EnumMatcher::new()
                #(#matcher_setup)*;

            // Use fuzzy matching to find the best variant
            let matched_variant = matcher.match_string(tag_value)
                .map_err(|_| {
                    // Build list of valid variant names
                    let valid_variants = vec![#(#variant_names),*];

                    // Find closest match using levenshtein distance
                    let closest = valid_variants.iter()
                        .min_by_key(|v| #tryparse_crate::deserializer::enum_coercer::levenshtein_distance(tag_value, v))
                        .map(|s| *s)
                        .unwrap_or("");

                    #tryparse_crate::error::ParseError::DeserializeFailed(
                        #tryparse_crate::error::DeserializeError::Custom(
                            format!(
                                "Unknown variant '{}' for tag field '{}'. Valid variants: [{}]. Did you mean '{}'?",
                                tag_value,
                                #tag_field,
                                valid_variants.join(", "),
                                closest
                            )
                        )
                    )
                })?;

            // Apply rename_all transformation to the matched variant
            let normalized_variant = #tryparse_crate::deserializer::struct_coercer::apply_rename_all(
                &matched_variant,
                #rename_all
            );

            // Get expected field names for this variant
            let expected_fields: Vec<&str> = match normalized_variant.as_str() {
                #(
                    #variant_names_normalized => vec![#(#variant_fields),*],
                )*
                _ => vec![],
            };

            // Clone object for normalization
            let mut normalized_obj = obj.clone();

            // Track if tag was normalized
            if tag_value != &normalized_variant {
                ctx.add_transformation(#tryparse_crate::value::Transformation::FieldNameCaseChanged {
                    from: tag_value.to_string(),
                    to: normalized_variant.clone(),
                });
                normalized_obj.insert(#tag_field.to_string(), Value::String(normalized_variant.clone()));
            }

            // Fuzzy match and normalize field names for internally-tagged enums
            // (adjacently-tagged enums don't need this as fields are in content)
            for expected_field in expected_fields {
                let matcher = #tryparse_crate::deserializer::struct_coercer::FieldMatcher::new(expected_field);
                if let Some((json_key, _)) = matcher.find_in_object(&obj) {
                    if json_key != expected_field {
                        // Field name differs - normalize it
                        if let Some(value) = normalized_obj.remove(json_key) {
                            normalized_obj.insert(expected_field.to_string(), value);
                            ctx.add_transformation(#tryparse_crate::value::Transformation::FieldNameCaseChanged {
                                from: json_key.clone(),
                                to: expected_field.to_string(),
                            });
                        }
                    }
                }
            }

            // Prepare value for deserialization
            #deserialization_code
                .map_err(|e| {
                    #tryparse_crate::error::ParseError::DeserializeFailed(
                        #tryparse_crate::error::DeserializeError::Custom(
                            format!("Failed to deserialize tagged enum: {}", e)
                        )
                    )
                })
        }
    }
}

/// Generate untagged enum deserialization code for enums with #[serde(untagged)].
///
/// Tries each variant in order and picks the best match based on transformation penalties.
fn generate_untagged_enum_deserialize(
    name: &syn::Ident,
    data: &syn::DataEnum,
    tryparse_crate: &TokenStream,
) -> TokenStream {
    let name_str = name.to_string();

    // Process all variants
    let variant_attempts: Vec<_> = data
        .variants
        .iter()
        .enumerate()
        .map(|(idx, variant)| {
            let variant_idx = idx;

            match &variant.fields {
                Fields::Unit => {
                    // Unit variant - can only match null or string matching variant name
                    quote! {
                        {
                            // Try to match unit variant
                            if let Ok(()) = value.value.as_null().ok_or_else(|| #tryparse_crate::error::ParseError::DeserializeFailed(
                                #tryparse_crate::error::DeserializeError::Custom("not null".to_string())
                            )) {
                                variant_matches.push((#variant_idx, 0));
                            }
                        }
                    }
                }
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    // Tuple variant with single field
                    let field_type = &fields.unnamed[0].ty;
                    quote! {
                        {
                            // Clone context to track transformations independently
                            let value_clone = value.clone();
                            let mut attempt_ctx = #tryparse_crate::deserializer::CoercionContext::new();

                            if let Ok(_val) = <#field_type as #tryparse_crate::deserializer::LlmDeserialize>::deserialize(&value_clone, &mut attempt_ctx) {
                                let score: u32 = attempt_ctx.transformations().iter().map(|t| t.penalty()).sum();
                                variant_matches.push((#variant_idx, score));
                            }
                        }
                    }
                }
                Fields::Named(_) => {
                    // Struct variant - use serde to try deserialization
                    let variant_ident = &variant.ident;
                    quote! {
                        {
                            // For struct variants, try serde deserialization
                            if let Ok(result) = <Self as ::serde::Deserialize>::deserialize(&value.value) {
                                // Check if it's the right variant
                                match result {
                                    Self::#variant_ident { .. } => {
                                        // This variant matched
                                        variant_matches.push((#variant_idx, 0));
                                    }
                                    _ => {
                                        // Different variant matched, skip
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Multi-field tuple variants not supported
                    quote! {}
                }
            }
        })
        .collect();

    // Build match arms for final deserialization
    let match_arms: Vec<_> = data
        .variants
        .iter()
        .enumerate()
        .map(|(idx, variant)| {
            let variant_ident = &variant.ident;
            let variant_idx = idx;

            match &variant.fields {
                Fields::Unit => {
                    quote! {
                        #variant_idx => Ok(Self::#variant_ident),
                    }
                }
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed[0].ty;
                    quote! {
                        #variant_idx => {
                            let val = <#field_type as #tryparse_crate::deserializer::LlmDeserialize>::deserialize(value, ctx)?;
                            Ok(Self::#variant_ident(val))
                        },
                    }
                }
                Fields::Named(_) => {
                    quote! {
                        #variant_idx => {
                            // For struct variants, deserialize the whole enum and verify it's the right variant
                            let result = <Self as ::serde::Deserialize>::deserialize(&value.value)
                                .map_err(|e| #tryparse_crate::error::ParseError::DeserializeFailed(
                                    #tryparse_crate::error::DeserializeError::Custom(format!("serde error: {}", e))
                                ))?;
                            Ok(result)
                        },
                    }
                }
                _ => {
                    quote! {
                        #variant_idx => Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                            #tryparse_crate::error::DeserializeError::Custom(
                                "Multi-field tuple variants not supported in untagged enums".to_string()
                            )
                        )),
                    }
                }
            }
        })
        .collect();

    // Also generate try_deserialize attempts (strict matching)
    let strict_attempts: Vec<_> = data
        .variants
        .iter()
        .enumerate()
        .map(|(idx, variant)| {
            let variant_idx = idx;

            match &variant.fields {
                Fields::Unit => {
                    quote! {
                        if value.value.is_null() {
                            strict_matches.push(#variant_idx);
                        }
                    }
                }
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed[0].ty;
                    quote! {
                        if let Some(_) = <#field_type as #tryparse_crate::deserializer::LlmDeserialize>::try_deserialize(value, &mut #tryparse_crate::deserializer::CoercionContext::new()) {
                            strict_matches.push(#variant_idx);
                        }
                    }
                }
                _ => quote! {}
            }
        })
        .collect();

    quote! {
        fn deserialize(
            value: &#tryparse_crate::value::FlexValue,
            ctx: &mut #tryparse_crate::deserializer::CoercionContext,
        ) -> #tryparse_crate::error::Result<Self> {
            use #tryparse_crate::deserializer::LlmDeserialize;
            use serde::Deserialize;

            // PHASE 1: Try strict matching first (try_deserialize - no coercion)
            let mut strict_matches: Vec<usize> = Vec::new();

            #(#strict_attempts)*

            // If exactly one strict match, use it
            if strict_matches.len() == 1 {
                let best_variant_idx = strict_matches[0];
                return match best_variant_idx {
                    #(#match_arms)*
                    _ => unreachable!(),
                };
            }

            // PHASE 2: If no strict matches or multiple matches, try lenient with scoring
            let mut variant_matches: Vec<(usize, u32)> = Vec::new();

            #(#variant_attempts)*

            // If no matches, return error
            if variant_matches.is_empty() {
                return Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                    #tryparse_crate::error::DeserializeError::Custom(
                        format!("No variant of {} matched the input", #name_str)
                    )
                ));
            }

            // Sort by score (lowest = best)
            variant_matches.sort_by_key(|(_, score)| *score);

            // Deserialize using the best match
            let best_variant_idx = variant_matches[0].0;
            match best_variant_idx {
                #(#match_arms)*
                _ => unreachable!(),
            }
        }
    }
}
