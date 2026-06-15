import importlib.util
import json
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
evaluator = load_script("eval_code_retrieval")
sys.modules["eval_code_retrieval"] = evaluator
privacy = load_script("check_public_benchmark_privacy")
leakage = load_script("check_retrieval_benchmark_leakage")
renderer = load_script("render_public_benchmark")
matrix_runner = load_script("run_public_benchmark_matrix")


class PublicBenchmarkTest(unittest.TestCase):
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

    def test_query_sampling_is_deterministic(self):
        qrels = [
            {"query_id": f"q{index}", "corpus_id": f"d{index}", "score": 1}
            for index in range(20)
        ]
        first = exporter.sampled_query_ids(qrels, 5, 42)
        second = exporter.sampled_query_ids(qrels, 5, 42)
        self.assertEqual(first, second)
        self.assertEqual(len(first), 5)

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
        self.assertNotIn("/home/", report)

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
        result = {"dataset": "public", "details": [{"query_id": "q1"}]}
        self.assertEqual(
            matrix_runner.publication_result(result),
            {"dataset": "public"},
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
            }
            matrix_runner.validate_reused_result(
                result,
                dataset,
                "hash",
                "binary",
            )
            with self.assertRaises(ValueError):
                matrix_runner.validate_reused_result(
                    result,
                    dataset,
                    "hash",
                    "different",
                )


if __name__ == "__main__":
    unittest.main()
