#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "pyarrow>=18,<22",
#   "requests>=2.32,<3",
# ]
# ///
"""Export pinned CoREB data into ivygrep retrieval benchmark layouts."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import shutil

DATASET = "hq-bench/coreb"
DEFAULT_SPLIT = "release_v2603"
PARQUET_API = f"https://datasets-server.huggingface.co/parquet?dataset={DATASET}"
EXPECTED_SHA256 = {
    "code2code_qrels": "be6f68a99ea041511850ace1c65f2a38dfb7072c2df9aba39e6d03ad0a5efb22",
    "code2code_queries": "0c54955559832342045159e29660e4110dfe4cb8d5f2a2a7bea467dce4319b90",
    "code2text_qrels": "e47a9b7a691323132e220113daf5d4ad137546afac769364343513521cfc013f",
    "code2text_queries": "8fa774266641fd1aa6933f8a3fa04b0b22912d31e01433f5714344a61b2ecc0b",
    "code_corpus": "516f72967bd9062437396921617e744a5ec533bf13cea897653b7b1a05b98e9f",
    "text2code_qrels": "eeb575e95115f33739bb832f6633fab7285d49e27e3bd50d8048a9044f54dd90",
    "text2code_queries": "22a77c9c1f0f8762b6af2c363a27d8c2144a39c379bcfe852de7226510e6c052",
    "text_corpus": "de4a706e6176ab862a122049ea2120e815f4402f328fa7e8de7c9afdea62126d",
}
TASKS = {
    "coreb-text2code": ("code_corpus", "text2code_queries", "text2code_qrels"),
    "coreb-code2code": ("code_corpus", "code2code_queries", "code2code_qrels"),
    "coreb-code2text": ("text_corpus", "code2text_queries", "code2text_qrels"),
}
EXTENSIONS = {
    "cpp": "cpp",
    "go": "go",
    "java": "java",
    "python": "py",
    "ruby": "rb",
}


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def validate_hash(config: str, digest: str, split: str) -> None:
    if split != DEFAULT_SPLIT:
        return
    expected = EXPECTED_SHA256[config]
    if digest != expected:
        raise ValueError(f"CoREB {config} SHA-256 changed: expected {expected}, got {digest}")


def fetch_parquet_urls(split: str, session=None) -> dict[str, str]:
    if session is None:
        import requests

        session = requests
    response = session.get(PARQUET_API, timeout=30)
    response.raise_for_status()
    files = {
        row["config"]: row["url"]
        for row in response.json()["parquet_files"]
        if row["split"] == split
    }
    needed = {config for configs in TASKS.values() for config in configs}
    missing = sorted(needed - set(files))
    if missing:
        raise ValueError(f"CoREB split {split} misses configs: {', '.join(missing)}")
    return files


def download_rows(url: str, session=None) -> tuple[list[dict], str]:
    if session is None:
        import requests

        session = requests
    import pyarrow.parquet as parquet

    response = session.get(url, timeout=60)
    response.raise_for_status()
    return parquet.read_table(io.BytesIO(response.content)).to_pylist(), sha256_bytes(
        response.content
    )


def code_document(row: dict, position: int) -> dict:
    language = str(row.get("language") or "").lower()
    extension = EXTENSIONS.get(language, "txt")
    document_id = str(row["code_id"])
    return {
        "_id": document_id,
        "title": "",
        "text": str(row["code"]),
        "metadata": {
            "path": f"documents/{position:06d}-{document_id}.{extension}",
            "language": language,
            "source": DATASET,
        },
    }


def text_document(row: dict, position: int) -> dict:
    document_id = str(row["text_id"])
    return {
        "_id": document_id,
        "title": "",
        "text": str(row["text"]),
        "metadata": {
            "path": f"documents/{position:06d}-{document_id}.md",
            "language": "markdown",
            "source": DATASET,
        },
    }


def query_document(row: dict) -> dict:
    return {"_id": str(row["query_id"]), "text": str(row["query"])}


def positive_qrels(rows: list[dict]) -> list[tuple[str, str, int]]:
    # CoREB grade 1 rows are hard negatives. Only grade 2 counts as relevant.
    return [
        (str(row["query_id"]), str(row["doc_id"]), int(row["relevance"]))
        for row in rows
        if int(row["relevance"]) >= 2
    ]


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def export_task(
    output_root: Path,
    task: str,
    split: str,
    tables: dict[str, list[dict]],
    hashes: dict[str, str],
    urls: dict[str, str],
) -> dict:
    corpus_config, query_config, qrels_config = TASKS[task]
    output = output_root / task
    temporary = output.with_name(f".{output.name}.tmp")
    shutil.rmtree(temporary, ignore_errors=True)
    temporary.mkdir(parents=True)

    corpus_builder = code_document if corpus_config == "code_corpus" else text_document
    corpus = [corpus_builder(row, index) for index, row in enumerate(tables[corpus_config])]
    queries = [query_document(row) for row in tables[query_config]]
    qrels = positive_qrels(tables[qrels_config])

    corpus_ids = {row["_id"] for row in corpus}
    query_ids = {row["_id"] for row in queries}
    if any(query_id not in query_ids or doc_id not in corpus_ids for query_id, doc_id, _ in qrels):
        raise ValueError(f"{task}: qrels reference missing queries or documents")

    write_jsonl(temporary / "corpus.jsonl", corpus)
    write_jsonl(temporary / "queries.jsonl", queries)
    (temporary / "qrels.tsv").write_text(
        "query-id\tcorpus-id\tscore\n"
        + "".join(f"{query_id}\t{doc_id}\t{score}\n" for query_id, doc_id, score in qrels),
        encoding="utf-8",
    )
    provenance = {
        "schema_version": 1,
        "dataset": DATASET,
        "split": split,
        "task": task,
        "license": "Apache-2.0",
        "configs": {
            config: {"url": urls[config], "sha256": hashes[config]}
            for config in TASKS[task]
        },
        "counts": {"corpus": len(corpus), "queries": len(queries), "qrels": len(qrels)},
        "relevance_threshold": 2,
    }
    (temporary / "provenance.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    shutil.rmtree(output, ignore_errors=True)
    temporary.rename(output)
    return provenance


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--task", action="append", choices=sorted(TASKS))
    args = parser.parse_args()

    selected = args.task or list(TASKS)
    urls = fetch_parquet_urls(args.split)
    configs = sorted({config for task in selected for config in TASKS[task]})
    tables: dict[str, list[dict]] = {}
    hashes: dict[str, str] = {}
    for config in configs:
        tables[config], hashes[config] = download_rows(urls[config])
        validate_hash(config, hashes[config], args.split)
    summaries = [
        export_task(args.output, task, args.split, tables, hashes, urls)
        for task in selected
    ]
    print(json.dumps(summaries, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
