---
specialist: common
scope: project
slug: dont-fabricate-commit-rationale
created: 2026-05-15
updated: 2026-05-15
tags: [commits, honesty, code-discipline]
---

**Lesson:** When writing a commit message for a small, undocumented change, never invent a plausible-sounding "why". Describe only what the code does. If the rationale isn't already documented somewhere (CHANGELOG, conversation, a comment), say so or ask.

**Why:** Marius rejected a commit on `src/retrieval/sync.rs` whose body I wrote saying the empty-chunk filter prevented "zero vector poisoning of semantic search results." I had no evidence for that — it sounded plausible because it fit the shape of the code, which is the most dangerous kind of guess. Commits outlive sessions; a fabricated rationale becomes "documented intent" that future readers (and future me) trust.

**How to apply:**

- For undocumented small fixes: subject line states the change, body either omits the why or quotes the only thing we know ("uncommitted on experiments since session start; no comment, no CHANGELOG entry — purpose inferred from the diff").
- For changes documented in CHANGELOG: paraphrase the CHANGELOG entry, do not extend it.
- For changes traced to an issue, spec, or conversation: cite the source (`closes BUG-NNN`, `per docs/superpowers/specs/...`, etc.).
- "Defensive guard against X" is acceptable when the diff literally adds a guard; "prevents production incident Y" is not unless Y is documented.
- When unsure: ask Marius for the why before composing the body, or commit with `--allow-empty-message` followed by a separate explanatory commit once the why is confirmed.
