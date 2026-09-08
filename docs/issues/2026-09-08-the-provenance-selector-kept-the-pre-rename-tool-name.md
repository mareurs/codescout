---
id: '94afb1ade8d7e40a'
kind: bug
status: fixed
title: The provenance selector kept the pre-rename tool name, and every librarian write went unattributable
tags:
- cluster/selector-narrower-than-its-population
topic: shared-checkout authorship
claimed_at: 2026-09-08
claimed_by: ad379a7c-a0cf-4c61-bcdb-f0696fea8c30
opened: 2026-09-08
owner: marius
related: []
severity: high
---

# BUG: the provenance selector kept the pre-rename tool name, and every librarian write went unattributable

## Summary

`scripts/file-provenance.py` answers *"which session wrote this working-tree file?"* — the
positive identifier `CLAUDE.md` § *Reaching a Peer Session* sends you to when a shared-`target/`
red carries no author. It matches tool names in Claude Code transcripts.

On 2026-09-02 `ceb5b57a` renamed the librarian tool `artifact` → `doc`. The probe's selector
kept `mcp__codescout__artifact`. For **six days** every librarian write — **501 write-action
`doc` calls** in this project, which is how essentially every bug file, tracker entry and
archive move is written here — was invisible to the instrument built to attribute writes.

The failure mode is `UNKNOWN`, and the script's own docstring forbids reading that as *"not
mine"*. So the blindness presented as the tool being appropriately humble. A wrong name would
have surfaced in a day.

## Symptom (Effect)

`UNKNOWN` for a file the caller wrote minutes earlier, with a caveat paragraph explaining that
`UNKNOWN` is a statement about coverage. Both halves true; together they are unfalsifiable.

Meanwhile `docs/PROBES.md` advertised the route as working — *"`doc()` write actions resolved
**id → path through the catalog**"* and *"Both routes are handled now"*. The claim was correct
when written and had been dead for six days.

## Reproduction

```
$ ./scripts/file-provenance.py --all docs/issues/<a file you just wrote through doc()>
UNKNOWN   docs/issues/...
          no record of any session writing this path in the window.
```

`--all` disables the window, so this is not the window narrowing the answer: there were zero
records.

## Environment

`experiments` @ `45c1e2ab`, 2026-09-08. Shared checkout, four live sessions.

## Root cause

`write_targets()` dispatches on the transcript's tool name. Its librarian branch read:

```python
elif name in ("mcp__codescout__artifact", "mcp__codescout__artifact_augment"):
```

The `id → abs_path` catalog resolution behind it was **correct and complete** — it had been
built deliberately, after a dogfooding pass found three files reporting `UNKNOWN` for exactly
this reason. Only the key was dead. Nothing was missing; the door was locked.

Secondary, and the reason `augment` needed its own repair: `artifact_augment` was a separate
*tool*, caught by a `name.endswith("_augment")` test. As a `doc()` *action* that test can never
fire, so the tool-name fix alone still left augmentation writes uncovered.

This is `IC-18` and specifically its sharpened mechanism note: **a selector and its population
have independent rates of change, and nothing couples them — staleness needs no author error at
all, only time.** No one was careless. The rename commit touched *"name, audit verb, adapter
classifier, hints"*; `scripts/` was not in anyone's field of view, and could not have been.


**The same defect was found in a sibling probe five days earlier, fixed correctly, and not swept.**
`f41bfeb963dcee56` — *"probe_guide_section_use.py's MECHANISM_TOOLS omits the renamed doc tool"* —
was opened, fixed at `d4ee86da` and closed on **2026-09-03**, the day after the rename. Same
class, same rename, same tag, same week. Its fix was local and correct: it added `"doc"` to one
tuple. Nothing asked *"which other selectors name a tool?"*, so this one sat five more days.

That is the argument for the gate rather than a second local repair, and it is the § *Observer
Blindness* ladder read literally: **an instance fix is not a class fix.** The population was
three files (`scripts/file-provenance.py`, `scripts/probe_guide_injection.py`,
`scripts/probe_guide_section_use.py`) plus `tests/file-provenance.sh`, and it is enumerable by
the same grep that finds the defect — which is exactly why leaving it to the next reader's
noticing was the wrong remedy. A trigger the model must notice is a policy, not a mechanism
(`skill-frictions:SKF-22`).
## Evidence

**Why 58 CI-enforced tests stayed green.** `tests/file-provenance.sh:262` wrote

```
tool_use "$A" mcp__codescout__artifact '{"action":"update",...}'
```

into a synthetic transcript, and the matcher held `mcp__codescout__artifact`. The fixture
re-types the string the code matches, so the two halves agree with each other and neither with
the running server — § *Testing Discipline*, *"mutate the PRODUCTION path, not the test's
inputs."* A rename moves reality and moves neither half. This is why the repair gates **both**
files: gating only the probe leaves the suite free to keep vouching for a name nothing answers
to.

**A gate for exactly this already existed, one directory away.**
`prompt_surfaces_reference_only_real_tools` (`src/server.rs`) cross-checks three prompt surfaces
against the live registry. `scripts/` was outside its population. § *Observer Blindness*, third
position: the bound lived in the enforcement layer, published to an audience that never reads
it, and the party who could see the gap is the one holding the tool list.

**Measured 2026-09-08**, `doc` calls across all sessions in this project, by action:

| | |
|---|---|
| write actions (`update` 353, `create` 52, `append_entry` 45, `move` 43, `delete` 6, `update_entry` 2) | **501** |
| read actions (`find` 188, `get` 127) | 315 |
| total | 816 |

The read majority is why the fix needed a control: counting *every* `doc` call would have made
a reader an author 315 times.

## Hypotheses tried

**The fix this bug was opened to implement was the wrong one, and running the reproduction is
what showed it.** `df517af91b43a5f7` proposes wiring the probe into the moment a red appears
(its option 3). That plan is a hypothesis about a working instrument. Run first, the probe
returned `UNKNOWN` on every live dirty file — so the wiring would have shipped a confident
silence into the exact situation it was built for. § *Bug Tracking*'s *"run the reproduction
before reading the fix plan"*, holding for the fourth time.

## Fix

**Shipped 2026-09-08, in two commits on `experiments`.** SHA and patch-id recorded as a pair at
fix time: the SHA is positional and dies when `experiments` is rebased, the patch-id is a content
hash of the diff and survives rebase and cherry-pick.

| what | SHA | patch-id |
|---|---|---|
| the selector repair, the `run_command` sibling, and the gate | `5ec4bac8` | `61e43235f4b7b9412613fff2ccd62845fef8499b` |
| `doc(move)`'s destination, found by dogfooding the first | `1e90561c` | `fcfc1957d165ea503c5c7208018250773ca03807` |

`write_targets()` names the live tool and keeps the retired ones — old transcripts still hold
them, so this set is append-only. `augment` joins `ARTIFACT_WRITE_ACTIONS` because the arm that
used to carry it cannot fire for an action. Folded in the same commit:
`2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md`, same function, same
class, and its `run_command` branch is a set widening of the `Bash` one.

`provenance_probes_reference_only_real_tool_names` (`src/server.rs`) couples selector to
population. Two directions, deliberately unequal:

- **Positive** — every write-capable tool must appear in the probe. A live write tool *missing*
  is the defect; a dead name sitting in a set is inert, since it only ever fails to match. This
  is the half that reds on a rename.
- **Negative** — no unmarked dead name. Weaker alone: its cheapest repair is to mark the old
  name `legacy`, which leaves the blindness untouched. It earns its place by forcing a
  *decision* at each site rather than letting a rename pass in silence.

`legacy` on the line is the escape, and it is **owed** rather than offered: retired names are
still in older transcripts, so the probe must keep matching them. A gate over a namespace with
no escape refuses work you can describe (§ *Parsers Over a Namespace*).


### The gate's measured ceiling — it would NOT have caught the 2026-09-03 instance

Stated here rather than discovered later, because a gate that looks broader than it is stops the
next person looking. It matches the **qualified** form, `mcp__codescout__<name>`. The sibling
bug's selector was

```python
MECHANISM_TOOLS = ("artifact", "doc", "librarian")   # scripts/probe_guide_section_use.py:120
```

— **bare substrings**, tested with `any(t in name for t in MECHANISM_TOOLS)`. Nothing in that
line is a qualified tool name, so the regex sees none of it. Run against the tree as it stood on
2026-09-02, this gate reds `file-provenance.py` and stays silent on `probe_guide_section_use.py`.
It closes half the class.

The bare form is genuinely harder and is **not** smuggled in here: `"artifact"` as a bare string
is indistinguishable from prose, so catching it needs a declared convention — a marker naming a
constant as a tool-name list — which is a design, not a widening. Worth doing; not worth doing
quietly inside a bug fix.

So the honest claim is: **the qualified form is now coupled to the registry; the bare-substring
form is not, and one known selector still uses it.** A count of "probes guarded" would be the
wrong headline — the unit is *forms*, and it is one of two.

### The second defect, found by using the first fix within the hour

`doc(action="move")` still attributed to nobody, and the cause is structural rather than an
oversight of the same kind. A move **re-keys** the artifact — `id = sha256(abs_path)` — so it
mints a new row and drops the old one. The transcript records the call's input, carrying the
**pre-move** id: a row the move itself deleted. `catalog_paths()` can never resolve it, in any
window, by construction. `new_rel_path` sits in the same input and needs no catalog at all; it
simply was not in the key list.

So archiving — the one librarian operation whose purpose is to move a file someone will later ask
about — was the operation the instrument could say least about. Found because a peer's staged
archive move blocked this bug's own fix commit, and routing it returned `UNKNOWN`.

### Residual, re-derived rather than carried over

`2b9cbd34630cf340`'s Fix section requires this explicitly: its **2.8%** is a figure about `Bash`
calls and is not one about `run_command`. Measured against the production `write_targets()`:

| tool | calls | carry a mutating form the matcher does not resolve |
|---|---|---|
| `run_command` | 21421 | **1.8%** |
| `Bash` | 7516 | **2.9%** |

**The `Bash` column is the control**, and it is why the other number is worth reporting: 2.9%
independently reproduces a 2.8% someone else derived by another route. Without it this is a bare
number from a selector I wrote myself.

And it needed the control: the **first** pass returned 22.5%, because the selector counted `2>&1`
— a file-descriptor redirect — as a file write. A plausible figure from a sloppy selector, which
is IC-18 appearing inside the measurement *of* IC-18. Top residual forms are `python3 -c` and
`python3 - <<`, targets inside a script body, unchanged here.
## Tests added

Nine cases in `tests/file-provenance.sh` (58 → 67), plus the Rust gate. Every assertion was
killed by mutating the **production** path:

| mutation | assertions killed |
|---|---|
| drop the `doc` name, keep the legacy ones — *the exact regression* | 4 |
| every `doc` action counts as a write | 2 |
| drop `augment` from the action set | 1 |
| drop `run_command` from the shell branch | 2 |
| shell branch attributes every path-shaped token it mentions | 7 |
| revert the probe fix, run the Rust gate | 1 (names the consequence) |
| unmarked retired name in the *suite* | 1 |

**Two of these assertions were vacuous when first written, and the mutation run is the only
reason that is known.** The `doc(get)/(find)` control used an id absent from the catalog, so
under *"count every `doc` call"* the lookup still resolved nothing and it still passed — it
could not reach the failing value. The `doc(augment)` case asserted against a path that B's
`append_entry` had already written, so an aggregate satisfied it whether or not `augment`
counted. Both now hold their own artifact id and path, annotated on the fixture line.

## Workarounds

None needed after the fix. Before it, the only reliable path→session route for a librarian
write was the socket enumeration plus a direct question to the peer.

## Resume

Nothing outstanding. `df517af91b43a5f7` stays **open** for its wiring half, with two measured
constraints for whoever takes it: the scan costs **5.0s** and does not cache, so it can only run
on a detected red; and native `Bash` bypasses `run_command` entirely, so a `run_command`-hosted
hint has a reachability ceiling worth stating at the site.

## References

- `docs/issues/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md`
  (`df517af91b43a5f7`) — the incident that sent me here; its option 3 remains unbuilt.
- `docs/issues/2026-09-07-file-provenance-reads-bash-but-not-codescouts-own-shell.md`
  (`2b9cbd34630cf340`) — the sibling, fixed in the same commit.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- `docs/PROBES.md` — vouched for the dead route; corrected here.
