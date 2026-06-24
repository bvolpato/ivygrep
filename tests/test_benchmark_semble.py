import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "benchmark_semble.py"
SPEC = importlib.util.spec_from_file_location("benchmark_semble", SCRIPT)
benchmark = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = benchmark
SPEC.loader.exec_module(benchmark)


class Target:
    def __init__(self, path, start_line=None, end_line=None):
        self.path = path
        self.start_line = start_line
        self.end_line = end_line

    @property
    def has_span(self):
        return self.start_line is not None and self.end_line is not None


class SembleBenchmarkTest(unittest.TestCase):
    def test_ndcg_rewards_earlier_relevant_hits(self):
        self.assertGreater(
            benchmark.ndcg_at_10([1, 3], 2),
            benchmark.ndcg_at_10([3, 8], 2),
        )

    def test_target_rank_matches_suffix_and_line_overlap(self):
        hits = [
            benchmark.RankedHit("src/auth.rs", 10, 20, "body", 1.0),
            benchmark.RankedHit("src/other.rs", 1, 5, "body", 0.5),
        ]
        self.assertEqual(
            benchmark.target_rank(
                hits, Target("repo/src/auth.rs", start_line=15, end_line=16)
            ),
            1,
        )
        self.assertIsNone(
            benchmark.target_rank(
                hits, Target("repo/src/auth.rs", start_line=30, end_line=40)
            )
        )

    def test_ivygrep_hits_are_sorted_by_score(self):
        response = {
            "hits": [
                {
                    "file_path": "low.rs",
                    "start_line": 1,
                    "end_line": 2,
                    "preview": "low",
                    "score": 0.1,
                },
                {
                    "file_path": "high.rs",
                    "start_line": 3,
                    "end_line": 4,
                    "preview": "high",
                    "score": 0.9,
                },
            ]
        }
        hits = benchmark.flatten_ivygrep_hits(response)
        self.assertEqual([hit.file_path for hit in hits], ["high.rs", "low.rs"])

    def test_persisted_hits_omit_retrieved_source(self):
        hit = benchmark.RankedHit("src/auth.rs", 10, 20, "secret body", 1.0)
        self.assertEqual(
            benchmark.persisted_hit(hit),
            {
                "file_path": "src/auth.rs",
                "start_line": 10,
                "end_line": 20,
                "score": 1.0,
            },
        )

    def test_verdict_tracks_inverted_winners(self):
        payload = {
            "generated_at": "2026-06-24T00:00:00+00:00",
            "ivygrep": {"sha": "ivy", "dirty": False},
            "semble": {"sha": "semble", "version": "test"},
            "summary": {
                "ivygrep": {
                    "ndcg_at_10": 0.9,
                    "latency_p50_ms": 4.0,
                    "latency_p95_ms": 8.0,
                    "mean_returned_tokens": 200,
                    "by_category": {
                        "architecture": 0.9,
                        "semantic": 0.9,
                        "symbol": 0.9,
                    },
                },
                "semble": {
                    "ndcg_at_10": 0.8,
                    "latency_p50_ms": 8.0,
                    "latency_p95_ms": 12.0,
                    "mean_returned_tokens": 100,
                    "by_category": {
                        "architecture": 0.8,
                        "semantic": 0.8,
                        "symbol": 0.8,
                    },
                },
            },
            "indexing": {
                "repo": {
                    "ivygrep": {"ready_ms": 200},
                    "semble": {"index_ms": 100},
                }
            },
            "refresh": {
                "ivygrep_lexical_refresh_ms": 25,
                "ivygrep_full_refresh_ms": 100,
                "semble_full_refresh_ms": 50,
            },
        }

        rendered = benchmark.render_markdown(payload)
        self.assertIn(
            "ivygrep leads overall retrieval quality by 0.100 nDCG@10 and warm "
            "p50 latency by 2.0x.",
            rendered,
        )
        self.assertIn(
            "Semble returns 2.0x fewer tokens in top-10 results.",
            rendered,
        )
        self.assertIn(
            "Semble full one-file refresh is 2.0x faster",
            rendered,
        )
        self.assertIn(
            "Semble indexing is faster on every benchmark repository",
            rendered,
        )
        self.assertIn("ivygrep leads every measured quality category.", rendered)

    def test_verdict_reports_ties_without_inventing_a_winner(self):
        payload = {
            "generated_at": "2026-06-24T00:00:00+00:00",
            "ivygrep": {"sha": "ivy", "dirty": False},
            "semble": {"sha": "semble", "version": "test"},
            "summary": {
                tool: {
                    "ndcg_at_10": 0.8,
                    "latency_p50_ms": 5.0,
                    "latency_p95_ms": 10.0,
                    "mean_returned_tokens": 100,
                    "by_category": {
                        "architecture": 0.8,
                        "semantic": 0.8,
                        "symbol": 0.8,
                    },
                }
                for tool in ("ivygrep", "semble")
            },
            "indexing": {
                "repo": {
                    "ivygrep": {"ready_ms": 100},
                    "semble": {"index_ms": 100},
                }
            },
            "refresh": {
                "ivygrep_lexical_refresh_ms": 25,
                "ivygrep_full_refresh_ms": 100,
                "semble_full_refresh_ms": 100,
            },
        }

        rendered = benchmark.render_markdown(payload)
        self.assertIn(
            "Overall retrieval quality and warm p50 latency are tied.", rendered
        )
        self.assertIn(
            "Both tools return the same mean token count in top-10 results.", rendered
        )
        self.assertIn("Full one-file refresh time is tied", rendered)
        self.assertIn("Initial indexing time is tied", rendered)
        self.assertIn("Every measured quality category is tied.", rendered)
        self.assertIn("| nDCG@10 | 0.800 | 0.800 | Tie |", rendered)


if __name__ == "__main__":
    unittest.main()
