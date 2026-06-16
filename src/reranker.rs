use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::protocol::SearchHit;

const FEATURE_SCHEMA: &[&str] = &[
    "log_total_score",
    "reciprocal_rank",
    "hit_count",
    "source_count",
    "source_lexical",
    "source_semantic",
    "source_literal",
    "source_path",
    "source_symbol",
    "query_preview_coverage",
    "query_path_coverage",
    "exact_query_preview",
    "exact_query_path",
    "support_path",
    "primary_source",
    "shallow_path",
    "query_length",
    "preview_length",
    "lexical_semantic",
    "literal_exact",
    "semantic_only",
    "score_preview_coverage",
    "rank_preview_coverage",
    "short_preview_coverage",
    "medium_preview_coverage",
    "long_preview_coverage",
    "short_semantic",
    "long_semantic",
    "short_literal",
    "long_literal",
    "preview_term_precision",
    "preview_term_f1",
    "weighted_preview_coverage",
    "informative_preview_coverage",
    "long_term_preview_coverage",
    "numeric_preview_coverage",
    "query_bigram_preview_coverage",
    "query_line_preview_coverage",
    "path_term_f1",
    "natural_language_preview_f1",
    "code_query_line_coverage",
];

const CODE_EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "cs", "go", "h", "hpp", "java", "js", "jsx", "kt", "kts", "php", "py", "rb",
    "rs", "scala", "swift", "ts", "tsx",
];

const UNINFORMATIVE_TERMS: &[&str] = &[
    "and", "are", "can", "class", "const", "def", "else", "false", "find", "fix", "for", "from",
    "function", "how", "import", "include", "int", "let", "new", "not", "null", "return", "should",
    "static", "string", "struct", "the", "this", "true", "use", "using", "value", "var", "void",
    "what", "when", "where", "which", "with",
];

const NATURAL_LANGUAGE_TERMS: &[&str] = &[
    "a",
    "an",
    "and",
    "appropriate",
    "can",
    "error",
    "find",
    "fix",
    "for",
    "following",
    "how",
    "in",
    "is",
    "of",
    "please",
    "should",
    "suggest",
    "the",
    "this",
    "to",
    "value",
    "what",
    "when",
    "where",
    "which",
    "with",
];

#[derive(Debug, Deserialize)]
struct LearnedModel {
    schema_version: u32,
    model_id: String,
    feature_schema: Vec<String>,
    weights: Vec<f32>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeStatus {
    pub mode: String,
    pub model_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    Learned,
    Deterministic,
}

#[derive(Debug)]
struct FileCandidate {
    path: PathBuf,
    hit_indices: Vec<usize>,
    total_score: f32,
    hit_count: usize,
    sources: HashSet<String>,
    preview: String,
    baseline_rank: usize,
    learned_score: f32,
    target_score: f32,
}

static MODEL: OnceLock<Result<LearnedModel, String>> = OnceLock::new();

fn load_model() -> &'static Result<LearnedModel, String> {
    MODEL.get_or_init(|| {
        let model: LearnedModel =
            serde_json::from_str(include_str!("../benchmarks/public/reranker_model.json"))
                .map_err(|error| format!("embedded reranker model is invalid: {error}"))?;
        if model.schema_version != 2 {
            return Err(format!(
                "unsupported reranker schema version {}",
                model.schema_version
            ));
        }
        if model.feature_schema.len() != FEATURE_SCHEMA.len()
            || !model
                .feature_schema
                .iter()
                .zip(FEATURE_SCHEMA)
                .all(|(actual, expected)| actual == expected)
        {
            return Err("embedded reranker feature schema does not match this binary".to_string());
        }
        if model.weights.len() != FEATURE_SCHEMA.len()
            || model.weights.iter().any(|weight| !weight.is_finite())
        {
            return Err("embedded reranker weights are invalid".to_string());
        }
        Ok(model)
    })
}

fn configured_mode() -> (Mode, Option<String>) {
    match std::env::var("IVYGREP_RERANKER") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "deterministic" | "disabled" | "off" => (Mode::Deterministic, None),
            "learned" | "auto" | "" => (Mode::Learned, None),
            _ => (
                Mode::Learned,
                Some(format!(
                    "unknown IVYGREP_RERANKER={value:?}; using learned mode"
                )),
            ),
        },
        Err(_) => (Mode::Learned, None),
    }
}

pub(crate) fn runtime_status() -> RuntimeStatus {
    let (mode, mut error) = configured_mode();
    if mode == Mode::Deterministic {
        return RuntimeStatus {
            mode: "deterministic".to_string(),
            model_id: None,
            error,
        };
    }
    match load_model() {
        Ok(model) => RuntimeStatus {
            mode: "learned".to_string(),
            model_id: Some(model.model_id.clone()),
            error,
        },
        Err(model_error) => {
            error = Some(match error {
                Some(config_error) => format!("{config_error}; {model_error}"),
                None => model_error.clone(),
            });
            RuntimeStatus {
                mode: "deterministic".to_string(),
                model_id: None,
                error,
            }
        }
    }
}

pub(crate) fn cache_identity() -> String {
    let status = runtime_status();
    match status.model_id {
        Some(model_id) => format!("{}:{model_id}", status.mode),
        None => status.mode,
    }
}

pub(crate) fn rerank_hits(query: &str, hits: &mut [SearchHit]) {
    if configured_mode().0 == Mode::Deterministic {
        return;
    }
    let file_count = hits
        .iter()
        .map(|hit| &hit.file_path)
        .collect::<HashSet<_>>()
        .len();
    if file_count < 5 {
        return;
    }
    let Ok(model) = load_model() else {
        return;
    };
    rerank_hits_with_model(query, hits, model);
}

fn rerank_hits_with_model(query: &str, hits: &mut [SearchHit], model: &LearnedModel) {
    let mut grouped = HashMap::<PathBuf, FileCandidate>::new();
    for (index, hit) in hits.iter().enumerate() {
        let entry = grouped
            .entry(hit.file_path.clone())
            .or_insert_with(|| FileCandidate {
                path: hit.file_path.clone(),
                hit_indices: Vec::new(),
                total_score: 0.0,
                hit_count: 0,
                sources: HashSet::new(),
                preview: String::new(),
                baseline_rank: 0,
                learned_score: 0.0,
                target_score: 0.0,
            });
        entry.hit_indices.push(index);
        entry.total_score += hit.score;
        entry.hit_count += 1;
        entry.sources.extend(hit.sources.iter().cloned());
        if entry.hit_indices.len() <= 3 {
            if !entry.preview.is_empty() {
                entry.preview.push('\n');
            }
            entry.preview.push_str(&hit.preview);
            entry.preview.truncate(12_000);
        }
    }

    let mut files = grouped.into_values().collect::<Vec<_>>();
    files.sort_by(|left, right| {
        right
            .total_score
            .total_cmp(&left.total_score)
            .then_with(|| left.path.cmp(&right.path))
    });

    for (rank, candidate) in files.iter_mut().enumerate() {
        candidate.baseline_rank = rank;
        let features = feature_vector(query, candidate);
        candidate.learned_score = model
            .weights
            .iter()
            .zip(features)
            .map(|(weight, feature)| weight * feature)
            .sum();
    }

    let baseline_scores = files
        .iter()
        .map(|candidate| candidate.total_score)
        .collect::<Vec<_>>();
    let mut learned_order = (0..files.len()).collect::<Vec<_>>();
    learned_order.sort_by(|left, right| {
        files[*right]
            .learned_score
            .total_cmp(&files[*left].learned_score)
            .then_with(|| files[*left].baseline_rank.cmp(&files[*right].baseline_rank))
    });
    for (rank, index) in learned_order.into_iter().enumerate() {
        files[index].target_score = baseline_scores[rank];
    }

    for candidate in files {
        if candidate.total_score > f32::EPSILON {
            let scale = candidate.target_score / candidate.total_score;
            for index in candidate.hit_indices {
                hits[index].score *= scale;
            }
        } else if let Some((first, rest)) = candidate.hit_indices.split_first() {
            hits[*first].score = candidate.target_score;
            for index in rest {
                hits[*index].score = 0.0;
            }
        }
    }

    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
}

fn feature_vector(query: &str, candidate: &FileCandidate) -> Vec<f32> {
    let path = candidate.path.to_string_lossy().replace('\\', "/");
    let path_lower = path.to_ascii_lowercase();
    let preview_lower = candidate.preview.to_ascii_lowercase();
    let terms = query_terms(query);
    let preview_terms = query_terms(&candidate.preview);
    let path_terms = query_terms(&path);
    let sources = &candidate.sources;
    let query_lower = query.trim().to_ascii_lowercase();
    let extension = Path::new(&path_lower)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let support = is_support_path(&path_lower);
    let lexical = sources.contains("lexical");
    let semantic = sources.contains("semantic");
    let literal = sources.contains("literal");
    let exact_preview = !query_lower.is_empty() && preview_lower.contains(&query_lower);
    let exact_path = !query_lower.is_empty() && path_lower.contains(&query_lower);
    let preview_coverage = coverage(&terms, &preview_lower);
    let (_, preview_precision, preview_f1) = set_overlap(&terms, &preview_terms);
    let (_, _, path_f1) = set_overlap(&terms, &path_terms);
    let informative_terms = terms
        .iter()
        .filter(|term| term.len() >= 4 && !UNINFORMATIVE_TERMS.contains(&term.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let long_terms = terms
        .iter()
        .filter(|term| term.len() >= 7)
        .cloned()
        .collect::<Vec<_>>();
    let numeric_terms = terms
        .iter()
        .filter(|term| term.chars().all(|character| character.is_ascii_digit()))
        .cloned()
        .collect::<Vec<_>>();
    let exact_line_coverage = line_coverage(query, &candidate.preview);
    let (natural_language, code_query) = query_shape(&terms, query);
    let score = candidate.total_score.max(0.0).ln_1p().min(4.0) / 4.0;
    let reciprocal_rank = 1.0 / (candidate.baseline_rank as f32 + 1.0);
    let short_query = terms.len() <= 5;
    let long_query = terms.len() >= 13;
    let medium_query = !short_query && !long_query;

    vec![
        score,
        reciprocal_rank,
        candidate.hit_count.min(4) as f32 / 4.0,
        sources.len().min(5) as f32 / 5.0,
        lexical as u8 as f32,
        semantic as u8 as f32,
        literal as u8 as f32,
        sources.contains("path") as u8 as f32,
        sources.contains("symbol") as u8 as f32,
        preview_coverage,
        coverage(&terms, &path_lower),
        exact_preview as u8 as f32,
        exact_path as u8 as f32,
        support as u8 as f32,
        (CODE_EXTENSIONS.contains(&extension) && !support) as u8 as f32,
        1.0 / (1.0 + path_lower.matches('/').count() as f32),
        terms.len().min(20) as f32 / 20.0,
        candidate.preview.chars().count().min(12_000) as f32 / 12_000.0,
        (lexical && semantic) as u8 as f32,
        (literal && (exact_preview || exact_path)) as u8 as f32,
        (semantic
            && !(lexical || literal || sources.contains("path") || sources.contains("symbol")))
            as u8 as f32,
        score * preview_coverage,
        reciprocal_rank * preview_coverage,
        short_query as u8 as f32 * preview_coverage,
        medium_query as u8 as f32 * preview_coverage,
        long_query as u8 as f32 * preview_coverage,
        (short_query && semantic) as u8 as f32,
        (long_query && semantic) as u8 as f32,
        (short_query && literal) as u8 as f32,
        (long_query && literal) as u8 as f32,
        preview_precision,
        preview_f1,
        weighted_coverage(&terms, &preview_terms),
        coverage(&informative_terms, &preview_lower),
        coverage(&long_terms, &preview_lower),
        coverage(&numeric_terms, &preview_lower),
        bigram_coverage(&terms, &preview_terms),
        exact_line_coverage,
        path_f1,
        natural_language * preview_f1,
        code_query * exact_line_coverage,
    ]
}

fn query_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in text.to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            if current.len() >= 2 && !terms.contains(&current) {
                terms.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() >= 2 && !terms.contains(&current) {
        terms.push(current);
    }
    terms
}

fn coverage(terms: &[String], text: &str) -> f32 {
    if terms.is_empty() {
        return 0.0;
    }
    terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count() as f32
        / terms.len() as f32
}

fn set_overlap(left: &[String], right: &[String]) -> (f32, f32, f32) {
    let left = left.iter().map(String::as_str).collect::<HashSet<_>>();
    let right = right.iter().map(String::as_str).collect::<HashSet<_>>();
    if left.is_empty() || right.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let overlap = left.intersection(&right).count() as f32;
    let recall = overlap / left.len() as f32;
    let precision = overlap / right.len() as f32;
    let f1 = if recall + precision > 0.0 {
        2.0 * recall * precision / (recall + precision)
    } else {
        0.0
    };
    (recall, precision, f1)
}

fn weighted_coverage(terms: &[String], text_terms: &[String]) -> f32 {
    if terms.is_empty() {
        return 0.0;
    }
    let present = text_terms
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let total = terms.iter().map(|term| term.len().min(16)).sum::<usize>();
    let matched = terms
        .iter()
        .filter(|term| present.contains(term.as_str()))
        .map(|term| term.len().min(16))
        .sum::<usize>();
    matched as f32 / total as f32
}

fn bigram_coverage(query: &[String], text: &[String]) -> f32 {
    let query_bigrams = query
        .windows(2)
        .map(|pair| (pair[0].as_str(), pair[1].as_str()))
        .collect::<HashSet<_>>();
    if query_bigrams.is_empty() {
        return 0.0;
    }
    let text_bigrams = text
        .windows(2)
        .map(|pair| (pair[0].as_str(), pair[1].as_str()))
        .collect::<HashSet<_>>();
    query_bigrams.intersection(&text_bigrams).count() as f32 / query_bigrams.len() as f32
}

fn line_coverage(query: &str, text: &str) -> f32 {
    let lines = query
        .lines()
        .map(normalize_whitespace)
        .filter(|line| line.len() >= 8)
        .collect::<HashSet<_>>();
    if lines.is_empty() {
        return 0.0;
    }
    let normalized_text = normalize_whitespace(text);
    lines
        .iter()
        .filter(|line| normalized_text.contains(line.as_str()))
        .count() as f32
        / lines.len() as f32
}

fn normalize_whitespace(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn query_shape(terms: &[String], query: &str) -> (f32, f32) {
    if terms.is_empty() {
        return (0.0, 0.0);
    }
    let natural_language = (terms
        .iter()
        .filter(|term| NATURAL_LANGUAGE_TERMS.contains(&term.as_str()))
        .count() as f32
        / (terms.len() as f32 / 2.0).max(3.0))
    .min(1.0);
    let punctuation = query
        .chars()
        .filter(|character| "{}[]();=<>:+-*/".contains(*character))
        .count() as f32;
    let code = (punctuation / (query.chars().count() as f32 / 40.0).max(4.0)).min(1.0);
    (natural_language, code)
}

fn is_support_path(path: &str) -> bool {
    path.split('/').any(|segment| {
        matches!(
            segment,
            "tools"
                | "tooling"
                | "scripts"
                | "script"
                | "examples"
                | "example"
                | "samples"
                | "sample"
                | "demos"
                | "demo"
                | "bench"
                | "benches"
                | "benchmarks"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, score: f32, preview: &str, sources: &[&str]) -> SearchHit {
        SearchHit {
            file_path: PathBuf::from(path),
            start_line: 1,
            end_line: 1,
            preview: preview.to_string(),
            reason: String::new(),
            score,
            sources: sources.iter().map(|source| (*source).to_string()).collect(),
        }
    }

    #[test]
    fn embedded_model_matches_feature_schema() {
        let model = load_model().as_ref().expect("model should load");
        assert_eq!(model.weights.len(), FEATURE_SCHEMA.len());
    }

    #[test]
    fn rust_features_match_public_trainer_fixture() {
        let candidate = FileCandidate {
            path: PathBuf::from("src/search.rs"),
            hit_indices: vec![0, 1],
            total_score: 0.5,
            hit_count: 2,
            sources: ["lexical".to_string(), "semantic".to_string()]
                .into_iter()
                .collect(),
            preview: "fn route_query() { learned_rerank(); }".to_string(),
            baseline_rank: 0,
            learned_score: 0.0,
            target_score: 0.0,
        };
        let actual = feature_vector("route learned query", &candidate);
        let expected = [
            0.10136628,
            1.0,
            0.5,
            0.4,
            1.0,
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.5,
            0.15,
            0.0031666667,
            1.0,
            0.0,
            0.0,
            0.10136628,
            1.0,
            1.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.6,
            0.75,
            1.0,
            1.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() < 1e-6,
                "feature {} ({}) differs: {actual} != {expected}",
                index,
                FEATURE_SCHEMA[index],
            );
        }
    }

    #[test]
    fn learned_reranker_keeps_scores_finite_and_positive() {
        let mut hits = vec![
            hit("src/a.rs", 2.0, "fn unrelated() {}", &["lexical"]),
            hit(
                "src/b.rs",
                1.0,
                "fn route_query() { learned_rerank(); }",
                &["lexical", "semantic"],
            ),
        ];
        let model = load_model().as_ref().expect("model should load");
        rerank_hits_with_model("route learned query", &mut hits, model);
        assert!(
            hits.iter()
                .all(|hit| hit.score.is_finite() && hit.score > 0.0)
        );
    }

    #[test]
    fn learned_reranker_latency_stays_bounded() {
        let model = load_model().as_ref().expect("model should load");
        let template = (0..100)
            .map(|index| {
                hit(
                    &format!("src/module_{index}.rs"),
                    2.0 - index as f32 / 100.0,
                    "fn route_query() { learned_rerank(); semantic_search(); }",
                    &["lexical", "semantic"],
                )
            })
            .collect::<Vec<_>>();
        let started = std::time::Instant::now();
        for _ in 0..100 {
            let mut hits = template.clone();
            rerank_hits_with_model("route learned semantic query", &mut hits, model);
        }
        let average = started.elapsed() / 100;
        assert!(
            average < std::time::Duration::from_millis(75),
            "average reranker latency exceeded budget: {average:?}"
        );
    }
}
