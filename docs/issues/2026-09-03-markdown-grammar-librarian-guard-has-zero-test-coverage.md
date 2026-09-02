---
status: open
opened: 2026-09-03
closed:
severity: medium
owner: marius
related: []
tags: ['cluster/unclassified']
kind: bug
---

# BUG: `guard_not_librarian_managed` on the markdown-grammar edit route has zero test coverage anywhere in the workspace

## Summary
`crate::util::librarian_guard::guard_not_librarian_managed` is reached from exactly two
call sites in `edit_file`'s two write grammars — the raw-text route
(`src/tools/edit_file/mod.rs:753`) and the heading-addressed markdown-grammar route
(`src/tools/markdown/edit_markdown.rs:1260`, reached via `edit_file`'s `heading`/`action`
params on a `.md`/`.markdown` path, formerly the standalone `edit_markdown` tool). Deleting
the guard call at the raw-text site reds a test. Deleting it at the markdown-grammar site
reds nothing — across the full workspace suite, both the lean (`--no-default-features`) and
default lanes.

## Symptom (Effect)
No observable symptom yet — this is a coverage gap, not a live incorrect behavior. If the
markdown-grammar route's guard call were ever accidentally deleted or short-circuited in a
refactor, no test would fail; a caller could edit a librarian-managed artifact's body
through the heading grammar (e.g. `edit_file(path="docs/trackers/foo.md", heading="## X",
action="replace", content=...)`) and bypass the "do not hand-edit a managed artifact"
protection silently.

## Reproduction
Discovered while implementing Task 8 (fold `edit_markdown` into `edit_file`,
`docs/superpowers/plans/2026-09-02-tool-surface-collapse.md`), in a git worktree at
`/home/marius/work/claude/codescout/.worktrees/tool-collapse`, git commit at time of
mutation `HEAD` of that worktree branch (pre-Task-8-fold state at the time of the mutation
run — the guard call site and its coverage gap predate Task 8 and are not introduced by it).

1. Open `src/tools/markdown/edit_markdown.rs` at (then) line 1260, inside the heading-grammar
   write path.
2. Comment out / delete the `guard_not_librarian_managed(...)?` call at that site only
   (leave the raw-text site's call in `src/tools/edit_file/mod.rs:753` untouched).
3. Run `cargo test --workspace --no-default-features` (lean lane, 3318 tests) and
   `cargo test --workspace` (default lane, 5035 tests).
4. Restore the deleted line.

## Environment
Linux, Rust workspace `codescout`, worktree `.worktrees/tool-collapse` off branch
`experiments`. MCP transport not involved — this is a pure `cargo test` mutation.

## Root cause
*Mechanism, not symptom:* the markdown-grammar write path
(`src/tools/markdown/edit_markdown.rs`, function reached via `edit_file::call` when
`heading`/`action`/`frontmatter`/a heading-addressed `edits[]` item is present on a
`.md`/`.markdown` path) calls `guard_not_librarian_managed` before applying any edit. Every
existing test that exercises this path either (a) never targets a file with librarian-managed
frontmatter (an `id:` key) or a path under `docs/trackers/`, so the guard's `Ok(())` fast
path is exercised but its `Err(...)` branch never is, or (b) tests the *raw-text* route's
guard instead, which is a separate call site with its own coverage
(`librarian_guard_fires_on_every_edit_file_write_path` in
`src/tools/edit_file/tests.rs`). No test asserts that a heading-addressed write to a
managed artifact is refused.

*Measured 2026-09-02*: deleted the guard call at
`src/tools/markdown/edit_markdown.rs:1260` (line number as of that commit; the guard call
now lives inside the same function post-Task-8-rename, see `Fix` below for current
location), ran both test lanes — 0 failures in either (3318 passed lean, 5035 passed
default, ignoring one pre-existing unrelated flaky test
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections`). Restored the line
and re-confirmed both lanes clean. By contrast, the identical mutation at the raw-text call
site (`src/tools/edit_file/mod.rs:753`) reds
`librarian_guard_fires_on_every_edit_file_write_path` immediately.

### Sharper mechanism, added by the controller after re-deriving the mutation (2026-09-03)

The account above says existing tests "never target a file with librarian-managed
frontmatter". True, and verified — `src/tools/markdown/tests.rs` has **zero** matches for
`librarian`, `id:` or `entry_prefix`, so the guard's `Err` branch is unreachable from that
file by construction. But that phrasing reads as an accident of fixture choice, and it is
not: the gap is **structural, known, and already named in this repo.**

`tests/edit_markdown_catalog_sync.rs`'s own module header states it:

> `librarian_guard` has the identical exposure and resolves it by never testing the global
> path — its own tests pass an oracle explicitly. That leaves the wiring itself unproven,
> which is exactly the shape `bug-fix-session-log:W-73` warns about: a guard that is
> written, tested, and never called reads as fully covered.

`guard_with_oracle` exists so a test can inject an oracle and never touch the process-wide
`ORACLE` slot (`src/util/librarian_guard.rs:118-120` — *"so a test never has to install into
the process-wide `OnceLock`"*). That is a good decision for the same reason
`edit_markdown_catalog_sync` gives — a contested global slot makes assertions
scheduling-dependent. Its cost is that it splits the guard into a **decision** that is
thoroughly tested and a **wiring** at each call site that is tested only if something else
happens to exercise it.

So the raw-text route is not covered because someone was careful. It is covered because a
shipped bypass forced a test:
`docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md`. The
markdown route never had that forcing event. This is `CLAUDE.md` § *Testing Discipline*'s
loudness law — *"an alarm nothing reaches is exactly as informative as no alarm"* — at call-site
granularity.

**Which makes the fix bigger than one test, and the real question a population one.**
`guard_not_librarian_managed` has four production call sites:

| call site | wiring test? |
|---|---|
| `src/tools/edit_file/mod.rs:753` (raw-text write) | yes — `librarian_guard_fires_on_every_edit_file_write_path`, forced by the 2026-08-16 bypass |
| `src/tools/markdown/edit_markdown.rs:1261` (markdown write) | **no — verified by mutation, twice** |
| `src/tools/read_file.rs:170` (shared read) | yes — added by Task 7's fix round, forced by the read twin of the same bypass |
| `src/tools/markdown/read_markdown.rs:516` (markdown read) | **not derived — do this before closing** |

Three of four are accounted for and the pattern across them is that **coverage arrived only
where a bug forced it**. The fourth is deliberately left underived rather than assumed: it is
the same shape as the one this file is about, and guessing it either way would be exactly the
unchecked-completion the ledger warns against. Derive it by the same mutation, per
`CLAUDE.md` — *"mutate once per guarded SITE, not once per feature."*

**Do not read this table as the whole population either.** It counts call sites of
*this* guard. `W-73`'s claim is about guards in general, so the transferable question is how
many OTHER guards in this tree are tested through an injected seam and never at the wiring.
That number is not derived here and should not be guessed from these four.
## Evidence
### Mutation run, markdown-grammar site (site 1)
Deleted the `guard_not_librarian_managed(...)?` call inside the heading-grammar write
function. `cargo test --workspace --no-default-features`: 3318 passed, 0 failed (1
pre-existing unrelated fail, the known-flaky idle-timeout test, present with or without the
mutation). `cargo test --workspace`: 5035 passed, 2 failed — same known-flaky test plus one
that was independently fixed this session (`server_tool_count_is_l3_target`, unrelated to
this mutation). **Zero tests newly failed as a result of this specific mutation.**

### Mutation run, raw-text site (site 2), for contrast
Deleted the `guard_not_librarian_managed(...)?` call inside `edit_file::call`'s raw-text
write branch (`src/tools/edit_file/mod.rs:753`).
`librarian_guard_fires_on_every_edit_file_write_path` failed immediately with an assertion
that a write to a managed artifact should have been refused but was not. Restored the line;
test passed again.

## Hypotheses tried
1. **Hypothesis:** the markdown-grammar route is untestable because the guard only ever
   sees non-managed paths in practice (e.g. it's redundant with an earlier check).
   **Test:** read `src/tools/markdown/edit_markdown.rs` around the call site and its
   surrounding control flow. **Verdict:** rejected — the call is a plain guard invocation
   with no preceding equivalent check; it is reachable on any `.md` path including managed
   ones. **Evidence link:** Root cause above.
2. **Hypothesis:** some *other* test indirectly covers this via an integration path (CLI,
   MCP end-to-end). **Test:** ran the full workspace suite (both lanes) with the mutation in
   place. **Verdict:** rejected — 0 new failures in either lane. **Evidence link:** Evidence
   above.

## Fix
*Not yet fixed — filing for tracking, per CLAUDE.md "capture on notice."* The fix is a
regression test analogous to
`librarian_guard_fires_on_every_edit_file_write_path` (`src/tools/edit_file/tests.rs`), but
targeting the heading-addressed grammar: call `edit_file` with `heading`/`action` params
against a path under `docs/trackers/` (or any path with a librarian-managed `id:`
frontmatter key), and assert the call is refused with the librarian-guard error rather than
applying the edit. Natural home: `src/tools/edit_file/tests.rs` (co-located with the
raw-text sibling) or `src/tools/markdown/tests.rs` (co-located with the other
markdown-grammar tests). No code fix is implied — the guard call itself is present and
correct; only the missing regression test is the deliverable.

## Tests added
None yet — this file exists to track the gap, not to close it. `N/A` justified: this bug
*is* "no test exists"; closing it means adding the test named in Fix above, at which point
this file should flip to `fixed` with `Tests added:` naming it.

## Workarounds
None needed — no live incorrect behavior today, only an unguarded refactor hazard.

## Resume
Add a test in `src/tools/edit_file/tests.rs` (or `src/tools/markdown/tests.rs`) that calls
`edit_file` with `heading="## Foo", action="replace", content="..."` against a
`docs/trackers/`-scoped or `id:`-frontmattered fixture path, and asserts the call returns
the librarian-guard refusal rather than writing. Confirm with the same mutation (delete the
guard call, expect this new test to red) before marking `fixed`.

## References
- `docs/superpowers/plans/2026-09-02-tool-surface-collapse.md` (Task 8, where this gap was
  discovered during Step 6's prescribed mutation test)
- `src/tools/edit_file/tests.rs::librarian_guard_fires_on_every_edit_file_write_path` (the
  sibling test that DOES cover the raw-text site)
- `src/tools/markdown/edit_markdown.rs` (heading-grammar write path, guard call site)
- `src/tools/edit_file/mod.rs:753` (raw-text write path, guard call site — covered)
- CLAUDE.md § *Testing Discipline*, "Mutate once per guarded SITE, not once per feature" —
  this bug is a direct instance of that law
