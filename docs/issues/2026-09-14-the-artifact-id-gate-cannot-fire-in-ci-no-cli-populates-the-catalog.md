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

The wiring half is **done**: `scripts/pre-commit-dead-artifact-ids.sh`, self-gating on staged
`.md` from the index, wired into `scripts/pre-commit-run.sh`.

1. ~~**Expose reindex on the CLI**, then add a step to the `Audit Doc Refs` job.~~ **BUILT,
   MEASURED, REVERTED 2026-09-14** — seeding CI's catalog mints the WRONG ids, because
   `id = sha256(ABSOLUTE path)`. A clone at a second path reports 50+ where the authors'
   path reports 9. See the ADR.

2. **Gate at commit time on a developer machine** — **SHIPPED.** The only observer with the
   whole namespace. It reuses `audit-doc-refs --paths` rather than re-deriving the parser or
   the exemption chain, so `archive_drop` / `issues_drop` / `code_block` apply for free; it
   filters to `artifact_id` + `artifact_missing` + `high`, which is exactly the class CI
   cannot reach, leaving every other finding to CI where it belongs. Cost measured: **0.16 s**
   for two staged files against ~60 s for the full corpus.

3. ~~Drop the CI step and keep the check as an MCP-time report.~~ Not needed — the CI step
   stays and is honest: `live_ids_or_disabled` (`ae6dc663`) disables the id check where the
   catalog is empty, so it runs and correctly claims nothing about ids.

**Three things it declines to claim, each named on stderr and each passing OPEN**, because a
guard whose absence blocks every commit is worse than the hole it closes: no runnable
binary; an unreadable audit report; and a file whose **staged bytes differ from the
worktree**. That last one is the interesting one — the other checks in this hook read
`git show :<path>`, and this one cannot, because materializing staged bytes to a temp path
re-keys every file's own id and would flag every self-citation as dead. So it compares the
two and audits only where they agree. Same rule the mutation probe learned the same day:
do not render a verdict over bytes you did not examine.
## Tests added

`tests/pre-commit-dead-artifact-ids.sh` — **19 assertions across 8 cases**, wired as its own
CI job. Driven through a stub binary via `CODESCOUT_BIN`, so it needs no Rust toolchain and
no catalog; that is deliberate rather than a shortcut, since a case that skips itself for a
missing toolchain reports success while testing nothing.

Cases 2–4 are the discrimination set: each flips exactly ONE of the three filter fields
(`med` instead of `high`, another `ref_kind`, a `resolved` verdict) so a filter that
silently widened to "any finding" reds here rather than on the live corpus. Case 8 pins
that an unstaged-markdown commit prints **nothing at all** — a hook that fires on every
commit to say nothing is how `--no-verify` gets learned.

Case 1 also pins the REMEDY TEXT by shape: both branches must survive (repoint a stale
citation, fence a deliberate mention) plus the stale-catalog case that is neither. A suite
that tests only a guard's predicate leaves its remedy untested by construction, and here
picking the wrong remedy destroys the record rather than merely failing.

**End-to-end, measured by hand 2026-09-14** against a real binary and a populated catalog,
since CI cannot host it: an inline dead id in a staged file REFUSES with exit 1 naming
`doc.md:3`; the same id fenced PASSES; and a clean staged file with a dead id only in the
worktree DECLINES and passes open.
## References

- `docs/conventions/cross-machine-catalog-resume.md` — the catalog is machine-local and arrives
  missing three layers, each silent in a different way
