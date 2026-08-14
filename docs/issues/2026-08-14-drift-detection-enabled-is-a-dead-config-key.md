---
id: a2d5ce3fe9211977
kind: bug
status: open
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
closed: null
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

Not implemented. The decision is what the key should become, and it is not
mechanical:

- **Remove it.** Drop the field from `src/config/project.rs` and
  `src/config/global.rs`, delete both manual rows, and remove companion's drift
  block. Honest and smallest. Costs the option of reviving the feature cheaply, and
  needs the companion change to land in the same cohort or session-start starts
  reading a key that no longer parses (harmless — it string-matches the file — but
  worth knowing).
- **Keep the key, fix the docs.** Mark it reserved/no-op in both manual pages.
  Cheapest, but a config key that does nothing is a trap for the next reader, and
  this bug is the second time the docs and this key have disagreed.
- **Reimplement drift on Qdrant.** Real work: it needs the prior generation of
  vectors retained, or a stored per-chunk embedding to diff against. Only worth it
  if the feature is actually wanted — check whether anyone used it before rebuilding.

Whichever is chosen, the companion half must be routed too, and that belongs with
`docs/trackers/2026-05-07-legacy-retrieval-removal.md` rather than duplicated here.

## Tests added

N/A — not fixed. A guard is easy for the remove-it path and worth adding with it: a
test asserting every `[embeddings]` config field is read somewhere outside the config
modules would catch this whole class. It would need an allow-list for
deliberately-reserved keys, which is exactly the honest place to record such a
decision.

## Workarounds

None needed — the flag is inert. Do not spend time toggling it to change drift
behaviour; there is no drift behaviour on the codescout side to change.

## Resume

Decide among the three options above. Before choosing "reimplement", establish
whether per-file semantic drift was ever used for anything — if the answer is
"companion printed warnings at session start and nobody acted on them," the remove
path is correct and the whole feature can go in one cohort with the companion block.

Do **not** start by editing the two manual pages in isolation: the docs are the
symptom, and fixing them alone leaves a dead config key that will re-grow
documentation.

## References

- `src/config/project.rs` — field definition + default
- `src/config/global.rs` — global-config merge
- `docs/manual/src/configuration/project-toml.md` — stale row, also cites a non-existent `threshold` param
- `docs/manual/src/configuration/embeddings.md` — stale row
- `docs/state-protocol.md` — already records the companion coupling and its removal condition
- `docs/trackers/2026-05-07-legacy-retrieval-removal.md` — owns the companion half
- `claude-plugins` → `codescout-companion/hooks/session-start.mjs` — the live reader of a dead table
- Found while fixing `docs/issues/2026-08-13-tools-semantic-search-manual-page-describes-legacy-interface.md`

