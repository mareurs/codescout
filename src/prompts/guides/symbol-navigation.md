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
  `kind` values vary by language: `function`, `class`, `struct`, `interface`,
  `type`, `enum`, `module`, `constant`. Run `symbols(path)` once on a
  representative file to see which kinds your LSP emits.
- **Who calls X?** `references(symbol, path)` — structured call sites, not
  `grep`.
- **Impact analysis before any structural change:**
  `call_graph(symbol, path, direction="callers")` traces blast radius;
  `direction="callees"` traces outbound flow (`max_depth` defaults to 3,
  capped at 10).
- **Name unknown?** Start with `semantic_search("what it does")`, then drill
  down with `symbols(name_path=...)`.

## Rust

- **`name_path` form:** `Type/method`, `impl Trait for Type/method`.
- **Find a method:** `symbols(name_path="Service/handle", include_body=true)`.
- **List by kind:** `symbols(path="src/", kind="struct")` (also `"interface"` for traits).
- **Language note:** trait impls use `impl Trait for Type/method`; rust-analyzer reports traits as `kind="interface"`.
- **Before refactor:** `call_graph(symbol="Service/handle", path="src/service.rs", direction="callers", max_depth=3)`.

## Python

- **`name_path` form:** `Class/method`, `module_func`.
- **Find a method:** `symbols(name_path="Service/handle", include_body=true)`.
- **List by kind:** `symbols(path="src/", kind="class")`.
- **Language note:** decorators are not part of the symbol — search by the decorated function's name.
- **Before refactor:** `call_graph(symbol="Service/handle", path="src/service.py", direction="callers", max_depth=3)`.

## TypeScript / JavaScript

- **`name_path` form:** `Class/method`, `exportedFunction`.
- **Find a method:** `symbols(name_path="Service/handle", include_body=true)`.
- **List by kind:** `symbols(path="src/", kind="class")` for classes; `kind="function"` for arrow fns.
- **Language note:** React function components are `kind="function"`, not `kind="class"`.
- **Before refactor:** `call_graph(symbol="Service/handle", path="src/service.ts", direction="callers", max_depth=3)`.

## Kotlin / Java

- **`name_path` form:** `Class/method`, `Object.companion/method`.
- **Find a method:** `symbols(name_path="Service/handle", include_body=true)`.
- **List by kind:** `symbols(path="src/", kind="class")` (covers classes, objects, annotations).
- **Language note:** annotations are not part of the symbol — search by method name.
- **Before refactor:** `call_graph(symbol="Service/handle", path="src/Service.kt", direction="callers", max_depth=3)`.

## Go

- **`name_path` form:** `Type/Method`, `PackageFunc`.
- **Find a method:** `symbols(name_path="Service/Handle", include_body=true)`.
- **List by kind:** `symbols(path="./", kind="function")` (covers funcs and methods).
- **Language note:** interfaces use `kind="interface"`; receiver methods stay in `Type/Method` form.
- **Before refactor:** `call_graph(symbol="Service/Handle", path="service.go", direction="callers", max_depth=3)`.

## C#

- **`name_path` form:** `Class/Method`, `Namespace.Class/Method` for nested.
- **Find a method:** `symbols(name_path="Service/Handle", include_body=true)`.
- **List by kind:** `symbols(path="src/", kind="class")` (also `"interface"`).
- **Language note:** properties surface as `kind="function"` getters/setters in some LSPs.
- **Before refactor:** `call_graph(symbol="Service/Handle", path="src/Service.cs", direction="callers", max_depth=3)`.

## Related

- `get_guide("progressive-disclosure")` — what to do when `symbols` output
  overflows to a `@tool_*` buffer.
- Iron Law 1 (`server_instructions`) — never `read_file` source; use
  `symbols(path)` for an overview, `symbols(name=..., include_body=true)` for
  bodies.
