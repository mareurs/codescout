"""Stage 0 — freeze a corpus and write its manifest.

`freeze()` copies a set of sources (transcript dirs, usage.db files, repo paths) into
`out_root/corpus_id/`, snapshotting each SQLite file with `sqlite3.Connection.backup` (never a
plain file copy — a live-written usage.db copied byte-for-byte can be torn mid-page), and writes
`manifest.json` recording a sha256 + byte size per frozen file, transcript/usage-row counts, tool
versions, and per-repo HEAD SHAs. `verify()` reads a frozen corpus's manifest back and recomputes
each listed file's sha256, returning the relative paths that no longer match (or are missing).

Global Constraint (see docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md
§ Global Constraints): corpora live OUTSIDE this repo, under
~/work/claude/measurement-corpora/<corpus-id>/. Only manifests, readouts and code are committed —
never a transcript, a usage.db copy, or anything that could contain the operator's private
CLAUDE.md. `freeze()` refuses an `out_root` inside the repo for exactly this reason.
"""
import hashlib
import json
import pathlib
import sqlite3
import subprocess
from datetime import datetime, timedelta, timezone

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]

GLOBAL_CONSTRAINT_MSG = (
    "freeze() refuses an out_root inside the repo (Global Constraint, "
    "docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md § Global Constraints): "
    "corpora live outside the repo, under ~/work/claude/measurement-corpora/<corpus-id>/. "
    "Only manifests, readouts and code are committed — never a transcript or a usage.db copy."
)


def _sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _parse_iso(s):
    return datetime.fromisoformat(s.replace("Z", "+00:00"))


def _format_iso(dt):
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _refuse_if_inside_repo(out_root):
    out_root = out_root.resolve()
    repo_root = REPO_ROOT.resolve()
    try:
        out_root.relative_to(repo_root)
    except ValueError:
        return  # not inside the repo — fine
    raise ValueError(GLOBAL_CONSTRAINT_MSG + f" (got out_root={out_root})")


def _tool_version(*argv):
    try:
        out = subprocess.run(argv, capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.SubprocessError):
        return None
    if out.returncode != 0:
        return None
    return (out.stdout or out.stderr or "").strip() or None


def _git_head_sha(repo_path):
    try:
        out = subprocess.run(
            ["git", "-C", str(repo_path), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if out.returncode != 0:
        return None
    return out.stdout.strip() or None


def _count_usage_rows(conn):
    # codescout's usage.db names its call-log table `tool_calls` (src/usage/db.rs). Any snapshot
    # this old or a differently-shaped one is tolerated — a count of 0 is honest, not a crash.
    try:
        cur = conn.execute("SELECT COUNT(*) FROM tool_calls")
        return cur.fetchone()[0]
    except sqlite3.Error:
        return 0


def _record_file(files, corpus_dir, dest):
    rel = dest.relative_to(corpus_dir).as_posix()
    files[rel] = {"sha256": _sha256_of(dest), "bytes": dest.stat().st_size}


def freeze(corpus_id, sources, out_root):
    """Copy `sources` into `out_root/corpus_id/`, write `manifest.json`, return it.

    `sources` keys: `transcript_dirs` (project dirs across profiles), `usage_dbs` (paths),
    `repos` ({name: path}), `bounds` ({start_utc, end_utc}).
    """
    out_root = pathlib.Path(out_root)
    _refuse_if_inside_repo(out_root)

    corpus_dir = out_root / corpus_id
    corpus_dir.mkdir(parents=True, exist_ok=True)

    files = {}
    transcripts = 0
    subagent_transcripts = 0
    usage_rows = 0

    # --- transcripts: every top-level *.jsonl, plus every <sid>/subagents/*.jsonl (R21: no
    # mtime filter — bounds are recorded in the manifest, never applied to file selection). ---
    transcripts_out = corpus_dir / "transcripts"
    for i, raw_dir in enumerate(sources.get("transcript_dirs", [])):
        src_dir = pathlib.Path(raw_dir)
        # Disambiguate by the profile directory two levels up (<profile>/projects/<slug>), with
        # an index prefix so two identically-named profiles/slugs still land in distinct dirs.
        try:
            profile_name = src_dir.parent.parent.name
        except Exception:
            profile_name = "profile"
        dest_project_dir = transcripts_out / f"{i:02d}-{profile_name}" / src_dir.name
        dest_project_dir.mkdir(parents=True, exist_ok=True)

        for jsonl in sorted(src_dir.glob("*.jsonl")):
            dest = dest_project_dir / jsonl.name
            dest.write_bytes(jsonl.read_bytes())
            _record_file(files, corpus_dir, dest)
            transcripts += 1

        for sub in sorted(p for p in src_dir.iterdir() if p.is_dir()):
            subagents_dir = sub / "subagents"
            if not subagents_dir.is_dir():
                continue
            dest_sub_dir = dest_project_dir / sub.name / "subagents"
            dest_sub_dir.mkdir(parents=True, exist_ok=True)
            for jsonl in sorted(subagents_dir.glob("*.jsonl")):
                dest = dest_sub_dir / jsonl.name
                dest.write_bytes(jsonl.read_bytes())
                _record_file(files, corpus_dir, dest)
                subagent_transcripts += 1

    # --- usage.db snapshots: sqlite3 .backup, never a byte copy (R20). ---
    usage_out = corpus_dir / "usage_dbs"
    for i, raw_db in enumerate(sources.get("usage_dbs", [])):
        db_path = pathlib.Path(raw_db)
        usage_out.mkdir(parents=True, exist_ok=True)
        dest = usage_out / f"{i:02d}-{db_path.name}"
        src_conn = sqlite3.connect(str(db_path))
        dest_conn = sqlite3.connect(str(dest))
        try:
            src_conn.backup(dest_conn)
        finally:
            src_conn.close()
        try:
            usage_rows += _count_usage_rows(dest_conn)
        finally:
            dest_conn.close()
        _record_file(files, corpus_dir, dest)

    # --- repos: not copied — just the HEAD sha at freeze time. ---
    repos = {}
    for name, path in sources.get("repos", {}).items():
        repos[name] = _git_head_sha(path)

    # --- bounds: retained window as given; decision window is the last 7 days of it (A1.2). ---
    raw_bounds = sources.get("bounds", {})
    retained_start = raw_bounds.get("start_utc")
    retained_end = raw_bounds.get("end_utc")
    bounds = {"retained": {"start_utc": retained_start, "end_utc": retained_end}}
    if retained_end:
        end_dt = _parse_iso(retained_end)
        decision_start = _format_iso(end_dt - timedelta(days=7))
        bounds["decision"] = {"start_utc": decision_start, "end_utc": retained_end}
    else:
        bounds["decision"] = {"start_utc": None, "end_utc": None}

    manifest = {
        "corpus_id": corpus_id,
        "created_utc": _format_iso(datetime.now(timezone.utc)),
        "bounds": bounds,
        "files": files,
        "counts": {
            "transcripts": transcripts,
            "subagent_transcripts": subagent_transcripts,
            "usage_rows": usage_rows,
        },
        "versions": {
            "codescout_sha": _git_head_sha(REPO_ROOT),
            "claude_code": _tool_version("claude", "--version"),
            "codex": _tool_version("codex", "--version"),
        },
        "repos": repos,
        "exclusions": [],
    }

    # R1: manifest.json is never itself listed in `files` and never hashed by verify().
    (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))

    return manifest


def verify(corpus_dir):
    """Recompute sha256 for every file `manifest.json` lists; return the relative paths that
    are missing or no longer match. Empty list means the corpus is intact."""
    corpus_dir = pathlib.Path(corpus_dir)
    manifest = json.loads((corpus_dir / "manifest.json").read_text())

    mismatches = []
    for rel, meta in manifest.get("files", {}).items():
        path = corpus_dir / rel
        if not path.is_file():
            mismatches.append(rel)
            continue
        if _sha256_of(path) != meta.get("sha256"):
            mismatches.append(rel)
    return sorted(mismatches)
