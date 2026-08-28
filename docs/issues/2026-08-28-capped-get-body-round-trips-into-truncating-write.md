---
status: open
opened: 2026-08-28
closed:
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

Not yet implemented. Preferred option is (a); it is the one that would have
caught this exact write.

**(a) Give the shrink guard a line-ratio arm.** In
`src/librarian/tools/update.rs:565-580`, fail when *either* ratio crosses the
threshold: `new.len()*2 < original.len() || new_lines*2 < original_lines`.
Cheap, symmetric with the existing predicate, and generalises past this
round-trip to any line-dropping write. Port to `MemoryStore::shrink_check`
(`src/memory/mod.rs`) in the same change — CM-6 gave it the identical
byte-only predicate. Report whichever dimension tripped, so the message names
what was actually lost.

**(b) Make the capped body unusable as a write payload.** When `apply_soft_cap`
truncates, `get` could rename the key (`body_partial`) or drop `body` in favour
of the overflow envelope. Stronger, and it breaks every existing reader of
`.body` on a >500-line artifact — a real cost, since the current shape is
useful for reading.

**(c) Documentation only.** Note the hazard in `get_guide("librarian")` beside
the `--params @<file>` guidance. Weakest: the failure mode is a pipeline that
never reads prose.

(a) and (c) compose. (b) should not ship without (a) anyway.

## Tests added

None yet — the fix is not written. A regression test for (a) is straightforward
and should assert the guard fires on a **line**-heavy truncation whose byte
ratio stays under threshold, i.e. it must be built from a body with long lines.
A test using uniform-length lines would pass with the bug present, because both
ratios move together — that fixture shape is the trap here.

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

Implement fix (a) in `src/librarian/tools/update.rs:565-580` and mirror it in
`MemoryStore::shrink_check` (`src/memory/mod.rs`). Write the regression test
first, with a long-line fixture as described under *Tests added* — verify it
fails before the fix by checking the byte ratio stays above 50% while the line
ratio drops below it. Gate: `cargo fmt`, `cargo clippy --workspace --all-targets
--features local-embed -- -D warnings`, `cargo test`, `cargo check
--no-default-features`.

## References

- `src/librarian/tools/get.rs:16,72-85` — `SOFT_CAP_LINES`, `apply_soft_cap`
- `src/librarian/tools/get.rs:1170-1177` — the test pinning capped-under-`--full`
- `src/librarian/tools/update.rs:565-580` — the byte-only shrink guard
- `src/memory/mod.rs` — `shrink_check`, same predicate (CM-6)
- `docs/trackers/resume-cross-machine-catalog-restore.md` — CM-5, the write that hit this
- `get_guide("progressive-disclosure")` — the ≳9 KB round-trip guidance this sits beside
