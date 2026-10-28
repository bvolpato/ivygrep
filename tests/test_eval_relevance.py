import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "eval_relevance.py"
SPEC = importlib.util.spec_from_file_location("eval_relevance", SCRIPT)
eval_relevance = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = eval_relevance
SPEC.loader.exec_module(eval_relevance)


class RelevanceEvaluationTest(unittest.TestCase):
    def test_candidate_audit_finds_deep_results_without_changing_final_ranks(self):
        case = eval_relevance.QueryCase("coverage", "find network handling", [
            eval_relevance.Judgment("src/network.rs", 3),
            eval_relevance.Judgment("src/protocol.rs", 2),
        ])

        def query_output(command, **kwargs):
            if "--status" in command:
                return '[{"has_neural_vectors": true}]'
            if "--add" in command or "--enhance-internal" in command:
                return "[]"
            paths = ["src/network.rs"]
            if "--no-limit" in command:
                paths.append("src/protocol.rs")
            return json.dumps([
                {"file_path": path, "hits": [{"sources": ["neural"], "neural_executed": True}]}
                for path in paths
            ])

        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "ig"
            binary.touch()
            for audited, expected_recall in ((False, 0.5), (True, 1.0)):
                with self.subTest(audited=audited):
                    arguments = [
                        "eval_relevance", "--repo", temporary, "--binary", str(binary),
                        "--skip-build", "--neural", "--json",
                        "--audit-recall" if audited else "--no-audit-recall",
                    ]
                    output = io.StringIO()
                    with mock.patch.object(sys, "argv", arguments), \
                         mock.patch.object(eval_relevance, "load_cases", return_value=[case]), \
                         mock.patch.object(eval_relevance, "run", side_effect=query_output), \
                         contextlib.redirect_stdout(output):
                        self.assertEqual(eval_relevance.main(), 0)
                    report = json.loads(output.getvalue())
                    self.assertIs(report["candidate_recall_audited"], audited)
                    self.assertEqual(report["mean_candidate_recall"], expected_recall)
                    self.assertEqual(report["mean_final_recall"], 0.5)
                    self.assertEqual(report["mean_mrr"], 1.0)
                    self.assertEqual(report["mean_recall5"], 0.5)
                    self.assertEqual(report["neural_queries_executed"], 1)
                    self.assertEqual(report["query_metrics"][0], {
                        "id": "coverage", "mrr": 1.0,
                        "ndcg10": report["mean_ndcg10"], "final_recall": 0.5,
                        "candidate_recall": expected_recall,
                        "audit_stage": eval_relevance.AUDIT_SATISFIED,
                        "neural_executed": True,
                    })

    def test_parse_search_output_deduplicates_paths_and_collects_sources(self):
        output = """
        [
          {
            "file_path": "src/search.rs",
            "hits": [
              {"sources": ["lexical", "hash"], "neural_executed": false}
            ]
          },
          {
            "file_path": "src/search.rs",
            "hits": [
              {"sources": ["literal"], "neural_executed": false}
            ]
          }
        ]
        """
        parsed = eval_relevance.parse_search_output(output)

        self.assertEqual(parsed.paths, ["src/search.rs"])
        self.assertEqual(parsed.sources, {"lexical", "hash", "literal"})

    def test_neural_execution_is_unobservable_without_hits(self):
        self.assertIsNone(eval_relevance.neural_execution_status([]))
        self.assertTrue(
            eval_relevance.neural_execution_status([{"neural_executed": True}])
        )
        self.assertFalse(
            eval_relevance.neural_execution_status([{"neural_executed": False}])
        )

    def test_first_relevant_rank_uses_primary_judgments(self):
        judgments = [
            eval_relevance.Judgment("docs/**", 1),
            eval_relevance.Judgment("src/search.rs", 3),
        ]

        rank = eval_relevance.first_relevant_rank(
            ["docs/search.md", "src/search.rs"],
            judgments,
        )

        self.assertEqual(rank, 2)

    def test_relevant_recall_counts_primary_patterns_once(self):
        judgments = [
            eval_relevance.Judgment("src/search.rs", 3),
            eval_relevance.Judgment("src/indexer.rs", 2),
            eval_relevance.Judgment("docs/**", 1),
        ]

        recall = eval_relevance.relevant_recall(
            ["src/search.rs", "src/search.rs", "docs/search.md"],
            judgments,
        )

        self.assertEqual(recall, 0.5)

    def test_classify_audit_stage(self):
        judgments = [eval_relevance.Judgment("src/search.rs", 3)]

        self.assertEqual(
            eval_relevance.classify_audit_stage(2, 2, 1.0, 5, judgments),
            eval_relevance.AUDIT_SATISFIED,
        )
        self.assertEqual(
            eval_relevance.classify_audit_stage(8, 8, 1.0, 5, judgments),
            eval_relevance.AUDIT_FIRST_USEFUL_LOW,
        )
        self.assertEqual(
            eval_relevance.classify_audit_stage(None, 3, 1.0, 5, judgments),
            eval_relevance.AUDIT_CANDIDATE_BUDGET,
        )
        self.assertEqual(
            eval_relevance.classify_audit_stage(None, None, 1.0, 5, judgments),
            eval_relevance.AUDIT_AFTER_FILTERING,
        )
        self.assertEqual(
            eval_relevance.classify_audit_stage(None, None, 0.0, 5, judgments),
            eval_relevance.AUDIT_BEFORE_FUSION,
        )

    def test_audit_stage_counts_include_not_audited_rows(self):
        rows = [
            {"audit_stage": eval_relevance.AUDIT_SATISFIED},
            {"audit_stage": eval_relevance.AUDIT_NOT_RUN},
        ]

        counts = eval_relevance.audit_stage_counts(rows)

        self.assertEqual(counts[eval_relevance.AUDIT_SATISFIED], 1)
        self.assertEqual(counts[eval_relevance.AUDIT_NOT_RUN], 1)


if __name__ == "__main__":
    unittest.main()
