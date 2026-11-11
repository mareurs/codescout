# project-activation-bootstrap — the wording the guide no longer carries

`src/prompts/guides/project-activation-bootstrap.md` is injected, unchanged, into every repo on
activation (`emit_session_opener` in `src/tools/core/emitters.rs` ships the file with no
per-repo substitution). A line that assumes this repo's layout is therefore wrong everywhere
else. The removed text is verbatim from commit `dd13ee219`
(`git show dd13ee219:src/prompts/guides/project-activation-bootstrap.md`).

## Editing the guide: what pins it

- **Size.** This guide is the session opener, so every cold session pays for it.
  `a_cold_p50_session_stays_under_the_committed_cold_emission_byte_ceiling` (`COLD_CEILING`,
  `src/server.rs`) had 12 B of headroom with the file at 2,792 B. A rewrite that added 202 B
  failed it by 190 B on 2026-11-11, so the neutral version had to be shorter than the old one.
  Re-run the test after any edit. Raising the ceiling is a spec amendment.
- **Scope: repo-neutral.** `no_guide_names_a_path_or_field_only_this_repo_has`
  (`src/prompts/mod.rs`) fails on a codescout-only path.
- **Companion copy.** `claude-plugins/codescout-companion/hooks/subagent-guidance.mjs` (lines
  92-107 on 2026-11-11) holds a separate copy of Phase 0, 1 and 2, including the bug-filter
  string. Change both together. That copy was not changed on 2026-11-11.
- **Short variant.** `project-activation-bootstrap-subagent.md` is what a subagent gets.
  `src/prompts/mod.rs` asserts on that file, not on this one.
- **Topic list.** The guide must keep naming `untrusted-content`; the pull-only reason for that
  topic depends on it.

## Before you plan or touch a contract — run reconnaissance

**Changed:** the body no longer assumes this repo's layout. The reconnaissance skill ships in the
codescout-companion plugin, not in the server. The server never writes
`docs/templates/session-log.md` into another repo. The skill itself files findings under
`docs/trackers/<topic>-session-log.md`, a path of that skill and not a requirement of the guide.

**Pre-cut text:**

> If you will write a plan, change a struct / function signature / API contract,
> or verify claims against `docs/trackers`, invoke the reconnaissance skill FIRST.
>
> - Claude Code: `/codescout-companion:reconnaissance`.
> - Other harnesses: follow `docs/templates/session-log.md`.
