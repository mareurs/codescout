# An id keyed on an absolute path cannot be checked off the machine that minted it

**Status:** accepted
**Date:** 2026-09-14

## Decision

The dead-artifact-id citation guard runs **only where a machine-wide librarian catalog
exists** — a developer machine. It does **not** run in CI, and `Audit Doc Refs` staying
green about artifact ids is correct rather than a gap. `live_ids_or_disabled`
(`ae6dc663`) makes an empty catalog disable the check, which is what implements this.

## Context

`id = sha256(abs_path)[..16]`, and the catalog is machine-wide by deliberate design — the
resolver's own comment reads *"an id minted in a sibling repo is a LIVE id — narrowing
this to the current project would invent dangling citations out of cross-repo references
that resolve perfectly well."*

Two properties follow, and the second is the one that decides this:

1. **The namespace spans every repo on the machine.** 8 citations in this corpus point at
   artifacts in `changelog-reader`, `backend-kotlin`, `eduplanner-ui`, `PFA`, `MRV-poc`
   and `whatsapp`. A single-repo observer reports all 8 as dead, and cannot tell them from
   genuinely dead ones — an id is an opaque hash and carries no repo.
2. **The namespace is keyed on an ABSOLUTE path, so it is not portable between checkouts
   at all.** The same file yields a different id under a different root:

   ```
   db1145e76b0cbda2   /home/marius/work/claude/codescout/docs/TAXONOMY.md
   07f4a7ad70f38757   /home/runner/work/codescout/codescout/docs/TAXONOMY.md
   ```

Property 2 is fatal to any CI placement, and it is invisible to the obvious experiment.
Measured by cloning this repo to a second path and auditing it there:

| catalog built at | artifacts indexed | high findings |
|---|---|---|
| the same path as the citing authors | 1683 | 9 — all cross-repo, zero genuine |
| **a clone at a different path** | 1682 | **50 — the display cap, all `artifact_id`** |

The first row is what a developer's own machine sees and it is why the check appears to
work. The second row is CI.

## Alternatives considered

- **`--reindex` on `codescout audit-doc-refs`, seeding CI's catalog before the audit.**
  Built and measured, then removed. It does populate the catalog (1683 artifacts from an
  empty workspace, resolved from `--project` rather than workspace roots) — and moves CI
  from vacuous-green to capped-red, with no true positives on either side. Its premise
  *"the catalog is empty, so seed it"* is true and insufficient: seeding produces the
  **wrong ids**. Rejected, with the reasoning left at the arg site because the idea is
  attractive enough that the next reader will have it too.

- **Scope the id set to the current project.** Rejected by the resolver's own documented
  reasoning and by 8 live counter-examples: it converts every legitimate cross-repo
  citation into a finding.

- **Re-key ids on `<repo>:<repo-relative-path>`, making citations portable.** This is the
  only alternative that would make a CI placement correct, and it is a migration touching
  every artifact id in every repo on the machine plus every citation in every document.
  Not now — recorded as the revisit trigger below.

- **Commit a manifest of this repo's ids.** Rejected: a manifest generated on a developer
  machine carries that machine's paths, so it has property 2 baked in and decays the
  moment anyone clones elsewhere.

## Consequences

**Now easier.** CI is honest: it makes no claim about artifact-id citations rather than a
false one. The check keeps full fidelity where it runs — on a developer machine it
resolves cross-repo ids correctly, which no CI placement could.

**Now harder.** A dead intra-repo citation can reach `master` uncaught by CI. The
compensating placement is commit time on the machine that minted the ids, which has the
whole namespace; that mechanism is **not yet built** and is tracked at
`2a97a4faddc0eace`. Until it exists, this class is caught by a developer running the audit
and by nothing else — stated plainly because a decision that quietly lowers coverage while
reading as a clean architectural result is the failure mode this repo tracks as
`cluster/declared-not-wired`.

## Change scenarios absorbed

- *A contributor adds `--reindex`, or any catalog-seeding step, to the `Audit Doc Refs`
  job to "fix" its silence about ids.* The measurement above says what they will get; the
  arg-site note puts it where they will be standing.
- *A second CI job wants to resolve artifact ids.* Same answer, same reason, already
  written down.

## Revisit when

- Artifact ids stop being keyed on an absolute path — if `id` ever becomes a function of
  `<repo> + <repo-relative path>`, property 2 disappears and CI becomes a valid observer
  for intra-repo citations. Property 1 would still require a scope disambiguator on
  cross-repo citations, for which this repo already has a convention in the
  `<repo>:<sha>` prefix discipline used for commits.
- The commit-time placement ships and is measured, at which point this ADR should name it
  rather than pointing at an open bug.

## Confidence

**High.** The decisive property is arithmetic — two sha256 values of two different strings
— and it was confirmed end-to-end by a clone at a second path rather than inferred from
the hash alone. The earlier reading that the empty catalog was the whole problem was
produced by a simulation that held the one variable that mattered constant, by accident.
