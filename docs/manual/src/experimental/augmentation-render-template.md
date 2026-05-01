# Augmentation: `render_template` + `params_schema`

> ⚠ Experimental — may change without notice.

Two optional fields on augmented artifacts that decouple live state (params)
from narrative (artifact body).

## `render_template`

A [MiniJinja](https://docs.rs/minijinja) template projecting `params` into a
markdown snippet injected under the `[LIVE]` header in `librarian_context`
output. Set it when you want a status table, flag grid, or F-N row list that
the agent can read without parsing raw JSON.

### Common patterns

```jinja
{# Table from an array #}
| id | status | owner |
|----|--------|-------|
{% for f in failures %}| {{ f.id }} | {{ f.status }} | {{ f.owner or "—" }} |
{% endfor %}

{# Dict iteration #}
{% for env, s in envs|items %}{{ env }}: {{ "✅" if s.enabled else "❌" }}
{% endfor %}

{# Filtered count #}
{{ items|selectattr("status","equalto","fail")|list|length }} failing
```

### Schema

Pass `render_template` to `artifact_augment` or `artifact_update_params`:

```json
{
  "render_template": "**Flag:** `{{ flag_name }}`\n..."
}
```

### Behaviour

- Evaluated at `librarian_context` read time against current `params`.
- Errors in template evaluation are surfaced inline (not fatal).
- Omit the field for `reflective` trackers — prose-only body needs no template.

## `params_schema`

A JSON Schema (draft-07+) validating `params` on every write:
`artifact_augment` (initial seed) and `artifact_update_params` merges.
Violations return a recoverable error — params are **not** written.

### When to use

- **Early life:** omit or use `additionalProperties: true`. Let the shape settle
  over 2-3 refreshes.
- **Mature:** add `required`, `enum`, `pattern` constraints to lock drift out.

### Example

```json
{
  "params_schema": {
    "type": "object",
    "required": ["failures"],
    "properties": {
      "failures": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["id", "status"],
          "properties": {
            "id":     { "type": "string", "pattern": "^F-\\d+$" },
            "status": { "type": "string", "enum": ["fail","pass","flaky","wontfix"] }
          }
        }
      }
    }
  }
}
```

## Known limitations

- Template is re-evaluated on every `librarian_context` call — no caching.
- Schema validation uses draft-07 semantics; newer keywords are ignored.
