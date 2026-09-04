---
id: '64270d0be99ffe70'
kind: bug
status: fixed
title: 'BUG: onboarding(force=true, refresh_prompt=true) silently discards refresh_prompt and runs full onboarding instead'
owners:
- marius
tags:
- cluster/accepted-parameter-silently-dropped
- onboarding
- tool-surface
topic: onboarding flag precedence
closed: 2026-09-04
fix_branch: experiments
fix_patch_id: 5a671249d5543de999512714d9ebebe3c916efdf
fix_sha: c79c629d99abc2736324b7a49d10804e6922dc28
opened: 2026-09-04
related:
- docs/issues/archive/2026-09-04-refresh-prompt-stamps-the-version-before-the-work.md
severity: medium
---

## Summary

`onboarding(force=true, refresh_prompt=true)` accepts both flags, honours `force`, and
**silently drops `refresh_prompt`**. The response is a well-formed full-onboarding payload that
never mentions the requested refresh, so the caller has no way to learn their parameter was
ignored.

The two flags address different work (`force` = re-scan and re-author memories; `refresh_prompt`
= regenerate the system prompt from current templates), so a caller passing both is asking for
both, not choosing between them.

## Reproduction

Session `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, 2026-09-04, on `experiments` at `0e25190d`.
Project was at `stored_version: 29` against `ONBOARDING_VERSION: 30`.

`onboarding(force=true, refresh_prompt=true)` returned:

```json
{
  "prompt_path": ".codescout/tmp/onboarding-prompt.md",
  "summary": "[bash, css, html, java, javascript, kotlin, markdown, python, rust, typescript] · workspace (2 projects)",
  "sections": ["1. ## Empty-Stub Convention (15 lines)", … 29 sections],
  "project_prompts": [ {codescout}, {codescout-embed} ],
  "synthesis_prompt_path": ".codescout/tmp/onboarding-workspace-synthesis.md",
  "instructions": "Onboarding required — this is a workspace with 2 projects. …"
}
```

No `stored_version`, no `current_version`, no mention of v29→v30, no note that a refresh was
requested. `.codescout/project.toml` was **not** stamped by this call (verified: mtime stayed
at its prior value until a later `refresh_prompt`-only call).

The immediately following `onboarding(refresh_prompt=true)` — same session, same state, one flag
removed — returned the correct thing:

```
System prompt outdated (v29 → v30) — a lightweight refresh is needed.
```

So the refresh was *available* and *needed* on the combined call, and was dropped.

## Mechanism

`src/tools/onboarding.rs`, `Onboarding::call` (285-304) — the whole body:

```rust
let force = parse_bool_param(&input["force"]);
let refresh_prompt = parse_bool_param(&input["refresh_prompt"]);

if refresh_prompt && !force {          // :293  ← the drop
    return handle_refresh_prompt(ctx).await;
}

if !force {
    if let Some(response) = handle_already_onboarded(ctx).await? {
        return Ok(response);
    }
}

perform_full_onboarding(root, ctx).await
```

`refresh_prompt` is guarded by `&& !force`, and there is no `else` branch, no warning, and no
field echoed in `perform_full_onboarding`'s response acknowledging the ignored flag.

Note the tool's own description documents them as independent capabilities, not alternatives:
*"force=true re-scans; refresh_prompt=true rebuilds the system prompt from templates."*
Nothing there implies precedence.

## Consequence

The failure mode is a **no-op that reads as success**, and it composes badly with the sibling
defect: a caller who asks for both, gets the full-onboarding path, then retries with
`refresh_prompt` alone, has now triggered
`docs/issues/archive/2026-09-04-refresh-prompt-stamps-the-version-before-the-work.md` — the
version is stamped current while the prompt is still stale. That is how the two defects were
found: in sequence, by a caller doing the obvious thing.

Full onboarding also has side effects the caller did not scope: this call rewrote two **tracked**
memory files, `.codescout/memories/onboarding.md` (a factual repair) and
`.codescout/memories/language-patterns.md` (**−90 lines**, deleting the Java / JavaScript /
Kotlin / Python / TypeScript sections). That deletion is a live instance of
`resume-cross-machine-catalog-restore.md`'s open `CM-6` — *"`memory(write)` has no shrink
guard"* — reached here through a flag the caller did not know was in control.
## Suggested direction (not a plan — reproduce first)

Three candidates, in preference order:

1. **Run both.** `force` then `refresh_prompt` is a coherent sequence — the full onboarding
   re-authors memories, and the refresh regenerates the prompt from them. This is probably what
   a caller passing both means.
2. **Refuse the combination** with a `RecoverableError` naming which flag to keep. Loud, cheap,
   and honest; costs the caller one round-trip.
3. **Echo the drop** — at minimum add `"refresh_prompt_ignored": true` plus a hint to the
   full-onboarding response, so the silence is broken even if the precedence stays.

(3) alone is the weakest: it fixes legibility, not the no-op. Prefer (1) or (2).

## Scope note

This file covers the **dropped parameter**. The eager version stamp in the same tool
(`:497`, `:513`) satisfies a different class (`record-asserts-an-unchecked-completion`) and is
filed separately, per `issue-clusters.md` § *Index*: *"if a finding satisfies a second class's
claim, it is a second bug file."*

## Fix

Fixed on `experiments` at `c79c629d` — patch-id `5a671249d5543de999512714d9ebebe3c916efdf`
(the SHA orphans on the next rebase; the patch-id does not). Same commit as the sibling
eager-stamp bug, because both live in `Onboarding::call`'s twenty lines.

**The reproduction changed the fix direction: report, do not refuse.** `force`
genuinely *is* a superset of `refresh_prompt` — `perform_full_onboarding` re-explores
the project *and* regenerates the system prompt, gated by
`onboarding_prompts_write_system_prompt_to_root_not_memory`. So the combination does
strictly more work rather than less, and refusing it would break a reasonable call
(*"give me everything fresh"*). The defect was never the precedence — it was accepting
a parameter, honouring a different one, and returning a well-formed payload that never
mentioned the substitution.

`call` now sets `refresh_prompt_subsumed` plus a note naming the lightweight
alternative. **And it says so on the compact line, not only in the JSON**
(`format_onboarding`), because a field no surface renders is decoration however
correctly written — CLAUDE.md § *Testing Discipline*, *loudness is a property of a
PATH*. The test asserts on the compact string for exactly that reason, and also that
the note is **absent** without the flag, since a marker always present distinguishes
nothing.

**Incidental find in the same surface, fixed here.** `build_activation_response` was
telling every session to run `onboarding(action="refresh_prompt")`. There is no
`action` parameter — `input_schema` declares only the booleans `force` and
`refresh_prompt`, and `call` reads `parse_bool_param(&input["refresh_prompt"])`. It
appeared to work only because the ignored flag fell through to `handle_already_onboarded`,
which instructed a refresh by a *different* route. That is why it survived: the wrong
call produced the right outcome, so nobody had a reason to look. Corrected to
`onboarding(refresh_prompt=true)`.

**Verification.** Gate green both lanes (`LEAN exit=0`, `DEFAULT exit=0`).
`onboarding_force_reports_that_it_subsumed_refresh_prompt` observed RED under a
mutation that emptied the compact note — it reported `"[rust]"` — confirming the
assertion reaches the rendered surface and not merely the JSON field.

## Resume

Nothing outstanding.
