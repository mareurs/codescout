"""Phase 1b Step 1: the two labellers, blind to each other and to the classifier.

    python3 run_labellers.py [--audit DIR] [--only codex|claude] [--workers N] [--dry-run]

Implements docs/evals/phase1b-local-classifier-preregistration.md § Step 1's labellers, over the
items label_items.py wrote (DIR/items.jsonl) and the menu draw_audit_sample.py wrote (DIR/menu.json):
1. Codex gpt-6-astra at medium, by `codex exec` on the ChatGPT subscription with the API-key
   variables stripped: ONE run, in a new CODEX_HOME (the auth link and the model config only), in
   a new directory holding only audit-instruction.md, menu.json and items.jsonl.
2. Claude Opus 5.5 by `claude -p` on the clean judge channel: JUDGE_CONFIG_DIR, refused unless
   phase2-score-dp1.dirty_reasons finds nothing (no CLAUDE.md, no plugins, no hooks). Items go in
   fixed batches of 25, in file order; each prompt is the instruction, menu.json and its batch,
   verbatim.
Every call's output is parsed as JSON lines and must hold exactly one valid answer
(score_audit.valid) for each item of the call and none for any other. A call that errors, or
returns an invalid or incomplete set, is re-run once, Codex in another new CODEX_HOME. A second
failure stops phase 1b at Step 1: exit 4, no labels file is written, and Claude batches not yet
started are cancelled.
Neither labeller sees the other's answers, key.jsonl, or anything of the classifier's.

Writes DIR/labels-codex.jsonl and DIR/labels-claude.jsonl (answers in items.jsonl order),
DIR/raw/ (every attempt's output, error and Codex log) and DIR/run-header.json (models, channel,
the sha256 of the three input files, times, and a stop reason if one fired). --dry-run prints the
plan and calls nothing.
"""
import argparse
import concurrent.futures as cf
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "stage2"))
import generate_synthetic as gs  # noqa: E402 -- the channels and the JSON-lines parser phase 1 used
import score_audit as sa  # noqa: E402

CLAUDE_MODEL = "claude-opus-5-5"         # audit_synthetic.py's Claude auditor, the same channel
CLAUDE_SYSTEM = "You are a careful auditor. Output only what the instructions ask for."
BATCH = 25
INSTRUCTION = HERE / "audit-instruction.md"
CODEX_DIRECTIVE = ("Read audit-instruction.md, menu.json and items.jsonl in this directory. Do exactly what "
                   "audit-instruction.md asks, for every item in items.jsonl. Reply with only the JSON lines it "
                   "specifies, one per item, and nothing else. Use no other file.")


class Stop(Exception):
    """A call failed twice: phase 1b stops at Step 1."""


def check_call(answers: list, ids: list[str], menu: set) -> str | None:
    """None when the answers are exactly one valid answer per id of this call; else the reason."""
    want, seen = set(ids), set()
    for a in answers:
        if not sa.valid(a, menu):
            return f"invalid answer {a.get('id') if isinstance(a, dict) else a!r}"
        if a["id"] not in want:
            return f"answer for an item not in this call: {a['id']}"
        if a["id"] in seen:
            return f"duplicate answer: {a['id']}"
        seen.add(a["id"])
    if seen != want:
        return f"{len(want - seen)} of {len(want)} items unanswered"
    return None


def run_call(name: str, ids: list[str], call, menu: set, raw_dir: Path) -> list[dict]:
    """At most two attempts of call(attempt) -> raw text. Returns the answers in `ids` order, or
    raises Stop naming both failures."""
    reasons = []
    for attempt in (1, 2):
        try:
            raw = call(attempt)
        except Exception as e:                       # noqa: BLE001 -- recorded; re-run once
            (raw_dir / f"{name}.a{attempt}.err").write_text(str(e))
            reasons.append(f"attempt {attempt}: {str(e)[:200]}")
            continue
        (raw_dir / f"{name}.a{attempt}.txt").write_text(raw)
        answers = gs.parse(raw)
        why = check_call(answers, ids, menu)
        if why is None:
            by_id = {a["id"]: a for a in answers}
            return [by_id[i] for i in ids]
        reasons.append(f"attempt {attempt}: {why}")
    raise Stop(f"{name}: " + "; ".join(reasons))


def batches(items: list[dict]) -> list[list[dict]]:
    return [items[s:s + BATCH] for s in range(0, len(items), BATCH)]


def claude_prompt(instruction: str, menu_text: str, batch: list[dict]) -> str:
    return (f"{instruction}\n\n## menu.json\n\n{menu_text}\n\n## Items\n\n"
            + "\n".join(json.dumps(i, ensure_ascii=False) for i in batch))


def codex_files(instruction: str, menu_text: str, items_text: str) -> dict[str, str]:
    return {"audit-instruction.md": instruction, "menu.json": menu_text, "items.jsonl": items_text}


def check_channel(cfg: str) -> None:
    if not cfg:
        raise SystemExit("refused: JUDGE_CONFIG_DIR is unset; Claude labels only on the clean judge channel")
    dirty = gs.sc.dirty_reasons(cfg)
    if dirty:
        raise SystemExit(f"refused: JUDGE_CONFIG_DIR {cfg} is not the clean channel: {dirty}")


def new_codex_home() -> Path:
    home = Path(tempfile.mkdtemp(prefix="codex-audit-home-"))
    (home / "auth.json").symlink_to(Path.home() / ".codex/auth.json")
    (home / "config.toml").write_text(f'model = "{gs.CODEX_MODEL}"\nmodel_reasoning_effort = "{gs.CODEX_EFFORT}"\n')
    return home


def codex_call(files: dict[str, str], home: Path, log_path: Path, timeout: int = 3600) -> str:
    """gs.codex_complete's invocation, with the three files in the directory instead of task.md."""
    work = Path(tempfile.mkdtemp(prefix="audit-codex-"))
    try:
        for name, text in files.items():
            (work / name).write_text(text)
        env = {k: v for k, v in os.environ.items() if k not in ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL")}
        env["CODEX_HOME"] = str(home)
        p = subprocess.run(
            ["codex", "exec", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "approval_policy=never",
             "-C", str(work), "--ephemeral", "-o", str(work / "last.txt"), CODEX_DIRECTIVE],
            capture_output=True, text=True, timeout=timeout, stdin=subprocess.DEVNULL, env=env)
        log_path.write_text(p.stdout + "\n--- stderr ---\n" + p.stderr)
        if p.returncode != 0:
            raise RuntimeError(f"codex exec exit {p.returncode}: {p.stderr.strip()[-200:]}")
        return (work / "last.txt").read_text()
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--audit", type=Path, default=HERE / "audit")
    ap.add_argument("--only", choices=("codex", "claude"), default=None)
    ap.add_argument("--workers", type=int, default=4, help="Claude batches in flight at once")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    d = args.audit
    instruction = INSTRUCTION.read_text()
    menu_text = (d / "menu.json").read_text()
    items_text = (d / "items.jsonl").read_text()
    menu = set(json.loads(menu_text))
    items = [json.loads(line) for line in items_text.splitlines()]
    ids = [i["id"] for i in items]
    who = [args.only] if args.only else ["codex", "claude"]
    header = dict(labellers=who, codex=f"{gs.CODEX_MODEL}/{gs.CODEX_EFFORT}", claude=CLAUDE_MODEL,
                  items=len(items), claude_batches=len(batches(items)),
                  sha256={n: hashlib.sha256(t.encode()).hexdigest()
                          for n, t in codex_files(instruction, menu_text, items_text).items()},
                  started=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if args.dry_run:
        print(json.dumps(header, indent=1))
        return 0
    cfg = os.environ.get("JUDGE_CONFIG_DIR", "")
    if "claude" in who:
        check_channel(cfg)
        header["claude_channel"] = cfg
    raw = d / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    got = {}
    try:
        if "codex" in who:
            files = codex_files(instruction, menu_text, items_text)
            got["codex"] = run_call("codex", ids, lambda a: codex_call(files, new_codex_home(), raw / f"codex.a{a}.log"),
                                    menu, raw)
        if "claude" in who:
            judge = gs.ClaudeGen(CLAUDE_MODEL, cfg, timeout=900)
            judge.SYSTEM = CLAUDE_SYSTEM

            def one(nb):
                n, b = nb
                return run_call(f"claude-b{n:02d}", [i["id"] for i in b],
                                lambda a: judge.complete(claude_prompt(instruction, menu_text, b))[0], menu, raw)
            ex = cf.ThreadPoolExecutor(args.workers)
            try:
                got["claude"] = [a for part in ex.map(one, enumerate(batches(items))) for a in part]
            finally:
                ex.shutdown(wait=True, cancel_futures=True)   # after a Stop, start no further batch
    except Stop as e:
        header.update(stop=str(e), ended=datetime.datetime.now(datetime.timezone.utc).isoformat())
        (d / "run-header.json").write_text(json.dumps(header, indent=1) + "\n")
        print(f"STOP at Step 1: {e}", file=sys.stderr)
        return 4
    for w, answers in got.items():
        (d / f"labels-{w}.jsonl").write_text("".join(json.dumps(a, ensure_ascii=False) + "\n" for a in answers))
    header["ended"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    (d / "run-header.json").write_text(json.dumps(header, indent=1) + "\n")
    print(f"labels written for {', '.join(got)}: {len(items)} items each")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
