"""Versioned public evaluation identities, independent of retrieval execution."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path


EXECUTION_SCHEMA_VERSION = 1
CAPTURE_PREFIX = "IVYGREP_RERANKER_CAPTURE\t"
CAPTURE_STAGE = "pre-learned-accepted-files"
# Rust str::trim uses Unicode White_Space. Python's default strip additionally
# removes U+001C..U+001F, which must remain part of the requested query here.
NATIVE_QUERY_WHITESPACE = (
    "\t\n\v\f\r \u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005"
    "\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000"
)
RERANK_FEATURE_SCHEMA = (
    "log_total_score",
    "reciprocal_rank",
    "hit_count",
    "source_count",
    "source_lexical",
    "source_semantic",
    "source_literal",
    "source_path",
    "source_symbol",
    "query_preview_coverage",
    "query_path_coverage",
    "exact_query_preview",
    "exact_query_path",
    "support_path",
    "primary_source",
    "shallow_path",
    "query_length",
    "preview_length",
    "lexical_semantic",
    "literal_exact",
    "semantic_only",
    "score_preview_coverage",
    "rank_preview_coverage",
    "short_preview_coverage",
    "medium_preview_coverage",
    "long_preview_coverage",
    "short_semantic",
    "long_semantic",
    "short_literal",
    "long_literal",
    "preview_term_precision",
    "preview_term_f1",
    "weighted_preview_coverage",
    "informative_preview_coverage",
    "long_term_preview_coverage",
    "numeric_preview_coverage",
    "query_bigram_preview_coverage",
    "query_line_preview_coverage",
    "path_term_f1",
    "natural_language_preview_f1",
    "code_query_line_coverage",
)


def validate_native_capture(record: dict, query: str, process_id: int) -> None:
    expected_keys = {
        "schema_version",
        "stage",
        "status",
        "reason",
        "query",
        "model_id",
        "ranking_context_lines",
        "feature_schema",
        "candidates",
        "process_id",
    }
    if not isinstance(record, dict) or set(record) != expected_keys:
        raise ValueError("native capture has missing or unsupported fields")
    if (
        type(record["schema_version"]) is not int
        or record["schema_version"] != 1
        or record["stage"] != CAPTURE_STAGE
    ):
        raise ValueError("unsupported native capture schema/stage")
    if (
        type(process_id) is not int
        or type(record["process_id"]) is not int
        or record["process_id"] != process_id
        or process_id <= 0
    ):
        raise ValueError("native capture is not from the fresh local query process")
    if record["query"] != query.strip(NATIVE_QUERY_WHITESPACE):
        raise ValueError("native capture query does not match the requested query")
    if (
        type(record["ranking_context_lines"]) is not int
        or record["ranking_context_lines"] != 2
    ):
        raise ValueError("native capture does not use canonical C2 ranking evidence")
    if record["feature_schema"] != list(RERANK_FEATURE_SCHEMA):
        raise ValueError("native capture feature schema differs from training")
    if record["model_id"] is not None and (
        not isinstance(record["model_id"], str) or not record["model_id"]
    ):
        raise ValueError("native capture has an invalid model identity")
    candidates = record["candidates"]
    if not isinstance(candidates, list):
        raise ValueError("native capture candidates must be an array")
    if record["status"] == "skipped":
        if (
            record["reason"]
            not in {
                "route-not-learned",
                "deterministic-mode",
                "fewer-than-five-files",
                "model-unavailable",
            }
            or candidates
        ):
            raise ValueError("invalid skipped native capture")
        return
    if (
        record["status"] != "applied"
        or record["reason"] is not None
        or not record["model_id"]
        or len(candidates) < 5
    ):
        raise ValueError("invalid applied native capture")
    paths = set()
    for rank, candidate in enumerate(candidates):
        keys = {
            "file_path",
            "total_score",
            "hit_count",
            "sources",
            "canonical_preview",
            "baseline_rank",
            "native_features",
        }
        if not isinstance(candidate, dict) or set(candidate) != keys:
            raise ValueError("native candidate has missing or unsupported fields")
        path = candidate["file_path"]
        if not isinstance(path, str) or not path or path in paths:
            raise ValueError("native capture candidate paths must be unique strings")
        paths.add(path)
        if (
            type(candidate["baseline_rank"]) is not int
            or candidate["baseline_rank"] != rank
        ):
            raise ValueError(
                "native capture candidate ranks are incomplete or reordered"
            )
        if type(candidate["hit_count"]) is not int or candidate["hit_count"] < 1:
            raise ValueError("native capture hit count is invalid")
        sources = candidate["sources"]
        if (
            not isinstance(sources, list)
            or not all(isinstance(source, str) for source in sources)
            or "backfill" in sources
        ):
            raise ValueError(
                "native capture must contain the accepted pre-backfill pool"
            )
        preview = candidate["canonical_preview"]
        if not isinstance(preview, str) or len(preview.encode("utf-8")) > 12000:
            raise ValueError(
                "native capture canonical preview exceeds its byte contract"
            )
        features = candidate["native_features"]
        if not isinstance(features, list) or len(features) != len(
            RERANK_FEATURE_SCHEMA
        ):
            raise ValueError("native capture feature vector length is invalid")
        if any(
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
            for value in [candidate["total_score"], *features]
        ):
            raise ValueError(
                "native capture contains nonfinite or nonnumeric features/scores"
            )


def parse_native_capture(stderr: str, query: str, process_id: int) -> dict:
    records = [
        line[len(CAPTURE_PREFIX) :]
        for line in stderr.split("\n")
        if line.startswith(CAPTURE_PREFIX)
    ]
    if len(records) != 1:
        raise ValueError(
            f"expected exactly one fresh native capture record, found {len(records)}"
        )
    try:
        record = json.loads(records[0])
    except (TypeError, json.JSONDecodeError) as error:
        raise ValueError("native capture JSON is incomplete or invalid") from error
    validate_native_capture(record, query, process_id)
    return record


EVALUATION_DEFAULTS = {
    "limit": 20,
    "query_id": [],
    "max_query_chars": None,
    "query_expansion": "none",
    "query_expansion_workers": 4,
    "probe_limit": None,
    "probe_query_chars": None,
    "rrf_k": 60.0,
    "original_weight": 1.0,
    "disable_memory_expansion": False,
    "capture_reranker": False,
    "context_lines": 2,
}
INTEGER_ENVIRONMENT = (
    "IVYGREP_NEURAL_THREADS",
    "IVYGREP_NEURAL_MEMORY_MB",
    "IVYGREP_NEURAL_ACCELERATOR_HANDLES",
    "IVYGREP_RERANK_LIMIT",
    "IVYGREP_INDEX_THREADS",
    "IVYGREP_NEURAL_BATCH_SIZE",
    "IVYGREP_SEARCH_DEADLINE_SECS",
    "RAYON_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "MKL_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
    "BLIS_NUM_THREADS",
)
FLAG_ENVIRONMENT = (
    "IVYGREP_ENHANCE_ON_BATTERY",
    "TOKENIZERS_PARALLELISM",
    "HF_HUB_OFFLINE",
    "TRANSFORMERS_OFFLINE",
    "CI",
)
# Explicit non-credential settings whose values may contain private paths or
# hardware selectors. Only their digests are publishable, never the raw values.
DIGEST_ENVIRONMENT = (
    "HF_HOME",
    "HF_HUB_CACHE",
    "XDG_CACHE_HOME",
    "CUDA_VISIBLE_DEVICES",
    "RUST_LOG",
)
PROFILE_ALIASES = {
    "": "static-retrieval-v1",
    "static": "static-retrieval-v1",
    "portable": "static-retrieval-v1",
    "static-retrieval": "static-retrieval-v1",
    "static-retrieval-v1": "static-retrieval-v1",
    "potion": "potion-code-16m-v1",
    "potion-code": "potion-code-16m-v1",
    "potion-code-16m": "potion-code-16m-v1",
    "potion-code-16m-v1": "potion-code-16m-v1",
    "model2vec-code": "potion-code-16m-v1",
    "general": "general",
    "minilm": "general",
    "all-minilm-l6-v2": "general",
    "code": "code-minilm-l6-v1",
    "codesearchnet": "code-minilm-l6-v1",
    "code-minilm-l6-v1": "code-minilm-l6-v1",
    "code-hq": "code-minilm-l12-v1",
    "code-high-quality": "code-minilm-l12-v1",
    "code-minilm-l12-v1": "code-minilm-l12-v1",
}


def canonical_sha256(value: object) -> str:
    return hashlib.sha256(
        json.dumps(
            value, sort_keys=True, separators=(",", ":"), allow_nan=False
        ).encode()
    ).hexdigest()


def safe_environment(
    environment: dict[str, str], *, capture_reranker: bool = False
) -> dict:
    """Read only an explicit non-credential whitelist; never dump environment."""
    profile = environment.get("IVYGREP_MODEL_PROFILE", "").lower()
    if profile not in PROFILE_ALIASES:
        raise ValueError("unsupported IVYGREP_MODEL_PROFILE in benchmark configuration")
    reranker = environment.get("IVYGREP_RERANKER", "").strip().lower()
    if reranker not in {"", "auto", "learned", "deterministic", "disabled", "off"}:
        raise ValueError("unsupported IVYGREP_RERANKER in benchmark configuration")
    values = {
        "IVYGREP_MODEL_PROFILE": PROFILE_ALIASES[profile],
        "IVYGREP_RERANKER": "deterministic"
        if reranker in {"deterministic", "disabled", "off"}
        else "learned",
        "IVYGREP_ENHANCE_MAX_LOAD_RATIO": 0.0,
        "IVYGREP_RERANKER_CAPTURE": capture_reranker,
        "IVYGREP_DISABLE_QUERY_CACHE": "IVYGREP_DISABLE_QUERY_CACHE" in environment,
        "IVYGREP_NEURAL_FOREGROUND_ACCELERATOR": (
            "cpu"
            if environment.get("IVYGREP_NEURAL_FOREGROUND_ACCELERATOR", "")
            .strip()
            .lower()
            in {"0", "false", "no", "off", "cpu"}
            else "auto"
        ),
    }
    for name in INTEGER_ENVIRONMENT:
        raw = environment.get(name)
        if raw in (None, ""):
            values[name] = None
            continue
        if not raw.isascii() or not raw.isdecimal():
            raise ValueError(f"invalid numeric benchmark setting: {name}")
        values[name] = int(raw)
    for name in FLAG_ENVIRONMENT:
        raw = environment.get(name)
        if raw is not None and raw.strip().lower() not in {
            "",
            "0",
            "1",
            "true",
            "false",
            "yes",
            "no",
            "on",
            "off",
        }:
            raise ValueError(f"invalid benchmark flag: {name}")
        values[name] = raw
    omp = environment.get("OMP_NUM_THREADS")
    if omp is not None and not all(
        part.strip().isascii() and part.strip().isdecimal() for part in omp.split(",")
    ):
        raise ValueError("invalid numeric benchmark setting: OMP_NUM_THREADS")
    values["OMP_NUM_THREADS"] = omp
    digests = {
        name: hashlib.sha256(environment[name].encode()).hexdigest()
        if name in environment
        else None
        for name in DIGEST_ENVIRONMENT
    }
    return {"settings": values, "setting_value_sha256": digests}


def dataset_fingerprint(dataset: Path) -> dict:
    actual = {
        name: sha256_file(dataset / name)
        for name in ("corpus.jsonl", "queries.jsonl", "qrels.tsv")
    }
    provenance_path = dataset / "provenance.json"
    if provenance_path.exists():
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        for name, expected in provenance.get("checksums", {}).items():
            if name in actual and actual[name] != expected:
                raise ValueError(
                    f"dataset content checksum differs from provenance: {dataset.name}/{name}"
                )
    return {
        "files": actual,
        "provenance_sha256": sha256_file(provenance_path)
        if provenance_path.exists()
        else None,
    }


def execution_harness(root: Path) -> dict[str, str]:
    return {
        name: sha256_file(root / "scripts" / name)
        for name in (
            "eval_code_retrieval.py",
            "export_public_retrieval.py",
            "public_retrieval_contracts.py",
            "run_public_benchmark_matrix.py",
        )
    }


def execution_request(
    dataset: Path,
    binary_sha256: str,
    mode: str,
    options: dict,
    environment: dict[str, str],
    runtime: dict,
    harness: dict,
    *,
    dataset_content: dict | None = None,
) -> dict:
    if set(options) - set(EVALUATION_DEFAULTS):
        raise ValueError("unsupported evaluation configuration fields")
    settings = {**EVALUATION_DEFAULTS, **options}
    for name in ("limit", "query_expansion_workers"):
        if type(settings[name]) is not int or settings[name] < 1:
            raise ValueError(f"invalid positive evaluation setting: {name}")
    for name in ("max_query_chars", "probe_limit", "probe_query_chars"):
        if settings[name] is not None and (
            type(settings[name]) is not int or settings[name] < 1
        ):
            raise ValueError(f"invalid positive evaluation setting: {name}")
    for name in ("rrf_k", "original_weight"):
        value = settings[name]
        if (
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
            or value <= 0
        ):
            raise ValueError(f"invalid finite evaluation setting: {name}")
    if (
        type(settings["capture_reranker"]) is not bool
        or type(settings["disable_memory_expansion"]) is not bool
        or settings["context_lines"] != 2
    ):
        raise ValueError("unsupported evaluation capture/context configuration")
    if not isinstance(settings["query_id"], list) or not all(
        isinstance(value, str) for value in settings["query_id"]
    ):
        raise ValueError("query IDs must be an explicit string list")
    settings["query_id"] = list(settings["query_id"])
    settings["probe_limit"] = settings["probe_limit"] or settings["limit"]
    if settings["probe_limit"] < settings["limit"]:
        raise ValueError("probe limit is below the original query limit")
    request = {
        "schema_version": EXECUTION_SCHEMA_VERSION,
        "dataset": dataset.name,
        "dataset_content": dataset_content
        if dataset_content is not None
        else dataset_fingerprint(dataset),
        "binary_sha256": binary_sha256,
        "mode": mode,
        "options": settings,
        "environment": safe_environment(
            environment, capture_reranker=settings["capture_reranker"]
        ),
        "runtime": runtime,
        "harness_sha256": harness,
    }
    canonical_sha256(
        request
    )  # Reject non-serializable/nonfinite inputs before execution.
    return request


def observed_configuration(index_configuration: dict) -> dict:
    return {
        key: index_configuration.get(key)
        for key in (
            "neural_profile",
            "neural_model",
            "neural_backend",
            "reranker_mode",
            "reranker_model",
            "reranker_candidate_limit",
        )
    }


def validate_observed_configuration(request: dict, observed: dict) -> None:
    settings = request["environment"]["settings"]
    if observed.get("reranker_mode") != settings["IVYGREP_RERANKER"]:
        raise ValueError(
            "observed reranker mode differs from requested benchmark configuration"
        )
    if (
        request["mode"] in {"blended", "neural"}
        and observed.get("neural_profile") != settings["IVYGREP_MODEL_PROFILE"]
    ):
        raise ValueError(
            "observed neural profile differs from requested benchmark configuration"
        )


def validate_execution(result: dict, expected_request: dict) -> None:
    execution = result.get("execution_provenance")
    if (
        not isinstance(execution, dict)
        or execution.get("schema_version") != EXECUTION_SCHEMA_VERSION
    ):
        raise ValueError(
            "legacy result lacks a supported execution fingerprint; rerun instead of relabeling it"
        )
    if (
        not isinstance(execution.get("source_commit"), str)
        or not execution["source_commit"]
        or not execution.get("executed_at")
    ):
        raise ValueError("reused execution is missing original source/time provenance")
    request = execution.get("request")
    if not isinstance(request, dict) or canonical_sha256(request) != execution.get(
        "request_sha256"
    ):
        raise ValueError("reused execution fingerprint is missing or corrupt")
    if request != expected_request:
        changed = sorted(
            key
            for key in set(request) | set(expected_request)
            if request.get(key) != expected_request.get(key)
        )
        raise ValueError(
            "reused execution configuration differs: " + ", ".join(changed)
        )
    if (
        result.get("binary", {}).get("sha256") != request["binary_sha256"]
        or result.get("mode") != request["mode"]
    ):
        raise ValueError("result identity conflicts with its execution fingerprint")
    fields = {
        "query_text_limit": "max_query_chars",
        "query_expansion": "query_expansion",
        "query_expansion_workers": "query_expansion_workers",
        "probe_limit": "probe_limit",
        "probe_query_chars": "probe_query_chars",
        "rrf_k": "rrf_k",
        "original_weight": "original_weight",
        "memory_expansion_disabled": "disable_memory_expansion",
    }
    if any(
        name not in result or result[name] != request["options"][option]
        for name, option in fields.items()
    ):
        raise ValueError(
            "result query configuration conflicts with its execution fingerprint"
        )
    capture = request["options"]["capture_reranker"]
    expected_scope = "native-training-capture" if capture else "retrieval-benchmark"
    expected_path = (
        "local-process-native-capture"
        if capture
        else ("local-process" if request["mode"] == "lexical" else "daemon")
    )
    if (
        result.get("measurement_scope") != expected_scope
        or result.get("warm_query_path") != expected_path
    ):
        raise ValueError(
            "result measurement path conflicts with its execution fingerprint"
        )
    if result.get("runtime") != request["runtime"]:
        raise ValueError(
            "result runtime metadata conflicts with its original execution"
        )
    observed = observed_configuration(result.get("index_configuration", {}))
    if execution.get("observed_configuration") != observed:
        raise ValueError(
            "result observed configuration conflicts with execution provenance"
        )
    validate_observed_configuration(request, observed)


def execution_summary(results: list[dict]) -> dict:
    if not results:
        raise ValueError("cannot summarize execution provenance without results")
    executions = [result["execution_provenance"] for result in results]
    commits = sorted({item["source_commit"] for item in executions})
    harnesses = {
        canonical_sha256(item["request"]["harness_sha256"]): item["request"][
            "harness_sha256"
        ]
        for item in executions
    }
    runtimes = {
        canonical_sha256(item["request"]["runtime"]): item["request"]["runtime"]
        for item in executions
    }
    return {
        "ivygrep_commit": commits[0] if len(commits) == 1 else "mixed",
        "execution_source_commits": commits,
        "harness_sha256": next(iter(harnesses.values())) if len(harnesses) == 1 else {},
        "execution_harnesses": list(harnesses.values()),
        "runtime": next(iter(runtimes.values()))
        if len(runtimes) == 1
        else {"mixed": True},
    }


def sha256_file(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def pretty_json_sha256(value: object) -> str:
    encoded = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    return hashlib.sha256(encoded).hexdigest()


def load_fit_ledger(model_path: Path, ledger_path: Path, expected_sha256: str) -> dict:
    if sha256_file(ledger_path) != expected_sha256:
        raise ValueError("model-fit ledger checksum changed")
    model = json.loads(model_path.read_text(encoding="utf-8"))
    ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    if ledger.get("schema_version") != 1:
        raise ValueError("unsupported model-fit ledger schema")
    if (
        ledger.get("model_sha256") != sha256_file(model_path)
        or ledger.get("model_id") != model["model_id"]
    ):
        raise ValueError(
            "model-fit ledger is not bound to the checkout-reference model"
        )
    expected = {source["dataset"]: source for source in model["training"]["sources"]}
    sources = ledger.get("sources", [])
    if len(sources) != len(expected) or {
        source["dataset"] for source in sources
    } != set(expected):
        raise ValueError(
            "model-fit ledger does not cover every recorded training source"
        )
    total = 0
    for source in sources:
        recorded = expected[source["dataset"]]
        ids = source.get("query_ids", [])
        if (
            not all(isinstance(value, str) for value in ids)
            or ids != sorted(set(ids))
            or len(ids) != recorded["queries"]
        ):
            raise ValueError(
                "model-fit query IDs must be complete, unique, sorted strings"
            )
        if pretty_json_sha256(ids) != source.get("query_ids_sha256"):
            raise ValueError("model-fit query-ID checksum changed")
        if "fit_query_ids" in recorded and ids != recorded["fit_query_ids"]:
            raise ValueError(
                "model-fit query IDs differ from the native training record"
            )
        provenance_sha = pretty_json_sha256(source.get("provenance"))
        expected_canonical = recorded.get(
            "dataset_provenance_canonical_sha256", recorded["dataset_provenance_sha256"]
        )
        if (
            provenance_sha != expected_canonical
            or source.get("dataset_provenance_sha256")
            != recorded["dataset_provenance_sha256"]
            or source.get("result_sha256") != recorded["result_sha256"]
        ):
            raise ValueError(
                "model-fit source provenance does not match recorded training"
            )
        total += len(ids)
    if total != ledger.get("queries") or total != model["training"]["queries"]:
        raise ValueError("model-fit ledger total differs from recorded training")
    return ledger


def audit_fit_queries(ledger: dict, datasets: list[Path], role: str) -> dict:
    # Repository-qualified IDs avoid treating unrelated datasets' numeric IDs as
    # the same query. Revision changes do not erase a known overlap.
    fit_ids: dict[str, set[str]] = {}
    for source in ledger["sources"]:
        repository = source["provenance"]["query_corpus"]["repository"]
        fit_ids.setdefault(repository, set()).update(source["query_ids"])
    records = []
    for dataset in datasets:
        provenance = json.loads(
            (dataset / "provenance.json").read_text(encoding="utf-8")
        )
        repository = (provenance.get("query_corpus") or {}).get("repository")
        if not isinstance(repository, str) or not repository:
            raise ValueError(
                f"{dataset.name}: query repository provenance is required for a fit-ID audit"
            )
        queries = [
            json.loads(line)
            for line in (dataset / "queries.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        ids = [str(query["_id"]) for query in queries]
        if len(ids) != len(set(ids)):
            raise ValueError(
                f"{dataset.name}: duplicate query IDs cannot certify fit disjointness"
            )
        overlap = sorted(set(ids) & fit_ids.get(repository, set()))
        records.append(
            {
                "dataset": dataset.name,
                "query_repository": repository,
                "queries": len(ids),
                "overlap_queries": len(overlap),
                "overlap_query_ids": overlap,
            }
        )
    total_overlap = sum(row["overlap_queries"] for row in records)
    if role == "fit-disjoint-diagnostic" and total_overlap:
        raise ValueError(
            f"declared fit-disjoint diagnostic overlaps {total_overlap} actual checkout-reference model-fit query IDs"
        )
    return {
        "schema_version": 2,
        "query_role": role,
        "reference": {
            "scope": "checkout-model",
            "verified": True,
            "model_id": ledger["model_id"],
            "model_sha256": ledger["model_sha256"],
        },
        "executed_binary": {
            "applicability": "unverified",
            "reason": "No executed-model checksum attestation binds the binary to the checkout-reference model.",
        },
        "queries": sum(row["queries"] for row in records),
        "overlap_queries": total_overlap,
        "datasets": records,
        "scope": "repository-qualified query-ID overlap against the checkout-reference model, not proof of executed-model disjointness, overfitting or semantic independence",
    }


def audit_public_profile(
    manifest: dict, profile: str, datasets: list[Path], manifest_path: Path
) -> dict:
    role = manifest["profiles"][profile].get("query_role", "public-diagnostic")
    config = manifest.get("reranker_fit_ledger")
    if not config:
        if role == "fit-disjoint-diagnostic":
            raise ValueError(
                "declared fit-disjoint diagnostic requires a model-bound fit ledger"
            )
        return {
            "schema_version": 2,
            "query_role": role,
            "reference": {"scope": "checkout-model", "verified": False},
            "executed_binary": {
                "applicability": "unverified",
                "reason": "No executed-model checksum attestation binds the binary to a checkout-reference model.",
            },
        }
    base = manifest_path.resolve().parent
    model_path = base / config["model"]
    ledger_path = base / config["path"]
    ledger = load_fit_ledger(model_path, ledger_path, config["sha256"])
    audit = audit_fit_queries(ledger, datasets, role)
    audit["reference"]["ledger_sha256"] = config["sha256"]
    return audit


def validate_public_selection(
    manifest: dict, profile: str, dataset: Path, provenance: dict
) -> None:
    config = manifest["tasks"][dataset.name]
    for field, revision_key in (
        ("query_corpus", "query_corpus_revision"),
        ("qrels", "qrels_revision"),
    ):
        if (
            revision_key in config
            and (provenance.get(field) or {}).get("revision") != config[revision_key]
        ):
            raise ValueError(
                f"{dataset.name}: exported {field} revision differs from requested manifest"
            )
    options = (
        manifest["profiles"][profile].get("task_options", {}).get(dataset.name, {})
    )
    expected = {
        "query_limit": options.get("sample_queries"),
        "corpus_limit": options.get("sample_corpus"),
        "seed": options.get("seed", 20260615)
        if options.get("sample_queries") is not None
        else None,
        "query_partition": options.get("query_partition"),
    }
    actual = provenance.get("sample") or {}
    if any(actual.get(key) != value for key, value in expected.items()):
        raise ValueError(
            f"{dataset.name}: exported sample/partition differs from requested profile"
        )
    with (dataset / "queries.jsonl").open(encoding="utf-8") as handle:
        query_count = sum(bool(line.strip()) for line in handle)
    if provenance.get("counts", {}).get("queries") != query_count:
        raise ValueError(
            f"{dataset.name}: exported query count differs from actual query bytes"
        )
