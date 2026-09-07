---
id: fede5ac714cf3c56
kind: bug
status: open
title: 'BUG: the stale-prompt banner tells the caller to run a call form the tool does not accept'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- prompt-surfaces
- onboarding
- remedy-text
closed: null
opened: 2026-09-07
owner: marius
severity: med
---

## Summary

`format_activate_project`'s stale-prompt banner says to run
`onboarding(action="refresh_prompt")`. There is no `action` parameter on the
`onboarding` tool — `refresh_prompt` is a boolean. The JSON field three lines above
it in the same function says the correct form, and carries a comment explaining why
the old one was wrong. The fix landed in the JSON and missed the formatter.

## Symptom (Effect)

The banner, rendered by `format_activate_project` (`src/tools/config/mod.rs`):

```
⚠ SYSTEM PROMPT STALE (v30 → v31): run onboarding(action="refresh_prompt") now.
```

The JSON field on the same result, observed live on 2026-09-07 from
`workspace(action="activate")` in this repo:

```json
"system_prompt_stale": {
    "stored_version": 30,
    "current_version": 31,
    "action": "Run onboarding(refresh_prompt=true) — tool names or signatures have changed."
}
```

Following the banner is not an error. `Onboarding::call` reads
`parse_bool_param(&input["refresh_prompt"])`; an input carrying only `action` leaves
that absent, so `refresh_prompt` is `false` and the call falls through to
`handle_already_onboarded` — which instructs `onboarding(refresh_prompt=true)`. The
caller recovers on a second round trip, having been told to do the wrong thing once.

## Reproduction

At `b600612f`, on `experiments`, with `.codescout/project.toml`'s
`onboarding_version` behind the compiled `ONBOARDING_VERSION`:

1. `workspace(action="activate", path="<this repo>")`
2. Read `system_prompt_stale.action` — correct form.
3. Render the same result through `format_activate_project` — dead form.

Step 3 is what a caller sees **instead of** step 2 whenever the response overflows;
see Root cause.

## Environment

Linux, `experiments` @ `b600612f`, MCP stdio, project codescout. The drift that
made the banner fire was `onboarding_version = 30` vs compiled `31`.

## Root cause

Two renderings of one fact, corrected in one place. The JSON field at the tail of
`build_activation_response` was fixed and commented; `format_activate_project`
composes its own sentence from `stored_version` and `current_version` and hardcodes
the remedy, so it never read the corrected string.

Reachability is narrow but real, and runs the wrong way. `workspace` does not
override `output_form()`, so it defaults to `OutputForm::Json` and the JSON is
returned inline in the ordinary case — which is why the live probe above showed the
correct form. `format_compact` is reached on **buffered overflow**, and
`src/tools/format.rs:49-50` states that path "shows the caller
`truncate_compact(format_compact(val), soft, hard)` and nothing else". So the one
surface that shows the banner is the one that shows *nothing but* the banner, with
the correct `action` string sitting unread in the buffer.

Measured 2026-09-07: live `workspace(action="activate")` returned the correct JSON
inline; the formatter's string was read at `src/tools/config/mod.rs` and the schema
at `src/tools/onboarding.rs` (`input_schema` declares `force` and `refresh_prompt`,
no `action`).

## Evidence

### The suite asserts the predicate and never the remedy

Three tests cover this banner —
`format_activate_project_prepends_warning_when_stale`,
`..._with_none_stored_version`, `..._no_warning_when_current`
(`src/tools/config/tests.rs`). Every assertion is about **whether the banner fires
and which version labels it carries**:

```rust
compact.starts_with("⚠ SYSTEM PROMPT STALE (v20 → v23):")
```

`starts_with` stops one character before the remedy. No assertion anywhere reaches
the call form, so the dead text is untested by construction and no mutation of it
reds anything.

Worse, two of the three fixtures **carry the dead form themselves**, in the
`action` field they never assert on:

```rust
"action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
```

So the stale text is preserved in the test data as though it were current, which is
where a reader would go to check.

## Hypotheses tried

1. **Hypothesis** — the stale warning never reaches a session at all (the alarm is
   on an unreached path). **Test** — called `workspace(action="activate")` live.
   **Verdict** rejected: it arrived, with the correct JSON form. The reason it was
   absent earlier in the session is that `workspace(post_compact=true)` is not the
   activation path. **Evidence** — the JSON quoted under Symptom.

## Fix

One-line correction in `format_activate_project` (`src/tools/config/mod.rs`) so the
banner names `onboarding(refresh_prompt=true)`, plus the assertion that was missing.

The assertion is the point, and it must be a **shape** assertion rather than a
pinned sentence: pinning the prose reds on every rewording and would rightly be
removed, but asserting the banner does not contain `action="` — and does name
`refresh_prompt=true` — reds exactly on the regression that happened and survives
a rewrite. Same reasoning as the `pre-push` guard's two-addressee assertion
(CLAUDE.md § Testing Discipline).

The two fixtures carrying the dead string get the live one, so the test data stops
vouching for it.

## Tests added

Pending.

