import importlib.util
from unittest import mock
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "eval_code_retrieval.py"
SPEC = importlib.util.spec_from_file_location("eval_code_retrieval", SCRIPT)
eval_code_retrieval = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(eval_code_retrieval)


class RetrievalMetricsTest(unittest.TestCase):
    def test_graded_metrics_reward_correct_order(self):
        judgments = {"best": 3, "related": 1}
        good = eval_code_retrieval.score_query(["best", "related"], judgments)
        bad = eval_code_retrieval.score_query(["missing", "related", "best"], judgments)
        self.assertGreater(good["ndcg_at_10"], bad["ndcg_at_10"])
        self.assertGreater(good["mrr_at_10"], bad["mrr_at_10"])
        self.assertEqual(good["recall_at_20"], 1.0)

    def test_missing_results_score_zero(self):
        score = eval_code_retrieval.score_query([], {"expected": 2})
        self.assertEqual(score["ndcg_at_10"], 0.0)
        self.assertEqual(score["mrr_at_10"], 0.0)
        self.assertEqual(score["precision_at_5"], 0.0)
        self.assertEqual(score["recall_at_20"], 0.0)

    def test_qrels_parser_accepts_beir_tsv(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "qrels.tsv"
            path.write_text(
                "query-id\tcorpus-id\tscore\nq1\td1\t3\nq1\td2\t1\n",
                encoding="utf-8",
            )
            self.assertEqual(
                eval_code_retrieval.load_qrels(path),
                {"q1": {"d1": 3, "d2": 1}},
            )

    def test_materialize_corpus_sanitizes_parent_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            dataset = root / "dataset"
            repo = root / "repo"
            dataset.mkdir()
            (dataset / "corpus.jsonl").write_text(
                '{"_id":"unsafe","text":"body","metadata":{"path":"../escape.rs"}}\n',
                encoding="utf-8",
            )
            mapping = eval_code_retrieval.materialize_corpus(dataset, repo)
            self.assertEqual(mapping, {"documents/000000-unsafe.txt": "unsafe"})
            self.assertFalse((root / "escape.rs").exists())

    def test_query_modes_are_explicit(self):
        self.assertEqual(eval_code_retrieval.query_args("lexical"), ["--lexical-only"])
        self.assertEqual(eval_code_retrieval.query_args("hash"), ["--hash"])
        self.assertEqual(eval_code_retrieval.query_args("hybrid"), [])
        self.assertEqual(eval_code_retrieval.query_args("neural"), [])

    def test_search_command_terminates_option_parsing_before_query(self):
        command = eval_code_retrieval.search_command(
            Path("ig"),
            "neural",
            20,
            "-----Input-----\nexample",
        )
        self.assertEqual(command[-2], "--")
        self.assertEqual(command[-1], "-----Input-----\nexample")

    def test_daemon_endpoint_matches_platform_transport(self):
        home = Path("benchmark-home")
        with mock.patch.object(eval_code_retrieval.os, "name", "nt"):
            self.assertEqual(
                eval_code_retrieval.daemon_endpoint_path(home),
                home / "daemon.port",
            )
        with mock.patch.object(eval_code_retrieval.os, "name", "posix"):
            self.assertEqual(
                eval_code_retrieval.daemon_endpoint_path(home),
                home / "daemon.sock",
            )

    def test_warm_query_path_is_explicit_for_lexical_mode(self):
        self.assertEqual(
            eval_code_retrieval.warm_query_path("lexical"), "local-process"
        )
        self.assertEqual(eval_code_retrieval.warm_query_path("hash"), "daemon")
        self.assertEqual(eval_code_retrieval.warm_query_path("neural"), "daemon")

    def test_neural_process_cold_sampling_loads_model_once(self):
        queries = [{"_id": str(index)} for index in range(3)]
        self.assertEqual(
            eval_code_retrieval.process_cold_queries("neural", queries),
            queries[:1],
        )
        self.assertEqual(
            eval_code_retrieval.process_cold_queries("hash", queries),
            queries,
        )

    def test_support_path_detection_is_specific(self):
        self.assertTrue(eval_code_retrieval.is_support_path("tests/search_test.rs"))
        self.assertTrue(eval_code_retrieval.is_support_path("docs/examples/basic.md"))
        self.assertFalse(eval_code_retrieval.is_support_path("src/search.rs"))
        self.assertFalse(eval_code_retrieval.is_support_path("src/contest.rs"))

    def test_support_query_detection(self):
        self.assertTrue(
            eval_code_retrieval.query_targets_support(
                "show an example test for retry behavior"
            )
        )
        self.assertFalse(
            eval_code_retrieval.query_targets_support(
                "where is retry behavior implemented"
            )
        )


if __name__ == "__main__":
    unittest.main()
