//! Port of BAML's test_class.rs test cases
//!
//! Tests struct deserialization with fuzzy field matching, optional fields,
//! nested structures, and more.
//!
//! Source: engine/baml-lib/jsonish/src/tests/test_class.rs

#[cfg(feature = "derive")]
use tryparse::parse_llm;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

// ================================================================================================
// Basic Struct Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct Foo {
    hi: Vec<String>,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct Bar {
    foo: String,
}

#[cfg(feature = "derive")]
#[test]
fn test_basic_struct_with_array() {
    let result: Result<Foo, _> = parse_llm(r#"{"hi": ["a", "b"]}"#);
    let foo = result.unwrap();
    assert_eq!(foo.hi, vec!["a", "b"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_wrapped_objects() {
    // Single object, expecting array → wraps in array
    let result: Result<Vec<Foo>, _> = parse_llm(r#"{"hi": "a"}"#);
    let foos = result.unwrap();
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].hi, vec!["a"]); // String "a" → ["a"]
}

#[cfg(feature = "derive")]
#[test]
fn test_extract_from_prose() {
    // Extract object from prose
    let result: Result<Foo, _> = parse_llm(r#"The output is: {"hi": ["a", "b"]}"#);
    let foo = result.unwrap();
    assert_eq!(foo.hi, vec!["a", "b"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_extract_with_extra_text() {
    let result: Result<Foo, _> = parse_llm(r#"This is a test. The output is: {"hi": ["a", "b"]}"#);
    let foo = result.unwrap();
    assert_eq!(foo.hi, vec!["a", "b"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_extract_with_suffix_text() {
    let result: Result<Foo, _> = parse_llm(r#"{"hi": ["a", "b"]} is the output."#);
    let foo = result.unwrap();
    assert_eq!(foo.hi, vec!["a", "b"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_string_with_escaped_quotes() {
    // String field containing escaped JSON
    let result: Result<Bar, _> = parse_llm(r#"{"foo": "[\"bar\"]"}"#);
    let bar = result.unwrap();
    assert_eq!(bar.foo, r#"["bar"]"#); // Should be treated as string, not parsed
}

#[cfg(feature = "derive")]
#[test]
fn test_string_with_nested_json() {
    // String field containing nested JSON
    let result: Result<Bar, _> = parse_llm(r#"{"foo": "{\"foo\": [\"bar\"]}"}"#);
    let bar = result.unwrap();
    assert_eq!(bar.foo, r#"{"foo": ["bar"]}"#);
}

#[cfg(feature = "derive")]
#[test]
fn test_string_with_markdown_in_value() {
    // String field value contains markdown with JSON
    let input = r#"
{
  "foo": "Here is how you can build the API call:\n```json\n{\n  \"foo\": {\n    \"world\": [\n      \"bar\"\n    ]\n  }\n}\n```"
}
"#;
    let result: Result<Bar, _> = parse_llm(input);
    let bar = result.unwrap();
    assert!(bar.foo.contains("Here is how you can build"));
    assert!(bar.foo.contains("```json"));
}

// ================================================================================================
// Optional Fields
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct OptionalFoo {
    foo: Option<String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_optional_field_missing() {
    // Missing optional field → None
    let result: Result<OptionalFoo, _> = parse_llm(r#"{}"#);
    let obj = result.unwrap();
    assert_eq!(obj.foo, None);
}

#[cfg(feature = "derive")]
#[test]
fn test_optional_field_empty_string() {
    // Empty string is still Some("")
    let result: Result<OptionalFoo, _> = parse_llm(r#"{"foo": ""}"#);
    let obj = result.unwrap();
    assert_eq!(obj.foo, Some("".to_string()));
}

// ================================================================================================
// Multi-Field Structs
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct MultiFieldFoo {
    one: String,
    two: Option<String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_multi_field_required_only() {
    let result: Result<MultiFieldFoo, _> = parse_llm(r#"{"one": "a"}"#);
    let obj = result.unwrap();
    assert_eq!(obj.one, "a");
    assert_eq!(obj.two, None);
}

#[cfg(feature = "derive")]
#[test]
fn test_multi_field_with_optional() {
    let result: Result<MultiFieldFoo, _> = parse_llm(r#"{"one": "a", "two": "b"}"#);
    let obj = result.unwrap();
    assert_eq!(obj.one, "a");
    assert_eq!(obj.two, Some("b".to_string()));
}

#[cfg(feature = "derive")]
#[test]
fn test_multi_field_in_markdown() {
    let input = r#"Here is how you can build the API call:
    ```json
    {
        "one": "hi",
        "two": "hello"
    }
    ```

    ```json
        {
            "test2": {
                "key2": "value"
            },
            "test21": [
            ]
        }
    ```"#;

    let result: Result<MultiFieldFoo, _> = parse_llm(input);
    let obj = result.unwrap();
    assert_eq!(obj.one, "hi");
    assert_eq!(obj.two, Some("hello".to_string()));
}

// ================================================================================================
// Structs with Arrays
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct MultiFieldWithList {
    a: i64,
    b: String,
    c: Vec<String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_struct_with_mixed_types() {
    let result: Result<MultiFieldWithList, _> =
        parse_llm(r#"{"a": 1, "b": "hi", "c": ["a", "b"]}"#);
    let obj = result.unwrap();
    assert_eq!(obj.a, 1);
    assert_eq!(obj.b, "hi");
    assert_eq!(obj.c, vec!["a", "b"]);
}

// ================================================================================================
// Nested Structs
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct InnerFoo {
    a: String,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct OuterBar {
    foo: InnerFoo,
}

#[cfg(feature = "derive")]
#[test]
fn test_nested_struct() {
    let result: Result<OuterBar, _> = parse_llm(r#"{"foo": {"a": "hi"}}"#);
    let obj = result.unwrap();
    assert_eq!(obj.foo.a, "hi");
}

#[cfg(feature = "derive")]
#[test]
fn test_nested_struct_in_markdown() {
    let input = r#"Here is how you can build the API call:
    ```json
    {
        "foo": {
            "a": "hi"
        }
    }
    ```

    and this is extra text"#;

    let result: Result<OuterBar, _> = parse_llm(input);
    let obj = result.unwrap();
    assert_eq!(obj.foo.a, "hi");
}

// ================================================================================================
// Fuzzy Field Matching Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct SnakeCaseStruct {
    field_name: String,
    another_field: i64,
}

#[cfg(feature = "derive")]
#[test]
fn test_camel_case_to_snake_case() {
    // camelCase in JSON → snake_case in struct
    let result: Result<SnakeCaseStruct, _> =
        parse_llm(r#"{"fieldName": "test", "anotherField": 42}"#);
    let obj = result.unwrap();
    assert_eq!(obj.field_name, "test");
    assert_eq!(obj.another_field, 42);
}

#[cfg(feature = "derive")]
#[test]
fn test_pascal_case_to_snake_case() {
    // PascalCase in JSON → snake_case in struct
    let result: Result<SnakeCaseStruct, _> =
        parse_llm(r#"{"FieldName": "test", "AnotherField": 42}"#);
    let obj = result.unwrap();
    assert_eq!(obj.field_name, "test");
    assert_eq!(obj.another_field, 42);
}

// ================================================================================================
// Type Coercion Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct TypeCoercionStruct {
    int_field: i64,
    float_field: f64,
    bool_field: bool,
    string_field: String,
}

#[cfg(feature = "derive")]
#[test]
#[allow(clippy::approx_constant)]
fn test_string_to_number_coercion() {
    // Strings → numbers
    let result: Result<TypeCoercionStruct, _> = parse_llm(
        r#"{
        "int_field": "42",
        "float_field": "3.14",
        "bool_field": true,
        "string_field": "hello"
    }"#,
    );
    let obj = result.unwrap();
    assert_eq!(obj.int_field, 42);
    assert_eq!(obj.float_field, 3.14);
    assert!(obj.bool_field);
    assert_eq!(obj.string_field, "hello");
}

#[cfg(feature = "derive")]
#[test]
#[allow(clippy::approx_constant)]
fn test_number_to_string_coercion() {
    // Numbers → strings
    let result: Result<TypeCoercionStruct, _> = parse_llm(
        r#"{
        "int_field": 42,
        "float_field": 3.14,
        "bool_field": "true",
        "string_field": 123
    }"#,
    );
    let obj = result.unwrap();
    assert_eq!(obj.int_field, 42);
    assert_eq!(obj.float_field, 3.14);
    assert!(obj.bool_field);
    assert_eq!(obj.string_field, "123");
}

// ================================================================================================
// Extra Fields Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct StrictStruct {
    expected_field: String,
}

#[cfg(feature = "derive")]
#[test]
fn test_extra_fields_ignored() {
    // Extra fields in JSON should be ignored
    let result: Result<StrictStruct, _> = parse_llm(
        r#"{
        "expected_field": "value",
        "extra_field_1": "ignored",
        "extra_field_2": 123
    }"#,
    );
    let obj = result.unwrap();
    assert_eq!(obj.expected_field, "value");
}

// ================================================================================================
// Edge Cases
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_empty_struct() {
    #[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
    struct EmptyStruct {}

    let result: Result<EmptyStruct, _> = parse_llm(r#"{}"#);
    assert!(result.is_ok());
}

#[cfg(feature = "derive")]
#[test]
fn test_single_field_struct() {
    #[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
    struct SingleField {
        value: i64,
    }

    let result: Result<SingleField, _> = parse_llm(r#"{"value": 42}"#);
    let obj = result.unwrap();
    assert_eq!(obj.value, 42);
}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn placeholder_when_derive_disabled() {}
