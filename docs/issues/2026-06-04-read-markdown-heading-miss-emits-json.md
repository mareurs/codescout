---
status: open
opened: 2026-06-04
closed:
severity: low
owner: marius
related:
  - docs/issues/2026-06-04-edit-file-old-string-miss-no-closest-match.md
tags: [read_markdown, error-ux, output-form, token-efficiency, progressive-disclosure]
kind: bug
---

# BUG: read_markdown heading-not-found emits verbose JSON instead of the compact text heading-map

## Summary
`read_markdown(path, heading=<missing>)` returns a verbose, pretty-printed JSON envelope
(`{ok:false, error, hint, headings:[{h,l},…]}`) when the requested heading doesn't exist.
The successful/default call returns the *same* heading information as a compact indented
text map (`  ## Voice  L3`). The tool already owns the text renderer that produces the
compact form — the error path just doesn't use it, because errors bypass the tool's
`OutputForm::Text` rendering. Same bug class as the edit_file zero-match asymmetry: the
error path discards a success-path renderer that's already in the file.

## Symptom (Effect)
Observed verbatim (calling a heading that doesn't exist):

```json
{
  "ok": false,
  "error": "heading \"## edit_file old_string not found\" not found",
  "hint": "pick a heading from `headings` array or use start_line/end_line",
  "headings": [
    { "h": "# Usage Analysis — 2026-05-29 (last 2 days, friction focus)", "l": 1 },
    { "h": "## Cross-Project Summary", "l": 6 },
    { "h": "### Top Frictions (ranked by recurrence × cross-project spread)", "l": 26 },
    ...
  ]
}
```

Compare a *successful* bare `read_markdown(path)`, which returns the identical heading set
as a compact text map:

```
260 lines  @file_923dda44

# The Codescout Pika  L1
  ## Voice  L3
  ## Operating Principles  L11
  ...
next: use "@file_923dda44" — heading="## Section" or start_line/end_line
```

The JSON form repeats `"h":`/`"l":` keys + braces + pretty-print indentation per heading —
roughly 3–4× the bytes of the equivalent `  ## Heading  L3` text line. For a file near
`HEADINGS_HARD_CAP` (40 headings) that is a meaningful, repeated token cost in an error
that fires often (heading-guessing is a common access pattern).

## Reproduction
Deterministic:

1. `git rev-parse HEAD` → `0930e3a6` (branch `experiments` at time of filing).
2. `read_markdown(path="any/file.md", heading="## DefinitelyNotAHeading")`.
3. Observe the `{ok:false, error, hint, headings:[{h,l},…]}` JSON envelope rather than the
   compact text heading-map the bare `read_markdown(path)` call returns.

## Environment
- codescout `experiments` @ `0930e3a6`; `src/tools/markdown/read_markdown.rs`.
- Surfaces on every heading-not-found and (same mechanism) every multi-heading miss.

## Root cause
`src/tools/markdown/read_markdown.rs`.

- The tool declares `fn output_form(&self) -> OutputForm { OutputForm::Text }`
  (`read_markdown.rs:489-490`) and renders its **success** Value into the compact heading
  map with a custom text renderer (`read_markdown.rs:505-565`, the indent/line-number
  formatting at `:512-513` and `:563-564`):

  ```rust
  let indent = " ".repeat((level - 1) * 2);
  out.push_str(&format!("{indent}{h}  L{l}\n"));
  ```

- The heading-not-found path (`read_markdown.rs:242-246`) returns an *error*, attaching the
  heading list as a structured extra:

  ```rust
  return Err(RecoverableError::with_hint(
      format!("heading {:?} not found", heading_query),
      "pick a heading from `headings` array or use start_line/end_line",
  )
  .with_extra("headings", serde_json::json!(headings_json))   // :246
  .into());
  ```

  where `headings_json` (`read_markdown.rs:237`) is a `Vec<Value>` of `{h, l}` objects.

`OutputForm::Text` only governs the `Ok(Value)` rendering. `RecoverableError` is serialized
by the central error layer as a JSON envelope (`{ok:false, error, hint, …extras}`) and
never passes through the tool's Text renderer. So the heading list — identical data to the
success map — is emitted as raw JSON. The same divergence applies to the multi-heading
not-found path (`read_markdown.rs:301-312`, `with_extra("section_map", …)`).

This is not a correctness defect — the data is complete and `RecoverableError` correctly
sets `isError:false` so sibling calls survive. It is a token-efficiency + consistency gap
that runs against the project's progressive-disclosure ethos (`relevant_guide_topic` for
this very tool is `"progressive-disclosure"`).

## Evidence

### The renderer and the error path are 250 lines apart in the same file
- Text renderer: `read_markdown.rs:489-490` (`output_form → Text`), `:505-565` (heading-map
  formatting).
- Error path that bypasses it: `read_markdown.rs:242-246` (single heading),
  `:301-312` (multi-heading).

### Existing tests pin the current JSON shape
`src/tools/markdown/tests.rs:1633-1637` and `:1503-1507` assert the `hint` + `headings`
JSON fields on the not-found path — so any fix that changes the representation must update
these.

## Hypotheses tried
1. **Hypothesis:** the success path also returns JSON and the client just renders it
   differently. **Test:** read `output_form` + the renderer. **Verdict:** rejected — success
   is `OutputForm::Text` with an explicit text heading-map builder (`:489-490`, `:505-565`);
   the divergence is real and lives in the error path.
2. **Hypothesis:** errors could simply be routed through `output_form`. **Test:** inspect the
   tool/error boundary. **Verdict:** deferred — `output_form` operates on `Ok(Value)`;
   `RecoverableError` is a distinct type serialized centrally. A blanket "render errors via
   output_form" change is broader than this bug; the targeted fixes below avoid it.

## Fix
**Plan (not yet implemented).** Two viable approaches; recommend (B) for UX, (A) if minimal
churn is preferred. Either way, factor the heading-map text builder (`read_markdown.rs:505-565`)
into a shared helper so both success and not-found paths use one formatter.

- **(A) Render the heading list as text inside the error.** In the not-found branch
  (`:242-246`), drop `.with_extra("headings", json!(…))` and instead embed the compact text
  map in the error message/hint via the shared helper:
  `format!("heading {q} not found. Available headings:\n{rendered_map}")`. Keeps the
  `{ok:false,…}` envelope but eliminates the verbose `headings:[{h,l},…]` array.

- **(B) Return the heading map as a successful response with a not-found note.** A
  heading-not-found is functionally "here's the map, pick one" — exactly the bare
  `read_markdown(path)` response. Return `Ok` with the heading-map (rendered by the existing
  `OutputForm::Text` path) prefixed by a one-line `requested heading "X" not found` note. No
  JSON, no envelope, full reuse of the success renderer. Most aligned with progressive
  disclosure. Cost: update the tests at `tests.rs:1633-1637` / `:1503-1507` and confirm no
  caller depends on the `ok:false` discriminator (RecoverableError is already `isError:false`,
  so MCP-level sibling-call semantics are unaffected).

Apply the same change to the multi-heading not-found path (`:301-312`).

## Tests added
N/A — not yet fixed. When implemented: a test in `src/tools/markdown/tests.rs` asserting the
not-found response contains the compact `<indent><heading>  L<line>` text form (and, for (B),
that it routes through the Text output form), replacing the current JSON-shape assertions at
`:1633-1637` / `:1503-1507`.

## Workarounds
Functional today — the headings ARE present in the JSON `headings` array, so the agent can
still pick a valid heading from the envelope (this is exactly how the triggering session
recovered). The cost is verbosity/tokens and inconsistency, not lost information.

## Resume
Extract the heading-map text builder at `read_markdown.rs:505-565` into a `fn` (e.g.
`fn render_heading_map(headings) -> String`). Then rewrite the not-found branch
`read_markdown.rs:242-246` per Fix (A) or (B); apply the same to `:301-312`. Update
`src/tools/markdown/tests.rs:1633-1637` and `:1503-1507` to assert the text form. Verify with
`cargo test --lib markdown`. Pairs with the related edit_file error-UX bug — same "error path
ignores the success-path renderer" pattern; consider fixing both together.

## References
- `src/tools/markdown/read_markdown.rs:242-246` (single-heading not-found error),
  `:301-312` (multi-heading), `:237` (`headings_json` shape), `:489-490` + `:505-565`
  (`OutputForm::Text` + heading-map text renderer).
- `src/tools/markdown/tests.rs:1633-1637`, `:1503-1507` (current JSON-shape assertions).
- `docs/issues/2026-06-04-edit-file-old-string-miss-no-closest-match.md` — sibling bug,
  same success/error representation asymmetry.
- `get_guide("progressive-disclosure")` / `docs/PROGRESSIVE_DISCOVERABILITY.md` — the
  token-efficiency philosophy this fix restores.
