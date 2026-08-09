---
id: '464dce7fe5cd6a3c'
kind: spec
status: draft
title: Design — Field-aware project-root stripping at the call_content chokepoint
owners:
- marius
tags:
- post-process
- path-display
- output-fidelity
- cross-cutting-law
- call_content
- architecture
topic: path-display-and-output-fidelity
---

# Design — Field-aware project-root stripping at the `call_content` chokepoint

**Bug:** `docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`
(`ece908f37854e557`)
**Status:** draft — design accepted 2026-08-09, not yet implemented.

> **Reading this file through codescout's own tools will corrupt the absolute paths in
> it.** That is the defect being fixed. Verify literals with `run_command`.

## 1. Problem

`post_process` (`src/server.rs:527`) strips the project root from **all** text in a tool
result, using a one-character lookbehind to guess whether an occurrence is a path value
(`strip_prefix_from_text`, `src/server.rs:1702`). It runs *after* the tool's typed
`serde_json::Value` has been rendered to text, so the only signal left is that lookbehind.
It is not enough, and the transform fails in both directions:

- **Over-strips.** A quoted path literal inside file content is preceded by `"` — a value
  boundary — so it is rewritten. An edit keyed on the displayed text then fails, and the
  error's "Nearest content" hint is filtered through the same transform, making the
  mismatch unfalsifiable from inside the session.
- **Under-strips.** In a serialized buffer envelope a newline is the two literal characters
  `\` `n`; the lookbehind sees `n`, calls it a path character, and skips. 85% of measured
  leaks are this.
- **Erases.** The bare-root branch (added for `tree`) makes a value that *equals* the root
  collapse to `""` — measured 136 times across 12 sessions in `workspace.project_root` and
  the librarian's `scope.abs_path` / `scope.git_root`.

Full symptom/evidence/measurement record lives in the bug file. This document decides
what to build.

## 2. Why keep stripping at all

Measured on 51 transcripts (117,717 lines, 16,083 tool results, 21.7 MB of output):
the strip's **yield ceiling is 4.79%** of pre-strip bytes, realistically ~3–4% once
content-heavy tools (whose bodies cite relative paths natively) are discounted. That is
~450–700 KB over the corpus — enough that deleting the transform outright forfeits a real
benefit, and not so much that the current mechanism's correctness cost is tolerable.

**The measurement chooses between "strip on text" and "strip on structure" — not between
"strip" and "don't."**

## 3. Decision

**Decision:** Path stripping moves from a text transform at the server edge
(`post_process`) to an **allowlist-driven walk of the tool's `serde_json::Value`** inside
`Tool::call_content` (`src/tools/core/types.rs:546`), ordered strictly before
`format_compact` and buffering. `post_process` retains only the annotation banner.

**Context:** The strip is a display concern. The domain layers (catalog, index, LSP)
legitimately hold absolute paths; the transform belongs at the presentation edge. That
edge for tools is `call_content` — where buffering, `format_compact`, and guide injection
already live. The strip was the one output transform exiled downstream to the server,
which is precisely why it had to guess.

**Alternatives considered:**

| Alternative | Why rejected |
|---|---|
| **B — extend the `run_command` exemption to content-bearing tools** | Repeats the May 2026 axis error. Rawness is a property of a *field*, not a *tool*: `grep` and `symbols` interleave path headers with content in one blob, so a whole-tool carve-out forfeits real savings and still leaves both the `""` collapse and the 85% under-strip. The allowlist never converges — every new content-returning tool must remember to join it. |
| **C — delete stripping; relativize at each tool's source** | 3–4% is real. And it multiplies the number of places that must get relativization right instead of reducing them. |
| **Harden the lookbehind heuristic in place** | No lookbehind can separate a `"` before a path field from a `"` before a code literal. The information needed was destroyed one layer earlier. |

**Consequences:**

- *now easier* — a new content-returning tool is safe by default; buffered summaries strip
  correctly (recovering the 102 measured leaks, since the summary is built by
  `format_compact` from an already-stripped `Value`); the cross-workspace pin mismatch
  becomes unrepresentable; `tree` owns its own header and the global bare-root branch dies.
- *now harder* — a path key nobody enumerated keeps its absolute prefix, silently
  forfeiting savings (verbose, not wrong — but invisible without the gate in §6). Errors
  follow a second policy (never stripped), so there are two output rules instead of one.

**Change scenarios absorbed:**

1. A new tool returns file content → it names the field whatever it likes and is safe,
   because only allowlisted keys are touched.
2. A tool is called under a `workspace=` pin → the strip resolves from the same `ctx` the
   tool body did; there is no parameter to pass wrongly.
3. A result grows past the buffer threshold → the summary is built from already-relative
   values, so overflow no longer changes stripping behaviour.
4. A path-shaped field is added later → worst case is a longer response, never a corrupted
   one.

**Revisit-when:** measured yield drops below ~1% (delete it instead), or the path-key
allowlist exceeds roughly a dozen names with no shared naming convention — that would mean
the keys are not a real category and the boundary is drawn on a false distinction.

**Confidence:** high on boundary placement and the failure-direction inversion (§5.1);
**medium on effort**, pending the key inventory (§8).

## 4. Chokepoint integrity

Verified 2026-08-09 by `grep "fn call_content"` over `src/**/*.rs` — the law has exactly
three sites, which is where this project's `tool-registration-rule-of-three` discipline
says centralisation is earned rather than guessed:

1. **`Tool::call_content` default impl** (`src/tools/core/types.rs:546`) — carries the
   strip. Serves every tool, for both `OutputForm::Json` and `OutputForm::Text`, because
   both read the same `Value`.
2. **`Onboarding::call_content`** (`src/tools/onboarding.rs:294`) — the only override, and
   **benign**: every path it emits is a hardcoded relative string
   (`".codescout/tmp/onboarding-prompt.md"`, `format!(".codescout/tmp/{}", file_name)`).
   It opts out and loses nothing.
3. **`route_tool_error`** (defined `src/server.rs:1129`, invoked from the result-assembly
   arm at `src/server.rs:794`) — errors never produce a `Value`. See §5.3.

The chokepoint has what it needs: `ToolContext` carries both `agent` and
`workspace_override`, so the root resolves as
`ctx.agent.project_root_for(ctx.workspace_override.as_deref())` — the *same* pin the tool
body used. This is what makes scenario 3 above structural rather than tested-for: the pin
is ambient, not passed.

## 5. Mechanism

### 5.1 Allowlist only — the failure direction is the design

Walk the `Value`. Relativize a string **only** when its key is in the path-key allowlist
(`file`, `path`, `rel_path`, `abs_path`, `dir`, … — inventory pending, §8). Matching is
exact prefix equality on a real string: no boundary heuristic, no lookbehind, no
JSON-escape blind spot.

**The content-key list is prose in this document, never a branch in the code.** An
allowlist of path keys fails toward *verbosity* when a key is unknown; a denylist of
content keys fails toward *corruption*. This inversion — not the JSON walk — is what makes
the defect class extinct. Everything else is optimisation.

### 5.2 Root-valued keys stay absolute

`project_root`, `git_root`, `scope.abs_path`, `cwd` and peers keep their full absolute
path. They are the anchor everything else is relative *to*; stripping the anchor is what
makes the rest unreadable, and collapsing it to `""` is the measured 136-event data loss.
Cost is ~35 bytes on a handful of fields per response.

### 5.3 Errors are never stripped

`route_tool_error` produces text with no `Value`, and errors are a rounding error by volume
(`edit_code`: 98 KB across 634 results in the corpus). More importantly `not_found_msg`
(`src/tools/edit_file/mod.rs:196`) deliberately embeds **raw file bytes** as "Nearest
content" — the one string in the system that must be byte-faithful, since it exists to
resolve exactly the mismatch stripping causes. Leaving errors verbatim also closes the loop
where a diagnostic and its subject were filtered identically.

### 5.4 Ordering is an invariant, not an accident

**strip → `format_compact` → buffer.** The 102 leaked occurrences are fixed *only* because
the strip runs before the summary is built. Reversed, the leak silently returns. Today the
call order is pinned incidentally by
`call_content_uses_format_compact_in_buffer_summary` (`src/tools/core/tests.rs:485`); this
design makes it explicit and gives it its own test (§7). An ordering dependency between two
cross-cutting transforms that is recorded only in prose is a co-change contract with no
mechanism — the exact failure this project has already paid for three times.

## 6. Enforcement — corpus gate

The allowlist is a co-change contract; prose will not hold it. Add a test that runs a
fixture set of tool calls and asserts **no absolute project root appears in any rendered
output**, except `run_command` and error results. A forgotten path key then fails CI
instead of quietly costing tokens, and the same fixture pins the §5.4 ordering by
exercising both the inline and the buffered path.

This is the mechanism the three stale doc comments in §9 prove is necessary: each was an
accurate statement of intent that nothing enforced.

## 7. Tests

One per proven failure, plus the two invariants:

| Test | Pins |
|---|---|
| quoted root literal in file content survives `read_file` and `grep` | the reported bug (S1) |
| bare root in file content is not collapsed to `""` | the erasure case |
| `workspace(activate).project_root` is absolute | §5.2, the 21 measured events |
| `artifact(find)` `scope.abs_path` / `git_root` are absolute | §5.2, the 115 measured events |
| a buffered result's `summary` carries relative paths | §5.4 + the 102 leaks |
| `tree` still collapses its common prefix | no regression on `2026-07-18` |
| `run_command` output remains verbatim | no regression on `2026-05-21` |
| edit-failure "Nearest content" reproduces the file's real bytes | §5.3 |
| corpus gate: no absolute roots in any rendered output | §6 |

## 8. Open work — the key inventory

**Not yet verified:** that path-keyed fields use consistent names across all tools. `grep`'s
were inferred from a single leak sample, not enumerated. Establishing the allowlist is real
work with a method, not a known quantity:

```
grep -n '"file"\|"path"\|"rel_path"\|"abs_path"\|"dir"' src/tools/**/*.rs
```
plus the librarian adapter (`src/librarian/adapter.rs`), which routes `artifact` &co
through the default `call_content`.

If that inventory turns up more than ~a dozen distinct key names with no convention, the
Revisit-when trigger in §3 has fired and the boundary should be re-examined before
implementation.

## 9. Documentation corrections

Three surfaces currently assert the opposite of the code. All are load-bearing — each is a
statement someone relied on:

1. `src/server.rs:1702` — *"avoids stripping the prefix when it appears embedded inside
   longer strings such as code literals"*. False: `"` is a boundary character, so quoted
   literals are the most-exposed shape.
2. `src/server.rs:1662` — *"`project_root` … pass through unchanged"*. True when written;
   repealed by the bare-root branch added 2026-07-18. Neither change knew about the other,
   because they were coupled through a fact neither one named.
3. `get_guide("progressive-disclosure")` § *Path-relative annotation* — overstates the
   banner's cadence (it is novelty-gated to once per activation, `src/server.rs:558`) and
   recommends verifying via `read_file(@tool_xxx, json_path=…)`, a path that is itself
   stripped. Only `run_command` escapes.

Per this project's `agentic-surface-as-moat` discipline, these are LLM-facing surface
changes and carry more weight than the equivalent backend edit — they are what a future
agent will believe.

## 10. Deletions

- The bare-root branch in `strip_prefix_from_text`. It exists solely for `tree`'s
  common-prefix header; under field-aware stripping that is `tree`'s own rendering concern.
- The text strip in `post_process`, which keeps only the annotation banner.

## 11. Provenance

Origin: a field report from a sibling repo (`mirela/backend-kotlin`) where an edit keyed on
displayed text failed and only `run_command` + `od -c` could see why. Tracing it surfaced
the deferred residual in
`docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md`
§ Resume, recorded 80 days earlier in an archived file — a surface nothing re-reads. That
carry-forward failure is itself the argument for §6: the fix was known and unenforced.
