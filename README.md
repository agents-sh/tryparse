# tryparse

Multi-strategy parser for messy, real-world data. Built to handle LLM responses with broken JSON, markdown wrappers, type mismatches, and inconsistent formatting.

## Quick Start

```toml
[dependencies]
tryparse = "0.4"
serde = { version = "1.0", features = ["derive"] }
```

```rust
use tryparse::parse;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct User {
    name: String,
    age: u32,
}

fn main() {
    // Handles markdown wrappers, trailing commas, unquoted keys, type coercion
    let messy_input = r#"
    Here's your data:
    ```json
    {
      name: "Alice",
      age: "30",
    }
    ```
    "#;

    let user: User = parse(messy_input).unwrap();
    println!("{:?}", user); // User { name: "Alice", age: 30 }
}
```

## Core Features

### With `serde::Deserialize`

Basic type coercion works out of the box:

```rust
use tryparse::parse;
use serde::Deserialize;

#[derive(Deserialize)]
struct Data {
    count: i64,      // "42" → 42
    price: f64,      // "3.14" → 3.14
    active: bool,    // "true" → true
    tags: Vec<String>, // "tag" → ["tag"]
}

let data: Data = parse(r#"{"count": "42", "price": "3.14", "active": "true", "tags": "tag"}"#).unwrap();
```

### With `LlmDeserialize` (derive feature)

Advanced features require the `derive` feature:

```toml
[dependencies]
tryparse = { version = "0.4", features = ["derive"] }
tryparse-derive = "0.4"
```

**Fuzzy field matching** - Handles different naming conventions:

```rust
use tryparse::parse_llm;
use tryparse_derive::LlmDeserialize;

#[derive(Debug, LlmDeserialize)]
struct Config {
    user_name: String,  // Matches: userName, UserName, user-name, user.name, USER_NAME
    max_count: i64,
}

let data: Config = parse_llm(r#"{"userName": "Alice", "maxCount": 30}"#).unwrap();
```

**Enum fuzzy matching** - Case-insensitive, partial matches:

```rust
#[derive(Debug, LlmDeserialize)]
enum Status {
    InProgress,  // Matches: "in_progress", "in-progress", "inprogress", "in progress"
    Completed,   // Matches: "complete", "COMPLETED", "done"
    Cancelled,
}
```

**Internally-tagged enum fuzzy matching** - Tag values match fuzzily:

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, LlmDeserialize, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Decision {
    StartTask { skill_id: String, reasoning: String },
    AskClarification { message: String },
}

// All of these work with fuzzy tag matching:
let d1: Decision = parse_llm(r#"{"type": "start_task", "skill_id": "test", "reasoning": "clear"}"#).unwrap();
let d2: Decision = parse_llm(r#"{"type": "StartTask", "skill_id": "test", "reasoning": "clear"}"#).unwrap();
let d3: Decision = parse_llm(r#"{"type": "startTask", "skill_id": "test", "reasoning": "clear"}"#).unwrap();
let d4: Decision = parse_llm(r#"{"type": "start-task", "skill_id": "test", "reasoning": "clear"}"#).unwrap();
let d5: Decision = parse_llm(r#"{"type": "STARTTASK", "skill_id": "test", "reasoning": "clear"}"#).unwrap();
```

**Union types** - Automatically picks the best variant:

```rust
#[derive(Debug, LlmDeserialize)]
#[llm(union)]
enum Value {
    Number(i64),
    Text(String),
    List(Vec<String>),
}

// Parses as Number(42)
let v1: Value = parse_llm("42").unwrap();

// Parses as Text("hello")
let v2: Value = parse_llm(r#""hello""#).unwrap();

// Parses as List(...)
let v3: Value = parse_llm(r#"["a", "b"]"#).unwrap();
```

**Implied key** - Single-field structs unwrap values:

```rust
#[derive(Debug, LlmDeserialize)]
struct Wrapper {
    data: String,
}

// Direct string wraps into the single field
let w: Wrapper = parse_llm(r#""hello world""#).unwrap();
assert_eq!(w.data, "hello world");
```

## API Reference

### Basic Parsing

```rust
// Parse with serde::Deserialize
fn parse<T: DeserializeOwned>(input: &str) -> Result<T>

// Parse with serde::Deserialize, get all candidates
fn parse_with_candidates<T: DeserializeOwned>(input: &str) -> Result<(T, Vec<FlexValue>)>

// Parse with custom parser configuration
fn parse_with_parser<T: DeserializeOwned>(input: &str, parser: &FlexibleParser) -> Result<T>

// Parse with metadata (strategy used, duration, candidates evaluated)
fn parse_with_metadata<T: DeserializeOwned>(input: &str) -> Result<(T, ParseMetadata)>
```

### Advanced Parsing (requires `derive` feature)

```rust
// Parse with LlmDeserialize trait (fuzzy matching, unions, etc.)
fn parse_llm<T: LlmDeserialize>(input: &str) -> Result<T>

// Parse with LlmDeserialize, get all candidates
fn parse_llm_with_candidates<T: LlmDeserialize>(input: &str) -> Result<(T, Vec<FlexValue>)>
```

### Utilities

```rust
// Score a candidate (lower is better)
fn score_candidate(candidate: &FlexValue) -> u32

// Rank candidates by score
fn rank_candidates(candidates: &mut [FlexValue])

// Get the best candidate
fn best_candidate(candidates: &[FlexValue]) -> Option<&FlexValue>
```

## How It Works

### 1. Multi-Stage Parsing Pipeline

```
Input String
    ↓
┌──────────────────────────────────┐
│ Pre-Processing                   │
│ • Remove BOM, zero-width chars   │
│ • Fix excessive nesting (>50)    │
│ • Normalize backslashes          │
└────────────┬─────────────────────┘
             ↓
┌──────────────────────────────────┐
│ Strategy Execution (parallel)    │
│ • DirectJson        (priority 1) │
│ • Markdown          (priority 2) │
│ • YAML              (priority 15)│
│ • JsonFixer         (priority 20)│
│ • Heuristic         (priority 30)│
│                                  │
│ → Produces Vec<FlexValue>        │
└────────────┬─────────────────────┘
             ↓
┌──────────────────────────────────┐
│ Scoring & Ranking                │
│ • Base score by source           │
│ • Transformation penalties       │
│ • Confidence adjustment          │
│ • Sort ascending (best first)    │
└────────────┬─────────────────────┘
             ↓
┌──────────────────────────────────┐
│ Deserialization                  │
│ • Try candidates in order        │
│ • Apply type coercion            │
│ • Track transformations          │
│ • Return first success           │
└──────────────────────────────────┘
```

### 2. Parsing Strategies

| Strategy | Priority | Description |
|----------|----------|-------------|
| **DirectJson** | 1 | Direct `serde_json::from_str()`. Fastest path for valid JSON. |
| **Markdown** | 2 | Extracts from markdown code blocks. Scores by keywords, position, size. |
| **YAML** | 15 | Parses YAML, converts to JSON. Requires `yaml` feature. |
| **JsonFixer** | 20 | Fixes common JSON errors (see below). |
| **Heuristic** | 30 | Pattern-based extraction from prose. Last resort. |

### 3. JSON Fixes Applied

The `JsonFixer` strategy handles:

- **Trailing commas**: `{"a": 1,}` → `{"a": 1}`
- **Unquoted keys**: `{name: "x"}` → `{"name": "x"}`
- **Single quotes**: `{'a': 1}` → `{"a": 1}`
- **Missing commas**: `{"a":1 "b":2}` → `{"a":1,"b":2}`
- **Unclosed braces/brackets**: `{"a": 1` → `{"a": 1}`
- **Comments**: `{"a": 1 /* comment */}` → `{"a": 1}`
- **Smart quotes**: `{"a": "value"}` → `{"a": "value"}`
- **Double-escaped JSON**: `"{\"a\":1}"` → `{"a":1}`
- **Template literals**: `` {`key`: "value"} `` → `{"key": "value"}`
- **Hex numbers**: `{"a": 0xFF}` → `{"a": 255}`
- **Unescaped newlines** in strings
- **JavaScript functions**: Removed entirely

### 4. Type Coercion

Applied during deserialization (works with both `Deserialize` and `LlmDeserialize`):

| From | To | Example |
|------|-----|---------|
| String | Number | `"42"` → `42` |
| String | Bool | `"true"` → `true` |
| Number | String | `42` → `"42"` |
| Float | Int | `42.0` → `42` |
| Single | Array | `"item"` → `["item"]` |

### 5. Field Matching (LlmDeserialize only)

Normalizes field names to snake_case and matches case-insensitively:

| Struct Field | Matches JSON Keys |
|--------------|-------------------|
| `user_name` | `userName`, `UserName`, `user-name`, `user.name`, `USER_NAME`, `username` |
| `max_count` | `maxCount`, `MaxCount`, `max-count`, `max.count`, `MAX_COUNT` |

**Note**: Does not handle acronyms perfectly. `XMLParser` becomes `x_m_l_parser` not `xml_parser`.

### 6. Scoring System

**Base Scores** (by source):
- Direct JSON: 0
- Markdown: 10
- YAML: 15
- Fixed JSON: 20 + (5 × number of fixes)
- Heuristic: 50

**Transformation Penalties**:
- String→Number: +2
- Float→Int: +3
- Field rename: +4
- Single→Array: +5
- Default inserted: +50

**Confidence Modifier**:
- Each transformation reduces confidence by 5%
- Final score += `(1.0 - confidence) × 100`

**Lower scores win**. Direct JSON with no coercion scores 0 (best possible).

## Examples

### Handling Markdown Responses

```rust
let llm_output = r#"
Sure! Here's the user data:

```json
{
  "name": "Alice",
  "age": 30,
  "email": "alice@example.com"
}
```

Let me know if you need anything else!
"#;

#[derive(Deserialize, Debug)]
struct User {
    name: String,
    age: i64,
    email: String,
}

let user: User = parse(llm_output).unwrap();
```

### Inspecting Parse Candidates

```rust
use tryparse::{parse_with_candidates, scoring::score_candidate};

let (result, candidates) = parse_with_candidates::<User>(messy_input).unwrap();

println!("Best result: {:?}", result);
println!("\nAll candidates:");
for (i, candidate) in candidates.iter().enumerate() {
    println!("  {}: {:?} (score: {})",
        i,
        candidate.source(),
        score_candidate(candidate)
    );

    // Inspect transformations
    for t in candidate.transformations() {
        println!("    - {:?}", t);
    }
}
```

### Custom Parser Configuration

```rust
use tryparse::parser::FlexibleParser;
use tryparse::parse_with_parser;

// Use builder pattern for custom configuration
let parser = FlexibleParser::builder()
    .without_heuristic()  // Disable heuristic extraction
    .without_markdown()   // Disable markdown extraction
    .build();

let data: User = parse_with_parser(input, &parser).unwrap();
```

### Complex Nested Structures

```rust
use std::collections::HashMap;

#[derive(Debug, LlmDeserialize)]
struct Project {
    name: String,
    owner: User,
    status: Status,
    tags: Vec<String>,
    metadata: HashMap<String, String>,
}

let project: Project = parse_llm(complex_json).unwrap();
```

### Union Types with Scoring

```rust
#[derive(Debug, LlmDeserialize)]
#[llm(union)]
enum Response {
    Success { data: User },
    Error { message: String, code: i64 },
    Pending { estimated_time: i64 },
}

// Automatically picks the variant that best matches the structure
let response: Response = parse_llm(api_response).unwrap();

match response {
    Response::Success { data } => println!("User: {:?}", data),
    Response::Error { message, code } => eprintln!("Error {}: {}", code, message),
    Response::Pending { estimated_time } => println!("Wait {}s", estimated_time),
}
```

## Feature Flags

```toml
# Default: includes markdown and yaml
[dependencies]
tryparse = "0.4"

# Minimal build (core JSON parsing only)
tryparse = { version = "0.4", default-features = false }

# With derive macros for LlmDeserialize
tryparse = { version = "0.4", features = ["derive"] }

# All features
tryparse = { version = "0.4", features = ["derive", "markdown", "yaml"] }
```

Available features:
- `markdown` (default) - Markdown code block extraction
- `yaml` (default) - YAML parsing support
- `derive` - Derive macro for `LlmDeserialize` (fuzzy field/enum matching, union types)

## Testing

```bash
# Unit tests (226 tests in lib)
cargo test --lib

# All tests (lib + integration + doc tests)
cargo test --all-features

# Minimal build tests
cargo test --no-default-features

# Run specific test
cargo test --test integration_test -- --nocapture
```

## Performance Considerations

- **Parsing is synchronous**: No async/await support
- **Memory overhead**: Tracks all parsing candidates and transformations
- **Strategy execution**: Some strategies run in parallel
- **Regex compilation**: Expensive regexes are compiled lazily and cached
- **Best for**: <1MB inputs, occasional parsing (not high-frequency loops)

Optimizations:
- Direct JSON (valid JSON) takes the fastest path
- Failed strategies short-circuit early
- Scoring is lazy (only computed when needed)
- Copy semantics for small types (enums, etc.)

## Debugging

Enable detailed logging:

```rust
env_logger::init();
std::env::set_var("RUST_LOG", "tryparse=debug");

let result = parse::<User>(input);
```

Inspect what went wrong:

```rust
match parse::<User>(input) {
    Ok(user) => println!("Success: {:?}", user),
    Err(e) => {
        eprintln!("Parse failed: {:?}", e);

        // Try getting candidates to see what was parsed
        if let Ok((_, candidates)) = parse_with_candidates::<User>(input) {
            for c in candidates {
                eprintln!("Candidate: {:?}", c.value);
            }
        }
    }
}
```

## Known Limitations

1. **Synchronous only** - No async parsing
2. **No streaming** - Requires complete input string
3. **Memory overhead** - Tracks all candidates and transformations
4. **Acronym handling** - `XMLParser` → `x_m_l_parser` (not `xml_parser`)
5. **Best-effort parsing** - May produce unexpected results on ambiguous input
6. **No custom deserializers** - Can't implement custom `Deserialize` logic for fields
7. **Internally-tagged enum fields** - Tag values support fuzzy matching, but variant fields use standard serde deserialization (exact match or serde's `rename_all`)

## Contributing

Requirements:
- Rust 1.70.0+
- Run `cargo fmt` before committing
- Pass `cargo clippy --all-targets --all-features`
- All tests must pass: `cargo test --all-features`

Pull request checklist:
1. Clear description of what and why
2. Tests for new functionality
3. Update README if API changes
4. No clippy warnings
5. All existing tests pass

## License

Apache-2.0

## Credits

Parsing algorithms inspired by [BAML's Schema-Aligned Parsing](https://github.com/BoundaryML/baml).
