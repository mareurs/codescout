---
id: '10df47a34b5b3fe7'
kind: bug
status: fixed
title: 'BUG: append_entry''s server-side body writer is undeclared in the artifact schema, so the one path that cannot produce an uncitable entry is invisible'
tags:
- prompt-surface
- librarian
- append_entry
- schema
topic: prompt-surfaces
closed: 2026-08-18
opened: 2026-08-18
owner: marius
related:
- docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md
- docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
severity: high
---

# BUG: `append_entry`'s server-side body writer is undeclared in the `artifact` schema, so the one path that cannot produce an uncitable entry is invisible

## Summary

`5d5ed457` ("the server writes the prose entry, so it cannot be born undefined")
gave `append_entry` a `title` + `body` + `anchor_heading` mode that assigns the id
**and** writes a `def_re`-conformant `## <ID> — <title>` heading in one atomic call.
`anchor_heading` is not a declared property of the `artifact` tool's input schema,
and `title` / `body` are documented as `create`-only. The single path that
structurally cannot produce an uncitable entry is therefore undiscoverable from the
only surface an agent reads — reachable only by first performing the fallible
reserve-then-hand-write flow and reading the follow-up hint.

## Symptom (Effect)

Calling the documented reservation form returns a hint naming a parameter the schema
does not declare:

```
artifact(action="append_entry", id="5696563f06b2c222", id_prefix="R")
→ {
    "id": "R-106", "reserved": true, "section_written": false,
    "next_step": "Reserved R-106 ... the entry itself is yours to write.
       Add the section as `## R-106 — <title>` ...
       Next time, pass `title`, `body` and `anchor_heading` to have the server
       write it and remove that failure mode entirely."
  }
```

Against the live `tools/list` payload, `artifact` advertises 51 properties and
`anchor_heading` is not among them:

```
$ jq -r '.[]|select(.name=="artifact")|.inputSchema.properties|has("anchor_heading")' tools_list.json
false
```

`title` and `body` are declared, but scoped away from this use:

```
"title": "create: artifact title"
"body":  "create: markdown body"
```

## Reproduction

1. `git rev-parse HEAD` → `e7a57f89` (branch `experiments`).
2. Drive the release binary's MCP handshake and dump `tools/list`
   (`target/release/codescout start`, `initialize` → `tools/list`).
3. Inspect `artifact.inputSchema.properties` — 51 keys, no `anchor_heading`.
4. Call `append_entry` with `id_prefix` only; read `next_step`.

## Environment

Linux, `experiments` @ `e7a57f89`, MCP stdio transport, project `codescout`.
Release binary built 2026-08-18 15:17.

## Root cause

Two edits that must ship together did not. The capability landed in
`5d5ed457` (2026-08-18 09:42):

- `src/librarian/tools/append_entry.rs:34` — `anchor_heading: Option<String>` on `Args`.
- `src/librarian/tools/append_entry.rs:112-132` — requires `title` + `body` +
  `anchor_heading` as a set; a partial set is refused naming exactly which field is
  missing.
- `src/librarian/catalog/augmentation.rs:836-1088` — `PendingSection` carries the
  anchor; `allocate_entry_id` writes the heading at the ledger's own level and
  validates the anchor exists verbatim, writing nothing at all on a bad anchor
  (pinned by `a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`).

The advertised schema was not updated in the same commit. `src/prompts/README.md`
requires a prompt-surface review for "any change to tool behavior or signatures …
new parameter semantics"; it did not run here.

*Measured 2026-08-18: `git log -S 'anchor_heading' -- src/librarian/` → single commit
`5d5ed457`; live `tools/list` dump confirms the property is absent. The parameter is
nonetheless accepted at runtime — F-1/F-2/W-1 in
`docs/trackers/prompt-surface-compaction-session-log.md` were all written with it,
each returning `"section_written": true`.*

## Evidence

### The undeclared path works, which is what makes the omission costly

Three consecutive calls passing `title` + `body` + `anchor_heading` returned
`{"section_written": true}` and produced `## F-1 — …`, `## F-2 — …`, `## W-1 — …`
headings that `link_scan`'s `def_re` accepts. The feature is complete; only its
advertisement is missing.

### The failure mode it removes is measured, not hypothetical

From the same work stream: 13 ledgers across five repos carried entries their body
never defines, the largest at 64 of 68; one namespace resolved to nothing against
117 live citations; ~30 of 39 sampled dangling tokens in this repo came from
row-only entries. `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`
is the sibling bug.

### The parent bug's Resume is stale by one commit

`docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md`
§ Resume still reads *"build the server-side body writer — extend `append_entry` so the
call that assigns an id also writes `## <ID> — <title>`"*. Its last commit is `32e8bf51`
at 09:32; `5d5ed457` landed at 09:42. Ten minutes, and nothing re-read the bug file.

## Hypotheses tried

1. **Hypothesis:** the hint advertises a parameter that was never implemented (vanity
   hint). **Test:** `grep 'anchor_heading' --glob '*.rs'`. **Verdict:** rejected —
   15 matches across two files including the `Args` field, the tri-field guard, and two
   pinned tests. **Evidence:** § Root cause.
2. **Hypothesis:** the parameter is declared but my dump was stale. **Test:** re-read
   `artifact.inputSchema.properties` from a `tools/list` served by the binary built at
   15:17, after `5d5ed457`. **Verdict:** rejected — 51 keys, no `anchor_heading`.

## Fix

Single-surface edit in `Artifact::input_schema()` (`src/librarian/tools/artifact.rs`):

1. Declare `anchor_heading` with the tri-field requirement stated ("pass with `title` +
   `body` to have the server write the section; all three or none").
2. Re-scope `title` / `body` descriptions to name both roles, not just `create`.
3. Fold the reserve-only path's `next_step` guidance into the schema so the correct
   call is reachable *before* the fallible one.

**Done in `01194e21` (`experiments`).** All three points landed.

**The byte note above was written before the measurement and is wrong; kept because the
correction is the useful part.** It argued for funding the 52nd param out of prose that
duplicates `get_guide("librarian")`, on the assumption that guide bytes are cheaper than
schema bytes. They are not. Measured 2026-08-18 across four sessions and three models,
**100.0% of input reads are cache hits**, so both surfaces sit in the same cached prefix
and are re-read at the same $0.30/M. A guide fired at turn K costs
`X × (N−K) × cache_read + X × cache_write` against the schema's `X × N × cache_read` —
break-even at **K ≈ 12.5 turns**, and `librarian` auto-injects on the first `artifact`
call. Moving that prose would have been a wash.

What actually paid for it: the declaration cost **+808 chars** and breached
`TOOL_SURFACE_CHAR_BUDGET` (58,572 → 59,380) on the gate's first real use. It was funded
by compressing the `workspace` description injected into all 24 pinnable tools (225 → 131
chars), keeping all four of its facts. Net **58,572 → 57,148**, with the budget ratcheted
down to the new total rather than left slack.

## Tests added

`server::tests::artifact_advertises_the_append_entry_section_writer` (`src/server.rs`) —
asserts `artifact`'s advertised schema carries `title`, `body` and `anchor_heading`.
Mutation-verified: renaming the schema key makes it fail, naming both the missing field
and the call site that accepts it.

The **general** form — every field the server accepts is advertised — is deliberately not
tested here. It wants the schema derived from `Args` via `schemars` (already a direct
dependency), deferred under this project's rule-of-three with one confirmed instance and
one unverified. See the spec's *Revisit-when*.

Also landed alongside: `server::tests::tool_surface_under_budget` and
`tool_surface_report_lengths` (`598b92f2`), which is why a future instance of this class
has to be paid for rather than absorbed.
## Workarounds

Pass `anchor_heading` anyway — it is accepted. Full form:

```
artifact(action="append_entry", id="<ledger id>", id_prefix="F",
         title="...", body="...", anchor_heading="## Template for new entries")
```

## Resume

N/A — fixed and verified on `experiments` (`01194e21`), gate green (`cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, `cargo test` 4,164 passed), regression test
mutation-verified.

No pending-master-SHA line: `git rev-list --left-right --count master...experiments`
returns `0 1051`, so the promotion path is **fast-forward** and this `experiments` SHA
already is the master SHA.
## References

- `src/librarian/tools/append_entry.rs:34,112-132`
- `src/librarian/catalog/augmentation.rs:836-1088`
- `5d5ed457` — feat(librarian): the server writes the prose entry
- `docs/trackers/prompt-surface-compaction-session-log.md` — F-1 (this bug's evidence), W-1
- `docs/trackers/reconnaissance-patterns.md` — R-106
- `docs/issues/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md` — parent bug, stale Resume
