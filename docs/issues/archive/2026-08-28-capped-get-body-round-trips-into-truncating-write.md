---
status: fixed
opened: 2026-08-28
closed: 2026-08-29
severity: high
owner: marius
related:
  - docs/trackers/resume-cross-machine-catalog-restore.md
  - docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md
tags:
  - librarian
  - data-loss
  - progressive-disclosure
kind: bug
---

# BUG: a capped `artifact get --full` body round-trips into a truncating `artifact update --body`, and the shrink guard cannot see it

## Summary

`artifact get --full` caps the body it returns at 500 lines. Feeding that body
back to `artifact update --body` silently deletes everything past the cap. The
byte-only shrink guard does not fire, because a truncated *prefix* of a
line-heavy document can retain most of the bytes while losing most of the lines.
Cost this session: 1047 of 1553 lines of `docs/trackers/prompt-hamsa-audit-log.md`
deleted by a write that reported `{"updated": true}`.

## Symptom (Effect)

```
$ codescout artifact get 59ebeebb6ed05c89 --full --json > /tmp/h.json
$ jq -r '.body' /tmp/h.json > /tmp/body.md
$ wc -l /tmp/body.md
500 /tmp/body.md          # the file's body is 1553 lines

$ codescout artifact update 59ebeebb6ed05c89 --body @/tmp/newbody.md
{ "id": "59ebeebb6ed05c89", "updated": true }

$ git show --stat --format= HEAD
 docs/trackers/prompt-hamsa-audit-log.md | 1051 +------------------
 1 file changed, 4 insertions(+), 1047 deletions(-)
```

No error, no warning, no non-zero exit. `updated: true` is the whole response.

## Reproduction

Commit `bc6eee3a` on `experiments`, any artifact whose body exceeds 500 lines.

1. `codescout artifact get <id> --full --json > /tmp/a.json`
2. `jq -r '.body' /tmp/a.json > /tmp/body.md` — observe fewer lines than the file
3. `codescout artifact update <id> --body @/tmp/body.md`
4. `git diff --stat` — everything past line 500 of the body is gone

Retains the failure with any edit spliced into step 2's output; the splice is
incidental, the truncation is in step 1.

## Environment

Linux, `experiments`, codescout v0.15.0, both the MCP tool surface and the
`codescout` CLI (`--full` shares `apply_soft_cap` with the MCP path).

## Root cause

Two independent halves, neither wrong on its own.

**Read side — a deliberate cap.** `apply_soft_cap`
(`src/librarian/tools/get.rs:72-85`) truncates at `SOFT_CAP_LINES = 500`
(`src/librarian/tools/get.rs:16`). `--full` opts out of *section-scoping*, not
out of the cap. This is by design and is pinned by a test asserting
`line_count should reflect lines in returned body, not full source`
(`src/librarian/tools/get.rs:1170-1177`).

**Write side — a guard that measures the wrong dimension.**
`src/librarian/tools/update.rs:570` refuses a body write only when
`new_content.len() * 2 < original.len()` — a **byte** ratio. Measured
2026-08-28 on the case above: 181,063 new bytes against 255,954 original =
**29% byte loss**, comfortably under the 50% threshold, so the guard correctly
declined to fire. The same write was a **68% line loss** (500 of 1553). The
divergence is not incidental: the capped prefix here is the artifact's Index
table, whose rows run 3–7 KB each, so the first third of the lines carries
two thirds of the bytes. Any document front-loaded with long lines inverts
the two ratios.

*Measured 2026-08-28*: `jq -c '.body_meta' probe.json` →
`{"line_count":499,"source_line_count":1553,"bytes":180729}`.

## Evidence

### The read side warns, three times over

The response is not silent about the cap. It carries all of:

```json
"body_meta": { "line_count": 499, "source_line_count": 1553 },
"overflow": {
  "shown_lines": 500,
  "total_lines": 1553,
  "hint": "Body exceeds soft cap (500 lines). Narrow with heading=\"<section>\" or start_line=N, end_line=M. ..."
}
```

So this is **not** a case of a tool lying. `jq -r '.body'` reads past every one
of those signals without touching them, and a shell pipeline has no reason to
look at sibling keys. The read side's contract is fine for a reader and unsafe
for a *pipeline*.

### Why the sibling bug did not cover this

`docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md` (CM-6, fixed
`5b7b82cc`) ported this same guard to `memory(write)`. Both guards share the
byte-ratio predicate, so both share this blind spot — the port was faithful,
and faithful to the gap.

## Hypotheses tried

1. **Hypothesis:** the writer mangled the body (re-serialisation bug).
   **Test:** `git show dde7491b:<path> | wc -l` → 1560, and the post-write file
   was 517. Compared `.body` from the read against the file directly.
   **Verdict:** rejected — the writer wrote exactly what it was given; the input
   was already short.
2. **Hypothesis:** `--full` bypasses the soft cap and this is a regression.
   **Test:** read `apply_soft_cap` and its call site; found the pinning test at
   `src/librarian/tools/get.rs:1170-1177`.
   **Verdict:** rejected — capping under `--full` is intended, tested behaviour.
3. **Hypothesis:** the shrink guard should have caught a 1047-line deletion.
   **Test:** computed both ratios from the real numbers.
   **Verdict:** confirmed as a gap, not a malfunction — 29% by bytes is under
   threshold; 68% by lines is over it. The guard measured the dimension that
   happened not to move.

## Fix

Fixed on `experiments` in `45a88531`, patch-id
`5bcb69c2d5f06b9126ea78c7e8cf2d640c097463`.

**Option (a) shipped, with a scope correction this file got wrong.** The plan
said two implementations; there are **three**. `edit_markdown`
(`src/tools/markdown/edit_markdown.rs`) carried a third private copy of the same
byte-only predicate and its own `SHRINK_GUARD_MIN_BYTES = 200`, and it had **no
shrink-guard test at all**. Fixing only the two named here would have left it
broken, which is the failure mode that let this bug exist in the first place.

The predicate, the floor and the report type therefore moved into a single
`crate::util::shrink_guard`, with all three surfaces calling it and each keeping
its own refusal text — `body_edits[]` for artifacts, `action='edit'` for
markdown, read-modify-write for memories. `check()` returns which dimension
tripped, and every message reports both, including the one that held: a reader
deciding whether to pass `force=true` needs to see that bytes were fine,
because that is the surprising part.

Option (c) shipped alongside it — `get_guide("librarian")` § *The shrink guard*,
the `force` schema descriptions on `artifact` and `edit_markdown`, and
`docs/architecture/augmented-artifacts.md`, whose "I have the body in hand"
section now names the capped-`full=true` case explicitly. All of it written
**shorter than what it replaced**: the guide section is drawn by every p50
session, and a first, more verbose draft failed
`a_p50_session_stays_under_the_committed_guide_byte_ceiling` at 12,308 B against
a 12,000 B ceiling with margin 0. That test states that raising the ceiling is a
spec amendment rather than a fix, so the section was compressed to net −70 B.

Option (b) — renaming the key to `body_partial` on a truncated read — was not
taken and is not owed. With the line arm in place the round-trip now fails loudly
at the write, which is where the caller can act on it.
## Tests added

Seven in `src/util/shrink_guard.rs`, one in `src/librarian/tools/update.rs`, two
in `src/tools/markdown/tests.rs`.

The reproduction was written first and observed **red** — `updated: true` on a
write dropping 90 of 100 lines — before any fix existed.

- `util::shrink_guard::tests::catches_a_line_truncation_that_keeps_the_bytes`
- `librarian::tools::update::tests::body_shrink_guard_catches_a_line_truncation_that_keeps_the_bytes`
- `tools::markdown::tests::shrink_guard_blocks_a_line_truncation_that_keeps_the_bytes`
- `tools::markdown::tests::shrink_guard_line_arm_yields_to_force` — the escape
  hatch on the new arm. A guard that over-fires with no way out is worse than
  the bug it closes.
- `util::shrink_guard::tests::a_single_line_document_relies_on_the_byte_arm` —
  pins that the line arm is *structurally unable* to fire on minified or
  single-paragraph content, so the byte arm must carry it.

**The fixture trap named in this file was real, and is now executable rather
than a comment.** Every line-arm fixture is built from lines of unequal length
and asserts that premise (`front.len() * 2 >= whole.len()`) before asserting the
behaviour, so a later edit that quietly makes the fixture uniform fails on the
premise instead of silently defanging the test.
`util::shrink_guard::tests::a_uniform_fixture_cannot_tell_the_arms_apart` exists
only to document the trap: it asserts that uniform lines yield
`ShrinkDimension::Both`, i.e. that such a fixture cannot distinguish the arms.

Clippy caught one defect in this very set: `assert_eq!(r.byte_pct, 100 - (4 *
100 / 600))` is `identity_op` — integer division floors the term to 0, so the
expression was a disguised `100`. Replaced with the literal and a note on why a
600→4 byte write reads as 100% rather than 99%.
## Workarounds

**Never build a write payload from a `get` response.** Rebuild it from the file
or from git:

```
git show <sha>:<path> > /tmp/f.md        # or: awk 'NR>=13' <path> > /tmp/body.md
# ... splice ...
codescout artifact update <id> --body @/tmp/body.md
```

This is what `docs/issues/archive/2026-08-28-duplicate-frontmatter-block-in-hamsa-log.md`
(CM-8) did, and why it was safe: its new body was an `awk` line-drop taken from
the **file**, never from `.body`.

If a `get` response must be used, check `body_meta.line_count` against
`body_meta.source_line_count` **before** the write, and treat the presence of an
`overflow` key as disqualifying.

After any full-body write, verify at the bytes: `git diff --stat <path>` should
show the insertions you intended and **zero** unexplained deletions.

## Resume

N/A — fixed and verified. Gate green on `45a88531`: `cargo fmt`, `cargo clippy
--workspace --all-targets --features local-embed -- -D warnings`, `cargo test`
(4637 passed, 0 failed), `cargo check --no-default-features`.
## References

- `src/librarian/tools/get.rs:16,72-85` — `SOFT_CAP_LINES`, `apply_soft_cap`
- `src/librarian/tools/get.rs:1170-1177` — the test pinning capped-under-`--full`
- `src/librarian/tools/update.rs:565-580` — the byte-only shrink guard
- `src/memory/mod.rs` — `shrink_check`, same predicate (CM-6)
- `docs/trackers/resume-cross-machine-catalog-restore.md` — CM-5, the write that hit this
- `get_guide("progressive-disclosure")` — the ≳9 KB round-trip guidance this sits beside
