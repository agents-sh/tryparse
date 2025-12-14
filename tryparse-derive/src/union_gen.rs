//! Union type code generation for LlmDeserialize derive macro.
//!
//! Handles enums marked with #[llm(union)] for score-based variant selection.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Fields;

/// Generate union deserialization code for enums with #[llm(union)].
///
/// Union types try each variant and pick the best match based on transformation penalties.
pub fn generate_union_deserialize(
    name: &syn::Ident,
    data: &syn::DataEnum,
    _attrs: &[syn::Attribute],
    tryparse_crate: &TokenStream,
) -> TokenStream {
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
            value: &#tryparse_crate::value::FlexValue,
            ctx: &mut #tryparse_crate::deserializer::CoercionContext,
        ) -> #tryparse_crate::error::Result<Self> {
            use #tryparse_crate::deserializer::LlmDeserialize;

            // BAML ALGORITHM: Try strict matching first (try_cast)
            if let Some(v1) = <#variant1_type as LlmDeserialize>::try_deserialize(value, ctx) {
                // Add UnionMatch transformation for strict match
                ctx.add_transformation(#tryparse_crate::value::Transformation::UnionMatch {
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
                ctx.add_transformation(#tryparse_crate::value::Transformation::UnionMatch {
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
                return Err(#tryparse_crate::error::ParseError::DeserializeFailed(
                    #tryparse_crate::error::DeserializeError::Custom(
                        "No union variant matched".to_string()
                    )
                ));
            }

            // Sort by score (lower is better)
            matches.sort_by_key(|m| m.score);

            // Add UnionMatch transformation to track which variant was selected
            let variant_index = (matches[0].variant - 1) as usize;
            ctx.add_transformation(#tryparse_crate::value::Transformation::UnionMatch {
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
