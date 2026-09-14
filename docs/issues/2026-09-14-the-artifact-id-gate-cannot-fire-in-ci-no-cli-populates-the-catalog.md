---
kind: bug
status: open
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: the artifact-id gate runs in CI and can never fire there — no CLI surface populates the catalog

## Summary

`audit_doc_refs`' `ArtifactId` check resolves cited 16-hex ids against the librarian catalog's
`artifact` table. **The catalog is machine-local and gitignored**, and the `Audit Doc Refs` job
seeds an empty workspace (`touch workspace.toml`) and never indexes. So in CI that table holds
**zero rows**.

Since `ae6dc663` (patch-id `dc984ea9668ba34ac18a38cb1c2473393bdfcc71`), an empty table disables the check rather than marking
every citation dead — which stops the false reds. The residual defect is the other half: **the gate
is registered on the one job it was armed for and cannot fire there.** Every piece is individually
correct — parser, resolver, severity chain, `--fail-on high`, the CI step — and nothing connects
the catalog to the job.

## Symptom (Effect)

Before the disabling fix: `Audit Doc Refs` red with 50 `artifact_missing` findings at `high`, **all
false positives**. All 37 distinct ids resolved on a developer machine with their files on disk.
Unpassable by any corpus change: a citation stops being "missing" against an empty table only by
ceasing to exist.

After it: the job is green and the check contributes nothing. A genuinely dead citation reaching
`master` is caught by no CI job. That is strictly better than a permanently red gate and it is not
the intended state.

## Reproduction

```bash
cargo build --release --bin codescout
tmp=$(mktemp -d); mkdir -p "$tmp/data" "$tmp/ws"; touch "$tmp/ws/workspace.toml"
XDG_DATA_HOME=$tmp/data LIBRARIAN_WORKSPACE=$tmp/ws/workspace.toml \
  ./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
python3 -c "import sqlite3;print(sqlite3.connect('$tmp/data/librarian/catalog.db')\
  .execute('SELECT count(*) FROM artifact').fetchone()[0], 'artifacts')"
```

`XDG_DATA_HOME` is what makes this reproducible — `dirs::data_local_dir()` honours it
(`src/librarian/mod.rs:98-100`), so CI's catalog conditions are constructible on any machine.

## Root cause

**There is no CLI path that writes the `artifact` table**, and more importantly there is no
path that would write the *right* ids. Measured 2026-09-14 against the release binary:

| tried | result |
|---|---|
| seed a real `[[roots]]` entry instead of an empty workspace | 0 artifacts — the audit does not reindex |
| `codescout index` | 0 artifacts — populates the **semantic** index (`added=5348`), not the catalog |
| `codescout doc find/get/create/…` | no reindex verb |
| a purpose-built `--reindex` flag | 1683 artifacts — **and 50+ false positives at a foreign path** |

`librarian(action="reindex")` is an **MCP tool only**. That was the first reading of this
bug and it was the shallow one: the missing CLI verb is real and fixing it changes nothing,
because the blocking property is that ids are keyed on an absolute path rather than on
anything a second checkout shares.

Two further facts found while measuring, each of which defeated an experiment before the
clone finally answered it: `reindex` refuses a root under a temp path
(`reindex_refuses_temp_root_into_real_catalog`), and it **skips linked git worktrees**
outright — *"skipping index of linked git worktree … index its main worktree instead"*. So
neither a temp dir nor a worktree can stand in for a foreign checkout; only a real clone at
a real path can.
## Evidence

The empty-catalog case was named in the code's own comment at
`src/librarian/tools/audit_doc_refs/mod.rs` before it happened — *"an empty set would mark every
cited id dead and turn one unreadable catalog into hundreds of confident, wrong findings on a gated
job"* — while the guard beside it covered only the read-**error** case. The author held the warning
and shipped the gap under it.

**Why local verification could not predict this.** The gate was reconciled to zero on a developer
machine whose catalog is a strict superset of CI's: it carries rows for sibling repos and for
artifacts at pre-archive paths. An id that resolves against a stale local row is dead in CI. The
instrument used to certify the tightening was structurally incapable of expressing the failure —
`cluster/repro-env-diverges-from-gate-env`, and the reason this file is tagged for the *other* half.

## Fix

Not done, and **direction 1 below has been falsified by measurement — do not build it.**

1. ~~**Expose reindex on the CLI**, then add a step to the `Audit Doc Refs` job.~~ **BUILT,
   MEASURED, REVERTED 2026-09-14.** `--reindex` does populate the catalog (1683 artifacts
   from an empty workspace). It does not help, because `id = sha256(ABSOLUTE path)`: a CI
   runner's checkout sits at a different root, so every id it mints differs from every id
   the docs cite. Cloning this repo to a second path and auditing there gives **50 high
   findings — the display cap, all `artifact_id`** — against 9 at the authors' own path.
   Seeding CI's catalog moves the job from vacuous-green to capped-red and buys no true
   positive. Full reasoning: `docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md`.

2. **Gate at commit time on a developer machine**, where the catalog is machine-wide and
   the ids resolve — including the cross-repo ones, which no CI placement can. This is now
   the only viable direction and **remains unbuilt**; it is what this bug tracks.

3. ~~Drop the CI step and keep the check as an MCP-time report.~~ Effectively what ships
   today: `live_ids_or_disabled` (`ae6dc663`) disables the check where the catalog is
   empty, so the CI step runs and correctly claims nothing.

Until (2) lands, **do not read a green `Audit Doc Refs` as evidence about artifact-id
citations** — and note this is now a *decision* rather than an accident, so the silence is
correct and the missing coverage is real at the same time.
## Tests added

`an_empty_artifact_table_disables_the_id_check_rather_than_dooming_every_citation`
(`src/librarian/tools/audit_doc_refs/mod.rs`) covers the disabling half, with a control asserting a
populated catalog still switches the check on. Mutation-verified: replacing
`(!ids.is_empty()).then_some(ids)` with `Some(ids)` reds it.

**Nothing tests the wiring half, by construction** — there is no wiring to test.

## References

- `docs/conventions/cross-machine-catalog-resume.md` — the catalog is machine-local and arrives
  missing three layers, each silent in a different way
