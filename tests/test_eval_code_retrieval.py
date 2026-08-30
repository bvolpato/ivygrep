import importlib.util
from unittest import mock
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "eval_code_retrieval.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("eval_code_retrieval", SCRIPT)
eval_code_retrieval = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(eval_code_retrieval)


class RetrievalMetricsTest(unittest.TestCase):
    def test_query_selection_preserves_dataset_order_and_rejects_missing_ids(self):
        queries = [{"_id": "q2"}, {"_id": "q1"}, {"_id": "q3"}]
        self.assertEqual(
            eval_code_retrieval.selected_queries(queries, ["q1", "q2", "q1"]),
            queries[:2],
        )
        self.assertIs(eval_code_retrieval.selected_queries(queries, []), queries)
        with self.assertRaisesRegex(ValueError, "missing: q4"):
            eval_code_retrieval.selected_queries(queries, ["q4"])

    def test_graded_metrics_reward_correct_order(self):
        judgments = {"best": 3, "related": 1}
        good = eval_code_retrieval.score_query(["best", "related"], judgments)
        bad = eval_code_retrieval.score_query(["missing", "related", "best"], judgments)
        self.assertGreater(good["ndcg_at_10"], bad["ndcg_at_10"])
        self.assertGreater(good["mrr_at_10"], bad["mrr_at_10"])
        self.assertEqual(good["recall_at_5"], 1.0)
        self.assertEqual(good["recall_at_10"], 1.0)
        self.assertEqual(good["recall_at_20"], 1.0)
        self.assertEqual(good["exact_at_5"], 1.0)
        self.assertEqual(good["exact_at_10"], 1.0)
        self.assertEqual(good["exact_at_20"], 1.0)

    def test_missing_results_score_zero(self):
        score = eval_code_retrieval.score_query([], {"expected": 2})
        self.assertEqual(score["ndcg_at_10"], 0.0)
        self.assertEqual(score["mrr_at_10"], 0.0)
        self.assertEqual(score["precision_at_5"], 0.0)
        self.assertEqual(score["recall_at_5"], 0.0)
        self.assertEqual(score["recall_at_10"], 0.0)
        self.assertEqual(score["recall_at_20"], 0.0)
        self.assertEqual(score["exact_at_5"], 0.0)
        self.assertEqual(score["exact_at_10"], 0.0)
        self.assertEqual(score["exact_at_20"], 0.0)

    def test_exact_recall_requires_every_memory(self):
        score = eval_code_retrieval.score_query(
            ["needed-a", "noise", "needed-b"],
            {"needed-a": 1, "needed-b": 1, "needed-c": 1},
        )
        self.assertEqual(score["recall_at_5"], 2 / 3)
        self.assertEqual(score["exact_at_5"], 0.0)

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
        self.assertEqual(eval_code_retrieval.query_args("blended"), [])
        self.assertEqual(
            eval_code_retrieval.query_args("neural"), ["--force-neural"]
        )

    def test_search_command_terminates_option_parsing_before_query(self):
        command = eval_code_retrieval.search_command(
            Path("ig"),
            "neural",
            20,
            "-----Input-----\nexample",
        )
        self.assertEqual(command[-2], "--")
        self.assertEqual(command[-1], "-----Input-----\nexample")

    def test_memory_query_expansion_keeps_original_and_adds_fixed_facets(self):
        variants = eval_code_retrieval.expanded_query_texts(
            "Plan a weekend away.",
            "memory-facets",
        )
        self.assertEqual(variants[0], "Plan a weekend away.")
        self.assertEqual(len(variants), 4)
        self.assertTrue(any("preferences" in variant for variant in variants[1:]))
        self.assertEqual(
            eval_code_retrieval.expanded_query_texts(
                "Plan a weekend away.",
                "memory-action",
            ),
            [variants[0], variants[3]],
        )

    def test_memory_query_expansion_supports_probe_pairs(self):
        all_variants = eval_code_retrieval.expanded_query_texts(
            "Plan a weekend away.",
            "memory-facets",
        )
        paired = eval_code_retrieval.expanded_query_texts(
            "Plan a weekend away.",
            "memory-context-action",
        )
        self.assertEqual(paired, [all_variants[0], all_variants[1], all_variants[3]])

    def test_memory_query_expansion_can_bound_probe_text(self):
        query = "0123456789"
        variants = eval_code_retrieval.expanded_query_texts(
            query,
            "memory-context",
            5,
        )
        self.assertEqual(variants[0], query)
        self.assertTrue(variants[1].endswith("01234"))

    def test_fuse_search_outputs_rewards_files_found_by_multiple_probes(self):
        fused = eval_code_retrieval.fuse_search_outputs(
            [
                [{"file_path": "a.md"}, {"file_path": "b.md"}],
                [{"file_path": "b.md"}, {"file_path": "c.md"}],
            ]
        )
        self.assertEqual(fused[0]["file_path"], "b.md")

    def test_single_query_keeps_native_file_score(self):
        native = {"file_path": "src/request.rs", "total_score": 3.0, "hit_count": 2}
        output = eval_code_retrieval.fuse_search_outputs([[native]])
        self.assertEqual(output[0]["total_score"], 3.0)
        self.assertEqual(native["total_score"], 3.0)

    def test_expansion_ranking_does_not_replace_native_score(self):
        output = eval_code_retrieval.fuse_search_outputs(
            [
                [
                    {"file_path": "a.rs", "total_score": 8.0},
                    {"file_path": "b.rs", "total_score": 3.0},
                ],
                [{"file_path": "b.rs", "total_score": 9.0}],
            ]
        )
        self.assertEqual(output[0]["file_path"], "b.rs")
        self.assertEqual(output[0]["total_score"], 3.0)
        self.assertAlmostEqual(output[0]["fusion_score"], 1 / 62 + 1 / 61)

    def test_fuse_search_outputs_can_anchor_original_ranking(self):
        outputs = [
            [{"file_path": "z-original.md"}],
            [{"file_path": "a-probe.md"}],
        ]
        unweighted = eval_code_retrieval.fuse_search_outputs(outputs, rrf_k=20)
        anchored = eval_code_retrieval.fuse_search_outputs(
            outputs,
            rrf_k=20,
            original_weight=2,
        )
        self.assertEqual(unweighted[0]["file_path"], "a-probe.md")
        self.assertEqual(anchored[0]["file_path"], "z-original.md")

    def test_parallel_search_commands_preserve_probe_order(self):
        commands = [["ig", "first"], ["ig", "second"], ["ig", "third"]]
        with mock.patch.object(
            eval_code_retrieval,
            "run_json",
            side_effect=lambda command, _cwd, _env: (command[-1], 0.0),
        ):
            outputs, elapsed_ms = eval_code_retrieval.run_search_commands(
                commands,
                Path("."),
                {},
                3,
            )
        self.assertEqual(outputs, ["first", "second", "third"])
        self.assertGreaterEqual(elapsed_ms, 0.0)

    def test_query_scope_stays_inside_materialized_repo(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            scope = repo / "users" / "user7"
            scope.mkdir(parents=True)
            query = {"metadata": {"scope": "users/user7"}}
            self.assertEqual(
                eval_code_retrieval.query_scope(query, repo),
                "users/user7/**",
            )
            command = eval_code_retrieval.search_command(
                Path("ig"),
                "neural",
                20,
                "what should I remember?",
                "users/user7/**",
                ["users/user7/s9.md"],
            )
            self.assertEqual(
                command[-6:],
                [
                    "--include",
                    "users/user7/**",
                    "--exclude",
                    "users/user7/s9.md",
                    "--",
                    "what should I remember?",
                ],
            )

    def test_query_scope_rejects_parent_path(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError, "unsafe query scope"):
                eval_code_retrieval.query_scope(
                    {"metadata": {"scope": "../private"}},
                    Path(temp),
                )

    def test_query_excludes_reject_parent_path(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError, "unsafe query exclude glob"):
                eval_code_retrieval.query_exclude_globs(
                    {"metadata": {"exclude_globs": ["../private.md"]}},
                    Path(temp),
                )

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
        self.assertEqual(eval_code_retrieval.warm_query_path("blended"), "daemon")
        self.assertEqual(eval_code_retrieval.warm_query_path("neural"), "daemon")

    def test_neural_process_cold_sampling_loads_model_once(self):
        queries = [{"_id": str(index)} for index in range(3)]
        self.assertEqual(
            eval_code_retrieval.process_cold_queries("neural", queries),
            queries[:1],
        )
        self.assertEqual(
            eval_code_retrieval.process_cold_queries("blended", queries),
            queries[:1],
        )
        self.assertEqual(
            eval_code_retrieval.process_cold_queries("hash", queries),
            queries,
        )

    def test_neural_execution_is_unobservable_without_hits(self):
        self.assertIsNone(eval_code_retrieval.neural_execution_status([]))
        self.assertTrue(
            eval_code_retrieval.neural_execution_status(
                [{"neural_executed": True}]
            )
        )
        self.assertFalse(
            eval_code_retrieval.neural_execution_status(
                [{"neural_executed": False}]
            )
        )

    def test_peak_rss_is_sampled_after_daemon_wait(self):
        events = []

        class FakeProcess:
            def poll(self):
                events.append("poll")
                return None

            def terminate(self):
                events.append("terminate")

            def send_signal(self, _signal):
                events.append("signal")

            def wait(self, timeout):
                events.append(("wait", timeout))

            def kill(self):
                events.append("kill")

        daemon_log = mock.Mock()
        daemon_log.close.side_effect = lambda: events.append("close")
        with mock.patch.object(
            eval_code_retrieval,
            "peak_child_rss_bytes",
            side_effect=lambda: events.append("rss") or 123,
        ):
            peak_rss = eval_code_retrieval.stop_daemon_and_measure_peak_rss(
                FakeProcess(), daemon_log
            )

        self.assertEqual(peak_rss, 123)
        wait_index = next(
            index
            for index, event in enumerate(events)
            if isinstance(event, tuple) and event[0] == "wait"
        )
        self.assertLess(wait_index, events.index("rss"))
        self.assertLess(events.index("close"), events.index("rss"))

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
