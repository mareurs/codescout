---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
- tool-surface-collapse
- gates
- memories
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: high
---

# BUG: the tool-surface collapse's sweep was measured over surfaces a gate walks, and the surfaces that actually reach an agent were in none of them

The 2026-09-02/03 tool-surface collapse (26 tools → 21) retired `read_markdown`,
`edit_markdown`, `artifact`, `artifact_event`, `artifact_augment` and `artifact_refresh`.
Six gates assert that no retired **call form** survives. All six walk prose:
prompt slices, guide bodies, `CLAUDE.md`, and a list of doc roots.

**Three surfaces that put text in front of an agent on every single session are in
none of them** — and one of the three is the running server's own output.

## Severity rationale

`high`, not `medium`: an agent that follows the surviving text emits a call to a tool
absent from its own `tools/list`. The failure is a hard MCP unknown-tool error, and the
text that produced it is auto-loaded rather than sought out.

## Instance 1 — the live server tells agents to call a tool it does not register

Directly **observed live** in this session, not derived from source. A
`doc(action="find", kind="bug", semantic=…)` call returned:

```
"snippet": "... [snippet truncated — read the span with artifact(get)]"
"cap_suppressed_hint": "... Read the whole thing with artifact(action=\"get\", id=…), or narrow the query..."
```

Sites:

- `src/librarian/tools/find.rs:923` — appended to **every** truncated semantic snippet
- `src/librarian/tools/find.rs:987` — the `cap_suppressed_hint` body
- `src/librarian/tools/find.rs:919-920` — the code comment above the first

**And the bug is pinned by its own suite.** `src/librarian/tools/find.rs:1972` asserts
`.contains("artifact(action=\"get\"")` — so correcting the hint reds the test that was
written to protect it. That assertion is doing real work (it checks the hint names a
*recovery action*, not only the condition); only its expected literal is stale.

No gate covers runtime hint strings. All six cover prose surfaces.

## Instance 2 — `.codescout/memories/` (42 git-tracked files, auto-loaded every session)

Listed by name in every session-start banner, and the model is instructed to read them
before exploring. Not walked by `reader_docs_contain_no_retired_call_forms`.

| file | line(s) | what is stale |
|---|---|---|
| `project-overview.md` | 28, 31, 33, 35 | **enumerates the tool inventory**: `read_markdown`, `edit_markdown`, `artifact`, `artifact_event`, `artifact_refresh`, `artifact_augment` |
| `architecture.md` | 37–38 | names `artifact_event.rs`, `artifact_refresh.rs` — **both files are GONE** |
| `conventions.md` | 108 | `artifact(action="move", …)` — `CLAUDE.md`'s copy of this same sentence was corrected to `doc(action="move")`; the memory's was not |
| `gotchas.md` | 158, 169, 248, 252, 265, 320, 322, 331, 332, 538, 568 | 11 call forms |
| `worktree-merge-catalog-reconciliation.md` | 36, 65, 80, 88, 123 | 5 call forms |
| `fable-tuning.md` | 5 | `artifact(find, tags=["fable"])` |
| `infra/headroom-trial-and-langfuse.md` | 8 | `artifact(action="get", id=…)` |
| `private-memories/local-environment.md` | 28 | `artifact(action="find", scope="umbrella")` |

`project-overview.md` is the worst of these: a tool inventory is the one document whose
whole purpose is to be believed about which tools exist.

**Not stale — do not sweep these:**

- `architecture.md:22` — `read_markdown.rs`, `edit_markdown.rs`. Both files **still
  exist**; the modules survived the collapse and `librarian/tools/update.rs` still calls
  `edit_markdown::perform_section_edit_ext`. Rust module paths are deliberately outside
  the gate's scope (`src/prompts/mod.rs:2008` says so).
- `test-design-discipline.md:283`, `infra/friction-measurement.md:112` — historical
  narrative about measurements taken when those tools existed.

## Instance 3 — `docs/issues/_TEMPLATE.md:20`

The template every bug file is copied from still instructs
`artifact(action="find", kind="bug", …)`. Self-propagating: each new bug file that
quotes its own discovery recipe reproduces the retired form.

## Not a defect (recorded so it is not "fixed" later)

- **`src/cli/format.rs:506`** — `"next_step": "Call artifact_refresh(id) on each item …"`
  is an **annotated inert fixture**. The comment above it says the runtime string lives
  at `src/librarian/tools/refresh_stale.rs:91` and already reads `doc(action="gather", …)`,
  and that this literal pins nothing. Correct as it stands.
- **The stale `## codescout` block in a resumed session's system prompt.** Verified by
  speaking MCP stdio to the binary directly: the live handshake sends 1827 chars with
  **zero** retired names and the new Iron Laws 4/5. The injected copy differs from the
  live text in laws 4 and 5 **only** — 1, 2, 3 and 6 are byte-identical — i.e. it is
  exactly the pre-Task-7/8 revision. Session `2cb44cd3…` kept its id across the restart,
  no CC config cache holds the text, and the transcript carries the old form from
  `2026-09-02T00:28Z`. Harness-side resume carryover, not a codescout defect. Clears on
  a genuinely new session.

## Why the gates missed it — the class

`reader_docs_contain_no_retired_call_forms` (`src/prompts/mod.rs:2018`) walks five roots
(`docs/manual/src`, `docs/architecture`, `docs/conventions`, `docs/adrs`,
`src/prompts/guides`) plus ~9 named files. Its **name** claims "reader docs". The
most-read docs in the repo — the memory store — are not in it, and neither is any
runtime string.

The gate author did think past the repo: `companion_surfaces_reference_only_real_tools`
(`src/server.rs:3759`) reaches into the plugin. So the population was extended once, on
the axis that was salient, and the two surfaces with the highest read-frequency were
still missed. Extending a population *by hand* is the failure mode; the sweep's
completeness was never derived from "what text reaches an agent", only from "what
directories did I think of".

Cheap tell, for the next campaign: rank candidate surfaces by **how often an agent reads
them without asking**, and start at the top. Runtime tool output and the auto-loaded
memory store are both at the top, and both were last.

## Reproduction

```
# Instance 1 — live, no build needed
doc(action="find", kind="bug", semantic="anything that truncates a snippet")
# → snippet ends "read the span with artifact(get)"; hints.cap_suppressed_hint names artifact(action="get")

# Instance 2
grep -rn 'read_markdown\|edit_markdown\|artifact_event\|artifact_refresh\|artifact(' \
  .codescout/memories .codescout/private-memories | grep -v artifact_augmentation

# Instance 3
grep -n 'artifact(' docs/issues/_TEMPLATE.md
```

## Verified state at filing

Established by direct stdio handshake with `/home/marius/.cargo/bin/codescout start --debug`
(`git_sha 26b1f5c6` = HEAD):

- **21 tools**, matching the pinned registry exactly
- server instructions **1827 chars**, zero retired names, new Iron Laws 4/5
- CLI exposes `doc`; no `artifact*` subcommands
- companion plugin **live SKILL.md bodies are clean** (only `README.md:245` and historical
  `docs/plans/*` still carry retired forms)

So the *registered surface* is correct and complete. What survived is only the text
describing it.

## Fix sketch (not applied)

1. `src/librarian/tools/find.rs:919-923, 987` → `doc(action="get", …)`; update the
   expected literal at `:1972`, keeping the assertion's intent.
2. Sweep `.codescout/memories/**` call forms; **preserve** `architecture.md:22` and the
   two historical-narrative lines. Rewrite `project-overview.md`'s inventory to the 21.
3. `docs/issues/_TEMPLATE.md:20` → `doc(action="find", …)`.
4. Extend `reader_docs_contain_no_retired_call_forms` to `.codescout/memories`,
   `.codescout/private-memories` and `docs/issues/_TEMPLATE.md`, keeping the per-root
   non-vacuity assertion so a lost root reds rather than passes silently.
5. New gate over **runtime** strings: no `RecoverableError` message, `hint`, or
   `next_step` literal in `src/` may contain a retired call form. This is the one that
   does not exist in any form today.

Step 5 is the load-bearing one — steps 1-4 fix instances, step 5 fixes the class.

## Fix applied 2026-09-03

Both gates were written FIRST and observed RED over the real instances before anything was
corrected — `reader_docs_…` at 23 violations, `runtime_strings_…` at 2 — then green.

**Two findings from the fix are worth more than the fix.**

**1. The obvious gate design would have shipped the defect it was written for.** The first
prototype required the retired call form to appear *after a `"` on the same line*, on the
reasoning that a runtime string is a string literal. That heuristic caught `find.rs:923` and
**missed `find.rs:987`** — which is a continuation line of a multi-line Rust string and
therefore carries no quote of its own. Had the Rust gate been written straight from the fix
sketch, it would have gone red on one site, that site would have been fixed, and the gate
would be **green with the other defect still shipping** — the one actually observed in live
output. The working design drops the string-literal test entirely and leans on
`#[cfg(test)]` truncation plus a comment skip, because a retired call form in non-test,
non-comment Rust is a string literal or a compile error, with nothing in between.

**2. The gate found three sites the hand-scan missed, and one was invisible to it by
construction.** The grep in § *Instance 2* walked `.codescout/memories` and
`.codescout/private-memories`. It never walked `.codescout/` itself — so
`.codescout/system-prompt.md` was outside the sample. That file is a git-tracked,
hand-authored agent prompt whose line 35 read *"Markdown → `read_markdown` / `edit_markdown`"*
and whose line 34 called `artifact(action="find", …)`. Repeating the exact population error
this bug is about, inside the investigation of it, one screen after writing it down. The
other two were second occurrences in files whose first occurrence I had already found
(`_TEMPLATE.md:81`, `local-environment.md:57`) — the reflex to fix the hit rather than
re-scan the file.

### Changes

| surface | sites | note |
|---|---|---|
| `src/librarian/tools/find.rs` | 923, 987 | the two live runtime hints → `doc(action="get")` |
| `src/librarian/tools/find.rs` | 1972 | the test that PINNED the bug; assertion intent kept, literal updated |
| `src/librarian/tools/find.rs` 762, 919; `librarian/catalog/chunk.rs` 36 | 3 | stale comments — out of gate scope by design, fixed by hand |
| `.codescout/memories/` | 8 files | incl. `project-overview.md`'s tool inventory, rewritten from a live `tools/list` handshake |
| `.codescout/memories/architecture.md` | 37–38 | `artifact_event.rs` / `artifact_refresh.rs` removed — files are gone; `artifact.rs` KEPT, it still exists |
| `.codescout/system-prompt.md` | 9, 34, 35 | hand-edited, NOT regenerated — see below |
| `.codescout/project.toml` | 15 | `onboarding_version` 29 → 30 |
| `.codescout/private-memories/` | 2 | gitignored; scanned locally, absent in CI by design |
| `docs/issues/_TEMPLATE.md` | 20, 81 | prescriptive, so a stale form self-propagates once per bug file |

**`.codescout/system-prompt.md` was hand-edited on purpose.** `workspace(activate)` reported
`system_prompt_stale: stored 29, current 30` and named `onboarding(refresh_prompt)` as the
repair. Running it would have been wrong: the file is git-tracked and hand-authored, carrying
curated project rules (the four-command gate and its ordering rationale, the
`--features dashboard` note, the `json!("ok")` write convention) that regeneration from
templates would have destroyed. The stale-version signal was right; the prescribed remedy
would have cost more than the staleness. `onboarding_version` was bumped by hand instead.

### Two things deliberately NOT changed

- **`src/cli/format.rs:506`** and **`src/usage/db.rs:2377`** both hold retired names in test
  fixtures, both annotated in place as load-bearing. `db.rs`'s is the sharper case: historical
  `usage.db` rows carry `tool_name = "artifact_augment"` and the pre-collapse message text, and
  the classifier must still map them — its comment states that "fixing" the literal would keep
  the test green while dropping the only coverage of the historical form. The `#[cfg(test)]`
  truncation excludes both without an allowlist.
- **`.codescout/memories/architecture.md:22`** — `read_markdown.rs`, `edit_markdown.rs`. Both
  files still exist and `librarian/tools/update.rs` still calls into them. The modules survived
  the collapse; only their `impl Tool` went.

### One obsolete gotcha retired in passing

`gotchas.md`'s *"`create(augment={...})` silently drops `entry_collection`"* documented a bug
that no longer exists and prescribed a workaround through a deleted tool. Verified fixed:
`src/librarian/tools/create.rs:55` declares the field, `:440` propagates it, `:790` pins it.
Rewritten as fixed-with-history rather than deleted, because the CLASS
(`cluster/accepted-parameter-silently-dropped`, `IC-15`) is still open even though this member
is closed.

### The gate gap that remains, stated rather than hidden

`runtime_strings_contain_no_retired_call_forms` reads **call forms only, in non-comment,
non-test lines**. A runtime string naming a retired tool *without* a paren — the shape
`src/usage/db.rs:2377` preserves deliberately — would not red it. The check ran manually and
found no live instance, but nothing keeps one out.

## Tests added

- `prompts::tests::runtime_strings_contain_no_retired_call_forms` — new; the runtime-string
  population, which no gate covered.
- `prompts::tests::reader_docs_contain_no_retired_call_forms` — extended with the `.codescout`
  root and `docs/issues/_TEMPLATE.md`.
