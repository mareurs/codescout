---
specialist: docs-lotus-frog
scope: project
slug: release-notes-soul
created: 2026-05-15
updated: 2026-05-15
tags: [release-notes, narrative, identity, changelog]
---

**Lesson:** When writing codescout release notes or "about this project" prose, structure follows the development dependency arc — compression → retrieval extraction → evals — and the soul line is *codescout uses codescout to grade codescout*.

**Why:** This project ships its thinking. Since v0.9.0, docs commits (132) edged out feat commits (130); trackers, eval rounds, and ADRs are librarian-managed artifacts, not byproducts. The development arc isn't decoration — it's a real dependency order: tool surface had to compress before retrieval could be cleanly extracted, and evals only made sense once the surface and retrieval were stable enough to grade. Release notes that mirror that order read as honest causation rather than chronology.

**How to apply:**

- **Three-act structure** for any release-notes draft spanning more than one internal version:
  1. *Compression* — tool consolidation, surface changes, what got collapsed and why.
  2. *Extraction / substrate* — what moved out of process, what got swapped, binary-size and dep deltas.
  3. *Evals* — what is now graded, the verdict numbers, what the evals surfaced and fixed.
- **Hook line template** when there is a release gap: `"<N> weeks. <commits> commits. <versions> versions written, none shipped — until now."` Lead with the gap, not an apology.
- **Numbers to always include** for a multi-version release: commit count since last public release, file count, SLOC, commit-type ratio (docs vs feat reveals the project's character), tool count before/after consolidation, binary size delta, eval pass rates.
- **Soul line**: end with the recursion claim. *codescout uses codescout to grade codescout.* It is literally true — nav-eval and edit-eval call live MCP tools, verdicts live as librarian-managed artifacts in `docs/trackers/`. Recursion is the product.
- **Do not split** an unshipped internal version chain (v0.10, v0.11, v0.12) into separate public releases when crates.io was also skipped — fold them into one release at the highest version, label clearly, and surface the migration cliff once instead of three times.
- **Pre-merge punchlist** for a folded release: bump `Cargo.toml`, land `experiments` → `master`, decide whether `Unreleased` graduates into the same version, then publish to crates.io and `gh release create`.
