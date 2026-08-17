---
kind: tracker
status: active
title: Reconnaissance patterns
tags:
- reconnaissance
- skill-meta
- scout
entry_high_water_R: 104
entry_prefix: R
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
- **Only 14 of 55 body entries carry a `Status:` line** — corrected 2026-08-16 from
  "16 of 63". That figure came from `grep -c 'Status:'`, which counts this bullet's
  own two prose mentions of the word alongside the field's actual uses; anchoring
  the pattern to line-start gives 14, then and now. The denominator moved too: 63
  predates the archive pass, and 55 body entries survive it. So **41 entries carry
  no disposition at all**. (Law B on this ledger's own instrument — a count of a
  word also counts the document's discussion of it.) Only two record a
  discharge — R-1 and R-3, both promoted to SKILL.md in May and still sitting in
  the active file. There is no disposition field, which is why `Promote-when`
  criteria go unharvested and why the archive policy had nothing to enforce
  against. **Adding a `Status:` to every entry is the single highest-value
  structural change to this tracker.**
- **Removal recovers less than entry counts imply.** Archiving 13 of 91 entries
  cut 5.8%, not ~14%, because roughly a third of the corpus lives in
  self-contained index-table rows rather than bodies. Second measurement of the
  same effect that day (the D1 guide dedup realised 41% of its byte estimate).
- **The disposition backlog is 25 entries, not "every entry without a `Status:`
  field"** (measured 2026-08-16). 57 body entries, 18 now carry the field. But 43
  carry a `Promote-when` criterion, and three of those — R-89, R-90, R-91 — were
  already adjudicated **in prose** (`Promote-when: FIRED …`) with no field to grep;
  a field-presence count misses them entirely. Those three are now normalized, which
  leaves **25 entries with an unharvested `Promote-when`** as the actionable queue.
  Note the shape: *field presence* and *adjudication presence* are different
  questions, and only the second one matters.
- **Twice in one sweep the instrument counted the document's discussion of a token
  as a use of it.** `grep -c 'Status:'` matched the bullet above that describes the
  field; a `/FIRED|fired/` adjudication probe matched R-45's "the tell that should
  have fired", promoting an unfired entry into the adjudicated set. Field detection
  needs a **structural** anchor — line-start, a key prefix — never a keyword. Prose
  and field share a vocabulary by construction, so this is not a pattern-quality
  problem that a better keyword fixes.

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
3. ~~**The promotion protocol is written but NOT DEPLOYED.**~~ **DEPLOYED and
   verified 2026-08-16.** The reconnaissance skill's Sync flow gained a third
   destination (the session-opening surface, gated on a measured base arm and a
   one-to-two slot budget) and an audit-on-promote step with four staleness modes
   — `claude-plugins:c889e83`. Bumped 1.16.4 → 1.16.5 and reinstalled to all three
   profiles; each cache copy is 30,078 bytes and `diff -q` against the source repo
   reports identical. The protocol now governs.

   *Verification note worth keeping.* The first check reported the third
   destination MISSING — `grep "three destinations"` returned 0 because the source
   reads `**three** destinations` and the markup breaks a literal match. A false
   negative produced by query shape, which is law C exactly, on the very pass that
   deployed law C's home. What settled it was `diff -q` against source: a whole-file
   comparison cannot be defeated by pattern shape, whereas a grep is only ever
   evidence about the pattern. **When verifying that a deployment landed, compare
   the artifact, do not search it.**

4. ~~**C is promoted in its weakest form, and that explains its recurrence.**~~
   **RE-PROMOTED 2026-08-16.** Phase 1 now carries all four mechanisms — scope
   (R-3), shape (R-77), encoding (this session), and R-79's hard rule that a
   negative search result must never authorise a deletion. The audit that
   produced it also found the protocol's own `Outgrown` precedent text **false**:
   it claimed a *fifth* self-labelled recurrence, where the entries self-label
   **four**, and R-87 is the law's *hit* rather than a failure. Corrected in the
   same pass. Full record: **R-93**.

   *Note for the next pass.* The `## Index` table's rows stop at **R-86**;
   R-87..R-93 exist as sections only. Entry identity is therefore the section
   headings, not the index — which is exactly what made `grep "^## R-(77|79) "`
   return a false zero during this audit, since R-77 and R-79 are index rows with
   no section. Either backfill the rows or state the convention change at the
   table's head: a half-maintained index is worse than none, because it reads as
   complete.

5. **B may be outgrown too, by the unreachable mode rather than the wording one.**
   The skill's substrate law already names *"a test suite importing an installed
   wheel instead of the working tree"*, which is R-89's stale-build case exactly
   — yet R-89 recurred ×4 citing a session-log entry as its parent, never the
   promoted law. The text was general enough and was never fetched. Fix is
   placement (destination 3), not a better sentence.

6. **The A-chain and C-chain are candidates for promotion, not archiving.** A at
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
>
> **Seven of the nine survive in-file (checked 2026-08-16).** R-56b and R-59b were
> split by `52fca682` and then archived by `b6bb6377`, the first R-N archive pass —
> so a both-formats count returns seven, not nine. The list above is the *record of
> the split*, not an inventory of what the file now holds; read it as history. This
> is R-94's law applied to this note — a declaration inventory and a delivery
> inventory diverge, and no gate connects prose counts to the entries they count.

| ID | Date | Verdict | Pattern | Evidence (session-log) |
|----|------|---------|---------|------------------------|
| R-55b | 2026-08-08 | miss | A host-specific symptom accepted as the norm without sweeping the other hosts | 9 of 15 declared roots swept; all nine had zero untracked entries |
| R-57b | 2026-08-08 | miss | An identifier's shape says nothing about whether the thing exists — check its declared root | caught by an unrelated tool response an hour after the claim was published |
| R-58b | 2026-08-08 | hit | Before fixing a heuristic, grep for other copies of it — `references()` cannot see a duplicated closure | `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` |
| R-87 | 2026-08-15 | hit | Before designing an abstraction, scout for the dispatch point that already exists | SD-1b |
| R-88 | 2026-08-15 | hit | The instrument that nominates a refactor group also fixes its axis, and that axis can be orthogonal to the real duplication | SD-3 + SD-10; `legibility_scan` tier-1 group, premise falsified by live A/B |
| R-89 | 2026-08-16 | miss ×3 | A tool's output is evidence about the code only if the running build contains it | SD-1b; `5917e37e`, `7c91cdf7` |
| R-90 | 2026-08-16 | miss ×2 | Two sessions, one working tree — `git add -A` silently annexes the other's staged work | `543086d1`, `8b27b1ea`; promote-when has fired (per-session worktrees — human call) |
| R-91 | 2026-08-16 | miss ×3 | A probe that cannot observe the thing the claim is about | benchmark + tracker-hygiene session |
| R-92 | 2026-08-16 | hit ×2 | A filed root cause is a hypothesis, and confirming it usually widens the bug | the two tool-quirk bugs filed by the 2026-08-16 hygiene sweep |
| R-93 | 2026-08-16 | hit | First audit-on-promote: C re-promoted, and the audit's own precedent text failed the audit | `claude-plugins:c889e83` (1.16.4 → 1.16.5) |
| R-94 | 2026-08-16 | hit ×1, miss ×2 (self-caught) | A wiring inventory is not a delivery inventory, and it is wrong in both directions | BL-25 (guide topics nothing triggers) |
| R-95 | 2026-08-16 | hit ×5 → rule | A deferral rationale is a claim, and it is the least-audited kind | BL-11 / BL-12 / BL-16 worktree cluster |
| R-97 | 2026-08-16 | miss (self-caught) → rule | **A classifier you just wrote has been calibrated on exactly one case: the one that made you write it.** Shipped `snapshot_drift` with the gate "body line-anchors ≥1 `PREFIX-N`", fitted to the tracker that motivated the bug; the SECOND real tracker was a false positive (params-canonical by design, 14 of 68 ids mentioned incidentally). The population was enumerable all along and is bimodal with no overlap — 100% contiguous-prefix (11 in sync) / 61% prefix (the real lag) / 21% scattered (the false positive) — so the discriminator was one script away. Same day, same shape, second instance: the audit itself reported "3 of 28 drifted" in a codescout conversation when the 28 spanned seven repos and one finding was another project's. Rule: run a new classifier over the whole enumerable corpus and read the distribution; the motivating case is the worst validation set, being the one it was fitted to. | BL-29; `0dbfd0ee`, `af2508b4`. Kin R-96 (same law, applied to tests rather than results), R-92, R-95, R-90 |
| R-100 | 2026-08-17 | miss ×2 (self-caught) → rule | A pinned test's rationale is the cheapest refutation of your own bug — read a guard's tests **by name** before filing that it misses a case | librarian-guard bug filed then retracted same day; `bb9a94d7` reverts the stamp it caused. Kin R-92, R-95, R-97 |
| R-99 | 2026-08-16 | hit | Three unrelated-looking ledger defects had one root cause — the entry template named one field of four and mentioned the index row as an aside. A convention documented anywhere but the thing authors copy is not a convention | 13 orphaned bodies + 39 missing dispositions + 9 duplicate ids, all in this file; fix is the template rewrite. Kin R-94, R-97, R-98 |
| R-98 | 2026-08-16 | miss (self-caught) → rule | An id read from a scan is stale the moment a peer session writes — re-check at the point of allocation, and against the working tree, not `HEAD` | near-miss on collision #10; `a1ac0317` (sweep) vs `2f94ce40` (peer's R-97 — originally cited as `c60242ac`, orphaned by their amend). Kin R-90, R-94, R-97 |
| R-96 | 2026-08-16 | miss (self-caught) → rule | Widening a gate disarms the tests that used it as scaffolding, and they go green for a new reason | GF-1 / GF-2, `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` |
| R-1 | 2026-05-19 | hit → promoted | Pre-dispatch grep for asserts on `include_str!`'d constants | mcp-prompt-redesign F-1 + W-1 |
| R-3 | 2026-05-19 | miss → promoted | Scout limited grep to one file/crate; cross-file asserts slipped | mcp-prompt-redesign F-2 |
| R-4 | 2026-05-19 | miss | Grep undercounts struct-field construction sites by 2-3× | mcp-prompt-redesign F-3 + W-2 |
| R-6 | 2026-05-28 | hit | Explicit recon invocation on substrate before mechanism design | prompt-guide-refactor F-2 + W-2 |
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
| R-21 | 2026-06-09 | hit | Verify a side-effect through its real production entry point (CLI/MCP), not a unit harness that bypasses `main.rs`; `references()` the operation to enumerate ALL call sites before placing it (`sync_project`: 5 sites, write reached 1 of 3 project paths) | index-freshness F-1 + W-1; commit 10dcfb9f |
| R-22 | 2026-06-11 | hit | Scout the LSP call path to confirm a staleness mechanism before choosing the fix layer (references false-zero: `did_open` syncs def-file only, no barrier; all LSP signals share the staleness so the fix must be LSP-independent) | issues/2026-06-09-references-false-zero; commit ddc7e3f1; kin R-21 |
| R-23 | 2026-06-11 | hit, then miss | Re-derive an inherited diagnosis from usage.db telemetry (hit); verify a shared single-holder-resource recovery by READING lock/process state, not by issuing a call from a 2nd client which re-creates the contention (miss) | bug-fix F-16 + W-12; issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision |
| R-24 | 2026-06-11 | hit | Scout the resource-key derivation before designing a concurrency test; path-keyed hashing makes worktrees a safe fan-out fixture | bug-fix W-13 + F-18; issues/2026-06-11-lsp-tools-ignore-workspace-pin-path |
| R-25 | 2026-06-11 | hit | Scout the catalog's status source + id-keying before archiving a librarian tracker (status lives in the catalog row not the file; `id=sha256(abs_path)` so `git mv`+reindex orphans history) → `artifact(update)`+`artifact(move)` | this session; commit `b487a69c`; kin R-24/R-15 |
| R-26 | 2026-06-11 | hit | A grep line-match locates a symbol; it doesn't confirm a mechanism — read the body before narrating "confirmed" (`kill_on_drop`→SIGKILL-orphan verified at `process.rs:66-135`, no reaper in the spawn path) | this session — mux-LSP-sharing brainstorm; kin R-19/R-5/R-23 |
| R-28 | 2026-06-12 | hit + miss | Enumerate a prompt surface's full gate set before editing (byte-for-byte slice, snapshot fixture, cap, ONBOARDING_VERSION pin, content tests); targeted `cargo test --lib <module>` filters miss cross-cutting gates in `server::tests` (over-budget get_guide description shipped via a narrow filter) | bug-fix F-20 + W-15; kin R-1/R-7/R-27 |
| R-29 | 2026-06-13 | hit | Verify a flight-recorder-harvested target exists in the active repo before ranking/acting on it — `.codescout/usage.db` is keyed by commit-SHA and mixes every project the process served (40 project_shas; a mirela `CalendarService` phantom surfaced in a codescout survey) | dzo-legibility F-1 + W-1; kin R-23 |
| R-33 | 2026-06-15 | hit | A reconciliation audit marked two adjacent legacy-era symbols (`fusion::rrf_fuse`, `schema::SearchResult`) both "graduated to live — keep"; `references()` showed OPPOSITE liveness (rrf_fuse test-only → deleted; SearchResult live → kept). Dead-vs-live is per-symbol call-graph, not file-proximity. | legacy-retrieval-removal L-08; this session; kin R-26/R-27/R-21 |
| R-34 | 2026-06-15 | hit | On a cross-platform branch, the host `cargo check` is necessary-not-sufficient for a rebase — it never compiles the gated target. After rebasing `vdi-windows` onto `experiments` (conflict in the `#[cfg(unix)]` peer gate itself), cross-compiled `--target x86_64-pc-windows-gnu` to confirm the incoming commits added no ungated unix-only code (EXIT=0). Compile the target the branch exists for, not just the host. | `vdi-windows` rebase this session; `src/tools/mod.rs` peer-gate; WIN-23; kin R-5 |
| R-35 | 2026-06-16 | hit | A tool's own error diagnostic is a hypothesis, not ground truth. `edit_code`'s "AST parse failed — likely syntax errors or duplicate siblings" was falsified by `symbols()` (clean parse, unique name_path); the archived backtick bug was already fixed. A throwaway dump of `extract_symbols_from_source`→`find_ast_end_line_in` on the real file pinned the real cause in one run: AST start 214 (annotation line) vs LSP 216 (`fun` line), matcher flips Some→None at the ±1 gate. Reproduce the failing internal call when a cheaper read disagrees with the error text. | bug-fix W-17/F-23; issues/2026-06-16-kotlin-edit-code-annotation-line-gap; kin R-5/R-32 (diagnostic/claim ≠ ground truth) |
| R-39 | 2026-07-10 | hit | Adding a tool param/alias is additive-safe in codescout: every `*_schema_*` test is positive-presence (`props["x"].is_object()` / `contains_key`), none enumerate the exact prop set, and no `input_schema()` sets `additionalProperties:false` — so a new prop can't break a snapshot, and an unknown key flows through to `call()` (the very mechanism the alias fix relies on). | param-alias-ergonomics session (this session); 3038 lib passed / 0 failed; kin R-28/R-36 |
| R-41 | 2026-07-17 | miss → promoted | A later table-rebuild migration (`CREATE _new`/`INSERT … SELECT`/`DROP`/`RENAME`) has a column list that is a silent ALLOW-LIST — a column an earlier migration added but the SELECT doesn't name is dropped on swap, no error. Adding a column is a seam whose far side is every later rebuild's SELECT. | Stage-2 review; `migrate_v6.rs::drop_legacy_and_stamp` dropped `slug`; fix 9aa8063f + test `migration_v6_single_open_preserves_v9_entry_graph_shape`; kin R-3/R-28 |
| R-42 | 2026-07-17 | miss → promoted | When a writer produces a new value shape (id-keyed ref, optional field), each reader's absent-key/None branch must RESOLVE the other shape, not dead-end (return empty / fall through) — a dead-end silently drops every value stored in that variant. Shared incidental test preconditions ("target always has a slug") mask it. | Stage-2 review; `get(include_links)` hid incoming-by-id backlinks for slug-less targets; fix 70d16686; kin R-27/R-21 |
| R-76b | 2026-08-15 | miss ×2 → rule | **Aggregate behaviour data is a screen, not a verdict — and it is wrong in BOTH directions.** An audit of 53,916 recorded tool calls ranked failures by frequency and by "what tool came next". Reading the actual arguments of ~12 instances overturned two conclusions. (a) **Overstated:** every rejection of a `json_path` wildcard was scored a successful recovery because the next call returned `success`. The arguments showed what those recoveries were — `$.items[*].abs_path` → read 393 raw lines; `$.entries[*].id` → `$.entries[4].id` one element per call; another → abandon the buffer and re-query upstream; another → grep the buffer. A green outcome column concealed a degraded workaround every time: it records that the call returned, never that the agent got what it asked for. (b) **Understated:** a guard scored at 35% "same-tool recovery" was in fact working — its correct recovery is a DIFFERENT tool (`symbols(include_body)` on the exact symbol wanted), which the same-tool metric counts as failure. Same-tool and adjacent-call metrics systematically penalise every guard whose right answer is another tool or needs a lookup first (widening one route's window from 1 call to 3 moved compliance 45% → 74%). And the sharpest defect was invisible to all aggregation: bucketing the refused ranges showed 69 of 244 were canonical imports reads (`1-20`, `1-30`), refused by a guard whose recommended alternative structurally cannot return imports. **Rule:** before filing or closing anything from aggregate counts, read the arguments of ten instances — and check whether the payload that would show intent is even being recorded. | 2026-08-15 tool-usage investigation TU-11; `docs/trackers/2026-08-15-tool-usage-investigation.md`. Kin R-50 (the view is not the set — this is its behavioural-telemetry form), R-75 (verify the artifact produced, not the exit code) |
| R-75 | 2026-08-13 | miss → rule | **A process-level env scrub is not configuration isolation — a program that reads config from disk reconstitutes what you removed, and the result looks like success.** An end-to-end probe launched under `env -i` with three explicit `CODESCOUT_*` vars was treated as hermetic on that basis. `main.rs` calls `load_startup_env`, which reads `~/.config/codescout/.env` (here a symlink to the repo's own `.env.amd`) and fills every key the caller left **unset** — so a url that had just been scrubbed came back, and the retrieval path silently discarded the `local-dir:` model in its favour. Exit **0**, `added=5`, no warning; the only tell was the artifact's shape, `FLOAT[768]` where the local model is 384-dimensional. This is the substrate rule (read what world the tool read, not just its verdict) failing where it was most needed: the negative control — does this run still succeed when I make the config source *provably* absent? — was never run. The repo already knew the escape (`tests/cli_artifact.rs:18-24` sets `CODESCOUT_ENV_FILE` to a nonexistent path for exactly this reason) and it was not consulted. **Rule:** before believing an isolated run, name every layer that can supply config — process env, dotenv, user config, project config, compiled default — and neutralise or observe each; then verify the *artifact produced*, not the exit code. Note the compensation: chasing the impossible dimension is what exposed the real defect, so the sloppy probe still paid — but it would have paid as a false "verified" had the dimension gone unread. | local-onnx-embedding session log F-7 + W-5; `docs/issues/2026-08-13-url-silently-overrides-local-dir-model.md` (fixed `38e0980b`). Kin R-50 (the view is not the set), R-6 (scout the substrate before mechanism design) |
| R-54 | 2026-08-05 | miss → rule | **Before counting rows, ask what one row IS and whether two rows can contain the same underlying event — and never let "zero observed" become "empty" without stating n.** A corpus of 63,574 rows was 444 sessions: each row was one request re-sending the whole conversation (143 per session, 165× double-count). A 64-row sample was 34 effective sessions; a top band of 13 rows was 6. Nesting is invisible downstream because the duplicated content is genuinely present in both rows. Sibling of R-53: that one guards what the corpus is MADE OF, this one guards what a ROW MEANS | provenance-probe F-14 + W-10; PV-65; PV-66 |
| R-53 | 2026-08-04 | miss → rule | **A corpus's composition is a seam — census it by producing tool before you measure it, and when you stratify by a magnitude, ask what makes things big.** One tool contributed 59.8% of all tool-result bytes as base64 image data that `json.dumps` had stringified into the text denominator; it also defined the top band of a size-stratified sample (11/13 sessions), so the magnitude axis was a producer axis in disguise. Invisible to thirteen rounds of internal checks — contamination moved numerator and denominator together. Caught only when the corpus owner said the traffic was not representative | provenance-probe F-13 + W-9; PV-61; PV-62 |
| R-52 | 2026-08-04 | miss → rule | **An artifact's ownership is the union of its inputs' ownership — choose the output location from the input set, never from where the code lives.** Sibling to R-51, not an amendment: R-51's failure produces wrong NUMBERS, this one produces MISPLACED DATA, and R-51's exclusion-at-entry fix leaves the artifact in the wrong place. A pipeline reading N corpora writes outside all N. Caught at staging: a probe in `codescout/scratch/` held ~14MB of symbol vocabularies from 8 repos including client work, and `semantic_search` was returning them — the retrieval consequence R-51's framing does not reach | provenance-probe F-2; PV-60; staging check 2026-08-04 |
| R-51 | 2026-08-03 (escalated 08-04) | miss → rule, promote-ready | **An instrument that writes into the corpus it measures.** New seam class: output-path ↔ measurement-domain overlap. A probe wrote its own artifacts (`sessions.json`, `vocab/*.json`) into the repo whose symbol vocabulary it was building; DF inflated 6.2× (35,496 → 221,214) with no error raised, caught only by an incidental before/after ratio check. Not scoutable statically — the overlap is created by *running* the pipeline. Complement of R-50: R-50 = name what the view dropped, R-51 = name what the view wrongly admitted | provenance-probe F-2 (fixed-verified); F-3 same log is the R-50-shaped twin |
| R-50 | 2026-07-28 | miss → rule | **The view is not the set.** Five distinct errors in one session share one shape: concluding from an enumeration that had silently excluded something. A liveness filter, then reusing the filtered list; `pgrep \| tail -1` picking an orphan not the live server; a SHA census matching frontmatter `id:` as commits; `ls \| wc -l` counting `.anchors.toml` sidecars as memories; a compact summary read as a whole result. Before concluding from a list, name what the view dropped — filter, cap, preview, or a pattern that matched the wrong tokens | bug-fix F-38 + W-30; also F-33/F-35 and the four corrections listed in-entry; kin R-10 (completeness scout), R-26/R-27 |
| R-49 | 2026-07-28 | hit | Re-scout your OWN bug file / plan before implementing it — an artifact written during the observation is a hypothesis that acquires the authority of a record the moment it is committed. When a root cause cites two functions, read the layer BETWEEN them: a chain's middle is where the guards live, and a mechanism inferred from its two ends will never mention them. Three failures of session-authored artifacts in one session (inverted fix option, overstated doc guarantee, unsupported root cause) | bug-fix F-37 + W-29, F-34/W-26, F-35; issues/2026-07-28-edit-code-target-base-from-stale-lsp-range (mitigated); kin R-32/R-46 |
| R-48 | 2026-07-28 | hit | A fix built from a bug report's reproduction inherits that reproduction's blind spots, and a test suite written against the same fixture inherits them too — seven tests all reused the report's `"\` shape and none could see that a plain `"` + raw newline (the commoner Rust form) was unprotected. Found by writing the end-to-end fixture the *natural* way and tracing the fix over it before running. For a syntax-level fix, enumerate the language's other syntaxes for the same construct before closing | bug-fix F-36 + W-28; issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents; kin R-26/R-27 (read ≠ verified), R-46 |
| R-47 | 2026-07-28 | hit | Enumerate the callers of every function in the chain a fix touches, not only the symbol the report names — the report walked one call path. `reindent_to`'s 3 callers were all `edit_code`, but its delegate `reindent_block` had a second production caller (`edit_file/mod.rs:747`) with the same defect, reached without passing through `reindent_to`. Relocating the fix one level down doubled the surface closed at the same size of change | bug-fix W-27; issues/2026-07-28-edit-code-reindent-shifts-string-literal-contents (archived, `79cd1428`); kin R-17/R-31/R-44 |
| R-45 | 2026-07-28 | hit | Relocating a file needs a discovery-by-scan grep; caller enumeration is blind to it (generalises R-44: `cfg` on a value → callers are the consumer set; `cfg` on a location → callers are half of it) | bug-fix F-33 + W-25 |
| R-44 | 2026-07-25 | hit | The write-side twin of R-43: before accepting a proposed `#[cfg]` gate on a `pub mod` declaration, enumerate the CONSUMER set (`grep <mod>::|use .*<mod>` at workspace root, `context_lines=2`) and check whether the config being gated OUT is the one the plan's own tests or invariants live in. Gating a module is a subtree delete; the declaration site cannot show its blast radius. | dependency-review session F-3 + W-2; `src/retrieval/mod.rs:3` + 14 ungated `RetrievalClient` consumers; kin R-43/R-5/R-17 |


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



**Status:** promote-when **FIRED** 2026-08-16 — fourth instance, raised from another
session (`28ea039a`, commit `5917e37e`: a committed claim about shipped guidance that
rested on a stale build). Full adjudication is the **Promote-when** line above; this
field only normalizes it. Added by the 2026-08-16 disposition sweep — the entry was
already adjudicated in prose, which is exactly why a `^**Status:**` grep did not see it.
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

**Third instance — 2026-08-16, Promote-when FIRED.** During the worktree-cluster
fixes, `git add -A` swept the concurrent session's newly-created bug file
(`2026-08-16-run-command-backticks-substituted-in-quoted-message.md`,
`e04115d9477d280b`) into `8b27b1ea`, a commit about the read-side worktree
notice. Content intact, attribution wrong, commit message silent — the same
shape as the first two, in the same direction as the second (this session as
perpetrator, not victim).

What makes it worth recording rather than embarrassing: **the corrected rule was
already written in this entry and was still not followed.** Step 2 says
`git add <paths> && git commit --only <paths>`. The reflex reached for `-A`
anyway, at the end of a long working stretch, on the third of three bug fixes.
An entry being *correct* does not make it *reached for*; a rule that only fires
when you remember to consult it is not yet a rule.

It recurred once more within the same hour, in the weaker form: `git add src/`
staged two files the other session was mid-edit on (`frontmatter.rs`, `mv.rs`).
That one was caught by reading `git status --short` before committing and undone
with `git restore --staged` — which is the actual working defence, and cheaper
than remembering the right flag: **read what you staged, not what you meant to
stage.** A directory pathspec is `-A` scoped to a subtree; it is not a fix.

**Promote-when: FIRED (3 instances).** The worktree split stops being optional
and becomes the default for concurrent sessions. Until it is, the enforceable
form is the readback, not the flag — `git status --short` between `add` and
`commit`, every time, because it catches every variant (`-A`, a directory
pathspec, a glob) with one habit instead of one habit per variant.

**Kin:** R-95 (a rationale nobody re-audits) — this entry's own corrected rule
was the un-audited claim the third instance walked past.

**Status:** promote-when **FIRED** (3 instances) — **promotion not applied.** The target
is adopting per-session git worktrees as the default for concurrent sessions; that is a
workflow change and an explicit human decision, still untaken. The interim enforceable
form is this entry's own corrected rule — a `git status --short` readback between `add`
and `commit`, which catches every variant with one habit. Added by the 2026-08-16
disposition sweep.
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

**Promote-when: FIRED 2026-08-16 — promoted.** The criterion's second arm ("one instance
where the rule is applied prospectively and prevents a claim") was met while live-verifying
the `56fe1dd4` fix. `librarian(audit_doc_refs, paths=["docs/issues"])` returned
`n_files_scanned: 0` with `exit_code: 0` — indistinguishable from a silent no-op, and this
repo's standing rule is to file on notice. Reading `mod.rs:341` first showed `paths` is a
**glob** list, so a bare directory matching nothing is correct behaviour. **No bug was
filed.** That is instance 1 of this entry — an existence check bound to the wrong question —
caught before the claim instead of a day after it.

A second prospective application in the same verification: three `audit_doc_refs` runs came
back `degraded: false` and were nearly read as "the degraded path is verified clean". They
were not evidence about it — one scanned 0 files, one had no `file_symbol` refs to reach the
LSP branch at all, one had two that resolved. Only a 333-file scan against a cold
rust-analyzer actually reached the branch, and it returned
`degraded_causes: {"rust": ["lsp_behind_index"]}`.

The three concrete forms are now in the `reconnaissance` memory topic (rule 7), with the
zero-result corollary appended. Craft-shaped, so the global `SKILL.md` remains the eventual
destination via the sync flow.

**Substrate note.** Instance 3's exhibit is repaired. `lsp_languages_offline` is now
`lsp_languages_degraded` with a per-language `degraded_causes` map (`56fe1dd4`,
`docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`), so that
particular self-report no longer lies — confirmed live on the rebuilt server, where the
same condition that once printed `offline: ["rust"]` now prints `lsp_behind_index`. The
rule stands unchanged: the field is still a claim, and the next one may not have been fixed.



**Status:** promote-when **FIRED** 2026-08-16 — **promoted.** The criterion's second arm
("one instance where the rule is applied prospectively and prevents a claim") was met
while live-verifying `56fe1dd4`: an `audit_doc_refs` call returning `n_files_scanned: 0`
was read as a possible silent no-op, and reading `mod.rs:341` first showed `paths` is a
glob list, so no bug was filed. Added by the 2026-08-16 disposition sweep.
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

## R-93 — First audit-on-promote: C re-promoted, and the audit's own precedent text failed the audit

**Observed:** 2026-08-16, first exercise of the audit-on-promote protocol that
shipped the same day (`claude-plugins:c889e83`, 1.16.4 → 1.16.5).

**The promotion.** Law C was carried in the skill as a single mechanism —
*"Grep scope: workspace root, not the file being modified"* (promoted 2026-05-23
from R-3) — while its chain had since added three more. Phase 1 now carries all
four: **scope** (R-3), **shape** (R-77 — a query that answers *"what is in this
directory?"* never answers *"does this thing have a test?"*), **encoding** (this
session, below), and the hard rule from R-79 that **a negative search result must
never authorise a deletion**.

**The audit found its own precedent false.** The `Outgrown` mode's worked example
— written the previous day, by the same hand — claimed *"a **fifth** self-labelled
recurrence"* over the chain `R-73b → R-77 → R-79 → R-87`. What the entries
actually self-label: R-77 says *"third instance"*, R-79 says *"fourth recurrence
of R-77"*, and the first two are folded inside R-73b. That is **four**. And
**R-87 is the law's *hit*** — the scout ran and found the abstraction already
there — not a fifth failure. Corrected in the same commit. Mode 1 (**False**)
firing against one-day-old text, inside the section that defines mode 1.

**Two fresh C instances, both in this session, both while auditing C.**

1. `grep "three destinations"` → 0, because the source reads `**three**
   destinations` and the markdown emphasis breaks the literal. A deployment check
   that nearly reported the opposite of the truth. Settled by `diff -q` against
   source.
2. `grep "^## R-(77|79) "` → 0, concluding those entries did not exist. They exist
   as **Index-table rows**, not `## R-N` sections. The file keeps entries under two
   conventions and I searched one — R-77's own mechanism, applied to R-77.

Both are the encoding/shape half of the law, and the promoted wording as it stood
covered neither. That is the datapoint justifying the rewrite: the narrow form was
live, loaded, and did not fire.

**The audit of the rest of the set.** Recorded so the next promotion inherits the
check rather than repeating it:

| Law | Mode checked | Verdict |
|---|---|---|
| C — search / absence | Outgrown | **Confirmed, fixed** — re-promoted with all four mechanisms |
| B — substrate / instrument (R-89) | Unreachable | **Confirmed, open** — the text is general enough and is never fetched. Fix is placement (destination 3), not wording. Still-open item 5. |
| `iron-laws-detail` guide's `cat`-is-permitted sentence | False | **Confirmed, fixed earlier** — 0/10 unaided survival; now pinned by test |
| Remaining Phase 1 laws | all four | **No recurrence on record** — no ledger entry cites any of them as a repeat, so no action. This row exists so the next audit does not re-derive it. |
| Whole set | Obsolete | **None** — no promoted law guards a failure that a structural gate now prevents |

**Verdict: rule.** Two process rules earned:

1. **When verifying that a deployment landed, compare the artifact, do not search
   it.** A grep is evidence about the pattern; a whole-file `diff` or checksum
   cannot be defeated by pattern shape. Now shipped as C's **encoding** clause.
2. **Audit the precedent text, not only the promoted rule.** The worked examples
   inside a protocol are load-bearing — they are what a reader pattern-matches on
   — and they go stale exactly like the rules they illustrate.

**Counterfactual.** Without the audit step, the re-promotion would have shipped
carrying a precedent that miscounts its own evidence and miscasts a hit as a
failure — in the one paragraph a future agent reads to decide whether *their*
promoted law is outgrown. The wrong lesson would have been: recurrences are
inevitable, five of them are normal.

**Kin:** R-3, R-73b, R-77, R-79 (the chain), R-50 (the view is not the set), R-87
(the hit), R-89 (the unreachable case audited alongside).

## R-94 — A wiring inventory is not a delivery inventory, and it is wrong in both directions

**Verdict:** hit ×1, miss ×2 (self-caught) · **Observed:** 2026-08-16, fixing BL-25
(guide topics nothing triggers)

The bug was found by counting `relevant_guide_topic()` implementations against the
`GUIDE_TOPICS` registry: 7 of 10 topics had no trigger, so 47,343 bytes of authored
guidance reached nobody. That count was **right**, and acting on it was right. But the same
count is wrong twice over, and both errors surfaced while fixing it.

| # | What the registry said | What was true | Why the inventory could not tell |
|---|---|---|---|
| 1 | 7 topics untriggered → undelivered | correct — 47,343 bytes reached nobody | the hit; this is the bug |
| 2 | `project-activation-bootstrap` untriggered → undelivered | **delivered on every session's first call** | a *second* delivery path — `call_content`'s empty-ledger branch fires it from any tool, naming no topic in any impl |
| 3 | `symbols` triggers `progressive-disclosure` → delivered | **delivered nothing on any result that fits** | the call site gates that topic on overflow having actually occurred, so the trigger fires into a downstream `false` |

**A trigger and a delivery are independent facts.** Row 2 is a trigger-less delivery; row 3
is a delivery-less trigger. A scan of the wiring registry produces false negatives and
false positives at the same time, and nothing in the registry hints at either — both
mechanisms live in the *consumer*, one branch above and one branch below the lookup.

**Rows 2 and 3 were caught by the gate written for row 1**, which is the part worth
keeping. The gate enumerated tool impls and failed `project-activation-bootstrap`; had I
trusted it, I would have re-added a redundant trigger to fix a non-problem. Reading the
consumer to find out *why* it failed is what surfaced both the second delivery path and the
downstream gate — and row 3 turned into free value, since a slot that delivered silence
now carries `symbol-navigation` at no cost.

**The rule.** Before concluding anything from a registry, wiring table, or
implementation count, **read the consumer** — the code that reads the registry — and answer
two questions:

1. *Can this thing be delivered by a path that does not appear in the registry?* (row 2)
2. *Is a registry entry sufficient for delivery, or is it gated further downstream?* (row 3)

An inventory answers "what is wired". Only the consumer answers "what arrives", and those
differ in both directions.

**Corollary for the gate you write.** A test that enumerates one mechanism will report the
other mechanism's output as missing. Encode the second path explicitly (here:
`SESSION_OPENING_GUIDE` is seeded as triggered-by-construction, with the reason in a
comment) — otherwise the gate generates exactly the false alarm it was built to prevent,
and the natural "fix" is to add redundancy.

**Relation to the laws.** Law C says a search that finds nothing is evidence about the
search; this is its structural twin — *a lookup that finds something is evidence about the
lookup*. Law D's "a test that cannot fail is not coverage" covers row 3 from the test side;
row 3 is the production-code version: a trigger that cannot fire is not delivery. Nearest
kin is R-91 (a probe that cannot observe the thing the claim is about), of which this is
the registry-grain case.

**Also seen this session, same shape, no ID of its own:**
`first_artifact_call_emits_librarian_hint` runs `find kind=tracker` and expects the
`librarian` guide — which now holds **only** because its fixture catalog is empty, so
`items: []` names no tracker path. It passes for a reason unrelated to its name. Left in
place with the dependency stated in the neighbouring test's doc comment rather than
silently depended on.

**Promote-when:** a second instance where reading the consumer overturns a registry-derived
conclusion. At that point rule 1 ("can it arrive by a path not in the registry?") is the
craft-shaped half and belongs in the `reconnaissance` memory topic.

## R-95 — A deferral rationale is a claim, and it is the least-audited kind

**Verdict:** hit ×5 in one cluster → rule

**Observed:** 2026-08-16, the three-bug worktree cluster (BL-11 / BL-12 / BL-16).

**The seam.** A bug file's `## Fix` section routinely records *why the work was
not done*: a cost estimate, a blocking dilemma, a "cannot reproduce". Those
sentences are claims about the substrate exactly like a root cause is — but they
are read as **settled analysis** rather than as hypotheses, because they are
phrased as decisions. R-92 established that a filed root cause is a hypothesis
that usually widens on contact. This is its sibling and it is worse, because a
wrong root cause gets corrected the moment you fix the bug, while a wrong
deferral rationale is never revisited at all — its whole function is to stop
anyone looking.

**Every deferral reason in this cluster was wrong, and all five were wrong in the
direction that made the work look bigger or impossible.**

| Filed rationale | Measured |
|---|---|
| "a 38-site mechanical change to `ToolContext`" | **133** construction sites (`guide_hints_emitted:` → 134 matches, one the declaration) |
| "pick one deliberately; do not add a second one-shot mechanism" — a binary between a `ToolContext` field and a ledger sentinel | A **third** option existed: a separate `notices` set on `GuideLedger`. One construction site, no namespace collision |
| the sentinel is "a semantic compromise" | It is a **live regression**: `types.rs:626` reads `if emitted.is_empty()`, which IS the session-opening guide's trigger |
| "the guard proves the condition is detectable, and then only spends the detection on half the surface" | It spends it on **neither**. `guard_worktree_write` returns `Ok` on line 1 in every real session |
| "Not yet reproduced — no worktree session with shadow rows was active" | A unit fixture reproduced **both** halves in milliseconds; the second half needed no worktree session at all |

**The tell they share.** Each rationale was *locally plausible and never executed*.
The 38 was never counted — and R-4 had already recorded a 13-vs-30 undercount for
**this same struct** three months earlier, so the ledger predicted the error and
nobody read the ledger. The sentinel's real cost was one `is_empty()` call away.
The guard's premise needed one probe. The "cannot reproduce" was true only of the
recipe the file had written for itself.

**Why the direction is not a coincidence.** A rationale is written at the moment
someone decides to stop. Nobody drafts an estimate that makes the work sound
easier than it is, because that estimate would not justify stopping. So the bias
is structural: deferral rationales inflate, and the inflated ones are the ones
that survive, because nothing ever re-costs them.

**Rule.** When picking up a bug that was deferred, **re-cost it before believing
it is expensive** — the estimate is the first thing to verify, not the last:

1. Any *number* in a `## Fix` (site counts, file counts, "N places") — re-run the
   count. Field-specific grep, or `cargo check` as the enumerator (R-4/R-5).
2. Any *binary* framing ("option A or option B, both bad") — the third option is
   usually inside a type one of them already touches. Read the consumer.
3. Any *premise* about existing machinery ("the guard proves X is detectable") —
   run it once. A shipped guard with tests can still be unreachable.
4. Any "cannot reproduce" — ask whether the *recipe* is what is expensive. A
   runtime recipe demanding rare setup often has a unit-fixture twin, and the
   easier half of the bug may not be the half the recipe describes.

**Cost avoided here.** Believing the file would have left BL-12 blocked on a
133-site refactor it never needed, shipped a sentinel that silently disabled the
session-opening guide fixed hours earlier, and left BL-11 closed as
`wontfix-false-alarm` — its own § Resume instructed exactly that if the
reproduction did not show two sections, and the reproduction as written could
not have been run.

**Kin:** R-92 (a filed root cause is a hypothesis — this is the same law applied
to the reasons for *not* acting), R-4 (grep undercounts construction sites, same
struct, predicted this), R-59 (an unreachable hypothesis is cheaper to eliminate
than to measure), R-90 (its own corrected rule was an un-audited claim that got
walked past a third time).

**Promote-when:** one more cluster where a deferral rationale is falsified on
contact. At that point this belongs in the skill's Phase 1 as an explicit step —
*re-cost the deferral before you accept it* — beside the existing
read-the-`## Fix`-as-a-plan rule (R-62).

---

## R-96 — Widening a gate disarms the tests that used it as scaffolding, and they go green for a new reason

**Verdict:** miss (self-caught by the suite) → rule · **Observed:** 2026-08-16, the IL-3
refactor (GF-1/GF-2 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`).

**What happened.** Making `git` flag-conditional in `is_unbounded_lhs` turned one
pre-existing test red: `il3_still_blocks_when_a_quoted_separator_precedes_a_real_pipe`,
whose fixture was `git log --oneline -50 --grep='a;b' | head -3`.

That test's subject is **quote-aware segmentation** — its own doc says so, and its two
sibling assertions use `cargo test`. The `-50` was incidental realism. But `-50` is a real
commit-count limit, so under the new classifier the command became legitimately allowed and
the fixture **stopped reaching the gate the test exists to exercise**.

**Why it earns a rule rather than a shrug.** The failure was visible only because the
widening happened to make the test RED. Had the fixture been `cargo test … | head`, the same
widening would have left it GREEN — passing for the same reason as before — and there would
have been no signal at all. The dangerous version is the inverse of what happened here: a
widening that leaves a scaffolding-fixture passing *for a new reason*, with nothing to notice.

**The tell.** A test goes red on a change that has nothing to do with its stated subject. The
question is then not *"did I break the subject?"* but **"is this red because I broke the
subject, or because I moved the scaffolding?"** — and the answer has to be proven, not argued.

**Do.** When widening a gate, classifier, or predicate:

1. For each newly-failing test, read its **doc** and its sibling assertions to find the stated
   subject. If the changed predicate is not that subject, the fixture is scaffolding.
2. Amend the **fixture** to restore its reach — never the rule, and never by deleting the case.
3. **Mutation-verify the amendment**: revert the widening and confirm the amended test still
   passes. If it goes red, the fixture change weakened it and the amendment is wrong.

Here, reverting `git` to `UNBOUNDED_PREFIXES` left the amended case green while turning all
five new positive cases red — so the fixture still blocks for the reason it always did, and
the new cases discriminate on the new behaviour and nothing else.

**Relation to law D** (*a test that cannot fail is not coverage*). D is the STATIC form: the
test never could fail. This is the DYNAMIC form — it could fail yesterday and cannot today, as
a side effect of a change elsewhere in the tree. Same consequence, but there is no authoring
defect to find, and review cannot see it because **the test file did not change**.

**Kin:** R-70 → R-73 → R-76 (the D chain), R-3/R-73b/R-77/R-79 (law C, whose false zeros ran
three times in the same session).

### Addendum 2026-08-16 — the NARROWING direction, and why minimal fixtures cluster on thresholds

Second instance the same day, from the other side. Adding a majority-coverage
requirement to `snapshot_drift`'s gate (`0dbfd0ee`) turned **four** pre-existing
tests red at once — not one.

Every one had a fixture sitting **exactly at or below the new boundary**: bodies
carrying 2 of 4 rows, or 1 of 2. Each had been written to express *"a maintained
snapshot that fell behind"*, and each had silently been expressing the ambiguous
50/50 case instead, because that was the smallest fixture that produced the old
behaviour.

**The sharpening this adds to the entry above.** A fixture chosen for
**minimality** lands on the smallest input that exercises the current rule — and
the smallest input is disproportionately likely to be a boundary value for a
threshold nobody has introduced yet. So scaffolding-fixture breakage is not
randomly distributed across a narrowing: it **concentrates at the new threshold**,
and a narrowing that touches a ratio should be expected to redden every fixture
that was minimal in that ratio.

**Direction matters, and this one is the safe direction.** Narrowing pushes
boundary fixtures RED, so the suite reports them — four at once, unmissable. The
dangerous case remains the one this entry already names: widening leaves them
GREEN for a new reason, with nothing to notice. The remedy is unchanged — amend
the fixture to restore its reach (here: enlarge until the ratio is unambiguous,
and make it explicit in the helper's signature rather than hardcoded), never the
rule — plus mutation-verify, which held: reverting the majority test reproduced
the original false positive and failed only the test written for it.

**Do, additionally:** when a change introduces a *ratio* or *threshold* where
there was a boolean, expect the minimal fixtures to be the ones that break, and
state the ratio explicitly in the fixture rather than leaving it implicit in a
hardcoded seed. A fixture that leaves the ratio implicit lands on the wrong side
of a future gate without saying so.

## R-97 — A classifier you just wrote has been calibrated on exactly one case: the one that made you write it

**Verdict:** miss (self-caught on the second real input) → rule

**Observed:** 2026-08-16, BL-29's snapshot-drift work (`99aaf83f` → `0dbfd0ee`).

**What happened.** Shipped a `doctor` check whose gate was *"if the body
line-anchors at least one `PREFIX-N`, this tracker keeps a snapshot"*. It was
derived from, and verified against, `prompt-hamsa-audit-log.md` — the tracker
that motivated the bug. Pointed at the **second** real tracker,
`provenance-subsystem.md`, it was a false positive: that tracker is
params-canonical by design (its own § *PV-N entries* says *"the canonical PV-N
rows live in the augmentation params, not in this file"*) and merely mentions 14
of 68 ids incidentally, in the first cells of unrelated tables and four `### PV-N`
write-ups. The gate read that as a snapshot 79% behind, and acting on it would
have duplicated into the file exactly what its author deliberately kept out.

**The cheap part was available all along: the population was enumerable.** One
query over all 28 augmented trackers, and the answer is not a judgement call — it
is bimodal with no overlap:

| coverage | shape | what it was |
|---|---|---|
| 100% | contiguous prefix | 11 maintained snapshots, in sync |
| 61% | contiguous prefix `1..14` | the real lag — correctly caught |
| 21% | scattered, holes throughout | the false positive |

A snapshot is appended to, so it lags at the **tail** and still carries most of
its rows; a document that mentions ids carries a scattered minority. That rule
was *measured*, not reasoned out, and it took one script.

**Rule.** Before shipping a classifier over a corpus you can enumerate, **run it
across the whole corpus and look at the distribution** — not only at the case
that motivated it. Bimodal with a gap means you have a threshold. Overlapping
means you do not have a classifier yet and should say so rather than pick a
number. The motivating case is the *worst* possible validation set: it is the one
the rule was fitted to.

**Second instance, same day, same shape.** The drift audit itself. It read the
catalog directly via sqlite and reported *"3 of 28 augmented trackers have
drifted"* inside a codescout conversation. The catalog is machine-wide: the 28
span **seven repos**, only 11 are codescout's, and one of the three drifted
trackers belonged to `mirela/backend-kotlin`. The count was arithmetically
correct and the population it described was never checked — an instrument
validated on its output instead of on its input set.
`get_guide("tracker-conventions")` and the tracker-hygiene skill both warn about
exactly this for `librarian(action="doctor")`; the warning was on record and
unread (law G).

**Cost avoided.** Believing the first gate would have meant rewriting 54 rows
into a tracker whose author had documented, twice, that the rows do not belong
there — and then nagging every params-canonical tracker on every write forever.

**Kin:** R-96 (its sibling from the other end — that entry is what changing a
threshold does to *tests*; this is what an unvalidated threshold does to
*results*), R-92 (a filed root cause is a hypothesis — so is a gate you just
wrote), R-95 (a deferral rationale is a claim), R-90 (read what you actually
selected, not what you meant to select), law B (the instrument decides the
answer), law G (the answer may already be on record).

**Promote-when:** one more classifier shipped without a corpus-wide distribution
check. At that point it belongs in the skill's Phase 1 as an explicit step —
*enumerate the population and look at the distribution before trusting a gate*.

---
## R-98 — An id read from a scan is stale the moment a peer session writes

**Verdict:** miss (self-caught by a re-check) → rule · **Observed:** 2026-08-16, the
R-N index-coverage sweep, with a second session live in the same checkout.

**Seam:** allocating a monotonic id in a file another agent can write.

**What happened.** The sweep opened with exactly the both-formats count this file's
own id-suffix note prescribes — `grep -o '^## R-[0-9]*'` and `grep -o '^| R-[0-9]*'`,
max across both. It returned **96**. Some minutes and thirteen index rows later, the
same command, run immediately before writing, returned **97**: a concurrent session
had written `## R-97` into the working tree. Allocating from the opening scan would
have minted collision number ten in a ledger that had just spent an entire pass
suffixing the first nine.

**Why the prescribed check was not enough.** The note fixes the *pattern* — count both
entry formats — and that part worked exactly as designed. What it does not fix is
*when*. A max-id is a fact about a file at an instant, and every intervening tool call
is time in which a peer can invalidate it. The check is not a preflight; it belongs in
the same breath as the write.

**And `git` would have been wrong in the other direction.** At the moment of the
re-check, `git grep 'R-97' HEAD` returned nothing and `git log -S 'R-97'` named no
commit — R-97 existed *only* in the working tree, uncommitted. A collision check run
against committed state would have reported the id free while it was already taken.
The working tree is the authority for allocation; `git` is the authority for
provenance. Asking either one the other's question is the defect.

**The rule.** Re-run the id scan in the same breath as the write — not at the start of
the pass, and not against `HEAD`. If the two scans disagree, a peer is active in the
file: allocate above the new max, and do not commit that file until their work is
committed, or you annex it (R-90).

**Status:** open — single datapoint, but fully evidenced. `a1ac0317` is this sweep;
`2f94ce40` is the peer's R-97, committed minutes after the working-tree write that the
re-check caught.

**Second instance, same shape, self-inflicted (2026-08-16).** This entry originally
cited that commit as `c60242ac` — the SHA it carried when the log was read. The peer
then `git commit --amend`ed it, minting `2f94ce40` and orphaning `c60242ac`, which now
survives only in the reflog. The citation inside an entry *about* reading stale facts
from a shared checkout was itself stale within minutes, and would have rotted silently
— an orphaned SHA still resolves locally via reflog, so `git cat-file -t` says `commit`
and nothing complains until a gc or a fresh clone. This does **not** fire the
Promote-when below, which is scoped to id allocation; it shows that scope is too
narrow. The general form: **any fact read from a shared checkout — max-id, SHA, file
contents, `git status` — is a snapshot a peer can invalidate before you write.**
Re-read at the write, and prefer a stable handle (entry id, path, subject line) over a
SHA while a peer may still be amending. The sweep's own invariant (every body entry has an index row) then
survived that peer write untouched — they added both formats for R-97 without ever
seeing the commit, which is weak evidence that the convention is discoverable from the
file alone.
**Promote-when:** a second stale-max allocation in any ledger → promote to the
reconnaissance SKILL.md *alongside* R-90, not separately: both are the
concurrent-sessions-in-one-checkout family and share one remedy.

**Kin:** R-90 (two sessions, one working tree — the annexation half of the same
hazard), R-94 (a declaration inventory and a delivery inventory diverge), R-97 (a check
validated only against the case that motivated it), and law B — here the instrument was
right and its *timing* was wrong, which the law does not currently cover.

## R-99 — The convention lives in the template, or it does not live

**Verdict:** hit (root cause found by asking why three unrelated defects co-occurred)
· **Observed:** 2026-08-16, the R-N index-coverage + disposition sweep.

**Seam:** any long-lived ledger with an author-facing entry template.

**What happened.** Three defects were repaired in this file in one pass, and they
looked unrelated: 13 body entries with no Index row; 39 of 57 entries with no
disposition field; nine ids allocated twice. Each already had a *documented*
convention somewhere in the file — the Index section carries an id-suffix note that
prescribes counting both entry formats, and the distillation notes call adding a
`Status:` "the single highest-value structural change to this tracker".

**The root cause was one place.** The `## Template for new entries` block named
exactly one field, `**Verdict:**`, and mentioned the index row as a passive aside
("Also update the Index table row at the top"). It said nothing about `Status:`,
nothing about `Promote-when:`, and nothing about how to allocate an id. Every author
who followed the template produced an entry missing precisely what the template
omitted — which is the entire defect set, and nothing else.

**The generalisation.** A convention documented anywhere *other than* the artifact
the author actually copies is not a convention — it is a fact about the file that the
next author will not read. Prose elsewhere in the same file does not count: the
id-suffix note sits ~200 lines above the template, was written by someone who had
just finished repairing nine collisions, and collision #10 was still one stale scan
away (R-98). The test is not "is it written down somewhere"; it is "is it in the
thing that gets copied".

**Fixed here** by moving all three into the template — the both-formats id scan with
its re-run-at-the-write caveat, the five-column index row plus the `comm` check that
detects orphans, and `Status:` marked required with the reason it is required.

**Promote-when:** a second ledger (T-N / F-N / W-N / GF-N / BL-N) found to have a
defect class whose root cause is an under-specified entry template → promote to the
reconnaissance SKILL.md as a scout step: *before appending to an unfamiliar ledger,
read its template and diff it against what existing entries actually carry.*

**Status:** open — single datapoint, but one covering three independent defect classes
in a single file, and the fix is applied in the same commit. The template is now the
thing to audit, not the entries.

**Kin:** R-98 (the id half of this, and why the scan caveat now lives in the
template), R-94 (a declaration inventory and a delivery inventory diverge), R-97 (a
rule validated only against the case that motivated it — here, a template validated
against no case at all).

## R-100 — A pinned test's rationale is the cheapest refutation of your own bug

**Verdict:** miss ×2 (self-caught, one of them after acting) → rule · **Observed:**
2026-08-17, filing and then retracting the librarian-guard bug.

**Seam:** any claim that a guard, gate or validator "misses" a case.

**What happened.** Measured that 26 of 66 tracker and bug files carry no frontmatter
`id:` and are therefore invisible to `is_librarian_artifact`. Filed it as a bug, ranked
three fix options, recommended widening the predicate to `kind: tracker` — and, before
testing that recommendation, stamped `id:` into the two ledgers I cared about most.

**The refutation was one symbol away.** `src/util/librarian_guard.rs` carries a test
named `a_catalogued_but_unaugmented_file_stays_directly_editable`, whose doc comment
states the design outright: guarding by catalog *membership* would refuse
`docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR, so the predicate is **augmentation**.
It then names `docs/trackers/skill-frictions.md` as the example that must stay editable —
a file I had cited in the bug's own Evidence section as proof of the gap.

**What not reading it cost.** The stamp silently disabled `docs/TAXONOMY.md`'s documented
`edit_markdown` append path for R-N, this repo's most active ledger. Nothing caught it:
no test covers "the documented call still works". It surfaced only because I probed that
call directly, after the fact.

**Why the test was the right place to look, and why I skipped it.** A guard's *intended*
scope lives in the tests that pin its boundaries, not in the function body — the body
shows what it does, the test shows what it is meant **not** to do. And the miss was not
for want of proximity: `symbols(path=…)` printed that test's name in the very output I
used to read the predicate. The name alone contradicted my bug. I read the implementation
and skipped its neighbours.

**The rule.** Before filing "X misses case Y", enumerate X's tests **by name** and look
for one that pins Y as deliberate. A test whose name is a sentence about behaviour is a
design decision under version control. If such a test exists, the finding is not a bug —
it is a proposal to change policy, which is a different document with a different burden
of proof. (That reframing is what produced the better answer here: neither predicate
matches the damage, because every defect measured was entry structure in an *unaugmented*
file, and the missing concept is a ledger.)

**Second instance, same session, weaker form.** The corrected error hint I then wrote
prescribed `artifact_augment(id=…, merge=true, prompt=…)` for an artifact with no
augmentation — which refuses with "call artifact_augment first". A remedy that cannot run
from the state it describes. Caught by running it against the live tool rather than
re-reading it. Its regression test then fell into the *keyword* trap while guarding
against the *prescription* one, which is a third instance of law B in a single day.

**Promote-when:** a second instance of a filed defect refuted by its own subject's test
names → promote to the reconnaissance SKILL.md as a Phase-1 scout step: for any
gate/guard/validator claim, enumerate the tests before writing the finding.

**Status:** open — two datapoints in one session, both self-caught, one only after acting.
The expensive half (acting before checking) is remediated: `bb9a94d7` reverted the stamp
and restored the documented path.

**Kin:** R-97 (a rule validated only against the case that motivated it), R-95 (a
rationale nobody re-audits — here the rationale was sound and simply unread), R-92 (a
filed root cause is a hypothesis), R-99 (the convention lives where authors look), and
law G (the answer may already be on record).

## R-101 — A test that DISTINGUISHES two hypotheses is not confirmation of one — check which way it points

**Observed:** 2026-08-17, filing
`docs/issues/2026-08-17-audit-doc-refs-misreads-include-str-arg-as-doc-relative.md`. A
`high` audit finding flagged a correct doc; the ref was `./render_template.j2`, quoted from
an `include_str!` argument. I hypothesised the resolver joined the ref to the *markdown
file's* directory rather than the Rust source's.

**The test I ran, and recorded as confirming:** rewrite the ref repo-relative, re-run the
audit. Result: `n_refs_resolved` 17477 → 17478, `n_refs_broken` unchanged. I wrote
*"confirmed — the base directory was the whole of it"*.

**Why that is backwards.** Under a markdown-relative join the base would have been
`docs/architecture/`, so the rewritten ref would have resolved as
`docs/architecture/src/librarian/…` — which does not exist, and the ref would have stayed
broken. It resolved. So the outcome *rules out* the markdown-relative base and establishes
`repo_root`. The measurement was correct, discriminating, and already in my hands; I read
its sign wrong, in the direction I had already committed to in prose two paragraphs above.

The real mechanism needed two halves, neither wrong alone: `resolve_file_path` joins
`repo_root`, where a leading `./` buys nothing; and `try_basename_fallback` opened with
`if raw_ref.contains('/') { return None }`, so the ref was disqualified from the fallback
**by the very prefix that made the positional attempt fail**. Corrected in `da55100a`,
which also found that my sketched parser-side fix was unreachable — `tokenize_code_span`
splits on `(` and `"`, so the macro context is gone before classification runs.

**The pattern.** Before recording a verdict, state what the *other* hypothesis predicts for
the same test. If both predict the observed outcome, the test discriminates nothing and the
verdict is unearned. If they predict opposite outcomes — the good case — then the test is
strong, and that is exactly when reading its sign from the direction you already lean is
most costly, because a strong test wrongly signed produces a *confident* wrong answer
rather than a weak one. A confirmation you can write without naming the counter-prediction
is a coherence check on your own prose, not a measurement.

This is the Conclude Last rule's twin, one level in: Conclude Last stops you narrating before
you evaluate. R-101 is the failure that survives it — I *did* evaluate, and evaluated a
real measurement, but scored it against one hypothesis instead of two.

**What limited the damage:** the bug file marked the root cause **inferred, not cited**,
naming the two files whose branches had not been read and adding a `## Resume` saying *"the
behaviour is measured; the code path is not."* `da55100a` opens by citing exactly that
marking — *"The bug filed this root cause as inferred and said so. Reading it corrected the
mechanism."* The prescription survived the wrong mechanism: option (b), fall through to the
basename fallback, was adopted verbatim. Labelling an inference as an inference is what
makes a wrong mechanism cheap to correct instead of something the next session builds on.

**Detector, usable in one line:** for every `**Verdict:** confirmed`, write the sentence
*"under the rival hypothesis this test would have shown ___"*. If that sentence cannot be
completed, the verdict is not yet earned.

**Status:** open — recorded 2026-08-17, single datapoint. Promote to the Hypotheses-tried
section of `docs/issues/_TEMPLATE.md` when a second verdict is found to have been scored
against one hypothesis; the template's **Verdict** field is where the counter-prediction
line belongs.
## R-102 — A root cause read from code is a hypothesis about which true statement is the operative one; implementing the fix is the measurement

**Observed:** 2026-08-17, working three bug files in one session. All three had a root
cause written from careful reading of the code. All three were wrong in a way only
*building the fix* exposed — not one was caught by re-reading.

**The three, because the shapes differ and that is the finding:**

1. **A negative claim about mechanism.**
   `2026-08-15-read-only-metadata-commands-blocked-on-source-paths` rebutted a reporter's
   "`wc` is in the block list" with *"There is no command list … a path predicate … there is
   no per-command carve-out."* `check_source_file_access` is a **two-part** predicate and
   `SOURCE_ACCESS_COMMANDS` is half of it. The rebuttal was more confident than the claim it
   corrected, and more wrong. Its repro was misattributed too: `ls … && sed …` blocked on
   `sed`, not on the `ls` it blamed.

2. **A correct mechanism with both prescriptions wrong.**
   `2026-08-15-read-file-force-ignored-on-full-reads` read `read_full_file`'s early return
   accurately and offered A (make `force` defeat the size budget) or B (declare it
   range-only). B was already shipped in the schema and Iron Law 1; A would have defeated
   progressive disclosure. The live defect — the parameter accepted and dropped in silence —
   was named by neither option.

3. **A diagnostic that does not discriminate.**
   `2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary` tabled
   `frontmatter_max > body_max` as meaning the ledger was compacted. It is also true
   immediately after *any* ordinary reservation, because the reservation writes the mark.
   The table looked like evidence and was a plausible reading.

**Why reading cannot catch these.** Reading tells you what the code *says*. Every one of
these root causes was a true statement about the code. What reading does not tell you is
which of several true statements is the **operative** one — which predicate half fires,
which of two documented behaviours is already shipped, which relation actually separates
the cases. That is a question about execution, and only execution answers it. A second
re-read is the same reader consulting the same belief; it cannot audit itself.

**What made all three cheap to correct: the label.** Each carried, or should have carried,
the template's *"inferred from `src/x.rs:12` — not measured"* line. `da55100a` opens by
citing exactly that: *"The bug filed this root cause as inferred and said so. Reading it
corrected the mechanism."* The prescription survived the wrong mechanism — option (b) was
adopted verbatim — because the document had already told the next reader which sentence to
distrust. An unlabelled wrong mechanism is not corrected; it is **built on**.

**Detector, and it costs nothing.** Before filing, mark every root-cause sentence as
measured or inferred, and prefer the inferred label when unsure. Then, **when the fix is
implemented, re-read the root cause** — implementing *is* the measurement, and it is the
last moment the correction is free. Three for three today; in every case the disproof
arrived while writing the fix, not while reviewing the file.

**The sharpest single tell:** a root cause that asserts an **absence** — "there is no
command list", "there is no carve-out", "nothing consumes this". Reading establishes
presence; only a search establishes absence, and the two feel identical from inside the
file you happen to be reading. Datapoint 1 is this exact shape, and it read as the most
authoritative sentence in the document.

**Relationship to R-101:** sibling, one layer out. R-101 is about mis-scoring a test you ran. R-102 is about not
having run one — and about the labelling that keeps that survivable.

**Status:** open — recorded 2026-08-17 with three datapoints from one session, which is
already past the usual promotion bar. Promote to `docs/issues/_TEMPLATE.md` by making the
measured/inferred split a required sub-field of `## Root cause` rather than a paragraph of
guidance inside it, and add a Fix-time step: *re-read the root cause once the fix compiles.*
## R-103 — A blast-radius audit is not a correctness audit, and enumerating the call sites makes it feel like both

**Observed:** 2026-08-17, fixing
`docs/issues/archive/2026-08-17-audit-doc-refs-misreads-include-str-arg-as-doc-relative.md`
(`da55100a`). The change altered `basename_candidate`, the eligibility rule shared by
`try_basename_fallback` and `unique_basename_path`.

**What I did, and it was the right method.** I enumerated every caller — `resolve_file_path`,
`resolve_file_line`, `resolve_link`, `resolve_file_symbol` — and read each one. That audit
paid: `resolve_link` anchors `./` to the markdown file's own directory, so stripping `./`
for fallback purposes would have let a same-basename file anywhere in the tree satisfy a
broken relative link. I guarded it and mutation-verified the guard.

**What I missed, in lines I had just read.** `resolve_file_symbol` called
`unique_basename_path` and fell through to `FileMissing` — and that helper returns `None`
for **both** zero matches and two-or-more, so a file existing *twice* was reported as gone.
A pre-existing defect, independent of my change, sitting in the exact `else` arm I had read
to decide my change was safe there. The peer session found it hours later (`3faddb15`).

**Why complete coverage did not help.** The population was right and every member was
visited. The *question* was singular: **"does my change break this caller?"** I never asked
**"is this caller correct?"** Those are different audits over the same list, and doing the
first thoroughly produces the felt sense of having done both — enumerating the call sites
is the expensive part, so once it is done the mind treats the list as discharged.

**The tell that suppressed the second question, and it is the sharp half.** The comment
directly above that code asserted the invariant:

> Same basename shorthand `resolve_file_path` and `resolve_file_line` accept … Without this
> the three ref kinds disagree about identical path parts.

I read that as evidence the parity existed. It described **intent**, not behaviour — only
the unique half was implemented. `3faddb15`'s own comment names it: *"The comment above used
to assert this while only the unique half was implemented."* A comment claiming an invariant
is the cheapest possible false positive: it sits exactly where you would look to verify, and
reads as confirmation.

**Counterfactual.** `FileMissing` carries gating severity — it is the verdict that failed CI
in the parent bug. Any doc citing `mod.rs::helper` where `mod.rs` is ambiguous would have
gated the build on a correct doc, which is the same class of false positive the parent bug
was filed for. Cost was low only because a second session read the same code with a
different question.

**The rule.** When a change forces you to enumerate a caller set, make **two** passes over
it and name them separately:

1. *Does my change break this caller?* — blast radius.
2. *Is this caller correct, independent of my change?* — correctness.

And treat any in-code comment asserting cross-site parity as a **hypothesis to check**, not
as evidence. You are already reading those lines; it is the cheapest moment to check the
claim and the moment you are least likely to, because the comment is answering the question
you came with.

**Relationship to R-101 and R-102.** Same family, third distinct failure. R-101 mis-scores a
test that was run. R-102 never runs one and survives on the inferred label. R-103 runs the
right audit over the right population against too few questions. In all three the instrument
was sound and the framing was not — which is why none of them is caught by re-reading more
carefully.

**Status:** open — single datapoint, fully evidenced (`da55100a` is the incomplete audit,
`3faddb15` the correction, and both comments are quotable). **Promote-when:** a second
blast-radius audit is found to have missed an independent defect in a site it visited — then
promote to the reconnaissance SKILL.md Phase 1 as an explicit two-pass step, since Phase 1
currently says "read callers if shape changes" and that phrasing asks only question 1.
## R-104 — A zero from a report is a claim about your query, not about the world — and it lies in three independent ways

**Observed:** 2026-08-17, across one session of `audit_doc_refs` and `link_scan` work. Five
wrong conclusions, all the same shape: a query returned nothing, or returned a number, and
I read it as a fact about the corpus.

**Pattern:** a findings-array query can answer confidently and wrongly for three reasons
that look identical from the outside. Each was hit at least once:

| Failure | Instance |
|---|---|
| **wrong key name** | `grep '"token":"HY-…"'` returned nothing and read as *"no HY token is broken"* — the `dangling` array called the same field `raw`. Half the population was never searched. |
| **wrong value vocabulary** | filtered findings on `verdict != "ok"`; the domain is `resolved \| missing \| resolved_basename \| file_missing \| unknown`. Printed **resolved** refs as problems. |
| **cap truncation** | looked for a ref in `findings[]`, did not find it, nearly concluded it resolved. `n_refs_found` was 64 against a 50-entry window — the ref could simply have been outside it. |

Two more of the five were the same disease outside tool output: `grep -c 'Status:'` counted
prose *about* Status, and `status: mitigated` in a "what's open" query returned 25 rows of
which 10 were already archived — a 2.5× inflated answer that looked authoritative.

**Why it survives care.** Every one of these queries was *structurally* anchored, which is
the rule `get_guide("tracker-conventions")` § *Detecting these fields* already teaches. The
technique was right; the **domain** was guessed. Anchoring on `"token":` instead of the word
"token" protects against prose contamination and not at all against the field being called
something else. So the existing rule is necessary and insufficient, and this entry is the
missing half.

**The check, in one line before believing any zero or count:** *did I read the key name, the
value domain, and the cap from the actual payload — or did I supply any of the three from
memory?* One `read_file("@tool_x", json_path="$.findings[0]")` answers all three, and it is
how each of the five was eventually caught.

**What made the difference in practice** was never re-reading the same query more carefully.
It was printing one whole record and looking at its keys, or shrinking the input until the
window could not truncate — the four-line fixture in
`docs/issues/archive/2026-08-17-audit-doc-refs-claims-file-missing-for-an-ambiguous-basename.md`
exists for exactly that reason. Kin to R-101: there the measurement was in hand and scored
against one hypothesis; here the measurement was never of the thing being claimed.

**Status:** open

**Promote-when:** a sixth instance, OR the first time a *report* carries its own key/value
legend and the reader still guesses. The substrate is already moving that way — `f908e883`
added `severity_legend` to `audit_doc_refs` and `7c218338` unified `link_scan`'s field names
— so the honest test of this entry is whether self-describing output retires it. If it does,
promote as a note on `docs/PROGRESSIVE_DISCOVERABILITY.md`'s legend pattern rather than as a
discipline agents must remember.

## Template for new entries

<!-- Insert new R-N entries above this line.

  1. GET THE ID FROM THE SERVER — do not compute it:

       artifact(action="append_entry", id="5696563f06b2c222", id_prefix="R")

     No `entry_collection`: this is a prose ledger, so the call reserves the next
     id under a transaction and writes nothing. The manual both-formats scan this
     step used to prescribe is obsolete — and it WAS the race. A max read at the
     start of a pass is stale by the time you write (R-98: a peer took R-97 with a
     four-minute margin), and allocating from a stale max is how the first nine
     collisions happened. Never suffix an id either: `R-72b` is not a valid entry
     token at all, since digit→letter is not a word boundary.

  2. WRITE THE SECTION. `edit_markdown` is refused here — the ledger is augmented —
     so use:

       artifact(action="update", id="5696563f06b2c222", patch={body_edits: [
         {heading: "## Template for new entries", action: "insert_before",
          content: "## R-N — title\n\n"
                   "**Verdict:** hit | miss [×N] → rule · "
                   "**Observed:** YYYY-MM-DD, <context>\n\n"
                   "**Seam:** <what was unverified>\n\n"
                   "<narrative>\n\n"
                   "**Promote-when:** <falsifiable criterion>\n\n"
                   "**Status:** open — <N datapoints>\n\n"
                   "**Kin:** R-x, R-y\n"}]})

     The heading must be `## R-N — <title>` exactly. `link_scan`'s `def_re` is
     `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`, so `## R-100` alone defines NOTHING and
     every citation of it dangles. A first cut of the 2026-08-17 archive migration
     used bare headings and pushed the project's dangling count UP, 720 → 761.

  3. ADD THE INDEX ROW in the same call — five columns, matching the header:

       | R-N | YYYY-MM-DD | verdict | pattern | evidence |

     EVERY body entry must have one. Verify with:

       comm -23 <(grep -o '^## R-[0-9]*b\?' <file> | sed 's/^## //' | sort -u) \
                <(grep -o '^| R-[0-9]*b\?' <file> | sed 's/^| //'  | sort -u)

     Empty output = clean. This check found 13 orphaned bodies on 2026-08-16.

  REQUIRED FIELDS — `**Status:**` IS NOT OPTIONAL. It is the disposition field,
  and it is the only thing that makes a fired `Promote-when` harvestable. Its
  absence is why 39 of 57 entries carried no disposition and why criteria went
  unharvested for three months. Write it even when the answer is
  "open — single datapoint".

  WHEN A CRITERION FIRES, UPDATE THE STATUS LINE. Recording a firing only in
  prose leaves it invisible to every field-presence sweep — that is exactly how
  R-89, R-90 and R-91 sat fully adjudicated and uncounted. And note the trap that
  cost three probes across 2026-08-16/17: detect these fields by STRUCTURAL
  anchor (line-start, key prefix), never by keyword. Prose and field share a
  vocabulary by construction, so `grep -c 'Status:'` also counts sentences *about*
  Status, `/fired/` matches "the tell that should have fired", and a test
  asserting a hint does not say `merge=true` fails on the hint that warns against
  it.

  Why this block carries all of it: R-99. A convention documented anywhere other
  than the thing authors copy is not a convention. -->
