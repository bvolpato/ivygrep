#!/usr/bin/env python3
"""Compare two ivygrep binaries with interleaved million-index queries."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path

import bench_million_chunks as benchmark


def result_record(
    client: benchmark.DaemonClient,
    query: str,
    expected_path: str,
) -> dict:
    return {
        **client.query(query),
        "expected_path": expected_path,
    }


def artifact(
    binary: Path,
    source_commit: str,
    corpus: Path,
    manifest: dict,
    records: list[dict],
    first_in_pair: int,
) -> dict:
    return {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "ivygrep_commit": source_commit,
        "binary": benchmark.binary_identity(binary),
        "runtime": benchmark.runtime_metadata(),
        "corpus": {
            **manifest,
            "manifest_sha256": benchmark.sha256_file(corpus / "corpus-manifest.json"),
        },
        "index": {
            "chunks_per_second": None,
        },
        "queries": {
            "warm_distinct": {
                **benchmark.summarize_queries(records),
                "paired_interleaving": True,
                "first_in_pair": first_in_pair,
            },
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--baseline-home", type=Path, required=True)
    parser.add_argument("--current-home", type=Path, required=True)
    parser.add_argument("--baseline-binary", type=Path, required=True)
    parser.add_argument("--current-binary", type=Path, required=True)
    parser.add_argument("--baseline-commit", required=True)
    parser.add_argument("--current-commit", required=True)
    parser.add_argument("--files", type=int, default=benchmark.DEFAULT_FILES)
    parser.add_argument(
        "--chunks-per-file",
        type=int,
        default=benchmark.DEFAULT_CHUNKS_PER_FILE,
    )
    parser.add_argument("--query-samples", type=int, default=200)
    parser.add_argument("--baseline-output", type=Path, required=True)
    parser.add_argument("--current-output", type=Path, required=True)
    args = parser.parse_args()
    args.corpus = args.corpus.resolve()
    args.baseline_home = args.baseline_home.resolve()
    args.current_home = args.current_home.resolve()
    args.baseline_binary = args.baseline_binary.resolve()
    args.current_binary = args.current_binary.resolve()

    manifest = benchmark.generate_corpus(
        args.corpus,
        args.files,
        args.chunks_per_file,
    )
    cases = benchmark.query_cases(
        args.query_samples,
        manifest["expected_chunks"],
        manifest["chunks_per_file"],
    )
    common_env = {
        **os.environ,
        "IVYGREP_NO_AUTOSPAWN": "1",
        "IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT": "1",
        "IVYGREP_ENHANCE_MAX_LOAD_RATIO": "0",
        "IVYGREP_RERANKER": "learned",
    }
    baseline_env = {**common_env, "IVYGREP_HOME": str(args.baseline_home)}
    current_env = {**common_env, "IVYGREP_HOME": str(args.current_home)}
    baseline_daemon, baseline_log, _ = benchmark.start_daemon(
        args.baseline_binary,
        args.corpus,
        baseline_env,
        args.baseline_home,
        "paired-baseline-daemon.log",
    )
    current_daemon, current_log, _ = benchmark.start_daemon(
        args.current_binary,
        args.corpus,
        current_env,
        args.current_home,
        "paired-current-daemon.log",
    )
    baseline_records = []
    current_records = []
    baseline_first = 0
    current_first = 0
    try:
        with (
            benchmark.DaemonClient(args.baseline_home, args.corpus) as baseline,
            benchmark.DaemonClient(args.current_home, args.corpus) as current,
        ):
            baseline.query("warmup generated operation")
            current.query("warmup generated operation")
            for index, (query, expected_path) in enumerate(cases):
                if index % 2 == 0:
                    baseline_records.append(
                        result_record(baseline, query, expected_path)
                    )
                    current_records.append(result_record(current, query, expected_path))
                    baseline_first += 1
                else:
                    current_records.append(result_record(current, query, expected_path))
                    baseline_records.append(
                        result_record(baseline, query, expected_path)
                    )
                    current_first += 1
    finally:
        benchmark.stop_daemon(current_daemon, current_log)
        benchmark.stop_daemon(baseline_daemon, baseline_log)

    baseline_artifact = artifact(
        args.baseline_binary,
        args.baseline_commit,
        args.corpus,
        manifest,
        baseline_records,
        baseline_first,
    )
    current_artifact = artifact(
        args.current_binary,
        args.current_commit,
        args.corpus,
        manifest,
        current_records,
        current_first,
    )
    args.baseline_output.write_text(
        json.dumps(baseline_artifact, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    args.current_output.write_text(
        json.dumps(current_artifact, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "baseline_p95_ms": baseline_artifact["queries"]["warm_distinct"][
                    "p95_ms"
                ],
                "current_p95_ms": current_artifact["queries"]["warm_distinct"][
                    "p95_ms"
                ],
                "samples": args.query_samples,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
