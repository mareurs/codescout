# edit_file Whitespace-Normalized Fallback — Design

**Status:** design / awaiting review
**Date:** 2026-06-04
**Author:** Marius (with Architecture Snow Lion)
**Related bug:** `docs/issues/2026-06-04-edit-file-old-string-miss-no-closest-match.md`
**Touches:** `src/tools/edit_file/mod.rs` (`perform_edit`, `:453-539`)

---

## Goal

Eliminate the `edit_file` recovery round-trip caused by `old_string` exact-match failures
on Kotlin/source whitespace, **without** weakening the safety that exact matching provides.
When an `old_string` fails to match only because of leading/trailing whitespace or
line-endings, and the intended location is provably unambiguous, `edit_file` applies the
edit itself (re-indented to the file's real formatting) instead of returning an error that
forces a `grep` → `read_file` → retry loop.

## Context / problem

Telemetry (`mirela/backend-kotlin/weekly-pattern/.codescout/usage.db`, 8 days) shows 10
`edit_file` "old_string not found / check whitespace" failures across 10 distinct Kotlin
files. Each recovers in 1–3 extra calls via the same loop. Root cause and evidence are in
the related bug file. The exact-match brittleness is by design (safe, no ambiguous edits);
the cost is the recovery round-trip. This design removes the round-trip for the
unambiguous-whitespace case.

## Decision record

- **Matching posture: whitespace-normalized, uniqueness-gated** (chosen 2026-06-04 over
  "fuzzy similarity" and "better errors only"). Rationale: a linter/AST check catches
  *grammar* breakage but is blind to an edit that lands in the *wrong-but-still-parses*
  location — the exact failure mode relaxation introduces. Therefore safety must come from
  the match being provably unique, not from post-hoc validation. Fuzzy similarity was
  rejected because it reintroduces silent wrong-location edits the backstop cannot catch.
- **Normalization envelope: conservative** — per-line leading + trailing whitespace +
  line-endings only. NOT internal whitespace, NOT unicode punctuation. A pure-indentation
  miss is fixed; a unicode/content miss (e.g. straight vs curly quote, em-dash) correctly
  falls through to the nearest-text error so the agent sees the real bytes and never has
  the tool guess at content.
- **Scope: line-aligned, single-edit only for v1.** Mid-line partial matches stay
  exact-only (re-indentation is ill-defined mid-line). The batch path (`edit[]`,
  `edit_file/mod.rs:335`) is deferred to a follow-up.

## Architecture

The change is confined to the `match_count == 0` branch of `perform_edit`
(`src/tools/edit_file/mod.rs:479-484`). Everything above it is unchanged, so every edit
that succeeds today succeeds identically — zero regression surface for passing edits.

```
perform_edit(path, old_string, new_string, replace_all):
  content = read(path)
  exact_count = content.matches(old_string).count()

  exact_count == 1, or >1 with replace_all   → apply exact            [UNCHANGED]
  exact_count > 1 && !replace_all             → ambiguity error         [UNCHANGED]
  exact_count == 0                            → normalized_fallback()   [NEW]

normalized_fallback():
  if old_string is not line-aligned (multi-line, whole lines)
        → nearest_text_error()                # mid-line stays exact-only
  matches = unique_normalized_windows(content, old_string)
  match matches.len():
    1  → apply_relaxed(window)
    0  → nearest_text_error()                 # content differs, not just ws
    >1 → ambiguous_normalized_error(line_ranges)
```

### Matching algorithm (`unique_normalized_windows`)

1. Split `old_string` into `old_lines` (drop a single trailing empty element from a
   trailing `\n`). `k = old_lines.len()`.
2. `norm(line) = line.trim()` — strips leading + trailing Unicode whitespace (incl. `\r`),
   preserves internal content exactly.
3. Split `content` into `content_lines`, tracking each line's `(start_byte, end_byte)`.
4. Slide a window of size `k`: window at index `i` matches iff
   `norm(content_lines[i+j]) == norm(old_lines[j])` for all `j in 0..k`.
5. Collect all matching `i`. Return the set.

Uniqueness is the safety guard: apply only when exactly one window matches.

**`replace_all` does not widen the relaxed path.** A relaxed apply targets exactly one
unique normalized window. `replace_all: true` with `>1` normalized matches returns the
ambiguous-normalized error (v1) — relaxed multi-site replacement multiplies both the
wrong-location risk and the per-site re-indentation complexity, and is out of scope.
`replace_all` still behaves as today on the exact path.
### Apply path (`apply_relaxed`) — the re-indentation requirement

The agent's `new_string` carries the agent's *wrong* indentation (that is why `old_string`
missed). Pasting it verbatim would fix the match but corrupt the formatting. So the apply
step re-indents:

- `agent_base` = leading whitespace of the first non-blank `old_line`.
- `file_base`  = leading whitespace of the first non-blank matched `content_line`.
- For each `new_string` line `L`:
  - if `L` starts with `agent_base` → `file_base + L[agent_base.len()..]` (swap base prefix,
    preserving relative indentation deeper in the block);
  - if `L` is blank (`norm(L) == ""`) → emit empty;
  - else (shallower than `agent_base`, e.g. a closing brace) → replace `L`'s own leading
    whitespace with the `file_base`-relative shift.
- Replace the exact byte span `[content_lines[i].start_byte .. content_lines[i+k-1].end_byte]`
  with the re-indented `new_string`.

**Worked example.** File block indented 8 spaces; agent sent `old_string`/`new_string` at 4
spaces. Normalized match is unique → apply, but every `new_string` line is shifted from
4-space to 8-space base. Result preserves the file's indentation, not the agent's.

**Open sub-decision for the plan:** tab/space-width handling when `agent_base` and
`file_base` mix tabs and spaces. Default for v1: treat indentation as opaque prefix strings
(swap `agent_base`→`file_base` literally); document that mixed tab/space *relative* indent
inside `new_string` deeper than base is preserved as-authored. A test pins the canonical
spaces-only case; the mixed case is a named risk to resolve during implementation.

### AST backstop (relaxed path only)

After building `new_content`, before writing, when `crate::ast::detect_language(path)` is
`Some(lang)`:

- `before = crate::ast::has_syntax_errors(&content, lang)`
- `after  = crate::ast::has_syntax_errors(&new_content, lang)`
- if `after && !before` → **reject**: do not write; return a `RecoverableError`
  ("whitespace-normalized match at lines X–Y would introduce syntax errors — aborted;
  verify with read_file and retry with exact text").

Compare before-vs-after (not "result is clean") so edits that *fix* a broken file are not
blocked. This stricter gate runs **only on the relaxed path**. The exact-match path keeps
today's warn-only `has_syntax_errors` behavior unchanged (`perform_edit`, post-write check).

### Response (visibility — the No-Echo exception)

A relaxed apply returns a note rather than the usual silent `json!("ok")`:

```json
{ "status": "ok",
  "applied_via": "whitespace-normalized match",
  "lines": "412-415",
  "note": "old_string matched after normalizing indentation/line-endings; verify the result" }
```

This is the sanctioned exception to the No-Echo-in-Writes principle (`CLAUDE.md` Design
Principles): the tool discovered genuinely new information — it landed at a location the
agent did not exactly specify. The exact-match path still returns bare `json!("ok")`. A
silent relaxed-apply is the quiet boundary-weakening that erodes trust; the note keeps the
relaxation visible.

### Error path (`nearest_text_error`) — supersedes bug Fix A

For the `0`-normalized-match case, return the nearest actual text (the bug's Fix A, now the
fallback rather than the whole fix): find the content window with the highest normalized
line-match ratio, include its line range + actual bytes in the error so the agent corrects
in one retry. For the `>1` case, list the candidate line ranges.

## Testing strategy (TDD; harness: `project_ctx()` in `src/tools/edit_file/tests.rs`)

Write each test failing first. No new env isolation needed (tempdir-based harness, no
`LIBRARIAN_*`).

1. `normalized_match_applies_on_indentation_only_diff` — 8-space file block, 4-space
   `old_string`; exact fails; unique normalized match; edit applies; response carries
   `applied_via` + `lines`.
2. `normalized_apply_reindents_new_string_to_file` — assert the written bytes use the
   FILE's indentation, not the agent's.
3. `exact_match_preferred_over_normalized` — exact `old_string` present → exact path,
   bare `json!("ok")`, no note (regression guard: passing edits unchanged).
4. `normalized_match_ambiguous_errors_without_writing` — normalized `old_string` matches
   2 windows → error listing line ranges; file unchanged.
5. `normalized_falls_through_on_content_diff` — difference is real content (curly vs
   straight quote / em-dash — the captured KDoc case) → 0 normalized matches →
   nearest-text error; file unchanged.
6. `normalized_apply_aborts_on_introduced_syntax_error` — relaxed edit that breaks parse
   on an AST-supported language → rejected; file unchanged.
7. `normalized_apply_allowed_when_file_already_broken` — file already has syntax errors;
   relaxed edit adds none new → applies (before/after semantics).
8. `mid_line_old_string_stays_exact_only` — non-line-aligned `old_string` miss → no
   normalized attempt; nearest-text error.

## Prompt-surface impact

`edit_file`'s behavior gains a recovery mode and a new response field. Per `CLAUDE.md`
"Prompt Surface Consistency": review the `edit_file` tool description and the
`error-handling` / iron-laws guide text — the LLM should know `edit_file` now auto-recovers
from whitespace-only misses and returns a verify note. Expected to be a
`server_instructions` / tool-description change only → **no `ONBOARDING_VERSION` bump**
(confirm during the plan: no change to the `onboarding_prompt` surface or `builders.rs`).

## Out of scope (v1)

- Batch `edit[]` path (`edit_file/mod.rs:335`) — follow-up once the core proves out.
- Mid-line / non-line-aligned normalized matching.
- Internal-whitespace or unicode-punctuation normalization.
- Fuzzy/similarity matching.
- External linters (ktlint/clippy/eslint) — the in-process tree-sitter check is the
  backstop; external toolchains are too slow/heavy for an edit tool and belong to the
  agent's own build/test step.

## References

- `docs/issues/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — bug + evidence;
  its Fix section should point here.
- `src/tools/edit_file/mod.rs:453-539` (`perform_edit`); `:479-484` (the `match_count == 0`
  branch this design fills); post-write `has_syntax_errors` check.
- `src/tools/edit_file/tests.rs:40,152` — `test_ctx` / `project_ctx` harness.
- `docs/PROGRESSIVE_DISCOVERABILITY.md`; `CLAUDE.md` Design Principles (No-Echo exception),
  Prompt Surface Consistency, Testing Patterns.
