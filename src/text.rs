use std::ops::Range;

use smallvec::SmallVec;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Tokenizer name registered with every Tantivy index.
pub const CODE_TOKENIZER_NAME: &str = "code";
pub const TRIGRAM_TOKENIZER_NAME: &str = "trigram";

#[derive(Clone, Default)]
pub struct AsciiTrigramTokenizer {
    token: Token,
}

pub struct AsciiTrigramTokenStream<'a> {
    text: &'a str,
    cursor: usize,
    token: &'a mut Token,
}

impl Tokenizer for AsciiTrigramTokenizer {
    type TokenStream<'a> = AsciiTrigramTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        self.token.reset();
        AsciiTrigramTokenStream {
            text,
            cursor: 0,
            token: &mut self.token,
        }
    }
}

impl TokenStream for AsciiTrigramTokenStream<'_> {
    fn advance(&mut self) -> bool {
        let bytes = self.text.as_bytes();
        while self.cursor.saturating_add(3) <= bytes.len() {
            let start = self.cursor;
            self.cursor += 1;
            let trigram = &bytes[start..start + 3];
            if !trigram.iter().all(u8::is_ascii_alphanumeric) {
                continue;
            }

            self.token.offset_from = start;
            self.token.offset_to = start + 3;
            self.token.position = start;
            self.token.text.clear();
            self.token
                .text
                .extend(trigram.iter().map(u8::to_ascii_lowercase).map(char::from));
            return true;
        }
        false
    }

    fn token(&self) -> &Token {
        self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        self.token
    }
}

pub fn build_trigram_analyzer() -> tantivy::tokenizer::TextAnalyzer {
    tantivy::tokenizer::TextAnalyzer::from(AsciiTrigramTokenizer::default())
}

/// A code-aware tokenizer that splits text on whitespace, punctuation,
/// camelCase boundaries, and underscore separators — producing lowercase
/// tokens that match natural-language queries against code identifiers.
///
/// For example, `calculateTaxTotal` emits `["calculate", "tax", "total"]`
/// and `std::io::Write` emits `["std", "io", "write"]`.
#[derive(Clone, Default)]
pub struct CodeTokenizer;

pub struct CodeTokenStream<'a> {
    text: &'a str,
    cursor: usize,
    pending: SmallVec<[PendingSegment; 8]>,
    pending_index: usize,
    position: usize,
    token: Token,
}

#[derive(Clone, Copy, Default)]
struct PendingSegment {
    offset_from: usize,
    offset_to: usize,
}

impl Tokenizer for CodeTokenizer {
    type TokenStream<'a> = CodeTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        CodeTokenStream {
            text,
            cursor: 0,
            pending: SmallVec::new(),
            pending_index: 0,
            position: 0,
            token: Token::default(),
        }
    }
}

fn is_code_separator(ch: char) -> bool {
    matches!(
        ch,
        '.' | ':'
            | '/'
            | '\\'
            | '-'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '='
            | ','
            | ';'
            | '"'
            | '\''
            | '`'
            | '!'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '+'
            | '|'
            | '~'
            | '?'
    )
}

impl TokenStream for CodeTokenStream<'_> {
    fn advance(&mut self) -> bool {
        loop {
            if self.pending_index < self.pending.len() {
                let segment = self.pending[self.pending_index];
                self.pending_index += 1;
                self.token.offset_from = segment.offset_from;
                self.token.offset_to = segment.offset_to;
                self.token.position = self.position;
                self.token.text.clear();
                push_ascii_lowercase(
                    &mut self.token.text,
                    &self.text[segment.offset_from..segment.offset_to],
                );
                self.token.position_length = 1;
                self.position += 1;
                return true;
            }

            if !self.fill_next_word_segments() {
                return false;
            }
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

impl CodeTokenStream<'_> {
    fn fill_next_word_segments(&mut self) -> bool {
        self.pending.clear();
        self.pending_index = 0;

        while let Some(range) = self.next_word_range() {
            split_identifier_segments_with_offsets(
                &self.text[range.clone()],
                range.start,
                &mut self.pending,
            );
            if !self.pending.is_empty() {
                return true;
            }
        }

        false
    }

    fn next_word_range(&mut self) -> Option<Range<usize>> {
        while self.cursor < self.text.len() {
            let ch = self.text[self.cursor..].chars().next()?;
            if ch.is_whitespace() || is_code_separator(ch) {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }

        if self.cursor >= self.text.len() {
            return None;
        }

        let start = self.cursor;
        while self.cursor < self.text.len() {
            let ch = self.text[self.cursor..].chars().next()?;
            if ch.is_whitespace() || is_code_separator(ch) {
                break;
            }
            self.cursor += ch.len_utf8();
        }

        Some(start..self.cursor)
    }
}

/// Build a [`tantivy::tokenizer::TextAnalyzer`] using the code-aware tokenizer.
pub fn build_code_analyzer() -> tantivy::tokenizer::TextAnalyzer {
    tantivy::tokenizer::TextAnalyzer::builder(CodeTokenizer)
        .filter(tantivy::tokenizer::RemoveLongFilter::limit(80))
        .build()
}

/// Splits an identifier like `calculateTaxTotal` or `snake_case_name` into
/// lowercase segments: `["calculate", "tax", "total"]`.
pub fn split_identifier_segments(token: &str) -> Vec<String> {
    let mut segments = SmallVec::<[PendingSegment; 8]>::new();
    split_identifier_segments_with_offsets(token, 0, &mut segments);
    segments
        .into_iter()
        .map(|segment| token[segment.offset_from..segment.offset_to].to_ascii_lowercase())
        .collect()
}

fn split_identifier_segments_with_offsets(
    token: &str,
    base_offset: usize,
    segments: &mut SmallVec<[PendingSegment; 8]>,
) {
    if token.is_ascii() {
        split_ascii_identifier_segments_with_offsets(token.as_bytes(), base_offset, segments);
        return;
    }

    let mut current_start = None;
    let mut current_end = 0usize;
    let mut prev_is_lower = false;
    let mut prev_is_alpha = false;

    for (offset, ch) in token.char_indices() {
        if !ch.is_ascii_alphanumeric() {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
            prev_is_lower = false;
            prev_is_alpha = false;
            continue;
        }

        let is_upper = ch.is_ascii_uppercase();
        let is_alpha = ch.is_ascii_alphabetic();

        if current_start.is_some() && is_upper && prev_is_lower {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
        }

        if current_start.is_some() && is_alpha != prev_is_alpha {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
        }

        if current_start.is_none() {
            current_start = Some(offset);
        }
        current_end = offset + ch.len_utf8();
        prev_is_lower = ch.is_ascii_lowercase();
        prev_is_alpha = is_alpha;
    }

    push_current_segment(
        segments,
        &mut current_start,
        base_offset,
        base_offset + current_end,
    );
}

fn split_ascii_identifier_segments_with_offsets(
    token: &[u8],
    base_offset: usize,
    segments: &mut SmallVec<[PendingSegment; 8]>,
) {
    if token.iter().all(|byte| byte.is_ascii_lowercase())
        || token.iter().all(|byte| byte.is_ascii_digit())
    {
        if !token.is_empty() {
            segments.push(PendingSegment {
                offset_from: base_offset,
                offset_to: base_offset + token.len(),
            });
        }
        return;
    }

    let mut current_start = None;
    let mut current_end = 0usize;
    let mut prev_is_lower = false;
    let mut prev_is_alpha = false;

    for (offset, byte) in token.iter().copied().enumerate() {
        if !byte.is_ascii_alphanumeric() {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
            prev_is_lower = false;
            prev_is_alpha = false;
            continue;
        }

        let is_upper = byte.is_ascii_uppercase();
        let is_alpha = byte.is_ascii_alphabetic();

        if current_start.is_some() && is_upper && prev_is_lower {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
        }

        if current_start.is_some() && is_alpha != prev_is_alpha {
            push_current_segment(
                segments,
                &mut current_start,
                base_offset,
                base_offset + current_end,
            );
        }

        if current_start.is_none() {
            current_start = Some(offset);
        }
        current_end = offset + 1;
        prev_is_lower = byte.is_ascii_lowercase();
        prev_is_alpha = is_alpha;
    }

    push_current_segment(
        segments,
        &mut current_start,
        base_offset,
        base_offset + current_end,
    );
}

fn push_current_segment(
    segments: &mut SmallVec<[PendingSegment; 8]>,
    current_start: &mut Option<usize>,
    base_offset: usize,
    offset_to: usize,
) {
    let Some(offset_from) = current_start.take() else {
        return;
    };
    segments.push(PendingSegment {
        offset_from: base_offset + offset_from,
        offset_to,
    });
}

fn push_ascii_lowercase(output: &mut String, text: &str) {
    if text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        output.extend(text.bytes().map(|byte| byte.to_ascii_lowercase() as char));
    } else {
        output.push_str(text);
    }
}

/// Returns the singular form of a token, or the token unchanged if it's too
/// short or non-alphabetic.
pub fn singularize_token(token: &str) -> String {
    if token.len() <= 3 || !token.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return token.to_string();
    }

    let token = token.to_ascii_lowercase();
    if let Some(singular) = irregular_singular(&token) {
        return singular.to_string();
    }

    if let Some(stem) = token.strip_suffix("ies")
        && stem.len() >= 2
    {
        return format!("{stem}y");
    }

    for suffix in ["ches", "shes", "sses", "xes", "zes"] {
        if let Some(stem) = token.strip_suffix(suffix)
            && stem.len() >= 2
        {
            return format!("{stem}{}", &suffix[..suffix.len() - 2]);
        }
    }

    if let Some(stem) = token.strip_suffix("s")
        && !token.ends_with("ss")
        && !token.ends_with("us")
        && !token.ends_with("is")
        && stem.len() >= 3
    {
        return stem.to_string();
    }

    token
}

fn irregular_singular(token: &str) -> Option<&'static str> {
    match token {
        "aliases" => Some("alias"),
        "analyses" => Some("analysis"),
        "buses" => Some("bus"),
        "children" => Some("child"),
        "criteria" => Some("criterion"),
        "feet" => Some("foot"),
        "geese" => Some("goose"),
        "indices" => Some("index"),
        "matrices" => Some("matrix"),
        "men" => Some("man"),
        "mice" => Some("mouse"),
        "people" => Some("person"),
        "statuses" => Some("status"),
        "teeth" => Some("tooth"),
        "vertices" => Some("vertex"),
        "women" => Some("woman"),
        _ => None,
    }
}

/// Returns the first source line after generated path headers, documentation
/// comments, and declaration attributes.
pub fn first_code_line_range(text: &str) -> Option<Range<usize>> {
    let mut offset = 0;
    let mut in_block_comment = false;

    for line in text.split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        let line_without_newline = line_without_newline
            .strip_suffix('\r')
            .unwrap_or(line_without_newline);
        let trimmed = line_without_newline.trim();

        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            offset += line.len();
            continue;
        }

        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            offset += line.len();
            continue;
        }

        // Annotations and attributes that share a line with the declaration
        // (`@Override public void run() {`) are stripped rather than hiding
        // the declaration behind them.
        let code = strip_leading_annotations(trimmed);
        if code.is_empty()
            || code.starts_with("//")
            || code.starts_with('#')
            || code.starts_with('*')
            || code.starts_with('@')
            || code.starts_with("*/")
            || code.starts_with('[') && code.ends_with(']')
        {
            offset += line.len();
            continue;
        }

        let leading_bytes = line_without_newline.len() - line_without_newline.trim_start().len()
            + (trimmed.len() - code.len());
        let trailing_bytes = line_without_newline.len() - line_without_newline.trim_end().len();
        return Some(offset + leading_bytes..offset + line_without_newline.len() - trailing_bytes);
    }

    None
}

/// Strips leading annotation / decorator / attribute tokens (`@Name`,
/// `@Name(...)`, `#[...]`) from a trimmed line and returns the remaining
/// declaration text. A line that only holds annotations becomes empty.
pub fn strip_leading_annotations(mut line: &str) -> &str {
    loop {
        if let Some(rest) = line.strip_prefix('@') {
            let identifier_end = rest
                .find(|character: char| {
                    !character.is_ascii_alphanumeric() && !matches!(character, '_' | '.' | '$')
                })
                .unwrap_or(rest.len());
            if identifier_end == 0 {
                return line;
            }
            let mut rest = &rest[identifier_end..];
            if rest.starts_with('(') {
                match matching_close_offset(rest, b'(', b')') {
                    Some(close) => rest = &rest[close + 1..],
                    None => return "",
                }
            }
            line = rest.trim_start();
            continue;
        }
        if line.starts_with("#[") {
            match matching_close_offset(line, b'[', b']') {
                Some(close) => line = line[close + 1..].trim_start(),
                None => return "",
            }
            continue;
        }
        return line;
    }
}

fn matching_close_offset(text: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in text.bytes().enumerate() {
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_annotations_are_stripped_from_declarations() {
        assert_eq!(
            strip_leading_annotations("@Override public void run() {"),
            "public void run() {"
        );
        assert_eq!(
            strip_leading_annotations(
                "@Route('/x', methods = {\"GET\"}) @Auth public function index()"
            ),
            "public function index()"
        );
        assert_eq!(
            strip_leading_annotations("#[inline] pub fn method(&self) {}"),
            "pub fn method(&self) {}"
        );
        assert_eq!(strip_leading_annotations("@property"), "");
        assert_eq!(strip_leading_annotations("@Route('/x'"), "");
        assert_eq!(
            strip_leading_annotations("pub fn plain() {}"),
            "pub fn plain() {}"
        );
        assert_eq!(strip_leading_annotations("@"), "@");

        let text = "// src/Worker.java\n\n@Override public void run() {\n";
        let range = first_code_line_range(text).unwrap();
        assert_eq!(&text[range], "public void run() {");
    }

    #[test]
    fn camel_case_split() {
        assert_eq!(
            split_identifier_segments("calculateTaxTotal"),
            vec!["calculate", "tax", "total"]
        );
    }

    #[test]
    fn snake_case_split() {
        assert_eq!(
            split_identifier_segments("apply_tax_rate"),
            vec!["apply", "tax", "rate"]
        );
    }

    #[test]
    fn upper_camel_case_split() {
        assert_eq!(
            split_identifier_segments("HttpResponseCode"),
            vec!["http", "response", "code"]
        );
    }

    #[test]
    fn single_word() {
        assert_eq!(split_identifier_segments("filter"), vec!["filter"]);
    }

    #[test]
    fn singularize_basic() {
        assert_eq!(singularize_token("taxes"), "tax");
        assert_eq!(singularize_token("limits"), "limit");
        assert_eq!(singularize_token("queries"), "query");
        assert_eq!(singularize_token("classes"), "class");
        assert_eq!(singularize_token("files"), "file");
        assert_eq!(singularize_token("status"), "status");
        assert_eq!(singularize_token("analysis"), "analysis");
        assert_eq!(singularize_token("indices"), "index");
        assert_eq!(singularize_token("statuses"), "status");
        assert_eq!(singularize_token("aliases"), "alias");
    }

    #[test]
    fn singularize_short_passthrough() {
        assert_eq!(singularize_token("id"), "id");
        assert_eq!(singularize_token("a"), "a");
    }

    #[test]
    fn singularize_non_alpha_passthrough() {
        assert_eq!(singularize_token("test123"), "test123");
    }

    #[test]
    fn first_code_line_skips_java_doc_and_annotation() {
        let text = "// src/GsonBuilder.java\n\n/**\n * Registers an adapter.\n */\n@CanIgnoreReturnValue\npublic GsonBuilder registerTypeAdapter(Type type, Object adapter) {\n";
        let range = first_code_line_range(text).unwrap();

        assert_eq!(
            &text[range],
            "public GsonBuilder registerTypeAdapter(Type type, Object adapter) {"
        );
    }

    #[test]
    fn first_code_line_skips_csharp_attribute() {
        let text = "// Example.cs\n\n/// Docs\n[Obsolete]\npublic void RegisterHandler() {}\n";
        let range = first_code_line_range(text).unwrap();

        assert_eq!(&text[range], "public void RegisterHandler() {}");
    }

    fn collect_tokens(text: &str) -> Vec<String> {
        use tantivy::tokenizer::Tokenizer;
        let mut tokenizer = CodeTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().text.clone());
        }
        tokens
    }

    #[test]
    fn code_tokenizer_preserves_segment_offsets() {
        use tantivy::tokenizer::Tokenizer;
        let mut tokenizer = CodeTokenizer;
        let mut stream = tokenizer.token_stream("src/calculateTax_total42.rs");
        let mut tokens = Vec::new();
        while stream.advance() {
            let token = stream.token();
            tokens.push((token.text.clone(), token.offset_from, token.offset_to));
        }

        assert_eq!(
            tokens,
            [
                ("src".to_string(), 0, 3),
                ("calculate".to_string(), 4, 13),
                ("tax".to_string(), 13, 16),
                ("total".to_string(), 17, 22),
                ("42".to_string(), 22, 24),
                ("rs".to_string(), 25, 27),
            ]
        );
    }

    #[test]
    fn code_tokenizer_camel_case() {
        assert_eq!(
            collect_tokens("calculateTaxTotal"),
            vec!["calculate", "tax", "total"]
        );
    }

    #[test]
    fn code_tokenizer_snake_case() {
        assert_eq!(
            collect_tokens("apply_tax_rate"),
            vec!["apply", "tax", "rate"]
        );
    }

    #[test]
    fn code_tokenizer_path_separators() {
        assert_eq!(collect_tokens("std::io::Write"), vec!["std", "io", "write"]);
    }

    #[test]
    fn code_tokenizer_dot_separators() {
        assert_eq!(
            collect_tokens("com.example.MyClass"),
            vec!["com", "example", "my", "class"]
        );
    }

    #[test]
    fn code_tokenizer_function_signature() {
        assert_eq!(
            collect_tokens("pub fn handleError(code: i32)"),
            vec!["pub", "fn", "handle", "error", "code", "i", "32"]
        );
    }

    #[test]
    fn code_tokenizer_file_path() {
        assert_eq!(
            collect_tokens("src/auth/login_handler.rs"),
            vec!["src", "auth", "login", "handler", "rs"]
        );
    }

    #[test]
    fn code_tokenizer_natural_language_query() {
        assert_eq!(collect_tokens("calculate tax"), vec!["calculate", "tax"]);
    }
}
