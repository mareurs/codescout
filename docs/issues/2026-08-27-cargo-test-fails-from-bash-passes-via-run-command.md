---
status: mitigated
opened: 2026-08-27
closed:
severity: medium
owner: marius
related: []
unverified: Root cause (two stale CODESCOUT_EMBED_* vars in ~/.claude-sdd and ~/.claude-kat settings.json) removed but NOT re-verified after a Claude Code restart — the running instance still carries them. The in-repo half (brittle strict-equality assertion) is fixed and mutation-checked. No regression test is possible for the root cause; it is machine config outside the repo.
tags: ["env", "test-harness", "bash", "run_command", "false-failure", "embedder"]
kind: bug
---

# BUG: `cargo test` fails from native `Bash` but passes via `run_command` — a false red that reads as a code regression

## Summary
`tools::memory::tests::write_and_read_roundtrip` fails when `cargo test` is run
from a native harness `Bash` call and passes when the same command is run through
codescout's `run_command`. The failure names a dead port. Nothing about the code
differs between the two runs — only the environment the test process inherits.
The test asserts strict equality against `"ok"`, so an attached warning fails it,
which makes an environment difference present as a code regression.

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
- Ambient `CODESCOUT_EMBED_URL=http://127.0.0.1:48080/v1` present in the desktop
  session AND in the MCP server's `/proc/<pid>/environ`

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

**Why `run_command` escaped it.** The MCP server additionally runs
`load_startup_env()`, which sets `CODESCOUT_EMBEDDER_URL=…:48081` from
`~/.config/codescout/.env`; that wins over both stale vars, and `run_command`
children inherit it. Native `Bash` children get the stale pair without the
corrective one.

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
   **Verdict:** in progress — this is the live hypothesis.

## Fix

**Both stale vars removed 2026-08-27** from `~/.claude-sdd/settings.json` and
`~/.claude-kat/settings.json`. `~/.claude/settings.json` left untouched (its 10
`CODESCOUT_*` vars are current and correct). Backups at
`<file>.bak-20260827` in each profile dir. Validated by `json.loads` before
writing, with the sibling keys (`ANTHROPIC_BASE_URL`,
`PUPPETEER_EXECUTABLE_PATH`) and the `permissions` block asserted intact.

**Not effective in the session that made the change.** Claude Code reads
`settings.json` at startup and sets the vars into its own process, so they persist
in every child of the running instance — confirmed: the direct-binary test still
FAILS here after the edit. Needs a Claude Code restart to verify.

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

1. Restart Claude Code, then confirm the root-cause removal took effect:
   `env | grep CODESCOUT_EMBED_` → expect empty.
2. Flip this file to `fixed` and drop `unverified:` once step 1 passes. Both
   halves are then done: the stale vars are gone from both profiles, and the test
   no longer misreports the condition.
3. Optional afterwards: decide whether to keep the `~/.cargo/config.toml` `[env]`
   pin. With the vars gone AND the assertion fixed it is doubly redundant, but it
   is harmless and guards against the same profile drift recurring.
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
