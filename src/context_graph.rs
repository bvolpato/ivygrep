use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::chunking::{language_for_path, resolve_type_alias};
use crate::indexer::open_sqlite_readonly;
use crate::merkle::MerkleSnapshot;
use crate::path_glob::PathGlobMatcher;
use crate::search::SearchOptions;
use crate::workspace::{Workspace, index_path_string};

const MAX_EDGES_PER_FILE: usize = 64;
const MAX_UNRESOLVED_DEPENDENCIES_PER_FILE: usize = 256;
const MAX_GRAPH_EDGES: usize = 192;
const MAX_GRAPH_EXPANSIONS: usize = 12;
const MIN_STATIC_EDGES_BEFORE_COCHANGE: usize = 2;
const MAX_COCHANGE_COMMITS: usize = 64;
const MANIFEST_NAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Gemfile",
    "composer.json",
    "Package.swift",
    "pubspec.yaml",
    "mix.exs",
    "CMakeLists.txt",
    "MODULE.bazel",
    "WORKSPACE",
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(i64)]
pub(crate) enum FileEdgeKind {
    Dependency = 1,
    Test = 2,
    Config = 3,
    Documentation = 4,
    CoChange = 5,
}

impl FileEdgeKind {
    fn from_i64(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Dependency),
            2 => Some(Self::Test),
            3 => Some(Self::Config),
            4 => Some(Self::Documentation),
            5 => Some(Self::CoChange),
            _ => None,
        }
    }

    fn weight(self) -> f64 {
        match self {
            Self::Dependency => 1.0,
            Self::Test => 1.08,
            Self::Config => 0.72,
            Self::Documentation => 0.64,
            Self::CoChange => 0.58,
        }
    }

    pub(crate) fn source_label(self) -> &'static str {
        match self {
            Self::Dependency => "graph_dependency",
            Self::Test => "graph_test",
            Self::Config => "graph_config",
            Self::Documentation => "graph_documentation",
            Self::CoChange => "graph_cochange",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct FileEdge {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub kind: FileEdgeKind,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct UnresolvedDependency {
    pub source_path: PathBuf,
    pub language: String,
    pub spec: String,
    pub lookup_key: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FileGraphExtraction {
    pub edges: Vec<FileEdge>,
    pub unresolved_dependencies: Vec<UnresolvedDependency>,
}

struct RustCrateContext {
    name: String,
    source_root: PathBuf,
}

struct DartPackageContext {
    name: String,
    source_root: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphExpansion {
    pub file_path: PathBuf,
    pub seed_path: PathBuf,
    pub kind: FileEdgeKind,
    pub outgoing: bool,
    pub score: f64,
    pub cochange_count: usize,
}

impl GraphExpansion {
    pub(crate) fn reason(&self) -> String {
        let file = self.file_path.display();
        let seed = self.seed_path.display();
        match (self.kind, self.outgoing) {
            (FileEdgeKind::Dependency, true) => format!("{seed} depends on {file}"),
            (FileEdgeKind::Dependency, false) => format!("{file} depends on {seed}"),
            (FileEdgeKind::Test, true) => format!("{file} tests {seed}"),
            (FileEdgeKind::Test, false) => format!("{seed} tests {file}"),
            (FileEdgeKind::Config, true) => format!("{file} configures {seed}"),
            (FileEdgeKind::Config, false) => format!("{seed} configures {file}"),
            (FileEdgeKind::Documentation, true) => format!("{seed} documents {file}"),
            (FileEdgeKind::Documentation, false) => format!("{file} documents {seed}"),
            (FileEdgeKind::CoChange, _) => format!(
                "changed with {seed} in {} recent commit{}",
                self.cochange_count,
                if self.cochange_count == 1 { "" } else { "s" }
            ),
        }
    }
}

#[cfg(test)]
pub(crate) fn extract_file_edges(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
    content: &str,
) -> Vec<FileEdge> {
    extract_file_graph(root, snapshot, rel_path, content).edges
}

pub(crate) fn extract_file_graph(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
    content: &str,
) -> FileGraphExtraction {
    let mut edges = BTreeSet::new();
    let mut unresolved_dependencies = BTreeSet::new();
    let language = language_for_path(rel_path).unwrap_or("text");
    let local_rust_crate = (language == "rust" && rust_file_may_import_library(rel_path))
        .then(|| rust_crate_context(root, snapshot, rel_path))
        .flatten();

    if supports_dependency_scan(language) {
        for spec in dependency_specs(language, content) {
            if is_javascript_package_specifier(language, &spec) {
                continue;
            }
            if let Some(target_path) = resolve_local_dependency(
                root,
                snapshot,
                rel_path,
                language,
                &spec,
                local_rust_crate.as_ref(),
            ) {
                insert_edge(&mut edges, rel_path, &target_path, FileEdgeKind::Dependency);
            } else {
                for lookup_key in dependency_lookup_keys(&spec) {
                    unresolved_dependencies.insert(UnresolvedDependency {
                        source_path: rel_path.to_path_buf(),
                        language: language.to_string(),
                        spec: spec.clone(),
                        lookup_key,
                    });
                }
            }
            if edges.len() >= MAX_EDGES_PER_FILE {
                break;
            }
        }
    }

    if is_source_language(language)
        && let Some(manifest) = nearest_manifest(root, snapshot, rel_path, language)
    {
        insert_edge(&mut edges, rel_path, &manifest, FileEdgeKind::Config);
    }
    for related in likely_test_edges(root, snapshot, rel_path) {
        if path_looks_like_test(rel_path) {
            insert_edge(&mut edges, &related, rel_path, FileEdgeKind::Test);
        } else {
            insert_edge(&mut edges, rel_path, &related, FileEdgeKind::Test);
        }
    }
    if language == "markdown" {
        for spec in markdown_specs(content) {
            if let Some(target) =
                resolve_local_dependency(root, snapshot, rel_path, "markdown", &spec, None)
            {
                insert_edge(&mut edges, rel_path, &target, FileEdgeKind::Documentation);
            } else {
                for lookup_key in dependency_lookup_keys(&spec) {
                    unresolved_dependencies.insert(UnresolvedDependency {
                        source_path: rel_path.to_path_buf(),
                        language: language.to_string(),
                        spec: spec.clone(),
                        lookup_key,
                    });
                }
            }
        }
    }

    FileGraphExtraction {
        edges: edges.into_iter().take(MAX_EDGES_PER_FILE).collect(),
        unresolved_dependencies: unresolved_dependencies
            .into_iter()
            .take(MAX_UNRESOLVED_DEPENDENCIES_PER_FILE)
            .collect(),
    }
}

pub(crate) fn resolve_dependency_spec(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    source_path: &Path,
    language: &str,
    spec: &str,
) -> Option<PathBuf> {
    let local_rust_crate = (language == "rust" && rust_file_may_import_library(source_path))
        .then(|| rust_crate_context(root, snapshot, source_path))
        .flatten();
    resolve_local_dependency(
        root,
        snapshot,
        source_path,
        language,
        spec,
        local_rust_crate.as_ref(),
    )
}

pub(crate) fn dependency_lookup_keys(value: &str) -> BTreeSet<String> {
    let value = value.strip_prefix("mod/").unwrap_or(value);
    value
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

pub(crate) fn path_lookup_keys(path: &Path) -> BTreeSet<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .flat_map(dependency_lookup_keys)
        .collect()
}

pub(crate) fn is_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| MANIFEST_NAMES.contains(&name))
}

pub(crate) fn configuration_target(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
) -> Option<PathBuf> {
    language_for_path(rel_path)
        .filter(|language| is_source_language(language))
        .and_then(|language| nearest_manifest(root, snapshot, rel_path, language))
}

fn supports_dependency_scan(language: &str) -> bool {
    matches!(
        language,
        "rust"
            | "python"
            | "javascript"
            | "typescript"
            | "go"
            | "c"
            | "cpp"
            | "objc"
            | "java"
            | "kotlin"
            | "scala"
            | "groovy"
            | "csharp"
            | "ruby"
            | "php"
            | "dart"
            | "elixir"
            | "erlang"
            | "haskell"
            | "lua"
            | "zig"
            | "shell"
            | "starlark"
            | "protobuf"
    )
}

fn is_source_language(language: &str) -> bool {
    supports_dependency_scan(language)
        || matches!(
            language,
            "nim"
                | "perl"
                | "swift"
                | "ocaml"
                | "clojure"
                | "r"
                | "julia"
                | "powershell"
                | "sql"
                | "thrift"
                | "graphql"
                | "terraform"
        )
}

fn insert_edge(
    edges: &mut BTreeSet<FileEdge>,
    source_path: &Path,
    target_path: &Path,
    kind: FileEdgeKind,
) {
    if source_path != target_path {
        edges.insert(FileEdge {
            source_path: source_path.to_path_buf(),
            target_path: target_path.to_path_buf(),
            kind,
        });
    }
}

fn dependency_specs(language: &str, content: &str) -> Vec<String> {
    let mut specs = BTreeSet::new();
    let mut go_import_block = false;
    let mut javascript_static_declaration = false;
    let mut python_import_module: Option<String> = None;
    let mut rust_grouped_import: Option<(String, usize)> = None;
    let mut scala_grouped_import: Option<(String, usize)> = None;
    let mut starlark_load = false;
    for raw_line in content.lines().take(20_000) {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        match language {
            "rust" => {
                let line = strip_rust_visibility(line);
                if let Some((grouped, depth)) = rust_grouped_import.as_mut() {
                    let value = line
                        .split("//")
                        .next()
                        .unwrap_or(line)
                        .trim_end()
                        .trim_end_matches(';');
                    grouped.push(' ');
                    grouped.push_str(value);
                    *depth += value.matches('{').count();
                    *depth = depth.saturating_sub(value.matches('}').count());
                    if *depth == 0 {
                        let (grouped, _) = rust_grouped_import.take().unwrap();
                        specs.extend(expand_grouped_spec(&grouped));
                    }
                    continue;
                }
                if let Some(value) = line.strip_prefix("mod ") {
                    let value = value.split("//").next().unwrap_or(value).trim_end();
                    if let Some(module) = value.strip_suffix(';') {
                        specs.insert(format!("mod/{}", module.trim()));
                    }
                } else if let Some(value) = line.strip_prefix("use ") {
                    let value = value
                        .split("//")
                        .next()
                        .unwrap_or(value)
                        .trim_end()
                        .trim_end_matches(';');
                    let value = import_target_without_alias(language, value);
                    let depth = value
                        .matches('{')
                        .count()
                        .saturating_sub(value.matches('}').count());
                    if depth > 0 {
                        rust_grouped_import = Some((value.to_string(), depth));
                    } else {
                        specs.extend(expand_grouped_spec(value));
                    }
                }
            }
            "python" => {
                if let Some(module) = python_import_module.as_deref() {
                    if !line.starts_with('#') {
                        insert_python_import_specs(&mut specs, module, line);
                    }
                    if line.contains(')') {
                        python_import_module = None;
                    }
                } else if let Some(value) = line.strip_prefix("from ") {
                    if let Some((module, members)) = value.split_once(" import ") {
                        let module = module.trim();
                        insert_python_import_specs(&mut specs, module, members);
                        if members.trim_start().starts_with('(') && !members.contains(')') {
                            python_import_module = Some(module.to_string());
                        }
                    }
                } else if let Some(value) = line.strip_prefix("import ") {
                    for part in value.split(',') {
                        specs.insert(part.split_whitespace().next().unwrap_or("").to_string());
                    }
                }
            }
            "javascript" | "typescript" => {
                if line.starts_with("import ")
                    || line.starts_with("export {")
                    || line.starts_with("export *")
                    || line.starts_with("export type {")
                    || line.starts_with("export type *")
                {
                    javascript_static_declaration = true;
                }
                if javascript_static_declaration {
                    if let Some(spec) = javascript_module_specifier(line) {
                        specs.insert(spec);
                        javascript_static_declaration = false;
                    } else if line.ends_with(';') {
                        javascript_static_declaration = false;
                    }
                }
                for marker in ["require(", "import("] {
                    if let Some(value) = value_after_marker(line, marker) {
                        specs.insert(value);
                    }
                }
            }
            "go" => {
                if line == "import (" {
                    go_import_block = true;
                } else if go_import_block && line == ")" {
                    go_import_block = false;
                } else if (go_import_block || line.starts_with("import "))
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "c" | "cpp" | "objc" => {
                if line.starts_with("#include")
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "java" | "kotlin" | "scala" | "groovy" | "csharp" => {
                if language == "scala"
                    && let Some((grouped, depth)) = scala_grouped_import.as_mut()
                {
                    let value = line
                        .split("//")
                        .next()
                        .unwrap_or(line)
                        .trim_end()
                        .trim_end_matches(';');
                    grouped.push(' ');
                    grouped.push_str(value);
                    *depth += value.matches('{').count();
                    *depth = depth.saturating_sub(value.matches('}').count());
                    if *depth == 0 {
                        let (grouped, _) = scala_grouped_import.take().unwrap();
                        specs.extend(expand_grouped_spec(&grouped));
                    }
                    continue;
                }
                if let Some(value) = line
                    .strip_prefix("import ")
                    .or_else(|| line.strip_prefix("using "))
                    .or_else(|| {
                        if language == "csharp" {
                            line.strip_prefix("global using ")
                        } else {
                            None
                        }
                    })
                {
                    let value = value
                        .split("//")
                        .next()
                        .unwrap_or(value)
                        .trim_end()
                        .trim_end_matches(';');
                    let value = import_target_without_alias(language, value);
                    let static_import = value.starts_with("static ");
                    let value = if matches!(language, "java" | "groovy" | "csharp") {
                        value.strip_prefix("static ").unwrap_or(value)
                    } else {
                        value
                    };
                    let value = if language == "csharp" {
                        value.strip_prefix("global::").unwrap_or(value)
                    } else {
                        value
                    };
                    let grouped_depth = value
                        .matches('{')
                        .count()
                        .saturating_sub(value.matches('}').count());
                    if language == "scala" && grouped_depth > 0 {
                        scala_grouped_import = Some((value.to_string(), grouped_depth));
                        continue;
                    }
                    let wildcard =
                        value.ends_with(".*") || language == "scala" && value.ends_with("._");
                    let value = value.trim_end_matches(".*").trim_end_matches("._");
                    let value =
                        if static_import && matches!(language, "java" | "groovy") && !wildcard {
                            value.rsplit_once('.').map_or(value, |(owner, _)| owner)
                        } else {
                            value
                        };
                    if language == "scala" {
                        specs.extend(expand_grouped_spec(value));
                    } else {
                        specs.insert(value.to_string());
                    }
                }
            }
            "ruby" => {
                if (line.starts_with("require ") || line.starts_with("require_relative "))
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "php" => {
                if let Some(value) = line.strip_prefix("use ") {
                    let value = value.trim_end_matches(';');
                    let value = import_target_without_alias(language, value);
                    specs.insert(value.replace('\\', "/"));
                }
                if (line.starts_with("require") || line.starts_with("include"))
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "dart" => {
                if (line.starts_with("import ")
                    || line.starts_with("export ")
                    || line.starts_with("part "))
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "elixir" => {
                if let Some(value) = line
                    .strip_prefix("alias ")
                    .or_else(|| line.strip_prefix("import "))
                    .or_else(|| line.strip_prefix("use "))
                {
                    specs.insert(value.split(',').next().unwrap_or("").trim().to_string());
                }
            }
            "erlang" => {
                if line.starts_with("-include")
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            "haskell" => {
                if let Some(value) = line.strip_prefix("import ") {
                    let value = value.trim_start_matches("qualified ");
                    specs.insert(value.split_whitespace().next().unwrap_or("").to_string());
                }
            }
            "lua" => {
                if let Some(value) = value_after_marker(line, "require(")
                    .or_else(|| line.strip_prefix("require ").and_then(first_quoted_value))
                {
                    specs.insert(value);
                }
            }
            "zig" => {
                if let Some(value) = value_after_marker(line, "@import(") {
                    specs.insert(value);
                }
            }
            "shell" => {
                if let Some(value) = line
                    .strip_prefix("source ")
                    .or_else(|| line.strip_prefix(". "))
                {
                    specs.insert(value.split_whitespace().next().unwrap_or("").to_string());
                }
            }
            "starlark" => {
                if line.starts_with('#') {
                    continue;
                }
                if line.starts_with("load(") {
                    starlark_load = true;
                }
                if starlark_load {
                    if let Some(value) = first_quoted_value(line) {
                        specs.insert(value);
                        starlark_load = false;
                    } else if line.contains(')') {
                        starlark_load = false;
                    }
                }
            }
            "protobuf" => {
                if line.starts_with("import ")
                    && let Some(spec) = first_quoted_value(line)
                {
                    specs.insert(spec);
                }
            }
            _ => {}
        }
    }
    specs
        .into_iter()
        .filter(|spec| !spec.trim().is_empty())
        .take(128)
        .collect()
}

fn strip_rust_visibility(line: &str) -> &str {
    if let Some(line) = line.strip_prefix("pub ") {
        return line;
    }
    line.strip_prefix("pub(")
        .and_then(|line| line.split_once(") ").map(|(_, line)| line))
        .unwrap_or(line)
}

fn python_import_members(value: &str) -> impl Iterator<Item = &str> {
    value
        .trim_matches(['(', ')'])
        .split(',')
        .filter_map(|member| member.split_whitespace().next())
        .filter(|member| !member.is_empty() && *member != "*")
}

fn insert_python_import_specs(specs: &mut BTreeSet<String>, module: &str, members: &str) {
    let relative_members = !module.is_empty() && module.chars().all(|character| character == '.');
    if !relative_members {
        specs.insert(module.to_string());
    }
    for member in python_import_members(members) {
        specs.insert(if relative_members {
            format!("{module}{member}")
        } else {
            format!("{module}.{member}")
        });
    }
}

fn import_target_without_alias<'a>(language: &str, value: &'a str) -> &'a str {
    if language == "csharp"
        && let Some((_, target)) = value.split_once('=')
    {
        return target.trim();
    }
    if matches!(language, "rust" | "kotlin" | "scala" | "groovy" | "php")
        && !value.contains('{')
        && let Some((target, _)) = value.split_once(" as ")
    {
        return target.trim();
    }
    value
}

fn expand_grouped_spec(value: &str) -> Vec<String> {
    let value = value.trim();
    let Some(open) = value.find('{') else {
        let target = value.split_whitespace().next().unwrap_or("");
        let terminal = target.rsplit([':', '.']).next().unwrap_or(target);
        if terminal == "self" {
            return target
                .strip_suffix("::self")
                .or_else(|| target.strip_suffix(".self"))
                .filter(|parent| !parent.is_empty())
                .map(str::to_string)
                .into_iter()
                .collect();
        }
        return (!target.is_empty() && !matches!(terminal, "_" | "*"))
            .then(|| target.to_string())
            .into_iter()
            .collect();
    };
    let mut depth = 0_usize;
    let close = value[open..]
        .char_indices()
        .find_map(|(offset, character)| {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(open + offset);
                    }
                }
                _ => {}
            }
            None
        });
    let Some(close) = close else {
        return vec![value[..open].trim_end_matches("::").to_string()];
    };
    let prefix = &value[..open];
    let (prefix, separator) = if let Some(prefix) = prefix.strip_suffix("::") {
        (prefix, "::")
    } else if let Some(prefix) = prefix.strip_suffix('.') {
        (prefix, ".")
    } else {
        (prefix, "::")
    };
    let group = &value[open + 1..close];
    let mut members = Vec::new();
    let mut member_start = 0;
    let mut member_depth = 0_usize;
    for (offset, character) in group.char_indices() {
        match character {
            '{' => member_depth += 1,
            '}' => member_depth = member_depth.saturating_sub(1),
            ',' if member_depth == 0 => {
                members.push(&group[member_start..offset]);
                member_start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    members.push(&group[member_start..]);
    members
        .into_iter()
        .flat_map(|member| {
            let nested = format!("{prefix}{separator}{}", member.trim());
            expand_grouped_spec(&nested)
        })
        .collect()
}

fn first_quoted_value(value: &str) -> Option<String> {
    let (start, quote) = value
        .char_indices()
        .find(|(_, character)| matches!(character, '"' | '\''))?;
    let rest = &value[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn value_after_marker(value: &str, marker: &str) -> Option<String> {
    value
        .find(marker)
        .and_then(|offset| first_quoted_value(&value[offset + marker.len()..]))
}

fn javascript_module_specifier(line: &str) -> Option<String> {
    value_after_marker(line, " from ")
        .or_else(|| line.strip_prefix("import ").and_then(first_quoted_value))
}

fn is_javascript_package_specifier(language: &str, spec: &str) -> bool {
    matches!(language, "javascript" | "typescript")
        && !spec.starts_with('.')
        && !spec.starts_with('/')
        && !spec.starts_with("@/")
        && !spec.starts_with("~/")
}

fn resolve_local_dependency(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    source_path: &Path,
    language: &str,
    spec: &str,
    local_rust_crate: Option<&RustCrateContext>,
) -> Option<PathBuf> {
    let spec = spec
        .trim()
        .trim_matches(['"', '\'', ';'])
        .split(['?', '#'])
        .next()?
        .trim();
    if spec.is_empty()
        || spec.starts_with("http:")
        || spec.starts_with("https:")
        || spec.starts_with("data:")
        || spec.starts_with("node:")
        || spec.starts_with("dart:")
        || is_javascript_package_specifier(language, spec)
    {
        return None;
    }

    let source_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let mut normalized = spec.replace("::", "/").replace('\\', "/");
    let mut dart_package_source_root = None;
    if let Some(package_spec) = spec.strip_prefix("package:") {
        if language != "dart" {
            return None;
        }
        let (package, module) = package_spec.split_once('/')?;
        let context = dart_package_context(root, snapshot, source_path)?;
        if package != context.name {
            return None;
        }
        normalized = module.to_string();
        dart_package_source_root = Some(context.source_root);
    }
    let rust_module_declaration = language == "rust" && normalized.starts_with("mod/");
    let python_relative = language == "python" && spec.starts_with('.');
    let starlark_relative = language == "starlark" && spec.starts_with(':');
    let starlark_workspace = language == "starlark" && spec.starts_with("//");
    if language == "starlark" && spec.starts_with('@') {
        return None;
    }
    if python_relative {
        let levels = spec
            .chars()
            .take_while(|character| *character == '.')
            .count();
        let module = spec[levels..].replace('.', "/");
        normalized = format!("{}{module}", "../".repeat(levels.saturating_sub(1)));
    } else if starlark_relative {
        normalized = spec.trim_start_matches(':').replace(':', "/");
    } else if starlark_workspace {
        normalized = spec
            .trim_start_matches('/')
            .trim_start_matches(':')
            .replace(':', "/");
    } else if matches!(
        language,
        "python" | "java" | "kotlin" | "scala" | "groovy" | "csharp" | "haskell" | "elixir"
    ) {
        normalized = normalized.replace('.', "/");
    }
    let mut package_relative = false;
    if language == "rust"
        && let Some(crate_context) = local_rust_crate
        && let Some(local_spec) = normalized.strip_prefix(&crate_context.name)
        && let Some(local_spec) = local_spec.strip_prefix('/')
    {
        normalized = format!("crate/{local_spec}");
        package_relative = true;
    }
    let crate_relative = normalized.starts_with("crate/");
    let source_relative = normalized.starts_with("self/")
        || normalized.starts_with("super/")
        || normalized.starts_with("./")
        || normalized.starts_with("../")
        || python_relative
        || starlark_relative;
    if language == "rust"
        && !rust_module_declaration
        && !crate_relative
        && !source_relative
        && normalized.contains('/')
    {
        let mut module_candidates = Vec::new();
        let mut prefix = Some(Path::new(&normalized));
        while let Some(path) = prefix {
            if path.as_os_str().is_empty() {
                break;
            }
            for base in [source_dir, Path::new("src")] {
                module_candidates.push(base.join(path).with_extension("rs"));
                module_candidates.push(base.join(path).join("mod.rs"));
            }
            prefix = path.parent();
        }
        let local_module = module_candidates
            .into_iter()
            .any(|candidate| existing_workspace_file(root, snapshot, &candidate).is_some());
        if !local_module {
            return None;
        }
    }
    normalized = normalized
        .strip_prefix("crate/")
        .or_else(|| normalized.strip_prefix("mod/"))
        .or_else(|| normalized.strip_prefix("self/"))
        .or_else(|| normalized.strip_prefix("@/"))
        .or_else(|| normalized.strip_prefix("~/"))
        .unwrap_or(&normalized)
        .to_string();
    let mut super_prefix = String::new();
    while let Some(rest) = normalized.strip_prefix("super/") {
        super_prefix.push_str("../");
        normalized = rest.to_string();
    }
    if !super_prefix.is_empty() {
        normalized = format!("{super_prefix}{normalized}");
    }
    normalized = normalized.trim_start_matches("./").to_string();

    let mut targets = vec![PathBuf::from(&normalized)];
    if language == "kotlin" {
        let path = Path::new(&normalized);
        let member_starts_lowercase = path
            .file_name()
            .and_then(|value| value.to_str())
            .and_then(|value| value.chars().next())
            .is_some_and(char::is_lowercase);
        let owner = path.parent().filter(|owner| {
            owner
                .file_name()
                .and_then(|value| value.to_str())
                .and_then(|value| value.chars().next())
                .is_some_and(char::is_uppercase)
        });
        if member_starts_lowercase && let Some(owner) = owner {
            targets.push(owner.to_path_buf());
        }
    }
    if language == "rust" {
        let mut parent = Path::new(&normalized);
        while let Some(next) = parent.parent() {
            if next.as_os_str().is_empty() {
                break;
            }
            targets.push(next.to_path_buf());
            parent = next;
        }
    }
    if language == "go" {
        let components = Path::new(&normalized).components().collect::<Vec<_>>();
        for offset in 1..components.len() {
            let suffix = components[offset..].iter().collect::<PathBuf>();
            if !suffix.as_os_str().is_empty() {
                targets.push(suffix);
            }
        }
    }

    let mut bases = Vec::new();
    if let Some(source_root) = dart_package_source_root {
        bases.push(source_root);
    } else if rust_module_declaration {
        bases.push(rust_module_declaration_base(source_path));
    } else {
        if source_relative
            || matches!(language, "c" | "cpp" | "objc" | "ruby" | "shell")
            || language == "rust" && !crate_relative
        {
            bases.push(source_dir.to_path_buf());
        }
        if package_relative && let Some(crate_context) = local_rust_crate {
            bases.push(crate_context.source_root.clone());
        }
        bases.extend([
            PathBuf::new(),
            PathBuf::from("src"),
            PathBuf::from("lib"),
            PathBuf::from("app"),
        ]);
        if matches!(language, "java" | "kotlin" | "scala" | "groovy") {
            for ancestor in source_dir.ancestors() {
                let Some(parent) = ancestor.parent() else {
                    continue;
                };
                if ancestor
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| matches!(name, "java" | "kotlin" | "scala" | "groovy"))
                {
                    bases.push(parent.join(language));
                    for source_root in ["java", "kotlin", "scala", "groovy"] {
                        if source_root != language {
                            bases.push(parent.join(source_root));
                        }
                    }
                }
            }
        }
        for ancestor in source_dir.ancestors().take(4) {
            bases.push(ancestor.to_path_buf());
        }
    }

    let source_extension = source_path.extension().and_then(|value| value.to_str());
    let mut suffixes = vec![PathBuf::new()];
    for extension in source_extension
        .into_iter()
        .chain(common_extensions(language).iter().copied())
    {
        suffixes.push(PathBuf::from(format!(".{extension}")));
    }
    for index_name in module_index_names(language) {
        suffixes.push(PathBuf::from(index_name));
    }

    let mut tried = HashSet::new();
    for target in targets {
        for base in &bases {
            let base_target = base.join(&target);
            if tried.insert(base_target.clone())
                && let Some(relative) = existing_workspace_file(root, snapshot, &base_target)
            {
                return Some(relative);
            }
            for extension in node_source_extensions(language, &base_target) {
                let candidate = base_target.with_extension(extension);
                if tried.insert(candidate.clone())
                    && let Some(relative) = existing_workspace_file(root, snapshot, &candidate)
                {
                    return Some(relative);
                }
            }
            for suffix in suffixes.iter().skip(1) {
                let candidate = if suffix.to_string_lossy().starts_with('.') {
                    PathBuf::from(format!("{}{}", base_target.display(), suffix.display()))
                } else {
                    base_target.join(suffix)
                };
                if !tried.insert(candidate.clone()) {
                    continue;
                }
                if let Some(relative) = existing_workspace_file(root, snapshot, &candidate) {
                    return Some(relative);
                }
            }
            if language == "go"
                && let Some(package) = target.file_name().and_then(|value| value.to_str())
            {
                let candidate = base_target.join(format!("{package}.go"));
                if tried.insert(candidate.clone())
                    && let Some(relative) = existing_workspace_file(root, snapshot, &candidate)
                {
                    return Some(relative);
                }
            }
            if language == "go"
                && let Some(relative) = first_go_package_file(root, snapshot, &base_target)
            {
                return Some(relative);
            }
        }
    }
    None
}

fn node_source_extensions(language: &str, path: &Path) -> &'static [&'static str] {
    if !matches!(language, "javascript" | "typescript") {
        return &[];
    }
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") => &["ts", "tsx"],
        Some("jsx") => &["tsx"],
        Some("mjs") => &["mts"],
        Some("cjs") => &["cts"],
        _ => &[],
    }
}

fn first_go_package_file(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    directory: &Path,
) -> Option<PathBuf> {
    let directory = normalize_relative_path(directory)?;
    if let Some(snapshot) = snapshot {
        let prefix = format!("{}/", index_path_string(&directory));
        return snapshot
            .files
            .range(prefix.clone()..)
            .take_while(|(path, _)| path.starts_with(&prefix))
            .map(|(path, _)| Path::new(path))
            .filter(|path| path.parent() == Some(directory.as_path()))
            .find(|path| is_go_package_file(path))
            .map(Path::to_path_buf);
    }

    let absolute_directory = root.join(&directory);
    if !absolute_directory.is_dir() {
        return None;
    }
    let mut candidates = fs::read_dir(absolute_directory)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| directory.join(entry.file_name()))
        .filter(|path| root.join(path).is_file() && is_go_package_file(path))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}

fn is_go_package_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "go")
        && !path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.ends_with("_test"))
}

fn module_index_names(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["mod.rs", "lib.rs"],
        "python" => &["__init__.py"],
        "javascript" | "typescript" => &["index.ts", "index.tsx", "index.js"],
        _ => &[],
    }
}

fn common_extensions(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["rs"],
        "python" => &["py", "pyi"],
        "javascript" => &["js", "jsx", "mjs", "cjs", "ts", "tsx"],
        "typescript" => &["ts", "tsx", "js", "jsx"],
        "go" => &["go"],
        "c" => &["h", "c"],
        "cpp" => &["h", "hpp", "hh", "cpp", "cc", "cxx"],
        "java" => &["java", "kt", "scala", "groovy"],
        "kotlin" => &["kt", "java", "scala", "groovy", "kts"],
        "scala" => &["scala", "java", "kt", "groovy"],
        "groovy" => &["groovy", "java", "kt", "scala"],
        "csharp" => &["cs"],
        "ruby" => &["rb"],
        "php" => &["php"],
        "dart" => &["dart"],
        "elixir" => &["ex", "exs"],
        "erlang" => &["erl", "hrl"],
        "haskell" => &["hs"],
        "lua" => &["lua"],
        "zig" => &["zig"],
        "shell" => &["sh", "bash"],
        "protobuf" => &["proto"],
        _ => &[],
    }
}

fn existing_workspace_file(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    relative: &Path,
) -> Option<PathBuf> {
    let normalized = normalize_relative_path(relative)?;
    let exists = snapshot.map_or_else(
        || root.join(&normalized).is_file(),
        |snapshot| snapshot.files.contains_key(&index_path_string(&normalized)),
    );
    exists.then_some(normalized)
}

fn normalize_relative_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

fn nearest_manifest(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
    language: &str,
) -> Option<PathBuf> {
    let parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
    for directory in parent.ancestors() {
        for manifest in manifest_names_for_language(language) {
            let candidate = directory.join(manifest);
            if candidate != rel_path
                && existing_workspace_file(root, snapshot, &candidate).is_some()
            {
                return Some(candidate);
            }
        }
    }
    None
}

fn manifest_names_for_language(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["Cargo.toml"],
        "javascript" | "typescript" => &["package.json"],
        "python" => &["pyproject.toml"],
        "go" => &["go.mod"],
        "java" | "kotlin" | "scala" | "groovy" => &["pom.xml", "build.gradle", "build.gradle.kts"],
        "ruby" => &["Gemfile"],
        "php" => &["composer.json"],
        "swift" => &["Package.swift"],
        "elixir" => &["mix.exs"],
        "c" | "cpp" | "objc" => &["CMakeLists.txt"],
        "starlark" => &["MODULE.bazel", "WORKSPACE"],
        "dart" => &["pubspec.yaml"],
        _ => &[],
    }
}

fn rust_crate_context(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
) -> Option<RustCrateContext> {
    let manifest = nearest_manifest(root, snapshot, rel_path, "rust")?;
    let package_root = manifest.parent().unwrap_or_else(|| Path::new(""));
    let content = fs::read_to_string(root.join(&manifest)).ok()?;
    let document = content.parse::<toml_edit::DocumentMut>().ok()?;
    let library = document.get("lib").and_then(toml_edit::Item::as_table_like);
    let name = library
        .and_then(|table| table.get("name"))
        .and_then(toml_edit::Item::as_str)
        .or_else(|| {
            document
                .get("package")
                .and_then(toml_edit::Item::as_table_like)
                .and_then(|table| table.get("name"))
                .and_then(toml_edit::Item::as_str)
        })?
        .replace('-', "_");
    let source_root = library
        .and_then(|table| table.get("path"))
        .and_then(toml_edit::Item::as_str)
        .map(|path| package_root.join(path))
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| package_root.join("src"));
    Some(RustCrateContext { name, source_root })
}

fn dart_package_context(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
) -> Option<DartPackageContext> {
    let manifest = nearest_manifest(root, snapshot, rel_path, "dart")?;
    let package_root = manifest.parent().unwrap_or_else(|| Path::new(""));
    let content = fs::read_to_string(root.join(&manifest)).ok()?;
    let name = content.lines().find_map(|line| {
        line.strip_prefix("name:")
            .map(str::trim)
            .map(|name| name.split('#').next().unwrap_or(name).trim())
            .map(|name| name.trim_matches(['\'', '"']))
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    })?;
    Some(DartPackageContext {
        name,
        source_root: package_root.join("lib"),
    })
}

fn rust_file_may_import_library(path: &Path) -> bool {
    for parent in path.ancestors().skip(1) {
        if parent.file_name().is_some_and(|name| name == "src") {
            return path
                .strip_prefix(parent)
                .ok()
                .and_then(|relative| relative.components().next())
                .is_some_and(|component| {
                    matches!(component, Component::Normal(value) if value == "main.rs" || value == "bin")
                });
        }
    }
    true
}

fn rust_module_declaration_base(source_path: &Path) -> PathBuf {
    let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let crate_root_directory = parent.file_name().is_some_and(|name| {
        matches!(
            name.to_str(),
            Some("tests" | "examples" | "benches" | "bin")
        )
    });
    match source_path.file_stem().and_then(|value| value.to_str()) {
        Some("lib" | "main" | "mod" | "build") | None => parent.to_path_buf(),
        Some(_) if parent.as_os_str().is_empty() || crate_root_directory => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
    }
}

fn test_suffixes(extension: &str) -> &'static [&'static str] {
    if extension == "rb" {
        &["_test", "_tests", "_spec", ".test", ".spec"]
    } else {
        &["_test", "_tests", ".test", ".spec"]
    }
}

fn test_directories(extension: &str) -> &'static [&'static str] {
    match extension {
        "rb" => &["test", "tests", "spec", "specs"],
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => &["test", "tests", "__tests__"],
        _ => &["test", "tests"],
    }
}

fn likely_test_edges(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
) -> Vec<PathBuf> {
    let Some(stem) = rel_path.file_stem().and_then(|value| value.to_str()) else {
        return Vec::new();
    };
    let extension = rel_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
    let is_test = path_looks_like_test(rel_path);
    let production_stem = stem
        .strip_prefix("test_")
        .or_else(|| stem.strip_suffix("_test"))
        .or_else(|| stem.strip_suffix("_tests"))
        .or_else(|| stem.strip_suffix("_spec"))
        .or_else(|| stem.strip_suffix(".test"))
        .or_else(|| stem.strip_suffix(".spec"))
        .unwrap_or(stem);
    let mut candidates = BTreeSet::new();

    if is_test {
        let parent = strip_test_component(parent);
        for base in [
            parent.clone(),
            PathBuf::from("src").join(&parent),
            PathBuf::from("lib").join(&parent),
            PathBuf::from("app").join(&parent),
        ] {
            candidates.insert(base.join(format!("{production_stem}.{extension}")));
        }
    } else {
        candidates.insert(parent.join(format!("test_{stem}.{extension}")));
        for suffix in test_suffixes(extension) {
            candidates.insert(parent.join(format!("{stem}{suffix}.{extension}")));
        }
        let adjacent_tests = parent.join("__tests__");
        candidates.insert(adjacent_tests.join(format!("{stem}.{extension}")));
        for suffix in [".test", ".spec"] {
            candidates.insert(adjacent_tests.join(format!("{stem}{suffix}.{extension}")));
        }
        let relative_under_source = rel_path
            .strip_prefix("src")
            .or_else(|_| rel_path.strip_prefix("lib"))
            .or_else(|_| rel_path.strip_prefix("app"))
            .unwrap_or(rel_path);
        let source_root = ["src", "lib", "app"]
            .into_iter()
            .find(|source_root| rel_path.starts_with(source_root));
        for directory in test_directories(extension) {
            candidates.insert(PathBuf::from(directory).join(relative_under_source));
            if let Some(source_root) = source_root {
                candidates.insert(
                    PathBuf::from(source_root)
                        .join(directory)
                        .join(relative_under_source),
                );
            }
        }
        let mirrored_parent = relative_under_source
            .parent()
            .unwrap_or_else(|| Path::new(""));
        for directory in test_directories(extension) {
            let mut mirrored_bases = vec![PathBuf::from(directory).join(mirrored_parent)];
            if let Some(source_root) = source_root {
                mirrored_bases.push(
                    PathBuf::from(source_root)
                        .join(directory)
                        .join(mirrored_parent),
                );
            }
            for mirrored_base in mirrored_bases {
                for suffix in test_suffixes(extension) {
                    candidates.insert(mirrored_base.join(format!("{stem}{suffix}.{extension}")));
                }
                candidates.insert(mirrored_base.join(format!("test_{stem}.{extension}")));
            }
        }
        if let Some(file_name) = rel_path.file_name() {
            for directory in test_directories(extension) {
                candidates.insert(PathBuf::from(directory).join(file_name));
            }
        }
    }

    candidates
        .into_iter()
        .filter(|candidate| {
            candidate != rel_path && existing_workspace_file(root, snapshot, candidate).is_some()
        })
        .take(8)
        .collect()
}

fn strip_test_component(path: &Path) -> PathBuf {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value)
                if value != "test"
                    && value != "tests"
                    && value != "spec"
                    && value != "specs"
                    && value != "__tests__" =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect()
}

fn path_looks_like_test(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(value) if value == "test" || value == "tests" || value == "spec" || value == "specs" || value == "__tests__")
    }) || path
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|stem| {
            stem.ends_with("_test")
                || stem.ends_with("_tests")
                || stem.ends_with("_spec")
                || stem.ends_with(".test")
                || stem.ends_with(".spec")
                || stem.starts_with("test_")
        })
}

fn markdown_specs(content: &str) -> Vec<String> {
    let mut specs = BTreeSet::new();
    for line in content.lines().take(20_000) {
        let mut remaining = line;
        while let Some(open) = remaining.find("](") {
            remaining = &remaining[open + 2..];
            let Some(close) = remaining.find(')') else {
                break;
            };
            let target = remaining[..close]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(['<', '>']);
            if !target.is_empty()
                && !target.starts_with('#')
                && !target.starts_with('/')
                && !target.contains("://")
                && !target.starts_with("mailto:")
                && !target.starts_with("data:")
            {
                specs.insert(target.to_string());
            }
            remaining = &remaining[close + 1..];
        }
    }
    specs.into_iter().take(128).collect()
}

pub(crate) fn expand_context_graph(
    workspace: &Workspace,
    seed_paths: &[PathBuf],
    options: &SearchOptions,
) -> Result<Vec<GraphExpansion>> {
    if seed_paths.is_empty() {
        return Ok(Vec::new());
    }
    let seeds = seed_paths
        .iter()
        .map(|path| index_path_string(path))
        .collect::<BTreeSet<_>>();
    let mut edges = load_persisted_edges(workspace, &seeds)?;
    if edges.len() < MIN_STATIC_EDGES_BEFORE_COCHANGE {
        edges.extend(recent_cochange_edges(workspace, seed_paths));
    }
    edges.sort();
    edges.dedup();
    edges.truncate(MAX_GRAPH_EDGES);

    let ranks = personalized_page_rank(&edges, &seeds);
    let path_matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let expected_language = options.type_filter.as_deref().and_then(resolve_type_alias);
    let mut best = BTreeMap::<PathBuf, GraphExpansion>::new();
    for edge in edges {
        let source = index_path_string(&edge.source_path);
        let target = index_path_string(&edge.target_path);
        let mut relationships = Vec::with_capacity(2);
        if seeds.contains(&source) {
            relationships.push((edge.target_path.clone(), edge.source_path.clone(), true));
        }
        if seeds.contains(&target) {
            relationships.push((edge.source_path.clone(), edge.target_path.clone(), false));
        }
        for (neighbor, seed, outgoing) in relationships {
            let neighbor_string = index_path_string(&neighbor);
            if !workspace.root.join(&neighbor).is_file()
                || options
                    .scope_filter
                    .as_ref()
                    .is_some_and(|scope| !scope.matches(&neighbor))
                || !path_matcher.matches(&neighbor)
                || expected_language
                    .is_some_and(|expected| language_for_path(&neighbor) != Some(expected))
            {
                continue;
            }
            let score =
                ranks.get(&neighbor_string).copied().unwrap_or_default() + edge.kind.weight() * 0.1;
            let expansion = GraphExpansion {
                file_path: neighbor.clone(),
                seed_path: seed,
                kind: edge.kind,
                outgoing,
                score,
                cochange_count: edge.cochange_count,
            };
            match best.get(&neighbor) {
                Some(existing) if existing.score >= expansion.score => {}
                _ => {
                    best.insert(neighbor, expansion);
                }
            }
        }
    }

    let mut expansions = best.into_values().collect::<Vec<_>>();
    expansions.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    expansions.truncate(MAX_GRAPH_EXPANSIONS);
    Ok(expansions)
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RankedEdge {
    source_path: PathBuf,
    target_path: PathBuf,
    kind: FileEdgeKind,
    cochange_count: usize,
}

fn load_persisted_edges(
    workspace: &Workspace,
    seeds: &BTreeSet<String>,
) -> Result<Vec<RankedEdge>> {
    let mut edges = Vec::new();
    let primary_path = if workspace.has_overlay() {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    };
    if primary_path.is_file() {
        edges.extend(query_edges(&open_sqlite_readonly(&primary_path)?, seeds)?);
    }
    if let Some(base_dir) = &workspace.base_index_dir {
        let shadowed = overlay_shadowed_paths(workspace);
        let base_path = base_dir.join("metadata.sqlite3");
        if base_path.is_file() {
            edges.extend(
                query_edges(&open_sqlite_readonly(&base_path)?, seeds)?
                    .into_iter()
                    .filter(|edge| !shadowed.contains(&index_path_string(&edge.source_path))),
            );
        }
    }
    Ok(edges)
}

fn query_edges(conn: &Connection, seeds: &BTreeSet<String>) -> Result<Vec<RankedEdge>> {
    let table_exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'file_edges')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !table_exists {
        return Ok(Vec::new());
    }
    let mut statement = conn.prepare_cached(
        "SELECT source_path, target_path, kind FROM file_edges
         WHERE source_path = ?1 OR target_path = ?1
         ORDER BY source_path, target_path, kind",
    )?;
    let mut edges = Vec::new();
    for seed in seeds {
        let rows = statement.query_map([seed], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (source, target, kind) = row?;
            if let Some(kind) = FileEdgeKind::from_i64(kind) {
                edges.push(RankedEdge {
                    source_path: PathBuf::from(source),
                    target_path: PathBuf::from(target),
                    kind,
                    cochange_count: 0,
                });
            }
        }
    }
    Ok(edges)
}

fn overlay_shadowed_paths(workspace: &Workspace) -> HashSet<String> {
    if !workspace.overlay_sqlite_path().is_file() {
        return HashSet::new();
    }
    let Ok(conn) = open_sqlite_readonly(&workspace.overlay_sqlite_path()) else {
        return HashSet::new();
    };
    let mut paths = HashSet::new();
    for table in ["tombstones", "chunks"] {
        let query = if table == "chunks" {
            "SELECT DISTINCT file_path FROM chunks"
        } else {
            "SELECT file_path FROM tombstones"
        };
        let Ok(mut statement) = conn.prepare(query) else {
            continue;
        };
        let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
            continue;
        };
        paths.extend(rows.filter_map(Result::ok));
    }
    paths
}

fn personalized_page_rank(edges: &[RankedEdge], seeds: &BTreeSet<String>) -> HashMap<String, f64> {
    let mut adjacency = HashMap::<String, Vec<(String, f64)>>::new();
    for edge in edges {
        let source = index_path_string(&edge.source_path);
        let target = index_path_string(&edge.target_path);
        let weight = edge.kind.weight() * (1.0 + edge.cochange_count as f64).ln_1p();
        adjacency
            .entry(source.clone())
            .or_default()
            .push((target.clone(), weight));
        adjacency.entry(target).or_default().push((source, weight));
    }
    let nodes = adjacency.keys().cloned().collect::<Vec<_>>();
    if nodes.is_empty() {
        return HashMap::new();
    }
    let seed_count = seeds.len().max(1) as f64;
    let mut rank = nodes
        .iter()
        .map(|node| {
            (
                node.clone(),
                if seeds.contains(node) {
                    1.0 / seed_count
                } else {
                    0.0
                },
            )
        })
        .collect::<HashMap<_, _>>();
    for _ in 0..12 {
        let mut next = nodes
            .iter()
            .map(|node| {
                (
                    node.clone(),
                    if seeds.contains(node) {
                        0.15 / seed_count
                    } else {
                        0.0
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        for (source, neighbors) in &adjacency {
            let total_weight = neighbors.iter().map(|(_, weight)| weight).sum::<f64>();
            if total_weight == 0.0 {
                continue;
            }
            let source_rank = rank.get(source).copied().unwrap_or_default();
            for (target, weight) in neighbors {
                *next.entry(target.clone()).or_default() +=
                    0.85 * source_rank * weight / total_weight;
            }
        }
        rank = next;
    }
    rank
}

fn recent_cochange_edges(workspace: &Workspace, seed_paths: &[PathBuf]) -> Vec<RankedEdge> {
    if !workspace.root.join(".git").exists() {
        return Vec::new();
    }
    const COMMIT_MARKER: &str = "__IVYGREP_COMMIT__";
    let Ok(log) = Command::new("git")
        .args([
            "log",
            &format!("--max-count={MAX_COCHANGE_COMMITS}"),
            "--no-merges",
            "--format=format:__IVYGREP_COMMIT__",
            "--name-only",
            "--no-renames",
            "HEAD",
        ])
        .current_dir(&workspace.root)
        .output()
    else {
        return Vec::new();
    };
    if !log.status.success() {
        return Vec::new();
    }

    let seeds = seed_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut counts = HashMap::<(PathBuf, PathBuf), usize>::new();
    let mut commit_paths = BTreeSet::new();
    let record_commit = |paths: &BTreeSet<PathBuf>, counts: &mut HashMap<_, _>| {
        let commit_seeds = paths.intersection(&seeds).cloned().collect::<Vec<_>>();
        for seed in commit_seeds {
            for path in paths {
                if path != &seed && workspace.root.join(path).is_file() {
                    *counts.entry((seed.clone(), path.clone())).or_default() += 1;
                }
            }
        }
    };
    for line in String::from_utf8_lossy(&log.stdout).lines() {
        let line = line.trim();
        if line == COMMIT_MARKER {
            record_commit(&commit_paths, &mut counts);
            commit_paths.clear();
        } else if !line.is_empty() {
            commit_paths.insert(PathBuf::from(line));
        }
    }
    record_commit(&commit_paths, &mut counts);

    let mut related = counts.into_iter().collect::<Vec<_>>();
    related.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    related
        .into_iter()
        .take(8)
        .map(|((seed, path), count)| RankedEdge {
            source_path: seed,
            target_path: path,
            kind: FileEdgeKind::CoChange,
            cochange_count: count,
        })
        .collect()
}

pub(crate) fn persist_file_edge(
    statement: &mut rusqlite::Statement<'_>,
    edge: &FileEdge,
) -> Result<()> {
    statement.execute(params![
        index_path_string(&edge.source_path),
        index_path_string(&edge.target_path),
        edge.kind as i64,
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn extracts_dependencies_tests_config_and_docs() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::create_dir_all(root.path().join("docs")).unwrap();
        fs::write(root.path().join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
        fs::write(root.path().join("src/helper.rs"), "pub fn helper() {}\n").unwrap();
        fs::write(root.path().join("tests/lib.rs"), "#[test]\nfn works() {}\n").unwrap();
        fs::write(root.path().join("docs/guide.md"), "# Guide\n").unwrap();
        fs::write(root.path().join("README.md"), "[guide](docs/guide.md)\n").unwrap();

        let source = "mod helper;\npub fn run() { helper::helper(); }\n";
        let edges = extract_file_edges(root.path(), None, Path::new("src/lib.rs"), source);
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/helper.rs")
        }));
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test && edge.target_path == Path::new("tests/lib.rs")
        }));
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Config && edge.target_path == Path::new("Cargo.toml")
        }));

        let docs = extract_file_edges(
            root.path(),
            None,
            Path::new("README.md"),
            "Read the [guide](docs/guide.md).",
        );
        assert!(docs.iter().any(|edge| {
            edge.kind == FileEdgeKind::Documentation
                && edge.target_path == Path::new("docs/guide.md")
        }));
    }

    #[test]
    fn config_edges_follow_source_ecosystem() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
        fs::write(root.path().join("package.json"), "{\"name\":\"demo\"}\n").unwrap();

        for (source, expected, unrelated) in [
            ("src/lib.rs", "Cargo.toml", "package.json"),
            ("frontend/main.ts", "package.json", "Cargo.toml"),
        ] {
            let edges = extract_file_edges(root.path(), None, Path::new(source), "");
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Config && edge.target_path == Path::new(expected)
            }));
            assert!(!edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Config && edge.target_path == Path::new(unrelated)
            }));
        }
    }

    #[test]
    fn missing_markdown_targets_are_persisted_for_later_resolution() {
        let root = tempfile::tempdir().unwrap();
        let content = "Read the [release guide](docs/release-guide.md).\n";

        let missing = extract_file_graph(root.path(), None, Path::new("README.md"), content);
        assert!(missing.edges.is_empty());
        assert!(missing.unresolved_dependencies.iter().any(|dependency| {
            dependency.language == "markdown"
                && dependency.spec == "docs/release-guide.md"
                && dependency.lookup_key == "guide"
        }));

        fs::create_dir_all(root.path().join("docs")).unwrap();
        fs::write(root.path().join("docs/release-guide.md"), "# Release\n").unwrap();
        let resolved = extract_file_graph(root.path(), None, Path::new("README.md"), content);
        assert!(resolved.unresolved_dependencies.is_empty());
        assert!(resolved.edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Documentation
                && edge.target_path == Path::new("docs/release-guide.md")
        }));
    }

    #[test]
    fn grouped_rust_imports_resolve_independently() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/auth")).unwrap();
        fs::write(root.path().join("src/auth/token.rs"), "pub struct Token;\n").unwrap();
        fs::write(
            root.path().join("src/auth/session.rs"),
            "pub struct Session;\n",
        )
        .unwrap();
        fs::write(root.path().join("src/auth.rs"), "pub mod token;\n").unwrap();
        fs::write(
            root.path().join("src/auth/self.rs"),
            "pub struct WrongSelf;\n",
        )
        .unwrap();
        fs::write(root.path().join("src/clock.rs"), "pub struct Clock;\n").unwrap();
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/lib.rs"),
            "use crate::{\n    auth::{self as auth_mod, token, session},\n    clock,\n};\n",
        );
        let targets = edges
            .iter()
            .filter(|edge| edge.kind == FileEdgeKind::Dependency)
            .map(|edge| edge.target_path.clone())
            .collect::<BTreeSet<_>>();
        assert!(targets.contains(Path::new("src/auth/token.rs")));
        assert!(targets.contains(Path::new("src/auth/session.rs")));
        assert!(targets.contains(Path::new("src/auth.rs")));
        assert!(!targets.contains(Path::new("src/auth/self.rs")));
        assert!(targets.contains(Path::new("src/clock.rs")));
    }

    #[test]
    fn rust_inline_comments_do_not_break_module_resolution() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/helper.rs"), "pub fn run() {}\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/lib.rs"),
            "mod helper; // auth helper\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/helper.rs")
        }));
    }

    #[test]
    fn rust_symbol_import_resolves_owning_module() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/auth.rs"),
            "pub fn rotate_refresh_token() {}\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/session.rs"),
            "use crate::auth::rotate_refresh_token;\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/auth.rs")
        }));
    }

    #[test]
    fn rust_package_import_resolves_local_library_module() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("crates/core/src")).unwrap();
        fs::create_dir_all(root.path().join("crates/core/tests")).unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("crates/core/Cargo.toml"),
            "[package]\nname = 'core-package'\nversion = '0.1.0'\n",
        )
        .unwrap();
        fs::write(
            root.path().join("crates/core/src/auth.rs"),
            "pub struct Session;\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/auth.rs"),
            "pub struct WrongSession;\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("crates/core/tests/integration.rs"),
            "use core_package::auth::Session;\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("crates/core/src/auth.rs")
        }));
    }

    #[test]
    fn rust_crate_name_lookup_is_limited_to_external_consumers() {
        for path in ["src/lib.rs", "src/auth.rs", "crates/core/src/auth.rs"] {
            assert!(!rust_file_may_import_library(Path::new(path)), "{path}");
        }
        for path in [
            "src/main.rs",
            "src/bin/server.rs",
            "tests/integration.rs",
            "crates/core/tests/integration.rs",
        ] {
            assert!(rust_file_may_import_library(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn source_file_finds_common_root_test_names() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::write(
            root.path().join("tests/auth_test.rs"),
            "#[test]\nfn works() {}\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/auth.rs"),
            "pub fn authenticate() {}\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test && edge.target_path == Path::new("tests/auth_test.rs")
        }));
    }

    #[test]
    fn source_file_finds_nested_source_root_test_mirror() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/foo")).unwrap();
        fs::create_dir_all(root.path().join("src/__tests__/foo")).unwrap();
        fs::write(
            root.path().join("src/__tests__/foo/user.test.ts"),
            "test('user', () => user());\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/foo/user.ts"),
            "export function user() {}\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test
                && edge.target_path == Path::new("src/__tests__/foo/user.test.ts")
        }));
    }

    #[test]
    fn source_file_finds_colocated_pytest_module() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/test_auth.py"),
            "def test_auth(): pass\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/auth.py"),
            "def authenticate(): pass\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test
                && edge.source_path == Path::new("src/auth.py")
                && edge.target_path == Path::new("src/test_auth.py")
        }));
    }

    #[test]
    fn ruby_spec_files_link_to_production_in_both_directions() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("app/models")).unwrap();
        fs::create_dir_all(root.path().join("spec/models")).unwrap();
        fs::write(root.path().join("app/models/user.rb"), "class User; end\n").unwrap();
        fs::write(
            root.path().join("spec/models/user_spec.rb"),
            "RSpec.describe User do; end\n",
        )
        .unwrap();

        for path in ["app/models/user.rb", "spec/models/user_spec.rb"] {
            let edges = extract_file_edges(root.path(), None, Path::new(path), "");
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Test
                    && edge.source_path == Path::new("app/models/user.rb")
                    && edge.target_path == Path::new("spec/models/user_spec.rb")
            }));
        }
    }

    #[test]
    fn test_file_edges_point_from_production_to_test() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::write(
            root.path().join("src/auth.rs"),
            "pub fn authenticate() {}\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("tests/auth_test.rs"),
            "#[test]\nfn authenticates() {}\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test
                && edge.source_path == Path::new("src/auth.rs")
                && edge.target_path == Path::new("tests/auth_test.rs")
        }));

        fs::write(root.path().join("src/session.py"), "def login(): pass\n").unwrap();
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("tests/test_session.py"),
            "def test_login(): pass\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test
                && edge.source_path == Path::new("src/session.py")
                && edge.target_path == Path::new("tests/test_session.py")
        }));

        fs::create_dir_all(root.path().join("src/foo")).unwrap();
        fs::create_dir_all(root.path().join("tests/foo")).unwrap();
        fs::write(root.path().join("src/foo/user.py"), "def user(): pass\n").unwrap();
        fs::write(
            root.path().join("tests/foo/test_user.py"),
            "def test_user(): pass\n",
        )
        .unwrap();
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/foo/user.py"),
            "def user(): pass\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Test
                && edge.source_path == Path::new("src/foo/user.py")
                && edge.target_path == Path::new("tests/foo/test_user.py")
        }));
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("tests/foo/test_user.py"),
            "def test_user(): pass\n",
        );
        assert!(
            edges
                .iter()
                .any(|edge| edge.source_path == Path::new("src/foo/user.py"))
        );

        fs::create_dir_all(root.path().join("src/__tests__")).unwrap();
        fs::write(
            root.path().join("src/widget.ts"),
            "export const widget = 1;\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/__tests__/widget.ts"),
            "test('widget', () => {});\n",
        )
        .unwrap();
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/__tests__/widget.ts"),
            "test('widget', () => {});\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.source_path == Path::new("src/widget.ts")
                && edge.target_path == Path::new("src/__tests__/widget.ts")
        }));
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/widget.ts"),
            "export const widget = 1;\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.source_path == Path::new("src/widget.ts")
                && edge.target_path == Path::new("src/__tests__/widget.ts")
        }));
    }

    #[test]
    fn single_quoted_imports_resolve() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("lib")).unwrap();
        fs::write(root.path().join("lib/helper.rb"), "def helper; end\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("lib/service.rb"),
            "require_relative 'helper'\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("lib/helper.rb")
        }));
    }

    #[test]
    fn typescript_runtime_specifiers_prefer_exact_then_source_extensions() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/helper.js"),
            "export const value = 1;\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/helper.ts"),
            "export const value = 2;\n",
        )
        .unwrap();

        let dependency = |root: &Path| {
            extract_file_edges(
                root,
                None,
                Path::new("src/main.ts"),
                "import { value } from './helper.js';\n",
            )
            .into_iter()
            .find(|edge| edge.kind == FileEdgeKind::Dependency)
            .unwrap()
            .target_path
        };
        assert_eq!(dependency(root.path()), Path::new("src/helper.js"));

        fs::remove_file(root.path().join("src/helper.js")).unwrap();
        assert_eq!(dependency(root.path()), Path::new("src/helper.ts"));
    }

    #[test]
    fn javascript_package_imports_do_not_bind_to_local_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/react.ts"),
            "export const React = {};\n",
        )
        .unwrap();

        let graph = extract_file_graph(
            root.path(),
            None,
            Path::new("src/main.ts"),
            "import React from 'react';\n",
        );
        assert!(!graph.edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/react.ts")
        }));
        assert!(
            graph
                .unresolved_dependencies
                .iter()
                .all(|dependency| dependency.spec != "react")
        );
    }

    #[test]
    fn typescript_import_attributes_keep_module_specifier() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/schema.json"), "{}\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main.ts"),
            "import schema from './schema.json' with { type: \"json\" };\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/schema.json")
        }));
    }

    #[test]
    fn typescript_multiline_static_imports_resolve() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/helper.ts"),
            "export const value = 1;\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main.ts"),
            "import {\n  value,\n} from './helper.js';\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/helper.ts")
        }));
    }

    #[test]
    fn typescript_type_star_reexports_resolve() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/release-types.ts"),
            "export type Release = string;\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/index.ts"),
            "export type * from './release-types.js';\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/release-types.ts")
        }));
    }

    #[test]
    fn dart_package_imports_resolve_within_nearest_package() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("packages/app/lib/src")).unwrap();
        fs::create_dir_all(root.path().join("lib/src")).unwrap();
        fs::write(
            root.path().join("packages/app/pubspec.yaml"),
            "name: my_app\n",
        )
        .unwrap();
        fs::write(
            root.path().join("packages/app/lib/src/auth.dart"),
            "bool authenticate() => true;\n",
        )
        .unwrap();
        fs::write(
            root.path().join("lib/src/auth.dart"),
            "bool wrongAuthenticate() => false;\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("packages/app/lib/main.dart"),
            "import 'package:my_app/src/auth.dart';\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("packages/app/lib/src/auth.dart")
        }));
        assert!(!edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("lib/src/auth.dart")
        }));
    }

    #[test]
    fn python_relative_imports_resolve_from_source_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("app/auth")).unwrap();
        fs::write(root.path().join("app/auth/token.py"), "def token(): pass\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("app/auth/service.py"),
            "from .token import token\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("app/auth/token.py")
        }));
    }

    #[test]
    fn python_relative_import_members_resolve_from_source_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("app/auth")).unwrap();
        fs::write(root.path().join("app/auth/helper.py"), "def work(): pass\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("app/auth/service.py"),
            "from . import helper\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("app/auth/helper.py")
        }));
    }

    #[test]
    fn python_absolute_import_members_resolve_submodules() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("app")).unwrap();
        fs::write(root.path().join("app/__init__.py"), "").unwrap();
        fs::write(root.path().join("app/helper.py"), "def work(): pass\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("service.py"),
            "from app import (\n    helper,\n)\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("app/helper.py")
        }));
    }

    #[test]
    fn starlark_labels_resolve_local_and_workspace_loads() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("tools")).unwrap();
        fs::write(root.path().join("defs.bzl"), "def root_rule(): pass\n").unwrap();
        fs::write(
            root.path().join("tools/defs.bzl"),
            "def rule_impl(): pass\n",
        )
        .unwrap();

        for spec in [":defs.bzl", "//tools:defs.bzl"] {
            let content = format!(
                "load(\n    # \":old_defs.bzl\",\n    \"{spec}\",\n    \"rule_impl\",\n)\n"
            );
            let edges = extract_file_edges(root.path(), None, Path::new("tools/BUILD"), &content);
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Dependency
                    && edge.target_path == Path::new("tools/defs.bzl")
            }));
        }

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("BUILD.bazel"),
            "load(\n    \"//:defs.bzl\",\n    \"root_rule\",\n)\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("defs.bzl")
        }));
    }

    #[test]
    fn jvm_imports_resolve_from_deep_source_packages() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/java/com/acme/project/util")).unwrap();
        fs::create_dir_all(root.path().join("src/main/kotlin/com/acme/project/module")).unwrap();
        fs::write(
            root.path()
                .join("src/main/java/com/acme/project/util/Helper.java"),
            "package com.acme.project.util; public class Helper {}\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/kotlin/com/acme/project/module/Service.kt"),
            "import com.acme.project.util.Helper\nclass Service(val helper: Helper)\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/main/java/com/acme/project/util/Helper.java")
        }));
    }

    #[test]
    fn java_static_imports_resolve_owning_class() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/java/com/acme/util")).unwrap();
        fs::create_dir_all(root.path().join("src/main/java/com/acme/service")).unwrap();
        fs::write(
            root.path().join("src/main/java/com/acme/util/Auth.java"),
            "package com.acme.util; public class Auth { public static void check() {} }\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/java/com/acme/service/Service.java"),
            "import static com.acme.util.Auth.check; // access check\nclass Service { void run() { check(); } }\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/main/java/com/acme/util/Auth.java")
        }));
    }

    #[test]
    fn csharp_static_using_resolves_owning_class() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/Acme/Util")).unwrap();
        fs::create_dir_all(root.path().join("src/Acme/Service")).unwrap();
        fs::write(
            root.path().join("src/Acme/Util/Auth.cs"),
            "namespace Acme.Util; public static class Auth { public static bool Check() => true; }\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/Acme/Service/Service.cs"),
            "global using static global::Acme.Util.Auth; // access check\nclass Service { bool Run() => Check(); }\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/Acme/Util/Auth.cs")
        }));
    }

    #[test]
    fn import_aliases_resolve_owning_files() {
        let root = tempfile::tempdir().unwrap();
        for (path, content) in [
            ("src/alias_clock.rs", "pub fn now() {}\n"),
            (
                "src/main/kotlin/com/acme/util/Auth.kt",
                "package com.acme.util\nclass Auth\n",
            ),
            (
                "src/main/groovy/com/acme/util/Auth.groovy",
                "package com.acme.util\nclass Auth {}\n",
            ),
            (
                "src/Acme/Util/Auth.cs",
                "namespace Acme.Util; class Auth {}\n",
            ),
            ("app/Acme/Util/Auth.php", "<?php namespace Acme\\Util;\n"),
        ] {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        for (language_path, import, target) in [
            (
                "src/service.rs",
                "use crate::alias_clock as clock;\n",
                "src/alias_clock.rs",
            ),
            (
                "src/main/kotlin/com/acme/service/Service.kt",
                "import com.acme.util.Auth as AliasAuth\n",
                "src/main/kotlin/com/acme/util/Auth.kt",
            ),
            (
                "src/main/groovy/com/acme/service/Service.groovy",
                "import com.acme.util.Auth as AliasAuth\n",
                "src/main/groovy/com/acme/util/Auth.groovy",
            ),
            (
                "src/Acme/Service/Service.cs",
                "global using AliasAuth = global::Acme.Util.Auth;\n",
                "src/Acme/Util/Auth.cs",
            ),
            (
                "app/Service.php",
                "<?php\nuse Acme\\Util\\Auth as AliasAuth;\n",
                "app/Acme/Util/Auth.php",
            ),
        ] {
            let edges = extract_file_edges(root.path(), None, Path::new(language_path), import);
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new(target)
            }));
        }
    }

    #[test]
    fn groovy_imports_resolve_package_classes_and_static_owners() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/groovy/com/acme/util")).unwrap();
        fs::create_dir_all(root.path().join("src/main/groovy/com/acme/service")).unwrap();
        fs::create_dir_all(root.path().join("src/main/java/com/acme/util")).unwrap();
        fs::write(
            root.path()
                .join("src/main/groovy/com/acme/util/Helper.groovy"),
            "package com.acme.util\nclass Helper {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("src/main/java/com/acme/util/Helper.java"),
            "package com.acme.util; class Helper {}\n",
        )
        .unwrap();
        fs::write(
            root.path()
                .join("src/main/groovy/com/acme/util/Auth.groovy"),
            "package com.acme.util\nclass Auth { static void check() {} }\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/groovy/com/acme/service/Service.groovy"),
            "import com.acme.util.Helper\nimport static com.acme.util.Auth.check\nclass Service {}\n",
        );
        for target in ["Helper.groovy", "Auth.groovy"] {
            let expected = Path::new("src/main/groovy/com/acme/util").join(target);
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Dependency && edge.target_path == expected
            }));
        }
    }

    #[test]
    fn scala_wildcard_imports_resolve_owning_object() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/scala/com/acme/util")).unwrap();
        fs::create_dir_all(root.path().join("src/main/scala/com/acme/service")).unwrap();
        fs::write(
            root.path()
                .join("src/main/scala/com/acme/util/Helpers.scala"),
            "package com.acme.util\nobject Helpers\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/scala/com/acme/service/Service.scala"),
            "import com.acme.util.Helpers._ // utilities\nclass Service\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/main/scala/com/acme/util/Helpers.scala")
        }));
    }

    #[test]
    fn kotlin_member_imports_resolve_owner_file() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/kotlin/com/acme/util")).unwrap();
        fs::write(
            root.path().join("src/main/kotlin/com/acme/util/Auth.kt"),
            "package com.acme.util\nobject Auth { fun check() = true }\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/kotlin/com/acme/service/Service.kt"),
            "import com.acme.util.Auth.check\nclass Service\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/main/kotlin/com/acme/util/Auth.kt")
        }));
    }

    #[test]
    fn scala_grouped_imports_resolve_independently() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/scala/com/acme/util")).unwrap();
        for owner in ["Auth", "Clock"] {
            fs::write(
                root.path()
                    .join(format!("src/main/scala/com/acme/util/{owner}.scala")),
                format!("package com.acme.util\nobject {owner}\n"),
            )
            .unwrap();
        }

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/main/scala/com/acme/service/Service.scala"),
            "import com.acme.util.{\n  Auth => AliasAuth,\n  Clock,\n}\nclass Service\n",
        );
        for owner in ["Auth", "Clock"] {
            let expected = PathBuf::from(format!("src/main/scala/com/acme/util/{owner}.scala"));
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Dependency && edge.target_path == expected
            }));
        }
    }

    #[test]
    fn rust_visible_module_declarations_resolve() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/public.rs"), "pub fn run() {}\n").unwrap();
        fs::write(root.path().join("src/scoped.rs"), "pub fn run() {}\n").unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/lib.rs"),
            "pub mod public;\npub(crate) mod scoped;\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/public.rs")
        }));
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/scoped.rs")
        }));
    }

    #[test]
    fn rust_file_modules_resolve_children_without_root_fallback() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/auth")).unwrap();
        fs::write(root.path().join("src/auth/token.rs"), "pub fn issue() {}\n").unwrap();
        fs::write(root.path().join("src/token.rs"), "pub fn wrong() {}\n").unwrap();

        let edges = extract_file_edges(root.path(), None, Path::new("src/auth.rs"), "mod token;\n");
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("src/auth/token.rs")
        }));
        assert!(!edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("src/token.rs")
        }));
    }

    #[test]
    fn rust_crate_root_modules_resolve_siblings() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("tests/integration")).unwrap();
        fs::write(root.path().join("tests/helper.rs"), "pub fn run() {}\n").unwrap();
        fs::write(
            root.path().join("tests/integration/helper.rs"),
            "pub fn wrong() {}\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("tests/integration.rs"),
            "mod helper;\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("tests/helper.rs")
        }));
        assert!(!edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("tests/integration/helper.rs")
        }));
    }

    #[test]
    fn go_module_imports_resolve_arbitrary_package_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("internal/auth")).unwrap();
        fs::write(
            root.path().join("internal/auth/aaa_test.go"),
            "package auth\n",
        )
        .unwrap();
        fs::write(
            root.path().join("internal/auth/client.go"),
            "package auth\n",
        )
        .unwrap();
        fs::write(
            root.path().join("internal/auth/session.go"),
            "package auth\n",
        )
        .unwrap();

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("cmd/server/main.go"),
            "package main\nimport \"github.com/acme/project/internal/auth\"\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("internal/auth/client.go")
        }));
    }

    #[test]
    fn go_import_blocks_ignore_commented_dependencies() {
        let root = tempfile::tempdir().unwrap();
        for package in ["active", "old"] {
            fs::create_dir_all(root.path().join(format!("internal/{package}"))).unwrap();
            fs::write(
                root.path().join(format!("internal/{package}/client.go")),
                format!("package {package}\n"),
            )
            .unwrap();
        }

        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("cmd/server/main.go"),
            "package main\nimport (\n  \"example.com/app/internal/active\"\n  // \"example.com/app/internal/old\"\n)\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("internal/active/client.go")
        }));
        assert!(!edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency
                && edge.target_path == Path::new("internal/old/client.go")
        }));
    }

    #[test]
    fn cochange_edges_use_recent_commits() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("tests")).unwrap();
        fs::write(root.path().join("src/auth.rs"), "pub fn auth() {}\n").unwrap();
        fs::write(root.path().join("tests/auth.rs"), "#[test]\nfn auth() {}\n").unwrap();
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["add", "."])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args([
                    "-c",
                    "user.name=ivygrep test",
                    "-c",
                    "user.email=ivygrep@example.invalid",
                    "commit",
                    "-q",
                    "-m",
                    "fixture",
                ])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );

        let workspace = Workspace::resolve(root.path()).unwrap();
        let edges = recent_cochange_edges(&workspace, &[PathBuf::from("src/auth.rs")]);
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::CoChange
                && edge.source_path == Path::new("src/auth.rs")
                && edge.target_path == Path::new("tests/auth.rs")
                && edge.cochange_count == 1
        }));
    }

    #[test]
    fn page_rank_prefers_stronger_graph_edges() {
        let seeds = BTreeSet::from(["src/lib.rs".to_string()]);
        let edges = vec![
            RankedEdge {
                source_path: PathBuf::from("src/lib.rs"),
                target_path: PathBuf::from("src/dependency.rs"),
                kind: FileEdgeKind::Dependency,
                cochange_count: 0,
            },
            RankedEdge {
                source_path: PathBuf::from("src/lib.rs"),
                target_path: PathBuf::from("docs/note.md"),
                kind: FileEdgeKind::Documentation,
                cochange_count: 0,
            },
        ];
        let ranks = personalized_page_rank(&edges, &seeds);
        assert!(ranks["src/dependency.rs"] > ranks["docs/note.md"]);
    }

    #[test]
    fn graph_reasons_follow_edge_direction() {
        let expansion = |kind, outgoing| GraphExpansion {
            file_path: PathBuf::from("neighbor"),
            seed_path: PathBuf::from("seed"),
            kind,
            outgoing,
            score: 1.0,
            cochange_count: 0,
        };

        assert_eq!(
            expansion(FileEdgeKind::Config, true).reason(),
            "neighbor configures seed"
        );
        assert_eq!(
            expansion(FileEdgeKind::Config, false).reason(),
            "seed configures neighbor"
        );
        assert_eq!(
            expansion(FileEdgeKind::Documentation, true).reason(),
            "seed documents neighbor"
        );
        assert_eq!(
            expansion(FileEdgeKind::Documentation, false).reason(),
            "neighbor documents seed"
        );
    }
}
