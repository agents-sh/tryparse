//! Core traits for custom LLM-aware deserialization.
//!
//! This module defines the fundamental traits and context for deserializing
//! LLM responses with sophisticated coercion logic ported from BAML.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::{constraints::ConstraintResults, error::Result, value::FlexValue};

/// Default maximum recursion depth for deserialization.
/// Matches BAML's limit to prevent stack overflow.
pub const DEFAULT_MAX_DEPTH: usize = 100;

/// Shared state that doesn't need to be cloned when entering scopes.
#[derive(Debug)]
struct SharedState {
    /// Track visited (type_name, value) pairs during strict matching (try_deserialize)
    visited_for_strict: HashSet<(String, FlexValue)>,
    /// Track visited (type_name, value) pairs during lenient matching (deserialize)
    visited_for_lenient: HashSet<(String, FlexValue)>,
    /// Constraint validation results
    constraints: ConstraintResults,
    /// Transformations applied during deserialization
    transformations: Vec<crate::value::Transformation>,
}

/// Context for deserialization with circular reference tracking.
///
/// This context maintains state during deserialization to detect and prevent
/// infinite loops when dealing with recursive types.
///
/// Uses BAML's approach: create new contexts when entering types rather than guards.
///
/// Performance optimization: Shared state (visited sets, transformations) uses
/// `Rc<RefCell<>>` to avoid expensive cloning on every scope entry.
#[derive(Debug, Clone)]
pub struct CoercionContext {
    /// Shared mutable state (visited sets, constraints, transformations)
    shared: Rc<RefCell<SharedState>>,
    /// Current nesting depth (cloned per scope)
    depth: usize,
    /// Maximum allowed depth
    max_depth: usize,
    /// Scope trail for error messages (e.g., ["root", "user", "address", "street"])
    scope: Vec<String>,
}

impl CoercionContext {
    /// Creates a new coercion context with default settings.
    ///
    /// Maximum depth is set to `DEFAULT_MAX_DEPTH`, matching BAML's limit.
    pub fn new() -> Self {
        Self {
            shared: Rc::new(RefCell::new(SharedState {
                visited_for_strict: HashSet::new(),
                visited_for_lenient: HashSet::new(),
                constraints: ConstraintResults::new(),
                transformations: Vec::new(),
            })),
            depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
            scope: vec!["<root>".to_string()],
        }
    }

    /// Creates a context with a custom maximum depth.
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            shared: Rc::new(RefCell::new(SharedState {
                visited_for_strict: HashSet::new(),
                visited_for_lenient: HashSet::new(),
                constraints: ConstraintResults::new(),
                transformations: Vec::new(),
            })),
            depth: 0,
            max_depth,
            scope: vec!["<root>".to_string()],
        }
    }

    /// Enters a new scope (e.g., entering a field).
    ///
    /// Returns a new context with the updated scope trail.
    /// This is cheap - only clones the scope Vec and Rc pointer.
    pub fn enter_scope(&self, name: &str) -> Self {
        let mut new_scope = self.scope.clone();
        new_scope.push(name.to_string());
        Self {
            shared: Rc::clone(&self.shared),
            depth: self.depth,
            max_depth: self.max_depth,
            scope: new_scope,
        }
    }

    /// Returns the current scope as a dotted path.
    ///
    /// Example: `<root>.user.address.street`
    pub fn scope_path(&self) -> String {
        self.scope.join(".")
    }

    /// Returns a reference to the scope trail.
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    /// Adds a constraint validation result.
    ///
    /// This allows tracking both @assert and @check constraints during deserialization.
    pub fn add_constraint(&mut self, result: crate::constraints::ConstraintResult) {
        self.shared.borrow_mut().constraints.add(result);
    }

    /// Returns all constraint validation results.
    pub fn constraints(&self) -> ConstraintResults {
        self.shared.borrow().constraints.clone()
    }

    /// Returns true if all assert-level constraints passed.
    ///
    /// Check-level constraints do not affect this result.
    pub fn all_asserts_passed(&self) -> bool {
        self.shared.borrow().constraints.all_asserts_passed()
    }

    /// Returns all failing assert-level constraints.
    pub fn failing_asserts(&self) -> Vec<crate::constraints::ConstraintResult> {
        self.shared
            .borrow()
            .constraints
            .failing_asserts()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Adds a transformation that occurred during deserialization.
    ///
    /// This allows tracking what modifications were made to the input data.
    pub fn add_transformation(&mut self, transformation: crate::value::Transformation) {
        self.shared
            .borrow_mut()
            .transformations
            .push(transformation);
    }

    /// Returns all transformations applied during deserialization.
    pub fn transformations(&self) -> Vec<crate::value::Transformation> {
        self.shared.borrow().transformations.clone()
    }

    /// Takes all transformations, leaving an empty vector.
    ///
    /// This is useful for moving transformations into a FlexValue after deserialization.
    pub fn take_transformations(&mut self) -> Vec<crate::value::Transformation> {
        std::mem::take(&mut self.shared.borrow_mut().transformations)
    }

    /// Checks if we can enter a type for strict matching.
    ///
    /// Returns an error if this would exceed the depth limit or create a cycle.
    pub fn check_can_enter_strict(&self, type_name: &str, value: &FlexValue) -> Result<()> {
        if self.depth >= self.max_depth {
            return Err(crate::error::ParseError::DeserializeFailed(
                crate::error::DeserializeError::DepthLimitExceeded {
                    depth: self.depth,
                    max_depth: self.max_depth,
                },
            ));
        }

        let pair = (type_name.to_string(), value.clone());

        if self.shared.borrow().visited_for_strict.contains(&pair) {
            return Err(crate::error::ParseError::DeserializeFailed(
                crate::error::DeserializeError::CircularReference {
                    type_name: type_name.to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Creates a new context with the given type/value pair marked as visited (strict mode).
    ///
    /// This matches BAML's approach of creating a new context instead of mutating.
    /// The visited set is shared, so marking as visited affects all contexts sharing this state.
    pub fn with_visited_strict(&self, type_name: &str, value: &FlexValue) -> Self {
        self.shared
            .borrow_mut()
            .visited_for_strict
            .insert((type_name.to_string(), value.clone()));

        Self {
            shared: Rc::clone(&self.shared),
            depth: self.depth + 1,
            max_depth: self.max_depth,
            scope: self.scope.clone(),
        }
    }

    /// Checks if we can enter a type for lenient matching.
    ///
    /// Returns an error if this would exceed the depth limit or create a cycle.
    pub fn check_can_enter_lenient(&self, type_name: &str, value: &FlexValue) -> Result<()> {
        if self.depth >= self.max_depth {
            return Err(crate::error::ParseError::DeserializeFailed(
                crate::error::DeserializeError::DepthLimitExceeded {
                    depth: self.depth,
                    max_depth: self.max_depth,
                },
            ));
        }

        let pair = (type_name.to_string(), value.clone());

        if self.shared.borrow().visited_for_lenient.contains(&pair) {
            return Err(crate::error::ParseError::DeserializeFailed(
                crate::error::DeserializeError::CircularReference {
                    type_name: type_name.to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Creates a new context with the given type/value pair marked as visited (lenient mode).
    ///
    /// This matches BAML's approach of creating a new context instead of mutating.
    /// The visited set is shared, so marking as visited affects all contexts sharing this state.
    pub fn with_visited_lenient(&self, type_name: &str, value: &FlexValue) -> Self {
        self.shared
            .borrow_mut()
            .visited_for_lenient
            .insert((type_name.to_string(), value.clone()));

        Self {
            shared: Rc::clone(&self.shared),
            depth: self.depth + 1,
            max_depth: self.max_depth,
            scope: self.scope.clone(),
        }
    }

    /// Returns the current depth.
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

impl Default for CoercionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Main trait for LLM-aware deserialization.
///
/// This trait provides two-mode deserialization:
/// 1. `try_deserialize` - Strict, fast-path matching (exact types only)
/// 2. `deserialize` - Lenient matching with type coercion
///
/// This matches BAML's `TypeCoercer` trait with `try_cast` and `coerce`.
pub trait LlmDeserialize: Sized {
    /// Attempts strict deserialization without coercion.
    ///
    /// This is the fast path that only succeeds if the value is already
    /// the correct type. No transformations are applied.
    ///
    /// Returns `None` if strict matching fails.
    fn try_deserialize(_value: &FlexValue, _ctx: &mut CoercionContext) -> Option<Self> {
        None
    }

    /// Deserializes with lenient matching and type coercion.
    ///
    /// This is the fallback path that applies transformations to convert
    /// between compatible types (e.g., string to number, array unwrapping).
    ///
    /// Returns an error if the value cannot be coerced to this type.
    fn deserialize(value: &FlexValue, ctx: &mut CoercionContext) -> Result<Self>;

    /// Helper to get the type name for error messages and circular detection.
    ///
    /// Default implementation uses `std::any::type_name`.
    fn type_name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::value::{FlexValue, Source};

    #[test]
    fn test_context_depth_limit() {
        let ctx = CoercionContext::with_max_depth(3);
        let v1 = FlexValue::new(json!(1), Source::Direct);
        let v2 = FlexValue::new(json!(2), Source::Direct);
        let v3 = FlexValue::new(json!(3), Source::Direct);
        let v4 = FlexValue::new(json!(4), Source::Direct);

        let ctx1 = ctx.with_visited_strict("T1", &v1);
        assert_eq!(ctx1.depth(), 1);

        let ctx2 = ctx1.with_visited_strict("T2", &v2);
        assert_eq!(ctx2.depth(), 2);

        let ctx3 = ctx2.with_visited_strict("T3", &v3);
        assert_eq!(ctx3.depth(), 3);

        // Should fail - depth limit reached
        let result = ctx3.check_can_enter_strict("T4", &v4);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_circular_detection_strict() {
        let ctx = CoercionContext::new();
        let value = FlexValue::new(json!({"recursive": true}), Source::Direct);

        let ctx1 = ctx.with_visited_strict("Node", &value);

        // Try to enter same type with same value again - should fail
        let result = ctx1.check_can_enter_strict("Node", &value);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_circular_detection_lenient() {
        let ctx = CoercionContext::new();
        let value = FlexValue::new(json!({"recursive": true}), Source::Direct);

        let ctx1 = ctx.with_visited_lenient("Node", &value);

        // Try to enter same type with same value again - should fail
        let result = ctx1.check_can_enter_lenient("Node", &value);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_cloning() {
        let ctx = CoercionContext::new();
        let value = FlexValue::new(json!(1), Source::Direct);

        let ctx1 = ctx.with_visited_strict("T", &value);
        assert_eq!(ctx1.depth(), 1);

        // Original context should be unchanged
        assert_eq!(ctx.depth(), 0);

        // Should be able to create another context from original
        let ctx2 = ctx.with_visited_strict("T", &value);
        assert_eq!(ctx2.depth(), 1);
    }

    #[test]
    fn test_separate_strict_lenient_tracking() {
        let ctx = CoercionContext::new();
        let value = FlexValue::new(json!(1), Source::Direct);

        // Enter in strict mode
        let ctx_strict = ctx.with_visited_strict("T", &value);

        // Should be able to enter same pair in lenient mode from original context
        let ctx_lenient = ctx.with_visited_lenient("T", &value);

        assert_eq!(ctx_strict.depth(), 1);
        assert_eq!(ctx_lenient.depth(), 1);
    }
}
