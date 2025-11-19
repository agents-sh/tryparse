//! Case-insensitive field matching tests
//!
//! These tests verify that field matching works across different naming conventions.
//! Field matching is handled by the struct deserializer's FieldMatcher,
//! which requires the LlmDeserialize trait (available with the `derive` feature).

#[cfg(feature = "derive")]
use tryparse::parse_llm;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

#[cfg(feature = "derive")]
#[derive(Debug, serde::Deserialize, LlmDeserialize)]
struct User {
    name: String,
    age: i64,
}

#[cfg(feature = "derive")]
#[test]
fn test_uppercase_fields() {
    let response = r#"{"NAME": "Alice", "AGE": 30}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should handle UPPERCASE fields via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[cfg(feature = "derive")]
#[test]
fn test_pascalcase_fields() {
    let response = r#"{"Name": "Bob", "Age": 25}"#;
    let result: Result<User, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should handle PascalCase fields via fuzzy matching"
    );
    let user = result.unwrap();
    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 25);
}

#[cfg(feature = "derive")]
#[test]
fn test_kebab_case_fields() {
    #[derive(Debug, serde::Deserialize, LlmDeserialize)]
    struct Config {
        user_name: String,
        max_count: i64,
    }

    let response = r#"{"user-name": "Charlie", "max-count": 100}"#;
    let result: Result<Config, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should handle kebab-case fields via fuzzy matching"
    );
    let config = result.unwrap();
    assert_eq!(config.user_name, "Charlie");
    assert_eq!(config.max_count, 100);
}

#[cfg(feature = "derive")]
#[test]
fn test_dot_notation_fields() {
    #[derive(Debug, serde::Deserialize, LlmDeserialize)]
    struct Config {
        user_name: String,
        max_count: i64,
    }

    let response = r#"{"user.name": "Dave", "max.count": 50}"#;
    let result: Result<Config, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Should handle dot.notation fields via fuzzy matching"
    );
    let config = result.unwrap();
    assert_eq!(config.user_name, "Dave");
    assert_eq!(config.max_count, 50);
}

// When derive feature is not enabled, these tests are skipped
#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn test_uppercase_fields() {}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn test_pascalcase_fields() {}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn test_kebab_case_fields() {}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn test_dot_notation_fields() {}
