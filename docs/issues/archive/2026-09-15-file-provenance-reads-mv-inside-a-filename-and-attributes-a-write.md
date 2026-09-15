---
id: 76c83a43c2d1752f
kind: bug
status: fixed
title: 'BUG: file-provenance matches `mv` inside the filename `mv.rs`, attributing a write from read-only commands — and lifts an unrelated later path'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
- attribution
- file-provenance
- shared-checkout
topic: authorship attribution
closed: 2026-09-15
opened: 2026-09-15
severity: med
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

| `cmd` (⏎ = a newline inside one command body) | result |
|---|---|
| `git diff --stat -- …/mv.rs` | `['.rs']` |
| `cargo fmt --check -- …/mv.rs` ⏎ `git diff --stat -- …/mv.rs` | **`['src/librarian/tools/mv.rs']`** |
| `cargo fmt --check -- …/mv.rs` ⏎ `git diff --stat -- …/other.rs` | **`['src/…/other.rs']`** |
| `git diff --stat -- …/update.rs` ⏎ `git diff --stat -- …/second.rs` | `[]` |
| `mv a.txt b.txt` | `['a.txt', 'b.txt']` |

**Row 2 reproduces the reported symptom, and this file's FIRST FILING DID NOT CONTAIN IT.**
Rows 1 and 3 are both real and neither yields *"mv.rs is attributed to its reader"*: row 1
yields the fragment `.rs`, which resolves to no file, and row 3 yields a third file named by
neither command's intent. It takes **two** `--`-bearing reads in one command body — the
second supplying the `--` that the first verb's overrunning tail honours — for the path
*under investigation* to be recorded as written by whoever is investigating it.

That is the self-pollution loop stated exactly, and more narrowly than the first filing
claimed: one read is inert; asking the question twice answers it wrongly.

Rows 4 and 5 are the controls that make the rest a measurement — a filename carrying no verb
substring yields nothing, and a real relocation still yields both operands, so the function
is not simply broken.

## Root cause

`scripts/file-provenance.py`, the `RELOCATORS` pattern — cited by name and not by line,
because the fix below moved it and a line ref would already point at a comment:

```python
RELOCATORS = re.compile(
    r"\b(git\s+mv|git\s+checkout|git\s+restore|mv|cp|install)\b([^;&|]*)")
```

and `_operands()`, which truncates to whatever follows the **first** `--` in the captured
tail. The captured tail is unbounded by line, so that `--` may belong to a different command
entirely.

The comment directly above the regex shows the author reasoning carefully about operand
**position** (*"`cp a b` writes only b, while `mv a b` writes b AND empties a"*) — the
analysis is about *which operand*, and never about whether the verb matched a verb.

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

**FIXED 2026-09-15** in `b59a035d` — patch-id `574120f8573a2f60aa00f6fd64d312d75b3bbbf7`.

Both prescribed halves shipped. The `RELOCATORS` pattern in `scripts/file-provenance.py`
now reads:

```python
RELOCATORS = re.compile(
    r"\b(git\s+mv|git\s+checkout|git\s+restore|mv|cp|install)(?=\s)([^;&|\n]*)")
```

1. **`(?=\s)` replaces the trailing `\b`.** A word boundary holds between `v` and `.`, so
   `\bmv\b` matched inside the filename. A real invocation always has whitespace before its
   operands, so this removes the entire mention class at no legitimate cost.
2. **`\n` joins the negated class.** The operand tail now ends with its own command, so
   `_operands()` can no longer honour a `--` belonging to a later one.

**Verified on the LIVE corpus with both regexes run over the same transcripts at the same
instant** — the only form that separates a real improvement from transcript churn, since
this tool's input changes continuously. `--all src/librarian/tools/mv.rs` named **15**
authors under the old pattern and **12** under the new one, at 2026-09-15T17:18+03:00.

The diff is **purely subtractive**: `2cb44cd3`, `63083c9e` and `aa272bed` — the last being
the session this bug reported by name — with **nothing added**. That direction is the claim
worth making: a fix to an over-matching regex can trade a false positive for a false
negative, which this file's own Resume section warned about, and no count of green
assertions would have shown it.

## Tests added

`tests/file-provenance.sh`, inside its existing *relocating verbs* section — **nine
assertions across four fixtures**, taking the suite 120 → 129. CI already runs this file
(`.github/workflows/ci.yml`), so no wiring was owed.

**Correction to this file's first filing.** It stated the harness "currently has nothing" of
the needed shape. That was wrong, and checkable in one `grep`: the section these belong in
already existed, with positive controls for `cp`, `mv`, `git mv` and `git checkout`
destinations *and* sources. What was absent was only the discrimination against a **mention**.

**And the test pair this file PRESCRIBED would have guarded neither half of the fix it
prescribed.** Measured by mutating each bound separately in an isolated copy, control
129/0 both before and after:

| mutation | the prescribed pair | the shipped suite |
|---|---|---|
| drop `(?=\s)`, keep `\n` | **SURVIVED** | KILLED (2 assertions) |
| keep `(?=\s)`, drop `\n` | **SURVIVED** | KILLED (3 assertions) |
| drop both — the original | KILLED | KILLED (9 assertions) |

Either bound alone satisfies the prescribed assertions, because each independently prevents
the *composite* failure they describe. So that pair guards the **conjunction** and neither
**site** — `mutate once per guarded SITE, not once per feature`, met in the wild, and
invisible to TDD: the red was observed with both bounds absent, which is the one
configuration that cannot distinguish them.

The two cases that close it are each shaped so the *other* bound cannot rescue the failure:

- **isolating `(?=\s)`** — one line, one command, and a real path standing *beside* the
  verb-shaped filename, so there is no line end for the `\n` bound to stop at;
- **isolating `\n`** — a **real** `mv`, so the lookahead is satisfied and irrelevant,
  followed by another command carrying `--`. Both directions are asserted here, because the
  unbounded tail does not merely *add* the victim: it **replaces** the `mv`'s own operands
  with it, so a false negative cannot hide behind a passing false positive.

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

Nothing owed on the filed defect. Two **adjacent** defects were found in the same code path
while fixing it, both surviving this fix, both filed separately rather than folded in:

- `cargo install ripgrep` → `['ripgrep']` and `apt install vim` → `['vim']`. The verb sits at
  a command position but belongs to *another program* as a subcommand.
- A **heredoc body** is scanned as command text. A probe-shaped heredoc yielded
  `['src/alpha.rs', 'src/beta.rs")]']` — lifting Python syntax along with the path.

Both share a root this fix does not address: `RELOCATORS` has no notion of **command
position**, only of token shape. Requiring the verb to be command-initial would answer the
first and most of the second, and is a larger change than the two bounds shipped here.
