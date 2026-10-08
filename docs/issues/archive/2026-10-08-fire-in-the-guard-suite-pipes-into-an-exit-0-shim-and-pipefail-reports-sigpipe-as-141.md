---
kind: bug
status: fixed
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-10-08
opened: 2026-10-08
owner: marius
related: []
severity: low
---

# BUG: fire() in the pre-push guard suite pipes into a shim that exits 0 without reading stdin, so a lost race reports SIGPIPE as exit 141 under pipefail

## Summary
In `tests/pre-push-foreign-session-guard.sh`, the case "target vanished: does NOT block" intermittently failed with exit 141 instead of 0. `fire()` piped the hook line into a degrade-open shim that exits 0 without reading stdin; when the shim exits before the writer writes, the write gets SIGPIPE and `pipefail` reports 141 in place of the shim's 0. Test-harness flake only; the guard itself was correct.

## Symptom (Effect)
Seen once on 2026-10-06 by an implementer subagent running with a private `TMPDIR`: the case "target vanished: does NOT block" failed with exit 141 where 0 was expected. An immediate rerun passed. Two independent reviewers could not reproduce it in 16 idle runs (8 on the base, 8 on head).

## Reproduction
Idle machine, same pipe structure as `fire()`, against a stand-in shim that says why and exits 0 at once (`shim.sh`):

```bash
#!/usr/bin/env bash
# shim.sh -- stand-in for the degrade-open pre-push shim: says why, exits 0, never reads stdin.
set -uo pipefail
printf 'hook missing or not executable\n' >&2
exit 0
```

```bash
#!/usr/bin/env bash
# repro.sh REPO SHIM N -- mirrors fire(): the writer runs a command substitution first
# (git rev-parse), the reader (the shim) exits without reading stdin.
set -uo pipefail
REPO="$1"; SHIM="$2"; N="$3"; ZERO=0000000000000000000000000000000000000000
sha() { git -C "$REPO" rev-parse "${1:-HEAD}"; }
bad=0; ok=0; last=none
for ((i = 0; i < N; i++)); do
    OUT="$(cd "$REPO" && printf 'refs/heads/main %s refs/heads/main %s\n' "$(sha)" "$ZERO" \
        | CLAUDE_CODE_SESSION_ID=a "$SHIM" origin x 2>&1)"
    EC=$?
    if [ "$EC" -eq 0 ]; then ok=$((ok + 1)); else bad=$((bad + 1)); last=$EC; fi
done
echo "pipe form:        iterations=$N exit0=$ok nonzero=$bad last_nonzero_exit=$last"

# Fix shape: no pipe. The line is computed first and handed over as a here-string.
bad=0; ok=0; last=none
for ((i = 0; i < N; i++)); do
    line="$(printf 'refs/heads/main %s refs/heads/main %s' "$(sha)" "$ZERO")"
    OUT="$(cd "$REPO" && CLAUDE_CODE_SESSION_ID=a "$SHIM" origin x 2>&1 <<<"$line")"
    EC=$?
    if [ "$EC" -eq 0 ]; then ok=$((ok + 1)); else bad=$((bad + 1)); last=$EC; fi
done
echo "here-string form: iterations=$N exit0=$ok nonzero=$bad last_nonzero_exit=$last"
```

Load recipe (this is what exposes it for the REAL `fire()`): start 128 busy loops, `timeout 240 sh -c 'while :; do :; done' &` (128 of them, on a 64-CPU machine), and loop the failing case while they run.

## Environment
Linux 7.2.8-zen1-2-zen, bash, 64 CPUs, codescout checkout on `experiments`. Rate depends on CPU contention (see Evidence).

## Root cause
`fire()` (`tests/pre-push-foreign-session-guard.sh` around line 450 before the fix) ran, under `set -uo pipefail`:

```bash
OUT="$(cd "$REPO" && printf '...%s...\n' "$(sha)" "$ZERO" | CLAUDE_CODE_SESSION_ID="$ALICE" "$SHIM" origin git@example.invalid:x 2>&1)"; EC=$?
```

The writer evaluates `$(sha)` (a `git -C ... rev-parse HEAD`) BEFORE it writes. The degrade-open shim (target vanished) prints its message and exits 0 WITHOUT reading stdin. When the shim exits before the writer writes, the write hits a closed pipe (SIGPIPE, status 141), and `pipefail` makes the pipeline status the RIGHTMOST non-zero status: 141, not the shim's 0.

Only an exit-0 shim exposes it. For an exit-1 shim the rightmost non-zero status is the shim's own 1, which hides the SIGPIPE. That is why only "target vanished" flaked, while "target present: shim delegates" (the shim stub exits 1) and "target restored" did not.

measured 2026-10-08: the stand-in (exit-0, same pipe structure) exits 141 in 7 of 400 idle iterations while the no-pipe form gives 0 of 400; the real `fire()` under CPU load fails 53 of 3000 (see Evidence). The code reading was confirmed by those runs.

## Evidence
1. Stand-in shim, same pipe structure, idle machine: 7 of 400 iterations exit 141. The no-pipe form (line built first, passed as a here-string): 0 of 400.
2. The REAL `fire()` with the real shim, idle machine, 3000 iterations: 0 failures.
3. The REAL `fire()` under CPU load (128 busy `while :; do :; done` loops on 64 CPUs), 3000 iterations: 53 failed (1.8%).
4. Stand-in under the same load, 1500 iterations: pipe form 454 failed (30%), here-string form 0.
5. The other pipe in the suite, `run()` (around line 122: precomputed line, builtin `printf`, the reader is a subshell that forks `timeout`/`env`): the same structure measured 0 of 3000 failures. Latent only; left unchanged.

## Hypotheses tried
1. **Hypothesis:** the real shim behaves like the stand-in, so the failing rate is about 1-2% on an idle machine. **Test:** a 200-iteration loop added to the suite against the old `fire()` (356 passed, 0 failed), and 3000 real iterations on an idle machine. **Verdict:** rejected for the real code. The real shim runs its own `git rev-parse --show-toplevel` first, so it is slower and the writer usually wins when the machine is idle. **Evidence:** Evidence 2. The loop test was discarded because it could not fail.
2. **Hypothesis:** the race flips under CPU contention. **Test:** the same real `fire()` under 128 busy loops. **Verdict:** confirmed. **Evidence:** Evidence 3 and 4.

## Fix
`fire()` builds the line first with a command substitution and hands it to the shim as a here-string (`<<<"$line"`), so there is no writer to lose a race. Change is in `tests/pre-push-foreign-session-guard.sh` (`fire()`, line 450).

- **SHA:** `f66bb05a` on `experiments` (not pushed).
- **patch-id:** `1c13325f3b30f97a71113e3ad532bb76c6cd64fa` (`git show f66bb05a | git patch-id --stable`, verified 2026-10-08).
- Gate: `./scripts/gate.sh` on `f66bb05a` ended `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Tests added
A DETERMINISTIC case, not a statistical one, in `tests/pre-push-foreign-session-guard.sh` (around lines 490-503): a `git` wrapper on `PATH` sleeps 0.3s in exactly `git -C <dir> rev-parse HEAD` (the writer's call) and leaves a marker file, so the old pipe form fails every time. Assertions: "target vanished, writer slower than the shim: still exits 0 (no SIGPIPE)", "and it still said so", and the positive control "the slowed writer really ran".

Verified RED against the old `fire()` (356 passed, 1 failed: exactly the first assertion, the positive control passing) and GREEN after the fix (357 passed, 0 failed). A first attempt at a 200-iteration loop test was discarded because it could not fail (Hypotheses tried, 1).

## Workarounds
Rerun the suite; the failure is rare on an idle machine.

## Resume
N/A - fixed and verified on `experiments`.

## References
- `tests/pre-push-foreign-session-guard.sh` (`fire()`, `run()`).
- Fix commit `f66bb05a` (`experiments`), patch-id `1c13325f3b30f97a71113e3ad532bb76c6cd64fa`.
- `docs/issues/archive/2026-09-07-the-pre-push-guards-refusal-text-executes-its-own-example-commands.md` (the `run()` helper's comment cites it).
