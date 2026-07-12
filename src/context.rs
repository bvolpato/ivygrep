use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::embedding::{EmbeddingModel, HashEmbeddingModel};
use crate::indexer::reconcile_worktree_overlay;
use crate::protocol::SearchHit;
use crate::search::{SearchContext, SearchOptions, hybrid_search_with_context};
use crate::symbols::{SymbolSearchMode, likely_definition_names, search_symbols_with_options};
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
    Test,
    Support,
    Related,
}

impl ContextRole {
    fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Definition => "definition",
            Self::Caller => "caller",
            Self::Test => "test",
            Self::Support => "support",
            Self::Related => "related",
        }
    }
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
        (task.to_string(), ContextRole::Primary, 1.0, 14usize),
        (format!("test {task}"), ContextRole::Test, 0.72, 8),
        (
            format!("documentation example {task}"),
            ContextRole::Support,
            0.58,
            8,
        ),
    ];

    let mut primary_hits = Vec::new();
    for (query, requested_role, weight, context_lines) in query_specs {
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
            if requested_role != ContextRole::Primary
                && (!hit_matches_task(&hit, task)
                    || classify_role(&hit, requested_role) != requested_role)
            {
                continue;
            }
            let role = classify_role(&hit, requested_role);
            add_candidate(
                &mut candidates,
                hit,
                role,
                format!("rank {} for {} retrieval", rank + 1, requested_role.label()),
                weight / (RRF_K + rank as f64 + 1.0),
            );
        }
    }

    let anchor_symbols = anchor_symbols(task, &primary_hits);
    let mut symbol_options = base_options.clone();
    symbol_options.limit = Some(4);
    symbol_options.context = 10;
    for symbol in &anchor_symbols {
        for (mode, role, weight, verb) in [
            (
                SymbolSearchMode::Definitions,
                ContextRole::Definition,
                0.82,
                "defines",
            ),
            (
                SymbolSearchMode::Callers,
                ContextRole::Caller,
                0.76,
                "calls",
            ),
        ] {
            match search_symbols_with_options(workspace, symbol, mode, &symbol_options) {
                Ok(hits) => {
                    for (rank, hit) in hits.into_iter().enumerate() {
                        let hit = focus_hit_on_symbol(
                            hit,
                            symbol,
                            symbol_options.context,
                            mode == SymbolSearchMode::Callers,
                        );
                        add_candidate(
                            &mut candidates,
                            hit,
                            role,
                            format!("{verb} {symbol}"),
                            weight / (RRF_K + rank as f64 + 1.0),
                        );
                    }
                }
                Err(error) => tracing::debug!("context relationship expansion failed: {error:#}"),
            }
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
        hit,
        roles: BTreeSet::new(),
        reasons: BTreeSet::new(),
        fused_score: 0.0,
    });
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
        ContextRole::Caller,
        ContextRole::Test,
        ContextRole::Definition,
        ContextRole::Support,
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
    let mut used_tokens = 0usize;
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
        let metadata_tokens = estimate_tokens(&candidate.hit.file_path.to_string_lossy())
            + roles.len().saturating_mul(2)
            + reasons
                .iter()
                .map(|reason| estimate_tokens(reason))
                .sum::<usize>()
            + 12;
        let remaining = budget_tokens.saturating_sub(used_tokens + metadata_tokens);
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
        let estimated_tokens = metadata_tokens + estimate_tokens(&preview);
        if preview.trim().is_empty() || estimated_tokens > budget_tokens.saturating_sub(used_tokens)
        {
            continue;
        }
        truncated |= preview_truncated;
        used_tokens += estimated_tokens;
        *file_counts
            .entry(candidate.hit.file_path.clone())
            .or_default() += 1;
        let start_line = candidate.hit.start_line.saturating_add(start_offset);
        let end_line = start_line.saturating_add(preview.lines().count().saturating_sub(1));
        items.push(ContextItem {
            file_path: candidate.hit.file_path,
            start_line,
            end_line,
            roles,
            reasons,
            sources: candidate.hit.sources,
            preview,
            estimated_tokens,
        });
    }
    ContextBundle {
        task: task.to_string(),
        workspace: workspace.to_path_buf(),
        budget_tokens,
        used_tokens,
        candidate_count,
        truncated,
        anchor_symbols,
        items,
    }
}

fn classify_role(hit: &SearchHit, requested: ContextRole) -> ContextRole {
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
        return ContextRole::Test;
    }
    let is_support = normalized
        .split('/')
        .any(|part| matches!(part, "docs" | "doc" | "examples" | "example" | "config"))
        || matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("md" | "toml" | "yaml" | "yml" | "json")
        );
    if is_support {
        return ContextRole::Support;
    }
    match requested {
        ContextRole::Test | ContextRole::Support => ContextRole::Related,
        role => role,
    }
}

fn anchor_symbols(task: &str, primary_hits: &[SearchHit]) -> Vec<String> {
    let mut scored = BTreeMap::<String, usize>::new();
    let explicit_symbols = task_symbols(task);
    for symbol in &explicit_symbols {
        scored.insert(symbol.clone(), 20);
    }
    if !explicit_symbols.is_empty() {
        return scored
            .into_keys()
            .take(MAX_ANCHOR_SYMBOLS)
            .collect::<Vec<_>>();
    }
    let task_terms = significant_task_terms(task);
    let mut fallback = None;
    for (rank, hit) in primary_hits.iter().take(3).enumerate() {
        for symbol in likely_definition_names(&hit.preview) {
            if symbol.len() >= 3 && !is_generic_symbol(&symbol) {
                fallback.get_or_insert_with(|| symbol.clone());
                let overlap = crate::text::split_identifier_segments(&symbol)
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
                    *scored.entry(symbol).or_default() +=
                        overlap.saturating_mul(8) + 3usize.saturating_sub(rank);
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
    symbols
        .into_iter()
        .map(|(symbol, _)| symbol)
        .take(1)
        .collect()
}

fn task_symbols(task: &str) -> Vec<String> {
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
            || part.contains('.')
            || part
                .chars()
                .skip(1)
                .any(|character| character.is_ascii_uppercase())
    })
    .filter(|part| !is_generic_symbol(part))
    .map(ToOwned::to_owned)
    .collect()
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
    output.push_str("\n## Evidence\n");
    for (index, item) in bundle.items.iter().enumerate() {
        let roles = item
            .roles
            .iter()
            .map(|role| role.label())
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "\n### {}. {}:{}-{} [{}]\n\n",
            index + 1,
            item.file_path.display(),
            item.start_line,
            item.end_line,
            roles
        ));
        if !item.reasons.is_empty() {
            output.push_str(&format!("Why: {}.\n\n", item.reasons.join("; ")));
        }
        let language = language_fence(&item.file_path);
        output.push_str(&format!("```{language}\n{}\n```\n", item.preview));
    }
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
            180,
            vec!["validate_token".to_string()],
            candidates,
        );
        assert!(bundle.used_tokens <= bundle.budget_tokens);
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
    }

    #[test]
    fn inferred_anchors_prefer_task_overlap() {
        let primary = vec![SearchHit {
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
        }];
        assert_eq!(
            anchor_symbols("add token budgeted context packs", &primary),
            ["build_context_bundle"]
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
