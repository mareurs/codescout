# Server Instructions Consolidation — Design

**Date:** 2026-05-07
**Author:** the Hamsa, on behalf of the user
**Related:** `docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md` (lands first)

## Problem

Two prompt surfaces in `src/prompts/` carry overlapping content for symbol
navigation, with diverging lifecycles:

- **`server_instructions.md`** — read fresh from disk at every `/mcp` connect.
  Contains a static `### Symbol Navigation Patterns` block listing per-language
  `name_path` syntax and `kind` filter quirks.
- **Per-project `system-prompt.md`** — generated once by onboarding, cached on
  disk under `.codescout/projects/<id>/system-prompt.md`, hand-curated thereafter.
  `build_system_prompt_draft()` in `src/prompts/builders.rs` calls
  `language_navigation_hints(lang)` and emits a `## Language Navigation` section
  for up to 3 detected languages.

A trace audit across ~85 codescout-onboarded projects (both `~/.claude` and
`~/.claude-sdd` profiles, ~30-90 days of sessions) found:

| Tier | Result |
|---|---|
| Tier 1 — model misuses navigation due to duplication | None observed. |
| Tier 2a — stale tool names in cached prompts | 19/36 cached prompts reference deprecated `find_symbol`; 15/36 reference `list_symbols`. |
| Tier 2b — generator output never lands | **0/36** cached prompts contain `## Language Navigation` or any of the canned example symbols emitted by `language_navigation_hints`. |
| Tier 3 — token dilution | Bounded; moot, since the section is never on disk. |

The duplication is not a two-source race. `server_instructions.md` is the only
surface delivering live language-navigation hints to the model.
`build_system_prompt_draft`'s language-navigation emission is dead on arrival —
human curation either strips or never preserves it.

A separate audit of `call_graph` mentions in `server_instructions.md` found six
references, none with full demonstrative arguments. The model is not skipping
`call_graph` from forgetfulness — the prompt currently demotes it to "step 2b"
of impact analysis and offers no worked example.

## Decisions

| Decision | Resolution |
|---|---|
| Single source for language-navigation hints | `server_instructions.md`. Per-project `system-prompt.md` carries no language-syntax content. |
| `system-prompt.md` role | One-shot scaffold for project-specific signal (entry points, key abstractions, project-specific search tips). Hand-curated post-onboarding by design. |
| Language scope at session start | Workspace aggregate. Sum each language's occurrences across all `DiscoveredProject.languages` entries (every project contributes weight 1.0 per language it lists), pick top 2 supported. |
| Example symbol naming | Fixed generic cast — `Service`, `Repository`, `Order`, `Account`, `find`, `handle`, `process`, `create`. Drift impossible. |
| `call_graph` placement | Promote to canonical `### Impact Analysis` workflow with full demonstrative arguments. Promote Iron Law 8 to name `call_graph` first, `references` second. Prune the five scattered one-liners. |
| ONBOARDING_VERSION | No bump. `server_instructions.md` is read fresh; cached prompts unchanged. |
| Stale tool names in 19/36 cached prompts | Out of scope — separate plan (Q2 sweep). |

## File Layout

**New:**

- `src/prompts/language_nav.rs` — pure module: `NavBlock` struct,
  `nav_block(lang)`, `generic_nav_block()`, `supported_languages()`,
  `rank_workspace_languages(projects, max)`,
  `render_symbol_navigation_block(projects)`. ~250 lines, fully unit-testable.

**Modified:**

- `src/prompts/server_instructions.md` — `### Symbol Navigation Patterns`
  body replaced by `{{symbol_navigation_block}}` substitution token.
  `### Impact Analysis` rewritten with full `call_graph` demonstration.
  Iron Law 8 rewritten to promote `call_graph`. Five scattered `call_graph`
  one-liners pruned (L110, L139, L150 trimmed, L287).
- `src/prompts/builders.rs` — delete `language_navigation_hints()` (lines 6-51).
  Delete the `## Language Navigation` emission block (~lines 271-282) from
  `build_system_prompt_draft`.
- `src/prompts/mod.rs` — extend `load_prompt()` helper to accept a
  `HashMap<&str, String>` of substitutions (or equivalent). Add
  `symbol_navigation_block` substitution.
- `src/server.rs` — wire the loader call to pass workspace projects into
  `render_symbol_navigation_block`.
- `src/tools/run_command/tests.rs` — delete tests on
  `language_navigation_hints` (lines 2126, 2136-2138). Migrate negation cases
  to `language_nav` module tests.

## `language_nav` Module

### Public surface (crate-private)

```rust
pub(crate) struct NavBlock {
    pub language: &'static str,
    pub display_name: &'static str,
    pub markdown: &'static str,
}

pub(crate) fn nav_block(lang: &str) -> Option<&'static NavBlock>;
pub(crate) fn generic_nav_block() -> &'static str;
pub(crate) fn supported_languages() -> &'static [&'static str];

pub(crate) fn rank_workspace_languages(
    projects: &[DiscoveredProject],
    max: usize,
) -> Vec<&'static str>;

pub(crate) fn render_symbol_navigation_block(
    projects: &[DiscoveredProject],
) -> String;
```

### Supported languages (day one)

`rust`, `python`, `typescript` (covers `tsx`/`jsx`/`javascript`),
`kotlin` (covers `java`), `go`, `csharp`.

### NavBlock schema (every language)

Five bullets, identical structure across languages:

1. `name_path` form
2. Find a method (full `symbols(...)` call with example)
3. List by kind (full `symbols(...)` call with idiomatic dir + canonical kind)
4. Language-specific note (one line — trait impls in Rust, decorators in Python, etc.)
5. Before refactor (full `call_graph(...)` call with example)

### Generic block (always appended)

```
### Generic Patterns (any language)

- `name_path` syntax: `Container/member` for methods on classes/structs/objects;
  bare name for top-level functions or types.
- `kind` filter values vary by language: `function`, `class`, `struct`, `interface`,
  `type`, `enum`, `module`, `constant`. Run `symbols(path)` once on a representative
  file to see what kinds your LSP emits.
- For impact analysis before any structural change:
  `call_graph(symbol, path, direction="callers")` traces blast radius;
  `direction="callees"` traces outbound flow.
- When the symbol's exact name is unknown, start with
  `semantic_search("what it does")` then drill down with `symbols(name_path=...)`.
```

### Ranker contract

- Aggregates `DiscoveredProject.languages` across the workspace.
- Each project contributes weight 1.0 per language it lists. (The languages
  Vec has no documented primary/secondary ordering — uniform weighting is the
  honest model.)
- Filters to languages where `nav_block()` returns `Some` — unsupported languages
  do not displace supported ones.
- Sorted descending by weight; ties broken alphabetically for determinism.
- Returns at most `max` entries.

### Render contract

```
### Symbol Navigation Patterns

[lead-in: language-agnostic bullets and language-kind quirks table — verbatim
 from the existing server_instructions.md content, preserved as a static lead-in]

[NavBlock #1 markdown]

[NavBlock #2 markdown]   ← omitted if workspace has only 1 supported language

### Generic Patterns (any language)

[generic block]
```

Edge cases:

- Empty workspace → lead-in + generic block, no per-language sections.
- Single supported language → lead-in + 1 NavBlock + generic block.
- Workspace with only unsupported languages → same as empty.

## `server_instructions.md` Changes

### `### Symbol Navigation Patterns` (replaced)

Body replaced by `{{symbol_navigation_block}}`. The lead-in content
(`Hierarchical nav` / `Kind filter + path scope` / `Find across project then
read body` bullets and the language-kind quirks table) is moved into the
`render_symbol_navigation_block` output as the static lead-in, so semantics are
preserved.

### Iron Law 8 (rewritten)

```
8. **CALL GRAPH BEFORE STRUCTURAL EDITS.** Before
   `edit_code(action="rename"|"replace")` of a function, method, or
   public type: `call_graph(symbol, path, direction="callers",
   max_depth=3)` first, then `references` for edit targets. Transitive
   callers are invisible to `references` alone.
```

### `### Impact Analysis — "What breaks if I change X?"` (rewritten)

```
### Impact Analysis — "What breaks if I change X?"

`references` = direct call sites. `call_graph` = transitive reach.
Both required for any rename / signature change / contract change.

1. `symbols(name="Service/handle", include_body=true)` — read it.
2. `call_graph(symbol="Service/handle", path="src/service.rs",
   direction="callers", max_depth=3)` — blast radius.
   Tree depth ≈ change risk: shallow = local; deep+branching = contract.
3. `references(symbol, path)` — file:line edit targets.
4. `symbol_at(path, line, fields=["hover"])` on non-obvious callers
   from step 2 — reveal concrete types behind generics/traits.
5. `edit_code(...)`.

`direction`: `callers` (refactors) | `callees` (flow) | `both` (hubs, rare).
`max_depth`: `1` ≈ references; `3` default; `5` only for deep reach.
Skip call_graph only for body-only edits with identical signature.
```

### Pruned `call_graph` mentions

- **L69 anti-pattern table fix cell** — shorten to `call_graph(...)` — see
  Impact Analysis. Cross-reference, not duplicate.
- **L110 Symbol Nav one-liner** — delete. Symbol Nav is for finding, not
  analyzing impact.
- **L139 LSP Workflow step 4** — replace with: "For impact analysis, see
  Impact Analysis."
- **L150 Search Routing** — keep one sentence answering "how do I trace
  transitive call graphs?"; cross-reference Impact Analysis for the worked
  example.
- **L287 Safe Rename step 2b** — delete the row. Safe Rename's first line
  becomes "Run Impact Analysis first."

## Token Accounting

| Change | Lines Δ | Tokens Δ |
|---|---|---|
| `### Symbol Navigation Patterns` static block → templated (workspace top-2 + generic) | +5 worst-case | +75 |
| `language_navigation_hints` and `## Language Navigation` block deleted from `system-prompt.md` generator | (offline) | (offline) |
| `### Impact Analysis` 7 → 14 lines | +7 | +110 |
| Iron Law 8 rewrite | +1 | +20 |
| Five `call_graph` one-liner prunes | −3 | −60 |
| **Net delta on `server_instructions.md`** | **+10** | **+145** |

~145 tokens added to the per-session injected prompt. ~0.1% of a 128k window.
Compactness preserved.

## Tests

### `language_nav` module unit tests

1. `nav_block_returns_some_for_supported_languages` — every language in
   `supported_languages()` resolves to `Some`.
2. `nav_block_returns_none_for_unsupported` — `bash`, `markdown`, `unknown` →
   `None`. Migrated from existing `language_navigation_hints` tests.
3. `every_nav_block_has_required_bullets` — parse each block, assert presence
   of the five canonical bullets.
4. `every_nav_block_uses_only_generic_example_names` — every example symbol
   comes from the fixed cast (`Service`, `Repository`, `Order`, `Account`,
   `find`, `handle`, `process`, `create`, `core`, `worker`). Drift guard.
5. `rank_workspace_languages_picks_top_2_by_weight` — synthetic projects
   rust×3, python×2, kotlin×1 → returns `["rust", "python"]`.
6. `rank_workspace_languages_filters_unsupported` — projects with `bash` and
   `rust` → returns `["rust"]`.
7. `rank_workspace_languages_deterministic_on_ties` — equal weights →
   alphabetical.
8. `rank_workspace_languages_caps_at_max` — `max=2` against 5 supported langs
   returns exactly 2.
9. `rank_workspace_languages_handles_empty_projects` — returns `vec![]`.
10. `render_symbol_navigation_block_with_no_languages` — lead-in + generic,
    no per-language sections, no panic.
11. `render_symbol_navigation_block_with_one_language` — single per-lang block.
12. `render_symbol_navigation_block_with_many_languages` — exactly 2 per-lang
    blocks.

### Loader / template substitution tests

13. `server_instructions_template_has_substitution_token` — raw template
    contains `{{symbol_navigation_block}}` exactly once.
14. `loaded_server_instructions_substitutes_token` — `load_prompt` against a
    fixture workspace produces a string with no `{{` residue and the expected
    language headings.
15. `loaded_server_instructions_no_residual_template_tokens` — no `{{` or
    `}}` sequences remain.

### Cross-prompt consistency

16. `prompt_surfaces_reference_only_real_tools` (existing) — updated to render
    the template with an empty project list before scanning, so it still
    validates rendered tool names.
17. `rendered_server_instructions_contains_no_deprecated_tool_names` — render
    against a 5-language synthetic workspace, scan for `find_symbol`,
    `list_symbols`, `replace_symbol`, `insert_code`, `rename_symbol`,
    `search_pattern`. None permitted.
18. `impact_analysis_section_contains_call_graph_with_full_arguments` — parse
    the section, assert `call_graph(` appears with `direction=` and
    `max_depth=` arguments. Drift guard against re-shrinking.
19. `iron_law_8_promotes_call_graph` — assert Iron Law 8 names `call_graph`
    before `references`. Guard against re-demotion.

### Removed tests

- `language_navigation_hints` tests in `src/tools/run_command/tests.rs` (lines
  2126, 2136-2138). Negation cases migrate to test #2.

## Migration & Rollout

1. `cargo fmt && cargo clippy -- -D warnings && cargo test`
2. `cargo build --release`
3. Restart MCP via `/mcp` against this repo (Rust + Python fixture +
   TypeScript fixture). Inspect rendered server instructions: confirm exactly
   2 language blocks, generic block present, no `{{` residue.
4. Activate `rust-library` fixture, reconnect, confirm 1 language block.

No backwards-compatibility shims. `language_navigation_hints` is `pub(crate)` —
direct deletion. No re-export, no comment grave-marker.

## ONBOARDING_VERSION

**Not bumped.** All changes target `server_instructions.md`, which is read
fresh at every `/mcp` connect. Cached `system-prompt.md` files lose the
`## Language Navigation` section *prospectively* (the generator no longer
emits it), but on-disk caches are untouched — and the audit found 0/36 ever
contained the section anyway.

## Risk Register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Workspace project list empty at server start | Low | `render_symbol_navigation_block` returns lead-in + generic. Test 10. |
| `DiscoveredProject.languages` empty for a project | Low | Filtered by ranker. No panic, no spurious entries. |
| Workspace mixes 5+ languages including esoteric ones | Medium | Unsupported filtered. User sees fewer hints, not broken hints. |
| Future tool deprecation leaks into a NavBlock | Low | Test 17 catches at build time. |
| Adding a second template token couples loader unhelpfully | Low | `load_prompt` accepts `HashMap<&str, String>` — extensible, not bespoke. |

## Out of Scope (Q2)

One-shot migration sweep for stale tool names in 19/36 existing cached
`system-prompt.md` files. Rewrites `find_symbol` → `symbols`, `list_symbols`
→ `symbols`, deprecated edit tools → `edit_code`, etc. Tracked separately —
the trace dive proved the staleness exists; the fix is mechanical and
independent of this refactor.

Plan path: `docs/superpowers/plans/2026-05-07-cached-prompt-migration-sweep.md`
(to be drafted after this design lands).

## References

- Trace audit (this session, the Hamsa, 2026-05-07) — established Tier 2b
  finding (0/36 cached prompts contain `## Language Navigation`).
- `docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md` — must land
  first, since this design depends on the `load_prompt` helper introduced
  there.
- `src/prompts/server_instructions.md` — primary surface modified.
- `src/prompts/builders.rs` — surface trimmed.
- `CLAUDE.md` § "Which surface needs a bump?" — explicit policy that
  `server_instructions.md` changes do not bump `ONBOARDING_VERSION`.
