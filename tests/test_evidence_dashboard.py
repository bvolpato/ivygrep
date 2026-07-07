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


claims = load_script("check_evidence_claims")
history = load_script("normalize_release_history")
renderer = load_script("render_evidence_dashboard")


class EvidenceDashboardTest(unittest.TestCase):
    def test_retained_publication_commit_does_not_require_git_history(
        self,
    ) -> None:
        retained = "a" * 40
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / "artifact.json"
            path.write_text("{}\n", encoding="utf-8")
            with mock.patch.object(renderer.subprocess, "run") as run:
                commit = renderer.publication_commit(root, path, retained)
        self.assertEqual(commit, retained)
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

    def test_daemon_summary_uses_last_retained_iteration(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "results.tsv"
            path.write_text(
                "iteration\tcommit\tmetric\tdelta\tguard\tstatus\tdescription\n"
                "0\tbase\t455.0\t0\t-\tbaseline\tbaseline\n"
                "1\tkept\t4.9\t-450.1\tpass\tkeep\tcache\n"
                "2\tdropped\t3.0\t-1.9\tfail\tdiscard\tbroken\n",
                encoding="utf-8",
            )
            source_commit, summary = renderer.summarize("daemon-cache", path)
        self.assertEqual(source_commit, "kept")
        self.assertEqual(summary["retained_warm_p95_ms"], 4.9)

    def test_release_gate_summary_requires_all_acceptance_controls(self) -> None:
        targets = "\n".join(
            (
                "x86_64-unknown-linux-musl",
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
            f"{targets}\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "release.yml"
            path.write_text(workflow, encoding="utf-8")
            _, summary = renderer.summarize("release-workflow", path)
        self.assertTrue(summary["artifact_acceptance"])
        self.assertEqual(summary["release_targets"], 5)
        self.assertTrue(summary["sbom"])
        self.assertTrue(summary["provenance"])

    def test_unsupported_marketing_claim_is_rejected(self) -> None:
        dashboard = {
            "claims": {
                "state_of_the_art": {"supported": False},
                "competitive": {"supported": False},
                "portable": {"supported": True},
            },
            "evidence": [
                {
                    "id": "daemon-cache",
                    "summary": {"retained_warm_p95_ms": 4.9},
                }
            ],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "README.md"
            path.write_text("state-of-the-art search", encoding="utf-8")
            self.assertTrue(claims.check(dashboard, [path]))

    def test_claim_control_files_can_describe_claim_policy(self) -> None:
        dashboard = {
            "claims": {
                "state_of_the_art": {"supported": False},
                "competitive": {"supported": False},
                "portable": {"supported": True},
            },
            "evidence": [
                {
                    "id": "daemon-cache",
                    "summary": {"retained_warm_p95_ms": 4.9},
                }
            ],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "claims-policy.md"
            path.write_text(
                "State of the art: not claimed\nCompetitive: not claimed\n",
                encoding="utf-8",
            )
            self.assertFalse(claims.check(dashboard, [path]))

    def test_sub_100_ms_claim_requires_matching_evidence(self) -> None:
        dashboard = {
            "claims": {
                "state_of_the_art": {"supported": False},
                "competitive": {"supported": False},
                "portable": {"supported": True},
            },
            "evidence": [
                {
                    "id": "daemon-cache",
                    "summary": {"retained_warm_p95_ms": 120.0},
                }
            ],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "site.html"
            path.write_text("Sub-100-ms warm daemon replay", encoding="utf-8")
            self.assertTrue(claims.check(dashboard, [path]))


if __name__ == "__main__":
    unittest.main()
