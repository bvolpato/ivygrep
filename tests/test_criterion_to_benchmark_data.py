#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Tests for Criterion benchmark JSON conversion."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "criterion_to_benchmark_data.py"
SPEC = importlib.util.spec_from_file_location("criterion_to_benchmark_data", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {SCRIPT}")
criterion_to_benchmark_data = importlib.util.module_from_spec(SPEC)
sys.modules["criterion_to_benchmark_data"] = criterion_to_benchmark_data
SPEC.loader.exec_module(criterion_to_benchmark_data)


class CriterionConversionTests(unittest.TestCase):
    def test_converts_all_benchmark_complete_rows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            input_path = Path(tmp) / "criterion-output.json"
            input_path.write_text(
                "\n".join(
                    [
                        json.dumps({"reason": "compiler-artifact"}),
                        "not json",
                        json.dumps(
                            {
                                "reason": "benchmark-complete",
                                "id": "indexer/incremental_reindex_no_change",
                                "mean": {"estimate": 2500.0},
                            }
                        ),
                        json.dumps(
                            {
                                "reason": "benchmark-complete",
                                "id": "chunking/chunk_rust_100_fns",
                                "median": {"estimate": 1250.0},
                                "mean": {"estimate": 9999.0},
                            }
                        ),
                    ]
                ),
                encoding="utf-8",
            )

            data = criterion_to_benchmark_data.convert(input_path)

        self.assertEqual(
            data,
            [
                {
                    "name": "indexer/incremental_reindex_no_change",
                    "value": 2.5,
                    "unit": "\u00b5s",
                },
                {"name": "chunking/chunk_rust_100_fns", "value": 1.25, "unit": "\u00b5s"},
            ],
        )

    def test_missing_estimate_fails_loudly(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            input_path = Path(tmp) / "criterion-output.json"
            input_path.write_text(
                json.dumps({"reason": "benchmark-complete", "id": "broken"}),
                encoding="utf-8",
            )
            with self.assertRaises(SystemExit):
                criterion_to_benchmark_data.convert(input_path)


if __name__ == "__main__":
    unittest.main()
