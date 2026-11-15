---
kind: entry
status: open
title: After /mcp the client kept the old doc schema without collection while the server accepted it
created: 2026-11-15
entry_kind: friction
status_since: 2026-11-15
---

**Observed:** after `/mcp` reconnected codescout in a Claude Code session (server `git_sha 0529d8274`, confirmed by `workspace(action="status")`), `ToolSearch("select:mcp__codescout__doc")` still returned the old `doc` schema without the `collection` argument. A `doc(create, collection="context-migration", kind="lesson", …)` call sent anyway was accepted by the server and wrote the entry (`111c9f61a`). Before the reload, the same call without the argument in the schema failed with `missing field rel_path`, because the old server had no entry mode.

**Cost:** an agent that trusts the loaded schema concludes the entry mode does not exist and falls back to a hand-written file or gives up. The source of truth is the server: `src/librarian/tools/artifact.rs:355` declares `collection` on `experiments`.

**Workaround:** read `server.git_sha` from `workspace(action="status")`; if it is current, send the argument even when the client's cached schema lacks it. The migrating-trackers skill should say this in its binary/probe step.
