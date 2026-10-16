# An id keyed on an absolute path cannot be checked off the machine that minted it

**Status:** accepted
**Date:** 2026-09-14

## Decision

The dead-artifact-id citation guard runs **only where a machine-wide librarian catalog
exists** — a developer machine *(and is trustworthy only where that catalog holds the ids
the citing authors minted, i.e. a checkout at the same absolute path; see the Amendment)*. It does **not** run in CI, and `Audit Doc Refs` staying
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
false one. The check keeps full fidelity where it runs — **on the machine, at the
absolute path, where the ids were minted**: there it resolves cross-repo ids correctly,
which no CI placement could. *(Qualified 2026-10-16: this originally read "on a developer
machine"; see the Amendment.)*

**Now harder.** A dead intra-repo citation can reach `master` uncaught by CI. The
compensating placement is commit time on the machine that minted the ids, which has the
whole namespace; that mechanism **shipped 2026-09-14** as
`scripts/pre-commit-dead-artifact-ids.sh` (`3ab306a6`, patch-id
`eddd04341f92e13022c53b2f8948fa30f136b2c3`) and is tracked at `e0f846901a299717`.

**This paragraph read *"not yet built"* until 2026-09-14 and is the reason the sentence now
names its own limits rather than declaring the hole closed.** The hook catches the class
before a commit exists and nothing catches it afterwards, so three states still pass
uncaught, each named on its stderr and each passing OPEN deliberately: no runnable binary,
an unreadable audit report, and staged bytes differing from the worktree. `--no-verify`
and a machine whose catalog never held the ids are two more. Stated plainly because a
decision that quietly lowers coverage while reading as a clean architectural result is the
failure mode this repo tracks as `cluster/declared-not-wired`.

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

## Amendment 2026-10-16 — "a developer machine" is not the frame; the minting path is

- **Status:** accepted correction to the Decision and Consequences wording. The decision
  (no CI placement, no `--reindex`) is unchanged. **Trigger:**
  `docs/issues/2026-10-16-artifact-id-adr-promises-full-fidelity-on-any-developer-machine.md`.

**What the code does, and does not, gate on.** `live_ids_or_disabled`
(`src/librarian/tools/audit_doc_refs/mod.rs`) is `(!ids.is_empty()).then_some(ids)`. The
only condition under which the check switches itself off is an **empty** catalog. Nothing
compares this checkout's absolute path with the path the citing authors' ids were minted
at. A developer whose checkout sits at a different path has a populated catalog, so the
check runs, and every id minted at the other path misses: the same wall the table's
"a clone at a different path" row measured (50, the display cap, all `artifact_id`).

So the Decision's "a developer machine" and the Consequences' "on a developer machine it
resolves cross-repo ids correctly" name the wrong frame. **Property 2 above is exactly the
reason**: an id is `sha256(absolute path)`, so fidelity is a property of *machine and path*
("the machine and path the citing ids were minted at"), not of "being a developer
machine". A second developer, or the same developer with a second clone, sits outside it.

The consequence for the compensating hook is the same: `scripts/pre-commit-dead-artifact-ids.sh`
is only as trustworthy as that frame, and on a checkout at another path it can report live
citations as dead. Nothing in the code detects that situation today; this amendment records
it rather than closing it. A guard that did would have to compare checkout roots, which the
id scheme (property 2) makes impossible to do from the id alone.

**The 8 versus 9 count.** Context property 1 and the second alternative say 8 (cross-repo
citations / live counter-examples). The measurement table says 9 findings for the
same-path row, and `src/cli/audit_doc_refs.rs` and commit `66cce698` say 9. The ADR never
recorded what separates the two figures, and neither was re-measured for this amendment, so
they are left unreconciled rather than guessed at: 9 is the figure with a recorded
measurement behind it (table row, arg-site comment, commit message), 8 is the count in the Context prose. The conclusion does not depend on which is right; in both, the ids counted are
cross-repo ones a single-repo observer cannot resolve.
