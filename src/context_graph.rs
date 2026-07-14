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

    if supports_dependency_scan(language) {
        for spec in dependency_specs(language, content) {
            if let Some(target_path) =
                resolve_local_dependency(root, snapshot, rel_path, language, &spec)
            {
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
        && let Some(manifest) = nearest_manifest(root, snapshot, rel_path)
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
        for target in markdown_targets(root, snapshot, rel_path, content) {
            insert_edge(&mut edges, rel_path, &target, FileEdgeKind::Documentation);
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
    resolve_local_dependency(root, snapshot, source_path, language, spec)
}

pub(crate) fn dependency_lookup_keys(value: &str) -> BTreeSet<String> {
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
        .and_then(|_| nearest_manifest(root, snapshot, rel_path))
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
    for raw_line in content.lines().take(20_000) {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") && language != "go" {
            continue;
        }
        match language {
            "rust" => {
                let line = strip_rust_visibility(line);
                let value = line
                    .strip_prefix("use ")
                    .or_else(|| line.strip_prefix("mod "));
                if let Some(value) = value {
                    specs.extend(expand_grouped_spec(value.trim_end_matches(';')));
                }
            }
            "python" => {
                if let Some(value) = line.strip_prefix("from ") {
                    if let Some((module, members)) = value.split_once(" import ") {
                        let module = module.trim();
                        let members = python_import_members(members);
                        if !module.is_empty() && module.chars().all(|character| character == '.') {
                            for member in members {
                                specs.insert(format!("{module}{member}"));
                            }
                        } else {
                            specs.insert(module.to_string());
                            for member in members {
                                specs.insert(format!("{module}.{member}"));
                            }
                        }
                    }
                } else if let Some(value) = line.strip_prefix("import ") {
                    for part in value.split(',') {
                        specs.insert(part.split_whitespace().next().unwrap_or("").to_string());
                    }
                }
            }
            "javascript" | "typescript" => {
                if (line.starts_with("import ") || line.starts_with("export "))
                    && let Some(spec) = last_quoted_value(line)
                {
                    specs.insert(spec);
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
                if let Some(value) = line
                    .strip_prefix("import ")
                    .or_else(|| line.strip_prefix("using "))
                {
                    specs.insert(
                        value
                            .trim_end_matches(';')
                            .trim_end_matches(".*")
                            .to_string(),
                    );
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
                    specs.insert(value.trim_end_matches(';').replace('\\', "/"));
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
                if let Some(value) = value_after_marker(line, "load(") {
                    specs.insert(value);
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

fn expand_grouped_spec(value: &str) -> Vec<String> {
    let value = value.trim();
    let Some((prefix, rest)) = value.split_once('{') else {
        return vec![value.to_string()];
    };
    let Some(group) = rest.split('}').next() else {
        return vec![prefix.trim_end_matches("::").to_string()];
    };
    let prefix = prefix.trim_end_matches("::");
    group
        .split(',')
        .filter_map(|member| {
            let member = member.split_whitespace().next()?;
            (!member.is_empty() && member != "self").then(|| format!("{prefix}::{member}"))
        })
        .collect()
}

fn first_quoted_value(value: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let Some(start) = value.find(quote) else {
            continue;
        };
        let rest = &value[start + quote.len_utf8()..];
        if let Some(end) = rest.find(quote) {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn last_quoted_value(value: &str) -> Option<String> {
    let mut result = None;
    for (offset, character) in value.char_indices() {
        if character != '"' && character != '\'' {
            continue;
        }
        let rest = &value[offset + 1..];
        if let Some(end) = rest.find(character) {
            result = Some(rest[..end].to_string());
        }
    }
    result
}

fn value_after_marker(value: &str, marker: &str) -> Option<String> {
    value
        .find(marker)
        .and_then(|offset| first_quoted_value(&value[offset + marker.len()..]))
}

fn resolve_local_dependency(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    source_path: &Path,
    language: &str,
    spec: &str,
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
        || spec.starts_with("package:")
    {
        return None;
    }

    let source_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let mut normalized = spec.replace("::", "/").replace('\\', "/");
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
        normalized = spec.trim_start_matches('/').replace(':', "/");
    } else if matches!(
        language,
        "python" | "java" | "kotlin" | "scala" | "csharp" | "haskell" | "elixir"
    ) {
        normalized = normalized.replace('.', "/");
    }
    let crate_relative = normalized.starts_with("crate/");
    let source_relative = normalized.starts_with("self/")
        || normalized.starts_with("super/")
        || normalized.starts_with("./")
        || normalized.starts_with("../")
        || python_relative
        || starlark_relative;
    if language == "rust" && !crate_relative && !source_relative && normalized.contains('/') {
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
    if source_relative
        || matches!(language, "c" | "cpp" | "objc" | "ruby" | "shell")
        || language == "rust" && !crate_relative
    {
        bases.push(source_dir.to_path_buf());
    }
    bases.extend([
        PathBuf::new(),
        PathBuf::from("src"),
        PathBuf::from("lib"),
        PathBuf::from("app"),
    ]);
    for ancestor in source_dir.ancestors().take(4) {
        bases.push(ancestor.to_path_buf());
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
        "java" => &["java"],
        "kotlin" => &["kt", "kts"],
        "scala" => &["scala"],
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
) -> Option<PathBuf> {
    let parent = rel_path.parent().unwrap_or_else(|| Path::new(""));
    for directory in parent.ancestors() {
        for manifest in MANIFEST_NAMES {
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
        ] {
            candidates.insert(base.join(format!("{production_stem}.{extension}")));
        }
    } else {
        for suffix in ["_test", "_tests", ".test", ".spec"] {
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
            .unwrap_or(rel_path);
        candidates.insert(PathBuf::from("tests").join(relative_under_source));
        candidates.insert(PathBuf::from("test").join(relative_under_source));
        let mirrored_parent = relative_under_source
            .parent()
            .unwrap_or_else(|| Path::new(""));
        for directory in ["tests", "test"] {
            let mirrored_base = PathBuf::from(directory).join(mirrored_parent);
            for suffix in ["_test", "_tests", ".test", ".spec"] {
                candidates.insert(mirrored_base.join(format!("{stem}{suffix}.{extension}")));
            }
            candidates.insert(mirrored_base.join(format!("test_{stem}.{extension}")));
        }
        if let Some(file_name) = rel_path.file_name() {
            candidates.insert(PathBuf::from("tests").join(file_name));
            candidates.insert(PathBuf::from("test").join(file_name));
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
                if value != "test" && value != "tests" && value != "__tests__" =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect()
}

fn path_looks_like_test(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(value) if value == "test" || value == "tests" || value == "__tests__")
    }) || path
        .file_stem()
        .and_then(|value| value.to_str())
        .is_some_and(|stem| {
            stem.ends_with("_test")
                || stem.ends_with("_tests")
                || stem.ends_with(".test")
                || stem.ends_with(".spec")
                || stem.starts_with("test_")
        })
}

fn markdown_targets(
    root: &Path,
    snapshot: Option<&MerkleSnapshot>,
    rel_path: &Path,
    content: &str,
) -> Vec<PathBuf> {
    let mut targets = BTreeSet::new();
    for line in content.lines().take(20_000) {
        let mut remaining = line;
        while let Some(open) = remaining.find("](") {
            remaining = &remaining[open + 2..];
            let Some(close) = remaining.find(')') else {
                break;
            };
            let target = remaining[..close].split_whitespace().next().unwrap_or("");
            if !target.starts_with('#')
                && let Some(path) =
                    resolve_local_dependency(root, snapshot, rel_path, "markdown", target)
            {
                targets.insert(path);
            }
            remaining = &remaining[close + 1..];
        }
    }
    targets.into_iter().take(16).collect()
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
    fn grouped_rust_imports_resolve_independently() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/auth")).unwrap();
        fs::write(root.path().join("src/auth/token.rs"), "pub struct Token;\n").unwrap();
        fs::write(
            root.path().join("src/auth/session.rs"),
            "pub struct Session;\n",
        )
        .unwrap();
        let edges = extract_file_edges(
            root.path(),
            None,
            Path::new("src/lib.rs"),
            "use crate::auth::{token, session};\n",
        );
        let targets = edges
            .iter()
            .filter(|edge| edge.kind == FileEdgeKind::Dependency)
            .map(|edge| edge.target_path.clone())
            .collect::<BTreeSet<_>>();
        assert!(targets.contains(Path::new("src/auth/token.rs")));
        assert!(targets.contains(Path::new("src/auth/session.rs")));
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
            "from app import helper\n",
        );
        assert!(edges.iter().any(|edge| {
            edge.kind == FileEdgeKind::Dependency && edge.target_path == Path::new("app/helper.py")
        }));
    }

    #[test]
    fn starlark_labels_resolve_local_and_workspace_loads() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("tools")).unwrap();
        fs::write(
            root.path().join("tools/defs.bzl"),
            "def rule_impl(): pass\n",
        )
        .unwrap();

        for spec in [":defs.bzl", "//tools:defs.bzl"] {
            let content = format!("load(\"{spec}\", \"rule_impl\")\n");
            let edges = extract_file_edges(root.path(), None, Path::new("tools/BUILD"), &content);
            assert!(edges.iter().any(|edge| {
                edge.kind == FileEdgeKind::Dependency
                    && edge.target_path == Path::new("tools/defs.bzl")
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
