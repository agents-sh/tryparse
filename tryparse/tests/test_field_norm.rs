//! Field fuzzy matching test
//!
//! Tests that field matching works across different naming conventions (camelCase → snake_case).
//! This requires the LlmDeserialize trait with the `derive` feature.

#[cfg(feature = "derive")]
use tryparse::parse_llm;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

#[cfg(feature = "derive")]
#[derive(Debug, serde::Deserialize, LlmDeserialize)]
struct Config {
    user_name: String,
    max_count: i64,
}

#[cfg(feature = "derive")]
#[test]
fn test_field_normalization_debug() {
    let response = r#"{"userName": "Alice", "maxCount": 30}"#;

    println!("Input: {}", response);

    let result: Result<Config, _> = parse_llm(response);
    match &result {
        Ok(config) => {
            println!("SUCCESS: {:?}", config);
            assert_eq!(config.user_name, "Alice");
            assert_eq!(config.max_count, 30);
        }
        Err(e) => {
            println!("FAILED: {:?}", e);
            panic!("Field fuzzy matching should work!");
        }
    }
}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn test_field_normalization_debug() {}
