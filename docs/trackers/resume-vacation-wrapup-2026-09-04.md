---
id: '770d75c364a466d5'
kind: tracker
status: draft
title: Resume queue — vacation wrap-up roster, 2026-09-04 (VW-N)
owners:
- marius
tags:
- resume-queue
- handoff
- cross-machine
- session-roster
- vacation
topic: cross-machine handoff and session wrap-up
time_scope: '2026-09-04'
---

# Resume queue — vacation wrap-up roster, 2026-09-04 (VW-N)

**Read this first on the laptop.** It is the only artifact that spans every repo, and it is
deliberately prose in git rather than catalog state, because the catalog does not travel.

## How the population was derived, and what the number is worth

**17 live sessions (16 peers plus the coordinator) across 4 profiles and 5 working directories,
by socket enumeration at 2026-09-04 05:38:40 UTC.** Derived with `scripts/peer-sessions.sh`,
which walks `/run/user/1000/cc-socks/*.sock` — **not** `ListAgents`, which is scoped to one
profile's registry and reported 11 of the 17. Ten stale sockets (no process) were excluded.

**A peer count is valid only at its instant**, which is why the timestamp is in the sentence
rather than the metadata. Sessions start and exit continuously; two honest counts hours apart
share almost none of the same PIDs. Do not reconcile this roster against a later enumeration and
read the difference as an error.

**PIDs are dead on arrival at the laptop.** They identify processes on the workstation and
nothing on any other machine. The durable identifiers here are the **sessionId** (given, since
the harness makes it a path component of the session's scratchpad) and the **commit SHA +
`Session-Id:` trailer** (verifiable by a third party who was not present and cannot ask). Names
are registry-minted and were measured on 2026-09-02 returning **two different values from two
live reads of one file 2.5 minutes apart** — do not attribute by name.

## FIRST THING ON THE LAPTOP — before reading anything else

The catalog (`~/.local/share/librarian/catalog.db`) is machine-local and gitignored. A fresh
clone arrives missing three layers, and **each is silent in a different way**: `doc(get)` returns
`augmentation: null` without comment, and a missing `cites` edge is indistinguishable from an
artifact that cites nothing. Nothing fails — you quietly get less.

Run [`docs/conventions/cross-machine-catalog-resume.md`](../conventions/cross-machine-catalog-resume.md)
in full. Measured on a previous 437-commit pull: **21 of 23 memories invisible to `recall`, 697 of
1117 `cites` edges absent, 22 trackers' augmentations gone** — including trackers this repo's
`CLAUDE.md` gives append and query recipes for.

**And check `librarian(action="doctor")`'s `missing_file` after the pull specifically.** An
artifact archived on the workstation arrives as a rename, but `id = sha256(abs_path)`, so the
laptop's catalog keeps the pre-move row. That page's own triage calls `missing_file` mostly
other-repo noise — true of the majority and wrong about you here, because this wrap-up archived
files. `reindex` is the repair.

## THE BLOCKER — unpushed work does not reach a laptop

**Every repo in this wrap-up has local-only commits.** A resume tracker written, committed and
never pushed is invisible from anywhere else, and this is the failure the whole exercise exists to
prevent. Measured 2026-09-04 05:40 UTC:

| repo | branch | unpushed | pushing is |
|---|---|---|---|
| MRV-poc | `dev` | **CLEARED** — sweep run, then **85** commits pushed `8448f4c2..90eb00b8`, ahead 0 | done |
| codescout | `master` | **139** | routine |
| mirela/backend-kotlin | `master` | **13** | routine |
| codescout | `experiments` | 1 | routine |
| prompt-engineering / claude-plugins | `master` / `main` | 1 each | routine |

**The two MRV-poc readings disagree (58 vs 68) and both are recorded rather than reconciled.**
They come from different sessions at different instants on a branch other sessions were actively
committing to, so the difference is expected churn, not an instrument fault. Re-derive at the
moment of pushing; do not treat either number as the count.

**MRV-poc carries a second gate that is not about git at all.** Its deploy is blocked on a
once-off legacy-OCR-memo sweep (`docs/ops/erasure.md`) *without which an erasure request returns
200 while the customer's text remains*. That is a correctness gate on customer data and it is
independent of the push decision — pushing without it would make a silent data-retention bug
reachable in production. Reported by `8f9907a3-fde3-415d-8eec-2f7cebd6978e`.

**A knock-on nobody would predict:** in MRV-poc, `append_entry` *correctly* refuses while the
ledger's own commits are unpushed, so four tracker filings are written out in a handoff document
but cannot be filed. The unpushed state is not only a transport problem; it has already stopped
work.

### MRV-poc resolved: sweep first — and the broadcast answer was not the final one

A coordinator broadcast asked Marius "what should be pushed?" and the answer was **push
everything, including MRV-poc**. The session actually holding that work put the same question to
him directly, carrying two facts it had verified first, and he chose **sweep first, then push**.

Both answers were his. The second is the one that governs, and the difference is not that he
changed his mind — it is that **the second question was askable only by the party holding the
domain knowledge**. What that session added:

- **The erasure endpoints have no frontend caller.** The only `del()` in the data facade is
  `/api/reviewer/chat/conversations/{id}`, so erasure is API-only and cannot be triggered by a
  reviewer clicking around. The realistic exposure is narrower than *"a data-retention bug
  reachable in production"* implies — which is how the coordinator had put it, without knowing.
- **The real gate is not the deploy.** It is *before the first erasure REQUEST against a
  pre-relayout document*. Their own runbook states the stronger *"at the deploy that first ships
  C1"*, deliberately left stronger because that is the form which survives being skim-read.

He chose the safe sequencing anyway: the sweep is two commands against
`sp-mrv-chat-readiness-dev` and removes the residual entirely, against carrying a
known-incomplete erasure through a vacation with nobody available to run it.

**The transferable rule: a relayed decision is information, and the party holding the work is the
only one who can ask the question properly.** The coordinator relayed rather than instructed,
explicitly, and stated its own dissent once and dropped it. That is what made the better question
reachable. Had the relay been framed as authorization, the broadcast answer would have executed.
**OUTCOME, and the measurement is the point.** The session verified the gate at the bytes before
asking anything: C1's endpoints and `_sweep_legacy_ocr_cache.py` are present on HEAD and **absent
on `origin/dev`**, so the push genuinely was the deploy that first ships C1 — the exact deploy
`docs/ops/erasure.md` gates. It then ran the sweep's report-only step to turn *"there may be a
problem"* into a number:

```
live entries 0 · legacy (unreachable by erasure) 97 · unrecognised 0
```

**97 of 97.** Every OCR memo entry in the deployed bucket was pre-relayout. Not a partial gap: a
customer erasure would have returned `200 {"memo_entries_evicted": 1}` while **100% of the cached
text remained**. The runbook's own safety precondition (`unrecognised` MUST be 0) was satisfied,
so applying the sweep was unambiguous. Applied, then the dry run was **re-run rather than trusting
the apply's own report** — `legacy: 0` — and only then pushed: `8448f4c2..90eb00b8`, ahead 0.

**The count was 85, not 58 or 68.** Other MRV-poc sessions committed 17 more between the flag and
the push. `behind: 0`, clean fast-forward, nothing clobbered. Three readings of one branch across
one morning — 58, 68, 85 — none wrong, all instants. **In a checkout this busy an ahead-count is a
fact about a moment, not a property of a branch**, and this roster carried two of the three as
though they bracketed a range.

**A stale line in THIS file was an active hazard, not merely wrong.** Until this edit the blocker
table read *"58–68 unpushed, gated"*, and a peer session was still relaying *"dev is at c53aa06c,
58 unpushed, do not push"* after the push had landed — an instruction to preserve a state that no
longer existed. Corrected here and directly by the session that pushed. This is the same
tracked-and-stale class the roster documents two sections down, occurring inside the roster,
within an hour of it warning about the class.
## Gitignored-but-real state — the failure `git status` cannot report

Raised by `3d806b09-79ae-482b-b362-ab526e9c189a` after the original brief missed it, and it
generalises past the catalog. MRV-poc holds a live re-authored fixture at
`tests/fixtures/golden/holdout_prose.json` — **DVC-managed and gitignored**, so every "is the tree
clean?" check answers *yes* while a real edit sits there. Disk md5 `f7cdba80…` against `.dvc`
record `0a6392f3…`; a fresh clone measures **35/42, not 36/42**. Left uncommitted deliberately: it
is an external-audience fixture at `burns_used 3/3`, so revising it is Marius's decision.

The general form, worth carrying to any future wrap-up: **an enumeration of sessions finds
sessions, and a `git status` finds tracked changes. Neither finds state the repo has been told to
ignore.** Ask for it explicitly; a bare "no" derived from `git status` is precisely the answer
that cannot see it.

**A worktree-only gitignored ledger is a third class again, and the destructor is a routine
command.** `8f9907a3-fde3-415d-8eec-2f7cebd6978e` holds an SDD ledger at
`.worktrees/readiness-erasure/.superpowers/sdd/2026-09-02-readiness-erasure/` — 912K across 43
files, gitignored at `.gitignore:49` **and** existing only inside a linked worktree. `git worktree
remove` on a merged branch destroys it with no warning and no trace, and the branch IS merged.
Decision recorded there: extract-not-commit, durable content lifted into the handoff and the
commit messages, and the handoff now says to delete the worktree *deliberately* rather than
incidentally. So the taxonomy of state a wrap-up must ask for is now three deep: **tracked and
stale**, **gitignored but real**, and **gitignored, real, and destroyed by a normal cleanup
command**.

### One roster row may be two generations of one thread — unresolved, recorded

`8f9907a3-fde3-415d-8eec-2f7cebd6978e` reports its own reload header as
`sid=8f9907a3 from=3d806b09-79ae-482b-b362-ab526e9c189a source=compact` — naming as its
*pre-compaction lineage* the very sessionId that PID 1689215 reported as its own current identity.

**What is established:** two distinct PIDs, two distinct sockets, both live at enumeration time,
both answered independently, and they reported different commits and different trackers. As
processes they are two.

**What is not established:** whether they are two independent *threads of work* or one thread's
two generations. `ListAgents` cannot resolve it and names are registry-minted, so the usual
instrument is the wrong one here.

Recorded rather than resolved because the honest claim is the disjunction. **The count in this
file is a count of SOCKETS, which is what the instrument measures** — read it as that, and do not
re-use it as a count of independent work streams without re-deriving it. Raised by the session
itself, against its own row, which is the only vantage from which it was visible at all.
**Re-enumerated 2026-09-04 06:06:33 UTC, and the proposed reconciliation is FALSIFIED.** The
hypothesis offered was that PID 1689215 could report `3d806b09` as its current identity, while
3737053's reload header names that id as its *predecessor*, only if 1689215 had outlived the
compaction. Start times rule that out: **3737053 started 11:52:35 and 1689215 started 14:59:47** —
the process naming `3d806b09` as its predecessor is the OLDER one by three hours. A later process
cannot be an earlier one's ancestor.

What fits the evidence instead: **a sessionId can be resumed into a NEW process after it has
already been compacted into a successor.** `3d806b09` compacted into `8f9907a3` at or before
11:52, and was then resumed at 14:59 into a fresh PID which reports it as current. Both run
concurrently — one holding the id, one naming it as lineage.

So the roster double-counts a **lineage**, never a process, and the correction stands for a
different reason than the one proposed. Consequence for any future enumeration: a socket count is
exact, and any attempt to derive *independent work streams* from it will over-count by however
many sessions have been compacted-then-resumed. Neither `ListAgents` nor the socket walk carries
the lineage field that would resolve it; only the session's own reload header does.

**And one non-responder has EXITED rather than gone silent.** PID 4020515 (MRV-poc, `.claude`,
started 2026-09-02 23:34:49) was live in the 05:38:40 enumeration and absent from the 06:06:33
one. Its state was never collected and cannot now be. That is a materially different roster entry
from "did not reply", and it is exactly why `*awaiting*` is defined in this file as *state never
collected* rather than *confirmed clean*.
### A session's REACH is not its cwd — the boundary no enumeration draws

Found at the close of the sweep, by a correction that was itself an inference. `4d30fbb4-f68c-4ae1-b38d-0e037ea28efc`
observed backend-kotlin's 15 commits arrive at origin at 08:57:08 without having pushed them, and
attributed it to *"another session in this checkout"* — eliminating over the four sessions whose
**cwd** is backend-kotlin, all of which it held subscriptions on and all of which reported clear.

The elimination was sound over its population and the population had the wrong boundary. The push
was made by the coordinator session (`f2ab55f8-e3ac-4e0a-9eef-3b92d77bac20`), whose cwd is
**codescout** and which reaches backend-kotlin only because `/home/marius/work/mirela` is an
**additional working directory** on it. Confirmed by matching the push's own output
(`356afc7c3..ba967b313`) against `git reflog show origin/master`.

**Every instrument used in this sweep reports where a session IS. What mattered was where it can
WRITE.** `scripts/peer-sessions.sh` prints exactly one cwd per session; the additional-working-directories
list is not enumerated anywhere. This is the profile-scoped `ListAgents` blind spot one level out —
not *the list is short*, but *the list is answering a different question than the one being asked
of it*.

**A count correction owed in the same breath, and recorded rather than quietly fixed.** This
roster first recorded backend-kotlin as **13** unpushed; the session read **15**; the push moved
**15**. Both readings were correct at their instants — the roster's at 05:40 UTC, before
`1a3e6fbc6` and `ba967b313` landed. The row carried the value without its timestamp, which is the
failure this repo has a standing rule against, committed inside the artifact warning about it.

**The shared-index race, now measured five times in one morning across two repos.** Three in the
codescout tree and two in backend-kotlin within the same minute — one session with a peer's file
one second from its commit, another with that session's file appearing between its `git add` and
its verification call. Every one committed by pathspec; none swept another. The window is roughly
one second wide and the mitigation works.
## Roster

Status vocabulary, as briefed to every session: **stop** = nothing outstanding;
**finalize** = a little left, being finished now; **resume-tracker** = mid-task, state written to
a committed tracker.

**12 of 16 replied.** PIDs are workstation-local and dead on arrival elsewhere; the sessionId is
the durable identifier.

| repo | status | sessionId | tracker / notes |
|---|---|---|---|
| codescout | *coordinator* | `f2ab55f8-e3ac-4e0a-9eef-3b92d77bac20` | this file; `1004633e`, `5862ee36` |
| codescout | **stop** | `d2bc134a-4e6b-470f-b742-1abd5b278279` | 11 commits; open items are Resume prose in three committed bug files |
| codescout | **stop** | `ffb95976-dc89-4cca-87aa-c026544faf2f` | `d5897c73`, `edc0087f`, `24c55642`; state in `retrieval-benchmark.md` § *2026-09-04 (dawn)* |
| codescout | **stop** | `08a2785b-94a2-463d-b70a-60a62fb4f8f6` | 4 commits; `d73ee203` carries its own Resume |
| codescout | **stop** | `66523284-814b-49b8-b8f8-820dc2b00be2` | 11 commits; stranded-vector class closed on all three paths, verified live at `f008e74f` |
| claude-plugins + prompt-engineering + codescout | **stop** | `78f179a8-3cf7-4ab1-bbca-266d93e1efe4` | `prompt-engineering/docs/trackers/eval-runbook.md`; `17c3a07` + `583bd9d` **pushed** after confirming directly with Marius; `6ac368a5` went out with the codescout push — all three repos zero ahead |
| mirela/backend-kotlin | **stop** | `f2f73e7a-0f3f-44d5-aa08-dab4742c4fb7` | `tracker-hygiene-log.md`; next sweep due 2026-10-02 |
| mirela/backend-kotlin | **stop** | `4d30fbb4-f68c-4ae1-b38d-0e037ea28efc` | 9 commits; **`innovaplan-live-contract.md` (IPC-N, 14 entries) is the authority for ITS/Innovaplan** |
| mirela/backend-kotlin | **stop** | `c3f2ceb3-f8ca-47a0-8fbc-c5a00982be3f` | read-only session, no edits; codescout MCP was disconnected there |
| mirela/backend-kotlin | **finalize** | `add4873b-9b69-4011-9ca1-bbc6fff9d33f` | `its-integration-index.md` + `innovaplan-live-contract.md`; 5 commits |
| MRV-poc | **resume-tracker** | `3d806b09-79ae-482b-b362-ab526e9c189a` | `docs/trackers/ingest-roadmap.md` § *Resume — Phase 3 gold re-authoring* |
| MRV-poc | **finalize** | `8f9907a3-fde3-415d-8eec-2f7cebd6978e` | `docs/handoffs/2026-09-04-readiness-erasure-merged-not-deployed.md` |
| MRV-poc × 4 | *awaiting* | — | two started 2026-08-31 and are 4 days idle |

**`*awaiting*` means the reply had not arrived when this file was committed, not that the session
is unresponsive.** A row still reading `*awaiting*` is a session whose state was never collected —
check that repo's own `git status` and trackers rather than assuming it was clean.

### A stale tracked file is worse than a missing one

Two sessions independently reported that their real debt was not uncommitted code but **prose
already in git that had stopped being true**, which no `git status` and no enumeration can find:

- `docs/trackers/its-integration-index.md` — the file a laptop session opens first for ITS — led
  with a block written the day *before* the 27 August Kedos call, in the future tense, warning
  about a defect since fixed. Corrected with a dated block above it and the superseded section
  marked in place.
- `docs/issues/2026-08-26-innovaplan-read-responses-are-enveloped.md` asserted three things that
  all stopped being true on 27 August — no API key, read side never live, `GetStrutturaCorso`
  blocked on agenda item D32. A hygiene sweep on 2 September did not catch it.

The general form: **the wrap-up question is not only "is anything uncommitted?" but "is anything
committed and now false?"** The second has no mechanical detector and both instances were found
only because someone re-read their own work.

### Running servers execute deleted binaries — and the obvious check lies

Reported by `66523284-814b-49b8-b8f8-820dc2b00be2`: at 03:07, **11 of 14 running codescout
servers were executing `(deleted)` inodes.** `cargo rb` replaces the binary; every already-running
process keeps the old image until `/mcp` restarts it. A session that archives an artifact before
reconnecting still strands its vectors — **and will confirm the fix is present by reading the
source**, which is the wrong instrument. The tell is one command: `readlink /proc/<pid>/exe`.
### The general form: committed is not in effect, and the author is the worst-placed observer

Sharpened by `78f179a8-3cf7-4ab1-bbca-266d93e1efe4`, whose framing is better than the two
instances it generalises.

Two failures were found tonight from unrelated directions. A `SKILL.md` edit is committed and
pushed and still reaches **no profile**, because skills load from `installPath` and nothing moves
until `release.sh` seeds a cache and repoints three install records. And 11 of 14 running
codescout servers were executing `(deleted)` inodes, so a fix present in the source was absent
from every running process.

The shared mechanism is not merely *the artifact in git is not the artifact in effect*. It is that
**the editing session is the observer most certain the change shipped, and the one structurally
least able to check** — it can see the diff and cannot see `installPath`; a process cannot see its
own `(deleted)` inode. Both are invisible from the inside and cheap from outside.

So the remedy in both cases is a **third-party probe, never a re-read**: served bytes against
`installPath`; `ls -l /proc/<pid>/exe`. Reading the source again is the instrument that returns
the wrong answer with the most confidence, and it is the one the author will reach for. That is
an `OB` admission test passed cleanly, from two subsystems at once.

**Concretely for whoever resumes:** the pushed `SKILL.md` sentence is `committed`, not `in
effect`. `release.sh` was deliberately not run at wrap-up — it runs the full suite and cold-restarts
three instances.
## Open items in codescout, none blocking

Carried from the session that coordinated this wrap-up. All still true, all cheap to file later;
none is a half-finished change sitting in the tree.

- **`owner:` vs `owners:` in bug frontmatter** — 495 files use the scalar, 109 the list, 68
  neither. The catalog indexes `owners`, so an owners-filter query returns `count: 0` for a file
  it demonstrably holds (verified at the tool surface on
  `2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`). `docs/issues/_TEMPLATE.md`
  prescribes the invisible spelling. Widest reach of anything on this list.
- **An `IC-14` 17th member** — the `ledger-counts` guard's stated subject is the `**Members:**`
  FIELD; its parsed extent is that field's first LINE (`scripts/pre-commit-ledger-counts.py:376`).
  Ruled a genuine member on the remedy test rather than a duplicate of the documented one-line
  constraint: the constraint's fix is a coupling note for a future parser refactor, this one's is
  a disclosure edit to the hook name and refusal text — different files, different fixes. The
  framing that makes it worth filing is that `:568` already says *"the `**Members:**` line"*, so
  the disclosure exists as one word that **reads as a synonym for the thing it distinguishes
  from**. Three sessions satisfied this gate without knowing what it checks. Credit
  `ffb95976-dc89-4cca-87aa-c026544faf2f` for the chunker citation and
  `d2bc134a-4e6b-470f-b742-1abd5b278279` for the amend.
- **Stray `.claude/` hook-state directories** — `docs/trackers/issue-clusters/.claude/` and
  `docs/superpowers/plans/.claude/`, a hook resolving state relative to the edited file's
  directory instead of the project root. Verified harmless to both ledger readers
  (`class_files()` globs `*.md` non-recursively; git plumbing otherwise), but it is the same
  pollution the cluster Index preamble already warns about for `docs/issues/`.
- **One sentence owed to `docs/issues/_TEMPLATE.md`'s Resume section** — a bare `:NNN` citation
  inherits the previously named file and has no basename for `audit_doc_refs` to fall back on, so
  nothing lints it and nothing can. Recorded as an uncovered case of an existing rule, `n=1`, not
  a new class.
- **Status and severity vocabulary drift** — `status: done` and `status: archived` sit outside
  the documented set and are invisible to both the open-query and any "what got fixed" reading;
  `severity: med` ×16 against `medium` ×272, absent on 102. Real, nearly cosmetic.

## What in this roster is VERIFIED, and what is a self-report

Raised by `66523284-814b-49b8-b8f8-820dc2b00be2` against this file's method, and it is correct:
**a roster is a collection of instrument reports, and this one asked every session for its claim
and none of them for a control.** That session measured, the same evening, **eight instrument
reports that returned a plausible WRONG value rather than erroring** — 0 of 8 caught by care or by
knowing the class, 8 of 8 caught by a cheap artifact in the output that a broken run cannot
fabricate. **Four of the eight had a summary line identical to a healthy run's.** Two happened to
parties who had just been taught that exact class, one mid-writeup of the previous instance.

So the honest split, rather than a uniform-looking table:

**Independently verified by the coordinator, at the bytes:**
- Every push range in the blocker table — run here, output read, `rev-list --count` confirmed 0.
- The backend-kotlin push attribution — push output matched against `git reflog show origin/master`.
- PID 4020515's exit — present at 05:38:40, absent at 06:06:33, two enumerations.
- The lineage start times that falsified the compaction hypothesis.
- Fast-forward safety on all three pushes, before pushing.

**Taken on the reporting session's word, and correctly so — they hold the context:**
- Every `UNCOMMITTED: no`.
- Every commit list.
- The DVC md5 divergence and the 35/42-vs-36/42 consequence.
- The `97 of 97` sweep reading, **including its `legacy: 0` confirmation** — see the correction
  immediately below, which retracts this file's earlier claim that it arrived with a control.

**A number that moved between two reports, which is the tell this section exists for:** running
codescout servers on `(deleted)` inodes read **11 of 14** at 03:07 and **8 of 14** at 06:06. Both
from the same instrument, neither wrong. A roster that quoted either as *the* figure would have
been publishing an instant as a property — the same error as the 58/68/85 ahead-counts, in a
different measurement.

**And the caveat that reaches every other number here:** a session running a REPLACED binary
reports evidence about the build it LOADED, not the tree on disk. Start time, cwd and a correct
`~/.cargo/bin` symlink all read healthy while this is true, so nothing else surfaces it. **8 of 14
live sessions were in that state at 06:06.** Any figure in this roster sourced from one of them
inherits that caveat.

The transferable instruction, and it is cheap: **ask for the control alongside the claim.** An
absurd mutation that must die, a control row that must stay green, a `truncated: true` flag, a
per-row label instead of a tally. Recorded as `bug-fix-session-log:W-106` (`f543cc4a`).

### RETRACTION: the exemplar this section held up did not have a control

This file previously credited `3d806b09-79ae-482b-b362-ab526e9c189a`'s `97 → 0` as the one figure
of the night that arrived with its control. **That was wrong, and the session corrected it against
itself after the line was committed and pushed.**

What was actually sent: apply the sweep, then re-run *the same script's* dry run and read
`legacy: 0`. Described as independent because it re-ran rather than trusting the apply's own
report. **It is not independent — it is W-106's case 4.** Same script, same code path, same
credentials, same bucket argument. A sweep pointed at the wrong bucket, or one whose list call
returned empty on a transient fault, prints:

```
live 0 · legacy 0 · unrecognised 0    ->  "Nothing to sweep."
```

Byte-identical to a healthy post-sweep run. The re-run cannot distinguish *the cache is empty*
from *I cannot see the cache*. At the moment that contract was published, 97 production objects
had been deleted and verified by an instrument that would have said the same thing had it been
broken.

**The real control, added an hour later and only because a third peer relayed W-106:** list the
bucket with a **different tool** (`gcloud storage ls`) and check two things at once — the OTHER
prefixes must still enumerate, and `_ocr_cache/**` must be empty. **12 workspace prefixes
returned; `_ocr_cache/**` matched no objects.** A broken lister returns nothing for both, so that
pair discriminates and the re-run does not. The conclusion held. It was not established when it
was published.

**So the discriminator was never "re-run it."** It is *use a different instrument, and include a
row that must stay green.* The re-run had neither half.

**And the count is 0 of 9, not 0 of 8.** That session had the class in front of it, was actively
applying it, and shipped the weaker check first — then, in the very command written to test this
roster's own point 1, ended with an unconditional `echo "(no STALE lines above ...)"` that printed
directly beneath the STALE line it had just found. Two instances in one session, both after
reading the warning. Which is the W-106 result restated rather than contradicted: **knowing the
class prevents none of them; a cheap artifact in the output catches all of them.**

The reason this retraction is in the file rather than a quiet edit: the line was **load-bearing**.
It was the exemplar separating *checked at the bytes* from *taken on a session's word*, and as
sent it belonged in the second column. A reader trusting this section would have learned the wrong
lesson from the one case it held up.
## Notes for whoever picks this up

**The instruments that answer "what changed?" are already written** — do not re-derive them.
`scripts/probe-cluster-census.py` for per-class membership, `scripts/probe-caveat-density.py` for
declared-uncertainty density, `scripts/peer-sessions.sh` for the session population.
[`docs/PROBES.md`](../PROBES.md) indexes all of them and names each one's blind spot.

**On the caveat-density probe specifically: read the top of its table, never the bottom.** It
counts *declared* uncertainty on a voluntary field, so a high rate is evidence of doubt and a low
rate is not evidence of confidence.
