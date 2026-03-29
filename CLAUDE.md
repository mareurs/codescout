# codescout

Rust MCP server giving LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Inspired by [Serena](https://github.com/oraios/serena).

You are a proficient Rust developer. You follow all known good/scalable patterns. You are honest and recognize your limits and your mistakes, you own them. If you are not sure, you always ask me for feedback.

## Development Commands

```bash
cargo build                        # Build (dev)
cargo build --release              # Build release binary (required before testing via MCP)
cargo test                         # Run tests
cargo clippy -- -D warnings        # Lint
cargo fmt                          # Format
cargo run -- start --project .     # Run MCP server (stdio)
cargo run -- index --project .     # Build embedding index
```

**Always run `cargo fmt`, `cargo clippy`, and `cargo test` before completing any task.**

**To test changes via the live MCP server, always run `cargo build --release` first**, then restart the server with `/mcp`. The MCP server runs the release binary — dev builds are not picked up.

## Tool Misbehavior Log — MANDATORY

**`docs/TODO-tool-misbehaviors.md` is a living document. You MUST maintain it.**

- **Before starting any task**, read it to know current tool limitations.
- **While working**, watch for: wrong edits, corrupt output, silent failures, misleading errors from codescout's own MCP tools.
- **When you notice anything unexpected**, add an entry to that file **before continuing** — even a one-liner. Capture: what you did, what you expected, what happened, and a probable cause.
- Do not wait until you finish the task. Log it immediately while context is fresh.

This applies to ALL unexpected tool behavior: `edit_file`, `rename_symbol`, `replace_symbol`, `find_symbol`, `semantic_search`, etc.


## Git Workflow

**This is a public repo.** Do not push incomplete or untested work.

### Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch** (or a dedicated feature branch). Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP (`cargo build --release` + `/mcp` restart).
- Never commit directly to `master` for in-progress or exploratory work.

### Documenting Features on `experiments`

When adding a feature commit to `experiments`, you MUST include documentation in the same commit:

1. Create `docs/manual/src/experimental/<feature-name>.md` — written as final user-facing
   docs with a single `> ⚠ Experimental — may change without notice.` callout at the top.
2. Add a line to `docs/manual/src/experimental/index.md` linking to the new page.

**Only features, not bug fixes.** Bug fixes need no experimental doc.

**If a feature is removed from `experiments`** (reverted or abandoned), delete its page and
remove its entry from `index.md` in the same commit.

**The experimental docs stay on `experiments` only.** `master`'s `experimental/index.md`
just points to the `experiments` branch on GitHub — it does not list features directly.
This means no cherry-picking of docs to master; the full pages are visible to anyone
browsing the experiments branch.

### Graduating a Feature (`experiments` → `master`)

When cherry-picking a feature to `master`, use `--no-commit` to bundle the doc graduation
into the same commit:

```bash
git cherry-pick --no-commit <sha>
# then make the four graduation changes:
# 1. Move docs/manual/src/experimental/<feature-name>.md to its target chapter
# 2. Remove the `> ⚠ Experimental` callout from the top of the page
# 3. Add the page to docs/manual/src/SUMMARY.md in the right place
# 4. Remove the feature's entry from docs/manual/src/experimental/index.md
git commit -m "feat(...): <description>"
```

The experimental doc page already exists on `experiments` — step 1 is a `git mv`, not a
rewrite. The ⚠ callout and the `experimental/index.md` entry are the only things to remove.

**Rebase note:** Because the graduation commit on `master` includes additional doc changes,
its patch differs from the original `experiments` commit. Git will **not** auto-skip it
during the subsequent `git rebase master` on `experiments`. After rebasing, drop the
now-superseded original commit manually:

```bash
git checkout experiments
git rebase master          # the original feature commit will NOT be auto-dropped
git rebase -i master       # drop the original feature commit from the list
```

### Release Cycle

Full release checklist — run from `master`, never from `experiments` or feature branches.

```bash
# 1. Bump version in Cargo.toml
#    Edit version = "X.Y.Z" in Cargo.toml

# 2. Build release binary and verify
cargo build --release
cargo test
cargo clippy -- -D warnings

# 3. Commit the version bump
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.Z"

# 4. Tag the release
git tag vX.Y.Z

# 5. Publish to crates.io
CARGO_REGISTRY_TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2) cargo publish

# 6. Push commit + tag
git push
git push --tags

# 7. Create GitHub release with release notes
gh release create vX.Y.Z --title "vX.Y.Z" --notes "release notes here"

# 8. Rebase experiments on the new master
git checkout experiments && git rebase master
```

**Notes:**
- Token is stored in `.env` (gitignored): `CARGO_REGISTRY_TOKEN=...`
- Use semver: patch for bug fixes, minor for new features, major for breaking changes
- Release notes should list features, dep upgrades, and doc changes
- Always rebase `experiments` after the release push

### Standard Ship Sequence

When a bug fix or tested feature on `experiments` is ready to land in `master`:

```bash
# 1. Commit on experiments (tests passing, clippy clean)
git add <files> && git commit -m "..."

# 2. Cherry-pick to master and push
git checkout master
git cherry-pick <commit-sha>
git push

# 3. Rebase experiments back on master (drops the cherry-picked commit automatically)
git checkout experiments
git rebase master
```

This is the default workflow for all completed work. The rebase step keeps `experiments`
clean — git detects the cherry-pick and skips the duplicate commit automatically.

### Commit Discipline

- **Batch related changes** into a single well-tested commit rather than committing every incremental step.
- **Only commit when the full fix/feature is working** — all tests pass, clippy clean, manually verified if applicable.
- **Do not push after every commit.** Accumulate local commits during a work session; push once when the work is solid.
- When iterating on a fix, keep working locally until the fix is confirmed, then commit the final state — not every intermediate attempt.

## Project Structure

See codescout memory `architecture` (Source Tree section).

## Design Principles

**Progressive Disclosure & Discoverability** — Every tool defaults to the most
compact useful representation. Details are available on demand via
`detail_level: "full"` + pagination. When results overflow, responses include
actionable hints and file distribution maps (`by_file`). See
`docs/PROGRESSIVE_DISCOVERABILITY.md` for the canonical patterns and
anti-patterns — **read it before adding or modifying any tool**.

**Token Efficiency** — The LLM's context window is a scarce resource. Tools
minimize output by default: names + locations in exploring mode, full bodies
only in focused mode. Overflow produces actionable guidance ("showing N of M,
narrow with..."), not truncated garbage.

**No Echo in Write Responses** — Mutation tools (`create_file`, `edit_file`,
`replace_symbol`, etc.) must never echo back what the LLM just sent. The caller
already knows the path, content, and size — reflecting them wastes tokens with
zero information gain. The only new information after a write is success/failure.
Return `json!("ok")` for writes; reserve richer responses for cases where the
tool discovers genuinely new information (e.g. LSP diagnostics after a write).

**Two Modes** — `Exploring` (default): compact, capped at 200 items. `Focused`:
full detail, paginated via offset/limit. Enforced via `OutputGuard`
(`src/tools/output.rs`), a project-wide pattern not per-tool logic.

**Tool Selection by Knowledge Level** — Know the name → LSP/AST tools
(`find_symbol`, `list_symbols`, `goto_definition`, `hover`). Know the concept →
semantic search first, then drill down. Know nothing → `list_dir` +
`list_symbols` at top level, then semantic search.

**Agent-Agnostic Design** — Tool descriptions, error messages, and server
instructions are the primary interface for LLMs. They must feel natural for
Claude Code (our primary consumer) but work for any MCP client (Gemini CLI,
Cursor, custom agents). In particular:
- Error hints should name codescout tools (`replace_symbol`, `insert_code`),
  not host-specific tools (`Edit`, `Write`). The LLM should never be tempted to
  sidestep codescout by falling back to its host's native file editing.
- The companion plugin (`code-explorer-routing`) adds Claude Code–specific
  enforcement (PreToolUse hooks) but the server itself must be self-contained:
  its gate logic, error messages, and instructions should guide any LLM toward
  the right tool without relying on external hooks.

## Testing Patterns

**Cache-invalidation tests use a three-query sandwich** — not two. The structure is:
1. Query → record baseline state
2. Mutate the underlying data (disk, cache, external system) without going through the normal notification path
3. Query again → assert result is **stale** (same as baseline) — this proves the bug exists
4. Trigger the invalidation (e.g. `did_change`, cache flush)
5. Query again → assert result is **fresh** (reflects the mutation)

A two-query test (baseline → post-invalidation) only confirms the happy path. The stale-assertion in step 3 is what makes it a *regression* test — it will fail if the underlying system ever changes to eagerly re-read on every query, alerting you that the invalidation logic has become wrong or unnecessary.

See `did_change_refreshes_stale_symbol_positions` in `src/lsp/client.rs` for the canonical example.

## Key Patterns

**Tool trait** (`src/tools/mod.rs`): Each tool is a struct implementing `name()`, `description()`, `input_schema()`, `async call(Value, &ToolContext) -> Result<Value>`. 27 tools registered. All use `#[async_trait]`.

**Tool↔MCP bridge** (`src/server.rs`): Tools registered as `Vec<Arc<dyn Tool>>`, dispatched dynamically in `call_tool`. Errors are routed through `route_tool_error`:
- `RecoverableError` (`src/tools/mod.rs`) → `isError: false` with JSON `{"error":"…","hint":"…"}` — LLM sees the problem and a corrective hint, **sibling parallel calls are not aborted by Claude Code**.
- Any other `anyhow::Error` → `isError: true` (fatal; something truly broke).

Use `RecoverableError` for expected, input-driven failures (path not found, unsupported file type, empty glob). Use plain `anyhow::bail!` for genuine tool failures (LSP crash, security violation, programming error).

**`ToolContext`** fields: `agent` (project state + config access), `lsp` (LSP client pool), `output_buffer` (session-scoped `@cmd_*`/`@file_*` handle store), `progress` (MCP progress reporter).

**Config** (`.codescout/project.toml`): Per-project settings including embedding model, chunk size, ignored paths. `ProjectConfig::load_or_default()` handles missing config gracefully.

**Embedding pipeline**: `chunker::split()` → `RemoteEmbedder::embed()` → `index::insert_chunk()` → `index::search()` (cosine similarity). All stored in `.codescout/embeddings.db`. Incremental updates via `find_changed_files()`: git diff → mtime → SHA-256 fallback chain. `semantic_search` warns when the index is behind HEAD.

## Prompt Surface Consistency

The project has **three prompt surfaces** that reference tool names:
- `src/prompts/server_instructions.md` — injected every MCP request
- `src/prompts/onboarding_prompt.md` — one-time onboarding
- `build_system_prompt_draft()` in `src/tools/workflow.rs` — generated per-project

**When tools get renamed/consolidated, all three need coordinated updates.** Files
closer to the change get updated; distant ones accumulate stale refs ("distance
from change" problem). Always grep all three surfaces when modifying tool names.

**Any change to tool behavior or signatures requires a prompt surface review.**
This includes: adding new tools, renaming tools, changing parameter semantics,
adding new error/fallback modes, or modifying response shapes. Ask yourself:
"Does the LLM need to know about this change to use the tool correctly?" If yes,
update all three surfaces in the same commit.

### Onboarding Version

When modifying system prompt surfaces, bump `ONBOARDING_VERSION` in
`src/tools/workflow.rs`. This triggers automatic system prompt refresh for all
projects onboarded with the previous version.

Bump when the generated system prompt would reference tool names, parameters,
or workflows that no longer exist:
- Tool names change (rename, consolidate)
- Tool parameter semantics change
- Server instructions (`server_instructions.md`) change significantly
- Onboarding prompt templates change in ways that affect the generated system prompt

Do NOT bump for:
- Bug fixes that don't change tool behavior
- Internal refactors
- Memory template changes (memories are re-read during refresh anyway)

`src/prompts/server_instructions.md` is injected **once at MCP session start**,
not per-request. Token cost is session-scoped, not per-call — invest in clarity
over brevity. Follow these rules when modifying it:

1. **Cap hard rules at 5–8.** Beyond 8 behavioral constraints, compliance on all
   drops. Consolidate, don't accumulate.
2. **No triple-layer repetition.** A rule in Iron Laws should NOT be restated in
   Anti-Patterns AND Rules. Max 2 appearances: once as a law, optionally once as
   a closing reminder (for the 1–2 most-violated rules only).
3. **Tables > prose** for decision-matrix content. Claude scans tables faster.
4. **End of prompt = highest compliance.** Put the most-violated rule(s) in the
   closing `## Rules` section — that's closest to generation.
5. **Don't document every param.** Pagination (`offset`, `limit`, `detail_level`)
   and aliases (`file_path`, `limit`) are discoverable from the tool schema. Only
   document params that change behavior in non-obvious ways.
6. **Prompt caching matters.** Keep section order stable between releases so the
   static prefix benefits from automatic caching. Don't reorganize for cosmetic reasons.
7. **You are the consumer.** When writing or reviewing prompt changes, think as
   the agent who will read this mid-task. Ask: "Would this have helped me find
   the right tool chain naturally?" Test by simulating a realistic task and
   checking whether the prompt guided you to the right flow. Usage data
   (`usage.db`) is the ground truth — if a tool has near-zero calls despite
   being useful, the prompt isn't surfacing it.

**Research:** See `docs/research/2026-03-21-claude-prompt-engineering.md` and
`docs/research/2026-03-21-superpowers-prompt-patterns.md` for the evidence behind
these rules.

## Companion Plugin: code-explorer-routing

This project has a companion Claude Code plugin at **`../claude-plugins/code-explorer-routing/`** that is **always active** when working on codescout. You must be aware of it.

**What it does:**
- `SessionStart` hook (`hooks/session-start.sh`) — injects tool guidance + memory hints into every session
- `SubagentStart` hook (`hooks/subagent-guidance.sh`) — same for all subagents
- `PreToolUse` hook on `Grep|Glob|Read` (`hooks/semantic-tool-router.sh`) — **blocks native Read/Grep/Glob on source files**, redirecting to codescout MCP tools

**Critical implication for working on this codebase:**
The `PreToolUse` hook will **block** any attempt to use the native `Read`, `Grep`, or `Glob` tools on source code files (`.rs`, `.ts`, `.py`, etc). You will see `PreToolUse:Read hook error` if you try.

**You MUST use codescout's own MCP tools to read source code:**
- `mcp__codescout__list_symbols(path)` — see all symbols in a file/dir
- `mcp__codescout__find_symbol(name, include_body=true)` — read a function body
- `mcp__codescout__search_pattern(pattern)` — regex search
- `mcp__codescout__semantic_search(query)` — concept-level search
- `mcp__codescout__read_file(path)` — for non-source files (markdown, toml, json)

**Configuration:**
- Auto-detects codescout from `.mcp.json` or `~/.claude/settings.json`
- Can be overridden via `.claude/code-explorer-routing.json`
- `block_reads: false` in that config to disable blocking (dev/debug use)

## Rust Coding Standards

See codescout memory `language-patterns` (Rust section) for anti-patterns and idiomatic patterns.


## Language-Specific LSP Issues

### Kotlin (kotlin-lsp, JetBrains)

**Single workspace session:** kotlin-lsp allows only one LSP process per project
directory. A second instance fails with *"Multiple editing sessions for one
workspace are not supported yet"*. codescout detects this on stderr and fails
fast instead of retrying for 120s.

**Cold start:** JVM bootstrap + Gradle import takes 8–15s. codescout retries the
LSP initialize handshake (5 × 3s backoff = ~30s window) to handle -32800
(RequestCancelled) during this phase.

**Per-instance isolation:** Each codescout process passes
`--system-path=/tmp/codescout-<PID>-kotlin-lsp` to avoid IntelliJ platform
`.app.lock` contention between instances.

**Tracking:** `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`

## Docs

- **`docs/PROGRESSIVE_DISCOVERABILITY.md`** — Canonical guide for output sizing, overflow hints, and agent guidance patterns. **READ THIS before adding or modifying any tool.**
- `docs/plans/2026-02-25-v1-implementation-plan.md` — Sprint-level plan (Phase 0–5, 15 sprints)
- `docs/ARCHITECTURE.md` — Component details, tech stack, design principles
- `docs/ROADMAP.md` — Quick status overview
