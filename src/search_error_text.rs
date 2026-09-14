// Pasted error output mixes static message text, which the code that raises
// the error contains verbatim, with values interpolated at runtime: absolute
// paths, URLs, hex ids, and numbers. Those values usually name the user's
// machine or environment (`/home/dev/.local/share/<app>/...`), so as query
// terms they pull in files by directory names. This module separates the two.
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Bounds the work spent on long pasted logs. Lines past the bound keep their
/// text in `retrieval_text` but are not classified or split into fragments.
const MAX_ERROR_QUERY_LINES: usize = 200;
/// Bare operator tokens mark a type or expression, such as `string | null` or
/// `Optional[str] = None`, rather than a message.
const OPERATOR_TOKENS: [&str; 10] = ["|", "||", "&&", "=", "==", "===", "!=", "=>", "->", ":="];
/// Opening and closing quotes that can wrap a value containing spaces.
const QUOTE_PAIRS: [(char, char); 5] = [
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
    ('\u{201c}', '\u{201d}'),
    ('\u{2018}', '\u{2019}'),
];
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

/// One date, time, or timestamp token: `2024-05-14`, `2024/05/14`,
/// `2024-05-14T10:32:11.123Z`, `[10:32:11,123]`.
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\[?(?:\d{4}[-/.]\d{2}[-/.]\d{2}(?:[T_]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?)?|\d{2}:\d{2}:\d{2}(?:[.,]\d+)?)(?:Z|[+-]\d{2}:?\d{2})?\]?,?$",
    )
    .expect("valid timestamp regex")
});

impl PastedError {
    pub(super) fn parse(query: &str, workspace_root: &Path) -> Option<Self> {
        let lines = query
            .lines()
            .take(MAX_ERROR_QUERY_LINES)
            .collect::<Vec<_>>();
        let counted_labels = lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                ERROR_LABEL
                    .find(line)
                    .is_some_and(|label| is_message_label(&lines, index, label.end()))
            })
            .collect::<Vec<_>>();
        if !counted_labels.contains(&true) && !lines.iter().any(|line| ERROR_MARKER.is_match(line))
        {
            return None;
        }

        let root = workspace_root.to_string_lossy().replace('\\', "/");
        let root = root.trim_end_matches('/');
        let mut retrieval_lines = Vec::with_capacity(lines.len());
        let mut fragments = Vec::new();
        // Indentation of an anyhow `Caused by:` header; the causes follow on
        // more indented lines.
        let mut cause_header_indent = None;
        for (index, line) in lines.iter().enumerate() {
            retrieval_lines.push(retrieval_line(line, root));

            let trimmed = line.trim();
            let in_cause_chain =
                cause_header_indent.is_some_and(|header| indentation(line) > header);
            if !trimmed.is_empty() && !in_cause_chain {
                cause_header_indent = trimmed
                    .eq_ignore_ascii_case("caused by:")
                    .then(|| indentation(line));
            }
            if FRAMING_LINE.is_match(line) {
                continue;
            }
            let (severity, body) = split_log_prefix(line);
            let message = ERROR_LABEL
                .find(body)
                .map_or(body, |label| &body[label.end()..]);
            let from_message_line =
                counted_labels[index] || severity == LogSeverity::Error || in_cause_chain;
            collect_fragments(message, from_message_line, &mut fragments);
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
        // Runs from message lines come first, so source excerpts and log prose
        // cannot evict the message; then longest first, ties by text.
        fragments.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.len().cmp(&left.1.len()))
                .then_with(|| left.1.cmp(&right.1))
        });
        let mut seen = HashSet::new();
        fragments.retain(|(_, fragment)| seen.insert(fragment.to_ascii_lowercase()));
        fragments.truncate(MAX_MESSAGE_FRAGMENTS);
        Some(Self {
            retrieval_text,
            fragments: fragments
                .into_iter()
                .map(|(_, fragment)| fragment)
                .collect(),
        })
    }
}

/// One line with runtime values removed. `key=value` pairs keep the key.
fn retrieval_line(line: &str, root: &str) -> String {
    let mut kept = Vec::new();
    for word in split_words(line) {
        match classify_word(word) {
            WordKind::AbsolutePath(path) => {
                if let Some(relative) = workspace_relative(path, root) {
                    kept.push(relative.to_string());
                }
            }
            WordKind::Volatile => {}
            WordKind::KeyValue(key, value) => match classify_word(value) {
                WordKind::AbsolutePath(path) => kept.push(
                    workspace_relative(path, root)
                        .map_or_else(|| key.to_string(), |relative| format!("{key}={relative}")),
                ),
                WordKind::Volatile => kept.push(key.to_string()),
                _ => kept.push(word.to_string()),
            },
            WordKind::Quoted | WordKind::RelativePath | WordKind::Text => {
                kept.push(word.to_string());
            }
        }
    }
    kept.join(" ")
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogSeverity {
    None,
    Error,
    Other,
}

/// Splits leading log decoration, timestamps and bracketed severities such as
/// `2024-05-14T10:32:11Z [ERROR]`, from the rest of the line.
fn split_log_prefix(line: &str) -> (LogSeverity, &str) {
    let mut severity = LogSeverity::None;
    let mut rest = line.trim_start();
    while let Some(token) = rest.split_whitespace().next() {
        match bracketed_severity(token) {
            Some(level) => {
                if severity != LogSeverity::Error {
                    severity = level;
                }
            }
            None if TIMESTAMP.is_match(token) => {}
            None => break,
        }
        rest = rest[token.len()..].trim_start();
    }
    (severity, rest)
}

fn bracketed_severity(token: &str) -> Option<LogSeverity> {
    let level = token
        .strip_suffix(':')
        .unwrap_or(token)
        .strip_prefix('[')?
        .strip_suffix(']')?;
    let is = |names: &[&str]| names.iter().any(|name| level.eq_ignore_ascii_case(name));
    if is(&["error", "err", "fatal", "critical", "crit", "severe"]) {
        Some(LogSeverity::Error)
    } else if is(&["warn", "warning", "info", "notice", "debug", "trace"]) {
        Some(LogSeverity::Other)
    } else {
        None
    }
}

/// Splits on whitespace, but keeps a quoted absolute path
/// (`"/home/Jane Doe/app.lock"`) or a quoted key value
/// (`msg="could not open cache"`) together across its spaces.
fn split_words(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut position = 0;
    while let Some(offset) = text[position..].find(|ch: char| !ch.is_whitespace()) {
        let start = position + offset;
        let end = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |length| start + length);
        let end = quoted_value_end(text, start, end).unwrap_or(end);
        words.push(&text[start..end]);
        position = end;
    }
    words
}

/// End of the word starting at `start` when it opens a quoted absolute path or
/// a quoted `key=` value whose closing quote lies beyond `end`.
fn quoted_value_end(text: &str, start: usize, end: usize) -> Option<usize> {
    let word = &text[start..end];
    let (index, open, close) = word.char_indices().find_map(|(index, ch)| {
        QUOTE_PAIRS
            .iter()
            .find(|(open, _)| *open == ch)
            .map(|(open, close)| (index, *open, *close))
    })?;
    let value_start = index + open.len_utf8();
    let inside = &word[value_start..];
    let opens_value = starts_absolute(inside)
        || word[..index]
            .strip_suffix('=')
            .is_some_and(|key| !key.is_empty() && is_key(key));
    if !opens_value || inside.contains(close) {
        return None;
    }
    let value_start = start + value_start;
    let after_close = value_start + text[value_start..].find(close)? + close.len_utf8();
    Some(
        text[after_close..]
            .find(char::is_whitespace)
            .map_or(text.len(), |length| after_close + length),
    )
}

fn starts_absolute(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.starts_with('/')
        || text.starts_with("~/")
        || text.starts_with("\\\\")
        || (bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

/// Keys of `key=value` pairs: `path`, `request_id`, `http.status`, `--output`.
fn is_key(key: &str) -> bool {
    let name = key
        .strip_prefix("--")
        .or_else(|| key.strip_prefix('-'))
        .unwrap_or(key);
    name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn split_key_value(word: &str) -> Option<(&str, &str)> {
    let (key, value) = word.split_once('=')?;
    (is_key(key) && !value.is_empty() && !value.starts_with('=')).then_some((key, value))
}

fn strip_quotes(value: &str) -> &str {
    value
        .trim_end_matches([',', ';', ')', ']', '}'])
        .trim_matches(|ch: char| {
            QUOTE_PAIRS
                .iter()
                .any(|(open, close)| ch == *open || ch == *close)
        })
}

enum WordKind<'a> {
    AbsolutePath(&'a str),
    RelativePath,
    Quoted,
    Volatile,
    Text,
    /// A `key=value` pair, with the raw value.
    KeyValue(&'a str, &'a str),
}

fn classify_word(word: &str) -> WordKind<'_> {
    if let Some((key, value)) = split_key_value(word) {
        return WordKind::KeyValue(key, value);
    }
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
    if let Some((key, value)) = split_key_value(core) {
        return WordKind::KeyValue(key, value);
    }
    if core.contains("://") {
        return WordKind::Volatile;
    }
    if starts_absolute(core) {
        return WordKind::AbsolutePath(core);
    }
    if is_number_like(core)
        || is_hex_id(core)
        || (core.starts_with(|ch: char| ch.is_ascii_digit()) && TIMESTAMP.is_match(core))
    {
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
/// quoted values without spaces, relative paths, `key=value` pairs, `: ` chain
/// separators, and sentence ends end a run, so every run is a substring of the
/// format string that produced it. Long messages are often split into adjacent
/// source literals at sentence boundaries.
fn collect_fragments(message: &str, from_message_line: bool, fragments: &mut Vec<(bool, String)>) {
    let mut current: Vec<&str> = Vec::new();
    for word in split_words(message) {
        match classify_word(word) {
            WordKind::Text => {}
            WordKind::KeyValue(_, value) => {
                push_fragment(&mut current, from_message_line, fragments);
                // A quoted prose value, such as `msg="could not open cache"`,
                // is message text of its own.
                if matches!(classify_word(value), WordKind::Quoted) {
                    collect_fragments(strip_quotes(value), from_message_line, fragments);
                }
                continue;
            }
            WordKind::AbsolutePath(_)
            | WordKind::RelativePath
            | WordKind::Quoted
            | WordKind::Volatile => {
                push_fragment(&mut current, from_message_line, fragments);
                continue;
            }
        }
        let starts_group =
            word.starts_with(['\'', '"', '`', '\u{2018}', '\u{201c}', '(', '[', '{']);
        if starts_group {
            push_fragment(&mut current, from_message_line, fragments);
        }
        current.push(word);
        let ends_run = word.ends_with([':', '.', '?', '!'])
            || word
                .trim_end_matches([',', ';', ':', '.', '!', '?'])
                .ends_with(['\'', '"', '`', '\u{2019}', '\u{201d}']);
        if ends_run {
            push_fragment(&mut current, from_message_line, fragments);
        }
    }
    push_fragment(&mut current, from_message_line, fragments);
}

fn push_fragment(
    words: &mut Vec<&str>,
    from_message_line: bool,
    fragments: &mut Vec<(bool, String)>,
) {
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
        fragments.push((from_message_line, fragment.to_string()));
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

    /// (workspace root, query, removed from retrieval text, kept in retrieval
    /// text, expected fragments)
    type RetrievalCase<'a> = (
        &'a str,
        &'a str,
        &'a [&'a str],
        &'a [&'a str],
        &'a [&'a str],
    );

    fn check_retrieval_cases(cases: &[RetrievalCase<'_>]) {
        for (root, query, removed, kept, fragments) in cases {
            let error = PastedError::parse(query, Path::new(root)).expect("pasted error output");
            for text in *removed {
                assert!(
                    !error.retrieval_text.contains(text),
                    "{text:?} should be removed from {:?}",
                    error.retrieval_text
                );
            }
            for text in *kept {
                assert!(
                    error.retrieval_text.contains(text),
                    "{text:?} should stay in {:?}",
                    error.retrieval_text
                );
            }
            assert_eq!(&error.fragments, fragments, "{query}");
        }
    }

    #[test]
    fn key_value_pairs_drop_runtime_values_and_keep_keys() {
        check_retrieval_cases(&[
            (
                "/work/app",
                "Error: failed to acquire index lock path=/home/dev/.local/share/orbit/index.lock request_id=7c41e0a9d2f3 status=500 url=https://orbit.example.com/v1/items/42 source=/work/app/src/lock.rs",
                &["share", "7c41e0a9d2f3", "500", "example.com", "/work/app"],
                &["path request_id status url source=src/lock.rs"],
                &["failed to acquire index lock"],
            ),
            (
                "/work/app",
                "Error: segment flush failed\nmsg=\"could not reopen segment writer\" file=\"/home/Jane Doe/.cache/orbit/segments.db\" attempt=3",
                &["Jane", "Doe", ".cache", "segments.db", "=3"],
                &["msg=\"could not reopen segment writer\" file attempt"],
                &["segment flush failed", "could not reopen segment writer"],
            ),
        ]);
    }

    #[test]
    fn quoted_absolute_paths_with_spaces_are_one_value() {
        check_retrieval_cases(&[
            (
                "/work/app",
                "Error: failed to acquire index lock \"/home/Jane Doe/.local/share/orbit/index.lock\": permission denied (os error 13)",
                &["Jane", "Doe", "share"],
                &["failed to acquire index lock permission denied"],
                &["failed to acquire index lock", "permission denied"],
            ),
            (
                "/work/app",
                "Error: failed to read orbit config 'C:\\Users\\Jane Doe\\AppData\\Local\\orbit\\settings.toml'",
                &["Doe", "AppData", "settings.toml"],
                &["failed to read orbit config"],
                &["failed to read orbit config"],
            ),
            (
                "/work/Jane Doe/app",
                "Error: cannot parse \"/work/Jane Doe/app/src/config.toml\": expected a table",
                &["Jane", "/work"],
                &["cannot parse src/config.toml expected a table"],
                &["expected a table", "cannot parse"],
            ),
        ]);
    }

    #[test]
    fn severity_prefixes_stay_out_of_fragments() {
        check_retrieval_cases(&[
            (
                "/work/app",
                "[ERROR] Failed to execute goal: java.lang.IllegalStateException: Order already submitted",
                &[],
                &[],
                &["Order already submitted", "Failed to execute goal"],
            ),
            (
                "/work/app",
                "Error: segment flush failed\n2024-05-14T10:32:11.123Z [WARN] retrying segment writer after timeout",
                &["2024", "10:32"],
                &["[WARN] retrying segment writer after timeout"],
                &[
                    "segment flush failed",
                    "retrying segment writer after timeout",
                ],
            ),
        ]);
    }

    #[test]
    fn message_line_fragments_survive_truncation() {
        let mut query = String::from(
            "Traceback (most recent call last):\n  File \"/srv/app/importer.py\", line 88, in run\n    self.process(batch)\n",
        );
        for line in [
            "loading customer records from the nightly export bundle",
            "normalizing customer addresses against the postal registry",
            "deduplicating customer records with the fuzzy matching rules",
            "writing normalized customer records into the staging schema",
            "refreshing materialized reporting views for the finance team",
            "publishing import completion events to the notification queue",
            "archiving processed export bundles into cold storage buckets",
            "rotating importer credentials after the scheduled maintenance",
            "recording import metrics for the operations dashboard today",
        ] {
            query.push_str(line);
            query.push('\n');
        }
        query.push_str("ValueError: bad input supplied");

        let fragments = parse(&query).fragments;
        assert_eq!(fragments.len(), 8);
        assert_eq!(fragments[0], "bad input supplied");
        assert!(
            fragments[1..].windows(2).all(|pair| {
                pair[0].len() > pair[1].len()
                    || (pair[0].len() == pair[1].len() && pair[0] <= pair[1])
            }),
            "{fragments:?}"
        );
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
