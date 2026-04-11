use pluralizer::pluralize;

use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Tokenizer name registered with every Tantivy index.
pub const CODE_TOKENIZER_NAME: &str = "code";

/// A code-aware tokenizer that splits text on whitespace, punctuation,
/// camelCase boundaries, and underscore separators — producing lowercase
/// tokens that match natural-language queries against code identifiers.
///
/// For example, `calculateTaxTotal` emits `["calculate", "tax", "total"]`
/// and `std::io::Write` emits `["std", "io", "write"]`.
#[derive(Clone, Default)]
pub struct CodeTokenizer;

pub struct CodeTokenStream {
    tokens: Vec<Token>,
    index: usize,
    token: Token,
}

impl Tokenizer for CodeTokenizer {
    type TokenStream<'a> = CodeTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let mut tokens = Vec::new();
        let mut offset = 0usize;

        for word in text.split(|ch: char| ch.is_whitespace() || is_code_separator(ch)) {
            if word.is_empty() {
                offset += 1; // separator char
                continue;
            }

            let segments = split_identifier_segments(word);
            let mut local_offset = offset;
            for seg in &segments {
                let tok = Token {
                    offset_from: local_offset,
                    offset_to: local_offset + seg.len(),
                    position: tokens.len(),
                    text: seg.clone(),
                    position_length: 1,
                };
                tokens.push(tok);
                local_offset += seg.len();
            }

            offset += word.len() + 1; // +1 for the separator
        }

        CodeTokenStream {
            tokens,
            index: 0,
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

impl TokenStream for CodeTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.token = self.tokens[self.index].clone();
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
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
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;
    let mut prev_is_alpha = false;

    for ch in token.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                segments.push(current.to_ascii_lowercase());
                current.clear();
            }
            prev_is_lower = false;
            prev_is_alpha = false;
            continue;
        }

        let is_upper = ch.is_ascii_uppercase();
        let is_alpha = ch.is_ascii_alphabetic();

        if !current.is_empty() && is_upper && prev_is_lower {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        if !current.is_empty() && is_alpha != prev_is_alpha {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        current.push(ch);
        prev_is_lower = ch.is_ascii_lowercase();
        prev_is_alpha = is_alpha;
    }

    if !current.is_empty() {
        segments.push(current.to_ascii_lowercase());
    }

    segments
}

/// Returns the singular form of a token, or the token unchanged if it's too
/// short or non-alphabetic.
pub fn singularize_token(token: &str) -> String {
    if token.len() <= 3 || !token.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return token.to_string();
    }

    let singular = pluralize(token, 1isize, false).to_ascii_lowercase();
    if singular.is_empty() {
        token.to_string()
    } else {
        singular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
