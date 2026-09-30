---
id: a90c15852addaa41
kind: bug
status: archived
title: encode_units tokenises each sentence alone, dropping the leading-space token at every sentence start
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-09-30
---

# BUG: encode_units tokenises each sentence alone, so every sentence start loses its leading-space token

**Valid:** dated 2026-09-25

## Summary

`encode_units` in `docs/evals/data/2026-09-24-rule-tell/stage3/train_arm.py` tokenises each `segment()` unit on its own and concatenates the ids, with a marker token between units. A unit that starts a sentence therefore begins with the no-space token, `Nobody`, where the same word in running text is the leading-space token `ĠNobody`. Both local arms were trained, calibrated and gated on inputs in which every sentence after the first opens with a token form that does not occur mid-text.

## Symptom (Effect)

Nothing fails. Train and inference share the encoding, so train/test parity holds, and every check aimed at the tokens passes. The effect is an input distribution shift of unmeasured size on both backbones.

## Reproduction

Run from the repo root, CPU only:

    import train_arm as ta; from transformers import AutoTokenizer
    tok = AutoTokenizer.from_pretrained(ta.ARMS["qwen"]["model"])   # or "mbert"
    alone  = tok("Nobody reads the manifest.", add_special_tokens=False)["input_ids"]
    spaced = tok(" Nobody reads the manifest.", add_special_tokens=False)["input_ids"]
    # alone starts 'Nobody'; spaced starts 'ĠNobody'.

Measured 2026-09-25 for both `answerdotai/ModernBERT-large` and `alibiserikbay/JevK5`: the `alone` ids occur as a contiguous run in `tok(" ".join(units))` **False**; the `spaced` ids **True**.

## Environment

transformers 5.17.0; both arms' tokenizers; any text with two or more units.

## Root cause

`encode_units` calls `tok(u, add_special_tokens=False)` per unit. BPE tokenizers of this family mark a word boundary with a leading `Ġ` on the word, so tokenising a unit alone drops that boundary for its first word.

## Evidence

Reported by the ModernBERT research subagent (2026-09-25), which cites ModernBERT maintainers on the tokenizer's leading-space convention and a NER user who gained about 1 point from a prefix-space fix (AnswerDotAI/ModernBERT issue #149). Verified by the reproduction above, for both tokenizers.

## Hypotheses tried

None; the mechanism is direct.

## Fix

**Fixed for new recipes; the registered `phase1` recipe is deliberately unchanged.** `encode_units` and `chunks` take `space_fix`; with it, every unit after the draft's first is tokenised as `" " + unit`, the form it has in running text. `encode_units` takes `first_index` so a window that starts mid-draft decides the space by the unit's *draft* index, not its position in the window, and `chunks` measures window length on the same spaced text it encodes. The recipes `s1-r1` and `s1-r2` set `space_fix=True`; `phase1` keeps `space_fix=False`, so the registered phase-1 and Stage-4 results still reproduce and are not retroactively re-encoded.

## Tests added

`EncodeUnitsLeadingSpace` in `tests/test_phase1b_training.py` (run: `python tests/test_phase1b_training.py` under the jevk5 venv; the file uses `unittest`, not pytest). Six tests: with `space_fix` every unit's ids occur as a contiguous run in the running-text tokenisation; without it a later unit does not (the reproduction, so the test discriminates); the first unit is identical either way; a window decides the space by draft index, not window position; `test_chunks_measure_the_same_text_they_encode` asserts the exact window structure `[(0,4),(2,3),(3,3)]` at `max_len` 21; and a draft that fits whole is encoded spaced too.

Mutation-tested once per site, eight mutations, all killed on the committed file: in `encode_units`, space never added / added to the first unit too / window index used in place of the draft index / space added when `space_fix` is off; in `chunks`, the length helper measuring unspaced text / spacing the first unit, the window call ignoring the draft index, and `space_fix` not passed to the whole-draft encoding. The first draft of the tests left two sites alive; the exact-structure test and the fits-whole test were added for them.

## Workarounds

None needed for the recorded results, which are internally consistent.

## Resume

Closed. The remaining question is not this bug's: whether `s1-r1`/`s1-r2` change the classifier's counts relative to `phase1` is what the phase-1b Stage 1 registration measures (`docs/evals/phase1b-local-classifier-preregistration.md`).

## References

- `docs/evals/data/2026-09-24-rule-tell/stage3/train_arm.py`, `encode_units`
- https://github.com/AnswerDotAI/ModernBERT/issues/149


## Fix provenance

- **Fix SHA:** `244269216f50cde2daf792eca7827ab8f3926585` (`experiments`), `--recipe` with `space_fix` in `train_arm.py`
- **Fix patch-id:** `ccf50df455db40a6099b663c48833c37e0e977bf`
- **Tests SHA:** `00006e75db22454afe64d18102d46833712a249a` (`experiments`)
- **Tests patch-id:** `787d0e30219d9872050a3a963d31d5772b98a806`
- **Gate:** 23 tests pass under `python tests/test_phase1b_training.py` (jevk5 venv). Python-only change; no Rust lane is affected.
