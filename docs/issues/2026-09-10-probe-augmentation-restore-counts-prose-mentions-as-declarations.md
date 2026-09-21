---
id: '9a1998aaf4b09801'
kind: bug
status: fixed
title: 'BUG: probe_augmentation_restore counted a fenced yaml example as a declaration, anchoring on a 4000-byte prefix instead of the frontmatter block'
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-09-10
owner: marius
related:
- docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md
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

**Correction, 2026-09-20 — the headline `47` is not reproducible, and the ceiling says it never was.** The script has exactly two commits (`9bfc7e4b` creation, `869af927` this fix), so the code that printed `declared sidecars : 47` on 2026-09-10 is byte-identical to the code that prints `24` today. The corpus is not the difference either: counted at `138598782ec477d1` (the last commit before 2026-09-11) and at HEAD, the line-anchored non-archive population is **25 files at both instants**. `find_declarers` `break`s after the first matching line, so it yields **at most one pair per file**, and the probe excludes `/archive/` — making 25 a hard ceiling on `declared` at that tree. 47 is above it.

So the number did not come from the tracked tree. `find_declarers` walks `repo.rglob("*.md")` — the **working tree**, including untracked and ignored files — which is exactly the worktree-vs-HEAD ambiguity `CLAUDE.md` § *Testing Discipline* names when it says a count needs its **tree** as well as its instant: *"a sweep in flight makes the worktree and HEAD disagree, so two sessions reading correctly at the same instant still differ."* This file gave the instant and an approximate commit (`47b818b8`+) and not the tree, and that is the one component that would have made it re-checkable. The defect below is real and was demonstrated on a fixture; only this number is withdrawn.

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

**Correction, 2026-09-20 — the mechanism above is stated one level too coarse.** *"Its selector cannot separate them"* is right; *"counts every occurrence of the string"* is not. `find_declarers` has used `line.startswith(DECL)` plus a `.yaml`/`.yml` suffix test since its first commit — it was **already structurally anchored**, which is why the prescribed remedy below reads as already-satisfied and why a reader could have closed this file as a non-bug.

The real defect is narrower and survives that anchoring: the scan ran over `p.read_text()[:4000]` — **line-start within an arbitrary 4000-character prefix**, not line-start *within the frontmatter block*. A fenced ```yaml documentation example begins at column 0 exactly like real frontmatter, so any such example falling inside a file's first 4000 characters is counted as a declaration. The instance is live in this corpus: `src/prompts/guides/tracker-conventions.md:471` holds `expects_augmentation: docs/augmentations/docs-trackers-tool-usage-patterns.yaml` as a body example, and escapes being counted today only because that file is 56 KB and line 471 sits past the cutoff. It is one edit — or one earlier example — away from counting.

That makes the class exactly as filed (`IC-6`, a parser over a namespace with no escape for *mention*), and makes the fix the same prescription taken one level further: anchor on the **frontmatter block**, which no body example can be inside, rather than on a byte window that a body example can drift into.

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

**Implemented 2026-09-20.** Added `frontmatter_block(text)` — the text strictly between a file's opening `---` and the next `---`, or `""` when there is none — and pointed `find_declarers` at that block instead of `head[:4000]`. The prescription above is honoured and sharpened: a body example is now unreachable by construction rather than by being far enough down the file. **No path exclusion list was used**, per this section's own prohibition.

The real-corpus count is **unchanged at 24 before and after** — this is a latent-defect fix, and the fixture is the only place the defect is observable today. That is stated rather than glossed: a reader comparing probe output across this commit will see no difference and should not read that as the fix doing nothing.

## Fix provenance

- **SHA:** `869af927` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `617bc032361efd88d877c9d48aa33bbaec6bba11` — content hash of the diff; survives rebase and cherry-pick. Derived through a file (`git show <sha> > f; git patch-id --stable < f`), never a pipe: the command buffer is capped and a hash of a truncated prefix is a valid-looking WRONG digest.

## Tests added

None. A regression test is cheap and specific: a fixture corpus containing one real
declaration plus one file that merely *mentions* `expects_augmentation` must report
`declared == 1`. That fixture is the whole bug.

**Added 2026-09-20:** `tests/test_probe_augmentation_restore.py` — the two-file fixture this section specified. One real frontmatter declaration, plus one file whose ONLY occurrence of the key is inside a fenced ```yaml body example; asserts `find_declarers` returns exactly 1. The mention file carries an inline annotation on its own fixture line saying what breaks if it is deleted — without it the test passes whether or not the fix exists, which is the monotone-under-removal trap this repo files repeatedly.

**Red observed twice, by two parties, and the second did not trust the first.** The implementing agent reported a `git stash` red (`AssertionError: 1 != 2`). That was then re-derived independently by loading the pre-fix and post-fix `find_declarers` side by side against one freshly-built fixture in a scratch directory — `PRE declared=2 -> ['mentions-only.md', 'real-tracker.md']`, `POST declared=1 -> ['real-tracker.md']`. A hand-back is model output, not evidence; the second run is what makes this a measurement.

## Workarounds

Read the probe's **body**, not its exit code: `restored == artifacts indexed`,
`restore errors: 0`, and `distinct prompts N of N` are the meaningful discriminators and
were all green here. The exit code is currently the least reliable line of its output.

## Resume

Open. Found incidentally while verifying a sidecar export
(`doctor --fix=export_augmentations --confirm`), which the probe confirms restores
correctly — so this bug blocks nothing and is filed on notice rather than as a blocker.
