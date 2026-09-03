import importlib.util
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import threading
from types import SimpleNamespace
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


benchmark = load_script("bench_million_chunks")
comparator = load_script("compare_million_benchmarks")
sys.modules["compare_million_benchmarks"] = comparator
renderer = load_script("render_million_benchmark")


def artifact(
    latencies,
    throughput=100.0,
    recall=1.0,
    commit="commit",
    size_bytes=1000,
):
    return {
        "ivygrep_commit": commit,
        "index": {
            "chunks_per_second": throughput,
            "size_bytes": size_bytes,
            "metrics": {
                "peak_disk_bytes": size_bytes * 2,
                "peak_rss_bytes": 100,
            },
        },
        "queries": {
            "warm_distinct": {
                "latency_samples_ms": latencies,
                "expected_recall_at_20": recall,
            }
        },
    }


class MillionBenchmarkTest(unittest.TestCase):
    def report_inputs(self):
        names = {
            "index_baseline": "index-baseline",
            "index_current": "index-current",
            "system_baseline": "system-baseline",
            "system_current": "system-current",
            "paired_baseline": "query-baseline",
            "paired_current": "query-current",
            "quality_baseline": "quality-current",
            "quality_current": "quality-current",
        }
        return {
            name: json.loads((ROOT / "docs/benchmarks" / f"public-million-{suffix}.json").read_text())
            for name, suffix in names.items()
        }

    def test_report_does_not_turn_unobserved_resources_into_improvements(self):
        inputs = self.report_inputs()
        for name in ("index_current", "system_current"):
            inputs[name]["index"]["metrics"].update(
                resource_samples=0, peak_rss_bytes=0, cpu_ms=0,
                filesystem_read_bytes=0, filesystem_write_bytes=0,
            )
        with mock.patch.object(comparator, "bootstrap_p95_ratio", return_value={
            "observed": 0.4, "ci95_lower": 0.35, "ci95_upper": 0.45,
        }):
            report = renderer.build_report(**inputs)
        for name in ("peak_rss_bytes", "filesystem_read_bytes", "filesystem_write_bytes"):
            self.assertIsNone(report["indexing"][name]["current"])
            self.assertIsNone(report["indexing"][name]["ratio"])
        self.assertIsNone(report["saturated_full_run"]["current_index_cpu_ms"])
        self.assertEqual(report["indexing"]["wall_ms"]["current"],
                         inputs["index_current"]["index"]["metrics"]["wall_ms"])
        # Final disk accounting is measured separately from process sampling.
        self.assertEqual(report["indexing"]["peak_disk_bytes"]["current"],
                         inputs["index_current"]["index"]["metrics"]["peak_disk_bytes"])
        self.assertFalse(report["gate"]["indexing_ceiling_documented"])
        markdown = renderer.render_markdown(report)
        self.assertIn("unobserved", markdown)
        self.assertIn("(n/a)", markdown)
        self.assertNotIn("reduced\nwrites", markdown)
        self.assertIn("FAIL", renderer.render_html(report))

    def test_report_handles_observed_zero_resource_baselines(self):
        inputs = self.report_inputs()
        inputs["index_baseline"]["index"]["metrics"].update(
            resource_samples=3, peak_rss_bytes=0, peak_disk_bytes=0,
            filesystem_read_bytes=0, filesystem_write_bytes=0,
        )
        with mock.patch.object(comparator, "bootstrap_p95_ratio", return_value={
            "observed": 0.4, "ci95_lower": 0.35, "ci95_upper": 0.45,
        }):
            report = renderer.build_report(**inputs)
        for name in (
            "peak_rss_bytes", "peak_disk_bytes", "filesystem_read_bytes", "filesystem_write_bytes",
        ):
            self.assertEqual(report["indexing"][name]["baseline"], 0)
            self.assertIsNone(report["indexing"][name]["ratio"])
        markdown = renderer.render_markdown(report)
        self.assertIn("(n/a)", markdown)
        self.assertNotIn("unobserved", markdown)
        self.assertNotIn("reduced\nwrites", markdown)
        renderer.render_html(report)

    def test_comparison_requires_observed_nonzero_resource_baselines(self):
        for missing_side in (
            "baseline", "current", "zero_baseline", "mixed_current", "legacy", "observed_zero",
        ):
            with self.subTest(missing_side=missing_side):
                baseline, current = artifact([100.0] * 40), artifact([100.0] * 40)
                if missing_side == "mixed_current":
                    current = [current, artifact([100.0] * 40)]
                    missing = current[-1]
                else:
                    missing = current if missing_side in ("current", "observed_zero") else baseline
                if missing_side != "legacy":
                    missing["index"]["metrics"].update(
                        resource_samples=3 if missing_side in ("zero_baseline", "observed_zero") else 0,
                        peak_rss_bytes=0,
                    )
                result = comparator.compare_runs(
                    [baseline], current if isinstance(current, list) else [current],
                    significant_regression_ratio=1.15,
                    required_warm_ratio=None, required_index_ratio=None,
                    maximum_quality_loss=0.0,
                )
                if missing_side in ("legacy", "observed_zero"):
                    self.assertEqual(result["peak_rss_ratio"], 1.0 if missing_side == "legacy" else 0.0)
                else:
                    self.assertIsNone(result["peak_rss_ratio"])
                self.assertEqual(result["peak_disk_ratio"], 1.0)
                self.assertTrue(result["passed"])

    @unittest.skipUnless(sys.platform == "linux", "Linux resource sampler")
    def test_process_timing_excludes_polling_interval_and_sampler_cleanup(self):
        clock = [10.0]
        process = mock.Mock(pid=123)
        process.poll.side_effect = [None, 0]

        def wait():
            clock[0] = max(clock[0], 10.012)
            return 0

        def delay(seconds):
            clock[0] += seconds

        process.wait.side_effect = wait
        monitor = mock.Mock()
        monitor.join.side_effect = lambda: delay(1.0)
        with (
            mock.patch.object(benchmark.subprocess, "Popen", return_value=process),
            mock.patch.object(benchmark.threading, "Thread", return_value=monitor),
            mock.patch.object(benchmark.time, "perf_counter", side_effect=lambda: clock[0]),
            mock.patch.object(benchmark.time, "sleep", side_effect=delay),
            mock.patch.object(Path, "read_text", side_effect=FileNotFoundError),
        ):
            _, metrics = benchmark.timed(["child"], ROOT, {})
        self.assertAlmostEqual(metrics["wall_ms"], 12.0)

    @unittest.skipUnless(sys.platform == "linux", "Linux resource sampler")
    def test_timing_preserves_child_failure_and_output(self):
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            benchmark.timed(
                [sys.executable, "-c", "import sys; print('out'); print('err', file=sys.stderr); sys.exit(7)"],
                ROOT, {},
            )
        self.assertEqual(caught.exception.returncode, 7)
        self.assertEqual(caught.exception.output.strip(), "out")
        self.assertEqual(caught.exception.stderr.strip(), "err")

    @unittest.skipUnless(sys.platform == "linux", "Linux resource sampler")
    def test_sampler_preserves_counters_for_process_names_with_spaces_and_parentheses(self):
        sampled = threading.Event()
        process = mock.Mock(pid=123)
        fields = ["0"] * 20
        fields[0], fields[11], fields[12] = "S", "120", "80"

        def read(path, *_args, **_kwargs):
            if path.name == "status":
                return "VmRSS: 32 kB\n"
            if path.name == "io":
                return "read_bytes: 3\nwrite_bytes: 4\n"
            sampled.set()
            return "123 (worker ) name) " + " ".join(fields)

        def wait():
            self.assertTrue(sampled.wait(2), "sampler did not run")
            return 0

        process.wait.side_effect = wait
        with (
            mock.patch.object(benchmark.subprocess, "Popen", return_value=process),
            mock.patch.object(benchmark.os, "sysconf", return_value=100),
            mock.patch.object(Path, "read_text", autospec=True, side_effect=read),
        ):
            _, metrics = benchmark.timed(["child"], ROOT, {})
        self.assertEqual(metrics["cpu_ms"], 2000)
        self.assertEqual(metrics["peak_rss_bytes"], 32 * 1024)
        self.assertEqual(metrics["filesystem_read_bytes"], 3)
        self.assertEqual(metrics["filesystem_write_bytes"], 4)
        self.assertGreaterEqual(metrics["resource_samples"], 1)

    @unittest.skipUnless(sys.platform == "linux", "Linux resource sampler")
    def test_sampler_failure_is_reported_to_the_caller(self):
        monitor_done = threading.Event()
        process = mock.Mock(pid=123)
        process.poll.return_value = None
        thread = threading.Thread

        def monitor(*, target, **kwargs):
            def run():
                try:
                    target()
                finally:
                    monitor_done.set()
            return thread(target=run, **kwargs)

        def wait():
            self.assertTrue(monitor_done.wait(2), "persistent sampler failure was not detected")
            return 0

        process.wait.side_effect = wait
        with (
            mock.patch.object(benchmark.subprocess, "Popen", return_value=process),
            mock.patch.object(benchmark.threading, "Thread", side_effect=monitor),
            mock.patch.object(Path, "read_text", side_effect=PermissionError("cannot sample live child")),
        ):
            with self.assertRaisesRegex(RuntimeError, "resource sampler failed") as caught:
                benchmark.timed(["child"], ROOT, {})
        self.assertIsInstance(caught.exception.__cause__, PermissionError)

    @unittest.skipUnless(sys.platform == "linux", "Linux resource sampler")
    def test_sampler_accepts_permission_race_after_child_exit(self):
        sampled = threading.Event()
        process = mock.Mock(pid=123)
        # The kernel can revoke /proc access before waitpid observes the exit.
        process.poll.return_value = None

        def read(path, *_args, **_kwargs):
            if path.name == "status":
                return "VmRSS: 32 kB\n"
            sampled.set()
            raise PermissionError("exiting process io is no longer readable")

        def wait():
            self.assertTrue(sampled.wait(2), "sampler did not run")
            return 0

        process.wait.side_effect = wait
        with (
            mock.patch.object(benchmark.subprocess, "Popen", return_value=process),
            mock.patch.object(Path, "read_text", autospec=True, side_effect=read),
        ):
            result, metrics = benchmark.timed(["child"], ROOT, {})
        self.assertEqual(result.returncode, 0)
        self.assertEqual(metrics["resource_samples"], 0)
        self.assertEqual(metrics["peak_rss_bytes"], 32 * 1024)

    def test_windows_client_authenticates_each_connection(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            token = bytes(range(16)).hex()
            endpoint = home / "daemon.port"
            endpoint.write_bytes(f"43123\r\n{token}\r\n".encode())
            client = benchmark.DaemonClient(home, home / "corpus")
            connection = mock.MagicMock()
            with (
                mock.patch.object(benchmark, "os", SimpleNamespace(name="nt")),
                mock.patch.object(benchmark.socket, "create_connection", return_value=connection) as connect,
            ):
                for _ in range(2):
                    client._connect()
                    client._close()
                self.assertEqual(connect.call_count, 2)
                connect.assert_called_with(("127.0.0.1", 43123), timeout=120)
                self.assertEqual(connection.sendall.call_args_list, [mock.call(token.encode() + b"\n")] * 2)
                for invalid in ("", "z" * 32, "a" * 31):
                    endpoint.write_text(f"43123\n{invalid}\n")
                    connect.reset_mock()
                    with self.assertRaisesRegex(ValueError, "authentication token"):
                        client._connect()
                    connect.assert_not_called()

    def test_cli_neural_query_requires_observed_execution(self):
        result = subprocess.CompletedProcess(
            [], 0, stdout=json.dumps([
                {"file_path": "source.rs", "hits": [{"neural_executed": True}]}
            ])
        )
        with mock.patch.object(benchmark.subprocess, "run", return_value=result) as run:
            measured = benchmark.run_query(
                Path("ig"), Path("corpus"), "query", {}, force_neural=True
            )
        self.assertTrue(measured["neural_executed"])
        self.assertIn("--force-neural", run.call_args.args[0])
        self.assertNotIn("--hash", run.call_args.args[0])
        result.stdout = json.dumps([{"file_path": "source.rs", "hits": [{}]}])
        with mock.patch.object(benchmark.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "did not report neural execution"):
                benchmark.run_query(Path("ig"), Path("corpus"), "query", {}, force_neural=True)
            self.assertFalse(
                benchmark.run_query(Path("ig"), Path("corpus"), "query", {})["neural_executed"]
            )

    def test_daemon_neural_query_requires_observed_execution(self):
        client = benchmark.DaemonClient(Path("home"), Path("corpus"), force_neural=True)
        response = {
            "type": "search_results",
            "hits": [{"file_path": "source.rs", "neural_executed": True}],
        }
        with mock.patch.object(client, "_send", return_value=(response, 1.0)) as send:
            self.assertTrue(client.query("query")["neural_executed"])
        self.assertIs(send.call_args.args[0]["force_neural"], True)
        for observed in (False, None, 1):
            response["hits"][0]["neural_executed"] = observed
            with mock.patch.object(client, "_send", return_value=(response, 1.0)):
                with self.assertRaisesRegex(RuntimeError, "did not report neural execution"):
                    client.query("query")

    def test_neural_suite_propagates_mode_to_every_query_path(self):
        measured = {
            "elapsed_ms": 1.0,
            "hit_count": 1,
            "paths": ["source.rs"],
            "neural_executed": True,
        }
        client = mock.MagicMock()
        client.__enter__.return_value = client
        client.query.return_value = measured

        def query(*_args, **kwargs):
            self.assertIs(kwargs["force_neural"], True)
            return measured

        with (
            mock.patch.object(benchmark, "run_query", side_effect=query),
            mock.patch.object(benchmark, "run_daemon_query", side_effect=query),
            mock.patch.object(benchmark, "DaemonClient", return_value=client) as constructor,
            mock.patch.object(benchmark, "start_daemon", return_value=(None, None, None)),
            mock.patch.object(benchmark, "stop_daemon"),
            mock.patch.object(benchmark, "profile_query_phases", return_value={}) as profile,
        ):
            result = benchmark.query_suite(
                Path("ig"), Path("corpus"), {"IVYGREP_HOME": "home"},
                2, 100, 10, force_neural=True,
            )
        constructor.assert_called_once_with(Path("home"), Path("corpus"), True)
        self.assertIs(profile.call_args.args[-1], True)
        for name, metrics in result.items():
            if name != "phase_timings":
                self.assertEqual(metrics["neural_queries_executed"], metrics["samples"])

    def test_latest_measured_release_snapshot_matches_trial_medians(self):
        snapshot = json.loads(
            (ROOT / "docs/benchmarks/public-million-current.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertRegex(snapshot["binary"]["version"], r"^ivygrep \d+\.\d+\.\d+$")
        measured_version = snapshot["binary"]["version"].removeprefix("ivygrep ")
        self.assertIn(f"{measured_version} release confirmation", snapshot["description"])
        self.assertEqual(snapshot["scope"], "synthetic hash-only scale and footprint measurement")
        self.assertIn("synthetic", snapshot["description"])
        self.assertIn("hash-only", snapshot["description"])
        self.assertEqual(snapshot["corpus"]["license"], "CC0-1.0")
        self.assertEqual(snapshot["harness"]["trials"], len(snapshot["trials"]))
        self.assertGreaterEqual(len(snapshot["trials"]), 3)
        for metric in (
            "index_wall_ms",
            "chunks_per_second",
            "index_size_bytes",
            "peak_rss_bytes",
            "warm_cli_p95_ms",
            "warm_engine_p95_ms",
            "process_cold_p95_ms",
            "concurrent_queries_per_second",
        ):
            self.assertEqual(
                snapshot["median"][metric],
                statistics.median(trial[metric] for trial in snapshot["trials"]),
            )

    def test_generated_corpus_is_its_own_git_workspace(self):
        with tempfile.TemporaryDirectory() as temp:
            outer = Path(temp)
            subprocess.run(["git", "init", "-q"], cwd=outer, check=True)
            corpus = outer / "corpus"

            benchmark.generate_corpus(corpus, files=1, chunks_per_file=1)

            top_level = subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                cwd=corpus,
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            ).stdout.strip()
            self.assertEqual(Path(top_level), corpus)

    def test_start_daemon_creates_fresh_home_before_opening_log(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp) / "fresh-home"

            class FakeProcess:
                def poll(self):
                    return None

            def fake_popen(*_args, **_kwargs):
                benchmark.daemon_endpoint(home).touch()
                return FakeProcess()

            with mock.patch.object(benchmark.subprocess, "Popen", fake_popen):
                process, log, log_path = benchmark.start_daemon(
                    Path("/tmp/ig"),
                    Path(temp),
                    {},
                    home,
                )

            self.assertIsInstance(process, FakeProcess)
            self.assertEqual(log_path, home / "million-benchmark-daemon.log")
            self.assertTrue(log_path.exists())
            log.close()

    def test_daemon_retry_excludes_stale_connection_from_latency(self):
        response = json.dumps({"type": "search_results", "hits": []}).encode() + b"\n"
        connections = []
        payloads = []
        responses = [[response, b""], [response]]

        class FakeReader:
            def __init__(self, values):
                self.values = values

            def readline(self):
                return self.values.pop(0)

            def close(self):
                pass

        class FakeConnection:
            def __init__(self, values):
                self.closed = False
                self.reader = FakeReader(values)

            def sendall(self, payload):
                payloads.append(payload)

            def makefile(self, _mode):
                return self.reader

            def close(self):
                self.closed = True

        client = benchmark.DaemonClient(Path("/tmp/home"), Path("/tmp/corpus"))

        def connect():
            connection = FakeConnection(responses[len(connections)])
            connections.append(connection)
            client.connection = connection
            client.reader = connection.makefile("rb")

        client._connect = connect
        clock = iter((0.0, 0.001, 10.0, 20.0, 20.002))
        with mock.patch.object(benchmark.time, "perf_counter", lambda: next(clock)):
            with client:
                client.query("first")
                result = client.query("second")

        self.assertEqual(len(connections), 2)
        self.assertTrue(all(connection.closed for connection in connections))
        self.assertAlmostEqual(result["elapsed_ms"], 2.0)
        self.assertTrue(
            all(
                json.loads(payload.decode())["protocol_version"]
                == benchmark.DAEMON_PROTOCOL_VERSION
                for payload in payloads
            )
        )

    def test_daemon_client_uses_current_protocol_version(self):
        response = json.dumps({"type": "search_results", "hits": []}).encode() + b"\n"
        payloads = []

        class FakeReader:
            def readline(self):
                return response

            def close(self):
                pass

        class FakeConnection:
            def sendall(self, payload):
                payloads.append(json.loads(payload))

            def makefile(self, _mode):
                return FakeReader()

            def close(self):
                pass

        client = benchmark.DaemonClient(Path("/tmp/home"), Path("/tmp/corpus"))
        client.connection = FakeConnection()
        client.reader = client.connection.makefile("rb")
        with mock.patch.object(benchmark.time, "perf_counter", side_effect=[0.0, 0.001]):
            client.query("first")

        self.assertEqual(
            payloads[0]["protocol_version"], benchmark.DAEMON_PROTOCOL_VERSION
        )

    def test_daemon_client_negotiates_an_older_protocol_version(self):
        responses = [
            json.dumps(
                {
                    "type": "error",
                    "message": "unsupported daemon protocol version 3; expected 2",
                }
            ).encode()
            + b"\n",
            json.dumps({"type": "search_results", "hits": []}).encode() + b"\n",
        ]
        payloads = []

        class FakeReader:
            def readline(self):
                return responses.pop(0)

            def close(self):
                pass

        class FakeConnection:
            def sendall(self, payload):
                payloads.append(json.loads(payload))

            def makefile(self, _mode):
                return FakeReader()

            def close(self):
                pass

        client = benchmark.DaemonClient(Path("/tmp/home"), Path("/tmp/corpus"))
        client.connection = FakeConnection()
        client.reader = client.connection.makefile("rb")
        with mock.patch.object(
            benchmark.time,
            "perf_counter",
            side_effect=[0.0, 0.001, 10.0, 10.002],
        ):
            result = client.query("first")

        self.assertEqual(
            [payload["protocol_version"] for payload in payloads],
            [benchmark.DAEMON_PROTOCOL_VERSION, 2],
        )
        self.assertEqual(client.protocol_version, 2)
        self.assertAlmostEqual(result["elapsed_ms"], 2.0)

    def test_dataset_provenance_ignores_unrelated_manifest_changes(self):
        matrix = {
            "results": [
                {
                    "dataset": "public-task",
                    "dataset_provenance": {
                        "revision": "abc",
                        "checksums": {"corpus": "123"},
                    },
                },
                {
                    "dataset": "public-task",
                    "dataset_provenance": {
                        "revision": "abc",
                        "checksums": {"corpus": "123"},
                    },
                },
            ]
        }
        self.assertEqual(
            renderer.dataset_provenances(matrix),
            {
                "public-task": {
                    "revision": "abc",
                    "checksums": {"corpus": "123"},
                }
            },
        )

    def test_generated_query_cases_map_to_the_expected_file(self):
        cases = benchmark.query_cases(2, 1_000_000, 100)
        self.assertEqual(
            cases[0],
            (
                "calculate invoice tax for regional order 0",
                "shard_00/module_00000.rs",
            ),
        )
        self.assertEqual(cases[1][1], "shard_00/module_00099.rs")

    def test_comparison_accepts_two_x_speedups_without_quality_loss(self):
        result = comparator.compare(
            artifact([100.0] * 40, throughput=100.0, commit="base"),
            artifact([40.0] * 40, throughput=220.0, commit="head"),
            significant_regression_ratio=1.15,
            required_warm_ratio=0.5,
            required_index_ratio=2.0,
            maximum_quality_loss=0.0,
        )
        self.assertTrue(result["passed"])
        self.assertEqual(result["failures"], [])

    def test_comparison_rejects_significant_latency_regression(self):
        result = comparator.compare(
            artifact([100.0] * 40),
            artifact([130.0] * 40),
            significant_regression_ratio=1.15,
            required_warm_ratio=None,
            required_index_ratio=None,
            maximum_quality_loss=0.0,
        )
        self.assertFalse(result["passed"])
        self.assertTrue(result["warm_distinct_p95_ratio"]["significant_regression"])

    def test_comparison_rejects_filtered_path_regression_when_warm_path_is_stable(self):
        baseline = artifact([100.0] * 40)
        current = artifact([100.0] * 40)
        baseline["queries"]["filtered"] = {
            "latency_samples_ms": [10.0] * 40,
            "expected_recall_at_20": 1.0,
        }
        current["queries"]["filtered"] = {
            "latency_samples_ms": [15.0] * 40,
            "expected_recall_at_20": 1.0,
        }

        result = comparator.compare(
            baseline,
            current,
            significant_regression_ratio=1.15,
            required_warm_ratio=None,
            required_index_ratio=None,
            maximum_quality_loss=0.0,
        )

        self.assertFalse(result["passed"])
        self.assertFalse(
            result["query_path_p95_ratios"]["warm_distinct"][
                "significant_regression"
            ]
        )
        self.assertTrue(
            result["query_path_p95_ratios"]["filtered"][
                "significant_regression"
            ]
        )

    def test_comparison_rejects_significant_index_regression_across_runs(self):
        baselines = [
            artifact([100.0] * 40, throughput=value, commit="base")
            for value in (100.0, 102.0, 98.0)
        ]
        currents = [
            artifact([100.0] * 40, throughput=value, commit="head")
            for value in (70.0, 72.0, 68.0)
        ]
        result = comparator.compare_runs(
            baselines,
            currents,
            significant_regression_ratio=1.15,
            required_warm_ratio=None,
            required_index_ratio=None,
            maximum_quality_loss=0.0,
        )
        self.assertFalse(result["passed"])
        self.assertTrue(result["index_throughput_ratio"]["significant_regression"])

    def test_comparison_rejects_index_size_regression(self):
        result = comparator.compare(
            artifact([100.0] * 40, size_bytes=1000),
            artifact([100.0] * 40, size_bytes=1100),
            significant_regression_ratio=1.15,
            required_warm_ratio=None,
            required_index_ratio=None,
            maximum_quality_loss=0.0,
        )
        self.assertFalse(result["passed"])
        self.assertEqual(result["index_size_ratio"], 1.1)
        self.assertTrue(
            any("index size ratio" in failure for failure in result["failures"])
        )


if __name__ == "__main__":
    unittest.main()
