use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
#[cfg(test)]
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Serialize;

use crate::context_graph::{
    FileEdgeKind, GraphExpansion, expand_context_graph, expand_context_tests, extract_file_graph,
};
use crate::context_input::{
    ContextChangeScope, ContextInputPath, ContextSeed, collect_context_input, path_is_git_ignored,
};
use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
use crate::indexer::reconcile_worktree_overlay;
use crate::path_glob::PathGlobMatcher;
use crate::protocol::SearchHit;
use crate::search::{SearchContext, SearchOptions, hybrid_search_with_context};
use crate::symbols::{
    SymbolSearchMode, likely_definition_names, search_symbol_relationships_in_current_index,
    search_symbols_in_current_index,
};
use crate::workspace::Workspace;

const MAX_ITEMS: usize = 20;
const TARGET_CONTEXT_ITEMS: usize = 14;
const MAX_ANCHOR_SYMBOLS: usize = 3;
const RRF_K: f64 = 10.0;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRole {
    Primary,
    Definition,
    Dependency,
    Dependent,
    Caller,
    Reference,
    Test,
    Config,
    Documentation,
    Related,
}

impl ContextRole {
    fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Definition => "definition",
            Self::Dependency => "dependency",
            Self::Dependent => "dependent",
            Self::Caller => "caller",
            Self::Reference => "reference",
            Self::Test => "test",
            Self::Config => "config",
            Self::Documentation => "documentation",
            Self::Related => "related",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ContextCoverage {
    pub files: usize,
    pub primary: usize,
    pub definitions: usize,
    pub dependencies: usize,
    pub dependents: usize,
    pub callers: usize,
    pub references: usize,
    pub tests: usize,
    pub config: usize,
    pub documentation: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextItem {
    #[serde(serialize_with = "crate::context_input::serialize_index_path")]
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub roles: Vec<ContextRole>,
    pub reasons: Vec<String>,
    pub sources: Vec<String>,
    pub preview: String,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextBundle {
    pub task: String,
    pub workspace: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_scope: Option<ContextChangeScope>,
    pub referenced_paths: Vec<ContextInputPath>,
    pub budget_tokens: usize,
    pub used_tokens: usize,
    pub candidate_count: usize,
    pub truncated: bool,
    pub anchor_symbols: Vec<String>,
    pub coverage: ContextCoverage,
    pub items: Vec<ContextItem>,
}

#[derive(Debug, Clone, Default)]
pub struct ContextBuildOptions<'a> {
    pub since: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct Candidate {
    hit: SearchHit,
    roles: BTreeSet<ContextRole>,
    reasons: BTreeSet<String>,
    fused_score: f64,
}

pub fn build_context_bundle(
    workspace: &Workspace,
    task: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    base_options: &SearchOptions,
    budget_tokens: usize,
) -> Result<ContextBundle> {
    build_context_bundle_with_options(
        workspace,
        task,
        embedding_model,
        base_options,
        budget_tokens,
        &ContextBuildOptions::default(),
    )
}

pub fn build_context_bundle_with_options(
    workspace: &Workspace,
    task: &str,
    embedding_model: Option<&dyn EmbeddingModel>,
    base_options: &SearchOptions,
    budget_tokens: usize,
    context_options: &ContextBuildOptions<'_>,
) -> Result<ContextBundle> {
    let fallback_model;
    let reconciliation_model = if let Some(model) = embedding_model {
        model
    } else {
        fallback_model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        &fallback_model
    };
    reconcile_worktree_overlay(workspace, reconciliation_model)?;

    let input = collect_context_input(workspace, task, context_options.since, base_options)?;

    let wants_vectors = embedding_model.is_some();
    let wants_neural = embedding_model.is_some_and(|model| model.model_identity().is_some());
    let search_context = SearchContext::load(
        workspace,
        embedding_model
            .filter(|_| wants_vectors)
            .map(EmbeddingModel::dimensions),
        wants_neural,
    )?;
    let candidate_limit = (budget_tokens / 250).clamp(8, 24);
    let mut candidates = BTreeMap::<(PathBuf, usize, usize), Candidate>::new();
    let mut input_graph_seeds = Vec::new();
    let mut retrieval_graph_seed_paths = Vec::new();
    let mut primary_hits = Vec::new();
    let mut fallback_input_count = 0usize;

    for seed in input.seeds.iter().take(24) {
        let Some(hit) = context_seed_hit(&workspace.root, seed, task)? else {
            continue;
        };
        let explicit_match = task_symbols(task).iter().any(|symbol| {
            let symbol = symbol.to_ascii_lowercase();
            hit.file_path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&symbol)
                || hit.preview.to_ascii_lowercase().contains(&symbol)
        });
        let task_relevant =
            seed.source == "task_input" || explicit_match || hit_matches_task(&hit, task);
        if !task_relevant {
            if fallback_input_count == 4 {
                continue;
            }
            fallback_input_count += 1;
        }
        primary_hits.push(hit.clone());
        input_graph_seeds.push(seed.clone());
        add_candidate(
            &mut candidates,
            hit.clone(),
            if task_relevant {
                ContextRole::Primary
            } else {
                ContextRole::Related
            },
            seed.reason.clone(),
            if task_relevant {
                0.20 + f64::from(seed.priority) * 0.10
            } else {
                0.03
            },
        );
        if let Some(role) = classify_file_role(&hit) {
            add_candidate(
                &mut candidates,
                hit,
                role,
                format!("{} file in context input", role.label()),
                0.08,
            );
        }
    }

    let query_specs = [
        (
            task.to_string(),
            ContextRole::Primary,
            1.0,
            14usize,
            "primary",
        ),
        (format!("test {task}"), ContextRole::Test, 0.72, 8, "test"),
        (
            format!("configuration documentation example {task}"),
            ContextRole::Related,
            0.64,
            8,
            "supporting",
        ),
    ];

    for (query, requested_role, weight, context_lines, retrieval_label) in query_specs {
        let mut options = base_options.clone();
        options.limit = Some(candidate_limit);
        options.context = context_lines;
        let hits = hybrid_search_with_context(
            &search_context,
            workspace,
            &query,
            embedding_model,
            &options,
        )?;
        if requested_role == ContextRole::Primary {
            primary_hits.extend(hits.clone());
        }
        for (rank, hit) in hits.into_iter().enumerate() {
            if requested_role == ContextRole::Primary && rank > 0 && !hit_matches_task(&hit, task) {
                continue;
            }
            let role = classify_role(&hit, requested_role);
            if requested_role != ContextRole::Primary
                && (!hit_matches_task(&hit, task)
                    || if requested_role == ContextRole::Related {
                        !matches!(role, ContextRole::Config | ContextRole::Documentation)
                    } else {
                        role != requested_role
                    })
            {
                continue;
            }
            if requested_role == ContextRole::Primary {
                retrieval_graph_seed_paths.push(hit.file_path.clone());
            }
            add_candidate(
                &mut candidates,
                hit,
                role,
                format!(
                    "rank {} for {retrieval_label} retrieval",
                    rank.saturating_add(1)
                ),
                weight / (RRF_K + rank as f64 + 1.0),
            );
        }
    }

    let anchor_symbols = anchor_symbols(task, &primary_hits);
    let relationship_anchors = relationship_anchor_keys(task, &anchor_symbols);
    let mut symbol_options = base_options.clone();
    symbol_options.limit = Some(4);
    symbol_options.context = 10;
    for symbol in &anchor_symbols {
        match search_symbols_in_current_index(
            workspace,
            symbol,
            SymbolSearchMode::Definitions,
            &symbol_options,
        ) {
            Ok(hits) => {
                for (rank, hit) in hits.into_iter().enumerate() {
                    add_candidate(
                        &mut candidates,
                        focus_hit_on_symbol(hit, symbol, symbol_options.context, false),
                        ContextRole::Definition,
                        format!("defines {symbol}"),
                        0.82 / (RRF_K + rank as f64 + 1.0),
                    );
                }
            }
            Err(error) => tracing::debug!("context definition expansion failed: {error:#}"),
        }
        if !relationship_anchors.contains(&symbol.to_ascii_lowercase()) {
            continue;
        }
        match search_symbol_relationships_in_current_index(workspace, symbol, &symbol_options) {
            Ok((callers, references)) => {
                for (role, hits, weight, verb, prefer_last) in [
                    (ContextRole::Caller, callers, 0.76, "calls", true),
                    (
                        ContextRole::Reference,
                        references,
                        0.68,
                        "references",
                        false,
                    ),
                ] {
                    for (rank, hit) in hits.into_iter().enumerate() {
                        add_candidate(
                            &mut candidates,
                            focus_hit_on_symbol(hit, symbol, symbol_options.context, prefer_last),
                            role,
                            format!("{verb} {symbol}"),
                            weight / (RRF_K + rank as f64 + 1.0),
                        );
                    }
                }
            }
            Err(error) => tracing::debug!("context relationship expansion failed: {error:#}"),
        }
    }
    if !anchor_symbols.is_empty() {
        let mut options = base_options.clone();
        options.limit = Some(candidate_limit.min(12));
        options.context = 8;
        let query = format!("test {}", anchor_symbols.join(" "));
        match hybrid_search_with_context(
            &search_context,
            workspace,
            &query,
            embedding_model,
            &options,
        ) {
            Ok(hits) => {
                for (rank, hit) in hits.into_iter().enumerate() {
                    if classify_file_role(&hit) != Some(ContextRole::Test)
                        || !(hit_matches_task(&hit, task)
                            || task_path_overlap(&hit.file_path, task) > 0
                            || hit_matches_any_anchor(&hit, &anchor_symbols))
                    {
                        continue;
                    }
                    add_candidate(
                        &mut candidates,
                        hit,
                        ContextRole::Test,
                        format!("rank {} for anchor test retrieval", rank.saturating_add(1)),
                        0.78 / (RRF_K + rank as f64 + 1.0),
                    );
                }
            }
            Err(error) => tracing::debug!("context anchor test retrieval failed: {error:#}"),
        }
    }

    let mut transient_seen = HashSet::new();
    let transient_seeds = input_graph_seeds
        .iter()
        .filter(|seed| transient_seen.insert(seed.file_path.clone()))
        .take(12)
        .cloned()
        .collect::<Vec<_>>();
    add_transient_graph_candidates(
        workspace,
        task,
        &transient_seeds,
        &search_context,
        base_options,
        &mut candidates,
    );

    let mut seen_seed_paths = HashSet::new();
    let seed_paths = input_graph_seeds
        .into_iter()
        .map(|seed| seed.file_path)
        .take(8)
        .chain(retrieval_graph_seed_paths.into_iter().take(4))
        .filter(|path| seen_seed_paths.insert(path.clone()))
        .take(12)
        .collect::<Vec<_>>();
    match expand_context_graph(workspace, &seed_paths, base_options) {
        Ok(expansions) => {
            for (rank, expansion) in expansions.into_iter().enumerate() {
                match search_context.representative_hit_for_file(
                    &expansion.file_path,
                    task,
                    base_options.skip_gitignore,
                ) {
                    Ok(Some(mut hit)) => {
                        if expansion.kind == FileEdgeKind::Documentation
                            && !graph_support_matches_task(&hit, task, expansion.kind)
                        {
                            continue;
                        }
                        let role = graph_context_role(expansion.kind, expansion.outgoing);
                        hit.sources.push(expansion.kind.source_label().to_string());
                        add_candidate(
                            &mut candidates,
                            hit,
                            role,
                            expansion.reason(),
                            0.74 / (RRF_K + rank as f64 + 1.0),
                        );
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::debug!("context graph hydration failed: {error:#}")
                    }
                }
            }
        }
        Err(error) => tracing::debug!("context graph expansion failed: {error:#}"),
    }
    match expand_context_tests(workspace, &seed_paths, base_options) {
        Ok(expansions) => {
            let mut test_hits = Vec::new();
            for expansion in expansions {
                match search_context.representative_hit_for_file(
                    &expansion.file_path,
                    task,
                    base_options.skip_gitignore,
                ) {
                    Ok(Some(hit)) => {
                        let path_overlap = task_path_overlap(&hit.file_path, task);
                        if path_overlap > 0 || hit_matches_task(&hit, task) {
                            test_hits.push((path_overlap, expansion, hit));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::debug!("context test graph hydration failed: {error:#}")
                    }
                }
            }
            test_hits.sort_by(|left, right| {
                right
                    .0
                    .cmp(&left.0)
                    .then_with(|| right.1.score.total_cmp(&left.1.score))
                    .then_with(|| right.2.score.total_cmp(&left.2.score))
                    .then_with(|| left.2.file_path.cmp(&right.2.file_path))
            });
            for (rank, (path_overlap, expansion, mut hit)) in test_hits.into_iter().enumerate() {
                hit.sources
                    .push(FileEdgeKind::Test.source_label().to_string());
                add_candidate(
                    &mut candidates,
                    hit,
                    ContextRole::Test,
                    expansion.reason(),
                    0.82 / (RRF_K + rank as f64 + 1.0) + path_overlap as f64 * 0.02,
                );
            }
        }
        Err(error) => tracing::debug!("context test graph expansion failed: {error:#}"),
    }

    Ok(assemble_bundle(
        task,
        &workspace.root,
        budget_tokens,
        anchor_symbols,
        input.change_scope,
        input.referenced_paths,
        candidates.into_values().collect(),
    ))
}

fn context_seed_hit(root: &Path, seed: &ContextSeed, task: &str) -> Result<Option<SearchHit>> {
    let Some(content) = context_seed_content(root, seed)? else {
        return Ok(None);
    };
    let mut chunks = crate::chunking::chunk_source(&seed.file_path, &content);
    if chunks.is_empty() {
        return Ok(None);
    }
    let terms = significant_task_terms(task);
    chunks.sort_by(|left, right| {
        seed_chunk_score(right, seed.line, &terms)
            .cmp(&seed_chunk_score(left, seed.line, &terms))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    let chunk = chunks.remove(0);
    let path_header = format!("// {}\n\n", seed.file_path.display());
    let preview = chunk
        .text
        .strip_prefix(&path_header)
        .unwrap_or(&chunk.text)
        .to_string();
    Ok(Some(SearchHit {
        file_path: seed.file_path.clone(),
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        preview,
        reason: seed.reason.clone(),
        score: 0.0,
        sources: vec![seed.source.clone()],
        neural_requested: false,
        neural_executed: false,
    }))
}

fn context_seed_content(root: &Path, seed: &ContextSeed) -> Result<Option<String>> {
    const MAX_CONTEXT_FILE_BYTES: u64 = 4 * 1024 * 1024;
    if let Ok(file) = crate::workspace_file::open(root, &seed.file_path) {
        if file.metadata()?.len() > MAX_CONTEXT_FILE_BYTES {
            return Ok(None);
        }
        let mut content = String::new();
        return match file
            .take(MAX_CONTEXT_FILE_BYTES + 1)
            .read_to_string(&mut content)
        {
            Ok(bytes) if bytes as u64 <= MAX_CONTEXT_FILE_BYTES && !content.trim().is_empty() => {
                Ok(Some(content))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => Ok(None),
            Err(error) => Err(error.into()),
        };
    }
    let Some(revision) = seed.git_revision.as_deref() else {
        return Ok(None);
    };
    let Ok(mut command) = git_seed_command(root) else {
        return Ok(None);
    };
    let prefix = command.args(["rev-parse", "--show-prefix"]).output()?;
    if !prefix.status.success() {
        return Ok(None);
    }
    let repo_path = Path::new(String::from_utf8_lossy(&prefix.stdout).trim())
        .join(&seed.file_path)
        .to_string_lossy()
        .replace('\\', "/");
    let object = format!("{revision}:{repo_path}");
    let Ok(mut command) = git_seed_command(root) else {
        return Ok(None);
    };
    let size = command.args(["cat-file", "-s", &object]).output()?;
    if !size.status.success()
        || String::from_utf8_lossy(&size.stdout)
            .trim()
            .parse::<u64>()
            .map_or(true, |size| size > MAX_CONTEXT_FILE_BYTES)
    {
        return Ok(None);
    }
    let Ok(mut command) = git_seed_command(root) else {
        return Ok(None);
    };
    let output = command.args(["cat-file", "blob", &object]).output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let content = match String::from_utf8(output.stdout) {
        Ok(content) if !content.trim().is_empty() => content,
        _ => return Ok(None),
    };
    Ok(Some(content))
}

#[cfg(unix)]
fn git_seed_command(root: &Path) -> std::io::Result<Command> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    let directory = crate::workspace_file::open_root(root)?;
    let mut command = Command::new("git");
    // SAFETY: fchdir is async-signal-safe. The closure owns the directory until
    // exec, so the child never resolves a workspace pathname after validation.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(directory.as_raw_fd()) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    Ok(command)
}

#[cfg(not(unix))]
fn git_seed_command(_root: &Path) -> std::io::Result<Command> {
    // Live files and indexed evidence remain available. A pathname-based cwd
    // cannot safely supply a historical-only seed after root replacement.
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "contained Git-history reads are unavailable on this platform",
    ))
}

fn seed_chunk_score(
    chunk: &crate::chunking::Chunk,
    line: Option<usize>,
    terms: &[String],
) -> usize {
    let line_score = line.map_or(0, |line| {
        if (chunk.start_line..=chunk.end_line).contains(&line) {
            10_000
        } else {
            1_000usize.saturating_sub(chunk.start_line.abs_diff(line).min(1_000))
        }
    });
    let lower = chunk.text.to_ascii_lowercase();
    line_score
        + terms
            .iter()
            .filter(|term| lower.contains(term.as_str()))
            .map(String::len)
            .sum::<usize>()
}

fn add_transient_graph_candidates(
    workspace: &Workspace,
    task: &str,
    seeds: &[ContextSeed],
    search_context: &SearchContext,
    options: &SearchOptions,
    candidates: &mut BTreeMap<(PathBuf, usize, usize), Candidate>,
) {
    let Ok(path_matcher) = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)
    else {
        return;
    };
    for seed in seeds {
        let seed_path = &seed.file_path;
        let Ok(Some(content)) = context_seed_content(&workspace.root, seed) else {
            continue;
        };
        let edges = extract_file_graph(&workspace.root, None, seed_path, &content).edges;
        let mut ordered_edges = Vec::new();
        let mut claimed = HashSet::new();
        for kind in [
            FileEdgeKind::Test,
            FileEdgeKind::Config,
            FileEdgeKind::Dependency,
            FileEdgeKind::Documentation,
        ] {
            if let Some((index, edge)) = edges
                .iter()
                .enumerate()
                .find(|(index, edge)| !claimed.contains(index) && edge.kind == kind)
            {
                claimed.insert(index);
                ordered_edges.push(edge.clone());
            }
        }
        ordered_edges.extend(
            edges
                .into_iter()
                .enumerate()
                .filter_map(|(index, edge)| (!claimed.contains(&index)).then_some(edge)),
        );
        for edge in ordered_edges.into_iter().take(12) {
            let outgoing = edge.source_path == *seed_path;
            let file_path = if outgoing {
                edge.target_path.clone()
            } else {
                edge.source_path.clone()
            };
            if !context_path_allowed(&file_path, options, &path_matcher) {
                continue;
            }
            let expansion = GraphExpansion {
                file_path: file_path.clone(),
                seed_path: seed_path.clone(),
                kind: edge.kind,
                outgoing,
                score: 0.0,
                cochange_count: 0,
            };
            let hit = search_context
                .representative_hit_for_file(&file_path, task, options.skip_gitignore)
                .ok()
                .flatten()
                .or_else(|| {
                    if !options.skip_gitignore && path_is_git_ignored(&workspace.root, &file_path) {
                        return None;
                    }
                    context_seed_hit(
                        &workspace.root,
                        &ContextSeed {
                            file_path: file_path.clone(),
                            line: None,
                            git_revision: None,
                            reason: expansion.reason(),
                            source: edge.kind.source_label().to_string(),
                            priority: 0,
                        },
                        task,
                    )
                    .ok()
                    .flatten()
                });
            let Some(mut hit) = hit else {
                continue;
            };
            hit.sources.push(edge.kind.source_label().to_string());
            add_candidate(
                candidates,
                hit,
                graph_context_role(edge.kind, outgoing),
                expansion.reason(),
                0.12,
            );
        }
    }
}

fn context_path_allowed(
    path: &Path,
    options: &SearchOptions,
    path_matcher: &PathGlobMatcher,
) -> bool {
    path_matcher.matches(path)
        && options
            .scope_filter
            .as_ref()
            .is_none_or(|scope| scope.matches(path))
        && options.type_filter.as_deref().is_none_or(|filter| {
            let expected = crate::chunking::resolve_type_alias(filter).unwrap_or(filter);
            crate::chunking::language_for_path(path) == Some(expected)
        })
}

fn graph_context_role(kind: FileEdgeKind, outgoing: bool) -> ContextRole {
    match (kind, outgoing) {
        (FileEdgeKind::Dependency, true) => ContextRole::Dependency,
        (FileEdgeKind::Dependency, false) => ContextRole::Dependent,
        (FileEdgeKind::Test, true) => ContextRole::Test,
        (FileEdgeKind::Test, false) => ContextRole::Dependency,
        (FileEdgeKind::Config, true) => ContextRole::Config,
        (FileEdgeKind::Config, false) => ContextRole::Related,
        (FileEdgeKind::Documentation, false) => ContextRole::Documentation,
        (FileEdgeKind::Documentation, true) => ContextRole::Related,
        (FileEdgeKind::CoChange, _) => ContextRole::Related,
    }
}

fn focus_hit_on_symbol(
    mut hit: SearchHit,
    symbol: &str,
    context_lines: usize,
    prefer_last: bool,
) -> SearchHit {
    strip_path_header(&mut hit);
    let lines = hit.preview.lines().collect::<Vec<_>>();
    let symbol = symbol.to_ascii_lowercase();
    let matches = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.to_ascii_lowercase().contains(&symbol).then_some(index))
        .collect::<Vec<_>>();
    let Some(focus) = (if prefer_last {
        matches.last()
    } else {
        matches.first()
    })
    .copied() else {
        return hit;
    };
    let start = focus.saturating_sub(context_lines);
    let end = focus
        .saturating_add(context_lines)
        .saturating_add(1)
        .min(lines.len());
    hit.start_line = hit.start_line.saturating_add(start);
    hit.end_line = hit
        .start_line
        .saturating_add(end.saturating_sub(start.saturating_add(1)));
    hit.preview = lines[start..end].join("\n");
    hit
}

fn strip_path_header(hit: &mut SearchHit) {
    let path_header = format!("// {}\n\n", hit.file_path.display());
    if let Some(preview) = hit.preview.strip_prefix(&path_header) {
        hit.preview = preview.to_string();
    }
}

fn add_candidate(
    candidates: &mut BTreeMap<(PathBuf, usize, usize), Candidate>,
    hit: SearchHit,
    role: ContextRole,
    reason: String,
    fused_score: f64,
) {
    let key = candidates
        .keys()
        .find(|(path, start, end)| {
            *path == hit.file_path
                && ranges_substantially_overlap(*start, *end, hit.start_line, hit.end_line)
        })
        .cloned()
        .unwrap_or_else(|| (hit.file_path.clone(), hit.start_line, hit.end_line));
    let candidate = candidates.entry(key).or_insert_with(|| Candidate {
        hit: hit.clone(),
        roles: BTreeSet::new(),
        reasons: BTreeSet::new(),
        fused_score: 0.0,
    });
    candidate.hit.sources.extend(hit.sources);
    candidate.hit.sources.sort();
    candidate.hit.sources.dedup();
    candidate.roles.insert(role);
    candidate.reasons.insert(reason);
    candidate.fused_score += fused_score;
}

fn ranges_substantially_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    let overlap_start = left_start.max(right_start);
    let overlap_end = left_end.min(right_end);
    if overlap_end < overlap_start {
        return false;
    }
    let overlap = overlap_end.saturating_sub(overlap_start).saturating_add(1);
    let shorter = left_end
        .saturating_sub(left_start)
        .saturating_add(1)
        .min(right_end.saturating_sub(right_start).saturating_add(1));
    overlap.saturating_mul(2) >= shorter
}

fn assemble_bundle(
    task: &str,
    workspace: &Path,
    budget_tokens: usize,
    anchor_symbols: Vec<String>,
    change_scope: Option<ContextChangeScope>,
    mut referenced_paths: Vec<ContextInputPath>,
    mut candidates: Vec<Candidate>,
) -> ContextBundle {
    let change_scope = change_scope.map(|scope| bound_change_scope(scope, budget_tokens));
    let referenced_path_limit = (budget_tokens / 64).clamp(1, 32);
    let referenced_paths_truncated = referenced_paths.len() > referenced_path_limit;
    referenced_paths.truncate(referenced_path_limit);
    let fixed_header_tokens = estimated_header_tokens(
        "",
        workspace,
        budget_tokens,
        &anchor_symbols,
        candidates.len(),
        change_scope.as_ref(),
        &referenced_paths,
    );
    let task_budget = budget_tokens.saturating_sub(fixed_header_tokens).min(1_024);
    let (output_task, task_truncated, _) = truncate_to_token_budget(task, task_budget, task);
    candidates.sort_by(|left, right| {
        right
            .fused_score
            .total_cmp(&left.fused_score)
            .then_with(|| left.hit.file_path.cmp(&right.hit.file_path))
            .then_with(|| left.hit.start_line.cmp(&right.hit.start_line))
    });
    let candidate_count = candidates.len();
    let required_roles = candidates
        .iter()
        .flat_map(|candidate| candidate.roles.iter().copied())
        .filter(|role| *role != ContextRole::Related)
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(candidates.len());
    let mut claimed = HashSet::new();
    for role in [
        ContextRole::Primary,
        ContextRole::Definition,
        ContextRole::Dependency,
        ContextRole::Dependent,
        ContextRole::Caller,
        ContextRole::Reference,
        ContextRole::Test,
        ContextRole::Config,
        ContextRole::Documentation,
    ] {
        if let Some((index, _)) = candidates
            .iter()
            .enumerate()
            .find(|(index, candidate)| !claimed.contains(index) && candidate.roles.contains(&role))
        {
            claimed.insert(index);
            ordered.push(candidates[index].clone());
        }
    }
    ordered.extend(
        candidates
            .into_iter()
            .enumerate()
            .filter_map(|(index, candidate)| (!claimed.contains(&index)).then_some(candidate)),
    );

    let mut items: Vec<ContextItem> = Vec::new();
    let mut used_tokens = estimated_header_tokens(
        &output_task,
        workspace,
        budget_tokens,
        &anchor_symbols,
        candidate_count,
        change_scope.as_ref(),
        &referenced_paths,
    );
    let mut file_counts = HashMap::<PathBuf, usize>::new();
    let mut truncated = task_truncated || referenced_paths_truncated;
    for candidate in ordered {
        if items.len() == MAX_ITEMS || used_tokens >= budget_tokens {
            truncated = true;
            break;
        }
        if items.len() >= TARGET_CONTEXT_ITEMS && context_roles_covered(&items, &required_roles) {
            truncated = true;
            break;
        }
        if file_counts
            .get(&candidate.hit.file_path)
            .is_some_and(|count| *count >= 3)
            || items
                .iter()
                .any(|item| substantially_overlaps(item, &candidate.hit))
        {
            continue;
        }

        let roles = candidate.roles.into_iter().collect::<Vec<_>>();
        let reasons = candidate.reasons.into_iter().collect::<Vec<_>>();
        let mut item = ContextItem {
            file_path: candidate.hit.file_path,
            start_line: candidate.hit.start_line,
            end_line: candidate.hit.end_line,
            roles,
            reasons,
            sources: candidate.hit.sources,
            preview: String::new(),
            estimated_tokens: 0,
        };
        let item_number = items.len() + 1;
        let wrapper_tokens = estimate_tokens(&render_markdown_item(item_number, &item));
        let remaining = budget_tokens.saturating_sub(used_tokens.saturating_add(wrapper_tokens));
        if remaining < 64 {
            truncated = true;
            break;
        }
        let per_item_budget = (budget_tokens / 4).clamp(96, 400).min(remaining);
        let (preview, preview_truncated, start_offset) =
            truncate_to_token_budget(&candidate.hit.preview, per_item_budget, task);
        if preview_truncated && estimate_tokens(&preview) < 32 {
            truncated = true;
            break;
        }
        item.preview = preview;
        item.start_line = item.start_line.saturating_add(start_offset);
        item.end_line = item
            .start_line
            .saturating_add(item.preview.lines().count().saturating_sub(1));
        item.estimated_tokens = estimate_tokens(&render_markdown_item(item_number, &item));
        if item.preview.trim().is_empty()
            || item.estimated_tokens > budget_tokens.saturating_sub(used_tokens)
        {
            continue;
        }
        truncated |= preview_truncated;
        used_tokens = used_tokens.saturating_add(item.estimated_tokens);
        let count = file_counts.entry(item.file_path.clone()).or_default();
        *count = count.saturating_add(1);
        items.push(item);
    }
    let mut bundle = ContextBundle {
        task: output_task,
        workspace: workspace.to_path_buf(),
        change_scope,
        referenced_paths,
        budget_tokens,
        used_tokens,
        candidate_count,
        truncated,
        anchor_symbols,
        coverage: ContextCoverage::default(),
        items,
    };
    finalize_bundle_metrics(&mut bundle);
    while bundle.used_tokens > bundle.budget_tokens && !bundle.items.is_empty() {
        bundle.items.pop();
        bundle.truncated = true;
        finalize_bundle_metrics(&mut bundle);
    }
    bundle
}

fn context_roles_covered(items: &[ContextItem], required_roles: &BTreeSet<ContextRole>) -> bool {
    required_roles
        .iter()
        .copied()
        .all(|role| items.iter().any(|item| item.roles.contains(&role)))
}

fn bound_change_scope(mut scope: ContextChangeScope, budget_tokens: usize) -> ContextChangeScope {
    let change_limit = (budget_tokens / 256).clamp(1, 12);
    let token_limit = (budget_tokens / 4).clamp(32, 1_024);
    let original_len = scope.changes.len();
    let mut used_tokens = 0;
    let mut changes = Vec::with_capacity(change_limit.min(original_len));
    for change in scope.changes.into_iter().take(change_limit) {
        let tokens = serde_json::to_string(&change)
            .map(|serialized| estimate_tokens(&serialized))
            .unwrap_or(token_limit.saturating_add(1));
        if tokens <= token_limit.saturating_sub(used_tokens) {
            used_tokens = used_tokens.saturating_add(tokens);
            changes.push(change);
        }
    }
    scope.changes_truncated |= scope.total_changes > changes.len() || original_len > changes.len();
    scope.changes = changes;
    scope
}

fn estimated_header_tokens(
    task: &str,
    workspace: &Path,
    budget_tokens: usize,
    anchor_symbols: &[String],
    candidate_count: usize,
    change_scope: Option<&ContextChangeScope>,
    referenced_paths: &[ContextInputPath],
) -> usize {
    let bundle = ContextBundle {
        task: task.to_string(),
        workspace: workspace.to_path_buf(),
        change_scope: change_scope.cloned(),
        referenced_paths: referenced_paths.to_vec(),
        budget_tokens,
        used_tokens: budget_tokens,
        candidate_count,
        truncated: true,
        anchor_symbols: anchor_symbols.to_vec(),
        coverage: ContextCoverage {
            files: MAX_ITEMS,
            primary: MAX_ITEMS,
            definitions: MAX_ITEMS,
            dependencies: MAX_ITEMS,
            dependents: MAX_ITEMS,
            callers: MAX_ITEMS,
            references: MAX_ITEMS,
            tests: MAX_ITEMS,
            config: MAX_ITEMS,
            documentation: MAX_ITEMS,
        },
        items: Vec::new(),
    };
    estimate_tokens(&render_markdown(&bundle)).saturating_add(8)
}

fn finalize_bundle_metrics(bundle: &mut ContextBundle) {
    let mut files = HashSet::new();
    let mut coverage = ContextCoverage::default();
    for item in &bundle.items {
        files.insert(&item.file_path);
        for role in &item.roles {
            match role {
                ContextRole::Primary => coverage.primary += 1,
                ContextRole::Definition => coverage.definitions += 1,
                ContextRole::Dependency => coverage.dependencies += 1,
                ContextRole::Dependent => coverage.dependents += 1,
                ContextRole::Caller => coverage.callers += 1,
                ContextRole::Reference => coverage.references += 1,
                ContextRole::Test => coverage.tests += 1,
                ContextRole::Config => coverage.config += 1,
                ContextRole::Documentation => coverage.documentation += 1,
                ContextRole::Related => {}
            }
        }
    }
    coverage.files = files.len();
    bundle.coverage = coverage;

    for _ in 0..4 {
        let estimated = estimate_tokens(&render_markdown(bundle));
        if estimated == bundle.used_tokens {
            break;
        }
        bundle.used_tokens = estimated;
    }
}

fn classify_role(hit: &SearchHit, requested: ContextRole) -> ContextRole {
    if requested == ContextRole::Primary {
        return ContextRole::Primary;
    }
    let file_role = classify_file_role(hit);
    match requested {
        ContextRole::Test if file_role == Some(ContextRole::Test) => ContextRole::Test,
        ContextRole::Config if file_role == Some(ContextRole::Config) => ContextRole::Config,
        ContextRole::Documentation if file_role == Some(ContextRole::Documentation) => {
            ContextRole::Documentation
        }
        ContextRole::Definition | ContextRole::Caller | ContextRole::Reference => requested,
        ContextRole::Related => file_role.unwrap_or(ContextRole::Related),
        _ => ContextRole::Related,
    }
}

fn classify_file_role(hit: &SearchHit) -> Option<ContextRole> {
    let path = &hit.file_path;
    let normalized = path.to_string_lossy().to_ascii_lowercase();
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let preview_lower = hit.preview.to_ascii_lowercase();
    let is_test = normalized.split('/').any(|part| {
        matches!(
            part,
            "test" | "tests" | "testing" | "spec" | "specs" | "__tests__"
        )
    }) || file.contains("_test.")
        || file.contains(".test.")
        || file.contains(".spec.")
        || preview_lower.contains("#[test]")
        || preview_lower.contains("@test")
        || preview_lower.contains("def test_")
        || preview_lower.contains("describe(")
        || preview_lower.contains("it(");
    if is_test {
        return Some(ContextRole::Test);
    }
    let is_documentation = normalized
        .split('/')
        .any(|part| matches!(part, "docs" | "doc" | "examples" | "example"))
        || path.extension().and_then(|extension| extension.to_str()) == Some("md");
    if is_documentation {
        return Some(ContextRole::Documentation);
    }
    let is_config = normalized.split('/').any(|part| part == "config")
        || file.starts_with('.')
        || file.contains("config")
        || matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("toml" | "yaml" | "yml" | "json")
        );
    if is_config {
        return Some(ContextRole::Config);
    }
    None
}

fn anchor_symbols(task: &str, primary_hits: &[SearchHit]) -> Vec<String> {
    let explicit_symbols = task_symbols(task);
    if !explicit_symbols.is_empty() {
        let mut anchors = Vec::new();
        let mut seen = HashSet::new();
        for symbol in explicit_symbols.into_iter().take(MAX_ANCHOR_SYMBOLS) {
            let terminal = terminal_symbol_member(&symbol).map(ToOwned::to_owned);
            for candidate in std::iter::once(symbol).chain(terminal) {
                if seen.insert(candidate.to_ascii_lowercase()) {
                    anchors.push(candidate);
                }
            }
        }
        return anchors;
    }

    let mut scored = BTreeMap::<String, usize>::new();
    let mut source_fallback = None;
    let mut test_fallback = None;
    let mut task_terms = significant_task_terms(task);
    if task_terms
        .iter()
        .any(|term| term == "pack" || term == "packs")
    {
        task_terms.push("bundle".to_string());
    }
    for (rank, hit) in primary_hits.iter().take(5).enumerate() {
        let is_test = classify_file_role(hit) == Some(ContextRole::Test);
        let mut names = if is_test {
            likely_test_subject_names(&hit.preview)
        } else {
            likely_definition_names(&hit.preview)
        };
        let mut seen_names = HashSet::new();
        names.retain(|name| seen_names.insert(name.clone()));
        let path_terms = hit
            .file_path
            .to_string_lossy()
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .flat_map(crate::text::split_identifier_segments)
            .collect::<HashSet<_>>();
        let path_overlap = task_terms
            .iter()
            .filter(|term| path_terms.contains(*term))
            .count();
        for symbol in names {
            if symbol.len() >= 3 && !is_generic_symbol(&symbol) {
                if is_test {
                    test_fallback.get_or_insert_with(|| symbol.clone());
                } else {
                    source_fallback.get_or_insert_with(|| symbol.clone());
                }
                let symbol_terms = crate::text::split_identifier_segments(&symbol);
                let overlap = symbol_terms
                    .iter()
                    .filter(|segment| {
                        task_terms.iter().any(|term| {
                            segment.as_str() == term
                                || segment.len() >= 4
                                    && term.len() >= 4
                                    && (segment.starts_with(term)
                                        || term.starts_with(segment.as_str()))
                        })
                    })
                    .count();
                if overlap > 0 {
                    let score = overlap.saturating_mul(12)
                        + path_overlap.saturating_mul(8)
                        + symbol_terms.len().min(4)
                        + 5usize.saturating_sub(rank);
                    scored
                        .entry(symbol)
                        .and_modify(|current| *current = (*current).max(score))
                        .or_insert(score);
                }
            }
        }
    }
    if scored.is_empty()
        && let Some(symbol) = source_fallback.or(test_fallback)
    {
        scored.insert(symbol, 1);
    }
    let mut symbols = scored.into_iter().collect::<Vec<_>>();
    symbols.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let best = symbols.first().map(|(_, score)| *score).unwrap_or_default();
    symbols
        .into_iter()
        .filter(|(_, score)| score.saturating_mul(3) >= best.saturating_mul(2))
        .map(|(symbol, _)| symbol)
        .take(MAX_ANCHOR_SYMBOLS)
        .collect()
}

fn relationship_anchor_keys(task: &str, anchors: &[String]) -> HashSet<String> {
    let explicit = task_symbols(task);
    if explicit.is_empty() {
        return anchors
            .iter()
            .map(|symbol| symbol.to_ascii_lowercase())
            .collect();
    }
    explicit
        .into_iter()
        .take(MAX_ANCHOR_SYMBOLS)
        .map(|symbol| symbol.to_ascii_lowercase())
        .collect()
}

fn terminal_symbol_member(symbol: &str) -> Option<&str> {
    let member = symbol
        .rsplit_once(['.', '#'])
        .map(|(_, member)| member)
        .or_else(|| symbol.rsplit_once("::").map(|(_, member)| member))?;
    (member.len() >= 3
        && member
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$'))
    .then_some(member)
}

fn likely_test_subject_names(text: &str) -> Vec<String> {
    let mut names = likely_jsx_component_names(text);
    names.extend(likely_called_names(text));
    names
}

fn likely_jsx_component_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        if rest.starts_with('/') {
            continue;
        }
        let Some(first) = rest.chars().next() else {
            break;
        };
        if !first.is_ascii_uppercase() {
            continue;
        }
        let end = rest
            .find(|character: char| {
                !character.is_ascii_alphanumeric()
                    && character != '_'
                    && character != '$'
                    && character != '.'
            })
            .unwrap_or(rest.len());
        let name = rest[..end].rsplit('.').next().unwrap_or_default();
        if name.len() >= 3 && !names.iter().any(|existing| existing == name) {
            names.push(name.to_string());
        }
    }
    names.reverse();
    names
}

fn likely_called_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let bytes = line.as_bytes();
        for (open, _) in line.match_indices('(') {
            let mut start = open;
            while start > 0 {
                let byte = bytes[start - 1];
                if byte.is_ascii_alphanumeric() || byte == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            let name = &line[start..open];
            let prefix = line[..start].trim_end();
            let is_definition = ["def", "fn", "func", "function", "fun"]
                .iter()
                .any(|keyword| prefix.ends_with(keyword));
            if name.len() >= 3
                && !is_definition
                && !is_generic_call(name)
                && !has_test_harness_receiver(prefix)
            {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn is_generic_call(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace('_', "");
    if normalized.starts_with("assert")
        || [
            "findallby",
            "findby",
            "getallby",
            "getby",
            "queryallby",
            "queryby",
        ]
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return true;
    }
    matches!(
        normalized.as_str(),
        "afterall"
            | "aftereach"
            | "act"
            | "beforeall"
            | "beforeeach"
            | "context"
            | "cleanup"
            | "debugassert"
            | "debugasserteq"
            | "deepequal"
            | "deepstrictequal"
            | "describe"
            | "doesnotmatch"
            | "doesnotreject"
            | "doesnotthrow"
            | "eprint"
            | "eprintln"
            | "eq"
            | "equal"
            | "equals"
            | "expect"
            | "fail"
            | "fixture"
            | "fit"
            | "format"
            | "if"
            | "it"
            | "match"
            | "mock"
            | "mount"
            | "notdeepequal"
            | "notequal"
            | "notstrictequal"
            | "ok"
            | "parametrize"
            | "patch"
            | "print"
            | "println"
            | "raises"
            | "rejects"
            | "render"
            | "renderhook"
            | "setuptest"
            | "some"
            | "specify"
            | "spyon"
            | "strictequal"
            | "suite"
            | "shallow"
            | "teardowntest"
            | "test"
            | "throws"
            | "vec"
            | "waitfor"
            | "waitforelementtoberemoved"
            | "within"
            | "xdescribe"
            | "xit"
            | "xtest"
    )
}

fn has_test_harness_receiver(prefix: &str) -> bool {
    let Some(chain) = prefix.strip_suffix('.') else {
        return false;
    };
    let expression = receiver_expression_suffix(chain.trim_end());
    let mut identifiers = expression
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|part| !part.is_empty());
    let Some(root) = identifiers.next() else {
        return false;
    };
    let receiver = identifiers.next_back().unwrap_or(root);
    is_test_harness_identifier(root) || is_test_harness_identifier(receiver)
}

fn receiver_expression_suffix(value: &str) -> &str {
    let mut depth = 0usize;
    let mut start = value.len();
    for (index, character) in value.char_indices().rev() {
        match character {
            ')' | ']' | '}' => depth += 1,
            '(' | '[' | '{' if depth > 0 => depth -= 1,
            '(' | '[' | '{' => break,
            _ if depth > 0 => {}
            _ if character.is_ascii_alphanumeric() || matches!(character, '_' | '$' | '.') => {}
            _ => break,
        }
        start = index;
    }
    &value[start..]
}

fn is_test_harness_identifier(identifier: &str) -> bool {
    let identifier = identifier.to_ascii_lowercase().replace('_', "");
    matches!(
        identifier.as_str(),
        "assert"
            | "expect"
            | "fireevent"
            | "jest"
            | "screen"
            | "should"
            | "sinon"
            | "userevent"
            | "vi"
    ) || identifier.starts_with("assert")
        || identifier.starts_with("expect")
        || identifier.ends_with("assert")
}

fn task_symbols(task: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    task.split(|character: char| {
        !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '$'
            && character != ':'
            && character != '.'
            && character != '#'
    })
    .map(|part| part.trim_matches([':', '.', '$', '#']))
    .filter(|part| part.len() >= 3)
    .filter(|part| !looks_like_path_location(part))
    .filter(|part| {
        part.contains('_')
            || part.contains("::")
            || part.contains('#')
            || part.contains('.') && !looks_like_file_name(part)
            || looks_like_mixed_case_identifier(part)
    })
    .filter(|part| !is_generic_symbol(part))
    .filter(|part| seen.insert(part.to_ascii_lowercase()))
    .map(ToOwned::to_owned)
    .collect()
}

fn looks_like_path_location(token: &str) -> bool {
    let Some((prefix, line_or_column)) = token.rsplit_once(':') else {
        return false;
    };
    line_or_column
        .chars()
        .all(|character| character.is_ascii_digit())
        && prefix.contains('.')
}

fn looks_like_mixed_case_identifier(token: &str) -> bool {
    let lower_to_upper = token
        .chars()
        .zip(token.chars().skip(1))
        .any(|(left, right)| left.is_ascii_lowercase() && right.is_ascii_uppercase());
    let leading_uppercase = token
        .chars()
        .take_while(|character| character.is_ascii_uppercase())
        .count();
    lower_to_upper
        || leading_uppercase >= 2
            && token
                .chars()
                .skip(leading_uppercase)
                .filter(|character| character.is_ascii_lowercase())
                .count()
                >= 2
}

fn looks_like_file_name(token: &str) -> bool {
    let Some((_, extension)) = token.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "c" | "cc"
            | "cpp"
            | "cs"
            | "css"
            | "dart"
            | "ex"
            | "exs"
            | "go"
            | "h"
            | "hpp"
            | "html"
            | "java"
            | "js"
            | "json"
            | "jsx"
            | "kt"
            | "kts"
            | "lua"
            | "md"
            | "php"
            | "proto"
            | "py"
            | "rb"
            | "rs"
            | "scala"
            | "sh"
            | "sql"
            | "swift"
            | "toml"
            | "ts"
            | "tsx"
            | "xml"
            | "yaml"
            | "yml"
            | "zig"
    )
}

fn is_generic_symbol(symbol: &str) -> bool {
    let mut characters = symbol.chars();
    let is_type_name = characters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        && characters
            .clone()
            .any(|character| character.is_ascii_lowercase())
        && characters.all(|character| character.is_ascii_alphanumeric());
    if is_type_name {
        return false;
    }
    matches!(
        symbol.to_ascii_lowercase().as_str(),
        "main"
            | "new"
            | "default"
            | "test"
            | "tests"
            | "config"
            | "result"
            | "error"
            | "name"
            | "value"
            | "item"
            | "items"
            | "query"
            | "options"
            | "context"
            | "token"
            | "token_mut"
            | "run"
    )
}

fn significant_task_terms(task: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    task.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 3)
        .filter(|term| {
            !matches!(
                term.as_str(),
                "the"
                    | "and"
                    | "for"
                    | "with"
                    | "from"
                    | "into"
                    | "where"
                    | "what"
                    | "how"
                    | "fix"
                    | "add"
                    | "change"
                    | "update"
                    | "make"
                    | "create"
                    | "ensure"
                    | "implement"
                    | "improve"
                    | "refactor"
                    | "remove"
                    | "support"
                    | "task"
                    | "behavior"
            )
        })
        .flat_map(|term| crate::text::split_identifier_segments(&term))
        .filter(|term| term.len() >= 3 && seen.insert(term.clone()))
        .collect()
}

fn hit_matches_task(hit: &SearchHit, task: &str) -> bool {
    let haystack = format!("{}\n{}", hit.file_path.display(), hit.preview).to_ascii_lowercase();
    let explicit_symbols = task_symbols(task);
    if !explicit_symbols.is_empty() {
        return explicit_symbols
            .iter()
            .any(|symbol| haystack.contains(&symbol.to_ascii_lowercase()));
    }
    let terms = significant_task_terms(task);
    let haystack_terms = haystack
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .flat_map(crate::text::split_identifier_segments)
        .collect::<HashSet<_>>();
    let matches = terms
        .iter()
        .filter(|term| {
            haystack_terms.iter().any(|candidate| {
                candidate == *term
                    || candidate.len() >= 4
                        && term.len() >= 4
                        && (candidate.starts_with(term.as_str()) || term.starts_with(candidate))
            })
        })
        .count();
    matches
        >= if terms.len() >= 5 {
            3
        } else if terms.len() >= 3 {
            2
        } else {
            1
        }
}

fn task_path_overlap(path: &Path, task: &str) -> usize {
    let path_terms = path
        .to_string_lossy()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .flat_map(crate::text::split_identifier_segments)
        .collect::<HashSet<_>>();
    significant_task_terms(task)
        .iter()
        .filter(|term| path_terms.contains(*term))
        .count()
}

fn hit_matches_any_anchor(hit: &SearchHit, anchors: &[String]) -> bool {
    let haystack = format!("{}\n{}", hit.file_path.display(), hit.preview).to_ascii_lowercase();
    anchors
        .iter()
        .any(|anchor| haystack.contains(&anchor.to_ascii_lowercase()))
}

fn graph_support_matches_task(hit: &SearchHit, task: &str, kind: FileEdgeKind) -> bool {
    if hit_matches_task(hit, task) {
        return true;
    }
    if kind != FileEdgeKind::Config {
        return false;
    }
    let terms = significant_task_terms(task);
    let haystack = format!("{}\n{}", hit.file_path.display(), hit.preview).to_ascii_lowercase();
    let haystack_terms = haystack
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .flat_map(crate::text::split_identifier_segments)
        .collect::<HashSet<_>>();
    terms
        .iter()
        .filter(|term| haystack_terms.contains(*term))
        .take(2)
        .count()
        >= 2
}

fn substantially_overlaps(item: &ContextItem, hit: &SearchHit) -> bool {
    if item.file_path != hit.file_path {
        return false;
    }
    ranges_substantially_overlap(item.start_line, item.end_line, hit.start_line, hit.end_line)
}

pub fn estimate_tokens(text: &str) -> usize {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Class {
        Word,
        Space,
        Newline,
        AsciiOther,
        Unicode,
    }
    let mut calibrated = 0usize;
    let mut conservative = 0usize;
    let mut class = None;
    let mut run = 0usize;
    let flush = |class: Option<Class>, run: usize| match class {
        Some(Class::Word) => (run.div_ceil(4), 0),
        Some(Class::Space) => (usize::from(run > 0), 0),
        Some(Class::Newline | Class::AsciiOther) => (run, 0),
        Some(Class::Unicode) => (0, run),
        None => (0, 0),
    };
    for character in text.chars() {
        let next = if character == '\n' {
            Class::Newline
        } else if character.is_whitespace() {
            Class::Space
        } else if character.is_ascii_alphanumeric() || character == '_' {
            Class::Word
        } else if character.is_ascii() {
            Class::AsciiOther
        } else {
            Class::Unicode
        };
        if class == Some(next) {
            run += 1;
        } else {
            let (calibrated_run, conservative_run) = flush(class, run);
            calibrated += calibrated_run;
            conservative += conservative_run;
            class = Some(next);
            run = 1;
        }
    }
    let (calibrated_run, conservative_run) = flush(class, run);
    calibrated += calibrated_run;
    conservative += conservative_run;
    // Thirty representative code packs measured 1.79x-1.87x above
    // o200k_base and cl100k_base. Calibrate measured ASCII/code classes while
    // retaining one estimate per Unicode scalar outside that sample.
    calibrated
        .saturating_mul(3)
        .div_ceil(5)
        .saturating_add(conservative)
}

fn truncate_to_token_budget(text: &str, budget: usize, task: &str) -> (String, bool, usize) {
    if text.trim().is_empty() {
        return (String::new(), false, 0);
    }
    if estimate_tokens(text) <= budget {
        return (text.trim().to_string(), false, 0);
    }
    let lines = text.lines().collect::<Vec<_>>();
    let terms = significant_task_terms(task);
    let explicit_symbols = task_symbols(task)
        .into_iter()
        .map(|symbol| symbol.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let focus = lines
        .iter()
        .enumerate()
        .max_by_key(|(_, line)| {
            let lower = line.to_ascii_lowercase();
            let exact_identifier_bonus =
                if explicit_symbols.iter().any(|symbol| lower.contains(symbol)) {
                    100
                } else {
                    0
                };
            exact_identifier_bonus
                + terms
                    .iter()
                    .filter(|term| lower.contains(term.as_str()))
                    .map(String::len)
                    .sum::<usize>()
        })
        .map(|(index, _)| index)
        .unwrap_or(lines.len() / 2);
    let mut start = focus;
    let mut end = focus.saturating_add(1).min(lines.len());
    if estimate_tokens(lines[focus]) > budget {
        let mut output = String::new();
        for character in lines[focus].chars() {
            let candidate = format!("{output}{character}");
            if estimate_tokens(&candidate) > budget {
                break;
            }
            output.push(character);
        }
        return (output.trim().to_string(), true, focus);
    }
    loop {
        let mut changed = false;
        if end < lines.len() {
            let candidate = lines[start..=end].join("\n");
            if estimate_tokens(&candidate) <= budget {
                end += 1;
                changed = true;
            }
        }
        if start > 0 {
            let candidate = lines[start - 1..end].join("\n");
            if estimate_tokens(&candidate) <= budget {
                start -= 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (lines[start..end].join("\n").trim().to_string(), true, start)
}

pub fn render_markdown(bundle: &ContextBundle) -> String {
    let mut output = String::new();
    output.push_str("# ivygrep context\n\n");
    output.push_str(&format!("Task: {}\n\n", bundle.task));
    output.push_str(&format!("Workspace: {}\n\n", bundle.workspace.display()));
    if let Some(scope) = &bundle.change_scope {
        let since = scope.since.as_deref().map_or_else(
            || "HEAD".to_string(),
            |reference| format!("{reference}...HEAD"),
        );
        output.push_str(&format!(
            "Changes: {} file{} from {since}{}{}\n",
            scope.total_changes,
            if scope.total_changes == 1 { "" } else { "s" },
            if scope.dirty_worktree {
                " plus dirty worktree"
            } else {
                ""
            },
            if scope.changes_truncated {
                " (list truncated)"
            } else {
                ""
            },
        ));
        let change_limit = (bundle.budget_tokens / 256).clamp(1, 12);
        for change in scope.changes.iter().take(change_limit) {
            let sources = change
                .sources
                .iter()
                .map(|source| source.label())
                .collect::<Vec<_>>()
                .join(", ");
            let rename = change.old_path.as_ref().map_or_else(String::new, |old| {
                format!(" from {}", crate::workspace::index_path_string(old))
            });
            output.push_str(&format!(
                "- {}: {}{rename} ({sources})\n",
                change.status.label(),
                crate::workspace::index_path_string(&change.file_path),
            ));
        }
        if scope.total_changes > change_limit {
            output.push_str(&format!(
                "- ... {} more\n",
                scope.total_changes - change_limit
            ));
        }
        output.push('\n');
    }
    if !bundle.referenced_paths.is_empty() {
        output.push_str("Input paths: ");
        output.push_str(
            &bundle
                .referenced_paths
                .iter()
                .map(|reference| {
                    reference.line.map_or_else(
                        || crate::workspace::index_path_string(&reference.file_path),
                        |line| {
                            format!(
                                "{}:{line}",
                                crate::workspace::index_path_string(&reference.file_path)
                            )
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        output.push_str("\n\n");
    }
    output.push_str(&format!(
        "Budget: {} / {} estimated tokens{}\n",
        bundle.used_tokens,
        bundle.budget_tokens,
        if bundle.truncated { " (truncated)" } else { "" }
    ));
    if !bundle.anchor_symbols.is_empty() {
        output.push_str(&format!("Anchors: {}\n", bundle.anchor_symbols.join(", ")));
    }
    output.push_str(&format!(
        "Coverage: {} files | {} primary | {} definitions | {} dependencies | {} dependents | {} callers | {} references | {} tests | {} config | {} docs\n",
        bundle.coverage.files,
        bundle.coverage.primary,
        bundle.coverage.definitions,
        bundle.coverage.dependencies,
        bundle.coverage.dependents,
        bundle.coverage.callers,
        bundle.coverage.references,
        bundle.coverage.tests,
        bundle.coverage.config,
        bundle.coverage.documentation,
    ));
    output.push_str(&format!(
        "Candidates: {} retrieved | {} selected\n",
        bundle.candidate_count,
        bundle.items.len()
    ));
    output.push_str("\n## Evidence\n");
    for (index, item) in bundle.items.iter().enumerate() {
        output.push_str(&render_markdown_item(index.saturating_add(1), item));
    }
    output
}

fn render_markdown_item(index: usize, item: &ContextItem) -> String {
    let roles = item
        .roles
        .iter()
        .map(|role| role.label())
        .collect::<Vec<_>>()
        .join(", ");
    let mut output = format!(
        "\n### {}. {}:{}-{} [{}]\n\n",
        index,
        crate::workspace::index_path_string(&item.file_path),
        item.start_line,
        item.end_line,
        roles
    );
    if !item.reasons.is_empty() {
        output.push_str(&format!("Why: {}.\n\n", item.reasons.join("; ")));
    }
    if !item.sources.is_empty() {
        output.push_str(&format!("Signals: {}.\n\n", item.sources.join(", ")));
    }
    let language = language_fence(&item.file_path);
    output.push_str(&format!("```{language}\n{}\n```\n", item.preview));
    output
}

fn language_fence(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js" | "jsx") => "javascript",
        Some("ts" | "tsx") => "typescript",
        Some("go") => "go",
        Some("java") => "java",
        Some("rb") => "ruby",
        Some("sh" | "bash") => "bash",
        Some("md") => "markdown",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(path: &str, line: usize, role: ContextRole, text: &str, score: f64) -> Candidate {
        Candidate {
            hit: SearchHit {
                file_path: PathBuf::from(path),
                start_line: line,
                end_line: line + text.lines().count().saturating_sub(1),
                preview: text.to_string(),
                reason: String::new(),
                score: 1.0,
                sources: vec!["lexical".to_string()],
                neural_requested: false,
                neural_executed: false,
            },
            roles: BTreeSet::from([role]),
            reasons: BTreeSet::from([format!("{} evidence", role.label())]),
            fused_score: score,
        }
    }

    #[test]
    fn token_estimate_is_code_aware_and_deterministic() {
        assert_eq!(estimate_tokens("calculate_tax"), 3);
        assert_eq!(estimate_tokens("fn x() {\n  x();\n}"), 9);
        assert_eq!(
            estimate_tokens("fn x() {\n  x();\n}"),
            estimate_tokens("fn x() {\n  x();\n}")
        );
        assert_eq!(truncate_to_token_budget("", 100, "task").0, "");
    }

    #[test]
    fn token_estimate_keeps_unicode_scalars_conservative() {
        assert_eq!(estimate_tokens(&"🙂".repeat(100)), 100);
        assert_eq!(estimate_tokens(&"索引".repeat(50)), 100);
    }

    #[test]
    fn task_path_seed_reads_current_file_at_referenced_line() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/auth.rs"),
            "pub fn refresh_token() -> bool { refresh_token_race_fix() }\nfn refresh_token_race_fix() -> bool { true }\n",
        )
        .unwrap();
        let hit = context_seed_hit(
            &root.path().canonicalize().unwrap(),
            &ContextSeed {
                file_path: PathBuf::from("src/auth.rs"),
                line: Some(2),
                git_revision: None,
                reason: "mentioned at line 2 in task or stack trace".to_string(),
                source: "task_input".to_string(),
                priority: 3,
            },
            "panic at src/auth.rs:2:7",
        )
        .unwrap()
        .unwrap();
        assert!(hit.preview.contains("refresh_token_race_fix"));
        assert_eq!(hit.sources, ["task_input"]);
    }

    fn commit_seed_source(root: &Path, content: &str) {
        fs::write(root.join("source.rs"), content).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "source.rs"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "fixture",
            ],
        ] {
            let output = Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn git_seed_fallback_rejects_a_redirected_workspace_root() {
        let fixture = tempfile::tempdir().unwrap();
        let directory = fixture.path().canonicalize().unwrap();
        let root = directory.join("workspace");
        let outside = directory.join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        commit_seed_source(&outside, "OUTSIDE_GIT_SEED_SENTINEL\n");
        fs::remove_file(outside.join("source.rs")).unwrap();
        let seed = ContextSeed {
            file_path: PathBuf::from("source.rs"),
            line: None,
            git_revision: Some("HEAD".to_string()),
            reason: "deleted file".to_string(),
            source: "git_diff".to_string(),
            priority: 3,
        };
        assert_eq!(
            context_seed_content(&outside, &seed).unwrap().as_deref(),
            Some("OUTSIDE_GIT_SEED_SENTINEL\n")
        );
        fs::rename(&root, directory.join("original")).unwrap();
        std::os::unix::fs::symlink(&outside, &root).unwrap();
        assert!(context_seed_content(&root, &seed).unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn git_seed_command_keeps_its_opened_root_after_replacement() {
        let fixture = tempfile::tempdir().unwrap();
        let directory = fixture.path().canonicalize().unwrap();
        let root = directory.join("workspace");
        let outside = directory.join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        commit_seed_source(&root, "INSIDE_GIT_SEED_SENTINEL\n");
        commit_seed_source(&outside, "OUTSIDE_GIT_SEED_SENTINEL\n");
        fs::remove_file(root.join("source.rs")).unwrap();
        fs::remove_file(outside.join("source.rs")).unwrap();

        let mut command = git_seed_command(&root).unwrap();
        fs::rename(&root, directory.join("original")).unwrap();
        std::os::unix::fs::symlink(&outside, &root).unwrap();
        let output = command
            .args(["cat-file", "blob", "HEAD:source.rs"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"INSIDE_GIT_SEED_SENTINEL\n");
    }

    #[cfg(windows)]
    #[test]
    fn git_seed_history_fails_closed_without_a_pinned_working_directory() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        commit_seed_source(&root, "HISTORICAL_GIT_SEED_SENTINEL\n");
        fs::remove_file(root.join("source.rs")).unwrap();
        let seed = ContextSeed {
            file_path: PathBuf::from("source.rs"),
            line: None,
            git_revision: Some("HEAD".to_string()),
            reason: "deleted file".to_string(),
            source: "git_diff".to_string(),
            priority: 3,
        };
        assert!(context_seed_content(&root, &seed).unwrap().is_none());
    }

    #[test]
    fn graph_roles_follow_edge_direction() {
        assert_eq!(
            graph_context_role(FileEdgeKind::Test, true),
            ContextRole::Test
        );
        assert_eq!(
            graph_context_role(FileEdgeKind::Test, false),
            ContextRole::Dependency
        );
        assert_eq!(
            graph_context_role(FileEdgeKind::Config, true),
            ContextRole::Config
        );
        assert_eq!(
            graph_context_role(FileEdgeKind::Config, false),
            ContextRole::Related
        );
        assert_eq!(
            graph_context_role(FileEdgeKind::Documentation, false),
            ContextRole::Documentation
        );
        assert_eq!(
            graph_context_role(FileEdgeKind::Documentation, true),
            ContextRole::Related
        );
    }

    #[test]
    fn bundle_respects_budget_and_keeps_relationship_roles() {
        let candidates = vec![
            candidate(
                "src/auth.rs",
                1,
                ContextRole::Primary,
                "pub fn validate_token(token: &str) -> bool { !token.is_empty() }",
                1.0,
            ),
            candidate(
                "src/server.rs",
                10,
                ContextRole::Caller,
                "pub fn authenticate(token: &str) -> bool { validate_token(token) }",
                0.8,
            ),
            candidate(
                "tests/auth_test.rs",
                3,
                ContextRole::Test,
                "#[test]\nfn rejects_empty_token() { assert!(!validate_token(\"\")); }",
                0.7,
            ),
        ];
        let bundle = assemble_bundle(
            "change validate_token",
            Path::new("/repo"),
            500,
            vec!["validate_token".to_string()],
            None,
            Vec::new(),
            candidates,
        );
        assert!(bundle.used_tokens <= bundle.budget_tokens);
        assert_eq!(
            bundle.used_tokens,
            estimate_tokens(&render_markdown(&bundle))
        );
        assert!(
            bundle
                .items
                .iter()
                .any(|item| item.roles.contains(&ContextRole::Primary))
        );
        assert!(
            bundle
                .items
                .iter()
                .any(|item| item.roles.contains(&ContextRole::Caller))
        );
    }

    #[test]
    fn item_target_waits_for_required_relationship_roles() {
        let mut candidates = vec![
            candidate("src/shared.rs", 1, ContextRole::Primary, "shared", 1.0),
            candidate("src/shared.rs", 1, ContextRole::Test, "shared", 0.9),
        ];
        candidates.extend((0..12).map(|index| {
            candidate(
                &format!("src/related_{index}.rs"),
                1,
                ContextRole::Related,
                "related",
                0.3 - index as f64 / 100.0,
            )
        }));
        candidates.push(candidate(
            "tests/fallback.rs",
            1,
            ContextRole::Test,
            "fallback test",
            0.1,
        ));
        candidates.push(candidate(
            "src/trailing.rs",
            1,
            ContextRole::Related,
            "trailing",
            0.0,
        ));

        let bundle = assemble_bundle(
            "change shared behavior",
            Path::new("/repo"),
            8_000,
            Vec::new(),
            None,
            Vec::new(),
            candidates,
        );

        assert!(bundle.items.len() >= TARGET_CONTEXT_ITEMS);
        assert!(bundle.items.len() < MAX_ITEMS);
        assert!(
            bundle
                .items
                .iter()
                .any(|item| item.file_path == Path::new("tests/fallback.rs"))
        );
        let required_roles = BTreeSet::from([ContextRole::Primary, ContextRole::Test]);
        assert!(context_roles_covered(&bundle.items, &required_roles));
    }

    #[test]
    fn rendered_pack_never_exceeds_requested_budget() {
        for budget in [256, 384, 800, 4_000] {
            let candidates = (0usize..30)
                .map(|index| {
                    candidate(
                        &format!("src/module_{index}.rs"),
                        1,
                        ContextRole::Primary,
                        &format!(
                            "pub fn implementation_{index}() {{\n{}\n}}",
                            "    execute_work();\n".repeat(80)
                        ),
                        1.0 / index.saturating_add(1) as f64,
                    )
                })
                .collect();
            let bundle = assemble_bundle(
                "change implementation behavior",
                Path::new("/repo"),
                budget,
                Vec::new(),
                None,
                Vec::new(),
                candidates,
            );
            assert!(bundle.used_tokens <= budget, "{bundle:#?}");
            assert_eq!(
                bundle.used_tokens,
                estimate_tokens(&render_markdown(&bundle))
            );
            assert!(bundle.items.len() <= MAX_ITEMS);
        }
    }

    #[test]
    fn long_issue_input_is_bounded_and_marked_truncated() {
        let task = "stack frame at src/auth.rs:42 with refresh token failure\n".repeat(2_000);
        let bundle = assemble_bundle(
            &task,
            Path::new("/repo"),
            256,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        );
        assert!(bundle.truncated);
        assert!(bundle.task.len() < task.len());
        assert!(bundle.used_tokens <= bundle.budget_tokens, "{bundle:#?}");
        assert_eq!(
            bundle.used_tokens,
            estimate_tokens(&render_markdown(&bundle))
        );
    }

    #[test]
    fn serialized_diff_metadata_stays_inside_small_pack_budget() {
        let changes = (0..40)
            .map(|index| crate::context_input::ContextChange {
                file_path: PathBuf::from(format!("src/changed_module_{index}.rs")),
                old_path: None,
                status: crate::context_input::ContextChangeStatus::Modified,
                sources: vec![crate::context_input::ContextChangeSource::Since],
            })
            .collect::<Vec<_>>();
        let bundle = assemble_bundle(
            "change modules",
            Path::new("/repo"),
            256,
            Vec::new(),
            Some(ContextChangeScope {
                since: Some("main".to_string()),
                base_commit: Some("abc123".to_string()),
                dirty_worktree: false,
                total_changes: changes.len(),
                changes_truncated: false,
                changes,
            }),
            Vec::new(),
            vec![candidate(
                "src/changed_module_0.rs",
                1,
                ContextRole::Primary,
                "pub fn changed_module() {}",
                1.0,
            )],
        );
        assert!(bundle.used_tokens <= bundle.budget_tokens, "{bundle:#?}");
        assert_eq!(
            bundle.used_tokens,
            estimate_tokens(&render_markdown(&bundle))
        );
        let scope = bundle.change_scope.as_ref().unwrap();
        assert_eq!(scope.changes.len(), 1);
        assert!(scope.changes_truncated);
        assert!(
            estimate_tokens(&serde_json::to_string(&scope.changes).unwrap())
                <= bundle.budget_tokens / 4
        );
    }

    #[test]
    fn overlapping_snippets_are_not_repeated() {
        let candidates = vec![
            candidate("src/lib.rs", 10, ContextRole::Primary, "a\nb\nc\nd", 1.0),
            candidate("src/lib.rs", 11, ContextRole::Related, "b\nc\nd\ne", 0.9),
        ];
        let bundle = assemble_bundle(
            "task",
            Path::new("/repo"),
            500,
            Vec::new(),
            None,
            Vec::new(),
            candidates,
        );
        assert_eq!(bundle.items.len(), 1);
    }

    #[test]
    fn task_symbols_require_identifier_shape() {
        assert_eq!(
            task_symbols("fix validateToken and calculate_tax"),
            ["validateToken", "calculate_tax"]
        );
        assert!(task_symbols("where is authentication handled").is_empty());
        assert!(task_symbols("choose HTTP TLS APIs").is_empty());
        assert!(task_symbols("panic at src/auth.rs:42:7").is_empty());
        assert!(task_symbols("fix src/auth.rs and config.yaml").is_empty());
        assert_eq!(
            task_symbols("change UserService and std::io and client.send"),
            ["UserService", "std::io", "client.send"]
        );
        assert_eq!(
            task_symbols("fix HTTPServer, URLParser, and JSONDecoder"),
            ["HTTPServer", "URLParser", "JSONDecoder"]
        );
    }

    #[test]
    fn qualified_explicit_anchors_include_terminal_members() {
        assert_eq!(
            anchor_symbols("fix client.send and server.receive retries", &[]),
            ["client.send", "send", "server.receive", "receive"]
        );
        assert_eq!(
            anchor_symbols("change UserService.handle through std::io", &[]),
            ["UserService.handle", "handle", "std::io"]
        );
        assert_eq!(
            anchor_symbols("fix Worker#run retries", &[]),
            ["Worker#run", "run"]
        );
        assert_eq!(
            relationship_anchor_keys(
                "fix client.send and server.receive retries",
                &anchor_symbols("fix client.send and server.receive retries", &[]),
            ),
            HashSet::from(["client.send".to_string(), "server.receive".to_string()])
        );
    }

    #[test]
    fn inferred_anchors_prefer_task_overlap() {
        let primary = vec![
            SearchHit {
                file_path: PathBuf::from("tests/context_test.rs"),
                start_line: 1,
                end_line: 1,
                preview: "fn bundle_respects_budget_and_keeps_relationship_roles() {}".to_string(),
                reason: String::new(),
                score: 2.0,
                sources: Vec::new(),
                neural_requested: false,
                neural_executed: false,
            },
            SearchHit {
                file_path: PathBuf::from("src/context.rs"),
                start_line: 1,
                end_line: 3,
                preview: "struct Candidate;\npub fn build_context_bundle() {}\nstruct SearchPool;"
                    .to_string(),
                reason: String::new(),
                score: 1.0,
                sources: Vec::new(),
                neural_requested: false,
                neural_executed: false,
            },
        ];
        assert_eq!(
            anchor_symbols("add token budgeted context packs", &primary),
            ["build_context_bundle"]
        );
    }

    #[test]
    fn inferred_anchors_fall_back_for_synonym_heavy_tasks() {
        let primary = vec![SearchHit {
            file_path: PathBuf::from("src/auth.rs"),
            start_line: 1,
            end_line: 1,
            preview: "pub fn validate_token(token: &str) -> bool { !token.is_empty() }".to_string(),
            reason: String::new(),
            score: 1.0,
            sources: Vec::new(),
            neural_requested: false,
            neural_executed: false,
        }];
        assert_eq!(
            anchor_symbols("fix authentication failures", &primary),
            ["validate_token"]
        );
    }

    #[test]
    fn inferred_anchors_preserve_pascal_case_type_names() {
        let primary = vec![SearchHit {
            file_path: PathBuf::from("src/token.rs"),
            start_line: 1,
            end_line: 1,
            preview: "pub struct Token(pub String);".to_string(),
            reason: String::new(),
            score: 1.0,
            sources: Vec::new(),
            neural_requested: false,
            neural_executed: false,
        }];
        assert_eq!(
            anchor_symbols("fix authentication failures", &primary),
            ["Token"]
        );
        assert!(is_generic_symbol("token"));
        assert!(!is_generic_symbol("Token"));
    }

    #[test]
    fn inferred_anchors_skip_test_harness_calls() {
        let primary = vec![SearchHit {
            file_path: PathBuf::from("src/auth.test.ts"),
            start_line: 1,
            end_line: 3,
            preview: "describe('auth', () => {\n  test('rejects invalid tokens', () => { expect(validateToken('')).toBe(false); });\n});"
                .to_string(),
            reason: String::new(),
            score: 1.0,
            sources: Vec::new(),
            neural_requested: false,
            neural_executed: false,
        }];
        assert_eq!(
            anchor_symbols("fix authentication failures", &primary),
            ["validateToken"]
        );
    }

    #[test]
    fn inferred_anchors_skip_assertion_methods() {
        for preview in [
            "assert.equal(validateToken(''), false);",
            "should.equal(validateToken(''), false);",
            "expect(result).toEqual(validateToken(''));",
        ] {
            let primary = vec![SearchHit {
                file_path: PathBuf::from("src/auth.test.ts"),
                start_line: 1,
                end_line: 1,
                preview: preview.to_string(),
                reason: String::new(),
                score: 1.0,
                sources: Vec::new(),
                neural_requested: false,
                neural_executed: false,
            }];
            assert_eq!(
                anchor_symbols("fix authentication failures", &primary),
                ["validateToken"],
                "{preview}"
            );
        }
    }

    #[test]
    fn inferred_anchors_prefer_source_subjects_over_test_helpers() {
        let primary = vec![
            SearchHit {
                file_path: PathBuf::from("src/user-card.test.tsx"),
                start_line: 1,
                end_line: 1,
                preview: "render(<UserCard />); screen.getByText('profile');".to_string(),
                reason: String::new(),
                score: 2.0,
                sources: Vec::new(),
                neural_requested: false,
                neural_executed: false,
            },
            SearchHit {
                file_path: PathBuf::from("src/user-card.tsx"),
                start_line: 1,
                end_line: 1,
                preview: "export function UserCard() { return <section />; }".to_string(),
                reason: String::new(),
                score: 1.0,
                sources: Vec::new(),
                neural_requested: false,
                neural_executed: false,
            },
        ];
        assert_eq!(
            anchor_symbols("fix profile display failures", &primary),
            ["UserCard"]
        );
    }

    #[test]
    fn jsx_components_are_test_subjects_without_source_hits() {
        assert_eq!(
            likely_test_subject_names(
                "render(<Provider><UserCard /></Provider>); screen.getByRole('article');"
            ),
            ["UserCard", "Provider"]
        );
    }

    #[test]
    fn generic_test_calls_cover_common_framework_shapes() {
        for call in [
            "assertEqual",
            "beforeEach",
            "describe",
            "equal",
            "expect",
            "raises",
            "spyOn",
        ] {
            assert!(is_generic_call(call), "{call}");
        }
        assert!(!is_generic_call("validateToken"));
        assert!(has_test_harness_receiver("expect(result)."));
        assert!(has_test_harness_receiver("expect(result).not."));
        assert!(!has_test_harness_receiver("expect(result); client."));
    }

    #[test]
    fn targeted_evidence_requires_task_overlap() {
        let unrelated = SearchHit {
            file_path: PathBuf::from("src/search.rs"),
            start_line: 1,
            end_line: 1,
            preview: "fn process_payment() { hybrid_search(); }".to_string(),
            reason: String::new(),
            score: 1.0,
            sources: Vec::new(),
            neural_requested: false,
            neural_executed: false,
        };
        assert!(!hit_matches_task(
            &unrelated,
            "fix search_call_sites for mixed-case symbols"
        ));
    }

    #[test]
    fn relationship_snippets_center_the_symbol() {
        let hit = SearchHit {
            file_path: PathBuf::from("src/cli.rs"),
            start_line: 100,
            end_line: 105,
            preview: "fn run() {\n    prepare();\n    build_context_bundle();\n    finish();\n}\n"
                .to_string(),
            reason: String::new(),
            score: 1.0,
            sources: vec!["caller".to_string()],
            neural_requested: false,
            neural_executed: false,
        };
        let focused = focus_hit_on_symbol(hit, "build_context_bundle", 1, false);
        assert_eq!(focused.start_line, 101);
        assert_eq!(focused.end_line, 103);
        assert_eq!(
            focused.preview,
            "    prepare();\n    build_context_bundle();\n    finish();"
        );
    }

    #[test]
    fn truncation_centers_explicit_identifier_and_reports_offset() {
        let text = (0..20)
            .map(|index| {
                if index == 10 {
                    "fn search_call_sites() {}".to_string()
                } else {
                    format!("let unrelated_{index} = true;")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (preview, truncated, offset) =
            truncate_to_token_budget(&text, 40, "fix search_call_sites");
        assert!(truncated);
        assert!(preview.contains("search_call_sites"));
        assert!(offset > 0);
    }
}
