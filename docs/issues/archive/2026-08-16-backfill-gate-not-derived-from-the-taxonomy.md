---
kind: bug
status: fixed
tags:
- usage-db
- analytics
- taxonomy
- silent-staleness
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-06-14-usage-db-diagnostic-columns-unbackfilled.md
- docs/trackers/2026-08-15-tool-usage-investigation.md
severity: medium
---

# BUG: the err_family re-classification gate is a hand-maintained integer nothing derives from the taxonomy

## Summary

`backfill_legacy_rows` re-classifies historical `err_family` values only when
`PRAGMA user_version < BACKFILL_VERSION`. `BACKFILL_VERSION` is a hand-edited constant
with **no derivation from, and no enforced relationship to, the contents of
`normalize_err_family`**. Extend the classifier without bumping it and every
already-backfilled `usage.db` keeps its historical rows `NULL` — the new families tag
only future rows. The failure is completely silent: no error, no warning, no failing
test, and the resulting ranking looks authoritative.

A second facet of the same mechanism: the backfill runs **only on DB open**, so a
project that is never activated again can never be re-classified at all, whatever the
version says.

## Symptom (Effect)

No error is emitted. The observable symptom is a *ranking that silently describes the
wrong population* — which is exactly how TU-5's headline came to be wrong by 11
percentage points.

Measured 2026-08-15, across every `usage.db` under `~/work/claude/*`:

```
claude-plugins       v=2  errors=19    unclassified=4
code-explorer.old    v=0  errors=1652  unclassified=600
codescout            v=3  errors=923   unclassified=26
headroom             v=0  errors=63    unclassified=48
hermes-agent         v=0  errors=14    unclassified=8
opencode             v=0  errors=1     unclassified=0
pi                   v=2  errors=15    unclassified=3
playground           v=1  errors=3     unclassified=0
prompt-engineering   v=2  errors=31    unclassified=5
researcher           v=2  errors=11    unclassified=1
topictracker         v=0  errors=26    unclassified=4
whatsapp             v=2  errors=1     unclassified=0
```

Three distinct `user_version` values are live at once (0, 1, 2, 3). Rows classified
under four different taxonomies sit in one queryable corpus with nothing on the row
recording which taxonomy classified it.

## Reproduction

```bash
# 1. Note the current constant and a live DB's marker.
#    src/usage/db.rs — const BACKFILL_VERSION
sqlite3 ~/work/claude/codescout/.codescout/usage.db "PRAGMA user_version;"

# 2. Add an arm to normalize_err_family (src/usage/db.rs) WITHOUT bumping
#    BACKFILL_VERSION. Build and reconnect:
cargo rb   # then /mcp

# 3. Rows whose error_msg the new arm matches are still NULL:
sqlite3 ~/work/claude/codescout/.codescout/usage.db \
  "SELECT count(*) FROM tool_calls WHERE err_family IS NULL AND error_msg LIKE '%<new pattern>%';"
```

The full suite stays green throughout — including
`backfill_reruns_when_the_taxonomy_version_advances`, for the reason in § Root cause.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio transport. Twelve
`usage.db` files under `~/work/claude/*`; 30-day retention enforced on write.

## Root cause

`backfill_legacy_rows` (`src/usage/db.rs`) opens with:

```rust
let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
if current >= BACKFILL_VERSION {
    return Ok(());
}
```

The gate compares a *stored* integer against a *compile-time* integer. Nothing computes
either from the classifier, so the invariant "the taxonomy changed ⇒ re-classify" is held
only by a human remembering to edit a second location. **The correctness of the analytics
depends on a convention, not on the code.**

`measured 2026-08-15: sqlite3 <db> "PRAGMA user_version"` across 12 DBs → four distinct
versions live simultaneously (table in § Symptom).

**Why no test can currently catch it.** The obvious guard —
`backfill_reruns_when_the_taxonomy_version_advances`, added the same day — seeds a DB at
`BACKFILL_VERSION - 1` and asserts a family appears on reopen. That proves the *mechanism*
re-runs on advance, but it cannot detect a taxonomy that grew *without* advancing: seeding
below the constant makes the backfill run unconditionally, so the probe family fills either
way. This limitation survived one rewrite of the test and is documented here so the next
reader does not mistake the test's presence for coverage of this bug.

**Second facet — open-triggered only.** The backfill is invoked from `open_db`, so a DB is
re-classified only if something opens it. Five DBs (`code-explorer.old`, `headroom`,
`hermes-agent`, `topictracker`, `opencode`) sit at `user_version=0` with a last write in
May/June; they never received the iron-law taxonomy and never will, because no session will
open those projects again.

## Evidence

### The 77% distortion this produced

TU-5 (`docs/trackers/2026-08-15-tool-usage-investigation.md`) reported *"855 of 2,751 errors
(31%) have `err_family IS NULL`"* and recommended extending the classifier. Splitting the
same corpus by `user_version` shows what that number was actually measuring:

| Population | Unclassified | Note |
|---|---|---|
| Frozen (`v=0`, last write May/Jun) | **660 (77%)** | never re-openable |
| Live (`v=2`, actively written) | 197 | the real surface |

660 + 197 = 857 exactly. The actionable rate was **197/997 = 19.8%**, not 31%. The headline
measured dead history because the corpus carries no marker distinguishing the two, and
`err_family IS NULL` reads identically whether it means *"the classifier has no arm for this"*
or *"this row predates the arm."*

**That conflation is the core harm.** `NULL` is overloaded across two unrelated causes, and
no column separates them.

## Hypotheses tried

1. **Hypothesis:** live DBs are stale because the version was never bumped when the
   iron-law arms were added.
   **Test:** query codescout's live DB for NULL rows whose message the *current* classifier
   would match.
   **Verdict:** **rejected.** The one candidate was `edit_markdown only supports .md files`,
   and the wrong-ext arm is scoped to `tool_name == "read_markdown"` — correctly
   unclassified, a coverage gap rather than staleness. `BACKFILL_VERSION` had been bumped
   properly in `78dac6bd`.
   **Evidence:** § Symptom table — `codescout` sits at the current version.

2. **Hypothesis:** the residue is dead-project DBs frozen at an older version.
   **Test:** `PRAGMA user_version` + unclassified count per DB; check the arithmetic against
   the lifetime total.
   **Verdict:** **confirmed.** 660 + 197 = 857, exact.
   **Evidence:** § Evidence, the 77% table.

## Fix

Fixed on `experiments`. The gate is now **derived**, not maintained.

**`const ERR_FAMILIES: &[&str]`** — all 38 families the classifier can emit, sorted and
deduplicated (39 `return Some(…)` sites; `heading_not_found` fires from two). This is the
prerequisite the Resume named: before it, the family set existed only as return values in an
if-chain and nothing could enumerate it, which is precisely why the gate had to be an integer
a human maintained.

**`fingerprint_families()` / `err_family_fingerprint()`** replace `const BACKFILL_VERSION:
i64 = 4`. FNV-1a over the sorted list, written out rather than delegated to `DefaultHasher` —
std does not guarantee that hasher's output across Rust releases, and this value is
*persisted*, so a silent implementation change would re-run the backfill on every DB on every
open forever with nothing saying why. The result is forced odd and positive so it can never
collide with `user_version`'s default of `0`, which must always read as *never backfilled*.

**The gate compares for equality, not `>=`.** A fingerprint is not ordered. Any DB carrying
another value — including one of the old sequential versions — re-runs the backfill once and
is then stamped. The re-run only fills `NULL` families, so the migration costs one pass and
repairs any DB that was silently stale.

**`user_version` was safe to repurpose**: it carried the backfill gate and nothing else — the
schema migrations in `open_db` all probe for columns (`SELECT <col> … LIMIT 0`) rather than
consult a version, so no schema decision rides on it.

### On the per-row `err_family_version` column

The filing listed this under *Consider alongside*, as the thing that would have prevented the
TU-5 misreading directly. **Deliberately not built**, with a reason that is checkable rather
than asserted: with a derived gate, any DB that is *opened* converges — mismatch →
re-classify → stamp — so after an open, `err_family IS NULL` unambiguously means *no arm
exists*. The overloading the filing described survives only in DBs that are never opened
again, and re-classifying those is already out of scope. The DB-level question *"is this
corpus current?"* is now well-defined (`user_version == err_family_fingerprint()`), where
before it meant *"compare against a constant someone may have forgotten to bump"*.

If per-row provenance is wanted later it is an additive nullable column, and the schema-
migration ordering hazard applies (every later rebuild's `INSERT … SELECT` is a silent
allow-list).
## Tests added

**`err_families_lists_exactly_what_the_classifier_can_emit`** — the guard that deletes the
human step. It `include_str!`s this file's own source, slices off the test module (so a
`return Some(…)` written in a test never reads as a family), regex-scans the classifier, and
asserts set equality with `ERR_FAMILIES` **in both directions**, plus sorted-ness and
no-duplicates. Written first and watched fail against `ERR_FAMILIES = &[]` — the failure
output *enumerated all 38 families*, which is how the const was populated.

It also asserts the scan found something at all: if the classifier's shape ever changes, a
vacuous guard is worse than a failing one.

**Mutation-verified against the actual bug.** A new arm (`return
Some("zz_mutation_probe_family")`) was added to `normalize_err_family` **without** touching
`ERR_FAMILIES` — the exact move that silently froze every DB under the old scheme. The guard
failed, naming the offending family and telling the developer what to do. Then reverted. The
old `backfill_reruns_when_the_taxonomy_version_advances` could not do this by construction,
and § Root cause explains why.

**`extending_the_taxonomy_moves_the_backfill_fingerprint`** — adding a family moves the
marker, and so does *renaming* one: a same-size taxonomy is still a different taxonomy, which
the old sequential scheme could not see either.

**`the_family_fingerprint_is_stable_odd_and_never_zero`** — pins the hash against a fixed
vector (`fingerprint_families(&["a"]) == 0x12926369`) rather than against `ERR_FAMILIES`, so
adding a family stays a one-line change while an accidental rewrite of the hash still fails.
Also pins the separator (without it `["ab","c"]` and `["a","bc"]` fingerprint alike) and the
odd/positive invariant.

*The pinned constant was computed by hand first and was wrong; an independent `python3`
evaluation of FNV-1a agreed with the Rust implementation, not with the hand arithmetic. Noted
because "the test now passes" would have been the wrong reason to adopt the value the code
emitted.*

**`backfill_reruns_when_the_stored_marker_does_not_match`** — the old version-advance test,
rewritten. It now seeds a pre-fingerprint sequential marker (the state every real `usage.db`
is in), asserts the re-classification happens, **and** asserts the DB is stamped with the
current fingerprint afterwards. Its doc comment states plainly what it does not cover, since
the previous version was mistaken for coverage of this bug.

Gate: **3968 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

**All three are obsolete as of the fix** — kept because they describe the pre-fix world and
because the reasoning behind the third is still worth knowing.

- ~~Bump `BACKFILL_VERSION` in the same edit as any `normalize_err_family` change.~~ The
  constant no longer exists. Adding an arm now fails
  `err_families_lists_exactly_what_the_classifier_can_emit` until the family is listed in
  `ERR_FAMILIES`, and listing it moves the fingerprint by arithmetic.
- ~~Rank on live DBs only, filtering by `user_version` before aggregating.~~ Still the right
  instinct, and now expressible exactly: a DB is current iff
  `user_version == err_family_fingerprint()`. Previously this meant "compare against a
  constant someone may have forgotten to bump", which is why the filter could not be trusted.
- ~~Treat `err_family IS NULL` as ambiguous.~~ For any DB that has been *opened* since the
  fix, it is not: an open converges the DB, so `NULL` means *no arm exists*. It remains
  ambiguous for the frozen corpora that will never be opened again — which is the same set
  that must be excluded from ranking anyway.
## Resume

None — the derivation is in place and the human step is gone.

The chain that now holds it: a new arm fails the source-scan guard → the developer adds the
family to `ERR_FAMILIES` → the fingerprint changes by arithmetic → every DB re-classifies on
its next open. No step in that chain depends on anyone remembering anything.

Still true and still out of scope: the five DBs frozen at `user_version=0` with last writes
in May/June will never re-classify, because nothing will open them. Exclude them from ranking
— which is now a one-expression test (`user_version != err_family_fingerprint()`) rather than
a comparison against a constant.
## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` — TU-5, and the § History entry
  recording the corrected headline and the re-rank.
- `docs/issues/archive/2026-06-14-usage-db-diagnostic-columns-unbackfilled.md` — the bug
  whose fix introduced this backfill and its version gate.
