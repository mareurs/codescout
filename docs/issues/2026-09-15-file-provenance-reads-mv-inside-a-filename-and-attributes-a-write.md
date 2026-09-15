---
id: d725fa54d387d55d
kind: bug
status: open
title: 'BUG: file-provenance matches `mv` inside the filename `mv.rs`, attributing a write from read-only commands — and lifts an unrelated later path'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
- attribution
- file-provenance
- shared-checkout
topic: authorship attribution
---

## Summary

`scripts/file-provenance.py` attributes a **write** from read-only commands, and the
attributed path need not be the one that triggered it. Two compounding defects in one
regex:

1. **No escape for MENTION.** `RELOCATORS` carries a bare `mv` alternative bounded by `\b`,
   and `\b` holds between `v` and `.` — so `mv` matches *inside the filename* `mv.rs`. Any
   file named after a relocating verb (`mv.rs`, `cp.rs`, `install.rs`) is attributed as
   written by anyone who merely names it in a shell command.
2. **The operand capture spans commands.** `([^;&|]*)` is a negated class, so it matches
   **newlines** and runs past the end of the triggering command; `_operands()` then honours
   a `--` belonging to a **later, unrelated** command and lifts that command's path.

So a session that runs `cargo fmt --check -- …/mv.rs` is recorded as having written a file
it only read, and a session that runs that line above any other `-- <path>` command is
recorded as having written **that other path**.

## Symptom (Effect)

The `run_command` hook that attaches authorship to a build red names the wrong session, with
the standard *"Another session is holding at least one of these. Ask before editing"*. The
reader's next action is to message a busy peer about a file they do not hold — or, worse, to
not edit their own file.

Observed 2026-09-15: a red from a test this session had just written named
`src/librarian/tools/mv.rs` as *"written by aa272bed-… [LIVE]"*. `git diff --stat` on that
file showed **one hunk, 66 insertions, all mine**; the named peer's worktree was clean and
no commit of theirs had ever touched it.

## Reproduction

Against the production function, via `importlib`, with controls:

```python
spec = importlib.util.spec_from_file_location('fp', 'scripts/file-provenance.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
list(m.relocation_targets(cmd))
```

| `cmd` | result |
|---|---|
| `git diff --stat -- src/librarian/tools/mv.rs` | `['.rs']` |
| `cargo fmt --check -- src/librarian/tools/mv.rs\ngit diff --stat -- src/foo.rs` | **`['src/foo.rs']`** |
| `git diff --stat -- src/librarian/tools/update.rs\ngit diff --stat -- src/bar.rs` | `[]` |
| `mv a.txt b.txt` | `['a.txt', 'b.txt']` |

Row 2 is the one that matters: a `--check` and a `--stat`, both read-only, and the recorded
write is a **third file** named in neither's intent. Rows 3 and 4 are the controls that make
rows 1–2 a measurement — a filename without a verb substring yields nothing, and a real
relocation still yields its operands correctly, so the function is not simply broken.

## Root cause

`scripts/file-provenance.py:199`:

```python
RELOCATORS = re.compile(
    r"\b(git\s+mv|git\s+checkout|git\s+restore|mv|cp|install)\b([^;&|]*)")
```

and `_operands()`, which truncates to whatever follows the **first** `--` in the captured
tail. The captured tail is unbounded by line, so that `--` may belong to a different command
entirely.

The comment directly above the regex shows the author reasoning carefully about operand
**position** (*"`cp a b` writes only b, while `mv a b` writes b AND empties a"*) — the
analysis is about which operand, and never about whether the verb matched a verb.

## Evidence

The mechanism was found by sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`, who audited
their own transcript through the tool's own `write_targets()` and found **5** matching
records, **every one a read** — two `git diff --stat`, one `cargo fmt --check`, two
`file-provenance.py` invocations. They also refuted the first hypothesis (this session's):
that a `mutation-probe.sh` run had written the file in an isolated worktree. It had not, and
cannot — the probe writes through `Path(sys.argv[1])`, never a literal, so it is invisible to
this tool by construction.

**The compounding property, and the reason this is worse than a false positive.** Four of
those five records **did not exist before the question was asked.** Every command run to
investigate *"did I write mv.rs?"* — `git diff --stat -- …/mv.rs`, `file-provenance.py …` —
adds another record asserting that you wrote `mv.rs`. **The instrument writes into what it
measures, and its confidence in the wrong answer grows monotonically with investigation.**
Whoever re-runs this next sees a larger count than the last reader and reads the increase as
corroboration. (Reconnaissance seam class *"an instrument that writes into the corpus it
measures"*, `codescout:R-51`, reached here through a different subsystem.)

## Hypotheses tried

1. **A `mutation-probe.sh` run in the peer's isolated worktree.** **Refuted** by
   `aa272bed`, mechanically rather than by inspection: the probe's write goes through
   `Path(sys.argv[1])`, which none of the tool's literal-binding patterns can match. This
   was this session's hypothesis and it was wrong.
2. **Stale transcript data.** **Refuted** — the records are real and current; they are
   correctly recording commands that genuinely ran. The tool is not lagging, it is
   misclassifying.

## Fix

Not attempted. The two halves are independent and the first is small:

1. **Require a word boundary that a filename cannot satisfy.** The verb must be followed by
   whitespace, not merely a non-word character — `(?:mv|cp|install)(?=\s)` rather than
   `…\b`. This removes the `mv.rs` class of false positive entirely and cannot affect a real
   invocation, which always has a space before its operands.
2. **Bound the operand capture to one command.** `([^;&|\n]*)` stops the tail at the line
   end, so `_operands()` can no longer honour a later command's `--`. Worth doing even if
   (1) lands, because any future verb-shaped token reaches the same code.

## Tests added

None. The shape a guard needs, and note that it is **cheap and this file currently has
nothing of it**: assert `relocation_targets()` returns `[]` for a read-only command naming a
verb-shaped filename, and assert it still returns both operands for a real `mv a b`. The
pair is what makes it a discrimination rather than a suppression — tightening the regex
until it matches nothing would pass the first assertion alone.

## Workarounds

Read the hook's attribution as **"this sid's transcript mentions this path"**, never as
"this sid wrote this path". To decide whether a peer holds a file, use the positive
instruments: `git diff --stat -- <path>` for the shared checkout, `git status --porcelain`
in their worktree, and the `Session-Id` trailer for committed work.

## Classification

`cluster/addressing-without-an-escape-hatch` (`IC-6`), the **no-escape-for-mention** half —
a namespace (shell command text) in which a token cannot be *named* without being
*interpreted*, and where the corpus contains a file whose name is a verb. Same shape as
`an-entry-id-cannot-be-mentioned-without-citing-it`, reached through a shell parser rather
than a citation parser.

## Resume

Fix (1) is a two-character change with a large blast radius reduction. Before shipping it,
run the controls above — a real `mv a b` must still yield both operands, or the fix trades a
false positive for a false negative in an instrument whose whole job is attribution.
