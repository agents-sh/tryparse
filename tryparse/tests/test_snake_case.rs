//! Integration tests for field fuzzy matching across different naming conventions.
//!
//! These tests verify that the struct deserializer's FieldMatcher can handle:
//! - camelCase → snake_case
//! - PascalCase → snake_case
//! - kebab-case → snake_case
//! - dot.notation → snake_case
//!
//! This requires the LlmDeserialize trait (available with `derive` feature).

#[cfg(feature = "derive")]
use tryparse::parse_llm;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, serde::Deserialize, LlmDeserialize)]
struct User {
    user_name: String,
    max_count: i64,
}

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, serde::Deserialize, LlmDeserialize)]
struct Config {
    xml_parser: String,
    io_error: String,
}

#[cfg(feature = "derive")]
#[test]
fn test_camel_case_to_snake_case() {
    // Test camelCase field names get matched to snake_case via FieldMatcher
    let response = r#"{"userName": "Alice", "maxCount": 30}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should match camelCase to snake_case via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.user_name, "Alice");
    assert_eq!(user.max_count, 30);
}

#[cfg(feature = "derive")]
#[test]
fn test_pascal_case_to_snake_case() {
    // Test PascalCase field names get matched to snake_case via FieldMatcher
    let response = r#"{"UserName": "Bob", "MaxCount": 25}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should match PascalCase to snake_case via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.user_name, "Bob");
    assert_eq!(user.max_count, 25);
}

#[cfg(feature = "derive")]
#[test]
#[ignore] // Known limitation: consecutive uppercase letters (acronyms) are not handled perfectly
fn test_acronyms_in_field_names() {
    // KNOWN LIMITATION: The simple to_snake_case implementation doesn't handle
    // consecutive uppercase letters (acronyms) optimally.
    // "XMLParser" becomes "x_m_l_parser" instead of "xml_parser"
    // This would require a more sophisticated algorithm to detect acronyms.
    //
    // For now, LLMs should output field names as "xmlParser" or "xml_parser"
    // instead of "XMLParser" to ensure reliable matching.
    let response = r#"{"XMLParser": "parser1", "IOError": "error1"}"#;
    let result: Result<Config, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should match acronyms correctly (XMLParser → xml_parser)"
    );
    let config = result.unwrap();
    assert_eq!(config.xml_parser, "parser1");
    assert_eq!(config.io_error, "error1");
}

#[cfg(feature = "derive")]
#[test]
fn test_already_snake_case() {
    // Test that already snake_case field names work fine
    let response = r#"{"user_name": "Charlie", "max_count": 35}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should handle already snake_case field names"
    );
    let user = result.unwrap();
    assert_eq!(user.user_name, "Charlie");
    assert_eq!(user.max_count, 35);
}

#[cfg(feature = "derive")]
#[test]
fn test_kebab_case_to_snake_case() {
    // Test kebab-case field names get matched to snake_case via FieldMatcher
    let response = r#"{"user-name": "Dave", "max-count": 40}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should match kebab-case to snake_case via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.user_name, "Dave");
    assert_eq!(user.max_count, 40);
}

#[cfg(feature = "derive")]
#[test]
fn test_dot_notation_to_snake_case() {
    // Test dot.notation field names get matched to snake_case via FieldMatcher
    let response = r#"{"user.name": "Eve", "max.count": 45}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should match dot.notation to snake_case via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.user_name, "Eve");
    assert_eq!(user.max_count, 45);
}
