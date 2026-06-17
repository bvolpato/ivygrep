use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Section {
    None,
    Token,
    Phrase,
}

fn main() {
    configure_windows_manifest();

    let input = Path::new("assets/query_aliases.toml");
    println!("cargo:rerun-if-changed={}", input.display());
    println!("cargo:rerun-if-changed=build.rs");

    let text = fs::read_to_string(input)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", input.display()));
    let aliases = parse_aliases(&text, input);

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo");
    let out_path = Path::new(&out_dir).join("query_aliases.rs");
    fs::write(out_path, generate_alias_module(&aliases))
        .expect("failed to write generated query aliases");
}

fn configure_windows_manifest() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let manifest = Path::new(
        &env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set by Cargo"),
    )
    .join("assets/windows/ivygrep.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-bin=ig=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=ig=/MANIFESTINPUT:{}",
        manifest.display()
    );
}

#[derive(Default)]
struct AliasData {
    tokens: Vec<(String, Vec<String>)>,
    phrases: Vec<(Vec<String>, Vec<String>)>,
}

fn parse_aliases(text: &str, input: &Path) -> AliasData {
    let mut data = AliasData::default();
    let mut section = Section::None;

    for (idx, raw_line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw_line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        match line.as_str() {
            "[token]" => {
                section = Section::Token;
                continue;
            }
            "[phrase]" => {
                section = Section::Phrase;
                continue;
            }
            _ => {}
        }

        let (key, value) = line.split_once('=').unwrap_or_else(|| {
            panic!(
                "{}:{line_no}: expected `key = [\"alias\"]`",
                input.display()
            )
        });
        let key = parse_key(key.trim(), input, line_no);
        let aliases = parse_string_array(value.trim(), input, line_no);
        validate_aliases(&aliases, input, line_no);

        match section {
            Section::Token => {
                validate_token(&key, input, line_no);
                data.tokens.push((key, aliases));
            }
            Section::Phrase => {
                validate_phrase(&key, input, line_no);
                let terms = key
                    .split_ascii_whitespace()
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                if terms.len() < 2 {
                    panic!("{}:{line_no}: phrase alias needs 2+ terms", input.display());
                }
                data.phrases.push((terms, aliases));
            }
            Section::None => panic!(
                "{}:{line_no}: alias must be under a section",
                input.display()
            ),
        }
    }

    sort_and_check_duplicates(&mut data, input);
    data
}

fn strip_comment(line: &str) -> String {
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == '#' && !in_string {
            return line[..idx].to_string();
        }
    }

    line.to_string()
}

fn parse_key(key: &str, input: &Path, line_no: usize) -> String {
    if key.starts_with('"') {
        if !key.ends_with('"') || key.len() < 2 {
            panic!("{}:{line_no}: unterminated quoted key", input.display());
        }
        return key[1..key.len() - 1].to_string();
    }

    if key.is_empty() {
        panic!("{}:{line_no}: empty key", input.display());
    }

    key.to_string()
}

fn parse_string_array(value: &str, input: &Path, line_no: usize) -> Vec<String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        panic!(
            "{}:{line_no}: value must be a string array",
            input.display()
        );
    }

    let mut aliases = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in value[1..value.len() - 1].chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => escaped = true,
            '"' => {
                if in_string {
                    aliases.push(current.clone());
                    current.clear();
                }
                in_string = !in_string;
            }
            ',' | ' ' | '\t' if !in_string => {}
            _ if in_string => current.push(ch),
            _ => panic!(
                "{}:{line_no}: expected quoted string array",
                input.display()
            ),
        }
    }

    if in_string {
        panic!("{}:{line_no}: unterminated string", input.display());
    }
    if aliases.is_empty() {
        panic!(
            "{}:{line_no}: alias array must not be empty",
            input.display()
        );
    }

    aliases
}

fn validate_aliases(aliases: &[String], input: &Path, line_no: usize) {
    let mut seen = HashSet::new();
    for alias in aliases {
        validate_token(alias, input, line_no);
        if !seen.insert(alias) {
            panic!("{}:{line_no}: duplicate alias `{alias}`", input.display());
        }
    }
}

fn validate_token(term: &str, input: &Path, line_no: usize) {
    if term.is_empty() {
        panic!("{}:{line_no}: empty term", input.display());
    }
    if term != term.to_ascii_lowercase() {
        panic!("{}:{line_no}: `{term}` must be lowercase", input.display());
    }
    if !term
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        panic!(
            "{}:{line_no}: `{term}` must be an ascii lowercase token",
            input.display()
        );
    }
}

fn validate_phrase(phrase: &str, input: &Path, line_no: usize) {
    let terms = phrase.split_ascii_whitespace().collect::<Vec<_>>();
    if terms.len() < 2 {
        panic!("{}:{line_no}: phrase alias needs 2+ terms", input.display());
    }
    for term in terms {
        validate_token(term, input, line_no);
    }
}

fn sort_and_check_duplicates(data: &mut AliasData, input: &Path) {
    data.tokens.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in data.tokens.windows(2) {
        if pair[0].0 == pair[1].0 {
            panic!("{}: duplicate token alias `{}`", input.display(), pair[0].0);
        }
    }

    data.phrases.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in data.phrases.windows(2) {
        if pair[0].0 == pair[1].0 {
            panic!(
                "{}: duplicate phrase alias `{}`",
                input.display(),
                pair[0].0.join(" ")
            );
        }
    }
}

fn generate_alias_module(data: &AliasData) -> String {
    let mut output = String::new();
    output.push_str("// @generated by build.rs from assets/query_aliases.toml\n");
    output.push_str("pub(crate) const TOKEN_ALIASES: &[(&str, &[&str])] = &[\n");

    for (token, aliases) in &data.tokens {
        output.push_str("    (");
        output.push_str(&format!("{token:?}"));
        output.push_str(", &[");
        push_string_literals(&mut output, aliases);
        output.push_str("]),\n");
    }

    output.push_str("];\n\n");
    output.push_str("pub(crate) const PHRASE_ALIASES: &[PhraseAlias] = &[\n");

    for (terms, aliases) in &data.phrases {
        output.push_str("    PhraseAlias { terms: &[");
        push_string_literals(&mut output, terms);
        output.push_str("], aliases: &[");
        push_string_literals(&mut output, aliases);
        output.push_str("] },\n");
    }

    output.push_str("];\n");
    output
}

fn push_string_literals(output: &mut String, values: &[String]) {
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("{value:?}"));
    }
}
