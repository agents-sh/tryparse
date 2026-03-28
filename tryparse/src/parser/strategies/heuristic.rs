//! Heuristic JSON extraction strategy.
//!
//! This strategy attempts to extract JSON from unstructured text by looking for
//! patterns that resemble JSON objects or arrays, even when buried in prose.

use super::ParsingStrategy;
use crate::{
    error::Result,
    value::{FlexValue, Source},
};

/// Maximum size of input to process (1MB) - DoS protection
const MAX_INPUT_SIZE: usize = 1024 * 1024;

/// Strategy that extracts JSON from unstructured prose.
///
/// This strategy uses heuristics to find JSON-like patterns in text:
/// - Looks for balanced braces/brackets
/// - Extracts potential JSON boundaries
/// - Attempts to parse each candidate
///
/// # Examples
///
/// ```
/// use tryparse::parser::strategies::{ParsingStrategy, HeuristicStrategy};
///
/// let strategy = HeuristicStrategy::default();
/// let input = r#"Sure! The data is {"name": "Alice", "age": 30} hope this helps!"#;
/// let result = strategy.parse(input).unwrap();
/// assert!(!result.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct HeuristicStrategy {
    /// Maximum number of candidates to try
    max_candidates: usize,
}

impl Default for HeuristicStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicStrategy {
    /// Creates a new heuristic strategy with default settings.
    #[inline]
    pub fn new() -> Self {
        Self { max_candidates: 20 }
    }

    /// Creates a new strategy with custom max candidates.
    #[inline]
    pub const fn with_max_candidates(max_candidates: usize) -> Self {
        Self { max_candidates }
    }

    /// Extracts potential JSON boundaries from text.
    ///
    /// Returns (start_index, end_index, pattern_type) tuples.
    fn find_json_boundaries(&self, input: &str) -> Vec<(usize, usize, &'static str)> {
        let mut boundaries = Vec::new();

        // Find object boundaries
        self.find_balanced_boundaries(input, '{', '}', "object", &mut boundaries);

        // Find array boundaries
        self.find_balanced_boundaries(input, '[', ']', "array", &mut boundaries);

        // Sort by start position and prefer longer matches
        boundaries.sort_by(|a, b| {
            let len_a = a.1 - a.0;
            let len_b = b.1 - b.0;
            a.0.cmp(&b.0).then(len_b.cmp(&len_a)) // Same start? Prefer longer
        });

        // Deduplicate overlapping regions (keep longer matches)
        let mut deduped = Vec::new();
        for boundary in boundaries {
            let overlaps = deduped.iter().any(|(start, end, _)| {
                // Check if this boundary overlaps with an existing one
                !(boundary.1 <= *start || boundary.0 >= *end)
            });
            if !overlaps {
                deduped.push(boundary);
            }
        }

        deduped
    }

    /// Finds balanced brace/bracket pairs.
    ///
    /// Boundaries are stored as byte offsets into `input`, not char indices,
    /// so they can be used directly with `&input[start..end]` even when the
    /// input contains multibyte UTF-8 characters.
    fn find_balanced_boundaries(
        &self,
        input: &str,
        open: char,
        close: char,
        pattern: &'static str,
        boundaries: &mut Vec<(usize, usize, &'static str)>,
    ) {
        // char_indices() yields (byte_offset, char) pairs.
        let chars: Vec<(usize, char)> = input.char_indices().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i].1 == open {
                // Found opening bracket/brace, find matching close
                if let Some(end_idx) = self.find_matching_close(&chars, i, open, close) {
                    let byte_start = chars[i].0;
                    // byte_end: start of the next char after end_idx, or the
                    // end of the string if end_idx is the last char.
                    let byte_end = chars
                        .get(end_idx + 1)
                        .map(|(offset, _)| *offset)
                        .unwrap_or(input.len());
                    boundaries.push((byte_start, byte_end, pattern));
                    i = end_idx + 1;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }

    /// Finds the matching closing bracket/brace.
    ///
    /// `chars` is a slice of `(byte_offset, char)` pairs (from
    /// `str::char_indices`). Returns the *vec index* (not byte offset) of the
    /// closing character, or `None` if the opening bracket is unbalanced.
    fn find_matching_close(
        &self,
        chars: &[(usize, char)],
        start: usize,
        open: char,
        close: char,
    ) -> Option<usize> {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (idx, &(_, ch)) in chars.iter().enumerate().skip(start) {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' | '\'' => in_string = !in_string,
                _ if ch == open && !in_string => depth += 1,
                _ if ch == close && !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(idx);
                    }
                }
                _ => {}
            }
        }

        None // Unbalanced
    }
}

impl ParsingStrategy for HeuristicStrategy {
    #[inline]
    fn name(&self) -> &'static str {
        "heuristic"
    }

    fn parse(&self, input: &str) -> Result<Vec<FlexValue>> {
        // DoS protection: don't process huge inputs
        if input.len() > MAX_INPUT_SIZE {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        let boundaries = self.find_json_boundaries(input);

        for (start, end, pattern) in boundaries.iter().take(self.max_candidates) {
            if start >= end || *end > input.len() {
                continue;
            }

            // Extract the substring
            let substring = &input[*start..*end];

            // Try to parse it
            if let Ok(value) = serde_json::from_str(substring) {
                candidates.push(FlexValue::new(
                    value,
                    Source::Heuristic {
                        pattern: pattern.to_string(),
                    },
                ));
            }
        }

        Ok(candidates)
    }

    #[inline]
    fn priority(&self) -> u8 {
        4 // Try after direct, markdown, and fixer
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_extract_json_from_prose() {
        let strategy = HeuristicStrategy::new();
        let input = r#"Sure! Here's the data: {"name": "Alice", "age": 30} hope that helps!"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"name": "Alice", "age": 30}));
    }

    #[test]
    fn test_extract_array_from_prose() {
        let strategy = HeuristicStrategy::new();
        let input = r#"The numbers are [1, 2, 3, 4, 5] as you can see."#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_multiple_json_in_prose() {
        let strategy = HeuristicStrategy::new();
        let input = r#"First: {"a": 1} and second: {"b": 2}"#;

        let result = strategy.parse(input).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_nested_json_in_prose() {
        let strategy = HeuristicStrategy::new();
        let input = r#"The user is {"name": "Alice", "address": {"city": "NYC"}} thanks!"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(
            result[0].value,
            json!({"name": "Alice", "address": {"city": "NYC"}})
        );
    }

    #[test]
    fn test_json_with_strings_containing_braces() {
        let strategy = HeuristicStrategy::new();
        let input = r#"Data: {"text": "Hello {world}"} done"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"text": "Hello {world}"}));
    }

    #[test]
    fn test_no_json_in_text() {
        let strategy = HeuristicStrategy::new();
        let input = "This is just plain text with no JSON.";

        let result = strategy.parse(input).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_unbalanced_braces() {
        let strategy = HeuristicStrategy::new();
        let input = r#"Invalid: {"name": "Alice" missing brace"#;

        let result = strategy.parse(input).unwrap();
        assert!(result.is_empty()); // Should not crash, just return empty
    }

    #[test]
    fn test_long_rambling_response() {
        let strategy = HeuristicStrategy::new();
        let input = r#"
        Well, let me think about this. The user you're asking about is quite interesting.
        They have been with us for a while. Actually, I should give you their data.
        The information is {"name": "Alice", "age": 30} as you can see.
        Let me know if you need anything else about this user or other users.
        "#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"name": "Alice", "age": 30}));
    }

    #[test]
    fn test_max_candidates() {
        let strategy = HeuristicStrategy::with_max_candidates(2);
        let input = r#"{"a": 1} {"b": 2} {"c": 3} {"d": 4}"#;

        let result = strategy.parse(input).unwrap();
        assert!(result.len() <= 2); // Should respect max_candidates
    }

    #[test]
    fn test_find_matching_close() {
        let strategy = HeuristicStrategy::new();
        let chars: Vec<(usize, char)> = r#"{"name": "Alice"}"#.char_indices().collect();

        let close_idx = strategy.find_matching_close(&chars, 0, '{', '}');
        assert_eq!(close_idx, Some(16)); // Closing brace is at vec index 16
    }

    #[test]
    fn test_find_matching_close_with_nested() {
        let strategy = HeuristicStrategy::new();
        let chars: Vec<(usize, char)> = r#"{"a": {"b": 1}}"#.char_indices().collect();

        let close_idx = strategy.find_matching_close(&chars, 0, '{', '}');
        assert_eq!(close_idx, Some(14));
    }

    #[test]
    fn test_dos_protection() {
        let strategy = HeuristicStrategy::new();
        let huge_input = "x".repeat(2 * 1024 * 1024); // 2MB

        let result = strategy.parse(&huge_input).unwrap();
        assert!(result.is_empty()); // Should reject huge inputs
    }

    // --- Non-ASCII / multibyte UTF-8 regression tests ---
    // These all previously panicked with "byte index N is not a char boundary"
    // because char indices were being used as byte offsets.

    #[test]
    fn test_non_ascii_thai_in_value() {
        let strategy = HeuristicStrategy::new();
        // Thai characters are 3 bytes each in UTF-8
        let input = r#"{"status":"complete","message":"สวัสดี"}"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value["message"], "สวัสดี");
    }

    #[test]
    fn test_non_ascii_emoji_in_value() {
        let strategy = HeuristicStrategy::new();
        // Emoji are 4 bytes each in UTF-8
        let input = r#"{"status":"complete","message":"Hello 🎉"}"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value["message"], "Hello 🎉");
    }

    #[test]
    fn test_non_ascii_chinese_in_value() {
        let strategy = HeuristicStrategy::new();
        // CJK characters are 3 bytes each in UTF-8
        let input = r#"Some text {"greeting": "你好世界"} more text"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value["greeting"], "你好世界");
    }

    #[test]
    fn test_non_ascii_prose_before_json() {
        let strategy = HeuristicStrategy::new();
        // Multibyte chars in the surrounding prose (before the JSON object)
        // cause the byte offset of '{' to diverge from its char index.
        let input = r#"สวัสดี: {"name": "Alice", "age": 30}"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"name": "Alice", "age": 30}));
    }

    #[test]
    fn test_non_ascii_accented_characters() {
        let strategy = HeuristicStrategy::new();
        // Accented chars are 2 bytes each in UTF-8
        let input = r#"Résumé: {"name": "Ångström", "city": "München"}"#;

        let result = strategy.parse(input).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].value["name"], "Ångström");
        assert_eq!(result[0].value["city"], "München");
    }
}
