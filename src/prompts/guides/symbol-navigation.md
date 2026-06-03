# Symbol Navigation

Per-language tips for codescout's symbol tools — `symbols`, `symbol_at`,
`references`, `call_graph`. When you know a name, navigate structurally
instead of grepping: `symbols` reads the AST/LSP index, so it finds
definitions and bodies precisely, where `grep` also matches comments,
strings, and unrelated identifiers.

## Generic patterns (any language)

- **Hierarchical nav** — a method on a class/struct/object, all languages:
  `symbols(name_path="Container/member", include_body=true)`. Use a bare
  name for top-level functions or types.
- **Find across the project, then read the body:**
  `symbols(name="edit_code")` to locate it, then
  `symbols(name_path="ToolName/edit_code", include_body=true)` for the body.
- **Kind filter + path scope:** `symbols(path="src/tools/", kind="struct")`.
  `kind` values vary by language (see table below). Run `symbols(path)` once on a
  representative file to see which kinds your LSP emits.
- **Who calls X?** `references(symbol, path)` — structured call sites, not
  `grep`.
- **Impact analysis before any structural change:**
  `call_graph(symbol, path, direction="callers")` traces blast radius;
  `direction="callees"` traces outbound flow (`max_depth` defaults to 3,
  capped at 10).
- **Name unknown?** Start with `semantic_search("what it does")`, then drill
  down with `symbols(name_path=...)`.

## Per-language quick reference

The generic patterns above already cover finding a method
(`symbols(name_path="Container/member", include_body=true)`), callers, and
impact analysis (`call_graph(symbol, path, direction="callers")` before any
structural change) — those are identical across languages. Only three things
vary per language: the `name_path` form, the `kind` to pass to
`symbols(path=..., kind=...)`, and a per-language gotcha.

| Language | `name_path` form | `kind` for list-by-kind | Gotcha |
|---|---|---|---|
| **Rust** | `Type/method`, `impl Trait for Type/method` | `struct` | rust-analyzer reports traits as `kind="interface"`; trait impls use the `impl Trait for Type/method` form |
| **Python** | `Class/method`, `module_func` | `class` | decorators aren't part of the symbol — search by the decorated function's name |
| **TS / JS** | `Class/method`, `exportedFunction` | `class`; `function` for arrow fns | React function components are `kind="function"`, not `class` |
| **Kotlin / Java** | `Class/method`, `Object.companion/method` | `class` (covers classes, objects, annotations) | annotations aren't part of the symbol — search by method name |
| **Go** | `Type/Method`, `PackageFunc` | `function` (covers funcs + methods) | interfaces use `kind="interface"`; receiver methods stay `Type/Method` |
| **C#** | `Class/Method`, `Namespace.Class/Method` (nested) | `class`, `interface` | properties surface as `function` getters/setters in some LSPs |

## Related

- `get_guide("progressive-disclosure")` — what to do when `symbols` output
  overflows to a `@tool_*` buffer.
- Iron Law 1 (`server_instructions`) — never `read_file` source; use
  `symbols(path)` for an overview, `symbols(name=..., include_body=true)` for
  bodies.
