//! Attribute parsing for LlmDeserialize derive macro.
//!
//! Handles parsing of #[llm(...)] and #[serde(...)] attributes.

use proc_macro2::TokenStream;

/// Metadata about an internally-tagged enum.
#[derive(Debug, Clone)]
pub struct TaggedEnumInfo {
    /// The tag field name (e.g., "type")
    pub tag_field: String,
    /// The content field name for adjacently-tagged enums (e.g., "data")
    pub content_field: Option<String>,
    /// The rename_all rule (e.g., "snake_case")
    pub rename_all: Option<String>,
}

/// Check if enum has #[llm(union)] attribute.
pub fn has_union_attribute(attrs: &[syn::Attribute]) -> bool {
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

/// Check if enum has #[serde(untagged)] attribute
pub fn has_untagged_attribute(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("serde") {
            // Parse as #[serde(untagged)]
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("untagged") {
                    found = true;
                }
                Ok(())
            });
            return found;
        }
        false
    })
}

/// Extract serde tag information from enum attributes.
///
/// Looks for #[serde(tag = "type")] and #[serde(rename_all = "snake_case")].
/// Returns Err with compile error if rename_all has an invalid value.
pub fn extract_tag_info(attrs: &[syn::Attribute]) -> Result<Option<TaggedEnumInfo>, TokenStream> {
    let mut tag_field: Option<String> = None;
    let mut content_field: Option<String> = None;
    let mut rename_all: Option<String> = None;
    let mut rename_all_lit: Option<syn::LitStr> = None;

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }

        // Use parse_nested_meta for syn 2.0
        if let Err(e) = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                // Parse tag = "value"
                let value = meta.value()?;

                // Try to parse as expression first to detect const paths
                match value.parse::<syn::Expr>() {
                    Ok(syn::Expr::Lit(expr_lit)) => {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            tag_field = Some(lit_str.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &expr_lit.lit,
                                "tag attribute value must be a string literal",
                            ));
                        }
                    }
                    Ok(syn::Expr::Path(_path)) => {
                        return Err(syn::Error::new_spanned(
                            meta.path,
                            "tag attribute must be a string literal, const paths are not supported",
                        ));
                    }
                    Ok(other) => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "tag attribute value must be a string literal",
                        ));
                    }
                    Err(e) => return Err(e),
                }
            } else if meta.path.is_ident("content") {
                // Parse content = "value" for adjacently-tagged enums
                let value = meta.value()?;

                // Try to parse as expression first to detect const paths
                match value.parse::<syn::Expr>() {
                    Ok(syn::Expr::Lit(expr_lit)) => {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            content_field = Some(lit_str.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &expr_lit.lit,
                                "content attribute value must be a string literal",
                            ));
                        }
                    }
                    Ok(syn::Expr::Path(_path)) => {
                        return Err(syn::Error::new_spanned(
                            meta.path,
                            "content attribute must be a string literal, const paths are not supported",
                        ));
                    }
                    Ok(other) => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "content attribute value must be a string literal",
                        ));
                    }
                    Err(e) => return Err(e),
                }
            } else if meta.path.is_ident("rename_all") {
                // Parse rename_all = "value"
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                rename_all = Some(lit.value());
                rename_all_lit = Some(lit);
            }
            Ok(())
        }) {
            eprintln!("Warning: Failed to parse serde attribute: {}", e);
        }
    }

    // Validate rename_all value if present
    if let Some(rule) = &rename_all {
        let valid = [
            "snake_case",
            "camelCase",
            "PascalCase",
            "kebab-case",
            "SCREAMING_SNAKE_CASE",
        ];
        if !valid.contains(&rule.as_str()) {
            // Use the stored LitStr for proper error span
            if let Some(lit) = rename_all_lit {
                let error = syn::Error::new_spanned(
                    lit,
                    format!(
                        "Invalid rename_all value: '{}'. Valid values: {}",
                        rule,
                        valid.join(", ")
                    ),
                );
                return Err(error.to_compile_error());
            }
        }
    }

    Ok(tag_field.map(|tag| TaggedEnumInfo {
        tag_field: tag,
        content_field,
        rename_all,
    }))
}

/// Apply rename_all transformation at compile time (in proc macro).
/// This is used to pre-compute normalized variant names for matching.
pub fn apply_rename_all_at_compile_time(s: &str, rule: &str) -> String {
    match rule {
        "snake_case" => {
            let mut result = String::new();
            for ch in s.chars() {
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
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
            }
        }
        "PascalCase" => {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
        "SCREAMING_SNAKE_CASE" => {
            let mut result = String::new();
            for ch in s.chars() {
                if ch.is_uppercase() && !result.is_empty() {
                    result.push('_');
                }
                result.push(ch.to_ascii_uppercase());
            }
            result
        }
        "kebab-case" => {
            let mut result = String::new();
            for ch in s.chars() {
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
        _ => s.to_string(),
    }
}

/// Validate variant-level rename_all attribute and print warning if invalid.
pub fn validate_variant_rename_all(variant: &syn::Variant) {
    for attr in &variant.attrs {
        if attr.path().is_ident("serde") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    let rule = lit.value();

                    let valid = [
                        "snake_case",
                        "camelCase",
                        "PascalCase",
                        "kebab-case",
                        "SCREAMING_SNAKE_CASE",
                    ];

                    if !valid.contains(&rule.as_str()) {
                        eprintln!(
                            "Warning: Invalid variant-level rename_all value '{}' on variant '{}'. Valid values: {}",
                            rule,
                            variant.ident,
                            valid.join(", ")
                        );
                    }
                }
                Ok(())
            });
        }
    }
}
