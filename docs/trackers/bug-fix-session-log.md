---
kind: tracker
status: active
title: Session Log — Bug-Fix Work Stream
tags:
- session-log
- bug-fix
topic: Multi-session bug-fix work stream — frictions and wins from closing open buffer/markdown bugs in docs/issues/
time_scope: open-ended
entry_prefix:
- F
- W
entry_high_water_F: 72
entry_high_water_W: 71
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
| F-41 | 2026-08-14 | high | plan-prose | fixed-verified | A bug file's argument for **not** fixing was itself false: `c7ba92f5` called both cases "status-string-only" and reasoned that widening the shared classifier was "strictly riskier". The classifier had 4 consumers, 3 of them live; the live path was already wrong, so widening was the fix, not the risk |
| F-42 | 2026-08-14 | med | self-friction | fixed-verified | Two `ProjectStatus` tests are vacuous on this host — ambient `CODESCOUT_EMBED_*` vars trip their skip guards, so a mutation run reported a false PASS until re-run under `env -u` |
| F-43 | 2026-08-14 | med | self-friction | fixed-verified | `cargo run --bin codescout` builds with DEFAULT features, which exclude `server-stack` — so `migrate-memories --in-place` resolved the sqlite-vec lite store and reported `read: 0`. Nearly reported as "this project has no semantic memories"; one `memory(recall)` call refuted it, and `cargo rb` showed `read: 15` |
| F-50 | 2026-08-15 | **high** | tooling | fixed-verified | Three unrelated tools answered about a subset while looking like they answered about the whole — a markdown auditor pointed at Rust (7 vs the real 95), a grep capped at 50 whose footer read "Showing 50 of 50" (vs 266), and a `head -8` buffer I grepped and reported as a corpus. Instance 3 put a false claim into a shipped commit body |
| F-51 | 2026-08-16 | med | architectural | fixed-verified | Shipped a process-global whose semantics contradict the project's only precedent for one — `OnceLock` (first-writer-wins) where `heartbeat.rs` had already chosen last-writer-wins on purpose; "one catalog per process" was verified in production and never in tests, where three helpers build a server each |
| F-49 | 2026-08-15 | **high** | self-friction | fixed-verified | A fix I shipped and archived as done was **inert on the transport the project actually uses** — its end-to-end test drove the one transport where the defect cannot appear, and live verification on the rebuilt server reproduced the original bug unchanged |
| F-47 | 2026-08-15 | med | self-friction | fixed-verified | Implemented a guard exemption, shipped it, and reverted it within the hour — the bug file described the refusal as an oversight, and it had both a dedicated test and purpose-built hint machinery, each one `grep` away |
| F-48 | 2026-08-15 | med | tooling | fixed-verified | A name-filtered `cargo test` gave false confidence: `--lib guard_` did not match `edit_file_warns_hint_suggests_remove_when_new_empty`, so the change that broke it passed its own targeted run and was committed |
| F-46 | 2026-08-15 | med-high | self-friction | fixed-verified | An existing integration test asserted `msg.contains("AST parse failed")` on a **syntactically perfect** fixture — it had been corroborating a false explanation of its own scenario since it was written, and made correcting the message look like a regression |
| F-45 | 2026-08-15 | low | codescout-tool | wontfix-by-design | The `edit_file` string-literal residual I documented an hour earlier bit me writing the very next hint: prose containing `impl ` inside a string literal was refused. Cost one rephrase. Evidence FOR the deliberate over-block, not against it |
| F-44 | 2026-08-14 | med | codescout-tool | fixed-verified (half) | `edit_file` and `edit_code` deadlock on "add a method to a trait impl nested in a test fn". `edit_file` half **fixed** in `138de7c5` (unanchored per-line keyword match — `via_trait ` matched `trait `); `edit_code` half open. **Two of my own claims in this entry were overstated and are corrected in its body** |
| F-53 | 2026-08-18 | med | architectural | fixed-verified | Proposed a PPID-derived session key without enumerating client session-resume, which restarts the parent while the conversation continues — refuting evidence was already in hand and unread |
| F-52 | 2026-08-18 | med | architectural | open | `Agent::project_root()` returns the FOCUSED SUB-PROJECT root, not the workspace root — wrong comparand for a "did the project change?" predicate |

| F-54 | 2026-08-18 | med | process | mitigated | A dispatched subagent's full-suite run read a *concurrent* session's mid-edit working tree, saw a prompt-surface snapshot test fail, and reported it as a live failure — the shared-checkout analogue of the `src/prompts/README.md` shared-branch verify hazard |
| F-55 | 2026-08-18 | med | codescout-tool | promoted-to-bug-tracker | `grep(glob=<abs path outside the project>)` returns `0 matches` instead of an error, and the warning blames hidden-path pruning — a cause it never checked, whose suggested fix cannot help. `path=` on the same file matches fine |
| F-56 | 2026-08-18 | med | process | wontfix-false-alarm | Ran the pre-promotion gate from a clone under `/tmp` and got a **false red** — three librarian temp-write-guard tests invert there, and cargo's fail-fast then skipped 21 other test binaries, so the run was simultaneously falsely negative and silently incomplete |
| F-57 | 2026-08-21 | med | process | mitigated | A concurrent Claude Code session's own commit swept up this session's uncommitted `resolver.rs`/`severity.rs` edits under an unrelated spec-work message — content verified intact, but no single clean commit exists to cite as the fix; same class as F-54 |
| F-58 | 2026-08-21 | low | plan-prose | mitigated | A same-day concurrent commit made a bug's own prescribed fix wrong before I got to it — `link_scan` gained a real `entry_cite` write path mid-day, inverting the bug's root cause |
| F-59 | 2026-08-25 | med | codescout-tool | fixed-verified | A filed bug's own root cause was wrong ("desktop-only remote host"); reproduction found a stale global symlink instead |
| F-60 | 2026-08-25 | low | cross-session | mitigated | Even after ListAgents/SendMessage coordination, a peer's routine commit silently absorbed this session's uncommitted append_entry writes |
| F-61 | 2026-08-26 | med | process | fixed-verified | Asserted "no live-pyright harness exists" from grepping ONE file, and nearly shipped an `unverified:` marker declaring the test impossible; `tests/e2e/` already held six live-LSP `find_referencing_symbols` expectations |
| F-63 | 2026-08-26 | med | process | fixed-verified | Reported "2 open bugs" four times from a query whose filter was the thing hiding the pile — 23 files in `docs/issues/`, 2 open, 17 terminal-unarchived, 4 zombie that no query in the project reaches |
| F-62 | 2026-08-26 | high | process | fixed-verified | All five feature-gated e2e language lanes had stopped compiling — `ToolContext` grew a field, the shared harness never got it — while `cargo test` stayed green, since it never builds a feature-gated target no lane names; recurrence guard shipped in `3784cb65` |
| F-64 | 2026-08-26 | med | plan-prose | fixed-verified | My own #18 bug file inflated its own fix — `code_vec` is per-project by FILE, so the "most likely to be got wrong" step is vacuous and its regression test would have passed for every implementation |
| F-65 | 2026-08-26 | low | skill-doc | open | The reconnaissance skill's worked exemplars show a `**Valid:**` form the server refuses (13-vs-0 in this tracker), and instruct you to copy them |
| F-67 | 2026-08-26 | med | self-friction | validated | Two tree-wide writes swallowed a peer's in-flight work — `cargo fmt` and `git add <dir>` |
| F-66 | 2026-08-26 | high | process | open | Quoted the substrate law at the user, then measured a retired sqlite store all session — backend is Qdrant, `codescout.db` untouched since Aug 25; five published figures described a dead world, and a passing positive control validated the instrument against the wrong database |
| F-69 | 2026-08-26 | med | process | fixed-verified | Bug-file citations written at a future `archive/` path schedule no sweep — the archive flow repairs citations that *were* correct, never one wrong the day it was written |
| F-72 | 2026-08-27 | high | self-friction | validated | Writing an honest caveat discharges the obligation to close it — two sessions, forty minutes apart, each named the exact missing check in prose and then stopped. The tell: a caveat that names a specific next action is a work item, not a deliverable. Mechanism behind [[R-95]], seen from inside |
| F-71 | 2026-08-26 | med | self-friction | fixed-verified | Three claims published from instruments that could not discriminate the hypothesis from its alternative — and the retraction of one lost a race to a peer's committed ledger entry |
| F-70 | 2026-08-26 | med | process | fixed-verified | A dead citation that was wrong when written is indistinguishable from one that decayed — 0 of 6 in a 321-citation sweep were decay, and three independent artifacts misread it from the prose; the `--diff-filter=AD` probe is the only discriminator |

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
| W-42 | 2026-08-15 | **high** | When a finding is "we cannot see class X of drift", build the seeing BEFORE finishing the sweep | SD-1 found 95 stale citations and 3 residuals needing judgment; SD-1b built the gate in between; SD-8's verification was then the gate reporting `verdict=resolved`, not me re-reading comments — the same eyeballing that let 95 rot. It also found drift outside the filed backlog, and caught three self-inflicted refs in the tracker prose describing the work | validated |
| W-41 | 2026-08-15 | **high** | Live-verify on the running server after every rebuild, even when the suite is green and the bug is already archived | The stale-symbols fix passed a real-rust-analyzer end-to-end test, cleared a 3814-test gate, was documented, archived, and reported as verified. It did nothing. The post-`/mcp` probe — external write, then `symbols()` with no flush — took under a minute and reproduced the original bug exactly. Without it, a closed bug file, a CHANGELOG entry and a promotion to master would all have asserted a fix that was not there | validated |
| W-39 | 2026-08-15 | high | Reproduce before trusting the filing — measured across a whole backlog | Fourteen bugs closed in one drive; **eleven had a filing whose reasoning was weaker than its own code**. Not wrong about the symptom — wrong about cause, scope, or what was already true. Three recurring shapes: absence claims ("no test discriminates", "no workaround exists", "reproduces only on Windows"), count-vs-category generalisation, and false comments about a peer component. Trusting any one filing's fix section would have produced the wrong change | validated |
| W-40 | 2026-08-15 | med-high | Time the subject against a trivial control on the same connection | A ~990 ms `semantic_search` was filed with ~950 ms unaccounted. Measuring it alone would have produced another bare number; measuring it beside `tools/list` (1 ms) and `tree depth=1` (39.5 ms) over one MCP session turned it into a breakdown — and showed the headline finding, that the figure is now **127 ms** and its largest identified slice is per-call dispatch shared by every tool | validated |
| W-38 | 2026-08-15 | high | Separate a safety guard's REFUSAL from its DIAGNOSIS | `insert position="after"` looked broken. Split three ways it wasn't: the refusal is correct (BUG-051, would splice mid-body), the *message* was the defect (asserted "AST parse failed" on files that parse), and the real limit is an LSP/AST asymmetry belonging to the extractor. Treating it as one bug would have reopened a closed corruption bug. Also: the codebase already had `has_syntax_errors` sitting next to an error that *guessed* about syntax errors | validated |
| W-37 | 2026-08-14 | high | Mutation-test the TEST, not only the code | The first version of the prefix regression test exercised the inherent `dense_query`/`dense_document` and would have passed with the trait method wired to `dense_query` — which is precisely where the defect lived. Mutation exposed the wrong-layer coverage; the fixed test then failed with mockito reporting the QUERY mock hit twice | validated |
| W-36 | 2026-08-14 | high | Refuse a default trait impl and the compiler becomes your enumerator | Adding `embed_document`/`embed_document_one` with NO default made `cargo check` list all five implementors, exactly matching the prediction. A default would have compiled silently and left every implementor free to keep inheriting the query method — i.e. the bug. The repo already had this precedent in `known_dim`'s doc comment | validated |
| W-35 | 2026-08-14 | med | A prompt-surface byte cap is a design reviewer | `every_tool_description_under_cap` rejected the doctor change twice (1960, then 1846 vs an 1800 cap) and was right both times: parameter detail belongs in the uncapped parameter descriptions, not the action list. Also revealed the `librarian` description sits at ~1790/1800 — 99.4% of budget | validated |
| W-34 | 2026-08-14 | high | Mutation-test a SHARED helper before converting its call sites | Building `FenceState` and mutating it twice, before touching any of 7 boolean sites, showed the run-length rule alone does NOT reject the line that triggered the report — the backtick info-string rule does. Without that run I would have shipped the "obvious" one-rule fix and left the actual trigger unfixed across all 7 | validated |
| W-33 | 2026-08-14 | med | Mutate each new guard and check WHICH assertion fires | 3 mutations, 3 catches; one showed the control test correctly still passing (under- vs over-blocking arms), another showed two assertions cover distinct properties rather than restating one. Also surfaced that no warning catches a struct field that never existed | validated |
| W-44 | 2026-08-16 | **high** | A sample from one language is not a sample of the corpus | Rust-only measurement said 56% of ref findings were dotted-identifier noise; the obvious fix was to suppress them. Every non-Rust file in the repo measured 5%. The same token is a Rust field AND a Python module — suppressing it globally would have deleted real refs from five languages while IMPROVING the metric that motivated it, since those refs were already `unknown`. Built `PathSyntax` instead; verified live: Rust unknown 56% → 0%, non-Rust byte-identical, resolved slightly up | validated |
| W-43 | 2026-08-15 | **high** | A backlog row's instruction can outlive its own hypothesis; run the instruction | SD-3 claimed four over-budget `::call` handlers should give up a shared extraction, and in the same field admitted the shape was UNVERIFIED — read the other three first. Reading falsified it (two pairs, on a different axis) and found the duplication that IS there: a scope-resolution prologue written three times, one copy drifted, whose live effect is `librarian(context, scope="all")` crossing the umbrella boundary. Acting on the hypothesis would have refactored ~1460 lines toward a shape that does not exist, and buried two of the three real sites | validated |
| W-45 | 2026-08-18 | med-high | Scout the change's own regression test AND the prose surface that documents the policy | `activate_project_resets_hints` (`server.rs:4292`) pins the exact policy being replaced, and activates the SAME root `make_server()` built the agent with — it inverts under the new policy and would have read as "your change is wrong" rather than "this test encodes the policy you replaced". Separately `workspace-state.md:51` documents the old mechanism and is `include_str!`'d into every session's context, so the code change alone ships a falsehood in an always-injected guide | validated |

| W-46 | 2026-08-18 | high | Before rehoming state from per-project to per-user, scout whether the tests' hermeticity *depended* on the per-project path | Plan's verbatim code would have made `guide_ledger_survives_mcp_restart` pass on the first `cargo test` and fail on every run after — a self-poisoning test — and pointed a 35-day GC at the developer's real `~/.local/state` | validated |
| W-47 | 2026-08-18 | med-high | Walk the live process tree before designing against process topology | Rendezvous entries would likely have been published from a shared path, giving 2 `codescout mux` processes a `<pid>.json` no hook can ever match — permanent stale entries in a brand-new directory; also supplied the GC sizing (25 live servers, 22 `claude`) the spec omits | validated |

| W-48 | 2026-08-18 | med-high | When a change invalidates a *rationale*, sweep the class corpus-wide and ask **by what method** the enumeration was done — a count is not a method | An Opus review that left zero surviving mutants in the code still listed only **5 of 11** doc sites naming `is_empty()` as the opener trigger; a targeted read found a 6th, a corpus sweep found 5 more. Six void-or-stale rationales would have survived in the exact files the next task's implementer reads | validated |
| W-49 | 2026-08-24 | high | Checking ListAgents/SendMessage before committing shared uncommitted docs avoided a collision with two concurrent peer sessions | Two files vanished from `git status` mid-session as peer sessions (`codescout-0e`, `codescout-ee`) committed them concurrently; asking first also surfaced a real `.gitignore` gap this session had missed | validated |
| W-50 | 2026-08-26 | high | Run recon at the commit boundary of your own fix, aimed at the claims you are about to write into the durable record rather than at the code | Would have shipped the `references` alias fix with zero automated coverage plus an `unverified:` marker asserting coverage was impossible; the scout instead produced a measured red→green (24/25 → 25/25) and found five e2e lanes that had silently stopped compiling | validated |
| W-52 | 2026-08-26 | high | Measure the corpus before building a detector — run its predicate over the whole population and name the true/false split | The obvious `[LIVE]`-body heuristic would have fired on 23 unaugmented artifacts of which 1 was defective; a 23:1 rot-detector gets switched off or learned-around, and the count cost ~40 seconds | validated |
| W-53 | 2026-08-26 | med | Verify a scan's FIRST finding against reality before acting on the worklist, and treat a false one as a bug in the instrument | The single archived_fix_sha finding was a false positive; checking it found two defects in the same parser, against an alternative of hunting a dead commit the check itself sizes at 2-153 candidates | validated |
| W-51 | 2026-08-26 | high | When a filed root cause is about persisted state, reproduce it by querying the datastore — not by re-running the tool that reported it | The filed "restore every augmentation" fix, run against the 21 trackers that were already augmented, replaces their params wholesale — the same call that took the T-N queue from 19 entries to 1 on 2026-08-16 — causing the loss it claimed to repair | validated |
| W-54 | 2026-08-26 | med | Before implementing from a bug file or plan YOU wrote, scout the seam its Fix section SIZES — not only the seam its Root cause cites | Would have implemented a project-scoped `DELETE` plus a "sibling project survives" test for an invariant the per-project-file schema already guarantees; that test passes for every possible implementation, including a wrong one, so it enters the suite as a permanent green-but-uninformative assertion | validated |
| W-57 | 2026-08-26 | high | Call the tool live on the real backend before shipping tool-facing OUTPUT — a fixture built from a bug report's quoted error string is NOT a live check | `67c548b9`'s payload-size hint matched the wording quoted in the issue (`exceed_context_size`, HTTP 400); the running server emits HTTP 500 `input is too large to process` from llama.cpp's n_batch path, so the arm was dead on the very stack it was written for — 3 tests, clippy clean, 4477 green, and an operator would have got the generic hint instead of "retrying will not help" | validated |
| W-55 | 2026-08-26 | high | Before trusting a diagnostic, confirm the serving process is not running a DELETED binary — `ls -l /proc/<pid>/exe`, with the pid found by walking `ps -o ppid=` up from a `run_command` shell | A peer session's `cargo rb` unlinked this session's server binary 70 minutes after it started; `doctor` then reported a check as broken that had been fixed 80 minutes earlier, and the next step was reopening a closed bug to repair a guard that works. Unlike R-89 the rebuild was someone else's, so commit, tree and binary mtime all read green and nothing in the transcript hinted at it | validated |
| W-56 | 2026-08-26 | high | EXECUTE a bug file's archive precondition rather than asserting it is met — it is a check someone deliberately deferred, so the cheapest disposal is also the only silent failure | "Confirm CI, then archive" read as boilerplate from a stricter era. CI had been red on every Test lane since 2026-08-19 — 3 OSes × 3 feature sets — on one test that resolves an embedder from ambient config; the mandated local gate runs in the developer's environment by construction and can never show it. Cost two gh calls; the alternative was archiving six files under a green that was true only on this machine | validated |
| W-58 | 2026-08-26 | med-high | Read the live envelope AND its formatter in ONE pass — the live response is a census of the keys that path emits, so any `Option`-guarded reader of a key absent from it is dead code | `format_index_status`'s model and timestamp arms have had no producer anywhere in the repo since `79e0e4f2` (2026-05-13, whose own message lists them as dropped). The test covering them hand-builds the response it asserts on, so it stayed green for 3.5 months across 4494 tests — and `docs/manual/src/troubleshooting.md:231` still tells users to compare `configured_model` against `indexed_with_model` to diagnose the exact model mismatch #18 was about, when `configured_model` has never been emitted by any commit in this repo's history | validated |
| W-59 | 2026-08-26 | high | "Cannot be separated" / "only CI can see this" is a claim about the SUBSTRATE, not the defect — price changing the substrate before accepting the limit | wine failed 9 guide_hint tests, MSVC 1, and the bug file recorded "whether the ninth is real cannot be separated from this run". Extracting PortableGit with `7z x` (a self-extracting archive — no installer runs) and pointing CODESCOUT_BASH at its Windows path took the module 9 → exactly the 1 MSVC had → 0. Cost ~5 min and 56 MB. The ninth was the ONLY production defect among all six Windows causes and 44 tests — noise is where a real signal is cheapest to lose | validated |
| W-60 | 2026-08-26 | high | Fix the failure MESSAGE before the failure, when the message would read the same in more than one broken world — on a slow or unreproducible surface it is the cheapest next measurement, not a detour | The wine lane's last red printed a bare empty string, and the test's name, doc comment and linked bug file all said *heredoc pipe-rewrite corruption* — which produces exactly that. One edit asserting the write's exit code, one CI run, and the payload said `{"timed_out":true,...}`: a hang, not corruption. The alternative was re-opening a closed masking bug on a platform with no local repro at ~7 min per round trip. Mirror of R-79 for reds; the same test carried both defects | validated |
| W-61 | 2026-08-26 | med-high | A tool response read through a harness is evidence about the HARNESS's encoding, not the callee's signature — make a new assertion fail on purpose before trusting the belief it encodes | Writing that guard I also wrote "`call` returns Ok for a gate refusal", inferred from the `{"ok": false, "error": ...}` every refused `run_command` had returned all session. The positive control did not fire: that JSON is MCP serializing a real `Err`, which `.expect` catches. A false claim about `Tool::call`'s error contract would have shipped in the comment most likely to be consulted next — and it is not checkable by reading the test, only by running the broken world. R-89's rendered-read law at the RPC boundary | validated |
| W-63 | 2026-08-26 | high | A regex over source and a structured tool are DIFFERENT INSTRUMENTS — the regex over-counts by exactly the fixtures, every time | Counting rotted `docs/issues/` citations for a severity-policy decision, grep said 77 (truth 55) and then 22 (truth 7); `audit_doc_refs` reads tree-sitter documentation nodes and has a test pinning that a path in a string literal is NOT a citation. The second wrong number was committed into a bug file as fact and had to be corrected — a figure a later session inherits without re-deriving. Survivable only because the filter was conservative BY CONSTRUCTION (a placeholder has no archived twin), which let a 30-file automated rewrite run with zero hand decisions | validated |
| W-64 | 2026-08-26 | high | Bulk-removing a suppression list is the cheapest audit of it — once a GREEN baseline makes the red unambiguous | The windows-gnu `--skip` list had grown to 32 write-only entries. Dropping 22 in one commit passed 21 of 22 on CI, and the 1 failure shared a cause with an entry already listed; +WINEPATH retired 3 more. 32 → 8, every survivor classified, and CI's only gnu-ABI lane went 4251 → 4293 tests. My by-hand arithmetic on the same list was wrong (19 vs 22, tripped by one entry matching four test names by substring). The blocking deferral was correct when written and expired when the lane went green — R-95 | validated |
| W-65 | 2026-08-26 | high | When a result is IMPOSSIBLE rather than merely wrong, suspect the RUN, not the code | A failing payload carried 2 of 4 keys that output.rs sets in ONE `if let` block; grep confirmed a single producer, so no branch could emit it. Re-running unchanged: 4293/0, twice. I was two steps from filing a platform defect against a peer's hour-old feature and "fixing" correct production code — with a wine Cygwin warning in the payload ready to anchor the wrong diagnosis. Wrong values mean wrong logic; impossible combinations mean a wrong observation, and that second hunt is invisible if you start from "which line computed this?" | validated |
| W-66 | 2026-08-26 | med | RED-before-GREEN on a bug's own prescribed fix, not just "bare vs. my fix" | Would have shipped a fix that changed nothing and closed a live defect | validated |
| W-67 | 2026-08-26 | low-med | Measured the 3 live zombie records before picking doc-fix vs. doctor-check | Would have built a staleness detector for a population with no measured neglect | validated |
| W-71 | 2026-08-26 | high | Replace the question, don't tighten the answer — a predicate needing an exhaustive population fails silently (which processes are stale / which commits are the peer's); one whose subject is self has a population of one, so completeness is not a precondition. Corollary: each session publishes its own list; nobody derives everyone's | Instance 1 would have left the oldest stale server running — the same process that then needed `-9` and is the clearest pre-fix datapoint for `ca2b0226`. Instance 2 would have credited the wrong session for the `EMBED_API_KEY` test repair, the [[F-71]] shape again. Note [[W-70]] described this class 40 min before I repeated it: a lesson requiring you to notice you are guessing is weaker than one removing the guess from the procedure (R-49's temporal-not-attentional argument, from the other side) | validated |
| W-70 | 2026-08-26 | high | Publish the retraction to the peer immediately even when you cannot fix it — two accidental sweeps of the same ledger, four minutes apart in opposite directions, both harmless solely because each session was told within ~3 min | Sweep 2 (`66671bf5`) happened with `git diff -- <file>` honestly run and PASSED; the peer's write landed between the diff and the add, so the guard narrows the race and does not close it. The only thing that prevented rather than repaired was a wire handshake — "file is clear as of `<sha>`, write freely". Absent disclosure, a session whose entry vanished re-runs `append_entry`, allocating a fresh id for content already in HEAD: two ids, divergent bodies, nothing validating for duplication | validated |
| W-69 | 2026-08-26 | high | Explicit-path staging + re-read status per commit, under three concurrent sessions — **bounded**: it discriminates FILES, so it saved a peer's `src/memory/*` and did nothing when a peer appended to this same ledger 3 min later (`02d80963` swept their F-71). The covering check is `git diff -- <path>` before `git add <path>` | Four peer commits landed inside one 3.5-min window between two of mine; `src/memory/*.rs` sat dirty as a third session's in-flight fix for a live bug, and one `git add -A` would have committed it inside a docs-only commit. (Corrected: the entry originally named that session; the name was an inherited guess and is struck — git metadata cannot attribute a commit to a session here, every commit carrying the same author and committer.) F-67 records that exact loss already happening once. Rule 3 is `codescout-77`'s: `git status` and `git diff` are not two readings of one world when a peer commits between them — they read a race as a stat-cache no-op and retracted it | validated |
| W-68 | 2026-08-26 | high | A bug's own root-cause claim ("no SIGTERM handler") was false — verified by reading the code before implementing the prescribed fix | Would have shipped a no-op fix and left an unbounded LSP-shutdown await masking a correctly-delivered signal, undocumented | validated |
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

**Status:** fixed-verified — the plan edit landed in the same 2026-05-18 session; [[W-2]] is
the win that records it.

**Corrected 2026-08-20 by the verify-open sweep.** The line read *"open → fixed-verified
after plan edit lands this turn"* for **94 days** — a conditional status whose condition was
met hours after it was written, with nothing to notice. It was accurate when written and
false by the end of that afternoon.

The reconnaissance `SKILL.md` quotes this entry as its **worked F-N exemplar**, with
`**Status:** fixed-verified — plan edit landed before any subagent ran.` (verified present in
the served `1.16.13` cache). So teaching material shipped to all three profiles disagreed
with the entry it was quoting, and the skill was the one that was right.

Third datapoint for a promotable rule: **future-tense prose in a disposition field ages into
a false open.** Siblings [[F-17]] (*"(open; … planned)"*) and [[F-44]] (*"Both should be
updated"*), both found the same day by D11's first sweep.
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

**Promoted-to:** `CLAUDE.md` (codescout project) § Ad-Hoc Session Logs — the "Verify-open
cadence" rule. D11-verified 2026-08-20: phrase present, 1 occurrence.

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

**Promoted-to:** `docs/issues/archive/2026-06-11-lsp-tools-ignore-workspace-pin-path.md`
— D11-verified 2026-08-20: file present, frontmatter `status: fixed`.

**Status:** promoted-to-bug-tracker — `docs/issues/archive/2026-06-11-lsp-tools-ignore-workspace-pin-path.md`. **Repointed 2026-08-20 by D11's first sweep:** the parenthetical read *"(open; 4-site swap + 3 per-tool regression tests planned)"* while the destination has been `status: fixed` and archived. A stale plan reading as an open one — the exact shape [[W-7]]'s verify-open cadence exists for, one level up.

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

**When:** About to implement `docs/issues/archive/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`.

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

**Fix idea / Pointer:** `docs/issues/archive/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`.
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

**Promoted-to:** `CLAUDE.md` (codescout project) § Bug Tracking. D11-verified 2026-08-20:
the quoted opening clause is present, 1 occurrence.

**Status:** promoted-to-permanent-docs — landed 2026-08-20 in `CLAUDE.md` § Bug Tracking.
Promoted text, verbatim, so a later reader can verify without re-deriving it: *"Run the
reproduction before reading the fix plan — the plan is a hypothesis about the reproduction."*
All three counterfactuals carried across with it. ("promote next session" stood unharvested
for six days; found by the sweep in `prompt-surface-compaction-session-log:F-7`.)

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

## F-41 — A bug file's argument for NOT fixing was itself false, and it suppressed a live-path fix

**Observed:** 2026-08-14, working `docs/issues/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md` (`c7ba92f5`) from the tranche-B backlog.

**When:** Before writing any code, reading the file's `## Root cause` — which contained a subsection titled *"Why the obvious fix is wrong"*.

**Expected (the file):** Two configurations get the wrong `embedding_backend` string. *"Both are status-string-only: the actual embedder construction … goes through `create_embedder_with_config` directly, never through `backend_is_local`."* Therefore widening `backend_is_local` "changes what those two functions decide for real queries — strictly riskier than a status-string residual", and the file existed "to record the residual rather than 'fix' it".

**Got (measured):** `references(symbol="backend_is_local")` returns **four** consumers, not one: `guard_sparse`, `dense_only`, `resolve_model_dim`, and `ProjectStatus`. Three are live behaviour. And `build_embedder` reads the predicate **three lines above** the construction the file said was unaffected:

```rust
Self::guard_sparse(config, lite)?;                // reads backend_is_local
let dense_only = Self::dense_only(config, lite);  // reads backend_is_local
…
codescout_embed::create_embedder_with_config(…)   // arm 6 -> LocalEmbedder
```

So a bare-name local config composed a **hybrid query against an embedder that emits no sparse vector** — precisely the "degraded recall that never shows as a failure" that `guard_sparse`'s own doc comment says it exists to make loud. The guard could not fire because the predicate it consults did not recognise the config.

**Probable cause:** The filing traced one consumer of a shared predicate (the status tool it was reporting) and generalised to "status-string-only". The claim it made — construction does not consult `backend_is_local` — is *true*; it just is not the claim that matters. Query *composition* consults it, in the same function.

**Severity:** high. Not for the size of the fix (one predicate) but for the shape of the error: the false premise had been written into the file **as a reason not to fix**, complete with a persuasive risk argument. A residual labelled "deliberately unfixed, and here is why" gets re-read as settled and skipped. Every later verify-open pass would have skipped it.

**Relationship to F-39:** F-39 (same day) recorded three bug files whose `## Root cause` was falsified on contact. This is the same family one level up — the falsified claim is the file's **fix rationale**, not its mechanism. That is the higher-leverage error, because a wrong root cause produces a wrong fix, while a wrong rationale produces *no* fix. Captured as `R-80` in `docs/trackers/reconnaissance-patterns.md`.

**Status:** fixed-verified — widened in `5b7536f5`, 5 new tests, mutation-verified, gate green in all three CI feature configs.

**Fix idea / Pointer:** `docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md` § Root cause → *"The filing's own reasoning was falsified"*.

## F-42 — Two status tests are vacuous on this host, and a mutation run reported a false PASS

**Observed:** 2026-08-14, mutation-testing the case-1 fix for `c7ba92f5`.

**When:** Reverting `model_names_local_backend`'s new branch to prove the two new tests discriminate. The `selection_tests` one failed as predicted. `status_reports_local_onnx_for_an_urlless_bare_model_name` **passed** — under a mutation that should have made it fail.

**Got:** Both `ProjectStatus` status tests open with ambient-env skip guards that `return` early when `CODESCOUT_EMBEDDER_MODEL` / `CODESCOUT_EMBED_MODEL` / an embedder url is set, because those override the `project.toml` the test just wrote. This host sets three of them:

```
CODESCOUT_EMBED_URL=http://127.0.0.1:48081/v1
CODESCOUT_EMBED_MODEL=CodeRankEmbed
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081
```

So the test — and its pre-existing I1 sibling, which has carried the same guard since it was written — assert nothing here. Re-running under `env -u CODESCOUT_EMBED_MODEL -u … ` failed them both with the expected `got: String("remote-http")`.

**Probable cause:** The guards are deliberate and documented ("skip rather than assert a premise that isn't true here") and correct in intent — the alternative is a test that fails for an unrelated reason. The defect is that the skip is announced only through `eprintln!`, which `cargo test` hides unless `--nocapture`. A skipped test and a passing test are indistinguishable in the output an author actually reads.

**Severity:** med. No production impact, but it silently voided a mutation check — the one procedure whose entire purpose is to prove a test can fail. Anyone developing these tests on a configured host gets a green suite that proves nothing.

**Workaround (used):** run the mutation under `env -u` for each of the four vars. Recorded in the bug file's § Tests added so the next session does not have to rediscover it.

**Status:** fixed-verified as a *finding* — discrimination was proved. The underlying test-design issue is unfixed and is the real follow-up: resolve the effective config at the edge and pass it in (option A of `docs/conventions/test-env-isolation.md`) rather than depending on ambient env, which would remove the need for the guard entirely.

**Fix idea / Pointer:** `src/tools/config/tests.rs` — `status_reports_local_onnx_for_an_urlless_bare_model_name` and `status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`. See also `docs/conventions/test-env-isolation.md`.

## W-34 — Mutation-test a shared helper BEFORE converting its call sites

**Observed:** 2026-08-14, fixing `docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md` — a boolean fence-tracker defect replicated across seven line-oriented markdown scanners.

**Pattern:** When the fix is "extract a shared helper, then convert N call sites", write the helper with its own tests and **mutate it** before touching any call site. The helper is where the actual rule lives; the call sites only inherit it.

**Counterfactual — and it is not hypothetical.** The bug file proposed one rule: *"store the opening fence's character and run length, and close only on a run of the same character that is at least as long, per CommonMark."* That is correct and insufficient. Mutating the helper showed why: the line that **actually triggered the report** is

```
```` ```markdown ````
```

whose leading run is **four** backticks — so it satisfies `run >= open_run` and the run-length rule does not reject it. What rejects it is CommonMark's *other* two constraints: a closer may be followed only by whitespace, and a backtick fence's info string may not contain a backtick. Dropping the info-string rule failed exactly one test with `left: []` — the whole document swallowed as code.

Had I implemented the file's single proposed rule and converted seven sites, all seven would still have mis-parsed the reported input, and the surface tests (written from the same fixture) would have been the only thing to catch it — after seven conversions instead of before any.

**Confirming data points:**
1. This session — two mutations on `FenceState`, each failing a disjoint, predicted test set (3 tests / 1 test) with the right assertion messages.
2. W-33 (same day) — the sibling pattern at the *guard* level; this is the same discipline applied to a shared *helper*, where the leverage is N× higher.

**Impact:** high. The multiplier is the number of call sites: a wrong helper is wrong everywhere at once, and "all the call sites agree" reads as corroboration.

**Promote-when:** a second shared-helper extraction where mutation-testing the helper changes the rule before conversion. At two datapoints, promote to the recon SKILL as: *"When the fix is extract-a-helper-then-convert-N-sites, mutate the helper before converting anything — a wrong rule is wrong N times and the call sites cannot dissent."*

**Status:** validated — single strong datapoint where the mutation demonstrably changed the rule.

## W-35 — A prompt-surface byte cap acted as a design reviewer

**Observed:** 2026-08-14, adding `limit`/`offset` to `librarian(action="doctor")` for `docs/issues/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md`.

**Pattern:** `server::tests::every_tool_description_under_cap` (`src/server.rs:2128`) enforces an 1800-char budget per tool description. It rejected the change **twice** — at 1960, then at 1846 after I trimmed — and both rejections were correct on the merits, not merely on bytes.

**Counterfactual:** my first instinct was to describe the new windowing in the tool's action list, where every agent pays for it on every call. The cap forced the question "does this belong here?", and the answer was no: parameter semantics belong in the (uncapped) `limit` / `offset` parameter descriptions and in the runtime overflow hint, which is where `docs/PROGRESSIVE_DISCOVERABILITY.md` § Pattern 1 requires the parameter to be named anyway. The final change carries the information in two better places and zero bytes of shared budget.

**Confirming data points:**
1. This session — two rejections, ending in a strictly better placement.
2. F-20 (2026-06-12) — the same gate caught a 329-char `get_guide` description against a 300 cap, but there the lesson recorded was "run the broader test target"; that it *improved the design* went unrecorded.

**Incidental finding worth carrying:** the `librarian` description sits at **~1790 of 1800 chars — 99.4% of budget**. The next action added to that tool trips this gate with no room to trim. Not a defect today; a known cliff.

**Impact:** med. The gate is cheap, already exists, and reframes a byte limit as an editorial one.

**Promote-when:** a third instance where the cap redirects content to a better surface rather than merely shortening it. Then note in `src/prompts/README.md` that the cap is a placement test, not just a size test.

**Status:** validated.

## F-43 — A default-features build reported an empty memory corpus, and it nearly became a finding

**Observed:** 2026-08-14, live-verifying `codescout migrate-memories --in-place` (the repair half of the memory query-prefix bug).

**When:** First run of the new CLI mode against this project, to confirm it reaches the real store.

**Expected:** a non-zero `read` — this project demonstrably has semantic memories.

**Got:**

```json
{"read":0,"upserted":0,"skipped":0,"anchors_attached":0,"dry_run":true,"mode":"in-place-reembed"}
```

I was one sentence away from reporting "this project has no semantic memories, so there is nothing to repair" — which would have closed the bug on a false premise **and** retracted a true earlier statement.

**Probable cause:** `cargo run --bin codescout` builds with **default features**. `server-stack` is *not* a default (`Cargo.toml`: `default = ["remote-embed", "http", "librarian"]`), so `QdrantSemanticMemoryStore` was not compiled, `VectorBackend::resolve()`'s `cfg(not(feature = "server-stack"))` arm chose sqlite-vec, and the CLI listed an empty *local* store. The live MCP server runs `cargo rb` — `build --release --features server-stack,local-embed` — and talks to Qdrant. Two different stores, one command, no error either way.

**What caught it:** `memory(action="recall", query="embedding backend and retrieval architecture")` returned three memories including `architecture`. A second, independent surface contradicting the first — which is the only reason this is an F-N and not a wrong conclusion in a commit message. Rebuilt with `cargo rb`, the same command reports `read: 15, anchors_attached: 37`.

**Severity:** med. No damage — dry-run wrote nothing — but the failure mode is a **false negative about live data on the verification step itself**, which is the worst place for one. Anything touching the live memory or vector store must be run from a `cargo rb` binary; `cargo run` silently answers about a different backend.

**Status:** fixed-verified — the real run reports 15/15 upserted. Recorded in the archived bug file's § Resume so the next session does not repeat it.

**Fix idea / Pointer:** `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md` § Resume. Kin R-81. Same family as R-77/R-79 (a negative result that is evidence about the instrument, not the world) — here the instrument is a *build configuration*.

## F-44 — `edit_file` and `edit_code` deadlock on a method inside a nested trait impl

**Observed:** 2026-08-14, adding `embed_document` to three identically-named `FixedEmbedder` test fakes in `src/tools/memory/tests.rs`.

**Got — three distinct tool defects in one edit:**

1. **`edit_file`'s content filter is a naive substring match.** It rejected a diff whose only offence was a local variable named `via_trait`: the string `via_trait ` contains `trait `. The filter is meant to catch `trait Foo {` definitions. Renaming to `d_via_seam` was the whole fix. (Third hit of `docs/issues/2026-08-11-edit-code-cannot-remove-nonempty-module.md`'s `fn `-filter this session; the substring aspect is new.)
2. **`edit_code` returns a disambiguator it then refuses.** Given three same-named impls it correctly errored with three fully-qualified paths — `<test_fn>/impl DenseEmbedder for FixedEmbedder/embed` — and said "copied verbatim". Copied verbatim, each one failed: *"cannot determine end of 'embed' for insert-after — AST parse failed"*.
3. **That error's hint is wrong.** *"The file likely has syntax errors that broke tree-sitter's parse"* — the file had none. The only failure at that moment was **semantic** (missing trait items), which tree-sitter does not see. A reader following the hint hunts for syntax errors that do not exist.

**Together:** `edit_file` refuses any content containing `fn `, and `edit_code` cannot insert after a method nested inside a test fn — so there is no supported way to add a method to such an impl. **Workaround:** `edit_code action="replace"` on the whole impl *object* (which needs no end-of-method detection) using the fully-qualified path. That worked on all three.

**Partial refutation worth recording:** `docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md` says there is "no way to disambiguate two symbols with an identical name_path". For this shape the error message *does* hand you the disambiguator — and resolution works for `replace`. That bug is narrower than filed; the adjacent `insert` failure is worse than filed.

**Severity:** med — no wrong output, but it cost several tool round-trips and the misleading hint pointed away from the real cause.

**Promoted-to:**
`docs/issues/archive/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md`
`docs/issues/archive/2026-08-11-edit-code-cannot-remove-nonempty-module.md`
— D11-verified 2026-08-20: both present, both `status: fixed`. The disambiguator file carries
both facets this entry assigned it (insert-after-nested-impl, ≥1 match; the misleading
"syntax errors" hint, 2 matches). Neither file back-cites `F-44`, so the anchor here is
content rather than a citation — the weaker form, kept because the destinations are archived
and archived surfaces are left alone.

**Status:** promoted-to-bug-tracker — belongs on the two existing `edit_code`/`edit_file` bug files rather than as a fourth. **Repointed 2026-08-20 by D11's first sweep:** *"Both should be updated with these facets"* is future tense and had been readable as an open action; the update had in fact landed, and both destinations are now fixed and archived. Facet 1 ("naive substring match") was **withdrawn** by this entry's own same-day correction, so it was correctly never promoted.

**Fix idea / Pointer:** `docs/issues/2026-08-11-edit-code-cannot-remove-nonempty-module.md` (substring filter), `docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md` (narrower than filed; add the `insert`-after-nested-impl failure and the wrong hint).


**Corrected 2026-08-14, same day — two of the three claims above were overstated.**
Both corrections came from reading the implementation instead of inferring it from
the error message, which is the discipline R-80 is about, applied to my own entry.

1. **"Naive substring match" was wrong.** The guard is diff-aware
   (`lines_only_in`, so a keyword on a line byte-identical in old and new counts
   as an unchanged anchor), gated on multi-line, and **already comment-aware**
   (`find_def_keyword` skips `//`, `/*`, `*`, `#`, with its own test
   `find_def_keyword_ignores_class_in_comment`). So my sub-claim that comments
   trip it was simply false. The real defect was one step in and much narrower:
   within a non-comment *changed* line, `line.contains(kw)` had no left word
   boundary. Fixed in `138de7c5` — 4 tests, mutation-verified.
2. **"Partly refutes the no-disambiguator bug" was too strong.** That bug's own
   case is two `impl` blocks at **file scope** — same enclosing scope, so no
   qualifier can exist, and it is genuinely unresolvable by name path. Today's
   case had duplicates in *different* scopes, which the name path already
   qualifies. So the bug is **narrowed, not refuted**: its finding stands for its
   own shape, and its title over-generalises. Recorded in that file's § Summary.

What survives unchanged: the `edit_code` `insert` failure on a nested trait-impl
method, and its hint blaming syntax errors on a file whose only problem was
semantic. Both now live in
`docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md`
§ Resume as item 2.

**Lesson for the ledger:** an entry written from a tool's *error message* inherits
that message's model of itself. `edit_file`'s error says "edit contains a symbol
definition", which invites "it matched a substring" — and the code says something
more careful. Reading `guard_structural_rewrite` first would have produced a
narrower, truer entry in one pass instead of two.
## W-36 — Refuse a default trait impl and the compiler becomes your enumerator

**Observed:** 2026-08-14, adding a document-side method to `CodeEmbedder` and `DenseEmbedder` for the memory query-prefix fix.

**Pattern:** When a trait gains a method whose *whole purpose* is that implementors must not fall back to the old behaviour, give it **no default implementation**. `cargo check` then lists every implementor that needs attention, and none can silently inherit the defect.

**Counterfactual:** a default delegating to `embed` would have compiled clean and left all five implementors free to keep routing documents through the query seam — i.e. the exact bug, now with a trait method that *looks* like it fixed it. No test would have failed, because no test existed for a method nobody called. Instead the compiler produced five `E0046`s naming `search.rs:456`, `sync.rs:2075`, and three `memory/tests.rs` sites — matching the prediction made before compiling.

**Confirming data points:**
1. This session — five implementors enumerated, prediction confirmed.
2. **The repo already knew this.** `CodeEmbedder::known_dim`'s doc comment: *"Deliberately has no default: a backend that silently inherited `None` would fall back to trusting a config value … defeating the point of asking."* Same reasoning, independently arrived at, now twice.

**Impact:** high. The alternative failure is invisible and permanent.

**Promote-when:** already at two independent datapoints. **Routing: craft-shaped, so the recon SKILL and not codescout memory** — "a trait method added to correct a behaviour gets no default impl, so the compiler enumerates the implementors" is true in any language with default trait/interface methods and would not mislead another project. (Only the *instances* — `known_dim`, `embed_document` — are project facts, and they live here in the tracker.) Sync-flow it as a one-line rule; do not spend a slot in the ~10-rule memory cap on a general lesson.

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the **served** `1.16.13` cache, not the repo
source: back-citation *"(W-36 in codescout's `docs/trackers/bug-fix-session-log.md`.)"*
present, 1 occurrence.

**Status:** promoted-to-permanent-docs — landed 2026-08-20 in `claude-plugins:23a11c3`,
shipped to all three profile caches in `1.16.11` (`claude-plugins:23ca288`) and verified
there at the served bytes. The commit alone did not make it live — see
`prompt-surface-compaction-session-log:F-9`. In
`claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md` § Phase 1 — Scout, as a
bullet opening verbatim: *"A trait or interface method added to CORRECT a behaviour gets no
default implementation — then the compiler enumerates the implementors."* Routed to the
SKILL and **not** codescout memory, exactly as this entry's Promote-when argued: the rule is
craft-shaped, and the ~10-rule memory cap is for project facts.

## W-37 — Mutation-test the test, not only the code

**Observed:** 2026-08-14, writing the wire-level regression test for the document seam.

**Pattern:** After writing a regression test, ask *which layer* it actually exercises — then mutate the layer you believe it protects. If the mutation passes, the test is aimed at the wrong seam.

**Counterfactual, and it is concrete.** My first version asserted on `EmbedderHttp::dense_query` and `dense_document` directly. Both are inherent methods. The defect lived one layer up, in `CodeEmbedder::embed_document_one`'s *routing* — so an impl wired to `dense_query` would have passed my test while reproducing the bug exactly. I noticed before mutating, extended the test to call through the seam (`expect(2)` on the document mock), and the mutation run then failed it with mockito reporting the **query** mock hit twice instead of once.

Relation to W-33/W-34: those mutate the *implementation* to check the tests can fail. This is the complement — checking the test is pointed at the layer where the defect can occur. A test can be perfectly capable of failing and still guard the wrong thing.

**Confirming data points:**
1. This session — wrong-layer coverage caught before commit, mutation-confirmed after the fix.
2. Kin F-36 (2026-07-28), where six tests reused the bug report's single shape and sampled one point of the class six times — the same error in the *input* dimension rather than the *layer* dimension.

**Impact:** high — an on-target-looking test that cannot see the defect is worse than no test, because it stops anyone looking again.

**Promote-when:** a third instance where asking "what layer does this assert on?" relocates a test. Then fold into the recon SKILL alongside W-33/W-34 as one mutation-discipline rule with three faces: can it fail, is it aimed at the right layer, does it sample the class.

**Status:** validated.

## F-45 — My own documented residual bit me within the hour

**Observed:** 2026-08-15, writing the replacement hint for `edit_code`'s
insert-after refusal — roughly an hour after shipping `138de7c5`.

**Got:** `edit_file` refused the edit with
`edit contains a symbol definition ("impl ")`. The offending text was **prose
inside a string literal**: "a method of an impl nested inside a function body".

**Why this is not a regression:** it is the exact residual documented at
`contains_def_keyword_at_word_start` when the word-boundary fix shipped — *"a
keyword inside a string literal still matches: the preceding `"` is not a word
character."* The boundary rule fixed identifiers (`via_trait`), and string
literals were left over-blocking **on purpose**, because the guard's errors are
asymmetric: a false positive costs one rejected edit, a false negative risks the
LSP range corruption of BUG-027.

**Cost:** one rephrase — `impl` → `impl-block`. About thirty seconds.

**Severity:** low, and recorded as *evidence for* the trade rather than against it.
The prediction made when the residual was documented ("rarer than identifiers,
cheap to work around") held on its first real encounter. Narrowing it needs
literal-awareness (`crate::util::text::scan_line` has it) and would trade a
thirty-second workaround for risk in a corruption guard.

**Status:** wontfix-by-design. Re-open only if the literal case starts costing
more than a rephrase — e.g. if it blocks an edit that cannot be reworded.

**Fix idea / Pointer:** `src/tools/edit_file/mod.rs` —
`contains_def_keyword_at_word_start` § "Two known residuals".

## F-46 — A test pinned a false diagnosis, keeping the wrongness alive

**Observed:** 2026-08-15, fixing `edit_code` insert-after's misleading error.

**When:** The full gate failed after the message change — one integration test,
`insert_code_after_refuses_when_ast_cannot_pin_symbol_end` (`tests/symbol_lsp.rs`).

**Expected:** a test guarding the refusal behaviour.

**Got:** it asserted `msg.contains("cannot determine end") && msg.contains("AST
parse failed")` — on a fixture that is **syntactically perfect**. Its own doc
comment describes the scenario accurately (the LSP reports a stale name `a` where
the AST has `alpha`), so nothing about it involves a parse failure. The test was
corroborating a false explanation of its own scenario, and had been since it was
written.

**Why it matters more than a stale assertion:** a test that pins a *wrong message*
inverts the incentive. Correcting the message shows up as a **failing gate**, which
reads as "my change broke something" — the exact signal that discourages fixing
it. The wrongness is not merely unguarded; it is actively defended.

This is a cousin of the `tests-that-cannot-fail` class, one step over: the test
*can* fail, and does — it just fails for the improvement rather than the
regression.

**Severity:** med-high. No user impact, but it kept a misleading diagnostic in
place across every session that hit it, and would have kept doing so.

**Status:** fixed-verified. The assertion now checks the message says "parses
cleanly", does **not** say "has syntax errors", and names the workaround — plus a
new counterweight test proving the broken-parse branch still fires.

**Fix idea / Pointer:** `tests/symbol_lsp.rs` —
`insert_code_after_refuses_when_ast_cannot_pin_symbol_end` and its new sibling
`insert_code_after_blames_syntax_errors_only_when_there_are_some`. Worth a sweep:
grep the suite for assertions on error *text* that encodes a CAUSE rather than a
behaviour, since each is a candidate for the same trap.

## W-38 — Separate a safety guard's refusal from its diagnosis

**Observed:** 2026-08-15, working the `edit_code` insert-after failure that had
deadlocked an edit the day before.

**Pattern:** When a safety guard produces a bad experience, ask which of three
things is actually defective before changing anything: **the refusal**, **the
explanation it gives**, or **the underlying capability** it is compensating for.
They have different owners and different risk.

**Counterfactual — concrete.** The bug file framed this as "`insert`'s
end-of-symbol bound fails", which invites making insert work. Split three ways:

| piece | verdict |
|---|---|
| the refusal | **correct, keep** — BUG-051's residual closure; trusting a short LSP end splices new code mid-body |
| the message | **the defect** — asserted "AST parse failed" + "the file likely has syntax errors" for every cause, false whenever the file parses |
| the capability | **an LSP/AST asymmetry** — tree-sitter's extractor does not surface methods of an impl nested in a function body; the LSP resolves them fine |

Had I treated it as one bug and "fixed insert", I would have reopened a closed
corruption bug (BUG-051) to solve a message problem.

**Sub-pattern worth its own line:** the fix for the message was to **call a
predicate the codebase already had**. `crate::ast::has_syntax_errors` existed, was
already used by `syntax_regressed` in the same subsystem, and answers exactly the
question the hint was guessing at. When an error message asserts a cause, look for
an existing check that can test it — an assertion is a hypothesis, and hypotheses
in this codebase are usually testable in one call.

**Confirming data points:**
1. This session — three-way split prevented reopening BUG-051; the message now
   discriminates, with a counterweight test per branch.
2. Kin: the `2c8690b5` gap-2 fix the same day, where the sibling-drop *check* was
   right and only its **scope** (descendants counted as siblings) was wrong — the
   same "the guard is fine, its inputs/output are not" shape.

**Impact:** high. The failure mode it avoids is un-fixing a corruption guard in
pursuit of ergonomics, which is the most expensive direction to be wrong in.

**Promote-when:** a third instance. Then fold into the recon SKILL as a triage
question: *"is the guard wrong, or is its explanation wrong, or is it compensating
for a missing capability?"*

**Status:** validated.

## F-47 — Implemented and shipped a guard exemption, then reverted it within the hour

**Observed:** 2026-08-15, working the `edit_code`-cannot-remove-a-non-empty-module
bug down to its last two gaps.

**When:** Gap 3 — *"`edit_file`'s filter refusing pure deletions"*. The bug file
presented it as an oversight (*"the filter fires on deletions just as much as
insertions"*) and recommended a narrow exemption for `new_string.is_empty()`. I
agreed, implemented it, wrote five tests, ran a targeted suite, and committed
(`80350afe`).

**Expected:** a small ergonomic fix to an over-broad guard.

**Got:** the full-workspace run failed on
`edit_file_warns_hint_suggests_remove_when_new_empty`, which asserts that exact
refusal — and `infer_edit_hint` turned out to carry machinery built *specifically*
to route the deletion case to `edit_code(action='remove')`. **The block was a
contract with purpose-built support, not an oversight.** Reverted in `9b902e0a`.

**Probable cause:** I took the filing's framing at face value on a *refusal*. Both
pieces of counter-evidence — the test and the hint branch — were one `grep` on the
error string away. My supporting argument was also backwards: I reasoned that
`edit_code(remove)` is unavailable exactly when its naming fails, so the fallback
must work — but those naming failures were two separate open bugs, and **the fix
for a dead end is to open the road, not remove the guardrail beside it.** (Both
were then fixed properly in `b2bc8edb`.)

**Workaround / correction:** exemption reverted; the refusal is now recorded as a
decision by `guard_blocks_pure_deletion_of_a_nonempty_module`, which points at its
hint sibling so the next reader finds both halves together.

**Severity:** med — two commits and roughly an hour, no shipped defect. The suite
caught it, which is the system working; the cost was avoidable by one search.

**Status:** fixed-verified — reverted, documented in the archived bug file's § Gap 3,
and generalised as R-85.

**Fix idea / Pointer:** before *removing or widening* a refusal, grep the refusal's
message string across the repo. A refusal with a dedicated test, a dedicated error
branch, or bespoke hint text is a designed contract. Absence of all three is the
only evidence that it was incidental.

## F-48 — A name-filtered test run gave false confidence and let F-47 reach a commit

**Observed:** 2026-08-15, immediately downstream of F-47.

**When:** After changing `guard_structural_rewrite`, I verified with
`cargo test --lib guard_` — 29 tests, all green, including the five I had just
written.

**Expected:** the filter `guard_` covers the guard's tests.

**Got:** it covers the tests *named* `guard_*`. The test that actually pinned the
behaviour I changed is called
`edit_file_warns_hint_suggests_remove_when_new_empty` — named for the **hint** it
asserts, not for the guard it exercises. The filter missed it, the targeted run was
green, and the change was committed. Only `cargo test --workspace` caught it.

**Probable cause:** a name filter selects by *naming convention*, not by
blast radius, and the two diverge exactly where a test is named for its subject
(the hint) rather than its mechanism (the guard) — which is usually the *better*
test name.

**Workaround:** run the full suite before committing a change to shared code, and
treat a filtered run as a fast inner loop only.

**Severity:** med — it is the mechanism that let a wrong change reach a commit. On
its own it is cheap; combined with F-47 it converted a search-first mistake into a
revert.

**Status:** fixed-verified — every subsequent fix this session ran
`cargo test --workspace` before commit.

**Fix idea / Pointer:** when the change is to a *shared guard or helper*, derive the
test set from `references(symbol=...)`, not from a name filter. The callers are the
blast radius; the name is a guess about it.

## W-39 — Reproduce before trusting the filing — measured across a fourteen-bug drive

**Observed:** 2026-08-15, closing the `docs/issues/` backlog to its floor.

**Pattern:** For every bug, reproduce the reported condition on HEAD *before*
reading its § Fix as guidance. Treat the filing's cause, scope and counts as
hypotheses to re-derive, not as findings to build on.

**Counterfactual:** Of fourteen bugs closed, **eleven had reasoning weaker than
their own code**, and following the filing would have produced the wrong change in
each:

| Filing said | Code said |
|---|---|
| "no test asserts either behaviour" (`format_hover`) | `hover_with_code_fence` asserted the discriminating pair already |
| "machine-specific path" (sweep scripts) | `code-explorer` is **this repo's** pre-rename name — the default meant the repo root |
| "reproduces only on a Windows host" (clippy) | reproduced on Linux via an installed cross target, and 1 of 3 lints was already gone |
| "no workarounds found" (CyberArk) | `default` features carry no `ort` at all |
| "insert cannot place a module at file scope" | `find_parent_symbol` returns `None` for top-level anchors — documented in its own doc comment |
| "companion behaves as if drift is on" (state-protocol) | the match is fail-**closed**; removal disables it |
| "the 8 highs are historical sections" | 10 highs, three classes, one a real broken ref |
| "symbols is lax / edit_code is strict" | three producers disagree; a **false code comment** said two of them agreed |
| "~950 ms unaccounted" | 127 ms, and the largest slice is dispatch shared by every tool |

The three recurring shapes are worth naming: **absence claims** (cheapest to write,
most expensive to trust, because checking one means searching rather than reading);
**count-vs-category generalisation** (R-84); and **false comments about a peer
component** — `// This matches how LSP reports symbols` was not a stale note, it was
the root cause, because nobody reconciled the two producers *while the comment said
they already agreed*.

**Confirming data points:** eleven of fourteen, this session. Prior: R-59, R-78,
R-80 record the same failure with smaller samples.

**Impact:** high — the discipline is what separated "close 14 bugs" from "make 11
wrong changes and close 14 bugs".

**Promote-when:** already load-bearing. CLAUDE.md's bug-tracking section should gain
one line: *a bug file's § Root cause and § Fix are hypotheses until re-derived on
HEAD; its § Symptom is the only part written from observation.* Promote at the next
CLAUDE.md edit.

**Status:** validated — nine tabulated instances in a single work stream.

## W-40 — Time the subject against a trivial control on the same connection

**Observed:** 2026-08-15, closing the `semantic_search` latency bug.

**Pattern:** To attribute latency, measure the subject *and* a deliberately trivial
case over the **same** live connection, warm, median of N. The control is what turns
a number into a breakdown.

**Counterfactual:** The bug asked where ~950 ms of a ~990 ms call went. Measuring
`semantic_search` alone yields *127 ms* — better, and still uninterpretable: is that
retrieval, dispatch, or transport? With two controls on the same session the answer
is structural rather than speculative:

| case | median ms | what it bounds |
|---|---|---|
| `tools/list` | 1.0 | JSON-RPC transport floor |
| `tree` depth=1 | 39.5 | tool dispatch + context, common to every tool |
| `semantic_search` limit=1 | 122.2 | fixed per-call cost |
| `semantic_search` limit=10 | 126.9 | + per-hit cost (only 4.7 ms) |

The limit=1/limit=10 pair **refuted** the filed overfetch-payload hypothesis with
four lines of output. The `tree` control produced the finding that outlived the bug:
~39 ms of dispatch is paid by *every* tool, and is now the largest identified slice
of a search — filed separately, because calling it "semantic_search is slow" would
repeat the exact mis-attribution the bug was opened to correct.

**Confirming data points:** one, but decisive — it closed a bug that had been open
since 2026-08-07 with four untested hypotheses.

**Impact:** med-high — cheap (a ~120-line probe script), and it converts "X is slow"
into "X costs A + B + C, of which B is not X's".

**Promote-when:** a second performance investigation uses the control pattern and
reaches a component-level attribution. At two datapoints, promote to the recon SKILL
as a measurement rule.

**Status:** validated — single datapoint, but it produced both a refutation and a
new separately-filed finding.

## F-49 — A fix I shipped, documented and archived was inert on the transport the project uses

**Observed:** 2026-08-15, live verification after `cargo rb` + `/mcp` on the
rebuilt server — the routine post-rebuild check, run on a bug already closed.

**When:** Verifying `eedb308c` ("re-sync a file that changed on disk before
answering about it"). Wrote a probe file, `symbols()` it, prepended three lines via
shell (an external write codescout is never told about), `symbols()` again.

**Expected:** the shifted line range, per the fix.

**Got:** `4-6` — the pre-write range. **The original bug, unchanged.**

**Probable cause — measured, not guessed.** `did_open` records the on-disk
signature, and on **socket** transport it never takes its already-open early
return, because the mux owns document dedup. So every `document_symbols`
re-recorded the *current* disk state, and the drift check running afterwards
compared the file against itself. Meanwhile the mux deduped the `didOpen`, so the
server kept its stale copy. **Two halves silently cancelling**, each correct in
isolation.

The end-to-end test passed throughout because it drives `LspClient::start` with
`mux: false` — the **stdio** path, where `did_open`'s early return hides the
difference. Rust in this project runs through the mux. **The test exercised the one
transport on which the defect cannot appear.**

**Workaround / fix:** `7c8863f0`. The drift check now runs *before* `did_open`, but
the repair that matters is making the recording rule explicit instead of
positional: `did_open` records vacant-only, `did_change` overwrites — *a
notification that may be deduped must not claim delivery*. Ordering becomes
irrelevant rather than load-bearing. The bookkeeping moved into a
`SyncedSignatures` type so the invariant is testable without standing up a mux,
which is exactly why it had no test before.

**Severity:** high — not for the defect (a stale read) but for **how close it came
to being believed**. It had a green 3814-test gate, a real-rust-analyzer end-to-end
test, a written-up bug file, an archive move, and a "verified" line in my own
report. Every one of those was wrong together.

**Status:** fixed-verified — re-verified live on the mux path (`7-9` → external
write → `10-12`, no flush), mutation-verified in unit tests, gate 3817 green.

**Fix idea / Pointer:** when a component has more than one transport or deployment
mode, a test that constructs the *simplest* one is a smoke test, not verification.
Either parameterise over the modes or assert on a mode-independent invariant — the
latter is what `SyncedSignatures` made possible here. Generalised as R-86.

## W-41 — Live-verify after every rebuild, even when the bug is already closed

**Observed:** 2026-08-15, post-`/mcp` verification pass at the end of a
fourteen-bug drive.

**Pattern:** After `cargo rb` + `/mcp`, re-run the *original reproduction* for each
fix on the live server — not a proxy, not the test suite, the reproduction. Treat a
closed bug file as a claim to be checked, not a record to be trusted.

**Counterfactual:** `eedb308c` had a real-rust-analyzer end-to-end test, a green
3814-test gate, a written bug file with a § Tests-added section, an archive move,
and my own "verified end to end" line in the session report. **All of it was
wrong.** The fix was inert on the mux path (F-49). The probe that caught it — write
a file, `symbols()`, prepend three lines via shell, `symbols()` again — took under
a minute.

Without that minute: a closed bug file asserting a fix that did not exist, a
CHANGELOG entry describing it, and a fast-forward to `master` carrying all three.
The next session to hit stale symbols would have found the bug marked *fixed and
archived* — the most expensive state for a live bug to be in, because it redirects
the reader away from the cause.

**Confirming data points:**
1. This session — `eedb308c` inert, caught only live.
2. The same pass confirmed three *other* fixes genuinely live (name-path
   reconciliation, ambiguity spans, `at_line` hint), so the check discriminates
   rather than always finding fault.
3. Prior: `docs/RELEASE.md` already carries a live-surface verification step from
   an earlier round — this is its first save.

**Impact:** high — it is the only gate that sees the *shipped configuration*. The
test suite chooses its own construction; the live server does not.

**Promote-when:** already promoted in shape — `docs/RELEASE.md`'s live-surface step
exists. Strengthen its wording at the next edit: the step must re-run the bug's
**original reproduction**, not merely confirm the tool responds. A smoke call would
have passed here.

**Status:** validated — one save, and it prevented a false "fixed" from reaching
`master`.

## F-50 — Three tools answered about a subset while looking like they answered about the whole

**Observed:** 2026-08-15, across the structural-debt refactor stream
(SD-1, SD-1b). Same failure three times in one session, from three unrelated
tools, and it put a false claim into a shipped commit body.

**When:** Each time I needed a COUNT — how many citations are stale, how many
matches exist, how many findings were capped.

**Expected:** That a tool asked for a measurement returns the measurement, or
says it cannot.

**Got:** Three confident answers about samples, none of them labelled as such.

1. **A markdown auditor pointed at Rust.** `audit_doc_refs --paths
   'src/**/*.rs'` reported 7 stale bug-file citations. The real number was
   **95 of 111**. `pulldown_cmark` reads `tokio::sync::Semaphore` as a link and
   yields 33,105 "refs", so the handful of `docs/`-shaped findings that surface
   are an arbitrary fraction. **A tool used outside its domain does not fail
   loudly — it answers.**
2. **A grep capped at 50 with a footer that read as a total.** Default-mode
   `grep` returned *"50 matches in 20 files"* and *"Showing 50 of 50"*;
   `mode="files"` on the identical pattern returned **266 in 94**. Building the
   substitution list from the first would have swept a fifth of the sites and
   reported done. The parallel session independently filed this as a bug file
   the same day.
3. **A truncated buffer read as a corpus.** I ran a scan as `… | head -8`,
   grepped the resulting eight-line `@cmd_` buffer for `code_comment_capped`,
   got zero, and wrote "the cap never fires" into commit `450880c7` and the
   tracker. It fires. Re-run bare, the findings array itself says
   `"shown": 50, "total": 51833` — so the second attempt was a 0.1% sample and
   equally worthless.

**Probable cause:** Every one of these returns a *well-formed* answer. There is
no error, no empty result, no warning — the shape of a truncated answer is
identical to the shape of a complete one. Three different truncation mechanisms
(domain mismatch, result cap, pipe) converge on the same silence.

**Workaround:** Before believing a count, establish what the denominator is
*from a second source*. Enumerate into a file and classify against the
filesystem; ask for `mode="files"`; run bare and query the buffer rather than
piping to a trimmer. For a sweep, re-classify afterwards and require the
residual to be **zero** — that check is what actually proved the 95 landed.

**Severity:** high — not for the wasted effort but because instance 3 reached a
commit body and a tracker as a stated fact, and instance 1 nearly scoped the
work at 7% of its real size.

**Status:** fixed-verified — all three corrected (`1f29fc9b`, `3a366d29`), and
the corrections cite the method rather than just the number.

**Fix idea / Pointer:** Generalises W-39's shape ("a count and a category are
two claims") one level out: *a count is a claim about a denominator, and the
denominator is the part tools silently substitute.*

## W-42 — A gate built in the morning verified the afternoon's work, on drift a human reading would have missed

**Observed:** 2026-08-15, SD-1b then SD-8, same session.

**Pattern:** When a work stream's finding is "we cannot see class X of drift",
build the seeing FIRST and let it verify everything after — including the
remaining items of the very backlog that motivated it.

SD-1 found 95 stale bug-file citations in Rust comments and three that resolved
nowhere. The sweep was mechanical; the three residuals needed judgment and were
deliberately deferred as SD-8. In between, SD-1b taught `audit_doc_refs` to
read documentation nodes in every language codescout has a grammar for. When
SD-8 came around, its verification was not me re-reading files — it was
`--paths` over the three touched files, checking `verdict=resolved`.

**Counterfactual:** Without the gate, SD-8's check is eyeballing three
comments, which is exactly the process that let 95 citations rot in the first
place. And it did more than confirm: it *found* drift I had not filed —
`src/fs/mod.rs:3` cites a path that no longer exists, and one of SD-8's own
items (the 2026-03-24 kotlin-lsp concurrent-instances slug, named without its
extension here because spelled in full it is itself an unresolvable citation)
surfaced from the gate rather than from my reading. It also caught **three separate self-inflicted
refs in the tracker prose describing the work** — writing about a dead
reference creates a dead reference, every time, and I would have shipped all
three.

**Confirming data points:**
1. SD-8 verified by the gate rather than by eye; both surviving citations
   report `verdict=resolved` (`c692f901`).
2. The gate found `src/fs/mod.rs:3` unprompted — real drift outside the filed
   backlog.
3. Three tracker-prose false refs caught pre-commit across `1f29fc9b`,
   `3a366d29`, `587b9681`.

**Impact:** high — converts a class of drift from "noticed when someone happens
to follow a link" into "reported on every run".

**Promote-when:** A second work stream builds its detector before finishing its
backlog and the detector catches something the backlog missed. At two
datapoints, promote to CLAUDE.md as: *when a finding is "we cannot see X",
build the seeing before finishing the sweep — the sweep is then verified rather
than asserted.*

**Status:** validated — single stream, three confirming data points within it.

## W-43 — A backlog row's instruction can outlive its own hypothesis; run the instruction

**Observed:** 2026-08-15, resuming the structural-debt stream at
`docs/trackers/structural-debt-refactor.md` SD-3, on `experiments` @ `1d22b715`.

**Pattern:** When a tracker row carries *both* a hypothesis and an instruction to
verify it before acting, run the instruction even when the hypothesis looks
obviously right — and when the reading falsifies it, mark the row `superseded`
with a successor rather than rewriting it in place. A row whose hypothesis was
wrong but whose instruction was sound is evidence about how the backlog is
written, and rewriting it destroys that.

SD-3's hypothesis: four `::call` handlers, all ranked tier-1 over-budget by
`legibility_scan`, are the measured friction and should give up a shared
extraction. SD-3's instruction, written into the same field: *"UNVERIFIED whether
the four share a phase structure. Read the other three BEFORE proposing
anything."*

**Counterfactual:** Acting on the hypothesis meant proposing an extraction from
`src/librarian/tools/get.rs` alone, or a four-way one, across ~1460 lines of
working code. Reading all four showed the shape does not exist: they are two
pairs on a different axis — `find`/`context` are query handlers,
`get`/`update` are single-artifact handlers — and the hypothesised shared
"apply overlay" phase is 2 of 4 (`get` and `find` share `shadow_main_pairs`;
`update` uses `resolve_write_target`; `context` has none).

The cost of getting this wrong was not only wasted effort. The reading found the
duplication that *is* there — the scope-resolution prologue written three times,
one copy drifted (SD-10) — and that copy is not a tidiness question but a live defect:
`librarian(action="context", scope="all")` runs with no scope clause where
`artifact(action="find", scope="all")` narrows to the umbrella, measured on the
running server, returning an artifact from outside the umbrella. Filed as
`docs/issues/archive/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`.

Had the extraction gone ahead on the hypothesised axis, the three-way duplication
would have been left untouched *and* made harder to see, since two of its three
sites sit inside handlers the refactor would have reorganised. The third site,
`src/librarian/tools/workspace_state_at.rs`, was never in the group at all.

**Confirming data points:**
1. This session — SD-3's hypothesis falsified, its instruction produced SD-10 and
   a high-severity bug file.
2. W-32 (2026-08-14) — the same shape one level down: *run the bug's reproduction
   before reading its fix plan, because the plan is a hypothesis about the
   reproduction*. Three fixes changed on contact.
3. W-39 (2026-08-15) — eleven of fourteen bug filings had reasoning weaker than
   their own code.

**Impact:** high — prevented a large no-op refactor of load-bearing handlers and
converted the effort into a measured, filed defect.

**Promote-when:** a third instance where a tracker/bug row's *verify-first*
instruction contradicts its own stated conclusion and the instruction wins. At
three, promote to `CLAUDE.md` as a backlog-reading rule: when a row states both a
conclusion and a check, the check is the load-bearing half — and prefer
`superseded` over in-place rewrite so the falsification stays legible.

**Status:** validated — two adjacent data points (W-32, W-39) already support the
general shape; this is the first at backlog-row grain.

## W-44 — A sample from one language is not a sample of the corpus

**Observed:** 2026-08-16, tuning `audit_doc_refs`' ref classifier after SD-1b's
code-comment surface finally ran against a current binary.

**Pattern:** Before tuning a rule that applies across N populations, measure the
thing you are tuning on in **at least two** of them. A rate computed on one
population is a fact about that population, not about the rule.

The first measurement was Rust only — `src/librarian/catalog/**`, 41 refs, **56%
`unknown`**, essentially all dotted identifiers (`commits.git_root`,
`artifact.id`, `report.remap`). The obvious reading, which is the one I wrote
down and recommended, was "the classifier over-produces on dotted tokens; suppress
them."

The second measurement — every non-Rust source file in the repo, 51 files, 39
refs — came back **5% unknown and 79% resolved**. Same classifier, same corpus,
one-eleventh the noise.

**Counterfactual.** `is_module_path` accepts all-lowercase dotted tokens, which is
simultaneously a Rust field, a Python module (`os.path`), a Go qualified name, and
a Java package segment. The token cannot distinguish them. Suppressing it
globally — the fix the Rust-only sample recommended — would have silently deleted
**real module references from Python, Go, Java, Kotlin and TypeScript**, in
service of a Rust habit of naming SQL columns in doc comments. Nothing would have
failed: the suppressed refs were already `unknown`, so the metric would have
*improved* while the tool got worse.

**What was built instead** (`experiments:9b3e1e76`): `PathSyntax`, threaded from
`scan_code_comments` into `classify`. Rust spells qualified names `a::b`, so a
dotted token is never a module path there; the dotted-module languages keep
theirs; shell/CSS/HTML have no module concept at all. Markdown maps to today's
behaviour, deliberately.

**Verified live on the rebuilt server, both directions:**

| corpus | refs | resolved | unknown |
|---|---|---|---|
| `src/librarian/catalog/**` before | 41 | 12 (29%) | **23 (56%)** |
| `src/librarian/catalog/**` after | 18 | 14 (78%) | **0** |
| all non-Rust before | 39 | 31 | 2 |
| all non-Rust after | 39 | 31 | 2 |

The non-Rust row is byte-identical — not one ref moved. Repo-wide Rust went
1,334 → 1,096 refs with `unknown` 358 → 116 and **resolved slightly UP** (661 →
667). The narrowing cost zero real references, which was the property most worth
proving and the one a metrics-only view would have hidden.

**Confirming data points:**
1. This session — a Rust-only noise rate of 56% vs 5% everywhere else.
2. F-50 (2026-08-15) — three tools answering about a subset while looking like
   they answered about the whole. Same failure at instrument grain rather than
   population grain.
3. The retracted 296-file measurement earlier the same day — a number taken from
   one window on a shared machine and read as a property of the subject.

**Impact:** high — prevented a change that would have deleted real references
across five languages while improving the metric that motivated it.

**Promote-when:** a third instance where a per-population measurement contradicts
a whole-corpus inference. At three, promote to `CLAUDE.md` beside the Conclude
Last rule: *a rate measured on one population is evidence about that population;
before generalising a fix from it, measure a second one.*

**Status:** validated — the corrective was built, and both the intended effect
and the invariance were measured on the running server rather than argued.

## F-51 — Shipped a process-global whose semantics contradict the project's only precedent for one

**Observed:** 2026-08-16, architecture review of `29f0c015` — the BL-33 librarian-guard fix
landed roughly an hour earlier in the same session.

**When:** Post-ship, before the pattern gets reused. Triggered by a request to survey the
core/librarian boundary, not by any failure.

**Expected (my own design note, written into the commit body *and* the bug file):** "There
is one catalog per process" — offered as the justification for reaching for a `OnceLock`
instead of threading a field through the core `ToolContext` (124 construction sites across
21 files).

**Got (scouted reality):** true in production, false in tests, and the wrong semantics
either way.

- `references(try_build_runtime_with)` → exactly **one** production caller,
  `src/server.rs:228`. It is reached via `CodeScoutServer::from_parts` from two entry
  points, `src/server.rs:1510` (stdio) and `:1571` (HTTP, whose own comment reads *"Build
  the server once (async), then clone per session"*). The production half of the claim
  holds.
- But `from_parts_with_env` has **three test-helper callers** (`src/server.rs:1686`,
  `:3713`, `:4084`), each building a fresh server with its own catalog, many times per test
  binary. `install_augmentation_guard_oracle` does `let _ = ORACLE.set(…)`, and
  `OnceLock::set` returns `Err` on every call after the first — discarded. **The first
  librarian-enabled server constructed in a `cargo test` process pins its catalog for the
  whole binary**; every later test's guard then consults a foreign catalog.
- Incidental from the same sweep: `try_build_runtime`
  (`src/librarian/adapter.rs:20`) has **zero callers**.

**Probable cause:** I verified the production call graph and stopped there. "One catalog per
process" is a claim about current state, and I checked only the half that supported the
design I had already chosen — the test half is precisely where a process-global's lifetime
stops matching a server's.

The sharper miss is the precedent. `src/heartbeat.rs:47` is this project's **only** other
global mutable state, and its doc comment reads *"Last-writer-wins and never cleared"* — a
deliberate choice with the reasoning written down beside it. I cited that file in the commit
body as precedent **for having a global at all**, without reading which semantics it picked.
It picked the ones that make this defect impossible. Reading a precedent for its existence
rather than for its content is the whole failure in one line.

**Workaround:** none needed — no live misbehaviour. Fix direction: `OnceLock` →
last-writer-wins (`RwLock<Option<Arc<dyn AugmentedArtifactOracle>>>`, `install` overwrites).
Production is unchanged (one server); in tests each construction replaces, so the guard
tracks the server under test rather than whichever happened to run first. ~5 lines, no
call-site churn.

**Severity:** med — the guard can only *under*-fire: a foreign path yields no catalog row,
`is_augmented` returns `false`, and it degrades to the frontmatter check that shipped in the
same commit. No data loss, no false refusal. But a **safety guard whose behaviour depends on
test ordering is not a guard you can cite**, and `let _ = set()` makes the second install
silent — nothing would ever surface it.

**Status:** fixed-verified — `053238cb` on `experiments`. `OnceLock` →
`RwLock<Option<Arc<dyn AugmentedArtifactOracle>>>`, `install` overwrites. `install_into` /
`read_from` are split out so the regression test owns its own slot and never mutates the
process-wide `ORACLE` — a global-semantics test that perturbs concurrent tests would be the
same class of defect it is testing for. Watched fail first with `install_into` written as
first-writer-wins, which reproduces `OnceLock` exactly. Gate: 3901 tests, clippy, fmt.

**Note on verification:** there is deliberately no live-MCP check here. The fix has **no
observable production difference** — production builds one server, so first- and
last-writer-wins are identical there; the whole defect lived in the test binary. Invoking
the running server would produce a green that proves nothing, which is the failure mode the
recon skill names as *"a result that is green and uninformative"*. The evidence is the
reproduced red, not a tool call.

**Fix idea / Pointer:** `src/util/librarian_guard.rs` (`ORACLE`,
`install_augmented_oracle`); precedent to copy verbatim: `src/heartbeat.rs:41-47`. Shipped
in `29f0c015`; bug file
`docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`.

## F-52 — `Agent::project_root()` is the FOCUSED SUB-PROJECT root, not the workspace root

**Observed:** 2026-08-18, designing the guide-ledger re-arm fix (make
`workspace(action="activate")` stop wiping `guide_hints_emitted` on every call).

**When:** About to specify the re-arm predicate — "clear only when the newly activated
root differs from the current one" — and reaching for the obvious comparand.

**Expected:** `ctx.agent.project_root().await` returns the root of the project the
session is working in, so `project_root() != new_root` answers "did the project change?".
This is what the name says and what `server.rs:249` (`let guide_project_root =
agent.project_root().await`) uses it for.

**Got (scouted reality):** `Agent::project_root` (`src/agent/mod.rs:1360-1363`) is
`inner.default_workspace()?.focused_project_root().ok()` — the **focused sub-project**,
not the workspace. Its own doc comment says so: *"even when the focused project is still
`Dormant` (i.e. after `switch_focus` to a sub-project…)"*. In this repo the workspace has
two projects (`codescout` at `.`, `codescout-embed` at `crates/codescout-embed`), so with
focus on the latter, `project_root()` is `<repo>/crates/codescout-embed`. A subsequent
`activate("<repo>")` — same workspace, same repo, no project change — would compare
unequal and re-arm. The correct comparand is `AgentInner::default_workspace_root`
(`src/agent/mod.rs:106`, already `pub`), which is what `Agent::activate:541` sets.

Second half of the same seam: `ActivateProject::call` has two paths, and only one has a
root at all. The bare-project-id **focus-switch** path (`src/tools/config/mod.rs:151-186`)
returns early via `activate_within_workspace`, never reaching the full-activation path —
so putting the predicate in the full-activation path alone gives "a sub-project focus
switch never re-arms" for free, with no comparand needed for it.

**Probable cause:** `project_root()` is named for the single-project era and kept its name
through the Phase-1 multi-workspace registry (`docs/plans/2026-05-30-per-request-workspace-pinning.md`).
Callers that want "the workspace" and callers that want "the focused project" both reach
for it, and it silently serves only the second.

**Workaround:** Compare the already-canonicalized `root` local in
`ActivateProject::call:196` (`root.canonicalize().unwrap_or(root)`) against
`inner.default_workspace_root`, read through the same `ctx.agent.inner.read().await` the
focus-switch path already takes. Canonicalization on both sides is load-bearing and
already handled — `Agent::activate:528` carries a comment recording a prior bug from
exactly this omission (*"activate(\".\") would compare unequal to Agent::new's
canonicalized home_root, making is_home false on the first re-activation and flipping to
read-only"*).

**Severity:** med — would have shipped a re-arm that mis-fires on any workspace with
sub-projects, i.e. codescout itself. The symptom is the bug the change exists to fix,
still happening, in the one repo where it would be tested by hand.

**Status:** open — design decided, not yet implemented.

**Fix idea / Pointer:** Design in progress this session; predicate belongs in
`src/tools/config/mod.rs` full-activation path. Consider a `pub async fn
default_workspace_root(&self)` accessor on `Agent` so the next caller does not repeat F-52.

---

## W-45 — Scout the change's own regression test before designing the change

**Observed:** 2026-08-18, `/codescout-companion:reconnaissance` invoked mid-brainstorm,
after the re-arm policy was decided but before any design doc or code was written.

**Pattern:** When a change alters a **policy** (not a shape), grep the test suite for the
test that pins the *current* policy, and grep the model-facing prompt surfaces for prose
that *documents* it. Both are part of the change's blast radius, and neither shows up in a
symbol-level scout of the code being edited.

**Counterfactual:** Two concrete misses, both caught pre-implementation:

1. `server::guide_hint_tests::activate_project_resets_hints` (`src/server.rs:4292-4318`)
   asserts *"activate should reset emitted set; first post-activate call should re-emit"*.
   It activates `dir.path()` — the **same** root `make_server()` (`src/server.rs:3822`)
   built the agent with. Under the new policy that is a same-project re-activation, so the
   test inverts. Implementing first would have produced a red test whose message reads
   "your change is wrong" when the correct reading is "this test encodes the policy you
   replaced" — the failure mode where a correct change gets reverted by its own suite. It
   needs splitting into two tests (same-root keeps, different-root re-arms), not tweaking.

2. `src/prompts/guides/workspace-state.md:51` states `guide_hints_emitted` is *"Cleared on
   every activation … After a clear, the next of either re-emits."* That file is
   `include_str!`'d at `src/prompts/mod.rs:445` and hard-injected into the model's context
   as the `workspace-state` guide. Shipping the code change alone would have left the
   model reading a false description of the mechanism, every session, in the guide whose
   entire job is to describe activation semantics — self-inflicted doc-vs-code drift on a
   model-facing surface, which no test catches (`prompts/mod.rs:1432` asserts topic ↔ body
   *existence*, not content).

**Confirming data points:**
1. This session — policy change, 1 inverted regression test + 1 stale model-facing guide,
   neither reachable from `symbols`/`references` on the code under edit.
2. W-8 (this log) — `prompt_surfaces` cap gate caught a byte-budget violation on the same
   family of `include_str!`'d surfaces; same lesson from the enforcement side.

**Impact:** med-high — prevented one likely revert-the-correct-change cycle and one
shipped falsehood in an always-injected guide.

**Promote-when:** A second policy-shaped change (not shape-shaped) where the pinning test
or the prose surface was the thing that drifted. At 2 datapoints, promote to the
reconnaissance SKILL.md Phase 1 as a seam class: *"policy change → grep for the test that
pins the old policy AND the prose surface that documents it."* Craft-shaped by the routing
test — a test encoding the policy you are replacing is not codescout-specific.

**Status:** validated — single datapoint, both gaps caught before any code was written.

---

## F-53 — Proposed a PPID-derived session key without enumerating client session-resume

**Observed:** 2026-08-18, designing the agent-agnostic session key for the guide-hint
ledger (same work stream as F-52 / W-45).

**When:** The user added two requirements mid-brainstorm — the ledger must work in git
worktrees and for agents other than Claude Code. `CLAUDE_CODE_SESSION_ID` is
CC-specific, so a harness-agnostic fallback was needed.

**Expected:** **Parent PID + parent start-time** is a sound agent-agnostic session key.
I scouted it before proposing: read the live process tree, confirmed every MCP-server
process has a harness process as its parent, confirmed the nested `codescout mux
--socket …` children are LSP workers that never construct a `CodeScoutServer` (so they
never compute a key), and paired the PID with `/proc/<ppid>/stat` field 22 against PID
reuse.

**Got:** The user pointed out **session resume**. `claude --resume <uuid>` /
`--continue` restarts the *client* process — new PID — while the conversation, and
its `CLAUDE_CODE_SESSION_ID`, continue unchanged. The refuting evidence was already in
data I had pulled 20 minutes earlier and not read for this question: session
`2c518eb6` spans 2026-08-06 → 2026-08-18 (12 days, 9630 calls, **67** MCP processes)
under a `claude --resume 2c518eb6-…` process started 2026-08-17 14:46 — a 12-day
conversation on a 1-day-old parent — and its ledger file is still keyed by the
unchanged uuid.

**Probable cause:** Every check I ran was aimed at the event that motivated the design
(MCP subprocess respawn) and none at the other lifecycle events of the process being
keyed on. The depth of the scout on that one event read as coverage of the key.

**Workaround:** PPID+start-time demoted from "the agent-agnostic answer" to a
last-resort rank above `uuid + warning`, with the resume hole documented at the call
site. The primary chain becomes an explicit `CODESCOUT_SESSION_ID` env var plus a
per-harness table selected by `clientInfo.name`; three research subagents were
dispatched to establish what each non-CC harness actually exposes.

**Severity:** med — would have shipped a fallback that silently loses the ledger on
every resume, i.e. re-injects every guide into a conversation that demonstrably still
holds them. Silent, and worst exactly where the state matters most.

**Status:** fixed-verified — caught in design, before any spec or code was written.

**Fix idea / Pointer:** R-105 in `docs/trackers/reconnaissance-patterns.md` carries the
general rule (*a key derived from runtime state is a claim about that state's lifetime;
enumerate the lifecycle events before proposing it*). Kin F-52, W-45.

---

## F-54 — A subagent's gate run read a concurrent session's mid-edit tree and reported a transient as a live failure

**Observed:** 2026-08-18, subagent-driven execution of
`docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md`, Task 1 fix round 1.

**When:** An implementer subagent ran the plan's mandated pre-commit gate
(`cargo fmt && cargo clippy -- -D warnings && cargo test`) in the main checkout while a
second Claude Code session was actively editing `src/prompts/` in the *same working
tree*.

**Expected:** The gate reports only on the dispatched task's own change
(`src/util/fs.rs`).

**Got:** `cargo test` reported `prompts::tests::prompt_surfaces_server_instructions_snapshot`
failing. The implementer traced it correctly via `git log` to the other session's commit
`32b34efa` ("revert(prompts): drop the IL1 overlap clause"), correctly declined to fix
it, and reported `DONE` with the failure as a standing concern. But the failure was
**transient**, not standing: the other session had `src/prompts/source.md` and its
snapshot momentarily inconsistent on disk mid-edit. Controller re-ran at the same HEAD:
that test passes and the full suite is green (4098 passed / 0 failed / 45 ignored).

**Probable cause:** Two agents share one checkout. `cargo test` reads the *working tree*,
not a commit, so any full-suite run is a race against every other session's unstaged
edits. `src/prompts/README.md` already documents a "shared-branch verify hazard" for the
prompt surfaces; this is the same hazard reached from the other direction — not a stale
*branch*, but a mid-write *tree*. Nothing in the subagent dispatch warned about it, so
the implementer had no frame for "failure outside my files ⇒ suspect a transient first".

**Workaround:** Two layers, both cheap.
1. Every implementer dispatch now carries: *a failing test in a file outside this plan's
   three is another session's in-flight work — do not fix it, re-run once, report it as
   an observation rather than a failure.*
2. The controller re-runs the full gate itself at the task's HEAD before marking the task
   complete, so a genuine regression cannot hide behind the same excuse.

**Severity:** med — cost one controller verification cycle and produced a report that
overstated the tree's health. The dangerous counterfactual is the inverse: an implementer
that "helpfully" fixes or reverts another session's in-flight file. Layer 1 forbids it
explicitly.

**Status:** mitigated — dispatch clause added for Tasks 2-6; the underlying race is
inherent to sharing a checkout and is not fixed.

**Fix idea / Pointer:** The structural fix is a git worktree per work stream, which this
session deliberately declined — `docs/architecture/companion-plugin.md` §"Concurrent
multi-workspace" means worktree subagents must pin `workspace=<abs path>` on every
codescout call, and `worktree-write-guard.sh` hard-denies write tools until an activate
clears `.cs-worktree-pending`. That per-dispatch cost was judged higher than this
per-incident one. Revisit if a second incident lands, or if an implementer ever *acts*
on a foreign failure rather than reporting it. Related: [[F-52]] (same work stream);
ledger `.superpowers/sdd/2026-08-18-guide-ledger-phase-a-storage/progress.md` Ruling 7.

## W-46 — Rehoming state from per-project to per-user silently de-hermeticises the tests that relied on the project path

**Observed:** 2026-08-18, pre-dispatch reconnaissance for Task 6 of
`docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md` (the guide-ledger
Phase A plan), immediately before dispatching the final task's implementer.

**Pattern:** When a change moves a component's storage from a **per-project** path to a
**per-user / global** one, the per-project path is often the *only* thing making the
existing tests hermetic — silently, because no test says so. Before dispatching, read the
tests that construct the component and ask what supplies their isolation today. If the
answer is "the tempdir that the production path happens to be derived from", the change
removes their isolation and the plan needs an injection seam, not just a new path.

Concretely, scout three things:
1. the production binding being replaced, and what it derives from;
2. every test that builds the component, and where *its* copy of that value comes from;
3. whether the project forbids the obvious test escape hatch (here, a "no `std::env::set_var`
   in tests" rule, so `$XDG_STATE_HOME` could not be redirected).

**Counterfactual:** The plan's Task 6 said, verbatim:

```rust
let guide_hints_dir = crate::util::fs::per_user_state_dir()
    .map(|d| d.join("codescout").join("guide_hints"));
```

Today's binding derives from `guide_project_root`, which in tests is a
`tempfile::tempdir()`. The replacement reads the developer's real `$XDG_STATE_HOME`/`$HOME`.
`guide_hint_tests::guide_ledger_survives_mcp_restart` (`src/server.rs:4328`) pins the
session ids `"restart-survival-session"` and `"other-session"` so both incarnations key on
one file, and its **first** assertion is `!…contains("librarian")` — "a fresh session starts
with an empty ledger". Under the plan's code that file becomes
`~/.local/state/codescout/guide_hints/restart-survival-session.json` and survives the run,
so the test **passes once and fails on every subsequent `cargo test`**. A self-poisoning
test is the worst failure shape available: green in the session that writes it, red for
whoever pulls next, and it looks like *their* change broke it.

Two aggravating factors the same scout surfaced: every other server-building test would
write into one shared real directory; and because Task 5 had just made `GuideLedger::load`
run a 35-day GC over its directory, the test suite would have been pointing a file-deleting
sweep at the developer's real state directory on every server construction.

**Confirming data points:**
1. This session (Task 6, guide-ledger Phase A) — caught pre-dispatch; the fix was to add
   `guide_hints_dir: Option<PathBuf>` to `ServerEnv`, whose own doc comment
   (`src/server.rs:55-63`) already declares it to be "everything `from_parts` would
   otherwise read from the process environment, captured as data so tests can inject it".
   The seam already existed; the plan just did not use it.
2. Same shape as [[F-3]]/[[W-2]] (2026-05-18): a pre-dispatch scout of the *tests* rather
   than the production code caught a plan defect that would have cost subagent round-trips.

**Impact:** high — the defect is not a compile error a subagent would bounce off; it
produces a passing first run. It would have shipped, and surfaced days later as an
unreproducible-on-my-machine failure.

**Promote-when:** a third instance of "a plan's new production path silently removed a
test's isolation" lands. At that point promote to CLAUDE.md as: *before changing where a
component stores state, read the tests that build it and name what supplies their
isolation.* Related: the `ServerEnv` injection pattern is already the project's answer and
is documented on the struct — the gap is remembering to look for it.

**Status:** validated — plan defect caught and corrected in the dispatch (controller
Ruling 21) before any implementer ran.

## F-55 — `grep(glob=<abs path outside the project>)` returned a lying zero, and the warning named a cause it had not checked

**Observed:** 2026-08-18, pre-plan reconnaissance for Phase B of the guide-ledger
spec, scouting the companion plugin's `hooks/lib.mjs` from the codescout project.

**When:** About to write a Phase B implementation plan whose §6 hook work lands in
that exact file. Needed to know which helpers already exist.

**Expected:** Either matches, or an error saying the target is outside the active
project.

**Got:** `0 matches`, plus a warning attributing the zero to unsearched hidden
directories and recommending `include_hidden=true`. Both are wrong here: the
target is not hidden, and the flag cannot help. The control call —
`grep(pattern=..., path=<the same absolute path>)` — matched on the first try.
The file exports 8 functions.

**Probable cause:** `glob` is matched against candidates produced by a walk rooted
at the active project, so an absolute foreign path can never match; `path` resolves
directly and escapes the root. Measured as a boundary (the A/B pair), not yet read
in the source. Compounding it, the hidden-paths warning appears to fire on *any*
zero rather than only when hidden pruning could explain it.

**Workaround:** Use `path=` for a single foreign file; `run_command` + shell `grep`
for a cross-repo sweep; or activate the other workspace.

**Severity:** med — a false negative that reads as a finding. Had it stood, the
Phase B plan would have been written believing `lib.mjs` exported nothing, and the
hook task would have duplicated `readInput`, `emit`, `detectFor` and `git` instead
of reusing them. This is the reconnaissance skill's own "a search that finds nothing
is evidence about the search, not about the world" — hit while running that skill.

**Status:** fixed-verified (2026-08-19) — promoted to
`docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md`
and fixed there in `c38bfd91` (patch-id `913ba9c70e0b`).

The first half of this entry's *Fix idea* shipped, with one correction the source supplied:
the boundary is the **search root**, not the project root — which is exactly why `path=`
works, since it makes the target the root instead of filtering a walk over a different one.
An absolute `glob` outside that root is now a `RecoverableError` naming `path=`.

The second half did **not** ship: the hidden-paths warning is still ungated. It is carried
as the bug's `unverified:` caveat, with a note that narrowing it should be measured first
rather than guessed — a warning narrowed on a guess is the same defect pointed the other
way.

**Fix idea / Pointer:** Reject an absolute `glob` outside the project root with a
`RecoverableError` naming `path=` as the remedy, and gate the hidden-paths warning
on hidden pruning actually being able to explain the zero.

## W-47 — Measuring the process tree beat reasoning about it: the rendezvous spec's key assumption held, and a second one it never mentioned did not

**Observed:** 2026-08-18, scouting §6 (companion rendezvous) of the guide-ledger
spec before writing the Phase B plan.

**Pattern:** When a design's safety rests on a claim about *runtime* process
topology — "the server's ppid is on the hook's ancestry" — walk the live tree and
count the population, rather than reasoning from how the processes are supposed to
be spawned. Both the confirmation and the surprise come from the same one-command
measurement.

**Counterfactual:** The spec's stated assumption held — this session's server
(`pid=1844342`) has `ppid=2417823`, the `claude` process that also spawns the hook,
so ancestry-based selection works. But the same walk showed **25 codescout processes
alive on this host**, two of which (`2220116`, `2280210`) are `codescout mux` LSP
multiplexers whose parent is *another codescout*, not `claude`. Those are invisible
from the spec, which describes one server per hook. Without the measurement the plan
would likely have published the rendezvous entry from a shared construction path,
giving every mux process a `<pid>.json` that no hook can ever match — permanently
stale entries, indistinguishable from dead servers, in a directory whose GC policy
Phase B still has to write. The population count (25 live, 22 `claude`) is also the
sizing input for that GC, and it was not in the spec either.

**Confirming data points:**
1. F-53 (2026-08-18) — a PPID-derived session key was proposed without enumerating
   client session-resume; the refuting evidence was already in hand and unread. Same
   root: reasoning about process identity instead of measuring it.
2. W-47 (this entry) — measuring first confirmed one assumption and refuted the
   completeness of another, before any plan text existed.

**Impact:** med-high — prevented a stale-entry class in a new on-disk directory,
and supplied the GC sizing the spec omits.

**Promote-when:** A third process-topology assumption is settled by measurement
rather than argument. At that point promote to the reconnaissance skill as a named
seam class: *runtime process topology is a seam; walk the tree before designing
against it.*

**Status:** validated — measurement taken this session, findings feed the Phase B
plan directly.

## F-56 — Gating inside the OS temp dir reds three tests by breaking a precondition they state in their own comments

**Observed:** 2026-08-18, re-running the pre-promotion code gate for the 1043-commit
`experiments` cohort.

**When:** The main checkout carried a peer session's uncommitted `server.rs` /
`guide_ledger.rs` edits, so the gate was correctly moved to an isolated clean clone. The
session scratchpad was the obvious home for it, and the scratchpad lives under `/tmp`.

**Expected:** A gate result that describes the branch.

**Got:** `cargo test --workspace` red — 3 failures out of 4005 — and, because cargo stops
at the first failing target, the integration tests never ran at all. The three were
`create_refuses_temp_workspace_into_real_catalog`,
`reindex_refuses_temp_root_into_real_catalog`, and
`wrapper_refuses_temp_workspace_into_real_outside_temp_catalog`.

**Probable cause:** Not probable — *stated*. All three construct their "real" catalog with
`tempfile::TempDir::new_in(std::env::current_dir())`, and each carries the comment
*"(Assumes the repo checkout is not itself under the OS temp dir, which holds here.)"* A
checkout under `/tmp` collapses the temp-workspace-vs-real-catalog distinction the guard
discriminates on, so the guard correctly does not fire and the refusal the test asserts
never happens. Re-run from a `git worktree` under `/home`, all three pass — verified as
`... ok` lines, not as an absence of failures, so they demonstrably ran rather than being
filtered out.

**Workaround:** Gate from a checkout outside the OS temp dir. `git worktree add --detach`
under the repo is cheapest and satisfies the precondition by construction.

**Severity:** med — the failure mode is a **false red**, which is more dangerous than it
first appears. A red reads as the *safe* error, so the natural response is to report "the
tip is red, promotion blocked" and stop; that manufactures a blocker out of the harness and
spends a session hunting a defect that does not exist. It also masks: fail-fast meant 21
other test binaries never ran, so the same result was falsely negative **and** silently
incomplete. `--no-fail-fast` fixes only the second half.

**Status:** wontfix-false-alarm — the tests are correct, their precondition is documented,
and the defect was in the choice of gate location.

**Fix idea / Pointer:** Nothing to fix in code. Worth one line in `docs/RELEASE.md`'s gate
section: run the pre-promotion gate from a checkout **outside** the OS temp dir, because
three librarian temp-write-guard tests silently invert there. Sibling of
`bug-fix-session-log:F-55` and of `bug-fix-session-log:F-54` — all three are results that
describe the *harness* while reading as facts about the *branch*, which is the recurring
shape in this stream.

## W-48 — A review found 5 of 11 instances of a doc-defect class; asking "by what method did you enumerate?" is what exposed the other 6

**Observed:** 2026-08-18, guide-ledger Phase C Task 2. A one-line predicate change
(`emitted.is_empty()` → `!emitted.contains(SESSION_OPENING_GUIDE)`) invalidated documentation
that named `is_empty()` as the session-opener's trigger.

**Pattern:** When a change invalidates a *rationale* rather than a fact, treat the affected
comments as a **defect class** and sweep the corpus — then ask the implementer **by what method**
it enumerated, and require a corpus-wide search rather than accepting a count. A partial sweep is
worse than none, because corrected comments clustered together read as finished.

Two sub-rules the datapoint produced:

1. **A found instance is evidence about the search, not the corpus.** Round 1's extra find came
   from a *targeted read* (tracing `notice_once` callers). It could not have found a site
   unrelated to `notice_once` — and two of the five later found were exactly that. The count
   looked like diligence; the method was the thing that mattered.
2. **Classify each site void-rationale vs merely-stale.** *Void-rationale* states a mechanism now
   factually false, and is the dangerous kind because a wrong reason invites action — e.g. the
   comment documenting why `notices` is a **separate set** from `emitted` ("a sentinel key would
   silently suppress `SESSION_OPENING_GUIDE`") became untrue, so the design stayed right while its
   justification vanished, inviting someone to collapse the two sets. *Merely-stale* keeps a
   correct conclusion and only misnames the mechanism — e.g. the `tick()`-ordering constraint,
   still necessary, still forbidding the same reordering, just naming a function that moved.

**Counterfactual, measured:** the Opus task review — which applied six mutations and found no
surviving mutants — listed **5** sites. The implementer's targeted read found a 6th. The
corpus-wide sweep found **5 more**, for 11 total. So the review caught **45%** of the class, and
a review good enough to leave zero coverage gaps in the *code* still under-counted the *docs* by
more than half. Without the sweep, six wrong rationales would have survived in the exact files
the next task's implementer reads while working on the sibling API.

**Confirming data points:**
1. W-9 (2026-05-15) — spot-check sibling callers of a just-fixed shared helper before closing the
   bug class. Same principle, code side.
2. R-3 / R-113 / R-77 / R-79 in `docs/trackers/reconnaissance-patterns.md` — "a search that finds
   nothing is evidence about the search, not about the world." This entry is the positive form:
   a search that finds *something* is also evidence about the search.
3. W-48 (this entry) — 11 sites, 5 found by review, method-questioning found the rest.

**Impact:** med-high — documentation-only blast radius, but the void-rationale sites actively
invite a wrong refactor, and one of them guarded a design decision (`notices` as a separate set)
whose justification would otherwise have silently disappeared.

**Promote-when:** a second change invalidates a rationale cluster and the method-question again
surfaces sites a review missed. At two datapoints, promote to CLAUDE.md as: *when a change makes a
stated rationale false, sweep the class corpus-wide and require the enumeration method, not the
count.*

**Status:** validated — single strong datapoint with hard numbers (5 of 11), pending a second.

## F-57 — A concurrent session's commit swept up this session's uncommitted code edits under an unrelated message

**Observed:** 2026-08-21, mid-fix on the `audit_doc_refs` MCP-method-name bug
(`docs/issues/2026-08-20-audit-doc-refs-reads-mcp-method-names-as-file-paths.md`). After
`cargo fmt` and launching clippy + the full test suite in the background, ran `git status
--short` before committing, as this project's git-safety discipline requires before every
commit.

**When:** Checking `git status` right before staging — the step that catches this, not a
speculative check.

**Expected:** `src/librarian/tools/audit_doc_refs/resolver.rs` and `severity.rs`, edited via
`edit_code` this session and never committed, to show as `M` (modified, unstaged).

**Got:** `git status --short` showed only the bug-tracker doc file as modified. The two Rust
files were already byte-identical to my working tree — because they were already in `HEAD`,
swept into two commits (`89eb83f0`, `f24c3788`) by a **concurrent Claude Code session**
(same author name, but co-authored by "Claude Opus 5 (1M context)" — a different model/session
instance, working unrelated `docs/superpowers/specs/` entry-validity work on the same
checkout) that ran `git commit` while my edits sat uncommitted in the shared working tree.
Confirmed via `git show <sha> -- <file>` that the landed content is exactly what this session
wrote and tested — no data loss — but the commit messages and `Co-Authored-By` trailers now
misattribute an unrelated fix, and there is no single clean commit to cite as "the fix" in the
bug file (`git show <sha> | git patch-id --stable` needs one).

**Probable cause:** the concurrent session almost certainly staged broadly (`git add -A` or
`git commit -a`) rather than explicit paths — exactly the hazard `docs/RELEASE.md` §
Concurrent-Work Rules already documents, and the same class as the prior instance recorded in
`prompt-surface-compaction-session-log:W-9` ("on a checkout shared with at least two other
agent sessions"). That instance's mitigation (`git add <explicit paths>` rather than `-A`)
already protected *this* session's own commits throughout — the collision originated on the
other session's side, which this session cannot control.

**Severity:** med — no data loss and nothing gated on it, but real recovery cost: git-log
archaeology to find where the code actually landed, and a reconstructed patch-id (`git diff
<base>..<final> -- <files> | git patch-id --stable`, since no single commit is the fix alone)
in place of the normal single-SHA citation the bug-tracker convention expects.

**Workaround:** Before every commit, `git status --short` (already this session's practice) —
if a file you edited but never committed is missing from the diff, `git log --stat` on recent
commits to find where it landed, then `git show <sha> -- <file>` to verify content integrity
before citing it. Never assume "not in `git status`" means "not yet done."

**Status:** mitigated — verified content integrity, cited both commits + a reconstructed
patch-id in the bug file, informed the user. Root cause (concurrent sessions on one checkout
can still absorb each other's uncommitted work) is a property of shared-checkout multi-session
use, not something one session's discipline alone can close.

**Valid:** dated 2026-08-21

True of this checkout's concurrent-session state at this moment; not a claim about any other
repo or session.

**Rests on:** `docs/RELEASE.md` § Concurrent-Work Rules; `prompt-surface-compaction-session-log:W-9`; kin to [[F-54]] in this same ledger — a dispatched subagent reading a concurrent session's mid-edit working tree. Same root cause (shared checkout, no isolation), different manifestation (there: a false test failure; here: a true fix silently misattributed).

**Fix idea / Pointer:** None owed — the existing explicit-path-staging discipline is the
correct standing mitigation and was already in effect. No new rule to promote.

## F-58 — A same-day concurrent commit made a bug's own prescribed fix wrong before I got to it

**Observed:** Bug `93a9e4055b10eb7a` (doctor.rs comment misnaming `entry_cite`'s writer) was
filed 2026-08-20 with root cause "`link_scan` never writes `entry_cite`" (grep evidence: 0
matches) and a `## Resume` instruction to rename `link_scan` to `append_entry` in the comment.
Re-grepping before editing (per [[reconnaissance]]) found 13 matches for `entry_cite` in
`link_scan/mod.rs`, including a real `entry_cite::insert_with(...origin=ORIGIN_SCAN)` call site.

**When:** 2026-08-21, picking up the bug from the open queue during a multi-bug sweep.

**Got:** Between the bug's filing and my session, a *different concurrent session* shipped
`b750419a` / `1b19e0db` / `383b394e` — giving `link_scan(write=true)` a real `entry_cite`
write path (origin=`scan`, pruned/re-derived per pass) that didn't exist when the bug was
filed. The bug's own prescribed fix ("`link_scan` never touches it, say `append_entry` is
the sole writer") would have been **newly wrong** if applied verbatim — `link_scan` now does
write it, just not as the sole writer, and not without the staleness caveat the comment
already made (now correctly, for the scan-origin rows specifically).

**Probable cause:** Multiple concurrent sessions actively shipping in the same area
(`entry_cite` / entry-graph machinery) during the same day. A bug's root-cause section is a
claim about the substrate *at filing time*; nothing re-validates it before someone acts on it.

**Severity:** low — caught before the wrong fix landed, cost one extra grep + read pass.

**Workaround:** Re-run the bug's own reproduction grep before trusting its root cause or
`## Resume` instructions, even for same-day bugs — "same day" is not "same commit."

**Status:** mitigated — no tooling change, just re-confirms CLAUDE.md's "run the reproduction
before reading the fix plan" rule holds even at same-day timescales when multiple sessions
are concurrently shipping in the same subsystem. See [[F-57]] for the sibling concurrent-
session hazard (commits landing under me) this session already hit twice.

**Valid:** dated 2026-08-21

## F-59 — A filed bug's own root cause was wrong ("desktop-only remote host"); reproduction found a stale global symlink instead

**Valid:** dated 2026-08-25

**Observed:** 2026-08-24/25, session verifying the statement-validity/provenance implementation on a laptop distinct from the machine that built the existing index.

**When:** Chasing an open bug (`docs/issues/archive/2026-08-23-index-build-fails-embed-batch-sparse-send.md`) whose own Root cause section was explicitly flagged `unverified: inferred from source, not measured over the network`.

**Expected:** The bug's stated hypothesis — `CODESCOUT_SPARSE_EMBEDDER_URL` pointed at a host/port only reachable from the original (desktop) machine — was the fix target.

**Got:** The URL was loopback (`127.0.0.1:48084`), not remote. Reproducing end-to-end (curl the port, `docker ps -a`, then tracing `load_startup_env()` in source) found the real chain: the machine-wide `~/.config/codescout/.env` symlink pointed at a stale, partially-removed compose profile (`.env.amd`) that re-enabled sparse embedding, contradicting the actually-running `gpu` profile, which keeps the sparse container stopped by design. Fixing the symlink (not the URL) resolved it.

**Probable cause:** An `unverified:` root cause written from source-reading alone, without a network trace, was nearly trusted as fact before reproduction corrected it.

**Workaround:** N/A — this is the general lesson, not a live issue.

**Severity:** med

**Status:** fixed-verified

**Fix idea / Pointer:** `docs/issues/archive/2026-08-23-index-build-fails-embed-batch-sparse-send.md` (root cause section rewritten with the corrected chain and citations).

---

## W-49 — Checking ListAgents/SendMessage before committing shared uncommitted docs avoided a collision with two concurrent peer sessions

**Valid:** dated 2026-08-25

**Observed:** 2026-08-24, mid-session, asked to commit ~40 files of pending tracker-hygiene-sweep output sitting uncommitted in the working tree, ownership unknown.

**Pattern:** Before committing someone else's uncommitted work, ran `ListAgents`, found two other active codescout sessions in the same repo (`codescout-ee`, `codescout-0e`), and pinged both via `SendMessage` asking whether the pending diff was theirs before touching it.

**Counterfactual:** Both peers turned out to say "not mine, go ahead" — but the check caught something real in the process. `codescout-0e` had *already* committed one of the files (`src/librarian/catalog/augmentation.rs`) between an earlier `git status` read and the eventual commit attempt, confirmed live when a second file (`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`) visibly vanished from `git status --short` mid-conversation as `codescout-ee` committed it. A blind commit at that point would have either failed confusingly on the first file or silently swept a stale version of the second, which a peer was actively appending to.

**Confirming data points:** (1) `augmentation.rs` vanished from `git diff` between two checks, traced to `codescout-0e`'s commit `a03b54b0`; (2) the subagent-activate bug file vanished from `git status --short` mid-turn, traced to `codescout-ee`'s commit `8db6cf3d`; (3) `codescout-0e` separately flagged a real `.gitignore` gap (`.codescout/librarian.db*` uncovered, unlike its sibling catalogs) during the coordination reply, which this session would not have found unprompted.

**Impact:** high

**Promote-when:** A second independent work stream hits the same shape (about to commit uncommitted state of unknown ownership while other sessions are active in the same repo) and the check pays off again — target `docs/RELEASE.md` § Concurrent-Work Rules, which documents git-state safety but not this specific "ask before committing" step.

**Status:** validated

---

## F-60 — Even after ListAgents/SendMessage coordination, a peer's routine commit silently absorbed this session's uncommitted append_entry writes

**Valid:** dated 2026-08-25

**Observed:** 2026-08-25, immediately after resuming from context compaction, while verifying the F-59/W-49 commit (`1e5069d0`) this session made minutes earlier.

**When:** `git show --stat 1e5069d0` reported only 2 insertions — far too few for two full prose entries — prompting a `git log -S "## F-59"` search across the file's history.

**Expected:** The F-59/W-49 entry bodies (47 lines including the frontmatter high-water bump) to be committed under `1e5069d0`, the commit this session authored specifically for them.

**Got:** They were already committed one commit earlier, under `a468e69d` — a peer session's commit titled "F-10 — Task 3's veto keys on a per-arm env var run_arms.py cannot set", touching this file *and* `prompt-surface-measurement-session-log.md` (the peer's actual, unrelated F-10 entry). The peer's own `git add`/`commit -a` swept up this session's already-on-disk-but-unstaged `append_entry` write before this session got to stage and commit it. `1e5069d0` then carried only the two Index/Wins-Index table rows added afterward.

**Probable cause:** `append_entry` writes straight to the file on disk, bypassing git staging. The F-59/W-49 coordination check (ListAgents/SendMessage before committing) only guards against *this* session committing *someone else's* uncommitted work — it does nothing to stop a peer's own routine commit from staging a directory or running `commit -a` and incidentally absorbing *this* session's unstaged edits to a shared file. Both sessions operate on one working tree, not isolated worktrees.

**Workaround:** `git add <specific file>` and commit immediately after any `append_entry`/`edit_markdown` write to a shared tracker, instead of batching several tracker edits before committing — the batching window is exactly what let the peer's commit land first.

**Severity:** low — no content was lost or corrupted; both entries are intact and correctly formatted. The only casualty is provenance: `git log`/`git blame` on the F-59/W-49 section now points to a commit message about an unrelated topic, breaking the "commit message names the entry" convention this file's own entries otherwise rely on for traceability.

**Fix idea / Pointer:** Commit each tracker append immediately rather than batching; a stronger structural fix (session-scoped worktrees) is out of scope for this session — see `docs/RELEASE.md` § Concurrent-Work Rules.

**Status:** mitigated

## F-61 — "No live-pyright harness exists" was a negative search of ONE file, and it nearly shipped a fix with no regression test

**Observed:** 2026-08-26, finishing the fix for `docs/issues/archive/2026-08-25-references-dead-ends-at-renaming-re-export.md`. Fix written, unit tests green, about to commit.

**When:** Choosing a test strategy for a fix whose entire behaviour is live-LSP integration.

**Expected (my claim to the user):** "There is no live-pyright harness in this repo — the Python tests in `src/tools/symbol/tests.rs` are synthetic `SymbolInfo` fixtures, so the alias path has no CI-enforced regression test." I asked the user to run `/mcp` so I could verify by hand instead.

**Got (scouted reality):** `tests/` holds a full declarative e2e harness — `tests/e2e/harness.rs`, `tests/e2e/test_python.rs`, and TOML expectation files at `tests/fixtures/core-expectations.toml` and `tests/fixtures/python-extensions.toml` — carrying **six existing live-LSP `find_referencing_symbols` expectations**, one of which (`refs_reexported_class`) already covers a *non-renaming* re-export. Enabling coverage took one TOML block plus three fixture files, and produced a clean RED (`FAIL refs_renaming_reexport_alias: symbol not found: apply_duty`, 24/25, 90s real LSP run) → GREEN (25/25) around the fix.

**Probable cause:** I grepped `src/tools/symbol/tests.rs` — one file — for `pyright|\.py"`, saw synthetic fixtures, and generalised to the repo. The reconnaissance law names this exactly: *a search that finds nothing is evidence about the search, not about the world*, under its **scope** mechanism ("search the tree, not the file you are editing"). Worse, this project's own `cargo-test-lib-skips-integration` memory prescribes the precise check I skipped — `grep -rn "<symbol_you_changed>" tests/` — and that memory was listed at session start.

**Workaround:** None needed. Added `[refs_renaming_reexport_alias]` to `tests/fixtures/python-extensions.toml` plus `library/pricing/__init__.py`, `library/pricing/duties.py` and `library/orders.py` — all NEW files, so no existing expectation's containment assertions could shift.

**Severity:** high — the fix would have shipped with zero coverage of the behaviour it changes, under an `unverified:` marker asserting that coverage was *impossible*. The marker is the durable damage: it tells every future reader not to look, which is worse than silence.

**Status:** fixed-verified — regression test lands with the fix; red and green both measured.

**Valid:** dated 2026-08-26

**Rests on:** the `cargo-test-lib-skips-integration` memory's rule, and the reconnaissance skill's negative-search law (scope mechanism).

**Fix idea / Pointer:** When about to write `unverified:` because "no harness exists", treat that sentence as the trigger to search `tests/`. An assertion that testing is impossible is a claim about the repo, and by the skill's own deferral-rationale law it is the least-audited kind — nobody re-checks a reason for not doing work.

## F-62 — All five feature-gated e2e language lanes had stopped compiling, invisibly, because `cargo test` never builds them

**Observed:** 2026-08-26, while enabling the Python e2e lane to host a regression test (see F-61).

**When:** First `cargo test --features e2e-python --test e2e_tests` of the session.

**Expected:** The lane runs, or fails on my new expectation.

**Got:** It did not compile at all.

```
error[E0063]: missing field `workspace_override` in initializer of `codescout::tools::ToolContext`
  --> tests/e2e/harness.rs:43:14
```

`ToolContext` gained a `workspace_override` field at some point; `tests/e2e/harness.rs` was never updated. Because `tests/e2e/mod.rs` gates every language module behind `#[cfg(feature = "e2e-rust" | "e2e-python" | …)]`, a plain `cargo test` never compiles the harness — so the breakage produced no error anywhere, for however long it has been there. All five lanes (rust, python, typescript, kotlin, java) share that harness, so all five were dead simultaneously.

One field fixed it. `cargo test --features e2e --no-run` now builds every lane, and `--features e2e-python` runs 24/24 green on the pre-existing expectations.

**Probable cause:** A structural blind spot rather than an oversight — the gate that would have caught it is excluded from the command everyone runs. This is the reconnaissance skill's *"a green result certifies the path that actually EXECUTED"* law in its **configured-out** form: `cargo test` was green through every commit that broke this, and would read the same in a fully broken world.

**Workaround:** N/A — fixed in place.

**Severity:** high — not for the missing field, which is trivial, but for the duration and the silence. Every live-LSP behavioural assertion in the repo (24 for Python alone, plus four other languages) was not merely unrun but unbuildable, while `cargo test` reported green. Any regression those lanes exist to catch has been uncaught for the whole window.

**Status:** fixed-verified — the one-field fix (`bd293162`) restored all five lanes, python 25/25; the recurrence guard shipped 2026-08-26 (`3784cb65`).

**Valid:** dated 2026-08-26

**Rests on:** the cfg-gating in `tests/e2e/mod.rs` and the feature list in `Cargo.toml:209-211`.

**Fix idea / Pointer:** **SHIPPED 2026-08-26** — `3784cb65` on `experiments`, patch-id `91594f7eada1d5747a9b5683b96a37753f69998f`.

The pointer was right that `tests/feature_lanes.rs` was the intended home, and checking it first — rather than adding a new guard — found something better than a missing step. **The guard already covered these features. They were `EXEMPT` entries.** Its doc comment defined an exemption as *"this cannot be built on a CI runner"*, while every reason in the list actually names a missing language server — a reason not to *execute*. `cargo check` needs none of them. So the lanes were never overlooked by the guard; they were let through its escape hatch, behind a rationale that read as a settled decision and was really a conflation of *run* with *compile*. That is the deferral-rationale law applying to a **guard's own exemption list**, which is a place worth watching: an exemption is the one kind of entry whose whole function is to stop anyone looking.

The project had already drawn the distinction once, for `retrieval-e2e` — exempt from running, compiled by the feature-check lane anyway, after 4 call sites rotted under a signature change. That reasoning sits in the CI job's own comment, two lines above where the e2e lanes should have been.

What shipped: `cargo check --features e2e --all-targets` in the `feature-check` job (the meta-feature covers all five); `every_exempt_feature_is_still_compiled_somewhere`, which fails if any exemption loses its compile lane and resolves meta-features transitively via `closure_from`; and a rewritten `EXEMPT` doc comment that says "not run" rather than "cannot be built", and names why the two differ.

Verified against the real defect rather than against its own green: with `workspace_override` removed again, `cargo test --test feature_lanes` still reported **3 passed** while `cargo check --features e2e --all-targets` failed with `E0063`. The structural test proves a lane is *declared*; only the lane proves the code *compiles*. Neither substitutes for the other, and the structural test reading green over broken code is this entry's own law in miniature.

## W-50 — Recon run at the commit boundary converted "unverifiable, ask the user to /mcp" into a measured red→green

**Observed:** 2026-08-26, invoked at the point where the `references` alias fix was written, gate-green, and about to be committed.

**Pattern:** Run reconnaissance at the **commit boundary of your own fix**, and aim it at the sentences you are about to write into the durable record — not at the code. The seam here was not a struct shape; it was three claims I had already drafted for the user and for an `unverified:` field: *no harness exists*, *the green covers this*, and *therefore verify by hand*. Each is a checkable assertion about the repo, and all three were false.

**Counterfactual:** Without the scout I would have committed a fix whose behaviour had **no** automated coverage, stamped `unverified:` with a claim that coverage was impossible, and handed the user a manual `/mcp` verification step. Concretely lost:

1. The `refs_renaming_reexport_alias` expectation — a real live-LSP regression test, measured RED (`symbol not found: apply_duty`, 24/25) before the fix and GREEN (25/25) after.
2. F-62 — five e2e language lanes that had stopped compiling entirely and were reporting nothing. Found only because I went looking for a harness I had asserted did not exist.
3. The `unverified:` marker itself, which would have instructed every future reader that this path *cannot* be tested here.

**Confirming data points:**
1. This session (F-61, F-62) — two false claims and one dead test suite, all surfaced by one scout at the commit boundary.
2. Same session, earlier: `references` returned `0 references` for `store_file` with a warming-index warning; corroborating with `grep` found 24 call sites across 6 files. The tool's own warning prompted the check — the same reflex, prompted externally rather than by the skill.

**Impact:** high — the difference between a fix with a regression test and a fix with a durable note saying it cannot have one.

**Promote-when:** A third instance where a commit-boundary scout invalidates a *claim about the repo* (as opposed to a claim about code shape). At three, promote to the skill's Phase 1 as an explicit bullet: *"Before writing `unverified:`, or telling the user something cannot be tested/measured here, scout that claim — an assertion of impossibility is a claim about the repo, and nobody re-audits a reason for not doing work."* Note this is close to the existing deferral-rationale law; the promotion may be an **outgrown** re-promotion of that law rather than a new one, since that law is scoped to a `## Fix` section's rationale and this instance was a live claim to the user. Audit it against the four staleness classes before adding.

**Status:** validated — two datapoints this session, both with measured counterfactuals.

**Valid:** dated 2026-08-26

**Rests on:** F-61 and F-62, same session — this win is their shared counterfactual, not independent evidence.

## W-51 — A filed root cause about STORED state is refuted by querying the store, not by re-running the tool that reported it

**Valid:** dated 2026-08-25

**Observed:** 2026-08-26, picking up `docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md`, whose § Root cause read "**Established.**" and whose title claimed every augmentation in the codescout catalog was gone.

**Pattern:** CLAUDE.md's "run the reproduction before reading the fix plan" — with the sharpening that when the claim is about *persisted state*, the reproduction is a query against the datastore, not a re-run of the tool that reported it. Re-running `find(augmented=true)` would only have re-asked the instrument that produced the false negative. Six SQL queries against `~/.local/share/librarian/catalog.db` settled it in about two minutes: per-row `created_at`/`updated_at`, a date histogram, the `upsert` conflict clause, a backup's row count, and `worktree_registration`.

**Counterfactual:** The filed fix was a one-time `artifact_augment(id=…, prompt=…, params={entries: […]}, render_template=…, entry_collection=…)` restore. Run against the 21 trackers that were *already* augmented, that call replaces params wholesale — `merge=false` resets all seven augmentation fields — and CLAUDE.md records this exact call taking the T-N queue from 19 entries to 1 on 2026-08-16. Restoring a loss that had not happened would have **caused** one, starting with `f2ecdd76a6189efb`'s live `observations` collection, and the catalog is not in git so nothing recovers it. The escalation also had reach beyond this file: it declared three CLAUDE.md-documented workflows "impossible right now", which is a standing instruction to future sessions not to attempt them.

**Confirming data points:**

1. `f2ecdd76a6189efb`'s augmentation row: `created_at = 2026-07-05T06:51:44Z` — seven weeks before the reported loss — with `observations` intact today.
2. `augmentation::upsert` stamps `updated_at` on conflict and never `created_at`, so the row cannot have been re-inserted later wearing an old date.
3. No codescout augmentation row carries a `created_at` **or** `updated_at` on 08-22/23/24; a restore would have stamped all 21 at once.
4. `~/.sync-backups/…/20260712/catalog.db` holds 53 augmentations against today's 70 — monotonic growth, no wipe.
5. Zero codescout rows in `worktree_registration`, ever, killing the overlay-shadow explanation the librarian guide would otherwise suggest.
6. The bug's own § Fix still read "Not yet planned" — nothing had restored the rows it claimed were destroyed, so their presence is not evidence of a repair.

**Impact:** high

**Promote-when:** a second filed root cause about persisted state is refuted by querying the store directly. Then promote into CLAUDE.md's reproduction rule as an explicit clause — *when the claim is about stored state, the reproduction is a query against the datastore, not a re-run of the reporting tool* — since the generic rule did not by itself say which instrument to reach for.

**Status:** validated

## F-63 — Reported "2 open bugs" four times from a query whose filter was the thing hiding the pile

**Observed:** 2026-08-26, across the whole session — every "what's open?" report I gave the user.

**When:** `artifact(find, kind="bug", filter={"status": {"in": ["open","investigating"]}})`, the query `get_guide("tracker-conventions")` and the activation bootstrap both prescribe as canonical.

**Expected:** a count of the live bug population.

**Got:** a count of the *non-terminal* population, which is a different thing. Counting `docs/issues/` by hand at the end of the session:

| | |
|---|---|
| files in `docs/issues/` | **23** |
| reported as open, four times | **2** |
| terminal-but-unarchived | **17** |
| `status: zombie` | **4** |

**Probable cause:** the query is correct and I read it as answering a question it does not ask. The filter names `open|investigating`; I heard "what is live". Those coincide only when the archive discipline is perfectly kept, which is exactly what the pile proves it was not.

Compounded twice more, both times the same shape:

1. Told the user "10 terminal-but-unarchived", derived from two doctor checks — whose SQL is `('fixed','mitigated','wontfix')` and `('fixed','mitigated')`. That count inherited *their* filters and missed the 4 zombies, which no query in the project reaches (fixed and archived same-session:
   `docs/issues/archive/2026-08-26-zombie-bug-files-are-reachable-by-no-query.md` — the
   canonical triage query now includes `zombie`).
2. The doctor's single `archived_fix_sha_unresolvable` finding, which I took at face value long enough to start working it, was a false positive — a cross-repo `<repo>:<sha>` pointer resolved against the wrong repo.

**Severity:** med — no wrong edit resulted, but the user was given a wrong number repeatedly and a triage decision ("2 open, both blocked, here's what else to do") rested on it.

**Status:** fixed-verified — the count is corrected, the zombie gap is filed, and 6 of the 17 are archived (`d5ed4d6f`).

**Valid:** invariant

**Rests on:** the status vocabulary having six values and every query naming a subset; true regardless of the current counts.

**Fix idea / Pointer:** The habit that would have caught it costs one command: when reporting a population, count the substrate once and reconcile it against the query. `ls docs/issues/*.md | wc -l` against `find(...)` disagreeing by 21 is not subtle — nobody had to know *why* to know something was wrong. Generalises past bug files: any report of the form "there are N X" derived from a filtered query owes one unfiltered count.

## W-52 — Measure the corpus before building a detector — the obvious heuristic was 23 false positives to 1

**Observed:** 2026-08-26, building the `augmentation_declared_but_absent` doctor check — the recurrence guard the research-index bug's own `unverified:` field said was missing.

**Pattern:** Before implementing a detector, run its predicate over the real corpus and count what it would fire on. Not a review of the idea, not a sample — the whole population, with the true/false split named.

The obvious implementation was to read each artifact's body for a `[LIVE]` claim: the research index's body promised a `[LIVE]` table it had no augmentation to render, so a body that mentions `[LIVE]` while unaugmented is exactly the defect. One script over all catalogued artifacts:

| | |
|---|---|
| artifacts scanned | 4,195 |
| mention `[LIVE]` outside fenced code | 29 |
| of those, carrying **no** augmentation | **23** |
| of those 23, actually defective | **1** |

The 23 are guides, specs, plans and bug files *describing the mechanism* — including the bug file for this very check. Intent is not recoverable from prose.

That killed the heuristic and pointed at the remedy the project already uses for the identical problem: `entry_prefix` is declared in committed frontmatter precisely because *"a declaration stored in the augmentation is absent in a fresh clone."* A ledger is declared, never inferred; so is an augmentation. Shipped as `expects_augmentation: true` (`dab5c530`), backfilled to all 22 (`d6d66e4c`).

**Counterfactual:** a check firing 23 false positives against 1 true one, in a rot-detector whose entire value is that nothing else re-reads `archive/`. It would have been switched off, or — worse — left on and learned-around, which is how a gate becomes decoration. And the measurement was ~40 seconds of scripting against a corpus already on disk.

**Confirming data points:**

1. This entry — 23:1, measured before a line of the check was written.
2. Same session, `structured_fix_pointers`: before skipping fenced lines I counted 75 declared pointer lines, 74 outside fences and 1 inside. That confirmed the fix loses no real declaration *and* sized the defect honestly as latent-not-live.
3. Same session, the rendezvous latch (`f1a1a59f`): the bug file's own sketched fix — a staleness bound on `hook_at` — was refuted by measuring `hook_at` across 5 healthy sessions (0.6h–25.0h, two of them predating their own process). No window separates "hook died" from "long conversation". The fix was closed off by data, not argument.

**Impact:** high — in all three, the measurement changed the design rather than confirming it, and in two of the three the "obvious" implementation was wrong.

**Promote-when:** a fourth instance where a pre-implementation corpus count changes a detector's design. At four, promote to CLAUDE.md beside the reproduction rule, as: *a detector's predicate is a hypothesis about the corpus — count what it fires on before building it, and report the true/false split, not just the hits.*

**Status:** validated — three datapoints this session, each with a measured counterfactual.

**Valid:** dated 2026-08-26

**Rests on:** [[F-63]] — the same session's failure mode inverted. F-63 is trusting a filter's output; this is measuring a predicate before trusting it.

## W-53 — Working an instrument's own worklist found two defects in the instrument before any of the work it listed

**Observed:** 2026-08-26, starting on the doctor's 69-violation report after extending doctor with a new check.

**Pattern:** When a scan hands you a worklist, verify the *first* finding against reality before acting on any of them — and treat a finding that turns out false as a bug in the instrument, filed and fixed, not as a row to skip.

The single `archived_fix_sha_unresolvable` finding said `codescout-companion:b8ffa8b` "no longer names an object in this repo". One command settled it:

```
git -C ../claude-plugins/codescout-companion cat-file -t b8ffa8b   → commit
```

It resolves, in the repo its own prefix names. The check had handed the whole `<repo>:<sha>` token to git, which reads `a:b` as *the object at path b inside tree-ish a*. It then offered recovery **by patch-id** — impossible across repos, and the file says so on the line directly below the SHA.

Verifying *that* turned up a second defect in the same parser: `structured_fix_pointers` scanned fenced lines too, so a file **quoting** a provenance block had the quotation counted as its own declaration. Self-demonstrating — the bug file I had just written for defect 1 reproduces the pointer it is about, and the scan counted it as a second declaration. Both fixed in `7b5325a9`.

**Counterfactual:** the alternative was to read the finding as true and go looking for a dead commit — a search the check's own text sizes at "2–153 ambiguous candidates". Worse is the path where I noticed it was wrong and simply moved on: a false positive left standing in a rot-detector teaches the reader to discount it, and this check exists precisely because *nothing else re-reads `archive/`*. A detector nobody believes is worse than no detector, because it also occupies the slot.

**Confirming data points:**

1. This entry — 1 finding checked, 2 defects found, 0 of the listed work done first.
2. Same session, [[W-51]]: the filed root cause of a data-loss bug was refuted by querying the datastore instead of re-running the tool that reported it. Same shape one layer up — the instrument's *output* was the thing to distrust.

**Impact:** med — neither defect was load-bearing today (the cross-repo case is one file; the fence case was latent), but both produce confident wrong answers in the one place the project relies on for rot detection, and both were cheap to close.

**Promote-when:** a third instance where checking a scan's first finding surfaces a defect in the scanner. Likely already at three counting [[W-51]] and the `find(augmented=true)` zero from the same session — but those are about *reading* an instrument's output, and this is about *acting* on a worklist; audit whether they are one law before promoting, rather than promoting a near-duplicate.

**Status:** validated

**Valid:** dated 2026-08-26

**Rests on:** [[F-63]] and [[W-51]] — same session, same family: an instrument's output is a claim about the world, not the world.

## F-64 — My own #18 bug file inflated its own fix — `code_vec` is per-project by FILE, so its "hardest part" is vacuous

**Observed:** 2026-08-26, pre-implementation scout of
`docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`
(GitHub #18, archived 2026-08-26 once fixed in `8504b1cf`) — a bug file I had
authored myself earlier in the same session.

**When:** About to implement the force-aware dimension migration, scouting
`src/retrieval/sqlite_code_store.rs` to decide how to scope the table drop.

**Expected (my own bug file):** Fix step 2 read *"Scope the drop to the project.
`src/retrieval/sqlite_code_store.rs` holds vectors for multiple projects; the
recreate must leave unrelated projects intact. **This is the part most likely to
be got wrong.**"* Its Resume section escalated that into the deciding question:
whether `code_vec` is *"shared across projects with `project_id` as a column —
that determines whether step 2 is a one-liner or the whole bug."*

**Got (scouted reality):** Neither branch of that question exists.
`SqliteVecCodeStore::conn_for` (`src/retrieval/sqlite_code_store.rs:58-77`) calls
`open_conn(&self.dir, &self.conns, project_id, ".db", …)` — **one SQLite file per
`project_id`**. `code_vec` is created inside that per-project DB and carries no
`project_id` column at all (`INSERT INTO code_vec (chunk_id, embedding)` at
`:205`; `DELETE FROM code_vec WHERE chunk_id = ?1` at `:236`; `query` reaches
project scope only by joining `code_chunk` at `:285-286`). Cross-project
isolation is a property of the **filesystem layout**, not of any query predicate,
so `DROP TABLE code_vec` under `conn_for(project_id)` cannot reach another
project. Confirmed on disk: `.codescout/embeddings/` holds `codescout.db`,
`project.db` and `codescout.memories.db` — one file per project id.

Step 2 is therefore vacuous. The residual question is far smaller and purely a
sqlite-vec detail: whether `DROP TABLE` on a `vec0` virtual table also drops the
shadow tables observed in the live DB (`code_vec_chunks`, `code_vec_rowids`,
`code_vec_info`, `code_vec_vector_chunks00`).

**Probable cause:** Written while triaging five GitHub issues in one pass, from
two plausible proxies instead of the code — `guard_index_dim`'s own error hint
(*"the vector table bakes the dimension in at creation and cannot migrate in
place"*) and the filename `sqlite_code_store.rs`, since a store *named* for one
backend reads as one shared store. I never opened `conn_for`. This is the
directional bias R-95/R-92 names, in its deferral form: the sentence was written
at the moment I decided to stop and defer, and no such sentence ever makes the
deferred work sound cheaper than it is.

**Workaround:** Corrected the bug file's Fix step 2 and Resume before writing any
implementation. Recorded the shadow-table question as the real residual.

**Severity:** med — I would have designed, implemented and tested a
project-scoping mechanism for an invariant the schema already guarantees. The
concrete waste: a `DELETE FROM code_vec WHERE chunk_id IN (SELECT chunk_id FROM
code_chunk WHERE project_id = ?)` variant in place of a plain `DROP TABLE`, plus
a "sibling project survives the migration" regression test. That test is the
worse half — it would have **passed for the wrong reason** (siblings live in a
different file, so every possible implementation passes it), making it exactly
the green-but-uninformative result this skill's Phase 1 warns about.

**Status:** fixed-verified — bug file corrected pre-implementation; no code was
written against the wrong model.

**Valid:** dated 2026-08-26

True of `SqliteVecCodeStore`'s one-file-per-project layout at `d5ed4d6f`;
re-verify if `conn_for` or `sqlite_vec_ext::open_conn` changes its keying.

**Rests on:** `open_conn`'s `project_id` + `".db"` arguments being the file key,
which is what makes cross-project isolation structural rather than query-enforced.

**Fix idea / Pointer:** `docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`,
Fix step 2 + Resume. Paired with the win recorded alongside this entry. The bug
itself shipped as `8504b1cf` (patch-id `be0fdace04f3c4aacef68af3efc7338056629a07`)
without the vacuous step and without the vacuously-passing test.

## W-54 — Scouting a self-authored bug file before implementing it refuted its central sizing claim — a form R-95 does not yet cover

**Observed:** 2026-08-26, immediately before implementing GitHub #18 from
`docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`
(archived once fixed in `8504b1cf`), authored earlier the same session.

**Pattern:** Before implementing from a bug file or plan **you wrote**, scout the
seam its Fix section *sizes* — not only the seam its Root cause cites. A sentence
of the form "this is the hard part", "N call sites", "one of two options, both
bad", "that determines whether this is a one-liner or the whole bug" is a claim
about the substrate wearing the clothes of a settled decision, and it is
load-bearing on the shape of the code you are about to write.

**Counterfactual:** Without this scout I would have implemented #18's Fix step 2
as written: a project-scoped `DELETE FROM code_vec WHERE chunk_id IN (SELECT
chunk_id FROM code_chunk WHERE project_id = ?)` in place of a plain `DROP TABLE`,
plus a "sibling project survives the migration" regression test. Both are dead
weight — `conn_for` gives every project its own `.db` file
(`src/retrieval/sqlite_code_store.rs:58-77`), so isolation is structural. The
test is the more expensive half: siblings live in a *different file*, so **every
possible implementation passes it**, including a wrong one. It would have entered
the suite as a permanent green-but-uninformative assertion — the exact failure
mode this skill's Phase 1 names — and future readers would have inherited the
false belief that `code_vec` is a shared table needing careful scoping.

**Confirming data points:**
1. F-64 (this session) — #18's self-authored "most likely to be got wrong" step
   was vacuous; `code_vec` is per-project by file.
2. Same session, triage of five GitHub issues: **two of five reports were wrong
   about their own mechanism**, and reading the code rather than the reports
   collapsed three of them (#15, #17, #19) into one root cause — a `?` on an
   embed call inside a loop that had already committed partial state. #17's
   reported symptom (`docs/` = 0 indexed) was not reproducible at all; #19's
   stated ordering ("fails before refreshing the catalog") was inverted. Neither
   was a self-authored artifact, so this datapoint supports the general law, not
   the authorship-specific one.

**Impact:** med — one avoided dead-code path plus one avoided permanently-green
test, per occurrence.

**Promote-when:** A second instance of a self-authored **sizing** claim (as
distinct from a *deferral* rationale) being refuted at the code. R-49 already
covers re-scouting a self-authored *root cause*, and R-95/R-92 covers re-costing
a *deferral rationale* — a claim written at the moment someone decided to stop.
Neither covers this shape: a sizing claim inside a Fix plan the author fully
intends to execute, where the same directional bias applies but the "decided to
stop" tell is absent. At 2 datapoints, extend R-95's law from "deferral
rationale" to "any sizing claim, deferred or not", since the remedy is identical
— re-run the number, read the consumer, probe the premise once.

**Status:** validated — single datapoint for the sizing-claim variant; drift
caught and the bug file corrected before any code was written.

**Valid:** dated 2026-08-26

One confirmed datapoint for the sizing-claim variant; the promote-when threshold
of 2 is not yet reached.

**Rests on:** R-49 (authorship is no exemption) and R-95/R-92 (a deferral
rationale is a claim) as the two adjacent promoted laws this variant falls
between — this win is the argument that the gap between them is real, not
independent evidence for either.

## F-65 — The reconnaissance skill's worked exemplars show a `**Valid:**` form the server refuses, and tell you to copy them

**Observed:** 2026-08-26, writing F-64 into this tracker via
`artifact(action="append_entry")` during a `/codescout-companion:reconnaissance`
run.

**When:** First `append_entry` call of the session, with the entry body patterned
on the skill's own worked exemplars as the skill instructs.

**Expected (skill exemplars):** `SKILL.md` § *Worked exemplars* shows both its
F-N and W-N examples declaring validity with a rationale appended after an
em-dash on the same line:

```
**Valid:** dated 2026-05-18 — true of `RecoverableError`'s field/method shape at that commit; re-verify if `src/tools/core/types.rs` changes.
```

and instructs *"Pattern your new entries on these, not the bare template."*

**Got:** The call was refused:

```
`**Valid:** dated 2026-08-26 — true of `SqliteVecCodeStore`'s one-file-per-project` is not an ISO date
hint: Use `dated YYYY-MM-DD`. The three forms are:
      **Valid:** invariant | dated YYYY-MM-DD | conditional — <event>
```

The parser consumes the **whole remainder of the line** after `dated` as the date
token, so any trailing prose fails calendar validation.

**Corroboration — the corpus agrees with the parser, not the skill.** Measured on
this tracker, 2026-08-26:

```
grep -c '^\*\*Valid:\*\* dated' docs/trackers/bug-fix-session-log.md          → 13
grep -c '^\*\*Valid:\*\* dated [0-9-]* *—' docs/trackers/bug-fix-session-log.md → 0
```

All 13 real declarations use the bare form. Zero use the exemplars' form. So this
is not a recent parser tightening that stranded old entries — the exemplar shape
has never been writable through `append_entry`, and every agent that follows the
skill's "pattern your entries on these" instruction burns a round-trip
discovering it.

**Probable cause:** The exemplars were transcribed into `SKILL.md` from prose
drafts rather than from entries the server had accepted, and nothing cross-checks
the skill's examples against the validity parser. The `conditional — <event>`
form legitimately *does* carry an em-dash, which makes the dated form's
em-dash look idiomatic by neighbourhood.

**Workaround:** Put the class alone on the `**Valid:**` line and the rationale on
a following line. Both F-64 and W-54 in this tracker are written that way and
were accepted.

**Severity:** low — one failed call, fully self-diagnosing (the hint names the
three legal forms). Recorded because it is *systematic* rather than incidental:
it fires for every agent that follows the documented instruction, and the fix is
a two-line edit to the skill.

**Status:** open — the fix belongs upstream in
`../claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
(a different repo, so any fix commit needs the `<repo>:<sha>` citation prefix per
CLAUDE.md). Not fixed here.

**Valid:** dated 2026-08-26

True of the skill text and the validity parser as of this date. Re-verify by
re-reading `SKILL.md` § *Worked exemplars*; the entry closes when the exemplars
show the bare form.

**Rests on:** the 13-vs-0 corpus measurement above, which is what distinguishes
"the skill's examples were never valid" from "the parser recently got stricter" —
the two need different fixes, and only the first is a skill defect.

**Fix idea / Pointer:** In `SKILL.md`, move the em-dash rationale off the
`**Valid:**` line in both worked exemplars. Optionally note that
`conditional — <event>` is the one form whose em-dash is part of the syntax.

## W-55 — A peer session's rebuild left this one answering from a deleted inode — `/proc/<pid>/exe` is the one-command tell

**Valid:** invariant

**Observed:** First real action after compaction, `librarian(action="doctor")` reported
`archived_fix_sha_unresolvable` against `codescout-companion:b8ffa8b` — a **cross-repo**
pointer, which is exactly the case `7b5325a9` had taught the check to skip earlier the
same morning. Both obvious readings were wrong: the guard is not incomplete, and the
pointer is not dead (`b8ffa8b` resolves in `claude-plugins`).

**Got:** the binary answering me was a ghost. Timeline, all measured:

| 09:32 | my MCP server (pid 84841) starts |
| 09:45 | `7b5325a9` commits the cross-repo skip |
| 10:42 | a **peer session** runs `cargo rb`, replacing the inode |
| 11:05 | I run `doctor` and get pre-fix behaviour |

`ls -l /proc/84841/exe` → `…/target/release/codescout (deleted)`. The on-disk binary has
the fix (`grep -c skipped_cross_repo_pointer target/release/codescout` → 1); the running
process kept its own unlinked copy of the 09:32 build. Every doctor number this session
reported was from code that predates two of its own commits.

**Why this is not just R-89 restated.** R-89's mechanism is a session outrunning *its own*
rebuild — recoverable by remembering what you did. Here the rebuild belonged to **someone
else** and left no trace in my transcript, so there is nothing to remember and no upstream
proxy to consult: commit, working tree and binary mtime all read green. The peer-rebuild
form is invisible by construction to the session it breaks.

The tell is also cheaper than R-89's prescribed probe. `/proc/<pid>/exe` naming a
`(deleted)` target is one `ls`, needs no knowledge of which string the fix added, and can
be run *before* trusting a diagnostic rather than after one surprises you. Find the serving
pid by walking `ps -o ppid=` up from a `run_command` shell — the shell's parent **is** the
MCP server.

**Counterfactual:** the next step was to reopen a bug closed 80 minutes earlier and
"repair" a guard that works. What it actually cost was one `ls`, and the honest word for
`7b5325a9` shifted from *live* to **committed and present in the on-disk binary, not
loaded here** — which is also the correct word for every fix a peer's rebuild has
overtaken.

**Status:** validated

**Promote-when:** a second session hits a peer-rebuild ghost, or the deleted-inode probe
lands in the reconnaissance skill's freshness bullet (which today prescribes the
string-grep probe only).

## W-56 — Executing a bug file's own archive precondition, instead of asserting it was met, found a month-old CI outage the mandated gate structurally cannot show

**Valid:** invariant

**Observed:** six terminal bug files were queued for archiving. Five had no precondition;
`2026-08-07-edit-code-remove`'s `## Resume` step 1 said *"Confirm CI, then archive."* The
cheap reading was boilerplate from a stricter era — the fix was 18 days old, its two tests
were mutation-verified, and the local gate had been green all session.

**Got:** CI has been red on **every** `Test` lane since at least 2026-08-19 — ubuntu,
macos and windows × default, no-features and local-embed, plus `server-stack` — while
`Clippy`, `Format`, `MSRV`, `Feature check` and `Tool Docs Sync` all pass. One test causes
it: `memory_embedder_is_built_from_the_shared_code_embedder` resolves an embedder through
`RetrievalConfig::from_env_and_project`, and this machine exports
`CODESCOUT_EMBEDDER_URL`. Reproduced by removing one variable:

```
env -u CODESCOUT_EMBEDDER_URL cargo test --lib   →  4326 passed; 1 failed; 8 ignored
```

**Why the mandated gate cannot show this, ever.** `cargo fmt && cargo clippy && cargo test`
runs *in the developer's environment* by construction, so a test that reads ambient config
is green there and red everywhere else, and the gate has no vocabulary for "green, but only
here". The pinpoint is what makes it durable rather than obvious: **one** test of 4335, so
4326 green lines scroll past the one that is green by luck — and locally it never fails at
all, not even intermittently. Every *"gate green"* line written into a bug file this month
rests on that reading.

**The precondition was the project's only instrument pointing at CI.** Nothing else in the
hygiene machinery reads it — not `doctor`, not the verify-open cadence, not
`audit_doc_refs`. One sentence in one file's Resume, written 2026-08-08 by an author who
could not have known, is what put the question. It cost two `gh` calls.

**The generalisable form:** an archive precondition is a **check someone deliberately
deferred**, so it is the one line in a bug file whose cheapest disposal — asserting it is
met — is also the only way it can fail silently. This is the deferral-rationale law
(`reconnaissance-patterns:R-95`) applied to preconditions rather than to fix estimates, and
it has the same direction of bias: the person discharging it wants to archive.

**Status:** validated

**Promote-when:** a second archive precondition turns up a live defect, or CI status becomes
something a routine check reads rather than something a human remembers to look at.

## F-66 — I quoted the substrate law at the user, then spent a session measuring a retired datastore

**Observed:** 2026-08-26, immediately after `index(action="verify")` returned its
first live output. Its numbers disagreed with the numbers I had been quoting all
session, and the reconciliation found the cause in my own measurements, not the
tool's.

**When:** Building index-integrity checks, having earlier in the same session
recited this exact law to the user from the reconnaissance skill: *"A tool that
resolves its target from the environment has a SUBSTRATE as well as a verdict…
A retired-but-still-present datastore keeps answering, so the failure is a
confident wrong number, never an exception. When two tools disagree, the question
is not whose logic is wrong but which world each one read."*

**Expected:** `sqlite3 .codescout/embeddings/codescout.db` reads this project's
live semantic index. I assumed this from a *reporter's* bug text
(`CODESCOUT_VECTOR_BACKEND=sqlite-vec`, on their machine) plus the presence of a
247 MB `codescout.db` on disk.

**Got:** The live backend is **Qdrant**. `CODESCOUT_VECTOR_BACKEND` is unset;
`VectorBackend::resolve` (`src/retrieval/code_store.rs:247-262`) defaults to
`Qdrant` when `server-stack` is compiled in, and `CODESCOUT_QDRANT_URL=http://127.0.0.1:6334`
is set and answering. `codescout.db` has an mtime of **Aug 25 09:13** — it was not
written once during this session.

The two worlds, measured minutes apart:

| | live tool (Qdrant) | `codescout.db` (retired) |
|---|---|---|
| distinct files | 1611 | 1593 |
| chunks | 47 647 | 46 979 |
| paths absent from disk | 6 | 18 |

**Probable cause:** three compounding steps, none of which raised anything.
(1) I took the backend from the *reporter's* environment block rather than from
this host's. (2) A 247 MB file with exactly the right name and schema is a
powerful false confirmation — the queries all succeeded and returned plausible,
internally consistent numbers. (3) I ran a **positive control** on the join and it
passed, which felt like rigor: the deliberately-broken key returned all 46 979
rows, proving the predicate discriminated. It did. Against the wrong database. A
positive control validates the instrument, never the substrate — and I had just
finished telling the user that rule 4 of `docs/PROBES.md` covers this class.

**Workaround / what actually survives.** The split is clean, because it follows
what each measurement talked to:

- **Invalidated — read the retired store:** `docs/` at 1089 files / 25 232 chunks;
  `MAX(LENGTH(content)) = 1200` in every language with 0 over 8000; `scripts/` at
  15 of 19; "0 vector holes, 46 979 = 46 979"; "18 orphans".
- **Unaffected — measured against the live embedder or filesystem directly:** the
  CodeRankEmbed 2048-token ceiling; the n_batch error string; 19 of 23 memories
  over ~2000 chars; `test-design-discipline.md` failing to embed; the four-segment
  verification and its unit-norm pooled vector. Every fix shipped this session
  rests on these, so no shipped code is affected.
- **Conclusions that survive on better evidence:** "docs/ is fully covered" is now
  confirmed by the live tool's `empty_eligible_dirs: []`. "An oversized chunk is
  not reachable" rests on `chunk_target = 1200` being enforced in code and on the
  independent measurement in
  `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`,
  not on the stale table I cited for it.

**Severity:** high — five figures published to the user as measurements of live
state were measurements of a retired one, and two landed in committed bug-file
prose as evidence. Nothing shipped is wrong, which is the only reason this is not
worse; the defect is entirely in the evidence layer.

**Status:** open — the two bug files citing stale-store numbers still need their
evidence re-labelled.

**Valid:** invariant

The law is invariant; the specific backend resolution is `dated 2026-08-26` and
changes if `CODESCOUT_VECTOR_BACKEND` is ever set on this host.

**Rests on:** `VectorBackend::resolve`'s `server-stack` default and the unset env
var — the two facts that make a present-and-plausible sqlite file the wrong
substrate rather than merely a stale one.

**Fix idea / Pointer:** Before quoting any number about "the index", read the
BACKEND first — `env | grep CODESCOUT_VECTOR_BACKEND` plus
`VectorBackend::resolve`'s default — and prefer the tool over the store file.
`index(action="verify")` now exists precisely so this question has a
substrate-correct answer; use it instead of `sqlite3`. Pairs with W-54: I scouted
my own sizing claim and not my own instrument.

## W-57 — RELEASE.md's required live-output step caught a branch that was dead on the very stack it was written for

**Observed:** 2026-08-26, working `docs/RELEASE.md`'s Standard Ship Sequence step 1
sub-clause: *"Changed tool-facing OUTPUT? Call the tool live on this repo and read
the bytes first — a green suite does not establish that what the tool SAYS is
useful."*

**Pattern:** For any change to tool-facing output — a warning, hint, completeness
note, summary — call the real backend once and read the real bytes before shipping,
**even when the fixtures were written from a bug report that quotes the error
verbatim.** A quoted error string is evidence about the reporter's stack, not
yours.

**Counterfactual, concrete.** `67c548b9` added a payload-size arm to
`classify_search_error` matching `"larger than the max context size"` /
`"exceed_context_size"` — the wording quoted in GitHub #15. Three tests passed,
clippy clean, 4477 green. Then I posted an oversized payload to the running
CodeRankEmbed server and it returned **HTTP 500 `input is too large to process.
increase the physical batch size`, type `server_error`** — llama.cpp's `n_batch`
path, not its `n_ctx`-per-slot path. Different status, different type, different
wording, different remedy. **The arm would never have fired on this stack.** An
operator hitting it would have got the generic embedder hint instead of "this is a
size problem, retrying will not help" — the exact misrouting that function's doc
comment and its six sibling tests exist to prevent. Fixed in `5f8c42ec`, with both
variants now pinned and the binary-search provenance in the doc comment.

The same step then paid twice more in the same session: it produced the first
substrate-correct measurement of this project's index (`F-66`), and it confirmed
the segmentation fix end-to-end against the real embedder — the memory that cannot
be embedded today splitting into four pieces that all embed, byte-exact.

**Confirming data points:**
1. This session — dead classifier arm, invisible to a green suite because every
   fixture used the reported wording.
2. `docs/RELEASE.md` already records two: the `grep` hidden-skip warning (7 tests,
   3522 green, CI 15/15 on attempt 1, bug archived, output useless — every fixture
   had exactly one hidden entry so the truncation branch never ran) and round 6's
   `create.rs` fix proved inert in a live session despite a green suite.

**Impact:** high — this is a *shipped-and-green* escape class, not a caught-in-dev
one. All three datapoints passed every gate the project has.

**Promote-when:** already promoted — the step is `required`, not advisory, in
`docs/RELEASE.md`. This entry exists to add the third datapoint and to sharpen the
trigger, which the current wording does not cover: **a fixture built from a bug
report's quoted error string is not a live check.** It reads like evidence from the
real world because it came from one — someone else's. Worth folding that sentence
into the step when it is next edited.

**Status:** validated — third datapoint, and the first where the fixture *looked*
empirical.

**Valid:** invariant

The class is invariant: assertions written from quoted error text can only ever
pin the quoted text.

**Rests on:** `docs/RELEASE.md` § *Before cherry-pick: read the live output of any
tool-facing change (required)* and its two prior datapoints, which this extends
rather than establishes.

## W-58 — The live-output read is a census of the keys a path emits — which is what exposes a reader of a key nothing writes

**Valid:** invariant

**Status:** validated

**Observed:** 2026-08-26, running `docs/RELEASE.md`'s required live-output step for
`48825529` — the `index(action="status")` envelope change. The step's stated purpose is
to check *your own* change. It found a defect that predates it by three and a half
months.

**Pattern:** Read the live envelope **and** its formatter in the same pass. The live
response is not only evidence about the change under review — it is a **census of the
keys that path actually emits**, and any `if let Some(...)` in the formatter over a key
absent from that census is dead code. Producer/consumer mismatch becomes free to spot,
because both halves are in front of you at once. Nothing else in the project surfaces
it: the readers are `Option`-guarded, so a missing key is silently inert rather than a
panic, a wrong value, or a failing test.

**Counterfactual, concrete.** `format_index_status`
(`src/tools/semantic/index.rs:968-1012`) ends with four appended segments; two of them —
`indexed_with_model` (`:995`) and `indexed_at` (`:998`) — have had **no producer
anywhere in the repo** since `79e0e4f2` (2026-05-13, "re-route IndexStatus to Qdrant"),
whose own commit message lists both under *"Dropped fields (sqlite-only metadata)"*.
Measured: `grep("indexed_with_model", mode="files")` → four files repo-wide, all of them
the two readers, one test, and two docs. Zero producers.

The durable harm is not the dead code. `docs/manual/src/troubleshooting.md:231` tells a
user diagnosing an embedding-model mismatch to read `configured_model` and
`indexed_with_model` off a `status` response and check they match. **`configured_model`
has never been emitted by any commit in this repo's history** — `git log -S` finds a
five-commit history for `indexed_with_model` and none at all for it. So the manual's
documented detection procedure for a model mismatch names two fields, neither of which
exists, for exactly the failure class GitHub #18 was about (a dimension mismatch *is* a
model mismatch). Filed as
`docs/issues/2026-08-26-index-status-model-fields-dropped-but-still-documented.md`
(fixed and archived same-session: `docs/issues/archive/2026-08-26-index-status-model-fields-dropped-but-still-documented.md`).

**What this sharpens, beyond `W-57`.** `W-57`'s three datapoints are all fixtures whose
*content* was wrong — the reported error wording, the single-hidden-entry case, the inert
`create.rs` path. This one is a fixture whose **provenance** is wrong.
`format_index_status_shows_model_and_timestamp` (`src/tools/semantic/tests.rs:502-528`)
hand-builds a `json!` literal containing `"indexed_with_model"` and `"indexed_at"`, then
asserts the formatter renders them. It has been green continuously since the fields were
deleted, and **no amount of case variety would ever have helped**: the test author wrote
the response as well as the assertion, so the test can only ever prove `push_str` works.
The tell is structural and greppable — **a formatter test that constructs its own input
by hand asserts on a shape the product may not produce.** It is the "fixture could never
reach the gate" mechanism, one layer up: not an input that fails a validator, but a
*response* the code under test never builds.

**Second-order finding: a partially-reversed removal is a worse record than a total
one.** `git_sync` and `last_indexed_commit` are on `79e0e4f2`'s identical dropped list,
and both are in the live envelope today (`src/tools/semantic/index.rs:686` restores them
via `index_state::git_sync_status`). So anyone comparing that commit message against
current behaviour finds it *partly false*, which reads as a stale commit message rather
than as a live inventory of what is still missing. The reversal that got done is what
concealed the reversals that did not.

**Impact:** medium-high. Nothing is corrupt and nothing crashes; a documented
user-facing diagnostic procedure is simply inoperable, and has been for three and a half
months across every gate the project has — 4494 green tests included, one of which is
*about* the missing fields.

**Promote-when:** the RELEASE.md step is already `required`, so no promotion is owed for
the step itself. The structural tell — *a formatter/renderer test that hand-builds the
value it formats proves nothing about the keys the product emits* — was to be promoted at
a second datapoint. **Swept 2026-08-26: there is no second datapoint. Do not re-run this
sweep; read the result below.**

### Sweep result 2026-08-26 — the class has exactly one member

Enumerated per CLAUDE.md's *"when a scout finds a defect CLASS, enumerate it across the
whole corpus before writing any fix."* Scope: all 59 `format_*` / `render_*` functions
taking `&Value`, across 24 files — every site where a formatter consumes an envelope
built elsewhere. Method: census every JSON key **read** by production code against every
key **written** by production code, with `#[cfg(test)]` bodies and `tests.rs` excluded
from the write-set on purpose, since a key whose only writer is a test literal is the
defect itself.

**Verdict: `indexed_with_model` and `indexed_at` are the only two.** 307 production read
keys; 16 survived the final predicate; 14 of those are legitimate — read from an
*external* producer that by design has no writer in `src/`: TOML config
(`vector_backend`, `ignored_paths`, `force_include`, `poetry`), markdown frontmatter
(`expects_augmentation`, `no_fix_commit`), an HTTP header (`authorization`), a
`package.json` (`scripts`, `packages`), an embedder's `/props` (`max_client_batch_size`),
an LSP protocol message (`registrations`), a caller-supplied tool argument (`timeout`),
and two Serialize struct fields (`by_tool`, `p50_ms` on `UsageStats` / `ToolStats`,
`src/usage/db.rs:753`+`:761`).

**The instrument took four passes, and each pass's own false positives were the only
thing that revealed the previous predicate was wrong** — which is the transferable part,
more than the zero:

| run | hits | what was wrong |
|-----|-----:|----------------|
| 1 | 114 | `result["k"] = json!(..)` counted as a *read*. Caught only because `legacy_semantic_index` appeared, and this session had **watched that key come back live** from `workspace(activate)` — a failed negative control, not a code review |
| 2 | 40 | `=` at end of line escaped the assignment exclusion (`more_hint`); `Some(&["reviewed"])` slice literals parsed as key lookups |
| 3 | 23 | writes via `"k".to_string()` on its own line (`members_hint`) and `.with_extra("k", ..)` (`section_map`) were invisible |
| 4 | 16 | stopped enumerating write *idioms* — there is no closed set — and asked instead whether the literal appears anywhere in production but the read |

**Known blind spot, stated so the zero is not over-trusted:** run 4's predicate cannot
see a key produced by `serde_json::to_value(struct)`, because the field name never appears
as a string literal. That is exactly how `by_tool` / `p50_ms` reached the final list, and
it is the one way a real instance could still be hiding. The known pair was re-checked
against it by hand — no Serialize struct declares an `indexed_with_model` or `indexed_at`
field (`IndexState` carries `last_indexed_at`, a different name) — so the confirmed
finding does not rest on the blind spot. Probe kept at
`scratchpad/key_census.py`; it is session-local, so re-deriving it costs more than
re-reading this table.

**What the zero is worth.** It is evidence the project does not have a systemic
formatter-test problem, which is what a two-instance result would have implied and what
would have justified a convention. One instance justifies a bug file, which is already
filed. Recording the negative here is the point: without it, the next session reading
W-58's Promote-when re-runs a four-pass sweep to rediscover the same zero.

**Rests on:** `docs/RELEASE.md` Standard Ship Sequence step 1 (live-output sub-clause);
the reconnaissance skill's *"a green result certifies the path that actually EXECUTED"*
rule, of which this is a new mechanism rather than a new instance.

## W-59 — When a diagnostic says two signals "cannot be separated", price the separation before accepting it — the blocker was a 56 MB download

**Valid:** invariant

**Observed:** the wine lane failed **9** tests in `server::guide_hint_tests`; MSVC failed
**1**. Eight were `no POSIX shell available` — wine has no Git Bash. I filed that split as
inference and wrote, in the bug file, *"whether the ninth is real cannot be separated from
this run."* That sentence is the friction: it reads as a fact about the problem and is
actually a fact about the **environment**, which is the mutable half.

**Got:** the separation cost one download. PortableGit is a 7z **self-extracting archive**,
so `7z x` unpacks it on Linux with no installer executing; `CODESCOUT_BASH` set to its
*Windows* path (wine maps `/` to `Z:`) satisfies a plain `is_file()` probe; Git Bash then
runs under wine — `uname -s` → `MINGW64_NT-10.0-19045`.

The module went **9 failures → exactly the 1 MSVC also had**, which *confirmed* the
eight-vs-one split rather than leaving it inferred, and then → **0**.

**And the ninth was the only production defect in the entire sweep.** Six Windows causes,
44 tests: 43 were fixtures. The one that touched a real user — `names_tracker_path` picking
a guide on a hardcoded forward-slash substring, so a Windows caller creating a tracker
silently got the *general* guide — was the one hiding behind the environment noise. That is
not a coincidence worth ignoring: **noise is where a real signal is cheapest to lose**,
because every explanation for the pile also explains the one.

**The generalisable form.** *"Cannot be separated"* / *"not reproducible here"* / *"only CI
can see this"* are claims about the **substrate**, not the defect, and substrate is the
part you can change. Before accepting one, price the change: an extractable archive, a
container, an env var, a target already installed. The reconnaissance law already says a
proposed fix is a claim about current state that must be verified — this is its complement
for a proposed **limit**. Cost here: ~5 minutes and 56 MB, against a defect that had
survived every Windows CI run since 2026-08-20 and would have survived this session too.

**Counterfactual:** the bug file would have shipped with group A reading *"8 noise, 1
unknowable"*, the production defect would have stayed unfound behind it, and the next
person would have re-derived the same split from the same two logs.

**Status:** validated

**Promote-when:** a second "cannot reproduce locally" is dissolved by changing the
substrate rather than the reasoning — or `scripts/build-windows.sh`'s new `CODESCOUT_BASH`
note is used by someone who did not write it.

## W-60 — Fix the failure MESSAGE before the failure, when the message is uninformative

**Valid:** dated 2026-08-26

**Observed:** the `windows-gnu` lane's last remaining failure printed only

```
the heredoc body must land byte-for-byte:
```

— an empty `content`. That string reads identically in at least four different broken
worlds: the write was refused, the write failed, the write timed out, or it wrote
somewhere else. It also meant the test's *first* assertion
(`!content.contains("codescout-unfiltered")`) had been passing vacuously on the empty
string.

**Got:** rather than diagnose the bug, I spent one edit making the message able to
discriminate — assert the write's `exit_code` before reading, and print both results on
failure (`7d8bfae7`). The very next CI run answered the question outright:

```
{"timed_out":true,"stderr":"Command timed out after 30 seconds","exit_code":null,…}
```

Not corruption at all. A hang.

**Counterfactual:** the test's name, its doc comment, and its linked bug file all say
*heredoc pipe-rewrite corruption*, and the empty file is exactly what that bug produced.
Every signal pointed at re-opening a closed corruption bug — in masking code, on a
platform with no local reproduction, at roughly 7 minutes per CI round trip. The message
fix cost one edit and one run and pointed somewhere else entirely
(`docs/issues/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md`).

**Why it generalises:** an uninformative failure message is not a documentation problem,
it is a *measurement* problem — the instrument cannot distinguish the hypotheses you are
about to spend hours on. Improving it is not a detour from the debugging; on a slow or
unreproducible surface it is usually the cheapest next measurement available. The tell is
specific and checkable: **if the message would read the same in more than one broken
world, fix the message first.**

Related: [[R-79]]'s "a green that reads the same in a broken world proves nothing" — this
is its mirror for reds, and the same test carried both defects at once.

**Status:** validated

**Promote-when:** a second work stream reports the same sequence — an ambiguous failure
message improved before diagnosis, and the improved message changing the diagnosis. At
two datapoints this belongs in the systematic-debugging skill's Phase 1, next to "Read
Error Messages Carefully", as its active counterpart: when the message cannot answer the
question, make it able to.

**Second instance, same day — and it does NOT fire the criterion above.** Dropping 22 stale
wine-lane skips left one failure,
`run_command_with_overflow_emits_progressive_hint_once`, whose assertion was the bare
"overflowing output should emit progressive-disclosure hint" — identical in four broken
worlds (timed out / no output / too little to overflow / refused by a gate). It was taught
to print the envelope BEFORE being diagnosed or skipped (`d0aabbe3`), which is the pattern
applied deliberately rather than in hindsight.

Recorded as evidence, not as a promotion trigger: the criterion asks for a second **work
stream**, and this is the same one. A lesson confirming itself inside the sitting that
produced it is the weakest possible evidence for it — that is precisely why the criterion
names a different stream. The commit message for `d0aabbe3` claimed this satisfied the
criterion; it does not, and this note is the correction. Status stays `validated`; the
Promote-when stays unfired.

## W-61 — A tool's rendered response shape is evidence about the transport, not its return type

**Valid:** dated 2026-08-26

**Observed:** adding an `assert_eq!(wrote["exit_code"], 0)` guard to a test (see [[W-60]]),
I wrote a comment explaining why it was needed: *"`call` returns Ok for a gate refusal and
for a non-zero exit alike, so the `expect` above proves only that the tool was reachable."*

That felt obviously true. Every time this session refused one of my own `run_command`
calls, the refusal came back as a **value**:

```json
{"ok": false, "error": "shell access to source files is blocked", "hint": "..."}
```

**Got:** I ran the positive control anyway — substituted a gate-blocked command to confirm
the new assertion fires. It did not fire. The `.expect` above it caught an `Err`:

```
panicked at ...:1544:10: writing the heredoc should succeed:
  shell access to source files is blocked — hint: use read_file(...)
```

The refusal is a genuine `Err` in Rust. The `{"ok": false, …}` I had been reading all
session is the **MCP layer serializing that Err for the wire**. I had inferred a function's
return type from its transport encoding, and written the inference into a comment that
would have been read as authoritative by whoever touched that test next.

The assertion is still correct and still needed — a non-zero exit *does* come back as `Ok`,
confirmed by substituting `exit 3` — but for one reason, not two. Comment corrected in the
same commit (`7d8bfae7`).

**Counterfactual:** without the control, a false claim about `Tool::call`'s error contract
ships inside a test comment, in the file most likely to be consulted when someone next
wonders whether `.expect` on a tool call is sufficient. The claim is not checkable by
reading the test; you have to run the broken world to see it.

**Why it generalises:** this is the reconnaissance law *"a rendered read is evidence about
content, not about bytes"* applied one layer up. **A tool response you read through a
harness is evidence about the harness's encoding, not about the callee's signature** —
and the two disagree exactly where errors are marshalled into values, which is most RPC,
MCP, and HTTP boundaries. The reflex that would catch it is cheap: when a new assertion
encodes a belief about an API's contract, make the assertion fail once on purpose before
trusting either the assertion or the belief.

**Status:** validated

**Promote-when:** a second instance where a positive control refutes a stated belief about
a callee's contract (not merely about a value). At two datapoints, this belongs alongside
the existing rendered-read law in the reconnaissance patterns ledger as its RPC-boundary
form.

## F-67 — Two tree-wide writes swallowed a peer's in-flight work in one session — `cargo fmt` and `git add <dir>`

**Valid:** invariant

**Status:** validated

**Observed:** 2026-08-26. Two separate incidents, same root shape, same session, on a
shared checkout with a peer session actively editing three files.

**What happened.**

1. **`cargo fmt`** — run bare as part of the pre-commit gate. It is a *tree-wide write*:
   it reformatted 2 lines in `tests/retrieval_unit.rs`, a file the peer was editing.
   Detected by comparing `git diff --stat` (142/53) against
   `git diff --ignore-all-space --stat` (140/51) — the 2-line delta is the whitespace,
   and nothing else would have shown it. Harmless in content, but it surfaces as an
   unexplained diff in someone else's editor.
2. **`git add docs/issues`** — staged a whole directory, and swept
   `docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md` into commit
   `38f15fa1`. That is **~168 lines of the peer's active investigation** (an Update
   section, and a root cause naming the `default_workspace_root` clobber from
   `3be6b587a9c92a7a`), now committed under a message about memory migration that does
   not mention it.

**Why the second one is the costly one.** Nothing was lost — the content is in the repo,
correct, and the peer's working tree still matches. What was lost is **findability**: a
commit message is the index into history, and theirs now points at the wrong subject.
Anyone later asking *"when did the read_only investigation land?"* will not find it by
message, only by path.

**Why I did not fix it.** `git reset --soft HEAD~1` would restore the exact pre-commit
state and is tempting. It is also history surgery on a shared branch while a peer holds
uncommitted work in it — which is how `F-57` and `F-60` happened in the first place. The
disclosure plus this entry costs a bookkeeping wrinkle; the rewrite risks the thing it is
trying to protect. **When the damage is bookkeeping and the remedy is history rewriting,
take the bookkeeping.**

**The rule.** On a checkout that may be shared, treat every tree-wide operation as a
write to someone else's files:

- `git add <explicit paths>`, never `git add -A` or `git add <dir>`. Read
  `git status --short` **in the same call as the commit** and check every staged path is
  yours — the output is right there, which is exactly why it gets skimmed.
- Formatters, linters with `--fix`, codemods, `cargo fmt` — all tree-wide. There is no
  per-file discipline that makes them safe; the discipline has to be *checking the tree
  first*, or scoping the tool to your own paths.
- The tell for both: a `git status` or `git diff --stat` listing a path you never opened.
  `F-57`/`F-60` are the same tell from the receiving end, which is why this is a
  recurrence and not a new class.

**Impact:** medium. No data loss in either incident, and both were caught in-session. But
`F-57` and `F-60` are the same failure with the roles reversed, so this is the third and
fourth datapoint for one hazard, and the first two were expensive enough to log.

**Promote-when:** already actionable — the `git add <explicit paths>` half belongs in
`docs/RELEASE.md`'s ship sequence next to the existing state-check step, since that is
where the commit actually gets made. Promote on one more incident, or immediately if a
future one loses content rather than only mislabelling it.

**Rests on:** `bug-fix-session-log:F-57`, `bug-fix-session-log:F-60`, and
`bug-fix-session-log:W-49` (checking `ListAgents`/`SendMessage` before committing shared
uncommitted docs — the coordination half of the same problem).

## F-68 — Shared-tree authorship attribution: two indirect heuristics both proved unreliable

**Observed:** 2026-08-26, mid-session, resolving whose uncommitted WIP sat in three files (`src/main.rs`, `src/migrate/memories.rs`, `src/tools/semantic/index.rs`) in codescout's shared working tree — this machine runs three concurrent Claude Code profiles against the same repo.

**When:** After a peer proposed `git log -1 --format='%h %s' -- <path>` as "the discriminator" for who authored an uncommitted diff, and separately after a peer quoted a live process count ("6 codescout server processes with `cwd=codescout`") as corroborating evidence.

**Expected (peer's claim):** a commit's subject line names the active work stream, so it discriminates current authorship; process count of a shared `cwd` roughly tracks concurrent session count.

**Got (scouted reality):** `src/main.rs`'s last commit was 11 days old, `src/migrate/memories.rs`'s was 12 days old — both predate the session entirely, so the heuristic answers "who committed here historically," never "who holds today's uncommitted diff." Re-running the identical process-count probe minutes apart gave 6, then 11 (my own independent count via `/proc/*/cwd`), then 13 — a ~2x swing from LSP-worker/indexer churn under the same `cwd`, not from session count.

**Probable cause:** both are indirect, environment-derived proxies for a question ("who is editing this file right now") that has a direct, primary-source answer — reached for first because they're cheap to check, not because they discriminate the actual question.

**Workaround:** `stat --format='%y %n'` mtime clustering is the correct first-order check — cheap, and answers "when," which the commit-history heuristic never could. Where mtime clustering still leaves multiple candidates, grep the candidate sessions' own transcript content for the disputed path/topic — direct and decisive (67 vs. 4 mentions settled the actual case in one call).

**Severity:** med — no data was lost, but two wrong provisional attributions were stated to peer sessions as findings before being retracted, and concurrent multi-profile sessions on one machine is this machine's documented normal operating mode, not a one-off.

**Status:** fixed-verified — mtime clustering + transcript-content grep resolved the actual case; both retracted heuristics were withdrawn by their proposer before being written anywhere durable.

**Valid:** dated 2026-08-26

True of the specific numbers measured this session; the general lesson (prefer mtime/content over commit-history/process-count for live-authorship questions) should outlive them.

**Rests on:** `docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md` — the bug that prompted the cross-session coordination this friction surfaced in.

## W-62 — Source-level verification of buddy's `from=` field overturned a peer's self-doubt — and forward-projected the same failure mode

**Observed:** 2026-08-26, during the same cross-session coordination thread as F-68. A peer session (`codescout-8f`) surfaced its own `SessionStart` hook output — `<!-- buddy:reloaded sid=... from=92412525-... source=compact -->` — where `92412525` matched the session independently identified as the author of the disputed uncommitted WIP, and explicitly could not rule out that this meant genuine conversational lineage (i.e. that the peer's own "not mine" claim might be unknowingly false).

**Pattern:** When a peer raises a self-implicating possibility grounded in an ambiguous hook artifact (a `from=<sid>` field that could mean "you inherited this content" or could mean something narrower), read the mechanism's actual source and test fixtures before accepting either the exculpatory or the incriminating reading — don't resolve on plausibility, and don't defer to the peer's own hedged uncertainty as if it settles anything.

**Counterfactual:** without checking, "codescout-8f's session might have unknowingly absorbed the memory-migration work" would have remained an open, unresolved cloud over a good-faith "not mine" claim already stated three times to two other sessions — exactly the kind of ambiguity that erodes trust in future good-faith denials from any session. The actual mechanism (`.buddy/.current_session_id`, project-scoped, last-writer-wins, `render_reload_block` in buddy 0.9.1's `scripts/reload.py:236`) fully clears it: the field records whichever session last touched buddy specialist state in this project directory, with zero relation to conversational content.

**Confirming data points:**
1. Source read (`render_reload_block`, `scripts/reload.py`) plus test fixtures (`tests/test_hooks_session_start.sh`) established the mechanism before any claim was made either way.
2. `codescout-8f` independently confirmed by reading the live file content post-hoc: `.buddy/.current_session_id` now holds their own sid, having been overwritten since their own `SessionStart` — directly demonstrating last-writer-wins in real time, on their own initiative, after being told they didn't need to check.
3. `codescout-8f`'s follow-up named the sharper implication unprompted: the *next* session to start in this project will read `from=b02898c3` and can draw the identical false inference forward — the failure mode rotates rather than resolving, a conclusion the source read alone would not have surfaced without their confirmation closing the loop.

**Impact:** high — resolved a live, concrete, peer-facing credibility question (not a hypothetical), using cheap evidence (one source read + one grep, no full-transcript read of another session's private content), and surfaced that the underlying plugin mechanism will keep producing this exact false signal for every future session in this project unless fixed upstream.

**Promote-when:** a second instance of a hook-emitted `from=`/`prev_sid`-shaped field (in this or any plugin) is taken at face value as lineage before being checked at the source. At 2 datapoints, promote to codescout project memory (topic: `reconnaissance`) as: "A session-identity or lineage-looking field from a Claude Code plugin hook is a claim about implementation, not fact — read the hook's source before treating `from=`/`prev_sid`/similar as evidence of content inheritance."

**Status:** validated — single datapoint with a strong counterfactual (a live, stated, three-times-repeated peer claim was genuinely at risk); awaiting a second occurrence for promotion.

**Valid:** dated 2026-08-26

**Rests on:** the source read of buddy 0.9.1's `scripts/reload.py` `render_reload_block`, and `codescout-8f`'s independent confirmation of the live file's last-writer-wins content.

## W-63 — A regex over source and a structured tool are different instruments — the regex over-counts by exactly the fixtures

**Valid:** dated 2026-08-26

**Observed:** measuring how many `docs/issues/` citations in source code had rotted, to
decide whether `audit_doc_refs` should promote code-comment findings from `Med` to
gating severity. I counted with `grep`. Twice.

**Got:** both counts were wrong, in the same direction, by the same mechanism.

| pass | regex said | truth | the difference |
|---|---|---|---|
| archive-move rot | **77** | **55** | 15 string literals + 7 teaching placeholders |
| step-2 residue | **22** | **7** | 15 string literals |

`audit_doc_refs` extracts refs from **documentation nodes** via tree-sitter — it has a
test pinning that `let p = "docs/issues/2026-01-01-x.md";` is *not* a citation while the
comment above it is. A regex has no such notion. So the two instruments answer
similar-sounding questions about different populations, and the regex's answer is always
the larger one.

**Counterfactual:** the first error was caught before it mattered. The second was
**committed into a bug file as a fact** and shipped, and had to be corrected in a
follow-up. A number in a bug file is exactly the kind of thing a later session inherits
without re-deriving.

**What makes it survivable:** a filter that is conservative *by construction* rather than
by judgement. Splitting the 77 on *"does a file of this basename exist in
`docs/issues/archive/`?"* isolated the 55 genuine cases without a single hand decision,
because a placeholder has no archived twin. The independent comment-line test then agreed
on all 55 with zero disagreements — two orthogonal filters converging is what made a
30-file automated rewrite safe to run unattended.

**The rule:** when the question is *"what does this structured tool see?"*, the only
instrument that answers it is that tool. A grep answers a different question — about the
bytes — and the gap between them is the fixtures, every time.

Related: [[W-61]] (a tool's rendered response shape is evidence about the transport, not
its return type) — same family, one layer down: there, the harness's encoding was mistaken
for the callee's signature; here, the bytes are mistaken for the parse.

**Status:** validated

**Promote-when:** a third instance in any work stream, or one instance where the
over-count changes a decision rather than just a recorded number. At that point it belongs
in the reconnaissance ledger next to the "a search that finds nothing is evidence about
the search" law, as its false-positive twin: a search that finds *too much* is also
evidence about the search.

## W-64 — Bulk-removing a suppression list is the cheapest audit of it — once a green baseline makes the red unambiguous

**Valid:** dated 2026-08-26

**Observed:** the `windows-gnu` lane's `--skip` list had grown to **32 entries**. A skip
list is write-only in practice: entries are added under pressure and nothing ever prompts
a re-check, so it reads as coverage while silently withholding it.

**Got:** removing them in bulk and letting the lane name the survivors, in two passes:

| pass | action | result |
|---|---|---|
| local (wine 11.16) | run with ZERO skips | 12 fail — so 22 of 32 entries named passing tests |
| CI (wine 9.0) | drop those 22 | **21 of 22 passed**; the 1 that did not shared a cause with an entry already on the list |
| — | add `WINEPATH` for `git.exe` | 3 more retired |

**32 → 8 entries**, every survivor classified with its own un-skip protocol, and the
gnu-ABI lane — CI's *only* non-MSVC Windows coverage — went from 4251 to 4293 tests.

**The precondition is the whole trick.** This is only cheap because the lane was **green
first**. Against a red baseline a red result is uninformative; against a green one, the
failing test names *are* the answer, delivered in one run. Deriving the same list by
reasoning per-entry would have cost far more and produced a worse answer — I know, because
my by-hand arithmetic on this same list was wrong (19 vs the measured 22, tripped by one
entry matching four test names by substring).

**What had to be re-costed first:** I had recorded, correctly, that these could not be
removed because a wine-11 measurement does not transfer to a wine-9 lane. That was true
when written and **expired the moment the lane went green** — the green baseline converted
a red run from noise into an instrument. A deferral rationale is a claim about current
state ([[R-95]]), and its premise had changed under it.

**Generalises to:** `--skip` lists, `#[ignore]`, lint allowlists, `continue-on-error`,
exclusion globs, `expected_failures`. Anywhere a suppression accumulates faster than it is
audited.

**Do it safely:** drop them on a branch with a known-green tip, in a commit that says it is
a measurement, and re-add the genuine ones with the failure text as their justification.
Every survivor then carries evidence instead of inheritance.

**Status:** validated

**Promote-when:** a second work stream audits a suppression list this way. At that point it
belongs in the reconnaissance skill as a named technique, next to the positive-control
guidance — it is the same idea applied to a *set*: make the instrument fail on purpose to
learn what it was actually holding.

## W-65 — When a result is IMPOSSIBLE rather than merely wrong, suspect the run — not the code

**Valid:** dated 2026-08-26

**Observed:** three unrelated tests failed together on one `windows-gnu` run. Two were a
peer's brand-new feature, which made "the new code is broken on Windows" the obvious
reading. One of the payloads was this:

```
{"exit_code":1,"stderr":"Cygwin WARNING:…","unfiltered_output":"@cmd_3ee4bbf5",
 "unfiltered_truncated":true}
```

**Got:** that response cannot exist. `src/tools/run_command/output.rs` sets
`stdout`, `unfiltered_output`, `unfiltered_output_lines` and `unfiltered_truncated` inside
**one** `if let` block. Two of the four are present and two are absent. No branch produces
that.

I checked the obvious alternative first — a second producer that had not been updated —
and `grep` over `src/**/*.rs` found exactly one site. At that point the shape itself was
the evidence: **a response that no code path can emit is a fact about the run, not the
code.** Re-running the identical command with no source change: 4293 passed, 0 failed.
Twice. The two tests also pass in isolation under wine in 0.52s.

**Counterfactual:** I was two steps from filing a platform defect against a colleague's
feature that shipped an hour earlier, and from "fixing" production code that was correct.
The stderr in the payload is the wine Cygwin FAST_CWD warning — a genuine red herring that
appears on *every* `bash.exe` here and would have anchored the wrong diagnosis nicely.

**The tell, stated so it is checkable:** before diagnosing a failure, ask whether the
observed output is *reachable* from the code as written. Wrong values mean the logic is
wrong. **Impossible combinations mean the observation is wrong** — a partial write, a
truncated read, contention, a stale binary, a clobbered fixture. Different failure, different
hunt, and the second one is invisible if you start from "which line computed this?".

Filed as `docs/issues/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` with the
mechanism explicitly unproven — contention fits all three symptoms but nothing was
instrumented, so `unverified:` says lead-not-finding, and the two tempting fixes (retries,
raising the SQLite busy timeout) are named as harmful rather than deferred.

Related: [[W-60]] — there the message could not discriminate between broken worlds; here
the message was fully detailed and described a world that does not exist. Both are
"interrogate the instrument before the subject".

**Status:** validated

**Promote-when:** a second instance where impossibility — rather than incorrectness —
redirected a diagnosis. At two datapoints this belongs in `systematic-debugging` Phase 1,
as the question to ask before Phase 2: *is this output reachable at all?*

### Correction, same day — the reasoning held, the conclusion over-reached

CI run `32997878934` then failed **two of those three tests** on a GitHub runner. So "it
was just a bad run" was wrong about the tests, and it is worth being precise about which
half of this entry survives.

The **payload** was different: `{"timed_out":true,…}`, the wine-9.0 hang signature, not the
partial-key payload. Those two tests have **two unrelated failure modes** that share a
name, and only the envelope distinguishes them. They are now group 6 of the lane's skip
list, alongside the heredoc and `yes | head` cases; the local flake this entry describes
narrows to one test.

**What survives, and is now better evidenced:** the impossible payload *did* correctly
identify a bad observation — that specific run really was contention, and the code really
was fine. What it did not license was the inference "therefore these tests are healthy".
An impossible result tells you *this observation* is untrustworthy. It says nothing about
whether a **different**, entirely trustworthy failure is also waiting in the same test.

So the rule tightens rather than weakens: *interrogate the instrument before the subject* —
and when the instrument turns out to be faulty, you have learned nothing yet about the
subject. Discarding a bad observation is not evidence of health; it returns you to having
no observation.

**Status:** validated (with the scope correction above)

## W-66 — RED-before-GREEN on a bug's OWN prescribed fix caught it doing nothing

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, implementing the fix for
`docs/issues/archive/2026-08-26-session-log-template-cites-own-ledger-ids-bare.md`. The
bug's own `## Fix` table prescribed a three-part `<repo>:<file-stem>:<ID>` qualification
for the template's `F-N`/`W-N` citations.

**Pattern:** Applied the prescribed fix, then ran the regression test against BOTH the
pre-fix bare form and the prescribed fix — RED in both cases, byte-identical output. The
`<repo>:` segment was being silently dropped by the extraction regex's leftmost-match
slide (the same failure class as an already-fixed sibling bug,
`reconnaissance-patterns:R-89`'s truncation case), so the "fix" degraded to the same
single-qualifier form as no fix at all.

**Counterfactual:** Watching only "bare form fails, my fix passes" — the more common
shape of a regression test — would have shown green, because the prescribed fix and the
correct single-qualifier fix produce IDENTICAL extraction output; the test cannot tell
them apart without a THIRD data point. Watching RED-before-GREEN on the literal
*prescribed* text (not a stand-in "any fix") is what surfaced that the three-part form
does nothing at all. Without it: a "fix" ships that changes no observable behavior, the
bug closes as fixed, and the real defect — silent wrong-binding for template citations
copied into another repo — stays live. Filed separately (still open) as
`docs/issues/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md`.

**Pattern, generalized:** a bug file's own `## Fix` section is a claim about what will
work, written at the same moment of confidence the root cause itself was — re-entering
it to implement is a seam like any other (`reconnaissance-patterns:R-49`). Test the
literal prescribed text, not a paraphrase of it.

**Status:** validated

## W-67 — Measuring the 3 live zombie records before picking a fix shape avoided an unneeded doctor check

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, picking up
`docs/issues/archive/2026-08-26-zombie-bug-files-are-reachable-by-no-query.md`, which
named two fix options (a new `doctor` staleness check, or widening the triage guide's
canonical query) and explicitly recommended checking whether any live zombie record was
actually neglected before choosing between them.

**Pattern:** Read all 3 live `status: zombie` records before picking. None showed active
neglect: one (`e817931e`) had been explicitly re-verified that same day
(`last_verified: 2026-08-26`) and was deliberately zombie by maintainer decision; one's
own title said its underlying bugs were already fixed/mitigated; the oldest had already
been through one reopen-and-fail-to-reproduce cycle. That is exactly the outcome the
bug's own Fix section said would point at the doc-only fix over the doctor check.

**Counterfactual:** Defaulting to "add a doctor check, that's the more thorough-looking
option" without the 3-record read would have built a staleness-window detector for a
population that measurement showed had no active neglect to detect — solving a problem
that, on the evidence, wasn't actually occurring, while the doctor check's own design
choice (the staleness window) was explicitly flagged in the bug as "a judgement" with no
data yet to make it. The doc-only fix took minutes; the alternative would have spent a
session-worth of design + test effort on a check whose trigger population was empty.

**Status:** validated

## F-69 — Bug-file citations written at a future `archive/` path schedule no sweep, so nothing ever repairs them

**Observed:** 2026-08-26, reconnaissance scout of the then-uncommitted diff on
`experiments`. (Mid-scout a concurrent session committed the work as `fcb86c16` +
`0e88c979`; the scout's conclusions survived because they were content-addressed —
grep results and code facts — rather than position-addressed. The same distinction
`docs/RELEASE.md` draws between a SHA and a patch-id.)

**When:** Scouting the seam before the diff was committed — three files, all
re-pointing `docs/issues/archive/<slug>.md` citations to `docs/issues/<slug>.md`.

**Expected (plan):** `get_guide("tracker-conventions")` § *Bug files* documents
citation drift in exactly one direction — archive the file, then re-point every
citation **in the same commit as the move**, using the prescribed `grep -rn` sweep
(whose `--include` list was itself widened on 2026-08-26 by
`docs/issues/archive/2026-08-26-archive-citation-sweep-grep-cannot-see-shell-or-yaml.md`).

**Got (scouted reality):** **4 citations across 3 files** named
`docs/issues/archive/<slug>.md` for two bugs that had **never been archived** and
were both still `status: open` at scout time:

- `src/librarian/tools/doctor.rs:2790` and `:5766` → `cited-prefix-with-no-definer-is-invisible`
- `src/prompts/guides/tracker-conventions.md:384` → same bug
- `docs/trackers/bug-fix-session-log.md:5647` → `link-scan-double-qualified-citation-silently-drops-repo-prefix`

`git log -S` puts the `doctor.rs` and guide sites in `c0ec5ace`
(*feat(doctor): add cited_prefix_with_no_definer check*) — written at the moment the
fix was authored, naming the path the file *would* occupy once archived. `9bd9632e`
then recorded that fix as **partial**, so the archive move never happened and the
predicted path never came to exist.

**Probable cause:** The guide's sweep is **triggered by the archive event**. A
citation written prospectively is created *without* one, so no sweep is ever
scheduled and no procedure owns the repair — it depends entirely on someone
noticing. The underlying slip is that **"fixed" and "archived" are two separate
events**, and the bug file's own status vocabulary allows an indefinite gap between
them (`fixed` is terminal; archiving is a later, manual `artifact(action="move")`).
A partial fix widens that gap to permanent.

Half the population was also structurally ungateable, though for an already-documented
reason rather than a new one: the two `.rs` doc-comment sites are forced High→Med by
`cap_code_comment` (`src/librarian/tools/audit_doc_refs/severity.rs:199-205`, pinned by
`code_comments.rs::a_high_severity_citation_in_a_comment_is_capped_to_med`), so
`--fail-on high` cannot fire on them — which is why the guide already says *"the grep is
the check — not a green CI run."* The other two sat in live markdown at full severity.
Note `ArchiveDrop` does **not** apply here: it keys on whether the **citing** document is
archived (`severity.rs:251`, `matches_archive(md_file)`), not the target.

**Workaround:** Corrected in `fcb86c16` (*fix(docs): correct premature archive/
citations for two still-open bugs*), patch-id
`51a4af509be224cc87839ebbada5d35f68ee3b4a`. Scout re-swept **the two slugs named
above**: exactly 4 sites, all now `docs/issues/`.

**Correction 2026-08-26 — “no stragglers” was unsupported, in this entry's own failure
mode.** That sentence originally read *“re-swept the class repo-wide … no stragglers.”* It
was not a class sweep. The grep was
`2026-08-26-(link-scan-double-qualified|cited-prefix-with-no-definer)` — two slugs — and a
two-slug predicate cannot license a statement about the class. The claim was therefore
**wrong when written**, not overtaken by events, which is what keeps it out of
`claim-decay` and squarely here.

*(Amended — that parenthetical originally read “`DC` is for claims that were exact at
write time”, which is **not** the ledger's test and contradicts `DC-2`, an entry that was
never true. `codescout-77` caught the inconsistency. The actual test is whether the
claim's truth is hostage to a substrate that can change with nothing re-checking it. This
entry's “no stragglers” fails that test for a different reason: its falsity was fixed at
write time by its own predicate's scope, and no later event could have made it true. See
`claim-decay` § The inclusion test.)*

Peer session `codescout-77` swept the real population — **321 distinct
`docs/issues/archive/` citations** — and found one straggler this entry missed: **F-59**
cited `2026-08-23-index-build-fails-embed-batch-sparse-send.md` at its `archive/` path
while it was still unarchived. They resolved it by *completing the archive* rather than
editing the citation — the better fix, because it makes the prediction true instead of
retracting it, and leaves the narrative intact. Verified here: the file is now in
`docs/issues/archive/`, nothing matching `sparse` remains in `docs/issues/`, and F-59's
citation resolves as written.

The entry's *conclusion* is unaffected — the procedural hole is real and the guide rule
stands. Only its coverage claim was overstated. Recording it plainly because this is
R-3/R-79 (*a search that finds nothing is evidence about the search; the trigger is “I am
about to say all X are Y”*) firing inside the entry that was documenting a different
instance of the same drift — the law was in context, quoted in the skill that produced
this scout, and still did not fire.

**Severity:** med — 4 dead citations needing a dedicated repair commit; two of them
in a surface CI is designed never to gate on, so absent the scout they would have
persisted until a reader followed the link.

**Status:** fixed-verified — repaired in `fcb86c16`; both slugs swept this session with
zero remaining `docs/issues/archive/` citations. The full 321-citation class was swept by
`codescout-77`, which found and resolved one further straggler — see the correction above.

**Valid:** dated 2026-08-26

True of the two bug files' status and the four citation sites at `fcb86c16`. The
procedural hole (no sweep is scheduled for a prospective citation) outlives them and
is what the fix idea addresses.

**Rests on:** `cap_code_comment`'s High→Med forcing and `matches_archive`'s
citer-side keying, both read in `severity.rs` this session; and on `git log -S`
attributing the two `doctor.rs` sites to `c0ec5ace`.

**Fix idea / Pointer:** Add one line to `get_guide("tracker-conventions")` §
*Bug files*, next to the archive sweep: **cite where the file IS, never where the
archive flow will put it.** A citation is written when the fix is authored; the
archive move is a separate, later event that a partial fix may cancel outright. The
archive sweep repairs citations that *were* correct — it can never reach one that
was wrong on the day it was written.

## W-68 — A bug's own root-cause claim ("no SIGTERM handler") was false — reading the actual code before implementing the prescribed fix found the real gap two layers deeper

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, picking up
`docs/issues/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`'s
top-priority Fix item ("0. Exit on `SIGTERM`. ... its absence is why days-old processes were
still holding stale config"). Before implementing it, `references(shutdown_signal)` and a
`git log -S` check were run first.

**Pattern:** A `tokio::signal::unix::signal(SignalKind::terminate())` handler already existed
in `src/server.rs`, correctly wired into the stdio server's main `select!` loop, since `e4c70c8f`
(2026-02-26) — six months before the bug was filed. The bug's own reproduction (SIGTERM sent,
process survived, SIGKILL required) was real; its diagnosis of *why* was not. The actual gap was
one step downstream: the unconditional `lsp.shutdown_all().await` that runs after the `select!`
resolves has no overall deadline, so a wedged LSP client can block the process past the signal
that correctly woke it. Fixed by wrapping that call in `shutdown_with_deadline()` (20s ceiling)
— `ca2b0226`.

**Counterfactual:** Implementing the bug's own item 0 as written ("install a SIGTERM handler")
would have added no code that compiles differently from what already exists, closed nothing, and
left the actual defect — an unbounded await able to mask a correctly-delivered signal —
unaddressed and undocumented, with the bug file reading `fixed` for a mechanism that was never
broken. Same shape as W-66, one layer further from the surface: there the prescribed fix compiled
and ran but changed nothing observable; here it would have compiled, run, and changed nothing
because the premise itself no longer held.

**Status:** validated

## F-70 — A dead citation that was wrong when written is indistinguishable from one that decayed, and three attempts in one session each got it wrong from the prose

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, sweeping every `docs/issues/archive/*.md` citation in the repo — 321 distinct, 6 dead.

**When:** Answering `codescout-df`'s question of whether any of the dead citations belonged in the new `claim-decay` ledger (`DC-N`), which tracks claims that were correct when written and later invalidated with nothing scheduled to re-check them.

**Expected:** That at least one would be decay. `CLAUDE.md` says of the kotlin-lsp bug file that it "was pruned as a dupe in `c6184884`" — which reads exactly like *correct citation, target later removed*.

**Got:** **Zero.** All three real dead citations were wrong on the day they were written, and the prose pointed the other way in every case:

| citation | history | verdict |
|---|---|---|
| `docs/issues/archive/2026-03-24-kotlin-lsp-concurrent-instances.md`, cited from `docs/archive/2026-03-01-TODO-lsp-cancelled-kotlin.md:14` | target added `docs/issues/` `dc44ac3d` 2026-03-24 → added at `archive/` `cbf7456d` 2026-04-21 → open copy deleted `c6184884` → archived copy deleted `771d9516` **2026-05-02**. Citing line written `f7801347` **2026-05-18** | wrong when written, by 16 days |
| `docs/issues/archive/2026-08-08-librarian-guard-misses-quoted-frontmatter-ids.md`, cited from `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md:50` | `git log --all --diff-filter=AD` over `*librarian-guard-misses-quoted-frontmatter-ids*` returns **one** row: created `09633f16` **2026-08-16**. Never at an `08-08` path, at any commit | wrong when written — a date typo |
| `docs/issues/archive/2026-08-23-index-build-fails-embed-batch-sparse-send.md` | cited at a path the archive flow had not yet created — `F-69`'s class | wrong when written; resolved by completing the archive |

**Probable cause:** "Was this true when it was written?" **reads as a fact about the prose and is only ever a fact about history.** Nothing in a citation's text records when its target existed, so every unaided reading substitutes the reader's model of what probably happened — and that model is built from the surrounding narrative, which is exactly what the author's own wrong model produced.

The kotlin-lsp case is the worked trap: the file *did* live at the cited archive path, for eleven days. Both halves of "it was archived, then pruned" are true. Only the **ordering against the citing commit** decides it, and no amount of re-reading either document surfaces that.

**The probe, which is mandatory rather than advisory:**

```
git log --all --diff-filter=AD --name-status -- '*<target-slug>*'   # every path the target held, and when
git log --all -S'<cited path>' -- <citing file>                     # when the citation was written
```

Compare the two. Nothing cheaper works, and every unaided attempt in this session was wrong.

**Evidence that this is a class and not one slip — three passes, three errors, all inside one small classification:**

1. `codescout-df` shipped `DC` with two mutually incompatible criteria (the body keyed on the missing repair trigger; its own `F-69` correction one commit later keyed on "exact when written") and did not notice — they select different sets, and `DC-2` sits in the symmetric difference.
2. This session classified the kotlin-lsp citation as decay from `CLAUDE.md`'s "pruned as a dupe" phrasing, and only the `--diff-filter` run inverted it.
3. `codescout-df`'s correction (`42dfa0ca`) adopted the right criterion — *is the claim's truth coupled to a substrate that can still change, with nothing scheduled to re-check it* — and then mapped the three states onto it inverted, placing `DC-2` in F/W and the plain error in `DC`. Flagged before it hardened.

**Severity:** med — a dead link in a retired document is low cost on its own, but the *misclassification* routes entries into the wrong ledger, and a ledger that has swallowed the wrong class stops being evidence for anything.

**Status:** fixed-verified — the `08-08`→`08-16` typo is repaired (provable target: the citing sentence quotes that document's "15 of 27 trackers are unprotected" verbatim). The kotlin-lsp citation is **deliberately left**: its target exists at no path, so there is no correct value to write, and inventing an editorial note inside a retired 2026-03-01 TODO is a larger liberty than the dead link costs. `get_guide("tracker-conventions")` says to leave `docs/issues/archive/**` alone; that rule scopes to *"a retired document citing a **moved** path"*, so a provable transcription error is outside it while this one is not.

**Rests on:** the four-commit lifecycle above, read via `--diff-filter=AD --name-status` this session; and on the citing-commit dates from `git log -S` scoped to each citing file.

**Fix idea / Pointer:** the discriminator is recorded in `codescout-df`'s `claim-decay` ledger as the required probe, credited. The complement worth knowing here: **a bare "0 of 6 were decay" is survivorship, not prevalence** — a citation that decayed and was then repaired never appears as dead at all, so this measures the composition of the *dead* population only. Quoting the number without that scope would decay into "the DC class is empty" within a session or two.

## W-69 — Explicit-path staging held across three concurrent sessions in one checkout; a tree-wide add would have swept a peer's in-flight bug fix

**Observed:** 2026-08-26, ~22:45–23:10. Three Claude sessions committing to one
`experiments` checkout simultaneously — this one, `codescout-77`, and **a third session
neither of us has been able to identify** (mid-fix on the memory-pollution bug
`1275a6ada95c7182`, "91.5% of the live memories collection is test-fixture data").
**Read both corrections before this entry's title.** The title overclaims: the practice
held against a peer's *other* files and failed against a *shared* one, three minutes later
and in this very ledger (Correction 2). Correction 1 strikes a third-session name this
entry originally asserted, wrongly.

**Pattern:** Under concurrent writers, three rules, all cheap:

1. **Re-read `git status` immediately before every commit** — never reuse a status
   taken earlier in the same turn. HEAD moves.
2. **Stage by explicit path** — never `git add -A`, `git add <dir>`, or `git commit -a`.
   **This protects you from the wrong FILES and from nothing else.** `git add <path>`
   stages *every* change under that path, a peer's included. For a ledger two sessions
   append to all evening, a peer holding uncommitted work inside the path you are staging
   is the **normal case, not the edge case**. See Correction 2.
3. **Take both halves of a comparison from ONE INVOCATION — not one command line.**
   Two commands are two different worlds, and *a single shell command containing two
   `git` calls is two commands*. Measured 2026-08-26 23:47, in a command literally
   labelled `=== one snapshot ===`: `git rev-list --count` returned 22, and the
   `git log` three statements later listed 23 — a peer's `cd374594` landed between
   them, inside one `run_command`. The semicolons bought nothing. Derive every figure
   from the output of **one** process, or state the two as separate observations.
5. **Run `git diff -- <path>` immediately before `git add <path>`, and confirm every
   hunk is yours.** This is the check that actually covers rule 2's blind spot, and it
   is one command. **Rule 1 does not substitute**: `git status` reports the file as
   modified, which is true and useless — it cannot distinguish *your* modification from
   a peer's. An instrument answering exactly what it was asked, bound to the wrong
   question, which is `bug-fix-session-log:F-71`'s law — and F-71 is what it ate.
   **But the check is NOT atomic with the `add`, and that is measured in both
   directions inside four minutes.** `02d80963` (mine, no check run) swept
   `codescout-77`'s F-71. `66671bf5` (theirs, check run and honestly passed) swept three
   of *this very correction's* edits — my write landed in the window between their `diff`
   and their `add`. Rule 5 narrows the race from minutes to seconds; it does not close
   it. **Closing it takes coordination, not a command: two sessions should not hold
   uncommitted work in one file at the same time.** Say so on the wire, and the file is
   yours until you say otherwise.
4. **State uncertainty in the original claim, at the strength the evidence supports.**
   In a shared checkout your retraction *races a peer's ledger, and the ledger can win* —
   see the correction below, where a prompt, unprompted retraction still arrived after
   this entry was committed. Rule 4 is `codescout-77`'s.

**Counterfactual — measured, not hypothesised.** Between this session's `f33caedf`
(23:05:04) and `143b55f1` (23:08:33) — a 3.5-minute window — **four commits from two
other sessions landed** (`f16e3101` 23:06:27, `9cfca8e8` 23:06:56, `a4ee4aa7` 23:07:03,
`55415973` 23:08:08). At the earlier commits `d9055d36` and `42dfa0ca` the working tree
carried `src/memory/semantic_store.rs`, `src/memory/sqlite_semantic_store.rs` and
`src/tools/memory/tests.rs` dirty — a third session's **in-flight fix for a live
bug**. A single `git add -A` would have committed a peer's half-finished memory fix
inside a docs-only commit, on a branch two other sessions were about to push.

This is not a hypothetical failure mode in this repo: `bug-fix-session-log:F-67` records
it *already happening once*, via `cargo fmt` and `git add <dir>`. Rule 2 is F-67's
promotion, and this is its first measured save under real concurrency. Rule 1 also fired
twice for real — the tree changed under this session between consecutive commits, and a
file (`docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`)
appeared dirty *between* two of them.

**Rule 3 is the sharp one, and it is `codescout-77`'s** (credited): *`git status` and
`git diff` are not two readings of one world when a peer commits between them.* They ran
`status`, saw four dirty Rust files, then ran `git diff --stat`, got empty, and concluded
"touched but content unchanged — a stat-cache artifact." The diff was empty because HEAD
had advanced between the two observations; the files were genuinely modified and genuinely
committed, at 23:06:56 and 23:07:03, in the gap. A race read as a no-op, one step from
being published to two users as a fabricated anomaly. They caught and retracted it
themselves.

This is the same law `get_guide("tracker-conventions")` § *Entry ids* already states —
*"a max-id is a fact about an instant"* — but that promotion is scoped to id allocation
and does not reach a `git status`/`git diff` pair. Same class: **any two-observation
comparison against a reference a peer can move.**

**Correction 2026-08-26 — the third session was named, and the name was wrong.** This
entry as committed in `b842913f` attributed the dirty `src/memory/*.rs` files to
`claude-plugins-15`. That attribution was `codescout-77`'s, rested on nothing but a
`ListAgents` start time overlapping an mtime range, and is **withdrawn**. Probed here
before accepting the withdrawal: that session's three commits (`9b6bb25`, `dba7fe1`,
`b0db2d1`) are **absent from this repo's object database**, consistent with it working in
`/home/marius/work/claude/claude-plugins`. The name is **struck, not replaced** — no third
candidate is named, because none is supported.

**Why attribution keeps failing here, which is the durable half.** Every commit in this
repo is authored *and* committed by `Marius Ailinca <ailinca.marius@gmail.com>`, verified
on all three memory commits. **Git metadata is structurally incapable of attributing a
commit to a session.** That is why four attribution errors landed today, each from a
different weak proxy — mtime ranges, `ListAgents` start times, a `.buddy` session-start
trace whose last row (22:05:13) predates the entire 23:01–23:13 commit window, and this
entry inheriting one of them. There is no strong instrument, so the correct output is
*unattributed*, not a better guess.

**My own defect, distinct from the bad attribution.** I wrote an inherited claim into a
committed ledger without probing it, in a session where I probed nearly everything else —
including a peer's history account one message earlier, specifically to avoid hypocrisy.
It slipped because it arrived as a settled fact (*“Third writer placed”*) rather than as a
claim. **A conclusion handed over as settled bypasses the reflex that a hedge would have
triggered** — which is `F-70`'s law aimed at an incoming message instead of at a citation,
and the reason rule 4 is worth more than “retract faster.”

**The counterfactual is untouched.** *“One `git add -A` at `d9055d36` commits a peer's
half-finished bug fix inside a docs-only commit”* is true whoever owns the files. The
save is real, measured, and independent of the name.

**Correction 2 (2026-08-26, 23:17) — this entry's title overclaimed, and the
counterexample is the commit that made Correction 1.** `02d80963`, three minutes after
this entry was written, **swept `codescout-77`'s in-flight `F-71` body section into a
commit of mine.** They ran `append_entry` on this file at ~23:16; I staged the same file
by explicit path at 23:16:44. Measured: `## F-71 —` is absent at `b842913f`, present at
`02d80963`, and `git log -S` names no other commit. Nothing was lost — F-71 is intact and
self-attributing, and its orphaned index row landed as `365513ef`.

**F-67's mechanism has now fired three times, and the third was through the practice this
entry credits with preventing it.** That is not an argument against the practice: the
`d9055d36` save against `src/memory/*` was real and load-bearing *precisely because* those
were files I was not touching. It is the boundary. Explicit paths discriminate **files**,
and a shared ledger is one file.

Both sessions detected this independently, within a minute of each other, and arrived at
the same boundary and the same remedy without conferring — which is the reason to trust it
rather than read it as a rationalisation written after the loss. `codescout-77` stated the
sharper form first: *the risk is not tree-wide versus explicit-path, it is whether a peer
has uncommitted work inside a path you are about to stage.* Rules 2 and 5 are rewritten
accordingly.

**Confirming data points:**
1. F-67 (this log) — a peer's in-flight work already lost once to `cargo fmt` +
   `git add <dir>`.
2. This session — three concurrent writers, four peer commits inside one 3.5-minute
   window, zero cross-contamination; every commit staged by explicit path and verified
   by `git show --stat` afterwards.
3. `codescout-77`'s status/diff race, same afternoon, same checkout — the same law in
   its read-only form.

**Impact:** high — the failure is silent, lands in someone else's commit, and is
discovered by whoever the half-finished code breaks.

**Promote-when:** one further datapoint of a two-observation race in a shared checkout.
At that point promote to `CLAUDE.md` § *Git Workflow*, as one line: **under concurrent
sessions, stage by explicit path and take both halves of a comparison from one command.**
Rules 1–2 are already implied by F-67; rule 3 is not stated anywhere outside the entry-id
context.

**Status:** validated, and **bounded**. The save is measured and real; the coverage this
entry originally claimed was not. Cross-file: protected, measured. Same-file: unprotected,
also measured — twenty minutes apart, in the same ledger.

**Valid:** dated 2026-08-26

The commit timeline and the dirty-file observations are facts about this checkout on this
afternoon; the rules are general.

**Rests on:** `bug-fix-session-log:F-67` for the prior loss, and `codescout-77`'s own
retraction for datapoint 3 — not independently reproduced here.

## F-71 — Three confident claims from instruments with no resolving power, and the one that mattered hardened in a peer's committed ledger before the retraction arrived

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, across one session sharing a checkout with two other live sessions.

**When:** Reporting tool output to a user and to peer sessions, under concurrent writes.

**Got:** Three claims published from instruments that could not have distinguished the hypothesis from its alternative. Each was self-corrected unprompted; two reached the user first, and one reached a peer's **committed** ledger entry.

| # | claim published | instrument | why it could not discriminate |
|---|---|---|---|
| 1 | "CI has three red lanes" | `gh run view --jq 'select(.conclusion!="success")'` | an in-progress job's `conclusion` is `""`, not `null`. The predicate matches *running* jobs. 15/17 were green and nothing had failed |
| 2 | "those files are touched but their content is unchanged" | `git status` then `git diff --stat` | the two observations straddled a peer's commit. Empty diff meant **HEAD had advanced**, not that content matched |
| 3 | "the third writer is `claude-plugins-15`" | ListAgents start time vs file mtimes | a session starting 11 min ago and mtimes spanning 8 is consistent with every session on the box. They were in a different repo entirely |
| 4 † | "`git status` shows this file modified, so staging it is clean" | `git status` before `git add <path>` | status reports *that* a file changed, never *by whom*. It cannot separate the stager's own edit from a peer's uncommitted edit **in the same file** |

† Row 4 was measured by `codescout-df`, not by this session, and is logged as reported rather than
verified here — the standard `claude-plugins:W-4` applied to my instance, returned in kind. It is in
this table rather than in an entry of their own at their request: same mechanism, different actor, and
an entry is worth more intact than distributed.

**Probable cause:** In all three the instrument answered fluently in its own terms. None errored, none returned empty — the shape that prompts a second look. A result that is *wrong* invites checking; a result that is **complete and wrong** does not.

**Counted honestly, per `claude-plugins:W-4`'s own standard.** A fourth case belongs here as illustration and NOT as evidence: this session initially read `CLAUDE.md`'s "pruned as a dupe" as decay, and `--diff-filter=AD` inverted it (that is `F-70`). It was caught **before** publication, so nobody was misled, and counting it would bank four where there are three. The temptation to count it is the same one the peer's entry names.

**The part that is not about instruments.** Claim 3 was retracted promptly and unprompted — and still too late. `codescout-df` had already written it into `W-69`, committed as `b842913f`. In a single session, "retract before it hardens" works because you own the whole timeline. **In a shared checkout your correction races a peer's ledger, and the ledger can win.**

So the rule is not *retract faster*:

> **State the uncertainty in the original claim at the strength the evidence supports, because the retraction may not arrive in time.**

What I had was "a session appeared 11 minutes ago and the mtimes overlap." What I sent was "the third writer IS X." Had the first sentence gone out, `W-69` would have been correct on its first write and no retraction would have been needed. The failure was not slow correction; it was a confident first claim from a weak instrument — `F-70`'s own law, pointed at my output instead of at a citation.

**The cost, traced end to end — four steps, eleven minutes.** This is not a hypothetical
blast radius; it is what actually happened, and `codescout-df` traced it from their side:

1. 23:0x — this session publishes "the third writer **is** `claude-plugins-15`" off mtime overlap.
2. `codescout-df` inherits it **as fact** and writes it into `W-69`, committed `b842913f` 23:13:17.
3. The retraction arrives after the commit. They write a correction, `02d80963` 23:16:44.
4. Staging that correction by explicit path swallows this session's in-flight `F-71` body — the
   loss `W-69` was written to prevent, committed in the act of correcting `W-69`.

Nothing was destroyed and no history was rewritten: splitting `02d80963` would mean rewriting
shared history in a checkout three sessions are committing into, which is the larger risk. The
section is intact in `HEAD` and self-attributing; only the index row was re-committed here
(`365513ef`). But the chain from *one weak first claim* to *a peer's lost authorship* runs four
steps and eleven minutes, and every step after the first was competent work.

**This also bounds `W-69`'s rule 2, and `codescout-df` names the boundary themselves:**
"stage by explicit path" is **file-granular**. It protects a peer editing a *different* file — the
`F-67` case it was named after, where the loss was cross-file — and does nothing at all for a peer
editing the *same* file. For two sessions both appending to one ledger all evening, same-file is the
**likely** case, not the edge case. The check that costs one command and would have caught it:
`git diff -- <file>` immediately before staging, confirming every hunk is yours.

**Received, and it generalises the whole set** — `claude-plugins-15`, offered in exchange: *re-reading an instrument with the same belief is itself a check with no resolving power.* Every case above had a cheap probe sitting unread (`.conclusion=="failure"`; one `git log` after the second observation; `.buddy/.session-start-trace.log`, which was on disk in this repo the whole time). **A probe you could have run is not the same as one you did.** Their own instance is a second shape worth knowing: a harness `verdict()` mapping *empty* hook output to `"allow"`, so an assertion passed both when the feature worked and when its emitting call was stubbed out entirely — found by mutating it, never by reading it.

That is why this needs no new operational bullet. The reconnaissance skill's positive-control rule already covers it: **an instrument with no resolving power fails a known-positive exactly as a tautological one does.** The guidance was not missing. It was not run.

**Severity:** med — no wrong code shipped and nothing was lost, but one error propagated into another session's durable record, and a ledger that has absorbed a wrong attribution stops being evidence.

**Status:** fixed-verified — all three corrected; claim 3's correction sent to both affected sessions with the naming struck rather than replaced, since the author is still unidentified and a third guess would repeat the defect.

**Rests on:** the three tool outputs as returned this session, and on `.buddy/.session-start-trace.log`'s last row being 22:05:13 — an hour before the 23:01–23:13 commit window — which is what proves the session census cannot support a commit attribution.

**Promote-when:** a fourth instance from an independent work stream. Not before: the phrasing is crisper than the evidence, and `claude-plugins:F-13` is a same-day measured case of promoting a law off thin evidence and nearly deleting shipped content on the strength of it. Related: `W-69` (explicit-path staging under concurrent sessions), `F-70` (the citation-side form), `F-67` (tree-wide writes swallowing peer work).

## W-70 — Publish the retraction to the peer immediately even when you cannot fix it, because they can — two accidental sweeps four minutes apart, both harmless for that reason alone

**Valid:** dated 2026-08-26

**Observed:** 2026-08-26, 23:13–23:22, two sessions appending to `docs/trackers/bug-fix-session-log.md` while a third committed to the same checkout.

**Pattern:** When you discover you have clobbered, mis-stated, or swept a peer's work — say so on the wire **within minutes, before you know how to fix it, and even if you cannot**. The peer can act on information you cannot act on yourself.

**Counterfactual — two sweeps, opposite directions, four minutes apart:**

| # | who swept whom | how it was caught |
|---|---|---|
| 1 | `codescout-df`'s `02d80963` (a `W-69` correction) absorbed this session's in-flight `F-71` body | they announced it unprompted, ~3 min later |
| 2 | this session's `66671bf5` absorbed three of their `W-69` correction edits | they announced it unprompted, ~3 min later |

**Neither was prevented.** Sweep 2 happened with the prescribed guard — `git diff -- <file>` immediately before `git add <file>` — **honestly run and passed**: the diff showed only this session's hunks, and their artifact write landed in the window *between the diff and the add*. Verified after the fact: `git show 66671bf5:…` contains "Correction 2 (2026-08-26, 23:17)" and "This protects you from the wrong FILES"; `git show 66671bf5~1:…` contains neither.

So what made both harmless was **not** a check. It was that the losing session was told in time to still do something.

**What would have happened otherwise, concretely.** A session whose entry vanished from its working tree does not conclude "a peer committed it" — it concludes the write failed. The repair reflex is to re-run `append_entry`, which allocates a **fresh id** for content already in `HEAD` under another number. That yields two entries, two ids, and divergent bodies in a ledger with nothing that validates for duplication; Both sweeps were three minutes from that outcome, twice.

**Correction (2026-08-26, from `codescout-df`, disclosed immediately — the practice this entry
is about, applied to this entry).** The paragraph above originally ended: *"`link_scan` reports
both as defined, so every citation resolves to whichever copy is active. Nobody finds it until a
reader hits the contradiction."* **That is wrong**, and wrong in the direction that flatters the
argument — it claimed a resolution conflict that cannot occur.

`append_entry` computes the next id from the live max across params entries **and the ids the
markdown body already claims, headings included**, precisely so a body that ran ahead of params
cannot be reissued. So with `F-71` already in `HEAD` from the sweep, a re-run allocates `F-72`; it
**cannot** reissue `F-71`. No ambiguity, no active-versus-archived tie-break, nothing for
`link_scan` to disambiguate — `F-71` and `F-72` each resolve cleanly to themselves.

Which makes the real failure **quieter than written, not louder**: two well-formed, individually
citable entries saying the same thing under different numbers, each looking correct in isolation,
with the duplication visible only to someone who reads both. A resolution conflict would at least
surface *somewhere*. This surfaces nowhere.

**The class that error belongs to is NOT `F-71`'s, and the difference is worth the paragraph.**
`F-71`'s four were instruments answering fluently — bound to the wrong question, but consulted.
This one is the opposite mechanism: **no instrument was consulted at all, because the answer I
wanted was already written.** The check that would have caught it — reading `append_entry`'s own
documented id-allocation contract, quoted above — was one call away, and it went unread precisely
*because* the false clause made the argument land harder. I was making the case that the stake was
worse than "harmless", and reached for a louder failure mode than the real one.

**The two feel identical from the inside.** In both you hold a claim you believe, and in both the
probe is cheap and unread. They differ only in *why* it stayed unread: in `F-71`, because an
instrument appeared to have answered; here, because the conclusion appeared not to need one. The
second is harder to catch, because there is no green result to mistrust — only a sentence that
reads well. `codescout-df` declined to file this separately, on the grounds that a second entry
would produce one lesson under two ids with divergent bodies, each correct in isolation — the exact
failure this entry's own stake paragraph describes. So it is recorded here, once.

The corrected clause rests on the documented id-allocation contract rather than an experiment, and
deliberately so: **testing it would require running `append_entry`, which allocates an id — the test
consumes the thing it measures.** That is the `instrument-writes-into-its-own-corpus` shape, and it
is why this one claim is argued from the contract instead of demonstrated.

**Why this is the entry and "retract before it hardens" is not.** That formulation was proposed earlier in the same exchange and **failed twice the same evening** — one claim hardened in a peer's committed ledger before the retraction landed (`F-71`, step 2), and one entry was swept despite a passing guard. Speed of retraction is not the variable. **Reach** is. The correct form is not *retract before it hardens*; it is *tell the person who can still act, immediately, and before you have a remedy.*

**The mechanism that actually closed the race was a message, not a command.** Both sessions converged on an explicit handshake — "`bug-fix-session-log.md` is clear as of `<sha>`, nothing in flight, write freely" — and that is the only thing in the exchange that prevented rather than repaired. `git diff` narrows the window from minutes to seconds; a handshake removes it, because it establishes that no peer *holds* uncommitted work in the file, which no local command can observe. Stated as a rule: **two sessions should not hold uncommitted work in the same file at the same time**, and the only instrument that can establish that is the wire.

**Impact:** high — two silent ledger corruptions avoided in one evening, in a file both sessions had been appending to for hours.

**Status:** validated

**Rests on:** the two commits above and their parents, read this session; and on `claude-plugins:W-4` + `F-71` for the instrument-with-no-resolving-power framing that made both sweeps legible as one class rather than two accidents.

**Credit:** `codescout-df` named this practice and declined to file it, on the grounds that it belonged with the session that had the second sweep. Sweep 2 is mine. Their `84b705d8` carries the complementary boundary on `W-69`'s rule 2.

**Promote-when:** a second, independent work stream reports a peer-sweep made harmless by prompt disclosure — target `docs/RELEASE.md` § Concurrent-Work Rules, which per `W-49` documents git-state safety and says nothing about disclosure. Two datapoints here, four minutes apart, one work stream, one evening: that is one event with two halves, not two events, and promoting on it would be the accretion the reconnaissance skill's audit exists to prevent.

## W-71 — Replace the question, don't tighten the answer — a predicate needing an exhaustive population fails silently; one with a population of one has no such precondition

**Valid:** invariant

**Observed:** 2026-08-26, ~23:35–23:50, three concurrent sessions in this checkout. Two
sessions independently made the same *shape* of error within an hour, and both remedies
converged on changing the question rather than being more careful with the answer.

**The two instances.**

1. **Which processes are stale?** A peer needed to kill MCP server processes predating a
   23:33:54 rebuild. They listed nine pids, pulled start times for six, and reported on the
   sample as though it were the population — missing the oldest, a two-and-a-half-hour
   process. Self-corrected. The remedy was not "pull all nine": it was
   `P=$PPID; while ...; do ps -o pid=,lstart=,comm= -p $P; P=$(ps -o ppid= -p $P); done` —
   walk your own parent chain. That answers *which process serves me*, whose population is
   one.
2. **Which commits are the peer's?** I attributed `b3d2a0c1` and `9eb0dd62` to that peer,
   in a report to the user and in a message to the peer themselves, on nothing but "new
   commits appeared and a peer was active." They were a **third** session's — one neither
   of us had in our population. The remedy is the same: *which commits are mine* needs no
   inference, because I know what I committed.

**The mechanism, stated generally.** A predicate of the form *"which members of set S have
property P"* is correct only if the enumeration of S is exhaustive. Nothing signals a
short enumeration — the query returns a complete-looking answer over whatever it saw, so
the failure is a confident wrong result, never an error. A predicate whose subject is
**self** has a population of one; completeness is not a precondition, so there is nothing
to under-sample. That is a structural fix, not a diligence fix.

**Corollary — a protocol, not a habit.** When sessions in a shared checkout need to know
who wrote what, each **publishes its own list**; nobody derives everyone's. Derivation
cannot detect an absent participant. Publication does not need to.

**Got — and this is the part worth the entry.** [[W-70]] was written **forty minutes
before** instance 2, by me, in this file, about exactly this error class ("no instrument was
consulted at all, because the answer I wanted was already written"). Writing the lesson down
did not prevent the repeat. That is a datum about ledgers, not a confession: an entry that
describes an error class trains recognition *after* the fact, and this class is defined by
feeling like knowledge at the time. It is the same argument the reconnaissance skill makes in
R-49 — "the countermeasure is temporal, not attentional" — reaching the same conclusion from
the other side. A lesson that requires you to *notice* you are guessing is weaker than one
that removes the guess from the procedure.

**Counterfactual.** Instance 1 would have left a two-and-a-half-hour stale server running
while its owner reported the kill complete — and that process was one of the two that
turned out to need `-9`, i.e. the single clearest pre-fix datapoint for the
`ca2b0226` SIGTERM work. Instance 2 propagated one repo-visible layer before retraction
(a message to the peer) and, unretracted, would have credited the wrong session for the
`EMBED_API_KEY` test repair — the same shape as [[F-71]], where a wrong attribution
hardened in a peer's committed ledger before the correction landed.

**Cross-checks that did work, recorded so the entry is not one-sided.** The parent-walk
gave `3691039 started 23:35:01`, but start time is an upstream proxy; the served bytes
were confirmed separately by checking three markers the peer named against the
`tracker-conventions` text actually received. And this entry's own two instances were
each confirmed by their subject, not inferred by their observer.

**Status:** validated

**Promote-when:** a third instance appears in any repo where the subject is neither
processes nor commits — i.e. the shape generalises past "concurrent sessions in one
checkout." Candidate home is `reconnaissance-patterns` as a sibling to R-79 (a search that
finds nothing is evidence about the search), which is the same law for a *query*; this is
that law for a *population*.

## F-72 — Writing the caveat discharges the obligation to close it — the honest limit is the failure mode, not the remedy

**Valid:** invariant

**Observed:** 2026-08-27, ~00:30–01:10, two sessions independently, within forty minutes,
in a shared investigation of `get_guide` delivery. Neither of us was being careless, and
that is the whole point of the entry.

**The two instances.**

1. **`claude-plugins-15`** measured guide delivery from one session, noticed the sampling
   defect themselves, and wrote it into the issue's `Not yet done`: *"two points selected
   by being the two sessions that happened to be talking to each other is not a sample."*
   Correctly scoped, honestly stated — and they then stopped, without looking for the
   population.
2. **Me.** I went looking for the census, failed to find it in three plausible paths, and
   told my user: *"I did not read the persistence code, so I cannot tell you whether the
   writer is inert or writes somewhere I did not look… one function read settles it — say
   the word and I'll do it."* Accurate, properly hedged, correctly naming the exact next
   action. Then I wrapped up the turn instead of reading the function.

The peer read it. It resolved in one look: the ledger is at
`~/.local/state/codescout/guide_hints/` (XDG **state**, not cache or data —
`src/server.rs:438-439`), holding **91 session ledgers**. Everything the investigation
needed was there.

3. **`claude-plugins-15` again, and this is the strongest instance because its
   counterfactual is MEASURED rather than argued.** Filing a bug about the companion
   hard-blocking `Bash` after an MCP disconnect, they recorded fix option (c) as *"needs a
   PostToolUse path that sees MCP transport errors — **not confirmed to exist**."* They did
   not confirm it. When their user later said "go", `codescout-companion/hooks/hooks.json`
   turned out to have carried PostToolUse matchers on MCP tools all along —
   `mcp__.*__workspace` on line 141. Verified before recording: `git log -S` dates that
   matcher to `2926543`, **2026-05-01**, roughly four months before the bug was filed —
   so the premise was not awaiting confirmation, it had been false for a third of a year.

   So the caveat did not merely fail to prompt the work. **It authorised skipping the
   work, it was false, and (c) shipped the moment anyone looked.** It also survived the
   author's own review and a commit message that quoted it approvingly — two further
   passes over a false premise, each of which read it as a finding because it was
   labelled as a limit.

4. **Me, inside the write-up of instance 3 — and this is the entry's own best evidence.**
   The paragraph above originally read "`mcp__.*__workspace` on line 141, **plus a
   line-151 matcher covering nearly every codescout tool**", offered as a second proof the
   premise was false. Line 151 is `cs-liveness.mjs`, and `git log -S 'cs-liveness'` dates
   it to `b0db2d1`, **2026-08-26 — tonight**. It is the **remedy for that very bug**, not
   evidence against its premise. I read the current file and attributed its contents to
   the past, so the entry claimed the premise was *doubly* false when it was *singly*
   false. Caught by `claude-plugins-15`; corrected above.

   Two things make this the sharpest instance in the set. First, **I ran `git log -S` on
   the first matcher and not on the second** — so the write-up carried one dated claim and
   one undated one, and only the dated one was about the past. I verified exactly the
   claim I thought to verify, in an entry whose subject is claims nobody thinks to verify.
   Second, and worse: **I already knew.** I had investigated `cs-liveness.mjs` earlier the
   same session, established it was a `PostToolUse` hook clearing pre-tool-guard's circuit
   breaker, and told my user that `/reload-plugins` was needed precisely to register it.
   The fact was in context. Knowing the law did not install the check, and neither did
   knowing the fact.

   This is the substrate question from `reconnaissance-patterns` one level up: not whose
   logic is wrong, but **which world each tool read**. A file read reports the present. It
   answers a question about the past only if you date it, and nothing in the reading says
   you failed to.

5. **Me again, in this entry's own `Promote-when`.** The first version read "a third
   instance appears" — a bar the entry had *already cleared* at the moment I wrote it,
   inside an entry about premises nobody checks. Replaced below with
   `claude-plugins-15`'s criterion.

**Mechanism.** A caveat is written at the moment you become aware a check is missing.
Writing it converts an open question into a *recorded* one, and recorded reads as handled.
The obligation is not dropped — it is discharged, by the act of describing it. That is why
it does not feel like cutting a corner, and why "be more diligent" cannot fix it: the
diligence already fired. It fired into prose instead of into a probe.

**This is the mechanism BEHIND [[R-95]], not a sibling of it.** R-95 records that a
deferral rationale is the least-audited kind of claim, and that the bias runs one way —
nobody drafts an estimate that makes the work sound easier, because that estimate would
not justify stopping. This entry says *why the author himself stops re-checking*: he
already paid the honesty cost. Having written the limit feels like having taken it
seriously. R-95 is measured from outside (nine rationales, all inflated); this is the view
from inside the moment.

**The discriminator, and it is cheap.** When you write a caveat, check whether it names a
*specific next action* — a file to read, a command to run, a path to check. If it does,
the caveat is not the deliverable; it is a work item you have just written down and are
about to not do. Both instances passed this test loudly: mine literally named "one function
read settles it", theirs named the missing population. **A caveat that names its own remedy
should be executed, not filed.** One that genuinely cannot be closed in the current turn is
the legitimate case, and it reads differently — it names a blocker, not a next step.

**Counterfactual.** Had either of us closed it when we wrote it, the investigation would
have reached the census forty minutes earlier — and it *inverted* two conclusions when it
arrived. `librarian-runtime`, split out of `librarian` by hand specifically to keep the
parent lean, has auto-injected in **0 of 91 sessions**; I had cited that split as the
*precedent* for a splitting proposal, when it was the already-run *experiment*, and a
failed one. And the median session takes **2** topics against my 6, so my own
counterexample arm — which had already reframed the peer's central claim — turned out to
be drawn from the top 11%.

**Status:** validated

**Promote-when:** TWO instances arrive from work streams that were **not about caveats at
all**. Deliberately not "a third instance", and the reason is this entry's own subject
matter. Five instances surfaced in one night, between two agents, inside a conversation
explicitly about this mechanism — that is evidence about the search, not about the base
rate, and it is the same tail defect that made both of our `get_guide` samples
unrepresentative (median 2 topics per session; the two of us at 6 and 5). Promoting a law
about false confidence on a corpus assembled by hunting for it would be a poor joke.
Criterion proposed by `claude-plugins-15`, who declined to promote it to the
reconnaissance skill for exactly this reason. Candidate home is `reconnaissance-patterns` beside R-95, as its
inside-view; the pairing is what makes both actionable, since R-95 tells you to re-cost a
deferral and this tells you why the author will not.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
