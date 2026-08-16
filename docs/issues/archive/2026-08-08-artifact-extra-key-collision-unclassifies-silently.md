---
id: '27c69239416f3667'
kind: bug
status: fixed
title: 'BUG: artifact `extra` writes a key that collides with a canonical frontmatter key, and the duplicate silently drops the artifact out of its own kind'
tags:
- librarian
- artifact
- frontmatter
- silent-failure
- bookkeeping
closed: 2026-08-08
opened: 2026-08-08
owner: marius
related: []
severity: high
---

# BUG: artifact `extra` writes a key that collides with a canonical frontmatter key, and the duplicate silently drops the artifact out of its own kind

## Summary

`artifact(create|update, extra={…})` writes each `extra` key verbatim into the YAML
frontmatter with no check against the canonical keys the librarian itself emits (`id`,
`kind`, `status`, `title`, `owners`, `tags`, `topic`, `time_scope`). Passing
`extra={"kind": "bug"}` therefore produces **two `kind:` lines in one mapping**. YAML
rejects the duplicate, the whole frontmatter block fails to parse, and the librarian falls
back to path-glob classification — silently. The artifact keeps its file, its id, and its
content, and stops being a bug.

Same consequence as `docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`
(`d61e3b535e2cae4a`, fixed): a correctly-authored, committed, pushed bug file never appears
in the answer to *"what's open?"*, and no gate notices. Different mechanism, same family —
which is why this one is filed rather than folded into that one.

## Symptom (Effect)

`docs/issues/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md` was created
2026-08-08 with correct frontmatter. Hours later the canonical triage query returned 11
open bugs and **this file was not among them**. `artifact(action="get")` on its id showed
why:

```
"kind":   "tracker",     <- file says bug
"status": "active",      <- file says open
"title":  null,          <- file has a title
"extra":  <absent>
```

and the body began with the frontmatter itself:

```
"body": "---\nid: a99388a299352d21\nkind: bug\nstatus: open\ntitle: edit_file's out-of-project ack handle…"
```

The frontmatter was being served as body — the parse had failed and nothing said so.
`librarian(action="reindex")` reported `unknown_count: 0` for it: not flagged as
unclassifiable, **confidently misclassified**.

## Reproduction

```
artifact(action="create", kind="bug", rel_path="docs/issues/x.md",
         title="…", extra={"kind": "bug", "opened": "2026-08-08"})
librarian(action="reindex")
artifact(action="find", kind="bug", filter={"status": {"in": ["open","investigating"]}})
# the new file is absent
```

The offending frontmatter, verbatim (note lines 3 and 13):

```yaml
---
id: a99388a299352d21
kind: bug
status: open
title: edit_file's out-of-project ack handle does not resolve …
owners:
- marius
tags:
- tooling
- write-guard
- ack
- misleading-error
kind: bug
opened: 2026-08-08
severity: low
---
```

Canonical keys are emitted first, then `extra` keys are appended — so a colliding `extra`
key always lands *after* the canonical one, in the same mapping.

## Environment

Linux, `experiments` at `0b9e7238`, codescout 0.15.0, live MCP server (release binary
rebuilt 2026-08-08 ~17:45).

## Root cause

`extra` is documented as *"custom frontmatter keys … Written verbatim to YAML and
round-trip-safe across updates"*. There is no collision check against the canonical key
set, and no validation that the emitted frontmatter re-parses.

**measured 2026-08-08:** removing the single duplicate `kind: bug` line and re-running
`librarian(action="reindex")` restored every field at once — `kind: bug`, `status: open`,
the full `title`, `owners: [marius]`, all four `tags`, and `extra: {opened, severity}` —
and the body moved back to starting at `## Summary`. One line, whole classification. That
is the experiment, not an inference from reading the parser.

**Do not infer the mechanism from a grep count.** `grep -c '^kind:'` over `docs/issues/`
returns 2 for this file *and* 2 for
`docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`,
which is classified correctly — its second match is inside a fenced example in the body.
The count does not distinguish "duplicate key in frontmatter" from "the word appears in
prose". Read the block.

## Evidence

### The scan that found the blast radius

`grep(pattern="^kind:", glob=["docs/issues/**/*.md","docs/trackers/**/*.md"], mode="files")`
→ 333 matches in 331 files. Exactly two files have 2; one is this defect, the other is the
false positive described above. So the corruption is **one file**, not a class — this time.

### Why it survived a full working day

The file reads correctly to a human, `git` shows nothing unusual, `audit_doc_refs` does not
look at frontmatter validity, and the canonical open-bug query returns a plausible-looking
list that is simply one shorter. There is no surface on which this looks wrong.

## Hypotheses tried

1. **Hypothesis:** duplicate `kind:` keys break the parse.
   **Test:** removed the second line, reindexed, re-fetched.
   **Verdict:** confirmed — every field returned in one step.
2. **Hypothesis:** any file with two `^kind:` matches is corrupted.
   **Test:** the archived sibling has two and classifies correctly.
   **Verdict:** rejected — the second match is body prose. The grep is a *finder*, not a
   test.

## Fix

**Implemented in `ca4b7f0d` (`experiments`).** Promotion to `master` is by fast-forward,
so this SHA *is* the master SHA — there is no second one to record later.

Two guards, at the two ends, because either alone
leaves a hole:

**1. Input boundary — refuse the collision.** `reject_reserved_extra_keys`
(`src/librarian/tools/create.rs`) returns a `RecoverableError` naming every clashing key
and pointing at the parameter that owns it. Called from `create` before the file is
written, and from `update`'s `call` before any of the three frontmatter-touching branches
— sited there rather than in `apply_frontmatter_patch`, which all three share and which is
infallible by design.

Refused rather than repaired, even when the values agree. Repair-and-continue is for input
whose intent is unambiguous; here the caller has said the same thing through two channels
and the right correction — drop the `extra` entry, or drop the typed parameter — is a
question about intent, not a typo. On `update` a `null` value is refused too: it would be
an RFC-7396 delete, which is a no-op for these keys, and answering a mistaken repair
attempt with an error that names the right parameter beats answering it with silence.

**2. Emitter — never write frontmatter that cannot be read back.**
`crate::librarian::frontmatter::write` now skips any `extra` key in `RESERVED_KEYS`. The
typed field above already carries that key with the catalog-indexed value, so dropping the
duplicate preserves meaning; emitting both destroys the whole block. This keeps `write`
infallible (it returns `String`, and panicking in a serializer is not an option) while
making the bad document unreachable from any caller, including internal ones that never
pass through guard 1.

`RESERVED_KEYS` and `reserved_keys_in_extra` live in `frontmatter.rs` beside the
`Frontmatter` struct, so adding a typed field without adding it to the list is a visible
omission rather than a silent re-opening.

**Not implemented: the `doctor` check.** Reporting artifacts whose on-disk frontmatter
fails to parse is still worth having — it would catch families these two guards do not
anticipate, and today a parse failure is indistinguishable from a genuinely unclassified
file in the reindex report. Left as its own change; with both guards in place nothing new
can reach that state through the tool surface.
## Tests added

Four, and the first is the reproduction:

- `a_reserved_key_in_extra_is_not_emitted_twice` (`src/librarian/frontmatter.rs`) — builds
  the exact shape (`extra` carrying `kind` alongside a legitimate custom key), writes, and
  **parses the result back**. The `kind:`-appears-once assertion alone would only say the
  symptom is gone; the round-trip says the document still means something. Also asserts
  the non-reserved key survives, and that `parse` routes `kind` to the typed field rather
  than to `extra`.
- `reserved_keys_in_extra_reports_every_clash_and_nothing_else` — the helper reports all
  clashes, in list order, and stays silent on a custom key.
- `create_rejects_an_extra_key_that_names_a_frontmatter_field` — refuses, names the clash,
  points at the right channel, **and leaves no file behind** for a later reindex to
  classify from a glob.
- `update_rejects_an_extra_key_that_names_a_frontmatter_field` — the other way in, with
  the file asserted byte-identical afterwards, plus the `null`-is-also-refused case.

**Discrimination measured, not assumed.** The emitter guard was disabled and the suite
re-run: `a_reserved_key_in_extra_is_not_emitted_twice` failed, printing the corruption
verbatim —

```
---
kind: bug
status: open
kind: bug
severity: low
---
```

— the same shape as the live casualty. Guard restored, suite green.

Gate: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 3574
passed / 0 failed / 44 ignored.
## Workarounds

Do not put canonical keys in `extra`. For bug files the canonical set is `kind`, `status`,
`title`, `owners`, `tags` — pass those as their own parameters. `extra` is for `opened`,
`closed`, `severity`, `owner`, `related`.

To detect an existing casualty: `artifact(action="get", id=…)` and check whether `body`
begins with `---`. If it does, the frontmatter is being served as body and the row is
misclassified.

## Resume

Fixed on `experiments`. Remaining:

1. **Confirm CI**, then archive via `artifact(action="move", …)` — never a bare `git mv`.
   No master-side SHA to record: this cohort promotes by **fast-forward**, so the
   `experiments` SHA is the master SHA (`docs/RELEASE.md` § *Large-Cohort Promotion*).
2. **Optional follow-up, not required to close this:** the `doctor` check described at the
   end of *Fix*. It is a different guarantee — detecting an already-corrupt file on disk,
   whatever wrote it — rather than more of the same one.

Before closing, re-run the scan in *Evidence*. It is cheap, and it is the only thing that
would surface a casualty created between the diagnosis and the fix.
## References

- `docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`
  — same family (bug file invisible to the canonical open-bug query), different mechanism,
  already fixed
- `docs/issues/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md`
  (`a99388a299352d21`) — the casualty, repaired in this same commit
- `get_guide("librarian")` § *artifact(action="create")* — where `extra` is specified
