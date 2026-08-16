---
status: fixed
opened: 2026-08-16
closed: 2026-08-16
severity: high
owner: marius
related: []
tags: [usage-db, telemetry, session-attribution, concurrency, analytics-integrity]
kind: bug
---

# BUG: `usage.db` attributes every call to a shared per-project file, so concurrent sessions collapse into one `cc_session_id`

## Summary

`src/usage/mod.rs:100-104` resolves `cc_session_id` by reading
`.codescout/cc_session_id` — a single per-project file — and never consults
`CLAUDE_CODE_SESSION_ID`. `src/server.rs:235-247` resolves the *same* concept by
preferring the env var and falling back to that file, and its comment states the
reason in as many words: the env var is *"per-process, so concurrent CC windows
don't collide."*

Telemetry does not follow it. With two Claude Code sessions in one project, both
servers write whichever id the file happened to hold, so every row is attributed
to one session — usually the one that started last. Every analysis keyed on
`cc_session_id` over a multi-session period is therefore wrong, and wrong in a way
that looks like clean data.

## Symptom (Effect)

Measured 2026-08-16 at `b6bb6377`, from a session whose own id is
`28ea039a-830a-4237-b034-7d284dcf24f3`:

```
$ cat .codescout/cc_session_id
8a62140a-fbe5-4c78-82d4-86cbc63df35d          # a DIFFERENT session
$ stat -c '%y' .codescout/cc_session_id
2026-08-16 14:30:11 +0300                     # rewritten minutes earlier
$ env | grep CLAUDE_CODE_SESSION_ID
CLAUDE_CODE_SESSION_ID=28ea039a-830a-4237-b034-7d284dcf24f3   # present in the server's env
```

The telemetry shows the switchover as a clean cut:

```
cc_session_id                          calls  first (UTC)          last (UTC)
8a62140a-fbe5-4c78-82d4-86cbc63df35d     900  2026-08-16 08:13:12  2026-08-16 11:32:34
2c518eb6-45d3-415d-aebe-8335b96191da     415  2026-08-16 05:45:27  2026-08-16 11:26:37
28ea039a-830a-4237-b034-7d284dcf24f3     265  2026-08-16 06:25:17  2026-08-16 08:09:03
```

`28ea039a` stops recording at 08:09 UTC. That session did not stop working — it
is the session that produced this bug file, hours later. Its calls from 08:13
onward are inside the 900 attributed to `8a62140a`.

## Reproduction

With two Claude Code sessions open on the same project:

1. `cat .codescout/cc_session_id` — one value, shared.
2. In either session, `env | grep CLAUDE_CODE_SESSION_ID` — a different value,
   per process.
3. Query `SELECT cc_session_id, COUNT(*) FROM tool_calls GROUP BY 1` — the calls
   of both sessions carry the file's value.

## Environment

codescout `experiments` at `b6bb6377`. Two concurrent Claude Code sessions on
`/home/marius/work/claude/codescout`, both with `CLAUDE_CODE_SESSION_ID` set in
the MCP subprocess environment.

## Root cause

One concept, two resolutions, and only one of them was updated when the env var
became available.

- `src/server.rs:235-247` — env var, then the file, then a random uuid. The
  comment cites `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md`
  and memory `claude-code-mcp-env`, and explains that the env var is preferred
  *because* the file collides across concurrent windows.
- `src/usage/mod.rs:100-104` — the file, unconditionally. No env read, no
  fallback ordering.

So the fix for collision was applied at the guide-ledger call site and not at the
telemetry call site. The guide ledger is demonstrably correct here: this session's
ledger lives at `.codescout/guide_hints/28ea039a-….json`, its own id, while its
telemetry rows carry `8a62140a`.

measured 2026-08-16: the three commands under *Symptom*, plus
`grep(pattern="cc_session_id", glob="src/usage/mod.rs", context_lines=5)` and the
same on `src/server.rs`, read directly rather than inferred.

## Evidence

### The two resolutions, side by side

```rust
// src/usage/mod.rs:100-104
let cc_session_id =
    std::fs::read_to_string(project_root.join(".codescout").join("cc_session_id"))
        .ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
```

```rust
// src/server.rs:235-247
let cc_session_id = env.cc_session_id.clone()          // CLAUDE_CODE_SESSION_ID
    .or_else(|| /* .codescout/cc_session_id */)
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
```

### Blast radius beyond this session

Retention is 30 days, and any prior analysis that grouped by `cc_session_id` over
a window with concurrent sessions inherited the same defect — including
per-session counts, "sessions affected" figures, and any friction rate expressed
per session. The tool-usage investigation tracker (TU-N) and CAP-1's own
substrate check both lean on this column.

## Hypotheses tried

1. **Hypothesis** — the env var is simply absent in the MCP subprocess, so the
   file fallback is doing its job. **Test** — `env | grep CLAUDE_CODE_SESSION_ID`
   from inside a `run_command` spawned by that server. **Verdict** — rejected;
   the variable is present and holds this session's id. The file is being read in
   preference to it because `usage/mod.rs` never looks at the env at all.

## Fix

**FIXED 2026-08-16 in `06498ed2` (experiments), via option 2.** The server keeps
the id it already resolved and passes it to `UsageRecorder::new`; the file read in
`usage/mod.rs` is gone, so there is one resolution site instead of two. Not yet
archived — see *Tests added* for the gate caveat.

Options as written before the fix:

1. **Minimal:** make `usage/mod.rs` use the same precedence as `server.rs` — env
   var, then file, then None. Two lines, and it makes concurrent attribution
   correct going forward.
2. **Better:** thread the already-resolved `cc_session_id` from `server.rs` into
   the usage recorder so there is a single resolution site and the two cannot
   drift again. The current bug exists precisely because there are two.
3. **Consider:** mark historical rows as unreliable rather than silently
   trusting them — a `session_attribution` column, or a documented cutoff date in
   the tracker that consumes them.

## Tests added

`usage::content_tests::record_content_uses_the_passed_cc_session_id_not_the_file`
(`src/usage/mod.rs`). The fixture writes a **different** id
(`other-session-from-the-file`) into `.codescout/cc_session_id` and asserts the
passed id wins — so under the old implementation it would have returned the file's
value and failed. The test discriminates rather than merely passing.

**Gate is partial, and this file stays out of `archive/` until it is not.**
`cargo test --lib -- usage` — 56 passed, 0 failed, on exactly this code; that
target compiles the whole lib test binary, so every `UsageRecorder::new` call site
is verified to compile. `cargo fmt` clean. The full suite and
`clippy --all-targets` could NOT be run: a concurrent session was mid-refactor in
`src/librarian/tools/audit_doc_refs`, leaving 17 compile errors in files this fix
does not touch (verified theirs by `git diff` before concluding, and the errors
changed between runs, which is what in-flight work looks like).

Archive trigger for this file: re-run `cargo clippy --all-targets -- -D warnings`
and the full `cargo test` once that tree is green, then move it.
## Workarounds

For any per-session analysis over a multi-session window, group by `session_id`
(the MCP server's own id, which is per-process) instead of `cc_session_id`, and
treat `cc_session_id` as project-scoped rather than session-scoped. Note this
loses the ability to follow one CC session across an `/mcp` reconnect, which is
what `cc_session_id` was introduced to provide.

## Resume

Fix landed; two things remain.

1. **Re-run the full gate** (`cargo clippy --all-targets -- -D warnings`, full
   `cargo test`) once the concurrent `audit_doc_refs` refactor compiles, then
   archive this file via `artifact(action="move", …)`.
2. **Decide what to do about historical rows.** Everything recorded before
   `06498ed2` under a concurrent-session window is mis-attributed and cannot be
   repaired from the data — the correct id was never written. Options are a
   documented cutoff date in whatever tracker consumes the column, or a
   `session_attribution` marker. CAP-1's substrate check and any per-session
   friction rate read this column, so the cutoff needs to be stated somewhere a
   future analysis will see it.

Note for anyone measuring cross-session collisions (CAP-4): counts taken before
this fix UNDER-report, because two sessions writing the same file appeared as one
session writing it twice.
## References

- `src/usage/mod.rs:100-104` — the file-only resolution
- `src/server.rs:228-247` — the env-preferring resolution and the comment naming the collision
- `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md`
- memory `claude-code-mcp-env`
- `docs/trackers/capability-proposals.md` CAP-1 — its substrate check reads this column
