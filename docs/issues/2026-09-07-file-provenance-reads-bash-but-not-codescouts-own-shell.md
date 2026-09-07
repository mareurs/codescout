---
id: '2b9cbd34630cf340'
kind: bug
status: open
title: 'BUG: file-provenance.py scans Bash but not run_command, so every write through codescout''s own shell is invisible to the instrument built to attribute it'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- shared-checkout
- provenance
- multi-session
topic: the working-tree provenance channel and what it cannot see
---

## Summary

`scripts/file-provenance.py` answers *"is this working-tree file mine?"* by scanning Claude Code
transcripts for tool calls whose input names a write target. Its dispatch has four branches —
`CS_WRITE_TOOLS`, the artifact tools, `NATIVE_WRITE_TOOLS`, and `elif name == "Bash"`.
**`mcp__codescout__run_command` is in none of them**, and the string does not occur anywhere in
the file (`grep -c run_command` → **0**).

So a session that does its shell work through codescout's own shell is invisible to the
instrument built to attribute it. This is not the documented residual — the script's own header
measures a **2.8%** miss rate for Bash commands carrying a mutating verb its heuristics do not
match. This is a **whole tool**, and `CLAUDE.md` § *Companion Plugin* states that
`run_command` and native `Bash` are **both deliberately permitted right now** pending an eval.
Which of the two a session happens to use decides whether its writes are attributable.

## Symptom (Effect)

Reproduced 2026-09-07 against a file whose author is known with certainty, because the author is
the reporter:

```
$ ./scripts/file-provenance.py --since 2026-04-24T08:17:07+03:00 src/agent/write_guard.rs
UNKNOWN   src/agent/write_guard.rs
          no record of any session writing this path in the window. That is a statement about
          coverage, NOT about ownership — Bash writes this tool's heuristics miss look
          identical. Do not read it as 'not mine'.
```

Every edit to that file in that window was mine, made this session, and committed by me at
`d1b6146d`. The window floor is the file's previous commit, which is the tool's own default.

**The tool is behaving correctly and its caveat is exactly right.** `UNKNOWN` is the safe
direction, it is never rendered as "not mine", and it prints its own coverage warning. Nothing
here is a false attribution. The defect is the size of the hole, and that the hole is
undocumented — a reader who has read the header believes the miss rate is 2.8%.

## Root cause

`write_targets`' dispatch keys on the tool NAME, and the shell branch names one shell:

```python
elif name == "Bash":
    cmd = inp.get("command")
    ...
```

`BASH_WRITE_PATTERNS`, `relocation_targets` and `python_write_targets` are all reached only from
inside that branch. They are good — `sed -i`, `tee`, `>` / `>>`, and `python -c` write literals
are all matched — and **none of them is consulted for a `run_command` call**, whose payload is
also `command`. The parsing half is right; the selection half never offers it the input.

## Evidence

The composition is what makes this expensive rather than untidy, and it is verifiable rather than
supposed. This session wrote `src/agent/write_guard.rs` entirely through
`run_command` — `python3 - <<'PY'` heredocs and one `sed -i` — because
`docs/issues/2026-09-07-the-source-redirect-names-tools-a-partial-tool-set-does-not-have.md`
left it no other route: the companion hook denies native `Read`/`Edit` on source files and
redirects to `read_file` / `edit_file` / `edit_code`, none of which this session's tool set
contains.

**So one bug systematically pushes writes into the other's blind spot.** Any session with a
partial tool set is forced to the shell, and if it reaches for codescout's shell — which every
Iron Law in this repo tells it to prefer — its writes become unattributable. The two were filed
independently and compose.

Measured cost the same morning, reported by `codescout-98` (sid unrecorded here; the report came
by peer message): an unattributable red gate. Three of my tests and six `cargo fmt --check` hunks
in `src/agent/write_guard.rs` reddened their gate run while my `acquire()` signature change was
mid-flight. They ran the instrument, got `UNKNOWN`, and **correctly declined to guess** — which
is the first recorded instance of a party believing this verdict instead of substituting a proxy.
`docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` records three
parties producing three wrong answers on one evening; this one produced none. The instrument's
design worked. Its coverage did not.

## Fix

Not fixed. The change is small and the decision is not:

- **Route `run_command` through the same branch.** Its payload key is `command`, identical to
  `Bash`, so `elif name in ("Bash", "mcp__codescout__run_command")` reuses every existing
  pattern. Cheapest, and it closes the hole this file is about.
- **Then re-measure the residual.** The header's **2.8%** is a figure about `Bash` calls in this
  project. It is not a figure about `run_command` calls and must not be reported as one after the
  branch widens — `run_command`'s usage profile is different (it is the recommended shell, so it
  carries proportionally more of the mechanical work). Derive it again; do not carry it over.
- **Do not narrow the caveat text.** `UNKNOWN`'s warning is what kept this from becoming a
  misattribution, and it should keep saying coverage rather than ownership even once coverage
  improves.

## Tests added

None yet. `tests/file-provenance.sh` has 43 cases. The regression to add is a transcript fixture
carrying a `mcp__codescout__run_command` call with a `sed -i` or heredoc write, asserting the
verdict line names the session — and the assertion must be on the **verdict**, not on the
presence of the path, because the existing suite already asserts on verdict lines precisely so a
caveat's wording cannot satisfy a check.

**And pair it with a control**: the same fixture under `Bash` must also resolve, or a green run
proves only that the fixture is unreachable.

## Workarounds

Ask the session. That is what `2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`
already prescribes for uncommitted state, and it remains the only positive instrument — a session
can quote its own sessionId from its scratchpad path. Once the work is committed the `Session-Id`
trailer answers it exactly.

## Resume

Widen the branch, re-derive the residual as its own number with its own unit, add the fixture plus
its control.

## References

- `scripts/file-provenance.py` — `write_targets`' dispatch; `CS_WRITE_TOOLS`;
  `BASH_WRITE_PATTERNS`.
- `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` — the bug
  this instrument was built for; its § *Implementation* documents the 2.8% residual this file
  narrows.
- `docs/issues/2026-09-07-the-source-redirect-names-tools-a-partial-tool-set-does-not-have.md` —
  the bug that forces writes into this blind spot.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- Found by sessionId `4eac25ba-b181-4dac-a5a1-ec88502a5bc5`, after a peer's gate went red on this
  session's uncommitted work and the instrument declined to name it.

