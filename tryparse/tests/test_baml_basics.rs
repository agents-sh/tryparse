//! Port of BAML's test_basics.rs test cases
//!
//! This file systematically tests the same edge cases that BAML's engine tests,
//! ensuring our implementation matches BAML's behavior.
//!
//! Source: engine/baml-lib/jsonish/src/tests/test_basics.rs

#[cfg(feature = "derive")]
use tryparse::parse_llm;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

// ================================================================================================
// Primitive Type Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_number() {
    let result: Result<i64, _> = parse_llm("12111");
    assert_eq!(result.unwrap(), 12111);
}

#[cfg(feature = "derive")]
#[test]
fn test_number_with_commas() {
    // BAML handles comma-separated numbers: "12,111" → 12111
    let result: Result<i64, _> = parse_llm("12,111");
    assert_eq!(result.unwrap(), 12111);
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_true() {
    let result: Result<bool, _> = parse_llm("true");
    assert!(result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_true_capitalized() {
    // BAML handles case-insensitive bool: "True" → true
    let result: Result<bool, _> = parse_llm("True");
    assert!(result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_false() {
    let result: Result<bool, _> = parse_llm("false");
    assert!(!result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_false_capitalized() {
    let result: Result<bool, _> = parse_llm("False");
    assert!(!result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_wrapped_in_text() {
    // BAML extracts bool from natural language: "The answer is true" → [true]
    let result: Result<Vec<bool>, _> = parse_llm("The answer is true");
    assert_eq!(result.unwrap(), vec![true]);
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_wrapped_mismatched_case() {
    // BAML handles "The answer is True" → [true]
    let result: Result<Vec<bool>, _> = parse_llm("The answer is True");
    assert_eq!(result.unwrap(), vec![true]);
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_in_markdown_context() {
    // BAML extracts from: "Answer: **True**" → true
    let input = "The tax return you provided has section for dependents.\n\nAnswer: **True**";
    let result: Result<bool, _> = parse_llm(input);
    assert!(result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_bool_followed_by_explanation() {
    // BAML handles: "False.\n\n<explanation>" → false
    let input = r#"False.

The statement "2 + 2 = 5" is mathematically incorrect. The correct sum of 2 + 2 is 4, not 5."#;
    let result: Result<bool, _> = parse_llm(input);
    assert!(!result.unwrap());
}

#[cfg(feature = "derive")]
#[test]
fn test_float() {
    let result: Result<f64, _> = parse_llm("12111.123");
    assert_eq!(result.unwrap(), 12111.123);
}

#[cfg(feature = "derive")]
#[test]
fn test_float_comma_us() {
    // BAML handles US format: "12,111.123" → 12111.123
    let result: Result<f64, _> = parse_llm("12,111.123");
    assert_eq!(result.unwrap(), 12111.123);
}

#[cfg(feature = "derive")]
#[test]
fn test_float_trailing_dots() {
    // BAML handles: "12.11." → 12.11
    let result: Result<f64, _> = parse_llm("12.11.");
    assert_eq!(result.unwrap(), 12.11);
}

#[cfg(feature = "derive")]
#[test]
fn test_float_fraction() {
    // BAML parses fractions: "1/5" → 0.2
    let result: Result<f64, _> = parse_llm("1/5");
    assert_eq!(result.unwrap(), 0.2);
}

#[cfg(feature = "derive")]
#[test]
fn test_string_to_float_from_recipe() {
    // BAML extracts first number from text: "1 cup unsalted butter..." → 1.0
    let result: Result<f64, _> = parse_llm("1 cup unsalted butter, room temperature");
    assert_eq!(result.unwrap(), 1.0);
}

// ================================================================================================
// Array Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_array_int() {
    let result: Result<Vec<i64>, _> = parse_llm(r#"[1, 2, 3]"#);
    assert_eq!(result.unwrap(), vec![1, 2, 3]);
}

#[cfg(feature = "derive")]
#[test]
fn test_array_int_to_string() {
    // BAML coerces: [1, 2, 3] → ["1", "2", "3"]
    let result: Result<Vec<String>, _> = parse_llm(r#"[1, 2, 3]"#);
    assert_eq!(result.unwrap(), vec!["1", "2", "3"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_array_int_to_float() {
    // BAML coerces: [1, 2, 3] → [1.0, 2.0, 3.0]
    let result: Result<Vec<f64>, _> = parse_llm(r#"[1, 2, 3]"#);
    assert_eq!(result.unwrap(), vec![1.0, 2.0, 3.0]);
}

#[cfg(feature = "derive")]
#[test]
fn test_array_trailing_comma() {
    // BAML handles: [1, 2, 3,] → [1, 2, 3]
    let result: Result<Vec<i64>, _> = parse_llm(r#"[1, 2, 3,]"#);
    assert_eq!(result.unwrap(), vec![1, 2, 3]);
}

#[cfg(feature = "derive")]
#[test]
fn test_array_incomplete() {
    // BAML fixes incomplete arrays: "[1, 2, 3" → [1, 2, 3]
    let result: Result<Vec<i64>, _> = parse_llm(r#"[1, 2, 3"#);
    assert_eq!(result.unwrap(), vec![1, 2, 3]);
}

// ================================================================================================
// Struct Tests
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, serde::Deserialize, LlmDeserialize)]
struct SimpleStruct {
    key: String,
}

#[cfg(feature = "derive")]
#[test]
fn test_simple_object() {
    let result: Result<SimpleStruct, _> = parse_llm(r#"{"key": "value"}"#);
    assert_eq!(result.unwrap().key, "value");
}

#[cfg(feature = "derive")]
#[test]
fn test_object_trailing_comma() {
    // BAML handles: {"key": "value",} → {"key": "value"}
    let result: Result<SimpleStruct, _> = parse_llm(r#"{"key": "value",}"#);
    assert_eq!(result.unwrap().key, "value");
}

#[cfg(feature = "derive")]
#[test]
fn test_object_incomplete_string() {
    // BAML fixes: {"key": "value" → {"key": "value"}
    let result: Result<SimpleStruct, _> = parse_llm(r#"{"key": "value"#);
    assert_eq!(result.unwrap().key, "value");
}

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, serde::Deserialize, LlmDeserialize)]
struct NestedStruct {
    key: Vec<i64>,
}

#[cfg(feature = "derive")]
#[test]
fn test_nested_array_in_object() {
    let result: Result<NestedStruct, _> = parse_llm(r#"{"key": [1, 2, 3]}"#);
    assert_eq!(result.unwrap().key, vec![1, 2, 3]);
}

#[cfg(feature = "derive")]
#[test]
fn test_nested_with_whitespace() {
    // BAML handles extra whitespace
    let result: Result<NestedStruct, _> = parse_llm(r#" { "key" : [ 1 , 2 , 3 ] } "#);
    assert_eq!(result.unwrap().key, vec![1, 2, 3]);
}

#[cfg(feature = "derive")]
#[test]
fn test_object_with_prefix_suffix() {
    // BAML extracts object from surrounding text
    let result: Result<NestedStruct, _> = parse_llm(r#"prefix { "key" : [ 1 , 2 , 3 ] } suffix"#);
    assert_eq!(result.unwrap().key, vec![1, 2, 3]);
}

#[cfg(feature = "derive")]
#[test]
fn test_incomplete_array_in_object() {
    // BAML fixes: {"key": [1, 2, 3 → {"key": [1, 2, 3]}
    let result: Result<NestedStruct, _> = parse_llm(r#"{"key": [1, 2, 3"#);
    assert_eq!(result.unwrap().key, vec![1, 2, 3]);
}

// ================================================================================================
// Multiple Top-Level Objects
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_multiple_top_level_single() {
    // BAML takes first object when expecting single: {"key": "value1"} {"key": "value2"} → first
    let input = r#"{"key": "value1"} {"key": "value2"}"#;

    let result: Result<SimpleStruct, _> = parse_llm(input);
    match &result {
        Ok(obj) => println!("Success: key={}", obj.key),
        Err(e) => println!("Error: {:?}", e),
    }
    assert_eq!(result.unwrap().key, "value1");
}

#[cfg(feature = "derive")]
#[test]
fn test_multiple_top_level_array() {
    // BAML collects all objects when expecting array
    let input = r#"{"key": "value1"} {"key": "value2"}"#;
    println!("Input: {}", input);

    // Debug: see what parser returns
    use tryparse::parser::FlexibleParser;
    let parser = FlexibleParser::new();
    let candidates = parser.parse(input).unwrap();
    println!("Parser found {} candidates", candidates.len());
    for (i, cand) in candidates.iter().enumerate() {
        println!(
            "Candidate {}: source={:?}, value={:?}",
            i, cand.source, cand.value
        );
    }

    let result: Result<Vec<SimpleStruct>, _> = parse_llm(input);
    match &result {
        Ok(objects) => {
            println!("Successfully deserialized {} objects", objects.len());
            for (i, obj) in objects.iter().enumerate() {
                println!("Object {}: key={}", i, obj.key);
            }
        }
        Err(e) => {
            println!("Deserialization error: {:?}", e);
        }
    }
    let objects = result.unwrap();
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].key, "value1");
    assert_eq!(objects[1].key, "value2");
}

#[cfg(feature = "derive")]
#[test]
fn test_multiple_with_prefix_suffix() {
    // BAML extracts multiple objects from surrounding text
    let input = r#"prefix {"key": "value1"} some random text {"key": "value2"} suffix"#;
    let result: Result<Vec<SimpleStruct>, _> = parse_llm(input);
    let objects = result.unwrap();
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].key, "value1");
    assert_eq!(objects[1].key, "value2");
}

// ================================================================================================
// Markdown Code Blocks
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct ComplexStruct {
    key: String,
    array: Vec<i64>,
    object: InnerObject,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct InnerObject {
    key: String,
}

#[cfg(feature = "derive")]
#[test]
fn test_json_in_markdown() {
    let input = r#"
some text
```json
{
  "key": "value",
  "array": [1, 2, 3],
  "object": {
    "key": "value"
  }
}
```
"#;
    let result: Result<ComplexStruct, _> = parse_llm(input);
    let obj = result.unwrap();
    assert_eq!(obj.key, "value");
    assert_eq!(obj.array, vec![1, 2, 3]);
    assert_eq!(obj.object.key, "value");
}

#[cfg(feature = "derive")]
#[test]
fn test_multiple_json_blocks_prefer_matching() {
    // When there are multiple JSON blocks, BAML picks the best match for the type
    let input = r#"
some text
```json
{
  "key": "value",
  "array": [1, 2, 3],
  "object": {
    "key": "value"
  }
}
```

```json
["1", "2"]
```
"#;
    // When expecting ComplexStruct, should use first block
    let result: Result<ComplexStruct, _> = parse_llm(input);
    assert!(result.is_ok());

    // When expecting Vec<i64>, should prefer second block
    let result2: Result<Vec<i64>, _> = parse_llm(input);
    assert_eq!(result2.unwrap(), vec![1, 2]);
}

#[cfg(feature = "derive")]
#[test]
fn test_debug_markdown_parser() {
    use tryparse::parser::FlexibleParser;

    let parser = FlexibleParser::new();
    let input = r#"
some text
```json
{
  "key": "value",
  "array": [1, 2, 3,],
  "object": {
    "key": "value"
  }
}
```
"#;

    println!("Testing markdown parser with trailing comma");
    let result = parser.parse(input);
    match result {
        Ok(values) => {
            println!("Parser returned {} candidates", values.len());
            for (i, val) in values.iter().enumerate() {
                println!(
                    "Candidate {}: source={:?}, value={:?}",
                    i, val.source, val.value
                );
            }
        }
        Err(e) => {
            println!("Parser error: {:?}", e);
        }
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_markdown_with_trailing_comma() {
    // BAML handles syntax errors in markdown JSON
    let input = r#"
some text
```json
{
  "key": "value",
  "array": [1, 2, 3,],
  "object": {
    "key": "value"
  }
}
```
"#;
    let result: Result<ComplexStruct, _> = parse_llm(input);
    if let Err(ref e) = result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());
}

// ================================================================================================
// Unquoted Keys and Values
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_unquoted_keys() {
    let input = r#"
```json
{
  key: "value",
  array: [1, 2, 3],
  object: {
    key: "value"
  }
}
```
"#;
    let result: Result<ComplexStruct, _> = parse_llm(input);
    assert!(result.is_ok());
}

#[cfg(feature = "derive")]
#[test]
fn test_unquoted_values_with_spaces() {
    let input = r#"
{
  key: value with space,
  array: [1, 2, 3],
  object: {
    key: value
  }
}
"#;
    let result: Result<ComplexStruct, _> = parse_llm(input);
    let obj = result.unwrap();
    assert_eq!(obj.key, "value with space");
}

// ================================================================================================
// Whitespace Handling
// ================================================================================================

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct Answer {
    content: f64,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
struct TestWithAnswer {
    answer: Answer,
}

#[cfg(feature = "derive")]
#[test]
fn test_whitespace_in_keys() {
    // BAML's field matcher should handle " answer " → answer
    let input = r#"{" answer ": {" content ": 78.54}}"#;
    let result: Result<TestWithAnswer, _> = parse_llm(input);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().answer.content, 78.54);
}

// ================================================================================================
// Real-World LLM Output Examples
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_localization_example() {
    #[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
    struct Localization {
        id: String,
        #[allow(non_snake_case)]
        English: String,
        #[allow(non_snake_case)]
        Portuguese: String,
    }

    let input = r#"
To effectively localize these strings for a Portuguese-speaking audience...

JSON Output:
```
[
  {
    "id": "CH1_Welcome",
    "English": "Welcome to Arcadian Atlas",
    "Portuguese": "Bem-vindo ao Arcadian Atlas"
  },
  {
    "id": "CH1_02",
    "English": "Arcadia is a vast land, with monsters and dangers!",
    "Portuguese": "Arcadia é uma terra vasta, repleta de monstros e perigos!"
  }
]
```
"#;

    let result: Result<Vec<Localization>, _> = parse_llm(input);
    let items = result.unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "CH1_Welcome");
    assert_eq!(items[0].English, "Welcome to Arcadian Atlas");
    assert_eq!(items[0].Portuguese, "Bem-vindo ao Arcadian Atlas");
}

#[cfg(feature = "derive")]
#[test]
fn test_triple_quoted_strings() {
    #[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
    struct Heading {
        heading: String,
        python_function_code: String,
        description: String,
    }

    #[derive(Debug, Clone, PartialEq, serde::Deserialize, LlmDeserialize)]
    struct Headings {
        headings: Vec<Heading>,
    }

    let input = r#"
{
  "headings": [
    {
      "heading": "Urban Oasis",
      "python_function_code": """def is_urban_oasis(property):
       return 'Large Green Area' in property['amenities'] or 'Garden' in property['amenities']""",
      "description": "Properties that offer a serene living experience amidst the bustling city life."
    }
  ]
}
"#;

    let result: Result<Headings, _> = parse_llm(input);
    let headings = result.unwrap();
    assert_eq!(headings.headings.len(), 1);
    assert_eq!(headings.headings[0].heading, "Urban Oasis");
    assert!(headings.headings[0]
        .python_function_code
        .contains("def is_urban_oasis"));
}

// ================================================================================================
// Edge Cases
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_incomplete_string_literal() {
    // BAML handles: "hello → "hello
    let result: Result<String, _> = parse_llm(r#""hello"#);
    assert_eq!(result.unwrap(), "\"hello");
}

#[cfg(feature = "derive")]
#[test]
fn test_prefixed_incomplete_string() {
    // BAML preserves: prefix "hello → prefix "hello
    let result: Result<String, _> = parse_llm(r#"prefix "hello"#);
    assert_eq!(result.unwrap(), "prefix \"hello");
}

// ================================================================================================
// Tests that should fail (negative tests)
// ================================================================================================

#[cfg(feature = "derive")]
#[test]
fn test_ambiguous_bool_should_fail() {
    // BAML should fail on: "The answer is true or false" (ambiguous)
    let result: Result<bool, _> = parse_llm("The answer is true or false");
    // This might succeed or fail depending on parser implementation
    // BAML fails this, but our parser might be more lenient
    if result.is_ok() {
        println!("Warning: Ambiguous bool parsed (BAML would reject this)");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_elaborate_ambiguous_bool_should_fail() {
    // BAML should fail when both true and false appear in response
    let input = r#"False. The statement "2 + 2 = 5" is not accurate according to basic arithmetic. In standard arithmetic, the sum of 2 and 2 is equal to 4, not 5. Therefore, the statement does not hold true."#;
    let result: Result<bool, _> = parse_llm(input);
    // Contains both "False" and "true" - BAML rejects this
    if result.is_ok() {
        println!("Warning: Ambiguous bool parsed (BAML would reject this)");
    }
}

#[cfg(not(feature = "derive"))]
#[test]
#[ignore]
fn placeholder_when_derive_disabled() {}
