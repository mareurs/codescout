---
kind: tracker
status: active
title: Architecture Boundary Measurement
tags:
- architecture
- measurement
- refactoring
- service-boundaries
topic: architecture-boundary-measurement
---

# Architecture Boundary Measurement

## Purpose

Measure the current coupling before deciding whether codescout should be split into
code-intelligence, execution, and knowledge services.

This is a measurement tracker, not an approved refactor. No architectural boundary is
settled, and no code should move until the baseline and decision criteria below are complete.

## Current hypothesis, not a decision

Keep one MCP server and one public tool surface while testing three internal ownership domains:

1. **Code intelligence** - LSP, tree-sitter, symbols, references, call graph, library navigation,
   and code-oriented semantic search.
2. **Agent execution** - workspace resolution, file operations, path security, write locking,
   shell execution, and change notification.
3. **Knowledge system** - markdown memory, librarian artifacts, links, trackers, provenance,
   freshness, and knowledge-oriented retrieval.

Treat retrieval and workspace identity as candidate shared substrates until measurements show
whether either belongs exclusively to one domain. Cross-domain operations such as `edit_code`,
`index`, `onboarding`, and workspace activation are candidate application workflows rather than
forced members of one domain.

## Frozen source populations

Use these exact path populations for the first baseline. Report unclassified Rust files instead of
silently assigning them.

### Intelligence

- `src/lsp/**`
- `src/ast/**`
- `src/tools/symbol/**`
- `src/tools/library.rs`
- `src/library/**`

### Execution

- `src/tools/read_file.rs`
- `src/tools/tree.rs`
- `src/tools/grep.rs`
- `src/tools/create_file.rs`
- `src/tools/edit_file/**`
- `src/tools/run_command/**`
- `src/tools/approve_write.rs`
- `src/util/path_security.rs`
- `src/agent/write_guard.rs`

### Knowledge

- `src/librarian/**`
- `src/memory/**`
- `src/tools/memory/**`

### Retrieval candidate substrate

- `src/retrieval/**`
- `src/embed/**`
- `src/tools/semantic/**`
- `crates/codescout-embed/src/**`

### Orchestration candidate substrate

- `src/server.rs`
- `src/agent/**`, excluding `src/agent/write_guard.rs`
- `src/tools/core/**`
- `src/tools/config/**`
- `src/prompts/**`

## Measurements owed

### 1. Static dependency edges

**Unit:** one unique `(production source file, target top-level module)` edge. Separate `use`
imports from concrete calls where possible. Exclude test-only modules and generated code.

Report:

- directed edge counts between every population;
- the raw edge list, so the totals can be independently recounted;
- concrete cross-domain references that bypass a trait or service interface;
- population-level cycles;
- unclassified files and feature-gated blind spots.

**Positive control:** `src/server.rs` must be observed importing or constructing registered tools.
If the instrument cannot find that edge, stop.

### 2. Runtime and context reach-through

Use the live registered tool list as the denominator. Do not copy a tool count from documentation.

Report:

- each `ToolContext` field and the distinct registered tools whose production call path reads it;
- each tool's capability set;
- tools touching 0, 1, 2, 3, and 4+ capabilities;
- direct `ctx.agent` reach-through sites, grouped by called Agent method or field;
- cross-domain workflow tools and their actual call paths;
- the separate librarian adapter context, rather than pretending it is the same type.

**Positive control:** verify one known pair, such as the workspace tool reading Agent state. Direct
field access and indirect dependence are different measurements and must not be merged.

### 3. Historical co-change

Freeze one shared window with:

```text
git rev-list --max-count=500 HEAD
```

For every SHA in that set, classify changed tracked `.rs` paths using the frozen populations.
The unit is one commit touching at least one Rust file in a population.

Report:

- per-population commit counts;
- every pair's intersection and union;
- Jaccard similarity;
- both conditional directions, `P(B|A)` and `P(A|B)`;
- commits touching three or more populations;
- one manually inspected multi-population commit as a positive control.

Stamp the exact `HEAD`, branch, command time, and whether tracked Rust files differ from `HEAD`.

### 4. Feature and dependency weight

Without building, run `cargo tree -e normal --prefix none --format '{p}'` for:

- `--no-default-features`;
- default features;
- `--no-default-features --features server-stack`.

Count unique exact package lines after sorting. Positive controls:

- librarian-only dependencies such as `serde_yml`, `pulldown-cmark`, and `jsonschema` are absent
  from lean and present in default;
- `qdrant-client` is absent from default and present in server-stack.

Record installed, debug, and release binary sizes if present, but state explicitly that file size
without build identity is not evidence about current `HEAD`.

### 5. Baseline verdict

Do not turn a large number into an automatic extraction recommendation. For each proposed boundary,
answer separately:

- Does it reduce source dependency edges?
- Does it reduce ambient `ToolContext` or Agent reach-through?
- Does history show independent change?
- Does it remove meaningful optional dependency or binary weight?
- Does it isolate a measured failure mode?
- What cross-domain workflow becomes more complicated?

The first implementation candidate remains an internal boundary with one MCP server. A crate or
process split requires evidence beyond directory size.

## Rejected evidence - do not reuse

Three exploratory subagent reports from 2026-09-10 were rejected and carry **no baseline value**:

1. A static-edge report returned `~16` edges, `4` concerns, and `0` cycles without stamping `HEAD`
   or providing a recountable edge population. Approximation is not a measurement.
2. A context report used generic search despite being instructed to use codescout, mixed direct and
   indirect `workspace_override` use, and contained internally inconsistent tool/capability totals.
3. A history report did not execute its proposed commands. It returned expectations as results and
   claimed a `HEAD`/timestamp combination incompatible with the current September 2026 history.

These reports are useful only as examples of failure modes. Re-run every number from scratch.

## Measurement discipline

Apply `docs/PROBES.md` literally:

- name what each predicate counts;
- stamp the tree and instant;
- include a known-positive control;
- treat truncated output as a floor;
- when two measurements disagree, rerun instead of explaining the disagreement;
- distinguish direct references, indirect capability dependence, historical co-change, and runtime
  failure coupling rather than collapsing them into one score.

No single "coupling score" is planned. The baseline is a table of independently interpretable
measurements.

## Resume protocol

Read **Status** and **Bounded baseline and verdict — 2026-09-13** first. Confirm home workspace and fresh git status before doing new work. Do not restart broad measurement merely because shared HEAD moved: preserve the named frozen snapshot and version any changed source-population definition. Design approval is next; runtime implementation is not yet authorized.
Derived 2026-09-16 from the slice records and the bug ledger below. Deliberately **unprefixed** — `T-N`, `R-N`, `F-N` and `W-N` are owned by other ledgers, and an ID-shaped token here would read as a citation into one of them.

### Ready — evidence in hand

1. **[DONE 2026-09-16]** ~~Archive the six architecture-probe bug files.~~ All six read `status: investigating` and *"No fix commit has been made. SHA and patch-id are therefore not available"*. That blocker had cleared: the fix is `d3a2c24f`, which introduced `scripts/architecture-boundary-probe.py` and its 218-line control suite in one commit. **Prerequisite re-run at current HEAD: 14/14 pass, `self-test: ok`** — re-run rather than cited because `40fb2843` later touched the script (inspected: it only de-hardcodes the `--runtime-binary` default). Recorded `d3a2c24f` + patch-id `f7ee24322b07702e7e87e5f4e17d78422089f2d1` on each file, flipped to `fixed` **through the catalog** (a raw frontmatter edit does not reach it — BL-48), archived via `doc(action="move")`, and repointed the twelve stale citations the moves created — six paths **and** six ids, since `id = sha256(abs_path)` re-keys every row. Verified zero stale remaining, with a control showing the grep fires.

2. **[DONE 2026-09-16]** ~~Commit the `W-3` session-log entry.~~ Landed in `215a5cad`, committed by peer `9403d62d` alongside the six archive moves after the joint-archive guard deadlock (`152f17f5`). The other two dirty files are not ours: `src/librarian/tools/doctor.rs` is attributed to a live peer by `scripts/file-provenance.py`, and the audit `jsonl` is machine noise.

3. **[DONE 2026-09-16, by peer `29420e72`]** ~~`c617b7bbbf85fa0c` — mutation-probe cannot verify a multi-file uncommitted change.~~ Fixed in `d8268215`, archived in `8ab825d6`, while this list was being written. Archiving re-keys the catalog row — `id = sha256(abs_path)` — so the id above is the post-archive one and any id cached for this bug before then has stopped resolving. The probe now carries the **whole working tree** into the isolated worktree — tracked edits, deletions and renames as one applied patch, plus untracked files — instead of only `--file`, and **refuses rather than falling back to `HEAD`** when the patch will not apply, since a verdict from a tree that is neither `HEAD` nor yours describes code nobody has. On a shared checkout it deliberately carries peers' in-flight work too, so the isolated tree matches what your own `cargo test` would compile and an `INCONCLUSIVE` means your real run would also have failed. This is the limitation that forced slice 1's mutations to be staged into the probe worktree by hand.

4. **[DONE 2026-09-16]** ~~`e1aaab73c7d3ba5f` — acking a dangerous command silently drops `run_in_background`.~~ Fixed in `1f36fe26`, patch-id `6e78e0e32346a3aa815454852772111c9e484924`; archived, which re-keyed it by the same rule — the id above is post-archive. Operator chose **honour the flag**: `PendingAckCommand` gains the field, `store_dangerous` takes a fourth argument, and the ack dispatch passes `stored.run_in_background` where it hardcoded `false`. The `PendingAckWrite` `Value` form is the more general shape and was rejected as **unreachable rather than inferior** — `store_dangerous` is called from inside `run_command_inner`, which never receives the input `Value`, so carrying it means an eleventh parameter threaded through a ten-parameter function and re-parsed to recover one bool already in scope. A second parameter dropped the same way is the signal to switch: two concretes, where today there is one. Two tests, **paired so neither is monotone** — backgrounding *everything* passes the positive test alone. Gate green all four lanes. Cluster `IC-15`.

### Blocked on authorization — and on a measurement nobody has taken

Slice 4 is not a next step; slice 3's prerequisite is now discharged but slice 3 itself remains unauthorized.

5. **[PREREQUISITE DISCHARGED 2026-09-16]** ~~Slice 3 prerequisite — audit the unresolved helper paths.~~ Audited; see *Follow-up measurements — 2026-09-16* § *Slice 3's prerequisite*. **Negative result: the unresolved routes hide no context dependency.** All three genuinely-unresolved helpers are pure string/URI functions with zero `ctx` references (control: the same method finds `ctx` in `grep.rs`'s `call`); the five unresolved `edit_file` actions dispatch in a span with zero `ctx` references; `unresolved_live_tools` is 0. So the baseline's helper-expanded footprints are complete with respect to every unresolved route, and slice 3 can be **designed** against them rather than against a population with unknown holes. **This does not authorize slice 3** and does not pick its migration target — the footprints remain lexical and scoped to the five declared populations, and complete-with-respect-to-unresolved-routes is not a call graph. Found on the way: the audit population was **5, not 28** — `20807215e1c20c3d`.

6. **[PREREQUISITE DISCHARGED 2026-09-16]** ~~Slice 4 prerequisite — a crash-injection or shared-edit parity experiment.~~ Run; see *Slice 4 — prerequisite experiment RUN 2026-09-16*. **The result argues against the slice's framing.** The divergence is reachable, `reindex` repairs it, and `doctor` reports nothing — each with a control. So the gap is a missing **detector**, not missing recovery, and the consolidation the slice proposes would force two deliberately-opposite orderings (`create` catalog-first, `update` disk-first) onto one and break whichever lost. Recommended instead: a `doctor` check comparing `file_sha256` to disk. Filed `bd117fbc0d1a0308`. **Still only half-measured** — one writer pair injected; `move`, `append_entry`, `augment` and `edit_file`'s catalog sync are untouched.

7. **`bd117fbc0d1a0308` — a catalog row left behind by a failed update is repairable but invisible.** Filed 2026-09-16 from the slice-4 experiment; `cluster/selector-narrower-than-its-population`. Open. The recommended fix is a `doctor` predicate, not a change to either write ordering.

8. **`20807215e1c20c3d` — the probe counts Rust keywords as unresolved helpers.** Filed 2026-09-16 from the slice-3 audit; `cluster/addressing-without-an-escape-hatch`. **Fixed 2026-09-16 in `082b632b`** — patch-id `87a2185ff7b99cf6f0ebbd2d30b90290d524d947`, archived; the audited population is now **5 pairs over 3 names** rather than 28 over 12. Fixing it does not change any conclusion recorded above — it changes the number a future reader must audit to reach them.

## Status

**Phase:** bounded architecture review **complete — all four slices resolved**. Slice 1 built; slices 2, 3 and 4 scouted and deliberately NOT built, each leaving a test or a filed gap instead of a migration. Read **Bounded baseline and verdict — 2026-09-13** for the measurements, then the four slice records at the end of this section.

**Verification:** 14 probe regression tests and self-test pass. Four applied mutation candidates were detected, zero survived. Second full gate completed with FMT_EXIT=0, CLIPPY_EXIT=0, LEAN_EXIT=0, DEFAULT_EXIT=0, in the required order; logs are under `.codescout/measurements/architecture-boundary/2026-09-13/gate2-*.log`. The earlier formatter refusal/default attribution failure remain in the first-run logs. These gates do not establish server-stack coverage.

**Source bound:** frozen snapshot 23adef79023ebc43ea30d8d8e50a2175bacab5aa, not current shared HEAD; corrected measurement commands retained their exit-2 HEAD-change warning. No numeric finding is a whole-project/compiler-resolution claim.

**Next action: none in this work stream.** All four slices are resolved and both remaining prerequisites were discharged 2026-09-16.

| slice | outcome | artifact |
|---|---|---|
| 1 — job lifecycle | **built**, MCP-verified against the live binary | `f098069a` |
| 2 — semantic results | scouted, parked; boundary already existed, invariant pinned | `8a1a44b3` |
| 3 — request context | scouted, not built; pin already shipped, population guard added | `d4ab86e2` |
| 4 — recoverable mutations | experiment run; slice reframed, gap filed | `8a76f435` |

**Three of four ended the same way, and that is the review's actual finding:** the proposed boundary already existed or solved a problem the measurement did not find, and what was missing each time was the invariant that keeps what exists. The output is three tests and two bug files, not three migrations.

**The follow-ups this section used to list as owed are done** — missing-symbol latency repeated with startup and miss separated, unresolved action paths resolved, merges and renames characterised. See **Follow-up measurements — 2026-09-16**. Widening the frozen population remains deliberately not done, gated as a separately versioned measurement.

**Open, and not blocking anything here:** `bd117fbc0d1a0308` (a catalog row behind its file is repairable but invisible). **Closed since:** `20807215e1c20c3d` (the probe counted Rust keywords as unresolved helpers) — fixed 2026-09-16 in `082b632b`, archived, population 28 → 5. **The one experiment still owed** is the other four writer pairs — `move`, `append_entry`, `augment`, `edit_file`'s catalog sync — none of which was injected, so *"across actual sibling implementations"* is two compared and four unexamined.

The measurement work stream — probe, regression tests, this tracker and its seven bug files — is committed in `d3a2c24f`.

### Slice 1 — approved and implemented

**Approved 2026-09-15** by the operator, with one amendment to the design as proposed: the background call **returns at spawn time** rather than after the 5s warm-up. That amendment turned out to resolve a tension rather than create one — `format_run_command` renders an absent `exit_code` as "running", which is a claim that could be false when emitted after a wait and is true by construction when emitted at spawn.

Shipped in one change, because each part makes the previous one non-vacuous:

- `BufferInner.background_jobs` is `HashMap<String, BackgroundJob>`, not `HashMap<String, PathBuf>`. A path cannot answer "did it finish, and how".
- A supervisor task owns the `Child` and records `JobState::Exited { code }` / `Failed`. `drop(child)` gave the process to tokio's orphan reaper and discarded the status; there is no other channel it exists on.
- The 5s warm-up window and `BackgroundKillGuard` are gone; the call returns immediately.
- Job state reaches the caller through the **response envelope** (`OutputBuffer::job_states_in` → a `jobs` array). It cannot travel the `@bg_` channel: that resolves by textual substitution to a filename, which is why a `tail @bg_x` returns the reader's exit code and never the job's. A job record with no read path would have been `cluster/declared-not-wired`.
- Eviction consults liveness and never unlinks a running job's log.

**Verification.** Gate green on all four lanes (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`), own test names read out of the default lane rather than the total. **Three mutations, each KILLED** in an isolated worktree: eviction predicate → FIFO (2 tests red), `status.code()` → `Some(0)` (red, `last seen: "exited 0"`), `job_states_in(command)` → `job_states_in("")` (red). Mutating the production path, not test inputs.

**Committed 2026-09-15** in `f098069a` — patch-id `8658a129d49444e33a6fce955a2f8b498bb743d1`. Archived with it: `b9935bb5470a799c` (background command loses terminal status) and `52f03908f97a948a` (eviction unlinks a running job's log), both carrying a killed mutation; and `d44b0a9aa38738f5` (the raw-pid kill guard), archived on the weaker basis that its remedy was deletion, so no regression test is possible — its **Tests added** section names what would re-introduce it. Scouting friction recorded as `architecture-boundary-session-log:F-2`; an instrument limitation found on the way as `c617b7bbbf85fa0c`.

**MCP-verified 2026-09-15** against the live server, after `cargo rb` + `/mcp` reconnect. The gate and the mutation runs both exercise test harnesses; this is the shipped binary answering the filed bug's own reproduction. All four `JobState` renderings observed:

| call, then `cat <handle>` | `exit_code` (the READER's) | `jobs[0].state` (the JOB's) |
|---|---|---|
| `sh -c 'exit 7'` | 0 | `exited 7` |
| `sh -c 'echo alive; sleep 45'` | 0 | `running` — while the log streamed `alive` concurrently |
| `sh -c 'kill -TERM $$'` | 0 | `terminated by signal` |
| `sh -c 'echo done-ok; exit 0'` | 0 | `exited 0` |

Row 1 is the bug's reproduction inverted: the `exit_code: 0` that used to be the caller's only signal is still there and still the reader's, with the job's real outcome beside it rather than instead of it. Row 4 matters for the same reason — two zeros, in different fields, both correct.

**Row 3 is the one the suite could not buy.** `a_signal_killed_job_reports_no_exit_code_rather_than_zero` constructs `JobState::Exited { code: None }` by hand, so it asserts about its own re-implementation rather than about the production path; only a real signal death makes `status.code()` return `None` for real. Recorded because this is a case where the live check was genuinely not redundant with a green suite.

Every spawn response carried no `exit_code` key and read `Started; outcome not yet observed` — nothing claimed before it was observed, which is the defect's other half.

Noted while verifying, not filed against this slice: `kill -9` trips the dangerous-command gate, and ack re-dispatch is forced foreground, so `run_in_background` is dropped on the acked call. Pre-existing and deliberate at the site; filed separately.

### Slice 2 — scouted 2026-09-15, PARKED with its invariant pinned

**No code migration, deliberately.** The scout found **no live misclassification**, and this records that rather than dressing a latent coupling as a fire.

**The proposal's framing is off by one step.** It reads as *introduce a boundary*; the boundary already exists and one concern already migrated to it. `Tool::call_content` holds the typed `Value` from `self.call(...)`, and field-aware path-stripping runs exactly there — moved up after operating on rendered text corrupted file content and collapsed root fields to `""` (`docs/issues/archive/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`). Slice 2 is **finishing that migration**, which lowers both its risk and its urgency.

**The decomposition the proposal's wording hides — three facts, two kinds:**

| fact | kind | known at |
|---|---|---|
| `outcome` | semantic | `self.call(...)`, and the `Err` arm |
| `error_msg` | semantic | same |
| `overflowed` | **rendering** — `output_id` is minted by buffering | the buffered arm |

So *"classify earlier"* is the wrong instruction: one of the three genuinely is not knowable earlier. The move is per-fact.

**Blast radius, two independent instruments agreeing:** `references` reports 96 `call_content` sites across 13 files with exactly **one** in production; `src/server.rs`'s own doc comment independently asserts it *"has no other production caller."* Different instruments, different scopes.

**Why parked rather than done.** Classification is correct today **only** because the buffered arm emits JSON unconditionally — it has no `output_form` branch — so overflow and non-JSON rendering are mutually exclusive by construction. Nothing stated or tested that. Pinning it costs one test; migrating costs a signature change across ~95 test sites for a defect that does not exist yet.

**Shipped instead:** `usage::content_tests::the_renderer_and_the_classifier_agree_about_overflow`, which drives the real buffered arm rather than a hand-built envelope, plus three doc corrections. **Counterfactual measured:** under a mutation that breaks the invariant, that test reds and **17 of 18** in its module stay green — including two named for the property destroyed. Full derivation: `architecture-boundary-session-log:W-2`.

**Revisit-when:** a second consumer of tool outcomes appears (an eval harness, a retry policy), or a new `OutputForm` variant is proposed — at that point the envelope type this decision rejects starts earning itself, and the pinned test is what will fail loudly rather than silently.

**Confidence:** high that the seam is where it is and that the blast radius is one production caller — both read at the bytes. Medium on the mechanism for carrying `overflowed` back out. Low that it is urgent.

### Slice 3 — scouted 2026-09-16, RECOMMEND NOT BUILDING IT AS WRITTEN

**The proposal is off by one step in the same way slice 2's was, and further along.** Slice 2's boundary partly existed; slice 3's per-request pin **fully exists and shipped 2026-05-31**. What does not exist is the invariant that keeps it.

**Cited from the imports, not the diagram.** `ctx.workspace_override` has **324 references across 47 files**. `Agent::with_project_at` (`src/agent/mod.rs:793`) and `with_project_at_mut` (`:977`) are the selector-aware accessors. Every one of the 21 registered tools has a production source file referencing the pin except `doc` and `librarian`, which reach it through `LibrarianAdapter` — `src/librarian/adapter.rs:285` resolves `ctx.workspace_override` and, per its own comment, *"an unresolvable pin surfaces loudly instead of falling back"* (`docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md`). The owning plan, `docs/plans/2026-05-30-per-request-workspace-pinning.md`, records **"Phases 0–3 COMPLETE"** and **"4a COMPLETE"**.

**What the slice is actually about, once that is subtracted.** The text says *"build on existing workspace pins rather than pretending they are missing"* — so the author knew. The live proposal is the second clause: *avoid repeated ambient resolution*. That is real and now measured: **95 per-request resolution call sites across 25 production files**, worst-first `src/agent/mod.rs` 16, `src/tools/onboarding.rs` 15, `src/tools/memory/mod.rs` 14, `src/tools/semantic/index.rs` 10, `src/tools/config/mod.rs` 7. A single `onboarding` call can re-resolve project identity around fifteen times.

**Change scenario, named and checked against history rather than imagined.** *"A new tool or call site is added and forgets the pin, so work silently scopes to the session project."* It has happened and is archived: `8f500ba0f27723a4` (references / symbol_at / call_graph ignored the pin), `4574d18db7aacec8` (one process-global active project), `f73130523241a666` (pin at an unparseable config), `6779f47d3c986e9c` (activation guard used wall-clock proximity). **All fixed.** A resolved request object threaded through the API would make forgetting unrepresentable — a caller could not reach a helper without it.

**Why that is still not worth building.** The 95 sites are the cost, not the evidence: threading a request object through 25 files is a migration whose risk is concentrated in exactly the write paths whose correctness is least observable, and it buys prevention of a defect class that is currently at zero live instances. The cheaper instrument that catches the same class already has a house pattern here — `every_manual_page_is_reachable_from_summary`, `every_safety_comment_precedes_an_unsafe_construct`, `every_refusing_hook_emits_the_shared_tail`, `every_declared_feature_has_a_lane_or_a_reason`. **There is no `every_tool_honors_the_workspace_pin`.** The pin has per-site tests (`call_tool_inner_honors_workspace_override_for_security_config`, `an_out_of_band_edit_reaches_the_pinned_security_config_too`) and no population guard.

```
**Decision:** Do not build the scoped request object. Keep the existing per-request
    pin plumbing and add the missing population guard that makes a pin-blind tool
    or call site fail the build.
**Context:** The pin shipped 2026-05-31 across 21 tools and the librarian adapter.
    The remaining exposure is not a missing boundary but an unasserted invariant:
    95 resolution sites, no test that a NEW one honors the pin.
**Alternatives considered:**
    - Scoped request object threaded through 95 sites — rejected: large migration,
      risk concentrated in write paths, zero live instances of the defect it
      prevents, and it re-derives a boundary that already exists.
    - Do nothing — rejected: the defect class has four archived instances, so
      "it has not recurred" is a statement about attention, not about structure.
    - Population guard (chosen) — matches the established `every_*` pattern and
      reds exactly on the failure the migration would prevent.
**Consequences:**
    now easier: a new tool that skips the pin cannot reach master silently.
    now harder: nothing structural. The 95 call sites stay, so "resolve once"
      remains unachieved and a reader must still not mistake site count for cost.
**Change scenarios absorbed:** a new tool, or a new helper on an existing tool,
    resolves project identity without consulting the per-request pin.
**Revisit-when:** the guard reds twice for genuinely different reasons — that is
    two concretes for the abstraction, where today there are none; OR a second
    per-request dimension appears beside the workspace pin (a tenant, a session
    identity, a read-only flag with its own resolution), at which point a request
    object carries two things and stops being a wrapper around one.
**Confidence:** high that the pin is fully wired, read at the bytes. Medium on the
    95 figure as a COST estimate — it counts call sites, not the difficulty of
    threading them. Low that any of this is urgent.
```

**The limit of this scout, stated because the recommendation depends on it.** What was verified is that every pinnable tool's production source **references** the pin, and that the adapter resolves it. That is **not** the same as every tool honoring it on every path — presence of an identifier is not correctness, and no instrument here reaches the stronger claim. **That gap is the argument for the guard rather than a caveat on it:** the guard is what would establish what this scout cannot.

**Doc-vs-code drift corrected on the way.** `src/tools/core/types.rs:89` read *"No tool reads it yet — Phase 3 wires the selector-aware accessors"* — false since 2026-05-31, three and a half months stale, sitting on the field itself where a designer would look first. `cluster/doc-contradicted-by-code`.

### Slice 4 — prerequisite experiment RUN 2026-09-16

The tracker listed *"a crash-injection or shared-edit parity experiment"* as never performed, which made the file/SQLite ordering and rollback semantics claims rather than observations. Run. **The result argues against the slice's own framing.**

**The experiment the production code had already specified.** `src/librarian/tools/update.rs` carried a comment saying the failure join was *"NOT REACHED BY ANY UNIT TEST … needs real lock contention (a second connection holding `BEGIN IMMEDIATE` past the 5s busy_timeout) to exercise, which no test in this suite constructs."* That is the prerequisite, written into the source by whoever last looked. Three tests now construct exactly that — on-disk catalog, second `rusqlite` connection holding the write lock — and each takes ~5 s of wall clock, which is the `busy_timeout` being genuinely paid rather than short-circuited.

| question | answer | how |
|---|---|---|
| Is the divergence **reachable**? | **Yes** | file carries the edit, row keeps the pre-edit `file_sha256`; the error names both halves |
| Is it **recoverable**? | **Yes** | `reindex` reconciles the row against a freshly computed hash of the bytes |
| Is it **observable**? | **NO** | `doctor` never names the artifact |

**The sibling asymmetry the slice was reaching for is real, and it is deliberate on both sides.** `create.rs` orders catalog-first / disk-last so a catalog failure leaves no orphan file (BUG-058, with a `RAISE(ABORT)` trigger test). `update.rs` orders disk-first / catalog-last. Opposite, and each is right for its own case: `create` has nothing to lose if the file never lands, while `update` has a caller holding a patch that must not be blindly re-applied — which is precisely what `file_written_but_catalog_failed` exists to say. **Consolidating them onto one ordering would break one of the two.**

**The finding that changes the slice.** Recovery exists and nothing triggers it. The caller who receives the error is told exactly what happened; the party the recovery actually depends on — a later session reading the catalog — gets no signal, because `doctor`'s catalog-health family has no predicate over stored-hash versus on-disk bytes. `missing_file` is adjacent and does not cover it: a changed file is present. So the window is not *"until the next reindex"* but *"until somebody reindexes for an unrelated reason"*, which is not a bound. Filed as `bd117fbc0d1a0308`, `cluster/selector-narrower-than-its-population`.

**The control is what makes that silence a measurement.** A `doctor` that reports nothing because the tree is healthy and one that reports nothing because the question is outside its scan return the same JSON. So the same fixture is broken a second, known-detectable way — the file is deleted, which `missing_file` owns — and doctor **does** name the artifact. Silent on the divergence, loud on the deletion, same scan, same run.

```
**Decision:** Do not build slice 4 as scoped. The consolidation it proposes would
    force two deliberately-opposite orderings onto one, and the rollback machinery
    it budgets for is answering a durability problem the experiment shows does not
    exist. Make the state OBSERVABLE instead.
**Context:** Reachable, repairable, unreported — measured, not argued. The gap is
    a missing detector, not missing recovery.
**Alternatives considered:**
    - Reorder `update` to match `create` — rejected: `create` can afford
      catalog-first because it has nothing to lose if the file never lands;
      `update` cannot, and its caller holds a non-idempotent patch.
    - Two-phase commit / rollback across file and SQLite — rejected: prices a
      durable divergence, and reindex already reconciles.
    - A `doctor` check comparing `file_sha256` to disk (recommended) — makes the
      existing recovery triggerable without touching either ordering.
**Consequences:**
    now easier: a row that has fallen behind its file becomes findable, so the
      repair that already works can be aimed.
    now harder: hashing every artifact per run is O(corpus), and `doctor` is a
      manual cadence rather than a gate. An `file_mtime` pre-filter is the obvious
      narrowing and is ITSELF a selector that can be narrower than its population —
      the exact class this defect was filed under.
**Change scenarios absorbed:** a write fails between its file half and its catalog
    half and nobody is watching the session that saw the error.
**Revisit-when:** a second writer pair appears with the same split (a catalog row
    plus a non-file side effect — a vector upsert, a remote index), at which point
    consolidation has two concretes instead of one; OR the `doctor` check is added
    and its cost is measured against a real corpus rather than estimated.
**Confidence:** high on all three experimental answers — each has a control and an
    observed outcome. Medium on the recommendation's cost estimate, which is a
    guess about hashing a corpus nobody has timed. Low that any of it is urgent:
    the defect class has zero live instances.
```

**What this does NOT establish.** One writer pair was exercised — `doc(action="update")`. `doc(action="move")`, `append_entry`, `augment` and `edit_file`'s catalog sync each have their own ordering and none was injected. The slice's phrase *"across actual sibling implementations"* is therefore still only half-measured: two siblings compared, four unexamined. That is the next experiment, not a caveat on this one.

## Follow-up measurements — 2026-09-16

Run 2026-09-16, current HEAD, by `scripts/architecture-boundary-probe.py context` plus targeted checks. The probe exited **0** with `worktree_changed_during_measurement: false` — both 2026-09-13 runs exited 2, so this is the first run whose source basis is not disputed by a mid-run HEAD move.

### 1. Missing-symbol latency — repeated, and the diagnosis it was reserved for is FALSIFIED

The baseline recorded one missing-target edit at **134,088 ms** against a following successful edit at 156 ms, and correctly declined to diagnose it. What was owed was *"repetition with separated startup/miss costs"*. Both arms below spawn a **fresh server process** so startup is paid inside the measurement rather than assumed away.

| population | call | n | median | range |
|---|---|---:|---:|---|
| 2-file fixture | cold HIT | 2 | 43.2 ms | 33.9–43.2 |
| 2-file fixture | cold MISS | 2 | 37.9 ms | 34.3–37.9 |
| 2-file fixture | warm (hit or miss) | 8 | ~1 ms | 0.8–1.3 |
| full workspace | cold MISS | 3 | 3,381.6 ms | 3,245.7–3,627.9 |
| full workspace | warm HIT | 3 | 1,819.1 ms | 1,803.0–1,823.3 |
| full workspace | warm MISS | 3 | 1,647.1 ms | 1,636.0–1,673.6 |

**A miss is not more expensive than a hit, at either scale.** Cold: 37.9 vs 43.2 ms. Warm on the full workspace: 1,647 vs 1,819 ms — the miss is marginally *faster*. The implied "missing-target lookups are slow" reading has a measurement against it now, in both directions.

**Startup is real and bounded:** ~1.7 s on the full workspace, ~40 ms on a 2-file crate. **The 134,088 ms figure is not reproduced at any scale tested** — the largest single reading across every arm is 3,627.9 ms, ~37× smaller.

**What this does NOT establish, stated because the gap is the same shape as the original error:** it does not explain the 134 s. Two variables are untested — contention on a *shared* MCP instance serving six sessions (every run here used a dedicated fresh process) and a cold rust-analyzer **disk** cache (these runs almost certainly hit a warm one). So the outlier is un-reproduced, not explained, and nothing here licenses calling the original reading wrong.

**The repetition caught one of its own.** An n=1 pass of the workspace arm returned **8,679 ms** for a warm MISS — 5× the hit beside it, and exactly the shape that would have justified the original diagnosis. It did not survive n=3 (1,636–1,674 ms). Had that arm been reported at n=1 it would have *confirmed* the hypothesis the full run falsifies.

### 2. Unresolved edit_file action footprints — resolved, and the instrument's scope is why they looked open

Counts are unchanged at current HEAD: **67 action rows, 49 `textual_branch`, 5 `unresolved`, 13 `no_advertised_action_enum`** — identical to 2026-09-13.

All five unresolved rows are `edit_file`'s `replace`, `insert_before`, `insert_after`, `remove`, `edit`. **They are dispatched in code**, at `src/tools/markdown/edit_markdown.rs:270` and its siblings, inside `plan_section_edit` (`:125-334`), and enumerated as `SECTION_EDIT_ACTIONS` at `:67`. The probe could not see them because its branch scan is **same-file** and the dispatch lives in a different module from `src/tools/edit_file/mod.rs`.

**Context footprint: zero.** `plan_section_edit` spans lines 125–334 and contains **no `ctx` reference**. Every one of the file's 11 `ctx` occurrences sits at line 1376 or later, inside `pub(crate) async fn edit(input, ctx)` — the tool entry point the probe already scans. So the five actions add no `ToolContext` read that the baseline missed.

### 3. Merge and rename characterization — the rename axis closes, the merge axis does not

Same frozen window, `23adef79`, 500 SHAs.

- **Merges in window: 5.** Confirms the baseline figure exactly. The probe still does not expand merge-parent diffs, so that bound is unchanged.
- **Renames of a tracked `.rs` path in window: 0.** With a **control** — the same detector finds **80** renamed paths of other extensions in the same window, and fires `R053` on a known `.md` rename — so the zero is a measurement, not a dead pattern.

**Consequence: renames cannot have distorted the per-population co-change counts**, because no `.rs` file was renamed in the window and the probe's unit is tracked-`.rs` paths. The historical section can be read without a rename caveat. The **merge** caveat stands unchanged.

**A counting trap worth recording, because it fires silently.** `git rev-list --max-count=500 --merges <sha>` returns **24** — `--max-count` limits the number of *merges emitted*, not merges *within the first 500 commits*. The in-window figure needs an intersection against the window list, which gives 5. The wrong form returns a plausible number ~5× too large and no error.

### 4. Widening the frozen population — deliberately NOT done

The tracker gates this as *"only as a separately versioned measurement"*, and that gate is correct: every total above is scoped to the five hand-drawn populations, so a widened population is a new baseline rather than a better one. Not attempted.

### A new probe defect this run surfaced — `unresolved_same_file_helpers` is 71% Rust keywords

The probe reports **28** unresolved same-file helpers across 15 tools. **20 of the 28 are not helpers at all** — they are Rust syntax caught by an identifier regex:

| reported "helper" | what it actually is | tools affected |
|---|---|---:|
| `let` | `let (a, b) = …` destructuring | 15 |
| `return` | `return (…)` | 1 |
| `cfg`, `derive` | attribute macros | 2 |
| `drop` | `std::mem::drop` | 1 |
| `any` | `cfg(any(…))` / `Iterator::any` | 1 |

Cause, at `scripts/architecture-boundary-probe.py:809`: `elif "ctx" in body and helper not in {"if", "match", "while", "for"}` — an **enumerated four-keyword denylist over an open namespace**, the `IC-6` shape. Verified: `let` has zero `fn` definitions anywhere in `src/`, and `let (` destructuring is present in the scanned files.

Of the 8 remaining, 3 are **local closures or `&dyn Fn` parameters** — `plan_path` (`edit_code.rs:410`), `collect_docstrings` (`list_overview.rs:216`), `name_ok` (`query.rs:56`) — whose bodies are inline in the enclosing function the probe already walks, so their `ctx` reads are captured and flagging them is double-counting.

**The genuine population is 5 `(tool, helper)` pairs over 3 names**, each unresolved for the right reason — a name collision with exactly two definitions, so the probe correctly declines to guess:

| helper | definitions | tools |
|---|---|---|
| `uri_to_path` | `src/fs/mod.rs:366`, `src/lsp/client.rs:41` | edit_code, references, symbol_at |
| `leading_ws` | `src/tools/markdown/edit_markdown.rs:1143`, `src/util/text.rs:36` | edit_file |
| `count_lines` | `src/tools/command_summary.rs:451`, `src/util/text.rs:14` | run_command |

So the audit population slice 3 was waiting on is **5, not 28** — the instrument inflated it 5.6×.

### Slice 3's prerequisite — DISCHARGED, with a negative result

Slice 3 is blocked on *"audit the unresolved helper paths before selecting the initial migration."* Audited, and the answer is that **the unresolved routes hide no context dependency at all**:

- All six definitions of the three genuinely-unresolved helpers take a `&str` or a URI and return a value. **Zero `ctx` references** in any of them. Control: the same method finds `ctx` in `grep.rs`'s `call`, so the zeros are measurements.
- The five unresolved `edit_file` actions dispatch in a span with **zero `ctx` references** (§ 2).
- `unresolved_live_tools` is **0** — every advertised tool resolved to source.

**Therefore the helper-expanded `ToolContext` footprints in the 2026-09-13 baseline are complete with respect to every unresolved route.** No tool reads a context field the baseline failed to attribute, and the capability buckets do not move.

**What this licenses, precisely:** slice 3 may now be *designed* against the recorded footprints rather than against a population with unknown holes. **It does not authorize slice 3** and does not settle its migration target — the footprints are still lexical, still scoped to the five declared populations, and a complete-with-respect-to-unresolved-routes claim is not a call-graph.

## Bounded baseline and verdict — 2026-09-13

**Status:** review evidence collected; recommendations below are proposals, not approved runtime changes.
**Valid:** dated 2026-09-13
**Rests on:** the frozen-source reports and live workflow record below; production-function reproductions and regression tests; architecture-boundary-session-log:F-1 and architecture-boundary-session-log:W-1.

### Evidence identity and acceptance bounds

Use `.codescout/measurements/architecture-boundary/2026-09-13/baseline-validated.json` for the numbers in this section: source `23adef79023ebc43ea30d8d8e50a2175bacab5aa`, branch experiments, measured **09:21:05–09:23:28 +03:00**. SHA-256: `aa9984c6f5b1dad485df3c6a326a89356a29f63b5f2c764d3062f5b8008b88a0`.

This is a **bounded frozen snapshot, not a clean current-tree certification**. The command exited 2 because shared HEAD moved to `e0b8d2359f9aec32d5f1cdbc18a49530f8725c1c` after collection. The report was written before that guard fired. Inspection of `frozen_source_tree` and `main` confirms static/context source and Cargo metadata were materialized from git archive at the starting HEAD, and history was explicitly queried at that SHA. The guard's failure is retained; it is not silently relabeled a passing run.

A second corrected run, `baseline-final.json`, also exited 2 when HEAD moved: source `02e61230b6708d8879185f6469ec02202bfdaacb`, ending HEAD `96574bfacee39c6e45168df3fd2eefb6d6847db6`, 09:24:51–09:27:03 +03:00. It independently recorded the same static edge total; its different history window is **not mixed into** this section. SHA-256: `9b6edf653fc70510dc109ab5007a1151cf73f2a7b0474149de61ba7da1d63797`.

Reject the static totals in `baseline-initial.json` and `baseline-actions.json`: external imports were falsely made local and identifiers containing “as” were mangled. Both production defects were reproduced, given failing regressions, corrected, and rerun. The change from 379 to 315 edge rows is an **instrument correction**, not an architectural improvement. See [invented imports](../issues/archive/2026-09-13-architecture-probe-invents-internal-imports.md) and [mangled identifiers](../issues/archive/2026-09-13-architecture-probe-mangles-as-identifiers.md).

The registry in the report came from a newly started installed release executable at `target/release/codescout`; its recorded size/mtime are not proof of source identity. The separate workflow checks used the attached MCP, whose status reported `git_sha=408709ea`, dirty build, deleted executable, PID 2178312. Neither runtime is silently equated to the frozen source.

### Filed defects

Seven bugs were filed from this work stream. The probe implementation and all
seven files are committed in `d3a2c24f`. Six are architecture-probe defects; the
seventh is a codescout defect this work surfaced, listed here because nothing
else found it — not because it is an instrument problem.

Architecture-probe defects:

- [Longer cycles omitted](../issues/archive/2026-09-13-architecture-probe-omits-longer-cycles.md) — `089e063d1419449f`.
- [Multiline context omitted](../issues/archive/2026-09-13-architecture-probe-misses-multiline-context.md) — `360ba099bdb3c1e5`.
- [Registration control accepts non-tools](../issues/archive/2026-09-13-architecture-probe-registration-control-accepts-nontools.md) — `a323e32a49c229a0`.
- [Test-field masking consumes production structure](../issues/archive/2026-09-13-architecture-probe-test-field-masking.md) — `be92a232bf40c27f`.
- [Invented internal imports](../issues/archive/2026-09-13-architecture-probe-invents-internal-imports.md) — `576276c90babb155`.
- [Mangled alias-like identifiers](../issues/archive/2026-09-13-architecture-probe-mangles-as-identifiers.md) — `5b4a43dea0277aa5`.

Not an instrument defect:

- [Background command loses terminal status](../issues/archive/2026-09-13-background-command-loses-terminal-status.md) — `b9935bb5470a799c`. A `run_command` defect, surfaced by the live workflow checks below. **Fixed and archived 2026-09-15** in `f098069a`.

The last two probe defects are the pair that invalidated the earlier static
totals, and both carry `cluster/addressing-without-an-escape-hatch` (`IC-6`) —
which is the reason the 379 → 315 change is an instrument correction and not an
architectural improvement.

**All six archived 2026-09-16**, fix-evidence requirements met: `d3a2c24f`,
patch-id `f7ee24322b07702e7e87e5f4e17d78422089f2d1`, recorded on each file.
That commit introduced the corrected probe and its control suite together, which
is why fix and tests share one SHA. The regression suite was **re-run at current
HEAD rather than cited** — 14/14 pass, `self-test: ok` — because `40fb2843`
touched the probe script after `d3a2c24f`; inspected, and it only de-hardcodes
the `--runtime-binary` default. The ids above are the **post-move** ones: `id =
sha256(abs_path)`, so archiving re-keyed every row and the pre-move ids no longer
resolve.

Rejected artifact, retained deliberately: `baseline-initial.json` (1,356,751
bytes, SHA-256
`b578889215d60ec471d4ca08ebc8de51b786976457307abbd7ec1a1739948df3`) **predates
the action-row additions** — its context object has no `action_rows`, so it is
not merely superseded on edge totals; it cannot answer an action question at
all. Every measurement artifact named in this section is tracked under
`.codescout/measurements/architecture-boundary/2026-09-13/`.
### Static dependency observations

**Unit:** unique (classified production source file, classified target module) lexical edge. The raw list, its independently deduplicated file/module keys, and the matrix sum each recount to **315**, of which **144** cross populations.

| Source → target | Intelligence | Execution | Knowledge | Retrieval | Orchestration |
|---|---:|---:|---:|---:|---:|
| Intelligence | 36 | 1 | 0 | 1 | 25 |
| Execution | 6 | 10 | 0 | 0 | 25 |
| Knowledge | 7 | 1 | 80 | 10 | 18 |
| Retrieval | 3 | 0 | 1 | 23 | 14 |
| Orchestration | 9 | 12 | 7 | 4 | 22 |

These are not whole-project edges or compiler-resolved calls. The declared source populations contain **202 files**, leave **110** non-test Rust paths unclassified, and record **636 unresolved internal-target reference sites**. Bare roots are now left unresolved by the normalizer rather than promoted to fabricated local paths; they are not included in that explicit-internal-target count.

Crucially, unclassified paths include `src/tools/output_buffer.rs`, `src/tools/output.rs`, `src/tools/progress.rs`, `src/usage/mod.rs`, `src/fs/mod.rs`, `src/symbol/edit.rs` and `src/util/librarian_sync.rs`. This is a valid first population contract but **not a complete harness architecture map**. Do not change the population and compare totals as though the denominator stayed fixed.

The population graph contains **32 simple directed cycles**, spanning all candidate populations collectively. That is a graph description, not 32 independent design defects. A concrete reference is not automatically an interface bypass: the frozen adapter accepts `Arc<dyn crate::lsp::LspProvider>`, and returns `Arc<dyn crate::tools::Tool>`; those are existing interfaces worth preserving. The adapter's source lines 28/171/175 were checked at the frozen SHA.

The import control observes **16 source construction sites**, not the live tool denominator. Other inspected edges include `src/librarian/artifact_store.rs:20` importing `crate::retrieval::qdrant::QdrantWrap`. Several upward “orchestration” references are shared `Tool` / `ToolContext` / `RecoverableError` contracts, not domain workflows. Moving those contracts can improve dependency direction without adding a remote service.

### Runtime surface and action distinctions

The fresh registry advertised **21 distinct tools**. The textual direct-read scan sees `agent` in **15** registered tools' entry bodies; helper expansion reaches **20**. These are distinct measures, and the expanded one is a heuristic footprint, not an observed execution trace.

Helper-expanded capability buckets contain: 0 → none; 1 → get_guide; 2 → approve_write, library, onboarding, tree; 3 → call_graph, create_file, edit_code, grep, index, memory, references, semantic_search, symbol_at, symbols; 4+ → doc, edit_file, librarian, read_file, run_command, workspace.

The action inventory has **54 advertised enum actions**, plus **13 no-enum tool rows**, making **67 rows**. Of the enum actions, **49** have a textual branch and **5** remain unresolved: all advertised edit_file actions. A textual branch alone does not imply its transitive capabilities are resolved: edit_code's branches have no direct context reads because they delegate, not because editing is context-free.

Useful distinctions in the separate librarian context:

| doc action | Direct handler fields observed | Shared dispatch-prefix fields |
|---|---|---|
| get | catalog, current_project | catalog |
| delete | artifact_store, catalog, current_project | catalog |
| find | artifact_store, catalog, current_project, embedding, workspace | catalog |

This demonstrates why “doc depends on everything” is too coarse for designing an internal boundary. It does **not** prove every invocation of find uses embedding or every helper has been followed. Preserve unknown routes explicitly.

### Historical co-change observations

The fixed window has **500 distinct SHAs**. Parent-row inspection found **5 merge commits**; the probe's diff-tree command does not expand merge-parent diffs. This baseline therefore describes the emitted tracked-Rust path changes in that window, including inline test changes, not every semantic change introduced by every merge.

Per-population commit membership, independently recounted from the raw rows: intelligence **23**, execution **22**, knowledge **58**, retrieval **4**, orchestration **80**. **9** rows touch three or more populations.

Percentages below are rounded to one decimal place. A and B follow the first two columns.

| A | B | Intersection | Union | Jaccard | P(B given A) | P(A given B) |
|---|---|---:|---:|---:|---:|---:|
| Intelligence | Execution | 5 | 40 | 12.5% | 21.7% | 22.7% |
| Intelligence | Knowledge | 4 | 77 | 5.2% | 17.4% | 6.9% |
| Intelligence | Retrieval | 3 | 24 | 12.5% | 13.0% | 75.0% |
| Intelligence | Orchestration | 9 | 94 | 9.6% | 39.1% | 11.3% |
| Execution | Knowledge | 6 | 74 | 8.1% | 27.3% | 10.3% |
| Execution | Retrieval | 2 | 24 | 8.3% | 9.1% | 50.0% |
| Execution | Orchestration | 11 | 91 | 12.1% | 50.0% | 13.8% |
| Knowledge | Retrieval | 3 | 59 | 5.1% | 5.2% | 75.0% |
| Knowledge | Orchestration | 19 | 119 | 16.0% | 32.8% | 23.8% |
| Retrieval | Orchestration | 3 | 81 | 3.7% | 75.0% | 3.8% |

All pair intersections and unions were recomputed from raw memberships and matched. These modest overlaps do not show that the proposed domains must always ship together, but do not establish deployment independence either. Retrieval's denominator is only four commits; avoid a strong independence claim from it.

**Manual control now actually inspected:** `f909a160c3693e12505955f1f9f5323c134695b2`, the edit_file content/body parameter change. Its librarian change passes the level-specific key into a shared markdown helper; its execution change modifies edit_file's schema/alias; its server changes are tests and a surface-budget ratchet. Thus the three-population membership is correct at the file unit, while reading it as a three-domain production change would be wrong. The probe's field name `positive_control_inspected_commit` alone never proved this inspection.

### Feature and dependency observations

**Unit:** unique exact non-empty printed cargo-tree package line, including its printed decorations; not distinct crates, build time, RAM or attributable binary bytes.

| Lane | Features | Printed package lines |
|---|---|---:|
| Lean | no default features | 226 |
| Default | defaults | 339 |
| Server-stack only | no defaults + server-stack | 324 |
| Deployed | defaults + server-stack,local-embed | 512 |

All recorded dependency controls passed: serde_yml, pulldown-cmark and jsonschema absent from lean/present in default; qdrant-client absent from default/present in server-stack. The deployed lane retains defaults and matches the inspected cargo rb feature selection. No binary-size reduction is predicted from these line counts.

### Live workflow characterization

Raw calls, returned content and timings: `.codescout/measurements/architecture-boundary/2026-09-13/workflow-contracts.json`, SHA-256 `07d77644966f4f07f5ddf57075193f1d4a7397e57056bc4235d696a5596a7d68`. **14 recorded calls on one attached MCP instance**, not a model-evaluation sample.

- In a disposable Rust fixture, symbols returned answer() = 41 and cargo test failed with actual 41 / expected 42. Missing-target edit returned a recovery hint; valid replacement returned success; cargo test then passed its one test. No project source was edited for this demonstration.
- The missing-target call took **134,088 ms**; the following successful edit took **156 ms**. This is an observed outlier, **not** a measured steady-state latency or proven cold-LSP cause. Repetition with separated startup/miss costs is still owed before diagnosing it.
- Background `exit 7` returned “Process running” after **5,183 ms**, without a terminal code. Reading its empty log returned the **reader's** exit code 0. An explicit-marker control exposed FIXTURE_EXIT=7 after **5,166 ms**, but the response still said running. Source inspection of `spawn_background_command` confirms Child is dropped and the message is unconditional. Filed as [background terminal status](../issues/archive/2026-09-13-background-command-loses-terminal-status.md).
- A 300-line command stayed inline; a 3,000-line command produced a buffer handle. A targeted follow-up returned exactly lines 1500, 1501, 1502. The initial buffered text-content blocks measured **246 UTF-8 bytes**, the follow-up **54 bytes**. This characterizes successful disclosure, not a token-cost saving against a counterfactual model run.
- Pinned Cargo.toml reads returned harness-contract-fixture for the temporary root and codescout for home. This verifies those pinned calls. No process-wide activation race was deliberately induced in the shared MCP.

Timings include awaited transport/tool overhead and exclude model thinking time. No token estimates, latency percentiles, before/after agent-efficiency claims, cancellation tests or ambiguous-target tests are asserted.

### Proposed decisions and sequence

**Decision:** retain one MCP server; strengthen internal execution/result/request boundaries before considering domain extraction.
**Context:** the measured job-status failure requires a semantic lifecycle contract, not another deployment. The static graph also mixes service logic with shared tool/error interfaces; current populations omit important harness code.
**Alternatives considered:** domain microservices add remote identity, partial failures and lifecycle coordination without demonstrated independent deployment needs; file-size-driven splitting changes organization without proving a workflow gain; more prose reminders retain the agent's bookkeeping burden.
**Consequences:** easier to preserve current tooling, pins and LSP/retrieval seams while improving composability; harder to enforce ownership until contracts and migration adapters are explicit.
**Change scenarios absorbed:** waiting on failed jobs, recovering after compaction, rendering the same semantic result for different clients, and operating on pinned projects while other requests run.
**Revisit-when:** independently deployable workloads, measured resource isolation requirements, or a calibrated broader graph show a crate/process split paying for its added workflow complexity.
**Confidence:** high against a process split now; medium on exact migration order.

Recommended implementation slices, **not yet authorized**:

1. **Job lifecycle as the first vertical slice of a typed result contract.** Retain job state and terminal outcome separately from output buffers; provide status/result/cancel operations or equivalent explicit continuations. Change scenario: start a failing build, compact, and recover its actual result without shell markers. Alternative: status prose/log conventions are smaller but cannot reliably distinguish silent completion. Cost: ownership, cancellation and retention rules must be designed and tested. Confidence high on the need, medium on API shape.
2. **Generalize semantic results before further renderer growth.** `src/usage/mod.rs::classify_content_result` currently parses the first rendered content block as JSON to infer error/overflow. Separate outcome, completeness, provenance, notices and follow-up arguments from rendering. Change scenario: text, JSON and buffered projections preserve identical semantics and telemetry. Alternative: splitting a large file alone leaves this coupling intact. Cost: compatibility adapter and incremental tool-family migration. Confidence high on boundary, medium on migration cost.
3. **Resolve request context once.** Build on existing workspace pins rather than pretending they are missing. A scoped request object should make project identity and available operations explicit and avoid repeated ambient resolution. Change scenario: tool work remains tied to the chosen project across async steps. Cost: threading the resolved context through helpers and keeping transport concerns out of domains. Confidence medium; audit the unresolved helper paths before selecting the initial migration.
4. **Prepared edit and recoverable catalog/file mutations.** Preserve existing write locks, path guards, syntax checks and notifications; consolidate their commit/recovery mechanics across actual sibling implementations. Change scenarios: semantic edits and text edits invalidate the same state; a catalog failure after a file change is recoverable. Cost: file/SQLite ordering, rollback/reconciliation semantics and concurrency tests. Confidence medium; no crash-injection or shared-edit parity experiment was performed here.

Against the original domain questions: intelligence/execution interfaces can reduce imports on named edit/navigation workflows, but no reduction has been implemented or measured; knowledge already has separate context and optional dependency gates, so start with recoverable operations rather than process extraction; retrieval has too little history here to justify an ownership decision. None of the proposals claims measured binary savings. Cross-domain editing/indexing/activation remains explicit orchestration, not something to force into one owner.

### Verification and remaining work

Probe: **14 regression tests pass**, self-test passes. Final checked worktree SHA-256: probe `c51c8a6eb63ca550fd12d63ef521b92e9c3b38117a525cf08697a2613701d50d`; tests `eb51fc6402ea7f94d29fc0579190756c352ff0b02c75c8220bdc735dec4b6e38`. Four isolated applied mutations were detected (zero survivors among those four); the two import fixes additionally have observed pre-fix failures. This does not establish exhaustive Rust lexical correctness.

The first full gate had formatter refusal, clippy/lean success and a default-lane failure in a peer-modified attribution test. A second full gate is recorded in `gate2-*.log`; terminal results are recorded in the current Status section when available. Do not credit the first run as green or these lanes as server-stack coverage.

No runtime refactor was implemented by the measurement work itself. The work stream is committed in `d3a2c24f`, and **the six architecture-probe defects it fixes were archived 2026-09-16** once the fix evidence was recorded on each file — gate green, a regression test, the fix SHA and its stable patch-id. See **Filed defects** above for the post-move ids.

Next useful work is **design approval for the job/result slice**, not more broad scans. Before implementation, settle job lifetime across reconnects/compaction, cancellation ownership, retention and backward-compatible rendering; add deterministic failed/success/cancelled job checks. Separate follow-ups: repeat the missing-symbol latency check; resolve edit_file/edit_code action footprints; widen the frozen population only as a separately versioned measurement; characterize merges/renames before stronger historical claims.
