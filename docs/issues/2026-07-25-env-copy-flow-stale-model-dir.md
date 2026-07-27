---
id: '8a4db17751b1e233'
kind: bug
status: open
title: Copy-based `.env` profile flow drifts — local `.env` pins a nonexistent CODESCOUT_MODEL_DIR that compose reads by default
tags:
- docker-compose
- configuration
- docs
- retrieval
closed: ''
opened: 2026-07-25
owner: marius
related:
- docs/issues/2026-07-25-coderankembed-gguf-source-404.md
- docs/issues/2026-07-25-compose-gpu-profile-ampere-only.md
severity: low
---

# BUG: Copy-based `.env` profile flow drifts — local `.env` pins a nonexistent CODESCOUT_MODEL_DIR that compose reads by default

## Summary

`docs/manual/src/concepts/retrieval-stack.md:119` documents wiring the stack by
**copying** `.env.amd` to `.env`. A copy goes stale the moment `.env.amd` is
updated, and Docker Compose reads repo-root `.env` **by default** for variable
interpolation. On this box the copy now sets
`CODESCOUT_MODEL_DIR=/home/marius/models`, a directory that does not exist,
while the maintained profile files correctly leave the variable unset so
compose's `${CODESCOUT_MODEL_DIR:-./models}` default applies. A bare
`docker compose --profile <x> up -d` from the repo root would bind-mount a
nonexistent path, which Docker silently auto-creates as an empty directory —
so the dense embedder fails to find its GGUF rather than reporting a config
error.

## Symptom (Effect)

The stale value, from repo-root `.env`:

```
CODESCOUT_RETRIEVAL_PROFILE=amd
CODESCOUT_MODEL_DIR=/home/marius/models
```

That path is absent:

```
$ ls -la /home/marius/models
ls: cannot access '/home/marius/models': No such file or directory
```

The GGUFs actually live in the repo, fetched 2026-07-25 14:13:

```
$ ls -la ./models
-rw-r--r-- 1 marius marius 274181152 Jul 25 14:13 CodeRankEmbed-f16.gguf
-rw-r--r-- 1 marius marius  90118048 Jul 25 14:13 CodeRankEmbed-Q4_K_M.gguf
drwxr-xr-x 1 marius marius       190 Jul 25 14:01 CodeRankEmbed
```

The running container was **not** affected, because it was launched with an
explicit `--env-file` (which replaces the default `.env` for interpolation):

```
$ docker inspect codescout-dense-cpu --format '{{range .Mounts}}{{.Type}} {{.Source}} -> {{.Destination}}{{"\n"}}{{end}}'
bind /home/marius/work/claude/codescout/models -> /models
```

So this is latent, not currently breaking: it fires only on an invocation that
omits `--env-file`.

## Reproduction

```bash
git rev-parse HEAD    # 52fcaf0118d9a6388a8c5828f1447b818d05f360 (branch: experiments)
grep CODESCOUT_MODEL_DIR .env          # -> /home/marius/models
ls /home/marius/models                 # -> No such file or directory

docker compose --profile cpu up -d dense-cpu     # NOTE: no --env-file
docker inspect codescout-dense-cpu --format '{{range .Mounts}}{{.Source}}{{"\n"}}{{end}}'
# expect: /home/marius/models  (auto-created, empty)
docker compose logs dense-cpu
# expect: model /models/CodeRankEmbed-Q4_K_M.gguf not found
```

**Not run** — reproducing it means starting a container that will fail its
healthcheck and restart under `restart: unless-stopped`. Deferred as low value;
the config facts above are established directly.

## Environment

- Linux 7.1.4-arch1-1
- codescout `experiments` @ `52fcaf01`
- repo-root `.env` — gitignored (local secrets file, also holds
  `CARGO_REGISTRY_TOKEN`), last modified 2026-07-25 13:57, 1133 bytes
- `.env.amd` last modified 2026-07-25 18:17, 2181 bytes
- `~/.config/codescout/.env` → symlink → `/home/marius/work/claude/codescout/.env.amd`

## Root cause

Two config-delivery mechanisms coexist and disagree, and the documented one is
copy-based:

1. **`docs/manual/src/concepts/retrieval-stack.md:119`** — "copy `.env.amd` (in
   the repo root) to `.env`". A copy has no link to its source; editing
   `.env.amd` leaves `.env` stale with no signal.
2. **`docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md:99,106`**
   — `ln -s .env.amd ~/.config/codescout/.env`, explicitly justified: *"The
   symlink means `.env.amd` stays the single source of truth; switching
   profiles [is re-pointing it]."* This is the mechanism actually in place for
   the MCP servers, and it is drift-proof.

The repo-root `.env` follows mechanism 1 and has gone stale relative to
`.env.amd`, which was updated today as part of the GGUF-source fix
(`docs/issues/2026-07-25-coderankembed-gguf-source-404.md`).

**Verified this session:**

- Repo-root `.env` sets `CODESCOUT_MODEL_DIR=/home/marius/models`; that path
  does not exist.
- `.env.amd` does **not** set `CODESCOUT_MODEL_DIR` at all — line 16 mentions it
  only inside a comment, `# Models needed in ${CODESCOUT_MODEL_DIR:-./models} — fetch with:`.
  So the maintained profile is correct and relies on the compose default.
- `.env.gpu` sets `CODESCOUT_MODEL_DIR=./models` explicitly — also correct.
- `docker-compose.yml:77` is `- ${CODESCOUT_MODEL_DIR:-./models}:/models:ro`, so
  an unset variable resolves correctly and only an explicitly-wrong value breaks.
- The live container's mount source is `/home/marius/work/claude/codescout/models`,
  confirming the running stack did not use repo-root `.env`.

Secondary drift in the same file: `.env` lacks `LIBRARIAN_EMBED_MODEL` /
`LIBRARIAN_EMBED_URL`, which `.env.amd:49-50` and `.env.gpu` both set. Per
`docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md:25`
that exact omission caused the 2026-07-10 librarian outage — the librarian reads
those separately from `CODESCOUT_EMBEDDER_*`. Harmless today only because the
MCP servers read `.env.amd` through the symlink, not `.env`.

## Evidence

### Repo-root `.env` vs `.env.amd`

`cat .env` → `CODESCOUT_RETRIEVAL_PROFILE=amd`,
`CODESCOUT_MODEL_DIR=/home/marius/models`, endpoints 48081/48084/48083, no
`LIBRARIAN_EMBED_*`.

`grep(pattern="CODESCOUT_MODEL_DIR|RETRIEVAL_PROFILE|LIBRARIAN_EMBED", path=".env.amd")`
→ 4 matches: `:16` (comment only), `:26 CODESCOUT_RETRIEVAL_PROFILE=amd`,
`:49 LIBRARIAN_EMBED_MODEL=CodeRankEmbed`,
`:50 LIBRARIAN_EMBED_URL=http://127.0.0.1:48081/v1`. **No assignment of
`CODESCOUT_MODEL_DIR`.**

### Which file the live stack used

`docker inspect codescout-dense-cpu` → mount source
`/home/marius/work/claude/codescout/models`, i.e. the `./models` default, not
`/home/marius/models`.

### Config-delivery mechanism conflict

`grep(pattern="\\.env\\.(gpu|cpu|amd)", glob=["*.md","*.sh","*.toml","*.json"])`
→ 30 matches across 14 files, including
`docs/manual/src/concepts/retrieval-stack.md:119` (copy flow) and
`docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md:99,106`
(symlink flow, with the single-source-of-truth rationale).

## Hypotheses tried

1. **Hypothesis:** both `.env` and `.env.amd` carry the stale
   `/home/marius/models`.
   **Test:** `grep CODESCOUT_MODEL_DIR .env.amd`.
   **Verdict:** rejected — `.env.amd` never assigns it; only the comment at
   `:16` mentions the name. An earlier read of this session asserted both files
   were affected; that was wrong and narrows the bug to the local copy.
   **Evidence:** Evidence § repo-root `.env` vs `.env.amd`.

2. **Hypothesis:** the running dense container is already broken by this.
   **Test:** `docker inspect` mount source.
   **Verdict:** rejected — resolved to `./models`, so the launch passed
   `--env-file`. Latent, not active.
   **Evidence:** Evidence § which file the live stack used.

## Fix

Two parts; the second is the one that stops recurrence.

1. **Local, immediate:** drop the `CODESCOUT_MODEL_DIR` line from repo-root
   `.env` (letting the compose default apply) or correct it to `./models`. Add
   the missing `LIBRARIAN_EMBED_MODEL` / `LIBRARIAN_EMBED_URL` pair.
   **Do not `cp .env.gpu .env`** — repo-root `.env` also holds
   `CARGO_REGISTRY_TOKEN`, which the profile files do not carry, and a copy
   would destroy it. That hazard is itself an argument against the copy flow.
2. **Repo-side:** change `docs/manual/src/concepts/retrieval-stack.md:119` from
   copy to symlink, matching the mechanism the self-load-dotenv design already
   settled on and the reasoning at
   `docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md:106`.
   If repo-root `.env` must remain a real file because it carries secrets, then
   document that profile config belongs in `--env-file` / the
   `~/.config/codescout/.env` symlink and that `.env` should hold **secrets
   only** — the two concerns are currently mixed in one file, which is what
   makes the copy flow lossy.

## Tests added

N/A — documentation and local config. No Rust surface. `librarian(action="audit_doc_refs")`
would catch a stale *path reference* in the manual but not a semantically wrong
env value, so there is no existing gate to extend.

## Workarounds

Always pass `--env-file` explicitly so repo-root `.env` is never the
interpolation source:

```bash
docker compose --profile cpu --env-file .env.amd up -d
docker compose --profile gpu --env-file .env.gpu up -d
```

To confirm what a running container actually got:

```bash
docker inspect <container> --format '{{range .Mounts}}{{.Source}} -> {{.Destination}}{{"\n"}}{{end}}'
```

## Resume

Decide the split first: does repo-root `.env` hold secrets only, or secrets plus
profile config? That answer determines whether the manual's step becomes a
symlink instruction or an `--env-file` instruction. Then edit
`docs/manual/src/concepts/retrieval-stack.md:119` accordingly and fix the local
`.env` by deleting its `CODESCOUT_MODEL_DIR` line — verify with
`docker compose --profile cpu config | grep -A1 volumes` that the resolved mount
is `./models` before starting anything.

## References

- `docs/manual/src/concepts/retrieval-stack.md:119` — the copy-based flow
- `docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md:25,99,106`
  — symlink flow + the 2026-07-10 `LIBRARIAN_EMBED_*` outage
- `docs/superpowers/plans/2026-07-10-codescout-self-load-dotenv.md:318-319,359`
  — `ln -s` / `CODESCOUT_ENV_FILE` alternatives
- `docker-compose.yml:77` — `${CODESCOUT_MODEL_DIR:-./models}:/models:ro`
- `.env.amd:16,26,49,50` / `.env.gpu` — the correctly-maintained profiles
- `docs/issues/2026-07-25-coderankembed-gguf-source-404.md` — why `.env.amd`
  changed today

