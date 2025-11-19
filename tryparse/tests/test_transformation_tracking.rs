//! Comprehensive tests for transformation tracking across all deserializers.

#[cfg(feature = "derive")]
use serde::Deserialize;
#[cfg(feature = "derive")]
use tryparse::parse_llm_with_candidates;
#[cfg(feature = "derive")]
use tryparse_derive::LlmDeserialize;

/// Test array-to-struct coercion with FirstMatch transformations.
#[cfg(feature = "derive")]
#[test]
fn test_array_to_struct_with_transformations() {
    #[derive(Debug, Deserialize, LlmDeserialize, PartialEq)]
    struct Person {
        name: String,
        age: i64,
    }

    // Array should be coerced to struct positionally
    let input = r#"["Alice", 30]"#;
    let (person, candidates) = parse_llm_with_candidates::<Person>(input).unwrap();

    assert_eq!(person.name, "Alice");
    assert_eq!(person.age, 30);

    // Check transformations
    let winner = &candidates[0];
    let transformations = winner.transformations();

    // Should have FirstMatch transformations for each field
    let first_matches: Vec<_> = transformations
        .iter()
        .filter(|t| matches!(t, tryparse::value::Transformation::FirstMatch { .. }))
        .collect();

    assert_eq!(
        first_matches.len(),
        2,
        "Should have 2 FirstMatch transformations"
    );

    // Test explanation_json
    let explanation = winner.explanation_json();
    assert!(explanation["transformations"].is_array());
    assert!(explanation["score"].is_number());
    assert_eq!(explanation["transformation_count"], 2);
}

/// Test array-to-struct with optional fields.
#[cfg(feature = "derive")]
#[test]
fn test_array_to_struct_with_optional_fields() {
    #[derive(Debug, Deserialize, LlmDeserialize, PartialEq)]
    struct PersonWithOptional {
        name: String,
        age: i64,
        #[serde(default)]
        city: Option<String>,
    }

    // Array with only required fields
    let input = r#"["Bob", 25]"#;
    let (person, candidates) = parse_llm_with_candidates::<PersonWithOptional>(input).unwrap();

    assert_eq!(person.name, "Bob");
    assert_eq!(person.age, 25);
    assert_eq!(person.city, None);

    // Check transformations
    let winner = &candidates[0];
    let transformations = winner.transformations();

    // Should have DefaultValueInserted for city
    let defaults: Vec<_> = transformations
        .iter()
        .filter(|t| {
            matches!(
                t,
                tryparse::value::Transformation::DefaultValueInserted { .. }
            )
        })
        .collect();

    assert_eq!(
        defaults.len(),
        1,
        "Should have 1 DefaultValueInserted transformation"
    );
}

/// Test union transformation tracking.
#[cfg(feature = "derive")]
#[test]
fn test_union_transformation_tracking() {
    #[derive(Debug, serde::Deserialize, LlmDeserialize, PartialEq)]
    #[llm(union)] // Use tryparse union attribute, not serde's untagged
    enum StringOrNumber {
        String(String),
        Number(i64),
    }

    // String should match without coercion
    let input = r#""hello""#;
    let (value, candidates) = parse_llm_with_candidates::<StringOrNumber>(input).unwrap();

    assert!(matches!(value, StringOrNumber::String(_)));

    // Check transformations
    let winner = &candidates[0];
    let transformations = winner.transformations();

    // Should have UnionMatch transformation
    let union_matches: Vec<_> = transformations
        .iter()
        .filter(|t| matches!(t, tryparse::value::Transformation::UnionMatch { .. }))
        .collect();

    assert_eq!(
        union_matches.len(),
        1,
        "Should have 1 UnionMatch transformation"
    );
}

/// Test explanation_json output format.
#[cfg(feature = "derive")]
#[test]
fn test_explanation_json_format() {
    #[derive(Debug, Deserialize, LlmDeserialize)]
    struct User {
        name: String,
        age: i64,
    }

    // Input with type coercion (age as string)
    let input = r#"{"name": "Charlie", "age": "35"}"#;
    let (user, candidates) = parse_llm_with_candidates::<User>(input).unwrap();

    assert_eq!(user.name, "Charlie");
    assert_eq!(user.age, 35);

    // Get explanation
    let winner = &candidates[0];
    let explanation = winner.explanation_json();

    // Verify structure
    assert!(explanation.is_object(), "Should be a JSON object");
    assert!(explanation["source"].is_object(), "Should have source");
    assert!(
        explanation["confidence"].is_number(),
        "Should have confidence"
    );
    assert!(explanation["score"].is_number(), "Should have score");
    assert!(
        explanation["transformations"].is_array(),
        "Should have transformations array"
    );
    assert!(
        explanation["transformation_count"].is_number(),
        "Should have transformation_count"
    );
    assert!(
        explanation["max_transformation_depth"].is_number(),
        "Should have max_transformation_depth"
    );

    // Verify source type
    assert_eq!(explanation["source"]["type"], "direct");

    // Verify transformations have correct format
    let transformations = explanation["transformations"].as_array().unwrap();
    for transformation in transformations {
        assert!(
            transformation.is_object(),
            "Each transformation should be an object"
        );
        assert!(
            transformation["type"].is_string(),
            "Each transformation should have a type"
        );
        assert!(
            transformation["penalty"].is_number(),
            "Each transformation should have a penalty"
        );
    }
}

/// Test percentage parsing with transformation tracking.
#[cfg(feature = "derive")]
#[test]
#[ignore] // TODO: Primitive-level transformation tracking needs additional work
fn test_percentage_with_transformations() {
    #[derive(Debug, Deserialize, LlmDeserialize)]
    struct Stats {
        success_rate: f64,
        error_rate: f64,
    }

    let input = r#"{"success_rate": "95%", "error_rate": "5%"}"#;
    let (stats, candidates) = parse_llm_with_candidates::<Stats>(input).unwrap();

    // Percentage values should be parsed as-is (not divided by 100)
    assert_eq!(stats.success_rate, 95.0);
    assert_eq!(stats.error_rate, 5.0);

    // Check transformations
    let winner = &candidates[0];
    let transformations = winner.transformations();

    // Should have StringToNumber transformations
    let string_to_number: Vec<_> = transformations
        .iter()
        .filter(|t| matches!(t, tryparse::value::Transformation::StringToNumber { .. }))
        .collect();

    assert!(
        !string_to_number.is_empty(),
        "Should have StringToNumber transformations"
    );
}

/// Test DefaultButHadUnparseableValue transformation.
#[cfg(feature = "derive")]
#[test]
fn test_default_but_unparseable() {
    #[derive(Debug, Deserialize, LlmDeserialize)]
    struct Config {
        name: String,
        #[serde(default)]
        count: Option<i64>,
    }

    // Array with unparseable second element
    let input = r#"["test", "not-a-number"]"#;
    let (config, candidates) = parse_llm_with_candidates::<Config>(input).unwrap();

    assert_eq!(config.name, "test");
    assert_eq!(config.count, None); // Should use default due to parse error

    // Check transformations
    let winner = &candidates[0];
    let transformations = winner.transformations();

    // Should have DefaultButHadUnparseableValue transformation
    let defaults_with_unparseable: Vec<_> = transformations
        .iter()
        .filter(|t| {
            matches!(
                t,
                tryparse::value::Transformation::DefaultButHadUnparseableValue { .. }
            )
        })
        .collect();

    assert_eq!(
        defaults_with_unparseable.len(),
        1,
        "Should have 1 DefaultButHadUnparseableValue transformation"
    );

    // Verify explanation includes the error
    let explanation = winner.explanation_json();
    let transformations_json = explanation["transformations"].as_array().unwrap();
    let default_unparseable = transformations_json
        .iter()
        .find(|t| t["type"] == "default_but_had_unparseable_value");

    assert!(
        default_unparseable.is_some(),
        "Should have default_but_had_unparseable_value in explanation"
    );
    let trans = default_unparseable.unwrap();
    assert_eq!(trans["field"], "count");
    assert!(trans["error"].is_string());
    assert!(trans["value"].is_string());
}
