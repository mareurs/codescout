---
id: f2ecdd76a6189efb
kind: tracker
status: active
title: Tool Usage Patterns
tags:
- grep
- prompt-quality
- iron-law-7
topic: tool usage patterns audit optimization prompt quality
entry_high_water_T: 32
entry_prefix: T
expects_augmentation: docs/augmentations/docs-trackers-tool-usage-patterns.yaml
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

**Valid:** dated 2026-05-03

An observation of one session (`c5daabbe`, eduplanner-ui): the same piped-build pattern run
7 times, at those specific call indices. The count and the session are fixed facts about
that transcript. The *prompt gap* it names — Iron Law #3 lacking a concrete build example —
is a claim about the current prompt surface and would need its own entry to be rested on;
it is not what this class covers. Declared 2026-09-01.
**Session:** c5daabbe (eduplanner-ui, 2026-05-03)

Build verification after each edit batch. Model ran the same piped pattern 7 times (#48, #73, #77, #111, #113, #151, #159). Iron Law #3 prohibits piping run_command output — the buffer workflow exists precisely for this.

**Why it keeps happening:** `npm run build` produces verbose output; piping to grep is a strong developer instinct. The rule exists but has no concrete build example to anchor it.  
**Prompt gap:** Iron Law #3 needs a before/after build example: `npm run build 2>&1 | grep` ✗ → `run_command("npm run build")` then `grep "error TS" @cmd_id` ✓.

### T-33 — native `Read` on an `@tool_*` codescout buffer handle
**Session:** 75bf7137 (codescout self, 2026-09-03)

`doc(action="find")` returned a large result as `@tool_69062af3` with a `read_file("@tool_69062af3", ...)` hint. Called native `Read("@tool_69062af3")` instead — errored `File does not exist`, because `@tool_*`/`@file_*` handles are codescout-MCP-internal buffer refs, resolvable only by codescout's own `read_file`/`run_command`, never the native `Read` tool.  
**Verdict:** wrong-tool.
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

**Valid:** dated 2026-08-06
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

**Valid:** dated 2026-08-06

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

**Valid:** dated 2026-08-06
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

### T-19 — A `limit` turned "none of these" into "none exist"

`grep(pattern="^id: ", glob="docs/**/*.md", limit=12)` → *"Showing 12 of 12 matches
across 11 files."* All 12 quoted or `null`, so the claim written — and committed — was
that codescout's librarian guard "covers `docs/trackers/` and little else".

Counted without a limit: **50** files carry an unquoted 16-hex `id:`, the largest group
being `docs/issues/archive` (27), ahead of `docs/trackers` (12).

The glob was never at fault — the same glob with a narrower pattern found the tracker
on the first try. The denominator was. This is **BL-2** seen producing a false claim in
shipped prose, which is a different severity argument from the one it was filed on:
`Showing N of N` is byte-identical to the string a complete result prints, so a capped
sample is indistinguishable from an exhaustive one and there is nothing to notice.

**Caller-side habit, independent of the fix:** any grep whose result will support a
claim about *absence* must run without a limit. A limit converts "none of these" into
"none exist".

### T-20 — Three proxies for "what code is running", none of them the process

Asking whether a just-committed fix was live, `strings target/release/codescout` found
the new response fields, and that was taken as confirmation. The conclusion was right;
the evidence was not — `strings` reads the **on-disk binary**, which is not the image a
long-lived server process is running. That is the same reason `codescout_sha` is the
recommended discriminator, so the check reached for exactly the artifact the
recommendation exists to replace.

Measured in one minute, all disagreeing:

| signal | said |
|---|---|
| `codescout version` (on-disk) | `8ad83c42`, `git_dirty: true` |
| running server's `usage.db` rows | `536b9581` |
| code actually executing | neither — contained `2d8c7f39` |

What settled it was **calling the changed behaviour** and reading the response. Only a
tool call interrogates the process serving the call; every other signal is a correlate.
(The dirty-build half is filed as BL-24.)

### T-21 — The docs demonstrated the dangerous path for the narrow task

Flipping one row's status on a 24-row tracker required re-sending all 24 rows through
`patch={params:…}`, whose RFC 7396 array semantics **replace** the collection. That call
had already taken this queue from 19 rows to 1 earlier in the same session.

`artifact(action="update_entry", entry_id=…, fields=…)` now patches one row in place —
three rows flipped, `entries_total` 24 before and 24 after — and `artifact(update)`
reports `entries_before`/`entries_after` on *every* params write, so a mistaken replace
is visible in the response that performed it.

The prompt-side half is the sharper finding: **CLAUDE.md's own worked example for this
tracker was the read-then-write that causes the loss** (`params={observations:
[...existing..., new]}`), and its step 2 used `edit_markdown` on a managed file, which is
refused. Both corrected, with two librarian-guide sections that recommended
`artifact_augment(merge=true)` for adding rows.

General shape worth carrying: when a tool has a safe narrow path and a dangerous general
one, the docs must not demonstrate the general one for a narrow task.


### T-22 — A gate that approves the tool name is not a gate on the operation

I needed one row out of the queue tracker. `read_file` on it was refused by the IL4 gate,
which named `read_markdown` as the fix; `read_markdown` then **succeeded**. Both are wrong
for a librarian-managed tracker — the rule is `artifact(get, entry_filter=…)` — but the
successful call felt like compliance, because a gate had just approved the tool name one
step earlier.

The cost was immediate and concrete: the snapshot I read was stale on three rows, two of
them citing artifact ids that no longer resolve (archiving re-keys). Entering through the
catalog returns live params on the first call; entering through the file returns a
git-visible cache of them with no staleness signal.

**Then the interesting part.** The first draft of this entry asserted that no guard exists
for `read_markdown` on trackers, and proposed adding one. Checking before writing it up:

```
read_markdown("docs/trackers/tool-usage-patterns.md")   -> REFUSED (librarian-managed)
read_markdown("docs/trackers/open-issue-work-queue.md") -> 285 lines
```

The guard exists. It did not fire for the queue, and the reason is the finding:
`is_librarian_artifact` (`src/util/librarian_guard.rs:31-45`) tests the frontmatter TEXT
for an `id:` value of exactly 16 lowercase-hex chars. `id: '9a892c2a5976e296'` is 18 chars
with its quotes, so the file reads as unmanaged. In `docs/trackers/` alone: **12 protected,
15 unprotected** — more trackers are unguarded than guarded, split by a serialisation
choice no author made deliberately, and the unguarded set includes the queue this session
maintained all day.

The same shape-not-value check has a mirror-image failure: `tool-usage-patterns.md` is
protected while asserting `abc513d3ee0f0b50`, an id that resolves to nothing (live:
`f2ecdd76a6189efb`). Protected by accident, on a value that is wrong.

Filed as `docs/issues/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`.

**The transferable rule:** a refusal is informative; permission is not. A gate that fires
tells you something real about the file. A gate that stays quiet tells you only that its
predicate returned false — which may mean "not managed" or may mean the predicate cannot
see what it is looking at. Do not read silence as clearance, and when about to write up
"no guard exists here", run the call that would prove it.
### T-23 — `edit_markdown(action="edit")` given `content` instead of `new_string`, five times, deleting silently

**Tool:** `edit_markdown` · **Verdict:** wrong-tool · **Session:** `2c518eb6`

Five calls of the shape `edit_markdown(action="edit", heading, old_string, content=…)`.
`content` is the replacement key for `replace` / `insert_*` and is simply unread by `edit`,
whose key is `new_string` — which was read as `.unwrap_or("")`. So each call deleted the
matched text and returned `{"status": "ok"}`.

| File | Lost | Reached |
|---|---|---|
| `CLAUDE.md` | prompt-cap sentence | `a8fdf055` — repaired by a peer in `179c48a7` |
| `.codescout/memories/conventions.md` | prompt-cap line | `a8fdf055` |
| `src/prompts/guides/librarian-runtime.md` | the `hints` description | `9cdb2f50` |
| `src/librarian/prompts/companion_hint.md` | example JSON + bullet list | `9cdb2f50` |
| `src/prompts/source.md` | Iron Law 1's overlap clause | caught pre-commit |

Three of five reached commits with `fmt`, `clippy` and 3991 tests green: neither guide has
content assertions, and the body-shrink guard only fires past 50%. **Detection was
incidental** — the next action measured the prompt slice's character count and it had gone
*down* 61 when it should have gone *up* 57. Without a number to contradict, all five would
have shipped.

Worth noting how the peer's repair reads: `179c48a7`'s subject is *"finish `a8fdf055`'s cap
correction — repair the sentence, fix the sibling"*. That is a follow-up improvement, not
damage control — which is how a silent deletion enters the record as ordinary churn.

**Tool side, fixed** (`f05be046c6c373d8`, archived): all **three** read sites now require
`new_string` to be present, name `content` when it is passed instead, and refuse both keys
together. `new_string=""` still deletes — the explicit empty string is what separates asking
for a deletion from forgetting the replacement, which the old default could not do.

The filed root cause named only **two** of the three sites. A grep scoped to the tool that
*names* the parameter finds two; only a repo-wide search for the expression found
`update.rs:224` in `apply_body_edits` — the worst of them, because that is the
`artifact(update, patch={body_edits})` path and it edits trackers and bug files.

**Caller side, not fixable by any guard.** `edit_markdown`'s success envelope is
`{"status": "ok"}` plus a hint about unread sections, reporting nothing about what changed.
`artifact(update)` already emits `prev_bytes` / `new_bytes` on its `field_patch` event for
exactly this reason. Until an equivalent exists here: **read back any section edited via
`action="edit"` rather than trusting the ok.**

### T-24 — A `git log -S` zero that was about to overturn a peer's dated claim

**Tool:** `run_command` · **Verdict:** debatable · **Session:** `2c518eb6`

Reviewing a peer session's guide fix, which asserted *"`wc` came off the blocked list on
2026-08-16"*. To check it I ran:

```
git log -S 'wc|stat' --format=… -- src/util/path_security.rs
```

Exit 0, no output. **`-S` is a literal-string pickaxe** — without `--pickaxe-regex` the
alternation is searched as the five-character string `wc|stat`, which appears nowhere. The
empty result was a fact about my regex, not about the history, and I was one step from
reporting the peer's claim as unsupported.

Re-run in the correct shape:

```
git log -3 --pickaxe-regex -S '\bwc\b' -- src/util/path_security.rs
→ be4a679b 2026-08-16 fix(il3): stop blocking wc on source — it returns a count, not content
```

Their claim was exactly right.

Kin to **T-19**, with the failure one layer earlier: T-19's zero came from a *limit*, this
one from *syntax*, and both are indistinguishable from a substantive zero at the call site.
No tool fix applies — `git log -S` behaving literally is correct and documented.

**The rule, with its risk multiplier named:** this zero was about to be used to *contradict
another agent's specific, dated, falsifiable claim*. A zero that would let you overturn
someone else's measurement earns one confirming query in a different shape before the
conclusion — because the cost of being wrong is not a re-run, it is a wrong correction
entered into the record.
### T-25 — IL-3 refusals are 42% of all tool errors, and the error text is not the gap

**Tool:** `run_command` · **Verdict:** wrong-tool · **Session:** `8a62140a`

**Measured** on `.codescout/usage.db`, the 30 hours to 2026-08-19T19:00Z:

| | last 30h | prior 30h |
|---|---:|---:|
| calls | 5087 | 5441 |
| errors | 175 (3.44%) | 218 (4.01%) |
| `il3_pipe_to_trimmer` | 50 | 47 |
| `il3_shell_on_source` | 24 | 44 |
| next-largest family (`librarian_managed_artifact`) | 17 | 23 |

**74 of 175 errors — 42% — come from one gate.** The overall error rate is *falling*
(3.44% against a stable ~4.0% baseline), so this is the steady state, not a regression.

**Why it is not an error-text problem.** The IL-3 refusal is close to an exemplary
`RecoverableError`: it names the `@cmd_*` buffer workflow, lists the allowed bounded-LHS
commands, enumerates the unbounded producers, and adds the subtle case — that `git` is
unbounded only *without* an output limiter, and that `--oneline` is not one. Nothing about
that text needs improving.

The tell is repetition. **5 of the pipe refusals and 4 of the shell-on-source refusals are
a repeat of an identical `(tool, err_family)` failure within five minutes** — 9 of the 20
repeat-failures in the whole window. An agent that read the remediation and still
re-composed the same shape is not under-informed at *read* time; it was under-informed at
*compose* time. The author of this entry hit the gate twice in this session while holding
the full Iron Law text in context.

**The specific gap.** Iron Law 3's always-loaded form is one line plus a parenthetical:
*"NEVER pipe unbounded run_command → run bare, query @cmd_* buffer"*. It covers the
canonical `cargo test | grep FAIL`. It does **not** cover the two shapes agents actually
reach for:

1. `<long-running thing> 2>&1 | tail -5` — the intent is bounding *output size*, not
   filtering, and the rule as written does not read as applying.
2. `git log --oneline | head` — `--oneline` looks like a limiter and is not.

Both appear only in `get_guide("iron-laws-detail")`, which is not loaded at the moment a
command is being composed.

**Two candidate fixes, in preference order:**

1. **Cheap and testable — make the gate echo the rewrite.** Have the refusal lead with the
   corrected form of *the command it just rejected*, not the general rule. This targets the
   repeat-within-5-minutes path directly and costs nothing in the prompt budget.
2. **Surface the two wrong shapes by example in the `source.md` slice.** ~120 characters
   against the 1900-char cap, aimed at 42% of all tool errors. Competes for slice budget,
   so it should be landed *after* (1) and measured against the same `usage.db` query —
   otherwise the two changes cannot be told apart.

**Do not** read this as an argument to loosen the gate. Both IL-3 conditions fired
correctly on every one of the 74 occasions, including against the author of this entry.

---

#### Correction 2026-08-20 — the volume is right, the word "friction" is wrong

This entry counted error **rows** and reported them as **friction** — and the project had
already recorded that this is the wrong move, **twice, before this entry was written.**

> `2026-08-15-tool-usage-investigation:TU-7`, 2026-08-15: *"Ranking error families by count
> puts `il3_pipe_to_trimmer` (262) near the top. It is **healthy**... So frequency is not the
> signal — recovery is."* Its augmentation prompt states it was *"recorded specifically so it
> is not re-litigated by someone ranking error families by count."*

`2026-08-16-iron-law-gate-firing-audit` then replicated TU-7 (96% recovery / 3% immediate
repeat) and its own read contract repeats the warning. T-25 was written 2026-08-19 and did
exactly the thing both documents exist to prevent. The 2026-08-20 pass below then re-derived
TU-7's conclusion from scratch, across five dispatched agents, at roughly 150k tokens —
spending real budget to rediscover a negative result that was on disk, indexed, and
citable the whole time.

**That is the finding worth keeping from this correction**, more than the numbers: a
well-written negative result with an explicit do-not-re-litigate note did not survive four
days. It is a live datapoint for `capability-proposals:CAP-7` (make record decay detectable
so corrections travel) — the decay here was not a stale *fact*, it was a stale *warning*,
which no freshness check would flag. Anyone ranking `err_family` by count should be routed
to TU-7 before they publish.

What the 2026-08-20 pass adds beyond TU-7 is scale and a base rate: a detector-validation
pass over **840 matched errors — 74.5% of the 30-day error corpus**, joined to transcripts
by exact `(tool_name, input_json)` equality, measuring what actually happened after each
one. Friction rate by family, against a **27.0% corpus base rate**:

| err_family | n | friction rate | lift |
|---|---:|---:|---:|
| `il2_structural_edit` | 47 | 4.3% | **0.16x** |
| `il3_shell_on_source` | 119 | 13.4% | **0.50x** |
| `il1_read_overlaps_symbol` | 82 | 18.3% | 0.68x |
| `il3_pipe_to_trimmer` | 188 | 19.1% | **0.71x** |
| **`err_family IS NULL`** | 62 | **53.2%** | **1.97x** |
| `read_markdown_overflow_threshold` | 12 | 58.3% | 2.16x |

**Every Iron-Law gate is below base rate.** Median calls-to-recovery across the whole
corpus is **1**. So the gates are simultaneously the highest-volume *and* the healthiest
errors here, and the 96%-comply / 47%-re-offend split this entry treats as a tension reads
more dully: re-offence is real, frequent, and largely harmless. (The friction label is that
pass's own construct — retry of the same mistake, or >=3 calls to recover, or no recovery
within 25 calls. Absolute rates move with the threshold; the ordering does not.)

**Who commits them is not who this entry assumed.** 72.7% of calls and 68.3% of errors in
`usage.db` originate in dispatched **subagents**, filed under the parent's `cc_session_id`
with no column to separate them — and Iron-Law violations concentrate **75-88%** in those
subagents, which never receive the parent's injected guides. That is a *dispatch-briefing*
fix (Iron Law 6, measured), not a `source.md` slice edit.

**And this entry's own denominator is suspect.** 30.9% of rows (8,980/29,103) carry a
possibly-wrong `cc_session_id`, so every per-session figure above is a parent/subagent
blend. Filed as
`docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md`.

**What survives:** fix (1) — have the gate echo the rewritten command — is still cheap,
still costs no slice budget, and still targets the repeat-within-5-minutes path. **What is
withdrawn:** fix (2). Do not spend 1900-char slice budget aiming at the corpus's healthiest
class. The priority queue for *new* gate authoring is `err_family IS NULL`, whose largest
member is the worktree/activate write block at 22 hits — see T-26 and
`docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md`
(fixed in `4c7608ee`: 69 of 73 classified — though note the NULL bucket turned out **not** to
be high-friction either, so this was taxonomy hygiene rather than the priority queue this
entry called it).

### T-26 — grepping transcripts for friction is a wrong-tool choice with a measured false-positive rate

**Tool:** `grep` (on transcript JSONL) vs structured telemetry · **Verdict:** wrong-tool · **Session:** `851504c5` + 5 dispatched POV agents

Four attempts to measure agent friction from Claude Code transcript text, each on a
different signal, each returning a plausible number and no error — which is why nothing
raised:

| Attempt | Result |
|---|---|
| Keyword search for user corrections (`no,`, `wait`, `don't`, `that's wrong`) over 154 user-role entries | **67 hits, 0 genuine.** Every one a synthetic wrapper: skill preambles (`Base directory for this skill:`), task-notification blocks, `<local-command-caveat>`, compaction summaries. After an exclusion list: **3** genuine corrections across 4 large sessions |
| Text-grep `This call is blocked` to count hook denials | **~4x overcount** (13 raw hits vs 3 real denials in one file) — the phrase also appears in prose *discussing* denials, including subagent reports about this very investigation |
| `is_error` as a friction flag | Dismissed as useless (1 hit in 683 entries) — **wrongly.** It is absent on codescout `RecoverableError`s *by design* (`RecoverableError -> isError:false`) and present on **153/153** hook denials. Two mechanisms, two records; one grep sees neither cleanly |
| `thinking` blocks as the substrate for logic-friction detection | **0 of 4,906 carry text.** Opus-5/Sonnet-5 emit an encrypted `signature` only; just `claude-sonnet-4-6` emits plaintext. ~10M reasoning tokens generated and discarded |

**The structured fields were there the whole time.** `usage.db`'s `err_family` (26 families,
backfilled, fingerprint-versioned) and Claude Code's own `toolDenialKind` (present on every
denial, `permission-rule` vs `user-rejected`) both predate every grep written above, and
both were reached only *after* a grep had already produced a number.

**Corollary that cost real work: the phrase families an agent guesses are the weak ones.**
Measured over 4,180 narration blocks — `actually` fired **337** times at ~25% precision;
`wait` fired **once**; while first-person error admissions (`I was wrong`, `I blurred`,
`misread`) fired **12** times at **100%** precision, and self-correction verbs (`corrects
my`, `contradicts what I`) **39** times at **100%** on the sampled subset.

**Prompt gaps.** *General:* when the question is about tool-call behaviour, the structured
telemetry is the instrument and the transcript is the corroborator — not the reverse. No
prompt surface says so, and `analyze-usage`'s nine canonical queries contain no
error-taxonomy query keyed on `err_family`, so an agent asked to "measure frictions" has no
obvious structured entry point. *codescout-specific:* the `RecoverableError -> isError:false`
convention is documented as an **error-handling** decision and nowhere as a **measurement**
consequence — an analyst reading transcripts cannot know that codescout's largest error
population is invisible to the protocol flag. One line in `get_guide("error-handling")`
would close it.

**Instruments filed as bugs:**
`docs/issues/archive/2026-08-20-cc-py-cost-double-counts-split-assistant-entries.md`
(2.1-2.6x cost
inflation),
`docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` (**52.5%**
of errors unattributable — the "38%" first recorded here was relayed without recomputation;
the alias half is fixed and archived in `db76f69a`, the `command` half is target-less by
decision),
`docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md`
(30.9% of rows mis-attributed).
### T-27 — `grep -n` on a `@tool_*` buffer numbers the buffer, not the source

**Tool:** `grep -n` on a `@tool_*` buffer · **Verdict:** wrong-tool

**Valid:** invariant

**What happened.** Scouting `surface_lib.py` before dispatching Task 3 of the
hidden-information eval, I ran `symbols(path=..., include_body=true)`. The result
overflowed into buffer `@tool_37834a1f`, so I followed with `grep -n "def " @tool_37834a1f`
to locate the functions — and read the returned line numbers as source lines. They are
offsets into the buffered JSON document. I briefed a subagent that `run()` was at
`surface_lib.py:427`, `collect_facts()` at `:221` and `log_run()` at `:371`. The file is
**203 lines**; the real definitions are at 181, 97 and 151.

**Why it is a trap rather than mere carelessness.** A `symbols` response carries *both*
kinds of number, in the same payload, formatted alike. Its summary lists true source
ranges as data — `Function 40-41 _response_raw`, `Function 44-53 split_facts` — while a
`grep -n` over the buffer holding that same response yields document offsets. Nothing
labels which is which. And one coincidence made the wrong set look independently
confirmed: `split_facts` genuinely is at source 44–53, and the buffer grep also reported
line 53 for it. A spot-check on the first hit would have passed.

**Cost, and why it was not worse.** Zero code impact. Every *behavioural* claim in the
brief — what `run()` does with the facts dict, that `log_run` formats five keys outside
the try/except, that `run_arms.py` builds one env for all arms — was read from bodies and
was correct. The implementer verified the citations against source, found the three bad
line numbers, and reported them rather than quietly adapting. That is the behaviour the
dispatch brief asked for, and it is the only reason this is an anecdote instead of a
finding.

**The rule.** Buffers are for retrieving content the summary elided — never for locating
it. To place a symbol, read the range `symbols` already returned, or `grep -n` the source
file. Positional metadata belongs to whatever artifact you numbered.

**Generalises past `symbols`.** Any tool whose output embeds positions — a diff with
`@@` hunks, a compiler log, a test report — has the same two coordinate systems inside one
buffer. This is the same error class as reading a measurement off a working tree with a
live mutation in it (`prompt-surface-measurement-session-log:W-5`): a number that is real,
from a surface that is not the one the claim is about.

### T-28 — `sqlite3` on a store file answered five questions about a store that was not live

**Tool:** `run_command` (`sqlite3 .codescout/embeddings/codescout.db`) where
`index(action="status")` / `index(action="verify")` was the right instrument.
**Verdict:** wrong-tool. **Severity:** high.

**Observed.** Across one session I answered roughly five questions about "the
index" by querying `.codescout/embeddings/codescout.db` directly: per-directory
coverage, per-language max chunk size, chunk↔vector holes, orphan count. On this
host that file is a **retired** store. `CODESCOUT_VECTOR_BACKEND` is unset,
`VectorBackend::resolve` (`src/retrieval/code_store.rs:247-262`) defaults to Qdrant
when `server-stack` is compiled in, and `CODESCOUT_QDRANT_URL` is set and
answering. The file's mtime predated the session entirely.

| | live tool (Qdrant) | `codescout.db` |
|---|---|---|
| distinct files | 1611 | 1593 |
| chunks | 47 647 | 46 979 |
| paths absent from disk | 6 | 18 |

**Ideal.** Ask the tool that *owns the backend resolution*.
`index(action="status")` was already available and reports the live store;
`index(action="verify")` now exists for the coverage questions specifically
(`e5821fec`). Reach for a store file only after establishing which store is live —
and then prefer the tool anyway, because it cannot be wrong about its own
substrate.

**Why no gate fired — the prompt gap.** Three independent reasons, and each one is
why this is a *prompt* observation rather than merely a mistake:

1. A bounded `sqlite3` read is not IL-3-blocked, correctly — it is a cat-like
   producer, not an unbounded one piped to a trimmer.
2. Every query succeeded and returned internally consistent numbers. No zero, no
   empty result, no error. `docs/PROBES.md` rules 3 and 4 both need something to
   catch on, and nothing was anomalous.
3. I ran a **positive control** on the join — a deliberately-broken key returned
   all 46 979 rows — and it passed, which felt like verification. It verified the
   predicate, not the database.

**Candidate fix.** Have `index(action="status")` name the resolved backend in its
envelope (`"backend": "qdrant" | "sqlite-vec"`). That makes the substrate visible
at the exact moment someone is forming a belief about the index, costs a handful of
bytes, and is the one signal that would have short-circuited all three reasons
above. Nothing in any prompt surface currently warns that
`.codescout/embeddings/*.db` may belong to a backend that is not in use.

**Status:** open.
**Refs:** `bug-fix-session-log:F-66`, `reconnaissance-patterns:R-116`.

### T-29 — Two OVERLAPPING `read_file` ranges, stitched as if continuous

**Tool:** `read_file(path, start_line, end_line)` × 2 — **verdict: wrong-tool**

Read `scripts/build-windows.sh` as `(1-45)` and then `(45-62)`, and treated the two
results as one continuous read. Line 45 came back in **both**, so the stitched text held a
duplicated `#` line that exists nowhere in the file. The `edit_file` anchor built from it
failed with `old_string not found`.

**Why it is worth an entry:** the failure surfaces one layer away from its cause, wearing
the costume of a different bug. `old_string not found` is exactly what a *stale* or
*concurrently modified* file looks like — and a peer session was in fact writing to this
repo at the time, so the wrong diagnosis was not merely available, it was plausible. What
prevented it was `edit_file` printing the real bytes in its error, which made the
non-existent duplicate line obvious in one round-trip.

**Ideal:** one call spanning the range, or `grep` for the anchor to get its exact bytes. If
two calls are genuinely needed, make the ranges **disjoint** (`1-44`, `45-62`) — an
off-by-one in the join between two reads is invisible, whereas an off-by-one inside a
single range is not.

**Prompt gap:** `read_file`'s contract is per-call and says nothing about composing calls,
so nothing warns that overlapping ranges duplicate their shared lines. This is the same
family as `bug-fix-session-log:W-63` — a reconstruction of what a tool returned, mistaken
for what the artifact contains.

### T-30 — `symbols(include_body=true)`'s rendered indentation copied straight into `edit_code`

**Tool:** `symbols(name_path=..., include_body=true)` → `edit_code(action="replace"/"insert", body=...)` — **verdict: legitimate, shape-mismatched**

Twice in one session, copied a symbol's body as shown by `symbols(include_body=true)` into
an `edit_code` call reproducing it (once a doc-comment block, once a whole function body via
`action="replace"`). Both times the write succeeded — syntactically valid Rust — but carried
one extra indent level relative to the file's actual bytes, caught only by `cargo fmt --check`
immediately after, never by inspection.

**Why it is worth an entry:** the failure is silent at write time, same shape as T-29 — a
reconstruction of what a tool *displayed*, mistaken for the file's bytes. `symbols` is a
pretty-printer (consistent, readable indentation for humans scanning many symbols at once);
it was never a promise that the returned whitespace matches the source file's own. Recurring
twice in one session, on two different symbols, argues the pattern rather than a fluke.

**Ideal:** for a whole-symbol `edit_code` `replace`/`insert` that reproduces multi-line
content, verify indentation against a raw byte read (`read_file(..., force=true)`) before
writing, or treat `cargo fmt` as a mandatory mechanical follow-up to any such call rather than
an optional gate check run later.

**Prompt gap:** `symbols`' `include_body=true` documentation ("pass include_body=true for the
full source") carries no caveat that the returned indentation is a display rendering, not
verified byte-identical to disk — the same class the reconnaissance skill already names
generically ("a rendered read is evidence about content, not about bytes"), but `symbols`'
own surface doesn't point a reader there for this specific case.
### T-31 — `memory(write)` used to ADD to a topic, which replaces it wholesale

**Session:** cross-machine catalog restore, 2026-08-28  
**Tool:** `memory`  
**Verdict:** wrong-tool

Called `memory(action="write", topic="gotchas", content=<two new sections>)` intending
to append. It replaces the topic: **391 lines / 17 sections became 66 / 2** — an 83%
loss — and returned a bare `{"status": "ok"}`. Fifteen sections including
`MCP Binary Symlink`, `Cherry-Pick SHA Discipline` and `Kotlin LSP Circuit-Breaker`
were destroyed.

The correct tool was `edit_markdown` on `.codescout/memories/<topic>.md`, which is
exactly what the recovery used. Recovered only because the file is git-tracked —
timing, not a defence — and the `.anchors.toml` sidecar churns alongside, so a
restore must take both files.

**Prompt gap.** `write` is listed beside `read` with no note that it is destructive,
and there is **no `append` action** to reach for instead. A caller who has just READ a
topic and wants to add to it has exactly one verb, and it silently means replace. The
artifact side already solved this: `artifact(update, patch={body})` carries a 50%
shrink guard needing `force=true` and emits a `field_patch` event with
`prev_bytes`/`new_bytes`. `src/tools/memory/mod.rs` has no equivalent — a grep for
`shrink|prev_bytes|guard|truncat` finds only output-side hits.

Filed: `docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md`
— **fixed and archived the same day** (`experiments` `5b7b82cc`, patch-id
`4477be7feb16fad3ff16b9dfabaa1e884a3ca53e`). The tool now refuses a write that
would drop a topic below half its bytes, unless `force=true`. The wrong-tool call
this entry records is the one that motivated the guard.

### T-32 — grep used as a proxy for a tool call it cannot stand in for

**Session:** cross-machine catalog restore, 2026-08-28  
**Tool:** `grep`  
**Verdict:** wrong-tool

Three times in one session, grep answered a *different question* than the one asked,
and each time returned a plausible **number** rather than an error — which is what
makes the class dangerous.

1. **Counting bare `---` to find duplicate frontmatter** → 22 affected trackers, **all
   false**. Session logs use `---` as an entry separator: same string, different
   position. Counting a frontmatter-only key (`kind:`) gives the true answer, **1**.
2. **Filtering `doctor` output by an absolute project path** → matched **nothing**,
   reading exactly like "this project is clean". Response paths are relativized, so
   project-internal ones are relative and only foreign repos stay absolute. The filter
   tested a string form the data never uses.
3. **Ranking trackers by documented-query count** → line-scoped grep said **0** and
   file-scoped said **5** for the *same* tracker. Neither is closer to the truth; both
   measure co-occurrence. Executing the `entry_filter` query was the only method that
   separated "queries this" from "mentions both", and it changed the worklist from 18
   trackers to 4.

**Prompt gap.** Iron Law 3 and the Search/Edit quickref route *between* grep,
`semantic_search` and `references`, but nothing warns that grep standing in for a tool
call measures **co-occurrence, not usage**. Candidate line: *"when a proxy and the real
call disagree, the proxy is measuring co-occurrence — run the thing."* Paired rule,
already in `get_guide("tracker-conventions")` for field detection and generalisable:
anchor detection on **structure** (line-start, a key prefix), never on a keyword, since
prose and field share a vocabulary by construction.

Recorded in memory `gotchas` § *Co-Occurrence Is Not Usage — Run The Query*.

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

### Iron Law 1 — state the overlap condition, not just the permission (2026-08-15) — MEASURED AND REFUTED 2026-08-18

> **This candidate is closed. Do not implement the wording below.** It shipped 2026-08-16, was
> deleted the next day by a character-budget refit (`391fdcdc`), was restored, and was then put
> through the subtract-and-measure protocol as prompt-hamsa **A-25** — where it lost. Reverted at
> `32b34efa` on `experiments`, behind an **inverted** guard test
> (`il1_does_not_carry_the_refuted_overlap_clause`) that fails if the clause returns. Everything
> below the ruling is kept because the *diagnosis* held up; only the *fix* did not.

| Arm (10 runs each, sonnet pinned, rule pre-registered at `e2fbefe2` before either arm ran) | Planned the refused bare line-range read |
|---|---|
| A — base, no clause | **10/10** — ship rule needed ≥ 3/10, so the deficit below is **confirmed** |
| B — with the candidate clause | **8/10** — ship rule needed ≤ 1/10 |

0/10 → 2/10 passing is Fisher p≈0.47: noise.

**Why it failed, which is the transferable part:** the clause is *informational*, not *directive*.
It states the gate's condition but supplies no procedure — and an agent asked for lines 40-55 cannot
know whether they overlap a symbol without checking, so the clause hands over a fact it cannot act
on. The one passing arm-B run is the tell: it called `symbol_at` to resolve exactly that unknown.
Arm A corroborates from the other side — its ten answers disagreed on the *signature* (`lines=`,
`start/end`, `start_line/end_line`, `line_range=[..]`) while agreeing completely on the *permission*.
The always-loaded text leaves an agent unsure only about parameter spelling. Not one of the twenty
runs opened the fixture to check for overlap.

**The live successor is a different intervention, not a re-reading of A-25.** A *directive* wording —
something shaped like "on a mid-file range, pass `force=true` or fetch the symbol by name" — supplies
a procedure rather than a fact, which is precisely what A-25 found lacking. It needs its own
pre-registered A-N row and its own treatment arm; the base arm is already measured at 10/10, so a
successor does not have to re-run it. Fork
`prompt-engineering/scenarios/il1-overlap-condition/` (`prompt-engineering:f2f7958`).

---

**Surface:** `src/prompts/source.md:8-10` (the `server_instructions` slice — always loaded).

**Current text** (restored by the revert): *"Line-range read_file is right for imports/glue;
force=true overrides."*

**Problem — confirmed by arm A, and it is still open.** A permission with its condition dropped. The
gate refuses any range overlapping any named symbol, which in a source file is nearly every line. The
on-demand `get_guide("iron-laws-detail")` is *correct* (*"Gate is overlap-based, not absolute"*) — so
this is a **compression defect**, not doc-vs-code drift: the condition lives only on the surface an
agent has to ask for, while planning happens against the one always in context. Measured cost: the
largest error family in the corpus (416 lifetime; 87 in August across 15 sessions).

**Candidate wording — REFUTED, retained only so a later session recognises it and does not re-add it:**

    Line-range read_file is fine for imports/glue; on source a range
    overlapping a symbol is refused - pass force=true for an exact slice.

**Constraint:** the slice is capped at 1900 **characters** (`src/prompts/README.md`), so something
else in it has to give. That trade was costed in the bug file; the § Fix cap table there is retired,
and the character accounting in its § Evidence *Regression* is the live one.

**Do not pair this with relaxing the gate.** T-18 records that the guard is healthy for the
symbol-body population; only the non-definition minority is mis-served, and that is better fixed at
the gate's head-read case than by weakening it — which is what actually shipped: the head-read
exemption (`start == 1 && end <= 60`) plus the extent-ordered hint exempt 102 of 103 `start == 1`
refusals, are code with their own tests, and were never subject to the prompt-eval gate.

Evidence: `docs/issues/archive/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md`
(archived 2026-08-18; catalog id `b4d48dbfecc205c9` — the move re-keyed it, so any earlier id cited
for this bug no longer resolves),
`docs/trackers/2026-08-15-tool-usage-investigation.md` TU-2 / TU-11b, and
`docs/trackers/prompt-hamsa-audit-log.md` A-25 for the full pre-registration and outcome.
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

Evidence: `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`, T-17.
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
