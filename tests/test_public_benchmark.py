import importlib.util
import json
import math
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


exporter = load_script("export_public_retrieval")
sys.modules["export_public_retrieval"] = exporter
coreb_exporter = load_script("export_coreb")
evaluator = load_script("eval_code_retrieval")
sys.modules["eval_code_retrieval"] = evaluator
privacy = load_script("check_public_benchmark_privacy")
leakage = load_script("check_retrieval_benchmark_leakage")
renderer = load_script("render_public_benchmark")
embedding_renderer = load_script("render_embedding_bakeoff")
matrix_runner = load_script("run_public_benchmark_matrix")
reranker_trainer = load_script("train_public_reranker")
reranker_renderer = load_script("render_public_reranker")


class PublicBenchmarkTest(unittest.TestCase):
    def test_coreb_hard_negatives_are_not_exported_as_relevant(self):
        rows = [
            {"query_id": "q1", "doc_id": "positive", "relevance": 2},
            {"query_id": "q1", "doc_id": "hard-negative", "relevance": 1},
            {"query_id": "q1", "doc_id": "irrelevant", "relevance": 0},
        ]
        self.assertEqual(
            coreb_exporter.positive_qrels(rows),
            [("q1", "positive", 2)],
        )

    def test_coreb_code_paths_preserve_language_extensions(self):
        document = coreb_exporter.code_document(
            {"code_id": "code-1", "code": "fn main() {}", "language": "cpp"},
            7,
        )
        self.assertEqual(document["metadata"]["path"], "documents/000007-code-1.cpp")

    def test_coreb_default_split_rejects_changed_files(self):
        with self.assertRaisesRegex(ValueError, "SHA-256 changed"):
            coreb_exporter.validate_hash("code_corpus", "0" * 64, "release_v2603")
        coreb_exporter.validate_hash("code_corpus", "0" * 64, "experimental")

    def test_matrix_modes_distinguish_production_and_forced_neural(self):
        self.assertEqual(
            matrix_runner.parse_modes("hybrid,blended,neural"),
            ["hybrid", "blended", "neural"],
        )

    def test_source_commit_override_tracks_the_benchmark_binary(self):
        self.assertEqual(
            matrix_runner.benchmark_revision(ROOT, "a" * 40),
            "a" * 40,
        )

    def test_manifest_covers_every_full_profile_task(self):
        manifest = exporter.load_manifest(
            ROOT / "benchmarks" / "public" / "manifest.json"
        )
        full = exporter.selected_tasks(manifest, "full", [])
        self.assertEqual(len(full), 20)
        self.assertEqual(set(full), set(manifest["tasks"]))
        self.assertGreaterEqual(
            manifest["profiles"]["public-core"]["minimum_queries"], 1000
        )
        self.assertIn(
            "codefeedback-st",
            manifest["profiles"]["public-core"]["tasks"],
        )
        self.assertTrue(
            set(manifest["profiles"]["reranker-train"]["tasks"]).isdisjoint(
                manifest["profiles"]["public-core"]["tasks"]
            )
        )
        challenge = manifest["profiles"]["sota-challenge"]
        self.assertGreaterEqual(challenge["minimum_queries"], 600)
        self.assertEqual(challenge["query_text_limit"], 2048)
        self.assertEqual(len(challenge["tasks"]), 6)
        self.assertIn("apps", challenge["tasks"])
        self.assertIn("CodeSearchNet-python", challenge["tasks"])
        self.assertIn("CodeSearchNet-java", challenge["tasks"])
        for task in challenge["tasks"]:
            self.assertEqual(challenge["task_options"][task]["sample_queries"], 100)
            self.assertEqual(challenge["task_options"][task]["sample_corpus"], 5000)

    def test_cached_datasets_must_meet_profile_query_minimum(self):
        manifest = {
            "profiles": {
                "public-core": {
                    "minimum_queries": 1000,
                }
            }
        }
        with self.assertRaisesRegex(
            ValueError, "profile public-core has 100 queries, below 1000"
        ):
            matrix_runner.validate_profile_query_count(
                manifest,
                "public-core",
                [{"counts": {"queries": 25}} for _ in range(4)],
            )

    def test_query_sampling_is_deterministic(self):
        qrels = [
            {"query_id": f"q{index}", "corpus_id": f"d{index}", "score": 1}
            for index in range(20)
        ]
        first = exporter.sampled_query_ids(qrels, 5, 42)
        second = exporter.sampled_query_ids(qrels, 5, 42)
        self.assertEqual(first, second)
        self.assertEqual(len(first), 5)

    def test_query_text_limit_comes_from_profile_or_override(self):
        manifest = {
            "profiles": {
                "plain": {},
                "challenge": {"query_text_limit": 2048},
            }
        }
        self.assertIsNone(matrix_runner.query_text_limit(manifest, "plain", None))
        self.assertEqual(
            matrix_runner.query_text_limit(manifest, "challenge", None),
            2048,
        )
        self.assertEqual(matrix_runner.query_text_limit(manifest, "challenge", 512), 512)
        with self.assertRaisesRegex(ValueError, "positive"):
            matrix_runner.query_text_limit(manifest, "plain", 0)

    def test_query_text_can_be_capped_for_prompt_dump_benchmarks(self):
        query = {"text": "abcdef"}
        self.assertEqual(evaluator.query_text(query, None), "abcdef")
        self.assertEqual(evaluator.query_text(query, 3), "abc")

    def test_query_partitions_are_disjoint_and_complete(self):
        qrels = [
            {"query_id": f"q{index}", "corpus_id": f"d{index}", "score": 1}
            for index in range(100)
        ]
        left = exporter.sampled_query_ids(
            qrels,
            None,
            42,
            {"modulus": 2, "residues": [0]},
        )
        right = exporter.sampled_query_ids(
            qrels,
            None,
            42,
            {"modulus": 2, "residues": [1]},
        )
        self.assertFalse(left & right)
        self.assertEqual(left | right, {f"q{index}" for index in range(100)})

    def test_language_extensions_are_neutral_and_portable(self):
        self.assertEqual(exporter.safe_extension("Python"), "py")
        self.assertEqual(exporter.safe_extension("C++"), "cpp")
        self.assertEqual(exporter.safe_extension("unknown"), "txt")

    def test_corpus_sampling_keeps_qrel_documents_and_is_deterministic(self):
        corpus = [{"_id": f"d{index}"} for index in range(100)]
        required = {"d3", "d77"}
        first = exporter.sampled_corpus_indices(corpus, required, 10, 42)
        second = exporter.sampled_corpus_indices(corpus, required, 10, 42)
        selected = {corpus[index]["_id"] for index in first}
        self.assertEqual(first, second)
        self.assertEqual(len(first), 10)
        self.assertTrue(required <= selected)

    def test_privacy_check_rejects_user_paths_and_supplied_terms(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            clean = root / "clean.json"
            clean.write_text('{"source":"public"}\n', encoding="utf-8")
            self.assertEqual(privacy.violations([clean]), [])
            unsafe = root / "unsafe.json"
            unsafe.write_text('{"path":"/home/example/private"}\n', encoding="utf-8")
            self.assertTrue(privacy.violations([unsafe]))
            named = root / "named.json"
            named.write_text('{"source":"sensitive-corpus"}\n', encoding="utf-8")
            self.assertTrue(privacy.violations([named], ["sensitive-corpus"]))

    def test_leakage_check_matches_long_query_text(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "src"
            dataset = root / "dataset"
            source.mkdir()
            dataset.mkdir()
            query = "where is the public payment retry implementation located"
            (dataset / "queries.jsonl").write_text(
                json.dumps({"_id": "q1", "text": query}) + "\n",
                encoding="utf-8",
            )
            (source / "clean.rs").write_text(
                'const LABEL: &str = "generic ranking";\n',
                encoding="utf-8",
            )
            self.assertEqual(leakage.find_leaks(source, [dataset]), [])
            (source / "leak.rs").write_text(
                f'const QUERY: &str = "{query}";\n',
                encoding="utf-8",
            )
            self.assertTrue(leakage.find_leaks(source, [dataset]))

    def test_renderer_uses_aggregate_metrics(self):
        metrics = {
            name: {
                "mean": 0.5,
                "standard_deviation": 0.0,
                "coefficient_of_variation": 0.0,
                "minimum": 0.5,
                "maximum": 0.5,
            }
            for name in (
                "ndcg_at_10",
                "mrr_at_10",
                "precision_at_5",
                "recall_at_20",
                "cold_latency_p50_ms",
                "cold_latency_p95_ms",
                "warm_latency_p50_ms",
                "warm_latency_p95_ms",
                "index_ms",
                "hash_enhancement_ms",
                "neural_enhancement_ms",
                "daemon_startup_ms",
                "neural_model_ready_ms",
                "index_size_bytes",
                "peak_child_rss_bytes",
            )
        }
        matrix = {
            "ivygrep_commit": "abc123",
            "profile": "public-core",
            "tasks": ["one"],
            "queries": 500,
            "repetitions": 3,
            "modes": ["hash"],
            "summary": {"hash": {"metrics": metrics}},
        }
        report = renderer.markdown(matrix)
        self.assertIn("500", report)
        self.assertIn("0.5000", report)
        self.assertIn("--profile public-core", report)
        self.assertNotIn("/home/", report)

    def test_renderer_reproduce_command_uses_matrix_profile(self):
        metrics = {
            name: {
                "mean": 0.5,
                "standard_deviation": 0.0,
                "coefficient_of_variation": 0.0,
                "minimum": 0.5,
                "maximum": 0.5,
            }
            for name in (
                "ndcg_at_10",
                "mrr_at_10",
                "precision_at_5",
                "recall_at_20",
                "cold_latency_p50_ms",
                "cold_latency_p95_ms",
                "warm_latency_p50_ms",
                "warm_latency_p95_ms",
                "index_ms",
                "hash_enhancement_ms",
                "neural_enhancement_ms",
                "daemon_startup_ms",
                "neural_model_ready_ms",
                "index_size_bytes",
                "peak_child_rss_bytes",
            )
        }
        matrix = {
            "ivygrep_commit": "abc123",
            "profile": "sota-challenge",
            "tasks": ["one"],
            "queries": 600,
            "repetitions": 1,
            "modes": ["hash"],
            "summary": {"hash": {"metrics": metrics}},
        }
        report = renderer.markdown(matrix)
        self.assertIn("--profile sota-challenge", report)
        self.assertIn("--output public-sota-challenge-results.json", report)

        matrix["query_text_limit"] = 2048
        report = renderer.markdown(matrix)
        self.assertIn("Query text limit: 2048 characters", report)
        self.assertIn("--max-query-chars 2048", report)

        html = renderer.html(matrix)
        self.assertIn("public-sota-challenge-results.json", html)
        self.assertNotIn("public-sota-challenge.md", html)
        self.assertIn("sota-challenge", html)
        self.assertIn("query char limit", html)

    def test_dataset_scope_note_discloses_sampling_and_license_limits(self):
        matrix = {
            "tasks": ["codefeedback-st"],
            "results": [
                {
                    "dataset": "codefeedback-st",
                    "dataset_provenance": {
                        "license": "not-declared-in-dataset-card",
                        "sample": {
                            "query_limit": 99,
                            "corpus_limit": 5000,
                        },
                    },
                }
            ],
        }
        note = renderer.dataset_scope_note(matrix)
        self.assertIn("99 queries, 5000 documents", note)
        self.assertIn("license", note.lower())
        self.assertIn("do not redistribute", note)

    def test_renderer_compares_matching_frozen_baseline(self):
        def matrix(commit: str, ndcg: float) -> dict:
            metrics = {
                name: {
                    "mean": ndcg,
                    "standard_deviation": 0.0,
                    "coefficient_of_variation": 0.0,
                    "minimum": ndcg,
                    "maximum": ndcg,
                }
                for name in (
                    "ndcg_at_10",
                    "mrr_at_10",
                    "precision_at_5",
                    "recall_at_20",
                    "cold_latency_p50_ms",
                    "cold_latency_p95_ms",
                    "warm_latency_p50_ms",
                    "warm_latency_p95_ms",
                    "index_ms",
                    "hash_enhancement_ms",
                    "neural_enhancement_ms",
                    "daemon_startup_ms",
                    "neural_model_ready_ms",
                    "index_size_bytes",
                    "peak_child_rss_bytes",
                )
            }
            return {
                "ivygrep_commit": commit,
                "profile": "public-core",
                "tasks": ["one"],
                "queries": 1000,
                "repetitions": 3,
                "modes": ["hash"],
                "summary": {"hash": {"metrics": metrics}},
                "task_summary": {
                    "one": {
                        "hash": {
                            "ndcg_at_10": metrics["ndcg_at_10"],
                            "mrr_at_10": metrics["mrr_at_10"],
                            "recall_at_20": metrics["recall_at_20"],
                        }
                    }
                },
            }

        report = renderer.markdown(matrix("current", 0.55), matrix("baseline", 0.50))
        self.assertIn("Change from frozen baseline", report)
        self.assertIn("+10.00%", report)
        self.assertIn("+0.0500", report)

    def test_matrix_uses_global_query_latency_percentiles(self):
        def result(dataset: str, queries: int, latency: float) -> dict:
            metrics = {name: 0.5 for name in matrix_runner.QUALITY_METRICS}
            metrics.update(
                {
                    "dataset": dataset,
                    "mode": "hash",
                    "run": 1,
                    "queries": queries,
                    "index_ms": 1.0,
                    "hash_enhancement_ms": 1.0,
                    "neural_enhancement_ms": 0.0,
                    "daemon_startup_ms": 1.0,
                    "neural_model_ready_ms": 0.0,
                    "index_size_bytes": 1,
                    "peak_child_rss_bytes": 1,
                    "details": [
                        {
                            "cold_latency_ms": latency,
                            "warm_latency_ms": latency,
                        }
                        for _ in range(queries)
                    ],
                }
            )
            return metrics

        summary = matrix_runner.aggregate_runs(
            [result("small", 1, 100.0), result("large", 19, 1.0)],
            ["hash"],
            1,
        )
        run = summary["hash"]["runs"][0]
        self.assertEqual(run["cold_latency_p95_ms"], 1.0)
        self.assertEqual(run["warm_latency_p95_ms"], 1.0)

    def test_publication_result_omits_per_query_details(self):
        result = {
            "dataset": "public",
            "retrieval_provenance": {"queries_with_neural_execution": 10},
            "details": [{"query_id": "q1"}],
        }
        self.assertEqual(
            matrix_runner.publication_result(result),
            {
                "dataset": "public",
                "retrieval_provenance": {"queries_with_neural_execution": 10},
            },
        )

    def test_embedding_partial_must_match_selected_binary(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            matrix_path = root / "matrix.json"
            partial_path = root / "partial.json"
            metrics = {
                name: {
                    "mean": 1.0,
                    "standard_deviation": 0.0,
                    "coefficient_of_variation": 0.0,
                    "minimum": 1.0,
                    "maximum": 1.0,
                }
                for name in embedding_renderer.METRICS
            }
            matrix_path.write_text(
                json.dumps(
                    {
                        "ivygrep_commit": "abc123",
                        "harness_sha256": {
                            "eval_code_retrieval.py": embedding_renderer.sha256_file(
                                ROOT / "scripts" / "eval_code_retrieval.py"
                            )
                        },
                        "neural_models": [{"profile": "static-retrieval-v1"}],
                        "summary": {"neural": {"metrics": metrics}},
                        "queries": 100,
                        "tasks": ["public"],
                        "repetitions": 1,
                        "task_summary": {
                            "public": {"neural": {"ndcg_at_10": metrics["ndcg_at_10"]}}
                        },
                        "results": [{"binary": {"sha256": "selected"}}],
                    }
                ),
                encoding="utf-8",
            )
            partial_path.write_text(
                json.dumps(
                    {
                        "dataset": "public",
                        "queries": 25,
                        "binary": {"sha256": "different"},
                        "index_configuration": {
                            "neural_model": {"profile": "general"}
                        },
                        **{name: 1.0 for name in embedding_renderer.METRICS},
                    }
                ),
                encoding="utf-8",
            )
            manifest = {
                "screening_budget": {},
                "candidates": {
                    "static-retrieval-v1": {"status": "selected-default"},
                    "general": {"status": "rejected"},
                },
            }
            with self.assertRaisesRegex(ValueError, "binary does not match"):
                embedding_renderer.build_report(
                    ROOT,
                    manifest,
                    {"static-retrieval-v1": matrix_path},
                    {"general": partial_path},
                )

    def test_reranker_features_are_bounded_and_deterministic(self):
        candidate = {
            "file_path": "src/search.rs",
            "total_score": 0.5,
            "hit_count": 2,
            "sources": ["lexical", "semantic"],
            "preview": "fn route_query() { learned_rerank(); }",
        }
        first = reranker_trainer.feature_vector(
            "route learned query", candidate, 0
        )
        second = reranker_trainer.feature_vector(
            "route learned query", candidate, 0
        )
        self.assertEqual(first, second)
        self.assertEqual(len(first), len(reranker_trainer.FEATURE_NAMES))
        self.assertTrue(all(math.isfinite(value) for value in first))
        self.assertTrue(all(0.0 <= value <= 1.0 for value in first))

    def test_public_reranker_evidence_passes_acceptance_gate(self):
        report = reranker_renderer.build_report(
            ROOT / "benchmarks" / "public" / "reranker_model.json",
            ROOT
            / "docs"
            / "benchmarks"
            / "public-reranker-deterministic-results.json",
            ROOT
            / "docs"
            / "benchmarks"
            / "public-reranker-learned-results.json",
        )
        self.assertTrue(report["model"]["offline_evaluation"]["gate"]["passed"])
        self.assertTrue(report["integrated_evaluation"]["gate"]["passed"])
        self.assertGreaterEqual(
            report["integrated_evaluation"]["metrics"]["ndcg_at_10"][
                "relative_delta"
            ],
            0.05,
        )

    def test_reused_result_must_match_binary_and_dataset(self):
        with tempfile.TemporaryDirectory() as temp:
            dataset = Path(temp) / "public"
            dataset.mkdir()
            (dataset / "provenance.json").write_text(
                json.dumps({"checksums": {"corpus.jsonl": "abc"}}),
                encoding="utf-8",
            )
            result = {
                "dataset": "public",
                "mode": "hash",
                "binary": {"sha256": "binary"},
                "dataset_provenance": {"checksums": {"corpus.jsonl": "abc"}},
                "query_text_limit": 2048,
            }
            matrix_runner.validate_reused_result(
                result,
                dataset,
                "hash",
                "binary",
                2048,
            )
            with self.assertRaises(ValueError):
                matrix_runner.validate_reused_result(
                    result,
                    dataset,
                    "hash",
                    "different",
                    2048,
                )
            with self.assertRaises(ValueError):
                matrix_runner.validate_reused_result(
                    result,
                    dataset,
                    "hash",
                    "binary",
                    None,
                )


if __name__ == "__main__":
    unittest.main()
