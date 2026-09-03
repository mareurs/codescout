---
status: fixed
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

**Fixed 2026-09-03** by `librarian_guard_fires_on_the_markdown_grammar_write_route`
(`src/tools/edit_file/tests.rs`), sitting directly beneath its raw-text sibling.

**The prescription below was WRONG, and following it literally would have produced a test
that fails against correct code.** It said to assert that a call with `heading`/`action`
against "any path with a librarian-managed `id:` frontmatter key" is refused. It is not:
since the 2026-09-01 narrowing, a file that is stamped ONLY — not augmented, not a ledger —
permits reads and body writes and refuses only frontmatter writes. The markdown-grammar
route passes `Access::BodyWrite` whenever the caller sent no `frontmatter` param, so a
heading edit on a stamped file returns `Ok` by design.

The failure mode this would have caused is the interesting part. Written as prescribed, the
test reds against the CORRECT guard; the obvious repair is to make the guard refuse body
writes again, which silently reverts a deliberate narrowing and re-locks every stamped file
in the repo. `docs/issues/_TEMPLATE.md` carries no `id:`, so files created the documented
way are unstamped while `doc(action="create")` stamps everything — the affected population
is selected by creation route, not by any property of the file (measured 2026-09-01: 57 of
120 tracked files under `docs/trackers/`, 206 across `docs/issues/`).

This is CLAUDE.md's *"run the reproduction before reading the fix plan — the plan is a
hypothesis about the reproduction"* holding for a plan written by the same corpus that
states the rule. What caught it was reading `Access`'s own table before writing the
assertion, not running anything.

**Original prescription, kept because it is the trap:** *"call `edit_file` with
`heading`/`action` params against a path under `docs/trackers/` (or any path with a
librarian-managed `id:` frontmatter key), and assert the call is refused with the
librarian-guard error rather than applying the edit."*
## Tests added

`librarian_guard_fires_on_the_markdown_grammar_write_route` — `src/tools/edit_file/tests.rs`.

An eight-row table over the heading-grammar route, spanning both `Access` values it can
pass and all three guard reasons:

| target | shape | access | expect |
|---|---|---|---|
| stamped | `frontmatter` | FrontmatterWrite | refuse |
| ledger | `heading`+`action` | BodyWrite | refuse |
| ledger | batch `edits[]` | BodyWrite | refuse |
| ledger | `frontmatter` | FrontmatterWrite | refuse |
| **stamped** | **`heading`+`action`** | **BodyWrite** | **ALLOW** |
| **stamped** | **batch `edits[]`** | **BodyWrite** | **ALLOW** |
| plain | `heading`+`action` | BodyWrite | allow |
| plain | `frontmatter` | FrontmatterWrite | allow |

The two bold rows are load-bearing: they fail in the OPPOSITE direction from every other
row, so without them the table is satisfied by a guard that refuses everything — exactly
the behaviour the narrowing removed. The allowed rows also assert the file actually
CHANGED, so a route that silently applied nothing cannot satisfy them.

**Failures are accumulated, not asserted per row, and the difference was measured.** A
per-row `assert!` panics on the first bad row and leaves the rest unrun: under the mutation
it reported **1 of 4** broken rows. The accumulating form reports **4 of 4**. Three rows
could otherwise rot indefinitely behind a neighbour that happens to fail first.
## Workarounds
None needed — no live incorrect behavior today, only an unguarded refactor hazard.

## Resume

Nothing owed. Closed with the mutation run the original Resume demanded, extended past what
it asked for: all FOUR `guard_not_librarian_managed` call sites were mutated independently,
not just the one this bug names — CLAUDE.md's *"mutate once per guarded SITE, not once per
feature."*

| site | access passed | mutation reds |
|---|---|---|
| `src/tools/edit_file/mod.rs` (raw-text write) | always `FrontmatterWrite` | `librarian_guard_fires_on_every_edit_file_write_path` |
| `src/tools/markdown/edit_markdown.rs` (heading write) | `BodyWrite` / `FrontmatterWrite` | **was NOTHING — now this bug's test, all 4 refusing rows** |
| `src/tools/read_file.rs` (read) | `Read` | `read_file_force_true_on_a_managed_ledger_is_still_refused` |
| `src/tools/markdown/read_markdown.rs` (read) | `Read` | `read_file_refuses_a_managed_ledger_and_names_doc` |

So exactly one of four sites was uncovered and this bug named it correctly — but that was
verified rather than assumed, and the two read sites had never been checked by anyone.

The guard FUNCTION carries 23 unit tests, and every one of them passes with the
markdown-grammar call site deleted. That is the site-vs-function distinction in one
measurement: a well-tested predicate says nothing about whether anyone calls it.

All mutation work was done in an isolated git worktree, never the shared checkout. Five to
six sessions were committing into `experiments` throughout; a peer committing inside a
mutation window would have captured a guard-less source file, and the resulting tree would
have passed its own tests.
## References
- `docs/superpowers/plans/2026-09-02-tool-surface-collapse.md` (Task 8, where this gap was
  discovered during Step 6's prescribed mutation test)
- `src/tools/edit_file/tests.rs::librarian_guard_fires_on_every_edit_file_write_path` (the
  sibling test that DOES cover the raw-text site)
- `src/tools/markdown/edit_markdown.rs` (heading-grammar write path, guard call site)
- `src/tools/edit_file/mod.rs:753` (raw-text write path, guard call site — covered)
- CLAUDE.md § *Testing Discipline*, "Mutate once per guarded SITE, not once per feature" —
  this bug is a direct instance of that law
