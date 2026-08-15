---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-06-14-usage-db-diagnostic-columns-unbackfilled.md
  - docs/trackers/2026-08-15-tool-usage-investigation.md
tags:
  - usage-db
  - analytics
  - taxonomy
  - silent-staleness
kind: bug
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

**Plan — derive the gate from the taxonomy instead of maintaining it.** Replace the integer
with a fingerprint over the set of families `normalize_err_family` can emit (e.g. a stable
hash of the sorted family-name list, stored in its own table or a `PRAGMA user_version`
successor). The backfill then re-runs whenever the stored fingerprint differs from the
current one. Adding an arm changes the fingerprint automatically, so the human step
disappears and the bug cannot recur.

This requires the family list to become enumerable — today it exists only as return values
scattered through an if-chain. Extracting a `const FAMILIES: &[&str]` that the classifier is
tested against is a prerequisite and is independently useful (it makes "every family has a
probe" testable).

**Consider alongside:** record the classifying taxonomy version *on the row*
(`err_family_version`), so a `NULL` from *"no arm exists"* is distinguishable from a `NULL`
from *"classified under an older taxonomy"* without inferring it from the DB-level marker.
That is what would have prevented the TU-5 misreading directly.

**Not in scope:** re-classifying the five frozen DBs. They are dead corpora; the correct
handling is to exclude them from ranking (now documented in the tracker), not to revive them.

## Tests added

`N/A — not yet fixed.` The existing `backfill_reruns_when_the_taxonomy_version_advances`
(`src/usage/db.rs`) covers the *mechanism*, not this bug; § Root cause explains why it
cannot. A regression test becomes possible once the family list is enumerable: assert that
mutating the family set changes the fingerprint.

## Workarounds

- **Bump `BACKFILL_VERSION` in the same edit as any `normalize_err_family` change.** The
  constant now carries a comment saying so, and a dated line per version.
- **Rank on live DBs only.** Filter to DBs at the current `user_version` before aggregating;
  never pool across versions.
- **Treat `err_family IS NULL` as ambiguous** until the per-row version exists — check the
  DB's `user_version` before concluding a family is missing from the classifier.

## Resume

Extract the emittable family list from `normalize_err_family` (`src/usage/db.rs`) into a
`const FAMILIES: &[&str]`, with a test asserting every entry is reachable from at least one
probe message. That unblocks both the fingerprint gate and the "every family is probed"
guard. Do this before adding further arms — each new arm added under the current scheme
widens the set of DBs whose history is silently frozen.

## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` — TU-5, and the § History entry
  recording the corrected headline and the re-rank.
- `docs/issues/archive/2026-06-14-usage-db-diagnostic-columns-unbackfilled.md` — the bug
  whose fix introduced this backfill and its version gate.
