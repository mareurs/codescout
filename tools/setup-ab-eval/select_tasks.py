"""Metadata-only task selection: the rule in selection-rule.json, applied.

    select(candidates, rule) -> [task id]      the registered selection, interleaved by source
    ranked(candidates, rule) -> {source: [id]} every eligible candidate in draw order (no cap)

A candidate is a dict. `select` projects it to `id`, `repo`, `gold_files` and
`gold_added_lines` before it does anything else, so no other field (an outcome such as `pass`
or `cost`, or anything added later) can reach the rule. `id` is read as identity only: the
development-task exclusion and the draw key. The rule was committed before any candidate
outcome was read (selection-rule.json, its own commit).

Building the candidate list reads task metadata, never a result file:

    foreign_candidate(record)       a SWE-bench record: its `patch` field
    private_candidate(task_dir)     an S5 task.json: `git diff --no-renames parent fix` in its
                                    repoPath, read-only through git, without the hidden test
                                    files and docs/** (private_gold_spec)

    python3 select_tasks.py --dataset <dataset.json> --s5-tasks <dir> --out <dir>

writes candidates.json (metadata only) and selection.json (the selected ids, the draw keys and
the per-source draw order kept for replacements). Stdlib only.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
RULE_PATH = HERE / "selection-rule.json"
FIELDS = ("id", "repo", "gold_files", "gold_added_lines")


def load_rule(path=RULE_PATH):
    return json.loads(Path(path).read_text())


def _project(candidate):
    """The four fields the rule may read; anything else is dropped here, unread."""
    missing = [f for f in FIELDS if f not in candidate]
    if missing:
        raise ValueError(f"candidate {candidate.get('id')!r} lacks {missing}")
    c = {f: candidate[f] for f in FIELDS}
    if not isinstance(c["id"], str) or not c["id"]:
        raise ValueError(f"candidate id must be a non-empty string: {c['id']!r}")
    if not isinstance(c["repo"], str) or not c["repo"]:
        raise ValueError(f"{c['id']}: repo must be a non-empty string")
    if not isinstance(c["gold_files"], list) or not all(isinstance(f, str) for f in c["gold_files"]):
        raise ValueError(f"{c['id']}: gold_files must be a list of paths")
    if type(c["gold_added_lines"]) is not int or c["gold_added_lines"] < 0:
        raise ValueError(f"{c['id']}: gold_added_lines must be a non-negative int")
    return c


def draw_key(rule, task_id):
    return hashlib.sha256(f"{rule['draw']['salt']}:{task_id}".encode()).hexdigest()


def _source_of(rule, repo):
    for name, src in rule["sources"].items():
        if repo in src["repos"]:
            return name
    return None


def _eligible(rule, c):
    b = rule["eligible"]
    files, added = len(c["gold_files"]), c["gold_added_lines"]
    return (b["gold_files"]["min"] <= files <= b["gold_files"]["max"]
            and b["gold_added_lines"]["min"] <= added <= b["gold_added_lines"]["max"])


def _projected(candidates):
    out, seen = [], set()
    for raw in candidates:
        c = _project(raw)
        if c["id"] in seen:
            raise ValueError(f"duplicate candidate id {c['id']!r}")
        seen.add(c["id"])
        out.append(c)
    return out


def ranked(candidates, rule):
    """{source: [id, ...]}: every eligible, non-excluded candidate of the source, in draw order."""
    excluded = set(rule["exclude_ids"])
    order = {name: [] for name in rule["sources"]}
    for c in _projected(candidates):
        source = _source_of(rule, c["repo"])
        if source is None or c["id"] in excluded or not _eligible(rule, c):
            continue
        order[source].append(c)
    return {name: [c["id"] for c in sorted(cs, key=lambda c: (draw_key(rule, c["id"]), c["id"]))]
            for name, cs in order.items()}


def select(candidates, rule, *, failed=()):
    """The registered selection: per source, walk the draw order under the repo cap up to
    `count`; then interleave the sources by rank. `failed` (keyword-only, an addition to the
    brief's signature) holds ids that failed a Stage-0 instrument gate: they are skipped, so the
    next admissible candidate of their source takes their place (the rule's replacement)."""
    projected = {c["id"]: c for c in _projected(candidates)}
    failed = set(failed)
    picks = {}
    for name, order in ranked(list(projected.values()), rule).items():
        src = rule["sources"][name]
        cap = src.get("max_per_repo")
        per_repo, chosen = {}, []
        for task_id in order:
            if len(chosen) >= src["count"]:
                break
            repo = projected[task_id]["repo"]
            if task_id in failed or (cap is not None and per_repo.get(repo, 0) >= cap):
                continue
            per_repo[repo] = per_repo.get(repo, 0) + 1
            chosen.append(task_id)
        picks[name] = chosen
    out = []
    for rank in range(max((len(v) for v in picks.values()), default=0)):
        for name in rule["sources"]:  # the rule's source order: private, then foreign
            if rank < len(picks[name]):
                out.append(picks[name][rank])
    return out


# --- metadata --------------------------------------------------------------------------------

def patch_stats(patch):
    """(files, added lines) of a unified diff: `diff --git` headers and `+` lines (not `+++`)."""
    files = re.findall(r"^diff --git a/(\S+) b/", patch, re.M)
    added = sum(1 for line in patch.splitlines() if line.startswith("+") and not line.startswith("+++"))
    return files, added


def foreign_candidate(record):
    files, added = patch_stats(record["patch"])
    return {"id": record["instance_id"], "repo": record["repo"], "gold_files": files,
            "gold_added_lines": added}


def private_gold_spec(task):
    """The registered private gold patch's pathspec: the whole diff without the hidden test
    files and without docs/** (selection-rule.json sources.private.gold_patch)."""
    hidden = list(task["hidden"]["files"]) + list(task["hidden"].get("extra", []))
    return [".", ":(exclude)docs/**"] + [f":(exclude){h}" for h in hidden]


def git_bytes(repo, *args):
    """Read-only git in the operator's checkout: committed objects only."""
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True).stdout


def private_gold_patch(task, extra_spec=()):
    return git_bytes(task["repoPath"], "diff", "--no-renames", "--binary", task["parent"], task["fix"],
                     "--", *private_gold_spec(task), *extra_spec).decode("utf-8", "replace")


def private_candidate(task_dir):
    task = json.loads((Path(task_dir) / "task.json").read_text())
    files, added = patch_stats(private_gold_patch(task))
    return {"id": task["id"], "repo": task["repo"], "gold_files": files, "gold_added_lines": added}


def build_candidates(dataset, s5_tasks, rule):
    foreign_repos = set(rule["sources"]["foreign"]["repos"])
    records = json.loads(Path(dataset).read_text())
    out = [foreign_candidate(r) for r in records if r["repo"] in foreign_repos]
    out += [private_candidate(d) for d in sorted(Path(s5_tasks).glob("cs-p*")) if (d / "task.json").is_file()]
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dataset", required=True, type=Path)
    ap.add_argument("--s5-tasks", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--failed", nargs="*", default=[])
    args = ap.parse_args(argv)
    rule = load_rule()
    candidates = build_candidates(args.dataset, args.s5_tasks, rule)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "candidates.json").write_text(json.dumps(candidates, indent=2) + "\n")
    chosen = select(candidates, rule, failed=args.failed)
    selection = {
        "rule_sha256": hashlib.sha256(RULE_PATH.read_bytes()).hexdigest(),
        "dataset_sha256": hashlib.sha256(args.dataset.read_bytes()).hexdigest(),
        "s5_tasks": str(args.s5_tasks),
        "failed": sorted(args.failed),
        "selected": chosen,
        "draw_order": ranked(candidates, rule),
        "keys": {c["id"]: draw_key(rule, c["id"]) for c in candidates},
        "metadata": {c["id"]: {"repo": c["repo"], "gold_files": len(c["gold_files"]),
                               "gold_added_lines": c["gold_added_lines"]}
                     for c in candidates if c["id"] in chosen},
    }
    (args.out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n")
    print(json.dumps({"selected": chosen}, indent=2))


if __name__ == "__main__":
    main()
