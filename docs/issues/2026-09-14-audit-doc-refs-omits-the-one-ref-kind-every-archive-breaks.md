---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: `audit_doc_refs` omits the one ref kind every archive breaks

## Summary

Archiving an artifact re-keys it — `id = sha256(abs_path)` — so every prose or params
citation of the old 16-hex id silently resolves to nothing. Two instruments could catch
this and the job is split so that neither does: `link_scan` **sees** it (`kind:
"ArtifactId"`, 625 dangling today) and gates nothing, while `audit_doc_refs` **gates CI**
at `high` and has no artifact-id variant in its `RefKind` enum at all. The re-key itself is
documented and correct; what is missing is any surface that reds when a citation of it goes
dead.

## Symptom (Effect)

A queue row names a bug that no longer resolves. The id is well-formed, so nothing looks
wrong until it is dereferenced:

```
doc(action="get", id="403e3fad0356f171")
-> unknown artifact id `403e3fad0356f171`
```

Seven rows of `docs/trackers/open-issue-work-queue.md` carry a Bug pointer in exactly this
state. Both stores agree — params and body hold the same dead id in all seven — so a
params/body reconciliation pass cannot surface it either.

## Reproduction

```
git rev-parse HEAD          # 979fe285 at time of filing
```

1. Archive a bug file the documented way:
   `doc(action="move", id=<id>, new_rel_path="docs/issues/archive/<same-name>.md")`.
   The response reports `id_changed: true` and the new id.
2. Leave any prose or params citation of the **old** id in place — this is the default,
   because nothing prompts for it.
3. `librarian(action="audit_doc_refs")` — the citation is not examined. Clean.
4. `librarian(action="link_scan", write=false)` — it appears under `dangling`, with
   `kind: "ArtifactId"`. Nothing consumes that.

## Environment

codescout `experiments`, Linux, MCP stdio. Catalog `~/.local/share/librarian/catalog.db`
(machine-local). Shared checkout, several concurrent sessions.

## Root cause

Two independent gaps, both structural.

**`audit_doc_refs` cannot express the reference.** `RefKind`
(`src/librarian/tools/audit_doc_refs/mod.rs:11`) has five variants — `FilePath`,
`FileLine`, `FileSymbol`, `ModulePath`, `Link`. No artifact-id variant exists, so a 16-hex
token is not a reference the parser recognises and no severity is ever assigned. A grep for
`artifact_id`, `resolve_cite` and a 16-hex pattern across
`src/librarian/tools/audit_doc_refs/*.rs` returns zero — measured 2026-09-14. This is the
linter that reds CI at `high`.

**`link_scan` can express it and gates nothing.** Its `dangling` array carries
`{"kind": "ArtifactId", "raw": ..., "src_id": ..., "line": ...}` rows, so detection is
already built and already running. `write=false` is the default and no gate consumes the
report; the four-command gate runs green with the count in the hundreds.

Measured 2026-09-14, all figures derived rather than cited — each names its unit:

```
queue rows whose Bug pointer resolves to no artifact         7   (of 77)
  ...positively matched to an archive move by recomputing
     sha256(<pre-archive abs_path>)[:16]                     7 of 7
  ...params and body disagreeing on any bug pointer          0

distinct 16-hex tokens in docs/**.md resolving to nothing  245
  ...matching a known docs/issues/archive move              125
live (non-archive) docs carrying >= 1 such token             34 files

link_scan dangling, whole project                          625
```

The seven, with the archived file each was minted from, fenced so this file does not
itself mint seven more dangling edges:

```
BL-5   3f88d49c38ced0c1 -> 9ea19e403dc550b5  2026-08-16-tracker-design-guidance-always-overflows.md
BL-6   a9644b964edac789 -> 07529c63f88bd713  2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md
BL-7   0a15c81150c4cce7 -> 26a793b432e0f29c  2026-08-15-write-scope-denial-does-not-name-approve-write.md
BL-8   c320b6564d1cb003 -> 3226924940fb30ce  2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md
BL-10  772fff5739620581 -> 1c57379181202ff7  2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md
BL-18  29f1ddf259562b7f -> c75dbf6291fc5876  2026-08-16-artifact-create-augment-drops-template-and-schema.md
BL-25  cfcbee6f7d047a55 -> 38e45994a0919313  2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md
```

## Evidence

### The enum, read rather than inferred

```
$ grep -n "pub enum RefKind" -A6 src/librarian/tools/audit_doc_refs/mod.rs
11:pub enum RefKind {
       FilePath,
       FileLine,
       FileSymbol,
       ModulePath,
       Link,
   }
```

### `link_scan` already classifies them

```
$ librarian(action="link_scan", write=false)
counts.dangling = 625
dangling[0] = {"src_id": "2dd9d90bc83f9f49", "raw": "b75d2660ef37198c",
               "kind": "ArtifactId", "line": 55}
```

`2dd9d90bc83f9f49` is `docs/trackers/bug-fix-session-log.md` — a live tracker, seven dead
ids in it.

### A mention is indistinguishable from a citation

The queue holds **22** dead 16-hex tokens, not 7. The other 15 are prose: `next` notes and
status cells that *name* a dead id in order to record that it was dead. One of them,
`403e3fad0356f171`, was written deliberately in `67bff6da` while repairing BL-16's pointer
— the sentence explaining the repair is itself counted as a broken citation. There is no
escape for mention, which is `IC-6` holding about the file that records it.

## Hypotheses tried

1. **Hypothesis** — params holds dead ids while the body has been repaired, so the two
   stores disagree and the body is the recoverable copy (the BL-16 / BL-1 two-stage shape).
   **Test** — compare the Bug column against `params.tasks[*].bug` for all 77 rows.
   **Verdict** — rejected. Both stores dead on the same seven, zero disagreements. The
   two-stage framing described two rows already repaired, not a live population.

2. **Hypothesis** — the population is ten rows, offered by a peer session.
   **Test** — re-derive. **Verdict** — rejected, and the cause is instructive: their
   comparison read a missing `bug` key as `""` and scored it against the template's `—`
   literal, so ten rows citing **no** bug id were counted as ten citing a stale one. The
   number measured an em-dash. Withdrawn by its author before I used it.

3. **Hypothesis** — nothing in the codebase detects dead artifact-id citations.
   **Test** — `link_scan` report mode. **Verdict** — rejected. Detection exists and is
   running; only the gating is absent. This is the difference between "build a detector"
   and "wire the one you have", and it inverts the fix.

## Fix

Not yet implemented. Two candidates, and they are not alternatives — the second is cheap
and the first is the one that closes it:

- **Wire the existing signal.** `link_scan`'s `dangling` already carries `kind:
  "ArtifactId"`. A gate over *live* (non-archive) docs, seeded at today's 34 files so it
  reds on growth rather than demanding a 34-file sweep first, costs no new detector.
  Archive paths must stay exempt for the same reason `audit_doc_refs` has `archive_drop`:
  a retired document citing a retired id is the historical record.
- **Give `audit_doc_refs` an `ArtifactId` variant** so the CI linter can express the
  reference at all. Larger, and it duplicates detection `link_scan` already performs.

**Neither should sweep the 245.** Most are in archived files where the dead id is correct
history, and the mention/citation ambiguity above means a mechanical repass would rewrite
sentences whose subject *is* a dead id.

## Tests added

None — no fix yet. The guard this needs is a red-on-growth assertion over live docs, and
the seeding value (34 files / 125 archive-matched tokens) is derived above so it can be
pinned without re-deriving under a different counting rule.

## Workarounds

- **Cite by `rel_path` or entry id, not by artifact id**, for anything that will be
  archived. `get_guide("tracker-conventions")` § *Cross-linking* already says entry IDs are
  stable and artifact ids are not; the queue's Bug column predates that and stores the id.
- **Re-point citations in the same commit as the move** — prescribed in
  `docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md`
  § *Workarounds*, which called it out for ids specifically: *"The guide covers paths but
  not ids; ids break the same way."* Unenforced since.
- **Recover a dead id positively** rather than guessing: recompute
  `sha256(<repo>/docs/issues/<basename>)[:16]` for each archived file and match. That is
  how all seven above were identified, and it needs no history.

## Resume

Decide between the two fix candidates — the wiring one is preferred and the decision is
about *where* the gate lives, since `link_scan` is not currently in any gate. Then seed the
threshold from the derivation in § *Root cause*, re-running it at that instant rather than
copying these numbers: the corpus moves, and a stale seed reds on churn.

Do **not** start by sweeping citations; establish the exemption for `docs/**/archive/**`
first, or the first run reports mostly history.

## References

- `src/librarian/tools/audit_doc_refs/mod.rs:11` — the `RefKind` enum
- `src/librarian/catalog/augmentation.rs` — `resolve_cite_ref`
- `docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md`
  — the re-key, and the workaround that named ids and was never enforced
- `docs/issues/archive/2026-09-04-artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md`
  — the same re-key stranding *vectors*; one mechanism, a different downstream store
- `docs/trackers/open-issue-work-queue.md` — BL-1 and BL-16, the two rows already repaired
