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
started are cancelled. At most --workers batches are pending, terminal failure is observed in
completion order, and an in-flight batch cannot start another attempt after the stop is signalled.
Neither labeller sees the other's answers, key.jsonl, or anything of the classifier's.

Writes DIR/labels-codex.jsonl and DIR/labels-claude.jsonl (answers in items.jsonl order),
DIR/raw/ (every attempt's output, error and Codex log) and DIR/run-header.json (models, channel,
the sha256 of the three input files, times, status, and a stop reason if one fired). The header is
created exclusively before model calls; an existing header, raw directory or labels file refuses
the run. --only selects a standalone invocation, not a way to resume or combine separate runs in
the same audit directory. A stopped or interrupted invocation stays reserved; there is no automatic
resume or reset of its attempt budget. --dry-run prints the plan, reserves nothing and calls nothing.
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
import threading
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
    raises Stop naming both failures. Cancellation never consumes another attempt."""
    reasons = []
    for attempt in (1, 2):
        try:
            raw = call(attempt)
        except cf.CancelledError:
            raise
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


def reserve_run(d: Path, header: dict) -> Path:
    """Claim one audit invocation before spending tokens; stopped runs remain claimed."""
    previous = [p.name for p in (d / "raw", d / "run-header.json", *d.glob("labels-*.jsonl"))
                if p.exists() or p.is_symlink()]
    if previous:
        raise SystemExit(f"refused: audit run already exists in {d}: {sorted(previous)}; do not relaunch")
    header_path = d / "run-header.json"
    try:
        with header_path.open("x") as out:  # exclusive creation also refuses simultaneous starters
            out.write(json.dumps(header, indent=1) + "\n")
    except FileExistsError:
        raise SystemExit(f"refused: audit run already reserved in {d}; do not relaunch") from None
    raw = d / "raw"
    raw.mkdir()  # an unexpected legacy writer must not be silently reused either
    return raw


def run_claude_batches(items: list[dict], instruction: str, menu_text: str, menu: set,
                       raw: Path, judge, workers: int) -> list[dict]:
    """Bound pending work; notice failure in completion order, return answers in item order."""
    stopped = threading.Event()

    def one(n, batch):
        def call(attempt):
            if stopped.is_set():
                raise cf.CancelledError()
            return judge.complete(claude_prompt(instruction, menu_text, batch))[0]
        try:
            return run_call(f"claude-b{n:02d}", [i["id"] for i in batch], call, menu, raw)
        except cf.CancelledError:
            return None
        except Exception:
            stopped.set()  # workers see failure before the controller observes the future
            raise

    jobs = iter(enumerate(batches(items)))
    parts, pending = {}, {}
    ex = cf.ThreadPoolExecutor(workers)
    try:
        while True:
            # Belt and braces: `call` re-checks the signal before every attempt and the signal never
            # resets, so a batch submitted after the stop cancels before any model call anyway. This
            # check only avoids submitting it; no model-call test can see the difference.
            while len(pending) < workers and not stopped.is_set():
                job = next(jobs, None)
                if job is None:
                    break
                n, batch = job
                pending[ex.submit(one, n, batch)] = n
            if not pending:
                break
            done, _ = cf.wait(pending, return_when=cf.FIRST_COMPLETED)
            # Read every completed outcome before scheduling any replacement work.
            for future in done:
                n = pending.pop(future)
                answers = future.result()
                if answers is not None:
                    parts[n] = answers
    finally:
        stopped.set()
        ex.shutdown(wait=True, cancel_futures=True)
    return [answer for n in sorted(parts) for answer in parts[n]]



def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--audit", type=Path, default=HERE / "audit")
    ap.add_argument("--only", choices=("codex", "claude"), default=None)
    ap.add_argument("--workers", type=int, default=4, help="Claude batches in flight at once")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if args.workers < 1:
        ap.error("--workers must be positive")
    d = args.audit
    instruction = INSTRUCTION.read_text()
    menu_text = (d / "menu.json").read_text()
    items_text = (d / "items.jsonl").read_text()
    menu = set(json.loads(menu_text))
    items = [json.loads(line) for line in items_text.splitlines()]
    ids = [i["id"] for i in items]
    who = [args.only] if args.only else ["codex", "claude"]
    header = dict(labellers=who, codex=f"{gs.CODEX_MODEL}/{gs.CODEX_EFFORT}", claude=CLAUDE_MODEL,
                  items=len(items), claude_batches=len(batches(items)), status="running",
                  sha256={n: hashlib.sha256(t.encode()).hexdigest()
                          for n, t in codex_files(instruction, menu_text, items_text).items()},
                  started=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if args.dry_run:
        print(json.dumps(header, indent=1))
        return 0
    cfg = os.environ.get("JUDGE_CONFIG_DIR", "")
    judge = None
    if "claude" in who:
        check_channel(cfg)
        header["claude_channel"] = cfg
        # Validate the subscription credentials before the preceding Codex call spends anything.
        judge = gs.ClaudeGen(CLAUDE_MODEL, cfg, timeout=900)
        judge.SYSTEM = CLAUDE_SYSTEM
    raw = reserve_run(d, header)
    got = {}
    try:
        if "codex" in who:
            files = codex_files(instruction, menu_text, items_text)
            got["codex"] = run_call("codex", ids, lambda a: codex_call(files, new_codex_home(), raw / f"codex.a{a}.log"),
                                    menu, raw)
        if "claude" in who:
            got["claude"] = run_claude_batches(items, instruction, menu_text, menu, raw, judge, args.workers)
    except Stop as e:
        header.update(status="stopped", stop=str(e), ended=datetime.datetime.now(datetime.timezone.utc).isoformat())
        (d / "run-header.json").write_text(json.dumps(header, indent=1) + "\n")
        print(f"STOP at Step 1: {e}", file=sys.stderr)
        return 4
    for w, answers in got.items():
        with (d / f"labels-{w}.jsonl").open("x") as out:
            out.write("".join(json.dumps(a, ensure_ascii=False) + "\n" for a in answers))
    header.update(status="completed", ended=datetime.datetime.now(datetime.timezone.utc).isoformat())
    (d / "run-header.json").write_text(json.dumps(header, indent=1) + "\n")
    print(f"labels written for {', '.join(got)}: {len(items)} items each")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
