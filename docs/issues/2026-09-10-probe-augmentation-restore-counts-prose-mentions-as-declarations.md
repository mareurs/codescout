---
id: '9a1998aaf4b09801'
kind: bug
status: open
title: 'BUG: probe_augmentation_restore counts prose mentions as declarations, so documenting the mechanism fails its own probe'
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-09-10
owner: marius
related:
- docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md
severity: low
---

## Summary

`scripts/probe_augmentation_restore.py` exits **1** on a corpus where every artifact
restores perfectly. Its `declared sidecars` figure counts every occurrence of the string
`expects_augmentation` in the corpus, including the prose that *documents* the mechanism —
so the pass/fail compares a count of **mentions** against a count of **artifacts**, and
adding documentation makes the probe fail. `docs/PROBES.md`, which documents this probe, is
one of the files inflating its own denominator.

## Symptom (Effect)

Measured 2026-09-10 at `47b818b8`+ against `target/release/codescout`:

```
declared sidecars : 47
artifacts indexed : 24
restored          : 24
restore errors    : 0
distinct prompts  : 24 of 24
...
FAIL: 47 declared, 24 restored     exit=1
```

Every discriminator the probe's own documentation names as meaningful is **green**:
`restored == artifacts indexed`, `restore errors: 0`, and `distinct prompts 24 of 24` —
the last being the check `docs/PROBES.md` singles out because "the count cannot see a
collision". The only red is the headline, and it is derived from the polluted number.

## Reproduction

```
python3 scripts/probe_augmentation_restore.py --binary target/release/codescout ; echo $?
```

Then measure the two populations directly:

```
git grep -l '^expects_augmentation:' -- '*.md' | wc -l    # 26 — real, line-anchored YAML keys
git grep -l 'expects_augmentation'    -- '*.md' | wc -l    # 38 — anything mentioning the string
```

The 12-file difference is entirely documentation *about* the convention:
`docs/PROBES.md`, `docs/conventions/cross-machine-catalog-resume.md`,
`src/prompts/guides/librarian-runtime.md`, `.codescout/memories/gotchas.md`,
`docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md`,
`docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md`, four archived bug files
and two trackers. (The file counts above and the probe's occurrence count differ because a
file may mention the string more than once; the *populations* are what this bug is about,
not either total.)

## Environment

- codescout at `47b818b8`+, `target/release` built 2026-09-10 09:56 (`cargo rb`).
- `docs/PROBES.md` records the baseline as **9/9 restored, 9 distinct prompts** (2026-08-30).
  The probe passed then and fails now with a *larger* and *healthier* corpus — the corpus
  did not regress, the documentation grew.

## Root cause

The denominator is a **grep over a namespace with no escape for mention**. A declaration is
`expects_augmentation:` as a line-anchored YAML key in frontmatter; a description is the same
string in a sentence, a fenced example, or a bug file recounting a past incident. The probe's
selector cannot separate them, so the two populations are summed into one number and compared
against a population containing only the first.

This is the failure mode `CLAUDE.md` § *Parsers Over a Namespace* names — *"a documentation
example of citation syntax counted as a real citation"* — with the parser one level out, over
a config key rather than an entry id.

**It is self-referential, which is what makes it worth a file rather than a one-line fix.**
Documenting a mechanism degrades the instrument that measures it, so the probe's health is
inversely coupled to how well the mechanism is explained. Nothing warns about this: the
probe's own page is one of the twelve.

## Evidence

- Probe output above, run against the live release binary.
- The 26/38 split, with the 12 extras enumerated and every one confirmed to be prose.
- `librarian(action="doctor")` reports `augmentation_declared_but_absent: 0` on the same
  corpus at the same instant. **The two instruments disagree because they ask different
  questions** — `doctor` checks declarations against *this* catalog, which holds the rows;
  the probe checks restore into a *fresh* catalog. `doctor`'s zero is not corroboration for
  the probe and does not falsify it either; it is the control showing no artifact is
  declared-but-absent, which is precisely why the probe's 23-artifact shortfall cannot be
  real.

## Hypotheses tried

1. **"My export broke it"** — falsified. The export added one artifact and moved
   `restored` 23 → 24; the FAIL predates it and its own row restores cleanly
   (`system-retrospective-improvements.md  tasks  params={}  tmpl=yes  prompt=343`).
2. **"23 artifacts declare `expects_augmentation: true` with no sidecar"** — falsified. That
   form is valid and would be a real finding, but the line-anchored count is 26 against 24
   sidecars, not 47.

## Fix

Not attempted here; filed on notice. The shape is to count **line-anchored frontmatter keys**
rather than string occurrences — the same structural-anchoring rule
`get_guide("tracker-conventions")` § *Detecting these fields* already states for
`**Status:**` and `**Valid:**`: *"Anchor detection on structure — line-start, a key prefix —
never on a keyword."* The rule exists; this instrument predates or ignores it.

**Do not fix by excluding a path list** (`docs/`, `*.md` under archive, …). That is a
selector narrower than its population wearing a different hat, and it breaks the next time
someone documents the convention somewhere new — which is the behaviour that produced this.

## Tests added

None. A regression test is cheap and specific: a fixture corpus containing one real
declaration plus one file that merely *mentions* `expects_augmentation` must report
`declared == 1`. That fixture is the whole bug.

## Workarounds

Read the probe's **body**, not its exit code: `restored == artifacts indexed`,
`restore errors: 0`, and `distinct prompts N of N` are the meaningful discriminators and
were all green here. The exit code is currently the least reliable line of its output.

## Resume

Open. Found incidentally while verifying a sidecar export
(`doctor --fix=export_augmentations --confirm`), which the probe confirms restores
correctly — so this bug blocks nothing and is filed on notice rather than as a blocker.
