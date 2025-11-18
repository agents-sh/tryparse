//! Flexible value types with metadata.

use std::hash::{Hash, Hasher};

use serde_json::Value;

/// Confidence reduction factor applied for each transformation.
/// Each transformation multiplies confidence by this value (0.95 = 5% reduction).
pub const CONFIDENCE_PENALTY_FACTOR: f32 = 0.95;

/// A flexible value that wraps a JSON value with parsing metadata.
///
/// This type tracks how the value was obtained and what transformations
/// were applied, which is useful for debugging and scoring candidates.
#[derive(Debug, Clone)]
pub struct FlexValue {
    /// The underlying JSON value.
    pub value: Value,
    /// Information about how this value was parsed.
    pub source: Source,
    /// List of transformations applied to get this value.
    transformations: Vec<Transformation>,
    /// Confidence score (0.0 - 1.0), higher is better.
    /// Starts at 1.0 and decreases with each transformation.
    confidence: f32,
    /// Maximum nesting depth where transformations occurred.
    ///
    /// This is used for recursive scoring - transformations at deeper
    /// levels are penalized more heavily (10x per level).
    max_transformation_depth: usize,
}

// Implement Hash and Eq based on the value only (for circular detection)
impl Hash for FlexValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the JSON value structurally without string conversion
        hash_json_value(&self.value, state);
    }
}

/// Hash a JSON value recursively without string conversion.
///
/// This is more efficient than converting to string for large values.
fn hash_json_value<H: Hasher>(value: &Value, state: &mut H) {
    // Hash the discriminant first to distinguish different types
    std::mem::discriminant(value).hash(state);

    match value {
        Value::Null => {
            // Discriminant already hashed
        }
        Value::Bool(b) => {
            b.hash(state);
        }
        Value::Number(n) => {
            // Convert to canonical representation for hashing
            if let Some(i) = n.as_i64() {
                0u8.hash(state); // Mark as integer
                i.hash(state);
            } else if let Some(u) = n.as_u64() {
                1u8.hash(state); // Mark as unsigned
                u.hash(state);
            } else if let Some(f) = n.as_f64() {
                2u8.hash(state); // Mark as float
                                 // Convert float to bits for hashing
                f.to_bits().hash(state);
            }
        }
        Value::String(s) => {
            s.hash(state);
        }
        Value::Array(arr) => {
            arr.len().hash(state);
            for item in arr {
                hash_json_value(item, state);
            }
        }
        Value::Object(obj) => {
            obj.len().hash(state);
            // Hash entries in sorted order for consistency
            // (serde_json::Map preserves insertion order, but we want value equality)
            let mut keys: Vec<_> = obj.keys().collect();
            keys.sort();
            for key in keys {
                key.hash(state);
                hash_json_value(&obj[key], state);
            }
        }
    }
}

impl PartialEq for FlexValue {
    fn eq(&self, other: &Self) -> bool {
        // Compare based on JSON value only
        self.value == other.value
    }
}

impl Eq for FlexValue {}

impl FlexValue {
    /// Creates a new `FlexValue` with the given value and source.
    ///
    /// # Examples
    ///
    /// ```
    /// use tryparse::value::{FlexValue, Source};
    /// use serde_json::json;
    ///
    /// let value = FlexValue::new(json!({"name": "Alice"}), Source::Direct);
    /// assert_eq!(value.confidence(), 1.0);
    /// ```
    #[inline]
    pub fn new(value: Value, source: Source) -> Self {
        Self {
            value,
            source,
            transformations: Vec::new(),
            confidence: 1.0,
            max_transformation_depth: 0,
        }
    }

    /// Creates a new `FlexValue` from a fixed JSON string with repairs.
    #[inline]
    pub fn from_fixed_json(value: Value, fixes: Vec<JsonFix>) -> Self {
        Self {
            value,
            source: Source::Fixed { fixes },
            transformations: Vec::new(),
            confidence: 0.9, // Start lower for repaired JSON
            max_transformation_depth: 0,
        }
    }

    /// Adds a transformation to this value's history.
    ///
    /// This reduces the confidence score slightly.
    pub fn add_transformation(&mut self, trans: Transformation) {
        self.transformations.push(trans);
        self.confidence *= CONFIDENCE_PENALTY_FACTOR;
    }

    /// Adds a transformation with depth tracking for recursive scoring.
    ///
    /// The depth indicates the nesting level at which this transformation occurred.
    /// Deeper transformations are penalized more heavily in scoring (10x per level).
    pub fn add_transformation_at_depth(&mut self, trans: Transformation, depth: usize) {
        self.transformations.push(trans);
        self.confidence *= CONFIDENCE_PENALTY_FACTOR;
        self.max_transformation_depth = self.max_transformation_depth.max(depth);
    }

    /// Returns the maximum transformation depth.
    #[inline]
    pub const fn max_transformation_depth(&self) -> usize {
        self.max_transformation_depth
    }

    /// Returns the confidence score for this value.
    ///
    /// Higher values (closer to 1.0) indicate more confident parsing.
    #[inline]
    pub const fn confidence(&self) -> f32 {
        self.confidence
    }

    /// Returns a reference to the transformations applied.
    #[inline]
    pub fn transformations(&self) -> &[Transformation] {
        &self.transformations
    }

    /// Consumes self and returns the transformations.
    #[inline]
    pub fn into_transformations(self) -> Vec<Transformation> {
        self.transformations
    }

    /// Returns a JSON representation of the transformation history and metadata.
    ///
    /// This provides a human-readable explanation of how the value was parsed
    /// and what transformations were applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use tryparse::value::{FlexValue, Source, Transformation};
    /// use serde_json::json;
    ///
    /// let mut value = FlexValue::new(json!(42), Source::Direct);
    /// value.add_transformation(Transformation::StringToNumber {
    ///     original: "42".to_string(),
    /// });
    ///
    /// let explanation = value.explanation_json();
    /// assert!(explanation["transformations"].is_array());
    /// assert!(explanation["score"].is_number());
    /// ```
    pub fn explanation_json(&self) -> Value {
        use serde_json::json;

        // Convert source to JSON
        let source_json = match &self.source {
            Source::Direct => json!({"type": "direct"}),
            Source::Markdown { lang } => json!({
                "type": "markdown",
                "language": lang,
            }),
            Source::Fixed { fixes } => json!({
                "type": "fixed",
                "fixes": fixes.iter().map(|f| f.description()).collect::<Vec<_>>(),
            }),
            Source::MultiJson { index } => json!({
                "type": "multi_json",
                "index": index,
            }),
            Source::MultiJsonArray => json!({"type": "multi_json_array"}),
            Source::Heuristic { pattern } => json!({
                "type": "heuristic",
                "pattern": pattern,
            }),
            Source::Yaml => json!({"type": "yaml"}),
        };

        // Convert transformations to JSON
        let transformations_json: Vec<Value> = self
            .transformations
            .iter()
            .map(transformation_to_json)
            .collect();

        // Calculate score
        let score = crate::scoring::score_candidate(self);

        json!({
            "source": source_json,
            "confidence": self.confidence,
            "score": score,
            "transformations": transformations_json,
            "transformation_count": self.transformations.len(),
            "max_transformation_depth": self.max_transformation_depth,
        })
    }
}

/// Helper function to convert a Transformation to JSON.
fn transformation_to_json(t: &Transformation) -> Value {
    use serde_json::json;

    match t {
        Transformation::ExtractedFromMarkdown => json!({
            "type": "extracted_from_markdown",
            "penalty": t.penalty(),
        }),
        Transformation::JsonRepaired { fixes } => json!({
            "type": "json_repaired",
            "fixes": fixes.iter().map(|f| f.description()).collect::<Vec<_>>(),
            "penalty": t.penalty(),
        }),
        Transformation::StringToNumber { original } => json!({
            "type": "string_to_number",
            "original": original,
            "penalty": t.penalty(),
        }),
        Transformation::FloatToInt { original } => json!({
            "type": "float_to_int",
            "original": original,
            "penalty": t.penalty(),
        }),
        Transformation::SingleToArray => json!({
            "type": "single_to_array",
            "penalty": t.penalty(),
        }),
        Transformation::FieldNameCaseChanged { from, to } => json!({
            "type": "field_name_case_changed",
            "from": from,
            "to": to,
            "penalty": t.penalty(),
        }),
        Transformation::DefaultValueInserted { field } => json!({
            "type": "default_value_inserted",
            "field": field,
            "penalty": t.penalty(),
        }),
        Transformation::ExtraKey { key } => json!({
            "type": "extra_key",
            "key": key,
            "penalty": t.penalty(),
        }),
        Transformation::ImpliedKey { field } => json!({
            "type": "implied_key",
            "field": field,
            "penalty": t.penalty(),
        }),
        Transformation::ObjectFromMarkdown { score } => json!({
            "type": "object_from_markdown",
            "score": score,
            "penalty": t.penalty(),
        }),
        Transformation::ArrayItemParseError { index, error } => json!({
            "type": "array_item_parse_error",
            "index": index,
            "error": error,
            "penalty": t.penalty(),
        }),
        Transformation::JsonToString { original } => json!({
            "type": "json_to_string",
            "original": original,
            "penalty": t.penalty(),
        }),
        Transformation::ConstraintChecked {
            name,
            passed,
            is_assert,
        } => json!({
            "type": "constraint_checked",
            "name": name,
            "passed": passed,
            "is_assert": is_assert,
            "penalty": t.penalty(),
        }),
        Transformation::DefaultButHadUnparseableValue {
            field,
            value,
            error,
        } => json!({
            "type": "default_but_had_unparseable_value",
            "field": field,
            "value": value,
            "error": error,
            "penalty": t.penalty(),
        }),
        Transformation::SubstringMatch { original, target } => json!({
            "type": "substring_match",
            "original": original,
            "target": target,
            "penalty": t.penalty(),
        }),
        Transformation::StrippedNonAlphaNumeric { original, stripped } => json!({
            "type": "stripped_non_alphanumeric",
            "original": original,
            "stripped": stripped,
            "penalty": t.penalty(),
        }),
        Transformation::UnionMatch { index, candidates } => json!({
            "type": "union_match",
            "index": index,
            "candidates": candidates,
            "penalty": t.penalty(),
        }),
        Transformation::FirstMatch { index, total } => json!({
            "type": "first_match",
            "index": index,
            "total": total,
            "penalty": t.penalty(),
        }),
    }
}

/// Information about how a value was parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Parsed directly as valid JSON.
    Direct,

    /// Extracted from a markdown code block.
    Markdown {
        /// The language tag (e.g., "json"), if any.
        lang: Option<String>,
    },

    /// Parsed after fixing/repairing the JSON.
    Fixed {
        /// List of fixes that were applied.
        fixes: Vec<JsonFix>,
    },

    /// One of multiple JSON objects found.
    MultiJson {
        /// Index of this object in the sequence.
        index: usize,
    },

    /// All JSON objects combined into an array.
    MultiJsonArray,

    /// Extracted using heuristic pattern matching.
    Heuristic {
        /// Description of the pattern used.
        pattern: String,
    },

    /// Parsed from YAML and converted to JSON.
    Yaml,
}

/// Types of JSON fixes that can be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonFix {
    /// Added quotes around unquoted object keys.
    UnquotedKeys,
    /// Removed trailing commas.
    TrailingCommas,
    /// Converted single quotes to double quotes.
    SingleQuotes,
    /// Added missing commas between items.
    MissingCommas,
    /// Closed unclosed braces or brackets.
    UnclosedBraces,
    /// Removed comments from JSON.
    Comments,
    /// Normalized Unicode smart/curly quotes.
    SmartQuotes,
    /// Normalized field names to snake_case.
    FieldNormalization,
    /// Unescaped double-escaped JSON string.
    DoubleEscaped,
    /// Converted template literals (backticks) to quotes.
    TemplateLiterals,
    /// Converted hex numbers to decimal.
    HexNumbers,
    /// Escaped unescaped newlines in strings.
    UnescapedNewlines,
    /// Removed JavaScript function definitions.
    JavaScriptFunctions,
    /// Converted Python triple-quoted strings to regular quoted strings.
    TripleQuotedStrings,
    /// Added quotes around unquoted values.
    UnquotedValues,
}

impl JsonFix {
    /// Returns the penalty score for this fix type.
    ///
    /// Lower penalties are better. Some fixes are more reliable than others.
    pub const fn penalty(&self) -> u32 {
        match self {
            // Low-risk fixes (definitely correct)
            Self::TrailingCommas => 1,
            Self::Comments => 1,
            Self::SmartQuotes => 1,
            Self::DoubleEscaped => 1,
            Self::TemplateLiterals => 1,
            Self::UnescapedNewlines => 1,
            Self::JavaScriptFunctions => 1,

            // Medium-risk fixes (usually correct)
            Self::SingleQuotes => 2,
            Self::UnquotedKeys => 2,
            Self::HexNumbers => 2,
            Self::TripleQuotedStrings => 2,
            Self::MissingCommas => 3,
            Self::UnclosedBraces => 3,

            // Higher-risk fixes (can cause issues)
            Self::UnquotedValues => 5, // Can wrap already-quoted strings incorrectly
            Self::FieldNormalization => 4,
        }
    }

    /// Returns a human-readable description of this fix.
    pub const fn description(self) -> &'static str {
        match self {
            Self::UnquotedKeys => "added quotes around object keys",
            Self::TrailingCommas => "removed trailing commas",
            Self::SingleQuotes => "converted single quotes to double quotes",
            Self::MissingCommas => "added missing commas",
            Self::UnclosedBraces => "closed unclosed braces/brackets",
            Self::Comments => "removed comments",
            Self::SmartQuotes => "normalized smart/curly quotes",
            Self::FieldNormalization => "normalized field names to snake_case",
            Self::DoubleEscaped => "unescaped double-escaped JSON",
            Self::TemplateLiterals => "converted template literals (backticks) to quotes",
            Self::HexNumbers => "converted hex numbers to decimal",
            Self::UnescapedNewlines => "escaped unescaped newlines in strings",
            Self::JavaScriptFunctions => "removed JavaScript function definitions",
            Self::TripleQuotedStrings => "converted triple-quoted strings to regular quotes",
            Self::UnquotedValues => "added quotes around unquoted values",
        }
    }
}

/// Transformations applied during parsing or deserialization.
#[derive(Debug, Clone, PartialEq)]
pub enum Transformation {
    /// Extracted value from markdown code block.
    ExtractedFromMarkdown,

    /// JSON was repaired before parsing.
    JsonRepaired {
        /// The fixes that were applied.
        fixes: Vec<JsonFix>,
    },

    /// String was converted to a number.
    StringToNumber {
        /// The original string value.
        original: String,
    },

    /// Float was rounded to an integer.
    FloatToInt {
        /// The original float value.
        original: f64,
    },

    /// Single value was wrapped in an array.
    SingleToArray,

    /// Field name case was changed to match struct field.
    FieldNameCaseChanged {
        /// Original field name from JSON.
        from: String,
        /// Target field name in struct.
        to: String,
    },

    /// Default value was inserted for missing field.
    DefaultValueInserted {
        /// Name of the field that got a default.
        field: String,
    },

    /// Extra key found in object that doesn't match any struct field.
    ExtraKey {
        /// The extra key name.
        key: String,
    },

    /// Entire object was coerced into a single struct field (implicit key).
    ImpliedKey {
        /// The field that received the object.
        field: String,
    },

    /// Object was extracted from markdown code block.
    ///
    /// This is tracked separately from ExtractedFromMarkdown to specifically
    /// identify objects that were parsed from markdown-wrapped JSON.
    ObjectFromMarkdown {
        /// Score penalty for this markdown extraction.
        score: i32,
    },

    /// Array item failed to parse.
    ///
    /// This transformation tracks array items that couldn't be deserialized
    /// and were skipped. The index indicates the position in the array.
    ArrayItemParseError {
        /// Index of the item that failed.
        index: usize,
        /// Error message describing the failure.
        error: String,
    },

    /// Object/Array was converted to a string representation.
    ///
    /// This transformation indicates that a composite type (object or array)
    /// was converted to its string representation, which is generally
    /// undesirable in union resolution.
    JsonToString {
        /// The original JSON value that was converted.
        original: String,
    },

    /// Constraint was validated during deserialization.
    ///
    /// This tracks both passing and failing constraints to provide
    /// visibility into validation that occurred.
    ConstraintChecked {
        /// Name of the constraint that was checked.
        name: String,
        /// Whether the constraint passed.
        passed: bool,
        /// Whether this was an assert (fails deserialization) or check (just tracked).
        is_assert: bool,
    },

    /// Default value used despite having an unparseable value.
    ///
    /// This indicates a field had a value that couldn't be parsed,
    /// so a default was used instead. More expensive than DefaultValueInserted
    /// because we had data but couldn't use it.
    DefaultButHadUnparseableValue {
        /// The field name.
        field: String,
        /// The unparseable value.
        value: String,
        /// Error message from parsing attempt.
        error: String,
    },

    /// Substring match was used for enum or string matching.
    ///
    /// Instead of exact match, a substring was found.
    SubstringMatch {
        /// The original string that was matched.
        original: String,
        /// The target that was matched against.
        target: String,
    },

    /// Non-alphanumeric characters were stripped for matching.
    ///
    /// Punctuation and special characters were removed to find a match.
    StrippedNonAlphaNumeric {
        /// The original string before stripping.
        original: String,
        /// The stripped string that matched.
        stripped: String,
    },

    /// Union variant was selected from multiple candidates.
    ///
    /// This tracks which variant won in a union type resolution.
    UnionMatch {
        /// Index of the winning variant.
        index: usize,
        /// Names of all candidate types.
        candidates: Vec<String>,
    },

    /// First match was selected from multiple options.
    ///
    /// When multiple candidates succeeded, the first one was chosen.
    /// This is used for array-to-struct coercion.
    FirstMatch {
        /// Index of the selected option.
        index: usize,
        /// Total number of candidates.
        total: usize,
    },
}

impl Transformation {
    /// Returns a penalty score for this transformation.
    ///
    /// Higher scores indicate less desirable transformations.
    #[inline]
    pub const fn penalty(&self) -> u32 {
        match self {
            Self::ExtractedFromMarkdown => 0, // Free, already in source
            Self::JsonRepaired { .. } => 0,   // Free, already in source
            Self::StringToNumber { .. } => 2,
            Self::FloatToInt { .. } => 3,
            Self::SingleToArray => 5,
            Self::FieldNameCaseChanged { .. } => 4,
            Self::DefaultValueInserted { .. } => 50, // Very expensive
            Self::ExtraKey { .. } => 10,             // Moderate penalty
            Self::ImpliedKey { .. } => 8,            // Moderate penalty
            Self::ObjectFromMarkdown { score } => {
                // Dynamic penalty based on score
                if *score >= 0 {
                    *score as u32
                } else {
                    0
                }
            }
            Self::ArrayItemParseError { index, .. } => {
                // Penalty increases with index (deeper errors are worse)
                1 + (*index as u32)
            }
            Self::JsonToString { .. } => 2, // Moderate penalty for type conversion
            Self::ConstraintChecked {
                passed, is_assert, ..
            } => {
                // Failed asserts are very expensive
                // Failed checks are moderate penalty
                // Passing constraints are free
                match (passed, is_assert) {
                    (false, true) => 100, // Failed assert - very expensive
                    (false, false) => 10, // Failed check - moderate penalty
                    (true, _) => 0,       // Passing constraint - free
                }
            }
            Self::DefaultButHadUnparseableValue { .. } => 2, // Had value but couldn't parse
            Self::SubstringMatch { .. } => 2,                // Fuzzy matching
            Self::StrippedNonAlphaNumeric { .. } => 3,       // More aggressive fuzzy matching
            Self::UnionMatch { .. } => 0,                    // Just tracking, not a penalty
            Self::FirstMatch { .. } => 1,                    // Slight penalty for array-to-struct
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::json;

    use super::*;

    #[test]
    fn test_flex_value_new() {
        let value = FlexValue::new(json!({"test": 1}), Source::Direct);
        assert_eq!(value.confidence(), 1.0);
        assert!(value.transformations().is_empty());
    }

    #[test]
    fn test_flex_value_hash_consistency() {
        use std::hash::{DefaultHasher, Hasher};

        // Helper to compute hash
        fn compute_hash(value: &FlexValue) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        // Equal values should have equal hashes
        let v1 = FlexValue::new(json!({"a": 1, "b": 2}), Source::Direct);
        let v2 = FlexValue::new(json!({"a": 1, "b": 2}), Source::Direct);
        assert_eq!(compute_hash(&v1), compute_hash(&v2));

        // Different values should have different hashes
        let v3 = FlexValue::new(json!({"a": 1, "b": 3}), Source::Direct);
        assert_ne!(compute_hash(&v1), compute_hash(&v3));

        // Different types should have different hashes
        let v4 = FlexValue::new(json!([1, 2]), Source::Direct);
        assert_ne!(compute_hash(&v1), compute_hash(&v4));
    }

    #[test]
    fn test_flex_value_hash_with_hashset() {
        let mut set = HashSet::new();

        let v1 = FlexValue::new(json!({"name": "test", "value": 42}), Source::Direct);
        let v2 = FlexValue::new(json!({"name": "test", "value": 42}), Source::Direct);
        let v3 = FlexValue::new(json!({"name": "other", "value": 42}), Source::Direct);

        // Insert first value
        assert!(set.insert(v1.clone()));

        // Inserting equal value should return false (already exists)
        assert!(!set.insert(v2));

        // Inserting different value should succeed
        assert!(set.insert(v3));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_flex_value_hash_nested_objects() {
        use std::hash::{DefaultHasher, Hasher};

        fn compute_hash(value: &FlexValue) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        // Deeply nested objects should hash consistently
        let nested = json!({
            "level1": {
                "level2": {
                    "level3": [1, 2, 3]
                }
            }
        });
        let v1 = FlexValue::new(nested.clone(), Source::Direct);
        let v2 = FlexValue::new(nested, Source::Direct);
        assert_eq!(compute_hash(&v1), compute_hash(&v2));
    }

    #[test]
    fn test_flex_value_hash_floats() {
        use std::hash::{DefaultHasher, Hasher};

        fn compute_hash(value: &FlexValue) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        // Floats should hash by their bit representation
        let v1 = FlexValue::new(json!(1.234), Source::Direct);
        let v2 = FlexValue::new(json!(1.234), Source::Direct);
        let v3 = FlexValue::new(json!(1.235), Source::Direct);

        assert_eq!(compute_hash(&v1), compute_hash(&v2));
        assert_ne!(compute_hash(&v1), compute_hash(&v3));
    }

    #[test]
    fn test_flex_value_hash_all_types() {
        use std::hash::{DefaultHasher, Hasher};

        fn compute_hash(value: &FlexValue) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        // Test all JSON types produce different hashes
        let null = FlexValue::new(json!(null), Source::Direct);
        let bool_val = FlexValue::new(json!(true), Source::Direct);
        let num = FlexValue::new(json!(42), Source::Direct);
        let string = FlexValue::new(json!("test"), Source::Direct);
        let array = FlexValue::new(json!([1, 2]), Source::Direct);
        let object = FlexValue::new(json!({"a": 1}), Source::Direct);

        let hashes = [
            compute_hash(&null),
            compute_hash(&bool_val),
            compute_hash(&num),
            compute_hash(&string),
            compute_hash(&array),
            compute_hash(&object),
        ];

        // All hashes should be unique
        let unique: HashSet<_> = hashes.iter().collect();
        assert_eq!(
            unique.len(),
            hashes.len(),
            "All types should have unique hashes"
        );
    }

    #[test]
    fn test_flex_value_transformation() {
        let mut value = FlexValue::new(json!(42), Source::Direct);
        assert_eq!(value.confidence(), 1.0);

        value.add_transformation(Transformation::StringToNumber {
            original: "42".to_string(),
        });

        assert_eq!(value.confidence(), 0.95);
        assert_eq!(value.transformations().len(), 1);
    }

    #[test]
    fn test_transformation_penalty() {
        assert_eq!(
            Transformation::StringToNumber {
                original: "42".into()
            }
            .penalty(),
            2
        );
        assert_eq!(Transformation::FloatToInt { original: 42.5 }.penalty(), 3);
        assert_eq!(
            Transformation::DefaultValueInserted {
                field: "test".into()
            }
            .penalty(),
            50
        );
    }

    #[test]
    fn test_json_fix_description() {
        assert!(JsonFix::UnquotedKeys.description().contains("quotes"));
        assert!(JsonFix::TrailingCommas.description().contains("trailing"));
    }

    #[test]
    fn test_source_equality() {
        assert_eq!(Source::Direct, Source::Direct);
        assert_ne!(Source::Direct, Source::MultiJsonArray);
    }
}
