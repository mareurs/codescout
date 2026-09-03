---
id: e613a2692b577b7a
kind: tracker
status: active
title: Session Log — Cluster Promotion (IC-N → OB-N)
owners:
- marius
tags:
- session-log
- clusters
- observer-blindness
- mechanism-design
topic: cluster promotion and mechanism design
entry_prefix:
- F
- W
entry_high_water_F: 8
entry_high_water_W: 3
---

# Session Log — Cluster Promotion (IC-N → OB-N)

> **Purpose:** Two-sided observation log for the work stream promoting `IC-N` defect
> classes (`docs/trackers/issue-clusters.md`) into `OB-N` rows with named mechanisms
> (`docs/trackers/observer-blindness.md`). Frictions are `F-N`, wins are `W-N`.
>
> **Append with one call** — the server allocates the id, writes `## F-N — <title>`,
> records the high-water mark, and stamps `**Valid:**`:
>
> ```
> artifact(action="append_entry", id="<this artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> **Then** add the Index / Wins Index row with the id the call returned. Never
> hand-allocate, never pre-write the row — a pre-written row consumes the id it names.
>
> Status vocabularies, category conventions and the full F-N / W-N entry templates are
> pinned in `docs/templates/session-log.md`; this file follows them rather than
> restating them, so a change there does not leave this copy stale.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-09-01 | med | architectural | open | `OB-7`'s mechanism cites an LSP-verified result to justify a text-search check |
| F-2 | 2026-09-01 | high | tooling | open | `ListAgents` enumerated 4 of 20 live sessions, so authorship inferred from it came from a 20%-complete population |
| F-3 | 2026-09-01 | med | architectural | fixed-verified | The split's load-bearing sentence was refuted as a population claim 67 seconds before it was committed |
| F-4 | 2026-09-01 | med | tooling | fixed-verified | The ledger's count cells go stale by CONCURRENCY — 3 re-derivations invalidated in one session by peer filings; gate shipped, caught a 4th drift on its first run |
| F-5 | 2026-09-01 | med | architectural | open | `IC-13`'s claim is true of 4 of its 16 members — and the "≥4" floor carried from a prior audit bounded the opposite set; two rulings owed |
| F-6 | 2026-09-01 | med | architectural | fixed-verified | Ruling 2 — all 7 `IC-13` non-members fit **no existing class**, unanimously across two independent readers; four-class partition taken → IC-19/20/21/22, IC-13 16→9 |
| F-7 | 2026-09-01 | med | tooling | **fixed-verified** | The ledger shipped without its corpus — 3 new classes published counts of 3/1/2 against **0 members at HEAD**; the gate reads the working tree, so a partial commit is invisible to it. Repaired; mechanism owed |
| F-8 | 2026-09-03 | low | documentation | open | A refusal hint's stated condition (`edit_file` "without its `frontmatter` param") is **always true of the call it refuses** — the real discriminator is extent-bounded vs raw-text grammar. I reported the guard as broken; the guard is correct and deliberate. `IC-22` fit left unsettled rather than resolved by filing |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-01 | med | re-probe the served copy after a rebuild (`readlink /proc/<pid>/exe`) | 5 of 11 peers on deleted inodes, invisible to every existing instrument; an open bug's fix shown to fail open | validated |
| W-2 | 2026-09-01 | med | compare pre-stage diff counts against `git commit`'s own `--stat` | capture of another session's hunks was invisible to four guards that all passed correctly; 23/16 measured vs 58/19 committed | validated |
| W-3 | 2026-09-01 | high | blind a review by REDACTING the reviewer's copy, then name the second leak channel | slug-only redaction would have shown one reader `IC-14` twice in prose — that file's retag was the audit's only promotion-tripping finding | validated |

---

## Scope note

This work stream is **adjudication and mechanism design**, not bug fixing — the unit of
work is a defect *class*, and its output is an `OB-N` row whose `Mechanism status` field
is honest about what has and has not been built. Bug-level findings belong in
`docs/issues/`; recon findings about the promotion process itself belong here.

Two standing hazards this stream has already produced, recorded so entries can cite them
rather than re-derive them:

- **Resemblance is not membership.** Twice on 2026-08-31 the answer to "does this belong
  in that class?" was no, on the remedy test rather than the description — `IC-8`
  declined for a decayed-conclusion pattern, `IC-10` declined for a verification-reflex
  pattern. Both descriptions fit; neither remedy did.
- **A count and the judgement that quotes it move independently.** The archive backfill
  updated every `Members:` line and left four `Promotes to:` fields reasoning from the
  superseded number. When a count moves, re-derive every judgement quoting it in the
  same pass.

---

**This log is written by several sessions, and its entries are NOT attributable to whoever
created the file.** Recorded because the inference was already made: on 2026-09-01 a peer read
`F-2` here, addressed it to the session that opened the log, and built `F-3` on that
attribution. `F-3`'s *finding* survives — the sentence it narrows was genuinely refuted — but
its authorship premise did not. The session that created this file appended `F-1` and `W-1`
only; `F-2` and `W-2` arrived from elsewhere and their author is not recoverable from the file,
from `git` (one author string for every session), or from the catalog.

So: **do not address an entry here to a session, and do not infer one from proximity or from
file ownership.** Ask, or cite the entry rather than a party. That is `issue-clusters:IC-10`
applying to this tracker specifically — and the instance above is the class compounding, since
the misattribution was made *while* reading a ledger about misattribution.
## F-1 — OB-7's mechanism cites an LSP-verified result to justify a text-search check, and the substrate gap is a deletion-authorising false positive

**Observed:** 2026-09-01, post-commit reconnaissance of `a2aedd49` (IC-3 → OB-7). Scouting three checkable claims I had committed and reported to the user as verified.

**When:** After promoting `IC-3`, before starting `IC-1`. The seam is `OB-7`'s `Mechanism status` field — the part that makes the entry a worklist item rather than advice.

**Expected (what I wrote in `OB-7` and in the commit message):** *"`references(symbol)` **decides this today** — verified on `chunk_size_for_model` … The check is *non-test caller count == 0*, and the partition needs call-site granularity rather than file granularity, since test modules live inline — which the AST index already carries (`grep` annotates each source hit with its enclosing symbol)."*

**Got (scouted reality):** the partition half is **true and now evidenced**, and the decidability half is **overstated in two ways I did not name.**

1. **Confirmed, and better than asserted.** `grep` does annotate each hit with its enclosing symbol path, and test call sites carry a `tests/` prefix: `[tests/names_path_containing_generalises_and_normalises_separators]` versus `[names_tracker_path]`. The file-granularity trap is real rather than hypothetical — `references` grouped `src/librarian/adapter.rs:1451` under a production file, and `grep` shows it is `tests/…`. That single case is the whole argument for call-site granularity, and I had asserted it without an instance.

2. **The entry names two instruments with different substrates and treats them as one.** I *verified* with `references`, which is **LSP-backed and semantic**; I *described the mechanism* using `grep`'s annotation, which is **text**. That is the substrate distinction this skill's Phase 1 makes central, committed by a session that had the rule loaded.

3. **The substrate gap is a false-positive mode, and it is the dangerous direction.** A symbol reached only through `dyn Trait` dispatch, a function pointer, a macro expansion or a re-export alias has **zero textual non-test hits**, so the grep form flags it dead. `references` would catch trait dispatch; `grep` would not. This codebase is dispatch-heavy by construction — `impl Tool`, `Arc<dyn CodeEmbedder>` — so the blind spot lands exactly where the check would be run. And a false "dead in production" finding **authorises a deletion on a negative search result**, which is this skill's hardest rule.

4. **I ran only a positive control.** Seven symbols probed (`chunk_size_for_model`, `fences_balanced`, `names_path_containing`, `leading_run`, `strip_matching_quotes`, `clean_prefix`, `is_librarian_id`, `literal_continuation_mask`, `min_indent_outside_literals`); **every one confirmed the "has production callers" state.** I never exhibited a symbol with zero non-test callers, so the state the check exists to detect is **unexercised**. Per the skill: a single confirmatory probe cannot reveal a missing state.

**Probable cause:** I generalised from one probe run for a *different* purpose — establishing that a former member is live today — to a claim about an instrument's decision procedure. The two questions share an output and not a warrant. Compounded by writing the mechanism paragraph after the probe rather than before, so the probe never had to match the sentence it was cited for.

**Workaround:** none needed operationally — nothing consumes `OB-7`'s mechanism yet. The correction is to the text: name `references` (LSP) as the instrument and `grep`'s annotation as the partition helper, state the dispatch/macro blind spot, downgrade *"decides this today"* to *"decides for by-name call sites"*, and record that the zero-caller state is unexercised.

**Severity:** med — no failure today, but the next person to build the check builds it on the instrument the entry names, inherits a blind spot concentrated in this codebase's dominant dispatch idiom, and the failure mode's output is a deletion candidate. The cost is bounded only because the entry is four hours old and nobody has acted on it.

**Status:** fixed-verified 2026-09-01. *This line read "open — correction not yet written to `OB-7`" until re-checked against source, and it was **zombie-open**: the correction had been written hours earlier, in `a2aedd49` and after. Verified at the bytes — `OB-7` § *Dead in production* now carries "decides for **by-name call sites**", and `IC-3`'s `Mechanism status` carries the matching sentence. This is `CLAUDE.md`'s verify-open cadence firing on the entry whose own subject is unverified claims, found only because a peer exchange sent me back through it.*

**Valid:** dated 2026-09-01

**Rests on:** the substrate law — *when two instruments disagree, the question is which world each one read* — and the positive-control law: an instrument must be exercised once per state you believe it can report.

**Fix idea / Pointer:** correct `OB-7`'s `Mechanism status` family-1 bullet and `IC-3`'s matching text in one edit; both carry the same sentence.

**Sharpened 2026-09-01, by the peer session, and it is worse than "no negative control".** The
nine probes were **monotone under the exact failure the check is for**. A symbol reached only
via `dyn Trait`, a function pointer, a macro or a re-export alias has zero textual non-test hits
— it presents as the **dead** state. Every one of the nine presented as *has-callers*, and a
symbol with visible textual callers is **by construction not the case that misclassifies**. So
the population was not merely missing a negative control; it was selected such that no member
could be one. Nine confirmations of a proposition none of them could have falsified — which is
`CLAUDE.md` § *Testing Discipline*'s monotone law applied to a probe population rather than to
an assertion, and the two are the same defect.

**What would close it:** seed the probe from the **dispatch** side rather than the symbol side
— enumerate `impl Tool` implementors and `Arc<dyn …>` construction sites, then ask the checker
whether it calls them dead. If it does, the false-positive mode is *demonstrated* rather than
reasoned. If it does not, that is the first genuine negative control.

**Severity is `med` only while nothing reads the mechanism, and should be re-read rather than
inherited when a consumer arrives.** The failure mode is not a wrong report — it is a deletion,
because a false *dead-in-production* finding authorises removal on a negative search result.

**Where the correction landed:** `IC-3`'s half is in codescout `77d4da06`, a **peer's** commit
about `IC-11` backfilling — the edits were staged in the shared index when it committed, and the
warning message was in flight while it did. `OB-7`'s half is in `8ceb9ea9`. Recorded because the
ledger prose cites this entry, so a reader following the correction arrives here and would
otherwise find no pointer back to the commit that carries it.
**PROBE RUN 2026-09-01, and it refuted my own hypothesis while producing the negative
control.** Seeded from the dispatch side as prescribed: enumerate every `impl Tool for X`
(41 impls, 37 production types), diff against `CodeScoutServer::new`'s registry.

**The false-positive mode I was worried about does not occur for this codebase's dominant
idiom, and the reason is worth keeping.** I feared `dyn Trait` dispatch would hide a symbol
from a text search. It does not: a trait object must be **constructed**, and construction is
by name — `Arc::new(Grep)` — so every live tool type is named at least once at registration.
The delegating pattern is by name too (`"activate" => ActivateProject.call(…)`,
`"register" => RegisterLibrary.call(…)`). Dispatch consumes the name; it does not erase it.
The genuine false-positive candidates are narrower than I wrote: **macro-generated names and
re-export-only aliases**, neither of which the tool registry uses.

**And the check found a real instance — the negative control that nine earlier probes could
not produce.** `ListFunctions` and `ListDocs`: `impl Tool`, 25 identifier hits across four
files, **every non-definition hit test code**, zero registration, zero production call sites.
Fifteen tests guarding two tools no agent can reach. Filed as
`docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md`. So the state
the check exists to detect is now *observed*, not merely reasoned — and the first observation
of it was a true positive.

**The cross-instrument check I designed was unavailable, which is its own finding.**
`references("ListFunctions")` returned **0** with its false-zero guard firing correctly
(*"appears as a whole word in 3+ other source files — the reference index may still be warming"*),
and `references("GetUsageStats")` returned `symbol not found`. So the LSP half of the
substrate comparison could not run, and the conclusion above rests on the text instrument
alone. That is exactly the limitation this entry was opened about, still true — the guard
naming it is the difference between a degraded instrument and a silent one.

**Net effect on `OB-7`:** the family-1 check is stronger than the correction credited it, and
its residual risk is narrower and differently shaped. `GetUsageStats` is left explicitly
unresolved rather than swept in.

**Sharpened again 2026-09-01 — there are TWO monotone-population mechanisms and this entry holds
only the fixable one.** `OB-9`'s caveat is the other, and the distinction is the peer's
(`codescout-e8` proposed it as a sibling; the upgrade below is what we settled on). This entry's
defect is **member-selection**: nine probes chosen such that no member *could* falsify, and the
repair is a tenth member that can — exhibit one symbol in the zero-caller state, which the
dispatch-side probe then did. `OB-9`'s is **observation-selection**: a reader who doubts a
published number and re-counts leaves no artifact, so the refuting outcome never becomes a
record. **The two do not share a remedy, and that is the whole point of separating them** —
"widen the sample" repairs member-selection and is a **no-op** for observation-selection, at any
corpus size, forever. A reflex answer that looks responsive and changes nothing is worse than an
obviously inapplicable one. Cross-reference kept here because a reader arriving at `F-1` for the
monotone law gets the tractable case and would otherwise leave believing it is the only one.
## W-1 — Re-probing the served copy after a rebuild found 5 of 11 peers on deleted inodes, and an open bug's fix failing open

**Observed:** 2026-09-01 00:35, immediately after the operator rebuilt the release binary and reconnected. Scouting whether the rebuild invalidated anything published earlier in the session.

**Pattern:** After a rebuild, **re-probe the copy the serving process actually holds, and check the same for every peer** — `readlink /proc/<pid>/exe`. A rebuild is a substrate change under every tool-behaviour claim made before it, and the claims do not announce their own staleness.

**Counterfactual:** two things would have gone unnoticed.

1. **`OB-7`'s mechanism claim was verified against the pre-rebuild binary.** Re-probed on the fresh one and it holds identically — `grep` still annotates each hit with its enclosing symbol and test sites still carry a `tests/` prefix. A *match*, and the point is that it was not knowable without the probe: this is the one claim in the entry that had already needed correcting once, so inheriting it across a substrate change would have been the second unverified inheritance of the same sentence.

2. **5 of 11 `codescout start` servers were holding deleted inodes at that moment** — started 11:20, 11:38, 14:46, 21:45 and 22:20, all pre-dating the 00:34 rebuild. Every existing instrument called them healthy: `peer-sessions.sh` prints start time and `cwd`, `ListAgents` lists sessions, and `~/.cargo/bin/codescout` is a correct symlink to the right path. Nothing in any of those outputs distinguishes a process serving current bytes from one serving replaced bytes.

**And the scout found a defect in an open bug's *proposed fix*, which is the part worth carrying.** `docs/issues/archive/2026-08-31-peer-sessions-never-compares-start-time-to-build-time.md` prescribes comparing process start time against binary mtime. Run against a genuinely stale pid it prints **nothing**: `readlink` returns the path with a literal ` (deleted)` suffix, `stat` on that string fails, `[ N -lt "" ]` errors to stderr and evaluates false, and the `&&` chain short-circuits. The one branch that must fire is the only one that cannot, and a caller redirecting stderr sees a clean report. The discriminator was already in the string the fix had just read and was discarded in favour of a timestamp proxy — which is `OB-6` in a proposed remedy rather than in shipped code.

**Confirming data points:**
1. This session — rebuild at 00:34, five stale peers found, proposed fix demonstrated failing open against `pid 997544`.
2. `reconnaissance-patterns:R-89`, ×4 recurrences, which is the same law from the build/process/distribution axes; this adds the **inode** axis, where the path and the symlink are both correct and only the held inode is wrong.
3. `OB-2` — the session that arms a shared-state trap gets no signal; here the arming action is a rebuild and the unsignalled parties are five peers.

**Impact:** med — no wrong result was published, because the re-probe matched. The value is that "matched" became a fact rather than an assumption, and the peer measurement surfaced a fix that would have shipped silently broken.

**Promote-when:** a second session finds a stale-inode peer that no existing instrument reported. At two datapoints this belongs in `docs/PROBES.md` as a named check, since it is one `readlink` and it answers a question three other instruments cannot.

**Status:** validated — single session, but the fix defect is independently reproducible from the commands recorded in the bug file.

**Valid:** dated 2026-09-01

**Rests on:** `reconnaissance-patterns:R-89` — freshness is a property of the copy that serves you — extended with the inode axis, and `OB-6`'s rule that a signal must come from the event's own side of the boundary.

## F-2 — ListAgents enumerated 4 of 20 live sessions, so every authorship inference drawn from it came from a 20%-complete population

**Valid:** dated 2026-08-31

**Observed:** 2026-09-01 ~00:40, issue-clusters hygiene pass on a shared checkout with concurrent sessions.

**When:** After `d617051b` captured three hunks of another session's `IC-1`/`OB-8` work, while trying to identify the owner in order to offer them a repair.

**Expected:** `ListAgents` enumerates the local sessions that could be writing this checkout, so a `busy` peer adjacent in time to a commit is its probable author.

**Got (measured):** `ListAgents` returned **4** peers across three calls spanning 20 minutes. `ls /run/user/1000/cc-socks/ | wc -l` returned **20** live session sockets; `pgrep -c -f claude` returned 42. The two peers whose addresses I actually held — `2481440.sock` (`codescout-5f`) and `2081267.sock` (`codescout-fc`) — are both present in the socket list, so the listing is a strict **4-of-20 subset**, not a stale snapshot. Six codescout commits landed 00:20–00:37; both enumerated codescout sessions deny authoring the `issue-clusters.md` ones.

**Probable cause:** the listing reaches only sessions sharing this one's config profile, and this machine runs three (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat` — CLAUDE.md § *Three Claude Code Instances*). Every session also commits as the same git author on the same checkout, so `%an` carries no signal either. **Adjacency and author are constant across sessions by construction** — no amount of care reading them helps, which is what makes this a mechanism problem rather than a diligence one.

**Cost, as it actually played out:** I attributed the swept hunks to `codescout-5f` from proximity; it denied, having made one commit to a different file. I reassigned to `codescout-fc` from proximity; it denied, having made zero codescout commits — and observed that my second inference sat *inside the message correcting the first*. Three `IC-10` firings in one exchange, two of them in corrections. The owner is still unidentified and is not reachable from anything I can query.

**Why it is worse than a wrong guess:** the failure returns a *plausible name*, never an error, and the repair it suggests — amending `d617051b`'s message to credit the owner — would write that name permanently into git, where it is durable and no longer falsifiable against a live session that can deny it. `codescout-fc`'s judgement was that this option is *worse than doing nothing*, and that is right.

**Workaround:** ask the session. `SendMessage` is the only signal that discriminated — but it reaches only the enumerated 4, so it does not close the gap; it merely declines to lie about it.

**Severity:** high — silent, produces a confident wrong answer, and its natural remedy makes the error permanent.

**Status:** open — mechanism still owed. **The published ratio was wrong and is corrected below**; `F-3` caught it. The claim it supports survives the correction; the number in this entry's own heading does not, and the heading is left intact because it is the citable token.

**Rests on:** the socket count being a live-session proxy — supported here by both known session addresses appearing in it, but not independently established as complete either.

**Fix idea / Pointer:** this is `IC-1`'s *visibility* half with a number attached, and the number is the contribution — the class text says "the peer cannot be enumerated", which reads as a possibility rather than a 5× under-count. Related: `docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md`, `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

**Correction, 2026-09-01 — the claim holds, the ratio does not, and it is not a single number.**
`F-3` caught a units mismatch here and was right to. `4 of 20` compares a **profile-scoped
instrument** against a **raw socket-file count**, and 7 of those 20 sockets are stale files with
no process — so "20 live session sockets" above is wrong on its own terms; 13 are live. Written
by this entry's author, because `F-3` deliberately declined to annotate another party's entry and
left it open; the pointer arrived by `codescout-b1` asking who owned `F-2`, which is the same
channel that resolved everything else tonight.

**But `F-3`'s replacement figure of `1 of 4` does not transfer to this session, and that is the
sharper finding.** That table was measured from a `~/.claude` vantage, where three of the four
listed peers sit in `mirela/backend-kotlin`. Re-derived here with `scripts/peer-sessions.sh` for
this `~/.claude-sdd` session: 5 live sessions have `cwd` inside this checkout (**4 peers**);
`ListAgents` returns 4 peers, of which **2** share this working tree (`2081267`, `2481440`) and 2
do not (`claude-plugins`, `agents/system`); the two same-checkout peers it misses (`2112574`,
`3708603`) are both `~/.claude`. The honest figure **here** is `2 of 4`.

**So the ratio is a property of (profile × checkout population), not of the tool.** No single
number is quotable as "the" under-count — quoting one is exactly how this entry went wrong the
first time, and a second entry quoting a different one would repeat it. What survives every
vantage, and is what `IC-1` actually needs: the instrument errs in **both directions at once**,
missing same-checkout peers on other profiles while reporting other-checkout peers on your own.
My figure and `F-3`'s disagree on the ratio and agree on that, which is the part to cite.

## W-2 — The commit's own --stat caught a capture that four correct guards passed

**Valid:** dated 2026-08-31

**Observed:** 2026-09-01, commit `d617051b` — a working-tree capture on a shared checkout, detected within one call of committing.

**Pattern:** Measure the diff's insertion/deletion counts immediately before staging, then read `git commit`'s **own** `--stat` line against that number. `23 insertions / 16 deletions` measured; `58 insertions / 19 deletions` committed. A foreign hunk cannot leave the count unchanged, so the number is the signal — and `git commit` prints it unprompted, so the check costs no extra command.

**Counterfactual:** without the comparison the capture is invisible, because **every other guard passed and each was correct to**. `unreviewed-content` printed `Passed` — this was an index commit, which its own § *WHAT IT DOES NOT CATCH* excludes by design, granting it *"the content was staged and is presumed reviewed"*. `git diff` on the single path had been read in its own call 40 seconds earlier and was genuinely clean. The commit named one explicit file. Four green signals, none of them wrong, and the capture sat underneath all of them. Detection would otherwise have waited for the owner to notice their work missing — which is the state `bug-fix-session-log:W-70` describes, where a captured session re-runs `append_entry` and allocates a fresh id for content already in `HEAD`.

**Why the count and not the filename:** the three axes every other check on this page depends on — do the sessions' files overlap, is the peer enumerable, was the commit pathspec-scoped or index-scoped — are exactly the axes that failed here (see `F-2`: the peer is *not* enumerable, 4 of 20). The count depends on none of them.

**Failure mode of this practice, measured 2026-09-01 over seven consecutive commits, and it is
structural rather than attentional.** The predicted stat is only a check if the prediction is
*measured*. Split by how the measurement was invoked:

| how | correct |
|---|---|
| `git diff --stat` as its own call, output read, message composed after | **5 / 5** |
| `git diff --stat && git add && git commit -m "…predicted N…"` in one chain | **0 / 2** |

Chaining puts the measurement and the write in the same invocation, so the message is composed
**before the output exists** — the prediction is written blind by construction, and "be careful"
cannot reach it. Both misses were mine (`c7be203f` 17/2 vs 19/1; `236a2873` 43/1 vs 47/0), and
the second landed in a commit whose own message discusses reading stats off artifacts.

**Why it matters beyond neatness:** a soft prediction degrades W-2's comparison from *detects a
foreign hunk* to *detects a large one*, because both sides of the comparison are then estimates.
The peer named that risk before I had a measurement for it (`codescout-e8`).

**Rule:** measure in a separate call, read the number, then write it. Never chain the stat into
the commit. This is the *make the correct path end in a safe state* shape — the split call has
no blind window to be disciplined about.

**What this does NOT establish, stated because I asserted it once already and it does not hold.** I told the user that disclosing the capture prevented the duplicate-work half, citing `d710e58d` — the owner's next commit — touching only `cluster-promotion-session-log.md` and `observer-blindness.md`, never `issue-clusters.md`, with no duplicated content (three marker strings, one occurrence each; verified). **That inference is unfounded.** I sent the disclosure to `codescout-fc`, which has since denied authoring any of it, so I have no evidence the message ever reached the owner. A world where disclosure did nothing and the owner simply had no `issue-clusters.md` work left pending produces the identical commit. It is the same error as `F-2` one layer up — attributing an effect to a party identified by proximity. The non-duplication is real and measured; its **cause** is not established.

**Confirming data points:**
1. This session — capture detected at commit time by count mismatch alone, after four correct greens.
2. Adjacent, not confirming: `codescout-5f` ran the same comparison successfully the same hour (`5d405b67`, measured 116/2 pre-stage, landed `116 insertions(+), 2 deletions(-)`). That is the check passing in a *clean* world, so it evidences the check is runnable, not that it detects.

**Measurement trap, from that same run:** `git diff | grep -c '^+'` counts the `+++ b/<path>` header, so its raw 117/3 was really 116/2. Compare `--stat` against `--stat`, or subtract the two header lines — a one-line discrepancy in either direction reads exactly like a small foreign hunk, so the trap manufactures a false positive for the very mechanism the check exists to find.

**Impact:** med — detection, never prevention; the commit has already happened when the number arrives. Its value is that it makes a capture *disclosable* at all.

**Promote-when:** a second capture is caught by this comparison, in a session that did not already know it was at risk. At 2 datapoints, promote to `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Detection* as standing practice rather than as one session's observation.

**Status:** validated — one datapoint, the detection itself measured and reproducible from the two numbers.

## F-3 — the split's load-bearing sentence was refuted as a population claim 67 seconds before it was committed

**Valid:** dated 2026-08-31

**Observed:** 2026-09-01, post-commit reconnaissance on `0dea224` (the `IC-1` → `IC-17` split). Scouting what the three peer commits that landed *during* my work had touched.

**When:** Immediately after committing the split. Two of the three peer commits — `6ada4d49` and `08d72a6e` — touched the seam the split's argument rests on, and neither was in HEAD when I began drafting.

**Expected (the ledger as committed):** `IC-1`'s falsification paragraph ends *"Enumeration was complete and changed nothing."* It is the load-bearing sentence: the whole split turns on enumeration having been available and having failed to prevent the collisions.

**Got (scouted reality):** `08d72a6e`, committed at 00:43:32 — **67 seconds before** my `0dea224` at 00:44:39 — measures `ListAgents` returning **4** peers across three calls spanning 20 minutes while `ls /run/user/1000/cc-socks/` returns **20**, with both of the relevant session addresses present in the socket list. So it is a strict 4-of-20 subset, not a stale snapshot. Read as a claim about the *population*, "enumeration was complete" is false on this project's own data.

**Probable cause:** The sentence was written in `d710e58d` (00:32) and I inherited it while editing the same entry twelve minutes later. Nothing was wrong at either write. The defect is that a *quantified* claim and a *per-pair* claim were expressed in one unqualified sentence, so a later measurement refutes the reading the words invite while leaving the reading the argument uses intact.

**Corrected 2026-09-01, on two independent counts, neither found by me.**

*(1) The attribution was wrong, and it is this ledger's own class.* As first written, the line above read *"before **its author** took that measurement"* — tying `F-2` to whoever wrote `d710e58d`. `F-2` and `W-2` were appended by a third party that the file, `git` (one author string for every session here) and the catalog all fail to identify. I inferred authorship from **file ownership**, while reading a log about misattribution, having corrected myself for this exact class twice in the preceding hour. File-ownership is a *better* proxy than commit-proximity and is still wrong — which is the point worth keeping. **Cite the entry, never a party.**

*(2) `F-2`'s number does not survive a units check.* `4 of 20` compares a profile-scoped instrument against a raw socket-file count. Measured on this host rather than inherited:

| quantity | count |
|---|---|
| socket files in `/run/user/1000/cc-socks/` | 20 |
| … of which the process is **dead** (stale file only) | 7 |
| live sessions machine-wide | 13 |
| live sessions with `cwd` inside this checkout | 5 (incl. this one) → **4 peers** |
| peers `ListAgents` returns | 4 |
| … of which are in **another checkout** (`mirela/backend-kotlin`) | 3 |
| … of which share this working tree | **1** |

The instrument is wrong in **both directions at once**: it misses 3 of the 4 peers who can reach these files, and reports 3 who share no tree with this session at all. The honest figure for the question `IC-1` asks — *can the writing session see the peers who can reach its files?* — is **1 of 4**, not 4 of 20. That 20% and 25% land close together is precisely what makes a unit mismatch hard to notice: the mis-scoped ratio is not absurd, it is *plausible*.

**Both corrections strengthen the split.** The population reading of *"enumeration was complete"* is not merely false, it is **75% false** on the population that matters. And the pair whose collision fired the falsifier sat inside the visible quarter — mutually enumerable while three of four same-checkout peers were not. So the falsification ran on the *most favourable sample available for coordination*, and coordination failed four times anyway. A sample unrepresentative in the direction **against** the conclusion is the strongest kind there is.

**Not annotating `F-2` itself.** It is another party's entry, marked `high` and `open`, and rewriting it would repeat the error this section records one layer down. The measurement above is offered beside it, not over it.

**Why it matters more than a wording slip:** the falsifier `IC-1` pre-registered needs only the **pair** — *"an instance where the writing session could enumerate the peer and still collided"* — and that is exactly what was measured (both sessions in each other's listing, eight messages exchanged). So `IC-17` never depended on the population claim. But a reader checking the ledger against `F-2` would refute the visible sentence and could reasonably discard a correct partition on it. A load-bearing sentence that is false under its natural reading and true under its intended one is worse than one that is simply wrong, because the error survives review by anyone who agrees with the conclusion.

**The direction is the interesting part.** A 4-of-20 instrument makes the *visibility* class worse than `IC-1` assumed, so the same measurement **strengthens `IC-1`** while leaving `IC-17`'s evidence untouched. Two halves of a former single class moving independently under new evidence is what a correct split should look like — this is the first post-hoc test the partition has had, and it passed.

**Workaround:** Narrowed the sentence in place to *"complete for this pair"*, with the 4/20 numbers, the `cluster-promotion-session-log:F-2` citation, and an explicit note that the falsifier needs only the pair. Not deleted — the original reading is what a future reader will attempt, so the refutation has to be visible next to it.

**Severity:** med — the split is unaffected and no member moves; the cost is a correct partition being discardable by a reader who checks it. Would be `high` if the population claim had been the one the falsifier consumed.

**Status:** fixed-verified — narrowing landed in the ledger; `tests/issue_clusters.rs` 6/6 unaffected (it gates tags, not prose).

**Rests on:** the falsification condition being per-pair, as `IC-1` itself worded it. If it is ever restated in population terms, this entry's resolution stops holding and the split needs re-arguing.

**Fix idea / Pointer:** `docs/trackers/issue-clusters.md` `IC-1`, falsification paragraph; `cluster-promotion-session-log:F-2` for the measurement; commit `0dea224` for the split.

## F-4 — the ledger's count cells go stale by CONCURRENCY, so a manual sweep is invalidated by the next commit

**Observed:** 2026-09-01, across one session that re-derived `docs/trackers/issue-clusters.md`'s
Index counts three separate times.

**When:** Post-rebuild reconnaissance, after committing a blind second-read audit whose commit
message asserted *"All 17 table cells re-derived against `git ls-files` and matching."*

**Expected:** A hand-maintained count table goes stale when someone forgets to update it — a
neglect failure, fixed by a sweep.

**Got (measured):** It goes stale because **peer sessions file bugs into the same checkout while
you are deriving**. Three invalidations, none of them anyone's mistake:

1. First derivation (session start) gave IC-3 = 20, IC-6 = 29 and the table said 19 / 27. Fixed
   in `eee6eedf`.
2. During the blind audit — the three classifier subagents ran ~6–9 minutes — a peer filed
   `docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md` (IC-3) and
   `docs/issues/archive/2026-09-01-status-locator-reads-any-table-row-as-a-status-row.md` (IC-6), taking
   them to 22 and 30. Caught only because the retag pass re-derived all 17 cells before
   committing `d928932e`.
3. About two hours later, the peer's audit-trail work landed three more archived bug files —
   `audit-records-the-statement-not-the-change-so-98-percent-is-empty` (IC-2),
   `audit-growth-concentrates-in-augmentation-params-health-blind-to-bytes` (IC-13),
   `audit-trigger-can-abort-writer-on-null-key-or-blob` (IC-14) — each +1, so `d928932e`'s
   "all 17 match" claim was false within hours of being true.

**Probable cause:** The count is a *query result* stored as *text*. Nothing binds the two. The
ledger already knows this — its own preamble says `n` is a snapshot — but the remedy it
prescribes is re-derivation, which is an act with a timestamp, and therefore has the same decay
the cell does. The gate that exists (`tests/issue_clusters.rs`) enforces **tag validity** — one
known slug per tracked file under `docs/issues/` — and does not read the Index table's numbers at
all, so a drifted cell is mergeable.

**Workaround:** None that holds. Re-deriving before any judgement that quotes a count is correct
and is what caught instances 2 and 3, but it cannot make the committed cell true afterwards.

**Severity:** med — no wrong judgement shipped, because every promotion call this session
(`IC-10` clearing both bars, `IC-6`'s rule already landed) was made against a freshly derived
number rather than a read cell. The cost is a commit message asserting a state that is no longer
true, in a file whose entire purpose is that its counts drive promotion decisions.

**Fix idea / Pointer:** Extend `tests/issue_clusters.rs`. It already has the two halves it needs:
`valid_slugs()` parses this ledger's `**Slug:**` declarations, and the test walks the bug corpus.
Add an assertion that each Index row's `n` equals the count of files carrying that slug. Note the
population differs from the existing check — cells count **open + archive**, while the tag-validity
check is deliberately open-only — so it is a second walk, not a reuse of the first. That makes a
drifted cell a red test rather than a thing the next reader happens to notice.

**Status:** fixed-verified — gate shipped in `tests/issue_clusters.rs` as
`every_index_count_matches_the_corpus`, with `every_declared_class_has_an_index_row` guarding it
against passing vacuously. **It caught a real drift on its first run**, ~40 minutes after a hand
re-derivation had reported all 17 cells matching: `IC-17` table 15 vs corpus 16, from a peer's
`workspace-activation-is-process-wide-and-a-subagent-can-flip-it` filed in between. That is this
entry's own claim reproducing itself against the fix for it, which is as direct a confirmation as
the friction could get.

**Valid:** dated 2026-09-01

**Rests on:** the ledger being a shared, concurrently-written artifact in a checkout with more
than one active session — not on any property of the counting method, which was correct each time.

## W-3 — blinding by redaction beat blinding by instruction — and the second leak channel was the one that mattered

**Observed:** 2026-09-01, designing the independent blind second read of the 2026-09-01 archive
classification pass (43 files, three Opus readers, `docs/trackers/issue-clusters.md`).

**Pattern:** When a review must be blind to a prior verdict, **remove the information from the
copy the reviewer reads** rather than instructing the reviewer not to look. Then ask what the
*second* channel is: the obvious identifier is rarely the only one carrying the answer.

Here the obvious channel was the `cluster/<slug>` frontmatter tag. The second was `IC-N` tokens
in prose — these bug files cite each other's classes, and a ledger entry's id is as good as its
slug. Both were redacted in working copies (`sed -E 's#cluster/[a-z0-9-]+#cluster/REDACTED#g;
s#\bIC-[0-9]+\b#IC-REDACTED#g'`), verified zero-leak on both patterns across all 43 files before
dispatch, with the ground truth held outside the readable directory.

**Counterfactual:** `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md`
frames its own classification in its `## Resume` and `## References` sections — *"this file's tag
is `cluster/…` and that is the trap"*, plus a reference line naming its `IC-N` disclaimer. Its
reader met that framing as `cluster/REDACTED` / `IC-REDACTED`, reasoned from the file's measured
root cause instead, and returned a **different class** — which the adjudication then confirmed
against the file's own text and applied, taking `IC-10` to n=3 and tripping its promotion
condition. Slug-only redaction would have shown that reader the string `IC-14` twice in prose,
anchoring it on the verdict under audit. The single highest-value finding of the audit came from
the channel the obvious redaction misses.

**Confirming data points:**
1. This session — `IC-N`-in-prose was a live leak channel in at least one of 43 files, and it was
   the file whose retag had a promotion consequence.
2. Pending: any future blind review in this repo where the second channel is checked for.

**Impact:** high for audit validity — a blind review that leaks is worse than no review, because
its agreement rate is then evidence about the leak rather than about the taxonomy. The 37/43
figure is only meaningful if blindness held.

**Promote-when:** a second blind-review design in this repo finds a non-obvious leak channel by
asking the question. At 2 datapoints, promote to the reconnaissance skill as a Phase 1 bullet:
*"blinding is structural, not instructed — redact the copy, then name the second channel."*

**Status:** validated — single datapoint, leak channel identified and closed before dispatch,
zero-leak verified by probe rather than assumed.

**Valid:** dated 2026-09-01

**Rests on:** the general principle that a review's independence is a property of what the
reviewer can *read*, not of what they were *told* — the same reasoning as `CLAUDE.md`
§ *Observer Blindness*'s third requirement, a check that runs when nobody is worried.

## F-5 — IC-13's claim is true of 4 of its 16 members, and the floor I carried pointed the wrong way

**Observed:** 2026-09-01. `IC-13`'s claim text was about to be ruled on using the figure *"at
least four members where the claim is false"*, carried over from the blind second-read audit.

**When:** Before the ruling, on the instinct that a floor two readers noticed while doing a
*different* job is not a measurement of a population.

**Expected:** The measurement would confirm the floor and put a denominator under it — roughly
"4-ish of 16, so widen the claim's wording."

**Got:** The floor was carried in the **wrong direction**, and the ruling it pointed at is the one
the evidence supports least. All 16 bodies read against the claim's *"without a marker"* clause,
one reader per file, quote required per verdict, and the expected answer deliberately withheld
from the readers:

| | | n |
|---|---|---:|
| **A** | claim holds as written — no marker anywhere | 4 |
| **B** | marker computed correctly, never reaches the reader | 5 |
| **C** | neither — cap announced, or no cap at all | 7 |

14 of 16 high confidence. **The claim as written is true of 4 of its 16 members.** "At least four"
had been read as bounding the members the claim *fails* for; it bounds the ones it *holds* for.
The true failure figure is **12 of 16**.

C is not one bucket, and the readers' own evidence forced the split: **C1 (4)** announce their cap
reachably and file a different defect; **C2 (2)** involve no truncation at all, classified "C by
elimination, not by fit"; **C3 (1)** carries a marker that is *present and wrong* — `grep`'s
`Showing N of N`, whose true total after `hit_cap` is **unknowable** rather than unreported, so
"add a marker" is not an available remedy. C3 is a fourth shape the three offered categories could
not express, and it surfaced only because rows carried quotes.

**Probable cause:** two compounding errors, both mine. The floor came from subagent reports —
which `reconnaissance-patterns` Law A names explicitly as *claims*, not artifacts — and it was
restated across three of my own messages without once being re-derived, so each restatement
inherited the direction of the first. Second, the framing ("the claim is too narrow") selected
what the number seemed to mean: a wording problem predicts B, so a floor of 4 read as "4 B's",
when B is in fact the *smallest* bucket and the membership problem is the largest.

**Workaround:** none needed — the measurement ran before the ruling, which is the whole of the
save.

**Severity:** med — nothing shipped. Had the ruling gone ahead on the floor, IC-13's claim would
have been widened to cover B while 7 non-members stayed in the class, which retroactively
legitimises drift and makes the class *less* falsifiable. Two of those seven disclaim the class in
their own text (`grep-narrowing-hint`, `append-entry-anchor`), so the wrong ruling would have
contradicted the corpus in writing.

**Fix idea / Pointer:** Two rulings owed, deliberately **not** folded together and in this order:
(1) widen the claim's clause to *"without a marker the caller can see"*, covering A+B = 9; (2)
re-adjudicate the 7 C's, which would take n from 16 to ~9. Order matters — widening first while
C members remain would legitimise them. Both are recorded in `IC-13`'s *Open ruling*. The
measurement itself is **one reader per file with no cross-check**, the same single-party property
the Index caveats for the archive pass, so 4/16 is a measurement and not yet a corroborated one.

**Status:** open — measurement complete and recorded; both rulings await a human call.

**Valid:** dated 2026-09-01

**Rests on:** the 16 per-file verdicts, each carrying a direct quote from the bug file, and on the
two members whose own text disclaims the class — those two need no adjudication from anyone.

## F-6 — ruling 2 — all 7 non-members fit NO existing class, unanimously; the readers split only on granularity

**Observed:** 2026-09-01, ruling 2 of the `IC-13` pair — re-adjudicating the seven members the
claim measurement (`F-5`) found outside the class.

**When:** After ruling 1 widened the clause. Run with **two independent readers over the same
seven files** rather than a split, deliberately: a membership ruling moves counts, and every
classification pass this session until now had been one reader per file.

**Expected:** Most of the seven would find homes among the other 16 classes, with maybe one
no-fit.

**Got:** **7 of 7 `NONE`, from both readers independently.** Not one of the seven belongs to any
existing class. Both readers reached this against all 17 claims, with `NONE` explicitly offered as
a first-class answer and forcing a fit explicitly forbidden.

**The agreement is stronger than the headline.** Comparing per file, the two readers produced
**the same remedy for all seven**, in some cases near-verbatim — file 3 as *"stop printing a
denominator you cannot compute; mark the number a floor"* against *"drop the denominator when
collection capped; publish count as floor"*; file 6 as *"window both ends; point the error at a
surface holding the anchor"* against *"make the withheld tail reachable; point the error at a
working surface"*. Since the remedy is this ledger's discriminator, seven agreed remedies is
agreement on the substance.

**They disagree only on GRANULARITY** — how to group those seven remedies into classes:

| file | reader 1 (3 classes) | reader 2 (4 classes) |
|---|---|---|
| 1 `heading-scoped-get-overflow-hint` | `MC-A` route-cannot-reach-payload | `M-D` hint-composed-without-the-request |
| 6 `append-entry-anchor` | `MC-A` | `M-A` wrong-key window (+`M-D`) |
| 2 `audit-doc-refs-gate` | `MC-B` derivation-on-surviving-window | `M-A` truncated-window-ordered-by-wrong-key |
| 4 `grep-narrowing-hint` | `MC-B` | `M-A` (+`M-B`) |
| 3 `grep-showing-n-of-n` | `MC-B` | `M-B` floor-published-as-a-total |
| 5 `unfiltered-output-ref` | `MC-C` magnitude-omitted | `M-C` instrument-omits-the-dimension |
| 7 `audit-growth` | `MC-C` | `M-C` |

**Files 5 and 7 are unanimous** — same pair, same claim, both readers: *a surface reports presence
or a count where the decision turns on magnitude*. That one needs no adjudication.

The dispute is whether *ordering-the-window* and *publishing-a-floor-as-a-total* are one class or
two, and whether file 6 is primarily a bad window or a bad route. Each side has a real argument
and they are not the same kind of argument:

- **Reader 1 merges** (`MC-B` = files 2, 3, 4) on **corpus evidence**: file 4's fix *reproduced
  file 3's defect one level down* and had to add the floor marker anyway — *"Without this the fix
  would have replaced one piece of false precision with another one level down."* Entanglement
  observed in the repair history, not argued from the claim.
- **Reader 2 splits** on the **remedy test**: *"M-B removes a false name and M-C adds a missing
  dimension. Two remedies, two classes."* Which is this ledger's own stated discriminator.

**Probable cause of the split:** the two passes cut on different axes and both said so. `F-5`
partitioned on *marker visibility* (C1/C2/C3 = 4/2/1); this pass partitions on *remedy*
(2/3/2). Reader 1 named the crossing explicitly — `MC-B` spans the C1/C3 boundary because grep's
two files share a remedy family while sitting either side of the visibility line. The partitions
are not in conflict; they answer different questions.

**Severity:** med — nothing is wrong in the tree. But the seven are still tagged `IC-13`, and
`IC-13`'s count cannot become honest until they move, so the ledger currently publishes n=16 for
a class whose claim reaches 9 of them.

**Fix idea / Pointer:** Open the new classes and retag. Blocked on one decision only: **3 classes
or 4.** Not blocked on membership, which is unanimous. Two consequences worth weighing before
choosing — reader 2's `M-A` arrives at **3 instances across 3 subsystems**, clearing the promotion
bar on creation, and reader 1's `MC-B` would arrive at 3 as well; the finer partition creates one
more class that starts at n=1. Whichever is chosen, `IC-13` falls 16 → **9**, which is exactly
what `F-5` projected.

**Status:** fixed-verified — **four-class partition taken 2026-09-01** (reader 2's finer cut, on
the remedy test being this ledger's stated discriminator). `IC-19`
`truncated-window-ordered-by-the-wrong-key` (n=3), `IC-20` `floor-published-under-the-name-of-a-total`
(n=1), `IC-21` `instrument-omits-the-dimension-that-grows` (n=2), `IC-22`
`hint-composed-without-the-request` (n=1). All seven retagged through the catalog; `IC-13` fell
16 → 9, exactly as `F-5` projected. Reader 1's merge argument is **not discarded** — it is
recorded inside `IC-20`, whose entry carries the corpus evidence that fixing a wrong ordering
hands you a wrong denominator, so the two co-occur even while they stay apart. The gate
re-derived every count and passed.

**Valid:** dated 2026-09-01

**Rests on:** two independent readers over the same seven files, every row carrying a direct quote
and an explicit remedy, and on the seven remedies agreeing pairwise — which is what makes this a
granularity dispute rather than a classification one.

## F-7 — the ledger shipped without its corpus — the gate reads the working tree, so a partial commit is invisible to it

**Observed:** 2026-09-01, post-rebuild reconnaissance immediately after `461c037a` shipped
ruling 2.

**When:** Comparing `HEAD` against the working tree — a habit adopted an hour earlier only
because a peer session reported the count gate going red on them for a reason its message could
not express.

**Expected:** `461c037a` shipped ruling 2 whole. The gate was green when I committed it.

**Got:** It shipped the **ledger** and not the **corpus**. Six of the seven retagged bug files
were never staged, so at `HEAD`:

| slug | working tree | HEAD |
|---|---:|---:|
| `capped-result-presented-as-complete` | 9 | **15** |
| `truncated-window-ordered-by-the-wrong-key` | 3 | **0** |
| `floor-published-under-the-name-of-a-total` | 1 | **0** |
| `instrument-omits-the-dimension-that-grows` | 2 | **0** |

Three of the four newly-opened classes had **zero members at `HEAD`** while the Index published
3, 1 and 2. Repaired in `4f598b5b`, and `HEAD` now derives 9 / 3 / 1 / 2 / 1.

**Probable cause — and the gate could not have caught it.**
`every_index_count_matches_the_corpus` runs `git grep` against the **working tree**, so it
validates *the state on disk*. That state was correct and self-consistent the whole time. A commit
that takes **some** of that state and not the rest produces a `HEAD` the gate never examined —
there is no moment at which the check runs against what was actually shipped. Local green, CI red,
and the red surfaces to whoever pushes next rather than to the author.

This is one turn sharper than the tracked-only property documented in that test's doc comment an
hour ago. That one is about files git **cannot see**; this one is about files git **can** see and
the commit did not take. Both make a local green a **deferral** rather than a clearance, which is
now two mechanisms with one consequence.

**Severity:** med — nothing was wrong on disk at any point, and no judgement rested on the bad
state. The cost was a `HEAD` that published counts its own corpus contradicted, discoverable only
by CI or by a scout that thought to compare the two.

**Fix idea / Pointer:** The invariant is a **commit-boundary** one — *at every commit, the
ledger's counts match the corpus at that commit* — and pre-commit is where this repo's other
commit-boundary invariants already live (`refuse a pathspec commit carrying unstaged content`,
`refuse an index commit carrying another session's staged paths`). A hook running the same
derivation against the **staged** state (`git grep --cached`, or a temp-index checkout) closes it
exactly. Deliberately **not** done by changing the test to read the index: that would red on every
ordinary unstaged edit, which is the state the test is useful in.

**Status:** **fixed-verified 2026-09-01** — `3be0088e`, patch-id `e27ec0a41eca7afa71bd591caf03c2ae5fa46292`. Built as `scripts/pre-commit-ledger-counts.py`, hook `ledger-counts`, reading the **index** only (`git ls-files` for the population, `git show :<path>` for content). Both partial-commit directions were reproduced against the real shape in a throwaway repo and both are refused: the ledger staged with retags unstaged (this entry's own shape) and a new tagged bug file staged without the ledger; the correct commit with both staged passes.

**Three things the build changed about the plan above, and each is worth more than the fix.** The `git grep --cached` sketch was not used — `--cached` searches the index for a *pattern* and yields no population, and the count needs `git ls-files` for *which* files the commit contains. The temp-index-checkout alternative proved unnecessary once measured: `git show :<path>` reads staged content directly. And the **language** was decided by a prohibition already written into `.pre-commit-config.yaml`'s own header — *"only checks that do not build may run at commit stage"*, because a cargo build takes the shared `target/` lock and serialises every concurrent session's commits. Measuring `cargo test --test issue_clusters` at **6.9s warm** rediscovered that independently, but the rule was there to be read first. A proposed fix is a claim about current state (`reconnaissance-patterns:R-49`), and so is a proposed *implementation*.

**The duplicated parse logic is held by a mechanism rather than by care** — `the_hook_script_agrees_with_this_gate` runs the script over the same substrate and compares both maps, so divergence reds. That test has a gap it structurally cannot see: mutating the Python to drop the inline `tags: [a, b]` arm left it **green**, because **zero** bug files in the corpus carry a `cluster/` tag in YAML flow style, so the live corpus cannot reach that branch. Closed by `the_hook_script_agrees_on_both_yaml_tag_styles`, pure over stdin. Four mutations run: two killed, one killed only by the new test, one survived as an **equivalent mutant** (the two block-form arms overlap on the same input) — established as equivalence rather than filed as a coverage gap.

**Valid:** dated 2026-09-01

**Rests on:** the measured `HEAD`-vs-worktree divergence above, and on the gate's own
implementation reading the working tree — not on any claim about how the commit was made.

## F-8 — a refusal hint's stated condition was always true of the call it refused, and I reported the guard as broken instead

**Valid:** dated 2026-09-03

**Observed:** In a user-facing report I wrote that `edit_file`'s librarian guard "refuses a
**body-only** edit while its own hint says body edits work" — offered as a probable defect, on
the evidence of exactly one refused call. Scouting the guard before the claim went any further
showed the central assertion is **false**. Recording it because the retraction is the cheap
part; the reason I made it is the reusable part.

**Reality, read this session:**

- `Access::FrontmatterWrite` is documented at `src/util/librarian_guard.rs:37-40` as *"A write
  that touches frontmatter, **or one whose extent the caller cannot bound**"* — the second
  clause is the whole answer and I had not read it.
- `edit_file`'s text grammar passes that value **deliberately**
  (`src/tools/edit_file/mod.rs:754-764`): a raw `old_string` may match *inside* the frontmatter
  block, so the call cannot promise body-only without asserting a negative it has not
  established. The comment says so and cites
  `docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md`.
- `edit_markdown`'s heading-addressed path *can* bound its extent, so it computes
  `access` from whether `frontmatter` is an object (`src/tools/markdown/edit_markdown.rs:1305-1309`)
  and passes `BodyWrite`. That is why the identical edit went through as markdown grammar
  moments after being refused as text grammar.

So the asymmetry I read as a contradiction is a deliberate, documented, per-grammar
conservatism with a bug file behind it, and `guard_not_librarian_managed` is behaving
correctly.

**What survives, and it is small:** the hint at `src/util/librarian_guard.rs:206-207` reads
*"Reads and BODY edits are allowed directly — read_file, and `edit_file` without its
`frontmatter` param, both work on this file."* `edit_file`'s **text** grammar has no
`frontmatter` param at all, so the hint's stated condition is **always true of the very call it
refuses**. The real discriminator is *extent-bounded (heading-addressed)* versus *extent-unbounded
(raw text)*, and the hint names neither. A caller who follows it literally — as I did — takes a
refused route and then reads the refusal as a contradiction, which is the loop this entry is.

**Category:** documentation / diagnostic wording. **Severity:** low — one refused call, no
data at risk, and the markdown grammar is right there. Raised above cosmetic only because the
hint's failure mode is to send the reader hunting for a guard bug that does not exist.

**Classification deliberately NOT made.** The shape resembles `IC-22` — the hint is composed
from the file's state (`stamped_only`) with no knowledge of which grammar called, so it
prescribes a route the caller structurally cannot take, which is `IC-22`'s fifth member almost
exactly. Not filed as a member, on an argument `codescout-e2` (sessionId `63083c9e`) made the
same evening about a sibling finding and which applies here unchanged: `IC-22`'s claim requires
the named route to return plausible data so that following it reads as *progress*, and this
reads as a *different error*; the entry already flags its grain question twice and warns against
widening it as a side effect of a filing; and the class documents a refusal to cross the ≥3 bar
by re-partitioning, since *"a promotion bought that way is indistinguishable, at the point of
use, from one a real third instance earned."* Settling `IC-22`'s grain question is worth more
than adding this to it. Left as an open question, not resolved by filing.

**Why the scout was worth running at all — this is `R-19` live.** The trigger was not an edit;
it was a *sentence*. The refusal had already happened, I had already routed around it, and the
task was complete. What made it a seam is that I had put a specific, checkable claim about
current code into a report to the user, from one observation and no reading. The skill's own
carve-out says describing behaviour is Q&A but *asserting* a checkable fact is not — and the
assertion here was wrong in its central claim while being right enough at the edges to survive
casual scrutiny. **Nothing downstream would have caught it:** the guard is not under test from
my side, the claim was in prose, and the next reader's most likely response is to trust it and
either file a phantom bug or stop using `edit_file` on stamped files.

**Status:** open — the hint wording is unfixed and unowned. Fixing it is a one-line change at
`src/util/librarian_guard.rs:206-207` naming the grammar rather than the parameter; whoever
takes it should decide the `IC-22` grain question first, because the answer determines whether
this is a wording fix or a member.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<this artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's own level,
     records the high-water mark and stamps `**Valid:** dated <today>` — one write.
     Then add the Index / Wins Index row with the id it returned. -->
