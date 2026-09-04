---
id: '0922dae6ee072f5a'
kind: bug
status: fixed
title: 'BUG: refresh_prompt stamps onboarding_version and returns version_stale:false before the prompt is regenerated, permanently consuming the staleness signal'
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
- onboarding
- prompt-surfaces
- observer-blindness
topic: onboarding version staleness signal
closed: 2026-09-04
fix_branch: experiments
fix_patch_id: 5a671249d5543de999512714d9ebebe3c916efdf
fix_sha: c79c629d99abc2736324b7a49d10804e6922dc28
opened: 2026-09-04
related:
- docs/issues/archive/2026-09-04-force-silently-discards-refresh-prompt.md
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

## Fix

Fixed on `experiments` at `c79c629d` — patch-id `5a671249d5543de999512714d9ebebe3c916efdf`
(the SHA orphans on the next rebase; the patch-id does not).

**Shape: a hash witness**, which is neither of the two directions suggested above.
`ProjectSection` gains `system_prompt_sha256` — the prompt's hash at *request* time,
i.e. the content the pending regeneration must supersede. `system_prompt_stale()`
treats a current hash that differs from that baseline as positive evidence the
regeneration happened, and `stamp_witnessed_refresh()` persists a fired witness. The
version is no longer written when a refresh is merely requested.

**Three corrections to this file's own analysis, all from the reproduction:**

1. **Four stamp sites, not two.** § *Mechanism* named `:497` and `:582`. It missed
   `perform_full_onboarding`'s tail — whose comment read *"Optimistic version write
   for full onboarding"* — and the fresh-config literal at `:925`. Full onboarding
   returns a `subagent_prompt` too, so it defers the prompt write exactly as the
   lightweight path does. A fix at the two named sites would have shipped this defect
   twice more, behind a passing test.
2. **Suggested direction 2 was unavailable as a mechanism.** It proposed stamping the
   version into `.codescout/system-prompt.md` itself. But **no production code writes
   that file** — every write is prose instructing a subagent to `create_file` it
   (`src/prompts/builders.rs:701`, `:909`), and the only `std::fs::write` calls to that
   path in the tree are in tests. A marker there would depend on subagent compliance:
   a policy, not a mechanism.
3. **The witness must PERSIST, or it is a slower form of the same defect.** A
   witnessed-but-unstamped regeneration reads correctly today and wrongly after the
   next `ONBOARDING_VERSION` bump — the stale baseline still differs from the current
   file, so the witness keeps vouching for a regeneration that predates the new
   templates. Both readers (the tool's re-entry and `build_activation_response`) now
   stamp, so it closes on whichever runs first.

**A trap avoided, recorded because it is this defect inverted.** § *Why this is
invisible…* notes that `project.toml` is gitignored while `system-prompt.md` is
tracked. That means a fresh clone arrives with **no baseline beside a present file**.
Reading that absence as a difference would have reported another machine's stale v29
prompt as current — the same false certification, relocated to clone time. A `None`
baseline is therefore *no evidence* rather than change, and the repo's pre-existing
`onboarding_triggers_refresh_when_version_stale` independently guards that line.

**Verification.** Gate green both lanes (`LEAN exit=0`, `DEFAULT exit=0`), with the 10
new tests read out of the run by name rather than inferred from a total. Mutation-
tested in three rounds, 16 observed REDs: `None => false` (the clone trap) reds 2 new +
2 pre-existing; inverting the witness reds 2 new + 2 pre-existing; disabling it in the
onboarding path reds exactly 7 — the six repaired fixtures plus the end-to-end witness
test, which is what establishes those fixtures are coupled to the mechanism rather than
cosmetically patched.

**Not exercised against a live MCP server.** This host's `.codescout/project.toml`
already stores version 30 == `ONBOARDING_VERSION`, so the witness short-circuits and
the stale path cannot be observed here without mutating that file. Coverage is
end-to-end through `Onboarding::call` / `call_content`, the real entry point; a live
check needs `cargo rb` plus an MCP reconnect.

**Six pre-existing fixtures had to be repaired**, and what they contained is worth
naming: they reported a fully-onboarded project while `.codescout/system-prompt.md` had
never been written — this very defect, sitting inside the fixtures meant to describe a
*completed* onboarding, because site 4 stamped at config creation. Repaired by
completing the flow (`simulate_subagent_prompt_write`), not by relaxing the assertions.

## Resume

Nothing outstanding.
