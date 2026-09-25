---
id: fae16c0498d9c977
kind: bug
status: open
title: encode_units tokenises each sentence alone, dropping the leading-space token at every sentence start
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
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

Not applied. The phase-1 and Stage-4 results were produced with the current encoding and stay as registered. The fix belongs in the next registration's code: prepend `" "` to every unit after the first, before tokenising. That changes the inputs, so it cannot be applied to a registered arm retroactively.

## Tests added

None yet. The regression test for the fix should assert that, for a multi-unit text, each unit's ids after the first occur as a contiguous run in the tokenisation of the running text.

## Workarounds

None needed for the recorded results, which are internally consistent.

## Resume

Fix together with the phase-1b recipe changes; see `docs/evals/phase1b-local-classifier-preregistration.md`.

## References

- `docs/evals/data/2026-09-24-rule-tell/stage3/train_arm.py`, `encode_units`
- https://github.com/AnswerDotAI/ModernBERT/issues/149
