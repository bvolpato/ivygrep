#!/usr/bin/env python3
"""Check daemon search results match local CLI search for representative queries."""

from __future__ import annotations

import argparse
from contextlib import closing
import json
import os
import re
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


TMP_ROOT = Path(tempfile.gettempdir()).resolve()
DEFAULT_HOME = TMP_ROOT / "ivygrep-daemon-equivalence-home"


@dataclass(frozen=True)
class Case:
    name: str
    args: list[str]
    path: Path | None = None


def run(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            cmd,
            cwd=cwd,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
            check=True,
        )
    except subprocess.CalledProcessError as error:
        error.add_note(error.stderr)
        raise


def ensure_bench_home_under_tmp(path: Path) -> Path:
    resolved = path.resolve()
    try:
        resolved.relative_to(TMP_ROOT)
    except ValueError as exc:
        raise SystemExit(f"--bench-home must resolve under {TMP_ROOT}, got {resolved}") from exc
    if resolved == TMP_ROOT:
        raise SystemExit(f"--bench-home must be a child of {TMP_ROOT}, got {resolved}")
    return resolved


def write_fixture(repo: Path) -> None:
    (repo / ".git").mkdir()
    (repo / "src").mkdir(parents=True)
    (repo / "tests").mkdir(parents=True)
    (repo / "docs").mkdir(parents=True)
    (repo / ".gitignore").write_text("target/\n", encoding="utf-8")
    (repo / "src" / "auth.rs").write_text(
        "\n".join(
            [
                "pub fn authenticate_user(token: &str) -> bool {",
                "    let csrf_guard = token.starts_with(\"csrf_\");",
                "    csrf_guard && token.len() > 8",
                "}",
                "",
                "pub fn refresh_session(user_id: u64) -> String {",
                "    format!(\"session_{user_id}\")",
                "}",
                "",
            ]
        ),
        encoding="utf-8",
    )
    (repo / "src" / "payments.py").write_text(
        "\n".join(
            [
                "def process_payment(amount_cents):",
                "    tax_total = amount_cents // 10",
                "    return amount_cents + tax_total",
                "",
            ]
        ),
        encoding="utf-8",
    )
    (repo / "tests" / "auth_test.rs").write_text(
        "fn test_authenticate_user() { assert!(true); }\n",
        encoding="utf-8",
    )
    (repo / "docs" / "auth.md").write_text(
        "Authentication docs mention csrf_guard and refresh_session behavior.\n",
        encoding="utf-8",
    )


class DaemonProcess:
    def __init__(self, proc: subprocess.Popen[bytes], log_file: Any) -> None:
        self.proc = proc
        self.log_file = log_file

    def stop(self) -> None:
        if self.proc.poll() is None:
            if os.name == "nt":
                self.proc.terminate()
            else:
                try:
                    os.killpg(self.proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if os.name == "nt":
                    self.proc.kill()
                else:
                    try:
                        os.killpg(self.proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                self.proc.wait(timeout=5)
        self.log_file.close()


def daemon_endpoint_path(bench_home: Path) -> Path:
    return bench_home / ("daemon.port" if os.name == "nt" else "daemon.sock")


def start_daemon(binary: Path, *, cwd: Path, env: dict[str, str], bench_home: Path) -> DaemonProcess:
    endpoint = daemon_endpoint_path(bench_home)
    endpoint.unlink(missing_ok=True)
    log_file = (bench_home / "equivalence-daemon.log").open("ab")
    popen_options = {"start_new_session": True} if os.name != "nt" else {}
    proc = subprocess.Popen(
        [str(binary), "--daemon"],
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=False,
        **popen_options,
    )
    daemon = DaemonProcess(proc, log_file)
    try:
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise RuntimeError("daemon exited before status became available")
            if endpoint.exists():
                try:
                    run([str(binary), "--status", "--json"], cwd=cwd, env=env)
                    return daemon
                except subprocess.CalledProcessError:
                    pass
            time.sleep(0.05)
        raise RuntimeError("daemon did not become ready")
    except BaseException:
        daemon.stop()
        raise


def normalize_output(raw: str) -> list[dict[str, Any]]:
    parsed = json.loads(raw)
    if not isinstance(parsed, list):
        raise ValueError("search output is not a JSON list")
    normalized = []
    for group in parsed:
        if not isinstance(group, dict):
            continue
        hits = []
        for hit in group.get("hits", []):
            if not isinstance(hit, dict):
                continue
            hits.append(
                {
                    "file_path": str(hit.get("file_path", "")),
                    "start_line": int(hit.get("start_line", 0)),
                    "end_line": int(hit.get("end_line", 0)),
                    "preview": str(hit.get("preview", "")),
                    "sources": sorted(str(source) for source in hit.get("sources", [])),
                }
            )
        normalized.append(
            {
                "file_path": str(group.get("file_path", "")),
                "hit_count": int(group.get("hit_count", len(hits))),
                "hits": hits,
            }
        )
    return normalized


def run_local_case(
    case: Case,
    *,
    binary: Path,
    cwd: Path,
    env: dict[str, str],
    repo: Path,
) -> list[dict[str, Any]]:
    path = case.path or repo
    base = [str(binary), "--json", *case.args, str(path)]
    return normalize_output(run([*base, "--no-watch"], cwd=cwd, env=env).stdout)


def run_daemon_case(
    case: Case,
    *,
    binary: Path,
    cwd: Path,
    env: dict[str, str],
    repo: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    path = case.path or repo
    base = [str(binary), "--json", *case.args, str(path)]
    daemon_first = run(base, cwd=cwd, env=env).stdout
    daemon_second = run(base, cwd=cwd, env=env).stdout
    return normalize_output(daemon_first), normalize_output(daemon_second)


def run_all_indices_local_case(
    *,
    binary: Path,
    cwd: Path,
    env: dict[str, str],
) -> list[dict[str, Any]]:
    all_indices = [str(binary), "--json", "--hash", "--all", "-n", "5", "authenticate user"]
    return normalize_output(run([*all_indices, "--no-watch"], cwd=cwd, env=env).stdout)


def run_all_indices_daemon_cache_case(
    *,
    binary: Path,
    cwd: Path,
    env: dict[str, str],
    repo: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    poison = [str(binary), "--json", "--hash", "-n", "5", "authenticate user", str(repo)]
    all_indices = [str(binary), "--json", "--hash", "--all", "-n", "5", "authenticate user"]
    run(poison, cwd=cwd, env=env)
    daemon_first = run(all_indices, cwd=cwd, env=env).stdout
    daemon_second = run(all_indices, cwd=cwd, env=env).stdout
    return normalize_output(daemon_first), normalize_output(daemon_second)


def canonical_content(groups: list[dict[str, Any]]) -> list[tuple[str, int, int, str]]:
    """Compare visible content, not layer-dependent BM25 scores or retrieval sources."""
    return sorted(
        (group["file_path"].replace("\\", "/"), hit["start_line"], hit["end_line"], hit["preview"])
        for group in groups for hit in group["hits"]
    )


def daemon_request(home: Path, request: dict[str, Any]) -> dict[str, Any]:
    protocol = (Path(__file__).resolve().parents[1] / "src/protocol.rs").read_text()
    version = int(re.search(r"DAEMON_PROTOCOL_VERSION: u32 = (\d+)", protocol)[1])
    if os.name == "nt":
        port = int((home / "daemon.port").read_text().strip())
        connection = socket.create_connection(("127.0.0.1", port), timeout=30)
    else:
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.settimeout(30)
        connection.connect(str(home / "daemon.sock"))
    with connection:
        connection.sendall(json.dumps({"protocol_version": version, **request}).encode() + b"\n")
        with connection.makefile("rb") as reader:
            response = json.loads(reader.readline())
    if response.get("type") == "error":
        raise AssertionError(f"daemon request failed: {response}")
    return response


def check_worktree_equivalence(binary: Path, parent: Path, env: dict[str, str]) -> int:
    checks = 0
    for transport in ("local", "daemon"):
        fixture = parent / f"layers-{transport}"
        main = fixture / "main"
        branch = fixture / "branch"
        sibling = fixture / "sibling"
        # Keep IPC paths short enough for macOS's Unix socket path limit.
        temporary_home = tempfile.TemporaryDirectory(prefix="ig-layer-")
        home = Path(temporary_home.name)
        main.mkdir(parents=True)
        environment = {**env, "IVYGREP_HOME": str(home), "IVYGREP_NO_AUTOSPAWN": "1"}
        daemon = None

        def git(path: Path, *args: str) -> None:
            run(["git", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false",
                 "-c", "user.name=worktree-e2e", "-c", "user.email=e2e@example.invalid",
                 "-c", f"core.hooksPath={fixture / 'no-hooks'}", *args], cwd=path, env=environment)

        def source(path: Path, marker: str) -> None:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f'pub fn {marker}() -> &\'static str {{ "{marker}" }}\n', encoding="utf-8")

        def index(path: Path, *, force: bool = False) -> None:
            args = [str(binary), "--add", str(path), "--hash", "--json"]
            if force:
                args.append("--force")
            if transport == "local":
                args.append("--no-watch")
            run(args, cwd=main, env=environment)

        def source_files(path: Path) -> dict[str, str]:
            return {p.relative_to(path).as_posix(): p.read_text(encoding="utf-8")
                    for p in path.rglob("*.rs")}

        def index_dir(path: Path) -> Path:
            for metadata in (home / "indexes").glob("*/workspace.json"):
                if Path(json.loads(metadata.read_text())["root"]).resolve() == path.resolve():
                    return metadata.parent
            raise AssertionError(f"workspace was not indexed: {path}")

        def assert_layers(path: Path) -> None:
            directory = index_dir(path)
            base = index_dir(main)
            reference = json.loads((directory / "base_ref.json").read_text())
            assert Path(reference["base_index_dir"]).resolve() == base.resolve()
            assert reference["base_generation"] == json.loads((base / "workspace.json").read_text())["index_generation"]
            assert reference["base_incarnation"] == (base / "index_incarnation").read_text().strip()
            for full_store in ("metadata.sqlite3", "tantivy", "vectors.usearch"):
                assert not (directory / full_store).exists(), f"materialized full store: {full_store}"
            assert (directory / "overlay_tantivy").is_dir()
            assert (directory / "overlay_vectors.usearch").is_file()
            original, current = source_files(main), source_files(path)
            divergent = {name for name, text in current.items()
                         if text.strip() and text != original.get(name)}
            hidden = {name for name, text in original.items() if text != current.get(name)}
            with closing(sqlite3.connect(directory / "overlay.sqlite3")) as database:
                actual = {row[0] for row in database.execute("SELECT DISTINCT file_path FROM chunks")}
                tombstones = {row[0] for row in database.execute("SELECT file_path FROM tombstones")}
            assert actual == divergent, f"{path.name}: overlay {actual} != delta {divergent}"
            assert tombstones == hidden, f"{path.name}: tombstones {tombstones} != hidden {hidden}"

        def compare(path: Path, stage: str) -> None:
            nonlocal checks
            # A fresh ordinary repository has no shared Git/index state with the worktree.
            with tempfile.TemporaryDirectory(prefix="oracle-", dir=fixture) as temporary:
                oracle_root = Path(temporary)
                oracle = oracle_root / "repo"
                oracle.mkdir()
                git(oracle, "init", "-q")
                for name, text in source_files(path).items():
                    destination = oracle / name
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_text(text, encoding="utf-8")
                oracle_env = {**environment, "IVYGREP_HOME": str(oracle_root / "home")}
                run([str(binary), "--add", str(oracle), "--force", "--hash", "--no-watch"],
                    cwd=oracle, env=oracle_env)
                cases = [
                    ("literal", ["--literal"], "orchard", {}),
                    ("regex", ["--regex"], "orchard_[a-z_0-9]+", {}),
                    ("hash", [], "orchard shared values", {}),
                    ("filtered", ["--include", "src/shared.rs"], "orchard shared values",
                     {"include_globs": ["src/shared.rs"]}),
                ]
                if transport == "local":
                    cases.append(("lexical", ["--lexical-only"], "orchard", {}))
                for name, flags, query, filters in cases:
                    if name == "hash":
                        # Preserve the first unprepared queries above, then exercise
                        # real hash vectors rather than only the lexical fallback.
                        targets = [(main, environment), (oracle, oracle_env)]
                        if path != main:
                            targets.append((path, environment))
                        for target, target_env in targets:
                            run([str(binary), "--enhance-hash-internal", str(target)],
                                cwd=main, env=target_env)
                    model_flags = [] if name == "lexical" else ["--hash"]
                    command = [str(binary), "--json", *model_flags, "-n", "50", "-C", "2", *flags, query]
                    expected = normalize_output(run([*command, str(oracle), "--no-watch"],
                                                    cwd=oracle, env=oracle_env).stdout)
                    if transport == "local":
                        observed = normalize_output(run([*command, str(path), "--no-watch"],
                                                        cwd=main, env=environment).stdout)
                    else:
                        request = {"type": {"literal": "literal_search", "regex": "regex_search"}.get(name, "search"),
                                   "path": str(path), "limit": 50, "context": 2,
                                   "type_filter": None, "scope_path": None, **filters}
                        request["pattern" if name == "regex" else "query"] = query
                        response = daemon_request(home, request)
                        assert response["type"] == "search_results", response
                        observed = [{"file_path": hit["file_path"], "hits": [hit]}
                                    for hit in response["hits"]]
                        # Replay the identical request to exercise the result cache too.
                        replay = daemon_request(home, request)
                        assert replay["hits"] == response["hits"], f"unstable cache: {stage}/{name}"
                    actual_content, expected_content = canonical_content(observed), canonical_content(expected)
                    assert actual_content == expected_content, (
                        f"{transport}/{stage}/{path.name}/{name}: layered != standalone\n"
                        f"layered={actual_content}\nstandalone={expected_content}"
                    )
                    assert len(actual_content) == len(set(actual_content)), "duplicate layer hits"
                    if name == "hash":
                        assert any("semantic" in hit.get("sources", [])
                                   for group in observed for hit in group["hits"]), (
                            f"{transport}/{stage}: hash-vector retrieval was not observed"
                        )
                    if name == "literal":
                        wanted = {name for name, text in source_files(path).items() if "orchard" in text}
                        assert {hit[0] for hit in actual_content} == wanted, "missing/extra source paths"
                    checks += 1
            if path != main:
                assert_layers(path)

        try:
            git(main, "init", "-q")
            git(main, "symbolic-ref", "HEAD", "refs/heads/main")
            for name in ("shared", "deleted", "renamed", "empty", "stable"):
                source(main / f"src/{name}.rs", f"orchard_{name}_v1")
            git(main, "add", ".")
            git(main, "commit", "-qm", "base")
            git(main, "worktree", "add", "-qb", "feature", str(branch))
            git(main, "worktree", "add", "-qb", "sibling", str(sibling))
            if transport == "daemon":
                daemon = start_daemon(binary, cwd=main, env=environment, bench_home=home)
            index(main)
            index(branch)
            index(sibling)
            compare(branch, "identical-base")

            source(branch / "src/shared.rs", "orchard_feature_v2")
            (branch / "src/deleted.rs").unlink()
            (branch / "src/renamed.rs").rename(branch / "src/moved.rs")
            (branch / "src/empty.rs").write_text("")
            source(branch / "src/added.rs", "orchard_feature_added")
            git(branch, "add", "-A")
            git(branch, "commit", "-qm", "feature delta")
            source(sibling / "src/shared.rs", "orchard_sibling_v2")
            index(branch)
            index(sibling)
            for path in (branch, sibling, main):
                compare(path, "divergent-worktrees")

            git(branch, "checkout", "--detach", "main")
            index(branch)
            compare(branch, "checkout-base")
            git(branch, "checkout", "feature")
            index(branch)
            compare(branch, "checkout-feature")

            source(main / "src/shared.rs", "orchard_main_v3")
            (main / "src/stable.rs").unlink()
            source(main / "src/main_only.rs", "orchard_main_only")
            git(main, "add", "-A")
            git(main, "commit", "-qm", "advance shared base")
            index(main, force=True)
            # Worktree indexes and their cached queries are deliberately not refreshed here.
            for path in (branch, sibling):
                compare(path, "base-forced-rebuild")

            source(main / "src/shared.rs", "orchard_main_v4")
            git(main, "add", "-A")
            git(main, "commit", "-qm", "incremental base update")
            index(main)
            for path in (branch, sibling):
                compare(path, "base-incremental-update")

            if daemon is not None:
                daemon.stop()
                daemon = None
            source(branch / "src/deleted.rs", "orchard_readded_v4")
            (branch / "src/added.rs").unlink()
            if transport == "daemon":
                daemon = start_daemon(binary, cwd=main, env=environment, bench_home=home)
            else:
                index(branch)
            compare(branch, "offline-delete-and-readd")
            compare(sibling, "sibling-after-restart")
            if daemon is not None:
                source(branch / "src/live_added.rs", "orchard_live_added_v5")
                source(branch / "src/shared.rs", "orchard_live_changed_v5")
                (branch / "src/moved.rs").unlink()
                deadline = time.monotonic() + 20
                while True:
                    response = daemon_request(home, {
                        "type": "literal_search", "path": str(branch),
                        "query": "orchard_live_added_v5", "context": 2,
                        "limit": 5, "type_filter": None, "scope_path": None,
                    })
                    if any(hit["file_path"].replace("\\", "/") == "src/live_added.rs"
                           for hit in response.get("hits", [])):
                        break
                    assert time.monotonic() < deadline, f"live watcher did not index addition: {response}"
                    time.sleep(0.05)
                compare(branch, "live-watcher-update")
                compare(sibling, "sibling-after-live-update")
        finally:
            if daemon is not None:
                daemon.stop()
            log = home / "equivalence-daemon.log"
            if log.exists():
                shutil.copyfile(log, fixture / "equivalence-daemon.log")
            temporary_home.cleanup()
    return checks


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench-home", type=Path, default=DEFAULT_HOME)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    bench_home = ensure_bench_home_under_tmp(args.bench_home)
    binary = args.binary.resolve() if args.binary else repo_root / "target" / "debug" / "ig"

    env = os.environ.copy()
    env["IVYGREP_HOME"] = str(bench_home)
    env["IVYGREP_NO_AUTOSPAWN"] = "1"
    env.setdefault("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0")
    env["RUST_BACKTRACE"] = env.get("RUST_BACKTRACE", "1")

    if not args.skip_build and args.binary is None:
        run(["cargo", "build", "--locked", "--bin", "ig"], cwd=repo_root, env=env)
    if not binary.exists():
        raise SystemExit(f"missing binary at {binary}")

    shutil.rmtree(bench_home, ignore_errors=True)
    fixture = bench_home / "fixture"
    fixture.mkdir(parents=True)
    write_fixture(fixture)

    run(
        [str(binary), "--add", str(fixture), "--force", "--json", "--no-watch", "--hash"],
        cwd=repo_root,
        env=env,
    )
    cases = [
        Case("hybrid_hash", ["--hash", "-n", "5", "authenticate user"]),
        Case("literal", ["--literal", "-n", "5", "csrf_guard"]),
        Case("regex", ["--regex", "-n", "5", r"csrf_[a-z]+"]),
        Case("type_filter", ["--hash", "--type", "rs", "-n", "5", "refresh session"]),
        Case("include_exclude", ["--hash", "--include", "src/**", "--exclude", "tests/**", "-n", "5", "authenticate user"]),
        Case("scope_file", ["--hash", "-n", "5", "tax total"], fixture / "src" / "payments.py"),
    ]

    failures: list[str] = []
    local_results = {
        case.name: run_local_case(
            case,
            binary=binary,
            cwd=repo_root,
            env=env,
            repo=fixture,
        )
        for case in cases
    }
    all_indices_local = run_all_indices_local_case(binary=binary, cwd=repo_root, env=env)

    daemon = start_daemon(binary, cwd=repo_root, env=env, bench_home=bench_home)
    try:
        for case in cases:
            daemon_first, daemon_second = run_daemon_case(
                case,
                binary=binary,
                cwd=repo_root,
                env=env,
                repo=fixture,
            )
            if local_results[case.name] != daemon_first:
                failures.append(f"{case.name}: local != daemon_first")
            if daemon_first != daemon_second:
                failures.append(f"{case.name}: daemon_first != daemon_second")
        daemon_first, daemon_second = run_all_indices_daemon_cache_case(
            binary=binary,
            cwd=repo_root,
            env=env,
            repo=fixture,
        )
        if all_indices_local != daemon_first:
            failures.append("all_indices_after_workspace_cache: local != daemon_first")
        if daemon_first != daemon_second:
            failures.append("all_indices_after_workspace_cache: daemon_first != daemon_second")
    finally:
        daemon.stop()

    metrics = {
        "equivalence_cases": len(cases) + 1,
        "equivalence_failures": len(failures),
        "worktree_equivalence_checks": check_worktree_equivalence(binary, bench_home, env),
    }
    print(json.dumps(metrics, sort_keys=True))
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
