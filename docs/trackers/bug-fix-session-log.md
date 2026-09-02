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
entry_high_water_F: 105
entry_high_water_W: 101
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
| F-105 | 2026-09-02 | high | test-rigor | fixed-verified | **A plan specified a complete test that could not fail, and the reason was a fallback the plan never mentions.** Task 9's test asserted only that `context`'s candidate ids hold no duplicate. Two defects were loud (it read `candidate_ids`, a field the tool does not emit; it called two non-existent helpers). The third is the one a careful transcription still ships: the fixture supplied **no embedder and no store**, which routes `context`'s topic branch into a `title\|topic contains` fallback that is **artifact-grain** — so the distinctness assertion is satisfied by the fallback while exercising none of the code under test, and is monotone under narrowing besides, so an empty page satisfies it too. Fix the two loud defects and it is green on a tree with `max_per_artifact` deleted. **The remedy is a fixture property, not an assertion:** the topic string matches no artifact's title or topic, so the fallback returns **zero** rows and a skipped semantic path reds a positive assertion instead of passing in silence. Adds a question the monotone-direction law does not reach — *what runs INSTEAD when this path is unavailable, and does that also pass?* A fallback is not a mutation of the feature, it is a second implementation of the same signature |
| F-104 | 2026-09-02 | med | delivery-path | promoted-to-bug-tracker | **The notice I fixed, verified and shipped is injected into the response and then discarded by 18 compact renderers.** `inject_notice` writes `_workspace_notice` into the `Value`; `call_content` hands that `Value` to `self.format_compact(&val)`, and no `format_compact` reads the key — so it survives only on the pretty-JSON branch. It reaches `artifact`; it does not reach `symbols`, `grep`, `tree`, `read_file`, `references`, `semantic_search`. **Both regression tests drive `EchoTool`, which takes the pretty-JSON branch**, so they exercise the one path production read tools do not; the two mutations I ran killed on the **decision** path (`notice_once`, `workspace_override`) and nothing reaches the **delivery** path. Found by a `/mcp` reconnect that recreated the original conditions, with a **refused write as the control** — the guard gates on the same flag, so its refusal proved every precondition held while two reads stayed silent. Three plausible causes were checked and died at the bytes first (stale binary, worktrees gone, peer activation). The live production check I had reported as satisfying `codescout-0a`'s standing ask is what hid it: every response I read the string in was `artifact`, the one shape where it works |
| F-103 | 2026-09-02 | med | docs-drift | fixed-verified | **The guide section a tool's caller actually receives is routable by `serves:`, so "the docs" is not one surface.** `489715ef` added `stage_together`/`stage_hint` to `artifact(action="move")` and documented the staging step in `tracker-conventions` § *Bug files* — but the section marked `<!-- serves: artifact.move, artifact.delete -->` in `src/prompts/guides/librarian.md` still printed the pre-fix response example, and `stage_together` appeared nowhere in that file. So the surface designed to teach a move caller taught the old shape. Surfaced **not by review**: the live probe run for `W-100` auto-injected the stale section into the same tool result as a response carrying the two fields it omits — drift and refutation arrived together. *Loudness is a property of a PATH* one layer up: the fix went on a path the caller does not traverse. Owed by anyone changing a response shape — grep `serves: <tool>.<action>` across `src/prompts/guides/`, a check nothing currently runs. Fixed in the same session |
| F-102 | 2026-09-02 | high | test-discipline | open | **Five assertions in one feature satisfied by text that survives deleting the thing they name — and the fifth was committed by the fix for the fourth.** `len()==2` where a constant, `h.level` and 0-indexing all pass; a `contains` on the token where replacing the rendered line list with an empty string stays green; a `contains` on *push* satisfied by the hint's explanatory sentence rather than by its remedy; a `contains` on `'7'` satisfied by the token `R-147` itself; and `len()==2` again, its message reading *leaving exactly the two `##` definitions*, equally satisfied by `[6,7]` — precisely what the mutation it guards against produces. Instance 5 sits in the diff written to fix instance 4, in the merge blocker's own evidence test, written by an implementer whose dispatch said *treat this as a class — fix any other assertion satisfiable by text that survives deleting the thing it names*; their report says **no fifth instance to report**. `OB-1` under controlled conditions: the author was not merely aware of the class, they were **holding an instruction to sweep for it, in the file they were sweeping**. Every catch came from a reviewer asking *what else satisfies this string?* — a different question, not a more careful reading of the same one, which is why care is the wrong instrument. Instance 5 shipped at `5eea9301`, parked with a ruling |
| F-101 | 2026-09-02 | med | measurement | open | **When you catch yourself explaining why a measurement is impractical, check whether the obstacle IS the result.** A `/mcp` reconnect drops you into the cleared-slot state a filed bug named as unowned; two calls closed it. Unpinned `symbols` returned home's answer, then a write was refused for the slot *still* being cleared — so **the read path has a silent home default that never sets the slot**, which is why worktree reads are silently wrong rather than blocked. Two confident wrong answers were drafted first: (1) "cleared slot auto-activates home", off the `project-activation-bootstrap` injection, which `get_guide("workspace-state")` says is re-sent on **every** reconnect regardless of project — the most legible signal was about the reconnect and nothing else; (2) "the probes interfere, the state is one-shot" — **wrong for the same reason the real answer is right**, and I had already written the table arguing it. Neither reached a peer or a commit only because the next tool call landed before the writeup did |
| F-100 | 2026-09-02 | high | measurement | open | **A window is a fact about the query, never about the thing — re-run once with it REMOVED, not widened.** Four instances in one evening across four sessions, each returning a clean, self-consistent, *wrong* answer. `cut -c1-150` hid **+1,508 chars** of hand-authored prose and nearly published a wrong correction *of* a peer; a `git grep` window stopped two files short of its own falsifier; a `grep` over stdout hid an argparse exit-2 written to **stderr** and cost **three** debugging rounds on code that worked — written by someone who had already messaged two peers about instance 1. The remedy is one command (drop the `cut`, drop `--include`, `2>&1`), not "be careful with filters", which is the remedy that failed four times, twice inside a correction of itself |
| F-99 | 2026-09-02 | high | measurement | fixed-verified | A spec's mechanism was **inferred from where the grep window stopped**. `context_lines=8` over `resolve.rs` showed the resolver's short-circuit and never reached the extractor two files upstream, so the spec claimed a same-file duplicate *"pushes two `DefinerRef`s"* when `extract.rs:399`/`:440` guard on a shared `seen_defs` and dedupe it to **one** at parse time. **The conclusion survived and its mechanism did not** — the dangerous shape, because an implementer re-reading a true claim backed by a false mechanism builds the check on `DocExtract.definitions` and gets a positive case that is *unrepresentable*: not wrong output, **no output ever**, with the red test pointing at the fixture rather than the design. Caught by a pre-dispatch scout one step before `writing-plans`; corrected in place at `fd6317a4` with the correction annotated rather than silently applied. The index row this entry needed was itself missing for ~6h — a section with no row is unreachable from the index, which is how an entry stops compounding |
| F-98 | 2026-09-02 | high | epistemic | fixed-verified | `8343d6ca` retracted a falsified remedy from a bug file, correctly scoped "bug file only, no counts moved" — and the same claim stood in `docs/trackers/issue-clusters.md:1139`, IC-17's `**Members:**`, labelled **verified**, written by the same session at `cd6bb36c`. **The correct scoping is what preserved the falsehood**: nothing relates a bug file to a ledger field quoting it — no edge, no diff hunk, no check. Worse than the unsafe instruction beside it: `:872` is advice that misfires, `:1139` was false and labelled verified. Found by `codescout-05` reading past the line I had *accurately* told it not to amend — an accurate but incomplete correction carries enough authority to stop the next person looking. Fixed `fea2e1ce`. |
| F-97 | 2026-09-02 | med | measurement | fixed-verified | Ran `cargo check --workspace --all-targets` (which does not lint at all), got exit 0, and published "all green" under the project gate's name — while `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` was red, exit 101, 10 errors. Had seen the seven `dead_code` warnings and described them as "expected"; `-D warnings` promotes each to an error. Quoted the gate's four commands in my own first message of the session and ran none of them for four hours. **A green from an instrument chosen for its speed is a fact about that instrument.** Caught by `codescout-26`, which held the failing code. |
| F-96 | 2026-09-02 | med | epistemic | fixed-verified | I published `core.hooksPath points at scripts/` as the reason a hook edit is instantly live here. Wrong twice. **The variable is unset** at every scope — this repo's *healthy* state, asserted by `tests/hook_config.rs`, and a *set* value once disabled every hook here for a day, so a reader acting on my sentence reproduces an archived bug. A peer caught that. **But the conclusion survived their correction and was independently false:** pre-commit clears unstaged changes before hooks (`staged_files_only.py:108`), so a `language: system` entry runs the **index** copy of its own script — measured, 3 cases. The exposure moment is `git add`, not the editor save. Both halves of the refutation were already filed in this repo, held one each by two sessions who agreed with each other instead of composing them. **A correction is a fresh claim — check the part of the original it leaves standing.** Second law: an `## Environment` block is the least-audited claim in a bug file, and the one downstream files copy |
| F-95 | 2026-09-01 | high | concurrent-sessions | mitigated | Mutation testing in the shared checkout put `if false && …` into a peer's **staged index**. Their review of the staged diff used a filtered grep that happened not to print the `if` line, so a disabled gate passed human review with a green suite behind it; the only thing that stopped the commit was the `unreviewed-content` pre-commit hook — which is about unstaged content, not about this defect, and would not have fired on a whole-tree `git add`. A mutation writes the SAME BYTES an intentional edit writes, so no observer can separate them: `file-provenance` correctly returned `SHARED` and named both sessions, and that is the most any instrument could say. Restored byte-exact, re-staged, peer notified, 7/7 green — but nothing prevents recurrence. Mutations belong in a worktree, not a shared tree |
| F-94 | 2026-09-01 | med | test-rigor | fixed-verified | The new dispersion test pins the gate's **existence**, not its **threshold**. Four mutation runs: disabling the gate kills only that test (so it earns its place), but moving `DISPERSION` 0.8 → 0.9 is **7/7 green**. Its two fixtures sit at the extremes — 0.33 and 1.00 — so every cut in (0.33, 1.00] satisfies it, leaving the shipped 0.8 unguarded across **(0.667, 1.0]**. The sole guard between 0.667 and 0.8 is `…_reports_only_the_active_projects_citers`, a *project-scoping* regression whose 3-cites/2-files ratio is incidental to its purpose and unannotated — so a tidy-up making that fixture read more like a real ledger would keep it passing and delete the last guard. `IC-14` turned on a test |
| F-93 | 2026-09-01 | med | measurement | fixed-verified | Two counts over a peer's **live append-only transcript** were true when published and false within the hour — `Write` 8 → **9**, `doctor.rs` mentions 80 → **187** — both inside a committed artifact. A retention-swept corpus makes every count a floor; an append-only one makes it a floor that **rises**, so the number decays toward looking like a conservative undercount rather than an error. The conclusion never moved and in fact strengthened (`Edit` still 0, now 9/9 native writes to `/tmp`), which removed the last thing that might have prompted a re-check. Remedy: state the PROPERTY ("every native write is scratch"), which cannot rot, not its cardinality, which rots at the ninth. **Found by running F-92's prescribed audit rather than filing it** — and that is the empirical proof F-92 was member-selection, not recording-filter: widening is a no-op for the latter and found two real defects here in one pass |
| F-92 | 2026-09-01 | med | self-friction | fixed-verified | Corroborated a CORRECT verdict with evidence from outside its own window. `file-provenance` returned `SHARED` on `bug-fix-session-log.md`; I supported it with "F-83 appears 7× and is not mine" — but F-83 entered at `5d405b67`, which **predates** the window floor `9188d000`. The real in-window write was the peer's W-91 at `da44f73d`, learned from them volunteering it. Right answer, invalid support: the worse failure, since re-running the tool re-confirms the verdict and never touches the reasoning. Same shape as the evening's three misattributions — every part true, no step checked against the object — except the instrument was already open and prints the window on its own output line. The verdict came from the tool and was checked; the corroboration was mine, added to make it sound better-supported, and entered through the one channel with no gate. `git log <floor>..HEAD -- <path>` answers it in one command |
| F-91 | 2026-09-01 | high | measurement | fixed-verified | A positive control certified the INCIDENT and was read as certifying the SUBSTRATE. The IC-10 calibration (`440dd773`) confirmed `cs_tool_log.jsonl` held the disputed writes, checked the other direction too (native writes: zero) and concluded "build it". It never asked whether that log retains records at all: `MAX_ENTRIES = 50` rolling (196 of 297 logs sit exactly at the cap), deleted on compact/resume/clear (`hook_helpers.py:244,420` — the calibrating session made **370** codescout calls and its own log held **1**), and `args` cut to 200 chars over unordered keys (11.5% of 1,920 write records carry no `path=`; 147 more carry a truncated one). The sample was the one session in the good state on all three axes. `wc -l .buddy/*/cs_tool_log.jsonl | sort -n | uniq -c` ends the design in one command. Nothing shipped — substrate rejected, channel built on transcripts, writer defect filed upstream |
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
| F-75 | 2026-08-27 | med | process | validated | A fix verified against its mechanism left its own headline aggregate untrue — the bug titled "78% other repos' rows" was fixed, three-part-verified and archived while the report was still **52%** foreign via six other checks. A title names a property of the report; a fix names a property of one check |
| F-74 | 2026-08-27 | high | self-friction | fixed-verified | A bug file's own `## Fix` plan partitioned a case that does not exist — `managed_roots` never contains another repo, so "rows under a different managed root" was empty and the prescribed fix would have changed the report by zero rows while passing its own test. Measuring the real partition (359 umbrella / 33 known-repo / 10 orphan) is what produced a working fix |
| F-73 | 2026-08-27 | med | self-friction | fixed-verified | A bug file's "four documentation surfaces" included one nothing reads — `companion_hint.md` is `include_str!`'d only by its own test, and grep grain cannot tell that from a live surface. A surface list is a reachability claim wearing a location list's clothes |
| F-72 | 2026-08-27 | high | self-friction | validated | Writing an honest caveat discharges the obligation to close it — two sessions, forty minutes apart, each named the exact missing check in prose and then stopped. The tell: a caveat that names a specific next action is a work item, not a deliverable. Mechanism behind [[R-95]], seen from inside |
| F-71 | 2026-08-26 | med | self-friction | fixed-verified | Three claims published from instruments that could not discriminate the hypothesis from its alternative — and the retraction of one lost a race to a peer's committed ledger entry |
| F-70 | 2026-08-26 | med | process | fixed-verified | A dead citation that was wrong when written is indistinguishable from one that decayed — 0 of 6 in a 321-citation sweep were decay, and three independent artifacts misread it from the prose; the `--diff-filter=AD` probe is the only discriminator |
| F-76 | 2026-08-29 | med | plan-prose | fixed-verified | A bug file's own evidence table cited `sync.rs:904` for the main sync path — that line is a `flush_pending` inside `sync_worktree`, the very call site the section exists to distinguish. On `sync_project` the vectors land inside `stream_index` (`:620`/`:626`, invoked at `:1017`). Conclusion unchanged, but a reader verifying it at that line is led straight back to the inverted fix |
| F-77 | 2026-08-29 | med | cross-session | mitigated | A peer's hazard report was accurate when written and false when read — eleven correct filenames described work committed eight hours earlier (`45a88531`); a second peer credited this session with an edit it never made. Cross-session messages carry a send time, never an observation time |
| F-82 | 2026-08-30 | high | measurement | open | Recommended a gate change to the operator as **"zero added time, strictly better"** on a symmetry argument I had not measured — in an argument whose subject was measurement discipline, on the same day I wrote `W-82`/`W-84`/`W-85` and corrected two of my own overstatements. Measured: documented order 71.8s (binary CLOBBERED, `exit 2`), reordered 80.4s, append-a-build 79.6s — so ~8.6s slower, not free, and the two fixes cost the same. Did not flip to "slower" either: two single runs on a machine with four concurrent builders is sampling+temporal adjacency (`reconnaissance-patterns:R-136`). The proposal survived on structure — four commands not five, prevents rather than repairs, and REMOVES the proposition whose exit code could be misread rather than documenting the trap — an argument only visible once the speed claim died. Caught by a peer's honesty marker about their OWN identical claim, not by any check of mine; second instance today after `F-81`, same shape |
| F-81 | 2026-08-30 | high | cross-session | open | Announced a mutation window to ONE peer, because my window collided with theirs and I wrote it as a reply — reply-shaped thinking silently scoped the broadcast. A second peer ran `cargo test` into my deliberate break, watched it pass on re-run after I had restored, and committed it as a "load-dependent flake" in a file they had never touched. Worse than silence: silence leaves the observer's uncertainty calibrated, while a partial announcement makes the ANNOUNCER believe coordination happened, so nobody is looking. Pairs with F-80 — an unannounced edit invents a phantom SESSION, an edit announced to a subset invents a phantom FLAKE, and the observer reaches for whichever ghost they already believe in. Mitigations: say what you break, not how; broadcast to every peer; and say that your broadcast is a subset, since `ListAgents` under-reports |
| F-80 | 2026-08-30 | high | cross-session | open | Closed an authorship question by ELIMINATION over a population no instrument reports completely, and sent it as a positive ID. `ListAgents` is scoped to one Claude profile's socket dir; this machine runs three, and a fourth session in `~/.claude-sdd` shared the checkout, wrote the contested hunks, and could not be seen or messaged (BL-58). Cost: `fix-embedding-transport-stage-1`'s correct first read — "not mine" — was abandoned to my well-argued wrong analysis, the inverse of every other misattribution that day. Remedy is a different KIND of instrument: grep session transcripts for `tool_use` write calls carrying a distinctive symbol from the diff, across ALL THREE profile dirs — positive identification, and it reaches exited sessions the process table cannot |
| F-79 | 2026-08-30 | low | cross-session | mitigated | Told a peer a shared file was clean, kept editing it, and their explicit-path `git add` swept my BL-51/BL-52 re-statusing into a commit about archiving a different bug. Not `W-69`'s committer-side check but its missing sender-side half: "clean" is a claim with an expiry only the sender can see, so an all-clear should name a SHA or be retracted the moment you touch the file again |
| F-78 | 2026-08-29 | low | self-friction | fixed-verified | Attributed a test failure to a peer's uncommitted file from a keyword count in its diff (19 × "timeout" across +137 lines). Wrong twice: the real cause was a `tokio::try_join!` race inside the test itself (`21174425`), and the "cheap decisive check" I first proposed — run it in isolation — PASSES, so it confirms the flake reading wrongly. The instrument whose subject was the failure was reading `embed_one_batch` |
| F-83 | 2026-09-01 | high | codescout-tool | open | `artifact(get)` carries `updated_at` per response but nothing says "changed since your last read", so two reads of one artifact 5.7 min apart composed a contradiction present in neither commit either side of it (75085 vs 78448 bytes = `13226bda^` vs `13226bda`). Five false "the ledger contradicts itself" findings were one step from a user-facing report; the natural reading blames the FILE, not the read |
| F-84 | 2026-09-01 | med | codescout-tool | open | A rebuild + `/mcp` refreshes the SERVER but not its LSP mux delegates. A `$PPID` walk proves this session fresh (pid 2695653, 15s post-build, `/proc/exe` with no ` (deleted)`), while six siblings ran the replaced inode — including this repo's own rust-analyzer mux, started 00:04:55, half an hour before the build. Binary mtime, the `~/.cargo/bin` symlink, a clean tree, the last source commit and the reconnect itself ALL read green; only `/proc/<pid>/exe` did not. R-89's fourth axis: delegate |
| F-85 | 2026-09-01 | med | self-friction | open | Committed the exact IC-10 attribution error I had corrected a peer for eight minutes earlier, in the next message, about that class. On a shared checkout git author is identical and `git status` shows the UNION of all sessions' work, so author / adjacency / dirty-file lists carry ZERO ownership signal by construction — not a care problem, no heuristic extracts it. Five wrong attributions across three sessions in 15 min; all five resolved by asking the session, and only by asking. That protocol is the mechanism IC-10 lacks |
| F-86 | 2026-09-01 | high | process | open | A guard's ABORT path is an unscoped write. My commit guard's success branch was scoped to one explicit path; its failure branch was a bare `git reset`, i.e. `--mixed HEAD` over the whole shared index. It fires exactly when a peer is active, because a foreign staged path is what trips it. Provable margin: the 16 staged paths landed as 0dea2246 at 00:44:39, my commit at 00:45:17 — any run of the old guard inside that window would have unstaged a batch its owner committed seconds later. The two runs that passed did so because nothing foreign happened to be staged, which is luck and reads identically to correctness. Remedy is to delete the branch, not scope it: `git commit -F <msg> -- <path>` has no mismatch to detect |
| F-87 | 2026-09-01 | med | self-friction | fixed-verified | A double-quoted grep pattern let the shell expand `$tmp` to empty, so the search actually run was `mv -f ""` and its clean zero read as "the commit deleted this mechanism". The line is present at `:141`. The zero landed right after `git show --stat` had proved that commit DID touch the file, so a prior belief was already in place and the false negative confirmed it rather than looking odd. Queued next action was retracting a correct root cause in my own just-filed bug file. Fourth mechanism for the `R-3`→`R-113`→`R-77`→`R-79`→`R-104` law, which covers scope, shape and encoding but assumes the tool received the pattern its author wrote |
| F-88 | 2026-09-01 | med | self-friction | fixed-verified | Bulk-inserted `## Fix provenance` into 7 bug files after verifying placement on the FIRST one only. One target already had that section (unbolded `- SHA:`, which the parser skips), so the insert produced two byte-identical headings — `IC-6`, and removable only via the `occurrence` disambiguator shipped by another file in the same batch. The post-hoc check asked *did it land between `## Fix` and `## Tests added`*, an assertion **monotone under "a section already existed here"**: it can confirm an insert worked, never that it was inappropriate. The precondition was *absent in all 7*, and one `grep -c '^## Fix provenance'` answers it. Also read `doctor`'s "no pointer is **declared**" as a claim about the file when it is a claim about the parse |
| F-89 | 2026-09-01 | med | cross-session | fixed-verified | Misattributed uncommitted `doctor.rs` work to `bcc98c22` by DIRECTORY ADJACENCY, inside the message correcting a peer for misattributing by elimination, citing `F-80` and `F-85` while doing it. Real owner was `c2a08c22` (codescout-68), who volunteered it. The `Session-Id` trailer I had just promoted exists only on COMMITTED work; the disputed work was uncommitted, so I swapped instruments mid-argument without noticing. Rule splits by state: committed -> trailer, positive and exact; uncommitted -> NO positive instrument exists on a shared checkout, ask the session, and until it answers the honest claim is "not mine", never "yours". **Sharpened: the real owner was INSIDE my visible four — I named an invisible session and offered its invisibility as corroboration, so the documented `ListAgents` gap (BL-58) explains neither this error nor the peer's. Three misattributions of one file in one evening, three parties, all three actively reasoning about attribution** |
| F-90 | 2026-09-01 | med | self-friction | fixed-verified | Published "the worktree guard MANDATES `git -C`, so the two guards are in direct tension" to five surfaces — three commit messages, a bug file, and a source comment — without probing it. One command refutes it: `git add --dry-run <path>` exits 0, unblocked. The guard refuses only commit-family verbs; both my blocked commands merely CONTAINED `git commit`. Attribution is recorded at STAGING time, so no tension exists on the path that matters. Population inflated too: `-C` is 32 of 1586 real `git add` calls (2.0%), not the mandated form. Being blocked twice felt like having tested it — refusal establishes what a guard refuses, never what it permits |

## Wins Index


| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-101 | 2026-09-02 | high | **Before filing a bug against the component that REPORTED an anomaly, ask what else was writing to the resource it read.** `edit_file` refused a mutation quoting the **pre-edit** line, at a line number, while `read_file` seconds later returned the post-edit line and a `cargo test` between them had already proven the post-edit line live. On a shared checkout the answer is routinely a peer's pre-commit hook, which empties the working tree of every unstaged change for its hook run | Not one wasted file. Two diagnostics had already run and **both pointed the wrong way while looking like progress**: `read_edit_target` (`src/tools/edit_file/mod.rs:678`) is a bare `std::fs::read_to_string` with **no cache**, and a two-edit scratch probe did not reproduce. Read together they invite *"the cache must be specific to indexed source files"* — a cache that does not exist, in a tool that is not at fault — while `IC-12` gained no member and kept its *"no downstream failure observed"* line. What closed it was a different question, not more care: pre-commit retains its stash at `~/.cache/pre-commit/patch<epoch>-<pid>` **permanently**, and the patch from the peer commit inside the window contains `-                1,` / `+                2,` verbatim. **The reusable half is that oracle** — readable *after* the window, and a more complete index than `git log`, since 2 of the 4 stash events here correspond to no commit at all | validated |
| W-100 | 2026-09-02 | med | **After a rebuild, verify a shipped tool change with one LIVE call rather than trusting the unit test that gated it.** `move_names_the_staging_action_not_only_the_two_paths` calls `mv::call` directly and therefore observes **absolute** paths — `strip_paths_in_value` runs later, at `Tool::call_content` — so it established that the fields exist and ordered correctly, and established nothing about whether the new `PATH_KEYS` entry relativizes at runtime | A `stage_together` holding absolute paths passes the unit suite **and** the corpus gate: `src/tools/core/path_strip.rs`'s own header states `no_absolute_project_paths_in_rendered_output` "only covers the file-tool surface … no librarian tool is in its fixture set", so librarian's path keys are exercised only by synthetic-`Value` unit tests. **Nothing in the suite covers end-to-end relativization of a librarian key**, so the defect would have shipped as two absolute paths rendered beside the relativized `old_abs_path`/`new_abs_path` in the same response — visible to every caller, invisible to every test. Binary provenance established positively rather than by mtime inference: server pid 190206, exe with no ` (deleted)` suffix, started 20:08:10 > binary built 20:06:36 > `489715ef` committed 17:11:17, ancestor of HEAD | validated |
| W-99 | 2026-09-02 | med | **A safety measure adopted by REASONING about a mechanism, rather than by reading it, is a hypothesis — and `git commit -- <paths>` is the one that fails.** A pathspec commit ignores the index — true, and where the reasoning stops — and reads the **working tree** at those paths, which is the half that matters on a shared checkout: it commits whatever a concurrent session wrote to the same file since you last looked. Staging is what satisfies the `unreviewed-content` hook; the pathspec is not | The hook refused a commit from a session that had reasoned about index safety explicitly across several windows and reached the wrong form **by** that reasoning — care was the instrument, and care is what failed. **Not an averted capture:** all 34 deleted lines were verified mine, so the refused commit would in fact have been clean. What was prevented is the *belief* persisting, and it emits no symptom until the one time a peer has touched the same file. The situation was real — 7 paths staged, 6 the peer's — so only the remedy was wrong, which is why nothing about the situation could have flagged it | validated |
| W-98 | 2026-09-02 | high | **A question about METHOD is a request to re-measure, not to re-explain — and when you re-derive, change the INSTRUMENT, not the care.** A challenge to a conclusion invites defence, and defending is often right; a challenge to the method cannot be answered by re-asserting the conclusion at all, because a sound conclusion drawn by an unsound method is indistinguishable from an unsound one at the point of publication. A more careful application of the same instrument reproduces the same blind spot — this is `W-94`'s *check it against a sample that differs* applied reflexively, when no peer is available | Two applications in one session, **one of which changed the answer**. (1) *Both owed citations closed, verified* rested on an unpinned `grep` returning zero; re-run **pinned** it returned zero again — sound, but the pinned run is the only reason that could be said, so the honest write-up was *the claim stands; the method that produced it did not deserve to be trusted*. (2) Measuring fix-SHA decay, `git cat-file -e` over 123 archived citations returned **0 dead** — which reads as *SHAs never die here*, and would have made a merge-vs-rebase decision look stakeless. `cat-file -e` tests object EXISTENCE; an orphan survives in the object store until `git gc` prunes it. Re-measured by **reachability** against `experiments` and `master`: **1 orphan**. Nothing in the first output marked it as answering a different question, it was self-consistent, and it pointed the same direction as the truth — which is exactly what would have made it durable | validated |
| W-97 | 2026-09-02 | med | **When a change removes a surface, sweep its references and sort each into POINTER (go consult this) or RECORD (this once happened). Repair the pointers; leave the records** — deleting a record to satisfy a sweep falsifies the evidence the surface ever existed. Paired with a mechanical twin that needs no judgement: `grep -ohE 'scripts/[a-z_-]+\.(py\|sh)' <touched files> \| sort -u`, then stat each | Measured on the `n`-column removal: **14** surviving references, **13 records, 1 pointer** — a linter flagging all 14 would be right once and destructive thirteen times. The live pointer was a section banner in `tests/issue_clusters.rs` (`6070ae75`); a second, found independently by `codescout-0d` (`005dd9e6`), was **itself the remedy for a stale claim** — a fix outliving its own surface with nothing pointing backwards from the removal commit. The path check caught a dangling pointer **inside the commit fixing dangling pointers** (`probe-cluster-express.py`, no such file) under a minute after writing it, by a maximally-primed author: priming did not help, the mechanism did. Denominator published — `0d` ran the same check over its own 6 files + `docs/conventions/*.md`, **10 paths, 9 resolve, 1 known fixture, no defect** — because a check mentioned only when it fires has none | validated |
| W-96 | 2026-09-02 | med | **`-z` and NUL-splitting travel together or neither travels.** Adding `-z` to a consumer that splits on *lines* turns a correct instrument into a wrong one, so *"harden it with `-z`"* is the specific regression to refuse when reviewing any path enumerator | A zero-byte file named `head.\naab0c4ef'"s` — newline plus both quote characters — appeared in the repo root minutes after two line-oriented hook parsers changed underneath this session (`689ceffb`). Both obvious moves were wrong. **The parser is fine:** `core.quotePath` defaults true, so `git diff --cached --raw` C-quotes the path onto ONE tab-delimited line and both hooks parse 2 rows for 2 staged files, field intact. **`-z` is the defect:** `git ls-files --others \| wc -l` → 2 (correct), `-z \| wc -l` → **1**, and `-z` + Python `.splitlines()` → the *right count* with *garbage rows*. No newline is needed to reach it — `core.quotePath` quotes any non-ASCII byte, so an em-dash in a bug-file slug counts correctly then cannot be opened (`git show :"…\342\200\224….md"` → rc=128). **Exposure today is 0** — nothing in `scripts/`, `tests/*.sh`, `.pre-commit-config.yaml` or `src/` pairs `-z` with a line splitter, and 0 tracked paths are C-quoted — so this is recorded as latent rather than live, with `_prime_index`'s `cat-file --batch` path left explicitly untested for want of a population. Subject expired mid-investigation: the file was gone ~6 min later and a `(none)` grep result was nearly read as a grep bug rather than a changed world; the finding survived only because it had already been reproduced in a throwaway repo the peers could not reach | validated |
| W-95 | 2026-09-02 | high | **Before filing a bug against a tool this repo builds, compare the running binary's mtime to the commit time of the fix that would explain the finding** — `stat -c '%y' target/debug/codescout` against `git log -1 --format='%cd' <sha>`. Two commands, no build. `doctor` reported 5 files as `non_terminal_status_with_fix_anchor`; four were artifacts of a binary **141 seconds older than `36cb17ed`**, the commit that fixed exactly that misreading. The report was complete, self-consistent and reproducible on that binary — and wrong. Confirmed by rebuild: the same check now reports **0**, and the zero discriminates, because the fifth file's anchor genuinely is under `## Fix` and would still fire had its `unverified:` discharge (`dde26886`) not also worked. A mechanism, not a policy: the trigger is a category you already know you are in. |
| W-94 | 2026-09-02 | high | **Get a claim checked by a peer whose SAMPLE differs** — different scope, different sample *time*, or different instrument; not a more careful reviewer, a differently-positioned one. Five errors in one ~5h session across seven co-located sessions, **none caught by its author**, two already committed: a stale compiler sample (`E0603` vs a peer's `E0425` on the same tree ~1min apart, no overlap), `F-97`, a committed-and-falsified count remedy, `F-98`, and a shared-blind-spot corroboration made **twice** — the second time 90 minutes after publishing the rule against it. Every correction came from the party who would have benefited from the error standing. The promotable form is **not** "get peer review" — that is a policy, and it failed against an author who had just written it down — but **state the scope and unit a claim was measured over**, so a reader can tell whether a second instrument is independent. |
| W-93 | 2026-09-01 | high | A green fixture suite met real data and the correction was to the QUESTION, not the code — run the instrument against the corpus before believing its design, not merely before shipping it | `file-provenance.py` passed 34/34 fixtures, then answered its own motivating file with **fifteen** sessions, every one a genuine lifetime author. Not a bug: "who has ever written this?" is answerable and useless, while "this is dirty NOW and reds my build, is it mine?" is bounded by the path's last commit. No fixture could pose it — each had one write in an empty timeline, which is what a fixture author naturally writes. Three assertions in the same suite also passed VACUOUSLY, each differently: a marker matched its own fixture path name (`src/undated.rs`), a case was answered by an earlier section's write to the same path (`docs/new.md`), and a negative passed because its fixture source was out-of-tree and discarded before any logic ran | validated |
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
| W-72 | 2026-08-27 | high | A negative result must name what it examined — AND stay quiet when it is trustworthy. Both halves load-bearing: naming nothing makes a false negative read as a finding; naming an unchecked cause ends the search for the real one | Three bugs fixed this session were one defect (`444d756c` grep glob, `fc1bbf21` sub-project memories, `76e287f8` wrong-tree not-found), and the codebase already held SIX independent instantiations of the rule, none of them named. `unsatisfiable_absolute_glob` states the principle in its own doc comment yet left the relative-glob case unguarded in the SAME function for nine days; `symbols::WalkAudit` carried the `accepted` counter `grep::WalkAudit` needed, one directory away, for months. Unnamed, each new tool re-derives it from its own outage | validated |
| W-70 | 2026-08-26 | high | Publish the retraction to the peer immediately even when you cannot fix it — two accidental sweeps of the same ledger, four minutes apart in opposite directions, both harmless solely because each session was told within ~3 min | Sweep 2 (`66671bf5`) happened with `git diff -- <file>` honestly run and PASSED; the peer's write landed between the diff and the add, so the guard narrows the race and does not close it. The only thing that prevented rather than repaired was a wire handshake — "file is clear as of `<sha>`, write freely". Absent disclosure, a session whose entry vanished re-runs `append_entry`, allocating a fresh id for content already in HEAD: two ids, divergent bodies, nothing validating for duplication | validated |
| W-69 | 2026-08-26 | high | Explicit-path staging + re-read status per commit, under three concurrent sessions — **bounded**: it discriminates FILES, so it saved a peer's `src/memory/*` and did nothing when a peer appended to this same ledger 3 min later (`02d80963` swept their F-71). The covering check is `git diff -- <path>` before `git add <path>` — **amended 2026-08-30: necessary, not sufficient.** The pre-add diff and the add are not atomic and the window is raceable; the guard that closes is a post-stage `git diff --cached`, and it must be a CONTENT diff. `--cached --stat` is NOT a check | Four peer commits landed inside one 3.5-min window between two of mine; `src/memory/*.rs` sat dirty as a third session's in-flight fix for a live bug, and one `git add -A` would have committed it inside a docs-only commit. (Corrected: the entry originally named that session; the name was an inherited guess and is struck — git metadata cannot attribute a commit to a session here, every commit carrying the same author and committer.) F-67 records that exact loss already happening once. Rule 3 is `codescout-77`'s: `git status` and `git diff` are not two readings of one world when a peer commits between them — they read a race as a stat-cache no-op and retracted it | validated |
| W-68 | 2026-08-26 | high | A bug's own root-cause claim ("no SIGTERM handler") was false — verified by reading the code before implementing the prescribed fix | Would have shipped a no-op fix and left an unbounded LSP-shutdown await masking a correctly-delivered signal, undocumented | validated |
| W-73 | 2026-08-29 | med | "Compile-error → green" is the trigger for spending a mutation: a test whose only observed RED was a compile error has never run its assertions against a wrong world. In statically typed languages that is the NORMAL TDD cycle, so the shape is common rather than rare | `guard_stale_binary`'s wiring test would have shipped looking like proof. The policy unit tests (`Some(true)` refuses, `Some(false)`/`None` do not) still pass when the guard is written, tested and never called — five green tests, defect 100% present, and nothing else in 4642 tests notices. Mutating the call to `let _ = ...` failed with the exact symptom its doc comment predicts. Second datapoint the same afternoon in `read_file.rs`, where two pre-existing tests assert the identical property and stay green because their 1200-short-line fixture can never reach the valve | **validated — promoted 2026-08-30** → memory `test-design-discipline` § The mechanical backstop |
| W-77 | 2026-08-30 | high | A remedy that closes the COMPILE half of a class reads as the fix, and the runtime half then stays open **and green**. The win is a refusal: declining to promote the lean-build gate change until an instance arrived that the EXISTING remedy provably could not reach | T12's remedy (add `--all-targets` to the lean check) was correct, sufficient for its instance, and closes compiling only. Promoting at T12 — the obvious moment, two datapoints in hand — would have added `--all-targets` to a `check`, the one remedy that cannot see a runtime failure, and marked the class closed. Then `2c6f2677` reddened the lean lane for **over a day** while ≥3 sessions ran the full documented gate green (mine 4×), because `check --all-targets` compiles the lean test targets and never runs them. Fourth instance arrived INSIDE the fix for the third: an `expect_err` that compiled clean by default and failed to compile lean (`Arc<dyn CodeEmbedder>` is not `Debug`). Shipped `6764eb18` as a substitution, not an addition — `cargo test --workspace --no-default-features` strictly dominates the `check` it replaces, list stays 4 commands, ~10s → ~20s — and closed a second gap found while writing it: the canonical list omitted `--all-targets` entirely, so T12's remedy had reached the practice and never the list | validated |
| W-74 | 2026-08-30 | med | When the closure step IS the broken operation, run it rather than routing around it — the write is mandatory, so the reproduction costs nothing and is the one moment the broken path runs against a known-correct expected answer. Distinct from CLAUDE.md's reproduce-before-the-fix-plan rule, which governs the START of a fix and weighs a real cost | Closing BL-48 required the exact `edit_markdown(frontmatter={set:{status:…}})` call BL-48 describes as broken; `artifact(update)` was the known, faster workaround. Writing the prediction down and then using the broken call yielded three findings unreachable by re-reading: the fix is not live in this server, so every catalog measurement in the window is of old code and no tool response says so; the desync is field-SELECTIVE (`get.rs:335` serves `status` from the catalog column, `:525-533` re-parses `extra` from disk), so one payload mixes two epochs and self-contradicts only when `extra` happens to be populated — a bug with none returns a stale status looking perfectly consistent; and chasing that to `/proc/<pid>/exe` found an UNLINKED binary, BL-45's own condition, which corrected a claim I had already written into the record (I cited the on-disk mtime as the running build; the process predates it) | open |
| W-76 | 2026-08-30 | high | Dry-run a writing `fix=` against the REAL corpus and read its output before the first confirmed run — a real corpus contains the input that breaks the convention | BL-50's sidecar name was keyed on the artifact's file stem; two unit tests covered it, gate green 4833/0, design reviewed. The export's dry run printed `docs/research/README.md -> docs/augmentations/README.yaml` on its first line. `README` is not a unique stem, so a second augmented README would have shared the sidecar, the second export overwriting the first's shape and both artifacts restoring to one prompt — the silent shape-loss the feature exists to prevent, reintroduced by its own naming. Invisible in code, tests and review; visible in one line of real output (`f565504a`) | validated |
| W-75 | 2026-08-30 | high | Reproduce before AND after each round of a fix, checking a wedge listener's own hit count, not just wall-clock time or a green test run | A round-1 isolation fix that passed all 77 memory tests still left 2 wedge hits (6.35s) from a third resolution path (`create_semantic_anchors`'s own `RetrievalClient::from_env`) that a concurrent session was independently fixing in the same live working tree mid-investigation — caught only by re-running the reproduction, not by trusting the passing suite | validated |
| W-78 | 2026-08-30 | high | Before attributing a test failure to the change under test, re-run the same failing tests IN THE SAME ENVIRONMENT at the unmodified base. The prompt to do so is a failure in a subsystem the change does not touch | A merge probe returned `4706 passed; 3 failed`; all three were `librarian` temp-guard tests and the branch touches `operator_rules`/`tools::core`/`prompts`, nothing near librarian. The control — same `/tmp` worktree, `checkout --detach experiments`, merge NOT applied — reproduced all three identically, so the failures were the probe's LOCATION, not the merge. Each test builds its "outside-temp" catalog via `TempDir::new_in(current_dir())`, which is inside temp when the checkout is; the assumption sits in a code comment and is enforced by nothing. Without the control the two readings were block-a-clean-merge or bisect-a-working-guard. The failing configuration is the SANCTIONED one — this project points its scratchpad at `/tmp` | validated |
| W-79 | 2026-08-30 | med | Verify a merged branch's imported bug files AT THE DOOR, in the landing session — and mark attribution as inference when nobody bisected | `03b3fb5c` imported five bug files, all `status: open`, authored against a base 103 commits back. One was already fixed: a severity-**high** futex-deadlock report whose own reproduction now runs 77 passed, no hang. It would have entered `experiments` describing a hang that does not happen and sat in every triage query until someone spent a session on it. Four were correctly open and confirmed at the bytes, which is the half that makes it a check rather than a rubber stamp. The flip names `fd638c76` as closer but records the non-reproduction as measured and the cause as NOT — the file already carried one careful negative, and a confident wrong attribution would have undone it | validated |
| W-87 | 2026-08-31 | high | **Partition an open-bug ledger by whether entries carry `unverified:`, then verify only the uncaveated remainder.** Sweep after an evening in which five sessions filed ~9 bug files and committed heavily without reconciling: **0 stale of 15**, against the cadence's cautionary 75% datapoint. 8 of 15 carried a precise `unverified:` — one naming the exact file and line in *another repo* still emitting a false sentence, another naming which of two fix options shipped and which was deferred. Those cannot go stale unnoticed because the caveat is queryable. All 5 uncaveated non-zombies verified by direct test rather than by reading the record: `tree(glob=".gitignore")` returned **0 files** for a tracked file (reproduced live); `--fix` still absent from `codescout doctor`; the buddy banner's cited sid found in `~/.claude`, **another profile**, modified 20 s earlier. Two findings a status check could not produce: `atomic_write`'s *rename* path is guarded while the *initial write* is not, so **a partial fix is what makes a record look addressed** — reading for the presence of a remedy is not reading for the absence of the defect; and reading the code turned up an unfiled sibling, `path.with_extension("tmp")` collapsing `foo.md` and `foo.rs` onto one `foo.tmp`. Second consecutive sweep where staleness tracked the ABSENCE of the caveat field (2-of-12, then 0-of-15). |
| W-86 | 2026-08-30 | high | **When reviewing a change of ARRANGEMENT, grep the file for prose describing the old one — the diff cannot show it.** A peer reordered CLAUDE.md's gate on my proposal and left two sentences of mine false: "the **third command** carries `--workspace`" (now the lean lane) and "still ~26s" (one lane, now beside 71.8s/80.4s). Both sit in UNCHANGED prose in the changed paragraph, so they render as context, not change — diff review is **structurally** blind to them, not merely likely to miss. Found only by grepping the file for my own earlier wording after standing down from editing it. Root cause is a rule this repo already has: an ordinal is **positional**, correct for one arrangement and silently wrong after any reorder — the defect a bare SHA has, which is why fixes are cited SHA + patch-id. "Both test lanes" is content-addressed and cannot rot | Both would have shipped: a correct, well-argued, gate-verified change by a careful session, with the defects in the half no reviewer looks at, in the file every session reads as authoritative — one of them contradicting a measurement in its own paragraph | validated |
| W-85 | 2026-08-30 | high | **Run a suspicious test alone — a green suite cannot distinguish a working guard from a disarmed one.** Closing `BL-66` I found the crate already had a test on that exact path (`probe_ollama_errors_when_unreachable`), nearly identical to the one I had just written. Re-running the SAME pre-fix code two ways: `--exact` **panics**, full `--lib` suite **passes 51/0**. `CryptoProvider::install_default` is process-global and every sibling constructing a `RemoteEmbedder` installs it first, so whichever won the race repaired the defect before the guard could observe it. A third distinct way a check can be worthless — assertion fine, path traversed, siblings disarm it — and both earlier remedies (non-monotone assertion, reachable path) pass it | Without the isolation run I had two wrong conclusions queued: that BL-66 survived only because root's `main.rs` shields the binary, and that my new test's sibling-contamination warning was foresight about a hazard. It was a demonstrated description of the test one file over. One `--exact` run, no code change, is the whole cost | validated |
| W-84 | 2026-08-30 | high | **Mutate once per call site — then check that the kill came from a POSITIVE test.** A peer's law (one rule implemented at N call sites needs N mutations; a single kill proves only that one site is guarded) applied to `entry_status_region`, which locates an entry's status by table row OR by heading + `Status:` line. Both sites turned out independently guarded — mutation A killed only the table test, B only the heading test. The finding is in the SURVIVORS: two `assert!(…is_empty())` tests passed under removal of the very locator each exercises, because a dead locator finds nothing and silence is exactly what they assert. This is not a lax assertion — `is_empty()` is already maximal; **the expected value of an absence test IS the output of the failure mode**, so it cannot be tightened. **Corrected same-day, see the entry:** pairing proves only that the mechanism is ALIVE. The general rule is that a test cannot detect a change its assertion is MONOTONE under — absence under removal, existence under widening — and measuring that found a real hole: under a forbidden-act mutation all six tests passed, so the heading locator's precision was covered zero times. Fixed and verified in both directions | I had those four tests as two pairs by luck, not design — a positive and a negative per rendering because both seemed worth stating, not because I knew the negative rested on the positive. Had one locator got only its silence half, that site would have been unguarded behind four green tests and a mutation story I would have believed: a failure mode with no observer, not a wrong result but an absent one | validated |
| W-83 | 2026-08-30 | high | **Measure the objection that got a task deferred, instead of accepting it or overriding it.** `BL-44` was dropped twice on a stated technical reason — a status comparison is "fragile, and a separate decision" — which was accurate and not decisive. A dry run over the real corpus before any Rust found the fragility is **one convention**: 20 of 26 naive findings are `**done, archived**` against a params value of `done-archived`. Absorbing that left 6 findings on 101 rows and a check worth having. The dry run then reshaped the design three more times — coverage 4 of 9 not 6 (I had read the schema and assumed the rendering, missing 30 of 131 entries), a boundary-anchored predicate instead of the obvious strip-punctuation one, and a `Status:`-line region instead of the whole section. | Shipping from the plan gives 77% noise, half the coverage while looking complete, and `done` matching inside `abandoned` | validated |
| W-82 | 2026-08-30 | high | **A positive control turned a correct conclusion supported by a meaningless number into a real one.** Verifying BL-64's `#[cfg(test)]` fix is absent from the release binary: `nm -C … \| grep -c reindex_cli` → **0**, the expected answer from a real check. Meaningless — the binary is stripped and `nm` reported **zero symbols total**. Caught by the positive control run in the same breath (a symbol known to be compiled in *also* returned 0). `strings` then discriminated: controls 2 and 6, targets 0 and 0. The valuable direction of the *plausible-value* class — its ~11 prior instances are post-hoc, this one was **prevented at the cost of one extra line**. The honest part: the conclusion was RIGHT either way, so stopping at `nm` would have published a true statement on a false basis and never taught me otherwise — method failed, conclusion survived, which is the configuration in which nothing ever corrects you. Now shipped as practice in `probe_augmentation_restore.py` and `packaged_includes.rs` | validated |
| W-81 | 2026-08-30 | high | **Choose a gate's surface by measuring its feedback latency, not by what kind of check it is.** `F-14` proposed "add a CI step" and it went unbuilt for months while its own prediction came true on the next escaping `include_str!`. The reading is not inertia — "CI gate" was the wrong half of the sentence to act on. Two measurements decided it: `cargo package --list` **does not build** (0.21s/package, so the cost objection died at the stopwatch), and the repo was **119 commits ahead of origin across two days**, so a CI-only gate would not have run once in that window. A gate's value is not *does it catch the defect* but *how long until it tells someone*, and that latency is a property of the workflow, not the check — here CI was a weekly signal in a per-commit costume. Put it on the fastest surface that can host it and let slower ones inherit it (`cargo test` is run by both the gate commands and CI). Sibling call: when a check must predict a tool's behaviour, **call the tool** — reimplementing `exclude`'s matching would have been the two-implementations defect deleted from `reindex_cli` two hours earlier | validated |
| W-80 | 2026-08-30 | high | When a fix is "introduce a shared constant so two sides cannot drift", a test that builds its fixture FROM that constant tests one side twice — drive the real producer into the real consumer instead | Written and deleted before committing. It asserted `classify_search_error` matches `SPARSE_MARKER` using a fixture formatted from `SPARSE_MARKER`, so reverting the producer's wording — the exact regression — moved it not at all: a guard for the bug that the bug would pass. Shipped, it would have read as coverage in every later review and the bug file would have cited it. The end-to-end replacement dies on that mutation, printing the original defect verbatim, while both constant-built tests stay green | validated |

| W-88 | 2026-08-31 | high | **A fix option that names a file KIND is a hypothesis about which instances are live — count the population before preferring it for being narrow.** Running the reproduction before the plan (CLAUDE.md § Bug Tracking) on the gitignored-anchor bug: the filed mechanism was churn — a lock file and a db "rewritten on every index build" — and both are **inert here**, last changed 2026-04-17 and 2026-05-13, the retrieval backend having moved to remote Qdrant. What fires is the file's own second-order effect, and it is a different KIND of defect: all five sidecars recorded one identical `.codescout/project.toml` hash matching no file present, mtime three days before the re-anchor that supposedly refreshed it. A tracked sidecar hashing a gitignored file is a cross-machine oscillation with **no fixed point** — A refreshes, commits A's hash, B is permanently stale, B refreshing flips A — so unlike churn, the repair action creates the next defect, for someone not present to see it. Paired second win: three write sites, `refresh_hashes` never re-seeding, confirmed by per-site mutation rather than argued | The plan's option (2), "exclude by kind — lock files, `*.db`, `*.sqlite`, cache dirs", is a churn detector, and reads as the prudent narrow choice **until** you count what it catches. The worst instance is a small, stable, hand-edited TOML: no matching extension, no cache directory, never rewritten. It would have excluded the two already-inert anchors and left `project.toml` — the one firing in all five memories — anchored: 12 bad anchors down to 7, and **zero** of the eight false staleness reports resolved, behind its own green tests. Separately, a fix at `seed_anchors` alone (the site "anchor-selection time" most naturally names) would have compiled, passed the gate, and changed nothing observable, since every affected memory already had a sidecar and so reaches only the two other sites | validated |

| W-89 | 2026-09-01 | high | **Compare `updated_at` across every response a cross-section claim rests on, before publishing it** — and escalate to `artifact_event(action="list")`'s `field_patch` `prev_bytes`/`new_bytes` to learn exactly which state each read saw. A versioned store read through more than one call is not a snapshot | Five discrepancies sitting in context (IC-3 20-vs-18; IC-13/14/15/16 0-vs-16/7/15/2) were **all** artifacts of the torn read [[F-83]] — both commits either side are internally consistent, so the ledger contradicted itself in none of sixteen rows. Publishing would have called a peer's correct in-flight backfill a five-row self-contradiction, inside a report whose declared subject was count-vs-prose staleness, which is what would have made it credible rather than suspect. Detection cost: one `artifact(get)`. The same check then fired AGAIN four minutes later on the same artifact — IC-3's target moved to `OB-7` and `cluster/doc-contradicted-by-code` went 1 → 4, carrying IC-11 over n≥3 — so it is the correct default for the whole window `ListAgents` reports a busy peer, not a one-off | validated |
| W-92 | 2026-09-01 | med | **Resolve authorship with the `Session-Id` commit trailer, never by elimination over `ListAgents`.** The trailer is a POSITIVE identifier (whose a commit IS), costs one `git log`, needs no socket enumeration, and reaches sessions that have already exited | A peer's heads-up asserted "your uncommitted doctor.rs work" reddened the shared gate. Not mine — all four of my commits contain 0 `.rs` files. Their diagnostics (`git status` + `git grep <symbol> HEAD`) soundly established "not mine"; the step to "yours" ran over an incomplete population — `peer-sessions.sh` 5, `ListAgents` 4, `peer-sessions.sh` 5, `ListAgents` 4 — **but that gap did NOT contain the answer: the real owner `c2a08c22` (codescout-68) was inside the visible four, for both of us.** Their error was conversational salience, not coverage; mine named an INVISIBLE session and cited its invisibility as corroboration. So "ListAgents under-reports" is true, documented (BL-58) and did no work in either failure — a false explanation built entirely from true parts. Cheaper than `F-80`'s transcript grep and `F-85`'s ask-the-session, and the only one that reaches exited sessions. `git log --author` is NOT a substitute: every session commits as the same author (IC-10). Cost was one turn spent disproving a negative, not zero. **Rule splits by state — committed: trailer; uncommitted: NO positive instrument exists, ask, and until answered say "not mine", never "yours" — see [[F-89]]** | validated |
| W-90 | 2026-09-01 | med | **Re-verify `path:line` citations in artifacts THIS session authored, after any rebuild or peer commit.** Authorship is no exemption (`R-49`) — a bug file's citations are written at fix time, while peers are still moving the substrate under them | Filed bug cited `post-index-change-stage-log.sh:142` for the mv/rm fallback; the line is `:141` and `:142` is blank. No gate would have caught it: `audit_doc_refs` DOES scan `**/*.sh`, but `scan_code_comments` forces those findings to `Med` and CI runs `--fail-on high`, so it passes by design — the citation survives until a human follows it onto whitespace. The same pass also separated real drift from a false accusation: a `docs(issues):`-titled commit genuinely had changed the script, which looked like a capture, but the diff was a comment-only re-point of an archived path, coherent with its message. Without reading it, the plausible move was filing a third capture bug against a correct commit | validated |
| W-91 | 2026-09-01 | high | **Re-read the substrate before a claim enters a DURABLE, queryable record** — the filesystem for a claim about the filesystem, the implementation for a claim that a capability is missing. Recorded as a **recurrence** of the reconnaissance skill's already-promoted current-state law, not as a new pattern, per that skill's § *Every promotion audits the promoted set* | Two catches. (1) A bug file's `## Residual — still open` said `.worktrees/bench` retains an orphaned gitdir; `ls .worktrees/` shows only `audit-trail-t1`. I was one call from writing a queryable `unverified:` field asserting an open residual that does not exist — into the single file tagged `cluster/record-asserts-an-unchecked-completion`. (2) I had begun drafting a bug asserting `unverified:` is unreachable by any query, on two true pieces of evidence (`find` → `unknown field`, and the librarian guide's "`extra` is NOT catalog-indexed"); `scan_terminal_status_with_caveat` is the deliberate reader and reports **65** records. Both false claims were backed by real evidence about something narrower than the sentence it was licensing. `outgrown` signal (n=1) on the promoted text: it names *fixes* and *prohibitions*, not **filed defect records** — the costliest surface, since a fix assuming a missing capability fails loudly at the call site while a filed bug is durable and nothing re-checks it. **Promote-when FIRED same-day on a MISS** — a filed bug claimed two parsers lacked a fence guard they already had; shipped in `claude-plugins` `b74c730` / codescout-companion 1.19.11 | promoted-to-permanent-docs |
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
> `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
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

**Got:** `memory(action="read", topic="gotchas")` (and every other project-scoped topic except `language-patterns`/`onboarding`) failed with "topic not found" — a separate bug this same session had already found and filed (`docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`): `workspace(activate)` reports 16 memory topics for the `codescout` project, but `memory(list/read)` only sees 2, even with `project_id` passed explicitly. With the documented fix unreadable, the `CARGO_TARGET_DIR=target-release-new` + `Stop-Process -Force` + copy-into-place workaround had to be re-derived from first principles (inspecting `git status` for a pre-existing `target-release-new/` directory as a clue, then confirming the pattern via `Get-Process`).

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
`docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
is now `zombie`, i.e. not reproducible on retest, so "filed but not yet fixed" was stale
in both of its clauses. Two independent things had to be re-checked to retire one entry,
which is the usual shape: a friction's stated cause and its observed cost decay on
different clocks.

**Fix idea / Pointer:** Fix `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`, then re-verify this specific recovery path against the (currently unreadable) `gotchas` memory's actual documented procedure to confirm today's improvised workaround matches or diverges from it.

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

**Status:** fixed-verified — **closed 2026-09-02 by the verify-open sweep, not by anyone who set out to fix it.** The upstream fix landed and nothing flipped this entry. Verified at the copy that LOADS, not at source (`R-89`'s distribution axis): all three profiles serve `codescout-companion@1.20.0` per their `installed_plugins.json`, and `grep -rc 'Valid:\*\* dated [0-9-]* *—'` over each served `skills/reconnaissance/` returns **0**. `references/worked-examples.md:50,96` now show the bare `**Valid:** dated 2026-05-18`, and served `SKILL.md:97` states the rule outright — *"`dated` takes no trailing text"* — which is more than this entry asked for. One stale cache (`~/.claude/…/1.16.15`) still holds the refused form and is served to nobody; left alone deliberately, since deleting a cache on a negative search is what `R-104` forbids.

**Original status line, kept because the sweep's finding is that it went unread:** *open — the fix belongs upstream in*
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
(`docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md`).

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
`docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`'s
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
`1fbfc9b2dd6cb376`, "91.5% of the live memories collection is test-fixture data").
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

### Amendment 2026-08-30 — the pre-add diff is necessary and not sufficient, and `--stat` is not a check at all

This entry prescribed `git diff -- <path>` before `git add <path>` as *the* covering
check. Two failures the same day, from opposite sides, say it is one of two.

**Failure 1 — the window is raceable (codescout-3b's datapoint).** They ran
`git diff` on this ledger, saw exactly their two lines, and staged it.
`git diff --cached` then showed 30: a peer had written a new entry between the read
and the add. The diff and the add are not atomic, and in a shared checkout that gap
is long enough. Their conclusion, which is right: the post-stage check cannot be
raced, because it reads the index you are about to commit rather than the tree you
looked at earlier.

**Failure 2 — mine, and it is the sharper half.** I *did* run the post-stage check
before `7930e0b7`. I ran `git diff --cached --stat`. It printed
`docs/trackers/bug-fix-session-log.md | 61 ++++++++++++++-` and I read that as my
own W-74 plus its index row. Two of those lines were codescout-3b's citation swaps,
and I committed them under my message.

A `--stat` reports a filename and a line count. It **structurally cannot** show whose
lines they are. So it delivers the entire feeling of having verified while showing
none of the evidence — which is worse than skipping the check, because skipping it
leaves you appropriately uncertain.

**The amended rule, three steps:**

1. `git diff -- <path>` before `git add <path>` — the cheap filter. Raceable; keep it
   anyway, it catches the common case for free.
2. `git diff --cached` after staging, **content, never `--stat`** — the guard. Reads
   the index, so nothing can land between it and the commit.
3. If the file holds a peer's uncommitted work that `git add <path>` cannot separate,
   **do not split by hand**. Name the foreign content in the commit message and
   credit it, or hand the file to whoever owns the rest of its dependency unit.

**Step 3 is not hypothetical.** `src/tools/memory/tests.rs` on this date held one
session's five test fixtures interleaved with another's regression test in a single
hunk with no separating context. Neither could split it. It resolved on a mechanical
fact rather than a negotiation: the fixtures call `set_code_search_for_test` and name
`CodeChunkSearch`, both `+0` at HEAD, so the file committed alone does not compile.
Ownership followed the dependency unit — all four files, one commit, by the session
that owned the seam.

**`--stat` belongs to a family worth naming**, because all four members were consulted
precisely *because* someone was being careful, and each returned a plausible value
instead of an error: `git diff --cached --stat` (whose lines); file mtimes, destroyed
as evidence by a `touch` run to bust a clippy fingerprint; a cached clippy
`Finished in 0.48s`, byte-identical to a green that validates the change; and
`ListAgents` reporting `Peer sessions (2)` where three were live
(`docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`).
The last is the worst — the others misdescribe artifacts, that one misdescribes who
else is writing to your tree, which is the premise this whole entry rests on.
### Promoted 2026-08-30 → `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`

The amendment above turned out to be one instance of a wider class, and the class was
promoted rather than this entry. Nine instruments observed the same day across four
concurrent sessions, each returning a plausible value where an error would have been
safe, each consulted **precisely because** its user was being careful: `--stat`, file
mtimes destroyed by the prober's own `touch`, a cached clippy green, a green
`cargo test` on a tree failing `-D warnings`, a mid-run log total, a filter that matched
no test, a page cap, a unit test passing while its caller ignored the policy, and
`ListAgents` reporting `Peer sessions (2)` with four sessions live.

The ADR keeps what this entry could not carry: that **the failure is proportional to
diligence** — skipping a check leaves you appropriately uncertain, running a `--stat`
leaves you confidently wrong — and that two instruments can *agree* while both are
wrong, which is what made the `ListAgents` case worse than under-reporting.

This entry stays as the staging-specific rule and the datapoint that found it. Five of
the nine instances have nothing to do with git, which is why the ADR and not this log is
the right home.
### Second instance, 2026-08-30 — the same class, the opposite direction, same two sessions

The amendment above was written after I swept two of a peer's lines into `7930e0b7`
behind a `git diff --cached --stat`. Six hours later the peer swept **172 lines of mine**
into `7e452f85` — the ADR (+23), this log's W-77 (+61), and `reconnaissance-patterns`'
R-127 (+63), none of which that commit's message mentions. I had all three staged; my own
`git commit` moments later returned *"nothing to commit, working tree clean"*, which is how
I found out.

Both times the **content survived and only the provenance was lost**, and both times the
decision was to leave history alone — the record of the sweep is worth more than clean
attribution on a docs commit.

**This is the amendment's confirming instance, not a counterexample.** A content
`git diff --cached` before `7e452f85` would have shown 172 lines across three files that
session had never touched — unmissable. What the rule cannot do is fire for someone who
does not run it, which is a different problem from being wrong.

What two instances in one day between the same pair actually establish: **in a shared
checkout the sweep is the default outcome, not the accident.** The first was mine, with the
rule un-amended; the second was theirs, with the amended rule published and unread. Neither
session was careless. The discipline has to be cheap enough to run every time or it will
lose to `git add -A` — which is why the amendment lands on *one extra command*, not on a
procedure.

*(Filed by the swept party. The swept party is the one who finds out, and only because
their own commit comes back empty — there is no notification, and the sweeper's commit
succeeds normally. That asymmetry is worth knowing: if your commit reports an unexpectedly
clean tree, check HEAD for your content before re-doing the work.)*
### The mechanism, supplied by the sweeper — and it invalidates this entry's headline claim

The title says *explicit-path staging held*. **It does not hold, and the reason is
structural rather than behavioural.** From the session that ran `7e452f85`:

> The git index is **shared across every session in this checkout**. I ran
> `git add <my-one-file> && git commit -F -`. `git commit` with no pathspec commits the
> whole index — including whatever you had staged at that instant.

That is correct explicit-path *staging*, done properly, and it still swept 172 lines. They
also ran `git diff` before staging — the cheap filter — which by construction cannot see
what another session stages a second later. So neither half of the discipline as previously
written was sufficient: **`git add <path>` scopes what YOU add; nothing about it scopes what
`git commit` takes.**

**The structural remedy is a pathspec on the COMMIT, not just on the add.** Measured in a
scratch repo rather than taken on trust — two sessions' files staged together,
`git commit -m … -- peer.txt`:

| | result |
|---|---|
| files in the commit | `peer.txt` only |
| `git diff --cached` afterwards | `mine.txt` — **still staged, untouched** |

**And one caveat, also measured, which the remedy's proposer did not mention.**
`git commit -- <path>` commits the **working-tree** content of that path, not the staged
content. Staging `STAGED-VERSION`, then appending `WORKTREE-ONLY-EXTRA` unstaged, then
committing by pathspec put *both* lines in the commit and left the tree clean. So the
pathspec form silently defeats a deliberately-staged subset of a file. In a shared checkout
that trade is worth taking — losing partial staging costs less than committing a peer's
work — but it is a trade, and anyone relying on `git add -p` for a hunk-level commit needs
to know the pathspec form discards that.

**Corrected rule, three lines:**

1. `git add <explicit paths>` — still right, still insufficient on its own.
2. `git commit -- <the same explicit paths>` — this is the part that actually scopes the
   commit. Without it, `git commit` takes the whole shared index.
3. `git diff --cached` (**content**, never `--stat`) before committing — the guard that
   catches what 1 and 2 missed, and the only one that shows *whose* lines they are.

Step 2 is new and is the load-bearing one. This entry previously argued 1 and 3 and
omitted it, which is why the class recurred in the opposite direction six hours later
between the same two sessions.
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

**What the instance ratio measures — read this before drawing anything from it.** Four of
the five instances above are mine or corrections to mine. That is **not** evidence about
who is worse at this; it is evidence about **who was holding the lens**. The author of a
ledger on unchecked premises generates instances at a rate nobody else in the room does,
because they are the only one auditing at that depth — `claude-plugins-15`'s two are here
because I went looking at their work, and there is no reason to expect the ratio to
survive anyone auditing mine as closely. Same defect as the `get_guide` samples: a count
assembled by looking is a fact about the looking. The `Promote-when` below forbids the
flattering reading and the unflattering one alike, which is the sign it sits at the right
place. (Observation `claude-plugins-15`'s.)

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

## F-73 — A bug file's "four documentation surfaces" included one nothing reads — grep grain cannot distinguish a live `include_str!` from a test-only one

**Observed:** 2026-08-27, pre-implementation scout for option C on bug
`docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md` (librarian `scope` default). About to edit the four
documentation surfaces the bug file enumerates.

**When:** Reading the seam before the first edit. The bug file was written
earlier *in the same session*, by me, from a grep.

**Expected (bug file):** four live documentation surfaces state the wrong
default and all four need correcting —
`src/librarian/tools/artifact.rs:70`, `src/librarian/tools/librarian.rs:61`,
`src/prompts/guides/librarian.md:79`, and
`src/librarian/prompts/companion_hint.md:56`.

**Got (scouted reality):** three are live; the fourth is inert.
`companion_hint.md` has exactly **one** reference in the entire tree —
`tests/librarian/companion_hint.rs:7`, which `include_str!`s it solely to assert
it mentions no unknown tool names (`hint_mentions_only_real_tools`). Nothing
under `src/` reads it: `grep "companion_hint"` over `src/**/*.rs` returns **0
matches**, and the only other hits are this session's own bug file, two 2026-05
plans naming it under its pre-dissolution path `crates/librarian-mcp/src/prompts/`,
and an archived bug that recorded *"`librarian-runtime.md` and `companion_hint.md`
have no [test]"*. It is a leftover of the crate dissolved in `d48bf992`, kept
syntactically honest by one test and served to nobody.

By contrast `guides/librarian.md` is genuinely live and reached two ways —
`src/prompts/mod.rs:503` (`topic_body`, i.e. `get_guide("librarian")`) and
`src/server.rs:1486` (`static_doc_sources`).

**Probable cause:** the bug file's surface list was built by grepping for the
*wrong string* and never by asking who *reads* each hit. A grep answers "where
does this text appear", which is not the question "which surfaces reach a user" —
and a file that is `include_str!`'d **by a test** looks identical, at grep grain,
to one `include_str!`'d by the server. Same class as R-49 (re-entering your own
artifact is a seam), with a distinct mechanism: not a wrong *claim*, a wrong
*reachability* assumption baked into a worklist.

**Workaround:** correct the three live surfaces. Fix `companion_hint.md` too —
it is three words and leaving a false statement in a tracked file is its own rot —
but do not count it as user-facing, and do not let its edit stand in for the
prompt-surface verification the live guide needs.

**Severity:** med — an implementer following the list would have edited a dead
file believing a user-facing surface was fixed, and would have reported four
surfaces corrected when three were. Costs no build failure, which is exactly why
nothing downstream would have caught it: there is no gate on "did this edit reach
anyone", and the file's one test passes either way since the edit touches no tool
name.

**Related, same scout (no separate entry — the test suite would have caught this
one loudly and cheaply):** `src/librarian/tools/context.rs:2412` asserts
`v["scope"]["applied"] == "repo"` for a scope-unspecified `context` call. Option C
changes that string to `"project"`. Worth noting for its fixture rather than its
failure: it builds `abs_path != git_root` deliberately (*"Place the other repo's
row outside the active git_root so scope=Repo excludes it"*), which is the shape
`scripts/probe_librarian_scope.py` measured at **0 of 923** real recorded scope
blocks. The suite exercises the case production never hits — so the test suite is
a better oracle for this change than the usage log is.

**Status:** fixed-verified — surface list corrected before any edit landed.

**Valid:** dated 2026-08-27

True of `companion_hint.md`'s reachability at `4da15749`; re-verify if anything
under `src/` starts loading it, which would make it live again.

**Rests on:** `grep "companion_hint"` over `src/**/*.rs` returning zero, and
`tests/librarian/companion_hint.rs:7` being the sole `include_str!` of it —
both read this session.

**Fix idea / Pointer:** When a bug file enumerates "surfaces to fix", record for
each one *who reads it* alongside the `path:line`. A surface list is a
reachability claim; it currently reads as a location list, and the two are only
accidentally the same. Bug `docs/issues/archive/2026-08-27-scope-default-is-repo-not-project-across-four-doc-surfaces.md`, this session.

## F-74 — A bug file's own fix plan partitioned a case that does not exist — `managed_roots` never contains another repo, so "rows under a different managed root" was the empty set

**Observed:** 2026-08-27, implementing the `doctor` scoping fix
(`docs/issues/archive/2026-08-27-doctor-reports-other-workspaces-rows-as-violations.md`),
written earlier the same session.

**When:** Scouting `scan_artifact_paths` before the first edit, with the fix plan
already written and approved.

**Expected (the bug file's own `## Fix`):** *"Drop rows whose `abs_path` resolves
under a **different** managed root from `violations`."* The plan's whole shape —
partition the firing rows by which managed root they belong to — rests on that
case existing.

**Got (scouted reality):** it does not exist. `managed_roots`
(`src/librarian/tools/mod.rs:215`) returns the active project's `git_root` and
`abs_path` plus the legacy `workspace.roots` entries — **never another repo**.
And `check_outside_managed_roots` fires only when `containing_root` matches
*nothing*. So every one of the 401 firing rows is under NO managed root; there is
no "different managed root" bucket to move them into. The prescribed fix would
have partitioned an empty set and changed the report by zero rows.

**Probable cause:** the plan was written from the *hint text* ("Rows outside the
active project's roots are EXPECTED when the catalog spans several workspaces"),
which describes the situation correctly in prose, and from `outside_roots_by_project`,
which really does group rows by project. Both make it sound as though the code
already knows which workspace a row belongs to. It does not — that grouping is
inferred from the path string, and nothing was ever compared against another
repo's root. R-95 class: a proposed fix is a claim about current state, and the
least-audited kind, because it reads as a settled decision rather than a
hypothesis.

**Workaround:** measure the partition instead of assuming it. Against the live
catalog (`~/.local/share/librarian/catalog.db`, 4,265 artifacts, 29 distinct
`commits.git_root`), with the session's real managed roots reconstructed from
`~/.config/librarian/workspace.toml`:

| bucket | rows |
|---|---|
| firing total | 402 |
| under an **umbrella member** of the active project's umbrella | 359 (89%) |
| under another repo the catalog holds **commits** for | 33 |
| under **neither** — nothing on this machine claims it | **10** |

That is a real discriminator, computable from data already on hand, and it is the
one the check's own doc comment describes in prose without ever computing. The
shipped fix uses it; the report goes 516 → ~124 findings and this check 401 → 10.

**Severity:** high — the prescribed fix would have shipped, passed its own test
(a test written against the same wrong model would have used two managed roots,
a configuration that never occurs), and left the 78%-noise report exactly as it
was, with a bug file marked `fixed` to stop anyone looking again.

**Status:** fixed-verified — plan corrected before any code was written; the
measured discriminator shipped instead.

**Valid:** dated 2026-08-27

The three bucket counts are a fact about this machine's catalog on this date;
the mechanism (`managed_roots` never contains a foreign repo) is invariant until
that function changes.

**Rests on:** `src/librarian/tools/mod.rs:215` (`managed_roots`) and
`check_outside_managed_roots`'s `containing_root(...).is_some()` early return,
both read this session; the bucket counts on one SQL partition over the live
catalog.

**Fix idea / Pointer:** When a `## Fix` plan names a CASE ("rows that are X"),
check the case is non-empty before designing the partition. A count is one query
and it is the difference between a fix and a no-op that reports success. Pairs
with [[F-73]] — same session, same bug file, both defects of a worklist written
from grep and prose rather than from the data.

## F-75 — A fix verified against its mechanism can leave its own headline aggregate untrue — re-measure the number in the title, not the one in the diff

**Valid:** invariant

**Observed:** 2026-08-27. The bug titled *"`doctor`'s report is 78% other repos'
rows"* (`b176ff103ec56c09`) was fixed, verified, and archived within three hours.
Its verification was unusually good — three parts, checked structurally rather than
by eye, including the one a naive fix breaks silently (the global metric survived:
112 metric keys vs 107 scoped keys, the five-key difference summing to exactly the
10 remaining violations).

**Got:** every claim it made was true, and its own headline was still false. The
title asserts a property of **the report** — *what share of it is foreign* — and the
fix addressed a property of **one check**. Re-measuring the headline question
immediately after, on the same rebuilt binary: 66 of 126 findings, **52%**, still
foreign, now arriving through six other checks. The three verified figures
(516 → 129 total, 401 → 10 for the check, metric unchanged) are all consistent with
that, and none of them asks it.

**Why nothing caught it:** the verification followed the *fix's* scope, which is the
natural thing to do and is what makes it invisible. A fix's scope is the mechanism it
changed; a bug title's scope is the symptom a reader complained about. Those are
different sets whenever a symptom has more than one cause — which is precisely the
case where the fix feels most complete, because the one cause you found really did
account for the largest share.

**The rule:** when a bug's title names an aggregate — a percentage, a share, a rate,
a total, "N% of X are Y" — **re-run the aggregate, not the mechanism, before
archiving.** It is one command, it is the number the title promised, and it is the
only measurement that can distinguish *closed* from *moved*. If the aggregate has
not moved the way the title implies, the honest archive note is "fixed the largest
contributor" and the bug stays open, or a successor is filed naming what remains.

**Generalises past this repo.** Any fix whose ticket says "N% of A are B" and whose
patch touches one producer of B. The tell is a verification section that reports
before/after for the mechanism and never restates the ticket's own number.

**Counterfactual:** without re-asking the headline question the leak would have read
as closed — archived, live-verified, "metric intact" — with 56 unactionable rows
across twelve repos still in every future `doctor` run, and no event scheduled to
re-check it. Filed as `docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md`
in `d0b3b668`.

**Status:** validated

**Promote-when:** a second instance appears of a fix that closed its mechanism while
leaving its ticket's own aggregate untrue. At two datapoints this belongs in the
archive-trigger text of `get_guide("tracker-conventions")`, beside the gate-green
requirement — "if the title names an aggregate, re-measure the aggregate" is the same
shape of pre-archive check as "regression test in place".

**Rests on:** the archive-trigger rule in `get_guide("tracker-conventions")`
(§ Bug files), which requires gate-green plus a regression test but says nothing
about the claim in the title.

## W-72 — A negative result that does not name what it examined is indistinguishable from a true absence

**Valid:** dated 2026-08-27

**Observed:** Three bugs fixed in one session (2026-08-27) turned out to be one defect
wearing three costumes: a tool returned a negative result — `0 matches`, `0 memories`,
`file not found` — without naming what it had examined. In every case the number was
true of what was searched and false as the answer the caller took away.

| Bug | The negative | What it never named |
|---|---|---|
| grep glob anchoring (`444d756c`) | `0 matches` | that the glob admitted no file at all |
| sub-project memories (`fc1bbf21`) | `0 memories` | which of two directories it read |
| workspace clobber, silent form (`76e287f8`) | `file not found` | which tree it searched |

**And the codebase already knew — six times, never once as a named thing.**
`unsatisfiable_absolute_glob`'s `RecoverableError`; `symbols::WalkAudit.accepted`
("Zero here is the strongest signal available that `root` is not the tree the caller
meant"); `grep::WalkAudit::completeness_warning`; `semantic_starved` (`e4569fcc`);
`check_tool_access`'s read-only hint (`00948381`); and `read_file`'s truncation flag
(`21507a26`, patch-id `1bc96ae356d6741c08dbcd5a30c462a8e87ab06b`). Each was
built as a one-off for its own bug.

**Counterfactual — the pattern being unnamed is *why* these were still open.**
`unsatisfiable_absolute_glob` shipped 2026-08-18 and its own doc comment states the
general principle ("A false negative that looks like a finding is worse than an error"),
yet the *relative*-glob case sat unguarded **in the same function** for nine days. And
`symbols::WalkAudit` carried the `accepted` counter that `grep::WalkAudit` needed, one
directory away, for months. Six correct instincts, no shared name, so each new tool
re-derived the lesson from its own outage instead of inheriting it.

**The rule, both halves load-bearing:** a tool that can return a negative result must be
able to say **what it examined** — and must stay **quiet when the negative is
trustworthy**. Dropping either half breaks it. `completeness_warning`'s doc states the
second: `None` is load-bearing "or the warning becomes noise attached to every empty
result and stops being read at all." `unsatisfiable_absolute_glob`'s note records the
matching failure in the other direction — a zero that carried the hidden-paths warning
whose remedy could not have helped, because "naming an unchecked cause ends the search
for the real one." That is why `grep`'s and `symbols`' plain zeros were deliberately
left bare by `76e287f8`.

**Status:** validated — promoted 2026-08-27 to
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

**Promote-when:** FIRED. Criterion was 2+ confirmations across work streams. Met at three
in one session, six prior independent instantiations, and two peer sessions hitting the
same class the same day (`5b9ebedf` grep/gitignored paths, `020ea69a` memory union — whose
own argument, "agreement is not correctness", is this rule's sharper edge).

## F-76 — A bug file's own evidence table cited a line from the call site it exists to distinguish

**Observed:** 2026-08-29, implementing Decision 1 (the zombie-server refusal) from
`docs/trackers/bug-ledger-resume-2026-08-28.md`, having been told to "start with the 1st".

**When:** Reading the fix plan before writing any code — CLAUDE.md's *run the reproduction
before reading the fix plan* rule applied to a bug file rather than a repro.

**Expected (bug file):**
`docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
§ *Re-costed 2026-08-28 — direction 2 is INVERTED at one of its two call sites* carries a
three-row evidence table headed "Measured in `src/retrieval/sync.rs`":

| line | what happens |
|---|---|
| 904 | `flush_pending(...)` — **vectors go into the shared store** |
| 1017 | `stream_index(...)` — embed pass completes |
| 1065 | the sidecar write |

Read as a description of the `sync_project` path, whose inversion is the section's subject.

**Got (scouted reality):** line 904 is a `flush_pending` call inside **`sync_worktree`**, not
`sync_project`. On the `sync_project` path the vectors land *inside* `stream_index` — its own
`flush_pending` calls at `sync.rs:620` and `:626` — and `stream_index` is invoked at `:1017`.
The sidecar write at `:1065` is correct, gated by `opts.record_index_state` at `:1056`.

So the table mixes a line from `sync_worktree` into a table about `sync_project` — in the very
section whose argument is that *the two call sites disagree and no single rule at the writer is
correct for both*. The conclusion is unaffected and in fact slightly strengthened: vectors still
precede the sidecar write on the main path, just via `stream_index` rather than a direct call.

**Probable cause:** The section was written while re-costing direction 2, with both call sites
open at once; `flush_pending` appears in both, so grepping the symbol name yields hits from each
and the nearer one got cited. This is `codescout:R-49`'s class — a self-authored artifact
written *during* the work it describes, at the moment the author's model is most confident.

**Workaround:** None needed for the fix; the corrected ordering is recorded in
`open-issue-work-queue:BL-45` so the next reader inherits the correction rather than
re-deriving it.

**Severity:** med — it did not change what I built, but it is the specific error that could
have. A reader verifying the claim by opening `sync.rs:904`, finding it in `sync_worktree`, and
concluding the re-costing was wrong would be led straight back to refusing the sidecar write —
the inverted fix this section exists to prevent, and the one the file says "would manufacture
the exact failure this file exists to prevent".

**Status:** fixed-verified — ordering re-derived at the bytes and recorded in `BL-45`; the
guard was placed ahead of BOTH paths, which is correct under either reading of the table.

**Valid:** dated 2026-08-29

True of `sync.rs` at `447ffd4a` plus this session's uncommitted change; the line numbers move
whenever `sync.rs` does, which is itself the argument for citing symbols over lines.

**Rests on:** `guard_stale_binary` being placed before the embed pass on both call sites, which
makes the fix correct regardless of which path the table meant.

**Fix idea / Pointer:** When a bug file's evidence table cites bare line numbers in a file with
two similar call sites, name the enclosing function in the row. `grep` output already carries it
(codescout's `grep` appends `[enclosing_symbol]`), so the information was present at the moment
the table was written and was dropped in transcription.

## F-77 — A peer session's hazard report was accurate when written and false when read, and nothing marks the difference

**Observed:** 2026-08-29, three interactive Claude sessions live in one checkout
(`codescout-97`, `fix-embedding-transport-stage-1`, `codescout-24`).

**When:** Assembling a resume-queue status report for the user, after asking both peers what
they owned.

**Expected (peer report):** `fix-embedding-transport-stage-1` reported, in a detailed and
otherwise accurate handoff: *"A THIRD session is active and uncommitted across
src/util/shrink_guard.rs (new), src/util/mod.rs, src/librarian/tools/{artifact,update}.rs,
src/memory/mod.rs, src/tools/markdown/*, src/tools/memory/mod.rs,
src/prompts/guides/librarian.md, CHANGELOG.md. Stage explicitly — never commit -a. Their
in-progress red test made cargo test look broken to me twice."*

**Got (scouted reality):** `git status --short` was clean, and `git show --stat 45a88531`
(10:16:20, *"fix(shrink-guard): refuse a write that loses lines while keeping bytes"*) lists
exactly that eleven-file set. The work had landed roughly eight hours earlier.
`codescout-24` independently confirmed nothing of its own was uncommitted in those files.

A second instance the same hour, opposite direction: `codescout-24` thanked *me* for a
`byte_pct`/dimension edit in its uncommitted `src/memory/mod.rs`. This session had made zero
edits at that point. It retracted on being told, correctly noting it had inferred *which*
actor from evidence that established only *an* actor.

**Probable cause:** A peer's report is a snapshot of its own last observation, and cross-session
messages carry no observation timestamp — only a send time, which is not the same thing. The ET
session's warning was true when it looked and stale when it sent. Nothing in the channel marks
the gap, and the report's confident specificity (eleven correct filenames) is what makes it
persuasive.

**Workaround:** Treat a peer's claim about repository state as a hypothesis and re-run the
cheap check — `git status --short`, `git show --stat <sha>` — before relaying it. Both took
seconds and both inverted the conclusion.

**Severity:** med — cost avoided rather than paid. Relaying it unchecked would have told the
user a live collision hazard existed in a shared checkout, plausibly deterring a test run, and
would have credited this session with an edit it did not make. The second is the worse one:
misattribution that reaches a commit message or a tracker entry is durable and near-impossible
to unpick later.

**Status:** mitigated — verified both claims this session and corrected both peers, who each
confirmed. No mechanism prevents recurrence.

**Valid:** conditional — until cross-session messages carry an observation timestamp distinct
from send time

**Rests on:** `git status` and `git show --stat` being cheap enough that verifying a peer claim
is never worth skipping.

**Fix idea / Pointer:** Two candidates. (1) Peers state repo-state claims with the evidence
attached — *"as of `git status` at 18:19"* — so the reader can judge staleness without re-running
anything. (2) Prefer claims a reader can check from committed history (a SHA) over claims about
working-tree state, which decays fastest. The `45a88531` pointer would have been self-verifying;
the file list was not. Related: `codescout:R-89`, freshness as a property of the copy that serves
you — this is the same law with a *peer's observation* as the stale copy.

**Third instance, and the one that sharpens the diagnosis (added 2026-08-29).** A peer committed
an untracked tracker, `docs/trackers/embedder-stack-ops-session-log.md`, attributing its `F-1`
entry to this session. This session never wrote to that file. It was not the committing session's
either — so a **fourth writer** exists in this checkout, still unidentified. Retracted by its
author in `9641798f`, with `F-1`'s content deliberately left untouched: only the claim about
authorship was wrong, and editing someone's finding to repair your own mistake about them is the
larger error.

That session's own account of the miss is the useful part: it had already read `F-1` closely
enough to notice it described operator exchanges that never happened in its session (*"this is the
laptop not the desktop"*), filed it as a *different incident*, and did not draw the next inference
— that a different incident implies a different session. **It held the disconfirming evidence and
did not use it.**

**And the instruments actively mislead here, which reframes the entry.** The two obvious
discriminators both answer confidently and wrongly: `git log --diff-filter=A -- <path>` on that
file now resolves to `b7c5ce0f`, the session that added it to the index rather than the one that
wrote it, and `git log -S'## F-1'` does the same. Every commit in this repo carries the same
author and committer, so **"which session wrote this" is not a question git can be asked at all**
— yet every instrument you would reach for returns a name. So the failure is not that we guess in
the absence of evidence; it is that the available instruments return confident answers to a
question they cannot see. That is `reconnaissance-patterns:R-125`'s abundant-result form, on
provenance instead of causation, and it is the third time today the same shape has produced a
wrong attribution between three sessions.

**Practical consequence:** authorship in a shared checkout is only knowable if the writer states
it. Nothing recovers it afterwards. A session that creates a tracker should sign it in the file.
## F-78 — A keyword count in a peer's diff was substituted for reading the code under the assertion

**Observed:** 2026-08-29, triaging two failures in a 4642-passed full-suite run, in a checkout
shared by three concurrent sessions.

**When:** Deciding which of two test failures were mine, immediately before reporting the gate
result to the user and messaging both peers.

**Expected (my inference):** `retrieval::embedder::tests::a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever`
failed. `src/retrieval/embedder.rs` was committed and clean, but the ET session's
`crates/codescout-embed/src/remote.rs` was dirty at +137 lines, and
`grep -c timeout` on its diff returned **19**. The failing assertion pins the `e.is_timeout()`
arm of the send error map. I concluded the peer's in-flight read-timeout port was the likely
cause and messaged them to that effect.

**Got — in two corrections, and the second reverses the first:**

1. `codescout-24` ran the full suite with that `remote.rs` dirty throughout (4644 passed, 0
   failed) and the test **5/5 in isolation** against the same tree. My attribution was wrong.
   We both then read that as a timing flake under concurrent load, comparing it to
   `docs/issues/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md`.
2. The ET session read the code and found a **real defect in the test**, fixed in `21174425`
   (*"fix(test): remove a race in the wedged-peer embedder test"*, +18/-10). Verified here at
   the bytes: `embed_one_batch` (`embedder.rs:606`) drives both legs through
   `tokio::try_join!` (`:617`), which returns whichever errors FIRST. The test pointed both
   dense and sparse bases at one wedged listener under a shared 250 ms read bound, while
   asserting on the **dense** leg's marker — so the surfacing message was a coin flip.
   `got: embed_batch sparse send` is the sparse leg winning. `.dense_only(true)` does not help:
   `dense_only` is consulted at `:540` and `:806` but never in `embed_one_batch`. Fixed by
   calling `dense_batch` directly — no join, no race, and it is the function that owns the
   error map under assertion. The test had been racy since `9f4debc3`.

**Probable cause (of my error):** I had a decisive instrument available and used a proxy —
keyword co-occurrence between the failure's subject and a diff's contents. The proxy is
seductive because it is *specific*: 19 is a number, "timeout" is the exact right word, and
specificity reads as rigour. It is still a measurement of a text, not of a cause.

**And the remedy I first reached for was also wrong, which is the more useful half.** My initial
write-up of this entry proposed "run the failing test in isolation" as the cheap decisive check.
It is not: the test **passes** in isolation — that is the whole shape of the bug. Running it
returns green and *confirms the flake hypothesis wrongly*, which is exactly what it did for
`codescout-24` and, at one remove, for me. The instrument whose subject was actually the failure
was **reading `embed_one_batch`**, about a minute's work, and neither of us did it.

**Workaround:** None needed — resolved upstream by `21174425`. Retraction of the `remote.rs`
attribution was sent; the retraction's own claim that the cause was unknown is now also
superseded.

**Severity:** low — revised down from med on evidence. The mis-aimed pointer cost the ET session
one `symbols()` read, not a bisect: I had sent the failing assertion text verbatim, which is what
made the report actionable despite the wrong attribution. Reporting the failure was net positive
— the defect is only observable under concurrent load, so the session that authored, ran and
shipped the test green could not have found it alone. The lesson is about the *attribution*, not
about reporting.

**Status:** fixed-verified — root cause found and fixed in `21174425`; mechanism independently
re-verified here against `embedder.rs:606-617`, `:540`, `:806`.

**Valid:** dated 2026-08-29

**Rests on:** `embed_one_batch` racing its two legs through `tokio::try_join!` and not consulting
`dense_only` — read directly, not taken from the peer's account.

**Fix idea / Pointer:** Two rules, and the second is the one that nearly escaped.
(1) A keyword count is evidence about a diff, never about a failure — the mirror of the ledger's
most-cited law (`codescout:R-3`/`R-113`/`R-77`/`R-79`/`R-104`), which is stated only for the
**empty** result. (2) When picking the "cheap decisive check", ask whether the instrument can
even *express* the failure: a test that passes in isolation cannot be diagnosed by running it in
isolation, and a green there is a negative result being read as absence. Filed as
`reconnaissance-patterns:R-125`. Also worth keeping: sending the verbatim assertion text is what
made a wrongly-attributed report useful anyway.
## W-73 — "Compile-error → green" is a cheap tell that a test has never been RED for a behavioural reason

**Observed:** 2026-08-29, implementing the zombie-server refusal (`open-issue-work-queue:BL-45`)
under TDD, and independently in `codescout-24`'s CM-7 buffer-clamp fix the same afternoon.

**Pattern:** CLAUDE.md already carries *demand a deliberate break* and *ask what mutation would
make this test fail*. What was missing is a **trigger** — a cheap, mechanical tell for *which*
tests to spend a mutation on. This is it:

> In a compiled language, a test whose only observed failure was a **compile error** has never
> been RED for a behavioural reason. Its assertions have never executed against a wrong world.
> Spend one mutation on it before believing it.

The tell is systematically common rather than rare, which is what makes it worth naming. In
Rust the normal TDD RED *is* a compile error — the function does not exist yet, the field is not
on the struct yet. The canonical cycle therefore produces exactly this shape by default:
`cannot find function` → implement → green. At no point did the assertion body run and fail.

My case: `sync_project_refuses_an_unlinked_binary_before_acquiring_the_index_lock` went
`E0425: cannot find function guard_stale_binary` → (implement + wire in one edit) → green. I
replaced the call site with `let _ = guard_stale_binary(...)` and it failed with exactly the
symptom its own doc comment predicts, `Ok(SyncReport { added: 0, .. })`, then restored. Cost:
one edit, one 20s targeted run.

**Counterfactual:** Without the mutation, a wiring test that never fired would have shipped
looking like proof. The specific hazard is that the *policy* unit tests (`Some(true)` refuses,
`Some(false)` and `None` do not) would still pass, so a `guard_stale_binary` that was written,
tested, and **never called** reads as fully covered — five green tests, a defect that is
100% present. This is not hypothetical: the whole point of the entry is that a guard's value is
entirely in its call sites, and nothing else in the suite would have noticed.

**Confirming data points:**
1. This session — `guard_stale_binary` wiring test; mutation fired with the predicted symptom.
2. `codescout-24`, same afternoon, `src/tools/read_file.rs` — mutation-checked its new CM-7 test
   (2 passed / 1 failed with the clamp disabled) **and** found two pre-existing tests asserting
   the exact same property that stay green with the defect present, because their fixture is
   1200 short lines and can never reach the oversized-line valve. That second half is the
   *fixture-unreachable* mechanism the reconnaissance skill already names; the mutation is what
   distinguished the two.
3. Prior art, same class, different session: CLAUDE.md's SDD-ruling lesson *"vacuous assertions
   cluster — 4 found, the fourth only because the final reviewer was told to hunt for one."*

**Impact:** med — one mutation per wiring test, ~1 minute each, against a failure mode that is
invisible to every other gate in the project. Both of this afternoon's instances were in
*load-bearing* code (an indexing guard; a buffer-read clamp), which is where the promoted
CLAUDE.md guidance already says to spend the extra pass.

**Promote-when:** Threshold met — two independent datapoints, two sessions, two subsystems, one
afternoon. Per the reconnaissance skill's *audit the promoted set*, this is case 2, **outgrown**:
the existing law is true and too narrow, so the remedy is to re-promote the evolved form rather
than file a third recurrence. Candidate destination is the `test-design-discipline` memory
(project-shaped surface, advertised by name at SessionStart) with the one-line rule: *a test
whose only observed RED was a compile error is unverified — mutate it once.* Held for the user's
call rather than written unilaterally, per the skill's *the channel is ungated — guard it*.

**Status:** validated — two datapoints, both with the mutation actually run and the predicted
symptom observed, not merely asserted.

**Valid:** dated 2026-08-29

**Rests on:** compile-error-as-RED being the normal TDD cycle in statically typed languages,
which is what makes the tell high-yield rather than a curiosity. Weakens in dynamic languages,
where a missing function fails at assertion time and the test *is* behaviourally RED.

**Sharpened 2026-08-29, and the sharpening is not mine.** `codescout-24` offered a better
statement of the underlying class after the third instance landed, and it subsumes the trigger
above rather than sitting beside it:

> The trap is not a weak assertion — it is **a fixture that cannot reach the code path**. The tell
> is that the test's DATA lacks the property under test, while its NAME and shape claim
> otherwise.

That is the more useful form, and it covers a case the compile-error trigger cannot: my trigger
only fires on tests that are **new**. A pre-existing test with an unreachable fixture was written
long ago, has always been green, and no authoring event will ever prompt anyone to look at it —
which is precisely where both of that session's instances sat. Keep both: the compile-error tell
is the cheap prompt at authoring time, the fixture-reachability question is the one to ask of a
test you inherited.

**Second instance, and the sharper of the two — `eval_matches_compile_on_fixture`
(`src/librarian/filter.rs`).** A *differential* test asserting that the SQL compiler and the
in-memory evaluator agree on the same filter AST. It passed before and after a fix to `in`/`nin`
on array columns, because its fixture table is `CREATE TABLE e (id, status, confidence, title)` —
**no array column at all**, so it cannot reach the branch under dispute. That session's reading of
why this one is worse than its own two: *"a differential test that cannot reach the branch is
worse than a plain one, because the shape advertises rigour."* Two-engine agreement is about the
strongest guard a reader can be shown, and it was silent on a defect where one engine returned
nothing and the other returned every row.

**Method note — corrected 2026-08-29, having first written it backwards.** The three were NOT all
found the same way, and the difference is useful rather than incidental:

| instance | how it was found | why that method |
|---|---|---|
| `filter.rs` `eval_matches_compile_on_fixture` (mine) | **read the fixture** — opened the test, read its `CREATE TABLE e (id, status, confidence, title)`, saw no array column | the missing property is **structural**: an absent column is visible in one line, no run needed |
| `read_file` over-budget line (`codescout-24`) | **ran a mutation** — disabled the clamp, 2 passed / 1 failed | the missing property is **distributional**: a 1200-short-line fixture does not announce that it can never produce a long line |
| ~~`shrink_guard`~~ (`codescout-24`) | **not an instance — struck 2026-08-29** | an *avoided* trap, not a discovered one: that session noticed while writing a NEW test that uniform-length lines make the byte and line ratios move together, and chose a different fixture. Nothing was ever blind |

So the cheap read works when the fixture lacks the property **structurally** (an absent column, an
absent field, a type never constructed — visible in one read of the fixture declaration), and the
mutation is the fallback when it lacks it only **distributionally** (1200 lines that could each
have been long, and none are — which announces nothing). Read first, mutate second. That makes the
intervention cheaper than "always mutate", which is how I first wrote it.

**Count corrected 2026-08-29: two discovered instances, not three.** `codescout-24` re-derived its
own rows while applying the method correction above and found that its `shrink_guard` case was
never an instance of this class — it was a trap **anticipated while writing a new test and
avoided**, not a blind incumbent discovered. Its own account of why it had been counted: *"the
only reason was to get the tally to three."* Struck in `ec853747`, recorded in place rather than
edited out, on the grounds that a tracker inflating its own evidence is exactly the failure that
file exists to catch. The avoided trap is still evidence the procedure works **prospectively**;
it is not evidence of a blind incumbent, and the two must not be summed.

**And note the fourth test in this session is a different class, not a fourth instance of this
one.** `guard_stale_binary`'s wiring test (`src/retrieval/sync.rs`) was found by running a
mutation, but its problem was never an unreachable fixture — it was compile-error-to-green, this
entry's own trigger. Conflating the two is what produced the backwards note, and it also went out
in a message to a peer, who wrote it into `test-escape-hardening` § *Scope of the claim* as "all
three found by running a mutation, none by reading." Corrected with them; recorded here because
the error was mine at the source.

Reading a test still tells you only what it asserts. But reading its **fixture** can tell you
whether the assertion is reachable — sometimes for free.

**Promotion, restated — with the honest count.** Two distinct claims, each at exactly two
datapoints, one afternoon, two sessions:

- *Compile-error → green means never behaviourally RED* — `guard_stale_binary`'s wiring test
  (mine) and `codescout-24`'s new CM-7 test. Both new tests, both mutation-checked, both fired.
- *A fixture that cannot reach the code path* — `eval_matches_compile_on_fixture` (mine, found by
  reading) and `read_file`'s two over-budget-line incumbents (theirs, found by mutation). Both
  pre-existing.

Threshold met for both, and **not** three-for-either. If this goes to the `test-design-discipline`
memory, the rule to write is the fixture-reachability one with the compile-error tell as its cheap
authoring-time prompt — not the trigger alone, which is the narrower half.

**Procedural finding, worth more than the rule it came from.** The correction chain ran: a peer
wrote up my instance secondhand → asked me to check it rather than assume → my check found a
*method* error (read vs mutation) that had nothing to do with attribution → applying that fix made
that session re-derive its own rows → which exposed a self-serving over-count in its evidence that
no attribution check was looking for. Neither of us predicted that when we agreed to the check; we
both framed it as an accuracy courtesy about *my* work. **Having the source check a write-up of
their own work is worth doing for what it forces the author to re-derive, not for the attribution
it confirms.**
### Promoted 2026-08-30 → memory `test-design-discipline` § The mechanical backstop

The `Promote-when` criterion had fired twice over by the time it was harvested: two instances
on 2026-08-29 (`guard_stale_binary`'s wiring test, then `read_file.rs`), and two whole cohorts
on 2026-08-30 — BL-48 and BL-60, each shipping 8 tests that went compile-error → green, each
spending 3 mutations, all 6 killed.

It went to the memory rather than to CLAUDE.md because it is a **review lens**, and that memory
is where the lenses live and is auto-loaded at session start. CLAUDE.md routes; the memory
instructs.

**A second, distinct lens was promoted alongside it**, and it is this entry's caller-side twin
rather than a restatement: *a passing unit test proves the function, never its reach — mutate
the CALLER.* W-73 says a test that never went red for a behavioural reason has not been
exercised. The twin says a test that HAS gone red still proves nothing about whether anything
calls the code. Two sessions found it independently the same day, each by running the mutation
rather than reasoning about coverage: `server.rs:374`'s install (deleting it leaves all 8 green)
and a peer's policy resolver (mutating the caller killed two wire tests and left the pure
resolver test passing). Neither would have been believed without the red — the peer had written
the prediction into a doc comment beforehand and says so.
## F-79 — I told a peer a file was clean, kept working on it, and their commit swept my changes

**Observed:** 2026-08-30, shared checkout, two live sessions.

**When:** After committing my own work and explicitly messaging a peer that
`docs/trackers/open-issue-work-queue.md` was clean so they could perform an archive move plus
citation re-points, which `get_guide("tracker-conventions")` requires to land in one commit.

**Expected:** their commit `a38ef4d1` contains the archive move and the re-points, and nothing
else.

**Got:** it also contains my re-statusing of `BL-51` (dropped) and `BL-52` (blocked), written
*after* I sent the all-clear. Verified in the commit's own diff: 5 `BL-51` hits, 5 `BL-52` hits,
and my distinctive phrasing (*"both claims refuted"*, *"sketched fix refuted"*). No data lost;
`git log` simply now answers *"why was BL-51 dropped?"* with a commit titled *"archive the
buffer-read bug"*.

**Probable cause:** mine, not theirs. My message *"open-issue-work-queue.md is clean"* was true
when sent. I then scouted BL-51/BL-52, re-statused both in that same file, and never re-notified.
They staged an explicit path — the correct discipline — and picked up my working-tree changes
because that is what `git add <path>` does.

**Not a duplicate of `W-69`, and that is the point.** `W-69` prescribes `git diff -- <path>`
before `git add <path>`. That is the **committer's** check and would indeed have caught this. The
half nobody had written down is the **notifier's** obligation:

> "Clean" is a statement about an **instant**. Having told a peer a file is clear, you own
> re-notifying them if you dirty it again — because a clearance handshake decays the moment its
> sender keeps working, and its receiver has no way to detect that.

The two are complementary and fail independently: the committer's check catches it at write time,
the notifier's discipline prevents the race from existing. `W-70` already found that a wire
handshake — *"file is clear as of `<sha>`, write freely"* — is the only thing that **prevents**
rather than repairs here. This entry is that handshake's missing second half: clearance is not a
state, it is a claim with a timestamp, and only the sender knows when it expires.

**Severity:** low — no loss, no rework, and the peer behaved correctly throughout. Recorded
because the cost is legibility, which is the failure mode this project's whole tracker discipline
exists to prevent, and because it is the fourth cross-session provenance error in two days, all
of them resolved by a check somebody could have run and nobody owned.

**Status:** mitigated — peer informed, entry filed. History deliberately NOT rewritten: amending
a shared commit to repair a provenance line is a worse trade than leaving it and recording what
happened.

**Valid:** invariant

The mechanism is a property of `git add <path>` plus asynchronous messaging, not of this repo or
this pair of sessions.

**Rests on:** `W-69` (explicit-path staging, and its `git diff` covering check) and `W-70` (the
clearance handshake as the only preventive measure). This entry supplies the sender-side
obligation neither states.

**Fix idea / Pointer:** When you send an all-clear on a shared file, either stop editing it until
the peer confirms, or send a retraction the moment you touch it again. Cheapest durable form:
name a SHA in the all-clear — *"clear as of `<sha>`"* — so the receiver can detect divergence
themselves instead of trusting a claim whose expiry only you can see.

**The committer's half, from the committer (added 2026-08-30, their words, their analysis).**
The session that made the commit declined to file its own entry — correctly, citing the
reconnaissance rule that a recurrence of an already-promoted law is a defect in the promoted
text, not a new datapoint — but pushed back on my taking all of the fault, and supplied the
sharper finding:

> I used `git add -- <explicit paths>`, not `git add -A`. That is the careful form, and I think
> it is precisely what **suppressed** the check — naming the paths FELT like the guard, so
> looking inside them seemed redundant. The explicit-path form protects against sweeping up
> files you never thought about; it does nothing about a file you did think about and were
> **wrong** about. **Naming a path you believe is yours is not evidence that it is.**

That is a defect in `W-69`'s wording, not a second instance of it. `W-69` currently leads with
explicit-path staging and carries `git diff -- <path>` as its covering check; the ordering
implies the first is the protection and the second the belt-and-braces, when the measured
failure is that the first **induces skipping** the second. Their recommendation, which I agree
with: put the diff check **above** the path discipline rather than beside it. A precaution that
feels like the guard is worse than no precaution, because it spends the vigilance the real check
needed.

Recorded here rather than as a `W-69` rewrite because two sessions should not both edit that
entry in the same hour — which is this very entry's subject. Flagged for the next reader of
`W-69` to fold in.

Their closing observation, worth keeping verbatim because it is the honest shape of the failure
and not a flourish: *"I wrote 'knowing a rule and executing it are different acts' to my user
about the paths-and-ids sweep, in the same breath as a commit where I had done exactly that to a
different rule."*
## W-74 — When the closure step IS the broken operation, run it — the reproduction is free

**Valid:** dated 2026-08-30

**Category:** method · **Status:** open — 1 datapoint

**Observed (2026-08-30, BL-48).** Closing the bug file required flipping
`status: open` → `fixed`. The documented call for that is
`edit_markdown(frontmatter={set: {status: …}})` — which is *the exact call BL-48
describes as broken*. The workaround (`artifact(action="update")`) was known,
available, and faster.

**Did instead.** Wrote the prediction down first — *"disk will flip; the catalog
will still say `open`"* — then used the broken call deliberately and queried the
catalog. Disk said `fixed`; `artifact(action="get")` said `open`.

**Counterfactual.** Routing around the bug would have closed the file cleanly and
taught nothing. Three findings came out of the reproduction, none of them
reachable by re-reading code:

1. **The fix is not live in this server**, so every catalog measurement taken in
   this window is a measurement of the old code — and no tool response says so.
2. **The desync is field-selective, not total.** Verified at
   `src/librarian/tools/get.rs:335` (`status` served from `row.status`, a catalog
   column) against `:525-533` (`extra` re-parsed from the file on every call, with
   a comment saying why). One `get` payload therefore mixes two epochs and
   contradicts itself — `"status": "open"` next to `"closed": "2026-08-30"`. That
   contradiction is the reader's **only** tell, and it appears solely because this
   file happened to carry `extra` keys. A bug with none returns a stale `status`
   that looks perfectly self-consistent.
3. **Chasing (1) to `/proc/<pid>/exe` corrected a claim already written into the
   bug file.** I had cited the on-disk binary's mtime (11:39:59) as the running
   server's build time. The process started 11:10:11 and its binary is *unlinked*
   — an mtime says nothing about a server that predates it. The conclusion held
   and strengthened; the stated reason was wrong, and only the probe caught it.
   That state is also BL-45's own condition, so `guard_stale_binary` would refuse
   a re-index here — except it is absent from the running image for the same
   reason.

**The rule.** When the bookkeeping step that closes a bug *is* the operation the
bug describes, run it rather than routing around it. The write has to happen
anyway, so the reproduction costs nothing, and it is the one moment the broken
path is exercised against a known-correct expected answer.

**Distinct from** CLAUDE.md's *"run the reproduction before reading the fix plan"*,
which governs the **start** of a fix and is justified by the plan being a
hypothesis. This governs the **end**, and rests on a different mechanism: the
operation was mandatory, so there is no cost to weigh at all.

**Promote-when:** a second instance where a closure step doubles as a
reproduction, **or** one where routing around a known-broken closure path hid a
live regression. At two, promote to CLAUDE.md § Bug Tracking beside the existing
reproduction rule.

## W-75 — Reproduction-driven fix caught a 2-round isolation gap and a live stale-buffer trap from a concurrent session

**Observed:** 2026-08-30, picking up the open half ("Fix 2") of `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`: extending ambient-config isolation to every `tools::memory::tests` fixture that builds a real `Agent`.

**Pattern:** Reproduce before fixing, and re-reproduce after each round rather than trusting the static trace. Point the ambient `CODESCOUT_EMBEDDER_URL` at a listener that accepts-and-never-answers (logging each connection), then check the listener's own hit count — not just test wall-clock time — before and after.

**What the reproduction caught that static reading alone would have missed:**

1. **Round 1** — static audit found 5 fixtures building `Agent::new(...)` directly (bypassing the two documented `test_ctx_with_project*` helpers): `test_ctx_no_project`, `multi_project_ctx`, `memory_write_routes_to_project_dir`, `memory_write_accepts_project_alias_for_project_id`, `workspace_ctx_with_sub_project`. Two of these (`memory_write_routes_to_project_dir`, `memory_write_accepts_project_alias_for_project_id`) reach a REAL successful "write" action with zero isolation. Measured before fixing: 12.25s / 4 wedge hits and 6.40s / 2 wedge hits respectively (vs. near-zero for an already-isolated control). Added `set_memory_embedder_for_test` + `set_semantic_memory_store_for_test` to all 5.

2. **Re-reproduction after round 1 still showed 2 wedge hits** (6.35s), not zero — proving the fix was incomplete despite passing all 77 memory tests. Tracing `create_semantic_anchors` found a THIRD, architecturally separate resolution path: it built its own `RetrievalClient::from_env(...)` for code-chunk search, which neither the embedder nor the store seam could reach (the code's own comment already documented this as intentional: "the embedder seam only covers the dense vector path").

3. **While tracing that third path, a second, unrelated agent session was independently editing `src/agent/mod.rs` and `src/tools/memory/mod.rs` in the same live working tree** — it had *already* built a proper `Agent::code_search` / `set_code_search_for_test` seam for exactly this gap, mid-investigation. A `symbols()` read of `create_semantic_anchors` returned the OLD `RetrievalClient::from_env` body from a stale buffer moments after the concurrent session's edit had landed on disk; re-reading fresh (`git status --short` showing `M src/agent/mod.rs` I had never touched was the tell) surfaced the new seam. That seam only auto-installs its `NoCodeSearch` default inside `test_ctx_with_project_raw` — none of the 5 fixtures from round 1 go through that helper, so all 5 still needed `set_code_search_for_test(NoCodeSearch)` added explicitly. After that: both previously-vulnerable tests ran in 0.03–0.04s with genuinely **zero** wedge connections.

**Counterfactual:** Trusting the round-1 fix because "all 77 tests still pass" would have shipped a test suite that looked hermetic and was not — the exact failure shape this whole bug is about (a coupling invisible to the pass/fail gate, visible only in wall-clock time and a wedge listener's hit count). Trusting a single static read of `create_semantic_anchors` mid-investigation would have led to either (a) redundantly reimplementing a seam a concurrent session had just built, or (b) missing it entirely and reporting the fix as complete when a third gap remained.

**Impact:** high — this closes a live pollution/coupling risk on a project whose own retrieval stack is genuinely running on the development machine right now, and it's a second, independently-found confirmation of `codescout:R-89`'s law ("a served copy can be stale even when the read succeeds") — this time on a *live concurrent edit* axis rather than build/process/distribution.

**Confirming data points:**
1. This session (2026-08-30) — two full rounds, both caught only by re-running the reproduction, not by reading the diff or trusting a green test run.

**Status:** validated — gate green (fmt, clippy --workspace --all-targets --features local-embed, cargo test 4810 passed, cargo check --no-default-features), reproduction confirms zero coupling.

**Valid:** dated 2026-08-30

**Fix commit:** `fd638c76` ("fix(memory): close the third ambient-config path in the memory tests (T10)") on `experiments`, patch-id `2afe9f1378e8dece47fea600ecf840c57a215ab0` (`git show fd638c76 | git patch-id --stable`, verified independently rather than trusting the relayed value).

**Rests on:** `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`'s updated `## Fix` section, which carries the exact before/after wedge-hit counts.

## W-76 — Running the feature found a defect both its unit tests agreed was correct

**Valid:** dated 2026-08-30

**Observed:** 2026-08-30, shipping BL-50 (augmentation shape travels in a committed
sidecar, `e799f29d`). The default sidecar path was keyed on the artifact's file stem:
`docs/trackers/tool-usage-patterns.md` → `docs/augmentations/tool-usage-patterns.yaml`.
Two unit tests covered it, the whole gate was green at 4833/0, and the design had been
reviewed and approved.

Then I ran `librarian(action="doctor", fix="export_augmentations")` as a dry run against
the real catalog. Its first line:

```
docs/research/README.md  ->  docs/augmentations/README.yaml
```

`README` is not a unique stem. A second augmented `README.md` anywhere in the tree would
have been assigned the same sidecar; the second export would silently overwrite the
first's shape, both artifacts would then declare that path, and both would restore to one
prompt. That is exactly the silent shape-loss the feature exists to prevent, reintroduced
by its own naming convention. Fixed in `f565504a` — the default is now derived from the
whole repo-relative path, which is injective because paths are.

**Why the tests could not see it, and this is the transferable half.** The unit test
asserted that `path_for` and `rel_path_for` **agree with each other**. They did. Both
were stem-keyed and both were wrong together, so the assertion passed on a defect it was
positioned to catch. A test that pins two functions to one another is blind to their
shared convention being unsound — it can only see them diverging.

**Counterfactual:** the collision is invisible in the code, in the tests, and in review.
It becomes visible the moment a real corpus is fed through, because a real corpus contains
a `README.md`. Nothing else on this project's gate would have surfaced it before the
second augmented README existed, at which point the loss is silent and already done.

**Status:** validated

**Promote-when:** a second instance appears of "the dry run of a writing fix exposed
something its tests asserted correctly". At that point the rule is stronger than this
entry: a `fix=` that writes should be dry-run against the real corpus before its first
confirmed run, and the output READ rather than counted.

## F-80 — I eliminated down to one candidate over a population no instrument reports completely, and a peer dropped a correct position because of it

**Valid:** dated 2026-08-30

**Observed:** 2026-08-30. Contested hunks appeared in `src/tools/memory/tests.rs`. Three
sessions each denied them. I read the first 28 of 200 added lines — the `NoCodeSearch`
stub and its doc comment, which genuinely WERE
`fix-embedding-transport-stage-1`'s — generalised from the part to the whole, and sent
`codescout-ae` a **positive identification**: that session wrote both the hunks and W-75.

Two things were wrong with it, and only one is the obvious one.

The visible error is arguing from 14% of the diff. The structural error is the closing
step: my support was "the other two candidates deny it, and `ListAgents` shows three
sessions." That is elimination, and **elimination is only as strong as the completeness of
the candidate set** — which no instrument here reports. `ListAgents` is scoped to one
Claude profile's socket directory. This machine runs three (`~/.claude`, `~/.claude-sdd`,
`~/.claude-kat`, per the first section of the global CLAUDE.md). A fourth session, live in
`~/.claude-sdd` since 11:10:52, shared the checkout, wrote the hunks, and could not be
seen or messaged by any of us. Filed as BL-58.

**Cost, which is what makes this worth an entry rather than a note.**
`fix-embedding-transport-stage-1`'s FIRST read — "these are not mine" — was correct, and
they abandoned it. Not under pressure: to a well-argued, artifact-citing analysis that
happened to be wrong. Every other misattribution that day was someone claiming authorship
too readily; this was the inverse, and I caused it. Their own account of it is the
sharpest statement of the lesson: a first-person negative about your own authorship is
close to the strongest evidence available in a shared checkout, and it should not yield
to a third party's inference from a diff, because the third party can only ever be
inferring.

**Remedy, and it is a different KIND of instrument rather than a more careful use of the
same one.** Parse the session transcripts for `tool_use` entries with a write tool
(`edit_file`, `edit_code`, `create_file`, `edit_markdown`, `artifact`) and match the input
payload against a distinctive symbol from the diff. That answers *who wrote this line*
directly, instead of answering it by exclusion. It also works on a session that has since
exited, which the process table cannot reach.

Glob **all three profile directories**, not just your own — that is the load-bearing half.
My first pass searched only `~/.claude` and returned a clean, complete-looking **zero** for
`"Network-free stubs"` across what I then believed was every transcript. Same blind spot,
reached by a different route; without CLAUDE.md's first section I would have concluded the
blocks had no author.

**Also caught, and worth recording because it is the same reflex in the opposite
direction:** mid-investigation I found four writes from me to that exact path, concluded
"it is mine", and was about to report a false confession. They were dated 2026-08-28 and
were about the memory-write shrink guard. Path matched, content did not. Path-matching is
elimination wearing a different hat.

**Status:** open — **promote-when FIRED 2026-09-01**, third instance recorded below. Promotion
to CLAUDE.md is now owed and is a user decision, not an agent one.

**Third instance — 2026-09-01, and it is the sharpest of the three because the instrument
printed the warning and I read past it.** A peer attributed `src/util/librarian_guard.rs` to
session `3e275c54` using `scripts/file-provenance.py`. I re-ran the same command on the same
path, got `UNKNOWN`, and charged them with reporting out-of-window evidence as in-window. The
refutation was wrong and the mechanism is nasty: the default window is **commit-relative**
(`scripts/file-provenance.py:325`, *"since this path was last committed"*), and `c26943b5`
landed at 14:59:59 — between their reading and mine — moving the floor past every write that
supported the claim. **A verification re-run therefore silently answers a DIFFERENT question
than the original**, and the commit is what erased its own provenance evidence.

The elimination is the same shape as this entry's: `UNKNOWN` is a partial population (writes
since the floor) and I read it as complete (*"the tool does not say what you reported"*). What
makes it worse than the two prior instances — the tool's own output said, verbatim, *"That is a
statement about coverage, NOT about ownership — Bash writes this tool's heuristics miss look
identical. Do not read it as 'not mine'."* I quoted that line in my own message and still used
the result as a refutation. Knowing the class, and being told it in the output, prevented
nothing — which is `Observer Blindness`'s whole argument for a mechanism over an intention.

**The positive identifier existed the entire time**, which is exactly what this entry's
Promote-when prescribes: `c26943b5` carries `Session-Id: 3e275c54-d006-4cdc-a02f-c77837c1f1a0`
in its commit trailer. One `git log -1 --format=%B <sha> | grep Session-Id` settles positively
what no amount of window-scoped absence can settle by elimination.

**Outcome differed from this entry's, and the difference is the lesson.** Here the peer
*dropped a correct position*. This time they re-derived, produced the commit trailer and the
window floors, and refuted me — so the cost was one round-trip rather than a lost correct
claim. The variable was not care on either side; it was that the peer treated my contradiction
as a claim to check rather than as authority. **SETTLED 2026-09-01 by asking the session, and the procedure is the finding.** `codescout-17`
confirmed it is `3e275c54-d006-4cdc-a02f-c77837c1f1a0`; `c26943b5` is theirs and the original
attribution was right. It holds this as a **fact rather than a derivation**, because the harness
hands every session a scratchpad path carrying its own session id as a path component
(`/tmp/claude-*/<project>/<session-id>/scratchpad`). Verified here against known ground truth:
this session's scratchpad parent directory is its session id, and it matches
`.buddy/.current_session_id` independently — two channels, one answer, neither inferred.

**So this entry's Promote-when now has an available PROCEDURE, which it did not when written:**
**ask the session, and have it quote its scratchpad path.** It is stronger than the commit's
`Session-Id:` trailer, because the trailer exists only once a commit lands and this works on
**uncommitted** state — which is where every misattribution today occurred. Record it for what it
is: a **session→id** join the session can publish on request, **not** a pid→session join. There is
no pid→session key on this machine, verified two ways.

**Keep the failed derivation in the record, because it is this entry's actual subject.** A chain
was offered — socket filename = PID, PID 3624594 = a live `claude` started 11:26:14, therefore
`3e275c54` — and it reached the **right answer without closing**. Checked against a positive
control on this session, `/proc/<pid>/environ` carries no session id at all and neither does the
transcript JSONL, so the join it needs does not exist; and three `claude` processes started
inside **145 seconds** (11:26:14 / 11:27:28 / 11:28:39), so "started 11:26" admits three
candidates. Content-match-plus-testimony landing on the truth does not retroactively make it a
key. **This is the case where a correct conclusion could have been drawn from insufficient
evidence and nobody would ever have noticed** — which is precisely the risk this entry exists to
name, arriving here as a near-miss rather than a failure.

**Population note — RETRACTED 2026-09-01, an hour after it was written, and the retraction is
the better entry.** It read: *"a second line of evidence WAS sound — `ListAgents` (3 live
sessions) and the filesystem's live-transcript-write set (3 files in 10 minutes) are
independent instruments returning the same closed population."* **False.** Both were
**per-profile**. The transcript glob ran only in `~/.claude-sdd/projects/...`; re-measured
across all three profiles, `~/.claude` held **3 more** freshly-written transcripts it never
looked at. And a socket enumeration — `/run/user/1000/cc-socks/`, which is per-**user** — lists
**6** live sessions whose cwd is this checkout, against the 2 peers `ListAgents` reported.

**The two instruments were not independent; they shared a scope, and agreed BECAUSE of it.**
One blind spot counted twice is indistinguishable from corroboration at the point of use — which
is why "cross-check with a second instrument" is not sufficient advice on its own, and why the
check has to be *does this instrument span the whole namespace*, not *do two of them agree*.

Worth recording plainly: this entry's subject is **a partial population read as complete**, and
the paragraph asserting my population was provably complete was itself a partial population read
as complete — written in the entry that defines the class, while I was actively invoking the
class. That is the fourth instance today of knowing-the-class preventing nothing, and it is the
argument for the mechanism rather than the discipline, made against its own author.

**Corrected procedure, from `codescout-17`, who found it via
`codescout-companion:reaching-peer-sessions`:** enumerate over **sockets across all profiles**
→ intersect with the write-derived set (`scripts/file-provenance.py`) → **ask the survivors**.
`ListAgents` belongs nowhere in an authorship question: **discovery is per-profile
(`$CLAUDE_CONFIG_DIR`), delivery is per-user (`/run/user/<uid>/cc-socks/`)**. The three sessions
neither of us could route to were addressable by socket path the entire time — so `b2a50de8` and
`bcc98c22` were never unreachable, and the fifth instance's conclusion that *"no routing decision
was available"* was a claim about the world that was a fact about the instrument. CLAUDE.md §
*Observer Blindness* carried the same false example and was corrected in the same pass.

**Promote-when:** **FIRED** — a third session-attribution error lands whose root cause is a
profile-scoped or otherwise partial population read as complete. Three now exist (this, the
buddy banner in BL-59, and the `file-provenance` window above). The rule to promote, unchanged
from when it was written: never close an authorship question by elimination; identify
positively from write history, across every profile. The third instance adds one clause worth
carrying with it — **a windowed instrument's zero is scoped to its window, and re-running it
later silently moves that window.** And the settlement above adds the procedure the rule always
lacked: **ask the session and have it quote its scratchpad path** — the one identifier that is
given rather than inferred, and the only one that works before a commit exists.

## W-77 — A remedy that closes the COMPILE half of a class reads as the fix — the runtime half then stays open and green

**Valid:** dated 2026-08-30

**Category:** gate-design · **Status:** validated

**Observed (2026-08-30).** The lean-build blind spot has now produced **four** instances.
After the second (T12), the remedy adopted was *"add `--all-targets` to the lean check"* —
which is correct, sufficient for that instance, and was reasonably read as closing the
class. It closes the **compile** half only.

`2c6f2677` (2026-08-29) added a `remote-embed` transport bail and turned three
pre-existing **ungated** tests red under `--no-default-features`. The lane stayed red for
**over a day**. During that window at least three sessions ran the full documented gate
green — I ran it four times myself — because `cargo check --no-default-features
--all-targets` **compiles** the lean test targets and never **runs** them. Only CI sees
it, at `ci.yml:174`, on the slow three-OS job.

**Counterfactual, and it is the point.** Had the gate been changed at T12 — the obvious
moment, with two datapoints already in hand — the change would have been to add
`--all-targets` to a `check`. That is *precisely the remedy that cannot see this class of
failure*, and the class would have been marked closed with the runtime half untouched.
Promoting earlier would have been worse than not promoting, because it would have
manufactured a false all-clear over the half that was still open.

**What made the difference was a refusal, not an insight:** declining to promote until an
instance arrived that the *existing* remedy provably could not reach. A runtime failure is
that instance — `check --all-targets` stays green whichever way it is fixed, so no version
of the old remedy could ever have caught it.

**The fourth instance arrived inside the fix for the third.** The peer's first draft used
`expect_err`, which compiled clean by default and failed to compile **lean**, because
`Arc<dyn CodeEmbedder>` is not `Debug`. So the blind spot has a floor below the one being
closed: a lean-only *compile* failure introduced by the patch repairing a lean-only
*runtime* failure.

**Shipped `6764eb18`.** One substitution in CLAUDE.md's gate list —
`cargo check --no-default-features` → `cargo test --workspace --no-default-features`. A
replacement rather than an addition: the new command compiles every target under
no-default-features *and* runs them, so it strictly dominates, the list stays four
commands long, and the cost goes ~10s → ~20s (the test phase measured 6.96s on top).

**A second gap fell out of writing it**, which nobody had noticed in four instances: the
canonical list said `cargo check --no-default-features` with **no `--all-targets`**, so it
did not even compile the lean test targets. T12's remedy had reached the *practice* and
never the *list*. `cargo test` builds all targets by construction, so one substitution
closed both.

**Corroboration worth recording:** both peer sessions reached the same conclusion
independently and neither edited CLAUDE.md; one wrote *"put my half in your raise however
you like."* Three sessions converging on a documentation change none made unilaterally is
a stronger signal than any one session's argument.

**Promote-when:** already promoted (CLAUDE.md § Development Commands, `6764eb18`). Re-open
if a fifth instance appears that `cargo test --workspace --no-default-features` also cannot
see — that would mean the class is about *feature-combination* coverage generally, not the
lean lane, and the remedy would be a matrix rather than a command.

## W-78 — The control run separated the instrument from the change under test, and it was one command

**Observed:** 2026-08-30, verifying whether `sdd/operator-rules-phase-2` could be
merged into `experiments`.

**Practice:** before attributing a test failure to the change under test, re-run
the same failing tests **in the same environment at the unmodified base**.

**Counterfactual, and it was close.** The merge probe ran in a scratch worktree
under the session scratchpad — i.e. under `/tmp`, which is where this project's
own system prompt instructs agents to put temporary work. `cargo test --workspace`
returned **4706 passed, 3 failed**. All three were `librarian` temp-guard tests; the
branch under test touches `src/operator_rules/`, `src/tools/core/`,
`src/prompts/guide_index.rs` and docs, and does not go near `librarian`.

That mismatch is the only thing that prompted a control instead of a report. The
control — same `/tmp` worktree, `git checkout --detach experiments`, merge **not**
applied — reproduced all three identically. The same three pass in the main
checkout in 0.11s.

**Root cause of the failures, once separated:** each test builds its
"outside-temp" catalog with `tempfile::TempDir::new_in(std::env::current_dir())`.
From a `/tmp` checkout that catalog is *inside* temp, so `catalog_is_real` is
false, `should_refuse` correctly returns false, and `expect_err` panics with a
message blaming the guard. The assumption is stated in a code comment
(*"Assumes the repo checkout is not itself under the OS temp dir, which holds
here"*) and enforced by nothing. Filed as
`docs/issues/archive/2026-08-30-temp-guard-tests-fail-from-a-tmp-checkout.md`
(archived 2026-08-31; fixed at `3ec8e500`, patch-id
`a09e8ef8809f8ccf2a7d3b0d52f50dce2cf58ad4`).

**What the counterfactual costs.** Without the control, the two available readings
were both wrong and both expensive: report "this branch breaks three librarian
tests" and block a clean merge, or spend an afternoon bisecting a guard that was
working correctly throughout. The merge was in fact clean — 4908/0 full and
3381/0 lean after landing.

**Why it generalises past this instance.** The failing configuration is not exotic;
it is the *sanctioned* one. Verifying a branch in a scratch worktree without
disturbing a shared tree is the correct move, and this project points that
scratch space at `/tmp`. So the trap is reachable by doing the right thing, and
the cost of the control is one command against an already-warm target dir.

**Status:** validated

**Valid:** invariant

**Promote-when:** a second instance appears where an environment-sensitive failure
was about to be attributed to a change. At two datapoints, promote to CLAUDE.md's
gate discussion as a sentence: *a failure in a subsystem the change does not touch
is a prompt to re-run at the base, not a finding.*

**Rests on:** nothing in the code — it is a method, not a claim about a code path.
The specific instance rests on `should_refuse`
(`src/librarian/tools/temp_write_guard.rs:14-21`) classifying by path prefix, and
on the three tests deriving "outside-temp" from `current_dir()`.

## W-79 — Verify a merged branch's bug files at the door, not months later

**Observed:** 2026-08-30, merging `sdd/operator-rules-phase-2` (`03b3fb5c`).

**Practice:** when a branch merge imports bug files, run the verify-open pass on
them **as part of landing**, in the same session, before the merge commit's
neighbours scroll away.

**Why it earns its place.** That branch carried five bug files, all
`status: open`, authored 1-2 days earlier against a base 103 commits back. One was
already fixed: `2026-08-29-memory-tests-deadlock-on-futex-in-isolation`, severity
**high**, whose own reproduction —
`cargo test --lib tools::memory::tests:: -- --test-threads=1`, the invocation its
Summary calls a hang "in complete isolation" — now runs 77 passed, no hang.

Without the pass it would have entered `experiments` as an open high-severity bug
describing a hang that does not happen, and sat in every triage query until
somebody spent a session reproducing it.

**The cost asymmetry is the argument.** Checking five files against current code
took one session and produced four confirmations plus one flip. Finding the same
staleness later costs a full investigation per entry, and the investigator starts
by believing the file. Earlier the same day, a verify-open sweep of the standing
ledger found exactly this shape twice (BL-50 and BL-51) — both had been true when
written and neither had a mechanism that would ever notice they had stopped being
true.

**Four were correctly open, which is the other half of the result.** A door check
that only ever flips things is not a check. `atomic_write`'s leak is genuinely
half-fixed (cleanup on `rename`'s `inspect_err`, none on the earlier
`std::fs::write`); no `include_str`/`cargo package` gate exists anywhere in the
repo; and both operator-rules routing bugs rest on one verified fact —
`Tool::selector_key` defaults to `None` (`src/tools/core/types.rs:1251`) and only
`LibrarianAdapter` overrides it, while OP-2/3/4 serve `memory.write`, `edit_file`
and `create_file`.

**Mark attribution as inference when it is inference.** The flip named `fd638c76`
(ET-9 T10) as the closer — its subject is exactly the mechanism, and two of the
tests the file lists are among the five fixtures it isolated — but nobody
bisected, and 103 commits were in scope. The record says the non-reproduction is
measured and the cause is not. That mattered here specifically: the file already
carried one careful negative, explicitly ruling out the archived wrong-port bug,
and a confident wrong attribution would have undone that care.

**Status:** validated

**Valid:** invariant

**Promote-when:** a second branch merge imports bug files and the pass finds
staleness again. At two datapoints, add a line to `docs/RELEASE.md`'s ship
sequence: *a merge that imports `docs/issues/` files owes a verify-open pass on
them before the merge commit lands.*

**Rests on:** the archive-and-status discipline in
`get_guide("tracker-conventions")`, and on `unverified:` being queryable — which
is what made the standing-ledger sweep cheap enough to be worth doing at all.

## W-80 — A regression test built from the shared constant cannot catch a producer that stops rendering it

**Status:** validated
**Valid:** invariant

**Observed.** 2026-08-30, fixing the sparse-leg classifier bug (`5dfa5051`, bug file
`docs/issues/archive/2026-08-30-sparse-status-errors-never-match-their-classifier-arm.md`).

The defect being fixed was a **producer/consumer wording drift**: root's
`embed_one_batch` emitted `embed_batch sparse status …` while `classify_search_error`
matched `embed sparse`, which that string does not contain — `embed` is followed by
`_`, not a space. The remedy was a shared constant, `SPARSE_MARKER`, rendered by both
producers and matched by the consumer.

The first regression test I wrote for it looked right and was worthless:

```rust
let marker = crate::retrieval::embedder::SPARSE_MARKER;
for err in [format!("stack search failed: {marker} send: connection refused"), …] {
    assert!(classify_search_error(&err, "codescout").contains("embedder"));
}
```

**It builds its input from the constant.** So it asserts the *consumer* matches the
constant — which was never in doubt — and is structurally blind to the *producer*
walking away from it, which is the entire defect. A guard for this bug that this bug
would pass.

Caught by asking the mutation question before committing rather than after: *what
edit makes this fail?* Reverting the producer's wording — the exact regression —
moves it not at all, because the fixture never came from the producer.

**Counterfactual.** Shipped, it would have read as coverage in every later review; the
bug's own file would have cited it; and the next drift would have gone out green. The
test's *name* would still have been accurate, which is what makes this class hard to
spot on reading.

**The remedy is a shape, not more assertions.** Delete it and drive the real producer
into the real consumer:

```rust
let err = e.embed_batch(&["x".to_string()]).await.expect_err(…).to_string();
let hint = classify_search_error(&err, "codescout");
```

Reverting the producer's wording now kills it, printing the original bug verbatim
(`hint: Stack reachable but query failed … qdrant logs`) — **while both
constant-built tests stay green**, which is the demonstration rather than the claim.

**The generalisation.** Wherever a fix is *"introduce a shared constant so two sides
cannot drift"*, a test that constructs its fixture from that constant tests one side
twice. The drift being prevented is between a **producer's output** and a
**consumer's input**, so at least one test has to obtain its input the way production
does. Same shape as `ET-5`'s cross-crate argument one level down: there the two sides
were in different crates and the remedy was to make them share a symbol; here they
share the symbol already and the remedy is to make one test refuse to use it.

**Promote-when:** a second instance in a different subsystem — at which point it
belongs beside *demand a deliberate break* in CLAUDE.md, as its corollary: demanding a
break tells you to mutate; this tells you **which** mutation is the honest one, since
mutating the constant leaves both sides consistent and proves nothing.

**Rests on:** `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` — a
green test built from the value under test is the same failure as a plausible number
standing in for a measured one.

## W-81 — Choose a gate's surface by measuring its feedback latency, not by what kind of check it is

**Observed:** 2026-08-30, building the packaging gate for the `include_str!`/`exclude`
class (`188cf9f0`, patch-id `c7604ff8088c8f32`).

**Valid:** invariant

**The counterfactual.** `F-14` proposed the remedy in writing and it was never built: *"add
a CI step that runs `cargo publish --dry-run`, or at minimum `cargo package --list` with an
assertion."* Its own prediction — *"future `include_str!(\"../docs/...\")` additions will
silently regress this until a CI gate or pre-commit hook covers it"* — then held on the very
**next** escaping site added to the codebase, `src/operator_rules/corpus.rs:16`, caught only
by a reviewer running `cargo package --list` by hand.

So the remedy was correct and unbuilt for months, and the obvious reading is inertia. The
useful reading is that **"CI gate" was the wrong half of the sentence to act on.**

**What decided it, and it was a number rather than a preference.** Two facts measured before
writing anything:

- `cargo package --list` **does not build**: 0.21s per package. The cost objection to
  putting it in `cargo test` did not survive contact with a stopwatch.
- This repo was **119 commits ahead of origin, across two days**, at the moment the gate was
  written. A CI-only gate would not have run once in that window.

The second is the one worth generalising. A gate's value is not (does it catch the defect)
but (**how long until it tells someone**), and that latency is a property of the workflow,
not of the check. In a repo that pushes hourly, CI is fine. In this one — where `experiments`
accumulates for days before a ship — CI is a *weekly* signal wearing a per-commit costume.

**The rule.** Before choosing where a check lives, measure the actual interval at which that
surface runs *in this repo, this month*. `git rev-list --count origin/<branch>..HEAD` is the
whole measurement for the CI case. Then put the check on the fastest surface that can host
it, and let the slower surfaces inherit it for free — `cargo test` is run by both the four
documented gate commands and CI, so choosing the test surface loses nothing and gains two
days.

**Second decision, same shape, recorded because the tempting alternative was worse.** The
gate needed to know whether a path survives `exclude`'s gitignore-style negation. Reimplementing
that matching in the test would have been a **second implementation of one operation** — the
exact defect class deleted from `reindex_cli` in `9f743091` two hours earlier — and it would
have agreed with itself while disagreeing with the tool that actually builds the tarball.
Shelling out to `cargo package --list` asks cargo what cargo will do. *When a check needs to
predict a tool's behaviour, call the tool.*

**Status:** validated

**Promote-when:** a second gate whose surface choice is decided by measuring push cadence.
Then CLAUDE.md's gate section gains a sentence — it currently justifies each of the four
commands by what it catches, and says nothing about how fast any of them says so.

## W-82 — A positive control turned a correct conclusion supported by a meaningless number into a real one

**Observed:** 2026-08-30, verifying that the BL-64 fix was absent from the release binary.

**Valid:** invariant

**The near-miss, and its shape is why it is a win rather than a note.** `reindex_cli` is
`#[cfg(test)]`, so its fix cannot be in the release binary. To verify rather than assert it,
I ran `nm -C target/release/codescout | grep -c reindex_cli` → **0**. The answer I expected,
arrived at by a check, matching a compile-time guarantee. Every reason to publish it.

It was meaningless. The binary is **stripped**: `nm` reported **zero symbols in total**. The
`0` described my instrument's failure, not the binary's contents.

**What caught it was running a positive control in the same breath** — `grep -c` for
`export_augmentation`, a symbol I knew was compiled in. It also returned 0, and two zeros
where one should have been non-zero is what exposed the instrument. Switching to `strings`
gave a discriminating run: controls **2** and **6**, targets **0** and **0**.

**Why this is the valuable direction of the class.** The project's *"a plausible value is
not a verification"* ADR was at eleven instances, and they are overwhelmingly **post-hoc**
— an error found after it shipped into a claim, a commit, or a peer's context. This one was
caught **before publication, by the practice, at no extra cost**: the control was one extra
line typed at the same time as the check. A class with eleven diagnoses and no worked
prevention reads as an inevitability; one prevented instance makes it a technique.

**The generalisation, and the part I had to be honest about.** A negative result needs a
positive control **whenever the instrument could fail silently**, which is nearly always for
anything shelling out. But note what did *not* save me: the conclusion was **correct** —
`reindex_cli` genuinely is not in that binary. Being right is not evidence the method
worked, and had I stopped at `nm` I would have published a true statement on a false basis
and never learned otherwise. That is the same asymmetry `R-130` records one level down
(a test that dies for the wrong reason still reports `KILLED`) and `R-129` records for
conclusions (*self-catching works on your method and fails on your conclusion*) — **here the
method failed and the conclusion survived, which is the configuration in which nothing ever
teaches you.**

**Status:** validated

**Promote-when:** already promoted in practice — the pattern is now written into
`scripts/probe_augmentation_restore.py`'s blind-spot list and `tests/packaged_includes.rs`'s
control test, both of which ship a "the scan actually found something" assertion beside the
invariant. Cite this entry when someone proposes deleting one as ceremony.

## W-83 — Measure the objection that got a task deferred, instead of accepting it or overriding it

**Status:** validated — the measurement changed the answer, and then changed the design three more times.
**Valid:** invariant
**Rests on:** `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`; `BL-44`; `docs/issues/archive/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md`.

**Observed.** `BL-44` had been dropped twice, both times on a stated technical objection
rather than on priority. `scan_params_behind_body`'s own doc comment carried it:

> *"Ids only, never statuses. A status mismatch between a params row and a rendered table
> cell needs a text comparison against a column whose format is each tracker's own choice
> — fragile, and a separate decision."*

That is a good objection, written by someone who had the code open. The two available
moves both look reasonable and are both wrong: **accept it** and the task stays dropped
forever, or **override it** and you ship the fragile thing they warned about.

**The third move is to measure the objection.** A dry run over the real corpus, before any
Rust:

| what the measurement showed | consequence |
|---|---|
| 26 findings on a naive comparison, **20 of them `**done, archived**` vs `done-archived`** | the "each tracker's own format" fragility is **one convention**, and absorbing it leaves 6 findings on 101 rows |
| a closed `status` enum in `params_schema` exists on 4 ledgers | no column ever has to be identified — any enum token in the region is evidence |
| 91.4% sensitivity over 536 simulated drifts | the check detects things, which six findings alone could never have shown |

The objection was **accurate and not decisive**. Fragility concentrated in a single
comma-vs-hyphen rendering is a different fact from fragility spread across nine bespoke
formats, and nothing but measurement separates them.

**Then the dry run kept paying, which is the part that argues for doing it before the
code rather than after.** It reshaped the design three more times:

- Coverage was **4 of 9 ledgers, not 6**. My first count read `params_schema` and assumed
  the rendering. Two ledgers render entries as a heading plus a `**Status:**` line, and a
  table-row-only locator — which the first draft was — skipped **30 of 131** exposed
  entries while reporting nothing amiss about them.
- The obvious fix for the comma case (strip punctuation, then compare) makes `done` a
  substring of `abandoned` and `open` of `re-opened`, both present in this repo's real
  status prose. The predicate became a boundary-anchored regex instead.
- Restricting the heading locator to the `Status:` line, not the section, came from seeing
  what section prose actually contains: *"was dropped as a design decision"*, *"local
  export DONE"*.

**Counterfactual.** Writing the check straight from the plan would have shipped something
that reported 26 findings of which 20 were noise, covered half the exposed entries while
looking complete, and matched status words inside longer words. It would have been turned
off within a week and `BL-44` would have been dropped a third time — with better evidence.

**And the measurement was itself wrong twice, which is why the instrument needs the same
scrutiny as the code.** The Python probe that specified the design reported `BL-46`'s row
as stating `done`; the shipped Rust says *"no enum value"*, correctly, because the probe
used the substring matching the Rust rejects. The implementation ended up **strictly
better than the instrument that specified it** — and that only surfaced by running both on
the same real data and diffing.

**Promote-when:** a second task is un-deferred this way. The candidate shape for CLAUDE.md
is a sentence beside *run the reproduction before reading the fix plan* — that rule says a
plan is a hypothesis about a reproduction; this one says **a deferral is a hypothesis about
a cost**, and both are checked the same way.

## W-84 — Mutate once per call site — then check the kill came from a positive test

**Valid:** dated 2026-08-30

**Observed:** A peer finished mutation-testing a feature and reported a law from it: their two mutations killed *different* tests, neither covering the other's site. Two shape-writing call paths existed (`merge=false`, and `merge=true` with a sibling field) and they had assumed one test covered both. Generalised — **where one rule is implemented at more than one call site, a single kill proves only that one site is guarded.**

**Got:** Applied it to `entry_status_region` (`src/librarian/tools/doctor.rs`, shipped this morning in `92228de0`), which implements one rule — *find where this entry states its status in the body* — at **two** locator sites: a table-row form and a heading + `Status:` line form. Baseline green (94/0), then one mutation per site:

| mutation | site disabled | test killed |
|---|---|---|
| A | table-row locator | `status_drift_fires_when_params_and_the_body_row_disagree` |
| B | heading locator | `status_drift_finds_the_heading_form_that_renders_no_table_row` |

Distinct kills, no overlap, each `left: 0, right: 1` — the scan going silent, which is what removing a locator does. **Both sites were already independently guarded; no defect found.** The law was worth running anyway: I had mutation-tested this same function twice earlier today along the *predicate* axis (`status_token_present`) and been satisfied, which is the part that generalises — a suite can be honestly, carefully mutation-tested along an axis that is not the one with two call sites on it.

**The finding is in the survivors, not the kills.** Two tests passed under removal of the exact locator each was written to exercise:

- `status_drift_is_silent_when_the_body_punctuates_the_status_differently` (table form)
- `status_drift_reads_the_status_line_and_not_the_sections_prose` (heading form)

Both are `assert!(scan_params_status_drift(&cat.conn).unwrap().is_empty())`. A removed locator returns `None`, the scan reports nothing, `is_empty()` holds. They pass for precisely the reason they exist to rule out.

**This is a different class from a lax assertion, and the difference is what makes it dangerous.** The peer's own case that day was `assert!(met > 0)` admitting a value the bug produced — too weak, curable by tightening to equality. This one cannot be tightened: `is_empty()` is *already* the strongest assertion available. **The expected value of an absence test is identical to the output of the failure mode.** So a silence test is never evidence on its own; it is evidence only when **paired** with a positive test on the same mechanism, and the pairing — not the assertion — carries the guarantee.

**Counterfactual:** my suite has that pairing on both sites *by luck, not design*. I wrote a positive and a negative case per rendering because both seemed worth stating, not as two pairs. Had I written only the silence half for one locator — an entirely natural thing to do for the heading form, whose whole point is *not* matching the surrounding prose — that site would have been completely unguarded behind four green tests, with a mutation-testing story I would have believed and reported.

**Composite rule (needs both halves):** mutate once per call site, **and** check that each site's kill comes from a *positive* test. A site whose only tests assert absence has no kill available at all — it will look guarded and report nothing.

**Status:** validated
**Promote-when:** one further instance of a silence-asserting test surviving removal of its own mechanism → promote to memory `test-design-discipline` alongside the existing vacuous-assertion guidance, since that memory currently frames vacuity as *laxness* and this class is not lax.


### Corrected the same day — the entry above overstates, and measuring the overstatement found a real hole

**What was wrong.** *"A silence test is evidence only when PAIRED with a positive test on the same mechanism, and the pairing carries the guarantee"* is too strong. Pairing establishes only that the mechanism is **alive**. It says nothing about whether the absence test can detect a **violation** — and neither of the two mutations above can measure that, because removing a mechanism produces exactly the outcome an absence test asserts. I wrote it as a finished claim, in this tracker, having measured nothing about it, roughly four hours after helping build the discipline that catches precisely this.

**The third mutation, and what it found.** For an absence test the informative mutation is not removal but the **forbidden act** — make the mechanism do the thing the test forbids. Applied here: the heading locator returns the *whole section* as the status region, which its own doc comment argues at length it must never do. Result: **all six `status_drift` tests passed.**

The silence test surviving was predicted. **The positive heading test surviving was not**, and that is the larger finding — it is not an absence test at all. So the heading locator's *precision* was covered **zero** times, not weakly, behind eight green tests and a doc comment justifying the property from real repo data.

**The general form** (peer's, and it subsumes mine — cite rather than restate):

> **A test cannot detect a change its assertion is monotone under.**
> Absence assertions are monotone under **removal**. Existence assertions are monotone under **widening**. Both look like guards; neither can fire in its own direction.
> Equality against an exact expected shape is monotone under nothing — widen, narrow, reorder or drop and it fails. That, not an argument, is what saved the peer's own tests: `met == 2` and `requests == [32, 32, 6]`. Had either been `met > 0`, it would have been monotone in exactly this direction and survived.

**The criterion this replaces "did it die?" with:** *did it die **on the assertion under test**?* A removal mutation and a forbidden-act mutation can both turn an absence test red, and only the second is evidence. The peer's case makes it concrete where mine cannot: removing their short-circuit kills the test via a panic at `.expect("embed_one_batch")`, which says nothing about whether `.expect(0)` has any power. **A silence test killed by a removal mutation is exactly as unverified as one never mutated — and now carries a green mutation story, which is worse than carrying none.**

**Corollary — the discriminating fixture.** The property is observable only where the widened region and the correct region **disagree**, and constructing that disagreement *is* the test. No strictness of assertion substitutes for it. Here: `params` says `open`, the `Status:` line says `done`, and the prose below happens to contain `open`.

**Fix shipped** — `status_drift_does_not_let_prose_discharge_a_real_disagreement`, verified in both directions: passes clean, and under the forbidden-act mutation it is the **only** one of seven that fails, on its own assertion, with the message naming the act. Gate green (fmt · clippy · 4878/0 · lean 3385/0).

**Scope of the defect — narrower than it sounds.** The failure mode is a false **negative**: a widened region makes the `params` status *more* likely to be found, so the scan under-reports. Everything the shipped scan **did** report stands, including the 6-of-66 drift a peer's independent run surfaced in `open-issue-work-queue.md`. The hole was in what it stayed silent about.

**Left visible rather than edited away**, on the peer's argument that the narrowing is sharper *because* it arrives against something stronger already written down.
## F-81 — An announcement sent to one peer is not an announcement

**Valid:** dated 2026-08-30

**Observed:** Four Claude sessions share this checkout. Peers announce "mutation windows" — deliberate breaks to confirm a test dies rather than merely passes — so others ignore the resulting red. I opened one in `src/librarian/tools/doctor.rs` and announced it **to a single peer**, because my window collided with theirs and I wrote the announcement as a *reply* to their message. Reply-shaped thinking silently scoped it to one recipient.

**Got:** A second peer ran a full `cargo test` straight into my mutation, saw `status_drift_fires_when_params_and_the_body_row_disagree` fail with `left: 0, right: 1`, then saw it pass in isolation and on re-run (because I had restored the file in between). They reported it to me as a **load-dependent flake** in a file they had never touched, and committed that characterisation in a commit message before I corrected it. It was my deliberate break, restored between their runs. A partial announcement is **worse than silence** — silence leaves a peer with an unexplained result, while a partial announcement manufactures a false record that coordination happened.

**The symmetry is the transferable part.** Two incidents the same afternoon, one root cause, opposite invented entities:

| incident | real cause | entity invented to explain it |
|---|---|---|
| 16:56 — unannounced mutation | a real write | a phantom **session** (an actor) |
| 17:15 — mutation announced to a subset | a real write | a phantom **flake** (a defect) |

An intermittent-looking result on a file nobody admits touching gets explained by whichever kind of ghost the observer already believes in. Neither of us reached for *"someone is mid-experiment and did not tell me"*, which was the boring truth both times.

**Mitigations, in the order they were found:**

1. **Say what you are about to break, not how to break it.** A precise announcement is a heads-up in intent and an *imperative in its grammar* — a reader arriving without context cannot tell which. Naming the behaviour and the tests that should die carries identical warning value with nothing to execute.
2. **Broadcast to every peer, not to the one you are mid-conversation with.** This entry.
3. **Say that your broadcast is a subset** (peer's rider, adopted). `ListAgents` under-reports: my own view read 2 peers and then 3 today, and a fourth session answered a peer and became unreachable within the hour. "Every peer I can see" is *already* incomplete, so an honest announcement states that rather than implying a completeness it cannot have.

**Operational change the peer took, worth keeping:** in a shared checkout, an unexplained intermittent result on a file you have not touched belongs at the **top** of the hypothesis list as an uncoordinated concurrent edit — above race, above cache, above flake. Not because it is likeliest in general, but because it is the only hypothesis that is both common here and invisible to every single-observer instrument.

**Status:** open
**Promote-when:** a third instance of a mutation window being misread by a peer → the announcement protocol is currently convention held in session memory only, and should become a documented surface (skill or `docs/conventions/`) rather than something each session re-derives.


### Withdrawn in part, same day — the carrier was wrong, and mitigation 2 is impossible

The **diagnosis** in this entry stands: a partial announcement is worse than silence, and it did
manufacture a phantom flake in a peer's tracker. What does not stand is the finding I attached
to it — that *a precise mutation announcement contains a verbatim executable instruction*, offered
as the mechanism behind the 16:56 phantom mutation.

**Refuted 2026-08-30.** The session that applied that mutation (`807989`) **never received the
announcement** — it is one of the two sessions invisible to `ListAgents` (`BL-58`). Nobody executed
a broadcast. The actual carrier was a **doc comment** at `src/retrieval/embedder.rs:1975` naming
the exact edit and function, written ~16:50 and acted on ~16:55 by a reader of the test. Verified
at the bytes rather than accepted: the comment says what it is reported to say, and
`git log --all -S` on the mutant's distinctive form returns 0 across every ref with a control
returning 3, so it came from no checkout.

The reasoning was sound and the mitigation costs nothing, so **say what you break, not how** stays
as practice — it simply cannot be cited as measured. Kept visible rather than deleted, on the same
grounds as `W-84`'s overstatement: a claim that arrives as a narrowing teaches more than one that
arrives clean.

**Mitigation 2 of this entry — "broadcast to every peer" — is now known to be unachievable**, not
merely hard. Two of five sessions in this checkout could not receive any message I sent, and one
had been running since before the day's first inter-session message. Mitigation 3 (*say that your
broadcast is a subset*) is correct and insufficient: an honest subset is still a subset. The real
remedy is not in the message layer at all — see `W-85`'s placement-cost section: the coordination
hint has to ride the artifact, because the artifact reaches the population the registry cannot.
## W-85 — Run a suspicious test alone — a green suite cannot distinguish a working guard from a disarmed one

**Valid:** dated 2026-08-30

**Observed:** Closing BL-66 (`probe_ollama` installed no rustls crypto provider; every external consumer of `codescout-embed` aborted at zero configuration), I found that `remote::tests::probe_ollama_errors_when_unreachable` **already existed** and was nearly identical to the regression test I had just written — same function, same `http://127.0.0.1:1`, same assertion. The path was already covered. Pre-fix it should have panicked, and the suite was green.

**Got:** Re-removed the install and ran the **same** pre-fix code two ways:

| run | result |
|---|---|
| `--lib remote::tests::probe_ollama_errors_when_unreachable -- --exact` | **panics**, `No provider set` |
| `--lib` (full suite, 51 tests) | **passes**, 51/0 green |

`CryptoProvider::install_default` is **process-global**, and `remote::tests` is full of siblings that construct a `RemoteEmbedder` — `from_url_*`, `custom_*`, `openai_*`, the retry tests — every one routing through `build_client`, whose first line installs it. Whichever sibling won the race installed the provider for everybody. The guard existed, ran on every invocation, reported green, and was **structurally incapable of failing**.

**A third distinct way a check can be worthless**, alongside the two recorded the same day:

| failure | what is wrong |
|---|---|
| `W-84` — monotone assertion | the assertion cannot move in the direction of the defect |
| `R-133` (peer) — unreachable path | no traversed path observes the failure |
| **this** | the assertion is fine, the path *is* traversed — **the test's own siblings repair the defect before it can be observed** |

The third is the nastiest, because both earlier remedies pass it. The assertion is a specific `Err` with a message check, not a silence test. The path runs on every suite invocation. Nothing about the test, read on its own, is wrong.

**Counterfactual:** without the isolation run I had two wrong conclusions queued and would have shipped either. First, I was about to write that BL-66 survived because root's `main.rs` shields codescout's binary — true, and not the reason the *crate's own* test missed it. Second, my new test's doc comment warns at length that a sibling constructing a `RemoteEmbedder` would disarm it; I wrote that as a **hypothesis about a hazard**, and it is in fact a **demonstrated description of the test one file over**. The warning was already history, not foresight.

**The cheap general move:** when a test covers a path guarded by process-global state — a `OnceLock`, a `set_default`, an env var, a static registry, an installed provider — run it with `--exact` before believing it. One command, no code change. A suite-green result is silent about which of the two states you are in, and the two are indistinguishable from every other vantage point.

**Status:** validated
**Promote-when:** a second instance of process-global contamination disarming a test → `docs/conventions/test-env-isolation.md`, which already covers env isolation but not the process-global-singleton case this one turns on.

**Promote it as a PROCEDURE, not as a fact — corrected same-day.** The original criterion said "promote to memory `test-design-discipline`", and a peer's closing observation argues that is the form which will not fire: *knowing a class does not activate at the moment of use; what activates is a mechanical tell external to the reasoning.* Their evidence was against themselves — two scope errors committed **after** writing the entry warning about that exact class, each caught by something outside the thinking (`exit_code` surviving `2>/dev/null`; a peer re-running the number).

It indicts this ledger's own shape. Checked across the day's wins, all three are **procedures**:

| entry | the thing that actually fires |
|---|---|
| `W-82` | run a positive control in the same breath |
| `W-84` | mutate once per call site |
| `W-85` | run it with `--exact` |

None is a fact to recall; each is a command to execute. A memory is read at session start and is silent at the moment of use, which is the only moment that matters here — so filing these as things to *know* predicts they will be known and not done. They belong where they execute: a gate step, a checklist line, a doc-comment on the test itself. This entry's fix already took that form without naming it — the added regression test carries its own warning in its header, at the place someone would have to edit to break it.

Worth pairing with `F-81`'s shape: both are cases where the remedy is structural placement rather than better knowledge.


### The cost of placement, measured the same day — what fires for everyone, fires concurrently

The correction above says a procedure belongs **where it executes** — a gate step, a checklist
line, a doc-comment on the test itself — and cites this entry's own fix as having taken that
shape. That is right, and here is its price, established by an incident that also refuted a
separate finding of mine.

`src/retrieval/embedder.rs:1975` carries exactly such a placement:

> *"Acceptance is a mutation: replace the `try_join!` in `embed_one_batch` with two sequential
> `.await`s and this test must fail deterministically."*

At ~16:50 a peer wrote it. At ~16:55 a **different** session read the test and ran the
mutation it prescribes, on the lines it names. The two collided in one working tree, and the
transient edit was read — by me among others — as a phantom, spawning an authorship hunt, a
tool-layer cache hypothesis, and a protocol finding of mine that is now withdrawn (see `F-81`).

**The property that makes placement work is the property that makes it collide.** A doc comment
reaches whoever touches the code — which is precisely the population that would perform the
mutation, and far better targeting than any message. It reaches them *whenever* they touch it,
including simultaneously. There is no version of "fires at the moment of use" that fires for
only one reader.

**The announcement protocol cannot patch this, structurally.** The colliding session never
received the broadcast — it sits outside the `ListAgents` registry that under-reports by 40%
(`BL-58`: 3 reported, 5 real). The sessions who most need an announcement are exactly the ones
that cannot receive one.

**So the collision guard has to ride the same carrier as the instruction.** If a doc comment is
strong enough to make two sessions run the same mutation five minutes apart, it is strong enough
to carry "this tree is shared — confirm `git status` is clean before applying, and revert
promptly." That reaches the population at risk by construction, which no message can. Offered to
the file's owner rather than applied: it is their file, and the comment itself is **not** the
defect — removing it would delete the acceptance criterion that makes the test verifiable.

**Status:** validated — the cost is measured, the remedy is proposed and unshipped.
**Promote-when:** the guard line is added and a second concurrent collision is either prevented
or observed anyway → that outcome decides whether artifact-carried coordination works at all, or
whether shared checkouts need something the artifact layer cannot provide.
## F-82 — Advocating a change to a shared contract on a claim I had not measured — in an argument about measurement

**Valid:** dated 2026-08-30

**Observed:** A peer proposed appending `cargo build --bin codescout` to the gate, to repair the librarian-less binary the lean lane leaves behind. I countered with a reorder — lean third, default-features fourth — and recommended it to my operator as **"zero added time, strictly better"**, on a symmetry argument: both orders run the same two invocations over the same two feature sets, cargo's cache is keyed on feature set rather than order, and clippy precedes both with a third set so neither inherits a warm cache.

I still believe the argument. I had not measured the direction it was about. The peer had timed the *reordered* sequence; nobody had timed the *documented* one, so the comparison had one side — and I was proposing to change `CLAUDE.md`'s gate, a contract every session pays on every task, on the missing half.

**Got:** measured it, and the claim died.

| order | steps 3+4 | binary after |
|---|---|---|
| documented (default 3rd, lean 4th) | 47.0 + 24.7 = **71.8s** | **clobbered**, `artifact --help` exits **2** |
| reordered (lean 3rd, default 4th) | 26.9 + 53.5 = **80.4s** | correct |
| documented + appended build | 71.8 + 7.8 = **79.6s** | correct |

The reorder is ~8.6s **slower**, and the two fixes land within a second of each other. Not "free", and not "strictly better" on the axis I claimed it on.

**What I did NOT then claim:** that the reorder is slower. Two single runs at different times on a machine with four sessions building concurrently is sampling *and* temporal adjacency together (`reconnaissance-patterns:R-136`), and 8.6s sits inside what concurrent builds produce here. The defensible statement is that the two fixes cost about the same.

**The proposal survived anyway, on better ground.** Four commands rather than five; it *prevents* rather than repairs, so the terminal state is correct by construction instead of by a step a hurried session can omit — silently, since the omitter's own gate still passes; and it needs no positive-verification line at all, because it **removes** the proposition whose exit code could be misread rather than documenting the trap. That last argument only became visible once the speed claim died, which is the argument for killing weak claims even when the conclusion holds.

**The part worth keeping.** The subject of the argument was measurement discipline. That did not inoculate the argument. I had spent the day writing `W-82` (run a positive control), `W-84` (mutate per call site), `W-85` (isolate before believing a green suite), and correcting two of my own overstatements — and then advanced an unmeasured number in support of changing the gate. Knowing the class does not fire at the moment of use; that is `W-85`'s own corrected promote-when, and this is it happening to its author within the hour.

**And it was not my check that caught it.** A peer flagged that *their* symmetry reasoning was unmeasured, in their own file, about their own claim. I recognised my identical claim in their disclosure. The tell was external, again — `exit_code` surviving `2>/dev/null`, a peer re-running a number, and now a peer's honesty marker about a different instance of the same argument.

**Second instance today of the same shape.** `F-81`'s withdrawn finding was also reasoned, never measured, and also concerned coordination. Both were plausible, both were mine, and both were caught by someone else's disclosure rather than by my own review.

**Status:** open
**Promote-when:** a third instance of advancing an unmeasured number in an argument *about* measurement → the remedy is not more knowledge, it is a mechanical one: any number offered in support of changing a shared contract must name the command that produced it, at the point it is offered.

## W-86 — Reviewing a reorder — grep the file for prose describing the old arrangement, because the diff cannot show it

**Valid:** dated 2026-08-30

**Observed:** A peer reordered `CLAUDE.md`'s gate (lean lane third, default-features last) on my proposal, with my measurements, and wrote it well. I had stood down from the file to avoid a collision, so all I could do was review.

**Got:** two sentences that the reorder had made **false**, both written by me an hour earlier in `4c88e129`:

| stale text | why the reorder falsified it |
|---|---|
| *"The **third command** carries `--workspace`…"* | after the swap the third command is the **lean** lane; `cargo test --workspace` is now fourth |
| *"…the gate is still four commands and still **~26s**"* | that `~26s` was one lane alone, and now sat beside "71.8s vs 80.4s", so a reader met two gate timings differing 3× |

**Neither was visible in the diff.** The reorder changed one line; both stale sentences live inside *unchanged* prose in that same paragraph, so they render as **context**, not as change. A reviewer reading the diff — the normal, diligent thing to do — sees `-` and `+` on the command list and nothing at all on the sentences the change invalidates.

I found them by **grepping the file for my own earlier wording** after standing down, which I only did because I could not edit and had nothing else to contribute.

**The general hazard:** *a diff shows what moved and hides every sentence that described the old arrangement — and those are exactly the sentences the move falsifies.* Diff review is **structurally** blind to this class, not merely likely to miss it. It generalises past ordering to any change of arrangement: renaming a symbol, resequencing steps, moving a section, changing a default. The prose describing the old shape sits in untouched lines by construction.

**The remedy is mechanical, which is what makes it usable** (`W-85`: a procedure, not a fact). Before approving a change of arrangement, grep the file for prose that *names positions* — ordinals (`third`, `last`, `above`, `below`), counts, and any number attached to one element of the set. Do it on the **file**, never on the diff.

**And the root cause is a rule this repo already has, in a new place.** *"The third command"* is a **positional** reference: correct for exactly one arrangement, silently wrong after any reorder. That is precisely the defect a bare SHA has, which is why the project pairs every fix SHA with a `patch-id` — a content hash that survives rebase. *"Both test lanes"* and *"the default-features lane"* are content-addressed: they name what the thing **is** rather than where it sits, so a reorder cannot falsify them.

A paragraph that tells its reader *the order is load-bearing* must not then refer to its commands by number. The peer put that in the shipped text as a rule rather than as a fix to two sentences, which is the right grain.

**Counterfactual:** both would have shipped. The change was correct, well-argued, gate-verified and committed by a careful session; the defects were in the half no reviewer looks at, in a file every session reads as authoritative, and one of them contradicted a measurement in its own paragraph.

**Status:** validated
**Promote-when:** a second instance of arrangement-change stranding prose → promote to `docs/RELEASE.md`'s review guidance, next to the mutation-testing note, since both are "what a green diff review does not tell you".

## W-87 — Verify-open sweep found 0 stale of 15 — and the `unverified:` field is why

**Valid:** dated 2026-08-31

**Status:** validated — second consecutive sweep where the caveat field predicted the result

**Observed:** 2026-08-31, after an evening in which five sessions filed ~9 bug files and
committed heavily without reconciling the ledger. Exactly the conditions the verify-open
cadence exists for, and the cadence's own cautionary datapoint is a **75%** zombie-open rate
in one tracker.

**Result: 0 stale of 15.** No entry needed a status change.

### The discriminator that made it cheap

Rather than deep-diving 15 entries, partition them by whether they carry an `unverified:`
frontmatter field — i.e. whether anyone has already reconciled the record against reality:

| | count | disposition |
|---|---|---|
| carries `unverified:` | **8** | already reconciled, caveat stated, correctly open |
| no `unverified:` | **7** | never revisited — the staleness candidates |

Then verify only the seven. Five were checked directly this pass; two are `zombie` entries,
which the conventions define as recurrence checks rather than tasks, and nothing was observed
this session for either.

### The five, each by direct test rather than by reading the record

| bug | test run | verdict |
|---|---|---|
| `tree(glob)` omits dotfiles | `tree(glob=".gitignore")` → **0 files**, for a file that exists and is tracked | **reproduced live** |
| temp-guard tests fail from `/tmp` | `TempDir::new_in(current_dir())` still at `temp_write_guard.rs:141`, no guard added | still open |
| CLI `doctor` has no `--fix` | `codescout doctor --help` → `--project`, `--json`, `--no-color`, `--fail-on-violations`. No `--fix` | still open |
| buddy banner names a peer's sid | this session's own banner cited `428b66b8`, found in `~/.claude` and modified 20 s earlier — a **live cross-profile** session | reproduced, **mechanism upgraded** |
| `atomic_write` leaks `.tmp` | `src/util/fs.rs:62` — the *rename* failure path now cleans up; the *initial write* `?` still returns early | still open, **partial fix present** |

### Two findings the sweep produced that a status check would not

**A partial fix is what makes a record look addressed.** `atomic_write`'s rename path is now
guarded with `inspect_err`, so a reader skimming for "is there cleanup?" finds cleanup. The
bug is specifically about the *initial write*, which is three lines above and unguarded.
**Reading for the presence of a remedy is not the same as reading for the absence of the
defect** — and only the second answers the question.

**Reading the code turned up an unfiled sibling.** `let tmp = path.with_extension("tmp")`
*replaces* the extension rather than appending, so `foo.md` and `foo.rs` in one directory both
resolve to `foo.tmp`. Concurrent `atomic_write`s to same-stem different-extension files
collide on one temp path. Latent and unfiled; noted here rather than in the bug, since it is a
different defect.

### Why 0-of-15 is a result rather than a null

The cadence was promoted on a 75%-stale observation, so a clean sweep is worth explaining
rather than filing away. **The `unverified:` field is doing the work.** Eight of fifteen
records carry a precise statement of what their status overstates — one names the exact file
and line in another repo that still emits a false sentence; another names which of two fix
options shipped and which was deliberately deferred. Those cannot go stale unnoticed, because
the caveat is queryable and the next reader inherits it.

Two consecutive sweeps now (2 stale of 12, then 0 of 15) where staleness tracked the *absence*
of that field. The prediction to test next time: **staleness is concentrated in records with
no `unverified:` line**, and a sweep can be scoped to those at little loss.

**Promote-when:** a third sweep confirms staleness concentrates in the no-caveat set — at
which point the cadence should say to partition first and verify the uncaveated remainder,
rather than implying every open entry needs re-checking.

## W-88 — reproduction-before-plan inverted the fix from a denylist to an invariant

**Valid:** dated 2026-08-31

**Class:** win — CLAUDE.md § *Bug Tracking*, "run the reproduction before reading the fix
plan". Fifth datapoint for that rule; first where the plan's own *recommended* option
survived but its stated **mechanism** did not.

**Observed:** `docs/issues/archive/2026-08-31-memory-anchors-include-gitignored-runtime-artifacts.md`
led with a lock file and a database "rewritten on every index build" — a churn story. Its
`## Fix options` offered (2) "exclude by kind — lock files, `*.db`, `*.sqlite`, anything under
an `embeddings/`-style cache dir", correctly self-labelled a denylist.

**Got:** reproducing first showed the churn half is **inert on this machine**.
`.codescout/write.lock` last changed 2026-04-17 and `.codescout/embeddings/project.db`
2026-05-13 — the retrieval backend had moved to remote Qdrant, so the local db is dead
weight. What actually fired was the file's *second-order* effect: all five sidecars recorded
one identical `.codescout/project.toml` hash (`34e422b9…`) matching no file present
(`1c616aaf…`), with an mtime three days **before** the re-anchor that supposedly refreshed
it. That is not churn but a cross-machine oscillation with no fixed point — a tracked sidecar
hashing a gitignored file, so A refreshes, commits A's hash, B is permanently stale, and B
refreshing flips A. The repair action creates the next defect.

**Counterfactual, and why it is not merely "a bit worse":** option (2) is a *churn* detector.
The worst instance is a small, stable, hand-edited TOML — no extension, no cache directory,
never rewritten. So a denylist would have excluded `write.lock` and `project.db`, the two
already inert, and left `project.toml` — the one anchor firing in all five memories —
anchored. It would have passed its own tests, shipped, and moved the count from 12 bad
anchors to 7 while resolving **zero** of the eight false staleness reports. Reading the plan
first makes (2) look like the narrow, prudent option; the reproduction is what shows it
misses the entire live population.

**Second, independent win in the same pass — the three-site read.** `refresh_hashes` never
re-seeds, and `memory(action="refresh_anchors")` calls it. Every affected memory already had
a sidecar, so a fix at `seed_anchors` alone — the site the plan's phrase
"anchor-selection time" most naturally names — would have compiled, passed a green gate, and
changed nothing observable for all five. Confirmed by mutation rather than argued: disabling
each site's filter kills that site's test **and no other**.

**Also worth keeping:** the file's `## Resume` had independently warned that "the memory is
fresh" is monotone under an empty anchor list and prescribed asserting on the anchor list's
contents. That held, and it is what made the two widening-direction twins obviously necessary
rather than optional. A plan can be wrong about the substrate and right about the test design
in the same breath.

**Promote-when:** a sixth datapoint for reproduction-before-plan, or a second where the
plan's *mechanism* was inert while its recommended option stood. The generalisable shape is
narrower and sharper than the existing rule: **a fix option that names a file KIND is a
hypothesis about which instances are live** — count the population it would actually catch
before preferring it for being narrow. That is the population form of `R-117`, arriving from
the remedy side rather than the diagnosis side.

## F-83 — `artifact(get)` has no changed-since-your-last-read signal, so a multi-call read tears silently and the composite reads as a defect in the FILE

**Observed:** 2026-09-01, auditing `docs/trackers/issue-clusters.md` (artifact
`1b5a080fe2efcb6b`) while peer session `codescout-fc [a90dcf]` was actively writing it.

**When:** Composing an audit report out of two `artifact(action="get")` calls issued ~5.7
minutes apart against the same artifact — `start_line=116` for the `## Index` table, and
`start_line=496` for the `IC-12`…`IC-16` entry bodies.

**Expected:** Two reads of one artifact inside a single task compose into one coherent
picture of that artifact.

**Got:** The two responses were served from different working-tree states. The Index read
carried `updated_at=1788210771474` and returned `n=0` cells for `IC-13`/`IC-14`/`IC-15`/`IC-16`;
the entry-body read carried `updated_at=1788211113779` and returned `**Members:** … n=16` for
`IC-13`. Reconciled exactly afterwards: 75085 bytes (= `prev_bytes` of the final `field_patch`
event, = `git show 13226bda^`) versus 78448 bytes (= `new_bytes`, = `git show 13226bda`). **Both
commits are internally consistent** — at `13226bda^` the table *and* the `**Members:**` fields
both read 0; at `13226bda` both read 16/7/15/2. The contradiction existed only in my composite.

**Probable cause:** Every response carries `updated_at`, so the information needed to detect
the tear is present — but correlating it across calls is entirely the caller's job, and no
field says *"this artifact changed since your last read."* A torn composite is therefore
indistinguishable from a genuinely self-contradictory document, and the natural reading blames
the file rather than the read.

**Workaround:** Before publishing any claim that spans two sections of one artifact, compare
`updated_at` across every response the claim rests on; re-read if they differ. To learn exactly
which state each read saw, use `artifact_event(action="list")` and match the `field_patch`
`prev_bytes`/`new_bytes` against `git show <sha>:<path> | wc -c` — that reconstruction is exact
and is what closed this.

**Severity:** high — this session was one step from publishing five fabricated *"the ledger
contradicts itself"* findings to the user, about a peer session's correct in-flight work.
Nothing in the tool output would have flagged it, and the audit's own subject was
count-versus-prose drift, so a false positive of exactly that shape was maximally credible.

**Status:** open

**Valid:** dated 2026-09-01

Observed against codescout at `c6d7d83b`; re-verify if `artifact(get)` gains a revision token
or an if-none-match style parameter.

**Rests on:** the `field_patch` event log's `prev_bytes`/`new_bytes` being an exact
reconstruction oracle for which state a given read saw — the same oracle `IC-12` prescribes for
this class (*"`artifact_event(action="list")`'s `field_patch` byte counts, which no git
operation touches"*).

**Fix idea / Pointer:** a monotonic per-artifact revision token echoed in every
`artifact(get)` response, plus an optional `if_unchanged_since=<token>` on follow-up reads that
errors rather than silently serving a newer state. Candidate cluster for a bug file is `IC-12`
(`cluster/transient-shared-state-lies-to-readers`), which currently sits at n=0 with *"nothing
to tag yet"* — but it differs from `IC-12`'s git-hook exemplar on the diagnostic axis: there
`git stash list` actively confirms the lie, whereas here `updated_at` does report the truth and
merely goes uncorrelated. Not filed as a bug file yet: the `cluster/` slug set is being
rewritten by the peer session right now and `tests/issue_clusters.rs` gates on it.

**Second instance, independent, reported by `codescout-d9` 2026-09-01 and cited as theirs.**
Two `artifact(get)` calls against the same artifact a minute apart returned **658** then **675**
lines, straddling commit `a2aedd49`. Different session, different artifact read, same tear —
which is what makes it independent rather than a retelling of mine.

**It does NOT advance `W-89`'s counter, and the distinction is the point.** `W-89`'s
`Promote-when` asks for a third instance in which *comparing `updated_at` catches* a torn read.
`codescout-d9` never ran that comparison: they read the line growth as "the peer is writing" and
re-derived their counts from git rather than the catalog, which is why their numbers held. They
reached the right **action** by a different route and, in their own words, "did not recognise it
as a tear until you named it". So this is a second datapoint for **the defect** (`F-83`) and
zero datapoints for **the check** (`W-89`) — an outcome that looks like a confirmation and is
not one. Counting it would be the exact error this tracker's own discipline warns against: name
the proposition a result proves, then ask whether a broken world produces the same result. A
session that never compares `updated_at`, and happens to re-derive from git, produces this
identical clean outcome.

## W-89 — Comparing `updated_at` across two reads of one tracker killed five false findings before they reached the user

**Observed:** 2026-09-01, at the end of a *"check all issue clusters"* audit of
`docs/trackers/issue-clusters.md`, immediately before reporting 16 cluster counts to the user.

**Pattern:** Before publishing a claim that spans two sections of one librarian artifact,
compare `updated_at` across every response the claim rests on. If they differ, re-read and
re-derive before saying anything. When you need to know exactly which state each read saw,
escalate to `artifact_event(action="list")` and match `field_patch` `prev_bytes`/`new_bytes`
against `git show <sha>:<path>`.

**Counterfactual:** Concrete and countable. The composite sitting in context asserted **five**
discrepancies between the ledger's Index table and its own entry bodies: `IC-3` table n=20
against a measured 18, and `IC-13`/`IC-14`/`IC-15`/`IC-16` table n=0 against measured
16/7/15/2. Every one was an artifact of the torn read (`F-83`) — both commits either side of
the tear are internally consistent. Publishing them would have (a) told the user their ledger
contradicted itself in five of sixteen rows when it contradicted itself in none, (b)
misattributed a peer session's correct in-flight backfill as drift, and (c) done so inside a
report whose declared subject was count-versus-prose staleness, which is precisely what would
have made the false finding credible rather than suspect. Detection cost was one extra
`artifact(action="get")`; the post-publication cost is a retraction plus a full re-audit,
because once five counts are disputed none of the other eleven are trusted either.

**Confirming data points:**
1. `F-83` (this session) — the torn read itself, reconstructed exactly from the `field_patch`
   byte counts (75085 → 78448, matching `13226bda^` → `13226bda`).
2. The same check fired **again, four minutes later, on the same artifact**: between the audit
   report and this reconnaissance pass, `IC-3`'s promotion target moved to `OB-7` and
   `cluster/doc-contradicted-by-code` went 1 → 4, carrying `IC-11` over the n≥3 threshold. So
   the re-read is not a one-off precaution against a single unlucky moment — it is the correct
   default for the whole window in which `ListAgents` reports a busy peer.

**Impact:** high — prevented five false findings in a user-facing report, and then caught two
further decays in the report already delivered.

**Promote-when:** a third independent instance where an `updated_at` comparison catches a torn
multi-call read. At 3 datapoints, promote to the reconnaissance skill's Phase 1 as a substrate
bullet — *a versioned store read through more than one call is not a snapshot* — routed
**craft-shaped**, not project-shaped, since it holds for any store whose reads are per-section
rather than per-document.

**Status:** validated

**Valid:** dated 2026-09-01

Two datapoints, both from one session; the promote-when threshold of 3 independent instances is
not yet reached.

**Rests on:** `F-83`'s byte-level reconstruction, and on `ListAgents` reporting peer liveness —
the busy peer `codescout-fc [a90dcf]` is what made a re-read obviously worth its cost rather
than paranoia. Without a liveness signal the same discipline still applies but has no cheap
trigger, which is the open half of this win.

## F-84 — A rebuild plus `/mcp` refreshes the server but not its LSP mux delegates — six stale-image processes stayed live, one of them this repo's rust-analyzer mux

**Observed:** 2026-09-01 00:35, immediately after a `cargo rb` release build (binary mtime
00:34:16) and an `/mcp` reconnect.

**When:** Scouting whether this session's tool calls were being served by the rebuilt binary —
`R-89`'s freshness law, which names **build**, **process** and **distribution** as three
independent axes that `mtime` answers none of.

**Expected:** memory `gotchas` § *MCP Binary Symlink* states the whole remedy in one line —
*"After a release build, run `/mcp` to reconnect."* Read plainly, a reconnect makes the
session fresh.

**Got:** The reconnect refreshes the **server** and nothing else. Probing the copy that
actually serves this session — `run_command`'s shell is a child of the serving process, so
`$PPID` *is* it — gives pid 2695653, started 00:34:31, with `/proc/2695653/exe` resolving to
`target/release/codescout` and **no ` (deleted)` suffix**: genuinely fresh, 15 s after the
build. But six sibling `codescout` processes were still live on the **replaced inode**, which
the kernel reports for free as `(deleted)`. Five are `start --debug` servers belonging to other
sessions/profiles (Aug 31 11:20, 11:38, 14:46, 21:45, 22:20). The sixth is the one that
matters: **`2344690`, the rust-analyzer mux for this very project** —
`--socket …codescout-rust-mux-7e868829c00fa9b2.sock`, `--cwd` this repo — started 00:04:55,
half an hour before the build.

**Probable cause:** a mux is keyed by project hash and shared across sessions, so a freshly
started server **attaches to whatever mux already holds the socket** rather than spawning its
own. The reconnect therefore cannot reach it. Observed, not inferred: the Kotlin mux is fresh
(2693964, started 00:34:23) while the Rust mux is not, and their command lines carry
`--idle-timeout 300` and `--idle-timeout 180` respectively. I have **not** proved why one
lapsed and the other did not — what is proved is that a rebuild plus a reconnect does not
refresh a mux that stays alive. If the mechanism is idle-lapse, the inversion is the finding:
the busier a language's mux, the likelier it is to be stale, so the delegate serving the most
queries is the one most likely to be running old code.

**Workaround:** `readlink /proc/<pid>/exe` and look for the trailing ` (deleted)`. It is the
only signal here that does not read green in the broken world — binary mtime, the
`~/.cargo/bin/codescout` symlink, a clean `git status`, the last source commit and the
reconnect itself **all** reported fresh while six stale images ran. **The `$PPID` walk and the sweep answer different questions, and must not be confused.**
`run_command`'s shell is a child of your **server**, so walking up from `$PPID` tells you
whether *your server* is fresh and nothing else. A mux is a **descendant** of some server, never
an ancestor of your shell — verified 00:41: the Kotlin mux `2693964`'s parent is server
`2693697`, which is **not** this session's server (`2695653`). So a mux cannot appear in your
ancestry, may belong to another session entirely, and is reachable only by the full
`pgrep -x codescout` sweep. A reader who runs the `$PPID` line, sees a live `exe` and concludes
their muxes are fine has been misled. Correction owed to `codescout-fc`, which caught it in the
first draft of this entry.

**Severity:** med — no wrong result is attributed to it here: this session issued no
`symbols` / `references` / `symbol_at` call after the rebuild, so the stale Rust mux served
nothing. The cost is that *"I reconnected"* is currently read as proof of freshness across the
whole tool surface, and the delegate axis is invisible to every documented check. Cross-session
corollary, live this evening: peers `codescout-fc` and `codescout-d9` are on stale images, so
tool-behaviour findings exchanged between sessions are not automatically comparable.

**Status:** open

**Valid:** dated 2026-09-01

A process list is a fact about an instant — re-derive with the `/proc/<pid>/exe` sweep rather
than trusting these pids.

**Rests on:** `R-89`'s three-axis freshness law, of which this is a fourth axis —
**delegate**; and on the kernel's `(deleted)` marker being an image-identity signal that no
userspace bookkeeping can falsify.

**Fix idea / Pointer:** memory `gotchas` § *MCP Binary Symlink* should say that the reconnect
does not reach muxes, and name the `/proc/<pid>/exe` sweep as the check — a one-line doc fix
that closes the "reconnect means fresh" reading. A stronger fix is a build-id handshake at mux
attach: a mux whose build id differs from the connecting server's is asked to exit and
respawn. That needs the mux's restart cost weighed first (a Kotlin mux respawn is expensive and
`gotchas` already documents cold-start pain), so it is a proposal, not a prescription.

**Corrected 2026-09-01 00:41, and the correction lowers the severity.** The headline example is
gone: rust mux `2344690` has **exited**. It was alive 30 minutes past its `--idle-timeout 180`
because it was in continuous use, and it died once that use stopped. So a stale mux is
**self-healing** — the exposure window is bounded by "time since last use", not by the session.
The mechanism in this entry stands (a reconnect does not refresh a live mux, and a fresh server
attaches to whatever holds the socket), but the *inversion* is the whole of the risk: a session
doing continuous Rust work keeps its own stale mux alive for exactly as long as it keeps
querying, so the delegate is stale precisely while it is being used and healthy the moment
nobody cares. That is worth stating plainly, because the first draft implied a persistent
stale-process population and the truth is a use-coupled one.

**Re-derived at 00:41, with pids, so the tag is checkable.** Five servers on a deleted image —
`997544`, `1222694`, `1888275`, `1998675`, `3001659` — each parented by a distinct `claude`
process, none of them this session's, `codescout-fc`'s or `codescout-d9`'s. Six live —
`2693697`, `2693964` (Kotlin mux), `2695653` (mine), `2697016` (fc's, confirmed by fc
independently), `2699287`, `2701461` (d9's). `codescout-fc` reported four deleted and missed
`3001659`; two independent sweeps of the same population disagreeing by one is itself the
reason to record pids rather than a count.

## F-85 — Committed the exact attribution error I had corrected a peer for eight minutes earlier — the signals I used carry zero ownership information by construction

**Observed:** 2026-09-01 00:39, in a message to `codescout-fc` disclosing two pre-commit stash
windows.

**When:** Eight minutes after correcting `codescout-d9` for attributing commits `77d4da06` /
`a2aedd49` and a session log to me on the basis of commit proximity — a correction whose text
named `IC-10` explicitly and quoted its claim that *every party infers it from proximity*.

**Expected:** Having just named the class, in writing, to the party that had instantiated it, I
would not instantiate it myself in the very next message.

**Got:** I told `codescout-fc` *"your in-flight backfill"* and *"your 16 modified files … +110
lines across issue-clusters.md"*. `codescout-fc` has made **zero** edits in this checkout this
session; all its work tonight is in `prompt-engineering`. Those hunks belong to a session
neither of us has identified. My evidence was (a) `codescout-d9`'s report that fc authored the
IC-1/OB-8 work — itself an unverified attribution, and one fc had **already denied to d9** —
and (b) `git status` adjacency plus fc being the only other *busy* codescout session in
`ListAgents`. Both are precisely the proximity signal I had just told d9 was worthless.

**Probable cause — structural, not attentional.** `codescout-fc`'s formulation is better than
mine and is the durable part of this entry: on a shared checkout every session commits as the
**same git author**, and `git status` reports the **union** of every session's uncommitted work.
Author, adjacency and dirty-file lists are therefore **constant across sessions by
construction** — they carry zero ownership signal, so no amount of checking extracts one. This
is `CLAUDE.md` § *Observer Blindness* holding exactly as written: *"every one was committed by
an author actively writing about that class … Knowing the class prevented none of the four."*
Five wrong attributions ran between three sessions in under fifteen minutes — d9→me ×3, d9→fc
×1, me→fc ×1 — and **every one was resolved by asking the session, and only by asking.**

**Workaround:** ask. `SendMessage` the session and let it answer for itself. That is `W-71`'s
*replace the question, don't tighten the answer* in a second domain: "who wrote this hunk"
needs an exhaustive population and fails silently, while "did you write this hunk" has a
population of one and cannot. Never write an ownership claim about a peer's work into a message
or a ledger without that confirmation.

**Severity:** med — it reached a peer message and would have reached an `IC-10` tag naming the
wrong session as the instance's subject. It reached no commit. The cost of this class is not
the individual error but that the **correcting** party is the most confident one, so the error
travels inside a message whose subject is correctness.

**Status:** open

**Valid:** dated 2026-09-01

**Rests on:** the structural argument, which needs no session's cooperation — author-constancy
and the `git status` union are properties of the substrate. It does **not** rest on fc's
self-report of idleness; that report is corroborated only in identity (its server pid `2697016`
is parented by `claude` pid `2081267`, matching the socket its messages arrive on), which
establishes who is speaking, not what they did.

**Fix idea / Pointer:** this is the mechanism `IC-10` is missing — its `**Mechanism status:**`
reads *"none yet"*. The mechanism is not a better provenance heuristic but the admission that
none exists, plus a cheap standing protocol: **an ownership claim about another session's work
requires that session's confirmation, and is otherwise not written.** That is an unconditional
policy tied to a trigger that happens anyway, which `CLAUDE.md` § *Observer Blindness* names as
the second-best mechanism shape — and it is the one thing that worked five times out of five
tonight. Offered to `IC-10`'s owner rather than written there, since two sessions are actively
editing that ledger.

**Amended 2026-09-01 01:00 — an instrument existed, it was correctly indexed, and it would not
have saved me.** `scripts/peer-sessions.sh` has shipped since `188ab791` (2026-08-30 20:08) and
is listed at `docs/PROBES.md:144`, on a page whose header reads *"start here before answering a
question with a number"*, under a row saying verbatim: **"Run it before any 'who else is here'
or 'who wrote this' reasoning."** Neither `codescout-d9` nor I ran it. So the sentence above
about the population being unenumerable is **wrong as written**: it was enumerable all night by
one command, and the ~25%-observability figure measures `ListAgents`, not the world.

**The correction does not rescue the remedy — it hardens it.** That probe **bounds the
population and does not attribute a write**; its own row and commit message both say so, and add
the part that matters here: *elimination over a complete set is still elimination, and a bigger
correct set makes a wrong conclusion more persuasive rather than less.* My error was elimination
("fc is the only other busy session"). Run the probe and I would have eliminated over 20 instead
of 4, reached the same wrong answer, and held it with more confidence. **Asking remains the only
discriminator**, unchanged, and is now the discriminator *even when the population is fully
known* — which is a stronger claim than the entry originally made.

**The real finding is a placement failure at the SECOND attempt, and it is measured twice.**
`188ab791`'s commit message records that on 2026-08-30 **six** authorship misattributions were
made in one afternoon, each *"a correct elimination over a population short by at least two"* —
and that the mitigation until then "existed only as prose inside the bug file, so it fired when
someone opened the bug rather than when someone was about to attribute a write." The fix was to
move it to `docs/PROBES.md` precisely so it would be reached at the moment of need. Tonight,
five misattributions across three sessions, and it was reached by nobody. That is the
reconnaissance skill's **Unreachable** audit category exactly: *general enough, still not
fetched, and the remedy is placement rather than rewording* — now with two rounds of placement
improvement behind it and no change in outcome. A probe nobody runs and a prose paragraph nobody
opens are the same artifact with different line numbers. Whatever closes this is not another
relocation.

## F-86 — A guard's abort path is an unscoped write — my `git reset` fallback would have unstaged a peer's 16-file batch under a minute before they committed it

**Observed:** 2026-09-01 00:45, on being warned by `codescout-d9` that 16 files belonging to
another session were staged in the shared index.

**When:** About to run a third commit with the same guard that had wrapped the previous two
(`5d405b67` at 00:33:36 and `c385b9d6` at 00:38:54):

```
git add docs/trackers/bug-fix-session-log.md
staged=$(git diff --cached --name-only)
if [ "$staged" = "docs/trackers/bug-fix-session-log.md" ]; then
    git commit -F $S/msg.txt
else
    echo "ABORT — staged set was: $staged"; git reset
fi
```

**Expected:** the guard is the careful half of the procedure. Its success branch is scoped to a
single explicit path precisely so a concurrent peer's work cannot be swept in.

**Got:** the **failure** branch is `git reset` — bare, therefore `--mixed HEAD`, therefore
**unstaging every path in the index, not the one the guard is about**. The success branch
inherited the scoping; the abort branch inherited none of it. So the branch that exists to
prevent damaging a peer is the only one in the block that damages a peer, and it fires
*exactly* when a peer is active, because a foreign staged file is what trips it.

**The counterfactual is provable and the margin was under a minute.** The 16 staged paths were
committed by their owner as `0dea2246` at **00:44:39**; my own inspection listing those same 16
necessarily preceded it, and my `git add` — which found the index already down to one file —
necessarily followed it. My commit landed at **00:45:17**. Had that third run used the old
guard at any point in the window ending 00:44:39, `staged` would have held 17 paths, the
comparison would have failed, and the block would have printed `ABORT` while unstaging a batch
its owner committed seconds later. The two runs that did use it passed only because no foreign
path happened to be staged at 00:33 or 00:38 — which is luck, and reads identically to
correctness.

**Probable cause:** the index is shared state and `git reset` takes no pathspec in this
formulation, but the deeper cause is that **nobody reviews an abort path**. It is the branch
labelled with the cautious word; it is read as "do nothing and stop". Here it is the only
unscoped write in the procedure.

**Workaround — remove the branch rather than scope it.** `git commit -F <msg> -- <path>`
commits the working tree at that path and ignores the index for every other path, so there is
no mismatch to detect and nothing to undo. Stage first (`git add <path>`) so the committing
blob equals the staged blob, which is what satisfies the `refuse a pathspec commit carrying
unstaged content` hook — the two hazards are simultaneously satisfiable, and the pathspec form
needs no failure branch at all. Mechanism established by `codescout-d9` (`08d72a6e`); the
abort-path defect is mine.

**Severity:** high — not for the content, which is safe (`--mixed` leaves the working tree
untouched), but for the **curation and the silence**. A peer who deliberately staged 16 of 21
dirty paths loses which 16, with no notification: `git reset`'s "Unstaged changes after reset"
listing prints into *my* session, never theirs. Their next bare `git commit` then fails or
commits the wrong set, at a moment they have no reason to connect to another session.

**Status:** open

**Valid:** dated 2026-09-01

The timing argument is a fact about one instant; the structural claim about abort paths is not
and does not decay with it.

**Rests on:** `git reset` with no pathspec defaulting to `--mixed HEAD` over the whole index —
a property of git, not of this repo; and on the commit ordering above, which is fixed by
`0dea2246`'s timestamp rather than by recollection.

**Fix idea / Pointer:** the portable form is **a guard's abort path is a write, and it inherits
none of the scoping applied to its success path** — so scope the failure branch explicitly, or
choose a primitive that has none. Note the relationship to `IC-14` is *inverted*: that class is
a guard whose coverage is **narrower** than its name, this is a guard whose **blast radius is
wider than its scope**, and the two need opposite tests ("what input satisfies the name but not
the implementation?" versus "what does the failure branch touch that the success branch was
careful about?"). Cluster is `IC-1`/`IC-17` territory — a write reaching past the peers you can
see — but `IC-17` was minted minutes ago (`0dea2246`) and I have not read it, so the
classification is deliberately left to that ledger's owner rather than guessed at here.

## F-87 — A double-quoted grep pattern let the shell eat `$tmp`, and the clean zero read as "the code was deleted"

**Observed:** 2026-09-01, post-rebuild reconnaissance over two bug files filed earlier
in the same session, verifying their `path:line` citations still held after peers landed
two commits.

**When:** Checking whether `71499331` — a commit titled `docs(issues): archive the
foreign-index fix` that also touched `scripts/post-index-change-stage-log.sh` — had moved
the mechanism my bug file cites.

**Expected:** `grep -n 'mv -f "$tmp"' scripts/post-index-change-stage-log.sh` to print the
atomic-replace line I had read minutes earlier.

**Got:** zero output. Read at face value this says the `mv`/`rm` fallback was **deleted** by
that commit — which would have made my just-filed bug file wrong about its own root cause,
and the natural next action was to file a correction retracting it.

The line is present, at `:141`. The pattern was written in **double quotes**, so the shell
expanded `$tmp` before `grep` ever saw it and the search actually run was `mv -f ""`.
`grep -c 'mv -f "\$tmp"'` (single-quoted) returns 1.

**Probable cause:** A shell variable inside a double-quoted grep pattern is indistinguishable
at a glance from a literal, and the failure has no error surface — `grep` is *given* a valid
pattern and correctly reports no match for it. The zero is a true statement about a query I
did not write. Compounding it: the query ran immediately after `git show --stat` proved that
commit *had* changed the file, so a prior belief ("the script moved") was already in place and
the zero confirmed it. That ordering is what made the false negative persuasive rather than
suspicious.

**Workaround:** Single-quote any grep pattern containing `$`, `` ` ``, `\`, or `!`. Where a
pattern must be assembled from a variable, echo it before use. For "did this change land?",
prefer comparing the artifact (`git show <sha> -- <path>`, checksum) over searching it — the
diff answers the question the grep only approximates.

**Severity:** med — no wrong artifact shipped, but the next action queued was a retraction of a
correct root cause in `docs/issues/archive/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md`.
Cost had it landed: a bug file arguing against its own measured evidence, plus a peer acting on
the retraction.

**Status:** fixed-verified — pattern re-run single-quoted, line confirmed at `:141`, and the
bug file's stale `:142` citation corrected in the same pass.

**Valid:** invariant

Shell expansion inside double quotes is a property of POSIX shells, not of this repo or this
grep version.

**Rests on:** `scripts/post-index-change-stage-log.sh:141` as of `71499331`; the *line number*
is dated, the quoting law is not.

**Fix idea / Pointer:** This is the skill's own "a search that finds nothing is evidence about
the search" law (`R-3` → `R-113` → `R-77` → `R-79` → `R-104`) in a mechanism that chain does not
yet name: the promoted text covers **scope**, **shape** and **encoding**, and this is a fourth —
**the pattern was mutated by the shell before the tool ran**. Candidate `R-N` proposal: add
*"quote the pattern"* to that bullet, since the existing three mechanisms all assume the tool
received the pattern the author wrote.

## W-90 — Post-rebuild recon caught a stale line citation in a bug file authored the same session

**Observed:** 2026-09-01, reconnaissance run immediately after a `cargo rb` + `/mcp`
reconnect, over two bug files filed earlier in the same session.

**Pattern:** After a rebuild/reconnect, re-verify the `path:line` citations in any artifact
**this session authored**, not just the code about to be touched. Authorship is not an
exemption (`R-49`), and a bug file's citations are written at fix time while the substrate is
still moving under peer commits.

**Counterfactual:** `docs/issues/archive/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md`
was filed citing `scripts/post-index-change-stage-log.sh:142` for the
`mv -f "$tmp" "$log" … || rm -f "$tmp"` fallback. The line is `:141`; `:142` is **blank**. The
off-by-one entered because the file was read once at `a987df96` and cited from that reading
while `71499331` was landing.

Nothing downstream would have caught it. `audit_doc_refs` **does** scan `**/*.sh`, but
`scan_code_comments` forces those findings to **`Med`**, and CI runs `--fail-on high` — so the
gate passes by design. The citation would have survived until a human followed it and landed on
whitespace, in a file whose whole argument is that a silent wrong answer costs more than a loud
one.

**Confirming data points:**
1. This session — `:142` → `:141`, corrected in the same pass that found it, before the file
   was ever committed.
2. Same pass, opposite direction: a commit titled `docs(issues): archive …` genuinely *had*
   changed `scripts/post-index-change-stage-log.sh`, which looked like a capture
   (`cluster/shared-resource-carries-no-owner` has 15 members). Reading the diff showed a
   comment-only re-point of an archived path — **coherent with its message**. The scout is what
   separated the real drift from the false accusation; without it, the plausible move was to
   file a third capture bug against a correct commit.

**Impact:** med — one wrong citation prevented, one wrong bug file prevented.

**Promote-when:** a second session finds a stale citation in an artifact it authored earlier in
that same session. At 2 datapoints, promote to CLAUDE.md § *Bug Tracking* as *"re-verify
`path:line` citations in bug files you authored this session before committing them — peers move
the substrate between the read and the write."*

**Status:** validated — single datapoint, drift caught and corrected before commit.

**Valid:** dated 2026-09-01

One confirmed datapoint; promote-when threshold (2) not reached.

**Rests on:** `audit_doc_refs`' `scan_code_comments` forcing `Med` on source-comment findings,
and CI running `--fail-on high` — the two together are why this class has no automated gate.

## F-88 — Bulk-inserted a section into 7 files after verifying the shape on 1 — one target already had it, and the check I ran could not have told me

**Observed:** 2026-09-01, anchoring the 11 live terminal bug files `doctor` reported under
`terminal_status_without_fix_anchor`.

**When:** Adding a `## Fix provenance` section to 7 of them via
`artifact(update, body_edits=[{action: "insert_after", heading: "## Fix"}])`. I applied the
first file alone, verified it, then batched the remaining six.

**Expected:** None of the 7 had a `## Fix provenance` section. That is what `doctor` said —
*"no `## Fix provenance` pointer is declared"* — and I read it as a statement about the files.

**Got:** `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`
already had one. My insert produced a **second** `## Fix provenance` heading in the same
file: two byte-identical headings, which is `IC-6` — the cluster this repo maintains for
exactly that — and removing mine required the `occurrence` disambiguator shipped by
`identical-headings`, another file in the same batch of seven.

The pre-existing section declared the same SHA and patch-id, written `- SHA:` / `- patch-id:`
rather than the bolded `- **SHA:**` that `structured_fix_pointers` matches. So `doctor`'s
sentence was true of the **parse** and false of the **file**, and I supplied the stronger
reading it never claimed.

**Probable cause:** The check I ran after the first insert asked *did the section land
between `## Fix` and `## Tests added`* — an assertion **monotone under "a section already
existed here"**. It can confirm an insert worked; it can never report that the insert was
inappropriate. Having satisfied it once, I generalised from n=1 to seven targets. This is
CLAUDE.md § *Testing Discipline*'s first law firing on a scout rather than on a test: I
mutated in the direction the assertion could see.

**Workaround:** `grep -c '^## Fix provenance' <all 7 targets>` before inserting into any.
One command, and it answers the precondition that actually mattered — *absent in all N* —
which no post-hoc placement check can establish.

**Severity:** med — one duplicate heading, self-inflicted, caught by re-running `doctor`;
cost was a remove-with-`occurrence` plus a diagnostic detour. It would have been **high**
had I taken the batch as done without re-running the check: a duplicate `## Fix provenance`
is permanently unaddressable by heading, and the file would have carried two provenance
blocks with no way to edit either by name.

**Status:** fixed-verified — duplicate removed, `doctor` re-run reports 0, and a from-disk
re-derivation sharing none of `doctor`'s code agrees.

**Valid:** dated 2026-09-01

True of `structured_fix_pointers`' accepted grammar at `370775ca`; re-verify if the parser
widens to accept unbolded pairs.

**Rests on:** `structured_fix_pointers` matching only the bolded `- **SHA:**` form and
skipping fenced lines (`src/librarian/tools/doctor.rs:4535-4579`), and on `doctor`'s finding
text saying *declared* where it means *parsed*.

**Fix idea / Pointer:** Before a bulk edit that **inserts** a section, scout the whole target
set for that section's presence. The parser half — an unbalanced fence silently disabling
every line-anchored field below it, which is why the anchor did not take even once written
correctly — is filed as
`docs/issues/archive/2026-09-01-an-unbalanced-fence-silently-disables-every-line-anchored-field.md`.

## W-91 — Re-reading the substrate before writing a durable claim killed two false records — one about the filesystem, one about a capability that already existed

**Observed:** 2026-09-01, twice in one session working the `docs/issues/` tracker.

**Pattern:** Before a claim goes into a **durable, queryable** record — a frontmatter field,
a new bug file's Summary — re-read the thing the claim is about, this session. The
filesystem for a claim about the filesystem; the implementation for a claim about a missing
capability.

**Counterfactual:** Two catches, neither of which would have surfaced later.

1. **A doc's claim about the filesystem.**
   `2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` carried a
   `## Residual — still open` section stating `.worktrees/bench` retains an orphaned gitdir
   pointing at a repository that does not exist. I was about to add an `unverified:` field
   describing that residual as open. `ls .worktrees/` returns only `audit-trail-t1` — `bench`
   is gone entirely. Without the check I would have written a **queryable** field asserting
   an open residual that does not exist, into the one file in the corpus tagged
   `cluster/record-asserts-an-unchecked-completion`.

2. **An absence claim about the implementation.** I had begun drafting a bug asserting
   `unverified:` is unreachable by any query, on two pieces of evidence that both held:
   `find` returns `unknown field 'unverified'`, and `get_guide("librarian")` states `extra`
   is *"NOT catalog-indexed / not filterable via find"*. Reading
   `scan_terminal_status_with_caveat` (`src/librarian/tools/doctor.rs`) showed the reader
   exists and is deliberate — its own doc-comment calls it *"the reader half of a feature
   that shipped without one"* — and a live run reports **65** records. The bug would have
   asserted a false gap in a ledger this project treats as load-bearing.

**Confirming data points:** the two above. Note what they share: in both, the false claim was
supported by real evidence that was simply about something narrower than the sentence it was
about to license — `find`'s field list is not the set of queries, and a `## Residual` heading
is not the filesystem.

**Impact:** high — (2) would have put a false claim into the permanent bug ledger; (1) into a
frontmatter field a query reads.

**This is a recurrence of an already-promoted law, not a new pattern — recorded as such
deliberately.** The reconnaissance skill's Phase 1 already carries *"a proposed fix — and
equally a prohibition — is a claim about CURRENT STATE; verify it before designing around
it"*, with *"treat 'X is already set' as a finding, not a dead end."* Datapoint 2 is exactly
its absence form. Per that skill's § *Every promotion audits the promoted set*, filing this
as a fresh pattern would be the defect that section names. The one thing the promoted wording
does **not** name is the surface I was on: it enumerates *fixes* and *prohibitions*, and a
**bug filing** is a third — arguably the costliest, since a fix that assumes a missing
capability fails loudly at the call site, while a filed bug asserting a missing capability is
durable, cited, and nothing re-checks it. That is an `outgrown` signal (category 2) on the
promoted text, of one datapoint.

**Promote-when: FIRED the same day — and the trigger is a MISS, which is what makes it
worth more than either catch above.** The second bug-filing-shaped instance of the absence
form is `docs/issues/archive/2026-09-01-an-unbalanced-fence-silently-disables-every-line-anchored-field.md`,
whose first version claimed the hand-rolled fenced-line convention also governs `**Valid:**`
and `**Rests on:**`, so every line-anchored field was exposed. Verified false:
`parse_validity` / `parse_rests_on` (`src/librarian/statements.rs`) already use `FenceState`,
**and** receive one section's text at a time with a fresh tracker per call, so neither axis
was ever open. A guard asserted missing that was in fact present — the absence form, on the
bug-filing surface, exactly the shape this entry predicted.

**What separates it from datapoints 1 and 2: there, doubt occurred and the claim never
shipped. Here the law did not fire.** The claim entered the filed record and was caught an
hour later on re-entry (`R-49`), by me, while implementing the fix it was written for. A
ledger holding only its catches has a population with no falsifying member in it, so
datapoints 1 and 2 establish a habit and this one establishes a rate.

**Correction 2026-09-01, raised by `codescout-e6` and verified here before accepting it:
the sentence above cited CLAUDE.md's *recording-filter* law and that citation was wrong.**
This is the **member-selection** half — the one the recording-filter law names as its own
contrast (*"The law above is about members: a population selected so no member can
falsify"*). One question separates them: **does widening the sample help?** Here it does.
The miss left artifacts — the filed bug file, its `## Correction` paragraph, and `800f1dec`'s
message — all on the record and greppable. The decisive proof is that **the repair was to
add the miss to this entry, and it worked**; under the recording-filter law that addition is
impossible, because there would be nothing to add. The remedies differ, which is why the
mislabel mattered: member-selection says *go read more filed records*, recording-filter says
*publish your confirmations*, and a reader inheriting the wrong tag reaches for the wrong one.

**There IS a recording-filter instance adjacent to this one, and it is a different
population.** Every time re-reading the substrate **confirmed** the claim, nothing was
produced — no finding, no entry, no commit. So this ledger can never answer *how often does
re-reading change the answer*, at any corpus size. CLAUDE.md's own worked example already
names that shape and its consequence: a confirming re-derivation *"is a **denominator**, not
a catch … bears on the base rate rather than on the hit rate."* W-91's catches are hit-rate
evidence with no denominator behind them. Keeping the two apart is the point — same session,
same practice, two adjacent laws, one instance of each.

**Denominator datapoint 1 — published because a confirming re-read otherwise leaves no
record, which is the whole remedy the paragraph above prescribes.** Later the same day,
`codescout-e6` excluded their `F-92` from `OB-1` on the grounds that its claim was
*unverified* rather than *under-specified*. I formed an objection — that `OB-1`'s own
mechanism, *"report a search count together with the key you searched for"*, would have
surfaced the defect, so the exclusion was too clean — and re-read `F-92` before sending it.
The objection died: F-92's own text says the window *"which I had designed forty minutes
earlier and which the tool prints on its own output line"*. `OB-1` requires the parameter to
be **invisible** to the author; here it was printed on screen and unread. Publishing a
parameter the author already has in front of them changes nothing, so the remedies really do
diverge and the exclusion was right.

**Nothing would have recorded this.** No finding, no correction, no commit — the peer would
never have learned an objection existed, and the base rate would stay unmeasurable. That is
exactly why it is written down, and it is worth noting how unnatural it felt to write: there
is no result here, only a re-read that changed nothing except whether a wrong message got
sent.

**Correction to this entry's own claim about the skill text, found by reading it.** The
paragraph above says the promoted wording *"enumerates fixes and prohibitions"* and does not
name the surface I was on. Read at the bytes 2026-09-01 (`skills/reconnaissance/SKILL.md`
in `claude-plugins`, Phase 1 — the current-state bullet): it **does** — *"stated as a
principle, it can travel into a filed defect entry and into a question put to a human"* —
but hangs that clause off the **prohibition** form alone. So what shipped is narrower and
sharper than what this entry proposed: not *name filed defect records* (already there), but
**detach the surface from the prohibition form**, since the absence form reaches the same
durable record and now has two instances doing so. That correction is this entry's own law
applied to this entry — a durable record making a claim about a substrate, narrowed by
re-reading the substrate before acting on it.

**Status:** promoted-to-permanent-docs — shipped in `claude-plugins` as `b74c730`
(patch-id `1e175ea0182cb6a134111fe7bb6baa23fd6085dd`), released as codescout-companion
`1.19.11` (`3e65211`), tracker refreshed at `11da775`, and the **served** copy probed
present in all three profiles per `R-89` — not the repo copy. Both original catches remain
confirmed against the substrate the same session.

**Valid:** dated 2026-09-01

**Rests on:** the skill's existing current-state law, of which this is an instance rather
than independent evidence — and on `scan_terminal_status_with_caveat` remaining the reader
for `unverified:`.

## W-92 — The Session-Id commit trailer is a positive authorship instrument; elimination over ListAgents is not

**Observed:** 2026-09-01. A peer session sent a courteous heads-up that "your uncommitted
doctor.rs work" (`scan_unterminated_fence`, written but not wired) had reddened the shared gate
for ~10 minutes — clippy `-D dead-code` aborting compilation plus two failing tests. The work
was not mine. I had not opened a `.rs` file all session.

**Pattern:** On a shared checkout, resolve authorship with the **`Session-Id` commit trailer**,
which this repo already writes on every commit. It is a *positive* identifier: it says whose a
commit **is**, rather than whose it is not.

```
git log -8 --format='%h %s%n  %(trailers:key=Session-Id,valueonly)'
```

Three sessions separated cleanly on one command:

```
d91c1155-…  me            7c44a605, 7278508e, fc48f829, d4c5ec46   — 0 .rs files in any
c2a08c22-…  codescout-68  (had volunteered its own id earlier)
bcc98c22-…  a fourth      d98a664b, 75c33bdc, bcf6075c — ALL src/librarian/tools/
```

`bcc98c22` owns other commits in the directory `doctor.rs` lives in, so I named it the likely
owner.

**That was WRONG, and the correction is the more useful half of this entry.** `800f1dec`
(`fix(doctor): structured_fix_pointers used a hand-rolled fence toggle; add unterminated_fence`)
carries `Session-Id: c2a08c22-…` — **`codescout-68`**, who then said so themselves. The
uncommitted work was theirs all along.

**The instrument is sound; my application of it was not, and the limit is sharp: the trailer
exists only on COMMITTED work.** The disputed work was *uncommitted*, so it carried no trailer,
and I substituted *neighbourhood adjacency* — "this session owns nearby commits in the same
directory" — and presented the result as a positive ID. That is elimination-by-proximity: the
same class of error I had just corrected in the peer, committed **in the message correcting
them**, about that class.

CLAUDE.md § *Observer Blindness* names this exactly — *"every one was committed by an author
actively writing about that class… knowing the class prevented none of the four."* Knowing it
prevented nothing here either. The usable rule is therefore narrower than this entry's title
suggests:

- **committed work** → the `Session-Id` trailer is a positive ID, cheap and exact;
- **uncommitted work** → there is *no* positive instrument on this checkout. Adjacency,
  directory, `ListAgents` and `git status` are all elimination in disguise. **Ask the session**
  (`F-85`), and until it answers, the honest statement is "not mine", never "yours".

**Counterfactual:** The peer's own diagnostics were sound and they ran them — `git status`
showing `doctor.rs` modified, `git grep <symbol> HEAD` returning absent. Those establish the work
is *not theirs*. The step that failed was assigning it to me on no instrument at all.

**~~elimination over a population no instrument reports completely~~ — that diagnosis was WRONG,
and it is the most instructive error in this entry.** It read: *"`peer-sessions.sh` showed 5
sessions while `ListAgents` showed 4, and the actual owner sits in that gap."* The actual owner
was `c2a08c22` / **codescout-68 — inside the visible four, for both of us.** The gap I blamed did
not contain the answer.

Corrected by the sender, who saw it before I did: *"I wasn't reasoning from an incomplete
enumeration — I wasn't reasoning from an instrument at all, just from who I'd been talking to."*
Conversational salience, not a coverage gap.

**And my own error has the same shape, one turn later and worse.** I named `bcc98c22` — and
cited *its invisibility to `ListAgents`* as if that corroborated the ID. The owner was the
session I could see; I picked one I could not, and read my inability to see it as evidence.

So *"`ListAgents` under-reports"* is **true, documented (BL-58), and did no work in either
error.** That is its own instrument-swap: a real limitation, correctly recalled, standing in for
a diagnosis nobody performed. A true fact adjacent to the failure is the most durable kind of
wrong explanation, because nothing about it ever reads as false.

**The cost was not zero, and this entry said it was for about ten minutes.** Corrected by the
peer themselves, who named the mechanism better than I had: *"my message asserted your authorship
in its first line and you had to spend a reply disproving a negative — that's the cost even when
the ask is 'nothing needed from you'."* A misattribution addressed TO someone is not free
because it is actionless; the recipient still has to spend a turn establishing a negative about
their own work, and the burden lands on the party with the least reason to suspect a problem.
Measured here: one full turn, four verification commands. Filing it in a bug file would have
been worse (`F-80`), but "worse" is not "the only cost".

That correction is itself the `R-49` shape — re-entering your own just-written artifact is a
seam, and authorship is no exemption. This entry was **twelve minutes old** when the claim in it
was refuted, by the one party structurally placed to know: the sender, who knew what writing the
message had cost them to send and what it cost me to answer. I could not see it because I was
the one who paid it and read the ask, not the turn, as the cost.

**Confirming data points:**
1. `F-80` (this log) — authorship closed by elimination, sent as a positive ID, and wrong;
   remedy proposed there was grepping session transcripts for `tool_use` write calls.
2. `F-85` (this log) — five wrong attributions across three sessions in 15 minutes, **all five**
   resolved by asking the session and only by asking.
3. This entry — the first time the `Session-Id` trailer was used as the instrument, and it is
   **cheaper than both** prior remedies: one `git log`, no socket enumeration, no transcript
   scan, and it reaches sessions that have already exited.

**Impact:** med — one misattribution corrected before it was recorded anywhere, and a positive
instrument identified that costs one command.

**Promote-when:** a second session resolves an authorship question with the `Session-Id` trailer.
At 2 datapoints, promote to CLAUDE.md § *Git Workflow* as *"resolve authorship with the
`Session-Id` trailer, never by elimination over `ListAgents` — it under-reports by construction
(BL-58), and the trailer reaches exited sessions."*

**Status:** validated — single datapoint, misattribution corrected at source.

**Valid:** dated 2026-09-01

**Rests on:** this repo's commit convention of writing a `Session-Id` trailer. If that convention
lapses the instrument disappears, which is itself an argument for keeping it.

## F-89 — I misattributed by directory adjacency in the message correcting a peer for misattributing by elimination

**Observed:** 2026-09-01. A peer misattributed uncommitted `doctor.rs` work to me. I corrected
them, citing `F-80` (*"closed an authorship question by ELIMINATION over a population no
instrument reports completely, and sent it as a positive ID"*), promoted the `Session-Id` commit
trailer as the positive instrument, and named `bcc98c22-…` as the likely owner.

**When:** In the correction message itself. `W-92` was written from it minutes later.

**Expected:** That the trailer, being a positive identifier, had given me a positive ID.

**Got:** The owner was **`c2a08c22`** — `codescout-68`, who committed the work as `800f1dec` and
then volunteered it unprompted. My reasoning for `bcc98c22` was that it owned *other* commits in
`src/librarian/tools/`. That is **elimination by directory adjacency** — structurally the same
error as the peer's, committed in the message correcting it, about that class.

**Probable cause, and it is a real limit rather than carelessness:** the trailer exists only on
**committed** work. The disputed work was *uncommitted*, so no trailer existed for it, and I
reached for the nearest proxy without noticing I had changed instruments mid-argument. The
sentence *"the Session-Id trailer is the positive instrument"* was true and did not apply to the
object in front of me.

**Sharpened 2026-09-01 by the peer, and it makes both errors worse than first written.** The real
owner, `c2a08c22` / codescout-68, was **inside the visible four** — for them *and* for me. So the
`ListAgents` coverage gap explains neither error. Theirs came from conversational salience (*"who
I'd been talking to"*, their words); mine picked a session I could **not** see and offered *its
invisibility as corroboration* — I wrote *"and it does not appear in my `ListAgents`"* as though
that supported the ID rather than undermining it.

That makes *"`ListAgents` under-reports"* a **true, documented (BL-58) fact that did no work in
either failure** — recalled correctly, deployed as a diagnosis nobody had performed. It is the
same instrument-swap one level up, and the hardest kind to catch: a false explanation assembled
entirely from true parts, where no individual claim ever reads as wrong.

**Three misattributions of one uncommitted file in one evening, by three parties, every one
actively reasoning about attribution** — peer→me, me→`bcc98c22` (inside the message correcting
the first, citing `F-80` as the reason not to), and codescout-68→me on dirty `src/librarian/
tools/` paths I have never touched. At three-for-three, "be careful" is empirically not the
remedy; that is CLAUDE.md § *Observer Blindness*'s exact argument for building the third thing.

CLAUDE.md § *Observer Blindness*: *"every one was committed by an author actively writing about
that class… knowing the class prevented none of the four."* Knowing it prevented nothing here.
Note also that `F-85` — *five wrong attributions, all five resolved by asking the session and
only by asking* — was already in this ledger, cited by me in the same message, and I still did
not ask.

**Workaround:** Split the rule by artifact state, which `W-92` now does:

- **committed** → `Session-Id` trailer. Positive, exact, one `git log`, reaches exited sessions.
- **uncommitted** → **no positive instrument exists on a shared checkout.** Adjacency, directory,
  `ListAgents`, `git status` and dirty-file lists are all elimination wearing different clothes.
  Ask the session. Until it answers, the only honest claim is *"not mine"* — never *"yours"*.

**Severity:** med — the wrong name went to one peer in one message and was corrected within the
hour by the true owner volunteering. It would have been high had it reached `W-92` unqualified,
which it did for roughly twenty minutes.

**Status:** fixed-verified — `W-92` body and index row corrected in place, both peers told.

**Valid:** invariant

The state-split is a property of git: an uncommitted change has no commit trailer. That does not
decay.

**Rests on:** this repo's convention of writing a `Session-Id` trailer, which is what makes the
committed half work at all.

**Fix idea / Pointer:** The asymmetry is worth a mechanism rather than a resolution to be
careful — that is § *Observer Blindness*'s whole point. Cheapest candidate: have the companion
stamp the session id into a per-session scratch file listing paths it has written, so
*uncommitted* work gains the positive instrument that only committed work has today. Until then
the answer is `F-85`'s: ask.

## F-90 — Being blocked twice felt like having tested it — I published an unprobed prohibition to five surfaces

**Observed:** 2026-09-01, fixing `staging_op`'s detached-flag parse bug (`7278508e`).

**When:** Writing the justification. The companion's worktree guard had blocked two of my
commands demanding `git -C <path>`, and `git -C <path> add` was the form losing the stager. I
concluded the two guards were in **direct tension** — following one defeats the other — and made
that the headline argument.

**Expected:** n/a — I did not test it. The blocks felt like evidence.

**Got:** Probed after a peer pushed back, one command:

```
git add --dry-run docs/RELEASE.md      -> exit 0, NOT blocked
```

The worktree guard refuses the **commit-family** verbs (`commit/push/reset/rebase/merge/
checkout -b`). It never refuses `git add`. Both of my blocked commands contained `git commit`;
the `git add` beside it was incidental. Since attribution is recorded at **staging** time, the
compliant path leaves staging bare and attributable, and the mandated `-C` lands on the commit,
which claims nothing. **The tension does not exist on the path where attribution happens.**

The population was inflated too, and I only measured it when prompted: **32 of 1586** real
`git add` invocations use `-C` — **2.0%** — and **0** use the separated `--git-dir <path>` form.
Not empty, so the fix is real and those 32 were genuinely mis-attributed in the recoverable
direction; but "2% of staging invocations" is a different claim from "the form the guard
mandates".

**Probable cause:** A prohibition reads as a *judgement* rather than as something anyone could
look up, so it does not trigger the reflex that would question a causal claim. Two standing rules
already covered this and neither fired. CLAUDE.md § *Companion Plugin*: *"do not read its
redirects as hard denials, and never infer a tool's availability from this file — call the tool
once instead."* The reconnaissance skill: *"a prohibition is a claim about CURRENT STATE… the
prohibition form is the costliest and the easiest to miss."* I had been blocked twice, which felt
like having tested it. Being blocked by a guard tells you what it refuses, never what it permits.

**Blast radius, because this is the part that stings:** the claim shipped to **five** surfaces
before anyone checked it — three commit messages (`7278508e`, `fc48f829`, and the peer messages
quoting it), the bug file `882fea0f3d66d72f`, and a comment block in
`scripts/post-index-change-stage-log.sh` where a future reader would have taken it as established.
Caught by `codescout-68`, who had hit both guards in sequence and noticed the ordering put the
`-C` on the commit, not the stage — and who flagged it as *"a lead again, not a finding"* rather
than asserting it. Their epistemic care is what made it cheap to check.

**Workaround:** Probe the permitted case, not the refused one. One `--dry-run` would have done it
at any point in the preceding two hours.

**Severity:** med — the *fix* is correct and unaffected; the *reasoning published alongside it*
was wrong, in a source comment that outlives the commit message.

**Status:** fixed-verified — comment block in `post-index-change-stage-log.sh` rewritten to state
the probe, the 2.0% measurement, and the retraction; bug file corrected.

**Valid:** invariant

"Being refused by a guard establishes what it refuses, not what it permits" is a property of
one-sided evidence, not of this guard.

**Rests on:** the probe (`git add --dry-run`, exit 0) rather than on the hook source, which
CLAUDE.md explicitly says is not ground truth for this guard.

## F-91 — A positive control certified the incident, not the substrate — and the substrate could not carry the feature

**Valid:** dated 2026-09-01

**Category:** measurement · **Status:** fixed-verified · **Valid:** dated 2026-09-01

**Observed:** The IC-10 calibration (committed `440dd773`) asked *"would a provenance channel
built on `.buddy/<sid>/cs_tool_log.jsonl` have answered the incident?"*, ran a two-directional
positive control on the owning session's log, and concluded **build it**. The control was
sound — that log really does hold the twelve `doctor.rs` writes, and the transcript really does
show zero native repo writes.

**Got:** The question the control does not answer is whether the log **retains records at all**.
It does not, on three independent axes, and one `wc -l` over the corpus showed it:

- `MAX_ENTRIES = 50`, a rolling cap — **196 of 297** logs sit exactly at it;
- deleted on compact / resume / clear (`hook_helpers.py:244,420`) — the session running the
  calibration had made **370** codescout calls and its own log held **1**;
- `args` cut to 200 chars over unordered keys — **11.5%** of 1,920 write records carry no
  `path=`, and 147 more carry a truncated one (`src/serve`, `src/lsp/m`).

The sampled session had compacted zero times and its writes fell inside the surviving window:
it was the one session in the good state **on all three axes at once**. Not a mistake about the
data — a mistake about the population, and the report read as a green light.

**Root limit:** this is *reconnaissance*'s "a green result certifies the path that actually
EXECUTED", firing on a control I designed to be careful — I checked the **other direction**
(native writes: zero) and reported that as the rigour, which is a check on the *inference* and
not on the *substrate*. A two-directional control over one sample is still one sample.

**Cheapest sufficient check, and it costs nothing:** before generalising from any sample of a
log, measure the log's own retention — line-count distribution, deletion sites, field caps.
`wc -l .buddy/*/cs_tool_log.jsonl | sort -n | uniq -c` would have shown a spike of 196 at
exactly 50 and ended the design in one command.

**Consequence:** none shipped. The substrate was rejected before any code was written, and the
channel was built on Claude Code's own transcripts instead — which also turn out to see the
`sed -i` / `cat >` writes the calibration had recorded as the channel's blind spot, because
that blind spot was measured *from the transcripts* in the first place.

**Fixed by:** `scripts/file-provenance.py` + `tests/file-provenance.sh` (43 cases).
The writer defect is filed at
`claude-plugins:docs/issues/2026-09-01-summarize-args-destroys-the-path-it-documents-preserving.md`.

## W-93 — A green fixture suite met real data and the correction was to the question, not the code

**Valid:** dated 2026-09-01

**Category:** design · **Status:** validated · **Valid:** dated 2026-09-01

**Observed:** `scripts/file-provenance.py` passed 34/34 fixture cases before it was ever run on
real data. Every fixture had one or two writes in an otherwise empty timeline, so the suite was
green on a tool that had no concept of time at all.

**Counterfactual:** the first real-data run — the positive control on the file that motivated
the bug — returned **fifteen sessions** for `src/librarian/tools/doctor.rs`, including the two
that a naive mention-count had already wrongly ranked first. Nothing was broken. Every one of
the fifteen had genuinely written the file at some point across its months-long life.

**What it revealed was a defect in the QUESTION, not the query.** "Who has ever written this
file?" is answerable and useless. "This file is dirty *now* and reds my build — is that mine?"
is the question at a red build, and it is bounded: writes older than the path's last commit are
baked into HEAD and say nothing about the working-tree delta. The default window became *since
this path was last committed*, per path.

**Why no test could have caught it.** The window is invisible to any fixture whose timeline
holds a single write, and a fixture author writing "the" test for authorship naturally writes
exactly that. This is not a coverage gap that a further case would have closed — it is a
question the fixtures could not pose, which is the *Parsers Over a Namespace* shape at the level
of test design: the suite exercises the inputs the fixture grammar admits.

**Promote-when:** a second instance where a green fixture suite meets real data and the
correction is to the *question* rather than the code. Pairs with F-91 — same session, same
direction, both caught by contact with the corpus rather than by more careful thought.

**Also caught by real data, worth recording separately:** three assertions in that suite passed
**vacuously**, each for a different reason, and all three were found by reading output rather
than by the suite failing —
(a) `has "...flagged as unplaceable in time" "$out" "undated"` matched the substring `undated`
in its own fixture **path name** `src/undated.rs`;
(b) `has "git mv destination"` was answered by an earlier section's write to the same fixture
path `docs/new.md`;
(c) `hasnt "cp source is not a write"` passed because the fixture source was `/tmp/staged.rs`,
which `normalize()` discards as out-of-tree before any verb logic runs.
All three are the fixture-detail law: the assertion states what must be true, and nothing states
which part of the **setup** is what makes it able to tell. Each fixture is now annotated on its
own line with what breaks if the detail goes.

## F-92 — Corroborated a correct verdict with evidence from outside its own window

**Valid:** dated 2026-09-01

**Category:** self-friction · **Status:** fixed-verified · **Valid:** dated 2026-09-01

**Observed:** Having shipped `scripts/file-provenance.py`, I reported its first live multi-author
verdict to the user: `docs/trackers/bug-fix-session-log.md` reads `SHARED` (me + `c2a08c22`),
*"corroborated independently by F-83 appearing 7× and not being mine."*

**Got:** F-83 entered the file at `5d405b67`, which **predates** the window floor the verdict
was computed under (`9188d000`, 04:10:32). A write outside the window is not evidence about the
window. The real in-window write was `c2a08c22`'s W-91 at `da44f73d` (04:49:34) — which I
learned from the peer volunteering it, not from any check of mine.

**The verdict was right and the evidence was invalid**, which is the worse of the two ways to be
wrong: a wrong verdict gets corrected the moment anyone re-runs the tool, while a right verdict
with fabricated support survives every check that only re-checks the answer.

**Root limit — this is the evening's own class, recurring inside the tool built to close it.**
Every part true: F-83 is real, it is in the file seven times, it is not mine. No individual
claim false, so nothing reads as wrong on review. And **no step was checked against the object
in front of me** — the window, which I had designed forty minutes earlier and which the tool
prints on its own output line. Same structure as the three misattributions in
`docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`, and this
time the instrument that would have answered was already open.

**Why "corroboration" specifically is the exposed spot:** the verdict itself came from the tool
and was therefore checked. The corroboration was mine, added to make the verdict *sound*
better-supported, and it entered the report through the one channel with no gate on it. A
sentence written to increase confidence is not itself subject to the check it is boosting.

**Cheapest sufficient check, and it is one command:**
`git log <floor>..HEAD -- <path>` — the window is printed by the tool, so the query needs no
recall. It returns `da44f73d` immediately.

**Consequence:** published to the user in a session summary; corrected in the same session
after the peer's message prompted a re-read. No artifact carried it.

## F-93 — Two counts over a live append-only transcript were true when published and false within the hour

**Valid:** dated 2026-09-01

**Category:** measurement · **Status:** fixed-verified · **Valid:** dated 2026-09-01

**Observed:** F-92's remedy was *"go audit the other fourteen published corroborations"* — and
that sentence is itself the F-72 shape (a caveat naming a specific next action is a work item),
so it was run rather than filed. Enumerating this session's corroborating claims from its own
transcript returned **15**, of which two carried numbers into a **committed** artifact.

**Got:** both numbers are now false.

| published in `05fceb5783a0290e` | at audit |
|---|---:|
| the owning session used `Write` **8** times | **9** |
| that transcript mentions `doctor.rs` **80** times | **187** |

**Root limit — the substrate is append-only and still being written.** Both counts were correct
when taken. The measured object is a peer's **live** CC transcript, and that peer spent the
evening messaging this session about `doctor.rs`; each reply added a scratchpad `Write` and more
mentions. A retention-swept corpus makes every count a **floor** — the familiar case, already
documented on several `docs/PROBES.md` rows. An append-only corpus makes every count a floor that
**rises**, which is the inverse and is worse in one specific way: the number decays in the
direction of looking like a *conservative undercount* rather than an error, so a reader who
re-derives it and gets more has no reason to suspect the original was ever a different value.

**And the conclusion never moved**, which is what removed the last prompt to look: `Edit` is still
0, and 9 of 9 native writes are still `/tmp` scratch — the claim "not one native write touched a
repo file" is now *better* supported than when published. A wrong number underneath a
strengthened conclusion has nothing anywhere pointing at it.

**Cheapest sufficient check:** state the **property**, not its cardinality — *"every native write
in that session targets scratch"* cannot rot, while *"all eight do"* rots the moment there is a
ninth. Where a count is genuinely wanted, stamp the instant it was taken.

**What this validates:** F-92's prescribed remedy was the correct one, and the discriminator that
picked it was right. Had F-92 been filed under the recording-filter law — as a peer proposed —
the remedy would have been *"instrument the doubt"*, and these two numbers would still be wrong
in a committed file. Widening the audit is a no-op for a Law 2 population and found two real
defects here in one pass, which is the empirical form of the distinction.

## F-94 — New dispersion test pins the gate's existence, not its threshold — 0.8 is unguarded across (0.667, 1.0]

**Observed:** 2026-09-01, reconnaissance on the then-uncommitted dispersion gate in
`scan_cited_prefix_with_no_definer` (`src/librarian/tools/doctor.rs`) — peer session
`codescout-68`'s in-flight change, committed shortly after.

**When:** After all 7 `cited_prefix` tests passed green, before the change was committed.

**Expected:** The new test
`cited_prefix_with_no_definer_suppresses_a_scattered_prefix_but_keeps_a_clustered_one`
guards the dispersion gate it introduces. Its own doc comment argues both directions sit in
one fixture "on purpose", the clustered prefix being "what makes this a discriminator rather
than a control".

**Got:** It guards the gate's **existence** and not its **threshold**. Four mutation runs,
each restored byte-exact from a scratchpad backup (`diff -q`) before the next:

| mutation | result |
|---|---|
| gate disabled (`if false && …`) | **only** the new test fails — it is the sole killer |
| `DISPERSION` 0.8 → 0.5 | `…_fires_above_threshold` + `…_reports_only_the_active_projects_citers` fail; new test **passes** |
| 0.8 → 0.6 | only `…_reports_only_the_active_projects_citers` fails |
| 0.8 → 0.9 | **7/7 green** |

Fixture ratios (files ÷ citations) across the module: `CL-N` 0.33 and `UT-N` 1.00 (the new
test), `T-N` 0.50 (`…_fires_above_threshold`), `ZZ-N` 0.667
(`…_reports_only_the_active_projects_citers`). The shipped 0.8 can therefore be moved
anywhere in **(0.667, 1.0]** with a fully green suite. The only guard between 0.667 and 0.8
is `cited_prefix_reports_only_the_active_projects_citers`, whose stated purpose is **project
scoping** (it is the regression for the 52%-foreign-rows bug) and whose 3-citations/2-files
ratio is incidental to that purpose and unannotated.

**Probable cause:** The test was designed against the two *classes* the comment names —
scattered vs clustered — rather than against the *constant* separating them. Its fixtures sit
at the extremes, 0.33 and 1.00, so every threshold in (0.33, 1.00] satisfies it. The comment
itself flags the cut as "FITTED to that n=14, on one corpus" and explicitly invites re-tuning
against a second corpus; that is precisely the edit no test in the module constrains. The
comment also names the boundary that matters — `GPT-N` (noise) and `CC-N`/`O-N` (real) "sit at
exactly 0.50" — and that *lower* boundary is guarded, but again incidentally, by `T-N`'s 0.50.

**Scope of the claim:** what was established is that no test constrains the constant in
(0.667, 1.0]. I did **not** measure which real-corpus prefixes sit in that window, so this is
not a claim that a specific number would flip — only that a re-tune there changes corpus
behaviour with zero test signal, against a comment whose measured 6-of-8 suppression is the
change's entire justification.

**Workaround:** None needed — the shipped constant is correct and the suite is green. The
exposure is to a future re-tune, or to a tidy-up of the `ZZ-N` fixture that made it read more
like a real ledger (more citations per file), which would keep that test passing while
deleting the last guard on the threshold.

**Severity:** med — nothing defective ships today; the cost lands on the next person to touch
the constant, which the comment invites. It is not `high` because the shipped value is right
and two fixtures do pin the lower half.

**Status:** fixed-verified — closed by `d44a4409`, patch-id
`f37ae73ffe72846ca0cceae19b9649f8c24b6a08`, on `experiments`.

**Verified independently rather than accepted on report.** The patch-id was recomputed here
and matched exactly; the suite was run at that commit in a **detached worktree**, because the
shared checkout did not compile at the time — an unrelated in-flight `librarian_guard`
`Access`-parameter refactor by a third session had the signature changed ahead of its call
sites. 8 passed / 0 failed, including the new
`cited_prefix_dispersion_threshold_is_pinned_from_both_sides` and all seven prior tests. Note
what that worktree also demonstrates: it is `F-95`'s remedy, used for the first time, on the
very entry `F-95` was filed against.

**Closed TIGHTER than filed, and by a mutation axis this entry missed.** The fix pins the
constant with a boundary pair rather than annotating `ZZ-N`: `LO-N` at 4 citations / 3 files
= 0.75 must stay **reported**, `HI-N` at 5 citations / 4 files = 0.80 must be **suppressed**.
Together they admit only **(0.75, 0.80]** — narrower than the (0.667, 1.0] window measured
above. Because `HI-N` sits *exactly* at the boundary it also pins the comparison as `>=`
rather than `>`. My four runs mutated the threshold's **value** and never its **operator**, so
that axis was unguarded here and I did not notice it was missing — a mutation population
selected along one dimension of a two-dimensional gate. Arithmetic re-derived from the
committed fixtures rather than taken from the report: LO `3×5=15 >= 4×4=16` false → reported;
HI `4×5=20 >= 5×4=20` true → suppressed.

**And this entry's own *Fix idea* offered the worse of the two remedies first.** It led with
annotating `ZZ-N`. Once the boundary pair exists, `ZZ-N` is no longer load-bearing for the
threshold — so the annotation would have documented a guard at the moment it stopped being
one, which is the same defect class this entry reported. Putting the guard in the test that
**owns** the constant is strictly better and is the shape to copy.

**Valid:** dated 2026-09-01

**Rests on:** the four mutation runs above, each verified green/red by
`cargo test --lib librarian::tools::doctor::tests::cited_prefix` and each preceded by a
byte-exact restore; and the fixture ratios read from the test bodies themselves, not inferred
— the gate-disabled run printed the real counts (`CL-N` 6 cites / 2 files, `UT-N` 4 / 4),
confirming the new test's doc-comment arithmetic at runtime.

**Fix idea / Pointer:** One annotation line on the `ZZ-N` fixture — CLAUDE.md § *Testing
Discipline*, "annotate a fixture's load-bearing detail, on the fixture line" — saying its
ratio is the last guard on `DISPERSION_NUM`/`DISPERSION_DEN`. Or a boundary case in the new
test at ~0.75/0.85, which pins the constant directly and needs no cross-test annotation.
This is an instance of `cluster/guard-narrower-than-its-name` (`IC-14`) turned on a test:
the guard's name says "suppresses a scattered prefix", and it does — for every threshold in
a range 2× wider than the one measured.

## F-95 — Mutation testing in the shared checkout poisoned a peer's staged index with a disabled gate

**Observed:** 2026-09-01, while running the mutation experiments recorded in `F-94` against
an uncommitted change in the shared `experiments` checkout.

**When:** Between mutation runs. Each run was: `cp` the file back from a scratchpad backup,
`sed` the constants or the condition, `cargo test`. The tree therefore held a deliberately
broken gate for seconds at a time, four times over.

**Expected:** Mutations are local and transient — backup, mutate, test, restore — and nobody
else observes the intermediate states.

**Got:** A peer session (`codescout-68`) was mid-commit on that exact file and ran `git add`
inside one of my windows. The index captured
`if false && by_file.len() * DISPERSION_NUM >= total * DISPERSION_DEN` — my mutation A, a
**disabled** gate. The peer had reviewed the staged diff, but with a filtered grep that
happened not to print the `if` line, so the review passed over it. The only thing that
stopped the commit was the `unreviewed-content` pre-commit hook refusing a pathspec commit
carrying unstaged content. The peer reported observing the constants change under them in
sequence — `5/4` → `if false &&` → `2/1` → `5/3` → `10/9` — which matches my mutation log
exactly.

**Probable cause:** A mutation writes the *same bytes* an intentional edit writes. There is
no channel by which a concurrent reader can tell one from the other, so no amount of care on
either side separates them — the peer's provenance probe (`scripts/file-provenance.py`)
correctly reported `SHARED` and named both session ids, and that is the most any observer
could have got: it can name the collision, never its semantics. This is
`cluster/shared-resource-carries-no-owner` (`IC-17`) with `cluster/blast-radius-exceeds-visibility`
(`IC-1`) on top — I could not see the peer, and the peer could not see that what they were
staging was not authored.

**Workaround:** Restored the file byte-exact from the scratchpad backup (`diff -q` → identical),
then `git add` on the path to overwrite the poisoned index entry. Verified: `git diff` on the
path empty; staged gate line is the enabled form with `NUM=5`/`DEN=4` and no `if false &&`;
`cargo test --lib librarian::tools::doctor::tests::cited_prefix` → 7 passed / 0 failed. Peer
notified with the full account and the tree handed back.

**Severity:** high — a no-op gate was one `git commit` from landing, behind a green test run
**and** a completed human review. It did not ship, but what stopped it was a hook about
*unstaged content*, entirely unrelated to the defect; had the peer staged the whole tree
rather than a pathspec, nothing in the path would have fired.

**Status:** mitigated — the incident is cleaned up and verified, but nothing prevents
recurrence. Any session may mutate any file in this checkout at any time.

**Valid:** dated 2026-09-01

**Rests on:** the peer's own message, which independently reported the sequence of constant
values it observed and named this session id from `scripts/file-provenance.py`; and the
verification commands above, run after the restore rather than assumed.

**Fix idea / Pointer:** Mutation testing must not run in a shared checkout — it belongs in a
git worktree (`superpowers:using-git-worktrees`) or against a committed base, so the shared
tree is never transiently wrong. That is the shape CLAUDE.md § *Observer Blindness* prescribes
— make the correct path end in a safe state, rather than resolving to be careful, because
neither party here was careless. The weaker alternative (announce before mutating) depends on
remembering, and the `.buddy/$SID/recon-active` marker this skill already sets is not enough:
it says a scout is running, not *which files are transiently untrustworthy*. The counterfactual
win belongs to the peer's `unreviewed-content` hook and is theirs to file.

**Refined 2026-09-01 by the change's author, and the refinement is sharper than the remedy
above.** The operative property is not *isolation* but **atomicity**. What made my window
collidable was that it spanned tool calls and therefore turns, leaving a mutant on disk for
minutes at a time. Their own mutation run did backup → mutate → test → restore → `sha256`
verify **inside one shell call**, so the window never crossed a turn boundary and no peer's
`git add` could land inside it. That costs nothing, needs no worktree, and works in a shared
checkout. A worktree still *eliminates* the class where one-call atomicity only collapses it,
so the two are complements rather than rivals — but for an ordinary mutation run, atomicity is
the cheaper correct answer and belongs first. `R-153`'s proposed SKILL.md bullet is updated to
lead with it. Their summary is worth keeping verbatim: *"your `cp`+`sed` approach was right;
what made it collide was that the window spanned turns."*

## F-96 — A peer corrected my mechanism, endorsed my conclusion, and the conclusion was independently false

**Valid:** dated 2026-09-02

**Observed:** I published `core.hooksPath points at scripts/` as the reason a hook edit is
instantly live for every session in this checkout. It is false at both levels. *Level 1 —
the variable:* `core.hooksPath` is **unset** here at every scope (`git config
--show-origin --get-all` → exit 1), which is this repo's healthy state, asserted by
`tests/hook_config.rs::a_set_core_hookspath_must_point_at_a_directory_that_exists`; the
archived instance of a *set* value silently disabled every hook here for a day
(`docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md`). A reader
reasoning forward from my sentence could have set it and reproduced that bug. *Level 2 —
the conclusion, which SURVIVED level 1's correction:* pre-commit clears unstaged changes
before running hooks (`staged_files_only.py:108`), so a `language: system` entry runs the
**index** copy of its own script. Measured in a throwaway repo: unstaged edit → hook ran
the index version; staged edit → hook ran the edit; a further unstaged edit → index version
again. **The exposure moment is `git add`, not the editor save.**

**Cost:** three durable surfaces carrying a dangerous-if-acted-on claim — my archived bug
file, a *peer's* `## Environment` block (which is where I got it), and commit `054c8a3e`'s
message, which is immutable on a shared branch. Two sessions propagated it before anyone
measured. Zero code impact; the developed-against-a-copy discipline was right for the wrong
reason, so nothing shipped badly.

**Category:** epistemic / doc-vs-substrate drift · **Severity:** med

**Two laws, and the second is the one this corpus does not yet state.**

**(a) An `## Environment` block is the least-audited claim in a bug file.** It reads as
background rather than as an assertion, so no reviewer treats it as checkable — and it is
the block downstream files *copy*, which is exactly how this travelled from a peer's file
into mine and then into a commit message. Every other section of a bug file makes a claim
someone argues with; this one makes claims nobody does. It is `R-104`'s prohibition form
one level down: not "X would be unsafe" but "X is simply how things are here."

**(b) Correcting a premise does not correct what was built on it — and the built thing
inherits the confidence of the correction.** `codescout-3e` corrected my mechanism
precisely and endorsed my conclusion in the same message ("Same instant-live property,
different path"); I accepted both, because a peer who has just caught me in an error reads
as *more* reliable, not less. The conclusion was independently false, for a reason neither
of us had looked up. **A correction is a fresh claim and gets a fresh check — including the
part of the original it leaves standing.** Note the shape: the two halves of the refutation
were *both already filed in this repo* (the `hooksPath` test, and bug `8cc95806a7b5f37a`
naming `staged_files_only.py:108` as a root cause the day before), held one each by two
sessions who agreed with each other instead of composing them. Agreement between parties
reading the same wrong premise is one blind spot counted twice — CLAUDE.md § *Observer
Blindness* states that for instruments; this is the same failure between **agents**.

**Rests on:** `git config --show-origin --get-all core.hooksPath` → exit 1 (2026-09-02);
`scratchpad/probe-hooklive.sh`, three cases, isolated throwaway repo;
`tests/hook_config.rs`; bug `8cc95806a7b5f37a` § Root cause.

**Status:** fixed-verified — both editable surfaces corrected in place (the archive file
and the peer's file, the latter flagged to its author); `054c8a3e`'s message is immutable
and is left standing, cited here as the third instance.

## F-97 — Reported `cargo check` green under the project gate's name while the clippy step was red

**Observed:** 2026-09-02 ~09:47, verifying tree state after a peer landed `e701ec59` on a shared checkout with seven live sessions.

**When:** Reporting tree state to the operator and to peers, immediately before others would budget work against it.

**Expected (what I published):** "All green" — a status table listing `cargo check --workspace --all-targets` (exit 0), `cargo test --test issue_clusters` (18/18), `scripts/pre-commit-ledger-counts.py` (exit 0), and git status.

**Got (reality):** The project gate's clippy line — `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` — was **red, exit 101, 10 error lines**. Seven `dead_code` warnings in `src/engines/` that I had seen, read, and explicitly described as *"only dead-code warnings, expected for a module not yet wired"* were each promoted to an error by `-D warnings`. `cargo check` does not lint at all, so it cannot express the question clippy answers.

**Probable cause:** Instrument substituted for speed, then reported under the gate's name. `cargo check` was chosen because it was fast and because a compile error was the hypothesis being tested; the result was then generalised to "the gate is green". CLAUDE.md § *Development Commands* warns about exactly one step up from this — *"the long clippy form is the gate, not garnish: bare `cargo clippy -- -D warnings` lints only the root package's non-test targets with default features, so it passes trees CI fails."* I ran something weaker than the weak form.

**The tell was available and unused:** I quoted the gate's four commands verbatim in my own first message of the session and then ran none of them for four hours. A green from an instrument chosen for its speed is a fact about that instrument.

**Workaround / correction:** Ran the real gate line, found the failures **confined** to `src/engines/` (zero errors elsewhere — the null result is what established single ownership), and republished the state with clippy listed as RED and attributed. Caught by `codescout-26`, not by me.

**Severity:** med — published a wrong tree-state to an operator and to six peer sessions, any of whom could have run the four-command gate, hit a red they did not cause, and spent time attributing it. No code was wrong; the *report* was. Cost one correction round-trip plus a full clippy run to re-derive.

**Status:** fixed-verified — corrected in the same session, gate line re-run, scope of failures established by null result.

**Valid:** dated 2026-09-02

True of this session's reporting, not of the code. Re-check if the gate's four commands change.

**Rests on:** CLAUDE.md § *Development Commands* — the four commands are four because each answers a question the others cannot, and `check` answers none of clippy's.

**Attribution correction, 2026-09-02 — and it is NOT a rename.** This entry credits the catch to
`codescout-26`. That name does not resolve to a session, and the honest repair is to say so rather
than substitute a better guess. Two candidates: `codescout-17` (sessionId
`9716a130-c93d-4a65-9ab2-ddc53d6d9cfb`) has since stated it signed an earlier message under that
name, and a real `codescout-26` also existed on `.claude-kat` and has since exited. The `from=`
socket that would have decided it is not recoverable from this session's context.

So the record stands as written — it is accurate about *what the signature said* — with the
attribution marked **unresolvable**. Closing it by picking the likelier candidate would be
elimination over a population of two, which is the thing `CLAUDE.md` § *Observer Blindness*
forbids, inside the entry that would be citing it.

The general form is filed as
`docs/issues/archive/2026-09-02-a-sessions-self-reported-name-is-not-self-verifiable.md`: a name is minted
into a per-profile registry and re-minted by compaction, resume, or a restart under another
profile, so a session reporting its own name quotes a belief rather than reading a fact. Only the
**sessionId** is structural. `CLAUDE.md` now says so in both places that prescribe asking.

**Fix idea / Pointer:** Before writing "green" anywhere a reader will act on, name which command produced it. A status table listing three commands and omitting the fourth reads as complete; the omission is invisible to the reader and to the author.

## F-98 — A correctly-scoped retraction left the same claim standing in a second artifact, labelled verified

**Observed:** 2026-09-02, after committing `8343d6ca` to retract a falsified remedy from `docs/issues/2026-09-02-one-ledger-file-serializes-every-class-edit.md`.

**When:** Believing the retraction complete and reporting it as such.

**Expected:** `8343d6ca` withdrew the index-free union count (`tracked ∪ git ls-files --others --exclude-standard`) after `codescout-20` falsified it at `tests/issue_clusters.rs:511-527` — `actual_counts` iterates `tracked_all_bug_files()`, so the field carries the **tracked** count and a union reds `every_bare_n_in_a_class_field_matches_the_corpus`. The commit was scoped **"bug file only, no counts moved"**, which was correct and deliberate.

**Got:** The same claim was still standing in a *second* artifact — `docs/trackers/issue-clusters.md:1139`, IC-17's `**Members:**` field, written by the same session at `cd6bb36c`, reading *"...returns the correct corpus without touching the index, **verified 2026-09-02**..."*. A reader arriving at IC-17 gets the falsified remedy asserted as verified, with no marker, and the retraction lives in a file they have no reason to open.

**Probable cause:** The correct scoping is what preserved the falsehood. Every rule in this repo says to scope a commit to what it changes and not to touch a shared unsplittable ledger without cause — and following all of them left the claim in the artifact that indexes the class the claim belongs to. Nothing relates a bug file to a ledger field quoting it: no edge, no diff hunk, no check names the section a later edit retired.

**This is worse than the unsafe instruction it sat next to.** `issue-clusters.md:872`'s *"`git add` first, then count"* is **advice that misfires** on a shared index. `:1139`'s tail was **false and labelled verified**, in the Index-bearing artifact.

**How it was found — and the near-miss is the point.** `codescout-05` found it. I had told it, accurately, that `:1139` was a *mention* of the instruction rather than a use, and that it should **not** amend it. That was true and it was **incomplete in a way that would have closed the search**: it read the whole field instead of stopping at the line I had characterised, and found the survivor three sentences later. An accurate, incomplete correction carries enough authority to stop the next person looking.

**Workaround / fix:** `fea2e1ce` — the sentence now opens *"not by the union — that prescription was FALSIFIED at `8343d6ca`, and this sentence asserted it as verified until..."*, cites the mechanism rather than a verdict, records why the union measured clean, and marks the surviving read-side finding as surviving. Prose only, no count moved.

**Severity:** high — a false claim labelled verified, in the artifact that indexes `cluster/doc-contradicted-by-code`, which is the class it instantiates. Would have propagated to any session reading IC-17 for the corrected count rule.

**Status:** fixed-verified — `fea2e1ce`, patch-id `d9a6dea264169f3e31d27adb3323b734be98afd5`; grep for the claim returns 0; gate 18/18.

**Valid:** invariant

The mechanism does not decay: two artifacts can carry one claim, and retracting it in either leaves the other standing, because nothing in markdown relates them.

**Rests on:** `observer-blindness:OB-12` — a section a later one falsifies produces no diff hunk, no broken link, and no check naming what it retired. This is OB-12 across two *files* rather than within one.

**Fix idea / Pointer:** Before scoping a retraction, grep the claim's distinctive phrase across `docs/` — not the file you are editing. If the claim was ever quoted into a tracker, ledger, or guide, the retraction owes a second commit, and the correctly-scoped first one will not schedule it.

## W-94 — Peers with a differing SAMPLE caught five errors no author caught, including two already committed

**Observed:** 2026-09-02, across a ~5h session with seven live sessions in one checkout (six peers plus me, by socket enumeration — `ListAgents` reports a per-profile subset).

**Pattern:** When a claim will be acted on, get it checked by a peer whose **sample differs** — different scope, different sample *time*, or different instrument. Not a more careful reviewer; a differently-positioned one. Then state the scope and unit of what you checked, so the peer can tell whether their sample is independent of yours.

**Counterfactual — five concrete instances, none caught by its author:**

1. **Stale compiler sample.** I ran `cargo check`, got `E0603` at `src/engines/coordinator.rs:9`, and told the owning session *"the fix is one line — the compiler hands it over."* `codescout-20` had sampled the same tree ~1 min earlier and got `E0425`, two types not found — **no overlap**. A subagent was writing the file between our samples. Without the cross-check, `codescout-26` applies a one-line `use` fix, is still red, and goes hunting a phantom second defect. (Both diagnoses were already stale on arrival; 26 had fixed it independently.)

2. **Substituted instrument.** `F-97` — I reported `cargo check` green as "all green" while the gate's clippy line was red with 10 errors. Caught by `codescout-26`, which held the failing code and knew the gate line applied to it.

3. **Falsified remedy, already committed.** I verified an index-free union count and shipped it in `4266be0f`. `codescout-20` falsified it at `tests/issue_clusters.rs:511-527`. Without it, that bug file would carry advice that **reds the ledger gate for anyone who follows it**.

4. **A retraction that missed a second artifact.** `F-98` — found by `codescout-05` reading past the line I had (accurately) told it not to amend.

5. **Shared blind spot reported as corroboration, twice.** I offered "catalog query and filesystem grep agree at 43" as confirmation — both read the worktree, so they agree *because* they share a property. Then did it again 90 minutes later, offering `grep -r` = union as corroboration — both answer the on-disk question. I had described this failure, published it, and named CLAUDE.md's independence rule, between the two instances.

**Confirming data points:** the five above, plus three misattributions of a single staging event (`20` → `05` → actually `26`'s subagent), each by a session mid-sentence about not routing by adjacency.

**Impact:** high — item 3 alone would have shipped actively harmful advice into a load-bearing ledger; item 1 would have cost a peer a debugging detour on a build blocking seven sessions.

**The strongest evidence is who supplied the corrections.** Every one came from the party who would have *benefited* from the error standing: `codescout-05` refusing an over-attribution in its own favour, then retracting its own proposed remedy; `codescout-20` diffing and withdrawing a claim inside its own commit message; `codescout-26` recording its subagent's mistake as its own and naming the rule it broke.

**Promote-when:** already at threshold by instance count, but the promotion is **not** "get peer review" — that is a policy, and item 5 shows a policy does not survive contact with an author who has just written it down. The mechanizable form is the one to promote: **make a claim state the scope and unit it was measured over**, so a reader can tell whether a second instrument is independent. Two of the five above are pure unit/scope omissions.

**Status:** validated — five independent instances in one session, each with a named cost and a named catcher.

**Valid:** dated 2026-09-02

Instance count is a fact about this session. The mechanism claim — that differing *sample* rather than greater *care* is what caught these — rests on all five having been missed by careful authors.

**Rests on:** CLAUDE.md § *Observer Blindness* position 2 (the reviewer who does not share the author's context) and its independence rule (two instruments agreeing is evidence only if their scopes differ).

**Fix idea / Pointer:** The cheap version, available in advance: before citing a confirming result, ask what property the two instruments share. If they share the one that would produce the error, their agreement is one blind spot counted twice.

## W-95 — Compare the running binary's mtime to the fix's commit time before filing against your own tool

**Observed:** 2026-09-02, working a 5-item `doctor` worklist in a repo whose MCP server *is* the binary under development.

**Pattern:** Before filing a bug against a tool this repo builds, compare the running binary's mtime to the commit time of any fix that would explain the finding. Two commands, no build:

```
stat -c '%y' target/debug/codescout          # 11:05:41
git log -1 --format='%cd' <suspect-fix-sha>  # 11:08:02
```

If the binary predates the fix, the finding is a **stale-instrument artifact**, not a defect.

**Counterfactual — concrete, and it was three minutes wide.** `doctor` reported 5 bug files as `non_terminal_status_with_fix_anchor`. Four had their patch-id under `## References` / `## Symptom` / a sub-section rather than `## Fix`. Reading the check at HEAD showed it scans `declared_patch_ids(&fix_section_body(&content))` — Fix-section only — which is exactly `36cb17ed`, *"a patch-id outside a Fix section is a citation, not an anchor"*. That landed at **11:08:02**; the running binary's mtime was **11:05:41**. **The report was 141 seconds older than its own fix.** Without the mtime check I would have filed a bug report against an already-closed bug, citing four files as evidence — and the report would have been internally consistent, reproducible on my binary, and wrong.

**Confirmed by rebuild, and the confirmation discriminates.** After `cargo rb` (binary mtime 11:29:19, now newer than the fix) the same check reports **0**. That zero is not merely agreeable: the fifth file's patch-id genuinely *is* under `## Fix`, so it would still fire unless the `unverified:` discharge (`dde26886`) had also worked. One hypothesis failing would have shown as 1, not 0. Publishing the confirmation rather than absorbing it, per CLAUDE.md § *Testing Discipline* — a re-derivation that confirms is a denominator.

**Why this is a mechanism and not a policy:** it does not require noticing anything. The trigger is "about to file against a tool this repo builds", which is a category you already know you are in, and the check is two commands with no build step. Contrast the failure it prevents, which is invisible from the inside — a stale binary produces a *complete, self-consistent, reproducible* report.

**Scope:** any repo where the agent's own tooling is the artifact under development. Here that is every codescout MCP tool. The same shape reaches `cargo rb` + `/mcp` generally: a tool's behaviour is a fact about the *running build*, never about `HEAD`.

**Impact:** high — prevents a wrong bug report against a fixed defect, which costs the fixer's time and pollutes a triage query this repo treats as load-bearing.

**Promote-when:** a second instance where a rebuild changes a tool's *reported findings* (not just its features). At two datapoints this belongs in CLAUDE.md beside the `cargo rb` + `/mcp` instruction, which today says how to rebuild and not when a stale build invalidates a measurement.

**Status:** validated — one instance, caught before filing, confirmed by rebuild with a discriminating zero.

**Valid:** dated 2026-09-02

The 141-second margin is an accident of this session. The mechanism does not depend on it: any margin at all produces the same class of wrong report.

**Rests on:** `F-97` (an instrument's result reported under another instrument's name) and `W-94` — this is the same family with the *time* axis rather than the scope axis, and unlike those it has a cheap standing check.

## W-96 — `-z` and NUL-splitting travel together or neither travels — the hardening flag is the hazard

**Valid:** dated 2026-09-02

**Observed:** A zero-byte untracked file named `head.\naab0c4ef'"s` — a newline plus *both* quote
characters, bytes `686561642e0a6161623063346566272273` — appeared in the repo root at 11:29:22,
the signature of a `>` redirect firing at a target assembled from prose. Two hook scripts had just
been changed underneath this session (`689ceffb`), and both parse staged paths line-oriented and
tab-delimited. Filing against them was the obvious move.

**It was wrong, and so was the obvious remedy.** Reproduced in a throwaway repo — never the shared
index, which six sessions were using:

- **The hooks are safe.** `core.quotePath` defaults to *true*, so git C-quotes such a path and
  `git diff --cached --raw` emits it on ONE tab-delimited line. Both scripts' `awk -F'\t'` plus
  `IFS=$'\t' read -r` parse **2 rows for 2 staged files, path intact**. The check can express the
  failure it did not find — a broken parser yields 3 rows or a truncated field — so this is
  evidence, not the silence a never-reached guard also produces.
- **The hardening flag is the hazard.** On `git ls-files --others --exclude-standard`:
  `| wc -l` → **2** (correct); `-z | wc -l` → **1** (wrong); `-z` piped to Python `.splitlines()` →
  count **2**, the *right number* with the *wrong rows* (`head.` and `aab0c4ef'"s\0normal.txt\0`).
  One `-z` input, two line-splitters, two different failures — one loud in the count, one silent in
  the count and garbage in the contents. The un-hardened default is the safe one.
- **No newline is required to reach this.** `core.quotePath` quotes any non-ASCII byte, so an
  em-dash or an accent in a bug-file slug is enough: such a path is counted correctly and then
  cannot be opened — `git show :"docs/issues/…\342\200\224….md"` → rc=128, `ambiguous argument`.

**Exposure today is zero, and that is stated rather than implied.** Nothing in `scripts/`,
`tests/*.sh`, `.pre-commit-config.yaml` or `src/` pairs `-z` with a line splitter, and
`git ls-files | grep -c '^"'` is **0** tracked paths. Latent, not live. Deliberately *untested*:
`_prime_index`'s `cat-file --batch` path (`scripts/pre-commit-ledger-counts.py:45`) — the empty
population offered nothing to test it against, so it stays a thing to check, not a thing concluded.

**The promotable rule:** `-z` and NUL-splitting travel together or neither travels. Adding `-z` to
a consumer that splits on lines converts a correct instrument into a wrong one, which makes
*"harden it with `-z`"* the specific regression to refuse in review of any path enumerator.

**Counterfactual.** Both live moves were wrong without the repro: file a parser bug against
`689ceffb`, whose parser is fine; or harden the enumerators with `-z`, which is the actual defect.
Cost was one throwaway repo and no contact with the shared index.

**And the subject expired mid-investigation** — the file was gone some six minutes later, cleaned
up by whoever made it, and `git ls-files --others` correctly stopped reporting it. A grep of mine
returned `(none)` and I nearly read that as a grep bug rather than a changed world. On a shared
checkout a working-tree observation is a **sample, not a fact**; this finding survived only because
it had already been reproduced somewhere the peers could not reach.

This is `IC-6`'s no-escape half — a namespace whose tokens the enumerator cannot represent — with
the twist that the documented hardening *inverts* the failure instead of removing it.

## F-99 — a spec's mechanism was inferred from where the grep window stopped, one file short of the dedup that made it false

**Severity:** high — the spec was committed (`ee561448`) and was one step from `writing-plans`, which would have handed the false mechanism to an implementer verbatim.

**Status:** fixed-verified — spec corrected in place; correction annotated rather than silently applied.

**Valid:** dated 2026-09-02

**Observed.** A design spec for a new `doctor` check asserted that a same-file duplicate
entry definition is invisible because `link_scan`'s resolver short-circuits: *"a same-file
duplicate pushes two `DefinerRef`s carrying the same `artifact_id`, so an entry citing a
duplicated sibling returns `SelfCite`"*. A pre-dispatch scout of
`src/librarian/tools/link_scan/extract.rs` found it pushes **one**, not two.

**The actual mechanism, one layer earlier.** `extract.rs:399` and `:440` both guard on a
shared `seen_defs` set:

```rust
if seen_defs.insert(token.clone()) {
    out.definitions.push(Definition { token, line });
}
```

`BTreeSet::insert` returns `false` for a token already present, so the **second**
`## R-147 — <title>` heading in a file is discarded at parse time. `DocExtract.definitions`
cannot *represent* the state, and `DefinitionIndex` — built by iterating that vector —
records one `DefinerRef`.

**Why this is `high` and not `med`.** The conclusion the spec drew (*this state is invisible*)
was **correct**, and correct for a stronger reason than the one written. That is the
dangerous shape: an implementer re-reading the spec finds a true claim supported by a false
mechanism, and the natural implementation — read `ex.definitions`, look for repeats — yields
a check **that can never fire on any input**. Not a check that fires wrongly; one whose
positive case is unrepresentable, so its own test would be red for a reason pointing at the
test rather than at the design. The spec now names the correct data source explicitly:
`librarian::preview::headings::parse(text)` plus `extract`'s `def_re()` shape, which is
`extract.rs:433-444` minus the dedup.

**How it got in.** The claim was assembled from a `grep` with `context_lines=8` over
`resolve.rs`. The context window showed the resolver's short-circuit and did not reach the
extractor two files upstream, so the mechanism was *inferred from where the search happened
to stop*. A grep window is a fact about a query, never about a pipeline — and the resulting
sentence read as verified because it quoted real code at real line numbers.

**Rests on:** `CLAUDE.md` § *Testing Discipline* (a test cannot detect what its recording
filters out — here the extractor is the filter, and the check would have been the test) and
the reconnaissance skill's Phase-3 rule, *name the proposition a confirming result proves*.
The quoted `resolve.rs` snippet was real and proved a real thing; it did not prove the thing
the sentence beside it claimed.

**Counterfactual.** Without the scout the spec goes to `writing-plans` unchanged. The
implementer builds Component A on `DocExtract.definitions`, the seeded two-heading fixture
produces one definition, and the task fails at test time with a symptom (`expected 1 finding,
got 0`) that points at the fixture. Recovery cost is a task cycle plus a redesign of the
check's data source — the part the spec now states outright.

## F-100 — A window is a fact about the query — re-run once with it removed, never widened

**Valid:** dated 2026-09-02

**Observed:** Four instances in one evening, across four sessions, of an instrument whose
**window silently excluded the refuting evidence** — each returning a clean, self-consistent,
*wrong* answer rather than an error. Severity `high`: one was three debugging rounds, one was a
correction of a peer that would have been published wrong, and none was caught by suspicion.

| session | instrument | window | what it hid |
|---|---|---|---|
| this one | `git show \| cut -c1-150` | columns | **+1,508 chars** of hand-authored prose past col 150 |
| `codescout-17` | a `git grep` scan | files | the dedup that falsified its own spec's mechanism |
| `codescout-e4` | `grep` with no `--include` | file types | (avoided — it deliberately dropped the filter) |
| this one | `grep` over stdout | **streams** | an argparse exit-2 written to stderr |

**The fourth is the sharpest and it happened AFTER I had written to two peers about the first.**
`probe-cluster-census.py --source=head` kept printing nothing. I verified the fixture; verified the
counts genuinely diverged (`worktree=2`, `head=1`); read the code and found it correct; discovered
my test harness was `cp`ing the new probe in and then running `git checkout -- .` over it; fixed
that — **still silent**. The actual cause: `head` was absent from argparse's `choices`, so every
run exited 2 with the message on **stderr**, which every one of my `grep` filters discarded. The
detector worked the whole time. Three rounds spent debugging working code, by someone who had
just published the rule.

**Why "be careful with filters" is not the remedy.** It is the remedy that failed four times in
one evening, twice inside a correction of itself. The window is chosen *for* a good reason — a
150-column cut makes a diff readable, a `grep` makes a monitor selective — and the cost is paid
by an observation the chooser has no reason to expect. `codescout-17` put it best and it is worth
quoting rather than paraphrasing: **a window is a fact about the query, never about the thing.**

**The mechanism, which is cheap and needs noticing nothing.** When an instrument returns a result
you are about to act on, re-run it **once with the window removed** — no `cut`, no `--include`,
`2>&1` instead of `2>/dev/null`. Not "widen it": *remove* it, because a wider window is another
window. That is one command, and it is the step that caught all three of tonight's diagnosed
instances; none was caught by doubting the answer.

**Its own denominator, published rather than absorbed.** The re-run *confirmed* on the sweep
transform (22 index cells, 31 bare `n=`, no cross-class contamination) — a case where the window
was fine. Recording only the catches would make the population look self-correcting, which is
CLAUDE.md § *Testing Discipline*'s recording-filter law, and is the same law as the entry itself
one level up: **this entry is an instrument whose window would exclude its own negative cases.**

**Rests on** `W-94` (a peer whose sample differs) and `W-95` (the binary's mtime). Same family —
those two are about a *stale* sample and this is about a *truncated* one — and unlike `W-94`'s
"get peer review", which is a policy that failed against an author who had just written it down,
this one is a command.

### Amendment, same day — three more, and the last is the entry failing at itself

Two **offered** by `codescout-00`/`cc` (sessionId `953b5e77`) and recorded as offered rather than
absorbed as catches, since counting a volunteered datapoint as a catch is what makes a population
look self-correcting: (5) a compound `run_command` where one clause named a source file, so the
guard refused the **whole string** and its `git reset` silently never ran — caught by checking
`git status` rather than the exit code; (6) it read the stage log, saw its own sessionId on all
ten rows, and a later failed commit **rewrote those rows to `(unrecorded)` underneath it**, so a
correct reading expired with no signal.

**(7) is mine and it is why this amendment exists.** Forty minutes after writing the entry above —
whose fourth instance *is* an argparse `choices` list rejecting `head` on stderr — I found the
identical defect in the sibling file: `scripts/pre-commit-ledger-counts.py` used `"head"`
internally and rejected it at the door (`--source must be index|worktree`). I had fixed **the
instance and not the class**, and it cost a confusing `exit=1` against `HEAD` that read as my own
new gate having a false positive.

So the remedy above is insufficient as written. *"Re-run once with the window removed"* finds the
instance in front of you and says nothing about the identical window in the file next door. The
repair is CLAUDE.md § *Testing Discipline*'s **per-SITE** rule arriving from the measurement side:
having found one, grep the sibling implementations for the same construct before closing. Here
that was one command — `grep -n 'choices=\|not in ("' scripts/*.py` — and it returns both.

**A second thing agreement does not establish**, from `cc`, which falsified a claim this session's
own shipped code was implicitly making. `probe-cluster-census.py` reports when its three sources
disagree, so silence read as *"trustworthy"*. It is not: a **coupled change split across the commit
boundary** leaves `HEAD` internally inconsistent, and all three sources then agree on a number that
is wrong — measured, a ~90-minute window where a budget raise had landed and the bytes justifying
it had not. Agreement is evidence about the **trees**, never about the change. The probe now says
so in the silent case, which is the only case where a reader forms the belief.

**A candidate arrived later, and it is recorded as a candidate — the identification stays lost.**
This entry's own instance included a default-lane run reporting `4957 passed; 1 failed` whose test
name my `| grep` filter discarded. `codescout-0d` reports (denominator: **8 isolated re-runs, 8
green**) that `peer::server::tests::run_exits_after_idle_timeout_with_no_connections` is a known
load-sensitive flake, filed at
`docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`. It sits in the
same target (`-p codescout --lib`), and my run was concurrent with three peer builds and logged
`Blocking waiting for file lock`.

**Consistent is not identified.** Every one of those facts holds equally if the failure was
something else in that target. The one artifact that would decide it — the name line — was
*destroyed* by the filter, not merely unread, and that is this entry's own law: a later fact cannot
reconstitute an observation the recording never made. Calling it "the flake" would be elimination
over an incomplete population, which this session has refused three times tonight on smaller
provocation. Strongest available candidate; nothing more.

## W-97 — When a surface is removed, repair the POINTERS and leave the RECORDS

**Valid:** dated 2026-09-02

**The cut:** when a change removes a surface, sweep for references to it and sort each into
**pointer** — prose telling a reader to go consult that surface — or **record** — prose narrating
what once happened. Repair the pointers. **Leave the records**, because deleting one to satisfy a
sweep falsifies the evidence that the surface ever existed.

**Measured 2026-09-02** on the removal of `issue-clusters.md`'s `n` column: **14** surviving
references corpus-wide, **13 records, 1 pointer**. The records are quoted panic output in two bug
files, past-tense incident narration in `claim-decay.md:401` and `issue-clusters.md:493`, and the
removal commit's own "the column is gone" notes. A linter that flagged all 14 would have been
right 1 time in 14 and destructive the other 13.

The one pointer was a live section banner in `tests/issue_clusters.rs` describing a check the
removal deleted, repaired at `6070ae75`. A second, in `issue-clusters.md:385`, was found and
repaired independently by `codescout-0d` at `005dd9e6` — and its shape is the sharper one: **that
sentence was itself the remedy for a stale claim**, so a fix outlived its own surface, with nothing
pointing backwards from the removal commit. No diff hunk in that paragraph, no broken link, no
check.

**The mechanism, which is the point — the discriminator above still needs a human, this does not.**
Every script path named in the files you touched, statted:

```
grep -ohE 'scripts/[a-z_-]+\.(py|sh)' <touched files> | sort -u    # then stat each
```

It caught a dangling pointer **inside the commit that was fixing dangling pointers** — the banner's
first draft cited `scripts/probe-cluster-express.py`, which does not exist — under a minute after
writing it, by an author who could not have been more primed. Priming did not help; the check did.

**Its denominator, published because a check mentioned only when it fires has none.** `codescout-0d`
ran the same check across its own six touched files plus `docs/conventions/*.md`: **10 paths, 9
resolve, 1 the known `scripts/does-not-exist.sh` fixture**, ~10 seconds, no defect found. That null
result is the reason the catch above is worth anything. Both runs flagged the same fixture — a
quoted probe result whose whole point is that the path is absent — and both verified it rather than
waving it through, which is the step "obviously a fixture" skips.

**Offered by `codescout-0d`, recorded as offered:** it filed a bug whose `## Root cause`
confidently named the wrong mechanism (`git commit -- <paths>` staging into the real index), having
just written the reproduction, and learned otherwise only because a peer's unrelated inference was
cheap to test. The true cause is a temporary `GIT_INDEX_FILE` during partial-commit hooks. Same
shape one level down: maximally primed, still wrong, caught by something mechanical run by someone
not looking for it.

**Where it stops being mechanisable, stated rather than papered over.** `0d`'s proposed twin — a
check that a bug file's `## Root cause` has an executed reproduction attached — has no
implementation either of us can see, and it left it there rather than inventing one. That is the
honest boundary: the *pointer/record* cut needs a reader, the *path resolves* check does not, and
the *claim was tested* check is currently neither.

**Correction, five minutes later, by dogfooding this entry against its own check.** Above I called
the path check *"the part that needs no judgement"*. **That is false, and the entry's own commit
falsified it.** Run over this entry it flagged **three** paths and found **zero** defects:

| flagged | what it actually is |
|---|---|
| `scripts/does-not-exist.sh` | a quoted probe result whose point is that the path is absent |
| `scripts/probe-cluster-express.py` | this entry *quoting the typo* as its worked example |
| `scripts/reload.py` | **buddy 0.9.1's** script, correctly qualified in prose as such |

The second is a *record* in exactly this entry's sense — quoting a bad path in order to say it is
bad. The third is a cross-repo citation the grep reads as local, because `scripts/` is assumed to
mean *this* repo's.

So the check is a **candidate generator, not a linter**, and every hit needs the pointer/record cut
applied to it. What it removes is the *noticing*, not the *judging* — it guarantees you look at
every path, and it took ~10 seconds to produce three candidates a reader then resolved in under a
minute. That is still worth having; it is just not the clean division of labour claimed above, and
the corrected split is: the machine enumerates, the reader classifies, and neither half works alone.

`codescout-0d`'s null run (10 paths, 9 resolve, 1 fixture) reads differently in this light too —
its "1 fixture" was a *classification it performed*, not a pass the check gave it.

**And one step deeper, traced by `0d` after the above was written.** Its classification of that
fixture was made by **trusting an earlier message of mine saying I had verified it**, not by
tracing it. So the run was machine-enumerates / reader-classifies exactly as corrected — with the
reader partly deferring to *another reader*. The denominator is unaffected (10 enumerated, 1
needing judgement); what moved is that its "known" was doing work nobody had done at that moment.
The fixture is real — `issue-clusters.md:701` quotes `Executable scripts/does-not-exist.sh not
found` to argue that a `language: system` hook naming a missing script fails **loud**, i.e. a
record in this entry's own sense — but it was *established* one message later than it was
*asserted*. Deference is the cheapest form of the recording filter: it produces a confident
classification with no observation behind it, and looks identical to one with.

### Three tiers of mechanisability — the ordering is the claim

| | check | who does what | moved tonight? |
|---|---|---|---|
| 1 | path exists | machine **enumerates**, reader **classifies** | **yes** |
| 2 | pointer vs record | reader classifies, **no enumerator** | no |
| 3 | claim was tested | **neither** | no |

Tier 1's enumerator is what removes **the noticing**, and the noticing is the scarce resource:
`probe-cluster-express.py` was caught inside a minute by a maximally-primed author, while `0d`'s
wrong `## Root cause` survived a commit precisely because nothing enumerated it.

**Tier 3 may be genuinely unmechanisable rather than merely unbuilt, and that is worth stating so
it stops reading as a gap someone will fill.** `0d`'s proposal — require a `## Reproduction` block
to carry an executed command with its real output — founders on *real*: nothing distinguishes a
pasted transcript from an invented one, so a check that accepts a plausible-looking block is
`cluster/assertion-satisfiable-by-accident`. A tier-3 check that cannot fail on a fabricated
reproduction is not a weak check; it is the class this repo already tracks.

**Promotion candidate, not promoted:** if the pointer/record cut recurs on a third removal it earns
a place in `docs/conventions/`. Two instances (this one, `0d`'s) are not a rule yet — and the
correction above is an argument for waiting, since the version that would have been promoted five
minutes ago carried a false claim about which half needs a reader.


## F-101 — The obstacle I was documenting was the answer — two wrong conclusions in five minutes

**Valid:** dated 2026-09-02

**Observed:** a `/mcp` reconnect puts the session in a state a filed bug named as **unowned and
untested** — the cleared activation slot. Probing it there closed the question in two calls. Along
the way I drafted **two** confident wrong answers in under five minutes, each falsified by the next
observation rather than by doubt.

**The result.** Unpinned `symbols` with a cleared slot returned **home's answer** (no error, no
zero); a write immediately after was **refused** for the slot still being cleared. So the read
resolved against home *and did not activate it*: **the read path has a silent home default that
never sets the slot.** The write path requires an explicit slot; the read path has a default that
never creates one. That is why a worktree read is silently wrong rather than loudly blocked — there
is nothing for a guard to check, because the read is behaving as designed.

**Wrong answer 1: "a cleared slot silently auto-activates home."** Drafted off the
`project-activation-bootstrap` guide arriving with the read, which reads exactly like *"you just
activated a project"*. Falsified by the guide that arrived one call later:
`get_guide("workspace-state")` states that **server construction re-arms the session-opening topic
alone on any non-empty reloaded ledger, so that body is re-sent on every `/mcp` reconnect
regardless of project.** The most legible signal in the response was evidence of the reconnect and
of nothing else.

**Wrong answer 2, and it is the instructive one: "the two probes interfere, so the state is
one-shot per reconnect."** I had already written the table explaining why the experiment was hard —
read tells you nothing, write destroys the condition — and it was wrong for the same reason the
real answer is right. **The probes do not interfere, precisely because the read does not touch the
slot.** The obstacle I was documenting *was the answer*. Had I stopped at "this is hard and needs
two reconnects", the file would carry a plausible, well-argued reason not to run the experiment
that was one call from completing.

**The transferable shape: when you catch yourself explaining why a measurement is impractical,
check whether the obstacle is the result.** A mechanism that blocks the probe is a fact about the
system, and it is usually the same fact the probe was after. That is cheaper than the two-reconnect
protocol I was about to prescribe, and it needs noticing rather than tooling — so it is filed
`open` with no mechanism, honestly.

**Cost, and why `med` rather than `low`:** neither wrong answer reached a peer or a commit, because
the next tool call landed before the writeup did. That is luck, not method — both were fully drafted
and one was in an `edit_markdown` payload that only failed because the *write path* refused for an
unrelated reason. The friction is that the refutation arrived by accident of ordering.

**Rests on** `F-100` (a window is a fact about the query) — same family, different axis: that one is
an instrument excluding evidence, this one is an instrument's *most legible output* being about
something else entirely. And `W-95`: the running binary here predates HEAD by **29 seconds**,
checked first, and was irrelevant only because the intervening commit touched zero code files.

## F-102 — five assertions satisfied by text that survives deleting what they name — the fifth in the fix for the fourth

**Valid:** dated 2026-09-02

**Severity:** high — five instances inside one five-task feature, the fifth committed *by the fix for the fourth*, by an implementer explicitly asked to sweep for it.

**Status:** open — the class is named in CLAUDE.md § *Testing Discipline* and every instance was caught by review, never by the author. No mechanism exists.

**Observed.** One defect shape recurred five times in a single feature branch
(`entry-id-collision`, merged `5eea9301`): **an assertion satisfied by text that survives deleting
the thing it names.**

| # | site | the assertion | what survives it |
|---|---|---|---|
| 1 | `doctor.rs`, Task 1 | `got[0].1.len() == 2` | a constant, `h.level` (`[2,2]`), 0-indexing (`[2,6]`) |
| 2 | `doctor.rs`, Task 2 | `detail.contains("R-147")` | replacing `lines_str` with `""` |
| 3 | `append_entry.rs`, Task 4 | `msg.contains("push")` | deleting the remedy sentence — *"until yours is **push**ed"* in the explanatory sentence satisfies it |
| 4 | `doctor.rs`, Task 2 fix | `detail.contains('7')` | the token `R-147` contains a `7`; only the `"11"` half discriminated |
| 5 | `doctor.rs`, final fix wave | `got[0].1.len() == 2`, message *"leaving exactly the two `##` definitions"* | `[6, 7]` — what a `push(section.end_line)` mutation produces |

**Why this is the entry and not five commit lines.** Each was fixed on notice, and the fix did not
reach the class. Instance 5 is in the diff written to fix instance 4, in the test that is the merge
blocker's primary evidence, by an implementer whose dispatch said *"treat this as a class — while you
are in these tests, if you spot another assertion satisfiable by text that survives deleting the thing
it names, fix it and say so."* Their report says **"no fifth instance to report."**

**This is `OB-1` measured under controlled conditions.** CLAUDE.md § *Observer Blindness* records four
instances of one class in one evening, *"every one committed by an author actively writing about that
class."* Here the author was not merely aware of the class — they were **holding an instruction to
sweep for it**, in the file they were sweeping. Knowing the class prevented nothing. What caught four
of five was a reviewer holding a different question, and the fifth was caught by a reviewer told to
*judge by mutation, not by presence*.

**The tell, and why "assert more precisely" is the wrong remedy.** Every one of the five is an
assertion whose *stated intent* is correct and whose *satisfiable set* is wider than that intent. The
author cannot see the gap, because they are checking the assertion against what they meant — and it
matches. Only someone asking *"what else satisfies this string?"* sees it. That is a different
question, not a more careful reading of the same one, which is why care is the wrong instrument.

**Candidate mechanism, unbuilt.** A reviewer prompt line — *"for each assertion, name one edit to the
production code that leaves it green"* — converts the check from reading to construction. Cheap, but
it is a policy on a prompt, and this class's own history says policies aimed at authors fail. The
stronger shape would be a lint over test bodies for `contains(<literal>)` where the literal is a
substring of another literal in the same assertion block. Not costed; recorded so the next session
does not re-derive the diagnosis before reaching the design question.

**Rests on:** CLAUDE.md § *Testing Discipline* (*a test cannot detect a change its assertion is
MONOTONE under*) — this is that law's sub-case where the monotone direction is **textual** rather than
structural, and `OB-1` § *the third position* for why the mechanism must not be another instruction to
the party who cannot see it.

**Counterfactual.** Instances 1-4 shipped green and were caught only because each task drew an
independent reviewer. Instance 5 shipped: it is in `experiments` at `5eea9301`, parked with a ruling
rather than fixed, because the process allows one fix wave after a final review and it arrived in that
wave's re-review. So the class's realised cost on this branch is one imprecise assertion in main.

**A sibling datapoint, contributed 2026-09-02 by session `f13f8169` at `codescout-17`'s request — and deliberately NOT counted as a sixth.** 17 offered it as *"the same law from the other side"* and added *"I would rather the count be right than mine"*, so the useful thing is the boundary, not the increment.

The case: fixing `worktree_read_notice` (`7a3aee93`) produced two tests, and mutating the production path once per **site** showed one of them green for the wrong reason. Re-adding the `notice_once` gate killed the repeat assertion — and left the *pinned-silence* test **green vacuously**, because the restored one-shot makes its pinned call silent through a path that has nothing to do with the pin. A single mutation run would have reported the pair as covered.

**Why it is not instance 6.** F-102's five are assertions whose **satisfiable set is textually wider than their stated intent** — `contains('7')` matched by `R-147`. Nothing textual is wrong with the pinned test's assertion: it is exact, and it means what it says. It goes vacuous because a *sibling mutation* removes the mechanism it silently depends on. Same consequence — a guard satisfied by a mechanism that never runs — reached by a different route, so folding it in would blur the shape this entry names precisely.

**What it adds instead is a remedy F-102 does not have.** The pinned test opens with a **fixture check** asserting the same context *does* emit while unpinned, which converts silence-as-evidence into silence-as-contrast: an absence assertion that cannot pass against a context emitting nothing for any reason. That is a construction the author can apply to their own test, where this entry's candidate mechanisms are both aimed at reviewers or linters. Whether it generalises is untested — one instance.

It also supplies a second, independent argument for **mutate per SITE, not per feature**: here the two sites are the two polarities of one behaviour rather than two call sites of one law, and the kill/survive table is what made the vacuity visible at all. `codescout-0a`, reviewing it, called the pairing *"a sharper instance than the one CLAUDE.md currently cites"* — recorded as a second party's read, not adopted as a verdict; promoting it over the shipped example is a decision for whoever next edits that section.

## W-98 — a question about METHOD is a request to re-measure, not to re-explain — and change the instrument, not the care

**Valid:** dated 2026-09-02

**Severity:** high (as a win: the pattern changed a published number once and confirmed a second)

**Status:** validated — two applications in one session, one of which changed the answer.

**Observed.** Twice in one session a published claim was challenged **on its method rather than on
its conclusion**, and both times the response was to re-derive the claim with a different instrument
instead of arguing the conclusion. The outcomes differed, which is the point:

**Case 1 — the conclusion survived, the method had not earned trust.** Published: *"both owed
citations are closed, verified"*, off an unpinned `grep` returning `0 matches`. A peer then reported
that unpinned reads can resolve against a different checkout and return a plausible wrong answer
rather than an error. Re-running the same query **pinned** to the main checkout returned `0` again —
sound. But the pinned run is the only reason that could be said, and at publication time it could
not have been. The correct write-up was *"the claim stands; the method that produced it did not
deserve to be trusted"*, not *"I was right"*.

**Case 2 — the conclusion did not survive, and the error was invisible in the output.** Measuring
whether cited fix-SHAs decay, the first pass used `git cat-file -e` over 123 archived citations and
returned **0 dead**. That reads as *"SHAs never die here"* and would have made a merge-vs-rebase
argument look stakeless. `cat-file -e` tests object **existence**; an orphaned commit survives in the
object store until `git gc` prunes it. Re-measured by **reachability** (`merge-base --is-ancestor`
against `experiments` and `master`): **1 orphan**. Nothing in the first output marked it as answering
a different question.

**Why the trigger is "method questioned", not "claim doubted".** A challenge to the conclusion invites
defence, and defending is often correct — the conclusion may be right. A challenge to the *method*
cannot be answered by re-asserting the conclusion at all, because the two are independent: a sound
conclusion from an unsound method is what Case 1 was, and it is indistinguishable from Case 2 at the
point of publication. **Treat "how did you measure that?" as a request to re-measure, never as a
request to re-explain.**

**The generalisable form.** When re-deriving, change the *instrument*, not the care. Case 2's second
pass was not a more careful `cat-file`; it asked a different question (reachable vs exists). Case 1's
second pass was not a more careful grep; it pinned the workspace. A more careful application of the
same instrument reproduces the same blind spot — which is `W-94`'s rule (*get a claim checked by a peer
whose SAMPLE differs*) applied reflexively, to yourself, when no peer is available.

**Counterfactual.** Without the re-derivation in Case 2, `0 of 123` ships as a measured fact into a
merge-strategy decision and into this ledger, where the next session inherits it. It is wrong, it is
self-consistent, and it points the same direction as the truth — which is what makes it durable: a
reader has no reason to re-check a number that agrees with their expectation. Case 1's counterfactual
is smaller and real: an unpinned zero would have stood as "verified" with no record that its scope was
never established.

**Promote-when:** a third case arises where re-derivation *changes* the answer (Case 2 is the first).
At two-with-one-change this is a validated habit; at three-with-two-changes it is a rule worth a
CLAUDE.md line, and the line to write is the trigger — *a question about method is a request to
re-measure* — not the virtue.

**Rests on:** `W-94` (*state the scope and unit a claim was measured over*) and `F-100` (*a window is
a fact about the query, never about the thing*). This is their reflexive case: the party best placed
to notice is the author, and the author is exactly the party who cannot — so the trigger has to be an
external question, which is the only part of this that is reliably available.

## W-99 — The pathspec commit I adopted AS the safety measure was the unsafe form

**Observed:** 2026-09-02, amending the artifact chunk-grain plan on a shared checkout while a peer held six staged paths in the index.

**Pattern:** The `unreviewed-content` pre-commit hook, which refuses `git commit -- <paths>` when those paths carry unstaged content, and prints both the correct sequence and the bug file recording four measured captures.

**Counterfactual:** **Not an averted capture, and saying so is the whole value of the entry.** I read all 34 deleted lines before committing and every one was mine, so the refused commit would in fact have been clean. What the hook prevented was the *belief* persisting. I had adopted `git commit -- <path>` **deliberately**, across several sessions, as the safe form to use while a peer's index was hot — reasoning that a pathspec commit ignores the index, which is true, and stopping there. It reads the **working tree** at those paths, which is the half that matters on a shared checkout: it commits whatever a concurrent session wrote to the same file since I last looked. The belief emits no symptom, because it commits correctly every time until the one time a peer has touched the same file. So the counterfactual is "it survives until it captures something", not "it captured something today" — which is a weaker claim than a win usually makes, and the accurate one.

**Confirming data points:**
1. The hook fired on a session that had reasoned about index safety **explicitly and at length**, and had reached the wrong form by that reasoning. Care was the instrument here, and it is the instrument that failed — the belief was not a lapse, it was a conclusion.
2. The refusal text names four measured captures already recorded, so the defect class has instances that do not depend on my particular wrong reasoning.
3. The situation the belief was adopted for was real, not imagined: `git diff --cached --name-only` showed 7 paths, 6 of them the peer's. Only the remedy was wrong, which is why nothing about the situation could have flagged it.
4. Staging, then reading `git diff --cached -U0` scoped to my path, reconciled to exactly 34 deletions (32 checkbox rewrites + 2 lines of a stale `**Files:**` block) — the arithmetic is what made "this content is mine" checkable rather than asserted. The hook's prescribed sequence is what produces that artifact; the pathspec form produces nothing to check.

**Impact:** med

**Promote-when:** a second session is recorded adopting a pathspec commit *as its safety measure*. That would make this a convergent wrong conclusion rather than one session's, and convergence is the threshold at which the refusal text stops being sufficient — a hook teaches the party who trips it, and a belief several sessions reach independently wants a line where sessions read before acting rather than after being refused.

**Status:** validated

**Valid:** dated 2026-09-02

**Rests on:** the hook's own refusal text, and `docs/conventions/shared-checkout-commit-sequence.md`.

## F-103 — the guide section served to `artifact.move` documented the pre-fix response shape

**Observed:** `489715ef` added `stage_together` / `stage_hint` to `artifact(action="move")` and documented the staging step in `get_guide("tracker-conventions")` § *Bug files*. But the guide section actually **served to a caller of that tool** is `src/prompts/guides/librarian.md` § *Archiving / Moving Trackers*, marked `<!-- serves: artifact.move, artifact.delete -->`, and it still printed the pre-`489715ef` response example — neither field, no staging instruction. `stage_together` appeared nowhere in that file.

**Cost:** an agent calling `move` receives that section on its first call and learns the old response shape from the surface designed to teach it. Bounded in practice only because the response itself now carries `stage_hint`, which is the mechanism half of the same fix.

**How it surfaced — not by review.** The live probe run for the paired win auto-injected that very section back into this session alongside a response containing the two fields it omits. The drift and its refutation arrived in the same tool result, and the entry would not exist without that adjacency.

**Mechanism, not carelessness.** `serves:` markers make guide sections routable per tool, so "the docs" is not one surface. I updated the section a human reading about bug files would find and missed the one the machine delivers on the call — this project's *loudness is a property of a PATH*, one layer up: the fix went on a path the caller does not traverse. Anyone changing a tool's response shape owes a grep of `serves: <tool>.<action>` across `src/prompts/guides/`, which is a check nothing currently runs.

**Fixed** in the same session, same section: response example updated to carry both fields, plus a paragraph stating that staging both halves is not optional.

- **Severity:** med
- **Status:** fixed-verified

**Valid:** dated 2026-09-02

## W-100 — a live call after the rebuild verified the half the unit test is blind to

**Pattern:** after a rebuild, verify a shipped tool change with one live call instead of trusting the unit test that gated it.

**What the test could not establish.** `move_names_the_staging_action_not_only_the_two_paths` calls `mv::call` directly, so it observes **absolute** paths — `strip_paths_in_value` runs later, at `Tool::call_content`. The test proved `stage_together` exists, is ordered deletion-first, and that `stage_hint` names the command; it proved **nothing** about whether the new `PATH_KEYS` entry actually relativizes at runtime. One live `artifact(action="move")` on the rebuilt binary returned both entries project-relative, which is exactly the half the test is structurally blind to.

**Counterfactual, concrete rather than rhetorical.** A `stage_together` holding absolute paths would have passed the unit suite *and* the corpus gate. `src/tools/core/path_strip.rs`'s own header states that `no_absolute_project_paths_in_rendered_output` "only covers the file-tool surface … no librarian tool is in its fixture set", so librarian's path keys "are exercised only by synthetic-`Value` unit tests". Nothing in the suite covers end-to-end relativization of a librarian key. The defect would have shipped as two absolute paths rendered beside the relativized `old_abs_path` / `new_abs_path` in the same response — visible to every caller, invisible to every test.

**Provenance established positively, not by mtime inference.** Server pid 190206, exe `target/release/codescout` with no ` (deleted)` suffix, started 20:08:10; binary built 20:06:36; `489715ef` committed 17:11:17 with `git merge-base --is-ancestor 489715ef HEAD` true. This is the same check `docs/issues/archive/2026-09-02-a-finished-bug-record-has-no-queryable-way-to-say-so.md`'s `unverified:` was held open for earlier the same day — applied here to this session's own fix rather than to someone else's, which is the harder direction to remember.

**Cost:** three calls — create a throwaway `kind=plan` artifact, move it, delete it. Deliberately not `kind=bug`, which would have perturbed `tests/issue_clusters.rs`' every-open-bug-declares-a-class gate. Both paths confirmed absent afterwards; the only working-tree change left was the guide repair from F-103.

- **Status:** validated

**Valid:** dated 2026-09-02

## F-104 — the notice I fixed, verified and shipped is dropped by 18 renderers — my tests drive the one fixture that keeps it

**Valid:** dated 2026-09-02

**Severity:** med — the fix shipped and works; what does not reach the caller is its output, on
the majority of read tools. No wrong answer was produced, and a defect I had declared verified
was live for four hours.

**Status:** promoted-to-bug-tracker —
`docs/issues/2026-09-02-the-worktree-notice-is-injected-then-discarded-by-every-compact-renderer.md`

**Observed.** A post-rebuild `/mcp` reconnect put this session back in the exact state
`7a3aee93` addresses: slot cleared, one linked worktree present. Two unpinned reads (`tree`,
`symbols`) carried **no** `_workspace_notice`. Under the code I had just shipped, both should
have.

**The scout did not stop at the first plausible cause, and three of them were wrong.** Each was
checkable and each died at the bytes: *the binary predates the fix* (built 20:06:36, fix at
16:58, zero code files changed since, servers started 20:07 on the live exe — `W-95` satisfied);
*the worktrees are gone* (`git worktree list` shows `tool-collapse` registered); *a peer
activated and the shared server carries the flag* (killed by the control below).

**The control is what made the reads diagnostic.** `create_file` was **refused** —
*"worktrees detected but `workspace(action='activate')` has not been called"* — seconds after
the silent reads. `guard_worktree_write` gates on the same `is_project_chosen_this_session()`
flag as the notice, so its refusal proves every precondition held. Two reads got nothing while
a write proved the condition true, which is the same probe shape that closed `F-101`, reused
deliberately.

**The discriminator turned out to be the RESPONSE SHAPE, not the condition.** `artifact(action=
"find")` in the same session seconds later carried the notice. `inject_notice` writes
`_workspace_notice` into the response `Value`; `call_content` then hands that `Value` to
`self.format_compact(&val)`, and no `format_compact` reads the key. **18 tools implement one.**
The notice survives only on the pretty-JSON branch.

**Why this is worth an entry rather than a line in the bug file: my own tests cannot see it, and
they are the tests I mutation-tested.** Both regression tests drive `EchoTool`, whose output
takes the pretty-JSON branch. So they exercise the branch production read tools do *not* take.
The two mutations I ran were against the production path of the **decision** — `notice_once`,
`workspace_override` — and both killed. Neither touched the **delivery** path, because no test
reaches it. A mutation run answers a question about the line you mutate; I had proved the notice
is *computed* and inferred that it is *delivered*.

**And the live check I was proudest of is what hid it.** Production verification — the string in
a real response, not the code path — was `codescout-0a`'s standing ask and I reported it
satisfied. It was: every response I read it in was `artifact`, the one shape where it works. A
correct method, run on an unrepresentative sample, returning a true result that licensed a false
generalisation. `codescout-0a` had already warned the method answers a narrower question than it
looks like it answers; they meant main-checkout-vs-worktree, and it is narrower on a second axis
neither of us named.

**Rests on:** `OB-15` (a silenced mechanism and an absent one produce identical observations —
this is that class holding against the very fix that produced the entry, and the reason it
survived a fix, two mutation kills and a live check); CLAUDE.md § *Testing Discipline*
(*loudness is a property of a PATH*, and *mutate the production path* — where "the production
path" has to mean the delivery path too, not only the decision).

**Promote-when:** a second instance of a framework-injected response field that a per-tool
renderer drops. `inject_hint` sits on the identical code path with the identical exposure and
was not checked here.

## F-105 — A plan's test could not fail — the fallback it never named is artifact-grain

**Category:** plan-drift / test-rigor
**Severity:** high
**Status:** fixed-verified
**Commit:** `e67c3221` (Task 9 of `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md`)

**Observed.** Task 9 specified a complete test — the "mechanical transcription" shape a model
is most likely to trust — and that test **could not fail**. Three things were wrong with it and
only the third is dangerous.

1. It read `out["candidate_ids"]`; `context` emits `included_ids`. Loud — `.as_array().unwrap()`
   panics on `None`.
2. It called `fixture_with_one_ledger_of_many_chunks()` and `run_context()`, neither of which
   exists. Also loud.
3. **Its fixture supplied no embedder and no store.** `context`'s topic branch then falls back
   to a `title|topic contains` filter — and that fallback is **artifact-grain**, so it satisfies
   a distinctness assertion while exercising none of the code under test. Distinctness is
   monotone under narrowing besides: an empty page satisfies it perfectly. Fix the two loud
   defects and the result is green on a tree with `max_per_artifact` deleted.

**Cost.** None realised — caught in Phase 1 scouting, before the test was written. Had it been
transcribed, Task 9 would have shipped a green non-guard over the contract the whole 12-task
plan exists to establish, behind a ticked box.

**Remedy — put the discriminator in the FIXTURE, not the assertion.** The topic string is a
substring of no artifact's title or topic, so the fallback returns **zero** rows; a skipped
semantic path then fails a positive "the other artifact is present" assertion instead of
passing in silence. Confirmed by mutating the production line twice: at `max_per_artifact = 2`
only distinctness fires (`["r/ledger.md", "r/ledger.md", "r/note.md"]`); at `51` only the note's
absence fires (`included_ids` = 50 copies of one artifact). Neither assertion subsumes the
other.

**Generalises to any code path with a SILENT FALLBACK.** The existing law is *"ask which
direction your assertion is monotone under"*. This adds a second question that the first does
not reach: **what does the system do instead when this path is unavailable, and does that also
pass?** A fallback is not a mutation of the feature — it is a *different implementation* of the
same signature, so an assertion written against the signature is blind to which one ran. Here
the fallback's grain happened to be the very property under test. The tell is cheap: if the
fixture omits any dependency the path requires, name what runs in its place.

**Valid:** dated 2026-09-02

**Rests on:** `src/librarian/tools/context.rs` keeping a non-semantic topic branch. If the
fallback is ever removed, condition (1) of the test's docstring becomes vacuous rather than
wrong, and the test quietly weakens — it does not break.

## W-101 — Verifying before filing turned an edit_file bug into a byte-level proof of a peer's stash window

**Status:** validated
**Promote-when:** a second instance appears where a tool's error message is treated as evidence about the world rather than about the tool's own read.

**Pattern.** Before filing a bug against the component that *reported* an anomaly, ask what
else was writing to the resource it read. On a shared checkout the answer is routinely "a
peer's pre-commit hook, which empties the working tree of every unstaged change for the
duration of its run".

**What happened.** `edit_file` refused a mutation with `old_string not found … Nearest content
at lines 689-690:` followed by the **pre-edit** content, quoted, at a line number — while
`read_file` seconds later returned the post-edit content, and a `cargo test` in between had
already proven the post-edit content was live. A bug file against `edit_file` was drafted.

**Counterfactual, and it is not "one wasted file".** Two diagnostics had already run and both
pointed the wrong way while looking like progress: `read_edit_target`
(`src/tools/edit_file/mod.rs:678`) is a bare `std::fs::read_to_string` with **no cache**, and a
two-edit scratch-file probe did not reproduce. Reading those as "so the cache must be specific
to indexed source files" is the natural next step, and it is the story the bug file would have
carried — sending the next reader after a cache that does not exist, in a tool that is not at
fault, while the real class (`IC-12`, `cluster/transient-shared-state-lies-to-readers`) gained
no member and kept its *"no downstream failure observed"* line.

**What closed it.** Not more care — a different question. `IC-12`'s own member list names
`pre_commit/staged_files_only.py:108`, and pre-commit leaves its stash on disk at
`~/.cache/pre-commit/patch<epoch>-<pid>` **permanently**. The patch from the peer commit that
landed inside the window contains, verbatim:

```
16248:-                1,
16249:+                2,
```

— the exact unstaged mutation, proving the file on disk carried the old line for that run.
Mechanism confirmed at the byte level, ~4 tool calls, no speculation left over.

**The reusable half is the oracle, not the caution.** Every remedy this class previously
named must be run *during* the window, which is the one thing a surprised reader cannot do.
The patch directory is readable afterwards and is a **more complete** index than `git log`:
two of the four stash events bracketing this window correspond to no commit at all, because an
aborted hook run stashes too. Written up in
`docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` § *Evidence*.

**Valid:** dated 2026-09-02

**Rests on:** pre-commit retaining stash patches under `~/.cache/pre-commit/`. If it ever
starts cleaning them up, the oracle disappears and the class returns to being diagnosable only
from inside the window.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
