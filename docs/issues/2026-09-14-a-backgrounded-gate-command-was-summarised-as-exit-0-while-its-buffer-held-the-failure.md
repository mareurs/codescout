---
id: '60ad8dda4d0b7ec3'
kind: bug
status: open
title: 'BUG: a backgrounded gate command was summarised as exit 0 while the buffer it pointed at held the compile failure'
tags:
- cluster/unclassified
- run-command
- progressive-disclosure
- shared-checkout
- observability
---

## Summary

A `run_command` call made with `run_in_background: true` returned a response whose summary
read `✓ exit 0`, for a `cargo clippy` invocation that had **failed to compile**. The real
failure was present in the referenced buffer the whole time. The caller — this session — read
the summary, treated the gate as green, and moved on with a truncated source file in the tree
that was redding the shared build for every other session.

Three controlled variants failed to reproduce it. The observation is recorded verbatim because
it is a *false green on a gate command*, which is the most expensive direction this class can
fail in, and because the same session's later foreground run of the identical command reported
the failure correctly.

## Symptom (Effect)

The call:

```
run_command(
  command: "cargo clippy --workspace --all-targets --features local-embed -- -D warnings 2>&1",
  run_in_background: true,
)
```

The response, verbatim:

```json
{
  "output_id": "@tool_9ffd17c9",
  "summary": "✓ exit 0  (query @bg_00000001)",
  "buffered_bytes": 363348
}
```

The referenced buffer, read later in the same session:

```
error: this file contains an unclosed delimiter
    --> src/util/path_security.rs:2919:90559
error: could not compile `codescout` (lib) due to 1 previous error; 1 warning emitted
error: could not compile `codescout` (lib test) due to 1 previous error
```

`cargo` exits `101` on that. The summary said `✓ exit 0`.

## Reproduction

**Not reproduced.** Three variants, all `run_in_background: true`, all in the same session
minutes afterward — every one returned `"hint": "Process running. Output captured in @bg_…"`
with **no exit claim at all**, which is the documented and correct shape:

| variant | command | response |
|---|---|---|
| slow fail | `echo starting; sleep 3; echo failing >&2; exit 7` | `Process running`, no exit claim |
| fast fail | `echo out; echo err >&2; exit 7` | `Process running`, no exit claim |
| large output + fail | `seq 1 80000; echo boom >&2; exit 7` | `Process running`, no exit claim |

The third was chosen to test the one variable visibly different about the clippy call — its
response was an **overflow envelope** (`output_id` + `summary` + `buffered_bytes`, 363 KB)
rather than the `Process running` shape. Output size alone did not produce it.

So the discriminating variable is **not** identified. What distinguishes the clippy call and
is not yet ruled out: it was a `cargo` invocation (the tool type-classifies these — the later
foreground run came back `"type": "build"`), it carried `2>&1`, and it ran while an MCP server
restart was imminent.

## Environment

`experiments`, 2026-09-14, live MCP over stdio. The same command run in the **foreground**
minutes later, after the tree was repaired, reported `"type": "build", "exit_code": 0`
correctly — and a foreground `cargo test` chain in the same session correctly surfaced
`exit 101` shapes elsewhere. The defect is not "this session cannot read exit codes".

## Root cause

**Not established, and deliberately left open.** Naming one without a reproduction would close
the inquiry — the failure mode this corpus already pays for. What is measured is the outcome:
a summary field asserted success while the buffer it pointed at held a compile failure.

## Evidence

The bracket is worth more than the headline, because it is a **positive** identification rather
than an elimination, and it re-dates a separate incident:

1. `cargo test --lib util::path_security` → **216 passed**. The file compiled here.
2. `scripts/fmt-mine.sh` → `formatted 1 file(s) written by this session` — rustfmt wrote
   `src/util/path_security.rs`.
3. `cargo clippy … run_in_background: true` → summary `✓ exit 0`; **buffer holds the compile
   error**. The file was already truncated (4954 → 2918 lines) at this point.

Step 3's buffer is what proves the truncation predates the clippy run, which places the
truncating write at step 2 and **not** at the MCP server restart that followed. This session
had earlier told a peer the truncation was "at or after the server restart"; that was wrong,
and the buffer is what corrected it.

**A peer independently proposed the weaker reading** — that the green was merely *stale*, a
correct result about an earlier instant — and reasoned it well from the ordering alone:
clippy ran after rustfmt and passed, which would place the break later. Reading the buffer
settles it the other way. Recorded because the weaker reading is the one that sounds more
rigorous, and accepting it would have sent the investigation somewhere there was nothing to
find.

## Hypotheses tried

1. **Hypothesis:** the buffer was overwritten and I am quoting a later run.
   **Test:** the quoted lines name `path_security.rs:2919`, a state that existed only in the
   pre-repair window; the post-repair file is 4966 lines and compiles.
   **Verdict:** rejected.
2. **Hypothesis:** any backgrounded non-zero exit is reported as `✓ exit 0`.
   **Test:** three variants above. **Verdict:** rejected — all three returned `Process running`.
3. **Hypothesis:** overflow-sized output is the trigger.
   **Test:** variant 3 (80 000 lines, non-zero exit). **Verdict:** rejected.

## Fix

Not designed — the mechanism is unidentified and a fix aimed at a guess would be unfalsifiable.
What the record asks for first is the discriminating variable.

One observation that should survive whatever the mechanism turns out to be: the `✓` glyph and
the `exit 0` are composed into the **summary**, the field a caller reads *instead of* the
buffer. Progressive disclosure exists so the buffer need not be read; that is exactly why a
wrong summary is not a cosmetic defect here. A summary that cannot establish an exit status
should decline to assert one — the `Process running` shape, which the three repro variants
returned, already does this correctly.

## Tests added

None — there is nothing to regress against until the trigger is known. A test asserting
"backgrounded non-zero exit is not summarised as `✓ exit 0`" would pass today against all three
reproducible variants and say nothing about the observed one, which is coverage in the
direction the corpus calls vacuous.

## Workarounds

**Run gate commands in the foreground.** This is independently required: `CLAUDE.md`
§ *Reaching a Peer Session* records that `run_in_background: true` returns before an exit
status exists to hook on, so the `attribute-red` hook never fires either. Confirmed live the
same day by a second session from the other side — they ran the gate backgrounded, got no
attribution, and had to run `scripts/file-provenance.py` by hand.

That ceiling and this defect are **different failures that share a trigger**: backgrounding
costs you the attribution channel (measured, documented), and — once — also handed back a green
that was not one. Do not conflate them; the first is established and the second is one
observation.

## Resume

Find the discriminating variable. Candidates not yet separated: `cargo`-classified commands
(`"type": "build"`) versus plain shell; a trailing `2>&1`; a process whose exit races the
response render; proximity to an MCP server restart. The cheapest next probe is a backgrounded
`cargo` command that fails to compile, in a throwaway crate, with and without `2>&1`.

**Note on the evidence's durability:** `@bg_*` handles are **recycled**. The three repro
variants above were issued in this same session and the second of them was assigned
`@bg_00000001` — the very handle that held the clippy evidence. The buffer quoted in
§ *Symptom* was read and transcribed **before** that happened, but it no longer exists to
re-read. Anyone re-deriving this must capture the buffer to a file, not to a handle.

## References

- `docs/issues/archive/2026-09-14-the-fmt-refusal-names-an-owner-who-holds-none-of-the-bytes.md` —
  the same day's `fmt-mine.sh` work; step 2 of the bracket is that script's rustfmt call.
- `CLAUDE.md` § *Reaching a Peer Session* — the documented `run_in_background` attribution
  ceiling, the sibling half of this trigger.
- `get_guide("progressive-disclosure")` — the summary/buffer contract this violates.
