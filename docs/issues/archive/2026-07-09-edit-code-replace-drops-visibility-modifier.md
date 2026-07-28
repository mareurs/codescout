---
status: mitigated
opened: 2026-07-09
closed: 2026-07-09
severity: low
owner: marius
related: []
tags: [edit_code, tool-quirk]
kind: bug
---

# BUG: `edit_code(action="replace")` silently drops the symbol's visibility modifier when omitted from the replacement body

## Summary
`edit_code(action="replace", ...)` preserves outer `#[...]` attributes and doc comments
(`///`) automatically per its documented heuristic, but does **not** preserve a plain
visibility keyword (`pub`, `pub(crate)`, etc.) that prefixes the signature. If the
replacement `body` omits the visibility keyword, the function silently loses it —
no error, no warning at edit time; the breakage only surfaces later at compile time
(or not at all, if the symbol happens not to be used outside its module).

## Symptom (Effect)
After calling `edit_code(action="replace", symbol="normalize_err_family", path="src/usage/db.rs", body="fn normalize_err_family(...) { ... }")` (body starting with `fn`, no `pub(crate)`), the tool returned `{"status": "ok", ...}` — no error. `symbols(name="normalize_err_family", include_body=true)` afterward showed the doc comment (`///...`) preserved, but the signature line read `fn normalize_err_family(...)` instead of `pub(crate) fn normalize_err_family(...)`.

The breakage was only caught on the next `cargo test`:
```
error[E0603]: function `normalize_err_family` is private
   --> src/usage/mod.rs:72:31
    |
 72 |             .and_then(|m| db::normalize_err_family(tool_name, m));
    |                               ^^^^^^^^^^^^^^^^^^^^ private function
```

## Reproduction
1. Git commit at time of observation: `52cc35a842e9f291251494a83f06de04b15c636f` (branch `experiments`).
2. Have a `pub(crate) fn foo(...) { ... }` with an outer doc comment, called from another module.
3. `edit_code(action="replace", symbol="foo", path="<file>", body="fn foo(...) { <new body> }")` — omit `pub(crate)` from `body`.
4. Tool returns `{"status": "ok", ...}`.
5. `symbols(name="foo", include_body=true)` — doc comment is preserved; visibility keyword is gone.
6. `cargo build`/`cargo test` from a caller in another module — `E0603: function is private`.

## Environment
codescout MCP server, `edit_code` tool, Rust extractor/editor. Project: codescout,
branch `experiments`, commit `52cc35a8`.

## Root cause
`edit_code`'s replace-path heuristic ("PRESERVES outer `#[...]` attributes ... and
any doc comments captured in the symbol's lead region") only accounts for the
attribute/doc-comment "lead region" ahead of the signature line. A bare visibility
keyword (`pub`, `pub(crate)`, `pub(super)`, etc.) is part of the signature line
itself, not the lead region, so it is not covered by the preserve-by-default
heuristic — and there's no separate preservation path for it. When the caller's
`body` starts directly with `fn` (omitting visibility), the replacement is applied
verbatim, silently downgrading the symbol to private.

## Evidence
Tool call and result (this session):
```
edit_code(action="replace", symbol="normalize_err_family", path="src/usage/db.rs",
          body="fn normalize_err_family(tool_name: &str, msg: &str) -> Option<&'static str> { ... }")
→ {"status": "ok", "replaced_lines": "159-263", ...}
```
Follow-up `cargo test --lib usage::db::tests::normalize_err_family_maps_iron_law_routing_errors`:
```
error[E0603]: function `normalize_err_family` is private
   --> src/usage/mod.rs:72:31
```
Re-running the same `edit_code(action="replace", ...)` call with `body` prefixed
`pub(crate) fn normalize_err_family(...)` fixed it immediately — confirmed by a
clean `cargo test --lib` pass right after.

## Hypotheses tried
1. **Hypothesis:** The attribute-preservation heuristic also covers visibility keywords.
   **Test:** Replaced body without `pub(crate)`, checked `symbols(include_body=true)` output and `cargo test`.
   **Verdict:** rejected — visibility was dropped; doc comment (unrelated to this hypothesis) was preserved, confirming the heuristic only covers the doc-comment/attribute lead region, not the signature's visibility keyword.
   **Evidence link:** see Evidence section above.

## Fix
Not fixed (tool-side; out of scope for the codescout-consuming task in progress).
Suggested tool-side fix: extend `edit_code(action="replace")`'s preserve-by-default
heuristic to also capture and re-apply the original symbol's visibility modifier
when the replacement `body`'s first token is `fn`/`struct`/`enum`/etc. without a
leading `pub`/`pub(crate)`/`pub(super)` — symmetric with how attributes are handled.

## Tests added
N/A — this is a tool-behavior bug in codescout's own `edit_code`, not in the
codescout codebase's product logic under test. No regression test was added
here; if `edit_code` itself gains test coverage for this case, it belongs in
codescout's own test suite for the edit_code tool implementation.

## Workarounds
When using `edit_code(action="replace", ...)` on a symbol with any visibility
modifier (`pub`, `pub(crate)`, `pub(super)`, etc.), always include that modifier
explicitly at the start of the replacement `body` — do not rely on it being
preserved automatically. Verify with `cargo check`/`cargo test` (or `symbols`)
immediately after any such replace, especially on symbols used outside their
own module.

## Resume
N/A — logged as a tool quirk for awareness; no active investigation. If it
recurs or blocks work again, consider filing/fixing in codescout's `edit_code`
implementation (`src/tools/edit_code/` — exact path not yet located).

## References
Discovered while fixing `heading_not_found` classifier quote-mismatch in
`src/usage/db.rs` (commit `52cc35a842e9f291251494a83f06de04b15c636f`).
