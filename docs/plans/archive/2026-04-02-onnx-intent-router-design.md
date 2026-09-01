---
status: archived
---
# ONNX Intent Router — Unified Tool Surface Design

**Date:** 2026-04-02
**Status:** Draft — blocked on Phase 0 (tool fixes + clean data collection)
**Branch:** experiments

> **PHASE 0 SHIPPED BY OTHER MEANS; THE ROUTER WAS NEVER BUILT — archived 2026-09-02.**
>
> Verified at the bytes on 2026-09-02: no `ort` / `onnxruntime` dependency in any manifest,
> and none of the proposed unified tools (`navigate`, `explore`, `edit`, `run`, `project`)
> exists. The classifier, the pipeline templates and the misclassification handling are all
> unbuilt.
>
> **But §*Phase 0* happened, by hand, over the following months — every tool this spec
> called dead or broken is gone:**
>
> | this spec's Phase 0 target | what actually happened |
> |---|---|
> | `goto_definition`, `hover` (0 and 1 calls) | folded into `symbol_at(path, line, fields=["def","hover"])` |
> | `rename_symbol`, `replace_symbol`, `remove_symbol`, `insert_code` | folded into `edit_code(action="rename"\|"replace"\|"remove"\|"insert")` |
> | `find_references` (broken on non-Rust) | now `references` |
> | `find_symbol` + `list_symbols` | now one `symbols` tool, overview or search by argument shape |
>
> **And the shape it argued for arrived without the ML.** This spec's premise was that an
> `action=`-dispatched surface matches the model's mental model better than one tool per
> operation. That is now how `workspace`, `index`, `library`, `artifact` and `librarian`
> all work — reached by hand, with no classifier, no `ort`, and no training data.
>
> So the part that was never built is precisely the speculative part: **the learned router.**
> Its §*Gate* was never met either — Phase 1 required a week of clean usage data and a
> new analysis in `docs/research/`, and no such analysis exists.
>
> **Re-open trigger, if any:** evidence that tool *selection* (not tool quality) is a
> measurable error source after the consolidation above. `docs/trackers/tool-usage-patterns.md`
> is where that evidence would appear, and `docs/trackers/design-backlog-session-log.md`
> already records one check of this spec's premise.

## Problem Statement

codescout exposes 27 MCP tools to LLMs. This creates three compounding problems:

1. **Token overhead** — 27 tool schemas consume significant context window per MCP session
2. **Tool selection errors** — LLMs frequently pick the wrong tool (e.g. `read_file` on source code instead of `find_symbol`/`list_symbols`)
3. **Agent barrier** — less sophisticated LLM clients (smaller models, non-Claude agents) struggle with large tool menus

## Current State (2026-04-02 usage analysis)

Before designing the router, the underlying tools must be healthy. They are not:

### Broken Tools (must fix)

| Tool | Error Rate | Issue |
|---|---|---|
| `edit_markdown` | 36-38% | Consistent across projects — tool logic bug |
| `edit_file` | 15-35% | Regression from 9.3% all-time |
| `remove_symbol` | 14-91% | 90.9% failure on non-Rust LSPs |
| `replace_symbol` | 7-33% | 32.6% failure on non-Rust LSPs |
| `insert_code` | 6-25% | 25% failure on non-Rust LSPs |
| `find_references` | 0-100% | 3/3 failed on non-Rust, 1/1 succeeded on Rust |
| `read_file` | 6-10% | Partially source-file blocks, partially genuine failures |

### Dead Tools (candidates for removal)

| Tool | 7-day calls | Notes |
|---|---|---|
| `goto_definition` | 0 | Zero usage across both projects |
| `hover` | 1 | Effectively unused |
| `rename_symbol` | 0 | Zero usage |
| `find_references` | 4 | Near-zero and broken |

### Healthy Tools

| Tool | Calls | Error Rate |
|---|---|---|
| `grep` | 556 | 0.0% |
| `find_symbol` | 1,189 | 1.5% |
| `run_command` | 1,349 | 2.3% |
| `semantic_search` | 99 | 0.0% |
| `memory` | 183 | 3.8% |
| `create_file` | 61 | 0.0% |
| `list_dir` | 178 | 0.0% |
| `read_markdown` | 499 | 3.0% |

## Phase 0: Fix, Prune, Collect (prerequisite — do this first)

**This phase must complete before any router work begins.**

### 0a. Fix Broken Tools

Priority order:
1. `edit_markdown` — P0, broken everywhere, 36-38% error rate
2. `edit_file` — P1, regression from 9.3% to 35%
3. Structural edit tools on non-Rust LSPs (`replace_symbol`, `remove_symbol`, `insert_code`) — P2
4. `find_references` — P2, fix for non-Rust LSPs
5. `read_file` — P3, investigate genuine failures vs intentional blocks

### 0b. Remove or Consolidate Dead Tools

Evaluate for removal:
- `goto_definition` — 0 calls. Can its functionality be absorbed by `find_symbol` with a flag?
- `hover` — 1 call. Same question.
- `rename_symbol` — 0 calls. Keep if fix is trivial, remove if fundamentally broken.

Decision on each tool made after investigation — some may be dead because of prompt surfacing issues rather than being genuinely useless. Fix prompt guidance first, then measure again.

### 0c. Collect Clean Usage Data

After fixes and removals are deployed:
- Run for **1 week** across at least 2 active projects (codescout + backend-kotlin)
- Collect clean `usage.db` with fixed tools
- Re-run the usage analysis to establish new baselines
- This clean dataset becomes the ground truth for classifier training

**Gate:** Phase 1 begins only when:
- All fixed tools have <10% error rate across projects
- 1 week of clean usage data collected
- New usage analysis written to `docs/research/`

## Solution

Replace the reduced tool surface with **6 intent-driven endpoints**. Tools the LLM calls with **known targets** (navigate, edit) use **structured parameters** — deterministic dispatch, no classifier. The **exploratory** tool (`explore`) uses an embedded ONNX classifier to route free-form queries. A file-type gate enforces hard rules across the surface.

### Design Principle: Match the LLM's Mental Model

The LLM is already an intent classifier. When it calls `find_symbol(symbol="Book", include_body=true)`, it has structured intent — forcing it through a natural-language query and back through a classifier is a round-trip through ambiguity. The classifier is most valuable where the LLM **genuinely doesn't know** which internal tool to use — exploratory, concept-level queries.

| LLM thinking | Tool | Interface |
|---|---|---|
| "Show me the Book struct" | `navigate` | **Structured** — I know what I want |
| "How does authentication work here?" | `explore` | **Free-form** — I'm investigating |
| "Replace the fmt method body" | `edit` | **Structured** — I know what to change |
| "Run the tests" | `run` | **Structured** — direct command |
| "Remember that auth uses JWT" | `memory` | **Structured** — existing interface |
| "Activate the other project" | `project` | **Structured** — config operation |

## Approach

**Approach A (Classifier-First)** scoped to `explore`, with **Approach B (Embedding Similarity)** as bootstrap:

- **v0:** Embedding similarity routing for `explore` — works immediately with existing infrastructure.
- **v1:** Trained ONNX classifier replaces embedding routing when it reaches >90% top-1 accuracy on held-out clean usage data.
- Embedding path becomes the fallback for low-confidence classifier outputs.
- `navigate`, `edit`, `run`, `memory`, `project` — no classifier, deterministic dispatch from structured params.

## Exposed Tool Surface

6 tools replace the (post-pruning) tool set:

### `navigate`

Navigate and read code — symbol lookups, type info, directory listings. The LLM has a **specific target** in mind.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `symbol` | string? | one of symbol/path | Symbol name or qualified path (e.g. `"Book"`, `"CodeScoutServer/from_parts"`) |
| `path` | string? | one of symbol/path | File or directory path |
| `action` | string? | no | `"definition"` (default), `"references"`, `"hover"`, `"list"`, `"goto"` |
| `kind` | string? | no | Filter by symbol kind: `"function"`, `"struct"`, `"trait"`, etc. |
| `include_body` | bool? | no | Include full symbol body (default false) |
| `detail` | string? | no | `"compact"` (default) \| `"full"` |
| `offset`/`limit` | u64? | no | Pagination |

**Dispatch rules (deterministic, no classifier):**

| Params provided | Internal tool |
|---|---|
| `symbol` (no action or action=definition) | FindSymbol |
| `symbol` + `action="references"` | FindReferences |
| `symbol` + `action="hover"` | Hover |
| `symbol` + `action="goto"` | GotoDefinition |
| `path` to file + `action="list"` | ListSymbols |
| `path` to directory | ListDir |
| `path` with glob pattern | Glob |
| `path` to config/data file | ReadFile |

**Note:** The dispatch table above includes tools that are currently dead/broken (GotoDefinition, Hover, FindReferences). Their inclusion here depends on Phase 0 outcomes — tools that are removed in 0b are dropped from this table. Tools that are fixed in 0a are kept.

**File-type gate:** `path` pointing to source files (`.rs`, `.ts`, `.py`, `.java`, `.kt`, `.go`, etc.) **never** routes to ReadFile. Overrides to ListSymbols (for files) or FindSymbol (if `symbol` also provided).

**Examples of natural usage:**
```
navigate(symbol="Book", include_body=true)              → FindSymbol
navigate(symbol="route_tool_error", action="references") → FindReferences
navigate(path="src/tools", action="list")                → ListSymbols
navigate(path="Cargo.toml")                              → ReadFile
```

### `explore`

Free-form investigation — the LLM is **searching, not navigating**. This is where the ONNX classifier routes and pipelines orchestrate.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | Free-form: "authentication workflow", "error handling patterns", "files matching *.rs in src/tools" |
| `scope` | string? | no | `"project"` (default) \| `"libraries"` \| `"all"` |
| `route_hint` | string? | no | Bypass classifier: `"semantic_search"`, `"grep"`, etc. |

**Internal routes:** Determined by Phase 0 outcomes. Expected: SemanticSearch, Grep, Glob, plus any navigation tools that prove useful for exploratory chaining (e.g. FindReferences if fixed).

**Adaptive behavior:** Simple intents → single-dispatch. Complex/exploratory intents → pipeline orchestration (see Pipeline Orchestration section).

**Examples of natural usage:**
```
explore(query="how does the output buffer work")         → SemanticSearch (or pipeline)
explore(query="TODO comments", route_hint="grep")        → Grep
explore(query="authentication workflow")                 → pipeline (if complexity > threshold)
```

### `edit`

Modify code — structural changes, renames, insertions, removals. The LLM has a **specific target and change** in mind.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `action` | string | yes | `"replace"`, `"insert"`, `"remove"`, `"rename"`, `"create"`, `"edit"` |
| `path` | string | yes | Target file path |
| `symbol` | string? | yes for replace/remove/rename | Qualified symbol name (e.g. `"MyStruct/my_method"`) |
| `content` | string? | yes for replace/insert/create/edit | New code |
| `new_name` | string? | yes for rename | New symbol name |
| `position` | string? | for insert | Where to insert: `"after:SymbolName"`, `"before:SymbolName"`, `"line:42"` |

**Dispatch rules (deterministic, no classifier):**

| Action | Internal tool |
|---|---|
| `replace` | ReplaceSymbol |
| `insert` | InsertCode |
| `remove` | RemoveSymbol |
| `rename` | RenameSymbol |
| `create` | CreateFile |
| `edit` | EditFile (imports, comments, literals, config) |

**Note:** `rename` and `remove` depend on Phase 0 fixes. If a tool remains unreliable after fixes, the corresponding action returns an error with guidance rather than silently failing.

**File-type gate:** `action="edit"` on source files with structural changes (definition keywords detected) returns an error directing to `action="replace"` or `action="insert"` instead.

### `run`

Execute shell commands. Direct passthrough — no classifier needed.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `command` | string | yes | Shell command |
| `working_dir` | string? | no | Working directory |
| `timeout` | u64? | no | Timeout in ms |

**Internal route:** RunCommand

### `memory`

Project memory — remember/recall/forget knowledge. Keeps its current structured interface.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `action` | string | yes | `"read"`, `"write"`, `"list"`, `"delete"`, `"remember"`, `"recall"`, `"forget"` |
| `topic` | string? | contextual | For read/write/delete |
| `content` | string? | for write/remember | Content to store |
| `query` | string? | for recall | Search query |
| `bucket` | string? | for remember/recall | `"code"`, `"system"`, `"preferences"`, `"unstructured"` |

**Internal route:** Memory (direct passthrough)

### `project`

Project configuration, indexing, and library management.

**Parameters:**

| Param | Type | Required | Description |
|---|---|---|---|
| `action` | string | yes | `"activate"`, `"status"`, `"index"`, `"index_status"`, `"list_libraries"`, `"register_library"` |
| `path` | string? | for activate | Project path |
| `read_only` | bool? | for activate | Read-only mode |
| `name` | string? | for register_library | Library name |

**Dispatch rules:**

| Action | Internal tool |
|---|---|
| `activate` | ActivateProject |
| `status` | ProjectStatus |
| `index` | IndexProject |
| `index_status` | IndexStatus |
| `list_libraries` | ListLibraries |
| `register_library` | RegisterLibrary |

### Response Shape

**For `navigate`, `edit`, `memory`, `project`, `run`** — standard responses, same as current internal tools. No routing metadata needed since dispatch is deterministic.

**For `explore`** — enriched with routing metadata:

```json
{
  "result": "<tool-specific output>",
  "routed_via": ["semantic_search", "find_symbol"],
  "pipeline": true,
  "template": "EXPLORE_CONCEPT",
  "confidence": 0.87,
  "hint": "narrow with scope='libraries' to include dependencies"
}
```

## Classifier Architecture

The ONNX classifier is scoped to `explore` only. The other 5 tools use deterministic dispatch.

### Model Design

**Task:** Multi-label classification with confidence scores + complexity signal. Input is the `query` string from `explore`.

```
Input text → Tokenizer → Embedding (reuse existing model) → Classification head
                                                           ├─ tool_probs: [N] (per-label sigmoid)
                                                           └─ complexity: [1]  (sigmoid)
```

**Classification targets:** Determined after Phase 0. The label set will be the exploration-relevant tools that are healthy (error rate <5%) and have meaningful usage volume after the fix+prune cycle. Expected minimum: SemanticSearch, Grep, Glob.

- **Embedding backbone:** Reuse existing embedding model (powers SemanticSearch). Frozen during initial training — only the classification head is trained. Fine-tune backbone later with sufficient real data.
- **Classification head:** 2-layer MLP. `embed_dim → 128 → N+1`. Size depends on final label count. Uses **per-label sigmoid** (not softmax) — multiple tools can have high probability simultaneously, required for pipeline template selection.
- **Complexity head:** Sigmoid output. Threshold at 0.5 — above triggers pipeline orchestration.

### Inference via `ort`

```
codescout binary
  └─ ort::Session (loaded lazily on first `explore` call)
       ├─ model.onnx (classification head only, ~50-100KB)
       └─ tool_labels.json (maps indices → internal tool names)
```

- Lazy loading: model loads on first `explore` call, not at startup.
- CPU-only inference, sub-5ms for classification head on cached embeddings.
- Model location: `~/.codescout/models/`
- **Version compatibility:** The ONNX model embeds a version tag and label count. On mismatch, falls back to embedding-only routing and logs a warning.

### Latency Budget

Full routing path budget for `explore`:

| Step | Target | Notes |
|---|---|---|
| Query embedding | <20ms | Cached if same model as SemanticSearch |
| Classification | <5ms | MLP on CPU |
| File-type gate | <1ms | String match |
| Dispatch + internal tool | existing latency | No change |
| **Total overhead** | **<30ms** | vs 0ms for direct tool call today |

`navigate`, `edit`, `memory`, `project`, `run` — zero routing overhead.

Pipeline calls: target <200ms for a 3-step pipeline.

**ONNX model size:** Classification head only (~50-100KB). Does **not** bundle the embedding backbone — reuses the existing model loaded for SemanticSearch.

### Embedding Similarity Baseline (v0)

Before the classifier is trained, `explore` routing uses cosine similarity:

```
query_embedding = embed(query)
scores = [cosine(query_embedding, tool_embedding) for tool in exploration_tools]
route to argmax(scores)
```

Tool embeddings pre-computed from each tool's description + 10-20 canonical example queries. Stored in a vec0 table (existing sqlite-vec infrastructure).

**Confidence threshold:** Minimum cosine similarity of 0.4 required. Below threshold → error with hint to use `route_hint` or switch to `navigate`.

### File-Type Gate

Hard gate applied across the entire surface:

```
navigate(path=source_file) → never ReadFile → ListSymbols/FindSymbol
edit(action="edit", path=source_file, structural=true) → error, use action="replace"
explore: classifier output → gate check → actual tool
```

For `explore`, gate overrides logged to `usage.db` as retraining signal.

## Pipeline Orchestration

When the classifier signals `complexity > 0.5` on an `explore` call, the router assembles a predefined pipeline template.

### Pipeline Templates

**Note:** Templates involving tools that are broken/removed in Phase 0 will be adjusted. The templates below represent the target state assuming all tools are healthy.

```
EXPLORE_CONCEPT:
  SemanticSearch(query) → top_hits
  FindSymbol(top_hits[0..3], include_body=true) → symbols
  FindReferences(symbols[0]) → call_sites
  Output: { concepts, definitions, usage }

TRACE_CALL_GRAPH:
  FindSymbol(query) → target
  FindReferences(target) → callers
  GotoDefinition(callers[0..5]) → definitions
  Output: { target, callers, definitions }

EXPLORE_FILE_AREA:
  Glob(pattern) → files
  ListSymbols(files[0..5]) → symbol_map
  Output: { files, symbol_map }

UNDERSTAND_SYMBOL:
  FindSymbol(name, include_body=true) → definition
  Hover(name) → type_info
  FindReferences(name) → usage
  Output: { definition, type_info, usage_count, top_call_sites }
```

Templates that depend on currently-broken tools (FindReferences, Hover, GotoDefinition) are deferred until those tools pass Phase 0 gates. EXPLORE_FILE_AREA works with healthy tools today.

### Template Selection

Based on classifier multi-label output:

| Top tools in output | Template |
|---|---|
| {SemanticSearch, FindSymbol, FindReferences} | EXPLORE_CONCEPT |
| {FindSymbol, FindReferences, GotoDefinition} | TRACE_CALL_GRAPH |
| {Glob, ListSymbols} | EXPLORE_FILE_AREA |
| {FindSymbol, Hover, FindReferences} | UNDERSTAND_SYMBOL |
| None matched | Single-dispatch to argmax |

**Matching semantics:** A template triggers when its required tool set is a **subset** of the top-K classifier outputs (K=5, threshold > 0.3 per-label). Priority follows the order listed above.

### Execution Engine

```rust
struct Pipeline {
    steps: Vec<PipelineStep>,
}

struct PipelineStep {
    tool: InternalTool,
    input_source: InputSource,  // FromQuery | FromPrevious(step_idx, field)
    max_fan_out: usize,         // cap items flowing to next step
}
```

Rules:
- **Fan-out cap** — each step limits results flowing forward (default 3-5).
- **Early termination** — empty step result → skip downstream, return partial with hint.
- **Budget-aware** — total output respects OutputGuard caps.
- **Parallel where possible** — independent steps run via `tokio::join!`.

## Misclassification Handling

Applies only to `explore` — the other tools have deterministic dispatch.

### Transparent Routing Metadata

Every `explore` response includes `routed_via` and `confidence`. The LLM can detect misroutes and retry.

### Explicit Override

`route_hint` on `explore` bypasses the classifier entirely.

### Escape to Structured Tools

If `explore` consistently misroutes, the LLM switches to `navigate` with exact params. The structured tools are always available as a reliable fallback.

### Misclassification Detection

Three signals, logged to `misclassifications` table in `usage.db`:

1. **Explicit** — LLM retries `explore` with `route_hint` → first call was likely misrouted
2. **Implicit** — LLM calls `explore` twice quickly with rephrased query → first result likely wrong
3. **Gate overrides** — file-type gate overrode classifier output → classifier was wrong

All three feed into retraining data.

## Training Data Strategy

### Source: Clean Usage Data (post-Phase 0)

```
usage.db (collected during Phase 0c — 1 week of clean data)
  → filter to exploration-relevant tools with <5% error rate
  → label: (query_text, tool_name, was_part_of_chain)
```

This is clean data from fixed tools — no need for the correction/relabeling pipeline that would have been required with current broken data. The exact label set and volume distribution will be known after Phase 0c completes.

### Source: Synthetic Augmentation

For each exploration label, generate ~200-300 natural-language queries. Focus on underrepresented tools — current data shows agents heavily favor `grep` over `semantic_search` even when semantic would be better. Synthetic examples must counterbalance this bias.

Expected focus areas:
- **SemanticSearch** — concept-level queries agents currently misroute to grep
- **Glob** — file discovery queries
- **Other labels** — determined by Phase 0 outcomes

### Validation

Hold out 20% of clean usage data. Metrics:
- Top-1 accuracy (single-dispatch) — target >90%
- SemanticSearch routing rate — target: significant increase from current 1.6%
- `route_hint` bypass rate (lower is better)
- `navigate` fallback rate (lower is better)

## Integration with Existing Architecture

### Server Structure Change

```
CodeScoutServer
  ├─ exposed_tools: Vec<Arc<dyn Tool>>                    // 6 tools
  ├─ internal_tools: HashMap<String, Arc<dyn Tool>>       // N tools (post-pruning count)
  └─ explorer_router: Arc<ExplorerRouter>                 // classifier + pipeline (explore only)
```

The `Tool` trait is unchanged. Surviving internal tools keep their implementations. The 6 new tools are facades: 5 with deterministic dispatch, 1 (`explore`) delegating to `ExplorerRouter`.

### New Modules

```
src/
  router/
    mod.rs            — ExplorerRouter struct, public API
    classifier.rs     — ONNX model loading, inference, embedding fallback
    pipeline.rs       — Pipeline execution engine
    templates.rs      — Template definitions
  tools/
    unified.rs        — 6 exposed tool structs (Navigate, Explore, Edit, Run, Memory, Project)
    ... (existing files unchanged, minus removed tools)
```

### Prompt Surface Updates

All three surfaces rewritten for the 6-tool interface:
- `server_instructions.md` — describe 6 tools, structured vs free-form distinction
- `onboarding_prompt.md` — simpler (6 tools, clear when-to-use-which guidance)
- `build_system_prompt_draft()` — reference unified surface

Bump `ONBOARDING_VERSION`.

### Backward Compatibility

- `navigate` structured params provide equivalent access to all read/navigation tools
- `edit` structured params provide equivalent access to all mutation tools
- `explore` with `route_hint` provides direct access to any exploration tool

### Companion Plugin Implications

With the file-type gate built into the server, the companion plugin's `PreToolUse` hooks become redundant for the new surface. However, they remain useful during transition.

### Model Distribution

```
~/.codescout/
  models/
    explorer_v1.onnx          — classifier model (~50-100KB)
    tool_embeddings.bin        — precomputed tool embeddings (v0 fallback)
    labels.json                — index-to-tool mapping
```

Ships as companion asset in GitHub Releases. Falls back to embedding-only if model missing.

## Phasing

### Phase 0: Fix, Prune, Collect (DO THIS FIRST)

- **0a.** Fix broken tools: edit_markdown, edit_file, structural edit tools on non-Rust LSPs, find_references, read_file
- **0b.** Evaluate and remove/consolidate dead tools: goto_definition, hover, rename_symbol
- **0c.** Deploy fixes, collect 1 week of clean usage data across 2+ projects
- **0d.** Write new usage analysis, establish baselines
- **Gate:** All fixed tools <10% error rate, 1 week clean data collected

### Phase 1: Foundation

- Add `src/router/` module structure
- Implement file-type gate (shared by navigate + edit)
- Implement embedding similarity baseline (v0 explorer router) using clean tool embeddings
- Add 6 exposed tool structs in `src/tools/unified.rs`
- Wire into `CodeScoutServer`
- Update all 3 prompt surfaces, bump `ONBOARDING_VERSION`

### Phase 2: Classifier

- Train on clean usage data + synthetic augmentation
- Classification head: frozen backbone → MLP → N+1 labels
- Export to ONNX, integrate via `ort`
- Validate: >90% top-1 on held-out clean data
- Swap in as primary router

### Phase 3: Pipelines

- Implement pipeline templates (scoped to healthy tools)
- Pipeline execution engine
- Complexity head training
- Integration tests per template

### Phase 4: Feedback Loop

- Misclassification logging
- Periodic retraining
- Model versioning + auto-update

### Phase 5: Companion Plugin Simplification

Once router gates are proven stable, revisit `codescout-companion` plugin.

**Entry criteria:** Gate override rate below 2% for 30+ days across 3+ active projects.
