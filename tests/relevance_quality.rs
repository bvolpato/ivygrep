use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{enhance_workspace_hash, index_workspace};
use ivygrep::protocol::SearchHit;
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;
use serial_test::serial;

#[derive(Debug)]
struct Judgment {
    path: &'static str,
    grade: u8,
}

#[derive(Debug)]
struct QueryCase {
    query: &'static str,
    judgments: Vec<Judgment>,
    forbidden_top3: Vec<&'static str>,
}

#[test]
#[serial]
fn labeled_relevance_suite_meets_quality_bar() {
    let (_tmp, workspace) = stage_relevance_corpus();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();

    let cases = vec![
        QueryCase {
            query: "where is tax calculated",
            judgments: vec![
                Judgment {
                    path: "src/payments/tax.rs",
                    grade: 3,
                },
                Judgment {
                    path: "src/payments/discount.rs",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["vendor/", "fixtures/", "data/"],
        },
        QueryCase {
            query: "refresh session token",
            judgments: vec![
                Judgment {
                    path: "src/auth/session.rs",
                    grade: 3,
                },
                Judgment {
                    path: "tests/auth_session_test.rs",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["vendor/", "fixtures/", "data/"],
        },
        QueryCase {
            query: "query parser grammar",
            judgments: vec![
                Judgment {
                    path: "src/search/query_parser.rs",
                    grade: 3,
                },
                Judgment {
                    path: "docs/search.md",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["fixtures/", "data/"],
        },
        QueryCase {
            query: "ranking score filter",
            judgments: vec![
                Judgment {
                    path: "src/search/ranking.rs",
                    grade: 3,
                },
                Judgment {
                    path: "docs/search.md",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["fixtures/", "vendor/", "data/"],
        },
        QueryCase {
            query: "binary file detection",
            judgments: vec![
                Judgment {
                    path: "src/io/binary_detector.rs",
                    grade: 3,
                },
                Judgment {
                    path: "tests/binary_detector_test.rs",
                    grade: 1,
                },
                Judgment {
                    path: "docs/binary-search.md",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["fixtures/", "vendor/", "data/"],
        },
        QueryCase {
            query: "password hashing",
            judgments: vec![Judgment {
                path: "src/auth/password.rs",
                grade: 3,
            }],
            forbidden_top3: vec!["fixtures/", "vendor/", "data/"],
        },
        QueryCase {
            query: "http route dispatch",
            judgments: vec![Judgment {
                path: "src/http/router.rs",
                grade: 3,
            }],
            forbidden_top3: vec!["fixtures/", "vendor/", "data/"],
        },
        QueryCase {
            query: "environment config loader",
            judgments: vec![Judgment {
                path: "src/config/env.rs",
                grade: 3,
            }],
            forbidden_top3: vec!["fixtures/", "vendor/", "data/"],
        },
        QueryCase {
            query: "background job scheduler",
            judgments: vec![
                Judgment {
                    path: "src/jobs/scheduler.rs",
                    grade: 3,
                },
                Judgment {
                    path: "examples/job_scheduler_demo.rs",
                    grade: 1,
                },
                Judgment {
                    path: "tools/job_scheduler_probe.rs",
                    grade: 1,
                },
            ],
            forbidden_top3: vec!["examples/", "tools/", "fixtures/"],
        },
    ];

    let mut diagnostics = Vec::new();
    let mut total_mrr = 0.0;
    let mut total_ndcg = 0.0;
    let mut total_precision = 0.0;
    let mut top_relevant = 0usize;
    let mut forbidden_top3 = 0usize;

    for case in &cases {
        let hits = hybrid_search(
            &workspace,
            case.query,
            Some(&model),
            &SearchOptions {
                limit: Some(10),
                context: 1,
                ..SearchOptions::default()
            },
        )
        .unwrap();
        assert!(!hits.is_empty(), "query {:?} returned no hits", case.query);

        let grades = grade_by_path(&case.judgments);
        let ranked_paths = hit_paths(&hits);
        let ranked_grades = graded_unique_file_results(&ranked_paths, &grades);

        let top_grade = ranked_grades.first().copied().unwrap_or_default();
        if top_grade >= 2 {
            top_relevant += 1;
        }

        let first_relevant_rank = ranked_grades
            .iter()
            .position(|grade| *grade >= 2)
            .map(|idx| idx + 1);
        if let Some(rank) = first_relevant_rank {
            total_mrr += 1.0 / rank as f64;
        }

        total_ndcg += ndcg_at(&ranked_grades, &case.judgments, 5);
        total_precision += precision_at(&ranked_grades, 3);

        let bad_paths = ranked_paths
            .iter()
            .take(3)
            .filter(|path| {
                case.forbidden_top3
                    .iter()
                    .any(|fragment| path.contains(fragment))
            })
            .cloned()
            .collect::<Vec<_>>();
        forbidden_top3 += bad_paths.len();

        diagnostics.push(format!(
            "query={:?} top_grade={} mrr_rank={:?} ndcg5={:.3} p3={:.3} top5={:?} forbidden_top3={:?}",
            case.query,
            top_grade,
            first_relevant_rank,
            ndcg_at(&ranked_grades, &case.judgments, 5),
            precision_at(&ranked_grades, 3),
            &ranked_paths[..ranked_paths.len().min(5)],
            bad_paths
        ));
    }

    let n = cases.len() as f64;
    let mean_mrr = total_mrr / n;
    let mean_ndcg = total_ndcg / n;
    let mean_precision = total_precision / n;
    let top_relevant_rate = top_relevant as f64 / n;

    assert!(
        top_relevant_rate >= 0.85,
        "top relevant rate too low: {top_relevant_rate:.3}\n{}",
        diagnostics.join("\n")
    );
    assert!(
        mean_mrr >= 0.90,
        "MRR@10 too low: {mean_mrr:.3}\n{}",
        diagnostics.join("\n")
    );
    assert!(
        mean_ndcg >= 0.82,
        "nDCG@5 too low: {mean_ndcg:.3}\n{}",
        diagnostics.join("\n")
    );
    assert!(
        mean_precision >= 0.45,
        "precision@3 too low: {mean_precision:.3}\n{}",
        diagnostics.join("\n")
    );
    assert_eq!(
        forbidden_top3,
        0,
        "low-authority noise leaked into top 3\n{}",
        diagnostics.join("\n")
    );
}

#[test]
#[serial]
fn unrelated_query_returns_no_low_confidence_recommendations() {
    let (_tmp, workspace) = stage_relevance_corpus();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();

    let hits = hybrid_search(
        &workspace,
        "quantum banana blender",
        Some(&model),
        &SearchOptions {
            limit: Some(10),
            context: 1,
            ..SearchOptions::default()
        },
    )
    .unwrap();

    assert!(
        hits.is_empty(),
        "unrelated query should not return arbitrary vector-neighbor recommendations, got {:?}",
        hit_paths(&hits)
    );
}

#[test]
#[serial]
fn doc_or_test_intent_can_surface_secondary_sources() {
    let (_tmp, workspace) = stage_relevance_corpus();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();
    enhance_workspace_hash(&workspace, &model).unwrap();

    let doc_hits = hybrid_search(
        &workspace,
        "binary detection docs",
        Some(&model),
        &SearchOptions {
            limit: Some(5),
            context: 1,
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert!(
        doc_hits
            .iter()
            .any(|hit| hit.file_path == Path::new("docs/binary-search.md")),
        "doc intent should allow documentation result, got {:?}",
        hit_paths(&doc_hits)
    );

    let test_hits = hybrid_search(
        &workspace,
        "refresh session token tests",
        Some(&model),
        &SearchOptions {
            limit: Some(5),
            context: 1,
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert!(
        test_hits
            .iter()
            .any(|hit| hit.file_path == Path::new("tests/auth_session_test.rs")),
        "test intent should allow test result, got {:?}",
        hit_paths(&test_hits)
    );
}

fn grade_by_path(judgments: &[Judgment]) -> HashMap<String, u8> {
    judgments
        .iter()
        .map(|j| (j.path.to_string(), j.grade))
        .collect()
}

fn hit_paths(hits: &[SearchHit]) -> Vec<String> {
    hits.iter()
        .map(|hit| hit.file_path.to_string_lossy().to_string())
        .collect()
}

fn precision_at(grades: &[u8], k: usize) -> f64 {
    let denominator = grades.len().min(k);
    if denominator == 0 {
        return 0.0;
    }
    let relevant = grades.iter().take(k).filter(|grade| **grade > 0).count();
    relevant as f64 / denominator as f64
}

fn graded_unique_file_results(paths: &[String], grades: &HashMap<String, u8>) -> Vec<u8> {
    let mut seen = std::collections::HashSet::new();
    paths
        .iter()
        .map(|path| {
            if seen.insert(path.as_str()) {
                *grades.get(path.as_str()).unwrap_or(&0)
            } else {
                0
            }
        })
        .collect()
}

fn ndcg_at(ranked_grades: &[u8], judgments: &[Judgment], k: usize) -> f64 {
    let actual = dcg(ranked_grades.iter().take(k).copied());
    let mut ideal = judgments.iter().map(|j| j.grade).collect::<Vec<_>>();
    ideal.sort_by(|a, b| b.cmp(a));
    let ideal = dcg(ideal.into_iter().take(k));
    if ideal <= f64::EPSILON {
        0.0
    } else {
        actual / ideal
    }
}

fn dcg(grades: impl Iterator<Item = u8>) -> f64 {
    grades
        .enumerate()
        .map(|(idx, grade)| {
            let gain = 2f64.powi(grade as i32) - 1.0;
            gain / ((idx + 2) as f64).log2()
        })
        .sum()
}

fn stage_relevance_corpus() -> (tempfile::TempDir, Workspace) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("relevance_repo");
    std::fs::create_dir_all(&root).unwrap();
    let ivygrep_home = tmp.path().join("ivygrep_home");
    unsafe { std::env::set_var("IVYGREP_HOME", &ivygrep_home) };

    write_file(
        &root,
        "src/payments/tax.rs",
        r#"
pub fn calculate_tax(subtotal: Money, region: Region) -> Money {
    let rate = lookup_tax_rate(region);
    subtotal * rate
}

fn lookup_tax_rate(region: Region) -> Money {
    Money::from_basis_points(region.tax_basis_points)
}
"#,
    );
    write_file(
        &root,
        "src/payments/discount.rs",
        r#"
pub fn apply_discount(subtotal: Money, coupon: Coupon) -> Money {
    subtotal - coupon.value
}
"#,
    );
    write_file(
        &root,
        "src/auth/session.rs",
        r#"
pub fn refresh_session_token(session: &Session) -> Token {
    let claims = validate_session(session);
    mint_token(claims)
}

pub fn validate_jwt(token: &Token) -> bool {
    token.expires_at > now()
}
"#,
    );
    write_file(
        &root,
        "src/auth/password.rs",
        r#"
pub fn hash_password(password: &str, salt: &[u8]) -> PasswordHash {
    argon2_hash(password, salt)
}

pub fn verify_password(password: &str, stored_hash: &PasswordHash) -> bool {
    constant_time_compare(password, stored_hash)
}
"#,
    );
    write_file(
        &root,
        "src/search/query_parser.rs",
        r#"
pub fn parse_query_grammar(input: &str) -> QueryAst {
    tokenize_terms(input).into_ast()
}

fn tokenize_terms(input: &str) -> Vec<Token> {
    input.split_whitespace().map(Token::from).collect()
}
"#,
    );
    write_file(
        &root,
        "src/search/ranking.rs",
        r#"
pub fn reciprocal_rank_fusion(lexical: &[Hit], semantic: &[Hit]) -> Vec<ScoredHit> {
    let fused = merge_ranked_lists(lexical, semantic);
    filter_meaningful_scores(fused)
}

fn filter_meaningful_scores(results: Vec<ScoredHit>) -> Vec<ScoredHit> {
    results.into_iter().filter(|hit| hit.score > 0.20).collect()
}
"#,
    );
    write_file(
        &root,
        "src/config/env.rs",
        r#"
pub fn load_environment_config() -> AppConfig {
    AppConfig::from_env_vars()
}
"#,
    );
    write_file(
        &root,
        "src/http/router.rs",
        r#"
pub fn dispatch_http_route(request: Request) -> Response {
    match request.path() {
        "/health" => health_check(),
        _ => route_not_found(),
    }
}
"#,
    );
    write_file(
        &root,
        "src/io/binary_detector.rs",
        r#"
pub fn detect_binary_content(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte == 0)
}
"#,
    );
    write_file(
        &root,
        "src/jobs/scheduler.rs",
        r#"
pub fn schedule_background_job(queue: &mut JobQueue) -> Option<Job> {
    queue.pop_next_ready_job()
}
"#,
    );
    write_file(
        &root,
        "tests/auth_session_test.rs",
        r#"
#[test]
fn refresh_session_token_extends_expiry() {
    assert!(true);
}
"#,
    );
    write_file(
        &root,
        "tests/binary_detector_test.rs",
        r#"
#[test]
fn binary_file_detection_skips_nul_bytes() {
    assert!(detect_binary_content(&[0, 1, 2]));
}
"#,
    );
    write_file(
        &root,
        "docs/search.md",
        r#"
# Search internals

The query parser feeds lexical ranking. Ranking score and filter thresholds are
documented here for maintainers, but implementation code lives in src/search.
"#,
    );
    write_file(
        &root,
        "docs/binary-search.md",
        r#"
# Binary search guide

This guide documents binary file detection behavior for maintainers and tests.
"#,
    );
    write_file(
        &root,
        "examples/job_scheduler_demo.rs",
        r#"
pub fn demo_background_job_scheduler() {
    println!("background job scheduler example");
}
"#,
    );
    write_file(
        &root,
        "tools/job_scheduler_probe.rs",
        r#"
pub fn probe_background_job_scheduler() {
    println!("debug background job scheduler");
}
"#,
    );
    write_file(
        &root,
        "fixtures/generated_snapshot.rs",
        r#"
// Generated fixture text repeats many tempting terms:
// session token query parser grammar tax calculated ranking score filter
// session token query parser grammar tax calculated ranking score filter
pub fn generated_fixture_do_not_rank() {}
"#,
    );
    write_file(
        &root,
        "vendor/payment_legacy.rs",
        r#"
pub fn calculate_tax_legacy_adapter() {
    // Old vendored implementation kept for migration notes.
}
"#,
    );
    write_file(
        &root,
        "data/config.json",
        r#"
{
  "notes": "environment config loader query parser ranking score filter session token tax"
}
"#,
    );

    let workspace = Workspace::resolve(&root).unwrap();
    (tmp, workspace)
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(PathBuf::from(rel));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content.trim_start()).unwrap();
}
