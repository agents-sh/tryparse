//! Tests for implied key inference (single-field struct coercion).
//!
//! BAML algorithm: When deserializing a single-field struct and the value
//! isn't an object, try to coerce the entire value into that single field.

use serde::Deserialize;
use tryparse::parse;
#[cfg(feature = "derive")]
use tryparse::parse_llm;

#[derive(Deserialize, Debug, PartialEq)]
struct Wrapper {
    data: String,
}

#[derive(Deserialize, Debug, PartialEq)]
struct NumberWrapper {
    value: i64,
}

#[test]
#[ignore] // Implied key only works with LlmDeserialize, not serde::Deserialize
fn test_single_field_string_implied_key() {
    // Direct string should wrap into single field
    // NOTE: This requires LlmDeserialize trait, not regular serde::Deserialize
    let response = r#""hello world""#;
    let result: Result<Wrapper, _> = parse(response);
    assert!(result.is_ok(), "Should coerce string into single field");
    let wrapper = result.unwrap();
    assert_eq!(wrapper.data, "hello world");
}

#[test]
#[ignore] // Implied key only works with LlmDeserialize, not serde::Deserialize
fn test_single_field_number_implied_key() {
    // Direct number should wrap into single field
    // NOTE: This requires LlmDeserialize trait, not regular serde::Deserialize
    let response = "42";
    let result: Result<NumberWrapper, _> = parse(response);
    assert!(result.is_ok(), "Should coerce number into single field");
    let wrapper = result.unwrap();
    assert_eq!(wrapper.value, 42);
}

#[test]
fn test_single_field_still_accepts_object() {
    // Single-field structs should still accept proper objects
    let response = r#"{"data": "hello"}"#;
    let result: Result<Wrapper, _> = parse(response);
    assert!(result.is_ok());
    let wrapper = result.unwrap();
    assert_eq!(wrapper.data, "hello");
}

#[test]
#[ignore] // Implied key only works with LlmDeserialize, not serde::Deserialize
fn test_single_field_array_implied_key() {
    #[derive(Deserialize, Debug)]
    struct ArrayWrapper {
        items: Vec<String>,
    }

    // Array should wrap into single field
    // NOTE: This requires LlmDeserialize trait, not regular serde::Deserialize
    let response = r#"["a", "b", "c"]"#;
    let result: Result<ArrayWrapper, _> = parse(response);
    assert!(result.is_ok(), "Should coerce array into single field");
    let wrapper = result.unwrap();
    assert_eq!(wrapper.items, vec!["a", "b", "c"]);
}

#[test]
#[cfg(feature = "derive")]
fn test_implied_key_with_llm_deserialize() {
    use tryparse_derive::LlmDeserialize;

    #[derive(Debug, serde::Deserialize, LlmDeserialize, PartialEq)]
    struct Wrapper {
        data: String,
    }

    // Implied key inference WORKS - string is wrapped into single field
    let response = r#""hello world""#;
    let result: Wrapper = parse_llm(response).unwrap();
    assert_eq!(result.data, "hello world");
    assert_eq!(
        result,
        Wrapper {
            data: "hello world".to_string()
        }
    );

    // Also works with numbers
    #[derive(Debug, serde::Deserialize, LlmDeserialize)]
    struct NumWrapper {
        value: i64,
    }

    let response2 = "42";
    let result2: NumWrapper = parse_llm(response2).unwrap();
    assert_eq!(result2.value, 42);

    // Note: Transformation tracking for deserialization is a separate enhancement task
    // The feature works, but transformations from StructDeserializer aren't yet
    // propagated back to FlexValue candidates
}

#[test]
fn test_multi_field_struct_rejects_non_object() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)] // Fields are deserialized but not directly accessed in test
    struct TwoFields {
        name: String,
        age: i64,
    }

    // Multi-field structs should NOT accept primitives
    let response = r#""hello""#;
    let result: Result<TwoFields, _> = parse(response);
    assert!(
        result.is_err(),
        "Multi-field struct should reject primitive"
    );
}

#[test]
#[ignore] // Implied key only works with LlmDeserialize, not serde::Deserialize
fn test_single_field_complex_value() {
    use serde_json::Value;

    #[derive(Deserialize, Debug)]
    struct JsonWrapper {
        content: Value,
    }

    // Complex object should wrap into single Value field
    // NOTE: This requires LlmDeserialize trait, not regular serde::Deserialize
    let response = r#"{"foo": "bar", "nested": {"num": 42}}"#;
    let result: Result<JsonWrapper, _> = parse(response);
    assert!(
        result.is_ok(),
        "Should wrap complex object into Value field"
    );
    let wrapper = result.unwrap();

    let obj = wrapper.content.as_object().unwrap();
    assert_eq!(obj.get("foo").unwrap(), "bar");
    assert_eq!(obj.get("nested").unwrap().get("num").unwrap(), 42);
}
