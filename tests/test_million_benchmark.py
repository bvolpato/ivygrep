import importlib.util
import json
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

        self.assertEqual(payloads[0]["protocol_version"], 2)

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
