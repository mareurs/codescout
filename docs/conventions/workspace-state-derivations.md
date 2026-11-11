# workspace-state — the wording the guide no longer carries

`src/prompts/guides/workspace-state.md` is served to every repo that runs codescout. It keeps
the contract: what `activate` does, which per-session sets survive it, and what a caller must
do. This file keeps the ledger internals that the guide shed on 2026-11-11, under the guide's
own headings. The removed text is verbatim from commit `4f444dd51`
(`git show 4f444dd51:src/prompts/guides/workspace-state.md`).

## Editing the guide: what pins it

- **Scope: repo-neutral.** `no_guide_names_a_path_or_field_only_this_repo_has`
  (`src/prompts/mod.rs`) fails if a guide names a codescout-only path or Rust field, such as
  `path_note_emitted_since_activation` or `src/main.rs`. Put such text here, not in the guide.
- **Headings.** Rename none. `§ Per-session state reset` is cited at two places in this guide
  (`What activate does` and `Anti-patterns`). The bare-citation floor
  (`checked >= 4` in `src/prompts/guide_index.rs`) counts them.
  `§ Per-call workspace pinning` is cited at `src/agent/mod.rs`.
- **No `serves:` lines.** The guide is pull-only and declares no section, so the 2,500 B
  section cap does not apply to it.

## Per-session state reset

**Kept in the guide:** the `guide_hints_emitted` table row (same-project re-activation keeps
the set, a project switch re-arms only `project-activation-bootstrap`, no hook means a full
clear, `post_compact` clears it unless the companion says otherwise, an explicit `get_guide`
and the first-touch auto-inject share one keyspace), and one sentence saying the set is keyed
by conversation identity.

**Pre-cut text — the table row, before it was shortened:**

> | `guide_hints_emitted` | With a companion rendezvous active, a **same-project re-activation keeps the ledger** — nothing is cleared, so nothing re-emits. A **genuine project switch re-arms only the project-scoped topic** (`project-activation-bootstrap`), leaving the tool-contract guides the model already holds in place. **Without a rendezvous, every activation still clears the whole ledger** — that blunt behaviour is retained deliberately: absent the companion hook a `/clear` is invisible to the server, and being surgical would silently starve the new conversation of guides it has never seen. It is also cleared on `workspace(post_compact=true)` (compaction re-arm) — unless the companion recorded that the conversation's last `SessionStart` was not a compaction (a plain `/mcp` inherits that record), in which case the call keeps it and reports `"ledger": "kept"`; **persisted per session**, so most topics survive `/mcp` restarts within one conversation instead of re-injecting guide bodies the conversation already holds — with one deliberate exception: server construction re-arms the session-opening topic alone on any non-empty reloaded ledger, so that one guide body is re-sent on every `/mcp` reconnect regardless of project. Written by **both** an explicit `get_guide(topic)` fetch and the first-touch auto-inject of a hint-carrying tool — one shared keyspace, so either path suppresses the other's re-emit. After a full clear or a scoped re-arm, the next touch of an affected topic re-emits. |

**Pre-cut text — the identity paragraph, removed except for its first idea:**

> **Identity behind `guide_hints_emitted`.** The ledger the table row above describes is keyed by **conversation identity**, not by project — it follows the conversation across activations and workspace switches within one MCP session, not whichever project happens to be active.
>
> - **Keyed tier** — when a conversation id is obtainable (Claude Code's `CLAUDE_CODE_SESSION_ID`, today), the ledger persists to disk under that id and survives `/mcp` reconnects within the same conversation, other than the session-opening-topic exception noted above.
> - **Anonymous tier** — when no identity is obtainable (every other client, or Claude Code before the companion hook has run), the ledger lives in-process only, is never persisted, and re-arms automatically after an idle interval — so a second conversation in a long-lived process isn't starved of guides forever.
>
> A companion hook (Claude Code only) can refresh the keyed tier's id mid-process through a pid-keyed rendezvous slot the server publishes at construction. That is how the server detects `/clear`, which mints a new conversation id without restarting the MCP subprocess. The server stays fully correct without the hook: absent it, the ledger degrades to the anonymous tier's idle-TTL behavior rather than silently suppressing guides the new conversation has never seen. This is the same flag the table above checks — a keyed conversation id on its own is not enough for the surgical re-arm; the hook must actually have stamped the slot at least once.

## Subagent semantics

**Kept in the guide:** the third bullet, now "The same path-relative banner flag".

**Pre-cut text:** "- The same `path_note_emitted_since_activation` flag" (the Rust field name).

## Cross-project workflow pattern

**Kept in the guide:** the three-step pattern, with the restore path written as
`/home/user/home-project`, because the old path named a project, not a neutral example.

**Pre-cut text:** `3. workspace(activate, path="/home/user/code-explorer")   # restore home`
