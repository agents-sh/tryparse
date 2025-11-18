//! Enum deserialization with BAML's fuzzy variant matching.
//!
//! Ported from `engine/baml-lib/jsonish/src/deserializer/coercer/ir_ref/coerce_enum.rs`
//! and `match_string.rs`.

use serde_json::Value;

use crate::{
    deserializer::struct_coercer::{remove_accents, strip_punctuation},
    error::{DeserializeError, ParseError, Result},
    value::FlexValue,
};

/// Strategy used for fuzzy matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    /// Exact case-sensitive match
    Exact,
    /// Unaccented match (café → cafe)
    Unaccented,
    /// Punctuation-stripped match
    PunctuationStripped,
    /// Case-insensitive match
    CaseInsensitive,
    /// Substring match
    Substring,
    /// Levenshtein distance (edit distance)
    Levenshtein,
}

/// Result of fuzzy matching with metadata.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The matched variant name
    pub variant: String,
    /// The strategy that was used to match
    pub strategy: MatchStrategy,
    /// Edit distance (for Levenshtein matches, 0 for others)
    pub distance: usize,
}

/// Metadata about an enum variant for fuzzy matching.
#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// The canonical variant name (e.g., "Success", "Error")
    pub name: String,
    /// Optional description for matching
    pub description: Option<String>,
}

impl EnumVariant {
    /// Creates a new enum variant.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Sets the description for this variant.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Returns all match strings for this variant.
    ///
    /// Port from `coerce_enum.rs:14-31`.
    fn match_strings(&self) -> Vec<String> {
        match &self.description {
            Some(desc) if !desc.trim().is_empty() => {
                vec![
                    self.name.clone(),
                    desc.clone(),
                    format!("{}: {}", self.name, desc),
                ]
            }
            _ => vec![self.name.clone()],
        }
    }
}

/// Enum variant matcher with BAML's fuzzy matching algorithm.
///
/// Implements multi-strategy matching:
/// 1. Exact match (case-sensitive)
/// 2. Unaccented match (café → cafe)
/// 3. Punctuation-stripped match
/// 4. Case-insensitive match
/// 5. Substring match
/// 6. Levenshtein distance (edit distance < 30% of input length)
///
/// Port from `match_string.rs` with enum-specific logic.
#[derive(Debug, Clone)]
pub struct EnumMatcher {
    /// List of enum variants
    variants: Vec<EnumVariant>,
}

impl EnumMatcher {
    /// Creates a new enum matcher with no variants.
    pub fn new() -> Self {
        Self {
            variants: Vec::new(),
        }
    }

    /// Adds a variant to the matcher.
    pub fn variant(mut self, variant: EnumVariant) -> Self {
        self.variants.push(variant);
        self
    }

    /// Match a string to an enum variant using BAML's algorithm.
    ///
    /// Port from `match_string.rs:39-133` with full fuzzy matching.
    ///
    /// # Returns
    /// - `Ok(variant_name)` if a match is found
    /// - `Err(...)` if no match found or ambiguous
    pub fn match_string(&self, input: &str) -> Result<String> {
        self.match_string_detailed(input)
            .map(|result| result.variant)
    }

    /// Match a string to an enum variant using BAML's algorithm, with detailed metadata.
    ///
    /// Returns match result including which strategy was used and distance metrics.
    ///
    /// # Returns
    /// - `Ok(MatchResult)` if a match is found, with strategy and distance information
    /// - `Err(...)` if no match found or ambiguous
    pub fn match_string_detailed(&self, input: &str) -> Result<MatchResult> {
        let input = input.trim();

        // Build candidates list: (variant_name, [match_strings])
        let candidates: Vec<(&str, Vec<String>)> = self
            .variants
            .iter()
            .map(|v| (v.name.as_str(), v.match_strings()))
            .collect();

        // Strategy 1: Exact case-sensitive match
        if let Some(matched) = self.try_exact_match(input, &candidates) {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::Exact,
                distance: 0,
            });
        }

        // Strategy 2: Unaccented case-sensitive match
        if let Some(matched) = self.try_unaccented_match(input, &candidates) {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::Unaccented,
                distance: 0,
            });
        }

        // Strip punctuation and try again
        let stripped_input = strip_punctuation(input);
        let stripped_candidates: Vec<(&str, Vec<String>)> = candidates
            .iter()
            .map(|(name, values)| {
                let stripped_values = values.iter().map(|v| strip_punctuation(v)).collect();
                (*name, stripped_values)
            })
            .collect();

        // Strategy 3: Punctuation-stripped match (case-sensitive)
        if let Some(matched) = self.try_exact_match(&stripped_input, &stripped_candidates) {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::PunctuationStripped,
                distance: 0,
            });
        }

        // Strategy 4: Case-insensitive match (after stripping punctuation)
        let lowercase_input = stripped_input.to_lowercase();
        let lowercase_candidates: Vec<(&str, Vec<String>)> = stripped_candidates
            .iter()
            .map(|(name, values)| {
                let lowercase_values = values.iter().map(|v| v.to_lowercase()).collect();
                (*name, lowercase_values)
            })
            .collect();

        if let Some(matched) = self.try_exact_match(&lowercase_input, &lowercase_candidates) {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::CaseInsensitive,
                distance: 0,
            });
        }

        // Strategy 5: Substring match
        if let Some(matched) = self.try_substring_match(&lowercase_input, &lowercase_candidates) {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::Substring,
                distance: 0,
            });
        }

        // Strategy 6: Levenshtein distance (edit distance)
        if let Some((matched, distance)) =
            self.try_edit_distance_match(&lowercase_input, &lowercase_candidates)
        {
            return Ok(MatchResult {
                variant: matched.to_string(),
                strategy: MatchStrategy::Levenshtein,
                distance,
            });
        }

        // No match found - find closest suggestion
        let suggestion = self.find_closest_suggestion(&lowercase_input, &lowercase_candidates);

        Err(ParseError::DeserializeFailed(
            DeserializeError::UnknownVariant {
                enum_name: "enum".to_string(),
                variant: input.to_string(),
                suggestion,
            },
        ))
    }

    /// Find the closest matching variant to suggest.
    ///
    /// Returns the closest variant if its edit distance is reasonable (< 50% of input length).
    fn find_closest_suggestion(
        &self,
        input: &str,
        lowercase_candidates: &[(&str, Vec<String>)],
    ) -> Option<String> {
        let mut best_match: Option<(&str, usize)> = None;
        let max_distance = (input.len() / 2).max(2); // At most 50% of length, minimum 2

        for (variant_name, match_strings) in lowercase_candidates {
            for match_str in match_strings {
                let distance = levenshtein_distance(input, match_str);
                if distance <= max_distance {
                    if let Some((_, best_dist)) = best_match {
                        if distance < best_dist {
                            best_match = Some((variant_name, distance));
                        }
                    } else {
                        best_match = Some((variant_name, distance));
                    }
                }
            }
        }

        best_match.map(|(name, _)| name.to_string())
    }

    /// Try exact match strategy.
    fn try_exact_match<'a>(
        &self,
        input: &str,
        candidates: &'a [(&'a str, Vec<String>)],
    ) -> Option<&'a str> {
        for (variant_name, match_strings) in candidates {
            if match_strings.iter().any(|s| s == input) {
                return Some(variant_name);
            }
        }
        None
    }

    /// Try unaccented match strategy.
    fn try_unaccented_match<'a>(
        &self,
        input: &str,
        candidates: &'a [(&'a str, Vec<String>)],
    ) -> Option<&'a str> {
        let unaccented_input = remove_accents(input);
        for (variant_name, match_strings) in candidates {
            if match_strings
                .iter()
                .any(|s| remove_accents(s) == unaccented_input)
            {
                return Some(variant_name);
            }
        }
        None
    }

    /// Try substring match strategy.
    ///
    /// Port from `match_string.rs:237-328`.
    ///
    /// Checks both directions:
    /// 1. Does a variant appear in the input? (e.g., "active" in "currently active")
    /// 2. Does the input appear in a variant? (e.g., "act" in "active")
    fn try_substring_match<'a>(
        &self,
        input: &str,
        candidates: &'a [(&'a str, Vec<String>)],
    ) -> Option<&'a str> {
        // First try: Find variants that appear in the input
        // (start_index, end_index, match_length, variant_name)
        let mut all_matches: Vec<(usize, usize, usize, &'a str)> = Vec::new();

        for (variant_name, match_strings) in candidates {
            for match_str in match_strings {
                // Check if variant appears in input
                for (start_idx, _) in input.match_indices(match_str.as_str()) {
                    let end_idx = start_idx + match_str.len();
                    all_matches.push((start_idx, end_idx, match_str.len(), variant_name));
                }
            }
        }

        // If we found matches where variants appear in input, use those
        if !all_matches.is_empty() {
            // Sort by length (longest first) to prefer exact matches
            all_matches.sort_by(|a, b| b.2.cmp(&a.2));

            // Return the variant with the longest match
            return Some(all_matches[0].3);
        }

        // Second try: Find variants that contain the input as substring
        let mut reverse_matches: Vec<(&'a str, usize)> = Vec::new();

        for (variant_name, match_strings) in candidates {
            for match_str in match_strings {
                // Check if input appears in variant
                if match_str.contains(input) {
                    reverse_matches.push((variant_name, match_str.len()));
                }
            }
        }

        if !reverse_matches.is_empty() {
            // Sort by match string length (shortest first) to prefer closest match
            reverse_matches.sort_by(|a, b| a.1.cmp(&b.1));
            return Some(reverse_matches[0].0);
        }

        None
    }

    /// Try edit distance (Levenshtein) match strategy.
    ///
    /// Port from `match_string.rs` edit distance logic.
    ///
    /// Accepts matches where edit_distance < input.len() / 3 (i.e., < 30% of input length).
    fn try_edit_distance_match<'a>(
        &self,
        input: &str,
        candidates: &'a [(&'a str, Vec<String>)],
    ) -> Option<(&'a str, usize)> {
        let mut best_match: Option<&'a str> = None;
        let mut best_distance = usize::MAX;

        for (variant_name, match_strings) in candidates {
            for match_str in match_strings {
                let distance = levenshtein_distance(input, match_str);
                if distance < best_distance {
                    best_distance = distance;
                    best_match = Some(variant_name);
                }
            }
        }

        // Accept if edit distance is small enough (< 30% of length)
        let threshold = if input.is_empty() { 0 } else { input.len() / 3 };

        if best_distance <= threshold {
            best_match.map(|m| (m, best_distance))
        } else {
            None
        }
    }
}

impl Default for EnumMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate Levenshtein distance between two strings.
///
/// Port from BAML's `match_string.rs:666-692`.
///
/// This is the classic dynamic programming algorithm for edit distance.
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first row and column
    for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    // Fill the matrix
    for (i, c1) in s1_chars.iter().enumerate() {
        for (j, c2) in s2_chars.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(
                    matrix[i][j + 1] + 1, // deletion
                    matrix[i + 1][j] + 1, // insertion
                ),
                matrix[i][j] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

/// Match a FlexValue to an enum variant.
///
/// Port from `coerce_enum.rs:76-108`.
pub fn match_enum_variant(value: &FlexValue, matcher: &EnumMatcher) -> Result<String> {
    match &value.value {
        Value::String(s) => matcher.match_string(s),
        Value::Number(n) => {
            // Try to convert number to string and match
            matcher.match_string(&n.to_string())
        }
        Value::Bool(b) => {
            // Try to convert bool to string and match
            matcher.match_string(&b.to_string())
        }
        _ => Err(ParseError::DeserializeFailed(
            DeserializeError::type_mismatch("string", "non-string"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::value::Source;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("hello", "help"), 2);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("saturday", "sunday"), 3);
    }

    #[test]
    fn test_enum_matcher_exact_match() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Error"))
            .variant(EnumVariant::new("Pending"));

        assert_eq!(matcher.match_string("Success").unwrap(), "Success");
        assert_eq!(matcher.match_string("Error").unwrap(), "Error");
        assert_eq!(matcher.match_string("Pending").unwrap(), "Pending");
    }

    #[test]
    fn test_enum_matcher_case_insensitive() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Error"));

        assert_eq!(matcher.match_string("success").unwrap(), "Success");
        assert_eq!(matcher.match_string("SUCCESS").unwrap(), "Success");
        assert_eq!(matcher.match_string("error").unwrap(), "Error");
        assert_eq!(matcher.match_string("ERROR").unwrap(), "Error");
    }

    #[test]
    fn test_enum_matcher_with_description() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Active").with_description("Currently active"))
            .variant(EnumVariant::new("Inactive").with_description("Not active"));

        // Exact match
        assert_eq!(matcher.match_string("Active").unwrap(), "Active");

        // Match by description
        assert_eq!(matcher.match_string("Currently active").unwrap(), "Active");

        // Match by combined "name: description"
        assert_eq!(
            matcher.match_string("Active: Currently active").unwrap(),
            "Active"
        );
    }

    #[test]
    fn test_enum_matcher_punctuation_stripping() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("InProgress"))
            .variant(EnumVariant::new("Completed"));

        // With punctuation
        assert_eq!(matcher.match_string("In-Progress").unwrap(), "InProgress");
        assert_eq!(matcher.match_string("in_progress").unwrap(), "InProgress");
    }

    #[test]
    fn test_enum_matcher_substring() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Processing"))
            .variant(EnumVariant::new("Completed"));

        // Substring match
        assert_eq!(
            matcher.match_string("Currently Processing").unwrap(),
            "Processing"
        );
        assert_eq!(
            matcher.match_string("Task Completed successfully").unwrap(),
            "Completed"
        );
    }

    #[test]
    fn test_enum_matcher_edit_distance() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Failure"));

        // Small typo - should match
        assert_eq!(matcher.match_string("Succes").unwrap(), "Success"); // 1 char off
        assert_eq!(matcher.match_string("Sucess").unwrap(), "Success"); // 1 char off
        assert_eq!(matcher.match_string("Failur").unwrap(), "Failure"); // 1 char off
    }

    #[test]
    fn test_enum_matcher_no_match() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Error"));

        // Completely different string
        let result = matcher.match_string("RandomValue");
        assert!(result.is_err());
    }

    #[test]
    fn test_enum_matcher_accents() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Café"))
            .variant(EnumVariant::new("Naïve"));

        // Without accents should match
        assert_eq!(matcher.match_string("Cafe").unwrap(), "Café");
        assert_eq!(matcher.match_string("Naive").unwrap(), "Naïve");
    }

    #[test]
    fn test_match_enum_variant_from_flex_value() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Error"));

        // String value
        let value = FlexValue::new(json!("Success"), Source::Direct);
        assert_eq!(match_enum_variant(&value, &matcher).unwrap(), "Success");

        // Case-insensitive
        let value = FlexValue::new(json!("success"), Source::Direct);
        assert_eq!(match_enum_variant(&value, &matcher).unwrap(), "Success");
    }

    #[test]
    fn test_match_string_detailed_strategies() {
        let matcher = EnumMatcher::new()
            .variant(EnumVariant::new("Success"))
            .variant(EnumVariant::new("Error"))
            .variant(EnumVariant::new("Naïve"));

        // Exact match
        let result = matcher.match_string_detailed("Success").unwrap();
        assert_eq!(result.variant, "Success");
        assert_eq!(result.strategy, MatchStrategy::Exact);
        assert_eq!(result.distance, 0);

        // Case-insensitive match
        let result = matcher.match_string_detailed("success").unwrap();
        assert_eq!(result.variant, "Success");
        assert_eq!(result.strategy, MatchStrategy::CaseInsensitive);
        assert_eq!(result.distance, 0);

        // Unaccented match
        let result = matcher.match_string_detailed("Naive").unwrap();
        assert_eq!(result.variant, "Naïve");
        assert_eq!(result.strategy, MatchStrategy::Unaccented);
        assert_eq!(result.distance, 0);

        // Punctuation-stripped match
        let result = matcher.match_string_detailed("Suc.cess").unwrap();
        assert_eq!(result.variant, "Success");
        assert_eq!(result.strategy, MatchStrategy::PunctuationStripped);
        assert_eq!(result.distance, 0);

        // Substring match
        let result = matcher.match_string_detailed("Succ").unwrap();
        assert_eq!(result.variant, "Success");
        assert_eq!(result.strategy, MatchStrategy::Substring);
        assert_eq!(result.distance, 0);

        // Levenshtein match (small edit distance)
        let result = matcher.match_string_detailed("Succss").unwrap();
        assert_eq!(result.variant, "Success");
        assert_eq!(result.strategy, MatchStrategy::Levenshtein);
        assert!(result.distance > 0);
    }
}
