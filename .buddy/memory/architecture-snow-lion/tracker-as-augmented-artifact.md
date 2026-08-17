---
specialist: architecture-snow-lion
scope: project
slug: tracker-as-augmented-artifact
created: 2026-05-07
updated: 2026-08-17
tags: [librarian, augmentation, doc-architecture, state-management, novel-pattern, link-scan, entry-identity, guard]
---

**Lesson:** Some markdown files here are **augmented artifacts** managed by the librarian — a persistent prompt plus structured params. Two corrections to how I described this on 2026-05-07, both measured on 2026-08-17, both load-bearing: **params are invisible in the file on disk**, and **entry identity lives in body headings, never in params or table rows.**

**Why:** My original entry said `docs/trackers/tool-usage-patterns.md` renders "the live params table at the top by the librarian." That is false about the file. Measured: the file contains frontmatter, prose, and 22 `### T-N` headings — **no rendered block, no params table, no `[LIVE]` marker.** `render_template` output is injected into `librarian(action="context")` only. Anyone opening the file on disk or on GitHub sees no params. I taught a layout that does not exist for three months.

The second correction is the one with teeth. `link_scan` derives an entry token's **definition** from a heading and from nothing else — `def_re` in `src/librarian/tools/link_scan/extract.rs` is `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`. So:

- a **params row** defines nothing;
- an **index table row** defines nothing;
- `## R-2` with no ` — title` defines nothing, and `### A-9 Addendum` is a section *about* A-9, not a definition of it;
- a suffixed id (`R-72b`, `F-6a`) is not a valid token at all — `\b[A-Z]{1,3}-\d+\b`, and digit→letter is not a word boundary — so it can never be defined *or* cited.

Evidence: `reconnaissance-patterns.md` carried 106 index rows against 58 body sections; its 48 row-only entries produced ~30 of the project's 39 sampled dangling citation tokens (720 dangling of 3359 project-wide). `tool-usage-patterns.md`, with 22 headings and 0 rows, produced **zero**. Migrating the 48 into headed archive sections took project dangling 720 → 615 and R-N danglings to 0. The substrate agrees: `append_entry` (`src/librarian/catalog/augmentation.rs`) folds `body_claimed_indices` into id allocation and its own comment calls the markdown body **"the canonical human-readable surface"**.

**How to apply:** First question is still "is this augmented?" — `artifact(action="get", id=…)` and read the `augmentation` field; `null` means a plain markdown file where `edit_markdown` works, non-null means route body edits through `artifact(update, patch={body_edits: […]})`. Then:

- **Never design a params-canonical ledger whose entries are cited by id.** Params are unreadable on disk and define no tokens. Params are for *lifecycle state* — status, dates, promote-when — and the body heading is the entry's identity. A hybrid is correct; params-only is not.
- **One entry format, never two.** A hand-maintained index table alongside body sections is the defect that generates the rest: the index falls behind, and ids allocated by scanning one format collide with the other. Either the headings are the index, or the index is rendered from params — never both by hand.
- **Compaction ladder is body → archived section keeping its heading.** Never reduce an entry to a bare row to "compact" it; that destroys its definition. Archival itself is safe — a unique definer resolves even when archived (`single_archived_definer_still_resolves`), and where two define one token the sole *active* one wins.
- **Check for an existing archive artifact before creating one.** I forked a second archive for one ledger and split its definitions into ambiguous tokens; query `artifact(find, …)` with a `rel_path contains "archive/<ledger>"` filter and `include_archived=true` first.
- **Do NOT "fix" the guard by stamping `id:` into frontmatter — I tried it, and it was wrong twice.** `is_librarian_artifact` (`src/util/librarian_guard.rs`) reads the file's own text for an `id:` line, so 26 of 66 tracker/bug files are unguarded. That is **deliberate**: the pinned test `a_catalogued_but_unaugmented_file_stays_directly_editable` argues that guarding by catalog *membership* would refuse `RELEASE.md`, `CONTRIBUTING.md` and every ADR, and it names `skill-frictions.md` — which I had cited as evidence of the "gap" — as a file CLAUDE.md documents `edit_markdown` for. Stamping an id guards on exactly the axis that test rejects, and it silently disabled TAXONOMY.md's documented append path for R-N until I reverted it (`bb9a94d7`).
- **The real finding: neither existing predicate matches the damage.** Augmentation protects params/body coherence; membership is too broad. But every defect measured here — 9 twice-allocated ids, 13 orphaned bodies, 48 entries with no defining heading, 39 of 57 with no disposition — was **entry structure in an unaugmented file**. The missing concept is a **ledger**: an artifact with an id namespace and entry invariants, which is exactly the ten prefixes `docs/TAXONOMY.md` enumerates and wires to nothing. Guard it *structurally*, not per-file: an edit whose target heading matches `^<PREFIX>-\d+` routes through the allocator; every other heading stays directly editable, so a typo fix in a 2,800-line tracker never becomes ceremony. Allocator prototyped and mutation-verified in `540c29c3` (`allocate_entry_id`).
- **Do not repeat the phrase "rendered at the top of the file."** The render target is `librarian(context)`. CLAUDE.md carries the same wrong claim and it is still unfixed.

The standard now lives in `get_guide("tracker-conventions")` § *Entry-level standard*, and the proposal to move id allocation into the server is CAP-5. See [[platform-law-leaks-at-call-sites]] for the sibling shape — a declared law that leaks at call sites nobody swept — and [[tests-that-cannot-fail]] for why "the suite is green" said nothing here.
