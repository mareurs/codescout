---
id: '04e069162e1fb15f'
kind: bug
status: mitigated
title: 'BUG: staging a path by name records the stager as its owner, so a peer''s file you stage is yours on the record and foreign-index has nothing to refuse'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- shared-checkout
- git-hooks
- multi-session
topic: the layer-2 owner field and the relation it actually records
---

## Summary

`.git/session-stage-log` is `IC-17`'s layer-2 mechanism — the owner field the ADR names as the
model to copy. It records **who staged a (blob, path) pair**, and
`scripts/pre-commit-foreign-index.sh` refuses a bare commit carrying pairs whose recorded session
is not yours.

**So a session that runs `git add -A` becomes the recorded owner of every path it swept**,
including untracked files another session is mid-write on. The guard then correctly finds nothing
foreign and passes. The capture proceeds under the sweeper's own `Session-Id` trailer, and every
instrument reports it as theirs — because by the mechanism's own definition, it is.

This is not the guard failing. It is the owner field answering *"who staged this?"* when the
decision needs *"whose content is this?"*.

## Symptom (Effect)

Nearly fired 2026-09-07, on this checkout, and was avoided by removing the files rather than by
any guard:

Four sessions were each writing a handover file into an untracked `handover/` directory. The
directory was not in `.gitignore`, so every file read as `??`. Any session running `git add -A`
followed by a **pathspec** commit of its own paths would have staged all four under its own sid —
and a later bare commit by that same session would have committed them, refused by nothing.

Spotted by `8dba66b0`; resolved by `cda3afe5` moving the directory out of the repo entirely
(`/home/marius/codescout-handover-2026-09-07/`, verified `diff -r` identical, nothing ever
staged). **The remedy was removing the resource, not fixing the guard** — which is the right call
under time pressure and leaves the mechanism unchanged.

## Root cause

Verified at the bytes 2026-09-07:

- `scripts/post-index-change-stage-log.sh` attributes a pair to the session whose hook fires,
  from `CLAUDE_CODE_SESSION_ID`. That is *the actor*, by design — the header documents a
  superseded rule where a batch was "claimed by whoever ran `git status` next", and the fix was
  to bind attribution to the staging session. Correct for its purpose.
- It rebuilds from `git diff --cached --raw`, which enumerates **newly-added untracked files**
  identically to modifications. Nothing distinguishes "I wrote this" from "I swept this".
- `scripts/pre-commit-foreign-index.sh` partitions into `mine[]` / `theirs[]` and refuses on
  `theirs`. A path you staged is in `mine`.

The three compose exactly: **stage it and it is yours, by the only definition the system has.**

### Why this is `IC-17` and not a guard bug

`docs/issues/2026-09-06-a-withheld-commit-is-indistinguishable-from-an-unpushed-one.md` already
extended this class once, in the same direction: git records an **author** and the decision needs
an **authorisation**, so *"add an owner field"* is not a sufficient statement of the remedy — the
field has to be the right relation.

This is the second instance of that extension, and the sharper one, because here the field was
**added deliberately by this project** for exactly this hazard and still records the wrong
relation. `IC-17`'s claim says a shared resource records *what* changed and never *who*. The
refinement both members force: **recording a `who` is not sufficient if it is the wrong `who`.**

## Evidence

The near-miss above is instance 1 and was not reached. What makes the mechanism claim solid is
that it is read from the scripts rather than from the incident — the three bullets in § *Root
cause* are each a line of shipped code, and no execution is needed to see that `mine` cannot
contain a foreign path.

**Not measured:** how often `git add -A` is actually used here. `CLAUDE.md` and the commit
sequence both prescribe explicit pathspecs, so the population may be small — but the handover
episode is a case where four sessions created untracked files simultaneously, which is exactly
when a sweep is tempting.

## Instance 2 — 2026-09-16, and it falsifies the remedy this file's own title implies

The title names `git add -A`, so a reader takes the **narrow** form as the safe one. It is not.

`e5691fad-9f78-4cd1-ad14-edfdd1fee41f` ran `git add docs/trackers/bug-fix-session-log.md` — one
explicit path, no `-A`, no directory pathspec, the narrowest form
`docs/conventions/shared-checkout-commit-sequence.md` prescribes — and staged `F-168` and
`F-170`, neither theirs, alongside their own `F-169` / `F-171`. Both guards passed, verified at
the predicates rather than from the incident: `scripts/pre-commit-unreviewed-content.sh:29`
compares staged against working tree and a full stage makes them identical;
`scripts/pre-commit-foreign-index.sh:152` reads `$git_dir/session-stage-log` and `:207` puts the
path in `mine[]`, because the stage log records the **stager** and that was them.

**Why this is worse than the `-A` case rather than a milder version of it.** Under `-A` the sweep
crosses files the sweeper has no business in, and the remedy *"name your paths"* reaches it. Here
the file **is** theirs to append to. Both ownership signals the system has — *did I write in this
file* and *did I stage this blob* — correctly answer **yes**, and the entries are still not
theirs. The per-file signal is **true and useless**, so there is no narrower `git add` available
and no rule of that shape that helps.

**The unit of ownership is the ENTRY; every instrument in the chain is path-grained.** That is
this record's claim holding one level below where it was written. `c054113b` resolved the
instance the only way available — four sessions' entries in one commit with authorship stated in
the message — after `29420e72` observed that no ordering lets each session commit its own, since
all four sections were already in the file: *"you first" and "me first" both capture, and everyone
waiting is a deadlock.*

Reported by `e5691fad` as a **near miss** — they unstaged by pathspec and nothing was lost, which
is the only reason this is evidence rather than an incident. Recorded here rather than filed as a
new bug: the mechanism is this file's, and a second record would have split one class across two
paths (`bug-fix-session-log:F-171`, `issue-clusters:IC-17`).

## Fix
### Re-verified 2026-09-24 — the `-A` headline does not reproduce; the named-path mode stands

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`), verifier evidence, reproduced with the real hook copied into a temp repo. **`git add -A` does not make you the recorded owner.** `CLAUDE_CODE_SESSION_ID=sessA git add -A` wrote the rows `-  351be5b my_file.txt unnamed` and `- c9f6d41 peer_file.txt unnamed` (owner `-`), and the real `pre-commit-foreign-index.sh` then refused the bare commit (exit 1, "blanket add"). That behaviour comes from `names_path` / the cold-log rule (`scripts/post-index-change-stage-log.sh:285`, `:391-399`), introduced by `fa9b3aff` on 2026-09-01, six days before this file was opened. So the title's `-A` claim was likely never true against shipped code; history was not replayed to confirm.

**The named-path mode is still open and structural:** `git add peer_file.txt` records `sessA … named`, so explicitly naming a shared file, or a peer's untracked file, still makes the stager its recorded owner. The refusal's own remedy ("re-stage by explicit path and the bare commit passes") steers toward exactly that route. Consider retitling this file to the named-path mode.

Not fixed. Three directions, cheapest first, none costed:

1. **`.gitignore` the shared-artifact directories** — removes this instance and no others. What
   was effectively done today, by moving the directory out of the tree.
2. **Refuse `git add -A` / `-u` at the hook**, or warn when a single index write claims paths
   the session has never written. The recorder cannot currently tell, which is the point — it
   would need a write-side signal, and `file-provenance.py` is the obvious source and is blind
   to `run_command` (`docs/issues/archive/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md`).
3. **Record the relation, not the actor** — a pair would carry *first observed writer* rather
   than *last stager*. This is the honest fix and the expensive one; it needs a write-side
   channel the working tree does not have, which is `IC-17`'s `NONE` row.

**Do not "fix" this by making `foreign-index` stricter.** It is behaving correctly on the data it
has; a stricter predicate over a wrong relation produces false refusals without closing this.

### Measured 2026-10-01 — what a write-side channel would cost, and the decision it leaves

**Reproduced by me, not read from the verifier.** A temp repo with the real `post-index-change-stage-log.sh` and `pre-commit-foreign-index.sh` installed: a peer's untracked `peer_file.txt` exists; `CLAUDE_CODE_SESSION_ID=sessA-0000 git add peer_file.txt` wrote the row `sessA-0000  8b04b90  peer_file.txt  named`, and the following **bare** `git commit` exited 0 and carried the peer's file. The named-path mode stands exactly as the 2026-09-24 note says.

**Direction 2 and 3's obvious source is too slow for the recorder.** `scripts/file-provenance.py --help` alone exceeded 30 s and timed out when I ran it (measured once, cause not investigated), and the companion hook's own advisory quotes about 7 s per file. Either is far over what a post-index hook can pay, because that hook fires on every index change.

**A cheaper write-side record already exists.** `usage.db`'s `tool_calls` carries `cc_session_id`, `tool_name`, `input_json`, `called_at` and `effect_class` for every MCP call. An id-bounded lookup of the last writers of `docs/trackers/bug-fix-session-log.md` (`WHERE id > max(id)-30000 AND tool_name IN ('edit_file','edit_code','create_file','doc') AND input_json LIKE '%<path>%' ORDER BY id DESC LIMIT 6`) took 6 ms and returned three sessions in order. So a hook could ask "did a session other than the stager write this path through a codescout tool?" at a cost it can afford.

**What that lookup cannot see, each a limit on any fix built on it:**

- A `doc(...)` write carries an artifact id in `input_json`, not a path, and trackers are written mostly that way. The three `doc` rows above matched; I did not establish that they matched on a write's path rather than on the path being mentioned in a body, so id-to-path resolution would be owed.
- A `run_command` shell write is recorded as a call, not as the files it touched.
- It is path-grained. Instance 2 above is entry-grained inside one file the stager legitimately wrote to, so a last-writer-per-file lookup cannot separate `F-168`/`F-170` from `F-169`/`F-171`. **No file-grain fix reaches instance 2.**

**The decision this leaves is the operator's, not mine to take by building:**

1. **Build the lookup into the recorder.** On the `named` route, if a codescout write to the path by another session is on record and none by the stager, record owner `-` with a new route (say `named-foreign`) so the guard refuses the bare commit. Covers instance 1's shape where the peer wrote through a tool with a path argument. Costs a change to the recorder, the guard's route handling, its remedy text and both suites, and it ships a guard narrower than its name unless the three limits above are stated at the refusal.
2. **Accept the residual**, retitle (done in this update), and leave the entry-grain case to `docs/conventions/shared-checkout-commit-sequence.md`'s "state authorship in the message" resolution, which is what `c054113b` did.

**Decision taken 2026-10-01: option 1, built.** See § *Shipped* below.

### Shipped 2026-10-01 — option 1, and what a live check showed

`scripts/post-index-change-stage-log.sh` gained `foreign_writer` (`3dea3b88`). On the `named` route only, it asks `usage.db` whether a session other than the stager has a live write to the path through a codescout tool and the stager has none. If so, the row's owner is **that session** and its route is `named-foreign`. `scripts/pre-commit-foreign-index.sh` needed no new decision, since an owner that is not you is already refused with the live/not-live lookup; it gained one paragraph saying the attribution is by write, not by staging. It fails open: any failure to read the record returns the old `named` claim.

**Correction, 2026-10-01, after checking the first version against live data.** The first live check reported that `docs/trackers/bug-fix-session-log.md` stayed `named` and blamed writes "the record does not see" (shell or CLI). **That explanation was wrong.** The call log held the peer's `doc(append_entry)` rows for that file throughout (session `2e3f6b65`, 11:53 UTC, the entries `F-189` and `W-150`). The lookup missed them because of its **time bound**: it counted writes since the path's last commit, but a commit takes whatever blob it was handed, and another session's commit of that file at 12:19 UTC did not contain the peer's entries, which were still uncommitted. Three defects were found and fixed in `dd7b1590`:

1. **Liveness.** A write is live until its **own writer** commits the path after it (the `Session-Id` trailer of each commit, newest per session), and nobody else's commit clears it. The lookback is three days: a write that nobody commits under its writer's own id stays live until the window ends, and a long window would name a stale session. Replayed read-only against the real log, the rule names `2e3f6b65` and not `a520c25a`, whose own commit cleared its writes.
2. **Substring matching.** The first version matched the `"path":"x"` text anywhere in a call, so an edit whose body merely quoted a path (a bug file about this lookup does) was named as that path's writer. Keys are now read as JSON (`json_extract` of the top-level `path`, `id`, `action`), guarded by `json_valid` inside a `CASE` because SQLite raises on the first malformed row it evaluates.
3. **Moves.** `doc(move)` carries the *source's* id, so an archive destination's own id appears in no call. 63 of the 194 committed paths with no recorded writer were move destinations; a move now also matches its `new_rel_path`.

**What the record still cannot see, measured.** Over the same three days, 589 committed `(commit, path)` pairs from trailer-named sessions; 395 (67%) have an earlier recorded write by a session the commit names. Of the 194 without one, the move destinations are now covered, and about 100 come from **one session that has no `tool_calls` rows at all** in that window, so it does not write through this project's codescout. **No recording inside codescout can see such a session, `run_command` windows included**; the only source that reaches it is transcript provenance (`scripts/file-provenance.py`, about 7 s a file), which is too slow for the recorder. I did not isolate how much of the rest is shell writes from sessions that do use codescout, and so did not build a `run_command` observer: its payoff is unmeasured.

**Limits, stated rather than implied:** a session outside codescout's reach (above); a `run_command` shell write or a `codescout doc` CLI write by a session that does log (unmeasured, never observed to matter); `doc create`, which carries no id; and the grain, which is the file, so two sessions' entries in one tracker stay one path. A stager who wrote a path by shell while a peer also has a live recorded write is refused, deliberately.

**Status is `mitigated`, not `fixed`:** the named-path mode reproduced on 2026-10-01 is closed for sessions that write through codescout, and the structural claim (an owner field recording the stager where the decision needs the content's author) still holds for the rest.

## Tests added

`tests/hooks-discrimination.sh`, section "named route attributes a path by WRITE", cases 1-21 (215 passed and 0 failed for the file). The real recorder and guard are driven in throwaway repos with a fixture `usage.db` named by `CODESCOUT_USAGE_DB`, so no case reads the real one. Cases carry explicit times, and commits carry `Session-Id` trailers (`commit_as`), so no case depends on two events landing in different seconds.

Cases 1, 4b, 5a, 7, 10, 15, 16-17 and 21 go red on the pre-change behaviour (22 assertions with the call site deleted); the "kept as named" cases pass on unchanged code and rest on mutation. Covered: foreign attribution and the guard's refusal and wording; a path both sessions wrote; no record; liveness (a peer's own commit clears it, a third session's commit does not, the stager's own committed write is not live, the newest of a session's commits wins, one commit naming two sessions clears both, a write past the lookback is unseen); each writing tool; each id-bearing `doc` action, a `doc` read and a move's destination; failed calls; two other writers; exact path matching (`_`, a longer path, a path quoted in another edit's text); an absolute path; an absent, unreadable, locked and partly malformed database; a blanket add; a read tool; the call-count lookback; and carry-over of the row through an unrelated stage.

**Mutation:** 38 sites, all killed except `.timeout 200`, which is tuning no case can distinguish and is annotated as such. Inert pieces deleted after surviving their mutation: `[ -r db ]`, `command -v sqlite3`, `timeout 2`, the `|| return` on the query, a session-id space trim, the `--since` bound on the trailer scan (a commit older than the lookback is older than every write it can see), and the `tool_name` test on the move clause. One test case needed repair after mutation showed it vacuous: case 20 passed under its own mutation because the valid row was printed before SQLite reached the malformed one. Not mutated: the `git rev-parse` fallback, and the `-n 300` cap on the trailer scan, which no case reaches. The suite's known flaky precondition cases appeared in several mutation runs and are the already-filed 2026-09-16 bug, not this change.

## Fix provenance

- **SHA:** `3dea3b88419765714d96ba31c7671b85f165fd71` (`experiments`)
- **patch-id:** `ba3c9f4e37fd4611e380ec5cc2e0c70dc188819a`

That pair is the first version. The follow-up that corrects its time bound, its matching and its handling of moves is SHA `dd7b1590111b02050ceedc0061d49eb4daf0bd18`, patch-id `e8311f5bdd489be999244df11358b939ff397c11` (`git show <sha> | git patch-id --stable`).

## Workarounds

Explicit pathspecs, which this repo already prescribes. `git add -A` is the hazard verb; the
sequence in `docs/conventions/shared-checkout-commit-sequence.md` § 4 already says
`git add <paths>` and this file is a reason rather than a new rule.

## References

- `scripts/post-index-change-stage-log.sh`, `scripts/pre-commit-foreign-index.sh` — the two
  halves.
- `docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md` § *Layer 2* — names
  `.git/session-stage-log` as the model to copy. Worth reading beside this: the model is sound
  and its relation is the thing to get right at the next site.
- `docs/issues/2026-09-06-a-withheld-commit-is-indistinguishable-from-an-unpushed-one.md` — the
  first instance of the same extension.
- Near-miss spotted by sessionId `8dba66b0-af4b-4cda-a333-54a0605b318e`, resolved by
  `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`. Filed by
  `4eac25ba-b181-4dac-a5a1-ec88502a5bc5`, who extended this same mechanism twice today and did
  not notice this about it.
