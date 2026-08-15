---
id: a2d5ce3fe9211977
kind: bug
status: fixed
title: 'BUG: drift_detection_enabled is a config key no codescout code reads, still documented as functional on two manual pages, and companion gates a dead sqlite query on it'
owners:
- marius
tags:
- config
- dead-code
- docs
- drift
- cross-repo
- legacy-retrieval
closed: 2026-08-15
opened: 2026-08-14
owner: marius
related:
- docs/issues/2026-08-13-tools-semantic-search-manual-page-describes-legacy-interface.md
- docs/trackers/2026-05-07-legacy-retrieval-removal.md
severity: medium
---

# BUG: `drift_detection_enabled` is a config key no codescout code reads, still documented as functional on two manual pages, and companion gates a dead sqlite query on it

## Summary

`[embeddings].drift_detection_enabled` parses, merges from global config, and has a
default — and then nothing in `src/` reads it. Setting it to `false` changes no
codescout behaviour, and setting it to `true` enables nothing. Two manual pages
still describe it as a working feature, one of them naming an output surface
(`workspace(action: status, threshold=...)`) that does not exist either.

Separately, codescout-companion's session-start hook *does* read it, and uses it to
gate a `SELECT … FROM drift_report` against the retired sqlite-vec database — a
table nothing populates any more.

## Symptom (Effect)

Three distinct wrong beliefs a reader can pick up:

1. From `docs/manual/src/configuration/project-toml.md:60` — that `index(action:
   build)` scores semantic drift per changed file, and that results are "queryable
   via `workspace(action: status, threshold=...)`". No `threshold` parameter exists
   on any tool; `IndexStatus::input_schema` declares no properties at all.
2. From `docs/manual/src/configuration/embeddings.md:148` — that the flag tracks
   "how much code meaning changes between index builds."
3. From companion's behaviour — that turning the flag on will produce session-start
   drift warnings. It will not: the query targets a dead table.

No error surfaces in any of the three cases. The flag is accepted, so config
validation passes, and the absence of output is indistinguishable from "nothing
drifted."

## Reproduction

```
grep -rn drift_detection_enabled src/
```

Measured 2026-08-14: hits in `src/config/project.rs` (lines 113, 115, 116, 234 —
doc comment, serde default, field, default fn) and `src/config/global.rs` (lines 20,
315, 323 — field, init, merge read). **No consumer.** Every
`config.embeddings.drift_detection_enabled` *read* in the repo lives under
`docs/superpowers/plans/` — historical plan text, not shipped code.

Companion side, in the `claude-plugins` repo:

```
codescout-companion/hooks/session-start.mjs:202
  driftEnabled = readFileSync(d.CS_CONFIG_FILE, 'utf8').includes('drift_detection_enabled = true');
codescout-companion/hooks/session-start.mjs:212
  .prepare('SELECT file_path, max_drift FROM drift_report WHERE max_drift > 0.1 ORDER BY max_drift DESC LIMIT 10')
```

## Environment

codescout repo, `experiments` branch. Cross-repo: the second half is
`claude-plugins`, `codescout-companion/hooks/session-start.mjs`.

## Root cause

Collateral from the sqlite-vec → Qdrant migration. Per-file semantic drift scoring
was a property of the legacy backend: it compared old and new chunk embeddings held
in `.codescout/embeddings.db` and wrote a `drift_report` table. The Qdrant stack
does not keep the previous generation of vectors around to diff, so the feature had
nowhere to live and its outputs (`drift_summary` on build, `drift` on status) were
dropped.

The **config key** was not dropped with them, and neither were the docs. This is the
same shape as the endpoint drift in
`docs/issues/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md`: a
half-applied change where the load-bearing code moved and the surrounding
declarations did not.

The companion coupling was foreseen — `docs/state-protocol.md:60` already records it:

> Companion's drift query depends on `meta` table and `drift_report` view; when the
> legacy index goes, companion drift detection must move to the new stack or be
> removed.

So the companion half is *known* debt owned by
`docs/trackers/2026-05-07-legacy-retrieval-removal.md`. The codescout-side dead key
and the two stale manual pages are not tracked anywhere, which is why this file
exists.

## Evidence

The replacement drift signal, for contrast, is narrower and real — `drift_note` in
`src/tools/semantic/semantic_search.rs`, set by `apply_worktree_plan_notes` when a
worktree's delta index is older than the main index it overlays. It is
worktree-scoped and has nothing to do with per-file semantic change, so it is not a
migration of the old feature and does not restore the old outputs.

## Hypotheses tried

1. **Hypothesis:** the key is read somewhere indirectly, e.g. via a serde-driven
   config consumer that greps wouldn't show. **Test:** searched the whole repo for
   the identifier, then specifically for `.drift_detection_enabled` field access.
   **Verdict:** rejected — the only field accesses are in the two config modules
   that define and merge it, plus plan documents.
2. **Hypothesis:** it is dead in codescout but alive in companion, so the key is
   still load-bearing as a cross-repo protocol field. **Test:** read
   `session-start.mjs:202,212`. **Verdict:** partially confirmed — companion reads
   the key, but the query it gates hits `drift_report` in the retired
   `embeddings.db`, which nothing writes. Alive as a *read*, dead as a *feature*.

## Fix

**Option 1 — remove it.** Shipped in `9b902e0a`.

Chosen over "keep the key, fix the docs" because a config key that does nothing is
a trap, and this was the *second* time the docs and this key disagreed — a
reserved/no-op row invites a third. Chosen over "reimplement drift on Qdrant"
because that needs a prior generation of vectors retained, and nothing suggests
anyone used the feature: the ask was never made, and the surface the manual
promised (`workspace(action: status, threshold=...)`) never existed to be used.
Git history keeps the revival path cheap.

Removed:

- `EmbeddingsSection::drift_detection_enabled` + `default_drift_detection_enabled`
  + the `Default` entry (`src/config/project.rs`)
- `GlobalEmbeddingsSection::drift_detection_enabled` (`src/config/global.rs`)
- rows/examples on `configuration/project-toml.md` (×3),
  `configuration/embeddings.md`, `tools/semantic-search.md`
- the `[memory] (drift_detection_enabled)` mention in `docs/state-protocol.md`
  — which named the wrong section too; the key lived under `[embeddings]`

Two tests used the field as a *vehicle* for testing merge machinery, not for its
own sake. Both were re-pointed at live fields:
`merge_toml_base_fills_missing_key` → `max_inflight`, and
`to_toml_value_emits_only_some_fields` widened to two sections so it still has a
`None` to discriminate against (`[embeddings]` was left with a single field, and
"emits only `Some`" is vacuous without a `None` in view).
## Tests added

`stale_drift_detection_enabled_key_is_ignored_not_rejected` — an existing
`project.toml` that still sets the key must parse with the key silently ignored,
not error on startup.

This is the assumption the whole removal rests on, so it is asserted rather than
assumed. It mirrors the precedent already in the file for the retired
`debug_enforce_symbol_tools` flag: `EmbeddingsSection` carries no
`deny_unknown_fields`, so unknown keys are skipped. The test also asserts that a
*neighbouring* key in the same section still lands — proof the stale key was
skipped rather than the whole `[embeddings]` table being dropped, which a bare
"it parses" assertion would not distinguish.

Gate: `cargo test --workspace` → **3805 passed / 0 failed / 50 ignored**;
clippy `--workspace --all-targets -D warnings` clean.
## Workarounds

None needed — the flag is inert. Do not spend time toggling it to change drift
behaviour; there is no drift behaviour on the codescout side to change.

## Resume

Closed. **The companion half needs no change for correctness**, which is the
opposite of what this file assumed.

`hooks/session-start.mjs:202` does:

```js
driftEnabled = readFileSync(d.CS_CONFIG_FILE, 'utf8').includes('drift_detection_enabled = true');
```

An absent key makes that `false`, so companion now **skips** its drift block —
exactly what is wanted, since the block queried the retired sqlite `drift_report`
table. The match is fail-closed.

`docs/state-protocol.md` claimed the opposite ("companion behaves as if drift
detection is on (default-true assumption)") and has been corrected. Worth noting
*how* that error survived: the contract doc stated a consequence of removal
without anyone having removed anything, so nothing ever tested the claim. A
predicted consequence in a contract doc is a hypothesis wearing a fact's clothes.

Remaining, cosmetic only: companion still carries a dead drift block and a
`drift_report` query against a retired table. Routed to
`docs/trackers/2026-05-07-legacy-retrieval-removal.md` rather than duplicated
here, as this file originally directed.
## References

- `src/config/project.rs` — field definition + default
- `src/config/global.rs` — global-config merge
- `docs/manual/src/configuration/project-toml.md` — stale row, also cites a non-existent `threshold` param
- `docs/manual/src/configuration/embeddings.md` — stale row
- `docs/state-protocol.md` — already records the companion coupling and its removal condition
- `docs/trackers/2026-05-07-legacy-retrieval-removal.md` — owns the companion half
- `claude-plugins` → `codescout-companion/hooks/session-start.mjs` — the live reader of a dead table
- Found while fixing `docs/issues/2026-08-13-tools-semantic-search-manual-page-describes-legacy-interface.md`
