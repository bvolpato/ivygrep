import importlib.util
import io
import json
from pathlib import Path
import random
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "soak_mcp_sessions.py"
SPEC = importlib.util.spec_from_file_location("soak_mcp_sessions", SCRIPT)
assert SPEC and SPEC.loader
soak = importlib.util.module_from_spec(SPEC)
with mock.patch.object(sys, "path", [str(SCRIPT.parent), *sys.path]):
    SPEC.loader.exec_module(soak)

MIB = 1024 * 1024
ROLLUP = """55d0c0a00000-7ffd1b5f6000 ---p 00000000 00:00 0                          [rollup]
Rss:              141204 kB
Pss:              133180 kB
Pss_Anon:         114280 kB
Shared_Clean:      16048 kB
Anonymous:        114280 kB
Swap:                  0 kB
"""


class ProcSamplingTest(unittest.TestCase):
    def test_smaps_rollup_reports_anonymous_memory_apart_from_mapped_index_files(self):
        sample = soak.parse_smaps_rollup(ROLLUP)
        self.assertEqual(sample, {"rss_bytes": 141204 * 1024, "pss_bytes": 133180 * 1024,
                                  "rss_anon_bytes": 114280 * 1024})
        with self.assertRaises(KeyError):
            soak.parse_smaps_rollup("Rss: 10 kB\n")

    def test_inotify_watches_are_counted_per_descriptor(self):
        fdinfo = ("pos:\t0\nflags:\t02004000\nmnt_id:\t15\nino:\t1057\n"
                  "inotify wd:2 ino:a1 sdev:fd00001 mask:fce ignored_mask:0 fhandle-bytes:8\n"
                  "inotify wd:1 ino:9f sdev:fd00001 mask:fce ignored_mask:0 fhandle-bytes:8\n")
        self.assertEqual(soak.inotify_watch_count(fdinfo), 2)
        self.assertEqual(soak.inotify_watch_count("pos:\t0\nflags:\t02\n"), 0)

    def test_idle_wakeups_ignore_threads_that_exited_and_count_new_threads_from_zero(self):
        before = {10: 1_000, 11: 50_000, 12: 7}
        after = {10: 1_040, 12: 7, 13: 5}
        # Thread 11 exited: its 50,000 earlier switches must not read as negative activity.
        self.assertEqual(soak.wakeups(before, after), 45)
        self.assertEqual(soak.wakeups(before, before), 0)

    def test_missing_process_cannot_pass_as_a_zero_sample(self):
        with mock.patch.object(Path, "read_text", side_effect=FileNotFoundError):
            with self.assertRaises(FileNotFoundError):
                soak.process_sample(123)

    def test_processes_are_classified_by_role_so_orphans_and_extra_daemons_show(self):
        kinds = soak.classify_processes([
            {"pid": 1, "argv": ["/bin/ig", "--daemon"]}, {"pid": 2, "argv": ["/bin/ig", "--mcp"]},
            {"pid": 3, "argv": ["/bin/ig", "--enhance-hash-internal", "/repo"]},
            {"pid": 4, "argv": ["python3", "soak_mcp_sessions.py"]}, {"pid": 5, "argv": ["/bin/ig", "--status"]}])
        self.assertEqual(kinds, {"daemon": [1], "mcp": [2], "enhancement": [3], "other": [5]})
        self.assertTrue(soak.lifecycle_gate(kinds | {"mcp": []}, expect_daemon=True)["passed"])
        self.assertFalse(soak.lifecycle_gate(kinds, expect_daemon=True)["passed"], "an MCP session outlived its client")
        self.assertFalse(soak.lifecycle_gate(kinds | {"mcp": [], "daemon": [1, 9]}, expect_daemon=True)["passed"])
        self.assertFalse(soak.lifecycle_gate(kinds | {"mcp": [], "daemon": []}, expect_daemon=True)["passed"])

    def test_a_forked_child_that_has_not_execed_yet_is_not_a_second_daemon_or_session(self):
        # Between fork and exec of `git` or a worker, the child still shows its parent's command line.
        kinds = soak.classify_processes([
            {"pid": 100, "ppid": 1, "argv": ["/bin/ig", "--daemon"]},
            {"pid": 101, "ppid": 100, "argv": ["/bin/ig", "--daemon"]},
            {"pid": 200, "ppid": 50, "argv": ["/bin/ig", "--mcp"]},
            {"pid": 201, "ppid": 200, "argv": ["/bin/ig", "--mcp"]},
            # A daemon that a session auto-spawned is a real daemon.
            {"pid": 300, "ppid": 200, "argv": ["/bin/ig", "--daemon"]}])
        self.assertEqual((kinds["daemon"], kinds["mcp"]), ([100, 300], [200]))

    def test_index_directories_of_deleted_roots_count_as_orphans(self):
        with tempfile.TemporaryDirectory() as temp:
            home, live = Path(temp) / "home", Path(temp) / "live"
            live.mkdir()
            for name, root in (("a", live), ("b", Path(temp) / "deleted-worktree")):
                index = home / "indexes" / name
                index.mkdir(parents=True)
                (index / "workspace.json").write_text(json.dumps({"root": str(root)}))
                (index / "metadata.sqlite3").write_bytes(b"x" * 8192)
            stats = soak.index_store_stats(home, sizes=True)
            self.assertEqual((stats["index_dirs"], stats["orphan_index_dirs"]), (2, 1))
            self.assertGreaterEqual(stats["index_bytes"], 2 * 8192)
            self.assertEqual(soak.index_store_stats(Path(temp) / "empty", sizes=False)["index_dirs"], 0)

    def test_vectors_count_as_current_only_for_the_index_generation_they_were_built_from(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            states = {"current": ("7", "7", ""), "edited-since": ("7", "6", "neural"), "waiting": (None, None, "queued")}
            for name, (hash_generation, neural_generation, phase) in states.items():
                index = home / "indexes" / name
                index.mkdir(parents=True)
                (index / "workspace.json").write_text(json.dumps({"root": f"/repos/{name}", "index_generation": 7}))
                if hash_generation:
                    (index / ".hash_enhanced_generation").write_text(hash_generation)
                    (index / ".neural_enhanced_generation").write_text(neural_generation + "\n")
                if phase:
                    (index / ".enhancing.phase").write_text(phase)
            progress = soak.enhancement_progress(home)
            self.assertEqual(progress["/repos/current"], {"hash": True, "neural": True, "queued": False})
            self.assertEqual(progress["/repos/edited-since"], {"hash": True, "neural": False, "queued": False})
            self.assertEqual(progress["/repos/waiting"], {"hash": False, "neural": False, "queued": True})


class GateTest(unittest.TestCase):
    def test_slope_separates_a_steady_climb_from_noise(self):
        rng = random.Random(7)
        flat = [{"elapsed_seconds": second * 10.0, "rss_anon_bytes": 300 * MIB + rng.randint(-MIB, MIB)}
                for second in range(720)]
        fit = soak.scale(soak.growth_per_hour(flat, "rss_anon_bytes"), MIB)
        self.assertLess(abs(fit["per_hour"]), 0.5)
        self.assertLess(fit["ci95_per_hour"], 0.5)
        self.assertAlmostEqual(fit["hours"], 1.6, places=1)
        leaking = [{**sample, "rss_anon_bytes": sample["rss_anon_bytes"] + int(sample["elapsed_seconds"] / 3600 * 8 * MIB)}
                   for sample in flat]
        fit = soak.scale(soak.growth_per_hour(leaking, "rss_anon_bytes"), MIB)
        self.assertAlmostEqual(fit["per_hour"], 8.0, delta=0.5)
        self.assertGreater(fit["per_hour"] - fit["ci95_per_hour"], 7.0, "8 MiB/h must not hide in the interval")
        with self.assertRaisesRegex(ValueError, "at least 3"):
            soak.linear_slope([(0.0, 1.0), (1.0, 2.0)])
        with self.assertRaisesRegex(ValueError, "different times"):
            soak.linear_slope([(1.0, 1.0)] * 5)

    def test_warmup_is_excluded_from_the_slope(self):
        samples = [{"elapsed_seconds": float(second), "fds": 500 if second < 20 else 40} for second in range(100)]
        self.assertEqual(soak.growth_per_hour(samples, "fds")["per_hour"], 0)

    def test_sessions_are_summed_for_the_machine_and_maxed_for_the_per_session_gate(self):
        small = {"rss_bytes": 15 * MIB, "pss_bytes": 5 * MIB, "rss_anon_bytes": 2 * MIB, "fds": 9, "threads": 3}
        fat = {"rss_bytes": 95 * MIB, "pss_bytes": 79 * MIB, "rss_anon_bytes": 74 * MIB, "fds": 9, "threads": 44}
        summary = soak.summarize_sessions([small] * 49 + [fat])
        self.assertEqual(summary["sessions"], 50)
        self.assertEqual(summary["total_rss_anon_bytes"], (49 * 2 + 74) * MIB)
        self.assertEqual((summary["rss_anon_bytes"], summary["threads"]), (74 * MIB, 44))
        with self.assertRaisesRegex(RuntimeError, "no MCP session"):
            soak.summarize_sessions([])

    def test_one_session_growing_fails_the_session_gate_even_when_the_rest_are_flat(self):
        budgets = soak.session_budgets(rss_growth_mib=16, fd_growth=4, thread_growth=2)
        flat = [{"rss_anon_bytes": 2 * MIB, "fds": 9, "threads": 3} for _ in range(100)]
        self.assertTrue(soak.resource_gate(flat, budgets)["passed"])
        growing = [{**sample, "rss_anon_bytes": (2 + index) * MIB} for index, sample in enumerate(flat)]
        self.assertFalse(soak.resource_gate(growing, budgets)["metrics"]["rss_anon_bytes"]["passed"])

    def test_churn_gate_rejects_watchers_and_index_directories_left_by_deleted_worktrees(self):
        budgets = {"rss_anon_bytes": 32 * MIB, "fds": 8, "threads": 4, "inotify_instances": 0,
                   "index_dirs": 1, "orphan_index_dirs": 1}
        baseline = {"rss_anon_bytes": 104 * MIB, "fds": 18, "threads": 44, "inotify_instances": 1,
                    "index_dirs": 1, "orphan_index_dirs": 0}
        self.assertTrue(soak.churn_gate(baseline, baseline | {"rss_anon_bytes": 110 * MIB}, budgets)["passed"])
        # What 500 churned worktrees left behind before deleted roots were released and collected.
        leaked = {"rss_anon_bytes": 790 * MIB, "fds": 1514, "threads": 551, "inotify_instances": 501,
                  "index_dirs": 501, "orphan_index_dirs": 500}
        gate = soak.churn_gate(baseline, leaked, budgets)
        self.assertFalse(gate["passed"])
        self.assertEqual({name for name, metric in gate["metrics"].items() if not metric["passed"]}, set(budgets))
        self.assertEqual(gate["metrics"]["inotify_instances"]["growth"], 500)

    def test_memory_is_reported_without_gating_where_it_cannot_tell_a_leak_from_retention(self):
        gate = {"passed": False, "sample_count": 40, "metrics": {
            "rss_anon_bytes": {"growth": 40 * soak.MIB, "budget": 32 * soak.MIB, "passed": False},
            "threads": {"growth": 1, "budget": 4, "passed": True}}}
        relaxed = soak.without_memory_gates(gate)
        self.assertTrue(relaxed["passed"])
        self.assertFalse(relaxed["metrics"]["rss_anon_bytes"]["gating"])
        self.assertFalse(relaxed["metrics"]["rss_anon_bytes"]["passed"], "the overrun stays visible")
        # A thread leak still fails.
        gate["metrics"]["threads"]["passed"] = False
        self.assertFalse(soak.without_memory_gates(gate)["passed"])

    def test_latency_drift_compares_first_and_last_quarter(self):
        self.assertIsNone(soak.latency_drift([(float(index), 5.0) for index in range(39)]))
        steady = soak.latency_drift([(float(index), 5.0) for index in range(400)])
        self.assertEqual((steady["first_p50_ms"], steady["last_p50_ms"], steady["p50_ratio"]), (5.0, 5.0, 1.0))
        slowing = soak.latency_drift([(float(index), 5.0 + index / 10) for index in range(400)])
        self.assertGreater(slowing["p50_ratio"], 4)
        self.assertGreater(slowing["last_p95_ms"], slowing["first_p95_ms"])


class WorkloadTest(unittest.TestCase):
    def test_calls_cover_every_mode_and_keep_evicting_the_query_cache(self):
        rng = random.Random(1)
        workspaces = [Path("/w/a"), Path("/w/b"), Path("/w/c")]
        calls = [soak.pick_call(rng, workspaces, workspaces[0], sequence) for sequence in range(3000)]
        self.assertEqual({kind for kind, _, _ in calls}, {name for name, _ in soak.CALL_WEIGHTS})
        self.assertEqual({tool for _, tool, _ in calls}, {"ig_search", "ig_status"})
        hybrid = [arguments["query"] for kind, _, arguments in calls if kind.startswith("hybrid")]
        self.assertGreater(len(set(hybrid)), 128 * 2, "distinct queries must outnumber the daemon's 128-entry cache")
        self.assertLess(len(set(hybrid)), len(hybrid), "repeated queries must exercise cache hits too")
        away = sum(1 for _, _, arguments in calls if arguments.get("path") not in (None, "/w/a"))
        self.assertTrue(0 < away < len(calls) / 5, "sessions mostly search their own workspace")
        packs = [arguments for kind, _, arguments in calls if kind == "context_pack"]
        self.assertTrue(all(arguments["output"] == "context_pack" for arguments in packs))
        for _, _, arguments in calls:
            self.assertLessEqual(sum(bool(arguments.get(mode)) for mode in ("literal", "regex", "symbol")), 1)

    def test_a_selected_call_kind_is_the_only_one_issued_and_scoped_calls_stay_in_the_workspace(self):
        with tempfile.TemporaryDirectory() as temp:
            workspace = Path(temp) / "workspace"
            (workspace / "src").mkdir(parents=True)
            rng = random.Random(7)
            calls = [soak.pick_call(rng, [workspace], workspace, sequence, (("scoped", 1),)) for sequence in range(200)]
            self.assertEqual({kind for kind, _, _ in calls}, {"scoped"})
            paths = {arguments["path"] for _, _, arguments in calls}
            # Scopes that this workspace does not have fall back to its root.
            self.assertEqual(paths, {str(workspace), str(workspace / "src")})

    def test_tool_errors_and_protocol_errors_never_count_as_successful_calls(self):
        ok = {"result": {"isError": False, "structuredContent": {"result_count": 2}, "content": []}}
        self.assertEqual(soak.tool_payload(ok), {"result_count": 2})
        with self.assertRaisesRegex(soak.McpError, "tool error: ivygrep daemon"):
            soak.tool_payload({"result": {"isError": True, "content": [{"text": "ivygrep daemon rejected"}]}})
        with self.assertRaisesRegex(soak.McpError, "JSON-RPC error"):
            soak.tool_payload({"error": {"code": -32603, "message": "boom"}})


class FakeProcess:
    """Stands in for `ig --mcp`: replies arrive on a pipe the client reads with `select`."""

    def __init__(self, replies: list[dict]):
        import os
        self.read_end, self.write_end = os.pipe()
        self.stdout = os.fdopen(self.read_end, "rb", buffering=0)
        self.stdin = io.BytesIO()
        self.pid = 4242
        os.write(self.write_end, b"".join(json.dumps(reply).encode() + b"\n" for reply in replies))

    def finish(self):
        import os
        os.close(self.write_end)


class McpClientTest(unittest.TestCase):
    def client(self, replies: list[dict]) -> tuple:
        process = FakeProcess(replies)
        self.addCleanup(process.stdout.close)
        with mock.patch.object(soak.subprocess, "Popen", return_value=process):
            return soak.McpClient(Path("ig"), {}, Path(".")), process

    def test_responses_are_matched_by_id_and_notifications_are_skipped(self):
        client, process = self.client([{"jsonrpc": "2.0", "method": "notifications/progress"},
                                       {"jsonrpc": "2.0", "id": 99, "result": {"stale": True}},
                                       {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}])
        self.assertEqual(client.request("ping", timeout=5)["result"], {"ok": True})
        sent = json.loads(process.stdin.getvalue())
        self.assertEqual((sent["method"], sent["id"]), ("ping", 1))
        process.finish()

    def test_a_silent_or_closed_session_fails_instead_of_hanging_the_soak(self):
        client, process = self.client([])
        with self.assertRaisesRegex(soak.McpError, "no MCP response within 0.05s"):
            client.request("ping", timeout=0.05)
        process.finish()
        with self.assertRaisesRegex(soak.McpError, "closed stdout"):
            client.request("ping", timeout=5)

    def test_first_searches_wait_while_the_daemon_reports_indexing(self):
        indexing = {"status": "indexing", "retry_after_secs": 0}
        client = mock.Mock()
        client.call.side_effect = [indexing, indexing, {"result_count": 1}]
        with mock.patch.object(soak.time, "sleep"):
            soak.wait_until_searchable(client, Path("/w/a"))
        self.assertEqual(client.call.call_count, 3)
        client.call.side_effect = None
        client.call.return_value = indexing
        with mock.patch.object(soak.time, "sleep"), mock.patch.object(soak.time, "monotonic", side_effect=[0, 601]):
            with self.assertRaisesRegex(soak.McpError, "still indexing"):
                soak.wait_until_searchable(client, Path("/w/a"))


if __name__ == "__main__":
    unittest.main()
