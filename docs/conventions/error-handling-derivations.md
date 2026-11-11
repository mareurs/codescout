# error-handling — the wording the guide no longer carries

`src/prompts/guides/error-handling.md` is served to every repo. It tells a caller how to read
the two error paths. The paragraph for authors of a codescout tool is a contributor's API, so it
lives here. The removed text is verbatim from commit `4f444dd51`
(`git show 4f444dd51:src/prompts/guides/error-handling.md`).

## Editing the guide: what pins it

- **Scope: caller-facing.** Say what the model sees: `isError`, the `"ok": false` body, the
  `hint` / `warning` / `must_follow` guidance and the read-only `ack="@ack_…"` text. The server
  emits all of these (`src/server.rs`, `src/tools/core/types.rs`).
- **Pull-only.** `PULL_ONLY_GUIDE_TOPICS` in `src/prompts/mod.rs` gives the reason. Change the
  reason when the guide's purpose changes.
- **No `serves:` lines.**

## How to tell them apart

**Moved here:** the "For tool authors" paragraph. It tells an author when to return
`RecoverableError` and when to return `anyhow::bail!`, and names the constructors.

**Pre-cut text:**

> For tool authors: return `RecoverableError::new(message)` — or the
> `RecoverableError::with_hint(message, hint)` / `with_warning` /
> `with_must_follow` constructors to attach severity-tagged guidance, then
> chain `.with_extra(key, value)` for structured fields — when the failure
> is the model's fault, bad input it can self-correct. Return
> `anyhow::bail!` when it isn't.
