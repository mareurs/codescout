---
id: '6e91fc0ad57756fe'
kind: spec
status: done
title: Claimable Bug State (`taken`) — Design
owners:
- marius
tags:
- bug-tracking
- status-vocabulary
- peer-sessions
- doctor
- liveness
topic: bug status vocabulary and session-backed claims
---

> **Status:** design approved in chat 2026-09-02; implementation plan not yet written.
> Successor artifact will be `docs/superpowers/plans/2026-09-02-bug-claim-liveness-impl.md`.

> **IMPLEMENTED — shipped 2026-09-02, recorded 2026-09-08.** `8f40eaad`, patch-id
> `bf101e249a2a37bcf6ab3c4dbcff7cd851b914fe`; `e801f5e1`,
> `8503f8d68cb97e6db3c6d0a1ce770c743f70b457`; `a9e37db2`,
> `4c33ec234b0f593489de0a79c61594443ba6aead`. The pair is recorded rather than the SHA alone
> because `experiments` is rebased after every ship, which orphans the SHA and leaves the
> patch-id as the only durable handle.
>
> Verified at the bytes: `scan_claim_liveness` is registered in `src/librarian/tools/doctor.rs`
> and its four checks report live (`claim_held_by_dead_session`, `claim_held_by_live_session`,
> `claim_unresolvable_here`, `claim_without_claimant`); `src/librarian/session_registry.rs`
> exists; the `taken` vocabulary reached `docs/issues/_TEMPLATE.md`.
>
> **The Status line above is six days stale and is kept only because it dates the design at
> approval time** — the successor plan it promises was written, executed and shipped.
>
> **Why a 2026-09-08 stamp on work that shipped 2026-09-02.** The implementation plan's
> § *Closing the loop* named this exact update as its first unchecked box, and the session
> holding that checklist exited before reaching it — with all three of this stream's files
> still **uncommitted**. A closing step that lives only in the worktree of the session that
> wrote it is not a step, and this one cost six days of a shipped feature reading as `draft`
> to nobody's disagreement.

## Problem

`status: investigating` carries two incompatible meanings at once:

1. *"An agent is working this right now — don't duplicate."*
2. *"Someone opened this two weeks ago, never concluded, and left."*

Nothing in the record distinguishes them, so meaning 1 is unreliable (you cannot trust
that anyone is actually there) and meaning 2 is invisible (a stalled investigation looks
identical to an active one). A reader picking work off the triage query cannot tell
whether they are about to collide with a live peer or inherit an abandoned thread.

The second failure is the motivating one: **an `investigating` bug decays silently.** No
event fires when the session that set it exits, so the record keeps asserting an activity
that stopped. That is `issue-clusters:IC-8` (*a record asserts a completed action nothing
re-checked*) holding over the bug ledger itself.

There is also no owner. Even where meaning 1 is true, the record names nobody, so
"who is on this and can I ask them?" is unanswerable from the file.

## Non-goals

- **IC class files are out of scope.** A bug has one obvious unit of work; an `IC-N` class
  does not — what gets claimed there is *the mechanism for a class*, and
  `**Mechanism status:**` is prose adjudication rather than a state machine. Prove the
  shape on bug files first, then decide. (Raised in brainstorming and deliberately
  deferred.)
- **No auto-release.** A dead claim is *reported*, never silently rewritten. Releasing is a
  judgement about whether the work stands, and the check cannot make it.
- **No cross-machine claim resolution.** Session registries are machine-local. A claim made
  on another host is reported as unresolvable-here, not as dead (see § *Three buckets*).

## The vocabulary change

`BUG_STATUSES` (`src/librarian/tools/create.rs:81-88`) grows from six to seven. The
discriminator between the new state and the old one is **liveness-backing**: whether the
record's claim of activity can be checked against something outside the record.

| status | means | backed by |
|---|---|---|
| `open` | nobody has started | — |
| **`taken`** | **a live session holds this right now** | `claimed_by` resolves to a running session |
| `investigating` | opened up, worked, **no live owner** | nothing — and that is now honest |
| `fixed` / `mitigated` / `wontfix` | terminal | fix anchor (existing `doctor` checks) |
| `zombie` | no longer observed, root cause unconfirmed | unchanged |

### The state machine

```
open ──claim──> taken ──conclude──> fixed | mitigated | wontfix
                  │  ▲
        session   │  │
        ended,    │  └──re-claim──┐
        no        │               │
        conclusion▼               │
              investigating ──────┘
```

The load-bearing edge is **`taken` → `investigating`**. When a claim goes dead, the bug
demotes to `investigating` — *not* to `open`, because work probably happened and the body
probably records it. This is what gives `investigating` a precise meaning for the first
time: it is the **residue** of an unconcluded claim. "There is prior work here; read it;
nobody is holding it."

That reframing is the point of the change. Adding `taken` is what lets `investigating`
stop lying.

## Frontmatter

```yaml
---
kind: bug
status: taken
claimed_by: 4a8fb556-240b-4ee9-9db5-ec3cc3008a17
claimed_at: 2026-09-02
---
```

**Store the sessionId and nothing else.** Not the session name, not the pid, not the
socket path. All three are derivable from the sessionId at read time, and all three decay:

- The **name** is registry-minted with `nameSource: "derived"` and is re-minted by
  compaction, resume, or a restart under another profile. `CLAUDE.md` § *Observer
  Blindness* records two misattributions from exactly this in one evening
  (`codescout-26`, `codescout-00`→`codescout-cc`), and states the rule this design
  follows: **attribute by sessionId, never by a self-reported name.**
- The **pid** is reused by the OS.
- The **socket path** is a function of the pid.

`claimed_at` is **informational only** — it records when the claim was made so a reader can
judge staleness by eye, and is deliberately *not* an input to the check. Liveness is
decided entirely by § *Liveness resolution*; a TTL on `claimed_at` was considered and
rejected in brainstorming as a proxy that misreports in both directions (a session working
a hard bug for 30h reads as expired; one that died in five minutes reads as fine for a
day).

`claimed_by` and `claimed_at` live in the artifact's `extra` map — custom frontmatter keys,
written verbatim to YAML and round-trip-safe across updates. The tradeoff, accepted:
`extra` is **not catalog-indexed**, so `find(filter={"claimed_by": …})` will not work.
`find(status="taken")` does, which yields the candidate set; resolving owners within it is
the `doctor` check's job. This avoids a catalog schema migration for a query nobody has
asked for.

### Claiming is a catalog write, not a file edit

```
artifact(action="update", id=…,
         patch={"status": "taken",
                "extra": {"claimed_by": "<sessionId>", "claimed_at": "2026-09-02"}})
```

A direct frontmatter edit does **not** reach the catalog (BL-48) — the claim would sit on
disk while every `find` reported the bug unclaimed. Releasing is the same call with
`status: "investigating"` and `extra: {"claimed_by": null, "claimed_at": null}` (a null
value deletes the key).

### A session already knows its own sessionId

No new API is needed for a session to claim. The harness makes the sessionId a path
component of the scratchpad directory:

```
/tmp/claude-<uid>/<project-slug>/<SESSION-ID>/scratchpad
```

Verified 2026-09-02: the id in this session's scratchpad path is byte-identical to the
`sessionId` field of its own registry row. So the identity is **given**, never inferred —
which is the property `CLAUDE.md` § *Observer Blindness* requires of any positive
identification.

## Liveness resolution

Given a `claimed_by` sessionId, resolve it against the session registries.

**Discovery, not hardcoding.** Glob `$HOME/.claude*/sessions/*.json`. This machine runs
three profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) and the set is
per-machine; hardcoding it would make the check quietly wrong on any other host. Each
registry row carries everything needed:

```json
{"pid": 2414613,
 "sessionId": "4a8fb556-240b-4ee9-9db5-ec3cc3008a17",
 "cwd": "/home/marius/work/claude/codescout",
 "procStart": "79345929",
 "messagingSocketPath": "/run/user/1000/cc-socks/2414613.sock",
 "name": "codescout-a8", "nameSource": "derived",
 "status": "busy", "updatedAt": 1788375958815}
```

**A registry row is not liveness.** Measured 2026-09-02: 42 registry files across three
profiles against 29 live sockets. Rows outlive their sessions. Liveness is a three-part
conjunction:

1. the socket exists at `messagingSocketPath`, **and**
2. `/proc/<pid>` exists, **and**
3. `/proc/<pid>/stat` field 22 (`starttime`) equals the row's `procStart`.

Condition 3 is what defeats **pid reuse**. With rows outliving sockets by 13 on this
machine today, a recycled pid landing on a stale row is not hypothetical, and without the
`procStart` comparison such a claim reports live — the wrong answer in the dangerous
direction, since it tells a reader to stay off work nobody is doing.

The join is exact, not approximate. Verified 2026-09-02 against a live session
(pid 2414613): `/proc/2414613/stat` field 22 read `79345929` and the registry row's
`procStart` read `79345929` — equal as strings, no unit conversion, no tolerance window.
System uptime at the time was ~79426875 ticks, confirming the value is boot-relative
starttime in clock ticks rather than a wall-clock stamp. Compare as strings; do not parse
to a number and do not allow a tolerance, because the whole value of the field is that a
reused pid produces a *different* starttime.

### Three buckets, not two

| bucket | condition | remedy |
|---|---|---|
| `claim_held_by_live_session` | all three conditions hold | informational; carries the `to:` value |
| `claim_held_by_dead_session` | sessionId found in a registry, liveness fails | demote to `investigating` |
| `claim_unresolvable_here` | sessionId in no local registry | **not** a defect — name the scope searched |

The third bucket is not padding. On any second machine, *every* claim made elsewhere is
unresolvable, and folding those into "dead" would produce a confident wrong answer at
scale. Per `docs/adrs/2026-08-27-negative-results-name-their-scope.md`, the finding names
the profile directories it searched rather than claiming a bare zero.

### The messaging affordance

For the live bucket, the check emits the exact `to:` value, so acting on a finding is a
copy-paste rather than a re-derivation:

- claimer is in the **same profile** as the reader → the bare `name`
- any **other profile** → `uds:/run/user/<uid>/cc-socks/<pid>.sock`

This is the addressing table from `CLAUDE.md` § *Reaching a Peer Session*, applied
automatically. It is also the half of the feature the request actually asked for: the
record should let you talk to whoever holds it.

Also surface the row's `cwd`, since a claimer working a *different checkout* is a
materially different situation from one in yours.

## The check

`scan_claim_liveness`, a 24th sibling to the 23 existing `scan_*` functions in
`src/librarian/tools/doctor.rs`. Input is catalog rows with `kind = 'bug' AND status =
'taken'`. Registry directory and socket directory are **parameters**, not ambient lookups,
so tests can seed a fake registry.

## Wiring — the part most likely to sink this

A new status the canonical triage query does not list is **invisible**. Today's query is

```
filter={"status": {"in": ["open", "investigating", "zombie"]}}
```

so shipping `taken` without updating it would make every claimed bug vanish from triage —
a fresh instance of `issue-clusters:IC-3` (*declaration is not execution*), in a repo that
gates against that class. Every surface below must move in the **same commit**. Grepped
2026-09-02:

| # | surface | what changes |
|---|---|---|
| 1 | `src/librarian/tools/create.rs:83` | `BUG_STATUSES` gains `"taken"` |
| 2 | `src/librarian/tools/doctor.rs:5325` | SQL `IN ('open','investigating')` |
| 3 | `src/librarian/tools/doctor.rs:4453` | message text naming the triage query |
| 4 | `src/prompts/guides/tracker-conventions.md:31` | status table |
| 5 | `src/prompts/guides/tracker-conventions.md:742-755` | 3 query spots + the `zombie` note |
| 6 | `src/prompts/guides/project-activation-bootstrap.md:11-13` | triage query |
| 7 | `docs/issues/_TEMPLATE.md:21,41` | query + status glossary |
| 8 | `CLAUDE.md` § *Querying active trackers* | triage query |
| 9 | `src/librarian/tools/doctor.rs:414` | **register** `scan_claim_liveness` — `all_violations.extend(…)` |
| 10 | `src/librarian/tools/librarian.rs:30` (`Librarian/description`) | name the new check so agents can route to it — reach it with `edit_code` |
| 11 | [`docs/PROBES.md`](../../PROBES.md):173 | the `doctor` row — the instrument index `CLAUDE.md` says to read first |

Note that #2 (`scan_non_terminal_status_with_fix_anchor`) must treat `taken` as
non-terminal: a claimed bug whose body has grown a `## Fix` section owes a status flip
exactly as `open` and `investigating` do.

**Surfaces 9-11 were added 2026-09-02 after a reconnaissance pass; the table shipped with
eight and was wrong.** #9 is the registration line without which the check compiles, passes
its own tests, and is never called. #10 and #11 are worse than they look: **five** surfaces
currently describe `doctor` as a *catalog-drift* scanner over ~6 checks when it runs **23**,
so the convention of describing a new check has already lapsed — the newest check,
`non_terminal_status_with_fix_anchor`, appears in none of them. Following local precedent
here would ship `scan_claim_liveness` **wired and undiscoverable**. Filed as
`docs/issues/archive/2026-09-02-doctor-doc-surfaces-describe-six-of-its-twenty-three-checks.md`;
scouted in `bug-claim-liveness-session-log:F-1`. The implementation plan must treat #9-11
as required, not optional.

The other two of those five are rustdoc module headers (`src/librarian/tools/doctor.rs:1`,
`src/cli/doctor.rs:1`). They describe the module rather than enumerate checks, so they are
**not** wiring for this plan — they move only if the bug above is fixed by re-describing
`doctor` by check family. Deliberately excluded here rather than forgotten.

Surfaces 4-6 are prompt surfaces; check `src/prompts/README.md` for whether
`ONBOARDING_VERSION` needs bumping and whether any slice crosses the 1900-character cap.

## Testing

Per `CLAUDE.md` § *Testing Discipline*:

- **Demand an observed RED, and mutate the production path.** The `procStart` comparison
  is the specific line to break: delete condition 3, seed a registry row whose pid is live
  but whose `procStart` differs, and the suite must go red. A test that only asserts the
  happy path is monotone under that removal and proves nothing.
- **One member per bucket.** A suite exercising only `dead` is monotone under
  `unresolvable_here` collapsing into it — the two are distinguished by a branch, and a
  test that never reaches the branch cannot see it merge.
- **Name the observer.** The check's consumer is the session running `doctor` before
  picking up work, and the triage query that now includes `taken`. If neither path reaches
  the finding, it is decoration however loudly written.
- Positive test that `"taken"` is accepted by `BUG_STATUSES`, alongside the existing
  `an_out_of_vocabulary_bug_status_is_refused` (`create.rs:577`).
- A wiring test that the triage query returns a `taken` bug — this is the guard against
  the IC-3 failure in § *Wiring*, and it must fail if any of surfaces 1-3 is missed.

## Risks

- **The claim is advisory.** Nothing prevents two sessions claiming the same bug, and the
  check is a scan rather than a lock. This is deliberate — a lock over a git-tracked file
  on a shared checkout has its own failure modes, several of them already filed
  (`2026-09-02-staging-is-not-a-state-you-can-hold.md`).
- **`extra` is not indexed**, so per-claimer queries are unavailable until someone wants
  them enough to migrate the schema.
- **Liveness is Linux-specific** (`/proc`, `/run/user/<uid>`). On other platforms the
  check should degrade to `claim_unresolvable_here` rather than guess — the third bucket
  already provides the honest answer.

## References

- `CLAUDE.md` § *Reaching a Peer Session*, § *Observer Blindness*
- `get_guide("tracker-conventions")` § *Bug files*
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- `docs/trackers/issue-clusters.md` — `IC-3`, `IC-8`
