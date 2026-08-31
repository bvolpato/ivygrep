use super::*;
use crate::embedding::{HashEmbeddingModel, NeuralModelIdentity};
use crate::indexer::{enhance_workspace_neural, index_workspace};
use crate::workspace::WorkspaceMetadata;
use serial_test::serial;
use tempfile::tempdir;

fn index_with_ignored(workspace: &Workspace, model: &dyn EmbeddingModel) {
    workspace.ensure_dirs().unwrap();
    workspace
        .write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 1,
            last_indexed_at_unix: None,
            watch_enabled: false,
            skip_gitignore: true,
            index_generation: 0,
        })
        .unwrap();
    index_workspace(workspace, model).unwrap();
}

fn paths(hits: &[SearchHit]) -> HashSet<PathBuf> {
    hits.iter().map(|hit| hit.file_path.clone()).collect()
}

#[test]
#[serial]
fn public_search_eligibility_ignores_invisible_top_docs() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("ignored")).unwrap();
    fs::write(root.path().join(".gitignore"), "ignored/\n").unwrap();
    for index in 0..800 {
        fs::write(
            root.path().join(format!("ignored/{index:04}.rs")),
            "fn uncommonmarker() {}\n",
        )
        .unwrap();
    }
    fs::write(
        root.path().join("visible.rs"),
        format!(
            "fn uncommonmarker() {{ /* {} */ }}\n",
            "filler ".repeat(300)
        ),
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_with_ignored(&workspace, &model);
    let control = hybrid_search(
        &workspace,
        "uncommonmarker",
        None,
        &SearchOptions {
            include_globs: vec!["visible.rs".to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        paths(&control),
        HashSet::from([PathBuf::from("visible.rs")])
    );

    for embedding in [None, Some(&model as &dyn EmbeddingModel)] {
        let hits = hybrid_search(
            &workspace,
            "uncommonmarker",
            embedding,
            &SearchOptions::default(),
        )
        .unwrap();
        assert_eq!(
            paths(&hits),
            paths(&control),
            "ignored rows exhausted candidates"
        );
        assert!(
            hits.iter()
                .any(|hit| hit.sources.iter().any(|source| source == "lexical"))
        );
        assert!(
            hits.iter()
                .any(|hit| hit.sources.iter().any(|source| source == "literal"))
        );
        assert!(
            hits.iter()
                .any(|hit| hit.sources.iter().any(|source| source == "exact-symbol")),
            "ignored definitions exhausted the symbol window"
        );
    }
    let with_ignored = hybrid_search(
        &workspace,
        "uncommonmarker",
        None,
        &SearchOptions {
            skip_gitignore: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        with_ignored
            .iter()
            .any(|hit| hit.file_path.starts_with("ignored"))
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(["-c", "commit.gpgSign=false"])
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[serial]
fn public_search_eligibility_refills_past_shadowed_and_deleted_base_chunks() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let base_root = tempdir().unwrap();
    git(base_root.path(), &["init", "-q"]);
    for index in 0..800 {
        fs::write(
            base_root.path().join(format!("hidden_{index:04}.txt")),
            "uncommonmarker\n",
        )
        .unwrap();
    }
    fs::write(
        base_root.path().join("visible.txt"),
        format!("uncommonmarker {}\n", "filler ".repeat(300)),
    )
    .unwrap();
    git(base_root.path(), &["add", "."]);
    git(base_root.path(), &["commit", "-qm", "base"]);
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    let base = Workspace::resolve(base_root.path()).unwrap();
    index_workspace(&base, &model).unwrap();
    let query_vector = model.embed(&build_semantic_query_text("uncommonmarker"));
    let connection = open_sqlite_readonly(&base.sqlite_path()).unwrap();
    let mut vectors = VectorStore::open(
        &base.vector_path(),
        model.dimensions(),
        HASH_VECTOR_QUANTIZATION,
        crate::vector_store::VectorTier::Hash,
    )
    .unwrap();
    let mut statement = connection
        .prepare("SELECT vector_key, file_path FROM chunks")
        .unwrap();
    for row in statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
        })
        .unwrap()
    {
        let (key, path) = row.unwrap();
        let mut vector = query_vector.clone();
        if path == "visible.txt" {
            let zero = vector.iter().position(|value| *value == 0.0).unwrap();
            vector[zero] = 0.1;
        }
        vectors.upsert(key, vector).unwrap();
    }
    vectors.save().unwrap();
    drop(vectors);
    drop(statement);
    drop(connection);
    let worktree_parent = tempdir().unwrap();
    let worktree = worktree_parent.path().join("worktree");
    git(
        base_root.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "eligibility",
            worktree.to_str().unwrap(),
        ],
    );
    for index in 0..800 {
        let path = worktree.join(format!("hidden_{index:04}.txt"));
        if index < 400 {
            fs::remove_file(path).unwrap();
        } else {
            fs::write(path, "replacement without the queried identifier\n").unwrap();
        }
    }
    let overlay = Workspace::resolve(&worktree).unwrap();
    index_workspace(&overlay, &model).unwrap();
    let options = SearchOptions {
        limit: Some(1),
        ..Default::default()
    };
    let semantic_control = hybrid_search(
        &overlay,
        "uncommonmarker",
        Some(&model),
        &SearchOptions {
            include_globs: vec!["visible.txt".to_string()],
            ..options.clone()
        },
    )
    .unwrap();
    assert!(
        semantic_control[0]
            .sources
            .iter()
            .any(|source| source == "hash"),
        "scoped hash control failed: {semantic_control:?}"
    );
    let control = hybrid_search(
        &overlay,
        "uncommonmarker",
        None,
        &SearchOptions {
            include_globs: vec!["visible.txt".to_string()],
            ..options.clone()
        },
    )
    .unwrap();
    assert_eq!(
        paths(&control),
        HashSet::from([PathBuf::from("visible.txt")])
    );
    for embedding in [None, Some(&model as &dyn EmbeddingModel)] {
        let hits = hybrid_search(
            &overlay,
            "uncommonmarker",
            embedding,
            &SearchOptions {
                // Exact identifiers normally skip vector stores. Exercise hash
                // retrieval as well without loading any neural model.
                force_neural: embedding.is_some(),
                ..options.clone()
            },
        )
        .unwrap();
        assert_eq!(
            paths(&hits),
            paths(&control),
            "hidden base rows exhausted the pool"
        );
        if embedding.is_some() {
            assert!(
                hits[0].sources.iter().any(|source| source == "hash"),
                "hidden base vectors displaced semantic corroboration: {hits:?}"
            );
        }
    }
    assert!(
        hybrid_search(&base, "uncommonmarker", None, &options)
            .unwrap()
            .iter()
            .any(|hit| hit.file_path != Path::new("visible.txt"))
    );
}

#[test]
#[serial]
fn public_search_eligibility_refills_ignored_and_orphan_ann_keys() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(
        root.path().join("ignored.txt"),
        "irrelevant hidden document\n",
    )
    .unwrap();
    fs::write(
        root.path().join("visible.txt"),
        "zircon transport troubleshooting\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_with_ignored(&workspace, &model);
    let connection = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
    let key = |path: &str| {
        connection
            .query_row(
                "SELECT vector_key FROM chunks WHERE file_path = ?1",
                [path],
                |row| row.get::<_, i64>(0),
            )
            .unwrap() as u64
    };
    let visible_key = key("visible.txt");
    let ignored_key = key("ignored.txt");
    let query = "zircon transport troubleshooting";
    let nearest = model.embed(&build_semantic_query_text(query));
    let mut visible = nearest.clone();
    let zero = visible.iter().position(|value| *value == 0.0).unwrap();
    visible[zero] = 1.0;
    let mut vectors = VectorStore::open(
        &workspace.vector_path(),
        model.dimensions(),
        HASH_VECTOR_QUANTIZATION,
        crate::vector_store::VectorTier::Hash,
    )
    .unwrap();
    vectors.upsert(visible_key, visible).unwrap();
    vectors.upsert(ignored_key, nearest.clone()).unwrap();
    for orphan in 1_000_000..1_000_128 {
        assert!(orphan != visible_key && orphan != ignored_key);
        let mut distractor = nearest.clone();
        distractor[zero] = 0.01 + (orphan - 1_000_000) as f32 * 0.002;
        vectors.add_unchecked(orphan, distractor).unwrap();
    }
    let crowded = vectors.search(&nearest, 50);
    assert!(!crowded.is_empty());
    assert!(
        crowded.iter().all(|hit| hit.key != visible_key),
        "ANN fixture did not displace visible key: {crowded:?}"
    );
    vectors.save().unwrap();
    drop(vectors);
    let options = SearchOptions {
        limit: Some(1),
        ..Default::default()
    };
    let control = hybrid_search(
        &workspace,
        query,
        Some(&model),
        &SearchOptions {
            exclude_globs: vec!["does-not-exist/**".to_string()],
            ..options.clone()
        },
    )
    .unwrap();
    assert_eq!(
        paths(&control),
        HashSet::from([PathBuf::from("visible.txt")])
    );
    assert!(control[0].sources.iter().any(|source| source == "hash"));
    let hits = hybrid_search(&workspace, query, Some(&model), &options).unwrap();
    assert_eq!(
        paths(&hits),
        paths(&control),
        "invisible ANN keys exhausted semantic candidates"
    );
    assert!(
        hits[0].sources.iter().any(|source| source == "hash"),
        "invisible ANN keys displaced semantic corroboration"
    );
}

struct FixedNeural(NeuralModelIdentity);

impl EmbeddingModel for FixedNeural {
    fn dimensions(&self) -> usize {
        self.0.dimensions
    }
    fn embed(&self, _: &str) -> Vec<f32> {
        let mut vector = vec![0.0; self.dimensions()];
        vector[0] = 1.0;
        vector
    }
    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
        Some(&self.0)
    }
}

#[test]
#[serial]
fn public_boolean_constraints_gate_lexical_literal_path_and_neural_sources() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(root.path().join("alpha.rs"), "fn alpha() {}\n").unwrap();
    fs::write(root.path().join("beta.rs"), "fn beta() {}\n").unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &hash).unwrap();
    let neural = FixedNeural(crate::embedding::configured_neural_model_identity());
    enhance_workspace_neural(&workspace, &neural).unwrap();
    for (model, forced) in [
        (None, false),
        (Some(&hash as &dyn EmbeddingModel), false),
        (Some(&neural as &dyn EmbeddingModel), true),
    ] {
        let options = SearchOptions {
            force_neural: forced,
            ..Default::default()
        };
        assert!(
            hybrid_search(&workspace, "alpha AND beta", model, &options)
                .unwrap()
                .is_empty(),
            "AND admitted a source that lacks a required term"
        );
        let or_hits = hybrid_search(&workspace, "alpha OR beta", model, &options).unwrap();
        assert!(!or_hits.is_empty());
        assert!(paths(&or_hits).is_subset(&HashSet::from([
            PathBuf::from("alpha.rs"),
            PathBuf::from("beta.rs")
        ])));
        for path in ["alpha.rs", "beta.rs"] {
            let scoped = hybrid_search(
                &workspace,
                "alpha OR beta",
                model,
                &SearchOptions {
                    include_globs: vec![path.to_string()],
                    ..options.clone()
                },
            )
            .unwrap();
            assert_eq!(
                paths(&scoped),
                HashSet::from([PathBuf::from(path)]),
                "OR branch was not reachable"
            );
        }
        if forced {
            assert!(or_hits.iter().all(|hit| hit.neural_executed));
        }
    }
    fs::write(
        root.path().join("both.rs"),
        "fn both() { let alpha = beta; }\n",
    )
    .unwrap();
    index_workspace(&workspace, &hash).unwrap();
    enhance_workspace_neural(&workspace, &neural).unwrap();
    let and_hits = hybrid_search(
        &workspace,
        "alpha AND beta",
        Some(&neural),
        &SearchOptions {
            force_neural: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(paths(&and_hits), HashSet::from([PathBuf::from("both.rs")]));
    assert!(and_hits[0].neural_executed);
    for query in [
        "alpha AND NOT beta",
        "alpha AND NOT (beta OR gamma)",
        "alpha AND NOT NOT alpha",
        "NOT beta",
    ] {
        let not_hits = hybrid_search(&workspace, query, None, &SearchOptions::default()).unwrap();
        let expected = if query == "alpha AND NOT NOT alpha" {
            HashSet::from([PathBuf::from("alpha.rs"), PathBuf::from("both.rs")])
        } else {
            HashSet::from([PathBuf::from("alpha.rs")])
        };
        assert!(
            !not_hits.is_empty(),
            "negative query lost its positive control: {query}"
        );
        assert!(
            paths(&not_hits).is_subset(&expected),
            "negative query leaked: {query}: {not_hits:?}"
        );
    }
}

#[test]
#[serial]
fn public_boolean_constraints_reject_unsupported_syntax_without_relaxing() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("alpha.txt"),
        "alpha beta gamma\nrock AND roll\nToken::OR\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(
        &workspace,
        &HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS),
    )
    .unwrap();
    for query in ["alpha AND", "(alpha OR beta", "\"alpha beta\" AND gamma"] {
        assert!(
            hybrid_search(&workspace, query, None, &SearchOptions::default()).is_err(),
            "unsupported Boolean input was relaxed: {query}"
        );
    }
    for query in ["alpha beta", "\"rock AND roll\"", "Token::OR", "AND"] {
        assert!(
            !hybrid_search(&workspace, query, None, &SearchOptions::default())
                .unwrap()
                .is_empty(),
            "ordinary query changed: {query}"
        );
    }
    for query in [r"\AND", r#""rock \" AND roll""#] {
        assert!(
            hybrid_search(&workspace, query, None, &SearchOptions::default()).is_ok(),
            "escaped operator text was treated as Boolean syntax: {query}"
        );
    }
}

#[test]
#[serial]
fn public_boolean_constraints_preserve_prefixed_quoted_operator_words() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("music.rs"),
        "pub fn rock_and_roll() { println!(\"rock AND roll\"); }\n// Token::OR rock\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(
        &workspace,
        &HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS),
    )
    .unwrap();
    for prefix in ["text:", "+", "-", "+text:", "-text:"] {
        let double = format!("{prefix}\"rock AND roll\"");
        let single = format!("{prefix}'rock AND roll'");
        assert!(tantivy::query_grammar::parse_query(&double).is_ok());
        assert!(tantivy::query_grammar::parse_query(&single).is_ok());
        let expected = hybrid_search(&workspace, &double, None, &SearchOptions::default())
            .unwrap_or_else(|error| panic!("double-quote control {double}: {error:#}"));
        if prefix == "text:" {
            assert!(
                !expected.is_empty(),
                "field quote control lost the indexed fixture"
            );
        }
        let actual = hybrid_search(&workspace, &single, None, &SearchOptions::default())
            .unwrap_or_else(|error| {
                panic!("single quote became Boolean syntax {single}: {error:#}")
            });
        assert_eq!(
            paths(&actual),
            paths(&expected),
            "quote forms disagree for {prefix}"
        );
        let unfinished = format!("{prefix}'rock AND roll");
        assert!(
            hybrid_search(&workspace, &unfinished, None, &SearchOptions::default()).is_err(),
            "unfinished prefixed quote hid an operator: {unfinished}"
        );
    }
    assert!(
        hybrid_search(
            &workspace,
            "Token::OR rock",
            None,
            &SearchOptions::default()
        )
        .is_ok(),
        "field/unary quote prefixes changed operator boundaries"
    );
}

#[test]
#[serial]
fn public_boolean_constraints_reject_unterminated_operator_quotes() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("alpha.txt"),
        "alpha beta gamma\nrock AND roll\nToken::OR\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(
        &workspace,
        &HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS),
    )
    .unwrap();
    // Both delimiters are rejected by the pinned parser. Public dispatch must
    // not hide these malformed Boolean requests from that strict parser.
    let ctx = SearchContext::load(&workspace, None, false).unwrap();
    let parser = lexical_query_parser(&ctx, false);
    assert!(parser.parse_query("\"alpha AND beta").is_err());
    assert!(parser.parse_query("'alpha AND beta").is_err());
    for query in [r#""NOT(beta)"#, "'NOT(beta)"] {
        assert!(
            hybrid_search(&workspace, query, None, &SearchOptions::default()).is_err(),
            "quoted no-space negation bypassed strict parsing: {query}"
        );
    }
    for delimiter in ['"', '\''] {
        for clause in ["alpha AND beta", "alpha OR beta", "NOT beta"] {
            let query = format!("{delimiter}{clause}");
            assert!(
                hybrid_search(&workspace, &query, None, &SearchOptions::default()).is_err(),
                "unterminated quote hid an explicit Boolean operator: {query}"
            );
        }
        let query = format!("alpha AND {delimiter}beta");
        assert!(
            hybrid_search(&workspace, &query, None, &SearchOptions::default()).is_err(),
            "Boolean query accepted an unterminated operand quote: {query}"
        );
    }
    for query in [
        r#""alpha beta"#,
        "'alpha beta",
        r#""rock AND roll""#,
        "'rock AND roll'",
        r#""NOT(beta)""#,
        "'NOT(beta)'",
        r#""rock AND roll" "unfinished"#,
        r#""rock \" AND roll""#,
        r#""alpha \AND beta"#,
        r"'alpha \OR beta",
        r"alpha \AND beta",
        "AND",
        "Token::OR",
    ] {
        assert!(
            hybrid_search(&workspace, query, None, &SearchOptions::default()).is_ok(),
            "ordinary or escaped source text was rejected as Boolean syntax: {query}"
        );
    }
}

#[test]
#[serial]
fn public_boolean_constraints_gate_symbol_and_path_variants() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("kind_class_needle.rs"),
        "pub fn kind_class_needle() {}\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(
        &workspace,
        &HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS),
    )
    .unwrap();
    assert!(
        hybrid_search(
            &workspace,
            "kind:Class AND needle",
            None,
            &SearchOptions::default()
        )
        .unwrap()
        .is_empty(),
        "symbol/path candidates escaped the kind constraint"
    );
}
