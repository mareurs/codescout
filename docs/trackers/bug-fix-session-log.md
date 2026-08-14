---
kind: tracker
status: active
title: Session Log — Bug-Fix Work Stream
owners: []
tags:
  - session-log
  - bug-fix
topic: Multi-session bug-fix work stream — frictions and wins from closing open buffer/markdown bugs in docs/issues/
time_scope: open-ended
---

# Session Log — Bug-Fix Work Stream

> **Scope:** Multi-session work to close the OPEN bugs in `docs/issues/`.
> Started 2026-05-17 when the controller began scouting the 4 open buffer/markdown
> bugs filed 2026-05-09. F-N entries capture drift between bug-file Resume hints
> and current code reality; W-N entries capture practices that prevented wasted
> fix work.


> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-17 | low | plan-prose | fixed-verified | Bug-file Resume paths cite non-existent layout |
| F-2 | 2026-05-17 | med | self-friction | fixed-verified | 2 of 3 buffer bugs likely stale — code reads correct |
| F-3 | 2026-05-18 | med | plan-prose | fixed-verified | Plan test assertions cited non-existent `RecoverableError.hint` field |
| F-4 | 2026-05-18 | med | codescout-tool | fixed-via-bug-tracker | `edit_markdown action="replace"` with a heading clobbers the whole section body |
| F-5 | 2026-05-18 | high | release-pipeline | mitigated | HEAD detached from `experiments` without `git checkout` in this session |
| F-6 | 2026-05-20 | med | release-pipeline | fixed-verified | HEAD non-compiling + 11 dormant clippy-1.95 lints exposed by toolchain bump |
| F-7 | 2026-05-21 | high | codescout-tool | mitigated | `references` undercounts vs `call_graph` (~18%); root is live-RA incompleteness, not position |
| F-8 | 2026-05-23 | med | codescout-tool | fixed-verified | `format_read_file` dispatches on `type`; json_path output collided → rendered `"0 lines"` |
| F-9 | 2026-05-24 | med | codescout-tool | fixed-verified | `audit_doc_refs` `fail_on` arg silently ignores `med`/`low` despite docs — engine fixed 2026-07-05, CLI surface 2026-08-07 (zombie-open in between) |
| F-10 | 2026-05-24 | low | release-pipeline | mitigated | RELEASE-TODO advertises "CI pipeline" as unchecked; workflow exists, push trigger pointed nowhere |
| F-11 | 2026-05-24 | med | release-pipeline | fixed-verified | CI runner missing `mold` linker required by `.cargo/config.toml` |
| F-12 | 2026-05-24 | med | codescout-tool-usage | fixed-verified | Dismissed `references`'s "use call_graph for authoritative callers" warning → shipped half-fix, missed `build.rs` duplicate |
| F-13 | 2026-05-25 | med | release-pipeline | fixed-verified | CHANGELOG entry written under wrong version label — `Cargo.toml`-as-authoritative assumption (0.13.0 already published) |
| F-14 | 2026-05-25 | high | release-pipeline | fixed-verified | `cargo publish` failed on `include_str!("../docs/...")` path stripped by `Cargo.toml` `exclude` — pre-publish gates couldn't detect |
| F-15 | 2026-06-09 | med | plan-prose | fixed-verified | Bug-file `project=`→`project_id=` fix plan misses 3rd test assertion (`tests.rs:257`) + cites non-existent fixture (shipped `890da4d6`; index row synced to body 2026-06-11) |
| F-16 | 2026-06-11 | high | self-friction | fixed-verified | Inherited "`edit_code` crashes Kotlin LSP" claim was a misdiagnosis — real cause was a kotlin-lsp RocksDB-lock deadlock |
| F-17 | 2026-06-11 | med | codescout-tool | promoted-to-bug-tracker | `references`/`symbol_at`/`call_graph` ignore the `workspace=` pin for relative path resolution |
| F-18 | 2026-06-11 | med | codescout-tool | open | Kotlin-lsp cold-start failed under 3× concurrent (6-JVM) load; tree-sitter masked it on `symbols` |
| F-19 | 2026-06-12 | med | plan-prose | fixed-verified | Explore-subagent report claimed `OutputForm::Text` bypasses the output buffer; `call_content` buffers unconditionally before the form branch |
| F-20 | 2026-06-12 | med | self-friction | fixed-verified | Targeted `cargo test --lib guide::` missed `server::tests::tool_descriptions_stay_under_budget`; shipped a 329-char get_guide description (cap 300) in `c799e887` |
| F-22 | 2026-06-14 | med | plan-prose | fixed-verified | Bug-file's "offset/limit never parsed anywhere" claim missed `OutputGuard::from_input` (`output.rs:96`); telemetry (191 calls) then resolved the fix to dispatch-layer option (a) |
| F-21 | 2026-06-14 | low | self-friction | fixed-verified | Off-the-cuff root-cause unification of the open-bug clusters over-generalized; code scout split it (catalog 2+3 share a global-abspath-catalog root; bug 1 + the 06-11 cluster don't) |
| F-23 | 2026-06-16 | high | codescout-tool | fixed-verified | `edit_code` "AST parse failed" hint named two wrong causes; real cause was AST(annotation-line)/LSP(`fun`-line) start-line gap exceeding the ±1 matcher tolerance on Kotlin methods with 2+ annotations |
| F-24 | 2026-06-20 | med | plan-prose | fixed-verified | Kotlin `-Xmx2g` fix documented "implemented on experiments" but was an uncommitted working-tree change at recon; concurrent session committed it as `3adb66e7` (incl. test) on `experiments` 2026-06-21 — not yet on master. llm-proxy `2906647`-on-master assertion checked out clean by contrast |
| F-25 | 2026-06-21 | med | plan-prose | open | Kotlin heap bug's root cause ("no `-Xmx` anywhere") missed kotlin-lsp's `intellij-server.vmoptions -Xmx2048m` — BUT that cap isn't reliably applied to codescout-spawned instances (06-20 GC-sawtooth reached 31 GiB heap despite it), so explicit `JAVA_TOOL_OPTIONS -Xmx2g` (`3adb66e7`) IS load-bearing, not redundant (self-corrected mid-session) |
| F-26 | 2026-06-21 | med | self-friction | fixed-verified | `replace_symbol_surfaces_stale_error_after_max_retries` failed on `experiments` — ROOT-CAUSED (2026-06-23): hermetic MockLsp test, NOT environmental; `758801d5` (F-23 fix) removed the AST-collection line-gate so `validate_symbol_range` pre-empted the staleness path. Master verified GREEN (still line-gated); orthogonal to the Kotlin cherry-pick. **Fixed `d079c054`** (reorder validators, option a); full suite green |
| F-27 | 2026-07-07 | med | self-friction | mitigated | Prior-session summary claimed two bug files were logged; neither exists on disk or in git history |
| F-28 | 2026-07-07 | low | plan-prose | wontfix-false-alarm | Inherited "stray lsp: field" claim in mv.rs was stale — ToolContext.lsp is now a legitimate field |
| F-29 | 2026-07-07 | med | self-friction | fixed-verified | Pre-compaction estimate of `ArtifactRow` fixture duplication ("3-4 sites") undercounted reality by 5x — actual: 21 constructors, 20 files |
| F-30 | 2026-07-07 | high | release-pipeline | fixed-verified | Upstream commit `62457959` bundled a stray `try_build_runtime(lsp.clone())` arg + `mv.rs` test had a phantom `lsp:` field — both broke `cargo rb` |
| F-31 | 2026-07-07 | med | self-friction | fixed-verified | Memory-tool project-scoping bug blocked reading the documented Windows binary-lock ("MCP Binary Symlink") gotcha mid-task, forcing rediscovery from scratch |
| F-32 | 2026-07-08 | med | self-friction | fixed-verified | `edit_code` rewrite of a test fn dropped a trailing assertion line that sat just outside an earlier batch-read's line-range window |
| F-34 | 2026-07-28 | med | plan-prose | fixed-verified | My own bug file's "cheapest" fix option would have broken `filter_sections_nested_h4_included_in_body`; its "cannot break language-patterns.md" claim was exactly backwards, and it ignored H1 entirely |
| F-38 | 2026-07-28 | med-high | self-friction | fixed-verified | A verify-open pass read `symbols`' compact summary as the whole result and filed "directory overview silently drops include_body" — 43/43 symbols had bodies in the 56 KB buffer named in the same response; the fix it recommended was premised on their absence |
| F-37 | 2026-07-28 | med | self-friction | fixed-verified | Filed a bug file whose `## Root cause` asserted a mechanism the code does not support — read both ends of a call chain (`target_base` write site, `editing_start_line`'s field) and inferred the middle, missing the `validate_symbol_position` guard that sits between them |
| F-36 | 2026-07-28 | med | self-friction | fixed-verified | The literal-scanner fix I had just shipped missed the commonest Rust multi-line-string form (plain `"` + raw newline); all six of its tests reused the bug report's `"\` shape, so the suite sampled one point of the class six times |
| F-35 | 2026-07-28 | low | self-friction | fixed-verified | Two doc comments I had just written made claims the code did not support (an unreachable fallback described as reachable; "reindents nothing" where the opener still shifts) — no gate can catch a doc comment that overstates a guarantee |
| F-33 | 2026-07-28 | low | self-friction | fixed-verified | Relocated the mux lock dir (`cfg(test)` redirect) before verifying nothing discovers those files by directory scan; assumption held, but left `peer_socket_differs_from_mux_and_shares_dir`'s name asserting an invariant now false in test builds |
| F-39 | 2026-08-14 | high | plan-prose | fixed-verified | Three bug files' `## Root cause` falsified on contact; `b86d81f1`'s premise would have left a measured IL3 **bypass** in place, `db5fe933`'s would have fixed 4 of 9 rows. Two carried their own refutation under Symptom |
| F-40 | 2026-08-14 | low | self-friction | mitigated | Mis-tranched 3 of 10 bugs as "no design decision needed" — triaged from titles when the blocked decision was named in each `## Resume` |

## Wins Index


| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-17 | med | Scout helper-fn bodies before fixing reported bugs | Would have written instrumentation / "fix" for `extract_lines` and `extract_json_path` despite both being correct + having passing tests | promoted-to-permanent-docs |
| W-2 | 2026-05-18 | med | Pre-dispatch recon scouts type accessors named in plan assertions | Task 2's first subagent would have failed `cargo check` on `err.hint.as_deref()` (no such field); 1+ wasted round-trip per test, controller drift mid-dispatch | validated |
| W-3 | 2026-05-18 | high | `git merge --ff-only <sha>` for detached-HEAD recovery under concurrent work | Naive `git branch -f` would silently traverse a parallel session's commit (the F-13 failure mode); `--ff-only` errors loudly on stale tip instead | promotion-eligible |
| W-4 | 2026-05-18 | high | Pre-fix recon validates filed-bug claims against pinned regression tests | Would have implemented a "fix" that broke the BUG-037 regression test `editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block`; bug filing itself was inaccurate (claimed attrs not included; actually they ARE via BUG-031) | validated |
| W-5 | 2026-05-24 | med | Deserialize before asserting: test against semantic data, not serialized text | Round-2 Windows fix asserted on JSON-encoded `text` containing escaped backslashes; passed Linux but broke Windows. Saves ≥1 CI cycle (10-15 min) per cross-platform test fix | validated |
| W-6 | 2026-05-24 | high | Cross-platform representation choices apply at every read AND write seam | Round 5 normalized writes only; round 6 had to fix 6 separate read-side boundaries (LIKE patterns, scope filters, substring checks). Missed read-side normalization is silent until integration. One miss (delete_orphan_repos LIKE) was destructive — wiped every catalog row. | validated |
| W-7 | 2026-05-25 | med | Verify-open recon flips zombie-fixed entries from `open` to `fixed-verified` | Without scout, F-6 + F-7 + F-11 would have continued to be counted as actionable backlog; future "what's open?" queries would have wasted ~30 min each re-investigating already-shipped fixes, or shipped them as known-issues in release notes. 3 zombies caught in one pass — promote-when criterion fired. | promoted-to-permanent-docs |
| W-8 | 2026-05-25 | high | `prompt_surfaces` test gate catches cap / snapshot / tool-name lint violations that `clippy`/`fmt` miss | Without `source_md_under_cap`, commit `4cc49ccb` would have shipped a server_instructions surface 339 bytes over the 2KB MCP cap, silently truncating Workspace gate + Deeper guidance in every fresh session for every project. Cost: workspace-restore slips + lost get_guide discovery surface, undetectable client-side. | validated |
| W-9 | 2026-06-05 | high | Spot-check sibling callers of a just-fixed shared helper before closing the bug class | Insert-only fix would ship while `edit_code` replace + remove still silently corrupt the LAST method of a Python class. Live repro: replacing `C/last` left orphaned `assert x` (`replaced_lines: 5-9`, off by one). `references(clamp_range_to_parent)` found both extra callers. | validated |
| W-10 | 2026-06-09 | med | Full-tree `grep <token>` before editing beats bug-file's hand-cited line list | Plan cited only `tests.rs:286-287`; line 257 flips to red + fixture hunt wasted on a 0-match surface | validated |
| W-11 | 2026-06-11 | high | Verify-open reconciliation against code+git de-zombies a backlog at scale | i1-refactor self-reported 13-of-14 pending but all 14 shipped; lsp-tools 3/3 fixed; both archived; ~46 open items mostly DONE-SINCE | validated |
| W-12 | 2026-06-11 | high | Re-derive an inherited "tool X breaks Y" claim from usage.db `tool_calls.outcome`+`lsp_events` before acting | Would have opened a bug against `edit_code`/AST (neither broken) + normalized a `create_file` workaround while the real cause (kotlin-lsp RocksDB-lock deadlock) left the LSP dead; transcript adjacency ≠ causation | validated |
| W-13 | 2026-06-11 | high | Test shared-LSP behavior via isolated worktree hashes, not contention on the live hash | Only alternative was N clients on the live hash = the R-23 move that SIGKILLed the user's working mux last session | validated |
| W-14 | 2026-06-12 | med | Verify a subagent's control-flow *mechanism* claim against the actual fn body before building a fix | Report's "Option A: set `output_form=Text`" compiles but leaves the 14 KB guide buffered → behavioral test fails → wrong-impl + re-debug cycle; reading `call_content` showed `exceeds_inline_limit` gates the branch unconditionally | validated |
| W-15 | 2026-06-12 | med | Enumerate a prompt surface's full gate set (byte-for-byte slice, snapshot, cap, version-pin, content tests) before editing | Blind edit hits ONBOARDING_VERSION==28 pin + snapshot + "6 memories" surprises as sequential red tests; pre-scout bundled all gate updates into one commit | validated |
| W-16 | 2026-06-14 | med | Scout each bug's actual body before standing behind a title-derived severity/priority triage of a bug set | Would have carried two wrong facts into a fix-priority recommendation: labeled `7ca71bf7` (catalog staleness) "data-loss" and missed that `3fc22ad2` is already partially fixed (lock-probe shipped 2026-06-11); 2nd same-day recurrence of F-21 | validated |
| W-17 | 2026-06-16 | med | When a tool's error text names a cause but a higher-level read contradicts it, reproduce the failing internal call on the real file and print its inputs before fixing | Two visible-but-wrong leads (hint said "syntax errors"; archived bug said "backtick mismatch", already shipped) would each have produced a no-op or wrong fix; a throwaway dump of `extract_symbols_from_source`→`find_ast_end_line_in` on the real file pinned AST 214 vs LSP 216 in one run, saving ≥1 wrong fix cycle (~4 round-trips) and a tolerance-widening band-aid | validated |
| W-18 | 2026-07-01 | med | Pre-impl recon confirms a parser-grammar fix plan is safe + names the regression tests it must keep green | Naive quoted-key fix flips `$.a[abc]`→Key (breaks `tests.rs:731`); blunt tokenizer rewrite breaks `$.a[0][-1]` (`tests.rs:683`); ≥2 red tests found only at cargo test; also pre-empted an over-engineered `Segment::QuotedKey` variant (Key already applies via `obj.get`) | validated |
| W-19 | 2026-07-05 | med | Pre-fix recon on `context(anchor_id)` starvation bug found `max_tokens_caps_inclusion` encodes "always include first" as deliberate design for topic-search mode, not a bug | A fix removing the `!markdown.is_empty()` packing guard globally would have broken that test's `assert_eq!(ids.len(), 1, ...)`; correct fix scopes the anchor's size cap to anchor-mode only | validated |
| W-20 | 2026-07-05 | high | Didn't accept a "triage the findings" task at face value — empirically bisected a surprising self-cite failure down to root cause instead of writing it off as expected noise | Would have reported 366 dangling / 232 ambiguous findings as "mostly expected per-tracker-ID-reuse noise" without noticing ~34 dangling + ~19 ambiguous were a real `link_scan` parsing defect (`ENABLE_YAML_STYLE_METADATA_BLOCKS` pairing any two bare `---` lines as YAML metadata, swallowing every other session-log entry's heading); found + fixed + live-verified (dangling 366→332, ambiguous 232→213, +16 edges, 8 pruned) instead of shipping an inflated triage report | validated |
| W-21 | 2026-07-07 | high | Grep the raw tracker file for the true max F-N/W-N before allocating a new ID — don't trust `artifact(get)`'s headings/body for large trackers | `artifact(get, full=false)`'s preview.headings for this tracker stopped at F-7 and `get(full=true)`'s body silently truncated at 499/1739 lines, both with no truncation flag; trusting either would have allocated "F-8"/"W-5", colliding with the 20 entries (F-8..F-27, W-5..W-20) actually in the file | validated |
| W-22 | 2026-07-08 | high | Diff every converted file against its pre-edit body before running tests, not after — a green suite doesn't prove fixture fields transcribed correctly | `git diff` review caught 2 silent field-omission bugs (`refresh_stale.rs` missing `file_sha256`; `constitution_check.rs` missing both `abs_path` and `file_sha256`) in the `ArtifactRow` builder migration before any test ran or commit landed — neither field is asserted by the affected tests, so `cargo test` would have stayed green with wrong fixture data | validated |
| W-23 | 2026-07-07 | high | Running full `cargo test --lib` after a single targeted fix, not just the touched test | 6 tests were still red after the ads_colon-only fix (2858/6); 1 of them was a REAL shipped production bug (SwitchAway hint mixing forward-slash + backslash paths in one sentence, `src/tools/config/mod.rs`), not test rot — narrow scoping would have shipped it untouched | validated |
| W-30 | 2026-07-28 | high | Re-run a bug file's reproduction against the live tool before choosing between its fix options — and read the buffer, not the summary | Both offered options were premised on bodies being absent; implementing either would have documented in code that directory overviews cannot carry bodies, contradicting a green regression test. The re-run collapsed the decision to "no change needed" and exposed the real defect: json_path_hint checked only the search-mode shape, so 50 KB of bodies advertised `$.files` | validated |
| W-29 | 2026-07-28 | high | Re-scout your own bug file's root cause before implementing its fix; when it cites two functions, read the layer between them | Would have shipped real work aimed at an unproven cause, closed the file with a misdirecting root-cause section, AND left the genuinely broken thing broken — the re-scout is what found `editing_start_line`'s discard-the-walk-back path sampling the column from a ` * ` comment continuation, one column deep, unrelated to LSP staleness | validated |
| W-28 | 2026-07-28 | med-high | Write the end-to-end verification fixture the way a user naturally would, not the way the bug report did — then trace the fix over it before running | Seven green tests, an archived bug file and a commit message claiming the literal class would all have shipped over a still-corrupting majority form. The next person to hit it would hit it as a new bug against a closed one, whose own evidence argued the mechanism was already understood | validated |
| W-27 | 2026-07-28 | med | Enumerate a shared helper's callers to decide WHERE a fix belongs, not just its blast radius | The bug file's own preferred fix sat in `reindent_to`; enumeration found `reindent_block`'s second production caller (`edit_file/mod.rs:747`) with the same defect, reachable without passing through `reindent_to`. That path guards writes with `has_syntax_errors`, which returns clean on a shifted literal — the bug would have stayed open behind a green gate, a closed bug file, and a passing syntax check | validated |
| W-26 | 2026-07-28 | med | A failing NEW test with an innocent implementation: diagnose the fixture before touching the code | Suspecting `heading_at` would have meant loosening the heading predicate to trim leading whitespace — making the new test pass while breaking `filter_sections_indented_heading_not_a_boundary`, then "fixing" that older test and landing a real regression. Reading the file back exonerated the implementation in one step; the defect was in `edit_code` | validated |
| W-25 | 2026-07-28 | med | Scout for discovery-by-directory-scan before relocating a file whose path is computed in one place | Defensive branch: thread a dir param through `LspManager::get_or_start` -> `get_or_start_via_mux` -> `claim_mux_lock`, a test concern in a production signature plus 17 individual test opt-ins, versus the 3-line `cfg(test)` seam that fixed all 17 at once. Optimistic branch: ship and later find mux sockets silently undiscoverable. Also surfaced the `#[ignore]`d test at manager.rs:2350 fixing for free and the peer subsystem's identical latent exposure | validated |
| W-24 | 2026-07-07 | med | Trace actual code (WAL pragma, absence of `wal_checkpoint`/`Drop`) before asserting a cross-project causal hypothesis, and label confidence explicitly | Without tracing `Catalog::open`, the answer to "could our force-kills cause the Mercury BOM catalog bug" would have been unciteable speculation; code + this session's own 3-concurrent-process observation produced a specific, appropriately-hedged (medium confidence) hypothesis addition instead | validated |
| W-31 | 2026-08-14 | med | `references` before instrumenting, when the question is "which path reaches X at runtime" | The bug prescribed a temporary `tracing::warn!` + full-suite run + revert; `references` returned ONE production caller in one call, and following it explained why the leak is specific to the default cargo-test lane — which the instrumentation would not have shown | validated |
| W-32 | 2026-08-14 | high | Run the bug's reproduction BEFORE reading its fix plan — the plan is a hypothesis about the reproduction | Three fixes changed on contact: a 1-field bug was 5 fields (a per-field fix would have repeated the July defect), a "parses fine" claim split into loud vs silent by the neighbouring key, and one fix direction INVERTED once fastembed's real 512-token ceiling was measured | validated |
| W-33 | 2026-08-14 | med | Mutate each new guard and check WHICH assertion fires | 3 mutations, 3 catches; one showed the control test correctly still passing (under- vs over-blocking arms), another showed two assertions cover distinct properties rather than restating one. Also surfaced that no warning catches a struct field that never existed | validated |

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Bug-file Resume paths cite non-existent layout

**Observed:** 2026-05-17, scouting `docs/issues/2026-05-09-*.md` to start fix work.

**When:** First `symbols(path)` calls after reading the 4 OPEN bug files.

**Expected:** Bug-file Resume sections cited `src/tools/buffer/handlers.rs` and `src/tools/search/grep.rs` as the starting points.

**Got:** `path not found: src/tools/buffer/` and `path not found: src/tools/search/grep.rs`. Real layout: `src/tools/output_buffer.rs`, `src/tools/read_file.rs` (with `read_from_buffer` helper inside), `src/tools/grep.rs` (top-level — no `search/` subdir).

**Probable cause:** Module restructure between 2026-05-09 (when bugs filed) and 2026-05-17 (current session). Bug files are append-only and never refresh path hints when the layout shifts.

**Workaround:** Ran `tree(src/tools)` to discover the real layout, then proceeded from `read_from_buffer`, `Grep::call`, `extract_lines`, `extract_json_path`.

**Severity:** low

**Status:** fixed-verified

**Fix idea / Pointer:** Resume sections for the 3 closed bug files (`grep-buffer-false-negatives`, `read-file-buffer-midpoint-empty`, `read-file-json-path-array-elements`) were rewritten in the same session to cite real symbols (`Grep::call`, `extract_lines` at `src/util/text.rs:23-33`, `extract_json_path` at `src/tools/file_summary/file_summary.rs:419`). Bug #1 (`edit-markdown-insert-after-h1`) still cites `src/tools/markdown/edit_markdown.rs` which is a real path — not affected.

---

## F-2 — 2 of 3 buffer bugs likely stale; code reads correct

**Observed:** 2026-05-17, reading `extract_lines` + `extract_json_path` bodies before writing fixes.

**When:** Investigating root causes for `2026-05-09-read-file-buffer-midpoint-empty.md` and `2026-05-09-read-file-json-path-array-elements.md`.

**Expected:** Per bug-file hypotheses, expected to find chunk-walker not advancing (`#3`) and array-index json_path mishandled (`#4`).

**Got:**
- `extract_lines` (12 lines, `src/util/text.rs:21-33`) — trivially correct line-range filter with existing passing test `extract_lines_out_of_bounds_returns_empty`. No chunk-walker exists.
- `extract_json_path` (`src/tools/file_summary/file_summary.rs:419`) — handles array indexing + has passing test `extract_json_path_array_index` using `$.users[0]`. Comment in code explicitly cites `$.symbols[0].body` as a working case.

**Probable cause:** Either (a) intervening commit closed the bugs without crediting back the bug files, or (b) original reporter observed empty STRING value at the json path (correct return) and misread it as "broken." `Bug #2` (`grep-buffer-false-negatives`) IS real — `Grep::call` has no `@*` ref handling at all.

**Workaround:** Plan to write probe tests mirroring the exact bug-file reproductions. If they pass: close as `wontfix-false-alarm` + commit the test as a regression guard. If they fail: real defect surfaces.

**Severity:** med

**Status:** fixed-verified

**Fix idea / Pointer:** Probe tests confirmed prediction:
- Bug #2 `grep-buffer-false-negatives` FAILED probe → confirmed real → fixed via `grep_in_buffer` + `@`-prefix branch in `Grep::call`.
- Bug #3 `read-file-buffer-midpoint-empty` PASSED probe → closed `wontfix`, test kept as regression pin.
- Bug #4 `read-file-json-path-array-elements` PASSED probe → closed `wontfix`, test kept as regression pin.

---

## W-1 — Scout helper-fn bodies before "fixing" reported bugs

**Observed:** 2026-05-17, bug-fix work stream kickoff.

**Pattern:** Read the `extract_lines` and `extract_json_path` bodies before writing any failing test or any fix. Each was small (~12 / ~50 lines) and surfaced its own correctness immediately.

**Counterfactual:** Without reading the helper-fn bodies first, would have written either (a) instrumentation patches to "debug" non-existent chunk-walker bugs in `extract_lines`, or (b) a "fix" adding array indexing to `extract_json_path` that already exists and is tested. Both = wasted commits + churn on green code. Concrete evidence: bug-file hypothesis "Buffer is chunked internally and the chunk-walker may not advance past the first chunk" was confidently wrong — there is no chunk walker.

**Confirming data points:**
1. `extract_lines` body reveals no chunk walker exists — bug-file hypothesis #3 invalidated in one read.
2. `extract_json_path_array_index` test (passing, `$.users[0]`) invalidates bug-file hypothesis #4 in one read.
3. By contrast, scouting `Grep::call` confirmed bug #2 is real — same scouting move found one true defect among three claimed defects.
4. Probe tests for #3 and #4 passed immediately, proving the prediction; probe test for #2 failed with an explicit panic naming the failure mode (`relative path '@tool_*' requires an active project`), letting the fix shape itself.

**Impact:** med

**Promote-when:** This is a specific instance of the existing reconnaissance discipline. Already covered by `codescout-companion:reconnaissance` skill. No promotion needed — count this as a confirming data point for the skill, not a new rule.

**Status:** validated

---
## F-3 — Plan test assertions cited non-existent `RecoverableError.hint` field

**When:** Pre-dispatch reconnaissance for the jsonpath negative-slice
implementation plan (`docs/superpowers/plans/2026-05-18-jsonpath-negative-slice.md`).
About to dispatch Task 1 subagent.

**Expected (plan):** `RecoverableError` has accessible `.hint: Option<String>`
field; plan tests used `err.hint.as_deref().unwrap_or("")`.

**Got (scouted reality):** `RecoverableError` at `src/tools/core/types.rs:169`
exposes `pub message: String` and `pub guidance: Option<Guidance>` — there is
NO `.hint` field. There IS a method `.hint() -> Option<&str>` that returns the
text only for the `Guidance::Hint` variant. The `Display` impl's own comment
explicitly recommends `to_string().contains(...)` for test assertions because
it renders `"{message} — Hint: {text}"` and is the documented stable contract:

> "Display renders only `message`. The structured `hint` and `recovery_steps`
> are intentionally omitted here so existing `to_string().contains(...)` test
> assertions stay stable."

Wait — re-reading: Display renders `message` PLUS `" — {field_name}: {text}"`
when guidance is present. The contract is: `to_string().contains(hint_text)`
holds. So tests can use `err.to_string().contains("...")` regardless of which
guidance variant is attached.

**Probable cause:** Plan was written from the design spec; spec didn't pin
the assertion-side accessor shape; writing-plans phase didn't scout
`RecoverableError`. Standard "scout helper-fn bodies" rule (W-1 in this same
session log) applies to type shapes too.

**Workaround:** Edit the plan's Task 2 + Task 3 test code to use
`err.to_string().contains("...")` everywhere a hint-text or message-text
assertion is made. Drops the `.hint` field reference. Less brittle than
`err.message.contains(...)` because it also covers cases where the failing
substring lives in the guidance text rather than the message text.

**Severity:** med — would have caused first subagent's tests to fail to
compile; controller would then absorb the failed-task drift mid-dispatch.

**Status:** open → fixed-verified after plan edit lands this turn.
## W-2 — Pre-dispatch recon caught test-shape error before any subagent ran

**When:** About to dispatch Task 1 of the jsonpath negative-slice plan to a
fresh subagent (subagent-driven-development mode).

**Pattern:** Before the first subagent dispatch on a plan that names *types*
in test assertions (not just *fns*), invoke the reconnaissance skill and
scout each referenced type's actual field/method shape. Specifically:
`symbols(name=<TypeName>, include_body=true)` for any type whose accessors
the plan tests mention.

**Counterfactual:** Without this scout, Task 2's first subagent would have
written `err.hint.as_deref().unwrap_or("")` and failed `cargo check` on the
first parse test. The subagent would have flailed (probable retries with
`err.guidance`, or `err.hint().unwrap_or("")`, or `err.to_string()`) without
the Display-impl contract context. Best case: 1 extra round-trip per failing
test (~11 round-trips for the 11 parser tests in Task 2). Worst case: the
subagent gives up, controller re-scopes plan mid-dispatch, F-N entry written
*after* drift instead of *before*.

**Confirming data points:**
1. F-3 (this session) — `RecoverableError.hint` field cited by plan does not
   exist; scout caught it pre-dispatch.
2. Pending: any future plan that names types in assertions.

**Impact:** med — saves ≥1 failed subagent task and prevents controller
context absorption of cascading test failures. The saving compounds
across the 4 implementation tasks in the plan.

**Promote-when:** A second pre-dispatch recon catches a similarly hidden
type-shape mismatch (any plan, any type). At 2 datapoints, promote to
CLAUDE.md's "Before dispatching the first subagent of an implementation
plan, scout every type whose accessors the plan asserts on" rule.

**Status:** validated — single datapoint, drift caught + fixed in the same
turn before any subagent dispatch. Awaiting promotion criterion.
## F-4 — `edit_markdown action="replace"` with a heading argument clobbers the whole section body

**Observed:** 2026-05-18, while adding the "When the Substrate Catches Itself" section to `docs/observations.md`.

**When:** Tried to add a new H2 section after the existing `## The Plugin Closes the Loop` via `edit_markdown(action="replace", heading="## The Plugin Closes the Loop", content="<new section + trailing closer line>")` — expecting insert-after-like semantics.

**Expected:** `action="replace"` with `heading=X` would either replace only the heading line or operate on a localized region near the anchor.

**Got:** The full body of `## The Plugin Closes the Loop` (~30 lines of SessionStart / SubagentStart / PreToolUse hook narrative + the marketplace install hint) was overwritten wholesale with the new section's content. The original body was destroyed.

**Probable cause:** `edit_markdown action="replace"` with a `heading` argument has "set the body of this section to `content`" semantics. The argument is the section anchor, not an insertion anchor; the operation wipes from the heading's end through to the next sibling heading. Not aliased with `insert_after`.

**Workaround:** Caught by the Frog discipline's post-edit verify (`read_markdown` after every write). Reconstructed the original body from an earlier in-session `read_markdown` snapshot, ran `edit_markdown action="replace"` again with the original content to restore it, then `edit_markdown action="edit"` for cosmetic blank-line repairs. Three extra round-trips beyond the intended insert.

**Severity:** med — data loss within session, fully recovered, but easy to miss without verify-after-edit.

**Status:** fixed-via-bug-tracker (Option A shipped this session); see `docs/issues/archive/2026-05-18-edit-markdown-replace-clobber.md` (status: fixed, closed 2026-05-18).

**Fix idea / Pointer:** Option A shipped — destructive-scope warning added to `long_docs()` and per-variant action descriptions in the schema for `EditMarkdown` in `src/tools/markdown/edit_markdown.rs`. Top-level `description()` stays under the 300-char budget (caught by `server::tests::tool_descriptions_stay_under_budget` on first attempt). Option B (force flag + size threshold) deferred until Option A is observed to be insufficient — bug-tracker entry retains the Option B sketch and three regression tests it would need.

---

## F-5 — HEAD detached from `experiments` without `git checkout` originating in this session

**Observed:** 2026-05-18, after several `/reload-plugins` calls + one `/mcp` reconnect cycle while working on observations.md.

**When:** Session started on `experiments` per the git status snapshot at the top of the system prompt. After multi-step work (tracker rectification, hook verification, observations.md edits), ran `git commit` for the observations work — output showed `[detached HEAD a70816b5]`, NOT `[experiments a70816b5]`.

**Expected:** `git checkout`-style branch detachment requires an explicit checkout call. Host operations (`/reload-plugins`, `/mcp` reconnect) and codescout MCP tool calls should leave HEAD on its current branch.

**Got:** `git reflog -15` revealed three unattributed HEAD moves between my last commit (`81a6d136`, on `experiments`) and the detached commit (`a70816b5`):
```
HEAD@{1}: checkout: moving from experiments to d5bf7116
HEAD@{2}: checkout: moving from 59f6b53c… to experiments
HEAD@{3}: checkout: moving from experiments to 59f6b53c
```
HEAD was actively bounced between `experiments` and parallel-session commit SHAs (the `feat(file_summary)` work the other session shipped). My session issued zero `git checkout` calls during that window.

**Probable cause:** Unknown. Candidates:
1. `codescout-companion` PostToolUse hook on `mcp__.*__workspace` (`cs-activate-project.sh` or `worktree-activate.sh`) running `git checkout` as a side effect.
2. Parallel session's commits propagating into shared workspace state in a way the host translates to a HEAD move on my side.
3. `/reload-plugins` re-running SessionStart-style hooks that touch git refs.

**Workaround:** Detected via the `[detached HEAD ...]` string in `git commit` output. Recovered via W-3 (`git merge --ff-only`). No data lost.

**Severity:** high — silent data-loss vector. Commits on detached HEAD are reachable only via reflog and expire on `git gc`. The user only sees the failure if they read the `git commit` output carefully — easy to miss.

**Status:** mitigated (2026-05-25 verify-open audit). The F-5 fix idea's hypothesis — "audit companion hooks that touch git state" — was discharged this session via an exhaustive grep across all 24 hook files in `~/work/claude/claude-plugins/codescout-companion/hooks/`. Every `git` invocation is read-only: `git rev-parse` (show-toplevel, git-common-dir, git-dir, HEAD), `git rev-list --count`, `git worktree list --porcelain`, `git remote get-url`. The single destructive-verb reference is a regex match-pattern in `git-worktree-guard.sh:38` used to BLOCK `git commit|push|reset --hard|rebase|merge|checkout -b` — a guard, not an invocation. No companion hook contains code that moves HEAD. Multiple `/reload-plugins` and `/mcp` reconnect cycles in subsequent sessions (including this one's post-compact reconnect + a deliberate `/reload-plugins` after this audit) have not reproduced the symptom. Note: this disposition is *audit-driven*, not *fix-shipped* — the original investigation premise turned out wrong; no specific fix can be cited as closing F-5. Symptom may have come from a one-time interaction outside the companion plugin (other plugins, IDE side-effects, race during workspace activation). If it recurs, reopen with the explicit reflog evidence + the non-companion source narrowed in mind.

**Fix idea / Pointer:** Open a bug file with the reflog snippet preserved. Audit companion hooks that touch git state — likely candidates: `hooks/cs-activate-project.sh`, `hooks/worktree-activate.sh`, anything in the SessionStart hook chain. If a hook is moving HEAD as a side effect of workspace activation, scope it to only run when the active project itself changed, not on every reconnect.

---

## W-3 — `git merge --ff-only` as the atomic recovery primitive under concurrent-work HEAD detachment

**When:** 2026-05-18, recovering commit `a70816b5` (observations.md narrative) from detached HEAD after `/reload-plugins` + `/mcp` cycle silently moved HEAD to a parallel session's commit SHA (`d5bf7116`).

**Pattern:**
1. Read full state in a single command — `git reflog -15 && git branch -v && git rev-parse HEAD && git symbolic-ref HEAD` — so the observation set is internally consistent before any write.
2. Recover with `git checkout <target-branch> && git merge --ff-only <recovered-sha>` in a single command. The `--ff-only` flag is the atomic safety: it succeeds silently if `<recovered-sha>` is a strict descendant of the target branch's current tip (history is not fabricated, only the branch pointer moves), and it fails loudly if anything diverged between the read and the write.

**Counterfactual:** Naive recovery `git branch -f experiments a70816b5` would force-move the branch ref regardless of whether `experiments` had moved between observation and action. If a parallel session shipped a commit to `experiments` in the gap between my `git reflog -15` read and the `git branch -f` write, the force-move would silently traverse that commit — the F-13 failure mode incarnate. `--ff-only` removes the traversal hazard by making git refuse to invent history; if `experiments` has moved, the merge errors out and the operator reconciles manually rather than discovering the loss later via reflog.

**Confirming data points:**
1. F-13 (2026-05-17, prior session) — `git reset --soft HEAD~1` erased a parallel session's T-13 commit because HEAD moved between observation and `reset` action. Recovered via reflog-quoted SHA. Driving incident for the existing CLAUDE.md § Concurrent-Work Rules block.
2. This session (2026-05-18) — F-5 detached-HEAD recovery used `git merge --ff-only a70816b5` after a single combined `reflog -15` read; experiments tip was `d5bf7116`, recovered-sha's parent was also `d5bf7116`, ff-only succeeded silently. Working tree clean, no data lost.

**Impact:** high. Concurrent-work git is the single most common silent-data-loss vector in multi-session work; `--ff-only` is one of the few git primitives that fails-loudly on stale-tip.

**Promote-when:** Two concretes reached (F-13 + F-5). Promote to CLAUDE.md § Concurrent-Work Rules with an explicit primitive callout: *"For detached-HEAD recovery, use `git checkout <branch> && git merge --ff-only <recovered-sha>` in one command. Never `git branch -f <branch> <recovered-sha>` — it traverses parallel commits silently."*

**Status:** promotion-eligible (criterion fires this session).

---

## W-4 — Pre-fix recon caught wontfix bug (BUG-037 was already shipped)

**Observed:** 2026-05-18, about to implement
`docs/issues/archive/2026-05-18-edit-code-replace-misses-outer-attrs.md`.

**Pattern:** Before writing code to "fix" a behavior reported as buggy
by a prior subagent's escape-hatch fallback (e.g. `python3 re.sub`),
scout the symbol that owns the behavior — including its sibling tests.
If a regression test pins the current behavior with a `BUG-XXX`
reference in its name or doc comment, the current behavior is
deliberate, not a bug. Update the filed bug to `wontfix` and pivot to
the real surface (usually documentation).

**Counterfactual:** Without this scout, I would have written code to
extend `editing_start_line`'s walk-back to include attribute-only
blocks above an `impl`/`fn`. That code would have broken the
`editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block`
regression test pinned by BUG-037, which catches the more dangerous
failure mode (silently dropping `#[async_trait]`-style attributes).
The TDD red-bar would have fired at cargo test, but only AFTER I'd
written the wrong-direction fix — at least 1 wasted round-trip plus
reviewer cycles.

The scout also caught that the bug description I just filed (≤2
minutes prior) was inaccurate: it claimed "outer attributes are NOT
included in the replace range," but the actual default behavior IS
inclusive via BUG-031 walk-back. The narrower BUG-037 guard is what
the user's pain point ran into.

**Confirming data points:**
1. F-3 (this same session log) — `RecoverableError.hint` field cited
   by a plan was non-existent; scout caught it pre-dispatch.
2. W-2 (this same session log) — the W-N for F-3.
3. **W-4 (this entry):** scout caught a wontfix-bug-filing mistake AND
   prevented a regression-test-breaking fix attempt. Two failure
   modes prevented in one scout.

**Impact:** high — scout prevented (a) bad bug filing being treated as
ground truth, and (b) wrong-direction fix attempt that would have
broken pinned regression coverage. The cost of NOT scouting compounds:
filed bug → planned fix → coded fix → red bar → debugging → revert →
correct fix. Each step is 5-20 minutes; total cost could be 1-2 hours
of churn.

**Promote-when:** Already at 3 datapoints (F-3, W-2, W-3). At 3
distinct datapoints, the recon habit is no longer probationary.
Promote to CLAUDE.md as a permanent rule:

> Before writing code to fix a behavior reported as wrong, scout the
> symbol that owns it AND its sibling tests. If a regression test
> pins the current behavior with a `BUG-XXX` doc reference, treat
> that as a strong "the current behavior is intentional" signal.
> Validate the bug filing's claim against the test pin before
> committing to a fix direction.

**Status:** validated — promotable; awaiting CLAUDE.md edit.

## F-6 — HEAD non-compiling on `experiments`; clippy 1.95 toolchain bump exposed 11 dormant lints

**Observed:** 2026-05-20, pre-commit recon for a 3-way commit split of uncommitted changes (IL3 narrow, probe sentinels, gfx1101 arch bump). User said `cargo clippy` must pass per CLAUDE.md; clippy failed with 11 errors, raising the question of whether the uncommitted diff introduced them.

**When:** Phase-1 scout: `git stash push -u` + `cargo clippy --all-targets -- -D warnings` against bare HEAD.

**Expected (working assumption):** HEAD `experiments` compiles cleanly. Uncommitted diff might or might not introduce lints; stash-and-reclippy isolates the blame.

**Got (scouted reality):** Two independent problems entangled.

1. **HEAD does not compile.** With uncommitted diff stashed, `cargo clippy` failed with `E0063: missing field guide_hints_emitted in initializer of ToolContext` at `src/tools/run_command/tests.rs:372`. The `guide_hints_emitted` field was added to `ToolContext` in commit `68947c4a feat(prompts): first-call hint mechanism for get_guide topics`, but the run_command test fixtures were not updated in the same commit. The uncommitted `tests.rs` changes (`+guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default()))` × 12 sites) are silently *fixing* this broken state, not adding new functionality. The split-commit plan must keep these test fixture edits paired with the IL3 commit (or be its own commit) — they are load-bearing.

2. **Clippy 1.95 added new lints** (URLs cite `rust-clippy/rust-1.95.0/`). With uncommitted diff applied, compilation passes, and 11 pre-existing lints fire across 6 unrelated files:
   - `src/librarian/preview/memory.rs:16`, `src/logging.rs:66`, `src/tools/file_group.rs:38` — `unnecessary_sort_by` (3)
   - `src/retrieval/sync.rs:174`, `src/tools/symbol/call_graph/traversal.rs:86` — `useless_conversion` on `.into_iter()` (2)
   - `src/tools/markdown/edit_markdown.rs:51, 174, 269` — `unnecessary_cast usize as usize` (3)
   - `src/util/path_security.rs:444, 452, 459` — `collapsible_match` in tool-name validate arms (3) — NOT in the IL3 functions edited by the uncommitted diff, in the unrelated `validate_tool_for_path` match block.

**Probable cause:**
- Problem 1: `feat(prompts)` commit landed without `cargo test` against `run_command` test fixtures, OR was tested locally with stale build artifacts that didn't recompile the affected test file. Pre-commit `cargo test` rule (CLAUDE.md) was elided.
- Problem 2: Rust toolchain auto-updated to 1.95 between last clean clippy run and now. New lints are mechanical.

**Workaround:**
- Problem 1: keep the `tests.rs` field-init edits in the same commit as the IL3 narrow, or as a precursor compile-fix commit. Don't split them off as "test cleanup."
- Problem 2: run `cargo clippy --fix --allow-dirty --allow-staged` to auto-apply the 11 fixes, then commit as `chore(clippy): adopt clippy 1.95 suggestions`. Keep it separate from feature commits to keep the lint-bump signal legible in `git log`.

**Severity:** med — Problem 1 means HEAD on a public-facing branch is broken; any developer cloning `experiments` would fail `cargo test` without the uncommitted local edits. Problem 2 is mechanical cleanup but blocks the project's CLAUDE.md "clippy clean before commit" gate.

**Status:** fixed-verified (2026-05-25). The clippy-clean state shipped to master as part of the cumulative session-work batch in merge `d1742c46`; no single labeled "clippy 1.95 cleanup" commit landed, so the entry stayed `open` despite the underlying condition being long-resolved. Verified this turn: `cargo clippy --all-targets -- -D warnings` on current experiments HEAD exits 0 with no warnings. Lesson — distributed-fix entries need an explicit close pass; the absence of a labeled commit doesn't mean the bug is still open.

**Fix idea / Pointer:** This recon session — split-commit plan adjusted to (1) clippy 1.95 auto-fix cleanup, (2) IL3 narrow + tests.rs field init + bug doc, (3) probe sentinels, (4) docker-compose gfx1101 + backtick-eof bug doc.

---

## F-7 — `references` undercounts vs `call_graph`; root is live-incomplete-LSP vs persistent edge-cache, NOT position

**Observed:** 2026-05-21, debugging the references-undercount bug
(`docs/issues/archive/2026-05-21-references-undercounts-vs-call-graph.md`). Live
`references(symbol="format_read_file", path="src/tools/read_file.rs")` returned 3;
`call_graph(direction="callers")` returned 17; `grep` ground-truth = 17 call sites
in `src/tools/edit_file/tests.rs`.

**When:** Phase-1 root-cause scout of the two tools' resolution paths, before any fix.

**Expected (initial hypothesis):** `references` queries LSP at the *item* start
(column of `pub`) instead of the identifier, so rust-analyzer misfires and
underreports. (Plausible because a wrong-token reference query returns garbage.)

**Got (scouted reality):**
- `references` (`src/tools/symbol/references.rs:54-59`) resolves position via
  `find_unique_symbol_by_name_path` → `sym.start_line/start_col`, then calls
  `client.references()` (`textDocument/references`, `include_declaration: true`,
  no truncation — `src/lsp/client.rs:1032-1058`).
- `SymbolInfo.start_line/start_col` come from **`selection_range.start`** (the
  identifier), NOT `range.start` (`src/lsp/client.rs:159-161`; pinned by test
  `convert_document_symbols_uses_selection_range`). **Position is correct** →
  position-bug hypothesis REJECTED.
- references' build-dir + scope filters dropped 0 here (`excluded=0`), so the live
  `textDocument/references` itself returned only 3 — the undercount is upstream of
  all our code.
- `call_graph` (`src/tools/symbol/call_graph/mod.rs:222-234`) reads a **persistent
  SQLite `EdgeCache.lookup_callers(symbol)` by name**; on cache hit it returns the
  17 edges with **no live LSP call**. The cache was populated earlier by
  `resolve_one_hop` when the set was complete.

**Probable cause:** `references` is at the mercy of rust-analyzer's *live* index
state for the queried symbol's references; for the large `cfg(test)` file
`edit_file/tests.rs` (~4400 lines) RA returned an incomplete set (3 of 17) at query
time. `call_graph` masks this by serving a complete persisted cache. NOT YET
CONFIRMED — pending the warming + non-test-symbol experiments below.

**Workaround:** use `call_graph(direction="callers")` for "who calls X"; fall back
to `grep` for non-call references. Treat a low `references` count as suspect.

**Severity:** high — a navigation tool silently returning ~18% of real callers will
mislead any refactor that trusts it.

**Status:** mitigated (2026-05-21, ratified 2026-05-25 verify-open pass). Mitigation shipped: `references_completeness_hint` cross-check in `src/tools/symbol/references.rs:25-35` + integration at `references.rs:168-189`. On undercount detection (call-hierarchy finds more call sites than references returned), the response carries a `completeness_warning` field directing the caller to use `call_graph(direction="callers")`. The underlying RA bug is not fixed (lives in rust-analyzer's domain) but the codescout-side surface is honest about its own incompleteness. Root-cause hypothesis updated: not symbol-shape, but **transient RA staleness** — commit `495c8640 docs(issues): correct references-undercount root cause — transient RA staleness, not symbol shape`. Bug file archived to `docs/issues/archive/2026-05-21-references-undercounts-vs-call-graph.md` with `status: mitigated`, `closed: 2026-05-21`. Mitigation commit: `3d984e77 feat(tools): extend OutputForm::Text sweep; add references completeness guard`.

**Fix idea / Pointer:** if confirmed RA-incompleteness, references needs either a
warmth/completeness guard (re-query until stable, or `did_open`+settle) or should
consult the same EdgeCache call_graph uses. Investigate `resolve_one_hop`
(`src/tools/symbol/call_edges/resolver.rs`) — does it use callHierarchy or
references? If callHierarchy is more complete than textDocument/references on RA,
references could switch to it.
## F-8 — `format_read_file` dispatches on `type` field, collided with json_path output

**Observed:** 2026-05-23, debugging "0 lines" output from `read_file(json_path=...)` on both `@tool_*` buffers and real JSON files.

**When:** Two consecutive failures — first reproducing the user-reported friction with `artifact(get, full=true)` → `read_file(@tool_*, json_path="$.body")` returning `"0 lines\n\n  Buffer: @file_*\n  hint..."`. Then auditing for other sites surfaced the same shape in `read_json_path_nav` for plain JSON files.

**Expected:** A valid `json_path` extraction renders with line count + content (small) or line count + buffer ref (large).

**Got:** `"0 lines"` regardless of extracted content size — even when the underlying `@file_*` buffer contained 128 lines of body.

**Root cause:** `format_read_file` (src/tools/read_file.rs:678) dispatches on `val["type"].as_str()` FIRST. Both `read_from_buffer` (line 175) and `read_json_path_nav` (line 354) emitted `"type": type_name` where `type_name` came from `extract_json_path` (values like `"string"`, `"object"`, `"array"`, `"number"`). These all fell through to `format_read_file_summary`'s `_ => {}` fallback case; `line_count` was never written by these paths, defaulted to `0`. Result: `"0 lines\n"` rendered with no content branch ever consulted.

The bug was invisible until a tool returned `type: <a value not in {source, markdown, json, toml, yaml, config, generic}>`. All in-tree summarizers happen to emit one of the seven known types, so test coverage didn't surface the gap.

**Workaround applied:** Renamed `"type"` to `"value_type"` in both emission sites. `format_read_file` no longer dispatches to summary mode for these results; Content mode (small) and "Old no-content buffered mode" (oversized) handle them correctly. Oversized branch also gains `total_lines: line_count` so the buffered-mode formatter renders an accurate count.

**Severity:** med — degrades agent UX on every `read_file(json_path=...)` call extracting a scalar/markdown body. No data loss; the buffered content was still reachable via the `@file_*` ref the formatter printed alongside the misleading "0 lines". User reported it via this session's transcript.

**Status:** fixed-verified — commit `16c5cfd2 fix(read_file): rename json_path output 'type' to 'value_type'`. Verified post-`/mcp` reconnect with `artifact(get, full=true)` → `read_file(@tool_*, json_path="$.body")` → `128 lines\n\n  Buffer: ...`. Also verified inline path with `read_file("scripts/package.json", json_path="$.private")` → `1 line\n\ntrue`.

**Fix idea / Pointer:** Defensive improvement candidate (not done in this commit): make `format_read_file_summary` log/warn on unknown `type` variants rather than fallthrough rendering `"{line_count} lines\n"` with no body branch. Would have caught the collision at first sighting.

## F-9 — `audit_doc_refs` `fail_on` arg silently ignores `med` and `low` despite docs advertising them

**Observed:** 2026-05-24, pre-implementation reconnaissance for the
`codescout audit-doc-refs` CLI subcommand (H-5 + R-1 shipping).

**When:** About to write the CLI arg parser with `--fail-on` accepting
`high | med | low | never`, per `CLAUDE.md` § Standard Ship Sequence step 5
and the librarian schema docstring.

**Expected (docs):** `librarian(action="audit_doc_refs", fail_on="med")`
returns exit_code=1 when at least one finding has `severity: med`. Same for
`low`. Documented in two places:
- `src/librarian/tools/librarian.rs` schema: *"exit_code 1 when findings reach this severity (high | med | low | never)"*
- `CLAUDE.md` § Standard Ship Sequence step 5 cites `audit_doc_refs --fail-on med`

**Got (scouted reality):** `src/librarian/tools/audit_doc_refs/mod.rs::build_response`
(line 542+) hard-codes only two truthy arms:

```rust
let exit_code: i32 = match fail_on {
    "high" if findings.iter().any(|f| f.resolution.severity == Severity::High
        && !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External)) => 1,
    "any" if n_broken + n_unknown > 0 => 1,
    _ => 0,
};
```

`fail_on="med"` and `fail_on="low"` fall through the wildcard arm and return
exit_code=0 — silently behaving like `never`. The `Severity` enum has all
three variants (High/Med/Low at line 65) so the data is there; the gating
arm just never references Med/Low.

**Probable cause:** Schema/docs were written aspirationally before
build_response was extended to honor the lower thresholds; the extension
never happened. CLI shipping is the natural forcing function — a real
`--fail-on` flag has to either match the docs or silently lie.

**Workaround (2026-05-24, SUPERSEDED 2026-08-07 — see Status):** CLI `--fail-on`
accepts only the values the MCP code actually honors: `high | any | never`.
Matches existing behavior; docs in `librarian.rs` schema + CLAUDE.md need a
follow-up reconciliation (either extend build_response or correct the docs).

**Severity:** med — would have produced a CI gate that silently lets `med`
findings through despite the user believing `--fail-on med` was active.
A noisy bug (silent no-op gates are the worst kind) but not cascading;
controller absorbs the divergence by accepting only verified values.

**Status:** fixed-verified — in two stages 2½ months apart, which is why this sat
`mitigated` long after the hard part was done. Found zombie-open on 2026-08-07 by
reading the CLI's `--help` while investigating task #17.

*Engine (2026-07-05):* `build_response` gained explicit `"med"` and `"low" | "any"`
arms plus an `other => Err(...)` arm, with eight tests (`fail_on_never_is_always_zero`
through `fail_on_unknown_value_is_rejected`). Filed and archived as
`docs/issues/archive/2026-07-05-audit-doc-refs-fail-on-doc-mismatch.md`. The "Fix idea"
below was taken, in its first form.

*CLI (2026-08-07):* the workaround outlived its cause. `src/cli/audit_doc_refs.rs` kept
`value_parser = ["high", "any", "never"]` and help text reading *"`med`/`low` are not yet
honored by the underlying engine — see F-9"* — a live-looking pointer at an already-closed
bug, which is the worst shape a stale doc can take, because a reader who follows it goes
to fix something already fixed. Both surfaces now read one shared list,
`audit_doc_refs::FAIL_ON_VALUES`, so the CLI cannot again refuse a value the engine
honors. Verified on the release binary rather than from the suite:
`[possible values: high, med, low, any, never]`, and per-level exit codes on
`docs/RELEASE.md` are high->0, med->1, low->1, any->1, never->0, with CI's repo-wide
`--fail-on high` still 0 (unchanged).

**Regression guard:** `fail_on_values_are_exactly_what_build_response_honors`
(`src/librarian/tools/audit_doc_refs/mod.rs`) pins both directions — every listed value
must be accepted by `build_response`, and `high`/`med`/`low`/`never` must stay listed.
Mutation-verified: dropping `"med"` from the list fails it with *"`med` is documented in
the librarian input schema and honored by build_response, so the CLI must accept it"*.
The one-directional form (iterate the list, assert each is accepted) stays **green** on a
deletion, which is precisely how this drifted for two months — so the membership
assertion is the load-bearing half, not the loop.

**Fix idea / Pointer:** Done — the first option was taken (extend `build_response`), then
the CLI was aligned to it. One question is deliberately left open, and is now surfaced in
the CLI's own long help rather than buried: `low`/`any` also trip on `resolved_basename`,
a SUCCESSFUL resolution that `docs/RELEASE.md` step 5 calls OK, because the exit-code
exemption covers only `Resolved | External`. Measured 2026-08-07: 3 of 44 findings on
`docs/RELEASE.md`, 2426 of 47094 repo-wide. That is the same `ResolvedBasename` accounting
gap that makes the summary counters non-exhaustive, so it belongs to the audit-doc-refs
direction decision (task #17), not here.

## F-10 — RELEASE-TODO advertises "CI pipeline" as unchecked; workflow already exists, push trigger points at nonexistent branch

**Observed:** 2026-05-24, scouting `.github/` directory before writing the CI
workflow for H-5 + R-1.

**When:** About to `create_file(.github/workflows/ci.yml, ...)` based on
RELEASE-TODO High Priority item *"CI pipeline — GitHub Actions workflow
running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` on
every PR. Single biggest protection against bad contributions."*

**Expected (docs):** No CI workflow exists; needs to be created from scratch.

**Got (scouted reality):** `.github/workflows/ci.yml` exists with 5 jobs:
- `fmt` (cargo fmt --check)
- `clippy` (cargo clippy -- -D warnings)
- `test` (3×3 matrix: linux/macos/windows × default/local-embed/no-features)
- `tool-docs-sync` (lints tools manual stays in sync with `src/tools/*.rs`)
- `msrv` (cargo check on 1.82)

Two bugs in the existing workflow:
1. `on.push.branches: [main]` — the repo's protected branch is `master`
   (per CLAUDE.md § Branch Strategy). Push-trigger is dead.
2. No `audit-doc-refs` job — the lint that catches doc/code drift on PRs.

**Probable cause:** RELEASE-TODO was authored before CI was built and never
updated when CI shipped. The `main` vs `master` branch mismatch is a
copy-from-template artifact (most repos default to `main`).

**Workaround (this session):** Skip the scratch creation; surgically edit
the existing `ci.yml` to (a) fix the push branches list, (b) add the
audit-doc-refs job. Update RELEASE-TODO to reflect the partial-shipped
state.

**Severity:** med — would have produced a duplicate / clobbering workflow
file. The existing workflow is invisible to a controller that trusts
RELEASE-TODO, so a from-scratch write was a real risk.

**Status:** mitigated — edits target the existing file this session.

**Fix idea / Pointer:** `.github/workflows/ci.yml`, `docs/RELEASE-TODO.md`
"High Priority" section.

## F-11 — CI runner missing mold linker required by `.cargo/config.toml`

**Observed:** 2026-05-24, CI smoke test for H-5 + R-1 after sccache fix
shipped. Audit Doc Refs / Clippy / MSRV jobs fail with
`collect2: fatal error: cannot find 'ld'` despite sccache now installing
correctly.

**When:** After mozilla-actions/sccache-action@v0.0.7 fix (7f107d8e) lands;
investigating "next layer" of pre-existing CI rot. Initially diagnosed as a
runner-image regression per researcher MCP synthesis on
`collect2: cannot find 'ld'`. Researcher cited slim-image binutils
omissions and `-fuse-ld=lld` configuration as causes. Scouted local config
to verify which applies.

**Expected (assumption):** Either the runner image is missing
`build-essential`, or the project uses default cc/ld (no special linker
config). Standard GitHub Actions ubuntu-latest should have `ld` in PATH.

**Got (scouted reality):** `.cargo/config.toml` lines 5-7:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "cc"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```
The project mandates the **mold linker** for x86_64 Linux. `collect2` is
GCC's internal linker driver; it reports the missing program as `'ld'` in
its error message regardless of which linker name was actually requested.
Researcher synthesis pointed at this exact false-positive pattern (their
note: *"the system fails to locate specific linkers like `ld.lld` when the
`-fuse-ld=lld` flag is invoked, rather than a total absence of the GNU
linker"*). Mold has the same shape — `-fuse-ld=mold` requires mold to be on
PATH.

Same config also mandates `rustc-wrapper = "sccache"` and `jobs = 64`. The
file looks like a developer's local performance config (mold + sccache +
64 jobs) but is committed to the repo.

**Probable cause:** `.cargo/config.toml` was authored for local build speed
(mold is much faster than ld for large Rust projects) and committed
because it provides project-wide consistency. CI was never updated to
install mold because CI hasn't successfully run since at least 2026-04-13
(per F-10 + the historical run count).

**Workaround:** Add `rui314/setup-mold@v1` step before `cargo build`
invocations in every affected job (clippy, test matrix, msrv,
audit-doc-refs). Format job unaffected (no compile step). Mirrors the
sccache install pattern.

Alternative: override the project rustflags in CI via env, e.g.
`CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=""`. Less clean —
diverges CI from local build behavior.

**Severity:** med — would have required a third CI iteration without the
.cargo/config.toml scout (the researcher MCP correctly pointed at the
`-fuse-ld=` pattern; scout confirmed which linker). Without the scout I'd
have tried `apt-get install -y build-essential binutils` (the obvious
guess) and watched it fail because the real issue isn't binutils.

**Status:** fixed-verified (2026-05-25). Commit `29075470 fix(ci): install mold linker required by .cargo/config.toml` (now on master via merge `d1742c46`) added `rui314/setup-mold@v1` to all four affected jobs — clippy, test, msrv (toolchain 1.88), audit-doc-refs. Verified by grep: 4 occurrences in `.github/workflows/ci.yml` at lines 30, 55, 114, 127. The "two pre-existing items behind this one" (macos/windows runner divergence, tool-docs-sync drift) were also addressed in the Windows portability rounds 1-8 + `4e475bad fix(tests): macOS /private/var symlink`. Same distributed-fix bookkeeping pattern as F-6 — entry stayed `open` because the fix landed under a `fix(ci):` label rather than an explicit "closes F-11" reference.

**Fix idea / Pointer:** `.github/workflows/ci.yml` — add
`uses: rui314/setup-mold@v1` step (after `mozilla-actions/sccache-action`,
before `Swatinem/rust-cache@v2`) in jobs clippy, test, msrv,
audit-doc-refs. Also: `.cargo/config.toml` may belong as
`.cargo/config.toml.example` with a CI-friendly version checked in instead.

## F-12 — Dismissed `references` "use call_graph" warning → shipped half-fix to `extract_surface`, missed build.rs duplicate

**Observed:** 2026-05-24, during CI rot rehab after CI run `26356842338` showed
Windows builds panicking on `extract_surface` with CRLF source.md.

**When:** Mid-fix scout. About to edit `src/prompts/source.rs::extract_surface`,
ran `references(symbol="extract_surface", path="src/prompts/source.rs")` to
verify call sites. Tool returned 3 results AND emitted this warning:

> `warning: references found 3, but call-hierarchy found 5 call sites —
>  rust-analyzer's textDocument/references is incomplete for this symbol.
>  Use call_graph(symbol, direction="callers") for the authoritative caller
>  set.`

**Expected (what I should have done):** Read the warning, run `call_graph` per
its instruction. The 2 missing call sites would have surfaced `build.rs:71`
calling its OWN `extract_surface` (duplicate parser declared at `build.rs:86`,
unreachable from `references` because build.rs lives outside the LSP project
index — it compiles as a separate unit).

**Got (what I did):** Skimmed the warning, treated the 3 hits as the full set,
shipped the CRLF fix only in `src/prompts/source.rs`. Pushed commit `c83b5544`,
verified local tests pass, monitored CI run `26357101302` — Windows tests
failed with the **exact same panic** as before. Spent another 5–10 minutes
investigating "why didn't the fix take" before realizing build.rs has a
duplicate parser the build script actually runs. Shipped follow-up fix in
`af64c737`.

**Probable cause:** F-7 (same tracker, 2026-05-21) already pinned the root
mechanism — `references` queries live `textDocument/references` which is
incomplete on some symbols. The tool surfaces this clearly with a per-call
warning naming the workaround. I dismissed it as a generic "tool noise" line
instead of a specific load-bearing pointer.

**Workaround:** When the `references` warning fires, **always** run
`call_graph(direction="callers")` before assuming the result is exhaustive.
Especially for: build scripts (build.rs), proc-macros, dev-dependencies,
doctests, or any code that lives outside the main crate's LSP index. Their
callers are invisible to live rust-analyzer.

**Severity:** med — cost was one wasted CI cycle (~10 min) + ~5 min of
investigation. Bigger lesson: any tool that explicitly tells you it's
incomplete is signalling at exactly the seams where the incompleteness will
bite. F-7 already documents the *mechanism*; F-12 documents the *cost of
dismissing the surfaced warning*.

**Status:** fixed-verified — `af64c737` shipped CRLF-tolerance to both
parsers. CRLF test in `src/prompts/source.rs` pins LF→CRLF byte-equality.
build.rs has no test surface (it runs at compile time on Windows runners; CI
matrix is the verification).

**Fix idea / Pointer:** None for the tools themselves — both `references`
and `call_graph` work as designed. The remediation is **behavioural**: read
the warning, follow its instruction. Consider promoting this to CLAUDE.md
under a "Tool warning discipline" section if a third datapoint lands.
References: F-7 (root mechanism), `docs/issues/archive/2026-05-21-references-undercounts-vs-call-graph.md`.
## W-5 — Test against semantics, not representation: deserialize before asserting on cross-platform output

**Observed:** 2026-05-24, during Windows CI rehab session — round-2 commit (98907430) shipped 14/16 fixes cleanly but two of them broke on the actual Windows runner despite passing locally on Linux.

**Pattern:** When patching a cross-platform test that asserts on a string containing path or path-like data, do not assert against the serialized text. Parse the response and assert against the deserialized data field instead. Test against the **semantic value**, not the **serialization artifact**.

**Counterfactual:** Without this discipline, round-2's two regressions cost a full CI cycle (~10 min) plus a round-3 fix commit. The specific failures:

1. `run_command_output_keeps_absolute_project_paths` (src/server.rs):
   - Asserted `text.contains(&abs)` where `text` is the JSON-serialized MCP response and `abs` is a raw filesystem path.
   - On Unix: forward-slashes don't need JSON escaping → assertion passed locally.
   - On Windows: JSON-serialized backslashes are doubled (`\` → `\\`) but `abs` has single backslashes → `contains` fails.
   - Fix: parse the JSON, extract `parsed["stdout"].as_str()`, assert against the deserialized string. Platform-agnostic.

2. `resolve_refs_substitutes_cmd_ref` (src/tools/output_buffer.rs):
   - Asserted `resolved.chars().nth("grep hello ".len()) == Some(MAIN_SEPARATOR)`.
   - On Unix: position 11 of `grep hello /tmp/...` is `/` (separator AND absolute-path prefix).
   - On Windows: position 11 of `grep hello C:\Users\...` is `C` (drive letter). MAIN_SEPARATOR is `\`, found at position 13 not 11.
   - Fix: accept either a leading separator OR a drive-letter prefix at positions 0-1. Both shapes valid for "absolute path".

The deeper lesson: a single-OS local pass is necessary but **not sufficient** for cross-platform correctness. Asserting against serialized text bakes in platform-specific serialization quirks (JSON escaping, separator characters as path prefixes). Asserting against deserialized data tests the substantive property under test (path roundtrip) rather than the encoding.

**Confirming data points:**
1. R2026-05-24 Windows CI run 26360060746 — 2 of 5 round-2 fixes regressed on Windows despite Linux pass.
2. Both failures shared the same root cause: assertion against representation, not semantics.

**Impact:** med — saves ≥1 CI cycle (10-15 min) per cross-platform test fix that touches output/serialization. Higher impact for tests that ship to multi-OS matrices where each cycle costs N×OS.

**Promote-when:** A third cross-platform CI fix that would have benefited from the "deserialize-then-assert" discipline. At 3 datapoints, promote to `CLAUDE.md` under a "Testing Patterns" section: "When asserting on responses that contain platform-varying data (paths, line endings, drive letters), parse and assert against the deserialized data field. The serialized text bakes in JSON escapes, separator characters, and other platform quirks that mask correctness drift."

**Status:** validated — single datapoint (this session), pattern explicitly captured in round-3 commit (bc05c0b3) message and code comments. Awaiting promotion criterion.

## W-6 — Cross-platform representation choices must be applied at every read AND write seam

**Observed:** 2026-05-24, Windows-default CI rehab continuation. Round 5 (6771cc1a) introduced forward-slash normalization for catalog writes (`artifact::upsert`, `artifact_id_from_abs`). This closed 6 of 18 failures but left 12 — including tests whose write path was now normalized but whose **read** path (LIKE patterns, scope filters, contains-substring checks) still used native separators.

**Pattern:** When you choose a normalized representation for storage (forward-slash strings, lowercased, base64, whatever) — apply that normalization at **every** boundary that produces a string compared against the stored form. Read-side normalization is not optional; it's the symmetric half of the choice. Missing read-side normalization is a silent miss because:
- Unit tests on the writer pass (forward-slash output matches forward-slash assertion).
- Unit tests on the reader pass (native LIKE pattern matches native stored string — until the writer changes).
- The cross-cutting bug only surfaces in integration where writes flow into the reader.

**Counterfactual:** Without W-6's discipline, round 5 looked complete (6 fixes shipped, indexer tests green) but 12 of 18 originally-failing tests still failed. Each was a separate boundary I'd missed:
- `delete_orphan_repos` LIKE pattern (2 tests) — wiped every row because the keep clause matched nothing on Windows.
- `path_prefix_clause` in scope filter (1 test) — filtered out all rows because the prefix had backslashes but stored rows had forward-slashes.
- `audit_doc_refs::parser::md_file` (2 tests) — map keys had backslashes; assertion expected forward-slashes.
- `audit_doc_refs::severity::matches_archive/matches_issues` (no failures yet, but latent) — `path.to_string_lossy().contains("docs/archive/")` returns false on Windows because the stringified path has backslashes.

The cost of missing these in round 5 was a wasted CI cycle (~15 min) and a confusing "we shipped 6, why are 12 still red" investigation.

**Confirming data points:**
1. Round 5 → Round 6 in this session: 6 boundaries forgot, 1 (delete_orphan_repos) caused total-row-deletion that would have been a destructive bug in production catalog migration.
2. Pending: any future representation choice in a multi-platform codebase.

**Impact:** high — applies to representation choices broadly (path separator, case normalization, encoding). Saves a CI cycle per missed boundary. Could prevent destructive production bugs (R5 nearly did, on the orphan-cleanup path).

**Promote-when:** A second representation-choice rollout that surfaces missed read-side boundaries. At 2 datapoints, promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` (or a new `docs/conventions/cross-platform-representation.md`) as a checklist for representation rollouts: "list every boundary that produces or compares the representation string; apply normalization at each."

**Concrete checklist (provisional, for future rollouts):**
1. Write site (DB upsert / serialization output): normalize on write.
2. ID generation: normalize on hash input.
3. Query LIKE patterns / IN clauses / WHERE comparisons: normalize on query construction.
4. In-memory filter clauses that compare against the stored form: normalize on filter construction.
5. Substring matching against literal forward-slash patterns (`contains("docs/...")`): normalize the path before checking.
6. Test assertions using the representation: assert against normalized form or normalize both sides before compare.

**Status:** validated — single datapoint (this session), but the pattern is broad enough that the next rollout will validate quickly.

## W-7 — Recon-driven distributed-fix sweep closes zombie-open entries

**Observed:** 2026-05-25, triage scout after user prompt "lets look at the
new frictions and fix them." Initial grep across four tracker surfaces
(U-N usage-frictions, H-N hookify, bug-fix session-log, R-N recon-patterns)
returned 4 open F-N entries in the bug-fix session log: F-5, F-6, F-7, F-11.

**Pattern:** When a fix lands as part of a larger labeled commit
(`fix(ci): install mold linker required by .cargo/config.toml`, or as
ambient cleanup folded into a "cumulative session work" merge) rather than
a commit explicitly naming the tracker entry it closes, the tracker entry
stays `open` indefinitely. The bookkeeping gap is silent — no test
fails, no CI gate trips, no PR comment flags it. The entry only flips
when someone manually verifies it.

Two of four "open" F-N entries this session turned out to be already
fixed:

- **F-6** (clippy 1.95 dormant lints) — verified by `cargo clippy
  --all-targets -- -D warnings` exit 0 on current `experiments` HEAD. The
  cleanup shipped distributed across multiple post-2026-05-20 commits and
  was carried to master in merge `d1742c46`.
- **F-11** (CI missing mold linker) — verified by grep on
  `.github/workflows/ci.yml`: 4 occurrences of `rui314/setup-mold@v1` at
  lines 30, 55, 114, 127, matching the proposed fix's 4-job scope exactly.
  Shipped as `29075470 fix(ci): install mold linker required by
  .cargo/config.toml`.

The Index table at the top of the session log was also stale — F-7, F-9,
F-10, F-11 were never added when those entries were inserted. Recon
caught this as a second-order bookkeeping gap and synced the rows.

**Counterfactual:** Without this scout pass, the next "what's open?"
query would have continued to count F-6 and F-11 as actionable backlog.
Concrete cost over time: one or both would eventually pull a session
into re-investigating an already-shipped fix (estimated 15-30 min of
context per re-investigation), or worse — a user-facing release-readiness
report would have advertised them as known issues. Two zombie entries
caught early; over a year of distributed-fix workflow the steady-state
count would likely grow.

**Confirming data points:**
1. F-6 — clippy 1.95 cleanup (this session). Shipped distributed across post-2026-05-20 commits, carried to master in merge `d1742c46`.
2. F-11 — CI mold linker install (this session). Shipped as `29075470 fix(ci): install mold linker required by .cargo/config.toml`.
3. F-7 — `references` undercount mitigation (this session). Shipped as `3d984e77 feat(tools): ... add references completeness guard`, root-cause refinement as `495c8640`, bug file archived to `docs/issues/archive/2026-05-21-...md` with `status: mitigated`.

**Impact:** med-to-high — saves N×30min per zombie entry detected, plus prevents false-positive items in human-facing backlog reports. 3-of-4 nominally-open F-N entries this session were actually closed weeks ago (75% zombie-open rate in this tracker).

**Promote-when:** FIRED — 3 datapoints in one scout pass. Promote to CLAUDE.md as project-wide cadence rule: "Before any 'what's open?' report or backlog triage, run a verify-open pass on bug-fix session-log entries with `Status: open` older than 14 days. Distributed fixes leave entries zombie-open by default — the absence of a commit message naming the tracker entry is not evidence the fix didn't ship."

**Related patterns:** the same shape applies to `docs/issues/*.md` bug files whose Fix section cites a SHA but whose `status:` frontmatter never flipped — see CLAUDE.md Standard Ship Sequence step 4 (the archive-move discipline) for the analogue at the bug-tracker level. Also pairs with the audit_doc_refs lint at the doc-link level: three independent surfaces (session-log entries, bug-file frontmatter, doc-link targets) all drift the same way under the same root cause — fix-then-forget.

**Status:** promoted-to-permanent-docs — graduated to CLAUDE.md § Ad-Hoc Session Logs as the "Verify-open cadence" rule (2026-05-25).

## W-8 — `prompt_surfaces` test gate catches cap violation before ship

**Observed:** 2026-05-25, Frog audit Gap 1 fix attempt. Added a "## Path
annotation" section to `src/prompts/source.md` server_instructions
surface, ran `cargo clippy` + `cargo fmt --check` (both green),
committed as `4cc49ccb`. Later `cargo test --lib` (full suite) caught
two failures: `prompts::redesign_invariants::source_md_under_cap`
(2139 chars; cap is 1800) and
`prompts::tests::prompt_surfaces_server_instructions_snapshot`
(1741 expected, 2139 actual).

**Pattern:** Whenever a commit touches any of the three prompt surfaces
(`src/prompts/source.md`, `src/prompts/guides/*.md`,
`build_system_prompt_draft()` in `src/prompts/builders.rs`), run
`cargo test --lib prompt_surfaces` BEFORE committing. `cargo clippy` +
`cargo fmt --check` are NOT sufficient — they don't run the cap test,
the snapshot test, or the cross-surface tool-name lint
(`server::tests::prompt_surfaces_reference_only_real_tools`). Each
of those three tests guards a different invariant; missing any one
ships a broken surface silently. The cap test is the load-bearing one
because Claude Code's MCP client truncates `initialize.instructions`
at ~2 KB (per U-8) and a silent overflow corrupts every session's
context for every project.

**Counterfactual:** Without `source_md_under_cap` catching the 2139-byte
overflow, commit `4cc49ccb` would have shipped a server_instructions
surface 339 bytes over cap. Claude Code MCP clients would have
truncated the surface at ~2 KB, silently cutting off the tail —
specifically: the "## Workspace gate" section and the "## Deeper
guidance" section (with the get_guide topic pointers). Every fresh
session for every project using the codescout MCP server would have
lost workspace-restore guidance (Iron Law-adjacent — leaking
foreign-project state) and the get_guide pointer table (the deep-
documentation discovery surface). Estimated steady-state cost: every
session with a workspace switch would risk failing to restore the
home workspace before turn end. Once shipped, the surface change is
live within seconds at every `/mcp` reconnect — no way to detect
truncation client-side until a downstream slip surfaces.

**Confirming data points:**
1. 2026-05-25 this session — Gap 1 fix attempt at commit `4cc49ccb`.
   Caught + corrected + relocated to progressive-disclosure guide
   (commit `a0d9e3b6` then cherry-picked to master as `8c101b91`).
2. Pending — any future prompt-surface edit where the test gate
   catches a violation that `clippy`/`fmt` missed.

**Impact:** high — the cap violation, if shipped, would have
degraded every MCP session for every consumer until manually
reverted. The detect-after-ship loop is ≥1 affected day per
consumer per detection delay.

**Promote-when:** A second prompt-surface edit (in any future
session) where the `prompt_surfaces` test gate catches a violation
that `clippy`/`fmt` missed. At 2 datapoints, promote to CLAUDE.md
§ Prompt Surface Consistency as a discipline rule: "Run `cargo test
--lib prompt_surfaces` before committing changes to any of the three
prompt surfaces (`source.md` slices, `build_system_prompt_draft()`).
`clippy` + `fmt` are not sufficient — they do not run the cap test,
snapshot test, or cross-surface tool-name lint."

**Status:** validated — single datapoint this session; the test gate
worked exactly as designed. Awaiting promotion criterion.

## F-13 — Wrote CHANGELOG entry under wrong version label (`0.13.0` instead of `0.14.0`)

**Observed:** 2026-05-25, Frog audit Phase 2, drafting the next-release CHANGELOG entry.

**When:** Composing `## [0.13.0] — 2026-05-25` heading for the `body_edits` shipment. About to commit + tag.

**Expected:** `Cargo.toml`'s `version = "0.13.0"` reads as "next-to-be-released version." Pick `0.13.0` for the CHANGELOG heading + git tag.

**Got (scouted reality):** `git tag --list 'v*'` showed `v0.13.0` already present (commit `b744fbd3 chore: bump version to 0.13.0`). The crates.io API confirmed `0.13.0` published `2026-05-18T13:31:22Z` — 7 days before this attempt. `Cargo.toml` had been bumped on 2026-05-18 and the version sat unchanged through 160+ subsequent commits. Per CLAUDE.md semver ("minor for new features"), `body_edits` justified `0.14.0`, not `0.13.1`.

**Probable cause:** Frog Phase 1 ("Frame — check what already exists") was scoped to repo-internal sources (manual, prompt guides, architecture page) but not to external release state. The `release-notes-soul` memory hints at this — "Pre-merge punchlist for a folded release: bump `Cargo.toml`, land `experiments` → `master`, decide whether `Unreleased` graduates into the same version, then publish to crates.io and `gh release create`" — implies `Cargo.toml` may lead crates.io. I knew this abstractly; didn't verify before writing.

**Workaround:** Self-corrected openly, `edit_markdown` swap `[0.13.0]` → `[0.14.0]` in CHANGELOG heading, `edit_file` swap `version = "0.13.0"` → `version = "0.14.0"` in `Cargo.toml`. Continued release sequence as `v0.14.0`.

**Severity:** med — caught pre-publish during user-prompted release prep; cost was one tag-list query, one crates.io API call, and two edits. Had I run `cargo publish` first (which I almost did before re-checking), it would have errored with `version already exists in registry` mid-flight and forced a version roll with a dirty already-pushed commit.

**Status:** fixed-verified — `v0.14.0` shipped successfully (codescout 0.14.0 on crates.io, master at `9e6edfc9`).

**Fix idea / Pointer:** Frog `SKILL.md` Phase 1 should add an explicit release-state verification step when the doc target is release-related (CHANGELOG, release notes, version-tied doc): _"Before writing a version-numbered heading, scout `git tag --list 'v*' --sort=-v:refname | head -1` and the crates.io API. The repo's `Cargo.toml` may lead the registry — confirm before committing to a version label."_ Cross-link from the project-side memory `release-notes-soul`.

## F-14 — `cargo publish` failed on `include_str!` path stripped by `Cargo.toml` exclude

**Observed:** 2026-05-25, first `cargo publish v0.14.0` attempt. Pre-publish gates (`cargo build --release` + `cargo test` (2620 passed) + `cargo clippy --all-targets -- -D warnings`) all green.

**When:** Standard Release Cycle step 5 (`cargo publish`), after step 2 (build/test/clippy) verified the working tree.

**Expected:** Verification compile inside the packaged tarball succeeds. The tarball is a hermetic snapshot of the crate.

**Got (scouted reality):**

```
error: couldn't read `src/../docs/PROGRESSIVE_DISCOVERABILITY.md`: No such file or directory (os error 2)
  --> src/server.rs:787:22
   |
787|             content: include_str!("../docs/PROGRESSIVE_DISCOVERABILITY.md"),
```

`Cargo.toml` had `exclude = [".codescout/", "docs/", "scripts/", ".github/", "CLAUDE.md"]`. `cargo package` strips `docs/` from the tarball, then the verification compile inside `target/package/codescout-0.14.0/` fails. `cargo build` from the repo root never trips it because the working-tree `docs/` is still in place.

**Probable cause:** Two paths diverge: (a) dev/CI build runs from the working tree where `exclude` doesn't apply; (b) publish runs in the packaged tarball where it does. Only path (b) tests the actual published artifact, and there's no CI step that exercises it — no `cargo package --list` or `cargo publish --dry-run` gate.

**Workaround:** Cargo's `exclude` accepts gitignore-style negation. Patched the list with `"!docs/PROGRESSIVE_DISCOVERABILITY.md"` after `"docs/"` — keeps the one needed doc in the tarball while excluding the rest. Verified via `cargo package --list --allow-dirty | grep -i progressive`, then amended the bump commit, re-pinned `v0.14.0` tag, re-published successfully.

**Severity:** high — publish-blocker. Manifested only at the irreversible step (`cargo publish`); pre-publish gates couldn't detect it. Future `include_str!("../docs/...")` additions will silently regress this until a CI gate or pre-commit hook covers it.

**Status:** fixed-verified — v0.14.0 published cleanly post-fix; the negation pattern shipped in `9e6edfc9`.

**Fix idea / Pointer:** Add a CI step that runs `cargo publish --dry-run` (or at minimum `cargo package --list` with an assertion that every `include_str!("../...")` path appears in the listing). Alternatively, a `pre-commit` hook grepping `src/` for `include_str!.*"\.\./` and cross-checking against `cargo package --list`. Track as its own bug file if no such CI step exists. Cross-references: `Cargo.toml` `exclude` list at repo root, `src/server.rs:787`.

## W-9 — Spot-check sibling callers of a just-fixed shared helper before closing the bug class

**Observed:** 2026-06-05, after fixing the `edit_code insert-after` parent-clamp off-by-one
(`do_insert`) for the last child of a dedent-delimited (Python) class. User asked to spot-check the
flagged replace-path lead before declaring the class of bug closed.

**Pattern:** When a fix corrects how ONE caller uses a shared boundary helper (here:
`parent.end_line` as an *exclusive* clamp bound), `references()` every other caller of that helper
and reproduce the same input shape against each before claiming the bug class is closed.
`references(clamp_range_to_parent)` surfaced two more production callers — `do_remove`
(`src/tools/symbol/edit_code.rs:454`) and `do_replace` (`:515`) — both using the identical bare
`parent_body_end_exclusive = parent.end_line` (no `+1`).

**Counterfactual:** The insert-only fix would have shipped while `edit_code action="replace"` and
`action="remove"` still silently corrupted the LAST method of any Python class. Live repro confirmed:
replacing `C/last` reported `replaced_lines: 5-9` — excluding line 10 (the trailing `assert x`) —
leaving the orphaned statement after the new body. A user replacing the last test in a test class
would get a half-replaced method + a leftover assertion: silent, `status: ok`, invisible to the
error-rate metric. Without the spot-check, 2 more silent-corruption paths ship and are each
re-discovered later at full debugging cost (this root-cause hunt took ~30 tool calls and 3 refuted
hypotheses).

**Confirming data points:**
1. Three callers, one root cause: `do_insert` (fixed), `do_remove` (`edit_code.rs:454`), `do_replace`
   (`:515`) all clamp with `parent_body_end_exclusive = parent.end_line`.
2. Live `edit_code replace` on `C/last` reproduced the leftover `assert x` against the shipped binary
   (`replaced_lines: 5-9`, off by one).

**Impact:** high — silent file corruption across 3 `edit_code` actions; the spot-check converted a
partial fix into a complete one.

**Promote-when:** A second instance where grepping sibling callers of a just-fixed shared helper
catches an under-scoped fix. At 2 datapoints, promote to CLAUDE.md: "When fixing a shared
boundary/clamp helper, `references()` every caller and reproduce each input shape before closing the
bug class."

**Status:** validated

---
## F-15 — Bug-file `project=`→`project_id=` fix plan misses a build-breaking 3rd test assertion + cites a non-existent fixture

**Observed:** 2026-06-09, scouting the only open bug (`docs/issues/archive/2026-06-09-onboarding-prompt-uses-project-not-project-id.md`) before editing `src/prompts/builders.rs`.

**When:** Pre-edit recon of the `project=` → `project_id=` fix across `build_per_project_prompt` / `build_synthesis_prompt` and their tests.

**Expected (bug-file Fix plan):** Edit builder emissions at `builders.rs:828,872-874,893` (and "verify" `:836` semantic_search); update assertions at `src/tools/run_command/tests.rs:286-287`; "refresh the prompt-surface fixtures (`tests/fixtures/prompt_surfaces/onboarding_prompt.md`)"; "consider a version bump."

**Got (scouted reality):**
- A **third** assertion at `src/tools/run_command/tests.rs:257` — `prompt.contains("project=\"backend\"")` in `build_per_project_prompt_contains_project_context` — also pins the buggy string. The plan cites only 286-287. After the builder emits `project_id="backend"`, `project=` is no longer a substring of `project_id=`, so line 257 fails `cargo test`. Following the plan verbatim breaks the build.
- `tests/fixtures/prompt_surfaces/` AND `src/prompts/source.md` hold **zero** `project=` occurrences (`grep project=` → 0). The builder prompts are ephemeral `.codescout/tmp/onboarding-project-<id>.md` files, NOT sliced into the `onboarding_prompt` surface. The fixture-refresh step is a dead lead; no snapshot to update, and (per CLAUDE.md surface table) NO `ONBOARDING_VERSION` bump is needed.
- `:836` semantic_search confirmed buggy — both `memory` and `semantic_search` tool schemas use `project_id`, neither accepts `project`. Resolvable in the same edit, not "verify separately."
- Two extra `project=` hits the plan omitted: `:715` (doc comment) and `:882` (`no \`project:\` parameter` prose).

**Probable cause:** The bug file's Fix section was written from hand-cited line numbers, not a full-tree `grep project=` sweep; the fixture line was assumed by analogy to the `onboarding_prompt` surface without confirming the builder output belongs to that surface.

**Workaround:** Corrected blast radius — `builders.rs` (7 hits: 715, 828, 836, 872-874, 882, 893) + `run_command/tests.rs` (3 hits: 257, 286, 287) + fixtures (0). Fix all three test assertions, skip the fixture step, no version bump.

**Severity:** med — the plan as written ships a builder fix that fails `cargo test` at `tests.rs:257`; controller absorbs it on first test run, but a subagent told to "update 286-287 per the bug file" would have flailed.

**Status:** fixed-verified — shipped 2026-06-09 (experiments `890da4d6`); the 3rd assertion at `tests.rs:257` and all blast-radius hits corrected, no version bump. Reconciled.

**Fix idea / Pointer:** `docs/issues/archive/2026-06-09-onboarding-prompt-uses-project-not-project-id.md`; this session.

---

## W-10 — Full-tree `grep <token>` before editing beats the bug-file's hand-cited line list

**Observed:** 2026-06-09, pre-edit recon of the `project=`→`project_id=` onboarding-prompt fix.

**Pattern:** Before editing to fix a token that appears in both source and test assertions, run one workspace-root `grep <token>` rather than trusting the bug file's hand-cited line numbers. Reconcile every hit — especially test assertions that pin the OLD string and will flip from green to red when the source changes.

**Counterfactual:** The bug file named `tests.rs:286-287` as the only tests to touch. Applying the builder fix + those two assertions would have left `tests.rs:257`'s `contains("project=\"backend\"")` asserting a string the new builder no longer emits → red `cargo test`, ≥1 debug round-trip to find the third assertion. It would also have burned time hunting for `project=` in `tests/fixtures/prompt_surfaces/onboarding_prompt.md` (0 occurrences — wrong surface) and possibly an unnecessary `ONBOARDING_VERSION` bump.

**Confirming data points:**
1. F-15 (this session) — grep found a 3rd build-breaking assertion + a non-existent fixture lead the bug file's Fix section missed.
2. F-3 / W-2 (2026-05-18) — same shape: plan cited test accessors that didn't match reality; pre-edit scout caught it.
3. codescout `reconnaissance-patterns.md` R-3 — grep scope = workspace root, not the file being modified; assertions and token substitutions cross module boundaries.

**Impact:** med — saves ≥1 failed test cycle and a fixture wild-goose-chase per such fix.

**Promote-when:** A third datapoint where a hand-cited bug-file line list omits a build-breaking test assertion. Then promote to CLAUDE.md bug-fix discipline: "Before fixing a token that appears in test assertions, `grep` it workspace-wide; the bug file's line list is a starting point, not the blast radius."

**Status:** validated

---
## W-11 — Verify-open reconciliation against code+git de-zombies a backlog at scale

**Observed:** 2026-06-09 to 06-11, a full survey of all trackers/bugs for remaining work; ran a 4-scout verify-open reconciliation across ~46 open friction/work items.

**Pattern:** Do not trust a tracker's `Status`/`open` field for a backlog report — reconcile each open item against current code + git history (`git log -S`/`--grep`, `symbols`, `grep`) and classify STILL-OPEN vs DONE-SINCE with evidence.

**Counterfactual:** Most open items were already shipped. `i1-refactor-tasks` self-reported 13 of 14 pending; reconciliation found ALL 14 shipped (`d11e830e`..`d8b38f26`). `lsp-tools-error-rate` listed 3 open root causes, all fixed (`78d1e392` + `retry_on_mux_disconnect`). Without the reconciliation a triage would have re-investigated (or re-implemented) dozens of done items and reported a falsely-huge still-to-do list.

**Confirming data points:** (1) i1-refactor 14/14 done, archived. (2) lsp-tools-error-rate 3/3 done, archived. (3) skill-frictions `/onboarding` F-001/F-002 reported open by a scout but marked (FIXED 2026-05-07) inline. (4) F-15 flipped to fixed-verified once the project_id fix shipped (`890da4d6`).

**Impact:** high — converted an untrustworthy ~46-item open list into a small real backlog; prevented rework on shipped features.

**Promote-when:** already partially codified (CLAUDE.md verify-open cadence, W-7). On a third large zombie-open sweep, promote a reindex-then-reconcile-before-any-backlog-report line into the Standard Ship Sequence.

**Status:** validated — single large sweep this session; reinforces the existing verify-open cadence (W-7, 2026-05-25).

---
## F-16 — Inherited "`edit_code` crashes the Kotlin LSP" claim was a misdiagnosis; real cause was an orphaned RocksDB index-lock holder

**Observed:** 2026-06-11, debugging a foreign project (`~/work/mirela/backend-kotlin`) whose prior session concluded "the `edit_code` write-path reliably crashes the Kotlin LSP (read-path `symbols` works), and `edit_file` refuses new `fun` definitions — so I'll use `create_file` instead."

**When:** Start of a systematic-debugging pass, before accepting the inherited diagnosis or filing any bug.

**Expected (inherited claim):** `edit_code` apply disconnects the Kotlin LSP; `symbols` (read) is unaffected — i.e. a write-path defect in `edit_code`.

**Got (scouted reality):** `<project>/.codescout/usage.db` `tool_calls` + `lsp_events` show `symbols` disconnecting *identically* (`LSP server disconnected`, rows 16934/16935/16944) and **every** `kotlin new_session` failing at ~4.9s. `debug.log` `lsp_stderr`: `org.rocksdb.RocksDBException: … rocks/v492/LOCK: Resource temporarily unavailable`. `fuser` on that LOCK named PID 2699281 — an orphaned direct-fallback `intellij-server` (owned by a 20h-old session's `codescout start --debug`) squatting on the shared RocksDB index lock. `edit_code` differs from `symbols` only because path-`symbols` can fall back to tree-sitter AST when the LSP is dead; `edit_code` needs `document_symbols` and cannot. `edit_file`'s "refuses `fun`" is the by-design structural-edit gate, not a fault.

**Probable cause:** The prior session inferred causation from transcript adjacency — an `edit_code` call was *interrupted by the user*, then narrated as "crashes the LSP" — without checking per-call `outcome` in usage.db. Correlation-in-transcript ≠ causation.

**Workaround:** `kill 2699281 2699279` freed the lock; a fresh `intellij-server` (PID 3029079) started and `fuser`-confirmed it holds the lock — Kotlin LSP recovered. Codescout-side defects filed in `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`.

**Severity:** high — accepting the claim would have sent the session hunting/patching `edit_code` + the AST extractor (neither broken) and normalized the `create_file` workaround, while the actual fix (kill the lock holder) was never attempted, leaving the user's Kotlin LSP dead indefinitely. Wrong-target-fix risk.

**Status:** fixed-verified — misdiagnosis corrected, root cause confirmed via `fuser`/process state, environment recovered + verified live, bug filed.

**Fix idea / Pointer:** `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`; `src/lsp/manager.rs:456` (flock-only liveness), `:485` (stderr→null), `:318` (direct-LSP fallback collision).

## W-12 — Re-derive an inherited "tool X breaks Y" claim from usage.db outcomes before acting on it

**Observed:** 2026-06-11, inheriting a prior session's conclusion that `edit_code` crashes the Kotlin LSP in `~/work/mirela/backend-kotlin`.

**Pattern:** When a session inherits a causal claim about tool behavior ("`edit_code` crashes the LSP", "`symbols` is fine"), re-derive it from the ground-truth telemetry — `<project>/.codescout/usage.db` `tool_calls.outcome`/`error_msg` and `lsp_events.outcome` — before filing a bug or attempting a fix. A transcript shows *order*, not *cause*; the per-call `outcome` columns show cause.

**Counterfactual:** Without the usage.db scout, this session would have accepted "edit_code write-path crashes the LSP" and (a) opened a bug against `edit_code` / the AST extractor (neither is broken), (b) treated the `create_file` workaround as the norm, and (c) left the *actual* cause — a kotlin-lsp deadlock on the shared RocksDB index lock — untouched. The scout (3 SQL queries against usage.db + 1 `fuser`) flipped the entire diagnosis from a non-existent `edit_code` defect to an environmental RocksDB-lock deadlock, which was ultimately resolved by clearing the deadlock + a fresh mux spawn (`edit_code` success confirmed: usage.db rows 16949/16950).

**Correction (same session):** the first "recovery verified" claim was wrong and is logged as its own lesson — a single `kill` only *rotates* the RocksDB lock-holder, and "verifying" recovery by issuing a call from a second client re-creates the contention (it spawned a squatter that broke the user's first restart). Captured as R-23 in `docs/trackers/reconnaissance-patterns.md`. The *re-derive-from-telemetry* pattern itself held; only the verification method was flawed.

**Confirming data points:**
1. F-16 (this session) — `symbols` disconnects identically to `edit_code` in usage.db; the claimed write-path asymmetry was a tree-sitter-AST-fallback artifact, not causation.
2. F-7 (this log, 2026-05-21) — a similar "tool X undercounts" surface claim whose root was live-LSP incompleteness, not the tool; resolved by checking the actual data path rather than the surface symptom.

**Impact:** high — converts a wrong-target debugging spiral into an environmental fix and prevents a spurious bug against a correct tool.

**Promote-when:** A second inherited tool-behavior claim is overturned by re-deriving from usage.db telemetry. At 2 datapoints, promote to CLAUDE.md as "Before acting on an inherited 'tool X breaks Y' claim, re-derive it from usage.db `tool_calls.outcome` + `lsp_events` — transcript adjacency is not causation."

**Status:** validated — diagnosis overturned via telemetry; environment recovered + confirmed (`edit_code` success in usage.db). Verification-method caveat logged as R-23.
## F-17 — `references`/`symbol_at`/`call_graph` ignore the `workspace=` pin for relative path resolution

**Observed:** 2026-06-11, during a 3-worktree concurrent kotlin-lsp stress test (3 subagents pinned to backend-kotlin worktrees + one controller call against the warm live mux).

**When:** each agent called `references(symbol, path=<project-relative>, workspace=<worktree abs>)` right after `symbols(path=<same relative>, workspace=<worktree>)` had succeeded.

**Expected:** the `workspace=` pin resolves the relative `path` against the pinned workspace, as `symbols`/`read_file` do.

**Got:** `path not found: ktor-server/.../CalendarService.kt` — 4/4 reproductions (3 agents + controller; warm 100-min mux and cold worktree muxes). Absolute path resolves.

**Root cause:** `references.rs:143` (and `symbol_at.rs:74/202`, `call_graph/mod.rs:402`) call the unpinned `resolve_read_path(&ctx.agent, rel_path)`; the pinned `resolve_read_path_for(…, ctx.workspace_override, …)` used by `list_overview.rs:277` was not propagated. `src/fs/mod.rs:44-51` documents the trap; `read_file` already closed the same gap.

**Severity:** med — three core LSP-nav tools silently fail under the documented per-request `workspace=` pin (the safe-concurrency mechanism). Workaround: absolute path.

**Status:** promoted-to-bug-tracker — `docs/issues/archive/2026-06-11-lsp-tools-ignore-workspace-pin-path.md` (open; 4-site swap + 3 per-tool regression tests planned).

**Fix idea / Pointer:** swap to `resolve_read_path_for` at the 4 sites; mirror `read_file_honors_workspace_override_pin`.

## F-18 — Kotlin-lsp cold-start did not survive 3× concurrent (6-JVM) cold start; tree-sitter fallback masked it on `symbols`

**Observed:** 2026-06-11, same 3-worktree stress test. Each backend-kotlin worktree is polyglot (Kotlin `ktor-server` + Rust `eduplanner-mcp`), so 3 kotlin-lsp + 3 rust-analyzer cold starts fired at once.

**When:** all three agents' first `symbols(path=<.kt file>)` triggered a kotlin-lsp cold start (`get_lsp_client`, `list_overview.rs:287`).

**Expected:** each worktree's kotlin mux comes up and persists (300s idle).

**Got:** agent #2's first call returned `LSP server disconnected` (recovered next call); all three worktree kotlin muxes ended as **orphan sockets** (bound, then exited via error path — no listener, no process) within ~3 min, well under idle timeout. `symbols` still returned correct data via tree-sitter fallback (masking the LSP failure); `references` (needs the LSP) returned definition-only.

**Root cause:** unconfirmed — codescout logs to stderr (host-captured, no file log this session). Strong hypothesis: 6 heavy LSP-JVM cold-starts at once → kotlin (IntelliJ) JVMs lost the init race under resource pressure → muxes errored after binding the socket. NOT RocksDB-lock contention (each worktree = isolated hash, no shared lock) and NO "mux process failed to start" cascade — a milder, different failure mode than the deadlock in `issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision`.

**Severity:** med — kotlin-lsp effectively unavailable under heavy concurrent cold start, silently masked on `symbols` by tree-sitter.

**Status:** open — mechanism log-unconfirmed.

**Probe (2026-06-11):** single-worktree cold-start probe (cs-probe) was INCONCLUSIVE via black-box. The pinned `symbols`+`references` calls touched NO kotlin-lsp-home dir (`find -mmin -5` empty) and spawned NO mux — the kotlin LSP was not exercised at all; results came from tree-sitter + grep. The probe symbol `CalendarService` is also non-discriminating: the warm live mux (`26a9e85d`, 100-min-old) returns the IDENTICAL "0 cross-file + warming" output, so `references` cannot tell LSP-up from LSP-absent. Proper confirmation needs `RUST_LOG=codescout::lsp=debug` + stderr capture, or a gated integration test asserting on `LspManager::get_or_start`. **Lead:** the 3× orphan-mux deaths were NON-index-contention init failures; Fix 4 only blocks poisoning for index-contention, so `config.mux` may have been poisoned to false after the stress run — which would explain cs-probe never attempting a mux. Confirm whether non-index-contention mux failures still poison the direct-fallback.

**Lead resolved (code, 2026-06-11) — FALSIFIED.** `config` in `LspManager::get_or_start` (`src/lsp/manager.rs:332`) is a per-CALL local; `config.mux = false` (`:372`) only redirects THIS call to direct mode — there is no cross-call/persistent poisoning. Fix 4's early-return on index-contention (`:363-365`) prevents THIS call's direct fallback from becoming a RocksDB-lock SQUATTER (per its comment), NOT config poisoning. So Fix 4 is correctly scoped; no broadening needed. The cs-probe no-mux behavior is therefore NOT poisoning and remains unexplained (candidates: cold-start exceeding the tool's internal LSP timeout → tree-sitter fallback returned before the mux bound; circuit-breaker window already expired at +30min). Resolve via the RUST_LOG harness.

**Fix idea / Pointer:** confirm with a single-worktree cold-start probe (expected to succeed) vs the 3× burst. If real, consider cold-start admission control / serialized JVM spawn, or surfacing "LSP warming, retry" instead of bare "disconnected".

## W-13 — Test shared-LSP behavior via isolated worktree hashes, not contention on the live hash

**Observed:** 2026-06-11, designing the "test the kotlin-lsp mux under multi-agent load" run after last session's R-23 miss (a 2nd-client call re-created contention on the live hash and SIGKILLed the user's working mux).

**Pattern:** the mux lock/socket + RocksDB home are keyed on `workspace_hash(workspace_root)` — the PATH (`src/lsp/mux/mod.rs:15-28`, `servers/mod.rs:81`; regression `socket_path_deterministic_for_same_workspace`). Scout that FIRST, then `git worktree add` N paths → N isolated hashes → fan out N agents pinned per-worktree. Concurrent cold starts are exercised without any path able to collide with the live working mux.

**Counterfactual:** without the isolation insight the only way to drive concurrent cold-start was N clients on the live `26a9e85d` hash — exactly the R-23 move that broke the env last session. The scout converted an unsafe-to-run test into a safe one; the live mux was verified untouched (same PIDs + lock timestamps) before and after.

**Confirming data points:**
1. This session — 3 worktrees, 3 concurrent agents; live mux PIDs 3071881/3071947/3071949 unchanged throughout.
2. R-23 (last session) — the negative: 2nd-client-on-live-hash broke the env.

**Impact:** high — unblocks all future concurrent-LSP testing on a live machine.

**Promote-when:** a second concurrent-LSP test reuses the worktree-isolation harness. At 2 datapoints, promote to CLAUDE.md (Testing Patterns) as the canonical concurrent-LSP test method.

**Status:** validated — single datapoint; harness worked, live mux provably untouched.
## F-19 — Explore-subagent report claimed `OutputForm::Text` bypasses the output buffer; `call_content` buffers unconditionally before the form branch

**Observed:** 2026-06-12, fixing `get_guide` returning a `@tool_*` handle ("Result stored in @tool_babfbd35 (14618 bytes)") instead of the full guide.

**When:** About to implement the fix, having dispatched an `Explore` subagent to map the buffer-routing mechanism.

**Expected (subagent report):** The report stated *"OutputForm::Text forces inline output always"*, listed `symbols` / `read_markdown` / `memory` / `call_graph` as tools that *"force inline output always"*, and recommended **Option A: override `output_form()` to `OutputForm::Text`** as the idiomatic fix.

**Got (scouted reality):** `Tool::call_content` (`src/tools/core/types.rs:485-615`) checks `exceeds_inline_limit(&json)` **unconditionally** at line ~554, *before* any form-based branching. `OutputForm::Text` only selects `format_compact` vs pretty-JSON inside the small-output `else` branch — it does **not** skip the overflow→buffer branch. So `symbols`/`memory` ARE buffered when large, and Option A would have left the 14 KB librarian guide buffered exactly as before.

**Probable cause:** The subagent inferred the mechanism from doc comments + the list of `output_form` override sites without tracing the branch *order* in the function body. The `output_form` doc comment ("compact summary only on buffered overflow") reads as if `Text` alters the overflow path — a plausible-but-wrong reading.

**Workaround:** Read the `call_content` body directly (`symbols(Tool/call_content, include_body=true)`), then implemented a dedicated `force_inline()` trait hook (default `false`) gating the overflow branch — `if exceeds_inline_limit(&json) && !self.force_inline()` — overridden `true` on `GetGuide`. Verified: new regression test + all 54 core `call_content` tests pass; clippy clean.

**Severity:** med — trusting the report would have compiled clean (a valid `output_form` override) but the large-topic behavioral test would have failed, costing a full wrong-implementation + re-debug cycle. Controller-absorbable, but real rework.

**Status:** fixed-verified — gap caught pre-implementation by scouting the actual fn body; correct fix landed + tested this session.

**Fix idea / Pointer:** `force_inline()` in `src/tools/core/types.rs`; `GetGuide` override + regression test in `src/tools/guide.rs`. Commit pending (cite master SHA after cherry-pick). Cross-cutting lesson → R-27 in `reconnaissance-patterns.md`.

---

## W-14 — Verify a subagent's control-flow *mechanism* claim against the actual fn body before building a fix on it

**Observed:** 2026-06-12, about to implement the `get_guide` inline-output fix off an `Explore` subagent's investigation report.

**Pattern:** When a subagent (or any prose artifact) asserts *how* control flow behaves — "X bypasses Y", "branch B fires when C" — read the actual function body before building a fix on the claim, especially when the claim names a specific branch/ordering. A `symbols(Sym, include_body=true)` is one call; an internally-inconsistent report is the "sounds right" red flag from CLAUDE.md's Iron Rules.

**Counterfactual:** The report recommended "Option A: set `output_form = Text`" and even flagged it "Recommended". It also contradicted itself (claimed Text "forces inline always" yet noted "the buffering check still applies"). Acting on Option A: GetGuide gets an `output_form() -> Text` override (compiles fine), tests run, the 14 KB librarian topic *still* returns a `@tool_` handle → behavioral test fails → re-debug, discover `exceeds_inline_limit` gates the branch unconditionally → redo the fix. Reading `Tool/call_content` (130 lines) up front collapsed that loop to zero.

**Confirming data points:**
1. F-19 (this session) — the report's core mechanism claim was false; the unconditional `exceeds_inline_limit` check at `types.rs:~554` runs before the `OutputForm` branch.
2. F-3 / W-2 (2026-05-18) — same family: a prose artifact (plan) asserted a code shape (`RecoverableError.hint` field) that scouting the body refuted before any wrong code was written.

**Impact:** med — saved one wrong-implementation + re-debug cycle; prevented shipping a non-fix that passes compile but not behavior.

**Promote-when:** A third datapoint where scouting the body refutes a subagent's *mechanism* (not just *shape*) claim. At that point promote to CLAUDE.md: "A subagent's description of control flow is a hypothesis; read the fn body before building a fix on it."

**Status:** validated — single fresh datapoint (F-19) plus the prior shape-claim family (F-3/W-2).

---
## F-20 — Targeted `cargo test --lib guide::` missed the `tool_descriptions_stay_under_budget` server gate; shipped an over-budget get_guide description

**Observed:** 2026-06-12, after the get_guide inline-output fix (commit `c799e887`).

**When:** Verifying the get_guide description edit before committing — ran `cargo test --lib guide::` + `cargo test --lib tools::core::` + clippy, all green.

**Expected:** A green run of the tool's own module tests covers a one-line description edit.

**Got:** The description ("…The full guide is always returned inline, never buffered to a @ref.") was 329 chars, over the 300-char cap enforced by `server::tests::tool_descriptions_stay_under_budget` (`src/server.rs:1598`). That gate lives in `server::tests`, not `tools::guide::`/`tools::core::`, so the targeted filters never ran it. The over-budget description shipped in `c799e887`; the regression surfaced only when the *onboarding* task later ran the full `cargo test --lib` suite.

**Probable cause:** A tool field's validating gate often lives in a different module (`server::tests`) than the code it guards; a per-module `cargo test --lib <module>` filter silently skips it.

**Workaround:** Trimmed to "Full guide returned inline." (commit `31a655e5`); full suite green.

**Severity:** med — shipped to experiments but caught before any master cherry-pick; no user impact, one extra fix commit. Good that the get_guide work hadn't been cherry-picked to master yet.

**Status:** fixed-verified — `31a655e5` on experiments; full lib suite (2690) passes.

**Fix idea / Pointer:** After editing any tool's `description()`/`input_schema()`, run `cargo test --lib server::` (or the full suite), not just the tool's module. Cross-cutting lesson → R-28 in `reconnaissance-patterns.md`.

---

## W-15 — Pre-edit recon of a prompt surface's full gate set before touching source.md

**Observed:** 2026-06-12, implementing the onboarding system-prompt root-file fix across `source.md` + 3 prompt files + `builders.rs`.

**Pattern:** Before editing a prompt surface, enumerate ALL its gates — not just the obvious cap. Scouted: the `@surface` markers (which surface drives the stored prompt → version bump), `extracts_onboarding_prompt_byte_for_byte`, the `prompt_surfaces_onboarding_snapshot` fixture, `source_md_under_cap` (2200-byte server_instructions cap), the `assert_eq!(ONBOARDING_VERSION, 28)` version-pin, `memory_templates_have_all_workspace_scope_sections` (heading-presence), and `workspace_prompt_requires_six_memories_per_project` (the `"6 memories"` string).

**Counterfactual:** Editing blind would have surfaced these as sequential red-test surprises: the `ONBOARDING_VERSION == 28` assert fails on the 28→29 bump (debug round-trip to locate a test in a different module); the "6 workspace-scope memories"→"5" Phase 5 edit *looks* like it might trip `workspace_prompt_requires_six_memories_per_project` until you confirm that test keys on Phase 3's `"6 memories"` string — without the scout you'd either avoid a correct edit or break the test. Pre-scouting bundled all gate updates into one commit; only the snapshot needed a (legitimate) re-bless.

**Confirming data points:**
1. `ONBOARDING_VERSION == 28` pin found pre-edit and updated in the same commit (no red surprise).
2. `"6 memories"` test scope confirmed Phase-3-only → Phase 5 "6→5" edit made safely.
3. Bug-file mechanism (all 4 claims) verified against code before implementing (applies R-27).

**Impact:** med — converted ~3 would-be iterative test failures into one planned edit set.

**Promote-when:** A 2nd prompt-surface session citing the full-gate-set scout → promote a "prompt-surface gate checklist" to CLAUDE.md § Prompt Surface Consistency (partial overlap with R-1/R-7 already).

**Status:** validated.

---
## F-21 — Off-the-cuff root-cause unification of the open-bug clusters over-generalized; code scout split it

**Observed:** 2026-06-14, during an "existing issues" review. After listing the 6 open bugs, I wrote an Insight asserting *"both clusters (catalog integrity + path-pinning) trace to one root theme: the catalog assumes one canonical filesystem identity per project,"* and offered to *"look at whether the three 2026-06-13 catalog bugs share a single fix."*

**When:** Read-only triage report, no edit pending — but the claim was a checkable root-cause assertion presented as analysis (R-19 territory: a recommendation, not Q&A). Reconnaissance invoked before the claim hardened into a scoping decision.

**Expected (my insight):** The three 2026-06-13 catalog bugs share a single root cause / single fix; the 2026-06-11 path-pinning cluster traces to the same "one canonical filesystem identity" theme.

**Got (scouted reality):**
- `32b58e13` (worktree): root cause is `index_repo_sync`'s `WalkBuilder::new(abs_root).standard_filters(true)` + root-anchored `.gitignore` + no linked-worktree detection (`src/librarian/indexer.rs:56`).
- `7ca71bf7` (repo rename): root cause is path-derived identity `artifact_id_from_abs` = `sha256(forward-slash abs_path)` (`src/librarian/ids.rs:17`) with no rename migration.
- `3ea49090` (orphan wipe): root cause is `reindex::call` building `active` from `ctx.workspace.roots` over a *global* catalog, then `delete_orphan_repos` deleting any row not under those roots — empty-active = `DELETE FROM artifact` (`src/librarian/catalog/artifact.rs:96`), and the test `delete_orphan_repos_empty_active_wipes_all` (`:259`) encodes the wipe *as intended behavior*.
- → **No single fix.** Three files, three mechanisms. Bugs **2+3** share a *narrower* genuine root: one machine-global DB keyed by absolute path, but reindex/orphan ops reason about it as per-workspace. Bug **1** shares only the *consequence* (duplicate rows), not the mechanism. The **2026-06-11 cluster** (artifact-move path-join, kotlin/RocksDB LSP, LSP workspace-pin) does not touch the catalog-identity model at all.

**Probable cause:** Pattern-matched the bug *titles* (all "catalog"/"path"-flavored, two adjacent dates) into one root cause without reading any implicated code this session. Adjacency + shared vocabulary read as shared mechanism — the "sounds right" trap.

**Correction:** Re-scoped to: 2+3 = one root theme (global-abspath-keyed catalog reasoned-about-as-per-workspace) needing two fixes; bug 1 = sibling consequence, separate fix (linked-worktree detection in the indexer); 06-11 cluster = unrelated (request/LSP path resolution).

**Severity:** low — framed as a hypothesis to investigate, caught before any planning. Counterfactual: had I acted on "single fix," I'd have scoped one fix and discovered mid-plan it needs ≥3, churning the plan and possibly mis-merging unrelated concerns.

**Status:** fixed-verified — corrected decomposition established from code this session, before any fix was scoped.

**Fix idea / Pointer:** R-32 (reconnaissance-patterns). Bugs `32b58e13` / `7ca71bf7` / `3ea49090`. Code: `src/librarian/indexer.rs:56`, `src/librarian/ids.rs:17`, `src/librarian/catalog/artifact.rs:96`, `src/librarian/tools/reindex.rs:148`.

## F-22 — Bug-file's "offset/limit never parsed" claim missed `OutputGuard::from_input`; telemetry then resolved the fix to dispatch-layer option (a)

**Observed:** 2026-06-14, pre-fix reconnaissance for `docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`. Scouting the seam before implementing the documented fix.

**When:** Verifying the bug file's Defect 1 / E-1 evidence ("`offset` and `limit` are not schema parameters and are never parsed") in order to choose between Fix A option (a) (schema + line-semantics mapping) and option (b) (path-local `RecoverableError`).

**Expected (bug file):** `offset`/`limit` are never parsed anywhere; the E-1/E-2 grep audit covered `read_file.rs` + `output_buffer.rs` and found no parse call, so the params are pure phantoms.

**Got (scouted reality):** `OutputGuard::from_input` at `src/tools/output.rs:96-97` DOES parse both — `optional_u64_param(input, "offset")` and `…"limit"`. `read_full_file` (the real-file path) builds the guard via `OutputGuard::from_input(input)`, so on real files `limit` caps the exploring-mode line window (via `guard.max_results`) while `offset` is parsed-then-unused (the overflow branch does `extract_lines(text, 1, max_lines)` — always from line 1). The buffer path (`read_from_buffer`) never builds a guard and only reads `start_line`/`end_line`, so there the params are genuinely dead — but the bug's *generalized* "never parsed anywhere" is false. E-1/E-2 scoped the grep to two files and missed `src/tools/output.rs`. The schema claim itself (L29-43 declares only `start_line`/`end_line`) is correct; the live repro E-3 confirms the MCP client forwards the non-schema params into `input` anyway, which is exactly why `OutputGuard` sees them on the real-file path.

**Probable cause:** R-3 — evidence grep scoped to the file(s) being modified, not the workspace root; the cross-module guard parser one call-hop away slipped.

**Workaround / corrected plan:** (1) Ship **Fix B** (hint strings → `start_line`/`end_line`) as-is — still correct. (2) Prefer Fix A **option (b)** (path-local `RecoverableError` inside `read_from_buffer`) over option (a): adding `offset`/`limit` to `read_file`'s `input_schema` would collide with `OutputGuard`'s existing page-offset/page-size semantics on the real-file path (same names, two meanings). (3) Word the option-(b) error path-locally ("on buffer reads, use start_line/end_line"), NOT "offset/limit are invalid" — they are valid for guard-paginated tools.

**Severity:** med — at face value the bug file would have steered the implementer to either a schema change that silently re-interprets real-file `offset`/`limit`, or a globally-worded error that contradicts `OutputGuard`'s documented Focused-mode pagination. Caught + corrected pre-implementation by the controller.

**Status:** fixed-verified — telemetry resolved the option choice (below); fix implemented + verified on `experiments` 2026-06-14.

**Resolution (telemetry, 2026-06-14):** usage.db across the 4 projects showed **191** offset/limit calls, **100% native-Read line-nav intent** (30 buffer silent-`success` + 161 real-file). This *inverted* the tentative option-(b) lean above: making offset/limit WORK as line aliases serves all 191; rejecting serves only 30 and leaves the 161 real-file cases. Implemented as dispatch-layer normalization in `ReadFile::call` (`normalize_line_nav_aliases` before the buffer fork) — covers both paths and structurally avoids the OutputGuard collision (range params route to `read_with_line_range`, never `read_full_file`'s guard). `cargo fmt`/`clippy`/`test` green (2733 lib tests); bug file updated; prompt surfaces reviewed (no change, no ONBOARDING_VERSION bump). Side-finding: `pika_observations` had 0 entries for this 30-occurrence bug because it keys on errors not silent-`success` — follow-up task filed.

**Fix idea / Pointer:** `docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`; recon R-31.

---
## W-16 — Recon caught title-derived triage imprecisions before they became a fix-priority recommendation

**Observed:** 2026-06-14, "check remaining open issues" review. After `artifact(find, kind=bug, status=open)` returned 6 bugs, I wrote a triage from the **titles alone** — clustered them (catalog ×4 / LSP / workspace-pin), ranked `3ea49090` "highest blast-radius," labeled three as "data-loss/data-corruption class," and called the set 6 flat `open` bugs — then offered to act on that ranking. Reconnaissance invoked before the ranking hardened into a fix-priority decision.

**Pattern:** Before standing behind a severity/priority triage of a bug *set*, read each bug's actual body (`artifact(get, id=…, full=true)` — never `read_markdown` on `docs/issues/`; the catalog holds status/tags), not just the `find` titles. Title vocabulary ("catalog", "orphan", "wipe") under-specifies severity class and hides partial-fix state.

**Counterfactual:** The scout confirmed the headline (`3ea49090` IS the top risk — empty-active runs `DELETE FROM artifact`, wiping the whole global catalog across workspaces, with a test *encoding the wipe as intended*), but corrected two checkable facts I'd already committed:
1. `7ca71bf7` is catalog **staleness** (dead/orphan rows after rename; tags `librarian/catalog/doctor`, NOT `data-loss`) — its data-loss risk exists only *via* the unsafe `3ea49090` prune path. I'd mislabeled it "data-loss class."
2. `3fc22ad2` (Kotlin mux/RocksDB) is **partially fixed** — its body has a "lock-probe … implemented 2026-06-11" subsection; not the flat-`open` I implied.
Had I acted on the uncorrected triage, a "fix the 3 data-loss bugs first" plan would have over-prioritized a staleness bug and re-done work already shipped for `3fc22ad2`.

**Confirming data points:**
1. F-21 (same day, this work stream) — a controller's title-derived *root-cause unification* of the same 6 bugs over-generalized; scout split it. **This is the 2nd same-day recurrence of the title-pattern-match trap** — the friction is intra-day recurrent, not a one-off.
2. R-19 (reconnaissance-patterns) — "assert a checkable fact only after reading it this session"; R-32 — the open-bug-cluster decomposition. Both held again here.

**Impact:** med — prevented two wrong facts from entering a fix-priority recommendation; no code/plan churn because the scout preceded any scoping.

**Promote-when:** if a 3rd title-derived cluster-triage slip lands, escalate the R-32/R-19 promote-when (the recurrence count, not the individual catches, is the case for a standing "scout bug bodies before triaging a set" rule).

**Status:** validated — drift caught + corrected before any fix-priority decision.

**Fix idea / Pointer:** This session's open-issues triage. Bugs scouted: `3ea49090`, `32b58e13`, `7ca71bf7`, `1a5acfc0`, `3fc22ad2`, `3fb29bc6`. Recon kin: F-21, F-22, R-19, R-32.

## F-23 — `edit_code` "AST parse failed" hint named two wrong causes; real cause was AST/LSP start-line gap

**Observed:** 2026-06-16, debugging a live `edit_code(insert, position="after")`
refusal on `backend-kotlin/.../RoomConstraintsTest.kt`.

**When:** Reproducing the user's failing insert before proposing a fix.

**Expected (error hint):** `"cannot determine end of '…' for insert-after — AST
parse failed"` with hint *"The file likely has syntax errors that broke
tree-sitter's parse, or the symbol has duplicate-name siblings without a clear
name_path."* — i.e. two suggested causes: broken parse, or ambiguous siblings.

**Got (scouted reality):** Neither. `symbols()` listed all 14 methods (clean
parse); the method had a unique `name_path` (no duplicate siblings). A throwaway
unit test dumping the *real* file's AST showed the method at AST `start_line=214`
(the `@Test` annotation line — tree-sitter's function node spans its annotations)
while kotlin-lsp reports `216` (the `fun` keyword line). The 2-line gap = annotation
count, and `collect_ast_candidates`' `abs_diff(start, lsp_start) <= 1` gate dropped
the only candidate → `find_ast_end_line_in` returned `None` *before* the
backtick-tolerant `name_path` matcher ran.

**Probable cause:** The refuse path (`editing_end_line_strict` → `ast_confirmed_end_line`
→ `find_ast_end_line_in`, `src/symbol/query.rs`) collapses all `None` returns into one
generic message. Three distinct conditions (broken parse, ambiguous siblings,
coordinate-system start-line gap) share the message; the third — the actual cause —
isn't even named in the hint.

**Workaround (user's, pre-fix):** created a separate test file
(`RoomConflictSpanOverlapTest.kt`) rather than inserting the sibling.

**Severity:** high — masked the real defect behind a hint pointing at non-existent
syntax errors; a reader trusting the hint would "fix" syntax that isn't broken or
re-do backtick normalization (already shipped 2026-05-29), never touching the line gate.

**Status:** fixed-verified — root cause fixed in `find_ast_end_line_in`
(`name_path`-first, no line gate); regression test
`find_ast_end_line_in_bridges_annotation_line_gap`; full bug file at
`docs/issues/archive/2026-06-16-kotlin-edit-code-annotation-line-gap.md`.

**Fix idea / Pointer:** `src/symbol/query.rs` `find_ast_end_line_in` / `collect_by_name`,
this session. Follow-up candidate: differentiate the three `None` causes in the
refuse message so the hint stops naming wrong causes.

## W-17 — Dump the actual matcher inputs on the real file; don't trust the tool's own diagnostic or the `symbols()` listing

**Observed:** 2026-06-16, root-causing the F-23 `edit_code` refusal.

**Pattern:** When an edit/navigation tool refuses with a diagnostic that names a
cause, and a higher-level read (`symbols()`) shows the file is *fine*, the two views
are using different coordinate systems. Resolve it by reproducing the failing
internal call on the real substrate: write a throwaway unit test that runs the exact
extractor + matcher (`extract_symbols_from_source` → `find_ast_end_line_in`) against
the real file via `include_str!`, and print the actual `name` / `name_path` /
`start_line` / `end_line` for the symbol — then replay the matcher at the candidate
line values to find the exact boundary where it flips `Some`→`None`.

**Counterfactual:** Without the dump I had two plausible-and-wrong hypotheses, both
endorsed by visible evidence: (1) the error hint said "syntax errors" — but `symbols()`
proved the parse was clean; (2) the related archived bug (2026-05-29) said "Kotlin
backtick name mismatch" — but that fix had already shipped and was present in the code.
Acting on either would have produced a no-op or wrong fix. The dump showed AST
`start=214` vs LSP `216` and `find_ast_end_line_in(215)->Some, (216)->None`, isolating
the ±1 line gate in one run — cause pinned, not inferred. Estimated cost avoided:
≥1 wrong fix cycle (edit + build + re-test + re-reproduce ≈ 4 round-trips) plus the
risk of shipping a tolerance-widening band-aid that mis-matches siblings.

**Confirming data points:**
1. F-23 (this session) — AST/LSP start-line gap, found by dumping real-file extractor
   output; both the error hint and the prior-bug analogy pointed elsewhere.
2. W-14 (this log) — verify a subagent's mechanism claim against the actual fn body.
3. W-1 (this log) — scout helper-fn bodies before "fixing" reported bugs.

**Impact:** med→high — converts a guess-and-check debug loop into a single
ground-truth measurement; scales with how misleading the tool's own diagnostic is.

**Promote-when:** A third instance where a tool's diagnostic text (not just a plan or
a subagent report) misdirects and an empirical dump of the internal call corrects it.
At that point promote to codescout memory `reconnaissance` as: *"When a tool's error
text names a cause but a higher-level read contradicts it, reproduce the failing
internal call on the real file and print its inputs before fixing (R-35, W-17)."*

**Status:** validated — single strong datapoint, cause confirmed by the passing
regression test built from the dumped values.

## F-24 — Kotlin `-Xmx2g` fix documented "implemented on experiments" but is an uncommitted working-tree change on no branch

**Observed:** 2026-06-20, pre-recon for the proposed "drive the Kotlin heap fix
to ship" follow-on, after a read-only session summarizing the headroom/llm-proxy work.

**When:** Verifying the git-state assertions I'd made in a summary (llm-proxy
`2906647` on master; Kotlin Fix 1 "done on experiments, not yet cherry-picked")
against the substrate — both originally asserted from docs, not from `git`.

**Expected (bug doc + my summary):** `docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`
§Fix/§Resume state *"Fix 1 is implemented on `experiments` (release binary rebuilt
via `cargo rb`, symlink intact)"* — i.e. a committed change on the `experiments` branch.

**Got (scouted reality):** The `-Xmx2g` edit is an **uncommitted working-tree change**.
`git -C codescout status --short src/lsp/servers/mod.rs` → ` M` (modified, unstaged);
`git log --all -S '-Xmx2g' -- src/lsp/servers/mod.rs` → **empty** (present on no
branch, not even `experiments`). The working tree IS coherent and compiles — the diff
adds both the two-arm `-Xmx2g` append and a regression test `kotlin_caps_jvm_heap`
whose helper `kotlin_java_tool_options` already exists at `mod.rs:367-373` — but none
of it is committed. `git stash list` empty; reflog tip clean (no concurrent HEAD moves).
By contrast the llm-proxy assertion checked out clean: `git branch --contains 2906647`
→ `* master`.

**Probable cause:** Bug-doc §Fix was written optimistically at edit-time ("rebuilt via
`cargo rb`" describes the *binary* state, conflated with *commit* state). `cargo rb`
compiles the working tree — it does not require a commit — so a rebuilt+symlinked live
binary can coexist with an uncommitted source, making "implemented on experiments" feel
true while `git log` disagrees. Classic R-19: a checkable git-state fact ("it's on
experiments") asserted from doc prose, not re-derived from `git` this session.

**Workaround:** None needed for correctness (tree compiles). Risk mitigation: commit the
edit to `experiments` before any branch-switching follow-on. On a shared `experiments`
branch with concurrent sessions (CLAUDE.md Concurrent-Work Rules), an uncommitted
working tree is the most loss-prone state — a sibling `git checkout`/`reset`/`stash` in
another session silently discards it, and there is no commit to recover from.

**Severity:** med — no cascade yet and the tree compiles, but the fix is one stray
working-tree mutation away from silent loss, and "not yet cherry-picked to master"
(true) masks the sharper "not yet committed to *any* branch." Controller-absorbable by
one `git commit`.

**Status:** fixed-verified (commit dimension) — resolved 2026-06-21. A concurrent
session committed the edit as **`3adb66e7` `fix(lsp): cap kotlin-lsp JVM heap with
-Xmx2g`** on `experiments` (verified: includes both the two-arm `-Xmx2g` append and the
`kotlin_caps_jvm_heap` regression test; `git branch --contains 3adb66e7` → `experiments`
only). At recon time HEAD was the older tip `d11b4bd1` (a clean ancestor of the fix
commit — verified linear, not divergent), so the working tree was genuinely
pre-commit; the substrate moved forward right after the scout. The benign branch of the
loss-risk this entry named. **Residual:** not yet cherry-picked to `master` and not yet
live-verified via `/mcp` — those remain open in the bug file's §Resume.

**Fix idea / Pointer:** Commit `src/lsp/servers/mod.rs` (the `-Xmx2g` append + the
`kotlin_caps_jvm_heap` test) to `experiments`, then update the bug file §Fix/§Resume to
cite the real commit SHA instead of the branch-name claim. Bug file:
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`. Substrate scouted this session.

## F-25 — Kotlin heap bug's root cause ("no `-Xmx` anywhere → 25% RAM default") missed the kotlin-lsp distribution's own `intellij-server.vmoptions -Xmx2048m` — but that cap is NOT reliably applied to codescout-spawned instances, so the explicit `-Xmx2g` fix IS load-bearing (self-corrected mid-session)

**Observed:** 2026-06-21, live-verifying the `-Xmx2g` fix (`3adb66e7`) after `cargo rb` + `/mcp`,
before cherry-picking to master.

**When:** Confirming the spawned `kotlin-lsp` JVM actually tops out at the cap. Found two live
JVMs, `jcmd`'d both, then read the bug doc's §Evidence to reconcile a contradiction.

**Expected (bug doc §Root cause):** *"No `-Xmx` is appended here or anywhere on the launch path …
the comment's cap is fictional."* I.e. nothing caps the heap, so it defaults to ~31 GiB.

**Got (scouted reality):** The kotlin-lsp distribution **does** ship a cap —
`/usr/share/kotlin/kotlin-lsp/bin/intellij-server.vmoptions` contains `-Xmx2048m` (= 2 GiB). So
"no `-Xmx` anywhere / the cap is fictional" is imprecise: there is one, in the distribution's
launcher config (codescout's `src/` just doesn't set it). **However**, that vmoptions cap is
**not reliably applied** to codescout-spawned instances: the bug doc's own §Evidence shows two
instances reaching a ~35.7 GiB RSS with a **20+ GiB GC sawtooth** (23→27→23 GB) on 2026-06-20 —
and a multi-GB GC sawtooth can only be reclaimable *Java heap* (native RocksDB/JNI memory is not
GC-reclaimed), so those JVMs genuinely ran a ~31 GiB heap *despite* the `-Xmx2048m` vmoptions
being installed since Jun 12. Likely codescout's `--system-path` / `-Duser.home` redirect alters
how the JetBrains launcher resolves its vmoptions file.

**Probable cause (of the doc imprecision):** The investigation grepped only codescout's own
launch path (`src/lsp/servers/mod.rs`) and concluded "no `-Xmx` anywhere," never inspecting the
kotlin-lsp distribution's launcher/vmoptions — the other half of the launch surface (R-3:
grep the whole launch surface, not just our file). It then under-specified *why* the heap was
still uncapped in practice (the vmoptions exists but doesn't take).

**Consequences for the fix — CORRECTED (this entry first concluded "redundant"; that was wrong):**
The explicit `-Xmx2g` goes into `JAVA_TOOL_OPTIONS`, which the JVM **always** honors regardless
of launcher/vmoptions quirks. Since the distribution's vmoptions cap is demonstrably *not*
reliable for our instances (the 31 GiB heap happened with it present), our flag is **load-bearing**,
not defense-in-depth — it is the cap that actually guarantees heap ≤ 2 GiB. Live-confirmed: the
codescout-repo JVM (PID 4100626, our flag present) `jcmd MaxHeapSize=2147483648`. My initial
"redundant" read came from one no-flag JVM (PID 4041946, a *different* — backend-kotlin —
workspace) sitting at 2 GiB; that was the case where vmoptions *did* apply, and I over-generalized
from it before weighing the GC-sawtooth heap evidence. Self-correction logged.

**Secondary point (still valid):** `-Xmx` bounds only the Java heap. The capped instance's RSS
(4.16 GiB) = 2 GiB heap + ~2 GiB native (RocksDB JNI / direct buffers / metaspace / code cache),
which `-Xmx` does not bound. The separate native-growth host-OOM hazard is addressed by making
`watch_memory` *actuate* (the deferred Fix 2), not by `-Xmx`. (The observed 68 GiB OOM was
`task=codescout` — our Rust process — unrelated to either.)

**Workaround / correction needed:** Correct the bug doc §Root cause to: (a) the distribution
*does* ship `-Xmx2048m` in `intellij-server.vmoptions`, so "no `-Xmx` anywhere" is imprecise;
(b) but it is not reliably applied to codescout-spawned instances (GC-sawtooth → 31 GiB heap
proof), so an explicit JVM-honored `-Xmx` in `JAVA_TOOL_OPTIONS` is the *reliable* cap →
Fix 1 is load-bearing and correct; (c) native (off-heap) growth remains, addressed by
`watch_memory` actuation (Fix 2).

**Severity:** med — the shipped code is correct and load-bearing (decision to ship reinforced),
so the cost was a near-miss wrong conclusion (caught before any doc rewrite shipped), not broken
code. Logged as a self-correction exemplar.

**Status:** open — bug-doc §Root cause precision-correction pending; the open *why* (which
launcher mechanism drops the vmoptions cap) is worth a one-line note but not blocking. Verified
this session via `jcmd` on both live JVMs, the vmoptions file, and the bug doc's GC-sawtooth
evidence.

**Fix idea / Pointer:** `/usr/share/kotlin/kotlin-lsp/bin/intellij-server.vmoptions` (`-Xmx2048m`);
`jcmd <pid> VM.flags`; bug doc §Evidence "Live growth curve" (GC sawtooth); bug file
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md` §Root cause + §Fix.
## F-26 — Full `cargo test` gate on `experiments` has a deterministic, fix-unrelated failure: `replace_symbol_surfaces_stale_error_after_max_retries` — ROOT-CAUSED: not environmental (hermetic MockLspClient); `758801d5` removed the AST-collection line-gate, so `validate_symbol_range` pre-empts the staleness path. Experiments-only; master green; write-refusal safety intact

**Observed:** 2026-06-21, full `cargo test` gate before cherry-picking the Kotlin `-Xmx2g`
fix (`3adb66e7`) to master. Root-caused 2026-06-23.

**When:** Pre-cherry-pick gate (CLAUDE.md: "cherry-pick to master only after tests + clippy
+ MCP verify").

**Expected (gate):** All tests green on `experiments` HEAD.

**Got:** 1 failure (reproduced in isolation 2×): `tests/symbol_lsp.rs:856
replace_symbol_surfaces_stale_error_after_max_retries` asserts the error "must still mention
staleness", but got `"LSP returned suspicious range for 'target' (lines 1-1, but AST shows it
spans to line 5) …"`.

**Root cause (CONFIRMED — supersedes the initial "environmental / live-kotlin-lsp" guess):**
The test is **hermetic** — `ctx_with_mock` + `MockLspClient` returns a deliberately stale range
`(0,0,0,0)` for `target` (real `target` is at lines 3–5). No live kotlin-lsp involved, so the
failure is pure deterministic code logic, not cold-start/contention. In
`fetch_validated_symbol`'s retry loop the order is `validate_symbol_range` (suspicious-range
guard) **then** `validate_symbol_position` (staleness). `validate_symbol_range` calls
`find_ast_end_line_in`, which commit **`758801d5`** (the F-23 Kotlin annotation-gap fix) changed
from line-gated collection (`collect_ast_candidates(…, lsp_start, …)`) to ungated
`collect_by_name` + `name_path`-first resolution. So for the stale `(0,0,0,0)` symbol it now
resolves the real AST end (line 5) despite the 2-line start gap → `ast_end(4) > sym.end_line(0)`
→ "suspicious range" bail on attempt 0 → becomes the terminal error after retries, never
reaching the staleness path. **Verified master is unaffected:** master's `find_ast_end_line_in`
still calls `collect_ast_candidates(…, lsp_start, …)` (line-gated) → returns `None` for this
input → `validate_symbol_range` passes → staleness path fires → test passes on master. The only
symbol-edit-path diff `master..experiments` is `758801d5` (`query.rs`, +118/−43); the test file
is byte-identical on both.

**Severity:** med → reclassified **low-as-correctness, med-as-gate**: BUG-041's *safety property*
(refuse the write rather than use stale offsets) is **preserved** — the tool still returns a
`RecoverableError`; only the message changed, and only for *persistent* staleness (transient
staleness still refreshes on retry and passes). So this is a stale, over-specific test assertion
exposed by `758801d5`'s widened AST resolution, not a corruption regression. It does block a
strict "all-green" gate on `experiments`.

**Impact on the cherry-pick:** NONE — master is green for this test, and the Kotlin fix
(`3adb66e7` + doc `21eaa696`) touches `mod.rs`/docs, not `query.rs`/`symbol_lsp.rs`. The
cherry-pick cannot change master's status here. F-26 is an experiments-only follow-up.

**Fix options (Phase 4 — not yet implemented; needs a decision):**
(a) **Reorder** `fetch_validated_symbol` to run `validate_symbol_position` (staleness) before
`validate_symbol_range` (suspicious-range) — makes the position-mismatch diagnosis win for stale
inputs, restoring "stale"; suspicious-range still fires for correctly-positioned-but-truncated
ranges (doesn't break F-23, whose Kotlin range is correctly positioned). Minimal + arguably more
correct. **Recommended.**
(b) Loosen the test to assert the safety property (any `RecoverableError` that refuses the write)
rather than the exact "stale" string.
(c) Make `validate_symbol_range` skip when the LSP range looks stale-degenerate (hard to
distinguish from truncation; least preferred).

**Status:** fixed-verified — fixed on `experiments` by `d079c054` `fix(symbol): order staleness
check before suspicious-range guard` (reorder, option (a)). The RED test
(`replace_symbol_surfaces_stale_error_after_max_retries`) now passes; full gate green
(`cargo fmt` clean, `cargo clippy --all-targets -D warnings` exit 0, `cargo test` 2797 lib +
all integration binaries, 0 failed). Experiments-only regression that never reached master, so
no master ship/archive step needed. Cross-ref F-23 (introduced it via `758801d5`), F-18 (the
live-LSP class I initially misattributed to).

**Fix idea / Pointer:** `src/symbol/query.rs` — `fetch_validated_symbol` (retry loop ordering),
`validate_symbol_range`, `find_ast_end_line_in` (line-gate removal in `758801d5`);
`tests/symbol_lsp.rs:824` the test. This session.
## W-18 — Pre-implementation recon confirmed a parser-grammar fix plan is safe + named the regression tests a naive fix would break

**Observed:** 2026-07-01, right after filing `docs/issues/archive/2026-07-01-read-file-jsonpath-dotted-object-keys-unreachable.md` (read_file json_path can't reach dotted object keys), before touching its Fix plan. Seam: `parse_json_path_segments` / `parse_bracket` in `src/tools/file_summary/file_summary.rs`.

**Pattern:** Before implementing a filed bug's Fix plan that changes a *parser's accepted grammar*, scout three things beyond the target function body: (a) callers via `references`, (b) the data model it feeds (`Segment` enum + the `resolve_json_segment` apply arm), and (c) every existing test that pins the *current* rejection behavior. Enumerate the old-grammar tests first so the fix is written to keep each green.

**Counterfactual:** The plan adds quoted-bracket-key support to `parse_bracket`. A naive impl that accepts any bracket inner as a key flips `$.a[abc]` from error→`Key("abc")`, breaking `parse_rejects_non_integer_bracket` (`tests.rs:731-733`). A blunt char-split tokenizer rewrite breaks `parse_chained_negative_after_positive` (`$.a[0][-1]`, `tests.rs:681-683`). Both surface only at `cargo test` — ≥2 red tests, each an implement→test→re-read cycle; dispatched to a subagent, the drift lives in two contexts. The scout also confirmed `Segment::Key` already applies via `obj.get(k)` (`resolve_json_segment:585`), so a dotted key needs **no** new enum variant — pre-empting an over-engineered `Segment::QuotedKey` addition.

**Confirming data points:**
1. `references(parse_json_path_segments)` → 1 internal caller (`extract_json_path:460`) + 13 tests; `parse_bracket` called only from within it. Contained blast radius, no cross-crate threading.
2. `parse_rejects_non_integer_bracket` (`tests.rs:731`) pins `[abc]`→error — the invariant the plan's "matching-quote strip only" rule preserves.
3. `parse_chained_negative_after_positive` (`tests.rs:683`) pins `$.a[0][-1]` — the chained-bracket handling the tokenizer rewrite must preserve ("split on `.` outside brackets" does).
4. `Segment::Key` apply arm (`resolve_json_segment:585`) is a plain `obj.get(k)` — a dotted key string works with the existing variant.

**Impact:** med — saves ≥2 failed-test round-trips and prevents an over-engineered enum variant; would be high if the fix were dispatched to a subagent (drift across two contexts).

**Promote-when:** A second pre-implementation recon on a parser/grammar-change bug catches a regression-test invariant the plan would have broken. Adjacent cluster already large (W-2, W-4, W-9, W-15, W-16 are all "pre-action recon validates a plan/bug claim against reality/tests") — flag for consolidation into one CLAUDE.md rule: *"Before implementing a grammar/parser-change fix, enumerate the tests pinning the current grammar and write the fix to keep each green."*

**Status:** validated

---
## W-19 — Pre-fix recon on context() starvation bug found a locked-in "always include first" test — scoped the fix to anchor mode only

**Observed:** 2026-07-05, resuming `docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md` (residual #2 from the tracker cross-linking arc). Seam: the packing loop in `src/librarian/tools/context.rs::call` (the section after `candidate_ids`/`sorted_ids` construction).

**Pattern:** Before fixing a "budget exhausted by the first item" bug in a shared packing/inclusion loop, grep the loop's own test module for existing tests whose *assertion* depends on the current behavior — not just tests that exercise the same function. A loop shared by multiple call modes (anchor / topic / goal-tracker) may have a test encoding the "buggy-looking" behavior as intentional for a *different* mode.

**Counterfactual:** The bug file's own hypothesis was "packer includes the anchor first, un-trimmed, exhausting max_tokens" — confirmed, but the obvious naive fix (remove the `!markdown.is_empty()` short-circuit so oversized first-items get rejected/trimmed unconditionally) would have flipped `max_tokens_caps_inclusion`'s `assert_eq!(ids.len(), 1, "max_tokens should cap inclusion to 1 artifact")` red, because that test's own comment says the guard's behavior is deliberate: "first artifact is always included (budget check only triggers on subsequent artifacts)." Reading the test before writing the fix showed the guarantee-≥1-result contract must survive for topic-search/goal-tracker modes; only anchor-mode needed a reserved-budget carve-out.

**Confirming data points:**
1. `tests/max_tokens_caps_inclusion` (`context.rs:507-543`) — comment + assertion pin "first artifact always included" for the `topic` search path at `max_tokens=15`.
2. The packing loop's guard `if !markdown.is_empty() && (markdown.len() + section.len()) > char_cap { break; }` — the `!markdown.is_empty()` clause is what exempts item #1 from the size check, and is shared by all three candidate-gathering branches (anchor / topic / goal-tracker), not anchor-specific.
3. Bug file's own root-cause section: "Hypothesis (unverified in detail)... Needs a read of the packing loop below `candidate_ids`" — confirmed the exact line, but the test-coupling wasn't discoverable from the bug file alone.

**Impact:** med — a naive fix would have shipped green on its own new anchor-mode test while failing an existing, orthogonal test in the same file; caught pre-implementation instead of at `cargo test`.

**Promote-when:** 7th datapoint in the "pre-action recon validates a plan/bug claim against reality/tests" cluster (W-2, W-4, W-9, W-15, W-16, W-18, now W-19). Per W-18's own note, this cluster is large enough to consolidate into one CLAUDE.md rule now rather than accumulate further — flagging for the user rather than unilaterally editing CLAUDE.md.

**Status:** validated

---

## W-20 — Didn't write off a surprising self-cite failure as noise — bisected it to a real link_scan parsing bug

**Observed:** 2026-07-05, triaging `link_scan`'s 366 dangling + 232 ambiguous findings (residual #2 from the tracker cross-linking arc). First hypothesis (from the F-N/W-N-is-locally-scoped convention) was that the whole bucket was expected noise. One finding broke that story: `bug-fix-session-log.md` (`2dd9d90bc83f9f49`) cited its own freshly-added `W-19` token as dangling — from the SAME file that defines `## W-19 — …` as a heading. Self-cite should have suppressed this.

**Pattern:** When a triage/report task produces a result that contradicts a fact you can directly verify ("this token IS defined right here, in this file"), don't fold it into the "expected noise" bucket by pattern-matching on the majority shape. Isolate the anomaly with a minimal, controlled repro using the actual extraction function directly (`extract(text)`), not just synthetic guesses — and when a first synthetic repro passes but the real file fails, bisect the REAL file's content (slice from a known-good anchor point, shrink toward the failure) rather than trust that "the code looks right on read."

**Counterfactual:** Reading `extract()`'s and `resolve()`'s source suggested the self-cite logic was correct — twice, code-reading alone said "this should work." Only empirical bisection (dumping raw pulldown_cmark events, then `Tag::MetadataBlock` Start/End spans) revealed `ENABLE_YAML_STYLE_METADATA_BLOCKS` pairs ANY two bare `---` lines as YAML metadata, not just leading frontmatter — and session-log trackers separate every entry with a bare `---`. Without the bisection, this would have shipped as "232 ambiguous / 366 dangling, mostly expected" in the ROADMAP/triage writeup, permanently under-counting the real link graph by the ~34 dangling + ~19 ambiguous findings this bug caused, and leaving 16 real edges undiscovered.

**Confirming data points:**
1. Isolated synthetic repro (heading immediately after a lone `---`, short body) extracted correctly — ruled out "any bare `---` before a heading breaks recognition."
2. Real-file tail slice from `## F-26` onward reproduced the failure; bisecting further down to just the 4 lines `**Status:** validated / (blank) / --- / ## W-19 …` still reproduced it.
3. Direct `Tag::MetadataBlock` event dump on the real file showed `METADATA START YamlStyle` at the `---` before `## W-19` and `METADATA END YamlStyle` at the NEXT entry's `---` — exact byte-range match to the swallowed span.
4. Fix (byte-offset frontmatter guard via `frontmatter::parse`, dropping the pulldown_cmark option) verified live: dangling 366→332, ambiguous 232→213, +16 edges added, 8 stale pruned, re-confirmed idempotent (438/438).

**Impact:** high — a plausible-sounding wrong story ("F-N/W-N reuse noise") would have shipped as the triage conclusion, permanently hiding a real extraction defect and 16+ real graph edges; caught only because one data point was checked against ground truth instead of pattern-matched into the majority bucket.

**Promote-when:** First datapoint of this specific pattern ("triage output contradicts a directly-verifiable fact — bisect, don't pattern-match"); track for a second occurrence before promoting to CLAUDE.md.

**Status:** validated

---

## F-27 — Prior-session summary claimed two bug files were logged; neither exists on disk or in git history

**Observed:** 2026-07-07, resuming the `symbols_overview_glob_marks_grammarless_language_warming_instead_of_dropping_file` Windows path-separator fix. User pasted a prior session's summary as the task brief.

**When:** Before starting the fix, searching for the bug file the prior summary said it had created, to read its root-cause section instead of re-deriving it from scratch.

**Expected:** Prior summary text: "I logged this as 2026-07-07-windows-glob-overview-path-separator-test-mismatch.md, along with the earlier memory-tool discrepancy I found: 2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md."

**Got:** `artifact(find, kind="bug")` returned 0 matches for either slug; `tree(glob="*2026-07-07*", path="docs/issues")` found 0 files; `git log --all --diff-filter=A -- 'docs/issues/2026-07-07*'` returned nothing across all branches/commits. Neither file was ever created or committed anywhere in this repo.

**Probable cause:** Unknown — the pasted summary is a self-report from a prior session/context that either ran against a different checkout, was interrupted before the file-creation call landed, or narrated the action without actually executing it.

**Workaround:** Independently re-derived root cause from the actual failing test (`src/tools/symbol/tests.rs:5008`) and the real source (`src/tools/symbol/list_overview.rs:268,295,310`) — matched the prior summary's diagnosis — then created `docs/issues/archive/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md` for real, status `fixed`. Did NOT fabricate the second (memory-tool) bug file — flagged it back to the user since there is no first-hand evidence to write from.

**Severity:** med — no wasted implementation effort (the claimed root cause was independently re-verified against source before being trusted), but taking "already logged" at face value would have left both bugs permanently untracked while everyone believed they were tracked.

**Status:** mitigated

**Fix idea / Pointer:** Treat "I already logged X" in any inherited session summary as a claim to verify (`artifact(find)` / `git log`), not a fact — same class as F-16/F-24 (inherited-claim misdiagnosis).

---
## F-28 — Inherited "stray lsp: field" claim in mv.rs was stale; ToolContext.lsp is now a legitimate field

**Observed:** 2026-07-07, applying an externally-supplied debugging finding for the doctor.rs `check_ads_colon` verbatim-prefix bug (see `docs/issues/archive/2026-07-07-doctor-ads-colon-verbatim-prefix-false-positive.md`).

**When:** Before touching `src/librarian/tools/mv.rs`, scouting the finding's "bonus find" claim that a stray `lsp:` field in `mv.rs`'s test `mk_ctx` referenced a field "that never existed on ToolContext" and had been removed.

**Expected:** Per the inherited report, `ToolContext` (`src/librarian/tools/mod.rs`) has no `lsp` field; the `lsp: crate::lsp::MockLspProvider::with_client(...)` line in `mv.rs`'s `mk_ctx` test helper is a leftover from an abandoned "thread lsp into librarian" attempt and a compile error.

**Got:** `ToolContext` at `src/librarian/tools/mod.rs:97-100` now has a legitimate `pub lsp: Arc<dyn crate::lsp::LspProvider>` field, deliberately threaded in at construction and documented with a comment citing `docs/issues/archive/2026-07-05-audit-doc-refs-lsp-stubbed-off.md`. `mv.rs:136`'s `lsp: ...` assignment is a correct, compiling use of that field, not a stray leftover — confirmed via `symbols(include_body=true)` and a full `cargo test --lib` (2954 passed, 0 failed).

**Probable cause:** The report describes an earlier state of the codebase, before `lsp` was legitimately added to librarian's `ToolContext` as part of the audit_doc_refs LSP-stubbed-off fix — same shape of drift as F-2/F-15/F-24 (inherited reports going stale as sibling work lands on the same branch).

**Workaround:** Skipped the `mv.rs` edit entirely.

**Severity:** low — no code was touched based on the stale claim; caught before any edit via routine reconnaissance rather than after a failed compile.

**Status:** wontfix-false-alarm

**Fix idea / Pointer:** N/A — no fix needed, the field is load-bearing.

---

## W-21 — Grep the raw tracker file for the true max F-N/W-N; `artifact(get)` silently truncates large trackers

**Observed:** 2026-07-07, allocating the next F-N/W-N IDs to file this session's findings into this tracker.

**Pattern:** Before allocating a new tracker ID, `grep(pattern="^## (F|W)-\\d+ —", path="docs/trackers/<file>.md")` against the raw file rather than trusting `artifact(get)`'s `preview.headings` or `get(full=true)`'s `body` field.

**Counterfactual:** `artifact(get, id="2dd9d90bc83f9f49", full=false)`'s `preview.headings` listed F-1..F-7 as the last entries (despite `preview.line_count: 1739` being correct), and `artifact(get, full=true)`'s `body` field silently truncated at 499 lines / ~46794 bytes — with no truncation flag in either response — landing mid-way through F-6/F-7, well short of the tracker's real F-27/W-20. Trusting either would have allocated new entries as "F-8"/"W-5", directly colliding with the 20 entries (F-8..F-27, W-5..W-20) already in the file. `librarian(action="reindex", scope="project")` (reported `updated: 10`) did not move the truncation point on a re-fetch, ruling out catalog staleness — filed as `docs/issues/archive/2026-07-07-artifact-get-full-body-silent-truncation.md`.

**Confirming data points:** 1) This session — the reconnaissance-skill invocation prompted a raw-file grep before append, which caught the true F-27/W-20 ceiling before any write landed. 2) No second datapoint yet — first observation of this failure mode.

**Impact:** high — an ID collision in a session-log tracker corrupts the Index/body correspondence that makes every prior F-N/W-N citation resolvable; the failure is silent (no error, just a plausible-looking duplicate ID) and would surface only when a future reader noticed two different bodies claiming the same ID.

**Promote-when:** A second large tracker (>500 lines / >~47KB body) exhibits the same `get`/preview truncation without a flag — at that point, escalate `docs/issues/archive/2026-07-07-artifact-get-full-body-silent-truncation.md`'s priority and promote this grep-first practice into `get_guide("tracker-conventions")`.

**Update 2026-07-10 (half fixed — practice still holds):** The counterfactual had two independent failures; one is now fixed. (1) `artifact(get, full=true)`'s *silent body truncation* is closed — the buffered response now surfaces a loud `"artifact body TRUNCATED — N of M lines are in $.body …"` summary (commit `97a36905`, `LibrarianAdapter::format_compact`); bug files `2026-07-07-artifact-get-full-body-silent-truncation.md` and `2026-07-09-artifact-get-full-true-body-silent-truncation.md` are both marked `fixed`. (2) `preview.headings` stopping early (F-7 here) while `preview.line_count` stays correct is **NOT** fixed and still reproduces (verified 2026-07-10 — every `get` on this tracker returned headings ending at F-7 with `line_count: 1841`). So grep-first remains the safe ID-allocation practice, now motivated by (2) alone. The Promote-when's body-path half can no longer recur; the `preview.headings` half stands as a separate, still-open defect.

**Status:** validated

---

## F-30 — Upstream commit bundled an unrelated stray argument that broke `cargo rb` twice in one pull cycle

**Observed:** 2026-07-07, during a `git fetch` + `git pull --rebase origin experiments` + `cargo rb` cycle to pick up 15-46 new upstream commits from a large forward-slash-normalization work stream.

**When:** Rebuilding the release binary immediately after rebasing local work onto `origin/experiments`.

**Expected:** A clean rebuild — the pulled commits' own stated purpose (forward-slash path normalization) should not touch unrelated call signatures.

**Got:** `cargo rb` failed with `E0061: this function takes 0 arguments but 1 argument was supplied` at `src/server.rs:153` — commit `62457959` ("fix(server): normalize post_process's root_prefix to forward-slash") bundled an unrelated third diff hunk changing `crate::librarian::try_build_runtime().await` to `crate::librarian::try_build_runtime(lsp.clone()).await`, even though `try_build_runtime`'s signature has taken zero arguments since its creation (`git log -p -S "pub async fn try_build_runtime"` shows exactly one match, the original definition). After fixing that and re-running `cargo test --lib`, a SECOND unrelated compile error surfaced in `src/librarian/tools/mv.rs`'s test fixture: a stray `lsp:` field on `librarian::tools::ToolContext`, a field that has never existed on that struct — same abandoned "thread lsp into librarian" attempt, different file.

**Probable cause:** Someone (or an agent) mid-session experimented with threading an `lsp` handle into librarian's runtime, abandoned it, and the revert was incomplete — two call/construction sites were left referencing the abandoned shape while the actual struct/fn definitions were never changed.

**Workaround:** Removed the stray `lsp.clone()` argument (`src/server.rs`) and the stray `lsp:` field (`src/librarian/tools/mv.rs`) to match the unchanged real signatures. Verified via `cargo test --lib` (2860 passed) before proceeding.

**Severity:** high — blocked the release build entirely; would have blocked ANY session rebuilding on `experiments` at that commit, not just this one.

**Status:** fixed-verified — `cargo rb` compiles clean at commit `0a1d8736` (server.rs fix) + part of `3149384b` (mv.rs fix); logged as `docs/issues/archive/2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md`.

**Fix idea / Pointer:** `docs/issues/archive/2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md`; before assuming a rebuild failure is local/environmental, `git log -p -S "<literal call text>"` on the specific broken call site to check whether the break shipped upstream.

---

## F-31 — Memory-tool bug blocked reading the documented Windows binary-lock workaround, forcing rediscovery from scratch

> **VERIFY-OPEN 2026-07-28 → `fixed-verified` (with a caveat that became its own
> bug).** The blocker is gone: `memory(action="list")` returns 21 topics (= the 21
> `.md` files on disk), and the content this entry could not reach is readable at
> `.codescout/memories/gotchas.md:25` (`## MCP Binary Symlink`). The underlying bug
> `docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
> is now `zombie` — symptom gone, root cause never confirmed, no attributable fix
> commit.
>
> **Caveat:** the *targeted* read still fails, for a different reason.
> `memory(read, topic="gotchas", sections=["MCP Binary Symlink"])` errors with "this
> memory has no ### sections to filter" because the `sections` filter matches `###`
> only and 15 of 21 memories use `##` (87 headings). So the rediscovery cost this
> entry describes is *reduced* — a full read now works — but not eliminated. Filed
> as `docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md`.
>
> Not covered: the original report followed `workspace(activate)` on a FOREIGN
> project. This pass ran on the home project, so the foreign-activate path the title
> names is unverified.

**Observed:** 2026-07-07, mid-session, `cargo rb` failed with `error: failed to remove file ...codescout.exe ... Access is denied (os error 5)` because 3 running `codescout.exe` MCP-server processes held the target binary open.

**When:** Attempting to rebuild the release binary while the MCP server (this very session's own connection) was live.

**Expected:** CLAUDE.md explicitly documents this exact scenario: "the binary symlink gotcha → memory `gotchas` (MCP Binary Symlink)" — a one-line memory read should have surfaced the known, established workaround immediately.

**Got:** `memory(action="read", topic="gotchas")` (and every other project-scoped topic except `language-patterns`/`onboarding`) failed with "topic not found" — a separate bug this same session had already found and filed (`docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`): `workspace(activate)` reports 16 memory topics for the `codescout` project, but `memory(list/read)` only sees 2, even with `project_id` passed explicitly. With the documented fix unreadable, the `CARGO_TARGET_DIR=target-release-new` + `Stop-Process -Force` + copy-into-place workaround had to be re-derived from first principles (inspecting `git status` for a pre-existing `target-release-new/` directory as a clue, then confirming the pattern via `Get-Process`).

**Probable cause:** The memory-tool project-scoping bug (F-cited above) made the documented institutional knowledge for this EXACT situation completely inaccessible mid-task, at the worst possible time (blocked on a build).

**Workaround:** Re-derived the fix independently: build into a side `CARGO_TARGET_DIR`, `Stop-Process -Force` the locking PIDs, copy the fresh exe over the locked path. Worked, but the user was not available to confirm the force-kill of their own live MCP session before it was performed autonomously — an operational-safety judgment call made harder by not having the documented precedent in hand.

**Severity:** med — no incorrect fix shipped, but real risk: an autonomous `Stop-Process -Force` against a live session's own server process, made without the benefit of prior documented guidance on whether that's the sanctioned procedure.

**Status:** fixed-verified (2026-07-29 verify-open pass) — was `open` in the body while
the index row already read `fixed-verified`; the two are now aligned, and the body was the
stale side.

The premise no longer holds, and it is directly testable rather than inferred: this entry
says the memory tool blocked reading the `MCP Binary Symlink` gotcha. That exact call now
works —
`memory(action="read", topic="gotchas", sections=["MCP Binary Symlink"])` returns the
section. Note that it works for a reason unrelated to this entry's root bug: the
`sections` filter was itself broken (`###`-only against a `##`-sectioned memory) and was
fixed and live-verified this session
(`docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md`).

The root bug this entry blamed —
`docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
is now `zombie`, i.e. not reproducible on retest, so "filed but not yet fixed" was stale
in both of its clauses. Two independent things had to be re-checked to retire one entry,
which is the usual shape: a friction's stated cause and its observed cost decay on
different clocks.

**Fix idea / Pointer:** Fix `docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`, then re-verify this specific recovery path against the (currently unreadable) `gotchas` memory's actual documented procedure to confirm today's improvised workaround matches or diverges from it.

---

## W-23 — Running the FULL `cargo test --lib` after a single targeted fix surfaced 5 more failures and 1 real shipped defect the narrow fix would have missed

**Observed:** 2026-07-07, after fixing the `librarian(doctor)` `ads_colon_in_abs_path` false-positive (a Windows extended-length-path `//?/` prefix not being recognized by `check_ads_colon`), the fix + its own 2 new unit tests were green in isolation.

**Pattern:** Instead of stopping at "the specific test I was asked about now passes," ran the full `cargo test --lib` before considering the task done. This surfaced 5 MORE pre-existing failures in unrelated modules (`agent::tests`, `server::tests`, 3x `tools::config::tests`), all from the same upstream forward-slash-normalization series but never caught because no one had run the full suite on Windows against that series before.

**Counterfactual:** Without the full-suite run, the task would have been reported "done" with `ads_colon` fixed but 6 tests still red in CI — including one (`activate_hint_shows_switched_when_away_from_home`) that, on inspection, was not test rot at all: `build_activation_response`'s `SwitchAway` hint text genuinely mixed forward-slash (`project_root_str`, via `to_forward_slash`) and native-backslash (`home_str`, via `.display().to_string()`) path renderings in the SAME sentence on Windows — a real, shipped production inconsistency in the MCP hint text served to every agent switching projects, not a test-only issue. Narrow-scoping to "just fix ads_colon" would have shipped that defect untouched.

**Confirming data points:**
1. `cargo test --lib` after the ads_colon-only fix: 2858 passed, 6 failed (vs. 2864/0 after all 7 fixes).
2. 5 of 6 failures were pure test-assertion drift (comparing native-separator `PathBuf::to_str()`/`.display()` against now-normalized forward-slash output) — test-side fixes.
3. 1 of 6 (`activate_hint_shows_switched_when_away_from_home`) traced to `src/tools/config/mod.rs:602-621` — a genuine source-side bug, fixed at the root (`home_str` now uses `to_forward_slash` like `project_root_str` does).

**Impact:** high — caught a real, user-facing (agent-facing) production defect that a narrowly-scoped "fix the one thing I was asked about" pass would have shipped.

**Promote-when:** Second occurrence of "narrow fix passes cargo test on the touched file/module, but full `cargo test --lib` finds sibling breakage from the same root-cause series" — this session is the first datapoint; W-6 (2026-05-24, cross-platform read/write seam misses) is a closely related but distinct prior instance.

**Status:** validated

---

## F-29 — Pre-compaction estimate of `ArtifactRow` fixture duplication ("3-4 sites") undercounted reality by 5x

**Observed:** 2026-07-07, resuming the code-duplication hunt after `/compact`, following up on the `TestToolContextBuilder` refactor's closing note that flagged `mk_row`/`seed_artifact`-style `ArtifactRow` fixtures as a smaller, secondary duplication candidate "at the rule-of-three threshold with only 3-4 sites."

**Expected (pre-compaction note):** ~3-4 call sites, borderline rule-of-three case, possibly not worth a dedicated builder.

**Got (scouted reality):** `grep(pattern="ArtifactRow\\s*\\{")` at workspace root found 81 struct-literal matches across 34 files; filtering to local test-fixture helper functions (`fn \w+(...) -> ArtifactRow`) found **21 independent constructors across 20 files** (`art`, `sample`, `sample_art`, `mk_row`, `row`, `sample_row` — inconsistent naming, one per file), each hand-rolling the same 15-field struct literal (`id, abs_path, kind, status, title, owners, tags, topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence`) with only 1-3 fields actually parameterized per call site. This is the same defect shape as the `ToolContext` duplication just fixed this session (also undercounted pre-execution: 27 estimated vs 65+ actual), not a smaller cousin of it.

**Probable cause:** The pre-compaction note was written from memory of a quick earlier grep, not a fresh recon pass — the same failure mode the reconnaissance skill exists to prevent (assuming stale shape instead of re-scouting before acting).

**Workaround:** None needed — recon ran before any edit, so no wrong-scoped work landed. Treating this as its own full `TestArtifactRowBuilder` extraction (peer to `TestToolContextBuilder`), not a "maybe not worth it" 3-4-site tweak.

**Severity:** med — no failed tool call resulted (recon caught it pre-edit), but acting on the stale "3-4 sites" estimate would have produced a scope-widening surprise mid-refactor identical to the `ToolContext` Phase-1 experience, this time avoidable in advance.

**Status:** fixed-verified — recon ran before any code change; corrected scope adopted before starting the `TestArtifactRowBuilder` extraction.

**Fix idea / Pointer:** `docs/trackers/bug-fix-session-log.md` W-22 (this session) — the actual extraction, once landed.

---

## W-22 — Diff-before-test discipline caught 2 field-omission bugs in the ArtifactRow builder migration before any test ran

**Observed:** 2026-07-08, Phase 3 of the `TestArtifactRowBuilder` migration (F-29), converting `refresh_stale.rs::sample_art` and `constitution_check.rs::sample_art` to builder calls.

**Pattern:** Adopted after the `find.rs::mk_ctx` miss in the `ToolContext` refactor (same session, prior phase): after every `edit_code` conversion in a batch, run `git diff` on the touched files and read every hunk field-by-field against the pre-edit body *before* running `cargo test`, not after.

**Counterfactual:** `git diff` on `refresh_stale.rs` showed the converted call omitted `.with_file_sha256("abc")` (original had a non-default sha; builder default is `String::new()`). Same pass on `constitution_check.rs` showed a worse miss: BOTH `.with_abs_path(format!("/test/{id}.md"))` (original path has no `/r/` segment; builder default does) AND `.with_file_sha256("x")` were dropped. Neither field appears to be asserted by any test in those files (`cargo test --lib` was not yet run against the buggy version, but both bugs are silent-until-asserted class defects — the kind that would pass green and only surface if a *future* test started asserting on `abs_path` or `file_sha256`). Without the diff-first step, both bugs would have shipped as silent fixture drift indistinguishable from a passing refactor.

**Confirming data points:** 1) `find.rs::mk_ctx` (ToolContext refactor, same session) — caught only after tests failed, requiring a `git stash` isolation investigation to rule out flakiness. 2) This entry — caught by diff review alone, zero failed test runs, zero investigation overhead. The same root cause (converting a fixture fn from memory/pattern-matching instead of re-reading its literal body) recurred, but the fix cost dropped from "diagnose a red test suite" to "catch it in a diff before running anything."

**Impact:** high — the second occurrence involved 2 silently-wrong fields in one function (abs_path AND file_sha256), which a green test suite would not have caught; this is exactly the failure class that erodes trust in "tests pass" as a completeness signal for fixture refactors.

**Promote-when:** Already promoted in practice this session (used across all of Phase 2 and Phase 3 of this refactor without further misses after adoption). Formalize: `docs/PROGRESSIVE_DISCOVERABILITY.md` or a CLAUDE.md note — "when converting N>1 hand-rolled struct-literal test fixtures to a shared builder, diff every converted file against its pre-edit body before running the test suite; do not rely on `cargo test` green as proof the literal fields transcribed correctly."

**Status:** validated — 2 real bugs caught pre-test, pre-commit, in a single phase.

---

## W-24 — Cross-session correlation of a live bug report against this session's own concrete observation produced an appropriately-confidence-labeled hypothesis instead of speculation

**Observed:** 2026-07-07, asked to re-read an unrelated (Mercury BOM project) bug report describing `reindex`/`find` catalog inconsistency and a suspected "restart reverts catalog to a stale snapshot" mechanism, then asked whether this session's own practice of `Stop-Process -Force`-killing `codescout.exe` before a binary swap could be a contributing cause.

**Pattern:** Rather than asserting a causal link on intuition, traced the actual catalog-open code (`src/librarian/catalog/mod.rs:157-176`) to confirm WAL mode + `busy_timeout=5000` is an explicitly *designed-for* multi-process scenario, checked for (and found none) any single-instance guard or graceful-shutdown checkpoint on `Catalog`, and cited this session's OWN directly-observed evidence (3 concurrent `codescout.exe` processes found via `Get-Process` before any intervention) as the reproduction path for the *precondition* — while explicitly labeling the causal claim itself "confidence: medium, plausible aggravating factor, not a confirmed root cause" rather than overclaiming a fix.

**Counterfactual:** Without tracing the actual `Catalog::open` code, the answer would have been speculation ("probably yes/no") with no citable evidence, either wrongly reassuring the user that force-killing is safe, or wrongly implicating a mechanism (e.g. WAL corruption) that the code doesn't actually support (WAL is crash-safe by design; the real gap is the ABSENCE of a single-instance guard, a subtler and more actionable finding).

**Confirming data points:**
1. `src/librarian/catalog/mod.rs:157-176` comment: "Cross-process writers (separate codescout server instances sharing one catalog file) block and retry instead of failing immediately" — confirms multi-process-by-design, not accidental.
2. `grep` for `wal_checkpoint`/`impl Drop for Catalog` across the whole repo: zero matches — confirms no explicit checkpoint-on-shutdown exists.
3. This session's own `Get-Process codescout` output, twice, each showing 3 live PIDs before any manual kill — direct, first-party evidence for the precondition (not inferred from the Mercury BOM report alone).

**Impact:** med — produced a citable, appropriately-hedged addition (Evidence E10, Hypothesis 6) to another project's live bug tracker instead of either silence or overclaimed certainty.

**Promote-when:** A second cross-project correlation where tracing actual code (not just citing symptoms) changes the confidence level assigned to a hypothesis.

**Status:** validated

---

## F-32 — `edit_code` rewrite of a test fn dropped a trailing assertion line outside the read window

**Observed:** 2026-07-08, flattening `CommonOpts` into 15 CLI arg structs (`src/cli/*.rs`). Rewrote `tests/run_state_at_rejects_missing_cutoff` in `artifact.rs` via `edit_code(action="replace")`.

**Expected:** the rewritten body would be complete — I'd read the function before editing.

**Got (scouted reality, via post-edit diff review):** the earlier `read_file(start_line=605, end_line=735)` call used to gather all 4 test bodies needing this edit ended its window at line 735, one line short of the function's actual closing content. The last line visible was `assert!(msg.contains("--commit"), "got: {msg}");` — I took that as the function's end and wrote a "corrected" body that stopped there, silently dropping the next line, `assert!(msg.contains("--timestamp"), "got: {msg}");`, which existed in the original but sat just outside my read window. `git diff` after the edit showed it as a deleted line with no corresponding addition — caught before compiling, before tests ran, before commit.

**Probable cause:** distinct from F-15/the `mk_ctx` miss (those were "didn't read the body at all") — here the body WAS read, but the read window's end coincided almost exactly with the function's true end, and I didn't verify the window actually reached a closing `}` before treating the visible content as complete.

**Workaround:** re-ran `edit_code(action="replace")` with the missing assertion restored, then re-diffed to confirm a clean state.

**Severity:** med — would have silently weakened a regression test (removing one of two assertions on a "missing cutoff" error message) with a green build; the diff-before-test discipline (W-22, same session) caught it before any test ran.

**Status:** fixed-verified — caught and corrected before compiling; diff re-verified clean.

**Fix idea / Pointer:** When a multi-symbol batch read's line-range window ends close to a target symbol's expected end, confirm the read actually captured a closing `}` (or re-read with a generous overshoot) before using it as the basis for an `edit_code` rewrite — don't trust "the last line I saw" as "the last line that exists."

---

## F-33 — Relocated the mux lock dir before verifying nothing discovers those files by directory scan

**Observed:** 2026-07-28, fixing the lock-file leak in `src/retrieval/index_lock.rs`
and `src/lsp/mux/mod.rs`. Recon was invoked *after* both edits were written and
`cargo clippy --all-targets -- -D warnings` was already green.

**When:** Mid-fix. The `index_lock` half used parameter injection
(`lock_path_in` / `acquire_in`), which is local and provably safe. The mux half
changed where files *live* — `mux_dir()` returns a per-process scratch
subdirectory under `cfg(test)` — and I wrote it on the unstated assumption that
every consumer computes its exact path rather than discovering files by scanning
the directory.

**Expected (my assumption):** No code enumerates `per_user_runtime_dir()` looking
for `codescout-*-mux-*.sock`.

**Got (scouted reality):** The assumption held, but only checkable after the fact.
`grep 'read_dir|glob'` across `src/**/*.rs` returns zero hits in any runtime-dir
context, and all four production call sites of the mux path helpers
(`src/lsp/manager.rs:831-832`, plus the `#[ignore]`d test at 2350-2351) go through
the two functions changed. The peer subsystem is fully separate
(`peer_*_for_workspace` in `src/socket_discovery.rs:43-56`, consumed by
`src/peer/{launch,registry,server}.rs`) and untouched.

One real residue: `src/socket_discovery.rs:64`'s test is named
`peer_socket_differs_from_mux_and_shares_dir`, and the mux/peer shared-directory
invariant its name implies is now FALSE in test builds. The body never asserted it
— it only checks `peer.parent() == per_user_runtime_dir()` and that the *filename*
lacks a `-mux-` infix — so the test passes and nothing will ever flag the
divergence. The name misleads silently.

**Probable cause:** Edit-then-scout inversion. `index_lock`'s seam was read this
session, so it felt like the mux seam had been too; it had not. The mux change is
also categorically different — it moves a *location* other code may depend on,
rather than adding a parameter — and that difference is what should have triggered
a blast-radius scout before the edit, not after.

**Workaround:** Left the test name (it is cited as a copy-pasteable `cargo test`
invocation at `docs/superpowers/plans/2026-06-01-peer-delegation-phase1.md:105`,
so renaming breaks a doc reference) and added a comment at
`src/socket_discovery.rs` recording that "shares_dir" refers to the peer socket
sitting in the per-user dir, not to co-location with mux, plus why the name stays.

**Severity:** low — a directory-scan consumer would have failed loudly in the
verification run that was already queued, and no production scan exists, so the
realistic cost was one failed 11s test run plus a redesign. The durable cost is the
misleading test name, which no gate can catch.

**Status:** fixed-verified — assumption confirmed by scout; name imprecision
neutralised by comment; full suite green (18 binaries, 0 failed, 3307 lib tests).

**Fix idea / Pointer:** `src/lsp/mux/mod.rs` `mux_dir()`; `src/socket_discovery.rs`
test comment. Generalisable rule in W-25.

## W-25 — Scout for discovery-by-directory-scan before relocating a file whose path is computed in one place

**Observed:** 2026-07-28, fixing the two-module lock-file leak
(`docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md`).

**Pattern:** When a fix changes *where* a file lives rather than *what* is in it,
the blast radius is not the set of callers of the path helper — it is the set of
code paths that locate the file *without* calling it. Two greps settle it:
`read_dir|glob` over the source tree, and every caller of the path helper
(`grep 'socket_path_for_workspace|lock_path_for_workspace'`). If the first is empty
and the second is exhausted by the functions being changed, relocation is safe and
the cheap `cfg(test)` redirect can stand.

**Counterfactual:** Without this scout there were two bad branches. Defensive:
reject `cfg(test)` and thread a directory parameter down through
`LspManager::get_or_start` -> `get_or_start_via_mux` -> `claim_mux_lock`, putting a
test-only concern in the LSP manager's public signature and requiring all 17
leaking tests to opt in individually — versus the 3-line seam that actually shipped
and fixed all 17 at once. Optimistic: ship the redirect and discover a scanner
later, in a subsystem where mux sockets simply stop being found. The scout also
surfaced two things the edit did not anticipate: the `#[ignore]`d wedged-mux test
at `src/lsp/manager.rs:2350-2351` stops leaking for free, and the peer subsystem
(`src/socket_discovery.rs:43-56`) shares the same real directory and the same
latent exposure — no files on disk, so not leaking, but the third instance of this
pattern if it is ever exercised.

**Confirming data points:**
1. F-33 (this session) — assumption verified only after the edit; held, but the
   scout is what made it knowable, and it caught the misleading test name.
2. W-6 (2026-05-24) — the read-side/write-side twin: normalizing writes without
   scouting every read seam left six read-side boundaries broken, one
   destructively. Same shape: a representation/location change whose blast radius
   lives at the seams that *consume* the value, not the ones that produce it.

**Impact:** med — saves one invasive-refactor-vs-cheap-seam misjudgement and one
round of silent-discovery-breakage risk per relocation.

**Promote-when:** A third relocation-shaped change (file path, socket path,
collection name, cache key) where the scan-vs-compute question decides the design.
At 3 datapoints, promote to the reconnaissance SKILL.md as a named seam class:
*"Relocation seam — when a change moves where a thing lives, grep for
discovery-by-scan before counting callers; a scan-based consumer is invisible to
caller enumeration."* This is craft-shaped, not project-shaped (it holds for any
language and any runtime directory), so the destination is the skill, not a
project memory.

**Status:** validated — single datapoint plus a same-shape precedent in W-6.
Awaiting the third relocation case.
## F-34 — My own bug file prescribed a fix that a pre-existing test would have broken

**Observed:** 2026-07-28, implementing
`docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md` — a bug I had
filed myself earlier the same session.

**When:** About to write the fix. The bug file's own Fix section listed, as option 1
("cheapest, consistent with the sibling fix"): *"Match any heading level (`##`,
`###`, `####`) and keep the existing case-insensitive comparison. Smallest change …
and cannot break `language-patterns.md`."*

**Expected (my own filed plan):** promoting every heading level to a section boundary
is safe.

**Got (scouted reality):** it breaks `filter_sections_nested_h4_included_in_body`
(`src/memory/filter.rs`), which exists precisely to assert that `####` is part of its
`###` section's **body**, not a sibling section. My "cannot break
`language-patterns.md`" claim was exactly backwards — `language-patterns.md` is the
one memory with `###` + `####`, so it is the file the naive fix would have damaged.

A second, larger miss in the same plan: option 1 said nothing about H1. Counting the
corpus showed 19 of 21 memories carry exactly one H1 as their title, so any
"shallowest level wins" variant that includes H1 makes the title the sole block and
nests every section inside it — filtering would *appear* to work while always
returning the whole document. That is a worse failure than the bug being fixed,
because it is silent.

**Probable cause:** the bug file was written from the tool's error text plus a
heading census of the *data*, without reading the function's own test module. Its Root
cause section even admitted the mechanism was "inferred". A fix option proposed from
inferred mechanism carries inferred safety, and I labelled it "cheapest" — which is
the part that would have made a future session trust it.

**Workaround:** discarded option 1. Shipped shallowest-level-among-H2..H6 with H1
excluded, which satisfies both conventions and leaves all 13 pre-existing tests
untouched. Rewrote the bug file's Fix section to record why option 1 was rejected, so
the dead end is not re-walked.

**Severity:** med — had I implemented my own filed plan without scouting, I would
have shipped a green-looking change (`language-patterns.md`'s own filter tests would
still pass on the `###` sections) that silently broke `####` nesting, plus an H1
variant that returns whole documents while reporting a match.

**Status:** fixed-verified — `d668927e`; 19/19 in `memory::filter`, full gate 3439
passed / 0 failed.

**Fix idea / Pointer:** generalised in R-46. A bug file's Fix section should be marked
as unvalidated until someone has read the target's test module — "inferred mechanism"
in Root cause should propagate a warning into Fix.

## W-26 — A failing new test with an innocent implementation: diagnose the fixture before touching the code

**Observed:** 2026-07-28, immediately after writing the headline regression test for
the memory section-filter fix. `filter_sections_matches_h2_sectioned_memory` failed on
`assert!(r.matched)`.

**Pattern:** when a *newly written* test fails against code you just wrote, check
whether the **fixture** is what you authored before concluding the implementation is
wrong. Read the test back off disk and compare it to the argument you sent. The
privileged clue is a test whose failure mode is *also* an explicitly tested intended
behaviour elsewhere in the same module — here `filter_sections_indented_heading_not_a_boundary`
asserts that an indented `### Fake` is body, and my fixture's headings had silently
acquired four leading spaces.

**Counterfactual:** the natural next move on "my `##` matching test fails" is to
suspect `boundary_level` / `heading_at` and start loosening the heading predicate —
e.g. trimming leading whitespace before matching. That change would have made the new
test pass **and** broken `filter_sections_indented_heading_not_a_boundary`, at which
point the obvious-looking move is to "fix" that older test too, landing a real
regression (indented markdown inside a section body would start splitting sections).
Instead, reading the file showed `    ## MCP Binary Symlink` and the implementation was
exonerated in one step — and the actual defect turned out to be in `edit_code`, not in
either the code or the test.

**Confirming data points:**
1. F-32 (2026-07-08) — `edit_code` rewrite of a test fn dropped a trailing assertion
   line outside the read window. Same root family: `edit_code` silently altering what
   you believe you wrote, detected only by reading the result back.
2. This session — `edit_code`'s `reindent_to` shifted a multi-line string literal's
   interior; filed as
   `docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`.

**Impact:** med — saves one plausible-but-wrong loosening of a parser predicate plus
the cascade of "fixing" the older test that would then fail.

**Promote-when:** a third instance of an `edit_code` write silently differing from the
submitted body. At 3 datapoints, promote to CLAUDE.md as *"after any `edit_code`
write, read the affected region back before trusting a test result against it"* — two
of the three datapoints are already in this log (F-32, this entry).

**Status:** validated — two datapoints in the same family, both `edit_code` write
fidelity, both caught by reading the result rather than by the gate.
## F-35 — My own doc comments made two claims the code did not support, and no test could catch either

**Observed:** 2026-07-28, immediately after `cargo check --lib` passed on the
`literal_continuation_mask` fix in `src/util/text.rs`. Re-reading what I had just
written, before running the suite.

**When:** Writing the doc comments for two new private helpers while implementing the
fix for `2026-07-28-edit-code-reindent-shifts-string-literal-contents`.

**Expected (what I wrote):**

1. On `min_indent_outside_literals`: *"Falls back to the whole block when every non-blank
   line is literal content."*
2. On `literal_continuation_mask`: *"a line-spanning literal that never closes … masks
   every line after it, which degrades to reindenting nothing and handing the block back
   byte-for-byte."*

**Got (traced reality):**

1. Unreachable as stated. The mask is built by pushing the state *before* scanning each
   line, and the scanner starts in `Scan::Code` — so index 0 is `false` unconditionally.
   If line 0 is non-blank there is always at least one unmasked non-blank line, and the
   `unwrap_or_else` fires only for an empty or all-blank block, where plain `min_indent`
   returns `""` anyway.
2. Wrong by one line, in the direction that matters. A never-closing literal masks lines
   1..n but *not* the opener, so the base is measured from the opener and the opener is
   the one line that still gets reindented. "Reindenting nothing" overstates the
   guarantee.

**Probable cause:** Both claims were written while reasoning forward from intent ("the
fallback exists, so describe when it fires"; "masking everything means shifting nothing")
rather than backward from the loop's first iteration. The mask's off-by-one at index 0 is
exactly the kind of boundary that intent-first prose skips.

**Workaround:** Rewrote both. (1) now states the fallback is reachable only for an
empty/all-blank block and that both functions agree on `""` there. (2) now says at most
the opening line is reindented.

**Severity:** low — no behavioural defect; the code was right both times. It is logged
because of what the detection channel was: the full gate is green either way, and
`audit_doc_refs` lints markdown, not Rust doc comments. Nothing in the pre-commit gate
can catch a doc comment that overstates a guarantee. The only detector is re-reading the
claim against the loop, which is the same discipline the Iron Rule already names for
user-facing claims — this entry extends it to comments, which outlive any answer.

**Status:** fixed-verified — both comments corrected before the commit; landed in
`79cd1428`.

**Fix idea / Pointer:** `src/util/text.rs`, docs on `literal_continuation_mask` and
`min_indent_outside_literals`. Second datapoint in a pattern worth watching: F-34 was a
claim in my own *bug file* that the code contradicted; this is a claim in my own *code
comment*. Both were caught by reading the artifact back against the substrate, neither by
a gate.

## W-27 — Enumerate a shared helper's callers before deciding where a fix belongs, not just to check blast radius

**Observed:** 2026-07-28, scouting `src/util/text.rs` before implementing the
reindent/literal fix. The bug file already prescribed a fix location; the scout was
nominally just to confirm the shape.

**Pattern:** When a bug report names a fix site inside a helper, run `references` /
`grep` on **every** function in that helper's call chain — not to size the blast radius,
but to ask *which link in the chain the defect actually lives on*. A report is written
from the one call path its author walked; the enumeration shows the others.

The bug file's option 1 and option 2 both located the fix in `reindent_to` — option 1 in
its base computation, option 2 in the shift it delegates. Enumerating turned up the
shape that decided it: `reindent_to` has 3 callers, all in `edit_code.rs`; but
`reindent_block`, the function it delegates the shift to, has a **second production
caller the report never mentions** — `src/tools/edit_file/mod.rs:747`, on the
whitespace-normalized-match repair path, computing its bases from the first non-blank
line instead of via `min_indent`. Same literal-shifting defect, reached without passing
through `reindent_to` at all.

So the mask went into `reindent_block`. Same size of change, twice the surface closed.

**Counterfactual:** Implementing either option as written would have left
`edit_file`'s repair path silently shifting literal contents. That path is *structurally
unable* to notice: it guards its result with `crate::ast::has_syntax_errors` before
writing, and a string literal with four extra spaces still parses — the guard returns
clean on exactly the corruption it is positioned to catch. The bug would have stayed
open behind a green gate, a closed bug file, and a passing syntax check, reachable only
by a `edit_file` caller who happened to hit the normalized-match path with a multi-line
literal in the replacement. Cost of the scout: one `grep`.

**Confirming data points:**
1. This session — `reindent_block`'s second caller relocated the fix and doubled its
   coverage; the report's own preferred option would have missed it.
2. R-45 (`reconnaissance-patterns`) — caller enumeration is blind to
   discovery-by-directory-scan. Same tool, opposite lesson: enumeration is necessary but
   its silence is not proof.

**Impact:** med — one grep converts a half fix into a whole one, on a defect class whose
defining property is that no gate detects it.

**Promote-when:** A third case where enumerating a *delegate's* callers (not the named
symbol's) relocates the fix. At 3 datapoints, promote to the reconnaissance skill as
"scout the callers of every function in the chain the fix touches, not only the one the
report names."

**Status:** validated — fix landed at the relocated site in `79cd1428`, with
`reindent_block_emits_literal_continuations_verbatim` pinning the `edit_file` entry point
specifically.

## F-36 — The fix I shipped covered the reported literal form, not the commonest one; six tests all reused the report's shape

**Observed:** 2026-07-28, ~15 minutes after committing `79cd1428`, while designing the
live end-to-end verification of that very commit.

**When:** Post-`/mcp` reconnect, working out what `edit_code` call would prove the
reindent/string-literal fix works against the release binary.

**Expected (what I had shipped):** a literal scanner covering every line-spanning literal
form the supported languages use — `"`/`'` held open by a trailing `\`, triple-quoted
Python, backtick, Rust raw string at any hash count. The doc comment enumerated all four
and the commit message claimed the class.

**Got (traced reality):** a `"` literal spanning lines on a **raw newline**, no trailing
`\`, was not covered. Rust permits raw newlines inside `"…"`, so
`let content = "# Heading\n\n## Section\n";` written across real lines is valid — and is
the commoner way to write a multi-line fixture. My end-of-line rule reset an unclosed `"`
to code, on the reasoning that a stray quote is likelier than a literal. For `'` that
reasoning is right (lifetimes, apostrophes); for `"` it is backwards, and `//` comments
were already bailed out, which removes the main source of stray quotes.

**Probable cause:** I built the scanner from the bug report's reproduction and then wrote
six tests against it. The report used `"\`. Every test therefore used `"\`. The test suite
had no way to catch the gap because the whole suite inherited one fixture shape — six
tests, one sample. Passing six tests read as covering the class; it covered the sample.

**Workaround:** none needed — widened the rule instead: an unclosed `"` at end of line is
now a confirmed multi-line literal, `'` still resets, and the asymmetry is documented as
deliberate with its reasoning. New test
`literal_continuation_mask_covers_a_raw_newline_double_quoted_literal`.

**Severity:** med — the shipped fix would have left the *majority* Rust fixture form
silently corrupting, behind a green gate, seven passing tests, an archived bug file and a
commit message claiming the class. Not high only because the window was 15 minutes and no
other caller hit it.

**Status:** fixed-verified — widened in the same session; gate 18 binaries / 3446 passed /
0 failed. Live confirmation of the widening awaits the next `cargo rb` + `/mcp`; the
original fix is already live-verified.

**Fix idea / Pointer:** `src/util/text.rs`, `scan_line`'s end-of-line match. Third
datapoint in the F-34 / F-35 family — a claim of mine that the substrate did not support,
caught by reading rather than by a gate. The distinguishing feature here: the claim was
in a *commit message and a doc comment enumerating a class*, and the enumeration was
incomplete rather than wrong. Enumerations are the easiest kind of claim to overstate,
because each item in the list is individually true.

## W-28 — Writing the end-to-end verification found what seven unit tests could not

**Observed:** 2026-07-28, immediately after the `/mcp` reconnect that made `79cd1428`
live.

**Pattern:** When a fix is verified end-to-end through the real tool, write the
verification fixture **the way a user would naturally write it** — not the way the bug
report wrote it, and not the way the unit tests write it. Then trace the fix over that
fixture *before* running it. The natural form and the reported form are different samples
of the defect class, and only the natural form tells you whether the fix generalises.

Concretely: to prove `edit_code` no longer shifts a multi-line literal, I had to write one
as a real multi-line literal. The natural way in Rust is a plain `"` with raw newlines.
Tracing my own scanner over that fixture showed it resetting at the first line end — the
gap in F-36. The six existing tests could not have found it: all six reused the report's
`"\` shape, so the suite sampled one point of the class six times.

**Counterfactual:** Without writing the live fixture, `79cd1428` ships as-is, with an
archived bug file, a commit message claiming the literal class, and seven green tests. The
commonest multi-line-string form in Rust keeps corrupting silently. The next person to hit
it hits it as a *new* bug against a closed one — and the closed file's own evidence
section would argue the mechanism was already understood and fixed, which is the worst
possible starting position for a rediscovery. Cost of the discipline: writing the fixture
the natural way instead of copying an existing one.

Secondary win from the same pass: the fixture stayed in the test file rather than being
thrown away, so `reindent_block_emits_literal_continuations_verbatim` now carries real
multi-line literals. The verification is *retained* — if `edit_code` regresses, rewriting
that test corrupts its own fixture and the assertion fails.

**Confirming data points:**
1. This session — the raw-newline gap, invisible to seven unit tests, found by writing one
   live fixture.
2. W-26 (same session) — a failing new test whose fixture, not implementation, was at
   fault. Same underlying asymmetry: the fixture is as much under test as the code, and
   reusing a known-good fixture shape hides whatever that shape does not exercise.

**Impact:** med-high — catches the class of incomplete fix that a green suite actively
camouflages, at the cost of authoring one fixture from scratch.

**Promote-when:** a second case where an end-to-end verification fixture, written in the
natural form rather than copied from the report, exposes a gap the unit tests shared. At 2
datapoints, promote to the reconnaissance skill: *"verify through the real tool with a
fixture written the natural way — a suite that inherits the report's fixture shape samples
one point of the defect class, however many tests it has."*

**Status:** validated — gap found, widened, and the fixture retained as a live regression
probe.

## F-37 — I filed a root cause I had not verified: read both ends of a call chain, inferred the middle

**Observed:** 2026-07-28, scouting `src/tools/symbol/edit_code.rs` before implementing the
fix prescribed by my own bug file from earlier the same session.

**When:** About to implement `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`.

**Expected (what the bug file asserted, as a finding):** `edit_code` computes the insert
column as `leading_ws(lines[editing_start_line(&sym, &lines)])` — an LSP index applied to
a freshly-read file — and when the LSP lags, the index selects a different line whose
indentation silently becomes the base. The file stated this in a `## Root cause` section,
with two code citations, as established.

**Got (scouted reality):** the citations are right and the conclusion does not follow.
Between the two functions I read sits `fetch_validated_symbol`, which calls
`validate_symbol_position` **before** anything else and rejects a stale `start_line` — the
name must appear on that line or just below — returning a `RecoverableError` so the fetch
retries with a fresh `did_change`. A wholesale stale position is caught, not silently
used. And for the anchor in question (`tests/reindent_to_preserves_blank_lines`), every
candidate value of `range_start_line` — the `#[test]` line, the `fn` line, or a collapsed
selection range on the identifier — sits at **column 4**. No reading of the observed range
produces the column 8 that was actually applied.

A second data point killed the "systematic" framing too: a later `edit_code insert` in the
same file, same `mod tests`, same anchor shape, body likewise at column 4, landed at
column 4. The two calls differ in one observable — the first reported a `range_repair`.

**Probable cause:** I read the two ends of a call chain and inferred the middle. The write
site (`target_base = leading_ws(lines[…])`) and the index's origin (`editing_start_line`
keying off `range_start_line`) both say what they say; the guard that sits between them
does not appear in either, and I did not go looking for it. The bug file was written
immediately after the observation, while the surprise was fresh — which is right for
capture and wrong for causation.

**Workaround:** rewrote `## Root cause` as two sections, *Established* and *Not
established*, and downgraded the entry to `mitigated`. What survived is sharper than the
original claim: `range_start_line` really is validated by nothing and repaired by nothing,
whereas `start_line` is validated on every fetch. Shipped the changes that stand on
verified properties alone, and explicitly deferred anything that assumes the index was
stale.

**Severity:** med — no wrong code shipped, because the scout happened before
implementation. But the file had already been committed (`3a9c5f26`) with a wrong
mechanism stated as fact, and the fix I was about to write would have been justified by
it. A bug file's `## Root cause` is what the next reader trusts instead of re-deriving.

**Status:** fixed-verified — file corrected, entry downgraded to `mitigated`, hazards that
*are* verified closed with four tests.

**Fix idea / Pointer:** `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`.
Third instance this session of my own artifact failing under later scrutiny — F-34 (a
bug file's fix option was inverted), F-35 (doc comments overstated a guarantee), F-37 (a
root cause the code did not support). See R-49 for the pattern.

## W-29 — Re-scout your own bug file before implementing its fix

**Observed:** 2026-07-28, immediately before writing the fix for a bug file I had authored
an hour earlier.

**Pattern:** Treat your own bug file's `## Root cause` as a hypothesis on re-entry, and
re-scout it with the same skepticism you would give a stranger's. Specifically: when a
root cause cites two functions, **read the layer between them** — a call chain's middle is
where the guards live, and a mechanism inferred from its two ends will not mention them.

**Counterfactual:** Without the re-scout, the fix ships justified by a mechanism that does
not hold. Three costs, and the third is the expensive one. (1) The chosen option would have
been "thread the AST-repaired start into the base" — real work aimed at a cause not shown
to exist. (2) The bug file would have been closed with its wrong story intact, so the next
recurrence arrives as a new bug against a closed one whose root cause section confidently
misdirects. (3) The genuinely broken thing would have stayed broken: the re-scout is what
surfaced `editing_start_line`'s documented discard-the-walk-back path, which returns
`range_start_line` unchanged and so samples the column from a ` * ` block-comment
continuation — one column deeper than the declaration. That is a real, code-verified
defect in Kotlin/Java/Javadoc-shaped files, it has no relationship to LSP staleness, and
nothing in the original bug file pointed at it. Cost of the re-scout: two symbol reads.

What made the difference concrete: the scout replaced a fix justified by *my belief about*
an observation with a fix justified by *a property of the code*, and the second one is
testable without an LSP. Four unit tests, no integration harness.

**Confirming data points:**
1. This session — root cause unsupported, and a different real defect found in its place.
2. W-26 / F-34 (same session) — the same bug file's *fix options* were inverted, caught by
   reading the target's test module. Same artifact, different section, same failure.

**Impact:** high — prevents shipping a fix whose justification is fictional, and the fix
that replaces it is better-grounded and cheaper to test. Two datapoints on one artifact
suggest the bug-file-as-hypothesis framing generalises.

**Promote-when:** already at 2 datapoints against the same artifact and 3 against
session-authored artifacts generally (F-34, F-35, F-37). Promote now to the reconnaissance
skill as a Phase 1 bullet: *"when re-entering your own bug file or plan to implement it,
re-scout the root cause — an artifact written during the observation is a hypothesis that
acquires the authority of a record the moment it is committed."*

**Status:** validated — fix landed on verified grounds; the entry it came from is now
`mitigated` with an honest not-reproduced note.

## F-38 — A verify-open pass filed a false narrowing by reading a compact summary as the result

**Observed:** 2026-07-28, re-running the reproduction in
`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` before
deciding between its two proposed fixes.

**When:** Earlier the same day, a verify-open pass on that bug file had narrowed its scope
and committed the narrowing.

**Expected (what the verify pass recorded, as a table, as fact):**

| call | `include_body=true` honored? |
|---|---|
| `symbols(path=<file>, include_body=true)` | yes |
| `symbols(path="src/peer", include_body=true)` | **no** — no bodies, no hint the flag was dropped |

**Got (re-run live):** 43 of 43 symbols carry a `body`, doc comments included. The flag is
honored. The pass had read the **compact summary** — a ~2 KB line-oriented preview that
cannot render a body at any size — and taken it for the whole result, while the full
result sat in a `@tool_*` buffer named in the same response.

**Probable cause:** the summary of a body-bearing overview and the summary of a body-less
one are the same shape, and at the time so were their hints (see W-30). But three signals
were available and none was consulted:

1. `buffered_bytes: 56915` in the same response object. 43 symbols of names and line
   numbers are not 56 KB.
2. The `hint` field naming the buffer, which is what the envelope exists to say.
3. `symbols_overview_honors_explicit_include_body_true`, a regression test added 07-19
   that asserts `files[0].symbols[0].body` on the **directory-scan** branch specifically,
   green the entire time.

`docs/PROGRESSIVE_DISCOVERABILITY.md` lists this by name under Anti-patterns — *"Treating the
summary as authoritative. It's a preview, not the whole result. Pull from the buffer before
drawing conclusions."* Knowing the rule did not help; the summary reads as an answer.

**Workaround:** retracted the narrowing in place, kept the wrong text verbatim under a
retraction header, and closed Bug A. Bug B (search-mode intermittent 0-matches) is
unaffected and is now the only reason the file is open.

**Severity:** med-high — the wrong finding was committed, and it *pointed the fix in the
opposite direction*. The file's own framing was "if the drop is a deliberate size guard,
the defect is the silence, and the fix is an overflow hint rather than honoring the flag."
Both branches of that sentence are premised on bodies being absent. Implementing either
would have added machinery around a flag that already worked.

**Status:** fixed-verified — retracted, Bug A closed, and the real adjacent defect fixed
(W-30) with two tests.

**Fix idea / Pointer:** fifth instance this session of one shape — concluding from a view
that had silently excluded something. Lock files filtered by a liveness check, then the
filtered list reused; `pgrep | tail -1` picking an orphan rather than the live server; a
SHA census matching frontmatter `id:` values as commits; `ls | wc -l` counting
`.anchors.toml` sidecars as memories; and this. See R-50 — *the view is not the set*.

## W-30 — Re-run the reproduction against the live tool before choosing between a bug file's fix options

**Observed:** 2026-07-28, deciding whether `symbols(path=<dir>, include_body=true)` should
honor the flag or emit an overflow hint — the two options its bug file laid out.

**Pattern:** When a bug file asks you to choose between fixes, **re-run its reproduction
first**, and read the *buffer*, not the summary. A tool call is cheap, current, and
authoritative in a way that both the file and the source read are not. Then let the byte
count arbitrate: an envelope reporting `buffered_bytes` far larger than the summary could
account for is telling you the summary is not the result.

Here the re-run collapsed the whole question. Neither option was needed — the flag was
already honored, 43/43 symbols with bodies — and both would have added machinery around
working behaviour.

**Counterfactual:** Without the re-run, the plausible path was: implement the overflow
hint the file recommended, ship it, close the bug — and thereby *document in code* that
directory overviews cannot carry bodies, which is false and which the 07-19 regression test
directly contradicts. A future reader would then have a hint, a closed bug file, and a
passing test making opposite claims, with the hint being the one they see at runtime.

The re-run also produced the fix that was actually needed, which no amount of reading
would have: with the real envelope in front of me, the missing signal was obvious.
`Symbols::json_path_hint` checked only the search-mode shape, so an overview carrying 50 KB
of bodies advertised `$.files` — byte-identical to one carrying none. For a buffered
result that hint is the only body signal in the envelope. Fixed to scan `files` for the
first entry with a body; two tests.

**Confirming data points:**
1. This session — re-running collapsed a two-option design decision into "no change
   needed", and surfaced the real defect one layer over.
2. W-29 (same session) — re-scouting a bug file's root cause before implementing found it
   unsupported, and likewise turned up a different real defect nearby. Same shape: the
   artifact was stale, the substrate was not.

**Impact:** high — one tool call replaced a design decision, prevented shipping a hint
asserting something false, and located the genuine defect.

**Promote-when:** at 2 datapoints with W-29. Both say the same thing from different angles:
**on re-entry, prefer the substrate over the artifact** — re-run the reproduction, re-scout
the root cause. Promote jointly to the reconnaissance skill rather than separately.

**Status:** validated.

## F-39 — Three bug files' `## Root cause` sections were falsified on contact, and one of them hid a security bypass

**Observed:** 2026-08-14, tranche-based sweep of the `docs/issues/` backlog. Each bug was
verify-opened before any fix.

**When:** Reading each bug's stated mechanism before implementing its Resume.

**Expected (bug files):** the `## Root cause` narrows the work.

**Got (scouted reality):** three of the eight worked reversed the work instead.

| Bug | Its claim | Reality |
|---|---|---|
| `db5fe933` state-protocol `.sh` | Resume warned *off* the buddy rows: "a different, legitimately-`.sh` component" | `ls buddy/hooks/` → no `session-start.sh` at all; `hooks.json:4` dispatches `node run.mjs session-start`. Scope was 4 rows; actual 9 |
| `b86d81f1` IL3 quoted pipe | "the caller already does the quote-aware thing via `pipeline_segments`" | `pipeline_segments` scanned raw bytes with **no quote state**. The same defect in two functions, one recorded as the other's solution |
| `a99388a2` edit_file ack | "the ack handle does not resolve" | Resolves in both call shapes. `151cc9df` (the fix) is an **ancestor of the SHA the bug cites**; no commits since. The ENOENT was the target file |

**Probable cause:** a bug file's Symptom is *observed*; its Root cause is usually
*inferred* and then written in the same declarative voice. Nothing marks the seam. Two of
the three even carried their own refutation: `a99388a2` recorded "the handle IS reaching
parameter validation" under Symptom and argued past it; `db5fe933` had left its
companion-side hypothesis `deferred` while stating the buddy-side equivalent as settled.

**Severity:** high — following `b86d81f1`'s premise would have fixed the false positive
and left the **bypass** in place: a quoted `;` before a real pipe hid it from the
enforcer entirely, measured at `exit_code: 0` on a `git log | head`. Following
`db5fe933` would have fixed 4 of 9 rows and closed the bug.

**Status:** fixed-verified — all three closed, each with the correction written into the
bug file rather than only into the commit.

**Fix idea / Pointer:** the template already distinguishes `measured` from `inferred` for
Root cause (added 2026-08-07, task #31). It is being used for the *mechanism* and not for
the *scope* or the *prescription*. Cheapest strengthening: require the Resume to name the
one command that would falsify it. All three here had one, and it was one line — `ls`,
`read pipeline_segments`, `merge-base --is-ancestor`.

## F-40 — I mis-tranched 3 of 10 bugs as "no design decision needed"

**Observed:** 2026-08-14, after triaging 32 non-terminal bug files into four tranches.

**When:** Building tranche B ("small code bugs, no design decision needed") from titles
plus a one-line skim, then discovering the decisions on contact.

**Got:** three of the ten needed an owner decision and moved to tranche C:

- `2faaf32a` sqlite-vec leak — the prescribed fix is a 14-call-site refactor, because the
  env read is three frames below the tests that trigger it.
- `7438a064` chunk-size dead code — needed a *measurement* (does fastembed truncate?)
  before the direction could be chosen, and the answer inverted it.
- `f95e6841` / others still unassessed.

**Probable cause:** the tranche was built from what the bug *reported*, and the decision
lives in what the fix *requires*. Two of the three said so in their own Resume
("decide, with the codebase owner"), which I had not read at triage time — titles and
severity only.

**Severity:** low — no wasted implementation; the cost was re-tranching mid-flow and one
report to the user revising its own estimate.

**Status:** mitigated — tranche C now carries them with the analysis done, which is worth
more than the original classification was.

**Fix idea / Pointer:** when triaging a backlog into effort classes, read each item's
`## Resume` — not its title. That is the field that names the blocked decision, and it is
one `artifact(get, headings=["## Resume"])` per item.

## W-31 — `references` answered a question a bug file scoped as "instrument and bisect"

**Observed:** 2026-08-14, `2faaf32a` (every `cargo test` leaks ~144 sqlite-vec DBs into
`$HOME`).

**Pattern:** when a bug asks *"which code path reaches X during the suite?"*, try
`references(X)` before instrumenting. A single production caller makes the question
static, not dynamic.

**Counterfactual:** the bug's Resume prescribed *"adding a temporary `tracing::warn!` on
the home-dir fallback branch and running the suite once"* — an edit, a full `cargo test`
(~2 min), log reading, then a revert. `references(SqliteVecCodeStore/from_env)` returned
**one** production caller (`client.rs:238`) in one call. Following that to
`VectorBackend::resolve` then explained the part the instrumentation would *not* have
shown: the leak is specific to the default lane because the default resolves to
`SqliteVec` under `cfg(not(feature = "server-stack"))`, which is also why the
server-stack lane looks clean.

**Confirming data points:**
1. This session — sole caller found statically; the instrumentation was never needed.
2. F-12 (2026-05-24), inverted: *dismissing* `references`' own "use `call_graph` for
   authoritative callers" warning shipped a half-fix.

**Impact:** med — saves a build+suite cycle per "who calls this at runtime" question, and
yields the *why* alongside the *where*.

**Promote-when:** a second bug file prescribes runtime instrumentation for a question
`references`/`call_graph` answers statically. At two datapoints, add to the recon skill's
Phase 1: "before instrumenting to find a runtime caller, check whether the static caller
set is size 1."

**Status:** validated.

## W-32 — Reproducing before fixing changed the fix three times in one session

**Observed:** 2026-08-14, across the whole tranche sweep.

**Pattern:** run the bug's own reproduction against the current tool *before* reading its
fix plan. Not to confirm the bug exists — to find out what the plan is actually about.

**Counterfactual, three times:**

1. `973f0c0f` — filed as "top-level `extra` dropped". Reproducing it, then probing
   `tags`/`topic`/`time_scope` on the same call, showed **five** params dropped by one
   mechanism. A per-field fix would have shipped the same defect a seventh time; the July
   fix for `status` alone is *why* this bug existed.
2. `037430886` — filed with `delete` "reparents, parses fine". Two reproductions showed it
   depends on the **neighbouring** key: scalar above → invalid YAML (loud); sequence above
   → `{'tags': ['alpha','a.md']}` (silent). Only the second justifies `severity: high`,
   and only reproduction distinguishes them.
3. `7438a064` — the fix direction *inverted*. Measuring fastembed's real ceiling (512
   tokens, uniform across models) showed the prescribed "wire the model-aware budget in"
   would over-chunk large-context local models against a hard cap. The dead code's
   deletion, not its promotion, was correct.

**Confirming data points:** the three above, plus W-30 (2026-07-28), which is the same
lesson at one datapoint.

**Impact:** high — in case 3 the plan-as-written would have introduced a bug; in case 1 it
would have left four siblings broken behind a passing test.

**Promote-when:** already at four datapoints with W-30. Promote to CLAUDE.md's bug-file
section: *"run the reproduction before reading the fix plan — the plan is a hypothesis
about the reproduction."*

**Status:** validated — promote next session.

## W-33 — Mutating each new regression test, and finding the pair discriminates

**Observed:** 2026-08-14, on all four code fixes that added guards.

**Pattern:** after a new test passes, break the fix it guards and confirm the test fails
— and check *which* assertions fire.

**Counterfactual:** three mutations, three catches, and the third taught something the
pass alone did not:

- removing the `tags` lift → `update_lifts_every_advertised_top_level_param` failed with
  the intended diagnostic. **Bonus:** it also tripped `field 'tags' is never read`, so
  under `-D warnings` a deleted lift fails two independent ways — but *no* warning catches
  a field that never existed, which is the original bug, so the struct now carries a
  comment against "cleaning up" an apparently-unused field.
- reverting `pipeline_segments` to `split(';')` → the bypass test failed **while the
  six-violation control test kept passing.** That is the pair working as designed: one arm
  catches under-blocking, the other over-blocking, and a mutation tripping only one is
  correctly diagnosed.
- forcing `read_edit_target`'s `NotFound` branch to `false` → the missing-target test
  failed on its *classification* assertion only; the path-naming assertion still passed,
  because the generic arm also names the path. Two assertions, two distinct properties,
  confirmed non-redundant.

**Impact:** med — cheap (one edit, one filtered test run, one revert) and it converts "the
test passes" into "the test can fail", which is the only version worth having. The
per-assertion detail is the part a plain pass/fail mutation check misses.

**Promote-when:** already project convention in spirit (memory `test-design-discipline`).
Worth adding the sharper form: *check which assertion fires, not just that one does.*

**Status:** validated.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
