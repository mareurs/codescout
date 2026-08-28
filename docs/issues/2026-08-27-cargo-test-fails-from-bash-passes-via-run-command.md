---
kind: bug
status: fixed
tags:
- env
- test-harness
- bash
- run_command
- false-failure
- embedder
closed: 2026-08-28
opened: 2026-08-27
owner: marius
related:
- docs/issues/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md
severity: medium
---

# BUG: `cargo test` fails from native `Bash` but passes via `run_command` — a false red that reads as a code regression

## Summary
`tools::memory::tests::write_and_read_roundtrip` fails when `cargo test` is run
from a native harness `Bash` call and passes when the same command is run through
codescout's `run_command`. The failure names a dead port. Nothing about the code
differs between the two runs — only the environment the test process inherits.
The test **asserted** strict equality against `"ok"`, so an attached warning failed
it — which made an environment difference present as a code regression. Both halves
are now closed: the assertion (2026-08-27) and the stale env (verified 2026-08-28).

## Symptom (Effect)
```
thread 'tools::memory::tests::write_and_read_roundtrip' panicked at src/tools/memory/tests.rs:166:5:
assertion `left == right` failed
  left: Object {"status": String("ok"), "warnings": Array [String("semantic anchor
        creation failed: dense embed connect failed:
        http://127.0.0.1:48080/v1/embeddings — the dense embedder is unreachable
        (connect/timeout). ...")]}
 right: "ok"
```

Nothing listens on 48080. The dense embedder is `codescout-dense-amd`, healthy on
**48081** (`docker compose ps`, up 3 days). `48080` is a literal nowhere in `src/`.

## Reproduction
Reproducible on demand, in both directions, at `a9628a23`:

```bash
# FAILS — native Bash, nothing exported
cargo test --lib --features local-embed tools::memory::tests::write_and_read_roundtrip

# PASSES — same command through codescout
run_command("cargo test --features local-embed")     # 4739 passed / 0 failed
```

**Note the masking hazard added 2026-08-27:** `~/.cargo/config.toml` now carries
`[env] CODESCOUT_EMBEDDER_URL = { value = "http://127.0.0.1:48081", force = false }`,
so *any* cargo-driven run now passes. To reproduce, bypass cargo and invoke the
test binary directly (`target/debug/deps/codescout-<hash>`), or temporarily move
that config aside.

## Environment
- Project: codescout, branch `experiments`, commit `a9628a23`
- Profile: `~/.claude-sdd`, Claude Code, MCP stdio
- `CODESCOUT_EMBEDDER_URL` unset in every shell checked (login, non-login, harness)
- Ambient `CODESCOUT_EMBED_URL=http://127.0.0.1:48080/v1` present in the harness
  shell. **The second half of this line — that the same value was also in the MCP
  server's `/proc/<pid>/environ` — does not survive re-checking**, and it was the
  observation that wrongly exculpated this variable. See § Root cause, *The
  name-vs-value error*.

## Root cause

**CONFIRMED 2026-08-27 by additive environment bisect.** Two stale variables in
this machine's Claude Code profile settings, each **independently sufficient**:

```
CODESCOUT_EMBED_MODEL = all-minilm
CODESCOUT_EMBED_URL   = http://127.0.0.1:48080/v1
```

Both are `apply_embed_overrides` inputs (`src/config/project.rs`), documented
there as *"Env var overrides (highest precedence, override everything)"* — so they
override `project.toml`'s correct `CodeRankEmbed` / `48081`. Nothing listens on
48080 and the stack does not serve `all-minilm`.

**Source: `~/.claude-sdd/settings.json` and `~/.claude-kat/settings.json` `env`
blocks.** `~/.claude/settings.json` carries the *correct* current config — ten
`CODESCOUT_*` vars including `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081` and
`CODESCOUT_EMBEDDER_MODEL_NAME=CodeRankEmbed-Q4_K_M.gguf`. The two stale ones use
the older `EMBED_` prefix rather than `EMBEDDER_`, which is why they never
collided with the new block and were never noticed: three-profile config drift of
exactly the kind the machine's global CLAUDE.md warns about.

**Why `run_command` escaped it — CONFIRMED 2026-08-28, and it is not the
`load_startup_env()` story.** Two different files carry an `env` block for the
**same two variable names**, with opposite values, feeding two different process
trees:

| File | Feeds | `CODESCOUT_EMBED_MODEL` / `_URL` |
|---|---|---|
| `~/.claude-sdd/settings.json` → `env` | Claude Code → native `Bash` children | `all-minilm` / `…:48080/v1` — **stale** |
| `~/.claude-sdd/.claude.json` → `.mcpServers.codescout.env` | MCP server → `run_command` children | `CodeRankEmbed` / `…:48081/v1` — **correct** |

Measured: `run_command("env | grep -E '^(CODESCOUT|LIBRARIAN)_'")` returns 16 vars,
and 15 of them match `.claude.json`'s block exactly. So `run_command` was never
rescued by some *third*, corrective variable — it received **correct values for the
very same two variables** that were stale on the `Bash` side.

`load_startup_env()` is ruled out as the rescuer by reading its input file.
`~/.config/codescout/.env` (symlink → `.env.amd`) sets `CODESCOUT_EMBEDDER_URL`,
`CODESCOUT_EMBEDDER_MODEL_NAME`, `CODESCOUT_SPARSE_EMBEDDER_URL`,
`LIBRARIAN_EMBED_MODEL` and `LIBRARIAN_EMBED_URL` — and **not** the short-prefix
`CODESCOUT_EMBED_MODEL` / `CODESCOUT_EMBED_URL` pair that `apply_embed_overrides`
reads. It contributes nothing to either variable that decided this bug. (The 16th
var, `LIBRARIAN_ENABLED=1`, is in neither file; `src/server.rs` only ever reads it.)

**The name-vs-value error — the transferable lesson.** § Environment recorded the
stale var as "present in the MCP server's `/proc/<pid>/environ` too", and that
observation is what retired the variable as a suspect: if both sides have it, it
cannot be what distinguishes them. The comparison was on the **name**. By
**value** the two sides disagreed, and that disagreement *is* the mechanism.
Re-checked 2026-08-28 across 7 live `codescout start` processes: three carry
`CODESCOUT_EMBED_URL=http://127.0.0.1:48081/v1`, four lack the var, and **none
carries `48080/v1`**. So the original reading cannot be reproduced and may have
come from a zombie server predating the `.claude.json` block (§ References).
Either way the rule is the same: when a variable appears on both sides of a
divergence, `grep -c` answers the wrong question — diff the values.

**Why every earlier single-variable test "refuted" the hypothesis** — the finding
worth carrying forward. With two independently sufficient causes, removing either
one leaves the other, so one-at-a-time elimination returns a clean *negative* for
a variable that genuinely is a cause. `env -u CODESCOUT_EMBED_URL` still failed,
and that correct experiment produced a misleading answer three times in a row.
The bisect worked because it was **additive from a known-good base**
(`{HOME, PATH}` passes) rather than subtractive from the broken state — 7
halvings over 130 variables.

measured 2026-08-27: additive bisect via
`target/debug/deps/codescout-<hash>` invoked directly (bypassing cargo, whose
`[env]` block otherwise masks the failure); `{HOME,PATH}` → ok,
`+CODESCOUT_EMBED_MODEL` → FAILED, `+CODESCOUT_EMBED_URL` → FAILED, full env minus
both → ok.
## Evidence
### The refuted mechanism, recorded so it is not re-proposed
Originally committed (`c37c7c98`) as fact: "`load_startup_env()` populates the MCP
server's env at startup, so `run_command` children inherit
`CODESCOUT_EMBEDDER_URL` and native `Bash` children do not." Corrected in
`4bf5604b` — row 1 of the table above refutes it.

### Excluded by direct check
- `XDG_CONFIG_HOME` — unset, so the global config dir resolves to
  `$HOME/.config/codescout`, and `~/.config/codescout/.env` (symlink →
  `.env.amd`) does set `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081`.
- `CODESCOUT_ENV_FILE` — unset.
- No shell profile (`~/.bashrc`, `~/.bash_profile`, `~/.profile`) sets
  `CODESCOUT_EMBEDDER_URL`; a login shell does not have it either.
- Ambient `CODESCOUT_EMBED_URL=…:48080` is the obvious suspect and is **not** it:
  `env -u CODESCOUT_EMBED_URL` still fails, and it is present in the MCP server's
  environ too.

## Hypotheses tried
1. **Hypothesis:** `run_command` children inherit `CODESCOUT_EMBEDDER_URL` from
   `load_startup_env()`; `Bash` children do not. **Test:** `env -i HOME PATH bash -lc`.
   **Verdict:** rejected — a near-empty env passes, so absence is not the cause.
2. **Hypothesis:** ambient `CODESCOUT_EMBED_URL=…:48080` overrides project config
   at highest precedence (`apply_embed_overrides`). **Test:** `env -u CODESCOUT_EMBED_URL`.
   **Verdict:** rejected — still fails; also present in the server's own environ.
3. **Hypothesis:** the anchor path reads `LIBRARIAN_EMBED_URL`. **Test:** set it alone.
   **Verdict:** rejected — still fails.
4. **Hypothesis:** something PRESENT in the harness environment causes it.
   **Test:** additive bisect from the known-good `{HOME, PATH}` base, driving the
   test binary directly to bypass the `~/.cargo/config.toml` `[env]` mask.
   **Verdict:** CONFIRMED — two stale vars in profile `settings.json`, each
   independently sufficient. See § Root cause.
5. **Hypothesis:** `load_startup_env()` is what rescued `run_command` (the
   corrective-third-variable story). **Test:** read `~/.config/codescout/.env`, and
   diff `run_command`'s live env against `.claude.json`'s `mcpServers.codescout.env`.
   **Verdict:** rejected 2026-08-28 — that file never sets the short-prefix pair;
   `.claude.json` does, with correct values. See § Root cause.
6. **Hypothesis:** the stale var was equally present server-side, so it cannot be
   the discriminator (the reasoning that retired hypothesis 2). **Test:** read
   `CODESCOUT_EMBED_URL` per-pid from all live servers' `/proc/<pid>/environ`.
   **Verdict:** rejected 2026-08-28 — 0 of 7 carry `48080/v1`. The names matched;
   the values never did. See § Root cause, *The name-vs-value error*.

## Fix

**Both stale vars removed 2026-08-27** from `~/.claude-sdd/settings.json` and
`~/.claude-kat/settings.json`. `~/.claude/settings.json` left untouched (its 10
`CODESCOUT_*` vars are current and correct). Backups at
`<file>.bak-20260827` in each profile dir. Validated by `json.loads` before
writing, with the sibling keys (`ANTHROPIC_BASE_URL`,
`PUPPETEER_EXECUTABLE_PATH`) and the `permissions` block asserted intact.

**Not effective in the session that made the change; VERIFIED after restart on
2026-08-28.** Claude Code reads `settings.json` at startup and holds the vars in
its own process, so every child of the running instance kept them and the
direct-binary test still FAILED there. After the restart, from native `Bash`:

| Check | Result |
|---|---|
| `env \| grep -c '^CODESCOUT_'` | **0** |
| any var name containing `EMBED` | **none** |
| `settings.json` → `env`, `-sdd` / `-kat` | 2 keys each, **0** `CODESCOUT_*` |
| `settings.json` → `env`, `~/.claude` | 10 `CODESCOUT_*`, all current (`48081`, `CodeRankEmbed-Q4_K_M.gguf`) |
| `target/debug/deps/codescout-<hash>` run directly — no cargo, no pin | `test result: ok. 1 passed; 0 failed` |
| `ss -ltn`, then `POST :48081/v1/embeddings` | `48081` listening, **HTTP 200** in 5 ms; `48080` not listening |

The last two rows are what make this a verification rather than a coincidence.
Invoking the test binary directly bypasses cargo, so the `~/.cargo/config.toml`
`[env]` pin cannot be what carried the run; and with no `CODESCOUT_EMBED_*` var
set, `apply_embed_overrides` has nothing to apply, so `48080` — a literal that
appears nowhere in `src/` and only ever entered through that override — can no
longer reach the embed path at all.

One thing deliberately **not** claimed: that no warning attaches. The relaxed
assertion no longer distinguishes those cases, which is its whole point, so a green
run is not evidence either way. `48081` answering `HTTP 200` is the direct evidence
that the service is there.

The machine-wide mitigation from `f2bc544e` remains and is independent of this:
`~/.cargo/config.toml` `[env]` pins `CODESCOUT_EMBEDDER_URL` for every cargo
invocation (`force = false`), which is why `cargo test` passes here right now even
though the direct binary does not.

**Assertion shape fixed 2026-08-27** at `src/tools/memory/tests.rs`. This was the
in-repo half of the problem, and it is what turned a missing optional service into
something indistinguishable from a broken build.

`write` answers with a bare `"ok"` (the no-echo write convention) or with
`{"status": "ok", "warnings": [...]}` when an optional step attached one. The test
asserted `assert_eq!(result, "ok")`, which fails on the object form. It now reads
the status out of either shape and asserts on that alone, printing the full result
on failure:

```rust
let status = result
    .as_str()
    .or_else(|| result["status"].as_str())
    .unwrap_or_else(|| panic!("write returned neither a bare status string nor a `status` field: {result}"));
assert_eq!(status, "ok", "write should report ok; full result: {result}");
```

Verified against the failing condition itself: with both stale vars STILL present
in the session env, the direct-binary run now passes (it previously failed), so
the warning path no longer reads as a regression. Both branches are exercised —
the object form by that direct run, the bare-string form by a cargo run under the
`[env]` pin. Mutation-checked: swapping the expected status to `"MUTANT"` fails
with `left: "ok" right: "MUTANT"` and the full result in the message, so the
assertion genuinely fires rather than passing vacuously.

Gate after the change: clippy 0, `cargo check --no-default-features` 0,
`cargo test` 4739 passed / 0 failed.
## Tests added

**`write_and_read_roundtrip` itself is now the guard**, in the sense that matters:
it no longer conflates an unreachable optional service with a write failure, and
it is mutation-checked to still catch a genuinely non-`ok` status. That is the
only in-repo assertion this defect could ever have had.

No test can prevent the root cause — a user profile carrying a stale
`CODESCOUT_EMBED_*` override is machine config outside the repo. What was fixable
in-tree was the *diagnosis cost*, and that is what changed: the same
misconfiguration now produces a passing test with a warning attached rather than a
red gate that reads as a broken build.
## Workarounds
- Already in place machine-wide via `~/.cargo/config.toml` `[env]`.
- On a machine without it: `export CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081`,
  or `set -a; . ~/.config/codescout/.env; set +a`.

## Resume

**Nothing outstanding.** Steps 1 and 2 of the previous resume list completed
2026-08-28 — the post-restart measurements are in § Fix, this file is `fixed`, and
`unverified:` is dropped.

One optional call was made rather than left open: **the `~/.cargo/config.toml`
`[env]` pin stays.** With the stale vars gone AND the assertion fixed it is doubly
redundant, but it is inert when work routes through `run_command`, load-bearing
again the moment profile drift recurs, and lives outside every repo — so it costs
nothing and guards a failure mode that has now happened once.

Ready to archive to `docs/issues/archive/`. The fix pointer — SHA and patch-id for
both in-repo commits — is already recorded in § References, so the move owes
nothing further beyond re-pointing citations of this path.
## References
- codescout memory `gotchas` § "`cargo test` Fails From Native `Bash` But Passes
  Via `run_command` — Export `CODESCOUT_EMBEDDER_URL`" — the durable note, incl.
  the refuted mechanism and the installed remedy.
- `c37c7c98` (asserted the wrong mechanism), `4bf5604b` (corrected it),
  `f2bc544e` (installed the cargo `[env]` remedy).
- `src/config/global.rs` — `load_startup_env`, `plan_startup_env`,
  `global_config_dir_from`.
- `src/config/project.rs` — `apply_embed_overrides` (`CODESCOUT_EMBED_MODEL` /
  `CODESCOUT_EMBED_URL`, documented as highest precedence).
- `docs/issues/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
  (`investigating`) — the standing bug this one's process census corroborates: 7
  live `codescout start` processes here, started across 21:12 on 2026-08-27 through
  07:42 on 2026-08-28, with **disagreeing** exec-time env (3 carry
  `CODESCOUT_EMBED_URL`, 4 do not). Not re-filed; recorded here as a datapoint. It
  is also the most likely source of the § Environment reading this file now
  retracts.

**Fix pointer** — SHA *and* patch-id, recorded at fix time so neither promotion
path owes a reconciliation later:

| Half | Pointer |
|---|---|
| In-repo (assertion reshape, `src/tools/memory/tests.rs`) | `ee81b7e7` on `experiments` — patch-id `d358e38077534db47773dcf2cc20d9530271781e` |
| Resolution write-up (this file + memory `gotchas`) | `3805b08e` on `experiments` — patch-id `7fffb50004ad383166b3bf2e3f74c30d5f5876e6` |
| Root cause (two deletions from profile `settings.json`) | **no SHA by nature** — machine-local config outside every repo; backups `*.bak-20260827` |
