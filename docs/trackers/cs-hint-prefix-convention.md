---
kind: tracker
status: draft
title: "[cs-hint] prefix + (BUG-N) cross-reference convention"
owners: []
tags:
  - errors
  - recoverable-error
  - dx
  - convention
---

# Tracker — `[cs-hint]` prefix + `(BUG-N)` cross-reference

## Why

`RecoverableError` already has structured guidance (`Guidance::{Hint, Warning, MustFollow}`) that the MCP transport surfaces as JSON keys (`hint` / `warning` / `must_follow`) under `{"ok": false, "error": ..., <field>: ...}`. The `Display` impl at `src/tools/core/types.rs:238-246` renders this prose-style as:

```
<message> — <field_name>: <text>
```

There is no visual marker that says "this guidance came from codescout." Agents and humans reading interleaved tool output (codescout + host shell + other MCPs + user-quoted strings) have to parse structure to tell them apart. Worse, when a hint covers a documented issue under `docs/issues/<date>-<slug>.md`, there is no stable cross-reference — readers cannot tell "this is the friendly form of a known bug" without re-deriving the link manually.

The proposal — two coupled pieces:

1. **`[cs-hint]` prefix.** `Display` renders codescout-originated guidance with a leading `[cs-hint] ` (or `[cs-warning]` / `[cs-must-follow]`) marker. The JSON envelope is unchanged — the prefix is presentation-layer only.
2. **`(BUG-N)` suffix.** When a hint covers a documented bug file, the call site passes the slug; `Display` renders a trailing `(see docs/issues/<slug>)` and the JSON envelope adds a `bug_ref` field.

## Design surfaces (open)

1. **Where the prefix lives.**
   - **A. Display impl** (`types.rs:238-246`). Cheapest — single edit, auto-applies to every call site. But the doc-comment at `types.rs:232-237` explicitly **promises Display stability** for `to_string().contains(...)` test assertions. Migration cost: every test fixture asserting on stringified `RecoverableError`.
   - **B. `route_tool_error`** (`src/tools/mod.rs`). Inject only on the JSON-emission path, leaving `Display` untouched. But the JSON envelope already has a structural `hint` key — adding `[cs-hint]` inside the string value is double-tagging.
   - **C. Hint text itself.** Every call site of `RecoverableError::with_hint(...)` writes `"[cs-hint] ..."` by convention. No infra change, but no enforcement either — drift inevitable, and the prefix becomes redundant with the JSON `hint` key.
   - **Lean: A.** The accessor `pub fn hint(&self) -> Option<&str>` is already exposed at `src/tools/core/types.rs:224`, so fixtures that need the hint text programmatically already have a stable surface. And because Display *prepends* the marker rather than replacing content, existing `.to_string().contains("...")` assertions (which match on substrings of the message body) keep passing — the migration cost is restricted to assertions that pin the *start* of the string (rare).

2. **Cross-reference shape — what `BUG-N` looks like.**
   - **i. Bare integer** (`BUG-127`). Cute, but `docs/issues/` uses date-slug filenames, not integers — no canonical registry to map against.
   - **ii. Date-slug** (`see docs/issues/2026-05-22-doc-resource-wrong-root`). Verbose but matches the on-disk filename convention exactly. Machine-followable with no resolver.
   - **iii. Short hash** of the issue file path. Stable, opaque — needs a resolver and a registry.
   - **Lean: ii.** Zero resolver cost; renames of the issue file are caught by `audit_doc_refs`.

3. **API shape for the cross-reference.**
   - **a. New constructor** — `RecoverableError::with_hint_for_bug(msg, hint, bug_slug: &str)`.
   - **b. Extend the enum** — `Guidance::HintWithRef(text, slug)` parallel variant.
   - **c. Builder method** — `.with_hint(...).citing_bug("...")` on a builder.
   - **Lean: a.** Keeps the enum stable, no call-site noise for non-cited hints.

4. **Scope of the `[cs-…]` prefix.** Only `RecoverableError`, or also `anyhow::bail!` paths?
   - **Lean: only `RecoverableError`.** `anyhow` bails surface as `isError: true` (fatal), not "hints" — different register, different visual marker if any.

## Counter-arguments

- **"The JSON envelope is already structurally tagged — well-behaved agents should read JSON, not prose."** True for ideal agents. But (a) `Display` strings end up in test fixtures, error chains, host-shell logs, stderr, and user-quoted snippets — surfaces where the JSON key has been flattened away; (b) the prefix costs ~10 chars and makes "grep for codescout hints" trivial in any text surface. The cost is paid once at format time; the readability win is paid every time someone looks.
- **"Adding `(BUG-N)` couples error messages to `docs/issues/` file naming."** Yes — but the coupling is the point. The bug-file archive flow already exists (`docs/issues/` ↔ `docs/issues/archive/` on ship), and `audit_doc_refs` already catches stale references. The convention rides on infra that's already load-bearing.
- **"Why not just trust the agent to follow up on a hint without a tracking link?"** Because hints fire from runtime; the call site that emits them rarely has session context for which issue they correspond to. The slug is the disambiguator — without it, a hint reading "path must be inside project root" is indistinguishable from any other path-related guidance, and the reader can't tell whether it's a known bug or a new symptom.

## Migration cost (rough)

- **~238 call sites** of `RecoverableError::with_hint(...)` across `src/` (verified via `grep 'with_hint\b' src | wc -l`). Most do **not** correspond to a tracked bug; the new constructor is opt-in, so existing sites stay untouched unless a contributor wants to cite a slug.
- **81 `.to_string().contains(...)` assertion sites** across 30 files (verified via codescout `grep('\.to_string\(\)\.contains', path='src')`). Spot-check shows these match on message-body substrings (`"not found"`, `"symbol definition"`, `"unsupported json_path segment"`) — all resilient to a prepended `[cs-hint] ` marker because `contains` is substring-tolerant. Real migration cost: any assertion that pins the **start** of the Display string (estimated <10 sites — needs explicit grep for `starts_with` / `assert_eq!` patterns before promoting).
- One `Display` impl edit, one new constructor (`with_hint_for_bug`), optional one new JSON field (`bug_ref`) in `route_tool_error`.
## Decision criteria (draft → active, or wontfix)

Promote to **`active`** (with an implementation plan) when **either**:

1. Two concrete user-reported confusions where an agent couldn't tell "is this codescout's hint or my shell's stderr?", **or**
2. One concrete instance where a bug-tracking workflow would have benefited from the `(BUG-N)` cross-reference being machine-followable from the error envelope (e.g., agent emits a hint, user wants to know "is there a tracker for this?", and the link is right there).

Mark **`wontfix`** if neither lands within a month. The JSON envelope is already structurally adequate; without datapoints, this is over-engineering.

## Pointers

- **Origin:** 2026-05-20 codescout field-notes lessons report (item: "cs-hint convention proposal" — verified not implemented in `src/`, only mentioned in `docs/lessons/...`).
- **Subject code:**
  - `src/tools/core/types.rs:172-181` — `RecoverableError` struct.
  - `src/tools/core/types.rs:128-155` — `Guidance` enum + accessors.
  - `src/tools/core/types.rs:232-246` — `Display` impl + the stability doc-comment.
  - `src/tools/mod.rs` — `route_tool_error` (JSON envelope formatting).
- **Related conventions:**
  - `docs/issues/_TEMPLATE.md` — bug-file shape and slug convention.
  - `docs/trackers/bug-fix-session-log.md` — multi-session bug-fix shipping log (potential first consumer of `(BUG-N)` cross-references).
  - `CLAUDE.md` § "Bug Tracking" — current bug-file lifecycle.
