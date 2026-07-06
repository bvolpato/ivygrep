import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "eval_relevance.py"
SPEC = importlib.util.spec_from_file_location("eval_relevance", SCRIPT)
eval_relevance = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = eval_relevance
SPEC.loader.exec_module(eval_relevance)


class RelevanceEvaluationTest(unittest.TestCase):
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
