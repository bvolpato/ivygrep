import importlib.util
from pathlib import Path
import sys
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

    def test_resource_gates_reject_rss_fd_and_thread_growth(self):
        budgets = {"rss_bytes": 32 * 1024**2, "fds": 8, "threads": 4}
        stable = [{"rss_bytes": 100 * 1024**2, "fds": 50, "threads": 16} for _ in range(100)]
        self.assertTrue(soak.resource_gate(stable, budgets)["passed"])
        for resource, increase in (("rss_bytes", 1024**2), ("fds", 1), ("threads", 1)):
            growing = [{**sample, resource: sample[resource] + index * increase}
                       for index, sample in enumerate(stable)]
            gate = soak.resource_gate(growing, budgets)
            self.assertFalse(gate["passed"], resource)
            self.assertFalse(gate["metrics"][resource]["passed"])
        with self.assertRaisesRegex(ValueError, "20 load samples"):
            soak.resource_gate(stable[:10], budgets)

    def test_resource_warmup_and_transient_peak_do_not_look_like_a_leak(self):
        samples = [{"rss_bytes": 10 if index < 20 else 100} for index in range(100)]
        samples[70]["rss_bytes"] = 1000
        gate = soak.resource_gate(samples, {"rss_bytes": 0})
        self.assertTrue(gate["passed"])
        self.assertEqual(gate["metrics"]["rss_bytes"]["peak"], 1000)

    def test_missing_process_or_rpc_failure_cannot_pass_as_zero_activity(self):
        with mock.patch.object(Path, "read_text", side_effect=FileNotFoundError):
            with self.assertRaises(FileNotFoundError):
                soak.process_sample(123)
        with mock.patch.object(soak, "daemon_request", side_effect=ConnectionRefusedError):
            with self.assertRaises(ConnectionRefusedError):
                soak.search(Path("home"), Path("repo"), "query")
        with mock.patch.object(soak, "daemon_request", return_value={"type": "status"}):
            with self.assertRaisesRegex(RuntimeError, "unexpected daemon search"):
                soak.search(Path("home"), Path("repo"), "query")


if __name__ == "__main__":
    unittest.main()
