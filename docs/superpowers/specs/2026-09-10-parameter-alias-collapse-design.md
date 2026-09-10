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

## Authority — this is an ADR amendment, not a new decision

`docs/adrs/2026-07-10-repair-and-continue-input-handling.md` (**accepted**) already decided the
runtime half: *repair the input, execute, return the result, attach an advisory correction note
— never `RecoverableError`.* Its Context cites a 72-DB / ~152k-call `usage.db` sweep and names
`file_path` for `path` and buffer handles under `output_id` as its examples. **That is this
document's scope, and this document does not get to redecide it.**

Read the **Amendment 2026-09-10** section of that ADR before this spec. It records the two
things that were genuinely open:

1. the law was half-implemented — the path family repairs and never notes, because the helpers
   return `&str` and structurally cannot report a correction; and
2. the ADR governs *acceptance* of a synonym and never addressed whether it should also be
   *advertised* as a schema property, which is the gap that produced the API-illegal `anyOf`.

Where this spec and the ADR disagree, the ADR wins. In particular the advisory field is
`corrections` and not a new name.

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

- **Rejecting aliases.** Tolerance is permanent. Callers are retrained by the `corrections`
  note, not
  by a failure. No deprecation window, no future hard error — that is a separate decision if
  it is ever wanted.
- **`symbols`' `name`/`query` and `name_path`/`symbol`.** True synonyms, but `symbols`
  **carries no top-level `required` at all** (`src/tools/symbol/symbols.rs:134-160`), so no
  false claim exists there and the change scenario that funds the path collapse is provably
  absent. Collapsing it would buy ~130 chars on a surface already measured as 100% cache-read
  and worth ~$0.0004 per request, plus tidiness. Consistency across the family is an
  aesthetic, not a change scenario. **Revisit-when:** a `usage.db` split on those four param
  names shows a retry or error asymmetry — an ambiguity cost is measurable, and nobody has
  measured it.
- **`librarian`'s `root`/`old_root`.** Unverified whether `root` names the same parameter for
  `merge_worktree` and `doctor` as it does for `rehome`. Not collapsed on an assumption.
- **`read_file`'s `offset`/`limit`.** These are a different calling convention, not a rename:
  `limit` is a *count* folded into `end_line` (`end_line = offset + limit - 1`). Collapsing
  them would mean computing rather than renaming, and a wrong computation is silent. Out of
  scope by decision.
- **`edit_code`'s `name_path`/`content`.** Already correct — advertised as one property each,
  alias accepted in `call()` and documented inline. They gain only the `corrections` note.

## Decisions already taken

| decision | value | who |
|---|---|---|
| scope | **path family only** — 23 alias properties across 8 tools. NOT `symbols`, NOT `librarian`, NOT `offset`/`limit` | operator, 2026-09-10; narrowed the same day after verifying `symbols` carries no top-level `required` (see Non-goals) |
| register | `corrections`, shaped `{keyed, hint}` as `src/librarian/tools/find.rs:1290` shapes it | **ADR 2026-07-10 amendment**. A first draft chose `warning`, derived from `Guidance`'s doc comment — wrong at the root, because `Guidance` attaches to a `RecoverableError` and this is the path where none is returned. `warning` would be a THIRD vocabulary for one concept beside `filter_warnings` and `corrections`. |
| cadence | announce on every affected call, statelessly | operator, 2026-09-10 — per-session suppression needs state on a server shared by many sessions, and a compacted or resumed context never sees the note it already consumed |
| conflict rule | canonical wins, and the ignored key is named in the `corrections` note | existing behaviour (`require_path_param` prefers `path`) plus the new announcement |

## Inventory and canonical choice

| tool | properties removed | canonical | note |
|---|---|---|---|
| `read_file` | `file_path`, `relative_path`, `file`, `output_id` | `path` | `path` already accepts `@tool_*`/`@cmd_*`/`@file_*` handles via `strip_buffer_ref_quotes`, so `output_id` is a rename, not a capability. `file_id` is already accepted with no property — the target shape. |
| `create_file` | `file_path`, `relative_path`, `file` | `path` | |
| `edit_file` | `file_path`, `relative_path`, `file` | `path` | |
| `edit_code` | `file_path`, `relative_path`, `file` | `path` | `name_path`→`symbol` and `content`→`body` are already collapsed (prose on the canonical property, not siblings); they gain the `corrections` note only |
| `references` | `file_path`, `relative_path`, `file` | `path` | |
| `symbol_at` | `file_path`, `relative_path`, `file` | `path` | |
| `call_graph` | `file_path`, `relative_path`, `file` | `path` | |
| `grep` | `file_path` | `path` | `path` is optional here; the collapse is unaffected by that |
| `symbols` | — | — | **out of scope**; see Non-goals |
| `librarian` | — | — | **out of scope**; see Non-goals |

**`symbols` was considered and dropped.** The reasoning is in Non-goals above: it carries no
top-level `required`, so there is no false claim to repair and no change scenario to fund the
change. `docs/manual/src/tools/api-redesign.md` documents `name_path` → `symbol` as an intended
rename, which is why it looked in-scope at first — an intended rename is not the same thing as
a defect, and this pass fixes a defect.
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
announce_corrections(&mut val, &corrections)        // sets `corrections`
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

### 3. The correction must survive all THREE render paths

This is the design's sharpest edge, and the first draft of this spec got it wrong by naming
two paths. There are three, and `Tool::call_content` already threads an existing notice
through every one of them.

| path | condition | what the caller receives | does a `val["corrections"]` survive? |
|---|---|---|---|
| buffered | `exceeds_inline_limit` | a fresh `{output_id, summary, hint, buffered_bytes}` envelope; `val` goes into the buffer | **no** — the caller never reads `val` |
| compact text | small + `OutputForm::Text` | `format_compact(&val)` rendered as text | **no** — `format_compact` selects the fields the tool knows about, and a framework-added key is not one of them |
| pretty JSON | small + `OutputForm::Json` | `serde_json::to_string_pretty(&val)` | yes |

The middle row is the dangerous one and is **not** an edge case: `OutputForm::Text` covers
roughly 18 tools including `read_file`, `symbols`, `grep` and `references` — the bulk of the
population this change touches.

**All three are already solved once, for `workspace_notice`**, and the second row was a
shipped bug:
`docs/issues/2026-09-02-the-worktree-notice-is-injected-then-discarded-by-every-compact-renderer.md`
— the notice was injected into the `Value` and then dropped by every compact renderer, on
"precisely the read surface it exists to caveat". `src/tools/symbol/edit_code.rs:184` is the
same lesson at tool scope: *"a warning only present in the raw JSON is a silent fix."*

So `announce_corrections` threads exactly like `workspace_notice` does, setting **`corrections`**
— never a new field name — at each of:

1. buffered envelope — inject alongside `output_id`/`summary`, via the `inject_notice` shape.
2. compact-text branch — prefix the rendered text, matching the existing
   `format!("⚠ {notice}\n\n{text}")` treatment, because a correction changes how the content
   should be read and must arrive before it.
3. pretty-JSON branch — set `val["corrections"]`.

**Do not implement this by setting `val["corrections"]` early and hoping.** That is the exact
shape of the bug cited above: it looks correct, passes a JSON-shaped test, and is silent on
the two paths most callers actually get.

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

Both are prose on the *canonical* property rather than separate properties, so nothing is
de-advertised — the runtime note replaces a sentence, and the aliases keep working. `librarian`'s
`old_root` description is **left alone**: `librarian` is out of scope.

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
- `docs/manual/src/**` uses `relative_path` across roughly ten examples. Mechanical, but a
  reader copying them would now be corrected on every call.
- **No served-guide edit is required, because `symbols` is out of scope.** Had it been in,
  `src/prompts/guides/symbol-navigation.md` teaches `symbols(name_path=…)` four times — and
  that guide is `include_str!`'d (`src/prompts/mod.rs:591`), so it ships atomically with the
  binary and carries no cache-staleness risk. The churn would have been plain work, not risk;
  it is simply unfunded.
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
- Conflict case: both canonical and alias supplied, different values — canonical wins, the
  `corrections` note
  names the ignored key.
- **One test per render path**, because they are three independent mechanisms and a JSON-shaped
  test says nothing about the other two: a small `OutputForm::Json` response (field present),
  a small `OutputForm::Text` response whose tool has a `format_compact` (prefix present in the
  rendered text), and a response large enough to buffer (correction present in the returned
  envelope, not only in the buffered payload). The middle one is the regression that already
  happened once to `workspace_notice`.
- `write_path` annotation present for `create_file(file_path=…)` — the incidental fix.
- **Mutation, on the production path, with an observed red for each** — an assertion's
  existence is not evidence. At minimum: delete one alias from `PATH_PARAM_ALIAS_MAP`; make
  `announce_corrections` a no-op; move normalization to after `write_path` capture; and
  **remove each of the three render-path insertions separately**, which must red three
  different named tests. If removing the compact-text insertion reds nothing, the suite has
  reproduced the 2026-09-02 bug.

## Rejected alternatives

- **Per-tool helper, each `call()` attaches its own `corrections` note.** `edit_code` alone has four
  success return sites and `read_file` branches through markdown, buffer and JSON paths; every
  branch would have to remember. A missed branch is silent, and the compact-form propagation
  would be re-solved eight times. This is CLAUDE.md's loudness law — an alarm nothing reaches
  is exactly as informative as no alarm.
- **Keep the properties, mark them deprecated in prose.** Removes neither the surface nor the
  ambiguity.
- **Reject aliases outright.** Breaks every caller habituated to Claude Code's native
  `Read(file_path=…)`, for no gain over correcting them.
