---
kind: tracker
status: active
title: Reconnaissance patterns
owners: []
tags:
  - reconnaissance
  - skill-meta
  - scout
---

# Reconnaissance patterns

Per-project R-N ledger for the `codescout-companion:reconnaissance`
skill in this project. See the canonical bootstrap, append rules,
sync flow, and R-N entry template in the skill's
`SKILL.md` and `references/reconnaissance-patterns-template.md`.

Three buckets: **hits** (scout caught drift), **misses** (scout missed,
downstream gate caught), **proposals** (vocabulary expansions for the
skill).

## Index

| ID | Date | Verdict | Pattern | Evidence (session-log) |
|----|------|---------|---------|------------------------|
| R-1 | 2026-05-19 | hit | Pre-dispatch grep for asserts on `include_str!`'d constants | mcp-prompt-redesign F-1 + W-1 |
| R-2 | 2026-05-19 | miss | Scout missed constant-write patterns (`.replace(TOKEN, ...)`) | mcp-prompt-redesign F-2 |
| R-3 | 2026-05-19 | miss → promoted | Scout limited grep to one file/crate; cross-file asserts slipped | mcp-prompt-redesign F-2 |
| R-4 | 2026-05-19 | miss | Grep undercounts struct-field construction sites by 2-3× | mcp-prompt-redesign F-3 + W-2 |
| R-5 | 2026-05-19 | proposal | Add "compiler as scout" as a Phase-1 tool alongside grep | covers R-4 |

## R-1 — Pre-dispatch grep for asserts on `include_str!`'d constants

**Verdict:** hit

**Observed:** 2026-05-19, MCP prompt channel redesign work stream
(`docs/trackers/mcp-prompt-redesign-session-log.md` F-1, W-1).

**Pattern:** Before rewriting a content file (`source.md`, embedded
templates, etc.) that backs a static constant via `include_str!`,
grep the codebase for asserts on that constant. Specifically:

```
<CONST>.contains(...)
<CONST>.find(...)
<CONST>.matches(...)
snapshot calls naming the surface file
```

Enumerate every test that will fail post-rewrite and name them in
the implementer's dispatch prompt.

**Evidence:** Without R-1, U4 implementer would have run the 4
planned `redesign_invariants` tests, hit 6 unplanned
`SERVER_INSTRUCTIONS`-asserting failures, and either reported
DONE_WITH_CONCERNS or BLOCKED. Estimated cost saved: 6-12 subagent
round-trips.

**Counterfactual confirmed by:** F-1 enumeration in
`mcp-prompt-redesign-session-log.md`, evidenced by ≥4 tests deleted
during U4 that were NOT in the plan's "1 test may break" prediction.

**Promote-when:** R-1 already validated once. Promote to SKILL.md
after a second `include_str!` rewrite work stream confirms the
pattern. Concrete addition: `SKILL.md § Phase 1 — Scout`, sub-bullet
"For `include_str!`'d content files, grep `<CONST>.contains / .find /
snapshot` to enumerate asserting tests."

---

## R-2 — Scout missed constant-write patterns (`.replace(TOKEN, ...)`)

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-2).

**Pattern that failed:** The scout grepped reads of the constant
(`<CONST>.contains`, `.find`, etc.) but did NOT grep *writes into*
the constant via runtime token substitution (`SERVER_INSTRUCTIONS
.replace(SYMBOL_NAV_TOKEN, &nav_content)`). When the token left
`source.md`, the `.replace` became a silent no-op — the
language-specific nav block was dropped at runtime. Recon missed it;
the spec reviewer flagged it during U4 review.

**Cost absorbed:** 1 extra fix-up subagent dispatch (U4 fix-up).

**Pattern proposal (folds into R-5):** Phase 1 grep should include
constant **writes** as well as reads:

```
<CONST>.replace(<TOKEN>, ...)
<CONST>.replacen(...)
write_str! / format! using the constant
```

For string-substitution prompts, also enumerate every `TOKEN`-style
constant declared near the surface and grep callers.

**Promote-when:** R-2 + one more "write-side substitution missed"
miss → promote the expanded grep vocabulary to SKILL.md.

---

## R-3 — Scout limited grep to one file/crate; cross-file asserts slipped

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-2, second half).

**Pattern that failed:** The scout grepped `src/prompts/` for
asserts on the rewritten content. A 7th broken test
(`server_instructions_documents_goal_tracker_discovery`) lived in
`src/server.rs` — outside the scout's grep scope. Recon missed it.

**Pattern proposal:** Phase 1 grep must default to the **workspace
root**, not the directory of the file being changed. Constants and
their callers cross crate / module boundaries; assertion sites do too.

**Cost absorbed:** 1 extra deletion in the U4 fix-up.

**Promote-when:** R-3 already validated as a needed default. Cheap
fix: add a sentence to `SKILL.md § Phase 1 — Scout` — "Grep scope
defaults to workspace root, not the file being modified."

**Status:** promoted to SKILL.md (claude-plugins:787cdec0, 2026-05-23). Added as a 4th bullet under Phase 1 — Scout, citing this R-3 row by name. Promote-when criterion fired with 1/1 datapoint, per the tracker's note ("already validated as a needed default").

---

## R-4 — Grep undercounts struct-field construction sites by 2-3×

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-3, W-2).

**Pattern that failed:** For "add a required field to widely-used
struct", scout grepped `ToolContext\s*\{|ToolContext::new` and
counted 13 sites. Reality required ~30 (one test file alone had 24
construction sites — many on single lines the regex matched once
per file rather than per occurrence; many nested inside macros and
helper factories).

**Cost absorbed:** Implementer fell back to a `perl -i -0pe` bulk
pass driven by `cargo build` errors. Two files double-inserted;
deduped manually. Net result correct but the controller-side scout
gave a wrong estimate of blast radius.

**Pattern proposal (covered by R-5):** For exhaustive enumeration
of construction sites of a struct that gains a non-`Option` field,
use `cargo build` as the scout. The compiler reports every missing
field; grep only approximates.

**Promote-when:** validated once already. Pairs with R-5 for the
expansion.

---

## R-5 — Add "compiler as scout" as a Phase-1 tool alongside grep

**Verdict:** proposal

**Source:** R-4 + W-2 in
`docs/trackers/mcp-prompt-redesign-session-log.md`.

**Proposal:** `SKILL.md § Phase 1 — Scout` currently lists grep,
`symbols`, and `references` as the scout's tools. Add a fourth:

> **For non-`Option` field additions and similar exhaustive
> enumeration problems, use the compiler as scout.** Add the field
> (or whatever forces every site to update), run `cargo build`, and
> let the compiler enumerate every site via "missing field" errors.
> This is exhaustive by construction. Grep is for *finding* a
> representative site; the compiler is for *counting* all of them.

**Why this is a phase-1 tool, not a phase-4 fallback:** the scout's
job is to estimate blast radius before dispatch. Wrong blast radius
estimate → wrong dispatch (one subagent vs N, or one prompt with 13
enumerated sites vs the right "use compiler-driven enumeration"
instruction). The compiler-as-scout pattern *informs the dispatch
prompt itself*, not just the implementation.

**Caveats:**
- Works only when the change *forces* all sites to update (required
  field, trait method without default, etc.). Default-`None`
  optional trait methods don't trigger compile errors.
- Cost: one `cargo build` cycle per scout pass. For codescout that's
  ~30-60s — acceptable.

**Threshold to promote:** R-4 + one more datapoint where a
struct-field-style change benefits from this approach. Currently
1/2.

---

## Template for new entries

<!-- Insert new R-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## R-N — title\n**Verdict:** ...\n...")
     Also update the Index table row at the top. -->
