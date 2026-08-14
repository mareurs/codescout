---
status: investigating
opened: 2026-08-13
closed:
severity: medium
owner: marius
related: []
tags: [worktree, workspace-activation, semantic-search, retrieval, companion-plugin, agent-agnostic]
kind: bug
---

# BUG: Claude Code's `EnterWorktree` desyncs codescout's active project, and activating the worktree strands semantic search

## Summary

Claude Code can switch the session into a linked git worktree (its `EnterWorktree`
tool). codescout does not follow, so its tools read the **main checkout** while
native tools operate on the worktree. Activating the worktree corrects the tree
and semantic search goes silent — it returns an empty result set with no hint,
indistinguishable from a query that legitimately matched nothing.

The divergence is **three-part**, not two: the tree, the vector index, and the
project's memory/sub-project topology all diverge independently.

A harness-side signal **already exists and already fires** — a `PostToolUse` hook
on `EnterWorktree` instructs the agent to activate and blocks codescout's *write*
tools until it does. Reads are unguarded by design, which is the actual hole.
## Status 2026-08-14 — HALF 2 SHIPPED, halves 1 and 3 still open

**Do not archive this file.** One of the three divergences is fixed; two are not.

### Half 2 — FIXED on `experiments`

Worktree semantic search works. Shipped as an 8-task plan
(`docs/superpowers/plans/2026-08-13-worktree-semantic-search.md`, spec
`docs/superpowers/specs/2026-08-13-worktree-semantic-search-design.md`) across
**`b7989098..bb26f43c`**, 25 commits, plus a fix wave and a closing pass after the
final whole-branch review.

What landed: a **per-worktree delta project** keyed on git's own worktree name
(`worktree_ids`/`delta_project_id`, `src/retrieval/sync.rs`); a content-hash dirty
set (`dirty_paths`, `src/retrieval/drift.rs`) that needs no base commit and inherits
no staleness window; `exclude_paths` on `CodeVectorStore::query` and `SearchOpts`;
`IndexState.dirty_paths` with `#[serde(default)]` and `schema_version` 1 → 2;
`sync_worktree` reachable only from the `index` tool, so `semantic_search` never
writes; and on Qdrant a **single union query** (`CodeVectorStore::query_overlay`)
rather than two merged result sets. Gate at the close: **3705 / 44** default,
**3718 / 51** `--features server-stack`, both clippy lanes clean.

The companion-plugin half of the fix — retiring the hook instruction that said
*"Do NOT run index in worktrees"*, which this plan made false, and replacing it with
correct `index(action="build")` guidance — is on branch
**`feat/worktree-index-hooks` @ `d65f96d` in `claude-plugins`, UNPUSHED**. That is a
separate repo with its own release checklist. **Until it lands, an agent entering a
worktree is still told not to index.**

**Known limitations that shipped deliberately**, all recorded with rulings:

- Three reachable **double-serve** states (main and the delta both returning a copy
  of one path). Two are flagged in the response (`worktree_state_warning` on the
  `Suspect` state; `drift_note` when main is newer). The third is a residual: a path
  reverted to main's exact bytes is absent from the new sidecar while its stale delta
  chunks survive an early return that skips the prune.
- A first-ever **failed** sync now leaves a full dirty set with an empty delta, which
  classifies `Healthy` — so the actionable "not yet indexed" hint is not shown.
- `last_indexed_at` after a failed sync means "when the dirty set was computed", not
  "when the delta was built", so a legitimate `drift_note` can be suppressed.
- **Sub-projects are not covered**: for a workspace sub-project, `<root>/.git` does
  not exist, `detect_worktree_info` returns `None`, and both the producer and the
  consumer fall through to the plain path — consistent, but the worktree hint never
  fires and the user sees an empty result with no explanation.

### Half 1 — PARTIALLY addressed

The `PostToolUse` hook on `EnterWorktree` exists and now carries correct index
guidance (pending the unpushed branch above). The gap this file documents — that the
signal **covers writes but not reads** — is unchanged: MCP write tools are blocked
until `workspace()` is called, reads are not.

### Half 3 — UNTOUCHED

Memory set and sub-project topology divergence. Nothing in this plan addressed it.

### Before archiving

Split halves 1 and 3 into their own bug files first. Archiving this one as-is would
retire two unfixed divergences along with the fixed one.
## Symptom (Effect)

User-reported, 2026-08-13 (not yet reproduced by an agent in-session):

1. Claude Code enters a worktree via `EnterWorktree`. codescout's active project
   is unchanged — it still points at the main checkout. Native tools operate on
   the worktree; codescout's `symbols`/`grep`/`edit_code`/`read_file` operate on
   main. Nothing reports the divergence.
2. Telling codescout to switch (`workspace(action="activate", path=<worktree>)`)
   corrects the tree, and **semantic search stops working** for that project.

Exact tool output for either half is not yet captured — see *Resume*.

## Reproduction

Not yet reproduced in-session. Intended steps:

```
# 1. from the main checkout, note the active project
workspace(action="status")

# 2. Claude Code side: EnterWorktree into a linked worktree
# 3. re-check — expected to still name the main checkout
workspace(action="status")

# 4. confirm the wrong-tree read: edit a file in the worktree natively,
#    then read the same relative path through codescout
# 5. activate the worktree, then query
workspace(action="activate", path="<repo>/.worktrees/<name>")
semantic_search(query="<something the corpus certainly contains>")
```

## Environment

- codescout `experiments` is the target branch, but **every code citation below was
  measured on `feat/local-onnx-query-path` @ `927feaf4`**, which is what the main
  checkout was on. See *Branch caveat* under Root cause — this is not a footnote,
  it changes how much the line numbers are worth.
- Claude Code with the `codescout-companion` plugin active. Native
  `Read`/`Grep`/`Glob`/`Edit`/`Write` and all native `Bash` are hard-denied on
  source, so codescout's tools are the *only* path to source — there is no
  fallback that happens to be correct.
- **Worktree location is `<repo>/.claude/worktrees/<name>`** — Claude Code's own
  convention, named after the branch. *Corrected: this file originally said
  `<repo>/.worktrees/<name>`, the convention `doctor` and `merge_worktree` use.*
  Harmless for detection (which is `.git`-pointer-based and convention-agnostic),
  wrong for anything that scans directories by path pattern.
## Root cause

Known for half 2. Half 1 is known to be a *coverage* gap rather than a missing
signal.

### Branch caveat — read before trusting a line number

The investigation ran against `feat/local-onnx-query-path` @ `927feaf4`, not
`experiments`. `git diff --stat experiments HEAD` over the relevant paths:
`retrieval/client.rs` +550, `retrieval/config.rs` +445, `retrieval/embedder.rs`
+543, `retrieval/search.rs` +286, `semantic_search.rs` +166, `tools/config/mod.rs`
+77 — 2,592 insertions across 12 files, on a branch whose stated purpose is
reworking the local ONNX **query path**.

So: the *behavioural* findings are solid, because they were observed against a
running server. The `path:line` **mechanism** claims are provisional and must be
re-read on `experiments` before any fix is built on them.

### Half 2 — `project_id` is a directory basename, and the collection is global

Measured 2026-08-13. The Qdrant collection is shared across every project
(`retrieval/config.rs` — `collection()` is `collection_prefix + kind`, and the
prefix comes only from `CODESCOUT_QDRANT_COLLECTION_PREFIX`), so **`project_id` is
the sole discriminator**. And `project_id` is `project.name` from
`.codescout/project.toml`, which falls back to the root directory's basename when
that file is absent (`src/config/project.rs`).

`.codescout/project.toml` is **gitignored**, so no linked worktree ever has one,
and activation does not create one. A worktree at `.claude/worktrees/peer-delegation`
therefore resolves `project_id = "peer-delegation"`, which matches zero points.
Hence a bare empty result — and `check_has_index` reporting `not_indexed` is
*correct* there, not a false negative.

The silence is the defect, not the emptiness.

### Half 1 — the signal exists; it covers writes but not reads

Measured 2026-08-13. `codescout-companion/hooks/hooks.json:141` registers
`PostToolUse` matcher `"EnterWorktree"` → `worktree-activate.mjs`, which emits an
`additionalContext` instruction to call `workspace(action="activate", …)` NOW,
drops a `.cs-worktree-pending` marker, and pairs with `worktree-write-guard.mjs`
to hard-deny `edit_code`/`edit_file`/`edit_markdown`/`create_file` until the
marker clears.

So writes are protected. `symbols`/`grep`/`read_file`/`references`/`semantic_search`
are not, and silently read main until the agent complies with an *advisory*
instruction. That — not "no signal exists" — is the defect.

### Half 3 — memory set and sub-project topology also diverge (new, unreported)

Measured 2026-08-13. The worktree activation listed **11** memory topics against
main's **21**, and **9** sub-projects (every `tests/fixtures/*`) against main's
**2**. Cause: `.codescout/memories/` is git-tracked, so a worktree serves *that
commit's* memories; `.codescout/workspace.toml` is gitignored, so sub-project
discovery falls back to auto-detect. CLAUDE.md already warns that a mis-rooted
`workspace.toml` silently redirects per-project memory writes — a worktree hits
the same class by **absence**.
## Evidence

All measured 2026-08-13 against a running server on `feat/local-onnx-query-path`
@ `927feaf4`, using the pre-existing worktree `.claude/worktrees/peer-delegation`.
No throwaway worktree was created; `.git/worktrees` was left unchanged.

### Silent empty, not a refusal

Control in the main checkout: `semantic_search(query="OutputGuard cap_items", limit=3)`
→ 3 hits from `src/tools/output.rs`; `workspace(status)` → `up_to_date, files 1416,
chunks 34635`.

After `workspace(action="activate", path=<worktree>, read_only=true)`, the identical
call returned, verbatim and complete:

```json
{"results": [], "total": 0, "truncated": false}
```

No `RecoverableError`, no hint, no staleness note. Same result via the per-call
`workspace=` pin, so it is not an activation artifact.

### The stale-vectors outcome is one string away

Pinned to the same worktree but with `project_id="codescout"` forced, the call
returned **main's three chunks with main's file paths**. The only thing separating
a worktree from main's vectors is that basename. Not today's default under Claude
Code (worktrees are branch-named), but a worktree named `codescout` — or two
sibling repos sharing a basename — lands there silently. This is a keying
weakness, not a worktree-specific one.

### The good error message exists and cannot fire

`classify_search_error` has a message for precisely this case — *"Qdrant collection
is missing for project `X`"* — but the collection is **shared and present**, so the
branch is unreachable on this path. The explanatory machinery was built for a
per-project-collection world.

### Two shipped surfaces contradict each other

`worktree-activate.mjs` says *"Do NOT run index in worktrees — the shared index is
read-only here."* codescout's own activation response says *"Run
index(action='build') to enable semantic_search."* Both fired in the same session.
Whichever fix wins must resolve this.

Relatedly, the hook's *"shared index"* intent is implemented by symlinking
`.codescout/embeddings` — the **legacy sqlite store**, which both activations this
session flagged as `legacy_semantic_index` needing `codescout migrate-memories`.
The live stack is Qdrant, keyed by the `project.toml` that is precisely what does
*not* get linked. **The sharing mechanism predates Qdrant and is now a no-op for
semantic search.**

### The server cannot compare its root to the caller's cwd

`grep("CLAUDE_PROJECT_DIR|current_dir\\(\\)", src)` → 18 hits, **zero** for
`CLAUDE_PROJECT_DIR`. The server's cwd is frozen at spawn. Per memory
`claude-code-mcp-env`, MCP `roots` — the protocol feature that would push a
workspace change — is **not supported client-side** (issue #57243, planned only).

### A worktree-aware banner already exists, and is silent in the desync case

`src/prompts/mod.rs` has `detect_worktree_info(root)` (filesystem-only, parses the
`.git` pointer, three passing tests), `ProjectStatus.worktree`, and a rendered
`- **Worktree:** branch \`X\` of \`Y\`` line refreshed on every activation — residue
of `docs/issues/archive/2026-05-28-path-annotation-spam.md`, whose title ends
*"activation + worktree state invisible"*. The lead paid off: detection is built.
The gap is orientation — the banner fires when the *active project* is a worktree,
and in half 1 the active project is **main**. It is silent exactly when needed.

It is also convention-agnostic: `is_linked_worktree` only requires a `worktrees`
component in the `gitdir:` pointer, so it already handles `.claude/worktrees/<name>`.
## Hypotheses tried

1. **Hypothesis:** already filed; about to re-file.
   **Test:** umbrella bug query incl. archived, on `worktree`; then a 12-id ledger
   sweep during investigation.
   **Verdict:** rejected — all `fixed`, all catalog-side. Nothing re-filed.

2. **Hypothesis:** retrieval has partial worktree handling with a gap.
   **Verdict:** rejected — two matches, both about skipping the `.git` pointer file.

3. **Hypothesis:** activating a worktree silently serves main's vectors (worst case).
   **Test:** run it.
   **Verdict:** **rejected as the default, confirmed as reachable.** Default is a
   bare empty result. Forcing a colliding `project_id` does serve main's vectors
   with main's paths. The worst case is one string away, so it is a live hazard
   rather than a non-issue.

4. **Hypothesis (mine, in this file's first draft):** there is likely no signal to
   miss — the gap is architectural.
   **Verdict:** **half falsified.** True that MCP pushes nothing to the server.
   False that no signal exists: a `PostToolUse` hook fires today and instructs the
   agent to activate. I inferred the absence of a mechanism from protocol shape
   without reading the plugin that already implements one.

5. **Hypothesis:** `index.status` from activation is a usable trigger for a fix.
   **Verdict:** rejected. Both home activations reported `not_indexed` while
   `workspace(status)` reported `up_to_date, 34635 chunks` and search worked.
   Measured, not diagnosed — and `tools/config/mod.rs` is +77 on this branch, so
   branch state is a likelier cause than the archived probe-cache bug
   (`2026-07-12-activate-index-status-stale-probe-cache-false-negative.md`,
   marked `fixed`). Probe `project_has_chunks` at query time instead of trusting
   the field. **Re-measure on `experiments` before calling that bug a zombie.**
## Fix

Ranked, with the first draft's candidates marked where the findings killed them.

### Half 1 — extend the existing hook; do not design a new mechanism

Keep it in the plugin: `EnterWorktree` is Claude-Code-specific and the server must
not learn its name (Agent-Agnostic Design). Highest-value change is to **cover
reads**, since writes are already hard-denied. `worktree-write-guard.mjs` already
has the exact detection (`--git-common-dir != --git-dir`, plus the marker) — either
widen its matcher to codescout's read tools, or make the pending marker produce a
loud advisory on reads. No server change needed.

- ~~(a) the companion plugin detects the switch and issues `workspace(activate)`~~ —
  **already shipped**, including that exact instruction. Not a decision.
- ~~(b) codescout notices its resolved root differs from the caller's cwd~~ —
  **falsified as unimplementable.** There is no caller cwd available: spawn-frozen
  `current_dir()`, `CLAUDE_PROJECT_DIR` never read (and also spawn-time), MCP
  `roots` unsupported. The property that made it attractive — "fails loudly in any
  harness" — is not obtainable this way. The achievable agent-agnostic version is
  weaker and differently shaped: warn when the *active project* is a worktree,
  which the `src/prompts/mod.rs` banner already does, and which says nothing about
  whether the harness agrees.

### Half 2 — decide the contract, then make one surface true

The user's correctness argument is binding and the measurements support it: main's
vectors *are* served under a colliding `project_id`, silently, with main's paths.
So **do not ship "symlink `project.toml`"** — that is the confidently-stale outcome,
not a fix.

1. **Cheapest honest fix — make the empty result speak.** When
   `project_has_chunks(project_id)` is false, `semantic_search` should return a hint
   naming the resolved `project_id`, the fact that this is a worktree, and both
   exits (index it, or use `symbols`/`grep`, which are correct here). Agent-agnostic,
   cheap, and it addresses the actual complaint — the user's problem is *silence*,
   not absence.
2. **Resolve the contradiction:** either the hook stops saying "do NOT run index in
   worktrees", or activation stops saying "run index(action='build')". Today both ship.
3. ~~(a) refuse with a `RecoverableError` — "the current behaviour if it already
   refuses"~~ — **falsified.** It does not refuse; it returns silent empty.
4. **(b) serve main's index with a staleness bound** — survives, and is closer to
   shipped than the first draft assumed. But the naive version *is* the stale-vectors
   outcome, so `git diff --name-only <base>` is not a refinement of this design, it
   **is** the design.
5. ~~(c) index the worktree as its own project~~ — still available, still expensive
   per worktree, and now clearly the wrong default given (1) is nearly free.

### Half 3 — memory/topology divergence

Undecided, and deliberately not bundled. Needs its own think: git-tracked memories
serving a stale commit is arguably correct behaviour for a worktree, while
`workspace.toml` falling back to auto-detect is arguably not. Splitting to its own
bug file is likely right once halves 1–2 land.
## Tests added

None yet — not fixed. When it is, the guard has to run *in* a worktree, which
`src/librarian/catalog/worktree.rs`'s existing tests already do somehow; read
those for the fixture pattern before inventing one. Note the trap this cohort
just hit (F-32): a test that only fails in an environment nobody runs is not a
guard. A worktree fixture must be created by the test, not assumed present.

## Workarounds

**Confirmed measured 2026-08-13**, which upgrades this from advice to a finding:
`grep(pattern="fn cap_items", path="src/tools/output.rs")` pinned to the worktree
returned 5 correct matches from the worktree's own files.

So — **losing semantic search is not losing code intelligence.** Filesystem-computed
tools (`symbols`, `grep`, `references`, `read_file`) all follow the worktree
correctly. Only the vector index is stale-by-construction, because only it is
precomputed and keyed per project.

Activate the worktree, accept no semantic search, and navigate by symbol and
reference. The alternative — staying in main and letting git move the work — loses
the isolation `EnterWorktree` was for.
## Resume

**Half 2 is done — do not re-investigate it.** Read the *Status 2026-08-14* section
above before anything else; it names what shipped, the SHA range, and the
limitations that shipped on purpose.

Concrete next actions, in order:

1. **Decide the companion branch.** `feat/worktree-index-hooks` @ `d65f96d` in
   `claude-plugins` is unpushed and unmerged. Until it lands the hook still tells
   agents not to index in a worktree, so half 2's fix is only reachable by a user who
   knows to run `index(action="build")` manually. That repo also has an open
   secret-guard decision on `recon/promote-substrate-bytes-secrets`, so sequence the
   two deliberately.
2. **Split halves 1 and 3 into their own bug files**, then archive this one. Do it
   through the librarian (`artifact(action="move", …)`), never a bare `git mv` —
   `id = sha256(abs_path)`.
3. **Triage the deferred minors** recorded in the run ledger at
   `.superpowers/sdd/2026-08-13-worktree-semantic-search/progress.md` (kept, not
   deleted). The final review's own triage lists what it judged must-fix versus
   fine-to-ship; the must-fix set was cleared by the fix wave.

One seam is knowingly untested and worth closing if this area is touched again:
reverting `semantic_search` to two `search_code` calls plus `merge_hits` would not
fail any test, because nothing builds a Qdrant-backed `RetrievalClient`. The sibling
seam (deleting Qdrant's `query_overlay` override) **is** now covered, by
`c284786c`.
## References

- `src/retrieval/sync.rs:107,385` — retrieval's only worktree mentions.
- `src/librarian/catalog/worktree.rs`, `src/librarian/tools/merge_worktree.rs` —
  the overlay design retrieval did not get; the precedent for Fix half-2 (b).
- `get_guide("workspace-state")` — activation, home/foreign, reset semantics.
- `docs/architecture/companion-plugin.md` — the harness-specific layer, likely
  home for Fix half-1 (a).
- memory `conventions` § Agent-Agnostic Design — why the server must not learn
  about `EnterWorktree` by name.
- `docs/trackers/release-promotion-session-log.md` F-32 — the ambient-dependency
  test trap this fix's guard must avoid.
