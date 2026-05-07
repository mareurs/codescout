You have just onboarded this project. Your job is to create memories and a system
prompt that give future AI sessions deep, accurate knowledge of this codebase.
For single-project repos, this means 6 memories. For multi-project workspaces,
see the WORKSPACE MODE section below (if present).

## THE IRON LAW

```
NO MEMORIES WRITTEN WITHOUT COMPLETING ALL EXPLORATION STEPS FIRST
```

```
DON'T LEAVE ANY STONE UNTURNED
```

**Violating the letter of this process is violating the spirit of onboarding.**

This is a one-time setup. Every future AI session depends on the accuracy of what you
write now. **Token efficiency is NOT a concern here. Thoroughness is the ONLY goal.**
Be exhaustive. Read widely. When in doubt, read more.

<HARD-GATE>
Do NOT call `memory(action: "write", ...)` until you have:
1. Completed ALL 7 exploration steps below
2. Verified EVERY item in the Phase 2 Gate Checklist
3. Written the Exploration Summary in your response

These gates are non-negotiable. There are no exceptions.
</HARD-GATE>

---

<!-- STABLE-HEADING: workspace_onboarding_prompt.md may reference this section by exact title. Do not rename without updating cross-references. -->
## Phase 0: Embedding Model Selection

The `onboarding` tool has already written a recommended model to `.codescout/project.toml`
based on your system hardware. Present the options to the user now, before indexing starts.

Use the `model_options` array from the Gathered Project Data below to build the menu.
Use the `hardware` field for the one-line system summary.

Present this to the user:

> **Choose an embedding model for semantic search.**
>
> Based on your system ({hardware.cpu_cores} CPU cores
> {if hardware.gpu: ", {hardware.gpu.name}"}
> {if hardware.ollama_available: ", Ollama running" else: ", no Ollama detected"}):
>
> {for i, opt in model_options:}
> {i+1}. {if opt.recommended: "★ "}`{opt.id}` — {opt.dims}d, {opt.context_tokens}-token context
>    {opt.reason}{if opt.recommended: " ← **Recommended**"}{if not opt.available: " *(not currently available)*"}
> {end}
>
> Press Enter to accept [1], or type a number to choose a different option.
>
> **Tip:** For multi-project workspaces, running a dedicated embedding server is
> recommended over the bundled model. Set `url` in `.codescout/project.toml` to
> point at any OpenAI-compatible endpoint (llama.cpp, Ollama, vLLM, TEI).
> See the embeddings guide for setup examples.

Wait for the user's response, then:

- **User presses Enter or types 1:** The config is already correct — proceed to Phase 1.
- **User types 2, 3, etc.:** Call `edit_file` on `.codescout/project.toml`.
  Change the `model` line to the selected option's ID. If the option is `url`,
  ask the user for their server URL and add both `model` and `url` fields.
  Confirm the edit, then proceed to Phase 1.
- **User types a custom model string:** Use that string directly in the `edit_file` call.
  If it looks like a URL, suggest adding it as `url` instead.

Then proceed to Phase 1 (Semantic Index Check).
## Phase 1: Semantic Index Check

Check the **Semantic index** line in the Gathered Project Data below.

### If the index is READY:

Announce to the user:

> "Semantic index is ready ({files} files, {chunks} chunks). I'll use
> `semantic_search` for concept-level exploration in Phase 2."

Proceed to Phase 2.

### If the index is NOT BUILT:

Semantic search is **strongly recommended** for thorough onboarding. Present
this to the user:

> **Semantic search is not set up yet.**
>
> The embedding index powers concept-level code exploration (`semantic_search`),
> which finds code by meaning — not just by name or text pattern. Without it,
> onboarding relies on symbol tools and regex search, which work but may miss
> non-obvious connections.
>
> **Options:**
> 1. **Build now** — I'll call `index(action='build')` and wait for it to finish.
>    Requires an embedding backend (bundled ONNX is the default, Ollama/OpenAI optional — see
>    `docs/manual/src/configuration/embedding-backends.md` for setup).
>    Takes 1–5 minutes depending on codebase size.
> 2. **Build from CLI** — Run `codescout index --project .` in another
>    terminal, then restart onboarding with `onboarding(force: true)`.
> 3. **Skip** — Proceed without semantic search. Exploration will use
>    `grep` (regex) instead of `semantic_search`. You can always
>    build the index later.

Wait for the user's choice before proceeding.

- **Option 1:** Call `index(action='build')`. Poll `index(action='status')` every 15
  seconds until the response shows completion or failure. If it fails, inform
  the user of the error and fall back to option 3.
- **Option 2:** Stop and wait for the user to return.
- **Option 3:** Proceed to Phase 2. Step 6 will use `grep` instead
  of `semantic_search`.

---

## Phase 2: Explore the Code

Your goal is to build a complete mental model of this codebase — enough to write
accurate, specific project memories in Phase 3. Use whatever tools and exploration
strategy you judge best. The gate checklist below is your hard constraint.

### Goals

- **Map the structure.** Understand the directory layout, module organization,
  and entry points. Know what lives where.
- **Understand core abstractions.** Identify the 3–5 key types/traits/classes
  that form the skeleton. Read their full implementations, not just signatures.
- **Read all architecture docs.** Completely — not skimmed. If docs exist, they
  contain decisions you need for accurate memories.
- **Trace at least 2 data flows.** Follow concrete operations end-to-end through
  the code, with actual function/method names — not just "the request goes through
  the middleware layer." Use `call_graph(symbol, direction="callees", max_depth=3)`
  to trace call chains; use `call_graph(direction="callers")` to size blast radius
  before edits.
- **Search by concept.** Run at least 5 semantic or keyword searches for concepts
  the codebase likely embodies (error handling, caching, authentication, etc.).
  Discover what the code does that README/docs don't mention.
- **Examine tests.** Read 2–3 test files to understand testing patterns, helpers,
  and fixtures used in this project.
- **Verify the build.** Confirm the project builds and tests pass.


---

### Phase 2 Gate Checklist

Before writing ANY memory, verify ALL of these are true. If any is unchecked, complete it first.

- [ ] Listed top-level structure AND ran `tree` on each major subdirectory
- [ ] Ran `symbols` on the top-level source AND on at least 4 subdirectories individually
- [ ] Read the FULL body (not just signature) of at least 5 core types/functions
- [ ] Read ALL architecture docs found, completely (not skimmed)
- [ ] Traced two distinct data flows from entry point to terminal output (use `call_graph(direction="callees")` for at least one)
- [ ] Ran at least 5 concept-level queries (`semantic_search` or `grep` fallback)
- [ ] Read 2–3 test files and understood the testing pattern
- [ ] Verified build/dev commands against actual repo contents

**If ANY item is unchecked: complete it before writing a single memory.**

---

### Exploration Summary

After completing all steps, write this summary **in your response, before calling any
`memory(action: "write", ...)` tool**:

> **What this system does** — in your own words, not the README's
> **The 5 most important types/modules** — name, file path, and role each plays
> **How a typical operation flows** — concrete function/method names, not just layers
> **What surprised you** — things the code does that documentation didn't mention

If you cannot write this from what you've explored, you have not explored enough.
Return to Phase 2.


---

## Red Flags — STOP and Return to Phase 2

If you notice any of these thoughts, STOP. Return to Phase 2 immediately.

- "I've read CLAUDE.md and the README — that's enough to write the memories"
- "The architecture doc covers everything I need"
- "I can infer how it works from the signatures and names"
- "I only need to survey the main files, not every module"
- "This project is small/simple, less exploration is fine"
- "I'll write the memory now and add details if something is wrong later"
- "I already understand this type of codebase"
- You have read fewer than 5 code bodies with `include_body=true`
- You have run `symbols` on fewer than 3 modules/directories
- You have traced only one data flow
- You have run fewer than 5 concept-level queries (semantic_search or grep)

**ALL of these mean: STOP. Return to Phase 2.**## Common Rationalizations

| Excuse | Reality |
|---|---|
| "CLAUDE.md and the README give me enough context" | Docs describe intent. Code reveals reality. Discrepancies hide in the code. |
| "I can infer implementations from names and signatures" | Assumptions about implementations produce wrong memories that mislead future sessions. |
| "I already understand this type of system" | Pattern recognition replaces exploration. This codebase has specific wiring that differs from the pattern. |
| "This is a small project, I can do less" | Small codebases still have gotchas. The steps scale down naturally — don't skip them. |
| "I'll refine the memories later if something is wrong" | Wrong memories mislead every session until someone notices and fixes them. Do it right once. |
| "Token efficiency matters here" | This is a ONE-TIME setup. Tokens spent here prevent thousands of wasted tokens in every future session. Be thorough. |
| "I traced one flow — that's enough" | One flow shows one path. A second reveals where paths diverge and where exceptions live. |
| "I read the docs — I understand the architecture" | Architecture docs describe the intended design. Code reveals the actual design. Read both. |

---



---## Phase 3: Write the Memories (Single-Project Mode)

> **If you see a "WORKSPACE MODE" section below**, skip this section entirely and
> follow the workspace flow instead. This section applies only to single-project repos.

Now write the memories. Your Phase 2 exploration must inform every memory — especially
`architecture` and `conventions`, which cannot be written accurately from documentation alone.

### Rules

1. **Do NOT duplicate auto-loaded context** — CLAUDE.md, project README, and referenced docs are already available every session. Memories must *supplement* them, not repeat them. If something is already documented, write a pointer (`see CLAUDE.md § Key Patterns`) rather than copying it.
2. **References over copies — drift is real** — Code and docs change. A memory that copies a code snippet or lists tool names will go stale silently and actively mislead future sessions. Prefer: `"see docs/ARCHITECTURE.md for the layer diagram"` over pasting the diagram. Reserve inline content for things that are NOT documented elsewhere.
3. **Memories capture gaps, not summaries** — Ask: "Would a future AI session know this from CLAUDE.md, the README, or the referenced docs?" If yes, skip it or point to the source. Only write it if the answer is no.
4. **Be specific where you do write** — Include file paths, exact command names, concrete patterns. "Uses clean architecture" is useless. "`api/ → service/ → repository/` with interface+impl pattern in `src/`" is useful.
5. **Be concise** — Each memory should be 15–40 lines. Longer means too much detail or duplication.
6. **Confirm with the user** — After creating all 6 memories, summarize what you wrote and ask if anything needs correction.
7. **Private memories** — Use `memory(action: "write", topic: ..., content: ..., private: true)` for project-local notes that should not appear in system instructions (e.g. personal debugging notes, temporary state). Standard `memory(action: "write", ...)` creates shared memories visible to all agents.

### Protected Memories

Check the `protected_memories` field from the onboarding tool response above. For
each memory you are about to write, check whether it appears there:

**If `protected_memories[topic].exists == false`:** Create fresh as normal.

**If `protected_memories[topic].exists == true` AND `staleness.untracked == false`
AND `staleness.stale_files` is empty:** The memory is fresh — all anchored source
files are unchanged. **Skip writing this topic entirely.** Tell the user:
> "Kept `[topic]` unchanged (all references still valid)."

**If `protected_memories[topic].exists == true` AND (`staleness.untracked == true`
OR `staleness.stale_files` is non-empty):** Run the merge flow:

1. The existing content is in `protected_memories[topic].content`.
2. For entries referencing files listed in `staleness.stale_files` (or all
   entries if `untracked`): use `symbols`, `read_file`, `grep`
   to verify whether each entry is still accurate.
3. Identify new discoveries from your Phase 2 exploration that belong in
   this memory.
4. Present a diff-style summary to the user:
   - **Stale (recommend removing):** [entries no longer accurate, with reason]
   - **Still valid (keeping):** [verified entries]
   - **New findings:** [discoveries from exploration]
   - **Proposed merged version:** [full content]
5. **Wait for user approval** before calling `memory(action="write")`.

**If a topic is NOT in `protected_memories`:** Write it as normal (overwrite).

The protected topics list is configured in `project.toml` under `[memory] protected`.
Users can add custom topics. The programmatic memories (`onboarding`, `language-patterns`)
are always excluded from protection.


Apply the **project-scope** sections of the included memory templates below. Write all 6 project-scope memories. Use the empty stub for `domain-glossary` and `gotchas` if nothing project-specific applies — do NOT skip them.

For `system-prompt`, apply the `workspace-scope: system-prompt` section (single-project flow treats the project as its own workspace).

{{include: memory-templates.md}}

## After Everything Is Created

## Coverage Verification

After writing all 6 project-scope memories, read each back:

```
memory(action: "read", topic: "<topic>")
```

Verify each is present (or matches the canonical empty stub for eligible topics). If any read fails or returns content shorter than the empty stub, retry the missing write up to 2 times. If still missing, abort with a clear error and do NOT proceed to CLAUDE.md refresh.

After confirming all 6 memories and the system prompt with the user, deliver this:

---

**Your codescout setup is complete.**

- **System prompt** (`.codescout/system-prompt.md`) — always-on project guidance,
  injected into every session. Edit anytime to refine how AI navigates your codebase.
- **Memories** — reference material read on demand via `memory(action: "read", topic: ...)`. Update
  with `memory(action: "write", topic: ..., content: ...)`.
- **Semantic memories** — use `memory(action: "remember", content: "...")` to store knowledge
  that doesn't fit a named topic. Search later with `memory(action: "recall", query: "...")`.
  Useful for preferences, patterns discovered during work, and cross-cutting notes.
- **Extended docs & project context** are available as MCP resources:
  - `doc://codescout-tool-guide` — long-form usage notes for every tool (examples, tradeoffs)
  - `memory://<name>` — project memory files (architecture, conventions, gotchas)
  - `project://summary` — active project + index + LSP snapshot
  Fetch via `resources/read <uri>` when you need more than a tool's short description.
- **Quick start for new tasks:**
  1. `memory(action: "read", topic: "architecture")` — orient yourself
  2. `symbols("src/")` — see the module structure
  3. `semantic_search("your concept")` — find relevant code
  4. `symbols(name="Name", include_body=true)` — read the implementation
  5. `librarian_context(topic)` → `artifact(action=find, semantic="...")` → `artifact(action=create)` — track decisions, issues, or plans as artifacts (call `librarian(action=tracker_design)` first for structured multi-entry trackers)
- **Library support:**
  - Libraries are **auto-discovered** when `symbol_at` resolves outside the project root.
  - `library(action="list")` — view all registered libraries and their index/version status.
  - `index(action='build', scope="lib:<name>")` — index a specific library for semantic search.
  - Once registered, use `scope="lib:<name>"` with `symbols`, `symbols`,
    `grep`, and `semantic_search` to navigate library code.

---

> **For workspace repos:** The above applies to single-project repos. For workspace repos,
> the subagent deep dives + workspace synthesis flow replaces this section. Summarize
> all per-project and workspace-level memories in one confirmation pass.

---

### Refresh CLAUDE.md

Compute the canonical memory table from what was written this run. Each row's "What's inside" cell is the first `## H2` of the memory body.

Read existing `CLAUDE.md`. Locate `## codescout Memories` (or propose adding it). Generate a unified diff for the table block.

Ask the user **once**:

```
Proposed CLAUDE.md memory-table update:

  [unified diff]

Apply? [y/N]
```

On `y`: `edit_markdown(path: "CLAUDE.md", action: "replace", heading: "## codescout Memories", content: <new table>)`. On `N` or no answer: log `claude_md: skipped (user declined)` for the final summary. No follow-up questions.
## Gathered Project Data

The data below was collected automatically. Use it as your starting point, then explore with codescout tools to fill gaps.

---

## Optional: Private Memories

After creating the 6 shared memories above, check if any personal context is worth
capturing now. Use `memory(action: "write", topic: ..., content: ..., private: true)` for anything specific
to your setup — local machine config, personal workflow preferences, or current WIP
context. This is optional; skip if nothing personal applies yet.

## Optional: Semantic Memories

For knowledge that doesn't fit a named topic — personal preferences, recurring patterns,
project-specific learnings — use semantic memories:

- `memory(action: "remember", content: "Always run integration tests with --release flag", bucket: "preferences")` — store a preference
- `memory(action: "remember", content: "The auth module uses a custom middleware chain")` — store a note (bucket auto-classified)
- `memory(action: "recall", query: "testing preferences")` — search by meaning later

Semantic memories with `bucket: "preferences"` are automatically included in future
onboarding prompts, so they persist across sessions without manual recall.
