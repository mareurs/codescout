---
id: '15e370080b595b11'
kind: bug
status: open
title: 'BUG: refresh_prompt stamps onboarding_version and returns version_stale:false before the prompt is regenerated, permanently consuming the staleness signal'
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
- onboarding
- prompt-surfaces
- observer-blindness
topic: onboarding version staleness signal
opened: 2026-09-04
related: []
severity: high
---

## Summary

`onboarding(refresh_prompt=true)` writes `onboarding_version = ONBOARDING_VERSION` into
`.codescout/project.toml` and returns `"version_stale": false` **before** any regeneration
happens. The response only *instructs* a subagent to do the work. If that step is not completed —
the caller stops, the subagent fails, the session is interrupted — the tree is left with a v29
`.codescout/system-prompt.md` marked current, and **nothing ever reports it again**.

The correct path ends in an unsafe state. Following the instruction disarms the detector before
the repair runs.

## Reproduction — observed, not constructed

Session `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, 2026-09-04, laptop (`archlinux`), on
`experiments` at `0e25190d`.

1. `workspace(action="activate")` reported the staleness correctly:

   ```json
   "system_prompt_stale": {
     "stored_version": 29,
     "current_version": 30,
     "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
   }
   ```

2. `onboarding(refresh_prompt=true)` at 12:14 returned:

   ```
   "System prompt outdated (v29 → v30) — a lightweight refresh is needed.
    Spawn a general-purpose subagent with model=sonnet to regenerate the system prompt."
   ```

3. State on disk immediately after, before any subagent ran:

   | file | tracked? | state |
   |---|---|---|
   | `.codescout/project.toml` | **gitignored** (`.gitignore:14`) | `onboarding_version = 30`, mtime `12:14:07` |
   | `.codescout/system-prompt.md` | **tracked in git** | v29 content, mtime `10:25:33` (that morning's `git pull` checkout) |

4. The signal is gone. A subsequent `workspace(action="activate")` reports no
   `system_prompt_stale` block, because `onboarding_version_stale()` compares the stamped 30
   against `ONBOARDING_VERSION == 30`.

## Mechanism

`src/tools/onboarding.rs`, `handle_refresh_prompt` (462-520):

```rust
let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
config.project.onboarding_version = Some(ONBOARDING_VERSION);   // :497
let toml_str = toml::to_string_pretty(&config)?;
std::fs::write(&config_path, &toml_str)?;                       // :499
…
Ok(json!({
    "version_stale": false,                                     // :513
    "stored_version": stored_version,
    "current_version": ONBOARDING_VERSION,
    "subagent_prompt": subagent_prompt,                          // ← the work, not yet done
}))
```

The write at `:497`-`:499` and the assertion at `:513` both precede the only thing that would
make them true. `handle_already_onboarded` does the same at `:582`.

## Why this is invisible to the party best placed to catch it

The stamp lands in a **gitignored, machine-local** file; the artifact it certifies is **tracked**.
So:

- The author sees a successful call and a plausible instruction. Nothing in the response says
  "a certification has been written that you have not yet earned".
- `git status` shows nothing — the stamp is not tracked, and the stale prompt is *unmodified*.
- Every other machine stamps independently, so one host can be locally "current" while serving
  a prompt every other host can see is behind. There is no cross-machine reconciliation for it,
  unlike the catalog layers (`docs/conventions/cross-machine-catalog-resume.md`).
- `.codescout/system-prompt.md` is injected into **every** codescout session in this repo at
  project activation (CLAUDE.md § *Docs*), so a stale tool name here is served to every session
  while all three `src/prompts/` surfaces stay green and gated.

## Suggested direction (not a plan — reproduce first)

Two shapes, in preference order:

1. **Stamp on completion, not on instruction.** Move the write out of `handle_refresh_prompt`
   into whatever observes the regenerated file — e.g. stamp when `.codescout/system-prompt.md`'s
   mtime/hash changes after the call, or expose an explicit `onboarding(action="confirm_refresh")`
   the subagent calls last. This is the "correct path ends in a safe state" shape
   (CLAUDE.md § *Observer Blindness*, position 3).
2. **Stamp the artifact, not the config.** Put the version in `.codescout/system-prompt.md`
   itself (a comment or frontmatter line), so the certification travels with the thing it
   certifies and cannot diverge from it. This also fixes the per-machine divergence for free,
   because the file is tracked.

`"version_stale": false` at `:513` should not be emitted by a call that has not verified the
regeneration either way — reporting the *request* as the *outcome* is the assertion half of the
defect and is separable from the write.

## Scope note

This file covers the **eager stamp**. The sibling defect in the same tool — `force=true`
silently discarding `refresh_prompt=true` at `:293` — satisfies a different class
(`accepted-parameter-silently-dropped`) and is filed separately, per
`issue-clusters.md` § *Index*: *"if a finding satisfies a second class's claim, it is a second
bug file."*

## Resume

Not started. The v29→v30 content gap on this machine was repaired separately in the same
session (a subagent regenerated `.codescout/system-prompt.md`), so the *instance* is closed
while the *defect* is open — do not read a current prompt here as evidence the bug is fixed.

