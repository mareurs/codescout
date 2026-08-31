---
kind: bug
status: fixed
tags:
- librarian
- augmentation
- sidecar
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related:
- '689fb62e40557480'
severity: medium
unverified: Not yet re-verified against a rebuilt live MCP — the running server still answers from the pre-fix binary. Both paths are test-guarded through the tool's own call() entry point, so this is a freshness caveat rather than a coverage gap.
---

# BUG: `artifact_augment`'s sidecar write-through republishes the entire row, not the field you patched

## Summary
`sidecar_write_through` (the mechanism that keeps a committed `docs/augmentations/*.yaml`
sidecar in sync with the catalog) fires whenever *any* shape field changes on a
`merge=true` `artifact_augment` call, and on firing it re-serializes the **whole**
current catalog row — `prompt`, `params_schema`, `render_template`, `entry_collection`,
`append_mode`, `history_cap` — not just the field the caller intended to change. If any
of the untouched fields is stale in the catalog relative to the sidecar (or relative to
what a human actually wants published), a single-field patch silently republishes that
staleness over the correct on-disk value.

## Symptom (Effect)
While fixing `docs/trackers/provenance-subsystem.md`'s augmentation this session,
a `merge=true` call that supplied only `params_schema` (widening the `status` enum)
also rewrote `docs/augmentations/docs-trackers-provenance-subsystem.yaml`'s
`render_template:` block — a field the call never mentioned — to whatever the catalog's
`render_template` happened to hold. In this case that value was itself stale (authored
2026-08-28, during the cross-machine catalog-loss window), so the write silently
republished a wrong `render_template` into the git-committed sidecar even though the
call's caller believed only `params_schema` was in scope.

Observed via `git diff` after the round-1 fix commit: the diff hunk touched
`render_template:` even though the CLI invocation
(`artifact-augment e12cd7e0060ed9b8 --merge --params-schema @...`) named only
`params_schema`.

## Reproduction
1. Have an augmented artifact whose catalog `render_template` has drifted from what the
   committed sidecar *should* say (any content mismatch works; in this case the catalog
   held the loss-period template, described in the corrected fix at patch-id
   `af52dd707d001887d36da9d03afa7a3ba07e2211` — see the provenance note below).
2. Run `artifact_augment(id=..., merge=true, params_schema={...})` — i.e. patch a shape
   field that is *not* `render_template`.
3. Inspect `docs/augmentations/<slug>.yaml` afterward: `render_template:` (and `prompt:`,
   `entry_collection:`, etc.) have all been rewritten to the catalog's current values,
   not left untouched.

Git commit at time of observation: patch-id `c5ad1831aec43681cf9f429fd4367967be7fbb09`
(the Task 2 fix that first hit this), corrected at patch-id
`af52dd707d001887d36da9d03afa7a3ba07e2211`.

**Provenance note — both SHAs are gone, and how they went is the point.** They were
`061850c5` and `67c13cbc` on `experiments`. The 2026-08-31 cross-machine rebase
**dropped both** as *"patch contents already upstream"*: the other host had independently
re-exported the same sidecar at `4bb0c76e` (patch-id
`ead8d9235d1e845df1a108ed717243fad23f3f82`) and produced **byte-identical** content
(`sha256 83d16ed646b08ba1…`). So the fix is present upstream under a different diff, which
is why a patch-id lookup for the originals now returns nothing while the *file* is
correct. Cited by patch-id above per CLAUDE.md; the SHAs are recorded here as history,
not as pointers.

## Environment
codescout repo, `experiments` branch, cross-machine SDD run (desktop +
`marius@192.168.1.162` laptop, both applying the same `artifact-augment` CLI calls
against their own local catalogs). MCP + CLI code paths share the same
`sidecar_write_through` function, so this reproduces identically via either surface.

## Root cause
`artifact_augment`'s merge=true branch, when any of
`prompt`/`params_schema`/`render_template`/`entry_collection`/`append_mode`/`history_cap`
is provided, does a full `augmentation::upsert` that **preserves** every field the
caller did not pass (`src/librarian/tools/augment.rs:390-427`, e.g.
`a.render_template.clone().or_else(|| existing.render_template.clone())` at
`src/librarian/tools/augment.rs:414-417`) — so the *database* row is correctly merged,
field-by-field. It then unconditionally calls `sidecar_write_through(&cat, &a.id)`
(`src/librarian/tools/augment.rs:431`, mirrored at the `merge=false` call site,
`src/librarian/tools/augment.rs:218`).

`sidecar_write_through` (`src/librarian/tools/augment.rs:63-72`) wraps
`augmentation_sidecar::write_through` (`src/librarian/augmentation_sidecar.rs:207-251`),
which:
- re-fetches the **entire** current augmentation row —
  `crate::librarian::catalog::augmentation::get(cat, artifact_id)` at
  `src/librarian/augmentation_sidecar.rs:238` — not just the changed field;
- re-serializes the **whole** row to the sidecar shape —
  `serde_yml::to_string(&AugmentationSidecar::from_row(&row))` at
  `src/librarian/augmentation_sidecar.rs:241`;
- and only skips the write if the **entire rendered file** is byte-identical to what's
  on disk (`src/librarian/augmentation_sidecar.rs:246`) — a guard against no-op writes,
  not against partial ones.

So the merge logic that correctly isolates the *database* write (preserve what wasn't
passed) has no counterpart on the *sidecar* write (publish only what changed). Any
shape-touching call republishes 100% of the row's shape fields to the committed file,
including ones that were merely *preserved* from the DB rather than intentionally
authored — and if the DB's preserved value is itself wrong, the sidecar now says so too.

measured 2026-08-31: read `src/librarian/tools/augment.rs:390-433` and
`src/librarian/augmentation_sidecar.rs:207-251` directly (via `symbols`/`read_file`
force=true) and confirmed both the preserve-on-DB-write and republish-whole-row-on-sidecar-write
behaviors in the literal source, plus the two call sites at
`src/librarian/tools/augment.rs:218` and `:431` (`grep sidecar_write_through\(`).

## Evidence
### `src/librarian/tools/augment.rs:385-396` — the merge=true trigger condition
```rust
if a.prompt.is_some()
    || a.params_schema.is_some()
    || a.render_template.is_some()
    || a.entry_collection.is_some()
    || a.append_mode.is_some()
    || a.history_cap.is_some()
{
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
```
Any one of six fields being `Some` enters the branch that ends in `sidecar_write_through`.

### `src/librarian/augmentation_sidecar.rs:237-251` — the write-through body
```rust
    let Some(row) = crate::librarian::catalog::augmentation::get(cat, artifact_id)? else {
        return Ok(None);
    };
    let rendered = serde_yml::to_string(&AugmentationSidecar::from_row(&row))
        .context("serializing augmentation sidecar")?;
    // Byte comparison, so a call that changes no shape leaves the file — and its mtime —
    // untouched. `params` are not part of this rendering, so a params-only write is a no-op
    // here by construction rather than by the caller remembering to skip it.
    if std::fs::read_to_string(&path).is_ok_and(|on_disk| on_disk == rendered) {
        return Ok(None);
    }
    std::fs::write(&path, rendered).with_context(|| format!("writing {}", path.display()))?;
    Ok(Some(rel))
}
```
The comment documents the params-only no-op case accurately, but says nothing about the
partial-shape-write case — the code has no such guard, only a whole-file byte comparison.

### Displaced value's recovery path
The correct `render_template` this write-through overwrote was not lost: it was
recovered from `~/.local/share/librarian/catalog.db.bak-20260831-preintegration` (the
pre-integration catalog backup) and re-applied at patch-id
`af52dd707d001887d36da9d03afa7a3ba07e2211` (see the provenance note above).
Had that backup not existed, the only record of the correct template would have been
whatever the sidecar held in git history before the trap fired.

## Hypotheses tried
1. **Hypothesis:** the write-through only republishes the field(s) actually passed in
   the call. **Test:** read `write_through`'s body and the `AugmentationSidecar::from_row`
   call site directly. **Verdict:** rejected — `from_row(&row)` takes the full
   post-upsert row, with no field-level diffing against what the caller passed.
   **Evidence link:** Evidence § write-through body.

## Fix

**Fixed on `experiments` at `6ae7d39a`** — patch-id
`f36fb473f8735539999ead742a811a2ea2035ceb`.

### The framing that produced the fix

This report's mechanism is exact and its *harm* statement is the one that led somewhere: not
"it publishes six fields" but **what that does in front of `sidecar_shape_drift`.**

That check's entire design position — stated at length in its own doc comment and repeated in
every finding it emits — is that when the row and the committed file disagree, **the direction
is undecidable without a human**: *"if the SIDECAR is right (you pulled someone else's shape
change), do NOT export — that would overwrite their shape with your stale row."*

The write-through was doing precisely that, automatically, catalog-wards, on a call that named
a different field. A mechanism that silently resolves a conflict in a fixed direction, standing
in front of a check built on the premise that the direction cannot be decided, is the defect —
and it explains why the loss was silent rather than merely surprising.

### What shipped

A disagreement on an **unauthored** field is now reported instead of resolved.

`write_through` takes an `Authored` argument. `Authored::Only(&[…])` for a merge call — the
fields it actually passed, since the upsert *preserves* the rest and a preserved field was
never authored by this caller. `Authored::All` for a replace, which speaks for the whole shape
including the omissions it resets. When a merge call would overwrite a field it did not author
and the sidecar disagrees on it, **nothing is written** and the caller gets a
`RecoverableError` naming the fields. The field the call *did* set travels on the re-run, once
the drift is resolved.

It reuses `drifting_fields` rather than adding a second comparator, so the refusal fires on
exactly the set `sidecar_shape_drift` reports. That function's exhaustive destructure then
covers this path too: adding a shape field breaks compilation there until it is compared, so
a new field cannot silently become unrefusable.

### The filed direction was rejected, and not for the reason first supposed

This section proposed publishing **only the named fields**, diffing field-by-field. My first
objection was that it reopens `689fb62e` (*"a schema edit silently does not travel"*). **That
objection was wrong** and is recorded because it was nearly acted on: a partial write still
publishes the field you edited, so the edit still travels.

The real objection is different. Row-wide rendering is what makes the sidecar *the catalog
row's committed projection*; a partial file matches no catalog row anywhere, so a fresh clone's
`reindex` would restore a blend of two hosts' shapes. The defect was publishing **without a
mandate**, not publishing **whole** — and once put that way the remedy is to withhold the write,
not to narrow it.

An unparseable sidecar still falls through and is overwritten, unchanged. That is
`sidecar_unparseable`'s case, and refusing there would block a repair on the one file nothing
else can read.
## Tests added

Two, in `src/librarian/tools/augment.rs`, driven through `ArtifactAugment.call` so they guard
the tool's behaviour rather than an internal return. RED first: the refusal test failed with
`String("ok")` — the bug returning success.

- `a_merge_call_refuses_to_republish_a_shape_field_it_did_not_set` — the guard. It asserts the
  error is a `RecoverableError`, that it **names** the field, and — the assertion that matters
  — that the sidecar still holds its original value afterwards. A refusal that already
  destroyed the value on the way to refusing is not a refusal, and only that third assertion
  can tell the difference.
- `a_replace_call_publishes_the_whole_shape_including_fields_it_omitted` — pins the guard
  **off** for `merge=false`, whose caller really does speak for every field including the ones
  it resets. It passed before the fix as well as after: its job is to fail against an
  over-broad guard, not against the pre-fix code.

**The non-vacuity guard already existed**, which is why no third test was written:
`a_merge_true_sibling_change_writes_through_too` asserts that a named field *does* still reach
the sidecar. Without it, this change is satisfied by refusing everything — and it would have
caught that, since it drives the same merge path.

**The fixture's load-bearing detail is annotated on its own line:** the committed sidecar must
hold a `render_template` the row does not. That asymmetry *is* the case under test — it is the
shape of the measured incident, where the on-disk value was correct and the row's was the
loss-window leftover. Make the two agree and the test passes while testing nothing.

Counts: default 4975 → **4977**; lean unchanged at 3412, correctly — `augment.rs` sits behind
the `librarian` feature, which the lean lane has off.

Clippy caught a redundant `.into()` that both test lanes passed over — the long
`--all-targets` form earning its place in the gate again.
## Workarounds
Before any `merge=true` `artifact_augment` call that touches only some shape fields,
diff the sidecar immediately after the call and confirm every field that wasn't
intentionally changed still matches its pre-call value — do not assume merge=true's
DB-level field preservation extends to the sidecar write.

## Resume
No open investigation — this is a filed observation, not an active fix. If picked up:
read `src/librarian/tools/augment.rs:377-433` and `src/librarian/augmentation_sidecar.rs:207-251`,
then decide whether `write_through` should accept a set of "fields this call actually
changed" so it can diff-and-patch the sidecar instead of blanket-republishing the row.

## References
- Inverse bug (the fix that introduced this side effect): `docs/issues/archive/2026-08-30-export-augmentations-will-not-rewrite-a-sidecar-whose-shape-changed.md`
  (artifact id `689fb62e40557480`, fixed at `5f88be65`, patch-id `59ba22f9d7a6dfed66fcd8e551e09455b5c58f32`).
  That bug was `export_augmentations` never refreshing a stale sidecar; this bug is the
  same write-through mechanism now refreshing *too much* of it on an unrelated field change.
- Fix-round commit that recovered the displaced value: patch-id
  `af52dd707d001887d36da9d03afa7a3ba07e2211` (SHA was `67c13cbc`, dropped in the
  2026-08-31 rebase as already-upstream — see *Provenance note* above). This entry is
  why the pair is recorded rather than the SHA alone: the SHA died the same day it was
  written and the patch-id is still the only thing that finds the change.
- `.superpowers/sdd/2026-08-31-cross-machine-catalog-recovery/task-2-report.md` — full
  session narrative (Task 2 + fix round 1).
