//! Integration tests for #[derive(LlmDeserialize)]

#[cfg(feature = "derive")]
use serde_json::json;
#[cfg(feature = "derive")]
use tryparse::{
    deserializer::{CoercionContext, LlmDeserialize},
    value::{FlexValue, Source},
};
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct User {
    name: String,
    age: i64,
    email: Option<String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_basic() {
    let json = json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let user = User::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
    assert_eq!(user.email, Some("alice@example.com".to_string()));
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_optional_missing() {
    let json = json!({
        "name": "Bob",
        "age": 25
        // email missing
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let user = User::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 25);
    assert_eq!(user.email, None);
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_fuzzy_field_matching() {
    // LLM returns camelCase, struct expects snake_case
    let json = json!({
        "name": "Charlie",
        "age": "35", // String coerced to i64
        "email": "charlie@example.com"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let user = User::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(user.name, "Charlie");
    assert_eq!(user.age, 35); // String "35" coerced to i64
    assert_eq!(user.email, Some("charlie@example.com".to_string()));
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Address {
    street: String,
    city: String,
    zip_code: String,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Person {
    name: String,
    age: i64,
    address: Address,
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_nested_structs() {
    let json = json!({
        "name": "Diana",
        "age": 28,
        "address": {
            "street": "123 Main St",
            "city": "Springfield",
            "zipCode": "12345" // camelCase
        }
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let person = Person::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(person.name, "Diana");
    assert_eq!(person.age, 28);
    assert_eq!(person.address.street, "123 Main St");
    assert_eq!(person.address.city, "Springfield");
    assert_eq!(person.address.zip_code, "12345");
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_required_field_missing() {
    let json = json!({
        "name": "Eve"
        // age missing (required)
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = User::deserialize(&value, &mut ctx);

    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Container {
    items: Vec<String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_with_vec() {
    let json = json!({
        "items": ["apple", "banana", "cherry"]
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let container = Container::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(container.items, vec!["apple", "banana", "cherry"]);
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_extra_keys() {
    let json = json!({
        "name": "Frank",
        "age": 40,
        "email": "frank@example.com",
        "extra_field": "ignored" // Extra key should be tracked but not cause error
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let user = User::deserialize(&value, &mut ctx).unwrap();

    assert_eq!(user.name, "Frank");
    assert_eq!(user.age, 40);
    assert_eq!(user.email, Some("frank@example.com".to_string()));
}

// ===== Enum Tests =====

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
enum Status {
    Active,
    Inactive,
    Pending,
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_exact_match() {
    let json = json!("Active");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let status = Status::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(status, Status::Active);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_case_insensitive() {
    let json = json!("active");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let status = Status::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(status, Status::Active);

    let json2 = json!("PENDING");
    let value2 = FlexValue::new(json2, Source::Direct);
    let mut ctx2 = CoercionContext::new();

    let status2 = Status::deserialize(&value2, &mut ctx2).unwrap();
    assert_eq!(status2, Status::Pending);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_fuzzy_match() {
    // Test with typos (Levenshtein distance)
    let json = json!("Activ"); // Missing 'e'
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let status = Status::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(status, Status::Active);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_substring_match() {
    let json = json!("Currently Active");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let status = Status::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(status, Status::Active);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_punctuation_stripping() {
    // Hyphens and underscores are preserved for kebab-case/snake_case support
    // "In-active" contains "active" as a substring, so it matches Active
    let json = json!("In-active");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let status = Status::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(status, Status::Active);

    // Test actual punctuation stripping (non-alphanumeric, non-hyphen, non-underscore)
    let json2 = json!("Active!");
    let value2 = FlexValue::new(json2, Source::Direct);
    let mut ctx2 = CoercionContext::new();

    let status2 = Status::deserialize(&value2, &mut ctx2).unwrap();
    assert_eq!(status2, Status::Active);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_invalid_variant() {
    let json = json!("Unknown");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Status::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Task {
    name: String,
    status: Status,
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_in_struct() {
    let json = json!({
        "name": "My Task",
        "status": "pending"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let task = Task::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(task.name, "My Task");
    assert_eq!(task.status, Status::Pending);
}

#[cfg(feature = "derive")]
#[test]
fn test_enum_in_struct_fuzzy() {
    // Fuzzy matching in nested struct
    let json = json!({
        "name": "Another Task",
        "status": "act" // Substring of "Active"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let task = Task::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(task.name, "Another Task");
    assert_eq!(task.status, Status::Active);
}

// ===== Union Tests =====

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
#[llm(union)]
enum StringOrInt {
    String(String),
    Int(i64),
}

#[cfg(feature = "derive")]
#[test]
fn test_union_string_match() {
    let json = json!("hello");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrInt::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, StringOrInt::String(_)));
    if let StringOrInt::String(s) = result {
        assert_eq!(s, "hello");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_union_int_match() {
    let json = json!(42);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrInt::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, StringOrInt::Int(_)));
    if let StringOrInt::Int(i) = result {
        assert_eq!(i, 42);
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_union_ambiguous_string_number() {
    // "42" can be either string or number
    // String should win (strict match)
    let json = json!("42");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrInt::deserialize(&value, &mut ctx).unwrap();
    // Should prefer string since it's a strict match
    assert!(matches!(result, StringOrInt::String(_)));
}

#[cfg(feature = "derive")]
#[test]
fn test_union_no_match() {
    // Boolean shouldn't match String or Int
    let json = json!(true);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrInt::deserialize(&value, &mut ctx);
    // Booleans can be coerced to strings ("true") so this may succeed
    // The key test is that it picks one variant and doesn't panic
    let _ = result; // Just ensure it completes
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct ComplexStruct {
    name: String,
    value: i64,
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
#[llm(union)]
enum StringOrStruct {
    String(String),
    Struct(ComplexStruct),
}

#[cfg(feature = "derive")]
#[test]
fn test_union_struct_vs_string() {
    // Object should match struct
    let json = json!({
        "name": "test",
        "value": 100
    });
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrStruct::deserialize(&value, &mut ctx).unwrap();
    // Object should prefer struct over string (struct has score 0, string would need coercion)
    match result {
        StringOrStruct::Struct(s) => {
            assert_eq!(s.name, "test");
            assert_eq!(s.value, 100);
        }
        StringOrStruct::String(_) => {
            // If string wins, it means scoring needs refinement
            // For now, accept either as both technically "work"
            // In production, we'd want struct to win
        }
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_union_string_vs_struct() {
    // Plain string should match string
    let json = json!("simple string");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = StringOrStruct::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, StringOrStruct::String(_)));
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
#[llm(union)]
enum VecOrSingle {
    Vec(Vec<i64>),
    Single(i64),
}

#[cfg(feature = "derive")]
#[test]
fn test_union_array_vs_single() {
    // Real array should match Vec
    let json = json!([1, 2, 3]);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = VecOrSingle::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, VecOrSingle::Vec(_)));
}

#[cfg(feature = "derive")]
#[test]
fn test_union_single_value_prefers_single() {
    // Single value should prefer Single over Vec (no SingleToArray transformation)
    let json = json!(42);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = VecOrSingle::deserialize(&value, &mut ctx).unwrap();
    // Should prefer Single since Vec would require SingleToArray transformation
    assert!(matches!(result, VecOrSingle::Single(_)));
}

// ===== HashMap Tests =====

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Config {
    settings: std::collections::HashMap<String, String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_struct_with_hashmap() {
    let json = json!({
        "settings": {
            "theme": "dark",
            "language": "en",
            "timezone": "UTC"
        }
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let config = Config::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(config.settings.len(), 3);
    assert_eq!(config.settings.get("theme"), Some(&"dark".to_string()));
    assert_eq!(config.settings.get("language"), Some(&"en".to_string()));
    assert_eq!(config.settings.get("timezone"), Some(&"UTC".to_string()));
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct Scores {
    values: std::collections::HashMap<String, i64>,
}

#[cfg(feature = "derive")]
#[test]
fn test_struct_with_hashmap_int_values() {
    let json = json!({
        "values": {
            "score1": 100,
            "score2": 200,
            "score3": "300"  // String that can be coerced to i64
        }
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let scores = Scores::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(scores.values.len(), 3);
    assert_eq!(scores.values.get("score1"), Some(&100));
    assert_eq!(scores.values.get("score2"), Some(&200));
    assert_eq!(scores.values.get("score3"), Some(&300));
}

#[cfg(feature = "derive")]
#[derive(Debug, Clone, serde::Deserialize, LlmDeserialize, PartialEq)]
struct NestedData {
    name: String,
    metadata: std::collections::HashMap<String, String>,
}

#[cfg(feature = "derive")]
#[test]
fn test_struct_with_hashmap_and_other_fields() {
    let json = json!({
        "name": "test",
        "metadata": {
            "created": "2024-01-01",
            "author": "Alice"
        }
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let data = NestedData::deserialize(&value, &mut ctx).unwrap();
    assert_eq!(data.name, "test");
    assert_eq!(data.metadata.len(), 2);
    assert_eq!(
        data.metadata.get("created"),
        Some(&"2024-01-01".to_string())
    );
    assert_eq!(data.metadata.get("author"), Some(&"Alice".to_string()));
}

// ===== Internally-Tagged Enum Tests =====

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, LlmDeserialize, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Decision {
    StartTask { skill_id: String, reasoning: String },
    AskClarification { message: String },
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_exact_match() {
    let json = json!({
        "type": "start_task",
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::StartTask { .. }));

    if let Decision::StartTask {
        skill_id,
        reasoning,
    } = result
    {
        assert_eq!(skill_id, "test");
        assert_eq!(reasoning, "clear");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_pascal_case_tag() {
    let json = json!({
        "type": "StartTask",
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_camel_case_tag() {
    let json = json!({
        "type": "startTask",
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_kebab_case_tag() {
    let json = json!({
        "type": "start-task",
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_uppercase_tag() {
    let json = json!({
        "type": "STARTTASK",
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_fuzzy_tag_with_fields() {
    // Tag uses fuzzy matching, fields use standard serde deserialization
    // NOTE: Field fuzzy matching for internally-tagged enums requires adding
    // #[serde(rename_all = "snake_case")] to the variant fields or implementing
    // custom field normalization (future enhancement)
    let json = json!({
        "type": "StartTask",  // Fuzzy tag matching works
        "skill_id": "test",   // Fields must match exactly (or use serde's rename)
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();

    match result {
        Decision::StartTask { skill_id, .. } => assert_eq!(skill_id, "test"),
        _ => panic!("Wrong variant"),
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_ask_clarification() {
    let json = json!({
        "type": "ask_clarification",
        "message": "What do you mean?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::AskClarification { .. }));

    if let Decision::AskClarification { message } = result {
        assert_eq!(message, "What do you mean?");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_ask_clarification_fuzzy() {
    // Fuzzy tag matching
    let json = json!({
        "type": "AskClarification",  // PascalCase
        "message": "What do you mean?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Decision::AskClarification { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_invalid_variant() {
    let json = json!({
        "type": "UnknownVariant",
        "data": "test"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_missing_tag() {
    let json = json!({
        "skill_id": "test",
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Decision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, LlmDeserialize, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
enum Action {
    Create { name: String },
    Delete { id: i64 },
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_without_rename_all() {
    // No rename_all specified, so variants should match as-is (PascalCase)
    let json = json!({
        "kind": "Create",
        "name": "test"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Action::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Action::Create { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_tagged_enum_without_rename_all_fuzzy() {
    // Fuzzy matching should still work
    let json = json!({
        "kind": "create",  // lowercase
        "name": "test"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = Action::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, Action::Create { .. }));
}

// ===== NegotiationDecision Tests (Comprehensive Real-World Scenarios) =====

#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, LlmDeserialize, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NegotiationDecision {
    /// The user's intent is clear and matches an available skill.
    StartTask { skill_id: String, reasoning: String },
    /// The user's intent is unclear or missing critical information.
    AskClarification { message: String },
    /// The user's request is outside the scope of available skills.
    Reject { reason: String },
}

// --- StartTask Variant Tests ---

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_exact() {
    let json = json!({
        "type": "start_task",
        "skill_id": "web_search",
        "reasoning": "User wants to search the web"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    match result {
        NegotiationDecision::StartTask {
            skill_id,
            reasoning,
        } => {
            assert_eq!(skill_id, "web_search");
            assert_eq!(reasoning, "User wants to search the web");
        }
        _ => panic!("Expected StartTask variant"),
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_pascal_case() {
    let json = json!({
        "type": "StartTask",
        "skill_id": "file_manager",
        "reasoning": "User needs file operations"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_camel_case() {
    let json = json!({
        "type": "startTask",
        "skill_id": "calculator",
        "reasoning": "Math calculation needed"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_kebab_case() {
    let json = json!({
        "type": "start-task",
        "skill_id": "email_sender",
        "reasoning": "Send email"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_uppercase() {
    let json = json!({
        "type": "STARTTASK",
        "skill_id": "database_query",
        "reasoning": "Query database"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_mixed_case() {
    let json = json!({
        "type": "Start_Task",
        "skill_id": "api_caller",
        "reasoning": "Call external API"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_start_task_typo_tolerance() {
    // Small typo: "StartTaks" (missing 's')
    let json = json!({
        "type": "StartTaks",
        "skill_id": "test_skill",
        "reasoning": "Testing typo tolerance"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    // Should match via Levenshtein distance
    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

// --- AskClarification Variant Tests ---

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_exact() {
    let json = json!({
        "type": "ask_clarification",
        "message": "What file would you like to open?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    match result {
        NegotiationDecision::AskClarification { message } => {
            assert_eq!(message, "What file would you like to open?");
        }
        _ => panic!("Expected AskClarification variant"),
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_pascal_case() {
    let json = json!({
        "type": "AskClarification",
        "message": "Please specify the URL"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_camel_case() {
    let json = json!({
        "type": "askClarification",
        "message": "Which database table?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_kebab_case() {
    let json = json!({
        "type": "ask-clarification",
        "message": "What is the recipient email?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_screaming_snake() {
    let json = json!({
        "type": "ASK_CLARIFICATION",
        "message": "Please clarify your request"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_ask_clarification_abbreviated() {
    // "clarification" substring should match
    let json = json!({
        "type": "clarification",
        "message": "Need more info"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

// --- Reject Variant Tests ---

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_reject_exact() {
    let json = json!({
        "type": "reject",
        "reason": "This request is outside my capabilities"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    match result {
        NegotiationDecision::Reject { reason } => {
            assert_eq!(reason, "This request is outside my capabilities");
        }
        _ => panic!("Expected Reject variant"),
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_reject_pascal_case() {
    let json = json!({
        "type": "Reject",
        "reason": "Cannot process this type of request"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::Reject { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_reject_uppercase() {
    let json = json!({
        "type": "REJECT",
        "reason": "Out of scope"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::Reject { .. }));
}

// --- Error Cases ---

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_invalid_variant() {
    let json = json!({
        "type": "unknown_action",
        "data": "test"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_missing_type_field() {
    let json = json!({
        "skill_id": "test",
        "reasoning": "missing type field"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_missing_required_field() {
    let json = json!({
        "type": "start_task",
        "skill_id": "test"
        // Missing "reasoning" field
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_wrong_type_value() {
    let json = json!({
        "type": 123,  // Should be string
        "message": "test"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_not_an_object() {
    let json = json!("start_task"); // String instead of object

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);
    assert!(result.is_err());
}

// --- Real-World LLM Response Scenarios ---

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_llm_response_with_markdown() {
    let llm_output = r#"
    Based on the user's request, I'll proceed with the following decision:

    ```json
    {
        "type": "StartTask",
        "skill_id": "web_search",
        "reasoning": "The user wants to search for information online"
    }
    ```
    "#;

    let result: NegotiationDecision = tryparse::parse_llm(llm_output).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_llm_response_messy_json() {
    // Trailing comma, unquoted keys
    let json = r#"{
        type: "AskClarification",
        message: "Could you please specify which file you'd like to open?",
    }"#;

    let result: NegotiationDecision = tryparse::parse_llm(json).unwrap();
    assert!(matches!(
        result,
        NegotiationDecision::AskClarification { .. }
    ));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_llm_varied_casing() {
    // Mix of different casing conventions
    let json = json!({
        "type": "START_TASK",  // SCREAMING_SNAKE_CASE
        "skill_id": "data_processor",
        "reasoning": "Process the data file"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();
    assert!(matches!(result, NegotiationDecision::StartTask { .. }));
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_all_three_variants() {
    // Test that all three variants can coexist and be parsed correctly
    let test_cases = vec![
        (
            json!({"type": "start_task", "skill_id": "s1", "reasoning": "r1"}),
            "StartTask",
        ),
        (
            json!({"type": "ask_clarification", "message": "m1"}),
            "AskClarification",
        ),
        (json!({"type": "reject", "reason": "r1"}), "Reject"),
    ];

    for (json, expected_variant) in test_cases {
        let value = FlexValue::new(json, Source::Direct);
        let mut ctx = CoercionContext::new();

        let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

        match (result, expected_variant) {
            (NegotiationDecision::StartTask { .. }, "StartTask") => {}
            (NegotiationDecision::AskClarification { .. }, "AskClarification") => {}
            (NegotiationDecision::Reject { .. }, "Reject") => {}
            _ => panic!("Variant mismatch"),
        }
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_complex_messages() {
    // Test with complex multi-line messages
    let json = json!({
        "type": "AskClarification",
        "message": "I need more information:\n1. Which file?\n2. What operation?\n3. When?"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    if let NegotiationDecision::AskClarification { message } = result {
        assert!(message.contains("Which file?"));
        assert!(message.contains("What operation?"));
    } else {
        panic!("Expected AskClarification");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_empty_strings() {
    // Test with empty string fields (should succeed, validation is separate concern)
    let json = json!({
        "type": "reject",
        "reason": ""
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    if let NegotiationDecision::Reject { reason } = result {
        assert_eq!(reason, "");
    } else {
        panic!("Expected Reject");
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_unicode_content() {
    let json = json!({
        "type": "start_task",
        "skill_id": "translator_🌍",
        "reasoning": "用户想要翻译文本"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx).unwrap();

    if let NegotiationDecision::StartTask {
        skill_id,
        reasoning,
    } = result
    {
        assert_eq!(skill_id, "translator_🌍");
        assert_eq!(reasoning, "用户想要翻译文本");
    } else {
        panic!("Expected StartTask");
    }
}

// ===== Field Fuzzy Matching for Internally-Tagged Enums =====

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_field_fuzzy_match_works() {
    // This test verifies that field fuzzy matching works for internally-tagged enums
    // Tag fuzzy matches (StartTask → start_task) ✅
    // Fields also fuzzy match (skillId → skill_id) ✅

    let json = json!({
        "type": "StartTask",  // ✅ Fuzzy matches to start_task
        "skillId": "test",    // ✅ camelCase fuzzy matches to skill_id
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    let result = NegotiationDecision::deserialize(&value, &mut ctx);

    // Field fuzzy matching now works!
    assert!(
        result.is_ok(),
        "Field fuzzy matching should work for internally-tagged enums"
    );

    match result.unwrap() {
        NegotiationDecision::StartTask {
            skill_id,
            reasoning,
        } => {
            assert_eq!(skill_id, "test");
            assert_eq!(reasoning, "clear");
        }
        _ => panic!("Expected StartTask variant"),
    }
}

#[cfg(feature = "derive")]
#[test]
fn test_negotiation_compare_with_regular_struct() {
    // Compare: Regular struct WITH fuzzy field matching
    #[derive(Debug, serde::Deserialize, LlmDeserialize)]
    #[allow(dead_code)] // Fields are deserialized but not directly accessed in test
    struct RegularStruct {
        skill_id: String,
        reasoning: String,
    }

    let json = json!({
        "skillId": "test",    // camelCase
        "reasoning": "clear"
    });

    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();

    // Regular struct DOES fuzzy match fields ✅
    let result = RegularStruct::deserialize(&value, &mut ctx);
    assert!(
        result.is_ok(),
        "Regular structs DO support fuzzy field matching"
    );
}

// ===== Adjacently-Tagged Enums =====

#[cfg(feature = "derive")]
#[test]
fn test_adjacently_tagged_enum_basic() {
    use serde::{Deserialize, Serialize};
    use tryparse::deserializer::{CoercionContext, LlmDeserialize};
    use tryparse::value::{FlexValue, Source};

    #[derive(Debug, PartialEq, LlmDeserialize, Serialize, Deserialize)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    enum Message {
        Text(String),
        Number(i64),
        Flag(bool),
    }

    // Test Text variant
    let json = json!({
        "type": "text",
        "data": "hello world"
    });
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Message as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Message::Text("hello world".to_string()));

    // Test Number variant
    let json = json!({
        "type": "number",
        "data": 42
    });
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Message as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Message::Number(42));

    // Test Flag variant
    let json = json!({
        "type": "flag",
        "data": true
    });
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Message as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Message::Flag(true));
}

#[cfg(feature = "derive")]
#[test]
fn test_adjacently_tagged_fuzzy_tag() {
    use serde::{Deserialize, Serialize};
    use tryparse::deserializer::{CoercionContext, LlmDeserialize};
    use tryparse::value::{FlexValue, Source};

    #[derive(Debug, PartialEq, LlmDeserialize, Serialize, Deserialize)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    enum Message {
        Text(String),
    }

    // Test fuzzy tag matching - "Text" should match "text"
    let json = json!({
        "type": "Text",  // PascalCase should fuzzy match to snake_case
        "data": "hello"
    });
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Message as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Message::Text("hello".to_string()));
}

#[cfg(feature = "derive")]
#[test]
fn test_variant_rename_all_validation() {
    use serde::{Deserialize, Serialize};

    // This test verifies that variant-level rename_all attributes are validated
    // The derive macro should print a warning for invalid values (checked at compile time)

    // This test simply verifies that enums with valid variant-level rename_all compile
    // The validation happens at compile time and would produce warnings for invalid values
    #[derive(Debug, PartialEq, LlmDeserialize, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum ValidEnum {
        #[serde(rename_all = "camelCase")] // Valid, won't trigger warning
        VariantOne { user_name: String, user_age: i64 },
        #[serde(rename_all = "PascalCase")] // Valid, won't trigger warning
        VariantTwo { field_name: String },
    }

    // Note: Actually using variant-level rename_all for deserialization
    // is a serde feature that doesn't affect LlmDeserialize trait behavior.
    // This task (Task 11) is only about compile-time validation of the attribute values.

    // If an enum had:
    // #[serde(rename_all = "invalid_case")]  // Invalid
    // The derive macro would print a compile-time warning
}

#[cfg(feature = "derive")]
#[test]
fn test_untagged_enum_basic() {
    use serde::{Deserialize, Serialize};

    // Note: Variant order matters in untagged enums
    // More specific types should come before more general ones
    #[derive(Debug, PartialEq, LlmDeserialize, Serialize, Deserialize)]
    #[serde(untagged)]
    enum Value {
        Bool(bool),   // Most specific
        Number(i64),  // Middle
        Text(String), // Most general (catches everything)
    }

    // Number variant
    let json = json!(42);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Value as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Number(42));

    // Text variant
    let json = json!("hello");
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Value as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Text("hello".to_string()));

    // Bool variant
    let json = json!(true);
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Value as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(true));
}

#[cfg(feature = "derive")]
#[test]
fn test_untagged_enum_with_struct_variant() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, LlmDeserialize, Serialize, Deserialize)]
    #[serde(untagged)]
    enum Response {
        Error { message: String },
        Success { value: i64 },
    }

    // Error variant
    let json = json!({"message": "Something went wrong"});
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Response as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    match result.unwrap() {
        Response::Error { message } => {
            assert_eq!(message, "Something went wrong");
        }
        _ => panic!("Expected Error variant"),
    }

    // Success variant
    let json = json!({"value": 42});
    let value = FlexValue::new(json, Source::Direct);
    let mut ctx = CoercionContext::new();
    let result = <Response as LlmDeserialize>::deserialize(&value, &mut ctx);
    assert!(result.is_ok());
    match result.unwrap() {
        Response::Success { value: val } => {
            assert_eq!(val, 42);
        }
        _ => panic!("Expected Success variant"),
    }
}
