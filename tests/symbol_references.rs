use std::fs;
use std::path::Path;

use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::index_workspace;
use ivygrep::symbols::{SymbolSearchMode, search_symbols};
use ivygrep::workspace::Workspace;
use serial_test::serial;

fn index(root: &Path, home: &Path) -> Workspace {
    fs::create_dir(root.join(".git")).unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    let workspace = Workspace::resolve(root).unwrap();
    index_workspace(&workspace, &HashEmbeddingModel::new(EMBEDDING_DIMENSIONS)).unwrap();
    workspace
}

#[test]
#[serial]
fn references_distinguish_function_values_from_calls_and_non_code() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = r##"fn audit_target<T>() {}
fn normal() { audit_target(); }
fn spaced() { audit_target (); }
fn multiline() {
    audit_target
        ();
}
fn generic() { audit_target::<Vec<u8>> (); }
fn callback() { let cb = audit_target::<u8>; }
fn argument() { consume(audit_target); }
fn commented() { audit_target /* gap */ (); }
fn noise() {
    let plain = "audit_target()";
    let raw = r#"audit_target()"#;
    // audit_target()
    /* audit_target () */
    prefix_audit_target();
    audit_target_suffix();
}
"##;
    fs::write(root.path().join("calls.rs"), source).unwrap();
    let workspace = index(root.path(), home.path());
    let references = search_symbols(
        &workspace,
        "audit_target",
        SymbolSearchMode::References,
        None,
        None,
    )
    .unwrap();
    let mut lines = references
        .iter()
        .map(|hit| hit.start_line)
        .collect::<Vec<_>>();
    lines.sort_unstable();
    assert_eq!(lines, [2, 3, 5, 8, 9, 10, 11], "{references:?}");
    let callers = search_symbols(
        &workspace,
        "audit_target",
        SymbolSearchMode::Callers,
        None,
        None,
    )
    .unwrap();
    let mut lines = callers.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
    lines.sort_unstable();
    assert_eq!(lines, [2, 3, 4, 8, 11], "{callers:?}");
}

#[test]
#[serial]
fn bounded_relationships_refill_after_rejecting_definitions() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    // Exceed both the initial candidate batch and the public result-count cap.
    for number in 0..1_100 {
        fs::write(
            root.path().join(format!("a{number:02}.rs")),
            "fn audit_target() {}\n",
        )
        .unwrap();
    }
    fs::write(
        root.path().join("z.rs"),
        "fn real_caller() { audit_target(); }\n",
    )
    .unwrap();
    let workspace = index(root.path(), home.path());
    for mode in [SymbolSearchMode::References, SymbolSearchMode::Callers] {
        for limit in [Some(1), Some(10), Some(usize::MAX), None] {
            let hits = search_symbols(&workspace, "audit_target", mode, limit, None).unwrap();
            assert_eq!(hits.len(), 1, "{mode:?}, {limit:?}: {hits:?}");
            assert_eq!(hits[0].file_path, Path::new("z.rs"));
        }
    }
}

#[test]
#[serial]
fn qualified_relationships_preserve_owners_across_spacing_and_generics() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("owners.rs"),
        "struct Router;\n\
         impl Router { fn audit_target<T>() {} }\n\
         struct Other;\n\
         impl Other { fn audit_target<T>() {} }\n\
         fn run() { Router :: audit_target::<u8> (); }\n\
         fn callback() { let cb = Router::audit_target::<u8>; }\n\
         fn wrong_owner() { Other::audit_target::<u8>(); }\n",
    )
    .unwrap();
    let workspace = index(root.path(), home.path());
    for name in [
        "Router::audit_target",
        "Router.audit_target",
        "Router#audit_target",
        "Router->audit_target",
    ] {
        let refs =
            search_symbols(&workspace, name, SymbolSearchMode::References, None, None).unwrap();
        let mut lines = refs.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, [5, 6], "{name}: {refs:?}");
        let callers =
            search_symbols(&workspace, name, SymbolSearchMode::Callers, None, None).unwrap();
        assert_eq!(callers.len(), 1, "{name}: {callers:?}");
        assert_eq!(callers[0].start_line, 5);
    }
}

#[test]
#[serial]
fn bounded_relationships_refill_after_language_filtering() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    for number in 0..240 {
        fs::write(
            root.path().join(format!("a{number:03}.py")),
            "audit_target()\n",
        )
        .unwrap();
    }
    fs::write(root.path().join("definition.rs"), "fn audit_target() {}\n").unwrap();
    fs::write(
        root.path().join("z.rs"),
        format!(
            "fn real_caller() {{\n{}audit_target();\n}}\n",
            "let noise_variable = 42;\n".repeat(20)
        ),
    )
    .unwrap();
    let workspace = index(root.path(), home.path());
    for mode in [SymbolSearchMode::References, SymbolSearchMode::Callers] {
        let hits = search_symbols(&workspace, "audit_target", mode, Some(1), None).unwrap();
        assert_eq!(hits.len(), 1, "{mode:?}: {hits:?}");
        assert_eq!(hits[0].file_path, Path::new("z.rs"));
    }
}

#[test]
#[serial]
fn multilingual_relationships_reject_declarations_strings_and_comments() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let fixtures = [
        (
            "calls.py",
            "py_target",
            "def py_target():\n    pass\ndef run():\n    py_target ()\n    cb = py_target\n    text = 'py_target()'\n    # py_target()\n",
        ),
        (
            "calls.ts",
            "ts_target",
            "function ts_target<T>() {}\nfunction run() {\n    ts_target<number> ();\n    const cb = ts_target;\n    const text = `ts_target()`;\n    // ts_target()\n}\n",
        ),
        (
            "calls.cpp",
            "cpp_target",
            "void cpp_target();\nvoid run() {\n    cpp_target ();\n    auto cb = &cpp_target;\n    auto text = \"cpp_target()\";\n    // cpp_target()\n}\n",
        ),
        (
            "Calls.java",
            "java_target",
            "class Calls {\n    void java_target() {}\n    void run() {\n        java_target ();\n        Runnable cb = this::java_target;\n        String text = \"java_target()\";\n        // java_target()\n    }\n}\n",
        ),
    ];
    for (path, _, source) in fixtures {
        fs::write(root.path().join(path), source).unwrap();
    }
    let workspace = index(root.path(), home.path());
    for (path, name, _) in fixtures {
        let refs =
            search_symbols(&workspace, name, SymbolSearchMode::References, None, None).unwrap();
        assert_eq!(refs.len(), 2, "{path}: {refs:?}");
        assert!(refs.iter().all(|hit| hit.file_path == Path::new(path)));
        assert!(
            refs.iter().any(|hit| hit.preview.contains("cb =")),
            "{path}: {refs:?}"
        );
        assert!(
            refs.iter()
                .all(|hit| !hit.preview.contains("text =") && !hit.preview.starts_with(['#', '/'])),
            "{path}: {refs:?}"
        );
        let callers =
            search_symbols(&workspace, name, SymbolSearchMode::Callers, None, None).unwrap();
        assert_eq!(callers.len(), 1, "{path}: {callers:?}");
        assert!(callers[0].preview.contains("run("));
    }
}
