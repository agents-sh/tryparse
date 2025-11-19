//! BRUTAL REALITY CHECK: What we DON'T handle
//!
//! This file documents real LLM response patterns that WILL break the library.
//! Stop lying about "handling all possible scenarios" - we don't.

use serde::Deserialize;
use tryparse::parse;
#[cfg(feature = "derive")]
use tryparse::parse_llm;

#[derive(Deserialize, Debug, PartialEq)]
struct User {
    name: String,
    age: u32,
}

// ============================================================================
// CRITICAL ARCHITECTURAL FLAW: Heuristic + Fixing Don't Combine
// ============================================================================

#[test]
fn test_malformed_json_in_prose() {
    // JSON is buried in prose AND has syntax errors
    // Multi-stage architecture SHOULD be able to handle this:
    // 1. Heuristic extracts {name: 'Alice', age: 30}
    // 2. Tries parsing - fails
    // 3. Applies fixes
    // 4. Parses successfully!
    //
    // Current issue: Extracted string isn't being passed through JSON fixer
    let response = r#"
    Sure! Here's the user data: {name: 'Alice', age: 30}
    Hope that helps!
    "#;

    let result: Result<User, _> = parse(response);
    assert!(
        result.is_ok(),
        "Multi-stage architecture should FIXED this!"
    );
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

// ============================================================================
// IGNORED TESTS FROM reality_check.rs - We're failing these!
// ============================================================================

#[test]
fn test_multiple_json_objects() {
    // MultipleObjectsStrategy is now enabled in default parser
    let response = r#"
    {"name": "Alice", "age": 30}
    {"name": "Bob", "age": 25}
    "#;

    let result: Result<Vec<User>, _> = parse(response);
    assert!(result.is_ok());
    let users = result.unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[1].name, "Bob");
}

#[test]
#[ignore] // No fuzzy field matching
fn test_case_insensitive_fields() {
    let response = r#"{"Name": "Alice", "AGE": 30}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_consecutive_commas() {
    // NEW: Garbage cleaning stage removes extra commas!
    let response = r#"{"name": "Alice",,,, "age": 30}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok(), "Garbage cleaner handles multiple commas");
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[test]
#[ignore] // Fixes are shallow only
fn test_deeply_nested_errors() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct Company {
        name: String,
        users: Vec<User>,
    }

    let response = r#"{
        "name": "ACME",
        "users": [
            {name: "Alice", age: 30},
            {name: 'Bob', age: '25'}
        ]
    }"#;
    let result: Result<Company, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[cfg(feature = "yaml")]
fn test_yaml_style() {
    // YAML support implemented!
    let response = "name: Alice\nage: 30";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok(), "YAML parsing should work");
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[test]
#[cfg(not(feature = "yaml"))]
#[ignore] // No YAML support without feature
fn test_yaml_style() {
    let response = "name: Alice\nage: 30";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

// ============================================================================
// REAL-WORLD SCENARIOS WE DEFINITELY FAIL
// ============================================================================

#[test]
#[cfg(feature = "derive")]
fn test_field_name_variations() {
    use tryparse_derive::LlmDeserialize;

    #[derive(Debug, serde::Deserialize, LlmDeserialize)]
    struct Config {
        user_name: String,
        max_count: i64,
    }

    // Field name fuzzy matching handles camelCase → snake_case!
    // This uses the struct deserializer's FieldMatcher, not parser-level normalization
    let response = r#"{"userName": "Alice", "maxCount": 30}"#;
    let result: Result<Config, _> = parse_llm(response);
    assert!(
        result.is_ok(),
        "Field fuzzy matching handles camelCase → snake_case"
    );
    let config = result.unwrap();
    assert_eq!(config.user_name, "Alice");
    assert_eq!(config.max_count, 30);
}

#[test]
#[cfg(not(feature = "derive"))]
#[ignore] // Requires fuzzy field matching which is only available with LlmDeserialize
fn test_field_name_variations() {}

#[test]
#[ignore] // Mixed quote styles in same object
fn test_mixed_quote_styles() {
    let response = r#"{"name": 'Alice', "age": "30"}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Template literals with backticks
fn test_template_literals() {
    let response = r#"{"name": `Alice`, "age": 30}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Multi-line strings without proper escaping
fn test_unescaped_newlines_in_strings() {
    let response = r#"{
        "name": "Alice
Bob",
        "age": 30
    }"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Double-escaped JSON
fn test_double_escaped() {
    let response = r#"{\"name\": \"Alice\", \"age\": 30}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // JSON wrapped in extra quotes
fn test_stringified_json() {
    let response = r#""{\"name\": \"Alice\", \"age\": 30}""#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Duplicate keys - which value wins?
fn test_duplicate_keys() {
    let response = r#"{"name": "Alice", "age": 30, "name": "Bob"}"#;
    let result: Result<User, _> = parse(response);
    // Undefined behavior - serde_json takes last value
    // But we should at least not panic
    assert!(result.is_ok());
}

#[test]
#[ignore] // Infinity and NaN (invalid JSON)
fn test_infinity_nan() {
    let response = r#"{"name": "Alice", "age": Infinity}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_err(), "Infinity is not valid JSON");
}

#[test]
#[ignore] // Scientific notation edge cases
fn test_scientific_notation() {
    let response = r#"{"name": "Alice", "age": 3e1}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Hex numbers
fn test_hex_numbers() {
    let response = r#"{"name": "Alice", "age": 0x1E}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Unicode zero-width spaces
fn test_zero_width_characters() {
    // Contains zero-width space (U+200B) before the colon
    let response = "{\"name\"\u{200B}: \"Alice\", \"age\": 30}";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // BOM at start
fn test_byte_order_mark() {
    let response = "\u{FEFF}{\"name\": \"Alice\", \"age\": 30}";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // HTML entities
fn test_html_entities() {
    let response = r#"{"name": "Alice&nbsp;Bob", "age": 30}"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Multiple code blocks - which one is real?
fn test_multiple_code_blocks() {
    let response = r#"
    Here's an example:
    ```json
    {"name": "Example", "age": 999}
    ```

    But here's the real data:
    ```json
    {"name": "Alice", "age": 30}
    ```
    "#;

    let result: Result<User, _> = parse(response);
    // Which one should we pick? First? Last? Biggest?
    // Currently we'd get the first one (wrong!)
    assert_eq!(result.unwrap().name, "Alice", "Should pick the 'real' data");
}

#[test]
#[ignore] // JSON mixed with code and explanations
fn test_mixed_content() {
    let response = r#"
    Let me calculate that for you.

    ```python
    user = {"name": "Example"}
    print(user)
    ```

    The actual result is: {"name": "Alice", "age": 30}

    Hope this helps!
    "#;

    let result: Result<User, _> = parse(response);
    assert_eq!(result.unwrap().name, "Alice");
}

#[test]
#[ignore] // Markdown table that looks like data
fn test_markdown_table() {
    let response = r#"
    | name  | age |
    |-------|-----|
    | Alice | 30  |
    "#;

    let result: Result<User, _> = parse(response);
    assert!(result.is_ok(), "Should parse markdown tables");
}

#[test]
#[ignore] // Plain text key-value
fn test_plain_text_key_value() {
    let response = "name: Alice\nage: 30";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // XML instead of JSON
fn test_xml() {
    let response = "<user><name>Alice</name><age>30</age></user>";
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok(), "Should parse XML");
}

#[test]
#[ignore] // JavaScript object with functions
fn test_javascript_object() {
    let response = r#"{
        name: "Alice",
        age: 30,
        greet: function() { return "Hi"; }
    }"#;
    let result: Result<User, _> = parse(response);
    // Should extract just the data fields
    assert!(result.is_ok());
}

#[test]
#[ignore] // Comments with nested delimiters
fn test_complex_comments() {
    let response = r#"{
        "name": "Alice", /* comment with } and { inside */
        "age": 30 // another comment with }
    }"#;
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

#[test]
#[ignore] // Very deeply nested structure
fn test_very_deep_nesting() {
    let response = format!(
        "{}{{\"name\": \"Alice\", \"age\": 30}}{}",
        "{".repeat(100),
        "}".repeat(100)
    );
    let result: Result<User, _> = parse(&response);
    // Should handle deep nesting without stack overflow
    assert!(result.is_ok());
}

#[test]
#[ignore] // Extremely large JSON (DoS test)
fn test_very_large_input() {
    let huge_array = format!("[{}]", "1,".repeat(1_000_000));
    let result: Result<Vec<i32>, _> = parse(&huge_array);
    // Should either parse or reject gracefully, not crash
    let _ = result;
}

#[test]
#[ignore] // Circular reference (as string)
fn test_circular_reference_string() {
    let response = r#"{"name": "Alice", "age": 30, "self": "[Circular]"}"#;
    // Should at least not panic
    let result: Result<User, _> = parse(response);
    assert!(result.is_ok());
}

// ============================================================================
// OPTIMIZATION FAILURES - User asked for "optimize as much as possible"
// ============================================================================

#[test]
#[ignore] // We do NO caching/memoization
fn test_no_memoization() {
    // If we try the same fix multiple times, we recompute it every time
    // No test for this, but it's a performance issue
}

#[test]
#[ignore] // We allocate strings for every fix attempt
fn test_excessive_allocations() {
    // We clone strings in every fix method
    // Could use Cow or in-place modifications
}

#[test]
#[ignore] // No early termination
fn test_no_early_termination() {
    // Even if we find a perfect match, we continue trying all strategies
    // Could add a quality threshold and stop early
}

// ============================================================================
// SUMMARY: What percentage of REAL scenarios do we handle?
// ============================================================================

// From reality_check.rs:
// - 15 tests pass
// - 7 tests ignored (failures)
// = 68% of basic scenarios

// From this file:
// - 0 tests pass
// - 40+ tests ignored (failures)
// = We fail at most real-world edge cases

// HONEST ASSESSMENT: We handle ~70% of basic scenarios, ~10% of edge cases
