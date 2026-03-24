//! Source-file chunking with a data-driven language registry.
//!
//! # Adding a new language
//!
//! 1. Add a [`LanguageDef`] entry to [`LANGUAGES`].
//! 2. Set `extensions` (without dots) and/or `filenames` for exact matches.
//! 3. Write a `detect_<lang>(trimmed_line) -> Option<ChunkKind>` function,
//!    or use [`detect_text_only`] for languages without structural boundaries.
//! 4. Done — indexing, search, and MCP pick it up automatically.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const TEXT_SNIFF_BYTES: usize = 8 * 1024;
const MIN_PRINTABLE_RATIO: f32 = 0.85;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ChunkKind {
    Function,
    Class,
    Module,
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

// ── Language Registry ──────────────────────────────────────────────────────

/// Defines a supported language: file-matching rules and structural chunking.
pub struct LanguageDef {
    /// Canonical language name used in search filters and metadata.
    pub name: &'static str,
    /// File extensions without leading dot. Matched case-insensitively.
    pub extensions: &'static [&'static str],
    /// Exact filename matches (e.g. `"Dockerfile"`). Also matches
    /// `Dockerfile.prod` (filename starts with pattern + `.`).
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
    // Filename matches first (Dockerfile, Makefile, Rakefile, …).
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        for lang in LANGUAGES {
            for pattern in lang.filenames {
                if filename == *pattern
                    || (filename.len() > pattern.len()
                        && filename.starts_with(pattern)
                        && filename.as_bytes()[pattern.len()] == b'.')
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

pub fn is_indexable_path(path: &Path) -> bool {
    language_for_path(path).is_some()
}

pub fn is_indexable_file(path: &Path, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if !is_probably_text(bytes) {
        return false;
    }
    if is_indexable_path(path) {
        return true;
    }
    // Unknown extension but content looks like text — index it anyway.
    true
}

pub fn chunk_source(rel_path: &Path, text: &str) -> Vec<Chunk> {
    let lang_def = find_language_def(rel_path);
    let language = lang_def.map(|d| d.name).unwrap_or("text").to_string();
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return vec![];
    }

    // Attempt 100% accurate AST chunking via Tree-sitter for supported languages
    if let Some(chunks) = try_tree_sitter_chunk_source(rel_path, text, &language, &lines)
        && !chunks.is_empty()
    {
        return chunks;
    }

    // Fall back to regex-based heuristic chunking
    let signatures = match lang_def {
        Some(def) => collect_signatures(def.detect_signature, &lines),
        None => vec![],
    };

    if signatures.is_empty() {
        return fallback_chunks(rel_path, &language, &lines);
    }

    let mut chunks = Vec::new();
    for (idx, (start_line, kind)) in signatures.iter().enumerate() {
        let start = *start_line;
        let end = signatures
            .get(idx + 1)
            .map(|(next, _)| next.saturating_sub(1))
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

    chunks
}

/// Uses Tree-sitter to reliably extract accurately bounded functions and classes
/// for supported main languages (Rust, Python, Go, JS, TS).
fn try_tree_sitter_chunk_source(
    rel_path: &Path,
    text: &str,
    language: &str,
    lines: &[&str],
) -> Option<Vec<Chunk>> {
    use streaming_iterator::StreamingIterator;
    use tree_sitter::{Parser, Query, QueryCursor};

    let mut parser = Parser::new();
    let query_str = match language {
        "rust" => {
            parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .ok()?;
            "(function_item) @fn (impl_item) @class (trait_item) @class"
        }
        "python" => {
            parser
                .set_language(&tree_sitter_python::LANGUAGE.into())
                .ok()?;
            "(function_definition) @fn (class_definition) @class"
        }
        "go" => {
            parser.set_language(&tree_sitter_go::LANGUAGE.into()).ok()?;
            "(function_declaration) @fn (method_declaration) @fn (type_declaration) @class"
        }
        "javascript" => {
            parser
                .set_language(&tree_sitter_javascript::LANGUAGE.into())
                .ok()?;
            "(function_declaration) @fn (method_definition) @fn (class_declaration) @class"
        }
        "typescript" => {
            parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                .ok()?;
            "(function_declaration) @fn (method_definition) @fn (class_declaration) @class (interface_declaration) @class"
        }
        _ => return None,
    };

    let tree = parser.parse(text, None)?;
    let query = Query::new(&parser.language().unwrap(), query_str).ok()?;
    let mut cursor = QueryCursor::new();

    let mut ranges = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), text.as_bytes());

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let start_line = capture.node.start_position().row;
            let end_line = capture.node.end_position().row;

            let kind = match capture.node.kind() {
                "class_definition"
                | "impl_item"
                | "trait_item"
                | "class_declaration"
                | "interface_declaration"
                | "type_declaration" => ChunkKind::Class,
                _ => ChunkKind::Function,
            };

            // Convert to 1-indexed bounds, end_line is inclusive in tree-sitter rows
            ranges.push((start_line + 1, end_line + 1, kind));
        }
    }

    if ranges.is_empty() {
        return None;
    }

    // Sort ranges by start line. We allow overlaps (e.g. an impl chunk containing multiple fn chunks).
    // Vector search actually benefits greatly from BOTH the large structural chunks AND the specific function chunks.
    ranges.sort_by_key(|r| r.0);

    let mut chunks = Vec::new();

    for (start, end, kind) in ranges {
        if start == 0 || start > lines.len() {
            continue;
        }
        let safe_end = end.min(lines.len());
        if safe_end < start {
            continue;
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
            kind,
        ));
    }

    Some(chunks)
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
    if starts_with_any(
        trimmed,
        &[
            "func ",
            "private func ",
            "public func ",
            "internal func ",
            "static func ",
            "override func ",
            "mutating func ",
            "init(",
            "deinit ",
        ],
    ) {
        Some(ChunkKind::Function)
    } else if starts_with_any(
        trimmed,
        &[
            "class ",
            "struct ",
            "enum ",
            "protocol ",
            "extension ",
            "public class ",
            "public struct ",
            "public enum ",
            "private class ",
            "final class ",
        ],
    ) {
        Some(ChunkKind::Class)
    } else {
        None
    }
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
    let mut hasher = Sha256::new();
    hasher.update(rel_path.to_string_lossy().as_bytes());
    hasher.update(start_line.to_le_bytes());
    hasher.update(end_line.to_le_bytes());
    hasher.update(text.as_bytes());
    let content_hash = hex::encode(hasher.finalize());

    Chunk {
        id: Uuid::new_v4(),
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
    fn elixir_chunker_detects_module_and_function() {
        let src = "defmodule Math do\n  def add(a, b) do\n    a + b\n  end\nend\n";
        let chunks = chunk_source(Path::new("math.ex"), src);
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Module));
        assert!(chunks.iter().any(|c| c.kind == ChunkKind::Function));
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
}
