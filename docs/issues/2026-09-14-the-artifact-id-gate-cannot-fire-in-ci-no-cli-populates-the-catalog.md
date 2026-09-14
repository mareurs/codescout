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

Since `f4dc25f9`'s successor (this fix), an empty table disables the check rather than marking
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

**There is no CLI path that writes the `artifact` table.** Measured 2026-09-14 against the release
binary:

| tried | result |
|---|---|
| seed a real `[[roots]]` entry instead of an empty workspace | 0 artifacts — the audit does not reindex |
| `codescout index` | 0 artifacts — populates the **semantic** index (`added=5348`), not the catalog |
| `codescout doc find/get/create/…` | no reindex verb |
| `audit-doc-refs --help` | no reindex knob |

`librarian(action="reindex")` is an **MCP tool only**. A CI job that runs the binary as a CLI cannot
reach it.

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

Not done. The disabling half shipped; the wiring half needs a decision:

1. **Expose reindex on the CLI** (`codescout librarian reindex`, or `audit-doc-refs --reindex`), then
   add a step to the `Audit Doc Refs` job. Restores the gate's meaning. Costs a new subcommand plus
   the CI minutes to index ~1680 artifacts.
2. **Leave it vacuous in CI and gate locally** via the pre-commit hook, where the catalog is real.
   Cheap, and moves a CI-time guard to commit time — the direction
   `docs/trackers/test-escape-hardening.md` argues for generally.
3. **Drop the CI step** and keep the check as an MCP-time report. Honest, and abandons the gate.

Until one lands, **do not read a green `Audit Doc Refs` as evidence about artifact-id citations.**

## Tests added

`an_empty_artifact_table_disables_the_id_check_rather_than_dooming_every_citation`
(`src/librarian/tools/audit_doc_refs/mod.rs`) covers the disabling half, with a control asserting a
populated catalog still switches the check on. Mutation-verified: replacing
`(!ids.is_empty()).then_some(ids)` with `Some(ids)` reds it.

**Nothing tests the wiring half, by construction** — there is no wiring to test.

## References

- `docs/conventions/cross-machine-catalog-resume.md` — the catalog is machine-local and arrives
  missing three layers, each silent in a different way
