# Tool Hint Improvements Design

**Date:** 2026-05-02  
**Status:** Approved  
**Scope:** Advisory hints in `grep`, `read_file`, and `run_command` source guard

---

## Problem

Three friction points observed in session `214c9c7d` (2026-05-02):

1. `grep(pattern="WriteMemory|ReadMemory|...")` used to find symbol declarations — no hint that `symbols`/`references`/`call_graph` would be more structured
2. `read_file` on small source files returns content silently — no hint toward symbol tools (overflow hint exists for large files, but silent below the cap)
3. `run_command` source-file guard blocks `grep <identifier> *.rs` but hints only "use grep(regex)" — misses the 3-tier symbol ladder

The `call_graph` Impact Analysis workflow is documented in `server_instructions.md` (line 229) but never surfaced dynamically in tool responses. Agents only reach for it if they remember.

---

## Design

### 1. `grep` — identifier-pattern hint

**Where:** `impl Tool for Grep / call`, after the result is assembled, before return.

**Heuristic:** Pattern is identifier-like when it matches:
```
^[A-Za-z_]\w*(\|[A-Za-z_]\w*)*$
```
i.e. a single identifier or pipe-alternation of identifiers, no regex metacharacters.

**Behaviour:** Append a `suggestion` field to the result JSON:
```json
{
  "matches": [...],
  "total": N,
  "suggestion": "Pattern looks like a symbol name. Consider: symbols(name='X') for declarations, references(symbol='X') for direct callers, call_graph(symbol='X', direction='callers') for transitive blast radius."
}
```

`suggestion` is only added when the heuristic fires. Existing `overflow`, `mode`, and `reason` fields are unaffected. Result is always returned — this is advisory, never blocking.

For pipe-alternation patterns (`A|B|C`), the suggestion uses the first alternative as the example name.

---

### 2. `read_file` — source file hint (all sizes)

**Where:** `read_full_file`, after building the inline result (the branch that currently returns without a hint for small files).

**Behaviour:** For source files (detected via `detect_file_type == Source`), always append a `hint` field:
```json
{
  "content": "...",
  "total_lines": N,
  "hint": "Source file. For overview: symbols(path). For a specific function: symbols(name='...', include_body=true)."
}
```

The overflow branch already emits a similar message — this makes the hint unconditional for source, regardless of file size.

Non-source files (config, markdown, JSON, generic) are unchanged.

---

### 3. `run_command` source guard — upgraded grep hint

**Where:** `check_source_file_access` in `src/util/path_security.rs`, the `match first_cmd` arm that currently handles `sed | awk`.

**Behaviour:** Add a `grep` arm. When the blocked command is `grep` AND the pattern argument is identifier-like (reuse same heuristic, extracted as a shared `fn is_identifier_pattern(s: &str) -> bool`), emit:

```
use symbols(name='X') for declarations, references(symbol='X') for direct callers,
call_graph(symbol='X', direction='callers') for transitive blast radius.
Re-run with acknowledge_risk: true if you need raw shell grep.
```

When the pattern is not identifier-like (genuine regex), fall through to the existing generic hint.

---

## Shared helper

Extract the identifier-pattern check as a small free function in `src/util/path_security.rs` (already imported by both grep and run_command):

```rust
/// Returns true if `pattern` is a plain identifier or pipe-alternation of identifiers.
pub fn is_identifier_pattern(pattern: &str) -> bool { ... }
```

`grep.rs` calls it. `check_source_file_access` calls it for the grep arm. No new module needed.

---

## Files touched

| File | Change |
|------|--------|
| `src/tools/grep.rs` | Add `suggestion` field when `is_identifier_pattern` fires |
| `src/tools/read_file.rs` | Add `hint` field for all source file inline reads |
| `src/util/path_security.rs` | Add `is_identifier_pattern` helper + grep arm in `check_source_file_access` |
| `src/tools/grep.rs` (tests) | Test: suggestion present for identifier pattern, absent for regex pattern |
| `src/tools/read_file.rs` (tests) | Test: hint present for small source file, absent for non-source |
| `src/util/path_security.rs` (tests) | Test: grep-with-identifier blocked with 3-tier hint; grep-with-regex uses generic hint |

No changes to `server_instructions.md`, `onboarding_prompt.md`, or `ONBOARDING_VERSION` — the Impact Analysis workflow already documents `call_graph` correctly. These changes make the runtime surface what the docs already say.

---

## What this does NOT change

- `grep` is never blocked — purely advisory
- `read_file` source gate is unchanged (currently permissive — source reads allowed)
- No new abstraction or module
- No changes to `call_graph`, `references`, or `symbols` tools themselves
- `run_command` guard escape hatch (`acknowledge_risk: true`) unchanged
