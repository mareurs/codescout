---
kind: bug
status: open
tags:
- cluster/capped-result-presented-as-complete
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
---

# BUG: `symbols(name=…)` renders a wrapped signature truncated at the open paren, and it reads as complete

## Summary

For a Rust function whose signature wraps across lines, `symbols(name=…)` prints the
declaration's **first line only** — `pub fn discover_projects(` — with no ellipsis, no
`… (truncated)` marker, and nothing else distinguishing it from a complete single-line
signature. An agent reading it gets **arity 0 and no return type** for a function that
takes three parameters and returns `Vec<DiscoveredProject>`. A third behaviour exists for
the same input class: some wrapped-signature functions render **no signature line at all**.

This is the defect the tool exists to prevent. `symbols` is Iron Law 1's prescribed
replacement for reading source, and the failure is silent in the direction that matters:
it does not withhold the shape, it supplies a **wrong** one that looks right.

## Symptom (Effect)

Three different renderings for the same query shape, in one repo, at
`6d89a69b`:

**1 — single-line signature: correct and complete.**

```
symbols(name="first_declaration_paragraph")

src/librarian/statements.rs (1)
  Function  131  first_declaration_paragraph
      fn first_declaration_paragraph(section_text: &str, re: &Regex) -> Option<String> {
```

**2 — wrapped signature: truncated at the paren, presented as complete.**

```
symbols(name="discover_projects")

src/workspace.rs (1)
  Function  29  discover_projects
      pub fn discover_projects(
```

**3 — wrapped signature: no signature line at all.**

```
symbols(name="stale_to_remove")

src/heartbeat.rs (4)
  Function  190  stale_to_remove
  Function  411  stale_to_remove_age_floor_retains_recent_victim
  Function  385  stale_to_remove_keeps_newest_n
  Function  401  stale_to_remove_noop_under_cap
```

Case 2 is the harmful one. Case 3 is merely unhelpful — an absent signature prompts a
follow-up call; a truncated one does not, because nothing marks it as partial.

Ground truth for case 2, via `symbols(name="discover_projects", include_body=true)`:

```rust
pub fn discover_projects(
    workspace_root: &Path,
    max_depth: usize,
    exclude: &[String],
) -> Vec<DiscoveredProject> {
```

**3 parameters, returns `Vec<DiscoveredProject>`.** The rendered form supports neither
fact and contradicts both.

## Reproduction

Commit `6d89a69b`, branch `experiments`, this repo, via live MCP:

1. `symbols(name="discover_projects")` — observe `pub fn discover_projects(`
2. `symbols(name="discover_projects", include_body=true)` — observe the real 3-arg
   signature
3. `symbols(name="stale_to_remove")` — observe no signature line at all

The overview form is a fourth data point and is *correct* about its own limits — it shows
no signature for top-level functions and so makes no false claim:

```
symbols(path="src/librarian/statements.rs")

  Function  131-174  first_declaration_paragraph
      Function  132-134  first_declaration_paragraph/is_bold_field_label  fn(line: &str) -> bool
```

Note the nested function *does* carry `fn(line: &str) -> bool`. So signature presence
varies by nesting as well as by wrapping.

## Environment

- codescout at `6d89a69b`, branch `experiments`
- rust-analyzer via the LSP mux; project `codescout`
- Observed through live MCP from a Claude Code session, `detail_level` default
  (`exploring`)

## Root cause

**Verified 2026-09-02** by reading the call path and by probe. The rendered "signature"
is not a signature field at all — it is a **one-line slice of the body**, which is why
nothing marks it as truncated: no code on this path knows it cut anything.

1. `symbols(name=…)` search routes through `client.workspace_symbols(&pattern)`
   (`src/tools/symbol/symbols.rs:591`) — the language server, not the AST.
2. rust-analyzer's `workspace/symbol` answers with a range covering the declaration's
   name line only, so the match arrives with **`end_line == start_line`**.
3. `focus_single_symbol` (`src/tools/symbol/symbols.rs:990`) guards on
   `if start == 0 || end < start { return; }` — **equality passes**. It computes
   `line_span = 1`, classifies a function as a leaf, and takes the final branch:
   *"Leaf (any size) or a small container: inline the full body"*, slicing
   `lines[start-1..end]` — **one line**.
4. That line is the first line of the declaration. Single-line signature → complete and
   correct. Wrapped signature → `pub fn discover_projects(`, arity 0, no return type.

**The defect is invisible without a language server.** With no LSP running, codescout
falls back to AST extraction, which reports true full ranges, `line_span` is the real
span, and the whole body inlines correctly. Measured on a fixture crate: cold probe
showed the full body (no defect visible); after a 90 s rust-analyzer warm-up the *same
binary on the same file* showed `pub fn resolve_manifest(`.

That property is worth stating separately because it inverts the usual testing
assumption: a test or fixture that does not wait for the language server observes the
**correct** behaviour and passes. The bug is only reachable when the tool is working at
full capability.

`auto_inline_small_bodies` (`symbols.rs:912`, `AUTO_INLINE_MAX_MATCHES = 2`,
`AUTO_INLINE_MAX_LINES = 40`) is a *different* path and is **not** the cause. An earlier
revision of this file blamed it, and blamed `extract_signature`
(`src/embed/ast_chunker.rs:408`); both were wrong. Recorded here rather than deleted
because the 40-line threshold is a plausible-looking explanation that fits three of the
four observations and will re-suggest itself to the next reader.

**Why `stale_to_remove` shows nothing** (§ Symptom case 3): the name matched **4**
symbols. `focus_single_symbol` acts on `matches.first_mut()` and the multi-match display
path does not render an inlined body, so the one-line slice never appears. Same root
cause, different surface.
## Evidence

### Existing test does not cover this

`validate_symbol_position_accepts_multiline_signature`
(`src/tools/symbol/tests.rs:4241`) looks like coverage and is not. It asserts
`validate_symbol_position(&sym, &lines).is_ok()` — a *position* property — and its fixture
sets `detail: None` without asserting anything about what gets rendered. The wrapped
signature is fixture furniture, not the subject.

This is `cluster/assertion-satisfiable-by-accident` adjacent: a test named for the input
class, asserting a different property of it.

## Hypotheses tried

1. **Hypothesis:** the truncation is the known `truncate_compact` cap.
   **Test:** checked for the `… (truncated)` marker that path appends
   (`src/tools/core/tests.rs:1095`).
   **Verdict:** rejected — no marker present.
   **Evidence:** § Symptom, case 2.

2. **Hypothesis:** the multi-line case is already covered by an existing test.
   **Test:** read `validate_symbol_position_accepts_multiline_signature` in full.
   **Verdict:** rejected — asserts position validity, `detail: None`, no render assertion.
   **Evidence:** § Evidence.

## Fix

Not attempted. Two things the fix must decide, and they are separable:

1. **Completeness** — render the full declaration through the closing `)` and return type,
   or
2. **Honesty** — if a partial render is kept for budget reasons, mark it, the way every
   other capped surface in this codebase does. `docs/PROGRESSIVE_DISCLOSURE.md` is the
   governing convention and `truncate_compact`'s `… (truncated)` is the in-repo precedent.

(2) alone closes the harm even if (1) is judged too expensive: a marked truncation sends
the agent to `include_body=true`; an unmarked one does not.

Case 3's silent absence should be reconciled in the same change — three renderings for one
input class is itself the bug.

## Tests added

None yet. The regression test must assert on the **rendered output**, not on
`SymbolInfo`, and must cover all three observed cases — a test written only against
`discover_projects` would pass under a fix that leaves `stale_to_remove` blank.

## Workarounds

`symbols(name=…, include_body=true)` returns the true signature in every case observed.
That is the only reliable route to a function's shape today.

## Resume

Find the render path for the per-symbol detail line: start at the `symbols` search-mode
formatter in `src/tools/symbol/`, and determine what supplies the line printed under
`Function <line> <name>`. Confirm whether it is `SymbolInfo::detail` from the LSP, a
first-source-line fallback, or `extract_signature`
(`src/embed/ast_chunker.rs:408`). Then re-run the three § Reproduction probes against a
candidate fix — all three, not just `discover_projects`.

## References

- Found while scouting the CAP-10 practice-rule eval, 2026-09-02. The rule under test
  asserts *"an overview gives you names; it does not give you shapes"* — the scout that
  checked that premise against the real tool found this instead.
- `docs/trackers/prompt-hamsa-audit-log.md` — A-35 / A-36, the two output-mode audits that
  preceded it.
- `docs/PROGRESSIVE_DISCLOSURE.md` — the capped-surface convention this violates.
