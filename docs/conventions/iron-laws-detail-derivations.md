# iron-laws-detail — the wording the guide no longer carries

`src/prompts/guides/iron-laws-detail.md` is served to every repo. It keeps the gate text the
server really emits, the exceptions and the briefing rule. This file keeps what the guide shed
on 2026-11-11. The removed text is verbatim from commit `4f444dd51`
(`git show 4f444dd51:src/prompts/guides/iron-laws-detail.md`).

## Editing the guide: what pins it

- **Scope: repo-neutral.** `no_guide_names_a_path_or_field_only_this_repo_has`
  (`src/prompts/mod.rs`) fails on a codescout-only path or field.
- **Gate text is contract.** The quoted refusals are emitted by the server: the Iron Law 1
  overlap refusal (`src/tools/read_file.rs`), the Iron Law 2 refusal (`src/tools/edit_file/mod.rs`),
  `IL3 violation` (`src/tools/run_command/mod.rs`) and `shell access to source files is blocked`
  (`src/tools/run_command/inner.rs`). Cut none of them.
- **Tests that read the guide** (`src/prompts/mod.rs`): the guide must not say
  `allowed on bounded files` or `by path, not by command`, and must say `acknowledge_risk: true`;
  it must name every `SOURCE_ACCESS_COMMANDS` entry in backticks; it must list `capture_incomplete`,
  `filter_not_run` and `unfiltered_truncated` under Iron Law 3 and nowhere else.
- **Headings.** Rename none. Archived bug files cite `§ Iron Law 5`, `§ Iron Law 3` and
  `§ *Gate is overlap-based, not absolute*`.
- **`refusal_predicate()`** in `src/prompts/mod.rs` restates the IL1, IL2 and IL3 gate
  conditions. Change the guide's gate paragraphs and that function together.
- **No `serves:` lines.** The guide is pull-only, so the 2,500 B section cap does not apply.

## Iron Law 5: the managed-artifact paragraph

**Kept in the guide:** "the path alone decides nothing".

**Pre-cut text:** "the path alone (for example `docs/trackers/`) decides nothing".

## Iron Law 6: subagent dispatch — parent briefs

**Kept in the guide:** why a brief is needed (shared ledger without the companion stamp, a
separate ledger with it, so Iron Law 6 is the only channel that always delivers), and a pointer
to `get_guide("workspace-state")` for the ledger itself.

**Pre-cut text — the substrate paragraph:**

> **Substrate fact this compensates for:** without the companion stamp, the
> `guide_hints_emitted` ledger is shared across the parent and its subagents,
> persisted per session so most topics survive `/mcp` restarts (the
> session-opening topic is a deliberate exception — server construction
> re-arms it on every reconnect; see `get_guide("workspace-state")`).
> In that case, once the parent triggers a topic hint, NO subagent receives that
> hint independently — the ledger says "already delivered." With the stamp, each
> subagent has its own ledger and its own auto-injects. A brief cannot know which
> case applies, so Iron Law 6 is the only channel that always delivers parent-known
> context to subagents.

**Pre-cut text — the explicit-fetch bullet, first sentence:**

> - **An explicit `get_guide(topic)` always returns the full body**, ledger
>   state notwithstanding — `src/tools/guide.rs` never withholds it.

The claim is true: `src/tools/guide.rs` says "The body is NEVER withheld". The guide now says
"the server never withholds it" and leaves the path here.
