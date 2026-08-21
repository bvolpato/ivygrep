import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_public_relevance",
    ROOT / "scripts" / "check_public_relevance.py",
)
check_public_relevance = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(check_public_relevance)


class PublicRelevanceGateTest(unittest.TestCase):
    def setUp(self):
        self.gates = {
            "profile": "public-core",
            "required_modes": ["lexical", "hybrid"],
            "datasets": {
                "public-task": {
                    "minimum_ndcg_at_10": 0.6,
                    "minimum_recall_at_20": 0.7,
                    "retention_modes": ["lexical"],
                    "retained_query_ids": ["q1", "q2"],
                }
            },
        }
        self.matrix = {
            "profile": "public-core",
            "modes": ["lexical", "hybrid"],
            "task_summary": {
                "public-task": {
                    mode: {
                        "ndcg_at_10": {"mean": 0.8},
                        "recall_at_20": {"mean": 0.9},
                    }
                    for mode in ("lexical", "hybrid")
                }
            },
            "results": [
                {"dataset": "public-task", "mode": "lexical", "run": 1},
                {"dataset": "public-task", "mode": "hybrid", "run": 1},
            ],
        }

    def write_details(self, root, details):
        path = root / "public-task-lexical-run-1.json"
        path.write_text(json.dumps({"details": details}), encoding="utf-8")

    def test_accepts_healthy_dataset_metrics_and_retained_queries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_details(
                root,
                [
                    {"query_id": "q1", "recall_at_20": 1.0},
                    {"query_id": "q2", "recall_at_20": 1.0},
                ],
            )
            self.assertEqual(
                check_public_relevance.validate_matrix(self.matrix, self.gates, root),
                [],
            )

    def test_rejects_dataset_regression_even_when_other_modes_are_healthy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_details(
                root,
                [
                    {"query_id": "q1", "recall_at_20": 1.0},
                    {"query_id": "q2", "recall_at_20": 1.0},
                ],
            )
            self.matrix["task_summary"]["public-task"]["hybrid"]["ndcg_at_10"][
                "mean"
            ] = 0.59
            errors = check_public_relevance.validate_matrix(
                self.matrix,
                self.gates,
                root,
            )
            self.assertTrue(any("public-task/hybrid: ndcg_at_10" in error for error in errors))

    def test_rejects_previously_successful_query_despite_healthy_aggregate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_details(
                root,
                [
                    {"query_id": "q1", "recall_at_20": 1.0},
                    {"query_id": "q2", "recall_at_20": 0.0},
                ],
            )
            errors = check_public_relevance.validate_matrix(
                self.matrix,
                self.gates,
                root,
            )
            self.assertTrue(any("lost every relevant result for q2" in error for error in errors))

    def test_rejects_missing_modes_queries_and_raw_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.matrix["modes"] = ["lexical"]
            errors = check_public_relevance.validate_matrix(
                self.matrix,
                self.gates,
                root,
            )
            self.assertTrue(any("missing required retrieval modes: hybrid" in error for error in errors))
            self.assertTrue(any("missing raw result file" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
