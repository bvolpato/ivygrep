#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "datasets>=3,<5",
# ]
# ///
"""Export pinned public CoIR datasets into ivygrep's retrieval layout."""

from __future__ import annotations

import argparse
import hashlib
import heapq
import json
from pathlib import Path
import random
import shutil
from typing import Iterable


LANGUAGE_EXTENSIONS = {
    "c": "c",
    "c#": "cs",
    "c++": "cpp",
    "cpp": "cpp",
    "go": "go",
    "java": "java",
    "javascript": "js",
    "js": "js",
    "php": "php",
    "python": "py",
    "ruby": "rb",
    "rust": "rs",
    "sql": "sql",
    "typescript": "ts",
}


def load_manifest(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def selected_tasks(manifest: dict, profile: str | None, tasks: list[str]) -> list[str]:
    if tasks:
        selected = tasks
    elif profile:
        try:
            selected = manifest["profiles"][profile]["tasks"]
        except KeyError as error:
            raise ValueError(f"unknown benchmark profile: {profile}") from error
    else:
        raise ValueError("provide --profile or at least one --task")

    unknown = sorted(set(selected) - set(manifest["tasks"]))
    if unknown:
        raise ValueError(f"unknown benchmark tasks: {', '.join(unknown)}")
    return list(dict.fromkeys(selected))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_extension(language: object) -> str:
    normalized = str(language or "").strip().lower()
    return LANGUAGE_EXTENSIONS.get(normalized, "txt")


def sampled_query_ids(
    qrels: Iterable[dict],
    sample_queries: int | None,
    seed: int,
    query_partition: dict | None = None,
) -> set[str]:
    query_ids = sorted({str(row["query_id"]) for row in qrels})
    if query_partition:
        modulus = int(query_partition["modulus"])
        residues = {int(value) for value in query_partition["residues"]}
        if modulus < 2:
            raise ValueError("query partition modulus must be at least 2")
        if not residues or any(value < 0 or value >= modulus for value in residues):
            raise ValueError("query partition residues must be within the modulus")
        query_ids = [
            query_id
            for query_id in query_ids
            if int.from_bytes(
                hashlib.sha256(query_id.encode()).digest()[:8],
                "big",
            )
            % modulus
            in residues
        ]
    if sample_queries is None or sample_queries >= len(query_ids):
        return set(query_ids)
    if sample_queries < 1:
        raise ValueError("--sample-queries must be positive")
    generator = random.Random(seed)
    return set(generator.sample(query_ids, sample_queries))


def sampled_corpus_indices(
    corpus,
    required_ids: set[str],
    sample_corpus: int | None,
    seed: int,
) -> list[int]:
    if sample_corpus is None or sample_corpus >= len(corpus):
        return list(range(len(corpus)))
    if sample_corpus < len(required_ids):
        raise ValueError(
            f"--sample-corpus={sample_corpus} is smaller than "
            f"{len(required_ids)} required qrel documents"
        )

    required_indices = []
    distractor_limit = sample_corpus - len(required_ids)
    distractors: list[tuple[int, int]] = []
    for index, row in enumerate(corpus):
        document_id = str(row["_id"])
        if document_id in required_ids:
            required_indices.append(index)
            continue
        if distractor_limit == 0:
            continue
        digest = hashlib.blake2b(
            f"{seed}:{document_id}".encode(),
            digest_size=8,
        ).digest()
        priority = int.from_bytes(digest, "big")
        item = (-priority, index)
        if len(distractors) < distractor_limit:
            heapq.heappush(distractors, item)
        elif item > distractors[0]:
            heapq.heapreplace(distractors, item)

    if len(required_indices) != len(required_ids):
        raise ValueError("qrels reference documents missing from the corpus")
    return sorted(required_indices + [index for _, index in distractors])


def export_task(
    task_name: str,
    task_config: dict,
    output_root: Path,
    sample_queries: int | None,
    sample_corpus: int | None,
    seed: int,
    query_partition: dict | None = None,
) -> dict:
    from datasets import load_dataset

    query_corpus_repo = f"CoIR-Retrieval/{task_name}-queries-corpus"
    qrels_repo = f"CoIR-Retrieval/{task_name}-qrels"
    query_corpus = load_dataset(
        query_corpus_repo,
        revision=task_config["query_corpus_revision"],
    )
    qrels_dataset = load_dataset(
        qrels_repo,
        revision=task_config["qrels_revision"],
    )
    test_qrels = list(qrels_dataset["test"])
    included_queries = sampled_query_ids(
        test_qrels,
        sample_queries,
        seed,
        query_partition,
    )
    query_by_id = {
        str(row["_id"]): row
        for row in query_corpus["queries"]
        if str(row["_id"]) in included_queries
    }
    missing_queries = included_queries - set(query_by_id)
    if missing_queries:
        raise ValueError(
            f"{task_name}: qrels reference missing queries: "
            + ", ".join(sorted(missing_queries)[:10])
        )
    required_corpus_ids = {
        str(row["corpus_id"])
        for row in test_qrels
        if str(row["query_id"]) in included_queries
    }
    corpus = query_corpus["corpus"]
    corpus_indices = sampled_corpus_indices(
        corpus,
        required_corpus_ids,
        sample_corpus,
        seed,
    )

    output = output_root / task_name
    temporary = output.with_name(f".{output.name}.tmp")
    shutil.rmtree(temporary, ignore_errors=True)
    temporary.mkdir(parents=True)
    corpus_path = temporary / "corpus.jsonl"
    queries_path = temporary / "queries.jsonl"
    qrels_path = temporary / "qrels.tsv"

    languages: set[str] = set()
    corpus_count = 0
    with corpus_path.open("w", encoding="utf-8") as handle:
        for position, source_index in enumerate(corpus_indices):
            row = corpus[source_index]
            language = str(row.get("language") or "").strip()
            if language:
                languages.add(language)
            document = {
                "_id": str(row["_id"]),
                "title": row.get("title") or "",
                "text": row.get("text") or "",
                "metadata": {
                    "path": (
                        f"documents/{position:09d}."
                        f"{safe_extension(row.get('language'))}"
                    ),
                    "language": language,
                    "source": query_corpus_repo,
                },
            }
            handle.write(json.dumps(document, ensure_ascii=True) + "\n")
            corpus_count += 1

    with queries_path.open("w", encoding="utf-8") as handle:
        for query_id in sorted(query_by_id):
            row = query_by_id[query_id]
            query = {
                "_id": query_id,
                "text": row.get("text") or row.get("query") or "",
            }
            handle.write(json.dumps(query, ensure_ascii=True) + "\n")

    qrel_count = 0
    with qrels_path.open("w", encoding="utf-8") as handle:
        handle.write("query-id\tcorpus-id\tscore\n")
        for row in test_qrels:
            if str(row["query_id"]) not in included_queries:
                continue
            handle.write(
                f"{row['query_id']}\t{row['corpus_id']}\t{int(row['score'])}\n"
            )
            qrel_count += 1

    provenance = {
        "schema_version": 1,
        "task": task_name,
        "license": task_config["license"],
        "license_source": (f"https://huggingface.co/datasets/{query_corpus_repo}"),
        "query_corpus": {
            "repository": query_corpus_repo,
            "revision": task_config["query_corpus_revision"],
        },
        "qrels": {
            "repository": qrels_repo,
            "revision": task_config["qrels_revision"],
            "split": "test",
        },
        "sample": {
            "query_limit": sample_queries,
            "corpus_limit": sample_corpus,
            "seed": seed if sample_queries is not None else None,
            "query_partition": query_partition,
        },
        "counts": {
            "source_corpus": len(corpus),
            "corpus": corpus_count,
            "queries": len(query_by_id),
            "qrels": qrel_count,
        },
        "languages": sorted(languages, key=str.lower),
        "checksums": {
            "corpus.jsonl": sha256_file(corpus_path),
            "queries.jsonl": sha256_file(queries_path),
            "qrels.tsv": sha256_file(qrels_path),
        },
    }
    (temporary / "provenance.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    shutil.rmtree(output, ignore_errors=True)
    temporary.rename(output)
    return provenance


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=root / "benchmarks" / "public" / "manifest.json",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profile")
    parser.add_argument("--task", action="append", default=[])
    parser.add_argument("--sample-queries", type=int)
    parser.add_argument("--sample-corpus", type=int)
    parser.add_argument("--seed", type=int, default=20260615)
    args = parser.parse_args()

    manifest = load_manifest(args.manifest)
    tasks = selected_tasks(manifest, args.profile, args.task)
    args.output.mkdir(parents=True, exist_ok=True)
    exported = []
    profile_options = (
        manifest["profiles"].get(args.profile, {}).get("task_options", {})
        if args.profile
        else {}
    )
    for task in tasks:
        options = profile_options.get(task, {})
        exported.append(
            export_task(
                task,
                manifest["tasks"][task],
                args.output,
                args.sample_queries
                if args.sample_queries is not None
                else options.get("sample_queries"),
                args.sample_corpus
                if args.sample_corpus is not None
                else options.get("sample_corpus"),
                options.get("seed", args.seed),
                options.get("query_partition"),
            )
        )
    summary = {
        "profile": args.profile,
        "tasks": [item["task"] for item in exported],
        "corpus": sum(item["counts"]["corpus"] for item in exported),
        "queries": sum(item["counts"]["queries"] for item in exported),
        "qrels": sum(item["counts"]["qrels"] for item in exported),
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
