---
kind: bug
status: investigating
tags:
- process-lifetime
- config-staleness
- index-state
- concurrency
opened: 2026-08-26
owner: marius
related:
- docs/issues/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
severity: high
unverified: 'Directions 2 and 3 remain unimplemented. Direction 1 (74dfbfca) is gate-green and mutation-verified but NOT yet live-verified: written_by only appears in index(action="status") once a sync has run through the new binary, so the first live read must follow cargo rb + /mcp + an actual index write. WHICH pid wrote any pre-2026-08-28 sidecar stays unprovable - the field fixes that going forward, not retroactively.'
---

# BUG: server processes on deleted binaries write their stale config into the live project sidecar

## Summary

Nine `codescout start` processes are running on this host, two of them since
**Aug 24 21:55** and **Aug 25 18:39**, and both are executing **deleted**
binaries. Neither carries a model env var, so each uses the compiled-in default
of whatever code it was built from. One of them re-indexed this project and
stamped `indexed_with_model: "all-minilm"` into `.codescout/index-state.json` —
a value that appears in no config file, no current source path, and no live
default. Shared per-project state is being written by processes whose code no
longer exists.

The vectors survived only by luck: the configured backend is llama-server, which
ignores the requested model name entirely. A zombie holding a `local:` model
would have written genuinely incompatible vectors into the same collection.

## Symptom (Effect)

`index(action="status")`, 2026-08-26, immediately after a `/mcp` reconnect:

```json
"indexed_with_model": "all-minilm",
"configured_model": "CodeRankEmbed",
"model_mismatch": { "indexed_with": "all-minilm", "configured": "CodeRankEmbed" }
```

The sidecar had been stamped `CodeRankEmbed` by an explicit CLI index run seven
minutes earlier. Something re-indexed in between and overwrote it.

## Reproduction

Not a recipe so much as an observation of the running host:

```
ps -eo pid,lstart,cmd | grep 'codescout start'
for p in <old pids>; do ls -l /proc/$p/exe; done
tr '\0' '\n' < /proc/<pid>/environ | grep CODESCOUT_EMBED
cat .codescout/index-state.json
```

## Environment

- Linux, branch `experiments`, HEAD `899c5212`
- Nine `codescout start` processes live; oldest two from Aug 24 and Aug 25
- Three Claude Code profiles on this machine (`~/.claude`, `~/.claude-sdd`,
  `~/.claude-kat`), each able to spawn its own server against the same repo

## Root cause

Four measured facts, 2026-08-26:

1. **The value is in nothing current.** `grep -rl 'all-minilm'` across `*.toml`,
   `*.json`, `*.env*` returns only `.codescout/index-state.json` itself. It is
   absent from `src/`'s defaults: `ProjectConfig`'s is `local:AllMiniLML6V2Q`
   and `RetrievalConfig::from_env` falls through to the same. It *is* present in
   the **retired** sqlite stores (`.codescout/embeddings/*.db`), i.e. it was the
   effective model in an earlier era.
2. **The old processes run deleted binaries.**

   ```
   pid 670141:  /home/marius/.../target/release/codescout (deleted)
   pid 1082945: /home/marius/.../target/release/codescout (deleted)
   ```

   Every `cargo rb` this session unlinked the inode they are still executing.

3. **Neither carries a model env var.** `pid 1082945` has no `CODESCOUT_EMBED*`
   at all; `pid 670141` has `CODESCOUT_EMBEDDER_URL`, `_PROTOCOL=llama-server`
   and `_MODEL_NAME`, but **not** `CODESCOUT_EMBED_MODEL` or
   `CODESCOUT_EMBEDDER_MODEL` — the two vars that actually set `model`. So each
   falls back to its own binary's compiled-in default.
4. **That default has moved.** `git log -S'all-minilm' -- src/ crates/` names
   `8ced0906` and `aa6bff1d`, so the string was live in code these processes
   predate.

**Mechanism:** a process's configuration is fixed at its start, and its *code* is
fixed at its build. A server that outlives both a config change and a rebuild
keeps answering — and keeps *writing* — from a world that no longer exists.
`write_index_state_with_dirty` is a whole-file overwrite of shared per-project
state, so the last writer wins regardless of which era it is from.

This is R-89's process axis with a write side. R-89 and `W-55` both concern a
stale process producing a wrong *answer*; here it produces a wrong *record*, in a
file other processes then read.


### Measured during the 2026-08-26 cleanup — the reaping mechanism, and a correction

Killing the pile produced better evidence than the filing did, and it **corrects** the
leak explanation above.

**`SIGTERM` reaps nothing.** All eleven stale servers were sent `SIGTERM`; **every one
survived**, and each needed `SIGKILL`. So the normal way to reap a process does not work
on this server, and any supervisor, script, or human reaching for `kill` will conclude it
worked and be wrong.

**But a clean client disconnect DOES reap.** The authoring session's own server
(`1455816`) vanished on the next `/mcp` reconnect without being signalled. So the exit
path is stdio EOF — the client closing the pipe — not a signal.

**Which makes the leak specific**, and narrower than "nothing reaps them": a session that
ends by closing its pipe cleans up after itself, and a session that dies *without*
closing it (client SIGKILL, crash, a `/clear` that reassigns the conversation, a
terminal closed out from under it) leaves a server alive **forever**, because the one
remaining lever — a signal — is ignored. That is why the survivors skewed old: they are
exactly the sessions that did not exit cleanly, accumulated over days.

**Scale, measured rather than estimated.** 14 processes matched `codescout start`.
**12 of 14 were on deleted binaries**, the authoring session's own included — so this was
not two stale outliers from Aug 24–25 but effectively *every* long-lived server on the
host executing code that no longer existed. After killing 11 and reconnecting, 4 remain,
all on live inodes.

**The control that validates the diagnosis.** One server (`2119451`, started 20:59) sat
alongside the zombies with the *same* env and the *same* repo but a **live** inode. Same
shape, healthy. That is what makes "deleted binary" the discriminator rather than "old
pid" or "missing env" — both of which also correlated, and neither of which is the
mechanism.

### Corrected 2026-08-26 — the `SIGTERM` handler already exists; the real gap is an unbounded LSP shutdown

The "exit on `SIGTERM`" framing below is wrong about current code, and it is worth stating
plainly rather than quietly rewriting: `src/server.rs`'s `shutdown_signal()` has installed a
`tokio::signal::unix::signal(SignalKind::terminate())` handler since `e4c70c8f` (2026-02-26,
six months before this bug), and it is correctly raced into the stdio server's main
`tokio::select!` loop. A `codescout start` process that receives `SIGTERM` does not ignore it.

What actually blocks the exit: after that `select!` resolves — by any arm, signal included —
the code unconditionally awaits `lsp.shutdown_all()` with **no overall deadline**.
`LspManager::shutdown_all` (`src/lsp/manager.rs`) drains every LSP client and calls
`client.shutdown().await` on each in sequence; `LspClient::shutdown` (`src/lsp/client.rs`) sends
a `"shutdown"` request and an `"exit"` notification — each internally bounded to ~30s per
attempt, with retries possible during LSP cold-start — before a reader-task join that itself has
only a 5s timeout. A wedged LSP client (or a `self.clients` lock held by something else that is
itself stuck) can block this chain well past what a human would wait before reaching for
`SIGKILL`. The process *did* receive and start handling `SIGTERM`; it just never got back out of
graceful shutdown to actually call `Ok(())` and exit. From outside, that is indistinguishable
from ignoring the signal — which is exactly what this bug's reproduction observed.

This does not touch directions 1 or 2 below, and does not explain **why** any particular client
was wedged (kotlin-lsp's known cold-start/circuit-breaker behavior per memory `gotchas` is a
plausible candidate, not a confirmed one for these specific zombies — the eleven original
processes were `SIGKILL`ed before their logs could be inspected, so the exact hang site for
*this* incident is not proven, only that the code path has no ceiling regardless).
## Evidence

### Why the vectors are nonetheless intact

Measured against the configured endpoint:

```
CodeRankEmbed          dim=768  first=0.078631
all-minilm             dim=768  first=0.078631
total-nonsense-model   dim=768  first=0.078631
```

llama-server ignores the `model` field and serves whichever gguf is loaded, so
every writer produced CodeRankEmbed vectors whatever it called them. The
`code_chunks` collection is uniformly 768-d Cosine, 610 434 points.

**This is luck, not a safeguard.** Had a zombie's default been a `local:` spec,
the backend would have been resolved *from that name* — fastembed, 384-d — and
the writes would have been genuinely incompatible. `CODESCOUT_MODEL_DIM=768`
would have caught the dimension change on the current process, but a zombie
without that env has no such pin.

### Measured 2026-08-26 23:38–23:40 — the first live `SIGTERM` test of `ca2b0226`

Three pre-rebuild servers were killed at the user's request. The result sorts them against
`ca2b0226` (22:32, *"bound LSP shutdown to a deadline so SIGTERM always reaps the
process"*):

| pid | started | vs `ca2b0226` | `SIGTERM` result |
|---|---|---|---|
| `3537046` | 23:25:45 | after | **exited cleanly** |
| `2139369` | 21:01:42 | before | hung — needed `-9` |
| `3007036` | 22:38:57 | after the commit, build unknown | hung — needed `-9` |

**One clean post-fix exit is the first positive datapoint for `ca2b0226`.** `3007036` is
deliberately counted neither way: a commit timestamp says nothing about when the binary was
rebuilt, and the binary that process loaded was overwritten by the 23:33:54 build, so which
code it ran is not recoverable.

**The handler was installed on both hangers and observed not to complete** — this file's
root cause seen directly rather than inferred. `/proc/<pid>/status` on both:

```
SigCgt: 0000000100014443    bit 14 set → SIGTERM is caught
SigIgn: 0000000000001000    SIGPIPE only
STAT:   Sl+                 still sleeping >60s after SIGTERM
```

**The observable is a hang, never a delayed exit — and that distinction picks the fix.**
If the handler sets a shutdown flag read only after the current stdin read returns, a
still-connected client means it never returns, so the handler is not *slow* but
structurally unable to run to completion while any client holds the pipe. A deadline
wrapped around the shutdown steps cannot help a handler that never reaches them; the remedy
would be a wakeup on the read. **Mechanism proposed by `codescout-77` and NOT verified
against the code path** — the process measurements above are observed, this paragraph is a
hypothesis about why, and the two should not be read at the same confidence.

**Consequence for this file's `unverified:` line.** Both long-lived servers running deleted
binaries — the exact population that caveat concerns — were killed at 23:39 and are gone.
Anything that needed reading out of a live one is no longer available; the window closed.


### Measured 2026-08-27 14:11 — a second harm channel, and the reconnect ritual is a race

Everything above concerns what a zombie **writes**. This is what one **answers**, which
is a separate channel with a separate blast radius — and the first measurement here with
a consequence attached rather than a process census.

**The consequence, at one instant.** After `5a7eb3e7` was committed and the release binary
rebuilt, two surfaces on this host disagreed about the same tool, same machine, same
corpus, same minute:

| surface | `cited_prefix_with_no_definer` | report total |
|---|---:|---:|
| `./target/release/codescout doctor --json` | 14 | 94 |
| `librarian(action="doctor")`, this session's MCP | **48** | **128** |

The CLI runs the freshly built bytes; the MCP server was still holding the inode it
started with. Nothing errors, nothing warns, and the stale number is the one that looks
authoritative — it comes from the tool the agent is told to use. A peer reading 48 would
have reasonably concluded the fix did not land.

**The census at 14:11:20**, `readlink /proc/<pid>/exe` per process:

```
PID       PPID      AGE         START                      STATE
3698153   1188076   14:35:43    Wed Aug 26 23:35:36 2026   STALE
3819561   3536641   14:24:30    Wed Aug 26 23:46:49 2026   STALE
397575    4149776   13:00:26    Thu Aug 27 01:10:53 2026   STALE
1753734   669510    05:59:04    Thu Aug 27 08:12:15 2026   STALE
2114123   2113539   05:24:27    Thu Aug 27 08:46:52 2026   STALE
2706473   2078094   04:13:39    Thu Aug 27 09:57:40 2026   STALE
2927980   2288300   03:41:44    Thu Aug 27 10:29:35 2026   STALE
3231468   3231135   02:51:28    Thu Aug 27 11:19:50 2026   STALE
3711234   3710909   01:45:19    Thu Aug 27 12:26:00 2026   STALE
3826646   3288897   01:30:35    Thu Aug 27 12:40:44 2026   STALE
513804    2684610   00:09:21    Thu Aug 27 14:01:57 2026   STALE
523970    2583124   00:08:27    Thu Aug 27 14:02:52 2026   STALE   ← this session's
```

**12 of 12 long-lived servers on deleted inodes.** Distinct `PPID`s, each a separate
`claude` process — these are per-session servers, not leaked children of one parent, so
the count tracks concurrent sessions rather than a spawn leak.

**The part that reframes the remedy.** PID `523970` is *this session's own server*. It
started at **14:02:52**, seconds after a deliberate `/mcp` reconnect performed precisely
to escape a stale binary. By **14:08:13** the on-disk binary had a new mtime and a new
inode (`151188549`); no build was run from this session after 14:02:52, so the rebuild
came from a peer in the same checkout.

**That server was current for 5 minutes 21 seconds.**

So "reconnect after you rebuild" is not a workaround, it is a **race** — it holds only
until the next peer's build, and in a three-session checkout under active development the
window is minutes. The operator-discipline framing (*remember to `/mcp`*) cannot close
this by construction: the event that invalidates your server is one you neither perform
nor observe.

**Why this is not the same finding as the sidecar one.** The sidecar harm is *write*-side
and persistent — a wrong value on disk that outlives the process. This is *read*-side and
transient, but it reaches an agent's reasoning directly, and it is invisible in exactly
the way the sidecar value was not: there is no artifact left behind to notice later. Both
directions in `## Fix` address the writer. Neither addresses the responder.
### Measured 2026-08-28 — zombification is a property of the BUILD LOOP, not of long sessions

This file has framed the population as long-lived processes drifting out of date — "since
Aug 24 21:55", "the oldest at 14h35m". That framing is wrong in a way that changes the
severity, and one line of the census says so:

```
ZOMBIE pid=4033882  started 07:42:16  →  deleted binary by 07:51:33
```

**Nine minutes.** Zombified not by age but by a routine `cargo rb` run from this session.
Every already-running server is invalidated the instant anyone rebuilds, so the exposure is
generated by the ordinary edit-build loop at whatever rate the host rebuilds — not by
sessions being left open. On this host that is several times an hour.

Full census, 2026-08-28 08:02:

| pid | started | binary | `CODESCOUT_EMBEDDER_PROTOCOL` |
|---|---|---|---|
| 2081534 | Aug 27 21:12 | **deleted** | `openai` |
| 2537187 | Aug 27 21:56 | **deleted** | `llama-server` |
| 2673924 | Aug 27 22:14 | **deleted** | `llama-server` |
| 3024371 | Aug 27 22:59 | **deleted** | `openai` |
| 3708928 | Aug 28 01:19 | **deleted** | `llama-server` |
| 4033882 | Aug 28 07:42 | **deleted** | `openai` |
| 4177476 | Aug 28 07:54 | live | `llama-server` |

**Six of seven on unlinked binaries** — and a second thing the earlier census could not
see. The Summary says "Neither carries a model env var, so each uses the compiled-in
default." No longer true: they now carry **different protocols**. Three speak `openai` and
four speak `llama-server`, concurrently, against the same project and the same endpoint. So
the divergence is no longer only *code* a process was built from — it is *live config*
differing between processes that write the same sidecar.

The sidecar at measurement time read `last_indexed_at: 04:42:33Z` — 07:42:33 local,
**17 seconds after pid 4033882 started**, which is what sync-on-activation looks like.
Strong, not proof; answering it for every future write is precisely what `written_by` does.

## Hypotheses tried

1. **Hypothesis:** a config file somewhere sets `all-minilm`.
   **Test:** `grep -rl` across every `*.toml` / `*.json` / `*.env*` in the repo,
   plus `~/.config/codescout/` (absent) and the project's own `project.toml`
   (`model = "CodeRankEmbed"`).
   **Verdict:** rejected. It is in no config.

2. **Hypothesis:** it is a current code default reached when no env is set.
   **Test:** read `ProjectConfig::default_for` (`local:AllMiniLML6V2Q`) and
   `RetrievalConfig::from_env`'s fall-through; `grep 'all-minilm'` across `src/`.
   **Verdict:** rejected for *current* code — which is what made the deleted-binary
   check the next step rather than a flourish.

## Fix

*Not yet implemented, and the shape matters more than the patch.*

The instinct is to kill the old processes. That is cleanup, not a fix — it
restores the invariant until the next long session, and nothing reports when it
breaks again.

Two candidate directions, in preference order:

1. **Make the writer identify itself.** `IndexState` already records *what* model
   wrote it; recording *who* — pid, binary mtime or build id, and whether
   `/proc/self/exe` is deleted — turns "the last writer wins" into something a
   reader can adjudicate. A `status` that could say *"this sidecar was written by
   a process running a deleted binary"* diagnoses the whole class, not this
   instance.
2. **Refuse to write from a deleted binary.** A process whose `/proc/self/exe` is
   gone should decline to overwrite shared per-project state and log why, rather
   than winning the race. Cheap to check on Linux; needs a decision about the
   non-Linux path, where `/proc` does not exist.

Do **not** "fix" this by having the newest process re-stamp the sidecar on
connect. That makes the symptom intermittent — whichever process last reconnected
wins — which is strictly worse than a stable wrong value, because it defeats
exactly the kind of investigation that found this.


### Re-ordered 2026-08-26 after the cleanup — a signal handler now outranks both

The two directions above stand, but a third is cheaper and fixes more, and the
measurements put it first:

0. **[superseded, see the Corrected subsection above] Exit on `SIGTERM`.** Restated: a
   `SIGTERM` handler already existed (since `e4c70c8f`, 2026-02-26) and was correctly wired
   in — the actual gap was that the LSP-shutdown step it hands off to had no deadline, so a
   wedged client could block the process past the signal that asked it to exit.

0'. **[done 2026-08-26] Bound the LSP shutdown step to a deadline.** `shutdown_with_deadline()`
    wraps `lsp.shutdown_all()` with a 20s ceiling in the stdio server's shutdown path, so the
    process now guarantees it returns from `main()` — and therefore exits — within a fixed grace
    period after any `select!` arm fires (signal, idle timeout, or client disconnect), regardless
    of what hangs inside LSP shutdown. Does not fix the underlying LSP-wedge cause (out of scope
    here); makes it survivable. **SHA:** `ca2b0226` (`experiments`). **patch-id:**
    `fd44c45488695a8870ddc6080520ee8a3b5a7119`.

With `SIGTERM` honoured, the remaining exposure is only the window between a client dying
uncleanly and someone noticing — which is what direction 1 (record *who* wrote the
sidecar) makes visible, and what direction 2 (refuse to write from a deleted binary) makes
harmless. All three compose; none replaces another.

**Still do not "fix" this by reaping on connect.** The reasoning above is unchanged and
the cleanup reinforced it: whichever process last reconnected would win, making the
symptom intermittent, which defeats exactly the investigation that found it.

### Added 2026-08-27 — a third direction, for the read side

Directions 1 and 2 both govern what a zombie **writes**. The 2026-08-27 measurement in
`## Evidence` shows a second channel they do not touch: a stale server **answers** with
stale code, and the answer carries no mark distinguishing it from a fresh one.

3. **Let a response declare the binary it came from.** The server already bakes a build
   SHA in at compile time — `codescout version` prints it. Surfacing that on a response,
   or at minimum on `workspace(action="status")`, lets a caller compare what answered
   against what is on disk. This is direction 1's idea (*make the writer identify
   itself*) applied to the responder, and it composes with both: additive, no behaviour
   change, and diagnosable from inside the session rather than requiring a `/proc` walk.

   Not costed yet, and two questions come first. **What is the trigger?** Stamping every
   response is noise; stamping only when the on-disk binary differs from the running one
   means a `stat` per response, or a cached check with its own staleness. **And is
   "differs" even the right predicate** — a peer's rebuild of unrelated code makes every
   other server "stale" while its answers stay correct, which is precisely the situation
   measured on 2026-08-27, so a naive warning would cry wolf continuously in the
   multi-session case it exists to serve.

   What the measurement does establish, independent of the design: **the operator-discipline
   remedy is not available.** "Reconnect after rebuilding" cannot work when the
   invalidating build is a peer's — this session's own server went stale 5m21s after a
   reconnect performed for exactly this reason. Any fix has to be in-band.
### Done 2026-08-28 — direction 1 shipped, and its stated blocker was not one

**SHA:** `74dfbfca` (`experiments`). **patch-id:**
`e9d6780b06fb9004067a0e573c80195b719b9d11`.

`IndexState` now carries `written_by { git_sha, git_dirty, pid, exe_deleted }`, written by
`write_index_state_with_dirty` (the single whole-file writer, as the Resume said), and
`index(action="status")` surfaces it when the recorded sha differs from the reading
binary's own.

**The Resume's blocker — "Both need the non-Linux answer for `/proc/self/exe`" — does not
survive re-costing.** Direction 1 wants three things and only the third touches `/proc`.
`CODESCOUT_GIT_SHA` and `CODESCOUT_GIT_DIRTY` are already baked in by `build.rs`, and
`std::process::id()` is portable. So the two portable fields shipped and `exe_deleted`
became an `Option<bool>` that is `None` off Linux — a bonus, not a gate. A sidecar stamped
with a sha different from the reader's own proves a different build wrote it **on every
platform, with no `/proc` walk**. The rationale had priced the whole direction at its
non-portable garnish.

Reported as **facts, not a verdict**, and only on difference — the presence-means-a-problem
convention `model_mismatch` and `last_sync_skipped` already use. That is also the answer to
direction 3's cry-wolf objection, which applies here too: a peer's rebuild of unrelated code
makes every other server "stale" while its answers stay correct, so a flat warning would
fire continuously in the very multi-session case the field exists to serve. `exe_deleted` is
what separates *"an older peer wrote this"* from *"code that no longer exists wrote this"*,
and it is the one a reader should act on.

`exe_is_deleted` was verified empirically rather than reasoned about: a binary deleted while
running sees the kernel append `" (deleted)"` to `/proc/self/exe`, and
`std::env::current_exe()` does **not** strip it. The predicate conjoins that suffix with
`!exists()`, so a `stat` failing for an unrelated reason cannot report a live binary as
deleted.

**Still open:** directions 2 (refuse to write from a deleted binary) and 3 (let a response
declare its build). All three compose — and direction 1 makes the class visible, which is
the precondition for judging whether 2's behaviour change is worth its untestability.

## Tests added

For the 2026-08-26 `shutdown_with_deadline` fix (`ca2b0226`), two unit tests in
`src/server.rs`, both `#[tokio::test(start_paused = true)]` per this file's established
timing-test convention (real-clock sleeps flake under CI/wine — see
`docs/issues/2026-08-07-windows-ci-timing-flakes-block-the-gate.md`):

- `shutdown_with_deadline_returns_after_deadline_when_inner_never_resolves` — a
  `std::future::pending::<()>()` inner future; asserts the wrapper still returns at/after the
  20s deadline instead of hanging forever.
- `shutdown_with_deadline_returns_promptly_when_inner_resolves` — an immediately-ready inner
  future; asserts the wrapper returns well before the deadline rather than always waiting it out.

None yet for the still-open directions 1/2. The tractable unit there is a
`should_write_shared_state()` predicate over a `/proc/self/exe`-deleted signal, testable by
injecting the signal rather than by staging a real zombie.
## Workarounds

Kill the stale servers and re-index:

```
ps -eo pid,lstart,cmd | grep 'codescout start'   # find ones older than today
kill <pid>                                        # then: codescout index
```

Verify with `index(action="status")` — `indexed_with_model` should equal
`configured_model`, and `model_mismatch` should be absent.

## Resume

**[done 2026-08-26, `ca2b0226`]** the shutdown-deadline fix (0') is shipped and tested; a
process that receives `SIGTERM` now provably exits within ~20s regardless of LSP-shutdown
hangs. **Still open:** decide between the two Fix directions before writing code; direction 1 is
diagnostic and additive, direction 2 changes behaviour under a condition that is
hard to test in CI. Both need the non-Linux answer for `/proc/self/exe`. Start by
reading `write_index_state_with_dirty`
(`src/retrieval/index_state.rs`) — it is the single whole-file writer of this
state, so it is also the single place a guard would go.

Also worth a separate look, out of scope here: the concurrent `codescout start` count has
gone from **nine** (2026-08-26) to **twelve** (2026-08-27 14:11), with the oldest at
14h35m and **12 of 12 on deleted inodes**. Each has a distinct `claude` parent, so this
tracks concurrent sessions rather than a spawn leak — but nothing reaps them, and every
one of them answers tool calls with the code it was built from. See the 2026-08-27
Evidence subsection: that is now a measured wrong answer, not a hypothetical.

## References

- `bug-fix-session-log:W-55` — a peer's rebuild left a session answering from a
  deleted inode; `ls -l /proc/<pid>/exe` is the tell, and it is what closed this
- `reconnaissance-patterns:R-89` — freshness is a property of the copy that
  serves you, and it breaks on build, process, and distribution axes. This is the
  process axis with a write side.
- `docs/issues/2026-08-26-index-status-model-fields-dropped-but-still-documented.md`
  — the field that made this visible; it had been absent from the envelope for
  three and a half months
- `src/retrieval/index_state.rs` — `write_index_state_with_dirty`, the whole-file
  writer of the shared sidecar
