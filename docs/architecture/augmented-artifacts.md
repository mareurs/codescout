# Augmented Artifacts

A pattern for storing structured data alongside human-readable markdown, with
auto-maintained synthesis between them. Used by `audit_doc_refs`,
`tool-usage-patterns`, goal trackers, and any "the markdown reflects live
state I cannot summarize in prose" surface.

This page is the mental model. The implementation lives in
`src/librarian/catalog/augmentation.rs` (catalog DB row, merge_params
validation) and `src/cli/mod.rs::artifact_augment` (CLI entry); the tool
surfaces are `doc(action="augment")`, `doc(action="gather")` / `doc(action="list_stale")`, and
`doc(action="update", commit_refresh=true)`.

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
optional `render_template` projects params into the `librarian(action="context")` bundle whenever the
artifact refreshes.

## Two faces: time-aware log and on-demand skill

An augmented tracker is more than "data + prose." Once augmented, the artifact
carries four things, and a reader — human or LLM — gets all of them at once:

1. **Data** — structured rows in `params`. Naming the array via
   `entry_collection` makes them filterable with
   `doc(action="get", entry_filter=…)`.
2. **Rendering** — an optional `render_template` projects `params` into the
   `librarian(action="context")` bundle — **not** into the `.md` file on disk. See
   § *How render_template works* for what this does and does not keep in sync.
3. **An embedded skill** — the augmentation `prompt` is a standing instruction,
   surfaced as a `[LIVE]` blockquote whenever the artifact enters a
   `librarian(action="context")` bundle. It tells the next agent *how to
   maintain this tracker, what its params mean, which gather sources refresh
   it.* The artifact teaches the reader how to use the artifact.
4. **History** — every body write emits a `field_patch` event;
   `doc(action="event_list")` plus `doc(action="state_at")` /
   `librarian(action="workspace_state_at")` answer "what did this tracker say
   at commit X / hold last week."

Read one way it is a **time-aware log** (face 4) you can replay; read another it
is an **on-demand skill** (face 3) the agent loads just-in-time. Two living
proofs carry all four faces: `tool-usage-patterns` (id `f2ecdd76a6189efb`) and
`doc-ref-audit` (id `fc97be512112fea4`).

**Why state this explicitly.** Usage telemetry across two independent codebases
shows the capability is *undiscovered at the point of use*: agents hand-maintain
structured tracker tables with `edit_file` (380 calls across 39 files in
codescout; 659 across 59 files in MRV-poc) while `entry_collection` is set on
6 artifacts in one repo and **zero** in the other, and the time-travel surfaces
sit near 0.1% of calls. The gap is discoverability, not missing mechanism — the
measured baseline and the efficacy rubric live in
`docs/evals/augmented-tracker-discovery.md`.

## The body / params / prompt split

An augmented artifact has **three controllable channels**:

| Channel | Where it lives | Lifecycle | Edited by |
|---|---|---|---|
| **Body** | The `.md` file on disk | Written by whoever edits it — **never auto-rendered from params** | Auto-render OR human via `edit_file` |
| **Params** | Catalog DB row (`augmentations.params`) | Mutated via `doc(action="augment", merge=true, augment={params: ...})` or by the producing tool | Programmatic only — never hand-edit a managed file's params via filesystem |
| **Prompt** | Catalog DB row (`augmentations.prompt`) | Set once at augmentation; carries the LLM-facing instruction for `doc(action="gather")` | `doc(action="augment", merge=false, augment={prompt: ..., params=...)` to replace |

Plus four optional fields stored alongside the prompt:

- `render_template` — MiniJinja template that projects `params` into a
  markdown snippet, used to keep the body in sync with the data.
- `params_schema` — JSON Schema that validates `params` on every merge.
- `append_mode` — if true, refreshes prepend a dated section instead of
  replacing the body. The prompt should produce only the new delta.
- `history_cap` — max number of `## YYYY-MM-DD` sections to retain in
  append-mode bodies.

## Why some markdown is "managed" (refuses direct read/edit)

When an artifact has an augmentation, `read_file` and `edit_file`
refuse to touch the file directly. The rationale: the body is **not the
source of truth** — params are. A direct edit would either be silently
overwritten by the next refresh, or would create a body that doesn't
match the params (leading to confusion about which is canonical).

The error redirects to the artifact tools:

```
'docs/trackers/doc-ref-audit.md' is a librarian-managed artifact —
do not read or edit it directly

Use artifact tools instead:
• Read:   doc(action="get", id="<id>")
• Find:   doc(action="find", semantic="<topic>")
• Edit:   doc(action="update", id="<id>", patch={...})
```

The gate is intentional friction — it forces you through a path that
respects the params/body distinction.



## Body editing surfaces — `body_edits` vs. `body`

`doc(update)` exposes two body-mutation modes plus an escape hatch.
Picking the wrong one cost a real ~600-line tracker body in 2026-05-25
(see `docs/issues/`).

| Patch shape | Effect | Guard |
|---|---|---|
| `patch={body_edits: [{heading, action, content?\|old_string+new_string?, at?, replace_all?, include_subsections?}, ...]}` | Surgical per-section edits. Each entry mirrors `edit_file`'s batch shape. Atomic (all-or-nothing). | `action="replace"` is refused when it would consume nested headings — **unless** `include_subsections: true`, which is the guard's off switch, not a guard. |
| `patch={body: "..."}` | Total overwrite — the new string becomes the entire body. | **50% shrink guard.** If the new body loses more than 50% of the old body's **bytes or lines**, the write is refused with `RecoverableError("body-shrink guard: ...")`, naming which dimension went over. |
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
- `payload={field: "body", prev_bytes, new_bytes, edits_count, mode: "overwrite"|"edits", forced, replaced_subsections}`

`prev_bytes` / `new_bytes` are **whole-file** aggregates, so they cannot express
a section-level loss: a `replace` that dropped a child heading while adding more
text than it removed records `prev_bytes < new_bytes` and reads as a benign
append. `replaced_subsections` (added 2026-08-06) is the field that names the
destroyed headings.

Query via `doc(action="event_list", id=X)` — a single body
write that shouldn't have happened is now reconstructable from the event
timeline without scraping `usage.db`.

**The anti-pattern to remember.** The 2026-05-25 incident:

```text
1. doc(get, id=X, heading="Currently Shipped")  → returns one section
2. doc(update, id=X, patch={body: <just that section>})  → WIPES body
```

**The second anti-pattern.** `body_edits` is surgical, but not *automatically*
safe. Using `replace` + `include_subsections: true` to add one entry to a section
means reconstructing that whole section from memory — and every child you forget
to re-emit is deleted:

```text
doc(update, id=X, patch={body_edits: [{
    heading: "## Wins", action: "replace", include_subsections: true,
    content: "## Wins\n\n### W-3 — new\n..."     # W-1 and W-2 are GONE
}]})
```

Nothing refuses this — the opt-in flag is what you passed to make it legal, and
the shrink guard is satisfied because the file grew. The surviving signal is
`replaced_subsections` in the response and the event payload. The safe shape for
"add a sibling" is `insert_after` targeting the last existing child heading, which
never re-emits content it isn't changing. Filed and fixed in `45669701`, which added the `replaced_subsections` reporting;
the bug file is archived at
`docs/issues/archive/2026-08-06-body-edits-section-replace-silent-data-loss.md`.

The `doc(get, heading=)` shape *returns* a section, but
`patch={body}` *replaces* the entire body with whatever string is passed.
The LLM's mental model "I have the body in hand, I'll write it back" is
wrong — it has *a section* in hand. The shrink guard catches the >50%
case; the surgical `body_edits[]` surface removes the temptation to write
a partial body in the first place.

**`full=true` does not exempt you from this, and that is the sharper trap.**
`get`'s body is capped at 500 lines (`SOFT_CAP_LINES`,
`src/librarian/tools/get.rs`); `full` opts out of section-scoping, not out of
the cap. So on any artifact over 500 lines the "I have the body in hand"
model is wrong in the one case where it feels safest. The response says so —
`body_meta.line_count` against `body_meta.source_line_count`, plus an
`overflow` object — but a shell pipeline reading `.body` never looks at
sibling keys, and the byte arm of the shrink guard cannot see a truncation
that keeps a document's long-lined front. That combination deleted 1047 of
1553 lines of a tracker on 2026-08-28 and returned `updated: true`; the line
arm was added the next day in response. See
`docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md`.

**Rule: never build a write payload from a `get` response.** Rebuild it from
the file or from `git show <sha>:<path>`.
## The `doc(action="augment")` lifecycle

`doc(action="augment")` controls the prompt + params + ancillary fields:

| Call shape | What happens |
|---|---|
| `doc(action="augment", id, augment={prompt: ..., params: ...})` (merge=false, default) | **Full replace.** Overwrites ALL six caller-controlled fields: prompt, params, render_template, params_schema, append_mode, history_cap. Fields you omit silently reset to None / false. |
| `doc(action="augment", id, merge=true, augment={params: {...}})` | **Params-only patch.** RFC 7396 merge-patch into existing params. Prompt and other fields unchanged. |
| `doc(action="augment", id, merge=true, augment={params: {key: null}})` | **Delete a params key.** RFC 7396 semantics: null deletes. |
| `doc(action="gather", id)` | **Read-only gather** — collects context for an LLM to synthesize. Does NOT write. The caller must follow up with `doc(update, commit_refresh=true)`. |
| `doc(update, id, commit_refresh=true)` | Records that a refresh cycle completed. Updates `last_refreshed_at` and optionally bumps body. |

The `merge=false` overwrite semantics are a foot-gun: if you mean to update
only the prompt but call `doc(action="augment", id, augment={prompt: "new"})` without
passing the existing params, params silently reset to `{}`. **Use
`merge=true` when patching one field.** Use `merge=false` only when
deliberately replacing the entire augmentation.

## How render_template works

When set, `render_template` is a MiniJinja template that runs every time
the artifact body refreshes. It receives `params` as input and produces
the markdown snippet.

Example shape from a goal tracker:

```jinja2
## Progress

{% for entry in progress_log -%}
- **{{ entry.date }}** ({{ entry.commit }}) — {{ entry.note }}
{% endfor %}
```

The body's `## Progress` section is auto-managed; the rest of the markdown
is hand-written prose that explains what the artifact is for and why.

**Where the render actually goes (verified against the code, 2026-08-17).**
`render_params` has exactly two production callers, and only one of them uses
the augmentation's *stored* template:

| Caller | Destination | Uses the stored `render_template`? |
|---|---|---|
| `src/librarian/tools/context.rs` | the `librarian(action="context")` markdown bundle, under the `[LIVE]` header | yes |
| `legibility_scan::render_managed_body` | the `.md` file body | **no** — a compiled-in `include_str!` of `src/librarian/tools/legibility_scan/render_template.j2`, for the `legibility-backlog` tracker only |

So **the stored `render_template` never reaches the file on disk.** The refresh
path does not consume it: every `render_template` occurrence in
`tools/refresh.rs` and `tools/update.rs` is a test fixture set to `None`. The
body is whatever the prompt + LLM produce during `doc(action="gather") →
doc(update, commit_refresh=true)`, with or without a template.

The one tracker whose body *does* track params proves the rule rather than
breaking it: `legibility_scan` had to write its own body-projector, and the
function's doc comment says why — *"Fixes F-8: previously `params` updated but
the body stayed stale, forcing a manual re-render after every scan."* It exists
because the general mechanism does not.

**Consequence for design.** Params are invisible to anyone reading the file on
disk or on a git host. Never make `params` the canonical home for entries that
are cited by id: `link_scan` derives an entry token's definition from a
**heading** and nothing else, so a params row defines no token. Params are for
lifecycle state; the body heading is the entry's identity. See
`get_guide("tracker-conventions")` § *Entry-level standard*.

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
doc(action="get", id="fc97be512112fea4", full=true)
read_file("@tool_*", json_path="$.augmentation.params")
```

### `tool-usage-patterns` — id `f2ecdd76a6189efb`

- **Body** (`docs/trackers/tool-usage-patterns.md`): full markdown prose,
  ~200+ lines. Per-observation analysis.
- **Params**: structured `observations` array — id, tool, verdict, prompt
  gap, status — per T-N entry.
- **Prompt**: refreshes the top-of-body table from `observations`.
- **`render_template`**: projects `observations[]` into a table that appears in
  the `librarian(action="context")` bundle. **It is not in the file.**

**Two corrections to this example (2026-08-17).** The heading's id is stale —
the artifact is `f2ecdd76a6189efb`, as CLAUDE.md and the file's own frontmatter
both state. And the split is *not* "structured-at-top, prose-at-bottom": the
file on disk contains frontmatter, prose, and 22 `### T-N` headings, with **zero
table rows and no rendered block**. The structured rows live only in the catalog;
reach them with `doc(action="get", id="f2ecdd76a6189efb", entry_filter={…})`.

That shape is the good one, incidentally — one heading per entry is what keeps
its `T-N` citations resolvable. Measured project-wide the same day, this tracker
contributed **zero** dangling entry tokens, while a sibling ledger carrying rows
instead of headings contributed roughly thirty.

## Common gotchas

- **Silent param wipe** (the `merge=false` foot-gun) — see lifecycle table
  above. Always prefer `merge=true` when patching.
- **The body file looks unchanged after a params update** — that is the designed
  behaviour, not a stale render. The stored `render_template` is projected into
  `librarian(action="context")`, never into the file, and the refresh path does
  not consume it at all. A refresh cycle cannot fix this because there is nothing
  to fix. If the body must show the data, write it there yourself via
  `doc(update, patch={body_edits: […]})` — or accept that the catalog is the
  only home for it and query with `entry_filter`. (This bullet previously told
  readers to "force a refresh", sending them into a cycle that cannot change the
  file.)
- **`read_file` rejects the file** — managed artifact gate is firing.
  Route through `doc(get, full=true)` then `read_file` with
  `json_path` to extract the field you need.
- **Params field is 5+ MB** — your `read_file` will route the result to a
  `@file_*` buffer. Use `json_path` to extract specific fields rather than
  scanning the whole blob. Example:
  `read_file("@tool_*", json_path="$.augmentation.params.issues[0]")`.
- **Params keys don't appear in indexes** — the librarian indexes
  `(id, kind, status, tags, title, abs_path, owners, topic)` but NOT
  params content. If you want to query by params (e.g. "find all
  goal-trackers with status_log entries past 2026-04"), you need to
  augment the librarian or post-filter after `doc(find)`.

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

- Tool surfaces: `doc(action="augment")`, `doc(action="gather")` / `doc(action="list_stale")`, `doc(update, commit_refresh=true)`
- Implementation: `src/librarian/catalog/augmentation.rs` (row + merge + schema validation)
- Schema: `params_schema` is enforced on every merge via `merge_params`
  (see `src/librarian/catalog/augmentation.rs::merge_params`)
- Templates: MiniJinja syntax with `params` as the sole top-level binding
- Two reference artifacts: `fc97be512112fea4` (doc-ref-audit),
  `f2ecdd76a6189efb` (tool-usage-patterns)
