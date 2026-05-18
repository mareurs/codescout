# Output Modes

codescout tools produce different amounts of detail depending on which mode
they operate in. Understanding the two modes — and when to switch between them
— is the key to using the tools efficiently.

## Exploring Mode (Default)

Exploring mode is the default for every tool that supports it. You do not need
to pass any parameter to get it.

In Exploring mode, tools return compact summaries:

- **Symbols:** name, kind, file path, start and end line. No function bodies.
- **Files:** paths only, no content.
- **Diffs:** truncated at a reasonable size with a count of omitted files.
- **Blame:** first 200 lines, with an overflow message if the file is longer.

Results are capped at **200 items** (or 200 files for directory-spanning tools).
If more results exist, the response includes an `overflow` object explaining
what was omitted and how to narrow the query.

**Example — exploring mode response for `symbols`:**

```json
{
  "file": "src/services/auth.rs",
  "symbols": [
    { "name": "AuthService", "kind": "Struct", "start_line": 12, "end_line": 30 },
    { "name": "handle_login", "kind": "Function", "start_line": 34, "end_line": 60 },
    { "name": "verify_token", "kind": "Function", "start_line": 61, "end_line": 88 }
  ]
}
```

No bodies, no children — just the shape of the file.

## Focused Mode (`detail_level: "full"`)

Focused mode returns full detail and paginates results. Activate it by passing
`detail_level: "full"` to any tool that supports it.

In Focused mode, tools return:

- **Symbols:** full bodies, all children, complete detail.
- **Files:** full content of matching entries.
- **Diffs:** complete diff output, paginated by file if needed.
- **Blame:** paginated line-by-line blame for the full file.

Results are paginated via `offset` and `limit` (default page size: **50**).
The first page starts at `offset: 0`. Subsequent pages use the `next_offset`
value from the overflow object.

**Example — focused mode response for `symbols`:**

```json
{
  "symbols": [
    {
      "name": "verify_token",
      "kind": "Function",
      "file": "src/services/auth.rs",
      "start_line": 61,
      "end_line": 88,
      "body": "pub fn verify_token(token: &str, secret: &[u8]) -> Result<Claims> {\n    let key = DecodingKey::from_secret(secret);\n    let validation = Validation::new(Algorithm::HS256);\n    let data = decode::<Claims>(token, &key, &validation)\n        .map_err(|e| AuthError::InvalidToken(e.to_string()))?;\n    if data.claims.exp < Utc::now().timestamp() as usize {\n        return Err(AuthError::TokenExpired);\n    }\n    Ok(data.claims)\n}"
    }
  ]
}
```

## Switching Between Modes

Pass `detail_level: "full"` to any tool that supports it:

```json
{
  "tool": "symbols",
  "arguments": {
    "pattern": "verify_token",
    "relative_path": "src/services/auth.rs",
    "include_body": true,
    "detail_level": "full"
  }
}
```

```json
{
  "tool": "symbols",
  "arguments": {
    "path": "src/services/",
    "detail_level": "full",
    "limit": 10
  }
}
```

## Text Form (Ripgrep-Style)

> **Status:** experimental — see [Experimental Features](../experimental/index.md).

Some locator tools — `grep`, `references`, `tree`, `symbols`, and
`call_graph` — emit **ripgrep-faithful plain text** instead of JSON when the
result fits inline. The wire form is selected by the tool (declared as
`OutputForm::Text` in the `Tool` trait), not by a parameter.

The format follows ripgrep's conventions so it is immediately legible to any
reader familiar with `rg`:

- A per-file `(count)` header before each block.
- `N:` separator on lines that matched the query.
- `N-` separator on context lines (when `context_lines > 0`).
- `--` between non-adjacent match blocks within the same file.

**Example — `grep` over a small result set:**

```text
src/services/auth.rs (2)
   52- /// Soft error returned when the token is malformed.
   54:     pub error: String,
   55-     pub code: u16,
--
   88:     fn verify_token(token: &str) -> Result<Claims> {

src/api/handlers.rs (1)
   23: use crate::services::auth::verify_token;
```

The same query returns JSON when the result overflows the inline budget and
spills into a `@cmd_*` buffer — buffered output keeps the original JSON shape
for downstream tooling. You do not pick the form; the tool picks it for you
based on size.

### Why the change

JSON for a handful of grep hits costs roughly 3-5× the tokens of the
equivalent ripgrep text and adds zero information — the reader's eye parses
`file:line:content` faster than a `{"path": ..., "line": ..., "content":
...}` envelope. Routing small, locator-shaped results through text frees
context for the actual work.

### Tools currently opted in

| Tool | Text form available | Notes |
|---|---|---|
| `grep` | yes | Includes `context_lines` formatting |
| `references` | yes | One line per reference |
| `tree` (with glob) | yes | One path per line |
| `symbols` | yes | Name + kind + file:line |
| `call_graph` | yes | File-grouped edge tree, top-20 files with `… N more` |

`semantic_search`, `memory(list|recall)`, and `IndexStatus` are queued to
opt in — see `docs/ROADMAP.md` *What's Next*.

## Overflow Messages

When the number of results exceeds the cap, the response includes an `overflow`
object at the top level:

```json
{
  "results": [ "..." ],
  "overflow": {
    "shown": 47,
    "total": 312,
    "hint": "Narrow with a file path or glob pattern"
  }
}
```

The `hint` tells you how to reduce the result set. Common hints suggest
narrowing the path, providing a more specific pattern, or adding a glob filter.

In Focused mode, the overflow object also includes `next_offset` when more
pages exist:

```json
{
  "results": [ "..." ],
  "overflow": {
    "shown": 50,
    "total": 312,
    "hint": "Use offset and limit to page through results",
    "next_offset": 50
  }
}
```

When `next_offset` is absent (or `null`), you are on the last page.

**Paginating through results:**

```json
{ "tool": "symbols", "arguments": { "pattern": "Error", "detail_level": "full", "offset": 0,  "limit": 50 } }
{ "tool": "symbols", "arguments": { "pattern": "Error", "detail_level": "full", "offset": 50, "limit": 50 } }
{ "tool": "symbols", "arguments": { "pattern": "Error", "detail_level": "full", "offset": 100, "limit": 50 } }
```

## Tools That Support Both Modes

These tools respect `detail_level`, `offset`, and `limit`:

| Tool | Exploring output | Focused output |
|------|-----------------|----------------|
| `symbols` | Names, kinds, files, lines | Full symbol trees with bodies |
| `symbols` | Names, kinds, locations | + bodies (when `include_body=true`) |
| `references` | Reference locations | Paginated reference list |
| `tree` | File paths | Paginated entries |
| `grep` | Top matches | Paginated full matches |
| `semantic_search` | Top matches with scores | Paginated full chunks |

## Tools With Fixed Output

Some tools always cap their output at a fixed limit and do not support mode
switching:

| Tool | Behaviour |
|------|-----------|
| `grep` | Always returns up to `max_results` matches (default: 50) |
| `tree` (with glob) | Always returns up to `max_results` paths (default: 100) |
| `symbols` | Returns all symbols in a file (bounded by nature) |

For these tools, use their own `limit` or `max_results` parameter to control
output size. They do not use the `detail_level` / `offset` / `limit` pattern.

## Practical Guidance

Use Exploring mode to find what you are looking for. Use Focused mode only once
you have a specific target. Switching early costs you context; switching at the
right time costs almost nothing.

If you see an overflow message in Exploring mode, prefer narrowing the query
(a more specific path, glob, or pattern) over switching to Focused mode. A
narrower Exploring query is usually more useful than a paginated Focused query
over 300 results.

If you see `next_offset` in Focused mode and you need all the results, page
through sequentially. Do not try to read all pages in a single pass unless you
need the full picture; often the first one or two pages contain the answer.

## Further Reading

- [Progressive Disclosure](progressive-disclosure.md) — the design principle
  behind the two-mode system and how `OutputGuard` enforces it
- [Symbol Navigation Tools](../tools/symbol-navigation.md) — the tools where
  `detail_level` has the most impact
