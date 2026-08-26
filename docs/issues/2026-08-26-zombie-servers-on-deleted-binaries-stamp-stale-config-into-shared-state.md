---
status: investigating
opened: 2026-08-26
severity: high
owner: marius
related:
  - docs/issues/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
tags: [process-lifetime, config-staleness, index-state, concurrency]
kind: bug
unverified: 'WHICH pid wrote the 14:29 sidecar is not proven — only that two long-lived servers run deleted binaries, carry no model env, and that no current config or code path produces the value written'
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

Also worth a separate look, out of scope here: **nine** concurrent `codescout
start` processes, two of them days old, is itself a leak. Three Claude Code
profiles on this host each spawn their own, and nothing appears to reap them.

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
