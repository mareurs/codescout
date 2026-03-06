# Memory Staleness Detection — Design

**Date:** 2026-03-06
**Status:** Approved
**Goal:** Automatically detect when code changes make project memories stale, surfaced via `project_status`.

## Problem

Markdown memories (architecture, conventions, onboarding) shape every codescout session. When the underlying code changes, these memories silently go stale — describing patterns that no longer exist or missing new ones. There's currently no link between memories and source files, so staleness is invisible until a human notices.

## Solution Overview

A dual-layer detection system using **path anchors** (explicit, user-curated file dependencies per memory) and **semantic anchors** (automatic embedding-based similarity tracking). Staleness is surfaced in `project_status` and flagged opportunistically during `index_project`.

### Priority

1. **Markdown memories** (architecture, conventions, etc.) — primary target
2. **Semantic memories** (remember/recall) — secondary, semantic anchors only

## Data Model

### Path Anchors — TOML Sidecar

Each markdown memory can have a companion file:

```
.codescout/memories/architecture.anchors.toml
```

```toml
# Source files this memory depends on.
# Edit this list to track additional files or remove irrelevant ones.
# codescout will warn when these files change significantly.

[[anchors]]
path = "src/server.rs"
hash = "a1b2c3..."

[[anchors]]
path = "src/tools/mod.rs"
hash = "d4e5f6..."
```

- User-editable, committed to repo
- Hashes are computed automatically; users only curate the `path` list
- One sidecar per markdown memory topic

### Semantic Anchors — Database Table

Stored in `embeddings.db`:

```sql
CREATE TABLE memory_anchors (
    id INTEGER PRIMARY KEY,
    memory_type TEXT NOT NULL,        -- 'markdown' | 'semantic'
    memory_key TEXT NOT NULL,         -- topic name or memory rowid
    file_path TEXT NOT NULL,          -- best-matching source file
    file_hash TEXT NOT NULL,          -- SHA-256 at anchor time
    similarity REAL NOT NULL,         -- similarity score at anchor time
    created_at TEXT NOT NULL,
    stale INTEGER NOT NULL DEFAULT 0, -- 0=fresh, 1=flagged by reverse drift
    UNIQUE(memory_type, memory_key, file_path)
);
```

No embedding column — we re-embed at check time (embeddings may change if the model changes). We store only the *result*: which file matched, at what similarity.

Section-level future extension: add optional `section TEXT` column (nullable, always NULL in v1).

## Write Path — Anchor Creation

Triggered when a memory is written (`memory(action="write")` or `memory(action="remember")`).

### For Markdown Memories

**Path anchors (TOML sidecar):**
1. Regex scan of memory content for file paths as a **seed**
2. Write to `.anchors.toml` — create if new, merge if existing (keep user-added paths, add new, don't remove user-curated)
3. Compute SHA-256 for each path that exists on disk
4. Paths that don't exist on disk are excluded from seeding

**Semantic anchors (database):**
1. Embed full memory content (1 embedding call)
2. `search_scoped()` against code chunk index — top 10
3. Filter: similarity ≥ 0.3 only
4. Deduplicate by file (keep highest-similarity chunk per file)
5. Exclude files already covered by path anchors
6. Insert into `memory_anchors` table

### For Semantic Memories

Semantic anchors only (same steps 1–6 above, `memory_type='semantic'`, `memory_key=rowid`). No TOML sidecar — these are granular and numerous.

### Cost Budget

- Path extraction: pure string scan, negligible
- Semantic anchoring: 1 embed + 1 vector search per memory — same as a single `semantic_search` call
- Runs only at memory write time (infrequent)

## Check Path — Staleness Detection in `project_status`

### Layer 1: Path Anchor Check (fast, deterministic)

For each markdown memory with an `.anchors.toml` sidecar:
1. Read TOML, iterate `[[anchors]]`
2. Compute current SHA-256 of each `path`
3. Compare against stored `hash`
4. Mismatch = stale. Missing file = flagged separately.
5. **Does NOT auto-update the hash** — stays stale until memory is re-written or explicitly refreshed

### Layer 2: Semantic Anchor Check (on-demand, embedding-based)

For each markdown memory:
1. Embed memory content
2. Search top-10 similar code chunks
3. Compare against stored semantic anchors:
   - File set drift: did the top matched files change?
   - Similarity drift: did scores drop significantly?
4. Flag if: top-file overlap drops below 50%, OR average similarity drops by > 0.15

**Layer 2 is skippable.** If no embedder is configured or the index doesn't exist, only Layer 1 runs. Path anchors alone are useful.

### Output Format

```json
{
  "memory_staleness": {
    "stale": [
      {
        "topic": "architecture",
        "reason": "3 of 5 anchored files changed",
        "changed_files": ["src/server.rs", "src/tools/mod.rs", "src/agent.rs"],
        "semantic_drift": 0.12
      }
    ],
    "fresh": ["conventions", "development-commands", "gotchas"],
    "untracked": ["onboarding"]
  }
}
```

- **stale**: path hash mismatches or significant semantic drift
- **fresh**: all anchors match
- **untracked**: no `.anchors.toml` sidecar
- `semantic_drift`: average similarity drop (omitted if no semantic anchors or no embedder)

## Reverse Drift Hook — Piggyback on Index Updates

Runs during `index_project`, after `compute_file_drift` for each changed file.

### Logic

```
for each file where drift > staleness_drift_threshold:
    1. Query memory_anchors WHERE file_path = ? → set stale=1
    2. Scan all .anchors.toml files for this file_path
       → if hash differs, record in lightweight cache
```

### Properties

- Stale flag is a **hint**, not authoritative — full check runs in `project_status`
- Does NOT update hashes in `.anchors.toml`
- Does NOT trigger any LLM action
- Cost: 1 SQL query per drifted file (most index runs have few high-drift files)

### Threshold

Configurable in `project.toml`:

```toml
[memory]
staleness_drift_threshold = 0.3
```

Below 0.3 drift = minor changes (whitespace, comments, trivial refactors). Above 0.3 = something meaningful shifted.

## Re-Anchoring — Clearing Staleness

### Path 1: Memory Re-Write (primary)

When `memory(action="write", topic="...")` is called:
1. New content written to memory file
2. TOML sidecar refresh: re-seed from content, merge with existing, recompute all hashes
3. Semantic anchors: delete old rows, create fresh from new embedding search
4. All stale flags cleared

### Path 2: Explicit Acknowledge

When the memory is still accurate but anchored files changed (internal refactor, pattern unchanged):

```
memory(action="refresh_anchors", topic="architecture")
```

Re-hashes all files in TOML and refreshes semantic anchors without touching memory content. The "I've reviewed this, it's still good" signal.

### Semantic Memories

No explicit re-anchor flow for v1. Semantic anchors created at `remember` time, checked passively. Natural similarity decay in `recall` results is self-correcting — stale memories surface less often.

## New Tool Surface

One new action on `memory` tool: `refresh_anchors(topic)`. No new tools.

## Configuration

```toml
[memory]
staleness_drift_threshold = 0.3   # min drift to trigger reverse-drift flag
semantic_anchor_min_similarity = 0.3  # floor for semantic anchor creation
semantic_anchor_top_n = 10        # chunks to consider for semantic anchoring
```

## Module Structure

```
src/memory/
├── mod.rs          # MemoryStore (existing)
├── classify.rs     # bucket classification (existing)
└── anchors.rs      # NEW: AnchorStore — TOML sidecar I/O, hash computation,
                    #       staleness check, merge logic, semantic anchor CRUD
```

Integration points:
- `src/tools/memory.rs` — wire `refresh_anchors` action, call anchor creation on write
- `src/tools/config.rs` (`project_status`) — call staleness check, include in output
- `src/embed/drift.rs` — reverse drift hook after `compute_file_drift`
- `src/tools/semantic.rs` (`index_project`) — trigger reverse drift scan

## Open Questions

1. **Anchor TOML in git?** Proposed yes (committed). Hashes are repo-specific but paths are shared knowledge. Alternative: gitignore the hashes, only commit paths.
2. **Batch embedding for Layer 2 check?** If there are many memories, we could batch embed all of them in one call. Worth it if > 5 memories need checking.
3. **Should `project_status` always run Layer 2?** Or only when Layer 1 finds nothing stale? Running both gives the fullest picture but costs embedding calls.
