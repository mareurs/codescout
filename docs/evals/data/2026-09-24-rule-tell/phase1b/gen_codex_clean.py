"""Phase 1b Step 2: the three Codex-written clean texts, generated once, after registration.

    python3 gen_codex_clean.py [--dry-run]

Implements docs/evals/phase1b-local-classifier-preregistration.md § Step 2:
- the prompt is codex-clean-prompt.md, fixed at registration;
- the channel is Step 1's Codex channel: `codex exec` with the API-key variables stripped, a new
  CODEX_HOME (run_labellers.new_codex_home), in a new directory holding only the prompt and
  menu.json, the 14 local rules with their law and form-2b spec text (draw_audit_sample.build_menu).
  codex_call below is run_labellers.codex_call with this step's directive; the registered Step 1
  scripts are not edited to share it.
The output must be exactly three objects with ids codex-clean-1, codex-clean-2 and codex-clean-3,
each once, each with a non-empty text. Anything else is re-run once, in another new CODEX_HOME; a
second failure exits 4 with nothing written.

The texts are generated once: the script refuses when codex-clean-texts.jsonl exists. It writes
that file (read by clean_texts.codex_clean) and codex-clean-run/ (each attempt's output, the Codex
logs, and a header with the prompt's and menu's sha256). The texts are test input only, never
training input; they then go through Step 1's labellers with the other new clean texts.
"""
import argparse
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
import clean_texts as ct  # noqa: E402
import draw_audit_sample as da  # noqa: E402
import run_labellers as rl  # noqa: E402

PROMPT = HERE / "codex-clean-prompt.md"
RUN_DIR = HERE / "codex-clean-run"
IDS = ("codex-clean-1", "codex-clean-2", "codex-clean-3")
DIRECTIVE = ("Read codex-clean-prompt.md and menu.json in this directory. Do exactly what codex-clean-prompt.md "
             "asks. Reply with only the three JSON lines it specifies, and nothing else. Use no other file.")


def menu_text() -> str:
    return json.dumps(da.build_menu(da.load_gate(), da.local_menu()), indent=1, ensure_ascii=False) + "\n"


def check_texts(objs: list) -> str | None:
    """None when objs are exactly the three registered ids, once each, each with a non-empty text."""
    seen = []
    for o in objs:
        if not (isinstance(o, dict) and o.get("id") in IDS and isinstance(o.get("text"), str) and o["text"].strip()):
            return f"not a clean-text object: {o!r}"[:200]
        seen.append(o["id"])
    if sorted(seen) != sorted(IDS):
        return f"ids {sorted(seen)}, want each of {list(IDS)} once"
    return None


def codex_call(files: dict[str, str], home: Path, log_path: Path, timeout: int = 1200) -> str:
    work = Path(tempfile.mkdtemp(prefix="codex-clean-"))
    try:
        for name, text in files.items():
            (work / name).write_text(text)
        env = {k: v for k, v in os.environ.items() if k not in ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL")}
        env["CODEX_HOME"] = str(home)
        p = subprocess.run(
            ["codex", "exec", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "approval_policy=never",
             "-C", str(work), "--ephemeral", "-o", str(work / "last.txt"), DIRECTIVE],
            capture_output=True, text=True, timeout=timeout, stdin=subprocess.DEVNULL, env=env)
        log_path.write_text(p.stdout + "\n--- stderr ---\n" + p.stderr)
        if p.returncode != 0:
            raise RuntimeError(f"codex exec exit {p.returncode}: {p.stderr.strip()[-200:]}")
        return (work / "last.txt").read_text()
    finally:
        shutil.rmtree(work, ignore_errors=True)


def generate(call, out: Path, run_dir: Path) -> tuple[int, list[str]]:
    """At most two attempts of call(attempt) -> raw text. Writes `out` on the first valid answer.
    Returns (exit code, reasons for the failed attempts)."""
    reasons = []
    for attempt in (1, 2):
        try:
            raw = call(attempt)
        except Exception as e:                           # noqa: BLE001 -- recorded; re-run once
            (run_dir / f"a{attempt}.err").write_text(str(e))
            reasons.append(f"attempt {attempt}: {str(e)[:200]}")
            continue
        (run_dir / f"a{attempt}.txt").write_text(raw)
        objs = rl.gs.parse(raw)
        why = check_texts(objs)
        if why is None:
            by_id = {o["id"]: o["text"] for o in objs}
            out.write_text("".join(json.dumps({"id": i, "text": by_id[i]}, ensure_ascii=False) + "\n" for i in IDS))
            return 0, reasons
        reasons.append(f"attempt {attempt}: {why}")
    return 4, reasons


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if ct.CODEX_CLEAN.exists():
        raise SystemExit(f"refused: {ct.CODEX_CLEAN.name} exists; the Codex clean texts are generated once")
    files = {"codex-clean-prompt.md": PROMPT.read_text(), "menu.json": menu_text()}
    header = dict(codex=f"{rl.gs.CODEX_MODEL}/{rl.gs.CODEX_EFFORT}", directive=DIRECTIVE,
                  sha256={n: hashlib.sha256(t.encode()).hexdigest() for n, t in files.items()},
                  started=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if args.dry_run:
        print(json.dumps(header, indent=1))
        return 0
    RUN_DIR.mkdir(exist_ok=True)
    rc, reasons = generate(lambda a: codex_call(files, rl.new_codex_home(), RUN_DIR / f"a{a}.codex.log"),
                           ct.CODEX_CLEAN, RUN_DIR)
    header.update(failed_attempts=reasons, ended=datetime.datetime.now(datetime.timezone.utc).isoformat())
    (RUN_DIR / "header.json").write_text(json.dumps(header, indent=1) + "\n")
    if rc:
        print("STOP: the Codex clean texts failed twice: " + "; ".join(reasons), file=sys.stderr)
        return rc
    for i, text in ct.codex_clean():
        print(f"{i}: {text}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
