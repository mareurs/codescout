---
id: 8f400a7f3a988172
kind: bug
status: fixed
title: 'BUG: file-provenance.py reads a workspace project id as a path, so relative writes after an id activation are credited to a directory that does not exist'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-01
opened: 2026-10-01
severity: low
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
- *Can a phantom collide with a real file?* **Reproduced 2026-10-01.** With an id `proj-real` whose `<active>/<id>` is a real directory holding `src/lib.rs`, the old script credited that REAL file to a session that activated the id and wrote `src/lib.rs` relatively. That is worse than the phantom path: it names a file someone may own.

## Fix

Direction 1 and the path half of direction 2, chosen 2026-10-01; direction 3 rejected on evidence.

- After an id activation the active tree is `UNKNOWN_TREE`, a sentinel compared by identity and
  never joined or resolved. A relative codescout write made while the base is that sentinel is
  credited to nobody, which is what `fmt-mine` refuses on. A relative path taken off an unknown
  tree stays unknown; an absolute activation, or a call pinned to an absolute `workspace=`,
  restores a known tree. The id test is the server's own rule (no separator means an id), with
  `.` and `..` carved out as paths.
- `session_activations` resolves a relative PATH activation against the checkout root instead of
  the script's cwd, so activating `.` is no longer read as a move when the script runs from a
  subdirectory.
- **Why not read the manifest (direction 3).** It cannot resolve the id this bug is about: this
  repository's `.codescout/workspace.toml` declares one `[[project]]`, `codescout`, while
  `codescout-embed` is auto-discovered at runtime and absent from it, and the file is gitignored
  and per machine. An id the script cannot resolve is unknowable, not guessed.

**What this does not change, stated so the next reader does not assume otherwise.** The real file
after an id activation still reads UNKNOWN, as it did, and so does a subagent write after the
parent activated the home project by id (case C): both need the id resolved, which this script
cannot do. The fix removes the phantom `MINE` and the cwd-dependent misread, not that loss of
recall. A bare relative directory name (`docs`) is read as an id too, because the server reads it
that way.

## Tests added

In `tests/file-provenance.sh`, section *a workspace project id is not a path*, each id case with an
absolute-path control. A helper imports the script and prints EVERY path `scan()` credits to a
session, so "credited nowhere" is an exact assertion: the first draft asserted on the old phantom
path alone, which a fix that merely renamed the garbage path would pass. Cases: id then relative
write (phantom and exact set); absolute activation of the same project (credited, exactly one path);
absolute after id (recovers); an absolute `workspace=` pin after an id (honoured); a relative path
activation off an unknown tree; a subagent write after the parent's id activation;
`run_command cwd=` after an id; an id naming a real directory (the previously derived case,
reproduced); and `activate "."` with the script run from a subdirectory.

Red against the script as it stood: 5 failures, then 6 with the real-directory case. 196 passed and
0 failed after. The suite had no `eq` helper, so seven new assertions first errored with `command not
found` and the run carried on; the total not growing is what showed it. Eight mutations on the final
bytes, one per site, each killed by its own assertions: the id predicate, its `.` carve-out, unknown
propagation to a relative path, the scan-level skip, the pin override, the `run_command cwd=` join,
the checkout-root argument, and the absolute short-circuit. A ninth clause I had written,
`target is UNKNOWN_TREE` in `session_activations`, was inert (no input distinguishes it from the
existing comparison) and was deleted rather than tested.

The other consumers' suites are unchanged: `attribute-red` 41/0, `fmt-mine` 50/0, `git-safe-reset`
35/0. The suite runs in CI as its own job; the local four-command gate does not.

## Fix provenance

- **SHA:** `43bf4053af506cf71701b04d10ad0733ca544b8d` (`experiments`)
- **patch-id:** `32251b655253f9539dfbd4a0011be25967f5fcb5` (`git show <sha> | git patch-id --stable`)

A follow-up commit, `f21be9895c3c8992974e3ccfa623916934d11d85`, adds the real-directory case to the
tests. Verified on `experiments` 2026-10-01 by running `tests/file-provenance.sh` directly (196
passed, 0 failed) and the mutations above. No Rust changed, so the four-command gate was not run.

## Workarounds
Activate with an absolute path, or pin a single call with `workspace=<absolute path>`; `write_base` reads that as a path, which it is.

## Resume

Closed.

## References
- PR #29's independent review, note 1, which found the first call site and called the behaviour "fail safe"; this file adds the second call site and the phantom `MINE`.
- `docs/issues/archive/2026-09-24-file-provenance-reads-no-transcripts-inside-a-worktree.md`: the sibling bug in the same script, which introduced `activated_tree`.
