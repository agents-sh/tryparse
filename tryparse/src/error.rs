//! Error types for LLM-to-struct parsing.

use std::fmt;

/// Format an unknown variant error message with optional suggestion.
fn format_unknown_variant_error(
    enum_name: &str,
    variant: &str,
    suggestion: &Option<String>,
) -> String {
    match suggestion {
        Some(s) => format!(
            "Unknown variant '{}' for enum {}. Did you mean '{}'?",
            variant, enum_name, s
        ),
        None => format!("Unknown variant '{}' for enum {}", variant, enum_name),
    }
}

/// Format an unknown field error message with optional suggestion.
fn format_unknown_field_error(
    struct_name: &str,
    field: &str,
    suggestion: &Option<String>,
) -> String {
    match suggestion {
        Some(s) => format!(
            "Unknown field '{}' in struct {}. Did you mean '{}'?",
            field, struct_name, s
        ),
        None => format!("Unknown field '{}' in struct {}", field, struct_name),
    }
}

/// Result type alias for parsing operations.
pub type Result<T> = std::result::Result<T, ParseError>;

/// Errors that can occur during parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// No valid candidates were found after trying all parsing strategies.
    #[error("No valid candidates found in response")]
    NoCandidates,

    /// All parsing strategies failed.
    #[error("All parsing strategies failed")]
    AllStrategiesFailed {
        /// Details of each failed strategy attempt.
        attempts: Vec<StrategyError>,
    },

    /// Deserialization failed for all candidates.
    #[error("Deserialization failed: {0}")]
    DeserializeFailed(#[from] DeserializeError),

    /// All candidates failed to deserialize.
    ///
    /// This provides details about each candidate that was tried and why it failed,
    /// which is more informative than just returning the first error.
    #[error("All {0} candidates failed to deserialize")]
    AllCandidatesFailed(AllCandidatesError),

    /// JSON parsing error from serde_json.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Configuration error.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Details about all failed deserialization attempts.
#[derive(Debug)]
pub struct AllCandidatesError {
    /// Each candidate that was tried and its error.
    pub attempts: Vec<CandidateError>,
}

impl fmt::Display for AllCandidatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} candidates", self.attempts.len())
    }
}

impl AllCandidatesError {
    /// Returns the number of failed attempts.
    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    /// Returns true if there are no attempts.
    pub fn is_empty(&self) -> bool {
        self.attempts.is_empty()
    }

    /// Returns the first error, if any.
    pub fn first_error(&self) -> Option<&DeserializeError> {
        self.attempts.first().map(|a| &a.error)
    }

    /// Returns detailed information about all attempts.
    pub fn details(&self) -> String {
        self.attempts
            .iter()
            .enumerate()
            .map(|(i, attempt)| {
                format!(
                    "  {}. [{}] (score: {}): {}",
                    i + 1,
                    attempt.source,
                    attempt.score,
                    attempt.error
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Details of a failed candidate deserialization attempt.
#[derive(Debug)]
pub struct CandidateError {
    /// Source of the candidate (e.g., "direct", "markdown", "yaml").
    pub source: String,
    /// Score of the candidate (lower is better).
    pub score: u32,
    /// Preview of the candidate value (first 100 chars).
    pub preview: String,
    /// The deserialization error.
    pub error: DeserializeError,
}

impl fmt::Display for CandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] (score: {}): {}",
            self.source, self.score, self.error
        )
    }
}

/// Details of a failed parsing strategy attempt.
#[derive(Debug, Clone)]
pub struct StrategyError {
    /// Name of the strategy that failed.
    pub strategy: &'static str,
    /// Error message describing why it failed.
    pub error: String,
}

impl StrategyError {
    /// Creates a new strategy error.
    #[inline]
    pub fn new(strategy: &'static str, error: impl Into<String>) -> Self {
        Self {
            strategy,
            error: error.into(),
        }
    }
}

impl fmt::Display for StrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.strategy, self.error)
    }
}

/// Errors that occur during deserialization.
#[derive(Debug, thiserror::Error)]
pub enum DeserializeError {
    /// Type mismatch between expected and found types.
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        /// Expected type name.
        expected: &'static str,
        /// Found type name.
        found: String,
    },

    /// Required field is missing from the input.
    #[error("Missing required field: {field}")]
    MissingField {
        /// Name of the missing field.
        field: String,
    },

    /// Invalid value encountered.
    #[error("Invalid value: {message}")]
    InvalidValue {
        /// Description of why the value is invalid.
        message: String,
    },

    /// Unknown variant for enum.
    #[error("{}", format_unknown_variant_error(.enum_name, .variant, .suggestion))]
    UnknownVariant {
        /// Enum type name.
        enum_name: String,
        /// The variant that was not recognized.
        variant: String,
        /// Suggested similar variant, if any.
        suggestion: Option<String>,
    },

    /// Unknown field in struct.
    #[error("{}", format_unknown_field_error(.struct_name, .field, .suggestion))]
    UnknownField {
        /// Struct type name.
        struct_name: String,
        /// The field that was not recognized.
        field: String,
        /// Suggested similar field, if any.
        suggestion: Option<String>,
    },

    /// Custom error message.
    #[error("{0}")]
    Custom(String),

    /// Depth limit exceeded during recursive deserialization.
    #[error("Depth limit exceeded: {depth} >= {max_depth}")]
    DepthLimitExceeded {
        /// Current depth.
        depth: usize,
        /// Maximum allowed depth.
        max_depth: usize,
    },

    /// Circular reference detected during deserialization.
    #[error("Circular reference detected for type: {type_name}")]
    CircularReference {
        /// Type name where the cycle was detected.
        type_name: String,
    },
}

impl DeserializeError {
    /// Creates a type mismatch error.
    #[inline]
    pub fn type_mismatch(expected: &'static str, found: impl Into<String>) -> Self {
        Self::TypeMismatch {
            expected,
            found: found.into(),
        }
    }

    /// Creates a missing field error.
    #[inline]
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Creates an invalid value error.
    #[inline]
    pub fn invalid_value(message: impl Into<String>) -> Self {
        Self::InvalidValue {
            message: message.into(),
        }
    }
}

impl serde::de::Error for DeserializeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_error_display() {
        let err = StrategyError::new("test_strategy", "something went wrong");
        assert_eq!(err.to_string(), "test_strategy: something went wrong");
    }

    #[test]
    fn test_deserialize_error_type_mismatch() {
        let err = DeserializeError::type_mismatch("integer", "string");
        assert!(err.to_string().contains("integer"));
        assert!(err.to_string().contains("string"));
    }

    #[test]
    fn test_parse_error_from_json() {
        let json_err = serde_json::from_str::<u32>("not a number").unwrap_err();
        let parse_err: ParseError = json_err.into();
        assert!(matches!(parse_err, ParseError::JsonError(_)));
    }

    #[test]
    fn test_unknown_variant_with_suggestion() {
        let err = DeserializeError::UnknownVariant {
            enum_name: "Status".to_string(),
            variant: "inprogress".to_string(),
            suggestion: Some("InProgress".to_string()),
        };

        let display = err.to_string();
        assert!(display.contains("inprogress"));
        assert!(display.contains("Status"));
        assert!(display.contains("Did you mean"));
        assert!(display.contains("InProgress"));
    }

    #[test]
    fn test_unknown_variant_without_suggestion() {
        let err = DeserializeError::UnknownVariant {
            enum_name: "Status".to_string(),
            variant: "xyz".to_string(),
            suggestion: None,
        };

        let display = err.to_string();
        assert!(display.contains("xyz"));
        assert!(display.contains("Status"));
        assert!(!display.contains("Did you mean"));
    }

    #[test]
    fn test_unknown_field_with_suggestion() {
        let err = DeserializeError::UnknownField {
            struct_name: "User".to_string(),
            field: "usernme".to_string(),
            suggestion: Some("username".to_string()),
        };

        let display = err.to_string();
        assert!(display.contains("usernme"));
        assert!(display.contains("User"));
        assert!(display.contains("Did you mean"));
        assert!(display.contains("username"));
    }

    #[test]
    fn test_all_candidates_error() {
        let err = AllCandidatesError {
            attempts: vec![
                CandidateError {
                    source: "direct".to_string(),
                    score: 0,
                    preview: r#"{"foo": "bar"}"#.to_string(),
                    error: DeserializeError::missing_field("required_field"),
                },
                CandidateError {
                    source: "markdown(json)".to_string(),
                    score: 5,
                    preview: r#"{"baz": 42}"#.to_string(),
                    error: DeserializeError::type_mismatch("string", "number"),
                },
            ],
        };

        assert_eq!(err.len(), 2);
        assert!(!err.is_empty());

        // Check first_error
        let first = err.first_error().unwrap();
        assert!(matches!(first, DeserializeError::MissingField { .. }));

        // Check details
        let details = err.details();
        assert!(details.contains("direct"));
        assert!(details.contains("markdown(json)"));
        assert!(details.contains("required_field"));

        // Check display
        assert_eq!(err.to_string(), "2 candidates");
    }

    #[test]
    fn test_candidate_error_display() {
        let err = CandidateError {
            source: "yaml".to_string(),
            score: 10,
            preview: "test".to_string(),
            error: DeserializeError::Custom("test error".to_string()),
        };

        let display = err.to_string();
        assert!(display.contains("yaml"));
        assert!(display.contains("10"));
        assert!(display.contains("test error"));
    }

    #[test]
    fn test_parse_error_all_candidates_failed() {
        let err = ParseError::AllCandidatesFailed(AllCandidatesError {
            attempts: vec![CandidateError {
                source: "direct".to_string(),
                score: 0,
                preview: "{}".to_string(),
                error: DeserializeError::missing_field("name"),
            }],
        });

        let display = err.to_string();
        assert!(display.contains("1 candidates"));
    }
}
