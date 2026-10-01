---
id: '046f3106e9d7baaf'
kind: bug
status: open
title: 'BUG: file-provenance.py reads a workspace project id as a path, so relative writes after an id activation are credited to a directory that does not exist'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-10-01
severity: low
unverified: 'STANDING: the misattribution of a REAL file (an id whose `<active>/<id>` is a real directory) is derived by reading, not reproduced. Everything in the table was measured.'
---

## Summary
`activated_tree` in `scripts/file-provenance.py` reads the `path` of a `workspace(action="activate")` call as a filesystem path. The tool also accepts a **workspace project id** there (its own schema: "project path or workspace project id"), and an id is not a path. After an id activation the script's idea of "the active tree" is `<active>/<id>`, a directory that usually does not exist, so every later relative codescout write is attributed to a phantom path. A second call site resolves the same value against the script's own working directory.

## Symptom (Effect)
Measured 2026-10-01 by running the real tool against throwaway repos and synthetic transcripts (recipe below). Each BUG row has a CONTROL that differs only in how the activation is spelled:

| case | activation | relative write | verdict for the REAL file | verdict for the phantom path |
|---|---|---|---|---|
| A control | absolute path of the project root | `src/lib.rs` | `MINE` | n/a |
| A bug | project id `codescout-embed` (root `crates/codescout-embed`) | `src/lib.rs` | **`UNKNOWN`** | `codescout-embed/src/lib.rs`: **`MINE`**, a path that does not exist |
| B control | none | `src/main.rs` | `MINE` | n/a |
| B bug | home project id `codescout` (root `.`) | `src/main.rs` | **`UNKNOWN`** | `codescout/src/main.rs`: **`MINE`**, a path that does not exist |
| C control | none, write comes from a SUBAGENT of the session | `src/main.rs` | `MINE` | n/a |
| C bug | home project id `codescout` by the parent, write from a SUBAGENT | `src/main.rs` | **`UNKNOWN`** | n/a |

`UNKNOWN` is the safe direction for `fmt-mine.sh`, which acts on the answer: it refuses instead of formatting. But it means a session that activated by id is told its own files have no owner, and the gate's first step refuses them without saying why. The phantom `MINE` rows are a positive claim about a file that is not there.

## Reproduction
The premise (that codescout really moves the active project to the member's ROOT, not to `<active>/<id>`) was checked on the live server: `workspace(action="activate", path="codescout-embed")` returned `project_root: .../codescout/crates/codescout-embed`, and the response's own banner read "paths are relative to .../crates/codescout-embed". The home project was restored right after.

Then, with `$TOOL` = `scripts/file-provenance.py`, in a throwaway repo (git init, two tracked files `src/main.rs` and `crates/codescout-embed/src/lib.rs`, both made dirty) and a fake `HOME` holding the transcript under `.claude-t/projects/<repo path with every non-alphanumeric byte replaced by "-">/<session>.jsonl` (the fixture shape of `tests/file-provenance.sh`):
```python
# one record per tool call, as real transcripts carry them
rec = lambda name, inp, sid=ME: {"type": "assistant", "cwd": MAIN, "sessionId": sid,
        "message": {"content": [{"type": "tool_use", "name": name, "input": inp}]}}
# case A-bug: activate by id, then write relatively
write_jsonl(session_file, [rec("mcp__codescout__workspace", {"action": "activate", "path": "codescout-embed"}),
                           rec("mcp__codescout__create_file", {"path": "src/lib.rs", "content": "x"})])
# ask (cwd = the repo, HOME = the fake one, CLAUDE_CODE_SESSION_ID = ME):
#   python3 $TOOL crates/codescout-embed/src/lib.rs   -> UNKNOWN
#   python3 $TOOL codescout-embed/src/lib.rs          -> MINE   (the phantom)
# case C-bug: put the create_file record in <session>/subagents/agent-1.jsonl (same sessionId) and only
# the activation of the HOME id "codescout" in the parent file; ask for src/main.rs -> UNKNOWN,
# while the same layout with NO activation in the parent -> MINE.
```

## Environment
`scripts/file-provenance.py` at `origin/experiments` (`6115958f`); `python3` on Linux; workspace of two projects, `codescout` (root `.`) and `codescout-embed` (root `crates/codescout-embed`).

## Root cause
One value, `inp["path"]`, is in a namespace with two kinds of names (a filesystem path, or a workspace project id) and `activated_tree` models one of them. Three consequences, at two call sites:
1. **`scan()` passes the running `active` tree.** `activated_tree` computes `active / Path(id)`. For an id that differs from its root (`codescout-embed` is at `crates/codescout-embed`, the home project `codescout` is at `.`) that names a directory that is not there. Every later relative codescout write is credited to `<phantom>/<path>`, and the real file has no owner.
2. **`session_activations()` passes `None` as `active`.** A relative `Path(id)` is then resolved by `target.resolve()` against the SCRIPT's working directory, not the checkout root, and compared with the checkout root. It never matches for a project id, so `parent_activated` is set even when the id was the home project, a no-op. `scan()` then treats a subagent's relative writes as unknowable and credits nobody (case C). The same defect would apply to a relative PATH, not only to an id.
3. **The docstring claims more than the code does.** It says "a wrong base only ever mis-files a relative path between two trees -- it never invents a write". Cases A and B credit a write to a path that exists in no tree.

It works by accident when the id equals the project's root directory name directly under the active root (`<root>/<id>` then really is the project root). Nothing in the code records that as the assumption.

## Evidence
The table above; the live activation response quoted in Reproduction; the two call sites in `scripts/file-provenance.py` (`scan` and `session_activations`) and the body of `activated_tree`. The codescout workspace-state guide states the server's own disambiguation rule: a bare token with no `/` is a project id (a focus switch), anything else is a path.

## Hypotheses tried
- *Is it only the first call site?* No: case C isolates the second. The subagent file carries no activation of its own, so `scan()`'s base is correct there and the `UNKNOWN` can only come from `parent_activated`.
- *Does "fails safe" hold?* Partly. The real file reads `UNKNOWN`, the direction `fmt-mine.sh` refuses on. The phantom `MINE` and the over-reach in case C are not "mis-filing between two trees".
- *Can a phantom collide with a real file?* If an id's `<active>/<id>` happens to be a real directory holding a real file, that file would be credited to a session that never wrote it. Derived by reading, **not reproduced**: no such layout exists in this repo.

## Fix
Open. Directions, none tried:
1. **Mirror the server's rule.** No `/` in the value means a project id; otherwise a path. An id cannot be resolved without the workspace manifest, which is gitignored and per machine, so the script must degrade, not guess: treat an id activation as **unknowable**. In `scan()` that means relative codescout writes after it are skipped, exactly as the existing `unknowable` branch skips them, until a later absolute activation. In `session_activations()` it means an id activation must not set `parent_activated` from a `resolve()` against the wrong directory.
2. **Pass the checkout root, not `None`,** as `active` in `session_activations()`, so a relative PATH is at least resolved against the right tree. Fixes the path half of consequence 2 only.
3. **Read the manifest when it exists** and resolve the id to its root, falling back to (1). Adds a per-machine dependency to a script whose point is to work on any clone.
Whatever is chosen needs both directions in the test: each id case above with its absolute-path control, and a bare-token path that IS a real directory.

## Tests added
N/A: open, no fix written. A fix belongs in `tests/file-provenance.sh` next to the worktree-activation cases, using the fixture helpers it already has.

## Workarounds
Activate with an absolute path, or pin a single call with `workspace=<absolute path>`; `write_base` reads that as a path, which it is.

## Resume
Pick direction 1 or 3 and write the cases from the table first (the id cases should fail before the fix). Re-run the reproduction on the then-current script, because the same file is edited by other sessions.

## References
- PR #29's independent review, note 1, which found the first call site and called the behaviour "fail safe"; this file adds the second call site and the phantom `MINE`.
- `docs/issues/archive/2026-09-24-file-provenance-reads-no-transcripts-inside-a-worktree.md`: the sibling bug in the same script, which introduced `activated_tree`.
