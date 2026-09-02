---
id: '9a892c2a5976e296'
kind: tracker
status: active
title: Open-Issue Work Queue (BL-N)
owners:
- marius
tags:
- backlog
- sequencing
- bugs
- work-queue
topic: work-queue
expects_augmentation: docs/augmentations/docs-trackers-open-issue-work-queue.yaml
---

> **Prefix:** `BL-N` — a row in this queue. Work-stream-scoped, defined here, not a project-wide
> namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). Deliberately **not** `T-N`, which
> belongs to `docs/trackers/tool-usage-patterns.md`.

## What this is, and what it is not

A **sequencing layer** over the open bug ledger, snapshotted 2026-08-16 from
`artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})` — 17 rows.

It exists because the ledger answers *what is broken* but not *what to pick up next*. A flat
`status="open"` query cannot express readiness, blockers, or the fact that two entries need the same
decision made once. That is all this file adds.

**It does not own bug status.** Every row points at a bug file, and that file is authoritative. If a
row says `open` and the bug says `fixed`, the bug is right and the row is stale. Never close a bug
from here — and never treat the one-line `next` as the instruction. It is a pointer to that bug's
`## Resume`, which carries the real next action along with the caveats.

## Queue — rendered snapshot (2026-08-18)

> **`params` is the source of truth; this table is a snapshot of it.** Params live in the librarian
> catalog (`~/.local/share/librarian/catalog.db`), which is **not** in the repo — so without this
> section the queue would be invisible to git and to any other checkout. Re-render it when rows
> change. Query the live rows with
> `artifact(get, id="9a892c2a5976e296", entry_filter={"status":{"eq":"open"}})`.

| ID | Ph | Task | Status | Bug |
|----|---:|------|--------|-----|
| BL-1 | 1 | json_path: add a `Segment::Wildcard` arm so the overflow hint's own recovery works | **done** | `875e5d03d980ceac` |
| BL-2 | 1 | grep: stop printing a self-refuting "Showing N of N" when collection hit the cap | **done, archived** | `8e665c2d041ebb04` |
| BL-3 | 1 | Tool schemas: stop advertising conditionally-required params as optional | **done, archived** | `02d2d9d8a7eeec2e` |
| BL-4 | 1 | usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer | **done, archived** | `fc00c33f2403ae8a` |
| BL-5 | 1 | librarian: split `tracker_design` so its guidance arrives inline | **done** | `3f88d49c38ced0c1` |
| BL-6 | 1 | read_file: give the buffered full-read summary an incompleteness signal | **done** | `a9644b964edac789` |
| BL-7 | 1 | Write-scope denial should name `approve_write` | **done** | `0a15c81150c4cce7` |
| BL-8 | 2 | `truncate_compact` cuts from the tail, destroying the overflow signal | **done** | `c320b6564d1cb003` |
| BL-9 | 2 | `server_instructions` arrives truncated mid-word, dropping the guide pointers | **done, archived** | `be057e5e9d7c4c16` |
| BL-37 | 2 | Kotlin warnings / workspace table / Custom Instructions cannot fit the 2048-char instructions channel — ordering fixes the common case (shipped `30f3df81`); the two oversized blocks need a new carrier | done | `e3437bd1ec116dec` |
| BL-10 | 2 | `audit_doc_refs` reads bare comment markers as file paths | **done** | `772fff5739620581` |
| BL-11 | 2 | `context`/`workspace_state_at`/`link_scan` never dedup the worktree overlay | done | `d31233700ca979c2` |
| BL-12 | 2 | worktree divergence guard covers writes but not reads | done | `c611a3dce4f05d45` |
| BL-35 | 2 | `guard_worktree_write` is dead code in production (startup cwd sets the flag it gates on) | **done, archived** | `a742a50ea6723daf` |
| BL-13 | 3 | IL1: run subtract-and-measure on the step-3 wording — ran as hamsa A-25, clause LOST (base 10/10, clause 8/10 vs a ≤ 1/10 ship bar), reverted at `32b34efa` behind an inverted guard | **done, archived** | `b4d48dbfecc205c9` |
| BL-14 | 3 | read_file: `force=true` silently discarded on whole-file reads | done | `1780acde047ffca2` |
| BL-15 | 3 | Read-only metadata commands (wc/ls/stat) blocked on source paths | done | `6902806f459fcf62` |
| BL-16 | 3 | Worktree activation diverges memory set and sub-project topology (topology CLOSED `1869adcb`; memories = option 1, a semantic call) | open | `403e3fad0356f171` |
| BL-17 | 4 | Reconcile a bug sitting in `archive/` while still marked `status: open` | **done** — measured 0; its own bug file is gone | — |
| BL-18 | 1 | `artifact(create)`: `augment` silently discarded five of its seven fields | **done** | `29f1ddf259562b7f` |
| BL-19 | 1 | Overflow envelopes with no compact summary waste a whole call | **done, archived** | `3d733b00b134634c` |
| BL-20 | 1 | params merge-patch wipes entry arrays wholesale — gave entries an update path (`update_entry`) + always-on counts | **done, archived** | `36eda0c2634dbea9` |
| BL-21 | 1 | edit_file's replace_all + batch paths write librarian-managed artifacts with no guard | **done, archived** | `e52abced30ff1dbc` |
| BL-22 | 1 | `move` broke the `id == hash(abs_path)` invariant, so the next reindex cascade-deleted the history | **done, archived** | `18a637f59289192c` |
| BL-23 | 3 | a moved artifact's frontmatter still asserts its pre-move id | **done, archived** | `61e2360408cb206b` |
| BL-24 | 2 | usage.db records a sha that need not describe the built code, and drops the dirty bit | **done, archived** | `0cd1fe818951b232` |
| BL-25 | 1 | the 2200-byte cap evicts rules into `get_guide` topics nothing triggers — 7 of 10 guides (~46 KB) have no trigger at all | **done** | `cfcbee6f7d047a55` |
| BL-26 | 2 | `get_guide("librarian-runtime")` says a move preserves the id; a move mints a new one — 2d8c7f39 repaired 3 of 4 copies | **done, archived** | `5d8584d109d876ea` |
| BL-27 | 3 | `update_entry`'s entry-param guard only fires when `fields` is absent; send both and `entry` is dropped silently | **done, archived** | `d082f963f57bd76b` |
| BL-28 | 3 | a directory named `--help` holding an initialised codescout project sits untracked in the repo root | **done, archived** | `ba6ab341eab97416` |
| BL-29 | 1 | `append_entry` writes catalog-only state, so this very snapshot drifts silently — tool says success, git says clean | partial (`99aaf83f` + `6ff00eee` + `0dbfd0ee`): drift reported at write time and by `doctor`; hamsa reconciled; gate now needs majority coverage; **0 trackers adrift** | `879b1b18243f20b2` |
| BL-30 | 2 | FRICTION: adding one entry costs four bookkeeping sub-tasks — id, workflow, row format, re-render | done | `c3f08f7cb8b386fe` |
| BL-31 | 2 | grep: `cap_grouped`'s file-diversity round-robin is unreachable, so overflow hints name walk-order files not hot ones | **done, archived** | `2a9fd7654cf82013` |
| BL-32 | 3 | R-N ledger reused nine ids for unrelated lessons — split by suffix in `52fca682`; the hand-allocation cause is BL-30 | done | `a0251c34af7aa012` |
| BL-33 | 1 | the librarian guard keys on YAML quoting, so 15 of 27 trackers (incl. this queue) are unprotected | **done, archived** | `e7353641aafe0098` |
| BL-34 | 2 | repairing a frontmatter id re-serializes the whole block, reformatting hand-authored YAML | **done, archived** | `529a6c05895cc686` |
| BL-36 | 1 | `artifact(update)` re-serializes the whole frontmatter block on a single-field patch — BL-34's mechanism at the mandated archive step | **done, archived** | `82ba248228301486` |
| BL-40 | 1 | every drift check asks whether the body kept up with params — nothing detects params falling behind a body that ran ahead | **done, archived** `87f3b936` — fired twice on its first live run, incl. a second repo nobody had looked at; data repair split to BL-42. Bug file archived; id re-keyed `bde782f4cc52ac22` → `0808a5251625e6db` | `0808a5251625e6db` |
| BL-41 | 1 | link_scan's dangling count is prefix-gated, so a namespace with zero definitions reports as healthy | **done, archived** `ff088630` — measured on the wire: dangling 548 → 471, but BL-41's own contribution is **ZERO**; the prediction in the bug file was wrong. Coverage completion → BL-43. Bug file archived; id re-keyed `52269554ea4f51a4` → `e891b7c6a5b1dbe7` | `e891b7c6a5b1dbe7` |
| BL-42 | 2 | DATA REPAIR: entry rows exist in a tracker body and in no params row, so `entry_filter` and every params-based query miss them | **done — codescout half**: the 6 WIN rows repaired, `doctor` 2→1. A second defect (7 rows present on both sides with disagreeing content) surfaced while diffing and was also repaired — split out as its own bug, not this one. SI×10 still handed off | `0808a5251625e6db` |
| BL-44 | 3 | NEW, surfaced while closing BL-42: no check detects an existing params row whose CONTENT has gone stale relative to its body (distinct from BL-40/BL-42, which are about row absence) | **done 2026-08-30** — `92228de0`, patch-id `0ba7a71b3f8462e8`. `scan_params_status_drift` ships beside the three id-set scans, on the same `ParamsBackedLedger`. Gate green (4873/0 full, 3385/0 lean — lean is unchanged because `doctor` is behind the `librarian` default feature and that lane compiles none of it). Measured before any Rust: a naive compare gives 26 findings of which **20 are `done, archived` vs `done-archived`**, and absorbing that one rendering takes it from 77% noise to 6 on 101 rows; sensitivity **91.4%** over 536 simulated drifts (490 flagged / 46 silent), which the violation message states rather than implying certainty; coverage **4 of 9** ledgers, not the 6 first estimated — I had measured the schema and assumed the rendering, and two ledgers use a heading plus `**Status:**` rather than a table row, so a single-locator version skipped 30 of 131 exposed entries silently. Boundary-anchored regex, not stripped punctuation, because stripping makes `done` a substring of `abandoned`. Two mutations, disjoint kills; the first exposed a test of mine that died on its first assertion without reaching the guard its name claimed (→ `reconnaissance-patterns:R-130`). Was dropped as "a design decision, not a queued task" with the 7 `windows-platform-support.md` rows repaired, then re-opened when the predicted failure fired. The class then recurred here: BL-49/56/60/64 held params rows disagreeing with this committed body, params always the older side; BL-60 read `open` / "not yet scouted" for a fix that was shipped, archived and live-verified, and carried the dead pre-archive bug id. The drop-time prediction in the BL-42 detail below named the consequence verbatim, and a resuming session hit it not knowing it existed. Now 2 of 2 mirrored trackers ever swept — and self-inflicted by the documented workflow, since `body_edits` moves the body with no surface reporting params left behind (`snapshot_stale` and `params_behind_body` are both id-set checks, and every id matched). Rows repaired; gap untouched | `640c4fc65a64461c` |
| BL-43 | 1 | complete BL-41's coverage — a ledger that declares no `entry_prefix` AND defines nothing is still invisible to the dangling gate | **dropped — handed off**, both targets are other repos; codescout's own declarations are already done | `e891b7c6a5b1dbe7` |
| BL-39 | 1 | the two sanctioned entry formats are not equivalent — a params-rendered index defines no citable token, so 117 BL-N citations (incl. this queue's own) resolve to nothing | **done, archived** — all steps shipped (`de4df2cd`, `f19d5296`, `758b37dc`, `d3c1e6ed`, backfills `f04e4c17`/`0d101eb8`/`f5f602e6`/`9703102c`/`c7bdfd22`). `doctor` `ledger_defines_nothing` 10→2, `entry_without_definition` 3→1. The blocking peer file committed and was archived; moved 2026-08-18, id re-keyed `d34dfcd2cc718bd8` → `9dc28c0860b214d9`; 19 citations across 10 live files re-pointed in the same commit | `9dc28c0860b214d9` |
| BL-38 | 1 | the librarian guard is blind to any artifact whose frontmatter omits `id:` — fixed by teaching it the `entry_prefix` ledger declaration; the plan's heading-scoped half was cut as unnecessary | done | `388290ad0f86fe03` |
| BL-45 | 1 | Decision 1: may a process on an unlinked binary re-index? Direction 2 of the zombie-server bug, corrected — refuse BEFORE the embed pass, not at the sidecar write | **done** — `22f8b8d5`, patch-id `fd2c453b…`. Hard refusal as a `RecoverableError` naming `/mcp`; `guard_stale_binary` guards both `sync_project` and `sync_worktree` ahead of the embed pass; 5 tests, wiring mutation-checked; live from the 2026-08-29 rebuild | `8400845b81ff0475` |
| BL-46 | 2 | Decision 2: the write-root split — unpinned WRITES resolve to the last writable root, unpinned READS keep resolving to the activated one | **not started** — needs a `last_writable_root` field plus write-awareness in `with_project_at`; ~4,600 tests sit on that primitive | — |
| BL-47 | 1 | `tags.in` returns zero while `tags.contains` finds the same row — and the librarian guide teaches the broken form | **done** — `9e4e2d36`, patch-id `cfac211d…`. Both engines routed through `json_each`; `nin` was the worse half, returning EVERY row incl. those holding the tag; 3 tests. Live-verified post-rebuild: same call 0 → 11 in scope | `1d085bcddf13d685` |
| BL-48 | 1 | `edit_markdown`'s frontmatter write never touches the catalog, so `find(kind="bug", status=…)` reports the pre-edit status indefinitely | **done** — `518549d6`, patch-id `c424f89f…`. Installed hook mirroring `librarian_guard`'s oracle; never creates a row; 8 tests, wiring mutation-checked both ways. Residual: the server-side install is covered by nothing. Bug file archived 2026-08-30 — the status flip reproduced the bug on itself, the fix not being live in this server | `013458f0acdb88b8` |
| BL-49 | 2 | `workspace(post_compact)` flushes LSP without prewarming — next nav call pays cold start and can blow the 60s timeout, while its hint promises no disruption | **partial** — hint + manual fixed; diagnosis corrected in 3 places. Its prescribed fix (a) was a NO-OP for its own Rust repro (`PREWARM_LANGUAGES` is JVM-only), and a mux keyed by workspace keeps the server warm across sessions, so the cold window is far narrower than filed. The actually-false sentence is cross-repo (`session-start.mjs:339`) and still emitting — stays open for that. Hint fix `ff90ce41`, patch-id `9da21228d4392923`; **observed live** in the running release binary on 2026-08-30 when `workspace(post_compact=true)` returned the new text after a compaction — first sighting in the wild, so this row reports it rather than inferring it from source | `d7072ed21959aca1` |
| BL-50 | 2 | `expects_augmentation` is a boolean, so a fresh clone knows an augmentation is missing but nothing records what it was | **done 2026-08-31** — item (3) discharged on the host that held the shapes; `e799f29d` + `1ad9af66` + `f565504a` + `e1b91221` + `c2039a16` (patch-id `63a943ba8e2a1a9b`), gate green 4834/0 full, 3360/0 lean. Shape travels in `docs/augmentations/<flattened-path>.yaml`; `reindex` re-attaches when the row is ABSENT and never overwrites a live one; `params` do not travel. The 9 shapes here are exported and committed, declarations upgraded to paths, verified live (0 `augmentation_declaration_unparseable`). Write-through CLOSED by `5f88be65` (patch-id `59ba22f9d7a6dfed`) — `artifact_augment` pushes a shape change to an *existing* sidecar at both write sites, mutation-verified per site; export still only CREATES. The note previously in this cell, *"not blocking — the export is idempotent"*, was wrong: idempotence was the defect's mechanism, not its mitigation (R-131), and it fired live within a day at `2a8decc5`. Not covered: drift from outside `artifact_augment` (hand-edit, graft, worktree merge) is still undetected — a `sidecar_shape_drift` check is the natural follow-on. Detection added by `03b86cd7` (`sidecar_shape_drift` + `sidecar_unparseable`), so the invariant no longer depends on call-site enumeration; it reports and has no `fix=`, and its first live run justified that better than the argument did — the queue drifted in BOTH directions at once, so direction is not even a property of an artifact. Repaired at `b05af94f`; check reports 0. **(3) UNBLOCKED 2026-08-30** — `git push origin experiments` landed 209 commits (`894a5e26..b05af94f`); all six sidecar-mechanism commits are on `origin/experiments` with 9 sidecars published. **CLOSED 2026-08-31** (`1ec456a4`, patch-id `a1967b8b671cba1c`) — "THERE" was this desktop, whose catalog held all 23. Corpus 9 → 23, 0 failures; probe reports 23/23 restored into a catalog that never held them, 23 distinct prompts, `params={}`. It was **14**, not 13: `claim-decay.md` was augmented and declared nothing | `11dec5e144ba0482` |
| BL-51 | 2 | a rendezvous slot that misses its SessionStart stamp can never be stamped again — Phase C inactive for that server's life | **dropped** — both claims refuted by their own author 90 min after filing; self-heals at next SessionStart; severity `informational`; code is JS in `claude-plugins`, not this repo | `d91e96485308ee2f` |
| BL-52 | 2 | the rendezvous gate latches open, so a hook going quiet mid-process leaves `/clear` invisible again | **blocked** — sketched fix refuted (`hook_at` ages 0.6–25h, so no window discriminates); viable fix is cross-repo + a design decision; next step is measurement, not code | `54a70b49f6f26681` |
| BL-53 | 3 | guide topics are atomic nodes in an unmodelled graph — also `GG-7`, sequenced there; do not fix from here | open (cross-ref) | `7579b32b1cd2362f` |
| BL-54 | 2 | workspace `read_only` flips mid-session with no `activate` — also `WP-5`; may share a `with_project_at` root cause with BL-46 | mitigated + archived 2026-09-02 (`3ccfefb2`) — per-call pinning is a probe-verified escape; the structural half was **declined**, not deferred | `6a3bb4d968d1d514` |
| BL-55 | 3 | three unrelated tests failed together on the wine lane under load — the reference case for "flaky by wall clock" vs "defect load exposes" (`F-78`) | open | `05b157e0c38b765a` |
| BL-56 | 1 | SDD ledger directory and its catalog rows both vanished between sessions — gitignored catalog means unrecoverable, not stale | **zombie 2026-08-30** — the disposition its own Resume prescribed. Hypotheses 4 and 6 acquitted from code + live measurement, plus a newly-found 9 (→ BL-64) acquitted twice. Survivor is 8 (a foreign `codex` writer), and it is **unfalsifiable, not untested**: the catalog keeps no write audit trail, so "who deleted these rows" has no answer once the window closes. Re-open trigger in frontmatter | `73158c500ff6b293` |
| BL-65 | 1 | the CLI's `doctor` exposes no `--fix`, so all six repairs are MCP-only | **open** — third instance of one mechanism in a day (after `19289b1f` and BL-60); strands `fix=export_augmentations`, which exists to run on the OTHER machine. Root cause INFERRED from `--help`. Two findings now argue for a key-set coverage test over a fourth round of flags | `2f2409074ee2319d` |
| BL-64 | 3 | `reindex_cli` is test-only and carries a broken copy of a DELETE deliberately removed for causing data loss | **done** — `9f743091`, patch-id `92db5adf65b7a748`. Took (a) delete-the-block over (b) plumb-`force`: **both** `index_repo` call sites are test-only while `index_repo` is public API, so (b) was a semver break serving only tests, building a reference implementation nothing references. A comment now stands where the block was — that is the remedy, the hazard being a reader "fixing" the missing `%`. Regression test mutation-verified: **with** `%` FAILS, **without** `%` passes | `6ff4394bb3b18d86` |
| BL-66 | 3 | `probe_ollama` is the one TLS construction site in either tree that installs no crypto provider, and its failure is reported as "Ollama is not reachable" | **done, archived** — `1909e5f0`, patch-id `90c1612bcd948c09e0fd373be2e754134bf9a463`. Fourth instance of the root/crate asymmetry, found by enumerating call sites of `install_default_crypto_provider` rather than by pairwise diff — `probe_ollama` has no root twin. **The reproduction falsified three premises of this row.** It **panics** inside `ClientBuilder::build()` rather than returning `Err`, so the "Ollama is not reachable" misreport described here is unreachable code and never prints; the URL scheme is irrelevant, so plain `http://localhost:11434` panics identically; and severity was `low` on an operator-with-TLS theory when in fact **every external consumer aborts at zero configuration**. (a) shipped; (b) dropped, its premise being the false one | `a7af9964a16e8056` |
| BL-67 | 2 | `export_augmentations` will not rewrite a sidecar whose shape changed, so a `params_schema` edit silently does not travel | **done, archived** — `5f88be65`, patch-id `59ba22f9d7a6dfed66fcd8e551e09455b5c58f32`. A peer commit that does not name this entry, closed by a **verify-open pass** rather than a gate. **The title now describes intended behaviour:** `export_augmentations` still reports `exported: 0` for a stale-but-existing sidecar, correctly — export CREATES, never refreshes. The fix is one layer up: `artifact_augment` writes a shape change through to an existing sidecar at both shape-writing sites, byte-guarded so a params-only merge leaves the file untouched by construction, and sidecar-ahead (BL-50's restore path) stays untouched. Re-reproduced end-to-end on a throwaway artifact: prompt and enum both travelled with no export step, after which export reported `0` — the original symptom's number, now meaning the opposite, which is why the naive check reads as *not fixed* | `689fb62e40557480` |
| BL-68 | 3 | 37 tests in `crates/codescout-embed/` are not compiled by either gate test command; 33 of them would execute | **done** — swapped `cargo test` → `cargo test --workspace` in `CLAUDE.md`; gate stays **four** commands, **+2.6s** warm, subsumption verified by `-- --list` set-difference (0 tests present in bare and absent from workspace). Measured: `-p codescout-embed --features remote-embed` → **56 compiled, 52 executing** (4 `ollama_*` are `#[ignore]`d), `--no-default-features` → **19**. Bare `cargo test` builds only the ROOT package (0 of them); `cargo test --workspace --no-default-features` builds the member with `remote-embed` off. Not a `tests/`-directory issue — `remote::tests` inline `#[cfg(test)]` is 32 of the 33, including the regression guards for three bugs fixed the same day. **CI does cover them** via its `default` matrix lane (`cargo test --workspace`, `flags: ""`), so this is `W-81`'s feedback-latency axis rather than missing coverage. Candidate remedy is a fifth gate command; adding one is the operator's call, surfaced with the measurement rather than applied | — |
| BL-57 | 1 | `@tool_*` buffer grep returns the JSON envelope, not the stdout | **done-archived — fixed (`61476cb5`) and archived 2026-08-30** | `4eea94e21203cd46` |
| BL-58 | 2 | ListAgents omits live cross-profile sessions in the same checkout, and two sessions' counts are **incomparable** rather than merely short | **partial** — root cause still blocked upstream (harness, not this repo); local mitigation SHIPPED as `scripts/peer-sessions.sh` + a `docs/PROBES.md` row, so it fires at the moment of use instead of when someone opens the bug file (`W-85`'s placement lesson, which this entry was itself an instance of). Measured 2026-08-30: `ListAgents` reported **3** in this checkout, the probe found **5**, and `807989` had run since **11:10:52** — before any inter-session message that day — invisible to every participant. The afternoon's six misattributions were eliminations over a set short by **two**. Invisible is not unreachable: any pid is addressable as `uds:<sock>`. The probe bounds the POPULATION and does not attribute a write, and says so in its own output. **Corrected before publication:** a relayed claim that one session's view was *entirely disjoint* from the five is an observed STATE, not a property — 0 of 5 at ~20:06, 1 of 5 at ~20:2x, same count with rotated membership; asking the source rather than relaying is what caught it. Sharper fact underneath: those readings showed **2 of 2** sessions outside this checkout and **1 of 4** inside it, so the view is skewed against the population every attribution asks about — elimination is *unrelated to the question*, not merely weak, and socket addressing is the only correct method rather than a fallback. Both open attributions were then settled by asking, one round each, after three rounds of elimination got them wrong | `1950479aff0acb5b` |
| BL-59 | 2 | the buddy compact banner's `from=<sid>` names another live session, reading as "your own pre-compaction transcript" | **blocked** — `claude-plugins`, not this repo. Worse than BL-58 in kind: that one understates who else writes your files, this overstates what **you** wrote, and cannot be refuted from the inside | `6411eb594cd7231d` |
| BL-60 | 1 | the CLI's `artifact create` / `artifact update` silently drop `time_scope` and `extra` | **done-archived** — `0c4931ef`, patch-id `a0a4a3b4…`. Both flags on both subcommands, marshalled at the depth each tool expects; `build_create_tool_args` extracted so the create half is testable without a catalog, as `19289b1f` already did for update. 8 tests, 3 mutations spent — incl. a characterization guard proving the extraction was behaviour-preserving. The bug's "nothing to run" was itself worth running: `--force` present alongside both flags absent proved the gap was current, not a stale build | `64c7dab799d1bca7` |
| BL-61 | 3 | ZOMBIE WATCH: `references` answers a warming LSP with `symbol not found` | open (**watch, not work**) — re-open trigger is `symbol not found` only; a timeout and a guarded zero are outside it, and BL-49 was checked against it | `d25aa6db7b4e6367` |
| BL-62 | 3 | ZOMBIE WATCH: two Windows CI tests flake on wall-clock/race assumptions | open (**watch, not work**) — check the wine skip-list first; W-64 took it 32 → 8 with every survivor classified | `e817931ef9d51dd0` |
| BL-63 | 3 | ZOMBIE WATCH: `symbols` search mode 0-matches then succeeds on retry | open (**watch, not work**) — Bug A fixed, Bug B mitigated + instrumented; read the instrumentation before treating it as live | `523233935cc53bc4` |
| BL-69 | 1 | Repair the 3 files with an unterminated fence | **done** 2026-09-01 — all three were one defect: a nested fence closing its enclosing ```markdown block early. Two widened to ````, one stray trailing delimiter deleted. `unterminated_fence` 3 → 0 | — |
| BL-70 | 1 | Catalog-integrity sweep — the 3 IN-REPO doctor findings | **done** 2026-09-01 — three, not six (the rest are in the whatsapp / eduplanner-ui repos). One was a NO-ACTION by the finding's own instruction: provenance-subsystem's missing headings are the compaction ladder's supported end state, and adding one would make the token ambiguous. I-8's params row written by whole-array rewrite (8/8 survive, catalog-only so it does not travel in git); claim-decay DC-2's body Status corrected to `check-shipped` | — |
| BL-71 | 1 | Triage the link-graph findings, and test whether the volume gate discriminates | **done** 2026-09-01, both halves. The 14 `cited_prefix_with_no_definer` are 8 non-citations and the top four by volume are all noise — filed as `9b67295c125cfcb6`, tagged IC-2, and **now FIXED** at `d44a4409` (patch-id `f37ae73ffe72846ca0cceae19b9649f8c24b6a08`) by gating on DISPERSION rather than volume — 13 findings → 7, six noise prefixes suppressed, zero real ones lost. Two of its original four options were circular (`prefix_is_known` is false for exactly this check's population, so reusing it would zero the check), so the shipped remedy was a fifth. The 13 `entry_cited_from_outside_but_undeclared` → 0 at `40c53197`: 4 invariant, 7 dated, 2 conditional, every date read from the record; BL-58 needed only relocating a mid-line declaration | `9b67295c125cfcb6` |

> **Params and body reconciled again** (2026-08-16, second pass — 31 rows). The
> previous reconciliation held for status but not for **ids**: BL-26 and BL-27 were
> archived, and `artifact(action="move")` re-keys, so params carried the new ids while
> this snapshot still cited `db02045fdbaaf860` / `ea21099f9d39f734` — neither of which
> resolves any more. Three rows had drifted (BL-2, BL-26, BL-27) and BL-31 was missing
> entirely.
>
> A **duplicate BL-31 row** also had to be removed here: two sessions hand-rendered the
> same new params entry into this table within minutes of each other, one of them with
> `—` placeholders for the fields it could not see. `append_entry` prevented the *id*
> collision server-side (the concurrent session's entry took BL-32, mine BL-33, with no
> coordination) — but nothing protects the hand-maintained snapshot from the same race.
> That gap between the two is BL-30's cost stated precisely.
>
> That is **BL-29** demonstrated, not a lapse: `update_entry` and `move` both write
> catalog-only state, so every entry-grain write silently ages this table. The check that
> catches it is `artifact(get, entry_filter=…)` against the live params before trusting
> any row here — an id that returns `count: 0` is archived, not deleted.
>
> Earlier note, still true: BL-1, BL-20 and BL-22 were flipped with
> `artifact(action="update_entry", …)`. The note that used to sit here said the flip was
> unsafe because there was no entry-grain update; that was BL-20, and it is now fixed.
> Its own row was the first thing the fix was used on.

Next actions per row live in each bug's `## Resume`, and in the live params — not duplicated here,
because a snapshot that carries instructions goes stale in the way that matters most.

## Per-entry detail
### BL-47 — `tags.in` returns zero while `tags.contains` finds the same row

**Status:** done — shipped `9e4e2d36`, live-verified, and the bug file archived 2026-08-30.
(Was `open — picked up this session`; the params row already said `done`, so body and params
disagreed until the archive pass reconciled them.)

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-28-tags-in-filter-returns-zero.md`. A filter form the librarian guide
actively teaches returns an empty result where the sibling operator finds the row. The failure
mode is a **zero that lies** — no error, no warning, just an answer that reads as "nothing
matches". That is the ledger's most-repeated law (`reconnaissance-patterns` law C) reproduced as
a defect in the query layer itself, which is why it sorts above larger items.

**Fixed 2026-08-29 — gate green, uncommitted, and NOT yet live.** *(True when written;
superseded the same day — see the shipped/live-verified lines that close this entry. Left
standing rather than deleted, because the gap between "gate green" and "live" is exactly
what the entry goes on to argue.)*

**Root cause, at `src/librarian/filter.rs`.** The array-column branch was gated on the *op*:
`if op == LeafOp::Contains && is_array_col`. `tags`/`owners` are stored as JSON arrays, so
`in` fell through to `tags IN (?)`, comparing the column's raw JSON text (`["session-log",
"bug-fix"]`) against a scalar. Never equal.

**`nin` was the worse half, and the bug file did not name it.** `in` returned nothing — a zero
that lies. `nin` returned **everything**, including rows that DO hold the listed tag, because
`tags NOT IN (?)` is true for every row whose JSON text differs from the scalar. A silent
false-*positive* generator: `nin` is the operator `find`'s own archived-hide uses, so this was
not a hypothetical path.

**Fix.** Gate on the *column*, let the op select the shape — `EXISTS (SELECT 1 FROM
json_each(tags) WHERE value IN (…))` for `in`, `NOT EXISTS` for `nin`. Both `in` paths now share
one `in_list_params` helper, deliberately: the defect was a second `in` code path that never
learned what the first knew. The in-memory `eval` twin got the matching type-driven fix
(`Value::Array(items) => …`), mirroring its own `Contains` arm, so the two engines cannot answer
the same AST differently.

**The existing differential test could not have caught this.** `eval_matches_compile_on_fixture`
passes before and after — its table is `CREATE TABLE e (id, status, confidence, title)`, with no
array column, so it can never reach the branch. Checked before writing rather than assumed, per
`bug-fix-session-log:W-73`; that makes three fixture-unreachable instances across three
subsystems in one afternoon.

**Tests (3):** an end-to-end `in` against a real `json()` column asserting `[t1, t2]` where the
defect gives `[]`; an end-to-end `nin` asserting `[t2, t3]` where the defect gives `[t1, t2, t3]`,
which also pins the empty-tags decision (holding none of the listed tags includes holding none at
all) as a decision rather than an accident; and an `eval` parity test covering hit, miss and
complement.

**Gate:** `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D
warnings`, `cargo check --no-default-features` all clean; full `cargo test` **4798 passed, 0
failed**.

**LIVE-VERIFIED 2026-08-29, after the operator rebuilt and restarted the MCP server.** The same
call — `artifact(find, filter={"tags": {"in": ["session-log"]}})` — that returned **0** against
the pre-fix binary now returns **11 in scope**. Probed rather than inferred from the rebuild
having happened.

The before/after pair is `reconnaissance-patterns:R-89` run end to end in one entry: the code was
correct and the tests green while the serving copy still returned the defect, so "fixed" and
"live" were genuinely different states for about an hour. The honest claim for an unprobed edit
is "fixed"; only the probe upgrades it.

**Side effect worth noting:** `get_guide("librarian")` § *Filter Syntax* teaches
`{"tags": {"in": ["foo", "bar"]}}` as an example — it auto-injected into this session in the same
response as the zero-result reproduction. No doc edit is owed: the taught form is now correct by
construction.
**Shipped `9e4e2d36` on `experiments`**, patch-id
`cfac211d37020aa4815ce7e0277c15704559ea13`.
### BL-48 — `edit_markdown`'s frontmatter write never touches the catalog

**Status:** done — fixed 2026-08-30 (`518549d6`, patch-id `c424f89f…`); bug file archived. Residual, already tracked in that file's `unverified:` and not work owed here: the server-side install at `src/server.rs:374` is covered by no test.

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`. Filed by a peer
session after two independent hits on one afternoon, one of them mine: a bug archived with
`status: fixed` on disk kept reporting `open` from `find(kind="bug", …)`, and surfaced in a
"what's open?" report. The guard at `edit_markdown.rs` correctly passes plain bug files through
(they are neither augmented nor ledgers), so catalog membership is exactly the population allowed
through whose row then lies. Corrupts the canonical triage query CLAUDE.md and the activation
bootstrap both prescribe.

**Fixed 2026-08-30 — `518549d6` on `experiments`**, patch-id
`c424f89f8aeb67eaa692eeda4a9812a13820041c`. Gate green: `fmt`, `clippy --workspace
--all-targets --features local-embed -D warnings`, `check --no-default-features
--all-targets`, full suite **4809 / 0**.

**Shape: option 1, via a hook rather than a call.** `edit_markdown` is a core tool and the
catalog lives behind `#[cfg(feature = "librarian")]`, so a direct call would compile locally and
fail CI's lean lane — the failure CLAUDE.md names explicitly. `src/util/librarian_sync.rs`
mirrors `librarian_guard`'s oracle: trait, process-wide slot, last-writer-wins for `F-51`'s
reason, no-op when unset. The two are siblings on purpose — the guard reads the catalog to
**refuse** a write; this writes the catalog after one it **allowed**.

**Why the exposed population is what it is.** The guard lets a plain catalogued file through
deliberately (`a_catalogued_but_unaugmented_file_stays_directly_editable`). Measured: **4 of 19**
live files under `docs/issues/` carry a stamped `id:`, so 15 were editable and every one could
desync. The protection was **accidental** — which is why "stamp them all" is not the fix:
`tracker-conventions` forbids stamping `id:` to guard a file, and it was tried and reverted in
`bb9a94d7`.

**Eight tests, and the ones that mattered were mutation-checked.** Four on the module core with
the hook passed explicitly; three against a real `Catalog` (status adopted, no row invented for
an uncatalogued path, silent fields preserved); one integration test in its own binary, because
the slot is process-wide and the unit-test binary contests it — server helpers install the real
syncer and would replace a recorder mid-run. Every one went straight to green, so each
load-bearing claim got a deliberate break: `status: fm.status…` → `status: row.status` fails 2 of
3 with `left: "open"`, the bug's own symptom; removing the `frontmatter_changed` gate yields
`["bug.md", "notes.md"]`; removing the call yields `[]`.

**Measured residual, stated rather than papered over.** The chain has four links and only three
are covered:

| link | covered? |
|---|---|
| `server.rs:374` installs the syncer | **no** |
| `edit_markdown` calls the sync when frontmatter changed | yes — two mutations |
| `sync_after_frontmatter_write` reaches the installed hook | yes — integration test, real global path |
| the syncer moves the catalog row | yes — real `Catalog`, mutation-checked |

Commenting out the install leaves **all eight tests green** — verified by running it, not
inferred. Closing that link needs a test that builds a real server and inspects the slot, which
is the same contested-slot problem, and it is the link `librarian_guard`'s own install has never
covered either. Suggested by `fix-embedding-transport-stage-1`, whose mutation proved the gap
rather than closing it.
### BL-49 — `workspace(post_compact)` flushes LSP without prewarming

**Status:** partial — hint + manual fixed (`ff90ce41`, patch-id `9da21228d4392923`), observed live in the release binary 2026-08-30. Its originally-prescribed fix was a no-op for its own repro. Stays open only for the cross-repo false sentence at `session-start.mjs:339`, which is still emitting.

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`. The next
navigation call pays cold start and can blow the 60s tool timeout, while the tool's own hint
promises no disruption. The misleading hint is the worse half: latency is survivable, a hint that
rules out the real cause ends the search for it.

**Worked 2026-08-30 — and the reproduction changed the fix, as the rule predicts.**
Running the record's own `## Resume` protocol before reading its `## Fix` section found
three independent defects in the diagnosis:

1. **Fix (a) was a no-op for this bug's own reproduction.** `PREWARM_LANGUAGES`
   (`src/lsp/mod.rs:25`) is `&["java", "kotlin"]`; the reported failure is Rust. "Make
   the flush path do what the activate path already does" would have changed nothing
   observable and closed the bug — the `bug-fix-session-log:W-66` shape.
2. **A workspace-keyed mux relocates the trigger.** `rust-analyzer` runs under
   `codescout mux --socket codescout-rust-mux-<hash>.sock --idle-timeout 180`, not under
   the server that uses it, so `shutdown_all` drains this server's clients and the
   language server survives. Measured: 18 minutes after a flush, with zero intervening
   LSP calls, `references` returned immediately and spawned no new process — served by a
   mux a *different session's* server had started. The record's Environment note reads
   the concurrent sessions as **contention**; they are the opposite.
3. **Fix (b) targets a sentence not in this repo.** The tool's hint promised nothing
   about cost; *"no disruption to the session"* is
   `claude-plugins/codescout-companion/hooks/session-start.mjs:339`.

Shipped the cheap half: the hint now prices the flush (cost, the condition that removes
it, and `re-run` as remedy), the manual's two stale passages are fixed, and the block
quoting the hook is left verbatim under a warning — a manual that quotes a hook has to
match the hook. One test, written and confirmed RED first, asserting the remedy and the
shared-server caveat rather than a latency.

Deferred the prewarm on stated grounds rather than by omission, and left the cross-repo
sentence named with its exact replacement. **This entry stays open for that sentence**,
which is the half this record itself calls the worse one.
### BL-50 — `expects_augmentation` records existence, not shape

**Status:** done 2026-08-31 — all three items discharged. The last ran on the host that still held the shapes, which turned out to be this desktop: its catalog had all 23, so `fix=export_augmentations` published the remaining 14 and the corpus went 9 → 23 (`1ec456a4`, patch-id `a1967b8b671cba1c`).

**Resolution 2026-08-31 — the note above was right to distrust the zero, and wrong about why.**

It offered one alternative explanation: that the declarations changed rather than the shapes arriving. Neither was the case. The declarations were untouched (13 still read bare `true`) and no shape had arrived. `augmentation_declared_but_absent: 0` was simply **correct and uninformative**: the check measures whether THIS catalog holds a row for a declaring artifact, and this catalog held every one of them. A host that can still export is exactly the host on which that check reads zero. It can never answer the travel question, on any machine — the two hosts differ, and the check only ever describes the one you are standing on.

The discriminator was the sidecar count against the declaration count: **9 against 23**. Both travel in git, so either host could have settled it without reaching the other, and no network access was needed to answer "confirm the export ran on the other host".

One case was worse than the residual described. `docs/trackers/claim-decay.md` was augmented here and declared **nothing at all** — so it was not among the 13 and raised no alarm anywhere, because `augmentation_declared_but_absent` can only fire on a file that declares. Undeclared is strictly worse than declared-but-absent: the alarm was designed to be unreachable for it. Exported and stamped, bringing the true residual to 14.

Evidence is the probe, not the export's own count: `scripts/probe_augmentation_restore.py` reports **23/23 restored into a catalog that never held them, 23 distinct prompts, `params={}` throughout** (baseline 9/9). Distinct prompts is the discriminator a count cannot be — N artifacts restoring ONE sidecar still counts N. Gate green: fmt, clippy, lean 3418/0, default 4985/0.

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-28-augmentation-declaration-records-existence-not-shape.md`. A boolean tells
a fresh clone that an augmentation is missing and nothing tells it what the augmentation WAS. The
catalog is machine-local and gitignored, so frontmatter is the only surface that could carry the
shape. Breaks CLAUDE.md's documented `entry_filter` recipes on every new machine with no recovery
path. Pairs with `docs/conventions/cross-machine-catalog-resume.md`.


**Update 2026-08-30 — mechanism shipped; three things remain, one of them time-sensitive.**

Shipped: `e799f29d` (sidecar format, `parse_declaration`, reindex attach,
`fix=export_augmentations`), `1ad9af66` (the cross-machine round trip in one test),
`f565504a` (path-keyed sidecar names), `e1b91221` (the three docs that asserted the old
invariant). Gate green 4833/0.

**The design decision, taken by the operator:** shape travels in a committed sidecar
*including* `prompt`; `params` stay catalog-only, because they are live state and
committing them recreates the drift class BL-29/BL-40/BL-42 closed. So a restored tracker
comes back working and empty.

**The sharpest edge was not the feature.** `doctor::declares_true` returned `false` for
any string outside `true/yes/on/1`, so introducing a sidecar PATH would have made
`expects_augmentation: docs/augmentations/…yaml` read as **not declared** and switched the
check off on the artifacts most carefully configured — the failure that function's own doc
comment warned about, delivered by the change quoting it. Replaced with a three-state
`parse_declaration`; anything uninterpretable is now reported rather than skipped.

**Measured here before writing anything, correcting the bug file's framing:** doctor
reports 13 on THIS machine, not only on a fresh clone. `artifact_augmentation.updated_at`
dates a restore burst on 2026-08-28 09:16–10:11 — the CM-N stream. So 22 lost on a
437-commit pull, 9 restored by hand, 13 never were. There is no catalog backup.

**Remaining:**

1. ~~**Run the export here.**~~ **DONE 2026-08-30** — `c2039a16`, patch-id
   `63a943ba8e2a1a9b`. 9 sidecars under `docs/augmentations/`, 9 declarations upgraded
   from `true` to the path they name.
2. **`artifact_augment` write-through.** A sidecar goes stale when shape changes until
   someone re-exports. Export covers backfill and is idempotent, so this is not blocking.
3. ~~**The 13, and this one has a deadline.**~~ **DONE 2026-08-31** — `1ec456a4`, patch-id
   `a1967b8b671cba1c`. The deadline framing was right and the count was not: **14**, once
   `claim-decay.md` (augmented, declaring nothing) is counted. "THERE" resolved to this
   desktop. MCP invoked it, as predicted; the CLI gap (BL-65) never bound.

**Update 2026-08-30 (later) — the export ran, and two claims were verified rather than
assumed.**

First, that the running server actually carried `f565504a`. The dry run's *names* are the
evidence, not the binary's mtime: `docs-research-README.yaml` rather than `README.yaml`
means path-keyed code is the code executing. A rebuild on disk does not change a running
process's image, so mtime is the wrong instrument here — it would have said yes an hour
before the answer was yes.

Second, that the new path-valued declarations parse. `doctor` reports **zero**
`augmentation_declaration_unparseable` across all 9, which is what distinguishes "declared,
with a sidecar" from the silent `declares_true` failure described above.
`augmentation_declared_but_absent` stays at 13 and that is correct — those never had rows
here, so they were never exportable from this machine, and the check's detail now names the
remedy instead of only the symptom.

A corpus test shipped with it: `every_committed_sidecar_parses_and_carries_no_params` reads
every committed sidecar through the same deserializer the restore path uses. Under a
hand-added `params:` key the sibling unit test `a_written_sidecar_carries_no_params_key`
**stayed green** — it asserts the *writer* never emits one and is structurally blind to a
file that already carries it. That producer-vs-consumer gap is the test's whole reason to
exist, and it is the same shape as the memory `tests-that-cannot-fail` records.

**Update 2026-08-30 (later still) — the end-to-end claim is now proven on the real corpus,
and it was never tested before.** The two halves each had a test and the *conjunction* had
none: the unit tests round-trip a **synthetic** sidecar, and
`every_committed_sidecar_parses_and_carries_no_params` proves the nine real files parse.
Both pass in a world where the real corpus restores nothing.

`scripts/probe_augmentation_restore.py` closes it — copies the nine artifacts and their
sidecars into a throwaway git repo, points a server at a catalog that has never held them,
runs one `reindex`. Result: **9 declared, 9 indexed, 9 restored, 0 errors, `params={}`
throughout, 9 distinct prompts.** That is the bug reproduced and closed, not argued.

**The first run of that probe said `restored: 0`** — and the feature was fine; the probe was
wrong. `restore_declared_augmentations` resolves each sidecar against
`current_project::lookup_git_root(path)` and `continue`s **silently** when there is none, and
the temp corpus had no `.git`. So it reported `0 restored, 0 errors`, which is
character-identical to "nothing needed restoring". Had I stopped there I would have filed a
regression against working code. The probe now runs `git init`, and that caveat is the first
line of its blind-spot list.

Also worth keeping: the **count cannot see a collision**. Nine artifacts all restoring one
sidecar still counts nine — which is exactly the `README` stem collision `f565504a` fixed. The
discriminator is distinct *prompts*, so that is what the probe asserts. `entry_collection` is
not one: two trackers legitimately share `tasks`, and an earlier draft of the probe printed
`8 of 9` under a banner reading "must be distinct", i.e. a correct run described as a failure.
### BL-51 — A rendezvous slot that misses its SessionStart stamp can never be stamped again

**Status:** dropped — the bug is `wontfix`, closed 2026-08-30. Nothing was fixed because nothing was broken: the claim this entry rested on was wrong, which its own bug file says is exactly why it should not have kept an open row here.

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-28-rendezvous-slot-never-stamped-leaves-phase-c-inactive.md`. Phase C stays
inactive for the life of that server. Sibling of BL-52; both are rendezvous lifecycle and the two
failure modes are complementary (never-entered vs never-left), so fixing one without the other
leaves the gate wrong in the opposite direction.

**Dropped 2026-08-29 on a pre-work scout, before any code was written.** Both of the file's
claims had already been refuted by its own author 90 minutes after filing: a `/mcp` reconnect
DOES inherit the predecessor stamp (a new pid's `hook_at` predated its own start by three
minutes), and the miss is not permanent — invariant 1 governs the *refresh* hook only and says
nothing about the other writer, so a later SessionStart-class event stamps it. The population
improved unaided, 3 of 8 unstamped → 1 of 6, and the survivor is `MRV-poc`, where the companion
is inactive and gate-closed is correct.

What remains is a frequency question with no known harm — the file's own words, *"not a defect
with a fix"* — plus one unexplained instance that served calls for ~8 hours with a null slot. The
code is JavaScript in `claude-plugins`, so it is not a codescout-repo change in any case.

**Pairing BL-51 with BL-52 as one piece of work was my error, corrected by the scout.** They are
opposite polarities of one mechanism, which made them look like a single fix; neither is a fix at
all.
### BL-52 — The rendezvous gate latches open

**Status:** blocked — the sketched fix is refuted (`hook_at` ages span 0.6–25h, so no window discriminates); a viable fix is cross-repo and needs a design decision. Next step is measurement, not code.

**Valid:** dated 2026-08-29

`docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md`. A companion
hook that goes quiet mid-process leaves `/clear` invisible again. See BL-51 — treat as one piece
of work.

**Blocked, not open — established on a pre-work scout 2026-08-29.** Three findings, all already
in the bug file and none of them visible from its title:

1. **The sketched fix is refuted in that form.** *"Treat the gate as open only while the last
   `hook_at` is within some window"* cannot work: measured across five healthy hook-installed
   sessions, `hook_at` ages span **0.6 h to 25.0 h**, and two of the five predate their own
   process by 7.8 h and 24.4 h through deliberate reconnect inheritance. A window would have to
   exceed ~25 h to avoid false-deactivating a healthy session, which detects nothing. Structural,
   not a tuning problem — the companion stamps only on `SessionStart`, so `hook_at` measures
   time-since-conversation-start and liveness is a different quantity.
2. **The viable fix is cross-repo and a design decision.** It needs an existing recurring hook to
   also stamp the slot, in `../claude-plugins/codescout-companion/`, at a real per-prompt write
   cost. The file names it explicitly as *"a design decision rather than a defect repair — so it
   is named here, not taken unilaterally."*
3. **The next step is measurement, not code.** Frequency has a first number — **0 of 5** stamped
   slots showed the latch-open-while-quiet state, joined against `usage.db` activity — but that is
   one ~20-minute snapshot. The file's own instruction is *re-run the join before closing*.

So the actionable item here is a longer-window frequency join, which is in-repo and cheap. Held
for the operator: it is a measurement to commission, not a defect to repair.
### BL-53 — Guide topics are atomic nodes in an unmodelled graph

**Status:** open — **cross-referenced, do not fix from here.**

**Valid:** dated 2026-08-29

`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`. 63% of the
guide corpus auto-injected in one session, and three guides cite sections the API cannot serve.
Also tracked as `resume-get-guide-section-grain-phases-2-3:GG-7`, which sequences it behind GG-1
and GG-2. Check that queue before starting; this row exists so the bug is not invisible to a
bug-first triage.

### BL-54 — Workspace `read_only` flips mid-session with no `activate` call

**Status:** investigating — **cross-referenced.**

**Valid:** dated 2026-08-29

`docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md`. Silently blocks every write.
Also `resume-workspace-pinning-phase-4b-5:WP-5`, listed there for adjacency to the write-root
split (BL-46) — the two may share a root cause in `with_project_at`, which is worth establishing
before either is designed.

### BL-55 — Three unrelated tests failed together on the wine lane under load

**Status:** open

**Valid:** dated 2026-08-29

`docs/issues/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md`. Believed genuine
wall-clock flakiness with no defect underneath — and that belief is now load-bearing, because
this session used it as the contrast class that (wrongly) explained a *real* defect: an
`embed_one_batch` `try_join!` race that load merely exposed (`bug-fix-session-log:F-78`,
fixed `21174425`). Distinguishing "flaky by wall clock" from "defect that load exposes" is the
actual work here, and this bug is the reference case for the former.

### BL-56 — SDD ledger directory and its catalog rows both vanished between sessions

**Status:** zombie 2026-08-30 — the disposition its own Resume prescribed. Hypotheses 4 and 6 acquitted from code plus live measurement, and a newly-found 9 (→ BL-64) acquitted twice. The survivor is 8 (a foreign `codex` writer), and it is **unfalsifiable, not untested**: the catalog keeps no write audit trail, so "who deleted these rows" has no answer once the window closes. Re-open trigger is in the bug file's frontmatter.

**Valid:** dated 2026-08-29

`docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md`. Data-loss class. The catalog is
gitignored, so a vanished row is unrecoverable rather than merely stale — which is what makes a
root cause worth more than a re-create. Phase 1 on that basis alone.

**Worked 2026-08-30 — closed as `zombie`, which is what its own Resume asked for.**

There was no reproduction to run (*"the loss was noticed on resuming after a compaction,
with no session running to observe the deletion"*), so the work was to settle the named
hypotheses. Three of four are now closed:

- **6 (concurrent writers) — acquitted** by the file's own criterion. `journal_mode` is
  `wal` on the live catalog; `Catalog::open` sets `busy_timeout = 5000` at both sites,
  pinned by `open_sets_busy_timeout_for_cross_process_writers`; `append_entry` runs in one
  `IMMEDIATE` transaction; migrations use `BEGIN IMMEDIATE` explicitly for the
  two-instances case.
- **4 (`prune_missing` dead-root) — rejected from the code.** `derive_dead_roots` skips
  any path that exists, and the record's own Symptom says both files were still on disk.
  The guard fires before the dead-root climb starts.
- **9 (a `force` reindex DELETE) — new, and rejected twice.** Found while chasing 4;
  filed as **BL-64**.

**The Resume's own step 2 was broken, and that is worth more than the acquittal.** It
prescribed reading `PRAGMA busy_timeout` from `sqlite3`. That pragma is per-connection,
so a fresh CLI connection reports its own default — measured `0`, against `5000` in the
code. Followed literally the recipe returns *"no timeout → largely convicts
concurrency"*, and no other answer was obtainable from it. A textbook member of
`docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`.

**Why it closes as zombie rather than fixed.** The survivor — a foreign `codex` client
writing to the shared machine-local catalog — is not merely untested, it is
**unfalsifiable after the fact**: nothing records which process mutated a row. The
incident is undiagnosed because the evidence was never captured, not because the search
stopped early, and a recurrence would be exactly as opaque. If it recurs, propose a write
audit trail *first*; re-running the hypothesis list would repeat a search whose outcome is
already known.

### BL-64 — `reindex_cli` carries a broken copy of a deliberately removed DELETE

**Status:** done — `9f743091`, patch-id `92db5adf65b7a748`. **Valid:** dated 2026-08-30

`docs/issues/archive/2026-08-30-reindex-cli-carries-a-broken-copy-of-a-deliberately-removed-delete.md`.

`src/librarian/mod.rs:392-398` still holds the pre-walk
`DELETE FROM artifact WHERE abs_path LIKE ?1` that `reindex.rs` removed from the real path
in `d482ca8a` for cascade-wiping augmentations through `ON DELETE CASCADE`. Its copy has
**no `%`**, and `LIKE` without a wildcard is equality — verified against SQLite directly —
so it matches zero rows. `force` reaches nothing else: `index_repo` takes no force
argument and hardcodes `index_repo_sync(…, false, false)`.

Harmless today, because the function is `#[cfg(test)]` and `codescout --help` lists no
`reindex` subcommand — checked against the shipped binary, not inferred from the source.

The hazard is what a future reader does to it. The block reads as an obvious missing-`%`
typo, and repairing the typo restores a cascade-delete of every augmentation under the
root. `reindex.rs`'s own comment records that arc — destructive DELETE, removed in
`d482ca8a`, then properly re-plumbed as `force_rewalk`. The CLI-named twin never left
stage one.

**Fixed 2026-08-30 — `9f743091`, patch-id `92db5adf65b7a748`.**

The record left (a) vs (b) genuinely open. One fact settled it that the bug file did not
have: **both** call sites of `index_repo` are test-only — `indexer.rs:1624` is inside
`#[cfg(test)] mod tests` (opening at `:707`) and `mod.rs:452` is inside `reindex_cli`,
itself `#[cfg(test)]` — while `index_repo` is public API via `lib.rs:39` → `mod.rs:24`. So
(b) was a semver-breaking signature change serving only test callers, to make a reference
implementation nothing references; the real path bypasses `index_repo` and calls
`index_repo_sync` directly. (a) touches only `#[cfg(test)] pub(crate)` code.

**The comment is the remedy, not the deletion** — the danger was always the next reader,
so the reasoning now stands where the block was, mirroring `tools/reindex.rs`.

`reindex_cli_never_wipes_augmentations_under_the_root` pins it, and the mutation matrix is
the result rather than the green tick: **with** `%` (the "typo fix") the test FAILS;
**without** `%` (the shipped code) it passes. It discriminates on the hazard, not on the
presence of a DELETE — and it confirms at runtime that the shipped statement was inert.
Two things stayed green under the failing mutation, which is why the assertion is on the
augmentation: the sibling `reindex_cli_indexes_repo`, and the test's own
`COUNT(*) == 1` — the count cannot see the wipe, because `id = sha256(abs_path)` means the
re-walk re-inserts the same row.

**Correction worth keeping:** the bug file's *Tests added* section said "if (a) is taken
there is nothing to assert; the deletion is the change." That was wrong. A deletion leaves
no artefact to assert on, but the **invariant it establishes** is assertable, and asserting
it is the only thing that stops the code returning. "Nothing to assert" is what a removed
guard always looks like from the inside.

One finding surfaced and deliberately not actioned: `index_repo` is `pub` on a `pub mod`
path with **no production caller at all**. Dead public API, not a defect — and removing it
would be semver-breaking for library consumers even though nothing here calls it.

Fix is (a) delete the block and the parameter, matching what the sibling actually did, or
(b) plumb `force_rewalk`/`force_embed` through `index_repo` — only 2 call sites. Not (c),
restoring the `%`.
### BL-57 — `@tool_*` buffer grep returns the JSON envelope, not the stdout

**Status:** done-archived — fixed and archived by the owning session 2026-08-30.

**Valid:** dated 2026-08-30

`docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`, tracked there as
`resume-cross-machine-catalog-restore:CM-7`. Reproduced and fixed by a peer
(`61476cb5`, patch-id `f459ee93c80aba7eab5c3f922d1a6982b0b02f24`): the buffer holds the whole
envelope on one line, `read_file` addresses by line, and that line exceeds
`INLINE_BYTE_BUDGET`, so every read overflows into a new buffer of the same shape — the
smallest addressable unit is larger than the largest returnable one, so it cannot converge.

**Correction 2026-08-30, by the owning session.** This entry previously said the bug row read
`investigating` "because the status was set through `edit_markdown` frontmatter, which is
**BL-48**". That was not true of this bug: its status was set through
`artifact(action="update", patch={status})`, which does write the catalog, and the row read
`fixed` from that point. **BL-48 is real and was hit — on a different artifact**, the
capped-get round-trip bug, where the frontmatter genuinely was set via `edit_markdown` and the
catalog row kept its old value until an `artifact(update)` repaired it. The two bugs were
adjacent in one session and got conflated; BL-48's evidence should point at that one.
### BL-58 — ListAgents omits live cross-profile sessions, and the counts are incomparable

**Status:** partial — root cause still blocked upstream (harness, not this repo), but the local mitigation SHIPPED: `scripts/peer-sessions.sh` plus a `docs/PROBES.md` row, so it fires at the moment of use rather than when someone opens the bug file.

**Valid:** dated 2026-08-31

*(Not a new judgement — the author's own class, relocated 2026-09-01. It was written
mid-line at the end of the `**Status:**` sentence above, and detection is line-anchored at
column 0, so it counted as undeclared and the entry showed up in
`entry_cited_from_outside_but_undeclared` while visibly carrying a declaration.)*

`docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`.
Discovery is scoped to the calling session's config profile; the socket directory
(`/run/user/<uid>/cc-socks/`) is machine-global. So a session in `~/.claude-sdd` writing
this very checkout is invisible to a `~/.claude` session, and vice versa.

**The count is the defect, not the omission.** Both sides report `Peer sessions (2)` over
**disjoint** sets — two agents comparing notes find their numbers agree and read the
agreement as corroboration. An incomplete count known to be incomplete is weak evidence;
two matching counts over disjoint populations manufacture confidence out of the very
discrepancy that should have exposed the gap. Measured population that day: at least six
sessions, and no single call enumerates them.

Cost, in one afternoon: six misattributions of authorship across four sessions, every one
a correct elimination over a short population. One session held a commit rather than
commit lines it believed were a peer's; another nearly filed a false confession.

Not fixable here. What IS available here is the mitigation, and it works today:
`ls /run/user/$(id -u)/cc-socks/` plus `/proc/<pid>/cwd` gives the real population, and
any session is addressable as `uds:<socket path>` whether or not it is listed — verified
by delivering to one.

### BL-59 — the buddy compact banner names a peer's session as your own

**Status:** blocked — `claude-plugins`, not this repo. **Valid:** dated 2026-08-30

`docs/issues/2026-08-30-buddy-compact-banner-names-a-peers-session-as-your-own.md`, filed
by `codescout-3b`. The SessionStart banner's `from=<sid>` reads as "this session's own
pre-compaction transcript" and can name a **different live session**.

One rung worse than BL-58, and the reason is worth keeping: BL-58 silently understates who
else is writing your files; this silently overstates what **you yourself** wrote — and a
session cannot refute it from the inside, because "you don't remember" is the hypothesis.
It nearly converted an honest denial into a false confession. What broke it was reading
the named session's actual write history and finding it disjoint.

Second datapoint, unnoticed at the time: this session's own banner named `codescout-3b`'s
sid as its origin.

### BL-60 — the CLI's `artifact create` / `artifact update` silently drop `time_scope` and `extra`

**Status:** done-archived — shipped 2026-08-30 (`0c4931ef`, patch-id `a0a4a3b4d0ea3f1b1d52e9299b9809dad98fcf05`), tests added, bug file archived. **Valid:** dated 2026-08-31

`docs/issues/archive/2026-08-30-cli-artifact-drops-time-scope-and-extra.md`. A **silent** loss on
the write path: the call succeeds, the fields are gone, and nothing surfaces until someone
diffs intent against the stored row.

It sorts above the other open items because of *which* fields it drops. `extra` is where
`unverified:`, `opened:`, `closed:` and `entry_prefix` live — the fields
`get_guide("tracker-conventions")` builds its whole queryability argument on, and the ones
that make a caveat reachable by a triage query instead of buried in prose. A write path
that drops them defeats the convention at the point of writing.

Not yet scouted. **Run the reproduction before reading its fix plan** — a 2026-08-14 sweep
changed the fix three times that way, and one of those changes inverted the direction.

**Done 2026-08-30 — `0c4931ef`, patch-id `a0a4a3b4d0ea3f1b1d52e9299b9809dad98fcf05`.**

The record said *"Read the two struct pairs side by side; there is nothing to run."*
Running it anyway cost two commands and returned something struct-reading could not:
`--force` was present while both target flags were absent, which proves the gap is in
**the code that ships today** rather than an artefact of a stale build. That is a
different claim from "the flags are missing", and it is the one that licenses the fix.

Shipped both flags on both subcommands, marshalled at the depth each tool expects —
nested in `patch` for update, top level for create — with `--extra` parsed as JSON and
`null` preserved, since `null` is how `extra` deletes a key.

Extracted `build_create_tool_args` from `run_create`. `19289b1f` had already done this
for update, and its doc comment states **this bug's own mechanism**: *"a field can
exist on the struct and never reach the tool … only testable if the translation is
reachable without a catalog."* The create side never got the same treatment, which is
precisely why the identical defect could sit there unobserved. Both halves are now
testable without a catalog.

Eight tests, all of which went compile-error → green, so three deliberate breaks were
spent per `bug-fix-session-log:W-73`. The one worth keeping past this bug is the
characterization guard: dropping `augment.prompt` from the extracted builder fails
`create_still_marshals_every_pre_existing_field`. Nothing else in 4819 tests would
have caught a field silently lost in a 60-line move — the same class of loss as the
bug, arriving by refactor instead of by omission.
### BL-61 — ZOMBIE WATCH: `references` answers a warming LSP with `symbol not found`

**Status:** open — **watch, not work.** **Valid:** conditional — until the trigger below fires

`docs/issues/2026-08-27-references-symbol-not-found-while-lsp-warms.md`, `status: zombie`:
no longer observed, root cause unconfirmed. Listed here only so the roster is complete —
which is BL-58's lesson applied to this queue, since these three sat live in the bug ledger
and invisible to the queue.

**Do not pick up.** Re-open trigger, quoted: `references` returns **`symbol not found`** for
a symbol `symbols(name=…)` resolves. A timeout and a guarded zero are both explicitly
outside it — BL-49 was checked against this trigger and is a separate bug. Its cold-start
mechanism is already refuted; do not re-file it.

### BL-62 — ZOMBIE WATCH: two Windows CI timing flakes

**Status:** open — **watch, not work.** **Valid:** conditional — until a wine/MSVC lane fails again

`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`, `status: zombie`.
Related to BL-55 by **symptom**, not by established cause. Before treating it as live,
check whether either test is already retired: `bug-fix-session-log:W-64` took the
windows-gnu skip-list from 32 entries to 8 with every survivor classified. The
discriminator BL-55 exists to draw — "flaky by wall clock" vs "a defect that load
exposes" — applies here too.

### BL-63 — ZOMBIE WATCH: `symbols` search 0-matches then succeeds on retry

**Status:** open — **watch, not work.** **Valid:** conditional — until the instrumentation records a recurrence

`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`,
`status: zombie` and already partly closed: Bug A fixed in `b2344aab`, Bug B mitigated and
**instrumented**. Read what the instrumentation has recorded before treating it as live —
that is the cheapest available evidence and it did not exist when the file was written.

Worth more than its phase-3 priority suggests if it does recur: retry-succeeds is exactly
what a false negative looks like from the caller's side, which is this project's
most-repeated law (`reconnaissance-patterns` law C, a zero that lies).
### BL-65 — the CLI's `doctor` exposes no `--fix`, so all six repairs are MCP-only

**Status:** open — unowned, self-contained, phase 1.

**Valid:** dated 2026-08-30

`docs/issues/2026-08-30-cli-doctor-exposes-no-fix-flag.md`. `librarian(action="doctor",
fix=…)` offers six repairs; `codescout doctor` offers none — its args are `--project`,
`--json`, `--no-color`, `--fail-on-violations`. The subcommand's own help calls it a
"Read-only scan", which is accurate about the CLI and describes half of what `doctor` is.

**Third instance of one mechanism in one day.** The CLI keeps its own clap structs and
hand-marshals into the tool's JSON; `Args` carries no `deny_unknown_fields` and every
optional field is `#[serde(default)]`, so a param on one surface is **defaulted in
silence** on the other rather than rejected. Preceded by the `--force` gap (`19289b1f`,
archived) and BL-60 (`0c4931ef`).

**Why it is worth more than its size.** It strands `fix=export_augmentations` (BL-50),
which exists specifically to be run on the machine whose catalog still HOLDS augmentations
this one lost — and the natural interface on that machine is the shell. Still reachable
there via MCP, so this is a usability gap and not a dead end; the file says so rather than
overstating it.

**Root cause is marked INFERRED** — read off `--help`, not from `DoctorArgs`. Reproduced by
running the binary rather than reading the struct, on BL-60's lesson that a bug file's
"nothing to run" is a claim rather than an instruction.

**The remedy two independent findings now point at**, and it is not a fourth round of
flags: a test asserting the CLI's marshalled key set covers the tool's `Args` field set.
That closes the mechanism instead of its next instance. BL-60's Resume argues the same.
### BL-66 — `probe_ollama` is the one TLS site that installs no crypto provider

**Status:** done-archived — fixed 2026-08-30, patch-id `90c1612bcd948c09e0fd373be2e754134bf9a463`. Verified at the bytes 2026-08-31: `probe_ollama` installs the provider before any client construction (`crates/codescout-embed/src/remote.rs:619`), guarded by its own single-test binary — and that binary is now reached by the gate (see BL-68).
**Valid:** dated 2026-08-30
**Rests on:** `docs/issues/archive/2026-08-30-probe-ollama-is-the-one-tls-site-that-installs-no-crypto-provider.md` (`a7af9964a16e8056`).

Found by taking up a peer's recommendation to audit other root/crate pairs for the
asymmetry behind the two bugs fixed on 2026-08-30 — the short-response panic and the
status-error classifier hijack. The audit's other pairs came back clean, and are worth
recording as such: the dense transport is now delegated so the pair no longer exists;
the sparse leg is root-only; `is_https_or_loopback` is duplicated but byte-identical in
logic, with root's coverage gap closed by `28bb6e8a`; the chunker is delegated; and the
`read_timeout` port that ET-4 predicted T6 would regress **did** happen, landing both
bounds in the crate with a comment explaining that neither subsumes the other and a test
that fails if `.read_timeout()` is removed.

The one asymmetry left is not a duplicated pair at all, which is why the pairwise audit
nearly missed it. `probe_ollama` bypasses `build_client` and is the only TLS construction
site in either tree that does not install the crypto provider first — found by
enumerating call sites of `install_default_crypto_provider` and noticing which
client-builder was absent from the list. `OLLAMA_HOST` is operator-controlled and may be
`https://`; the probe runs **before** `RemoteEmbedder::ollama`, so it is the first
handshake, and with `rustls-no-provider` it fails. The caller then reports *"Ollama is not
reachable … Start Ollama"*, which misdiagnoses the cause and prescribes a remedy that
cannot work.

codescout's own binary is immune because `main.rs:253` installs the provider at startup,
process-globally. That immunity is the *mechanism*, not a mitigation: root's behaviour is
what hides the gap, so the crate ships it to every external consumer while the one caller
that would notice is shielded — the same structure as the other two instances.

**It also contradicts ET-4's stated direction.** That entry reads *"root's copy is the one
missing the guard, both times"*, and treats deleting root's duplicate as the remedy. All
three instances found on 2026-08-30 have the crate as the deficient side, so the drift is
bidirectional and ET-4's "audit each pair before deleting" caveat is the load-bearing part
of it rather than a footnote.

**Next:** run the reproduction (a consumer binary depending only on the crate, no provider
install of its own, `OLLAMA_HOST=https://…`) before choosing between (a), (b) and (c) — it
decides whether rustls surfaces a distinguishable provider error at all.

### BL-67 — `export_augmentations` will not rewrite a sidecar whose shape changed

**Status:** done-archived — fixed 2026-08-30 by `5f88be65`, patch-id `59ba22f9d7a6dfed66fcd8e551e09455b5c58f32`; bug file archived.
**Valid:** dated 2026-08-30
**Rests on:** `docs/issues/archive/2026-08-30-export-augmentations-will-not-rewrite-a-sidecar-whose-shape-changed.md` (`689fb62e40557480`); `BL-50` item (2), where this was first recorded as a known consequence.

BL-50 recorded this as non-blocking on the reasoning that "the export is idempotent".
Idempotent is exactly the problem: the export satisfies a sidecar's **existence**, and
re-running it on a changed shape is a no-op that reports success.

Found while widening this queue's own `status` enum from 7 values to 10 (`2a8decc5`). The
catalog took the change immediately — `artifact(get)` and `doctor` both saw the new enum,
and `params_status_drift` went 6 → 1 on the strength of it. `fix=export_augmentations`
then reported `exported: 0`, and the committed YAML still carried the seven-value list at
line 68 along with a prose prompt restating the old vocabulary. Had that shipped, the next
machine's `reindex` would have re-attached the stale shape and silently invalidated every
row written under the new one.

The reason it is split from BL-50 rather than folded into it: BL-50's remaining item is
**structurally unclosable from this machine** (a shape whose only copy is another
machine's gitignored catalog), while this is a write-path defect with a cheap local
reproduction. Two items with the same origin and unrelated remedies do not belong on one
row.

**The fix is a design question, not a one-liner, and the obvious answer is probably
wrong.** Unconditional re-export would clobber a sidecar that is legitimately ahead of the
catalog — which is precisely BL-50's restore path, where the committed shape is the only
surviving copy. Catalog-ahead means an edit did not travel; sidecar-ahead means another
machine's shape has not been adopted yet. Only the first is a defect, so the primitive is
probably a `doctor` check reporting divergence in both directions with the write behind an
explicit fix.

**Next:** run the reproduction — it distinguishes an existence check from a broken
content comparison from a deliberate no-clobber guard, and those need different fixes.


### BL-68 — 37 tests in `crates/codescout-embed/` are not compiled by either gate test command

**Status:** done — closed by the 2026-08-30 gate swap to `cargo test --workspace`, confirmed by re-measurement 2026-08-31 rather than inferred: that lane runs **56** `codescout-embed` tests (55 lib + the 1 integration binary, `ollama_probe_installs_its_own_crypto_provider`), **52 executing**, 4 `#[ignore]`d — identical to the `-p codescout-embed --features remote-embed` column below. The 37-test hole is closed.

Note which way it closed, because the condition below did not fire: no fifth gate command was adopted and the crate's features did not change. `--workspace` was *swapped in* for the bare form, a route the condition did not name — so this entry would have stayed open indefinitely on its own trigger.

**Valid:** dated 2026-08-31

Measured 2026-08-30, both numbers from a clean run:

| command | tests run in `codescout-embed` |
|---|---|
| `cargo test` (gate cmd 3) | **0** — builds only the root package's targets |
| `cargo test --workspace --no-default-features` (gate cmd 4) | **19** — member built with `remote-embed` **off** |
| `cargo test -p codescout-embed --features remote-embed` | **56 compiled** (55 lib + 1 integration), **52 executing** |

**37 compiled tests are invisible to the documented gate, and 33 of them are live guards** — the
other 4 are `remote::tests::ollama_*`, `#[ignore]`d pending a running Ollama, so they are not
coverage either way. Both numbers are recorded because they answer different questions, and
because a peer and I independently produced one each: they measured 56 and I measured 52, and
the comfortable explanation on offer was "the tree gained tests between the two runs". It had
not — a re-run an hour later returned byte-identical counts. The gap was passed-vs-total, and
accepting the timestamp story would have retired the discrepancy without either of us noticing
that an `#[ignore]`d test was padding a coverage number. **It is not a `tests/`-directory
property** — that was the first hypothesis and it is wrong: `remote::tests`, an inline
`#[cfg(test)]` module, is 32 of the 33. The property is *a workspace member's feature-gated
tests*, whatever form they take.

The 33 include regression guards for three bugs fixed on 2026-08-30 —
`a_short_response_errors_instead_of_panicking`,
`a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever`,
`an_oversize_batch_is_split_into_batch_size_requests`.

**CI is not affected.** Its `default` matrix lane runs `cargo test --workspace ${{ matrix.config.flags }}`
with `flags: ""`, which covers all 52. So this is `W-81`'s axis — *how long until the check tells
someone* — not an absence of coverage. A locally-green gate is silent about 33 tests and the
signal arrives at push time.

**A near-miss worth recording with it.** The first version of this finding was going to be
"no gate lane runs it at all", which is strictly worse and was stated confidently. It came
from grepping `ci.yml` for lines containing `--features` / `--no-default-features`, which
returned the two matrix configs that have them and silently dropped the third, whose `flags`
is the **empty string**. A filter is a hypothesis about where the thing lives, and an empty
value is exactly what a pattern-based filter cannot see. Third instance that day of a search's
**scope**, not its pattern, being the thing that was wrong.

**Not applied, deliberately.** The remedy is probably a fifth gate command — `cargo test
--workspace`, which subsumes bare `cargo test` and would keep the gate at four. But the gate
is a ~20s contract in `CLAUDE.md` that every session pays on every task, and changing it is
the operator's call, not a session's. Surfaced with the measurement attached.

**Found independently by two sessions the same day**, which is the argument that it is
structural rather than one person's oversight. One commit message (`236f31a4`) cited a gate
that did not run its own new test — accurate about what was run, misleading about what it
proved.

**Resolved 2026-08-30 — the operator took the swap.** `cargo test` → `cargo test --workspace`
in `CLAUDE.md` § *Development Commands*. A swap, not a fifth command, so the gate is still four.

The safety argument is a **set difference, not a count**: `cargo test -- --list` and
`cargo test --workspace -- --list`, sorted, with `comm -23` — **0** tests present in bare and
absent from workspace (4924 → 4980 listed). Counting alone would have been the weaker check,
and this entry exists partly because a count was mistaken for a measurement twice already.

Cost **+2.6s** warm: 23.5s → 26.1s, two runs of each, stable to ~60ms. The **first**
measurement read bare at 48.0s and workspace at 25.9s — i.e. workspace apparently *faster* —
which was a compile-order artifact: the first command paid the build. Discarded and re-run
warm rather than reported. Worth recording next to the `#[ignore]` and empty-flag cases as a
third way a timing number can be confidently wrong.

Live prescriptive surfaces updated in the same commit so the gate does not drift across them:
`CONTRIBUTING.md` § *Before Submitting a PR*, `docs/RELEASE.md` § *Release Cycle* and
§ *Sequence*, and the shipped prompt guide `src/prompts/guides/tracker-conventions.md` — whose
archive-trigger gate list was independently stale on clippy, and which now **points at
CLAUDE.md** instead of restating a list that can rot on its own. Archived bug files recording
which gate *they* ran were left alone: historical records, per `get_guide("tracker-conventions")`.
### BL-45 — Decision 1: may a process on an unlinked binary re-index?

**Status:** done — the decision was taken and shipped: `22f8b8d5`, patch-id `fd2c453b…`. `guard_stale_binary` refuses **before the embed pass** (not at the sidecar write, the inversion this entry identified) on both `sync_project` and `sync_worktree`; hard refusal as a `RecoverableError` naming `/mcp`; 5 tests, wiring mutation-checked; live from the 2026-08-29 rebuild. The prose below predates the decision and states the question it answered — read it as history, not as an open ask.

**Valid:** dated 2026-08-29

**Rests on:** `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md` § *Re-costed 2026-08-28 — direction 2 is INVERTED at one of its two call sites*.

Direction 2 of the zombie-server bug, as drafted, refuses the **sidecar write**. That is
inverted at the main sync path: the vectors are already in the store by then, so refusing
destroys the evidence and keeps the damage. The corrected shape refuses **before the embed
pass** — decline to re-index, not decline to record.

The detector already ships: `exe_is_deleted()` and `WriterProvenance.exe_deleted` in
`src/retrieval/index_state.rs`. Only the policy is missing.

**Ordering re-verified independently 2026-08-29**, and the bug file's evidence table needs
one correction. It attributes the vector write to `sync.rs:904`, but line 904 is
`flush_pending` inside **`sync_worktree`**, not the main path. On the `sync_project` path
the vectors land *inside* `stream_index` (its own `flush_pending` calls at `sync.rs:620`
and `:626`), which is invoked at `sync.rs:1017`; the sidecar write is at `sync.rs:1065`,
gated by `opts.record_index_state` at `:1056`. The conclusion is unchanged and slightly
strengthened — vectors still precede the sidecar write — but the cited line belongs to the
other call site.

The two call sites do genuinely disagree on ordering, as the bug file says:
`sync_worktree` writes its sidecar at `sync.rs:839`, **before** `flush_pending` at `:899`
and `:904`. So no single rule at the writer is correct for both.

**Implementation constraint.** `exe_deleted` is `Option<bool>`, where `None` means "could
not tell" (non-Linux, where `/proc/self/exe` does not exist). The gate must fire only on
`Some(true)`. Reading `None` as deleted would make every non-Linux platform refuse to
index — the field's own doc comment states this rule.

**Decided 2026-08-29 by the operator: refuse.** A process on an unlinked binary
declines to re-index and returns a `RecoverableError` naming the remedy — if the
index is genuinely needed, the operator restarts the MCP server. The strict
option was chosen over the narrower config-divergence gate and over
warn-and-proceed, with the cost understood: on this host it stops 6 of 8 running
servers from indexing, and because a rebuild invalidates every running server
instantly, that cost lands on every `cargo rb`.

**Implemented the same session.** `guard_stale_binary(exe_deleted: Option<bool>)`
in `src/retrieval/sync.rs`, called as the first statement of BOTH indexing paths
— `sync_project` (ahead of the index lock) and `sync_worktree` (ahead of the
dirty-set sidecar). Wiring both is the point: the two disagree on sidecar
ordering, so a guard at the writer could never be correct for both, and a zombie
barred from one path must not simply take the other.

A `writer: Option<WriterProvenance>` seam was added to `SyncOpts` and to
`sync_worktree`, documented as a test seam alongside the existing
`index_lock_dir` one: detection reads this process's own `/proc/self/exe`, which
a test cannot make say "deleted" without unlinking the running test binary.
`None` is every production caller.

Five tests: three on the policy (`Some(true)` refuses; `Some(false)` and `None`
do not — `None` separately, because it means "could not tell" on non-Linux and
reading it as deleted would refuse to index on every such platform), and one
wiring proof per path asserting the refusal precedes even the index-lock
acquisition.

**The wiring test was mutation-checked, not assumed.** It went from compile
error straight to green, so it had never been observed to fail for a behavioural
reason — the vacuous-assertion shape. Replacing the call with
`let _ = guard_stale_binary(...)` made it fail with exactly the symptom its doc
comment predicts, `Ok(SyncReport { added: 0, .. })`; then restored.

**Gate:** `cargo fmt`, `cargo clippy --workspace --all-targets --features
local-embed -- -D warnings`, and `cargo check --no-default-features` all clean;
full `cargo test` 4642 passed / 2 failed, both failures in two peer sessions'
uncommitted files (`crates/codescout-embed/src/remote.rs` and
`src/tools/read_file.rs`), neither in the three files this change touches. The
clippy gate earned its keep here: `src/bin/sync_project.rs` builds `SyncOpts`
exhaustively, and only `--all-targets` reaches a bin target.

**Known hazard, not yet addressed.** `writer: None` means live detection, so a
running test process whose binary is replaced by a concurrent rebuild would read
its own exe as deleted and its sync tests would begin refusing. Did not occur in
this run, but three sessions sharing one checkout is the shape that would
trigger it.
**Shipped `22f8b8d5` on `experiments`**, patch-id
`fd2c453bfd381f63cd8fb6f773efdab30551feed`. The pair is recorded once, at fix time, because an
`experiments` SHA orphans on the next rebase while the patch-id is a content hash of the diff and
survives both rebase and cherry-pick — so nothing is owed later and no session has to reconcile a
promotion path.

**Live from the 2026-08-29 rebuild.** One consequence worth stating plainly, since it changes the
ordinary edit-build loop: from the *next* `cargo rb` onward, this and every other already-running
server will decline to re-index until restarted. That is the decision working, not a regression.
Reads and navigation are unaffected — only indexing refuses.
### BL-46 — Decision 2: the write-root split

**Status:** not-started — needs a `last_writable_root` field plus write-awareness in `with_project_at`; ~4,600 tests sit on that primitive.

**Valid:** dated 2026-08-29

**Rests on:** `docs/trackers/bug-ledger-resume-2026-08-28.md` § *Two decisions waiting on a human*.

Answered in principle 2026-08-28: a read-only activation means *exploring only, no
writing*. One implementation was tried and **reverted**, with the blast radius measured —
making a read-only activation resident-but-never-default broke **14 tests**
(`activate_replaces_previous_project`, `is_home_false_after_switching`,
`activate_hint_shows_switched_when_away_from_home`, …). Those tests are the specification
of browse-by-activate, and the change would have forced a `workspace=` pin on every call to
explore the repo just activated — which is not "exploring only", it is "not exploring".

The remaining gap is narrower, and is the whole change:

> Unpinned **writes** should resolve to the last writable root, while unpinned **reads**
> keep resolving to the activated one.

Both go through a single `with_project_at` today, so this needs a second field
(`last_writable_root`, set only on a writable activation) plus write-awareness at
resolution. `call_content` already computes `is_write(&input)` at the choke point, so the
signal exists; the work is threading it through a core primitive that ~4,600 tests sit on.

### BL-44 — a params row can drift out of sync with its body counterpart with no check on either side of that direction

**Valid:** conditional — until a drift check compares body and params field-by-field rather than by id

The claim is that a gap exists, so it holds only while the gap does. Partly narrowed since
it was written: `doctor`'s `params_status_drift` now compares the **status** field on both
sides (it fired on `claim-decay:DC-2` on 2026-09-01 and was correct). Every other field is
still set-difference-on-ids only, so the entry stands. Declared 2026-09-01.

Surfaced while executing BL-42's data repair: diffing `windows-platform-support.md`'s body table against `params` field-by-field (not just by id) found 7 rows present on both sides with different content — `WIN-1`, `WIN-4`, `WIN-5`, `WIN-20`, `WIN-27` had a stale pre-archive `ref`; `WIN-28` and `WIN-29` were worse, `params` held an earlier `open` snapshot with a superseded root-cause summary while the body already carried the resolved `fixed` story. This tracker's own `entry_filter={"status":{"eq":"open"}}` convention would have returned two closed issues as open, with the wrong explanation.

`doctor`'s `params_behind_body` (BL-40) does not cover this: it computes set difference on ids, so a row present on both sides with disagreeing fields passes it silently. `update_entry` already warns one direction (`snapshot_stale`, params-changed/body-didn't) but nothing scans the other direction — a body edit that never touches params leaves no trace.

Repaired the 7 known-bad rows via `update_entry` (safe — every row already existed) and re-verified by diffing all 35 `WIN-N` rows field-by-field: zero mismatches remain. Left open as a design question whether the fix should be a `doctor` check (report-only, matching `params_behind_body`'s shape) or a write-time guard (symmetric to `snapshot_stale`, firing on a body edit instead of a params edit). Status `dropped` here because nothing is queued to execute — see `docs/issues/archive/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` (id `640c4fc65a64461c`, `mitigated`) for the full evidence and the two candidate shapes.

One section per BL-N, and its only job is to **define the token**. `link_scan` derives a citable
definition from a `## <ID> — <title>` heading and from nothing else, so until 2026-08-18 this
ledger — which defines the queue everything else cites — could not be cited at all: 117 BL-N
references from 37 other files resolved to nothing, and the table above defined none of them.
Deliberately terse; the table holds status, phase and bug id, and `next` (catalog-side) holds the
working notes.

**When you add a BL-N, add its section here too.** Not optional and not the table — confirm with
`librarian(action="doctor")`, which reports `ledger_defines_nothing` / `entry_without_definition`.
See `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`.

### BL-1 — json_path: add a Segment::Wildcard arm so the overflow hint's own recovery works
**done**

### BL-2 — grep: stop printing a self-refuting "Showing N of N" when collection hit the cap
**done**

### BL-3 — Tool schemas: stop advertising conditionally-required params as optional
**done**

### BL-4 — usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer
**done**

### BL-5 — librarian: split tracker_design so its guidance arrives inline
**done**

### BL-6 — read_file: give the buffered full-read summary an incompleteness signal
**done**

### BL-7 — Write-scope denial should name approve_write
**done**

### BL-8 — truncate_compact cuts from the tail, destroying the overflow signal
**done**

### BL-9 — server_instructions arrives truncated mid-word, dropping the guide pointers
**done**

### BL-10 — audit_doc_refs reads bare comment markers as file paths
**done**

### BL-11 — context and workspace_state_at never dedup the worktree overlay
**done**

### BL-12 — worktree divergence guard covers writes but not reads
**done**

### BL-13 — IL1: run subtract-and-measure on the step-3 wording
**done** — ran as prompt-hamsa A-25 and the clause LOST; reverted behind an inverted guard.

### BL-14 — read_file: force=true silently discarded on whole-file reads
**done**

### BL-15 — read-only metadata commands blocked on source paths
**done**

### BL-16 — worktree activation diverges memory set and sub-project topology

**open** — but for one half rather than two. The **topology** half is closed
(`1869adcb`): `load_discover_settings` reads through to the main checkout's
`workspace.toml` when the worktree has none, which is option 3's goal without its
file-sync cost, and the activation notice gained a third topology state
(`inherited`) so it stops claiming discovery "ran with defaults" about a walk that
inherited main's.

What remains is **option 1 only**, and it is a decision rather than an
implementation: should a worktree serve the MAIN checkout's memories (the
librarian-overlay precedent — overlay onto main, fork on first write) or its own
commit's? Option 3 is spent; do not re-raise it.
### BL-17 — reconcile a bug that sits in archive/ while still marked status: open
**done** — measured 0 instances; the bug file it was filed against is itself gone.

### BL-18 — artifact(create): augment silently discarded five of its seven fields
**done**

### BL-19 — overflow envelopes with no compact summary waste a whole call
**done**

### BL-20 — params merge-patch wipes entry arrays wholesale
**done** — gave entries an update path (`update_entry`) plus always-on counts.

### BL-21 — edit_file's replace_all and batch paths write managed artifacts unguarded
**done**

### BL-22 — reindex re-keys moved artifacts and cascade-deletes their events
**done** — one reindex destroyed 11 events while reporting `removed: 0`.

### BL-23 — a moved artifact's frontmatter still asserts its pre-move id
**done**

### BL-24 — usage.db records a sha that need not describe the built code
**done**

### BL-25 — the byte cap evicts rules into get_guide topics nothing triggers
**done** — 7 of 10 guides (~46 KB) had no trigger at all.

**Valid:** dated 2026-08-18

### BL-26 — get_guide("librarian-runtime") said a move preserves the id
**done** — one fact in four files; an earlier pass repaired three and missed this one.

### BL-27 — update_entry's entry-param guard only fires when fields is absent
**done**

### BL-28 — a directory named `--help` sits untracked in the repo root
**done**

### BL-29 — append_entry writes catalog-only state, so the committed snapshot drifts

**Valid:** conditional — until the snapshot gate reaches majority coverage

Its own status line names the condition: drift is now reported at write time and by
`doctor`, and 0 trackers are adrift, but the gate still needs majority coverage. When that
lands the entry is spent, not merely older. Declared 2026-09-01.
**open** — partial: drift is now reported at write time and by `doctor`, and 0 trackers are adrift; the gate still needs majority coverage.

### BL-30 — FRICTION: adding one tracker entry costs four bookkeeping sub-tasks
**open** — the hand-allocation root cause behind BL-32.

### BL-31 — grep: cap_grouped's file-diversity round-robin is unreachable
**done** — overflow hints named walk-order files rather than hot ones.

### BL-32 — reconnaissance-patterns.md reused nine ids for unrelated lessons
**open** — split by suffix already; the hand-allocation cause is BL-30.

### BL-33 — the librarian guard keys on YAML quoting, leaving trackers unprotected
**done** — 15 of 27 trackers, this queue included.

### BL-34 — repairing a frontmatter id re-serializes the whole block
**done**

### BL-35 — guard_worktree_write is dead code in production
**done** — the startup-cwd fallback sets the very flag it gates on.

### BL-36 — artifact(update) re-serializes the whole frontmatter block on a single-field patch
**done** — BL-34's mechanism, firing at the mandated archive step.

### BL-37 — Kotlin warnings, workspace table and Custom Instructions cannot fit the channel
**open** — ordering fixed the common case; the two oversized blocks need a new carrier.

### BL-38 — the librarian guard is blind to any artifact whose frontmatter omits `id:`
**done** — fixed by teaching it the `entry_prefix` ledger declaration. Its "26 of 66 unprotected" framing was later retracted, and the `id:`-stamping remedy it suggested was reverted in `bb9a94d7`.

### BL-39 — the two sanctioned entry formats are not equivalent
**done, archived** — a params-rendered index defines no citable token. All steps shipped, `doctor` verified (`ledger_defines_nothing` 10→2, `entry_without_definition` 3→1). The blocking peer file committed and was archived; the bug itself moved to `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (id re-keyed `d34dfcd2cc718bd8` → `9dc28c0860b214d9`), all live citations re-pointed in the same commit.

### BL-40 — every drift check asks whether the body kept up with params, never the reverse
**done** `87f3b936` — `params_behind_body` in `doctor`, same two sets subtracted the other way. Ids only; the message names `append_entry` and says explicitly not to re-render from params, because that is `snapshot_drift`'s remedy and here it would delete the newer record. Deliberately **not** gated on `body_keeps_snapshot` — that gate is right for the row question and would silence a body id the catalog has never seen, which is the whole finding. Also extracted `params_backed_ledgers`, since this would have been the third hand-rolled copy of a 45-line preamble shared by three checks that must agree on what a ledger is.

Found by nearly publishing from the stale side: `windows-platform-support.md` had 29 params rows against 35 in the body, with two statuses stale, and `append_entry`'s `warning` was the only surface that could see this direction — only during an append, which that ledger had not had since the divergence. On its first live run the check fired **twice**: the WIN case, and `mirela/…/solver-invariants.md` at 10 of 68, a different repo no surface had ever reported. Data repair for both → **BL-42**.
### BL-41 — link_scan's dangling count is prefix-gated, so a whole namespace can read as healthy
**done** `ff088630` — 129 dead `WIN-N` citations moved the project total by zero. The gate is right in intent (it suppresses `CI-2`-shaped prose) and wrong in discriminator: it cannot tell "not a namespace" from "a namespace that is wholly broken". Fixed by gating on the `entry_prefix` **declaration** as well as on observed definitions.

The wiring is the load-bearing choice and it is not the one the bug file proposed: the declaration rides on `DocExtract.declared_prefixes`, populated inside `extract()`, which already holds the whole file text including frontmatter — so **there is no wire for a caller to forget**. Threading it through `DefinitionIndex::build` would have made an omission a silent no-op and touched 11 test call sites. Pinned by an end-to-end test that runs a real row-only body through the real extractor into the real index, the only one of five that fails if either half drops its end.

**Still open on this entry:** the predicted dangling RISE is unmeasured. `link_scan` runs in the MCP server and has no CLI, so it needs `cargo rb` + `/mcp` first. Pre-fix baseline, taken on the old binary right before the rebuild: dangling **548**, ambiguous 410, 3,649 citations, 1,055 artifacts, `edges_added` 0.

### BL-42 — DATA REPAIR: rows live in a body and in no params row, so params-based queries miss them
**open, scoped to codescout 2026-08-18.** Surfaced by BL-40's `params_behind_body` on its first live run, so this is the fix proving itself rather than fresh breakage.

**Ours:** `windows-platform-support.md` WIN-30, WIN-32..WIN-36 (6 of 35). Left stale on purpose at an earlier session close so the new check had something true to find; that duty is discharged.

**Corrected on two counts 2026-08-18, both found by trying to execute the repair rather than by re-reading the filing.**

- **Severity is LOWER than filed.** The heading above and the original text said the ids could be reissued. They cannot, today: `append_entry` allocates `params_next.max(body_max + 1)`, and `body_max` folds in the very rows this check reports — it would mint `WIN-37`. `append_entry`'s own comment says so outright (*"Folding the body's max in makes the reissue impossible instead of silent"*). Reissue becomes possible only after a compaction moves those rows to an archive companion, and this ledger carries no committed `entry_high_water_WIN`, so that is the real risk — conditional, not current. The present harm is query invisibility plus a committed body that disagrees with the catalog.
- **Difficulty is HIGHER than filed.** It is not 6 `append_entry` calls. Neither `append_entry` (which overwrites `entry["id"]` with its own allocation) nor `update_entry` (which patches an existing row and never changes the row count) can create a row at a GIVEN id. The repair needs the wholesale write — `artifact_augment(merge=true, params={issues: […all 35 rows…]})`, or the CLI's `--params @<file>` past the inline budget. **Build the full array programmatically, not by hand:** a params patch replaces the collection, and re-typing 29 existing rows is exactly the transcription risk that took another tracker from 19 entries to 1.

Verify: `doctor` `params_behind_body` 2 → 1.

**Handed off:** `mirela/backend-kotlin/docs/trackers/solver-invariants.md` SI-59..SI-68 (10 of 68) — never reported by any surface before BL-40. Same two corrections apply to it. Recorded here only so the finding is not lost with this queue's scope change; it will be solved in that repo.

**Codescout half DONE 2026-08-18.** Built the full 35-row array programmatically — the existing 29 read back from `params`, the 6 missing rows parsed mechanically from the body's own `## Issue index` table (never hand-retyped) — and wrote it via `artifact_augment(merge=true, params_path=...)`. Verified `doctor` `params_behind_body` 2 → 1; the remaining hit is mirela's, per the handoff above.

**A second, distinct defect surfaced while building that diff.** Comparing the body table against params row-for-row (not just by id, but field-by-field) found **7 rows present on both sides with different content**: `WIN-1`, `WIN-4`, `WIN-5`, `WIN-20`, `WIN-27` had a stale pre-archive `ref`; `WIN-28` and `WIN-29` were worse — `params` held an earlier `open` snapshot with a superseded root-cause summary, while the body already carried the resolved `fixed` story. A canonical `entry_filter={"status":{"eq":"open"}}` query — the exact pattern this tracker's own augmentation prompt recommends — would have surfaced two closed issues as open, with the wrong explanation. Repaired via 7 `update_entry` calls (safe: every row already existed, no wholesale rewrite needed) and re-verified by diffing all 35 rows field-by-field — zero mismatches remain. Filed separately as `docs/issues/archive/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` (id `640c4fc65a64461c`, `mitigated`): the seven rows are fixed, but nothing detects this class of drift — a row present on both sides with disagreeing fields — so it can recur the next time a body table is edited without a matching `update_entry`.
### BL-43 — complete BL-41's coverage: an undeclared, undefined ledger is still invisible
**dropped from this queue 2026-08-18 — handed off, not withdrawn.** Both remaining targets are other repos (stefanini `CR`×8, researcher `T`×2).

**Codescout's half is already done.** `entry_prefix` is declared on all four ledgers backfilled in `c7bdfd22`, and every one of the nine prefixes declared in this repo now has at least one defining heading. Nothing here is affected.

**The finding stands** and lives in the BL-41 bug file: a ledger that declares no `entry_prefix` AND defines nothing is still invisible to the dangling gate, so BL-41's retrospective coverage is incomplete by construction — measured, not predicted. Its prospective value is unaffected: the next ledger created here with `entry_prefix` and row-only entries dangles loudly.
#### Measurement (2026-08-18) — recorded while BL-43 was still open

**open** — measured 2026-08-18, and it is the honest counterpart to BL-41's result. BL-41's marginal effect on the current corpus is **zero**: all nine declared prefixes (`GF`, `CAP`, `U`, `H`, `FND`, `T`, `R`, `SD`, `HY`) now have at least one defining heading, and `SD`/`GF`/`FND`/`T` got their declaration and their headings in the *same* commit (`c7bdfd22`), so the widened gate never had a case to fire on.

The surviving hole is concrete rather than hypothetical: `stefanini/…/june-fixes-review-followups.md` holds 8 `CR` entries, defines none of them, and declares no `entry_prefix` — so a wholly-broken namespace is **still** invisible, which is the exact defect BL-41 reports, surviving its own fix. The bug file predicted this limit ("improves coverage without completing it"); it is now instantiated.

Remedy is one frontmatter line per ledger. Both targets are outside the working dirs of the session that shipped BL-41, so it needs the owner's go-ahead — and it shares its two files with BL-39 step 4's remainder, so doing them together makes one citation sweep instead of two.
### BL-69 — repair the 3 files with an unterminated fence

Found by the `unterminated_fence` check shipped at `800f1dec`. Three catalogued plans reach
EOF with a fence open, so every line below the opener is read as code by line-anchored
scans:

| file | opener |
|---|---|
| `docs/superpowers/plans/2026-02-28-prompt-injection-design.md` | L180 |
| `docs/superpowers/plans/2026-05-01-call-graph.md` | L1326 |
| `docs/superpowers/plans/2026-06-19-codescout-pi-integration.md` | L466 |

Each needs a **close-vs-delete judgement read from the surrounding prose** — which is why
the check ships with no `fix=` mode and why the repair was deliberately not folded into the
commit that found them.

### BL-70 — catalog-integrity sweep: the 3 in-repo doctor findings

**Three, not six** — half of what `doctor` reports under these checks is in *other* repos
and is not codescout's to fix: two `worktree_scoped_row` under
`/home/marius/work/claude/whatsapp`, one `params_status_drift` in `eduplanner-ui`. Counting
before partitioning is the whole point of this row.

- **`entry_without_definition`** — `docs/trackers/provenance-subsystem.md`, 38 of 68 `PV-N`
  rows lack a heading *in this file*. **Read before fixing:** the finding itself says all 31
  cited ones are defined in a sibling artifact, which may be the intended split.
- **`params_behind_body`** — `docs/trackers/test-escape-hardening.md`, `I-8` is anchored in
  the body with no `params` row, so it is absent from every `entry_filter` query.
- **`params_status_drift`** — `docs/trackers/claim-decay.md`, `DC-2` params says
  `check-shipped` while its body `Status:` says `fixed-once`.

### BL-71 — triage the link-graph findings, and test whether the volume gate discriminates

26 findings, but the likely shape is a **check defect, not 26 citation repairs**.

`cited_prefix_with_no_definer` (14) is dominated by prose acronyms: `SHA-N` is `SHA-256`
(19 citations), `UTF-N` is `UTF-8` (20), plus `RFC-N`, `GPT-N`, and `N-N` — the literal
`PREFIX-N` people write when *describing* the convention.

`get_guide("tracker-conventions")` says the check *"fires only above a citation-volume
threshold to stay quiet on incidental prose"*. But common acronyms are the **highest**-volume
tokens in a technical corpus, so volume is anti-correlated with the property it was chosen
to select for. `link_scan`'s resolver already suppresses these correctly
(`src/librarian/tools/link_scan/resolve.rs`, `prefix_is_known`); `doctor`'s check is its
deliberate complement and inherits no such gate.

Steps: triage the 14 into real vs acronym; check whether `TC-N` (106 citations) is genuine;
then decide between reusing the resolver's suppression and adding a shape rule. Handle
`entry_cited_from_outside_but_undeclared` (12) separately — that one is a real worklist and
should not be swept up in a check fix.
## Phase descriptions

Phases encode **readiness, not importance.** A phase-3 item may matter far more than a phase-1 one;
it simply cannot be started by an agent alone.

### Phase 1 — Ready

The mechanism has been read at the bytes and the edit site is named. An agent can open the bug, go to
the cited line, and work. Eight rows.

Worth noting what makes these ready: each names a `path:line`. That is the difference between a bug
someone can pick up and a bug someone must first re-investigate — and it is why the bug template asks
for `path:line` on every root-cause claim.

### Phase 2 — Investigate first

The defect is real but the mechanism is **inferred** rather than measured, or the emission site has
not been located. Acting directly here means acting on an unverified premise, which this repo has
been bitten by: of five bugs worked on 2026-08-07, all five had a false premise or a wrong
prescription (W-13, `docs/trackers/release-promotion-session-log.md`).

BL-11 is the clearest case — its root cause is explicitly marked inferred, and its own Resume asks
for a worktree reproduction before any fix.

### Phase 3 — Blocked

Gated on something an agent should not decide alone:

- **BL-14, BL-15, BL-16** each present mutually-exclusive options. These are cheap to unblock — each
  needs one answer, not a discussion — and BL-15's answer may be `wontfix`, which is a legitimate
  outcome, not a failure.
- **BL-13** is gated on an external eval run (`../prompt-engineering/`), not a preference. Steps 1
  and 2 of that bug are already shipped and verified live; only the prompt wording awaits
  subtract-and-measure, which per `src/prompts/README.md` governs whether *any* prompt-surface change
  ships.

### Phase 4 — Ledger hygiene

BL-17: one bug sits at `docs/issues/archive/…` while its frontmatter still says `status: open`. It
was fixed (`43fac6c8`) and moved, but the status flip was missed — so it appears in every
"what's open?" query while being physically archived. Exactly the drift the archive-through-the-catalog
rule exists to prevent.

## Sequencing notes

Two clusters are worth taking together rather than one at a time:

- **The overflow/handle cluster** — BL-1, BL-2, BL-6, BL-8 all concern a result that was cut and
  whether the caller can tell. They share a root shape: *a truncated payload that reads as complete.*
  The `grep` byte-budget fix (archived 2026-08-16) is the first of this family and its
  `… [truncated: N of M bytes shown]` marker is the pattern the rest should match. Fixing them as a
  set gives one consistent signal rather than four dialects.
- **The worktree cluster** — BL-11, BL-12, BL-16. BL-16 needs a decision that likely constrains
  BL-12's design, so answer BL-16 first even though BL-12 is nominally less blocked.

BL-3 and BL-1 carry the strongest measured evidence: `missing_required_param` is the largest
non-routing error family (38 hits / 20 sessions) and `json_path_key_miss` is 27 hits / 17 sessions,
both from the 2026-08-15 tool-usage investigation. If picking by impact rather than readiness, start
there.

## History

### 2026-08-18 (later) — BL-40 and BL-41 shipped, step 4 taken from 10 open ledgers to 2

Worked the order the previous entry designated, and it was the right order for the reason it
gave: BL-41 changes what the dangling number means, so measuring the backfills first would
have scored them against a baseline about to move.

**BL-40 — `params_behind_body` (`87f3b936`).** The inverse of `snapshot_drift`: same two sets,
subtracted the other way. Two decisions each pinned by a test rather than a comment — not
gated on `body_keeps_snapshot` (that gate would silence a body id the catalog has never seen,
which is the entire finding), and the message names `append_entry` while explicitly forbidding
a body re-render, because inheriting `snapshot_drift`'s remedy here is data loss rather than
noise. Six tests, five watched fail on their assertions; the sixth passes on a stub **by
design**, since it asserts the check stays silent on a lagging body and only discriminates once
the code can subtract backwards. Refactored the 45-line preamble the three entry-drift scans
share into `params_backed_ledgers` — this would have been the third copy, and three checks that
must agree on what a ledger *is* drifting apart is the failure mode this whole family is about.

**It paid off on first contact with the real catalog**, which is the step distinct from the
gate: fired twice, neither vacuous. The WIN near-miss it was filed for, and
`mirela/…/solver-invariants.md` at 10 of 68 — a different repo, never reported by any surface,
with ten ids that were never allocated. Split to **BL-42**.

**BL-41 — declared prefixes widen the gate (`ff088630`).** The wiring differs from what the bug
file proposed, and deliberately: the declaration rides on `DocExtract.declared_prefixes`,
populated inside `extract()`, which already holds the whole file text. There is therefore **no
wire for a caller to forget**; threading it through `DefinitionIndex::build` would have made an
omission a silent no-op across 11 call sites. Its known cost is inherent — once a prefix is
known, prose that merely looks like a token is reported too, and there is a measured instance
(`bug-fix-session-log.md:467` says "a parallel session's T-13 commit", an old plan's task
numbering). Right trade: a false positive costs one glance, the false negative cost 129
silently-dead citations.

**Step 4 (`c7bdfd22`) — 49 entries made citable across four codescout-local ledgers**, each
shaped to what its ledger needed rather than one template. `SD` had no table and no sections,
so its whole substantive record lived only in the git-ignored catalog. `GF` is a dated snapshot
whose prompt forbids rewriting sections while telling readers to read a section that did not
exist — anchors only, pointing at where its evidence already sits. `FND` had nothing at all for
four of its eighteen claims. Plus mirela's `G`×6 and `OTK-35`, left **uncommitted** — that is
the user's own checkout, with 9 modified files from a concurrent session on
`feat/year-scoped-catalog`.

**Two beliefs from the previous close turned out wrong when checked.**

1. *"Backfilling `fable-tuning-tasks` would make its `T` tokens ambiguous."* False.
   `tool-usage-patterns` spells its first thirteen entries zero-padded (`T-001`..`T-013`) and
   its later ones `T-14`..`T-24`, and the resolver matches **token strings**, so the spaces are
   disjoint and `T-1`..`T-12` were always safe. The wrong version of this belief nearly blocked
   correct work; filed the real hazard as
   `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`
   (fixed and archived 2026-08-21),
   because the disjointness is a formatting accident that nothing records or enforces, and two
   ordinary edits would break it.
2. The closing report's `dangling` 547 and `ledger_defines_nothing` 8 both read one lower when
   re-measured. A peer session shares this machine-local catalog. Recorded as measured, not
   attributed — the point being that a catalog number is a fact about an instant.

**Next, in this order.**

1. **`/mcp`, then `librarian(action="link_scan")`.** BL-41's central claim is still unverified.
   Report the two effects separately: BL-41 pushes dangling **up**, `c7bdfd22`'s 49 headings
   push it **down**, so a flat total would be two real effects cancelling rather than a no-op.
   `edges_added` is the clean read on the backfill half. Do not archive BL-41 before this.
2. **BL-42**, `solver-invariants` first — id reissue is silent corruption, and it needs a
   confirm because that checkout has concurrent work.
3. **researcher `T`×2 and stefanini `CR`×8** — the last two `ledger_defines_nothing`. Both are
   **outside this session's working dirs**, so they need the user's go-ahead, not just a
   decision. Read the prefix-`T` bug file before touching researcher's: giving its `T-1`/`T-2`
   headings is precisely what makes `prompt-hamsa-audit-log`'s bare citations ambiguous, and
   the fix then is to qualify them as `fable-tuning-tasks:T-N` in the same commit.
4. `provenance-subsystem`'s 42 uncited `PV` stay undefined on purpose — only 22 of 64 entries
   are cited, and the ledger's own convention is narrative only where a row is insufficient.
#### Addendum — measured on the wire, and BL-41's prediction was wrong

`cargo rb` + `/mcp`, then verified the **server process** was running the new code rather than
trusting that a build existed: `librarian(action="doctor")` reported `params_behind_body`, which
is BL-40's check and shipped in the same binary. Its counts matched the CLI exactly.

| metric | pre-fix | post-fix | Δ |
|---|---|---|---|
| `dangling` | 548 | **471** | **−77** |
| `ambiguous` | 410 | 411 | +1 |
| `edges_desired` | 860 | 895 | +35 |
| `edges_added` (write=true) | — | **44** | — |
| `citations` | 3,649 | 3,718 | +69 |

The 44 edges are materialized; the graph now holds **895** `cites` edges, which is a measured
absolute rather than a running sum across sessions.

**BL-41's bug file predicted the dangling total would RISE, and called that "the fix working".
It fell by 77, and BL-41's contribution to the number is ZERO.** Not because the fix is broken
— because it keys on the *declaration*, and no prefix in this corpus is any longer declared and
wholly undefined. Verified by inventory, not inferred: nine declared prefixes, every one with
≥1 defining heading. The four ledgers that WERE wholly undefined received their declaration and
their headings in the same commit, so the widened gate never had a case to fire on. The −77 and
the 44 edges belong entirely to the backfill.

Recording the mismatch rather than reframing the prediction to fit the result. Two things follow.
First, `edges_added` really is the clean read on backfill progress — as the file argued, and
**`edges_missing` is its dry-run twin** (`write=false` leaves `edges_added` at 0, which is easy to
misread as "nothing happened"). Second, BL-41's value is prospective: the next ledger created with
`entry_prefix` and row-only entries dangles loudly instead of silently. Its retrospective coverage
is incomplete, concretely so — filed as **BL-43**.

Both bug files are left `fixed` and **NOT archived**, deliberately. BL-43 and BL-39's step-4
remainder target the same two out-of-scope ledgers; archiving now would bury BL-43's rationale in
a file nothing re-reads and force the 11-citation sweep (3 of them in Rust source) twice instead
of once.
#### Scope decision — 2026-08-18: cross-repo trackers leave this queue

Explicit call: **other repos' trackers and issues are solved in those repos, properly.** This
queue is codescout's. What that changed, and what it deliberately did not:

| item | disposition |
|---|---|
| BL-39 step 4 | **complete for codescout** — no codescout ledger reports `ledger_defines_nothing`; researcher `T`×2 + stefanini `CR`×8 handed off |
| BL-42 | **split** — the 6 WIN rows are ours; SI-59..SI-68 handed off |
| BL-43 | **dropped, handed off** — both targets are elsewhere; codescout's own declarations already done |
| mirela G×6 + OTK-35 | left uncommitted in that checkout for its owner; pure additions, 41 + 8 lines |

Two things this does NOT mean. The **findings** are not withdrawn — each is recorded above with
its measurement, because a scope change should not be able to erase evidence. And the
**codescout-owned halves of cross-repo findings stay ours**: the `frontmatter_id_mismatch`
defect lives in `doctor.rs` and `mv.rs` even though every affected file is in another repo, so
it is fixed here against synthetic fixtures.

This also unblocks the archive sweep that was deferred. BL-39, BL-40 and BL-41 were all held
`fixed`-but-unarchived for one reason — BL-43 and step 4's remainder shared their two out-of-scope
files, and archiving early would have buried the rationale and forced the 11-citation sweep twice.
With the remainder handed off, all three can archive in one pass with one sweep.
### 2026-08-18 (session close) — the citability chain, and what a later session should pick up

One long session. Written here rather than in any `next` field because **`next` lives only in the
catalog, which is machine-local and git-ignored** — the defect BL-29 names, and the reason a
session's own record has to be committed prose.

**Closed.** BL-13 (IL1 wording measured as hamsa A-25 and REFUTED — reverted at `32b34efa` behind an
inverted guard test, bug archived). U-44 (companion IL3 warn-hook — fix was already shipped in
1.16.9; verified across all three profiles and on the wire, and the closure found `docs/architecture/companion-plugin.md`
wrong in **every** row, since the 1.14.0 `.sh`→`.mjs` port was never swept there). The
`tracker-conventions` id-stamping bug (`450df27edcfe9c08`), whose real origin turned out to be
`tracker_design`'s **archetype defaults** rather than the guide.

**BL-39, steps 0-3 and 5 shipped.** `de4df2cd` `body_defined_indices` → `f19d5296`
`undefined_in_body` on both entry-writing tools → `758b37dc` the two `doctor` checks → `d3c1e6ed`
guide + `librarian.md` + the archetype defaults. Step 4 (backfill) is **4 of 13 ledgers**: WIN
(`f04e4c17`), BL (`0d101eb8`), PV (`f5f602e6`), A (`9703102c`). Project dangling **621 → 547** and
**67 edges** materialised.

**Filed this session and NOT yet worked: BL-40 and BL-41.** Both were found by doing step 4 rather
than by looking for them, and both are cheap:

- **BL-40** — no check sees params falling behind a body that ran ahead. Nearly published two wrong
  statuses and dropped six entries from `windows-platform-support.md`.
- **BL-41** — `link_scan`'s dangling count is prefix-gated, so a wholly-undefined namespace reads as
  healthy. 129 dead `WIN-N` citations moved the total by zero.

**Read BL-41 before trusting any dangling number**, including the 621 → 547 above. Which metric
moves depends on whether the prefix already had one definition: `edges_added` measures
`ledger_defines_nothing` progress, `dangling` measures `entry_without_definition` progress, and
`doctor` measures both. That was established by prediction across four backfills, not in hindsight.

**Next, in this order.** BL-40 and BL-41 first — both are small, and BL-41 changes what the numbers
mean for the rest of step 4. Then the remaining codescout-local ledgers by external citation count:
SD + GF (~36), `fable-tuning-findings.md` (FND, 19). **`fable-tuning-tasks.md` is a prefix decision,
not a backfill** — its `T` prefix is already defined by `tool-usage-patterns.md` (T-001…T-24), so
adding `## T-N` headings there makes those tokens *ambiguous* rather than resolved. Four ledgers in
`researcher` / `mirela` / `stefanini` need their own workspaces activated and are a separate call.

**Two standing constraints for whoever continues.** `tracker_design`'s SYSTEM_PROMPT has **~102
bytes** of inline headroom, down from ~640 — treat it as zero, and `default_response_fits_inline` is
what will say so. And `body_claimed_indices` also counts a heading inside a fenced block and a
code-first `` `A-3` `` heading, so the *claimed* set is not merely "defined plus rows".

### 2026-08-18 — the shell-gate cluster closed, and the snapshot had drifted on six rows

**Five bugs closed across two sessions**, none of which had a BL row — they were filed
and fixed inside a day, faster than this queue turns over. `find`'s silent `count: 0`
(`9d386566`), `chunk_id` collapsing duplicate chunks (`933af744`), the IL3 heredoc gap
plus `find` dropping `rel_path` (`4fad1aa4`), the newline separator in both gates
(`308014b5`), `artifact(get)` returning bare `null` (`9a71357e`), the source gate
counting relative tokens as in-project (`be2d7781`), and worktree reads answering about
the old checkout (`dd788ce1`). The IL3 advisory hook was deleted outright in
`codescout-companion` 1.16.9 rather than corrected. All recorded in `CHANGELOG.md`
(`fa107a6a`) — which had itself gone 27 commits without an update.

**The snapshot was wrong on six rows.** BL-6, BL-7, BL-8, BL-10 and BL-25 read `open`
here while params said `done`; BL-16 read `blocked` while params said `open`. So the
committed file — the only copy that exists in git — advertised **eight** open items when
four were live. That is BL-29 again, and worth stating plainly: every fix in this queue's
history that flipped a row with `update_entry` aged this table by one row, and the drift
is invisible until someone diffs params against the body. The check is
`artifact(get, id="9a892c2a5976e296", entry_filter={"status":{"ne":"done"}})` — run it
before trusting any "what's open" read of this section.

**BL-38 added** for the ledger-aware librarian guard, which had a 770-line plan
(`8a793791`, at `docs/superpowers/plans/`, not `docs/plans/`) and no row. It is the only remaining open bug whose defect is still actively
producing damage rather than waiting on a decision.

**BL-13 is no longer merely blocked.** The A-25 pre-registration landed (`e2fbefe2`) with
a numeric ship/no-ship rule fixed *before* either arm runs — ship the clause only if arm A
base is >= 3/10 and arm B clause is <= 1/10; arm A <= 1/10 is a ceiling and the 57
characters get reverted. What remains is running the arms in `prompt-engineering`, not
deciding anything here.

**Verify-open pass run on the two rows whose bugs were absent from the open-bug query.**
BL-16 (`403e3fad0356f171`) and BL-29 (`879b1b18243f20b2`) are both genuinely `mitigated`,
not zombie-open — clean, against this project's measured 75% zombie rate.

### 2026-08-16 — opened

Snapshotted 17 open bugs into BL-1..BL-17 with per-row next actions taken from each bug's `## Resume`
rather than invented. Phase assignment reflects readiness as of this date.

Context: this queue was created at the end of a session that fixed three bugs
(`grep` byte budget — archived; IL1 steps 1-2 — verified live; plus the IL1 prompt wording) and filed
three new ones. The remaining 17 are what was left standing.

### 2026-08-16 — BL-18 added, found by building this file

Creating this tracker surfaced its own bug. `artifact(create, augment={…})` accepts only `prompt`
and `params`; the `render_template`, `params_schema` and `entry_collection` passed alongside them
were silently discarded, and the call still returned success. Both had to be re-applied with a
follow-up `artifact_augment(merge=true)`.

Filed as `29f1ddf259562b7f` and queued as BL-18. It is a recurrence of a class already fixed once in
the same file (`artifact(create)` dropping `topic`, archived 2026-07-13), and it is compounded by
`tracker_design`'s own Final step listing `params_schema` and `render_template` among the fields to
pass to `create` — guidance followed exactly here, with both fields lost.

Worth noting for whoever works the queue: **BL-18 was found by using the tooling, not by reading
it.** Three of this session's bugs came the same way. A queue built by hand is also a probe.

### 2026-08-16 — BL-5 and BL-18 fixed together

Taken as a pair because both edit `tracker_design`'s `SYSTEM_PROMPT`: BL-5 had to shrink it, BL-18
had to correct its Final step. Doing them in one pass avoided touching the same 100-line constant
twice.

**BL-5** — `tracker_design` went from **~41,000 to 9,358 bytes**, from overflowing on 6 of 6 calls to
arriving inline. The split (menu inline, one archetype per named fetch) was the planned half; the
unplanned half was `existing_trackers`, which at a cap of 30 with six fields per row was ~7 KB —
larger than the entire archetype menu. Capped at 5 rows of `{id, title, kind}`, with Step 7 rewritten
to send the caller to a semantic `artifact(find)` for the collision check a title scan cannot do.

**BL-18** — `AugmentSpec` widened from 2 fields to all 7 and gained `deny_unknown_fields`, so
`create` both accepts the full augmentation shape and rejects typos instead of discarding them. The
advertised schema and `tracker_design`'s Final step now say the same thing the code does.

One lesson worth carrying: **BL-5's first regression test was wrong in a way that would have shipped
the bug.** Written against an empty catalog it read 10,396 bytes; the same code against a full
catalog read 17,456. `existing_trackers` is empty in a bare fixture and populated in production, so
the test would have gone green while every real call still overflowed. A size assertion has to be
made against the shape that ships — the same *wrong population* error TU-5 was corrected for.

### 2026-08-16 — BL-1 fixed; BL-19 filed; the queue was not actually in git

**BL-1** — `[*]` now parses and projects, the recovery hint is derived from the payload's shape
instead of the constant `$.field`, and both rejection hints plus
`get_guide("progressive-disclosure")` advertise the grammar. Verified live: the exact call that used
to be rejected, `read_file("@tool_…", json_path="$.augmentation.params.tasks[*].id")`, returned all
18 BL ids from a buffered handle.

**BL-19** — filed from a complaint about the fix's own output. The hint is now correct, but the
*envelope* still costs a whole call to return nothing: `artifact(get)` answers with `output_id`, a
byte count and a hint, and nothing about the artifact. The librarian adapter's `format_compact` has
exactly one case — a body-truncation warning — so every other response falls through to the generic
"Result stored in …". Fixing the hint makes the second call land; it does not make it unnecessary.

**And this file was not what it appeared to be.** The BL rows are `params`, and params live in the
librarian catalog under `~/.local/share/`, **not in the repo**. The markdown carried frontmatter and
prose only — so the queue existed on this machine and nowhere else, which is the opposite of why a
tracker was chosen over Claude Code's per-profile memory in the first place. The rendered snapshot
above fixes that. Worth knowing when creating any augmented tracker: writing a good body does not
make its live state durable, and the file will not look wrong.

**BL-20** — filed from an own goal committed while writing the line above. Flipping BL-1 to `done`
via `patch={params:{tasks:[one row]}}` deleted BL-2..BL-19: merge-patch replaces arrays wholesale,
and the call answered `updated: true`. The rows survived only because the snapshot had been written
minutes earlier, for an unrelated reason.

That near-miss is the argument for the snapshot, independent of git: **params have no shrink guard,
no `force` gate, no report of what a write destroyed, and no version control** — all four of which
the body surface has. The less-protected surface is the one with no backup. Keep a rendered table in
the body of every entry-bearing tracker.


### 2026-08-16 — six fixed and archived; the queue's own tooling was most of the work

**Shipped, all on `experiments`, all fast-forward (so each SHA *is* the master SHA):**

| BL | what | SHA |
|---|---|---|
| BL-1 | `[*]` projection + payload-derived hint (+ depth walk) | `7c91cdf7`, `336d3b04` |
| BL-20 | `update_entry` — entry-grain patching; always-on entry counts | `02a87a83` |
| BL-21 | librarian guard hoisted into `read_edit_target` (all 3 write paths) | `47abcb6d` |
| BL-22 | `move` grafts history onto the new id instead of stranding it | `2d8c7f39` |
| BL-26 | `librarian-runtime` guide corrected + guard test over every guide | `6018b7ad` |
| BL-27 | `entry`-param guard fires whenever `entry` is present | `6018b7ad` |

Plus `61ab520a`: **14 `fixed`-but-unarchived bug files archived**, and three deliberately
left open because their Resumes describe real undone work —
`audit-doc-refs-gate-hides-its-own-cause`, `edit-code-remove-ast-repair-over-deletes`,
`workspace-toml-mis-rooted`. Those three were indistinguishable from the other fourteen by
status, path or age; only reading the Resumes separated them.

**BL-22 is the one to understand before touching the catalog.** `move` used to preserve an
artifact's id while changing its path, which breaks the invariant `doctor.rs` states twice
(`id == artifact_id_from_abs(abs_path)`). The next `reindex` then re-keyed the row and
`upsert`'s abs_path pre-clean cascade-deleted its events. Measured: one reindex took the
catalog from **1845 to 1834 events while reporting `removed: 0`**, and archived bug files
carried 0.02 events/row against 0.65 for live ones. Fixing it is what made the 14-file
sweep above safe to run at all.

**Two guards were written to the reproduction rather than the condition**, hours apart:
`edit_file`'s covered 1 write path of 3, and `update_entry`'s read
`entry.is_some() && fields.is_none()` so sending both dropped `entry` again. Both were
caught by the other session. The test shape that catches this class is a table containing a
row that was **green before the fix** — it proves the table discriminates rather than
refusing everything.

### 2026-08-16 — BL-2 fixed; the filed root cause was understating it

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-2 | grep stops printing a denominator the walk never counted | `4b77dff5` + `358f1ced` |
| BL-31 | filed — found while fixing BL-2 | (open) |

**The filing was wrong about scope, and the reconnaissance is what caught it.** BL-2 was
filed as a corner case: "N of N" appears *when* `budget >= total`. Tracing `max` from its
single binding showed it serves as **both** the collection break threshold and
`cap_grouped`'s display budget, so `visible.len() == total` unconditionally and the second
disjunct of `truncated = hit_cap || total > visible.len()` is **dead code**. The hint had
never printed anything else. Had the fix been written to the filing, it would have
branched on a condition that cannot occur.

**Three surfaces, not one.** `format_overflow` already rendered the honest `… showing
first N` when `shown == total`; grep's own hint then re-asserted `Showing N of N` right
after it, under a header stating the count as fact. The one true clause was the shortest
and sat between the two that overrode it. Fixing only the hint would have left the header
— the line a reader anchors on — still wrong.

**The test shape, third instance this session.** Two rows over one corpus, capped and
complete; the complete row was **green before the fix**. That is the only shape that can
catch a defect whose symptom is that two renderings are byte-identical — asserting on the
capped string alone passes against the bug. Same shape as the `edit_file` guard (1 write
path of 3) and the `update_entry` guard (1 input shape of 2).

**Verified by invoking, not by inspecting** (T-20). `cargo test` proved `Grep::call` →
`format_compact`; only the live MCP call proved the running server serves it:

```
grep(pattern="^use serde_json::json;", glob="src/**/*.rs", limit=5)
5 matches (capped) in 5 files
  … showing first 5 — Collection stopped at limit=5, so the true total is unknown …
```

**BL-31 fell out of the same reading.** Because collection stops at `max` in walk order,
`cap_grouped`'s round-robin never runs from grep. Live proof, same session: a capped
search offered three files holding **one match each** as ways to narrow a result already
capped at five — advice that is not merely unranked but inert. Two capped searches before
it returned `across 1 files`, so the common capped result is one file's worth, which is
also why BL-2's new `(capped)` header marker is invisible in the common case. The two
defects interact; fixing BL-31 restores the marker.

**Shared-tree hazard #2.** `cargo fmt` formats the whole crate, and it was run while the
concurrent session had uncommitted edits under `src/librarian/tools/audit_doc_refs/`. Any
reformatting of their in-progress files landed in *their* diff. Pathspec-scoped commits
protect the index; they do not protect against a whole-crate formatter. Prefer
`cargo fmt -- <file>` on a shared tree.

### 2026-08-16 — BL-33: the convention was hiding the answer

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-33 | guard keys on what is actually managed, not on YAML quoting | `29f0c015` |

**Filed as a quoting bug; fixed as a predicate bug.** `is_librarian_artifact` tested the
frontmatter text for a 16-lowercase-hex `id:`, and a quoted `'…'` is 18 characters, so
protection depended on which serialiser last wrote the file — 12 guarded trackers, 15 not,
the queue among the unguarded. The quoting fix alone closes 86 files repo-wide.

**The user's course-correction is the entry worth keeping.** My first pass concluded the
id-less trackers were *not* a defect: the stamped id looked like an intentional opt-in
marker, and a catalog-backed guard would refuse `edit_markdown` on
`docs/trackers/skill-frictions.md`, which CLAUDE.md documents. That reasoning deferred to a
convention instead of testing it — *"lets correct it properly not follow a rule written 10
years ago"*. Two queries then settled the design:

```
docs/RELEASE.md, CONTRIBUTING.md, PROGRESSIVE_DISCOVERABILITY.md, TAXONOMY.md, ROADMAP.md
  -> ALL catalog rows                       (so membership is the WRONG predicate)
artifact(find, augmented=true, scope="repo")
  -> count: 16, all trackers                (so augmentation is the RIGHT one)
```

Augmentation is exactly the set where state lives *outside* the file. It keeps
`skill-frictions.md` editable — but now because that is correct, not because it is written
down — and it catches `artifact-augmentation-followups.md`, augmented with **no** frontmatter
id, which no amount of string-parsing could ever reach.

**Cost of the naive route, measured before rejecting it:** the core `ToolContext`
(`src/tools/core/types.rs`) has no catalog handle, and adding a field means editing **124
construction sites across 21 files**. A trait object installed once at server construction
costs one line in `server.rs`. `guard_with_oracle` takes the oracle explicitly so no test
installs into the `OnceLock` and none can poison another in the same binary.

**Verified live on four cases**, the two negatives mattering as much as the positives:
augmented-with-no-id refused, augmented-with-quoted-id refused, prose tracker readable,
`docs/RELEASE.md` readable.

**A closing loop.** This queue is now guarded against the direct `read_markdown` that
T-22 caught me making — the observation and the fix landed in the same session, and
`artifact(get, entry_filter=…)` is now the only way in.

### 2026-08-16 — BL-34 fixed, and the corpus proved something the unit tests could not

`858f22ec` — `frontmatter::replace_id_line` splices the `^id:` line instead of round-tripping
the block through `parse` → `write`. `mv::repair_frontmatter_id` now reads through the parser
(authoritative about *whether* a repair is needed) and writes through the splice (authoritative
about *which bytes move*). When they disagree — a folded or flow-mapped `id:` the line scan
cannot see — it warns and leaves the file alone. Declining beats reformatting: a stale id is a
broken citation, a reformat is data loss.

**Verified live against the named regression corpus**, `/home/marius/work/mirela/eduplanner-ui`
(reverted on purpose 2026-08-16 precisely so it could serve as one):

| | before | after |
|---|---|---|
| files | 26 | 26 |
| diff | 402+ / 370− | **26+ / 26−** |
| lines/file | 30 | **1** |

Every hunk in `git diff -U0` is `@@ -2 +2 @@`, and every `−`/`+` line begins with `id:`. Both
ADR/FDR templates are byte-identical outside that line — `created: {YYYY-MM-DD}` intact.

**Three things worth carrying forward.**

1. **The corpus caught a class the unit tests structurally could not.** `title: "{Title}"` and
   `category: a | b | c` are shapes a *round-trip* mangles and a *splice* never sees. The fix
   is not "handle more YAML shapes correctly" — it is "stop reading shapes you have no business
   rewriting." A narrower contract needs fewer cases.

2. **Fourth instance this session of a test that exercises the mechanism but not the call
   site.** `move_rewrites_the_frontmatter_id_it_just_invalidated` was green through this entire
   defect, because it only ever asserted *that the id changed*. When a fix lands in a shared
   helper, drive at least one real call site — and mutation-verify that one, since it is the
   only test that can fail for the right reason.

3. **A read-only precondition check is cheap and buys the apply.** Before writing into a
   foreign repo, `grep -c '^id:'` over the 26 flagged files confirmed every one would take the
   splice path rather than the decline path. The four files the doctor did *not* flag
   (ADR-0023, ADR-0024, two READMEs) have no `id:` at all — the abstention branch, visible for
   free in the same result.

The 26-line repair is left **uncommitted** in `eduplanner-ui`'s working tree. It is the correct
repair now, but committing into another repo is not this session's call.
### 2026-08-17 — BL-35 fixed

Swapped `guard_worktree_write`'s gate from `is_project_explicitly_activated` to
`is_project_chosen_this_session` (option 1 from the bug's own three) — the
maintainer's call, made directly, accepting that this repo (two live
worktrees at the time) starts refusing un-activated writes immediately.
Three regression tests added (`src/tools/core/tests.rs`), reusing the
`seed_linked_worktree` / `rooted_ctx` fixtures the read-side notice fix had
already built — including one pinning the case that matters most for a
re-armed refusal: no worktrees, never activated, still allowed. Bug archived
(`1523556488a95de2` → `a742a50ea6723daf`); citations of the old path in
`src/tools/core/types.rs`, `guards.rs`, `tests.rs` re-pointed in the same pass.
### 2026-08-17 — BL-14, BL-15, BL-37(partial); and `experiments` was red at a clean tree

Compaction handoff for the BL-N run. Everything below is on `experiments`; gate green at
`9ad5818d` — `cargo fmt --check` clean, `clippy --all-targets -D warnings` clean,
`cargo test` **4055 passed / 0 failed / 45 ignored**.

**Closed**

| Item | Fix | Archive |
|---|---|---|
| BL-15 / B-6 — read-only metadata blocked on source paths | `90c5aea1` | `c4211712` |
| BL-14 / B-1 — `force=true` discarded in silence on whole-file reads | `2703410e` | `201628f9` |
| `frontmatter_max` dropped at the MCP boundary | `4cdafd9a` | `8a91e950` |

**BL-37 — partial.** Interim priority-ordered trim shipped (`30f3df81`; note shortened
`9f4807ef`). Open on the **carrier decision alone**, which is the maintainer's.

**Fixed outside the queue**

- `bf485a00` — `append_entry`'s hint asserted `##` for every ledger; now derived from the
  body (mode of the existing entry headings). `docs/PROGRESSIVE_DISCOVERABILITY.md` gained
  **Anti-Pattern 5 — Asserting a Convention the Tool Never Read** as the standing rule.
- `fd9e63d0` — Iron Law 3 describes two gates three paragraphs apart that share vocabulary
  (`wc` appears in three roles on one page); signposted.
- `9ad5818d` — **`experiments` was red at a clean tree.** `90d76d8a` re-armed
  `guard_worktree_write` deliberately and left `tests/integration.rs` asserting the removed
  behaviour. Inverted rather than deleted; the odd-looking
  `assert!(is_project_explicitly_activated)` is now the point, not the premise.

**Filed** — `audit_doc_refs` `include_str!` false positive (fixed by a peer, `da55100a`);
`/mcp` refreshes schemas but not `server_instructions`; U-40 (`old_string` "not found" cannot
distinguish a bad needle from a bad haystack); U-41 (`snapshot_stale` overclaims); R-101, R-102.

#### Standing hazards for the next session — read before writing anything

1. **Writes now require an explicit activate in this repo.** `90d76d8a` re-armed the
   worktree guard on `is_project_chosen_this_session`, and this checkout has two linked
   worktrees, so the first `edit_code` / `edit_file` of a session is **refused** until
   `workspace(action="activate", path="/home/marius/work/claude/codescout")`. Hit live on
   2026-08-17. Working as designed; not a bug.
2. **A peer session shares this tree.** It broke compilation three times mid-turn
   (`audit_doc_refs`, `link_scan`), so `cargo test` is not always available. Commit by
   **pathspec**, never `git add -A`; use `cargo fmt -- <file>` rather than whole-tree fmt.
   A full-suite red is worth attributing before assuming it is yours.
3. **`cargo test --lib` does not build `tests/integration.rs`** — memory
   `cargo-test-lib-skips-integration`. That is exactly how a deliberate behaviour change
   left HEAD red for ~40 minutes.
4. **The instructions surface cannot be verified from the session that renders it.** The
   workaround, and it works: speak MCP to the release binary directly —
   `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' | ./target/release/codescout start --project .`
   That is how `30f3df81` was verified on the wire.
5. **Reindex before any `artifact(find)` you will act on.** Measured twice today: the
   catalog lagged disk by one file, and a lagging catalog and an empty filesystem return
   byte-identical answers.

#### Resume

Nine bugs open. Next by evidence quality: **`artifact(find)` is silent about unindexed
files** — fix shape settled by precedent (make the zero self-describing, as `grep`'s
hidden-path skip was), needs care because `find` is on the hot path. Then the **BL-37
carrier decision**, which is a design call rather than an implementation.

Promotion is **not** blocked by any of the above, but is not gated either: `0 941`
fast-forward available, code gate green, and `docs/RELEASE.md`'s step-4 documentation gate
is unrun — the CHANGELOG / mdBook / README checks, plus the audit re-run against a **fresh
clone**, which the file warns is the one green not to believe from a working tree.
### Resume — state at compaction, 2026-08-16

**15 of 35 rows open.** Phase 1 remaining: **BL-4, BL-19** (BL-29 is phase 1 but the other
session's). Phase 2: BL-9, BL-24, BL-30, BL-31, BL-35. Phase 3: BL-13, BL-14, BL-15, BL-16,
BL-28, BL-32. Phase 4: BL-17.

BL-29/BL-30/BL-35 belong to the concurrent session. **BL-4 and BL-19 are the natural next** —
BL-19 is a progressive-disclosure sibling of the BL-2 fix (incompleteness signals on buffered
output).

**Working practices, carried forward:**

- **Read the filing as a hypothesis, not a spec.** BL-2's filed root cause was narrower
  than the defect; the fix would have been written to a branch that cannot execute.
  Trace the variable from its binding before implementing the fix a bug file prescribes.
- **The catching test contains a row that was green before the fix.** Four for four
  this session on guard/message defects — BL-34 made it a helper-vs-call-site variant.
- **When the fix lands in a shared helper, drive at least one real call site**, and
  mutation-verify that test specifically. The helper's own tests cannot prove the caller
  reaches for it.
- **Verify a fix is live by invoking it**, never by inspecting the binary or
  `codescout_sha` (T-20).
- **Archive with `artifact(action="move")` and re-point ids in the same commit** — read
  `id_changed`. An id that returns `count: 0` from `find` is archived, not deleted.
- **Check the snapshot against live params before trusting a row** (BL-29). Three rows
  had drifted here, all from catalog-only writes.
- **A concurrent session shares this working tree, index, and formatter.** Commit by
  pathspec; prefer `cargo fmt -- <file>`. It also shares the **git index**: a peer's
  `git commit` swept a file this session had `git add`-ed into their commit (observed
  2026-08-16, `2f94ce40`). Stage immediately before committing, not early.
- **Bug-file location vs status — the standing check** (BL-17). The invariant is that no
  *archived* bug carries a non-terminal status:

  ```
  artifact(action="find", kind="bug", filter={"and": [
    {"abs_path": {"contains": "issues/archive/"}},
    {"status":   {"in": ["open", "investigating"]}}]})
  ```

  It must return **0** — measured 0 on 2026-08-16. The *inverse* (terminal status, still
  in `docs/issues/`) is **not** a defect and should not be swept: every such file states
  its residual work in `## Resume` — a fixed *damage* with an unfixed *trigger*, a
  mitigation with a root cause still open, a deliberate `open` on an unreproduced
  flake. Archiving those would hide real work to satisfy a tidiness rule. Check the
  Resume before moving anything.
- **Before writing into a foreign repo, run the read-only precondition check first.** The
  dry-run says *which* files; a `grep` on the shape says *which code path* they will take.
  Both are free, and together they make the apply reviewable in advance (BL-34).

### 2026-08-18 (later) — BL-38 fixed, and two of its three pieces were already shipped

`f4db4e9c` and `9ac00440` (**experiments**) closed BL-38. The interesting part is what
reconnaissance found before any code was written: the plan's three-piece design stood in
three different states.

Pieces 1 and 2 — declare `entry_prefix` in frontmatter, wire `allocate_entry_id` to the
tool surface — were **already shipped**, the latter at
`src/librarian/tools/append_entry.rs:91` and hardened since by three follow-up fixes. The
bug file said "NOT yet called from any MCP tool"; true at `540c29c3`, stale by the time it
was read.

Piece 3's goal shipped: `declared_entry_prefixes` plus a third arm in the guard's union, so
a ledger is guarded by its declaration rather than incidentally. All five ledgers that
exist today were *already* guarded — two by a stamped `id:`, three by augmentation — so the
protection was accidental, not principled. The reachable hole was proved end-to-end rather
than argued: a scratch ledger with `entry_prefix: ZZ` / `entry_high_water_ZZ: 3` / no `id:`
/ not augmented accepted a hand-written `## ZZ-4` heading via `edit_markdown` and left the
mark at 3, which is the input compaction later reads back to reissue `ZZ-4`.

Piece 3's *mechanism* was **cut**. The heading-scoped guard existed to answer the objection
that a whole-file guard makes a typo fix in a 2,800-line tracker a ceremony — and that
premise is false. `artifact(update, patch={body_edits: [{heading, action: "edit",
old_string, new_string}]})` is already a section-scoped swap and works on any catalog row,
verified on `skill-frictions.md`, which has neither `id:` nor augmentation. Building it
would have cost a fourth parameter on a public function threaded through three call sites,
for a class of file with zero current members. What survives is hint text routing both
intents.

One defect the plan itself introduced, caught in review: it held the guard's hand-rolled
`entry_prefix` reader and the librarian's `serde_yml` one in agreement with a doc comment.
That is a co-change contract enforced by prose — the shape that cost this project 48
needlessly-compiled crates (`docs/adrs/2026-07-25-embedding-transport-boundary.md`). Now a
parity test over 11 YAML forms.

Follow-up filed rather than folded in — and **fixed later the same day at `d3c1e6ed`**, archived to
`docs/issues/archive/2026-08-18-tracker-conventions-guide-recommends-reverted-id-stamping.md`.
`get_guide("tracker-conventions")` had gone on prescribing stamping `id:` into frontmatter — the
remedy BL-38's own file retracted and `bb9a94d7` reverted — on a surface that auto-injects at the
first `artifact` call of every session, so the disproved advice sat somewhere louder than its own
retraction. It now says *declare `entry_prefix`* instead, which BL-38's fix is what made true.

Chasing it one layer down found the origin: `librarian(tracker_design)`, the surface you are told
to call *before* creating a tracker, shipped the losing shape as an archetype **default** —
`task_list` with no per-entry section at all, `failure_table` calling entry headings "optional",
`constitution` prescribing `## C-N` without the dash-and-title that makes a heading define
anything. The guide was the second-loudest place the shape was taught, not the cause.
