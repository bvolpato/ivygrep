import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "soak_daemon.py"
SPEC = importlib.util.spec_from_file_location("soak_daemon", SCRIPT)
assert SPEC and SPEC.loader
soak = importlib.util.module_from_spec(SPEC)
with mock.patch.object(sys, "path", [str(SCRIPT.parent), *sys.path]):
    SPEC.loader.exec_module(soak)


class DaemonSoakTest(unittest.TestCase):
    def test_probe_rejects_stale_content_duplicates_and_deleted_hits(self):
        expected = "pub fn soak_revision() -> u64 { 42 }"
        hit = {"file_path": soak.PROBE, "preview": expected + "\n"}
        self.assertTrue(soak.probe_matches([hit], expected))
        self.assertFalse(soak.probe_matches([hit], expected.replace("42", "43")))
        self.assertFalse(soak.probe_matches([hit, hit], expected))
        self.assertFalse(soak.probe_matches([hit], None))
        self.assertFalse(soak.probe_matches([], expected))
        self.assertTrue(soak.probe_matches([], None))

    def test_watcher_waits_for_indexed_revision_and_fails_if_it_stays_stale(self):
        expected = "pub fn soak_revision() -> u64 { 2 }"
        fresh = [{"file_path": soak.PROBE, "preview": expected}]
        stale = [{"file_path": soak.PROBE, "preview": expected.replace("2", "1")}]
        with mock.patch.object(soak, "search", side_effect=[stale, fresh]) as search, mock.patch.object(soak.time, "sleep"):
            soak.watcher_observed_probe(Path("home"), Path("repo"), expected)
            self.assertEqual(search.call_count, 2)
        with mock.patch.object(soak, "search", return_value=stale), mock.patch.object(soak.time, "monotonic", side_effect=[0, 21]):
            with self.assertRaisesRegex(AssertionError, "stale probe"):
                soak.watcher_observed_probe(Path("home"), Path("repo"), expected)

    def test_stale_probe_evidence_records_what_each_layer_holds(self):
        with tempfile.TemporaryDirectory() as temporary:
            home, repo = Path(temporary) / "home", Path(temporary) / "repo"
            index = home / "indexes" / "workspace"
            index.mkdir(parents=True)
            (repo / "src").mkdir(parents=True)
            current = "pub fn soak_revision() -> u64 { 42 }"
            stale = current.replace("42", "41")
            (repo / soak.PROBE).write_text(current + "\n")
            connection = sqlite3.connect(index / "metadata.sqlite3")
            connection.execute("CREATE TABLE chunks (file_path TEXT NOT NULL, text TEXT NOT NULL)")
            connection.execute("INSERT INTO chunks VALUES (?, ?)", (soak.PROBE, stale))
            connection.execute("INSERT INTO chunks VALUES (?, ?)", ("src/lib.rs", "pub fn other() {}"))
            connection.commit()
            connection.close()
            (index / "merkle_snapshot.json").write_text(json.dumps({"files": {soak.PROBE: "1-0"}}))
            # The store holds the previous revision, and the daemon answers with it.
            hit = [{"file_path": soak.PROBE, "preview": stale, "reason": "lexical"}]
            with mock.patch.object(soak, "search", return_value=hit), \
                    mock.patch.object(soak, "run", return_value="?? src/soak_probe.rs\n") as run:
                evidence = soak.stale_probe_evidence(home, repo, {})
            self.assertEqual(run.call_args.args[0][0], "git")
            self.assertEqual(evidence, {
                "probe_file_text": current + "\n",
                "git_worktree_clean": False,
                "index_stores": [{"sqlite_probe_texts": [stale], "snapshot_lists_probe": True,
                                  "clean_checkout_state_recorded": False}],
                "daemon_uncached_query_hits": [{"preview": stale, "reason": "lexical"}],
            })
            # A fact that cannot be read does not hide the other facts.
            (index / "merkle_snapshot.json").unlink()
            (repo / soak.PROBE).unlink()
            with mock.patch.object(soak, "search", return_value=[]), mock.patch.object(soak, "run", return_value=""):
                evidence = soak.stale_probe_evidence(home, repo, {})
            self.assertIsNone(evidence["probe_file_text"])
            self.assertTrue(evidence["git_worktree_clean"])
            self.assertTrue(evidence["index_stores"].startswith("unavailable: "))
            self.assertEqual(evidence["daemon_uncached_query_hits"], [])

    def test_resource_gates_reject_rss_fd_and_thread_growth(self):
        budgets = soak.resource_budgets(rss_growth_mib=32, total_rss_growth_mib=96, fd_growth=8, thread_growth=4)
        stable = [{"rss_bytes": 100 * 1024**2, "rss_anon_bytes": 60 * 1024**2, "fds": 50, "threads": 16}
                  for _ in range(100)]
        self.assertTrue(soak.resource_gate(stable, budgets)["passed"])
        for resource, increase in (("rss_anon_bytes", 1024**2), ("rss_bytes", 2 * 1024**2), ("fds", 1),
                                   ("threads", 1)):
            growing = [{**sample, resource: sample[resource] + index * increase}
                       for index, sample in enumerate(stable)]
            gate = soak.resource_gate(growing, budgets)
            self.assertFalse(gate["passed"], resource)
            self.assertFalse(gate["metrics"][resource]["passed"])
        with self.assertRaisesRegex(ValueError, "20 load samples"):
            soak.resource_gate(stable[:10], budgets)

    def test_mapped_index_page_swings_do_not_look_like_a_leak(self):
        # Reindexing replaces mapped segments, so file-backed RSS can move by
        # tens of MiB between windows while anonymous memory stays flat.
        budgets = soak.resource_budgets(rss_growth_mib=32, total_rss_growth_mib=96, fd_growth=8, thread_growth=4)
        anon = 55 * 1024**2
        samples = [{"rss_bytes": anon + (30 if index < 24 else 80) * 1024**2, "rss_anon_bytes": anon,
                    "fds": 30, "threads": 20} for index in range(40)]
        gate = soak.resource_gate(samples, budgets)
        self.assertTrue(gate["passed"], gate)
        self.assertEqual(gate["metrics"]["rss_bytes"]["growth"], 50 * 1024**2)
        self.assertEqual(gate["metrics"]["rss_anon_bytes"]["growth"], 0)

    def test_resource_warmup_and_transient_peak_do_not_look_like_a_leak(self):
        samples = [{"rss_bytes": 10 if index < 20 else 100} for index in range(100)]
        samples[70]["rss_bytes"] = 1000
        gate = soak.resource_gate(samples, {"rss_bytes": 0})
        self.assertTrue(gate["passed"])
        self.assertEqual(gate["metrics"]["rss_bytes"]["peak"], 1000)

    def test_missing_process_or_rpc_failure_cannot_pass_as_zero_activity(self):
        child = subprocess.Popen([sys.executable, "-c", ""])
        child.wait()
        # Linux fails on the /proc read. macOS asks libproc about a process that no longer exists.
        with mock.patch.object(Path, "read_text", side_effect=FileNotFoundError):
            with self.assertRaises((FileNotFoundError, ProcessLookupError)):
                soak.process_sample(child.pid)
        with mock.patch.object(soak, "daemon_request", side_effect=ConnectionRefusedError):
            with self.assertRaises(ConnectionRefusedError):
                soak.search(Path("home"), Path("repo"), "query")
        with mock.patch.object(soak, "daemon_request", return_value={"type": "status"}):
            with self.assertRaisesRegex(RuntimeError, "unexpected daemon search"):
                soak.search(Path("home"), Path("repo"), "query")

    @unittest.skipUnless(sys.platform == "darwin", "macOS libproc sampler")
    def test_macos_sample_follows_descriptors_and_threads_of_a_live_process(self):
        before = soak.process_sample(os.getpid())
        self.assertEqual(set(before), {"rss_bytes", "rss_anon_bytes", "fds", "threads"})
        release = threading.Event()
        thread = threading.Thread(target=release.wait)
        thread.start()
        try:
            with open(os.devnull, "rb"):
                during = soak.process_sample(os.getpid())
        finally:
            release.set()
            thread.join()
        self.assertEqual(during["fds"], before["fds"] + 1)
        self.assertEqual(during["threads"], before["threads"] + 1)
        self.assertGreater(min(before["rss_bytes"], before["rss_anon_bytes"]), 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
