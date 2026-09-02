---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md
- docs/issues/archive/2026-09-02-a-byte-ceiling-test-cannot-see-a-member-stop-delivering.md
- docs/superpowers/plans/2026-08-27-get-guide-section-grain.md
severity: medium
unverified: no remedy attempted here — decomposition is out of scope for fix round 1, and trimming the just-landed `artifact.move` section would edit another session's work; whether the deficit (427 B) closes on its own as other sections shrink, or needs an explicit trim, is unknown.
---

# BUG: a freshly-corrected byte ceiling reds within 20 minutes of landing, on a shared checkout

## Summary

`fix-round-1` for Plan 3 Task 2 corrected `CEILING` in
`a_p50_session_stays_under_the_committed_emission_byte_ceiling`
(`src/server.rs`) from an undeliberated 13,300 to a deliberate 12,244 (the
12,116 B measured p50 total plus the 128 B margin precedent). Before that
fix could even be committed, an unrelated, concurrent, legitimate commit
(`35b9ef71`, landed 20:15:20 — a doc fix to the `artifact.move`-served
section of `src/prompts/guides/librarian.md`) grew the real total to
12,671 B, 427 B **over** the corrected ceiling. The test now fails on
`experiments` HEAD, for a reason fix round 1 explicitly may not use to
fix it: raising `CEILING` again is the exact "spec amendment, not a fix"
move the test's own panic message and Fix 1's ruling both forbid.

## Symptom (Effect)

```
thread 'server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling' panicked at src/server.rs:8694:9:
p50 session emitted 12671 B after the primary (whole librarian topic is 23895 B, ceiling 12244 B, margin 0 B). Raising CEILING is a spec amendment, not a fix — it is not the remedy for this failure. The standing remedy is decomposing § Body Editing Surfaces in the librarian guide, already recorded in `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` § Out of scope for Phase 1. If that is not what happened here, check whether a section grew past its own per-section cap (a separate gate/test) or a `serves:` declaration is broader than intended.
test server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 measured; 5014 filtered out; finished in 0.11s
```

Deterministic — re-run twice, identical `12671 B` both times.

## Reproduction

At `experiments`, HEAD `450d34fd` (2026-09-02 20:32:57 +0300), with fix
round 1's `src/server.rs` `CEILING = 12_244` staged but not yet committed:

```
cargo test --lib a_p50_session_stays_under_the_committed_emission_byte_ceiling -- --nocapture
```
→ FAILED, `total = 12671`, `CEILING = 12244`, deficit 427 B.

## Environment

`experiments`, shared checkout, active peer sessions (this session
observed at least two others landing commits — `c45dd5ef-...` at
20:15:20, and a third whose in-progress `src/librarian/catalog/find.rs`
work blocked this session's compile from 20:39 to ~20:42 — during this
one gate run).

## Root cause

`CEILING` is a single fixed number checked against a corpus
(`src/prompts/guides/librarian.md`, 23,895 B, six `serves:`-routed
sections) that multiple concurrent sessions edit continuously and
independently — each edit individually correct, reviewed, and gated on
its own. The ceiling has no absorption budget for organic growth between
the moment it is measured and the moment it is committed on a checkout
this active; measured here at ~530 B/17 min (the gap between `35b9ef71`
at 20:15:20 and this run), which is not a rate this fix can be sized
against without becoming exactly the "10% headroom" move Fix 1 rejected
for the opposite reason (it would re-loosen the gate on the only axis it
has ever caught anything on).

Confirmed by direct attribution, not inferred: `git show 35b9ef71 --
src/prompts/guides/librarian.md` shows +8 net lines added inside the
`<!-- serves: artifact.move, artifact.delete -->` section — exactly one
of the six shapes this test sums. `git status --short --
src/prompts/guides/` is clean (no uncommitted guide edits), so the
growth is fully attributable to already-committed history, not to any
in-progress peer working-tree state. `d94dd53d` (2026-08-31, +19 lines,
same file) is the only other recent contributor found by inspection but
predates whatever baseline `12,116` was measured against, so is not
double-counted here.

*measured 2026-09-02 20:4x (this session): `cargo test --lib
a_p50_session_stays_under_the_committed_emission_byte_ceiling
-- --nocapture` twice, both `12671`; `git show 35b9ef71 --
src/prompts/guides/librarian.md` for the attribution.*

## Evidence

### E1 — the growth is entirely in one `serves:`-routed section

```
diff --git a/src/prompts/guides/librarian.md b/src/prompts/guides/librarian.md
@@ (inside "## Archiving / Moving Trackers", serves: artifact.move, artifact.delete)
-"history_grafted": {"events": 3, "links": 1, ...}, "moved": true}
+"history_grafted": {"events": 3, "links": 1, ...},
+ "stage_together": ["<old>", "<new>"], "stage_hint": "...", "moved": true}
+
+**`stage_together` names both halves, and staging both is not optional.** [...7 more lines...]
```

This is a legitimate, well-justified fix (repairs a stale response example
that a different bug, `F-103`, had already flagged) — not scope creep,
not an accidental `serves:` broadening. The class this bug records is
that the ceiling has no way to distinguish "this growth is legitimate"
from "this growth should trip the gate," because it only sees the sum.

## Hypotheses tried

1. **Hypothesis** — the deficit is caused by fix round 1's own diff (a
   regression I introduced).
   **Test** — checked whether `12671 <= 13300` (the pre-fix-round-1
   ceiling); it is, so the tree was and remains green under the OLD
   ceiling, and only fails once the corrected, tighter `CEILING = 12244`
   is applied.
   **Verdict** — rejected. Fix round 1's diff does not touch any guide
   content; it only tightens the threshold the content is compared
   against, which is precisely what exposed this.
2. **Hypothesis** — an in-progress peer's uncommitted working-tree edit
   (the `find.rs` compile break active during this same session) is
   inflating the guide content.
   **Test** — `git status --short -- src/prompts/guides/` → clean.
   **Verdict** — rejected. All growth is committed history.

## Fix

Not attempted here — out of scope for fix round 1, which is explicitly
scoped to the five review findings in
`.superpowers/sdd/2026-09-02-layer-2a-3-wiring-and-one-budget/fix-round-1-brief.md`
and explicitly forbids re-raising `CEILING` as a remedy. Two directions
for whoever picks this up:

- **Decomposition** (the standing remedy the panic message already
  names): `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md`
  § *Out of scope for Phase 1* — presumably now in scope, given this is a
  second measured overrun.
- **A targeted trim** of the `artifact.move`-served section, following
  the `2026-08-28` precedent cited in Fix 1's own comment (compress
  ~430+ B rather than raise the ceiling) — but this edits a section
  another session (`c45dd5ef-...`) landed 17 minutes before this bug was
  filed, for a documented and reviewed reason; trimming it without that
  session's context risks re-introducing the stale-example defect
  `35b9ef71` just fixed.

## Tests added

None — the failing test already exists and already discriminates
correctly; this file records why it is currently red and why fix round 1
does not attempt to make it green.

## Workarounds

None. `cargo test --workspace` is red on `experiments` HEAD (plus fix
round 1's staged, uncommitted diff) until one of the § *Fix* directions
lands.

## Resume

Decide between decomposition (bigger, addresses the class) and a
targeted trim (smaller, addresses this instance) — read
`get-guide-section-grain.md` § *Out of scope for Phase 1* first, since a
second measured overrun in the same day may be the trigger that
retires that scoping decision. Re-run the reproduction command above
before trusting any stale byte figure — this checkout's guide content
moves fast enough that the number in this file is already a historical
snapshot by the time it is read.

## References

- `src/server.rs:8575` — `CEILING`, corrected by fix round 1.
- `src/prompts/guides/librarian.md` — the corpus; the
  `artifact.move`-served section specifically.
- `35b9ef71` — the commit that grew it, landed during this session's gate
  run window.
- `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` § *Out
  of scope for Phase 1* — the standing decomposition remedy.
- `.superpowers/sdd/2026-09-02-layer-2a-3-wiring-and-one-budget/fix-round-1-brief.md`
  — the ruling this bug does not act against.
