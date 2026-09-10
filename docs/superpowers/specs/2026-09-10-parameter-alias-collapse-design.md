---
kind: spec
status: draft
title: Parameter alias collapse — advertise one name, correct the rest at runtime
owners:
- marius
tags:
- tool-surface
- schema
- progressive-disclosure
topic: tool parameter surface
---

# Parameter alias collapse — advertise one name, correct the rest at runtime

## Problem

Eight tools advertise the same file path under up to five names — `path`, `file_path`,
`relative_path`, `file`, `output_id` — as sibling schema properties whose descriptions read
`Alias for path`. That is 23 advertised properties for one concept. `symbols` does the same
thing for two more concepts (`name`/`query`, `name_path`/`symbol`), and `librarian` for one
(`old_root`/`root`).

Three costs, in increasing order of importance:

1. **Surface.** ~26 properties of pure duplication on a tool surface that is under a byte
   ratchet (`TOOL_SURFACE_CHAR_BUDGET`, `src/server.rs`).
2. **Ambiguity.** A caller reading the schema cannot tell which name is preferred, because
   the schema does not say. Four equal-looking options invite a coin flip per call.
3. **It produced an unshippable schema.** Advertising aliases as co-equal properties makes
   `required: ["path"]` a false claim, which is correct to notice — and the only JSON Schema
   construct that states the true requirement (`anyOf` over the alternatives) is rejected by
   the Anthropic Messages API at the top level, so the client drops the whole tool. That
   defect shipped and is recorded in
   `docs/issues/archive/2026-09-10-seven-tools-carried-an-api-illegal-top-level-anyof-and-were-dropped-client-side.md`.
   Removing the `anyOf` (commit `2735df73`) stopped the bleeding by making the schema state
   *nothing* about which name is required. **This spec removes the reason the question ever
   arose**: with one advertised name, `required` can name it honestly.

## Goals

- Exactly one advertised name per concept, on every tool in scope.
- Every non-canonical name a caller sends is still **accepted** — silently correct behaviour,
  never a failure — and the caller is **told** it was corrected.
- The correction reaches the caller in every response shape, including the compact summary
  used when a response overflows to a buffer.
- No tool can accept an alias and forget to announce it. That property must be structural,
  not a convention each tool re-implements.

## Non-goals

- **Rejecting aliases.** Tolerance is permanent. Callers are retrained by the warning, not
  by a failure. No deprecation window, no future hard error — that is a separate decision if
  it is ever wanted.
- **`read_file`'s `offset`/`limit`.** These are a different calling convention, not a rename:
  `limit` is a *count* folded into `end_line` (`end_line = offset + limit - 1`). Collapsing
  them would mean computing rather than renaming, and a wrong computation is silent. Out of
  scope by decision.
- **`edit_code`'s `name_path`/`content`.** Already correct — advertised as one property each,
  alias accepted in `call()` and documented inline. They gain only the warning.

## Decisions already taken

| decision | value | who |
|---|---|---|
| scope | path family + true duplicates (`symbols`, `librarian`); not `offset`/`limit` | operator, 2026-09-10 |
| register | `warning`, not `hint` | derived from `Guidance`'s own doc comments (`src/tools/core/types.rs`): `Warning` is "off-golden-path — reconsider before proceeding", which is what a non-canonical name is |
| cadence | announce on every affected call, statelessly | operator, 2026-09-10 — per-session suppression needs state on a server shared by many sessions, and a compacted or resumed context never sees the warning it already consumed |
| conflict rule | canonical wins, and the ignored key is named in the warning | existing behaviour (`require_path_param` prefers `path`) plus the new announcement |

## Inventory and canonical choice

| tool | properties removed | canonical | note |
|---|---|---|---|
| `read_file` | `file_path`, `relative_path`, `file`, `output_id` | `path` | `path` already accepts `@tool_*`/`@cmd_*`/`@file_*` handles via `strip_buffer_ref_quotes`, so `output_id` is a rename, not a capability. `file_id` is already accepted with no property — the target shape. |
| `create_file` | `file_path`, `relative_path`, `file` | `path` | |
| `edit_file` | `file_path`, `relative_path`, `file` | `path` | |
| `edit_code` | `file_path`, `relative_path`, `file` | `path` | `name_path`→`symbol` and `content`→`body` already collapsed; add warnings only |
| `references` | `file_path`, `relative_path`, `file` | `path` | |
| `symbol_at` | `file_path`, `relative_path`, `file` | `path` | |
| `call_graph` | `file_path`, `relative_path`, `file` | `path` | |
| `grep` | `file_path` | `path` | `path` is optional here; the collapse is unaffected by that |
| `symbols` | `name`, `name_path` | `query`, `symbol` | see below |
| `librarian` | `root` | `old_root` | **inverted**: the schema itself says `old_root` is the "preferred alias… the one the doctor hints and error text surface", so the alias is canonical and `root` is the back-compat name |

**`symbols` canonical choice.** Keep `query` and `symbol`, drop `name` and `name_path`.
Two reasons: `symbol` is already the family-wide name for this concept on `references`,
`call_graph` and `edit_code`, so keeping it is what makes the family consistent; and
`docs/manual/src/tools/api-redesign.md` already documents `name_path` → `symbol` as the
intended rename, so this completes a migration rather than starting one.

The cost is real and must land in the same commit — see *Coupled surfaces*.

## Design

### 1. Tools declare their aliases

A new defaulted method on `Tool` (`src/tools/core/types.rs`):

```rust
/// Non-canonical parameter names this tool accepts, as (received, canonical).
///
/// Advertise ONLY the canonical name in `input_schema`. The dispatch boundary
/// rewrites these before `call()` runs and tells the caller it did.
fn param_aliases(&self) -> &'static [(&'static str, &'static str)] {
    &[]
}
```

Path-taking tools return a shared `PATH_PARAM_ALIAS_MAP` derived from the existing
`PATH_PARAM_ALIASES` (`src/fs/mod.rs`) so the accept-set stays declared once.

### 2. The boundary normalizes, then announces

Both halves go in `Tool::call_content` (`src/tools/core/types.rs:979`), which is already the
place where responses are post-processed (path stripping, root annotation, guide-hint
injection, overflow buffering).

```
normalize_params(&mut input, self.param_aliases())  ->  Vec<Correction>
    ... existing selector / is_write / write_path capture ...
    ... self.call(input, ctx) ...
    ... existing strip / annotate ...
announce_corrections(&mut val, &corrections)        // sets `warning`
    ... and appends to the compact summary (see 3)
```

Four constraints this ordering satisfies, each discovered in the existing code:

- **Never clone `input`.** `call_content` warns three times that `create_file`/`edit_file`
  carry whole file bodies in `input`. `normalize_params` mutates in place and returns only a
  small `Vec` of `(from, to)` pairs.
- **Normalization must precede `selector_key`, `is_write` and `write_path`.** All three read
  `input` before `call()`. `write_path` reads `input.get("path")` specifically, so today a
  `create_file(file_path=…)` call yields `write_path = None` and loses its write-path
  annotation. Normalizing first fixes that as a side effect; a test must pin it, because it
  is a behaviour change nobody asked for and it should not be silent.
- **`Onboarding` overrides `call_content`** (`src/tools/onboarding.rs:445`) and is the only
  tool that does. An override bypasses the normalizer entirely. It declares no aliases today;
  a gate must assert that any `call_content` override declares none, so a future override
  cannot silently opt out of the mechanism.
- **`grep`'s `path` is optional.** Normalization is independent of requiredness — it renames
  a key if present and does nothing otherwise.

### 3. The correction must survive compaction

This is the design's sharpest edge. When a response exceeds the inline limit, the caller does
**not** receive the JSON — it receives `format_compact(&val)` plus a buffer handle. A
`warning` key added to `val` is then present only in the buffered payload the caller has not
read.

The codebase already hit this and solved it once by hand: `src/tools/symbol/edit_code.rs:184`
appends `result["warning"]` onto its compact base, with the comment *"a warning only present
in the raw JSON is a silent fix."*

So `announce_corrections` has **two** insertion points at the one boundary:

1. `val["warning"]` — the JSON field, for inline responses and for the buffered payload.
2. the compact summary string, appended after `format_compact`, for overflowing responses.

This mirrors the existing `inject_notice`/`_workspace_notice` pattern, which for the same
reason also prepends into `run_command`'s `stdout` — the channel that is actually read.

Message shape, one line, naming the tool so it is actionable out of context:

```
'file_path' is not a parameter of read_file — corrected to 'path'. Use 'path' next time.
```

On conflict, name the loser explicitly:

```
'file_path' is not a parameter of read_file — 'path' was also supplied and won;
the 'file_path' value was ignored.
```

### 4. The per-call helpers keep their fallbacks, and that is deliberate

Once the boundary normalizes, `call()` always sees the canonical key, so the alias fallbacks
inside `require_path_param` / `get_path_param` (`src/fs/mod.rs`) and
`require_str_param_or_hint` (`src/tools/core/params.rs`) become redundant. **Keep them.**

The reason is a test-validity hazard, not defensiveness for its own sake: a large number of
existing unit tests invoke `tool.call(json!({…}), ctx)` **directly**, bypassing
`call_content` and therefore the normalizer. Those tests pass aliases today and would break
en masse if the fallbacks went. Keeping them costs nothing and makes the two layers
independent.

The corollary is load-bearing for the gates: **a test that calls `call()` directly proves
nothing about normalization or announcement.** Gate 3 below must drive `call_content`.

### 5. Inline alias mentions come out too

Three descriptions document a tolerated name in prose rather than as a property, and they are
now redundant — the runtime announces the correction, so advertising it costs bytes for
nothing:

- `edit_code`'s `symbol`: *"Alias: `name_path` … is accepted."*
- `edit_code`'s `body`: *"Alias: `content` … is accepted."*
- `librarian`'s `old_root`: *"Preferred alias of root…"* — must be rewritten anyway, since
  `root` stops being advertised and `old_root` becomes simply the parameter name.

`read_file`'s `offset`/`limit` descriptions keep the phrase "Native-Read-style alias" because
those params are out of scope and genuinely remain a second convention. Gate 1's patterns
(`"Alias for "`, `"(alias of "`) do not match that phrasing, so it will not red — verify that
rather than assume it.

## Gate replacement — the old gates go vacuous

Four registry-wide schema gates in `src/server.rs` find offenders by parsing `"Alias for "`
property descriptions. **Removing the properties leaves them nothing to scan, so they pass by
finding nothing** — CLAUDE.md § *Testing Discipline*'s first law: an absence assertion is
monotone under removal. `EXPECTED_ALIAS_COUNTS_BY_TOOL` becomes all-zeros and stops
discriminating. They must be replaced, not edited.

The replacement invariant is stronger and checkable, and it runs in the opposite direction —
the old gates said *"if you accept an alias, declare it"*; the new ones say *"if you accept an
alias, do not declare it, and do announce it"*:

1. `no_schema_property_declares_itself_an_alias` — no property description matches
   `"Alias for "` or `"(alias of "`. Reds if a collapsed alias is reintroduced as a property.
2. `every_declared_alias_is_absent_from_the_schema` — for every tool, `param_aliases()` keys
   are disjoint from `input_schema.properties`. This is the honesty half, inverted.
3. `every_declared_alias_is_normalized_and_announced` — per **alias**, not per tool: drive a
   real call through `call_content` with the alias set, assert `call()` saw the canonical key
   and the response carries the correction. Per-member, because CLAUDE.md's population law
   says an aggregate cannot verify a per-member claim.
4. `call_content_overriders_declare_no_aliases` — the `Onboarding` hole named above.
5. Keep `no_tool_schema_declares_a_top_level_combinator` unchanged — it is orthogonal and
   still the thing that stops the API-illegal shape returning.

## Coupled surfaces — all in the same commit

Renaming what we advertise makes our own documentation teach non-canonical names, which would
then warn on every call it recommends.

- `src/prompts/README.md:33` states *"aliases (`file_path`, `limit`) are discoverable from the
  tool schema"* as the reason not to document them. **That becomes false** — after this change
  an alias is discoverable nowhere and is announced only when used. The rule must be rewritten,
  not deleted: the new reason not to document aliases is that there is exactly one name.
- `src/prompts/guides/symbol-navigation.md` teaches `symbols(name_path=…)` in four places and
  is **served into sessions**. Must become `symbols(symbol=…)`.
- `docs/manual/src/**` uses `relative_path` and `references(name_path, path)` across roughly
  ten examples. Mechanical, but a reader copying them would now be warned.
- The four prompt surfaces are gated for stale *tool names* only, not stale *parameter*
  names, so nothing catches this class automatically. That gap is why this section exists and
  is itself worth a follow-up gate.

## Budget

The collapse removes ~26 properties at roughly 65 serialized chars each, so order 1.5–2k off
the current 56_485. The figure is **not** predicted here: measure with
`cargo test --lib tool_surface_report_lengths -- --nocapture` and ratchet
`TOOL_SURFACE_CHAR_BUDGET` to the exact measured total, per that constant's own rule that
headroom is removed rather than banked.

## Testing

- Per-alias round trip through `call_content` for all ~26 aliases (gate 3 above).
- Conflict case: both canonical and alias supplied, different values — canonical wins, warning
  names the ignored key.
- Overflow case: a response large enough to buffer, asserting the correction appears in the
  **compact summary**, not only in the buffered JSON. This is the one that would otherwise
  regress silently.
- `write_path` annotation present for `create_file(file_path=…)` — the incidental fix.
- **Mutation, on the production path, with an observed red for each** — an assertion's
  existence is not evidence. At minimum: delete one alias from `PATH_PARAM_ALIAS_MAP`; make
  `announce_corrections` a no-op; move normalization to after `write_path` capture; drop the
  compact-summary insertion point while keeping the JSON one. Each must red a *different*
  named test, and the last is the one the existing suite would most plausibly miss.

## Rejected alternatives

- **Per-tool helper, each `call()` attaches its own warning.** `edit_code` alone has four
  success return sites and `read_file` branches through markdown, buffer and JSON paths; every
  branch would have to remember. A missed branch is silent, and the compact-form propagation
  would be re-solved eight times. This is CLAUDE.md's loudness law — an alarm nothing reaches
  is exactly as informative as no alarm.
- **Keep the properties, mark them deprecated in prose.** Removes neither the surface nor the
  ambiguity.
- **Reject aliases outright.** Breaks every caller habituated to Claude Code's native
  `Read(file_path=…)`, for no gain over correcting them.
