# Escaped-quote recovery tier for `edit_file`

**Date:** 2026-06-16
**Status:** implemented (2026-06-16) — `decode_literal_escapes_incl_quotes` + second recovery rung in `perform_edit`; 3 unit + 4 integration tests
**Components:** `src/tools/edit_repair.rs` (`decode_literal_escapes`), `src/tools/edit_file/mod.rs` (`perform_edit`)
**Related designs:**
- `2026-06-04-edit-file-whitespace-normalized-fallback-design.md` — the normalized-windows fallback this sits beside
- `2026-06-16-edit-file-structural-guard-diff-aware-design.md` — sibling `edit_file` fix from the same scout (independent code path)

## Problem

When an `edit_file` `old_string` does not match the file, `perform_edit` runs a recovery
ladder before failing: **escape-decode** → **whitespace-normalized windows** → AST gate →
not-found. The first rung, `decode_literal_escapes`, repairs *literal* `\n`/`\t`/`\r` that
an MCP client delivered un-decoded. It **deliberately leaves escaped quotes (`\"`, `\'`)
intact** (`src/tools/edit_repair.rs:28`, comment + the `decode_literal_escapes_leaves_other_escapes_intact`
test pin this).

Both recovery rungs are therefore **blind to over-escaped quotes**:
- escape-decode decodes the newline but leaves `\"` → decoded string still ≠ file;
- the normalized-windows fallback only trims *whitespace* per line — it does not touch
  interior `\"` vs `"` — so trimmed lines still differ → 0 windows → "No exact or
  whitespace-normalized match."

### Evidence (usage.db, 2026-06-02 → 2026-06-14)

`edit_stale_match` on `edit_file` = **13** failures (the `err_family` also lumps in 4
`edit_markdown` + 1 `read_markdown` — those are out of scope here). Of the 13, **5 trace
to one 7-minute session** (`src/tools/memory/tests.rs`, cc_session `126c61d2`,
2026-06-09 08:32–08:39): the agent repeatedly sent over-escaped quotes, e.g.

```
old_string = "    assert!(!project_mem_path.exists(), \"memory should be deleted\");\n}"
```

where the file holds a real newline and plain quotes `"`. The `\n` decoded; the `\"`
did not; no match; the agent retried five times. **One unhandled escape class = 38% of
edit_file stale-matches** in the window — and recurring as recently as 2026-06-14 (live,
not a graveyard).

## Goal / success criteria

- An `old_string` whose **only** mismatch from the file is over-escaped quotes (`\"`/`\'`,
  with or without accompanying `\n`/`\t`/`\r`) is **auto-recovered and applied**, gated on a
  unique match — the same safety contract the existing escape-decode tier uses.
- The conservative `decode_literal_escapes` contract and its tests are **unchanged**:
  default decoding still leaves quotes intact for every other caller.
- Whitespace-significant languages (py/yaml/hs) remain safe — quote decoding is
  whitespace-neutral, so it does not reintroduce the indentation hazard the normalized
  fallback is disabled for.

## Design

A **second recovery rung** in `perform_edit`'s `match_count == 0` block, placed immediately
after the existing conservative escape-decode attempt and **before** the
`indentation_significant` bail (quote decoding is whitespace-neutral, so it is valid for
py/yaml too).

### `src/tools/edit_repair.rs`

Refactor to avoid duplication: extract the existing body into a private inner fn with a
flag; keep the public conservative wrapper; add a quote-inclusive wrapper.

```rust
fn decode_literal_escapes_inner(s: &str, decode_quotes: bool) -> Option<String> {
    // identical scan; in the match on chars.peek(), additionally handle
    //   Some('"') | Some('\'') if decode_quotes => push the unescaped quote, set changed
    // \\ (doubled backslash) is NOT decoded — see Decisions.
}

/// Conservative: \n \t \r only. Unchanged public contract (quotes left intact).
pub(crate) fn decode_literal_escapes(s: &str) -> Option<String> {
    decode_literal_escapes_inner(s, false)
}

/// Aggressive recovery: also decodes \" and \'. Used only as a second-tier
/// recovery after the conservative decode fails to produce a match.
pub(crate) fn decode_literal_escapes_incl_quotes(s: &str) -> Option<String> {
    decode_literal_escapes_inner(s, true)
}
```

### `src/tools/edit_file/mod.rs::perform_edit`

After the existing `if let Some(decoded_old) = decode_literal_escapes(old_string) { … }`
block, add:

```rust
// Second-tier recovery: over-escaped quotes (\" / \'). A common MCP-client failure
// (5/13 edit_file stale-matches, 2026-06-09). Same unique-match gate keeps it safe:
// we only reach here because the exact match already failed, and we only apply on a
// unique decoded match. Decodes both old and new (an over-escaping client over-escapes
// both); the "verify the result" note flags the rare asymmetric case.
if let Some(decoded_old) = decode_literal_escapes_incl_quotes(old_string) {
    let dcount = content.matches(decoded_old.as_str()).count();
    if dcount == 1 || (replace_all && dcount >= 1) {
        let decoded_new = decode_literal_escapes_incl_quotes(new_string)
            .unwrap_or_else(|| new_string.to_string());
        let candidate = content.replace(decoded_old.as_str(), &decoded_new);
        let new_content = finalize_edit_content(
            std::path::Path::new(path), &content, candidate, &decoded_new,
            |d| content.replace(decoded_old.as_str(), d),
        ).into_content();
        commit_edit(ctx, &resolved, &new_content).await?;
        return Ok(json!({
            "status": "ok",
            "applied_via": "escape-decoded match (quotes)",
            "note": "old_string matched after decoding escaped quotes; verify the result"
        }));
    }
}
```

## Invariants preserved

- **`decode_literal_escapes` default behavior unchanged** — quotes still survive for every
  existing caller (including the conservative first tier and the `new_string` decode there).
  Its three existing unit tests pass unmodified.
- **Unique-match gate** (`dcount == 1 || (replace_all && dcount >= 1)`) reused verbatim —
  the new tier can never apply an ambiguous replacement.
- **Recovery ordering** — conservative tier first; the quote tier is strictly additive and
  only runs when the conservative decode produced no unique match. `\n`-only inputs behave
  exactly as today.
- **Indentation-significant safety** — quote decoding touches no leading/trailing
  whitespace, so running it before the `indentation_significant` bail is sound (unlike the
  normalized-windows fallback, which is correctly disabled there).

## Decisions / edge cases

- **Decode both `old_string` and `new_string` with quotes.** A client that over-escapes the
  match target almost always over-escapes the replacement too. The rare exception
  (`old_string` over-escaped, `new_string` *intentionally* carries `\"`) is caught by the
  existing `"verify the result"` note — the same safety valve the `\n`/`\t` tier already
  relies on. Applying the *un*-decoded `new_string` instead would write literal `\"` into
  the file, which is strictly worse.
- **Decode `\"` and `\'` only — never `\\`.** Doubled-backslash decoding is genuinely
  dangerous (regex literals, Windows paths) and the data shows no friction for it.
  Conservative by default; recover only the class agents actually fumble.
- **`applied_via` distinct value** (`"escape-decoded match (quotes)"`) so telemetry and the
  Pika usage scan can tell the new tier apart from the conservative one.

## Out of scope

- **Batch mode (`edits[]`) has no recovery ladder at all** (exact-match only — no decode, no
  normalized windows, no nearest-content hint). That asymmetry is a separate, latent issue
  (code-confirmed, not yet observed biting `edit_file` in the data) and is best fixed by
  factoring the single-path resolver into a shared helper — a refactor tracked separately.
- The `err_family = edit_stale_match` classifier conflating `edit_file` / `edit_markdown` /
  `read_markdown` is a telemetry-accuracy issue, not a user friction — noted, not fixed.

## Tests

**Unit (`src/tools/edit_repair.rs` tests):**
1. `decode_literal_escapes_incl_quotes("a\\\"b")` → `Some("a\"b")`; conservative
   `decode_literal_escapes("a\\\"b")` → `None` (contract unchanged).
2. Mixed: `decode_literal_escapes_incl_quotes("x\\nassert(\\\"m\\\")")` →
   `Some("x\nassert(\"m\")")` (newline *and* quotes decoded).
3. `decode_literal_escapes_incl_quotes("a\\\\b")` leaves the doubled backslash intact
   (no `\\` decoding).

**Integration (`perform_edit`):**
4. **Regression — the 5× cluster (single-line variant).** File contains
   `assert!(x, "msg");`; `old_string = assert!(x, \"msg\");` (over-escaped, no newline).
   Assert applied with `applied_via == "escape-decoded match (quotes)"`.
5. **Regression — newline + quotes** (the literal tests.rs shape). Assert recovery applies.
6. **Safety: genuine `\"` in file is untouched.** File contains a Rust literal
   `let s = "a\"b";`; `old_string` matches it exactly (with the real `\"`). Assert
   `match_count > 0` → exact path applies, recovery never entered (`applied_via` absent).
7. **Safety: ambiguous decode does not apply.** Quote-decoded `old_string` matches >1 region
   and `replace_all` is false → the tier does NOT write; flow falls through to the normalized
   fallback / not-found.

**Test-construction note** (CLAUDE.md edit_file rule): the recovery fixtures must keep
`old_string` a non-substring of the file (the `\"` form is not literally present — the file
has plain `"`), so the exact path correctly misses and the recovery rung is the one
exercised. Assert on the `applied_via` marker so a mis-route fails loudly rather than passing
on the exact path.

## Prompt surface impact

None required — the recovery is transparent to the caller (it just succeeds, with an
informational `note`). No tool signature, parameter, or routing change. **No
`ONBOARDING_VERSION` bump.** Optional: a one-line mention in the `edit_file` tool
description or `error-handling` guide that over-escaped quotes are auto-recovered — low
priority, not load-bearing.
