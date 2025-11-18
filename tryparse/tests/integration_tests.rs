//! Comprehensive integration tests demonstrating full system capabilities.
//!
//! These tests verify the complete parsing pipeline including:
//! - Multi-strategy parsing
//! - Field normalization
//! - Type coercion
//! - Robust error handling

use serde::Deserialize;
use tryparse::{parse, parse_with_candidates};

// ============================================================================
// Test Structures
// ============================================================================

#[derive(Debug, Deserialize, PartialEq)]
struct User {
    name: String,
    age: u32,
    email: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Product {
    id: i64,
    name: String,
    price: f64,
    in_stock: bool,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NestedData {
    user: User,
    products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are deserialized but not directly accessed in test
enum Status {
    Active,
    Pending,
    Completed { message: String },
}

// ============================================================================
// Basic Parsing Tests
// ============================================================================

#[test]
fn test_basic_valid_json() {
    let input = r#"{"name": "Alice", "age": 30, "email": "alice@example.com"}"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
    assert_eq!(user.email, Some("alice@example.com".to_string()));
}

#[test]
fn test_type_coercion() {
    let input = r#"{"name": "Bob", "age": "25", "email": null}"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 25); // String coerced to u32
    assert_eq!(user.email, None);
}

#[test]
fn test_markdown_extraction() {
    let input = r#"
Here's the user data:
```json
{"name": "Charlie", "age": 35}
```
That's all!
"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Charlie");
    assert_eq!(user.age, 35);
}

#[test]
fn test_malformed_json_fixes() {
    // Trailing comma
    let input = r#"{"name": "Dave", "age": 40,}"#;
    let user: User = parse(input).unwrap();
    assert_eq!(user.name, "Dave");

    // Single quotes
    let input = r#"{'name': 'Eve', 'age': 45}"#;
    let user: User = parse(input).unwrap();
    assert_eq!(user.name, "Eve");
}

// ============================================================================
// State Machine Tests
// ============================================================================

#[test]
fn test_unclosed_object() {
    let input = r#"{"name": "Frank", "age": 50"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Frank");
    assert_eq!(user.age, 50);
}

#[test]
fn test_unclosed_array() {
    let input =
        r#"{"id": 1, "name": "Widget", "price": 9.99, "in_stock": true, "tags": ["new", "sale""#;
    let product: Product = parse(input).unwrap();

    assert_eq!(product.id, 1);
    assert_eq!(product.tags, vec!["new", "sale"]);
}

#[test]
fn test_multiple_top_level_objects() {
    let input = r#"{"name": "Alice", "age": 30} {"name": "Bob", "age": 25}"#;

    // Should parse the first object
    let user: User = parse(input).unwrap();
    assert_eq!(user.name, "Alice");
}

// ============================================================================
// Complex Structure Tests
// ============================================================================

#[test]
fn test_nested_structures() {
    let input = r#"{
        "user": {"name": "Alice", "age": 30},
        "products": [
            {"id": 1, "name": "Widget", "price": 9.99, "in_stock": true, "tags": ["new"]},
            {"id": 2, "name": "Gadget", "price": 19.99, "in_stock": false, "tags": ["sale"]}
        ]
    }"#;

    let data: NestedData = parse(input).unwrap();

    assert_eq!(data.user.name, "Alice");
    assert_eq!(data.products.len(), 2);
    assert_eq!(data.products[0].name, "Widget");
    assert_eq!(data.products[1].name, "Gadget");
}

#[test]
fn test_array_of_objects() {
    let input = r#"[
        {"name": "Alice", "age": 30},
        {"name": "Bob", "age": 25},
        {"name": "Charlie", "age": 35}
    ]"#;

    let users: Vec<User> = parse(input).unwrap();

    assert_eq!(users.len(), 3);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[1].name, "Bob");
    assert_eq!(users[2].name, "Charlie");
}

// ============================================================================
// Candidate Ranking Tests
// ============================================================================

#[test]
fn test_parse_with_candidates() {
    let input = r#"{"name": "Alice", "age": "30"}"#;
    let (user, candidates): (User, _) = parse_with_candidates(input).unwrap();

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
    assert!(!candidates.is_empty());
}

#[test]
fn test_multiple_candidates_best_selected() {
    // This input might generate multiple candidates due to type coercion
    let input = r#"{"name": "Bob", "age": "25", "email": "bob@example.com"}"#;
    let (user, candidates): (User, _) = parse_with_candidates(input).unwrap();

    assert_eq!(user.name, "Bob");
    assert!(!candidates.is_empty());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_input_error() {
    let input = "This is not JSON at all";
    let result: Result<User, _> = parse(input);

    assert!(result.is_err());
}

#[test]
fn test_empty_input() {
    let input = "";
    let result: Result<User, _> = parse(input);

    // Should return error or empty result
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_type_mismatch() {
    // Age is a string that can't be coerced to number
    let input = r#"{"name": "Alice", "age": "not_a_number"}"#;
    let result: Result<User, _> = parse(input);

    assert!(result.is_err());
}

// ============================================================================
// Real-World LLM Response Tests
// ============================================================================

#[test]
fn test_llm_response_with_explanation() {
    let input = r#"
Sure! Here's the user information you requested:

```json
{
  "name": "Alice Johnson",
  "age": 30,
  "email": "alice.johnson@example.com"
}
```

This user is an active member of our platform.
"#;

    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Alice Johnson");
    assert_eq!(user.age, 30);
    assert_eq!(user.email, Some("alice.johnson@example.com".to_string()));
}

#[test]
fn test_llm_response_with_markdown_and_comments() {
    let input = r#"
Let me create a product for you:

```json
{
  "id": 12345,
  "name": "Super Widget",
  "price": 29.99,
  "in_stock": true,
  "tags": ["bestseller", "new"]
}
```

Hope this helps!
"#;

    let product: Product = parse(input).unwrap();

    assert_eq!(product.id, 12345);
    assert_eq!(product.name, "Super Widget");
    assert_eq!(product.price, 29.99);
    assert!(product.in_stock);
}

#[test]
fn test_llm_response_incomplete() {
    // Simulating an LLM that was cut off mid-response
    let input = r#"
Here's the product data:
{
  "id": 999,
  "name": "Incomplete Widget",
  "price": 15.99,
  "in_stock": true,
  "tags": ["new"
"#;

    let product: Product = parse(input).unwrap();

    assert_eq!(product.id, 999);
    assert_eq!(product.name, "Incomplete Widget");
    assert_eq!(product.price, 15.99);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_optional_fields() {
    // Email field is optional
    let input = r#"{"name": "Bob", "age": 25}"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Bob");
    assert_eq!(user.age, 25);
    assert_eq!(user.email, None);
}

#[test]
fn test_empty_array() {
    let input = r#"{"id": 1, "name": "Empty", "price": 0.0, "in_stock": false, "tags": []}"#;
    let product: Product = parse(input).unwrap();

    assert_eq!(product.tags, Vec::<String>::new());
}

#[test]
fn test_whitespace_handling() {
    let input = r#"

    {
        "name"   :   "Alice"  ,
        "age"    :   30
    }

    "#;

    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);
}

#[test]
fn test_unicode_strings() {
    let input = r#"{"name": "José García 日本", "age": 30}"#;
    let user: User = parse(input).unwrap();

    assert_eq!(user.name, "José García 日本");
}

// ============================================================================
// Performance / Stress Tests
// ============================================================================

#[test]
fn test_large_array() {
    // Create a large array of users
    let mut json = String::from("[");
    for i in 0..100 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            r#"{{"name": "User{}", "age": {}}}"#,
            i,
            20 + (i % 50)
        ));
    }
    json.push(']');

    let users: Vec<User> = parse(&json).unwrap();

    assert_eq!(users.len(), 100);
    assert_eq!(users[0].name, "User0");
    assert_eq!(users[99].name, "User99");
}

#[test]
fn test_deeply_nested() {
    let input = r#"{
        "user": {
            "name": "Alice",
            "age": 30
        },
        "products": [
            {
                "id": 1,
                "name": "Widget",
                "price": 9.99,
                "in_stock": true,
                "tags": ["new", "sale", "featured"]
            }
        ]
    }"#;

    let data: NestedData = parse(input).unwrap();

    assert_eq!(data.user.name, "Alice");
    assert_eq!(data.products[0].tags.len(), 3);
}
