---
kind: tracker
status: active
title: Reconnaissance patterns
tags:
- reconnaissance
- skill-meta
- scout
entry_high_water_R: 181
entry_prefix: R
expects_augmentation: docs/augmentations/docs-trackers-reconnaissance-patterns.yaml
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

**Chain.** R-3 → R-113 → R-77 → R-79 → R-87 — the entries themselves label these
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

**Exemplars.** R-9, R-68, R-112.

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
   suffixes go by date, so its `R-73b` and this file's `R-113` are different
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
> **2026-08-21 correction.** The seven suffixed ids that survived (R-55b, R-57b, R-58b, R-72b,
> R-73b, R-74b, R-76b) were STILL not valid entry tokens under this grammar — suffixing never
> fixes that, regardless of which instance keeps the bare number. Renumbered to fresh ids
> (R-109–R-115) instead, since the "preserve which instance a citation meant" benefit this
> note describes turned out to be false at the resolver: the bare-number instance was usually
> the row-only one, so citations resolved to nothing rather than to the intended lesson. See
> `docs/issues/archive/2026-08-16-reconnaissance-ledger-reuses-ten-ids-for-different-lessons.md`.
>
> **Seven of the nine survive in-file (checked 2026-08-16).** R-56b and R-59b were
> split by `52fca682` and then archived by `b6bb6377`, the first R-N archive pass —
> so a both-formats count returns seven, not nine. The list above is the *record of
> the split*, not an inventory of what the file now holds; read it as history. This
> is R-94's law applied to this note — a declaration inventory and a delivery
> inventory diverge, and no gate connects prose counts to the entries they count.

| ID | Date | Verdict | Pattern | Evidence (session-log) |
|----|------|---------|---------|------------------------|
| R-180 | 2026-09-04 | miss ×4 → rule | **A unitless count crosses a session boundary as a cost estimate; the instrument built to replace it inherits the claim's bias; and a partition that SUMS proves nothing when its total shares the buckets' predicate.** Published *"~50 in-tree `ToolContext` constructions, all `#[cfg(test)]`"* from a **capped** grep whose pattern also matched `struct`/`impl`/`->` headers — files-with-a-match, quoted as constructions, over codescout's own *"50 across 50 files is a floor, not a count"* warning. The cost was not "a reader re-derives it": a peer **spot-checked 2 of ~20 files, deferred to a wider sweep that did not exist**, and re-priced an architecture decision on it to their user — two sessions holding one wrong number with **real** agreement about the true half beside it. The replacement classifier then invented production sites **twice**, both toward findings: `\b(struct|impl|->)` never fires on `) -> ToolContext {` (space→hyphen is no word boundary), and `#[cfg(test)]` on an `impl` read as production to mod-shaped detection — neither on the blind-spot list I had written in its own header. **Then the fix was the same error one level up:** "two instruments sharing no code both said 202" — both were `\bToolContext\s*\{`, one predicate run twice. The peer's **substring** predicate said **203**, simultaneously (HEAD `531d7ee3` at 15:53:48; `git status` clean of `.rs` at 16:01:19 — so **not** the churn they proposed), and the gap was one line: `Arc::new(LibToolContext {`, an alias where `\b` fails because `b` is a word character. That `\b` was right for the core bucket and silently corrupted the librarian one, which is why it survived — an error protecting the number under scrutiny is one nobody checks. Corrected: 60 headers + **1** core prod + **136** core test + **2** librarian prod + 4 librarian test = 203, so **137** core sites, ~3× the first figure. **Runnable:** a partition proves completeness only when its total comes from a different PREDICATE, not merely different code; name the predicate beside the number; localise a peer's differing count before blaming timing (a simultaneity check is one call); and test a load-bearing exclusion regex against the construct it excludes. | `7320d27d`-adjacent; script `scratchpad/toolctx_literals2.py`; peer `codescout-ae` supplied the independent predicate; kin R-179, R-177, R-178, R-3/R-113, R-5 |
| R-179 | 2026-09-04 | hit → widens the deferral-rationale law (1 datapoint for the new half; 10th for the cost half) | **Discharging a deferral's prohibition can hand you the fix's best argument — which the prohibition could not have contained.** A bug file forbade its own one-line fix pending two checks, one of which named *"the in-process subagent path"*. Both cleared in ~10 min, and that path **does not exist**: `Server::build_context` is the only production constructor of a core `ToolContext` (~50 others all `#[cfg(test)]`), so a subagent's calls arrive through the same `call_tool_inner` and its pin is a per-call argument, never inherited state — `R-117`'s empty-population shape wearing a prohibition's clothes. **The new half is what the check RETURNED:** the other prerequisite surfaced `call_tool_inner` already granting a pinned write-tool call write residency under a comment reading *"the pin itself already is the caller's consent"*, so the guard was refusing what the layer directly above it had accepted. That converts the change from *"new policy on a write path, proceed carefully"* to *"restore consistency inside one call path"* — an argument a stop-rationale cannot hold, because whoever writes one has stopped reading, and the fact lives one layer up. **Tell:** a rationale that reasons entirely inside one function while the question is a contract between layers. | `7320d27d` + archived bug `2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`; kin `R-117`, `R-49`, and the skill's deferral bullet |
| R-178 | 2026-08-31 | miss → rule (3 instances, one session) | **`origin/<branch>` is a LOCAL cache, so "N unpushed" without a fetch is a claim about your last fetch.** Reported "2 unpushed", then "1", then "3" from `git rev-list --count origin/experiments..HEAD`; a fetch showed **ahead 0, behind 18** — a peer had already pushed the branch, carrying my commits, because `push` sends the whole branch ref not one session's work. Two consequences followed within the hour: a peer **rebase orphaned 3 of my 6 commits** minutes after I cited them by SHA (recovered by patch-id — byte-identical diffs under new SHAs, which is the SHA+patch-id rule paying off inside one afternoon rather than across a release), and "I committed it" proved a different claim from "it is still at that SHA". **Runnable:** `git fetch` before quoting any ahead/behind number; `git ls-remote origin refs/heads/<branch>` when it matters. Temporal row of PROBES rule 6 — an instrument reporting perfectly on an instant that has passed; it fired twice this session, here and on a frozen 106-session frame that had lost 76 members to disk cleanup in four days. | this session, caught only when the user said "push"; kin R-136, R-146 |
| R-177 | 2026-08-31 | miss → rule (4 defects, one instrument) | **A new instrument's most persuasive output is the one that confirms the hypothesis you built it to test.** A probe written to decide whether to split `tracker-conventions` reported, before any positive control, **31% of the topic never engaged** — exactly what a would-be splitter wants, and fabricated: a fenced worked example teaching the `## <ID> — <title>` syntax was parsed as a real heading, inventing a 12,124 B phantom section and STEALING those bytes from the real one (17,323 → 5,116). The control found three more: signature strings matched against any tool input made the `create_file` that wrote the probe score all six sections 100%; `tags: [append_entry, …]` on a bug filed *about* the librarian credited untouched sections; and blending two populations gave 71.7%, coincidentally matching a prior study's **71%** and reading as cross-instrument convergence — **a spurious corroboration is worse than a spurious number, because it recruits a second instrument as a witness**. Split, the populations are 45% and 92%. All four returned plausible, well-formatted, wrong answers, and three pointed the author's way: defects producing implausible numbers get caught by reading the output, so the survivors are SELECTED for plausibility. **Runnable:** run the positive control before the first real run, not after the first surprising one; cross-check a derived count against an independent artefact's structural invariant (the phantom was localised by a 7-vs-8 section-COUNT disagreement, not by re-reading the parser); and when the remedy is "remember to split", put the split in the instrument. | this session; instrument `scripts/probe_guide_section_use.py`, defects recorded at the lines that fix them; kin R-138, PROBES rules 4 and 6 |
| R-176 | 2026-09-02 | miss, self-caught on a re-run — and it nearly cost a correct peer report (1 instance; a second session hit the same thing and was saved by a guard) | **A pipeline's `$?` is the LAST stage's status.** Verifying a peer's claim that a pre-commit hook was failing, I ran `python3 hook.py 2>&1 \| tail -15; echo "exit=$?"` and got `exit=0`. The hook was exiting **1** the whole time — `$?` reported `tail`'s status. **Severity is that I almost dismissed an accurate report on it:** the green was not a weak signal to weigh but a confident, specific, wrong answer, arriving attached to real output that made it look corroborated. **Sibling of `CLAUDE.md`'s `&&` gate ruling** — a composition operator silently changing *which command's status you read*, failing in the direction that reads as "nothing wrong". `\|` is the worse of the two: `&&` at least visibly skips a command, whereas a pipeline runs everything and hands you a status belonging to a different program, so there is no missing output to notice. **The tell is that the exit code and the printed text disagree**, and a reader who checks only the code never sees the text contradicting it. **Remedy is structural:** redirect and read `$?` directly, or `set -o pipefail` (preferred over `${PIPESTATUS[0]}` — no index to go stale when a stage is added). **The non-obvious half:** the peer's own check came out RIGHT, not from care but because codescout's IL-3 guard refused their unbounded pipe and forced a redirect. That guard is documented as **context economy**; nobody would look in it for exit-code integrity, and a guard whose second benefit is undocumented gets removed for failing to justify its first. | `R-171` twin (a bound cutting the payload; this cuts the *status*), `R-174` (instruments in different worlds), `OB-1` § *the third position*; peer `codescout-5e` supplied the guard observation |
| R-175 | 2026-09-02 | near-miss, caught by a pre-implementation scout of the write sites (1 instance) | **A gate quantified over a population you never enumerated is a hypothesis in gate's clothing.** A spec written ~1 hour earlier, same session and author, specified a gate as *"no registered `ledger_prefix` is a prefix of another"*. Two `grep`s over every `GuideLedger` mutation returned **six** production writers where the spec assumed two — and the session opener stamps `SESSION_OPENING_GUIDE`, a bare topic name `guide-sections` also owns, an overlap argued deliberately at the site. **The spec contained no wrong fact**; every sentence was true of the two engines it had examined. The error was generalising a rule over *all* engines from a population never counted. **It would have red on correct code the day it landed**, and the cheap repairs at that moment — delete the overlapping registration, or widen to a negative predicate — both erase the finding and one makes the gate permanently unfalsifiable. **Second-order:** the same enumeration surfaced a **seventh engine**, invisible to the prompt-surface inventory the roster had walked; the discriminator was sound and the *instrument* was narrow, so "six" was a count of what one instrument could see rather than of a closed set. | spec `0021bead4e5a01e2` § *Gates*, which carries the correction inline rather than shipping the fixed form silently; `64a0a64c`; kin the `serves:`-coverage gate built as a finite 88-row checklist for the same reason |
| R-174 | 2026-09-02 | near-miss, caught by an instrument DISAGREEMENT run for an unrelated reason (1 instance) | **A line number is a coordinate in a WORLD, and on a shared checkout the tools do not share one.** Writing a spec closure whose whole argument is two call sites, I cited `types.rs:1076-1083` and `:1063-1066` from codescout's `grep`/`read_file`. Chasing an unrelated question I ran `symbols` and `git show HEAD:` on the same file and got **1484** where `grep` said **1278**: the file was `M` and **206 lines shorter than HEAD** (1321 vs 1527), a peer mid-refactor extracting `guide_emit.rs`. `grep`/`read_file` read the **worktree**, `symbols` the **AST/LSP index**, `git show HEAD:` the **commit** — three instruments, two worlds, **no error from any of them**; the numbers described code that exists in no commit and may never. **Not simply the promoted substrate law — its REMEDY fails here.** That law says to read the instrument's `loaded N from X` preamble and reconcile it; codescout's navigation tools print no preamble and no N, so the prescribed check is a **no-op** against the tool this project uses most — `OB-1`'s *supplied-and-unread* shape, where a remedy that cannot apply is worse than none because it reads as covered. **And the catch INVERTS the usual worry:** agreement would have been the failure (`grep` and `read_file` share a world, so they corroborate perfectly while both describe uncommitted code) and the *disagreement* with a differently-scoped instrument carried the information. **Remedy, unconditional and cheap:** resolve any line ref against `git show HEAD:<path>` before it enters a spec, bug file, tracker or commit message, and say which world it names — HEAD is the only one of the three that is stable and shared with the reader. **Second-order:** a normal stale ref is *decay* (`audit_doc_refs`, `DC-N` cover it); this one is **false at the moment of writing** and no freshness check catches it, because re-running the citation's own instrument reproduces the same number. | `07a808c0`; kin `R-89` (a fourth freshness axis — the copy that ANSWERS you, not the one that serves you), `R-170`, `OB-1` § *the third position* |
| R-173 | 2026-09-02 | miss, unrecoverable — emitted before it was noticed (1 instance) | **A classifier's REJECTED branch is where the leak is — scan a config for a property, print the key and the verdict, never the value.** Measuring `IC-4`'s env surface, a loop asked only *"does each path-valued var in `.env` still resolve?"* — and its `*)` arm, written as a courtesy to show what was skipped, printed a credential in full into a transcript. `.env` is gitignored and untracked, so nothing reached git; the exposure was created **entirely by the read**. **The asymmetry is structural:** the accepted branch is what the task is about and gets the attention, so the else-arm is written as an afterthought and reviewed as one. `.env`, `settings.json`, `.npmrc`, `~/.cargo/credentials` and CI variable dumps are all files whose *shape* is the subject of routine questions and whose *contents* are not. **Remedy:** emit the value only in the branch where the value IS the finding — a missing path must name itself to be actionable, a resolving one need not. **Checked, and not a repo defect:** no script under `scripts/` reads `.env` or dumps `key=value`; the hazard lives in ad-hoc shell written for a one-off question, which is exactly the code no reviewer sees. | pairs with `R-171` as its opposite — that one is a read emitting too LITTLE (a bound cutting the payload, an absence then acted on), this one a read emitting too MUCH in the arm chosen for completeness; both from fixing the output shape before knowing the input |
| R-172 | 2026-09-02 | hit, narrowly — caught by an isolating re-run before sending (1 instance) | **A result that falsifies a documented invariant is usually the invariant's already-recorded RESIDUAL.** The full four-command gate, run in the documented order, gave 10 of 11 `cli_artifact` failures — the librarian-less binary — against `CLAUDE.md`'s bold claim that *"following the gate cannot arm the trap … provided both lanes actually run"*. Both ran, both exited 0, and a draft saying the claim was false got written. Re-running the two lanes ALONE: lean leaves it lean, default restores it, exit 0, 11/11 — the ordering is sound and a peer's concurrent lean lane had landed inside the window. **What makes it an entry: the corpus had predicted it three days earlier, in a field built for exactly that, and reading the claim could never have found it.** The fix's own bug file (`…-shared-target-dir-feature-clobber-reds-the-cli-tests.md`, `status: fixed`, **archived**) carries in `unverified:`: *"the fix closes the TERMINAL state only, not the window … two sessions gating concurrently still collide and nothing detects that."* The residual is invisible to the canonical triage query by construction — archiving is the normal end state — so the check is `find(kind="bug", include_archived=true, …)` and then read `unverified:` before writing the word "false". **Not `R-163`:** there the attributed cause was the unchecked claim; here observation and cause were both right and the error under construction was the CONCLUSION'S SCOPE — a true local measurement generalised into a falsification, by an instrument that could not see the concurrency it was subject to. | residual confirmation recorded on `d2b0e9c1b9802432`'s `unverified:`; gate green at 01:32; kin `R-166` (the residual survived only because it was in a queryable field, not a commit message) |
| R-171 | 2026-09-02 | miss ×3 in one session, third caught in draft (3 instances) | **A bounded read returns an absence indistinguishable from a real one — and `tail -c` on one long line cuts the OTHER end.** Three reflexive bounded reads of output whose value was in the part the bound removed: `cargo test … \| tail -8` lost the failure message of a test whose doc comment says the message names the offending file; `\| tail -3` lost which 2 of 18 failed; and `grep -o '…' \| tail -c 1300` cut the **head** of a single long line, producing a drafted accusation that a peer's edit "isn't where they say it is" — it was exactly where they said. **The law is not about `tail`:** the bound was chosen before the shape was known, so the negative it returns is acted on as a real absence. `tail -c N` on one line is sharpest, silently reversing which end is kept. **"Be careful with flags" is the wrong remedy** — all three were the progressive-disclosure habit correctly applied to the one output class where the payload IS the tail of a long thing. **Mechanical remedy:** redirect diagnosis-bearing output to a file and query it, never pipe to `tail`; this is already the Iron-Law-3 pattern, and the gap is that it reads as a VOLUME rule when this is a SHAPE rule, so it never fires for a 3-line pipe. Adjacent but explicitly **not** an instance: six commit outputs printing `Stashing unstaged files …` were fully present, read, and pre-classified as chrome — unread differs from uncaptured and this remedy does nothing for it. | `R-3` twin (disciplines the reader of a result; this disciplines the reader's INSTRUMENT); kin `R-167` (unanchored over-match) as its opposite — that one admits too much, this one shows too little |
| R-170 | 2026-09-02 | near-miss, caught before any write (1 instance) | **A coverage ratio is a scope question before it is a drift finding — and the scope lived in the ENFORCEMENT layer.** Auditing `docs/issues/` for missing `cluster/<slug>` tags: live 34/34 tagged, archive 156/529, by month 0% → 33% → **42% (2026-08)** → 100%. That reads as drift, and I had a 236-file retro-tagging campaign half-composed. `tests/issue_clusters.rs`'s module header says the opposite in writing — the archive is out of scope by design, *"279 archived files in the backfilled window match none of them… forcing a fit would corrupt the counts that promotion reads"* — so the campaign was the specific thing already considered and ruled out, and would have inflated every `IC-N` with non-members. **The structural cause outlives the escape:** `issue-clusters.md` is where a count is READ, and it guards exactly one failure mode (*"trust the query; re-run it before trusting the count"*) — defending the count's freshness while saying nothing about its population, so re-running as instructed yields a fresh number still scoped to a 34%-tagged corpus, and the instruction to trust it is what stops you asking. **A number and the scope that validates it must co-locate at the point of READING, not of enforcement.** Mirror of the standing law: that one disciplines a prohibition you HAVE read; here the tree held one I had not, and my proposal was exactly what it forbade — so grep the enforcement layer (`tests/`, `scripts/pre-commit-*`, hooks) for the population's name, not only the docs. Cheap tell, no judgement needed: a ratio neither ~0% nor ~100% is a boundary someone drew, not drift. | `85915e8b`; two instruments of genuinely different scope (file frontmatter vs the catalog's `artifact.tags`) agreed on all 563 files; the scope was found by opening `tests/issue_clusters.rs` for an unrelated reason |
| R-169 | 2026-09-02 | miss, self-inflicted (1 instance) | **Running the population check can make the attribution error MORE likely, not less — because it retires the feeling of not having checked.** I ran `/codescout-companion:reaching-peer-sessions`, got the correct socket-scoped table (16 sessions across 3 profiles; 6 in this checkout = 5 peers plus me), then attributed file authorship **by adjacency anyway**: everything in `git status` I had not written became "the peer's", meaning the one peer I happened to know about. Three of the six files were a different session's; `scripts/file-provenance.py` partitions them cleanly into two author ids. I shipped the wrong partition to my user *and* to the peer, in a message whose stated purpose was shared-checkout hygiene. **The enumeration ran, succeeded and was reported accurately — it answered *who is present*, and I used it for *who wrote this*, a different question with a different instrument.** The skill's own text says so verbatim (*"Enumerating a complete set still only bounds who was present — it does not attribute a write. To attribute one, ask."*) and was in my context when I wrote the message. | `88355d1d`; `scripts/file-provenance.py`; CLAUDE.md § *Reaching a Peer Session* ("Never route by adjacency") |
| R-168 | 2026-09-02 | hit, scout refuted the sentence about to be written (1 instance) | **An instrument's report can EXPIRE — the loud state converts itself to silence on a timer.** Building `IC-4`'s worktree-gitdir surface, I was about to write *"the gate scans the filesystem because `git worktree list` cannot see this"* — clean, plausible, and reading as measured because the archived instance (`2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path`) does record an orphan the list omits. The probe refuted it: **immediately after a repo rename git DOES report it**, tagged `prunable gitdir file points to non-existent location`. Transcript and probe disagreed and both were right — about different *times*. The third party is a clock: `git gc` runs `git worktree prune --expire 3.months.ago` (`gc.worktreePruneExpire`) and **deletes the admin directory**, for a worktree whose files are still on disk, because git judges it by a path that moved. The archived bug is the post-expiry state; the rename was three months prior. **Why this is worse than a narrow instrument:** the window in which it works is the window in which nobody is looking yet, and a gate built on it **starts passing at the moment the defect becomes invisible** — its green is anti-correlated with the property. **Not `R-163`/`R-164`/`R-167`, nor `CLAUDE.md`'s recording law:** all four are instruments whose *scope* is narrower than it appears, wrong the same way on every run. This one is correct today and wrong later, with no event marking the transition, so re-running it is not a check on it — the instrument-shaped twin of `IC-11`. **Remedy:** ask whether the instrument has a garbage collector. Something that tidies "stale" records is, from the defect's point of view, something that destroys evidence, and it will be a well-behaved subsystem doing its documented job. | `7eead422` (`tests/config_propagation.rs`); `issue-clusters:IC-4` surface 2; four throwaway-repo probes, ~6 min; kin `IC-11`, contrast `R-163`/`R-164`/`R-167` |
| R-167 | 2026-09-02 | hit (4 instances) | **An UNANCHORED pattern over-matches, and the surplus is PLAUSIBLE — `R-3`'s opposite direction.** Four in one session, one reader, each a `grep` written from intent rather than anchored to the target's grammar: `git grep -l 'cluster/<slug>'` counted files that merely NAME the class in prose (the surplus was the file **retagged out** of it, whose prose records the class it left — nearly reported to a peer as *their* drift); `^  [a-z_]+ ` dropped `state-at` and published 8 CLI verbs where `CLAUDE.md` says 9, in the very check that file warns you will get wrong; `fn selector_key` matched a TEST NAME by prefix and produced *"exactly five override sites"*, told to a peer as verified, against a real four; `n[=≥]` over a whole file would have gated a promotion **threshold** as a count. **Not `R-3`:** that one disciplines the zero and its tell is an absence you can notice — here the pattern over-matches, the surplus looks right, and nobody re-reads a number that looks right. **Remedy:** anchor to the grammar (`^[[:space:]]*-[[:space:]]*<tok>[[:space:]]*$` for a YAML item; a word boundary for a Rust item) and, where no anchor is obvious, construct one example of what else your pattern admits. The corpus already prescribed the anchored form in a gate's own failing-assert text and I used the unanchored one anyway — knowing the rule is not the mechanism. | `65fe14b1`, `fbe8e200`; opposite-direction twin of `R-3`, kin to `R-160` (hit-rate camouflage vs population camouflage) |
| R-166 | 2026-09-01 | miss, self-inflicted, surfaced by a DISAGREEMENT (1 instance) | **A finding parked in a commit message has no citable home, so the next session that needs it cites something adjacent.** I judged a clean near-miss — a grep whose **context window stopped one line short** of the tag it was checking for, so a present tag read as absent — *"worth recording in the message since it did not earn an entry"*, and put it in `8b24df96`'s message. Seventeen minutes later `codescout-b7` needed exactly that case as the **type-B exemplar** in `R-162`'s addendum, found it in no entry, and cited `R-163` — which contains no zero at all. Their sweep confirmed the vacuum: `clipped` / `context window` / `one line short` occur nowhere in the ledger but their own table. **Two halves:** (a) a commit message is durable but **not addressable** — `link_scan` binds a token to a `## PREFIX-N — title` heading and nothing else, so a lesson there is re-narratable but not citable, and *"this doesn't earn an entry"* is judged by the author, the party least able to know who will need it; (b) **a citation is evidence about the citer's model of the target, never about the target, until the target is read** (`codescout-b7`'s line — `R-3`'s twin, disciplining the writer of a *reference* where `R-3` disciplines the reader of a *result*). **What caught it was not a check.** Re-homing the citation from `R-163` to `R-165` on scope grounds is what sent them to open `R-163`; had I cited from `R-163` as originally offered, their table would have **agreed** with my citation and nothing would have surfaced — two independently-wrong claims pointing at each other, indistinguishable at the point of use from two right ones. **Remedy:** if a finding is worth a paragraph in a commit message it is worth an entry (one call vs a fabricated citation — and an unwritten entry produces no error, no gap report and no dangling citation, because a citation that was never possible cannot dangle); and open an entry before resting a *particular* proposition on it. | `R-162` addendum + its same-hour correction; `8b24df96` message (where the case was stranded); `R-165` scope note (the disagreement); kin `R-3`, `R-163`, `R-161` |
| R-165 | 2026-09-01 | miss, peer-caught (1 instance) | **A deletion's stale references point INWARD from files the diff never touched, so reviewing the change cannot find them.** `30b6fc41` deleted four `selector_key` overrides; I corrected the four doc comments it falsified and reported the sweep complete. A **fifth** survived in `action_selector_key`'s own doc — naming a deleted symbol, explaining a deferral that no longer applied, and prescribing an adoption that had already happened. **The two directions:** comments that *lived in* the deleted code appear in the diff (that is how four were found); comments that *pointed at* it sit in untouched files and are **structurally absent from it**. Reading the diff harder cannot reach them — a coverage failure, not a thoroughness one. Proximity did not help either: the fifth was one function from the change. **Pairs with `R-162` as the mirror instrument:** grepping the deleted token is guaranteed-zero and useless; grepping the deleted symbol's NAME is exactly right and non-zero *because* the references survive. Same technique, opposite value, discriminated by which side of the deletion you stand on. Neither is an `R-3` instance — both queries were well-formed and correctly scoped. **Remedy:** grep the tree for the deleted name before claiming a sweep is complete, and state the sweep's scope ("every falsified comment in the diff" ≠ "in the tree") — I published the stronger reading of the weaker act, `R-161` a third time. | found by `codescout-b7`; fixed at the site in `action_selector_key`'s doc; **the table in this row is superseded by `R-162`'s addendum**, which adds the asymmetric timing (Type A decidable *before* the query, Type B only after and only by a differently-scoped instrument) and the trap that B's standard remedy — widen the window — is a **no-op** against A; mechanisable as an `audit_doc_refs` extension resolving `Type::method` in Rust doc comments — candidate `I-N`, unfiled |
| R-164 | 2026-09-01 | near-miss, self-caught (3rd of family in one session) | **A mutation's kill COUNT answers a different question than necessity, and the reassuring number establishes least.** Verifying `30b6fc41`: the canonical mutation (trait default returns `None`) killed **11** tests; the per-site mutation (one `None` override on `ReadFile` alone) gave **4826 pass, exactly 1 fail**, naming `read_file`. I had the 11 in hand and was composing it as the new gate's justification — but **10 of the 11 pre-existed**, so it establishes that the *inversion* matters and says nothing about the gate. **Why the number actively misleads:** a mutation answers "is this line guarded?", not "is MY guard load-bearing?"; the kill count measures the existing guard population at that site, which is **largest at the canonical site** because prominence is what got it covered. The count is anti-correlated with the information wanted, and a big number is what does not prompt a second look. **Distinct from `R-158`** (which case to mutate) **and from the standing per-site law** (how many sites) — this is how to read the result, and it bites even when both are obeyed. **Remedy:** never report a kill count without naming the proposition it establishes; the reportable form is "mutating a site this guard uniquely covers left M others green and reddened only this one." | `30b6fc41` patch-id `db34821395d6257eb37562bb779ed9ab4eba091e`, whose message records both mutations and states the 11-kill is not evidence for the gate; kin `R-158` |
| R-163 | 2026-09-01 | miss, reporter-retracted within the hour (1 instance) | **A peer's observation and its attributed CAUSE are two claims, and the real number makes the inferred condition read as measured.** A peer reported two flake reproductions *plus* a condition ("6 live sessions **and** a concurrent cargo holding the target lock — moved the rate from ~1-in-N to 2-of-2"). I had verified an unrelated numeric claim of theirs the same hour (byte arithmetic, `+3 −1 = +2`, exact) and took this one whole — into a durable bug record, into its **queryable `unverified:` field** as "bisect now feasible", and into a specified next probe. Retracted by the reporter from their own buffers: run 1's lock-waits were at *build start*, run 2 had **zero** and failed anyway. Their words: *"I had a coincidence and named a mechanism for it."* **Not `R-160`/`R-117`:** the figure was REAL — two runs did fail — and only the *condition* was inferred, so the observation lent its credibility to the attribution riding along with it. **What defeated an engaged reflex:** I hedged the **causal** reading ("candidate, not a finding") while recording the **observational** claim as fact — care applied one level from where it was needed. Unused tell: "2-of-2" is a denominator over a population the reporter selected. **Cost asymmetry:** it reached a field a triage query returns, not a sentence a reader re-reads. **Remedy:** one question — *"how did you establish that?"* | `ee9d8d80ad5ecdc8` § *RETRACTED, same day, by the reporter*, which preserves the superseded text and the corrected `unverified:`; kin `R-160`, `R-117`, `R-161`, `R-162` |
| R-162 | 2026-09-01 | miss, peer-caught (1 instance) | **A DELETION cannot introduce the token you grep for, so the zero is guaranteed before you run the query.** Screening a subagent's collision warning on `src/librarian/adapter.rs`, I grepped the staged file for `action_selector_key`, got **0**, and overruled the subagent. Measured on `30b6fc41`: that token occurs **0 times both before and after** the peer's change — the query returned the same answer in both worlds, zero bits. The discriminating query was one line away (`fn selector_key` goes **2 → 1**); `git diff --cached`, which the peer ran, named both authors immediately. **Not R-3 again:** R-3's three ways a zero lies — scope, shape, encoding — are properties of how the *query* was written, and mine was well-formed, correctly scoped, right file. This is a property of the **hypothesis**: the proposition has a direction and a token search reads only one of them, so it is blind to half the space, silently. Widening corpus, glob or pattern fixes none of it. It is CLAUDE.md § Testing Discipline's recording-filter law arriving at a peer-coordination check instead of a test. **The escalation is the expensive part:** the instrument did not merely fail to inform, it **outranked a correct signal** — a subagent had already found the collision by the right method. The asymmetry that made it feel safe: a positive hit *would* have been evidence, so a one-outcome query felt like a two-outcome test. **Rule:** to detect a peer's change in a file, diff the file; you cannot know which direction their change ran. **Addendum:** two ways to hold a well-formed zero — the query that *could not* have returned non-zero (hypothesis has a direction) vs. one that *could* but whose window excluded the answer (`R-163`). Type A is decidable **before** running it and free to prevent; type B only after, and only by a second instrument of different scope. **The trap: "widen the sample", the reflex remedy, is correct for B and a no-op for A at any corpus size.** No instrument reports which one you hold. **Correction (same day):** the addendum's table first named `R-163` as the type-B entry; it is not — `R-163` inherited a *real* figure with an inferred cause, no zero and no window, so neither half of the discriminator reaches it. The type-B case is a peer-reported near-miss written up nowhere. The miscitation was the addendum committing `R-163`'s own law while generalising it: **a citation is evidence about the citer's model of the target, not about the target, until the target is read.** | this session; `30b6fc41` patch-id `db34821395d6257eb37562bb779ed9ab4eba091e`; sub-shape named by `compact-root-claude-md`; parent `R-3` |
| R-161 | 2026-09-01 | hit ×2, neither by method (2 instances) | **The weaker act wears the stronger act's appearance, so completing it feels like completing the stronger one.** Two unrelated-looking cases, one shape; care is fully engaged on the weaker act, which is why the substitution is invisible. **(a) Checking a warning is not clearing a change.** `codescout-b7` verified all three facts of a peer warning, found a fourth that made them inapplicable, and moved to dispatch — the warning was discharged, the change was never reviewed. An unrelated scout stopped them; their words: *"luck of sequencing, not method."* The change carried **two** defects neither party had named. **(b) A present fixture reads as coverage.** Gate 7's `output_id` probe exercises real branches in `symbols`/`references`/`call_graph` and is **inert** for the only declaring topic, whose router never reads that key — not wrong, and indistinguishable from load-bearing from outside. **Remedy for (b) is the INVERSE of an existing law:** CLAUDE.md says annotate a fixture's *load-bearing* detail so a tidy-up cannot silently remove it; this says annotate a fixture as *inert* so nobody credits it with coverage it lacks. One guards silent removal, the other silent credit, and the second is worse — false coverage stops the next person looking. **Proposals:** (a) Phase 2 — discharging a warning is not an outcome, name what remains unchecked; (b) CLAUDE.md § Testing Discipline, operator's call, not filed by this entry — and state the ASYMMETRY rather than the pairing: false coverage stops the next person looking, silent removal does not. **Addendum:** 4th instance (declining to message a running implementer — "one more thing" wears the appearance of fixing the task), and remedy (b) has a gap: a property jointly covered by N files has N candidate annotation sites and no owner, so annotate-at-the-fixture-line has nothing to fire at. | worked example shipped at `b4fa1be6`; `response-envelope-session-log:F-1` (b7's withdrawal); kin `R-159` — mechanism-vs-reachability is the same substitution one layer down |
| R-160 | 2026-09-01 | miss, peer-caught (1 instance) | **Partial success is the camouflage: one pattern over a heterogeneous population misses its odd member.** Probing five keys of a session-registry file with one grep built as `"$k":"[^"]*"`, I reported four present and `pid` absent — then offered "pid is only the filename" to a peer as a refinement. Wrong: the bytes are `"pid":3624594`. Four keys are JSON **strings** and `pid` is a **number**, so the type-uniform pattern excluded exactly the one key typed differently. What made it persuasive is that the instrument was **visibly working** — a uniform 0/5 would have sent me to the pattern, where 4/5 made the blank read as a property of the data. The peer's own diagnosis (minified, no space after colon) was also wrong; `grep -c '": '` returns 0. Fourth false-negative filter of mine in one session, three in `cargo test`: a bare name with `--exact` matched 0 of 4820 and reported `ok`; `-- guide` matched 132 tests but not the gate under test; `-- valid_slugs` named no test at all. Each returned a well-formed plausible result, never an error. **Proposal:** `R-3` bullet — check the pattern against the population's ODD member, and treat a partial hit as a stronger warning than a total miss. | this session, verified in the bytes both ways; sibling of `R-158` — there the *control* was drawn from the most prominent member, here the *pattern* was fitted to the most typical one |
| R-159 | 2026-09-01 | hit (1 instance) | **Verifying a MECHANISM is not verifying its REACHABILITY, and a clean mechanism check feels like the strong form of confirmation.** Refused to relay a peer's claim unverified (`R-154`) and checked all three load-bearing facts in the bytes: `Shape::matches` opening `let Some(sel) = sel else { return false }` (`guide_index.rs:179`), the trait default returning `None` (`types.rs:1439`), and exactly five `selector_key` override sites of six files. All three held; I relayed it as time-critical. **Correct, and moot** — the recipient found a fourth fact neither of us sought: `progressive-disclosure.md` carries **zero** `serves:` markers (`librarian.md` is the only declaring guide), so `guide_blocks_for` takes the `!GUIDE_INDEX.declares(topic)` branch at `types.rs:1019` and `Shape::matches` is **never consulted** for that topic. Every check asked how the code behaves *when entered*; none asked *is it entered*. This is § *Testing Discipline*'s "loudness is a property of a PATH" aimed at a claim rather than a guard — `BL-66` is an `abort!` nothing reaches, this is a verified blocker nothing reaches — and the remedy transposes: name the live configuration in which the mechanism fires. One `grep -l 'serves:' src/prompts/guides/*.md` answers it. **Not an argument against verifying before relaying:** reachability is a FOURTH question, and verifying the mechanism harder never reaches it. | mirror of `R-3` — that says prove your instrument can *find*, this says prove your finding can *fire*; sibling `R-154` (relay discipline, which worked) |
| R-158 | 2026-09-01 | miss twice, self-caught on the retry (2 instances) | **The canonical case is the worst positive control — prominence is why it is already covered.** `R-3` says run a positive control and is silent on *which* case. The case that comes to mind first is the highest-traffic, best-documented one, which is exactly the one most likely already protected: prominence causes both memorability and coverage, so the correlation is systematic rather than luck. Mutating `artifact.create` to verify a new declared-shape gate reddened **two pre-existing tests**, because it is one of six shapes a hard-coded high-volume test pins — "proving" the gate fires while proving nothing about its necessity. Re-mutating on `artifact.link` (one of 18 of 24 unpinned shapes) gave the real figure: 132 guide tests pass, the new gate alone fails. Same session, same mistake: annotating `symbol-navigation` first to verify Gate 7, the one topic an existing byte-identity test happens to pin via its `symbols` fixture. **The two failed in OPPOSITE directions** — one made the gate look more necessary than it is, the other made the tree look less defended, retracting a published "passes all six gates". So the bias is toward whatever the canonical case happens to be, and cannot be corrected by leaning either way. **Proposal:** choose controls from the population's UNREMARKABLE members; pairs with CLAUDE.md's "mutate once per guarded SITE", which says how many and is silent on which. | this session; gates `1b02f36b` and `b769277b`; the retraction is recorded in `system-retrospective-improvements:T-15a` |
| R-157 | 2026-09-01 | hit (1 instance) | **A work-split is TWO claims, and the "easy half" label is the one that tells you not to check.** `R-95` established that a **deferral** rationale is inflated in the direction that justifies stopping. This is the mirror, with the opposite sign: when work is split so the hard half can be deferred, the easy half's cost is **deflated** to justify the split. And the easy half is the more dangerous one — a deferral says *"do not do this"*, so nobody acts on it, whereas *"ready now, build-safe, no judgement required"* says *"do this without thinking"*, which actively suppresses the scout that would refute it. `T-15a` carried a three-clause label and all three were false. *"Pure annotation"* — Gate 5 makes the **topic** the unit, so no topic holding an over-cap section can be partially annotated, and the recorded per-topic counts describe a partition the gate forbids. *"Build-safe"* — inverted: `Shape::matches` treats `selector_key: None` as no-match by design, and every tool routing to the three target topics returns `None`, so annotating REMOVES whole-guide delivery in favour of the preamble. *"Ready now"* — four of nine topics are pull-only, where `GetGuide` serves `topic_body()` and never consults the index. The easy half was **empty**: the only annotatable topic left is the five-over-cap one deferred as the hard half. **Proposal:** widen the `R-95`/`R-92` Phase 1 bullet to cover the split, naming `no judgement required`, `build-safe` and `mechanical` as triggers. | `system-retrospective-improvements:T-15a` (superseded text kept inline); Gate 7 at `b769277b`; kin `R-95` — same law, opposite sign |
| R-156 | 2026-09-01 | hit (1 instance) | **A guard's suggested REMEDY is a claim about your situation, and it was written without it.** The `foreign-index` pre-commit hook correctly refused a bare commit whose index held a peer's staged path, then prescribed *"commit your own paths by pathspec"* — which would have committed the ledger carrying that peer's `IC-22` `n=2` **without** the bug file satisfying it, producing a `HEAD` where `every_index_count_matches_the_corpus` fails against a corpus of 1. The guard is sound on the axis it owns and has no way to see that a count cell and a bug file are **one change**; correct against capture, red `HEAD` under coupling. This is `R-49`'s *proposed fix is a claim about current state* arriving from a **mechanism** rather than a person, and harder to doubt for that reason — a hook's text reads as policy, not as a proposal, and the refusal's authority carries over to the advice attached to it. **Remedy:** check refusal and prescription separately — name what the commit must contain for every gate to hold at `HEAD`, *then* pick a commit form. Here the pathspec form was right and needed all three paths, which the guard's example neither had nor could have. | `455184eb` (the commit; verified worktree/`HEAD`/declared agree at 9, 6, 2); inverse of `R-155` — that is a gate reading the wrong tree, this is advice moving you to a tree the gate never read |
| R-155 | 2026-09-01 | hit (1 instance) | **A gate that reads the WORKING TREE certifies a state you may not be shipping.** Law *B*'s axis, applied to git state rather than to build features: `every_index_count_matches_the_corpus` runs `git grep` over the working tree, was green, and said nothing about what got committed — six of seven retagged files went unstaged, so three newly-opened classes published counts of 3/1/2 against **zero members at `HEAD`**. No amount of re-running helps, because the shipped state is not one the check can address: a partial commit ships a state that never existed on disk. Silent in the author's direction (local green, CI red, surfacing to whoever pushes next) and actively reassuring, since running the gate before committing answers a different question than the commit poses. **Runnable, one call:** `git grep -cl <pat> -- <paths> \| wc -l` against `git grep -cl <pat> HEAD -- <paths> \| wc -l`; any divergence after a commit you believed complete is a partial commit. Only run because a peer's report had made HEAD-vs-worktree the habit an hour earlier. **Proposal:** Law *B* sub-shape — name the tree your instrument read, and check it is the tree you are shipping. | `cluster-promotion-session-log:F-7` (the working, repaired at `4f598b5b`); sibling of `R-89` — that one is *which build*, this is *which tree*, the same question of a different substrate |
| R-154 | 2026-09-01 | hit (1 instance) | **A NUMBER inherited from a subagent report is a claim, and a floor carries no denominator to check it against.** Law *A* already names *"a subagent's report"* as a claim; what let this one through is that the claim was a **quantity**, and quantities read as measurements someone else already took. Two subagents auditing a different question each noted `IC-13`'s *"without a marker"* clause looked false for “at least four” members; that floor crossed three of my messages, a commit message and a tracker entry, and was about to ground a taxonomy ruling. Measuring it (16 files, one reader each, quote per verdict, expected answer withheld) gave **A=4 / B=5 / C=7** — the claim holds for 4, and the floor had been carried in the **opposite direction** to its meaning: read as bounding the members the claim *fails* for, it bounded the ones it *holds* for (true failure figure 12 of 16). Two properties made it durable: a floor has **no denominator**, so ≥4 is compatible with 4/16 and 4/4 — opposite rulings — and re-reading a bound tells you nothing new; and the **framing selected its meaning**, since “the claim is too narrow” predicts B, so the digits read as “4 B’s” when B is the *smallest* bucket. **Proposal:** Law *A* sub-shape — a number from another task is a claim; re-derive it, and demand its denominator before it bounds anything. | `cluster-promotion-session-log:F-5` (the 16 per-file verdicts, each quoting the file that settles it); kin `cluster-promotion-session-log:F-4` — a count cell stale by *concurrency* rather than provenance, so numbers here rot two ways with different remedies (re-derive vs gate) |
| R-153 | 2026-09-01 | hit on the finding, **miss on the method** → proposal (1 instance) | **Every Phase 1 instrument the skill prescribes is READ-ONLY; mutation is the one that writes, and it collided with a peer.** Reading a test says what it asserts; only mutating the code says what it catches — the right instrument for law *D*, which this ledger already carries at 7%. Four runs proved a new dispersion constant unguarded across (0.667, 1.0], a result no reading produces. They also put a **disabled gate into a peer's staged index**, where it survived their review of the staged diff and was stopped only by an unrelated `unreviewed-content` hook. Care helped neither party: a mutation writes the same bytes an edit writes, so the peer's provenance probe returning `SHARED` was the *ceiling* on what any observer could learn. The skill already has a side-effect caveat for a read-only step (the `ytt`/`helm` transcript-disclosure bullet), so the category exists and the write case is missing from it. **Proposal:** Phase 1 bullet — mutation belongs in a worktree or against a committed base; `recon-active` says a scout is running, not which files are transiently untrustworthy. **Promoted-set audit recorded in the entry** (4 laws checked: 0 false, 0 obsolete, 1 outgrown-and-widened-here). | `bug-fix-session-log:F-94` (the finding), `bug-fix-session-log:F-95` (the collision); kin `R-150` — same shared-tree family, that one a destructive read-side step, this one a destructive write-side one |
| R-152 | 2026-09-01 | hit for `R-49` — this entry records the AUDIT, not the hit | **A recurrence of an already-promoted law is a defect in the promoted text, not a new entry**, so the question is which staleness category the promoted law falls into today rather than whether it deserves a fresh number. `R-49` checked against all four categories: healthy, no re-promotion owed. The gap it surfaces is **attribution, not text** — a catch claimable by two mechanisms leaves neither credited. | `0ed6cb18`; SKILL.md § *Every promotion audits the promoted set*; *(index row backfilled 2026-09-02 from the entry body)* |
| R-151 | 2026-09-01 | hit — and the unusual kind, because **no downstream gate existed to confirm it** | **A design's quantitative premise is a hypothesis about a substrate nobody sampled.** Scoping T-7 began by histogramming the live `catalog_audit` table rather than by reading the approved spec's Phase 2 reasoning. The spec's one *named* volume term (reindex churn) measured **0.4%** of rows; the **unnamed** term was **98.5%** — empty `{}` diffs from an UPDATE trigger with no `WHEN` clause. Nothing downstream would ever have contradicted the premise. | `0ed6cb18`; `catalog_audit` histogram; *(index row backfilled 2026-09-02 from the entry body)* |
| R-150 | 2026-08-31 | miss → rule (1 instance, self-caught) | **An ownership assumption must not authorise a deletion — it is an attribution claim, and here the refuting datum was inside the path being deleted.** A post-rebuild scout verified all three `R-89` axes positively, then found 9 of 13 live servers on pre-rebuild bytes. Cleaning up after itself, it read an untracked `docs/issues/.buddy/` as its own marker debris and `rm -rf`'d it. The path carried a **session id — visible, unread — belonging to a different, live peer**; the scout's own marker had landed correctly elsewhere. Worse than the misattribution: the scout had just raised the question the deleted files' mtimes would have answered (was the peer's judge narrative being *split* across two dirs, or was this an abandoned duplicate?), so the deletion removed its own adjudicating evidence — **permanently unanswerable**. Checking *after* is what rescued the report: the peer's canonical `.buddy/<sid>/` was intact and still being written, so the cost fell from "destroyed a live peer's judge state" to "removed a stray duplicate" — two orders of magnitude, one call apart. **Tell:** the word *my* in front of a path on a shared tree. **Runnable:** `ls -l` before `rm`; on a tree shared with peers the owner is in the path and costs one call. | this session; sibling of the Phase 1 rule that a negative search result must not authorise a deletion — this is the ownership form, which that wording does not reach; kin `R-123` (adjacency-as-cause) and `scripts/peer-sessions.sh`'s own warning not to infer authorship from presence, inverted here into inferring it from absence. Also surfaced `.gitignore:43`'s root-anchored `/.buddy/*`, so a nested `.buddy/` is untracked rather than ignored; fix is upstream in `claude-plugins`, filed |
| R-147 | 2026-08-31 | miss → rule | **A quotation that asserts its own fidelity does not check it — and the assertion is what stops the reader looking.** A manual block reproduced a plugin's SessionStart injection under a ⚠️ note saying it was quoted **verbatim** from `session-start.mjs:339`, that its last line was known-wrong, and that it was reproduced unchanged *"because a manual that quotes a hook has to match the hook"* — correct reasoning, exact `file:line`, and the known-wrong line did match. **Two of the other three lines had drifted anyway** (hook emits `POST-COMPACT: Context was just compacted.` / `workspace(post_compact=true)`; block showed `codescout PostCompact: …` / the `action: status` form), and the section's closing prose repeated the second error. The label INVERTS the check: a reader told "quoted verbatim" has been told the check was done, so the assurance substitutes for it — an unlabelled quote invites *is this current?*, a labelled one answers pre-emptively and wrongly. It also survived a deliberate correction pass the day before, which edited this very block to add the warning and did not re-derive the rest: **a targeted edit is exactly what will not look at the parts you are not thinking about.** Caught by comparing against what a live session actually received, not by re-reading. **Runnable:** treat a quotation of a live emitter as a DERIVED artifact — re-derive the whole block whenever you touch any line of it, or drop the fidelity claim and say *paraphrased*. | this session; instance in `d7072ed21959aca1` § Fix, fixed at `2c730ebd`; documentation-side twin of R-89; kin R-142, R-144 |
| R-146 | 2026-08-31 | miss → rule (5 instances, one day), Promote-when FIRED | **A measurement of state someone else is editing expires before the message carrying it arrives — and the cost FLIPS SIGN rather than decaying.** A peer reported the lean lane red naming two `tools::tree` tests, with sound attribution (neither name exists at `git show HEAD`, so new-and-red not a regression) — but *new-and-red is RED's own signature*, so the very evidence proving it was not a regression is what should have marked it transient. Re-measured here: **7 passed, 0 failed**, both named tests green; `grep cfg(feature` → 0 matches, so not lane-specific; the file read **+204 → +214 → +360** across three readings, 146 lines added between their run and mine. Acting on the stale warning costs the INVERSE of what it prevents — not a session blaming itself for another's red, but a session distrusting a green that is real. Two sibling instances the same evening: an unbacked "the tree dotfile bug is mine" that displaced the bug's own filer, retracted as *"a claim with no work behind it … asserts a state that has an expiry and does not carry one."* **Runnable:** ship the derivation (command + instant + cheap re-check), not the value; re-run on receipt whenever the artifact is under active edit. | this session, verified by re-measurement; kin R-98, R-142, R-143 |
| R-145 | 2026-08-31 | miss → rule | **Co-occurrence in a working-tree snapshot is not evidence of one change.** `git status` showed `src/tools/tree.rs` (+204) and `src/util/fs.rs` (+58); I broadcast them to four peer sessions as one change and they were **two authors**, the `fs.rs` delta an unrelated `atomic_write` tmp-file leak fix. Verified after: the fs.rs diff has **0** occurrences of `hidden_at_root` and **5** of `atomic_write`, and the tree.rs diff adds **no** `util::fs` import — no dependency edge in either direction. The instrument caused it: `git diff -U0 -- <a> <b>` into one buffer carries no author column and no separator a skim registers, and the disconfirming evidence was already sitting in that buffer's tail. Complement of R-50 rather than an instance — that is a view which silently DROPPED, this is one that silently MERGED, and in a shared checkout `git status` is a union over N concurrent authors. **Runnable:** `--stat` per path, and look for a dependency edge before calling two paths one change. | this session, corrected by an author's reply after 4 messages had gone out; kin R-50, R-142, R-4 |
| R-144 | 2026-08-31 | miss → rule (3 instances, one stream) | **A tripwire aimed at a FABRICATED fixture cannot detect the change it was written to detect.** A test meant to notice a future change must assert on a value the system PRODUCES; given a literal it holds equally in the world where the change landed and the world where it did not, so it is silent in both. Three instances, escalating: (1) `RoutedEchoTool { name: "memory" }` supplied the `selector_key` production `Memory` lacked, so the whole operator-rules suite was green while every triggered rule was dead — fixed `2447f709`; (2) a regression test asserted `by_check.get(…).is_none()` to mean *found nothing here*, encoding the exact ambiguity it guarded, and passing under BOTH conflated world-states — fixed `09cd1b46`; (3) worst, `op_4s_path_predicate_cannot_fire_against_a_write_response_today` **advertised itself as a detector** — *"when this test starts failing, that is the fix landing"* — and did not fail when the fix landed at `a6b4fc35`, because its fixture was a hand-written response bound to a variable named `observed`. It bought a tripwire's confidence and delivered none; a fabricated fixture is what a reader skims past, because it looks like setup. **Tell:** a literal beside an assertion about production behaviour — ask *did anything under test produce this value?* **Runnable:** a test naming a future condition must obtain its fixture from the pipeline it watches, or say in its doc that it cannot. | this session, all three fixed; sibling of `observer-blindness:OB-5` (which covers the vocabulary half — this covers whether the fixture was produced at all); kin R-139 |
| R-141 | 2026-08-31 | miss → rule (2 instances sourced, sibling of R-140) | **The referent you did not open is the part that gets EXECUTED — and a lineage anchor names lineage, not authorship.** One session relayed a `git worktree add` they had not run; another cited `R-3` for a clause it does not contain. Both had checked the surrounding prose. A shell command reads as *mechanical* and an id as a *pointer*, so **neither reads as an assertion needing support** — and an id additionally reads as *precise*, which reads as *verified*, making a wrong id more persuasive than a vague gesture at the same wrong idea. The composite case is worse and is a designed trade: the SKILL.md bullet carrying the positive-control instruction ends `(R-3 → R-113 → R-77 …)`, and **`R-3`'s own Promoted-to field says the back-citation was chosen OVER a verbatim quote so it survives rewrites** — i.e. it survives by not tracking which clause came from where. So *"R-3 says X"* can be false while *"the bullet R-3 anchors says X"* is true, with nothing in the citation's surface distinguishing them. Remedy is unconditional, not vigilant: **all three sessions were actively writing about unverified relaying at the time.** |
| R-140 | 2026-08-31 | miss → rule (2 instances, one direction) | **A warning that prescribes an action must state its recovery cost, DERIVED — understating it licenses the thing warned against.** An overstated cost fails safe; an understated one converts a warning into permission, so the error and the harm point the SAME direction. Instance 1, verified here: the `.worktrees/bench` warning stated recovery as one `git worktree add --detach`, which fails — the path holds 24 entries and bench is not a registered worktree, so real recovery is move-then-add plus a **163M re-index**. Instance 2, reported first-hand: the relaying session verified the warning's DESCRIPTIVE claims and never ran the command, so their operator got the understated cost with a verification wrapped round it — confirmation of one half reading as confirmation of the whole. The tempting symmetric second direction (*omit* the cost and it gets discounted) is recorded as an **untested PREDICTION with its own promote-when**, because asking its supposed source first-hand revealed no incident behind it — an argument in the past habitual, read as a report. Writing it as a pairing would have manufactured symmetry inside an entry about warnings that mislead. |
| R-139 | 2026-08-30 | miss → rule (parent of R-138) | **The parameter your own context supplies for free is the one you will omit — and it is invisible to you SPECIFICALLY.** Four instances in one evening across three sessions, every one found by review and none by self-review: a count published without its counting rule (4 off a trimmed view / 8 verbs / **9** lines with clap's auto-`help`); a query published without its key (fenced blocks containing `cargo test` — missed a parenthetical fifth site); a second query without its key (`cargo clippy` — unreachable for the wrapper form `scripts/build-windows.sh clippy …`); and an instruction without naming which part is the evidence (*"verify positively with `artifact --help`"* — the author's own probe discarded output and tested only the exit code, so nothing in their practice could surface the ambiguity). Result is never an error, always a **confident wrong answer**, so nothing downstream catches it. **Why peer review works where care does not:** the missing parameter is invisible precisely to whoever holds it, making a second reader a different INSTRUMENT rather than a redundancy — and it predicts which reader, one who does not share the author's context, not a more careful one. Test: *what did I know that the reader will not, that made this unambiguous to me?* Publish a number's **derivation** ("8-or-nothing because the `#[cfg]` sits on the whole variant") not the number; report a count **with the key searched for**; name which half of an instruction is the evidence. **Do not budget for vigilance** — all four were committed by authors actively writing about this class, one reintroducing a bare integer *in the commit fixing a bare integer*; what caught the wrapper was an unconditional policy (verify before contradicting a peer). Corollary: consolidating N copies removes the redundancy that could have caught an error in them — keep the derivation, not the copies | this session + `codescout-ae` + `git-travel-augmentation-shape`; `334cf64b`, `2f412a1c`, `bdfd7a62`, `610e132a`; kin R-138, law B |
| R-138 | 2026-08-30 | miss → rule (child of R-139) | **A sweep that searches by SHAPE returns a confident wrong COUNT — and two agreeing sweeps are one sweep.** Two sessions independently enumerated every site transcribing the four-command gate, to deduplicate them; both returned **four**, the real count was **five**, and the agreement raised confidence in the wrong number instead of exposing it. Both had searched for the gate's shape — a fenced block containing `cargo test` — while `docs/ROADMAP.md:93` states the same proposition inline in parentheses mid-sentence, invisible to that query and to every refinement of it. Escalates law C from a zero to a **plausible positive integer**, which is worse: a sweep that missed a site is byte-identical in output to a sweep over a corpus that has none, and unlike a zero it never invites suspicion. **Corollary, causal not coincidental:** the evading site was also the STALEST (three commands, unrevised since 2026-08-06) — a site that evades one sweep evades every later maintenance pass for the same reason, so it accrues all the drift the found sites had corrected; expect prose restatements to be the worst copies and look there FIRST. It compounded through the reference graph: that paragraph pointed at the weakest of the five as "the full gate definition", so a reader distrusting local prose and following the canonical pointer landed on the copy linting neither `codescout-embed` nor the `local-embed` module — the label discharged the scrutiny. Tells: query units differ from question units (fenced-blocks vs statements-of-X); independence of authorship is not independence of METHOD. **Third instance the same evening generalises it past prose:** `ci.yml:284` is `scripts/build-windows.sh clippy …`, a WRAPPER invocation containing no `cargo clippy` substring, so it is invisible to any audit keyed on one — the class is *any query keyed on a token the site does not contain*, not prose-vs-fenced-block. Caught by a POLICY (verify before contradicting a peer), not by attention; knowing the class prevented none of the three instances, all three committed while writing about it. Do: search the narrowest token, then broaden past the tool name (`clippy`, not `cargo clippy`); read one whole file end-to-end before trusting a corpus count; report the count AND the form searched for; budget for an unconditional check rather than for vigilance | this session + `codescout-ae` (`bdfd7a62`); `docs/trackers/gate-contract-consolidation.md`; kin R-4, R-3, law C |
| R-137 | 2026-08-30 | miss → rule | **After compaction, a peer is a better witness to your past than you are.** `codescout-ae` asserted a hand-edit was theirs; I relayed it unchecked; they later checked their commit list, found the SHA absent, and corrected the record — including the claim they had never said it. I still held the message and quoted it verbatim; their context had been compacted between the two, and they had offered exactly that bound themselves. Three claims resolving differently: the hand-edit happened (true, by diff), they authored it (false, by their commit check), they never said so (false, by my transcript). **The session was right where it re-derived and wrong where it recalled**, and only the transcript-holder could separate those. Inverts the usual default that first-person testimony outranks a bystander's — here the bystander holds the bytes and the first person holds a summary, and the loss is invisible from the inside. Rules: never relay a peer's claim about their own authorship unchecked; if you hold the transcript, quote it rather than contradict; prefer a re-derivable instrument to anyone's memory. Tell: a reconstructed memory carries a detail that does not fit — this one said "this morning" of a **16:50** SHA, in plain text both of us read | kin R-134 |
| R-136 | 2026-08-30 | technique (parent generalisation from `codescout-ae`) | **Adjacent-proposition errors come in THREE kinds, and each needs a DIFFERENT check.** Nine confident wrong answers in one day were all real measurements, faithfully reported, of a proposition next to the one asked — but the remedies diverge, and running the wrong check leaves the error standing while feeling rigorous. *Propositional* (the probe answers a different question: `du` for absence, a build's exit code for "the binary carries the subcommands") needs a **positive control**. *Temporal* (right about an instant, read as standing: `fmt` reporting a reverted mutation) needs **bracketing** — and a control does nothing, because the probe was working perfectly. *Sampling* (right for this observer: one session's `ListAgents` skew) needs a **second observer**, which neither of the others touches. Counterexample kept inside the statement, so it is never written as *every*: the mutual-deference deletion misread no measurement at all — two correct reads, two correct acts, composing badly — the one class instrument discipline cannot reach. Operational half: **a caveat should name its sub-kind**, since *"two readings is not a measurement"* tells a reader to go find a second observer rather than run a control. |
| R-142 | 2026-08-31 | miss (2 instances) | **Circular corroboration — a peer restating your claim is your own reading handed back, and nothing marks it.** Two parties agreeing is evidence only if they did not get it from each other. I read a peer's past-habitual *argument* ("a warning with no stated recovery cost tends to get discounted") as a *report* of an incident; the same peer then described their own error as "the other failure of the same pair", which read as first-hand confirmation and was my own reading returned — their actual error points the SAME direction as mine. I reported both directions to my operator as observed. Caught because `codescout-f0` declined to write the pairing from my summary and asked the other session first-hand, which is the working countermeasure. Upstream of R-140 (its manufactured symmetry is this loop's product) and kin to R-141; corollary of `observer-blindness:OB-4` — distinct authorship supplies independence no more reliably than a distinct tool name. **Runnable:** before counting a confirmation, ask *could they have got this from me?* | this session; 1st self-caught after a peer's first-hand check, 2nd disclosed by the peer who made it. 2nd instance closed through a COMMIT MESSAGE: a peer quoted my own already-retracted number into `2f434fba` as though measured, and I withdrew a correct retraction on the strength of it — genre supplied the authority, the artifact is durable and stays wrong, and the loop destroyed a true datapoint rather than adding a false one. A reader cannot audit a cited number's provenance, so it must be marked at the write: cite what you did not measure as *reported by X*. Kin R-139, R-140, R-141, R-145 |
| R-143 | 2026-08-31 | miss → rule (2 instances, one reached a shipped artifact) | **Verify the ACTIONABLE half of a report — and a value whose instrument you did not state is actionable.** Clause 1: descriptive claims are what a reader *evaluates*, the prescription is what they *execute*, so effort flows to the legible half and the reader gets an unchecked instruction inside a verified envelope. Clause 2 is what the naive form gets wrong: **a bare value is not descriptive, it is a prescription** — checking it means running something, nothing says what, so the reader invents a rule, and two honest verifiers disagree in the form of a *correction* rather than a conflict. Instance 2: a peer reported `retrieval-benchmark.md` — **8 references**; I published **17** under a column headed *Verified*; the answer is **15**, stable since `d6d66e4c`, so neither was stale and both were derived. **Seven plausible instruments on that one file give seven answers (14/15/15/41/40/8/9), and `8` is exactly the occurrence count of the pinned SHA** — so their number was likely right under its own rule and my correction wrong under mine, with no rule stated anywhere. The two claims I got right that evening (`PROBES.md:116`→**141**; the corpus was already deleted once) were the ones whose instrument admits no choice: **where it admits one, diligence yields a confident wrong answer, which travels with a verifier's name on it.** Law B turned on the verifier (the entry first mis-cited this as R-136 — instance 3, in-entry). Mechanised at one site — `.worktrees/README.md` now carries the grep, not the number. | this session; the two instances are mine, recorded from the other side in R-140 § Instance 2; slot reserved by R-140 and R-141 |
| R-135 | 2026-08-30 | miss | **A `du` proves size, never absence — a TRUE measurement written up in the past tense.** An archived bug file is `status: fixed` and closes *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state"*; 14 days later `.worktrees/bench` is still on disk at exactly 174M/163M, dir mtime **2026-05-12** (three months BEFORE the closure) and a gitdir still naming the pre-rename `code-explorer` path, which rules out delete-then-recreate since the file's own rebuild command would name `codescout`. The `du` ran before the removal and was written up as its result. Distinct from R-125 (abundant vs empty) and from the self-validating-gate class: this is a **correct positive number transferred to a proposition it does not support**. Audit verdict on the promoted set: **UNREACHABLE, not Outgrown** — Phase 3's *"name the proposition it proves, then ask whether a broken world produces the same result"* covers it exactly and would have caught it; a broken world yields the identical `du`. Remedy is placement, not wording: the risky moment is writing a **closure**, when the author is furthest from the evidence and most certain. No promotion proposed — one verified instance, and the session-opening surface additionally needs a base arm nobody has run | this session (`docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`, `worktree-cleanup-session-log:F-1`); three sibling instances reported by a peer session and **not verified by me**; kin R-125, law B |
| R-134 | 2026-08-30 | miss → rule | **A peer view is ARBITRARY with respect to the population, not merely short.** Five Claude sessions shared this checkout and produced six misattributions in an afternoon, every one an elimination over the peers the asker could see. Both open mysteries resolved in ONE round each by enumerating from the OS (`pgrep -x claude` + `readlink /proc/$p/cwd`) and messaging invisible sessions at `uds:/run/user/1000/cc-socks/<pid>.sock`. Escalates R-50: a short view makes elimination weak, so you hedge; a DISJOINT view makes it unrelated, so hedging still draws the conclusion — you must change instruments. Measured by `codescout-f0`, then CORRECTED by them the same evening (`71fdbef4`): the first reading showed 0 of the 5 in-checkout sessions, later readings 1 of 5 — the count held at 3 while the MEMBERSHIP rotated, which demonstrates arbitrariness where a single snapshot could only assert disjointness. Either reading kills elimination; the zero was a reading, not a property, and publishing it as one was this entry's own failure mode arriving through the record of itself. Tells: the instrument's units differ from the question's (sessions-a-transport-knows vs processes-with-this-cwd), and repeated "must be the remaining one" is equally produced by the answer lying outside the set. Corollary: in a live transcript a COUNT is contaminated by the act of asking (hits went 2→10 because sessions grepped for it) — prefer an ordinal | this session + `codescout-f0`, `codescout-fe`; kin R-50 |
| R-133 | 2026-08-30 | miss → rule | **Loudness is a property of a PATH, not of a failure.** Three failures in one afternoon across three subsystems: a stale sidecar restores clean reporting success; a widened status region silently discharges the disagreement the scan exists to report; and BL-66, which *aborts the process* — maximally loud — and survived anyway because nothing in-tree reaches it (verified here: `install_default_crypto_provider()` is called unconditionally at every construction site, `main.rs:253` / `agent/mod.rs:448` / `reranker.rs:80` / `embedder.rs:339`, and `transport.rs:34` states the invariant as a reason not to handle the error). So the axis is not loud-vs-silent output but whether any TRAVERSED path observes the failure. When adding a guard, name the path that reaches it and the observer who acts on it; "an external consumer we do not have" is a legitimate reason to keep it and is not coverage of our own risk. Reachability twin of R-132's monotonicity. Tell: ask what an observer would SEE differently if this were broken right now | this session + `codescout-ae` (`e6414362`, BL-66); kin R-131, R-132 |
| R-132 | 2026-08-30 | technique | **Mutate once per guarded SITE, not once per feature.** A mutation run answers a question about one LINE, not about a feature; where a law is implemented at N call sites, one kill proves exactly one site is guarded and says nothing about the other N−1. `artifact_augment` had two shape-writing paths (`merge=false`, and the sibling patch inside `merge=true`); mutating each separately killed DIFFERENT tests, neither failing under the other's mutation — a single mutation would have yielded the reasonable conclusion "the write-through is covered" with the second site unguarded. Pairs with R-130 (read which assertion died) and R-131 (its transitions-side twin) **Limit (general form):** *a test cannot detect a change its assertion is MONOTONE under* — absence assertions are monotone under removal, existence assertions under widening, so a property held by one of each is covered ZERO times, not weakly. Pairing an absence test with a positive one does NOT rescue it (my first fix, falsified by `codescout-ae` in `e6414362`: the violation mutation killed none of six tests, the positive one surviving because "a region CONTAINING the drift" is monotone under widening). Ask which direction each assertion is monotone under, and mutate the other way | this session's 4-mutation matrix; `codescout-ae`'s `entry_status_region` (both sites guarded) = datapoint 2; Promote-when FIRED |
| R-131 | 2026-08-30 | miss → rule | **Individually correct guards can compose into a door nobody can open.** Three sidecar write paths each carried a defensible, test-pinned guard — export skips an existing sidecar (idempotence), `reindex` attaches only when a row is absent (repair not sync), `artifact_augment` did not touch the file — and together left NO path that updates a sidecar after the first export. Invisible in every individual diff, because each guard is right. Fired within a day: a shape edit reported `exported: 0` while the committed YAML held the superseded enum, and a fresh clone would have restored the old shape reporting `augmentations_restored: 1` — **a stale sidecar is strictly worse than an absent one**, absence being loud and staleness restoring clean. Rule: enumerate a state's TRANSITIONS (create/restore/update) and map sites onto them; a transition with no owner is the defect. Tell: a skip justified by "idempotent", which is a property of repeated identical calls and says nothing about changed input | BL-50 item (2); peer repair `2a8decc5`; kin R-132, R-47 |
| R-130 | 2026-08-30 | technique (found by a peer, checked against my own matrices) | **A mutation kill is not evidence that the named guard fired.** One level below [[tests-that-cannot-fail]]: a test can die under your mutation for a reason unrelated to the clause its name claims, and the matrix records `KILLED` either way. `codescout-ae`'s `status_token_present_does_not_match_a_status_word_inside_a_longer_word` died on its **first** assertion (the positive separator one) and never reached the boundary assertions it is named for — a tick for a guard that never ran. Fixed by splitting, with the discriminating evidence coming from a **second** mutation (strip-punctuation normalisation, the obvious alternative implementation) that the separator clause survives: two mutations, two disjoint kills, each test provably guarding its own clause. Rules: read which ASSERTION died, not the test name; one mutation is never enough for a multi-clause test; `positive`-then-`negative` ordering is the risky shape because the positive clause absorbs the kill. Re-checked both matrices I published today — both died for their named reason, but by luck (each had one load-bearing assertion, so no earlier clause was available to absorb it) |
| R-129 | 2026-08-30 | miss (×1, reported + relayed) + near-miss (×2) — **promote-when FIRED same day** | **In a shared checkout, the deliberate break this project mandates is indistinguishable from a defect.** CLAUDE.md and `sdd-ruling-log` require *demand a deliberate break* — break the thing, watch the specific test die. Nothing in a shared tree distinguishes that from a real failure. Mutation-verifying a packaging gate, I removed a `Cargo.toml` `exclude` negation; an IL-3 refusal killed the chained restore and the window stayed open for minutes. `codescout-ae`: *"had my gate landed inside your window, I'd have seen a red packaging test and no way to know it was deliberate — I'd very likely have reported it, and on today's form at the wrong person."* Worse, no `git add` is needed for the break to reach them: `cargo test` compiles the WORKING TREE and cargo does not consult git, so their "4862 passed" silently included my untracked test and a third session's dirty files, under a "HEAD is green" claim. **Rule: announce a mutation window BEFORE opening it, not after closing it** — one message, false alarm becomes a no-op. Distinct from [[R-90]], which is about writes crossing between sessions; here nothing crosses and the harm is entirely in the other session's reading, so no git discipline reaches it. Sub-lesson: a cleanup step chained AHEAD of a blockable step is not protected by being first — the block takes the whole command. **Instance 3 fired both promote conditions within minutes of the entry being written:** `codescout-ae` measured a lean-lane failure, checked attribution, and reported it as the documented feature-gating class; I verified it was not mine and relayed it to its owner citing that class. All three of us were careful and all three were wrong — there was no defect. `swap-dense-leg` was mutation-checking, and the deletion of `\|\| err_str.contains(SPARSE_STATUS_MARKER)` landed BETWEEN the gate's `cargo test` and its lean lane; the "full passes / lean fails" shape had nothing to do with feature gating. Their framing, sharper than mine: the working-tree hazards make **numbers** unreliable, this one makes a peer confidently report a **specific defect** with correct evidence and a wrong conclusion — **the hazard where being careful and being wrong are most compatible**, and attribution discipline does not touch it. Target: CLAUDE.md beside *demand a deliberate break*. Held for an operator call, because per-session worktrees would dissolve [[R-90]] and this together |
| R-128 | 2026-08-30 | technique (validated on first use) | **Enumerate the call sites of a must-call function and look for the absentee.** Asked to audit root/crate *pairs* for a named defect class, the pair-shaped sweep came back clean — correctly, and that clean result was the confirming-negative worth doubting. Inverting the question from "which functions are duplicated?" to "which stated invariant does a site break?" found `BL-66`: root's `transport.rs` asserts `install_default_crypto_provider` runs "at every construction site", five call sites exist, and one crate client-builder is absent from the list. The pairwise diff **structurally** could not find it — the defective function has no twin, and an absentee is defined by there being one place it should be and isn't. Blind spot: only finds violations of invariants someone has *stated*. |
| R-127 | 2026-08-30 | miss (self-caught in-turn) → confirms [[R-125]] | **I broke R-125's clause verifying R-125's own release, minutes after promoting it.** Checking whether `codescout-companion` 1.19.8 had propagated to the plugin caches, I ran `find … -path "*reconnaissance/SKILL.md" | head -1`, grepped the clause out of the one path it returned, got `0` in all three profiles, and reported *"that's the drift, caught live"* — citing `claude-plugins`' own stale-cache tracker entry as corroboration. **False.** Every profile holds TWO version-keyed cache dirs, `1.19.7/` and `1.19.8/`; `head -1` returned the stale one each time, and all three `1.19.8/` copies contain the clause. `head -1` answers *"the first path the walk yielded"*, never *"the path that loads"* — and a version-keyed cache holding two versions is the normal state, not an edge case. Two separable aggravators: **the zero AGREED with me** (law C's usual framing assumes a surprising zero prompts a re-check; agreement suppresses it, so a confirming negative needs the scrutiny a contradicting one gets for free), and **authorship is not activation** — having written, argued and shipped the clause minutes earlier did not make it fire. Caught only by re-reading my own command inside the sentence I was about to publish, which is exactly where R-125 placed it (Phase 3, the act of writing). That makes the placement argument tested rather than reasoned | severity low — self-caught in one turn, retracted in the same message, nothing built on the false reading; violates `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` Decision clause 1 verbatim; no promotion owed, the clause it confirms is already shipped |
| R-126 | 2026-08-30 | miss (found by dry run) | **An agreement assertion cannot see a shared convention being wrong.** `path_for` and `rel_path_for` were tested against EACH OTHER and agreed — both stem-keyed, both unsound, since a stem is not unique and `docs/research/README.md` is really augmented here. Gate green 4833/0 on a defect the test was positioned to catch. Tell, available at write time: ask what the assertion RANGES OVER — one input and two implementations, or the input space. Write the property (injective? round-trips? unique across the corpus?) and keep the agreement as a second assertion. Distinct from law C: no zero, no error, just two functions quietly agreeing on one file (`f565504a`, `bug-fix-session-log:W-76`) |
| R-125 | 2026-08-29 | miss (self-correcting) → promote-ready | **Law C is stated only for the EMPTY result — and the remedy I first proposed for that gap was law C again.** A keyword count in a peer's diff (19 × "timeout" across +137 lines) was read as causation and the peer was pointed at their own uncommitted work; the real defect was a `tokio::try_join!` race inside the test itself, racy since `9f4debc3`, fixed in `21174425`. The first draft of this entry prescribed *"run the failing test in isolation"* as the cheap decisive check — but it **passes** in isolation, so that green confirms the flake reading wrongly, which is law C's original form committed while writing the entry about law C. Before calling any check decisive, ask whether it can EXPRESS the failure | `bug-fix-session-log:F-78`; mechanism re-verified here at `embedder.rs:606-617` (`try_join!`) and `:540`/`:806` (`dense_only` never consulted by `embed_one_batch`); cost was one `symbols()` read rather than a bisect, because the report carried the failing assertion text verbatim; **second datapoint, volunteered by the fix's own author**: it verified the fix with 10/10 isolated runs — the same instrument, same blind spot, that had already acquitted the bug twice; the structural fact (`dense_batch`, one leg, no `try_join!`, verified at `:992-996`) is what carries it. Three passes of one instrument in one afternoon, each green read as proof of a different proposition. Threshold **fired**. Disposition **corrected from case 2 to case 3**: both datapoints occurred with the skill LOADED and law C's text already covers both errors, so this is not a wording gap and a sixth mechanism on the file's longest bullet is the accretion the audit section forbids — loaded is not reached, and the remedy is placement (a trigger-shaped clause attached to the act of citing a green) not rewording. Promotion owed as a SKILL.md PR, not yet raised; kin [[R-3]]/[[R-113]]/[[R-77]]/[[R-79]]/[[R-104]] — all five of law C's recurrences are the empty form |
| R-124 | 2026-08-27 | miss (paired hit) | **One law, hit where the skill was invoked and never fetched where it was not.** The prohibition form of *"a fix — and equally a prohibition — is a claim about CURRENT STATE"* went unchecked: CLAUDE.md's "all native `Bash` are hard-denied" was acted on unverified and used to tell the user the mandated gate was unrunnable. The same law's request form had fired correctly hours earlier, stopping a deliberately-deleted `shell_enabled` switch from being re-added. Reading the hook SOURCE would have confirmed the wrong answer — `pre-tool-guard.mjs:177` is `enforce("This call is blocked...")` — and one positive control refuted it | `shell-gating-session-log:F-1` (miss) + `shell-gating-session-log:W-1` (hit); `cat src/main.rs` matched the guard's own block branch at `:165` and executed anyway, PostToolUse hint only; `BREAKER_THRESHOLD = 3` stands the guard down by design, so "hard" is wrong even as intent; audit verdict mode 3 **Unreachable** (placement, not wording) — no law added, base arm owed; kin [[R-19]] (same law, assert-a-checkable-fact form, already quoted in SKILL.md § When NOT to Use), [[R-89]] (recurred for the same placement reason) |
| R-123 | 2026-08-27 | miss (x2) | **Adjacency is not causation — the nearest recent commit is a suspect, not a cause.** Twice in one session an observation was attributed to the most recent commit touching that surface, and both times the real cause predated it: `Monitor` executing in a deny-list arm was a list that lacked it (fixed 16 min earlier in `e67d419`), and `artifact(get)`'s `headings_truncated` was six weeks old (`3bccb234`), not the peer commit 12 min prior. Adjacency is what makes the check feel unnecessary — same file, same day, plausible subject line | cost: two bug reports, one filed `high` against the wrong layer, then retracted; sibling of [[R-118]] (a check that was refused) with the opposite origin and the same output |
| R-122 | 2026-08-27 | miss | **Writing the lesson down was the act that broke the instrument.** `R-121` prescribed *"record how it was verified unique rather than the token itself"* and named its probe token inline in the same sentence; the token now returns a hit from the tracker, so the recorded repro would tell the next session the fix regressed — or worse, that the bug never existed. Structural, not careless: a probe for an EXCLUSION mechanism must live only in the excluded region, and any ledger recording it sits in the searched region by construction, so the write *is* the contamination | verified live on the rebuilt binary at `ee7d9a3a`; `R-121` repaired in the same pass to carry the two-command derivation; kin [[R-118]] (a refused verification returns an error, not a fact) |
| R-121 | 2026-08-27 | hit | **A fix justified as "symmetric with the existing X" inherits X's scope without the property that made that scope honest.** `hidden_at_root`'s root-only `read_dir` is honest because its remedy (`include_hidden=true`) is root-agnostic. A gitignore clause copied to that shape names the six gitignored entries at this repo's root and misses `.superpowers/sdd/.gitignore:1:*` at depth 2 — the one rule that caused the reported zero. `completeness_warning`'s own doc already condemns it: *"naming an unchecked cause ends the search for the real one"* | `0f7105b8bebc600b` § Fix, corrected in the same pass; the repro also needed a fresh probe, the bug file's text having come to match its own old one; kin [[R-117]] (a proposed fix that would pass green and move nothing), [[R-118]] (ground truth before the claim) |
| R-120 | 2026-08-27 | hit | **A decision procedure is a seam — reading it far enough to get an ANSWER is not far enough to get its PRECONDITIONS.** A promotion route was picked straight from the routing test, skipping two paragraphs gating the adjacent destination and an entire successor section imposing a promoted-set audit; the target ledger's own required-field contract was missed the same way. Both omissions sit *after* the answer, and neither raises: a malformed entry is invisible to field-presence sweeps, an unaudited PR merges as cleanly as an audited one | `prompt-surface-measurement-session-log:F-43`; kin [[R-49]] (own plan, one turn old), [[R-95]] (a verdict reads as settled, not as a claim), [[R-89]] (19 cached plugin versions checked — the served copy is what has reach) |
| R-108 | 2026-08-21 | miss — caught on the first live call after shipping, not by the measurement that authorised the deferral | **A deferral justified by an AGGREGATE is silent about the tail — and the tail is the population the deferred mechanism governs.** Observed in codescout Layer 4 (`librarian(action="context")`, entry-grain). An aggregate that licenses *"not worth building yet"* averages over exactly the cases the deferred mechanism exists to serve. Kin `R-95` — *a deferral rationale is a claim, and the least-audited kind*. | *(index row backfilled 2026-09-02 from the entry body)* |
| R-107 | 2026-08-21 | hit | **A spec's account of an EXISTING table is a claim, and the constraint layer is where it is least likely to have been read.** Caught before modifying `link_scan::call` (220 lines) to materialize entry-grain citations into `entry_cite`, per a spec that described that table's prune-and-re-materialize behaviour as something already supported. | *(index row backfilled 2026-09-02 from the entry body)* |
| R-106 | 2026-08-18 | hit (pre-edit) → rule | **A generated surface is ground truth about cost and about nothing else — read the generator before proposing the remedy.** Measured the live `tools/list` payload, found one 225-char `workspace` description repeated verbatim on 24 tools, and proposed a "free mechanical dedup, zero risk". The source holds exactly ONE copy: `inject_workspace_param` injects it into every `pinnable()` tool at list time | `src/server.rs:496-508` + `:1024-1026`; prompt-surface-compaction-session-log:F-2, :W-1; kin R-91, R-100 |
| R-105 | 2026-08-18 | miss (human review) → rule | **A key derived from runtime state is a claim about that state's lifetime — enumerate the lifecycle events before proposing it.** Proposed parent-PID + parent-start-time as the agent-agnostic session key for the guide ledger and scouted it hard against ONE event (MCP subprocess respawn); never enumerated client *resume*, which restarts the parent while the conversation continues. The falsifying evidence was already in hand and unread | bug-fix-session-log:F-53; session 2c518eb6 spans 12 days / 9630 calls / 67 MCP procs under a 17h-old `claude` process; kin R-91, R-50 |
| R-104 | 2026-08-17 · 2026-08-21 | miss ×8 (self-caught) → **promoted** | **A zero from a report is a claim about your query, not about the world — and it lies in three independent ways.** Five wrong conclusions in one session, from a wrong key name (searched `"token"` while the `dangling` array calls the field `raw` — half the population never searched), a wrong value vocabulary (filtered on `verdict != "ok"` against a five-value domain, printing *resolved* refs as problems), and cap truncation (ref absent from `findings[]`, but `n_refs_found` was 64 against a 50-entry window). Structural anchoring is necessary and insufficient: it protects the technique while the **domain** is guessed. **Widened 2026-08-21:** not about zeros and not about reports — about **who supplied the predicate**. A hand-built instrument (path, pattern, sort key, field name) answers in its own terms without complaint, and can return a full, plausible, WRONG answer with no zero involved. Remedy is a positive control, not care | 5 instances incl. `grep -c 'Status:'` counting prose about Status, and a `status: mitigated` "what's open" query 2.5× inflated by already-archived rows. Substrate moving to retire it: `f908e883` (`severity_legend`), `7c218338` (link_scan field unification) — and that prediction **held**: `counts.entry_edges` read cleanly on sight 2026-08-21. Three further instances that day were all hand-rolled shell, where no publisher can add a legend: a version-keyed cache path guessed flat (empty output, not even `0`), a backtick eaten by `grep -c "$s"`, and `sort` on `ps lstart` ordering by weekday name. Kin R-101, R-103 |
| R-103 | 2026-08-17 | miss (peer-caught) → rule | **A blast-radius audit is not a correctness audit, and enumerating the call sites makes it feel like both.** Read all four callers of a changed eligibility rule and correctly guarded `resolve_link` — then missed a pre-existing defect in the exact `else` arm just read, because the question asked was only *"does my change break this caller?"*, never *"is this caller correct?"*. The suppressor: an in-code comment asserting cross-site parity that described **intent**, not behaviour — the cheapest false positive, sitting where you would look to verify | `da55100a` (the incomplete audit) vs `3faddb15` (peer session's correction, hours later). `FileMissing` carries gating severity, so the miss would have failed CI on a correct doc — the same class the parent bug was filed for. Kin R-101, R-102, R-104 |
| R-102 | 2026-08-17 | miss ×3 (self-caught) → rule | **A root cause read from code is a hypothesis about which true statement is the OPERATIVE one; implementing the fix is the measurement.** Three bug files in one session, three root causes written from careful reading, all three wrong, not one caught by re-reading: a two-part predicate rebutted as though it had no command list, a correct mechanism with both prescriptions wrong, and a diagnostic that does not discriminate. Sharpest tell — a root cause asserting an **absence**; reading establishes presence, only a search establishes absence | 3 datapoints, one session: `2026-08-15-read-only-metadata-commands-blocked-on-source-paths`, `2026-08-15-read-file-force-ignored-on-full-reads`, `2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary`. The measured/inferred label is what kept all three cheap (`da55100a`). Kin R-101 (one layer in), R-103 |
| R-101 | 2026-08-17 | miss (self-caught) → rule | **A test that DISTINGUISHES two hypotheses is not confirmation of one — state what the RIVAL hypothesis predicts before recording a verdict.** Ran a discriminating test on `include_str!` ref resolution and recorded it as *"confirmed — the base directory was the whole of it"*; the outcome actually **ruled out** the hypothesis it was read as confirming. The measurement was correct and already in hand, scored against one hypothesis in the direction committed to in prose two paragraphs above. Detector: for every `**Verdict:** confirmed`, complete *"under the rival hypothesis this test would have shown ___"* | `docs/issues/archive/2026-08-17-audit-doc-refs-misreads-include-str-arg-as-doc-relative.md`; corrected in `da55100a`, which also found the sketched parser-side fix unreachable. The bug's *"inferred, not cited"* label is what made the wrong mechanism cheap. Kin R-102, R-103, R-104 |
| R-109 | 2026-08-08 | miss | A host-specific symptom accepted as the norm without sweeping the other hosts | 9 of 15 declared roots swept; all nine had zero untracked entries |
| R-110 | 2026-08-08 | miss | An identifier's shape says nothing about whether the thing exists — check its declared root | caught by an unrelated tool response an hour after the claim was published |
| R-119 | 2026-08-27 | miss — a rebuild caught it, not recon; recon had the evidence and did not look | **Two enumerations of my own loaded context disagreed in-session (18 vs 23 memories) and I never compared them.** Law A (*ground truth is the artifact; everything else is a claim about it*) applied one level up than usual — not to a symbol, but to the **enumeration of what I had loaded**. Both listings sat in context, and neither was treated as a claim about the other. | `codescout:020ea69a`; *(index row backfilled 2026-09-02 from the entry body)* |
| R-118 | 2026-08-27 | hit — recon's own controls caught it before anything was written down | **Scouting a fix primed me to find its bug where it wasn't — and a BLOCKED ground-truth check left the belief unmarked.** Law C (*a search that finds nothing is evidence about the search*) **over**-applied: the first of its kind among the ledger's 16 C-entries, every other one being law C *under*-applied — a zero read as fact when it was an artefact of the search. The inverse has its own trigger: a refused or errored verification leaves the prior belief standing with nothing marking it unverified. | *(index row backfilled 2026-09-02 from the entry body)* |
| R-117 | 2026-08-27 | hit (pre-edit) → **promoted** | **A fix that names a POPULATION asserts it is non-empty — and that assertion is the one that fails green.** A bug file's `## Fix` prescribed dropping "rows under a *different* managed root"; `managed_roots` never returns a foreign repo, so the case was the empty set and the fix would have compiled, passed a test written from the same model, and moved zero rows. Third grammar of the promoted proposed-fix law, so the bullet was WIDENED rather than a fourth added | `bug-fix-session-log:F-74`; measured partition 402 → 359 umbrella / 33 known-repo / 10 orphan; kin [[R-106]] (24 "duplicates" were one source copy) and the 32 KB hook whose trigger population was empty |
| R-111 | 2026-08-08 | hit | Before fixing a heuristic, grep for other copies of it — `references()` cannot see a duplicated closure | `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` |
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
| R-115 | 2026-08-15 | miss ×2 → rule | **Aggregate behaviour data is a screen, not a verdict — and it is wrong in BOTH directions.** An audit of 53,916 recorded tool calls ranked failures by frequency and by "what tool came next". Reading the actual arguments of ~12 instances overturned two conclusions. (a) **Overstated:** every rejection of a `json_path` wildcard was scored a successful recovery because the next call returned `success`. The arguments showed what those recoveries were — `$.items[*].abs_path` → read 393 raw lines; `$.entries[*].id` → `$.entries[4].id` one element per call; another → abandon the buffer and re-query upstream; another → grep the buffer. A green outcome column concealed a degraded workaround every time: it records that the call returned, never that the agent got what it asked for. (b) **Understated:** a guard scored at 35% "same-tool recovery" was in fact working — its correct recovery is a DIFFERENT tool (`symbols(include_body)` on the exact symbol wanted), which the same-tool metric counts as failure. Same-tool and adjacent-call metrics systematically penalise every guard whose right answer is another tool or needs a lookup first (widening one route's window from 1 call to 3 moved compliance 45% → 74%). And the sharpest defect was invisible to all aggregation: bucketing the refused ranges showed 69 of 244 were canonical imports reads (`1-20`, `1-30`), refused by a guard whose recommended alternative structurally cannot return imports. **Rule:** before filing or closing anything from aggregate counts, read the arguments of ten instances — and check whether the payload that would show intent is even being recorded. | 2026-08-15 tool-usage investigation TU-11; `docs/trackers/2026-08-15-tool-usage-investigation.md`. Kin R-50 (the view is not the set — this is its behavioural-telemetry form), R-75 (verify the artifact produced, not the exit code) |
| R-116 | 2026-08-26 | miss | **A positive control validates the INSTRUMENT, never the SUBSTRATE.** Phase 1's substrate law was read, quoted aloud to the user, then violated for a whole session: `.codescout/embeddings/codescout.db` was queried as the live index when `CODESCOUT_VECTOR_BACKEND` is unset and `VectorBackend::resolve` defaults to Qdrant under `server-stack`. Live 1611 files / 47 647 chunks vs the file's 1593 / 46 979. Three things defeated the existing text: the backend was taken from the *reporter's* env block rather than this host's; a 247 MB file with the right name and schema is a powerful false confirmation, so no zero and no error gave PROBES rules 3/4 anything to catch on; and a positive control on the join **passed** — proving the predicate discriminated, in the wrong database. Caught only by `index(action="verify")`'s first live run disagreeing. | `bug-fix-session-log:F-66`; `tool-usage-patterns:T-28`; promoted to codescout memory `gotchas` |
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

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the **served** `1.16.13` cache: back-citation
*"(R-1 + R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* present,
1 occurrence. This entry and [[R-3]] are the ledger's two oldest promotions and the only ones
that anchored on a back-citation from the start — which is why they are the only two that
survived every later rewording of the surrounding bullets without maintenance.

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

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the **served** `1.16.13` cache: back-citation
*"(R-3 → R-113 → R-77 → R-79 in codescout's `docs/trackers/reconnaissance-patterns.md` …)"*
present, 1 occurrence. The bullet has been rewritten and extended repeatedly since 2026-05
and the anchor still resolves — a verbatim quote would have needed re-syncing each time.

**Status:** promoted to SKILL.md (claude-plugins:787cdec0, 2026-05-23). Added as a 4th bullet under Phase 1 — Scout, citing this R-3 row by name. Promote-when criterion fired with 1/1 datapoint, per the tracker's note ("already validated as a needed default").

---

## R-4 — Grep undercounts struct-field construction sites by 2-3×

**Status:** open — verdict `miss` (Index row). Not promoted; not back-cited in the served
`SKILL.md`.

**Corrected 2026-08-20, hours after being set wrong by this same sweep.** An earlier pass this
day marked this entry `promoted` on the strength of `grep -c 'R-4'` against the skill returning
1. That predicate counts **any mention**. The single hit is an eval-fixture list — *"the six
MISS cases (R-2, R-4, R-8, R-10, R-19, R-23) as a hard regression gate"* — which records that
this entry is a **test case for** the skill, the opposite of a lesson promoted **into** it.
Naming what the predicate literally counts, then comparing that to the question, is the
control that was skipped. Same error hit [[R-8]] and [[R-87]].

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

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

**Status:** open — verdict `miss → proposal` (Index row). The proposal has **not** landed; not
back-cited in the served `SKILL.md`.

**Corrected 2026-08-20**, same error as [[R-4]]: marked `promoted` on a `grep -c` that counts
any mention, where the sole hit is the eval-fixture regression-gate list. The Index's
`proposal` was right all along.

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

**Status:** open — verdict `hit → proposal` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. Not back-cited in the served `SKILL.md`, so the proposal has not landed; datapoint count not re-assessed here.

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

**Status:** open — verdict `hit → proposal` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. Not back-cited in the served `SKILL.md`, so the proposal has not landed; datapoint count not re-assessed here.

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

**Status:** promoted — the SKILL.md change described below **did land**. This line's closing
sentence, *"Formal sync flow … still pending a commit"*, was true when written and has been
false ever since; nothing re-read it.

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ When NOT to Use — the *describe-vs-assert* cut, back-cited as `(R-19)`. D11-verified
2026-08-20 in the served `1.16.14` cache, 1 occurrence.

**Fourth datapoint for the conditional-disposition class** — with `bug-fix-session-log:F-3`
(*"after plan edit lands this turn"*), `:F-17` (*"planned"*) and `:F-44` (*"should be
updated"*). A disposition field should record what **is**; intent belongs in a `Fix idea` /
`Next` line where nothing reads it as state. Found 2026-08-20 while correcting three
mislabelled promotions in this same ledger — the audit that produced the error also produced
the datapoint.

**Original assessment, unchanged — 3 datapoints** (2 this session: a hit then a miss; + 1 cross-session retroactive hit 2026-06-21, see Recurrence above — re-confirmed all three shapes, caught line-citation drift). The 3rd does **not** advance the promote-when (it confirmed, did not refute) and does **not** verify skill efficacy (recon was again user-invoked, not auto-fired) — efficacy still N=0. **Acted 2026-06-09 (user decision):** rejected the project-local CLAUDE.md route as too narrow for a systemic lesson; instead tightened the recon SKILL.md `When NOT to Use` Read-only-Q&A exemption to draw the *describe-vs-assert* line (Hamsa-audited — a cut/bound, not an added trigger; `claude-plugins` working tree, uncommitted). This front-runs the cross-session-3rd caveat by deliberate choice (recurrence-after-documentation judged strong enough). **Efficacy unverified — N=0**: no behavioral eval (`docs/evals/reconnaissance-output.md` not yet authored); the existing trigger eval scores the description string, not body guidance, so it does not measure this change. Formal sync flow (PR + pinned SKILL.md commit SHA + skill version) still pending a commit.

**Source:** `src/tools/output_buffer.rs:251`, `src/tools/core/types.rs:435`, `src/retrieval/sync.rs:34`, `src/tools/run_command/tests.rs:2034` (line numbers refreshed 2026-06-21; symbol names are the stable anchors); this session's headroom cross-pollination analysis. Kin: R-14.

**Valid:** dated 2026-08-20

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

(hit) Before testing shared-LSP behavior under concurrency, I scouted how the mux lock/socket + RocksDB home are keyed: `workspace_hash(workspace_root)` — the path (`src/lsp/mux/mod.rs:15-28`; `servers/mod.rs:81`; regression `socket_path_deterministic_for_same_workspace`). That single fact determined the whole test design — isolated worktree hashes cannot collide with the live mux, so a 3-agent concurrent cold-start runs safely on a working machine. Directly answers last session's R-23 miss (verifying shared-resource recovery by contending on the live hash). The same run also surfaced a 4-site `workspace=` pin gap and a concurrent-cold-start kotlin-lsp failure.

**Evidence:** bug-fix W-13 + F-18; `issues/2026-06-11-lsp-tools-ignore-workspace-pin-path`.
## R-25 — Scout the catalog's status source + id-keying before archiving a librarian tracker

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

(hit) `/reconnaissance` invoked as I was about to archive 3 session-logs by hand-editing `status: archived` frontmatter + `git mv` to `archive/`. Scouting the librarian first overturned the plan: `artifact(find)`'s active-vs-hidden verdict reads the **catalog DB row's** `status`, not the file (`find.rs:90` → `{nin: HIDDEN_STATUSES}`, defined `mod.rs:27`). A `git mv` + frontmatter edit touches neither the catalog status nor — until someone runs `reindex` — anything the librarian queries, so all 3 would have kept showing `active`. Worse: `id = sha256(abs_path)[..16]` (`ids.rs:17-23`), so the eventual reindex mints a NEW id for the moved file and the cleanup `DELETE … id NOT IN seen_ids` (`indexer.rs:56-237`, `index_repo_sync`) drops the old row — orphaning its `events`/augmentation. Correct mechanism: `artifact(action="update", patch={status:"archived"})` (writes catalog row + frontmatter now) then `artifact(action="move")` (atomic rename + abs_path re-point, `mv.rs:14-75`). Path-keyed-id kin of R-24 and R-15.

**Evidence:** this session — archived `kotlin-lsp-disk` + `metadata-filtering` + `mcp-prompt-redesign` session-logs; commit `b487a69c`. Doc gap (promotion candidate): CLAUDE.md's archive instruction ("set `status: archived` AND `git mv`") is file-only — it omits that the librarian's catalog status needs a *tool* write, so following it literally produces a cosmetically-archived-but-still-active tracker. Fix → prescribe `artifact(update)`+`artifact(move)` for librarian-tracked files.

## R-26 — A grep line-match locates a symbol; it does not confirm a mechanism

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

(hit) `/reconnaissance` invoked after I'd already written "the orphan-squatter mechanism, confirmed" into a mux-LSP-sharing brainstorm — but "confirmed" rested on a `grep` hit (`kill_on_drop` at `process.rs:93`, `Command::new` at `:86`), not a read of the function. Scouting the actual body (`run`, `src/lsp/mux/process.rs:66-135`) is what earned the word: the LSP child is spawned `.kill_on_drop(true)` with **no** `setsid` / `process_group` / `pre_exec` / signal handler anywhere in the spawn path, so `kill_on_drop` — which rides `Child::drop` and never runs under SIGKILL — is the *sole* teardown. The SIGKILL-mux → orphaned-JVM → immortal-RocksDB-lock-holder mechanism (the systemd-reparented kotlin-lsp we found on backend-kotlin) therefore holds. The grep proved *presence* of `kill_on_drop`; only the read ruled out the *falsifier* — a process-group or signal handler that would reap the child regardless. Lesson: a grep line-number is Phase-1 location, not Phase-2 confirmation — never narrate "confirmed" on a mechanism off a grep hit; read the body. Kin of R-19 (assert-a-checkable-fact only after reading) and R-5 (grep ≠ proof).

**Evidence:** this session — backend-kotlin mux/RocksDB lock-contention brainstorm; claim verified read-only against `src/lsp/mux/process.rs:66-135` (no commit). Open edge (R-15 kin, still unscouted): the same brainstorm's *upgrade* recommendation rests on an upstream changelog line — kotlin-lsp v261.13587.0 "indices … properly shared between multiple projects and LS instances" — known only via a WebFetch fast-model paraphrase of `RELEASES.md`, not a raw read; explicitly flagged verify-before-acting and not yet acted on.

## R-28 — Enumerate a prompt surface's full gate set before editing; targeted test filters miss cross-cutting gates

**Status:** open — verdict `hit + miss` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

(hit + miss) Editing the onboarding system-prompt instructions touched `source.md` (onboarding_prompt surface), `memory-templates.md` (`{{include}}`'d into both single + workspace flows), `workspace_onboarding_prompt.md`, and `builders.rs`. **Hit:** pre-edit recon enumerated the gate set — `extracts_onboarding_prompt_byte_for_byte`, the `prompt_surfaces_onboarding_snapshot` fixture, the 2200-byte `source_md_under_cap`, the `assert_eq!(ONBOARDING_VERSION, 28)` version-pin, the workspace-scope heading-presence test, and the `"6 memories"` content test — so every gate update landed in one commit and only the snapshot needed a legitimate re-bless. **Miss:** a sibling change earlier the same session (get_guide description, `c799e887`) was verified with a *targeted* `cargo test --lib guide::` that did not include `server::tests::tool_descriptions_stay_under_budget` (`src/server.rs:1598`); the 329-char (cap 300) description shipped and was caught only by the onboarding task's full-suite run (bug-fix F-20). Lesson: a tool/field's validating gate often lives in a DIFFERENT module (`server::tests`) than the code it guards; `cargo test --lib <module>` filters silently skip it. Enumerate gates by what they assert on, not by where the edit sits — and run the full suite (or `server::`) for anything a global budget/snapshot gate validates. Kin R-1/R-7 (include_str'd-constant invariants), R-27 (verify before building on a claim).

**Evidence:** bug-fix F-20 + W-15; commits `8427ae4a` (onboarding fix), `31a655e5` (description-budget), `c799e887` (where the gap shipped); verified read-only against `src/prompts/source.rs`, `src/prompts/mod.rs`, `src/server.rs:1598`.
## R-29 — Verify a flight-recorder-harvested target exists in the active repo before ranking it

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

**Date:** 2026-06-13 · **Verdict:** hit · **Kin:** R-23 (usage.db telemetry re-derivation), R-19 (cross-project recommendation scouting)

**Context:** Dzo Phase-1 legibility survey — ranking refactor targets by truncated-`symbols` recurrence harvested from `.codescout/usage.db`.

**Scout:** Before ranking, existence-checked each harvested target against the repo (`grep -rl <symbol> --include=*.rs`, `wc -l <path>`, `symbols` resolution). `CalendarService` (3 truncated fetches) failed the check; a follow-up `project_sha` query traced it to `project_sha=1e8b9eb1`, path `/home/marius/work/mirela/backend-kotlin/.worktrees/cs-stress-*/.../CalendarService.kt` — a mirela Kotlin stress-test session.

**Finding:** `.codescout/usage.db` is **not** single-project. It is keyed by commit-SHA-at-call-time and holds 40 distinct `project_sha` values, mixing every project the process-global server served. A symbol/path harvested from telemetry is a *candidate*, not a confirmed in-repo target.

**Lesson (the recon rule):** Telemetry-harvested targets need an in-repo existence verification before they drive a ranking or a refactor. The verification is one `grep -rl` per path-less candidate; the filter for a clean survey is `WHERE project_sha IN (<repo's shas>)` or a path-prefix match.

**Counterfactual:** Without the check, `CalendarService` (tied with mid-tier real targets at 3 fetches) ranks ~#4, and the Dzo runs `symbols`/`semantic_search` readings that return empty against code not in the repo — churn, and a possible phantom tracker.

**Proposal:** Bake a `project_sha`/path-prefix filter into the Pika + Dzo survey queries so the contamination is excluded at the source, not caught per-symbol. Promote-when: a second cross-project phantom surfaces in a flight-recorder survey.

**Evidence:** `docs/trackers/archive/dzo-legibility-session-log.md` F-1 + W-1.

## R-33 — Dead-vs-live is a per-symbol call-graph fact, not a file-proximity fact

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

**Date:** 2026-06-15 · **Verdict:** hit

**Seam:** `src/embed/fusion.rs::rrf_fuse` + `BM25Result` vs `src/embed/schema.rs::SearchResult`. The legacy-retrieval-removal tracker's 2026-06-14 reconciliation marked **both** "graduated into live consumers — NOT deletable" because they are adjacent symbols from the same legacy-fusion era.

**Scout:** `references(rrf_fuse)` → 1 def + 5 test-only call sites, **zero production callers**. `references(SearchResult)` → a live consumer (`apply_file_diversity_cap` in `semantic_search.rs`). The two symbols had **opposite** liveness; file proximity hid it.

**Disproof:** deleted `fusion.rs` (+ its `pub mod` decl + the dead `rrf_fuse_integration_*` test); `cargo test` 2869 passed, clippy `-D warnings` clean. The "not deletable" claim was false for `rrf_fuse`.

**Lesson:** an audit that reasons about dead-vs-live from imports / co-location / "same era" will mis-classify. Liveness is a per-symbol call-graph fact — confirm each symbol with `references()` / `call_graph()`, never by its neighborhood.

**Promote-when:** a 2nd proximity-misclassification caught by a call-graph scout → promote to the reconnaissance SKILL ("dead-code claims are per-symbol call-graph facts, not file-level facts"). Kin: R-26 (grep line-match ≠ mechanism), R-27 (prose claim is a hypothesis), R-21 (`references()` before acting).

## R-34 — On a cross-platform branch, the host `cargo check` is not the rebase gate; cross-compile to the target the branch exists to support

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

(hit) Rebased `vdi-windows` onto `experiments` — 41 ours / 89 theirs. The sole conflict was in `src/tools/mod.rs`, in the very `#[cfg(unix)]` gating the branch exists to maintain: `experiments` added `pub mod guide_ledger;` and our `d07d7b18` added `#[cfg(unix)]` to `pub mod peer;` at the same line; both kept. The seam I scouted was not the conflict (that resolution is trivially correct — two independent edits) but the **completeness of the platform gating after absorbing 89 incoming commits**. A host `cargo check` (Linux/unix) only re-compiles the path that already built; it never exercises the Windows side, so it cannot answer "is the rebase correct?" for the dimension this branch is *about*. Scouted the real target: `cargo check --target x86_64-pc-windows-gnu` (MinGW + windows-gnu target both present from the branch's existing local loop) — EXIT=0, clean but for 3 pre-existing dead-code warnings (unix-only LSP-mux helpers in `src/lsp/manager.rs`, already tracked as WIN-23). 

**Counterfactual:** had any of the 89 `experiments` commits introduced an ungated `peer::` use, `std::os::unix::*` import, or other unix-only call, a Linux-only "looks good" would have shipped a broken Windows build that surfaces only on the user's next VDI compile — maximally far from the rebase, hard to attribute back to it. The cross-compile makes the gate fire at the seam, not in the field.

**Generalization:** when rebasing/merging a branch whose purpose is a non-host target (platform gating, no_std, a different arch/ABI, a feature-flag matrix), the host build is necessary-not-sufficient. Compile the target the branch exists for before claiming the integration is correct. Mirrors R-5 ("compiler as scout") but on the *target* axis rather than the call-site axis.

**Evidence:** `vdi-windows` rebase this session (master-side conflict resolve commit on the rebased tip); `src/tools/mod.rs` peer-gate; `cargo check --target x86_64-pc-windows-gnu` EXIT=0; WIN-23 dead-code warning cluster (`windows-platform-support.md`).
## R-35 — A tool's own error diagnostic is a hypothesis, not ground truth; reproduce the failing internal call on the real file

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. Cited by § *The seven laws* as the head of the error-message chain (R-35 → R-71 → R-82), so the lesson is distilled in-file even though the entry itself is not back-cited in the served `SKILL.md`.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

**Observed:** 2026-07-10, unifying the path-param alias set across the file/markdown/symbol tools and adding `file_path`/`output_id` schema properties (param-alias-ergonomics session, driven by a usage.db error-pattern sweep across 72 project DBs).

**Scout done (before editing a shared contract + several `input_schema`s):** grepped the whole tree for schema-shape tests. Every `*_schema_*` test is *positive-presence* — `schema["properties"]["x"].is_object()`, `contains_key(...)`, `enum.contains(...)`; none enumerate the exact property set or assert a property is *absent*. Also confirmed no tool `input_schema()` sets `additionalProperties: false` (`src/server.rs` assembles the schema object without it).

**Two consequences, both confirming the edit was safe:** (1) adding schema properties (`file_path`, `output_id`) *cannot* break any existing test; (2) aliases work at runtime *precisely because* there is no strict schema validation — an unknown key flows through to `call()`, which is the whole mechanism the alias fix relies on. The `create_file` `file_path` fix earlier in the session had already proven the passthrough empirically.

**Verdict:** hit — confirmed empirically. After adding `require_str_param_or_hint`, the shared `fs::require_path_param` alias, and 3 schema props, the full `cargo test --lib` run was **3038 passed / 0 failed**. The scout let me add the schema surface without hedging or a discover-by-breakage cycle.

**Generalization:** before worrying that adding a tool param/alias will break a schema snapshot or be rejected by strict validation — in codescout neither risk exists. Schema tests guard *presence*, not exact shape; schemas are *open* (`additionalProperties` unset). A param/alias addition is additive-safe. The one real gate a *description* edit must still clear is `server::tests::tool_descriptions_stay_under_budget` (R-28/R-37) — but adding a schema *property* (not touching `description()`) doesn't approach it. Kin: R-36 (serde has no `deny_unknown_fields` → the sibling open-schema property at the config-data layer).

**Evidence:** positive-presence tests in `src/tools/{memory,semantic,symbol,ast,run_command}/tests.rs` + `src/tools/peer.rs`; schema assembly in `src/server.rs` (no `additionalProperties`); gate: 3038 passed / 0 failed. Promote-when (2nd datapoint): distill "adding a tool param/alias is additive-safe — schema tests are presence-only, schemas are open" into codescout memory `reconnaissance`. param-alias-ergonomics session (this session); commit pending.

## R-41 — A table-rebuild migration's `INSERT … SELECT` column list is a silent allow-list

**Status:** promoted — verdict `miss → promoted` (Index row). **Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md` § Phase 1 — Scout, as the *seam class: schema-migration ordering* bullet. D11-verified 2026-08-20 in the served `1.16.13` cache: back-cited by id, 1 occurrence. The Index knew; the entry did not.

**Verdict:** miss → promoted (was a pending seam-class in SKILL.md Phase 1; Stage-2 is the confirming datapoint)

**Observed:** 2026-07-17, entry-graph Stage-2 (v9 catalog migration adding `artifact.slug` + `entry_cite`). The implementer added the column; an Opus task review caught the drop.

**Pattern:** Adding or changing a column/field is a seam whose far side is every LATER migration that rebuilds the same table (`CREATE table_new` / `INSERT … SELECT` / `DROP old` / `RENAME`). The rebuild's `INSERT … SELECT` column list is a silent ALLOW-LIST: any column it does not name is dropped when the rebuilt table is swapped in — no error, no failing test unless one asserts the column survives. The scout question is not "did I add the column?" but "does every column an earlier migration added still appear in every later rebuild's SELECT?"

**What happened:** v9 added `artifact.slug`. An earlier migration, `migrate_v6.rs::drop_legacy_and_stamp`, rebuilds the `artifact` table via copy-and-rename; its `INSERT … SELECT` did not name `slug`, so a single open through v6 dropped the just-added column (and cascade-dropped `entry_cite`). The bug shipped into the branch; the Opus task-1 review caught it against the diff, not the implementer's own scout.

**Counterfactual:** Untested, a v6-path open would silently degrade every entry-graph tracker to file-grain — slugs gone, cites cascade-dropped — with green tests, surfacing only when a downstream cite query returned empty.

**Proposal:** promote from pending seam-class to a Phase-1 named seam. When a diff adds a column, `grep` the migrations dir for the table name and read every later rebuild's SELECT list; a migration-shape regression test (open through the rebuild, assert the new column's value survives) is the durable gate. Fixed 9aa8063f + `migration_v6_single_open_preserves_v9_entry_graph_shape`.

**Evidence:** tracker-entry-graph Stage-2 (experiments); `src/librarian/catalog/migrate_v6.rs`; fix 9aa8063f; test `migration_v6_single_open_preserves_v9_entry_graph_shape`; kin R-3 / R-28.

## R-42 — A reader's None/absent branch that dead-ends silently drops every value the writer stored in that variant

**Status:** promoted — verdict `miss → promoted` (Index row). **Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md` § Phase 1 — Scout, as the *seam class: writer-shape ↔ reader-surfacing* bullet. D11-verified 2026-08-20 in the served `1.16.13` cache: back-cited by id, 1 occurrence. The Index knew; the entry did not.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

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
`docs/trackers/archive/dependency-review-session-log-2026-08-25.md` F-3 + W-2; kin R-43 (read-side
twin), R-5 (compiler backstop that would NOT have caught the dead test), R-17
(sibling-caller spot-check).
## R-45 — Hit: relocating a file needs a discovery-by-scan grep, which caller enumeration cannot substitute for

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

**Verdict:** hit (ordering caveat — recon ran after the edit, before the gate).

**Valid:** invariant

The claim is a structural property of two techniques, not of this incident: caller
enumeration answers *who calls this symbol*, and a relocation changes *where a file lives*,
so no amount of caller coverage reaches the consumer that resolves a path. The `mux_dir`
case is evidence for it, not the claim. Declared 2026-09-01.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

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

**Valid:** dated 2026-09-02

*(Re-measured 2026-09-02 — see § **Re-run** at the end. The declaration previously read `dated 2026-07-28` and sat **between items 1 and 2 of the numbered list below**, at column 0, which is where the parser wants it and nowhere a reader wants it: with no blank line before `2.`, rule 2 rendered as lazy continuation of the `**Valid:**` paragraph rather than as a list item, so the entry's "two separable rules" displayed as one. `scan_dated_stale` reported this entry for 35 days and could not have reported that — it parses the declaration and has nothing to say about the prose around it.)*

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

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the **served** `1.16.13` cache, not the repo
source: back-citation *"(R-49 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"*
present, 1 occurrence.

**Status:** promoted-to-permanent-docs — landed 2026-08-20 in `claude-plugins:23a11c3`,
shipped to all three profile caches in `1.16.11` (`claude-plugins:23ca288`) and verified
there at the served bytes. The commit alone did not make it live — see
`prompt-surface-compaction-session-log:F-9`. In
`claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md` § Phase 1 — Scout, as a
bullet opening verbatim: *"Re-entering your OWN bug file or plan to implement it counts as a
seam — authorship is no exemption."* The read-the-layer-between-two-functions clause and the
*temporal, not attentional* countermeasure both carried across. Also had no `Status:` line
before the `F-7` sweep.

### Re-run 2026-09-02 — the rate held; the countermeasure did not

Produced by `librarian(action="doctor")`'s `entry_dated_stale`, which reported this entry at
**35 days, exposure 17** — the highest-exposure decayed statement in the project. The instruction
that check gives is *re-run the measurement and record the new figure*, so:

**The original figure, 2026-07-28:** *"the third session-authored artifact to fail under later
scrutiny in one sitting"* — `R-46`/`F-34`, `F-35`, `F-37`. **n=3, one sitting.**

**The new figure, 2026-09-02, one sitting:** **two more**, both by this entry's re-reader, and one
of them is the tight case rather than the loose one.

- **`tracker-hygiene-log:HY-25` — a full instance, both halves.** Written while doing the work it
  describes, published with `**Mechanism status:** none yet` and a three-option list, then re-entered
  **to implement those options** — R-49's exact trigger. Option 3 had shipped two weeks earlier as
  `scan_cited_but_undeclared`. Retracted 40 minutes after filing.
- **`bug-fix-session-log:F-96` — the mechanism half only, recorded as such.** A claim (`core.hooksPath
  points at scripts/`) published to three surfaces *as a reason*, false at two levels. It matches
  *"written while doing the work they describe, when the writer's model is most confident and least
  tested"*, but nobody re-entered it to implement anything, so it is not the re-entry trigger. Counted
  under the mechanism, not under the trigger — the distinction this entry's own "n=3 in one sitting"
  did not draw, and should have.

**Running total: n=5 across two sittings 36 days apart**, unit = *session-authored artifacts that
failed under later scrutiny in the sitting that produced them*. The rate claim holds.

**What does NOT hold is this entry's countermeasure, and that is the finding.** *"The countermeasure
is temporal, not attentional — re-read on re-entry, not harder on write"* is half right, and the half
it misses is the one that did the work:

| sitting | instances | what actually caught them |
|---|---|---|
| 2026-07-28 | 3 | **the author's own re-scout** — temporal, same observer |
| 2026-09-02 | 2 | **neither was self-caught** |

`F-96` was caught by a **peer session** (`codescout-3e`) contradicting a published claim. `HY-25` was
caught by an **unconditional standing rule** — CLAUDE.md § *Observer Blindness*'s *grep `tests/`,
`scripts/pre-commit-*` and hooks for a population's name before any campaign over it* — which fires
on "about to start a campaign", a trigger that occurs anyway and is unrelated to suspicion. **In
neither case did re-reading on re-entry fire.** In `HY-25`'s case re-entry *did* happen — that is
what the session was doing — and re-reading the entry produced no doubt at all, because the entry was
internally consistent. What broke it was reading a **different artifact** (the code).

So the refinement: **re-reading is temporal but it is still the same observer, and this class's
defining property is that the author holds the belief that makes the error invisible.** Ranked by
what was measured rather than by what is comfortable:

1. **A standing rule tied to an unrelated trigger** — fires without suspicion. Caught `HY-25`.
2. **A peer with a different context** — not a more careful reader, a differently-situated one.
   Caught `F-96`.
3. **The author's own re-read on re-entry** — what this entry recommends. Caught 3 of 3 in 2026-07,
   **0 of 2 in 2026-09.** Not refuted — 5 datapoints, and the 2026-07 three are real — but it is now
   the *weakest* of three measured instruments rather than the recommendation.

**Owed downstream, not done here:** the served `SKILL.md` § Phase 1 bullet carries the *temporal, not
attentional* wording verbatim, so the skill currently recommends the instrument that scored 0 of 2.
That is a cross-repo edit in `claude-plugins/codescout-companion/` and a change to a promoted law, so
it is named rather than taken unilaterally — and it needs a third sitting before the ranking above is
worth shipping into a skill. **Re-run trigger:** the next time `entry_dated_stale` reports this entry.

**Status:** promoted-to-permanent-docs, and **the promoted wording is now known to be incomplete** —
see § *Re-run 2026-09-02*.

## R-50 — The view is not the set: five errors, one shape

**Status:** open — verdict `miss → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

**Verdict:** miss → rule. Recon did not prevent these; they were caught downstream, each
by a different accident. The value is in the shape they share.

**Valid:** invariant

*The view is not the set* is a law about filtered reads; the five instances are its
evidence. A sixth would strengthen it, never date it. Declared 2026-09-01.

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

**Status:** promoted 2026-08-20 — verdict `miss → rule, promote-ready` (Index row). The
`promote-ready` mark had stood since **2026-08-04, sixteen days**, and nothing ever queried
for that state.

**Valid:** invariant

An instrument writing into its own corpus is a hazard of the arrangement, not of any one
probe. Corroborated by the project itself: this entry is already promoted into the served
`SKILL.md`, which is what a law-shaped claim earns. Declared 2026-09-01.

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the served `1.16.14` cache: back-cited as
*"(R-51, the complement of R-50, in codescout's `docs/trackers/reconnaissance-patterns.md`.)"*,
1 occurrence. The bullet carries **both** forms this entry's verdict asked for — where output
*happens* to land, and whether the system's own emissions re-enter its input — plus the
self-confirming-trailer corollary.

This entry is the concrete answer to the re-open criterion on
`docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md`: *"re-open … if
an unharvested `Promote-when` is found to have cost something."* It was found by the sweep
that bug provoked, and it had been sitting in plain sight, marked ready, in a field no query
read.

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

**Status:** open — verdict `miss → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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

**Status:** open — verdict `miss → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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

**Status:** open — verdict `miss → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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


## R-109 — Miss: a host-specific symptom accepted as the norm without sweeping the other hosts

**Verdict:** miss (caught by a later sweep, not by the scout that should have run first)

2026-08-08, reviewing a bug file that reported 8 permanent `??` entries under `.codescout/projects/` and framed them as "the normal state for the cross-project flows CLAUDE.md documents". The framing was accepted long enough to build a re-diagnosis on top of it.

The cheap check was a sweep of the population the claim generalises over. `~/.config/librarian/workspace.toml` declares 10 `[[roots]]` plus two umbrellas — 15 paths. Nine of those repos have a `.codescout/projects/` tree; **all nine had zero untracked entries**, via three different mechanisms (nothing generated lands there; a structural allow-list; a blanket `.codescout/` ignore). The reported host was an outlier, not the baseline.

**The trap that made "zero" misleading in the other direction:** two of the nine carried `projects/<id>/` directories that were **empty**, and git cannot see an empty directory — so a clean `git status` was not evidence about their contents either way; `find -type d` was needed to see them at all. That part of the lesson holds.

**Corrected 2026-08-08 — those two were labelled "phantoms" on a bad test.** See R-57. `claude-plugins/…/mcp-server` is a **legitimate** sub-project directory (`root: session-bridge/mcp-server`); `eduplanner-site/…/optaplanner` is undetermined. The rule above survives because it rests on empty-directory invisibility, not on those two being defects; the illustration was wrong, the rule was not.

**Rule:** when a report's evidence is `git status` on one machine, (a) sweep the other members of whatever population the claim generalises over before accepting the framing, and (b) do not read a clean `git status` as absence — an empty directory is invisible to git, so census the filesystem, not the index.

**Promote-when:** a second instance where a single-host symptom was generalised without a sweep. Pairs with R-38 (a concurrent session had already measured it) — both are "widen the evidence base before theorising".

## R-110 — Miss: an identifier's shape says nothing about whether the thing exists — check its declared root

> **Id split 2026-08-16.** This entry reused a number already taken by a
> different lesson. `R-57` (index row, 2026-08-06) is *"when the seam is a TOOL,
> the scout is one real invocation whose output you read"*. This one is `R-110`.
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

## R-111 — Hit: before fixing a heuristic, grep for other copies of it — `references()` cannot see a duplicated closure

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

**Status:** open — verdict `miss → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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

**Status:** open — verdict `hit` (Index row). Not promoted; the skill mentions this entry only
as a **cross-reference inside another entry's bullet** — *"(R-87 is the same law's *hit*: the
scout …)"* — which is someone else's promotion citing this one as kin.

**Corrected 2026-08-20**, same error as [[R-4]] and [[R-8]]: a mention-counting predicate stood
in for "has its own promoted bullet". Three of the five entries that pass flagged `promoted`
were wrong; only [[R-41]] and [[R-42]] were real.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line and was invisible to every disposition query. Not back-cited in the served `SKILL.md`, so not promoted; datapoint count not re-assessed here.

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
counterfactual); `docs/issues/archive/2026-08-15-context-scope-all-crosses-umbrella-boundary.md`
(the defect); `docs/trackers/structural-debt-refactor.md` SD-3 → SD-10.

**Promote-when:** a second instance where an instrument-nominated group's axis
turns out orthogonal to the duplication, and the decisive site sits outside the
group. At two, promote step 2 into this skill's Phase 1 as a named seam class
(*instrument-nominated group*), since it is craft-shaped — it holds for any
ranking tool that scores symbols independently, not just this repo's.

## R-115 — Aggregate behaviour data is a screen, not a verdict

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

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the **served** `1.16.13` cache, not the repo
source: back-citation *"(R-89 in codescout's `docs/trackers/reconnaissance-patterns.md`…)"*
present, 1 occurrence. The back-citation was added *because* of this entry: its bullet was
reworded the same day it was promoted, and a quote-only anchor would have gone red on a
promotion that had worked.

**Status:** promoted-to-permanent-docs, then **re-promoted the same day** — the promoted text
was Outgrown before it was ever live.

- **Promoted** 2026-08-20 in `claude-plugins:23a11c3` as *"Build freshness and process
  freshness are two separate facts, and `mtime` answers neither."* Both freshness axes this
  entry required were named in it.
- **Audited and found Outgrown** (rubric row 2) hours later — by its own promotion. `23a11c3`
  did not bump `plugin.json`, and the plugin cache is keyed on that version, so all three
  profiles kept serving the pre-edit copy: committed, reviewed, in force nowhere. Neither a
  build nor a process was involved, which makes **distribution** a third axis of the same law.
- **Re-promoted** in `claude-plugins:a5df5bd`, shipped in `1.16.12`, verified at the served
  bytes in all three caches. The bullet now opens: *"Freshness is a property of the copy that
  SERVES you, and it breaks on three independent axes — build, process, and distribution.
  `mtime` answers none of them."* Per the skill's own audit rule — a recurrence of an
  already-promoted law is a defect in the promoted text, not a new entry — no fourth bullet
  was added.

Full account, including the four proxies that read green in the broken world:
`prompt-surface-compaction-session-log:F-9`. Note
this entry carried **no `Status:` line at all** until the sweep in
`prompt-surface-compaction-session-log:F-7`: the criterion fired 2026-08-16 and stayed
invisible to every field-presence query for four days, which is the failure mode, not a
footnote to it.



**Promote-when fired:** 2026-08-16 — fourth instance, raised from another
session (`28ea039a`, commit `5917e37e`: a committed claim about shipped guidance that
rested on a stale build). **Harvested 2026-08-20**; the disposition is the `**Status:**`
line at the top of this entry. Added by the 2026-08-16 disposition sweep — the entry was
already adjudicated in prose, which is exactly why a `^**Status:**` grep did not see it.

**Valid:** dated 2026-08-20

> **Relabelled 2026-08-20 from a second `**Status:**` line.** Measured across all live
> trackers, this was the **only** entry of 149 carrying two — and by then they disagreed:
> the top read `promoted-to-permanent-docs`, this one still read `FIRED`. Which one a
> field-presence scan reports depends only on whether it takes the first match or the last,
> and neither choice is wrong in a way the scan could detect. One disposition field per
> entry; a fired-criterion datapoint is evidence, so it keeps its own label.
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
(`archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`,
`5606ab6e35618aea` — archived 2026-08-19, id re-keyed by the move) into `8b27b1ea`, a commit about the read-side worktree
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

**Fourth instance — 2026-08-30, and it is the one that indicts the flag in favour of the
readback.** `git add docs/trackers/open-issue-work-queue.md && git commit -F -` — this
entry's own *second draft*, the form it labels "worse than useless" — swept 147 lines of a
peer's staged work (an ADR, `bug-fix-session-log:W-77`, and this file's `R-127`) into
`7e452f85`, a commit about BL-50. Same shape and same direction as instances 2 and 3: this
session as perpetrator, content intact, attribution wrong, commit message silent.

What is new is not the sweep. It is **why the guard missed**. This session was not ignoring
the rule — it ran a *content* diff, deliberately, citing `W-69`. It ran it on the wrong side
of the staging boundary: `git diff -- <path>` **before** `git add`, which structurally cannot
see what a peer stages one second later. So the failure was not "forgot the rule" but
"executed the wrong member of the rule's family", and no amount of sharpening the *flag*
would have prevented it.

That is the argument for this entry's step 3 over its step 2. A flag has to be recalled at
the moment of the commit; a `git status --short` readback between `add` and `commit` is one
habit that catches every variant, and here it would have printed four files where one was
intended. `git commit --only <paths>` would also have held — but only in the counterfactual,
and the entry has now recorded **four** instances of a correct rule not being reached for.

Discovery was the peer's, not this session's: their own `git commit` returned "nothing to
commit, working tree clean", which is this entry's step 3 read from the victim side.

**Status:** promote-when **FIRED** (4 instances, was 3) — **promotion not applied.** The target
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

**Status:** open — verdict `hit ×2` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. Two datapoints already recorded in the verdict. Not back-cited in the served `SKILL.md`; kin to [[R-49]], which *was* promoted — worth checking whether this is the same law.

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

**Status:** open — verdict `hit` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. It is the precedent the audit-on-promote convention cites (`claude-plugins:c889e83`, 1.16.4 → 1.16.5), so the *convention* landed even though this entry is not itself back-cited in the served `SKILL.md`.

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
recurrence"* over the chain `R-113 → R-77 → R-79 → R-87`. What the entries
actually self-label: R-77 says *"third instance"*, R-79 says *"fourth recurrence
of R-77"*, and the first two are folded inside R-113. That is **four**. And
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

**Kin:** R-3, R-113, R-77, R-79 (the chain), R-50 (the view is not the set), R-87
(the hit), R-89 (the unreachable case audited alongside).

## R-94 — A wiring inventory is not a delivery inventory, and it is wrong in both directions

**Status:** open — verdict `hit ×1, miss ×2 (self-caught)` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. Three datapoints already recorded in the verdict. Not back-cited in the served `SKILL.md`, so not promoted.

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

**Status:** promoted 2026-08-20 — verdict `hit ×5 → rule` (Index row). The criterion fired
the same day this entry was surfaced by the verify-open sweep, which had found it carrying no
`**Status:**` line at all.

**Promoted-to:** `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`
§ Phase 1 — Scout. D11-verified 2026-08-20 in the served `1.16.14` cache: back-cited as
*"(R-95 + R-92 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"*, 1 occurrence,
all three profiles byte-identical at 37817 bytes.

**Promote-when FIRED 2026-08-20 — second cluster.** The criterion asks for *"one more cluster
where a deferral rationale is falsified on contact"* — a **cluster**, not a datapoint count.
Worth stating plainly: the sweep that surfaced this entry first called it ready on the
strength of `hit ×5`, and that is **not what the criterion asks** — all five are one cluster.
Characterising readiness by a count without reading the criterion is the same shape as the
measurement rule this ledger keeps re-earning. The criterion fired on different evidence.

The second cluster is this project's own
`docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md`, closed
`wontfix` on four rationales, every one falsified within 24 hours:

| Filed rationale | Measured 2026-08-20 |
|---|---|
| "the precise version needs a **schema change**" | one markdown field (`**Promoted-to:**`) in `docs/templates/session-log.md`. Zero code. |
| "plus a retroactive **back-fill**" — cited as the cost justifying deferral | 13 entries in one pass, every destination still recoverable — and the back-fill **found** three defects ([[F-17]], [[F-44]], `release-promotion-session-log:F-1`) plus five unrecorded promotions. It was the payoff, not the cost. |
| "a candidate population of **101 entries** for the naive check, which is noise" | the 101 counted `Status: validated`; the precise population was **13** claims. The noise was a property of the predicate, not of the problem. |
| "the generalisation … was **the agent's**, not a response to repeated cost" | `tracker-hygiene-log:HY-11`, filed 2026-08-17 and marked *user-raised*, had already specified the same detector under the same name, `D11`. |

Nine rationales across two clusters, every one inflating in the direction that justified
stopping. That direction is this entry's structural claim, and it now holds in a second work
stream, a different domain, and a different author.

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
| the sentinel is "a semantic compromise" | It is a **live regression**: `Tool::call_content` (`src/tools/core/types.rs`) gated the session-opening guide on `emitted.is_empty()` at the time of this observation — any key landing in `emitted` suppressed it. (As of Phase C, 2026-08-18, the trigger is `!emitted.contains(SESSION_OPENING_GUIDE)`; a sentinel outside that literal topic string no longer collides — see `GuideLedger::notices`.) |
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

**Valid:** dated 2026-08-20

---

## R-96 — Widening a gate disarms the tests that used it as scaffolding, and they go green for a new reason

**Status:** open — verdict `miss (self-caught) → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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

**Kin:** R-70 → R-73 → R-76 (the D chain), R-3/R-113/R-77/R-79 (law C, whose false zeros ran
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

**Status:** open — verdict `miss (self-caught) → rule` (Index row), lifted 2026-08-20 by the verify-open sweep; this entry carried no `**Status:**` line. `→ rule` records that it fed a distilled law, not that it reached the skill: not back-cited in the served `SKILL.md`. Datapoint count not re-assessed here.

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

**Valid:** invariant

The claim — *a zero is a fact about your query, not about the world* — is a law. **One
caveat, and it is why this was nearly declared `conditional` instead:** the title's *"three
independent ways"* is an enumeration, and enumerations of a class grow. This one already
has — the served `SKILL.md` chains the law across `R-3` → `R-113` → `R-77` → `R-79` →
`R-104`, with a sixth at `R-4`. Read the three as illustrative, not exhaustive: the law does
not decay when a fourth arrives, only the count does. Declared 2026-09-01.

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

### 2026-08-21 — the criterion fired, and it fired outside this entry's own domain

Three more in one session, taking the count to **eight**. All three were instruments **I
built myself**, not reports someone else produced — which is the widening, because every
original instance was a query against a findings array whose keys, value domain and cap were
somebody else's to publish.

| Failure | Instance |
|---|---|
| **path guessed, not resolved** | Probed three plugin caches at `…/codescout-companion/skills/…`, got empty output from all three, and read it as *"the fix did not ship"*. The cache is version-keyed — `…/codescout-companion/<version>/skills/…` — so the path never existed, and `grep -c` on a missing file prints nothing at all. Note it did not even print `0`: **empty is not zero**, and I nearly reported it as one. |
| **the shell ate the predicate** | `grep -c "$s"` where `$s` held a backtick returned `0` for a string that is present exactly once. Read as *"the `source.md` edit is not in the binary"*. |
| **wrong sort algorithm — and NO zero involved** | Ranked processes by `ps lstart` through a lexical `sort`, which orders on **weekday name** (`Wed` > `Tue` > `Thu` > `Mon`). Confidently reported four two-day-old processes as the newest on a machine whose newest was 17 seconds old. |

**Why the third matters more than the other two.** Instances 1 and 2 are the promoted Phase 1
bullet's *scope* and *encoding* arms wearing new clothes, and in both a zero (or a blank) is
what eventually raised the question. Instance 3 produced **no zero and no absence** — a full,
plausible, correctly-formatted ranking that was entirely wrong. Neither this entry's wording
(*"a zero from a report"*) nor the search-zero law (*"a search that finds nothing"*) reaches
it, because nothing was missing.

**The widened rule.** The failure is not about zeros, and not about reports. It is about **who
supplied the predicate**. Whenever you hand-build an instrument — a path, a pattern, a sort
key, a field name, a filter — that component came from memory, and the instrument will answer
in its own terms without complaint. The remedy is not care and not re-reading: it is a
**positive control** — before believing the result, make the instrument find or rank one case
whose answer you already know. All three above were caught that way and by nothing else.

**Knowing this entry did not prevent any of the three.** They were committed in a session that
had read R-104 in full, quoted it back to the user, and cited it in an unrelated commit
message — the same self-referential shape the Measurement iron rule already records about
itself (*"two of the seven were committed while writing the entry that documents it"*). That
is evidence for placement over wording: a law you can recite is not a law you apply.

**Status:** promoted — criterion fired 2026-08-21 at eight instances (five 2026-08-17, three
2026-08-21) and was harvested the same day. Phase 1's search-zero bullet now carries the
widened form and the positive-control remedy.

**Promoted-to:** `codescout-companion/skills/reconnaissance/SKILL.md` § Phase 1 —
`claude-plugins:577e8e1`, patch-id `300d76f780d15cb6466ad0ea94a1257166f42d4b`, shipped in
plugin version **1.16.16** (`78d3284`).

Verified at the **served** copy, not the source: all three profile caches
(`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) resolve `1.16.16` and contain `positive
control`, byte-identical at md5 `5f1efb9258`. Negative control run in the same pass — the
`1.16.15` copy scores **0** for the same predicate, so the check distinguishes rather than
merely matching. That negative control is this entry's own widened rule applied to the
verification of this entry's own promotion.

**Promote-when:** **FIRED.** The original criterion was *"a sixth instance, OR the first time
a report carries its own key/value legend and the reader still guesses"*, and the honest
reading is that its prediction half **held**: self-describing output is retiring the
report-shaped failures — `link_scan`'s `counts.entry_edges` was legible on sight on
2026-08-21 with no legend needed. What it did not predict is that the failures would migrate
to hand-rolled shell, where there is no publisher to add a legend.

So the target is **not** `docs/PROGRESSIVE_DISCOVERABILITY.md`'s legend pattern as originally
guessed — a legend cannot fix a sort you wrote yourself. Route instead to the reconnaissance
skill's Phase 1, widening the existing *"a search that finds nothing is evidence about the
search"* bullet to cover instruments that return a **wrong** answer rather than **no** answer,
with the positive control as the remedy. Craft-shaped (true in any repo), so it goes via the
skill sync flow rather than a codescout memory; the cross-repo half is filed as
`claude-plugins:docs/issues/2026-08-21-zero-law-does-not-cover-wrong-answer-instruments.md`.

(Original criterion, retained for the record:) a sixth instance, OR the first time a *report*
carries its own key/value legend and the reader still guesses. The substrate is already moving that way — `f908e883`
added `severity_legend` to `audit_doc_refs` and `7c218338` unified `link_scan`'s field names
— so the honest test of this entry is whether self-describing output retires it. If it does,
promote as a note on `docs/PROGRESSIVE_DISCOVERABILITY.md`'s legend pattern rather than as a
discipline agents must remember.

## R-105 — Miss: a derived identity key verified against ONE lifecycle event, generalised to "stable"

**Verdict:** miss (human review) → rule · **Observed:** 2026-08-18, designing an agent-agnostic session key for the guide-hint ledger

**Seam:** the lifecycle of the process a derived key is derived FROM — enumerated for one event (MCP subprocess respawn), never for the others (client resume / continue / fork / reparent).

codescout's guide ledger keys per-conversation state so it survives the client respawning
the stdio MCP subprocess. `CLAUDE_CODE_SESSION_ID` is Claude-Code-specific, so I proposed
**parent PID + parent start-time** as the agent-agnostic fallback — and scouted it, at
length: read the live process tree, confirmed every MCP server's parent is a harness
process, confirmed the nested `codescout mux --socket …` children are LSP workers on a
different code path that never build a ledger, and paired the PID with `/proc/<ppid>/stat`
field 22 to defeat PID reuse. Every one of those checks is true. Every one is about the
same single event — the subprocess respawn that motivated the design.

The human partner named the case the scout never enumerated: **session resume**.
`claude --resume` / `--continue` restarts the *client* process, so the PPID changes while
the conversation continues. The falsifying evidence was already in hand and unread —
session `2c518eb6` spans 2026-08-06 → 2026-08-18 (12 days, 9630 calls, 67 MCP processes)
under a `claude` process started 2026-08-17 14:46, i.e. a 12-day conversation on a
1-day-old parent, and its ledger file is still keyed by the unchanged session uuid. A PPID
key would have dropped the ledger at every resume — the worst case to lose, because a
restored conversation still holds the guide bodies in context.

**The rule.** A key derived from runtime state is a claim about that state's *lifetime*.
Before proposing one, enumerate the lifecycle events of the thing it is derived from and
say what the key does at each — not only the event that motivated the design. For a
process-derived key that is at minimum: child respawn, parent restart, parent death and
reparent, fork, and reuse of the identifier. Thoroughly verifying the motivating event
verifies the motivating event, and a scout that deep reads as coverage.

**Promote-when:** a second instance of a derived key — identity, cache-invalidation, or
dedup — proposed after verifying a single lifecycle event. At 2 datapoints, promote to
SKILL.md Phase 1 as a seam class: *"deriving a key from runtime state → enumerate that
state's lifecycle events before proposing it, not just the one that motivated it."*

**Status:** open — 1 datapoint

**Kin:** R-91 (state what a measurement cannot see before attaching a conclusion to it), R-50 (the view is not the set)

## R-106 — A generated surface is ground truth about cost, and about nothing else

**Verdict:** hit (pre-edit) → rule

**Observed:** 2026-08-18, codescout prompt-surface audit. Recon invoked by the user after the baseline report was delivered, before any compaction edit.

**Seam:** the `workspace` parameter as it appears in the MCP `tools/list` payload, versus the code that puts it there.

I drove the release binary over stdio with a real `initialize` + `tools/list` handshake and ran a duplicate-detector across every advertised property description. It found the `workspace` param's 225-char description repeated **verbatim on 24 of 27 tools** — 5,400 chars, 5,175 redundant, 8.8% of all schema bytes. I ranked that as the first compaction target and described it to the user as *"mechanical, no eval needed, no judgment involved"* and *"a free 5.2 KB with zero eval risk"*.

The scout read the generator. `src/server.rs:1024-1026` calls `CodeScoutServer::inject_workspace_param` for every tool whose `pinnable()` returns true; that function (`src/server.rs:496-508`) inserts **one** hard-coded `json!` block, idempotently, at `list_tools` time. The source was already DRY. The duplication is a property of the MCP wire format, where each tool carries its own complete schema.

The measurement was right and the inference was wrong, in a specific way worth naming: *duplicated on the wire* and *authored 24 times* are different claims, and only the second licenses "dedupe it". Every byte I counted is genuinely paid on every request — the cost finding survives intact. What did not survive is the remedy, its risk profile, and its priority. The real lever turned out to be `t.pinnable()`: auditing which tools need a workspace pin at all removes whole 259-byte blocks instead of trimming one shared sentence, and that lever is **invisible from the wire dump** because a tool that is not pinnable simply has no such property to observe.

The same pass, run the other way round, produced the mirror-image finding and is the reason this entry is a rule rather than an anecdote. `append_entry`'s response hint instructs the caller to pass `anchor_heading`; the advertised schema declares no such property. Reading only the generated surface concludes the hint is advertising a phantom. Reading only the source concludes the feature ships fine. Reading both states the actual defect — implemented at `append_entry.rs:34`, undeclared on the only surface an agent sees (`prompt-surface-compaction-session-log:F-1`, severity high). **Neither surface alone was sufficient, in either direction.**

**Promote-when:** a second session makes a generated-surface-vs-generator inference error, or is saved from one. At two datapoints, fold into `## The seven laws` as a named clause under **law A** — *ground truth is the artifact, and a generated artifact is ground truth about cost only; authorship, and therefore the remedy, lives in the generator.* Note the pull toward law B ("the instrument decides the answer") is a near-miss: the instrument here was correct and well-chosen. The error was in what the reading licensed, not in the reading.

**Status:** open — single datapoint, caught pre-edit, no wrong edit shipped.

**Kin:** R-91, R-100 (self-caught refutation before filing), and law A.

---

## R-107 — HIT — a spec's account of an existing table is a claim, and the constraint layer is where it is least likely to have been read

**Verdict:** hit.

**When:** 2026-08-21, before modifying `link_scan::call` (220 lines) to materialize
entry-grain citations into `entry_cite`, per a spec that described the table's
prune-and-re-materialize behaviour as something already supported.

**What the scout read that the spec did not:** the live `sqlite_master` DDL and the whole
of `entry_cite.rs`. Two gaps, both invisible from the spec and neither capable of raising
an error:

1. **No delete path existed.** The module had `insert_with`, `outgoing`, `incoming`,
   `incoming_like` and a private `collect`. "Pruned and re-materialized per scan" was
   machinery to build, not to call.
2. **`origin` is not in the primary key** — `PRIMARY KEY (src_slug, src_local, dst_ref,
   rel)`, with `origin` merely a column. Paired with `insert_with`'s `INSERT OR IGNORE`,
   an edge the scan derives that a human already wrote is silently dropped and keeps
   `origin='write'`. Correct precedence, but it makes *edges derived* and *rows written*
   different numbers that look identical from the call site.

**Why the spec missed it:** it was written against the table's **shape** (the `origin`
column exists, reserved for exactly this) without reading the key beside it. The
`INSERT OR IGNORE` semantics live in a doc comment on the function; the PK lives in the
schema. Neither half reveals the interaction, and the spec's author had read one.

**Downstream confirmation:** three mutations on the shipped code — delete the origin
filter, delete the `src_slug` scope, hardcode `insert_with` to `Ok(1)` — were all CAUGHT
by tests that only exist because the scout found the gaps. The third is defect 2 written
out literally.

**The generalisable form.** Reconnaissance already treats a *proposed fix* and a
*prohibition* as claims about current state. A **spec's description of existing
machinery** is the same kind of claim, and it has a characteristic blind spot: authors
read the table's columns and the function's signature, both of which are visible from a
symbol listing, and skip the PK, the unique indexes, the FK actions and the conflict
clause — which are visible only in the DDL or buried in a doc comment. Read the DDL.

**Proposal:** fold into Phase 1's seam-class list, next to the schema-migration-ordering
bullet, as: *a spec that says "the existing table already supports X" is a claim about the
constraint layer — read the DDL (`sqlite_master`, the migration, the `CREATE`), not just
the struct and the function signatures. Conflict clauses (`INSERT OR IGNORE`), PK column
sets, and FK actions decide behaviour that neither the row type nor the call site shows.*

**Evidence:** `statement-validity-session-log:F-5` (the scout), `W-6` (the counterfactual),
commit `7468902b` (the code the scout produced).

**Status:** open — proposal not yet synced to SKILL.md.

**Valid:** dated 2026-08-21

**Rests on:** the reconnaissance skill's own principle that a claim about configuration
reads as a fact rather than an assertion, and so slips past the reflex that would question
a causal claim.

## R-108 — a deferral justified by an AGGREGATE is silent about the tail — and the tail is the population the deferred mechanism governs

**Valid:** invariant

**Verdict:** miss — caught on the first live call after shipping, not by the measurement that
authorised the deferral.

**Observed:** 2026-08-21, codescout Layer 4 (`librarian(action="context")` entry-grain
anchor). Neighbour **ordering** was deferred with an explicit, honest, *measured*
justification, recorded in `context-performance:CTX-1`:

> Neighbours sort by `(direction, reference)`, which is arbitrary. It mattered when only 2 of
> 25 were served; **at 98% served it decides almost nothing.**

The 98% was real — swept over 931 anchors. The deferral still failed immediately.

**Pattern that failed:** ordering only *does* anything for the anchors whose neighbourhood
**overflows the budget**. Those are, by definition, the population the mechanism governs — and
they are exactly the ones the 98% figure excludes. The aggregate was computed over the
anchors where the deferred mechanism is inert, then used to license deferring it for the
anchors where it is not.

It cost immediately and specifically. Alphabetical `direction` sorts
`cited-by` < `cites` < **`mutual`**, so the class the packer had just been taught to label as
the strongest tie sorted **last**. On the first live call, [[R-3]] served eleven one-way
citations and dropped **both** its mutual partners — [[R-77]] and [[R-79]], which are its own
documented chain.

**Pattern proposal:** when a deferral rests on an aggregate, **name the sub-population the
deferred mechanism actually acts on, and check the aggregate was computed over THAT.** The
tell is a rationale of the form *"X decides almost nothing, because metric M is high"* where
M is high precisely when X is dormant. Two cheap checks:

- **Invert the statistic.** "98% fit" means "2% overflow" — is the deferred thing about the
  98 or the 2? If the 2, the number argues the opposite of what it was quoted for.
- **Run the mechanism once on a member of the governed sub-population** before accepting the
  deferral. One live call would have shown zero `mutual` labels in the pack.

**Cost absorbed:** one commit (`f58ab393`), no user-visible damage — the defect was found by
verifying live rather than by trusting the local test suite, which was green throughout.

**Kin:** [[R-95]] — *a deferral rationale is a claim, and the least-audited kind*. This is not
a recurrence of it but a **distinct failure mode**, and worth separating: R-95's instances are
rationales that were never costed (a site count nobody re-ran, a binary nobody looked past).
Here the rationale **was** costed, with a real sweep, and the measurement was sound — its
*denominator* was wrong. A reviewer applying R-95 would ask "did anyone check this number?",
get a truthful yes, and stop.

**Promote-when:** a second instance where a measured aggregate licenses deferring a mechanism
that only acts outside that aggregate's population. At two, this belongs in the reconnaissance
SKILL.md next to R-95's bullet, phrased as the inversion check.

**Status:** open — 1/1 datapoint. Recorded now because the mechanism is precise and the
cheap check ("invert the statistic") is one sentence.

## R-116 — MISS — the substrate law was quoted aloud and then violated for a whole session; a positive control made it feel checked

**Verdict:** miss.

**What recon should have caught.** Phase 1 already carries the law verbatim: *"A
tool that resolves its target from the environment has a SUBSTRATE as well as a
verdict… A retired-but-still-present datastore keeps answering, so the failure is a
confident wrong number, never an exception."* I read it, quoted it to the user in
this session, and then spent the session querying
`.codescout/embeddings/codescout.db` as if it were this project's live index. It is
not: `CODESCOUT_VECTOR_BACKEND` is unset, `VectorBackend::resolve`
(`src/retrieval/code_store.rs:247-262`) defaults to Qdrant under `server-stack`,
and `CODESCOUT_QDRANT_URL` is set and answering. The file's mtime predated the
session.

**What caught it instead.** A downstream gate, and only because I had just built
it: `index(action="verify")`'s first live run returned 1611 files / 47 647 chunks
against the sqlite file's 1593 / 46 979. Two instruments, two numbers, and the
skill's own instruction — *"the question is not whose logic is wrong but which
world each one read"* — is what turned the discrepancy into the diagnosis in one
step. Without that tool existing, nothing in the session would have disagreed with
me. See `bug-fix-session-log:F-66`.

**Why the existing text did not fire — the useful part.** Three compounding
reasons, and only the third is new:

1. I took the backend from the **reporter's** environment block (their bug text
   said `CODESCOUT_VECTOR_BACKEND=sqlite-vec`) rather than from this host. The law
   says read the substrate; it does not say *whose* substrate, and a bug report
   supplies a plausible one for free.
2. A 247 MB file with exactly the right name, schema and internal consistency is a
   powerful false confirmation. Every query succeeded. Nothing was empty, nothing
   errored, no zero appeared — so rules 3 and 4 of `docs/PROBES.md` (a zero lies /
   run a positive control) had no hook to catch on.
3. **I ran a positive control and it passed.** A deliberately-broken join key
   returned all 46 979 rows, proving the predicate discriminated. It did — against
   the wrong database. This is the gap: rule 4 tells you to validate the
   *instrument*, and a passing control feels like it has validated the *answer*.
   It has not. The two are orthogonal, and passing one while failing the other is
   the most confident possible wrong state.

**Proposal.** Phase 1's substrate bullet ends by naming failure shapes ("an ORM
pointed at a stale replica, a linter reading a cached AST, a test suite importing
an installed wheel"). Add the interaction with rule 4, because they currently read
as independent safeguards and this session shows one masking the other:

> A positive control validates the INSTRUMENT, never the SUBSTRATE. Passing one
> against the wrong datastore is the most confident wrong state available — the
> predicate provably discriminates, so the number looks earned. Establish which
> world you are reading *before* proving your query works in it, and prefer the
> tool that resolves the backend over a file you can name.

**Confirming data points:** 1 (this session). The substrate law itself is at many;
what is new is the positive-control interaction.

**Promote-when:** a second instance of a validated instrument reporting confidently
on the wrong substrate. At 2, fold the paragraph above into the skill's Phase 1
substrate bullet via the Sync flow. Below that threshold it stays here, because one
datapoint is exactly the evidence the skill's own *Every promotion audits the
promoted set* section says is not enough to widen a promoted law.

**Status:** open — proposal drafted, threshold not met.

**Valid:** invariant

The positive-control/substrate orthogonality is a law. The specific backend
resolution on this host is `dated 2026-08-26` and changes the moment
`CODESCOUT_VECTOR_BACKEND` is set.

**Rests on:** `VectorBackend::resolve`'s `server-stack` default plus an unset env
var — together they make a present, plausible, schema-correct sqlite file the
*wrong* substrate rather than merely a stale one.

## R-117 — A fix that names a POPULATION asserts it is non-empty — and that assertion is the one that fails green

**Observed:** 2026-08-27, implementing the `doctor` outside-roots scoping fix from
a bug file written earlier the same session. Recon ran before the first edit.

**Scout (reality):** the bug file's `## Fix` prescribed *"drop rows whose
`abs_path` resolves under a **different** managed root"*. Reading
`managed_roots` (`src/librarian/tools/mod.rs:215`) and
`check_outside_managed_roots` showed the case is the **empty set**:
`managed_roots` returns the active project's `git_root`/`abs_path` plus the
legacy `workspace.roots` and never another repo, and the check fires only when
`containing_root` matches *nothing*. Every one of the 401 firing rows is under no
managed root; there was no "different root" bucket to move anything into.

**Counterfactual (hit value):** the prescribed fix would have compiled, shipped,
and **passed its own test** — a test written from the same wrong model would have
constructed two managed roots, a configuration that never occurs — while changing
the report by **zero rows**. The bug file would then read `fixed`, which is
precisely the artifact that stops anyone looking again. The working fix needed a
discriminator that had to be *measured*, not reasoned: partitioning the 402 firing
rows against umbrella members and `commits.git_root` gave 359 / 33 / **10**, and
the 10 are the whole actionable population.

**The generalisable form.** Reconnaissance already treats a *proposed fix* and a
*prohibition* as claims about current state. A fix that names a **POPULATION** is
the same claim in a third grammar: *"drop the rows that are X"*, *"dedup the 24
duplicate descriptions"*, *"hook the calls above 32 KB"* each assert that a set
exists to act on. The count is one query.

**Why this form is the dangerous one: it fails GREEN.** The other two forms fail
when you look — *"X is already set"* is a visible finding. An empty population
produces a fix that compiles, a test that passes, a diff that reviews cleanly, and
a report that is unchanged. Nothing anywhere raises.

**Prior datapoints — three instances, three systems, none of which raised:**

1. **This one.** `managed_roots` never returns a foreign repo → the named case was
   empty. `bug-fix-session-log:F-74`.
2. **[[R-106]]** — a "free mechanical dedup, zero risk" of one 225-char
   description repeated on 24 tools. The source holds exactly **one** copy;
   `inject_workspace_param` injects it at list time. The 24 duplicates were a
   rendering, not a population.
3. **The context-utilisation programme's sole build decision** — a PostToolUse
   hook on `mcp__.*` at 32 KB, justified by "137 calls carrying 61.1% of
   information-bearing tokens". After excluding base64 browser payloads, **zero**
   non-browser MCP calls in the entire corpus reach 32 KB. *"Trigger population
   empty, not merely smaller."*

**Audit of the promoted set** (required of every promotion; verdicts recorded so
the next one inherits the check rather than repeating it):

| promoted law | mode checked | verdict |
|---|---|---|
| *A proposed fix — and equally a prohibition — is a claim about CURRENT STATE* | **Outgrown** | **Confirmed outgrown.** Its examples cover only the "X is not already done" assertion; the population form recurred under it unnamed. This promotion IS the remedy — widen, do not add a sibling bullet. |
| *A search that finds nothing is evidence about the search* | False / Obsolete | **Holds, do not cut.** Fired twice today, once inside this very scout: `grep "Trigger population empty, not merely smaller"` returned 0 because the phrase spans a line break. Partially gated now — codescout's `grep` prints *"this zero describes what was searched"* on every empty result — but the gate covers a grep's zero, not a **report's**, which is the costlier half. |
| *Freshness is a property of the copy that SERVES you* ([[R-89]]) | Unreachable | **Holds and was reached.** Governed both live-verifies this session; each fix was reported as "committed and tested, not live" until `cargo rb` + `/mcp` + one probe. No change needed. |

**Proposal:** widen the existing Phase 1 bullet rather than add a fourth — a
recurrence of a promoted law is a defect in the promoted text, and this ledger's
own R-3 → R-113 → R-77 → R-79 chain is the cost of filing recurrences instead of
re-promoting. Add the population form with its fails-green tell, and update the
provenance tail.

**Status:** promoted — SKILL.md bullet widened this session.

**Valid:** dated 2026-08-27

The three bucket counts are facts about this machine's catalog on this date; the
mechanism (a named population may be empty, and that failure is green) is
invariant.

**Rests on:** `src/librarian/tools/mod.rs:215` and
`check_outside_managed_roots`'s `containing_root(...).is_some()` early return,
both read this session; [[R-106]] and the context-utilisation entry for the two
prior datapoints; `bug-fix-session-log:F-74` for the full narrative.

## R-118 — Hit: scouting a fix primed me to find its bug where it wasn't — and a BLOCKED ground-truth check left the belief unmarked

**Verdict:** hit — recon's own controls caught it before anything was written down.

**Law:** C — *A search that finds nothing is evidence about the search.* This entry is
that law **over**-applied, which is the first of its kind in the ledger's 16 C-entries.
Every other one is law C under-applied: a zero read as fact when it was an artefact of the
search. This is the inverse, and it has a distinct trigger.

**Context:** 2026-08-27, scouting a rebuilt binary after `444d756c`
(*"fix(grep): a zero from a glob that opened no file says so"*) — a fix for a grep **false
negative**. First probe: `grep(pattern="pub fn", glob="src/tools/grep.rs")` → `0 matches`,
and *without* the new glob warning. I read that as the fix having a gap on the exact case
its own bug file is named after, and had begun framing it as a finding.

**What was actually true:** `src/tools/grep.rs` has 62 function declarations and **zero**
`pub fn` or `pub(crate) fn` — they are all module-private. The zero was a faithful report
about the pattern. The tool was correct in all four probes, and correct to withhold the
glob clause, because the glob *had* opened the file.

**The trigger, and why it is worth its own entry: scouting a fix primes you to find the
bug it fixed.** I was inspecting a freshly-patched false-negative surface, so a zero
*looked* like a false negative. Law C normally guards against trusting a zero; here the
expectation supplied the law's conclusion before the evidence did. The tool had said the
right thing — only the hidden-paths caveat, no glob clause — and I supplied the stronger
reading myself.

**The mechanism that let it get that far:** my ground-truth check was
`run_command("grep -c 'pub fn' src/tools/grep.rs")` and it was **refused by IL-3** (shell
content-readers on in-project source). I proceeded on the belief the blocked call was
meant to test. A refused verification returns an *error*, not a fact — but it leaves the
prior belief exactly where it was, with nothing marking it unverified. A failed check and
an unrun check are indistinguishable one turn later.

**What caught it:** running controls instead of concluding. `pattern="e"` on the same glob
returned 1731 matches in that same file, which proved the glob opened it and collapsed the
false-negative reading in one call.

**Proposal (for SKILL.md, at a second datapoint):** when a probe's ground-truth check is
*refused* rather than answered, treat the probe as un-grounded and re-establish ground
truth through a permitted tool before reading the result. Concretely: a blocked
`run_command` is not a null result, and the cheapest recovery is usually the same question
asked through the codescout tool the guard is pointing at.

**Confirming data points:**

1. This entry.
2. Related but distinct: `prompt-surface-measurement-session-log:F-41` (a filtered query
   reported as a finding, refuted by its own unfiltered control) — same remedy, a control,
   but that one was a missing control rather than a *blocked* one.

**Promote-when:** a second occurrence where a refused or errored verification is followed
by a claim that depended on it. At two, promote the proposal above into the skill's
Phase 1.

**Status:** validated — single datapoint, caught in-flight, nothing published.

**Valid:** invariant

**Rests on:** law C as stated in this ledger, and CLAUDE.md *Conclude Last* — specifically
its warning that a claim which "sounds right" is when verification matters most.

## R-119 — Miss: two enumerations of my own loaded context disagreed in-session (18 vs 23 memories) and I never compared them

**Verdict:** miss — a rebuild caught it, not recon. Recon had the evidence and did not look.

**Law:** A — *Ground truth is the artifact. Everything else is a claim about it.* Applied
one level up than usual: not to a symbol, but to the **enumeration of what I had loaded**.

**Context:** 2026-08-27. `codescout:020ea69a` (*"a sub-project's memories are the union of
both its stores"*) fixed `memory(list)`. Before it: 18 topics. After: 23. The five recovered
are exactly the namespaced ones — `infra/*`, `research/*`. So for the whole session, Phase 0
of the activation bootstrap ("load what the project already knows") ran against a surface
silently returning 78% of what it had.

**The evidence was in my own context and I never compared it.** The SessionStart banner
listed 18. `workspace(activate)`, called minutes later in the same session, listed 23. Two
enumerations of one thing, disagreeing, both sitting in the transcript. Nothing flags a
disagreement between two surfaces; only reading them against each other does, and reading a
banner as scenery rather than as data is why I did not.

**What it cost, and what it did not.** One recovered memory,
`infra/friction-measurement`, carries an instrument table with a row on `cc.py`'s profile
discovery — the same `~/.claude`-hardcoded-vs-`CLAUDE_CONFIG_DIR` defect I filed from
scratch this morning as `prompt-engineering:1758a24c9f1c648f`. Having it would have let me
file that as a **second instance of a known class with a proven one-line fix** instead of a
novel finding, and would have argued for a sweep.

But the row said **BROKEN (filed)** and the code was already **fixed** — verified at
`cc.py:23` before writing any of this. So "I would have known" is only half true: I would
have had the class (valuable, and now cross-referenced into the issue) and been wrong about
the instance. A stale memory recovered is not a fact recovered. Both the memory row and the
issue have been corrected.

**The pattern this is the third of, today.** A surface reporting a partial result in a shape
indistinguishable from a complete one:

1. `append_entry`'s anchor — `artifact(get)` returns 21 of 85 headings, front-anchored, and
   the anchor is conventionally last (`codescout:3e8826ccef87c8dd`).
2. `grep`'s zero under `include_hidden=true` — gitignored paths excluded, no clause says so
   (`codescout:0f7105b8bebc600b`).
3. This — `memory(list)` at 18 of 23, no count, no truncation flag.

None of the three lies. All three answer a narrower question than the one asked and present
it as the answer to the broader one.

**Proposal (for SKILL.md, at a second datapoint):** in Phase 0, when two surfaces enumerate
the same thing, read them against each other before trusting either. Concretely for this
harness: the SessionStart memory banner and `workspace(activate)`'s `memories` array are
independent enumerations — a mismatch is a finding, and it is free to check.

**Confirming data points:**

1. This entry.
2. Pending: the next session where two context-enumerating surfaces disagree.

**Promote-when:** a second occurrence of two enumerations disagreeing unnoticed. At two,
promote the proposal into Phase 0.

**Status:** validated — single datapoint; the discrepancy is reconstructable from this
session's own transcript, which is what makes it a miss rather than an anecdote.

**Mechanism found 2026-08-27, after this entry was written, and it is not where the entry
guessed.** The gap is not `codescout:020ea69a` (which fixed `memory(list)`, 18 → 23, and
merely made the divergence visible) and not `codescout:76e2d6cd` (which fixed a *fourth*
server-side reader whose block contradicted activate's array **inside one message**). The
banner is a **companion-plugin** surface with its own enumeration:
`detect.mjs:166-175` and `detect.py:175-179` both read `.codescout/memories/`
non-recursively and then discard directories with an `isFile` / `is_file()` guard, so
`infra/` and `research/` are skipped whole. Filed as
`claude-plugins:docs/issues/archive/2026-08-27-cs-memory-names-skips-namespaced-memories.md`
(fixed same day, `claude-plugins:b305b1b5` / patch-id `8ad092d3`).

That sharpens this entry rather than retiring it. The same `CS_MEMORY_NAMES` feeds
`subagent-guidance.mjs`, which injects it into a subagent's Phase 0 **in place of** the
`memory(action="list")` call — so a subagent is handed the short list instead of the call
that would return the full one, and cannot reach a namespaced memory by any route. The
proposal below is the remedy that works today; the plugin fix is the one that removes the
need for it.

**Valid:** invariant

**Rests on:** the activation bootstrap's Phase 0, and law A as stated in this ledger.

## R-120 — A decision procedure is a seam — I read the routing far enough to get an answer, not far enough to get its preconditions

**Valid:** dated 2026-08-27

**Verdict:** hit — a scout of a *procedural* seam caught three skipped gates before any
write landed · **Observed:** 2026-08-27, user-invoked recon immediately after I had
recommended a promotion route for the importance×cost explore-vs-ask rule.

**Seam:** the promotion procedure itself — `SKILL.md` § *Promotion routing* and the
section that follows it, plus the target ledger's own required-field contract. Every seam
in this ledger up to now has been code, a tool response, or an artifact. This one was a
**decision procedure**, and it has the same property that makes a seam a seam: my next
action depended on its current shape, and I had read it only far enough to select an
option.

Three divergences, none of which would have failed loudly. (1) I planned to write the
`R-N` entry in the *session-log* vocabulary — `**Rests on:**`, `**Status:** validated` —
where this ledger's augmentation prompt pins **Verdict, Observed, Seam, narrative,
Promote-when, Status, Kin** and a `Verdict` of `hit | miss | proposal`. (2) I omitted §
*Every promotion audits the promoted set* entirely: promoting a law obliges re-verifying
the already-promoted ones against False / Outgrown / Unreachable / Obsolete **and
recording the verdict**, which is a third step my two-step plan had no slot for. (3) I
named destination 1 (`SKILL.md`) when the rule's failure class is **Unreachable** — it
lives behind a deliberate memory read while governing a decision made in a task's first
thirty seconds — and class 3's stated remedy is *"placement, not rewording … the fix is
the session-opening surface"*. That is destination 2, gated on a measured base arm that
does not exist; `prompt-engineering:scenarios/exploration-protocol` is the nearest
candidate and measures Phase-0 ledger consult, a different rule.

The generalisable part is the failure mode, not the three findings: **reading a decision
procedure far enough to get an answer feels identical to reading it far enough to get its
preconditions.** A routing test that yields a destination is satisfying in a way that
stops the read — the question was answered — and both of the things I missed sat *after*
the answer: two paragraphs of gating on the adjacent option, and an entire successor
section imposing an obligation. Neither would have raised. A malformed `R-N` entry is
invisible to every field-presence sweep, which is the same defect this ledger's own prompt
records as having left 39 of 57 entries unharvestable for three months; a `SKILL.md` PR
with no audit merges exactly as cleanly as one with.

**Counterfactual:** without the scout, two artifacts land wrong. The `R-N` entry ships
missing 3 of 7 required fields into the ledger whose prompt names that as its historical
defect, and the promoted set goes un-audited on the one occasion the skill designates for
auditing it — an omission with no later trigger, since the next promotion would inherit
the same gap and the section exists precisely to stop that compounding.

**Promote-when:** A second scout of a procedural seam — a documented decision procedure,
routing table, or checklist rather than code — catches a precondition or successor
obligation skipped because the procedure had already yielded an answer. At 2 datapoints,
add a Phase 1 bullet: *a decision procedure is a seam; read its preconditions and the
section after it, not just far enough to select an option.*

**Status:** open — 1 datapoint. Evidence in
`prompt-surface-measurement-session-log:F-43`.

**Kin:** R-49 (re-entering your own plan is a seam; authorship is no exemption — here the
plan was one turn old), R-95 (a rationale reads as a settled decision rather than a claim;
a routing verdict does the same), R-89 (the served copy is the one that counts — checking
all 19 cached `codescout-companion` versions is what established the rule had no reach
outside one memory file).

## R-121 — A symmetric fix inherits the asymmetry it was copied across

**Valid:** dated 2026-08-27

**Law:** C — a search that finds nothing is evidence about the search. Secondarily A.

**Verdict:** hit — recon changed the fix before a line was written.

**Observed:** 2026-08-27, resuming `docs/issues/archive/2026-08-27-grep-zero-is-silent-for-gitignored-paths-and-include-hidden-does-not-reach-them.md` (`0f7105b8bebc600b`, archived on fix) to implement its own Fix candidate (1).

**Expected (the plan, which I wrote myself earlier the same day):** *"Add a gitignore clause to the zero-match warning — 'N path(s) under the search root were skipped as gitignored', ideally naming a few, as the hidden clause does. Fixes the reported defect and is symmetric with what already exists."*

**Got (scouted reality):** the symmetry does not hold, because the two filters have different shapes.

- `WalkAudit::hidden_at_root` (`src/tools/grep.rs:1146-1177`) is root-only **by explicit design** — its doc comment says *"Only the search root is inspected — one `read_dir`, no recursion — so the warning wording claims nothing about deeper hidden directories."* That is honest for dotfiles because the clause's remedy (`include_hidden=true`) is itself root-agnostic.
- Gitignore has no honest root approximation. `git check-ignore` over all 40 entries at this repo's root returns exactly six: `.env`, `.fastembed_cache`, `models`, `target`, `temp-docs`, `.worktrees`.
- **`.superpowers/` is not among them.** It is hidden-only. The exclusion that produces the reported zero is `.superpowers/sdd/.gitignore:1:*` — **depth 2**, a nested `.gitignore`, invisible to any root-level scan.

So the "symmetric" fix would emit a warning naming six innocent paths and still miss the guilty one. `completeness_warning`'s own doc comment already condemns exactly this: *"naming an unchecked cause ends the search for the real one."* The plan proposed re-introducing the failure mode the function was written to avoid.

**Counterfactual:** the natural regression test for candidate (1) plants a match inside a **root-level** gitignored directory — because that is what the fix scans. It would have passed. The bug would have been closed, the warning would have grown a clause, and the exact repro in the bug file would still reproduce. The defect ships behind a green test, which is the expensive shape.

**Second-order finding — the repro self-contaminated.** The bug file quotes its own probe string, so the pattern it prescribes now returns three hits in the bug file itself and demonstrates nothing. A fresh token had to be derived from the gitignored ledger first, validated by two commands: `git grep -l <token> | wc -l` returning **0** tracked files, against `grep -c <token> <path-in-the-excluded-dir>` returning **1**. **A probe string written into a searchable file stops being a probe.** Record that derivation, never the token — naming one here spends it, which is exactly what the first draft of this entry did, one paragraph after diagnosing the mechanism. See `R-122`.

**Proposal:** when a fix is justified as *"symmetric with the existing X"*, scout what makes X's scope honest before copying its shape. `hidden_at_root`'s root-only scan is honest because its remedy is root-agnostic; copying the scan without the property that licensed it produces a clause that claims more than it can prove. Symmetry is a claim about two mechanisms, and it is checkable.

**Status:** validated — drift caught pre-implementation; the bug file's Fix section was corrected in the same pass.

## R-122 — Writing the lesson down was the act that broke the instrument

**Valid:** invariant

**Law:** B — the instrument decides the answer.

**Verdict:** miss — the lesson was stated and violated in the same sentence, and only a live re-run caught it.

**Observed:** 2026-08-27, recon pass on a rebuilt binary, verifying the `grep` gitignore clause (`ee7d9a3a`) live.

**Expected:** re-run the probe `R-121` recorded, confirm the widened search now carries the clause.

**Got:** the probe returns a match — in `docs/trackers/reconnaissance-patterns.md`, inside `R-121`, in the sentence reading *"A probe string written into a searchable bug file stops being a probe… record how it was verified unique rather than the token itself."* The entry named its replacement token inline while prescribing the opposite. One hour between writing the rule and breaking it; the rule was never wrong.

**Why it is structural, not a slip.** A probe for an **exclusion** mechanism has one requirement: it exists only inside the excluded region. Any ledger that records it sits in the *searched* region by construction — that is what makes it a ledger. So for every search-shaped finding, the sentence "here is the token that demonstrates this" is self-defeating: **the write is the contamination**, and it fires regardless of care, because care is not the failing input. `R-121` was written by an agent who had just diagnosed this exact mechanism in the bug file one paragraph above.

**Counterfactual:** the next session verifying this fix runs the recorded token, gets a hit, and concludes either that the fix regressed or that the bug never existed. The second is the dangerous reading — it invites a revert of a correct fix, and the "evidence" for it is a search that looks clean.

**Proposal:** for any finding whose evidence is *a search returning nothing*, record the **derivation** — the command that mints and validates a fresh token — never the token. The general form is wider than probes: any recorded artifact that participates in the measurement it documents will perturb that measurement on re-run. Ask of every recorded value, "would a future reader's use of this change what it measures?" If yes, record the recipe.

**Repair applied:** `R-121`'s paragraph now carries the two-command derivation (`git grep -l` returning zero tracked files against `grep -c` returning one in the excluded path) and names no token, which restores the probe it had spent.

**Status:** validated — caught live, repaired in the same pass.

## R-123 — Adjacency is not causation — the nearest recent commit is a suspect, not a cause

**Valid:** invariant

**Law:** A — ground truth is the artifact.

**Verdict:** miss, twice in one session, same shape both times.

**Observed:** 2026-08-27. Two independent investigations, hours apart, both attributed an observation to the most recent commit near it. Both were wrong, and in both cases one `git log -S` or `git log -- <path>` would have settled it before any writing began.

| # | observation | attributed to | actual cause |
|---|---|---|---|
| 1 | `Monitor` executed in an arm that denied it | `--disallowedTools` being "enforced per name" | `Monitor` was **not on the list** when that round ran; added 16 min later in `e67d419`. The evidence pool straddled the fix. |
| 2 | `artifact(get)` now reports `headings_truncated` | the peer's `25fe3fb5`, committed 12 min earlier and touching `artifact(get)` headings | the field shipped in `3bccb234` on **2026-07-10**, six weeks earlier |

**Why the pull is strong.** In both cases the recent commit was *genuinely adjacent* — same file, same surface, same day, plausible summary line. Adjacency is what makes the inference feel unnecessary to check: the commit is right there, it touches the thing, and no step in between raises. Case 1 cost two bug reports, one of them filed at `high` against the wrong layer. Case 2 cost nothing only because it was checked.

**The asymmetry that makes this cheap to fix.** Confirming a cause costs one command — `git log -S'<the exact token>'` for a field or string, `git log -- <path>` for a file, and read the *dates*, not just the order. Being wrong costs a retraction plus everything built on top. There is no version of this where the check is not worth running.

**Distinguishing it from `R-118`.** `R-118` is *a refused verification returns an error, not a fact* — a check that was attempted and did not run. This is a check that was never attempted, because the answer looked already known. The failure modes are opposite in origin and identical in output: a confident claim with nothing behind it.

**Proposal:** before writing any sentence of the form "X changed because of Y", where Y is a commit, run `git log -S` on the exact token and read the date. If the token predates the change you are explaining, the explanation is wrong regardless of how well it fits.

**Status:** validated — two datapoints in one session, both with concrete cost.

**Promote-when:** a third instance, or one where a wrong attribution reaches a shipped surface rather than a bug file. At that point it belongs in the seven laws under A, phrased as *"adjacency is not causation — date the token before you credit the commit."*

## R-124 — Same law hit and missed in one session — a doc-asserted prohibition taken at face value

**Verdict:** miss — with a paired hit of the same law hours earlier, which is the
point of the entry.

**Observed:** 2026-08-27, session that shipped `6058dad6` (hide `run_command`
when `shell_command_mode = "disabled"`).

**Seam:** whether a capability named as unavailable in a doc is actually
unavailable at runtime.

**The pair.** One law — Phase 1's *"A proposed fix — and equally a prohibition —
is a claim about CURRENT STATE"* — fired correctly in one direction and was never
fetched in the other, in one session:

- **Hit** (`shell-gating-session-log:W-1`): the request "lets add a configuration
  where we can disable run_command entirely" asserted the feature was absent. The
  scout ran, found `shell_command_mode = "disabled"` already shipped and tested,
  and prevented re-adding the `shell_enabled` switch this repo had deleted as
  redundant. The recon skill was invoked on this one.
- **Miss** (`shell-gating-session-log:F-1`): CLAUDE.md's "all native `Bash` are
  hard-denied" asserted a capability was absent. I took it at face value and
  reported to the user that the project's mandated gate could no longer be run.
  The user corrected it in one message. The recon skill was NOT invoked — the
  claim arose mid-report, not at an implementation seam.

**Evidence.** A positive control settled it in one call: `cat src/main.rs` matches
`pre-tool-guard.mjs:165`'s own most-specific block branch
(`/^cat .*\.(rs|ts|...)/`) and executed anyway, returning file contents with only
a PostToolUse `[cs-hint]`. Reading the hook source would have concluded the
opposite — line 177 is `enforce("This call is blocked...")`. Two further facts
falsify "hard" even as design: `BREAKER_THRESHOLD = 3` stands the guard down after
3 unanswered redirects ("advisory context, NOT an auto-approval"), and
`~/.claude-sdd/settings.json` carries eight `Bash(...)` `permissions.allow`
entries CLAUDE.md never mentions.

**Classification against *Every promotion audits the promoted set*:** failure mode
**3, Unreachable** — the promoted text is correct and was not fetched at the
moment of need. Not mode 1 (not false), not mode 2 (wording already covers the
prohibition form explicitly), not mode 4 (the failure plainly still happens). The
remedy is placement, not rewording. Note `R-19` is the same law in its
assert-a-checkable-fact form and is *already* quoted in SKILL.md § When NOT to
Use — so the text was not merely unfetched, it was unfetched despite sitting in
the section that governs exactly this situation (a claim made during read-only
reporting).

**Promote-when:** do NOT add a law. Per the routing rules, a session-opening
promotion needs a base arm showing an unaided agent does not already verify a
doc-asserted prohibition before acting on it. Until that measurement exists this
stays an audit row. If a third instance lands with the skill uninvoked, that is
the base-arm signal.

**Status:** open — the CLAUDE.md sentence was rewritten 2026-08-27 and
`shell-gating-session-log:F-1` is now `fixed-verified`, so the *instance* is
closed. This entry stays open on the part that is not: no base arm has been run,
so the mode-3 Unreachable verdict has no measurement behind its remedy.
Re-probing for the fix also widened the instance — native `Read` and `Edit` reach
source files unblocked too, and `Grep`/`Glob` are absent from the tool list
rather than denied, so the one sentence had collapsed three different mechanisms
into a single false absolute.

**Kin:** `R-19` (assert-a-checkable-fact form of the same law, quoted in
SKILL.md), `R-89` (freshness/placement — the other law that recurred because it
was never fetched), Law A in `## The seven laws`.

**Valid:** dated 2026-08-27

The classification rests on the promoted SKILL.md text as of codescout-companion
at this checkout; re-read Phase 1's prohibition bullet before reusing the
mode-3 verdict.

**Rests on:** the positive control, not the hook source — the two disagree, and
that disagreement is the entry's whole content.

## R-125 — Miss: law C covers the empty result, not the abundant one — and my first fix for that was law C again

**Verdict:** miss — recon did not catch it, and the first remedy I proposed for the miss was
another instance of the same law.

**Evidence:** 2026-08-29, triaging a full-suite run (4642 passed, 2 failed) in a checkout shared
by three sessions. `retrieval::embedder::tests::a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever`
failed. `src/retrieval/embedder.rs` was clean and committed, but a peer's
`crates/codescout-embed/src/remote.rs` was dirty at +137 lines and `grep -c timeout` on its diff
returned **19** — and the failing assertion pins the `e.is_timeout()` arm. I told the peer their
in-flight read-timeout port was the likely cause.

It was not. The real defect was in the **test**: `embed_one_batch` (`embedder.rs:606`) drives
the dense and sparse legs through `tokio::try_join!` (`:617`), which returns whichever errors
first; the test wedged both bases under one shared read bound while asserting on the dense leg's
marker, making the surfacing message a coin flip. `.dense_only(true)` never applied —
`dense_only` is read at `:540` and `:806`, never in `embed_one_batch`. Racy since `9f4debc3`,
fixed in `21174425` by calling `dense_batch` directly. Mechanism verified here at the bytes, not
taken from the peer's account. Full narrative: `bug-fix-session-log:F-78`.

**Why law C did not fire.** Law C — *a search that finds nothing is evidence about the search* —
is stated entirely in terms of the **empty** result, and all five recurrences
(`R-3` → `R-113` → `R-77` → `R-79` → `R-104`) are zeros that lied. Nothing in that wording
reaches a search that comes back **full**. The failure is identical in kind: the instrument
answered truthfully about a *text* and I read the answer as being about *causation*. The
abundant form is arguably worse, because a zero at least prompts a question, whereas `19` is
quantitative and reads as rigour.

**The part worth the entry.** My first draft of this R-N proposed the remedy *"run the failing
thing in isolation — that is the instrument whose subject is the failure."* **That is wrong here,
and wrong in exactly the way law C describes.** This test *passes* in isolation; passing under no
contention is the shape of the bug. `codescout-24` ran it 5/5 isolated and we both read that
green as evidence of *no defect* — a negative result taken for absence, which is law C's original
form, committed while I was writing the entry about law C. The instrument whose subject was
actually the failure was **reading `embed_one_batch`**: about a minute, and neither of us did it.

**Proposal (revised).** Extend law C to both polarities, and make the selection test explicit
rather than naming a favoured instrument:

> A search result is evidence about the search — when it is empty **and** when it is full. A
> count of matches measures a text, never a cause. Before trusting any check as decisive, ask
> whether it can **express** the failure at all: a test that passes in isolation cannot be
> diagnosed by running it in isolation, and a green there is absence-of-evidence wearing
> evidence-of-absence's clothes. When a failure is contention-dependent, the code path under the
> assertion is the artifact; the run is not.

**Severity of the miss:** low — revised down. The mis-aimed pointer cost the peer one `symbols()`
read rather than a bisect, because the report carried the failing assertion text verbatim.
Reporting the failure was net positive: the defect is only observable under concurrent load, so
the session that wrote, ran and shipped the test green could not have found it alone. The
lesson is about attribution, not about reporting.

**Second datapoint — the fix author, verifying with the same blind instrument.** Volunteered by
`fix-embedding-transport-stage-1` against its own work, unprompted. Having fixed the race, it
ran the test **10/10 in isolation** and led its report with that number. But 10/10 uncontended is
evidence about uncontended runs — the identical instrument, aimed at the identical blind spot,
that had acquitted the bug twice already. Had the race still been present it would very likely
have returned 10/10 anyway, which is exactly what it did for `codescout-24` and for me. What
actually carries the fix is **structural**: verified here at `embedder.rs:992-996`, the test now
calls `e.dense_batch(&["x"])` directly — one leg, no `try_join!`, so there is no race left to
lose, and `dense_batch` is also the function that owns the `is_timeout()` error map under
assertion. The repeat runs are corroboration and rank second.

That makes three passes of one instrument in one afternoon — diagnosis, peer confirmation, and
fix verification — each reading the same green as proof of a different proposition. The law bites
the author of a fix as readily as its diagnostician, and this instance landed minutes after that
session had written a commit message about vacuous verification.

**Status:** promoted 2026-08-30 — branch `recon/r-125-name-the-proposition`, commit `ea90d80` in `claude-plugins`. Landed at the **top of Phase 3 — Externalize**, per the case-3 disposition argued below: placement, not rewording, attached to the act of writing an entry rather than to a class of tool. Not pushed and no PR opened — that is outward-facing and awaits the operator. Two datapoints for the
can-this-instrument-express-the-failure form (the isolation run as diagnosis; the isolation run
as fix-verification), one for the abundant-result form.

**Promote-when:** **fired 2026-08-29.**

**Disposition — corrected, and the correction is the finding.** I first filed this as case 2
(**outgrown**), the disposition law C has earned four times. `fix-embedding-transport-stage-1`
contested it and was substantially right: **both** of today's instances occurred in sessions with
the skill LOADED — it invoked reconnaissance three times — and law C's existing text already
covers both errors. *"An instrument that returns a full answer is evidence about the predicate you
supplied"* states my keyword-count error better than I restated it, and *"run a positive control
… one per state you believe the instrument can report"* is exactly the check that would have
caught the 10/10: a positive control against a known-racy build shows the isolated run cannot
report that state at all. So this is **not a wording gap**, and a sixth mechanism bolted onto
what the ledger itself notes is already the longest bullet in the file is the accretion the audit
section exists to prevent.

Where I part from that session: it concluded *neither* case 2 nor case 3. Case 3 is
**Unreachable** — *"general enough, and still not reached at the moment of need… Remedy is
placement, not rewording"* — and its precedent (`R-89`) happens to be text that was never
fetched, which is what makes "reached" read as "loaded". It is not the same thing. Both of us had
the bullet in context and neither of us consulted it at the decision point, which is precisely
*not reached at the moment of need*. **Loaded is not reached.** So this is case 3, and the fix
that session proposes — a short trigger-shaped clause attached to an action — IS case 3's
prescribed remedy, placement rather than rewording. Its diagnosis and mine converge on the same
patch by different routes; only the label differs, and the label is what tells the next auditor
not to lengthen the bullet again.

The placement is also *not* the session-opening surface, which case 3's precedent reaches for.
That channel exists for laws recurring in sessions that never invoke the skill; these two sessions
did invoke it. The gap is that law C is written as a property of **instruments**, and both of us
needed it as a check on **a sentence we were about to write**. Neither of us was reasoning about
an instrument at the moment of failure — we were writing up a result.

**Proposed clause (trigger-shaped, attaches to an act of writing, not to a class of tool):**

> Before citing a green — or any confirming result — as evidence, name the proposition it proves,
> then ask whether a broken world produces the same result. An uncontended 10/10 is the output a
> still-racy test also gives.

**Routing.** Craft-shaped (true in any repo, no project dialect), so the destination is a PR
against `codescout-companion/skills/reconnaissance/SKILL.md`, citing `R-125` and
`bug-fix-session-log:F-78`. The PR description must carry the load-bearing fact — **both
datapoints occurred with the skill loaded** — because it is the one most likely to be dropped in
summarising, and without it a reviewer will read this as the sixth wording gap and lengthen the
bullet. If the reviewer still prefers mechanism six, the evidence supports that; they should just
see first that the add-a-mechanism reading has been tried five times against this law and that
neither of today's failures was a wording gap. **Raised 2026-08-30** as `ea90d80` on `recon/r-125-name-the-proposition`.

The commit message opens with *"BOTH RECORDED FAILURES HAPPENED WITH THIS SKILL ALREADY
LOADED"* — first line, capitalised — because this entry identified that as the fact most
likely to be lost in summarising, and a reviewer who loses it reads the patch as a sixth
wording gap and lengthens the bullet. The message also states outright that law C's existing
text already covers both errors correctly, so the add-a-mechanism reading is pre-empted
rather than merely unmentioned.

**A converging datapoint arrived the next day, and it changes how much weight the clause
carries.** The same principle was promoted independently on the codescout side as
`docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`, distilled from **nine**
instruments across four concurrent sessions in one day: `git diff --cached --stat` answering
"how many lines" when asked "whose lines"; mtimes destroyed by the prober's own `touch`; a
cached clippy green; a green `cargo test` on a tree failing `-D warnings`; a mid-run log
total; a filtered test count; a page cap; a passing unit test whose caller ignored the
policy; and `ListAgents` reporting a confident count of an incomplete set.

That ADR is repo-shaped and governs instrument **choice**; this clause is the craft-shaped
half and fires at the moment a claim is **written down**. Neither subsumes the other, and
the two were reached by different routes from non-overlapping evidence — which is the
strongest argument available that the underlying law is real rather than an artefact of one
bad afternoon.

**Kin, and an admission:** [[R-120]] — *"a decision procedure is a seam; reading it far enough to
get an ANSWER is not far enough to get its PRECONDITIONS"* — describes exactly how the wrong
disposition got filed. The routing test answered cleanly (craft-shaped → SKILL.md) and I let that
clean answer carry the **diagnosis** as well, when it only ever settles the **destination**. Same
ledger, five entries earlier, same pass.

**And it happened twice in this thread, to two different procedures — which is the stronger
datapoint for [[R-120]] than either instance alone.** I read the promotion **routing test** far
enough to get an answer and stopped before the preconditions that govern the diagnosis.
`fix-embedding-transport-stage-1` read case 3's **definition** far enough to reach its precedent
(`R-89`, never-fetched text) and let that precedent narrow the definition, concluding "neither of
the four" when the definition's own words — *at the moment of need* — already fit. Same failure,
opposite artifacts: one procedure under-read past its answer, one under-read past its example.
Neither of us was careless; a decision procedure that yields a clean result is exactly the shape
that stops you reading on. Its own diagnosis of the miss: *"I had the right remedy and the wrong
label, which is worse than the reverse, because the label is what stops the next auditor
lengthening the bullet a sixth time."*

**Valid:** dated 2026-08-29

**Rests on:** `embed_one_batch`'s `try_join!` race and its non-consultation of `dense_only`, both
read directly at `embedder.rs:606-617`, `:540`, `:806`.
## R-126 — A test that pins two functions to each other is blind to their shared convention being wrong

**Valid:** invariant

**Rests on:** the distinction between an *agreement* assertion and a *property*
assertion. An agreement assertion can only observe divergence; it is satisfied by any
world in which both sides are wrong the same way.

**The shape.** Two functions produce related values — an encoder and a decoder, a writer
and a reader, a path-builder and the declaration that names the path. The natural test
asserts they agree. That assertion is real and worth keeping: a disagreement is a genuine
defect class. But it cannot see the case where both implement one unsound convention,
because agreement is exactly what that case produces.

**Measured 2026-08-30**, BL-50. `augmentation_sidecar::path_for` and `rel_path_for`
derived a sidecar name from an artifact's **file stem**. The unit test asserted both
returned the stem-keyed name — one absolute, one repo-relative — and passed. Whole gate
green at 4833/0, design reviewed and approved.

The convention was unsound: a stem is not unique. `docs/research/README.md` is a real
augmented artifact in this repo, so any second augmented `README.md` would have shared its
sidecar, the second export silently overwriting the first's shape, both artifacts then
restoring to one prompt. The test was positioned exactly where that defect lived and could
not report it, because both halves were wrong together (`f565504a`).

**The tell, and it is available at write time.** Ask what the assertion RANGES OVER. An
agreement assertion ranges over one input and two implementations. A property assertion
ranges over the input space: *is this mapping injective?* *does it round-trip for any
value?* *is the result unique across the corpus?* When two functions share a derivation,
the property is the thing under test and the agreement is a corollary — write the
property, keep the agreement as a second assertion. The corrected test does both: two
same-stem paths must not collide, AND the two functions must still agree.

**What surfaced it** was not analysis but a dry run against the real corpus: the export's
first printed line was the collision. A real corpus contains a `README.md`. See
`bug-fix-session-log:W-76` for that half — this entry is the structural law, that one is
the practice.

**Relation to law C** (a zero that lies): distinct. Law C is about a RESULT that misreads
as an answer. This is about an ASSERTION whose form excludes the defect from its range.
They compose badly — a stem collision produces no zero and no error, just two artifacts
quietly agreeing on one file.

## R-127 — Miss: I broke R-125's clause verifying R-125's own release, minutes after promoting it

**Verdict:** miss, self-caught within the turn. The strongest confirming instance
[[R-125]] has, because the author of its clause committed its exact error while checking
that the clause had shipped.

**Evidence:** 2026-08-30, immediately after `ea90d80` merged and `codescout-companion`
released as 1.19.8. The check was whether the new Phase-3 clause had propagated from the
repo into the plugin caches all three profiles load from. I ran:

```
find ~/.claude/plugins -path "*reconnaissance/SKILL.md" | head -1
```

then grepped the clause out of the single path it returned — `0` in each of the three
profiles. I reported *"that's the drift, caught live"*, citing `claude-plugins`' own
tracker entry about stale plugin-cache drift on this host as corroboration.

**It was false.** Every profile holds **two** version-keyed cache directories, `1.19.7/`
and `1.19.8/`, and `head -1` returned the stale `1.19.7` copy each time. Enumerating all
six copies instead of sampling one shows `clause=1` in every `1.19.8/`. The release was
correct and complete, in all three profiles, and had been the whole time.

**Why the instrument lied in R-125's exact shape.** `head -1` answers *"the first path this
walk happened to yield"*. The question was *"what does the loaded copy contain"*. Those
differ whenever more than one copy exists — which is the normal state of a version-keyed
cache, not an edge case. The `0` was true of the file it read and said nothing about the
proposition it was cited for, and a count reads as a measurement rather than as a sample.

**Two aggravating factors, and they are separable.**

- **The zero agreed with me.** `claude-plugins` documents stale-cache drift on this host,
  so `0` confirmed a hypothesis already held. Law C's usual framing assumes a surprising
  zero prompts a re-check; this is the inverse case, where agreement *suppresses* the
  re-check. A confirming negative deserves the scrutiny a contradicting one gets
  automatically.
- **Having just written the rule did not make it fire.** The clause had been merged,
  released, and propagated minutes earlier, and I had argued its placement at length.
  Authorship is not activation.

**What actually caught it:** re-reading my own command while composing the write-up, and
noticing `head -1` inside the sentence I was about to publish. That is precisely where
[[R-125]] placed the clause — Phase 3, the act of writing — and it is the only reason the
claim was retracted before anything was built on it. The placement argument is now tested
rather than merely reasoned: the clause fires at write-up time because that is where the
author is forced to state the proposition out loud.

**Severity:** low. Self-caught in one turn, retracted in the same message, no decision
taken on the false reading. The value is entirely in the provenance.

**Status:** validated — a confirming instance of [[R-125]], not a new law. No promotion
owed; the clause it confirms is already shipped.

**Valid:** dated 2026-08-30

**Rests on:** the six cache paths enumerated under
`~/.claude*/plugins/cache/sdd-misc-plugins/codescout-companion/{1.19.7,1.19.8}/`, and
`docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`, whose Decision clause 1
this violates verbatim.

## R-128 — Enumerate the call sites of a must-call function and look for the absentee — it finds what a pairwise diff cannot

**Status:** validated — found a real defect on first use (`BL-66` / `a7af9964a16e8056`), now **fixed and archived** (`1909e5f0`, patch-id `90c1612bcd948c09e0fd373be2e754134bf9a463`). The reproduction raised its severity `low` → `high` and falsified three premises of the filing, so the technique found something strictly worse than what it was credited with: not a TLS-only misreport for operators with an https host, but a process abort for **every external consumer of the crate at zero configuration**. Worth noting for the technique's own account — the absentee it surfaced was the whole finding, and every later correction came from running the reproduction, not from more reading.
**Valid:** invariant
**Rests on:** `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`; the framing that a clean result must be interrogated, not banked.

**Observed:** asked to audit a repo's root/crate boundary for a defect class named from two
instances — *a hazard handled on root's side of a duplicated pair and never on the crate's,
where root's own behaviour masks the gap so the crate ships it to every external consumer
while the one caller that would notice is shielded.*

**The brief's own noun nearly cost the finding.** "Audit the **pairs**" implies diffing twin
functions, and a pair-shaped sweep is what I ran first. It came back clean, correctly: four
pairs, all either delegated, byte-identical, or already reconciled. A clean sweep against a
class with two confirmed instances is exactly the confirming-negative the ADR warns about,
and stopping there would have produced a plausible "no further instances" — the shape of
answer that is hardest to doubt because it agrees with hoping you are done.

**What worked instead was inverting the question.** Not *"which functions are duplicated?"*
but *"which invariant does this codebase assert, and which sites break it?"* Root's
`transport.rs` states one in a comment — *"the only documented failure is TLS backend
initialisation, which `install_default_crypto_provider` has already performed at **every
construction site**"*. Enumerating the call sites of that must-call function gave five, and
one HTTP-client builder in the crate was **absent from the list**. That absentee was the
defect: reachable for external consumers, invisible in-process because root's startup call
installs it globally first — the masking mechanism the class is made of.

**Why the pairwise sweep structurally could not find it.** The defective function has no
root twin. A diff needs two things to compare; an absentee is defined by there being only
one place it *should* have been and isn't. The two instruments have disjoint blind spots,
which is the whole point of naming this one separately rather than treating it as "audit
harder".

**The technique, stated so it transfers:**

1. Find a function or step the code says must always precede something — a provider
   install, a lock acquisition, a validation, a normalisation. Prose comments asserting
   *"at every call site"* / *"always performed before"* are the richest source: someone
   wrote that sentence because they had to reason about it.
2. `references(symbol, path)` or `grep` its call sites and list them.
3. List the sites that need it — the constructors, the entry points, the builders.
4. **The defect is the set difference**, and it is usually one element.

**Blind spot, so this is not over-trusted:** it only finds violations of invariants someone
has *stated*. An invariant that was always merely assumed leaves nothing to enumerate, and
this technique is silent on it — exactly as silent as the pairwise diff was on the
absentee. Neither instrument covers the other's gap, so a clean result from one is not
evidence about the other's territory.

**Promote-when:** a second defect is found this way, or a session runs the enumeration and
the set difference is empty on a surface later shown to be defective (which would bound the
technique rather than confirm it).

## R-129 — In a shared checkout, the deliberate break this project mandates is indistinguishable from a defect

**Verdict:** miss (×1, reported and relayed) + near-miss (×2) · **Observed:** 2026-08-30, four sessions in one checkout

**Valid:** invariant

CLAUDE.md and `sdd-ruling-log` both mandate the practice: *demand a deliberate break* —
never "add the check and re-run a green suite", but break the thing and watch the specific
test die. This entry is about what that practice costs when the tree is shared, which no
existing entry covers: **every other session sees the break as a defect, with nothing in
the tree distinguishing it from one.**

**Instance 1 (mine, the near-miss).** Mutation-verifying a new packaging gate, I removed
`"!docs/trackers/operator-rules.md"` from `Cargo.toml`'s `exclude` so I could watch the
gate fail. It did, correctly. An IL-3 refusal then killed my restore step mid-command —
the restore was chained after a `sed` on a source file, which the guard blocks — so the
mutation sat on disk several minutes longer than intended. `codescout-ae` ran the
four-command gate in that window's vicinity. Their own words: *"had my gate landed inside
your Cargo.toml mutation window, I'd have seen a red packaging test and had no way to know
it was a deliberate mutation. I'd very likely have reported it as a defect — and, on
today's form, at the wrong person."*

**Instance 2 (the general form, same session).** `cargo test` in a shared checkout compiles
the **working tree**, and cargo does not consult git. So an untracked file from another
session is silently in your run: `codescout-ae`'s "4862 passed" included my uncommitted
`tests/packaged_includes.rs` and a third session's three dirty source files, and they had
told their user "HEAD is green" on the strength of it. A deliberate break needs no
`git add` to reach a peer's gate.

**Instance 3 — it actually happened, and it happened to this entry within minutes of the
entry being written.** `codescout-ae` measured a lean-lane failure in a sparse-classifier
test, checked attribution before sending, and reported it as the documented
full-passes/lean-fails feature-gating class. I verified it was not mine, relayed it to
`swap-dense-leg-remote-embedder` as theirs, and cited the CLAUDE.md class in doing so. All
three of us were careful. **All three of us were wrong.**

There was no lean-only defect. `swap-dense-leg` was mutation-checking: mutation 2 deleted
`|| err_str.contains(SPARSE_STATUS_MARKER)` — exactly the arm the failing test depends on —
ran the subset, and restored it. The quoted failure is character-identical to their expected
output for that mutation. The "full passes, lean fails" shape had a duller cause than
feature gating: **a four-command gate runs over several minutes, and the deletion landed
between the `cargo test` and the lean lane.** Confirmed by me on the restored tree with the
check this repo's own trap demands — `cargo test --workspace --no-default-features
a_sparse_status_body` reports **`1 passed`**, not a filtered `0 passed`.

Their framing is sharper than the one this entry opened with, and is the reason it earns a
separate entry rather than a line under the working-tree hazards: *"the index/formatter/
working-tree hazards make **numbers** unreliable. This one makes a peer confidently report a
**specific defect** with correct evidence and a wrong conclusion — the failure is genuine,
reproducible, and reads exactly like a real regression."* **It is the hazard where being
careful and being wrong are most compatible.** Attribution discipline does not help: the
report was routed correctly on the second try and was still about nothing.

One more thing I got wrong in the same episode, smaller and the same shape. When my own lean
run came back green I wrote *"the peer's failure resolved in the interim"* — outcome right,
stated cause a guess. They had restored a mutation, not fixed a defect. A green re-run tells
you the state now; it does not tell you why, and this entry is about how expensive that gap
is when several sessions edit one tree.

**Instance 4, and it widens the entry: the break does not have to be deliberate.**
`swap-dense-leg` reported `tests/packaged_includes.rs` failing `cargo fmt --check` and
correctly declined to touch a file they had not written. It was mine, and it was real — for
about three minutes, between my writing the file and my running `cargo fmt` before
committing. Verified after the fact from two directions: `cargo fmt --check` on the tree
exits 0, and `rustfmt --check` on the **committed blob** from `188cf9f0` exits 0, so the
file has never been unformatted in any committed state.

So the population is not "deliberate breaks". It is **every intermediate state of every
session's work**, and ordinary in-progress files produce the same false report as a mutation
does. A peer's gate samples the tree at an arbitrary instant; nothing marks which files are
mid-edit, and `cargo` consults the filesystem rather than git. Deliberate breaks are merely
the subset an author can *predict* and therefore announce — which is why the announcement
rule is worth having and also why it cannot be the whole remedy.

**The two-sided protocol,** which is `codescout-ae`'s formulation and the durable output of
the day. Mine: *announce the window before opening it, not after closing it* — prevents the
alarm. `swap-dense-leg`'s: *an instrument that cannot see the tree's intent supports
"something is red", never "this ships" — so ask the owner before filing* — prevents the
misfiling when the announcement is missed or, as in instance 4, was never possible. Either
alone leaves the gap open. Note both halves are the same shape as this file's recurring law:
an under-reporting instrument supports the weaker claim, never the stronger one.

**Why it has to be two-sided — `codescout-ae`'s formulation, sharper than the tidy version.**
The tempting summary is "three sessions each caught one of the others' errors and none
caught their own." That is nearly right, and the imprecision matters. They *did* self-catch
twice today, both **method** failures: a `strings` discriminator that predated the fix
(caught by running the positive control) and a scope error where the "before" check read one
file while the binary contains every file (caught by widening). What no one self-caught were
the **conclusions** — the misrouting and the mutation misdiagnosis — and both came from a
peer.

> **Self-catching works on your method and fails on your conclusion.** A method error has a
> check you can run; a conclusion error is invisible from inside, because the evidence
> supporting it is real.

Same shape as instance 4: the population is not "errors people were careless about", it is
**errors whose evidence looks correct from where the author stands**. No amount of care
reaches those, which is exactly why the remedy is another session rather than a stricter
personal discipline.

**The rule.** *Announce a mutation window before opening it, not after closing it.* It costs
one message and converts a false alarm into a no-op. It generalises past mutation testing to
any deliberate break left on disk: a RED-first commit not yet made, a temporarily stubbed
function, a config edited to force a failure path.

**What makes this different from `R-90`.** R-90 is about *writes* crossing between sessions
— `git add`/`git commit` annexing a peer's work. This is about *reads*: nothing crosses, no
file is claimed, and the harm is entirely in the other session's interpretation. The remedy
is therefore not a flag or a readback but a message, and no git discipline reaches it.

**Why the restore failed, worth keeping separately.** I chained `cp backup Cargo.toml && sed
-i <source file> && cargo test` in one command. The `sed` on a source file is IL-3-blocked,
and the block killed the **whole** command — including the restore that had been placed
first for safety. *A cleanup step chained ahead of a blockable step is not protected by
being first; the block takes the command, not the step.* Put restores in their own call.

**The remedy, supplied 2026-08-31 by `codescout-ae` and validated by a real near-miss.**
"Put restores in their own call" identified the problem and does not solve it: a separate
restore call still never runs if the session is killed, times out, or is interrupted before
reaching it. Their first attempt at the gate-order mutations ran through `cargo test`, hit
the 30s timeout **mid-sequence**, and the shell died with `CLAUDE.md` possibly mutated — in a
checkout five sessions share, any of which could have committed it.

What saved it was a `trap … EXIT` installed **before the first `sed`**, and they verified it
had fired with `cmp` rather than assuming.

> **A mutation of a shared file needs its restore installed BEFORE the mutation and
> INDEPENDENT of the happy path** — because the failure mode that gets you is the one where
> your cleanup line never runs at all.

The lesson is explicitly *not* "use a longer timeout". A longer timeout shrinks the
probability and leaves the mechanism intact; `trap` removes the state in which the mutation
can outlive the process. Same shape as `R-141`'s *omit rather than promise* — prefer the
variant that cannot fail over the variant that usually succeeds.

A second technique from the same run, worth having: they ran the mutations against the
**already-compiled test binary**, since `CLAUDE.md` is read at runtime. No recompile between
mutations, which cut the exposure window on a shared file from minutes to milliseconds. Where
the thing under test is data rather than code, the window is a choice.

**First application, ~1 hour after writing — and it is a PARTIAL confirmation, which is the
useful kind.** `codescout-ae` opened a mutation window on `doctor.rs` and announced it
first, naming the two tests that would go red and their expected duration. I held my gate
runs, at no cost, and said so. The first half of the protocol did exactly what it is for:
the announcement let a peer decide whether to care, and the answer was *no* — which is the
normal case and why it is cheap.

**The close failed, through nothing the author did.** Their MCP server dropped mid-run while
the mutation was on disk; for part of the window they had no codescout tools to restore
with, and the announced "~2 minutes" ran long. A peer holding gate runs on their word had no
way to learn the window had outlived its estimate. So the rule needs a companion clause:
**announce the close explicitly, because a silent window is indistinguishable from a
finished one.** An announced duration is an estimate, not a contract, and the estimate is
exactly what fails when something goes wrong.

**And the worse consequence, which is not a false alarm at all: the operator rebuilt during
the window.** That release binary was compiled from deliberately-broken source. Here the
blast radius was nil — verified rather than assumed: `git grep 'fn status_token_present'
HEAD` returns **0**, the function exists only in uncommitted work. But the general case is
strictly worse than everything else in this entry: **a rebuild inside a mutation window
bakes the mutation into the binary every session then uses, and `git_sha` names a commit
that never contained it.** Every earlier hazard here produces a wrong *report*; this one
produces a wrong *artifact*, and the MCP server serving every session is that artifact. See
[[R-130]] for the sibling in test-reading, and `docs/PROBES.md`'s `codescout version` note,
which this sharpens — the field is not merely a lower bound on what got compiled, it can
name a commit whose source is not in the binary at all.

**Status:** promote-when **FIRED** on the day of writing — promotion not yet applied.

**Promote-when (as written):** *a third instance, or one where the false alarm was actually
reported rather than caught.* **Both** conditions fired within minutes, by instance 3. The
target is CLAUDE.md, beside *demand a deliberate break* — that is the practice generating
this, and in a shared checkout the two have to be read together. Wording to promote: **break
deliberately, but announce the window first; and when a peer reports a failure in a tree you
are mutating, say so before they diagnose it.**

Holding the promotion for an operator decision rather than applying it, on `R-90`'s own
lesson: that entry's Promote-when has been FIRED and unapplied across four instances because
its target is a workflow change (per-session worktrees) that only the operator can take.
**Correcting my own framing of that trade, on evidence from `codescout-ae` that I verified
in code.** I wrote that *per-session worktrees would dissolve `R-90` and `R-129` together*,
as though the structural fix were a clean win deferred out of inertia. It is not free, and
the cost falls on exactly the surface these entries live in.

`append_entry` **refuses id allocation from a worktree checkout** —
`src/librarian/tools/append_entry.rs:93-103`, a `RecoverableError` whose hint prescribes
"record the entry in a worktree-local file and fold it into the ledger after the merge". The
refusal is not fussiness: its own comment explains the alternative is **unrepairable**,
because `merge_worktree`'s renumber runs only over params rows — two prose `## PREFIX-N`
sections merge into one file and the token acquires two active definers.

So under per-session worktrees, every `F-N` / `W-N` / `R-N` / `BL-N` / `ET-N` append becomes
two steps with a merge between them. **Today alone that is `R-127`, `R-128`, `R-129`,
`W-78`, `W-79`, `F-80`, `BL-66`, `ET-10` and several bug files, across three sessions.**
`codescout-ae` recommended a worktree to their operator for `BL-44` and withdrew it for this
reason: a worktree cleans up the gate signal while complicating the half that records what
you learned.

That does not settle the question, it reframes it. Worktrees are a structural fix with a
**known cost in ledger friction**, not an obviously-better fix that keeps being deferred out
of inertia — and the operator should have both numbers. The CLAUDE.md rule remains the
cheaper move, and it is now cheaper for a stated reason rather than by assumption.

## R-130 — A mutation kill is not evidence that the named guard fired

**Verdict:** technique (found by `codescout-ae`, checked against my own day's work) · **Observed:** 2026-08-30

**Valid:** invariant

Mutation testing's whole appeal is that it converts "the test passes" into "the test can
fail". This entry is the level below that, and it is not covered by
[[tests-that-cannot-fail]] or by the *vacuous assertion* material: **a test can die under
your mutation for a reason that has nothing to do with the guard its name claims.** The
matrix records `KILLED`, you tick the box, and the named clause was never exercised.

**The instance.** Mutation-checking BL-44's `status_token_present`, `codescout-ae` replaced
a boundary-anchored regex with a plain `contains` and predicted two deaths. Both died. But
`status_token_present_does_not_match_a_status_word_inside_a_longer_word` died on its
**first** assertion — the positive separator one — and never reached the boundary
assertions its name is about. It reported as a kill for a guard it had not run.

The fix was to split it, and the discriminating evidence came from a *second* mutation:
strip-punctuation normalisation, the obvious alternative implementation their doc comment
argues against. That keeps `**done, archived**` matching `done-archived` (separator test
green) while making `done` a substring of `abandoned` (boundary test dies). Two mutations,
two **disjoint** kills, each test provably guarding its own clause.

**The rule.** A kill counts only if you know **which assertion** died. In practice:

- Read the panic's assertion, not just the test name. `left`/`right` and the message tell
  you which clause fired; the name only tells you what someone intended.
- **One mutation is never enough for a multi-clause test.** Design a second mutation that
  the first clause survives — if you cannot construct one, the clauses are not separable
  and the test is really one test wearing two names.
- A test whose assertions are ordered `positive` then `negative` is the risky shape: the
  positive one is usually the easier thing to break, so it absorbs the kill.

**Checked against my own work the same day, rather than assumed.** Two mutation matrices I
published today, both re-examined for this failure mode:

- `reindex_cli_never_wipes_augmentations_under_the_root` — died on the augmentation
  `.expect(...)`, with the earlier `COUNT(*) == 1` assertion **passing first**. Right
  reason, and I had already recorded that the count could not see the wipe.
- `every_escaping_include_str_survives_cargo_package` — died on the `missing` assertion,
  and the panic named `docs/trackers/operator-rules.md` specifically. Right reason.

Both survive the check. That is luck as much as design: each has one load-bearing assertion,
so there was no earlier clause available to absorb the kill. A test with two negative
assertions would not have been so forgiving, and neither matrix would have shown it.

**Status:** open

**Promote-when:** one instance where a published matrix turns out to have ticked a clause
that never ran. Then it belongs beside *demand a deliberate break* in CLAUDE.md, as its
qualifier — that rule as written says break it and watch the test die, and this says
watching it die is not enough.

## R-131 — Three individually correct guards composed into a door nobody could open

**Valid:** invariant

**Status:** open

**Observed:** BL-50 shipped the mechanism that makes augmentation *shape* travel in git.
Three sites touch a sidecar, and each carried a deliberate, defensible guard, each pinned
by its own test:

- `doctor(fix="export_augmentations")` **skips an artifact whose sidecar already exists** —
  that is precisely what makes it idempotent, and its test says so in as many words:
  `"already-exported artifacts must be skipped, not rewritten"`.
- `reindex` **attaches only when no augmentation row exists** — repair, never sync, so a
  live augmentation whose params have moved on can never be clobbered by a stale committed
  file.
- `artifact_augment` **did not touch the sidecar at all** — it is a catalog tool.

Every one of those is correct in isolation, and I wrote or reviewed all three.

**Got:** after the first export, **no path could update a sidecar.** The composition was a
one-way door, and no reviewer of any single guard could see it, because each guard is
right. It fired within a day of shipping: a peer widened a tracker's `params_schema` enum,
ran the export, and got `exported: 0` while the committed YAML still held the superseded
seven-value list. They hand-edited the YAML (`2a8decc5`). Had they read that `0` as
"nothing to do" rather than interrogating it, a fresh clone's `reindex` would have attached
the **old** shape and reported `augmentations_restored: 1` — success.

**A stale sidecar is strictly worse than an absent one.** Absence is loud:
`augmentation_declared_but_absent` reports it and the whole `expects_augmentation`
mechanism exists to make it loud. Staleness restores clean and reports success. So the
feature converted a reported failure into a silent wrong answer for the update case — a
regression in *kind*, not degree, shipped by three correct changes.

**The pattern.** When an invariant is maintained at more than one site, reviewing each
site's guard against its own local correctness cannot find this class. The question that
finds it is not *"is this guard right?"* but *"for each transition of the state this guard
protects, which site owns it?"* Here the transitions were **create / restore / update**,
and `update` had no owner. Enumerate the transitions, then map sites onto them; a
transition with no site is the defect, and it is invisible in every individual diff.

**Cheap tell: a skip condition justified by the word "idempotent."** Idempotence is a
property of repeated *identical* calls. It says nothing about a call whose input has
changed — and "the input changed" is exactly the update transition. Any guard whose
rationale is idempotence is worth checking for a missing update owner.

**Second tell, from my own record:** I wrote in BL-50 that item (2) was *"not blocking,
since the export is idempotent."* That sentence names the mechanism of the bug and
concludes it is safe. When a risk assessment's justification restates the risky behaviour,
it is not an argument.

**Promote-when:** a second instance of individually-correct guards leaving a transition
unowned is recorded here — then promote to CLAUDE.md beside *demand a deliberate break*,
as an enumerate-the-transitions rule for any invariant with more than one write site.

**Rests on:** the `docs/augmentations/` sidecar design (BL-50) and the decision that the
catalog is authoritative while the sidecar is its committed projection. If that direction
ever inverts — sidecar authoritative, catalog derived — the analysis changes but the
transition-enumeration lesson does not.

## R-132 — Mutate once per guarded SITE, not once per feature

**Valid:** invariant

**Status:** open

**Observed:** closing R-131's gap meant hooking a sidecar write-through into
`artifact_augment`, which turned out to have **two** shape-writing paths, not one:
`create_or_replace_augmentation` (`merge=false`) and the sibling-field patch inside the
`merge=true` branch. I wrote a test for each and, out of habit rather than design, mutated
each hook separately instead of mutating "the feature" once.

**Got:** the two mutations killed **different** tests, and neither test failed under the
other's mutation.

| mutation | test that died | failure text |
|---|---|---|
| `merge=false` hook removed | `a_shape_change_writes_through_to_the_committed_sidecar` | `left: "before", right: "after"` |
| `merge=true` hook removed | `a_merge_true_sibling_change_writes_through_too` | `left: None, right: Some("rows")` |
| never-creates guard removed | `write_through_never_creates_a_sidecar_that_does_not_exist` | the forbidden file existed |

A single mutation would have produced one kill and the entirely reasonable conclusion
*"the write-through is covered."* The second site would have been unguarded, and the gap
would have been the same one R-131 describes — reopened one layer down, inside the fix for
it.

**The pattern.** A mutation run answers a question about **one line**, not about a feature.
Where a law is implemented at N call sites, N mutations are needed; one kill proves exactly
one site is guarded and says nothing about the other N−1. This is the testing-side twin of
R-131: there, enumerate the *transitions* of a state; here, enumerate the *sites* that
implement a law, and mutate each.

**Sharpening (credit: `codescout-ae`, this session).** The kill is only worth what its
*message* says. "Does the test die?" is weaker than **"does it die for the reason the test
names?"** — an `assert!(sidecar.exists())` would have died under mutation 1 too, but for
"file missing" rather than for the real defect. All three assertions above are equality
against a specific expected value, so mutation 1's `left: "before"` shows the sidecar
holding the **superseded** shape — the exact production failure, not an incidental one.
Their independent case: a sequential-await mutation produced `left: 1, right: 2`, so a
laxer `assert!(n > 0)` would have **passed** under the very bug the test existed to catch.

**Datapoint 2, MEASURED (credit: `codescout-ae`).** They ran the per-site check on
`entry_status_region`, a scan they had already mutation-tested twice today along a
*different* axis (its predicate) and been satisfied with. Result: **both sites independently
guarded** — mutation A (table-row locator) killed only the table test, mutation B (heading
locator) only the heading test, disjoint kills. A boring outcome, and that is the honest
cost side: the check is cheap and often confirms what you hoped.

**The LIMIT of this law — an absence test is satisfied by the mechanism being dead.** Also
`codescout-ae`'s, and measured against this suite rather than assumed. `assert!(x.is_empty())`
or `assert!(!file.exists())` states that nothing happened; a dead mechanism also produces
nothing. Unlike a too-lax assertion this cannot be tightened — `is_empty()` is already
maximal, and **the expected value of an absence test simply IS the failure mode's output.**
Measured here as **mutation 4**: making `write_through` return `Ok(None)` immediately, the
whole mechanism dead, left `write_through_never_creates_a_sidecar_that_does_not_exist`
**passing**, while both positive tests died. So its per-site kill in the table above was
real, but the test is evidence *only* because a positive test on the same mechanism is
paired with it. **The rule, in three pieces** — final form owed to `codescout-ae`, who corrected the
two-piece version I first wrote here:

- For a **positive** test, the informative mutation REMOVES the mechanism.
- For an **absence** test, removing the mechanism *produces the asserted outcome*, so that
  mutation is uninformative by construction.
- For an absence test, the informative mutation is the one that makes the mechanism
  **perform the forbidden act**.

Pairing with a positive test establishes only that the mechanism is ALIVE. It does **not**
establish that the absence test can SEE a violation — that was the gap in my first
formulation, and both checks are needed. Measured on this ledger's own suite, on both axes:
mutation 4 (mechanism dead) left the never-creates test passing, confirming vacuity under a
dead mechanism; **mutation 3** made `write_through` commit the forbidden act — guard
deleted, parent directories created so the write would actually land — and the test died on
its own assertion. So the absence test here is verified in both directions. `codescout-ae`
reports their two `is_empty()` silence tests have live-mechanism pairing but no violation
mutation, which by this rule leaves their detection ability **unknown rather than
established** — the distinction the third piece exists to make.

**FINAL FORM — monotonicity, and it supersedes the three pieces above** (`codescout-ae`,
`e6414362`, measured). They ran the violation mutation on their own scan and it killed
**none of six tests**. The absence test survived, as the three-piece rule predicts. But the
*positive* test survived too — it asserts that a region **containing** the drift is found,
which is monotone under widening, so no widening can kill it. That falsifies my "pair it
with a positive test" remedy directly: **both assertion shapes are blind in their own
direction**, so pairing two blind tests yields no sight.

> A test cannot detect a change its assertion is **monotone** under. Absence assertions are
> monotone under removal; existence assertions are monotone under widening. Both look like
> guards and neither fires in its own direction.

That is the general law, and absence-vs-positive is one instance of it. The practical form:
for each test, ask **which direction of change its assertion is monotone under**, and pick
the mutation that moves the other way. A property covered by two mutually-monotone tests is
covered **zero** times, not weakly — which is why the gap here was invisible to a suite that
looked complete.

**The strongest datapoint of the day, and not a confirmation.** This found a real
zero-covered property in code shipped that morning: a widened region makes the params status
more likely to be found, so the scan silently *discharges* the disagreement it exists to
report — a false negative, the failure mode that leaves no trace. Closed by a discriminating
fixture (params `open`, Status line `done`, the word `open` in the prose below), verified in
both directions: passes clean, and under the mutation it is the only one of seven that
fails, on its own assertion.

**A discriminating fixture has a load-bearing SETUP detail, and no assertion can name it**
(`codescout-ae`). The assertion states what must be true; nothing states which part of the
*setup* is what makes the test able to tell the difference. Their case: the fixture
discriminates only because the word `open` appears in prose below a `done` status line — tidy
the prose and the test silently stops discriminating, and no assertion can catch it, because
that change is monotone too. Mine:
`a_params_only_merge_leaves_the_committed_sidecar_byte_identical` works only because a
trailing YAML comment no serializer emits is appended to the fixture; without it the
assertion passes for an unconditional rewrite, `params` not being part of the rendering.
Put the reason **on the fixture line**, stating what breaks if the detail goes — not in the
test name, not in the assertion message, and never as a bare "do not edit".

**Superseded note (kept, because the prediction is the point):** `codescout-ae` reported having mutation-tested the same
function twice today along a *different* axis (its predicate), been satisfied, and only then
found `entry_status_region` had two locators behind one rule. If that run confirms one
locator unguarded, this entry has its second datapoint from an independent tracker. Their
framing is worth keeping either way: *a suite can be mutation-tested, honestly and
carefully, along an axis that is not the one with two call sites on it.* Satisfaction with a
mutation run is not coverage of the sites.

**Promote-when:** **FIRED 2026-08-30** at two independent instances — this session's
write-through pair (different tests died per site) and `codescout-ae`'s `entry_status_region`
(both sites already guarded). **PROMOTED 2026-08-31** by operator decision, in `4f79909d`. The
target is CLAUDE.md's testing discipline beside *demand a deliberate break*, and it should
land **as a pair with its limit**: a per-site mutation rule that does not mention absence
tests would licence exactly the kind of clean kill that proves nothing.

## R-133 — Loudness is a property of a path, not of a failure

**Valid:** invariant

**Status:** open

**Observed:** three failures found in one afternoon across three unrelated subsystems, by
three sessions. The first two look like one lesson and the third breaks it open.

1. **A stale augmentation sidecar** (this session). A fresh clone's `reindex` attaches the
   superseded shape and reports `augmentations_restored: 1` — success. The absence it
   replaced was loud: `augmentation_declared_but_absent` reports it by design.
2. **A widened status region** (`codescout-ae`, `e6414362`). Swallowing a whole section
   makes the params status *more* likely to be found, so the scan silently **discharges**
   the disagreement it exists to report. A false negative in machinery built to report.
3. **BL-66** (`codescout-ae`, reported; the mechanism verified here). A missing rustls crypto
   provider **aborts the process** — maximally loud, no silence anywhere — and the defect
   survived anyway, because nothing in this tree ever reaches it.

**Got:** the obvious synthesis from (1) and (2) is *"a silent wrong answer is worse than a
loud missing one."* True, and (3) shows it is the wrong axis. Loud output did not help,
because **no traversed path observes it.**

**Verified here, and stronger than reported.** `codescout-ae` framed (3) as "the one caller
who would have heard it installs the provider at startup". It is not one caller:
`install_default_crypto_provider()` is called unconditionally at **every** in-tree
construction site — `src/main.rs:253`, `src/agent/mod.rs:448`, `src/retrieval/reranker.rs:80`,
`src/retrieval/embedder.rs:339` — and `src/retrieval/transport.rs:34-38` carries a comment
*stating the invariant as a reason not to handle the error*: "the only documented failure is
TLS backend initialisation, which `crate::install_default_crypto_provider` has already
performed at every construction site." So the abort is not merely unreached by accident; it
is unreachable **by design, deliberately, at every site**, and a comment says so. The alarm
is in perfect working order and is wired to a door that is welded shut.

**The law.** Loudness is a property of a **path**, not of a failure. An alarm nobody
traverses is exactly as informative as no alarm. The axis is not loud-vs-silent output; it
is *whether any path an observer actually walks reaches the failure*. (1) and (2) fail on
output; (3) fails on reachability; all three are the same defect class — **nothing anyone
sees is different when the thing is broken.**

**How to apply.** When adding or reviewing a guard, an alarm, an error return, or a
`panic!`, name two things and write them down:

- the **path** that reaches it — a concrete caller, not "a caller";
- the **observer** who acts on what it emits.

If the honest answer is "an external consumer of this crate, which we do not have in-tree",
that is a legitimate reason to keep it — but it is **not coverage of our own risk**, and a
test asserting the alarm fires is testing a path no in-tree execution takes. Say so at the
test, or the green tick will later be read as protection.

**Corollary for tests, and the tell.** This is the reachability twin of R-132's
monotonicity: there, a test cannot see a change its assertion is monotone under; here, a
test cannot protect a path nothing executes. Both look like guards, both are green, and
neither is reached by the failure it names. The tell is common to all three cases above —
**ask what an observer would SEE differently if this were broken right now.** If the answer
is "nothing", the guard is decoration regardless of how loudly it is written.

**Promote-when:** a fourth instance, or one where the reachability question is asked *before*
the guard ships rather than after. Then promote alongside R-132 — they are one rule about
what a green suite is evidence for, and splitting them across CLAUDE.md would lose that.

**PROMOTED 2026-08-31** in `4f79909d`, and **both** branches of the criterion had fired, not
just one. The fourth instance: `drifting_fields` enumerated the six shape fields by hand, so a
seventh added later would travel, restore, and be silently exempt from drift-checking — an
alarm that fires only for the fields someone remembered, inside the safety net built to end
that class (fixed by an exhaustive destructure in `f4c52a24`). And the *asked-before-shipping*
branch fired too: `sidecar_shape_drift` named its observer (whoever runs `librarian(doctor)`)
at design time rather than after. Landed beside R-132 as its promote-when required, under a
heading scoped to every test rather than to multi-agent runs — § SDD Rulings, where the
family's earlier instances live, is framed "before your next multi-agent run" and would have
under-scoped both laws.

**Rests on:** the decision that `install_default_crypto_provider` is called at every
construction site rather than once at a single entry point. If that ever centralises, (3)'s
reachability changes and this entry's third datapoint needs re-reading — the law does not.

## R-134 — A peer view can be disjoint from the population, not merely short

**Valid:** invariant

**Status:** open

**Observed:** five Claude sessions shared this checkout on 2026-08-30 and spent the
afternoon misattributing each other's writes — six times by one session's own count. Every
attempt was an *elimination over the peers the asker could see*. Two mysteries stayed open
for hours: an unexplained mutation in `embedder.rs` at 16:56, and three worktree documents
nobody would claim.

**Got:** both resolved in one round each, by **enumerating the population from the OS and
messaging by socket path** instead of reasoning over `ListAgents`.

```
for p in $(pgrep -x claude); do
  case "$(readlink /proc/$p/cwd)" in /path/to/repo*) echo "$p $(stat -c %y /proc/$p)";; esac
done
```

Five pids with cwd here; `ListAgents` showed three. An invisible session is still
addressable as `uds:/run/user/1000/cc-socks/<pid>.sock`, and both answered immediately —
one claiming the worktree files, the other claiming the 16:56 mutation with a fingerprint
argument (`git log --all -S` returning zero for a form that demonstrably compiled).

**The escalation, and why this is not just R-50 again.** R-50 says *the view is not the
set* — name what the view dropped. A sibling finding says the view is **time-varying**, so
a count is stale the moment it is read. Both are claims about a view being **short**. The
sharper fact (measured by `codescout-f0`, 2026-08-30): a view can be **ARBITRARY** with respect to — and was once observed entirely disjoint from —
from the population you care about. Their `ListAgents` listed three peers —
`changelog-reader-d8`, `system-d9`, `claude-plugins-08` — **none of which was any of the
five codescout sessions.** Zero overlap.

**CORRECTED the same evening, and the correction is the better finding** (`codescout-f0`,
`71fdbef4`). That zero was a **reading, not a property**. Three readings from the only
session that can take them:

| ~20:06 | `changelog-reader-d8`, `system-d9`, `claude-plugins-08` | **0** of the 5 |
| ~20:2x | `system-d9`, `claude-plugins-08`, `codescout-fe` | 1 of the 5 |
| ~20:3x | `system-d9`, `claude-plugins-08`, `codescout-fe` | 1 of the 5 |

`codescout-fe` started at 17:39, before all three readings, so its absence at 20:06 was not a
start-time artefact. **The count held at 3 while the membership rotated.**

So disjointness was never the load-bearing part — **arbitrariness** is, and the rotation
demonstrates it where a single snapshot could only assert it: a reader cannot re-derive the
set by looking again. Either reading kills elimination on its own, so nothing above depends
on the zero. I had published the snapshot as a property, in an entry whose subject is
instruments that answer an adjacent question — which is the failure mode arriving through
the record of itself.

A sharper figure sits under the current reading, with its own caveat attached: 7 sessions
live, 5 with cwd here, 2 elsewhere; the three visible peers were **2 of the 2 outside** plus
**1 of the 4 inside**. On that reading the view is not merely arbitrary but *inverted* —
near-complete for the sessions sharing nothing, near-empty for the population every
attribution question is about. Two readings is not a measurement and the bias must not be
cited as one.

**And it did not survive — refuted one reading later** (`6521df4b`), exactly as that caveat
anticipated. A third observer, population unchanged at 5-in-checkout plus 2-elsewhere, saw
the opposite direction:

| `codescout-f0` (807989) | outside **2 of 2** | in-checkout **1 of 4** |
| `swap-dense-leg` (801487) | outside **0 of 2** | in-checkout **2 of 4** |

So the omission is not *inverted* with respect to the tree — it is not **oriented** with
respect to the tree at all. Arbitrariness stands; the directional refinement does not.

Note which way that cuts, because it is the opposite of the usual moral. A directional bias
would have been the **more useful** finding — a direction can be corrected for, an arbitrary
omission cannot — and it is precisely the stronger, more useful claim that failed. The weaker
one survived, and the weaker one is all the conclusion ever needed: *elimination over visible
peers is unrelated to the question*, which follows from arbitrariness alone. Three readings
from two observers support it; nothing supports the refinement.

**Three corrections to this entry within an hour, each caught by another party rather than
by its author** — and each author had already applied the discipline that catches it, one
level too late. That is the entry's subject recurring in its own record, and the reason it is
dated rather than tidied.

That is a different kind of defect and it demands a different response. A short view makes
elimination *weak*, so you widen it or hedge the conclusion. A disjoint view makes
elimination **unrelated** to the question — no amount of care with the wrong instrument
converges, and hedging a conclusion drawn from it is still drawing it. You must change
instruments.

**How to apply.** Before eliminating over a set, ask whether the enumerating instrument is
*known* to cover the population, or merely *plausibly* covers it. `ListAgents` enumerates
sessions a transport can see; "sessions writing to this working tree" is a filesystem-and-
process fact. They are different populations that happen to overlap sometimes, and nothing
in either name says so. When they diverge you get a well-formed answer to a question you did
not ask.

**Cheap tells, both seen here:** the instrument's units are not the question's units
(sessions-a-transport-knows vs processes-with-this-cwd); and eliminating over a small set
keeps yielding "must be the remaining one" — a shape that is equally produced by the true
answer being outside the set entirely.

**A second-order trap, worth its own line** (credit `codescout-f0`): in a live transcript, a
**count** is contaminated by the act of asking. Their grep for a filename went from 2 hits
to 10 because three sessions asking about it added mentions. Line *position* relative to the
turn boundary is not contaminated. When the corpus you are measuring records your
measurement, prefer an ordinal to a count.

**Promote-when:** one more case where the right instrument was available and unused. The
promotion target is the coordination guidance rather than CLAUDE.md's testing discipline —
this is about who is in the tree, not about what a green suite proves.

**Rests on:** sessions being OS processes with an inspectable cwd and a per-pid socket. If
peers ever become remote or multiplexed behind one process, `pgrep` stops being the
authoritative instrument and this needs re-deriving — the *lesson* survives, the recipe does
not.

## R-135 — Miss: a true measurement written up in the past tense — law B was promoted, correct, and never reached

**Verdict:** miss — the scout did not run at the moment of need, and the law that would
have caught it was already promoted and already correctly worded.

**The instance.** `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`
is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` closes with *"174 MB reclaimed,
163 MB of it regenerable `.codescout` index state."* On 2026-08-30 `.worktrees/bench` was
still on disk at **exactly** 174M total and **exactly** 163M in `.codescout`, with dir mtime
**2026-05-12** — three months *before* the closure — and a `.git` still naming the
pre-rename `code-explorer` path, which rules out delete-then-recreate since the file's own
documented rebuild command would have written a gitdir naming `codescout`.

The `du` was real. It was run before the removal, and then written up as the removal's
result. **A `du` proves size, never absence.**

**Why this is law B and not a new law.** *The instrument decides the answer* — but the
sharper form here is that the instrument answered a **true** question adjacent to the one
the sentence claimed. Not an empty result (law C), not a green that certifies nothing, not a
self-validating gate: a correct positive number, transferred to a proposition it does not
support. The skill's Phase 3 already states the remedy exactly — *"name the proposition it
proves, then ask whether a broken world produces the same result"* — and a broken world
(removal failed) produces the identical `du`, because the `du` preceded it.

**Diagnosis against the four-way promoted-set audit: UNREACHABLE, not Outgrown.** The
promoted text needs no rewording; it covers this case cleanly. It was simply not fetched at
the moment of writing a closure. Per the skill's own routing, the remedy for Unreachable is
**placement, not wording** — and the placement question is whether a closure-writing step
should carry the check, since that is the one moment the author is furthest from the
evidence and most certain.

**What this entry does NOT claim.** A peer session reports three sibling instances measured
in this tree the same day — a `strings` grep on a stripped binary returning a plausible 0, a
mutation kill crediting a guard that never ran, and an alarm wired to an unreachable code
path. Those are **reported, not verified by me**, and are recorded here as leads rather than
datapoints. One verified instance does not meet a promotion threshold, and promoting to the
session-opening surface additionally requires a **base arm** — a measurement that an unaided
agent does not already do this — which nobody has run. So: no promotion proposed. Verify the
three before anyone counts to four.

**Counterfactual.** The closure's reader inherits a false statement of fact with real
numbers attached. It survived 14 days and was found only because an unrelated worktree audit
happened to `ls` the directory. Nothing routine surfaces it: `git worktree list`,
`git worktree prune --dry-run` and `git status` are all silent on `.worktrees/`, the first
two because no registration exists and the third because the path is untracked.

**Status:** open — filed as `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`; the disk half is resolved (184M reclaimed, `bench` deliberately kept), the placement question is not.

**Valid:** dated 2026-08-30

The `du` figures and mtime are true of this machine at that instant; the law is not time-bound.

**Rests on:** law B (*The instrument decides the answer*) and the Phase 3 imperative in the
reconnaissance SKILL.md; sibling `R-125` covers the abundant-vs-empty axis, which this is
not.

## R-136 — Adjacent-proposition errors come in three kinds, and each needs a different check

**Valid:** invariant

**Status:** promote-when **FIRED** 2026-08-30, **partially applied the same day.**
`swap-dense-leg-remote-embedder` wrote `PROBES.md` rule 6 (`83d9eee0`) citing this entry
rather than restating it. The split they chose is deliberate and better than the one
proposed here: the **three-way mapping** (kind → what the probe is right about → which
check reaches it) lives in the page, because a reader at the moment of use must not need a
second lookup to learn which check applies; the **worked instances and the counterexample**
stay here, because those are what would drift if copied. Rule 6 is consequently the only
rule on that page without its incident inline, and the row says so explicitly — the obvious
future "fix" is to paste the examples in, which is the duplication the page exists to
prevent.

**Rests on:** law B (*the instrument decides the answer*), and on nine measured errors from
2026-08-30 across five sessions in one checkout.

The parent generalisation is `codescout-ae`'s, formed after a day in which five sessions
produced nine confident wrong answers: **nearly every one was a real measurement,
faithfully reported, of an ADJACENT proposition.** Not a broken probe, not carelessness —
a working instrument answering a question next to the one being asked, with a
plausible-looking bridge between them.

That names what the errors had in common. It does not tell you what to *do*, because the
three sub-kinds below need **three different checks**, and running the wrong one leaves the
error standing while feeling rigorous.

### The three kinds

| kind | the gap | the check that settles it |
|---|---|---|
| **Propositional** | the probe answers a different question | **a positive control** — does it detect a known positive? |
| **Temporal** | the probe was right about an *instant*, read as standing | **bracket the measurement** — capture state either side |
| **Sampling** | right for *this observer*, read as true of all | **a second observer** |

**A COUNT cannot answer a COMPLETENESS question — the sharpest case, three instances in one
hour.** Verifying `.worktrees/bench` still matches its pinned commit `ede25e69`, two sessions
produced two different, plausible, well-formed counts of "files present":

| observer | number | what it actually counted |
|---|---|---|
| `codescout-ae` | **851** | files matching their walk |
| `codescout-f0` | **778** | same, with `.codescout/` and `.git` pruned — and **61 tracked paths live under `.codescout/`** |

Neither is the answer, and the 778 was the dangerous one: it supports *"73 files are
missing, the corpus is damaged"*, which would have derailed an operator decision about
whether the 174M corpus is safe to keep. Both numbers answer *how many things match my
filter*. The question was **is anything missing**, and only a set difference answers it:

```
git ls-tree -r --name-only ede25e69 | sort > tracked
(cd .worktrees/bench && find . -mindepth 1 \( -type f -o -type l \) -printf '%P\n') | sort > ondisk
comm -23 tracked ondisk | wc -l     # 0 — every tracked path present
```

**`comm` cannot be wrong about this in the way a count can**, because it compares the two
sets the question is about rather than summarising either. A count summarises, and a summary
discards exactly the structure the question needs.

**And the defect shipped INTO the probe that warns about it — the best specimen of the set.**
`scripts/peer-sessions.sh`'s new summary read *"ListAgents shows 2 of the 7 live sessions. 2
are invisible"*. Under a cwd filter, 2 + 2 + self = **5**, not 7: the counters increment
*after* the filter's `continue` and are therefore filtered, while `$live` is not — a filtered
numerator over an unfiltered denominator, in the line whose entire purpose is warning about
counts that answer a different question. Caught by running the probe **both ways** rather than
once. The fix carries the measurement in a comment so the next editor cannot re-derive the
bug by accident.

From the same rewrite, the positive form: unreadable `environ` yields `?` rather than
defaulting to the default profile, because *"we could not look"* and *"it is `~/.claude`"* are
different facts — collapsing them would have the probe committing the sin it exists to expose.

Two more from the same hour, both `codescout-ae`'s, both the same shape:

- **A diff against an empty index.** `GIT_INDEX_FILE=/tmp/… git diff --stat ede25e69`
  returned `851 files changed, 272362 deletions(-)`, exit 0. The index isolation was
  *correct*; the question was wrong — a fresh index is empty, so this diffed commit against
  nothing. Note why it was persuasive: the file count **matched the true count**, because 851
  is exactly "every tracked file, reported as deleted". Plausibility manufactured by the same
  fact that made it meaningless.
- **`find -type f` twice in one session**, read as *things that are there*. It means *regular
  files*: it missed a symlink here, and earlier reported "0 files" for two directories that
  `rmdir` then refused as non-empty — the second time after the first had already been
  written up.

**Propositional.** `du` proving a directory's size, read as proving absence. `nm` on a
stripped binary returning 0, read as the code being gone. `cargo build` exiting 0, read as
*the binary carries the librarian subcommands* — two propositions with a plausible bridge.
An ignore-count padding a coverage number. `pgrep -f … | head -1` naming *a* process, read
as naming *the* session.
*The check:* run the probe against something you know is positive. `git log --all -S 'let
sparse_nonempty = async'` returning 0 is only evidence once
`sparse_nonempty) = tokio::try_join` returns 3 from the same probe.

**Temporal.** `cargo fmt --check` reporting a mutation that had already been reverted — the
probe read the file perfectly, at a moment that had passed. A `read_file` and a `grep`
straddling one real write, agreeing with each other and with neither the before nor the
after. A `ListAgents` reading published as a property of the tool.
*The check:* bracket it. `stat` the file before and after a gate run turns *"the gate
passed"* into *"the gate passed **on these bytes**"*. **A positive control does nothing
here** — the probe was working perfectly, which is exactly why this kind survives careful
instrument checking.

**Sampling.** One session's `ListAgents` showing 2-of-2 outside-checkout peers and 1-of-4
in-checkout ones, read as the tool being biased against the working directory. A second
observer measured the opposite direction (0-of-2 and 2-of-4), and the bias claim died.
*The check:* another observer. Neither a control nor a bracket touches this one.

> **This row's own case, followed to the end — and the lesson is sharper than "get a second
> observer".** Both sessions then concluded the omission was **arbitrary**. It is not:
> visibility is deterministic on `CLAUDE_CONFIG_DIR`, verified 7/7 across every live process
> and closed from *both* cells (2 + 3 + the two observers = 7, each seeing exactly their own
> profile minus themselves).
>
> > **"Arbitrary" is what two irreconcilable single-observer views look like from inside.**
>
> That is the sampling row's real failure mode. A lone observer does not merely get a *weak*
> reading — they cannot distinguish *scoped by a variable I cannot see* from *no rule at all*,
> however carefully they look, because both produce a set with no visible structure. Two
> observers with contradictory data feel like evidence of noise and are actually evidence of a
> **partition**: the disagreement is the signal, not the obstacle.
>
> So the check is not "add an observer and average". It is: **when observers disagree, look
> for the variable that makes both readings correct.** Averaging here would have produced a
> meaningless middle; the contradiction was the whole datum.
>
> Recorded because the taxonomy predicted the right instrument and neither session reached for
> it for an hour — knowing the row exists did not make anyone run its check.

### The counterexample belongs in the statement

Say *nearly* every, never *every*. The **mutual-deference deletion** of the same day does
not fit: two sessions each read the other's bug file, each judged it the better record, and
each deferred — one deleted theirs, one reduced theirs to a stub — and together they
destroyed the fuller analysis. No measurement was misread. Both readings were correct and
both acts were correct; the failure was in **composition**.

That is the one class this discipline cannot reach, and the reason to keep it visible: no
amount of checking your own instruments prevents two correct actions from composing badly.
Same family as `R-129`.

### The operational half — a caveat should name its sub-kind

The taxonomy pays off at the moment you hedge. *"Two readings is not a measurement"* is a
**sampling-adjacency flag**, and it is why the peer who tested that claim went and got a
second observer rather than running a control or re-reading. A caveat that merely expresses
doubt tells a reader to trust you less; **a caveat that names which check would settle it
tells them where to aim.** Both of the day's caveats that were written this way were
falsified within the hour, by the check they named — which is the system working, not
failing.

### An instance found while verifying this entry's own citation

Recorded because it is the propositional row biting the person who wrote it, twice inside
three minutes, on the check that seemed most obviously correct.

To confirm rule 6's citation resolved, the natural move is `librarian(action="link_scan")`
and a look for `R-136` in the `dangling` bucket. It was absent. **That is evidence of
nothing:** the finding arrays are **capped** — `dangling` and `ambiguous` both returned
exactly 50 entries with a `truncated` flag set — so absence from them is indistinguishable
from being past the cap. A zero read off a truncated report is the same shape as a `du`
read as absence.

The second attempt looked better and was also wrong. `PROBES.md` does carry an outgoing
`cites` edge to this tracker, which reads as confirmation — until you ask whether it
predates the citation. It does: `PROBES.md` referenced `reconnaissance-patterns` **twice
before** rule 6 landed and three times after, so the artifact-level edge is fully explained
without rule 6 and attributes nothing. Edges are artifact-grained; the citation is
entry-grained. Adjacent again.

What can honestly be claimed: the entry is defined by a `## R-136 — <title>` heading, which
is the shape `link_scan` requires to define a token, and the citation
`reconnaissance-patterns:R-136` names a file that exists and an entry that exists. **Well
formed and resolvable by construction — not verified by the scan.** The distinction is the
whole point of this entry, so stating it the strong way would have been self-refuting.

**Promote-when (original, now fired):** a sixth `PROBES.md` rule is written, or a session
running a check from the wrong sub-kind is recorded. **Both** happened on the day of
writing — the rule above, and the two mis-checks recorded in this section.

**Promote-when (next):** an entry-grained citation check exists, or `link_scan` surfaces its
truncation at the call site loudly enough that a reader cannot mistake a capped bucket for a
clean one. `PROBES.md` is the placement `codescout-ae` argued for and
is the better home for the table, since it fires exactly when someone is about to answer a
question with a number — but it should cite this entry rather than restate it, so the
worked instances live in one place.

## R-137 — After compaction, a peer is a better witness to your past than you are

**Valid:** invariant

**Status:** open

**Observed:** `codescout-ae` told me a hand-edited sidecar (`2a8decc5`) was theirs. I repeated
it in a later message. They then checked their own commit list, found the SHA absent, and
wrote back correcting the record — including the claim that the assertion had never come from
them, and had entered the thread from me.

I still held the message. Verbatim, from their own earlier text:

> *"2a8decc5's hand-edited sidecar is the one I wrote by hand this morning when export
> reported 0, and it says the right thing in different bytes."*

Their context had been compacted between the two messages. They had offered exactly this bound
themselves — *"I cannot rule out having written something to that effect before my context was
compacted, since I no longer hold that transcript"* — and it is what happened.

**Got:** three distinct claims, and they resolve differently:

| claim | verdict | best instrument |
|---|---|---|
| the sidecar was hand-edited | **true** | the file and its diff |
| `codescout-ae` authored the hand-edit | **UNDETERMINED** | no instrument either of us holds — see below |
| `codescout-ae` never said they did | **false** | *my* retained transcript |

The session was right about the fact it re-derived and wrong about the fact it remembered —
and the peer holding the transcript was the only party who could tell those apart.

**Corrected within the hour, by `codescout-ae`, and it is the operational half.** I first
recorded that middle row as **false**, on the strength of their commit-list check. That check
answers *"did you COMMIT this"*; the claim was about a **hand-edit**. In this checkout those
come apart routinely — whoever runs `git add` owns the SHA regardless of who typed the change,
and there is a documented instance from the same day of a peer committing a bug file and
attributing it to the wrong session. So `codescout-ae` may well have hand-edited that sidecar
and had it committed by someone else, which would make their **original** statement true and
their retraction false. Nothing available to either of us distinguishes those, and recording
"false" was me accepting a refutation that did not reach the claim.

> **A check that answers an adjacent question refutes nothing, however cleanly it runs.**
> "A compacted session's self-claims are weak" tells you to distrust. *"Check whether your
> check answers the question you asked"* tells you what to do instead.

**And the check itself was a compacted artefact.** The commit list came from their compaction
summary, not from a retained transcript — so a post-compaction artefact was used to adjudicate
a dispute about post-compaction reliability, and presented as primary evidence. A list of hex
strings is far more robust than prose recollection, but it is still a reconstruction.

That yields the sharper law, and it is theirs: **a compacted session may not be able to tell
which of its own instruments are themselves compacted.** The SHA list *felt* like ground truth
in a way the prose memory did not, and that felt-difference was tracking nothing real. Which
is the same defect as everything else measured today — an instrument that runs cleanly and
answers a neighbouring question — arriving this time as the sensation of certainty rather than
as a number.

**The law.** After compaction a session's claim about **its own past** is weaker evidence than
a peer's retained transcript. This inverts the usual default, where first-person testimony
about one's own actions outranks a bystander's: here the bystander may hold the literal bytes
while the first person holds a summary. A compacted session is not lying and is not unreliable
in general — it is reasoning from a lossy reconstruction, and the loss is invisible from the
inside, which is the whole difficulty.

**How to apply.**

- **Do not relay a peer's claim about their own authorship without a check.** Not because
  peers are unreliable, but because their evidence about their past may be strictly worse than
  yours. I treated an assertion as checked when nothing had checked it.
- **If you hold the transcript, you are the witness.** Say so with the quote rather than
  "you did" — the quote is what the other party can act on, and a bare contradiction between
  two memories has no resolution procedure.
- **Prefer a re-derivable instrument over anyone's memory** — a commit-list check, a
  `git log -S` fingerprint, a timestamp pair. Both parties here were right exactly where they
  re-derived and wrong exactly where they recalled.
- **Calibrate on what happened, not on the shape of the error.** Their stated worry was that
  I would discount their future attribution claims. The correct adjustment is the opposite:
  they reach for a check faster than anyone here, and that is the behaviour to weight.

**The tell, which both of us read and neither caught.** The false claim carried its own
inconsistency in plain text: it said *"this morning"*, and `2a8decc5` is **16:50**. They later
used that same mismatch to argue the attribution had come from somewhere other than the log —
correct reasoning, aimed at the wrong author. **A reconstructed memory tends to carry a
detail that does not fit**, and a timestamp against a claimed time-of-day is the cheapest
place to look.

**Promote-when:** a second instance where a peer's retained transcript settles a question a
compacted session got wrong about itself. Promotion target is the coordination guidance
alongside R-134, not the testing discipline — this is about who can testify to what.

**Rests on:** sessions being compacted independently while messages persist in the recipient's
context. If transcripts ever become shared or durably queryable by their author, the asymmetry
disappears and this stops being a law about witnesses — though the instrument-over-memory half
survives regardless.

## R-138 — a sweep that searches by shape returns a confident wrong count

**Valid:** invariant

**Rests on:** the search is over *propositions* (places that state X), but every cheap query is over *forms* (places matching a pattern). The gap between those two sets is invisible to the query by construction, so no refinement of the query closes it.

**Status:** validated — 3 instances, 3 sessions. The founding pair was 4-vs-5 in one run; the third (2026-09-01) is 4-vs-60, an order of magnitude worse.

**Observed (2026-08-30).** Two sessions independently enumerated every place the four-command gate is transcribed, in order to deduplicate them. Both returned **four**. The real count was **five**. The agreement between two independent sweeps was not corroboration — it was the same defect run twice, and it raised confidence in a wrong number rather than exposing it.

**The miss.** Both sweeps searched for the gate's **shape**: a fenced code block containing `cargo test`. The fifth site, `docs/ROADMAP.md:93`, states the same proposition inline, in parentheses, mid-sentence:

> ``CLAUDE.md's pre-commit line (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`) covers **one of nine** CI test cells``

It is invisible to that search and to every refinement of it — a better regex over fenced blocks still never reaches a parenthetical. It was found only when a third pass read the *file* rather than querying the corpus.

**Why it is dangerous rather than merely wrong.** The failure returns a **clean count with nothing marking the omission**. A sweep that missed a site is byte-identical in its output to a sweep over a corpus that has none. There is no partial result, no warning, no `truncated` flag — the same property that makes a zero from a mis-scoped `grep` unsafe, one level up: here it is not a zero but a plausible positive integer, which is worse, because a zero at least invites suspicion.

> **Enumerate by proposition, not by shape.** "Every place that states the gate" and "every fenced block listing cargo commands" are different queries with different answers, and only the second one is easy to write. When you cannot express the first, say so in the finding — report the count *and the form you searched for*, so the reader can see what it could not have found.

**Corollary, and it is the actionable half: the site that evades the search is the worst copy.** `ROADMAP.md:93` was the **stalest** of the five — three commands, unrevised since 2026-08-06, predating both the wide clippy form and the lean lane. This is causal, not coincidental: a site that evades one sweep evades every subsequent maintenance pass for the same reason, so it accumulates all the drift the found sites had corrected. **Expect prose restatements to be the worst copies, and go looking for them first rather than last.**

**It compounded through the reference graph.** That same paragraph pointed readers at `docs/RELEASE.md` § *Large-Cohort Promotion* as "the full gate definition" — which was the weakest of the five, running `cargo clippy --all-targets -- -D warnings` and so reaching neither `crates/codescout-embed` nor the `local-embed`-gated module. So a reader who did the **right** thing — distrusting the local prose, following the canonical pointer — landed on the least complete list and stopped. A stale pointer is worse than a stale copy: it converts the careful reader's correct instinct into the wrong destination, and the label *"full gate definition"* discharges the scrutiny that would have caught it.

**Third instance, same evening, different surface — and it is the one that generalises the rule past "prose".** Checking the clippy enumeration above before disputing it, `codescout-ae` grepped `cargo clippy`, got three hits, and was one keystroke from replying that the fourth form did not exist. It is `ci.yml:284`, `scripts/build-windows.sh clippy --all-targets -- -D warnings` — a **wrapper** invocation, so it contains no `cargo clippy` substring at all and is invisible to any audit keyed on one, which is every audit anyone would naturally write.

So the class is not *prose vs fenced block*. It is **any query keyed on a token the site does not contain** — and the site's author had no reason to include it, because the wrapper is the point. Broadening from `cargo clippy` to bare `clippy` found it immediately; the narrower key was the entire defect.

**What caught it was a policy, not attention.** The second pattern was only run because a peer was about to contradict someone and the standing rule is *verify before contradicting*. That rule fires whether or not the reader suspects anything, which is exactly why it works — and it is the same shape as the other relapse that evening, where a bare integer was reintroduced in the very commit fixing a bare integer. **Knowing the class did not prevent any of the three instances; a procedure caught one.** Do not budget for vigilance here; budget for a check that runs unconditionally.

**Detection, for next time.** Three cheap moves, in order of payoff:

1. **Read one whole file end-to-end** before trusting a corpus-wide count. The sweep that found site five was `git show HEAD:<file>` piped to a grep for the *individual command names*, not the block.
2. **Search for the narrowest token, not the construct** — `cargo clippy` rather than a fenced block containing it. A token appears in every form the proposition can take.
3. **Treat two agreeing sweeps as one sweep** unless they used different *query shapes*. Independence of authorship is not independence of method, and the second sweep here inherited the first's blind spot without either session sharing a query.


**Third instance, 2026-09-01 — and the ratio is 15×, not 1.25×.** Mining the bug corpus for a
candidate defect class, the proposition was *"bug files whose root cause is a selector narrower
than the population it names"*. The query was over **titles** matching
`only (scans|counts|reads|matches)|narrower|subset|blind to|misses`, and returned **4**. The same
proposition queried over **bodies** returned **60**. Both are clean positive integers, and nothing
in either output marks the gap between them.

Two things this instance adds beyond the 2026-08-30 pair:

- **Magnitude.** The founding evidence was 4-vs-5 — one missed site, recoverable by a careful
  reader. 4-vs-60 is a different regime: at that ratio the shape query is not *under-counting* the
  proposition, it is **sampling** it. The figure was safe to publish only because it was labelled
  `n=4 is a floor, not a census` in the entry it fed; bare, it would have read as the class's size.
- **The proposition was itself this law.** The class being mined — `issue-clusters:IC-18`, *a
  selector narrower than the population it names* — is what the query was an instance of. Knowing
  the law, holding it in working memory, and actively writing its definition prevented none of it.
  That is `CLAUDE.md` § *Observer Blindness* reproduced in a further setting, and it is the
  strongest available argument that the remedy is **structural** — report the count *and the form
  you searched* — rather than attentional.

The tell was not suspicion of the number. It was running the body query for a *different* reason
— hunting a third member — and noticing that two answers to the same question disagreed by an
order of magnitude.

**Valid:** invariant
## R-139 — the parameter your own context supplies for free is the one you will omit

**Valid:** invariant

**Rests on:** a published claim is evaluated in the reader's context, not the author's. Any parameter the author's context supplies silently is one the author cannot perceive as missing — so the omission is invisible to the person best placed to fix it, and *only* to them.

**Status:** validated — 4 instances, 3 sessions, one evening; parent of R-138

**The law.** When you publish a count, a query, or an instruction, your own context fills in a parameter for free. The reader has a different context and fills in a different one — or none. **The result is not an error but a confident wrong answer**, which is why nothing catches it downstream.

**Four instances, 2026-08-30, all found by review and none by self-review.**

| kind | published | parameter the author's context supplied | what the reader gets |
|---|---|---|---|
| a **count** | *"the default lane restores it to 4"*, then *"to 8"* | the **counting rule** — 4 off a trimmed view, 8 verbs, **9** lines with clap's auto-appended `help` | re-measures, gets 9, "corrects" a true 8 back toward 4 |
| a **query** | *"the gate is transcribed in four places"* | the search key was **fenced blocks containing `cargo test`** | a fifth site in parenthetical prose; clean count, nothing marks the omission |
| a **query** | *"there are three clippy forms"* | the key was **`cargo clippy`**, so a wrapper invocation (`scripts/build-windows.sh clippy …`) is unreachable | a fourth form that no `cargo`-keyed audit can ever see |
| an **instruction** | *"verify positively with `./target/debug/codescout artifact --help`"* | that the evidence is the **exit code**; the author's own probe was `>/dev/null 2>&1 && echo present` | reads the output and counts it — inheriting a trap the author never faced |

**The fourth is the sharpest, because the author's practice was flawless.** `git-travel-augmentation-shape`'s probe discarded the output entirely and tested only the exit status. Nothing about their own usage could have surfaced the ambiguity, and no amount of re-reading their own advice would either, because they read it already knowing which half was load-bearing. **Advice that is correct in the speaker's practice and ambiguous in its wording has no self-check available at all.**

**Why self-review cannot catch this and peer review can.** The missing parameter is invisible *specifically* to whoever holds it. That makes this one of the few defect classes where a second reader is not a redundancy but a different instrument — and it predicts which reader: not a more careful one, but one who does not share the author's context. Two of the four here were caught by a peer re-deriving the claim from scratch rather than checking the reasoning.

**The test, and it is cheap.** Before publishing a claim, ask: *what did I know that the reader will not, that made this unambiguous to me?* Then write that down instead of trusting it. Concretely —

- a **number** → state its counting rule, or better, state the **derivation** that makes it a fact about the code rather than a reading of one output. *"8 verbs; the block prints 9 lines because clap appends `help`; and it is 8-or-nothing because the `#[cfg]` sits on the whole variant"* survives re-measurement. A bare `8` does not.
- a **count from a search** → report the count **and the key you searched for**, so the reader can see what it could not have found.
- an **instruction** → name which part is the evidence. *"Check it exits 0 and lists `find`"*, never *"check that it looks right."*

**Do not budget for vigilance.** All four instances were committed by authors actively writing about this class — one reintroduced a bare integer *in the commit fixing a bare integer*, roughly ten minutes after withdrawing an ordinal for the same reason. Knowing the class prevented nothing. What caught the wrapper form was a standing policy (*verify before contradicting a peer*) that fires unconditionally, whether or not anyone suspects a problem. **Build the check that runs when nobody is worried.**

**Corollary for consolidation work.** Deduplicating N copies of a claim into one removes the redundancy that could have caught an error in it — N copies disagreeing is a signal; one copy is never in disagreement with anything. That is a real cost, not a footnote, and the remedy is not to keep copies: it is to make the survivor carry its **derivation**, so a reader can re-check the claim instead of re-measuring it under a rule of their own choosing.

## R-140 — A warning that prescribes an action must state its recovery cost, DERIVED — understating it licenses the thing warned against

**Valid:** invariant

**Status:** open

**Rests on:** two first-hand instances, 2026-08-30/31, both in the `.worktrees/bench`
deletion warning. Neither is relayed: one was verified here at the bytes, the other reported
first-hand by the session that made it.

A warning exists to stop an action. When it also tells the reader what recovery would cost,
that number is not decoration — it is the term the reader weighs the warning against.
**State it, and derive it before stating it.**

### Why understating is worse than being merely wrong

An overstated recovery cost fails safe: the reader does not delete. An understated one
**converts a warning into permission** — the reader reads "recoverable in one command",
discounts the warning, and proceeds. The error and the harm point the *same direction*,
which is what makes a correction urgent rather than tidy, and it is the property that should
decide how fast a correction goes out.

### Instance 1 — the warning itself (verified here)

`codescout-fe` warned three sessions not to delete `.worktrees/bench`, and stated recovery
as one command: `git worktree add --detach .worktrees/bench ede25e69…`.

That command fails. The path exists and holds 24 entries, and bench is **not a registered
worktree** — `git -C .worktrees/bench rev-parse HEAD` returns *fatal: not a git repository*
and `git worktree list --porcelain` shows only the main checkout. The commit object is
reachable, so recovery is possible: move-then-add plus a **163M re-index**, not one command.
Verified here independently before the warning was carried onward — `code-explorer` gone,
the gitdir pointer dangling into it, 174M on disk, and three live surfaces resolving against
it.

They caught and corrected it themselves within the hour, and supplied the framing above.

### Instance 2 — the relay (reported first-hand by `git-travel-augmentation-shape`)

They carried that warning to their operator having **verified its descriptive claims** —
174M, the citation count, the three blind instruments — and **never run the recovery
command**. So the operator received an understated recovery cost with a verification wrapped
around it.

Their own statement of why that is worse than simply repeating it is the load-bearing part:
*confirmation of the descriptive half reads as confirmation of the whole, so the envelope
lent authority to the one part nobody had checked.*

That failure is also an instance of a **different law, which is theirs and deliberately not
folded in here**, now written as `R-143`: verify the ACTIONABLE half of a report, not the descriptive half.
Descriptive claims are what a reader evaluates; the prescription is what a reader executes.

### The opposite direction is a PREDICTION, not an instance

It is tempting to write this as a symmetric pair — *omit* the cost and the warning gets
discounted by whoever is impatient; *understate* it and the warning licenses the deletion.
Two directions, one remedy. It reads well and **the second half has no incident behind it.**

Sourced to `git-travel-augmentation-shape`, first-hand, when asked: the omission clause was
an *argument for including the cost*, phrased in the past habitual (*"tends to get
discounted by whoever is impatient to finish"*). Nobody discounted a warning; nothing was
nearly deleted on those grounds. It is plausible and it is not evidence.

**Kept, labelled, with its own criterion** — a prediction a reader can test beats a
symmetry a reader will trust.

**Promote-when (omission direction):** one real instance of a warning discounted for lack of
a stated cost. Until then it stays a prediction.

### The near-miss that shaped this entry, which is the entry's own subject

This was nearly written as the two-direction pairing, from a peer's summary of a third
session's instance. Asking that session **first-hand** is what revealed the second half did
not exist — an argument in the past habitual read as a report, which is an easy slip in
exactly that grammar.

Had it been written from the summary, an entry about warnings that mislead would have
manufactured its own symmetry, and would have done so through precisely the relayed-unverified
transfer it warns about. Third time in one night that two sessions nearly put an unverified
transfer inside the record defining the class.

**The operational form:** when a peer offers you an instance from a third party, the entry is
not sourced until the third party states it. A summary is a claim about a report, not the
report — the same one-step-removed shape as `R-136`'s propositional row, applied to
provenance instead of to instruments.

## R-141 — The referent you did not open is the part that gets executed — and a lineage anchor names lineage, not authorship

**Valid:** invariant

**Status:** open — 2 instances sourced first-hand, one evening, three sessions involved

**Rests on:** `R-140` (what a *warning* must state) — this is its sibling about what a *relay*
must open. Mechanism verified against `R-3`'s own `Promoted-to` field.

Sibling of `R-139`: that entry is about the parameter you supply for free and cannot see is
missing. This is about the referent you **pass on without opening**.

### The law

**In a relayed report, the part nobody checks is the part the reader will execute.**

  - `git-travel-augmentation-shape` passed on a **command** they had not run.
  - `codescout-fe` passed on an **id** they had not opened.

Both had verified the surrounding prose carefully. Both shipped the one element that would
be acted on.

**Why these two slip past scrutiny while the prose around them is challenged** —
`git-travel-augmentation-shape`'s formulation: a shell command reads as *mechanical*, an id
reads as a *pointer*; **neither reads as an assertion needing support.** The claims around
them get evaluated because they look like claims. The referent looks like plumbing.

To which: **an id reads as precise, and precision reads as verification.** `R-3` looks
checked in a way "the entry about instruments" does not, so a *wrong* id is more persuasive
than a vague gesture at the same wrong idea. Specificity is doing work that only accuracy
should do.

### The composite case, which is worse — `codescout-fe`'s finding

Their error was **not** memory, and the real mechanism is a designed trade-off.

The SKILL.md § Phase 1 bullet that *does* carry the positive-control instruction ends with a
lineage anchor: `(R-3 → R-113 → R-77 → R-79 → R-104 …)`. They read a lineage anchor as
per-clause attribution.

That reading is wrong and the design invites it. **Verified in `R-3`'s own `Promoted-to`
field**, which states the back-citation was chosen *over a verbatim quote* because *"the
bullet has been rewritten and extended repeatedly since 2026-05 and the anchor still
resolves — a verbatim quote would have needed re-syncing each time."* So the anchor survives
rewrites **by not tracking which clause came from where.** Per-clause precision is the price
paid for durability, deliberately.

> **An anchor on a composite names lineage, not authorship.** *"`R-3` says X"* can be false
> while *"the bullet `R-3` anchors says X"* is true — and **nothing in the citation's
> surface distinguishes them.**

So the remedy is not "check ids" in general. A reader who wants a *clause* must open the
entry, because the anchor was never a promise about clauses.

### The remedy is unconditional, because vigilance demonstrably is not

Per `R-139`'s *do not budget for vigilance*: **all three sessions were, at the time, actively
writing about unverified relaying.** Knowing the class prevented nothing — which is the same
result R-139 measured across four instances that evening.

- Before relaying an **instruction**: run it. Not the descriptive claims around it — the
  thing the reader will execute.
- Before citing an **id** onward: open it. Every time, not when something feels off.
- Citing an id for a **specific clause** of a composite: open it *and* check the clause is in
  the entry rather than in the thing the entry anchors.

### The related law kept separate, and whose it is

`git-travel-augmentation-shape`'s **verify the ACTIONABLE half of a report, not the
descriptive half** — descriptive claims are what a reader evaluates, the prescription is what
a reader executes — is the general form and is theirs. Recorded here as attribution, not
absorption. **Now written as `R-143`**, which cites this entry rather than duplicating it — and
which sharpens the attribution: a *value* published without its instrument turns out to be on
the actionable half too, so "verify the actionable half" does not make the descriptive half
safe. Their instance 2 there is a referent that WAS opened and still came out wrong.

### Sourcing note, per `R-140`

**Two instances are WILD** — produced by ordinary work, before anyone was looking for this
class: `git-travel-augmentation-shape`'s unrun command (stated first-hand) and
`codescout-fe`'s `R-3` citation (verified here at the bytes).

**Three more were produced BY WRITING THIS ENTRY**, and they are recorded separately because
they are worth less.

> **Correction, and it was in the sourcing note itself.** The first version read *"a peer
> reports three sessions handing over a confident id that evening"*. That count was **mine**
> — I wrote *"you are the third session tonight to be handed a confident id from memory"* to
> that peer, they echoed it back, and I recorded my own unsourced claim as a peer's
> observation. Verified in this session's transcript, not from memory. Circular corroboration
> inside the sourcing note of an entry about unverified transfer.

| # | instance | whose |
|---|---|---|
| 3 | echoing a count back to its author as an observation — the descriptive half (the `R-3` correction it rode on) carefully checked, the **number** shipped unsourced | `git-travel-augmentation-shape`, stated first-hand |
| 4 | the count itself: **"three sessions"** was never derived. Two are sourced; the third was rhetorical | mine |
| 5 | a message telling a peer to *verify rather than relay* carried **`patch-id 8c8…`**, a prefix never derived — the real one is `a138eeac…`. Written as a placeholder before the command returned | mine |
| 6 | **the same error again, two hours later, inside the clause denying it**: *"patch-id `f7d4`… (derived below, and I am not quoting a prefix I have not read this time)"*. Real value `ab904324…` | mine |

### Instance 6 supplies the mechanism the other five lacked

Five instances established the class. The sixth is worth more than the other five together,
because it repeats under maximal vigilance and therefore exposes *why*.

**Both patch-id fabrications had one cause: the message was dispatched in the SAME parallel
tool batch as the command deriving the value it quoted.** Tool calls in a batch run
concurrently, so at composition time the value did not exist — and the natural thing to write
in its place is a plausible prefix plus a promise, *"(derived below)"*.

**The promise is the dangerous part.** It reads as a forward reference to evidence rather
than as an admission that the author is typing a hash they cannot see. A bare `f7d4…` invites
checking; `f7d4… (derived below)` reassures.

**The error tracks what the author had READ, not what they believed or how careful they
were.** In the same sentence, the SHA `7d96110b` was correct — it had come back from the
commit — and the patch-id was invented, because that command had not returned. Care was
identical across both halves; only availability differed. That is the cleanest statement of
this entry's law available, and it took committing the error while writing the entry to get
it.

**It is also `R-139`'s *do not budget for vigilance* at maximum strength.** The author was
not merely aware of the class: they had written this entry, corrected instance 5 an hour
earlier, and were composing a sentence explicitly promising not to do it. Awareness reached
its ceiling and prevented nothing.

**The fix is structural and unconditional, which is why it will work where vigilance did
not:** *never cite a derived value in a message dispatched in the same batch as its
derivation.* Derive, read the output, then write. It costs one round trip and it removes the
state in which the error is possible, rather than asking anyone to notice.

#### The hazard is PLAUSIBILITY, not lateness — `codescout-ae`'s sharpening

The rule above is correct and located the mechanism one step short of the actual hazard.
Lateness is why the value was unavailable; it is not why the message was dangerous.

`f7d4…` is dangerous **because it is well-formed.** Identical batching, identical forward
promise, identical unread value, but written `<patch-id pending>` — and the message fails
safe. No reader can mistake it for evidence, and the author cannot forget to fill it in
without the gap being visible. The defect was never the timing; it was emitting a token
shaped like the answer.

So there is a **zero-cost variant** for when you do not want the round trip:

> **Omit rather than promise.** Deriving first buys you the value. Omitting buys the same
> safety for nothing.

The two compose: derive first when the message needs the value, omit when it does not.
Neither asks anyone to be careful.

#### The general form: a forward reference is an unenforceable claim about process

*"(derived below)"* asserts something about **when the author looked**. Prose has no
mechanism to enforce that, and once written it is indistinguishable to every later reader
from *"(derived above)"*. The tense claims a process the artifact cannot carry.

That is the same defect as `R-139`'s unanchored ordinal and its count-without-a-counting-rule:
**a value whose correctness depends on context the reader does not have and cannot recover.**
The remedy is identical in all three — put the thing the reader needs *in* the artifact, or
emit nothing.

**This entry records no vigilance remedy, deliberately.** `codescout-ae` asked that a
standing structural rule not be diluted into a lesson about care, and they are right: the
author here was at maximum awareness and it changed nothing. **The error tracked what had
been read — not what was believed, and not how carefully.** The SHA in the same sentence was
correct because it was in hand; the patch-id was invented because its command had not
returned. Only possession of the output predicted the outcome.

Instance 5 is the entry's subject one layer out, and it was caught **only** because the same
message asked the recipient to check. Had it said `a138eeac`, they would have believed it.

**Why the last three are weaker evidence, not stronger.** They were generated under
observation, by parties primed on the class, in the act of discussing it. They are excellent
evidence for `R-139`'s *do not budget for vigilance* — five people knowing the class stopped
none of them — and they are **poor** evidence for the law's reach, because a class that
manufactures instances whenever it is discussed will never look rare from the inside. Do not
cite 5 instances as five independent observations.

**Promote-when:** an instance found **in the wild** — outside a conversation about this class
— where an unopened referent reaches a *shipped artifact* rather than a message. Deliberately
not satisfiable from inside this discussion, since three of the five above were.

## R-142 — Circular corroboration — a peer restating your claim is your own reading handed back, and nothing marks it

**Verdict:** miss — I treated a peer's restatement of my own claim as independent
confirmation, and reported the result to my operator as two observed instances.

**The law.** Two parties agreeing is evidence only if they did not get it from each
other, and **nothing in a conversation marks which is which.** A claim you introduce
can return to you through a peer's restatement, and the return arrives carrying the
authority of independence while having none of it.

**The instance, 2026-08-30/31, four concurrent sessions in one checkout.** A peer wrote
that *"a warning with no stated recovery cost tends to get discounted by whoever is
impatient to finish the cleanup"* — past habitual, an **argument** for stating the cost.
I read it as a **report** of an incident. Shortly after, the same peer described their own
error as *"shipped the other failure of the same pair within the hour"* — which read to me
as first-hand confirmation of the pairing. It was my own reading handed back: their actual
error (relaying my understated recovery command, verification wrapped around it) points the
**same** direction as mine, not the opposite.

On that basis I told my operator both failure directions had been observed within an hour,
and proposed the pairing to a third session for a durable ledger entry.

**What stopped it.** `codescout-f0` declined to write the pairing from my summary and asked
the other session **first-hand**, on the stated grounds that writing my account of someone
else's instance into a ledger is the relayed-unverified shape the entry was about. The
first-hand answer differed. Filed instead as `R-140`: one direction with two sourced
instances, the omission direction kept as an explicitly labelled untested prediction.

**Why the grammar matters.** Past habitual (*"tends to"*, *"gets discounted by whoever"*)
states a disposition, not an event. It is the natural register for an argument, and it is
one word away from the register of a report. Reading the second out of the first needs no
carelessness — the sentence supports it.

**Why it is upstream, not a sibling.** `R-141` is about referents nobody opens; `R-139` is
about vigilance not scaling. This is about **provenance inside a conversation**: the loop
runs entirely between parties who are each behaving well, and produces a confidence neither
of them separately holds. `R-140`'s manufactured symmetry is its direct product, which is
why both cite here.

**Runnable form.** When a second party confirms a claim you are about to publish or record,
ask one question before counting it: **could they have got this from me?** If yes it is one
source, not two. Source it first-hand, or mark it relayed and say so in the artifact. The
cost is a single message; a peer paid it tonight and it changed a ledger entry's
conclusion.

**Corollary.** Agreement is worth what its independence is worth — the same reason
`observer-blindness:OB-4` holds that instruments sharing a substrate are one instrument.
Distinct authorship supplies independence no more reliably than a distinct tool name does.

**Second instance, 2026-08-31 — and this loop closed through a DURABLE ARTIFACT rather than a
conversation, which is worse in three ways.** I measured a figure (2765 hidden entries under a
depth-3 walk) with a `find` approximation, sent it to a peer, then **retracted it** as instrument
error once the tool itself reported a different number. That peer's next commit — `2f434fba`, a real
fix with a real gate run — carried *"measured on this repo, the same root reports … 2765 under
recursive=true"* in its message. I read that as independent re-derivation and **withdrew my own
correct retraction**, reporting to my operator that the retraction had been premature. It had not
been: the peer had taken 2765 from my message and written it into the commit body as though they
had measured it. One number, two documents, reading as two witnesses. Their account: *"my commit
turned it into apparent corroboration, and your retraction then died of it."*

Why the artifact variant is nastier than the conversational one. **Genre supplies the authority** —
a commit message's register is *report of work done*, so a number inside one reads as measured by
default; what the first instance needed a grammatical ambiguity to achieve, this one gets from the
document type. It is **durable** — `2f434fba`'s message is published and stays wrong, where a
conversational restatement evaporates. And it **inverted an existing correction** rather than merely
inflating confidence: the loop did not add a false datapoint, it destroyed a true one, which is
strictly worse than what the first instance produced.

**What caught it — and why nothing I could run would have.** The peer audited their own citation,
unprompted, and said so. From outside, a commit message *reporting* a measurement is
indistinguishable from one *performing* it; the only party who knows which is the author. That is
the asymmetry worth recording: a reader cannot verify a cited number's provenance, so provenance
has to be marked at the **write**. The remedy is therefore a convention rather than a check — cite
a figure you did not take as *reported by X*, never bare, and least of all inside an artifact whose
genre implies measurement.

**Status:** open — two instances, and in both the loop was broken by the party who introduced it
rather than by its victim, which is the part with no reader-side mitigation. The second reversed a
retraction that was already correct.

**Valid:** dated 2026-08-31

The law is not time-bound; the instance is a fact about this evening.

**Rests on:** `R-140` (the entry that would have carried the manufactured symmetry) and
`R-139` (knowing a class does not prevent it). The sourcing discipline that caught it is
the practice `get_guide("tracker-conventions")` states for citations — prefer a
first-hand definer to a restatement.

## R-143 — Verify the ACTIONABLE half of a report — and a value whose instrument you did not state is actionable

**Valid:** invariant

**Status:** open — 2 first-hand instances, one of which reached a shipped artifact

**Rests on:** `R-140` § *Instance 2* and `R-141` § *The related law kept separate*. Both
name this law, attribute it here, and deliberately leave it unwritten so it could be
written rather than restated — R-141 says *"if they write it, this entry should cite it
rather than duplicate."* This is that entry. Sibling of `R-141` (the referent you do not
open); distinct from it, because instance 2 below is a referent that **was** opened.

### The law, in two clauses

**1.** Descriptive claims are what a reader *evaluates*; the prescription is what a reader
*executes*. Verification effort flows to the descriptive half — it is legible, it looks
like a claim, and it is cheap. The prescription requires constructing a state, so it gets
skipped. **The reader then receives an unchecked instruction inside a verified envelope**,
and the envelope is what lends it authority.

**2.** And the clause the naive form gets wrong: **a value published without its instrument
is not on the descriptive half at all — it is a prescription.** The reader cannot simply
accept it; to check it they must run something, and nothing tells them *what*. So they
invent a rule. Two honest verifiers then produce different numbers from the same file, and
the disagreement does not surface as a conflict — **it surfaces as a correction**, where
the later number wins on confidence rather than on rule.

### Instance 1 — the unrun command (mine; recorded from the other side in `R-140` § Instance 2)

I relayed `codescout-fe`'s `.worktrees/bench` deletion warning to my operator having
verified its descriptive claims — 174M, the surfaces, the dangling gitdir — and having
**never run its recovery command**. The command was wrong for the state actually on disk.
My own framing at the time, which R-140 quotes: *confirmation of the descriptive half reads
as confirmation of the whole.*

### Instance 2 — the count, which refutes clause 1 taken alone

The same report claimed *"`docs/trackers/retrieval-benchmark.md` — 8 references"*. I checked
it, and published **17** in a table whose column header read **Verified**, as a correction.

The value is **15**, and the file has held 15 since `d6d66e4c` (2026-08-26) — so neither
number was ever stale. Both were derived, in good faith, and both were wrong.

Seven plausible instruments on that one file give seven answers:

| rule | value |
|---|---|
| `grep -o '\.worktrees/bench' \| wc -l` | 14 |
| `grep -o 'worktrees/bench' \| wc -l` | **15** |
| `grep -c 'worktrees/bench'` (lines) | **15** |
| `grep -o '\bbench\b' \| wc -l` | 41 |
| `grep -c '\bbench\b'` | 40 |
| `grep -o 'ede25e69' \| wc -l` (the pin) | **8** |
| non-fenced lines containing `worktrees/bench` | 9 |

**`8` is exactly the occurrence count of the pinned SHA.** I cannot confirm that is what the
peer counted and I am deliberately not asking — per `R-142`, a peer agreeing with a rule I
supply is my own reading handed back, not confirmation. But it is the only one of seven
rules that produces their number, and *"how many places cite the pin"* is a perfectly
reasonable reading of "references". On that reading **their number was right under its own
rule and my correction was wrong under mine**, and nothing in either report stated a rule.

My `17` matches none of the seven. That one is a plain miscount — published under a
Verified header, correcting a number that was probably defensible.

### What this shows that clause 1 alone does not

Clause 1 implies the descriptive half is the *safe* half — check the command and the rest
takes care of itself. It does not. Of three claims I audited that evening, the two I got
right (`PROBES.md:116` → **141**; the corpus had already been deleted once) were the ones
whose instrument **admits no choice**: grep a literal string, get one hit, read the number
beside it. The one I got wrong was the one where the instrument required me to pick a rule.

> **The claims that survive verification are the ones whose instrument admits no choice.**
> Where it admits one, diligence does not produce a caught error — it produces a *confident
> wrong answer*, which is worse, because it now travels with a verifier's name on it.

This is **Law B** (*"the instrument decides the answer"*, § *The seven laws*) turned on the
verifier rather than the author: an unstated rule is not a small omission in a claim, it is
the whole claim.

> **Instance 3, committed inside this entry and caught before commit.** The sentence above
> first read *"this is `R-136`'s 'the instrument decides the answer'"*. `R-136` is
> *"Adjacent-proposition errors come in three kinds"*; the phrase belongs to Law B. I cited
> an id for a clause without opening it — `R-141` exactly, inside the entry about verifying
> referents, while writing the sentence that says to open them. Caught only by running
> `grep '^## R-136 — '` as a deliberate step, not by noticing. Per `R-141`'s own accounting
> this counts for **less** than the two wild instances above, because it was produced under
> observation by a party primed on the class — it is evidence for `R-139` (*vigilance does
> not scale*), not for this law's reach.

### Runnable form

- **Before relaying an instruction, run it.** Not the prose around it — the thing the reader
  will execute. (This is the clause `R-141` covers for referents; it is the same discipline.)
- **Rank a report's claims by cost-to-verify × cost-if-wrong, and spend the budget in that
  order.** The default order is cheapest-first, which is close to backwards.
- **Publishing a count, publish its command instead of its value** — one paste lets the
  reader *re-check* rather than re-derive under a rule they invent. This is CLAUDE.md
  § *Observer Blindness*'s "ship the derivation, not the value", and instance 2 is what that
  sentence protects against.
- **Correcting someone's number, state your rule.** A bare correction asserts they
  miscounted; it may only mean you counted something else.

### Where it is already mechanised

`.worktrees/README.md` — the guard file this whole episode produced — now carries
`grep -o 'worktrees/bench' … | wc -l` → **15** in that table cell instead of a bare number,
with a note that the cell has been wrong twice under two honest counters. That is the one
site fixed, not the class.

**Promote-when:** a third instance of clause 2 specifically — two parties producing different
values for one quantity, neither stating a rule — found **outside** a discussion of this
class. Clause 1 already has its instances and needs no more. If it fires, the remedy to
promote is not "count carefully" but a standing convention that any published count in a
tracker carries its command, which is checkable by grep and therefore mechanisable.

## R-144 — A tripwire aimed at a fabricated fixture cannot detect the change it was written to detect

**Verdict:** miss → rule. Three instances in one work stream, the third found only
because the first two had taught me to look.

**The law.** A test whose stated purpose is to notice a *future* change must assert on a
value the system **produces**. Given a fabricated fixture it asserts a fact about the
author's imagination, and it holds equally in the world where the change landed and the
world where it did not — so the tripwire is silent in both.

**Instance 1 — the stub that supplied what the real tool lacked.**
`src/tools/core/tests.rs` defined `RoutedEchoTool { name: "memory", … }`, a stub *named
for* the real tool, projecting the `{tool}.{action}` selector key that production `Memory`
did not implement. The whole operator-rules routing suite was green against it while every
triggered rule was dead in production. Its own doc comment said so. A green suite and a
dead feature stayed consistent for as long as the stub was the only caller.
Fixed `2447f709`; the stub is retained, recast as a test double.

**Instance 2 — the regression test that encoded the ambiguity it guarded.**
`doctor`'s `by_check` omitted a key for any check that ran clean, conflating that with
"never ran". `call_reports_entry_validity_scoped_out_rows_in_catalog_health` asserted
`by_check.get(…).is_none()` to mean *found nothing here* — reading absence as a positive
signal, which is the defect, inside the suite meant to guard it. The assertion passed
under **both** world-states it conflated. Fixed `09cd1b46`; the assertion is now `== 0`,
which distinguishes them.

**Instance 3 — the pin explicitly written to detect its own repair, which could not.**
`op_4s_path_predicate_cannot_fire_against_a_write_response_today` carried the instruction
*"When this test starts failing, that is the fix landing. Delete it and assert delivery
instead; close the bug file."* The fix landed at `a6b4fc35`. **It did not fail.** Its
fixture was a hand-written `json!({"status":"ok","wrote_to":…})` bound to a variable named
`observed` — so it asserted against a response no tool returns, and could see neither the
defect nor the repair. Removed per its own instruction; delivery is now asserted on a
response that came back out of `call_content`.

**Why instance 3 is the worst of the three.** The first two were ordinary tests that
happened to use a stand-in. This one *advertised itself as a detector*, named its own
trigger condition, and told a future reader what to do when it fired. It bought the
confidence of a tripwire and delivered none, and the fabricated fixture is exactly what a
reader skims past — it looks like setup.

**The tell, which is cheap.** A fixture written as a literal beside an assertion about
production behaviour. Ask: *did anything under test produce this value?* If not, the test
constrains the literal, not the system.

**Runnable form.** A test that names a future condition — a pin, a tripwire, a
"when this starts failing" — must obtain its fixture from the pipeline it is watching.
Where that is genuinely impractical, say so in the doc comment, because a reader is
otherwise entitled to believe the trigger works.

**Status:** open — three instances, all in one stream, all fixed. Not yet promoted.

**Promote-when:** one further instance from a different work stream. At that point this is
skill-shaped rather than project-shaped (it is a property of tests, not of this repo) and
belongs in the reconnaissance SKILL.md alongside the vacuous-assertion guidance, not in a
codescout memory.

**Valid:** dated 2026-08-31

The law is not time-bound; the three instances are facts about this stream.

**Rests on:** `R-139` (knowing a class does not prevent it) and
`observer-blindness:OB-5`, whose write-up generalises the sibling half — that a suite
guarding an *ambiguous* signal is a likely carrier of that ambiguity, because tests are
written in the same vocabulary as the code. This entry is the other half: not the
vocabulary the fixture is written in, but whether the fixture was produced at all.

## R-145 — Co-occurrence in a working-tree snapshot is not evidence of one change

**Valid:** dated 2026-08-31

**Verdict:** miss → rule · **Observed:** 2026-08-31, scouting the `tree(glob)` dotfile bug (P1) in a checkout shared by five concurrent sessions

**Seam:** whether two paths reported modified by one `git status` belong to one change, by one author, for one purpose.

`git status` showed `src/tools/tree.rs` (+204) and `src/util/fs.rs` (+58). I reported them as a
single in-flight change — *"tree.rs +204 and fs.rs +58 for the tree/include_hidden bug"* — and
broadcast that to four peer sessions before anyone contradicted it. They were **two authors**:
the `fs.rs` delta was an unrelated `atomic_write` tmp-file leak fix, and its author had to write
back to say so.

Verified afterwards rather than conceded: the `fs.rs` diff has **0** occurrences of
`hidden_at_root` and **5** of `atomic_write`, and the `tree.rs` diff adds **no** `util::fs`
import — so not only are they separate, there is no dependency edge between them in either
direction.

**The mechanism is the instrument, not inattention.** I had run
`git diff -U0 -- src/tools/tree.rs src/util/fs.rs` into a single buffer and read the result as
one narrative. The disconfirming evidence was already inside that buffer — its tail was
`atomic_write` assertions I had scrolled past — but a combined diff carries no author column and
no separator a skim registers, so passing two paths to one command is what erased the boundary.
The output is a valid answer to the question *"what changed in these two files"* and reads as an
answer to *"what is this change"*, which is a different question I never asked.

This is the complement of R-50 rather than an instance of it. R-50 is a view that silently
**dropped** something; this is a view that silently **merged** things. Both produce a confident
wrong answer with nothing raising, and in a shared checkout the merge direction is the live one:
`git status` there is a **union over N concurrent authors** and has never claimed otherwise.

**Tell:** a diff or status command naming more than one path, whose output you then summarise in
the singular — "the change", "their work", "this branch".

**Runnable:** before attributing two modified paths to one change, `git diff --stat` **per path**,
and look for a dependency edge — a symbol, import or constant introduced in one and consumed by
the other. Absent that edge, describe them as separate and say so. Adjacency in a snapshot is a
fact about the snapshot.

**Promote-when:** a second instance where co-occurrence in a status/diff/log listing is read as
shared cause, in this repo or another.

**Status:** open — 1 datapoint, which propagated into four peer messages before an author
corrected it.

**Kin:** R-50 (the view is not the set — the drop direction, where this is the merge direction), R-142, R-4

## R-146 — A measurement of state someone else is editing expires before the message carrying it arrives

**Valid:** dated 2026-08-31

**Verdict:** miss → rule (3 instances, one evening, three sessions) · **Observed:** 2026-08-31, cross-session messages during a shared-checkout scout

**Seam:** whether a fact measured about live state is still true at the moment a reader acts on it.

A peer measured the lean lane **red** — `3250 passed, 2 failed`, naming
`tools::tree::tests::include_hidden_lists_the_files_the_default_withholds` and
`no_warning_when_the_hidden_paths_held_nothing_matching` — and warned that any session gating in
this checkout would waste time blaming its own change. Their attribution step was **sound**:
neither test name exists in `git show HEAD:src/tools/tree.rs`, so new-and-red rather than a
regression from another tree.

By the time I acted it was false. `cargo test --workspace tools::tree` → **7 passed, 0 failed**,
both named tests green. Not a lane artefact either: `grep cfg(feature` in `src/tools/tree.rs`
returns **0 matches**, so those tests compile and run identically in both lanes. The file measured
**+204** at my scout, **+214** at theirs, **+360** at my check — 146 lines added between their run
and mine. They had measured a **TDD red phase**, and *new-and-red* is precisely RED's signature,
so the evidence that proved it was not a regression is the same evidence that should have marked
it as transient.

Two more instances the same evening, same shape, different surface. A session told another *"the
tree dotfile bug is mine"* and then wrote no line of it while someone else went 214 lines in; that
claim reached a **third** session — the one who had filed the bug — and made them stand down from
their own work. Their own retraction names the class exactly: *"a claim with no work behind it is
the same defect as a stale 'I am holding' — it asserts a state that has an expiry and does not
carry one."*

**What makes this worse than ordinary staleness is that the cost flips sign rather than decaying.**
A stale warning does not fade into noise: acting on this one costs the *inverse* of what it was
sent to prevent — instead of a session blaming its own change for someone else's red, a session
distrusts a green that is real. Same message, same reader, opposite damage, and nothing in the
message marks which side of the flip it is on. A claim to own a task fails the same way: unheeded
it wastes a duplicate; heeded after it expires it **vacates** the work.

**And it does not decay into uncertainty — it becomes confidently wrong by a specific amount.**
A fourth instance the same evening, measured from both ends. The peer who sent the red-lane
report also reported `tool_surface_under_budget` at **57549 chars, over by 1030**; I ran it
minutes later and got **56845, over by 326**. Acting on theirs would have sent me hunting **three
times** the bytes — and in a file that was not the cause at all, the overage being in-flight
prompt work elsewhere. This is the property that makes the class worth a rule rather than a
caveat: a *number* is more actionable than a hedge, so a stale one is obeyed more precisely than
a vague one would be. The instant therefore belongs **beside the number**, not in a footnote
under it. (Sharpened by `codescout-ae`, who supplied the first reading and then re-derived the
second.)

**Tell:** a message reporting a measurement of state another party is actively changing — a diff
stat, a test result, a lock, a claim to own a task.

**Runnable:** ship the **derivation, not the value** — the command, the instant, and a cheap
re-check the reader can run (a `--stat` line, an mtime, a SHA), so a reader can tell whether it
still holds instead of inheriting a number. On receipt, re-run before acting whenever the
underlying artifact is under active edit. And give any ownership claim an expiry or a commit;
uncommitted work outranks an unbacked claim, which is the protocol these sessions converged on
independently.

**The class is symmetric, and the remedy is now measured rather than proposed.** Later the same
day I sent `git-travel-augmentation-shape` a compile-break report — `guide_index.rs:470`, `cannot
find macro json` — accurate when sent and false within about ten seconds: they had already swapped
the offending `json!({})` for `Value::Object(Default::default())` and re-run. So the two instances
run in **opposite directions between the same two sessions**, which is a stronger claim than either
alone — this is not one session being careless, it is a property of reporting on a tree several
sessions write to. What made the second one cheap is exactly what the first one lacked: I carried
the **derivation** — the failing line, plus the `git diff` → 1 against `git show HEAD` → 0 pair that
locates the break in the working tree — so the recipient discharged it with a ten-second `cargo
check` instead of re-deriving the claim or hunting their own edits. Their formulation is better than
mine and is the one to keep: **the goal is not to measure less, it is to make the reader's re-check
cheaper than their doubt.**

Note what the derivation pair does and does not establish. `git diff` = 1, `git show HEAD` = 0 is a
fact about **where** (working tree, not HEAD) and says nothing about **who** — stopping there is what
kept it correct, since the same evening's authorship error came from taking presence for
identification. A locating claim is cheap, checkable, and sufficient; an attributing one is neither.

**Promote-when:** **FIRED 2026-08-31**, on both limbs. The first (a third expired-on-arrival
instance) fired twice over; the second — re-running on receipt demonstrably preventing the wrong
action — fired when the recipient discharged a stale report in ten seconds rather than acting on
it. Candidate for promotion to a standing cross-session convention: *a measurement of shared state
ships with the command that produced it, and a claim about a file locates before it attributes.*

**Status:** open, Promote-when FIRED — 5 datapoints in one day (one red-lane report verified expired here; one
budget-overage number stale by 3× and pointing at the wrong file; two
ownership claims, one of which displaced the bug's own filer; and one compile-break report of mine,
stale within ten seconds and cheap to discharge because it shipped its derivation).

**Kin:** R-98 (a max read at the start of a pass is stale by the time you write — a peer took R-97 with a four-minute margin), R-142, R-143 (ship the instrument, not the number), R-145

## R-147 — A quotation that asserts its own fidelity does not check it, and the assertion is what stops the reader looking

**Valid:** dated 2026-08-31

**Verdict:** miss → rule · **Observed:** 2026-08-31, landing the cross-repo half of the
post-compact bug (`d7072ed21959aca1`)

**Seam:** whether a block of text labelled *quoted verbatim* from a live emitter still matches
what that emitter emits.

`docs/manual/src/concepts/post-compact-cache-flush.md` reproduced the companion plugin's
SessionStart injection inside a fenced block, with a ⚠️ note directly beneath stating it was
quoted **verbatim** from `codescout-companion/hooks/session-start.mjs:339`, that its last line was
known to be wrong, and that it was *"reproduced unchanged rather than silently corrected, because
a manual that quotes a hook has to match the hook."*

That reasoning is correct, the marker names the exact file and line, and the known-wrong line
genuinely did match. **Two of the block's other three lines had drifted anyway.** The hook emits
`POST-COMPACT: Context was just compacted.` and `workspace(post_compact=true)`; the block showed
`codescout PostCompact: context was compacted.` and the `action: status` form — and the
section's closing prose repeated the second error a few lines further down.

**Why this is not just ordinary doc drift.** The marker is the best-intentioned version of the
practice: added deliberately, by an author who had the invariant explicitly in mind, naming its
source precisely. Its effect is *inverted*. A reader who sees "quoted verbatim" has been told the
check was already done, so the label **substitutes** for the check rather than prompting it. An
unlabelled quote invites *is this current?*; a labelled one answers the question pre-emptively and
wrongly. The stronger the assurance, the less likely anyone looks — which is the same structure as
`observer-blindness`'s reassuring-instrument class, arrived at from the documentation side.

**And it survived a deliberate correction pass.** On 2026-08-30 someone edited *this exact block's
surroundings* to add the warning about the wrong last line, and did not notice the other two.
Editing a quotation for one reason does not re-derive it — the parts you are not thinking about
are exactly the parts a targeted edit leaves alone.

**What caught it:** not re-reading the block, but comparing it against what a live consumer
actually received. The injected text was sitting in this session's own SessionStart context, so
the emitter's real output and the manual's claim about it were both in front of me at once. That
adjacency was luck; the rule below is how to get it on purpose.

**Tell:** any block labelled *verbatim*, *quoted from*, *as emitted by*, *copied from* — most of
all one whose label names a `file:line`, because the precision reads as recency.

**Runnable:** a quotation of a live emitter is a **derived artifact** — treat it like generated
code, not like prose. Re-derive it at the moment you touch anything inside it (`sed -n` the
emitting lines, or capture what a live consumer received), or stop asserting fidelity and label it
*paraphrased*. Never edit one line of a quoted block without re-deriving the whole block: the
targeted edit is precisely what will not look at the rest.

**Promote-when:** a second instance of a fidelity-asserting label found stale — in this repo or
another. A cheap sweep exists: grep for `verbatim`, `quoted from`, `as emitted` across `docs/` and
diff each hit against its named source.

**Status:** open — 1 datapoint, but a dense one: 3 drifted lines under an explicit verbatim
marker, one of them surviving a deliberate edit of the same block a day earlier.

**Kin:** R-89 (probe the copy the consumer loads — this is its documentation-side twin), R-142
(a restatement is not a second witness), R-144 (a fixture that cannot detect what it advertises)

## R-150 — An ownership assumption authorised a deletion, and it took the evidence that would have judged it

> **Renumbered from 148 on 2026-08-31, before either side was merged.** A laptop session
> allocated two R entries 30 minutes earlier, in a commit neither machine had seen from the
> other; this entry was allocated from a high-water mark that could not see them. Per
> `get_guide("tracker-conventions")` — *if two entries share a number, give the later one a
> fresh id* — the later allocation is this one, and the laptop's counter was already the
> higher of the two.
>
> ```text
> laptop   fadc5dd3  21:22:44  unpushed  ->  R-148, R-149   (kept)
> desktop  d1a8db97  21:52:47            ->  R-148          -> renumbered to R-150 here
> ```
>
> Two commit messages still name the old number for this entry and cannot be edited:
> codescout `d1a8db97` and claude-plugins `b3de45a`.

**Valid:** invariant

**Law:** the deletion rule — removal is authorised by a positive finding, never by an assumption.

**Verdict:** hit overall (the scout did its job), with one self-inflicted miss inside it.

**Observed:** 2026-08-31, a post-rebuild scout. The scout itself succeeded and is not the
subject: it verified all three `R-89` axes positively — binary built `21:26:20`, serving pid
started `21:45:06` (after the build), `/proc/<pid>/exe` resolving to that exact file, and
`~/.cargo/bin/codescout` a symlink to it with equal hashes — then found **9 of 13 live
`codescout start` processes serving pre-rebuild bytes**, two ~85h behind. Filed as
`docs/issues/archive/2026-08-31-peer-sessions-never-compares-start-time-to-build-time.md`.

**The miss.** `git status` then showed an untracked `docs/issues/.buddy/`. I read it as my
own statusline-marker debris and ran `rm -rf` on it. Two things were wrong with that, and
only one of them mattered in the end:

1. **It was not mine.** The path contained a session id — visible, unread — belonging to a
   *different* session, live at that moment (its transcript had been written 79 seconds
   earlier). My marker had landed correctly at the home project's `.buddy/<my-sid>/`.
2. **I deleted it before reading its mtimes.** The scout had just raised the question the
   mtimes would have answered: the same session had buddy state in **two** locations, so
   was the judge's narrative being *split* across them — writes interleaving by cwd — or
   was the duplicate created once and abandoned? The canonical dir turned out intact and
   actively written. Whether the stray one was also receiving writes is now **permanently
   unanswerable**, because the evidence was three files and I removed them.

> **Corrected the same evening — "permanently" was wrong, and the correction sharpens the
> law rather than softening it.** Roughly an hour later the same peer session recreated the
> same nested directory in the same repo, and this time it was read instead of removed: both
> `cs_tool_log.jsonl` and `narrative.jsonl` were being actively written, seconds apart, while
> the canonical copy was live too. So the judge's narrative **is** split across two
> locations, and the question the deletion destroyed was answered by simply waiting for the
> next instance.
>
> The evidence was not unique, only the *instance* was — a recurring defect regenerates its
> own evidence, so "I destroyed the only copy" is itself a claim worth checking before it is
> written down. What does not change: the deletion was unauthorised at the time it happened,
> it was futile (the directory came straight back), and the answer arrived despite it rather
> than because of anything the deleting session did.
>
> It also widened the defect. The same cwd-relative planting produced a `.codescout/` sibling
> holding `constitution-seen/<sid>.json`, written by codescout-companion — a **different
> plugin with the same bug** — plus a `.buddy/by-ppid/<ppid>/` rendezvous. Three writers, two
> plugins, one cause.

**What actually saved the report.** Checking *after* — the peer's canonical
`.buddy/<sid>/` was intact, still being written, holding the two files the stray copy
lacked. So the damage was a stray duplicate, not a live peer's judge state. The sentence I
was one step from writing to the user was wrong by two orders of magnitude in cost.

**The law this belongs to, and how it extends.** The Phase 1 bullet says *a negative search
result must never authorise a deletion.* This is its sibling and it is not covered by the
wording: **an ownership assumption must not authorise one either.** "That's my debris" is an
attribution claim — the same class as *"the nearest recent commit caused this"* in `R-123` —
and here the refuting datum was sitting in the path being deleted. Attribution is also
precisely what `scripts/peer-sessions.sh` warns against in its closing output: *"do not
infer authorship from who else was present."* I inferred authorship from who else was
*absent*.

**Cheap, and asymmetric.** `ls -l` costs one call and is not destructive. `rm -rf` on a
shared tree cannot be undone, and what it takes is exactly the mtimes and sizes that would
have told you whether you should have run it. Read first is not a general virtue here — it
is the only ordering in which the question survives the answer.

**Why it also found a real defect.** The stray directory was visible at all because
`.gitignore:43` is `/.buddy/*` — **root-anchored**, confirmed with `git check-ignore -v`,
which reports nothing for `docs/issues/.buddy/x`. So a `.buddy/` created under any
subdirectory is untracked rather than ignored, and a peer running `git add -A` commits
another session's tool log and narrative into the repo. The cause is upstream — a hook
writing to a relative `.buddy/` path from a non-root cwd — so the fix belongs in
`claude-plugins`, not in this `.gitignore`: there is no clean gitignore pattern that
matches nested `.buddy/` while leaving the root rule's `!/.buddy/memory/` negation
reachable, since git will not descend into a directory it has ignored.

**Proposal.** Add to the Phase 1 deletion sentence: *removal is authorised by a positive
finding — and "this is my own debris" is not one. Read the artifact (`ls -l`, owner, ids in
the path) before removing it; on a tree shared with peer sessions, the id is in the path
and costs one call to read.*

## R-151 — A design's quantitative premise is a hypothesis about a substrate nobody sampled

**Verdict:** hit — and the unusual kind, because **no downstream gate existed to confirm it**.

**What happened.** Scoping T-7 began by histogramming the live `catalog_audit` table rather
than by reading the approved spec's Phase 2 reasoning. The spec's one named volume term
(reindex churn) measured **0.4%** of rows; the unnamed term was **98.5%** — empty `{}` diffs
from an UPDATE trigger with no `WHEN` clause. Full evidence:
`catalog-audit-trail-session-log:F-1` and `catalog-audit-trail-session-log:W-1`.

**Why this is a distinct shape from the ledger's existing negative-result family.** The
`R-3 → R-113 → R-77 → R-79 → R-104` chain disciplines a **search** — a query whose zero, or
whose confident full answer, is evidence about the predicate rather than the world. This is
adjacent but not the same: the instrument was correct, the query was correct, and **nobody
had run one**. The design was reasoning about a distribution that had never been sampled,
because at brainstorming time the table did not yet exist — and nothing scheduled a
re-measurement for the moment it did.

`R-117`'s population form is the closest existing relative and is its **mirror image**: there
a fix named a population that turned out empty; here a design named a term that turned out
250× smaller than an unnamed one. Both fail **green**, which is the property that unites
them. The difference that matters for routing is the trigger: `R-117` fires when you are
*writing a fix that names a set*, and no `R-N` currently fires when you are *reading an
approved design that rests on a rate*. That is the gap this entry records.

**Why the existing promoted law did not reach it.** CLAUDE.md § Bug Tracking carries *"Run
the reproduction before reading the fix plan — the plan is a hypothesis about the
reproduction"* at four datapoints. It is phrased for **bug fixes** and keyed on the word
*reproduction*. Here there was no bug, no reproduction, and an approved spec — so the law was
loaded and did not fire. By this skill's own four-way audit that is the **outgrown** category
(still true, too narrow), not *unreachable*: the text does not cover designs, so a better
placement would not have helped. The remedy is re-promotion of an evolved form, not a
restatement.

**Proposal.** Widen the CLAUDE.md law from *reproduction* to *substrate*: **a design whose
argument rests on a volume, rate, or population owes one measurement of that quantity before
its plan is written — and a design written before its own substrate existed owes it again at
implementation time.** The second clause is the load-bearing half here and is absent from
every current form of the rule.

**Promote-when:** one further instance of a design (not a bug fix) whose central quantitative
claim is falsified by measuring first. At 2, promote the widened form; cite this entry and
`catalog-audit-trail-session-log:W-1`.

**Audit of the promoted set** (required by § *Every promotion audits the promoted set*), run
2026-09-01 against the four categories:

- *False* — none found. The bug-fix form's claim still holds; its four datapoints are intact.
- *Outgrown* — **one, this entry's subject.** Recorded above, not yet re-promoted, because it
  sits at one datapoint in the design form.
- *Unreachable* — not applicable here; the law was in context and did not match, which is
  narrowness rather than placement.
- *Obsolete* — none; no structural gate measures a design's quantitative premises, and W-1's
  counterfactual is precisely that no gate in the chain reads data rather than code.

**Valid:** dated 2026-09-01

**Rests on:** `catalog-audit-trail-session-log:F-1`'s measurement table; the spec text at
`docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` § Phase 2 as it stood
before `b0bdc4b1`.

## R-152 — R-49 audited against the four staleness categories — healthy; the gap is attribution, not text

**Verdict:** hit for `R-49` — **and this entry exists to record the audit, not the hit.**

Per this skill's § *Every promotion audits the promoted set*, a recurrence of an
already-promoted law is a defect in the promoted text rather than a new entry. So the
question here is not "does this deserve an R-number" but "which of the four staleness
categories does `R-49` fall into today". Checked, 2026-09-01:

- **False?** No. `R-49` says re-entering your own artifact is a seam, authorship no
  exemption. The T-7 pre-flight scan found four non-existent interface references in a plan
  its author had written 40 minutes earlier (`codescout:catalog-audit-trail-session-log:F-4`).
  The law describes exactly what happened.
- **Outgrown?** No — and this is the one worth stating, because it is the category the
  previous audit put the *reproduction* law into. `R-49`'s wording already covers plans as
  well as bug files, and covers implementing your own artifact as well as re-reading it.
  Nothing in this recurrence sits outside its text.
- **Unreachable?** No. It was in context (this session ran the skill earlier) and the scan
  happened.
- **Obsolete?** No. No structural gate compares a plan's identifiers against the symbol
  index; the compiler catches three of the four *after* dispatch, and the fourth
  (`EnvGuard`) it never catches at all, because the wrong-but-available substitute compiles.

**Verdict: `R-49` is healthy. No re-promotion owed.** Recorded so the next promotion
inherits this check instead of repeating it.

**The one thing this run adds that `R-49` does not say:** attribution. Two mechanisms fired
together — `R-49` from the recon pass, and `subagent-driven-development`'s *mandatory*
pre-flight scan — and the catch cannot be assigned to either. `codescout:catalog-audit-trail-session-log:W-3`
carries that caveat in full. The consequence for this ledger is a measurement rule rather
than a scouting rule: **when two loaded mechanisms both cover a catch, the datapoint counts
for neither's promote-when threshold.** Counting it for both is how a set of laws inflates
its own evidence base, and the failure is silent — every entry looks individually honest.

**Promote-when:** a second occasion where a catch is claimable by two mechanisms. At 2,
promote the counting rule into this skill's *Every promotion audits the promoted set*
section, where the four categories already live and where a shared-attribution case
currently has nowhere to go.

**Valid:** dated 2026-09-01

**Rests on:** `codescout:catalog-audit-trail-session-log:F-4` and
`codescout:catalog-audit-trail-session-log:W-3`, same session; and on `R-49`'s text as it
stands today, re-read rather than recalled for this audit.

## R-153 — Every Phase 1 instrument is read-only; mutation is the one that writes, and it collided with a peer

**Verdict:** hit on the finding, **miss on the method** — and the miss is the entry.

**What happened:** 2026-09-01. Scouting an uncommitted dispersion gate in
`scan_cited_prefix_with_no_definer`, reading the test was not enough to answer *"does this
test guard the constant it introduces?"* — the only instrument that answers it is mutation:
change the code, run the suite, read which tests die. Four runs established the constant is
unguarded across (0.667, 1.0] (`bug-fix-session-log:F-94`), which no amount of reading would
have produced. It also put a **disabled gate into a peer's staged index**, where it passed
their review and was stopped only by an unrelated pre-commit hook
(`bug-fix-session-log:F-95`).

**The gap in this skill:** every instrument Phase 1 prescribes is **read-only** — read the
symbol body, read the callers, run the call once and inspect output, verify at the bytes,
render one file not the directory. Mutation is the natural escalation for law *D — a test
that cannot fail is not coverage*, which this ledger already carries at 7% of entries, and it
is the first prescribed-adjacent step that **writes**. Nothing in Phase 1 marks that
transition. The skill even has a *disclosure* caveat for one read-only step (the `ytt`/`helm`
bullet: read-only against the cluster is not read-only against your transcript) — so the
category "this scout step has a side effect" exists, and the write case is missing from it.

**Why care did not help either party.** A mutation writes the same bytes an intentional edit
writes. The peer's provenance probe correctly returned `SHARED` and named both sessions; that
is the ceiling on what any observer could learn, because the semantics are not in the file.
This is `OB`-shaped: the party who can see it (me) has the parameter — *these bytes are
temporary* — and no channel carries it.

**Proposal for `SKILL.md`:** a Phase 1 bullet, roughly — *A scout that MUTATES is no longer a
scout, and the shared working tree is not yours. Reading a test tells you what it asserts;
only changing the code tells you what it catches, so mutation is the right instrument for law
D — but it writes, and a concurrent `git add` cannot distinguish your mutation from an edit.
Make the mutate-and-restore window **un-interleavable**: backup, mutate, test, restore and
verify — all inside ONE call, so it never spans a turn. Reach for a worktree or a committed
base when you want the class eliminated rather than merely collapsed.*

**Refined the same day by the peer this collided with, and the refinement inverts the emphasis
this entry was drafted with.** My original proposal led with *worktree*. Theirs leads with
*atomicity*, and theirs is better: what made my window collidable was not that the tree was
shared — it is always shared — but that the window **spanned tool calls and therefore turns**,
leaving a mutant on disk for minutes. One-call atomicity costs nothing, needs no worktree, and
works in the shared checkout. The worktree remains the stronger form (eliminates vs collapses)
and is right when a run is long or a rebuild is needed — it is what verified `F-94`'s closure
an hour later, when the shared tree would not compile. So: **atomicity first, worktree when the
work is long.** Both, not either.

**Second lesson, and it is about the mutation POPULATION rather than the tree.** My four runs
mutated the threshold's *value* (0.5, 0.6, 0.9) and never its *operator*. The shipped gate is
`files * NUM >= total * DEN` — two dimensions, and I sampled one. The author's fix pins both,
using a fixture sitting *exactly* on the boundary so that `>=` → `>` flips it. Generalised:
**when mutating a comparison, mutate the operator as well as the constant, and place one
fixture exactly on the boundary** — an off-boundary fixture cannot distinguish the two
operators at any threshold. This is `Phase 1`'s population law (*a fix that names a population
asserts that population is non-empty*) turned on the mutation set itself: I never asked what
my mutations could not express.

**Sharpened once more by the same peer, and this is the form to promote.** "Mutate the operator
as well as the constant" states the remedy; the *reason* is stronger and generalises further:
a boundary comparison has **two free parameters**, and a fixture placed off the boundary can
only ever probe one of them. An off-boundary fixture cannot distinguish `>=` from `>` at **any**
threshold whatsoever — so no amount of value-mutation reaches that axis, and a mutation run
that only moves the constant will report the operator as covered by never asking. The rule that
generalises is therefore about the FIXTURE, not the mutation: **put a fixture exactly on every
boundary you introduce**, and the operator mutation then has something it can kill. Stated that
way it is a design rule that fires when the guard is written, not a discipline that has to be
remembered at mutation time — which is the `Observer Blindness` preference for a check that
runs when nobody is worried.

**Promote-when:** one further instance of a reconnaissance step writing to a shared checkout —
mutation, a scratch fixture, a temporary `#[ignore]`, a bisect. At 2 datapoints, PR against
`codescout-companion/skills/reconnaissance/SKILL.md` citing this `R-N` and `F-95`. Routing per
§ *Promotion routing*: **craft-shaped** — it would not mislead another project, since it is a
property of concurrent agents on one checkout rather than of this repo's dialect. It does not
meet the session-opening-surface bar, which needs a measured base arm.

**Audit of the promoted set** (required by § *Every promotion audits the promoted set*, and
recorded so the next promotion inherits it rather than repeating it): checked the four Phase 1
laws nearest this one. *Substrate/verdict* — **not false, not outgrown**; it disciplines which
world an instrument reads and says nothing about writing to it. *Rendered read vs bytes* —
**still true**, and it is what made the byte-exact `diff -q` restore the right check here.
*Negative-search* — **unreachable is the risk, not falsity**; unchanged by this. *Green
certifies the executed path* — **outgrown in one direction and this entry is the widening**:
it tells you a green may prove nothing, and the way to find out is mutation, which the skill
then never mentions. No law was found obsolete; none cut.

**Status:** open — proposal at 1 datapoint.

**Valid:** dated 2026-09-01

**Rests on:** `bug-fix-session-log:F-94` and `bug-fix-session-log:F-95`, both of which carry
their own verification commands; and this ledger's own law D, whose 7% share is what makes
mutation a recurring need rather than a one-off.

## R-154 — a NUMBER inherited from a subagent report is a claim, and a floor carries no denominator to check it against

**Verdict:** hit — the scout ran before the ruling and inverted its conclusion.

**Observed:** 2026-09-01, adjudicating `IC-13`'s claim text in `docs/trackers/issue-clusters.md`.

**Seam:** a **number** carried across tasks. Two subagents, auditing a different question, each
remarked in passing that `IC-13`'s *"without a marker"* clause looked false for some members — "at
least four". That floor then travelled through three of my own messages, a commit message, and a
tracker entry, and was about to be the basis of a taxonomy ruling.

**Narrative.** Law A already covers this and names the exact artifact type: *"a subagent's
report"* is a **claim**, not ground truth. What made it slip through is that the claim was a
*number*, and numbers do not read as claims — they read as measurements that someone else already
took. Nothing about "at least four" prompts the question "four out of how many, established how?"

Measuring it (16 files, one reader each, quote per verdict, expected answer withheld) returned
**A=4 / B=5 / C=7**: the claim holds for 4, and the floor had been carried in the **opposite
direction** to its meaning — read as bounding the members the claim *fails* for, when it bounded
the ones it *holds* for. The true failure figure is 12 of 16. The largest bucket, C at 7, was a
category I had almost not offered, and two of its members disclaim the class **in their own text**.

Two properties made this durable rather than a one-off slip, and both generalise:

- **A floor has no denominator, so it cannot be checked by inspection.** "≥4" is compatible with
  4/16 and 4/4, which imply opposite rulings. It survived every re-reading because re-reading a
  bound tells you nothing new.
- **The framing selected the number's meaning.** I was holding "the claim is too narrow" — a
  wording problem, which predicts B — so ≥4 read as "4 B's". Under a membership framing the same
  digits would have read as "4 C's". The number did not disambiguate; my hypothesis did.

**Promote-when:** a second instance of a *quantity* (not a fact) inherited from a subagent report
or an earlier session and used without re-derivation. At 2 datapoints, propose a Law A sub-shape:
*a number from another task is a claim; re-derive it, and demand its denominator before it can
bound anything.*

**Status:** validated — single datapoint, caught before the ruling, cost was one measurement pass.

**Kin:** Law A (*ground truth is the artifact; a subagent's report is a claim*), of which this is
the quantity-shaped sub-case. Also `cluster-promotion-session-log:F-5`, the full working, and
`cluster-promotion-session-log:F-4`, where a different number — a count cell — went stale by
concurrency; the two together say that numbers in this repo rot both by *provenance* and by
*time*, and that the remedies differ (re-derive vs gate).

**Valid:** dated 2026-09-01

**Rests on:** `IC-13`'s 16 per-file verdicts, each quoting the bug file that settles it.

## R-155 — a gate that reads the working tree certifies a state you may not be shipping

**Verdict:** hit — the scout caught a bad `HEAD` that every gate had passed.

**Observed:** 2026-09-01, post-rebuild reconnaissance after shipping a taxonomy ruling.

**Seam:** the **commit boundary**. A green check, and a `HEAD` the check never read.

**Narrative.** Law *B* says a result is evidence about the configuration that produced it before
it is evidence about the code. The configuration axis here is neither build features nor which
binary — it is **which git state the instrument read**. `every_index_count_matches_the_corpus`
runs `git grep` over the **working tree**. It was green, correctly, and said nothing whatever
about what got committed: six of seven retagged files were left unstaged, so three newly-opened
classes published counts of 3, 1 and 2 against **zero members at `HEAD`**.

The general form is worth more than the instance: **a gate that reads the working tree certifies
the state on disk, and a partial commit ships a state that never existed on disk.** No amount of
re-running it before or after helps, because the shipped state is not one it can address. The
tell is a check whose input is "the tree" while the thing you are about to publish is "a subset of
the tree".

Two properties made it durable. It is **silent in the author's direction** — local green,
CI red, surfacing to whoever pushes next. And the repository's own habit of running the gate
before committing actively *reassures*: the check ran, it passed, and it answered a different
question than the one the commit posed.

**Runnable, and it cost one call:**

```
git grep -clE '<pattern>' -- '<paths>' | wc -l          # working tree
git grep -clE '<pattern>' HEAD -- '<paths>' | wc -l     # what you actually shipped
```

Any divergence after a commit you believed complete is a partial commit. I only ran it because a
peer had reported the same gate reddening for a reason its message could not express, which had
made HEAD-vs-worktree the habit an hour earlier — so the credit is a peer's report, not
carefulness.

**Promote-when:** a second instance of a check whose input tree differs from the state being
published — a linter over the working tree gating a commit, a test over `HEAD` gating a merge
result, a formatter run on unstaged files. At 2 datapoints, propose a Law *B* sub-shape: *name the
tree your instrument read, and check it is the tree you are shipping.*

**Status:** validated — single datapoint, caught by the scout, repaired at `4f598b5b`; the
preventing mechanism (a pre-commit hook deriving against the staged state) is proposed and not
built.

**Kin:** Law *B*. Sibling of `R-89` (freshness of the copy that serves you) — that one is *which
build*, this is *which tree*, and both are the same question asked of a different substrate. Also
`cluster-promotion-session-log:F-7`, the full working, and `IC-13`'s own gate, whose doc comment
already recorded the narrower tracked-only form of this.

**Valid:** dated 2026-09-01

**Rests on:** the measured HEAD-vs-worktree divergence, and on the gate's implementation reading
the working tree.

## R-156 — a guard's suggested remedy is a claim about YOUR situation, and it was written without it

**Observed:** 2026-09-01, committing `455184eb`. Three files formed one coupled change: a ledger row (`IC-22`, `n` 1→2), the bug file that count requires, and my own unrelated edits to the same ledger. The bug file had been staged by a peer.

**Got:** the `foreign-index` pre-commit guard refused the bare commit — correctly; the index held another session's path. Its message then prescribed the remedy:

> Commit your own paths by pathspec — that form ignores the shared index:
> `git commit -- <your paths>`

Following it would have committed the ledger **carrying the peer's `n=2`** without the bug file that satisfies it, producing a HEAD where `every_index_count_matches_the_corpus` fails against a corpus of 1. The guard is sound on the axis it owns (do not file another session's work under your message) and has no way to see that a count cell and a bug file are **one change**. Its remedy is correct against capture and, under coupling, ships a red `HEAD`.

**The general shape:** a guard is written against *its* failure mode by an author who cannot know yours. Its refusal is evidence; its **suggested fix is a hypothesis about a situation it has not examined**, and it arrives with the authority of the refusal attached — which is what makes it slip past the reflex that would question ordinary advice. This is `R-49`'s *"a proposed fix is a claim about current state"* arriving from a **mechanism** rather than a person, and it is harder to doubt for exactly that reason: a hook's text reads as policy, not as a proposal.

**Remedy:** treat the refusal and the prescription as separately checkable. Ask what the guard is defending, then whether the prescribed form has the property you need — here, *does the state this produces satisfy the other gate?* The check is cheap and mechanical: name what your commit must contain for every gate to hold at `HEAD`, and only then pick a commit form. In this case the pathspec form was still right; it just needed **all three paths**, which the guard's example did not include and could not have.

**Counterfactual:** without the check the commit would have been green locally (the working tree has every file), green in the guard, and red at `HEAD` for every peer who pulled — `R-155` again, arrived at from the opposite direction. `R-155` is about a gate reading the wrong tree; this is about *advice* that moves you to a tree the gate never read.

**Status:** validated — one datapoint, but the mechanism is inspectable rather than inferred: the guard's text is in `.pre-commit-config.yaml`'s `foreign-index` hook and contains no notion of inter-file coupling.

**Deepened 2026-09-01, and deliberately NOT counted as a second instance.** `codescout-b7`, reading `scripts/pre-commit-unreviewed-content.sh` to check an unrelated claim, found the same prescription is narrower in a **second, independent** way: `git commit -- <paths>` commits the **working tree** at those paths, so on a shared checkout it also takes whatever a concurrent session wrote to the *same* file between your edit and your commit. The hook's own header says so and states the bound — *"it narrows the window, it does not close it — only a per-session worktree does that."* So the remedy trades index-capture for **worktree-capture** rather than closing capture, and *"commits only those paths regardless of index state"* was never the safety property it reads as.

That is this entry's law firing on this entry's own worked example, twice over, in opposite directions: the remedy ships a red `HEAD` under coupling **and** it leaves a narrower capture window open. Both were discovered by reading the guard's implementation rather than its message.

**It is one instance, not two, and the distinction is the point.** Both findings are about the *same* prescription, so counting them separately would promote this entry on a doubled datapoint — which is the shape `IC-22`'s uncounted-instances note refuses one ledger over. The `Promote-when` below stands unfired.

**One mitigation worth knowing, measured rather than assumed:** for a pathspec commit that was `git add`ed first, `pre-commit-unreviewed-content.sh` compares the about-to-be-committed blob against the **real index** blob and refuses on any difference — so a peer's same-path write landing after your `add` is caught, not captured. The residual window is between reading the diff and running `add`. Audited on this session's own three pathspec commits touching shared files: no unintended capture, and the one that carried a peer's edit did so knowingly and with their consent recorded in the message.

**Promote-when:** a second instance where a guard's or linter's suggested fix is locally correct and globally wrong — arising from a **different** guard, not a further consequence of this one. Then it belongs next to `R-49` in the *proposed fix is a claim* family rather than as its own entry.

**Valid:** invariant

**Rests on:** the general principle that a mechanism's prescription is authored without the caller's context, not on the specific hook — which may gain coupling-awareness and would not falsify this.

## R-157 — A work-split is two claims — and the "easy half" label is the one that says don't check

**Valid:** dated 2026-09-01

**Verdict:** hit — the scout ran before the annotation pass and refuted its whole premise.

**Observed:** 2026-09-01, entering the `T-15a` task recorded in
`system-retrospective-improvements` as *"ready now, no judgement required … Pure annotation,
build-safe."* Scouted the guide-index gate structure and delivery mechanics before writing a
single declaration.

**The law.** `R-95` established that a **deferral** rationale is a claim, and that its bias has
a direction: nobody drafts an estimate that makes deferred work sound *easier*, because that
estimate would not justify stopping. This is the mirror case, and it has the opposite sign.
**When work is SPLIT into an easy half and a hard half so the hard half can be deferred, the
easy half's cost is written to justify the split — so it is DEFLATED.** Both halves of the
split are claims; `R-95` audits one of them.

**Why the easy half is the more dangerous one.** A deferral rationale says *"do not do this"*,
so nobody acts on it — it is durable and wrong, but inert. A label reading *"ready now,
build-safe, no judgement required"* says *"do this without thinking"*, which is an instruction
to **skip the scout**. It does not merely fail to prompt verification; it actively suppresses
the check that would refute it. The phrase *"no judgement required"* is the tell.

**Evidence — three independent refutations of one three-clause label:**

1. *"Pure annotation"* — false. Gate 5 (`every_section_of_a_declaring_topic_is_reachable`)
   makes the **topic** the unit, not the section: the first declaration in a topic obliges
   every other section in it. So the four topics with an over-cap section cannot be partially
   annotated at all, and the recorded per-topic counts (`6/7`, `10/11`, `8/9`, `10/15`) describe
   a partition the gate does not permit.
2. *"Build-safe"* — false, and inverted. `Shape::matches` opens `let Some(sel) = sel else {
   return false }` — deliberate, "do not turn it into a wildcard". Only five of the registry's
   tools override `selector_key`; **every** tool routing to `progressive-disclosure` (nine),
   `symbol-navigation` (three) and `workspace-state` (one) returns `None`. Annotating them does
   not add delivery, it REPLACES whole-guide delivery with the preamble alone. Reproduced in
   the delivered bytes.
3. *"Ready now"* — false. Four of the nine topics are in `PULL_ONLY_GUIDE_TOPICS`, and
   `GetGuide` serves `topic_body()` without ever consulting the index, so declarations there
   are inert regardless. `tracker-conventions` is the only annotatable topic left, and it is
   the five-over-cap one — i.e. `T-15b`, the half that was deferred.

**Counterfactual.** Without the scout: ~13–40 sections annotated across three to nine files,
gates 1–6 green, and a silent downgrade of three topics' guide delivery to preamble-only. Three
of the four topics turned out to have a pre-existing guard, so the likeliest outcome is a
confusing red on a fixture assertion (`a_topic_without_declarations_reports_false`) that an
author would legitimately re-point before proceeding — the one topic whose only signal is
re-pointable is `progressive-disclosure`, the largest of the three.

**Proposal.** The Phase 1 bullet promoted from `R-95`/`R-92` reads *"a deferral rationale is a
claim, and the least-audited kind."* Widen it to **"a work-split is two claims, and the
easy-half label is the one that tells you not to check"** — naming the deflation direction and
the `no judgement required` / `build-safe` / `mechanical` phrasings as triggers. Same law, one
mechanism it does not currently cover.

**Rests on:** `Shape::matches`'s `None`-is-no-match contract, and Gate 5's topic-level scope.
Both pinned by tests as of `b769277b`; re-verify if either changes.

## R-158 — The canonical case is the worst positive control — prominence is why it's already covered

**Valid:** dated 2026-09-01

**Verdict:** miss, twice, in one session — and caught only because the law was applied a second
time rather than because it was known.

**Observed:** 2026-09-01, twice while mutation-verifying two new gates (`1b02f36b`, `b769277b`).

**The law.** `R-3`'s promoted form says *"run a positive control … one per state you believe the
instrument can report."* True, and it does not say which case to use. **The case that comes to
mind first is the canonical, highest-traffic, most-documented one — which is exactly the case
most likely to be already covered, so a control built on it confirms and teaches nothing.**
Prominence is what makes a case memorable *and* what got it protected. The two properties share
a cause, so the correlation is systematic, not luck.

**Both instances, same shape:**

1. Verifying Gate 6 (declared `serves:` shapes must name a live tool/action). Mutated
   `artifact.create` → `artifact.creat`. It reddened — and reddened **two pre-existing tests**,
   because `artifact.create` is one of six shapes a hard-coded high-volume test
   (`librarian_declares_the_six_highest_volume_artifact_shapes`) already pins. Stopping there
   would have "proved" the gate fires while proving nothing about whether it was **needed**.
   Re-mutated on `artifact.link` — one of the 18 of 24 declared shapes nothing pins — and the
   real figure appeared: **132 guide tests pass, Gate 6 alone fails.**
2. Verifying Gate 7 (a declaring topic needs a live route). Annotated `symbol-navigation`
   first — the topic `a_non_declaring_topic_is_byte_identical_to_today` happens to pin, because
   that test uses `symbols(path=".")` as its fixture. Re-ran on `progressive-disclosure` and
   `project-activation-bootstrap`, which is what produced the honest result: **three of four
   candidate topics already had a guard**, and only one (`progressive-disclosure`) had nothing
   but a re-pointable fixture assertion.

**The asymmetry that makes this worth an entry.** The two instances failed in **opposite
directions** from the same mistake. In (1) the over-covered control made the new gate look more
necessary than it was. In (2) it made the tree look less defended than it is — and I had already
published *"passes all six gates"*, which the second mutation retracted. So this does not bias
toward optimism or pessimism; it biases toward **whatever the canonical case happens to be**,
which is unpredictable and therefore not correctable by leaning one way.

**Proposal.** Extend the positive-control sentence in the Phase 1 bullet: *"choose the control
from the population's UNREMARKABLE members — the canonical case is the one most likely to be
already covered, so it is the worst available control."* Pairs with CLAUDE.md § Testing
Discipline's *"mutate once per guarded SITE"*, which says how many controls to run and is silent
on which. Two datapoints, both this session, both mine, in two different subsystems.

**Cost if it had gone unnoticed:** one over-claimed gate justification and one published claim
("passes all six gates") left standing. The second was already in a commit message before the
re-mutation retracted it.

**Rests on:** the two pre-existing tests named above continuing to pin those specific
fixtures — `librarian_declares_the_six_highest_volume_artifact_shapes`' hard-coded six, and
`a_non_declaring_topic_is_byte_identical_to_today`'s `symbols` call. If either generalises, the
instances stay valid as history but the ledger should not be read as still describing the tree.

## R-159 — verifying a MECHANISM is not verifying its REACHABILITY, and a clean mechanism check feels like the strong form

**Observed:** 2026-09-01. A peer reported that a spec's planned change would not unblock what its author might assume, because `selector_key` returns `None` for every tool routing to `progressive-disclosure` and `Shape::matches` rejects `None` by construction. I refused to relay it unverified — `R-154` — and checked all three load-bearing facts myself: `Shape::matches` at `src/prompts/guide_index.rs:179` opening `let Some(sel) = sel else { return false }`; the trait default at `src/tools/core/types.rs:1439` returning `None`; and exactly five override sites out of six files containing `fn selector_key`. All three held. I relayed it as time-critical to a session mid-implementation.

**Got:** correct, and **moot**. The recipient found a fourth fact neither of us had looked for: `src/prompts/guides/progressive-disclosure.md` contains **zero** `serves:` markers — `librarian.md` is the only declaring guide in the tree — so `guide_blocks_for` takes the `!GUIDE_INDEX.declares(topic)` branch at `types.rs:1019` and **`Shape::matches` is never consulted for that topic at all.** The blocker is real, verified, and sits on no live path. It fires only if someone later adds `serves:` to that guide.

**The gap:** I verified the mechanism and never asked whether anything reaches it. Every one of my three checks was a question about how the code *behaves when entered*; none was the question *is it entered*. That distinction is invisible from inside the verification, because a mechanism check that comes back clean feels like the strong form of confirmation — I had read the actual bytes, not a doc.

**This is `CLAUDE.md` § *Testing Discipline*'s "loudness is a property of a PATH, not of a failure" pointed at a claim rather than a guard.** `BL-66` is an `abort!` nothing arrives at; this is a *verified blocker* nothing arrives at. Same structure, and the same remedy transposes: **name the concrete caller that reaches it.** For a guard that means naming who trips it; for a relayed claim it means naming the live configuration in which the mechanism fires. Had I asked *"which guide declares a shape today?"* — one `grep -l`, the check the recipient actually ran — the answer is one file and not this one.

**Cost and non-cost.** Cheap here: one read for a session mid-SDD-run, and they recorded the trap in their spec as a checked non-assumption, so the warning bought something despite being inapplicable. The failure mode worth respecting is the other branch — a *stop-work* warning routed on a verified-but-unreachable mechanism, which is expensive precisely because every fact in it survives scrutiny.

**Not an argument against verifying before relaying.** `R-154` still holds and the verification was right; the three facts are true and now pinned by a peer's Gate 7. **Reachability is a fourth question, additional to the three, and it is the one that decides whether the claim is actionable.** Verifying the mechanism harder never reaches it.

**Status:** validated — one datapoint, mechanism inspectable rather than inferred (`grep -l 'serves:' src/prompts/guides/*.md` returns one file; the branch is at `types.rs:1019`).

**Promote-when:** a second instance where a verified mechanism turns out to sit on no live path. Then it belongs beside `R-3`'s positive-control rule as its mirror — that one says prove your instrument can *find*; this says prove your finding can *fire*.

**Valid:** invariant

**Rests on:** the general distinction between a mechanism's behaviour and its reachability, not on this guide corpus — adding `serves:` to a second guide would change the example and not the law.

## R-160 — Partial success is the camouflage — one pattern over a heterogeneous population misses its odd member

**Valid:** dated 2026-09-01

**Verdict:** miss — I asserted a fact about the world from a search that found nothing, and a
peer session caught it. Downstream catch, not a self-catch.

**Observed:** 2026-09-01, verifying a peer's claim that
`$CLAUDE_CONFIG_DIR/sessions/<pid>.json` carries a pid→sessionId join. I probed five keys in one
loop and reported that four were present and that **`pid` was not a field, only the filename** —
offering that back to the peer as a refinement to their bug file. It was wrong. The bytes are
`"pid":3624594`.

**Mechanism, and it is not the obvious one.** The loop built one pattern for all five keys:

```
grep -o "\"$k\":\"[^\"]*\"" "$f"      # requires a QUOTED value
```

`sessionId`, `name`, `messagingSocketPath` and `cwd` are JSON **strings**; `pid` is a JSON
**number**. The type-uniform pattern silently excluded the one key typed differently. The peer's
own diagnosis — that the file is minified and I had grepped for `": "` with a space — is also
wrong: `grep -c '": '` over the file returns **0**, so no spacing assumption was involved either
way. Verified before conceding, which is how the sharper cause surfaced.

**The new mechanism this adds to `R-3`. Partial success is the camouflage.** `R-3`'s promoted
form warns that a zero is evidence about the search. What made this one persuasive is that the
search was **4/5 correct**: four keys came back with real values, so the instrument was visibly
working, and the fifth's blank read as a property of the data rather than of the pattern. A
uniform 0/5 would have prompted an immediate look at the pattern. **A heterogeneous population
queried with one pattern fails exactly on its odd member, and the successes are what make that
failure legible as absence.**

**Fourth false-negative filter of my own in this one session**, three of them in `cargo test`:
`-- <bare name> --exact` (matched 0 of 4820, reported `ok`), `-- guide` (matched 132 tests but
not the gate under test, whose name contains no "guide"), `-- valid_slugs` (no such test exists;
the real names are `every_declared_class_has_an_index_row` and siblings), and this grep. Each
returned a **well-formed, plausible result** rather than an error, and two of the four were
about to be published.

**Proposal.** Add to the `R-3` bullet: **"a query over a heterogeneous population encodes a type
or shape assumption — check the pattern against the ODD member, not the typical one, and treat a
partial hit as a stronger warning than a total miss."** Note this is `R-158`'s sibling with the
same root: there, the *control* was drawn from the population's most prominent member; here the
*pattern* was fitted to the population's most typical member. Same bias, two different
instruments.

**Rests on:** the registry being minified JSON with `pid` as a bare number. If a future Claude
Code version pretty-prints it or quotes `pid`, the instances stay valid as history but the byte
detail does not.

## R-161 — The weaker act wears the stronger act's appearance — discharging a warning, and a present-but-inert fixture

**Valid:** dated 2026-09-01

**Verdict:** hit on both instances, but by luck of sequencing in the first and by a peer's
observation in the second — neither was reached by method.

**Observed:** 2026-09-01, two exchanges with `codescout-b7` and `codescout-3c`.

**The law.** Two acts came up an hour apart that had nothing in common on the surface, and the
shape is the same: **the weaker act wears the stronger act's appearance, so completing it feels
like completing the stronger one.** Neither failure is a lapse in care — care is fully engaged on
the weaker act, which is precisely why the substitution is invisible.

**Instance 1 — checking a warning is not clearing a change.** `b7` received a three-fact warning
from me about `selector_key`, verified all three in the bytes, found a fourth fact that made them
inapplicable to their change, and their next move was to dispatch the next task. The warning had
been *discharged*; the change had not been *reviewed*. What stopped them was an unrelated scout of
a different question, and they said so plainly: *"I got there by luck of sequencing, not by
method."* Their change then turned out to carry **two** defects neither of us had named — the gate
it proposed already existed, and its predicate would have been permanently false. **A peer warning
that turns out not to apply reads as "cleared", and clearing is a claim about the whole change
that answering a warning never makes.**

**Instance 2 — a present fixture reads as coverage.** Gate 7's probe set contains an
`{"output_id", "overflow"}` row that exercises real branches in `symbols` / `references` /
`call_graph` — and the gate evaluates only *declaring* topics, where the librarian router reads
`abs_path` / `violations` and never looks at either key. So the row changes no outcome. It is not
wrong; it is **inert**, and inert looks identical to load-bearing from the outside.

**The remedy for instance 2 is the INVERSE of a law this project already has.** CLAUDE.md §
Testing Discipline says *annotate a fixture's load-bearing detail on the fixture line*, so a
tidy-up cannot silently remove what makes the test discriminate. This is the same failure from the
other end: **annotate a fixture as inert, so nobody credits it with coverage it does not
provide.** One guards against silent removal, the other against silent credit — and `b7`'s reading
is that the second is worse, because a false sense of coverage actively stops the next person
looking. Shipped as a worked example in `b4fa1be6`: *"nothing breaks if it goes now, and it must
not be cited as overflow coverage."* Keeping the row while explicitly denying it credit is a
better disposal than deleting it (which loses the probe for when it becomes live) or quietly
retaining it (which is the defect).

**Proposal, two destinations by the routing test.** Instance 1 is recon-shaped and belongs in the
Phase 2 *Compare* step: **discharging a warning is not an outcome — name what the warning covered
and what remains unchecked.** Instance 2 is craft-shaped and belongs in CLAUDE.md § Testing
Discipline as the stated inverse of the load-bearing-fixture law; it is **not** filed there by
this entry, because CLAUDE.md edits are the operator's call and that file was being modified by
another session at the time. Surfaced, with the worked example already committed.

**Cost if unnoticed:** instance 1 would have dispatched a task implementing a non-bug as a silent
regression; instance 2 leaves a probe that a future reader cites as overflow coverage while it
provides none.

**Rests on:** the librarian adapter's router reading only `abs_path` / `violations`
(`src/librarian/adapter.rs`), and `types.rs:1271-1279` reading `output_id` from the tool's own
`val` rather than the envelope — so the key is present for self-buffering tools like
`run_command` and absent for `read_markdown`. Re-verify if either moves.

### Addendum 2026-09-01 — the remedy has a gap, found within minutes of filing

`codescout-b7` turned this law on their own in-flight work immediately after reading it and
returned two things that change the entry.

**A fourth instance, self-identified, and it is about the disposal rather than the defect.** They
decided against messaging their running implementer mid-task, on the grounds that *"I'll just send
one more thing" is its own weaker-act-wearing-a-stronger-act's-appearance* — sending a correction
wears the appearance of fixing the task, while actually mutating a running task's requirements,
which is what a fix loop exists to avoid. Filed here rather than as a new entry because the
**direction is constant**, which is the property that makes the class invisible rather than merely
missed: **the weaker act is always the one being fully attended to.**

**And a genuine gap in remedy (b), which the entry above states too confidently.** *Annotate the
fixture as inert, at the fixture line* presumes the detail **has** a line. Their `get.rs` and
`adapter.rs` tests jointly cover one property — one pins what the stub emits, the other pins that
such a value suppresses the map — and it survives the obvious mutation, because an empty **array**
also yields `None` from `section_headings_summary` (`rendered.is_empty()` returns before the length
check), so a stub emitting `[]` stays correct. The real gap is thinner than *"the fixture is
fake"*: **nothing names the interface the two files rely on.** The load-bearing detail *spans* two
files, so neither file is its site — CLAUDE.md's annotate-at-the-fixture-line law meeting a fixture
whose line is in a different file. **A property jointly covered by N files has N candidate
annotation sites and no owner, so the law silently has nothing to fire at.** That is a distinct
sub-shape from an inert fixture and needs a different remedy — probably a named contract both
files cite, not a comment in either.

**Preferred wording for the CLAUDE.md proposal, theirs and better than mine:** state the
**asymmetry**, not the pairing — *false coverage stops the next person looking; silent removal does
not.* The pairing is symmetrical and reads as tidy; the asymmetry is the reason to act.

**Also worth recording about their method — corrected at their own insistence, and the correction
is the finding.** The first version of this note credited them with verifying *"my tests are
end-to-end"* rather than asserting it, and called it the one move that does not bottom out in the
same substitution. They rejected the credit: the check worked *only* because they had just read
`F-1`, where a fixture had fooled them four hours earlier. **A primed check, not a standing one**
— the same shape as their *"luck of sequencing, not method"* admission about instance (a).

So the honest tally is worse than the entry first implied, and more useful: **both saves in this
thread came from recent damage, not from method.** Four instances, and the mechanism count is
**zero**. That is the `OB-N` admission test met squarely — a law with instances and no mechanism
is a design worklist item, not a solved problem, and the temptation this entry nearly indulged was
to bank a peer's alertness as if it were a procedure. Recording who was primed by what is the
nearest thing available to a base rate here.

**Concrete remedy for the cross-file gap, derived by `b7` after the amendment.** "Assert the
property, with a comment naming the cross-file dependency" dies to the same objection — it puts the
annotation on one side. What replaced it: **the contract must be named and cited from BOTH sides,
not commented on one.** In their case `stub_preview`'s doc comment in `get.rs` and
`section_headings_summary`'s at `adapter.rs:747` each state and cite the same contract — *a
body-selected read's `preview.headings` is not a renderable array, which is what suppresses the
section map* — so either site alone is the defect the constraint exists to prevent. That is the
shape the generalisation above was reaching for: not an annotation with no owner, but one contract
with two citers.

## R-162 — A DELETION cannot introduce the token you grep for — the zero is guaranteed before you run it

**Valid:** invariant

**Verdict:** miss. The scout ran, produced a zero, and the zero was used to overrule a
correct signal. Caught by a peer's independent `git diff --cached`, one command before a
broken commit.

**Observed:** 2026-09-01, coordinating a shared-checkout commit with `compact-root-claude-md`
(pid 3624594, `.claude-sdd`). My fix-wave subagent flagged that the index held a collision in
`src/librarian/adapter.rs` and recommended syncing before committing. I screened the claim by
grepping the staged file for `action_selector_key`, got **0**, concluded no peer refactor was
present, and overruled the subagent.

**The law, and why it is decidable in advance.** `R-3` says a search that finds nothing is
evidence about the search rather than about the world — a caution you apply *after* reading a
zero. This is the sharper form, with a predicate you can evaluate *before* running the query:

> When the change class you are screening for is a **deletion**, a search for the token
> being introduced returns zero regardless of the file's contents. The query cannot express
> the state it is being used to rule out.

Measured on the commit that landed (`30b6fc41`): `action_selector_key` occurs **0 times in
`adapter.rs` both before and after** the peer's change. My query returned the same answer in
the world where their work was staged and in the world where it was not — zero bits. The
discriminating query was one line away and I did not run it: `fn selector_key` goes **2 → 1**
across the same commit, and `git diff --cached` — which the peer ran — named both authors
immediately.

**The direction problem is what makes token queries unsafe here in general, not just this
once.** At screening time you do not know which way a peer's edit ran. A token query is a bet
on direction: it can only detect an *introduction*, so it is blind to exactly half the
hypothesis space, and blind *silently*. The operational rule takes no judgement:

> To detect a peer's change in a file, **diff the file**. Do not grep it for a token — you
> cannot know which direction their change ran.

**Why this is not merely R-3 again.** `R-3`'s three named ways a zero lies are *scope*,
*shape* and *encoding* — all properties of how the query was written. This one is a property
of the **hypothesis**: the query is well-formed, correctly scoped, and searches the right
file. It fails because the proposition under test has a direction and the instrument only
reads one of them. Widening the corpus, the glob, or the pattern fixes none of it. It is
CLAUDE.md § *Testing Discipline*'s recording-filter law — *a test cannot detect what its
recording filters out* — arriving at a peer-coordination check instead of at a test.

**The escalation is the expensive part.** The bad instrument did not merely fail to inform;
it **outranked a correct signal**. A subagent had already identified the collision by the
right method and recommended the right action. A zero from a query that could not have
returned anything else was treated as strong enough to reverse that. Note the asymmetry that
made it feel safe: a *positive* hit would have been real evidence, so the query felt like a
test with two outcomes when it had one.

**Cost if unnoticed:** the commit would have carried the peer's `LibrarianAdapter::selector_key`
deletion **without** the `types.rs` default inversion it is atomic with — leaving the override
gone and the default still `None`, which kills librarian guide routing outright. Not untidy
attribution: a bisect hazard, landing under my `Session-Id` trailer with the peer's reasoning
nowhere in the message.

**Rests on:** nothing in the tree — the law is about query direction, not about any current
symbol. The worked measurement rests on `30b6fc41` (patch-id
`db34821395d6257eb37562bb779ed9ab4eba091e`) still being resolvable.

**Status:** open — proposal for the skill's Phase 1, one line: *when screening for a peer's
in-flight change, diff the file; a token grep can only see an introduction.*

**Credit:** the sub-shape was named by `compact-root-claude-md`, who ran the diff that caught
it and declined to write the entry so the counterfactual would stay with the session that
lived it.


### Addendum 2026-09-01 — two ways to hold a well-formed zero, and the standard remedy only fixes one

Written with `compact-root-claude-md`, who filed the second instance the same evening and
offered to carry this; it stays here because the first instance is the one that needs the
distinction. Cited from `R-165`, the instrument-versus-direction entry whose pairing table this
generalises — not from `R-163`, which was the first home offered and the wrong one, for the
reason recorded below.

**Both instances are a well-formed query, correctly scoped, returning zero. They are different
defects.**

| | this entry (`R-162`) | the type-B case |
|---|---|---|
| the query | *could not* have returned non-zero | *could* have |
| what failed | the **hypothesis** — it has a direction, the instrument reads one of them | the instrument's **scope** — the window excluded the answer |
| the case | a deletion cannot introduce the token grepped for | a grep whose context window stopped one line short of the tag it was checking for |

**Where the type-B case comes from, corrected 2026-09-01 within the hour.** This table first
named `R-163` as the type-B entry. It is not: `R-163` is about a peer's *observation* arriving
with an inferred *cause* attached, where the figure was real and only the attribution was
unverified — no zero, no window, and neither half of this discriminator reaches it. `R-163`'s
own `**Rests on:**` now says so explicitly, and the correction is `compact-root-claude-md`'s.

The type-B case is real but **is not written up anywhere in this ledger**; it reached me as a
peer's message and nothing else. Cited here as what it is.

**And the miscitation was this addendum committing R-163's own law while generalising it.** I
took a detail from a peer's message, inferred which entry carried it, and cited that entry
without opening it — an observation and my attribution of it to a source, recorded as one
claim. The general reflex was engaged and pointed elsewhere: I was verifying *their* commits by
patch-id at the time. **`R-3`'s rule that a search's result is evidence about the search has a
citation twin that this corpus had not stated — a citation is evidence about the citer's model
of the target, never about the target, until the target is read.** Cheap to check: one
`artifact(get, heading=…)` would have shown `R-163` contains no zero at all.

**The discriminator, and its timing is the asymmetric part.**

- **Type A is decidable *before* you run it**, from the query's shape against the hypothesis's
  direction: *could this query return non-zero in the world I am trying to rule out?* Costs
  nothing. It is the only failure of the two that is free to prevent.
- **Type B is decidable only *after***, and only by a second instrument whose **scope** differs.
  Re-reading the same window with the same belief does not reach it — which is `R-3`'s
  positive-control rule, and why that rule is about a *different* defect than this entry's.

**The trap is that the standard remedy for B is a no-op against A.** "Widen the window, re-run
bigger, take a larger sample" is the reflex answer to a suspicious zero, it is correct for B,
and it changes **nothing** for A at any size — a directionally-blind query stays blind however
much corpus you feed it. This is CLAUDE.md § *Testing Discipline*'s recording-filter law
arriving as a query rather than as a test: *"'Widen the sample' fixes member-selection and
changes nothing here, at any corpus size — which is what makes it worse than a small sample,
because the reflex answer looks responsive."*

**And no instrument reports which one you are holding.** Both return the same artifact: zero
results, no error, from a query you can defend. So the question to ask of a zero is not "is this
right?" but **"which of the two am I holding?"** — and if the answer is A, no amount of
re-running helps, while if it is B, nothing but re-running does.

**Provenance is the uncomfortable part and belongs in the record.** Both instances were
committed the same evening, by two sessions who were at that moment writing up laws about
instruments that answer a narrower question than they appear to. The sharpest case is the
correction above: a miscitation committed *inside the entry generalising the class*. Knowing the
class prevented neither of us — which is the `OB-N` shape, and the reason the remedy here is a question asked at query-authoring time rather than a resolution to
read zeros more carefully.
## R-163 — A peer's observation and its attributed CAUSE are two claims; the real number makes the inferred condition read as measured

**Valid:** invariant

**Verdict:** miss. The peer's figure was verified and their *attribution* was not, and the
unverified half reached a queryable `unverified:` field on a durable bug record, where it
asserted that a bisect was feasible. Caught by the reporter's own re-check within the hour —
not by me, and not by any gate.

**Observed:** 2026-09-01. A peer reported two reproductions of a known low-rate flake *and*
attributed them to a condition: *"both were full `cargo test --workspace` runs on a box with 6
live sessions and a concurrent cargo build holding the target lock… that moved the rate from
~1-in-N to 2-of-2."* I had verified an unrelated numeric claim of theirs the same hour — byte
arithmetic on a snapshot delta, `+3 −1 = +2`, which checked out exactly — and then took this
one whole. I wrote it into a durable bug record (`ee9d8d80ad5ecdc8`), **updated its queryable
`unverified:` field to say a bisect was now feasible**, and specified the next probe on that
basis.

Retracted by the reporter within the hour, from their own raw buffers: run 1's two lock-waits
were at *build start* and resolved before any test ran; run 2 had **zero** lock-wait lines and
failed anyway. Lock contention is therefore not necessary, refuted by their own second
reproduction. Their words: *"I had a coincidence and named a mechanism for it."* The probe I
had specified would have spent effort on a variable they had already accidentally controlled
for, in the direction of innocence.

**Mechanism — and why this is not the existing law about verifying a peer's number.** An
observation and the *attribution of a condition to it* are two claims, delivered as one
sentence by the party best placed to conflate them. The corpus already says verify a peer's
figure (`R-160`) and that a proposed fix is a claim about current state (`R-117`). Neither
reaches this, because here the figure was **real** — two runs did fail — and only the
*condition* was inferred. The observation lends its credibility to the attribution riding
along with it.

**What made it slip past a reflex that was actively engaged.** I hedged, and hedged the wrong
clause: I wrote "a discriminating candidate, not a finding" about the **causal** reading while
recording the **observational** claim ("2-of-2 under a named condition") as fact. Care was
fully applied one level away from where it was needed — `R-161`'s shape, arriving through a
peer channel. The tell available at the time and not used: **"2-of-2" is a denominator over a
population the reporter selected**, and I never asked how the two runs were chosen, how many
runs there were in total, or how the condition was established.

**Cost, and why it is worse than an ordinary wrong belief.** It reached a *queryable field* on
a durable record. A wrong narrative sentence is re-read by whoever next opens the file; a wrong
`unverified:` is what a triage query returns, and it said "feasible" about work that is not.
Same asymmetry as `R-117`'s durable-record surface: a fix built on a bad premise fails at the
call site, a filed claim built on one is cited and re-checked by nothing.

**Remedy, stated as a question rather than as vigilance:** when a peer hands over an
observation *plus* a cause, record the observation and ask one question about the cause —
*"how did you establish that?"* One message, and the reporter here answered it unprompted an
hour later, which is evidence the question was cheap and would have been welcomed. Do not
apply the general "verify a peer's number" reflex and consider the transaction closed: the
number may be the sound half.

**Status:** open — the remedy is a one-question habit and is unmechanised. No gate can catch
this; the nearest mechanism is that a `**Rests on:**` line required on a bug-file claim would
have forced the provenance question at write time.

**Rests on:** `docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`
§ *RETRACTED, same day, by the reporter*, which preserves the superseded text and the corrected
`unverified:`. Related: `R-160` (verify the figure), `R-117` (a claim about current state),
`R-161` (care applied one level from where it was needed), and `R-162` — the same peer's own
write-up of the mirror-image failure in the same exchange, whose addendum carries the Type A /
Type B discriminator for zeros. That discriminator deliberately does **not** cover this entry:
the claim inherited here was not a zero at all, but a *real* figure with an inferred cause
attached — which is exactly why neither the before-you-run shape check nor the
widen-the-window remedy reaches it. The question that does is the one in *Remedy* above.

**Promote-when:** a second instance where a peer-supplied *attribution* rather than a
peer-supplied *number* reaches a durable record. At two, this belongs in CLAUDE.md §
*Reaching a Peer Session*, whose current text disciplines routing and counting but says nothing
about inheriting a peer's causal reading.

## R-164 — A mutation's kill COUNT answers a different question than necessity, and the reassuring number is the one that establishes least

**Valid:** invariant

**Verdict:** near-miss, self-caught. The misleading number was in hand and being composed as
the new gate's justification; the per-site mutation rule (CLAUDE.md § *Testing Discipline*)
caught it before publication. Third instance of the family in one session, by the agent who
wrote up the first two — so knowing the law is demonstrably not the mechanism.

**Observed:** 2026-09-01, verifying `30b6fc41` (inverting `Tool::selector_key`'s default so
every tool opts into operator-rule routing). I added a registry-wide gate and mutated to check
it. Two mutations, and the numbers point opposite ways:

| mutation | result |
|---|---|
| **canonical** — trait default returns `None` again | **11 tests fail** |
| **per-site** — one `selector_key -> None` override on `ReadFile` alone | 4826 pass, **exactly 1 fails**, naming `read_file` |

I had the 11 in hand and was composing it as evidence for the new gate. It is not: **10 of the
11 already existed.** The canonical mutation establishes that the *inversion* matters and says
nothing whatsoever about whether the gate I just wrote was needed. Only the per-site mutation
does that — and it shows the gate is the sole signal when a single tool opts out, which is the
case a tool added next month would actually hit.

**Mechanism, and why the number is actively misleading rather than merely uninformative.** A
mutation run answers *"is this line guarded?"* A new guard's author wants the answer to *"is
MY guard load-bearing?"* — a different question. The kill count measures the size of the
existing guard population at the mutated site, and that population is **largest at the
canonical site**, because prominence is what got it covered in the first place. So the count is
*anti*-correlated with the information wanted: the more reassuring the number, the less it
establishes. And a big number is exactly what does not prompt a second look, which is why this
is a distinct failure from picking the wrong case.

**Not a duplicate of `R-158`, and not of CLAUDE.md § *Testing Discipline*'s per-site rule.**
`R-158` is about **which case to mutate** (the canonical one is the worst positive control).
The standing law is about **how many sites** to mutate (once per guarded site, not once per
feature). This is about **how to read the result** — and it bites even when both of those are
obeyed, because I *did* go on to mutate per-site; the near-miss was publishing the canonical
count as the gate's justification while doing so.

**Third instance in one session, same family.** `artifact.create` in `1b02f36b` (two
pre-existing tests fired, proving nothing about Gate 6); `symbol-navigation` in `b4fa1be6`
(the one topic an existing byte-identity test pins, which forced a published retraction); and
this. The rate matters: three in one sitting, by an agent who had written the first two up.
Knowing the law did not prevent the third — which is the `observer-blindness` signature and the
argument for a mechanism rather than more care.

**Remedy, and it is a sentence-level discipline because that is where the error lands:** never
report a kill count without naming the proposition it establishes. For a new guard, the
reportable figure is not *"the mutation killed N tests"* but *"mutating a site this guard
uniquely covers left M other tests green and reddened only this one."* The second form is
falsifiable by the reader; the first is not, and reads stronger.

**Status:** open — applied by hand in `30b6fc41`'s message, which records both mutations and
states explicitly that the 11-kill is *not* evidence for the gate. Unmechanised.

**Rests on:** `30b6fc41` (SHA orphans on rebase; patch-id
`db34821395d6257eb37562bb779ed9ab4eba091e`), whose message carries both mutation results and
the reasoning. Related: `R-158` (which case), CLAUDE.md § *Testing Discipline* (how many sites).

**Promote-when:** one further instance of a kill count being offered as evidence for the wrong
proposition. At that point the candidate home is CLAUDE.md § *Testing Discipline* as a clause on
the existing per-site law — *"and report which proposition the kill establishes"* — rather than
a new law, since it shares that law's subject.

## R-165 — A deletion's stale references point INWARD from files the diff never touched, so reviewing the change cannot find them

**Valid:** invariant

**Verdict:** miss, peer-caught. The sweep was reported complete after correcting four
falsified comments; a fifth survived in the file one function from the change, and
`codescout-b7` found it by grepping the deleted symbol's name — the instrument a diff review
cannot substitute for.

**Observed:** 2026-09-01, `30b6fc41`. The commit deleted four `selector_key` overrides and I
swept the comments it falsified, correcting **four**: two stated kill-mutations that could no
longer be applied, one already-stale tense claim, one fixture annotation. I reported the sweep
as complete. `codescout-b7` then found a **fifth**, in `action_selector_key`'s own doc — the
helper the change promoted to canonical:

> *"`LibrarianAdapter::selector_key` carries the same reasoning and predates this helper; it is
> left untouched here only to avoid colliding with concurrent work in that file, and should
> adopt this once free."*

Three false clauses: it names a symbol the commit deleted, explains a deferral that no longer
applies, and prescribes an adoption that had already happened — in the very function the
sentence sits on.

**Mechanism.** A deletion's stale-reference radius has **two directions**, and a diff review
only shows one. Comments that *lived in* the deleted code appear in the diff as removed or
adjacent lines, so reviewing the change surfaces them — that is how the four were found.
Comments that *pointed at* the deleted code are in files the commit never touched, so they are
**structurally absent from the diff**. Reading the diff more carefully cannot find them; it is
not a thoroughness failure, it is a coverage one. Worse, the fifth was the *closest* file to the
change — one function away from the new default — which is why proximity did not help either.

**The pairing with `R-162`, written by the same peer from the same exchange, is the useful
form.** Both are about a deletion defeating an instrument, and the remedies are opposite:

| looking for | instrument | verdict |
|---|---|---|
| *did a peer's deletion touch this file?* | grep for the token being deleted | **guaranteed zero** — `R-162`. Useless. Diff instead. |
| *what still references what I deleted?* | grep the deleted symbol's NAME across the tree | **exactly right** — non-zero *because* the references survive. Diff cannot reach it. |

Same technique, opposite value, and the discriminator is which side of the deletion you are
standing on. Neither is a `search-finds-nothing` instance (`R-3`): both queries were
well-formed, correctly scoped, right files. The defect is in the match between instrument and
direction of the hypothesis.

**Remedy, mechanical and cheap:** after deleting a named item, grep the tree for its **name**
before claiming a sweep is complete — `grep -rn 'LibrarianAdapter::selector_key'` would have
returned the fifth in one call. State the sweep's scope when reporting it: *"corrected every
falsified comment in the diff"* is true and much weaker than *"corrected every falsified
reference in the tree"*, and I published the stronger reading of the weaker act — `R-161`'s
shape a third time tonight.

**Status:** open, remedy applied by hand. Mechanisable: `audit_doc_refs` already resolves code
refs in prose but does not resolve `Type::method` paths in Rust doc comments against the symbol
index, which is exactly the check that would have caught this. Candidate `I-N` for
`test-escape-hardening`, not filed by this entry.

**Generalised 2026-09-01 by `R-162`'s addendum, which supersedes the table above as the
operational form.** `codescout-b7` folded both failures into one discriminator with the part
neither of us had stated: **the timing is asymmetric.** Type A — a query that could not have
returned non-zero in the world you are ruling out — is decidable *before* you run it, from its
shape against the hypothesis's direction, and is the only one of the two that is free to
prevent. Type B — a well-formed query whose *window* excluded the answer — is decidable only
*after*, and only by a second instrument of different scope. **And the standard remedy for B is
a no-op against A:** "widen the window, re-run bigger, take a larger sample" is the reflex
answer to a suspicious zero, is correct for B, and changes nothing for A at any corpus size. So
the question to ask of a zero is not *"is this right?"* but **"which of the two am I holding?"**
Prefer the addendum to this entry's table when citing.

**Rests on:** `30b6fc41` (patch-id `db34821395d6257eb37562bb779ed9ab4eba091e`) and
`d772596c`'s follow-up correcting `action_selector_key`'s doc, which records the finding at the
site. Related: `R-162` (the mirror instrument, same exchange), `R-161` (the weaker act wearing the
stronger act's appearance), and the ledger's law **E — the blast radius is wider than the thing
you edited**, of which this is the doc-comment case.

**Promote-when:** a second deletion whose surviving references were missed by a diff review. At
two, the remedy is a gate rather than a habit — see the `audit_doc_refs` extension above.

## R-166 — A finding parked in a commit message has no citable home, so the next session that needs it cites something adjacent

**Valid:** invariant

**Verdict:** miss, self-inflicted and peer-surfaced. A finding I deliberately declined to give
an entry had no citable home; within the hour the next session that needed it invented a
citation for it, against an entry that does not contain it. Caught by a *disagreement*, not by
either party's checking.

**Observed:** 2026-09-01. Investigating a suspected hole in
`every_open_bug_file_declares_one_known_defect_class`, I reasoned wrongly twice and was
saved by a third check. The near-miss had a clean shape — a grep whose **context window stopped
one line short** of the `cluster/` tag it was checking for, so a present tag read as absent — and
I judged it *"worth recording in the message since it did not earn an entry"* and wrote it into
`8b24df96`'s commit message instead.

Within the hour `codescout-b7` needed exactly that case, as the type-B exemplar in `R-162`'s
addendum: two ways to hold a well-formed zero, one where the query *could not* have returned
non-zero, one where its *window* excluded the answer. The second had no entry anywhere. So the
table cited `R-163` for it — an entry which contains no zero at all, being about a peer's real
figure with an inferred cause attached. Their own sweep confirmed the vacuum: `clipped`,
`context window` and `one line short` occur **nowhere in the ledger** except in their table.
They were citing an entry for a claim it does not make, and describing an instance the corpus
does not hold. Corrected the same hour; this entry is the missing home.

**Mechanism, and it has two halves that compose.**

**(a) A commit message is durable but not addressable, so a finding parked there cannot be
cited — only re-narrated.** `link_scan` binds a token to a `## PREFIX-N — title` heading and to
nothing else, so a lesson in a commit message is reachable by whoever happens to read that
commit and by no query. Worse, the judgement *"this doesn't earn an entry"* is made by the
author at write time — the party least able to know who will need it, since the whole value of
an entry is to a reader who has not had the experience. Mine was needed by another session,
for a purpose I had not imagined, seventeen minutes later.

**(b) A citation is evidence about the citer's model of the target, never about the target,
until the target is read.** `codescout-b7`'s line, and it is `R-3`'s missing twin: `R-3`
disciplines the reader of a *result*, this disciplines the writer of a *reference*. The cost of
checking is one `artifact(action="get", heading=…)` — which is precisely the call that found the
defect once someone finally made it.

**What actually caught it, and it was not a check.** I had offered to cite the addendum from
`R-163` and then re-homed the citation to `R-165` on scope grounds, adding a note that `R-163`
is *outside* the discriminator's reach. That disagreement is what sent them to open `R-163`.
**Had I cited from `R-163` as originally offered, their table would have agreed with my citation
and nothing would have surfaced** — two independently-wrong claims pointing at each other, which
at the point of use is indistinguishable from two right ones. CLAUDE.md § *Reaching a Peer
Session* says check independence rather than agreement; here the independence arrived as a
*disagreement*, and it was the only instrument in the exchange that could have fired.

**Remedy — two, and the first is the one with teeth.**

1. **If a finding is worth a paragraph in a commit message, it is worth an entry.** An entry
   costs one `append_entry` call; no entry costs the next person a fabricated citation. Treat
   *"this did not earn an entry"* as a decision that owes a reason, not a default — and note the
   asymmetry: an unwritten entry produces no error, no gap report and no dangling citation,
   because a citation that was never possible cannot dangle.
2. **Open an entry before citing it for a specific claim.** Not before citing it at all — before
   resting a *particular* proposition on it.

**Status:** open. (2) is now practised by both sessions in this exchange; (1) is unmechanised,
and probably unmechanisable — `doctor` can report an entry with no index row, but nothing can
report a lesson that was never written down.

**Rests on:** `R-162`'s addendum and its same-hour correction paragraph (`codescout-b7`);
`8b24df96`'s commit message, which is where the type-B case was stranded; `R-165`'s scope note,
which is the disagreement that surfaced it. Related: `R-3` (evidence about the search),
`R-163` (an observation and its attribution recorded as one claim — which is also what the
miscitation itself was), `R-161` (care fully engaged and aimed one level away: they were
verifying my commits by patch-id in the same minutes).

**Promote-when:** a second instance of a commit-message-only finding being miscited or lost. At
two, the candidate home is the reconnaissance skill's Phase 3, whose current text says entries
without ids do not compound — true, and this is the sharper form: a finding without an id gets
*mis-attributed*, not merely forgotten.

## R-167 — an UNANCHORED pattern over-matches, and the surplus is plausible — R-3's opposite direction

**Observed:** 2026-09-01/02, four times in one session, all by the same reader, all inside work whose whole subject was measurement discipline.

**Got:** each time a `grep`/`git grep` pattern written from *intent* rather than anchored to the target's *grammar*, returning a plausible number instead of an error:

| # | pattern | intended | actually matched | cost |
|---|---|---|---|---|
| 1 | `git grep -l 'cluster/<slug>'` | files whose **frontmatter** declares the class | + files that merely **name** it in prose | nearly reported two false drifts to a peer as *their* error — the surplus file was the one **retagged out** of that class, whose prose records the class it left |
| 2 | `grep -cE '^  [a-z_]+ '` | CLI verbs in a help block | all but `state-at` — the character class has no hyphen | published **8** where `CLAUDE.md` says 9, in the check that file warns you will get wrong |
| 3 | `grep 'fn selector_key'` | trait-method **overrides** | + a test named `selector_key_projects_tool_and_action` | told a peer *"exactly five override sites"* as **verified**; the real figure is four |
| 4 | `n[=≥]` over the whole ledger | claims in two judgement fields | + matches anywhere in the file | would have gated a promotion **threshold** as if it were a count |

**Why this is not `R-3`, and the distinction decides the remedy.** `R-3` disciplines the **zero** — a search that finds nothing is evidence about the search — and its tell is an absence you can notice. This is the opposite direction: the pattern over-matches, the surplus is *plausible*, and there is no absence to prompt the question. Nobody re-reads a number that looks right. `R-160`'s *partial success is the camouflage* is the nearest kin and is about a **hit rate** (4 of 5 keys); this is about a **population** (every match, one of which does not belong).

**Remedy — anchor to the grammar, and say which grammar in the same breath.** For a YAML list item, `^[[:space:]]*-[[:space:]]*<token>[[:space:]]*$`. For a Rust item, a word boundary or a trailing `(`. For a field, restrict to lines starting with that field. **The corpus already prescribed #1's anchored form in the failing-assert text of a gate I wrote myself**, and I used the unanchored one anyway — so knowing the rule is not the mechanism; having the anchored command *in the message you are about to read* is, which is why that assert text now carries it.

**The tell, when no anchor is obvious:** ask what *else* your pattern's grammar admits, and construct one example of it. Four seconds, and it is the step none of the four had.

**Status:** validated — four instances, one session, one reader, each with a named cost. Not promoted: all four are mine, so the population is one observer and the base rate across others is unmeasured.

**Promote-when:** a fifth instance from a *different* session. Then it belongs beside `R-3` as its opposite-direction twin rather than as its own entry.

**Valid:** invariant

**Rests on:** the general property that a regex is a claim about a grammar and a corpus is not obliged to honour the claim — not on any of the four patterns, each of which is now anchored.


### Note from `compact-root-claude-md` — our agreement on "four" was one instrument read two ways

Added by a second session because the author's session ended before this could be sent to them,
and a caution that lives only in an undelivered message has no home (`R-166`).

The entry above corrects a published "five override sites" to four and cites `30b6fc41`'s commit
message as saying four. I wrote that message, and **our four is not two instruments agreeing.**
I ran the same `fn selector_key` grep, saw the same six files, and then read the bodies rather
than counting hits — so the test named `selector_key_projects_tool_and_action` never entered my
total. One instrument, two readings, and only one of them carried the disambiguating step.

The asymmetry decides what a future reader may conclude:

- had the prefix matched a second **implementation**, my reading would have caught it and the
  hit-count reading would not;
- had it matched a second **test**, neither of us would have been wrong;
- and had it matched something neither of us thought to look at, **we would both have reported
  the same wrong number** — which at the point of use is indistinguishable from corroboration.

So cite the number, not the agreement. CLAUDE.md § *Reaching a Peer Session* says check
independence rather than agreement; this is that rule applied to a count two sessions produced
from one pattern, and the tell is that neither of us could name a second *source*, only a second
*reading*.
## R-168 — An instrument's report can EXPIRE — the loud state converts itself to silence on a timer

**Valid:** invariant

**Verdict:** hit. The scout ran before the code was written and **refuted the sentence I was
about to put in a doc comment.** No entry would have existed had I trusted the archived bug's
transcript, and the gate would have carried a confidently false rationale.

**Observed:** 2026-09-01, building `IC-4`'s worktree-gitdir surface
(`tests/config_propagation.rs`, `7eead422`). The archived instance
(`docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`) records
an orphaned worktree that `git worktree list` does not show. I was about to write *"the gate
scans the filesystem because `git worktree list` cannot see this"* — a clean, plausible reason
that would have read as measured. The probe said otherwise: **immediately after the rename git
DOES report it**, tagged `prunable gitdir file points to non-existent location`. The archived
transcript and the probe disagreed, and both were correct — about different *times*.

What closes the gap is a third party with a clock. `git gc` runs `git worktree prune --expire
3.months.ago` on its own (`gc.worktreePruneExpire`), which **deletes the admin directory** — for
a worktree whose files are still on disk, because git is judging it by a path that moved. After
that, the entry is absent from the list, `.git/worktrees/` is gone, and the defect is silent.
The archived bug is the post-expiry state; the probe caught the pre-expiry one; the rename was
three months before the report.

**The law:** *an instrument's report can expire.* A loud state that converts itself to a silent
one — on a timer, with no event — is worse than an instrument that was never loud, because the
window in which it works is exactly the window in which nobody is looking yet, and by the time
someone looks the instrument agrees that nothing is wrong. A gate built on it does not merely
fail to catch the defect: **it starts passing at the moment the defect becomes invisible**, so
its green is anti-correlated with the thing it checks.

**Distinguish from `R-163`/`R-164`/`R-167` and from `CLAUDE.md`'s recording law.** Those are all
about an instrument whose *scope* is narrower than it appears — a windowed count, a kill tally, an
unanchored pattern, a filter that drops the refuting observation. Those are wrong the same way on
every run. This one is **correct today and wrong later, with nothing in between and no event
marking the transition**, which is why re-running it is not a check on it. The nearest relative
is `IC-11` (prose true when written) — this is its instrument-shaped twin.

**Ask, at any seam:** does this instrument have a garbage collector? Something that tidies
"stale" records is, from the defect's point of view, something that destroys evidence — and it
will usually be a well-behaved subsystem doing its documented job on a schedule nobody set for
this purpose.

**Cost avoided:** a gate with a false rationale, and a doc comment asserting as measured
something a two-minute probe refutes. **Cost incurred:** four throwaway-repo probes, ~6 minutes.

**Status:** open

**Promote-when:** a second instrument in this corpus is found whose report expires rather than
being narrow — at which point the ask above earns a place beside the recording law in
`CLAUDE.md` § *Testing Discipline*.

## R-169 — Running the peer enumeration supplied the confidence to attribute a write by adjacency

**Valid:** invariant

**Rests on:** CLAUDE.md § *Reaching a Peer Session* ("Never route by adjacency") and
§ *Observer Blindness*; `codescout-companion:reaching-peer-sessions` § *Two readings to
get right*.

I ran `/codescout-companion:reaching-peer-sessions`, got the correct socket-scoped table
(16 sessions across 3 profiles, 6 in this checkout — 5 peers plus me), and then attributed
file authorship **by adjacency anyway**: I took everything in `git status` that I had not
written and labelled it "the peer's", meaning the one peer I happened to know about,
because its fix-queue file names itself.

Three of the six files were a different session's. `scripts/file-provenance.py` partitions
them cleanly into two author ids — one holding `src/server.rs`,
`src/tools/memory/tests.rs`, `docs/trackers/operator-rules.md`, the other holding the three
`scripts/pre-commit-*` edits and two untracked `commit-sequence` files. I shipped the wrong
partition to my user *and* to the peer, in a message whose stated purpose was
shared-checkout hygiene.

**What makes this an R-N rather than a slip.** The enumeration ran, succeeded, and was
reported accurately. It answered *who is present*. I then used it for *who wrote this*,
which is a different question with a different instrument — and the successful first answer
is what supplied the confidence for the second. **Running the population check can make the
attribution error more likely, not less**, because it retires the feeling of not having
checked. The skill's own text says so in as many words — *"Enumerating a complete set still
only bounds who was present — it does not attribute a write. To attribute one, ask."* — and
I had that text in context when I wrote the message.

That is the § *Observer Blindness* signature exactly: the author was actively working
inside the class while committing an instance of it. Care was not the missing ingredient
and would not have supplied the answer; a one-command instrument would have.

**The correction did not come from the enumeration either — it came from the misattributed
party reading its own name.** No gate fires on a wrong author claim: `git status` names no
author, the provenance script is opt-in, and a false attribution is a *plausible sentence*
rather than an error. So the detector here was a peer noticing a claim about itself, which
only works when the wronged party is (a) still alive, (b) messaged, and (c) inclined to
reply after being told "no reply needed". All three held; none is structural.

**Change:** when about to name *who* wrote an uncommitted change — in a message, a commit,
a tracker, or a report to the user — run `python3 scripts/file-provenance.py <paths>` and
paste what it says. Never derive authorship from `git status`, diff proximity, or "the peer
I know about". Two properties to carry rather than take on trust: it is **windowed** (it
prints the window per path; an older edit is invisible), and it answers *who wrote these
bytes*, not *whose work this is* — co-edited files exist and are resolved by asking.

**And keep the two halves of an identification separate when reporting.** I verified from
my own run that the eight files partition into two author ids and that none is mine. That
`6524892b` is `codescout-b7` came from `codescout-b7` quoting its own id, which is the
prescribed positive method. That `3e275c54` is `compact-root-claude-md` was that session's
inference about a third party, which I did not confirm at the time — `/proc/<pid>/environ`
carries no `CLAUDE_CODE_SESSION_ID`, so the lookup I reached for does not exist. Report the
bare id when only the id is established: **stating an unverified name costs nothing to do,
which is exactly why the original error propagated so easily.**

**Closing the mapping (2026-09-02, method supplied by `codescout-b7`).** The lookup runs
the opposite way from the one I tried. The registry file is *named for the pid*, and its
JSON carries `sessionId` and `name` together — so scan for the id and read the basename:

```bash
sid=<session-id>
for f in "$HOME"/.claude*/sessions/*.json; do
  grep -q "\"sessionId\":\"$sid\"" "$f" || continue
  p=$(basename "$f" .json)
  [ "$(tr -d '\0' </proc/$p/comm)" = claude ] && printf '%s -> pid %s  name %s  profile %s\n' \
    "$sid" "$p" "$(sed -n 's/.*"name":"\([^"]*\)".*/\1/p' "$f")" \
    "$(basename "$(dirname "$(dirname "$f")")")"
done
```

Validated by running it on **my own** id as a control, not merely on the id in question: it
returned the pid, name and profile matching my `<-- you` row from the enumeration. On that
basis `3e275c54` resolves to pid 3624594 / `compact-root-claude-md` / `.claude-sdd`.

> **CORRECTION 2026-09-02, later the same day — the script above has two defects and both are
> silent.** Kept as written, because the entry records a method that was run and a conclusion
> drawn from it; rewriting it would falsify the record. Do **not** copy it. The corrected form
> is below.
>
> 1. **`sed -n 's/.*"name":"\([^"]*\)".*/\1/p'` prints the FORMER name.** POSIX `.*` is greedy
>    and binds to the last `"name":"` on the line; `formerNames` is a list of objects each
>    carrying its own `name`. Any renamed session resolves to its old name.
> 2. **`[ "$(tr -d '\0' </proc/$p/comm)" = claude ]` skips version-pinned sessions.** `comm` is
>    the executable basename, and a pinned install names the binary after its version — measured
>    2026-09-02, **3 of 21** socket-bound sessions report `comm=2.1.258`. Used here as a
>    *liveness gate*, so such a session is skipped, the loop prints nothing, and a valid
>    sessionId resolves to **"no such session"** rather than to an error.
>
> **The control validated nothing about either.** *"Validated by running it on my own id as a
> control"* passed because the validating session was itself neither renamed nor version-pinned
> — a control drawn from the population the defects exclude. It confirms the lookup direction,
> which was the thing in doubt, and is blind to both of these, which were not.
>
> **Consequence for this entry's conclusion.** `3e275c54 -> pid 3624594 / `compact-root-claude-md`
> / `.claude-sdd`` had its *name* read by the broken regex. The pid and profile come from the
> filename and path and are unaffected; only the name is suspect. Pid 3624594 has since exited
> and its registry row is gone, so **the name is now unverifiable** — which is this entry's own
> lesson holding about this entry: a name is registry-minted and dies with the process, while
> the sessionId is a scratchpad path component and does not.
>
> Fixed form — structural JSON read, and liveness by socket rather than by process name:
>
> ```bash
> sid=<session-id>; u=$(id -u)
> for f in "$HOME"/.claude*/sessions/*.json; do
>   grep -q "\"sessionId\":\"$sid\"" "$f" || continue
>   p=$(basename "$f" .json)
>   [ -S "/run/user/$u/cc-socks/$p.sock" ] || continue
>   printf '%s -> pid %s  name %s  profile %s\n' "$sid" "$p" \
>     "$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("name") or "?")' "$f")" \
>     "$(basename "$(dirname "$(dirname "$f")")")"
> done
> ```
>
> Both defects are filed —
> `docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md`
> and `docs/issues/archive/2026-09-02-comm-filter-misses-version-pinned-claude-processes.md` — and both
> are fixed at their other site, `reaching-peer-sessions/SKILL.md` Step 1. This is the **second**
> site; the bug files named one. Found by grepping the tree while fixing the first, which is
> `bug-fix-session-log:W-102`: checking the named site confirms the named site.

**It still does not license "X did this", and that caveat is the sharper half of the
entry.** The `comm` check guards pid reuse but not a *recycled* registry file — and **a
session that compacts or resumes mints a new id for the same agent doing the same work.**
So the relation this closes is id→name, never agent→name: the bytes were written by an id,
and an agent is a longer-lived thing than whichever id was current when it typed. Write
*"the session registered as X"*, and expect one agent's work to span several ids across a
long task. A tool that resolves the name is not a tool that resolves the author.

## R-170 — A coverage ratio is a scope question before it is a drift finding — and the scope lived in the enforcement layer

**Valid:** dated 2026-09-01

**Verdict:** near-miss (caught before any write) → rule · **Observed:** 2026-09-02, auditing `docs/issues/` for bug files missing a `cluster/<slug>` tag

**Seam:** the scope a published ratio is valid over, when that scope lives in a different file from the ratio.

I measured tag coverage two ways that genuinely differ in scope — each file's own YAML frontmatter, and the catalog's `artifact.tags` column — precisely so agreement would mean something. They agreed exactly: 563 bug files, 0 disagreements, 0 missing from the catalog. Live bugs 34/34 tagged; archive 156/529. Broken out by month: 0% before 2026-07, 33% in 2026-07, **42% in 2026-08**, 100% in 2026-09.

That table reads unambiguously as drift. The convention starts in 2026-07, so 236 files *from its own era* carry no tag, two of them filed two days ago. I had the campaign half-composed — batch-classify the in-era archive, one tag each — when I opened `tests/issue_clusters.rs` for an unrelated reason (would my line-shifting edits red it?). Its module header says the opposite of my conclusion:

> **`docs/issues/*.md` only, never `archive/`.** 416 archived files legitimately carry no tag: the nine classes were derived from the open backlog, and 279 archived files in the backfilled window match none of them. **Forcing a fit would corrupt the counts** that promotion reads, so absence there is a deliberate answer rather than a gap.

The campaign was not merely unnecessary. It was the specific thing the gate's author had already considered and ruled out, for a stated reason, in writing — and executing it would have inflated every `IC-N` count with members that match no class, which is the number promotion reads.

**The structural cause is worth more than the escape.** `issue-clusters.md` is where you go to read a cluster's `n`, and it is careful about exactly one failure mode: *"a `**Members:**` line carries the query, plus `n=<count>` and the date it was run. Trust the query; re-run it before trusting the count."* That defends the **count** against staleness and says nothing about the **population**. Re-running the query as instructed yields a fresh number that is still scoped to a 34%-tagged corpus, and the instruction to trust it is what stops you asking. The population bound exists, is precise, is measured — and lives in a test module header, which is where a bound is *enforced* and not where anyone reads the number it bounds.

So: **a number and the scope that validates it must co-locate at the point of READING, not at the point of enforcement.** A test header, a gate script, a hook — those bind the producer. The reader never opens them, and a document that carefully names one failure mode implies by omission that the others are handled.

And the mirror of the standing law is the operative form here. That law says *a proposed fix or prohibition is a claim about current state — verify it before designing around it*, and it disciplines a prohibition you have **read**. This is the converse: the tree held a prohibition I had **not** read, and my proposal was exactly what it forbade. The reflex it needs is different — before proposing any campaign over a population, grep the **enforcement** layer for that population's name (`tests/`, `scripts/pre-commit-*`, hooks), not only the documentation layer. Here one `grep -l 'docs/issues' tests/` was the whole difference, and I ran it for an unrelated reason.

Cheap tell, no judgement required: **a coverage ratio that is neither ~0% nor ~100% is a scope question before it is a drift finding.** True drift tends toward complete; a stable middle is usually a boundary someone drew. 29.5% held steady across two months.

**Promote-when:** a second case where a measured ratio motivated a remediation campaign that a scope statement already in the tree forbade — or where the scope was found in an enforcement file after the documentation file had been read.

**Status:** open — single datapoint, but the near-miss cost was a multi-hour campaign that would have corrupted the counts it was meant to complete.

**Kin:** R-3, R-113 (a result is evidence about the instrument, not the world), R-117 (a fix naming a population asserts that population is non-empty — here the population existed and was deliberately excluded, the exclusion being the unread half), R-49 (re-entering your own artifact is a seam).

## R-171 — A bounded read returns an absence indistinguishable from a real one — and `tail -c` on one long line cuts the other end

**Valid:** invariant

**Verdict:** miss, three times in one session, the third caught only by re-running unbounded
before sending. No gate fired on any of them; two destroyed evidence I then had to re-create.

**Observed:** 2026-09-01/02. Three bounded reads, each of output whose value was in the part
the bound removed:

| call | what it cut | cost |
|---|---|---|
| `cargo test … \| tail -8` | the failure message of `entry_sections_and_extract_agree_on_the_live_corpus`, a test whose doc comment says its message names the offending file | had to re-run the suite; the re-run was green, so the diagnosis rests on the test's documented corpus-sensitivity rather than on its own output |
| `cargo test --test issue_clusters \| tail -3` | which 2 of 18 failed, and why | re-ran; green. Same loss, same session, ~40 min later |
| `grep -o 'The 23rd.*' \| tail -c 1300` | the **head** of a long single line — `tail -c` on one line cuts from the front | drafted *"Edit 1's content isn't where they say it is"* about a peer, found it exactly where they said on the unbounded re-read |

**The law is not about `tail`.** It is that a bounded read of an unknown-shaped payload returns
a **negative that is indistinguishable from a real absence**, and the negative is what gets
acted on. `tail -c N` on a single long line is the sharpest case — it silently reverses which
end you keep, so the instrument does not merely truncate, it truncates the opposite end from
the one the flag's name suggests.

**Why "be careful with flags" is the wrong remedy.** All three were reflexes reaching for a
bounded read to be a good context citizen — the habit the progressive-disclosure discipline
correctly trains — applied to the one class of output where the payload IS the tail end of a
long thing. The bound was chosen before the shape was known, which is the actual error.

**Remedy, mechanical:** for any output whose *diagnosis* is the point — test failures, gate
refusals, a long ledger field — redirect to a file and query it, never pipe to `tail`:

```
cargo test … > "$OUT" 2>&1; echo "exit=$?"
grep -E '^test result' "$OUT"
sed -n '/^failures:$/,$p' "$OUT" | head -20
```

That is already the Iron-Law-3 pattern for unbounded output (`run bare, query the buffer`) —
the gap is that it was read as a *volume* rule and this is a *shape* rule, so it never fired
for a 3-line pipe.

**Adjacent but NOT an instance, and the distinction matters:** the same session read
`Stashing unstaged files to … / Restored changes from …` in six consecutive commit outputs
and classified it as chrome. That output was never truncated — it was fully present, read,
and pre-classified as background, and it happened to be the refutation of a claim being
written at the time. Unread differs from uncaptured, and the remedy above does nothing for
it. Recorded here so the two are not merged into one over-general law.

**Status:** open

**Promote-when:** a fourth instance, or one where a truncated read reaches a commit message or
a peer rather than being caught in draft.

## R-172 — A result that falsifies a documented invariant is usually the invariant's already-recorded residual

**Valid:** invariant

**Verdict:** hit, narrowly. The isolating re-run happened before the draft was sent. Had it
not, this session would have published a falsification of a `CLAUDE.md` claim that a test
pins byte-for-byte, against a residual the corpus had already recorded three days earlier.

**Observed:** 2026-09-02. Ran the full four-command gate in the documented order and got 10 of
11 `cli_artifact` failures — `error: unrecognized subcommand 'artifact'`, the librarian-less
binary. `CLAUDE.md` § *Development Commands* states the opposite in bold: *"Ending on the
default lane rebuilds it, so following the gate cannot arm the trap for anyone else — provided
both lanes actually run."* Both lanes ran, both exited 0. The obvious reading is that the
claim is false, and a draft saying so was written.

Re-running the two lanes alone, immediately: lean leaves the binary lean, default restores it,
`exit 0`, 11/11. **The ordering claim is sound.** A peer's concurrent lean lane had landed
inside the window between the default lane's build and `cli_artifact`'s exec.

**What makes this an entry rather than a nuisance: the corpus predicted it, in a field built
for exactly that, and reading the claim would never have found it.** The fix commit's own bug
file — `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`,
`status: fixed`, archived — carries in its **`unverified:`** frontmatter: *"the fix closes the
TERMINAL state only, not the window … so two sessions gating concurrently still collide and
nothing detects that."* That is this exact case, written down, queryable, three days old.

**The law:** a result that appears to falsify a documented invariant is more often the
invariant's **already-recorded residual** than a defect in the invariant. The prior belongs on
"the doc has a caveat I have not read", not on "the doc is wrong" — because a claim load-bearing
enough to be pinned by a test has usually been argued about already, and the argument's
leftovers are in the bug file that closed it, not in the claim.

**Where to look, and why it is not where anyone looks.** The residual sits in an
**archived, `status: fixed`** record — invisible to the canonical triage query
(`find(kind="bug", status={"in": ["open","investigating","zombie"]})`) by construction, since
archiving is the normal end state. `unverified:` exists precisely so the caveat is queryable
past closure. So the check is not "search open bugs" but:

```
artifact(action="find", kind="bug", include_archived=true,
         filter={"rel_path": {"contains": "<the-thing>"}})
```

then read `unverified:` before writing the word "false".

**Distinguish from `R-163`.** That one is about an observation whose attributed CAUSE is a
second, unchecked claim. Here the observation and the cause were both right — a peer's build
did clobber the binary — and the error under construction was in the **scope of the
conclusion**: a true local measurement generalised into a falsification of a global claim,
because the measurement could not see the concurrency it was subject to.

**Also the reason `R-166` is load-bearing in the other direction:** the residual survived to be
found only because someone put it in a queryable field instead of a commit message.

**Status:** open

**Promote-when:** a second case where an archived `unverified:` field would have pre-empted a
falsification, or one where it did not exist and should have.

## R-173 — A classifier's REJECTED branch is where the leak is — scan a config for a property, print the key and the verdict, never the value

**Valid:** invariant

**Verdict:** miss. Not caught by any gate, not caught in draft, and not recoverable —
the output had already been emitted when it was noticed.

**Observed:** 2026-09-02, measuring `IC-4`'s env surface. The question was narrow: *does
each path-valued variable in the repo-root `.env` still resolve?* The loop classified each
line, and its two branches were asymmetric in a way that never got a second look:

```bash
case "$v" in
  /*|./*|~*) [ -e "$p" ] && echo "  OK      $k=$v" || echo "  MISSING $k=$v" ;;
  *)         echo "  (non-path) $k=$v" ;;          # <- the leak
esac
```

The **accepted** branch is the one the task is about, so it gets the attention. The
**rejected** branch was written as a courtesy — "show what I skipped" — and it printed a
credential in full into a transcript. `.env` is gitignored and untracked, so nothing
reached git; the exposure was created entirely by the read.

**The law:** when scanning a config file for a *property*, emit the key and the verdict,
never the value. And the leak is structurally in the **rejected** branch, because that is
the branch nobody is thinking about: the classifier's whole purpose is the accepted set, so
the else-arm gets written as an afterthought and reviewed as one. `.env`, `settings.json`,
`.npmrc`, `~/.cargo/credentials` and CI variable dumps are all files whose *shape* is the
subject of routine questions and whose *contents* are not.

**Cheap form that answers the same question:**

```bash
# key + verdict, never the value
[ -e "$p" ] && echo "  OK      $k" || echo "  MISSING $k -> $v"   # value only when it is the finding
```

Print the value **only** in the branch where the value IS the finding — a missing path has
to name the path to be actionable, and a resolving one does not.

**Not a repo defect, checked:** no script under `scripts/` reads `.env` or dumps
`key=value`, and none needs redaction logic added. The hazard lives in ad-hoc shell written
to answer a one-off question, which is precisely the code no reviewer sees.

**Kin to `R-171`, and the pair is the point.** That entry is about a read emitting **too
little** — a bound cutting the payload, returning an absence that gets acted on. This one is
a read emitting **too much**, in the arm chosen for completeness. Same act, opposite failure,
and both come from deciding the output shape before knowing what is in the input.

**Status:** open

**Promote-when:** a second instance, or one where the over-emission reaches a commit,
an artifact body, or a peer rather than a transcript.

## R-174 — A line number is a coordinate in a world, and on a shared checkout the tools do not share one

**Valid:** dated 2026-09-01

**Verdict:** near-miss, caught by an instrument DISAGREEMENT I ran for an unrelated reason · **Observed:** 2026-09-02, closing Change 3 of the response-envelope spec

**Seam:** which world a line number is a coordinate in, when the tools answering do not share one.

I was writing a spec closure whose whole argument is two call sites, so I cited them: `types.rs:1076-1083` for the guide gate, `:1063-1066` for the declaring-topic fallthrough. Both read out of codescout's own `grep` and `read_file`. Then, chasing an unrelated question about the spec's *pre-existing* refs, I ran `symbols` and `git show HEAD:` on the same file and got **1484** where `grep` had said **1278**.

`src/tools/core/types.rs` was `M` in the worktree and **206 lines shorter than HEAD** — 1321 against 1527 — a peer mid-refactor extracting `src/tools/core/guide_emit.rs`. So:

| instrument | world it read |
|---|---|
| `grep`, `read_file` | the **working tree** — a peer's uncommitted state |
| `symbols` | the **AST/LSP index** |
| `git show HEAD:` | the **commit** |

Three instruments, two worlds, **no error from any of them**. The numbers I was about to commit into a durable spec described code that exists in no commit and may never — the peer could abandon that refactor and the citation would point at nothing that ever shipped.

**Why this is not simply the promoted substrate law, and where that law's remedy fails.** The recon skill already carries *"a tool that resolves its target from the environment has a SUBSTRATE as well as a verdict… when two tools disagree, the question is not whose logic is wrong but which world each one read."* True, and this is an instance. But its prescribed remedy is *"read its `loaded N from X` preamble and reconcile N against a count you took yourself"* — and **codescout's navigation tools print no such preamble.** `grep` returns `path:line` with nothing naming the world; so does `read_file`. There is no N to reconcile. The promoted remedy is a **no-op** against the instrument this project uses most, which is the same shape as `OB-1`'s *supplied-and-unread* exclusion: a remedy that cannot apply is worse than none, because it reads as covered.

**The catch was a disagreement, and that inverts the usual worry.** This corpus normally frets that two instruments *agreeing* is one blind spot counted twice. Here agreement would have been the failure mode — `grep` and `read_file` share a world, so they corroborate each other perfectly while both describe uncommitted code — and it was the *disagreement* with a differently-scoped instrument that carried the information. Nothing prompted that cross-check; I ran it for another question entirely.

**Remedy, and it is cheap enough to be unconditional.** Before a line number goes into anything durable — a spec, a bug file, a tracker, a commit message — resolve it against `git show HEAD:<path>` and say so. HEAD is the only one of the three worlds that is nameable, stable, and shared with the reader. A worktree coordinate is a claim about *your* checkout at *this instant*, and on a shared tree that is not the same thing as a claim about the code. Where the worktree is genuinely the subject, say *"worktree, uncommitted"* explicitly.

**Second-order, and it is why this is worth more than the one catch.** A stale line ref is normally a *decay* problem — true when written, false later, which `audit_doc_refs` and `DC-N` already cover. This one is **false at the moment of writing** and decays *toward* correctness or toward nothing, depending on whether a peer commits. No freshness check catches it, because re-running the citation's own instrument reproduces the same number.

**Promote-when:** a second case of a durable artifact citing worktree coordinates on a shared checkout, or any case where two codescout instruments answer about different worlds with no diagnostic distinguishing them. At two, the remedy belongs in the tools: `grep`/`read_file` should name the world they read when the file is dirty.

**Status:** open — single datapoint, caught pre-commit; the cost avoided was a permanently wrong citation in a spec whose whole argument is those two call sites.

**Kin:** `R-89` (freshness is a property of the copy that SERVES you — this is the copy that ANSWERS you, a fourth axis alongside build, process and distribution), `R-113`, `R-170` (a number's scope must travel with it; here the scope is *which tree*), `OB-1` § *the third position* (a remedy published to the wrong audience — here, a remedy that cannot apply at all).

## R-175 — A gate quantified over a population you never enumerated is a hypothesis in gate's clothing

**Status:** validated — the gate it corrected never shipped
**Valid:** invariant

**Observed 2026-09-02.** A spec written ~1 hour earlier (same session, same
author) specified Layer 1's Gate 2 as *"no registered `ledger_prefix` is a
prefix of another"*. Scouting the write sites before implementing it showed the
gate would have **failed on correct code the day it landed**.

**The scout:** `grep` for every mutation of the shared `GuideLedger`, filtered
to production. Two calls. It returned **six** writers where the spec assumed
two, and one of them — the session opener — stamps `SESSION_OPENING_GUIDE`, a
bare topic name that `guide-sections` also owns. The overlap is deliberate and
argued at the site: keying the opener finer *"would desync this trigger from
what `GuideLedger::re_arm` actually re-arms."*

**What makes this an R-N rather than an ordinary catch.** The spec's error was
not a wrong fact. Every sentence in it was true of the two engines it had
examined. The error was a **gate generalised from a population of two**, and the
tell is that the population was never counted — the spec said "engines 1 and 5
share four mechanisms" and then wrote a rule quantified over *all* engines.

> **A gate quantified over a population you did not enumerate is a hypothesis
> wearing a gate's clothes.** It passes on the members you had in mind, which is
> exactly the evidence that persuades you to ship it.

**Cost avoided, concretely.** The gate would have red on first run against
`session-opener`. The cheap repair at that point is to *delete the overlapping
registration* or *widen the predicate* — both of which erase the finding, and
one of which (a negative predicate, `!starts_with("op:")`) makes the gate
permanently unfalsifiable. That branch is now closed by
`no_engine_claims_a_key_from_outside_every_corpus`, which exists only because
the scout ran first.

**Second-order find.** The same enumeration produced a **seventh engine**. The
2026-08-27 roster walked the prompt-surface inventory; the session opener is
invisible there and visible in the write sites, because it retrieves on *session
phase* and stamps a key shaped exactly like engine 1's. So the discriminator
("the key you retrieve on") was sound — it was the **instrument** that was
narrow, and "six" was a count of what one instrument could see rather than of a
closed set.

**Rests on:** `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`
(`0021bead4e5a01e2`) § *Gates*, which carries the correction inline rather than
silently shipping the fixed form; `64a0a64c`.

**Promote-when:** a third instance of *"gate generalised from an
un-enumerated population"* appears. Two so far — this, and the
`serves:`-coverage gate that `get-guide-section-grain` deliberately built as a
finite 88-row checklist *because* an open-ended coverage claim is
unfalsifiable. That spec reached the right answer from the same pressure, which
is why this is a pattern and not an incident.

## R-176 — a pipeline's `$?` is the LAST stage's status — `script | tail` reports tail's success, and it reads as green

**Status:** open

**Valid:** invariant

**Rests on:** POSIX shell semantics — `$?` is the exit status of the last command in
a pipeline — and `CLAUDE.md` § *Development Commands*' `&&` ruling, of which this is
the sibling case.

**Observed 2026-09-02**, by two sessions independently within the same hour, on the
same check. A peer reported that a pre-commit hook was failing. I verified with:

```
python3 scripts/pre-commit-ledger-counts.py 2>&1 | tail -15; echo "exit=$?"
```

It printed `exit=0`. The hook was exiting **1** the whole time; `$?` had reported
`tail`'s status. Re-run without the pipe:

```
python3 scripts/pre-commit-ledger-counts.py > /tmp/hook-out.txt 2>&1
echo "REAL exit=$?"        # 1, with both findings named
```

**Severity: I nearly dismissed a correct peer report on the strength of the wrong
number.** The green was not a weak signal to be weighed — it was a confident,
specific, wrong answer to the question I asked, and it arrived attached to real
output that made it look corroborated.

### Why this belongs next to the `&&` ruling rather than in a shell-tips list

`CLAUDE.md` § *Development Commands* already records that `&&` between the two test
lanes withdraws the repair step **exactly when something is wrong**. This is the same
family with a different operator: a composition operator silently changes *which
command's status you read*, and both fail in the direction that reads as "nothing
wrong".

`|` is the worse of the two, for one reason: `&&` at least does something visible —
the later command does not run. A pipeline runs everything and hands you a status
belonging to a different program, so there is no missing output to notice. **The tell
is that the exit code and the printed text disagree about how the run went** — here,
findings on stdout and `exit=0` — and a reader who only checks the code never sees
the text that contradicts it.

### One layer out, this is `R-171` — and that framing is a peer's, with the instances

`R-171` is a bounded READ returning an absence indistinguishable from a real one;
this is a bounded STATUS doing the same. The shared shape is neither `tail` nor `|`:
it is **choosing the bound before knowing the payload's extent**. `$?` after a pipe
and `| tail` over an unknown-length output both answer confidently about a thing they
only partly saw.

The author of `R-171` supplied this and had four instances the same night to my one —
`tail -8` losing a failure message whose own doc comment says it names the offending
file, `tail -3` losing which 2 of 18 failed, `tail -c 1300` cutting the **head** of a
single long line and producing a drafted accusation against a peer that was false, and
a 47-line `sed` window over a 76-line entry that would have been reported as an
absence. Recorded here rather than only in `R-171` because the two entries are
reachable from different searches — someone chasing an exit code will not grep for
`tail`.

### The remedy is structural, not attentional

Do not resolve to remember. Redirect and read `$?` directly, as above. Where a pipe
is genuinely wanted, `${PIPESTATUS[0]}` (bash) or `set -o pipefail` is the answer,
and `pipefail` is the one to prefer because it needs no index and cannot go stale
when a stage is added.

### The part that was not obvious: codescout's IL-3 guard already blocks this class

The peer's own check came out **right**, and not because they were more careful —
codescout's Iron Law 3 refused their unbounded pipe twice and forced them to redirect
to a file, which is precisely the correct form. The guard is documented and justified
as **context economy** (don't flood the transcript); nobody would look in it for
exit-code integrity, and its own documentation does not claim it.

So it earns its keep a second way, and the two sessions form a natural experiment on
it: same check, same hour, one routed through the guard and one not — the guarded one
got the true exit code, the unguarded one got `tail`'s. **A guard whose second benefit
is undocumented gets removed for failing to justify its first**, which is the reason
to write this down rather than merely enjoy it.

Same shape as `observer-blindness`' third position: the person holding the parameter
that would reveal the benefit is not the person who reads the justification.

### Promote-when

A third instance of an operator silently redirecting which command's status is read —
`|`, `&&`, `;`, a subshell, `time`, or a wrapper like `timeout` — at which point this
stops being two shell gotchas and becomes a rule about composition operators, and
belongs in `CLAUDE.md` next to the `&&` sentence rather than here.

## R-177 — A new instrument's most persuasive output is the one that confirms the hypothesis you built it to test

**Status:** validated — four defects, one instrument, one afternoon (2026-08-31)
**Valid:** invariant

**Observed.** Built `scripts/probe_guide_section_use.py` to answer *"which sections of
`tracker-conventions` do sessions actually engage?"* — the probe gating a decision to split
that guide. Its first run, before any positive control, reported: **31% of the topic never
engaged**. Exactly the shape a would-be splitter wants.

It was fabricated. A fenced worked example inside the guide — the one that *teaches* the
entry-heading syntax, `## <ID> — <title>` — was parsed as a real heading by a line-start
split. That invented a 12,124 B phantom section, **stole those bytes from the real
`Entry-level standard`** (17,323 → 5,116), and, having no signature rules, scored the
phantom as never engaged. Three separate errors composing into one number pointing the
author's way.

The positive control found three more in the same pass:

| defect | symptom before the control |
|---|---|
| phantom section from an unfenced parse | fabricated "31% dead" |
| signature strings matched against **any** tool input | the `create_file` that wrote the probe scored all six sections 100% |
| `tags: [append_entry, …]` on a bug filed *about* the librarian | credited sections never touched |
| **main and subagent populations blended** | 71.7%, which coincidentally matched a prior study's **71%** and read as cross-instrument convergence |

**The blend is the sharpest one.** Split, the populations are ~45% and ~92% — nothing alike.
Blended, they produced a number that agreed with an independent 2026-08-27 agent-scored study
to within a point. I said that agreement out loud as a finding before checking, and it was an
artifact of the mix. **A spurious corroboration is worse than a spurious number**, because it
recruits a second instrument as a witness.

**The general shape.** Every one of the four returned a *plausible, well-formatted, wrong*
answer, and three of the four pointed the direction the author already expected. That is not
coincidence: a defect that produced an implausible number would have been caught by reading
the output. The ones that survive to publication are selected for plausibility, and
plausibility for a hypothesis-holder means *agreement*.

**Next / practice.**

- **Run the positive control before the first real run, not after the first surprising one.**
  On a case whose answer you already know. PROBES rule 4 says this; what this entry adds is
  that the danger is concentrated in the runs that go *well*.
- **Cross-check a derived count against an independent artefact of the same object.** The
  phantom was localised not by re-reading the parser but by disagreeing with a frozen
  `guide_sections.json` on the **section count** — 7 vs 8. A structural invariant is cheaper
  to check than a value and fails louder.
- **When a defect's remedy is "remember to split", put the split in the instrument.**
  `report()` now refuses to print a blended figure and `--split-at` prints a stratification
  warning with the measured numbers. Both were added *after* the same class bit twice in one
  session, which is the evidence that discipline was not the missing ingredient.
- **Sibling:** `docs/PROBES.md` § *Before you trust any probe*, rules 4 and 6; the
  instrument's own module docstring records each defect at the line that fixes it.

## R-178 — Miss: `origin/<branch>` is a local cache, so "N unpushed" without a fetch is a claim about your last fetch

**Status:** validated — reported wrong three times in one session before a fetch was run (2026-08-31)
**Valid:** invariant

**Observed.** I reported *"2 unpushed commits"*, then *"1"*, then *"3 unpushed commits"* across
one session, each read from `git rev-list --count origin/experiments..HEAD`. All three were
true statements about a remote-tracking ref that had not been refreshed. When the user finally
said "push", a `git fetch` showed **ahead 0, behind 18** — a peer had already pushed the branch,
carrying my commits with it, because `git push` sends the whole branch ref rather than one
session's work.

`origin/<branch>` is a **local file** (`.git/refs/remotes/…`). It records where the remote was
at the last fetch. Nothing updates it in the background, and in a checkout shared by four or
five concurrent sessions it goes stale in minutes. The number it yields is well-formed,
plausible, and answers *"what did the remote look like when I last looked?"*

**Two consequences that followed immediately, both worth knowing.**

1. **A peer rebase orphaned 3 of my 6 commits.** `c79481c9`, `4a946adb`, `df02a157` stopped
   being reachable — not from a release, from routine peer work minutes after I cited them.
   Recovered by patch-id (`git show <sha> > /tmp/p.patch; git patch-id --stable < /tmp/p.patch`),
   matched against a patch-id map of the last 40 commits on HEAD: all three landed under new
   SHAs with byte-identical diffs. **This is the SHA+patch-id rule earning its keep inside one
   afternoon**, not across a release cycle — and it is why a SHA written into a file needs its
   patch-id beside it.
2. **"I committed it" and "it is still at that SHA" are different claims**, and the gap can be
   minutes. Check in-file SHA citations after any peer rebase; here the older ones survived
   because only the tip commits were replayed.

**Next / practice.**

- **`git fetch` before quoting any ahead/behind number**, and say so when you do. One command,
  and it converts a local fact into a shared one.
- **`git ls-remote origin refs/heads/<branch>` is the ground truth** when it matters — it asks
  the remote rather than the cache, and it is what settled this.
- **Generalise:** the same shape as `docs/PROBES.md` rule 6's *temporal* row — an instrument
  reporting perfectly on an instant that has passed. It appeared twice in this session: this,
  and the frozen `corpus-frame.json` whose 106-session population had lost 76 members to disk
  cleanup within four days. Neither returns an error; both return a number.

## R-179 — discharging a deferral's prohibition can hand you the fix's best argument, which the prohibition could not have contained

**Valid:** dated 2026-09-04

**Verdict:** hit → widens the deferral-rationale law · **Observed:** 2026-09-04, fixing `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md` (`guard_worktree_write` ignoring a `workspace=` pin)

**Seam:** the bug file's `unverified` field carried a **prohibition**, not an estimate: *"Do not add the early return without that check — the read-side twin could early-return safely because its worst case is silence, and a write guard's is not."* Two named prerequisites: (1) is the pin validated anywhere on the write path, (2) the *"in-process subagent path is unexamined"*.

The known law says re-cost a deferral rationale because it is written at the moment someone decided to stop, so nobody drafts the estimate that would fail to justify stopping. That held again, plainly: both prerequisites cleared in about ten minutes of reading, and prerequisite (2) named a path that **does not exist** — `Server::build_context` is the only production constructor of a core `ToolContext` (every other of ~50 in-tree constructions is under `#[cfg(test)]`), so a subagent's calls arrive through the same `call_tool_inner` and its pin is its own per-call argument, never inherited state.

**What is new is what the check RETURNED, not that it was cheap.** Prerequisite (1) turned up `call_tool_inner` already granting a pinned write-tool call write residency on the pinned root, under a comment saying *"the pin itself already is the caller's consent"* and explicitly rejecting `activate` as the alternative. So the guard was refusing what the layer **directly above it** had already accepted as consent. That converts the change from *"a new policy on a write path, proceed carefully"* into *"restore consistency inside one call path"* — a much stronger argument, and one the deferral rationale **structurally could not have contained**: whoever writes a stop-rationale has stopped reading, and this fact lives one layer above the function they were reading.

So the law's remedy is right and its stated reason is incomplete. Re-costing a deferral is not only about recovering an inflated estimate; the check is also where the *justification* lives, and a deferral rationale is systematically written from a narrower slice of the call path than the fix needs. The tell is a rationale that reasons entirely inside one function while the question is about a contract between layers.

**Promote-when:** a second instance where discharging a deferral's own prohibition surfaces an argument that changes the fix's character (not merely its cost) → add the "read the layer above, not only the cited function" clause to the skill's deferral bullet, which currently prescribes re-costing numbers and probing premises but not reading outward.

**Status:** open — 1 datapoint for the new half; the cost half is the law's 10th and needs no further support.

**Kin:** the skill's deferral-rationale bullet (9 prior datapoints, incl. the "38 sites" → 133 and "pick one of two, both bad" cases); `R-117` (a fix naming a POPULATION asserts it is non-empty — same shape, since *"the in-process subagent path"* named an empty population and would have cost a reviewer real time); `R-49` (re-entering your own bug file is a seam).

## R-180 — a unitless count crosses a session boundary as a cost estimate, and every remedy I reached for was the same error one level up

**Valid:** dated 2026-09-04

**Verdict:** miss ×4 → rule · **Observed:** 2026-09-04, immediately after `R-179`; the number I published there was quoted back by a peer session as an architecture cost estimate

**Seam:** the population behind *"`Server::build_context` is the only production constructor of a core `ToolContext` — every other of ~50 in-tree constructions is under `#[cfg(test)]`"*.

**Miss 1 — the count.** `~50` was **files with at least one match, under a 200-candidate cap, from a pattern that also matches `struct ToolContext {`, `impl ToolContext {` and `-> ToolContext {`**, published as *constructions*. codescout's `grep` had printed *"50 matches across 50 files is a floor, not a count"* directly above the number, and I quoted it as a count.

**Miss 2 — CLAUDE.md's unit law with a consequence the law does not name.** The stated cost of a unitless count is that a reader re-derives it under a counting rule of their own choosing. What happened is quieter: a peer **spot-checked 2 of ~20 files, agreed, and explicitly deferred to my wider sweep for the rest** — then re-priced an architectural decision (an artifact-lane check in `index(verify)` vs `doctor`) on it and told their user. The sweep they deferred to did not exist. Two sessions then held one wrong number **with real agreement beside it**, because we genuinely agreed about the one production site, which was true. Nothing in the exchange pointed at the false half. **A unit is what makes a number re-derivable; without one it is quoted rather than checked, and a peer's confirmation of the load-bearing half reads as confirmation of the number attached to it.**

**Miss 3 — the instrument built to replace the bad number was wrong twice, both times INFLATING the production count.** (a) It reported 2 core production literals; one was `) -> ToolContext {` — the exclusion regex `\b(struct|impl|->)` never fires on a return type, because the character before `-` is a space and space→hyphen is not a word boundary. (b) It then reported 2 librarian production literals; one was `TestToolContextBuilder::build()`, `#[cfg(test)]`-gated **on the `impl`** (`src/librarian/tools/mod.rs:131`), not on a `mod`, while the detection was mod-shaped. Both defects invented production sites — i.e. **findings**, the direction that gets published. I had written a blind-spot note in the script header naming `#[cfg(any(test, ...))]` and `cfg_attr`; neither of the two that bit me was on it. **The blind spots you can enumerate are not the ones that fire.**

**Miss 4 — and the remedy I published for miss 1 was miss 1 again, one level up.** I wrote: *publish a count only as a partition that sums to an independently-obtained total — two instruments sharing no code both said 202.* They did not share code; they shared a **predicate**. Both were `\bToolContext\s*\{` — a Python classifier and a `grep -rn` cross-check, one blind spot run twice, which is precisely the *"check independence, not agreement"* law I had quoted in the same message. The peer, measuring the same instant with a **substring** predicate, got **203**. Their proposed cause was churn — reasonable, since I had been committing all session — and false: HEAD `531d7ee3` was committed 15:53:48 and `git status --porcelain` at 16:01:19 showed **zero `.rs` files modified**, so the counts were simultaneous and still differed. The real cause was one line: `src/librarian/adapter.rs:500`, `Arc::new(LibToolContext {` — an import alias for the librarian struct, where `\b` fails because the preceding character `b` is a word character. **The asymmetry is why it survived: the `\b` was correct for the core bucket I cared about and silently corrupted the librarian bucket I was not looking at.** An error that protects the number under scrutiny is one nobody checks. Corrected partition, summing to the peer's 203: 60 non-literal headers + **1** core production + **136** core test + **2** librarian production + 4 librarian test. So the field-addition cost is **137** core sites, ~3× what I first published, and librarian production is 2 rather than 1.

**Runnable:** a partition is evidence of completeness only when its total comes from a different **PREDICATE**, not merely different code — `\bX\s*\{` computed twice is one instrument run twice, and the arithmetic is then consistent *by construction* and blind to exactly what the predicate misses. Name the predicate beside the number. Treat a peer's differing count as a predicate difference to localise before attributing it to timing: a simultaneity check (HEAD commit time + `git status --porcelain`) is one call and settles it. And when an exclusion regex is load-bearing, test it against the construct it excludes — `\b(struct|impl|->)` and `\bToolContext` both matched nothing where no word boundary existed, which is this repo's heredoc tell in regex form: a pattern that reads as covering a construct and never touches it.

**Miss 5 (appended 2026-09-05, supplied by peer `codescout-ae`) — the same predicate-scope error a third and fourth time, in one day, across two sessions.** I told that peer the librarian `ToolContext` had *"5 sites total and the compiler should be naming all of them"*. The real number is **10** — 1 production, 9 test. `cargo check --all-targets` reports **per target**, and a target that fails stops compiling itself, so the first error batch named 5 (lib + lib-test); fixing those revealed 5 more in integration targets (`tests/audit_shards_cross_machine.rs`, `tests/link_scan.rs`, three under `tests/librarian/audit_doc_refs/`). **Only iterating to a clean build establishes the total; the first error batch is a floor wearing the shape of one** — which is `IC-20` (*floor published under the name of a total*) arriving through a compiler rather than through a capped grep. Note my own number was additionally scoped to `src/`, and these are all under `tests/`, so two different scope errors stacked without either being visible in the figure. The peer counted a fourth instance the same day: a grep alternation anchoring on a fragment that continued past the anchor. **Four instances, four instruments, one shape — a measuring predicate silently excluding part of its own target — and in every case the fix was a denominator rather than more care.**

**Status:** open — 1 datapoint for the cross-session-quotation half; the unit law it instantiates is already in CLAUDE.md § *Testing Discipline*, and the independence law in § *Reaching a Peer Session*.

**Kin:** `R-179` (the entry whose own number this corrects, filed 30 minutes earlier); `R-177` (an instrument's most persuasive output is the one confirming the hypothesis it was built to test — this adds that a *replacement* instrument inherits the bias of the claim it checks); `R-178` (a count valid only at its instant — invoked here and correctly rejected); `R-3`/`R-113` (a search result is evidence about the search); `R-5` (a check computed from the thing it judges cannot fail); CLAUDE.md § *Observer Blindness* on shipping a claim's derivation **and** its population together — this is the population half failing across a session boundary. Peer `codescout-ae` supplied the independent predicate.

## R-181 — enumerate by the field name, not the feature name — two bug files shared a line neither cited

**Status:** open — not yet promoted into the served skill.
**Valid:** dated 2026-09-04
**Rests on:** the four sites are enumerated in `c79c629d`'s diff; the "no production code
writes `system-prompt.md`" claim rests on `src/prompts/builders.rs:701` and `:909` being
prose, and on the only `std::fs::write` calls to that path in the tree being in tests.

Two open bug files prescribed fixes for `src/tools/onboarding.rs`. Reconnaissance run
*before* reading either plan — per CLAUDE.md § *Bug Tracking* — changed both.

1. **Site count was wrong by 2x.** The eager-stamp file named `:497` and `:582`. One
   `grep onboarding_version` found **four**: also `perform_full_onboarding`'s tail, whose
   comment read *"Optimistic version write for full onboarding"*, and the fresh-config
   literal. All four return a `subagent_prompt`, so all four defer the work they certify.
   A fix at the two named sites would have shipped the defect twice more, behind a
   passing suite — the `mutate once per guarded SITE` law arriving as a live case.

2. **The preferred remedy was unavailable as a mechanism.** The file wanted the version
   stamped into `.codescout/system-prompt.md` itself, so the certification would travel
   with the artifact. One grep for that path settled it: **no production code writes that
   file.** Every write is prose instructing a subagent to `create_file` it. The remedy
   would have rested on subagent compliance — a policy, prescribed by a file arguing for
   mechanisms. Ask *who writes this artifact* before designing anything that marks it.

**Why the third site hid — the transferable part.** It is not in `handle_refresh_prompt`
nor the already-onboarded path, the two places a reader thinking about *"refresh"* looks.
It sits at the tail of a 220-line function doing something else, and its comment names
the **other** bug's flag combination (`force=true on existing project`). The two bug
files shared a line neither cited. **So enumerate by the FIELD name, not the feature
name**: `grep onboarding_version` returned all four in one call; reading the two
functions the files named would have returned two, and read as complete.

**The fixture layer held the same defect, which is why no test caught it.** Six
pre-existing tests reported a fully onboarded project while
`.codescout/system-prompt.md` had never been written, because site 4 stamped at config
creation. The tests did not miss the defect — they **encoded** it, inside the fixtures
meant to describe a *completed* onboarding. Repaired by completing the flow, not by
relaxing assertions; disabling the new witness reds exactly those six plus the
end-to-end test, which is what establishes the repair is load-bearing.

**Same-day third instance of `R-180`'s class, in a different tool.** Verifying that my
10 new tests ran, `grep -E "(system_prompt_stale_|records_a_baseline_without_stamping|…) \.\.\. ok$"`
reported **2 of 10** — every alternative but one continues past the fragment matched, so
the ` \.\.\.` anchor could not follow it. A plausible number, not an error. What
localised it was a **denominator**, not care: `^test .* ok$` counted 8949, matching the
run's own total, and a known-existing test appeared exactly 2x (once per lane), which
proved the buffer complete and moved the fault to the pattern. `R-180`'s `\b`-vs-
`LibToolContext` miss and this one are independent instances of *a measuring predicate
silently excluding its own target*, hours apart, in `grep` and in Python.

## Template for new entries

<!-- Insert new R-N entries above this line.

  1. WRITE THE ENTRY — ONE CALL. The server allocates the id AND writes the
     section:

       artifact(action="append_entry", id="5696563f06b2c222", id_prefix="R",
                anchor_heading="## Template for new entries",
                title="<title>",
                body="**Verdict:** hit | miss [×N] → rule · "
                     "**Observed:** YYYY-MM-DD, <context>\n\n"
                     "**Seam:** <what was unverified>\n\n"
                     "<narrative>\n\n"
                     "**Promote-when:** <falsifiable criterion>\n\n"
                     "**Status:** open — <N datapoints>\n\n"
                     "**Kin:** R-x, R-y\n")

     No `entry_collection`: entries here are `## R-N` body sections, not params
     rows. Passing `anchor_heading` + `title` + `body` TOGETHER is what makes the
     server write the section itself — omit any of the three and it only reserves
     the id, leaving the section yours to write.

     The server formats the heading as `## R-N — <title>`, which is the only shape
     `link_scan` defines a token in (`def_re` = `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`),
     so `## R-100` alone defines NOTHING and every citation of it dangles. A first
     cut of the 2026-08-17 archive migration used bare headings and pushed the
     project's dangling count UP, 720 → 761. Letting the server write the heading
     is what removes that failure mode rather than warning about it.

     Never compute the id, and never suffix one. A max read at the start of a pass
     is stale by the time you write (R-98: a peer took R-97 with a four-minute
     margin), and allocating from a stale max is how the first nine collisions
     happened. `R-72b` is not a valid entry token at all, since digit→letter is not
     a word boundary.

     SUPERSEDED 2026-08-20 — this used to be TWO steps: reserve the id, then write
     the section via `artifact(update, patch={body_edits:[…]})`, because
     `append_entry` wrote nothing without `entry_collection` and `edit_markdown` is
     refused on a guarded ledger. That two-step was an instance-level workaround,
     and it carried its own failure mode — a reserved id whose section never
     landed. An augmentation being present does not push this ledger off the
     section-writing path; `append_entry`'s own `seed_prose` test pins that.

  2. ADD THE INDEX ROW — a SECOND call, after the entry lands, using the id
     `append_entry` returned. Five columns, matching the header:

       | R-N | YYYY-MM-DD | verdict | pattern | evidence |

     EVERY body entry must have one. Verify with:

       comm -23 <(grep -o '^## R-[0-9]*b\?' <file> | sed 's/^## //' | sort -u) \
                <(grep -o '^| R-[0-9]*b\?' <file> | sed 's/^| //'  | sort -u)

     Empty output = clean. This check found 13 orphaned bodies on 2026-08-16.
     Write the row AFTER the section exists, never before: the allocator counts an
     id claimed by an index row, so a pre-written row consumes the id it names.

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
