# Full-Read Fidelity — Design

**Status:** draft, awaiting review
**Author:** Claude (session 069cdecb), 2026-08-07
**Branch:** `experiments` @ `d7988aca`
**Problem owner:** Chris

> **Provenance — read before trusting a line number.** This document was written
> *on a different machine, against a different checkout*, by an external codescout
> user (see [`index.md`](index.md)). It arrived here as a WhatsApp attachment on
> 2026-08-10 and is reproduced verbatim except that absolute home paths were
> rewritten `/Users/<him>/` → `~/`. Every `file.rs:NNN` below is relative to
> `experiments @ d7988aca`, **not** to this checkout — D1 and D2 both still
> reproduce here, but at different lines. Nothing in it has been edited to match
> our tree, deliberately: it is evidence, not documentation.

---

## 1. Problem

codescout's progressive-disclosure layer is doing its job — it keeps context small.
But on one path it produces a response that **reads as a complete answer while
containing zero content**, and on that same path the documented escape hatch is
silently discarded. The result is not "codescout hides code"; it is "codescout lets
an agent conclude from a preview without knowing it was a preview."

This design fixes the two mechanisms that allow that, and corrects one piece of
standing guidance that overstates a third.

### 1.1 What is NOT broken (explicitly preserve)

Verified this session; these are the benefits the fix must not regress.

| Behaviour | Evidence |
|---|---|
| Buffer fidelity is exact — `@file_*` returns byte-accurate content | Read `@file_dc207020` lines 643–700, matched source exactly |
| Constants match documentation, no drift | `src/tools/core/types.rs:18-51` vs `get_guide("progressive-disclosure")` |
| `tree` announces its own cap | `src/tools/tree.rs:178-180` sets `depth_capped`; `:322-327` renders `[depth capped at 3 …]` |
| Overflow is uncommon, not pervasive | `usage.db` (`~/work/hmm`, 6,621 calls): 4.9% overall overflow |
| The shell gate has a working override | `run_command(..., acknowledge_risk=true)` executed `wc` successfully |

Overflow rate by tool (same DB), for calibration:

| Tool | Calls | Overflowed | % |
|---|---|---|---|
| `symbols` | 1,125 | 133 | 11.8 |
| `run_command` | 1,774 | 116 | 6.5 |
| `grep` | 1,316 | 56 | 4.3 |
| `read_file` | 1,186 | 10 | **0.8** |

Note `read_file` is the *least* frequent overflower. This is a low-frequency,
high-consequence defect: rare, but when it fires the agent gets a confident-looking
outline and no content.

---

## 2. Defects

### D1 — `force=true` is discarded on whole-file reads

`src/tools/read_file.rs:133-147`:

```rust
let force = input["force"].as_bool().unwrap_or(false);

if let (Some(start), Some(end)) = (start_line, end_line) {
    return read_with_line_range(path, …, start, end, &source_tag, ctx, force);
}
read_full_file(path, &text, &resolved, &input, &source_tag, ctx)   // ← no `force`
```

`force` reaches `read_with_line_range` only. `read_full_file` never receives it, and
at `:653` buffers unconditionally on `exceeds_inline_limit(text)`.

**Consequence:** there is no way to force a full read of a file over 10 KB. The
parameter is accepted and ignored.

**Measured:** `read_file("src/tools/read_file.rs", force=true)` on a 59,718-byte file
returned a 25-symbol outline and **0 lines of content**.

**Nuance:** the schema documents `force` as *"Skip source-symbol hint and read the raw
line range"* (`:43`) — so whole-file was arguably never in scope. That makes this a
design gap rather than a coding error, but the user-visible effect is identical: an
agent that passes `force=true` believes it overrode the default and did not.

### D2 — the buffered summary carries no incompleteness signal

`src/tools/file_summary/file_summary.rs:52-80` (`summarize_source`) returns exactly:

```json
{ "type": "source", "line_count": N, "symbols": [...] }
```

`read_full_file` then adds only `result["file_id"]`. There is **no** `hint`, no
`complete: false`, no shown-vs-total count.

The renderer prints `line_count` as a bare header — `"1505 lines"` — which reads as a
property of the answer, not as a warning that 1,505 lines were withheld. The sole
signal is a trailing `Buffer: @file_…`, which reads as a bonus affordance.

This is inconsistent with the rest of codescout: `tree` emits an explicit
`[depth capped at 3 — use max_depth=N …]` note (`tree.rs:322-327`), and
`run_command`'s envelope carries shown/total counts. `read_file`'s full-read path is
the outlier.

**Key enabling discovery:** `format_read_file_summary`
(`src/tools/read_file.rs:894-1041`) **already renders a `hint` field when present**:

```rust
if let Some(hint) = val["hint"].as_str() {
    out.push_str(&format!("\n  {hint}"));
}
```

So D2 needs no renderer change for the hint — only population at the source.

### D3 — read-only metadata blocked by the shell gate (minor)

`run_command("wc -lc src/…")` → `shell access to source files is blocked`. `wc` is
pure metadata and cannot leak more than `symbols` already does. The override works,
so this is friction, not a wall — it nudges toward guessing file size.

---

## 3. Design

### 3.1 D1 — make `force` mean force

Thread `force` into `read_full_file` and branch on it.

**Strengthened 2026-08-08 — `detail_level='full'` cannot bypass this either.**
`OutputGuard::from_input` is called at `read_file.rs:697`, *after* the
`exceeds_inline_limit` early-return block at `:653-688`. Mode is therefore never
consulted on the over-budget path — the function has already returned.
`OutputGuard::from_input` (`output.rs:89-112`) maps `detail_level:"full"` →
`OutputMode::Focused`, but nothing on this path ever reads it.

Consequence: for a source file over 10 KB there is currently **no parameter of any
kind** — not `force`, not `detail_level='full'`, not `limit` — that returns content
inline. Both documented escape hatches are inert here. This is a stronger case for D1
than the first draft stated.

Semantics when `force=true` **and** the file exceeds the inline limit:

- Return content inline, not a summary.
- Bound it by a new, deliberately generous ceiling `FORCE_MAX_BYTES` rather than
  `INLINE_BYTE_BUDGET`, so "force" is not quietly re-capped at the value it was
  overriding.
- If the file exceeds even `FORCE_MAX_BYTES`, return the first chunk **plus**
  `complete: false`, `next:` continuation, and the buffer id — never a bare outline.

`force=false` (default) keeps today's behaviour exactly. This is purely additive.

**Open decision for review — `FORCE_MAX_BYTES` value.** Trade-off: the session
context is large (1M), so a 60 KB file inline (~15 K tokens) is affordable; a 5 MB
generated file is not. Candidate: **262,144 bytes (256 KB, ~65 K tokens)**, ~26× the
default budget and still ~6% of a 1M window. Not yet pinned — Chris to confirm.

### 3.2 D2 — explicit counts + hint on every buffered summary

Populate in `read_full_file`, after the file-type `match` and before `Ok(result)`, so
**one insertion point covers all seven summary types** (source, markdown, json, yaml,
toml, config, generic):

```
result["complete"]    = false
result["shown_lines"] = 0
result["total_lines"] = <line count>
result["hint"]        = "Preview only — 0 of N lines shown. Full content in
                         <file_id>: read_file(\"<file_id>\", start_line=…, end_line=…)."
```

Renderer: add a counts line to `format_read_file_summary` (`read_file.rs:894-1041`).
The `hint` line already renders at `read_file.rs:1036-1038` (line numbers corrected
2026-08-08 — the first draft cited `:1035-1037`, off by one); the counts need one new
`push_str`.

**This is not a new convention — it already exists 13 lines below.** Verified
2026-08-08: the *under*-budget branch of the same function (`read_file.rs:701` onward)
already builds an `OverflowInfo { shown, total, hint }` with a tailored source-code
hint. So `read_full_file` implements the correct pattern for files that fit the byte
budget but have many lines, and omits it for files that blow the byte budget — the
more severe case. D2 makes the two branches consistent rather than introducing
anything new.

**Blast radius is small and verified.** `references(summarize_source)` returns exactly
2 hits — the definition and the single call at `read_file.rs:660`. Fields are purely
additive, so no existing consumer breaks.

### 3.3 D3 — allow read-only metadata commands

**Corrected 2026-08-08 after re-verification — the first draft of this section named the
wrong gate.**

The block is *not* a bounded/read-only command allowlist (that is the separate IL3
pipe gate). The real mechanism is `check_source_file_access`
(`src/util/path_security.rs:929-991`), invoked from
`src/tools/run_command/inner.rs:272-279` under `!buffer_only && !acknowledge_risk`.

It is a **two-part heuristic**: a segment is blocked only when its *first token* matches
a blocked-command regex **and** the segment also matches a source-extension regex.

```rust
// src/util/path_security.rs:825
const SOURCE_ACCESS_COMMANDS: &str = r"\b(cat|head|tail|sed|awk|less|more|wc|grep)\b";
```

`wc` is hardcoded in that list — which is why `wc -lc src/tools/output.rs` was blocked.

**Fix:** remove `wc` from `SOURCE_ACCESS_COMMANDS` (`path_security.rs:825`). `wc` emits
counts only and cannot return file content, so it leaks strictly less than `symbols`
already does.

**Do NOT touch `SOURCE_EXTENSIONS` (`path_security.rs:822`).** Verified two scopes: it
is consumed by `check_source_file_access:936` *and* by `is_source_path:995-1000`, which
gates `edit_file` multi-line source edits. Editing it would silently widen or narrow the
edit_file gate. `SOURCE_ACCESS_COMMANDS` by contrast has exactly one consumer
(`:933`), so changing it is contained.

**Counter-argument, for the reviewer:** `wc` may be listed deliberately, to discourage
shell-based reasoning about source files generally rather than to prevent content leak.
If that was the intent, drop this item — it is the lowest-value of the three fixes and
carries no dependency from D1 or D2.
### 3.4 Guidance layer — correct the tree claim

`~/.claude/engineering-standards.md` currently asserts the `tree`
depth cap applies *"even with `recursive=true`"*.

Verified at `src/tools/tree.rs:97-110`: the cap fires **only** when `recursive=true`
**and** no explicit `max_depth`. Default (no flags) is depth **1**. Both `max_depth=N`
and `detail_level='full'` bypass it entirely, and it self-announces.

The standing instruction is stricter than reality, which costs calls and teaches
distrust of a tool that is behaving correctly. Rewrite to state the actual rule, and
retarget the surrounding paragraph from "tree lies to you" to the accurate and more
useful "**pull the buffer before concluding**" — which is the real failure mode, and
the one D1/D2 make structurally harder to hit.

---

## 4. Non-goals

- Changing `MAX_INLINE_TOKENS` or any existing budget. The defaults are sound; 4.9%
  overflow is healthy behaviour, not a problem to tune away.
- Weakening the Iron Laws. `symbols`-over-full-read stays the default for source
  navigation; this only makes the documented override real.
- Touching `symbols` (11.8% overflow). Higher rate, but its summary is a legitimate
  navigational answer, not a stand-in for withheld content. Out of scope; revisit
  separately if desired.

---

## 5. Risks

| Risk | Mitigation |
|---|---|
| `force=true` on a huge file floods context | `FORCE_MAX_BYTES` ceiling + chunked continuation; never unbounded |
| New fields break a summary consumer | Additive only; `references` confirms 2 call sites total |
| Prompt-surface byte caps (2,200) | **Resolved 2026-08-08 — verified, not a risk.** Searched two scopes: `src/prompts/*.md` and `src/prompts/builders.rs`. Neither asserts anything about `tree`'s depth cap or `read_file`'s `force`. The only `force: true` hits are `onboarding(force: true)` (`source.md:157`, `workspace_onboarding_prompt.md:42`) — a different tool; the only `max_depth` hit is `call_graph` (`source.md:189`). No `src/prompts` change is needed, so the 2,200-byte cap and the shared-branch verify hazard do not apply. |
| Hint text drifts from real behaviour | Regression test asserts `complete=false` + `hint` present on an over-budget read |

---

## 6. Verification plan

1. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` — full gate.
2. New regression tests:
   - over-budget read **without** `force` → `complete=false`, `hint` present,
     `total_lines` correct;
   - over-budget read **with** `force` → actual content returned, not an outline;
   - file over `FORCE_MAX_BYTES` with `force` → content + `complete=false` + `next`.
3. `cargo rb`, then `/mcp` reconnect.
4. Live MCP verify: rerun the exact failing call from this session —
   `read_file("src/tools/read_file.rs", force=true)` — and confirm content, not an outline.

---

## 7. Sequencing

Design doc (this file) → review → implementation plan
(`docs/FULL_READ_FIDELITY_PLAN.md`) → review → code. No code before both are approved.

Bug files to open on approval: D1 and D2 as `kind: bug` under `docs/issues/`; D3 as a
friction note.

## 8. Integration on this machine — verified 2026-08-08

The first two drafts reasoned about source in the working tree without confirming that
the *running* server was built from it. That check is recorded here.

### 8.1 The running server matches the source read

| Fact | Value | How verified |
|---|---|---|
| Server invocation | `~/codescout/target/release/codescout start --debug` | `claude mcp list` — ✔ Connected |
| Path used | Direct release-target path, **not** the `~/.cargo/bin/codescout` symlink | same |
| Binary build time | 2026-08-04 12:46:25 (32,462,576 bytes) | `stat` |
| Source files newer than binary | **zero** | `find src -name "*.rs" -newer target/release/codescout` → empty |
| Worktree | main worktree @ `d7988aca [experiments]`; `.worktrees/native-retrieval-stack` @ `5e0fe184` is **not** the server's source | `git worktree list` |

Every `path:line` citation in this document is therefore against code that is live in
the connected server.

`--debug` is *"verbose logging + detailed usage recording"* (`src/main.rs:44-47`,
parsed `:220`, threaded `:243-251`). It does not affect output budgets or truncation.

### 8.2 The budgets are compile-time only — no config escape

Searched two scopes, both empty for budget-related config:

- `grep env::var("[A-Z_]+")` over `src/**/*.rs` → 39 hits across 20 files, **all**
  retrieval / embedder / Qdrant / LSP / librarian / probe. None touch inline budgets.
- Live `~/.config/codescout/.env` → 11 keys, all `CODESCOUT_{EMBEDDER,QDRANT,MODEL_DIM,
  QUERY_PREFIX,RERANKER,VECTOR_BACKEND,DISABLE_SPARSE}` / `LIBRARIAN_EMBED_*`.

The constants at `src/tools/core/types.rs:18-51` are `const` with no override path.
**Therefore this cannot be fixed by configuration — it requires a code change and a
rebuild.** `cargo rb` is verified as `build --release` (`.cargo/config.toml:19`; note
the file is modified-uncommitted, and the alias comment records that since 2026-08-04
it is equivalent to a plain `cargo build --release` because `server-stack` is now a
default feature). Ship path: `cargo rb` → `/mcp` reconnect.

### 8.3 Second enforcement layer: companion-plugin hooks

`read_file` is intercepted *before* codescout by a PreToolUse hook — a layer the first
two drafts did not account for.

- `hooks.json` registers `il4-deny-hook.mjs` on matcher `mcp__.*__read_file`.
- **It does not affect D1 or D2.** Verified by reading it: it exits early unless the
  path matches `/\.md$/i` (narrow — not `.markdown`, not `.mdx`), and it never
  inspects `force`, size, or `detail_level`.

Plugin location — **CLAUDE.md is stale on this machine.** CLAUDE.md says the companion
lives at `../claude-plugins/codescout-companion/`; that path does not exist here
(`ls -d` empty). It is marketplace-installed at
`~/.claude/plugins/cache/sdd-misc-plugins/codescout-companion/`, enabled via
`settings.json → enabledPlugins → codescout-companion@sdd-misc-plugins`.

Two versions are cached, `1.14.0` and `1.16.3`. I **cannot determine from here** which
is active — `~/.claude/plugins/config.json` does not exist and both dirs share an mtime.
This is immaterial: `diff -rq` across both `hooks/` dirs shows they differ in exactly
one file, `subagent-guidance.mjs` (a `SubagentStart` hook). Every hook relevant to this
design — `hooks.json`, `il4-deny-hook.mjs`, `pre-tool-guard.mjs`, `session-start.mjs` —
is byte-identical across the two versions.

### 8.4 Guidance surfaces — §3.4 is correctly scoped

Three surfaces inject standing guidance on this machine. Only one carries the incorrect
`tree` claim:

| Surface | Wiring | Carries the wrong `tree` claim? |
|---|---|---|
| `~/.claude/engineering-standards.md` | `settings.json → hooks.UserPromptSubmit → jq -n --rawfile ctx …` | **Yes — the only one** |
| companion `session-start.mjs` | plugin `SessionStart` | No — its only buffer text is the `run_command` pipe rule (`:306-310`), which is accurate |
| codescout `src/prompts/*` | MCP `server_instructions` | No — only `onboarding(force: true)` and `call_graph(max_depth=3)` |

**Consequence for §3.4:** because the hook reads the markdown at every prompt via
`--rawfile`, editing `engineering-standards.md` is a **pure content edit** — it does
not touch `settings.json` and needs no hook rewiring. Lower risk than the first draft
implied. It still applies to every project on this machine, so it remains Chris's call.
