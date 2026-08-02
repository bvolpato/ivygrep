import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

SCRIPT = ROOT / "scripts" / "export_memoryquest.py"
SPEC = importlib.util.spec_from_file_location("export_memoryquest", SCRIPT)
export_memoryquest = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(export_memoryquest)

RENDER_SCRIPT = ROOT / "scripts" / "render_memory_benchmark.py"
RENDER_SPEC = importlib.util.spec_from_file_location(
    "render_memory_benchmark", RENDER_SCRIPT
)
render_memory_benchmark = importlib.util.module_from_spec(RENDER_SPEC)
assert RENDER_SPEC.loader is not None
RENDER_SPEC.loader.exec_module(render_memory_benchmark)


class MemoryQuestExporterTest(unittest.TestCase):
    def test_exports_raw_sessions_without_label_leakage(self):
        user = {
            "demographics": {"user_id": "user0"},
            "sessions": [
                {
                    "id": "s1",
                    "date": "2026-01-02",
                    "topic": "SECRET CONSTRUCTION TOPIC",
                    "domains": ["Travel"],
                    "is_required": True,
                    "conversation": [
                        {"user": "I prefer quiet hotels."},
                        {"assistant": "I will remember that."},
                    ],
                },
                {
                    "id": "s2",
                    "date": "2026-03-04",
                    "topic": "FUTURE DISTRACTOR",
                    "domains": ["Travel"],
                    "is_required": False,
                    "conversation": [
                        {"user": "I changed my hotel preference."},
                        {"assistant": "I will remember that later."},
                    ],
                },
            ],
            "queries": [
                {
                    "date": "2026-02-03",
                    "query": "Can you find somewhere I would enjoy?",
                    "needed_references": [
                        ["2026-01-02", "The user prefers quiet hotels."]
                    ],
                }
            ],
        }

        corpus, queries, qrels, counts = export_memoryquest.export_records([user])

        self.assertEqual(len(corpus), 2)
        self.assertIn("I prefer quiet hotels.", corpus[0]["text"])
        self.assertNotIn("SECRET CONSTRUCTION TOPIC", corpus[0]["text"])
        self.assertNotIn("is_required", corpus[0]["text"])
        self.assertEqual(queries[0]["metadata"]["scope"], "users/user0")
        self.assertEqual(
            queries[0]["metadata"]["exclude_globs"],
            ["users/user0/s2.md"],
        )
        self.assertEqual(qrels, [("user0:q000", "user0:s1", 1)])
        self.assertEqual(counts["resolved_references"], 1)

    def test_refuses_to_replace_unrecognized_directory(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "existing"
            output.mkdir()
            (output / "personal.txt").write_text("keep", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "unrecognized output directory"):
                export_memoryquest.validate_replaceable_output(output)

            self.assertEqual(
                (output / "personal.txt").read_text(encoding="utf-8"),
                "keep",
            )

    def test_renderer_rejects_benchmark_only_expansion_as_default(self):
        result = {
            "mode": "blended",
            "query_expansion": "memory-facets",
            "memory_expansion_disabled": True,
            "retrieval_provenance": {
                "mode_semantics": "blended-routing",
                "force_neural": False,
            },
        }
        with self.assertRaisesRegex(ValueError, "benchmark-only query expansion"):
            render_memory_benchmark.validate_published_mode("blended", result)

    def test_renderer_resolves_explicit_source_commit(self):
        current = render_memory_benchmark.git_revision(ROOT)
        resolved = render_memory_benchmark.git_revision(ROOT, current)
        self.assertEqual(resolved, current)


class MemoryBenchmarkProvenanceTest(unittest.TestCase):
    def test_release_equivalence_rejects_changed_benchmark_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess = render_memory_benchmark.subprocess
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            (root / "src").mkdir()
            (root / "src" / "lib.rs").write_text("before\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "before",
                ],
                cwd=root,
                check=True,
            )
            build_commit = render_memory_benchmark.git_revision(root)
            (root / "src" / "lib.rs").write_text("after\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "after",
                ],
                cwd=root,
                check=True,
            )
            subprocess.run(
                ["git", "tag", "-m", "test", "v-test"], cwd=root, check=True
            )
            result = render_memory_benchmark.release_equivalence(
                root, build_commit, "v-test"
            )
        self.assertFalse(result["benchmark_inputs_unchanged"])
        self.assertEqual(result["changed_benchmark_inputs"], ["src/lib.rs"])

    def test_published_memory_report_has_release_provenance(self) -> None:
        report = json.loads(
            (ROOT / "docs/benchmarks/public-memory-retrieval-results.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            report["modes"]["blended"]["binary"]["version"], "ivygrep 1.2.7"
        )
        self.assertEqual(report["release_equivalence"]["tag"], "v1.2.7")
        self.assertTrue(
            report["release_equivalence"]["benchmark_inputs_unchanged"]
        )
        self.assertIn(
            "Binary provenance",
            (ROOT / "docs/benchmarks/public-memory-retrieval.html").read_text(),
        )


if __name__ == "__main__":
    unittest.main()
