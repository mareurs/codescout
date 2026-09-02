---
kind: tracker
status: active
title: 'documentation denies a capability the code has since gained, because the prose was true when written'
owners:
- marius
tags:
- defect-classes
- clusters
- doc-contradicted-by-code
topic: issue clusters and rule promotion
---

## IC-11 — documentation denies a capability the code has since gained, because the prose was true when written

**Slug:** `cluster/doc-contradicted-by-code`
**Claim:** A document states a behaviour the code contradicts. The statement was *true when written*; the code later gained or lost the capability. Nothing checks prose against code systematically, and the corrective pass that *does* happen is a hand-enumerated sweep whose completeness is unfalsifiable — it reports the surfaces it changed, never the ones it missed. Unlike a wrong statement, this defect has no authoring error to find.
**Members:** `filter={"tags": {"contains": "cluster/doc-contradicted-by-code"}}` — **`n=16`, 2026-09-02, re-derived** over TRACKED bug files with the anchored form below — **+1: `a-moved-mechanism-leaves-its-old-address-cited-across-the-crate`**, and it is the member that supplies this claim's own *"completeness is unfalsifiable"* clause with a **number**. A refactor moved the session-opener trigger out of `call_content` into `emitters::emit_session_opener` and rewrote it from `!emitted.contains(SESSION_OPENING_GUIDE)` to `ledger.contains(topic)`; **ten** doc and code comments across four files still cite the old form — wrong file, wrong function, wrong expression. One (`server.rs:1157`) is production code; one (`server.rs:8623`) is the remediation text *inside a failure message*, prescribing work on a branch the same commit deleted, so it does not merely misinform but **dispatches** — the `IC-17`/`Mechanism status` shape one surface over. Three review rounds each named one site, the author repaired the named site and offered a greppable sweep of the module, and **the grep and the fix shared a scope**, so the grep confirmed the fix because of the blind spot they had in common. It also **falsifies the remedy the author had just written**: all ten cite the expression verbatim in backticks with no line numbers — the form `coordinator.rs` was at that moment teaching as durable — and all ten rotted anyway, because the refactor rewrote the expression itself. Durable is the **named item** or the **property**; corrected at `9f1d92be`. Its process half is `prompt-surface-measurement-session-log:F-49`. **A seventh surface: a Rust doc comment citing a moved symbol**, which no instrument reads — `audit_doc_refs` resolves *paths* in code comments and nothing resolves a cited symbol or module against the LSP index already loaded. **Spread not re-adjudicated for it.** — **+1 (fifteenth): `a-worklist-field-announcing-an-absence-outlives-the-mechanism`**, the class arriving on a TRACKER FIELD rather than a code comment or a schema, and the sharpest member so far because that field is *consumed as a worklist*: `IC-17`'s `**Mechanism status:**` read *"Nothing owns … the git index"*, written at `0dea2246` (2026-09-01 00:44:39) and falsified by `99d5acac` at 01:48:45 the same night — 64 minutes, then a full day unread. `CLAUDE.md` § *Observer Blindness* routes `Mechanism status` rows to `H-N`/`I-N`, so the stale sentence did not merely misinform, it **dispatched**: the filing session had proposed "give the index an owner" as new design work and was reading `append_entry.rs` to price a build of something already running for a day. What stopped it was an unrelated `ls scripts/`, not any check. Corrected at `3151201a`. **+1: `advertised-required-overstates-what-the-code-enforces`**, the same prose-vs-code mechanism moved from the action enum to the `required` list: the schema names one member of an alternation the code implements (a documented alias set, plus `workspace`'s conditional default) and nothing relates the two. Its site count is published as a **candidate list of ten, not a census** — two proven by probe, eight declaring `required: ["path"]` but never checked for routing through the alias resolvers — so the class gains one member, not ten. Previously four added by the 2026-09-02 tool-surface review, all on the `tools/list` surface itself: `workspace-schema-requires-an-action-the-code-does-not`, `artifact-patch-schema-describes-a-failure-that-no-longer-happens`, `index-description-omits-the-verify-action`, `artifact-action-labels-omit-delete-move-and-update-entry` (the last is a partial fit — two of its three omitted actions predate the label, so it is incomplete-from-birth rather than decayed; kept here on the mechanism, prose-about-code with no check reading one against the other). **Spread not re-adjudicated for these four**; they add a sixth surface, the MCP tool schema. Previously **`n=7`, 2026-09-02**, re-derived with the anchored `git grep -clE` form over `docs/issues/*.md` + `docs/issues/archive/*.md`. **That form read 6 and a plain `grep` read 7 for as long as the new member was untracked** — the blindness this field's own parenthetical predicted (*"blind until the file is tracked"*), met by the next person to add a member; and the obvious remedy is wrong. *Superseded 2026-09-02:* this field closed **"`git add` first, then count"**, which is correct on a solo tree and, on a shared index, instructs a session to stage a file it did not write. It did exactly that — `docs/issues/archive/2026-09-02-one-ledger-file-serializes-every-class-edit.md` is the record, and its third arrival route is a **reader**, holding nothing to commit, mutating state six sessions were using solely to obtain a number. Instead: derive over **tracked** files with the anchored form above and issue no `git add`; add only the member **this same operation commits**, which you know without consulting the index; and never count a `git ls-files -o --exclude-standard` hit, because that member is a peer's and counting it predicts a commit nobody has made. The union of tracked-plus-untracked was proposed as the fix and falsified the same day: it answered 21 for one class, and 21 was right **only because** that file's owner chose to commit — had they abandoned it the gate would have reddened, with nothing differing on the deriving session's side. A prediction about other sessions' future commits reads identically to a measurement. The field asks how many are **tracked**, and `tests/issue_clusters.rs:511-527` — `actual_counts` over `tracked_all_bug_files()` — is the definition of that question rather than a proxy for it. (Defect and corrected rule: `codescout-05`. Union falsified: `codescout-20`. Record carried: `codescout-ca`.) — **+1: `a-fix-comment-cited-a-line-its-own-commit-displaced`**, whose twist is that the falsifying edit and the false citation are **the same commit**. `0cb617cc` wrote a doc comment in `doctor.rs` citing `append_entry.rs:203` for the symbol making its claim true, while inserting 18 lines above that line in `append_entry.rs`; the symbol is at `:221` and `:203` now holds an unrelated error string. So the pair was **never** consistent — not even at the instant it was committed, which defeats the one repair this class usually admits: a reader diffing the citing commit against its parent still sees a citation resolving to unrelated text. The cited *site* was also wrong before the shift, since `:221` merely formats a hint and does not decide the value; the real mechanism is `body_entry_heading_level` in `augmentation.rs:1425-1436`. Caught only because a reviewer was instructed to verify the justifying claim **independently** rather than accept it — under an ordinary read the citation resolves to unrelated text and nothing distinguishes *stale pointer* from *false claim*. It is also the second member whose surface is a **Rust doc comment citing a moved address**, the surface this field already records as one *no instrument reads*. **Count not re-derived for it.** — **+1: `two-trackers-have-no-open-append-path`**, the same prose-true-when-written mechanism arriving on a **routing** surface rather than a descriptive one. `CLAUDE.md` § *Session Intelligence Trackers* and `docs/TAXONOMY.md` both route `T-N` and `I-N` to `append_entry`; `allocate_entry_id` (`augmentation.rs:971-983`) refuses both, because neither tracker declares `entry_prefix` in frontmatter — a requirement introduced when the counter had to survive a fresh clone, i.e. after the routing prose was written. Direction is *assert*, not *deny*, which this class already contains (`advertised-required-overstates-what-the-code-enforces`), so it is the same mechanism and not an inversion of it. What is new is the **cost of being wrong**: the other documented route is closed by the guard's `augmented` reason, so BOTH refuse, and the only route left open is `artifact_augment(merge=true, params={…})` — the call `CLAUDE.md` itself records destroying 18 of 19 `T-N` entries on 2026-08-16. Every prior member misinforms; this one misroutes toward the destructive call, which is a different severity of the same defect. Note also that each refusal reads as a guard working correctly, so a session hitting one routes around it and files nothing — which is why a live, fully-closed append path survived unrecorded. **Count not re-derived for it.** — **+1: `the-entry-shape-table-prescribes-the-count-its-own-gate-refuses`**, the **third** instance inside the ledger that defines this class, after the two the Index blockquote already records parenthetically. § *The entry shape*'s `**Members:**` row read *"the query, plus a bare count and the date it was run"* while `no_class_field_states_a_bare_n` names *"reintroduce a bare count into any `**Members:**` field"* as the mutation it exists to kill — the authoring surface prescribing precisely what the enforcing surface rejects. **What makes it an entry rather than a repeat:** the 2026-09-02 count-policy inversion updated three surfaces — the Index blockquote, the pre-commit script and the test — and missed a fourth **two sections above the Index in the same file**. That says something the earlier two did not: this ledger is the document describing its own machinery, so every machinery change creates a documentation obligation *here*, and the obligation is discharged wherever the author happened to be reading. The Index was updated because the change was about the Index; nobody was looking at the field table. Detection is structurally absent rather than merely missing — the gate is an **absence** assertion over class files, monotone under removal and blind to parent-ledger prose, and no check compares a field reference against the code that reads that field. Fixed in the filing commit; **no regression test, deliberately** — the assertion would compare a markdown cell against a test's doc comment, which is a parser over two namespaces and would earn its own `IC-6` entry. **A sixth doc surface only if a *field-definition table* is judged distinct from the tracker worklist field counted for the fifteenth; spread not re-adjudicated for it.**

**The twelfth is a DIAGNOSTIC STRING rather than a document**, which widens the surface set to a seventh — hook output. `foreign-index-refusal-names-a-cause-no-route-produces`: `scripts/pre-commit-foreign-index.sh:209` explains an unrecorded owner as *"staged before this guard was installed"*. That was plausibly true at `e3c75306` (2026-09-01 02:01:38), when it was the only route to `-`; `fa9b3aff` (03:58:14) and `92dfa4e4` (14:00:58) added two further routes to the same state, and neither had any reason to read a sentence in the other script. The decay is measured by `git log -S` on the three strings rather than asserted. Kept here on the same mechanism as the fourth member above — prose-about-code with no check reading one against the other — and it is the first member where the prose is *emitted at runtime to the party it misleads*, which is why the reader is not merely uninformed but actively told the tree is not theirs.

**The seventh is the first member whose prose lives INSIDE the code file it describes**, and the first whose claim is **forward-looking**: `2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work` — `scan_dated_stale`'s doc comment said Task 7 was *"not-yet-shipped"*, Task 7 shipped as `scan_cited_but_undeclared` two weeks earlier, and a reader wrote a worklist item proposing to build it. Two things it adds that the other six do not.

**(a) The decay trigger inverts.** The six existing members are positive claims that rot when the code *changes*. A forward reference is a claim of **absence**, so it rots when the project **succeeds** — and the party holding the falsifying fact is the implementer, whose diff adds the new symbol and never touches the sentence three functions above it. Self-review of that diff structurally cannot surface it. Measured population: **3 doc-comment forward references in the Rust corpus, 1 stale.** The two survivors split usefully — `tests/librarian/goal_eval.rs:64` is wired to a failing rubric and an `#[ignore]` naming its precondition, so shipping the thing changes a test's state; `src/agent/mod.rs:672` is pure prose and is one feature away from rotting identically. **The forward reference that survives success is the one tied to a check.**

**(b) Proximity is refuted as a remedy.** Same file, three functions away — the tightest coupling short of the same line — and it still rotted. Any "keep the docs next to the code" answer to this class is answered by this member.

**Previous reading, kept because the count-vs-prose lag is this field's own recurring defect:** n=6, 2026-09-01, by query after a probed archive pass. **Fourteen candidates were probed and three passed**; the ten that did not are deliberately untagged, not pending. See *The probe* below. **Four are `fixed` and archived; a fifth opened the same day** — `2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed`, a manual page still framing librarian as a separate sister MCP server after the tool collapse. *(This sentence read "all four … the class now has no live instance" for two hours, until its own author opened the fifth. Premise moved, conclusion did not — the fourth instance of that in this file today, and the one thing that would have caught it is `every_index_count_matches_the_corpus`, which is blind until the file is tracked.)* **And a sixth, opened later the same day** — `2026-09-01-claude-md-denies-a-pid-to-session-join-the-registry-carries`: `CLAUDE.md` § *Observer Blindness* denies a pid→session join that live registry entries carry. So the class has **three** live instances rather than one, and the Index row named the sixth before this field did — the fifth time in this file that a count moved and the prose beside it did not.
**Blind party:** the *reader*, routed to the document by its own scope claim and given no signal to cross-check. The author of the prose is not blind — they wrote something true. The author of the *code* change is differently blind: gaining a capability gives you no reason to search prose for sentences your feature just falsified.
**Promotes to:** `not yet` — `n=15` (was `n=14` earlier the same day, and `n=7` until the 2026-09-02 review added four schema-surface members), and the count bar has been cleared since the probe (this line read `n=6` until 2026-09-02, `n=4` until 2026-09-01, and `n=1 taggable` before that — backticked, so the gate reads them as quotations and leaves them alone). **The 2026-09-02 update missed this line and `every_bare_n_in_a_class_field_matches_the_corpus` caught it**, which is the sixth occurrence of *count moved, prose beside it did not* on this entry and the first one a gate found rather than a reader: three surfaces carry the number — `**Members:**`, this field, and the Index row — and updating two of three is the default outcome, not a lapse. **It recurred the same day and the same gate caught it again**: the twelfth member's author updated the Index row and `**Members:**`, missed this field, and was refused — the seventh occurrence of the drift, and the second a gate found rather than a reader. **A third time, and this one inverts the ratio**: the fifteenth member's author updated `**Members:**` alone and was refused on the Index row *and* this field — the eighth occurrence, the third the gate caught, and the first where the surfaces missed outnumbered the surface updated. **That entry originally cited a "0 for 3" three-of-three rate; it was withdrawn within the hour and the reason is worth more than the figure.** Its denominator was *gate refusals* — a population defined by failure — so the numerator could not have been anything but zero, and an author who updates all three surfaces correctly is never refused and leaves no artifact at all. Successes are unrecorded BY CONSTRUCTION, which is `CLAUDE.md` § *Testing Discipline*'s recording-filter law arriving inside the entry that argues from it; no larger sample repairs it, because widening a population does not create the missing observations. What is defensible is a count and not a rate: **three recorded occurrences since the gate existed, all three caught by the gate rather than by a reader, with the warning paragraph present and unread throughout.** A fourth candidate was offered by `.claude-sdd`'s session — refused on this same entry earlier the same day, having staged members with no ledger update at all — and is recorded here as *offered*, not absorbed, since absorbing a volunteered datapoint as a catch is what makes a population look self-correcting. The argument never needed the rate: a warning three consecutive authors read past is not the remedy, and the gate is. The aggravating detail is worth the line: that author had read this very sentence, in this session, minutes before making the edit. A warning sitting one line above the number does not survive an edit made two fields away. The gate does. What holds promotion is not the count but the three-way remedy split adjudicated below — **and the spread, which the Index row records as adjudicated at 4 doc surfaces / 4 subsystems and explicitly *not* re-adjudicated for the sixth or the seventh** (the seventh adds a fifth surface, a Rust doc comment). The likely target is `DC` (`docs/trackers/claim-decay.md`): a true-when-written claim that silently decayed is that ledger's subject, and this class is the bug-corpus entry point to it rather than a competitor — the same relationship `IC-8` declares.
**Mechanism status:** none yet, and the nearest existing mechanism does not cover it. `librarian(action="audit_doc_refs")` lints *references* — paths, symbols, line numbers, link targets — so a document may cite every path correctly and still assert the opposite of what the code at those paths does. The remedy would have to check claims, not refs. **That is the right conclusion for the wrong reason on one of the four members** — see *The spread* below, where a member citing four names that do not exist is missed anyway, and not because it asserts anything.
**Valid:** dated 2026-09-01

Seed instance: `2026-08-31-librarian-runtime-guide-denies-the-augmentation-sidecar`. The served `librarian-runtime` guide states augmentation has *"**No** — there is no on-disk representation"* and that sharing it is *"local-only by design"*. Both sentences were accurate when written. The sidecar shipped as `e799f29d` on 2026-08-30, and a deliberate sweep the **same day** — `e1b91221`, *"state that augmentation shape now travels, in the three places that said otherwise"* — corrected `CLAUDE.md`, `docs/conventions/cross-machine-catalog-resume.md` and `tracker-conventions.md`. Not this guide. So the drift is **one day old**, and the mechanism is an enumeration produced from memory, not neglect: "three places" reads as a finding and is a list. The guide mentioned `sidecar`/`expects_augmentation` zero times against `tracker-conventions`' thirteen.

**Fixed 2026-09-01** — SHA `0523b823`, patch-id `9ec0e7c8911be27700318ba60b945454275391e7`. **Four**
sentences corrected, not the two the bug filed: reading the rest of the section found *"An augment
produces no git diff … `git status` stays clean"* false since write-through (`sidecar_write_through`,
`augment.rs:248`), and the `reindex` bullet true but silently incomplete. The bug's own enumeration
had stopped at the examples that prompted it — the same mechanism it was filed against, one level in.

**And the zero-times count above decayed the instant the fix landed** — it now reads 5. That is this
class holding about its own seed paragraph, which is why the sentence is now past tense rather than
corrected in place: re-pointing it to 5 would schedule the identical decay for the next reader. The
standing guard is `prompts::redesign_invariants::no_guide_denies_the_augmentation_sidecar`, sibling
to the test written for the *2026-08-16* miss of this same section — two three-place sweeps, one
file. It asserts both directions, because an absence half alone is monotone under removal, and each
half was mutation-verified separately.

**The cost is not that a reader is misinformed — it is that the reader stops.** A sentence saying a capability does not exist terminates the search that would have found it. Measured downstream the same day: a consumer repo held two augmentations in a machine-local catalog with no sidecar and no declaration, one clone away from silent loss, because the guide consulted for exactly that question said there was nothing to export. `doctor`'s `augmentation_declared_but_absent` could not report it either — that check fires only on a *declared* sidecar that is missing, so undeclared-and-unexported reads identically to nothing-to-declare.

**The same guide, the same section, fifteen days earlier — and it is already tagged.**
`docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` is
`status: fixed`, and reports that this *same* § *Where catalog state lives* section claimed
`artifact(action="move")` "preserves `id`" when a move necessarily re-keys the row. It
carries the tag `doc-vs-code` — the shape had an informal label before it had a class, which
is the ordinary way a class announces itself. It stays untagged for `cluster/` purposes under
the archive policy, so it does not raise the count. Two of the three open files carrying
`doc-vs-code` are correctly filed under `IC-2`; the tag is a secondary descriptor there, not
the primary defect.

**Kept apart from `IC-3` and `IC-8` on their own falsifiers, not on judgement.** `IC-3` is a surface declaring a capability production never reaches; this is its mirror — production reaches a capability the surface denies — and `IC-3`'s falsifier explicitly ejects the mirror case (*"the wiring existed and the declaration was merely wrong, which is an ordinary bug rather than this class"*). `IC-8` is an assertion written at the moment of intent and read forever after as outcome; this prose was not intent, it was correct observation, which is why no plausibility check catches either one.

**Falsified by** an instance where the documentation was wrong on the day it was written. That is an ordinary authoring error with an author to find, and it does not belong here.

**The probe, run 2026-09-01 — and it is the reason this class was backfilled last.** This entry's
admission question is *"was the prose true when written"*, which is a fact about **history**, not
about the text. No amount of reading either surface answers it: a document that contradicts the
code reads identically whether the code moved under it or the author was simply wrong. So the
archive pass that tagged `IC-13`–`IC-16` from claims alone could not tag this one, and fourteen
candidates went to `git log -S` instead.

**Three passed** and are now tagged. `recoverableerror-display-doc-contradicts-code` is the
cleanest: the `Display` doc comment said it omitted `hint`/`guidance`, and `dc8f0f1f`
(2026-05-09) is titled *"Display surfaces RecoverableError guidance text"* — the code **gained**
the behaviour, so the comment was true until that commit and false after it, with no author to
find. `tools-semantic-search-manual-page-describes-legacy-interface` describes the pre-Phase-7
interface, accurate until Phase 7 replaced it. `test-env-isolation-doc-prescribes-rejected-remedy`
prescribed option B until `a656f8ce` marked that remedy non-viable.

**One was refuted, and it is the one this entry singles out.**
`docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` — same guide,
same § *Where catalog state lives* — looked like the strongest member available. Its own
`## Hypotheses tried` states the question and defers it: *"the guide is describing an older
behaviour that was correct when written. **Test** — `git log` on `src/librarian/tools/mv.rs` for a
commit removing id-stability. **Verdict** — deferred; irrelevant to the fix either way."* Running
that deferred test settles it against the class: `mv.rs`'s full history contains **no commit that
removed id-stability**. The id has been `sha256(abs_path)` since the file existed, and the
2026-08-16 commits nearest the question — *"move now grafts history onto the new id instead of
stranding it"*, *"repair the frontmatter id the move just invalidated"* — **cope with** the
re-keying rather than introduce it. The guide was wrong on the day it was written. That is this
entry's own falsifier (*"an ordinary authoring error with an author to find"*), so it stays out,
and the earlier note that it "stays untagged under the archive policy" is superseded by a reason
that survives the policy changing.

**Ten remain untagged because the probe did not establish them, which is not the same as pending.**
The recurring shape among them: the doc and the contradicting code appear to have coexisted from
the start — `path_security.rs`'s module doc promises `RecoverableError` while `bail!` is present
in the file's own creation commits; `scope`'s documented `project` default against a compiled
`Repo` that only two commits ever touched, the second being the fix. Each is *probably* an
authoring error, and "probably" is exactly the standard this entry exists to refuse. They are
listed as probed-and-not-established so the next reader does not re-derive the same fourteen.

**The spread, adjudicated 2026-09-01 — and the unit matters, as it did for `IC-14`.** Four members,
**four distinct documentation surfaces** (user manual, conventions doc, Rust doc comment, served
guide) and **four distinct subsystems** (retrieval, test infrastructure, error handling, librarian).
The two counts agree at 4, which is worth stating rather than picking one — agreement is evidence,
and a bare *"4"* hides which question it answers. `IC-14`'s did **not** agree (8 members, 6 guards),
so the agreement here is a fact about this population and not a property of the ledger.

**But the four split three ways on what the prose is a claim ABOUT, and the three have three
different remedies** — the same shape `IC-14` turned out to have, and the same reason one promoted
rule would be right about a third of it:

- **Behavioural claim** (2) — the prose asserts what the code does or does not do, and every path
  and symbol it cites is correct. `recoverableerror-display-doc-contradicts-code`, where the doc
  comment claims an omission the `fmt` body does not make, and the sidecar guide. **Unreachable by
  any reference check, by construction.**
- **Decision claim** (1) — `test-env-isolation-doc-prescribes-rejected-remedy` prescribes option B
  after `a656f8ce` recorded that remedy **NOT VIABLE**. The falsifying artifact is *another
  document*, so neither reading the code nor checking its refs reaches it. This is the member with
  a measured live cost: its own summary records the doc having made engineers reproduce a purged
  bug "at least twice in one session".
- **Named-entity claim** (1) — `tools-semantic-search-manual-page-describes-legacy-interface` names
  four things that do not exist (`score`, `language`, `detail_level`, `offset`). It is not an
  assert-the-opposite at all, and it is the mechanizable one.

**The mechanism finding corrects this entry's own stated reason.** `Mechanism status` above says
`audit_doc_refs` cannot cover the class because a document "may cite every path correctly and still
assert the opposite". True of the first two shapes; the **wrong reason** for the third, which cites
nothing correctly — four dead names — and is missed anyway. Two hypotheses were tried before the
right one, and both are recorded because each is the plausible answer:

1. *The names sit in a fenced JSON block and the scanner skips fences.* **Refuted.** `parser.rs:48`
   emits candidates while `in_code_block`, pinned by `parser_walks_fenced_code_blocks`
   (`parser.rs:1021`). A fenced ref is **severity-capped** to `code_block`, never dropped
   (`severity::cap_code_block`, applied at `resolver.rs:671`, pinned at `resolver.rs:866`). Same
   structure as the forced-`Med` on code comments: found, then downgraded below `--fail-on high`.
2. *So it is found and downgraded.* Also wrong. `RefKind` has exactly five variants — `FilePath`,
   `FileLine`, `FileSymbol`, `ModulePath`, `Link` (`src/librarian/tools/audit_doc_refs/mod.rs:11`)
   — and **all five are locations**. A JSON response field and a tool parameter are neither. The
   instrument does not downgrade them; it never sees them.

So the class has one buildable sub-remedy with a concrete shape — a candidate kind for tool params
and response fields, checkable against the live schema, which is the same set-difference the tool
registry guard uses — against "check claims, not refs" for the other two, which is not buildable
today. **`Mechanism status` stays `none yet` because the majority shape has no instrument, but it
is `none yet` for two reasons now, and only one of them is hard.**

*(Both hypotheses above were mine, stated confidently, and each was refuted by one grep. The
fence one is the instructive failure: fenced content being illustrative rather than real is
`IC-6`'s subject, so the wrong answer was the one the neighbouring class made most available.)*
