//! Markdown extraction strategy.

use once_cell::sync::Lazy;
use regex::Regex;

use super::ParsingStrategy;
use crate::{
    error::Result,
    value::{FlexValue, Source},
};

/// Static regex for extracting markdown code blocks.
/// Compiled once and reused across all MarkdownStrategy instances.
static CODE_BLOCK_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)```(\w*)\n(.*?)```").unwrap());

/// Strategy that extracts JSON from markdown code blocks.
///
/// This strategy looks for JSON content wrapped in markdown code fences:
/// - ` ```json ... ``` `
/// - ` ``` ... ``` ` (generic code blocks)
///
/// # Examples
///
/// ```
/// use tryparse::parser::strategies::{ParsingStrategy, MarkdownStrategy};
///
/// let strategy = MarkdownStrategy::default();
/// // Input with markdown code fence containing JSON
/// let input = "Here's the data:\n```json\n{\"name\": \"Alice\"}\n```\n";
/// let result = strategy.parse(input).unwrap();
/// assert!(!result.is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct MarkdownStrategy;

impl MarkdownStrategy {
    /// Creates a new markdown extraction strategy.
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Removes trailing commas from JSON.
    ///
    /// This is a simple version of JsonFixerStrategy's fix_trailing_commas,
    /// applied to markdown-extracted content before parsing.
    fn remove_trailing_commas(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == ',' {
                // Look ahead to see if this is a trailing comma
                let mut is_trailing = false;
                let mut temp_peek = chars.clone();

                // Skip whitespace
                while let Some(&next) = temp_peek.peek() {
                    if next.is_whitespace() {
                        temp_peek.next();
                    } else if next == '}' || next == ']' {
                        is_trailing = true;
                        break;
                    } else {
                        break;
                    }
                }

                if !is_trailing {
                    result.push(c);
                }
                // If trailing, skip it (don't add to result)
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Extracts all code blocks from markdown text.
    ///
    /// Returns tuples of (language_tag, content).
    fn extract_code_blocks<'a>(&self, input: &'a str) -> Vec<(Option<String>, &'a str)> {
        CODE_BLOCK_REGEX
            .captures_iter(input)
            .filter_map(|cap| {
                let lang = cap.get(1).and_then(|m| {
                    let s = m.as_str();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                });
                let content = cap.get(2)?.as_str().trim();
                Some((lang, content))
            })
            .collect()
    }

    /// Checks if a language tag suggests JSON content.
    #[inline]
    fn is_json_lang(lang: &Option<String>) -> bool {
        match lang {
            Some(l) => {
                let lower = l.to_lowercase();
                lower == "json" || lower == "jsonc" || lower == "json5"
            }
            None => false,
        }
    }

    /// Scores a code block based on contextual heuristics.
    ///
    /// Higher scores indicate more likely to be the "real" data vs examples.
    /// Returns a score from 0-100.
    fn score_block(
        &self,
        input: &str,
        block_content: &str,
        block_index: usize,
        total_blocks: usize,
    ) -> i32 {
        let mut score = 50; // Baseline

        // Find the position of this block in the input
        if let Some(block_pos) = input.find(block_content) {
            // Get text before this block (context)
            let context_before = &input[..block_pos];
            let context_lines: Vec<&str> = context_before
                .lines()
                .rev()
                .take(5) // Look at last 5 lines before block
                .collect();
            let context = context_lines.join(" ").to_lowercase();

            // Positive indicators (this is the real data)
            let positive_keywords = [
                ("real", 20),
                ("actual", 20),
                ("result", 15),
                ("answer", 15),
                ("final", 15),
                ("correct", 15),
                ("here is", 10),
                ("here's", 10),
                ("output", 10),
                ("response", 8),
            ];

            for (keyword, points) in &positive_keywords {
                if context.contains(keyword) {
                    score += points;
                }
            }

            // Negative indicators (this is an example/demo)
            let negative_keywords = [
                ("example", -25),
                ("sample", -20),
                ("demo", -20),
                ("test", -15),
                ("illustration", -15),
                ("for instance", -15),
                ("like this", -10),
                ("such as", -10),
            ];

            for (keyword, points) in &negative_keywords {
                if context.contains(keyword) {
                    score += points;
                }
            }
        }

        // Position-based scoring: later blocks often more important
        // (after examples comes the real data)
        if total_blocks > 1 {
            let position_score = (block_index * 30) / (total_blocks - 1);
            score += position_score as i32;
        }

        // Size-based scoring: larger objects might be more complete
        let field_count = block_content.matches(':').count();
        if field_count > 5 {
            score += 10;
        } else if field_count > 10 {
            score += 20;
        }

        score
    }
}

impl ParsingStrategy for MarkdownStrategy {
    #[inline]
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn parse(&self, input: &str) -> Result<Vec<FlexValue>> {
        let blocks = self.extract_code_blocks(input);

        // Collect valid JSON blocks with their scores
        let mut scored_candidates: Vec<(i32, FlexValue)> = Vec::new();

        // First, try blocks explicitly marked as JSON
        let json_blocks: Vec<_> = blocks
            .iter()
            .enumerate()
            .filter(|(_, (lang, _))| Self::is_json_lang(lang))
            .collect();

        if !json_blocks.is_empty() {
            for (index, (lang, content)) in json_blocks.iter() {
                // Apply trailing comma removal before parsing
                let fixed_content = self.remove_trailing_commas(content);

                // Try direct parsing first
                if let Ok(value) = serde_json::from_str(&fixed_content) {
                    let score = self.score_block(input, content, *index, json_blocks.len());
                    scored_candidates.push((
                        score,
                        FlexValue::new(value, Source::Markdown { lang: lang.clone() }),
                    ));
                } else {
                    // Direct parsing failed, try JSON fixer strategies
                    // This handles unquoted keys, triple-quoted strings, etc.
                    use super::JsonFixerStrategy;
                    let fixer = JsonFixerStrategy::default();
                    if let Ok(fixed_candidates) = fixer.parse(&fixed_content) {
                        for flex_val in fixed_candidates {
                            let score = self.score_block(input, content, *index, json_blocks.len());
                            // Preserve markdown source (overwrite fixer source)
                            scored_candidates.push((
                                score,
                                FlexValue::new(
                                    flex_val.value,
                                    Source::Markdown { lang: lang.clone() },
                                ),
                            ));
                        }
                    }
                }
            }
        } else {
            // Try unmarked code blocks if no JSON blocks were found
            let unmarked_blocks: Vec<_> = blocks
                .iter()
                .enumerate()
                .filter(|(_, (lang, _))| lang.is_none())
                .collect();

            for (index, (lang, content)) in unmarked_blocks.iter() {
                // Only try if it looks like JSON
                let trimmed = content.trim();
                if trimmed.starts_with('{') || trimmed.starts_with('[') {
                    // Apply trailing comma removal before parsing
                    let fixed_content = self.remove_trailing_commas(trimmed);

                    // Try direct parsing first
                    if let Ok(value) = serde_json::from_str(&fixed_content) {
                        let score = self.score_block(input, content, *index, unmarked_blocks.len());
                        scored_candidates.push((
                            score,
                            FlexValue::new(value, Source::Markdown { lang: lang.clone() }),
                        ));
                    } else {
                        // Direct parsing failed, try JSON fixer strategies
                        use super::JsonFixerStrategy;
                        let fixer = JsonFixerStrategy::default();
                        if let Ok(fixed_candidates) = fixer.parse(&fixed_content) {
                            for flex_val in fixed_candidates {
                                let score =
                                    self.score_block(input, content, *index, unmarked_blocks.len());
                                scored_candidates.push((
                                    score,
                                    FlexValue::new(
                                        flex_val.value,
                                        Source::Markdown { lang: lang.clone() },
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Sort by score (descending) and return candidates
        scored_candidates.sort_by(|a, b| b.0.cmp(&a.0));

        // Return all candidates in score order (best first)
        // The parser framework will use the best one
        Ok(scored_candidates
            .into_iter()
            .map(|(_, candidate)| candidate)
            .collect())
    }

    #[inline]
    fn priority(&self) -> u8 {
        2 // Try after direct JSON but before repair
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_extract_json_code_block() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
Here's the response:
```json
{"name": "Alice", "age": 30}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!({"name": "Alice", "age": 30}));
        assert!(matches!(
            result[0].source,
            Source::Markdown { lang: Some(_) }
        ));
    }

    #[test]
    fn test_extract_generic_code_block() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
Response:
```
{"name": "Bob"}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!({"name": "Bob"}));
        assert!(matches!(result[0].source, Source::Markdown { lang: None }));
    }

    #[test]
    fn test_multiple_code_blocks() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
First block:
```json
{"id": 1}
```

Second block:
```json
{"id": 2}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 2);
        // Results are now sorted by score (best first)
        // Second block scores higher due to position-based scoring
        assert_eq!(result[0].value, json!({"id": 2}));
        assert_eq!(result[1].value, json!({"id": 1}));
    }

    #[test]
    fn test_prefer_json_tagged_blocks() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
Generic:
```
{"generic": true}
```

JSON:
```json
{"tagged": true}
```
"#;
        let result = strategy.parse(input).unwrap();

        // Should only return the JSON-tagged block
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!({"tagged": true}));
    }

    #[test]
    fn test_no_code_blocks() {
        let strategy = MarkdownStrategy::default();
        let result = strategy.parse("Just plain text").unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_invalid_json_in_code_block() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```json
{invalid json}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_jsonc_language_tag() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```jsonc
{"name": "Alice"}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!({"name": "Alice"}));
    }

    #[test]
    fn test_json5_language_tag() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```json5
{"name": "Alice"}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!({"name": "Alice"}));
    }

    #[test]
    fn test_extract_code_blocks() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```json
{"a": 1}
```

```python
print("hello")
```

```
{"b": 2}
```
"#;
        let blocks = strategy.extract_code_blocks(input);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].0, Some("json".to_string()));
        assert_eq!(blocks[1].0, Some("python".to_string()));
        assert_eq!(blocks[2].0, None);
    }

    #[test]
    fn test_is_json_lang() {
        assert!(MarkdownStrategy::is_json_lang(&Some("json".to_string())));
        assert!(MarkdownStrategy::is_json_lang(&Some("JSON".to_string())));
        assert!(MarkdownStrategy::is_json_lang(&Some("jsonc".to_string())));
        assert!(MarkdownStrategy::is_json_lang(&Some("json5".to_string())));
        assert!(!MarkdownStrategy::is_json_lang(&Some("python".to_string())));
        assert!(!MarkdownStrategy::is_json_lang(&None));
    }

    #[test]
    fn test_nested_braces_in_code_block() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```json
{
  "user": {
    "name": "Alice",
    "address": {
      "city": "NYC"
    }
  }
}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].value,
            json!({
                "user": {
                    "name": "Alice",
                    "address": {
                        "city": "NYC"
                    }
                }
            })
        );
    }

    #[test]
    fn test_array_in_code_block() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
```json
[1, 2, 3, 4, 5]
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_trailing_comma_in_markdown_json() {
        let strategy = MarkdownStrategy::default();
        let input = r#"
some text
```json
{
  "key": "value",
  "array": [1, 2, 3,],
  "object": {
    "key": "value"
  }
}
```
"#;
        let result = strategy.parse(input).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].value,
            json!({
                "key": "value",
                "array": [1, 2, 3],
                "object": {
                    "key": "value"
                }
            })
        );
    }

    #[test]
    fn test_remove_trailing_commas() {
        let strategy = MarkdownStrategy::default();

        // Test array with trailing comma
        let input1 = "[1, 2, 3,]";
        let result1 = strategy.remove_trailing_commas(input1);
        assert_eq!(result1, "[1, 2, 3]");

        // Test object with trailing comma
        let input2 = r#"{"a": 1, "b": 2,}"#;
        let result2 = strategy.remove_trailing_commas(input2);
        assert_eq!(result2, r#"{"a": 1, "b": 2}"#);

        // Test nested structure with trailing commas
        let input3 = r#"{"array": [1, 2,], "obj": {"x": 1,}}"#;
        let result3 = strategy.remove_trailing_commas(input3);
        assert_eq!(result3, r#"{"array": [1, 2], "obj": {"x": 1}}"#);

        // Test no trailing comma (should be unchanged)
        let input4 = "[1, 2, 3]";
        let result4 = strategy.remove_trailing_commas(input4);
        assert_eq!(result4, "[1, 2, 3]");
    }
}
