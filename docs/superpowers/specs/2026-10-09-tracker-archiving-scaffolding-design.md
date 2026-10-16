---
id: '401f57bc56347455'
kind: spec
status: draft
title: Tracker archiving scaffolding
owners:
- marius
tags:
- tracker-archiving
- scaffolding
- design
topic: Shared tracker archiving preservation machinery
---

# Tracker archiving scaffolding

## Design brief

The user wants both shrinking of ongoing ledgers and retirement of completed trackers. Invocation will eventually be hybrid: explicit requests and automatic cleanup. A codescout guide or persistent prompt, plus a configurable cleanup aggressiveness, is a possible later policy layer.

The initial deliverable is the preservation machinery and a usable explicit archive operation. Success means that archiving keeps complete evidence, entry identity, structured rows, citation resolution and allocation state consistent. A repeated or interrupted operation must not duplicate or lose history.

This is a draft design for review. It is not an implementation or an approved automatic-cleanup policy.

## Architecture

Use one archive service behind the existing doc tool family, with preview as its default. Separate selection policy from preservation mechanics:

1. A caller chooses explicit entries or a whole tracker, and supplies a reason.
2. A planner reads the complete source, destination and relevant catalog state, builds proposed changes, and reports blockers.
3. An executor revalidates that plan under the project write guard and applies it with recovery records.
4. Verification checks entry conservation, citation resolution and allocation state, then returns an observed result.

The future cleanup guide and aggressiveness configuration will feed requests into this same service. They will not bypass preservation checks, declare a tracker finished solely from age, or maintain a second archive implementation.

## Logical ledger identity

An entry belongs to a ledger independently of the artifact currently storing its section.

Persist three custom frontmatter fields through the existing extra map:

- archive_ledger_id: a stable opaque family identifier. For a previously undeclared family, derive it deterministically from the source artifact identity and inspected content hash, then persist it once. Never recalculate a recorded value after an edit or move.
- archive_role: root or entries.
- archive_qualifiers: the root's original file stem and any explicitly recorded historical root qualifiers.

An entry companion carries the same family identity as its root and role entries. The family relation and qualifier declarations are committed metadata, so a fresh catalog can rebuild them. These fields do not change the existing path-derived artifact ID contract.

There must be exactly one root per family. Multiple families claiming one qualifier, or duplicate definitions of an entry inside one family, are reported as ambiguity. Do not resolve either by silently preferring the newest or active artifact.

Existing archives without these fields remain legacy artifacts. Do not infer lineage from filename similarity. A caller can explicitly select an existing companion; the preview must expose the association, reject conflicts, and write the association only when the archive operation applies.

Archive companions receive existing entry IDs through transfer; they never allocate new ones. append_entry must refuse archive_role entries and retired roots. Ordinary appends allocate only at an active root. Reopening a root is an explicit lifecycle change, not an implicit effect of append.

## Entry archival

A request selects exact entry IDs from one declared ledger. Selection is not a status transition: the service preserves each entry's disposition and promotion fields as recorded.

Reuse the shared, fence-aware entry section boundaries used by citation extraction. Slice original bytes using those boundaries; do not use EntrySection.text as the lossless payload because its lines().join("\n") representation normalizes line endings.

Move complete sections, including subordinate evidence. Preserve their headings, relative ordering, substantive text and line endings. The only permitted payload transformation is repair of supported Markdown link targets whose relative base changes when the section moves. Preview exposes each such repair; literal commands, fenced examples and ordinary prose are preserved. If a link cannot be repaired with a precise parser span, report a blocker instead of guessing. Reject duplicate IDs, overlapping selected sections, and a selected section containing an unselected defining entry. Do not invent an outcomes digest or summarize the archived evidence.

The normal destination is one persistent companion:
docs/trackers/archive/<root-stem>-archived-entries.md

Use committed family metadata to identify an existing companion. A filename collision is a blocker, not permission to overwrite an unrelated file or mint another companion silently.

The companion records per-entry archive_entry_provenance in committed frontmatter: source relative path, source content hash, original entry hash, archive date, reason, and exact reversible link-target replacements. Store original and replacement strings with their entry-local spans, not only an aggregate hash. Reversing those declared repairs must reconstruct the original entry bytes and match its original hash.

The root retains its non-entry instructions. Before removing any sections or rows, persist each prefix's maximum across source body IDs, params IDs, existing frontmatter marks and machine-local reservations in the root's committed high-water mark. Both the prose and params allocation paths must honor that mark, including after catalog loss. Existing marks never decrease. Remove selected rows from recognizable declared snapshot tables together with their sections. Unrecognized snapshot formats are explicit blockers; successful section transfer must not leave a live table presenting transferred entries as live.

For augmented ledgers, transfer the matching params rows in the same operation. Preserve every field of each row, and retain the source's unrelated params and rows. Validate the destination collection and schema before writing. A row-only entry with no defining section, or a section/row mismatch, blocks apply rather than triggering a lossy reconstruction. The archive companion retains an entry_collection so archived rows can still be filtered.

Machine-generated collections need their producer to participate: otherwise the next generation could recreate a removed row. Report producer-managed collections as unsupported for entry transfer until a producer adapter implements the same planning contract.

## Whole-tracker retirement

For the first implementation, retirement sets status archived in place, records archived_at and the supplied reason, and preserves the entire body, augmentation, family metadata and counters.

Keeping the path initially preserves path citations, artifact-ID citations and existing file-stem qualifiers. Changing status can still change the resolver's active-definer tie-break for external bare tokens, so retirement uses the same pre/post citation comparison and blocks those changes until the citing text is qualified. Archived discovery continues through the existing include_archived query. A generated tracker cannot retire while its producer is still registered to regenerate it; its adapter must acknowledge retirement.

Physical relocation is a separate operation already exposed by doc(move). It changes artifact identity and has its own reference-repair obligations. It is not part of the initial retirement apply path.

This deliberately proposes an amendment to the existing dated-move retirement policy. The core archived state is a lifecycle state, not a directory location. Reconcile the live policy surfaces explicitly if this design is approved; do not leave two contradictory recipes.

## Citation and retrieval behavior

A moved section changes its relative-link base. Rewrite supported Markdown link destinations from the source directory to the companion directory, preserving fragments, queries and link labels. Absolute external links remain unchanged. References shared outside the selected sections cannot be transplanted silently: copy the exact required definition when unambiguous, otherwise block with the missing dependency. Persist the exact repairs in entry provenance and verify the repaired targets. Source-relative links in preserved code examples remain historical evidence.

Extend citation resolution to use committed ledger families:

- A qualified citation resolves its qualifier to a family, then finds the entry's unique physical definer within that family.
- A bare sibling citation inside a family searches that family before global token lookup.
- Global bare-token resolution otherwise retains its existing behavior. Preview compares pre/post resolution for citations of the affected entries across the complete project catalog, including archived citers. A previously resolved external bare citation that would become ambiguous or point elsewhere is a blocker; identify its caller so it can be qualified before a new preview. Apply must not convert ambiguity into a plausible wrong edge.
- A direct citation of an archive companion's own stem continues to work.
- An ambiguity yields candidates and a reason, never a guessed edge.

The graph still points at the artifact physically containing the proof. Reattribute scanner-owned edges after transfer, and transfer or repoint affected manual entry citations without dropping their metadata. Whole-artifact grafting is not an entry-transfer substitute: it would move unrelated history too.

Existing doc(get, entry_filter) remains artifact-scoped. Add family-aware entry retrieval through doc(get, entry_id), with include_archived controlling retrieval of a transferred entry. Return the physical artifact and entry location explicitly. Never return an empty successful result because the entry was found but archived; identify that state and the retrieval option.

Archive defaults remain hidden in broad live-work discovery while explicit archived retrieval remains possible.

## Request and result contract

Proposed doc action archive:

- id: the source artifact.
- archive_mode: entries or tracker.
- entry_ids: required, non-empty and unique for entries; forbidden for tracker.
- dst_id: optional explicitly selected entry companion; forbidden for tracker.
- reason: required, non-empty provenance.
- force: false or absent previews; true applies. It means apply, never bypass integrity checks.
- plan_digest: required when applying, obtained from preview.

Preview must be read-only, including in a worktree: no fork, file creation, frontmatter stamp, reservation, journal or event.

Return a typed plan containing scope, operation mode, source and destination, selected entry IDs, proposed changes, byte and entry counts, citation impacts, blockers, and a digest of the complete inputs and requested operation. Explain blocker vocabulary present in the response. Counts refer to the complete inspected population, with output pagination separate from computation.

Apply recomputes and checks the digest while holding the write guard. Changed source bytes, destination bytes or relevant catalog rows yield stale_plan without mutation. Blocked or unsupported requests produce actionable scoped results and never claim that an archive occurred.

An applying worktree request is refused until the changes are merged and apply runs against the authoritative checkout. The first implementation does not archive main-checkout files through a shadow.

## Recovery and verification

Filesystem replacements and SQLite commits are not one atomic primitive. Use a durable per-operation manifest with preimage/postimage hashes and backups under .codescout/archive-operations/. Persist it before the first file replacement.

A small catalog operation record, committed in the same transaction as row changes, distinguishes a committed operation from an interrupted pre-commit operation. This is recovery state, not a new general entry identity table.

Stage complete files and validate them before publishing. Preserve foreign edits: recovery may restore only a file matching that operation's expected postimage, or accept a file already matching its preimage. A third hash is a recovery conflict requiring attention; never overwrite it.

On an incomplete operation with no catalog commit marker, restore the published file preimages. With a commit marker, verify postimages and finish the recorded operation. Retrying a completed plan returns its recorded outcome without transferring again; if files have subsequently changed, report that observation separately from the historical outcome.

Before reporting an applied operation, verify:

- each selected ID has exactly one defining section across the family;
- unselected source entries and params remain present;
- reversing declared link-target repairs reconstructs each original section byte-for-byte, while complete structured rows match their preimages;
- counters have not decreased;
- supported qualified and intra-family citations still resolve to their intended entry;
- affected graph rows are attributed to the current physical owners.

Derived vector refresh failures are reported separately from the durable transfer outcome. Do not report a verified semantic index when its refresh failed.

## Module boundaries and initial support

Create a small librarian archive module for typed requests/results, family metadata validation, planning, execution/recovery and storage adapters. Thin MCP and CLI handlers call the same service.

Initial adapters support declared prose ledgers, augmented entry ledgers with matching section/row structure, and whole-document retirement. All catalogued tracker forms receive a truthful capability result. Reflective documents without entry IDs support whole-document retirement; generated collections require producer support for transfer or retirement. An unsupported format is an explicit result rather than an empty plan.

Reuse shared frontmatter parsing, heading/fence parsing, project write locking, schema validation, event recording and catalog audit machinery. Read current implementations before choosing helpers; reuse cannot mean copying their discipline without tests.

Write a concise developer-facing archiving contract and reconcile the taxonomy/archive policy and the serving retirement recipe. The larger cleanup guide, scheduling, thresholds and aggressiveness presets are the next policy layer.

## Alternatives weighed

1. Filtering historical entries reduces query context but leaves growing files and supplies no conserved transfer.
2. Shared archive planning/execution with stable ledger lineage supplies a reusable foundation for both explicit and automatic callers. Recommended.
3. Converting every entry to its own artifact would simplify physical entry identity but requires a larger migration of ledger, graph and augmentation contracts.

For whole retirement, an initial move wrapper reuses existing directory conventions but immediately inherits path and artifact-ID repair. In-place retirement makes the first foundation smaller and preserves those references; relocation remains a separate capability.

## Validation

Use isolated fixtures and the repository gate. Tests must observe archive results, not merely helper coverage:

- preview makes no filesystem or catalog writes, including when called from a worktree;
- apply transfers selected raw sections and complete params rows while retaining unselected siblings;
- LF, CRLF, nested fences, duplicate IDs and overlapping headings exercise the actual payload boundaries;
- moved relative links still reach their original targets; absolute links and literal examples are untouched; recorded repairs reconstruct the original section bytes;
- a root qualifier and bare sibling citation still resolve after the sibling is split into a companion;
- two ledgers with identical F/W tokens remain distinct, including the case where retiring the sole active definer would change a previously resolved external bare citation;
- companion reuse is deterministic and cannot overwrite an unrelated file;
- fresh-catalog reconstruction and subsequent allocation do not reuse an archived ID on either the prose or params path; archive companions and retired roots refuse new allocation;
- stale inputs cause zero writes;
- failures after each publish/commit boundary recover without loss, and recovery refuses foreign edits;
- retrying a completed plan does not duplicate entries;
- archived entries remain explicitly retrievable and absent from normal live discovery;
- producer-managed or malformed shapes return the correct capability/blocker rather than success.

For load-bearing review, actually apply candidate mutations to transfer selection, family discrimination, counter preservation, read-only preview and recovery hash checks. Report observed killed/surviving mutation counts. Write tests for an observed surviving mutation only after observing it survive.

## Current evidence and implementation boundary

The scout inspected entry_sections, citation resolve and entry-edge attribution, frontmatter extra storage, allocation high-water marks, move/graft behavior and the doc write-effect guard. The section text-normalization finding is code-inspected; no archive transfer has been run. Self-review also identified relative-link-base changes during relocation; the draft now permits precisely recorded link-target repairs while requiring reconstruction of the original payload.

The existing gate exits before testing in this Codex session because CLAUDE_CODE_SESSION_ID is unset. This draft does not claim tests passed. Resolve the execution environment before implementing and validating the scaffolding.

Two existing documentation conflicts are tracked in:
- [Taxonomy destination conflict](../../issues/2026-10-08-taxonomy-archive-cadence-conflicts-with-ratified-policy.md)
- [Retirement compaction conflict](../../issues/2026-10-08-tracker-hygiene-compaction-drops-citable-entry-headings.md)

[Work-stream reconnaissance log](../../trackers/tracker-archiving-session-log.md).
