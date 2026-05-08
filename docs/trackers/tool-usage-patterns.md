---
id: abc513d3ee0f0b50
kind: tracker
status: active
title: Tool Usage Patterns
owners: []
tags:
- grep
- prompt-quality
- iron-law-7
topic: tool usage patterns audit optimization prompt quality
time_scope: null
---


# Tool Usage Patterns

Our internal instrumentation for codescout tool decisions — analogous to Langfuse but for
tool selection quality rather than token cost. Each observation (T-N) is a real tool call
or sequence extracted from a session trace, judged against the ideal.

Feed into: Iron Law updates, Anti-Patterns table, prompt surface revisions.

## Scope and methodology

Verdict:
- **legitimate** — right tool; prompt should affirm this case
- **debatable** — a better tool exists but requires more setup; prompt is missing guidance
- **wrong-tool** — a structured tool clearly wins; add to anti-patterns

Proof standard: claims about "better tool" must be verified by actually running the
alternative (via `/codescout-companion:explore-project`) before updating verdict.

---

## grep observations

### T-001 — Multi-symbol property scan across constraint directory
**Session:** 64618681 (Kotlin backend, 2026-05-03)  
**Pattern:** `isManual|isPinned|isStage|lessonType|LessonType|manual|stage.*lesson|MANUAL|PINNED`  
**Path:** `ktor-server/.../solver/constraints` (directory)

Agent needed to find *which constraint files* reference policy-related fields simultaneously.
Initial assessment flagged this as debatable (suggested semantic_search instead).

**Verified 2026-05-03 via live exploration:** `semantic_search("constraint checks manual or
pinned lesson")` returned test files, loaders, docs — missed the core constraint files.
`grep` with scoped path gave 48 directly actionable hits. **grep was correct.**  
**Verdict:** legitimate — no prompt gap.

### T-002 — Access-pattern refinement via boundary regex
**Session:** 64618681 (Kotlin backend, 2026-05-03)  
**Pattern:** `manualPolicy\.|skipManual|isManual\b|\.isManual|isStage\b|\.isStage`  
**Path:** same constraints directory

Follow-up to T-001. Agent found the files and now wants to understand *how* `manualPolicy`
is accessed. Boundary regex (`\b`, `\.`) distinguishes method call vs field access.

`references(symbol="isManual", path=<defining file>)` would be more precise, but the agent
didn't know the defining file yet. The right sequence: `symbols(name="isManual")` to find
the defining file, then `references(symbol, path)` for call sites.  
**Verdict:** debatable — prompt gap: after grep finds the files, next step should be
`symbols(name=X)` → `references(symbol, path)`, not another grep.

### T-003 — Enum/constant + property name mix
**Session:** 64618681 (Kotlin backend, 2026-05-03)  
**Pattern:** `isTeachingDayOfWeek|teachingDays|weekend|dayOfWeek|SATURDAY|SUNDAY`  
**Path:** same constraints directory

Task shifted to teaching-day constraints. Mixed bag: `SATURDAY`/`SUNDAY` are enum values
(legitimate grep targets); `isTeachingDayOfWeek` and `teachingDays` are property names
(symbols/references would be more precise).  
**Verdict:** legitimate for the constant portion — prompt gap: enum/constant values not
explicitly listed as valid grep targets in Iron Law #7.

---

## semantic_search observations

### T-004 — Concept search scoped to known purpose directory
**Session:** 64618681 (Kotlin backend, 2026-05-03)  
**Query:** `"constraint checks manual or pinned lesson"`  
**Scope:** whole codebase (default)

Agent (in original session) implicitly used semantic_search as a discovery tool. We ran
it explicitly in the verification exploration.

**What it returned:** 10 hits — test files, docs, loaders, ManualLessonPolicy config.
Did surface `ManualLessonPolicy.kt` but not the constraint *implementation* files
(`SchedulingPolicyConstraints.kt`, `LegalLimitConstraints.kt`, etc.).

**What grep gave:** `grep(pattern="\.isManual|\.isPinned", path="constraints")` → 48 hits,
all constraint files, directly actionable with file+line.

**Root cause:** semantic_search is trained on whole-codebase concept similarity. When the
target code is already in a known purpose directory, the embedding similarity pulls in
distant conceptually-related files (tests, docs, config) before the local implementations.
grep with `path=` is a hard filter; semantic_search is a soft ranking.

**When semantic_search wins:** whole-codebase concept discovery when you don't know which
directory to look in. "How does the solver handle room conflicts?" with no known path.  
**Verdict:** wrong-tool for scoped directory search.

---


### T-005 — `npm run build 2>&1 | grep` × 7 in one session
**Session:** c5daabbe (eduplanner-ui, 2026-05-03)

Build verification after each edit batch. Model ran the same piped pattern 7 times (#48, #73, #77, #111, #113, #151, #159). Iron Law #3 prohibits piping run_command output — the buffer workflow exists precisely for this.

**Why it keeps happening:** `npm run build` produces verbose output; piping to grep is a strong developer instinct. The rule exists but has no concrete build example to anchor it.  
**Prompt gap:** Iron Law #3 needs a before/after build example: `npm run build 2>&1 | grep` ✗ → `run_command("npm run build")` then `grep "error TS" @cmd_id` ✓.

### T-006 — `cat file | head -50` to read source (#123)
**Session:** c5daabbe (eduplanner-ui, 2026-05-03)

Double violation: Iron Law #1 (reading source via shell) + Iron Law #3 (piping). `symbols(path=...)` gives structured output in fewer tokens.  
**Verdict:** wrong-tool.

### T-007 — `grep(^import, specific_file)` before editing
**Session:** c5daabbe (eduplanner-ui, 2026-05-03) — ×8 (calls 52, 61, 97, 102, 106, 127–134)

Checking existing imports before adding a new one. `grep(pattern="^import", path=<single file>)` — fast, returns only import lines. `symbols(path=file)` returns all symbols (heavier). Used in batch (8 files before bulk editing) — efficient.  
**Verdict:** legitimate.

### T-008 — `edit_file` drift for structural TS mutation callbacks
**Session:** c5daabbe (eduplanner-ui, 2026-05-03) — ~20 calls

Started with `edit_code` correctly for service file structural changes (calls 39–45), then drifted to `edit_file` for adding `onError` callbacks inside `useMutation` bodies in hook files. Both are structural TS edits — `edit_code` is correct for both.

**Pattern:** model used the right tool first then regressed. `edit_file` feels natural when "adding a field to an options object".  
**Prompt gap:** Anti-Patterns table should add: "Adding a callback/handler inside a function call → `edit_code`, not `edit_file`".

## onboarding observations

### T-009 — workspace onboarding HARD-GATE checked one topic per project
**Tool:** `memory` (read)  
**Verdict:** wrong-tool — gate logic was a single read per project; should have been a 6×N matrix.  
**Prompt gap:** workspace_onboarding_prompt.md HARD-GATE language was "verify project-overview" rather than "verify all required topics". Fixed 2026-05-08 with Phase 4 Coverage Verification read-back loop that checks all 6 mandatory topics per project and retries missing ones before proceeding to workspace synthesis.

## Prompt improvement candidates

### Iron Law #7 — Scope distinction for grep vs semantic_search

Add to decision tree:

> - "Which files in **this directory** reference these fields/constants?" → `grep` with `path=` scope ✓
> - "Which files **anywhere in the codebase** deal with this concept?" → `semantic_search("concept")` ✓
> - "How is a symbol accessed at call sites?" → `symbols(name=X)` to find defining file, then `references(symbol, path)` — not another grep
> - "Searching for an **enum value or string constant**?" → `grep` ✓ — constant values aren't navigable symbols

### Anti-Patterns table — Add missing rows

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `grep("field")` to find all usages | `references(symbol, path)` | LSP finds structured usages; grep matches comments, strings, unrelated identifiers |
| `grep("ENUM_VALUE")` on source | grep ✓ if value isn't a navigable symbol | Enum/constant values are a legitimate grep target |
| `grep("a\|b\|c")` across a dir to find concept files | `grep` with `path=` scope ✓ (if dir known); `semantic_search` only for whole-codebase | Semantic search adds noise when scope is already known |
| `semantic_search("concept")` when you already know the directory | `grep(pattern, path=<dir>)` | Embeddings rank by whole-codebase similarity; grep is a hard path filter |

## History


### 2026-05-08 — I-20 onboarding refactor (workspace prompt restructure)
Added T-009. Key finding: HARD-GATE was checking only 1 of 6 required memories per project — systematic under-coverage. Fixed by Phase 4 Coverage Verification matrix with 2-attempt retry loop.

### 2026-05-03 — Session c5daabbe analysis (eduplanner-ui error handling refactor)
Added T-005–T-008. Key findings: Iron Law #3 (piped run_command) is the most persistent violation — 7× in one session on `npm run build`. edit_file drift: model uses edit_code correctly then regresses to edit_file for same-type edits in a different file category. grep(^import) on specific files before editing confirmed legitimate.

### 2026-05-03 — Renamed and expanded from grep-usage-patterns
Expanded scope from grep-only to all tools. Added T-004 (semantic_search miss).
Proven via live `/codescout-companion:explore-project` run: semantic_search inferior
for scoped-directory multi-symbol discovery. Framing: internal Langfuse for tool decisions.

### 2026-05-03 — Initial population (as grep-usage-patterns)
First 3 observations from session 64618681 (Kotlin backend). G-001 verdict corrected
from debatable to legitimate after live proof.

