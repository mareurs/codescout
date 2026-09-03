---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: medium
unverified: which commit narrowed the allowance is inferred from history, not measured by running the pre-fold code
---

# BUG: the librarian guard refuses `edit_file`'s text grammar on a stamped artifact while its own hint promises that call works

## Summary

`src/util/librarian_guard.rs:207` tells the caller:

> Reads and BODY edits are allowed directly — read_file, and edit_file without its
> `frontmatter` param, both work on this file.

`edit_file` **without** a `frontmatter` param, using the text grammar
(`old_string`/`new_string`), is refused on a stamped artifact — with that exact message. Only
the markdown grammar (`heading` + `action`) actually works. The hint names the wrong condition:
it says the discriminator is the `frontmatter` param, and the real discriminator is which
grammar the call uses.

## Symptom (Effect)

Both calls below target the body of the same file and pass no `frontmatter` param. The first is
refused, the second succeeds.

```
edit_file(path="docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md",
          old_string="- `src/librarian/tools/artifact/append_entry.rs` — …",
          new_string="- `src/librarian/tools/append_entry.rs` — …")
```
```
'…' is a librarian-managed artifact (stamped — it carries a librarian `id:`, so its
frontmatter is catalog-indexed and a direct frontmatter edit would not reach the catalog)
— do not edit its frontmatter directly

hint: … Reads and BODY edits are allowed directly — read_file, and edit_file without its
`frontmatter` param, both work on this file.
```
```
edit_file(path=<same>, heading="## Sites (initial)", action="edit",
          old_string=<same>, new_string=<same>)
→ {"status": "ok"}
```

Every clause of the refusal is about frontmatter. The call contained none.

## Reproduction

Verified 2026-09-03 at `ed8843cf`, and the refusal is **categorical on grammar, not on
content** — a text-grammar call whose `old_string` cannot match anything in the file is refused
with the identical frontmatter message rather than a not-found error:

```
edit_file(path="docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md",
          old_string="THIS_STRING_DOES_NOT_EXIST_IN_THE_FILE_12345",
          new_string="ZZZ_NEVER")
→ same frontmatter refusal
```

That is the decisive probe: the guard runs before content matching, so no property of the
edit's target can be what triggers it.

## Environment

Linux, codescout `experiments` `ed8843cf`, MCP transport.

## Root cause

**Not established at the source** — `src/util/librarian_guard.rs` has not been read past the
message at `:207`, and the frontmatter is marked `unverified:` accordingly.

What is established: the message was introduced by `c26943b5` (2026-09-01), whose subject is
*"fix(guard): the stamped arm refuses frontmatter writes only — reads and body edits are
safe"*. So the sentence was an accurate statement of intent when written, and the behaviour
has since narrowed underneath it. That makes this `IC-11` in its literal form — a document
stating a behaviour the code contradicts, true when written, the capability lost later.

**Best lead for where it narrowed:** `af974c0a` (2026-09-03), *"feat(edit_file): markdown edits
by heading, folding `edit_markdown` in; one grammar per batch"*. That fold gave `edit_file` two
grammars over markdown, and the guard predates the split — so a check written when there was
one body-edit path plausibly now gates only the path that did not move. **Inferred from commit
subjects and the runtime probe, not measured** — nobody has run the pre-fold binary.

## Evidence

### The hint is load-bearing, not decoration

Iron Law 5 already routes markdown edits to the heading grammar, so an agent following it never
meets this. But the hint exists precisely for the agent who did not, and it tells them the one
thing that will not work while naming a condition (`frontmatter` param) they have already
satisfied. The cost is a caller who reads the refusal, confirms their call meets its stated
requirement, and has no next move the message suggests — which is what happened here before the
heading form was tried.

### It is silent in exactly the direction that hides it

The refusal is loud, so nothing is corrupted and no data is at risk. What it costs is a caller's
turn plus the credibility of the message — and because the markdown grammar does work, anyone
who reaches for the heading form first will never see this. The observer who meets the bug is
the one least equipped to tell whether the guard or their call is wrong.

## Hypotheses tried

1. **Hypothesis:** the refusal is about the edit's content or target region.
   **Test:** text-grammar call with a deliberately unmatchable `old_string`.
   **Verdict:** rejected — identical refusal, so the guard precedes content matching.

2. **Hypothesis:** the call implicitly carried a `frontmatter` param.
   **Test:** re-read the two calls above; neither names one.
   **Verdict:** rejected.

3. **Hypothesis:** the message was always wrong (an authoring error), which would put this
   outside `IC-11` — whose claim explicitly excludes a statement that was never true.
   **Test:** `git log -S 'both work on this file'`.
   **Verdict:** rejected. `c26943b5`'s subject states the intent the message describes, so it
   was true as written. This is drift, not a wrong statement — which is what makes `IC-11` the
   right class rather than the convenient one.

## Fix

Read `src/util/librarian_guard.rs` and establish which call shapes the stamped arm actually
refuses. Then **either**:

- restore the allowance, so a text-grammar body edit on a stamped artifact succeeds and the
  message becomes true again; **or**
- keep the restriction and rewrite the message to name the real discriminator — *"use the
  heading grammar (`heading` + `action`); the text grammar is not available on stamped
  artifacts"* — which is a documented limitation rather than a silent reinterpretation, per
  `CLAUDE.md` § *Parsers Over a Namespace*.

The second is likely correct if the restriction is deliberate (Iron Law 5 points that way), but
that is a design call and the message is wrong under either.

**Second observation, 2026-09-03 — the discriminator is now MEASURED, not inferred.** A later
session hit this independently and ran the A/B: two calls on
`docs/trackers/prompt-surface-compaction-session-log.md`, differing **only** in grammar and
neither passing `frontmatter`.

| call | params | result |
|---|---|---|
| A | `path`, `old_string`, `new_string` (identical, so nothing could be written) | **refused**, with the `:207` message |
| B | `path`, `heading`, `action="edit"`, `old_string`, `new_string` | `status: "ok"`, written |

So *"which grammar the call uses"* is established at the runtime boundary rather than read off
commit subjects. What stays unverified is unchanged and is the `unverified:` note's subject:
`src/util/librarian_guard.rs` has still not been read past `:207`, so **where** the narrowing
lives is inferred. Prefer the identical-`old_string` form above when re-probing — it exercises
the refusal without any possibility of a write.

A duplicate of this bug was filed the same day and deleted on discovery
(`librarian-guard-blames-frontmatter-for-a-body-only-edit`, tagged `IC-22`). The retained
reading is **`IC-11`**: the sentence was accurate when `c26943b5` wrote it, so this is a
capability lost under a standing document, not a hint composed without consulting its request.
The duplicate was filed because the filer did not run the open-bug triage query first — the
step `get_guide("project-activation-bootstrap")` § Phase 0 exists for.

SHA: *pending.* patch-id: *pending.*


**Fixed 2026-09-03.** Root cause confirmed at the source: `read_edit_target` in
`src/tools/edit_file/mod.rs` passes `Access::FrontmatterWrite` unconditionally for every
plain-text-grammar write (it cannot prove its `old_string` stays out of the frontmatter
block), while `src/tools/markdown/edit_markdown.rs::edit` passes `Access::BodyWrite` for the
heading grammar (provably scoped to one section). The restriction itself is deliberate and
correct — Iron Law 5 already routes markdown edits to the heading grammar. Only the message
was wrong. Corrected the `stamped_only` hint in `guard_with_oracle`
(`src/util/librarian_guard.rs`) to name grammar (heading vs. plain text) as the discriminator
instead of the `frontmatter` param's presence, and added
`the_stamped_hint_names_grammar_not_the_frontmatter_param_as_the_discriminator` pinning the
corrected wording.

Committed at `81bed596`, patch-id `81b10af7d03e05e1037868f51384a1f243d86bf5`. Gate green on
`experiments`: `cargo fmt --check`, `cargo clippy --workspace --all-targets --features
local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test
--workspace` — exit 0 on all four (one default-lane run hit the already-tracked flaky
`run_exits_after_idle_timeout_with_no_connections` (`ee9d8d80ad5ecdc8`) under heavy concurrent
load; a clean retry passed 5113/5113).
## Tests added

None. A regression test is the unmatchable-`old_string` probe above: it asserts the guard's
refusal *shape* without depending on any file's content.

## Workarounds

Use the heading grammar on stamped artifacts: `edit_file(path=…, heading="## …",
action="edit", old_string=…, new_string=…)`. This is Iron Law 5's prescription anyway, so the
workaround is the recommended route and the bug is in the error path only.

## Resume

Read `src/util/librarian_guard.rs` around `:207` and find the branch that classifies a
text-grammar `edit_file` as a frontmatter write. Confirm or refute the `af974c0a` lead by
checking whether the guard's call-shape test predates the two-grammar split. Then choose one of
the two fixes above — do not fix the message and the behaviour in opposite directions.

## References

- `src/util/librarian_guard.rs:207` — the message.
- `c26943b5` (2026-09-01) — introduced it; subject states the intended allowance.
- `af974c0a` (2026-09-03) — the grammar fold, the best lead for where the allowance narrowed.
- `docs/issues/2026-09-03-markdown-grammar-librarian-guard-has-zero-test-coverage.md` — a peer's
  open bug reporting that `guard_not_librarian_managed` on the markdown-grammar edit route has
  **no test coverage anywhere in the workspace**. Different route, same guard, and it is the
  reason a narrowing like this one could land unnoticed. These two should be read together.
