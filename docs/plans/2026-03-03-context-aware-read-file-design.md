# Context-Aware `read_file` Design

**Date:** 2026-03-03
**Status:** Proposed

## Problem

`read_file`'s smart buffering summarizes large files by type (source → symbols,
markdown → headings, config → first 30 lines, generic → head+tail), but the
summaries are **shallow**: markdown headings lack line ranges, config shows raw
preview text with no structure, and JSON files get the generic head+tail
treatment. The LLM knows *what's in the file* but can't efficiently *navigate*
to the part it needs.

## Solution

Two complementary enhancements to `read_file`:

1. **Enriched structural summaries** — every summary includes line ranges,
   nesting, and structural metadata so the LLM can follow up with precise reads.
2. **Format-specific navigation parameters** — optional params (`heading`,
   `json_path`, `toml_key`) that let the LLM request content by semantic
   location instead of line numbers.

## Scope

Three format families, prioritized by frequency in LLM workflows:

| Format | Summary | Navigation Param |
|--------|---------|-----------------|
| Markdown (`.md`) | Heading tree with line ranges, all levels | `heading` |
| JSON (`.json`) | Schema shape: top-level keys, types, sizes, lines | `json_path` |
| TOML (`.toml`) / YAML (`.yaml`, `.yml`) | Table/key structure with line ranges | `toml_key` |

Source code files are out of scope — they already have `list_symbols` /
`find_symbol` for structural navigation.

## Enriched Summaries

### Markdown

Current: flat list of H1/H2 headings (no line info, no H3+).

New: heading tree with line ranges, all heading levels (up to 20 entries):

```json
{
  "type": "markdown",
  "line_count": 847,
  "ref": "@file_abc123",
  "headings": [
    { "heading": "# Getting Started", "level": 1, "line": 1, "end_line": 45 },
    { "heading": "## Installation", "level": 2, "line": 3, "end_line": 28 },
    { "heading": "## Configuration", "level": 2, "line": 29, "end_line": 45 },
    { "heading": "# API Reference", "level": 1, "line": 46, "end_line": 847 },
    { "heading": "## Authentication", "level": 2, "line": 48, "end_line": 120 },
    { "heading": "### OAuth", "level": 3, "line": 60, "end_line": 95 }
  ]
}
```

Each heading's `end_line` is computed as the line before the next heading of the
same or higher level (or EOF for the last heading).

### JSON

Current: treated as generic (head+tail preview).

New: schema shape with top-level structure:

```json
{
  "type": "json",
  "line_count": 1200,
  "ref": "@file_def456",
  "schema": {
    "root_type": "object",
    "keys": [
      { "path": "$.name", "type": "string", "line": 2 },
      { "path": "$.version", "type": "string", "line": 3 },
      { "path": "$.dependencies", "type": "object", "count": 47, "line": 5 },
      { "path": "$.scripts", "type": "object", "count": 12, "line": 180 },
      { "path": "$.devDependencies", "type": "object", "count": 23, "line": 250 }
    ]
  }
}
```

Shows top-level keys with their types, collection sizes, and approximate line
positions. For arrays at the root, shows element type and count.

### TOML / YAML

Current: first 30 lines as raw preview.

New: table/section structure with line ranges:

```json
{
  "type": "config",
  "format": "toml",
  "line_count": 340,
  "ref": "@file_ghi789",
  "sections": [
    { "key": "[package]", "line": 1, "end_line": 8 },
    { "key": "[dependencies]", "line": 10, "end_line": 85 },
    { "key": "[dev-dependencies]", "line": 87, "end_line": 120 },
    { "key": "[profile.release]", "line": 122, "end_line": 130 },
    { "key": "[[bin]]", "line": 132, "end_line": 140 }
  ]
}
```

## Navigation Parameters

Three new optional parameters on `read_file`, each specific to a format:

### `heading` (Markdown only)

```
read_file("docs/guide.md", heading="## Authentication")
```

- Matches by heading text: exact match first, then prefix match, then substring
- Returns content from the heading line to the next heading of same/higher level
- Response includes structural metadata (see Response Format below)

### `json_path` (JSON only)

```
read_file("package.json", json_path="$.dependencies")
read_file("data.json", json_path="$.users[0]")
```

- JSONPath subset: `$.key`, `$.key.nested`, `$.key[N]` (no wildcards, no filters)
- Parses JSON, extracts the subtree at the specified path
- Returns pretty-printed content with type info

### `toml_key` (TOML/YAML only)

```
read_file("Cargo.toml", toml_key="dependencies")
read_file("config.yaml", toml_key="database.connection")
```

- Dot-separated path for nested keys, or bare name for TOML tables
- Parses the file, extracts the section
- Returns re-serialized content with sibling information

### Constraints

- **Mutually exclusive** with each other and with `start_line`/`end_line`
- **Format mismatch** → `RecoverableError` with hint suggesting the correct param
- **Not found** → `RecoverableError` listing available headings/keys/paths
- **Still works on small files** (<200 lines) — acts as a content filter

## Response Format

When a navigation parameter is used:

```json
{
  "content": "## Authentication\n\nThis API uses OAuth 2.0...",
  "line_range": [48, 120],
  "breadcrumb": ["# API Reference", "## Authentication"],
  "siblings": ["## Endpoints", "## Rate Limiting", "## Error Codes"],
  "format": "markdown"
}
```

For JSON:

```json
{
  "content": "{\n  \"serde\": \"1.0\",\n  \"tokio\": { ... }\n}",
  "line_range": [10, 85],
  "path": "$.dependencies",
  "type": "object",
  "count": 47,
  "format": "json"
}
```

**OutputBuffer integration:** If the extracted section exceeds 200 lines, it goes
through the buffer system and returns a `@file_*` ref — enabling progressive
drill-down: summary → section → buffer query.

## Implementation Strategy

### Files to modify

| File | Changes |
|------|---------|
| `src/tools/file_summary.rs` | Split `Config` into `Json`/`Yaml`/`Toml`; new summarizers with structural metadata |
| `src/tools/file.rs` (ReadFile) | Add params to `input_schema()`; dispatch to extractors in `call()` |
| `src/prompts/server_instructions.md` | Document new params and summary formats |

### New modules/functions

- `summarize_json(content) -> Value` — parse JSON, extract schema shape
- `summarize_yaml(content) -> Value` — parse YAML, extract top-level structure
- `summarize_toml(content) -> Value` — parse TOML, extract table structure
- `summarize_markdown(content) -> Value` — enrich with line ranges, all heading levels
- `extract_markdown_section(content, heading) -> Result<SectionResult>`
- `extract_json_path(content, path) -> Result<SectionResult>`
- `extract_toml_key(content, key) -> Result<SectionResult>`

### Dependencies

- `serde_json` — already present
- `toml` — already present
- `serde_yaml` — new dependency (lightweight, well-maintained)
- No external JSONPath library — implement the simple subset (dot + array index)

### Error handling

All navigation errors use `RecoverableError` (not fatal):
- Format mismatch: hint with the correct param name
- Path/heading not found: hint listing available options
- Malformed JSON/TOML/YAML: hint with parse error location

## Non-goals

- Full JSONPath spec (wildcards, filters, recursive descent)
- XML/HTML support (can be added later with same pattern)
- CSV/TSV support (can be added later)
- Modifying the OutputBuffer system itself
