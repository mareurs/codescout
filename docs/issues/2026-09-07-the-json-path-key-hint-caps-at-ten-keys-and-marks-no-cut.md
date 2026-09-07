---
id: '35fadcf5656b865d'
kind: bug
status: open
title: 'BUG: read_file''s json_path key-miss hint caps its key list at ten and marks no cut, so absence from it reads as absence from the object'
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
- progressive-disclosure
- error-messages
- measurement
topic: error messages that present a truncated enumeration as complete
---

# BUG: the `json_path` key-miss hint caps at ten keys and marks no cut

## Summary

`read_file(..., json_path=...)` answers a missing key with `Available keys: <list>`. The list is
capped at **ten** by a hardcoded `.take(10)` with no count, no ellipsis and no marker, so an
object with more than ten keys yields an enumeration that is **complete-looking and silently
short**. A caller who reads it as the object's key set concludes a present key is absent.

Reached only by a caller who has already guessed wrong — which is exactly when they are relying
on the hint to tell them what is really there.

## Symptom (Effect)

Against a live `librarian(action="doctor")` buffer:

```
read_file("@tool_7a2908a2", json_path="$.catalog_health.zzz_probe_certainly_absent")

error: path segment 'zzz_probe_certainly_absent' not found
hint:  Available keys: hidden_rows, slug_coverage, move_candidates,
       outside_roots_by_project, outside_roots_scoped_by_project,
       entry_validity_scoped_by_project, cited_prefix_scoped_by_project,
       row_checks_scoped_by_project, declared_roots, archived_fix_shas
```

Ten keys. Both of the following resolve on **the same buffer** and appear in neither the list nor
any warning:

```
read_file("@tool_7a2908a2", json_path="$.catalog_health.audit")  -> 22 lines of content
read_file("@tool_7a2908a2", json_path="$.catalog_health.hint")   -> a full paragraph
```

Nothing in the error says the enumeration was cut.

## Reproduction

At `02ebd38e`, any object with more than ten keys:

```
librarian(action="doctor")
read_file("@tool_<id>", json_path="$.catalog_health.<any-absent-key>")   # lists 10
read_file("@tool_<id>", json_path="$.catalog_health.audit")              # resolves
```

Deterministic. Not specific to `doctor` — any buffer or on-disk JSON with an >10-key object at the
addressed level reproduces it.

## Environment

`experiments` at `02ebd38e`; MCP server pid 1007917 running `target/release/codescout` (mtime
2026-09-06 22:51:21, no `(deleted)` marker, zero `src` commits after it — so the running binary is
the tree).

## Root cause

`src/tools/file_summary/file_summary.rs:863`, in `resolve_json_segment`'s `Segment::Key` arm:

```rust
let available = obj.keys().take(10).cloned().collect::<Vec<_>>().join(", ");
```

A hardcoded `.take(10)`, joined and emitted with no arity and no cut marker. `serde_json::Map`
preserves insertion order here — the observed list is not alphabetical — so the keys dropped are
the **last inserted**, which are also the **newest to be added to a payload** and therefore the
ones a caller is most likely to be reaching for.

For `catalog_health` specifically, `src/librarian/tools/doctor.rs` inserts `audit` at `:950` and
`hint` at `:951`, positions eleven and twelve. Both are dropped. Measured 2026-09-07 by probing a
deliberately absent key on a fresh server, then resolving each unlisted key individually — so this
is a property of the hint, not an artefact of one bad query.

**`cluster/capped-result-presented-as-complete`, and this member has an unusually sharp form of
it: the discipline already exists one function away and was not applied here.** The same `doctor`
call's own envelope prints

```
TRUNCATED: violations[592 of 775] — absence from a cut list is not evidence.
```

for its arrays. So the codebase states the exact law this hint violates, in the same response, for
a different field.

**Not a truncated buffer**, which is the natural hypothesis and was the first one offered: the
envelope does report `violations[592 of 775]`, so truncation is salient. It is ruled out by both
unlisted keys resolving through `json_path` on the same buffer — the resolver sees the whole
object; only the enumeration is short.

## Evidence

### The cap, and its two witnesses

| probe | result |
|---|---|
| `$.catalog_health.<absent>` | hint lists **10** keys |
| `$.catalog_health.audit` | resolves, 22 lines — **not listed** |
| `$.catalog_health.hint` | resolves, one paragraph — **not listed** |

### The same response carries the correct discipline for a different field

```
TRUNCATED: violations[592 of 775] — absence from a cut list is not evidence.
```

## Hypotheses tried

1. **Hypothesis:** the `@tool_*` buffer is capped, so `catalog_health`'s later keys fall outside
   the retained prefix and the enumeration is honest about a truncated object.
   **Test:** resolved each unlisted key individually against the same buffer.
   **Verdict:** rejected — `audit` and `hint` both return full content, so the resolver holds the
   whole object. **Evidence:** *The cap, and its two witnesses*.
   **Worth keeping:** this was the first diagnosis offered and it is the one the envelope's own
   `violations[592 of 775]` line invites. It implies a different remedy (page the buffer) from the
   real one (fix the hint), which is why rejecting it mattered rather than being pedantry.

2. **Hypothesis:** a hardcoded cap in the key enumeration.
   **Test:** read `resolve_json_segment` at `src/tools/file_summary/file_summary.rs:856-868`.
   **Verdict:** confirmed — `.take(10)` at `:863`.

## Fix

**A fix is IN FLIGHT from `ba061586-6581-4656-b0c5-acad83474de5`, and its shape is better than
the one this file first proposed.** They kept the code and gave me the record explicitly, so this
is a split rather than a duplication. Observed in their working tree at 04:42Z:

```rust
const HEAD: usize = 7;
const TAIL: usize = 3;
let names: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
```

**Windowed at both ends, not head-only-with-a-count.** This file originally proposed keeping
`take(10)` and appending the arity. That is worse for the reason the defect exists: with
`preserve_order`, head-only truncation always drops the **newest** keys — the ones a caller has
not yet learned and is therefore reaching for. Reporting the arity would have told the caller
something was missing without ever showing them the thing they wanted. A HEAD/TAIL window keeps
both ends and states the elision.

**And it is the SECOND site of one law, which is the part that outlives this bug.** Their comment
names `resolve_section_range`, one function above in the same file, as having already been fixed
for headings — same bias, same remedy, archived bug file of its own. So the correct implementation
existed a few lines away and the sibling was not swept. That is `CLAUDE.md` § *Testing Discipline*
— *mutate once per guarded SITE, not once per feature* — and it means the ledger question is not
"is this fixed" but "how many other key-enumeration sites are there".

**Their measurement supersedes mine, and I re-derived it rather than taking it:** `catalog_health`
holds **13** keys and `take(10)` elides exactly three — `open_bug_source_citations`, `audit`,
`hint`. All three resolve individually on the same buffer. My earlier guess of
`move_candidates_detail` as the third was wrong; it is inserted conditionally
(`doctor.rs:910`) and was absent from this run's object entirely, so it was never elided.

**Nothing to apply here.** Left to their commit. This file exists because they judged the
Members-line work not worth doing with a machine move imminent, and asked for it to be taken.
## Tests added

**None yet, and they belong to the in-flight fix rather than to this file.** Specified here
because the assertion shape is the easy thing to get wrong: When it is, the regression test must assert the message
**names the arity** on an >10-key object, not merely that a miss errors: an assertion that the call
fails is monotone under exactly this defect, since the hint can lose every key beyond ten and still
fail correctly. Pair it with a ≤10-key object asserting **no** cut marker appears, or the test
passes against an implementation that always claims truncation.

## Workarounds

Treat the key list as a floor, never a census. To learn whether a key exists, **probe it** rather
than reading the enumeration:

```
read_file("@tool_<id>", json_path="$.<path>.<the-key-you-want>")
```

A resolution is proof of presence; absence from the hint is proof of nothing.

## Resume

Apply the `:863` change above, add the two-directional test from *Tests added*, and decide the
`:1238` YAML sibling separately.

## References

- `src/tools/file_summary/file_summary.rs:856-868` — the arm and the `.take(10)`.
- `src/tools/file_summary/file_summary.rs:1238-1242` — the uncapped YAML sibling.
- `src/librarian/tools/doctor.rs:950-951` — `audit` and `hint`, the two dropped keys.
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` — the class.
- Found by sessionId `ba061586-6581-4656-b0c5-acad83474de5`, who hit it while verifying whether
  their own audit fix was in a stale MCP server and nearly reported it absent. Root-caused here;
  their truncated-buffer diagnosis is recorded above as hypothesis 1 because it is the one the
  envelope invites.
- `docs/trackers/design-backlog-session-log.md` `F-10` — the adjacent staleness finding from the
  same incident. Distinct: `F-10` is `by_check` being scoped to a build; this is an enumeration
  capped irrespective of build.
