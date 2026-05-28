# Retrofitting a Tracker for Filtering

Converts a **prose tracker** (entries as `## X-N` sections + `**Key:** value`
lines — the `reflective` archetype shape) into a **filterable tracker**
(entries as a structured array in augmentation `params` — the `failure_table`
shape), so `artifact(get, id, entry_filter={…})` can query its rows.

This is an agent-readable procedure, not a tool. There is no `retrofit`
command — you follow these steps by hand using the existing
`artifact`/`artifact_augment` tools.

## When to retrofit

- The tracker has repeating numbered entries (`F-N`, `U-N`, `W-N`, roadmap items).
- You want to query them by metadata (status, category, severity, …) via
  `artifact(get, entry_filter=…)`.
- It is currently prose-only — no `entry_collection` declared on its augmentation.

If a tracker is already structured (entries live in a params array, e.g. the
`failure_table` archetype's `failures`), you only need step 5 — just declare
the `entry_collection` pointer.

## Procedure

1. **Read the tracker.** `artifact(action="get", id="<id>", full=true)`.
   Identify the repeating `## X-N — …` sections and the `**Key:** value` lines
   under each.
2. **Derive the schema.** Each `**Key:**` becomes a field. Pin types: enums for
   `status` / `severity`, strings for dates/titles, integers for counts. Use the
   `failure_table` archetype's `params_schema_example` (from
   `librarian(action="tracker_design")`) as the template.
3. **Build the array.** Populate `params.<collection> = [{id, …fields…}]` — one
   object per `## X-N` section. Keep `id` on every object so a filtered row is
   traceable back to its section.
4. **Write a `render_template`** (MiniJinja) that reproduces the existing
   `## X-N` prose from the params array, so a human reading the rendered body
   sees no change. (See the `failure_table` archetype's `render_template_example`.)
5. **Declare the pointer.**
   ```
   artifact_augment(
       id="<id>",
       merge=false,
       prompt="<existing prompt>",
       params={ "<collection>": [ …objects… ] },
       params_schema={ … },
       render_template="…",
       entry_collection="<collection>",
   )
   ```
   `merge=false` overwrites ALL caller-controlled fields, so pass `prompt` /
   `render_template` / `params_schema` back in the same call — omitting one
   resets it to `None`.
6. **Verify.** `artifact(action="get", id="<id>", full=true)` — the rendered body
   must match the original section-for-section. Then test a filter:
   ```
   artifact(action="get", id="<id>", entry_filter={"status": {"eq": "open"}})
   ```
   The returned `entries` are the matching rows; `entry_total` is the count of
   object entries considered.

## Filter syntax

`entry_filter` uses the same AST as `artifact(find)`'s `filter`:

- Leaf: `{field: {op: value}}` — ops `eq`, `ne`, `in`, `nin`, `gt`, `lt`,
  `gte`, `lte`, `contains`, `prefix`.
- Composite: `{and: [...]}`, `{or: [...]}`, `{not: {...}}`.

Note the entry-grain engine matches `contains` case-insensitively (ASCII), the
same as SQLite `LIKE` on the `find` side. A field absent from an entry never
matches a leaf op. (One documented divergence from the SQL engine:
`{not: {<leaf over an absent field>}}` — the in-memory engine treats the absent
leaf as `false` then negates to `true`, whereas SQL's three-valued logic would
exclude the row. Keep `not` over fields your entries always carry.)

## Notes

- **Never delete prose in the same step you add params.** Mutate the body via
  `artifact(action="update", patch={body_edits: […]})`, never a wholesale
  `body` overwrite — the 50% body-shrink guard exists to catch exactly this.
- **Keep `id` on every entry object** so filtered rows map back to their
  `## X-N` section.
- Only `params`-based shapes are filterable. A `reflective` tracker whose body
  *is* the content (no structured params) must be retrofit per the steps above
  before `entry_filter` will work on it.
