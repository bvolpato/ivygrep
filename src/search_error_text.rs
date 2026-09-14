// Pasted error output mixes static message text, which the code that raises
// the error contains verbatim, with values interpolated at runtime: absolute
// paths, URLs, hex ids, and numbers. Those values usually name the user's
// machine or environment (`/home/dev/.local/share/<app>/...`), so as query
// terms they pull in files by directory names. This module separates the two.
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Bounds the work spent on long pasted logs. Lines past the bound keep their
/// text in `retrieval_text` but are not classified or split into fragments.
const MAX_ERROR_QUERY_LINES: usize = 200;
/// Bare operator tokens mark a type or expression, such as `string | null` or
/// `Optional[str] = None`, rather than a message.
const OPERATOR_TOKENS: [&str; 10] = ["|", "||", "&&", "=", "==", "===", "!=", "=>", "->", ":="];
const MAX_MESSAGE_FRAGMENTS: usize = 8;
const MIN_FRAGMENT_WORDS: usize = 2;
const MIN_FRAGMENT_CHARS: usize = 12;

pub(super) struct PastedError {
    /// The query with interpolated values removed, for token-based retrieval.
    /// Absolute paths inside the workspace keep their workspace-relative form.
    pub(super) retrieval_text: String,
    /// Static message text between interpolated values, for literal matching.
    pub(super) fragments: Vec<String>,
}

/// Error labels at the start of a line: `Error:`, `error[E0425]:`, `Caused by:`,
/// `0: ` chain entries, `ValueError:`, `java.lang.IllegalStateException:`,
/// `Uncaught TypeError:`, `Exception in thread "main" ...:`, pytest `E   `.
static ERROR_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?:E\s+)?(?:\d+:\s+)?(?:\[(?i:error|fatal)\]\s*)?(?:(?i:error|fatal|panic|caused by)|Uncaught\s+[A-Za-z_$][\w$]*|Exception in thread\s+"[^"]*"\s+[\w$.]+|(?:[A-Za-z_$][\w$]*\.)*[A-Za-z_$][\w$]*(?:Error|Exception))(?:\[[^\]]*\])?:(?:\s|$)"#,
    )
    .expect("valid error label regex")
});

static ERROR_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^Traceback \(most recent call last\):|^\s*\[(?i:error|fatal)\]|\(os error \d+\)|\bpanicked at\b",
    )
    .expect("valid error marker regex")
});

/// Stack frames, traceback headers, and other framing that carries no message.
/// The last alternative is a whole Go frame line, `\t/src/app/main.go:44 +0x1e`.
static FRAMING_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?:at\s|File\s+"|\.\.\.\s+\d+\s+more|Traceback \(most recent call last\)|goroutine\s+\d+\s+\[|exit status\s+\d+|[\^~]+\s*$|\S+:\d+\s+\+0x[0-9a-fA-F]+\s*$)"#,
    )
    .expect("valid framing regex")
});

impl PastedError {
    pub(super) fn parse(query: &str, workspace_root: &Path) -> Option<Self> {
        let lines = query
            .lines()
            .take(MAX_ERROR_QUERY_LINES)
            .collect::<Vec<_>>();
        let detected = lines.iter().enumerate().any(|(index, line)| {
            ERROR_MARKER.is_match(line)
                || ERROR_LABEL
                    .find(line)
                    .is_some_and(|label| is_message_label(&lines, index, label.end()))
        });
        if !detected {
            return None;
        }

        let root = workspace_root.to_string_lossy().replace('\\', "/");
        let root = root.trim_end_matches('/');
        let mut retrieval_lines = Vec::with_capacity(lines.len());
        let mut fragments = Vec::new();
        for line in &lines {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let mut kept = Vec::with_capacity(words.len());
            for word in &words {
                match classify_word(word) {
                    WordKind::AbsolutePath(path) => {
                        if let Some(relative) = workspace_relative(path, root) {
                            kept.push(relative.to_string());
                        }
                    }
                    WordKind::Volatile => {}
                    WordKind::Quoted | WordKind::RelativePath | WordKind::Text => {
                        kept.push((*word).to_string());
                    }
                }
            }
            retrieval_lines.push(kept.join(" "));

            if FRAMING_LINE.is_match(line) {
                continue;
            }
            let message = ERROR_LABEL
                .find(line)
                .map_or(*line, |label| &line[label.end()..]);
            collect_fragments(message, &mut fragments);
        }

        retrieval_lines.extend(
            query
                .lines()
                .skip(MAX_ERROR_QUERY_LINES)
                .map(str::to_string),
        );
        let retrieval_text = retrieval_lines.join("\n").trim().to_string();
        if !retrieval_text.chars().any(|ch| ch.is_ascii_alphanumeric()) {
            return None;
        }
        let mut seen = std::collections::HashSet::new();
        fragments.retain(|fragment| seen.insert(fragment.to_ascii_lowercase()));
        fragments.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
        fragments.truncate(MAX_MESSAGE_FRAGMENTS);
        Some(Self {
            retrieval_text,
            fragments,
        })
    }
}

/// Whether a line-leading label introduces a message rather than a field in
/// pasted source, such as `error: string | null;`, `fatal: true`, a YAML key,
/// or a docstring `Raises:` entry. `Caused by:` headers always count.
fn is_message_label(lines: &[&str], index: usize, label_end: usize) -> bool {
    let line = lines[index];
    if line
        .trim_start()
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("caused by:"))
    {
        return true;
    }
    if line.trim_end().ends_with([',', ';', '{', '(', '[', '=']) {
        return false;
    }
    let remainder = line[label_end..].trim();
    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    let words = tokens
        .iter()
        .filter(|token| token.chars().any(char::is_alphabetic))
        .count();
    if words < 2
        || tokens.iter().any(|token| OPERATOR_TOKENS.contains(token))
        || is_single_quoted(remainder)
    {
        return false;
    }
    // An entry nested under a line ending in `:`, `{`, `(` or `[` is a field,
    // key, or docstring section item. Test runners nest real messages under
    // prose lines such as `● suite › test`, which still count.
    !parent_line(lines, index)
        .is_some_and(|parent| parent.trim_end().ends_with([':', '{', '(', '[']))
}

fn is_single_quoted(text: &str) -> bool {
    let text = text.trim_end_matches([',', ';']);
    let mut chars = text.chars();
    match (chars.next(), chars.next_back()) {
        (Some(first), Some(last)) if matches!(first, '\'' | '"' | '`') && first == last => {
            !chars.as_str().contains(first)
        }
        _ => false,
    }
}

/// Nearest earlier non-empty line indented less than `lines[index]`.
fn parent_line<'a>(lines: &[&'a str], index: usize) -> Option<&'a str> {
    let own = indentation(lines[index]);
    if own == 0 {
        return None;
    }
    lines[..index]
        .iter()
        .rev()
        .copied()
        .find(|line| !line.trim().is_empty() && indentation(line) < own)
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

enum WordKind<'a> {
    AbsolutePath(&'a str),
    RelativePath,
    Quoted,
    Volatile,
    Text,
}

fn classify_word(word: &str) -> WordKind<'_> {
    let quoted = word.starts_with(['\'', '"', '`', '\u{2018}', '\u{201c}'])
        && word
            .trim_end_matches([',', ';', ':', '.', ')', '!', '?'])
            .ends_with(['\'', '"', '`', '\u{2019}', '\u{201d}']);
    let core = word.trim_matches(|ch: char| {
        matches!(
            ch,
            '\'' | '"'
                | '`'
                | '\u{2018}'
                | '\u{2019}'
                | '\u{201c}'
                | '\u{201d}'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | ','
                | ';'
                | ':'
                | '.'
                | '!'
                | '?'
        )
    });
    if core.is_empty() {
        return WordKind::Text;
    }
    if core.contains("://") {
        return WordKind::Volatile;
    }
    let bytes = core.as_bytes();
    let windows_drive = bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    if core.starts_with('/') || core.starts_with("~/") || core.starts_with("\\\\") || windows_drive
    {
        return WordKind::AbsolutePath(core);
    }
    if is_number_like(core) || is_hex_id(core) {
        return WordKind::Volatile;
    }
    if core.contains('/') || core.contains('\\') {
        return WordKind::RelativePath;
    }
    if quoted {
        return WordKind::Quoted;
    }
    WordKind::Text
}

fn is_number_like(core: &str) -> bool {
    let core = core
        .strip_prefix("0x")
        .filter(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map_or(core, |_| "0");
    core.bytes().any(|byte| byte.is_ascii_digit())
        && core.bytes().all(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'.' | b':' | b',' | b'-' | b'+' | b'x')
        })
}

fn is_hex_id(core: &str) -> bool {
    let hex = core
        .bytes()
        .filter(|byte| *byte != b'-')
        .collect::<Vec<_>>();
    hex.len() >= 7
        && hex.iter().all(u8::is_ascii_hexdigit)
        && hex.iter().any(u8::is_ascii_digit)
        && hex.iter().any(u8::is_ascii_alphabetic)
}

fn workspace_relative<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        return None;
    }
    let normalized = path.replace('\\', "/");
    let remainder = normalized.strip_prefix(root)?.strip_prefix('/')?;
    (!remainder.is_empty()).then(|| &path[path.len() - remainder.len()..])
}

/// Splits one message line into runs of static text. Interpolated values,
/// quoted values without spaces, relative paths, `: ` chain separators, and
/// sentence ends end a run, so every run is a substring of the format string
/// that produced it. Long messages are often split into adjacent source
/// literals at sentence boundaries.
fn collect_fragments(message: &str, fragments: &mut Vec<String>) {
    let mut current: Vec<&str> = Vec::new();
    for word in message.split_whitespace() {
        let boundary = !matches!(classify_word(word), WordKind::Text);
        if boundary {
            push_fragment(&mut current, fragments);
            continue;
        }
        let starts_group =
            word.starts_with(['\'', '"', '`', '\u{2018}', '\u{201c}', '(', '[', '{']);
        if starts_group {
            push_fragment(&mut current, fragments);
        }
        current.push(word);
        let ends_run = word.ends_with([':', '.', '?', '!'])
            || word
                .trim_end_matches([',', ';', ':', '.', '!', '?'])
                .ends_with(['\'', '"', '`', '\u{2019}', '\u{201d}']);
        if ends_run {
            push_fragment(&mut current, fragments);
        }
    }
    push_fragment(&mut current, fragments);
}

fn push_fragment(words: &mut Vec<&str>, fragments: &mut Vec<String>) {
    if words.is_empty() {
        return;
    }
    let joined = words.join(" ");
    words.clear();
    let fragment = joined.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
    let word_count = fragment
        .split_whitespace()
        .filter(|word| word.bytes().filter(u8::is_ascii_alphabetic).count() >= 2)
        .count();
    if word_count >= MIN_FRAGMENT_WORDS && fragment.len() >= MIN_FRAGMENT_CHARS {
        fragments.push(fragment.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::PastedError;

    fn parse(query: &str) -> PastedError {
        PastedError::parse(query, Path::new("/work/app")).expect("pasted error output")
    }

    #[test]
    fn anyhow_chain_keeps_message_and_drops_environment_values() {
        let error = parse(
            "Error: failed to acquire index lock /home/dev/.local/share/app/indexes/7c41e0a9d2f3/index.lock\n\nCaused by:\n    Resource temporarily unavailable (os error 11)",
        );
        assert!(!error.retrieval_text.contains("share"));
        assert!(!error.retrieval_text.contains("7c41e0a9d2f3"));
        assert!(
            error
                .retrieval_text
                .contains("failed to acquire index lock")
        );
        assert_eq!(
            error.fragments,
            vec![
                "Resource temporarily unavailable".to_string(),
                "failed to acquire index lock".to_string(),
            ]
        );
    }

    #[test]
    fn quoted_values_and_chain_separators_end_fragments() {
        let error = parse(
            "Traceback (most recent call last):\n  File \"/home/dev/app/main.py\", line 12, in <module>\n    main()\nInvalidURL: Invalid percent-escape sequence: 'settings'",
        );
        assert_eq!(
            error.fragments,
            vec!["Invalid percent-escape sequence".to_string()]
        );
        assert!(!error.retrieval_text.contains("home"));
    }

    #[test]
    fn workspace_paths_stay_workspace_relative() {
        let error = parse("Error: cannot parse /work/app/src/config.toml: expected `=`");
        assert!(error.retrieval_text.contains("src/config.toml"));
        assert!(!error.retrieval_text.contains("/work/app"));
    }

    #[test]
    fn pasted_source_is_not_error_output() {
        for query in [
            "def main():\n    raise ValueError(\"bad value\")",
            "if err != nil {\n    return fmt.Errorf(\"open %s: %w\", path, err)\n}",
            "failed to acquire index lock",
            // Field lines whose key is an error label.
            "interface State {\n  loading: boolean;\n  error: string | null;\n}",
            "return res.status(500).json({\n  error: err.message,\n});",
            "responses:\n  '400':\n    error:\n      type: object",
            "type Payload {\n  error: String\n  data: User\n}",
            "struct Response {\n    error: Option<String>,\n}",
            "    error: str",
            "config {\n  fatal: true\n}",
            "export interface ApiResult {\n  data?: User\n  error: string | null\n}",
            "class Result:\n    value: int = 0\n    error: Optional[str] = None",
            "res.status(404).json({\n  error: 'Not found'\n})",
            "en:\n  errors:\n    error: Something went wrong",
            "def withdraw(self, amount):\n    \"\"\"Withdraw money.\n\n    Raises:\n        ValueError: If the amount is negative.\n    \"\"\"",
            "type Result struct {\n\tError error\n}\n\n// Error handling in the daemon",
        ] {
            assert!(
                PastedError::parse(query, Path::new("/work/app")).is_none(),
                "{query}"
            );
        }
    }

    #[test]
    fn test_runner_messages_nested_under_prose_lines_are_error_output() {
        for query in [
            " FAIL  src/Button.test.tsx\n  ● Button › renders the label\n\n    TypeError: Cannot read properties of undefined (reading 'label')\n\n      at Object.<anonymous> (src/Button.test.tsx:14:35)",
            "  1) UserService\n       creates a user:\n     Error: Validation failed: email is required\n      at Context.<anonymous> (test/user.test.js:22:11)",
        ] {
            assert!(
                PastedError::parse(query, Path::new("/work/app")).is_some(),
                "{query}"
            );
        }
    }

    #[test]
    fn lines_past_the_classification_bound_stay_in_retrieval_text() {
        let mut query = String::from("Error: failed to open workspace cache\n");
        for index in 0..250 {
            query.push_str(&format!("    worker {index} still waiting\n"));
        }
        query.push_str("    tail_only_marker reached\n");
        assert!(parse(&query).retrieval_text.contains("tail_only_marker"));
    }
}
