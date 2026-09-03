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
fn go_generic_callers_require_function_declarations() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("lib")).unwrap();
    fs::write(
        root.path().join("go.mod"),
        "module example.test/generic\n\ngo 1.23\n",
    )
    .unwrap();
    fs::write(
        root.path().join("lib/target.go"),
        "package lib\nfunc AuditTarget[T any](values ...T) {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("target.go"),
        "package fixture\nfunc AuditTarget[T any](values ...T) {}\n",
    )
    .unwrap();
    let source = r#"package fixture
import "example.test/generic/lib"; import "slices"
func inferred() { AuditTarget(1) }
func explicit() { AuditTarget[int](1) }
func zero() { AuditTarget[int]() }
func multiple() { AuditTarget[int](1, 2) }
func composite() { AuditTarget[[]int]([]int{}) }
func parenthesized() { (AuditTarget[int])(1) }
func parenthesizedZero() { (AuditTarget[int])() }
func qualified() { lib.AuditTarget[int](1) }
func qualifiedZero() { lib.AuditTarget[int]() }
func deferred() { defer AuditTarget[int](1) }
func goroutine() { go AuditTarget[int](1) }
func callback() { _ = AuditTarget[int] }
func argument() { consume(AuditTarget[int]) }
func consume(f func(...int)) {}
func PairTarget[T, U any](first T, second U) {}
func pair() { PairTarget[int, string](1, "two") }
type Conversion[T any] []T
func conversion() { _ = Conversion[int]([]int{1}) }
var callbacks = []func(int){func(int) {}}
func indexed() { index := 0; callbacks[index](1) }
func indexedLiteral() { callbacks[0](1) }
func external() { _ = slices.Clone[[]int]([]int{1}) }
"#;
    fs::write(root.path().join("calls.go"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (query, expected_callers, expected_references) in [
        (
            "AuditTarget",
            vec![3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
            vec![3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        ),
        ("lib.AuditTarget", vec![10, 11], vec![10, 11]),
        ("PairTarget", vec![18], vec![18]),
        ("slices.Clone", vec![], vec![24]),
    ] {
        for (mode, expected) in [
            (SymbolSearchMode::Callers, expected_callers),
            (SymbolSearchMode::References, expected_references),
        ] {
            let hits = search_symbols(&workspace, query, mode, None, None).unwrap();
            let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
            lines.sort_unstable();
            assert_eq!(lines, expected, "{query}, {mode:?}: {hits:?}");
            assert!(
                hits.iter()
                    .all(|hit| hit.file_path == Path::new("calls.go"))
            );
        }
    }
    for query in ["Conversion", "callbacks"] {
        let hits =
            search_symbols(&workspace, query, SymbolSearchMode::Callers, None, None).unwrap();
        assert!(hits.is_empty(), "{query}: {hits:?}");
    }
    let filtered = ivygrep::symbols::search_symbols_with_options(
        &workspace,
        "AuditTarget",
        SymbolSearchMode::Callers,
        &ivygrep::search::SearchOptions {
            type_filter: Some("go".to_string()),
            limit: None,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 11);
}

#[test]
#[serial]
fn go_type_references_exclude_declarations_and_preserve_type_uses() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = r#"package fixture
type Plain int
type Generic[T any] []T
type Alias = Plain
type (
    Grouped string
    GroupedAlias = Generic[int]
)
func use(value Plain, items Generic[int], alias Alias, grouped Grouped, other GroupedAlias) {
    _ = Plain(1)
    _ = Generic[int]([]int{1})
    _ = Alias(2)
}
type Recursive struct { Next *Recursive }
type Wrapped Generic[int]
"#;
    fs::write(root.path().join("types.go"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (query, expected) in [
        ("Plain", vec![4, 9, 10]),
        ("Generic", vec![7, 9, 11, 15]),
        ("Alias", vec![9, 12]),
        ("Grouped", vec![9]),
        ("GroupedAlias", vec![9]),
        ("Recursive", vec![14]),
        ("Wrapped", vec![]),
    ] {
        let hits =
            search_symbols(&workspace, query, SymbolSearchMode::References, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, expected, "{query}: {hits:?}");
    }
}

#[test]
#[serial]
fn go_interface_method_references_exclude_declarations_and_preserve_uses() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = r#"package fixture
type Input struct{}
type Output struct{}
type Reader interface {
    Read(input Input) (result Output)
}
func call(reader Reader, input Input) Output { return reader.Read(input) }
func value(reader Reader) func(Input) Output { return reader.Read }
type InlineReader interface { Read(Input) Output }
"#;
    fs::write(root.path().join("interfaces.go"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (query, expected) in [
        ("Read", vec![7, 8]),
        ("reader.Read", vec![7, 8]),
        ("Input", vec![5, 7, 8, 9]),
        ("Output", vec![5, 7, 8, 9]),
        ("Reader", vec![7, 8]),
        ("InlineReader", vec![]),
    ] {
        let hits =
            search_symbols(&workspace, query, SymbolSearchMode::References, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, expected, "{query}: {hits:?}");
    }
    let callers =
        search_symbols(&workspace, "Read", SymbolSearchMode::Callers, None, None).unwrap();
    assert_eq!(callers.len(), 1, "{callers:?}");
    assert_eq!(callers[0].start_line, 7);
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
fn qualified_relationships_match_generic_receiver_names() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = "struct Router<T>(T);\n\
        impl<T> Router<T> { fn audit_target<U>() {} }\n\
        struct Other<T>(T);\n\
        impl<T> Other<T> { fn audit_target<U>() {} }\n\
        fn single() { Router::<u8>::audit_target(); }\n\
        fn nested() { Router::<Vec<Vec<u8>>>::audit_target(); }\n\
        fn generic_method() { crate::Router::<Vec<u8>>::audit_target::<Option<u8>>(); }\n\
        fn trivia() {\n\
            Router /* > is comment text */ :: < Vec<u8> > /* receiver */ ::\n\
            /* name */ audit_target /* method */ ::<u8> ();\n\
        }\n\
        fn callback() { let cb = Router::<Vec<u8>>::audit_target::<u8>; }\n\
        fn wrong_owner() { Other::<Router<u8>>::audit_target::<u8>(); }\n\
        fn wrong_nested() { Other::<Vec<Router<u8>>>::audit_target(); }\n\
        fn nested_owner() { Router::<u8>::Other::audit_target(); }\n\
        fn wrong_prefix() { NotRouter::<u8>::audit_target(); }\n\
        // Router::<u8>::audit_target();\n\
        fn noise() { let text = \"Router::<u8>::audit_target()\"; }\n";
    fs::write(root.path().join("owners.rs"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (mode, expected) in [
        (SymbolSearchMode::References, vec![5, 6, 7, 10, 12]),
        (SymbolSearchMode::Callers, vec![5, 6, 7, 8]),
    ] {
        let hits = search_symbols(&workspace, "Router::audit_target", mode, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, expected, "{mode:?}: {hits:?}");
    }
}

#[test]
#[serial]
fn php_nullsafe_method_calls_are_callers() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("calls.php"),
        "<?php\n\
         class Service { public function audit_target() {} }\n\
         function ordinary($service) { $service->audit_target(); }\n\
         function nullsafe($service) { $service?->audit_target(); }\n\
         function noise() { $text = '$service?->audit_target()'; }\n\
         // $service?->audit_target();\n",
    )
    .unwrap();
    let workspace = index(root.path(), home.path());
    for mode in [SymbolSearchMode::References, SymbolSearchMode::Callers] {
        let hits = search_symbols(&workspace, "audit_target", mode, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, [3, 4], "{mode:?}: {hits:?}");
    }
}

#[test]
#[serial]
fn swift_interpolation_preserves_calls_and_function_values() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = r##"func audit_target() -> String { "ok" }
func ordinary() { audit_target() }
func line() { let s = "\(audit_target())" }
func raw() { let s = #"\#(audit_target())"# }
func multi() { let s = """
\(audit_target())
""" }
func callback() { let cb = audit_target }
func interpolated_callback() { let s = "\(audit_target)" }
func plain() { let s = "audit_target()" }
func raw_plain() { let s = #"audit_target()"# }
func nested_plain() { let s = "\("audit_target()")" }
// audit_target()
/* audit_target() */
"##;
    fs::write(root.path().join("calls.swift"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (mode, expected) in [
        (SymbolSearchMode::References, vec![2, 3, 4, 6, 8, 9]),
        (SymbolSearchMode::Callers, vec![2, 3, 4, 5]),
    ] {
        let hits = search_symbols(&workspace, "audit_target", mode, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, expected, "{mode:?}: {hits:?}");
    }
}

#[test]
#[serial]
fn dart_cascade_calls_exclude_properties_arguments_and_computed_callees() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = "class Service { void audit_target() {} }\n\
        void ordinary(Service s) { s.audit_target(); }\n\
        void cascade(Service s) { s..audit_target(); }\n\
        void nullaware(Service? s) { s?..audit_target(); }\n\
        void property(Service s) { s..audit_target; }\n\
        void callback(Service s) { var cb = s.audit_target; }\n\
        void argument(Service s) { s..invoke(s.audit_target); }\n\
        void chained(Service s) { s..other.audit_target(); }\n\
        void receiver(Service s) { s..audit_target.toString(); }\n\
        void indexed(Service s) { s..callbacks[audit_target](); }\n\
        void nullaware_property(Service? s) { s?..audit_target; }\n\
        void nullaware_chained(Service s) { s..other?.audit_target(); }\n\
        void plain() { var text = 's..audit_target()'; }\n\
        // s..audit_target();\n\
        /* s..audit_target(); */\n";
    fs::write(root.path().join("calls.dart"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for (mode, expected) in [
        (SymbolSearchMode::References, (2..=12).collect::<Vec<_>>()),
        (SymbolSearchMode::Callers, vec![2, 3, 4, 8, 12]),
    ] {
        let hits = search_symbols(&workspace, "audit_target", mode, None, None).unwrap();
        let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
        lines.sort_unstable();
        assert_eq!(lines, expected, "{mode:?}: {hits:?}");
    }
}

#[test]
#[serial]
fn php_qualified_relationships_match_sigiled_receivers() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let source = "<?php\n\
        class Service { public function audit_target() {} }\n\
        function ordinary($service) { $service->audit_target(); }\n\
        function nullsafe($service) { $service?->audit_target(); }\n\
        function property($service) { $cb = $service->audit_target; }\n\
        function nullsafe_property($service) { $cb = $service?->audit_target; }\n\
        function wrong($other) { $other->audit_target(); }\n\
        function wrong_nullsafe($other) { $other?->audit_target(); }\n\
        function noise() { $text = '$service?->audit_target()'; }\n\
        // $service->audit_target();\n";
    fs::write(root.path().join("calls.php"), source).unwrap();
    let workspace = index(root.path(), home.path());
    for name in ["service->audit_target", "$service->audit_target"] {
        for (mode, expected) in [
            (SymbolSearchMode::References, vec![3, 4, 5, 6]),
            (SymbolSearchMode::Callers, vec![3, 4]),
        ] {
            let hits = search_symbols(&workspace, name, mode, None, None).unwrap();
            let mut lines = hits.iter().map(|hit| hit.start_line).collect::<Vec<_>>();
            lines.sort_unstable();
            assert_eq!(lines, expected, "{name}, {mode:?}: {hits:?}");
        }
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
