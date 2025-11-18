//! Integration tests for constraint validation system.
//!
//! These tests demonstrate how to use constraints during deserialization
//! to validate values beyond just type checking.

use serde_json::json;
use tryparse::{
    constraints::{Constraint, ConstraintLevel, ConstraintResults},
    deserializer::{CoercionContext, LlmDeserialize},
    value::{FlexValue, Source},
};

/// Example struct for testing constraints.
#[derive(Debug, PartialEq)]
struct User {
    name: String,
    age: i64,
}

/// Manual implementation showing constraint integration.
impl LlmDeserialize for User {
    fn deserialize(value: &FlexValue, ctx: &mut CoercionContext) -> tryparse::error::Result<Self> {
        // Extract fields
        let obj = value.value.as_object().ok_or_else(|| {
            tryparse::error::ParseError::DeserializeFailed(
                tryparse::error::DeserializeError::TypeMismatch {
                    expected: "object",
                    found: value.value.to_string(),
                },
            )
        })?;

        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                tryparse::error::ParseError::DeserializeFailed(
                    tryparse::error::DeserializeError::MissingField {
                        field: "name".to_string(),
                    },
                )
            })?
            .to_string();

        let age = obj.get("age").and_then(|v| v.as_i64()).ok_or_else(|| {
            tryparse::error::ParseError::DeserializeFailed(
                tryparse::error::DeserializeError::MissingField {
                    field: "age".to_string(),
                },
            )
        })?;

        // CONSTRAINT VALIDATION: Age must be positive
        let age_positive = Constraint::assert("age_positive", "age must be greater than 0");
        let result = age_positive.validate(age > 0);
        ctx.add_constraint(result.clone());

        // If it's a failing assert, stop deserialization
        if result.is_failing_assert() {
            return Err(tryparse::error::ParseError::DeserializeFailed(
                tryparse::error::DeserializeError::Custom(format!(
                    "Constraint '{}' failed: {}",
                    age_positive.name, age_positive.description
                )),
            ));
        }

        // CONSTRAINT VALIDATION: Name must not be empty (check only)
        let name_not_empty = Constraint::check("name_not_empty", "name should not be empty");
        let result = name_not_empty.validate(!name.is_empty());
        ctx.add_constraint(result);
        // Note: check-level constraints don't fail deserialization

        Ok(User { name, age })
    }
}

#[test]
fn test_constraint_assert_passes() {
    let value = FlexValue::new(
        json!({
            "name": "Alice",
            "age": 30
        }),
        Source::Direct,
    );

    let mut ctx = CoercionContext::new();
    let result = User::deserialize(&value, &mut ctx);

    assert!(result.is_ok(), "Deserialization should succeed");
    let user = result.unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 30);

    // Check constraints were tracked
    assert_eq!(ctx.constraints().len(), 2);
    assert!(ctx.all_asserts_passed());
}

#[test]
fn test_constraint_assert_fails() {
    let value = FlexValue::new(
        json!({
            "name": "Bob",
            "age": -5
        }),
        Source::Direct,
    );

    let mut ctx = CoercionContext::new();
    let result = User::deserialize(&value, &mut ctx);

    // Should fail because age is negative (fails assert)
    assert!(
        result.is_err(),
        "Deserialization should fail for negative age"
    );

    // Constraint was tracked even though it failed
    assert_eq!(ctx.constraints().len(), 1);
    assert!(!ctx.all_asserts_passed());

    let failing = ctx.failing_asserts();
    assert_eq!(failing.len(), 1);
    assert_eq!(failing[0].constraint.name, "age_positive");
}

#[test]
fn test_constraint_check_does_not_fail_deserialization() {
    let value = FlexValue::new(
        json!({
            "name": "",  // Empty name
            "age": 25
        }),
        Source::Direct,
    );

    let mut ctx = CoercionContext::new();
    let result = User::deserialize(&value, &mut ctx);

    // Should succeed even though name is empty (it's a check, not an assert)
    assert!(
        result.is_ok(),
        "Deserialization should succeed for empty name"
    );
    let user = result.unwrap();
    assert_eq!(user.name, "");
    assert_eq!(user.age, 25);

    // Both constraints were tracked
    assert_eq!(ctx.constraints().len(), 2);

    // Assert passed, check failed
    assert!(ctx.all_asserts_passed());

    // Can inspect check results
    let constraints = ctx.constraints();
    let checks = constraints.checks();
    assert_eq!(checks.len(), 1);
    assert!(!checks[0].passed());
    assert_eq!(checks[0].constraint.name, "name_not_empty");
}

#[test]
fn test_constraint_results_display() {
    let mut results = ConstraintResults::new();

    results.add(Constraint::assert("age_positive", "age must be > 0").validate(true));
    results.add(Constraint::check("name_not_empty", "name should not be empty").validate(false));

    let display = format!("{}", results);
    assert!(display.contains("2 total"));
    assert!(display.contains("age_positive"));
    assert!(display.contains("name_not_empty"));
    assert!(display.contains("PASS"));
    assert!(display.contains("FAIL"));
}

#[test]
fn test_constraint_levels() {
    let assert_constraint = Constraint::assert("test_assert", "must pass");
    assert_eq!(assert_constraint.level, ConstraintLevel::Assert);
    assert!(assert_constraint.is_assert());
    assert!(!assert_constraint.is_check());

    let check_constraint = Constraint::check("test_check", "should pass");
    assert_eq!(check_constraint.level, ConstraintLevel::Check);
    assert!(check_constraint.is_check());
    assert!(!check_constraint.is_assert());
}

#[test]
fn test_multiple_failing_asserts() {
    // Example with multiple assert constraints
    #[derive(Debug)]
    #[allow(dead_code)] // Fields are deserialized but not directly accessed in test
    struct Product {
        name: String,
        price: f64,
        quantity: i64,
    }

    impl LlmDeserialize for Product {
        fn deserialize(
            value: &FlexValue,
            ctx: &mut CoercionContext,
        ) -> tryparse::error::Result<Self> {
            let obj = value.value.as_object().ok_or_else(|| {
                tryparse::error::ParseError::DeserializeFailed(
                    tryparse::error::DeserializeError::TypeMismatch {
                        expected: "object",
                        found: value.value.to_string(),
                    },
                )
            })?;

            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let price = obj.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let quantity = obj.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);

            // Multiple constraints
            let price_positive = Constraint::assert("price_positive", "price must be > 0");
            let price_result = price_positive.validate(price > 0.0);
            ctx.add_constraint(price_result.clone());

            let qty_positive = Constraint::assert("qty_positive", "quantity must be >= 0");
            let qty_result = qty_positive.validate(quantity >= 0);
            ctx.add_constraint(qty_result.clone());

            // Check if any asserts failed
            if !ctx.all_asserts_passed() {
                let failing = ctx.failing_asserts();
                let names: Vec<_> = failing.iter().map(|r| &r.constraint.name).collect();
                return Err(tryparse::error::ParseError::DeserializeFailed(
                    tryparse::error::DeserializeError::Custom(format!(
                        "Constraints failed: {:?}",
                        names
                    )),
                ));
            }

            Ok(Product {
                name,
                price,
                quantity,
            })
        }
    }

    let value = FlexValue::new(
        json!({
            "name": "Widget",
            "price": -10.0,    // Invalid
            "quantity": -5     // Invalid
        }),
        Source::Direct,
    );

    let mut ctx = CoercionContext::new();
    let result = Product::deserialize(&value, &mut ctx);

    assert!(
        result.is_err(),
        "Should fail with multiple constraint violations"
    );

    // Both asserts should have been tracked
    let failing = ctx.failing_asserts();
    assert_eq!(failing.len(), 2);

    let names: Vec<_> = failing.iter().map(|r| r.constraint.name.as_str()).collect();
    assert!(names.contains(&"price_positive"));
    assert!(names.contains(&"qty_positive"));
}

#[test]
fn test_constraint_transformation_tracking() {
    use tryparse::value::Transformation;

    let mut value = FlexValue::new(json!(42), Source::Direct);

    // Simulate adding a constraint check as a transformation
    value.add_transformation(Transformation::ConstraintChecked {
        name: "value_in_range".to_string(),
        passed: true,
        is_assert: false,
    });

    // Check that the transformation was recorded
    let transformations = value.transformations();
    assert_eq!(transformations.len(), 1);

    match &transformations[0] {
        Transformation::ConstraintChecked {
            name,
            passed,
            is_assert,
        } => {
            assert_eq!(name, "value_in_range");
            assert!(passed);
            assert!(!is_assert);
        }
        _ => panic!("Expected ConstraintChecked transformation"),
    }
}

#[test]
fn test_constraint_penalty_scores() {
    use tryparse::value::Transformation;

    // Passing constraints have no penalty
    let passing_check = Transformation::ConstraintChecked {
        name: "test".to_string(),
        passed: true,
        is_assert: false,
    };
    assert_eq!(passing_check.penalty(), 0);

    let passing_assert = Transformation::ConstraintChecked {
        name: "test".to_string(),
        passed: true,
        is_assert: true,
    };
    assert_eq!(passing_assert.penalty(), 0);

    // Failed checks have moderate penalty
    let failed_check = Transformation::ConstraintChecked {
        name: "test".to_string(),
        passed: false,
        is_assert: false,
    };
    assert_eq!(failed_check.penalty(), 10);

    // Failed asserts have very high penalty
    let failed_assert = Transformation::ConstraintChecked {
        name: "test".to_string(),
        passed: false,
        is_assert: true,
    };
    assert_eq!(failed_assert.penalty(), 100);
}

#[test]
fn test_constraint_context_scope_tracking() {
    let ctx = CoercionContext::new();
    assert_eq!(ctx.scope_path(), "<root>");

    let ctx2 = ctx.enter_scope("user");
    assert_eq!(ctx2.scope_path(), "<root>.user");

    let ctx3 = ctx2.enter_scope("address");
    assert_eq!(ctx3.scope_path(), "<root>.user.address");

    // Can still add constraints at nested scopes
    let mut ctx3_mut = ctx3;
    let constraint = Constraint::check("city_valid", "city should be set");
    ctx3_mut.add_constraint(constraint.validate(true));

    assert_eq!(ctx3_mut.constraints().len(), 1);
}
