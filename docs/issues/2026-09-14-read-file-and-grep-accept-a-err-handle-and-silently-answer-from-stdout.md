---
id: fc08bf52ac6b478d
kind: bug
status: taken
title: 'BUG: read_file and grep accept a .err buffer handle and silently answer from stdout'
tags:
- cluster/addressing-without-an-escape-hatch
- output-buffer
- run-command
- progressive-disclosure
---

## Summary

A buffer handle may carry a **`.err` suffix** selecting the stderr stream.
`OutputBuffer::get_with_refresh_flag` does `id.strip_suffix(".err")` to **resolve** the entry
and returns the whole `BufferEntry`, leaving **stream selection** to each caller.

`read_file` and `grep` never do the selection. They **accept** the token, **resolve** it, and
serve **stdout** — no error, no warning.

```
run_command("grep -c MARKER @cmd_x.err")   ->  1              correct
grep(pattern="MARKER", path="@cmd_x.err")  ->  4000: stdout line 4000
read_file("@cmd_x.err")                    ->  4001 lines of stdout
```

**`grep` is the worst of the three**, and the reason is the line number. Given an alternation
of a stderr token and a stdout token it matched the stdout one and reported `4000:`. The reader
gets a **confident, well-formed, wrong citation** — not a miss they can notice, but a hit they
can act on. `IC-6`'s *no disambiguator* half exactly: the token is accepted and silently
resolves to the wrong target.

## Symptom (Effect)

A reader checking whether a command's stderr contains something gets `0`, or worse a match from
the other stream, and has no signal that the question was answered about a different stream.

## Reproduction

2026-09-14, `experiments` at `9b6f4713`. A script emitting 4000 stdout lines plus one marked
stderr line, buffered, then queried three ways on the same handle:

| caller | `@cmd_x` | `@cmd_x.err` |
|---|---|---|
| `run_command` interpolation | stdout | **stderr — correct** |
| `read_file("@cmd_x.err")` | stdout | **stdout, silently** |
| `grep(pattern, path="@cmd_x.err")` | stdout | **stdout, silently, with a line number** |

Independently reproduced on a separate handle by sessionId
`aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66` before accepting the report.

## Root cause

**`BufferEntry` exposes two streams and no policy, so every consumer invented one.** Verified by
enumerating the resolvers rather than by reading the two known ones — the population is **five**,
holding **four** policies:

| site | policy |
|---|---|
| `src/tools/output_buffer.rs` interpolation | selects on the `.err` suffix — correct |
| `src/tools/run_command/output.rs` | always attaches `e.stderr` as its own field |
| `src/tools/read_file.rs` `read_from_buffer` | `.stdout` unconditionally — **defect** |
| `src/tools/grep.rs` buffer branch | `.stdout` unconditionally — **defect** |
| `src/peer/server.rs` | concatenates `stdout\nstderr` — a deliberate third policy |

`src/tools/markdown/read_markdown.rs` is a sixth resolver, scoped out by
`if path.starts_with("@file_")`; `@file_*` entries carry empty stderr by construction, so it is
correct **by accident of a prefix guard** rather than by design.

**The premise "three callers" was wrong and was corrected by enumeration**, which is the reason
this file states the population rather than the two symptoms.

## Evidence

### E1 — the existing test is monotone under this defect

`stderr_suffix` (`src/tools/output_buffer.rs`) asserts
`get("{id}.err").stderr == "error msg"`. That tests the **resolver**, and passes whether or not
any caller selects the right stream — `CLAUDE.md` § *Testing Discipline*: an assertion computed
over a population cannot verify a claim about a member. The suffix has therefore been
green-and-broken for as long as both callers have existed.

### E2 — the mechanism is on no agent-facing surface

`.err` appears in **no** `get_guide` topic, **no** `src/prompts/` slice, **not**
`.codescout/system-prompt.md`, and nowhere in `docs/` except two superseded March design plans.

**Three sessions concluded in one evening that a buffered stderr was unrecoverable, and none
tested the suffix.** That is the sharper half of this bug: a mechanism that ships, works, and is
undiscoverable is not a documentation gap — it is why three independent readers were confidently
wrong in the same direction, and why one of them filed
`docs/issues/2026-09-14-every-reader-of-a-cmd-buffer-takes-stdout-only-so-the-stored-stderr-reaches-nobody.md`
with a false central claim.

## Fix

Planned, not yet landed:

1. `OutputBuffer::get_stream(id) -> Option<String>` applying the suffix policy **once**; route
   `read_file` and `grep` through it.
2. A **population guard** that reds when a new site takes `.stdout` off a resolved entry. This is
   the load-bearing half — the helper shortens the right path without removing the wrong one, so
   a guard is what actually closes it (`CLAUDE.md` § *Observer Blindness* position 3).
3. Document the suffix on at least one agent-facing surface.

**Deliberately out of scope:** `src/peer/server.rs`'s concatenation (a legitimate different
policy — a peer reading a handle wants everything) and the interpolation path (correct, tested,
and carrying `@tool_*` pretty-printing that a shared selector would have to grow a branch for).

## Tests added

Three behavioural, one predicate, one allowlist-hygiene, plus the population guard.

**Each behavioural test asserts a presence AND an absence**, because "the stderr token is
present" is satisfied by a fix that **concatenates** both streams — which would mislead every
caller while passing. Each stream carries a token the other does not.

- `read_file_err_handle_reads_stderr_not_stdout` — `.err` reads stderr, stdout does not leak
  through it, and the **bare handle keeps its stdout contract**. That third assertion is not
  padding: buffer line numbering is stdout-relative for a bare handle and `sed -n 'N,Mp'
  @cmd_x` callers depend on it, so it pins what this fix must not move.
- `grep_on_an_err_handle_searches_stderr_not_stdout` — same three directions.
- `the_matcher_discriminates_the_defect_shape` — drives the guard's predicate both ways.
- `every_allowlist_entry_is_live_and_justified` — a stale exemption names a file nobody can
  find; an empty reason is an exemption nobody can evaluate.

**A test defect found by running it, worth recording because the remedy is a law this repo
already states.** The grep test first asserted `format!("{r:?}").contains("STDOUT_ONLY_TOKEN")`
and **failed against correct code**: grep's `suggestion` field ECHOES THE PATTERN back
(*"Consider: symbols(name='STDOUT_ONLY_TOKEN')"*), so a zero-match response contains the token.
The discriminator — `total` — was already in the output, unused, while the test reached for a
proxy over the whole serialized blob. `CLAUDE.md` § *Testing Discipline*: *where a system
already names its own failure state, assert on the name, not on a proxy for it.*

**Mutations run**, `./scripts/mutation-probe.sh --strict`, isolated worktree, one per guarded
SITE:

| site | mutation | result |
|---|---|---|
| `read_file.rs` | `.get_stream(path)` → `.get(path).map(\|e\| e.stdout)` | **KILLED** — *"got content: STDOUT_ONLY_TOKEN"* |
| `grep.rs` | `.get_stream(raw_path)` → `.get(raw_path).map(\|e\| e.stdout)` | **KILLED** — `total` 0, expected 1 |
| `tests/buffer_stream_policy.rs` guard | revert a site and demand the guard reds | see below |

**`--strict` paid for itself during these runs, which is worth one line since it shipped
hours earlier.** The MCP server in this session predates the `type: "test"` envelope fix, so
the probe's verdict line did **not** arrive — the exact defect
`docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-….md` records. What
did arrive was `exit_code: 101` rather than `3`, which under `--strict` separates KILLED from
INCONCLUSIVE with no verdict line at all. That is the claim `--strict` was built on — the exit
code is the only channel surviving every renderer — observed rather than argued.
## Workarounds

Use the interpolation form — `run_command("grep PATTERN @cmd_x.err")` — which selects correctly.
Do **not** pass a `.err` handle to `read_file` or the `grep` tool until this is fixed: they
answer about stdout and say nothing about having done so.

## Resume

Start from the population table in § *Root cause*; it is the enumeration, not a sample. The guard
in § *Fix* item 2 is the deliverable that outlives the two site fixes.
