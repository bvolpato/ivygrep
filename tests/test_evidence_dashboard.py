import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


history = load_script("normalize_release_history")
renderer = load_script("render_evidence_dashboard")
current_head = load_script("run_current_head_benchmark")


class EvidenceDashboardTest(unittest.TestCase):
    def test_current_head_relevance_rejects_stale_binary_versions(self) -> None:
        report = json.loads(
            (ROOT / "docs/benchmarks/current-head-relevance.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(current_head.validate_report(report), [])
        report["binary"]["version"] = "ivygrep 0.0.1"
        self.assertTrue(
            any("does not match" in error for error in current_head.validate_report(report))
        )

    def test_current_head_relevance_rejects_changed_fixture_provenance(self) -> None:
        report = json.loads(
            (ROOT / "docs/benchmarks/current-head-relevance.json").read_text(
                encoding="utf-8"
            )
        )
        report["fixture"]["sha256"] = "0" * 64
        self.assertTrue(
            any("fixture SHA-256" in error for error in current_head.validate_report(report))
        )

    def test_published_commit_does_not_require_git_history(
        self,
    ) -> None:
        published = "a" * 40
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / "artifact.json"
            path.write_text("{}\n", encoding="utf-8")
            with mock.patch.object(renderer.subprocess, "run") as run:
                commit = renderer.publication_commit(root, path, published)
        self.assertEqual(commit, published)
        run.assert_not_called()

    def test_release_history_retains_missing_sidecars(self) -> None:
        releases = [
            {
                "tag_name": "v1.0.0",
                "published_at": "2026-01-01T00:00:00Z",
                "html_url": "https://example.test/v1.0.0",
                "assets": [
                    {
                        "name": "ivygrep-v1.0.0-linux-x86_64-musl.tar.gz",
                        "size": 123,
                        "browser_download_url": "https://example.test/archive",
                    },
                    {
                        "name": "ivygrep-v1.0.0-linux-x86_64-musl.tar.gz.sha256",
                        "size": 64,
                    },
                ],
            }
        ]
        document = history.normalize(releases)
        archive = document["releases"][0]["archives"][0]
        self.assertTrue(archive["checksum"])
        self.assertFalse(archive["sbom"])
        self.assertFalse(archive["provenance"])
        self.assertIsNone(archive["binary_size_bytes"])

    def test_release_history_reads_binary_size_from_archive(self) -> None:
        releases = [
            {
                "tag_name": "v1.0.0",
                "published_at": "2026-01-01T00:00:00Z",
                "html_url": "https://example.test/v1.0.0",
                "assets": [
                    {
                        "name": "ivygrep-v1.0.0-linux-x86_64-musl.tar.gz",
                        "size": 123,
                        "browser_download_url": "https://example.test/archive",
                    }
                ],
            }
        ]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / releases[0]["assets"][0]["name"]
            binary = root / "ig"
            binary.write_bytes(b"binary")
            with tarfile.open(archive, "w:gz") as handle:
                handle.add(binary, arcname="ivygrep-v1.0.0-linux/ig")
            document = history.normalize(releases, root)
        self.assertEqual(
            document["releases"][0]["archives"][0]["binary_size_bytes"],
            len(b"binary"),
        )

    def test_windows_release_history_reads_binary_size(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "ivygrep-v1.0.0-windows-x86_64.zip"
            with zipfile.ZipFile(archive, "w") as handle:
                handle.writestr("ivygrep-v1.0.0-windows-x86_64/ig.exe", b"exe")
            self.assertEqual(history.binary_size(archive), 3)

    def test_release_gate_summary_requires_all_acceptance_controls(self) -> None:
        targets = "\n".join(
            (
                "x86_64-unknown-linux-musl",
                "x86_64-unknown-linux-gnu",
                "aarch64-unknown-linux-musl",
                "x86_64-apple-darwin",
                "aarch64-apple-darwin",
                "x86_64-pc-windows-msvc",
            )
        )
        workflow = (
            "artifact-acceptance:\n"
            "needs: artifact-acceptance\n"
            "scripts/verify_release_artifact.py\n"
            "scripts/e2e_x86_baseline.sh\n"
            "scripts/e2e_cached_model.sh\n"
            "anchore/sbom-action@\n"
            "actions/attest@\n"
            "archive_name: linux-x86_64-musl\n"
            "archive_name: linux-x86_64-cuda\n"
            "archive_name: linux-aarch64-musl\n"
            "archive_name: macos-x86_64\n"
            "archive_name: macos-aarch64\n"
            "archive_name: macos-aarch64-metal\n"
            "archive_name: windows-x86_64\n"
            f"{targets}\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(workflow, encoding="utf-8")
            _, summary = renderer.summarize("release-workflow", path)
        self.assertTrue(summary["artifact_acceptance"])
        self.assertEqual(summary["release_targets"], 6)
        self.assertEqual(summary["release_archives"], 7)
        self.assertTrue(summary["sbom"])
        self.assertTrue(summary["provenance"])

    def test_public_retrieval_summary_prefers_blended_routing(self) -> None:
        result = {
            "ivygrep_commit": "a" * 40,
            "summary": {
                "blended": {
                    "metrics": {
                        "ndcg_at_10": {"mean": 0.31},
                        "mrr_at_10": {"mean": 0.27},
                    }
                },
                "neural": {
                    "metrics": {
                        "ndcg_at_10": {"mean": 0.29},
                        "mrr_at_10": {"mean": 0.25},
                    }
                },
            },
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "retrieval.json"
            path.write_text(json.dumps(result), encoding="utf-8")
            _, summary = renderer.summarize("public-retrieval-current", path)
        self.assertEqual(summary["mode"], "blended")
        self.assertEqual(summary["ndcg_at_10"], 0.31)

    def test_current_million_summary_preserves_scope_and_release_binary(self) -> None:
        document = {
            "binary": {
                "commit": "a" * 40,
                "sha256": "b" * 64,
                "version": "ivygrep 1.2.7",
            },
            "corpus": {"license": "CC0-1.0"},
            "harness": {"trials": 3},
            "median": {
                "chunks_per_second": 123.0,
                "index_size_bytes": 456.0,
                "peak_rss_bytes": 789.0,
                "warm_cli_p95_ms": 1.25,
            },
            "scope": "synthetic hash-only scale and footprint measurement",
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "million.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            source, summary = renderer.summarize("million-scale-current", path)
        self.assertEqual(source, "a" * 40)
        self.assertEqual(summary["version"], "ivygrep 1.2.7")
        self.assertEqual(summary["scope"], "synthetic hash-only scale and footprint measurement")
        self.assertEqual(summary["harness"]["trials"], 3)

    def test_explicit_publication_revision_stays_stable_across_artifact_commit(self) -> None:
        document = {
            "binary": {
                "commit": "a" * 40,
                "sha256": "b" * 64,
                "version": "ivygrep 1.2.7",
            },
            "corpus": {"license": "CC0-1.0"},
            "harness": {"trials": 3},
            "median": {
                "chunks_per_second": 123.0,
                "index_size_bytes": 456.0,
                "peak_rss_bytes": 789.0,
                "warm_cli_p95_ms": 1.25,
            },
            "scope": "synthetic hash-only scale and footprint measurement",
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "docs").mkdir()
            artifact = root / "docs" / "million.json"
            release_history = root / "docs" / "history.json"
            manifest = root / "manifest.json"
            artifact.write_text(json.dumps(document) + "\n", encoding="utf-8")
            release_history.write_text(
                json.dumps({"schema_version": 1, "releases": []}) + "\n",
                encoding="utf-8",
            )
            manifest.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "evidence": [
                            {
                                "id": "million-scale-current",
                                "kind": "scale",
                                "label": "Current",
                                "path": "docs/million.json",
                            }
                        ],
                        "release_history": "docs/history.json",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            git = renderer.subprocess
            git.run(["git", "init", "-q"], cwd=root, check=True)
            git.run(["git", "add", "."], cwd=root, check=True)
            git.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "baseline",
                ],
                cwd=root,
                check=True,
            )
            baseline = git.run(
                ["git", "rev-parse", "HEAD"],
                cwd=root,
                check=True,
                text=True,
                stdout=git.PIPE,
            ).stdout.strip()
            manifest.write_text(
                manifest.read_text(encoding="utf-8").replace(
                    '"path": "docs/million.json"',
                    f'"path": "docs/million.json", "publication_commit": "{baseline}"',
                ),
                encoding="utf-8",
            )
            git.run(["git", "add", "manifest.json"], cwd=root, check=True)
            git.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "pin baseline",
                ],
                cwd=root,
                check=True,
            )
            document["description"] = "changed artifact"
            artifact.write_text(json.dumps(document) + "\n", encoding="utf-8")
            with mock.patch.object(renderer, "build_histories", return_value={}):
                before = renderer.build_dashboard(root, manifest)
            before_json = json.dumps(before, sort_keys=True)
            git.run(["git", "add", "docs/million.json"], cwd=root, check=True)
            git.run(
                [
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "publish artifact",
                ],
                cwd=root,
                check=True,
            )
            with mock.patch.object(renderer, "build_histories", return_value={}):
                after = renderer.build_dashboard(root, manifest)
            after_json = json.dumps(after, sort_keys=True)

        self.assertEqual(before, after)
        self.assertEqual(before_json, after_json)
        current = before["evidence"][0]
        self.assertEqual(
            current["publication_status"], "not pinned to an immutable revision"
        )
        self.assertIsNone(current["immutable_url"])

    def test_dashboard_output_is_metrics_only(self) -> None:
        dashboard = json.loads(
            (ROOT / "docs" / "benchmarks" / "evidence-dashboard.json").read_text(
                encoding="utf-8"
            )
        )
        html = (
            ROOT / "docs" / "benchmarks" / "evidence-dashboard.html"
        ).read_text(encoding="utf-8")
        self.assertEqual(
            {
                "evidence",
                "freshness",
                "histories",
                "release_history",
                "release_history_artifact",
                "schema_version",
            },
            set(dashboard),
        )
        self.assertIn("Benchmark dashboard", html)
        self.assertIn("nDCG@10", html)
        self.assertIn("Index throughput", html)
        self.assertIn("not pinned to an immutable revision", html)
        self.assertNotIn("policy", html.lower())
        current = next(
            item
            for item in dashboard["evidence"]
            if item["id"] == "public-retrieval-current"
        )
        release = next(
            item
            for item in dashboard["evidence"]
            if item["id"] == "release-workflow"
        )
        self.assertEqual(current["summary"]["mode"], "blended")
        self.assertIn("Historical", current["label"])
        self.assertEqual(
            dashboard["freshness"]["evidence"]["public-retrieval-current"]["status"],
            "historical",
        )
        self.assertEqual(release["summary"]["release_archives"], 7)
        current_scale = next(
            item
            for item in dashboard["evidence"]
            if item["id"] == "million-scale-current"
        )
        self.assertEqual(
            dashboard["freshness"]["evidence"]["million-scale-current"]["status"],
            "historical",
        )
        current_head_item = next(
            item
            for item in dashboard["evidence"]
            if item["id"] == "current-head-relevance"
        )
        self.assertEqual(
            current_head_item["summary"]["version"],
            f"ivygrep {dashboard['freshness']['package_version']}",
        )
        self.assertEqual(
            dashboard["freshness"]["evidence"]["current-head-relevance"]["status"],
            "current",
        )
        self.assertEqual(current_scale["summary"]["version"].split()[0], "ivygrep")
        self.assertTrue(dashboard["release_history"]["releases"][0]["tag"].startswith("v"))
        self.assertIn("Current package retrieval screen", html)
        self.assertIn("Latest measured release scope", html)


if __name__ == "__main__":
    unittest.main()
