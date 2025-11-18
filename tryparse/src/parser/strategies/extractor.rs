//! Extraction strategies that find candidate JSON substrings in text.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::{error::Result, parser::Candidate};

/// Static regex for extracting markdown code blocks.
/// Compiled once and reused across all MarkdownExtractor instances.
static CODE_BLOCK_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)```(\w*)\n(.*?)```").unwrap());

/// Trait for strategies that extract candidate JSON substrings from text.
///
/// Extractors don't parse or fix JSON - they just locate potential JSON
/// content within larger text bodies.
pub trait Extractor: Send + Sync + std::fmt::Debug {
    /// Returns the name of this extractor for debugging.
    fn name(&self) -> &'static str;

    /// Extracts candidate substrings from the input.
    ///
    /// Returns a vector of candidates, or an error if extraction failed.
    /// An empty vector indicates no candidates were found.
    fn extract(&self, input: &str) -> Result<Vec<Candidate>>;

    /// Returns the priority of this extractor.
    ///
    /// Lower values are tried first.
    fn priority(&self) -> u8;
}

/// Direct extractor that treats the entire input as a candidate.
#[derive(Debug, Clone, Default)]
pub struct DirectExtractor;

impl Extractor for DirectExtractor {
    fn name(&self) -> &'static str {
        "direct"
    }

    fn extract(&self, input: &str) -> Result<Vec<Candidate>> {
        if input.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Candidate::direct(input)])
    }

    fn priority(&self) -> u8 {
        1 // Try first - fastest
    }
}

/// Heuristic extractor that finds JSON-like structures in prose.
///
/// Uses balanced brace/bracket matching to extract potential JSON objects
/// and arrays from unstructured text.
#[derive(Debug, Clone)]
pub struct HeuristicExtractor {
    /// Maximum number of candidates to extract
    max_candidates: usize,
}

impl Default for HeuristicExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicExtractor {
    /// Maximum size of input to process (1MB) - DoS protection
    const MAX_INPUT_SIZE: usize = 1024 * 1024;

    /// Creates a new heuristic extractor with default settings.
    pub fn new() -> Self {
        Self { max_candidates: 20 }
    }

    /// Creates a new extractor with custom max candidates.
    pub const fn with_max_candidates(max_candidates: usize) -> Self {
        Self { max_candidates }
    }

    /// Extracts potential JSON boundaries from text.
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
    fn find_balanced_boundaries(
        &self,
        input: &str,
        open: char,
        close: char,
        pattern: &'static str,
        boundaries: &mut Vec<(usize, usize, &'static str)>,
    ) {
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == open {
                // Found opening bracket/brace, find matching close
                if let Some(end_idx) = self.find_matching_close(&chars, i, open, close) {
                    boundaries.push((i, end_idx + 1, pattern));
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
    fn find_matching_close(
        &self,
        chars: &[char],
        start: usize,
        open: char,
        close: char,
    ) -> Option<usize> {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (idx, &ch) in chars.iter().enumerate().skip(start) {
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

impl Extractor for HeuristicExtractor {
    fn name(&self) -> &'static str {
        "heuristic"
    }

    fn extract(&self, input: &str) -> Result<Vec<Candidate>> {
        // DoS protection: don't process huge inputs
        if input.len() > Self::MAX_INPUT_SIZE {
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
            candidates.push(Candidate::heuristic(substring, pattern.to_string()));
        }

        Ok(candidates)
    }

    fn priority(&self) -> u8 {
        2 // After direct
    }
}

/// Markdown code block extractor.
///
/// Extracts content from markdown fenced code blocks (```).
#[derive(Debug, Clone, Default)]
pub struct MarkdownExtractor;

impl MarkdownExtractor {
    /// Creates a new markdown extractor.
    pub fn new() -> Self {
        Self
    }
}

impl Extractor for MarkdownExtractor {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn extract(&self, input: &str) -> Result<Vec<Candidate>> {
        let mut candidates = Vec::new();

        for cap in CODE_BLOCK_REGEX.captures_iter(input) {
            let lang = cap.get(1).map(|m| m.as_str());
            let content = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            if !content.trim().is_empty() {
                let lang_opt = if let Some(l) = lang {
                    if !l.is_empty() {
                        Some(l.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };
                candidates.push(Candidate::markdown(content.trim(), lang_opt));
            }
        }

        Ok(candidates)
    }

    fn priority(&self) -> u8 {
        2 // Same as heuristic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_extractor() {
        let extractor = DirectExtractor;
        let candidates = extractor.extract(r#"{"name": "Alice"}"#).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].content, r#"{"name": "Alice"}"#);
    }

    #[test]
    fn test_direct_extractor_empty() {
        let extractor = DirectExtractor;
        let candidates = extractor.extract("   ").unwrap();
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_heuristic_extractor() {
        let extractor = HeuristicExtractor::new();
        let input = r#"Sure! Here's the data: {"name": "Alice", "age": 30} hope that helps!"#;

        let candidates = extractor.extract(input).unwrap();
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].content, r#"{"name": "Alice", "age": 30}"#);
    }

    #[test]
    fn test_heuristic_extractor_multiple() {
        let extractor = HeuristicExtractor::new();
        let input = r#"First: {"a": 1} and second: {"b": 2}"#;

        let candidates = extractor.extract(input).unwrap();
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_heuristic_extractor_array() {
        let extractor = HeuristicExtractor::new();
        let input = r#"The numbers are [1, 2, 3, 4, 5] as you can see."#;

        let candidates = extractor.extract(input).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].content, "[1, 2, 3, 4, 5]");
    }
}
