//! Struct deserialization with BAML's fuzzy field matching algorithms.
//!
//! Ported from `engine/baml-lib/jsonish/src/deserializer/coercer/ir_ref/coerce_class.rs`
//! and `match_string.rs`.

use std::any::Any;

use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;

use crate::{
    deserializer::traits::CoercionContext,
    error::{DeserializeError, ParseError, Result},
    value::{FlexValue, Transformation},
};

/// Field matcher with BAML's fuzzy matching strategies.
///
/// Implements multi-strategy matching:
/// 1. Exact match (case-sensitive)
/// 2. Unaccented match (handles Unicode: é → e, ß → ss, etc.)
/// 3. Punctuation-stripped match
/// 4. Case-insensitive match
/// 5. Substring matching (if enabled)
#[derive(Debug, Clone)]
pub struct FieldMatcher {
    /// Original expected field name
    pub expected: String,
    /// Allow substring matching (used for field names, not usually enabled)
    pub allow_substring: bool,
}

impl FieldMatcher {
    /// Creates a new field matcher.
    ///
    /// # Arguments
    /// * `expected` - The expected field name
    ///
    /// # Examples
    /// ```
    /// use tryparse::deserializer::struct_coercer::FieldMatcher;
    ///
    /// let matcher = FieldMatcher::new("user_name");
    /// ```
    pub fn new(expected: &str) -> Self {
        Self {
            expected: expected.to_string(),
            allow_substring: false,
        }
    }

    /// Enables substring matching.
    pub fn with_substring_match(mut self) -> Self {
        self.allow_substring = true;
        self
    }

    /// Find field in object with BAML's fuzzy matching algorithm.
    ///
    /// Port from `match_string.rs`: Multi-strategy matching with Unicode normalization.
    ///
    /// # Strategies (in order):
    /// 1. Exact case-sensitive match
    /// 2. Unaccented case-sensitive match (é→e, ß→ss)
    /// 3. Exact match after stripping punctuation
    /// 4. Case-insensitive match after stripping punctuation
    /// 5. Substring match (if enabled)
    ///
    /// # Examples
    /// ```
    /// use tryparse::deserializer::struct_coercer::FieldMatcher;
    /// use serde_json::json;
    ///
    /// let value = json!({"userName": "Alice"});
    /// let obj = value.as_object().unwrap();
    /// let matcher = FieldMatcher::new("user_name");
    ///
    /// // Should match even with different case/format
    /// assert!(matcher.find_in_object(obj).is_some());
    /// ```
    pub fn find_in_object<'a>(
        &self,
        obj: &'a Map<String, Value>,
    ) -> Option<(&'a String, &'a Value)> {
        // Generate variations of the expected name
        let expected_camel = to_camel_case(&self.expected);
        let expected_snake = to_snake_case(&self.expected);

        // Strategy 1: Exact case-sensitive match
        if let Some((k, v)) = obj.iter().find(|(k, _)| k.as_str() == self.expected) {
            return Some((k, v));
        }

        // Strategy 2: Case variations (camelCase ↔ snake_case)
        // Check if the key matches any variation of the expected name
        // OR if the expected name matches any variation of the key
        if let Some((k, v)) = obj.iter().find(|(k, _)| {
            let key = k.as_str();
            let key_camel = to_camel_case(key);
            let key_snake = to_snake_case(key);

            // Key matches expected variations
            key == expected_camel || key == expected_snake ||
            // Key variations match expected
            key_camel == self.expected || key_snake == self.expected ||
            // Key variations match expected variations
            key_camel == expected_camel || key_snake == expected_snake
        }) {
            return Some((k, v));
        }

        // Strategy 3: Unaccented case-sensitive match
        let unaccented_expected = remove_accents(&self.expected);
        if let Some((k, v)) = obj
            .iter()
            .find(|(k, _)| remove_accents(k) == unaccented_expected)
        {
            return Some((k, v));
        }

        // Strategy 4: Match after stripping punctuation (case-sensitive)
        let stripped_expected = strip_punctuation(&self.expected);
        if let Some((k, v)) = obj
            .iter()
            .find(|(k, _)| strip_punctuation(k) == stripped_expected)
        {
            return Some((k, v));
        }

        // Strategy 5: Case-insensitive match after stripping punctuation
        let stripped_lower_expected = stripped_expected.to_lowercase();
        if let Some((k, v)) = obj
            .iter()
            .find(|(k, _)| strip_punctuation(k).to_lowercase() == stripped_lower_expected)
        {
            return Some((k, v));
        }

        // Strategy 6: Substring match (only if enabled)
        if self.allow_substring {
            // Try to find any key that contains the expected name (case-insensitive)
            let lower_expected = self.expected.to_lowercase();
            if let Some((k, v)) = obj.iter().find(|(k, _)| {
                let lower_key = k.to_lowercase();
                lower_key.contains(&lower_expected) || lower_expected.contains(&lower_key)
            }) {
                return Some((k, v));
            }
        }

        None
    }

    /// Check if a key matches the expected field name.
    ///
    /// Uses the same fuzzy matching logic as `find_in_object`.
    pub fn matches(&self, key: &str) -> bool {
        // Create a temporary map to use find_in_object
        let mut temp_map = Map::new();
        temp_map.insert(key.to_string(), Value::Null);
        self.find_in_object(&temp_map).is_some()
    }
}

/// Convert snake_case to camelCase.
///
/// # Examples
/// ```
/// use tryparse::deserializer::struct_coercer::to_camel_case;
///
/// assert_eq!(to_camel_case("user_name"), "userName");
/// assert_eq!(to_camel_case("first_name"), "firstName");
/// ```
pub fn to_camel_case(s: &str) -> String {
    // Fast path: if no underscores, check if already camelCase
    if !s.contains('_') {
        // If first char is lowercase and no changes needed, could return as-is
        // but we need to ensure first char is lowercase
        let mut chars = s.chars();
        if let Some(first) = chars.next() {
            if first.is_lowercase() {
                return s.to_string();
            }
        }
    }

    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    let mut first_char = true;

    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
            first_char = false;
        } else if first_char {
            result.push(ch.to_ascii_lowercase());
            first_char = false;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Convert camelCase, kebab-case, and dot.notation to snake_case.
///
/// This normalization helps with field matching across different naming conventions.
///
/// # Examples
/// ```
/// use tryparse::deserializer::struct_coercer::to_snake_case;
///
/// assert_eq!(to_snake_case("userName"), "user_name");
/// assert_eq!(to_snake_case("firstName"), "first_name");
/// assert_eq!(to_snake_case("user-name"), "user_name");
/// assert_eq!(to_snake_case("user.name"), "user_name");
/// ```
pub fn to_snake_case(s: &str) -> String {
    // Fast path: if already snake_case (all lowercase, no dots/hyphens except underscores)
    let needs_conversion = s
        .chars()
        .any(|ch| ch.is_uppercase() || ch == '-' || ch == '.');
    if !needs_conversion {
        return s.to_string();
    }

    // Pre-allocate with extra space for potential underscores
    let mut result = String::with_capacity(s.len() + s.len() / 4);

    for ch in s.chars() {
        if ch.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '.' {
            // Convert hyphens and dots to underscores for normalization
            result.push('_');
        } else {
            result.push(ch);
        }
    }

    result
}

/// Apply serde rename_all transformation to a string.
///
/// Supports all serde rename_all cases:
/// - `snake_case` - Convert to snake_case
/// - `camelCase` - Convert to camelCase (first char lowercase)
/// - `PascalCase` - Convert to PascalCase (first char uppercase)
/// - `SCREAMING_SNAKE_CASE` - Convert to SCREAMING_SNAKE_CASE
/// - `kebab-case` - Convert to kebab-case
///
/// # Examples
/// ```
/// use tryparse::deserializer::struct_coercer::apply_rename_all;
///
/// assert_eq!(apply_rename_all("StartTask", "snake_case"), "start_task");
/// assert_eq!(apply_rename_all("StartTask", "camelCase"), "startTask");
/// assert_eq!(apply_rename_all("StartTask", "PascalCase"), "StartTask");
/// assert_eq!(apply_rename_all("StartTask", "kebab-case"), "start-task");
/// ```
pub fn apply_rename_all(s: &str, rule: &str) -> String {
    match rule {
        "snake_case" => to_snake_case(s),
        "camelCase" => to_camel_case(s),
        "PascalCase" => {
            // First char uppercase, rest unchanged
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
        "SCREAMING_SNAKE_CASE" => to_snake_case(s).to_uppercase(),
        "kebab-case" => to_snake_case(s).replace('_', "-"),
        _ => s.to_string(),
    }
}

/// Remove accents from characters to enable fuzzy matching.
///
/// Port from `match_string.rs:141-159`.
///
/// Handles:
/// - Combining diacritical marks (é → e, ñ → n)
/// - German ligatures (ß → ss)
/// - Nordic ligatures (æ → ae, ø → o)
/// - French ligatures (œ → oe)
///
/// # Examples
/// ```
/// use tryparse::deserializer::struct_coercer::remove_accents;
///
/// assert_eq!(remove_accents("café"), "cafe");
/// assert_eq!(remove_accents("Straße"), "Strasse");
/// assert_eq!(remove_accents("København"), "Kobenhavn");
/// ```
pub fn remove_accents(s: &str) -> String {
    // Fast path: if all ASCII, no accents to remove
    if s.is_ascii() {
        return s.to_string();
    }

    // Handle ligatures separately since they're not combining marks
    let s = s
        .replace('ß', "ss")
        .replace('æ', "ae")
        .replace('Æ', "AE")
        .replace('ø', "o")
        .replace('Ø', "O")
        .replace('œ', "oe")
        .replace('Œ', "OE");

    // Remove combining diacritical marks (é → e, ñ → n)
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Strip punctuation from a string, keeping only alphanumeric, hyphens, and underscores.
///
/// Port from `match_string.rs:135-139`.
///
/// # Examples
/// ```
/// use tryparse::deserializer::struct_coercer::strip_punctuation;
///
/// assert_eq!(strip_punctuation("user.name"), "username");
/// assert_eq!(strip_punctuation("first_name"), "first_name");
/// assert_eq!(strip_punctuation("user-id"), "user-id");
/// ```
pub fn strip_punctuation(s: &str) -> String {
    // Fast path: if all chars are already valid, return as-is
    let needs_stripping = s
        .chars()
        .any(|c| !c.is_alphanumeric() && c != '-' && c != '_');
    if !needs_stripping {
        return s.to_string();
    }

    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Metadata about a struct field for deserialization.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    /// Field name in the struct
    pub name: String,
    /// Type name for error messages
    pub type_name: &'static str,
    /// Whether this field is optional (Option<T>)
    pub is_optional: bool,
}

impl FieldDescriptor {
    /// Creates a new field descriptor.
    pub fn new(name: impl Into<String>, type_name: &'static str, is_optional: bool) -> Self {
        Self {
            name: name.into(),
            type_name,
            is_optional,
        }
    }
}

/// Helper for deserializing struct fields with BAML's two-mode coercion.
///
/// Port from `coerce_class.rs`.
///
/// This struct manages the deserialization of struct fields using BAML's algorithm:
/// 1. Try strict matching first (exact keys, no transformations)
/// 2. Fall back to lenient matching (fuzzy field names, type coercion)
/// 3. Handle optional vs required fields
/// 4. Track extra keys and implicit key coercion
pub struct StructDeserializer {
    /// Field descriptors
    fields: Vec<FieldDescriptor>,
    /// Transformations applied during deserialization
    transformations: Vec<Transformation>,
}

impl StructDeserializer {
    /// Creates a new struct deserializer with no fields.
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            transformations: Vec::new(),
        }
    }

    /// Adds a field descriptor.
    pub fn field(mut self, descriptor: FieldDescriptor) -> Self {
        self.fields.push(descriptor);
        self
    }

    /// Try strict deserialization (try_cast mode only).
    ///
    /// Attempts to deserialize using only strict mode:
    /// - Exact key matching (no fuzzy matching)
    /// - No extra keys allowed
    /// - Uses try_deserialize on fields
    ///
    /// Returns None if strict matching fails.
    pub fn try_deserialize<F>(
        &self,
        value: &FlexValue,
        ctx: &mut CoercionContext,
        type_name: &str,
        mut deserialize_fn: F,
    ) -> Result<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext) -> Option<Box<dyn Any>>,
    {
        // Must be an object for strict mode
        let obj = match &value.value {
            Value::Object(obj) => obj,
            _ => {
                return Err(ParseError::DeserializeFailed(
                    DeserializeError::type_mismatch("object", "non-object"),
                ));
            }
        };

        // Circular reference detection
        ctx.check_can_enter_strict(type_name, value)?;
        let mut nested_ctx = ctx.with_visited_strict(type_name, value);

        // Try strict matching only
        let mut result = std::collections::HashMap::new();

        // STRICT: Exact key match only
        for field in &self.fields {
            let value = obj.get(&field.name).ok_or_else(|| {
                ParseError::DeserializeFailed(DeserializeError::missing_field(&field.name))
            })?;

            let flex_value = FlexValue::new(value.clone(), crate::value::Source::Direct);

            // Try strict deserialization (try_deserialize)
            let field_value = deserialize_fn(&field.name, &flex_value, &mut nested_ctx)
                .ok_or_else(|| {
                    ParseError::DeserializeFailed(DeserializeError::type_mismatch(
                        field.type_name,
                        "value",
                    ))
                })?;

            result.insert(field.name.clone(), field_value);
        }

        // STRICT: No extra keys allowed
        if obj.len() != self.fields.len() {
            return Err(ParseError::DeserializeFailed(DeserializeError::Custom(
                "Extra fields not allowed in strict mode".to_string(),
            )));
        }

        Ok(result)
    }

    /// Deserialize struct with BAML's two-mode approach.
    ///
    /// Port from `coerce_class.rs:24-138` (try_cast) and `139-400` (coerce).
    ///
    /// # Arguments
    /// * `value` - The FlexValue to deserialize from (must be Object)
    /// * `ctx` - Coercion context for circular detection
    /// * `type_name` - Name of the struct type (for circular detection)
    /// * `deserialize_fn` - Function that deserializes each field by name
    ///
    /// # Returns
    /// A map of field name → deserialized value (as Box<dyn Any>)
    pub fn deserialize<F>(
        &mut self,
        value: &FlexValue,
        ctx: &mut CoercionContext,
        type_name: &str,
        mut deserialize_fn: F,
    ) -> Result<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext, bool) -> Result<Box<dyn Any>>,
    {
        // Must be an object
        let obj = match &value.value {
            Value::Object(obj) => obj,
            Value::Array(arr) => {
                // BAML ALGORITHM: For single-field structs, try to coerce array
                if self.fields.len() == 1 {
                    return self.try_single_field_coercion(value, ctx, type_name, deserialize_fn);
                }
                // BAML ALGORITHM: Try array-to-struct coercion
                // Match array elements to struct fields in order: [val1, val2] → {field1: val1, field2: val2}
                return self.try_array_to_struct_coercion(arr, ctx, type_name, deserialize_fn);
            }
            _ => {
                // BAML ALGORITHM: For single-field structs, try to coerce entire value
                if self.fields.len() == 1 {
                    return self.try_single_field_coercion(value, ctx, type_name, deserialize_fn);
                }
                return Err(ParseError::DeserializeFailed(
                    DeserializeError::type_mismatch("object", "non-object"),
                ));
            }
        };

        // BAML ALGORITHM: Circular reference detection
        ctx.check_can_enter_lenient(type_name, value)?;
        let mut nested_ctx = ctx.with_visited_lenient(type_name, value);

        // BAML ALGORITHM: Try strict matching first (try_cast)
        if let Some(result) = self.try_strict_match(obj, &mut nested_ctx, &mut deserialize_fn) {
            return Ok(result);
        }

        // BAML ALGORITHM: Fall back to lenient matching (coerce)
        self.try_lenient_match(obj, &mut nested_ctx, deserialize_fn)
    }

    /// Try strict matching (BAML's try_cast).
    ///
    /// - Exact key match only (no fuzzy matching)
    /// - Use try_deserialize (strict mode)
    /// - No extra keys allowed
    fn try_strict_match<F>(
        &self,
        obj: &Map<String, Value>,
        ctx: &mut CoercionContext,
        deserialize_fn: &mut F,
    ) -> Option<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext, bool) -> Result<Box<dyn Any>>,
    {
        use crate::value::Source;

        let mut result = std::collections::HashMap::new();

        // STRICT: Exact key match only
        for field in &self.fields {
            let value = obj.get(&field.name)?;
            let flex_value = FlexValue::new(value.clone(), Source::Direct);

            // Try strict deserialization (try_deserialize)
            let field_value = deserialize_fn(&field.name, &flex_value, ctx, true).ok()?;
            result.insert(field.name.clone(), field_value);
        }

        // STRICT: No extra keys allowed
        if obj.len() != self.fields.len() {
            return None;
        }

        Some(result)
    }

    /// Try lenient matching (BAML's coerce).
    ///
    /// - Fuzzy field matching
    /// - Use deserialize (lenient mode)
    /// - Handle optional fields with defaults
    /// - Track extra keys
    fn try_lenient_match<F>(
        &mut self,
        obj: &Map<String, Value>,
        ctx: &mut CoercionContext,
        mut deserialize_fn: F,
    ) -> Result<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext, bool) -> Result<Box<dyn Any>>,
    {
        use crate::value::Source;

        let mut result = std::collections::HashMap::new();
        let mut matched_keys = std::collections::HashSet::new();

        // LENIENT: Fuzzy field matching
        for field in &self.fields {
            let matcher = FieldMatcher::new(&field.name);

            match matcher.find_in_object(obj) {
                Some((actual_key, value)) => {
                    matched_keys.insert(actual_key.clone());

                    // Track field name transformation
                    if actual_key != &field.name {
                        self.transformations
                            .push(Transformation::FieldNameCaseChanged {
                                from: actual_key.clone(),
                                to: field.name.clone(),
                            });
                    }

                    let flex_value = FlexValue::new(value.clone(), Source::Direct);

                    // Deserialize with lenient mode
                    match deserialize_fn(&field.name, &flex_value, ctx, false) {
                        Ok(field_value) => {
                            result.insert(field.name.clone(), field_value);
                        }
                        Err(e) => {
                            if field.is_optional {
                                // Optional field - use None/default
                                let transformation = Transformation::DefaultValueInserted {
                                    field: field.name.clone(),
                                };
                                self.transformations.push(transformation.clone());
                                ctx.add_transformation(transformation);
                                // Caller should handle Option::None
                                continue;
                            } else {
                                // Required field - propagate error
                                return Err(e);
                            }
                        }
                    }
                }
                None => {
                    // Field not found
                    if field.is_optional {
                        // Optional field - use default
                        let transformation = Transformation::DefaultValueInserted {
                            field: field.name.clone(),
                        };
                        self.transformations.push(transformation.clone());
                        ctx.add_transformation(transformation);
                        // Caller should handle Option::None
                        continue;
                    } else {
                        // Required field missing
                        return Err(ParseError::DeserializeFailed(
                            DeserializeError::missing_field(&field.name),
                        ));
                    }
                }
            }
        }

        // BAML ALGORITHM: Track extra keys
        for (key, _value) in obj.iter() {
            if !matched_keys.contains(key) {
                let transformation = Transformation::ExtraKey { key: key.clone() };
                self.transformations.push(transformation.clone());
                ctx.add_transformation(transformation);
            }
        }

        Ok(result)
    }

    /// Try to coerce entire value into a single-field struct (BAML's implicit key logic).
    ///
    /// Port from `coerce_class.rs:224-310`.
    fn try_single_field_coercion<F>(
        &mut self,
        value: &FlexValue,
        ctx: &mut CoercionContext,
        _type_name: &str,
        mut deserialize_fn: F,
    ) -> Result<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext, bool) -> Result<Box<dyn Any>>,
    {
        assert_eq!(
            self.fields.len(),
            1,
            "Single field coercion requires exactly one field"
        );

        let field = &self.fields[0];

        // Try to deserialize the entire value as the single field
        match deserialize_fn(&field.name, value, ctx, false) {
            Ok(field_value) => {
                let transformation = Transformation::ImpliedKey {
                    field: field.name.clone(),
                };
                self.transformations.push(transformation.clone());
                ctx.add_transformation(transformation);

                let mut result = std::collections::HashMap::new();
                result.insert(field.name.clone(), field_value);
                Ok(result)
            }
            Err(e) => {
                if field.is_optional {
                    // Optional field - use default
                    let transformation = Transformation::DefaultValueInserted {
                        field: field.name.clone(),
                    };
                    self.transformations.push(transformation.clone());
                    ctx.add_transformation(transformation);
                    Ok(std::collections::HashMap::new())
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Try to coerce an array to a struct by matching elements to fields in order.
    ///
    /// BAML ALGORITHM: Array-to-struct coercion
    /// Example: ["Alice", 30] → User { name: "Alice", age: 30 }
    ///
    /// This tries to match array elements to struct fields positionally.
    /// - First array element → first field
    /// - Second array element → second field
    /// - etc.
    ///
    /// Adds FirstMatch transformation to track this coercion.
    fn try_array_to_struct_coercion<F>(
        &mut self,
        arr: &[Value],
        ctx: &mut CoercionContext,
        _type_name: &str,
        mut deserialize_fn: F,
    ) -> Result<std::collections::HashMap<String, Box<dyn Any>>>
    where
        F: FnMut(&str, &FlexValue, &mut CoercionContext, bool) -> Result<Box<dyn Any>>,
    {
        use crate::value::Source;

        // Must have at least as many elements as required fields
        let required_count = self.fields.iter().filter(|f| !f.is_optional).count();
        if arr.len() < required_count {
            return Err(ParseError::DeserializeFailed(DeserializeError::Custom(
                format!(
                    "Array has {} elements but struct requires {} fields",
                    arr.len(),
                    required_count
                ),
            )));
        }

        let mut result = std::collections::HashMap::new();

        // Try to match each array element to the corresponding field
        for (index, field) in self.fields.iter().enumerate() {
            if index >= arr.len() {
                // No more array elements - field must be optional
                if field.is_optional {
                    let transformation = Transformation::DefaultValueInserted {
                        field: field.name.clone(),
                    };
                    self.transformations.push(transformation.clone());
                    ctx.add_transformation(transformation);
                    continue;
                } else {
                    return Err(ParseError::DeserializeFailed(DeserializeError::Custom(
                        format!(
                            "Required field '{}' missing - array only has {} elements",
                            field.name,
                            arr.len()
                        ),
                    )));
                }
            }

            let element = &arr[index];
            let flex_value = FlexValue::new(element.clone(), Source::Direct);

            // Try to deserialize this element as the field
            match deserialize_fn(&field.name, &flex_value, ctx, false) {
                Ok(field_value) => {
                    // Add FirstMatch transformation to track positional matching
                    let transformation = Transformation::FirstMatch {
                        index,
                        total: arr.len(),
                    };
                    self.transformations.push(transformation.clone());
                    ctx.add_transformation(transformation);

                    result.insert(field.name.clone(), field_value);
                }
                Err(e) => {
                    if field.is_optional {
                        // Optional field failed to parse - use default
                        let transformation = Transformation::DefaultButHadUnparseableValue {
                            field: field.name.clone(),
                            value: element.to_string(),
                            error: e.to_string(),
                        };
                        self.transformations.push(transformation.clone());
                        ctx.add_transformation(transformation);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Returns the transformations applied during deserialization.
    pub fn transformations(&self) -> &[Transformation] {
        &self.transformations
    }

    /// Consumes self and returns the transformations.
    pub fn into_transformations(self) -> Vec<Transformation> {
        self.transformations
    }
}

impl Default for StructDeserializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_remove_accents_basic() {
        assert_eq!(remove_accents("café"), "cafe");
        assert_eq!(remove_accents("naïve"), "naive");
        assert_eq!(remove_accents("résumé"), "resume");
    }

    #[test]
    fn test_remove_accents_german() {
        assert_eq!(remove_accents("Straße"), "Strasse");
        assert_eq!(remove_accents("Grün"), "Grun");
        assert_eq!(remove_accents("Über"), "Uber");
    }

    #[test]
    fn test_remove_accents_nordic() {
        assert_eq!(remove_accents("æ"), "ae");
        assert_eq!(remove_accents("Æ"), "AE");
        assert_eq!(remove_accents("ø"), "o");
        assert_eq!(remove_accents("Ø"), "O");
        assert_eq!(remove_accents("København"), "Kobenhavn");
    }

    #[test]
    fn test_remove_accents_french() {
        assert_eq!(remove_accents("œ"), "oe");
        assert_eq!(remove_accents("Œ"), "OE");
        assert_eq!(remove_accents("cœur"), "coeur");
        assert_eq!(remove_accents("œuvre"), "oeuvre");
    }

    #[test]
    fn test_strip_punctuation() {
        assert_eq!(strip_punctuation("user.name"), "username");
        assert_eq!(strip_punctuation("first_name"), "first_name");
        assert_eq!(strip_punctuation("user-id"), "user-id");
        assert_eq!(strip_punctuation("email@address"), "emailaddress");
        // Note: spaces are also removed (not alphanumeric, -, or _)
        assert_eq!(strip_punctuation("hello, world!"), "helloworld");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("user_name"), "userName");
        assert_eq!(to_camel_case("first_name"), "firstName");
        assert_eq!(to_camel_case("email_address"), "emailAddress");
        assert_eq!(to_camel_case("a_b_c"), "aBC");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("userName"), "user_name");
        assert_eq!(to_snake_case("firstName"), "first_name");
        assert_eq!(to_snake_case("emailAddress"), "email_address");
        assert_eq!(to_snake_case("ABC"), "a_b_c");
    }

    #[test]
    fn test_field_matcher_exact_match() {
        let obj = json!({"user_name": "Alice"}).as_object().unwrap().clone();
        let matcher = FieldMatcher::new("user_name");

        let result = matcher.find_in_object(&obj);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "user_name");
        assert_eq!(value, &json!("Alice"));
    }

    #[test]
    fn test_field_matcher_case_insensitive() {
        let obj = json!({"UserName": "Alice"}).as_object().unwrap().clone();
        let matcher = FieldMatcher::new("user_name");

        let result = matcher.find_in_object(&obj);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "UserName");
        assert_eq!(value, &json!("Alice"));
    }

    #[test]
    fn test_field_matcher_with_punctuation() {
        // Test that hyphens and dots are normalized to underscores for matching
        let obj = json!({"user-name": "Alice"}).as_object().unwrap().clone();
        let matcher = FieldMatcher::new("user_name");

        // "user-name" should match "user_name" because to_snake_case normalizes hyphens
        let result = matcher.find_in_object(&obj);
        assert!(result.is_some(), "kebab-case should match snake_case");
        let (key, value) = result.unwrap();
        assert_eq!(key, "user-name");
        assert_eq!(value, &json!("Alice"));

        // Test dot notation matching
        let obj3 = json!({"user.name": "Charlie"}).as_object().unwrap().clone();
        let result3 = matcher.find_in_object(&obj3);
        assert!(result3.is_some(), "dot.notation should match snake_case");
        let (key3, value3) = result3.unwrap();
        assert_eq!(key3, "user.name");
        assert_eq!(value3, &json!("Charlie"));

        // And exact matches still work
        let obj2 = json!({"user_name": "Bob"}).as_object().unwrap().clone();
        let result2 = matcher.find_in_object(&obj2);
        assert!(result2.is_some());
    }

    #[test]
    fn test_field_matcher_with_accents() {
        let obj = json!({"café": "Espresso"}).as_object().unwrap().clone();
        let matcher = FieldMatcher::new("cafe");

        let result = matcher.find_in_object(&obj);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "café");
        assert_eq!(value, &json!("Espresso"));
    }

    #[test]
    fn test_field_matcher_no_match() {
        let obj = json!({"first_name": "Alice"}).as_object().unwrap().clone();
        let matcher = FieldMatcher::new("last_name");

        let result = matcher.find_in_object(&obj);
        assert!(result.is_none());
    }

    #[test]
    fn test_field_matcher_substring_disabled_by_default() {
        let obj = json!({"user_name_extra": "Alice"})
            .as_object()
            .unwrap()
            .clone();
        let matcher = FieldMatcher::new("user_name");

        // Should NOT match because substring is disabled
        let result = matcher.find_in_object(&obj);
        assert!(result.is_none());
    }

    #[test]
    fn test_field_matcher_substring_enabled() {
        let obj = json!({"user_name_extra": "Alice"})
            .as_object()
            .unwrap()
            .clone();
        let matcher = FieldMatcher::new("user_name").with_substring_match();

        // Should match because substring is enabled
        let result = matcher.find_in_object(&obj);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "user_name_extra");
        assert_eq!(value, &json!("Alice"));
    }

    #[test]
    fn test_matches_method() {
        let matcher = FieldMatcher::new("user_name");

        // Exact match
        assert!(matcher.matches("user_name"));
        // camelCase variant
        assert!(matcher.matches("userName"));
        // PascalCase variant
        assert!(matcher.matches("UserName"));
        // Case variations
        assert!(matcher.matches("USER_NAME")); // uppercase with underscore
        assert!(matcher.matches("User_Name")); // mixed case with underscore
                                               // Should NOT match different field
        assert!(!matcher.matches("first_name"));
        // Should NOT match without underscore (strip_punctuation keeps underscores)
        assert!(!matcher.matches("username"));
    }

    // ===== StructDeserializer Tests =====

    #[test]
    fn test_struct_deserializer_strict_match() {
        use crate::value::Source;

        let obj = json!({"name": "Alice", "age": 30});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer = StructDeserializer::new()
            .field(FieldDescriptor::new("name", "String", false))
            .field(FieldDescriptor::new("age", "i64", false));

        let result =
            deserializer.deserialize(&value, &mut ctx, "TestStruct", |name, val, _ctx, strict| {
                if strict {
                    // Strict mode: only accept exact types
                    match (name, &val.value) {
                        ("name", Value::String(s)) => Ok(Box::new(s.clone()) as Box<dyn Any>),
                        ("age", Value::Number(n)) if n.is_i64() => {
                            Ok(Box::new(n.as_i64().unwrap()) as Box<dyn Any>)
                        }
                        _ => Err(ParseError::DeserializeFailed(
                            DeserializeError::type_mismatch("", ""),
                        )),
                    }
                } else {
                    // Lenient mode: allow coercion
                    match name {
                        "name" => Ok(Box::new(val.value.to_string()) as Box<dyn Any>),
                        "age" => Ok(Box::new(42i64) as Box<dyn Any>),
                        _ => Err(ParseError::DeserializeFailed(
                            DeserializeError::type_mismatch("", ""),
                        )),
                    }
                }
            });

        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 2);
        assert!(fields.contains_key("name"));
        assert!(fields.contains_key("age"));
    }

    #[test]
    fn test_struct_deserializer_fuzzy_field_names() {
        use crate::value::Source;

        // LLM returns camelCase, but struct expects snake_case
        let obj = json!({"userName": "Alice", "emailAddress": "alice@example.com"});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer = StructDeserializer::new()
            .field(FieldDescriptor::new("user_name", "String", false))
            .field(FieldDescriptor::new("email_address", "String", false));

        let result =
            deserializer.deserialize(&value, &mut ctx, "User", |_name, val, _ctx, _strict| {
                // Always return success for this test
                if let Value::String(s) = &val.value {
                    Ok(Box::new(s.clone()) as Box<dyn Any>)
                } else {
                    Err(ParseError::DeserializeFailed(
                        DeserializeError::type_mismatch("string", "other"),
                    ))
                }
            });

        assert!(result.is_ok());
        let fields = result.unwrap();

        // Check that both fields were matched
        assert!(fields.contains_key("user_name"));
        assert!(fields.contains_key("email_address"));

        // Check transformations
        let transformations = deserializer.transformations();
        assert_eq!(transformations.len(), 2);
        // Both should have FieldNameCaseChanged transformations
        assert!(transformations.iter().any(
            |t| matches!(t, Transformation::FieldNameCaseChanged { from, to }
            if from == "userName" && to == "user_name")
        ));
        assert!(transformations.iter().any(
            |t| matches!(t, Transformation::FieldNameCaseChanged { from, to }
            if from == "emailAddress" && to == "email_address")
        ));
    }

    #[test]
    fn test_struct_deserializer_optional_field_missing() {
        use crate::value::Source;

        let obj = json!({"name": "Alice"});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer = StructDeserializer::new()
            .field(FieldDescriptor::new("name", "String", false))
            .field(FieldDescriptor::new("age", "Option<i64>", true)); // Optional

        let result =
            deserializer.deserialize(&value, &mut ctx, "User", |name, val, _ctx, _strict| {
                match name {
                    "name" => {
                        if let Value::String(s) = &val.value {
                            Ok(Box::new(s.clone()) as Box<dyn Any>)
                        } else {
                            Err(ParseError::DeserializeFailed(
                                DeserializeError::type_mismatch("string", "other"),
                            ))
                        }
                    }
                    "age" => {
                        // Won't be called since field is missing and optional
                        unreachable!()
                    }
                    _ => Err(ParseError::DeserializeFailed(
                        DeserializeError::type_mismatch("", ""),
                    )),
                }
            });

        assert!(result.is_ok());
        let fields = result.unwrap();

        // Only "name" should be present
        assert_eq!(fields.len(), 1);
        assert!(fields.contains_key("name"));
        assert!(!fields.contains_key("age"));

        // Check transformation for default value
        let transformations = deserializer.transformations();
        assert!(transformations.iter().any(
            |t| matches!(t, Transformation::DefaultValueInserted { field }
            if field == "age")
        ));
    }

    #[test]
    fn test_struct_deserializer_required_field_missing() {
        use crate::value::Source;

        let obj = json!({"name": "Alice"});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer = StructDeserializer::new()
            .field(FieldDescriptor::new("name", "String", false))
            .field(FieldDescriptor::new("age", "i64", false)); // Required

        let result =
            deserializer.deserialize(&value, &mut ctx, "User", |_name, _val, _ctx, _strict| {
                Ok(Box::new(String::new()) as Box<dyn Any>)
            });

        // Should fail because "age" is required but missing
        assert!(result.is_err());
        if let Err(ParseError::DeserializeFailed(DeserializeError::MissingField { field })) = result
        {
            assert_eq!(field, "age");
        } else {
            panic!("Expected MissingField error");
        }
    }

    #[test]
    fn test_struct_deserializer_extra_keys() {
        use crate::value::Source;

        let obj = json!({"name": "Alice", "age": 30, "extra_field": "ignored"});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer = StructDeserializer::new()
            .field(FieldDescriptor::new("name", "String", false))
            .field(FieldDescriptor::new("age", "i64", false));

        let result =
            deserializer.deserialize(&value, &mut ctx, "User", |_name, val, _ctx, _strict| {
                match &val.value {
                    Value::String(s) => Ok(Box::new(s.clone()) as Box<dyn Any>),
                    Value::Number(n) => Ok(Box::new(n.as_i64().unwrap_or(0)) as Box<dyn Any>),
                    _ => Err(ParseError::DeserializeFailed(
                        DeserializeError::type_mismatch("", ""),
                    )),
                }
            });

        assert!(result.is_ok());

        // Check that extra key was tracked
        let transformations = deserializer.transformations();
        assert!(transformations
            .iter()
            .any(|t| matches!(t, Transformation::ExtraKey { key }
            if key == "extra_field")));
    }

    #[test]
    fn test_struct_deserializer_single_field_implicit_key() {
        use crate::value::Source;

        // Entire array coerced into single "items" field
        let arr = json!(["a", "b", "c"]);
        let value = FlexValue::new(arr, Source::Direct);
        let mut ctx = CoercionContext::new();

        let mut deserializer =
            StructDeserializer::new().field(FieldDescriptor::new("items", "Vec<String>", false));

        let result =
            deserializer.deserialize(&value, &mut ctx, "Container", |name, val, _ctx, _strict| {
                assert_eq!(name, "items");
                // Simulate deserializing the array
                if let Value::Array(_) = &val.value {
                    Ok(
                        Box::new(vec!["a".to_string(), "b".to_string(), "c".to_string()])
                            as Box<dyn Any>,
                    )
                } else {
                    Err(ParseError::DeserializeFailed(
                        DeserializeError::type_mismatch("array", "other"),
                    ))
                }
            });

        assert!(result.is_ok());
        let fields = result.unwrap();
        assert_eq!(fields.len(), 1);
        assert!(fields.contains_key("items"));

        // Check ImpliedKey transformation
        let transformations = deserializer.transformations();
        assert!(transformations
            .iter()
            .any(|t| matches!(t, Transformation::ImpliedKey { field }
            if field == "items")));
    }

    #[test]
    fn test_struct_deserializer_circular_detection() {
        use crate::value::Source;

        // Create a recursive structure
        let obj = json!({"name": "Node", "child": {"name": "Child"}});
        let value = FlexValue::new(obj, Source::Direct);
        let mut ctx = CoercionContext::new();

        // First call should succeed
        ctx = ctx.with_visited_lenient("Node", &value);

        let mut deserializer =
            StructDeserializer::new().field(FieldDescriptor::new("name", "String", false));

        // Try to deserialize again with the same type+value pair
        let result =
            deserializer.deserialize(&value, &mut ctx, "Node", |_name, _val, _ctx, _strict| {
                Ok(Box::new(String::new()) as Box<dyn Any>)
            });

        // Should detect circular reference
        assert!(result.is_err());
        if let Err(ParseError::DeserializeFailed(DeserializeError::CircularReference {
            type_name,
        })) = result
        {
            assert_eq!(type_name, "Node");
        } else {
            panic!("Expected CircularReference error, got: {:?}", result);
        }
    }
}
