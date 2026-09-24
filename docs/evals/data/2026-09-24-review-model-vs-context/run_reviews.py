"""Run the model x context review cells (registration: docs/evals/review-model-vs-context-2026-09-24.md).

Each run: a fresh detached worktree at the stimulus commit, one reviewer CLI on its
subscription with a clean channel (no user instructions, no plugins/hooks, no MCP), the
brief (+ the author's own prior summary in PRIMED cells), and the final message saved.
No API key reaches either child: ANTHROPIC_* and OPENAI_API_KEY are stripped.
"""
from __future__ import annotations

import concurrent.futures as cf
import json
import os
import pathlib
import random
import subprocess
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path("/home/marius/work/claude/codescout")
WT = pathlib.Path("/home/marius/work/claude/codescout.worktrees")
CFG = pathlib.Path(os.environ["RMVC_SCRATCH"])          # holds claude-cfg/ and codex-home/
OUT = pathlib.Path(os.environ.get("RMVC_OUT", HERE / "runs"))
TIMEOUT = 2400
N = 3
SEED = 20260924

STIM = {
    "s1": {"sha": "a8835d06b41dd6901943a796ad2b18aa8abad933",
           "scope": ("the rule-tell evaluation campaign: docs/evals/rule-tell-scoring-2026-09-23.md, "
                     "docs/evals/phase1-local-classifier-preregistration.md, "
                     "docs/evals/rule-injection-timing-preregistration.md, "
                     "scripts/phase1-span-selector.py, scripts/phase1-local-l0.py, "
                     "scripts/phase1-rule-selection.py, scripts/phase2-score-dp1.py, and the committed "
                     "data in docs/evals/data/2026-09-24-rule-tell/"),
           "primed": HERE / "primed-s1.md"},
    "s2": {"sha": "898d3ea37e16e31b0dc76f77e0ed631dcbe7a9c8",
           "scope": ("the most recent work on the rule-tell evaluation campaign, committed since "
                     "9c0d2505: the S0 form 4 and form 4q results and the Stage 2 mined-pair candidate "
                     "build. Files: docs/evals/rule-tell-scoring-2026-09-23.md, "
                     "docs/evals/phase1-local-classifier-preregistration.md, "
                     "scripts/phase1-span-selector.py, scripts/phase2-score-dp1.py, tests/, and "
                     "docs/evals/data/2026-09-24-rule-tell/ (including its stage2/ directory)"),
           "primed": HERE / "primed-s2.md"},
}

BRIEF = """You are reviewing work in this repository, a git checkout at commit {sha}.

Review scope: {scope}.

Your job: find defects -- in the code, in the data files, and in any claim the documents make -- that would make a published number, a conclusion, or a planned next step wrong. Check claims against the committed data yourself; do not rely on the documents' own summaries.
{primed}
Rules: do not edit, create or delete files inside this checkout, and do not run git commands that change it; write scratch files only under /tmp/rv-{run}/. Make no calls to any language model, network API or web service. Do not run cargo or the Rust test suite.

When you are done, give your review as your final message: a numbered list of findings, each with (a) the defect in one sentence, (b) the evidence -- file:line, and/or the command you ran and what it returned, (c) its impact. Then list the checks you ran that came out clean. Stop when the scope is reviewed."""

PRIMED_INTRO = ("\nBelow is the author's own summary of this work, as they reported it to the operator "
                "before this review:\n\n<author_summary>\n{text}\n</author_summary>\n")


def env_without_keys(**extra):
    e = {k: v for k, v in os.environ.items()
         if k not in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "OPENAI_API_KEY")}
    e.update(extra)
    return e


def run_one(cell: dict) -> dict:
    rid = cell["id"]
    wt = WT / f"rmvc-{rid}"
    subprocess.run(["git", "-C", str(REPO), "worktree", "add", "--detach", str(wt), STIM[cell["stim"]]["sha"]],
                   capture_output=True, check=True)
    primed = PRIMED_INTRO.format(text=STIM[cell["stim"]]["primed"].read_text()) if cell["context"] == "primed" else ""
    brief = BRIEF.format(sha=STIM[cell["stim"]]["sha"][:8], scope=STIM[cell["stim"]]["scope"],
                         primed=primed, run=rid)
    (OUT / f"{rid}.brief.txt").write_text(brief)
    t0 = time.time()
    if cell["model"] == "claude":
        cmd = ["claude", "-p", brief, "--model", "claude-opus-5-5", "--dangerously-skip-permissions",
               "--strict-mcp-config", "--no-session-persistence", "--output-format", "json",
               "--disallowedTools", "WebFetch", "WebSearch", "Bash(git push:*)", "Bash(git commit:*)"]
        env = env_without_keys(CLAUDE_CONFIG_DIR=str(CFG / "claude-cfg"))
    else:
        cmd = ["codex", "exec", "--skip-git-repo-check", "--sandbox", "workspace-write",
               "-c", "approval_policy=never", "-C", str(wt), "--ephemeral",
               "-o", str(OUT / f"{rid}.codex-last.txt"), brief]
        env = env_without_keys(CODEX_HOME=str(CFG / "codex-home"))
    try:
        p = subprocess.run(cmd, cwd=wt, env=env, capture_output=True, text=True,
                           timeout=TIMEOUT, stdin=subprocess.DEVNULL)
        code, stdout, stderr = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired as e:
        code, stdout, stderr = "timeout", (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or ""), ""
    dur = round(time.time() - t0)
    review, model_seen, err = "", None, None
    if cell["model"] == "claude":
        try:
            d = json.loads(stdout)
            d = next((x for x in reversed(d) if x.get("type") == "result"), {}) if isinstance(d, list) else d
            review = d.get("result") or ""
            model_seen = list((d.get("modelUsage") or {}).keys())
            err = d.get("is_error")
        except Exception as ex:  # noqa: BLE001
            err = f"unparseable: {ex}"
    else:
        f = OUT / f"{rid}.codex-last.txt"
        review = f.read_text() if f.exists() else ""
        model_seen = [ln.split(":", 1)[1].strip() for ln in stdout.splitlines() + stderr.splitlines()
                      if ln.startswith("model:")][:1]
    (OUT / f"{rid}.review.md").write_text(review)
    (OUT / f"{rid}.log").write_text((stdout or "")[-20000:] + "\n--- stderr ---\n" + (stderr or "")[-5000:])
    dirty = subprocess.run(["git", "-C", str(wt), "status", "--porcelain"], capture_output=True, text=True).stdout
    subprocess.run(["git", "-C", str(REPO), "worktree", "remove", "--force", str(wt)], capture_output=True)
    meta = {**cell, "exit": code, "seconds": dur, "model_seen": model_seen, "is_error": err,
            "review_chars": len(review), "checkout_dirty": dirty.strip()[:500]}
    (OUT / f"{rid}.meta.json").write_text(json.dumps(meta, indent=1))
    print(json.dumps({k: meta[k] for k in ("id", "exit", "seconds", "review_chars", "model_seen")}), flush=True)
    return meta


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    cells = [{"stim": s, "model": m, "context": c, "rep": r, "id": f"{s}-{m}-{c}-{r}"}
             for s in STIM for m in ("claude", "codex") for c in ("cold", "primed") for r in range(N)]
    random.Random(SEED).shuffle(cells)
    only = sys.argv[1:]
    if only:
        cells = [c for c in cells if c["id"] in only]
    done = {p.name.split(".")[0] for p in OUT.glob("*.meta.json")}
    cells = [c for c in cells if c["id"] not in done]
    print(f"{len(cells)} runs to do", flush=True)
    with cf.ThreadPoolExecutor(4) as ex:
        list(ex.map(run_one, cells))
    return 0


if __name__ == "__main__":
    sys.exit(main())
