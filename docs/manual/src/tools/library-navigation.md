# Library Navigation

These two tools let you register, inspect, and semantically search third-party
library source code — directly from within your agent workflow, without leaving
the project.

All library access is **read-only**. Editing tools operate only on project code.

> **See also:** [Library Navigation](../concepts/library-navigation.md) — how
> auto-discovery works, the scope parameter, and when to navigate library source.

---

## Auto-discovery

The most common way libraries enter the registry is automatically: when an LSP
`symbol_at` request returns a path outside the project root (e.g. a Rust
crate in `~/.cargo/registry/`, a Python package in `.venv/`), codescout
walks the parent directories looking for a package manifest (`Cargo.toml`,
`package.json`, `pyproject.toml`, `go.mod`) and registers the library.

After auto-discovery, symbol navigation tools can follow references into library
code without any manual setup. Use the `scope` parameter to explicitly target
libraries in searches (see below).

---

## `scope` parameter

Once libraries are registered, pass the optional `scope` string to any symbol
or search tool:

| Value | What it searches |
|---|---|
| `"project"` (default) | Only project source code |
| `"lib:<name>"` | A specific registered library, e.g. `"lib:serde"` |
| `"libraries"` | All registered libraries |
| `"all"` | Project source + all libraries |

Tools that accept `scope`:
`symbols`, `symbols`, `references`, `semantic_search`

All results include a `"source"` field (`"project"` or `"lib:<name>"`) to
distinguish origin.

---

## `library`

Dispatched by `action`: `"list"` or `"register"`.

**Purpose:** Show all registered libraries, their root paths, and whether a
semantic index has been built for each. Use `library(action: list)`.
You can also register a new library manually with `library(action: register)`.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `action` | string | yes | — | `"list"` or `"register"` |
| `path` | string | for `register` | — | Root path of the library to register |
| `name` | string | no | — | Friendly name for the library (inferred from manifest if omitted) |

**Example (list):**

```json
{ "action": "list" }
```

**Output:**

```json
{
  "libraries": [
    {
      "name": "serde",
      "root": "/home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.195/",
      "indexed": false
    },
    {
      "name": "tokio",
      "root": "/home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/tokio-1.35.1/",
      "indexed": true
    }
  ],
  "total": 2
}
```

**Tips:**

- Libraries with `"indexed": false` support symbol navigation (LSP + tree-sitter)
  but not `semantic_search`. Run `index(action: build)` with the library's root path to add semantic search.
- The registry is stored in `.codescout/libraries.json`. You can inspect it
  directly if you need to edit or remove an entry.

---
## `list_libraries`

Backward-compatible alias for `library(action="list")`. The dedicated tool is
still registered; new code should prefer the action-dispatched form.

## `register_library`

Backward-compatible alias for `library(action="register")`. The dedicated tool
is still registered; new code should prefer the action-dispatched form.## Indexing a Library for Semantic Search

Once a library is registered (via `library(action: list)` or auto-discovery), build its
semantic index by pointing `index(action: build)` at its root:

```json
{
  "tool": "index",
  "arguments": { "action": "build", "scope": "lib:serde" }
}
```

After indexing, `semantic_search` with `scope: "lib:<name>"` searches within that library:

```json
{
  "tool": "semantic_search",
  "arguments": { "query": "channel with backpressure", "scope": "lib:tokio" }
}
```

**Tips:**

- Only index libraries you actively need to search semantically. LSP symbol
  navigation (`symbols`, `symbols`) works without indexing.
- Indexing a large library (e.g. `tokio`) may take a few minutes on the first
  run. The library path is shown in `library(action: list)` output.
