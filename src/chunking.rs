//! Source-file chunking with a data-driven language registry.
//!
//! # Adding a new language
//!
//! 1. Add a [`LanguageDef`] entry to [`LANGUAGES`].
//! 2. Set `extensions` (without dots) and/or `filenames` for exact matches.
//! 3. Write a `detect_<lang>(trimmed_line) -> Option<ChunkKind>` function,
//!    or use [`detect_text_only`] for languages without structural boundaries.
//! 4. Done — indexing, search, and MCP pick it up automatically.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use uuid::Uuid;

const TEXT_SNIFF_BYTES: usize = 8 * 1024;
const MIN_PRINTABLE_RATIO: f32 = 0.85;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ChunkKind {
    Function,
    Class,
    Module,
    Documentation,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Uuid,
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub language: String,
    pub kind: ChunkKind,
    pub content_hash: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RustDocInclude {
    pub source_line: usize,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct ChunkedSource {
    pub chunks: Vec<Chunk>,
    pub rust_doc_includes: Vec<RustDocInclude>,
}

// ── Language Registry ──────────────────────────────────────────────────────

/// Defines a supported language: file-matching rules and structural chunking.
pub struct LanguageDef {
    /// Canonical language name used in search filters and metadata.
    pub name: &'static str,
    /// File extensions without leading dot. Matched case-insensitively.
    pub extensions: &'static [&'static str],
    /// Filename matches (e.g. `"Dockerfile"`). A plain pattern also matches
    /// variants such as `Dockerfile.prod`; a leading `=` requires exact match.
    pub filenames: &'static [&'static str],
    /// Inspects a **trimmed** source line; returns `Some(kind)` when the
    /// line opens a structural boundary, `None` otherwise.
    pub detect_signature: fn(&str) -> Option<ChunkKind>,
}

/// Master language table. Order matters only when extensions overlap —
/// the first match wins.
static LANGUAGES: &[LanguageDef] = &[
    // ── Systems ────────────────────────────────────────────────────────
    LanguageDef {
        name: "rust",
        extensions: &["rs"],
        filenames: &[],
        detect_signature: detect_rust,
    },
    LanguageDef {
        name: "go",
        extensions: &["go"],
        filenames: &[],
        detect_signature: detect_go,
    },
    LanguageDef {
        name: "c",
        extensions: &["c", "h"],
        filenames: &[],
        detect_signature: detect_c,
    },
    LanguageDef {
        name: "cpp",
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx", "hh"],
        filenames: &[],
        detect_signature: detect_cpp,
    },
    LanguageDef {
        name: "zig",
        extensions: &["zig"],
        filenames: &[],
        detect_signature: detect_zig,
    },
    LanguageDef {
        name: "nim",
        extensions: &["nim", "nims"],
        filenames: &[],
        detect_signature: detect_nim,
    },
    // ── JVM ────────────────────────────────────────────────────────────
    LanguageDef {
        name: "java",
        extensions: &["java"],
        filenames: &[],
        detect_signature: detect_java,
    },
    LanguageDef {
        name: "kotlin",
        extensions: &["kt", "kts"],
        filenames: &[],
        detect_signature: detect_kotlin,
    },
    LanguageDef {
        name: "scala",
        extensions: &["scala", "sc"],
        filenames: &[],
        detect_signature: detect_scala,
    },
    LanguageDef {
        name: "groovy",
        extensions: &["groovy", "gvy"],
        filenames: &[],
        detect_signature: detect_groovy,
    },
    // ── .NET ───────────────────────────────────────────────────────────
    LanguageDef {
        name: "csharp",
        extensions: &["cs"],
        filenames: &[],
        detect_signature: detect_csharp,
    },
    // ── Web / scripting ────────────────────────────────────────────────
    LanguageDef {
        name: "python",
        extensions: &["py", "pyi"],
        filenames: &[],
        detect_signature: detect_python,
    },
    LanguageDef {
        name: "typescript",
        extensions: &["ts", "tsx", "mts", "cts"],
        filenames: &[],
        detect_signature: detect_typescript_javascript,
    },
    LanguageDef {
        name: "javascript",
        extensions: &["js", "jsx", "mjs", "cjs"],
        filenames: &[],
        detect_signature: detect_typescript_javascript,
    },
    LanguageDef {
        name: "ruby",
        extensions: &["rb", "rake"],
        filenames: &["Rakefile", "Gemfile"],
        detect_signature: detect_ruby,
    },
    LanguageDef {
        name: "php",
        extensions: &["php"],
        filenames: &[],
        detect_signature: detect_php,
    },
    LanguageDef {
        name: "perl",
        extensions: &["pl", "pm"],
        filenames: &[],
        detect_signature: detect_perl,
    },
    LanguageDef {
        name: "lua",
        extensions: &["lua"],
        filenames: &[],
        detect_signature: detect_lua,
    },
    // ── Apple / mobile ─────────────────────────────────────────────────
    LanguageDef {
        name: "swift",
        extensions: &["swift"],
        filenames: &[],
        detect_signature: detect_swift,
    },
    LanguageDef {
        name: "dart",
        extensions: &["dart"],
        filenames: &[],
        detect_signature: detect_dart,
    },
    LanguageDef {
        name: "objc",
        extensions: &["m", "mm"],
        filenames: &[],
        detect_signature: detect_objc,
    },
    // ── Functional ─────────────────────────────────────────────────────
    LanguageDef {
        name: "elixir",
        extensions: &["ex", "exs"],
        filenames: &[],
        detect_signature: detect_elixir,
    },
    LanguageDef {
        name: "erlang",
        extensions: &["erl", "hrl"],
        filenames: &[],
        detect_signature: detect_erlang,
    },
    LanguageDef {
        name: "haskell",
        extensions: &["hs"],
        filenames: &[],
        detect_signature: detect_haskell,
    },
    LanguageDef {
        name: "ocaml",
        extensions: &["ml", "mli"],
        filenames: &[],
        detect_signature: detect_ocaml,
    },
    LanguageDef {
        name: "clojure",
        extensions: &["clj", "cljs", "cljc", "edn"],
        filenames: &[],
        detect_signature: detect_clojure,
    },
    // ── Scientific / data ──────────────────────────────────────────────
    LanguageDef {
        name: "r",
        extensions: &["r", "R"],
        filenames: &[],
        detect_signature: detect_r,
    },
    LanguageDef {
        name: "julia",
        extensions: &["jl"],
        filenames: &[],
        detect_signature: detect_julia,
    },
    // ── Shell ──────────────────────────────────────────────────────────
    LanguageDef {
        name: "shell",
        extensions: &["sh", "bash", "zsh", "fish"],
        filenames: &[],
        detect_signature: detect_shell,
    },
    LanguageDef {
        name: "powershell",
        extensions: &["ps1", "psm1", "psd1"],
        filenames: &[],
        detect_signature: detect_powershell,
    },
    // ── Query / schema ─────────────────────────────────────────────────
    LanguageDef {
        name: "sql",
        extensions: &["sql"],
        filenames: &[],
        detect_signature: detect_sql,
    },
    LanguageDef {
        name: "protobuf",
        extensions: &["proto"],
        filenames: &[],
        detect_signature: detect_protobuf,
    },
    LanguageDef {
        name: "thrift",
        extensions: &["thrift"],
        filenames: &[],
        detect_signature: detect_protobuf, // same heuristics
    },
    LanguageDef {
        name: "graphql",
        extensions: &["graphql", "gql"],
        filenames: &[],
        detect_signature: detect_graphql,
    },
    // ── Infrastructure ─────────────────────────────────────────────────
    LanguageDef {
        name: "terraform",
        extensions: &["tf", "tfvars", "hcl"],
        filenames: &[],
        detect_signature: detect_terraform,
    },
    LanguageDef {
        name: "starlark",
        extensions: &["bzl", "bazel", "star"],
        filenames: &["=BUILD", "=WORKSPACE", "=MODULE"],
        detect_signature: detect_starlark,
    },
    LanguageDef {
        name: "dockerfile",
        extensions: &[],
        filenames: &["Dockerfile"],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "makefile",
        extensions: &["mk"],
        filenames: &["Makefile", "makefile", "GNUmakefile"],
        detect_signature: detect_text_only,
    },
    // ── Markup / style ─────────────────────────────────────────────────
    LanguageDef {
        name: "markdown",
        extensions: &["md", "mdx"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "html",
        extensions: &["html", "htm", "xhtml"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "css",
        extensions: &["css", "scss", "sass", "less"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "xml",
        extensions: &["xml", "xsl", "xslt", "svg", "plist"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    // ── Config / data ──────────────────────────────────────────────────
    LanguageDef {
        name: "config",
        extensions: &["toml", "yaml", "yml", "ini", "cfg", "env"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "json",
        extensions: &["json", "jsonl", "json5", "geojson"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
    LanguageDef {
        name: "text",
        extensions: &["txt", "log", "csv", "tsv", "rst", "adoc"],
        filenames: &[],
        detect_signature: detect_text_only,
    },
];

// ── Public API ─────────────────────────────────────────────────────────────

/// Resolve a file path to its language definition from the registry.
fn find_language_def(path: &Path) -> Option<&'static LanguageDef> {
    // Filename matches first (Dockerfile, Makefile, Rakefile, etc.).
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        for lang in LANGUAGES {
            for pattern in lang.filenames {
                let exact_only = pattern.strip_prefix('=');
                if exact_only.is_some_and(|pattern| filename == pattern)
                    || (exact_only.is_none()
                        && (filename == *pattern
                            || (filename.len() > pattern.len()
                                && filename.starts_with(pattern)
                                && filename.as_bytes()[pattern.len()] == b'.')))
                {
                    return Some(lang);
                }
            }
        }
    }

    // Extension matches (case-insensitive).
    let ext = path.extension().and_then(|e| e.to_str())?;
    LANGUAGES
        .iter()
        .find(|lang| lang.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
}

pub fn language_for_path(path: &Path) -> Option<&'static str> {
    find_language_def(path).map(|def| def.name)
}

/// Resolve a user-provided type filter to a canonical language name.
///
/// Accepts canonical names (`rust`), file extensions (`rs`, `py`, `md`),
/// and common aliases (`c++`, `bash`, `js`, `objective-c`).
/// Returns `None` if no match is found.
pub fn resolve_type_alias(input: &str) -> Option<&'static str> {
    let lower = input.to_ascii_lowercase();
    let lower = lower.trim_start_matches('.');

    // 1. Exact canonical name match
    if let Some(lang) = LANGUAGES.iter().find(|l| l.name == lower) {
        return Some(lang.name);
    }

    // 2. Extension match (e.g. "rs" → "rust", "py" → "python")
    if let Some(lang) = LANGUAGES
        .iter()
        .find(|l| l.extensions.iter().any(|e| e.eq_ignore_ascii_case(lower)))
    {
        return Some(lang.name);
    }

    // 3. Common aliases not covered by name or extension
    let alias = match lower {
        "c++" | "cplusplus" => "cpp",
        "c#" => "csharp",
        "js" => "javascript",
        "ts" | "tsx" => "typescript",
        "jsx" | "mjs" | "cjs" => "javascript",
        "bash" | "zsh" | "fish" => "shell",
        "yml" | "toml" | "yaml" | "ini" | "cfg" => "config",
        "objective-c" | "objective_c" | "objectivec" => "objc",
        "proto" => "protobuf",
        "tf" | "hcl" => "terraform",
        "gql" => "graphql",
        "rb" | "rake" => "ruby",
        "kt" | "kts" => "kotlin",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "pl" | "pm" => "perl",
        "clj" | "cljs" | "cljc" => "clojure",
        "jl" => "julia",
        "ps1" | "psm1" => "powershell",
        "sc" => "scala",
        "htm" | "xhtml" => "html",
        "scss" | "sass" | "less" => "css",
        "mdx" => "markdown",
        "pyi" => "python",
        _ => return None,
    };
    Some(alias)
}

pub fn is_indexable_path(path: &Path) -> bool {
    language_for_path(path).is_some()
}

/// A single line (run with no `\n`) this long marks a file as minified or a
/// packed blob. No hand-written source has a 50 KB line; minified JS/CSS bundles
/// and packed data do. Such files produce one enormous, low-value chunk, so we
/// skip them regardless of total size.
const MAX_SOURCE_LINE_BYTES: usize = 50_000;

pub fn is_indexable_file(path: &Path, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if !is_probably_text(bytes) {
        return false;
    }
    if is_minified_blob(bytes) {
        return false;
    }
    if is_indexable_path(path) {
        return true;
    }
    // Unknown extension but content looks like text — index it anyway.
    true
}

pub fn is_indexable_file_reader<R: Read>(_path: &Path, reader: &mut R) -> io::Result<bool> {
    let mut sample = Vec::with_capacity(TEXT_SNIFF_BYTES);
    let mut buf = [0u8; TEXT_SNIFF_BYTES];
    let mut no_newline_run = 0usize;

    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }

        let bytes = &buf[..read];
        if sample.len() < TEXT_SNIFF_BYTES {
            let needed = TEXT_SNIFF_BYTES - sample.len();
            sample.extend_from_slice(&bytes[..bytes.len().min(needed)]);
            if sample.len() == TEXT_SNIFF_BYTES && !is_probably_text(&sample) {
                return Ok(false);
            }
        }

        if scan_minified_run(&mut no_newline_run, bytes) {
            return Ok(false);
        }
    }

    if !is_probably_text(&sample) {
        return Ok(false);
    }

    // Unknown extension but content looks like text: same behavior as
    // is_indexable_file.
    Ok(true)
}

/// Detects minified bundles / packed blobs: any run of at least
/// [`MAX_SOURCE_LINE_BYTES`] bytes with no `\n`, anywhere in the file.
///
/// Scanning the longest no-newline run (rather than only a fixed prefix) catches
/// bundles that keep a short license banner before a giant minified body, and
/// gives the same answer whether called on a full file (indexing) or a small
/// sample (health-check probes) — as long as the run is present in the bytes
/// provided. Short-circuits as soon as the threshold is hit.
fn is_minified_blob(bytes: &[u8]) -> bool {
    let mut no_newline_run = 0usize;
    scan_minified_run(&mut no_newline_run, bytes)
}

fn scan_minified_run(no_newline_run: &mut usize, bytes: &[u8]) -> bool {
    for &b in bytes {
        if b == b'\n' {
            *no_newline_run = 0;
        } else {
            *no_newline_run += 1;
            if *no_newline_run >= MAX_SOURCE_LINE_BYTES {
                return true;
            }
        }
    }
    false
}

/// Maximum number of immediately-preceding comment/attribute lines folded into
/// the following definition chunk (#59). Caps pathological merges of huge banner
/// comments while still absorbing normal doc-comments.
const MAX_LEADING_COMMENT_LINES: usize = 20;

/// Only very large BUILD-like sources benefit from per-target AST chunks.
/// Smaller sources keep bounded text chunks to avoid diluting retrieval scores.
const STARLARK_TARGET_AST_LINE_THRESHOLD: usize = 500;

/// Heuristic: does a *trimmed* line look like a doc-comment, attribute, or
/// decorator that conventionally sits directly above a definition (for the given
/// `language`)?
///
/// Tree-sitter function/class nodes exclude a definition's leading doc-comment,
/// so without folding them in, each comment line lands in its own 1-line gap and
/// becomes a standalone `Module` chunk — large index bloat and bare-comment
/// search hits. We fold such lines into the following definition chunk instead.
fn is_leading_doc_line(trimmed: &str, language: &str) -> bool {
    // Covers //, ///, //! (C/Rust/Go/Java/JS/TS/Swift/Scala/Dart…), block comment
    // bodies (/* * */), -- (SQL/Haskell/Lua), ; (Lisp/asm), and triple-quoted docstrings.
    const PREFIXES: &[&str] = &["//", "/*", "*", "--", ";", "\"\"\"", "'''"];
    if PREFIXES.iter().any(|p| trimmed.starts_with(p)) {
        return true;
    }
    if trimmed.starts_with('@') {
        return matches!(
            language,
            "python" | "java" | "kotlin" | "typescript" | "javascript" | "swift" | "scala" | "dart"
        );
    }
    // Rust attributes attach to the item directly below, so fold them in.
    if trimmed.starts_with("#[") || trimmed.starts_with("#!") {
        return true;
    }
    // A bare `#` is a line comment only in languages that use it as one. In
    // C/C++/Obj-C/C# a leading `#` is a preprocessor directive (#include,
    // #define, #pragma, #region) — real, independently-retrievable code that
    // must NOT be folded into the following definition.
    if trimmed.starts_with('#') {
        return matches!(language, "python" | "ruby" | "php" | "perl" | "shell");
    }
    false
}

fn leading_doc_start(start_line: usize, language: &str, lines: &[&str]) -> usize {
    let mut start = start_line;
    while start > 1 {
        let previous = lines[start - 2].trim();
        if !is_leading_doc_line(previous, language) {
            break;
        }
        start -= 1;
    }
    start
}

fn should_skip_tree_sitter_for_generated_source(
    language: &str,
    lines: &[&str],
    detect_signature: fn(&str) -> Option<ChunkKind>,
) -> bool {
    if language != "rust" || lines.len() < 200 {
        return false;
    }

    let generated_header = lines.iter().take(20).any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("@generated")
            || lower.contains("generated")
            || lower.contains("automatically generated")
            || lower.contains("do not edit")
    });
    if !generated_header {
        return false;
    }

    lines
        .iter()
        .filter(|line| detect_signature(line.trim()).is_some())
        .take(50)
        .count()
        >= 50
}

pub fn chunk_source(rel_path: &Path, text: &str) -> Vec<Chunk> {
    chunk_source_with_metadata(rel_path, text).chunks
}

pub(crate) fn chunk_source_with_metadata(rel_path: &Path, text: &str) -> ChunkedSource {
    let lang_def = find_language_def(rel_path);
    let language = lang_def.map(|d| d.name).unwrap_or("text").to_string();
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return ChunkedSource {
            chunks: Vec::new(),
            rust_doc_includes: Vec::new(),
        };
    }

    // Attempt 100% accurate AST chunking via Tree-sitter for supported languages.
    // Large generated Rust files with many simple item signatures are handled by
    // the fallback splitter to avoid paying AST parse/query cost for low-value structure.
    let skip_tree_sitter = lang_def.is_some_and(|def| {
        should_skip_tree_sitter_for_generated_source(&language, &lines, def.detect_signature)
    });
    if !skip_tree_sitter
        && let Some(chunked) = try_tree_sitter_chunk_source(rel_path, text, &language, &lines)
        && !chunked.chunks.is_empty()
    {
        return chunked;
    }

    // Fall back to regex-based heuristic chunking
    let signatures = match lang_def {
        Some(def) => collect_signatures(def.detect_signature, &lines),
        None => vec![],
    };

    if signatures.is_empty() {
        return ChunkedSource {
            chunks: fallback_chunks(rel_path, &language, &lines),
            rust_doc_includes: Vec::new(),
        };
    }

    let chunk_starts = signatures
        .iter()
        .map(|(start_line, _)| leading_doc_start(*start_line, &language, &lines))
        .collect::<Vec<_>>();
    let mut chunks = Vec::new();
    for (idx, (_, kind)) in signatures.iter().enumerate() {
        let start = chunk_starts[idx];
        let end = chunk_starts
            .get(idx + 1)
            .map(|next| next.saturating_sub(1))
            .unwrap_or(lines.len());

        if end < start {
            continue;
        }

        let block = lines[start.saturating_sub(1)..end].join("\n");
        let text = format!("// {}\n\n{}", rel_path.to_string_lossy(), block);

        chunks.push(make_chunk(
            rel_path,
            start,
            end,
            text,
            language.clone(),
            kind.clone(),
        ));
    }

    ChunkedSource {
        chunks,
        rust_doc_includes: Vec::new(),
    }
}

/// Uses Tree-sitter to reliably extract accurately bounded functions and classes
/// for supported languages (Rust, Python, Go, JS, TS/TSX, Java, C#, PHP, Ruby,
/// Swift, C, C++, Scala, Kotlin, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart,
/// Objective-C, Perl, and Starlark macro sources).
fn try_tree_sitter_chunk_source(
    rel_path: &Path,
    text: &str,
    language: &str,
    lines: &[&str],
) -> Option<ChunkedSource> {
    try_tree_sitter_chunk_source_with_timeout(
        rel_path,
        text,
        language,
        lines,
        std::time::Duration::from_millis(100),
    )
}

fn try_tree_sitter_chunk_source_with_timeout(
    rel_path: &Path,
    text: &str,
    language: &str,
    lines: &[&str],
    parse_timeout: std::time::Duration,
) -> Option<ChunkedSource> {
    use streaming_iterator::StreamingIterator;
    use tree_sitter::QueryCursor;

    let (grammar, query) = tree_sitter_query(rel_path, language, lines.len())?;

    // The production caller uses a 100ms budget to prevent hangs on massive
    // minified files. ParseOptions replaces timeout_micros in tree-sitter 0.26.
    let start_time = std::time::Instant::now();
    let mut parse_cancelled = false;
    let mut cb = |_state: &tree_sitter::ParseState| {
        if start_time.elapsed() >= parse_timeout {
            parse_cancelled = true;
            std::ops::ControlFlow::Break(())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    };
    let options = tree_sitter::ParseOptions::new().progress_callback(&mut cb);

    let bytes = text.as_bytes();
    let len = bytes.len();
    let tree = TREE_SITTER_PARSER.with(|slot| {
        let mut parser = slot.borrow_mut();
        parser.set_language(&grammar).ok()?;
        parser.parse_with_options(
            &mut |i, _| {
                if i < len {
                    &bytes[i..]
                } else {
                    Default::default()
                }
            },
            None,
            Some(options),
        )
    })?;
    if parse_cancelled {
        return None;
    }
    let mut cursor = QueryCursor::new();

    let mut ranges = Vec::new();
    let mut rust_doc_includes = Vec::new();
    let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            if capture_name == "doc_include" {
                if let Some(include) = rust_doc_include(capture.node, text) {
                    rust_doc_includes.push(include);
                }
                continue;
            }
            if capture_name.starts_with('_') {
                continue;
            }

            let start_line = capture.node.start_position().row;
            let end_line = capture.node.end_position().row;

            let kind = match capture_name {
                "module" => ChunkKind::Module,
                "class" => ChunkKind::Class,
                _ => match capture.node.kind() {
                "class_definition"
                | "impl_item"
                | "trait_item"
                | "class_declaration"
                | "abstract_class_declaration"
                | "interface_declaration"
                | "type_alias_declaration"
                | "type_declaration"
                | "enum_declaration"
                | "annotation_type_declaration"
                | "struct_declaration"
                | "record_declaration"
                | "trait_declaration"
                | "class"
                | "module"
                | "protocol_declaration"
                | "extension_declaration"
                // C/C++
                | "struct_specifier"
                | "enum_specifier"
                | "union_specifier"
                | "class_specifier"
                | "namespace_definition"
                // Scala
                | "trait_definition"
                | "object_definition"
                // Haskell
                | "data_type"
                | "instance"
                // OCaml
                | "type_definition"
                | "module_definition"
                // Obj-C
                | "class_interface"
                | "class_implementation"
                | "category_interface"
                | "category_implementation"
                // Perl
                | "package_statement" => ChunkKind::Class,
                _ => ChunkKind::Function,
                },
            };

            // Convert to 1-indexed bounds, end_line is inclusive in tree-sitter rows
            ranges.push((start_line + 1, end_line + 1, kind));
        }
    }

    if ranges.is_empty() && rust_doc_includes.is_empty() {
        return None;
    }

    // Sort by start line; keep overlapping structural chunks (impl+fn).
    ranges.sort_by_key(|r| r.0);

    let mut chunks = Vec::new();

    // Track which 1-indexed lines are covered by AST chunks (start..=end)
    let mut covered = vec![false; lines.len() + 1]; // index 0 unused

    for (start, end, kind) in &ranges {
        let mut start = *start;
        let end = *end;
        if start == 0 || start > lines.len() {
            continue;
        }
        let safe_end = end.min(lines.len());
        if safe_end < start {
            continue;
        }

        // Fold a contiguous block of immediately-preceding comment/attribute
        // lines (a definition's doc-comment) into this chunk so it enriches the
        // definition's embedding instead of becoming its own 1-line `Module`
        // chunk. See #59. Lines already covered by an enclosing chunk (e.g. an
        // impl block) are left alone.
        let mut absorbed = 0usize;
        while start > 1 && absorbed < MAX_LEADING_COMMENT_LINES {
            let prev = start - 1; // 1-indexed line directly above
            if covered[prev] {
                break;
            }
            let line = lines[prev - 1].trim();
            if line.is_empty() || !is_leading_doc_line(line, language) {
                break;
            }
            start = prev;
            absorbed += 1;
        }

        for flag in covered.iter_mut().take(safe_end + 1).skip(start) {
            *flag = true;
        }

        let block_lines = &lines[(start - 1)..safe_end];
        let block_text = format!(
            "// {}\n\n{}",
            rel_path.to_string_lossy(),
            block_lines.join("\n")
        );
        chunks.push(make_chunk(
            rel_path,
            start,
            safe_end,
            block_text,
            language.to_string(),
            kind.clone(),
        ));
    }

    // Emit module-level chunks for uncovered line ranges (imports,
    // constants, top-level expressions, etc.).
    let mut gap_start: Option<usize> = None;
    for i in 1..=lines.len() {
        if !covered[i] && !lines[i - 1].trim().is_empty() {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
        } else if let Some(gs) = gap_start {
            let gs_end = i - 1;
            if gs_end >= gs {
                let block_lines = &lines[(gs - 1)..gs_end];
                let block_text = format!(
                    "// {}\n\n{}",
                    rel_path.to_string_lossy(),
                    block_lines.join("\n")
                );
                chunks.push(make_chunk(
                    rel_path,
                    gs,
                    gs_end,
                    block_text,
                    language.to_string(),
                    ChunkKind::Module,
                ));
            }
            gap_start = None;
        }
    }
    // Trailing gap at EOF
    if let Some(gs) = gap_start {
        let gs_end = lines.len();
        let block_lines = &lines[(gs - 1)..gs_end];
        let block_text = format!(
            "// {}\n\n{}",
            rel_path.to_string_lossy(),
            block_lines.join("\n")
        );
        chunks.push(make_chunk(
            rel_path,
            gs,
            gs_end,
            block_text,
            language.to_string(),
            ChunkKind::Module,
        ));
    }

    chunks.sort_by_key(|c| c.start_line);

    rust_doc_includes.sort_by(|left, right| {
        left.source_line
            .cmp(&right.source_line)
            .then_with(|| left.path.cmp(&right.path))
    });
    rust_doc_includes.dedup();

    Some(ChunkedSource {
        chunks,
        rust_doc_includes,
    })
}

fn rust_doc_include(node: tree_sitter::Node<'_>, text: &str) -> Option<RustDocInclude> {
    let attribute = node.named_child(0)?;
    if attribute.kind() != "attribute" {
        return None;
    }
    let attribute_name = attribute.named_child(0)?.utf8_text(text.as_bytes()).ok()?;
    if attribute_name != "doc" {
        return None;
    }

    let value = attribute.child_by_field_name("value")?;
    if value.kind() != "macro_invocation" {
        return None;
    }
    let macro_name = value
        .child_by_field_name("macro")?
        .utf8_text(text.as_bytes())
        .ok()?;
    if macro_name != "include_str" {
        return None;
    }

    let mut value_cursor = value.walk();
    let token_tree = value
        .named_children(&mut value_cursor)
        .find(|child| child.kind() == "token_tree")?;
    let mut cursor = token_tree.walk();
    let mut arguments = token_tree.named_children(&mut cursor);
    let literal = arguments.next()?;
    if arguments.next().is_some()
        || !matches!(literal.kind(), "string_literal" | "raw_string_literal")
    {
        return None;
    }
    let literal_text = literal.utf8_text(text.as_bytes()).ok()?;
    let path = decode_rust_string_literal(literal_text)?;
    Some(RustDocInclude {
        source_line: node.start_position().row + 1,
        path: PathBuf::from(path),
    })
}

fn decode_rust_string_literal(literal: &str) -> Option<String> {
    if literal.starts_with('"') {
        return serde_json::from_str::<String>(literal).ok();
    }

    let quote = literal.find('"')?;
    if !literal[..quote].starts_with('r') {
        return None;
    }
    let hashes = &literal[1..quote];
    if !hashes.chars().all(|ch| ch == '#') {
        return None;
    }
    let suffix = format!("\"{hashes}");
    let value = literal.strip_suffix(&suffix)?;
    Some(value[quote + 1..].to_string())
}

pub(crate) fn chunk_rust_doc_include(
    owner_path: &Path,
    source_line: usize,
    included_path: &Path,
    text: &str,
) -> Vec<Chunk> {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    const WINDOW: usize = 80;
    const OVERLAP: usize = 20;
    const MAX_CHUNKS: usize = 64;
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < lines.len() && chunks.len() < MAX_CHUNKS {
        let end = (start + WINDOW).min(lines.len());
        let block = lines[start..end].join("\n");
        let text = format!(
            "// {}\n// Rust documentation included from {}\n\n{}",
            owner_path.to_string_lossy(),
            included_path.to_string_lossy(),
            block
        );
        chunks.push(make_chunk(
            owner_path,
            source_line,
            source_line,
            text,
            "rust".to_string(),
            ChunkKind::Documentation,
        ));
        if end == lines.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP);
    }
    chunks
}

thread_local! {
    /// Indexing already distributes files over worker threads. Reusing one
    /// parser on each worker avoids constructing a parser for every file.
    static TREE_SITTER_PARSER: std::cell::RefCell<tree_sitter::Parser> =
        std::cell::RefCell::new(tree_sitter::Parser::new());
}

/// Compiled queries are immutable and grammar-specific, so compile each one
/// once rather than once per file on large initial indexes.
fn tree_sitter_query(
    rel_path: &Path,
    language: &str,
    line_count: usize,
) -> Option<(tree_sitter::Language, &'static tree_sitter::Query)> {
    use std::sync::OnceLock;

    macro_rules! cached_query {
        ($cell:ident, $grammar:expr, $query:expr) => {{
            static $cell: OnceLock<Option<tree_sitter::Query>> = OnceLock::new();
            let grammar: tree_sitter::Language = $grammar.into();
            let query = $cell
                .get_or_init(|| tree_sitter::Query::new(&grammar, $query).ok())
                .as_ref()?;
            Some((grammar, query))
        }};
    }

    match language {
        "rust" => cached_query!(
            RUST_QUERY,
            tree_sitter_rust::LANGUAGE,
            "(function_item) @fn (impl_item) @class (trait_item) @class (inner_attribute_item) @doc_include (attribute_item) @doc_include"
        ),
        "python" => cached_query!(
            PYTHON_QUERY,
            tree_sitter_python::LANGUAGE,
            "(function_definition) @fn (class_definition) @class"
        ),
        "go" => cached_query!(
            GO_QUERY,
            tree_sitter_go::LANGUAGE,
            "(function_declaration) @fn (method_declaration) @fn (type_declaration) @class"
        ),
        "javascript" => cached_query!(
            JAVASCRIPT_QUERY,
            tree_sitter_javascript::LANGUAGE,
            "(function_declaration) @fn (method_definition) @fn (class_declaration) @class"
        ),
        "typescript"
            if rel_path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tsx")) =>
        {
            cached_query!(
                TSX_QUERY,
                tree_sitter_typescript::LANGUAGE_TSX,
                "(function_declaration) @fn (method_definition) @fn (class_declaration) @class (abstract_class_declaration) @class (interface_declaration) @class (type_alias_declaration) @class (enum_declaration) @class"
            )
        }
        "typescript" => cached_query!(
            TYPESCRIPT_QUERY,
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            "(function_declaration) @fn (method_definition) @fn (class_declaration) @class (abstract_class_declaration) @class (interface_declaration) @class (type_alias_declaration) @class (enum_declaration) @class"
        ),
        "java" => cached_query!(
            JAVA_QUERY,
            tree_sitter_java::LANGUAGE,
            "(class_declaration) @class (interface_declaration) @class (enum_declaration) @class (annotation_type_declaration) @class (method_declaration) @fn (constructor_declaration) @fn"
        ),
        "csharp" => cached_query!(
            CSHARP_QUERY,
            tree_sitter_c_sharp::LANGUAGE,
            "(class_declaration) @class (interface_declaration) @class (struct_declaration) @class (record_declaration) @class (enum_declaration) @class (method_declaration) @fn (constructor_declaration) @fn"
        ),
        "php" => cached_query!(
            PHP_QUERY,
            tree_sitter_php::LANGUAGE_PHP,
            "(class_declaration) @class (interface_declaration) @class (trait_declaration) @class (enum_declaration) @class (function_definition) @fn (method_declaration) @fn"
        ),
        "ruby" => cached_query!(
            RUBY_QUERY,
            tree_sitter_ruby::LANGUAGE,
            "(class) @class (module) @class (method) @fn (singleton_method) @fn"
        ),
        "swift" => cached_query!(
            SWIFT_QUERY,
            tree_sitter_swift::LANGUAGE,
            "(class_declaration) @class (struct_declaration) @class (protocol_declaration) @class (extension_declaration) @class (function_declaration) @fn (initializer_declaration) @fn"
        ),
        "c" => cached_query!(
            C_QUERY,
            tree_sitter_c::LANGUAGE,
            "(function_definition) @fn (struct_specifier) @class (enum_specifier) @class (union_specifier) @class"
        ),
        "cpp" => cached_query!(
            CPP_QUERY,
            tree_sitter_cpp::LANGUAGE,
            "(function_definition) @fn (class_specifier) @class (struct_specifier) @class (enum_specifier) @class (namespace_definition) @class"
        ),
        "scala" => cached_query!(
            SCALA_QUERY,
            tree_sitter_scala::LANGUAGE,
            "(class_definition) @class (trait_definition) @class (object_definition) @class (function_definition) @fn (val_definition) @fn"
        ),
        "kotlin" => cached_query!(
            KOTLIN_QUERY,
            tree_sitter_kotlin_ng::LANGUAGE,
            "(function_declaration) @fn (class_declaration) @class (object_declaration) @class (type_alias) @class"
        ),
        "elixir" => cached_query!(
            ELIXIR_QUERY,
            tree_sitter_elixir::LANGUAGE,
            "((call target: (identifier) @_module_keyword (arguments (alias)) (do_block)) @module (#any-of? @_module_keyword \"defmodule\" \"defprotocol\" \"defimpl\")) ((call target: (identifier) @_function_keyword (arguments [(identifier) (call target: (identifier)) (binary_operator left: (call target: (identifier)) operator: \"when\")]) (do_block)?) @fn (#any-of? @_function_keyword \"def\" \"defp\" \"defdelegate\" \"defguard\" \"defguardp\" \"defmacro\" \"defmacrop\" \"defn\" \"defnp\"))"
        ),
        "zig" => cached_query!(
            ZIG_QUERY,
            tree_sitter_zig::LANGUAGE,
            "(function_declaration) @fn (test_declaration) @fn (variable_declaration (struct_declaration)) @class (variable_declaration (enum_declaration)) @class (variable_declaration (union_declaration)) @class (variable_declaration (opaque_declaration)) @class (variable_declaration (error_set_declaration)) @class"
        ),
        "bash" | "shell" => cached_query!(
            BASH_QUERY,
            tree_sitter_bash::LANGUAGE,
            "(function_definition) @fn"
        ),
        "haskell" => cached_query!(
            HASKELL_QUERY,
            tree_sitter_haskell::LANGUAGE,
            "(function) @fn (signature) @fn (data_type) @class (class) @class (instance) @class"
        ),
        "ocaml" => cached_query!(
            OCAML_QUERY,
            tree_sitter_ocaml::LANGUAGE_OCAML,
            "(value_definition) @fn (type_definition) @class (module_definition) @class"
        ),
        "lua" => cached_query!(
            LUA_QUERY,
            tree_sitter_lua::LANGUAGE,
            "(function_declaration) @fn (function_definition) @fn"
        ),
        "dart" => cached_query!(
            DART_QUERY,
            tree_sitter_dart::LANGUAGE,
            "(class_declaration) @class (function_signature) @fn (method_signature) @fn"
        ),
        "objc" => cached_query!(
            OBJC_QUERY,
            tree_sitter_objc::LANGUAGE,
            "(class_interface) @class (class_implementation) @class (protocol_declaration) @class (category_interface) @class (category_implementation) @class"
        ),
        "perl" => cached_query!(
            PERL_QUERY,
            tree_sitter_perl::LANGUAGE,
            "(function_definition) @fn (package_statement) @class"
        ),
        "starlark"
            if rel_path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("bzl") || extension.eq_ignore_ascii_case("star")
                }) =>
        {
            cached_query!(
                STARLARK_DEFINITION_QUERY,
                tree_sitter_starlark::LANGUAGE,
                "(function_definition) @fn"
            )
        }
        "starlark" if line_count > STARLARK_TARGET_AST_LINE_THRESHOLD => {
            cached_query!(
                STARLARK_TARGET_QUERY,
                tree_sitter_starlark::LANGUAGE,
                "(expression_statement (call) @fn)"
            )
        }
        _ => None,
    }
}

// ── Signature Detection ────────────────────────────────────────────────────

fn starts_with_any(line: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| line.starts_with(p))
}

/// Returns true when a line looks like a C-family function definition:
/// contains `(`, ends with `{`, and is not a control-flow keyword.
fn is_c_like_function_line(trimmed: &str) -> bool {
    if !trimmed.contains('(') || !trimmed.ends_with('{') {
        return false;
    }
    !starts_with_any(
        trimmed,
        &[
            "if ", "if(", "else", "for ", "for(", "while ", "while(", "switch ", "switch(", "do ",
            "do{", "} else", "} catch", "return ", "return(", "#", "//", "/*", "case ",
        ],
    )
}

/// No structural boundaries — files use fixed-size window chunks.
fn detect_text_only(_trimmed: &str) -> Option<ChunkKind> {
    None
}

// ── Systems languages ──────────────────────────────────────────────────

fn detect_rust(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "fn ",
            "pub fn ",
            "pub(crate) fn ",
            "pub(super) fn ",
            "async fn ",
            "pub async fn ",
        ],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "struct ",
            "pub struct ",
            "enum ",
            "pub enum ",
            "trait ",
            "pub trait ",
            "union ",
            "pub union ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["impl ", "mod ", "pub mod ", "pub(crate) mod "]) {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_go(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("func ") {
        Some(ChunkKind::Function)
    } else if trimmed.starts_with("type ") {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_c(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["struct ", "union ", "enum ", "typedef "]) {
        Some(ChunkKind::Class)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_cpp(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &["class ", "struct ", "enum ", "union ", "typedef "],
    ) {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["namespace "]) {
        Some(ChunkKind::Module)
    } else if starts_with_any(trimmed, &["template "]) {
        Some(ChunkKind::Class)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_zig(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["fn ", "pub fn ", "export fn "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["const ", "pub const ", "var ", "pub var "])
        && trimmed.contains("struct")
    {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_nim(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["proc ", "func ", "method ", "iterator "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["type "]) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

// ── JVM languages ──────────────────────────────────────────────────────

fn detect_java(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.contains(" class ")
        || trimmed.starts_with("class ")
        || starts_with_any(
            trimmed,
            &["interface ", "public interface ", "protected interface "],
        )
    {
        Some(ChunkKind::Class)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_kotlin(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "fun ",
            "suspend fun ",
            "private fun ",
            "override fun ",
            "internal fun ",
        ],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "class ",
            "data class ",
            "sealed class ",
            "enum class ",
            "abstract class ",
            "open class ",
            "interface ",
            "object ",
            "annotation class ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_scala(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &["def ", "private def ", "override def ", "protected def "],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "class ",
            "case class ",
            "trait ",
            "object ",
            "abstract class ",
            "sealed trait ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_groovy(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["def ", "private def ", "static def "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["class ", "interface ", "enum "]) {
        Some(ChunkKind::Class)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── .NET ───────────────────────────────────────────────────────────────

fn detect_csharp(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "class ",
            "public class ",
            "internal class ",
            "abstract class ",
            "static class ",
            "sealed class ",
            "partial class ",
            "interface ",
            "public interface ",
            "struct ",
            "public struct ",
            "enum ",
            "public enum ",
            "record ",
            "public record ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["namespace "]) {
        Some(ChunkKind::Module)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── Web / scripting ────────────────────────────────────────────────────

fn detect_python(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["def ", "async def "]) {
        Some(ChunkKind::Function)
    } else if trimmed.starts_with("class ") {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_typescript_javascript(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "function ",
            "export function ",
            "async function ",
            "export async function ",
        ],
    ) || trimmed.contains(" => ")
    {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "class ",
            "export class ",
            "abstract class ",
            "export abstract class ",
            "interface ",
            "export interface ",
            "type ",
            "export type ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_ruby(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("def ") {
        Some(ChunkKind::Function)
    } else if trimmed.starts_with("class ") {
        Some(ChunkKind::Class)
    } else if trimmed.starts_with("module ") {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_php(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "function ",
            "public function ",
            "private function ",
            "protected function ",
            "static function ",
            "public static function ",
        ],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "class ",
            "interface ",
            "trait ",
            "abstract class ",
            "final class ",
            "enum ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else if trimmed.starts_with("namespace ") {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_perl(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("sub ") {
        Some(ChunkKind::Function)
    } else if trimmed.starts_with("package ") {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_lua(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["function ", "local function "]) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── Apple / mobile ─────────────────────────────────────────────────────

fn detect_swift(trimmed: &str) -> Option<ChunkKind> {
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
    for (index, raw_token) in tokens.iter().enumerate() {
        let token = raw_token.split(['(', ':', '{']).next().unwrap_or(raw_token);
        match token {
            "func" | "init" | "deinit" | "subscript" => return Some(ChunkKind::Function),
            "class"
                if tokens.get(index + 1).is_some_and(|next| {
                    matches!(
                        next.split(['(', ':', '{']).next().unwrap_or(next),
                        "func" | "var" | "let" | "subscript"
                    )
                }) =>
            {
                continue;
            }
            "class" | "struct" | "enum" | "protocol" | "extension" | "actor" => {
                return Some(ChunkKind::Class);
            }
            "public" | "private" | "fileprivate" | "internal" | "package" | "open" | "final"
            | "indirect" | "override" | "required" | "convenience" | "static" | "mutating"
            | "nonmutating" | "isolated" | "nonisolated" | "distributed" | "lazy" => {}
            _ if token.starts_with('@') => {}
            _ => return None,
        }
    }
    None
}

fn detect_dart(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &["class ", "abstract class ", "mixin ", "extension "],
    ) {
        Some(ChunkKind::Class)
    } else if is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_objc(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["@interface ", "@implementation ", "@protocol "]) {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["- (", "+ ("]) || is_c_like_function_line(trimmed) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── Functional languages ───────────────────────────────────────────────

fn detect_elixir(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["def ", "defp ", "defmacro ", "defmacrop "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["defmodule ", "defprotocol ", "defimpl "]) {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_erlang(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("-module(") {
        Some(ChunkKind::Module)
    } else if trimmed.starts_with("-export(")
        || trimmed.starts_with("-spec ")
        || (!trimmed.starts_with('%')
            && !trimmed.starts_with('-')
            && !trimmed.is_empty()
            && trimmed.as_bytes()[0].is_ascii_lowercase()
            && trimmed.contains('(')
            && trimmed.contains("->"))
    {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_haskell(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("module ") {
        Some(ChunkKind::Module)
    } else if !trimmed.starts_with(' ')
        && !trimmed.starts_with('-')
        && !trimmed.starts_with('{')
        && !trimmed.is_empty()
        && trimmed.contains(" :: ")
    {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &["data ", "newtype ", "class ", "instance ", "type "],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_ocaml(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["let ", "let rec "]) {
        Some(ChunkKind::Function)
    } else if trimmed.starts_with("type ") {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["module ", "module type "]) {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_clojure(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["(defn ", "(defn- ", "(defmacro ", "(defmethod "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["(deftype ", "(defrecord ", "(defprotocol "]) {
        Some(ChunkKind::Class)
    } else if trimmed.starts_with("(ns ") {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

// ── Scientific / data ──────────────────────────────────────────────────

fn detect_r(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.contains("<- function") || trimmed.contains("= function(") {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_julia(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(trimmed, &["function ", "macro "]) {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["struct ", "mutable struct ", "abstract type "]) {
        Some(ChunkKind::Class)
    } else if trimmed.starts_with("module ") {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

// ── Shell ──────────────────────────────────────────────────────────────

fn detect_shell(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("function ") || (trimmed.contains("()") && trimmed.ends_with('{')) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

fn detect_powershell(trimmed: &str) -> Option<ChunkKind> {
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("function ") || lower.starts_with("filter ") {
        Some(ChunkKind::Function)
    } else if lower.starts_with("class ") {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

// ── Query / schema ─────────────────────────────────────────────────────

fn detect_sql(trimmed: &str) -> Option<ChunkKind> {
    let upper = trimmed.to_ascii_uppercase();
    if starts_with_any(
        &upper,
        &[
            "CREATE FUNCTION",
            "CREATE PROCEDURE",
            "CREATE OR REPLACE FUNCTION",
            "CREATE OR REPLACE PROCEDURE",
        ],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        &upper,
        &[
            "CREATE TABLE",
            "CREATE VIEW",
            "CREATE INDEX",
            "CREATE TYPE",
            "CREATE OR REPLACE VIEW",
        ],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_protobuf(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("rpc ") {
        Some(ChunkKind::Function)
    } else if starts_with_any(trimmed, &["message ", "enum ", "service "]) {
        Some(ChunkKind::Class)
    } else {
        None
    }
}

fn detect_graphql(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "type ",
            "input ",
            "enum ",
            "interface ",
            "union ",
            "scalar ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else if starts_with_any(trimmed, &["query ", "mutation ", "subscription "]) {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── Infrastructure ─────────────────────────────────────────────────────

fn detect_terraform(trimmed: &str) -> Option<ChunkKind> {
    if starts_with_any(
        trimmed,
        &[
            "resource ",
            "data ",
            "module ",
            "provider ",
            "variable ",
            "output ",
            "locals ",
        ],
    ) {
        Some(ChunkKind::Module)
    } else {
        None
    }
}

fn detect_starlark(trimmed: &str) -> Option<ChunkKind> {
    if trimmed.starts_with("def ") {
        Some(ChunkKind::Function)
    } else {
        None
    }
}

// ── Internal helpers ───────────────────────────────────────────────────

fn is_probably_text(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(TEXT_SNIFF_BYTES)];
    if sample.is_empty() {
        return false;
    }

    if sample.contains(&0) {
        return false;
    }

    if std::str::from_utf8(sample).is_ok() {
        return true;
    }

    let printable = sample
        .iter()
        .filter(|&&byte| {
            matches!(byte, b'\n' | b'\r' | b'\t' | 0x0C) || (0x20..=0x7E).contains(&byte)
        })
        .count();

    (printable as f32 / sample.len() as f32) >= MIN_PRINTABLE_RATIO
}

fn collect_signatures(
    detect: fn(&str) -> Option<ChunkKind>,
    lines: &[&str],
) -> Vec<(usize, ChunkKind)> {
    let mut out = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(kind) = detect(trimmed) {
            out.push((idx + 1, kind));
        }
    }
    out
}

fn fallback_chunks(rel_path: &Path, language: &str, lines: &[&str]) -> Vec<Chunk> {
    let window = 80usize;
    let overlap = 20usize;

    let mut chunks = Vec::new();
    let mut start = 1usize;

    while start <= lines.len() {
        let end = (start + window - 1).min(lines.len());
        let block = lines[start - 1..end].join("\n");
        let text = format!("// {}\n\n{}", rel_path.to_string_lossy(), block);

        chunks.push(make_chunk(
            rel_path,
            start,
            end,
            text,
            language.to_string(),
            ChunkKind::Text,
        ));

        if end == lines.len() {
            break;
        }

        start = end.saturating_sub(overlap) + 1;
    }

    chunks
}

fn make_chunk(
    rel_path: &Path,
    start_line: usize,
    end_line: usize,
    text: String,
    language: String,
    kind: ChunkKind,
) -> Chunk {
    let mut content_hash_data = Vec::with_capacity(text.len() + 32);
    content_hash_data.extend_from_slice(rel_path.to_string_lossy().as_bytes());
    content_hash_data.extend_from_slice(&start_line.to_le_bytes());
    content_hash_data.extend_from_slice(&end_line.to_le_bytes());
    content_hash_data.extend_from_slice(text.as_bytes());
    let content_digest = xxhash_rust::xxh3::xxh3_128(&content_hash_data).to_le_bytes();
    let content_hash = hex::encode(content_digest);

    Chunk {
        // Transient chunk ids do not back persisted identity. Reuse the stable
        // digest to avoid an RNG syscall per chunk during indexing.
        id: Uuid::from_bytes(content_digest),
        file_path: rel_path.to_path_buf(),
        start_line,
        end_line,
        text,
        language,
        kind,
        content_hash,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_chunker_extracts_functions() {
        let src = r#"
pub fn calculate_tax(amount: f64) -> f64 {
    amount * 0.2
}

pub fn calculate_total(amount: f64) -> f64 {
    amount + calculate_tax(amount)
}
"#;

        let chunks = chunk_source(Path::new("src/tax.rs"), src);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Function);
        assert!(chunks[0].text.contains("calculate_tax"));
    }

    #[test]
    fn generated_rust_fallback_keeps_own_doc_comments() {
        let mut src = String::from("//! Deterministic generated module.\n\n");
        for index in 0..60 {
            src.push_str(&format!(
                "/// generated purpose {index}.\npub fn generated_operation_{index:03}(value: u64) -> u64 {{\n    value + {index}\n}}\n\n"
            ));
        }

        let chunks = chunk_source(Path::new("src/generated.rs"), &src);

        assert!(chunks.len() >= 60);
        assert!(
            chunks[0].text.contains("generated purpose 0"),
            "first generated chunk should keep its own doc comment: {:?}",
            chunks[0].text
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.text.contains("generated purpose 59")),
            "last generated chunk should keep its own doc comment"
        );
    }

    #[test]
    fn chunk_ids_are_stable_for_same_chunk_identity() {
        let src = "pub fn stable() {}\n";
        let first = chunk_source(Path::new("src/lib.rs"), src);
        let second = chunk_source(Path::new("src/lib.rs"), src);
        let changed_content = chunk_source(Path::new("src/lib.rs"), "pub fn changed() {}\n");
        let changed_path = chunk_source(Path::new("src/other.rs"), src);

        assert_eq!(first[0].id, second[0].id);
        assert_ne!(first[0].id, changed_content[0].id);
        assert_ne!(first[0].id, changed_path[0].id);
    }

    #[test]
    fn rust_doc_include_attributes_are_parser_derived() {
        let src = r###"
#![doc = include_str!("../docs/middleware.md")]
#[doc = include_str!(r#"route-layer.md"#)]
pub fn configure_router() {}

const TEMPLATE: &str = include_str!("not-doc.md");
#[doc = include_str!(concat!("not-", "literal.md"))]
pub fn unsupported_dynamic_include() {}
"###;

        let chunked = chunk_source_with_metadata(Path::new("src/middleware/mod.rs"), src);
        assert_eq!(
            chunked.rust_doc_includes,
            vec![
                RustDocInclude {
                    source_line: 2,
                    path: PathBuf::from("../docs/middleware.md"),
                },
                RustDocInclude {
                    source_line: 3,
                    path: PathBuf::from("route-layer.md"),
                },
            ]
        );
        assert!(
            chunked
                .chunks
                .iter()
                .any(|chunk| chunk.text.contains("configure_router"))
        );
    }

    #[test]
    fn rust_doc_include_chunks_are_bounded_and_owned_by_module() {
        let documentation = (0..181)
            .map(|line| format!("Tower middleware routing detail {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_rust_doc_include(
            Path::new("src/middleware/mod.rs"),
            7,
            Path::new("src/docs/middleware.md"),
            &documentation,
        );

        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|chunk| {
            chunk.file_path == Path::new("src/middleware/mod.rs")
                && chunk.start_line == 7
                && chunk.end_line == 7
                && chunk.language == "rust"
                && chunk.kind == ChunkKind::Documentation
                && chunk.text.contains("src/docs/middleware.md")
        }));

        let huge_documentation = "bounded documentation line\n".repeat(10_000);
        assert_eq!(
            chunk_rust_doc_include(
                Path::new("src/lib.rs"),
                1,
                Path::new("docs/huge.md"),
                &huge_documentation,
            )
            .len(),
            64
        );
    }

    #[test]
    fn fallback_chunker_splits_large_text() {
        let src = (0..250)
            .map(|i| format!("line_{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_source(Path::new("README.md"), &src);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.kind == ChunkKind::Text));
    }

    #[test]
    fn txt_files_are_indexable() {
        assert_eq!(language_for_path(Path::new("docs/notes.txt")), Some("text"));
        assert!(is_indexable_path(Path::new("docs/notes.txt")));
    }

    #[test]
    fn unknown_text_extensions_are_indexable_by_content() {
        assert!(is_indexable_file(
            Path::new("docs/pipeline.unknown"),
            b"hello from a custom extension\n"
        ));
    }

    #[test]
    fn binary_content_is_not_indexable() {
        let binary = b"\x89PNG\r\n\x1a\n\0\0\0IHDR";
        assert!(!is_indexable_file(Path::new("assets/logo.dat"), binary));
    }

    // ── New language tests ────────────────────────────────────────────

    #[test]
    fn c_chunker_extracts_functions_and_structs() {
        let src =
            "struct Point {\n    int x, y;\n};\n\nint add(int a, int b) {\n    return a + b;\n}\n";
        let chunks = chunk_source(Path::new("math.c"), src);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Class));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn swift_chunker_extracts_funcs_and_classes() {
        let src = "class AppDelegate {\n    func application() {\n    }\n}\n";
        let chunks = chunk_source(Path::new("App.swift"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Class));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn swift_fallback_detects_stacked_declaration_modifiers() {
        assert_eq!(
            detect_swift("public final class Application: Sendable {"),
            Some(ChunkKind::Class)
        );
        assert_eq!(
            detect_swift("public convenience init(environment: Environment) {"),
            Some(ChunkKind::Function)
        );
        assert_eq!(
            detect_swift("public class func bootstrap() {"),
            Some(ChunkKind::Function)
        );
        assert_eq!(detect_swift("public var application: Application {"), None);
    }

    #[test]
    fn shell_chunker_detects_functions() {
        let src =
            "#!/bin/bash\n\nsetup() {\n  echo setup\n}\n\nfunction teardown {\n  echo done\n}\n";
        let chunks = chunk_source(Path::new("run.sh"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn ruby_chunker_detects_class_and_method() {
        let src = "class Calculator\n  def add(a, b)\n    a + b\n  end\nend\n";
        let chunks = chunk_source(Path::new("calc.rb"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Class));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn kotlin_chunker_detects_class_and_fun() {
        let src = "data class User(val name: String) {\n}\n\nfun greet(user: User) {\n    println(user.name)\n}\n";
        let chunks = chunk_source(Path::new("User.kt"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Class));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn kotlin_tree_sitter_detects_modified_interfaces_and_type_aliases() {
        let src = "public interface Flow<out T> {\n    suspend fun collect(value: T)\n}\n\npublic typealias Channel<T> = Flow<T>\n";
        let chunks = chunk_source(Path::new("Flow.kt"), src);
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Class && chunk.text.contains("interface Flow")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Class && chunk.text.contains("typealias Channel")
        }));
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Function && chunk.text.contains("suspend fun collect")
        }));
    }

    #[test]
    fn starlark_registry_recognizes_bazel_paths() {
        for path in [
            "BUILD",
            "BUILD.bazel",
            "WORKSPACE",
            "MODULE.bazel",
            "defs.bzl",
            "rules.star",
        ] {
            assert_eq!(
                language_for_path(Path::new(path)),
                Some("starlark"),
                "{path} should be indexed as Starlark"
            );
        }
        assert_eq!(resolve_type_alias("bazel"), Some("starlark"));
        assert_eq!(resolve_type_alias("bzl"), Some("starlark"));
        assert_eq!(language_for_path(Path::new("BUILD.md")), Some("markdown"));
        assert_eq!(language_for_path(Path::new("MODULE.md")), Some("markdown"));
    }

    #[test]
    fn elixir_chunker_detects_module_and_function() {
        let src = "defmodule Math do\n  def add(a, b) do\n    a + b\n  end\nend\n";
        let chunks = chunk_source(Path::new("math.ex"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Module));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn elixir_tree_sitter_detects_qualified_modules_and_guarded_functions() {
        let src = "defmodule Phoenix.Channel do\n  def join(topic, socket) when is_binary(topic) do\n    {:ok, socket}\n  end\n\n  defp authorize(socket), do: socket\nend\n";
        let chunks = chunk_source(Path::new("channel.ex"), src);
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Module && chunk.text.contains("Phoenix.Channel")
        }));
        assert!(
            chunks.iter().any(|chunk| {
                chunk.kind == ChunkKind::Function && chunk.text.contains("def join")
            })
        );
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Function && chunk.text.contains("defp authorize")
        }));
    }

    #[test]
    fn zig_tree_sitter_detects_containers_and_functions() {
        let src = "pub const Client = struct {\n    pub fn send(self: *Client) void {\n        _ = self;\n    }\n};\n\npub const State = enum { ready, stopped };\n";
        let chunks = chunk_source(Path::new("client.zig"), src);
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Class && chunk.text.contains("const Client")
        }));
        assert!(
            chunks.iter().any(|chunk| {
                chunk.kind == ChunkKind::Class && chunk.text.contains("const State")
            })
        );
        assert!(
            chunks.iter().any(|chunk| {
                chunk.kind == ChunkKind::Function && chunk.text.contains("fn send")
            })
        );
    }

    #[test]
    fn leading_doc_comment_folds_into_following_definition() {
        // #59: a doc-comment directly above a function must be folded into the
        // function chunk, not emitted as its own standalone single-line chunk.
        let src = "package demo\n\n// CalculateTax computes the tax for an amount.\nfunc CalculateTax(amount int) int {\n\treturn amount * 2\n}\n";
        let chunks = chunk_source(Path::new("tax.go"), src);

        let func = chunks
            .iter()
            .find(|c| c.kind == ChunkKind::Function)
            .expect("function chunk should exist");
        assert!(
            func.text.contains("CalculateTax computes the tax"),
            "doc-comment should be folded into the function chunk, got: {:?}",
            func.text
        );
        // The comment must not survive as its own standalone 1-line Module chunk.
        let standalone_comment = chunks.iter().any(|c| {
            c.kind == ChunkKind::Module
                && c.start_line == c.end_line
                && c.text.contains("CalculateTax computes the tax")
        });
        assert!(
            !standalone_comment,
            "doc-comment must not be a standalone Module chunk"
        );
    }

    #[test]
    fn is_leading_doc_line_excludes_c_family_preprocessor() {
        // #59 review: a leading `#` is a comment in some languages but a
        // preprocessor directive in C/C++/Obj-C/C# — those must NOT be folded.
        assert!(!is_leading_doc_line("#include <stdio.h>", "c"));
        assert!(!is_leading_doc_line("#define SCALE 2", "cpp"));
        assert!(!is_leading_doc_line("#region Helpers", "csharp"));
        assert!(!is_leading_doc_line("#import <Foo.h>", "objc"));
        // `//` and block comments fold regardless of language.
        assert!(is_leading_doc_line("// doc", "c"));
        assert!(is_leading_doc_line("/// rustdoc", "rust"));
        // Rust attributes attach to the item below — fold them.
        assert!(is_leading_doc_line("#[derive(Debug)]", "rust"));
        // `#` is a genuine line comment in these languages — fold it.
        assert!(is_leading_doc_line("# explain the function", "python"));
        assert!(is_leading_doc_line("# explain the method", "ruby"));
    }

    #[test]
    fn c_preprocessor_not_folded_into_definition() {
        // End-to-end: #include/#define stay independently retrievable and are
        // not pulled into the following function chunk.
        let src = "#include <stdio.h>\n#define SCALE 2\n// doc: compute value\nint compute_value(int x) {\n    return x * SCALE;\n}\n";
        let chunks = chunk_source(Path::new("calc.c"), src);
        if let Some(func) = chunks.iter().find(|c| c.text.contains("compute_value")) {
            assert!(
                !func.text.contains("#include") && !func.text.contains("#define"),
                "preprocessor must not be folded into the function chunk: {:?}",
                func.text
            );
        }
        assert!(
            chunks.iter().any(|c| c.text.contains("#include")),
            "preprocessor directives must remain indexed somewhere"
        );
    }

    #[test]
    fn skips_minified_single_line_blobs() {
        // A large single-line blob (minified bundle) is not indexable.
        let minified = vec![b'a'; 60_000];
        assert!(!is_indexable_file(Path::new("bundle.min.js"), &minified));
        // A bundle with a short license banner before a giant minified body is
        // still detected (the long run is found beyond the prefix).
        let mut banner_then_blob = b"// Copyright 2026 Example Inc.\n// MIT License\n".to_vec();
        banner_then_blob.extend(std::iter::repeat_n(b'a', 60_000));
        assert!(!is_indexable_file(
            Path::new("vendor.min.js"),
            &banner_then_blob
        ));
        let mut streamed = std::io::Cursor::new(&banner_then_blob);
        assert!(!is_indexable_file_reader(Path::new("vendor.min.js"), &mut streamed).unwrap());
        // Normal multi-line source of similar size IS indexable.
        let mut normal = Vec::new();
        for _ in 0..3000 {
            normal.extend_from_slice(b"let x = computeValue(input);\n");
        }
        assert!(is_indexable_file(Path::new("app.js"), &normal));
        let mut streamed = std::io::Cursor::new(&normal);
        assert!(is_indexable_file_reader(Path::new("app.js"), &mut streamed).unwrap());
        // A short single-line file (under the threshold) stays indexable.
        assert!(is_indexable_file(
            Path::new("oneliner.js"),
            b"const x = 1;\n"
        ));
    }

    #[test]
    fn dockerfile_detected_by_filename() {
        assert_eq!(
            language_for_path(Path::new("Dockerfile")),
            Some("dockerfile")
        );
        assert_eq!(
            language_for_path(Path::new("Dockerfile.prod")),
            Some("dockerfile")
        );
    }

    #[test]
    fn makefile_detected_by_filename() {
        assert_eq!(language_for_path(Path::new("Makefile")), Some("makefile"));
    }

    #[test]
    fn sql_chunker_detects_create_statements() {
        let src = "CREATE TABLE users (\n  id INT PRIMARY KEY\n);\n\nCREATE FUNCTION add(a INT, b INT)\nRETURNS INT\nAS $$ SELECT a + b; $$;\n";
        let chunks = chunk_source(Path::new("schema.sql"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Class));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
    }

    #[test]
    fn new_extensions_are_recognized() {
        assert_eq!(language_for_path(Path::new("main.cpp")), Some("cpp"));
        assert_eq!(language_for_path(Path::new("App.cs")), Some("csharp"));
        assert_eq!(language_for_path(Path::new("lib.ex")), Some("elixir"));
        assert_eq!(language_for_path(Path::new("run.sh")), Some("shell"));
        assert_eq!(language_for_path(Path::new("query.sql")), Some("sql"));
        assert_eq!(language_for_path(Path::new("page.html")), Some("html"));
        assert_eq!(language_for_path(Path::new("style.css")), Some("css"));
        assert_eq!(language_for_path(Path::new("app.json")), Some("json"));
        assert_eq!(language_for_path(Path::new("main.tf")), Some("terraform"));
        assert_eq!(
            language_for_path(Path::new("schema.proto")),
            Some("protobuf")
        );
        assert_eq!(language_for_path(Path::new("script.lua")), Some("lua"));
        assert_eq!(language_for_path(Path::new("app.dart")), Some("dart"));
        assert_eq!(language_for_path(Path::new("main.swift")), Some("swift"));
        assert_eq!(language_for_path(Path::new("lib.hs")), Some("haskell"));
        assert_eq!(language_for_path(Path::new("User.kt")), Some("kotlin"));
        assert_eq!(language_for_path(Path::new("App.scala")), Some("scala"));
    }

    #[test]
    fn test_tree_sitter_timeout_fallback() {
        // 200k-deep nested brackets: pathological input for tree-sitter's parser
        let pathological_json = "[".repeat(200_000) + &"]".repeat(200_000);

        let start = std::time::Instant::now();
        let chunks = chunk_source(Path::new("massive.json"), &pathological_json);
        let elapsed = start.elapsed().as_millis();

        assert!(elapsed < 1000, "Chunking took too long: {}ms", elapsed);
        assert!(
            !chunks.is_empty(),
            "Fallback chunker should have returned chunks"
        );
    }

    #[test]
    fn go_chunker_detects_func_and_type() {
        let src = "package main\n\nfunc Add(a, b int) int {\n\treturn a + b\n}\n\ntype Config struct {\n\tHost string\n\tPort int\n}\n";
        let chunks = chunk_source(Path::new("main.go"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("Add")),
            "Go chunker should detect Add function"
        );
    }

    #[test]
    fn typescript_chunker_detects_function_and_class() {
        let src = "export function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n\nexport class UserService {\n  getUser(id: number): User {\n    return {} as User;\n  }\n}\n";
        let chunks = chunk_source(Path::new("service.ts"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("greet")),
            "TS chunker should detect greet function"
        );
    }

    #[test]
    fn typescript_chunker_detects_type_enum_and_abstract_class_definitions() {
        let src = "export type WSSHandlerOptions<T> = {\n  socket: T;\n};\n\nexport enum Transport {\n  WebSocket,\n}\n\nexport abstract class Adapter {\n  abstract run(): void;\n}\n";
        for path in ["service.ts", "service.tsx"] {
            let chunks = chunk_source(Path::new(path), src);
            for name in ["WSSHandlerOptions", "Transport", "Adapter"] {
                assert!(
                    chunks.iter().any(|chunk| {
                        chunk.kind == ChunkKind::Class && chunk.text.contains(name)
                    }),
                    "{path} should index {name} as a definition: {chunks:#?}"
                );
            }
        }
    }

    #[test]
    fn tree_sitter_parser_reuse_switches_grammars() {
        let go = chunk_source(
            Path::new("main.go"),
            "package main\n\nfunc LoadConfig() string { return \"go\" }\n",
        );
        let typescript = chunk_source(
            Path::new("service.ts"),
            "export function loadConfig(): string { return \"ts\"; }\n",
        );
        let rust = chunk_source(
            Path::new("lib.rs"),
            "pub fn load_config() -> &'static str { \"rust\" }\n",
        );

        assert!(go.iter().any(|c| c.text.contains("LoadConfig")));
        assert!(typescript.iter().any(|c| c.text.contains("loadConfig")));
        assert!(rust.iter().any(|c| c.text.contains("load_config")));
    }

    #[test]
    fn tsx_paths_use_the_tsx_grammar() {
        let src = "interface Props { title: string }\nexport function Card(props: Props) {\n  return <section>{props.title}</section>;\n}\n";
        let (tsx_grammar, _) =
            tree_sitter_query(Path::new("Card.tsx"), "typescript", src.lines().count()).unwrap();
        let (typescript_grammar, _) =
            tree_sitter_query(Path::new("card.ts"), "typescript", src.lines().count()).unwrap();
        let mut parser = tree_sitter::Parser::new();

        parser.set_language(&tsx_grammar).unwrap();
        let tsx_tree = parser.parse(src, None).unwrap();
        assert!(
            !tsx_tree.root_node().has_error(),
            "TSX syntax should parse without errors through the TSX grammar"
        );

        parser.set_language(&typescript_grammar).unwrap();
        let typescript_tree = parser.parse(src, None).unwrap();
        assert!(
            typescript_tree.root_node().has_error(),
            "fixture should require TSX rather than plain TypeScript grammar"
        );

        let chunks = chunk_source(Path::new("Card.tsx"), src);
        assert!(chunks.iter().any(|chunk| {
            chunk.kind == ChunkKind::Function && chunk.text.contains("<section>")
        }));
    }

    #[test]
    fn java_chunker_detects_class_and_method() {
        let src = "public class Calculator {\n    public int add(int a, int b) {\n        return a + b;\n    }\n\n    public int multiply(int a, int b) {\n        return a * b;\n    }\n}\n";
        let chunks = chunk_source(Path::new("Calculator.java"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("Calculator")),
            "Java chunker should detect Calculator class"
        );
    }

    #[test]
    fn csharp_chunker_detects_class_and_method() {
        let src = "public class BillingService {\n    public decimal CalculateTotal(decimal subtotal) {\n        return subtotal * 1.2m;\n    }\n}\n";
        let chunks = chunk_source(Path::new("BillingService.cs"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("CalculateTotal")),
            "C# chunker should detect CalculateTotal"
        );
    }

    #[test]
    fn php_chunker_detects_class_and_method() {
        let src = "<?php\nclass InvoiceService {\n    public function calculateTotal(float $subtotal): float {\n        return $subtotal * 1.2;\n    }\n}\n";
        let chunks = chunk_source(Path::new("InvoiceService.php"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("calculateTotal")),
            "PHP chunker should detect calculateTotal"
        );
    }

    #[test]
    fn ruby_chunker_detects_module_and_method() {
        let src = "module Billing\n  class InvoiceService\n    def calculate_total(subtotal)\n      subtotal * 1.2\n    end\n  end\nend\n";
        let chunks = chunk_source(Path::new("invoice_service.rb"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.text.contains("InvoiceService") || c.text.contains("calculate_total")),
            "Ruby chunker should detect class or method"
        );
    }

    #[test]
    fn swift_chunker_detects_struct_and_method() {
        let src = "struct InvoiceService {\n    func calculateTotal(subtotal: Double) -> Double {\n        subtotal * 1.2\n    }\n}\n";
        let chunks = chunk_source(Path::new("InvoiceService.swift"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("calculateTotal")),
            "Swift chunker should detect calculateTotal"
        );
    }

    #[test]
    fn python_chunker_detects_class_and_function() {
        let src =
            "class Engine:\n    def start(self):\n        pass\n\ndef helper():\n    return 42\n";
        let chunks = chunk_source(Path::new("engine.py"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.text.contains("Engine") || c.text.contains("helper")),
            "Python chunker should detect class or function"
        );
    }

    #[test]
    fn json_file_produces_chunks() {
        let src = r#"{"key": "value", "nested": {"a": 1, "b": 2}}"#;
        let chunks = chunk_source(Path::new("data.json"), src);
        assert!(!chunks.is_empty(), "JSON files should produce chunks");
    }

    #[test]
    fn yaml_file_produces_chunks() {
        let src = "name: test\nversion: 1.0\ndependencies:\n  - foo\n  - bar\n";
        let chunks = chunk_source(Path::new("config.yaml"), src);
        assert!(!chunks.is_empty(), "YAML files should produce chunks");
    }

    #[test]
    fn typescript_captures_top_level_constants() {
        let src = r#"import { Plugin } from "sdk";

const COMMAND_NAME = "gquota";

export function register(p: Plugin) {
    p.run(COMMAND_NAME);
}
"#;
        let chunks = chunk_source(Path::new("plugin.ts"), src);
        let all_text: String = chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all_text.contains("COMMAND_NAME"),
            "top-level constant must be captured in a chunk, got {} chunks covering: {:?}",
            chunks.len(),
            chunks
                .iter()
                .map(|c| format!("L{}-{}", c.start_line, c.end_line))
                .collect::<Vec<_>>()
        );
        assert!(
            all_text.contains("gquota"),
            "string literal 'gquota' must be present in chunk text"
        );
    }

    // ── Tree-sitter language promotion tests ──────────────────────────

    #[test]
    fn c_tree_sitter_detects_struct_and_function() {
        let src = "struct Point {\n    int x;\n    int y;\n};\n\nint add(int a, int b) {\n    return a + b;\n}\n";
        let chunks = chunk_source(Path::new("math.c"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Point"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Function && c.text.contains("add"))
        );
    }

    #[test]
    fn cpp_tree_sitter_detects_class_and_function() {
        let src =
            "class Engine {\npublic:\n    void start() {}\n};\n\nint main() {\n    return 0;\n}\n";
        let chunks = chunk_source(Path::new("engine.cpp"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Engine"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Function && c.text.contains("main"))
        );
    }

    #[test]
    fn scala_tree_sitter_detects_object_and_trait() {
        let src = "object Main {\n  def run(): Unit = println(\"hi\")\n}\n\ntrait Service {\n  def execute(): Int\n}\n";
        let chunks = chunk_source(Path::new("Main.scala"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Main"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Service"))
        );
    }

    #[test]
    fn bash_tree_sitter_detects_functions() {
        let src = "#!/bin/bash\n\nsetup() {\n    echo setup\n}\n\nfunction teardown {\n    echo done\n}\n";
        let chunks = chunk_source(Path::new("run.sh"), src);
        let fn_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.kind == ChunkKind::Function)
            .collect();
        assert!(
            fn_chunks.len() >= 2,
            "bash should detect at least 2 functions, got {}",
            fn_chunks.len()
        );
    }

    #[test]
    fn haskell_tree_sitter_detects_functions_and_data() {
        let src = "module Main where\n\nadd :: Int -> Int -> Int\nadd x y = x + y\n\ndata Color = Red | Blue\n";
        let chunks = chunk_source(Path::new("Main.hs"), src);
        assert!(
            chunks.iter().any(|c| c.text.contains("add")),
            "haskell should detect add function"
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Color"))
        );
    }

    #[test]
    fn ocaml_tree_sitter_detects_let_and_type() {
        let src = "let add a b = a + b\n\ntype color = Red | Blue\n\nmodule M = struct end\n";
        let chunks = chunk_source(Path::new("lib.ml"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Function && c.text.contains("add"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("color"))
        );
    }

    #[test]
    fn lua_tree_sitter_detects_functions() {
        let src = "function add(a, b)\n    return a + b\nend\n\nlocal function helper()\n    return 42\nend\n";
        let chunks = chunk_source(Path::new("util.lua"), src);
        let fn_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.kind == ChunkKind::Function)
            .collect();
        assert!(
            fn_chunks.len() >= 2,
            "lua should detect at least 2 functions, got {}",
            fn_chunks.len()
        );
    }

    #[test]
    fn dart_tree_sitter_detects_class_and_function() {
        let src = "class Calculator {\n  int add(int a, int b) {\n    return a + b;\n  }\n}\n\nvoid main() {\n  print('hi');\n}\n";
        let chunks = chunk_source(Path::new("main.dart"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Calculator"))
        );
        assert!(chunks.iter().any(|c| c.text.contains("main")));
    }

    #[test]
    fn objc_tree_sitter_detects_interface_and_implementation() {
        let src = "@interface Foo : NSObject\n- (void)bar;\n@end\n\n@implementation Foo\n- (void)bar {\n}\n@end\n";
        let chunks = chunk_source(Path::new("Foo.m"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Foo"))
        );
    }

    #[test]
    fn perl_tree_sitter_detects_sub_and_package() {
        let src = "package Math;\n\nsub add {\n    my ($a, $b) = @_;\n    return $a + $b;\n}\n\nsub mul {\n    return $_[0] * $_[1];\n}\n";
        let chunks = chunk_source(Path::new("Math.pm"), src);
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Class && c.text.contains("Math"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| c.kind == ChunkKind::Function && c.text.contains("add"))
        );
    }

    #[test]
    fn starlark_tree_sitter_chunks_macro_definitions() {
        let src = "def checkout_targets(name):\n    return [name]\n\ndef payment_targets(name):\n    return [name]\n";
        let chunks = chunk_source(Path::new("defs.bzl"), src);

        assert!(chunks.iter().all(|c| c.language == "starlark"));
        assert!(chunks.iter().any(|c| {
            c.kind == ChunkKind::Function
                && c.text.contains("checkout_targets")
                && !c.text.contains("payment_targets")
        }));
        assert!(chunks.iter().any(|c| {
            c.kind == ChunkKind::Function
                && c.text.contains("payment_targets")
                && !c.text.contains("checkout_targets")
        }));
    }

    #[test]
    fn starlark_cancelled_tree_sitter_parse_uses_fallback_path() {
        let src = "def target(name):\n    return name\n".repeat(2_000);
        let lines: Vec<&str> = src.lines().collect();

        assert!(
            try_tree_sitter_chunk_source_with_timeout(
                Path::new("defs.bzl"),
                &src,
                "starlark",
                &lines,
                std::time::Duration::ZERO,
            )
            .is_none()
        );
    }

    #[test]
    fn starlark_small_build_files_retain_text_chunks() {
        let src = "go_library(\n    name = \"checkout_target\",\n)\n\ngo_library(\n    name = \"payment_target\",\n)\n";
        let chunks = chunk_source(Path::new("BUILD.bazel"), src);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Text);
        assert!(chunks[0].text.contains("checkout_target"));
        assert!(chunks[0].text.contains("payment_target"));
    }

    #[test]
    fn starlark_very_large_build_files_chunk_individual_targets() {
        let padding = "# generated target metadata\n".repeat(STARLARK_TARGET_AST_LINE_THRESHOLD);
        let src = format!(
            "go_library(\n    name = \"checkout_target\",\n)\n{padding}\ngo_library(\n    name = \"payment_target\",\n)\n"
        );
        let chunks = chunk_source(Path::new("BUILD.bazel"), &src);

        assert!(chunks.iter().any(|c| {
            c.kind == ChunkKind::Function
                && c.text.contains("checkout_target")
                && !c.text.contains("payment_target")
        }));
        assert!(chunks.iter().any(|c| {
            c.kind == ChunkKind::Function
                && c.text.contains("payment_target")
                && !c.text.contains("checkout_target")
        }));
    }

    #[test]
    fn resolve_type_alias_canonical_names() {
        assert_eq!(resolve_type_alias("rust"), Some("rust"));
        assert_eq!(resolve_type_alias("python"), Some("python"));
        assert_eq!(resolve_type_alias("markdown"), Some("markdown"));
        assert_eq!(resolve_type_alias("javascript"), Some("javascript"));
        assert_eq!(resolve_type_alias("typescript"), Some("typescript"));
    }

    #[test]
    fn resolve_type_alias_extensions() {
        assert_eq!(resolve_type_alias("rs"), Some("rust"));
        assert_eq!(resolve_type_alias("py"), Some("python"));
        assert_eq!(resolve_type_alias("md"), Some("markdown"));
        assert_eq!(resolve_type_alias("go"), Some("go"));
        assert_eq!(resolve_type_alias("java"), Some("java"));
        assert_eq!(resolve_type_alias("cs"), Some("csharp"));
        assert_eq!(resolve_type_alias("cpp"), Some("cpp"));
        assert_eq!(resolve_type_alias("cc"), Some("cpp"));
        assert_eq!(resolve_type_alias("sql"), Some("sql"));
        assert_eq!(resolve_type_alias("html"), Some("html"));
        assert_eq!(resolve_type_alias("css"), Some("css"));
    }

    #[test]
    fn resolve_type_alias_common_aliases() {
        assert_eq!(resolve_type_alias("c++"), Some("cpp"));
        assert_eq!(resolve_type_alias("c#"), Some("csharp"));
        assert_eq!(resolve_type_alias("js"), Some("javascript"));
        assert_eq!(resolve_type_alias("ts"), Some("typescript"));
        assert_eq!(resolve_type_alias("bash"), Some("shell"));
        assert_eq!(resolve_type_alias("yml"), Some("config"));
        assert_eq!(resolve_type_alias("objective-c"), Some("objc"));
        assert_eq!(resolve_type_alias("proto"), Some("protobuf"));
    }

    #[test]
    fn resolve_type_alias_case_insensitive() {
        assert_eq!(resolve_type_alias("Rust"), Some("rust"));
        assert_eq!(resolve_type_alias("PYTHON"), Some("python"));
        assert_eq!(resolve_type_alias("Rs"), Some("rust"));
        assert_eq!(resolve_type_alias("PY"), Some("python"));
        assert_eq!(resolve_type_alias("C++"), Some("cpp"));
    }

    #[test]
    fn resolve_type_alias_with_dot_prefix() {
        assert_eq!(resolve_type_alias(".rs"), Some("rust"));
        assert_eq!(resolve_type_alias(".py"), Some("python"));
        assert_eq!(resolve_type_alias(".md"), Some("markdown"));
    }

    #[test]
    fn resolve_type_alias_unknown() {
        assert_eq!(resolve_type_alias("foobar"), None);
        assert_eq!(resolve_type_alias("xyz"), None);
    }
}
