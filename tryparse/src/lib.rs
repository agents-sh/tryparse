//! # tryparse
//!
//! A forgiving parser that converts messy LLM responses into strongly-typed Rust structs.
//!
//! This library handles common issues in LLM outputs like:
//! - JSON wrapped in markdown code blocks
//! - Trailing commas
//! - Single quotes instead of double quotes
//! - Unquoted object keys
//! - Type mismatches (string numbers, etc.)
//!
//! ## Quick Start
//!
//! ```rust
//! use tryparse::parse;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Debug)]
//! struct User {
//!     name: String,
//!     age: u32,
//! }
//!
//! // Parse messy LLM output with unquoted keys and string numbers
//! let messy_response = r#"{name: "Alice", age: "30"}"#;
//!
//! let user: User = parse(messy_response).unwrap();
//! assert_eq!(user.name, "Alice");
//! assert_eq!(user.age, 30); // Automatically coerced from string
//! ```
//!
//! ## Features
//!
//! - **Multi-Strategy Parsing**: Tries multiple approaches to extract JSON
//! - **Smart Type Coercion**: Converts between compatible types automatically
//! - **Transformation Tracking**: Records all modifications made during parsing
//! - **Candidate Scoring**: Ranks multiple interpretations by quality
//! - **Zero Configuration**: Works out of the box with sensible defaults
//!
//! ## Advanced Usage
//!
//! For more control over the parsing process:
//!
//! ```rust
//! use tryparse::{parse_with_candidates, parser::FlexibleParser};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Data {
//!     value: i32,
//! }
//!
//! let response = r#"{"value": "42"}"#;
//!
//! // Get all candidates with metadata
//! let (result, candidates) = parse_with_candidates::<Data>(response).unwrap();
//!
//! // Or use the parser directly
//! let parser = FlexibleParser::new();
//! let flex_values = parser.parse(response).unwrap();
//! ```

pub mod constraints;
pub mod deserializer;
pub mod error;
pub mod parser;
pub mod scoring;
pub mod value;

// Ensure primitive type implementations are linked
// This prevents "trait bound not satisfied" errors in integration tests
#[doc(hidden)]
pub fn __ensure_primitives_linked() {
    deserializer::primitives::__ensure_linked();
}

use deserializer::{CoercingDeserializer, CoercionContext, LlmDeserialize};
use error::{ParseError, Result};
use parser::FlexibleParser;
use serde::de::DeserializeOwned;
use value::FlexValue;

/// Parses an LLM response into a strongly-typed Rust struct.
///
/// This is the main entry point for the library. It combines flexible parsing
/// with smart type coercion to handle messy LLM outputs.
///
/// # Examples
///
/// ```
/// use tryparse::parse;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// let response = r#"{"name": "Alice", "age": "30"}"#;
/// let user: User = parse(response).unwrap();
/// assert_eq!(user, User { name: "Alice".into(), age: 30 });
/// ```
///
/// # Errors
///
/// Returns `ParseError::NoCandidates` if no valid JSON could be extracted.
/// Returns `ParseError::DeserializeFailed` if deserialization fails for all candidates.
pub fn parse<T: DeserializeOwned>(input: &str) -> Result<T> {
    let (result, _candidates) = parse_with_candidates(input)?;
    Ok(result)
}

/// Parses an LLM response and returns both the result and all candidates.
///
/// This variant provides access to all parsing candidates with their metadata,
/// allowing inspection of what transformations were applied.
///
/// # Examples
///
/// ```
/// use tryparse::parse_with_candidates;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Data {
///     value: i32,
/// }
///
/// let response = r#"{"value": "42"}"#;
/// let (data, candidates) = parse_with_candidates::<Data>(response).unwrap();
///
/// assert_eq!(data.value, 42);
/// assert!(!candidates.is_empty());
/// ```
///
/// # Errors
///
/// Returns `ParseError::NoCandidates` if no valid JSON could be extracted.
/// Returns `ParseError::DeserializeFailed` if deserialization fails for all candidates.
pub fn parse_with_candidates<T: DeserializeOwned>(input: &str) -> Result<(T, Vec<FlexValue>)> {
    let parser = FlexibleParser::new();
    let candidates = parser.parse(input)?;

    if candidates.is_empty() {
        return Err(ParseError::NoCandidates);
    }

    // Try to deserialize each candidate
    let mut errors = Vec::new();
    let ranked = scoring::rank_candidates(candidates);

    // Clone each candidate individually instead of cloning entire vector upfront.
    // This is more efficient when early candidates succeed (common case).
    for i in 0..ranked.len() {
        let candidate = &ranked[i];
        let mut deserializer = CoercingDeserializer::new(candidate.clone());
        match T::deserialize(&mut deserializer) {
            Ok(value) => {
                return Ok((value, ranked));
            }
            Err(e) => {
                // Collect detailed error information for this candidate
                let source_name = match &candidate.source {
                    value::Source::Direct => "direct".to_string(),
                    value::Source::Markdown { lang } => {
                        format!("markdown({})", lang.as_deref().unwrap_or(""))
                    }
                    value::Source::Fixed { .. } => "fixed".to_string(),
                    value::Source::MultiJson { index } => format!("multi_json[{}]", index),
                    value::Source::MultiJsonArray => "multi_json_array".to_string(),
                    value::Source::Heuristic { pattern } => format!("heuristic({})", pattern),
                    value::Source::Yaml => "yaml".to_string(),
                };
                let preview: String = candidate.value.to_string().chars().take(100).collect();
                let score = scoring::score_candidate(candidate);

                errors.push(error::CandidateError {
                    source: source_name,
                    score,
                    preview,
                    error: e,
                });
            }
        }
    }

    // All candidates failed - return aggregated errors
    Err(ParseError::AllCandidatesFailed(error::AllCandidatesError {
        attempts: errors,
    }))
}

/// Parses an LLM response using a custom parser.
///
/// This allows you to configure the parsing strategies used.
///
/// # Examples
///
/// ```
/// use tryparse::{parse_with_parser, parser::FlexibleParser};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Data {
///     value: i32,
/// }
///
/// let parser = FlexibleParser::new();
/// let response = r#"{"value": 42}"#;
/// let data: Data = parse_with_parser(response, &parser).unwrap();
/// ```
pub fn parse_with_parser<T: DeserializeOwned>(input: &str, parser: &FlexibleParser) -> Result<T> {
    let candidates = parser.parse(input)?;

    if candidates.is_empty() {
        return Err(ParseError::NoCandidates);
    }

    let ranked = scoring::rank_candidates(candidates);

    for candidate in ranked {
        let mut deserializer = CoercingDeserializer::new(candidate);
        if let Ok(value) = T::deserialize(&mut deserializer) {
            return Ok(value);
        }
    }

    Err(ParseError::NoCandidates)
}

// ================================================================================================
// LlmDeserialize API
// ================================================================================================

/// Parses an LLM response using BAML's deserialization algorithms.
///
/// This function uses the custom `LlmDeserialize` trait which provides:
/// - Fuzzy field matching (camelCase ↔ snake_case)
/// - Enum variant fuzzy matching
/// - Union type scoring
/// - Two-mode coercion (strict + lenient)
/// - Circular reference detection
///
/// # Examples
///
/// ```rust
/// use tryparse::parse_llm;
///
/// #[cfg(feature = "derive")]
/// use tryparse::deserializer::LlmDeserialize;
///
/// #[cfg(feature = "derive")]
/// use tryparse_derive::LlmDeserialize;
///
/// #[cfg(feature = "derive")]
/// #[derive(Debug, LlmDeserialize, PartialEq)]
/// struct User {
///     name: String,
///     age: i64,
/// }
///
/// #[cfg(feature = "derive")]
/// {
///     // Type coercion: age as string → i64
///     let response = r#"{"name": "Alice", "age": "30"}"#;
///     let user: User = parse_llm(response).unwrap();
///     assert_eq!(user.name, "Alice");
///     assert_eq!(user.age, 30);
/// }
/// ```
///
/// # Errors
///
/// Returns `ParseError::NoCandidates` if no valid JSON could be extracted.
/// Returns `ParseError::DeserializeFailed` if deserialization fails for all candidates.
pub fn parse_llm<T: LlmDeserialize>(input: &str) -> Result<T> {
    let (result, _candidates) = parse_llm_with_candidates(input)?;
    Ok(result)
}

/// Parses an LLM response using BAML's algorithms and returns all candidates.
///
/// This variant provides access to all parsing candidates with their metadata,
/// showing what transformations were applied by the fuzzy matching system.
///
/// # Examples
///
/// ```rust
/// use tryparse::parse_llm_with_candidates;
///
/// #[cfg(feature = "derive")]
/// use tryparse::deserializer::LlmDeserialize;
///
/// #[cfg(feature = "derive")]
/// use tryparse_derive::LlmDeserialize;
///
/// #[cfg(feature = "derive")]
/// #[derive(LlmDeserialize)]
/// struct Data {
///     value: i64,
/// }
///
/// #[cfg(feature = "derive")]
/// {
///     let response = r#"{"value": "42"}"#;
///     let (data, candidates) = parse_llm_with_candidates::<Data>(response).unwrap();
///     assert_eq!(data.value, 42);
/// }
/// ```
///
/// # Errors
///
/// Returns `ParseError::NoCandidates` if no valid JSON could be extracted.
/// Returns `ParseError::DeserializeFailed` if deserialization fails for all candidates.
pub fn parse_llm_with_candidates<T: LlmDeserialize>(input: &str) -> Result<(T, Vec<FlexValue>)> {
    let parser = FlexibleParser::new();
    let candidates = parser.parse(input)?;

    if candidates.is_empty() {
        return Err(ParseError::NoCandidates);
    }

    // Rank candidates by quality
    let ranked = scoring::rank_candidates(candidates);

    // BAML TWO-MODE COERCION:
    // 1. First pass: Try strict deserialization (try_deserialize) on all candidates
    //    This allows array candidates to win for Vec<T> before single-value wrapping
    // 2. Second pass: Try lenient deserialization (deserialize) on all candidates
    //    This applies coercions like single-value wrapping for Vec<T>

    // First pass: Strict mode (try_deserialize)
    for (idx, candidate) in ranked.iter().enumerate() {
        let mut ctx = CoercionContext::new();
        if let Some(value) = T::try_deserialize(candidate, &mut ctx) {
            // Merge transformations from deserialization into the winning candidate
            let mut updated_ranked = ranked.clone();
            for transformation in ctx.transformations() {
                updated_ranked[idx].add_transformation(transformation.clone());
            }
            return Ok((value, updated_ranked));
        }
    }

    // Second pass: Lenient mode (deserialize)
    for (idx, candidate) in ranked.iter().enumerate() {
        let mut ctx = CoercionContext::new();
        match T::deserialize(candidate, &mut ctx) {
            Ok(value) => {
                // Merge transformations from deserialization into the winning candidate
                let mut updated_ranked = ranked.clone();
                for transformation in ctx.transformations() {
                    updated_ranked[idx].add_transformation(transformation.clone());
                }
                return Ok((value, updated_ranked));
            }
            Err(_) => {
                // Continue to next candidate
                continue;
            }
        }
    }

    // All candidates failed
    Err(ParseError::NoCandidates)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize, Debug, PartialEq)]
    struct User {
        name: String,
        age: u32,
    }

    #[test]
    fn test_parse_clean_json() {
        let input = r#"{"name": "Alice", "age": 30}"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.name, "Alice");
        assert_eq!(user.age, 30);
    }

    #[test]
    fn test_parse_with_type_coercion() {
        let input = r#"{"name": "Bob", "age": "25"}"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.age, 25);
    }

    #[test]
    fn test_parse_markdown() {
        let input = r#"
Here's the user:
```json
{"name": "Charlie", "age": 35}
```
"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.name, "Charlie");
    }

    #[test]
    fn test_parse_with_trailing_comma() {
        let input = r#"{"name": "Dave", "age": 40,}"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.name, "Dave");
    }

    #[test]
    fn test_parse_with_unquoted_keys() {
        let input = r#"{name: "Eve", age: 45}"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.name, "Eve");
    }

    #[test]
    fn test_parse_with_single_quotes() {
        let input = r#"{'name': 'Frank', 'age': 50}"#;
        let user: User = parse(input).unwrap();
        assert_eq!(user.name, "Frank");
    }

    #[test]
    fn test_parse_with_candidates() {
        let input = r#"{"name": "Grace", "age": "55"}"#;
        let (user, candidates): (User, _) = parse_with_candidates(input).unwrap();
        assert_eq!(user.name, "Grace");
        assert!(!candidates.is_empty());
    }

    #[test]
    fn test_parse_invalid_input() {
        let input = "This is not JSON at all";
        let result: Result<User> = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_array() {
        let input = r#"[{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}]"#;
        let users: Vec<User> = parse(input).unwrap();
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn test_parse_nested_struct() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Address {
            city: String,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct Person {
            name: String,
            address: Address,
        }

        let input = r#"{"name": "Alice", "address": {"city": "NYC"}}"#;
        let person: Person = parse(input).unwrap();
        assert_eq!(person.address.city, "NYC");
    }

    #[test]
    fn test_parse_with_custom_parser() {
        let parser = FlexibleParser::new();
        let input = r#"{"name": "Alice", "age": 30}"#;
        let user: User = parse_with_parser(input, &parser).unwrap();
        assert_eq!(user.name, "Alice");
    }
}
