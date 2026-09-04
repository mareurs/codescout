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
| MRV-poc | `dev` | **58–68** (two sessions read different values — see below) | **gated** — triggers a GKE rollout; operator requires explicit confirmation |
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
| claude-plugins + prompt-engineering + codescout | **finalize** | `78f179a8-3cf7-4ab1-bbca-266d93e1efe4` | `prompt-engineering/docs/trackers/eval-runbook.md`; `17c3a07`, `6ac368a5`, `583bd9d` |
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

## Notes for whoever picks this up

**The instruments that answer "what changed?" are already written** — do not re-derive them.
`scripts/probe-cluster-census.py` for per-class membership, `scripts/probe-caveat-density.py` for
declared-uncertainty density, `scripts/peer-sessions.sh` for the session population.
[`docs/PROBES.md`](../PROBES.md) indexes all of them and names each one's blind spot.

**On the caveat-density probe specifically: read the top of its table, never the bottom.** It
counts *declared* uncertainty on a voluntary field, so a high rate is evidence of doubt and a low
rate is not evidence of confidence.
