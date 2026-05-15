#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for the Linux kernel relevance benchmark harness."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "bench_linux_relevance.py"
SPEC = importlib.util.spec_from_file_location("bench_linux_relevance", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
bench_linux_relevance = importlib.util.module_from_spec(SPEC)
sys.modules["bench_linux_relevance"] = bench_linux_relevance
SPEC.loader.exec_module(bench_linux_relevance)


class LinuxRelevanceScoringTests(unittest.TestCase):
    def test_default_query_manifest_loads(self) -> None:
        cases = bench_linux_relevance.load_cases(bench_linux_relevance.DEFAULT_QUERIES)
        self.assertGreaterEqual(len(cases), 10)
        self.assertTrue(all(case.judgments for case in cases))

    def test_exact_core_hits_score_high_without_spam(self) -> None:
        cases = bench_linux_relevance.load_cases(bench_linux_relevance.DEFAULT_QUERIES)
        case_scores = []
        for case in cases:
            core_paths = [judgment.pattern for judgment in case.judgments if "*" not in judgment.pattern]
            paths = [*core_paths, "Documentation/noise.rst", "tools/noise.c"][:10]
            case_scores.append(bench_linux_relevance.score_case(case, paths, limit=50))

        metrics = bench_linux_relevance.aggregate(case_scores, elapsed_ms=1.0, limit=50)
        self.assertGreater(metrics["linux_relevance_score"], 70.0)
        self.assertGreater(metrics["mean_ndcg10"], 0.8)

    def test_spam_at_top_penalizes_score(self) -> None:
        case = bench_linux_relevance.QueryCase(
            id="synthetic",
            query="where is core implementation",
            intent="core should beat docs",
            judgments=[bench_linux_relevance.Judgment("kernel/core.c", 3)],
            spam_patterns=["Documentation/**"],
        )
        good = bench_linux_relevance.score_case(
            case,
            ["kernel/core.c", "Documentation/noise.rst"],
            limit=50,
        )
        bad = bench_linux_relevance.score_case(
            case,
            ["Documentation/noise.rst", "kernel/core.c"],
            limit=50,
        )

        good_metrics = bench_linux_relevance.aggregate([good], elapsed_ms=1.0, limit=50)
        bad_metrics = bench_linux_relevance.aggregate([bad], elapsed_ms=1.0, limit=50)
        self.assertLess(
            bad_metrics["linux_relevance_score"],
            good_metrics["linux_relevance_score"],
        )
        self.assertEqual(bad_metrics["forbidden_top3_count"], 1)

    def test_broad_pattern_gets_credit_once(self) -> None:
        case = bench_linux_relevance.QueryCase(
            id="broad",
            query="page fault handling",
            intent="one broad class",
            judgments=[
                bench_linux_relevance.Judgment("mm/memory.c", 3),
                bench_linux_relevance.Judgment("arch/*/mm/fault.c", 2),
            ],
            spam_patterns=[],
        )

        score = bench_linux_relevance.score_case(
            case,
            [
                "arch/parisc/mm/fault.c",
                "arch/sh/mm/fault.c",
                "arch/m68k/mm/fault.c",
                "mm/memory.c",
            ],
            limit=50,
        )

        self.assertLessEqual(score["ndcg10"], 1.0)


if __name__ == "__main__":
    unittest.main()
