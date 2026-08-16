---
kind: tracker
status: active
title: Reconnaissance patterns
owners: []
tags:
  - reconnaissance
  - skill-meta
  - scout
---

# Reconnaissance patterns

Per-project R-N ledger for the `codescout-companion:reconnaissance`
skill in this project. See the canonical bootstrap, append rules,
sync flow, and R-N entry template in the skill's
`SKILL.md` and `references/reconnaissance-patterns-template.md`.

Three buckets: **hits** (scout caught drift), **misses** (scout missed,
downstream gate caught), **proposals** (vocabulary expansions for the
skill).

## The seven laws (distilled 2026-08-16)

Ninety-one entries written over three months state **seven laws**. Read these
first; the entries below are the evidence for how each was earned, not seven
different things to remember.

The distillation was produced by classifying every entry's full text (three
parallel passes, 99 graded instances — more than 91 because nine ids carried two
lessons each). Counts are of instances, not ids.

---

### A — Ground truth is the artifact. Everything else is a claim about it. (35 · 35%)

**Law.** A doc, a bug file, a plan, an error message, a memory, a subagent's
report, a commit message, and your own prior belief are all *claims*. Open the
thing they describe before acting on them.

**Sub-shapes, each its own recurrence chain.**

- *The bug file you are implementing from* — R-49 → R-62 → R-78 → R-80 → R-84.
  Its `## Symptom` was observed; its `## Root cause` is usually someone's
  unverified reading, and its `## Fix` is a plan. Run the one command that
  falsifies the root cause before writing code.
- *The error message* — R-35 → R-71 → R-82. A tool's own diagnostic is a
  hypothesis about itself, and its remediation hint is a claim about an API.
- *The document, memory, or plan naming code* — R-13, R-14, R-64, R-69, R-74.

**Do.** Name the artifact that would settle it, and open it. If you cannot name
one, you are not verifying — you are re-reading your own belief.

---

### B — The instrument decides the answer. (18 · 18%)

**Law.** A result is evidence about the *configuration that produced it* before
it is evidence about the code. Build features, feature flags, which binary, which
tree, which platform, which transport, which process — each silently changes what
a command can possibly report.

**Chain.** R-81 → R-86 → R-89 → R-91, with R-91 the general case: *state what a
measurement cannot see before attaching a conclusion to it.*

**Do.** Before citing any output as evidence about code you have edited this
session, run a probe that can only succeed on the build you think you are
running — and confirm the serving *process* postdates that build. Those are two
facts, and mtime answers neither (R-89).

---

### E — The blast radius is wider than the thing you edited. (17 · 17%)

**Law.** Changing a symbol obliges enumerating what consumes it — callers, trait
implementors, data fixtures, generated surfaces, serde shapes, other copies of
the same heuristic. `references()` sees calls; it does not see closures,
fixtures, or a second hand-written copy.

**Do.** Enumerate consumers before the edit, not after the gate reddens.

---

### C — A search that finds nothing is evidence about the search. (16 · 16%)

**Law.** Absence is a claim about coverage. A file-scoped grep cannot prove
"never", a bounded search cannot authorise a deletion, and a view is not the set.

**Chain.** R-3 → R-73b → R-77 → R-79 → R-87 — the entries themselves label these
"third", "fourth", "fifth" recurrence. This is the law the ledger has failed
hardest to internalise.

**Do.** Phrase the query as the question you actually have, scope it to the tree
rather than the file, and require two independent positive signals before letting
a negative result authorise removal.

---

### D — A test that cannot fail is not coverage. (7 · 7%)

**Law.** A test proves something only if it runs and only about what it
exercises. Three ways it silently doesn't: never compiled (feature-gated into a
lane nothing enables), filtered out, or testing the callee while nothing tests
the dispatch that selects it.

**Chain.** R-70 → R-73 → R-76.

**Do.** Confirm the test *runs* before citing it. After any extraction, ask what
now chooses this, and mutate that choice — deleting the dispatch, not breaking
the function.

---

### F — A subagent knows only what the brief told it. (3 · 3%)

**Law.** Dispatch is a seam. Session state, pinned refs, and what the controller
already discovered do not travel unless written down; a SHA in a brief is a claim
with an expiry date.

**Exemplars.** R-9, R-68, R-72b.

---

### G — The answer may already be on record. (3 · 3%)

**Law.** Not a falsified claim — a lookup never performed. The bug ledger, an
existing comment, a concurrent session's measurement, or an in-repo implementation
of the convention you are about to hand-roll.

**Exemplars.** R-55, R-60.

---

### What this pass found about the ledger itself

- **Graph clustering does not work here**, tried twice: all `R-N` mentions as
  edges put 80 of 91 in one component; explicit `kin`/`recurrence` edges only,
  60 of 91. That is not a method failure, it is the finding — everything is kin
  to everything because these are seven laws restated with different nouns.
- **57 of 91 entries cite kin.** Authors have been hand-linking clusters for
  three months; the clusters existed and had no name until now.
- **Six entries are labelled recurrences outright**, and the C-chain contains a
  self-declared *fifth*. The ledger records its own failures to prevent.
- **Only 16 of 63 body entries carry a `Status:` line**, and only two record a
  discharge — R-1 and R-3, both promoted to SKILL.md in May and still sitting in
  the active file. There is no disposition field, which is why `Promote-when`
  criteria go unharvested and why the archive policy had nothing to enforce
  against. **Adding a `Status:` to every entry is the single highest-value
  structural change to this tracker.**
- **Removal recovers less than entry counts imply.** Archiving 13 of 91 entries
  cut 5.8%, not ~14%, because roughly a third of the corpus lives in
  self-contained index-table rows rather than bodies. Second measurement of the
  same effect that day (the D1 guide dedup realised 41% of its byte estimate).

### Reproducing the per-entry assignment

The per-entry table (theme, canonical law, `promote-when` state, `dup-of`) is
deliberately **not** transcribed here. It is not the expensive part: deriving the
seven themes was, and they are above. Re-classifying entries against a taxonomy
that already exists is mechanical, and transcribing 99 rows across two suffix
schemes — this file's date-based `b`, and the reading-order `a`/`b` one classifier
used — is exactly where a wrong id would enter and be trusted.

To reproduce: classify each entry's full text against A-G above, one primary
theme each, and record `dup-of` strictly (near-identical law, not merely same
theme). Read **both** formats — heading entries and self-contained index rows —
or roughly a third of the corpus is invisible, which is the error that produced
the id collisions in the first place.

The chains recorded under each law are the durable output of that pass and should
be treated as findings, not as a summary to re-derive.

### Still open after this pass

1. **Three supersessions for R-67..R-91 are NOT applied.** The classifier that
   found them labelled collisions `a`/`b` by reading order while this file's
   suffixes go by date, so its `R-73b` and this file's `R-73b` are different
   entries. They need per-entry re-identification against the suffixed file
   before archiving — acting on that mapping would let the id-collision defect
   corrupt the cleanup meant to fix it.
2. **R-90's `Promote-when` may already have fired.** Its criterion is "a third
   instance"; its own body documents one (`543086d1`, the mirror-direction
   annexation). If it has, the ratified policy says promote — and R-90's
   promotion target is adopting per-session git worktrees as the default for
   concurrent sessions. That is a workflow change and needs a human call.
3. **The A-chain and C-chain are candidates for promotion, not archiving.** A at
   35% and the five-deep C recurrence are the two laws this project demonstrably
   cannot hold in working memory; they belong in a surface that is delivered, not
   in a 228 KB file that must be opened. Note the constraint: seven of ten
   `get_guide` topics have no trigger at all
   (`docs/issues/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`,
   BL-25), so "put it in a guide" is not by itself delivery.

## Index

> **Id-suffix convention (2026-08-16).** Nine ids had been allocated twice, to
> unrelated lessons — the ledger carries two entry formats (`## R-N` + body, and
> a self-contained index row), and allocating by `grep '^## R-'` is blind to the
> second, so numbers already taken by rows were handed to new bodies. Resolved by
> **suffix, not renumber**: for each collision the EARLIER instance keeps the bare
> number, so the 57 existing kin-citations still resolve to what they most likely
> meant, and the later instance takes `b` — R-55b, R-56b, R-57b, R-58b, R-59b,
> R-72b, R-73b, R-74b, R-76b. Renumbering was rejected: it would have destroyed
> which instance a citation meant, and that information exists nowhere else.
>
> When allocating a new id, count **both** formats —
> `grep -o '^## R-[0-9]*' <file>` **and** `grep -o '^| R-[0-9]*' <file>`. The
> heading-only grep is what caused this.
> (`docs/issues/2026-08-16-reconnaissance-ledger-reuses-ten-ids-for-different-lessons.md`)

| ID | Date | Verdict | Pattern | Evidence (session-log) |
|----|------|---------|---------|------------------------|
| R-1 | 2026-05-19 | hit → promoted | Pre-dispatch grep for asserts on `include_str!`'d constants | mcp-prompt-redesign F-1 + W-1 |
| R-2 | 2026-05-19 | **archived** → R-4 + R-34 | Scout missed constant-write patterns (`.replace(TOKEN, ...)`) | mcp-prompt-redesign F-2 |
| R-3 | 2026-05-19 | miss → promoted | Scout limited grep to one file/crate; cross-file asserts slipped | mcp-prompt-redesign F-2 |
| R-4 | 2026-05-19 | miss | Grep undercounts struct-field construction sites by 2-3× | mcp-prompt-redesign F-3 + W-2 |
| R-5 | 2026-05-19 | **archived** → R-34 | Add "compiler as scout" as a Phase-1 tool alongside grep | covers R-4 |
| R-6 | 2026-05-28 | hit | Explicit recon invocation on substrate before mechanism design | prompt-guide-refactor F-2 + W-2 |
| R-7 | 2026-05-28 | **archived** → R-1 | Invariant test on `include_str!`'d file not pre-enumerated | prompt-guide-refactor F-4 + W-3 |
| R-8 | 2026-05-28 | miss → proposal | `edit_markdown(action='replace')` shape unverified on marker-bearing section | prompt-guide-refactor F-7 |
| R-9 | 2026-05-28 | proposal → drafted | Session-state recon for subagent dispatch | prompt-guide-refactor F-6 + W-4 |
| R-10 | 2026-05-29 | miss → proposal | Buffered tool output parsed for structured extraction without a completeness scout | metadata-filtering F-4 + W-1 |
| R-11 | 2026-05-30 | hit → proposal | Concept docs diverged from code on concurrency semantics (GRADLE_USER_HOME "isolation"; per-path mux) | issues/2026-05-30 concurrency bug files |
| R-12 | 2026-05-30 | hit → proposal | Plan's proposed data structure cited the symptom layer, not the structural layer (flat `ActiveProject` HashMap vs existing `Workspace` registry) | concurrency-fix F-1 |
| R-13 | 2026-05-30 | hit → proposal | Cross-repo doc drift: codescout `CLAUDE.md` stale vs `claude-plugins` hook (cd-passthrough removed, wrong filename, +9 undoc'd hooks); intra-repo `audit_doc_refs` structurally can't see it | commit 7187396a |
| R-14 | 2026-06-01 | hit (confirmed) | Specialist cited a dated memory (`outputguard-cross-cutting-law`, 2026-05-07) as a load-bearing design claim (`@ref` buffers process-local); scouted current code before the design rested on it — confirmed | `output_buffer.rs:42` |
| R-15 | 2026-06-03 | hit | Scout external-tool on-disk state against bug-doc claims before a fix depends on addressing it (analyzer dir 128-bit hash ≠ codescout 64-bit `ws_hash`) | kotlin-lsp-disk F-1 |
| R-16 | 2026-06-04 | hit → promoted | Pre-dispatch scout of the plan's OWN splice code caught a double-newline bug (+ substring-overlap test mis-routing 3× → CLAUDE.md; whole-workspace `cargo fmt` churn caught at pre-commit diff-scout) | edit_file-normalized-fallback (this session) |
| R-17 | 2026-06-05 | hit | Spot-check sibling callers of a just-fixed shared helper before closing the bug class (`references(clamp_range_to_parent)` found `do_remove`/`do_replace` shared the off-by-one) | bug-fix W-9 + issues/2026-06-05-edit-code-insert-after-last-python-method |
| R-18 | 2026-06-05 | hit | Scout a classifier's actual return type AND domain coverage before keying a feature off it (`detect_language` returns `Option<&str>` not an enum, and does not recognize YAML → guard keyed on extension, not name) | edit_file indent-significant guard (this session); commit c99d4228 |
| R-19 | 2026-06-09 | hit, miss, then cross-session hit (2026-06-21) | Scout home-project internals before presenting cross-project "B benefits from A" recommendations (claim `@ref` no-dedup → confirmed; claim "summarization generic" → drift: `format_compact` is per-tool; 2026-06-21 re-scout re-confirmed all 3 shapes + caught line-citation drift) | this session + 2026-06-21 re-scout; `output_buffer.rs:251`, `types.rs:435`, `sync.rs:34`; kin R-14 |
| R-20 | 2026-06-09 | **archived** → R-49 + R-3 | A bug-file's hand-cited fix-plan line list is not the blast radius (workspace-root grep found a 3rd build-breaking assert at `tests.rs:257` + a 0-match fixture lead) | bug-fix F-15 + W-10 |
| R-21 | 2026-06-09 | hit | Verify a side-effect through its real production entry point (CLI/MCP), not a unit harness that bypasses `main.rs`; `references()` the operation to enumerate ALL call sites before placing it (`sync_project`: 5 sites, write reached 1 of 3 project paths) | index-freshness F-1 + W-1; commit 10dcfb9f |
| R-22 | 2026-06-11 | hit | Scout the LSP call path to confirm a staleness mechanism before choosing the fix layer (references false-zero: `did_open` syncs def-file only, no barrier; all LSP signals share the staleness so the fix must be LSP-independent) | issues/2026-06-09-references-false-zero; commit ddc7e3f1; kin R-21 |
| R-23 | 2026-06-11 | hit, then miss | Re-derive an inherited diagnosis from usage.db telemetry (hit); verify a shared single-holder-resource recovery by READING lock/process state, not by issuing a call from a 2nd client which re-creates the contention (miss) | bug-fix F-16 + W-12; issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision |
| R-24 | 2026-06-11 | hit | Scout the resource-key derivation before designing a concurrency test; path-keyed hashing makes worktrees a safe fan-out fixture | bug-fix W-13 + F-18; issues/2026-06-11-lsp-tools-ignore-workspace-pin-path |
| R-25 | 2026-06-11 | hit | Scout the catalog's status source + id-keying before archiving a librarian tracker (status lives in the catalog row not the file; `id=sha256(abs_path)` so `git mv`+reindex orphans history) → `artifact(update)`+`artifact(move)` | this session; commit `b487a69c`; kin R-24/R-15 |
| R-26 | 2026-06-11 | hit | A grep line-match locates a symbol; it doesn't confirm a mechanism — read the body before narrating "confirmed" (`kill_on_drop`→SIGKILL-orphan verified at `process.rs:66-135`, no reaper in the spawn path) | this session — mux-LSP-sharing brainstorm; kin R-19/R-5/R-23 |
| R-27 | 2026-06-12 | **archived** → R-26 | A subagent's prose control-flow claim is a hypothesis, not a finding — read the fn body before a fix rests on its mechanism (Explore report: "`OutputForm::Text` forces inline always" + recommended it as the fix; `call_content` gates the buffer on `exceeds_inline_limit` UNCONDITIONALLY before the form branch) | bug-fix F-19 + W-14; kin R-26/R-19/R-9 |
| R-28 | 2026-06-12 | hit + miss | Enumerate a prompt surface's full gate set before editing (byte-for-byte slice, snapshot fixture, cap, ONBOARDING_VERSION pin, content tests); targeted `cargo test --lib <module>` filters miss cross-cutting gates in `server::tests` (over-budget get_guide description shipped via a narrow filter) | bug-fix F-20 + W-15; kin R-1/R-7/R-27 |
| R-29 | 2026-06-13 | hit | Verify a flight-recorder-harvested target exists in the active repo before ranking/acting on it — `.codescout/usage.db` is keyed by commit-SHA and mixes every project the process served (40 project_shas; a mirela `CalendarService` phantom surfaced in a codescout survey) | dzo-legibility F-1 + W-1; kin R-23 |
| R-31 | 2026-06-14 | **archived** → R-49 + R-3 | A bug-file's "param never parsed anywhere" claim, evidenced by a grep scoped to the modified file(s), missed a parser one call-hop away — `read_file`'s "offset/limit never parsed" missed `OutputGuard::from_input` (`output.rs:96`), which `read_full_file` calls. Follow the param into callees; don't trust the file-scoped audit. | bug-fix F-22; issues/2026-06-14-read-file-offset-limit; kin R-3/R-26 |
| R-30 | 2026-06-13 | miss → proposal | When a structural feature appears to *block* a tool, test the tool's documented alternate addressing form before refactoring code around it — read `find_unique_symbol_by_name_path`/`symbol_name_matches` but never tried the qualified `impl Trait for Type/method` form the not-found hint itself *documents*, so concluded `edit_code` was blocked and relocated 11 trait impls for a non-block. Reading the resolver ≠ testing the escape hatch. | dzo-legibility F-9; ADR docs/adrs/2026-06-13-drop-name-collision-defect.md; kin R-26/R-27 (read ≠ verified) |
| R-32 | 2026-06-14 | hit | An off-the-cuff root-cause unification across a bug cluster is a hypothesis, not a finding — read each bug's implicated code before presenting "they share one root cause / one fix" (3 open "catalog/path" bugs by title → 3 distinct files/mechanisms; only 2 shared a narrower root: global-abspath catalog reasoned-about-as-per-workspace) | bug-fix F-21; issues 2026-06-13 catalog trio; kin R-12/R-19/R-27 |
| R-33 | 2026-06-15 | hit | A reconciliation audit marked two adjacent legacy-era symbols (`fusion::rrf_fuse`, `schema::SearchResult`) both "graduated to live — keep"; `references()` showed OPPOSITE liveness (rrf_fuse test-only → deleted; SearchResult live → kept). Dead-vs-live is per-symbol call-graph, not file-proximity. | legacy-retrieval-removal L-08; this session; kin R-26/R-27/R-21 |
| R-34 | 2026-06-15 | hit | On a cross-platform branch, the host `cargo check` is necessary-not-sufficient for a rebase — it never compiles the gated target. After rebasing `vdi-windows` onto `experiments` (conflict in the `#[cfg(unix)]` peer gate itself), cross-compiled `--target x86_64-pc-windows-gnu` to confirm the incoming commits added no ungated unix-only code (EXIT=0). Compile the target the branch exists for, not just the host. | `vdi-windows` rebase this session; `src/tools/mod.rs` peer-gate; WIN-23; kin R-5 |
| R-35 | 2026-06-16 | hit | A tool's own error diagnostic is a hypothesis, not ground truth. `edit_code`'s "AST parse failed — likely syntax errors or duplicate siblings" was falsified by `symbols()` (clean parse, unique name_path); the archived backtick bug was already fixed. A throwaway dump of `extract_symbols_from_source`→`find_ast_end_line_in` on the real file pinned the real cause in one run: AST start 214 (annotation line) vs LSP 216 (`fun` line), matcher flips Some→None at the ±1 gate. Reproduce the failing internal call when a cheaper read disagrees with the error text. | bug-fix W-17/F-23; issues/2026-06-16-kotlin-edit-code-annotation-line-gap; kin R-5/R-32 (diagnostic/claim ≠ ground truth) |
| R-36 | 2026-06-28 | hit (validates R-3 / R-20) | Struct/config-field removal blast radius includes serde **data fixtures**, not just source — scope the verification drift-grep to `tests/` (incl. `tests/fixtures/**/*.toml`), not just `src/`. `SecuritySection` has no `#[serde(deny_unknown_fields)]`, so stale keys (`shell_enabled`, `shell_output_limit_bytes`) in two fixture `project.toml`s were silently dropped — `cargo test` stayed green; only the `src/ tests/` drift-grep surfaced them. Same serde-silent-ignore property that makes the user migration a footgun is what hid the fixture drift from compiler + tests. Promote-when (2 datapoints) → codescout memory `reconnaissance` (project-shaped: Rust+serde+TOML). | issues/2026-06-28-vestigial-shell-output-limit-bytes; tests/fixtures/{rust,kotlin}-library/.codescout/project.toml:18-19; kin R-3/R-20 |
| R-37 | 2026-07-03 | **archived** → R-28 | Adding a get_guide topic edits TWO gated surfaces beyond registration: the tool description (300-char budget test in `server::tests`) and the no-arg listing (hardcoded topic count). Scout the tests' assertions, not their names; convert hardcoded counts to derive from the canonical const. Full `--lib` gate absorbed the miss pre-commit. | tracker-as-skill session (untrusted-content topic, A-5); kin R-28/R-1/R-7 |
| R-38 | 2026-07-03 | **archived** → R-55 | Re-derived a finding a concurrent session had already MEASURED because the existing audit-log wasn't scouted first — independently built a precision gate for the tracker-hygiene D1 gap, then found the plugins Hamsa ledger had already measured it (n=5, both tiers). The seam for a derive/record-a-finding task includes the KNOWLEDGE state (ledger entries on the subject), not just the code. | A-8/A-9 session (#22); plugins Hamsa ledger; kin R-19 |
| R-39 | 2026-07-10 | hit | Adding a tool param/alias is additive-safe in codescout: every `*_schema_*` test is positive-presence (`props["x"].is_object()` / `contains_key`), none enumerate the exact prop set, and no `input_schema()` sets `additionalProperties:false` — so a new prop can't break a snapshot, and an unknown key flows through to `call()` (the very mechanism the alias fix relies on). | param-alias-ergonomics session (this session); 3038 lib passed / 0 failed; kin R-28/R-36 |
| R-40 | 2026-07-10 | **archived** → R-69 | usage.db error COUNTS are commit-mixed AND time-spanning — a high-count friction may already be FIXED in current code; verify the candidate against today's substrate before fixing. The `json_path` quoted-key friction (~200 events, the largest non-alias cluster) was already closed 2026-07-01 (`split_on_unbracketed_dot` + `strip_matching_quotes`); a count-only ranking would have "fixed" it twice. | param-alias-ergonomics session (this session); `file_summary.rs`; kin R-29/R-23 |
| R-41 | 2026-07-17 | miss → promoted | A later table-rebuild migration (`CREATE _new`/`INSERT … SELECT`/`DROP`/`RENAME`) has a column list that is a silent ALLOW-LIST — a column an earlier migration added but the SELECT doesn't name is dropped on swap, no error. Adding a column is a seam whose far side is every later rebuild's SELECT. | Stage-2 review; `migrate_v6.rs::drop_legacy_and_stamp` dropped `slug`; fix 9aa8063f + test `migration_v6_single_open_preserves_v9_entry_graph_shape`; kin R-3/R-28 |
| R-42 | 2026-07-17 | miss → promoted | When a writer produces a new value shape (id-keyed ref, optional field), each reader's absent-key/None branch must RESOLVE the other shape, not dead-end (return empty / fall through) — a dead-end silently drops every value stored in that variant. Shared incidental test preconditions ("target always has a slug") mask it. | Stage-2 review; `get(include_links)` hid incoming-by-id backlinks for slug-less targets; fix 70d16686; kin R-27/R-21 |
| R-55 | 2026-08-06 | miss → proposal | **A file's open-bug ledger is part of its shape — query it by FILE IDENTITY, not by task category.** The bootstrap guide already carries the rule ("Bug or regression work: `artifact(find, kind=bug, status=open)` … don't re-file a filed bug as new") and it was auto-injected into the session's FIRST tool response. It still missed, because its trigger is the task's self-classification: a session framed as *docs + merge prep* answers "am I doing bug work?" with no, right up until it edits a file that has an open high-severity bug against it. Retrigger on the edit instead: before the first change to any file, `artifact(action="find", kind="bug")` constrained to that path or subsystem. What the ledger holds is not reachable any other way — the code does not show it and neither does git log: **chosen-but-unimplemented fix candidates, preconditions on landing, and cross-change sequencing decisions.** | release-promotion F-2; `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (open, high) named this exact fix as candidate 1 verbatim — reimplemented at `ca442498` with its stated precondition ("validated against the retrieval benchmark … must be measured, not assumed") unmet and its recorded ordering (throughput work first, being vector-identical) jumped; separately re-filed `2026-07-28-audit-doc-refs-json-pointer-false-positive` as new. Both surfaced ~60 tool calls late, by an unrelated verify-open pass, not by recon. Kin R-49 (re-scout your own bug file before implementing it), R-19 |
| R-56 | 2026-08-06 | miss → proposal | **The record of a change is not the change — verify at the artifact, never from the report of it.** Two shapes, same session. (a) A written prescription goes stale against later edits of *its own file*: three bug files' `## Fix` sections were materially wrong — one closed with "that figure needs correcting regardless" while a later **CORRECTED** note at the top of the same file retracted the comparison the claim rested on; one called a duplication "harmless in size" when a pre-fix test showed it duplicated to EOF; one ranked "exit on stdio EOF" as the cheapest fix when reading the code showed it was already implemented. (b) A *session summary* asserted tracker work that never landed: it recorded WIN-28/WIN-29 rows added to `windows-platform-support.md` and an R-55 entry written — the WIN rows were absent entirely, and R-55 existed as an index row with no body. Proposal: before citing any prior entry as done, grep the target artifact for the ID. Before implementing a bug file's Fix, read the code it names and check for a later dated note in the same file that contradicts it. | release-promotion F-4; ast-chunker trailing-gap dump (chunk `"}\n\nfn tail_marker…"` lines 12-16 proved EOF-wide duplication); `ResilientStdin` absorbs only `WouldBlock` so 0-byte EOF already propagates; `grep 'WIN-2[89]' docs/trackers/windows-platform-support.md` → 0 matches. **Already at 2 datapoints — promote-when has fired:** `bug-fix-session-log` F-27 records the identical shape from an earlier session ("Prior-session summary claimed two bug files were logged; neither exists on disk or in git history"), and F-24 a near-miss ("documented 'implemented on experiments' but is an uncommitted working-tree change on no branch"). Three independent instances of *the record asserting work that did not land*. Promote to CLAUDE.md as: **before citing prior work as done — an entry, a row, a commit, a file — grep the artifact for it.** Kin R-19 (read the symbol before asserting a checkable fact), R-49, R-55 |
| R-57 | 2026-08-06 | hit → proposal | **When the seam is a TOOL, the scout is one real invocation whose output you read — and first, a check that the artifact you invoked was built from current source.** Reading an implementation tells you what it intends; running it tells you what it does. Five defects lived in that gap, and every one surfaced from output, not from source: (a) `matched_path_or_any_parents` **panics** rather than errors on an out-of-root path — unknowable from a signature returning `Match`, not `Result`; the first real run died with SIGABRT (exit 134). (b) `resolve_file_line` had no outside-project guard, so a doc citing `/etc/passwd:12` came back **`Resolved`**, range-checked against the host's real file — found because a test written for the panic asserted on the actual verdict. (c) The extractor walks *code-block text* (`Event::Text(content) if in_code_block`), not only spans and links as its own bug file asserted — measured split was 18 prose / 18 fenced / 2 indented, so half the population was invisible to the written claim. (d) `RefPosition` was populated by the parser and read by **nothing**: the discriminator the fix needed already existed, visible only by grepping its uses. (e) mdBook link semantics — ~28 correct links reported missing because the resolver knew one of the repo's two conventions. **Verify the instrument before the measurement:** the local release binary was two days old and predated all four source files of the tool under test; `find src crates -name '*.rs' -newer target/release/codescout` caught it before a single number was trusted. Proposal: for tool-behaviour seams, add to Phase 1 — *invoke once, read the real output, and confirm the binary is newer than the sources you changed.* | This session: `EXIT=134` panic at gitignore.rs:232 in the vendored ignore-0.4.25 crate (un-code-spanned: it is a registry path, not a repo path); `assertion left != right … /etc/passwd:12 must not resolve, left: Resolved`; `grep RefPosition` → 13 matches, all writes + test constructors, zero reads in the decision path; `ls -la target/release/codescout` 2026-08-04 vs. four newer `audit_doc_refs/*.rs`. Kin R-50 (the view is not the set — the 50-finding cap again made every aggregate wrong, ×5 low), R-19, R-56 |
| R-58 | 2026-08-06 | miss → proposal | **A gate whose verdict depends on the filesystem must be scouted against a CLEAN CHECKOUT, and a rule about paths must be tested in both `foo` and `foo/` form.** Two blind spots, one incident. (a) `Audit Doc Refs` exited 0 locally six consecutive times and went red in CI. The four findings were `.git/worktrees/`, `.claude/worktrees/`, `.codescout/private-memories/` — directories a working tree *has* and a fresh clone has not, so locally the refs **resolved** and produced no finding at any severity. The local run was not a weaker gate; it could not see the class. A working tree is strictly more populated than a checkout (untracked runtime state, worktrees, build output, caches), so every existence check is optimistic in the direction that yields a false pass. Fix, and it costs one command: `git clone --depth 1 file://$PWD /tmp/x` and scan that with the same binary — 881 files, 46673 refs, 0 high, proof rather than a guess, and no CI round-trip. (b) The cap that should have caught two of them **had a passing test** — asserting `.worktrees/my-feature`, a file *inside* an ignored dir. Docs name the directory, `.claude/worktrees/`, and the matcher reads the final path component, which a trailing separator leaves empty; every directory-shaped ref escaped while the suite stayed green. The fixture shape was the blind spot, not the assertion. Proposal for Phase 1: when the seam is a filesystem-dependent gate, scout a clean checkout; when the change is a path predicate, enumerate both separator forms. | release-promotion W-7; CI run 31106899289 `Audit Doc Refs: failure` with 4 high vs. local `EXIT=0` ×6; fix + fresh-clone proof in `6348dfad`. Kin R-57 (run the tool, don't read it — this is the same law applied to the *environment* rather than the binary), W-5 |
| R-62 | 2026-08-07 | hit → proposal | **A bug file's `## Fix` section is a plan, and plans get scouted like any other plan — read the code it prescribes changing before you implement it.** WIN-30's Fix said the background-output test used a fixed wait that should become a bounded poll. The test *already* polled — `for _ in 0..50` with a 100 ms sleep, a 5 s bound — because the Fix had been written from the CI log line (`background command output not captured`) without opening `src/tools/run_command/tests.rs`. Implementing it as written would have been a **no-op committed as a fix**: green suite, archived bug, surviving flake, and a bug file that then asserts the poll was fixed — so the next occurrence reads as a regression of something that never happened. The scout also relocated the real defect, which the prescription had missed entirely: the poll's `if let Ok(v) = out` discarded the `Err` arm, making "never ran" and "still flushing" indistinguishable, which is *why* the CI red carried no information. Prescriptions derived from failure output are the highest-risk kind, because that output is exactly what the plausible-but-wrong mechanism would produce. Proposal for Phase 1: scout a bug file's or plan's prescribed fix like plan code; a `## Fix` naming no `path:line` for the code it changes has probably not opened it. | WIN-30 (`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`), Root cause + Fix corrected in place; release-promotion F-11 / W-11. The replacement diagnostic paid off on its first run under wine, naming the missing `py` and retiring one member of WIN-27's twelve-test unknown-root-cause cluster. Kin R-19, and the same doc-vs-code drift class as F-3. **Third datapoint, self-referential, same session:** the `grep` bug file I wrote an hour later prescribed inverting `hidden()` into a counting `filter_entry` closure — derived from reading `src/tools/grep.rs:106` but not the `ignore` crate's semantics, where `hidden` also honours `FILE_ATTRIBUTE_HIDDEN` on Windows. Implementing it as written would have silently changed which files are searched on the one platform the repo cannot test locally. So the rule applies to a Fix section written from *partial* code reading too, not only from log lines: scout the dependency's contract, not just the call site that names it |
| R-63 | 2026-08-07 | hit | **A target's existing tests encode the behaviour you are about to change; read them before editing, not after the gate turns red.** About to invert `resolve_file_symbol` so a mid-index LSP stops reporting existing symbols as missing. Scouting the function first surfaced `resolver_prefers_disk_truth_on_lsp_lag`, which asserted exactly the behaviour being removed — and whose **own name was the argument against its assertion**: it wrote `pub fn bar() {}` to disk and then expected `bar` to be reported missing, when disk truth is that `bar` exists. Renamed and inverted deliberately, with the inversion recorded, rather than discovered as a failing test and "fixed" by adjusting the assertion — which is the likely outcome under time pressure, and which would have quietly re-encoded the old rule. The same scout also found `docs/superpowers/specs/2026-05-16-audit-doc-refs-design.md`'s Tier-1 table stating the old rule as *design intent*, a surface no test covers and `audit_doc_refs` cannot check (prose semantics, not refs). Sibling of R-46: a test module encodes intent the implementation cannot reveal — R-46 read it to CHOOSE between fixes, this read it to discover the fix contradicted a documented one. | `src/librarian/tools/audit_doc_refs/resolver.rs`; commit `6e608545`; release-promotion #17/#18 |
| R-64 | 2026-08-07 | hit | **Scout RUNTIME state, not only code, before committing to a long job — a config file's claim about the datastore is a claim, and the datastore can be asked directly.** Task #21 was a ~2h `index --force`, justified partly by `.env.gpu` asserting that re-enabling sparse makes a force reindex **mandatory** because chunks written while the flag was set carry an empty sparse vector. Before running it, the live Qdrant collection was queried: it already declared a `sparse` vector (`modifier: idf`), and **92.3% of a 2000-point sample carried POPULATED sparse vectors, 7.7% empty, 0 absent.** The prediction was directionally right and the magnitude — unstated, and load-bearing — was wrong: 7.7% degrade gracefully to dense-only in RRF, so nothing gated the flag flip. `absent` would have been the real hazard, since a missing named vector can error a hybrid query; absent was zero. Generalisation: when the seam is "what state is the datastore actually in", the scout is a query, not a grep — and it is available *before* the expensive operation, not only after it fails. | `.env.gpu` re-enable note vs Qdrant scroll; commits `da5176d5`, `9886773e`; release-promotion round 9 |
| R-65 | 2026-08-07 | **miss → proposal** | **Build configuration is a seam, and it was not treated as one.** A `--force` reindex ran seven minutes dense-only against sqlite-vec instead of hybrid against Qdrant, because it was launched with `cargo build --release` — which omits the `server-stack` feature, so `VectorBackend::resolve()` returns `SqliteVec`, which sets `lite`, which sets `dense_only`. Two behaviours changed silently from one build flag. CLAUDE.md documents `cargo rb` for exactly this and `.cargo/config.toml` explains it in six lines, so the information was present and simply not consulted — the seam "which binary am I about to run, and what does its feature set change" was never asked. Caught only by comparing container traffic (dense 73,232 log lines in 2 min, sparse **zero**), i.e. by scouting side effects after the fact. Proposal for Phase 1: **before any job whose behaviour depends on compiled features, scout the build — confirm the binary's provenance and features, not just that a binary exists.** Cheap form: have the tool announce its own resolved configuration in line one, which is what the fix shipped (`backend="qdrant" sparse="on"`). The durable lesson is that a documented rule is not a scout; consulting it is. | release-promotion F-17; commit `5b77b8b8`; `src/retrieval/code_store.rs::VectorBackend::resolve` |
| R-66 | 2026-08-08 | hit | **When a gate reports N failures, scout the gate's own severity and scope policy before fixing N things — its silence is a policy statement, not evidence of correctness.** CI's `Audit Doc Refs` failed with exactly **one** high finding: a manual page citing a bug file that had been archived (path moved) the day before. The one-line fix was obvious and would have been wrong-sized in both directions. Reading the tool instead of the finding produced the real shape: `try_basename_fallback` returns `None` for any ref containing `/`, so all 25 dangling citations were genuinely `Missing`; `default_severity` bands `Missing => High`; and `apply_drops` keys on the **citing** document, so `archive/` and `docs/issues/` drop one band each. That is why 24 siblings were invisible — **by design, and correctly**: rewriting an archived snapshot to satisfy a linter that is already ignoring it would falsify the record. So the scope became 12 live surfaces fixed, 13 historical ones deliberately left. The same read then answered a question the gate cannot: `DEFAULT_AUDIT_GLOBS` omits `CHANGELOG.md` and `CONTRIBUTING.md` entirely — 18 broken refs and 8 high findings that no CI run will ever report. **Sub-lesson: a count that moves less than your edit count is a finding.** Fixing 5 markdown refs moved `n_refs_broken` by 3 (9080 → 9077); the missing 2 were the unscanned CHANGELOG pair, and that arithmetic gap is what exposed the coverage hole. Sibling of R-63 (read the target's tests) one level up: read the *judge's* rules, not just the verdict. | `src/librarian/tools/audit_doc_refs/{resolver.rs:82, severity.rs:21, mod.rs:149}`; CI run 31218234007; `docs/issues/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md` |
| R-67 | 2026-08-08 | hit | **Scout the artifact that claims the work is already done — a doc comment, a test name, a count — because that is the thing which stops the next reader from looking.** Three instances in one PR review, each confirmed at the bytes. (1) `resolve_git_bash_never_selects_the_wsl_launcher` passed because it injected `PATH=C:\Windows\System32`, the spelling the CODE constructs, while Windows setup writes `system32` and `Path` equality folds case only on the drive-letter prefix — so the exclusion never fired on a real host, and the test was named after a guarantee it did not provide. (2) `posix_tokenize`'s doc said it "feeds the security layer's dangerous-command and pipeline checks"; `git grep` returns zero production callers and `is_dangerous_command` still uses `split_whitespace`. (3) `summary.total` printed a confident headline number taken over a truncated set while `by_check` counted the full one. Each names a real hazard and asserts it handled, which is strictly worse than silence: a reviewer greps for the coverage, finds it, and stops. **The scout is cheap and specific — for a claim, find the caller; for a test, ask what mutation survives it; for a count, ask which set it was taken over.** Third sibling to R-63 (read the target's tests) and R-66 (read the judge's rules): read the *reassurance*. | release-promotion F-21; `src/platform/{windows.rs,mod.rs}`, `src/librarian/tools/doctor.rs`; commit `6f261da9` |
| R-68 | 2026-08-08 | hit | **When dispatching reviewers at a diff, the seam is WHICH TREE they will read — and the default is the wrong one.** Four subagents were sent at PR #10 while the shared checkout sat on `experiments`, the merge BASE. `symbols()`, `grep()` and `read_file()` all resolve against the working tree, so every reviewer would have read pre-PR bytes and reported confidently on code not in the diff — a review that looks real and is worthless, with no signal distinguishing it from a good one. The brief therefore pinned three refs explicitly (base `47998a33`, head `57ac707f`, tested merge ref `refs/tmp/pr10merge` = `a42f124b`) and mandated `git show <ref>:<path>` into a scratch file before reading. Related and non-obvious: for a `pull_request` event the tested tree is the MERGE ref, a synthetic commit existing on no branch — so "review the PR" and "review the branch tip" are different requests against different bytes (F-23). Iron Law 6 in its sharpest form: a subagent mis-discovering what the controller already knew is a dispatch defect, and the controller is the only party positioned to notice. | release-promotion W-15, F-23; PR #10 review, 2026-08-08 |
| R-69 | 2026-08-08 | miss | **Before working from a bug file, plan, or spec, `git log -- <the file it names>`.** A bug file is a claim about the world at the moment it was written, and nothing re-validates it. The AST-chunker file was maintained carefully by two sessions — they corrected the "~2h" re-embed to a measured 8.06 min and corroborated the 12% single-line figure from an independent direction — while the fix had already shipped in `ca442498` two days earlier and the file still read "Not yet implemented". Every number was right; the code had moved. Numbers-auditing cannot detect this, because the one surface that changed is the one a numbers audit never touches. The command is one line and would have caught it on 2026-08-07. Generalises past bug files to any document that names code it does not contain. | release-promotion F-30; chunker file reconciled 2026-08-08 |
| R-70 | 2026-08-08 | miss | **Before citing a test as coverage, confirm it RUNS — `cargo test --test <file>` and look for the name in the output.** A `#[cfg(feature = "x")]` on a test is a claim that some lane enables `x`; verify it (`grep -rn '<feature>' .github/workflows/`) rather than assume. `server-stack` was in neither `default` nor any workflow, so its tests were never compiled — and a filtered-out test is indistinguishable from a test that does not exist, which is worse than an obvious hole because the file *looks* covered: grep for the symbol, find a test named after it, stop. Three mechanisms make a test unable to fail (deleted with its consumer, asserting a subset under a name claiming all, never compiled) and all three were live in this repo on one afternoon. | release-promotion F-31; buddy memory `tests-that-cannot-fail` |
| R-71 | 2026-08-08 | hit | **A remediation hint inside an error message is a claim about an API, not an instruction — verify the symbol before writing it.** `qdrant-client`'s failure text reads *"Set check_compatibility=false to skip version check"*, and `check_compatibility` is a struct field; the public builder method is `skip_compatibility_check()`. Writing what the message said would not have compiled. Cheap in Rust, where the compiler is the scout of last resort — the same reflex against a runtime-resolved name (an env var, a JSON key, a CLI flag) fails silently at runtime instead, and the message's authority makes it the least likely line anyone re-checks. Scout cost was one grep of the vendored crate, which also surfaced that the diagnostic is a `println!` rather than `eprintln!` — the actual defect. | release-promotion F-32 + W-24; `docs/issues/archive/2026-08-08-qdrant-compat-check-printlns-to-stdout.md` |
| R-72b | 2026-08-13 | miss | **A git SHA in a subagent brief is a claim with an expiry date — re-derive HEAD at dispatch, and tell the agent to report drift.** Briefed an agent with `experiments @ c5f434df`, five days stale. The checkout had moved to a feature branch carrying **+2,592 insertions across the retrieval query path** — the exact subsystem under investigation — so every `path:line` in the returned report silently described a different branch than the one requested. The behavioural findings survived because they were observed against a running server; the mechanism citations did not. Scout cost is one `git branch --show-current && git log --oneline -1` at dispatch time. Cheaper still, add *"verify the branch and tip yourself and report any disagreement with this brief"* to the brief — the agent found it here unprompted, but only because it happened to run `git worktree list`. Generalises to any brief carrying a version, path, or line number the parent has not re-read this turn. | release-promotion F-34 |
| R-73b | 2026-08-13 | miss | **Before reasoning about whether a mechanism exists, grep the layer that would own it.** Twice in one session I inferred absence from architecture and was wrong: *"MCP pushes no cwd change, so there is likely no signal to miss"* (a `PostToolUse` hook on `EnterWorktree` already fires and instructs — `hooks.json:141`), and *"how would we express an exclusion across two backends with different `project_id` semantics"* (`exclude_languages` was already that mechanism, contract-tested in both stores, divergence documented in-code). Both inferences were structurally sound and both were false, because architecture tells you what *can* exist, not what *does*. The refutation is a single grep of the owning layer — the harness directory for a harness signal, the sibling axis for a filter mechanism — and it is cheaper than the reasoning it replaces. It also changes the work: "design this" becomes "extend that". | release-promotion F-35 |
| R-74b | 2026-08-13 | hit | **Before recommending at a trait boundary, enumerate the implementors and read the second one.** A fix design was recommended after reading `qdrant.rs:317`'s `must_not` filter; the second `CodeVectorStore` impl opens a database **file** per `project_id` (`sqlite_code_store.rs:70`), making the recommendation inexpressible there. The signature `query(collection, project_id, …)` is what hides it — one parameter name, two meanings — and the pair is contract-tested for parity, so the suite would have stayed green while the backends diverged in behaviour. The scout is `grep -n "impl .* for"` on the trait's file, or `references()` on the trait. The durable form: "the shape of the impl I read" is not "the shape of the boundary", and a parameter name shared by two implementations is not a shared meaning. | release-promotion F-36; caught by applying architecture-snow-lion Self-Trap 2 |
| R-72 | 2026-08-11 | miss (recurrence of R-70) → proposal | **A plan's code block is a claim about an API, and the workspace usually already holds the answer — grep for an in-repo user of the type before transcribing, and check which lane actually compiles the file.** PR #13's `LocalOnnxEmbedder` was transcribed verbatim from its plan (`docs/superpowers/plans/2026-08-08-local-onnx-embedder.md:403`, `fn embed_texts(&self)`) against `fastembed 5`, whose `TextEmbedding::embed` takes **`&mut self`** (`fastembed-5.13.4/src/text_embedding/impl.rs:447`, un-code-spanned: registry path, not a repo path). The correction was already in this workspace, written as a comment: `crates/codescout-embed/src/local.rs:82` — *"fastembed 5 changed embed() to &mut self — Mutex serializes access across spawn_blocking tasks"* — one `grep TextEmbedding crates/` away at plan-authoring time, and the line above it (`Arc<Mutex<TextEmbedding>>`) is the whole fix. R-70's mechanism is what let it reach CI: `src/retrieval/mod.rs:11` gates the module on `local-embed-dynamic`, which no test lane and no command in the PR's own five-line test plan enables — only `cargo check --features local-embed-dynamic --all-targets` (`.github/workflows/ci.yml:157`) compiles the file, so a PR reporting 3396 passing tests had compiled the file it exists to add exactly zero times. The author *did* run that command locally; it died inside `ort-sys`'s build script (CyberArk EPM, `os error 5`) before reaching the crate — an attempted compile that produced no compile, which reads in a test plan like an environment note rather than a coverage hole. Second error, same file, same root: `.unwrap_err()` needs `Self: Debug`, and `TextEmbedding` (`init.rs:127`) has no `Debug` impl, so the reflex `#[derive(Debug)]` is also uncompilable — both wrong fixes cost a full ort build to disprove, the grep cost nothing. Proposal for Phase 1: when plan code calls a third-party type, grep the whole workspace for an existing user before transcribing it; and treat *"which lane compiles this file?"* as part of the feature-gate scout rather than as CI's job — an attempted-but-aborted build is not a build. | pr-review F-5 + F-6 + W-4; CI run 31377316509 `Feature check (opt-in build configs): FAILURE` (E0596 + E0277) against 16 green lanes; `docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`. Kin R-70 (a `#[cfg(feature)]` test is a claim that some lane enables it — here a whole module), R-71 (an error message's remediation hint is a claim about an API; here a *plan* is), R-34 (the host build is not the gate for the target the branch exists to support) |
| R-73 | 2026-08-11 | hit → proposal | **A guard's unit test proves the guard works. It does not prove anything calls it — mutate the CALL SITE, not just the function.** Across one 9-task branch, three separate things were correct, unit-tested, and deletable from their call sites with the entire suite green: (a) `guarded_api_key`'s invocation — replacing it with the raw config key survived 3505 tests, and that mutation IS a project.toml credential sent cleartext to `http://embed.example.com`; (b) `guard_sparse`'s invocation — deleted, suite green, which is the silent dense degradation the guard exists to prevent; (c) the ENTIRE project-config read that a whole task existed to add — replaced with `let embeddings = None` and 3647 tests passed. In each case the function's own test genuinely died when the function body was mutated, which is what made the coverage look real. The two questions — *does this guard work* and *does this path invoke it* — read identically and are answered by different mutations. Fix pattern that worked three times: extract the wiring into a named, callable seam (`build_embedder`, `merge_embed_config`, `map_from_env_error`) so the guard sits inside something a test can invoke; testability shaped the factoring, and none of the extractions changed behaviour. Corollary observed twice: an UNCOVERED branch is where the next bug lands — a mutation that SURVIVED (`unwrap_or(fallback)` → `unwrap_or(0)`) marked the exact arm into which a later fix introduced a Critical two rounds afterwards. Proposal for Phase 1 and for every implementer brief: require the call-site mutation explicitly, and treat "the guard has a test" as necessary and not sufficient. | local-onnx-embedding session log F-1, W-2; `src/retrieval/client.rs` `build_embedder`; branch `feat/local-onnx-query-path`, 15 fix rounds. Kin R-49 (your own bug file is a hypothesis), R-57 (run the tool, don't read it) |
| R-76b | 2026-08-15 | miss ×2 → rule | **Aggregate behaviour data is a screen, not a verdict — and it is wrong in BOTH directions.** An audit of 53,916 recorded tool calls ranked failures by frequency and by "what tool came next". Reading the actual arguments of ~12 instances overturned two conclusions. (a) **Overstated:** every rejection of a `json_path` wildcard was scored a successful recovery because the next call returned `success`. The arguments showed what those recoveries were — `$.items[*].abs_path` → read 393 raw lines; `$.entries[*].id` → `$.entries[4].id` one element per call; another → abandon the buffer and re-query upstream; another → grep the buffer. A green outcome column concealed a degraded workaround every time: it records that the call returned, never that the agent got what it asked for. (b) **Understated:** a guard scored at 35% "same-tool recovery" was in fact working — its correct recovery is a DIFFERENT tool (`symbols(include_body)` on the exact symbol wanted), which the same-tool metric counts as failure. Same-tool and adjacent-call metrics systematically penalise every guard whose right answer is another tool or needs a lookup first (widening one route's window from 1 call to 3 moved compliance 45% → 74%). And the sharpest defect was invisible to all aggregation: bucketing the refused ranges showed 69 of 244 were canonical imports reads (`1-20`, `1-30`), refused by a guard whose recommended alternative structurally cannot return imports. **Rule:** before filing or closing anything from aggregate counts, read the arguments of ten instances — and check whether the payload that would show intent is even being recorded. | 2026-08-15 tool-usage investigation TU-11; `docs/trackers/2026-08-15-tool-usage-investigation.md`. Kin R-50 (the view is not the set — this is its behavioural-telemetry form), R-75 (verify the artifact produced, not the exit code) |
| R-75 | 2026-08-13 | miss → rule | **A process-level env scrub is not configuration isolation — a program that reads config from disk reconstitutes what you removed, and the result looks like success.** An end-to-end probe launched under `env -i` with three explicit `CODESCOUT_*` vars was treated as hermetic on that basis. `main.rs` calls `load_startup_env`, which reads `~/.config/codescout/.env` (here a symlink to the repo's own `.env.amd`) and fills every key the caller left **unset** — so a url that had just been scrubbed came back, and the retrieval path silently discarded the `local-dir:` model in its favour. Exit **0**, `added=5`, no warning; the only tell was the artifact's shape, `FLOAT[768]` where the local model is 384-dimensional. This is the substrate rule (read what world the tool read, not just its verdict) failing where it was most needed: the negative control — does this run still succeed when I make the config source *provably* absent? — was never run. The repo already knew the escape (`tests/cli_artifact.rs:18-24` sets `CODESCOUT_ENV_FILE` to a nonexistent path for exactly this reason) and it was not consulted. **Rule:** before believing an isolated run, name every layer that can supply config — process env, dotenv, user config, project config, compiled default — and neutralise or observe each; then verify the *artifact produced*, not the exit code. Note the compensation: chasing the impossible dimension is what exposed the real defect, so the sloppy probe still paid — but it would have paid as a false "verified" had the dimension gone unread. | local-onnx-embedding session log F-7 + W-5; `docs/issues/2026-08-13-url-silently-overrides-local-dir-model.md` (fixed `38e0980b`). Kin R-50 (the view is not the set), R-6 (scout the substrate before mechanism design) |
| R-77 | 2026-08-14 | miss (third recurrence) → rule | **A search that does not cover the space produces the same confident absence as no search at all — and feels safer, because you did look. When asserting that no artifact of a CLASS exists, search for the class across the tree, not for a filename in the directory where you expect it.** Briefing an implementer on companion-plugin hook work, I ran `ls` on `codescout-companion/hooks/`, saw `session-start.test.sh`, `worktree-write-guard.test.sh` and five other `*.test.sh` siblings, saw no `worktree-activate.test.sh`, and wrote *"this hook had no test while nearly every sibling has one"* into the brief. **`tests/test-worktree-activate.sh` had existed all along** — 86 lines, 6 tests, on the repo's shared `tests/lib/fixtures.sh` helpers, passing 6/6, covering four cases the new suite did not (silent exit without codescout, fallback worktree detection, the `.codescout` symlink, the embeddings symlink). The repo keeps suites under **two** conventions (`hooks/*.test.sh` and `tests/test-*.sh`) and I searched one, so the implementer correctly built a second, redundant harness against a false premise. This is the **third** instance of the same class in one work stream, and the disguise is what makes it worth its own row: the first two were architectural inference (concluding the `EnterWorktree` hook did not exist — it was at `hooks.json:141`; concluding no exclusion mechanism existed — `exclude_languages` was already contract-tested in both stores), where the tell is *"I reasoned instead of grepping"*. Here I **did** search, and the conclusion still inherited a scope nobody stated. `ls hooks/` answers *"what is in hooks?"*; it never answers *"does this hook have a test?"* Caught only because the reviewer read the repo rather than trusting the brief's premise — the third time in that run that the *verify-don't-accept* instruction paid for itself. Rule for Phase 1: phrase the query as the question you actually have (`grep -rl worktree-activate` across the repo, not `ls` of one directory), and treat any absence claim in a brief as needing a tree-wide search behind it. | worktree-semantic-search session log F-5; fix `d65f96d`. Kin R-3 (scout limited grep to one file/crate), R-45 (relocating a file needs discovery-by-scan, which caller enumeration cannot substitute for), R-26 (a grep line-match locates a symbol, it does not confirm a mechanism) |
| R-76 | 2026-08-14 | miss (recurrence of R-73) → rule | **Extracting a decision into a testable unit CREATES a new seam — the dispatch to it — and the seam inherits none of the tests. R-73 said mutate the call site; this is R-73 one layer up, at a trait override, in a session whose own tracker already carried R-73.** The final review of the worktree-delta-search branch found `merge_hits` sorting Qdrant RRF scores as if comparable across two queries — measured on 576k live points, RRF returns `1/(1+rank)`, so a 3-chunk delta scored identically to a 500k-chunk index and the delta took a fixed **3/12** of every page. The fix moved the union into `CodeVectorStore::query_overlay`, overridden by Qdrant. `build_query_filter` then had 4 pure unit tests **plus** a live-Qdrant test proving real effect. The **override that routes Qdrant to it had none**: its only caller is `RetrievalClient::search_in`, no test builds a Qdrant-backed client, and the live test called `wrap.hybrid_query(...)` directly one layer beneath. Deleting the override silently reverted to the two-query default — C1 back — with **3718 tests and all 4 ignored qdrant tests green**, and the fix-wave report described the seam as *"pinned only by `#[ignore]`d live tests"* when it was pinned by nothing. Same session, same shape one layer lower: four Task-7 mutations survived 3688 tests (`file_name()`→`to_string_lossy()` on the delta key; swapping the two `SearchOpts` assignments; deleting the `worktree_state_warning` assignment; swapping the drift-note arguments) — every pure decision function was tested and every assignment wiring them together was not. The aggravating fact is that R-73 already prescribed *"require the call-site mutation explicitly in every implementer brief"* and the fix-wave brief did not, so this is failure to apply a written rule rather than absence of one. Remedy cost one changed call — point the existing live test at the trait method instead of the inherent one — and the confirming mutation produced the run's cleanest evidence: with the override deleted, the "union query" row came back **byte-identical** to the two-query row (delta 3/12, noise 2) while the suite stayed green. Rule for Phase 1 and for every brief: after any extraction, ask *what now chooses this, and is that choice tested?* — and require a mutation that deletes the dispatch, not just one that breaks the callee. Two verification techniques from the same reviews, both cheap and both reusable: **empirical non-vacuity** (copy the tree to scratch, revert only the fixed block, run, and report the pass/fail split — 3/7 and 6/3 were confirmed this way rather than reasoned), and **corroborating a claim about now-unobservable state by arithmetic** (a report's pre-edit grep of 21 matches in 5 files was confirmed post-edit as 43 − 19 new-file − 3 added = 21; as the reviewer put it, *"that arithmetic is not something a fabricated report lands on by accident"*). | worktree-semantic-search SDD ledger, final whole-branch review + fix-wave re-review; `src/retrieval/code_store.rs` `query_overlay`, `src/retrieval/qdrant.rs` `build_query_filter`, fix `c284786c`. Kin R-73 (mutate the call site, not the function), R-70 (a test that cannot run is indistinguishable from one that does not exist), R-49 |
| R-78 | 2026-08-14 | miss ×3 in one sweep (recurrence of R-59) → rule | **A bug file's `## Symptom` is observed; its `## Root cause` is usually inferred and then written in the same declarative voice. Nothing marks the seam, so the inference reads as the finding — and a wrong one does not merely waste work, it can redirect you away from a worse defect.** Verify-opening eight backlog bugs, three had premises that falsified on contact. `db5fe933` (state-protocol `.sh`) actively warned the fixer OFF five rows, calling buddy's hook "a different, legitimately-`.sh` component"; `ls buddy/hooks/` returns `hook_dispatch.py, hooks.json, judge.env, run.mjs` — no `session-start.sh` exists anywhere, and `hooks.json:4` dispatches `node run.mjs session-start`. Following it would have fixed **4 of 9** rows and closed the bug. `b86d81f1` (IL3 quoted pipe) asserted "the caller already does the quote-aware thing via `pipeline_segments`"; that function scanned raw bytes with **no quote state at all** — the same defect in two functions, one recorded as the other's solution. Following it would have fixed the false positive and left a **measured bypass** in place: a quoted `;` before a real pipe hid it from the enforcer, an unbounded-producer-to-trimmer `git log ... | head -3` returning `exit_code: 0`. `a99388a2` (edit_file ack handle) claimed the `@ack_*` mechanism was broken; it resolves in both call shapes, `git merge-base --is-ancestor 151cc9df 8f724171` is **true** (the fix was already in the SHA the bug cites) and `git log --since` over that area is empty, so the ENOENT was the target file. The aggravating detail: **two of the three carried their own refutation.** `a99388a2` recorded "the handle IS reaching parameter validation" under Symptom and argued past it; `db5fe933` left the companion-side hypothesis `deferred` while stating the buddy-side equivalent as settled. R-59 already says a bug file's Root cause is an unverified reading; what this adds is that the *falsifying command is usually one line and already implied by the file* — `ls`, read one function, one `merge-base`. **Rule for Phase 1:** before implementing a bug's Resume, extract the single command that would falsify its Root cause and run that first; and read the file's own Symptom/Hypotheses for a sentence that already contradicts it. | bug-fix session log F-39; fixes `2a36bfbf`, `6bb88a22`, `ffaf67a3`. Kin R-59 (the seam this recurs from), R-26 (a grep line-match locates a symbol, it does not confirm a mechanism), R-19 (do not assert a checkable fact unread) |
| R-82 | 2026-08-15 | miss ×2 in one day → hard rule | **A tool's error message is a hypothesis about itself, not a reading of its own code — verify before recording it or acting on it.** Twice in one day I believed one and was wrong both times. (a) `edit_file`'s *"edit contains a symbol definition"* led me to write "naive substring match" into F-44; the guard is actually diff-aware, multi-line-gated and comment-aware, and the real defect was one layer in (an unanchored per-line match). (b) `edit_code`'s *"AST parse failed — the file likely has syntax errors"* was emitted on a syntactically perfect file; the true cause is that tree-sitter's extractor does not surface methods of an impl nested in a function body, which the LSP resolves fine. In both cases the message named a **mechanism**, and in both cases the mechanism it named was not the one operating. The asymmetry is what makes this expensive: an error is written once by an author reasoning about the general case, then read as evidence by every session that hits it — so a wrong one is a false premise with unlimited distribution. Worse, F-46 (same day) shows a test can *pin* the wrong message, so correcting it surfaces as a failing gate. **Rule:** when an error message states a cause and you are about to record it, cite it, or act on it, open the emitting code first — it is one `grep` on the message string. Treat the message as the *symptom*, never the diagnosis. Corollary: when writing an error yourself, prefer a checked cause over an asserted one — `has_syntax_errors` already existed next to the hint that was guessing about syntax errors. | bug-fix work stream; F-44 (corrected), F-46, W-38; `138de7c5`, `cafa4b37`. Kin R-80 (a bug file's fix rationale is unverified), R-59/R-78 (its root cause is unverified) — same failure with a different author: there the bug file, here the tool itself |
| R-86 | 2026-08-15 | miss (a shipped, archived, "verified" fix) → hard rule | **A component's transport or deployment mode is an instrument, and a test that constructs the simplest one verifies the simplest one.** `eedb308c` claimed to re-sync a file changed on disk. It shipped **inert on the mux path**, which is the path this project uses for Rust, and I archived the bug as fixed. Mechanism: `did_open` records the on-disk signature, and on SOCKET transport it never takes its already-open early return (the mux owns document dedup), so every `document_symbols` re-recorded the current state before the drift check ran — comparing the file against itself — while the mux deduped the `didOpen` so the server kept its stale copy. Two halves silently cancelling. The end-to-end test passed the whole time because it drives `LspClient::start` with `mux: false`: **it exercised the one transport on which the defect cannot appear.** This is the R-81 family with a new instrument — there the build's *feature set*, here the *transport* — and both are invisible in the code you are reading and in the test that passes. **Rule:** before calling a fix verified, name every transport / deployment mode the component has and ask which one the test constructed and which one production runs. If they differ, the test is a smoke test. Prefer asserting a **mode-independent invariant** over parameterising: extracting `SyncedSignatures` made "a notification that may be deduped must not claim delivery" testable without standing up a mux, which is why it had no test before. Corollary: a fix whose correctness depends on **call ORDER** between two functions is one refactor from silent regression — encode the rule in the data (`record(overwrite: bool)`) so ordering stops being load-bearing. | bug-fix work stream; F-49, W-41; `eedb308c` (inert) → `7c8863f0` (real). Kin R-81 (build configuration as instrument), R-77/R-79 (a bounded probe's empty result), W-41 (live-verify after rebuild) |
| R-83 | 2026-08-15 | miss (a week stale) → rule | **"Reproduces only on platform X" is a claim about your tooling, not about the bug — check `rustup target list --installed` before believing it.** Two bugs sat scoped to a Windows host for a week (`910c9920` clippy drift, and the framing around `9fa78b81`). `x86_64-pc-windows-gnu` was an installed *target* the whole time, and `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` reproduced both on Linux in 40 s. **Cross-target lint/typecheck needs no linker, no emulator, and no second machine** — it is the platform's `cfg` evaluated locally. It also corrected the count: only **2 of the 3** filed lints still fire; the `thread_local` one was a clippy-version artifact on code that already carried `const { }`. Generalises past Rust: any toolchain with a cross target (`GOOS`, `--target`, a cfg flag) can evaluate another platform's conditional code where you sit. **Rule:** before recording a bug as host-scoped, spend one command asking whether the *checker* can be pointed at that host's configuration. The fix that follows is usually also mis-scoped — here the `needless_return` was never about the keyword, it was that `#[cfg(unix)]` erased every assertion around it and left a test that ran, asserted nothing, and reported `ok`. | bug-fix work stream; W-39; `2f76136e`. Kin R-81 (the build configuration is an instrument), R-77 family (a search that does not cover the space) |
| R-84 | 2026-08-15 | miss → hard rule | **A count and a category are two claims. Verifying the category on a sample does not verify it across the count — and the summary is what decides whether anyone reads the rows.** A bug file stated *"the 8 high findings are historical release sections"*, having checked that label against the `src/embed/*` subset, where it was true. Re-measured on HEAD: **10** highs, in **three** classes. Seven were historical. One was `src/foo.rs` — an illustration of a generated header format, not a reference. And **one was live drift the gate caught the moment it could see the file**: `CHANGELOG.md` cited a `docs/issues/…` path after that bug file had been archived to `docs/issues/archive/`, with the *very next bullet* citing an archived file correctly. The recommended fix — forgive released sections wholesale — would have forgiven the real bug along with the rest. **Rule:** when a filing pairs a number with a label ("the N findings are all X"), treat them as separate claims and re-derive both; if the label came from a sample, say which sample. Sub-rule for reconciliation work: classify every row before designing the sweep, because a sweep designed from the majority class silently absorbs the minority. | bug-fix work stream; W-39; `130c93a5`. Kin R-80 / R-78 / R-59 (a bug file's own reasoning is unverified), R-54 (before counting rows, ask what one row IS) |
| R-85 | 2026-08-15 | hit ×2, opposite verdicts → hard rule | **When a test blocks your change, ask what it is a statement ABOUT: a defect, or a contract. The two are indistinguishable from the failure message and demand opposite responses.** Both happened the same day, on the same kind of surface. (a) `edit_file_warns_hint_suggests_remove_when_new_empty` failed after I exempted pure deletions from `guard_structural_rewrite`. It pins a **contract** — and `infer_edit_hint` carries machinery built specifically to route the deletion case to `edit_code(remove)`. The block was designed, not overlooked; **my code was wrong and I reverted it** (`80350afe` → `9b902e0a`). (b) `did_change_refreshes_stale_symbol_positions` failed after `document_symbols` learned to re-sync a file changed on disk. Its step 3 asserted that a query after an external write returns *stale* positions — a **demonstration of the defect**, written to justify making callers flush by hand. That premise was the bug; the test became obsolete the instant it closed, and rewriting it was right (`eedb308c`). **Rule:** before yielding to or overriding a failing test, read what it was written to establish. Purpose-built support nearby (a bespoke hint, a dedicated error branch) is evidence of *contract*; prose like "should still return the old…" or "reproduce the … bug" is evidence of *defect*. Getting it backwards costs an implement-then-revert cycle in one direction and a worked-around fix in the other. | bug-fix work stream; F-47, W-39; `9b902e0a`, `eedb308c`. Kin R-82 (an error message is a hypothesis), F-46 (a test can pin a false diagnosis) |
| R-81 | 2026-08-14 | miss (fifth in the R-77 family) → new instrument class | **A build configuration is an instrument, and a zero from the wrong one is evidence about the build.** Live-verifying the new `migrate-memories --in-place` against this project, `cargo run --bin codescout` reported `read: 0` — one sentence from "this project has no semantic memories, nothing to repair", which would have closed a bug on a false premise and retracted a true earlier statement. Cause: `cargo run` builds **default features**, and `server-stack` is not one (`default = ["remote-embed", "http", "librarian"]`), so `VectorBackend::resolve()` took its `cfg(not(server-stack))` arm and listed an empty sqlite-vec *lite* store — while the live server, built by `cargo rb` (`--features server-stack,local-embed`), talks to Qdrant. Two stores, one command, no error either way. Refuted by `memory(action="recall")` returning three memories including `architecture`; the `cargo rb` binary then reported `read: 15, anchors_attached: 37`. R-77/R-79 named the class for *search scope*; this is the same error where the instrument is the **compiled feature set**, which is invisible in the command you typed and in its output. **Rule:** before believing any zero/empty from a locally-built binary that touches live state, confirm the binary was built with the same features as the running service — for codescout that means `cargo rb` and `./target/release/codescout`, never `cargo run`. And corroborate on a second surface: the falsifying `recall` was one call. | bug-fix work stream; F-43, and the archived memory query-prefix bug's § Resume. Kin R-77 / R-79 (a bounded search's empty result), R-50 (the view is not the set) |
| R-80 | 2026-08-14 | hit (extends R-59/R-78 one level up) | **Verify the bug file's argument for NOT fixing, not only its root cause.** Two of the three bug files worked this round carried a section arguing the obvious fix was wrong — `c7ba92f5` had *"Why the obvious fix is wrong"*, concluding both symptoms were "status-string-only" and that widening the shared classifier was "strictly riskier than a status-string residual". One `references(symbol="backend_is_local")` call refuted it: **four** consumers, three of them live query behaviour, and `build_embedder` reads the predicate **three lines above** the construction the file said was unaffected. The live path was already wrong, so widening was the fix rather than the risk — the risk being weighed was the risk of *not* widening (F-41). R-59 and R-78 cover an unverified `## Root cause`; this is the same failure applied to the **fix rationale**, and it is strictly worse: a wrong root cause yields a wrong fix that a test can catch, while a wrong rationale yields *no fix*, wearing a persuasive justification that makes every later verify-open pass skip the file as settled. **Rule:** a `## Root cause` section that argues against fixing is a claim about the blast radius of a change; check it the way you would check any other claim — enumerate the symbol's real consumers with `references`, and read the function that consumes it, not only the function that reports it. The falsifying call is one line and the file itself names the symbol. | bug-fix work stream; F-41 + `5b7536f5`. Kin R-59 / R-78 (a bug file's root cause is unverified reading), R-3 (grep the workspace, not the file in front of you) |
| R-79 | 2026-08-14 | miss (fourth recurrence of R-77) → hard rule | **A bounded search returning nothing is evidence about the search, not about the world — and it must never authorise a destructive action.** Reclaiming 25 GB of leaked test databases, nine survivors needed attributing to real projects. A check looping each name over three parent directories reported **`backend-kotlin` and `MRV-poc` as orphaned**. Widening to `find /home/marius/work -maxdepth 4 -type d -name <p>` found both immediately: `/home/marius/work/mirela/backend-kotlin` and `/home/marius/work/stefanini/southpole/MRV-poc`. Acting on the shallow result would have deleted **118 MB of live index for the project task #46 tracks 13 pending `worktree_scoped_row` merges against**. R-77 named this class the same day at brief-writing cost; this is the same error where the output is irreversible, which is what earns a separate row. The two stores actually removed each satisfied **both** conditions — no directory at depth 4 **and** older than 30 days — and one (`code-explorer.db`) had had its absence independently measured earlier in the same session when task #45 removed its registry root. **Rule:** a delete may be authorised by a *positive* finding (this path is gone, measured) but never by a *negative* search result alone; require two independent signals, and prefer one established for a different reason. | bug-fix work stream, `2faaf32a` cleanup (25 GB → 285 MB, `25d5a84d`). Kin R-77 (the class), R-50 (the view is not the set), R-3 (a grep scoped to one file misses cross-boundary sites) |
| R-74 | 2026-08-11 | miss ×6 → proposal | **The controller's own plan text is a claim, not an instruction — write briefs that demand a dead mutant rather than a green run, and implementers will correct the plan instead of transcribing it.** Six defects in one plan, every one caught by an implementer or reviewer following the evidence requirement rather than the instruction: (1) Global Constraints mandated `EnvGuard` + `#[serial]` in three places — banned crate-wide by `docs/conventions/test-env-isolation.md`, and written from memory of the convention rather than from it; (2) a guard comparison, `model_dim.unwrap_or(index_dim)` vs `index_dim`, whose operands default to each OTHER so it can never fail in the common case; (3) a comment asserting `strip_prefix("local:")` would swallow `"local-dir:"` — false, byte 5 is `-`, disproved by mutation; (4) a supplied "fix" asserting a library registry against a FRESH LITERAL in the test, so it guarded upstream drift and not our shipped value — the implementer implemented it, mutation-tested it, found it inert, and replaced it with shared constants; (5) a single-fixture mutation test that could not discriminate two mutants, because a guard clause short-circuits before the branch the second one needs; (6) `#[test]` + `futures::executor::block_on` against a fn that calls `spawn_blocking`, which panics on every call including the failure path the test asserted on. The through-line: each was plausible prose that only reading or running the substrate could refute. What made the corrections happen was the brief's standing line — *a test you have not seen fail is not evidence*, and *when an instruction of mine contradicts what you measured, follow the measurement and say so*. Proposal: put both sentences in every implementer brief, and record controller errors in the ledger by number so the rate is visible rather than smoothed away. | local-onnx-embedding session log F-2, F-3, W-4; plan corrections `893ade0e` + the `docs(plan)` commits on `feat/local-onnx-query-path`. Kin R-62 (a `## Fix` section is a plan and gets scouted like one), R-71 (an error message's hint is a claim about an API) |
| R-61 | 2026-08-07 | hit → rule | **Before filing a bug against configuration that looks wrong, grep for a test that asserts it.** A live `pgrep` of three kotlin-lsp mux instances showed the worktree instance carrying `GRADLE_USER_HOME=/tmp/codescout-mux-gradle-26a9e85d58931839` — its **parent repo's** hash — while its `-Duser.home=` and `--system-path=` both used its own `509dba098b20943c`. That is the exact shape of a cross-instance leak, and it is the family a previously-fixed bug lived in (per-instance system-path, `dc44ac3d`), so the prior felt high. It is asserted behaviour: `kotlin_worktree_shares_gradle_home_but_not_system_path` in `src/lsp/servers/mod.rs` requires *“worktree and its main repo must share GRADLE_USER_HOME”* and *“must NOT share the IntelliJ system dir”* — deliberate, because Gradle caches are huge and worth sharing across worktrees while the IntelliJ index is not. The asymmetry that reads as a bug **is** the design. Recorded as an explicit *non-finding* in the kotlin bug so the next reader does not re-investigate it. Rule for Phase 1: when a config value surprises you, search the suite for its invariant before writing the bug — a deliberate asymmetry and a leak look identical from the process table. | kotlin-lsp uncapped-heap bug, 2026-08-07 update; pids 173473 / 3148316 / 3478380. Kin R-19 (assert nothing checkable you have not read this session) |
| R-60 | 2026-08-07 | hit → proposal | **When you implement a surface that already exists elsewhere in the codebase, read the existing one's CODE COMMENTS, not just its shape — they name the bugs already paid for, and you are about to stand where that author stood.** Adding a `completeness_warning` to `symbols` search meant copying the convention from `references.rs`. Its rendering code carries a comment: *“BUG 2026-05-21: surface the completeness warning … in both the zero-refs and the normal branch.”* That is not commentary on `references.rs` — it describes the trap waiting in `symbols`: `format_search_symbols` returns `"0 matches"` from an early return **before** any warning or overflow rendering, so a warning appended at the end of the function would be invisible in the one case it exists for (an agent deciding whether a zero means *absent* or *not searched*). The convention's shape (`result["completeness_warning"]`) transfers on its own; the *lesson* transfers only if you read the comment. Proposal for Phase 1: when reusing an in-repo convention, grep its implementation for `BUG`/`bug` comments — the cheapest available list of what that surface has already cost someone. | release-promotion W-10 / F-10 session; tests `format_search_symbols_surfaces_completeness_warning_on_zero_matches` + `format_search_symbols_leaves_a_trustworthy_zero_bare`; the mutation reverting the early-return warning kills the first and nothing else. Kin R-57 (read what it does, not what it intends) |
| R-59 | 2026-08-06 | hit → proposal | **Before building the harness a bug file asks for, read the FALLBACK GATE — the condition that decides whether the suspected path is even reachable. It can eliminate the hypothesis the harness was designed to measure.** The `symbols` search flake was filed "root cause unconfirmed; needs a controlled, scripted reproduction (N parallel `symbols(name=X)` calls immediately post-activation)", and every signal pointed at LSP warming — including the source's own comment naming *"silent workspace/symbol on a still-indexing server"*, plus three genuinely silent degradation paths (budget-exceeded → `Ok(Vec::new())` with only a `tracing::warn!`; task error and join error both `continue`d). Reading one line killed it: the tree-sitter fallback is gated on `matches.is_empty()` — the aggregate of *already-filtered* matches — so it runs whenever the final answer would be zero, regardless of which languages degraded, and non-matching LSP results never populate `matches` and so cannot suppress it. Therefore **a 0-match implies tree-sitter also found nothing, and tree-sitter never touches the LSP**: server readiness cannot produce the symptom. The surviving hypothesis is a different bug class — both paths key off one `root` from `require_project_root_for` (`src/tools/symbol/symbols.rs:225`), and a root not yet settled post-activation makes the LSP query one tree while the walker walks the same wrong tree, both correctly returning nothing. Kin to the archived shared-server active-project race. Proposal for Phase 1: when a bug names a suspected mechanism, locate the fallback/guard that would mask it and read that condition **before** costing a reproduction — an unreachable hypothesis is cheaper to eliminate than to measure. | release-promotion W-8; the harness as specified would have instrumented LSP readiness, i.e. the wrong variable; correct instrument is the resolved `root` per 0-match. Kin R-57, R-50 (the view is not the set) |
| R-54 | 2026-08-05 | miss → rule | **Before counting rows, ask what one row IS and whether two rows can contain the same underlying event — and never let "zero observed" become "empty" without stating n.** A corpus of 63,574 rows was 444 sessions: each row was one request re-sending the whole conversation (143 per session, 165× double-count). A 64-row sample was 34 effective sessions; a top band of 13 rows was 6. Nesting is invisible downstream because the duplicated content is genuinely present in both rows. Sibling of R-53: that one guards what the corpus is MADE OF, this one guards what a ROW MEANS | provenance-probe F-14 + W-10; PV-65; PV-66 |
| R-53 | 2026-08-04 | miss → rule | **A corpus's composition is a seam — census it by producing tool before you measure it, and when you stratify by a magnitude, ask what makes things big.** One tool contributed 59.8% of all tool-result bytes as base64 image data that `json.dumps` had stringified into the text denominator; it also defined the top band of a size-stratified sample (11/13 sessions), so the magnitude axis was a producer axis in disguise. Invisible to thirteen rounds of internal checks — contamination moved numerator and denominator together. Caught only when the corpus owner said the traffic was not representative | provenance-probe F-13 + W-9; PV-61; PV-62 |
| R-52 | 2026-08-04 | miss → rule | **An artifact's ownership is the union of its inputs' ownership — choose the output location from the input set, never from where the code lives.** Sibling to R-51, not an amendment: R-51's failure produces wrong NUMBERS, this one produces MISPLACED DATA, and R-51's exclusion-at-entry fix leaves the artifact in the wrong place. A pipeline reading N corpora writes outside all N. Caught at staging: a probe in `codescout/scratch/` held ~14MB of symbol vocabularies from 8 repos including client work, and `semantic_search` was returning them — the retrieval consequence R-51's framing does not reach | provenance-probe F-2; PV-60; staging check 2026-08-04 |
| R-51 | 2026-08-03 (escalated 08-04) | miss → rule, promote-ready | **An instrument that writes into the corpus it measures.** New seam class: output-path ↔ measurement-domain overlap. A probe wrote its own artifacts (`sessions.json`, `vocab/*.json`) into the repo whose symbol vocabulary it was building; DF inflated 6.2× (35,496 → 221,214) with no error raised, caught only by an incidental before/after ratio check. Not scoutable statically — the overlap is created by *running* the pipeline. Complement of R-50: R-50 = name what the view dropped, R-51 = name what the view wrongly admitted | provenance-probe F-2 (fixed-verified); F-3 same log is the R-50-shaped twin |
| R-50 | 2026-07-28 | miss → rule | **The view is not the set.** Five distinct errors in one session share one shape: concluding from an enumeration that had silently excluded something. A liveness filter, then reusing the filtered list; `pgrep \| tail -1` picking an orphan not the live server; a SHA census matching frontmatter `id:` as commits; `ls \| wc -l` counting `.anchors.toml` sidecars as memories; a compact summary read as a whole result. Before concluding from a list, name what the view dropped — filter, cap, preview, or a pattern that matched the wrong tokens | bug-fix F-38 + W-30; also F-33/F-35 and the four corrections listed in-entry; kin R-10 (completeness scout), R-26/R-27 |
| R-49 | 2026-07-28 | hit | Re-scout your OWN bug file / plan before implementing it — an artifact written during the observation is a hypothesis that acquires the authority of a record the moment it is committed. When a root cause cites two functions, read the layer BETWEEN them: a chain's middle is where the guards live, and a mechanism inferred from its two ends will never mention them. Three failures of session-authored artifacts in one session (inverted fix option, overstated doc guarantee, unsupported root cause) | bug-fix F-37 + W-29, F-34/W-26, F-35; issues/2026-07-28-edit-code-target-base-from-stale-lsp-range (mitigated); kin R-32/R-46 |
| R-48 | 2026-07-28 | hit | A fix built from a bug report's reproduction inherits that reproduction's blind spots, and a test suite written against the same fixture inherits them too — seven tests all reused the report's `"\` shape and none could see that a plain `"` + raw newline (the commoner Rust form) was unprotected. Found by writing the end-to-end fixture the *natural* way and tracing the fix over it before running. For a syntax-level fix, enumerate the language's other syntaxes for the same construct before closing | bug-fix F-36 + W-28; issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents; kin R-26/R-27 (read ≠ verified), R-46 |
| R-47 | 2026-07-28 | hit | Enumerate the callers of every function in the chain a fix touches, not only the symbol the report names — the report walked one call path. `reindent_to`'s 3 callers were all `edit_code`, but its delegate `reindent_block` had a second production caller (`edit_file/mod.rs:747`) with the same defect, reached without passing through `reindent_to`. Relocating the fix one level down doubled the surface closed at the same size of change | bug-fix W-27; issues/2026-07-28-edit-code-reindent-shifts-string-literal-contents (archived, `79cd1428`); kin R-17/R-31/R-44 |
| R-46 | 2026-07-28 | **archived** → R-63 | Choosing between candidate fixes needs the target's TEST module read, not just the target: the implementation shows what it does, only the tests show what may not change (a bug file's "cheapest, cannot break X" option was exactly inverted). Corpus census, not code, caught the companion H1 trap | bug-fix F-34 + W-26; kin R-19/R-44 |
| R-45 | 2026-07-28 | hit | Relocating a file needs a discovery-by-scan grep; caller enumeration is blind to it (generalises R-44: `cfg` on a value → callers are the consumer set; `cfg` on a location → callers are half of it) | bug-fix F-33 + W-25 |
| R-44 | 2026-07-25 | hit | The write-side twin of R-43: before accepting a proposed `#[cfg]` gate on a `pub mod` declaration, enumerate the CONSUMER set (`grep <mod>::|use .*<mod>` at workspace root, `context_lines=2`) and check whether the config being gated OUT is the one the plan's own tests or invariants live in. Gating a module is a subtree delete; the declaration site cannot show its blast radius. | dependency-review session F-3 + W-2; `src/retrieval/mod.rs:3` + 14 ungated `RetrievalClient` consumers; kin R-43/R-5/R-17 |
| R-43 | 2026-07-25 | **archived** → R-26 + R-50 | An attribute-binding claim (`#[cfg]`, `#[serde(...)]`, `#[tokio::test]`) can never be made from a grep hit: grep prints matching lines with line numbers but ELIDES the gap between them, and an attribute binds to the *next item* — usually not itself a match. Read the region before asserting the binding. | dependency-review session; `src/retrieval/mod.rs:1-17` (cfg at L1/L12 bound to `artifact`/`qdrant`, not `embedder` L7 / `reranker` L14); caught by `cargo check` (16× E0433); kin R-19/R-5/R-3 |


## R-1 — Pre-dispatch grep for asserts on `include_str!`'d constants

**Verdict:** hit

**Observed:** 2026-05-19, MCP prompt channel redesign work stream
(`docs/trackers/archive/mcp-prompt-redesign-session-log.md` F-1, W-1).

**Pattern:** Before rewriting a content file (`source.md`, embedded
templates, etc.) that backs a static constant via `include_str!`,
grep the codebase for asserts on that constant. Specifically:

```
<CONST>.contains(...)
<CONST>.find(...)
<CONST>.matches(...)
snapshot calls naming the surface file
```

Enumerate every test that will fail post-rewrite and name them in
the implementer's dispatch prompt.

**Evidence:** Without R-1, U4 implementer would have run the 4
planned `redesign_invariants` tests, hit 6 unplanned
`SERVER_INSTRUCTIONS`-asserting failures, and either reported
DONE_WITH_CONCERNS or BLOCKED. Estimated cost saved: 6-12 subagent
round-trips.

**Counterfactual confirmed by:** F-1 enumeration in
`mcp-prompt-redesign-session-log.md`, evidenced by ≥4 tests deleted
during U4 that were NOT in the plan's "1 test may break" prediction.

**Promote-when:** R-1 already validated once. Promote to SKILL.md
after a second `include_str!` rewrite work stream confirms the
pattern. Concrete addition: `SKILL.md § Phase 1 — Scout`, sub-bullet
"For `include_str!`'d content files, grep `<CONST>.contains / .find /
snapshot` to enumerate asserting tests."

**Status:** promoted to SKILL.md (claude-plugins:f842848, 2026-05-28). Added as a 5th bullet under Phase 1 — Scout, citing R-1 + R-7 by name with the loophole-closing cross-reference from the "When NOT to Use" rewrite (same commit). Promote-when criterion fired with 2/2 datapoints — R-1 (mcp-prompt-redesign work stream, 2026-05-19) and R-7 (this session's prompt-guide-refactor F-4 + W-3, 2026-05-28).

---

## R-3 — Scout limited grep to one file/crate; cross-file asserts slipped

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-2, second half).

**Pattern that failed:** The scout grepped `src/prompts/` for
asserts on the rewritten content. A 7th broken test
(`server_instructions_documents_goal_tracker_discovery`) lived in
`src/server.rs` — outside the scout's grep scope. Recon missed it.

**Pattern proposal:** Phase 1 grep must default to the **workspace
root**, not the directory of the file being changed. Constants and
their callers cross crate / module boundaries; assertion sites do too.

**Cost absorbed:** 1 extra deletion in the U4 fix-up.

**Promote-when:** R-3 already validated as a needed default. Cheap
fix: add a sentence to `SKILL.md § Phase 1 — Scout` — "Grep scope
defaults to workspace root, not the file being modified."

**Status:** promoted to SKILL.md (claude-plugins:787cdec0, 2026-05-23). Added as a 4th bullet under Phase 1 — Scout, citing this R-3 row by name. Promote-when criterion fired with 1/1 datapoint, per the tracker's note ("already validated as a needed default").

---

## R-4 — Grep undercounts struct-field construction sites by 2-3×

**Verdict:** miss

**Observed:** 2026-05-19, same work stream
(`mcp-prompt-redesign-session-log.md` F-3, W-2).

**Pattern that failed:** For "add a required field to widely-used
struct", scout grepped `ToolContext\s*\{|ToolContext::new` and
counted 13 sites. Reality required ~30 (one test file alone had 24
construction sites — many on single lines the regex matched once
per file rather than per occurrence; many nested inside macros and
helper factories).

**Cost absorbed:** Implementer fell back to a `perl -i -0pe` bulk
pass driven by `cargo build` errors. Two files double-inserted;
deduped manually. Net result correct but the controller-side scout
gave a wrong estimate of blast radius.

**Pattern proposal (covered by R-5):** For exhaustive enumeration
of construction sites of a struct that gains a non-`Option` field,
use `cargo build` as the scout. The compiler reports every missing
field; grep only approximates.

**Promote-when:** validated once already. Pairs with R-5 for the
expansion.

---

## R-6 — Explicit recon invocation on substrate before mechanism design

**Verdict:** hit

**Observed:** 2026-05-28, prompt+get_guide refactor work stream
(`docs/trackers/prompt-guide-refactor-session-log.md` F-2 + W-2).

**Pattern:** Before locking the v1 design for a new runtime mechanism
(in-band hard-injection of get_guide content), invoked
`/codescout-companion:reconnaissance` to scout the actual substrate.
Read `ToolContext::guide_hints_emitted`, `CodeScoutServer::build_context`,
the workspace-reset trigger at `ActivateProject::call`, and existing tests
at `server.rs:2711-2840`. Discovered the ledger lives on `CodeScoutServer`
(per-MCP-session, shared via Arc across all per-request ToolContexts
including subagents) — NOT on `Agent` state as the brainstorm had assumed.

**Evidence:** Without the scout, task #3 in the brainstorm would have
shipped a parallel per-Agent ledger, conflicting with the existing one
(2 sources of truth) or superseding it (breaking 6 existing tests at
`src/server.rs:2711-2840`). The substrate finding ALSO vindicated Iron
Law 6 architecturally — subagents are structurally blind to topics the
parent triggered (W-2), so the "parent must brief" law isn't stylistic
but substrate-mandated.

**Counterfactual confirmed by:** F-2 and W-2 in
`docs/trackers/prompt-guide-refactor-session-log.md`. Recon-before-build
prevented at least 150 LOC of duplicate mechanism, AND surfaced the
architectural reality that anchors Iron Law 6.

**Promote-when:** R-6 is a single datapoint of "explicit invocation
produces win" — pair with R-1 type hits to argue for promoting "always
scout substrate state before locking a design that assumes specific
storage" to SKILL.md. Currently 1/2.

---

## R-8 — Miss: `edit_markdown(action='replace')` shape unverified on marker-bearing section

**Verdict:** miss → proposal

**Observed:** 2026-05-28, same work stream
(`docs/trackers/prompt-guide-refactor-session-log.md` F-7).

**Pattern that failed:** Used `edit_markdown(action='replace',
heading='## Deeper guidance', ...)` on `src/prompts/source.md` without
first scouting the section's body. The body contained inline
`<!-- @end -->` and `<!-- @surface onboarding_prompt -->` HTML-comment
markers that demarcate prompt surfaces; replace wiped them, breaking
the build (`surface 'onboarding_prompt' not found`). Hit a second time
on the next edit attempt — lost the intro paragraph that lived between
the `<!-- @surface onboarding_prompt -->` opener and the next heading.
Both losses were caught only by the build's downstream gates
(extract_surface panic, snapshot test regen detecting the gap on diff).

**Pattern proposal (new vocabulary for SKILL.md § Phase 1 Scout):**
*"When using `edit_markdown(action='replace')`, FIRST read the
section's body with `read_markdown(heading=...)`. Replace overwrites
the entire body verbatim. If the body contains structural HTML-comment
markers (`<!-- @surface NAME -->`, `<!-- @end -->`, project-specific
sentinels), the new content must explicitly include them or the
replace will drop them silently."*

The F-7 fix (commit 80f2fbca) adds a programmatic gate that catches
this at the editor level. R-8 is the human-discipline counterpart
that prevents the gate from ever needing to fire.

**Cost absorbed:** 2 edit attempts, 1 destructive working-tree recovery
incident (separately captured at `~/.buddy/memory/common/never-git-checkout-to-exclude-wip.md`),
1 commit amend. ~15 minutes of friction + 1 erosion of user trust.

**Promote-when:** R-8 + one more "replace dropped structural content"
incident (e.g. in a tracker template that has separator lines) → promote
to SKILL.md § Phase 1 Scout. Currently 1/2.

---

## R-9 — Proposal: session-state recon for subagent dispatch

**Verdict:** proposal

**Source:** F-6 in
`docs/trackers/prompt-guide-refactor-session-log.md` + the verification
subagent's self-assessment (W-4).

**Pattern that failed:** Dispatched a subagent with full Iron Law 6
briefing (file paths, symbol names, F-2/W-2 finding pointer). The
briefing was rated "self-discovery cost ≈ zero" (W-4) but the parent's
predictions about V2 auto-inject behavior were wrong — the subagent's
first `symbols()` call DID fire `progressive-disclosure` injection
that the parent claimed was already triggered. Cause: the parent
didn't communicate the **session-state ledger** — which topics had
actually been re-triggered in the post-`/mcp`-reconnect window.

**Proposal:** Add to SKILL.md § Phase 1 Scout, sub-bullet for
subagent-dispatch case:

> **For subagent dispatch, also scout session-level state** — what
> topics has the parent triggered, what workspace is active, what's
> already in the @ref buffer. The `guide_hints_emitted` ledger is
> per-MCP-session and shared between parent and subagent; the subagent
> can't see it from inside a tool call. Brief it explicitly:
> *"I've triggered: [librarian, progressive-disclosure]"* lets the
> subagent predict its own V2 auto-inject behavior accurately.

**Why this is a phase-1 tool, not a phase-4 fallback:** the scout's
job is to enumerate what the subagent will need. Session-state IS
context the subagent needs; without it, the subagent makes wrong
predictions (per W-4 Section E) and the parent's prediction becomes
falsifiable rather than substrate-derived (per F-6).

**Caveats:**
- The `guide_hints_emitted` ledger has no read-only query tool; the
  parent has to remember what it triggered. Future enhancement could
  expose `workspace(status, include=['guide_hints'])` or similar.
- Wall-clock session vs. post-reconnect window is a real distinction
  (per W-2's amendment). Parent should brief based on
  post-reconnect-window state, not full session history.

**Threshold to promote:** R-9 + one more datapoint where a subagent
mis-predicts V2/session-state behavior. Currently 1/2.

**Status:** drafted into SKILL.md preemptively (claude-plugins:f842848, 2026-05-28). Added as a 6th bullet under Phase 1 — Scout, naming subagent dispatch's session-state-scout requirement and the recommended brief shape (`"I've triggered: [librarian, progressive-disclosure]"`). Ships at 1/2 datapoints because the F-6 critique came verbatim from a verification subagent's own self-assessment (W-4 Section E), which is unusually high signal for a single datapoint — the future subagent who'd benefit from this guidance is exactly the agent that named the gap. Revised promote-when: if R-9's pattern catches a similar miss in a future session → mark `validated`; if no further misses surface within 3 multi-session work streams that involve subagent dispatch, the proactive ship was correct.

---

## R-10 — Miss: buffered artifact body parsed for structured extraction without a completeness scout

**Verdict:** miss → proposal

**Observed:** 2026-05-29, metadata-filtering work stream
(`docs/trackers/archive/metadata-filtering-session-log.md` F-4 + W-1).

**Pattern that failed:** Retrofitting `codescout-usage-frictions` to be
`entry_filter`-searchable required parsing the tracker's body into a structured
array. `artifact(get, full=true)` returned a 36 KB `@tool_*` buffer whose `body`
field was truncated at U-18 by the progressive-disclosure inline budget; the
parse silently produced 15 of 22 entries (U-19..U-25 dropped). No Phase-1 scout
verified that the parsed body was *complete* before it became the input to a
write (`artifact_augment`). The drift was caught only at post-augment
verification, by noticing the get response's `preview.headings` listed entries
beyond the parsed tail.

**Pattern proposal:** Add to Phase-1 Scout — *when a buffered tool output
(`@tool_*` / `@cmd_*`) is the input to a structured extraction or write, treat
its completeness as an unverified shape.* Reconcile the parsed item count against
an independent server-side view (`preview.headings` for artifacts, total/by_file
for search tools) before acting on it. Buffered bodies are silently clipped at
the inline budget; the truncation carries no in-band marker.

**Cost absorbed:** 1 incomplete catalog write (corrected before any consumer
queried it) + 1 re-read (line-range get) + 1 `merge=false` re-augment with a
widened schema enum. Recoverable, but had verification not cross-checked the
preview, a 7-of-22-entry index would have shipped with no error and no git diff.

**Promote-when:** A second instance of a buffered tool output truncating a
structured-extraction input. At 2 datapoints, fold into a Phase-1 Scout bullet
in SKILL.md ("buffered outputs are unverified shape for extraction/writes").

**Status:** proposal — single datapoint (F-4 + W-1, this session). Awaiting a
second occurrence before SKILL.md promotion.

---
## R-11 — Concept docs diverged from code on concurrency semantics

**Verdict:** hit → proposal

**Date:** 2026-05-30

**Scout:** Before running a multi-instance / multi-worktree concurrency experiment on
backend-kotlin, scouted `docs/manual/src/concepts/{cross-process-write-serialization,
kotlin-lsp-multiplexer}.md` against the actual code (`src/lsp/mux/mod.rs`,
`src/lsp/servers/mod.rs`). Two doc-vs-reality gaps surfaced *before* acting:

1. **"Isolated GRADLE_USER_HOME to prevent daemon contention between instances"**
   (`kotlin-lsp-multiplexer.md` § Gradle Isolation) reads as *per-instance* isolation.
   Code: `src/lsp/servers/mod.rs:63` hard-codes a single fixed
   `GRADLE_USER_HOME=/tmp/codescout-mux-gradle` shared by **every** kotlin JVM. The
   isolation is from the user's `~/.gradle`, **not** between worktrees/instances.
2. **Cross-worktree JVM multiplication is undocumented.** Neither doc states that the mux
   socket is keyed on workspace **path** (`src/lsp/mux/mod.rs:14,20`), so N worktrees of one
   repo spawn N JVMs against one shared, unguarded IntelliJ system-path. The mux docs imply
   "one JVM per project"; reality is "one per path."

**Counterfactual:** Without the scout, I'd have framed the experiment as "the mux dedups, so
worktrees are cheap" and mis-read the 6-JVM / shared-system-path result as a bug in my setup
rather than the designed (under-documented) behavior. The scout also corrected the user's
premise that subagents create *separate instances* (they share one server → a different
conflict regime entirely).

**Proposal:** When scouting any "isolation" / "per-X" claim in concept docs, grep the
constant the doc names (`GRADLE_USER_HOME`, `system-path`) and confirm the isolation key
matches the doc's stated granularity. Doc adjectives ("isolated", "per-instance") are
assertions to verify against the keying expression, not facts.

**Evidence (bug trackers):** `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`,
`docs/issues/archive/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`.

**Promote-when:** A second scout catches a doc "isolation/per-X" adjective contradicted by a
shared constant. At 2 datapoints, promote to the skill as a Phase-1 rule:
"verify isolation-claim adjectives against the keying expression."
## R-12 — Plan's proposed data structure cited the symptom layer, not the structural layer

**Verdict:** hit → proposal

**Date:** 2026-05-30

**Scout:** Before implementing `docs/plans/2026-05-30-per-request-workspace-pinning.md`, scouted the plan's named seams against current code: `AgentInner` (`src/agent/mod.rs:82`), `ActiveProject` (`:135`), the four resolution accessors, and `Agent::activate` (`:330`). The plan's Design proposed a flat `HashMap<PathBuf, Arc<RwLock<ActiveProject>>>` registry. Scouting `src/workspace.rs:316` revealed the racing slot is actually `AgentInner.workspace: Option<Workspace>`, and `Workspace` is **already** a multi-project registry (`projects: Vec<Project>`, each `Dormant`/`Activated(Box<ActiveProject>)`, + `focused: Option<String>`) carrying an existing per-request resolver `Workspace::resolve_root` (`:373`, "explicit id > file hint > focused"). The correct registry unit is `Workspace`, one abstraction layer above the plan's `ActiveProject`.

**Counterfactual:** Without the scout, Phase 1 builds a flat `ActiveProject` HashMap that collides with the existing `Workspace` abstraction; the collision surfaces only after the structure is wired, forcing a full Phase-1 rewrite plus a wasted call-site migration against the wrong structure. Caught pre-implementation (F-1 in `concurrency-fix-session-log.md`); the correction is a plan revision — net *less* code, since `resolve_root` already exists.

**Proposal:** When a plan's Design names a *data structure* to introduce, scout the existing abstraction at that layer before Phase 0 — grep for the field the plan would add (`projects:`, `workspace:`) and read the struct that owns the racing slot. A plan written from a bug file inherits the bug file's *symptom-layer* framing (here: "single global active project"); the *structural layer* (a `Workspace` nesting N projects) may already implement half the fix. Verify the plan's granularity against the owning struct, not the symptom description.

**Evidence (session-log):** concurrency-fix F-1 (this session).

**Promote-when:** A second pre-implementation scout catches a plan proposing a data structure that duplicates an existing abstraction one layer up. At 2 datapoints, promote to the skill as a Phase-1 rule: "scout the struct that owns the racing slot before trusting a plan's proposed data structure."
## R-13 — Cross-repo doc↔plugin drift is invisible to intra-repo `audit_doc_refs`

**Verdict:** hit → proposal

**Observed:** 2026-05-30, integration-design session (Hermes/OpenClaw + codescout). A workflow mapping agent flagged that codescout's `CLAUDE.md` described companion `pre-tool-guard.sh` behavior that no longer matched the plugin. Scouted the authoritative source before fixing.

**Scout (reality):** Read `../claude-plugins/codescout-companion/hooks/hooks.json` + `pre-tool-guard.sh` headers via `grep` (source-file shell blocked; used codescout `grep` tool). Doc was stale three ways: (1) named hook `semantic-tool-router.sh`, which does not exist (real file: `pre-tool-guard.sh`); (2) matcher documented as `Grep|Glob|Read`, actually `Grep|Glob|Read|Bash|Edit|Write`; (3) the "Cross-repo work (companion ≥ 1.11.1)" block described a `cd`-passthrough escape that was removed when the hook was hardened 2026-05-21 (now all `Bash` → `run_command`, sibling git via `git -C /abs/path`). Plus 9 registered hooks the doc never mentioned.

**Counterfactual (hit value):** An agent trusting the stale `CLAUDE.md` would run `cd ../sibling && git …` expecting passthrough → now hard-`deny`ed → failed Bash call + confusion; or chase a nonexistent `semantic-tool-router.sh`. Scouting `hooks.json` (authoritative source) instead of patching only the one line the user named caught 3× the drift. Fixed in `commit 7187396a`.

**Cross-cutting lesson / proposal:** This drift is structurally INVISIBLE to `librarian(audit_doc_refs)` — that lint scans only the active project's own `docs/**`, so a codescout doc stale about a *sibling repo's* code (the companion lives in `../claude-plugins/`) can never be flagged. Recon caught it only because an agent diffed the two repos. Proposal: when scoping the autonomous ops daemon (integration Pattern 3), point `audit_doc_refs` at BOTH repos, or add a cross-repo doc-vs-source audit mode. Until then, cross-repo plugin↔doc drift has no automated gate — it needs an explicit recon pass.

**Relation:** Same family as R-11 (docs diverged from code on concurrency) but the novel axis is CROSS-REPO + the audit blind spot. R-11/R-12 compared codescout docs vs codescout code; this is codescout docs vs `claude-plugins` code.

**Promote-when:** A second cross-repo doc-drift datapoint (codescout doc stale about `claude-plugins`, or vice-versa) → promote a Phase-1 Scout bullet: "For docs describing a *sibling repo's* code (plugin hooks, cross-repo configs), scout the authoritative source in that repo — `audit_doc_refs` cannot see across repo boundaries."

**Status:** open — single datapoint; proposal awaiting a second cross-repo drift datapoint before SKILL.md promotion.

**Source:** `commit 7187396a` (companion-docs fix, this session); `CLAUDE.md` § "## Companion Plugin: codescout-companion".
## R-14 — Scout specialist-memory-sourced design claims against current code

**Verdict:** hit (confirmed — no drift, but the scout was load-bearing)

**Observed:** 2026-06-01, mid-brainstorm of the peer-delegation protocol. The summoned Architecture Snow Lion cited the project memory `architecture-snow-lion/outputguard-cross-cutting-law.md` (dated 2026-05-07) to assert codescout's `@ref` buffers are process-local — the basis for a hard design finding ("`tool.call` breaks `OutputGuard` across the peer boundary; requester A cannot read peer B's `@tool_X`"). Scouted the current code before letting the design rest on it.

**Scout (reality):** `src/tools/output_buffer.rs:42` — `OutputBuffer { inner: Mutex<BufferInner> }`; `BufferInner` holds `entries: HashMap`, `order: Vec` (LRU), `max_entries` (`new(20)`). Thread-safe **in-memory LRU**, held as `Arc<OutputBuffer>` in the tool context; the `workspace-state` guide classifies buffers as session-resident ("NOT cleared … remain readable"). No disk / shared backing (the `source_path` on `@file_*` entries is the file they *point at*, not a shared registry). `call_content` (`src/tools/core/types.rs:485`) is the dispatch+buffering entry point (test: `call_content_buffers_large_output`).

**Outcome:** MATCH — claim confirmed. The cross-process boundary problem is real; the design's "re-buffer on the requester" resolution stands on read code, not a 3-week-old memory.

**Cross-cutting lesson:** A summoned specialist citing a *dated* memory as the basis for a load-bearing design decision is a seam — exactly like a plan citing a struct field. Memories carry an `updated:` date precisely because they are snapshots. The Snow Lion's Operating Principle 2 ("cite the import, not the diagram") extends to "not the memory either." Scout before the design depends on it; confirm-or-dissolve is itself the value. Had buffers moved to disk since 2026-05-07, the design would have invented a non-problem and bolted on an unnecessary proxy mechanism.

**Promote-when:** a second instance where a specialist/CLAUDE.md memory citation, once scouted, turns out *stale* (drift) → promote a Phase-1 Scout bullet: "Treat specialist/CLAUDE.md memory citations as snapshots; verify the cited symbol/contract against current code before a design or edit depends on it."

**Status:** open — single datapoint; confirmed-match this time.

**Source:** `src/tools/output_buffer.rs:42`, `src/tools/core/types.rs:485`; this session's peer-delegation brainstorm.
## R-15 — Scout external-tool on-disk state against bug-doc claims before a fix depends on addressing it

**Verdict:** hit (caught doc-vs-filesystem gap pre-implementation)

**Observed:** 2026-06-03, systematic-debug pass on the kotlin-lsp unbounded-disk bug (`docs/issues/archive/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`). About to evaluate fix candidate #2 — "on idle-timeout, remove *that workspace's* analyzer dir" — which presumes codescout can address the analyzer dir from its own `ws_hash`.

**Scout (reality):** Listed the live `--system-path` dirs vs `~/.config/JetBrains/analyzer/workspaces/*`. codescout's `ws_hash` (`src/socket_discovery.rs:10`, `DefaultHasher` → `{:016x}`) is **16 hex chars**; the analyzer dirs are **32 hex chars** (128-bit, IntelliJ path-hash). None of the 3 live system-path hashes (`c85ec91bdbfd1aee`, `26a9e85d58931839`, `7e868829c00fa9b2`) appear among the 8 analyzer dirs.

**Outcome:** GAP — the bug file's Evidence ("`<hash>` matches codescout's `workspace_hash` granularity") conflated *granularity* with *derivable key*. Fix #2 is not viable without replicating IntelliJ's hash (fragile, version-coupled). Re-ranked toward the env-redirect fix (codescout owns the base path) — captured as kotlin-lsp-disk F-1; corrected the bug file.

**Cross-cutting lesson:** Recon's "read the actual response shape, not docs" extends to the *filesystem state of external tools*, not just code symbols and API responses. A bug doc's claim about *where another process writes* and *how it keys those paths* is a seam — verify it against the live tree before a fix design rests on addressing those paths. Cheap (`ls`/`du`), and it dissolved a doomed fix direction before any code.

**Promote-when:** a second instance where a fix design assumed an external tool's on-disk path was addressable from our own key/hash and the live tree disproved it → promote a Phase-1 Scout bullet: "When a fix must locate files another process writes, list the live tree and confirm the key is one we control or can derive — not merely the same granularity."

**Status:** open — single datapoint; gap caught + bug doc corrected this session.

**Source:** `src/socket_discovery.rs:10`; `~/.config/JetBrains/analyzer/workspaces/` live listing; bug `docs/issues/archive/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`.

---
## R-16 — Pre-dispatch scout of the plan's OWN splice code caught a double-newline bug before dispatch

**Verdict:** hit (caught a correctness bug in plan-authored code at the seam, pre-dispatch)

**Observed:** 2026-06-04, subagent-driven execution of the edit_file whitespace-normalized fallback plan, about to dispatch the Task 4 (integration) implementer.

**Scout (reality):** Re-read the plan's own `match_count==0` apply code at the byte level. `find_normalized_windows` sets `end_byte` to EXCLUDE the matched block's trailing newline (so `content[end_byte..]` re-supplies it), but `reindent_block` re-emits a trailing newline when `new_string` ends in `\n` — so an `old_string`/`new_string` ending in `\n` would splice a DOUBLE newline (spurious blank line). The common no-trailing-newline case was correct, hiding it.

**Outcome:** Fixed in the dispatch before any subagent ran (`let replacement_src = new_string.strip_suffix('\n').unwrap_or(new_string);` before reindent) + a dedicated regression test. Drift never reached the implementer — textbook recon-before-dispatch.

**Cross-cutting lesson:** "Scout the seam before you act" applies to your OWN plan code, not just existing substrate. The writing-plans phase can author a subtly wrong splice/offset whose error is invisible in the common case; a controller re-read at the byte level (where exactly does the replacement land vs where the matched span ends) catches it for one read. Byte-offset / newline boundaries are a seam.

**Related session observations (same work stream):**
- MISS → promoted: plan test fixtures used `old_string`s that were literal substrings of the file, silently routing "normalized-path" tests through the EXACT path (3 instances); caught by per-task + holistic review, not the plan. Promoted to CLAUDE.md Testing Patterns (substring-overlap rule).
- Pre-commit diff-scout: `git diff --stat` before committing revealed Task 6's whole-workspace `cargo fmt` had churned 9 unrelated rustfmt-drifted files; verified pure-formatting via the raw diff (incl. correcting my own false "logic change" alarm from a corrupted grep) and excluded them from the feature. Lesson: in a drifted/shared repo, scope `cargo fmt -- <files>` or use `cargo fmt --check`, and `git diff --stat` before any `git add`.

**Promote-when:** a second instance where a controller re-read of plan-authored offset/splice/boundary code catches a bug pre-dispatch → promote a writing-plans/recon bullet: "Before dispatching an integration task, re-read any plan code that computes byte offsets, ranges, or splice boundaries — author error there is invisible in the common case."

**Status:** open — single datapoint for the splice bug; the substring-overlap sub-pattern reached promotion (3 datapoints → CLAUDE.md).

**Source:** `src/tools/edit_file/mod.rs` `perform_edit` `match_count==0` arm; plan `docs/superpowers/plans/2026-06-04-edit-file-whitespace-normalized-fallback.md`.

---
## R-17 — Spot-check sibling callers of a just-fixed shared helper before closing the bug class

**Verdict:** hit (recon caught a 3× blast radius; live repro + regression tests confirmed)

**Observed:** 2026-06-05, after fixing an `edit_code insert-after` parent-clamp off-by-one in
`do_insert` (last child of a dedent-delimited Python class). About to declare the bug class closed;
user asked to spot-check the flagged replace-path lead.

**Scout (reality):** `references(clamp_range_to_parent)` surfaced two more production callers —
`do_remove` (`edit_code.rs:454`) and `do_replace` (`:515`) — both converting `parent.end_line` into
an exclusive clamp bound with the identical bare-`end_line` off-by-one. Reproduced live against the
shipped binary before fixing: `edit_code replace` on the last method reported `replaced_lines: 5-9`
(excluding the trailing-`assert` line), leaving it orphaned after the new body; `remove` left it
behind. The AST-extractor and `do_insert`-specific reasoning were the wrong layer — the seam was the
**shared boundary helper's call contract**.

**Outcome:** Fixed all three sites (`+ 1`), added `replace_`/`remove_last_python_method_*` regression
tests (both verified fails-without/passes-with by reverting the `+1`), all 54 `symbol_lsp` tests green
including the `bug034_guard_*` over-extension guards. Captured as W-9 in `bug-fix-session-log.md`.

**Cross-cutting lesson:** when a fix corrects how ONE caller derives an argument to a shared
range/clamp/boundary helper, the same derivation error almost certainly lives at the other callers.
`references(helper)` + reproduce each call site's input shape BEFORE closing the bug class. A
single-call-site fix to a multi-caller helper-usage bug ships a partial fix that re-surfaces at full
debugging cost on the untouched paths.

**Also this session (re-confirms R-16's fmt sub-lesson):** whole-workspace `cargo fmt` churned files a
concurrent session left rustfmt-drifted (markdown `.rs` + a `server.rs` reflow). Caught via
`git diff --stat`; will commit file-scoped (`edit_code.rs` + `tests/symbol_lsp.rs`) rather than
`git add -A`. Second datapoint for "scope `cargo fmt -- <files>` / `--check` in a shared repo."

**Promote-when:** a second instance where `references()`-ing the callers of a just-fixed shared helper
catches an under-scoped fix → promote to CLAUDE.md: "When fixing how a caller uses a shared
boundary/clamp/offset helper, scout every other caller of that helper and reproduce each before
closing the bug class."

**Status:** open — single datapoint for the sibling-caller pattern.

**Source:** `src/tools/symbol/edit_code.rs` (`do_insert`/`do_remove`/`do_replace`);
`docs/issues/archive/2026-06-05-edit-code-insert-after-last-python-method.md`; W-9 in
`docs/trackers/bug-fix-session-log.md`.

---
## R-18 — Scout a classifier's actual return type AND domain coverage before keying a feature off it

**Verdict:** hit (recon corrected a type-shape assumption and surfaced a coverage gap before any code was written; clean compile + 210 tests + live verify confirmed)

**Observed:** 2026-06-05, pre-edit scout for the `edit_file` whitespace-normalized-fallback guard (disable the fallback for indentation-significant languages). About to write a guard keyed on `crate::ast::detect_language`.

**Scout (reality):** My mental model assumed `detect_language` returned a `Language` enum I would `matches!` on. Reading the real signature: it returns `Option<&'static str>` (canonical name strings), and — load-bearing — it does **not recognize YAML at all** (`.yaml`/`.yml` → `None`), so YAML currently takes the fallback with no AST gate either. The recognized indentation-significant set is just `python`/`haskell`.

**Outcome:** Changed the guard from enum-variant matching to **extension-based** classification (`py`/`pyi`/`hs`/`yaml`/`yml`) before a line was written. A name-based check (`detect_language(path) == Some("python")`) — the natural fix after the compile error — would have shipped the guard with YAML still ungated: a hole in exactly the language I had named in my own review critique. Shipped as `c99d4228`; 210 edit_file tests + live `.py`-refused / `.rs`-applied verification through the rebuilt server.

**Cross-cutting lesson:** when a new feature keys behavior off an existing classifier/predicate, scout TWO things, not one: (1) the function's actual return *type* (enum vs string vs bool — the assumption that bites at compile time), and (2) its *domain coverage* — which inputs return `None` / fall through (the gap that is invisible from the function name and silently survives a green build). The coverage gap is the dangerous half: it does not fail loudly. Here YAML's absence from `detect_language` was the whole reason to classify by extension instead.

**Promote-when:** a second instance where scouting a classifier/predicate's return shape AND domain coverage (not merely its existence) changes a feature's implementation → promote to CLAUDE.md: "Before keying behavior off an existing classifier, read its return type and enumerate which inputs it does NOT cover."

**Status:** open — single datapoint.

**Source:** `src/ast/mod.rs::detect_language`; `src/tools/edit_file/mod.rs::indentation_significant`; commit `c99d4228`.

---

## R-19 — Scout home-project internals before presenting cross-project "B benefits from A" recommendations

**Verdict:** miss → hit (retroactive) — two shape claims about codescout were presented as fact during a "what can codescout learn from headroom" analysis *before* either was scouted; a user-invoked recon pass then confirmed one and refuted the other.

**Observed:** 2026-06-09, after exploring the sibling `headroom` project to find codescout improvements. The prior turn delivered two recommendations resting on claims about codescout's *own* internals: (1) "`@ref` buffers are per-call handles, no content dedup" → add BLAKE3 dedup; (2) "codescout's overflow summarization is generic" → add per-tool error-keyword/path preservation. Neither claim was scouted against codescout's code before being presented — both were synthesized from memory + CLAUDE.md + the headroom comparison.

**Scout (reality):**
- Claim 1 — CONFIRMED. `src/tools/output_buffer.rs:250-251`: `id = format!("@tool_{:08x}", now.wrapping_add(inner.counter) as u32)` — time + monotonic counter, not content-addressed; identical content mints a fresh handle every call. Bonus: `content_hash()` (SHA-256, not BLAKE3 — corrected 2026-06-09; BLAKE3 was a conflation with Headroom's CCR) already exists at `src/retrieval/sync.rs:29` for embedding dedup — the primitive is present, just unwired from output buffers. Recommendation strengthened, not refuted.
- Claim 2 — DRIFT. `src/tools/core/types.rs:387` defines `Tool::format_compact(&self, result) -> Option<String>`, a **per-tool** summary hook each tool overrides (`None` → the generic "Result stored in @tool_xxx" fallback), and `run_command` already prioritizes stderr (`run_command/tests.rs:2034`). Summaries are tool-aware *by design* — not "generic." The accurate gap is narrower: no *content-level error-keyword preservation* primitive (no `preserve_error_keywords` / `always_keep` analogue), and the per-tool summaries are hand-written rather than profile-driven.

**Outcome:** 1 of 2 recommendations rested on a stale assumption. Corrected to the user before any code was written on the wrong premise.

**Cross-cutting lesson:** Kin to R-14 (scout dated-*memory* citations), but the source here was an *unsourced assumption* in a comparative analysis, and the scout was *retroactive* — the user's `/reconnaissance` invocation caught it, not a pre-emptive pass. A recommendation about the home project's internals is a shape claim — a seam — even when it feels like settled knowledge. In "project B benefits from project A" framing, the home-project half of every recommendation must be scouted against home-project code *before* it is presented. The pull to state the home side from memory is strongest precisely because it's "your" project.

**Recurrence (2026-06-09, same session — datapoint 2):** the pattern repeated *after this entry documented it*. I wrote "`content_hash()` is BLAKE3" into BOTH this tracker and `headroom-cross-pollination.md` without reading `src/retrieval/sync.rs:29` — it is **SHA-256** (`sha2 = "0.10"`). The BLAKE3 label was a conflation with Headroom's CCR (which genuinely uses BLAKE3). Recon did **not** catch it; it surfaced only when the user asked "what is BLAKE3?", forcing the read. Datapoint 1 ("generic summarization") was a recon *hit* — caught by a pre-emptive scout. This is a *miss*, and a wrong fact reached disk in two artifacts before correction. Recurrence-after-documentation is the signal that prose alone isn't holding the lesson.

**Recurrence (2026-06-21 — datapoint 3, cross-session HIT):** a new session asked "understand how llm-proxy/headroom integrate with codescout," and my answer repeated three home-project shape facts (the `@tool_*` handle minting, `content_hash`, `format_compact`) cited from `headroom-cross-pollination.md` rather than read this session. A user-invoked `/reconnaissance` scout then read all three against current code BEFORE they stood: **all three shapes RE-CONFIRMED** — handle minting is time+counter (`output_buffer.rs:251`), `content_hash` IS SHA-256 (`sync.rs:34-38`), `format_compact` IS a per-tool trait hook (default `None`, overridden in `config/mod.rs`). First clean *hit* on this seam (datapoints 1-2 were a hit+miss, same session). NEW finding: the **line citations drifted** since 2026-06-09 — `sync.rs:29`→`:34`, `types.rs:387`→`:435` (the `output_buffer.rs:250-251` cite held). The drift hit this entry's own `Source` line and `headroom-cross-pollination.md` C-1/C-2 — vindicating the lesson: cite the *symbol name* (greppable, stable), treat the *line number* as perishable, and re-read before re-asserting. Citations refreshed in both artifacts this session.

**Promote-when:** a second instance where a comparative / cross-project analysis presents a home-project shape claim that a later scout refutes → promote a Phase-1 Scout bullet: "Before presenting a recommendation that asserts how the home project currently works, scout the cited symbol/contract against current code — comparative analysis is not an exemption." (Note: R-14's own promote-when wants a second *stale dated-memory* instance specifically; this entry is adjacent, not that second datapoint.)

**Status:** open — **3 datapoints** (2 this session: a hit then a miss; + 1 cross-session retroactive hit 2026-06-21, see Recurrence above — re-confirmed all three shapes, caught line-citation drift). The 3rd does **not** advance the promote-when (it confirmed, did not refute) and does **not** verify skill efficacy (recon was again user-invoked, not auto-fired) — efficacy still N=0. **Acted 2026-06-09 (user decision):** rejected the project-local CLAUDE.md route as too narrow for a systemic lesson; instead tightened the recon SKILL.md `When NOT to Use` Read-only-Q&A exemption to draw the *describe-vs-assert* line (Hamsa-audited — a cut/bound, not an added trigger; `claude-plugins` working tree, uncommitted). This front-runs the cross-session-3rd caveat by deliberate choice (recurrence-after-documentation judged strong enough). **Efficacy unverified — N=0**: no behavioral eval (`docs/evals/reconnaissance-output.md` not yet authored); the existing trigger eval scores the description string, not body guidance, so it does not measure this change. Formal sync flow (PR + pinned SKILL.md commit SHA + skill version) still pending a commit.

**Source:** `src/tools/output_buffer.rs:251`, `src/tools/core/types.rs:435`, `src/retrieval/sync.rs:34`, `src/tools/run_command/tests.rs:2034` (line numbers refreshed 2026-06-21; symbol names are the stable anchors); this session's headroom cross-pollination analysis. Kin: R-14.

## R-21 — Verify a side-effect through its real entry point; `references()` the operation before placing it

**Verdict:** hit — live verification caught a gap that 46 unit tests + a functional test could not.

**Observed:** 2026-06-09, index-freshness scope-a. The sidecar write was placed in `IndexProject::call` (the MCP tool path) and "verified" by unit tests on `write_index_state` / `git_sync_status` + a hook functional test — all green.

**Scout (reality):** A live `codescout index` (CLI) produced no sidecar (`No such file or directory`). `references(RetrievalClient/sync_project)` enumerated **5 call sites** — 3 project (`index.rs:304` MCP, `main.rs:259` CLI, `bin/sync_project.rs:29`) + 2 library (`index.rs:130`, `agent/mod.rs:1493`). The write reached **1 of 3** project paths; the CLI (which the companion hook invokes) and the standalone bin wrote nothing.

**Outcome:** GAP. Moved the write to the `sync_project` chokepoint, gated by `SyncOpts.record_index_state`; live-verified `behind:1 → reindex → up_to_date` through the reconnected MCP server. Commit `10dcfb9f`.

**Cross-cutting lesson:** Two scouts the plan skipped. (1) `references()` the operation that *owns* the side-effect to enumerate ALL entry points before deciding where the effect lives — a unit test proves one path; `references()` proves coverage. (2) Verify through the real production entry point (the CLI/MCP path the consumer uses), not a unit harness that bypasses `main.rs`. Kin to R-17 (spot-check sibling callers of a shared helper) and the Snow Lion memory `cross-cutting-side-effects-at-the-chokepoint`.

**Promote-when:** a second instance where a `references()` entry-point audit or live-entry-point verification catches a gap unit tests missed → promote a verification-discipline bullet to CLAUDE.md / SKILL.md.

**Status:** open — single strong datapoint (live proof + `references()` both load-bearing this session).

**Source:** `references(RetrievalClient/sync_project)` → 5 sites; `src/retrieval/sync.rs`, `src/main.rs:259`, `src/bin/sync_project.rs:29`; commit `10dcfb9f`; index-freshness session-log F-1 + W-1. Kin: R-17, Snow Lion `cross-cutting-side-effects-at-the-chokepoint`.

## R-22 — Scout the LSP call path to confirm a staleness mechanism before choosing the fix layer

**Verdict:** hit — reading `References/call` + `LspClient::references` confirmed the false-zero mechanism, which determined the fix layer (LSP-independent corroboration, not an LSP retry).

**Observed:** 2026-06-11, scoping the `references-false-zero-stale-graph` bug (co-author-filed, severity high); asked to confirm the mechanism before fixing.

**Scout (reality):** `LspClient::references` calls `did_open` on the DEFINITION file only, then `textDocument/references` — no project-load / post-reindex barrier, so caller files the LSP has not yet loaded are absent (false `0`). The pre-existing completeness cross-check (`references_completeness_hint`) compares against `callHierarchy/incomingCalls`, ALSO LSP-backed, so it shares the staleness and is blind to a shared-zero. The cold-start retry budget includes references, but a definition-only result is a *successful* response, so it never retries.

**Outcome:** HIT. The scout ruled out the tempting-but-wrong fixes (add an LSP retry; trust the call-hierarchy guard) and pointed at the only trustworthy second opinion — an LSP-INDEPENDENT text scan, mirroring `call_graph` Phase B. Fix landed (`ddc7e3f1`) and live-reproduced: cold call returned `0` + guard warning; warm call returned 8 refs in 4 files, no warning.

**Cross-cutting lesson:** When the symptom is a same-query-different-result-over-time, scout whether the answering path shares a freshness root with every candidate corroboration signal. If all cross-checks are backed by the same lagging index/LSP, only an out-of-band source (text / tree-sitter) can corroborate. Kin to R-21 — both are the-obvious-second-opinion-is-not-independent.

**Promote-when:** a second instance where a shared-staleness scout redirects a fix from the lagging layer to an independent corroboration, then promote a corroborate-with-an-out-of-band-source bullet to CLAUDE.md / SKILL.md.

**Status:** open — single strong datapoint (scout + live repro both load-bearing). Same-session sibling: scouting `apply_body_edits` + `edit_markdown` validation before the U-26 fix (got the action grammar right pre-edit).

**Source:** `src/tools/symbol/references.rs` (`References/call`), `src/lsp/client.rs` (`references`), `src/tools/symbol/call_graph/mod.rs` (Phase B); bug `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md`; commit `ddc7e3f1`. Kin: R-21.
## R-23 — Re-derive an inherited diagnosis from telemetry (hit); verify a shared single-holder-resource recovery by reading state, not by calling from a 2nd client (miss)

**Verdict:** hit, then miss — the diagnostic scout was load-bearing and correct; the *recovery-verification* scout was self-defeating and caused a real regression.

**Observed:** 2026-06-11, debugging a foreign project (`~/work/mirela/backend-kotlin`) whose prior session concluded "`edit_code` crashes the Kotlin LSP." Asked to debug systematically.

**Scout (reality):**
- *Hit:* re-derived the inherited claim from `<project>/.codescout/usage.db` (`tool_calls.outcome`/`error_msg`) + `lsp_events.outcome` + `debug.log` `lsp_stderr` + live `fuser`/`ps`/`ss`. `symbols` disconnected identically to `edit_code`; the real cause was a kotlin-lsp **RocksDB index-lock deadlock** (a direct-fallback LSP grabbed the lock before any mux could establish), not a write-path defect. The transcript's `edit_code`-then-failure adjacency was correlation, not cause (the call had been user-*interrupted*).
- *Miss:* to "verify recovery" after killing the orphan lock-holder, I issued a `references` call from the **debugging session's** codescout server (workspace-pinned). That call spawned a direct-fallback LSP **owned by the debugging session**, which grabbed the just-freed RocksDB lock — making me the new squatter and breaking the user's *first* MCP restart (`tool_calls` 16948; `lsp_events` 354/355).

**Outcome:** The diagnostic hit prevented a wrong-target fix (a bug against `edit_code`/AST, neither broken). The verification miss cost one user-visible regression cycle; recovery only became durable after clearing the *entire* deadlock (kill all kotlin-lsp for the hash + remove the stale mux `.lock`) and a fresh `codescout start` spawned a healthy shared mux — `edit_code` then succeeded (`tool_calls` 16949/16950).

**Cross-cutting lesson:** A *recovery* scout of a **shared, single-holder resource** (an OS lock, a singleton LSP/mux, a RocksDB index) must be **read-only** — inspect process/lock/socket state (`fuser`, `ss -xlp`, `ps`), never issue an operational call from a second client. The call is itself a mutation that re-acquires the contended resource, so it both invalidates the test and can re-break the very thing you "verified." Hand operational verification to the owning client; the debugger only reads state. Kin to R-21/R-22 (the obvious second opinion is not independent) — here the obvious verification is not side-effect-free.

**Promote-when:** a second instance where a recovery/verification call from a non-owning client perturbs a shared resource. At 2 datapoints, promote a "verify shared-resource recovery by reading state, not by calling" bullet to SKILL.md (Phase 4 / verification composition) and CLAUDE.md.

**Status:** open — single strong datapoint; both the hit (diagnosis flip) and the miss (self-inflicted regression) are load-bearing and live-confirmed.

**Source:** `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`; `src/lsp/manager.rs:432-539` (`get_or_start_via_mux`), `:456` (flock-only liveness), `:485` (stderr→null); bug-fix session-log F-16 + W-12. Kin: R-15 (external-tool on-disk state), R-21, R-22.

## R-24 — Scout the resource-key derivation before designing a concurrency test; path-keyed hashing makes worktrees a safe fan-out fixture

(hit) Before testing shared-LSP behavior under concurrency, I scouted how the mux lock/socket + RocksDB home are keyed: `workspace_hash(workspace_root)` — the path (`src/lsp/mux/mod.rs:15-28`; `servers/mod.rs:81`; regression `socket_path_deterministic_for_same_workspace`). That single fact determined the whole test design — isolated worktree hashes cannot collide with the live mux, so a 3-agent concurrent cold-start runs safely on a working machine. Directly answers last session's R-23 miss (verifying shared-resource recovery by contending on the live hash). The same run also surfaced a 4-site `workspace=` pin gap and a concurrent-cold-start kotlin-lsp failure.

**Evidence:** bug-fix W-13 + F-18; `issues/2026-06-11-lsp-tools-ignore-workspace-pin-path`.
## R-25 — Scout the catalog's status source + id-keying before archiving a librarian tracker

(hit) `/reconnaissance` invoked as I was about to archive 3 session-logs by hand-editing `status: archived` frontmatter + `git mv` to `archive/`. Scouting the librarian first overturned the plan: `artifact(find)`'s active-vs-hidden verdict reads the **catalog DB row's** `status`, not the file (`find.rs:90` → `{nin: HIDDEN_STATUSES}`, defined `mod.rs:27`). A `git mv` + frontmatter edit touches neither the catalog status nor — until someone runs `reindex` — anything the librarian queries, so all 3 would have kept showing `active`. Worse: `id = sha256(abs_path)[..16]` (`ids.rs:17-23`), so the eventual reindex mints a NEW id for the moved file and the cleanup `DELETE … id NOT IN seen_ids` (`indexer.rs:56-237`, `index_repo_sync`) drops the old row — orphaning its `events`/augmentation. Correct mechanism: `artifact(action="update", patch={status:"archived"})` (writes catalog row + frontmatter now) then `artifact(action="move")` (atomic rename + abs_path re-point, `mv.rs:14-75`). Path-keyed-id kin of R-24 and R-15.

**Evidence:** this session — archived `kotlin-lsp-disk` + `metadata-filtering` + `mcp-prompt-redesign` session-logs; commit `b487a69c`. Doc gap (promotion candidate): CLAUDE.md's archive instruction ("set `status: archived` AND `git mv`") is file-only — it omits that the librarian's catalog status needs a *tool* write, so following it literally produces a cosmetically-archived-but-still-active tracker. Fix → prescribe `artifact(update)`+`artifact(move)` for librarian-tracked files.

## R-26 — A grep line-match locates a symbol; it does not confirm a mechanism

(hit) `/reconnaissance` invoked after I'd already written "the orphan-squatter mechanism, confirmed" into a mux-LSP-sharing brainstorm — but "confirmed" rested on a `grep` hit (`kill_on_drop` at `process.rs:93`, `Command::new` at `:86`), not a read of the function. Scouting the actual body (`run`, `src/lsp/mux/process.rs:66-135`) is what earned the word: the LSP child is spawned `.kill_on_drop(true)` with **no** `setsid` / `process_group` / `pre_exec` / signal handler anywhere in the spawn path, so `kill_on_drop` — which rides `Child::drop` and never runs under SIGKILL — is the *sole* teardown. The SIGKILL-mux → orphaned-JVM → immortal-RocksDB-lock-holder mechanism (the systemd-reparented kotlin-lsp we found on backend-kotlin) therefore holds. The grep proved *presence* of `kill_on_drop`; only the read ruled out the *falsifier* — a process-group or signal handler that would reap the child regardless. Lesson: a grep line-number is Phase-1 location, not Phase-2 confirmation — never narrate "confirmed" on a mechanism off a grep hit; read the body. Kin of R-19 (assert-a-checkable-fact only after reading) and R-5 (grep ≠ proof).

**Evidence:** this session — backend-kotlin mux/RocksDB lock-contention brainstorm; claim verified read-only against `src/lsp/mux/process.rs:66-135` (no commit). Open edge (R-15 kin, still unscouted): the same brainstorm's *upgrade* recommendation rests on an upstream changelog line — kotlin-lsp v261.13587.0 "indices … properly shared between multiple projects and LS instances" — known only via a WebFetch fast-model paraphrase of `RELEASES.md`, not a raw read; explicitly flagged verify-before-acting and not yet acted on.

## R-28 — Enumerate a prompt surface's full gate set before editing; targeted test filters miss cross-cutting gates

(hit + miss) Editing the onboarding system-prompt instructions touched `source.md` (onboarding_prompt surface), `memory-templates.md` (`{{include}}`'d into both single + workspace flows), `workspace_onboarding_prompt.md`, and `builders.rs`. **Hit:** pre-edit recon enumerated the gate set — `extracts_onboarding_prompt_byte_for_byte`, the `prompt_surfaces_onboarding_snapshot` fixture, the 2200-byte `source_md_under_cap`, the `assert_eq!(ONBOARDING_VERSION, 28)` version-pin, the workspace-scope heading-presence test, and the `"6 memories"` content test — so every gate update landed in one commit and only the snapshot needed a legitimate re-bless. **Miss:** a sibling change earlier the same session (get_guide description, `c799e887`) was verified with a *targeted* `cargo test --lib guide::` that did not include `server::tests::tool_descriptions_stay_under_budget` (`src/server.rs:1598`); the 329-char (cap 300) description shipped and was caught only by the onboarding task's full-suite run (bug-fix F-20). Lesson: a tool/field's validating gate often lives in a DIFFERENT module (`server::tests`) than the code it guards; `cargo test --lib <module>` filters silently skip it. Enumerate gates by what they assert on, not by where the edit sits — and run the full suite (or `server::`) for anything a global budget/snapshot gate validates. Kin R-1/R-7 (include_str'd-constant invariants), R-27 (verify before building on a claim).

**Evidence:** bug-fix F-20 + W-15; commits `8427ae4a` (onboarding fix), `31a655e5` (description-budget), `c799e887` (where the gap shipped); verified read-only against `src/prompts/source.rs`, `src/prompts/mod.rs`, `src/server.rs:1598`.
## R-29 — Verify a flight-recorder-harvested target exists in the active repo before ranking it

**Date:** 2026-06-13 · **Verdict:** hit · **Kin:** R-23 (usage.db telemetry re-derivation), R-19 (cross-project recommendation scouting)

**Context:** Dzo Phase-1 legibility survey — ranking refactor targets by truncated-`symbols` recurrence harvested from `.codescout/usage.db`.

**Scout:** Before ranking, existence-checked each harvested target against the repo (`grep -rl <symbol> --include=*.rs`, `wc -l <path>`, `symbols` resolution). `CalendarService` (3 truncated fetches) failed the check; a follow-up `project_sha` query traced it to `project_sha=1e8b9eb1`, path `/home/marius/work/mirela/backend-kotlin/.worktrees/cs-stress-*/.../CalendarService.kt` — a mirela Kotlin stress-test session.

**Finding:** `.codescout/usage.db` is **not** single-project. It is keyed by commit-SHA-at-call-time and holds 40 distinct `project_sha` values, mixing every project the process-global server served. A symbol/path harvested from telemetry is a *candidate*, not a confirmed in-repo target.

**Lesson (the recon rule):** Telemetry-harvested targets need an in-repo existence verification before they drive a ranking or a refactor. The verification is one `grep -rl` per path-less candidate; the filter for a clean survey is `WHERE project_sha IN (<repo's shas>)` or a path-prefix match.

**Counterfactual:** Without the check, `CalendarService` (tied with mid-tier real targets at 3 fetches) ranks ~#4, and the Dzo runs `symbols`/`semantic_search` readings that return empty against code not in the repo — churn, and a possible phantom tracker.

**Proposal:** Bake a `project_sha`/path-prefix filter into the Pika + Dzo survey queries so the contamination is excluded at the source, not caught per-symbol. Promote-when: a second cross-project phantom surfaces in a flight-recorder survey.

**Evidence:** `docs/trackers/archive/dzo-legibility-session-log.md` F-1 + W-1.

## R-33 — Dead-vs-live is a per-symbol call-graph fact, not a file-proximity fact

**Date:** 2026-06-15 · **Verdict:** hit

**Seam:** `src/embed/fusion.rs::rrf_fuse` + `BM25Result` vs `src/embed/schema.rs::SearchResult`. The legacy-retrieval-removal tracker's 2026-06-14 reconciliation marked **both** "graduated into live consumers — NOT deletable" because they are adjacent symbols from the same legacy-fusion era.

**Scout:** `references(rrf_fuse)` → 1 def + 5 test-only call sites, **zero production callers**. `references(SearchResult)` → a live consumer (`apply_file_diversity_cap` in `semantic_search.rs`). The two symbols had **opposite** liveness; file proximity hid it.

**Disproof:** deleted `fusion.rs` (+ its `pub mod` decl + the dead `rrf_fuse_integration_*` test); `cargo test` 2869 passed, clippy `-D warnings` clean. The "not deletable" claim was false for `rrf_fuse`.

**Lesson:** an audit that reasons about dead-vs-live from imports / co-location / "same era" will mis-classify. Liveness is a per-symbol call-graph fact — confirm each symbol with `references()` / `call_graph()`, never by its neighborhood.

**Promote-when:** a 2nd proximity-misclassification caught by a call-graph scout → promote to the reconnaissance SKILL ("dead-code claims are per-symbol call-graph facts, not file-level facts"). Kin: R-26 (grep line-match ≠ mechanism), R-27 (prose claim is a hypothesis), R-21 (`references()` before acting).

## R-34 — On a cross-platform branch, the host `cargo check` is not the rebase gate; cross-compile to the target the branch exists to support

(hit) Rebased `vdi-windows` onto `experiments` — 41 ours / 89 theirs. The sole conflict was in `src/tools/mod.rs`, in the very `#[cfg(unix)]` gating the branch exists to maintain: `experiments` added `pub mod guide_ledger;` and our `d07d7b18` added `#[cfg(unix)]` to `pub mod peer;` at the same line; both kept. The seam I scouted was not the conflict (that resolution is trivially correct — two independent edits) but the **completeness of the platform gating after absorbing 89 incoming commits**. A host `cargo check` (Linux/unix) only re-compiles the path that already built; it never exercises the Windows side, so it cannot answer "is the rebase correct?" for the dimension this branch is *about*. Scouted the real target: `cargo check --target x86_64-pc-windows-gnu` (MinGW + windows-gnu target both present from the branch's existing local loop) — EXIT=0, clean but for 3 pre-existing dead-code warnings (unix-only LSP-mux helpers in `src/lsp/manager.rs`, already tracked as WIN-23). 

**Counterfactual:** had any of the 89 `experiments` commits introduced an ungated `peer::` use, `std::os::unix::*` import, or other unix-only call, a Linux-only "looks good" would have shipped a broken Windows build that surfaces only on the user's next VDI compile — maximally far from the rebase, hard to attribute back to it. The cross-compile makes the gate fire at the seam, not in the field.

**Generalization:** when rebasing/merging a branch whose purpose is a non-host target (platform gating, no_std, a different arch/ABI, a feature-flag matrix), the host build is necessary-not-sufficient. Compile the target the branch exists for before claiming the integration is correct. Mirrors R-5 ("compiler as scout") but on the *target* axis rather than the call-site axis.

**Evidence:** `vdi-windows` rebase this session (master-side conflict resolve commit on the rebased tip); `src/tools/mod.rs` peer-gate; `cargo check --target x86_64-pc-windows-gnu` EXIT=0; WIN-23 dead-code warning cluster (`windows-platform-support.md`).
## R-35 — A tool's own error diagnostic is a hypothesis, not ground truth; reproduce the failing internal call on the real file

(hit) Debugging a live `edit_code(insert, position="after")` refusal on
`backend-kotlin/.../RoomConstraintsTest.kt`. The refusal text — *"cannot determine
end of '…' — AST parse failed"* with hint *"the file likely has syntax errors … or
duplicate-name siblings"* — named two causes, both falsified by a higher-level read:
`symbols()` listed all 14 methods (clean parse), and the `name_path` was unique. A
second plausible lead, the archived 2026-05-29 "Kotlin backtick mismatch" bug, was
already fixed and present in the code. Two visible leads, both dead ends.

The seam was the matcher (`find_ast_end_line_in` / `collect_ast_candidates`,
`src/symbol/query.rs`) and the **coordinate systems its two inputs live in**. I
resolved it by reproducing the exact internal call on the real substrate: a throwaway
unit test `include_str!`-ing the real file, running
`extract_symbols_from_source → find_ast_end_line_in`, and printing each symbol's
`name` / `name_path` / `start_line` / `end_line`, then replaying the matcher across
candidate line values. One run pinned it: AST `start_line=214` (the `@Test`
annotation line — tree-sitter's function node spans its annotations) vs kotlin-lsp
`216` (the `fun` line); `find_ast_end_line_in(215)->Some(266)`, `(216)->None`. The
±1 line gate was the cause — never named by the diagnostic.

**Counterfactual:** acting on either visible lead yields a no-op ("fix" syntax that
isn't broken) or a wrong fix (re-do shipped backtick normalization). The empirical
dump converted guess-and-check (≥1 wrong edit+build+re-reproduce cycle, ~4
round-trips) plus the temptation to widen the ±1 tolerance (a sibling-mismatch
band-aid) into a single ground-truth measurement that pointed at the real fix
(`name_path`-first matching, no line gate).

**Generalization:** when a tool refuses with a diagnostic that names a cause, and a
higher-level read of the same artifact contradicts that cause, the two are using
different internal representations. Don't pick among the named causes — reproduce the
failing internal call on the real file and print its actual inputs. Extends R-5
("compiler as scout") and the W-14/W-1 "verify the mechanism against the real body"
line to a tool's *self-diagnosis*: the error string is the least-trustworthy witness
when a cheaper read already disagrees with it.

**Verdict:** hit. Cited as W-17 / F-23 in `docs/trackers/bug-fix-session-log.md`; root
cause + fix in `docs/issues/archive/2026-06-16-kotlin-edit-code-annotation-line-gap.md`.
Promote-when: a 3rd instance where a tool diagnostic misdirects and an empirical dump
of the internal call corrects it → distill into codescout memory `reconnaissance`.

**Evidence:** `src/symbol/query.rs` `find_ast_end_line_in`/`collect_by_name`; throwaway
dump output (AST 214 / LSP 216, matcher flips Some→None at 216); regression test
`find_ast_end_line_in_bridges_annotation_line_gap`; `cargo test --lib` 2790 passed.

## R-39 — Adding a tool param/alias is additive-safe (positive-presence schema tests, no `additionalProperties:false`)

**Observed:** 2026-07-10, unifying the path-param alias set across the file/markdown/symbol tools and adding `file_path`/`output_id` schema properties (param-alias-ergonomics session, driven by a usage.db error-pattern sweep across 72 project DBs).

**Scout done (before editing a shared contract + several `input_schema`s):** grepped the whole tree for schema-shape tests. Every `*_schema_*` test is *positive-presence* — `schema["properties"]["x"].is_object()`, `contains_key(...)`, `enum.contains(...)`; none enumerate the exact property set or assert a property is *absent*. Also confirmed no tool `input_schema()` sets `additionalProperties: false` (`src/server.rs` assembles the schema object without it).

**Two consequences, both confirming the edit was safe:** (1) adding schema properties (`file_path`, `output_id`) *cannot* break any existing test; (2) aliases work at runtime *precisely because* there is no strict schema validation — an unknown key flows through to `call()`, which is the whole mechanism the alias fix relies on. The `create_file` `file_path` fix earlier in the session had already proven the passthrough empirically.

**Verdict:** hit — confirmed empirically. After adding `require_str_param_or_hint`, the shared `fs::require_path_param` alias, and 3 schema props, the full `cargo test --lib` run was **3038 passed / 0 failed**. The scout let me add the schema surface without hedging or a discover-by-breakage cycle.

**Generalization:** before worrying that adding a tool param/alias will break a schema snapshot or be rejected by strict validation — in codescout neither risk exists. Schema tests guard *presence*, not exact shape; schemas are *open* (`additionalProperties` unset). A param/alias addition is additive-safe. The one real gate a *description* edit must still clear is `server::tests::tool_descriptions_stay_under_budget` (R-28/R-37) — but adding a schema *property* (not touching `description()`) doesn't approach it. Kin: R-36 (serde has no `deny_unknown_fields` → the sibling open-schema property at the config-data layer).

**Evidence:** positive-presence tests in `src/tools/{memory,semantic,symbol,ast,run_command}/tests.rs` + `src/tools/peer.rs`; schema assembly in `src/server.rs` (no `additionalProperties`); gate: 3038 passed / 0 failed. Promote-when (2nd datapoint): distill "adding a tool param/alias is additive-safe — schema tests are presence-only, schemas are open" into codescout memory `reconnaissance`. param-alias-ergonomics session (this session); commit pending.

## R-41 — A table-rebuild migration's `INSERT … SELECT` column list is a silent allow-list

**Verdict:** miss → promoted (was a pending seam-class in SKILL.md Phase 1; Stage-2 is the confirming datapoint)

**Observed:** 2026-07-17, entry-graph Stage-2 (v9 catalog migration adding `artifact.slug` + `entry_cite`). The implementer added the column; an Opus task review caught the drop.

**Pattern:** Adding or changing a column/field is a seam whose far side is every LATER migration that rebuilds the same table (`CREATE table_new` / `INSERT … SELECT` / `DROP old` / `RENAME`). The rebuild's `INSERT … SELECT` column list is a silent ALLOW-LIST: any column it does not name is dropped when the rebuilt table is swapped in — no error, no failing test unless one asserts the column survives. The scout question is not "did I add the column?" but "does every column an earlier migration added still appear in every later rebuild's SELECT?"

**What happened:** v9 added `artifact.slug`. An earlier migration, `migrate_v6.rs::drop_legacy_and_stamp`, rebuilds the `artifact` table via copy-and-rename; its `INSERT … SELECT` did not name `slug`, so a single open through v6 dropped the just-added column (and cascade-dropped `entry_cite`). The bug shipped into the branch; the Opus task-1 review caught it against the diff, not the implementer's own scout.

**Counterfactual:** Untested, a v6-path open would silently degrade every entry-graph tracker to file-grain — slugs gone, cites cascade-dropped — with green tests, surfacing only when a downstream cite query returned empty.

**Proposal:** promote from pending seam-class to a Phase-1 named seam. When a diff adds a column, `grep` the migrations dir for the table name and read every later rebuild's SELECT list; a migration-shape regression test (open through the rebuild, assert the new column's value survives) is the durable gate. Fixed 9aa8063f + `migration_v6_single_open_preserves_v9_entry_graph_shape`.

**Evidence:** tracker-entry-graph Stage-2 (experiments); `src/librarian/catalog/migrate_v6.rs`; fix 9aa8063f; test `migration_v6_single_open_preserves_v9_entry_graph_shape`; kin R-3 / R-28.

## R-42 — A reader's None/absent branch that dead-ends silently drops every value the writer stored in that variant

**Verdict:** miss → promoted (was a pending seam-class in SKILL.md Phase 1; Stage-2 is the confirming datapoint)

**Observed:** 2026-07-17, entry-graph Stage-2 whole-branch review (write-time cites: the writer stores references keyed either by 16-hex id or by `<slug>:<local>`).

**Pattern:** When a diff's writer produces a new value shape — an id-keyed reference, an optional field, a Some/None variant — read the writer AND every reader of that shape. The diagnostic question is whether each reader's absent-key / None branch actually RESOLVES the other shape, or dead-ends (returns empty, falls through) and so silently drops every value the writer stored in that variant. Confirm each reader handles BOTH present/Some and absent/None — not just the case the writer's own test constructs. Shared incidental preconditions between writer and reader tests (e.g. "the target always has a slug") mask the gap.

**What happened:** `get(include_links)` surfaced a tracker's backlinks. Its outgoing/incoming-by-slug branches were gated on the target HAVING a slug; a slug-less target's incoming-by-id backlinks were silently hidden — the "no slug" branch dead-ended instead of falling back to id-keyed resolution. The writer's tests always constructed slug-bearing targets, so nothing failed. Caught in the final whole-branch review.

**Counterfactual:** Untested, every backlink INTO a slug-less tracker would be invisible in `get` output — data present in the catalog, absent from the view, green tests throughout.

**Proposal:** promote from pending seam-class to a Phase-1 named seam. For a new writer shape, `references()` its readers and check each variant branch resolves rather than dead-ends; add a reader test with the absent/None precondition the writer's tests don't construct. Fixed 70d16686 (incoming-by-id runs unconditionally; slug-gated branches are additive).

**Evidence:** tracker-entry-graph Stage-2 (experiments); `src/librarian/tools/get.rs`; fix 70d16686; kin R-27 / R-21.

**Generalization (extends R-29 / R-23):** usage.db is keyed by commit-SHA and accumulates across every project AND every commit the process ever served. A high error count is a *hypothesis about the current binary*, not a fact about it — reproduce a telemetry-surfaced friction against today's code (read the impl, or run the tool once) before fixing. Promote-when (2nd datapoint) → codescout memory `reconnaissance`.

**Evidence:** `src/tools/file_summary/file_summary.rs` `split_on_unbracketed_dot` / `strip_matching_quotes`; live re-repro of the filter inversion (`unknown field \`contains\``). param-alias-ergonomics session.


## R-44 — Hit: a proposed `#[cfg]` gate needs its consumer set enumerated, not its declaration site read

**Verdict:** hit (recon caught it pre-dispatch; no downstream gate was reached because no code was written) — write-side twin of R-43, backstopped by R-5

**Observed:** 2026-07-25, pre-dispatch reconnaissance on Stage 1 of
`docs/plans/2026-07-25-embedding-transport-consolidation.md`. Same work stream
as R-43, one session later.

**Pattern.** R-43 covers *reading* gating that already exists. This is the
other direction: a plan task that says *"gate `pub mod X;` on feature F"* is
proposing a **subtree delete** of X under `not(F)`. Its blast radius is X's
transitive import set, which the declaration site cannot show and which the
plan's own line citations will not mention. Two checks before accepting such a
task:

1. **Enumerate consumers.** `grep "<mod>::|use .*<mod>"` at *workspace root*
   (per R-3) with `context_lines=2` — the context is what reveals whether each
   consumer is itself gated, which a bare match cannot.
2. **Ask which config is being gated out, and what lives there.** If the plan
   also adds a test or asserts an invariant under `cfg(not(F))`, gating the
   module on F deletes that test from the `not(F)` build — while `cfg(not(F))`
   already excludes it from the `F` build. Net: it compiles in zero
   configurations, `cargo test` stays green, and nothing reports the hole.

**What happened.** Plan Task 1.3 read *"Gate `pub mod reranker;`
(`src/retrieval/mod.rs:14`) and `pub mod client;` on `server-stack`."* The
declaration-site citations were both correct — `mod.rs:3` and `:14` are indeed
ungated. The gap was entirely on the consumer side: `RetrievalClient::from_env`
has ~14 ungated call sites, two of them in *ungated sibling modules*
(`src/retrieval/search.rs:3`, `src/retrieval/sync.rs:195`), and the plan's own
load-bearing invariant proof requires `from_env` reachable under
`not(server-stack)`. Task 1.0 — scheduled to land *first* — puts a
`#[cfg(not(feature = "server-stack"))]` invariant test inside `client.rs`, so
Task 1.3 would have made it uncompilable in both configurations.

**Why this is the expensive class.** The 14 E0433 errors are loud and the
compiler catches them (R-5). The dead test is **silent** — green suite, plan
records the invariant as pinned, guard does not exist. No gate in the plan
checks that a test compiles in the configuration it names. Recon is the only
thing in the loop that looks at task ordering against cfg reachability.

**Proposal.** Add to SKILL.md Phase 1 as a named seam class, alongside
schema-migration ordering and writer-shape ↔ reader-surfacing:

> **Seam class: conditional-compilation gate.** A task proposing `#[cfg]` on a
> module/item declaration is a subtree delete. Enumerate the consumer set at
> workspace root with context, and check whether the config being gated out is
> where the plan's tests or invariants live — a test under `cfg(not(F))` inside
> a module gated on `F` compiles in zero configurations and fails silently.

**Routing note.** Craft-shaped for Rust generally, so SKILL.md is the
destination rather than project memory. **No promotion yet — n=1.** R-43 is not
a second datapoint for this rule: it is the read-side twin, a different
imperative in the same family. Counting the two together would inflate a
family-level pattern into a rule-level threshold. Hold the SKILL.md PR and the
`reconnaissance` memory write until a second consumer-set case lands (hit or
miss, any work stream); the ledger entry carries it at n=1.

**Evidence:** `src/retrieval/mod.rs:3,14`; `src/retrieval/search.rs:3`;
`src/retrieval/sync.rs:195`; `src/retrieval/client.rs:6,17,65-71`; 14
`RetrievalClient::from_env` sites across `src/tools/{semantic,memory,config}`,
`src/tools/onboarding.rs`, `src/agent/mod.rs`, `src/main.rs`,
`src/dashboard/api/index.rs`; work-stream narrative in
`docs/trackers/dependency-review-session-log.md` F-3 + W-2; kin R-43 (read-side
twin), R-5 (compiler backstop that would NOT have caught the dead test), R-17
(sibling-caller spot-check).
## R-45 — Hit: relocating a file needs a discovery-by-scan grep, which caller enumeration cannot substitute for

**Verdict:** hit (ordering caveat — recon ran after the edit, before the gate).

Fixing the two-module lock-file leak
(`docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md`), the `lsp/mux`
half changed *where* files live: `mux_dir()` returns a per-process scratch
subdirectory of `per_user_runtime_dir()` under `cfg(test)`. Caller enumeration is
the reflex for a signature change, and it was done — all four production sites of
`socket_path_for_workspace` / `lock_path_for_workspace` resolve to
`src/lsp/manager.rs:831-832` plus the `#[ignore]`d test at 2350-2351. But caller
enumeration is structurally blind to the consumer that matters for a *relocation*:
code that finds the file by scanning the directory never calls the path helper at
all. A separate `read_dir|glob` grep over `src/**/*.rs` was required, and returned
zero runtime-dir hits — which is what licensed the cheap 3-line seam over threading
a directory parameter through `LspManager::get_or_start` and opting 17 tests in
individually.

Generalises R-44 ("a proposed `#[cfg]` gate needs its consumer set enumerated, not
its declaration site read") by naming *which* consumer set: for a `cfg` gate on a
**value**, callers are the consumer set; for a `cfg` gate on a **location**, callers
are only half of it, and the invisible half is discovery-by-scan. Same shape as W-6
(2026-05-24) one level up — a representation change whose blast radius sat at the
read seams, not the write seams, and cost six broken boundaries including one
destructive.

The scout also produced two findings the edit had not anticipated: the `#[ignore]`d
wedged-mux test stops leaking for free, and `peer_*_for_workspace`
(`src/socket_discovery.rs:43-56`) shares the same real directory with the same
latent exposure — dormant only because no `codescout-peer-*` files exist on disk.

**Ordering caveat, recorded rather than smoothed over.** Recon was invoked after
both edits were written and clippy was green. The assumption held, so nothing broke,
but it was unverified at edit time. The tell that should have fired: the two halves
of this fix are different seam classes — `index_lock`'s is a parameter addition
(blast radius = callers, fully enumerable), the mux's is a relocation (blast radius
includes non-callers). Treating them as one "same fix, same risk" unit is what
deferred the scout.

**Evidence:** `bug-fix-session-log.md` F-33 (ordering inversion + the misleading
`peer_socket_differs_from_mux_and_shares_dir` name it left behind) and W-25 (the
pattern, with both counterfactual branches costed). Verification: 18 binaries, 0
failed, 3307 lib tests; index-lock leak 7→0 files per run, mux leak 18→1.

**Promote-when:** a third relocation-shaped change (file path, socket path,
collection name, cache key) where scan-vs-compute decides the design. Craft-shaped,
not project-shaped — holds in any language with a shared runtime directory — so the
destination is `SKILL.md` Phase 1 as a named seam class, not a project memory.

## R-47 — Hit: enumerate the delegate's callers too; the report walked one call path

**Verdict:** hit.

The bug file for the reindent/string-literal defect named its fix site twice — option 1
in `reindent_to`'s base computation, option 2 in the shift `reindent_to` delegates. Both
read as choices about *how thorough* to be. Enumerating the whole chain showed they were
also a choice about *where*, and that both options were one level too high:

- `reindent_to` — 3 callers, all in `src/tools/symbol/edit_code.rs`.
- `reindent_block`, its delegate — the same 3 transitively, **plus**
  `src/tools/edit_file/mod.rs:747`, which calls it directly with bases taken from the
  first non-blank line rather than via `min_indent`, on the whitespace-normalized-match
  repair path. Same literal-shifting defect, never touching `reindent_to`.
- `min_indent` — exactly one production caller (`reindent_to`), which is what made it
  safe to leave its public contract alone and add a sibling instead.

Putting the literal mask in `reindent_block` — the function that performs the shift —
fixed both entry points for the same size of change. Putting it in `reindent_to` would
have fixed one.

What makes this a recon lesson rather than a lucky catch: the missed path is *invisible
to its own guard*. `edit_file` validates every write with
`crate::ast::has_syntax_errors(&new_content, lang)` and refuses if the edit introduces
errors — and a string literal with four extra spaces of interior indentation still
parses. The one mechanism positioned to catch this returns clean on it. So the residue
would have survived a green gate, a closed bug file, and a passing syntax check, with no
surface left that could report it.

The generalisable form: a bug report is written from the single call path its author
walked. Scouting the named symbol confirms that path. Scouting the symbols it *delegates
to* is what finds the paths the author never saw — and a defect in shared machinery lives
on whichever link of the chain actually performs the offending operation, not on
whichever link the reporter reached it through.

Relation to kin entries: R-17 says spot-check a just-fixed shared helper's sibling
callers before closing the bug class — R-47 moves that check *before* choosing the fix
site, where it can still change the answer. R-31 follows a param one hop into callees;
this follows the operation. R-44 enumerates a consumer set rather than reading a
declaration site. R-45 is the counterweight: enumeration's silence is not proof, since
it cannot see discovery-by-directory-scan.

**Evidence:** `bug-fix-session-log.md` W-27. Shipped `79cd1428`; six new value-shaped
tests including `reindent_block_emits_literal_continuations_verbatim`, which pins the
`edit_file` entry point specifically so the second path cannot regress silently. Full
gate 18 binaries / 3445 passed / 0 failed / 44 ignored, clippy clean. Bug (archived):
`docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`.

**Promote-when:** a second case where enumerating a *delegate's* callers (not the named
symbol's) relocates the fix site. Craft-shaped — true of any language with shared
helpers — so the destination is `SKILL.md` Phase 1: *"scout the callers of every
function in the chain the fix touches, not only the one the report names; a defect in
shared machinery lives on the link that performs the operation."*

## R-48 — Hit: a fix built from the report's reproduction inherits its blind spots, and so does every test written against it

**Verdict:** hit — though a near-miss: the gap survived commit, a green gate, seven tests
and an archive, and was caught only at the live-verification step.

The reindent/string-literal fix shipped in `79cd1428` recognised a `"` literal spanning
lines via a trailing `\`, which is what the bug report's fixture used. It did **not**
recognise a plain `"` with a raw newline — valid Rust, and the commoner way to write a
multi-line fixture. Six tests accompanied the fix. All six reused the report's shape, so
the suite sampled one point of the defect class six times and reported it as coverage.

What caught it was writing the end-to-end verification. Proving the fix live meant writing
a multi-line literal *the way one naturally writes one*, and tracing the scanner over that
fixture showed it resetting at the first line end. The trace, not the run, was the
detector — the failure would have been silent in the run too, since a corrupted fixture
just changes what the test asserts about.

Two generalisable rules, and they are separable:

1. **For a syntax-level fix, enumerate the language's other syntaxes for the same
   construct before closing.** Rust spells a multi-line string four ways — raw newline,
   `\` continuation, `r"…"`, `r#"…"#`. The report used one. A fix whose doc comment
   enumerates a class should have a test per item of the enumeration, sourced from the
   language reference rather than from the report.
2. **Write the end-to-end verification fixture in the natural form, not copied from the
   report or the existing tests.** A suite that inherits one fixture shape cannot see past
   it, and the count of passing tests is no evidence at all about the shapes it does not
   contain.

A note on why this class is dangerous rather than merely embarrassing: the incomplete fix
is *camouflaged by its own artifacts*. A green gate, a closed bug file whose evidence
section explains the mechanism correctly, and a commit message claiming the class all
point away from the residue. A rediscovery arrives as a new bug filed against a closed one
that appears already understood.

Relation to kin entries: R-26/R-27 ("read ≠ verified") is the same shape one level down —
there, reading code was mistaken for testing it; here, testing a sample was mistaken for
covering a class. R-46 says the target's tests encode what may not change; R-48 is the
converse warning — tests also encode what was never asked, and their silence is not
coverage.

**Evidence:** `bug-fix-session-log.md` F-36 (the gap, with severity rationale) and W-28
(the fixture-authoring discipline that caught it, with counterfactual). Widened in the same
session; gate 18 binaries / 3446 passed / 0 failed / 44 ignored, clippy clean. Bug
(archived, with a follow-up section recording the widening):
`docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`.

**Promote-when:** a second case where an end-to-end fixture written in the natural form
exposes a gap the unit tests shared. Craft-shaped — true of any language and any test
suite — so the destination is `SKILL.md`, either as a Phase 4 bullet (verification is a
scout, not just a check) or as a "When NOT to Use" counterweight: a green suite is not a
scout, and the number of passing tests says nothing about the shapes absent from all of
them.

## R-49 — Hit: your own bug file is a hypothesis; re-scout it before implementing its fix

**Verdict:** hit.

An hour after filing `2026-07-28-edit-code-target-base-from-stale-lsp-range.md`, scouting
before implementing its prescribed fix showed the `## Root cause` section was not
supported. The file cited two real things — `target_base = leading_ws(lines[…])` at the
write site, and `editing_start_line` keying off `range_start_line` at the index's origin —
and concluded "stale LSP index → wrong line sampled". Between those two functions sits
`fetch_validated_symbol`, which runs `validate_symbol_position` first and *retries* on a
stale `start_line`. The guard appears in neither cited function, so a mechanism inferred
from the chain's two ends could not have mentioned it. Two further checks finished it off:
every candidate value of `range_start_line` for that anchor sits at column 4, and a later
insert in the same file with the same shape landed correctly.

Two separable rules:

1. **When a root cause names two functions, read the layer between them.** Guards,
   validators, and retries live in the middle of a call chain precisely because neither
   end is responsible for them. Reading both ends feels like following the data; it is
   actually sampling the two points least likely to contain a guard.
2. **Re-scout your own artifact on re-entry.** A bug file written while the surprise is
   fresh is exactly right for *capture* and unreliable for *causation* — and the moment it
   is committed, its `## Root cause` becomes what the next reader trusts instead of
   re-deriving. Give it the skepticism you would give a stranger's.

The payoff was not merely avoiding a wrong fix. The re-scout found a *different*,
code-verified defect in the same neighbourhood: `editing_start_line` has a documented
discard-the-walk-back path that returns `range_start_line` unchanged, which for a
KDoc/Javadoc block leaves it on a ` * ` continuation line — one column deeper than the
declaration. Sampling the base there re-bases every inserted body one space off, silently,
with no relationship to LSP staleness at all. A fix justified by a *property of the code*
replaced one justified by *a belief about an observation*, and the replacement was
testable with four plain unit tests instead of an LSP harness.

This is the third session-authored artifact to fail under later scrutiny in one sitting —
R-46/F-34 (a bug file's cheapest fix option was exactly inverted), F-35 (two doc comments
overstated guarantees the code did not give), and this. The common factor is not
carelessness in any one of them: it is that all three were written *while doing the work
they describe*, when the writer's model is at its most confident and least tested. The
countermeasure is temporal, not attentional — re-read on re-entry, not harder on write.

Relation to kin entries: R-32 says an off-the-cuff root-cause unification across a bug
cluster is a hypothesis, not a finding — R-49 is the single-bug case, and adds that
authorship is no exemption. R-46 is the same artifact failing in its *fix options* rather
than its root cause. R-26/R-27 ("read ≠ verified") is the underlying shape.

**Evidence:** `bug-fix-session-log.md` F-37 (the unsupported claim, with what survived it)
and W-29 (the re-scout, with a three-part counterfactual). Bug entry downgraded to
`mitigated` with `## Root cause` split into *Established* / *Not established*. Shipped:
`anchor_indent` in `src/symbol/edit.rs`, used by both `do_insert` and `do_replace`, with
four unit tests including one asserting **both** columns of the ` * `-continuation hazard.
Gate 18 binaries / 3450 passed / 0 failed / 44 ignored, clippy clean.

**Promote-when:** criterion already met — 2 datapoints against the same artifact (F-34,
F-37) and 3 against session-authored artifacts generally. Craft-shaped, so the destination
is `SKILL.md` Phase 1: *"re-entering your own bug file or plan to implement it counts as a
seam — scout the root cause again; and when it cites two functions, read the layer between
them."* Pairs naturally with the existing "plan code looks fictional" row in the
composition table, which covers someone else's plan but not your own.

## R-50 — The view is not the set: five errors, one shape

**Verdict:** miss → rule. Recon did not prevent these; they were caught downstream, each
by a different accident. The value is in the shape they share.

Five distinct wrong conclusions in a single session, each from a *view* of a set mistaken
for the set:

| what was read | what the view excluded | wrong conclusion |
|---|---|---|
| lock files filtered by a `kill -0` liveness check | a real lock whose process had just exited | "no lock ⇒ not indexing" |
| `pgrep … \| tail -1` | PID order is not start order; 17 other servers | "the config change did not take effect" |
| a SHA census matching 16-hex tokens | frontmatter `id:` values are also 16-hex | 80 experiments-only SHAs (really 49) |
| `ls \| wc -l` on the memories dir | `.anchors.toml` sidecars, two subdirs | "33 files vs 21 topics — a 12-file gap" |
| a tool's compact summary | ~54 KB of bodies in the buffer it named | "directory overview drops `include_body`" |

Every one produced a *confident, specific, checkable* claim. None was a vague guess — which
is the point: a filtered view yields precise wrong answers, and precision reads as rigour.
Three of the five were committed before being caught.

**The rule.** Before concluding from an enumeration, name what the view dropped. Four
recurring droppers, in rough order of how often they bite:

1. **A filter you applied** — and then reused the filtered output as if it were the source.
   The `kill -0` case: the filter was correct, reusing its output as the population was not.
2. **A cap or a truncation** — `head`, `tail`, `limit`, a preview, a hard byte budget. `tail
   -1` of an unordered list is a random element, not "the newest".
3. **A pattern matching more than you meant** — `[0-9a-f]{16}` matches commit SHAs *and*
   artifact ids; `*` matches sidecars and directories. Validate a token's identity before
   counting it (`git cat-file -e <tok>^{commit}`, `find -name '*.md'`).
4. **A rendering layer** — a summary, a compact form, a display-time transform. The
   codescout-specific instance: every buffered response reports `buffered_bytes` and names
   an `@ref`; if the byte count exceeds what the summary could account for, the summary is
   not the result. That number is the cheapest completeness check available and it is
   already in the response.

**Cheapest counters**, all one command: count the population two ways and compare;
validate one sampled token's identity; read the buffer rather than the preview; sort by the
key you actually mean rather than the one that is convenient.

Relation to kin entries: R-10 is the ancestor — "buffered artifact body parsed for
structured extraction without a completeness scout" — and R-50 generalises it from buffers
to every view. R-26/R-27 ("read ≠ verified") is adjacent but distinct: there the failure is
not checking at all; here it is checking a surface that answers a narrower question than
the one asked, which is harder to notice precisely because a check *did* happen.

**Evidence:** `bug-fix-session-log.md` F-38 + W-30 for the fifth row, which is the only one
with a code fix attached (`Symbols::json_path_hint` now advertises a body in the overview
shape, two tests). Rows 1–4 were corrected in `acd001fb`, this session's env-drift
investigation, the archived-bug SHA census, and a near-miss caught before reporting.

**Promote-when:** the count is the argument — five instances in one session is already past
any reasonable threshold, and the four droppers above are craft-shaped (true of any shell,
any tool, any language). Promote to `SKILL.md` Phase 1 as a scouting bullet: *"before
concluding from an enumeration, name what the view dropped — a filter you applied, a cap, a
pattern matching more than you meant, or a rendering layer. A filtered view yields precise
wrong answers."* The codescout-specific corollary (check `buffered_bytes` before trusting a
summary) is project-shaped and belongs in the `reconnaissance` memory instead.

---

## R-51 — Miss: an instrument that writes into the corpus it measures

**Date:** 2026-08-03 · **Verdict:** miss → rule · **Kin:** R-50 (complement), R-10 (completeness scout)

**Seam class (new): output-path ↔ measurement-domain overlap.** When an
analysis writes its artifacts *inside* the corpus it reads, the instrument
becomes part of its own subject. Nothing errors; the numbers simply start
describing the measurement.

**What happened.** The provenance probe (`scratch/provenance-probe/`) builds a
per-repo symbol vocabulary by walking every source file under each repo root.
Its own outputs — `sessions.json` (3.4 MB of session metadata), `vocab/*.json`,
`raw_results.json` — are written into `scratch/` **inside the codescout repo**,
and the walker's `SKIP_DIRS` covered build artifacts (`target`, `node_modules`,
`.venv`) but not the probe. codescout's document-frequency vocabulary inflated
35,496 → **221,214** distinct identifiers (6.2×) while `n_files` moved 1374 →
1383. Every downstream metric (M1/M2/M3/M5) is computed against that vocabulary.

**Why recon missed it.** The output directory did not exist when the walker was
written, so at the moment the seam could have been scouted there was nothing on
disk to scout. The overlap is created *by running the pipeline*, not by its
static shape — which is why a read-the-code scout would not have found it either.
It was caught only because an unrelated before/after sanity comparison made an
implausible ratio visible. No deliberate check was responsible.

**Relation to R-50.** R-50 says *name what the view dropped* — a filter, a cap, a
preview. This is its complement: **name what the view wrongly admitted.** Both
fail the same way (a conclusion drawn from a set whose membership was never
stated), from opposite directions. R-50 covers under-inclusion; R-51 covers
over-inclusion by self-reference.

**Proposal (SKILL.md Phase 1 bullet, if a second datapoint lands):**

> **Seam class: output-path ↔ measurement-domain overlap.** Before the first run
> of any pipeline that reads a corpus and writes artifacts, state where its
> output lands relative to what it reads. If the output path is inside the read
> path, assert the exclusion in the walker and log the excluded-path count on
> every run — a self-contaminating corpus produces no error, only wrong numbers,
> and the contamination grows with each run. Applies to indexers, vocabulary
> builders, corpus statistics, embedding jobs, and any `docs/`-writing analysis
> whose corpus includes `docs/`.

**Evidence:** `provenance-probe` session log F-2 (fixed-verified; vocabularies
rebuilt, DF back to 35,329 baseline, all 80-session results produced post-fix).
Secondary: F-3 in the same log is an R-50-shaped instance from the same session
(a pooled 3.0% compaction rate hiding 100% compaction in the largest sessions),
which is why the two rules are filed as complements rather than merged.

**Escalation 2026-08-04 — this is not a one-off.** Design review on the
provenance programme established that the seam is *permanent and by design* for
the system this probe was measuring, not an accident of where the scratch
directory happened to live. A provenance sidecar consumes the session tool-call
log and writes attribution records plus `Derived-From:` commit trailers; those
records then enter subsequent sessions' context and subsequent commits' history.
The instrument would sit inside its own corpus permanently. Second-order
corollary: a staleness check reading git history must not treat its OWN trailers
as derived-from evidence, or the system becomes self-confirming.

That is the second datapoint, and it is stronger than the first — the first was
a pipeline that accidentally read its own output, the second is an architecture
that necessarily does. Recorded as PV-27 in
`docs/trackers/provenance-subsystem.md`.

**Verdict:** miss → rule, **promote-ready**. Two datapoints (probe F-2
accidental; PV-27 by-design). The SKILL.md Phase-1 bullet above should now say
that the check applies both to where output *happens* to land and to whether the
system's own emissions re-enter its input — the second is invisible to a
directory-exclusion check and needs an explicit provenance-of-provenance rule.

---

## R-52 — An artifact's ownership is the union of its inputs' ownership

**Date:** 2026-08-04 · **Verdict:** miss → rule · **Kin:** R-51 (sibling, not amendment)

**Why a sibling and not an amendment to R-51.** The two failures are different and
fixing the first does nothing about the second. R-51's failure produces **wrong
numbers** — a pipeline reads its own output and the measurement is contaminated.
This one produces **misplaced data** — the artifact sits somewhere it should never
have been, and the numbers can be perfectly correct while it does.

**R-51's fix treats the symptom.** "Assert the exclusion at pipeline entry and log
the excluded count" makes the measurement correct while leaving the artifact in
the wrong place. The structural fix is one level up, and it makes the exclusion
unnecessary as a side effect: **a pipeline reading N corpora writes outside all
N.** An exclusion rule is what you need when the output path was chosen from
where the *code* lives rather than from what the code *reads*.

**The rule, in operational form — decidable before the pipeline exists:**

> An artifact's ownership is the **union of its inputs' ownership**. Choose the
> output location from the **input set**, never from where the code lives.

**What happened.** The provenance probe lived in `codescout/scratch/` because
that is where the session was running. It reads eight repositories, several of
them client work, and writes symbol vocabularies (~14 MB), path fragments and
prompt excerpts derived from all of them. By the rule above its output belongs to
eight owners and therefore to none of them — it should have been written outside
every input repo from the start. Staging caught it before anything was committed;
`/scratch/` is now gitignored with the reason recorded.

**The retrieval consequence, which R-51's framing does not reach.** F-2 fixed the
*measurement* consequence (the probe's own walker indexing its output). The
*retrieval* consequence went unchecked for thirteen rounds: codescout indexes the
project through a separate pipeline with its own exclusion rules, so
`semantic_search` over this repo returned client identifiers from `scratch/` to
any session working here — confirmed empirically, all six results for a probe-
specific query came from `scratch/provenance-probe/*.py`. That is the one route
where the data reaches a context **without a decision behind it**. Note codescout
behaved correctly: its indexer respects `.gitignore`, and `scratch/` was not
ignored. The defect was the output location, not the indexer.

**Third instance in a week of the same design-time move** — with R-53's
(PV-53's) "what can this metric not see?" and PV-58's "can these two things share
a domain?", all three are answerable from the *definition* of the artifact,
before any inspection of it. Reaching for the empirical version instead (measure
the mix; scan the output files) is what delayed all three.

**Corollary for the rebuild.** When the probe is rebuilt from its method
descriptions: paths parameterised, output outside every input repo, input set
**declared** rather than walked. The 42 hardcoded client paths in the scripts are
downstream of an output location nobody chose deliberately — fixing the location
fixes most of them by construction.

**Rule applied, not merely recorded (2026-08-04).** The gitignore closed the
COMMIT path and the INDEX path but left ~19 MB on disk inside the repo — still
reachable by `git add -f`, by backup/sync of the working tree, and by any tooling
that does not consult `.gitignore`. Per this rule the artifacts were relocated to
`~/.local/share/provenance-probe`, outside all eight input repositories.
Verified: every input repo clean of probe residue, repo working tree clean, new
location outside every input tree. The `/scratch/` gitignore stays as
defence-in-depth for the next probe that gets the location wrong.

Note the ordering this exposes: an exclusion rule is a *containment* measure and
contains only the paths it is consulted on. Relocation removes the artifact from
every path at once, which is why the structural fix is one level up rather than a
stronger version of the same fix.

**Verdict:** miss → rule, **applied**. Second datapoint for the R-51 family and
the first to separate misplaced-data from wrong-numbers.
## R-53 — A corpus's composition is a seam; census it by producer before measuring

**Class:** input-validity — the analysed corpus vs. the corpus you believe you
have. Sibling to R-50 (*the view is not the set*) and R-51 (*an instrument that
writes into the corpus it measures*). R-50 is about what an enumeration silently
**dropped**; R-51 about what a corpus wrongly **admitted** through your own
writing; R-53 is about what the corpus **contained all along that is not the kind
of thing you are counting**.

**The seam.** Every metric in a measurement programme depends on the shape of the
corpus, and the corpus is exactly the thing nobody reads this session — it is too
big to read, which is why it is being measured. That is the definition of a seam:
your next action depends on the current shape of something you have not read.
Scout it the same way you would scout a struct before editing it.

**Miss (2026-08-04, provenance probe, round 14).** Thirteen rounds of analysis ran
over a Langfuse corpus in which `chrome-devtools__take_screenshot` contributed
**59.8% of all tool-result bytes** — 71 calls, 22.04 MB, of which 71/71 payloads
were >90% base64 (median 100.0% of the payload inside one contiguous base64 run,
median whitespace 0.00%) and 37/71 were truncated at the flattener's 400 KB cap.
The pipeline called `flatten(tool_result['content'])`, i.e. `json.dumps`, so an
image content block became a giant text string and entered the token accounting.

Two consequences, and the second is the one that generalises furthest:

1. **Wrong denominator.** Base64 contains zero codebase-specific tokens by
   construction, so it inflated every share figure and depressed every utilisation
   figure *at the same time*. No individual number looked wrong. Reconciliation
   checks passed — they reconcile a numerator against a denominator that were
   contaminated together.
2. **Confounded sampling frame.** The sample was stratified into five bands by
   total context size, and base64 is what *made* those sessions large. Browser
   sessions by band: S 0/10, M 0/13, L 0/14, XL 1/14, **XXL 11/13**; browser share
   of tool bytes 0% / 0% / 0% / 19.9% / **81.2%**. So every "as context grows, X"
   finding from rounds 8–13 was reading a **producer** axis wearing a
   **magnitude** label. On clean sessions the trend inverts: utilisation goes
   19.0% → 35.7% → 32.8% → 31.5% across S–XL, then 6.2% at n=**2**. The decay was
   the top band, and the top band was one tool.

**Cost.** Reversed the programme's sole remaining build decision. The intervention
(a PostToolUse hook on `mcp__.*` triggering at 32 KB) had been justified by "137
calls carrying 61.1% of information-bearing tokens"; after exclusion, **zero**
non-browser MCP calls in the entire corpus reach 32 KB. Trigger population empty,
not merely smaller.

**The scout, and it is cheap.** Before any metric, print one table: per producing
tool — call count, total bytes, share of bytes, count above your threshold of
interest, and number of distinct sessions. Roughly fifty lines of code, ten
minutes. Read it with two questions:

- *Does any single producer exceed ~20% of the bytes?* If yes, that is a validity
  question before it is a distribution feature. Ask what those bytes **are** —
  character-class statistics are enough (base64 runs, whitespace fraction) and
  require reading no content, which also keeps the check privacy-safe.
- *Is that producer concentrated in one stratum?* If yes, your stratification
  variable is partly that producer, and every trend along the axis is suspect.

**Why no internal check found it.** Thirteen rounds included null models, domain
matching, byte-total reconciliation, and sustained adversarial review by an
independent session that reversed four conclusions. All of them operate *inside*
the frame the corpus defines. Corpus-composition errors are outside it by
construction. The person who can see them is whoever knows the provenance of the
inputs — here, the owner, whose entire contribution was one sentence saying the
browser traffic was not normal flow. Pair this rule with that habit: when someone
with provenance knowledge makes an offhand representativeness claim, census first
and argue after (session-log W-9).

**Kin.** R-50 (the view is not the set) — same family, different position: R-50
guards the *listing*, R-53 guards the *substrate*. F-10 in the same probe found a
classifier's catch-all default read as a category; R-53 is that error one level
down, in the data rather than in the labels. PV-25 (pin the unit before the
threshold) is the metric-side twin — R-53 is the corpus-side one, and this
programme did the first carefully and never did the second.

---

## R-54 — A row is not an observation until you have checked the unit and the nesting

**Class:** input-validity, sampling-frame layer. Third in the family: R-50 guards
the **listing** (what did the view drop), R-53 guards the **substrate** (what is
the corpus made of), R-54 guards the **row** (what does one record mean, and are
two records independent).

**The seam.** Between a table's row count and the number of *things* you are
studying sits a mapping nobody reads: rows-per-thing, and whether rows overlap in
content. It is a seam because every downstream statistic depends on it and it is
never visible in the statistic itself — a mean over nested rows is a perfectly
well-formed mean.

**Miss (2026-08-05, provenance probe, round 15).** The Langfuse arm treated
`observations` rows as sessions. They are **requests**. Because each request
re-sends the whole conversation, the corpus averaged **143 rows per session**, and
summing row lengths double-counted content **165×** (34.8 GB summed vs 0.21 GB of
distinct final contexts). Consequences, all invisible to internal checks:

- **Effective n collapsed.** 64 sampled rows = **34 distinct sessions**. The top
  band's 13 rows = **6 sessions**; its 11 screenshot-bearing rows = **4
  conversations**. The de-duplication in place was `if L in seen_len` — distinct
  *byte length* — which cannot dedup at session grain, because different turns of
  one conversation naturally differ in length. It looked like a dedup and was not
  one.
- **A stratification axis meant something else.** Rows were banded by size, but a
  session *traverses* every band as it grows, so the bands were
  **conversation-depth** strata wearing size labels. Sampling the top band meant
  sampling late turns of long conversations.
- **An absence claim over-generalised.** "Zero non-browser MCP calls ≥32 KB" was
  true of 34 sessions and reported as *the trigger population is empty*. At full
  population it is **2 calls / 0.11% of tool bytes**. The recommendation held; the
  claim did not.

**Cost.** A headline reported and committed one day was falsified the next, and
every pooled magnitude from six prior rounds had to be superseded.

**The scout, and it is three queries.** Before any statistic:

1. `count(*)` vs `uniqExact(<thing_id>)` — rows per thing. If the ratio is not
   ~1, your rows are not observations.
2. `sum(len)` vs `sum(max(len) per thing)` — the double-count factor. A large
   ratio means rows nest, and every sum is inflated.
3. After sampling, `uniqExact(<thing_id>)` **on the sample** — the effective n.
   Report this number, not the row count.

Then one discipline in prose: **an absence claim carries its denominator.** Write
"zero in 34 sessions", never "empty". The qualified form invites the question that
finds the defect; the unqualified form silently promotes a sample statement to a
population statement, and nothing downstream can catch the promotion.

**Why no internal check found it.** Nesting duplicates *real* content — the bytes
are genuinely in both rows — so reconciliation, byte totals, and null models all
pass. Adversarial review shares the analyst's frame and cannot see it either. The
person who can is whoever knows how much work the data represents. Here it took
one sentence from the corpus owner: *we should have more than 64 sessions, much
more.* Pair with R-53 and session-log W-9/W-10: **show the owner the corpus census
and the sampling frame before you show anyone the findings.** Both reversals in
this probe lived in those two artifacts, and both were readable in under a minute.

---


## R-55b — Miss: a host-specific symptom accepted as the norm without sweeping the other hosts

**Verdict:** miss (caught by a later sweep, not by the scout that should have run first)

2026-08-08, reviewing a bug file that reported 8 permanent `??` entries under `.codescout/projects/` and framed them as "the normal state for the cross-project flows CLAUDE.md documents". The framing was accepted long enough to build a re-diagnosis on top of it.

The cheap check was a sweep of the population the claim generalises over. `~/.config/librarian/workspace.toml` declares 10 `[[roots]]` plus two umbrellas — 15 paths. Nine of those repos have a `.codescout/projects/` tree; **all nine had zero untracked entries**, via three different mechanisms (nothing generated lands there; a structural allow-list; a blanket `.codescout/` ignore). The reported host was an outlier, not the baseline.

**The trap that made "zero" misleading in the other direction:** two of the nine carried `projects/<id>/` directories that were **empty**, and git cannot see an empty directory — so a clean `git status` was not evidence about their contents either way; `find -type d` was needed to see them at all. That part of the lesson holds.

**Corrected 2026-08-08 — those two were labelled "phantoms" on a bad test.** See R-57. `claude-plugins/…/mcp-server` is a **legitimate** sub-project directory (`root: session-bridge/mcp-server`); `eduplanner-site/…/optaplanner` is undetermined. The rule above survives because it rests on empty-directory invisibility, not on those two being defects; the illustration was wrong, the rule was not.

**Rule:** when a report's evidence is `git status` on one machine, (a) sweep the other members of whatever population the claim generalises over before accepting the framing, and (b) do not read a clean `git status` as absence — an empty directory is invisible to git, so census the filesystem, not the index.

**Promote-when:** a second instance where a single-host symptom was generalised without a sweep. Pairs with R-38 (a concurrent session had already measured it) — both are "widen the evidence base before theorising".

## R-57b — Miss: an identifier's shape says nothing about whether the thing exists — check its declared root

> **Id split 2026-08-16.** This entry reused a number already taken by a
> different lesson. `R-57` (index row, 2026-08-06) is *"when the seam is a TOOL,
> the scout is one real invocation whose output you read"*. This one is `R-57b`.
> A citation of bare `R-57` written before 2026-08-08 means the other.

**Verdict:** miss (caught by an unrelated tool response an hour after the claim was committed and published)

2026-08-08. Investigating unvalidated `project_id`, two directories were classified as phantoms
by this test:

```
ls -d <repo>/<id>     # not found  =>  "<id> is not a project"
```

The test is invalid, and not merely unlucky. **A sub-project's id is its directory basename, not
its path.** A project declared at `session-bridge/mcp-server` gets the id `mcp-server`, so
`ls -d <repo>/mcp-server` fails while the project is entirely real. The check carries no
information about existence.

It surfaced by accident: `workspace(action="activate", path=claude-plugins)` prints its project
list, and there it was — `{id: "mcp-server", root: "session-bridge/mcp-server", languages:
["rust"]}`. One call, available from the start, never made.

**Rule:** to decide whether an id names a real project, read its **declared root** —
`project_status`, or the `workspace` array that `workspace(action="activate")` already returns.
Never infer existence from where the id would sit if the naming scheme were positional. More
generally: when a claim is "identifier X does not correspond to anything", the evidence must come
from the registry that owns X, not from a filesystem probe at a path you derived yourself — the
derivation is the assumption under test.

**Cost:** a wrong claim in a committed bug file, two tracker entries, and a public PR comment;
two empty directories deleted on a wrong premise (harmless — one regenerates, the other was
already inert). The underlying bug was never in doubt: it was reproduced directly with ids
invented for the purpose, and that evidence never depended on these two.

**Promote-when:** immediately, if a second existence-claim is made from a self-derived path.
Pairs with R-26 (a grep line-match locates a symbol, it does not confirm a mechanism) — same
family: a real observation at a location you chose, mistaken for evidence about the thing you
were actually asking about.

## R-58b — Hit: before fixing a heuristic, grep for other copies of it — `references()` cannot see a duplicated closure

**Verdict:** hit

2026-08-08, fixing the buffer-only classifier's path-likeness rule
(`docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md`). The bug file named
one site: the closure inside `OutputBuffer::resolve_refs`. Editing it looked complete.

A `grep buffer_only` run to find a test harness surfaced a **second** implementation —
`OutputBuffer::is_buffer_only`, a public function with its own copy of the same rule. The
two had already diverged:

| copy | splitter | `../` |
|---|---|---|
| `resolve_refs` closure | `shell_words` (quote-aware) | **no** |
| `is_buffer_only` | `split_whitespace` (quote-blind) | yes |

So the gate answered differently depending on which entry point asked, and fixing only
the named site would have left the public one — the one other modules call — unfixed and
still quote-blind.

**Why the usual tools miss this.** `references(symbol)` finds callers of a *named* symbol.
An inlined closure has no name, so a duplicated rule expressed as a closure in one place
and a function body in another is invisible to caller enumeration in both directions. The
only detector is a text grep for the *rule's distinguishing tokens* — here
`starts_with('/')`, `contains('/')`, or the concept name `buffer_only`.

**Rule:** when a bug names one site of a *heuristic* (as opposed to a function call),
grep the tree for the rule's distinguishing literals before editing, and treat any second
copy as already-diverged until diffed. Then extract the predicate so the copies cannot
drift again — two copies of one rule is duplication, not two implementations, so
rule-of-three does not apply and extraction is earned immediately.

**Promote-when:** a second instance where a duplicated *inline* rule was found by literal
grep after `references()` came back complete. Closely related to the codescout memory
`platform-law-leaks-at-call-sites` ("grep the whole tree for the OLD pattern, not the
declaring module") — this is that law applied to a heuristic rather than a subprocess
call, where the tell is a code shape rather than a syscall string.

## R-75 — A process-level env scrub is not configuration isolation

**Observed:** 2026-08-13, verifying end-to-end that the local ONNX embedding path runs
offline, against the release binary rather than the test suite.

**Verdict:** miss → rule.

**What happened.** The probe ran under `env -i` with only `HOME`, `PATH`, and three
explicit `CODESCOUT_*` variables, including `CODESCOUT_EMBEDDER_MODEL=local-dir:<weights>`.
That was treated as sufficient isolation. It was not: `main.rs` calls `load_startup_env`
(`src/config/global.rs:118-154`), which reads `~/.config/codescout/.env` **from disk** — on
this host a symlink to the repo's own `.env.amd`. Dotenv precedence is "already-set wins,
unset keys are filled", so the scrubbed `CODESCOUT_EMBEDDER_URL` was restored, and
`build_embedder` then discarded the `local-dir:` model in the url's favour.

**Why it was not caught immediately.** Every observable said success: exit 0,
`added=5 updated=0 deleted=0 elapsed_ms=118`, no warning, no error. The run *looked*
exactly like the run that would have proved the feature works. The discriminator was not in
the output at all — it was in the artifact: the `vec0` table read `FLOAT[768]`, and
AllMiniLM-L6-v2 is 384-dimensional. A dimension is a fact the pipeline cannot fake.

**Why the usual scout step missed it.** The reconnaissance substrate rule already says to
read a tool's `loaded N from X` preamble and reconcile it. Here there was no preamble to
read — config loading is silent by design — so the rule had nothing to attach to, and
"I removed the variables myself" felt like stronger evidence than any preamble. The missing
move is a **negative control**: make the suspected config source provably absent and check
that the run's behaviour *changes*. Pointing `CODESCOUT_ENV_FILE` at a nonexistent path
immediately produced a different result (`local weights: descended into the sole snapshot
directory`, `FLOAT[384]`), which is what identified the substrate.

**The knowledge already existed in the repo.** `tests/cli_artifact.rs:18-24` neutralises
this exact trap, with a comment explaining why. It was not consulted before trusting the
result — the same failure shape as R-59 (an artifact that presents as prior reconnaissance).

**Rule.** Before believing an "isolated" run, enumerate every layer that can supply
configuration — process env, startup dotenv, user config, project config, compiled default —
and neutralise or observe each one. Then verify **the artifact the run produced** (a stored
dimension, a written row, a file's bytes), not its exit code. An exit code is the program's
opinion; the artifact is what it did.

**Note the compensation, and its limit.** The flawed probe still paid off — chasing an
impossible dimension is what exposed a genuine defect (`docs/issues/2026-08-13-url-silently-overrides-local-dir-model.md`,
fixed `38e0980b`). But that was luck in the choice of what to inspect. Had the dimension
gone unread, this session would have filed "local ONNX path verified end-to-end" on the
strength of a run that never loaded the weights.

**Promote-when:** a second instance where an isolated-looking run was reconstituted by a
disk-read config layer. At two datapoints this belongs in `SKILL.md` Phase 1 as an explicit
bullet beside the existing substrate rule, phrased as the negative control rather than as a
list of config layers (the layers are project-specific; the control generalises).

**Kin:** R-50 (the view is not the set — same shape at the config layer: concluding from an
enumeration that had silently readmitted something), R-6 (scout the substrate before
mechanism design), R-59 (a repo artifact that already knew the answer, unconsulted).

## R-87 — Hit: before designing an abstraction, scout for the dispatch point that already exists

**Observed:** 2026-08-15, SD-1b. Asked how to generalise doc-ref extraction
across "the other supported languages" and how to "abstract it away".

**The scout, in three calls.** `grep tree-sitter Cargo.toml` → nine grammars
vendored. `grep 'LANGUAGE.into()'` → one dispatch site,
`src/ast/mod.rs::get_ts_language`, whose own doc comment reads *"the single
source of truth for tree-sitter language resolution. Both the AST parser and
the embedding chunker use this function."* `grep 'fn detect_language'` → the
extension→language half, with a `detect_language_vs_get_ts_language_contract`
test already keeping the pair honest.

**Verdict: hit.** The abstraction existed, documented, with two consumers and a
contract test. The feature became its **third consumer** — zero per-language
code — rather than a fourth language mapping to keep in sync. Designing before
scouting would have produced a `trait CommentExtractor` with an impl per
language: one-implementor bureaucracy, and precisely what this project's
`tool-registration-rule-of-three` memory exists to prevent.

**The generalisable move:** an "abstract across N variants" request is a
*seam*, and the far side is whether the variants already resolve through one
place. Three greps answer it: the dependency manifest (how many variants are
there), the `.into()` / registry / match site (is there one door), and a
contract test (does anything hold the halves together). A codebase that has
solved this once usually says so in a doc comment — `get_ts_language`
literally did.

**Two things the scout did NOT settle, and both mattered:**

- *The uniformity assumption underneath the abstraction.* Comment node kinds
  differ per grammar (`line_comment`, `comment`, `block_comment`), and the
  in-tree evidence covered only **five of nine**. `kind().contains("comment")`
  unifies them, but that was a hypothesis until a test drove all ten language
  keys. **Scouting a dispatch point tells you the door exists, not that
  everything behind it has the same shape.**
- *A variant that documents differently in kind.* Python's docstrings are
  `expression_statement > string`, not comments at all — a comment-only
  extractor returns its `#` notes and silently drops every docstring. The
  scout found the dispatch; the user caught the shape. Worth adding to the
  seam checklist: for any "all N languages" feature, ask which language does
  the thing *differently in kind*, not just differently in name.

**Evidence:** `experiments:450880c7`. W-42 in
`docs/trackers/bug-fix-session-log.md`.

**Verdict:** hit — promoted-when a second "abstract across variants" request is
answered by finding the existing dispatch. At two datapoints this belongs in
the reconnaissance skill file (in the codescout-companion repo, so not a path
resolvable from here) as a named seam class: *the variant-dispatch seam*.

## R-88 — Hit: the instrument that nominates a refactor group also fixes its axis, and that axis can be orthogonal to the real duplication

**Verdict:** hit — the scout falsified the group's premise and located the actual
duplication, which a live A/B measurement then confirmed as a defect.

**Seam:** four `::call` handlers nominated by `legibility_scan` as tier-1
over-budget bodies, proposed in `docs/trackers/structural-debt-refactor.md` SD-3
as the source of a shared extraction.

**What the scout did.** Read all four bodies in full before proposing anything,
smallest first (`src/librarian/tools/update.rs`, 272 lines; then
`src/librarian/tools/find.rs`, 327) to form the hypothesis, then the two the
tracker had already characterised in detail (`src/librarian/tools/context.rs`,
379; `src/librarian/tools/get.rs`, 482) to test it. Reading
`src/librarian/tools/get.rs` first would have anchored the scout on a shape the
tracker had already described.

**Finding.** The four do not share a phase structure. They are two pairs on an
axis the nomination cannot express: `find`/`context` are query handlers,
`get`/`update` are single-artifact handlers. Following the shared-looking
fragments outward with `grep` instead found the real duplication — a
scope-resolution prologue written three times, one copy drifted — and its third
site is `src/librarian/tools/workspace_state_at.rs`, a file comfortably under the
body budget that `legibility_scan` never flagged.

**The cross-cutting lesson.** `legibility_scan` ranks by symbol body size and
observed per-symbol cost. That makes it structurally capable of seeing only
duplication *within* one symbol; a law duplicated *across* symbols in different
files is invisible to it by construction, and the files holding the other copies
need not be large. So a group it nominates is a list of *expensive symbols*, not
a list of *related symbols*, and the two are easy to confuse because the output
looks the same either way. This is the same class as SD-3's own recorded caveat
(identical cost tuples across four distinct symbols mean attribution is coarser
than per-symbol) — one level further out.

**Rule to apply next time.** Before extracting from any instrument-nominated
group: (1) read every member, not the highest-ranked one; (2) for each block that
*looks* shared, grep its most distinctive token across the whole tree rather than
across the group — the decisive third copy is the one the instrument did not
nominate. Both steps are cheap; step 2 is what turned this from a falsified
hypothesis into a filed defect.

**Evidence:** `docs/trackers/bug-fix-session-log.md` W-43 (full narrative and
counterfactual); `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`
(the defect); `docs/trackers/structural-debt-refactor.md` SD-3 → SD-10.

**Promote-when:** a second instance where an instrument-nominated group's axis
turns out orthogonal to the duplication, and the decisive site sits outside the
group. At two, promote step 2 into this skill's Phase 1 as a named seam class
(*instrument-nominated group*), since it is craft-shaped — it holds for any
ranking tool that scores symbols independently, not just this repo's.

## R-76b — Aggregate behaviour data is a screen, not a verdict

**Observed:** 2026-08-15, auditing codescout's tool surface across 13 merged `usage.db` corpora
(53,916 calls, 460 sessions).

**Verdict:** miss ×2 → rule.

**What the aggregate said.** Failures were ranked by frequency and by a sequence measure — for each
error, what tool came next and did it succeed. That produced a clean-looking ranking, two filed bugs,
and a confident story about which guards were healthy.

**What reading ~12 rows' actual arguments showed.** The aggregate was wrong in both directions.

*Overstated.* Every rejection of an unsupported `json_path` wildcard was scored a successful
recovery, because the following call returned `success`. The arguments showed what those recoveries
actually were: `$.items[*].abs_path` → `read_file(lines 92-485)`, 393 lines of raw JSON to obtain one
field; `$.entries[*].id` → `$.entries[4].id`, one element per call; another → abandon the buffer and
re-run the upstream query; another → grep the buffer with a regex. **Not one recovery got what was
asked for cheaply.** `outcome='success'` records that a call returned, never that the agent got what
it wanted — and those are different questions.

*Understated.* The opposite error on a different guard. Its "35% same-tool recovery" counted only a
retry of the same tool; the traces showed the correct recovery is a **different** tool, and that
agents took it and got exactly what they wanted. Two related metric traps, both systematic:

- **same-tool recovery** penalises every guard whose right answer is another tool — which is what a
  routing guard IS;
- **adjacent-call compliance** penalises every guard whose correct recovery needs a lookup first.
  Widening one route's window from 1 call to 3 moved compliance from 45% to 74%. The 45% figure
  would have justified a redesign the 74% figure shows is unnecessary.

*And the sharpest defect was invisible to every aggregate.* Bucketing the refused ranges by shape
showed 69 of 244 were canonical imports reads (`start_line=1, end_line=20`), refused by a guard whose
recommended alternative — a definition projection — structurally cannot return imports. No amount of
counting errors or sequencing tools surfaces that; it required looking at what was asked for.

**Why recon did not fire on its own.** The audit *felt* like measurement rather than a seam: it was
read-only, quantitative, and produced numbers. But a behavioural corpus is a **substrate**, and
reading a derived statistic from it is reading a verdict, not the world — the same shape as R-50's
"the view is not the set", and as R-75's "verify the artifact produced, not the exit code". The
distinguishing tell here: **the conclusion was about intent** ("the agent recovered") while the data
only contained shape ("a later call returned success"). Whenever a claim's subject is intent and the
columns are mechanics, the gap must be closed by reading instances.

**Also: check whether the intent-bearing field is even recorded.** This audit initially concluded the
arguments were unavailable, because the capture is behind a `--debug` flag in the source. It was
wrong — 95% of rows had them, because every server on the host runs with that flag. A capability
read from code as "gated" may be live in deployment; check the running process, not only the branch.

**Rule.** Before filing or closing anything from aggregate behaviour counts, read the arguments of
about ten instances. Both corrections here came from fewer than a dozen rows and neither was visible
from any amount of aggregation. Use the aggregate for **ranking where to look**; use instances for
**deciding what is true**.

**Promote-when:** a second investigation where instance-reading reverses an aggregate conclusion. At
two datapoints this belongs in `SKILL.md` Phase 2 (Compare) as an explicit bullet — phrased as the
intent-vs-mechanics tell rather than as "read some rows", which is too weak to act on.

**Kin:** R-50 (the view is not the set), R-75 (verify the artifact, not the exit code), R-59 (an
artifact that presents as prior reconnaissance).

## R-89 — Miss ×3: a tool's output is evidence about the code only if the running build contains it

**Verdict:** miss — the same unchecked premise cost three separate conclusions in
one session before it was named.

**Seam:** any claim of the form "the code does X" backed by an MCP tool result.
The tool runs inside a server process built at some unknown past moment; the
source on disk and the code producing the answer are two different things, and
nothing in the response says so.

**What it cost, three times.**

1. **A gate that had never run.** SD-1b extended `audit_doc_refs` to scan code
   comments. For the rest of that session every audit went through a binary
   predating it, scanning **958 files instead of 1,402** — markdown only. The
   feature was reported as shipped and verified while never once executing. The
   discrepancy was visible in the file count the whole time and read as noise.
2. **A repo-wide number quoted from a degraded scan.** `broken: 10130` was taken
   while `scan_meta.degraded == true` with rust offline, then nearly used to size
   a sweep. Scoped scans of the same tree came back non-degraded — so the
   degradation is a *scale* artefact, and repo-wide counts are systematically
   less trustworthy than subset ones, which is the opposite of the intuition that
   a bigger scan is a better measurement.
3. **A verification plan that could not work on the host it was written for.**
   The `#77` sqlite-vec fix was to be confirmed by watching `$HOME` stop growing.
   But `cargo rb` builds `--features server-stack`, and its own alias comment says
   that makes `VectorBackend::resolve()` return Qdrant — so the sqlite path is
   never exercised here and the counter could not move whatever the code did. A
   zero would have been read as success.

**What actually settled it — a discriminating probe, not metadata.** Binary mtime
and `git merge-base --is-ancestor` establish *when* and *from what commit*, which
is weaker than it looks: it says the source was present, not that the feature is
reachable. What worked was picking an input whose result differs between the two
candidate builds and running it:

- *Does a bare prose path get extracted?* Only post-SD-1b. It did not → binary was
  stale, conclusively.
- *Does scanning a `.rs` file return any refs at all?* Only with the code globs. It
  did → binary was current.

Each is one call and admits no interpretation.

**The environment has the same problem.** `env | grep CODESCOUT_...` reads *this
shell*, not the server, which self-loads `.env` at startup. Checking the server's
actual configuration meant reading `.env` and `.cargo/config.toml` — the alias
comment in the latter is what finally explained the backend.

**Rule to apply next time.** Before citing tool output as evidence about code you
have edited this session: (1) run a probe that can only succeed on the new build;
(2) read `scan_meta.degraded` and treat a degraded result as an upper bound with
unknown noise, never as a count; (3) if the claim depends on configuration,
resolve it from what the *server* loads, not from your shell.

**Evidence:** `docs/trackers/bug-fix-session-log.md` W-41 (live-verify after every
rebuild) is the parent rule; this is the sharper form — *verify the build, not
just the behaviour*. W-44 is the sibling failure at population grain rather than
build grain.

**Fourth instance — the Promote-when fired the same day, from another session.**
Added 2026-08-16, hours after this entry was written; the heading's `×3` is the
original count. Session `28ea039a` committed `5917e37e`, which adds an
ALWAYS-VERIFY imperative to `src/prompts/guides/project-activation-bootstrap.md`,
and then reported to the user that the guidance was shipped and delivered once
per session at first activation. That is a committed claim resting on a
stale-build assumption, and it was wrong at the runtime layer: the guide body is
`include_str!`'d into the binary (`src/prompts/mod.rs:160`), so the text a session
receives is fixed at *build* time and frozen again at *process start*. The
following session's own auto-injected bootstrap guide arrived **without** the new
paragraph.

What makes it a clean confirmation is that metadata pointed the wrong way and the
probe pointed the right way — exactly the asymmetry this entry describes. Binary
mtime (10:56) was *newer* than the commit (09:15), which reads as "the fix is in."
It was: `grep -c "Do not hypothesise" target/release/codescout` → `1`. The build
was fine. The stale layer was the **process**: this session's server is PID
773774, started 09:28, i.e. before the 10:56 build that first compiled the string.
A second, independent probe settled it without ambiguity — `json_path="$.items[*]"`
was still refused, though `[*]` support landed in `7c91cdf7` at 10:50. Two probes,
two different subsystems, same verdict.

**This widens the rule.** The parent form is *verify the build, not just the
behaviour*. The sharper form is: **build freshness and process freshness are two
separate facts, and mtime answers neither.** A long-lived MCP session can outlive
any number of rebuilds; `cargo rb` alone changes nothing until `/mcp` reconnects,
which is precisely why memory `development-commands` pairs them. For anything
`include_str!`'d — every guide, every prompt surface — the honest ship claim is
"committed and compiled," and "live" needs a fresh process plus one probe.

**Promote-when:** ~~one more instance~~ **FIRED 2026-08-16** (fourth instance
above — a committed claim, from an independent session, on the same day). Promote
the probe technique into the skill's Phase 1 as a named step for any session that
edits a tool it also uses — the normal condition in this repo, and the reason the
failure recurs. The Phase-1 step should name both freshness axes: run a probe that
can only succeed on the new build, and confirm the serving process postdates that
build.


## R-90 — Miss ×2: two sessions, one working tree — `git add -A` silently annexes the other's staged work

**Verdict:** miss, twice in one day. Both times the content survived and the
*reason* did not.

**Seam:** any `git add` / `git commit` in a repo where another agent session is
live. The index is process-shared state, and nothing in `git status` says who
staged a line.

**What it cost.** On 2026-08-16 a concurrent session's `git add -A` swept this
session's staged, unrelated work into its own commit, twice:

- a filed bug file (`librarian-runtime` doc-vs-code) landed in `618acd57`,
  *"retract the benchmark ground-truth claim"*;
- a guide dedup cut (D1) landed in `148aabe6`, *"count non-empty segments, so
  bare // /// //! stop being paths"*.

Each commit is correct about its own content and silent about the annexed
change. Nothing is lost in the working-tree sense — the files are in HEAD. What
is lost is the **commit message**: D1's containment proof and its byte-realisation
measurement were written and never committed, and had to be re-recorded in a
tracker row (`docs/trackers/prompt-hamsa-audit-log.md`, A-22) afterwards. The
durable damage is attribution: `git log -- <path>` now points a later reader at
an `audit_doc_refs` commit to explain why a guide section was deleted.

**Why this is a recon miss and not bad luck.** The state was checkable before
acting, both times, and was checked in the wrong window. `git status` *was* run
and *did* show the files staged — but staged-ness was read as "mine, awaiting my
commit" rather than "in a shared index that another process may commit at any
moment." The seam is not the file. It is the index, and the index carries no
ownership marker.

**Rule to apply next time.** In a repo where a second session may be live:

1. Stage and commit in one call; never stage and then go do other work.
2. **Use `git commit --only <paths>`.** This rule was wrong twice before it was
   right, and both corrections came from executing it rather than re-reading it.

   - *First draft:* "prefer `git commit <paths>`." Fails on new files —
     `pathspec ... did not match any file(s) known to git`, because commit-by-path
     resolves against the index and an untracked file is not in it.
   - *Second draft:* "chain `git add <paths> && git commit`." This is worse than
     useless: it narrows the staging window but `git commit` with no pathspec
     commits **the entire index**, including whatever the other session staged.
     Following it, this session committed a concurrent session's file rename into
     `543086d1`, a commit about tracker policy — becoming the perpetrator of the
     exact hazard this entry records. The hazard is symmetric, and "stage only my
     paths" does not make the commit mine.
   - *Correct form:* `git add <paths> && git commit --only <paths>` (or `-o`).
     `--only` commits exactly the named paths and ignores the rest of the index,
     which is the isolation the first two drafts were reaching for.

   The general lesson is not about git. **An advice entry nobody has executed is
   a hypothesis**, and this one read as obviously right through two wrong
   versions. Both were caught within the hour only because the entry was being
   used, not reviewed — which is the argument for harvesting `Promote-when`
   criteria on a schedule rather than on inspiration.
3. Read an empty `git status` after your own staging as evidence that *someone
   else committed*, not that your commit succeeded. `git log -1 --format=%s --
   <path>` names who took it.
4. The structural fix is a per-session worktree. The repo already supports it
   (`.worktrees/`) and the librarian has overlay + `merge_worktree` semantics
   built for exactly this.

**Evidence:** commits `618acd57`, `148aabe6`; A-22's outcome field carries the
rationale the second sweep displaced.

**Promote-when:** a third instance, or one where the annexed change is not merely
mis-attributed but semantically wrong in its host commit — a revert that also
reverts the annexed work, say. At that point the worktree split stops being
optional and becomes the default for concurrent sessions.
## R-91 — A probe that cannot observe the thing the claim is about (three instances, one session)

**Verdict:** miss (×3) · **Observed:** 2026-08-16, benchmark + tracker-hygiene session

Three separate wrong claims in one session, all the same shape: a **real measurement**
attached to a conclusion the measurement was **structurally incapable of observing**. None
was a careless reading; each probe returned exactly what it was asked, and each question was
the wrong one.

| # | probe run | claim attached | why the probe could not see it |
|---|---|---|---|
| 1 | `ls` / existence check over the **working tree** | "`run-tc-benchmark.py` cites 5 deleted files, so TCs are unpassable" — **filed as a bug** | the harness scores against a corpus **pinned at `ede25e69`**, never against HEAD. All 5 exist there; 851/851 baseline files present |
| 2 | `rocm-smi ... \|\| nvidia-smi ...` | "the log spans **three** machines; the A5000 is a separate NVIDIA host" — **written into a commit** | `\|\|` short-circuits. ROCm answered, so the NVIDIA branch **never ran**. Both cards are in the same box; there are two machines |
| 3 | read `scan_meta.lsp_languages_offline: ["rust"]` | "rust-analyzer is offline, discard this audit" | the field is a **claim about a dependency**, not a probe of it. `references()` answered from that same LSP seconds later |

Cost: one bug filed and retracted the same day, one committed tracker entry corrected the
same day, and one valid 40-file audit nearly discarded. Instances 1 and 2 both reached
**committed artifacts** before being caught — this class does not stop at the draft.

**The rule.** Before attaching a conclusion to a measurement, say in one sentence what the
measurement can and cannot see. Three concrete forms:

- **Pinned/configured corpora:** when a tool reads a corpus it selects itself (a pinned
  worktree, a configured collection, a baseline SHA), verify against *that* corpus. The tree
  you are standing in is not evidence about it.
- **Never probe for presence with `A || B`.** Short-circuit makes absence of B
  unobservable whenever A succeeds. Run both, unconditionally, when the question is "which
  of these exist".
- **A tool's self-reported health field is a claim, not a measurement.** Probe the
  dependency directly before believing it.

**Discriminating-probe technique, worth reusing.** Instance 3 was settled by picking a call
that *requires* the dependency: `symbols()` is tree-sitter-backed and succeeds whether or not
the LSP is up, so it cannot distinguish the states; `references()` needs the LSP and can.
One round-trip. When two hypotheses predict the same output from your current probe, the
fix is a different probe, not a longer stare — the operational form of the Iron Law's
"evaluation means new information".

**Relation to R-89.** R-89 says a tool's output is evidence about the code only if the
running build contains it. R-91 is its sibling and strictly wider: *evidence about anything
requires that the probe could have observed the alternative.* R-89 is the build-grain case;
instance 1 is the corpus-grain case; instance 2 is the shell-operator case.

**Promote-when:** a fourth instance, OR one instance where the rule is applied
prospectively and prevents a claim. At that point promote the three concrete forms to the
`reconnaissance` memory topic — they are craft-shaped, not project-shaped, so the global
SKILL.md is the eventual destination via the sync flow.


## R-92 — A filed root cause is a hypothesis, and confirming it usually widens the bug

**Verdict:** hit (×2) · **Observed:** 2026-08-16, fixing the two tool-quirk bugs the
2026-08-16 hygiene sweep filed

Both bugs were filed with their root cause explicitly marked unverified — `6c0952e8`:
*"Inferred from the two error messages, not read from source"*; `49e562fc`: *"Unknown — not
yet traced to a line."* Honest filings. Scouting each before writing code confirmed both
inferences **and found both had understated the defect.**

| bug | filed as | what the source said | scope change |
|---|---|---|---|
| `6c0952e8` | `artifact(update)`'s hint misroutes **`kind`** | shared validator `reject_reserved_extra_keys`, one hardcoded hint written for `create` | `update` has **no** top-level reserved-key parameters at all — its six settable ones live inside `patch`. The hint misrouted **7 keys**, not 1 |
| `49e562fc` | mechanism unknown; suspected `degraded` over-fires | one writer, `note_degraded`, **three** call sites — only one is "offline", and the middle fires on a branch whose own comment says *the server ANSWERED* | `degraded` was never wrong; the coverage really was incomplete. Only the **word** was false. The fix moved from "stop over-firing" to "carry the cause" |

The second row is the sharper one. The filing's suspected direction — that `degraded`
itself over-fires — would have **undone a deliberate earlier fix** (`2026-08-06` added that
branch precisely because a mid-index server silently costs 60-69 resolutions). Reading the
code before acting is what kept a correct behaviour from being reverted as a bug.

**Why widening is the norm rather than luck.** An inferred root cause is inferred from the
one symptom the reporter hit. The reporter reached for `kind` because bug files carry
`kind: bug`; nothing prompted them to try `status`. The filed scope is therefore a lower
bound *by construction*, and the scout's question is not only "is this true?" but **"is
this all of it?"**

**Second finding — a fourth shape for Law D.** Neither bug had a test that could have
caught it, and both had a test sitting over the exact function:

- `create_rejects_an_extra_key_…` and `update_rejects_an_extra_key_…` both assert only
  that the message **names the clashing key** — which the wrong hint also did.
- `audit_doc_refs`'s suite asserted that `degraded` was *set*, never what the field was
  *called*.

Law D lists three ways a test silently is not coverage (never compiled, filtered out,
callee-tested-not-dispatch). This is a fourth: **the test asserts the half of the output
that was not broken.** It has a predictable trigger — when the defect is in *guidance* (a
hint, an error message, a health field, a doc string), the existing test almost always pins
the *detection*, because detection is what the original author was thinking about.

**The rule, three steps, in order.**

1. Treat `## Root cause` as a hypothesis and open the code it names. (A-chain, sixth
   instance: R-49 → R-62 → R-78 → R-80 → R-84 → R-92.)
2. When it survives, **re-derive its scope** — ask what else reaches the same line. Both
   bugs here grew on this step, neither on step 1.
3. Before writing the regression test, read what the existing test asserts. If it pins the
   detection and your defect is in the guidance, it cannot catch you; the new test must
   assert the guidance and then be **mutation-verified** against the old behaviour. Both
   were: reverting each fix reproduces the filed symptom verbatim and fails the new test.

**Substrate note.** R-91's instance 3 — `lsp_languages_offline` read as fact — *was* this
same `49e562fc`. The field is now `lsp_languages_degraded` with a per-language
`degraded_causes` map (`56fe1dd4`), so that particular self-report no longer lies. R-91's
third concrete form stands unchanged as a rule; one of its exhibits has been repaired.

**Promote-when:** a third instance of step 2 widening a filed bug. At that point step 2
belongs in the `reconnaissance` memory topic as a one-line imperative — *"a filed root
cause is a lower bound; re-derive its scope"* — since it is craft-shaped, not
project-shaped.
## Template for new entries

<!-- Insert new R-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## R-N — title\n**Verdict:** ...\n...")
     Also update the Index table row at the top. -->
