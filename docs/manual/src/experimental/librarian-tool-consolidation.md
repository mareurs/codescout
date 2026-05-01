> ⚠ Experimental — may change without notice.

# Librarian Tool Consolidation (22 → 16 tools)

Six single-purpose tools absorbed into their natural parent tools, reducing the librarian API surface from 22 to 16 tools.

## What Changed

| Removed tool | Now use |
|---|---|
| `artifact_list_by_kind` | `artifact_find` with `kind` param |
| `artifact_observe` | `artifact_event_create` with `kind=note` |
| `tracker_create` | `artifact_create` with `status=active` + `augment={prompt,params}` |
| `artifact_update_params` | `artifact_augment` with `merge=true` |
| `artifact_links` | `artifact_get` with `include_links=true`, `links_direction`, `links_rel` |
| `artifact_refresh_commit` | `artifact_update` with `commit_refresh=true` |

## New Parameters

### `artifact_find`
- `kind: string` — shortcut for `filter: {kind: {eq: "..."}}` 
- `status: string` — shortcut for `filter: {status: {eq: "..."}}`

### `artifact_create`
- `status: string` — initial status (default: `"draft"`)
- `augment: { prompt, params? }` — attach augmentation atomically (replaces separate `tracker_create` + `artifact_augment`)

### `artifact_augment`
- `merge: bool` — when `true`, RFC 7396 shallow merge-patch on `params` only; `prompt` ignored; errors if no augmentation exists yet

### `artifact_get`
- `links_direction: "out"|"in"|"both"` — filter links by direction (default: `"both"`); requires `include_links=true`
- `links_rel: string` — filter links to a specific rel type; requires `include_links=true`

### `artifact_update`
- `commit_refresh: bool` — when `true`, also increments `refresh_count` and sets `last_refreshed_at` (replaces separate `artifact_refresh_commit` call)
