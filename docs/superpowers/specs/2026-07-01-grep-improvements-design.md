# Design — grep improvements: index-aware hits, parity flags, overflow ergonomics, correctness

- **Date:** 2026-07-01
- **Status:** approved (brainstorm) — pending implementation plan
- **Scope:** `src/tools/grep.rs` (`Grep::call`, `grep_in_buffer`, `build_grep_regex`) + one AST-extractor call site
- **Author:** Marius (with Claude)

## Motivation

`grep` is heavily used and healthy (MRV-poc usage.db: 1429 calls, 5 errors, avg 25 ms)
but 19 calls overflowed the 50-line cap, and the tool lacks common ergonomics
(case-insensitivity, whole-word, file-type/glob filter, hidden-file opt-in). It also
leaves value on the table: grep owns its match loop (a reimplementation over the `ignore`
crate, *not* a shell-out to `rg`), so it can cheaply attach the **enclosing symbol** to a
hit via the AST index — something plain `rg` cannot do and an LLM consumer usually wants.

## Chosen approach

Extend the existing `grep` tool with new, backward-compatible params. Rejected
alternatives: a separate tool (fractures the agent's mental model), and shelling out to
`rg` (loses `@buffer` grep + AST integration, adds a binary dependency). Every new param
defaults to current behavior.

## New parameters (all declared in `input_schema` — no silent-param-drop)

| param | type | default | effect |
|---|---|---|---|
| `ignore_case` | bool | `false` | `RegexBuilder::case_insensitive(true)` |
| `whole_word` | bool | `false` | wrap pattern as `\b(?:…)\b` |
| `glob` | string \| string[] | — | file filter (`*.rs`, `**/*.md`) via `ignore` overrides |
| `include_hidden` | bool | `false` | search dotfiles / `.github/` (flips `.hidden()`) |
| `mode` | `"lines"` \| `"files"` | `"lines"` | `"files"` = ranked files + per-file counts, no line content |

Symbol annotation (Component 4) is automatic — no param.

## Components

### Component 1 — Shared regex build (refactor; enables 2 & buffer parity)

Today `Grep::call` builds its regex inline while `grep_in_buffer` uses `build_grep_regex`.
Unify both on:

```rust
fn build_grep_regex(pattern: &str, ignore_case: bool, whole_word: bool)
    -> Result<(regex::Regex, bool /* is_literal_fallback */), RecoverableError>
```

Literal-fallback logic (invalid-regex → escape-and-retry unless `is_regex_like`) moves
inside the helper. `whole_word` wraps the (already literal-or-regex) body in `\b(?:…)\b`;
`ignore_case` sets `.case_insensitive(true)`. Both filesystem and `@buffer` paths call it,
so `ignore_case`/`whole_word` work in buffer mode for free.

### Component 2 — ripgrep-parity flags

- `ignore_case`, `whole_word` — via Component 1.
- `glob` — `ignore::overrides::OverrideBuilder` on the `WalkBuilder` (whitelist semantics:
  a glob restricts the walk to matches). Accepts a single string or a list. No `glob` =
  search all (default).
- `include_hidden` — `WalkBuilder::hidden(!include_hidden)`.

### Component 3 — Overflow ergonomics (`mode="files"`)

Returns `{ "files": [{ "file": ..., "count": N }, …], "total": T, "files_count": F }`
ranked by count desc, no per-line content — the `rg -l` + `rg -c` combination. Broad
searches that overflow `lines` mode become a navigable summary. `context_lines` is
ignored in `files` mode. The `lines`-mode overflow hint gains:
*"…or `mode=\"files\"` for a per-file count summary."*

### Component 4 — Index-aware hits (differentiator)

In **`lines` mode, when the result set does not overflow** (`total ≤ limit`): for each
distinct **source-language** file among the matches, parse once with the tree-sitter AST
extractor `extract_symbols_from_source` (`src/ast/parser.rs`) — *not* LSP (deterministic,
avoids the lazy-start shape-shift documented in memory `gotchas`). Build an interval map
of `(start_line, end_line) → symbol` and attach `symbol` = the **innermost** enclosing
symbol name to each hit.

- Skipped for: markdown/config/unknown-language files, `files` mode, and any overflowing
  search (bounds cost to ≤ `limit` files).
- One parse per distinct file, memoized in a `HashMap<PathBuf, Vec<Symbol>>` for the call.
- Innermost = smallest range containing the match line.

Output shape (simple mode) gains an optional `symbol` on each item:
```json
{ "file": "src/tools/file_summary/file_summary.rs", "line": 557,
  "content": "fn parse_bracket(inner: &str) -> …", "symbol": "parse_bracket" }
```

### Component 5 — Correctness / guidance

- **Gate the identifier suggestion** on `total == 0`. Today it fires on every
  identifier-shaped pattern regardless of hit count — misleading on empty results (the
  archived `docs/issues/archive/2026-05-09-grep-buffer-false-negatives.md` noted this). When
  there are matches, the agent wants text grep; don't nag. (Threshold tunable; start at 0.)
- **Non-UTF8 files:** today `read_to_string` fails → silent `continue`. Switch to a byte
  read + binary sniff (NUL byte in first ~8 KB → skip as binary) + `String::from_utf8_lossy`
  otherwise, so latin-1/mixed files are searched. Surface a `skipped_binary` count only
  when `> 0`.

## Data flow (filesystem path, after changes)

```
call(input)
  → parse params (pattern, path, limit, context_lines, ignore_case, whole_word,
                  glob, include_hidden, mode)
  → if path starts with '@' → grep_in_buffer (Components 1,5 apply; 2,3,4 do not)
  → validate_read_path
  → (re, literal_fallback) = build_grep_regex(pattern, ignore_case, whole_word)   [C1]
  → WalkBuilder(hidden=!include_hidden).overrides(glob)                            [C2]
  → for each file: byte-read + binary sniff + lossy decode                         [C5]
       match lines → collect
  → mode == "files"  → ranked file+count summary                                  [C3]
    mode == "lines"  → grouped-by-file (existing)
       if total ≤ limit && !overflow → attach enclosing symbol per source hit     [C4]
  → suggestion only if total == 0                                                  [C5]
```

## Testing (TDD, RED first per `conventions`)

| Component | Tests |
|---|---|
| 1 | `ignore_case` matches mixed case (fs + buffer); `whole_word` excludes substrings; literal-fallback preserved |
| 2 | `glob="*.rs"` includes only rust; multi-glob list; `include_hidden` finds a dotfile; no-glob searches all |
| 3 | `mode="files"` returns ranked counts, no content; overflow hint mentions `mode="files"` |
| 4 | innermost enclosing symbol correct; absent on overflow; absent on markdown; memoized (one parse/file) |
| 5 | suggestion present at 0 matches, absent at ≥1; non-UTF8 latin-1 file matched; binary file skipped + counted |

Fixtures: existing `tests/fixtures/*-library` crates + inline temp files. Assert on
match semantics, not serialized text where cross-platform (per session-log W-5).

## Surfaces to touch (prompt consistency)

- `grep` `description()` — mention `ignore_case` / `glob` / `mode`.
- server_instructions "Search/Edit decision quickref" line for grep.
- `docs/` grep references + `get_guide("progressive-disclosure")` if output shape notes needed.
- No tool rename → `prompt_surfaces_reference_only_real_tools` stays green; still run it.

## Phasing (each independently shippable)

1. **C1 + C5** — shared regex refactor + correctness. Small, high-value, no new surface.
2. **C2** — parity flags.
3. **C3** — `mode="files"`.
4. **C4** — index-aware hits (largest; lands on the refactored base).

## Risks & mitigations

- **C4 cost** — bounded by the no-overflow gate (≤ `limit` distinct files, deduped/memoized).
- **glob default** — a test pins "no glob = search all" so overrides don't accidentally
  become a deny-all.
- **lossy decode** — the NUL sniff prevents binary files from producing garbage matches.
- **buffer parity** — C2's `glob`/`include_hidden`/`mode`/`symbol` are filesystem-only;
  buffer mode gets only C1 (`ignore_case`/`whole_word`) + C5 suggestion-gate. Documented,
  not a gap.

## Non-goals

Multiline (cross-line) matching; replace/rewrite; a separate `type` param (subsumed by
`glob`); LSP-sourced symbols (AST extractor is the deliberate choice); keys containing `]`
in json_path (unrelated).
