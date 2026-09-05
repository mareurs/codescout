---
id: ab6b2f115c1f36ef
kind: bug
status: fixed
title: 'BUG: the prompt-surface gate misses a backticked tool name in call form, and one has been served for four months'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`prompt_surfaces_reference_only_real_tools` (`src/server.rs:3645`) extracts candidates with
`` r"`([a-z][a-z_0-9]{2,})`" `` — the closing backtick must follow the identifier **immediately**.
A tool name written in **call form** inside backticks — `` `librarian_context(topic)` `` — matches
nothing, because the next byte after the identifier is `(`, not a backtick.

`src/prompts/source.md:352` has read

```
5. `librarian_context(topic)` → `doc(action=find, semantic="...")` → `doc(action=create)`
```

since `librarian_context` was retired into `librarian(action="context")` by the 2026-05-02
librarian-tools-collapse. That line is inside the `onboarding_prompt` slice — its rendered copy is
`tests/fixtures/prompt_surfaces/onboarding_prompt.md:312` — so **every onboarded agent has been told
to invoke a tool that has not existed for four months**, with the gate green the whole time.

## Symptom (Effect)

The onboarding prompt's "track decisions as artifacts" step names `librarian_context(topic)` as the
entry point. An agent following it issues an unknown-tool call and gets a hard `tools/list` miss on
its first artifact action — at onboarding, which is exactly when it has least context to recover.

## Reproduction

At `92fad220` on `experiments`:

```python
import re
gate = re.compile(r'`([a-z][a-z_0-9]{2,})`')          # src/server.rs:3707
call = '5. `librarian_context(topic)` -> `doc(action=create)`'
bare = '`librarian_context` is retired'
gate.findall(call)   # []                      <- escapes
gate.findall(bare)   # ['librarian_context']   <- would red
```

The token is backticked. The gate still sees nothing. `cargo test --workspace` is green.

## Environment

`experiments` @ `92fad220`. Both `source.md:352` and the gate are shared with `master`; this is not
branch-scoped exposure.

## Root cause

**The regex anchors on the delimiter rather than on the token boundary.** `` `ident` `` and
`` `ident(...)` `` are the same claim about `ident` — the second is a *stronger* one, since call
syntax says "invoke it like this" rather than merely naming it. The gate reads only the weaker form.

The repo already knows call form is the dangerous form: the guide-body gate at
`src/prompts/mod.rs:2034` matches on `"artifact("`, `"artifact_event("`, `"read_markdown("` … and
its own failure text (`:2138`) reads *"These say 'this is how you invoke the tool', and the tool does
not exist."* That gate is call-form-aware but **denylist-driven** — six names, all from the
2026-09-02 collapse. `librarian_context` was retired four months earlier and was never added.

So the two gates are blind in complementary directions and their intersection is empty:

| gate | knows call form? | checks the live registry? | misses |
|---|---|---|---|
| `prompt_surfaces_reference_only_real_tools` | no | **yes** | any retired name written as `name(` |
| guide-body denylist (`prompts/mod.rs:2034`) | **yes** | no (6 hardcoded names) | any retired name not on the list |

A name that is both *in call form* and *not on the denylist* is checked by neither. That is
`librarian_context(topic)`, and it is why four months passed.

## Evidence — the population, and what the count does NOT cover

Swept the served surfaces for backtick-delimited `name(` tokens not resolving to a registered tool
(21 names, taken from the running server's `tools/list`):

| surface | call-form non-tools | verdict |
|---|---|---|
| `src/prompts/source.md` | `librarian_context` | **real escape** |
| `tests/fixtures/prompt_surfaces/onboarding_prompt.md` | `librarian_context` | same occurrence, rendered |
| `tests/fixtures/prompt_surfaces/server_instructions.md` | — | clean |
| `.codescout/system-prompt.md` | `call`, `call_content`, `call_tool_inner` | Rust fn names in prose; surface is read by neither gate |

**Population: one token, one authored occurrence.** Stated with its unit because the value of the
fix is the gate, not the token.

Three things this sweep does **not** establish, named here so nobody credits it with them:

1. `build_system_prompt_draft` is the gate's third surface and is **generated at runtime** — the
   sweep read files and never saw it. Its call-form population is unmeasured.
2. It says nothing about names carrying *other* suffixes inside backticks (`` `tool.action` ``,
   `` `tool[x]` ``), which escape by the same mechanism and were not enumerated.
3. `.codescout/system-prompt.md` is read by neither gate, so it is clean today by luck, not by guard.

## Hypotheses tried

1. **Hypothesis:** this session's `companion_hint.md` fix had failed to ship — the release binary
   showed 0 hits for `doc action=event_create`.
   **Verdict:** rejected, and the probe was the defect. The file writes `` `doc` with `action=find` ``;
   the pattern had been reconstructed from memory rather than read off the file. A positive control
   against the source killed it. (`R-3`: a search that finds nothing is evidence about the search.)
2. **Hypothesis:** this is a rediscovery of `4e4762b735deb392` (same regex, filed open).
   **Verdict:** rejected. That file is about tokens written **without** backticks — its Summary
   frames the population as *"tool names the author happened to backtick"*. This token **is**
   backticked and escapes anyway, so that sentence is incomplete rather than merely narrow. The two
   escapes also need different remedies: anchoring on `(` does not reach Iron Law 4, whose text is
   `read_markdown (heading-addressed)` — a space before the paren.

## Fix

Add a **call-form** pass to `prompt_surfaces_reference_only_real_tools`: extract
`([a-z][a-z_0-9]{2,})\(` across each surface and require every hit to resolve to a registered tool.

Why this one is cheap to keep green, where dropping the backtick anchor is not (the objection
`6ccfcc15423f2ae5` correctly raises): **`(` is self-anchoring.** Ordinary English prose does not put
an open paren flush against a snake_case word, so the false-positive rate is near zero — the sweep
above found exactly three non-tool hits across four files, all Rust function names, all in a surface
the gate does not read. No allowlist growth is needed for the surfaces it does read.

Then repair the instance: `src/prompts/source.md:352`

```
-  5. `librarian_context(topic)` → …
+  5. `librarian(action="context", topic=…)` → …
```

and regenerate `tests/fixtures/prompt_surfaces/onboarding_prompt.md`.

**Done 2026-09-05.** The substitution landed as `librarian(action=context, topic="...")`, matching
the style of `doc(action=find, semantic="...")` already on that line. Three follow-on obligations,
all discharged:

- **`ONBOARDING_VERSION` 30 → 31.** Required, not optional: the changed line is in the
  `onboarding_prompt` slice, which produces the **stored per-project** system prompt, so
  already-onboarded projects keep serving the cached v30 text without a bump. A pin test
  (`tools::run_command::tests::system_prompt_points_to_tool_guide_resource`) asserts the literal
  value precisely so the bump cannot be silent; it moved with it.
- **The 1900-character cap was never at risk** — it governs the `server_instructions` slice, and
  this edit is in `onboarding_prompt`. Confirmed by `source_md_under_cap` staying green. The
  rendered onboarding surface grew 19119 → 19133 bytes, exactly the +14 of the substitution, which
  is also the check that no second change rode along.
- **The scoping decision.** The pass is `cfg!(feature = "librarian")`-gated; see § Tests added for
  the measurement that forced it.

**Not done, and deliberately so:** `.codescout/system-prompt.md`, this repo's own fourth surface, is
still v30. Regenerating it requires `cargo rb` + `/mcp` FIRST — `onboarding(refresh_prompt=true)`
read `v30 → v30` while the running server was the 10:39 binary, because `source.md` is
`include_str!`'d and therefore frozen at build time and again at process start. Refreshing before
the rebuild would have produced a plausible, committed, wrong artifact generated from the
pre-fix templates, with nothing to flag it.

Fix SHA: `bc2ea2ae35db0a1f6aedeed2dbe33698f8b0e628`
Patch-id: `28f808a00b3e53fc70f0b127ea5186190add4a13`

## Tests added

None new. The repair is a second extraction pass inside the existing
`prompt_surfaces_reference_only_real_tools`, and what makes it evidence is an **observed RED on the
production path**, taken twice, in opposite directions:

| lane | `source.md:352` | result |
|---|---|---|
| default | stale token restored | **FAILED** — `onboarding_prompt.md: ``librarian_context(`` is written as a tool CALL but names no registered tool`, and nothing else |
| default | repaired | ok |
| lean (`--no-default-features`) | stale token restored | ok — the pass is `cfg!(feature = "librarian")`-scoped, so this is the scoping working, not the guard missing |

The second row of that table is the one that would have been skipped, and it is the one that proves
the `cfg!` wrapper did not quietly disable the guard in the lane that matters. A green default lane
after the repair proves nothing on its own — it is the identical output of a gate that still cannot
see call form.

The lean row is a deliberate, named vacuity rather than coverage: under `--no-default-features` the
librarian tools are unregistered while the surfaces stay one `include_str!`'d constant, so the lean
registry is a strict subset of what the prose describes. Measured before scoping: **4 false
findings** (`doc(` ×2, `librarian(` ×2), **0 true ones**.
## Workarounds

When retiring a tool, sweep for the call form as well as the bare and unbackticked forms:

```
grep -rn '<tool>(' src/prompts/ .codescout/ tests/fixtures/prompt_surfaces/
```

## Resume

Wire the call-form pass, observe the RED, fix `source.md:352`, regenerate the fixture, re-run the
gate. Then re-read `6ccfcc15423f2ae5` § Summary — its population sentence needs the correction in
§ Hypotheses above, and its § Evidence table can take a fourth row.

## References

- Sibling escape through the same regex, opposite direction: `4e4762b735deb392`
  (`docs/issues/2026-09-02-the-prompt-surface-gate-is-backtick-scoped-so-the-iron-laws-are-invisible-to-it.md`).
  **Cite that id, not the one printed in the file's own frontmatter.** That file says
  `id: '6ccfcc15423f2ae5'`, which resolves to nothing — it is a worktree-minted id that survived the
  merge into the main checkout, and `doctor` reports it as one of 8 `frontmatter_id_mismatch` rows
  (all dated 2026-09-02, all worktree-born). `fix="repair_frontmatter_id"` is the wired repair; it
  rewrites each file's own `id:` line and does **not** reach prose elsewhere citing the dead value.
- Retirement that stranded the token: `docs/superpowers/plans/2026-05-02-librarian-tools-collapse.md:971`
  — `` `librarian_context` → `librarian(context)` ``.
- `CLAUDE.md` § *Parsers Over a Namespace* — a parser is correct on every input it accepts; the
  defect is the input it makes unrepresentable. Here the unrepresentable input is *"a tool name
  mentioned with its arguments"*.
- `CLAUDE.md` § *Testing Discipline* — "Ask which direction each test is monotone under, and mutate
  the *other* way." Both gates were mutated in the direction each already covered.
- Found 2026-09-05 during a post-rebuild freshness scout, from a probe that was itself wrong first.
