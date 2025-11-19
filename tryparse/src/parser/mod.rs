//! Parser module that coordinates parsing strategies.

mod candidate;
mod cleaner;
pub mod state_machine;
pub mod strategies;

pub use candidate::{Candidate, CandidateSource};
pub use cleaner::{Cleaner, GarbageCleaner};
use strategies::{
    DirectExtractor, DirectJsonStrategy, Extractor, HeuristicExtractor, HeuristicStrategy,
    JsonFixerStrategy, MarkdownExtractor, MarkdownStrategy, MultipleObjectsStrategy,
    ParsingStrategy, RawPrimitiveStrategy, StateMachineStrategy,
};

use crate::{error::Result, value::FlexValue};

/// Maximum nesting depth before extraction is triggered to prevent stack overflow.
pub const MAX_NESTING_DEPTH: usize = 50;

#[cfg(feature = "yaml")]
use strategies::YamlStrategy;

/// Flexible parser that tries multiple strategies to extract JSON.
///
/// The parser applies strategies in priority order and collects all
/// successful candidates. Each candidate includes metadata about how
/// it was parsed.
///
/// # Examples
///
/// ```
/// use tryparse::parser::FlexibleParser;
///
/// let parser = FlexibleParser::default();
/// let candidates = parser.parse(r#"{"name": "Alice"}"#).unwrap();
/// assert!(!candidates.is_empty());
/// ```
#[derive(Debug)]
pub struct FlexibleParser {
    /// Parsing strategies in priority order.
    strategies: Vec<Box<dyn ParsingStrategy>>,
}

impl Clone for FlexibleParser {
    fn clone(&self) -> Self {
        // Recreate with default strategies
        // (We can't clone trait objects without adding a clone method to the trait)
        Self::new()
    }
}

impl Default for FlexibleParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FlexibleParser {
    /// Creates a new flexible parser with default strategies.
    ///
    /// Default strategies (in priority order):
    /// 1. DirectJsonStrategy - Fast path for valid JSON
    /// 2. JsonFixerStrategy - Repair common JSON errors
    /// 3. RawPrimitiveStrategy - Handle raw primitives like "true", "12,111"
    /// 4. StateMachineStrategy - State machine-based robust parsing
    /// 5. HeuristicStrategy - Extract JSON from prose
    /// 6. MarkdownStrategy - Extract from code blocks (if feature enabled)
    /// 7. YamlStrategy - Parse YAML and convert to JSON (if feature enabled)
    pub fn new() -> Self {
        let mut strategies: Vec<Box<dyn ParsingStrategy>> = vec![
            Box::new(DirectJsonStrategy),
            Box::new(JsonFixerStrategy::default()),
            Box::new(RawPrimitiveStrategy::new()),
            Box::new(StateMachineStrategy::new()),
            Box::new(HeuristicStrategy::default()),
            Box::new(MarkdownStrategy),
            Box::new(MultipleObjectsStrategy::new()),
        ];

        #[cfg(feature = "yaml")]
        {
            strategies.push(Box::new(YamlStrategy));
        }

        // Sort by priority
        strategies.sort_by_key(|s| s.priority());

        Self { strategies }
    }

    /// Creates a new parser with custom strategies.
    ///
    /// Strategies will be sorted by priority automatically.
    pub fn with_strategies(mut strategies: Vec<Box<dyn ParsingStrategy>>) -> Self {
        strategies.sort_by_key(|s| s.priority());
        Self { strategies }
    }

    /// Returns a builder for creating a parser with custom configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use tryparse::parser::FlexibleParser;
    /// use tryparse::parser::strategies::DirectJsonStrategy;
    ///
    /// // Create a minimal parser with only direct JSON
    /// let parser = FlexibleParser::builder()
    ///     .without_defaults()
    ///     .with_strategy(Box::new(DirectJsonStrategy))
    ///     .build();
    ///
    /// // Create a parser without YAML support
    /// let parser = FlexibleParser::builder()
    ///     .without_yaml()
    ///     .build();
    /// ```
    pub fn builder() -> FlexibleParserBuilder {
        FlexibleParserBuilder::new()
    }
}

/// Builder for creating a `FlexibleParser` with custom configuration.
pub struct FlexibleParserBuilder {
    use_defaults: bool,
    #[cfg(feature = "yaml")]
    without_yaml: bool,
    without_markdown: bool,
    without_heuristic: bool,
    additional_strategies: Vec<Box<dyn ParsingStrategy>>,
}

impl FlexibleParserBuilder {
    /// Creates a new builder with default settings.
    fn new() -> Self {
        Self {
            use_defaults: true,
            #[cfg(feature = "yaml")]
            without_yaml: false,
            without_markdown: false,
            without_heuristic: false,
            additional_strategies: Vec::new(),
        }
    }

    /// Disables all default strategies.
    ///
    /// You must add at least one strategy manually for parsing to work.
    pub fn without_defaults(mut self) -> Self {
        self.use_defaults = false;
        self
    }

    /// Disables YAML parsing strategy.
    #[cfg(feature = "yaml")]
    pub fn without_yaml(mut self) -> Self {
        self.without_yaml = true;
        self
    }

    /// Disables markdown code block extraction.
    pub fn without_markdown(mut self) -> Self {
        self.without_markdown = true;
        self
    }

    /// Disables heuristic JSON extraction from prose.
    pub fn without_heuristic(mut self) -> Self {
        self.without_heuristic = true;
        self
    }

    /// Adds a custom parsing strategy.
    ///
    /// The strategy will be sorted by priority with other strategies.
    pub fn with_strategy(mut self, strategy: Box<dyn ParsingStrategy>) -> Self {
        self.additional_strategies.push(strategy);
        self
    }

    /// Builds the `FlexibleParser` with the configured settings.
    pub fn build(self) -> FlexibleParser {
        let mut strategies: Vec<Box<dyn ParsingStrategy>> = if self.use_defaults {
            let mut default_strategies: Vec<Box<dyn ParsingStrategy>> = vec![
                Box::new(DirectJsonStrategy),
                Box::new(JsonFixerStrategy::default()),
                Box::new(RawPrimitiveStrategy::new()),
                Box::new(StateMachineStrategy::new()),
                Box::new(MultipleObjectsStrategy::new()),
            ];

            if !self.without_heuristic {
                default_strategies.push(Box::new(HeuristicStrategy::default()));
            }

            if !self.without_markdown {
                default_strategies.push(Box::new(MarkdownStrategy));
            }

            #[cfg(feature = "yaml")]
            if !self.without_yaml {
                default_strategies.push(Box::new(YamlStrategy));
            }

            default_strategies
        } else {
            Vec::new()
        };

        // Add custom strategies
        strategies.extend(self.additional_strategies);

        // Sort by priority
        strategies.sort_by_key(|s| s.priority());

        FlexibleParser { strategies }
    }
}

impl FlexibleParser {
    /// Parses the input using all strategies and returns all candidates.
    ///
    /// Each strategy is tried in priority order. All successful parses
    /// are returned as candidates.
    ///
    /// Returns an empty vector if no strategy succeeds.
    pub fn parse(&self, input: &str) -> Result<Vec<FlexValue>> {
        // Use the new multi-stage approach which fixes the architectural flaw
        self.parse_multi_stage(input)
    }

    /// Multi-stage parsing: extract, clean, fix, and parse candidates.
    ///
    /// This fixes the architectural flaw where extraction and fixing couldn't
    /// work together. Now we:
    /// 0. Pre-process: Remove invisible characters (BOM, zero-width spaces, etc.)
    /// 1. Try all registered strategies (direct JSON, YAML, etc.) on the full input
    /// 2. Extract candidate substrings (heuristic, markdown, direct)
    /// 3. Clean candidates (remove garbage, normalize whitespace)
    /// 4. For each candidate, try parsing directly
    /// 5. If that fails, apply fixes and try again
    ///
    /// Optimizations:
    /// - Early termination: Stops after finding candidates from high-priority strategies
    /// - Avoids extraction/fixing if direct parsing succeeds
    fn parse_multi_stage(&self, input: &str) -> Result<Vec<FlexValue>> {
        // Pre-processing: Clean up common issues that break parsing
        let cleaner = GarbageCleaner::new();

        // Step 0: Extract from deep nesting if needed (prevents stack overflow)
        let deep_nesting_extracted = cleaner.extract_from_deep_nesting(input, MAX_NESTING_DEPTH);
        let input_after_nesting = deep_nesting_extracted.as_deref().unwrap_or(input);

        // Step 1: Remove invisible characters
        let step1 = cleaner.remove_invisible_chars(input_after_nesting);

        // Step 2: Fix unnecessary backslashes
        let preprocessed = cleaner.fix_unnecessary_backslashes(&step1);
        let input = preprocessed.as_str();

        let mut all_candidates = Vec::new();

        // Stage 0: Try all registered strategies on the full input first
        // This includes DirectJsonStrategy, YamlStrategy, etc.
        // OPTIMIZATION: Try strategies in priority order and stop early if we find direct JSON
        let mut needs_normalization = false;
        for strategy in &self.strategies {
            match strategy.parse(input) {
                Ok(mut candidates) => {
                    let is_direct = candidates
                        .iter()
                        .any(|c| matches!(c.source, crate::value::Source::Direct));

                    // Direct JSON might need field normalization
                    if is_direct {
                        needs_normalization = true;
                    }

                    all_candidates.append(&mut candidates);

                    // OPTIMIZATION: If we found direct JSON or YAML, don't try other strategies
                    // Note: We do NOT early-return for MultiJsonArray - let other strategies
                    // create individual candidates too, so deserialization can pick the right one
                    // based on target type (Vec<T> vs T)
                    if !all_candidates.is_empty()
                        && (is_direct
                            || candidates
                                .iter()
                                .any(|c| matches!(c.source, crate::value::Source::Yaml)))
                    {
                        // Apply field normalization to direct JSON if needed
                        if needs_normalization {
                            all_candidates = self.normalize_candidates(all_candidates)?;
                        }
                        return Ok(all_candidates);
                    }
                }
                Err(_) => {
                    // Strategy failed, continue with others
                }
            }
        }

        // If we found candidates from strategies, return them
        if !all_candidates.is_empty() {
            return Ok(all_candidates);
        }

        // Stage 1: Extract candidates
        let extracted = self.extract_candidates(input)?;

        // OPTIMIZATION: If no candidates extracted, return early
        if extracted.is_empty() {
            return Ok(Vec::new());
        }

        // Stage 2: Clean candidates
        let cleaned = self.clean_candidates(extracted)?;

        // Stage 3: For each cleaned candidate, try to parse
        let fixer = JsonFixerStrategy::default();

        for candidate in cleaned {
            // Try direct parsing first
            if let Ok(value) = serde_json::from_str(&candidate.content) {
                all_candidates.push(FlexValue::new(value, candidate.to_source()));

                // OPTIMIZATION: If we successfully parsed without fixes, we have a good candidate
                // Continue to collect all candidates but we know we have at least one good result
                continue;
            }

            // If direct parsing failed, try applying fixes
            match fixer.parse(&candidate.content) {
                Ok(mut fixed_candidates) => {
                    // Update the source to indicate it came from extraction + fixing
                    for fc in &mut fixed_candidates {
                        // Combine the extraction source with the fix source
                        if let crate::value::Source::Fixed { fixes } = &fc.source {
                            fc.source = crate::value::Source::Fixed {
                                fixes: fixes.clone(),
                            };
                        }
                    }
                    all_candidates.append(&mut fixed_candidates);
                }
                Err(_) => {
                    // This candidate couldn't be parsed even after fixes
                    continue;
                }
            }
        }

        Ok(all_candidates)
    }

    /// Cleans extracted candidates to remove garbage and normalize.
    fn clean_candidates(&self, candidates: Vec<Candidate>) -> Result<Vec<Candidate>> {
        let cleaner = GarbageCleaner::new();
        let mut cleaned = Vec::new();

        for candidate in candidates {
            match cleaner.clean(&candidate)? {
                Some(cleaned_candidate) => cleaned.push(cleaned_candidate),
                None => cleaned.push(candidate), // No cleaning needed
            }
        }

        Ok(cleaned)
    }

    /// Normalizes field names in FlexValue candidates (for direct JSON).
    fn normalize_candidates(&self, candidates: Vec<FlexValue>) -> Result<Vec<FlexValue>> {
        let cleaner = GarbageCleaner::new();
        let mut normalized = Vec::new();

        for flex_value in candidates {
            // Check if this is a double-escaped JSON string
            if let serde_json::Value::String(s) = &flex_value.value {
                // Try to parse the string content as JSON
                if let Ok(inner_value) = serde_json::from_str::<serde_json::Value>(s) {
                    // This was double-escaped! Use the inner value instead
                    normalized.push(FlexValue::new(inner_value, flex_value.source));
                    continue;
                }
            }

            // Convert FlexValue back to JSON string, apply all normalizations, and re-parse
            match serde_json::to_string(&flex_value.value) {
                Ok(json_str) => {
                    // Apply invisible character removal first
                    let invisible_removed = cleaner.remove_invisible_chars(&json_str);

                    // Then normalize field names
                    let normalized_str = cleaner.normalize_field_names(&invisible_removed);

                    if normalized_str != json_str {
                        // Something was normalized, create new FlexValue
                        if let Ok(new_value) = serde_json::from_str(&normalized_str) {
                            normalized.push(FlexValue::new(new_value, flex_value.source));
                            continue;
                        }
                    }
                    // No normalization or failed, keep original
                    normalized.push(flex_value);
                }
                Err(_) => {
                    // Can't serialize, keep original
                    normalized.push(flex_value);
                }
            }
        }

        Ok(normalized)
    }

    /// Extracts candidate substrings from the input using all extractors.
    fn extract_candidates(&self, input: &str) -> Result<Vec<Candidate>> {
        let mut candidates = Vec::new();

        // Create extractors
        let extractors: Vec<Box<dyn Extractor>> = vec![
            Box::new(DirectExtractor),
            Box::new(HeuristicExtractor::default()),
            Box::new(MarkdownExtractor),
        ];

        // Run all extractors
        for extractor in extractors {
            match extractor.extract(input) {
                Ok(mut extracted) => {
                    candidates.append(&mut extracted);
                }
                Err(_) => {
                    // Continue with other extractors
                }
            }
        }

        Ok(candidates)
    }

    /// Returns the number of strategies registered.
    #[inline]
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    /// Returns the names of all registered strategies in priority order.
    pub fn strategy_names(&self) -> Vec<&'static str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_new_parser() {
        let parser = FlexibleParser::new();

        // Markdown and MultipleObjects are always enabled, YAML is optional
        #[cfg(feature = "yaml")]
        assert_eq!(parser.strategy_count(), 8);

        #[cfg(not(feature = "yaml"))]
        assert_eq!(parser.strategy_count(), 7);
    }

    #[test]
    fn test_strategy_priority_order() {
        let parser = FlexibleParser::new();
        let names = parser.strategy_names();

        // MultipleObjectsStrategy should be first (priority 0) to detect multiple JSON objects
        // before DirectJsonStrategy (priority 1) parses only the first object
        assert_eq!(names[0], "multiple_objects");
        assert_eq!(names[1], "direct_json");
    }

    #[test]
    fn test_parse_direct_json() {
        let parser = FlexibleParser::new();
        let result = parser.parse(r#"{"name": "Alice"}"#).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"name": "Alice"}));
    }

    #[test]
    fn test_parse_with_trailing_comma() {
        let parser = FlexibleParser::new();
        let result = parser.parse(r#"{"name": "Alice",}"#).unwrap();

        assert!(!result.is_empty());
        // Should have candidates from both direct (fail) and fixer (success)
    }

    #[test]
    fn test_parse_markdown() {
        let parser = FlexibleParser::new();
        let input = r#"
```json
{"name": "Bob"}
```
"#;
        let result = parser.parse(input).unwrap();

        assert!(!result.is_empty());
        assert_eq!(result[0].value, json!({"name": "Bob"}));
    }

    #[test]
    fn test_parse_empty_input() {
        let parser = FlexibleParser::new();
        let result = parser.parse("").unwrap();

        // Empty input returns a raw string candidate from RawPrimitiveStrategy
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!(""));
    }

    #[test]
    fn test_parse_invalid_text() {
        let parser = FlexibleParser::new();
        let result = parser.parse("This is just plain text").unwrap();

        // Plain text returns a raw string candidate from RawPrimitiveStrategy
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!("This is just plain text"));
    }

    #[test]
    fn test_multiple_candidates() {
        let parser = FlexibleParser::new();
        // Single quotes - will be fixed by JsonFixerStrategy
        let result = parser.parse(r#"{'name': 'Alice'}"#).unwrap();

        // Should have at least one candidate from fixer
        assert!(!result.is_empty());
    }

    #[test]
    fn test_with_custom_strategies() {
        let strategies: Vec<Box<dyn ParsingStrategy>> = vec![Box::new(DirectJsonStrategy)];
        let parser = FlexibleParser::with_strategies(strategies);

        assert_eq!(parser.strategy_count(), 1);
        assert_eq!(parser.strategy_names()[0], "direct_json");
    }
}
