"""SWE-bench-Live MultiLang Rust records: load them, project them, select from them by metadata.

    load_records(path, expected_sha256)  the jsonl, hash-checked, every REQUIRED key present
    candidate(record)                    the fixed projection the rule may read
    eligible(candidate, rule)            the registered rule (multilang-selection-rule.json)
    select_eligible(records, rule)       eligible instance ids in select_tasks.draw_key order
    DATASET_SHA256                       the sha256 of the registered multilang-rust.jsonl

`candidate` projects a record to `id`, `repo`, `gold_files`, `gold_added_lines`, `created_at`,
`expected_tests` and `nonempty` (which REQUIRED fields hold something) before `eligible` runs, so
no other field (an outcome such as `pass`, or anything added later) can reach the rule. Selection
reads metadata only, never a result. Stdlib only.
"""
import hashlib
import json
from datetime import date, datetime
from pathlib import Path

import select_tasks

HERE = Path(__file__).resolve().parent
RULE_PATH = HERE / "multilang-selection-rule.json"
# sha256 of the SWE-bench-Live MultiLang Rust export (171 records) the controls are run on
DATASET_SHA256 = "bf2dd8252a07d0f275663ef8f6ab87571dd9bc4ce386abf9ec98d3cd6bb51a81"
REQUIRED = ("instance_id", "repo", "base_commit", "created_at", "patch", "test_patch",
            "FAIL_TO_PASS", "PASS_TO_PASS", "test_cmds", "print_cmds", "log_parser", "docker_image")


def _day(created_at):
    """The calendar date of an ISO timestamp (`Z` accepted), or None for anything else."""
    if not isinstance(created_at, str):
        return None
    try:
        return datetime.fromisoformat(created_at).date()
    except ValueError:
        return None


def load_rule(path=RULE_PATH):
    return json.loads(Path(path).read_text())


def load_records(path, expected_sha256=None):
    """The records of a jsonl file. ValueError on a hash mismatch, a missing REQUIRED key or a
    duplicate instance_id; the hash is checked before the file is parsed."""
    raw = Path(path).read_bytes()
    if expected_sha256 is not None:
        actual = hashlib.sha256(raw).hexdigest()
        if actual != expected_sha256:
            raise ValueError(f"{path}: sha256 {actual} is not the expected {expected_sha256}")
    records, seen = [], set()
    for n, line in enumerate(raw.decode().splitlines(), 1):
        if not line.strip():
            continue
        record = json.loads(line)
        missing = [k for k in REQUIRED if k not in record]
        if missing:
            raise ValueError(f"{path}:{n}: record lacks {missing}")
        if _day(record["created_at"]) is None:
            raise ValueError(f"{path}:{n}: created_at is not an ISO date string: {record['created_at']!r}")
        if record["instance_id"] in seen:
            raise ValueError(f"{path}:{n}: duplicate instance_id {record['instance_id']!r}")
        seen.add(record["instance_id"])
        records.append(record)
    return records


def candidate(record):
    """The only fields the rule reads. `patch_stats` counts `diff --git` files and added lines
    (`+`, not `+++`) of the gold patch; `expected_tests` is len(F2P) + len(P2P)."""
    files, added = select_tasks.patch_stats(record["patch"])
    return {"id": record["instance_id"], "repo": record["repo"], "gold_files": files,
            "gold_added_lines": added, "created_at": record["created_at"],
            "expected_tests": len(record["FAIL_TO_PASS"]) + len(record["PASS_TO_PASS"]),
            "nonempty": sorted(k for k in REQUIRED if record[k])}


def eligible(candidate, rule, *, ignore_size=False):
    """The cutoff (by date) and the `require` fields always; unless `ignore_size`, also the
    hardness test (files >= min_files or added lines >= min_added_lines) and max_added_lines."""
    day = _day(candidate["created_at"])
    if day is None or day < date.fromisoformat(rule["created_at_min"]):
        return False  # fail closed: an unparseable date never passes the contamination cutoff
    if any(f not in candidate["nonempty"] for f in rule["require"]):
        return False
    if ignore_size:
        return True
    files, added = len(candidate["gold_files"]), candidate["gold_added_lines"]
    hard = files >= rule["min_files"] or added >= rule["min_added_lines"]
    return hard and added <= rule["max_added_lines"]


def select_eligible(records, rule, *, ignore_size=False):
    """Eligible instance ids, ascending by (draw key, id). `ignore_size` is passed to `eligible`
    (the cutoff and the required fields still apply)."""
    ids = [c["id"] for c in map(candidate, records) if eligible(c, rule, ignore_size=ignore_size)]
    return sorted(ids, key=lambda i: (select_tasks.draw_key(rule, i), i))
