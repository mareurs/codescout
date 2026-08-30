---
status: fixed
opened: 2026-08-29
closed: 2026-08-29
severity: medium
owner: marius
related: []
tags: ["docker-compose", "retrieval-stack", "embedder", "env-config", "multi-machine-config"]
kind: bug
unverified: 'fix is to a gitignored .env file — no commit/SHA/patch-id exists for it; only the code-adjacent class of the bug (docker-compose reads an un-validated, git-ignored .env for a bind-mount path) is undocumented anywhere else'
---

# BUG: project-root `.env` carried multiple stale desktop-machine values, silently masked by ambient shell/MCP env

## Summary
The project-root `.env` (gitignored) carried **two** stale, desktop-machine-oriented
values: `CODESCOUT_MODEL_DIR=/home/marius/models` (doesn't correspond to where the gguf
files actually live, `<repo>/models`) and `CODESCOUT_RETRIEVAL_PROFILE=amd` (this laptop
has an NVIDIA GTX 1660 Ti, not an AMD/ROCm card — `amd` is the desktop machine's profile,
confirmed by the user). Both were masked for different reasons rather than one:
`CODESCOUT_MODEL_DIR` was overridden by an ambient shell export every interactive session
happened to carry; `CODESCOUT_RETRIEVAL_PROFILE` was masked simply because every
manual invocation this session used an explicit `--profile gpu` CLI flag (which overrides
any env-derived default) rather than the bare `./scripts/retrieval-stack.sh up|down`,
which resolves its profile from `${CODESCOUT_RETRIEVAL_PROFILE:-cpu}` — reading straight
from whatever `.env` the launching `claude` process loaded at startup (from its CWD, the
project root) and passed down to the spawned codescout MCP server's environment. The first
invocation that ran with a **clean, explicit** environment (a new systemd user unit added
to auto-start the GPU profile at boot, with `Environment=CODESCOUT_RETRIEVAL_PROFILE=gpu`
pinned explicitly) sidestepped both masks and — on its *second* trigger, after the first
`CODESCOUT_MODEL_DIR` fix — surfaced the `dense-gpu`/`reranker-gpu` model-load failure that
led to finding both stale values.

## Symptom (Effect)
`dense-gpu` and `reranker-gpu` (both `llama.cpp:server-cuda`) entered a crash-restart loop
(`docker compose --profile gpu ps -a` → `Restarting (1)`) immediately after being brought
up via `systemctl --user start codescout-retrieval-stack.service`. Container logs:

```
gguf_init_from_file: failed to open GGUF file '/models/CodeRankEmbed-Q4_K_M.gguf' (No such file or directory)
llama_model_load: error loading model: llama_model_loader: failed to load model from /models/CodeRankEmbed-Q4_K_M.gguf
...
main: exiting due to model loading error
```

and identically for `reranker-gpu`'s `/models/bge-reranker-v2-m3-Q4_K_M.gguf`. `docker exec
codescout-dense-gpu ls -la /models` (while the earlier, unrelated manual restart was
healthy) versus `ls -la /home/marius/models` on the host showed the latter as a freshly
Docker-auto-created **empty** directory (`total 0`, owned by `root`, `mtime` matching the
exact moment the systemd unit ran) — the classic Docker behavior of silently creating a
missing bind-mount source as empty rather than erroring.

## Reproduction
1. `git rev-parse HEAD` at time of discovery: `5de41c5a` (branch `experiments`).
2. Confirm the ambient masking: `env | grep CODESCOUT_MODEL_DIR` in an interactive
   `run_command`/shell session → `CODESCOUT_MODEL_DIR=./models` (correct, masks `.env`).
3. Confirm the actual `.env` value (project root, gitignored): `grep MODEL_DIR .env` →
   `CODESCOUT_MODEL_DIR=/home/marius/models` (wrong — no models were ever placed there).
4. Run `docker compose --profile gpu up -d` from a process with **no** `CODESCOUT_MODEL_DIR`
   in its environment (a systemd unit with `WorkingDirectory=<repo>` and no
   `Environment=CODESCOUT_MODEL_DIR=...` line reproduces this cleanly) → `dense-gpu` and
   `reranker-gpu` both fail to find their gguf files and crash-loop.

## Environment
Arch Linux laptop (GTX 1660 Ti, 6GB VRAM), Docker Compose v2 (`docker compose` CLI plugin),
codescout repo at `/home/marius/work/claude/codescout`, branch `experiments`, commit
`5de41c5a`. `.env` is gitignored (`.gitignore:38`, confirmed via `git check-ignore -v .env`)
— this is a **local machine config** bug, not a code/repo bug shared across clones.

## Root cause
`docker-compose.yml` mounts `${CODESCOUT_MODEL_DIR:-./models}:/models:ro` for
`dense-gpu`/`reranker-gpu` (grep confirmed: `docker-compose.yml:83` region). Docker Compose
resolves `${VAR:-default}` from (in precedence order) the invoking process's own
environment, then the project directory's `.env` file, then the inline default. The
project-root `.env` (not `.env.gpu`/`.env.cpu`/`.env.amd`, which all correctly say
`CODESCOUT_MODEL_DIR=./models`) carried a stale absolute path from some earlier machine
setup, never updated when models were colocated under `<repo>/models`. Because every
interactive shell used this session (and presumably every prior debugging session) already
had `CODESCOUT_MODEL_DIR=./models` exported ambient-ly, process-env precedence always won
over the wrong `.env` value — so the bug was invisible to every manual `docker compose`
invocation and only surfaced the moment something ran with a genuinely clean environment.

*Measured 2026-08-29: `docker exec codescout-dense-gpu ls -la /models` before the fix →
empty dir owned by root; after the fix → the four real gguf files, matching
`ls -la <repo>/models` on the host exactly.*

## Evidence
### Container crash logs (`docker logs codescout-dense-gpu --tail 40`, 2026-08-29)
```
gguf_init_from_file: failed to open GGUF file '/models/CodeRankEmbed-Q4_K_M.gguf' (No such file or directory)
...
main: exiting due to model loading error
```

### Host-side phantom directory (`ls -la /home/marius/models`, before fix)
```
total 0
drwxr-xr-x 1 root   root      0 Aug 29 20:04 .
drwx------ 1 marius marius 1252 Aug 29 20:04 ..
```
(Empty, root-owned, `mtime` = the exact moment the systemd unit's `docker compose up -d`
ran — Docker auto-creating a missing bind-mount source rather than erroring.)

### `.env` stale value (`grep MODEL_DIR .env`, before fix)
```
CODESCOUT_MODEL_DIR=/home/marius/models
```

### Post-fix verification (`docker exec codescout-dense-gpu ls -la /models`, after fix)
```
-rw-r--r-- 1 ubuntu ubuntu  90118048 Jul 25 11:13 CodeRankEmbed-Q4_K_M.gguf
-rw-r--r-- 1 ubuntu ubuntu 274181152 Jul 25 11:13 CodeRankEmbed-f16.gguf
-rw-r--r-- 1 ubuntu ubuntu 438376864 Jul 27 05:13 bge-reranker-v2-m3-Q4_K_M.gguf
```
Both `dense-gpu` and `reranker-gpu` reached `/health` → `200` within ~24s of the fixed
restart; `docker compose --profile gpu ps -a` showed all four GPU-profile containers
`Up ... (healthy)`.

## Hypotheses tried
1. **Hypothesis:** GPU VRAM contention (three concurrent CUDA services on a 6GB card).
   **Test:** `nvidia-smi` during the crash-loop.
   **Verdict:** rejected — 5.3GB free, nowhere near exhausted; the failure was a model-load
   error before any meaningful VRAM allocation, not an OOM.
2. **Hypothesis:** stale/corrupted image or a transient Docker registry issue.
   **Test:** none needed — the error is an explicit "No such file or directory" for a
   bind-mounted path, not an image-pull or checksum failure.
   **Verdict:** rejected on the log evidence alone.
3. **Hypothesis:** `.env`'s `CODESCOUT_MODEL_DIR` is wrong and only masked by ambient shell
   state. **Test:** read `.env` directly; check `/home/marius/models` on host.
   **Verdict:** confirmed — see Evidence.

## Fix
Edited the project-root `.env` (gitignored, local-machine file — **no commit exists for
this change**):
```
- CODESCOUT_MODEL_DIR=/home/marius/models
+ CODESCOUT_MODEL_DIR=./models
- CODESCOUT_RETRIEVAL_PROFILE=amd
+ CODESCOUT_RETRIEVAL_PROFILE=gpu
```
Removed the empty, Docker-auto-created `/home/marius/models` stale directory (`rmdir`,
verified empty first). Restarted via the actual systemd-unit path
(`systemctl --user restart codescout-retrieval-stack.service`) to validate the
`MODEL_DIR` fix against a clean environment rather than my own shell's masking export.

The `RETRIEVAL_PROFILE` fix could not be live-validated the same way within this session:
`.env` is read once by `claude` at its own startup and passed to the codescout MCP server
subprocess's spawn environment, so the *already-running* session's ambient
`CODESCOUT_RETRIEVAL_PROFILE` stays `amd` until the next MCP/`claude` restart — confirmed
by re-checking `/proc/<codescout-pid>/environ` after the edit, still `amd`. The on-disk fix
is correct and takes effect on the next restart; the systemd unit is unaffected either way
since it pins `Environment=CODESCOUT_RETRIEVAL_PROFILE=gpu` explicitly.

**SHA / patch-id:** N/A — `.env` is gitignored (`.gitignore:38`), so there is no commit for
this specific fix. The one code-adjacent risk this bug points at — `docker-compose.yml`
trusting an unvalidated, un-templated local `.env` for a bind-mount path with no existence
check — is not itself changed by this fix and remains a latent footgun for the next machine
setup; see Resume.

## Tests added
N/A — this is a local machine config value, not code; there is nothing in the repo to add
a regression test to. The mitigating control now in place is structural instead: the new
`~/.config/systemd/user/codescout-retrieval-stack.service` runs the whole GPU profile with
a clean, explicit environment on every login/boot, which will re-surface this class of
drift immediately (crash-loop within seconds) rather than letting it hide indefinitely
behind an interactive shell's ambient exports.

## Workarounds
Export `CODESCOUT_MODEL_DIR=./models` (or the correct absolute repo path) in any shell
before running `docker compose` manually, if `.env` should ever drift again.

## Resume
N/A — fixed and verified live (container mount contents + health checks + a live
`semantic_search` call after the fix). If this recurs on a different machine, check
`.env`'s `CODESCOUT_MODEL_DIR` first — the profile-specific `.env.gpu`/`.env.cpu`/`.env.amd`
files were never wrong, only the bare `.env` that `docker compose` auto-loads by default.

## References
- `docs/trackers/embedder-stack-ops-session-log.md` (F-1: the earlier, wrong VRAM-contention
  hypothesis from the same investigation this bug was found during).
- `docker-compose.yml` (`dense-gpu`/`reranker-gpu` service definitions, `${CODESCOUT_MODEL_DIR:-./models}` mount).
- `~/.config/systemd/user/codescout-retrieval-stack.service` (the clean-environment unit that surfaced this).
