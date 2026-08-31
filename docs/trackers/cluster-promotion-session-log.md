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
entry_high_water_F: 3
entry_high_water_W: 2
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

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-09-01 | med | re-probe the served copy after a rebuild (`readlink /proc/<pid>/exe`) | 5 of 11 peers on deleted inodes, invisible to every existing instrument; an open bug's fix shown to fail open | validated |
| W-2 | 2026-09-01 | med | compare pre-stage diff counts against `git commit`'s own `--stat` | capture of another session's hunks was invisible to four guards that all passed correctly; 23/16 measured vs 58/19 committed | validated |

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

**Status:** open — correction not yet written to `OB-7`.

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
`docs/issues/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md`. So the state
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
## W-1 — Re-probing the served copy after a rebuild found 5 of 11 peers on deleted inodes, and an open bug's fix failing open

**Observed:** 2026-09-01 00:35, immediately after the operator rebuilt the release binary and reconnected. Scouting whether the rebuild invalidated anything published earlier in the session.

**Pattern:** After a rebuild, **re-probe the copy the serving process actually holds, and check the same for every peer** — `readlink /proc/<pid>/exe`. A rebuild is a substrate change under every tool-behaviour claim made before it, and the claims do not announce their own staleness.

**Counterfactual:** two things would have gone unnoticed.

1. **`OB-7`'s mechanism claim was verified against the pre-rebuild binary.** Re-probed on the fresh one and it holds identically — `grep` still annotates each hit with its enclosing symbol and test sites still carry a `tests/` prefix. A *match*, and the point is that it was not knowable without the probe: this is the one claim in the entry that had already needed correcting once, so inheriting it across a substrate change would have been the second unverified inheritance of the same sentence.

2. **5 of 11 `codescout start` servers were holding deleted inodes at that moment** — started 11:20, 11:38, 14:46, 21:45 and 22:20, all pre-dating the 00:34 rebuild. Every existing instrument called them healthy: `peer-sessions.sh` prints start time and `cwd`, `ListAgents` lists sessions, and `~/.cargo/bin/codescout` is a correct symlink to the right path. Nothing in any of those outputs distinguishes a process serving current bytes from one serving replaced bytes.

**And the scout found a defect in an open bug's *proposed fix*, which is the part worth carrying.** `docs/issues/2026-08-31-peer-sessions-never-compares-start-time-to-build-time.md` prescribes comparing process start time against binary mtime. Run against a genuinely stale pid it prints **nothing**: `readlink` returns the path with a literal ` (deleted)` suffix, `stat` on that string fails, `[ N -lt "" ]` errors to stderr and evaluates false, and the `&&` chain short-circuits. The one branch that must fire is the only one that cannot, and a caller redirecting stderr sees a clean report. The discriminator was already in the string the fix had just read and was discarded in favour of a timestamp proxy — which is `OB-6` in a proposed remedy rather than in shipped code.

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

**Status:** open — no mechanism. The instrument's own population is not queryable through the instrument, which is why three passive re-reads produced three identical wrong answers.

**Rests on:** the socket count being a live-session proxy — supported here by both known session addresses appearing in it, but not independently established as complete either.

**Fix idea / Pointer:** this is `IC-1`'s *visibility* half with a number attached, and the number is the contribution — the class text says "the peer cannot be enumerated", which reads as a possibility rather than a 5× under-count. Related: `docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md`, `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

## W-2 — The commit's own --stat caught a capture that four correct guards passed

**Valid:** dated 2026-08-31

**Observed:** 2026-09-01, commit `d617051b` — a working-tree capture on a shared checkout, detected within one call of committing.

**Pattern:** Measure the diff's insertion/deletion counts immediately before staging, then read `git commit`'s **own** `--stat` line against that number. `23 insertions / 16 deletions` measured; `58 insertions / 19 deletions` committed. A foreign hunk cannot leave the count unchanged, so the number is the signal — and `git commit` prints it unprompted, so the check costs no extra command.

**Counterfactual:** without the comparison the capture is invisible, because **every other guard passed and each was correct to**. `unreviewed-content` printed `Passed` — this was an index commit, which its own § *WHAT IT DOES NOT CATCH* excludes by design, granting it *"the content was staged and is presumed reviewed"*. `git diff` on the single path had been read in its own call 40 seconds earlier and was genuinely clean. The commit named one explicit file. Four green signals, none of them wrong, and the capture sat underneath all of them. Detection would otherwise have waited for the owner to notice their work missing — which is the state `bug-fix-session-log:W-70` describes, where a captured session re-runs `append_entry` and allocates a fresh id for content already in `HEAD`.

**Why the count and not the filename:** the three axes every other check on this page depends on — do the sessions' files overlap, is the peer enumerable, was the commit pathspec-scoped or index-scoped — are exactly the axes that failed here (see `F-2`: the peer is *not* enumerable, 4 of 20). The count depends on none of them.

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

**Probable cause:** The sentence was written in `d710e58d` (00:32) before its author took that measurement, and I inherited it while editing the same entry twelve minutes later. Nothing was wrong at either write. The defect is that a *quantified* claim and a *per-pair* claim were expressed in one unqualified sentence, so the later measurement refutes the reading the words invite while leaving the reading the argument uses intact.

**Why it matters more than a wording slip:** the falsifier `IC-1` pre-registered needs only the **pair** — *"an instance where the writing session could enumerate the peer and still collided"* — and that is exactly what was measured (both sessions in each other's listing, eight messages exchanged). So `IC-17` never depended on the population claim. But a reader checking the ledger against `F-2` would refute the visible sentence and could reasonably discard a correct partition on it. A load-bearing sentence that is false under its natural reading and true under its intended one is worse than one that is simply wrong, because the error survives review by anyone who agrees with the conclusion.

**The direction is the interesting part.** A 4-of-20 instrument makes the *visibility* class worse than `IC-1` assumed, so the same measurement **strengthens `IC-1`** while leaving `IC-17`'s evidence untouched. Two halves of a former single class moving independently under new evidence is what a correct split should look like — this is the first post-hoc test the partition has had, and it passed.

**Workaround:** Narrowed the sentence in place to *"complete for this pair"*, with the 4/20 numbers, the `cluster-promotion-session-log:F-2` citation, and an explicit note that the falsifier needs only the pair. Not deleted — the original reading is what a future reader will attempt, so the refutation has to be visible next to it.

**Severity:** med — the split is unaffected and no member moves; the cost is a correct partition being discardable by a reader who checks it. Would be `high` if the population claim had been the one the falsifier consumed.

**Status:** fixed-verified — narrowing landed in the ledger; `tests/issue_clusters.rs` 6/6 unaffected (it gates tags, not prose).

**Rests on:** the falsification condition being per-pair, as `IC-1` itself worded it. If it is ever restated in population terms, this entry's resolution stops holding and the split needs re-arguing.

**Fix idea / Pointer:** `docs/trackers/issue-clusters.md` `IC-1`, falsification paragraph; `cluster-promotion-session-log:F-2` for the measurement; commit `0dea224` for the split.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<this artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's own level,
     records the high-water mark and stamps `**Valid:** dated <today>` — one write.
     Then add the Index / Wins Index row with the id it returned. -->
