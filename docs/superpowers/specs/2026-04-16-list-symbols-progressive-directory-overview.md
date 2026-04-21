# list_symbols: Progressive Directory Overview

**Date:** 2026-04-16
**Status:** Approved

## Problem

`list_symbols` on a non-root directory uses `max_depth(Some(1))` in the WalkBuilder, meaning
only immediate children are walked. Directories that contain only subdirectories (no source files
at depth 1) return an empty result with no hint — indistinguishable from a broken index or wrong
path.

Root cause: `src/tools/symbol.rs` ~line 724:
```rust
let is_project_root = rel_path == "." || rel_path.is_empty();
let walker = ignore::WalkBuilder::new(&full_path)
    .max_depth(if is_project_root { None } else { Some(1) })
    ...
```

The asymmetry was intended to "avoid dumping entire subtrees" but `cap_files` already handles
that. The `max_depth` guard is redundant and harmful.

## Solution: Budget-Aware Progressive Disclosure

Replace the `max_depth` asymmetry with a three-mode dispatch that adapts output shape to tree
size. The tool always recurses, but what it returns scales inversely with volume.

### Phase 0 — Fast Count Walk

Before any symbol parsing, enumerate files recursively (filesystem only, no LSP, no AST).
Produces:
- `total_files: usize`
- `subdirs: Vec<(String, usize)>` — meaningful subdirectories with file counts (see grouping rule below)

This walk is cheap: pure FS traversal, no parsing. Cost is O(n) stat calls.

**Grouping rule — collapse pass-through directories:**
Strict "immediate subdir" grouping produces useless single-entry maps on sparse hierarchies
(e.g. `kotlin/` → one entry `edu/`, not the useful `edu/planner/api`, `edu/planner/domain`).
Instead, collapse single-child directories until a meaningful branch point is found:

```
fn find_split_point(dir: &Path) -> &Path:
    immediate_subdirs = source_subdirs_at_depth_1(dir)
    direct_files      = source_files_at_depth_1(dir)
    if direct_files == 0 and immediate_subdirs.len() == 1:
        return find_split_point(immediate_subdirs[0])  // descend
    return dir  // branching here, or mixed dir+subdirs

fn count_files_by_subdir(root, dir):
    split = find_split_point(dir)
    group by immediate child of split
```

For `ktor-server/src/main/kotlin`:
- `kotlin/` → one child `edu/`, no direct files → descend
- `edu/` → one child `planner/`, no direct files → descend
- `planner/` → three children `api/`, `domain/`, `service/` → split here
- Result: `[api/(12), domain/(8), service/(15)]`

### Three Output Modes

| Total files | Mode | Returned |
|-------------|------|----------|
| ≤ 30 | `symbols` | existing behavior — full symbol tree, recursive |
| 31–80 | `class_overview` | immediate subdirs + AST class names per subdir |
| 81–200 | `directory_map` | immediate subdirs + file counts only |
| > 200 | `directory_map` (capped) | top 15 subdirs by file count + overflow hint |

**Edge case — flat directory (no subdirs):** if `count_files_by_subdir` returns an empty subdir
vec (all files sit directly in the target dir), skip the overview modes entirely and fall back to
`symbols` mode with the existing `cap_files` limit. A directory map with no subdirectories
conveys nothing useful.

### Constants

```rust
const LIST_SYMBOLS_RECURSE_SMALL:  usize = 30;
const LIST_SYMBOLS_RECURSE_MEDIUM: usize = 80;
const LIST_SYMBOLS_MAX_SUBDIRS:    usize = 15;
```

These join the existing constants at the top of `src/tools/symbol.rs`.

---

## Output Shapes

### `class_overview` mode (31–80 files)

Class names extracted via **tree-sitter AST only** — no LSP. Includes kinds: class, struct,
object, interface, enum (language-dependent). Names only, no signatures, no bodies.

```json
{
  "directory": "ktor-server/src/main/kotlin",
  "mode": "class_overview",
  "subdirectories": [
    {
      "path": "edu/planner/api",
      "file_count": 12,
      "classes": ["CourseController", "EnrollmentController", "PlannerApi"]
    },
    {
      "path": "edu/planner/domain",
      "file_count": 8,
      "classes": ["Course", "Student", "Enrollment"]
    }
  ],
  "total_files": 45,
  "hint": "Found 45 files across 3 directories — showing top-level classes (AST). Drill down with list_symbols('edu/planner/api') for full symbols, or list_symbols('ktor-server/src/main/kotlin/**/*.kt') to scan the full tree."
}
```

### `directory_map` mode (81–200 files)

```json
{
  "directory": "ktor-server/src/main",
  "mode": "directory_map",
  "subdirectories": [
    { "path": "kotlin/edu/planner/api",     "file_count": 12 },
    { "path": "kotlin/edu/planner/domain",  "file_count": 8  },
    { "path": "kotlin/edu/planner/service", "file_count": 15 }
  ],
  "total_files": 127,
  "hint": "Found 127 files across 8 directories — too large for symbol overview. Drill down with list_symbols('<subdir>') or use list_symbols('ktor-server/src/main/**/*.kt') to scan the full tree with file cap."
}
```

### `directory_map` mode (> 200 files, capped)

Same shape as above. Subdirs capped at `LIST_SYMBOLS_MAX_SUBDIRS` (15), sorted descending by
file count (largest subtrees first — most likely to be interesting).

```json
{
  "directory": "src",
  "mode": "directory_map",
  "subdirectories": [
    { "path": "src/tools",   "file_count": 42 },
    { "path": "src/lsp",     "file_count": 28 },
    ...
  ],
  "total_files": 312,
  "overflow": { "shown": 15, "total": 23, "hint": "..." },
  "hint": "Found 312 files across 23 directories — showing 15 largest. Drill down with list_symbols('<subdir>')."
}
```

---

## Code Changes

All changes in `src/tools/symbol.rs`. No new files, no new tools.

### 1. New constants

Add alongside existing constants (~line 499):
```rust
const LIST_SYMBOLS_RECURSE_SMALL:  usize = 30;
const LIST_SYMBOLS_RECURSE_MEDIUM: usize = 80;
const LIST_SYMBOLS_MAX_SUBDIRS:    usize = 15;
```

### 2. New helper: `count_files_by_subdir`

```rust
fn count_files_by_subdir(root: &Path, dir: &Path) -> (usize, Vec<(String, usize)>)
```

- Calls `find_split_point(dir)` to resolve the meaningful branch directory (collapse
  pass-through single-child dirs with no direct source files)
- Walks recursively from the split point (no depth limit, respects `.gitignore`, skips hidden)
- Counts only files where `ast::detect_language` returns `Some(_)`
- Groups by immediate subdirectory of the **split point** (not the original `dir`)
- Returns `(total, Vec<(display_path, count)>)` sorted descending by count
- Files directly in the split point are counted in total but not in the subdir vec

Helper `find_split_point(dir: &Path) -> PathBuf`:
- If `dir` has zero direct source files and exactly one immediate subdir → recurse into that subdir
- Otherwise → return `dir`
- Max recursion depth: 10 (guard against degenerate trees)
### 3. New helper: `ast_class_names_for_dir`

```rust
fn ast_class_names_for_dir(dir: &Path) -> Vec<String>
```

- Walks only `dir` (depth 1 — immediate files only, since this is called per subdir)
- Calls `ast::extract_symbols(file)` on each source file
- Filters to top-level symbols with kind in: `Class`, `Struct`, `Interface`, `Object`, `Enum`
- Returns deduplicated names, sorted

### 4. `ListSymbols::call` directory branch

Replace the current directory branch (~lines 718–850) with:

```
subdir_counts, total = count_files_by_subdir(root, full_path)

match total:
    0           → existing empty-result path (no source files found)
    1..=SMALL   → existing recursive symbols path (remove max_depth asymmetry)
    SMALL+1..=MEDIUM → class_overview: build subdirs with ast_class_names_for_dir per subdir
    _           → directory_map: build subdirs with counts, cap at MAX_SUBDIRS
```

The `symbols` mode path is the existing directory walk code with `max_depth(None)` replacing
the asymmetric check.

---

## What Does Not Change

- Glob path branch — unchanged
- File path branch — unchanged
- `depth`, `include_docs`, `scope`, `detail_level` params — all still apply in `symbols` mode
- `cap_files` — still used in `symbols` mode
- No new tool registered

## API Contract Change

**This is an API shape change, not just a behavior change.**

The directory branch response schema changes from:
```json
{ "directory": "...", "files": [...] }
```
to one of three shapes depending on mode:
```json
{ "directory": "...", "mode": "class_overview", "subdirectories": [...], "total_files": N, "hint": "..." }
{ "directory": "...", "mode": "directory_map",   "subdirectories": [...], "total_files": N, "hint": "..." }
{ "directory": "...", "files": [...] }  // symbols mode — unchanged
```

Callers parsing `result["files"]` will silently get `null` when the tool returns an overview
mode. **Server instructions must be updated** to tell agents about the new response shapes and
how to detect which mode was returned (check for `result["mode"]`).

### `force_mode` escape hatch

To prevent silent param-ignoring surprises, add a `force_mode` parameter:

| Value | Behaviour |
|-------|-----------|
| `"auto"` (default) | size-based mode selection |
| `"symbols"` | always return full symbol tree (existing behaviour, bypasses thresholds) |

When `force_mode: "symbols"` is set, `depth`, `include_docs`, and `detail_level` are always
honoured. Agents that need symbol output can opt out of the size-based switching explicitly.

Schema addition to `input_schema`:
```json
"force_mode": {
  "type": "string",
  "enum": ["auto", "symbols"],
  "description": "Override mode selection. 'symbols' forces full symbol output regardless of directory size."
}
```
## Tests

- `list_symbols_on_dir_with_only_subdirs_returns_directory_map` — the original bug case
- `list_symbols_small_tree_returns_symbols` — ≤30 files, existing behavior preserved
- `list_symbols_medium_tree_returns_class_overview` — 31–80 files, class names present
- `list_symbols_large_tree_caps_subdirs` — >200 files, subdirs capped at 15
- `list_symbols_class_overview_uses_ast_not_lsp` — mock LSP unavailable, classes still returned

Use existing fixture projects (kotlin-library, java-library) for realistic file counts.
