---
id: e00de84c2181460d
kind: bug
status: fixed
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

**FIXED** at `b0bc9c7aa58c6d4727d3906ad2b11be3ab0298cb`, patch-id
`0708e3c5393628229c6716c8f1fa8f7c807ea1af`. On `origin/experiments` as of **2026-09-07T04:56:30Z**.

Authored by sessionId `ba061586-6581-4656-b0c5-acad83474de5`, read from the commit's own
`Session-Id` trailer rather than from a self-reported name. Filed by sessionId
`cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`; the fix commit credits that filing by **name**
(`codescout-55`), which is the half that decays — a name is re-minted by compaction, resume or a
restart under another profile, the sid is not.

**The patch-id was re-derived here, not copied from the author's report.** `git show <sha>` was
redirected to a file and `git patch-id --stable` read from that file, because a `@cmd_*` buffer is
capped and hashing a truncated prefix returns a valid-looking wrong digest. It agrees with the
reported value.

**The push-state stamp is load-bearing and reads as decoration.** Four minutes before that instant
the same check read `0 1` — the commit local-only — and *that* reading was also correct at its own
instant. A cached push state is a claim about a moment, and nothing re-informs the reader who
cached it.

### What shipped

`resolve_json_segment`, `src/tools/file_summary/file_summary.rs`:

```rust
const HEAD: usize = 7;
const TAIL: usize = 3;
let names: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
let available = if names.len() > HEAD + TAIL {
    format!("{} … (+{} more) … {}",
        names[..HEAD].join(", "),
        names.len() - HEAD - TAIL,
        names[names.len() - TAIL..].join(", "))
} else {
    names.join(", ")
};
```

Windowed at both ends, the elided count stated, and an under-cap branch that emits no marker.

### The arity-only fix this file first proposed is REJECTED — do not retry it

This file originally proposed keeping `take(10)` and appending the count. That is worse, and the
reason is the reason the defect exists: with `preserve_order`, head-only truncation always drops
the keys inserted **last** — in a report object the newest, and therefore the ones a caller has not
yet learned and is reaching for. Reporting the arity tells a caller something is missing without
ever showing them the thing they wanted. A two-ended window keeps both.

### What the record-holder verified, and what it cites

Verified at the bytes by the filing session:

- The regression test `json_path_key_miss_hint_states_its_elision_and_keeps_the_tail` is **green**
  in the default lane — `cargo test --workspace <name>`, 1 passed, exit 0. The workspace compiled,
  so a third session's dirty `src/librarian/indexer.rs` was not in the way. This was the *default*
  lane deliberately: it is the gate's terminal step and leaves a librarian-bearing
  `target/debug/codescout`, so verifying here cannot arm the next session's `cli_doc` trap.
- **The `Available …` hint family is closed at four sites, all in `file_summary.rs`** — headings
  (already windowed and marked, with its own archived bug file), JSON keys (this fix), and the TOML
  and YAML hints. Both of the latter `join` a `collect()` with no `take` in front of it
  (`t.keys().cloned().collect()`, `keys.into_iter().map(…).collect()`), so they cannot truncate
  silently; their risk is hint *size*, a different defect.

Cited from `ba061586` rather than re-derived, and the reason is not laziness: a mutation run edits
a file three sessions are building against, and this checkout already had one live dirty `src/`
file.

- Two mutations on the production path, both observed RED. `TAIL=0` → a head-only window **that
  marks itself**; `HEAD=100` → all 13 keys, no marker. The first is the one worth having, because
  the marker assertion alone would have passed it — which is why the test also carries an under-cap
  case, so an implementation that *always* marks cannot pass the other two.
- Full gate green (fmt, clippy, lean, default), with the test read out **by name** from both lanes
  rather than from either lane's total.

### One number in the fix commit does not reproduce — re-derive it, do not cite it

The commit message says *"The 39 `.take(N)` sites repo-wide are NOT audited"*. Counted at
`b0bc9c7a` under `src/**/*.rs`:

| population | matches | files |
|---|---|---|
| `\.take\([0-9]+\)` — integer literal | **58** | 33 |
| `\.take\([0-9A-Za-z_]+\)` — any single argument | **105** | 54 |

Neither is 39, and the fix had already removed one literal `take(10)` before these were counted,
so the pre-fix figures are one higher again. The same paragraph of that message states that *"a
count with an unstated unit is the thing this class is made of"* — and then carries one, which is
this corpus's own § *Testing Discipline* law firing inside the sentence that invokes it. Nothing
downstream depends on the figure. The **bounded** half of that paragraph — the four-site
`Available …` family — reproduces exactly and is verified above.

### The sibling site is the part that outlives this bug

The correct implementation was ~400 lines above **in the same file**: `resolve_section_range` had
the identical defect for headings, with the identical insertion-order bias, and had already been
fixed with a HEAD/TAIL window and a `(+N more)` marker under its own archived bug file. The sibling
enumeration was never swept. That is § *Testing Discipline* — *mutate once per guarded SITE, not
once per feature* — in an unusually sharp form: the remedy was not merely known, it was **merged,
in this file**, and still not applied at the second site.

### A correction retained, because the method changes how the number reads

The third elided key is `open_bug_source_citations`, not `move_candidates_detail`. The earlier
guess was wrong: `move_candidates_detail` is inserted conditionally (`doctor.rs:910`) and was absent
from that run's object entirely, so it was never elided at all. Kept because *13 keys* is a property
of one live run, not of the type — a conditionally-inserted key moves the population.
## Tests added

`json_path_key_miss_hint_states_its_elision_and_keeps_the_tail`
(`src/tools/file_summary/tests.rs`), shipped with the fix. Fixture: 13 insertion-ordered keys
`k00`–`k12`. Three assertions, each reding on a different wrong implementation:

| assertion | reds on |
|---|---|
| hint contains `(+3 more)` | silent truncation |
| hint contains `k10`, `k11`, `k12` | a head-only window — **even one that marks itself** |
| a 3-key object lists `a, b, c` and contains no `more)` | an implementation that *always* marks |

The pre-existing `extract_json_path_not_found` asserted only `is_err()`, so it was green through
the entire defect — monotone under exactly this failure.

**THE SHAPE THIS FILE ORIGINALLY SPECIFIED WAS ITSELF BLIND, AND IT IS THE SAME LAW.** This
section previously asked for two directions: *name the arity* on an >10-key object, and *no marker*
under the cap. Both pass against a head-only window that reports its own count correctly — the
exact implementation the author later mutated to (`TAIL=0`) and observed RED against the **third**
assertion, the surviving tail. So the specified test would have accepted the defect's near-twin.
The missing direction was not the marker but the *bias*: what a truncation drops, not whether it
admits dropping. Recorded because this file was written by a session applying § *Testing
Discipline* while specifying a test monotone under the widening it was guarding.

The fixture's two load-bearing details are annotated on their own lines in the test: the key count
must exceed `HEAD + TAIL` or the windowing branch never runs, and `k12` must be the final
insertion or the tail assertion stops discriminating.
## Workarounds

Treat the key list as a floor, never a census. To learn whether a key exists, **probe it** rather
than reading the enumeration:

```
read_file("@tool_<id>", json_path="$.<path>.<the-key-you-want>")
```

A resolution is proof of presence; absence from the hint is proof of nothing.

## Resume

**Nothing to resume.** Fixed at `b0bc9c7a` (patch-id `0708e3c5393628229c6716c8f1fa8f7c807ea1af`),
on `origin/experiments`, regression test green. Both sub-questions this section used to hold are
closed:

- The `:863` change — shipped, as a two-ended window rather than the arity this file first
  proposed. See § *Fix*.
- The YAML sibling — **decided: not this defect.** It `join`s a `collect()` with no `take` in front
  of it, as does the TOML hint, so neither can truncate silently. Their exposure is hint *size*,
  which is a different bug and is not filed.
## References

Line numbers are anchored to `38430896` — **pre-fix**. `b0bc9c7a` rewrote the JSON arm, so the
same lines now hold the windowed form, not the defect.

- `src/tools/file_summary/file_summary.rs:856-868` @ `38430896` — the arm and the `.take(10)`.
- `src/tools/file_summary/file_summary.rs:1238-1242` @ `38430896` — the YAML sibling, verified
  uncapped and out of scope (§ *Resume*).
- `src/librarian/tools/doctor.rs:950-951` — `audit` and `hint`, two of the three dropped keys.
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` — the class.
- Found by sessionId `ba061586-6581-4656-b0c5-acad83474de5`, who hit it while verifying whether
  their own audit fix was in a stale MCP server and nearly reported it absent. Root-caused and
  recorded by sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`; fixed by the finder. Their
  truncated-buffer diagnosis is kept above as hypothesis 1 because it is the one the envelope
  invites — and because the two diagnoses point at different files, which is what made
  discriminating them worth the probe.
- `docs/trackers/design-backlog-session-log.md` `F-10` — the adjacent staleness finding from the
  same incident. Distinct: `F-10` is `by_check` being scoped to a build; this is an enumeration
  capped irrespective of build.
