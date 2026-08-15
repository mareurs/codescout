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
**Tool:** mcp__codescout__memory (read)  
**Verdict:** wrong-tool — gate logic was a single read per project; should have been a 6×N matrix.  
**Prompt gap:** workspace_onboarding_prompt.md HARD-GATE language was "verify project-overview" rather than "verify all required topics". Fixed 2026-05-07 with Phase 4 read-back loop.

## filter observations

### T-010 — `artifact(find)` with `rel_path` filter hits SQLite column-not-found
**Tool:** `artifact` (find)  
**Verdict:** wrong-tool outcome — correct tool, broken field alias.  
**What happened:** LLM passed `filter: {"rel_path": {"contains": "tool-usage-patterns"}}` (the example literally shown in the tool schema at `artifact.rs:85`). Got runtime error `no such column: rel_path`. Schema v6 dropped `repo` and `rel_path` as separate DB columns, consolidating to `abs_path` — but `ALLOWED_FIELDS` in `filter.rs` still listed both dead names, so the filter compiled to invalid SQL.  
**Fix:** Remapped `rel_path` → `abs_path` in `compile_leaf` before SQL generation; removed phantom `repo` from `ALLOWED_FIELDS`. Regression tests added: `rel_path_filter_compiles_to_abs_path_sql` + `repo_filter_rejected` (`filter.rs`).  
**Prompt gap:** None needed — the tool description example was correct and now works. Root cause was a schema-migration orphan in the filter allowlist, not a prompt issue.


## artifact observations

### T-011 — `artifact(get, start_line, end_line)` off-by-one via an invisible blank separator

A per-repo `usage.db` mining pass (backend-kotlin + codescout, 2026-07-01 → 2026-07-09) surfaced 7 occurrences of the same downstream signature: `read_file(json_path="$.body")` on an `artifact(get)` result buffer returning `"0 lines"`. Root cause: `artifact(get, start_line=1, end_line=1)` addressed the blank separator line between the frontmatter's closing `---` and the body content — not the first visible content line — because `frontmatter::parse()`'s remainder was assigned to `parsed_body` verbatim, unshifted. Not a caller mistake — the tool's own 1-indexing silently pointed one line too early for every line-oriented consumer (full, slice, heading). See `docs/issues/archive/2026-07-09-artifact-get-line-slice-blank-separator-offset.md` for the full root-cause trace (including two rejected hypotheses) and fix.

### T-012 — `artifact(get, heading=)` required the full heading text, not a short id

Same mining pass found 6 occurrences in backend-kotlin (`heading="SI-20"`, `headings=["SI-19","SI-20"]`, `headings=["## SI-22","## SI-1"]`, `heading="SI-29"`, `heading="## SI-33"`) plus 1 in codescout, spanning a week — every heading query style against a numbered-section tracker (`## SI-N — <long title>`) missed, `body_meta.heading_missing: true`, no error. `get.rs` used a bespoke exact-match-only matcher instead of the shared, already-tested `file_summary::resolve_section_range` 4-tier fuzzy cascade that backs `read_markdown`/`edit_markdown`'s documented "fuzzy matched" `heading=` param — a prompt-surface consistency gap, not a caller error. See `docs/issues/archive/2026-07-09-artifact-get-heading-exact-match-only.md`.

### T-14 — The ledger query that was never made: `artifact(find, kind="bug")` before editing a file

**Tool:** `artifact` · **Verdict:** wrong-tool (omitted call) · **Observed:** 2026-08-06,
self-inflicted, during `experiments` → `master` merge preparation.

The session opened with an uncommitted change in `src/embed/ast_chunker.rs`. The *code*
seam was scouted properly — symbol bodies read, module tests run, real chunk output
dumped (T-15). The **decision** seam was never scouted. No
`artifact(find, kind="bug")` ran until roughly 60 tool calls later, and then only for an
unrelated verify-open pass.

**What the missing call would have returned.** Two open files, both directly on point:

- `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (open, high) whose *Fix
  candidate 1* is, verbatim, *"Introduce `AST_CHUNK_MIN` (~200-300 chars) and coalesce
  consecutive inner declarations below it into one chunk, keeping the container header"*
  — the change that was then implemented at 250 chars. The same file carries a landing
  precondition (*"validated against the retrieval benchmark … must be measured, not
  assumed"*) and a sequencing decision (throughput work first, being vector-identical).
  Both were violated.
- `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`, whose root
  cause a newly-filed bug duplicated.

**Why the existing guidance did not fire — the transferable part.** The rule exists and
was *in context from the first tool response*: `get_guide("project-activation-bootstrap")`
is auto-injected on `workspace(activate)` and its Phase 0 says *"Bug or regression work:
`artifact(action="find", kind="bug", status="open")` — the known-bug ledger. Don't re-file
a filed bug as new."*

It is trigger-shaped by **task category**. An agent that has classified its session as
*documentation + merge prep* answers "am I doing bug work?" with no — and keeps answering
no right up to the moment it edits a file with an open high-severity bug against it. A
category trigger cannot fire for someone whose self-classification excludes the category.

**Prompt gap / proposed fix.** Retrigger on **file identity** rather than task category:
before the first edit to any file, query the ledger scoped to that path or subsystem. What
the ledger holds is unreachable by any other means — not from the code, not from `git log`:
chosen-but-unimplemented fix candidates, preconditions on landing, and cross-change
ordering decisions. Filed as R-55 (`miss → proposal`) in
`docs/trackers/reconnaissance-patterns.md`; narrative in F-2 of
`docs/trackers/release-promotion-session-log.md`.
## run_command observations

### T-013 — `cargo test 2>&1 | tail -25`, then grepping that 25-line buffer as proof of a clean 3400-test suite

**Tool:** `run_command` · **Verdict:** wrong-tool (IL3 violation) · **Observed:** 2026-07-28,
self-inflicted by the assistant while gating a commit.

Ran `cargo test 2>&1 | tail -25` to "check the gate", then ran
`grep -cE 'FAILED|panicked at' @bg_xxx` against the resulting buffer and got `0`. Reported the
gate as green. The buffer held **25 lines**. The grep searched 25 lines of an ~18-binary,
3400-test run and returned a confident zero.

**Why this is worse than a plain IL3 violation.** The usual failure mode of piping to a trimmer
is losing information you then notice is missing. Here the trimmed output *still looked like
evidence*: a `test result: ok` line, a `Doc-tests` line, and a zero-match failure grep. Every
signal a reader uses to conclude "suite is green" was present in the 25 surviving lines, and all
of them were true — of 25 lines. The count of `test result: ok` was 2, which should have been the
tell (the run has 18 binaries), and it was not noticed until a later check happened to print it.

**What the server-side gate does and doesn't catch.** The IL3 enforcement blocked several
*other* attempts in the same session — `find … | head -5`, `git show --stat | head -12`,
`docker logs | tail` — with a clear message. It did not block this one, because the pipe was
constructed inside a `run_command` whose left-hand side was `cargo test` **and** the whole thing
was launched with `run_in_background: true`, so the string reaching the gate differed. Worth
checking whether backgrounded commands bypass the IL3 check.

**Correct pattern**, which the same session used successfully afterwards:

```
run_command("cargo test")                       # bare, full output buffered
grep -cE 'test result: FAILED|panicked at' @bg_x # 0
grep -E 'test result:' @bg_x | grep -v '0 failed' # empty = all clean
grep -cE 'test result: ok' @bg_x                 # 18 = every binary reported
grep -cE 'Doc-tests' @bg_x                       # 1 = run reached the end
```

The last two lines are the ones that matter and the trimmed version could not have produced
them: **assert the expected number of binaries reported, and that the run reached its end.** A
failure count of zero over an unknown denominator is not evidence.

**Prompt gap.** The Iron Law states the rule ("run bare, query the buffer") and the anti-pattern
list names the `cargo test 2>&1 | grep FAILED` shape specifically. What neither says is what a
*sufficient* green-gate assertion looks like. A reader who internalises "don't pipe" can still
write `grep -c FAILED @buffer` and stop there. Suggested addition to the
progressive-disclosure guide's run_command section: when asserting a suite is clean, assert
completeness (binary count, terminal marker), not just absence of failures.


### T-15 — A throwaway printing test beat reading the source, for an internal pure function

**Tool:** `run_command` · **Verdict:** legitimate · **Observed:** 2026-08-06.

Question: what does `nodes_to_chunks` actually emit when it decomposes a container?
Rather than derive it from the source — which was open, short, and readable — inserted a
throwaway `#[test]` printing every chunk's line span, metadata and content, and ran
`cargo test … -- --nocapture`. Two calls, then deleted.

**It contradicted the source-based prediction.** Reading the code, the coalesce run looked
padded by the container's trailing `}` gap. The dump showed the padding was a **leading**
gap spanning lines 1-8 — the entire file prefix before the container, emitted because the
recursion re-derives gaps against the whole `source` with `prev_end` reset to 0. That shape
was not predicted, and it is a second, pre-existing defect (now
`docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`).

It also produced the exact assertion strings — `src/mystore.rs :: impl MyStore ::     pub fn build(&self)`,
leading whitespace intact — that seven new tests now pin. Guessed strings would have
needed a round-trip each to correct.

**Prompt gap.** The rule exists but is scoped to the wrong things. This skill's Phase 1 says
*"For tools / external APIs: read the actual response shape, not docs"*, and
`get_guide("project-activation-bootstrap")` Phase 2 says *"A claim about how a TOOL behaves
needs the call run once and the real output read — reading the source alone misses runtime
shape."* Both frame it around tools and external APIs. The temptation is **strongest** for an
internal pure function, precisely because the source is right there and reading it feels
authoritative. Suggested widening: *"tools, APIs, and any function whose output shape you
are about to assert on."* Paired with W-1 in
`docs/trackers/release-promotion-session-log.md`.

### T-16 — Reporting a three-command local subset as "the gate"

**Tool:** `run_command` · **Verdict:** debatable · **Observed:** 2026-08-06.

Ran `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, found
them green, and proceeded through merge preparation treating the branch as gate-clean.

The **calls** are defensible — `--all-targets` is a superset of CI's `cargo clippy`, so that
part was stricter, not looser. The defect is the **inference**: three commands on one host
OS with default features were reported as the merge gate.

**What the real gate is.** `.github/workflows/ci.yml` runs a 3x3 matrix
(ubuntu/macos/windows × default/local-embed/no-features) plus `Format`, `Clippy`,
`Tool Docs Sync`, `MSRV`, `windows-gnu` and `Audit Doc Refs`. The three local commands cover
**one of nine** test cells.

**Proof standard met — the alternative was run.** Executing the two non-default configs
locally surfaced five genuine failures the default build cannot see: two compile-time
(`tests/link_scan.rs` and `src/server.rs`'s `make_server` using `librarian` symbols with no
`#[cfg]`) and three behaviour-time (`server::tests` asserting the `artifact` tool is
registered). Feature-gate rot is invisible to a default-features build **by construction**.
`gh run list --branch experiments` then showed failure on every run back to 2026-07-13.

**Prompt gap.** CLAUDE.md's *"Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`
before completing any task"* is per-task hygiene that reads as a complete gate, and
`docs/RELEASE.md`'s ship sequence reinforced it (*"tests passing, clippy clean"*) with no
mention of feature configs or of consulting CI at all. Partially closed: `docs/RELEASE.md`
now carries a *Large-Cohort Promotion* section listing all five steps with the ancestry
check first. Residual is a single local alias that runs all five — ROADMAP standing backlog;
narrative in F-3.
## read_file observations

### T-17 — Following the overflow hint literally, into an unsupported grammar

`artifact(find)` overflowed and returned `@tool_f99fdcd8` with the hint *"read_file("@tool_f99fdcd8",
json_path="$.field") to extract a specific field"*. Applying that hint to an array payload —
`json_path="$.artifacts[*].title"` — was rejected as `unsupported json_path segment '[*]'`.

The tool choice was **right**: `read_file` on the buffer is the documented recovery. The grammar is
a proper subset of JSONPath (`parse_bracket`, `src/tools/file_summary/file_summary.rs:570-626`,
accepts `.key`, `["key"]`, `[N]`, `[-N]`, `[-N:]` and nothing else). The working recovery was
`run_command("grep -o '\"title\": \"[^\"]*\"' @tool_f99fdcd8")` — leaving the tool surface for the
shell.

Not idiosyncratic. Across 13 merged `usage.db` corpora, `[*]` is **22 of 30** rejected segments
(73%), and two more used multi-field selection (`['id','title','rel_path']`) — so agents want
*projection*, not merely a wildcard. Tracing six instances showed **every** recovery the sequence
view scored as successful was a degraded workaround: 393 raw lines read to obtain one field;
`$.entries[4].id` one element per call; abandoning the buffer to re-run the upstream query.

The compounding matters more than the count: the tools with the **highest overflow rates**
(`librarian` 40.3%, `semantic_search` 20.9%, `artifact` 7.7%) are precisely the ones whose payload
is a list of records. The highest-overflow tools feed the weakest recovery path.

### T-18 — The guard refuses the read Iron Law 1 says is correct

`read_file(path, start_line, end_line)` on source was refused twice in one session with *"source
range overlaps named symbol(s)"* — on a 27-line slice of a method and a 95-line slice of a test
module. `force=true` returned both exactly.

The corpus makes the shape precise. Of 244 refusals carrying arguments (July onward): 97 small
slices (≤40 lines), **84 file-head reads** (`start_line` ≤ 5) of which **69 stop by line 60**, 56
medium, 7 large. The head reads are unambiguous:

    read_file("src/librarian/catalog/mod.rs",     1, 20)   refused
    read_file("src/librarian/workspace.rs",       1, 20)   refused
    read_file("src/librarian/current_project.rs", 1, 30)   refused

Lines 1–20 is the canonical *"show me the imports"* read — named in Iron Law 1 as permitted — and
it is refused because a `mod` or struct begins inside it. For these, `symbols` **cannot**
substitute: `iron-laws-detail.md:12-16` states it is a definition projection that does not return
imports/`use`/`package`. So on ~28% of refusals the hint's primary suggestion is structurally
incapable, and `force=true` — the only thing that works — is offered second.

**But the guard is healthy for the larger population,** and that is worth recording so nobody
relaxes it wholesale. Traced sequences show agents refused a blind line range going on to fetch the
exact symbols they wanted, by name:

    ERR  read_file("src/embed/ast_chunker.rs", 2076, 2189)
      N1 symbols(name="tests/split_file_rust_populates_metadata_headers", include_body=true)  ok
      N2 symbols(name="tests/inner_method_signature_skips_doc_comments", include_body=true)   ok

That is the guard working as designed — and the "same-tool recovery" metric scores it as a failure,
because the correct answer is a *different* tool. One guard, two populations: healthy for
symbol-body reads, structurally wrong for non-definition text.

## Prompt improvement candidates

### Input-shape frictions are repair candidates, not prompt candidates (2026-07-10)

A usage.db sweep (72 DBs) surfaced param-*shape* frictions — `file_path` for
`path`, inverted filter leaves `{op:{field,value}}`, buffer handles under
`output_id`. These are malformed-input mistakes (the agent picked the right
tool), **not** tool-selection mistakes — so they are deliberately NOT T-N
observations. They were resolved by **repair-and-continue** (auto-correct the
deterministic mistake + attach an advisory note) rather than a
`server_instructions` edit, because the correction note is self-describing at
the moment of the mistake. See ADR
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`. Rule of thumb: a
*deterministic* input mistake is a repair candidate; only a genuine
tool-*selection* gap is a prompt candidate.

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

### Iron Law 1 — state the overlap condition, not just the permission (2026-08-15)

**Surface:** `src/prompts/source.md:8-9` (the `server_instructions` slice — always loaded).

**Current text:** *"Line-range read_file is fine for imports/glue."*

**Problem:** a permission with its condition dropped. The gate refuses any range overlapping any
named symbol, which in a source file is nearly every line. The on-demand
`get_guide("iron-laws-detail")` is *correct* (*"Gate is overlap-based, not absolute"*) — so this is a
**compression defect**, not doc-vs-code drift: the condition lives only on the surface an agent has
to ask for, while planning happens against the one always in context. Measured cost: the largest
error family in the corpus (416 lifetime; 87 in August across 15 sessions).

**Candidate wording**, condition included and escape named:

    Line-range read_file is fine for imports/glue; on source a range
    overlapping a symbol is refused - pass force=true for an exact slice.

**Constraint:** the slice is capped at 2200 bytes (`src/prompts/README.md`), so something else in it
has to give. That trade is the open decision, not the wording.

**Do not pair this with relaxing the gate.** T-18 records that the guard is healthy for the
symbol-body population; only the non-definition minority is mis-served, and that is better fixed at
the gate's head-read case than by weakening it.

Evidence: `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md`,
`docs/trackers/2026-08-15-tool-usage-investigation.md` TU-2 / TU-11b.

### Progressive-disclosure guide — document the handle addressing grammar (2026-08-15)

**Surface:** `get_guide("progressive-disclosure")` (guide topic, not the byte-capped slice).

**Problem:** it documents the handle families (`@cmd_*`, `@tool_*`, `@file_*`, `@ack_*`) and never
the grammar for addressing into them, so the supported `json_path` subset is discoverable only by
rejection. Separately, the overflow envelope's hint models recovery as a scalar extraction
(`$.field`) while the payloads that overflow are lists — the hint cannot work for the result it is
attached to.

**Candidates:** (a) state the supported forms in the guide; (b) have the envelope emit a
list-shaped example when the buffered payload's top level is an array; (c) support `[*]` in the
parser, which alone covers 73% of observed rejections.

Evidence: `docs/issues/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`, T-17.
## History

### 2026-05-08 — filter.rs rel_path alias fix (T-010)
Added T-010. Root cause: schema v6 dropped `rel_path`/`repo` columns in favour of `abs_path`, but `ALLOWED_FIELDS` kept both dead names. LLM passed the documented `rel_path` filter example and hit `no such column`. Fixed by remapping `rel_path` → `abs_path` in `compile_leaf`; removed `repo` from allowlist. Two regression tests added.

### 2026-05-03 — Session c5daabbe analysis (eduplanner-ui error handling refactor)
Added T-005–T-008. Key findings: Iron Law #3 (piped run_command) is the most persistent violation — 7× in one session on `npm run build`. edit_file drift: model uses edit_code correctly then regresses to edit_file for same-type edits in a different file category. grep(^import) on specific files before editing confirmed legitimate.

### 2026-05-03 — Renamed and expanded from grep-usage-patterns
Expanded scope from grep-only to all tools. Added T-004 (semantic_search miss).
Proven via live `/codescout-companion:explore-project` run: semantic_search inferior
for scoped-directory multi-symbol discovery. Framing: internal Langfuse for tool decisions.

### 2026-05-03 — Initial population (as grep-usage-patterns)
First 3 observations from session 64618681 (Kotlin backend). G-001 verdict corrected
from debatable to legitimate after live proof.
