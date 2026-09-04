---
kind: tracker
status: active
title: 'authorship on a shared checkout is unrecoverable after the fact, so every party infers it from proximity'
owners:
- marius
tags:
- defect-classes
- clusters
- authorship-unrecoverable-after-the-fact
topic: issue clusters and rule promotion
---

## IC-10 — authorship on a shared checkout is unrecoverable after the fact, so every party infers it from proximity

**Slug:** `cluster/authorship-unrecoverable-after-the-fact`
**Claim:** On a shared checkout there is no attribution channel, so authorship cannot be recovered after the fact. Every party therefore infers it from proximity — who else was active, which file appeared when — and proximity is not evidence.
**Members:** `filter={"tags": {"contains": "cluster/authorship-unrecoverable-after-the-fact"}}` **New member 2026-09-02: `a-sessions-self-reported-name-is-not-self-verifiable`** — a refinement of this claim rather than another instance of it. The claim says authorship is inferred from proximity for want of a channel; this member shows the **positive** channel the corpus prescribes as the escape from proximity-inference is itself partially decayable. A sessionId is structural (a scratchpad-path component, so a session can read its own off disk); a NAME is registry-derived and re-minted by compaction or resume with nothing re-informing the running context. So the remedy is not *ask* versus *infer* — it is **ask for the sessionId, not the name**. — `n=3`, 2026-09-01, by query. The seed narrative below is not a bug file and is not counted. Second member `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md` is the **read-side**: the write-side asks *who wrote this*, the read-side asks *is this mine* — and on a shared checkout neither is answerable for uncommitted state. Subsystem spread is now 2 (companion plugin banner; shared build + git tooling), so a third instance meets the promotion threshold. **That third arrived the same day, and it is a third subsystem** — `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md`, retagged here from `IC-14` by the independent blind second read on the file's own measured root cause: *"the cause is attribution, not enumeration"*, and the guard *"behaved correctly throughout … a correct consumer of corrupted input"* — which is `IC-14`'s claim falsified rather than merely outweighed. Its stage log assigned each `(blob, path)` pair to whichever session's hook **observed** it first, and `git status` fires that hook, so a staged batch was claimed by whoever polled next: proximity read as authorship, exactly. Spread is therefore 3 (companion plugin banner; shared build + git tooling; the pre-commit stage log) and **both promotion bars clear as of 2026-09-01**. **+1: `a-transported-catalog-carries-its-host-identity`** (2026-09-04) — `resolve_host_id` (`src/librarian/catalog/audit/host.rs:159`) reads `audit_host_id` from **`catalog_meta`** and mints one only when absent, so a host that receives another host's catalog adopts its audit identity silently. Observed: `catalog_meta.audit_host_id = ripper-65e654` on a machine whose hostname, `/etc/hostname` and every `candidate_name()` env source all yield `archlinux`, after `catalog.db` was replaced wholesale at 09:45 (backup `catalog.db.bak-preworkstation-20260904-094517`, 7.4 MB → 124 MB). 20,011 rows in `.codescout/audit/ripper-65e654-202609.jsonl` are stamped with that host, 58 of them written by a session running on the laptop. **Why it belongs here rather than in a config-propagation class:** the axis shifts from SESSION authorship to HOST authorship, but the claim is the one this class states — after the fact, no instrument can say which machine wrote a row, and `read_shards` keys on the host segment of the filename so it cannot separate them later. **What makes it the sharper instance:** this class's own shipped mechanism is the thing that breaks. `516da1df` committed the first shard as *"commit this host's audit shard so a clone can answer for its history"*, and `shard_file_name`'s doc comment states the lost invariant verbatim — *"host keeps two machines off each other's lines entirely"*. The file is tracked and `.gitattributes:10` declares it `merge=union`, so a union merge folds two machines' lines into one stream with nothing marking the seam: the misattribution is published rather than local, and irreversible once merged. Session attribution survives (`actor` carries a sessionId), which is the recovery route and also why the loss is specifically the HOST half.
**Blind party:** every party equally, which is what makes it different from an ordinary mistake. The information does not exist to be careless with: `git` collapses all sessions into one author string, and an untracked file carries no origin at all.
**Promotes to:** **clears both bars as of 2026-09-01 — `n=3`, spread 3** (the buddy compact banner, the shared-build red with no author, the foreign-index pre-commit guard). The target is `H` (a provenance channel is a mechanism, not a discipline), not `OB`; nothing is written yet, so *cleared* is not *promoted*. **The third member arrived by retag, not by filing** — the blind second read moved `2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` here from `IC-14`, which is the retag this entry's own promotion condition (*"instance 3 meets it"*) was waiting on. *(This field read `not yet` — `n=2` for hours after that retag tripped it, standing beside an Index row that already read `n=3`, while `IC-14`'s field still claimed the same file as its 8th member: one file, two fields, both reasoning from before the move. The Index cells are gated by `every_index_count_matches_the_corpus`; these fields are not, which is the whole distance between the two.)*
**Mechanism status:** `shipped (partial)` — **checked against the code 2026-09-03, and the previous `none yet` was wrong.** The committed half of the table below is built and running: `scripts/prepare-commit-msg-session-id.sh` writes a `Session-Id` trailer, the shim is installed at `.git/hooks/prepare-commit-msg` (opt-in, `scripts/install-hooks.sh --with-session-id`, wired at `scripts/install-hooks.sh:209`), and all 20 commits at the time of checking carried a trailer resolving to three distinct sessions. **The uncommitted half still has none**, which is what keeps this `partial` rather than `shipped`.

**How the row stayed wrong is the part worth keeping.** `scripts/prepare-commit-msg-session-id.sh:15-16` reads *"That is docs/trackers/issue-clusters.md IC-10 (`authorship-unrecoverable-after-the-fact`), which **until this hook** had `Mechanism status: none yet`."* The author cited the class from the code, wrote down that this field would need updating, and did not update it — so the ledger went on advertising an absence its own mechanism had already closed. The citation runs code→ledger and nothing runs the other way, which is why care did not catch it and a **derivation** would have: `grep -rE 'IC-10|authorship-unrecoverable' src/ tests/ scripts/` returns the hook in one call. Same shape the Index solved for `n` by deleting the stored column.
**Valid:** dated 2026-08-31

**Split from `IC-1` deliberately, on the remedy test.** `IC-1` claims a write reaches further than the set of peers you can see, and its remedy is an ownership protocol over the shared resource. This class claims something narrower and later: once the write has happened, *who did it* is not recoverable. Its remedy is a provenance channel. Same substrate, different missing thing — which is the same test that keeps `IC-1` and `IC-2` apart despite both reducing to "a component reasoning about a scope it cannot observe". `buddy-compact-banner-names-a-peers-session-as-your-own` was filed under `IC-1` and is moved here: its defect is that `from=<sid>` names another live session as your own predecessor, which is misattribution, not blast radius.

**Seed evidence — an exchange between two sessions produced three misattributions, all while reasoning about this class.** Sessions `codescout-kat` and `codescout-23`, 2026-08-31, both actively working the `IC` ledger:

1. `codescout-kat` told `codescout-23` "your nested-hook-state bug reasons that session 3a6d634e… wrote `.buddy/`". The reasoning is in that file, but the file is not `codescout-23`'s.
2. `codescout-kat` warned `codescout-23` that "your untracked librarian-runtime bug file" would red the cluster gate. Also not theirs.
3. `codescout-23`, correcting the above, argued the file was `codescout-kat`'s because *"your own `2ed2e716` calls it 'my nested-hook-state bug'"*. `codescout-kat` authored exactly two commits that session, `351836a8` and `522675a6`; `2ed2e716` is neither.

The file belongs to a third session neither had enumerated. `git log` shows why the dispute was unresolvable from inside it: `2ed2e716`, `e14b230e`, `351836a8` and `522675a6` all read the same author and email, because git has no session dimension — the field is a constant and carries zero information. The one channel that did work was accidental: `.buddy/by-ppid/<pid>/session_id` on disk, which exists for unrelated reasons and is untracked.

Note the pattern is `OB-1`'s — *"the author, specifically"*. All three attributions were made by parties who had just read the evidence, in messages *about* attribution failure. Knowing the class prevented none of them, which is the standing argument against answering this kind of defect with care rather than mechanism.

**Falsified by** an attribution dispute on a shared checkout that a party could settle from committed state alone.

**The instrument exists, and it splits by state — established 2026-09-01, three misattributions of ONE file in one evening.** A fourth, fifth and sixth instance of the seed pattern above, and the sharpest yet because the parties were mid-argument *about this class* and citing F-80 by name:

| state | instrument |
|---|---|
| committed | **`Session-Id` commit trailer.** Positive, exact, one `git log`, reaches sessions no socket enumerates including exited ones. |
| **uncommitted** | **none exists.** Directory adjacency, `ListAgents`, `git status`, dirty-file lists and conversational proximity are all elimination in disguise. Ask the session; until it answers the supportable claim is **"not mine"**, never "yours". |

The sequence: a session running the gate hit a red build from a peer's un-wired function, established "not mine" correctly by `git status` + `git grep HEAD`, then asserted an owner from conversational proximity — wrong. The correcting peer named a different session from *directory* adjacency — also wrong, and filed against themselves as `bug-fix-session-log:F-89` for committing the corrected error inside the correction. The true owner was settled only once the work was **committed** and its trailer existed, then volunteered by that session unprompted.

**What this adds to the class: the trailer is a real positive instrument, and reaching for it on uncommitted state is an instrument swap that reads as rigour.** The sentence "the trailer is the positive instrument" was true and did not apply to the object in front of either party. The remedy the evening actually supports is the terminal state, not a better inference — stop at "not mine".

**A TOOL commits this class too, which is the strongest evidence the ledger has for it.** The three misattributions above were made by agents, so "be more careful" remains an available (wrong) reading. `pre-commit` removes it. Its post-hook check is an unconditional whole-tree diff — `files_modified = diff_before != diff_after`, `pre_commit/commands/run.py:203-206`, no per-hook opt-out in 4.6.2 — and it reports any difference as *"files were modified by this hook"*. The tool has no way to ask who wrote them, so it attributes by **proximity in time**, exactly as the agents attributed by proximity in the working tree.

Measured 2026-09-01: it refused a push on a **green** `cargo test --workspace` run, naming `docs/trackers/claim-decay.md` — a file nothing under `src/` writes (`claim-decay` appears there three times, all citations in comments) and which a peer session was editing at that moment. The false-failure window is the hook's runtime: sub-second for the commit-stage checks, 30-80s for the workspace test. The push stage was withdrawn for this reason (`5fbc65fb`), and the reasoning is inline in `.pre-commit-config.yaml` so nobody re-adds one without meeting it.

That this arrived *inside infrastructure built by the session that opened the class, four hours after opening it*, is the `OB-1` signature again — and it is why the remedy field reads `H` (a provenance channel) rather than any amount of care.

**Adjacent, and deliberately NOT counted as a member: the verification reflex is trained on the
technical domain and does not fire on the social one.** Named by `codescout-e8` about itself on
2026-09-01, after three wrong assertions to a peer in one evening — that peer reachability was
partitioned (it was comparing a send-by-address against a send-by-name, two different
operations); that five newly-opened classes had "landed with instances" (all five read `n=0
tagged`, and the commit subject says so); and a near-miss on routing a pattern to `IC-8` that
was caught only because it happened to read the admission test first. Its own diagnosis: *"I
have been rigorous about every claim concerning the corpus and casual about every claim
concerning the collaboration. Code claims get a probe; claims about who did what and why get an
inference."*

**It generalises to at least two sessions.** The receiving session made the same move earlier
the same evening: told that `IC-9`'s count had gone from 1 to 3, it asserted a mechanism — that
a recursive grep over `docs/issues/` had counted slug strings inside untracked session logs —
and published it before checking. The real cause was two mis-tagged archive files, a decision by
another party. Both errors are the same shape: **a mechanism inferred for another party's action
and asserted at the confidence reserved for measured facts**, while every claim either session
made about the corpus that evening was probed first.

**Kept out of the member count on purpose, and the reason is the same test that governs this
ledger.** `IC-10`'s claim is that the information *does not exist* to be careless with — git
collapses sessions into one author string. This is the opposite: in all four cases the answer
**was** available and cheap (a Members line, a tag history, two tool signatures), and nobody
looked. It also fails `OB`'s admission test outright, since a more careful version of the same
party would have caught it, which is precisely what disqualifies an `OB` row. Folding it in
would inflate `IC-10` on a family resemblance — the error `IC-9` was corrected for four hours
earlier. Recorded here because this is where a reader of `IC-10` will look for it, and because
the standing remedy is cheap and already written down: *verify before contradicting a peer* —
which holds equally for **agreeing** with one, the direction three of these four ran.
