import importlib.util
import copy
import contextlib
import io
import json
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


contracts = load_script("public_retrieval_contracts")
sys.modules["public_retrieval_contracts"] = contracts
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
current_head_runner = load_script("run_current_head_benchmark")


def recorded_options(request):
    options = request["options"]
    capture = options["capture_reranker"]
    return {
        "query_text_limit": options["max_query_chars"],
        "query_expansion": options["query_expansion"],
        "query_expansion_workers": options["query_expansion_workers"],
        "probe_limit": options["probe_limit"],
        "probe_query_chars": options["probe_query_chars"],
        "rrf_k": options["rrf_k"],
        "original_weight": options["original_weight"],
        "memory_expansion_disabled": options["disable_memory_expansion"],
        "measurement_scope": "native-training-capture"
        if capture
        else "retrieval-benchmark",
        "warm_query_path": "local-process-native-capture"
        if capture
        else ("local-process" if request["mode"] == "lexical" else "daemon"),
        "runtime": request["runtime"],
    }


def cached_result_fixture(root: Path, *, mode: str = "blended", query_limit=None):
    dataset = root / "public"
    dataset.mkdir()
    (dataset / "corpus.jsonl").write_text('{"_id":"d1","text":"body"}\n')
    (dataset / "queries.jsonl").write_text('{"_id":"q1","text":"query"}\n')
    (dataset / "qrels.tsv").write_text("query-id\tcorpus-id\tscore\nq1\td1\t1\n")
    provenance = {
        "counts": {"queries": 1},
        "languages": ["Rust"],
        "checksums": {
            name: contracts.sha256_file(dataset / name)
            for name in ("corpus.jsonl", "queries.jsonl", "qrels.tsv")
        },
    }
    (dataset / "provenance.json").write_text(json.dumps(provenance))
    request = contracts.execution_request(
        dataset,
        "b" * 64,
        mode,
        {"max_query_chars": query_limit},
        {},
        {"machine": "fixture"},
        {"eval.py": "harness"},
    )
    configuration = {
        "neural_profile": "static-retrieval-v1",
        "neural_model": {"model_id": "fixture"},
        "reranker_mode": "learned",
        "reranker_model": "fixture",
        "reranker_candidate_limit": 100,
    }
    result = {
        "dataset": dataset.name,
        "mode": mode,
        "queries": 1,
        "details": [{"query_id": "q1", "warm_latency_ms": 1.0, "cold_latency_ms": 2.0}],
        "binary": {"sha256": "b" * 64},
        **recorded_options(request),
        "dataset_provenance": provenance,
        "query_text_limit": query_limit,
        "index_configuration": configuration,
        "execution_provenance": {
            "schema_version": 1,
            "request": request,
            "request_sha256": contracts.canonical_sha256(request),
            "source_commit": "a" * 40,
            "executed_at": "2026-01-01T00:00:00+00:00",
            "observed_configuration": contracts.observed_configuration(configuration),
        },
    }
    return dataset, request, result


def native_capture_record(query="validate_token", process_id=123):
    return {
        "schema_version": 1,
        "stage": contracts.CAPTURE_STAGE,
        "status": "applied",
        "reason": None,
        "query": query,
        "process_id": process_id,
        "model_id": "public-linear-reranker-v2",
        "ranking_context_lines": 2,
        "feature_schema": list(contracts.RERANK_FEATURE_SCHEMA),
        "candidates": [
            {
                "file_path": f"documents/d{rank}.rs",
                "total_score": 3.0 - rank / 10,
                "hit_count": 1,
                "sources": ["lexical"],
                "canonical_preview": "fn validate_token() {}",
                "baseline_rank": rank,
                "native_features": [0.125] * len(contracts.RERANK_FEATURE_SCHEMA),
            }
            for rank in range(5)
        ],
    }


def native_training_fixture(
    root: Path, query="validate_token", effective_query=None
):
    dataset = root / "public"
    dataset.mkdir()
    corpus = [
        {
            "_id": f"d{index}",
            "text": "fn validate_token() {}",
            "metadata": {"path": f"documents/d{index}.rs"},
        }
        for index in range(5)
    ]
    (dataset / "corpus.jsonl").write_text(
        "".join(json.dumps(row) + "\n" for row in corpus)
    )
    (dataset / "queries.jsonl").write_text(
        json.dumps({"_id": "q1", "text": query}) + "\n"
    )
    (dataset / "qrels.tsv").write_text("query-id\tcorpus-id\tscore\nq1\td0\t1\n")
    provenance = {
        "counts": {"queries": 1},
        "query_corpus": {"repository": "fixture/queries"},
        "checksums": {
            name: contracts.sha256_file(dataset / name)
            for name in ("corpus.jsonl", "queries.jsonl", "qrels.tsv")
        },
    }
    (dataset / "provenance.json").write_text(json.dumps(provenance))
    request = contracts.execution_request(
        dataset,
        "b" * 64,
        "hash",
        {"capture_reranker": True},
        {},
        {"machine": "fixture"},
        {"eval.py": "fixture"},
    )
    configuration = {
        "reranker_mode": "learned",
        "reranker_model": "public-linear-reranker-v2",
        "reranker_candidate_limit": 100,
    }
    record = native_capture_record(
        query if effective_query is None else effective_query
    )
    repo = root / "old-materialized-repo"
    for candidate in record["candidates"]:
        candidate["file_path"] = str(repo / candidate["file_path"])
    receipts = root / "trace.json.native-captures"
    receipts.mkdir()
    receipt = receipts / "q000000"
    stderr = "model log\n" + contracts.CAPTURE_PREFIX + json.dumps(record) + "\n"
    stdout = json.dumps([{"file_path": "documents/d0.rs", "total_score": 900.0}])
    receipt.with_suffix(".stderr.log").write_text(stderr)
    receipt.with_suffix(".stdout.json").write_text(stdout)
    receipt.with_suffix(".command.json").write_text(
        json.dumps(
            {
                "argv": ["ig", "--", query],
                "process_id": 123,
                "query": query,
                "cwd": str(repo),
            }
        )
    )
    receipt.with_suffix(".exit.json").write_text(
        json.dumps({"process_id": 123, "returncode": 0})
    )
    capture = {
        "record": record,
        "process_id": 123,
        "receipt_name": receipt.name,
        "candidate_document_ids": [f"d{index}" for index in range(5)],
        "stderr_sha256": contracts.sha256_file(receipt.with_suffix(".stderr.log")),
        "stdout_sha256": contracts.sha256_file(receipt.with_suffix(".stdout.json")),
    }
    result = {
        "dataset": dataset.name,
        "mode": "hash",
        "binary": {"sha256": "b" * 64},
        **recorded_options(request),
        "queries": 1,
        "query_text_limit": None,
        "query_expansion": "none",
        "measurement_scope": "native-training-capture",
        "index_configuration": configuration,
        "execution_provenance": {
            "schema_version": 1,
            "request": request,
            "request_sha256": contracts.canonical_sha256(request),
            "source_commit": "a" * 40,
            "executed_at": "2026-01-01T00:00:00+00:00",
            "observed_configuration": contracts.observed_configuration(configuration),
        },
        "native_capture_contract": {
            "schema_version": 1,
            "stage": contracts.CAPTURE_STAGE,
            "transport": "fresh-process-stderr",
            "ranking_context_lines": 2,
            "feature_schema": list(contracts.RERANK_FEATURE_SCHEMA),
            "receipt_directory": receipts.name,
            "applied_queries": 1,
            "skipped_queries": 0,
            "skip_reasons": {},
        },
        "details": [
            {
                "query_id": "q1",
                "native_capture": capture,
                "ranked_hits": [
                    {
                        "document_id": "d0",
                        "file_path": "documents/d0.rs",
                        "total_score": 900.0,
                    }
                ],
            }
        ],
    }
    result_path = root / "trace.json"
    result_path.write_text(json.dumps(result))
    return dataset, result_path, result, receipt


class PublicBenchmarkTest(unittest.TestCase):
    def test_current_head_evidence_rejects_changed_search_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = root / "tests" / "fixtures" / "ivygrep_relevance_queries.json"
            harness = root / "scripts" / "eval_relevance.py"
            source = root / "src" / "search.rs"
            for path in (fixture, harness, source):
                path.parent.mkdir(parents=True, exist_ok=True)
            (root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
            fixture.write_text('{"queries": [{}]}\n')
            harness.write_text("original harness\n")
            source.write_text("fn original_search() {}\n")
            report = {
                "binary": {"version": "ivygrep 1.2.3"},
                "fixture": {"sha256": current_head_runner.sha256_file(fixture)},
                "harness": {"sha256": current_head_runner.sha256_file(harness)},
                "source": {"sha256": current_head_runner.source_inputs_sha256(root)},
                "modes": {
                    "foreground": {
                        "queries": 1,
                        "mean_ndcg10": 1.0,
                        "mean_mrr": 1.0,
                        "mean_candidate_recall": 1.0,
                        "no_hit_queries": 0,
                    },
                    "hash-enriched": {
                        "queries": 1,
                        "mean_ndcg10": 1.0,
                        "mean_mrr": 1.0,
                        "mean_candidate_recall": 1.0,
                        "no_hit_queries": 0,
                    },
                },
            }
            self.assertEqual(current_head_runner.validate_report(report, root=root), [])

            source.write_text("fn replacement_search() {}\n")
            self.assertTrue(
                any("source SHA-256" in error for error in
                    current_head_runner.validate_report(report, root=root))
            )

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

    def test_fit_ledger_binds_all_actual_training_sources_and_model(self):
        manifest = exporter.load_manifest(ROOT / "benchmarks/public/manifest.json")
        config = manifest["reranker_fit_ledger"]
        base = ROOT / "benchmarks/public"
        ledger = contracts.load_fit_ledger(
            base / config["model"], base / config["path"], config["sha256"]
        )
        model = json.loads((base / config["model"]).read_text())
        self.assertEqual(ledger["queries"], model["training"]["queries"])
        self.assertEqual(
            {row["dataset"] for row in ledger["sources"]},
            {row["dataset"] for row in model["training"]["sources"]},
        )
        with tempfile.TemporaryDirectory() as temporary:
            changed_model = Path(temporary) / "model.json"
            model["weights"][0] += 0.5
            changed_model.write_text(json.dumps(model))
            with self.assertRaisesRegex(ValueError, "bound"):
                contracts.load_fit_ledger(
                    changed_model, base / config["path"], config["sha256"]
                )

    def test_actual_fit_id_cannot_pass_declared_disjoint_diagnostic(self):
        base = ROOT / "benchmarks/public"
        config = json.loads((base / "manifest.json").read_text())["reranker_fit_ledger"]
        ledger = contracts.load_fit_ledger(
            base / config["model"], base / config["path"], config["sha256"]
        )
        source = ledger["sources"][0]
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / source["dataset"]
            dataset.mkdir()
            (dataset / "provenance.json").write_text(json.dumps(source["provenance"]))
            (dataset / "queries.jsonl").write_text(
                json.dumps({"_id": source["query_ids"][0], "text": "fixture"}) + "\n"
            )
            audit = contracts.audit_fit_queries(ledger, [dataset], "regression")
            self.assertEqual(audit["overlap_queries"], 1)
            with self.assertRaisesRegex(ValueError, "overlaps 1 actual"):
                contracts.audit_fit_queries(
                    ledger, [dataset], "fit-disjoint-diagnostic"
                )
            provenance = source["provenance"].copy()
            provenance["query_corpus"] = {"repository": "unrelated/query-namespace"}
            (dataset / "provenance.json").write_text(json.dumps(provenance))
            self.assertEqual(
                contracts.audit_fit_queries(
                    ledger, [dataset], "fit-disjoint-diagnostic"
                )["overlap_queries"],
                0,
            )

    def test_disjoint_claim_requires_a_model_bound_ledger(self):
        manifest = {
            "profiles": {"diagnostic": {"query_role": "fit-disjoint-diagnostic"}}
        }
        with self.assertRaisesRegex(ValueError, "requires a model-bound"):
            contracts.audit_public_profile(
                manifest, "diagnostic", [], Path("manifest.json")
            )

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

    def test_corpus_sampling_note_states_indexed_versus_full_corpus(self):
        matrix = {
            "tasks": ["codefeedback-st", "cosqa"],
            "results": [
                {
                    "dataset": "codefeedback-st",
                    "dataset_provenance": {
                        "counts": {"corpus": 5000, "source_corpus": 156526},
                        "sample": {"corpus_limit": 5000},
                    },
                },
                {
                    "dataset": "cosqa",
                    "dataset_provenance": {
                        "counts": {"corpus": 20604, "source_corpus": 20604},
                        "sample": {"corpus_limit": None},
                    },
                },
            ],
        }
        note = renderer.corpus_sampling_note(matrix)
        self.assertIn("1 of 2 corpora sampled to 5,000 documents", note)
        self.assertIn("codefeedback-st 156,526 documents (5,000 indexed)", note)
        self.assertIn("cosqa (20,604)", note)
        self.assertIn("not comparable", note)
        self.assertEqual(
            renderer.corpus_sampling_note({"tasks": ["x"], "results": []}),
            "Corpus sampling: provenance counts unavailable; see raw JSON.",
        )

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

    def test_reference_support_feature_matches_runtime_roles(self):
        self.assertFalse(reranker_trainer.is_support_path("docs/support.md"))
        self.assertFalse(reranker_trainer.is_support_path("src/request_test.rs"))
        self.assertTrue(reranker_trainer.is_support_path("tools/request.rs"))
        self.assertTrue(reranker_trainer.is_support_path("examples/request.rs"))

    def test_native_feature_fixture_matches_python_reference(self):
        fixture = json.loads(
            (ROOT / "tests/fixtures/reranker_feature_contract.json").read_text()
        )
        self.assertEqual(
            fixture["feature_schema"], list(reranker_trainer.FEATURE_NAMES)
        )
        for case in fixture["cases"]:
            with self.subTest(case=case["name"]):
                actual = reranker_trainer.feature_vector(
                    case["query"], case["candidate"], case["baseline_rank"]
                )
                for name, left, right in zip(
                    fixture["feature_schema"],
                    actual,
                    case["expected_features"],
                    strict=True,
                ):
                    self.assertAlmostEqual(left, right, places=6, msg=name)

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

    def test_public_core_report_is_not_claimed_as_unseen_queries(self):
        matrix = json.loads(
            (ROOT / "docs/benchmarks/public-code-retrieval-results.json").read_text()
        )
        report = renderer.markdown(matrix)
        self.assertNotIn("Held-out queries:", report)
        self.assertIn("regression", report.lower())

    def test_training_rejects_already_learned_scores(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "fixture"
            dataset.mkdir()
            (dataset / "queries.jsonl").write_text(
                '{"_id":"q1","text":"handle request"}\n'
            )
            (dataset / "qrels.tsv").write_text(
                "query-id\tcorpus-id\tscore\nq1\td1\t1\n"
            )
            (dataset / "provenance.json").write_text("{}\n")
            result = dataset / "result.json"
            result.write_text(
                json.dumps(
                    {
                        "binary": {"sha256": "binary"},
                        "queries": 1,
                        "query_expansion": "none",
                        "index_configuration": {"reranker_mode": "learned"},
                        "details": [
                            {
                                "query_id": "q1",
                                "ranked_hits": [
                                    {
                                        "document_id": "d1",
                                        "file_path": "src/request.rs",
                                        "total_score": 3.0,
                                        "hit_count": 1,
                                        "sources": ["lexical"],
                                        "preview": "handle request",
                                    }
                                ],
                            }
                        ],
                    }
                )
            )
            with self.assertRaisesRegex(ValueError, "deterministic"):
                reranker_trainer.load_examples([(dataset, result)])

    def test_native_capture_is_exactly_one_fresh_local_versioned_record(self):
        record = native_capture_record()
        line = contracts.CAPTURE_PREFIX + json.dumps(record) + "\n"
        self.assertEqual(
            contracts.parse_native_capture("log\n" + line, record["query"], 123), record
        )
        for stderr, query, pid in (
            ("log only\n", record["query"], 123),
            (line + line, record["query"], 123),
            (line, "other query", 123),
            (line, record["query"], 456),
        ):
            with (
                self.subTest(stderr=stderr[:25], query=query, pid=pid),
                self.assertRaises(ValueError),
            ):
                contracts.parse_native_capture(stderr, query, pid)

    def test_native_capture_matches_runtime_trim_without_rewriting_receipts(self):
        for raw, effective in (
            ('  alpha "beta gamma"\n', 'alpha "beta gamma"'),
            ('\t alpha\n"beta gamma" \r\n', 'alpha\n"beta gamma"'),
            ("\u0085\u00a0alpha\u2003\u3000", "alpha"),
            ("\x1calpha\x1f", "\x1calpha\x1f"),
            ("\ufeffalpha\ufeff", "\ufeffalpha\ufeff"),
        ):
            record = native_capture_record(query=effective)
            line = contracts.CAPTURE_PREFIX + json.dumps(record) + "\n"
            with self.subTest(raw=raw):
                self.assertEqual(contracts.parse_native_capture(line, raw, 123), record)
                with self.assertRaisesRegex(ValueError, "query"):
                    contracts.parse_native_capture(line, raw + "different", 123)

        raw = '\t alpha\n"beta gamma" \r\n'
        with tempfile.TemporaryDirectory() as temporary:
            dataset, result_path, _, receipt = native_training_fixture(
                Path(temporary), raw, 'alpha\n"beta gamma"'
            )
            before = receipt.with_suffix(".command.json").read_bytes()
            examples, _ = reranker_trainer.load_examples([(dataset, result_path)])
            self.assertEqual(examples[0]["query"], raw)
            self.assertEqual(examples[0]["candidates"][0]["features"], [0.125] * 41)
            command = json.loads(before)
            self.assertEqual(command["query"], raw)
            self.assertEqual(command["argv"][-1], raw)
            self.assertEqual(receipt.with_suffix(".command.json").read_bytes(), before)

    def test_native_capture_uses_lf_framing_for_unicode_json_payloads(self):
        for separator in ("\u0085", "\u2028", "\u2029"):
            record = native_capture_record(query=f"alpha{separator}beta")
            record["candidates"][0]["canonical_preview"] = f"return{separator}value"
            line = (
                contracts.CAPTURE_PREFIX + json.dumps(record, ensure_ascii=False) + "\n"
            )
            with self.subTest(separator=repr(separator)):
                self.assertEqual(
                    contracts.parse_native_capture(
                        "log\n" + line, record["query"], 123
                    ),
                    record,
                )
                with self.assertRaisesRegex(ValueError, "exactly one"):
                    contracts.parse_native_capture(line + line, record["query"], 123)

    def test_native_capture_rejects_nonfinite_incomplete_or_backfill_features(self):
        for mutation in ("nonfinite", "schema", "rank", "backfill", "truncated"):
            record = native_capture_record()
            if mutation == "nonfinite":
                record["candidates"][0]["native_features"][0] = float("nan")
            elif mutation == "schema":
                record["feature_schema"] = list(reversed(record["feature_schema"]))
            elif mutation == "rank":
                record["candidates"][0]["baseline_rank"] = 2
            elif mutation == "backfill":
                record["candidates"][0]["sources"].append("backfill")
            else:
                record["candidates"].pop()
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                contracts.validate_native_capture(record, record["query"], 123)

    def test_native_capture_skipped_status_is_explicit_not_recomputed(self):
        record = native_capture_record()
        record.update(
            status="skipped", reason="route-not-learned", model_id=None, candidates=[]
        )
        contracts.validate_native_capture(record, record["query"], 123)
        record["candidates"] = native_capture_record()["candidates"]
        with self.assertRaisesRegex(ValueError, "skipped"):
            contracts.validate_native_capture(record, record["query"], 123)

    def test_training_uses_native_features_and_verifies_original_receipts(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, result_path, result, receipt = native_training_fixture(
                Path(temporary)
            )
            with mock.patch.object(
                reranker_trainer,
                "feature_vector",
                side_effect=AssertionError("must not reconstruct native features"),
            ):
                examples, sources = reranker_trainer.load_examples(
                    [(dataset, result_path)]
                )
            self.assertEqual(
                examples[0]["candidates"][0]["features"],
                [0.125] * len(contracts.RERANK_FEATURE_SCHEMA),
            )
            self.assertEqual(examples[0]["candidates"][0]["grade"], 1)
            self.assertEqual(sources[0]["fit_query_ids"], ["q1"])
            result["details"][0]["native_capture"]["record"]["candidates"][0][
                "native_features"
            ][0] = 0.5
            result_path.write_text(json.dumps(result))
            with self.assertRaisesRegex(ValueError, "original stderr"):
                reranker_trainer.load_examples([(dataset, result_path)])
            result["details"][0]["native_capture"]["record"]["candidates"][0][
                "native_features"
            ][0] = 0.125
            result_path.write_text(json.dumps(result))
            receipt.with_suffix(".stderr.log").write_text("missing capture\n")
            with self.assertRaisesRegex(ValueError, "provenance"):
                reranker_trainer.load_examples([(dataset, result_path)])

    def test_local_capture_missing_record_preserves_raw_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            process = mock.Mock(pid=123, returncode=0)
            process.communicate.return_value = (
                b"[]\n",
                b"daemon returned without local capture\n",
            )
            receipt = root / "captures/q000000"
            with mock.patch.object(evaluator.subprocess, "Popen", return_value=process):
                with self.assertRaisesRegex(ValueError, "exactly one"):
                    evaluator.run_captured_query(
                        ["ig", "query"],
                        root,
                        {"IVYGREP_HOME": str(root / "home")},
                        "query",
                        receipt,
                    )
            self.assertEqual(
                receipt.with_suffix(".stderr.log").read_text(),
                "daemon returned without local capture\n",
            )
            self.assertEqual(receipt.with_suffix(".stdout.json").read_text(), "[]\n")

    def test_native_capture_document_mapping_is_portable_but_contained(self):
        record = {"candidates": [{"file_path": r"C:\repo\documents\d0.rs"}]}
        self.assertEqual(
            evaluator.captured_document_ids(
                record, Path(r"C:\repo"), {"documents/d0.rs": "d0"}
            ),
            ["d0"],
        )
        record["candidates"][0]["file_path"] = r"C:\repo\..\outside.rs"
        with self.assertRaisesRegex(ValueError, "escapes"):
            evaluator.captured_document_ids(
                record, Path(r"C:\repo"), {"documents/d0.rs": "d0"}
            )

    def test_reuse_rejects_legacy_result_without_execution_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset = Path(temporary) / "fixture"
            dataset.mkdir()
            (dataset / "provenance.json").write_text(
                json.dumps(
                    {
                        "checksums": {"corpus.jsonl": "same"},
                    }
                )
            )
            result = {
                "dataset": "fixture",
                "mode": "blended",
                "binary": {"sha256": "binary"},
                "dataset_provenance": {"checksums": {"corpus.jsonl": "same"}},
                "query_text_limit": None,
                "index_configuration": {
                    "neural_profile": "static-retrieval-v1",
                    "reranker_mode": "learned",
                },
            }
            with self.assertRaisesRegex(ValueError, "execution|fingerprint|legacy"):
                matrix_runner.validate_reused_result(
                    result, dataset, "blended", "binary", None
                )

    def test_reused_result_must_match_binary_and_dataset(self):
        with tempfile.TemporaryDirectory() as temp:
            dataset, request, result = cached_result_fixture(
                Path(temp), mode="hash", query_limit=2048
            )
            matrix_runner.validate_reused_result(
                result,
                dataset,
                "hash",
                "b" * 64,
                2048,
                expected_request=request,
            )
            with self.assertRaises(ValueError):
                matrix_runner.validate_reused_result(
                    result,
                    dataset,
                    "hash",
                    "different",
                    2048,
                    expected_request=request,
                )
            with self.assertRaises(ValueError):
                matrix_runner.validate_reused_result(
                    result,
                    dataset,
                    "hash",
                    "b" * 64,
                    None,
                    expected_request=request,
                )

    def test_reuse_rejects_model_reranker_threads_and_harness_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, request, result = cached_result_fixture(Path(temporary))
            for environment in (
                {"IVYGREP_MODEL_PROFILE": "general"},
                {"IVYGREP_RERANKER": "deterministic"},
                {"IVYGREP_NEURAL_THREADS": "7"},
            ):
                with self.subTest(environment=environment):
                    changed = contracts.execution_request(
                        dataset,
                        "b" * 64,
                        "blended",
                        {},
                        environment,
                        request["runtime"],
                        request["harness_sha256"],
                    )
                    with self.assertRaisesRegex(ValueError, "configuration differs"):
                        matrix_runner.validate_reused_result(
                            result,
                            dataset,
                            "blended",
                            "b" * 64,
                            None,
                            expected_request=changed,
                        )
            changed = copy.deepcopy(request)
            changed["harness_sha256"]["eval.py"] = "changed"
            with self.assertRaisesRegex(ValueError, "harness_sha256"):
                contracts.validate_execution(result, changed)

    def test_reuse_tracks_query_cache_disable_presence_even_for_zero(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, request, result = cached_result_fixture(Path(temporary))
            fingerprints = []
            for value in ("1", "0", ""):
                changed = contracts.execution_request(
                    dataset,
                    "b" * 64,
                    "blended",
                    {},
                    {"IVYGREP_DISABLE_QUERY_CACHE": value},
                    request["runtime"],
                    request["harness_sha256"],
                )
                with (
                    self.subTest(value=value),
                    self.assertRaisesRegex(ValueError, "configuration differs"),
                ):
                    matrix_runner.validate_reused_result(
                        result,
                        dataset,
                        "blended",
                        "b" * 64,
                        None,
                        expected_request=changed,
                    )
                fingerprints.append(contracts.canonical_sha256(changed))
            self.assertEqual(len(set(fingerprints)), 1)

    def test_foreground_accelerator_uses_runtime_cpu_auto_semantics(self):
        key = "IVYGREP_NEURAL_FOREGROUND_ACCELERATOR"
        cpu = contracts.safe_environment({key: "cpu"})
        auto = contracts.safe_environment({key: "auto"})
        self.assertEqual(cpu["settings"][key], "cpu")
        self.assertEqual(auto["settings"][key], "auto")
        self.assertEqual(auto, contracts.safe_environment({}))
        for value in ("0", "false", "no", "off", " CPU "):
            self.assertEqual(cpu, contracts.safe_environment({key: value}))
        for value in ("1", "true", "auto", "", "other-runtime-auto-value"):
            self.assertEqual(auto, contracts.safe_environment({key: value}))
        self.assertNotEqual(cpu, auto)

    def test_reuse_rejects_expansion_and_tampered_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, request, result = cached_result_fixture(Path(temporary))
            changed = contracts.execution_request(
                dataset,
                "b" * 64,
                "blended",
                {"query_expansion": "retrieval-facets", "rrf_k": 1.0},
                {},
                request["runtime"],
                request["harness_sha256"],
            )
            with self.assertRaisesRegex(ValueError, "options"):
                contracts.validate_execution(result, changed)
            result["execution_provenance"]["request"]["options"]["rrf_k"] = 1.0
            with self.assertRaisesRegex(ValueError, "corrupt"):
                contracts.validate_execution(result, request)

    def test_reuse_verifies_dataset_bytes_not_only_copied_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, _, _ = cached_result_fixture(Path(temporary))
            (dataset / "queries.jsonl").write_text('{"_id":"q1","text":"different"}\n')
            with self.assertRaisesRegex(ValueError, "checksum differs.*queries.jsonl"):
                contracts.dataset_fingerprint(dataset)

    def test_configuration_whitelist_does_not_publish_credentials_or_paths(self):
        environment = {
            "HF_TOKEN": "private-token",
            "AWS_SECRET_ACCESS_KEY": "private-secret",
            "IVYGREP_UNKNOWN_TOKEN": "another-secret",
            "HF_HOME": "/private/user/cache",
            "IVYGREP_MODEL_PROFILE": "general",
            "IVYGREP_NEURAL_THREADS": "2",
        }
        safe = contracts.safe_environment(environment)
        encoded = json.dumps(safe)
        for private in (
            "private-token",
            "private-secret",
            "another-secret",
            "/private/user/cache",
            "HF_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
            "IVYGREP_UNKNOWN_TOKEN",
        ):
            self.assertNotIn(private, encoded)
        self.assertEqual(safe["settings"]["IVYGREP_MODEL_PROFILE"], "general")
        self.assertEqual(safe["settings"]["IVYGREP_NEURAL_THREADS"], 2)

    def test_aggregation_preserves_original_execution_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, request, result = cached_result_fixture(Path(temporary))
            original = copy.deepcopy(result["execution_provenance"])
            summary = contracts.execution_summary([result])
            self.assertEqual(summary["ivygrep_commit"], "a" * 40)
            self.assertEqual(summary["harness_sha256"], request["harness_sha256"])
            self.assertEqual(summary["runtime"], request["runtime"])
            self.assertEqual(result["execution_provenance"], original)
            other = copy.deepcopy(result)
            other["execution_provenance"]["source_commit"] = "c" * 40
            self.assertEqual(
                contracts.execution_summary([result, other])["ivygrep_commit"], "mixed"
            )

    def test_matrix_reuse_keeps_execution_source_separate_from_new_aggregation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dataset, request, result = cached_result_fixture(root)
            binary = root / "never-executed-binary"
            binary.write_text("fixture\n")
            request["binary_sha256"] = contracts.sha256_file(binary)
            result["binary"]["sha256"] = request["binary_sha256"]
            result["execution_provenance"]["request_sha256"] = (
                contracts.canonical_sha256(request)
            )
            historical = json.loads(
                (
                    ROOT / "docs/benchmarks/public-code-retrieval-results.json"
                ).read_text()
            )["results"][0]
            cached = {
                **historical,
                **result,
                "queries": 1,
                "details": [
                    {"query_id": "q1", "warm_latency_ms": 1.0, "cold_latency_ms": 2.0}
                ],
            }
            work = root / "work"
            work.mkdir()
            cached_path = work / "public-blended-run-1.json"
            cached_path.write_text(json.dumps(cached))
            before = cached_path.read_bytes()
            manifest = root / "manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "profiles": {
                            "probe": {"tasks": ["public"], "minimum_queries": 1}
                        },
                        "tasks": {"public": {}},
                    }
                )
            )
            output = root / "matrix.json"
            argv = [
                "matrix",
                "--manifest",
                str(manifest),
                "--profile",
                "probe",
                "--datasets-root",
                str(root),
                "--work-root",
                str(work),
                "--binary",
                str(binary),
                "--modes",
                "blended",
                "--runs",
                "1",
                "--skip-build",
                "--skip-export",
                "--reuse-results",
                "--output",
                str(output),
            ]
            with (
                mock.patch.object(sys, "argv", argv),
                mock.patch.object(
                    matrix_runner, "benchmark_revision", return_value="c" * 40
                ),
                mock.patch.object(
                    evaluator, "runtime_metadata", return_value=request["runtime"]
                ),
                mock.patch.object(
                    evaluator, "expected_execution_request", return_value=request
                ),
                mock.patch.object(
                    contracts,
                    "execution_harness",
                    return_value=request["harness_sha256"],
                ),
                mock.patch.object(matrix_runner.subprocess, "run"),
                mock.patch.object(
                    matrix_runner,
                    "run_evaluation",
                    side_effect=AssertionError("must reuse, not run retrieval"),
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                self.assertEqual(matrix_runner.main(), 0)
                published_text = output.read_text()
                output.unlink()
                original_aggregate = matrix_runner.aggregate_runs

                def replace_during_aggregation(*args, **kwargs):
                    binary.write_bytes(b"replacement binary\n")
                    return original_aggregate(*args, **kwargs)

                with mock.patch.object(
                    matrix_runner,
                    "aggregate_runs",
                    side_effect=replace_during_aggregation,
                ):
                    with self.assertRaisesRegex(ValueError, "binary changed"):
                        matrix_runner.main()
                self.assertFalse(output.exists())
            matrix = json.loads(published_text)
            self.assertEqual(matrix["ivygrep_commit"], "a" * 40)
            self.assertEqual(
                matrix["aggregation_provenance"]["source_commit"], "c" * 40
            )
            self.assertEqual(
                matrix["results"][0]["execution_provenance"],
                cached["execution_provenance"],
            )
            self.assertEqual(cached_path.read_bytes(), before)

    def test_evaluator_rejects_persistent_binary_replacement_before_publication(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dataset, _, _ = cached_result_fixture(root)
            binary = root / "mutable-binary"
            binary.write_bytes(b"initial binary\n")
            options = {**contracts.EVALUATION_DEFAULTS, "capture_reranker": True}
            args = evaluator.argparse.Namespace(
                dataset=dataset,
                binary=binary,
                mode="hash",
                output=root / "result.json",
                source_commit="a" * 40,
                **options,
            )

            def captured_query(command, cwd, env, query, receipt):
                binary.write_bytes(b"replacement binary\n")
                record = native_capture_record(query=query)
                record.update(
                    status="skipped",
                    reason="route-not-learned",
                    model_id=None,
                    candidates=[],
                )
                return [], 0.1, {"record": record, "process_id": 123}

            def status(command, cwd, env):
                return [
                    {
                        "root": str(cwd),
                        "index_size_bytes": 1,
                        "reranker_mode": "learned",
                        "reranker_model": "public-linear-reranker-v2",
                    }
                ], 0.0

            completed = evaluator.subprocess.CompletedProcess(
                [], 0, stdout="fixture version\n", stderr=""
            )
            with (
                mock.patch.dict(evaluator.os.environ, {}, clear=True),
                mock.patch.object(evaluator.subprocess, "run", return_value=completed),
                mock.patch.object(evaluator, "run_json", side_effect=status),
                mock.patch.object(
                    evaluator, "run_captured_query", side_effect=captured_query
                ),
            ):
                with self.assertRaisesRegex(ValueError, "binary changed"):
                    evaluator.evaluate(args)

    def test_public_selection_rejects_changed_profile_sampling(self):
        with tempfile.TemporaryDirectory() as temporary:
            dataset, _, result = cached_result_fixture(Path(temporary))
            manifest = {
                "tasks": {"public": {}},
                "profiles": {
                    "probe": {
                        "task_options": {"public": {"sample_queries": 1, "seed": 9}}
                    }
                },
            }
            with self.assertRaisesRegex(ValueError, "sample/partition"):
                contracts.validate_public_selection(
                    manifest, "probe", dataset, result["dataset_provenance"]
                )

    def test_native_fit_and_evaluation_ids_are_disjoint_by_query_repository(self):
        fit = [
            {
                "dataset": "train",
                "query_repository": "same/repository",
                "query_id": "q1",
            }
        ]
        evaluation = [
            {
                "dataset": "alias",
                "query_repository": "same/repository",
                "query_id": "q1",
            }
        ]
        with self.assertRaisesRegex(ValueError, "overlaps 1 actual"):
            reranker_trainer.ensure_fit_disjoint(fit, evaluation)
        evaluation[0]["query_repository"] = "other/repository"
        reranker_trainer.ensure_fit_disjoint(fit, evaluation)

    def test_new_native_fit_ledger_binds_exact_used_query_ids(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dataset, result_path, _, _ = native_training_fixture(root)
            examples, sources = reranker_trainer.load_examples([(dataset, result_path)])
            report = {
                "schema_version": 2,
                "model_id": "fixture-model",
                "feature_schema": list(contracts.RERANK_FEATURE_SCHEMA),
                "weights": [0.0] * len(contracts.RERANK_FEATURE_SCHEMA),
                "training": {
                    "queries": len(examples),
                    "sources": sources,
                    "ivygrep_commit": "a" * 40,
                },
            }
            model = root / "model.json"
            model.write_text(json.dumps(report))
            ledger_path = root / "fit.json"
            reranker_trainer.write_fit_ledger(
                report, model, [(dataset, result_path)], ledger_path
            )
            ledger = contracts.load_fit_ledger(
                model, ledger_path, contracts.sha256_file(ledger_path)
            )
            self.assertEqual(ledger["sources"][0]["query_ids"], ["q1"])
            ledger["sources"][0]["query_ids"] = ["other"]
            ledger["sources"][0]["query_ids_sha256"] = contracts.pretty_json_sha256(
                ["other"]
            )
            ledger_path.write_text(json.dumps(ledger))
            with self.assertRaisesRegex(ValueError, "differ from the native"):
                contracts.load_fit_ledger(
                    model, ledger_path, contracts.sha256_file(ledger_path)
                )

    def test_checksum_bound_writer_preserves_lf_bytes_on_crlf_default(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dataset, result_path, _, _ = native_training_fixture(root)
            examples, sources = reranker_trainer.load_examples([(dataset, result_path)])
            report = {
                "schema_version": 2,
                "model_id": "fixture-model",
                "weights": [0.0] * len(contracts.RERANK_FEATURE_SCHEMA),
                "training": {
                    "queries": len(examples),
                    "sources": sources,
                    "ivygrep_commit": "a" * 40,
                },
            }
            model = root / "model.json"
            model.write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n", newline="\n"
            )
            ledger_path = root / "fit.json"
            original_open = Path.open

            def crlf_default(path, *args, **kwargs):
                mode = args[0] if args else kwargs.get("mode", "r")
                if "w" in mode and "b" not in mode and kwargs.get("newline") is None:
                    kwargs["newline"] = "\r\n"
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", crlf_default):
                reranker_trainer.write_training_json(model, report)
                reranker_trainer.write_fit_ledger(
                    report, model, [(dataset, result_path)], ledger_path
                )
            ledger = json.loads(ledger_path.read_bytes())
            self.assertEqual(
                contracts.sha256_file(model), contracts.pretty_json_sha256(report)
            )
            self.assertEqual(
                contracts.sha256_file(ledger_path), contracts.pretty_json_sha256(ledger)
            )
            self.assertEqual(
                ledger["model_sha256"], contracts.pretty_json_sha256(report)
            )


if __name__ == "__main__":
    unittest.main()
