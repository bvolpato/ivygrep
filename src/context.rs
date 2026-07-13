use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
use crate::indexer::reconcile_worktree_overlay;
use crate::protocol::SearchHit;
use crate::search::{SearchContext, SearchOptions, hybrid_search_with_context};
use crate::symbols::{
    SymbolSearchMode, likely_definition_names, search_symbol_relationships_in_current_index,
    search_symbols_in_current_index,
};
use crate::workspace::Workspace;

const MAX_ITEMS: usize = 20;
const MAX_ANCHOR_SYMBOLS: usize = 3;
const RRF_K: f64 = 10.0;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRole {
    Primary,
    Definition,
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
    pub callers: usize,
    pub references: usize,
    pub tests: usize,
    pub config: usize,
    pub documentation: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextItem {
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
    pub budget_tokens: usize,
    pub used_tokens: usize,
    pub candidate_count: usize,
    pub truncated: bool,
    pub anchor_symbols: Vec<String>,
    pub coverage: ContextCoverage,
    pub items: Vec<ContextItem>,
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
    let fallback_model;
    let reconciliation_model = if let Some(model) = embedding_model {
        model
    } else {
        fallback_model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        &fallback_model
    };
    reconcile_worktree_overlay(workspace, reconciliation_model)?;

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

    let mut primary_hits = Vec::new();
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
            primary_hits = hits.clone();
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
            add_candidate(
                &mut candidates,
                hit,
                role,
                format!("rank {} for {retrieval_label} retrieval", rank + 1),
                weight / (RRF_K + rank as f64 + 1.0),
            );
        }
    }

    let anchor_symbols = anchor_symbols(task, &primary_hits);
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

    Ok(assemble_bundle(
        task,
        &workspace.root,
        budget_tokens,
        anchor_symbols,
        candidates.into_values().collect(),
    ))
}

fn focus_hit_on_symbol(
    mut hit: SearchHit,
    symbol: &str,
    context_lines: usize,
    prefer_last: bool,
) -> SearchHit {
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
    let end = (focus + context_lines + 1).min(lines.len());
    hit.start_line = hit.start_line.saturating_add(start);
    hit.end_line = hit.start_line.saturating_add(end.saturating_sub(start + 1));
    hit.preview = lines[start..end].join("\n");
    hit
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
    let overlap = overlap_end - overlap_start + 1;
    let shorter = (left_end - left_start + 1).min(right_end - right_start + 1);
    overlap.saturating_mul(2) >= shorter
}

fn assemble_bundle(
    task: &str,
    workspace: &Path,
    budget_tokens: usize,
    anchor_symbols: Vec<String>,
    mut candidates: Vec<Candidate>,
) -> ContextBundle {
    candidates.sort_by(|left, right| {
        right
            .fused_score
            .total_cmp(&left.fused_score)
            .then_with(|| left.hit.file_path.cmp(&right.hit.file_path))
            .then_with(|| left.hit.start_line.cmp(&right.hit.start_line))
    });
    let candidate_count = candidates.len();
    let mut ordered = Vec::with_capacity(candidates.len());
    let mut claimed = HashSet::new();
    for role in [
        ContextRole::Primary,
        ContextRole::Definition,
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
        task,
        workspace,
        budget_tokens,
        &anchor_symbols,
        candidate_count,
    );
    let mut file_counts = HashMap::<PathBuf, usize>::new();
    let mut truncated = false;
    for candidate in ordered {
        if items.len() == MAX_ITEMS || used_tokens >= budget_tokens {
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
        let remaining = budget_tokens.saturating_sub(used_tokens + wrapper_tokens);
        if remaining < 64 {
            truncated = true;
            break;
        }
        let per_item_budget = (budget_tokens / 4).clamp(96, 600).min(remaining);
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
        used_tokens += item.estimated_tokens;
        *file_counts.entry(item.file_path.clone()).or_default() += 1;
        items.push(item);
    }
    let mut bundle = ContextBundle {
        task: task.to_string(),
        workspace: workspace.to_path_buf(),
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

fn estimated_header_tokens(
    task: &str,
    workspace: &Path,
    budget_tokens: usize,
    anchor_symbols: &[String],
    candidate_count: usize,
) -> usize {
    let bundle = ContextBundle {
        task: task.to_string(),
        workspace: workspace.to_path_buf(),
        budget_tokens,
        used_tokens: budget_tokens,
        candidate_count,
        truncated: true,
        anchor_symbols: anchor_symbols.to_vec(),
        coverage: ContextCoverage {
            files: MAX_ITEMS,
            primary: MAX_ITEMS,
            definitions: MAX_ITEMS,
            callers: MAX_ITEMS,
            references: MAX_ITEMS,
            tests: MAX_ITEMS,
            config: MAX_ITEMS,
            documentation: MAX_ITEMS,
        },
        items: Vec::new(),
    };
    estimate_tokens(&render_markdown(&bundle)) + 8
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
        return explicit_symbols
            .into_iter()
            .take(MAX_ANCHOR_SYMBOLS)
            .collect::<Vec<_>>();
    }

    let mut scored = BTreeMap::<String, usize>::new();
    let mut fallback = None;
    let mut task_terms = significant_task_terms(task);
    if task_terms
        .iter()
        .any(|term| term == "pack" || term == "packs")
    {
        task_terms.push("bundle".to_string());
    }
    for (rank, hit) in primary_hits.iter().take(5).enumerate() {
        let file_role = classify_file_role(hit);
        let mut names = if file_role == Some(ContextRole::Test) {
            likely_called_names(&hit.preview)
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
                fallback.get_or_insert_with(|| symbol.clone());
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
        && let Some(symbol) = fallback
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
            if name.len() >= 3 && !is_definition && !is_generic_call(name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn is_generic_call(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "eprint"
            | "eprintln"
            | "format"
            | "if"
            | "match"
            | "ok"
            | "print"
            | "println"
            | "some"
            | "vec"
    )
}

fn task_symbols(task: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    task.split(|character: char| {
        !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '$'
            && character != ':'
            && character != '.'
    })
    .map(|part| part.trim_matches([':', '.', '$']))
    .filter(|part| part.len() >= 3)
    .filter(|part| {
        part.contains('_')
            || part.contains("::")
            || part.contains('.') && !looks_like_file_name(part)
            || looks_like_mixed_case_identifier(part)
    })
    .filter(|part| !is_generic_symbol(part))
    .filter(|part| seen.insert(part.to_ascii_lowercase()))
    .map(ToOwned::to_owned)
    .collect()
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
        Other,
    }
    let mut total = 0usize;
    let mut class = None;
    let mut run = 0usize;
    let flush = |class: Option<Class>, run: usize| match class {
        Some(Class::Word) => run.div_ceil(4),
        Some(Class::Space) => usize::from(run > 0),
        Some(Class::Newline | Class::Other) => run,
        None => 0,
    };
    for character in text.chars() {
        let next = if character == '\n' {
            Class::Newline
        } else if character.is_whitespace() {
            Class::Space
        } else if character.is_ascii_alphanumeric() || character == '_' {
            Class::Word
        } else {
            Class::Other
        };
        if class == Some(next) {
            run += 1;
        } else {
            total += flush(class, run);
            class = Some(next);
            run = 1;
        }
    }
    total + flush(class, run)
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
    let mut end = (focus + 1).min(lines.len());
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
        "Coverage: {} files | {} primary | {} definitions | {} callers | {} references | {} tests | {} config | {} docs\n",
        bundle.coverage.files,
        bundle.coverage.primary,
        bundle.coverage.definitions,
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
        output.push_str(&render_markdown_item(index + 1, item));
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
        item.file_path.display(),
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
        assert_eq!(estimate_tokens("calculate_tax"), 4);
        assert_eq!(estimate_tokens("fn x() {\n  x();\n}"), 15);
        assert_eq!(
            estimate_tokens("fn x() {\n  x();\n}"),
            estimate_tokens("fn x() {\n  x();\n}")
        );
        assert_eq!(truncate_to_token_budget("", 100, "task").0, "");
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
    fn rendered_pack_never_exceeds_requested_budget() {
        for budget in [256, 384, 800, 4_000] {
            let candidates = (0..30)
                .map(|index| {
                    candidate(
                        &format!("src/module_{index}.rs"),
                        1,
                        ContextRole::Primary,
                        &format!(
                            "pub fn implementation_{index}() {{\n{}\n}}",
                            "    execute_work();\n".repeat(80)
                        ),
                        1.0 / (index + 1) as f64,
                    )
                })
                .collect();
            let bundle = assemble_bundle(
                "change implementation behavior",
                Path::new("/repo"),
                budget,
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
    fn overlapping_snippets_are_not_repeated() {
        let candidates = vec![
            candidate("src/lib.rs", 10, ContextRole::Primary, "a\nb\nc\nd", 1.0),
            candidate("src/lib.rs", 11, ContextRole::Related, "b\nc\nd\ne", 0.9),
        ];
        let bundle = assemble_bundle("task", Path::new("/repo"), 500, Vec::new(), candidates);
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
