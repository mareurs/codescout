---
id: ded9143998081efc
kind: bug
status: open
title: 'BUG: PROBES.md offers inspection as co-equal to a behavioural probe, and every inspection instrument''s miss proves nothing'
tags:
- cluster/unclassified
---

## Summary

[`docs/PROBES.md`](../PROBES.md):180 answers *"is my change in this binary?"* with two instruments
presented as co-equal:

> To settle "is my change in this binary", probe a **behaviour** or `strings` the binary; never the
> metadata

They are not co-equal. A **behavioural** probe answers in both directions. A `strings` search
answers in **one**: a hit proves presence, a miss proves nothing, because the scan cannot be shown
to see every literal the binary holds. The row names no blind spot for the second instrument, in a
file whose stated premise is that each row names the blind spot that would make you mis-trust its
output.

That cell is otherwise the most carefully caveated in the file — it already retracts `codescout
version`'s `git_sha` twice over, including the sharp case where a binary contains source in no
commit. The `strings` half sits beside those uncaveated, which is the shape worth recording: **a
correct, hard-won warning about instrument A reads as though instrument B beside it was checked
too.**

**AND THE DEFECT IS THE CLASS, NOT `strings` — THE ROW WOULD STILL BE WRONG WITH `strings`
DELETED.** A reader who correctly distrusts `strings` reaches for the next plausible inspection:
`ldd`, `nm`, `objdump`, `readelf`. Every one is one-directional for the same reason — **absence is a
property of the SCAN, not of the binary.** Measured below as a third instance, on `ldd`. So the
remedy is not to swap the instrument named; it is to state that the whole inspection family answers
only in the affirmative, and that the behavioural probe is the one that answers both ways. (Raised
by sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f` on auditing this file; verified here.)

## Symptom (Effect)

A reader follows the documented probe, gets `0`, and concludes the change is absent. The conclusion
may be right — it was, on 2026-09-15, twice, by two sessions independently — but it is not
*supported*, and a binary that **does** carry the change can return the same `0`. Silent, and
returns a plausible number rather than an error.

## Reproduction

Measured 2026-09-15, two binaries, same control string (`LIBRARIAN_ARTIFACT_VEC_MIGRATE` — present
in source **both** before and after the PR, which is what makes it a control rather than a probe):

| binary | size | `strings \| grep -c` | truth |
|---|---|---|---|
| pre-rebuild, inode `200046579` | 65,061,280 | **0** | present in source |
| post-rebuild, inode `200141988` | 42,560,784 | **1** | present in source |

**So the blindness is per-BINARY, not a property of `strings` over Rust release binaries.** The
first reading is `embedder-stack-ops-session-log:F-6`'s (sessionId
`f5f48b42-6d84-482e-84a4-8eaebb0ce60f`), whose original reason — *"Rust merges string literals into
large `.rodata` blobs, so a line-oriented search over a release binary cannot express the question"*
— predicts `0` on the second binary, where the answer is `1`. **They have since verified the second
reading independently and retracted that sentence in `00be0fcf`**, quoting it so a reader knows
which claim was withdrawn.

**The invocation is not the variable.** Same session's control: `strings`, `strings -a` and
`strings -n 6` all agree on the current binary, `-a` and the default emitting the same 372,081
lines.

## Environment

codescout `experiments` at `b4660a99`; `target/release/codescout` rebuilt 14:54:32 by
`cargo rb` (= `build --release --features server-stack`, carrying no `local-embed`).

## Root cause

**Not established, and now partly unestablishable.** Two explanations stay live and cannot be
separated: *that build merged the literals differently*, or *the original probe was flawed in a way
no longer reconstructable*. `f5f48b42` recorded it as unattributable rather than taking the
flattering branch, and the reason it cannot be settled is the sharper half of this bug:

**THE ARTEFACT THAT PRODUCED THE `0` IS GONE.** The 65 MB binary was overwritten by the 14:54
rebuild; only the 42 MB copy exists. So a past `0` is not merely uninterpretable, it is
**unauditable** — and that is the normal case, not bad luck, because the object a probe was run
against is routinely destroyed by the next build. An instrument whose negative cannot be re-examined
after the fact needs its caveat *at the recommendation site*; there is no later moment at which a
reader can recover the answer.

## Evidence

Both sessions reached the **correct verdict by an unsound route**, which is better evidence that the
route is at fault than either session's reasoning alone:

- `f5f48b42` probed `'DROP TABLE IF EXISTS artifact_vec'` → `0`, ran the control that showed the
  method could not support it, and switched to a behavioural probe — schema version written to a
  scratch catalog: `12` + creates the v1 table ⇒ pre-v13 binary. One command, discriminates both
  ways.
- This session (`f0b1a4c7-e991-4478-bf22-b088483b6821`) probed `'rebuilding artifact_vec_v2 at new
  dimension'` → `0` and published *"settled at the bytes"*. Two controls were run and **both were
  the wrong kind of control**: they established those *probe strings* were non-discriminating
  (`artifact_vec` survives the v1 retirement — an older peer re-creates it empty, and the live
  catalog shows exactly that: table present, `artifact_vec_rowids = 0`, v2 at 61,489 vectors; and
  `artifact_vec_cascade_delete` predates the PR) rather than establishing that `strings` can see
  this class of literal **at all**.

**That distinction is the whole finding: a control that rules out your probe STRING is not a control
that validates your INSTRUMENT.** Both of this session's controls were sound for the question they
asked; neither asked the question that mattered.

**THIRD INSTANCE — `ldd`, one step after the `strings` failure, and it widens this bug from one
instrument to the family.** `f5f48b42` asked whether the ONNX runtime was actually LINKED, ran
`ldd`, read `no onnxruntime in the link map`, and concluded `local-embed` had left the build.
**`ldd` cannot answer that question on this crate.** Verified here at the bytes:
`crates/codescout-embed/Cargo.toml`:23 declares `local-embed = ["dep:fastembed",
"fastembed/ort-download-binaries-native-tls", …]` — a **static** prebuilt runtime, and the file's
own comment at `:18` says so in those words. The dynamic variant is a **separate** feature,
`local-embed-dynamic` → `fastembed/ort-load-dynamic`, needed on windows-gnu where `ort` ships no
prebuilt. A statically linked runtime appears in no link map, so `ldd` returns the identical empty
result with the feature in **or** out — confirmed on the current binary:
`ldd target/release/codescout | grep -c onnxruntime` → **0**, which is what a static-linked runtime
and an absent one both give. The conclusion survived only on the 62 → 40 MB size drop: right answer,
wrong instrument, rescued by a second one. Corrected by its author in `552a3c71`.

**And how that instance was caught is itself evidence for the remedy.** Not by a control, and not by
its author re-reading their own entry — they had revised `F-6` twice and read past it both times. It
surfaced only when they audited **this file**, a different artifact with a different framing. Two
revisions of a lessons-learned record by the person who wrote it did not surface the defect that
record describes. That is the argument for the caveat living at the recommendation site rather than
in a log, made by the log's own author against their own record.

## Hypotheses tried

`IC-20` (`floor-published-under-the-name-of-a-total`) is the near-miss and is **rejected by its own
falsification test** — *"Falsified by a member whose true total was recoverable — that is an ordinary
reporting bug, fixed by reporting it, and this class claims the value is gone."* Here the truth was
recoverable and was recovered, by the behavioural probe. `IC-20` requires the walk to have
**stopped** with the value unknowable; `strings` does not stop, it under-collects, and a different
instrument answers.

Recorded because the fit is tempting at the claim line and fails at the test. Note also what this
instance is **not** evidence about: `IC-20` carries an open question on *"whether recoverable by a
sibling code path with a cost the sibling doesn't pay should count as recoverable"*. The sibling
here answers a **different question** (does the binary have the behaviour), not the same statistic
by another route, so this settles nothing either way on that boundary.

## Fix

Not designed, and the cheap half needs no diagnosis: **stop presenting the two as alternatives.**
Name the asymmetry at the recommendation site — a hit is sound, a miss is uninterpretable and
later unauditable — and make the behavioural probe *the* instrument, with `strings` demoted to
corroboration.

Do **not** close this by deleting `strings` from the row. A positive hit is genuinely sound and
cheap, and it found real drift before — the `git_sha`-vs-`params_status_drift` instance that same
cell records was caught by exactly this rule. The defect is the missing asymmetry, not the
instrument — **and the `ldd` instance proves deletion could not work anyway**: remove `strings` and
the next reader reaches for `ldd`, `nm` or `readelf` and inherits the identical one-directionality.
Name the property of the FAMILY, not a caveat on one member.

## Resume

One build flag at a time on a fixed source tree — `--features server-stack` vs `local-embed`, then
`lto`, then `codegen-units` — checking whether the control string survives `strings` in each. **Keep
each binary**; the reason this bug cannot be closed from the existing evidence is that the
interesting one was overwritten. If a flag is the differentiator, the caveat can name the *regime*
rather than only the direction, which is strictly better.

## References

- `docs/PROBES.md`:180 — the row recommending both instruments; unchanged as of `b4660a99`, and
  `00be0fcf` did not touch it
- `docs/trackers/embedder-stack-ops-session-log.md` `F-6` (`3cbaf75df04686dd`) — the incident, the
  first measurement, the retraction, and the `scripts/rb.sh` seam. **This file deliberately does not
  re-file that**; it is about the PROBES.md recommendation, which `F-6` does not own
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the principle the row violates
- `docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md` — weighed, rejected
