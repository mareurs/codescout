# Design — Closest-match diagnostics for scoped-edit misses

**Date:** 2026-07-19
**Status:** proposed
**Author context:** brainstormed with the Architecture Snow Lion lens; scouted `plan_scoped_edit` + `apply_body_edits`.

---

## 1. Problem

A scoped `action="edit"` (heading + `old_string` → `new_string`) fails with:

```
old_string not found in section '## State'. The text must match exactly (whitespace-sensitive).
```

The message names *that* the match failed but not *why* or *what is actually there*, so the agent must issue a **blind re-read** of the section, then a second edit — two wasted round-trips per miss.

Observed instance (`claude-plugins/docs/trackers/version-bump-checklist.md`, an augmented tracker):
- Agent's `old_string`: `` _Last refresh: `8481bea`_ ``
- Current body: `` _Last refresh: `ddf8215`_ `` (the value drifted between the agent's read and its edit — this tracker re-renders on every refresh, `refresh_count: 45`)

The single most common cause of this failure class is **invisible byte differences** that look identical on screen: a non-breaking space (U+00A0), a trailing space, tab-vs-spaces, a zero-width char, or CRLF-vs-LF.

### Leverage

`plan_scoped_edit` (`src/tools/markdown/edit_markdown.rs`) is the **shared chokepoint**: both `edit_markdown` and the librarian's `apply_body_edits` (`src/librarian/tools/update.rs`, `artifact(update, patch={body_edits:[...]})`) call it for scoped edits. Enriching the not-found error here improves both surfaces at once. The error is emitted as `anyhow::anyhow!(...)`; both callers wrap it in a `RecoverableError` with a generic hint — so the enriched message content rides through unchanged on every surface.

## 2. Goals / non-goals

**Goals**
- When `old_string` is not found, show the agent the **closest actual text** in the section so the next edit is one-shot, with no blind re-read.
- **Classify** the specific difference that caused the miss — especially invisible-character differences — as a first-class output.

**Non-goals (explicitly deferred)**
- **No auto-repair.** The tool never retargets a write to a whitespace-normalized match. Writes get a higher bar (repair-and-continue law: accept an explicit target, never auto-guess one), and the code cases below make silent retargeting unsafe.
- **No augmented-render detection/enforcement.** The deeper issue — that `## State` is a render of params and should be updated via `params`, not `body_edits` — is a separate work stream. We do NOT detect rendered regions or block edits to them. We DO, however, surface a context-aware **hint** toward params on the failure signature that warrants it (§4b) — a nudge, not a gate.

**Goals (added)**
- On a miss, return the **most useful next-action hint** given the failure tier and the surface — not a fixed string (§4b).

## 3. Core principle — tolerant to find, exact to show, never to change

Whitespace-insensitivity is safe for *locating* the intended line but dangerous for *displaying or applying* text. Ignoring whitespace to display would **hide** the exact byte that caused the miss (the agent copies normalized text and it *still* fails), and to apply would corrupt semantically-significant whitespace:

- **Code (whitespace = syntax):** Python indentation (`    x` vs `\tx` vs `        x` are different programs), Makefile tabs, YAML indentation, heredocs/diff blocks inside ``` fences.
- **Markdown structure:** two trailing spaces = a hard `<br>`; leading indent = nested list / blockquote level.
- **Invisible chars:** NBSP / zero-width / CRLF — the very thing to *reveal*, not smooth over.

Therefore: **locate** by raw character similarity (no whitespace folding needed — raw similarity already ranks both the SHA drift and whitespace misses correctly), then **classify** the difference on the raw bytes, and always **show raw bytes** with whitespace made visible. Nothing is normalized for output; nothing is auto-applied.

## 4. The tiered diagnostic

On `!section.contains(old_string)`, locate the best-matching line (single-line `old_string`) or contiguous same-line-count window (multi-line `old_string`) by raw normalized similarity, then:

| Tier | Condition | Message |
|---|---|---|
| **A — invisible/whitespace** | Best match's differing chars are **all** whitespace/invisible | `old_string not found in '<h>'. Closest line differs only in whitespace/invisible characters: <classification>. Copy these exact bytes:` + the real line rendered with whitespace visible |
| **B — visible drift** | Best match differs in visible content, similarity ≥ threshold | `old_string not found in '<h>'. Closest text (did it change since you read it?):` + `want:`/`have:` (byte-exact) |
| **C — nothing close** | Best similarity < threshold | current generic message + `No similar text in this section — is it under a different heading? (available: <first few section lines / sibling headings>)` |

Tier A **classification** names the first divergence precisely (one or more of):
- `tab where old_string has N spaces (col C)`
- `trailing space(s) on the file line`
- `non-breaking space U+00A0 at col C (looks like a normal space)`
- `zero-width character U+200B at col C`
- `line endings differ: file is CRLF, old_string is LF`
- `leading indent differs: file has N, old_string has M`

Whitespace rendering for display uses visible markers (e.g. `·` space, `→` tab, `⏎` for a trailing line-end, `⟨NBSP⟩` / `⟨ZWSP⟩` for named invisibles) so the difference is *seeable* in a terminal.

**Code-fence awareness.** If the best-match line lies inside a ``` fenced block or a ≥4-space indented code block (reuse `compute_section_end`'s fence tracking logic), append: `note: inside a code block — whitespace is significant; copy the bytes exactly.`

## 4b. Context-aware hint — the most useful next action

The diagnostic (§4) tells the agent *what* is wrong. The **hint** tells it *what to do next*, and the most useful next action depends on the failure signature **and the surface**. Selection is layered:

**Tier-adaptive base hint (shared, in `plan_scoped_edit`):**

| Tier | Base hint |
|---|---|
| A — whitespace/invisible | "Copy the exact bytes shown above — the only difference is invisible whitespace." |
| B — visible drift | "The text changed since you last read it — re-read this section for the current value, then retry with it." |
| C — nothing close | "old_string isn't in this section — verify the heading; available sections: `<…>`." |

**Surface-aware enrichment (in the librarian `apply_body_edits`, which knows the target is augmented):** when the miss is **Tier B on an augmented artifact**, append the nudge that would have actually resolved the motivating failure:

> "This artifact is augmented (params + prompt render its body). A drifted value usually means the body is a *render of `params`* — update `patch={params:{…}}` and re-render rather than hand-editing the rendered text."

This is the deliberately-minimal bridge to the deferred augmented-render work: a **hint, not detection or enforcement**. It fires precisely on the signature that warrants it (value drift + augmented target), so it never nags on a whitespace miss or a wrong-heading miss.

**Mechanism.** `plan_scoped_edit` returns the miss as a **downcastable typed error** — `ScopedEditMiss { tier, heading, diagnostic, base_hint }` — whose `Display` renders `diagnostic` + `base_hint`, so `edit_markdown` surfaces the full message with zero extra work. `apply_body_edits` calls `err.downcast_ref::<ScopedEditMiss>()` and, **only** when it holds an augmented artifact and `tier == VisibleDrift`, appends the params nudge. Any caller without extra context simply shows the `Display` output. The tier signal crossing the boundary is what makes the hint context-aware rather than a fixed string.

## 5. Algorithm

```
locate(old_string, section):
  old_lines = old_string.split('\n')
  windows   = contiguous windows of section lines with len == old_lines.len()
  best      = argmax over windows of similarity(join(window), old_string)   # raw, normalized Levenshtein in [0,1]
  return (best_window, best_score)
```

- **Similarity:** normalized Levenshtein (`1 - dist/maxlen`) over raw bytes/chars. Reuse a `strsim`-style dependency if one is already vendored; otherwise a ~20-line internal `levenshtein` (small inputs, no perf concern).
- **Bounds (always cheap):** skip the fuzzy pass and fall straight to Tier C when the section exceeds a cap (e.g. `> 400` lines or `> 64 KB`), or when `old_string` is empty. The common section is < 1 KB.
- **Threshold:** Tier B/C boundary at a fixed similarity (e.g. `0.5`); tune with tests. Below it, "closest" would be noise.
- **Output size:** cap the shown snippet — a single line (truncated with `…` past e.g. 200 chars) for single-line; for multi-line, show only the diverging lines of the window, not the whole thing (progressive-disclosure discipline).

## 6. Where it lives

- All new diagnostic logic in `src/tools/markdown/edit_markdown.rs`, invoked from the existing `!section.contains(old_string)` branch of `plan_scoped_edit`.
- **Typed error:** a new `ScopedEditMiss { tier: MissTier, heading: String, diagnostic: String, base_hint: String }` (with `MissTier { WhitespaceInvisible, VisibleDrift, NoClose }`), implementing `Display` (renders `diagnostic` + `base_hint`) and `std::error::Error`. `plan_scoped_edit` returns `Err(ScopedEditMiss{…}.into())` — an `anyhow::Error` that is **downcastable**. Signature stays `Result<Vec<PlannedEdit>>`.
- New private helpers (same file): `diagnose_scoped_miss(section, old_string, heading) -> ScopedEditMiss`, `similarity(a, b) -> f64`, `classify_whitespace_diff(want_line, have_line) -> Option<String>`, `render_visible_whitespace(line) -> String`, `line_in_code_block(section, line_idx) -> bool`.
- **`edit_markdown`** callers: no change needed — the wrapper's `format!("...{e}...")` picks up the rich `Display`. (Keep the existing generic `RecoverableError` hint, or drop it in favor of `base_hint` — decide in the plan; do not double-hint.)
- **`apply_body_edits`** (`src/librarian/tools/update.rs`): after the `perform_scoped_edit`/`plan_scoped_edit` call errors, `err.downcast_ref::<ScopedEditMiss>()`; when it holds an **augmented** artifact (the update handler already has the augmentation loaded) and `tier == VisibleDrift`, append the params-nudge hint from §4b. Otherwise surface the `Display` output unchanged.
- No tool-schema change on either surface.
## 7. Testing

- **Tier B (the motivating case):** `old_string` with a stale SHA → message contains both the stale and current SHA lines, byte-exact.
- **Tier A — each invisible class:** NBSP-vs-space, trailing space, tab-vs-4-spaces, zero-width char, CRLF-vs-LF — each asserts the specific classification string and that the shown bytes are exact (not normalized).
- **Tier A must not fire on visible drift**, and Tier B must not fire on a pure whitespace diff (classification gate correctness).
- **Tier C:** genuinely absent `old_string` → generic + "different heading?" hint; and the large-section bound → Tier C without running the fuzzy pass.
- **Multi-line `old_string`:** best-window location + per-line divergence display.
- **Code-fence note:** a miss on a line inside a ``` block appends the significance warning.
- **Regression:** all existing `plan_scoped_edit` / scoped-edit tests still pass (the success path is untouched; only the error branch changes).
- **Cross-surface:** an `apply_body_edits` (librarian) scoped-edit miss surfaces the enriched message (proves the chokepoint carries through).

**Context-aware hint tests**
- Each tier's `Display` carries the correct tier-adaptive base hint (A: "copy exact bytes"; B: "changed since you read it — re-read"; C: "verify the heading").
- `ScopedEditMiss` round-trips through `anyhow::Error` and `downcast_ref` recovers `tier`.
- Librarian enrichment matrix: augmented + `VisibleDrift` → params nudge appended; augmented + `WhitespaceInvisible` → **no** nudge; augmented + `NoClose` → no nudge; non-augmented + `VisibleDrift` → no nudge (nudge is librarian-only). Asserts the hint is precise, not noisy.

## 8. Open questions

- Exact similarity threshold and section-size cap — pin empirically in tests.
- Whether to reuse an existing edit-distance crate vs a small internal fn — resolve in the plan by checking `Cargo.toml`.
- Terminal-safety of the visible-whitespace markers (avoid ambiguous glyphs); the classification text is the load-bearing signal, markers are a secondary aid.

## 9. Decision record (ADR summary)

**Decision:** On a scoped-edit `old_string` miss, `plan_scoped_edit` emits a tiered diagnostic — invisible/whitespace classification (Tier A), closest visible-content diff (Tier B), or a "wrong heading?" nudge (Tier C) — locating by raw similarity and showing byte-exact text with whitespace made visible. Each miss carries a **context-aware, tier-adaptive hint** (via a downcastable `ScopedEditMiss`), with a librarian-only params nudge on the augmented + value-drift signature. Diagnose-only; never auto-repair; never normalize output.
**Alternatives:** auto-repair whitespace-normalized matches (rejected — unsafe for code/structure, violates the write-target-never-guessed bar); bare "not found" (status quo — forces blind re-reads).
**Consequences:** miss path drops from two wasted round-trips to one; every scoped-edit surface benefits via the shared chokepoint; adds a small, well-tested char-classification helper. Cost is bounded (fuzzy pass gated by section size).
**Confidence:** high on the model and the diagnose-only decision; medium on threshold/cap constants (tests will pin them).
