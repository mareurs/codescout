---
kind: bug
status: taken
tags:
- cluster/doc-contradicted-by-code
- peer-sessions
- authorship
- provenance
claimed_by: c9ab2c8d-dd74-43f4-9940-25756379a312
closed: null
opened: 2026-09-01
owner: marius
related:
- docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md
- docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md
severity: medium
---

# `CLAUDE.md` denies a pid→session join that every live registry entry carries

**Found:** 2026-09-01, while routing a shared-checkout commit question.
**Affects:** `CLAUDE.md` § *Observer Blindness* (final sentence of the authorship
paragraph), and `scripts/file-provenance.py`, which stops one join short of an address.

## Summary

`CLAUDE.md` closes its authorship procedure with:

> Neither `/proc/<pid>/environ` nor the transcript JSONL carries a session id, so there is
> **no pid→session join**; this is a **session→id** join the session publishes on request.

The premise is true and the conclusion is false. Both named sources genuinely lack a session
id — but `<config-dir>/sessions/<pid>.json` carries `sessionId`, `pid`, `messagingSocketPath`
and `name` **in one record**, so the join is not merely available, it is a complete four-way
one, offline, with nothing to ask.

The cost is not cosmetic. The paragraph's own remedy is *"ask the session and have it quote
its scratchpad path"* — a round trip that spends a peer's turn — and it is presented as the
best available identifier precisely because the cheaper one is declared not to exist.

## Symptom (Effect)

`scripts/file-provenance.py` names writers as **session ids**:

```
SHARED    docs/trackers/issue-clusters.md
          window: writes at or after 2026-09-01T12:40:35+00:00
          written by THIS session (b2a50de8)
          written by 6524892b-e096-4c65-b3a5-89cdc9cd49ed
```

A session id is not addressable. `SendMessage` takes a name or a socket path, so under the
documented procedure the id is a dead end and the next step is to broadcast to a candidate
set and ask. `CLAUDE.md` records what that costs: a 2026-09-01 broadcast reached two of five
peers, **both innocent**, while the two real writers sat among the sessions `ListAgents`
omits.

## Reproduction

Measured 2026-09-01 on this machine, 16 live sessions across 3 profiles:

```bash
# the join the doc says does not exist
python3 -c "
import json; d=json.load(open('$HOME/.claude-sdd/sessions/2601241.json'))
print(d['pid'], d['sessionId'], d['name'], d['messagingSocketPath'])"
# 2601241 6524892b-e096-4c65-b3a5-89cdc9cd49ed codescout-b7 /run/user/1000/cc-socks/2601241.sock
```

Universality, rather than one lucky record:

```bash
# live registry entries: 16   carrying sessionId+socket+name: 16
```

Both `/proc/<pid>/environ` and the transcript JSONL were re-checked and do lack it, so the
doc's premise is not the error — only the inference from it.

### Second route — from a COMMIT to the session that wrote it

The route above starts from a pid and is about a **live writer**. Starting from a commit closes
the other half — **committed state** — and it is the half that comes up in practice, because the
question is usually *"who wrote this line?"* rather than *"who is running right now?"*.

Measured 2026-09-01 by `codescout-3e`, resolving the author of `75ec6310` with no prior knowledge
of which session it was:

```bash
git log -1 --format='%b' 75ec6310 | tail -1
# Session-Id: b2a50de8-a666-4933-b030-f7bf8e18fd6a

grep -l "b2a50de8-a666-4933-b030-f7bf8e18fd6a" ~/.claude*/sessions/*.json
# /home/marius/.claude/sessions/2201565.json      -> pid 2201565, name codescout-3c, profile .claude
```

Three hops, all of them lookups rather than inferences: **commit → `Session-Id` trailer →
registry entry → pid, name, profile, socket.** The peer was then addressed directly at
`uds:/run/user/1000/cc-socks/2201565.sock` — cross-profile, so the bare name would have been
refused.

Note what this does *not* weaken. `CLAUDE.md` § *Observer Blindness* is right that the trailer
"exists only once a commit lands, and uncommitted state is where every misattribution that day
occurred" — that stays true, and the scratchpad-path ask remains the route for uncommitted work.
What the section gets wrong is narrower and is this file's subject: it states there is **no
pid→session join** at all, and the registry is a join in both directions. For committed state the
chain above is complete and positive, needing no elimination step and no candidate set.

## Worked demonstration — the same session, 20 minutes later

The routing question that surfaced this produced three unknown writer ids across two files.
All three joined to live, named, addressable peers with **nothing asked of anyone**:

| session id | name | pid | profile | reachable as |
|---|---|---|---|---|
| `3e275c54` | `codescout-17` | 3624594 | `.claude-sdd` | `uds:/run/user/1000/cc-socks/3624594.sock` |
| `bf44ba81` | `codescout-8a` | 3261760 | `.claude` | `uds:/run/user/1000/cc-socks/3261760.sock` |
| `6524892b` | `codescout-b7` | 2601241 | `.claude-sdd` | `uds:/run/user/1000/cc-socks/2601241.sock` |

Two profiles, so a per-profile sweep would have resolved at most two of the three — the join
inherits the scope rule that governs every other peer instrument here, and the enumeration
must span `$CLAUDE_CONFIG_DIR` profiles exactly as the socket sweep does. Enumerated on this
host: `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`.

**And the join earned its keep on the first use.** A peer, coordinating in good faith,
attributed an untracked bug file to this session — *"your `…-peer-idle-timeout-…`"* — on
adjacency alone. `file-provenance.py` returned `PEER`, naming `3e275c54` and `bf44ba81`:
neither this session nor the peer making the claim. Under the documented procedure the
correction stops there, at *"not mine, and I cannot say whose"*. With the join it completes —
both writers are named, live and one message away. That is the difference between refuting an
attribution and repairing one.

### The same error, in both directions, twenty minutes apart

Recorded because a one-sided version of this would read as the instrument vindicating its
finder. Both sessions in the exchange made the adjacency error, at each other:

- **The peer → me.** *"Your `…-peer-idle-timeout-…`"* — an untracked file sitting beside my
  edits. Provenance: `PEER`, written by `3e275c54` and `bf44ba81`, neither of us.
- **Me → the peer.** I wrote *"your `src/server.rs`"* in a message **and in the message of
  commit `455184eb`**, on the same basis: it was dirty in a tree where they were the peer I
  happened to be talking to. Provenance: `3e275c54` — `codescout-17`, a third session. They
  had not opened the file.

Neither of us was careless, and both of us knew the rule; one of us was *writing about the
rule* at the time. That is the § *Observer Blindness* admission test passing on the nose —
"be careful" is the wrong instrument, and what closed both was the same mechanical join.

**The asymmetry that matters for repair:** their misattribution cost a message. Mine went
into a **commit message**, where it is durable, pulled by every peer, and not correctable
without rewriting a SHA others may hold. `455184eb`'s *"Their src/ work … left in the working
tree"* is wrong about the owner and stands uncorrected in git; this file is the correction.
An attribution is cheap to make and expensive to retract exactly in proportion to how durable
the surface is — which argues for resolving one **before** it reaches a commit message, not
for resolving it more carefully.

## Corroboration, and one refinement that does not hold

**No longer single-party.** `codescout-17` verified the join independently and against a
ground truth this session does not have: their own session id, which the harness publishes to
them as a path component of their scratchpad. `~/.claude-sdd/sessions/3624594.json` →
`"sessionId":"3e275c54-…"`, matching their scratchpad path exactly. That is the check this
finding most needed — the original measurement could only show the field is *present* and
*consistent*, never that it names the session it claims to.

**Refinement accepted — the registry is per-profile, and that is the same scope trap one layer
down.** A pid belonging to another profile is simply *not a file* in yours, so a
single-profile lookup returns `no such file`, which reads as **"no join"** rather than **"wrong
directory"**. So the join inherits `ListAgents`' defect exactly, and a correct finding becomes
a wrong conclusion on its next use unless the lookup sweeps every `$CLAUDE_CONFIG_DIR`. Any
fix must iterate profiles, not just read one.

**Refinement NOT accepted — `pid` is a field, not only the filename.** `codescout-17` reported
that `pid` appears only as the filename (`sessions/<pid>.json`), making the join
filename→contents, *"which matters for anyone grepping for a `pid` field and concluding it is
absent (I did, for one call)"*. Measured here across all three profiles: **16 of 16** live
entries carry a `pid` **field**, including `3624594.json`, their own.

**The cause — measured by its author, and it is not the one proposed here.** This file first
guessed whitespace: minified bytes `"pid":3624594` against a grep for the pretty-printed
`"pid": `. **Falsified** — `grep -c '": '` over that file returns **0**, so no spacing
assumption was in play. `codescout-17` supplied the real mechanism: their loop built one
pattern for five keys,

```
grep -o "\"$k\":\"[^\"]*\"" "$f"      # requires a QUOTED value
```

and `sessionId`, `name`, `messagingSocketPath` and `cwd` are JSON **strings** while `pid` is a
**number**. The type-uniform pattern excluded exactly the one key typed differently.
Reproduced here: 4 of 5 keys match, `pid` alone returns nothing.

**And the part worth carrying is why it persuaded.** The search was **4/5 correct**, so the
instrument was visibly working and the one blank read as a property of the *data* rather than
of the *pattern*. A uniform 0/5 sends you straight to the pattern; a partial hit does not.
**Partial success is the camouflage** — which makes this a strictly worse case than `R-3`'s
empty result, because the usual tell (nothing came back, so suspect the search) is exactly
what a 4/5 removes. Their session logged it as `reconnaissance-patterns:R-160`, their fourth
false-negative filter that day; one `cargo test` filter reported `ok` having matched 0 of 4820
tests.

Kept in full rather than silently corrected, because the sequence is the point: a refinement
was offered and was wrong, the correction to it proposed a mechanism that was **also** wrong,
and only the third account — from the party who still had the failing command — held. Two of
the three wrong steps were confident and neither produced an error. It also means the join is
available **both ways** — pid→session from the filename, and session→pid by scanning contents — so a
repair does not have to derive one direction from the other.

## Root Cause

The sentence generalises from two sources to a namespace. Both are **process**-level
artifacts, and the session id lives in the **registry**, which is the one surface the
authorship procedure never reads for this purpose — although the peer-enumeration skill
already opens exactly these files, for `name` and `status`, and simply does not read the
adjacent `sessionId` field.

So this is `IC-11` (*documentation denies a capability the code has since gained*) with an
`IC-18` flavour: the selector (*"process-level sources"*) is narrower than the population it
was taken to cover (*"anywhere a session id might live"*).

## Impact

Routing on a shared checkout is the corpus's most expensive recurring failure, and this
removes the last hop. With the join, the sequence

> enumerate over sockets → intersect with the write-derived set → **ask** the survivors

becomes

> enumerate over sockets → **join** the write-derived id to a live socket → address the writer

Asking survives as the fallback for a writer whose session has exited (no `/proc`, possibly no
registry entry). That is a real residue and the reason this is not a total replacement: the
join covers **live** writers only, which is the common case precisely because
`file-provenance.py` is windowed to recent writes.

## Fix

Two parts. **Both are now done**, and they were done at different times by different
sessions — part 1 had already landed before this fix was picked up.

1. ~~Correct the `CLAUDE.md` sentence.~~ **Already true when this was re-opened**, 2026-09-08.
   The denial sentence is gone (`grep -n "no pid→session join" CLAUDE.md` → no match) and
   § *Reaching a Peer Session* now publishes the route positively at `CLAUDE.md:341`, with the
   per-profile scope rule attached and the caveat that the registry row is still self-asserted —
   one level short of proof, one full level above a self-report. Nothing was owed here.

2. **Done.** `scripts/file-provenance.py` now joins each named sessionId against every profile's
   live registry and prints `[LIVE]` with the `uds:` socket to `SendMessage`, or
   `[not live — cannot be asked]`.

### What the second part actually required, beyond the sentence above

**Liveness had to be checked, not inferred from the row.** A registry file outlives its session,
so `sessions/<pid>.json` existing is not reachability. The reason that matters more than it
sounds: routing to a dead session ENOENTs, and that error is *byte-identical* to a cross-profile
**name** refusal — which the peer skill tells you to answer by switching to the `uds:` form. So an
unchecked row does not merely mislead; it sends the reader down a documented remedy for a
different problem, and they retry instead of re-attributing. Liveness is socket-present **and**
pid-alive; `PermissionError` from `kill(pid, 0)` counts as alive, or a live foreign-profile
session reports as exited.

**The dead majority collapses to one inline marker; only reachable sessions expand.** Measured on
this repo: `--all` over `docs/trackers/issue-clusters.md` names **35** lifetime authors of which
**1** is reachable. The first cut printed a full "you cannot ask it" sentence per session and
buried the single row the reader came for.

**The address given is `uds:`, never the name.** A name resolves only inside its own profile and
is re-minted by compaction, resume or a restart elsewhere; the sessionId is not. The name is
printed as a label beside it.

**A sid in two live rows is reported, not resolved.** `IC-6`'s no-disambiguator half — silently
addressing one of two is a coin flip.

**`UNKNOWN` is untouched.** The join must not manufacture an address where there was no
attribution, so that verdict keeps its coverage caveat and acquires no session line and no footer.

**The footer carries unit, scope and instant** — `N of M named session(s) live at <ts>, across K
profile(s)`. The instant reads as decoration and is the half that gets dropped: two honest
enumerations hours apart share almost no pids, so an unstamped count makes ordinary churn present
as a tooling defect and sends the reader to debug a working instrument.

### One thing this fix got wrong first, and the peer who caught it

The first cut hardcoded three profiles — `.claude`, `.claude-sdd`, `.claude-kat` — which is the
per-profile-subset hazard *this very bug file is about*, reproduced inside the instrument that
fixes it. Raised by sessionId `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`, who named
`default_profile_dirs()` in `src/librarian/session_registry.rs` as the Rust function that had
already made the opposite choice, deliberately, for the same reason.

Verified rather than accepted, and it was **worse than reported**: this machine carries **7**
`.claude*` directories and **5** with a `sessions/`, so the hardcoded list was already a subset of
*this* box, not merely of some future host. Both `registry_roots()` and its pre-existing twin
`transcript_roots()` now share one discovered `profile_dirs(leaf)` helper — fixing one and leaving
the other is how a class survives being fixed.

The `.claude`**`-`** separator is load-bearing: a bare `startswith(".claude")` also admits
`.claudeish`, which is the same prefix-swallow as the git-verb regex that refused read-only
plumbing.
## Tests added

`tests/file-provenance.sh` grew from **68** to **102** cases, in two sections.

**`== a sessionId is an ADDRESS, not just evidence ==`** (26 cases) drives the join through
synthetic registry fixtures: a live peer resolving to name/status/pid/profile/`uds:` socket; a row
whose socket is gone; a row whose pid is dead (socket present, so it fails if only the socket half
is checked); an unregistered sid that must still be *named* even though it cannot be asked; a peer
found in the **second** profile; the footer's unit/scope/instant; one sid in two live rows; and
`UNKNOWN` acquiring neither a liveness claim nor a footer. The dead-pid case derives its pid by
scanning `/proc` rather than hardcoding one — a hardcoded "dead" pid becomes a live one the day
the kernel reuses it, and the case would then pass for the wrong reason instead of failing.

**`== profile DISCOVERY ==`** (8 cases) exists because of a measured hole, not by symmetry.

### The observed reds — 13 mutations of the production path, 0 survivors

Run against `scripts/file-provenance.py` itself, not against a re-implementation in the fixture.
Each killed a **named** assertion rather than collapsing the suite:

| mutation | result |
|---|---|
| liveness check removed entirely | KILLED (5) |
| socket checked, pid not | KILLED (2) |
| pid checked, socket not | KILLED (3) |
| only the first registry root scanned | KILLED (5) |
| address printed as name, not `uds:` | KILLED (1) |
| sid collision silently resolved to first | KILLED (1) |
| footer drops the live/named counts | KILLED (1) |
| profile label taken from the leaf dir | KILLED (2) |
| footer printed unconditionally | KILLED (1) |
| discovery reverted to the hardcoded 3 | KILLED (3) |
| discovery returns nothing at all | KILLED (4) |
| `.claude-` separator relaxed to a bare prefix | KILLED (1) |
| existence filter dropped from discovery | KILLED (2) |

### Why the last four are the ones worth reading

The first nine were green on the first mutation run. The last four **survived** it — `discovery
reverted to a hardcoded list` and `discovery returns nothing at all` both left **94/94 green**.

Every case in the suite injects `FILE_PROVENANCE_ROOTS` / `FILE_PROVENANCE_REGISTRY_ROOTS` to stay
hermetic, and that same override filters the production default out of the recording. This is not
a thin sample that a wider corpus would fix: the refuting outcome leaves **no artifact**, at any
corpus size. The `profile DISCOVERY` section omits the override deliberately so the default branch
is observable at all — do not "tidy" those cases onto the shared fixture roots.

The hole was found by mutating rather than by reading, and it was in code written **in direct
response to** a peer's correction — that is, in the part of the patch its author had most recently
thought hardest about.

**Independently reproduced, and it is not an artifact of this patch.** `ad379a7c` ran the same
mutation against the **committed** suite, before any of this work: `transcript_roots() -> return
[]` leaves their **68/68 green**. So the function deciding *where the tool looks at all* could
return nothing and the shipped suite would call the tool correct. Both fixture helpers — `run()`
at `:86` and `runc()` at `:260` — inject `FILE_PROVENANCE_ROOTS`, so the default was filtered out
of the recording in all 68.

**And the sharper form, which is theirs.** They had run six mutations that day and reported six
kills — every one against the **dispatch** (which tool names count, which actions are writes), and
none against the **scope**. That was not restraint: the fixture cannot *express* a scope mutation,
so the missing axis left no artifact either. Their mutation population was itself filtered by what
the harness could see — the recording-filter law one level above where it was being applied, inside
the run used to certify the fix.

The generalisation worth keeping: **hermeticity and default-path coverage are in direct tension.**
A hermetic fixture buys isolation by overriding precisely the thing a default-path mutation would
perturb, and the tension is invisible from inside a green run. `== profile DISCOVERY ==` is the
right shape *because* it breaks hermeticity deliberately and says so on the fixture line.
## Notes

**Not** filed as a bug in the harness. The registry contents are exactly right; the defect is
that our own documented procedure asserts they are not.

**Observer note.** The blind party is the author of the authorship procedure, who had just
finished measuring that `/proc` and the transcripts lack the id — the strongest position from
which to believe the negative, and the one with no reason to open a third file. Found here
only because a routing question forced a *positive* identification, and the registry was
already open for `name`. That is the paragraph's own law holding on the paragraph.
