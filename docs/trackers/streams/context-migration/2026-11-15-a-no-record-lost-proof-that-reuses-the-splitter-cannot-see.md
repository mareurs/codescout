---
kind: entry
status: open
title: A no-record-lost proof that reuses the splitter cannot see the splitter's misses
created: 2026-11-15
entry_kind: lesson
status_since: 2026-11-15
---

**Observed:** the v1 design of Plan L-E proved M1 (no record lost) with check items driven by the same splitter that picks the records (`def_re` plus `entry_sections`). The lifecycle line's Opus review (`docs/research/2026-11-15-plan-le-design-review.md`, C2) showed what that hides: `## R-56b` and `## R-59b` in `archive/reconnaissance-patterns-archived-entries.md:332,346` are not definitions, so they would be non-record sections, and a `drop` would lose two records with a clean check. `prompt-hamsa-audit-log.md` repeats `A-38` on 11 headings, so the parent section would copy its subsections twice. Both verified by grep on 2026-11-15.

**Lesson:** a check that reuses the transform's own classifier is common-mode: whatever the classifier misses, the check misses too. A proof of "nothing lost" must account for the source independently of how the transform split it. Design v2 (`8e8730071`) makes the partition the proof: every source line belongs to exactly one carried record, readme section, dropped section or the replaced frontmatter. A heading with a token-shaped string that is not a definition is a "possible record" and blocks until the mapping decides.

**Rests on:** [[2026-11-15-tracker-migration-design]]
