# workspace_state_at — Time-Travel Workspace Snapshot

The `workspace_state_at` tool returns a snapshot of every artifact in scope
as it stood at a given commit or timestamp, with a per-artifact comparison of
`freshness_at_as_of` (freshness replayed up to the cutoff) vs `freshness_now`
(freshness from current state).

## Use cases

- "What was stale at the time of release v2.3?"
- "Which specs were unreviewed when we cut the RC?"
- "Show me everything that has become stale since the last review round."

## Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `commit` | string | — | Commit hash as cutoff. Exactly one of `commit` / `timestamp` required. |
| `timestamp` | integer | — | Unix epoch ms as cutoff. Exactly one of `commit` / `timestamp` required. |
| `scope` | string | `project` | `project` \| `repo` \| `umbrella` \| `all` |
| `kinds` | string[] | all | Filter by artifact kind (e.g. `["spec", "adr"]`). |
| `include_archived` | boolean | `false` | Include archived / superseded artifacts. |
| `freshness_filter` | string[] | all | Only return artifacts whose `freshness_at_as_of` is one of `fresh`, `stale`, `unknown`, `superseded`. |

## Response shape

```json
{
  "as_of": 1714300000000,
  "scope": { "scope": "all", ... },
  "artifacts": [
    {
      "id": "specs/auth-redesign",
      "kind": "spec",
      "status_at_as_of": "active",
      "freshness_at_as_of": "stale",
      "freshness_now": "fresh",
      "freshness_changed": true,
      "latest_event_at_as_of": { "id": "...", "kind": "reviewed", "created_at": 1714200000000 },
      "supersession_chain": [],
      "rel_path": "specs/auth-redesign.md",
      "repo": "my-repo"
    }
  ],
  "hints": {
    "scope_fallback": false,
    "more_in_scope": 42,
    "hint": "Result capped at 200. Narrow with `kinds`, `freshness_filter`, or a tighter scope."
  }
}
```

## Cap behaviour

Results are capped at **200 artifacts**. When more candidates exist,
`hints.more_in_scope` reports the total excess and `hints.hint` suggests
how to narrow the query.

## Freshness semantics

- **`freshness_at_as_of`** — computed by replaying only events with
  `created_at ≤ cutoff`. This is the true historical freshness.
- **`freshness_now`** — computed from all events without a cutoff (current
  state). Uses `file_mtime` from the current artifact row in both cases.
- **`freshness_changed = true`** when the two values differ — the most
  interesting signal for "what has drifted since commit X?"
