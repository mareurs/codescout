---
id: 857ccb33f26c7363
kind: bug
status: fixed
title: 'BUG: file-provenance prints the window on every verdict except UNKNOWN, the one whose meaning is the window'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- file-provenance
- shared-checkout
- diagnostics
topic: authorship attribution on a shared checkout
---

# BUG: file-provenance's UNKNOWN branch omits the one line that explains it

## Summary

`scripts/file-provenance.py` prints `window: writes at or after <floor>` on `MINE`, `SHARED`
and `PEER`, and **not** on `UNKNOWN` — the single verdict whose entire meaning is the window.
The default floor is *"since this path was last committed"* (`:599-608`), so a **clean file
always returns UNKNOWN**, correctly and uninformatively.

The accompanying prose then explains `UNKNOWN` via exactly one cause — a Bash write the
heuristics missed — and never mentions the far more common one: the file is committed, so
nobody has bytes at risk. Two opposite situations render in identical text:

| input | what UNKNOWN means | what the output says |
|---|---|---|
| clean file | **nobody, dispositively** — nothing is at risk | "statement about coverage, not ownership" |
| dirty file, unattributed write | **a real coverage gap** | the same sentence |

## Symptom (Effect)

Measured 2026-09-16, in the field rather than constructed. Session `9e022ef0` ran the tool on
three `.rs` files I had committed minutes earlier, got `UNKNOWN` on all three, and reported it
as the tool failing to attribute — concluding that diffing a symbol against `HEAD` was
*"strictly better than provenance."* Those are not competing instruments; one answers *who has
uncommitted bytes here* and the other *is this mechanism new*. The `UNKNOWN` text is what
licensed the comparison.

Nothing failed. No error, no exit code, and the reader was experienced enough to be running
provenance unprompted on a shared checkout — which is the population this tool exists for.

### Four incidents, three readers, one day — and they cost different things

All 2026-09-16, all in the field, none constructed. Reported by the readers themselves.

1. **`9e022ef0`, three `.rs` files, clean.** Read as attribution failure; concluded a different
   instrument was "strictly better than provenance". (Above.)
2. **`f5f48b42`, `src/tools/output_buffer.rs`, ~09:24Z.** Asking whether a peer's claim of two
   live writers was right. Got `UNKNOWN` + `(47 write(s) predate the window)` and **could not
   price the gap without a second run**; `--all` returned **8 writers, 3 live**. This is the
   expensive direction — the verdict masked *live peers*, which is the one thing the tool
   exists to surface.
3. **`e5691fad`, twice** — on `2026-09-16-archiving-a-peers-bug-file` and on
   `architecture-boundary-measurement.md` before repointing its ids. Read `UNKNOWN` as *proceed
   with caution*; both were in fact **dispositive clearances**.

**Incident 3 names a cost this file did not originally argue, and it is the larger one.** Their
formulation: *the misreading does not only make you over-cautious, it makes you unable to cite
the instrument that actually settled it.* Over-caution is visible and self-limiting — you go
ask, it costs a round trip, you proceed. Reaching the right answer by a **weaker argument than
the one available** is neither: the conclusion is correct, so nothing prompts a second look, and
the strong argument stays unreachable indefinitely. A reviewer reads a defensible hedge, not an
error, and has no reason to upgrade it.

So the population served by a fix is not only *readers made over-cautious*. It is also *readers
who reached the right answer and recorded a weaker justification than the tool had already
computed for them* — *invisible by construction*, because nothing they did was wrong.

## Reproduction

```
git commit -- <path>                       # any path, any session
python3 scripts/file-provenance.py <path>
  -> UNKNOWN   <path>
     no record of any session writing this path in the window. ...
     (N write(s) exist but predate the window; re-run with --all to see them)
```

The `hidden` count is the only hint the window is doing the work, and it reads as an aside.
Compare a **dirty** path, where the same tool prints `window: writes at or after <ts>` and the
reader can see immediately what was excluded.

## Root cause

Read, not inferred. `scripts/file-provenance.py:623-640` versus `:642-647`:

```python
if not who_set:                       # UNKNOWN
    print(f"UNKNOWN   {rel}")
    print("          no record of any session writing this path in the window. ...")
    if hidden: print(f"          ({hidden} write(s) exist but predate the window; ...)")
    continue
...
print(f"{verdict:9} {rel}")
if floor:                             # MINE / SHARED / PEER only
    print(f"          window: writes at or after {floor}")
```

**The author already performed this exact harmonisation for the sibling field**, and the
comment at `:617-620` states the general reason:

> Records the window excluded — present regardless of verdict, because a hidden write is
> exactly as real on a MINE path as on an UNKNOWN one. Equals `len(records)` whenever
> `who_set` is empty, which is what makes this a drop-in for the count the UNKNOWN branch
> used to compute only for itself.

`hidden` was lifted to print on every verdict. `floor` was not, and the reason given for
`hidden` applies to it verbatim. One law, two sites, fixed at one.

### What `UNKNOWN` actually covers — measured, because a peer's frequency claim was wrong

`f5f48b42` observed that the `hidden` count is what saved them (*"47 writes predate the
window"* is the only hint the window is doing work) and proposed that `UNKNOWN` with
`hidden == 0` — total silence — is the case the default floor produces **most often**. The
mechanism is right and **the frequency is not**. Measured over the first 12 tracked
`src/**/*.rs`, 2026-09-16: of 7 `UNKNOWN` verdicts, **5 printed the hidden hint and 2 did not**.
The hinted case is the common one; the silent one is real but the minority.

The scan also surfaced a distinction neither of us had drawn — an **untracked** file has no
floor at all, so its silence is *correct*:

| state | `floor` | `hidden` | what `UNKNOWN` means | what prints |
|---|---|---|---|---|
| untracked | `None` | 0 | no records at all | nothing — **correct**, no window exists |
| tracked, clean, records exist | last commit | >0 | nobody holds uncommitted bytes | hidden hint, no floor |
| tracked, clean, no records | last commit | 0 | nobody, and no coverage either | **nothing at all** |
| tracked, dirty, writers outside the window | last commit | >0 | the window is too narrow for the question asked | hidden hint, no floor |

Rows 2 and 4 are the ones that matter and the hidden hint **cannot separate them** — it says
writes predate the window without saying what the window is, which is exactly incident 2. Row 3
is the fully silent case. Row 1 needs no repair and a fix must not add noise there.

## Classification

`cluster/value-correct-in-a-frame-its-name-does-not-state` (`IC-24`), on the **verdict word**,
which is a fifth axis alongside coordinate space, unit, citation form and scope word. `UNKNOWN`
is exactly right in the since-last-commit frame and exactly recoverable (`--all`, or running
before you commit); the word means *epistemic uncertainty* and the computation means *an empty
window*. IC-24's blind party holds here without adjustment: the producer is the party for whom
"no writes in window" and "unknown owner" coincide, and re-running the tool returns the same
correct value.

**Weighed and rejected.** `cluster/guard-narrower-than-its-name` — nothing is refused.
`cluster/selector-narrower-than-its-population` (`IC-18`) — the selector is right and the
population is right; only the report of them is short. **And stated because the fit should be
contestable rather than asserted:** a reader who thinks the naming half is incidental and the
branch asymmetry is the whole defect would file this under a class about a fix applied at one
of N sites, and that reading is defensible. It is IC-24 here because the asymmetry is only
expensive *through* the word — a reader who knew the window would not be misled by the missing
line.

## Fix
FIXED in two commits, both halves. The second corrects the first; the plan as originally
written is left below, because two of the corrections are about it.

| commit | patch-id |
|---|---|
| `4077a9f7` | `ffd3107ce4a6da2de165bd0c2de0b57dd4f65220` |
| `57bb954e` | `1282290562ad21fdbd03a9e1b8843eb900daca09` |

**Half 1.** The window prints on `UNKNOWN`, lifted exactly as `hidden` was. `if floor:` is
unchanged, so an untracked path stays silent.

**Half 2.** "The one blind spot is a Bash write" is now "One blind spot", and a `LIKELY
CAUSE:` line precedes the caveats rather than following them.

**WHAT SEPARATES ROWS 2 AND 4 — the pair this file says `hidden` cannot separate.** A new
`worktree_is_dirty()` reads git rather than the transcripts, so it shares **no blind spot**
with the heuristics: a Bash write they miss still dirties the tree. `clean` therefore
settles what *no record* cannot, and the clearance is dispositive rather than a hedge.
`dirty` says the window is too narrow and sends the reader to `--all`, which is incident 2's
actual remedy.

**TWO CORRECTIONS TO THE PLAN BELOW, both earned by running the reproduction first.**

1. The proposed guard `hidden == len(records)` is a **tautology inside this branch** — an
   empty `who_set` means `in_window` and `undated` are both empty, so `hidden` always
   equals `len(records)` here. It discriminates nothing and `records` is the whole
   condition. Stated in a comment at the site so nobody restores it as a guard.
2. The reproduction produced a row the *Root cause* table does not have: **tracked, DIRTY,
   zero records** (`.codescout/audit/ripper-65e654-202609.jsonl`). `hidden == len(records)`
   is `0 == 0` there, so the plan as written would have claimed *no session holds
   uncommitted bytes* about a file that holds them — one false claim traded for another.
   Requiring `records` excludes it: it now prints the window and no cause line, which is
   correct, because with no records we know nothing.

**AND THE FIRST DRAFT OF THE FIX SHIPPED AN INSTANCE OF THE DEFECT IT WAS FIXING.**
`worktree_is_dirty()` read `git status --porcelain -- <path>` alone. For a path the repo
does not **track** that command *succeeds and prints nothing* — byte-identical to a clean
tracked file — so the helper answered `False` and `UNKNOWN` printed a dispositive clearance
about a path git holds no baseline for. `ls-files --error-unmatch` is the discriminator and
now runs first. Caught by a mutation-driven assertion rather than by reading: the docstring
had already called the three-valued `None` load-bearing while nothing checked it. The same
draft also claimed the floor was *"its last commit"*, which is false under `--since`; the
clause is gone, since the floor is printed on the line above.

Not attempted. Two halves, and unlike the last bug I wrote that sentence about, the cheap half
is genuinely safe alone — it is the *same* half, not a louder version of a quieter one.

1. **Print `window: …` on `UNKNOWN` too.** One line moved above the branch, exactly as `hidden`
   was. This is the whole mechanical defect.
2. **Name the benign cause first in the prose.** The current text leads with the blind spot,
   which is the rarer case; a clean file is the common one and currently goes unnamed. Something
   of the shape *"this path is committed, so nobody holds uncommitted bytes in it"* when
   `hidden == len(records)` and the worktree is clean, with the coverage caveat kept second.

Half 1 without half 2 already fixes the reported misreading: `window: writes at or after
<a timestamp two minutes ago>` is self-explaining next to a file you just committed.

### The two halves are an OMISSION and an ACTIVE FALSE CLAIM, and only the first is in the title

`9e022ef0`'s sharpening, taken: half 1 is a missing line, but half 2 is not a milder version of
it. *"The one blind spot is a Bash write"* is a **completeness assertion**, and it is false —
the common cause is a committed file whose floor sits at its own commit time. A reader who does
exactly what the message instructs goes to investigate the rarer cause, sent there by a sentence
that forecloses the likelier one.

That makes the two **independently sufficient**, in opposite ways: printing the floor makes the
situation self-evident *despite* the prose, and correcting the prose helps *without* the floor.
Two sites, two repairs, and the title names only the first. Left as-is deliberately — the
filename is the slug and renaming it costs every citation — but a reader who fixes only what the
title says will leave a false sentence shipping.

And a constraint the table in *Root cause* adds: a fix must stay quiet on an **untracked** path,
where `floor` is `None` and the silence is already correct. `if floor:` is the right guard; the
bug is only that it sits below the `continue`.

## Tests
DONE. `tests/file-provenance.sh` 144 → 153, with all six new assertions observed red first.

Shape assertions rather than prose pins, as this section asked: `window: writes at or after`
plus its floor **value**, `dispositive`, `too narrow`, `LIKELY CAUSE`. The fixture path
`src/frame_probe.rs` deliberately shares no substring with any marker asserted on it — the
constraint the `--since` section already annotates.

**MUTATION EVIDENCE**, once per guarded site and re-run across every bound after the last
change. Baseline `passed=153 failed=0`; verdicts read off the suite's own count line.

| # | mutation | verdict |
|---|---|---|
| M1 | delete the `UNKNOWN` window print | KILLED (4) |
| M2 | drop the `records` guard | KILLED (1) |
| M3 | clearance fires on any known dirtiness | KILLED (2) |
| M4 | remove the too-narrow branch | KILLED (1) |
| M5 | an untracked path reads as clean | KILLED (1) |
| M6 | the defensive `status` returncode branch | SURVIVED — predicted, annotated inert at the site |

M2 and M5 **survived all 150 assertions** of the first draft, which is why the two cases
that kill them exist at all; M5's survival is what exposed the tracked-path defect above.
M6 is the third reading of `SURVIVED` — reachable by no input in the tree, annotated as
defensive so it is not credited with coverage.

**TWO THINGS THE SUITE'S OWN SHAPE CORRECTED**, recorded because each was asserted falsely
first. `tests/file-provenance.sh:602` already runs `git init`, under a section headed *"the
DEFAULT window derives from git, and is the load-bearing half"* — so the default floor was
**not** unexercised, and a second fixture repo stood up on that belief has been removed. The
belief came from a `git init` grep whose output `head -40` truncated before line 602: an
absence read off a cap, which is this file's own verdict one namespace over. And
`mutation-probe.sh` **does** carry uncommitted files into its worktree — verified at 154
local / 154 isolated after a first probe that could not discriminate — so committing before
mutating was belt-and-braces, not a requirement.

`./scripts/gate.sh` → `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. No Rust changed, so the gate
reports that the tree is green rather than anything about this fix; the coverage claim rests
entirely on the suite and the mutation table above.

None yet. The shape a guard needs, noting that this is the half `tests/file-provenance.sh` is
structurally weakest on: 140 assertions there are about **verdicts** — which party is named —
and this defect is entirely in the **caveat lines** beside a verdict that is already correct.
`CLAUDE.md` § *Testing Discipline* names the general case (a suite tests a guard's predicate and
never its remedy text) and its measured remedy is a **shape** assertion, not a pinned sentence:
assert `UNKNOWN`'s output contains a `window:` line whenever a floor exists. That reds on the
deletion and survives rewording.

## References

- `scripts/file-provenance.py:599-608` (floor), `:617-620` (the sibling harmonisation and its
  stated reason), `:623-640` (UNKNOWN), `:642-647` (the branch that prints the window).
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a zero owes its scope. This one
  names the scope *word* and withholds the scope *value*, on the branch where the value is the
  answer.
- `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`
