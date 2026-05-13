#!/usr/bin/env python3
"""Check daemon search results match local CLI search for representative queries."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_HOME = Path("/tmp/ivygrep-daemon-equivalence-home")
TMP_ROOT = Path("/tmp").resolve()


@dataclass(frozen=True)
class Case:
    name: str
    args: list[str]
    path: Path | None = None


def run(cmd: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True, check=True)


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
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.proc.wait(timeout=5)
        self.log_file.close()


def start_daemon(binary: Path, *, cwd: Path, env: dict[str, str], bench_home: Path) -> DaemonProcess:
    socket = bench_home / "daemon.sock"
    socket.unlink(missing_ok=True)
    log_file = (bench_home / "equivalence-daemon.log").open("ab")
    proc = subprocess.Popen(
        [str(binary), "--daemon"],
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=False,
        start_new_session=True,
    )
    daemon = DaemonProcess(proc, log_file)
    try:
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise RuntimeError("daemon exited before status became available")
            if socket.exists():
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


def run_case(
    case: Case,
    *,
    binary: Path,
    cwd: Path,
    env: dict[str, str],
    repo: Path,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    path = case.path or repo
    base = [str(binary), "--json", *case.args, str(path)]
    local = run([*base, "--no-watch"], cwd=cwd, env=env).stdout
    daemon_first = run(base, cwd=cwd, env=env).stdout
    daemon_second = run(base, cwd=cwd, env=env).stdout
    return normalize_output(local), normalize_output(daemon_first), normalize_output(daemon_second)


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
    daemon = start_daemon(binary, cwd=repo_root, env=env, bench_home=bench_home)
    cases = [
        Case("hybrid_hash", ["--hash", "-n", "5", "authenticate user"]),
        Case("literal", ["--literal", "-n", "5", "csrf_guard"]),
        Case("regex", ["--regex", "-n", "5", r"csrf_[a-z]+"]),
        Case("type_filter", ["--hash", "--type", "rs", "-n", "5", "refresh session"]),
        Case("include_exclude", ["--hash", "--include", "src/**", "--exclude", "tests/**", "-n", "5", "auth"]),
        Case("scope_file", ["--hash", "-n", "5", "tax total"], fixture / "src" / "payments.py"),
    ]

    failures: list[str] = []
    try:
        for case in cases:
            local, daemon_first, daemon_second = run_case(
                case,
                binary=binary,
                cwd=repo_root,
                env=env,
                repo=fixture,
            )
            if local != daemon_first:
                failures.append(f"{case.name}: local != daemon_first")
            if daemon_first != daemon_second:
                failures.append(f"{case.name}: daemon_first != daemon_second")
    finally:
        daemon.stop()

    metrics = {
        "equivalence_cases": len(cases),
        "equivalence_failures": len(failures),
    }
    print(json.dumps(metrics, sort_keys=True))
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
