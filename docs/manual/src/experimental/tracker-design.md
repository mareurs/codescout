# `tracker_design`

> ⚠ Experimental — may change without notice.

Teaching tool that guides tracker creation. Call it **before** `tracker_create`
whenever the user asks to create a tracker.

## What it returns

```json
{
  "design_version": "1",
  "system_prompt": "...",   // 7-step design guide
  "archetypes": [...],       // 6 archetype definitions
  "existing_trackers": [...],// current trackers in catalog (cap 30)
  "next_step": "..."
}
```

## The 7 steps

1. **Pick an archetype** — match intent to one of 6 archetypes.
2. **Write the augmentation prompt** — imperative, names sources, states conflict resolution.
3. **Design params** — live state only, flat, stable keys.
4. **Decide schema discipline** — loose early, lock when mature.
5. **Compose `render_template`** — MiniJinja projecting params to markdown.
6. **Sketch body skeleton** — prose sections + History block.
7. **Check for collisions** — `existing_trackers` prevents duplicate concerns.

## Archetypes

| Name | When to use |
|------|-------------|
| `deployment_state` | Feature flag / env rollout state per environment |
| `failure_table` | Numbered F-N list from a test/eval suite |
| `metric_baseline` | Living benchmark log with baseline + session deltas |
| `audit_issues` | Numbered audit output with severity + status |
| `task_list` | Phase-based task list with done/open/blocked status |
| `reflective` | Design brainstorm or decision log — prose-driven |

Each archetype ships with `params_shape_example`, `params_schema_example`,
`render_template_example`, `body_skeleton`, and `prompt_template`.

## Usage

```
tracker_design()                    // load guide + landscape
→ pick archetype, compose spec
tracker_create(repo, rel_path, title, prompt, params, ...)
```

Pass `intent` to `tracker_design` for future tailoring (reserved, currently
echoed back in the response).

## Known limitations

- `existing_trackers` is capped at 30 entries (scope=repo).
- `intent` field is reserved; no tailoring is applied yet.
