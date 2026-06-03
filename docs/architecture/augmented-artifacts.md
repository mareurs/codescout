# Augmented Artifacts

A pattern for storing structured data alongside human-readable markdown, with
auto-maintained synthesis between them. Used by `audit_doc_refs`,
`tool-usage-patterns`, goal trackers, and any "the markdown reflects live
state I cannot summarize in prose" surface.

This page is the mental model. The implementation lives in
`src/librarian/catalog/augmentation.rs` (catalog DB row, merge_params
validation) and `src/cli/mod.rs::artifact_augment` (CLI entry); the tool
surfaces are `artifact_augment`, `artifact_refresh`, and
`artifact(action="update", commit_refresh=true)`.

## Why this exists

Plain markdown trackers work fine until the data shape outgrows what a human
wants to maintain by hand. A 12,000-entry audit table, a structured T-N
observation set with cross-referenced verdicts, a goal-tracker's progress
log — these are *data* the LLM (or a tool) maintains, but rendering them as
prose for human reading is still useful. Two problems:

1. **Data + prose in one markdown file** — the file becomes unreadable.
   Humans see 12K JSON-ish lines; the librarian sees 12K low-signal nodes.
2. **Data in a separate JSON file** — the markdown loses its grounding
   ("see the JSON for state") and the librarian can't index the data.

Augmented artifacts decouple the two without divorcing them: data lives in
the catalog DB as structured `params`; the markdown body holds prose; an
optional `render_template` can project params into the body whenever the
artifact refreshes.

## Two faces: time-aware log and on-demand skill

An augmented tracker is more than "data + prose." Once augmented, the artifact
carries four things, and a reader — human or LLM — gets all of them at once:

1. **Data** — structured rows in `params`. Naming the array via
   `entry_collection` makes them filterable with
   `artifact(action="get", entry_filter=…)`.
2. **Rendering** — an optional `render_template` projects `params` into the
   markdown body, so the human view never drifts from the data.
3. **An embedded skill** — the augmentation `prompt` is a standing instruction,
   surfaced as a `[LIVE]` blockquote whenever the artifact enters a
   `librarian(action="context")` bundle. It tells the next agent *how to
   maintain this tracker, what its params mean, which gather sources refresh
   it.* The artifact teaches the reader how to use the artifact.
4. **History** — every body write emits a `field_patch` event;
   `artifact_event(action="list")` plus `artifact(action="state_at")` /
   `librarian(action="workspace_state_at")` answer "what did this tracker say
   at commit X / hold last week."

Read one way it is a **time-aware log** (face 4) you can replay; read another it
is an **on-demand skill** (face 3) the agent loads just-in-time. Two living
proofs carry all four faces: `tool-usage-patterns` (id `b3fa993849ac83ab`) and
`doc-ref-audit` (id `fc97be512112fea4`).

**Why state this explicitly.** Usage telemetry across two independent codebases
shows the capability is *undiscovered at the point of use*: agents hand-maintain
structured tracker tables with `edit_markdown` (380 calls across 39 files in
codescout; 659 across 59 files in MRV-poc) while `entry_collection` is set on
6 artifacts in one repo and **zero** in the other, and the time-travel surfaces
sit near 0.1% of calls. The gap is discoverability, not missing mechanism — the
measured baseline and the efficacy rubric live in
`docs/evals/augmented-tracker-discovery.md`.

## The body / params / prompt split

An augmented artifact has **three controllable channels**:

| Channel | Where it lives | Lifecycle | Edited by |
|---|---|---|---|
| **Body** | The `.md` file on disk | Re-rendered when artifact refreshes (if `render_template` set) | Auto-render OR human via `edit_markdown` |
| **Params** | Catalog DB row (`augmentations.params`) | Mutated via `artifact_augment(merge=true, ...)` or by the producing tool | Programmatic only — never hand-edit a managed file's params via filesystem |
| **Prompt** | Catalog DB row (`augmentations.prompt`) | Set once at augmentation; carries the LLM-facing instruction for `artifact_refresh(gather)` | `artifact_augment(merge=false, prompt=..., params=...)` to replace |

Plus four optional fields stored alongside the prompt:

- `render_template` — MiniJinja template that projects `params` into a
  markdown snippet, used to keep the body in sync with the data.
- `params_schema` — JSON Schema that validates `params` on every merge.
- `append_mode` — if true, refreshes prepend a dated section instead of
  replacing the body. The prompt should produce only the new delta.
- `history_cap` — max number of `## YYYY-MM-DD` sections to retain in
  append-mode bodies.

## Why some markdown is "managed" (refuses direct read/edit)

When an artifact has an augmentation, `read_markdown` and `edit_markdown`
refuse to touch the file directly. The rationale: the body is **not the
source of truth** — params are. A direct edit would either be silently
overwritten by the next refresh, or would create a body that doesn't
match the params (leading to confusion about which is canonical).

The error redirects to the artifact tools:

```
'docs/trackers/doc-ref-audit.md' is a librarian-managed artifact —
do not read or edit it directly

Use artifact tools instead:
• Read:   artifact(action="get", id="<id>")
• Find:   artifact(action="find", semantic="<topic>")
• Edit:   artifact(action="update", id="<id>", patch={...})
```

The gate is intentional friction — it forces you through a path that
respects the params/body distinction.



## Body editing surfaces — `body_edits` vs. `body`

`artifact(update)` exposes two body-mutation modes plus an escape hatch.
Picking the wrong one cost a real ~600-line tracker body in 2026-05-25
(see `docs/issues/`).

| Patch shape | Effect | Guard |
|---|---|---|
| `patch={body_edits: [{heading, action, content?\|old_string+new_string?, at?, replace_all?, include_subsections?}, ...]}` | Surgical per-section edits. Each entry mirrors `edit_markdown`'s batch shape. Atomic (all-or-nothing). | Per-entry `include_subsections` guard for `action="replace"`. |
| `patch={body: "..."}` | Total overwrite — the new string becomes the entire body. | **50% shrink guard.** If the new body is more than 50% shorter than the old body, the write is refused with `RecoverableError("body-shrink guard: ...")`. |
| `force=true` (top-level on the call) | Bypass the shrink guard. | Use only when shrinkage is intentional (full rewrite, archiving). |

**Mutual exclusion.** `patch={body, body_edits}` together returns
`RecoverableError("body and body_edits are mutually exclusive")`. Pick one.

**Exemptions to the shrink guard.** It does not fire when:

- The original file is under 200 bytes (the threshold is meaningless for
  near-empty shells; new artifacts inside this window can shrink freely).
- The augmentation has `append_mode + history_cap` set — legitimate
  history trimming is expected to shrink the body on each refresh.
- The caller passed `force=true`.

**Patch-key strictness.** `UpdatePatch` now uses
`#[serde(deny_unknown_fields)]`. Misspelled keys like `body_prepend_section`
return `RecoverableError("unknown field 'body_prepend_section'")` listing
the valid fields, instead of silently no-opping.

**Forensic trail.** Every body mutation emits an `events` row:

- `kind="field_patch"`
- `payload={field: "body", prev_bytes, new_bytes, edits_count, mode: "overwrite"|"edits", forced}`

Query via `artifact_event(action="list", artifact_id=X)` — a single body
write that shouldn't have happened is now reconstructable from the event
timeline without scraping `usage.db`.

**The anti-pattern to remember.** The 2026-05-25 incident:

```text
1. artifact(get, id=X, heading="Currently Shipped")  → returns one section
2. artifact(update, id=X, patch={body: <just that section>})  → WIPES body
```

The `artifact(get, heading=)` shape *returns* a section, but
`patch={body}` *replaces* the entire body with whatever string is passed.
The LLM's mental model "I have the body in hand, I'll write it back" is
wrong — it has *a section* in hand. The shrink guard catches the >50%
case; the surgical `body_edits[]` surface removes the temptation to write
a partial body in the first place.
## The artifact_augment lifecycle

`artifact_augment` controls the prompt + params + ancillary fields:

| Call shape | What happens |
|---|---|
| `artifact_augment(id, prompt=..., params=...)` (merge=false, default) | **Full replace.** Overwrites ALL six caller-controlled fields: prompt, params, render_template, params_schema, append_mode, history_cap. Fields you omit silently reset to None / false. |
| `artifact_augment(id, merge=true, params={...})` | **Params-only patch.** RFC 7396 merge-patch into existing params. Prompt and other fields unchanged. |
| `artifact_augment(id, merge=true, params={key: null})` | **Delete a params key.** RFC 7396 semantics: null deletes. |
| `artifact_refresh(action="gather", id)` | **Read-only gather** — collects context for an LLM to synthesize. Does NOT write. The caller must follow up with `artifact(update, commit_refresh=true)`. |
| `artifact(update, id, commit_refresh=true)` | Records that a refresh cycle completed. Updates `last_refreshed_at` and optionally bumps body. |

The `merge=false` overwrite semantics are a foot-gun: if you mean to update
only the prompt but call `artifact_augment(id, prompt="new")` without
passing the existing params, params silently reset to `{}`. **Use
`merge=true` when patching one field.** Use `merge=false` only when
deliberately replacing the entire augmentation.

## How render_template works

When set, `render_template` is a MiniJinja template that runs every time
the artifact body refreshes. It receives `params` as input and produces
the markdown snippet that becomes (or merges into) the body.

Example shape from a goal tracker:

```jinja2
## Progress

{% for entry in progress_log -%}
- **{{ entry.date }}** ({{ entry.commit }}) — {{ entry.note }}
{% endfor %}
```

The body's `## Progress` section is auto-managed; the rest of the markdown
is hand-written prose that explains what the artifact is for and why.

**Without `render_template`**, the body is whatever the prompt + LLM
produces during `artifact_refresh(gather) → artifact(update)`. With it,
the body is mechanically derived from params and the LLM's job is just to
update params correctly.

## Worked examples

### `doc-ref-audit` — id `fc97be512112fea4`

- **Body** (`docs/trackers/doc-ref-audit.md`): 187 bytes. Just the auto-
  managed message "Auto-managed by `librarian(audit_doc_refs)`."
- **Params**: 5.4 MB. Holds the `issues` array (12,753 entries as of
  2026-05-23) plus audit metadata.
- **Prompt**: Tells the audit tool how to merge new findings into the
  issues array (lifecycle, n-allocation, severity escalation).
- **No `render_template`**: the body is a stable one-liner; nothing
  needs synthesizing from params.

Inspect:

```text
artifact(action="get", id="fc97be512112fea4", full=true)
read_file("@tool_*", json_path="$.augmentation.params")
```

### `tool-usage-patterns` — id `b3fa993849ac83ab`

- **Body** (`docs/trackers/tool-usage-patterns.md`): full markdown prose,
  ~200+ lines. Per-observation analysis.
- **Params**: structured `observations` array — id, tool, verdict, prompt
  gap, status — per T-N entry.
- **Prompt**: refreshes the top-of-body table from `observations`.
- **`render_template`**: projects `observations[]` into a "live params
  table" that's rendered at the top of the file on refresh.

The split is **structured-at-top, prose-at-bottom** — humans grok the
prose, the LLM updates params, the table auto-syncs.

## Common gotchas

- **Silent param wipe** (the `merge=false` foot-gun) — see lifecycle table
  above. Always prefer `merge=true` when patching.
- **The body file looks unchanged after a params update** — render_template
  hasn't run. Force a refresh: `artifact_refresh(gather)` → synthesize →
  `artifact(update, commit_refresh=true)`.
- **`read_markdown` rejects the file** — managed artifact gate is firing.
  Route through `artifact(get, full=true)` then `read_file` with
  `json_path` to extract the field you need.
- **Params field is 5+ MB** — your `read_file` will route the result to a
  `@file_*` buffer. Use `json_path` to extract specific fields rather than
  scanning the whole blob. Example:
  `read_file("@tool_*", json_path="$.augmentation.params.issues[0]")`.
- **Params keys don't appear in indexes** — the librarian indexes
  `(id, kind, status, tags, title, abs_path, owners, topic)` but NOT
  params content. If you want to query by params (e.g. "find all
  goal-trackers with status_log entries past 2026-04"), you need to
  augment the librarian or post-filter after `artifact(find)`.

## When to augment vs. when not to

Augment when:

- The artifact carries structured state that's mutated programmatically
  (audit findings, observation rows, progress log entries)
- The data shape exceeds 10-20 entries — past that, hand-maintained
  markdown breaks down
- You want the markdown body to auto-sync with the data via a template

Do NOT augment when:

- The tracker is purely narrative (recon session logs, design proposals)
- The "data" is just a handful of fields better expressed as frontmatter
- You'd be the only producer/consumer of the params — keep it as prose

## Pointers

- Tool surfaces: `artifact_augment`, `artifact_refresh`, `artifact(update, commit_refresh=true)`
- Implementation: `src/librarian/catalog/augmentation.rs` (row + merge + schema validation)
- Schema: `params_schema` is enforced on every merge via `merge_params`
  (see `src/librarian/catalog/augmentation.rs::merge_params`)
- Templates: MiniJinja syntax with `params` as the sole top-level binding
- Two reference artifacts: `fc97be512112fea4` (doc-ref-audit),
  `b3fa993849ac83ab` (tool-usage-patterns)
