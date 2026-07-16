use std::path::Path;

use assert_cmd::Command;
use fs_extra::dir::{CopyOptions, copy as copy_dir};
use ivygrep::embedding::create_hash_model;
use ivygrep::indexer::index_workspace;
use ivygrep::workspace::{Workspace, WorkspaceMetadata};
use ivygrep::{config, ipc};
use predicates::prelude::PredicateBooleanExt;
use serial_test::serial;

fn stage_fixture_repo(name: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let fixture_root = Path::new("tests/fixtures").join(name);
    let target_root = tmp.path().join("workspace");

    std::fs::create_dir_all(&target_root).unwrap();
    let mut opts = CopyOptions::new();
    opts.overwrite = true;
    opts.copy_inside = true;
    copy_dir(&fixture_root, &target_root, &opts).unwrap();

    let home = tmp.path().join("ivygrep_home");
    (tmp, target_root, home)
}

fn create_unhealthy_index_fixture(root: &Path, home: &Path, skip_gitignore: bool) -> Workspace {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };

    let workspace = Workspace::resolve(root).unwrap();
    workspace.ensure_dirs().unwrap();
    workspace
        .write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore,
            index_generation: 0,
        })
        .unwrap();
    std::fs::write(workspace.sqlite_path(), "").unwrap();
    std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
    std::fs::write(workspace.vector_path(), "").unwrap();
    workspace
}

fn write_stale_daemon_socket(home: &Path) {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    config::ensure_app_dirs().unwrap();
    ipc::cleanup_socket();
    std::fs::write(ipc::socket_path().unwrap(), b"stale daemon socket").unwrap();
}

fn init_git_repo(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success());
}

fn git_checked(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[serial]
fn cli_help_snapshot() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output)
        .unwrap()
        .replace("Usage: ig.exe ", "Usage: ig ");

    insta::assert_snapshot!("cli_help", text);
}

#[test]
#[serial]
fn cli_hardware_works_without_writable_app_storage() {
    let home_file = tempfile::NamedTempFile::new().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .arg("hardware")
        .env("IVYGREP_HOME", home_file.path())
        .env_remove("IVYGREP_MODEL_PROFILE")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("Installed build:"));
    assert!(text.contains("Recommended build:"));
    assert!(text.contains("Model profile: static-retrieval-v1"));
    assert!(text.contains("Profile acceleration: CPU optimized"));
}

#[test]
#[serial]
fn cli_hardware_json_is_machine_readable() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .args(["--json", "hardware"])
        .env_remove("IVYGREP_MODEL_PROFILE")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();

    assert!(report["cpu_threads"].as_u64().unwrap() > 0);
    assert_eq!(report["model_profile"], "static-retrieval-v1");
    assert_eq!(report["accelerator_applies_to_profile"], false);
    assert!(report["recommended_runtime_ready"].is_boolean());
    assert!(report["optimal_build"].is_boolean());
    assert!(matches!(
        report["recommended_build"].as_str().unwrap(),
        "portable" | "cuda" | "metal"
    ));
}

#[test]
#[serial]
fn cli_hardware_reports_transformer_acceleration_truthfully() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .arg("hardware")
        .env("IVYGREP_MODEL_PROFILE", "general")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    let expected = if text.contains("Installed build: metal")
        && text.contains("Recommended build: metal")
        && !text.contains("Missing runtime:")
    {
        "Profile acceleration: metal enabled"
    } else if text.contains("Installed build: cuda")
        && text.contains("Recommended build: cuda")
        && !text.contains("Missing runtime:")
    {
        "Profile acceleration: cuda enabled"
    } else {
        "Profile acceleration: CPU (accelerator-capable profile)"
    };
    assert!(
        text.contains(expected),
        "unexpected hardware report:\n{text}"
    );
}

#[test]
#[serial]
fn cli_status_honors_no_color() {
    let home = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .arg("--status")
        .env("IVYGREP_HOME", home.path())
        .env("NO_COLOR", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        !output.contains(&0x1b),
        "NO_COLOR output must not contain ANSI escapes"
    );
}

#[test]
#[serial]
fn cli_interactive_long_flags_are_accepted() {
    for flag in ["--interactive", "--ui"] {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
        cmd.arg(flag).arg("--version").assert().success();
    }
}

#[test]
#[serial]
fn cli_short_i_is_not_interactive_alias() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.arg("-i").arg("--version").assert().failure();
}

#[test]
#[serial]
fn cli_query_json_snapshot() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "where is the tax calculated?"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let mut value: serde_json::Value = serde_json::from_slice(&output).unwrap();

    if let Some(array) = value.as_array_mut() {
        for file in array.iter_mut() {
            if let Some(total_score) = file.get_mut("total_score") {
                *total_score = serde_json::json!("<score>");
            }

            if let Some(hits) = file.get_mut("hits").and_then(|hits| hits.as_array_mut()) {
                for hit in hits {
                    if let Some(score) = hit.get_mut("score") {
                        *score = serde_json::json!("<score>");
                    }
                }
            }
        }
    }

    insta::assert_yaml_snapshot!("cli_query_json", value);
}

#[test]
#[serial]
fn cli_file_name_only_json_snapshot() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--file-name-only",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    insta::assert_yaml_snapshot!("cli_file_name_only_json", value);
}

#[test]
#[serial]
fn cli_first_line_only_text_output() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--first-line-only",
            "--hash",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("pub fn calculate_tax"));
    assert!(!text.contains("amount * rate"));
}

#[test]
#[serial]
fn cli_query_with_explicit_path_json() {
    let (tmp, target_root, home) = stage_fixture_repo("rust_repo");
    let target_root_str = target_root.to_string_lossy().into_owned();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(tmp.path())
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "-f",
            "where is the tax calculated?",
            &target_root_str,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(!files.is_empty());
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("rust_repo/src/lib.rs"))
    );
}

#[test]
#[serial]
fn cli_query_word_add_is_treated_as_query() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "add"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let _value: serde_json::Value = serde_json::from_slice(&output).unwrap();
}

#[test]
#[serial]
fn cli_context_json_respects_budget_and_captures_relationships() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(
        root.join("src/auth.rs"),
        "pub fn validate_token(token: &str) -> bool { !token.is_empty() }\n\
         pub fn authenticate(token: &str) -> bool { validate_token(token) }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("config/auth.toml"),
        "# Change validate_token behavior configuration\n[authentication]\nvalidation_function = \"validate_token\"\nempty_token_behavior = \"reject\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("docs/auth.md"),
        "# Change validate_token behavior\n\nThe `validate_token` example rejects empty tokens.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tests/auth_test.rs"),
        "#[test]\nfn empty_tokens_are_rejected() {\n    assert!(!validate_token(\"\"));\n}\n",
    )
    .unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "context",
            "--budget",
            "4000",
            "change validate_token behavior",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let used = value["used_tokens"].as_u64().unwrap();
    assert!(used <= 4000, "bundle exceeded budget: {value:#}");
    assert_eq!(
        value["anchor_symbols"],
        serde_json::json!(["validate_token"])
    );
    let items = value["items"].as_array().unwrap();
    assert!(!items.is_empty());
    let roles = items
        .iter()
        .flat_map(|item| item["roles"].as_array().unwrap())
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    assert!(roles.contains(&"primary"), "{value:#}");
    assert!(roles.contains(&"caller"), "{value:#}");
    assert!(roles.contains(&"reference"), "{value:#}");
    assert!(roles.contains(&"test"), "{value:#}");
    assert!(roles.contains(&"config"), "{value:#}");
    assert!(roles.contains(&"documentation"), "{value:#}");
    assert!(
        value["coverage"]["files"].as_u64().unwrap() >= 2,
        "{value:#}"
    );
    assert!(
        value["coverage"]["references"].as_u64().unwrap() >= 1,
        "{value:#}"
    );
    assert!(
        items
            .iter()
            .any(|item| item["file_path"] == "tests/auth_test.rs"),
        "{value:#}"
    );
}

#[test]
#[serial]
fn cli_context_graph_tracks_dependencies_across_incremental_reindex() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"context-graph\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/auth.rs"),
        "use crate::old_clock::now; // current clock\npub fn rotate_refresh_token() { now(); }\n",
    )
    .unwrap();
    std::fs::write(root.join("src/old_clock.rs"), "pub fn now() -> u64 { 1 }\n").unwrap();
    std::fs::write(root.join("src/new_clock.rs"), "pub fn now() -> u64 { 2 }\n").unwrap();

    let run_context = || {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args([
                "--json",
                "--hash",
                "context",
                "--budget",
                "4000",
                "rotate refresh token expiration",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()
    };
    let dependency_paths = |value: &serde_json::Value| {
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| {
                item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("graph_dependency"))
            })
            .map(|item| item["file_path"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };

    let before = run_context();
    assert!(
        dependency_paths(&before).contains(&"src/old_clock.rs".to_string()),
        "{before:#}"
    );

    std::fs::write(
        root.join("src/auth.rs"),
        "use crate::new_clock::now; // replacement clock\npub fn rotate_refresh_token() { now(); }\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();
    let after = run_context();
    let paths = dependency_paths(&after);
    assert!(paths.contains(&"src/new_clock.rs".to_string()), "{after:#}");
    assert!(
        !paths.contains(&"src/old_clock.rs".to_string()),
        "{after:#}"
    );
}

#[test]
#[serial]
fn cli_context_e2e_resolves_multilanguage_dependencies() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mixed-workspace\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"mixed-workspace\",\"scripts\":{\"start_schema_widget\":\"vite\"}}\n",
    )
    .unwrap();
    std::fs::write(root.join("go.mod"), "module example.com/context\n").unwrap();
    std::fs::write(
        root.join("mix.exs"),
        "defmodule Context.MixProject do\nend\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("app")).unwrap();
    std::fs::create_dir_all(root.join("app/Acme/Util")).unwrap();
    std::fs::create_dir_all(root.join("cmd/server")).unwrap();
    std::fs::create_dir_all(root.join("errors")).unwrap();
    std::fs::create_dir_all(root.join("frontend")).unwrap();
    std::fs::create_dir_all(root.join("frontend/components/Button")).unwrap();
    std::fs::create_dir_all(root.join("internal/auth")).unwrap();
    std::fs::create_dir_all(root.join("lib/my_app")).unwrap();
    std::fs::create_dir_all(root.join("lib/local")).unwrap();
    std::fs::create_dir_all(root.join("lib/release")).unwrap();
    std::fs::create_dir_all(root.join("lib/src")).unwrap();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("src/Acme/Service")).unwrap();
    std::fs::create_dir_all(root.join("src/Acme/Util")).unwrap();
    std::fs::create_dir_all(root.join("src/generated")).unwrap();
    std::fs::create_dir_all(root.join("src/main/kotlin/com/acme/project/alias")).unwrap();
    std::fs::create_dir_all(root.join("src/main/kotlin/com/acme/project/member")).unwrap();
    std::fs::create_dir_all(root.join("src/main/kotlin/com/acme/project/module")).unwrap();
    std::fs::create_dir_all(root.join("src/main/scala/com/acme/project/grouped")).unwrap();
    std::fs::create_dir_all(root.join("src/main/scala/com/acme/project/member")).unwrap();
    std::fs::create_dir_all(root.join("src/main/scala/com/acme/project/module")).unwrap();
    std::fs::create_dir_all(root.join("src/main/groovy/com/acme/project/module")).unwrap();
    std::fs::create_dir_all(root.join("src/main/groovy/com/acme/project/util")).unwrap();
    std::fs::create_dir_all(root.join("src/main/java/com/acme/project/module")).unwrap();
    std::fs::create_dir_all(root.join("src/main/java/com/acme/project/model")).unwrap();
    std::fs::create_dir_all(root.join("src/main/java/com/acme/project/util")).unwrap();
    std::fs::create_dir_all(root.join("crates/core/src")).unwrap();
    std::fs::create_dir_all(root.join("crates/core/tests")).unwrap();
    std::fs::create_dir_all(root.join("src/release")).unwrap();
    std::fs::create_dir_all(root.join("tools")).unwrap();
    std::fs::write(root.join("app/__init__.py"), "").unwrap();
    std::fs::write(root.join("pubspec.yaml"), "name: context_app\n").unwrap();
    std::fs::write(
        root.join("service.py"),
        "from app import (\n    helper,\n)\n\ndef run_release_helper():\n    return helper.work()\n",
    )
    .unwrap();
    std::fs::write(
        root.join("app/GroupedService.php"),
        "<?php\nuse Acme\\Util\\{\n    Auth,\n    Clock,\n};\nuse Acme\\Util\\Logger, Acme\\Util\\Tracer;\nfunction verifyPhpGroupedRelease() { return Auth::check() && Clock::ready() && Logger::ready() && Tracer::ready(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("cmd/server/main.go"),
        "package main\n\nimport (\n  \"example.com/context/internal/auth\"\n  \"github.com/pkg/errors\"\n)\n\nfunc start_auth_server() { auth.Connect(); errors.New(\"start\") }\n",
    )
    .unwrap();
    std::fs::write(root.join("errors/errors.go"), "package errors\n").unwrap();
    std::fs::write(
        root.join("frontend/main.ts"),
        "import { runWidgetHelper } from \"./helper.js\";\nimport { runRootAbsoluteHelper } from '/frontend/root-absolute.js';\nimport { Button } from './components/Button';\nimport {\n  runMultilineHelper,\n} from './multiline.js';\nexport type * from './release-types.js';\nimport schema from './schema.json' with { type: \"json\" };\nimport { packageShadow } from 'package_shadow';\n\nexport function start_nodenext_widget() { return runWidgetHelper(); }\nexport function start_root_absolute_widget() { return runRootAbsoluteHelper(); }\nexport function start_directory_widget() { return Button; }\nexport function start_multiline_widget() { return runMultilineHelper(); }\nexport function start_type_barrel_widget() { return true; }\nexport function start_schema_widget() { return schema.release; }\nexport function avoid_package_shadow() { return packageShadow(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("frontend/helper.ts"),
        "export function runWidgetHelper() { return \"ready\"; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/main.dart"),
        "import 'package:context_app/src/release.dart';\nimport 'local/release_helper.dart';\nbool verifyDartPackageRelease() => verifyRelease();\nbool verifyDartRelativeRelease() => verifyRelativeRelease();\n",
    )
    .unwrap();
    std::fs::write(
        root.join("scripts/main.lua"),
        "local release = require \"release.module\"\nfunction verify_lua_release() return release.verify() end\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/my_app/service.ex"),
        "defmodule MyApp.Service do\n  alias MyApp.ReleaseAuth\n  def verify_elixir_release, do: ReleaseAuth.verify()\nend\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/BUILD.bazel"),
        "load(\":defs.bzl\", \"rule_impl\")\n\ndef configure_release_widget():\n    return rule_impl()\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tools/defs.bzl"),
        "def rule_impl():\n    return \"configured\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/java/com/acme/project/module/Service.java"),
        "package com.acme.project.module;\nimport com.acme.project.util.Helper;\nimport com.acme.project.model.Outer.Inner;\nimport static com.acme.project.util.Auth.check;\nclass Service {\n    void assembleMavenRelease() { Helper.run(); }\n    void verifyNestedRelease(Inner value) {}\n    void verifyStaticRelease() { check(); }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/groovy/com/acme/project/module/Service.groovy"),
        "package com.acme.project.module\nimport com.acme.project.util.Helper\nclass Service { def assembleGroovyRelease() { Helper.run() } }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("BUILD.bazel"),
        "load(\n    # \"//:old_root_defs.bzl\",\n    \"//:root_defs.bzl\",\n    \"root_rule\",\n)\n\ndef assemble_root_release():\n    return root_rule()\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/Acme/Service/Service.cs"),
        "global using static global::Acme.Util.Auth;\nglobal using AliasAuth = global::Acme.Alias.Util.Auth;\nclass Service { bool verifyCsharpRelease() => Check(); bool verifyCsharpAliasRelease() => AliasAuth.Check(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/kotlin/com/acme/project/module/AliasService.kt"),
        "package com.acme.project.module\nimport com.acme.project.alias.Helper as AliasHelper\nclass AliasService { fun assembleKotlinAliasRelease() = AliasHelper.run() }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/kotlin/com/acme/project/module/MemberService.kt"),
        "package com.acme.project.module\nimport com.acme.project.member.Auth.check\nclass MemberService { fun assembleKotlinMemberRelease() = check() }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/scala/com/acme/project/module/GroupedService.scala"),
        "package com.acme.project.module\nimport com.acme.project.grouped.{\n  Auth,\n  Clock,\n}\nclass GroupedService { def assembleScalaGroupedRelease() = Auth.check() && Clock.ready() }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/scala/com/acme/project/module/MemberService.scala"),
        "package com.acme.project.module\nimport com.acme.project.member.Helpers.helper\nclass MemberService { def assembleScalaMemberRelease() = helper }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/core/Cargo.toml"),
        "[package]\nname = \"context-core\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/core/tests/release_integration.rs"),
        "use context_core::{\n    crate_auth::verify_crate_auth,\n    release::{self as release_mod, token, session},\n};\n#[test]\nfn verify_crate_package_release() { verify_crate_auth(); }\n#[test]\nfn verify_nested_group_release() { release_mod::verify(); token::verify(); session::verify(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/release.rs"),
        "mod token;\npub fn verify_file_module_release() { token::verify(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/path_owner.rs"),
        "#[path = \"generated/release_path.rs\"]\nmod release_path;\npub fn verify_path_module_release() { release_path::verify(); }\n",
    )
    .unwrap();

    let run_context = |query: &str| {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args(["--json", "--hash", "context", "--budget", "3000", query])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()
    };
    let has_graph_dependency = |value: &serde_json::Value, expected: &str| {
        value["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == expected
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("graph_dependency"))
        })
    };
    let has_graph_source = |value: &serde_json::Value, expected: &str, source: &str| {
        value["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == expected
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(source))
        })
    };

    let before = run_context("change run_release_helper");
    assert!(
        !has_graph_dependency(&before, "app/helper.py"),
        "{before:#}"
    );
    let before_go = run_context("change start_auth_server");
    assert!(
        !has_graph_dependency(&before_go, "errors/errors.go"),
        "{before_go:#}"
    );
    let before_maven = run_context("change assembleMavenRelease");
    assert!(
        !has_graph_dependency(
            &before_maven,
            "src/main/java/com/acme/project/util/Helper.java"
        ),
        "{before_maven:#}"
    );
    let before_nested_java = run_context("change verifyNestedRelease");
    assert!(
        !has_graph_dependency(
            &before_nested_java,
            "src/main/java/com/acme/project/model/Outer.java"
        ),
        "{before_nested_java:#}"
    );
    let before_grouped_php = run_context("change verifyPhpGroupedRelease");
    for target in ["Auth.php", "Clock.php", "Logger.php", "Tracer.php"] {
        let expected = format!("app/Acme/Util/{target}");
        assert!(
            !has_graph_dependency(&before_grouped_php, &expected),
            "{before_grouped_php:#}"
        );
    }
    let before_bazel_root = run_context("change assemble_root_release");
    assert!(
        !has_graph_dependency(&before_bazel_root, "root_defs.bzl"),
        "{before_bazel_root:#}"
    );
    let before_static_java = run_context("change verifyStaticRelease");
    assert!(
        !has_graph_dependency(
            &before_static_java,
            "src/main/java/com/acme/project/util/Auth.java"
        ),
        "{before_static_java:#}"
    );
    let before_import_attribute = run_context("change start_schema_widget");
    assert!(
        !has_graph_dependency(&before_import_attribute, "frontend/schema.json"),
        "{before_import_attribute:#}"
    );
    assert!(
        has_graph_source(&before_import_attribute, "package.json", "graph_config"),
        "{before_import_attribute:#}"
    );
    assert!(
        !has_graph_source(&before_import_attribute, "Cargo.toml", "graph_config"),
        "{before_import_attribute:#}"
    );
    let before_groovy = run_context("change assembleGroovyRelease");
    assert!(
        !has_graph_dependency(
            &before_groovy,
            "src/main/groovy/com/acme/project/util/Helper.groovy"
        ),
        "{before_groovy:#}"
    );
    let before_static_csharp = run_context("change verifyCsharpRelease");
    assert!(
        !has_graph_dependency(&before_static_csharp, "src/Acme/Util/Auth.cs"),
        "{before_static_csharp:#}"
    );
    let before_alias_csharp = run_context("change verifyCsharpAliasRelease");
    assert!(
        !has_graph_dependency(&before_alias_csharp, "src/Acme/Alias/Util/Auth.cs"),
        "{before_alias_csharp:#}"
    );
    let before_alias_kotlin = run_context("change assembleKotlinAliasRelease");
    assert!(
        !has_graph_dependency(
            &before_alias_kotlin,
            "src/main/kotlin/com/acme/project/alias/Helper.kt"
        ),
        "{before_alias_kotlin:#}"
    );
    let before_member_kotlin = run_context("change assembleKotlinMemberRelease");
    assert!(
        !has_graph_dependency(
            &before_member_kotlin,
            "src/main/kotlin/com/acme/project/member/Auth.kt"
        ),
        "{before_member_kotlin:#}"
    );
    let before_grouped_scala = run_context("change assembleScalaGroupedRelease");
    for target in ["Auth.scala", "Clock.scala"] {
        let expected = format!("src/main/scala/com/acme/project/grouped/{target}");
        assert!(
            !has_graph_dependency(&before_grouped_scala, &expected),
            "{before_grouped_scala:#}"
        );
    }
    let before_member_scala = run_context("change assembleScalaMemberRelease");
    assert!(
        !has_graph_dependency(
            &before_member_scala,
            "src/main/scala/com/acme/project/member/Helpers.scala"
        ),
        "{before_member_scala:#}"
    );
    let before_crate_package = run_context("change verify_crate_package_release");
    assert!(
        !has_graph_dependency(&before_crate_package, "crates/core/src/crate_auth.rs"),
        "{before_crate_package:#}"
    );
    let before_multiline_typescript = run_context("change start_multiline_widget");
    assert!(
        !has_graph_dependency(&before_multiline_typescript, "frontend/multiline.ts"),
        "{before_multiline_typescript:#}"
    );
    let before_root_absolute = run_context("change start_root_absolute_widget");
    assert!(
        !has_graph_dependency(&before_root_absolute, "frontend/root-absolute.ts"),
        "{before_root_absolute:#}"
    );
    let before_directory_index = run_context("change start_directory_widget");
    assert!(
        !has_graph_dependency(
            &before_directory_index,
            "frontend/components/Button/index.jsx"
        ),
        "{before_directory_index:#}"
    );
    let before_file_module = run_context("change verify_file_module_release");
    assert!(
        !has_graph_dependency(&before_file_module, "src/release/token.rs"),
        "{before_file_module:#}"
    );
    let before_path_module = run_context("change verify_path_module_release");
    assert!(
        !has_graph_dependency(&before_path_module, "src/generated/release_path.rs"),
        "{before_path_module:#}"
    );
    let before_type_barrel = run_context("change start_type_barrel_widget");
    assert!(
        !has_graph_dependency(&before_type_barrel, "frontend/release-types.ts"),
        "{before_type_barrel:#}"
    );
    let before_dart_package = run_context("change verifyDartPackageRelease");
    assert!(
        !has_graph_dependency(&before_dart_package, "lib/src/release.dart"),
        "{before_dart_package:#}"
    );
    let before_dart_relative = run_context("change verifyDartRelativeRelease");
    assert!(
        !has_graph_dependency(&before_dart_relative, "lib/local/release_helper.dart"),
        "{before_dart_relative:#}"
    );
    let before_lua = run_context("change verify_lua_release");
    assert!(
        !has_graph_dependency(&before_lua, "lib/release/module.lua"),
        "{before_lua:#}"
    );
    let before_elixir = run_context("change verify_elixir_release");
    assert!(
        !has_graph_dependency(&before_elixir, "lib/my_app/release_auth.ex"),
        "{before_elixir:#}"
    );
    let before_package_shadow = run_context("change avoid_package_shadow");
    assert!(
        !has_graph_dependency(&before_package_shadow, "frontend/package_shadow.ts"),
        "{before_package_shadow:#}"
    );
    let before_nested_group = run_context("change verify_nested_group_release");
    for target in [
        "crates/core/src/release.rs",
        "crates/core/src/release/token.rs",
        "crates/core/src/release/session.rs",
    ] {
        assert!(
            !has_graph_dependency(&before_nested_group, target),
            "{before_nested_group:#}"
        );
    }

    std::fs::write(
        root.join("app/helper.py"),
        "def work():\n    return \"done\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("internal/auth/client.go"),
        "package auth\n\nfunc Connect() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/java/com/acme/project/util/Helper.java"),
        "package com.acme.project.util;\nclass Helper { static void run() {} }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/java/com/acme/project/util/Auth.java"),
        "package com.acme.project.util;\nclass Auth { static void check() {} }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/java/com/acme/project/model/Outer.java"),
        "package com.acme.project.model;\nclass Outer { static class Inner {} }\n",
    )
    .unwrap();
    for (class, method) in [
        ("Auth", "check"),
        ("Clock", "ready"),
        ("Logger", "ready"),
        ("Tracer", "ready"),
    ] {
        std::fs::write(
            root.join(format!("app/Acme/Util/{class}.php")),
            format!(
                "<?php namespace Acme\\Util; class {class} {{ public static function {method}() {{ return true; }} }}\n"
            ),
        )
        .unwrap();
    }
    std::fs::write(root.join("frontend/schema.json"), "{\"release\": true}\n").unwrap();
    std::fs::write(
        root.join("frontend/multiline.ts"),
        "export function runMultilineHelper() { return \"ready\"; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("frontend/root-absolute.ts"),
        "export function runRootAbsoluteHelper() { return \"ready\"; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("frontend/components/Button/index.jsx"),
        "export const Button = 'ready';\n",
    )
    .unwrap();
    std::fs::write(
        root.join("frontend/release-types.ts"),
        "export type Release = string;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("frontend/package_shadow.ts"),
        "export function packageShadow() { return false; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/src/release.dart"),
        "bool verifyRelease() => true;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/local/release_helper.dart"),
        "bool verifyRelativeRelease() => true;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/release/module.lua"),
        "local module = {}\nfunction module.verify() return true end\nreturn module\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/my_app/release_auth.ex"),
        "defmodule MyApp.ReleaseAuth do\n  def verify, do: true\nend\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/groovy/com/acme/project/util/Helper.groovy"),
        "package com.acme.project.util\nclass Helper { static def run() {} }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("root_defs.bzl"),
        "def root_rule():\n    return \"root configured\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/Acme/Util/Auth.cs"),
        "namespace Acme.Util; static class Auth { public static bool Check() => true; }\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("src/Acme/Alias/Util")).unwrap();
    std::fs::write(
        root.join("src/Acme/Alias/Util/Auth.cs"),
        "namespace Acme.Alias.Util; static class Auth { public static bool Check() => true; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/kotlin/com/acme/project/alias/Helper.kt"),
        "package com.acme.project.alias\nclass Helper { companion object { fun run() = true } }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main/kotlin/com/acme/project/member/Auth.kt"),
        "package com.acme.project.member\nobject Auth { fun check() = true }\n",
    )
    .unwrap();
    for (owner, member) in [("Auth", "check"), ("Clock", "ready")] {
        std::fs::write(
            root.join(format!(
                "src/main/scala/com/acme/project/grouped/{owner}.scala"
            )),
            format!(
                "package com.acme.project.grouped\nobject {owner} {{ def {member}() = true }}\n"
            ),
        )
        .unwrap();
    }
    std::fs::write(
        root.join("src/main/scala/com/acme/project/member/Helpers.scala"),
        "package com.acme.project.member\nobject Helpers { def helper = true }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/core/src/crate_auth.rs"),
        "pub fn verify_crate_auth() {}\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("crates/core/src/release")).unwrap();
    std::fs::write(
        root.join("crates/core/src/release.rs"),
        "pub mod token;\npub mod session;\npub fn verify() {}\n",
    )
    .unwrap();
    for module in ["token", "session"] {
        std::fs::write(
            root.join(format!("crates/core/src/release/{module}.rs")),
            "pub fn verify() {}\n",
        )
        .unwrap();
    }
    std::fs::write(root.join("src/release/token.rs"), "pub fn verify() {}\n").unwrap();
    std::fs::write(
        root.join("src/generated/release_path.rs"),
        "pub fn verify() {}\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();

    let python = run_context("change run_release_helper");
    assert!(has_graph_dependency(&python, "app/helper.py"), "{python:#}");

    let typescript = run_context("change start_nodenext_widget");
    assert!(
        has_graph_dependency(&typescript, "frontend/helper.ts"),
        "{typescript:#}"
    );
    let multiline_typescript = run_context("change start_multiline_widget");
    assert!(
        has_graph_dependency(&multiline_typescript, "frontend/multiline.ts"),
        "{multiline_typescript:#}"
    );
    let root_absolute = run_context("change start_root_absolute_widget");
    assert!(
        has_graph_dependency(&root_absolute, "frontend/root-absolute.ts"),
        "{root_absolute:#}"
    );
    let directory_index = run_context("change start_directory_widget");
    assert!(
        has_graph_dependency(&directory_index, "frontend/components/Button/index.jsx"),
        "{directory_index:#}"
    );
    let type_barrel = run_context("change start_type_barrel_widget");
    assert!(
        has_graph_dependency(&type_barrel, "frontend/release-types.ts"),
        "{type_barrel:#}"
    );
    let dart_package = run_context("change verifyDartPackageRelease");
    assert!(
        has_graph_dependency(&dart_package, "lib/src/release.dart"),
        "{dart_package:#}"
    );
    let dart_relative = run_context("change verifyDartRelativeRelease");
    assert!(
        has_graph_dependency(&dart_relative, "lib/local/release_helper.dart"),
        "{dart_relative:#}"
    );
    let lua = run_context("change verify_lua_release");
    assert!(
        has_graph_dependency(&lua, "lib/release/module.lua"),
        "{lua:#}"
    );
    let elixir = run_context("change verify_elixir_release");
    assert!(
        has_graph_dependency(&elixir, "lib/my_app/release_auth.ex"),
        "{elixir:#}"
    );
    let package_shadow = run_context("change avoid_package_shadow");
    assert!(
        !has_graph_dependency(&package_shadow, "frontend/package_shadow.ts"),
        "{package_shadow:#}"
    );

    let go = run_context("change start_auth_server");
    assert!(
        has_graph_dependency(&go, "internal/auth/client.go"),
        "{go:#}"
    );
    assert!(!has_graph_dependency(&go, "errors/errors.go"), "{go:#}");

    let bazel = run_context("change configure_release_widget");
    assert!(has_graph_dependency(&bazel, "tools/defs.bzl"), "{bazel:#}");

    let maven = run_context("change assembleMavenRelease");
    assert!(
        has_graph_dependency(&maven, "src/main/java/com/acme/project/util/Helper.java"),
        "{maven:#}"
    );

    let nested_java = run_context("change verifyNestedRelease");
    assert!(
        has_graph_dependency(
            &nested_java,
            "src/main/java/com/acme/project/model/Outer.java"
        ),
        "{nested_java:#}"
    );

    let grouped_php = run_context("change verifyPhpGroupedRelease");
    for target in ["Auth.php", "Clock.php", "Logger.php", "Tracer.php"] {
        let expected = format!("app/Acme/Util/{target}");
        assert!(
            has_graph_dependency(&grouped_php, &expected),
            "{grouped_php:#}"
        );
    }

    let bazel_root = run_context("change assemble_root_release");
    assert!(
        has_graph_dependency(&bazel_root, "root_defs.bzl"),
        "{bazel_root:#}"
    );

    let static_java = run_context("change verifyStaticRelease");
    assert!(
        has_graph_dependency(
            &static_java,
            "src/main/java/com/acme/project/util/Auth.java"
        ),
        "{static_java:#}"
    );

    let import_attribute = run_context("change start_schema_widget");
    assert!(
        has_graph_dependency(&import_attribute, "frontend/schema.json"),
        "{import_attribute:#}"
    );
    assert!(
        has_graph_source(&import_attribute, "package.json", "graph_config"),
        "{import_attribute:#}"
    );

    let groovy = run_context("change assembleGroovyRelease");
    assert!(
        has_graph_dependency(
            &groovy,
            "src/main/groovy/com/acme/project/util/Helper.groovy"
        ),
        "{groovy:#}"
    );

    let static_csharp = run_context("change verifyCsharpRelease");
    assert!(
        has_graph_dependency(&static_csharp, "src/Acme/Util/Auth.cs"),
        "{static_csharp:#}"
    );

    let alias_csharp = run_context("change verifyCsharpAliasRelease");
    assert!(
        has_graph_dependency(&alias_csharp, "src/Acme/Alias/Util/Auth.cs"),
        "{alias_csharp:#}"
    );

    let alias_kotlin = run_context("change assembleKotlinAliasRelease");
    assert!(
        has_graph_dependency(
            &alias_kotlin,
            "src/main/kotlin/com/acme/project/alias/Helper.kt"
        ),
        "{alias_kotlin:#}"
    );

    let member_kotlin = run_context("change assembleKotlinMemberRelease");
    assert!(
        has_graph_dependency(
            &member_kotlin,
            "src/main/kotlin/com/acme/project/member/Auth.kt"
        ),
        "{member_kotlin:#}"
    );

    let grouped_scala = run_context("change assembleScalaGroupedRelease");
    for target in ["Auth.scala", "Clock.scala"] {
        let expected = format!("src/main/scala/com/acme/project/grouped/{target}");
        assert!(
            has_graph_dependency(&grouped_scala, &expected),
            "{grouped_scala:#}"
        );
    }
    let member_scala = run_context("change assembleScalaMemberRelease");
    assert!(
        has_graph_dependency(
            &member_scala,
            "src/main/scala/com/acme/project/member/Helpers.scala"
        ),
        "{member_scala:#}"
    );

    let crate_package = run_context("change verify_crate_package_release");
    assert!(
        has_graph_dependency(&crate_package, "crates/core/src/crate_auth.rs"),
        "{crate_package:#}"
    );
    let file_module = run_context("change verify_file_module_release");
    assert!(
        has_graph_dependency(&file_module, "src/release/token.rs"),
        "{file_module:#}"
    );
    let path_module = run_context("change verify_path_module_release");
    assert!(
        has_graph_dependency(&path_module, "src/generated/release_path.rs"),
        "{path_module:#}"
    );
    let nested_group = run_context("change verify_nested_group_release");
    for target in [
        "crates/core/src/release.rs",
        "crates/core/src/release/token.rs",
        "crates/core/src/release/session.rs",
    ] {
        assert!(
            has_graph_dependency(&nested_group, target),
            "{nested_group:#}"
        );
    }
}

#[test]
#[serial]
fn cli_context_e2e_preserves_adjacent_tests_after_source_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src/components")).unwrap();
    std::fs::create_dir_all(root.join("__tests__/components")).unwrap();
    std::fs::create_dir_all(root.join("src/mirrored")).unwrap();
    std::fs::create_dir_all(root.join("src/__tests__/mirrored")).unwrap();
    std::fs::create_dir_all(root.join("app/models")).unwrap();
    std::fs::create_dir_all(root.join("spec/models")).unwrap();
    std::fs::write(
        root.join("src/components/widget.ts"),
        "export function render_release_widget() { return 1; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("__tests__/components/widget.test.ts"),
        "test(\"release widget\", () => render_release_widget());\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/mirrored/panel.ts"),
        "export function render_mirrored_panel() { return 1; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/__tests__/mirrored/panel.test.ts"),
        "test(\"mirrored panel\", () => render_mirrored_panel());\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/auth.py"),
        "def refresh_colocated_auth():\n    return 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/test_auth.py"),
        "def test_refresh_colocated_auth():\n    assert refresh_colocated_auth() == 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("app/models/user.rb"),
        "class User\n  def refresh_rspec_user = 1\nend\n",
    )
    .unwrap();

    let run_context = |query: &str| {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args(["--json", "--hash", "context", "--budget", "3000", query])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()
    };
    let includes_graph_test = |value: &serde_json::Value, expected: &str| {
        value["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == expected
                && item["roles"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("test"))
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("graph_test"))
        })
    };

    let before_jest = run_context("change render_release_widget");
    assert!(
        includes_graph_test(&before_jest, "__tests__/components/widget.test.ts"),
        "{before_jest:#}"
    );
    let before_pytest = run_context("change refresh_colocated_auth");
    assert!(
        includes_graph_test(&before_pytest, "src/test_auth.py"),
        "{before_pytest:#}"
    );
    let before_mirrored_jest = run_context("change render_mirrored_panel");
    assert!(
        includes_graph_test(
            &before_mirrored_jest,
            "src/__tests__/mirrored/panel.test.ts"
        ),
        "{before_mirrored_jest:#}"
    );
    let before_rspec = run_context("change refresh_rspec_user");
    assert!(
        !includes_graph_test(&before_rspec, "spec/models/user_spec.rb"),
        "{before_rspec:#}"
    );

    std::fs::write(
        root.join("spec/models/user_spec.rb"),
        "RSpec.describe User do\n  it { expect(User.new.refresh_rspec_user).to eq(1) }\nend\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();
    let added_rspec = run_context("change refresh_rspec_user");
    assert!(
        includes_graph_test(&added_rspec, "spec/models/user_spec.rb"),
        "{added_rspec:#}"
    );

    std::fs::write(
        root.join("src/components/widget.ts"),
        "export function render_release_widget() { return 2; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/auth.py"),
        "def refresh_colocated_auth():\n    return 2\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/mirrored/panel.ts"),
        "export function render_mirrored_panel() { return 2; }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("app/models/user.rb"),
        "class User\n  def refresh_rspec_user = 2\nend\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();

    let after_jest = run_context("change render_release_widget");
    assert!(
        includes_graph_test(&after_jest, "__tests__/components/widget.test.ts"),
        "{after_jest:#}"
    );
    let after_pytest = run_context("change refresh_colocated_auth");
    assert!(
        includes_graph_test(&after_pytest, "src/test_auth.py"),
        "{after_pytest:#}"
    );
    let after_mirrored_jest = run_context("change render_mirrored_panel");
    assert!(
        includes_graph_test(&after_mirrored_jest, "src/__tests__/mirrored/panel.test.ts"),
        "{after_mirrored_jest:#}"
    );
    let after_rspec = run_context("change refresh_rspec_user");
    assert!(
        includes_graph_test(&after_rspec, "spec/models/user_spec.rb"),
        "{after_rspec:#}"
    );
}

#[test]
#[serial]
fn cli_context_e2e_resolves_late_markdown_links() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(
        root.join("docs/README.md"),
        "# Release process\n\nUse document_release_process with the [release guide](release-guide.md).\n",
    )
    .unwrap();
    std::fs::write(
        root.join("release-guide.md"),
        "# Root distractor\n\nThis file must not satisfy the nested relative link.\n",
    )
    .unwrap();

    let run_context = || {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args([
                "--json",
                "--hash",
                "context",
                "--budget",
                "3000",
                "change document_release_process",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()
    };
    let includes_graph_guide = |value: &serde_json::Value, expected: &str| {
        value["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == expected
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("graph_documentation"))
        })
    };

    let before = run_context();
    assert!(
        !includes_graph_guide(&before, "docs/release-guide.md"),
        "{before:#}"
    );
    assert!(
        !includes_graph_guide(&before, "release-guide.md"),
        "{before:#}"
    );

    std::fs::write(
        root.join("docs/release-guide.md"),
        "# Guide\n\ndocument_release_process validates artifacts and publishes release notes.\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();

    let after = run_context();
    assert!(
        includes_graph_guide(&after, "docs/release-guide.md"),
        "{after:#}"
    );
    assert!(
        !includes_graph_guide(&after, "release-guide.md"),
        "{after:#}"
    );
}

#[test]
#[serial]
fn cli_context_e2e_refreshes_imports_after_manifest_identity_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'old-package'\nversion = '0.1.0'\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/auth.rs"),
        "pub fn validate_manifest_refresh() -> bool { true }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tests/integration.rs"),
        "use new_package::auth::validate_manifest_refresh;\n#[test]\nfn verify_manifest_refresh() { assert!(validate_manifest_refresh()); }\n",
    )
    .unwrap();

    let run_context = || {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args([
                "--json",
                "--hash",
                "context",
                "--budget",
                "3000",
                "change verify_manifest_refresh",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<serde_json::Value>(&output).unwrap()
    };
    let includes_graph_auth = |value: &serde_json::Value| {
        value["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == "src/auth.rs"
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("graph_dependency"))
        })
    };

    let before = run_context();
    assert!(!includes_graph_auth(&before), "{before:#}");

    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = 'new-package'\nversion = '0.1.0'\n",
    )
    .unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "--no-watch", "."])
        .assert()
        .success();

    let after = run_context();
    assert!(includes_graph_auth(&after), "{after:#}");
}

#[test]
#[serial]
fn cli_context_captures_type_constructor_relationships() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/service.rs"),
        "pub struct UserService(pub u64);\n\
         pub fn load_user_service() -> UserService {\n\
             UserService(7)\n\
         }\n",
    )
    .unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "context",
            "--budget",
            "2000",
            "change UserService construction",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(value["anchor_symbols"], serde_json::json!(["UserService"]));
    assert!(
        value["coverage"]["callers"].as_u64().unwrap() >= 1,
        "{value:#}"
    );
    assert!(
        value["coverage"]["references"].as_u64().unwrap() >= 1,
        "{value:#}"
    );
    assert!(
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["preview"].as_str().unwrap().contains("UserService(7)")),
        "{value:#}"
    );
}

#[test]
#[serial]
fn cli_context_qualified_method_anchor_finds_definition() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("workspace");
    let home = tmp.path().join("ivygrep_home");
    init_git_repo(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/client.rs"),
        "pub struct Client;\n\
         impl Client {\n\
             pub fn send(&self) -> bool { true }\n\
         }\n\
         pub struct Server;\n\
         impl Server {\n\
             pub fn receive(&self) -> bool { true }\n\
         }\n\
         pub fn deliver(client: &Client, server: &Server) -> bool {\n\
             client.send() && server.receive()\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/unrelated.rs"),
        "pub fn send() -> bool { true }\n\
         pub fn queue_message() -> bool { send() }\n",
    )
    .unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "context",
            "--budget",
            "2000",
            "fix client.send and server.receive behavior",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(
        value["anchor_symbols"],
        serde_json::json!(["client.send", "send", "server.receive", "receive"])
    );
    assert!(
        value["coverage"]["definitions"].as_u64().unwrap() >= 1,
        "{value:#}"
    );
    let reasons = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|item| item["reasons"].as_array().unwrap())
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    assert!(reasons.contains(&"defines send"), "{value:#}");
    assert!(reasons.contains(&"defines receive"), "{value:#}");
    assert!(reasons.contains(&"calls client.send"), "{value:#}");
    assert!(reasons.contains(&"calls server.receive"), "{value:#}");
    assert!(!reasons.contains(&"calls send"), "{value:#}");
    assert!(!reasons.contains(&"references send"), "{value:#}");
    let definition_previews = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            item["roles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|role| role == "definition")
        })
        .map(|item| item["preview"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        definition_previews
            .iter()
            .any(|preview| preview.contains("fn send")),
        "{value:#}"
    );
    assert!(
        definition_previews
            .iter()
            .any(|preview| preview.contains("fn receive")),
        "{value:#}"
    );
}

#[test]
#[serial]
fn cli_lexical_context_never_requests_neural_enhancement() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--hash", "-f", "add"])
        .assert()
        .success();

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--lexical-only", "--wait-for-enhancement", "context", "add"])
        .assert()
        .success();
}

#[test]
fn cli_context_help_lists_only_relevant_options() {
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["context", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--budget"))
        .stdout(predicates::str::contains("--since"))
        .stdout(predicates::str::contains("--lexical-only"))
        .stdout(predicates::str::contains("--literal").not())
        .stdout(predicates::str::contains("--limit").not());
}

#[test]
#[serial]
fn cli_context_combines_since_dirty_and_stack_trace_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(
        root.join("src/auth.rs"),
        "pub fn refresh_token() -> bool { false }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/server.rs"),
        "use crate::auth::refresh_token;\npub fn login() { refresh_token(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/session.cs"),
        "class Session { static void Refresh() {} }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("tests/auth_test.rs"),
        "#[test]\nfn refresh_token_is_safe() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.1.0'\n",
    )
    .unwrap();
    std::fs::write(root.join("README.md"), "Refresh token behavior.\n").unwrap();
    git_checked(&root, &["init", "-q", "-b", "main"]);
    git_checked(&root, &["config", "user.email", "test@example.com"]);
    git_checked(&root, &["config", "user.name", "Test"]);
    git_checked(&root, &["add", "."]);
    git_checked(&root, &["commit", "-qm", "base"]);

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--add", "--force", "--no-watch", "--hash"])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success();

    git_checked(&root, &["switch", "-qc", "feature"]);
    std::fs::write(
        root.join("src/auth.rs"),
        "pub fn refresh_token() -> bool { refresh_token_race_fix() }\nfn refresh_token_race_fix() -> bool { true }\n",
    )
    .unwrap();
    git_checked(&root, &["add", "src/auth.rs"]);
    git_checked(&root, &["commit", "-qm", "fix refresh race"]);
    std::fs::write(
        root.join("tests/auth_test.rs"),
        "#[test]\nfn dirty_regression_guard() { assert!(true); }\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(root.join("config/auth.toml"), "refresh = true\n").unwrap();

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args([
            "--json",
            "--hash",
            "context",
            "--since",
            "main",
            "fix refresh token race",
        ])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let bundle: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(bundle["change_scope"]["since"], "main");
    assert_eq!(bundle["change_scope"]["dirty_worktree"], true);
    assert!(bundle["change_scope"]["total_changes"].as_u64().unwrap() >= 3);
    assert!(
        bundle["change_scope"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| {
                change["file_path"] == "src/auth.rs"
                    && change["sources"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|source| source == "since")
            })
    );
    assert!(bundle["items"].as_array().unwrap().iter().any(|item| {
        item["file_path"] == "src/auth.rs"
            && item["preview"]
                .as_str()
                .unwrap()
                .contains("refresh_token_race_fix")
    }));
    assert!(bundle["items"].as_array().unwrap().iter().any(|item| {
        item["file_path"] == "tests/auth_test.rs"
            && item["preview"]
                .as_str()
                .unwrap()
                .contains("dirty_regression_guard")
            && item["roles"]
                .as_array()
                .unwrap()
                .iter()
                .any(|role| role == "test")
    }));

    let trace_output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--json", "--hash", "context", "-"])
        .arg(&root)
        .write_stdin("at App.Session.Refresh() in C:\\agent\\work\\repo\\src\\session.cs:line 1\n")
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let trace_bundle: serde_json::Value = serde_json::from_slice(&trace_output).unwrap();
    assert_eq!(
        trace_bundle["referenced_paths"][0]["file_path"],
        "src/session.cs"
    );
    assert_eq!(trace_bundle["referenced_paths"][0]["line"], 1);
    assert!(
        trace_bundle["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["file_path"] == "src/session.cs"
                    && item["sources"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|source| source == "task_input")
            }),
        "trace bundle: {trace_bundle:#}"
    );
}

#[test]
#[serial]
fn cli_context_bounds_changes_to_requested_scope_and_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(root.join("pkg")).unwrap();
    std::fs::create_dir_all(root.join("other")).unwrap();
    for index in 0..6 {
        std::fs::write(
            root.join(format!("pkg/module_{index}.rs")),
            format!("pub fn module_{index}() {{}}\n"),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("other/noise_{index}.rs")),
            format!("pub fn noise_{index}() {{}}\n"),
        )
        .unwrap();
    }
    git_checked(&root, &["init", "-q", "-b", "main"]);
    git_checked(&root, &["config", "user.email", "test@example.com"]);
    git_checked(&root, &["config", "user.name", "Test"]);
    git_checked(&root, &["add", "."]);
    git_checked(&root, &["commit", "-qm", "base"]);

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--add", "--force", "--no-watch", "--hash"])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success();

    git_checked(&root, &["switch", "-qc", "feature"]);
    for index in 0..6 {
        std::fs::write(
            root.join(format!("pkg/module_{index}.rs")),
            format!("pub fn module_{index}() {{ changed(); }}\n"),
        )
        .unwrap();
        std::fs::write(
            root.join(format!("other/noise_{index}.rs")),
            format!("pub fn noise_{index}() {{ changed(); }}\n"),
        )
        .unwrap();
    }
    git_checked(&root, &["add", "."]);
    git_checked(&root, &["commit", "-qm", "change modules"]);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args([
            "--json",
            "--hash",
            "context",
            "--since",
            "main",
            "--budget",
            "256",
            "review changed modules",
        ])
        .arg(root.join("pkg"))
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let bundle: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let scope = &bundle["change_scope"];
    assert_eq!(scope["total_changes"], 6);
    assert_eq!(scope["changes"].as_array().unwrap().len(), 1);
    assert_eq!(scope["changes_truncated"], true);
    assert!(
        scope["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|change| { change["file_path"].as_str().unwrap().starts_with("pkg/") })
    );
}

#[test]
#[serial]
fn cli_context_since_hydrates_deleted_files_and_callers() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/auth.rs"),
        "pub fn refresh_token() -> bool { true }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/server.rs"),
        "use crate::auth::refresh_token;\npub fn login() { refresh_token(); }\n",
    )
    .unwrap();
    git_checked(&root, &["init", "-q", "-b", "main"]);
    git_checked(&root, &["config", "user.email", "test@example.com"]);
    git_checked(&root, &["config", "user.name", "Test"]);
    git_checked(&root, &["add", "."]);
    git_checked(&root, &["commit", "-qm", "base"]);

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--add", "--force", "--no-watch", "--hash"])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success();

    git_checked(&root, &["switch", "-qc", "feature"]);
    std::fs::remove_file(root.join("src/auth.rs")).unwrap();
    git_checked(&root, &["add", "-u"]);
    git_checked(&root, &["commit", "-qm", "remove auth"]);

    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args([
            "--json",
            "--hash",
            "context",
            "--since",
            "main",
            "review this deletion",
        ])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let bundle: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        bundle["change_scope"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["file_path"] == "src/auth.rs" && change["status"] == "deleted")
    );
    assert!(
        bundle["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == "src/auth.rs"
                && item["preview"].as_str().unwrap().contains("refresh_token")
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|source| source == "git_deleted")
        }),
        "deleted seed missing: {bundle:#}"
    );
    assert!(
        bundle["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == "src/server.rs"
                && item["roles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|role| role == "caller" || role == "reference")
        }),
        "deleted callers missing: {bundle:#}"
    );
}

#[test]
#[serial]
fn cli_context_maps_new_jvm_and_dotnet_tests() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let home = tmp.path().join("home");
    let java_source = root.join("src/main/java/com/acme/Auth.java");
    let dotnet_source = root.join("src/Auth/Services/Token.cs");
    std::fs::create_dir_all(java_source.parent().unwrap()).unwrap();
    std::fs::create_dir_all(dotnet_source.parent().unwrap()).unwrap();
    std::fs::write(&java_source, "class Auth {}\n").unwrap();
    std::fs::write(&dotnet_source, "class Token {}\n").unwrap();
    git_checked(&root, &["init", "-q", "-b", "main"]);
    git_checked(&root, &["config", "user.email", "test@example.com"]);
    git_checked(&root, &["config", "user.name", "Test"]);
    git_checked(&root, &["add", "."]);
    git_checked(&root, &["commit", "-qm", "sources"]);
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--add", "--force", "--no-watch", "--hash"])
        .arg(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success();

    let java_test = root.join("src/test/java/com/acme/AuthTest.java");
    let dotnet_test = root.join("tests/Auth.Tests/Services/TokenTests.cs");
    std::fs::create_dir_all(java_test.parent().unwrap()).unwrap();
    std::fs::create_dir_all(dotnet_test.parent().unwrap()).unwrap();
    std::fs::write(&java_test, "class AuthTest {}\n").unwrap();
    std::fs::write(&dotnet_test, "class TokenTests {}\n").unwrap();

    for (query, expected_source, expected_test) in [
        (
            "change Auth",
            "src/main/java/com/acme/Auth.java",
            "src/test/java/com/acme/AuthTest.java",
        ),
        (
            "change Token",
            "src/Auth/Services/Token.cs",
            "tests/Auth.Tests/Services/TokenTests.cs",
        ),
    ] {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .args(["--json", "--hash", "context", query])
            .arg(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let bundle: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let items = bundle["items"].as_array().unwrap();
        assert!(
            items
                .iter()
                .any(|item| item["file_path"] == expected_source)
        );
        assert!(items.iter().any(|item| {
            item["file_path"] == expected_test
                && item["roles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|role| role == "test")
        }));
        assert!(items.iter().any(|item| {
            item["reasons"].as_array().unwrap().iter().any(|reason| {
                reason.as_str().is_some_and(|reason| {
                    reason.contains(expected_source) && reason.contains(expected_test)
                })
            })
        }));
    }

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["--add", "--hash", "--no-watch", "."])
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .assert()
        .success();

    for (query, expected_source) in [
        ("change AuthTest", "src/main/java/com/acme/Auth.java"),
        ("change TokenTests", "src/Auth/Services/Token.cs"),
    ] {
        let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .args(["--json", "--hash", "context", query])
            .arg(&root)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let bundle: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert!(bundle["items"].as_array().unwrap().iter().any(|item| {
            item["file_path"] == expected_source
                && item["roles"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|role| role == "dependency")
                && item["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|source| source == "graph_test")
        }));
    }
}

#[test]
fn cli_context_rejects_out_of_range_budget() {
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .args(["context", "--budget", "255", "task"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "context --budget must be between 256 and 131072 tokens",
        ));
}

#[test]
#[serial]
fn cli_add_waits_for_hash_enhancement() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env_remove("IVYGREP_NO_AUTOSPAWN")
        .env_remove("IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT")
        .args([
            "--add",
            "--hash",
            "--no-watch",
            "--wait-for-enhancement",
            ".",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Indexed") || text.contains("indexed"));

    unsafe { std::env::set_var("IVYGREP_HOME", &home) };
    let workspace = Workspace::resolve(&target_root).unwrap();
    assert!(
        !workspace.needs_hash_enhancement(),
        "--wait-for-enhancement returned before hash vectors completed"
    );
}

#[test]
#[serial]
fn cli_verbose_json_includes_reason() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--verbose",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let mut has_reason = false;
    if let Some(files) = value.as_array() {
        for file in files {
            if let Some(hits) = file.get("hits").and_then(|hits| hits.as_array()) {
                for hit in hits {
                    if hit
                        .get("reason")
                        .and_then(|reason| reason.as_str())
                        .is_some_and(|reason| !reason.trim().is_empty())
                    {
                        has_reason = true;
                    }
                }
            }
        }
    }

    assert!(has_reason);
}

#[test]
#[serial]
fn cli_query_from_subdirectory_is_scope_restricted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let scoped = root.join("scoped");
    let other = root.join("other");
    init_git_repo(&root);
    std::fs::create_dir_all(&scoped).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    std::fs::write(
        scoped.join("match.rs"),
        "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
    )
    .unwrap();
    std::fs::write(
        other.join("match.rs"),
        "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&scoped)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "applyFilter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(!files.is_empty());
    assert!(files.iter().all(|path| path.starts_with("scoped/")));
}

#[test]
#[serial]
fn cli_scoped_literal_search_survives_high_scoring_parent_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let scoped = root.join("scoped");
    let other = root.join("other");
    init_git_repo(&root);
    std::fs::create_dir_all(&scoped).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    for i in 0..700 {
        std::fs::write(
            other.join(format!("targettoken_noise_{i:03}.rs")),
            format!(
                "pub fn noisy_{i}() {{\n    // {}\n}}\n",
                "targettoken ".repeat(80)
            ),
        )
        .unwrap();
    }

    std::fs::write(
        scoped.join("match.rs"),
        "pub fn scoped_match() -> &'static str { \"targettoken\" }\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&scoped)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--literal",
            "-f",
            "-n",
            "1",
            "targettoken",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(
        files,
        vec!["scoped/match.rs"],
        "literal search from a subdirectory should not lose scoped hits behind high-scoring parent matches"
    );
}

#[test]
#[serial]
fn cli_prevent_nested_indexing() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("parent");
    let child = parent.join("child");
    std::fs::create_dir_all(&child).unwrap();

    let home = tmp.path().join("ivygrep_home");

    // Index the child repository
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.current_dir(&child)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "."])
        .assert()
        .success();

    // Try to index the parent repository (should fail)
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&parent)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "."])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("because it contains already indexed sub-workspaces"));
    assert!(text.contains("You must remove them first"));
    assert!(text.contains(&format!(
        "ig --rm {}",
        child.canonicalize().unwrap().display()
    )));
}

/// Regression: `ig --literal gquota` must find the term inside a top-level
/// `const` declaration in TypeScript, not just inside functions/classes.
#[test]
#[serial]
fn cli_literal_finds_top_level_string_constant() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();

    std::fs::write(
        root.join("plugin.ts"),
        r#"import { Plugin } from "sdk";

const GEMINI_QUOTA_COMMAND = "gquota";

export function registerCommands(p: Plugin) {
    p.registerCommand(GEMINI_QUOTA_COMMAND, () => {
        console.log("checking quota...");
    });
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("README.md"),
        "# Plugin\n\nRun `/gquota` to check your quota.\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "--literal", "-f", "gquota"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files: Vec<&str> = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect();

    assert!(
        files.iter().any(|p| p.contains("plugin.ts")),
        "literal search must find gquota in plugin.ts, got files: {:?}",
        files
    );
    assert!(
        files.iter().any(|p| p.contains("README.md")),
        "literal search must find gquota in README.md, got files: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_literal_and_regex_fall_back_when_static_daemon_socket_is_stale() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");
    unsafe { std::env::set_var("IVYGREP_HOME", &home) };

    let workspace = Workspace::resolve(&target_root).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    write_stale_daemon_socket(&home);
    let mut literal_cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let literal_output = literal_cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "--literal", "-f", "calculate_tax"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let literal_value: serde_json::Value = serde_json::from_slice(&literal_output).unwrap();
    assert!(
        literal_value.as_array().unwrap().iter().any(|entry| entry
            .get("file_path")
            .and_then(|value| value.as_str())
            .is_some_and(|path| path.ends_with("src/lib.rs"))),
        "literal search should fall back locally after stale daemon socket: {literal_value:#?}"
    );

    write_stale_daemon_socket(&home);
    let mut regex_cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let regex_output = regex_cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "--regex", "-f", "calculate_.*tax"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let regex_value: serde_json::Value = serde_json::from_slice(&regex_output).unwrap();
    assert!(
        regex_value.as_array().unwrap().iter().any(|entry| entry
            .get("file_path")
            .and_then(|value| value.as_str())
            .is_some_and(|path| path.ends_with("src/lib.rs"))),
        "regex search should fall back locally after stale daemon socket: {regex_value:#?}"
    );
}

/// Regression: hybrid (default) mode must also surface top-level constants.
#[test]
#[serial]
fn cli_hybrid_finds_top_level_string_constant() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();

    std::fs::write(
        root.join("plugin.ts"),
        r#"import { Plugin } from "sdk";

const GEMINI_QUOTA_COMMAND = "gquota";

export function registerCommands(p: Plugin) {
    p.registerCommand(GEMINI_QUOTA_COMMAND, () => {
        console.log("checking quota...");
    });
}
"#,
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "gquota"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files: Vec<&str> = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect();

    assert!(
        files.iter().any(|p| p.contains("plugin.ts")),
        "hybrid search must find gquota in plugin.ts, got files: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_doctor_json_reports_unhealthy_zero_chunk_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(tmp.path())
        .env("IVYGREP_HOME", &home)
        .args([
            "--doctor",
            "--deep",
            "--json",
            root.to_str().expect("UTF-8 fixture path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["state"], "unhealthy");
    assert_eq!(value["healthy"], false);
    assert_eq!(value["chunk_count"], 0);
    assert!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|finding| finding.as_str())
            .any(|finding| finding.contains("zero chunks")),
        "doctor findings should mention the zero-chunk failure mode: {value:#}"
    );
}

#[test]
#[serial]
fn cli_doctor_fix_repairs_unhealthy_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .args(["--doctor", "--fix", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["state"], "healthy");
    assert_eq!(value["healthy"], true);
    assert_eq!(value["repaired"], true);
    assert!(
        value["chunk_count"].as_u64().unwrap_or_default() >= 1,
        "doctor --fix should rebuild the index: {value:#}"
    );
    assert!(
        value["index_components"]["stored_chunks_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0,
        "doctor should report stored chunk bytes: {value:#}"
    );
    assert_eq!(value["compaction"]["healthy"], true);
}

#[test]
#[serial]
fn cli_status_json_reports_storage_tiers_and_compaction_health() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();
    let home = tmp.path().join("ivygrep_home");

    unsafe { std::env::set_var("IVYGREP_HOME", &home) };
    let workspace = Workspace::resolve(&root).unwrap();
    index_workspace(&workspace, create_hash_model().as_ref()).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .args(["--status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let status = &value[0];
    assert!(
        status["index_components"]["stored_chunks_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0,
        "{value:#}"
    );
    assert!(
        status["index_components"]["graph_bytes"]
            .as_u64()
            .unwrap_or_default()
            > 0,
        "{value:#}"
    );
    assert_eq!(status["compaction"]["healthy"], true);
}

#[test]
#[serial]
fn cli_query_auto_repairs_unhealthy_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "answer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(
        files.iter().any(|path| path.ends_with("lib.rs")),
        "search should recover from an unhealthy index and return lib.rs: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_query_repairs_corrupt_tantivy_after_search_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    unsafe { std::env::set_var("IVYGREP_HOME", &home) };
    let workspace = Workspace::resolve(&root).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();
    std::fs::write(workspace.tantivy_dir().join("meta.json"), b"not valid json").unwrap();
    assert_eq!(
        workspace.index_health().state,
        ivygrep::workspace::WorkspaceIndexState::Unhealthy
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "answer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(
        files.iter().any(|path| path.ends_with("lib.rs")),
        "search should repair corrupt Tantivy and return lib.rs: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_query_cleans_stale_legacy_watcher_pid() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");
    unsafe { std::env::set_var("IVYGREP_HOME", &home) };

    let workspace = Workspace::resolve(&target_root).unwrap();
    let model = create_hash_model();
    let _ = index_workspace(&workspace, model.as_ref()).unwrap();
    std::fs::write(workspace.watcher_pid_path(), "999999").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "where is the tax calculated?"])
        .assert()
        .success();

    assert!(
        !workspace.watcher_pid_path().exists(),
        "query should remove stale legacy watcher pid files"
    );
}
