---
status: open
opened: 2026-08-15
closed:
severity: high
owner: marius
related: []
tags: [prompt-surfaces, server_instructions, get_guide, external-report]
kind: bug
---

# BUG: `server_instructions` arrives at the model truncated mid-word, dropping the guide pointers

# Summary

The `server_instructions` surface is capped at 2,200 chars server-side, but the client
cuts it below that — so the surface arrives truncated mid-word, ending
`- "iron-… [truncated]`. The `iron-laws-detail` and `symbol-navigation` guide pointers
and the trailing content never reach the model. A model that never learns those topics
exist cannot call `get_guide` for them.

## Symptom (Effect)

Observed directly in this session's own system prompt. The MCP server-instructions
block ends:

```
Before deeper work in an area below, call get_guide(topic) FIRST:
- "librarian"               — artifacts, filters, trackers
- "tracker-conventions"     — frontmatter, archive, status
- "progressive-disclosure"  — output budgets, @ref buffers
- "error-handling"          — RecoverableError vs anyhow::bail
- "workspace-state"         — activate, home/foreign, reset
- "iron-… [truncated]
```

The list is cut in the middle of the `iron-laws-detail` entry.

## Reproduction

Reproduces on every session on this host at `821f9d0d` — read the MCP server
instructions block in the system prompt and observe the trailing `[truncated]` marker.
The external reporter observed the identical cut on macOS, and measured the slice at
**2,203 bytes** against a real client cut of roughly 2 KB.

## Environment

Reported on macOS against `experiments @ d7988aca` (Claude Code). Reproduced on Linux
at `821f9d0d`, Claude Code, stdio transport. Cross-platform and cross-machine — this is
not a local misconfiguration.

## Root cause

`MAX_INSTRUCTIONS_CHARS = 2200` (`src/prompts/mod.rs:1193`). The adjacent comment reads
*"The 2200 cap gives ~200 bytes …"* of headroom against the channel limit documented in
`docs/architecture/mcp-channel-caps.md`.

The cap is therefore set **at** the cliff rather than below it, and the reporter's
measurement (2,203 bytes emitted) says the real content can exceed even the nominal
2,200. Whatever headroom was intended is not being realised in practice.

The failure is invisible from inside the server: the cut happens client-side, after
emission, so no server-side test or assertion observes it.

*Measured 2026-08-15: this session's own system prompt, read directly. The 2,203-byte
figure is the external reporter's measurement on his host, not re-measured here.*

## Evidence

### What is lost

The truncated tail contains the `get_guide` topic pointers. In this session the
`iron-laws-detail` entry is cut mid-token, and any entries after it are gone entirely,
along with the Project Status block the reporter also notes as missing.

### Why the loss is self-concealing

The pointers are the mechanism by which a model discovers that deeper guidance exists.
Losing them is silent: nothing signals absence, and the model has no way to know a
topic it was never told about is callable. The reporter's phrasing is exact — *the
instruction telling the model not to trust truncated output was itself truncated before
it arrived*.

### Compounding interaction

`docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`
records that the `iron-laws-detail` guide contains a false claim. This bug means most
sessions never reach the pointer to that guide at all — so the two defects mask each
other: the guide is rarely read, and when read, misleads.

## Hypotheses tried

1. **Hypothesis:** the truncation is a display artifact of the transcript rather than
   the real prompt. **Test:** compare against the reporter's independent observation on
   a different OS and checkout. **Verdict:** rejected — identical cut point
   (`- "iron-…`) on two hosts, two platforms, two checkouts.

## Fix

Not yet implemented. Three parts, in order:

1. **Measure the real client cut point.** Emit a slice of known length and compare it
   against what arrives in the system prompt. Until that number exists, any new cap is
   another guess — and the current 2,200 with "~200 bytes" of nominal headroom is
   already an empirically falsified guess.
2. **Lower `MAX_INSTRUCTIONS_CHARS`** (`src/prompts/mod.rs:1193`) beneath the measured
   limit, with real margin. The existing test at `:1199` then becomes meaningful
   instead of merely green.
3. **Reorder the surface.** The `get_guide` topic list is the most load-bearing content
   in the slice and is currently last, so it is exactly what gets cut. Move it earlier
   so truncation lands on less critical text.

Point 3 is the same head-placement principle as
`docs/issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`:
when a channel truncates from the tail, ordering is the cheap fix and the budget is the
expensive one. Two independent surfaces in this codebase lose their most important
content to a tail cut for the same reason.

That sibling is now **fixed** (`bb2a9625`), which makes it a worked precedent rather than
just an analogy: nine call sites across five surfaces, producer-side ordering only, with
the cutter deliberately left alone. It also found three further signals riding the same
cut that its own title never mentioned — worth expecting the same here.
## Tests added

None yet — **but a test already exists and passes, which is the sharpest part of this
bug.**

`src/prompts/mod.rs:1199` (`redesign_invariants::source_md_under_cap`) asserts:

```rust
rendered.len() <= MAX_INSTRUCTIONS_CHARS,
```

So the repo *does* gate the slice against its own 2,200-char cap, and that gate is
green. The surface still arrives truncated, because **the cap the test enforces is not
the limit that actually cuts.** The client cuts below 2,200; the test measures against
2,200. A green suite is therefore positive evidence of nothing, and has been the whole
time.

This is the same shape as the defects this report is otherwise about: a check that
reports success for a question nobody asked. What is missing is a test anchored to the
*observed* client limit, not to the constant the server chose.
## Workarounds

Call `get_guide("iron-laws-detail")` explicitly — the topic works even when its pointer
never arrived. The topic list is documented in `CLAUDE.md` and
`docs/trackers/archive/get-guide-topics.md`.

## Resume

Determine the real client-side byte limit for MCP `server_instructions` (compare
emitted length against what arrives in the system prompt), then lower
`MAX_INSTRUCTIONS_CHARS` at `src/prompts/mod.rs:1193` beneath it and add a rendered-length
test. Verify the claim in `docs/architecture/mcp-channel-caps.md` still matches
observed behaviour.

## References

- `docs/trackers/bistriceanu/index.md` § B-10
- `src/prompts/README.md` — the 2200-byte slice cap and the shared-branch verify hazard
- `docs/architecture/mcp-channel-caps.md` — cited by the cap's own comment
- **`docs/trackers/reconnaissance-patterns.md` § R-86** — this bug is that rule,
  independently rediscovered from outside. R-86 was written 2026-08-15 about an LSP fix
  that shipped inert because *"the end-to-end test drives `LspClient::start` with
  `mux: false` — it exercised the one transport on which the defect cannot appear."*
  Its rule: **"name every transport / deployment mode the component has and ask which
  one the test constructed and which one production runs. If they differ, the test is a
  smoke test."**

  `source_md_under_cap` (`src/prompts/mod.rs:1199`) constructs the *server-side render*
  and measures it against the *server's own constant*. Production is the client
  channel, which cuts lower. Same shape, different subsystem, found on the same day by
  an outside user who had never heard of R-86 — which is decent evidence the rule
  generalises past LSP transports to any surface with a cap on both sides of a boundary.
