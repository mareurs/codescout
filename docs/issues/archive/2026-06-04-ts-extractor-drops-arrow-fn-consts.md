---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: high
owner: marius
related: [2026-06-04-ts-extractor-drops-namespace-abstract-class]
tags: [edit_code, ast, typescript, javascript, tree-sitter, arrow-function]
kind: bug
---

# BUG: TS/JS AST extractor drops `const X = () => {}` (function-valued bindings) → `edit_code` fails on them

# Summary
`extract_ts_symbols` had no arm for `lexical_declaration` / `variable_declaration`, so
function-valued bindings — `const X = () => {}`, `export const X = () => {}`,
`let X = function () {}` — never entered the symbol tree. This is the **dominant modern JS/TS
definition idiom** (React function components, exported handlers, hooks), so a large fraction of
real-world TS/JS symbols were invisible to `symbols`, `symbol_at`, and `edit_code`. Found by the
extractor-coverage audit.

# Symptom (Effect)
`symbols(path)` omits arrow/function-expression consts; `edit_code(symbol="App", …)` on a
`const App = () => {…}` component → `AST parse failed`, though tsserver resolves it.

# Reproduction
Verified 2026-06-04 via `extract_symbols_from_source` (unit harness):
```typescript
const handler = () => {};
export const Component = () => { return null; };
let legacy = function () {};
const COUNT = 5;            // data const — intentionally NOT extracted
const config = { a: 1 };    // object const — intentionally NOT extracted
```
Pre-fix: none of `handler`/`Component`/`legacy` extracted.
Post-fix: `handler`, `Component`, `legacy` extracted as Functions; `COUNT`/`config` still skipped.

# Environment
codescout (`experiments` HEAD), tree-sitter-typescript. Node kinds confirmed via a parse-tree dump:
`lexical_declaration > variable_declarator{name: identifier, value: arrow_function |
function_expression}`; non-function consts have `value` = `number`/`object`.

# Root cause
No `lexical_declaration` arm at all. (The same audit found `const` was never extracted in any
form — but only the function-valued case is a meaningful symbol; plain data consts are out of
scope by design, matching the tool's symbol-navigation focus.)

# Fix
**Implemented 2026-06-04 on `experiments`** (`src/ast/parser.rs`): added a
`lexical_declaration | variable_declaration` arm to `extract_ts_symbols` that iterates
`variable_declarator` children and extracts (as `Function`) only those whose `value` field is an
`arrow_function` or `function_expression`. Exported forms are reached via the existing
`export_statement` unwrap. Plain data consts are deliberately skipped.

**Verification (2026-06-04):** regression test below; clippy clean; full lib suite green
(2611 pass, 7 ignored, 0 fail). Live needs a `/mcp` restart.

# Tests added
- `ast::parser::tests::ts_arrow_function_consts_are_extracted` — `handler`, `Component`, `legacy`
  extracted; `COUNT`, `config` asserted **absent** (the by-design skip).

# Workarounds
- `edit_file` with explicit context (when not blocked by `debug_enforce_symbol_tools`).

# Resume
**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master. Ships alongside the other
extractor fixes; on landing `git mv` to `docs/issues/archive/` and cite the **master-side** SHA.
Possible follow-up: object-literal methods (`const obj = { m() {} }`) and class-expression consts
(`const C = class {}`) are still not extracted — lower priority than arrow components.

# References
- `src/ast/parser.rs` `extract_ts_symbols` (`lexical_declaration` arm).
- Sibling: `2026-06-04-ts-extractor-drops-namespace-abstract-class.md`.
