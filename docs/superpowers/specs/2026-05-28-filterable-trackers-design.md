# Filterable Trackers — Design Spec

- **Date:** 2026-05-28
- **Status:** draft (design approved in brainstorm; awaiting spec review)
- **Work-stream log:** `docs/trackers/metadata-filtering-session-log.md` (F-1, F-2)
- **Author:** Marius + Claude (Codescout Pika POV)

---

## 1. Problem

The librarian can filter **across artifacts** by frontmatter — `artifact(find, kind="bug", status="open")` works today via the `FilterNode` AST compiled to SQL (`src/librarian/filter.rs`). There is **no way to filter the entries _inside_ one tracker**: a roadmap with `software`/`architecture`/`hardware` items, or the `U-N`/`F-N`/`W-N` rows in a session log, can only be retrieved whole (`artifact(get, full=true)`) or sliced by `## H2` heading.

Two classes of tracker exist:

| Path | Example | Entry data today | Filterable? |
|---|---|---|---|
| **A — structured** | `failure_table` archetype (`tracker_design.rs:79`) — `params.failures: [{id,status,owner,…}]` | clean, schema-validated JSON in `params` | data is ready; **no engine reads it** |
| **B — prose** | `reflective` archetype (`tracker_design.rs:257`); session logs (`F-N`/`W-N`) | `## X-N` headings + `**Key:** value` lines in the body | **neither data nor engine** |

## 2. Goals / Non-goals

**Goals**
- Filter the per-entry rows of a **single** tracker by metadata, using the **same `{field:{op:value}}` AST syntax** as artifact-level filtering.
- Return **structured rows** (matching entry objects as JSON), progressive-disclosure governed.
- **Generic:** any tracker opts in by declaring where its entries live; existing trackers get it without a rewrite (retrofit guide).

**Non-goals (YAGNI — explicitly out of scope)**
- ❌ Cross-tracker entry search (`find(entry_filter)`). Documented as a **future graduation path** (§9), not built.
- ❌ A migration command/tool. The retrofit path is a **guide** an agent follows by hand.
- ❌ An automatic body→params extractor. Same reason.

## 3. Design overview — four pieces

| # | Piece | What it is | Cost |
|---|---|---|---|
| 1 | **`entry_collection` contract** | One nullable field on the augmentation naming the params key that holds the filterable array | one DB column + serde |
| 2 | **Entry-filter engine** | A new in-memory `eval(&FilterNode, &Map) -> bool` in `filter.rs`; new `entry_filter` param on `artifact(get)` | ~120 LoC + consistency test |
| 3 | **`tracker_design` teaching** | One new step in `SYSTEM_PROMPT` + `entry_collection` in entry-bearing archetype examples | content |
| 4 | **Retrofit guide** | Agent-readable doc: convert a `reflective`-shape tracker into a `failure_table`-shape one | one doc |

Only **piece 2** is net-new code. Pieces 1, 3, 4 reuse the augmentation/archetype machinery that already ships.

## 4. Piece 1 — the `entry_collection` contract

**Why a contract is needed (F-2):** entry-bearing archetypes already exist but name their arrays differently — `failure_table` uses `failures`, `task_list`/`goal` use `children`. A generic engine cannot guess. So the tracker declares it.

**Change:** add to `AugmentationRow` (`src/librarian/catalog/augmentation.rs:6-24`):

```rust
/// Names the params array whose objects are the tracker's filterable
/// entry rows (e.g. "failures", "children", "frictions"). None = the
/// tracker is not entry-filterable.
pub entry_collection: Option<String>,
```

- Delivered as a **nullable DB column**, mirroring exactly how `render_template` / `params_schema` / `append_mode` / `history_cap` were added (schema migration + `row_from_sql` + `upsert`).
- Exposed on `artifact_augment` alongside the existing optional fields. Per the `merge=false` overwrite semantics already documented for that tool, omitting it on a non-merge call resets it to `None` — call out in the tool description (same foot-gun as `render_template`).
- **Semantics:** points at a key in `params` whose value MUST be a JSON array of objects. Validated lazily at query time (§7), not at augment time, to avoid coupling to `params` write order.

## 5. Piece 2 — the entry-filter engine

### 5a. The evaluator

The filter **AST type** (`FilterNode`, `LeafOp`) is reused verbatim. The **engine is new**: `compile()` only emits SQL (F-1), and its `ALLOWED_FIELDS` allowlist would reject arbitrary entry fields like `category`. So we add a sibling evaluator in `src/librarian/filter.rs`:

```rust
/// Evaluate a filter AST against one entry object (in-memory).
/// No ALLOWED_FIELDS gate: matching a HashMap has no SQL-injection surface.
pub fn eval(node: &FilterNode, entry: &serde_json::Map<String, Value>) -> Result<bool>;
```

- `And`/`Or`/`Not` recurse. `Leaf` looks the field up in `entry`; a missing field is a non-match (returns `false`), never an error.
- Op semantics **must mirror `compile_leaf`** so artifact-grain and entry-grain filtering behave identically:

| Op | In-memory semantics (must match SQL) |
|---|---|
| `eq`/`ne` | JSON value equality, with numeric coercion (`2` == `2.0`) |
| `in`/`nin` | filter value is an array; membership of the entry value |
| `gt`/`lt`/`gte`/`lte` | typed compare — numbers numerically, strings lexically; mismatched types → non-match |
| `contains` | entry value is a string → substring; entry value is an array → membership (mirrors SQL's scalar-`LIKE` vs `tags`/`owners` `json_each` split) |
| `prefix` | string starts-with |

- Shares `LeafOp::from_str` (so unknown-op errors are identical) and the value-coercion helper with `compile`.

### 5b. The surface

`artifact(get)` (`src/librarian/tools/get.rs::call`, 93-363) gains one field:

```rust
entry_filter: Option<FilterNode>,
```

Flow when `entry_filter` is set (the handler **already loads `aug`**):
1. Require the artifact be augmented with a non-null `entry_collection` (else RecoverableError, §7).
2. Parse `aug.params` (stored as raw JSON text), index into `entry_collection`.
3. `eval()` each object; collect matches.
4. Return `{ "entries": [ …matching objects… ], "entry_total": N }`. Subject to the standard `@tool_*` overflow buffer (progressive disclosure).

`entry_filter` is **orthogonal** to the body selectors (`full`/`heading`/`headings`/`start_line`) — it returns entry rows, not body — so it is *not* added to the "at most one body selector" mutual-exclusion guard; it may accompany a metadata-only get.

## 6. Piece 3 — `tracker_design` teaching

`tracker_design` is live content (a `const SYSTEM_PROMPT` + `archetypes()` JSON returned at call time — **no `ONBOARDING_VERSION` bump**, it is not a cached prompt surface).

- Add a short step to `SYSTEM_PROMPT` (between the render_template and body-skeleton steps): *"If a tracker's entries should be queryable, set `entry_collection` to the params key holding them — this makes `artifact(get, entry_filter=…)` work."*
- Add `"entry_collection": "failures"` to `archetype_failure_table`'s example (and `"children"` to `task_list`/`goal`) so the contract is shown, not just told.

## 7. Error handling

All input-driven failures use `RecoverableError` (`isError:false`, sibling calls survive), consistent with `compile_leaf`'s existing `with_hint` pattern:

| Condition | Error |
|---|---|
| `entry_filter` set, artifact not augmented or `entry_collection` is `None` | "tracker is not entry-filterable — declare `entry_collection` on its augmentation, or retrofit it (see `docs/conventions/retrofitting-trackers-for-filtering.md`)" |
| `entry_collection` names a missing `params` key, or the value is not an array | "`entry_collection` points at `<key>` but params has no array there" |
| Unknown op / malformed leaf | identical to `compile`'s errors (shared `LeafOp::from_str`) |

## 8. Testing

- **Dual-engine consistency test (the key regression guard against F-1 drift):** build a fixture set of entry objects; load them BOTH into a temp SQLite table (one column per field) and keep them as JSON; run a battery of `FilterNode`s through `compile()`→SQL and through `eval()`; assert the matching id-sets are identical. Where SQL/JSON type semantics cannot align, the table in §5a is canonical and `eval` is tested against expected directly.
- **Per-op unit tests** for `eval` (each row of the §5a table, plus missing-field → non-match).
- **`get` integration tests:** augmented tracker with `entry_collection` + `entry_filter` returns the expected rows; non-augmented tracker → RecoverableError; `entry_collection` naming a missing/non-array key → RecoverableError; `entry_filter` alongside a body selector returns both body and entries.
- No cache-invalidation sandwich needed — the engine reads `params` fresh each call (no entry-level cache).

## 9. Future graduation path

If cross-tracker entry search is ever wanted (`artifact(find, entry_filter=…)` across all trackers), the engine promotes to the **SQL-table route**: a catalog `entries(artifact_id, entry_id, key, value)` table populated at reindex by reading each augmented tracker's `entry_collection`. `eval`'s op semantics (§5a) become the spec for that table's SQL. This is deliberately deferred — single-tracker `eval` is the same logic at smaller scale, so nothing is wasted.

## 10. Piece 4 — the retrofit guide

A new agent-readable doc at `docs/conventions/retrofitting-trackers-for-filtering.md`, referenced from `tracker_design`'s `SYSTEM_PROMPT` and `get_guide("tracker-conventions")`. Procedure:

1. **Identify entries.** Find the repeating `## X-N` sections (e.g. `## F-7 — …`).
2. **Derive the schema.** The `**Key:** value` lines are the fields. Map to a JSON-Schema (`status` → enum, dates → string, etc.) — `failure_table`'s `params_schema_example` is the template.
3. **Write the params array.** Populate `params.<collection>[] = [{id, …fields…}]` from the existing sections.
4. **Write the `render_template`** so the rendered body reproduces the existing `## X-N` prose (so humans see no change).
5. **Declare `entry_collection`** = the array key.
6. **Verify** the rendered body matches the original section-for-section before committing.

## 11. Surfaces to update (project discipline)

Per `CLAUDE.md` "Prompt Surface Consistency" — new param + new augment field require coordinated updates:
- `artifact` tool description (`get` action — document `entry_filter`).
- `artifact_augment` tool description (document `entry_collection` + its `merge=false` reset behavior).
- `get_guide("librarian")` filter section (note the entry-grain twin) and `get_guide("tracker-conventions")` (link the retrofit guide).
- `server_instructions` surface if the entry-filter is worth a one-liner. **No `ONBOARDING_VERSION` bump** (no cached-prompt surface changes).
- Run `prompt_surfaces_reference_only_real_tools` after edits.
