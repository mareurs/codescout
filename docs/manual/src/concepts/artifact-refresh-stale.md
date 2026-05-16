# `artifact_refresh_stale`

Discovery tool: surfaces augmented artifacts whose last refresh is older than a
threshold. Returns them oldest-first (never-refreshed first) so the agent knows
what to call `artifact_refresh` on next.

## Schema

```json
{
  "threshold_hours": 24,
  "limit": 10,
  "scope": "project"
}
```

All fields are optional.

| Field | Default | Notes |
|-------|---------|-------|
| `threshold_hours` | `24` | Hours since last refresh to consider stale |
| `limit` | `10` | Max results (capped at 50) |
| `scope` | `"project"` | `project` \| `repo` \| `all` |

## Output

```json
{
  "count": 2,
  "threshold_hours": 24,
  "items": [
    {
      "id": "abc123",
      "kind": "tracker",
      "title": "My Tracker",
      "rel_path": "codescout/docs/trackers/my-tracker.md",
      "last_refreshed_at": null,
      "refresh_count": 0,
      "age_hours": null
    }
  ],
  "next_step": "Call artifact_refresh(id) on each item..."
}
```

`age_hours: null` means never refreshed. Items ordered: never-refreshed first,
then oldest `last_refreshed_at` ascending.

## Typical workflow

```
artifact_refresh_stale(scope="repo")
→ pick item from list
artifact_refresh(id)
→ synthesize new content
artifact_update(id, { body: "..." })
artifact_refresh_commit(id)
```

## Known limitations

- `scope=umbrella` is not supported (returns a recoverable error).
- Threshold is wall-clock time only — no per-artifact config yet.
- No priority weighting; ordering is strictly oldest-first.
