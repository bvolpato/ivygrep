use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QueryIntent {
    ExactIdentifier,
    Path,
    LiteralOrError,
    NaturalLanguage,
    DocsTestsExamples,
    Mixed,
}

impl QueryIntent {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ExactIdentifier => "exact-identifier",
            Self::Path => "path-file",
            Self::LiteralOrError => "literal-error",
            Self::NaturalLanguage => "natural-language-implementation",
            Self::DocsTestsExamples => "docs-tests-examples",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueryRouting {
    pub(crate) intent: QueryIntent,
    pub(crate) use_neural: bool,
    pub(crate) lexical_multiplier: usize,
    pub(crate) literal_multiplier: usize,
    pub(crate) semantic_multiplier: usize,
    pub(crate) symbol_limit: usize,
}

impl QueryRouting {
    pub(crate) fn classify(query: &str) -> Self {
        let trimmed = query.trim();
        let terms = raw_query_terms(trimmed);
        let lower = trimmed.to_ascii_lowercase();
        let concise_path_query = !trimmed.contains('\n') && terms.len() <= 8;
        let has_path_shape = concise_path_query
            && (trimmed.contains('/')
                || trimmed.contains('\\')
                || trimmed.split_whitespace().any(|term| {
                    Path::new(term.trim_matches(|ch: char| {
                        matches!(ch, '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';')
                    }))
                    .extension()
                    .is_some_and(|extension| {
                        let length = extension.to_string_lossy().len();
                        (1..=8).contains(&length)
                    })
                }));
        let has_literal_shape = trimmed.contains('\n')
            || trimmed.contains('"')
            || trimmed.contains('\'')
            || lower.contains("error:")
            || lower.contains("exception")
            || lower.contains("traceback")
            || lower.contains("failed to");
        let targets_support = terms.iter().any(|term| {
            matches!(
                term.as_str(),
                "doc"
                    | "docs"
                    | "documentation"
                    | "readme"
                    | "test"
                    | "tests"
                    | "testing"
                    | "example"
                    | "examples"
                    | "sample"
                    | "samples"
            )
        });
        let exact_identifier = !trimmed.is_empty()
            && !trimmed.contains(char::is_whitespace)
            && trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.' | '$'));

        let intent = if has_path_shape {
            QueryIntent::Path
        } else if has_literal_shape {
            QueryIntent::LiteralOrError
        } else if exact_identifier {
            QueryIntent::ExactIdentifier
        } else if targets_support {
            QueryIntent::DocsTestsExamples
        } else if terms.len() >= 13 {
            QueryIntent::NaturalLanguage
        } else {
            QueryIntent::Mixed
        };
        match intent {
            QueryIntent::ExactIdentifier => Self {
                intent,
                use_neural: false,
                lexical_multiplier: 8,
                literal_multiplier: 6,
                semantic_multiplier: 1,
                symbol_limit: 100,
            },
            QueryIntent::Path => Self {
                intent,
                use_neural: false,
                lexical_multiplier: 8,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::LiteralOrError => Self {
                intent,
                // Large literals are often pasted code or detailed prompts.
                // Keep exact retrieval dominant while retaining semantic recall.
                use_neural: terms.len() >= 13,
                lexical_multiplier: 10,
                literal_multiplier: 8,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::NaturalLanguage => Self {
                intent,
                use_neural: true,
                lexical_multiplier: 5,
                literal_multiplier: 4,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::DocsTestsExamples => Self {
                intent,
                use_neural: true,
                lexical_multiplier: 5,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 50,
            },
            QueryIntent::Mixed => Self {
                intent,
                use_neural: true,
                lexical_multiplier: 5,
                literal_multiplier: 5,
                semantic_multiplier: 1,
                symbol_limit: 100,
            },
        }
    }
}

pub(crate) fn corpus_candidate_multiplier(document_count: u64) -> usize {
    match document_count {
        0..=50_000 => 1,
        50_001..=500_000 => 2,
        _ => 3,
    }
}

pub(crate) fn neural_fallback_needed(
    routing: QueryRouting,
    force_neural: bool,
    top_score: Option<f32>,
    runner_up_score: Option<f32>,
) -> bool {
    const SCORE_THRESHOLD: f32 = 2.0;
    const SCORE_GAP_THRESHOLD: f32 = 0.25;

    if force_neural {
        return true;
    }
    if !routing.use_neural {
        return false;
    }

    let Some(top_score) = top_score else {
        return true;
    };
    let score_gap = runner_up_score.map_or(top_score, |score| top_score - score);
    top_score < SCORE_THRESHOLD || score_gap < SCORE_GAP_THRESHOLD
}

pub(crate) fn raw_query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}
