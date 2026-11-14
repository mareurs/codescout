# Librarian (embedded in codescout)

`librarian-mcp` is no longer a standalone MCP server. It is embedded inside
the codescout binary as a subsystem that is compiled into every build, with
no Cargo feature to turn it off.

The two librarian tools — `doc` and `librarian` — are
advertised alongside codescout's core toolset, and the librarian server instructions block is
appended to codescout's MCP `instructions` field. (`workspace_state_at` was
folded into `librarian(action="workspace_state_at")`; see
[Librarian Tools Collapse](librarian-tools-collapse.md).)

## Build-time control

There is none: the librarian is compiled into every build, including
`--no-default-features`. The `default` feature list names only the transports and the
embedder client:

```toml
# Cargo.toml
[features]
default = ["remote-embed", "http"]
```

```bash
# Any build includes the librarian
cargo build --release

# Without the default features (no `http`, no `remote-embed`) — still includes the librarian
cargo build --release --no-default-features
```

## Runtime override

Librarian registration is **enabled by
default**. Opt out per session via env var, or per project via `project.toml`.

| Knob | Value | Effect |
|------|-------|--------|
| `LIBRARIAN_ENABLED` env | `0` / `false` / `off` / `no` | Disable for this codescout process |
| `LIBRARIAN_ENABLED` env | `1` / `true` / `on` / `yes` | Force enable (overrides project.toml) |
| `[librarian] enabled = false` in `<project>/.codescout/project.toml` | bool | Per-project disable when env unset |
| (default) | — | **Enabled** |

The env var wins; project.toml is consulted only when the env var is unset.

To opt out globally, set `LIBRARIAN_ENABLED=0` in the codescout MCP server
launch env (e.g. the `env` block of `.mcp.json` or your shell rc).

## First run

On first start the server creates `librarian/workspace.toml` in your user config
directory when it is absent, so a new machine reaches a working librarian with
no manual file. That directory is `~/.config` on Linux,
`~/Library/Application Support` on macOS and `%APPDATA%` on Windows. The file
holds only comments and is safe to edit. The server never overwrites
it. If the location is not writable, the server starts on an empty default and
logs a warning.

Two files share the name `workspace.toml`. This one is global and per user. The
other is `<project>/.codescout/workspace.toml`, which declares one project's
sub-projects and umbrellas (`doctor` reads it).

Creation applies only to the default path. If `LIBRARIAN_WORKSPACE` names a
file that does not exist, nothing is created and the librarian stays disabled:
its tools are not registered, and the reason is logged. A file that exists but
does not parse disables it the same way.

## The vector cache

The librarian keeps the vectors it makes in a cache. The cache is one file, `vector-cache.db`. It is in the same directory as the catalog file, `catalog.db`. It is not in a repository.

The file belongs to one user. Every repository, worktree and profile of that user shares it. If you set `LIBRARIAN_DB`, the cache file moves with the catalog file.

The cache key has two parts. The first part is the embedding setup: the model name, the text budget and the version of the text format. The second part is the exact title and text that the librarian embeds. The same text under a different title, model name, budget or format version does not match. It gets its own vector.

Two embedding servers can run under one model name. They then share keys. If the weights behind a model name change, stop codescout and remove the cache file. Then run `reindex` with `reembed=true` for each project.

A vector goes into the cache only when the librarian embeds the text. The cache is empty when it is new. The librarian does not embed a file that has not changed. So the cache holds no vector for such a file. When the cache first appears, it holds no vector for any file that you already indexed.

The cache fills from text that the librarian embeds after the cache exists. This text comes from a new file, from a changed file, or from a `reindex` with `reembed=true`. To fill the cache for a project, run `reindex` with `reembed=true` for that project. This embeds each chunk again. It takes the full embedding time, one time.

When a file moves, its chunks get new identifiers. The text does not change. If the cache holds the vectors of that text, the librarian takes them from the cache. It does not call the embedding server. It stores each vector in the search index under the new chunk identifier. A copied file and a rebuilt catalog work the same way. If the cache does not hold the vectors, the librarian embeds the text and keeps the new vectors in the cache.

So a file that git moves reuses its vectors only if the librarian embedded its text after the cache existed. Until then, the librarian embeds the moved file one time.

The result of `reindex` has an `embed_cache` field. It has four values:

- `status` is `off` (no cache is in use), `on`, or `unavailable` (the cache file could not be opened).
- `hits` is the number of vectors that came from the cache.
- `stored` is the number of vectors written to the cache.
- `errors` is the number of times the cache failed.

In the `reindex` result, `embedded` counts only the vectors that the embedding server made. The vectors from the cache are in `hits`.

The `backfill-chunks` command uses the cache in the same way. Its result has the same `embed_cache` field. In that result, `embedded` is the number of vectors written. `embed_cache.hits` is the number of those vectors that came from the cache.

`reindex` with `reembed=true` does not read the cache. It embeds each chunk again and writes the new vector to the cache. This replaces the old row.

The cache only saves time. A cache problem never makes `reindex` or `backfill-chunks` fail. If the cache file is missing, damaged, locked or of a newer format, the librarian calls the embedding server for each chunk. It counts each problem in `embed_cache.errors`. If the cache file could not be opened, the next `reindex` or `backfill-chunks` call tries to open it again.

Nothing removes rows from the cache. The file only grows. Each distinct embed input adds about 3 KB. For the corpus of the codescout repository, an estimate is about 439 MB.

To remove the cache, first stop every codescout process that uses it. Then delete `vector-cache.db`, `vector-cache.db-wal` and `vector-cache.db-shm`. Do not delete the file while a process uses it. The next embed creates a new empty file.

The cache does not yet fill itself from vectors that already exist. It does not yet remove old rows. Both are planned work.

## What you lose with `librarian` off

<!-- audit-doc-refs:ignore-refs `tools/list` — the MCP method name, not a path (the repo root has a tools/ directory, so the audit would read it as a missing file) -->

- The two librarian tools disappear from `tools/list`.
- The librarian instructions block is omitted from the MCP `instructions`
  field, so the LLM gets no hint that artifact tooling exists.
- The on-disk catalog (SQLite at `$XDG_DATA_HOME/librarian/catalog.db`) and
  workspace.toml are untouched — flipping the feature back on resumes where
  the previous session left off.

## Opting out in production

Production users of codescout-as-MCP who do not use artifact tooling can
opt out via `LIBRARIAN_ENABLED=0` to avoid the
token overhead of the librarian tool descriptions. The cargo feature can
also be compiled out entirely (`--no-default-features`) for a leaner
production binary.
## Default scope: project (not workspace)

All listing tools default to `scope="project"`, returning only artifacts
under the agent's current sub-project. The current project resolves from
cwd → nearest `.git` ancestor → workspace root from the global `librarian/workspace.toml`
(in your user config directory).

`librarian_reindex` follows the same default. A force-wipe under
`scope="project"` only deletes rows whose `rel_path` starts with the
current sub-project's subdir — sibling projects under the same workspace
root are preserved.

| Scope | Coverage |
|-------|----------|
| `project` (default) | Current sub-project only |
| `repo` | Whole workspace root (all sub-projects under it) |
| `umbrella` | All members of the declared umbrella for the current project |
| `all` | Workspace-wide |

Read tools surface a `scope` block + `hints` (`more_in_repo`,
`more_in_workspace`) so the LLM can widen on demand. Reindex echoes its
`scope` and resolved `targets` in the response.

## Per-project classifier overrides

<!-- audit-doc-refs:ignore — this section lists classifier *patterns*, not citations.
     A pattern is configuration: it matches files that may or may not exist in any
     given project, and several of these do not exist in this repo. -->

Drop a `<project>/.codescout/librarian.toml` to declare classification
rules for that project's paths without touching the global
`librarian/workspace.toml` in your user config directory. Schema matches the global file's
`[[rule]]` blocks. Rule precedence is **project > workspace > built-in
defaults**, first-match-wins.

```toml
# <project>/.codescout/librarian.toml
[[rule]]
glob = "codescout/docs/reviews/**/*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "codescout/docs/agents/*.md"
kind = "doc"
```

Built-in defaults already cover common patterns: `CHANGELOG.md`,
`CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, `docs/QUICK-START.md`,
`docs/QUICKSTART.md`, `docs/concepts/**`, `docs/configuration/**`,
`docs/experimental/**`, `docs/manual/**` (doc), `docs/issues/**` (tracker),
`docs/trackers/**` (tracker), `docs/TODO-*.md` and `docs/TODO/*.md` (tracker),
`docs/review-*.md` (memory, dated), `docs/research/*.md` (memory, dated),
`docs/superpowers/specs/*.md` (spec), `docs/superpowers/plans/*.md` (plan),
`docs/audits/*.md` (audit), `docs/handoffs/*.md` (handoff),
`docs/runbooks/*.md` (runbook), `docs/adrs/*.md` (adr), `ROADMAP.md`
(roadmap), `**/prompts/*.md`, `src/**/prompts/*.md`,
`crates/**/prompts/*.md`. A file's own frontmatter `kind:` outranks every
rule. The override file is for project-specific
patterns the defaults can't reasonably guess.

## Migration from standalone librarian-mcp

Earlier sessions ran `librarian-mcp` as a separate stdio MCP server. That
binary no longer exists — `crates/librarian-mcp` is now lib-only. The
`codescout-companion` plugin's session-start hook no longer injects a
separate librarian companion-hint block; the librarian instructions are
served through codescout's own `instructions` field.

`~/.claude/.claude.json` should have only one MCP server entry (`codescout`).

codescout self-loads a startup dotenv, so the registration needs no `env` block.
Point it at a retrieval-profile file once:

    ln -s /abs/path/to/codescout/.env.amd ~/.config/codescout/.env
    # or export CODESCOUT_ENV_FILE=/abs/path/to/.env.amd

Then register the bare binary:

    claude mcp add codescout -s user -- /abs/path/to/codescout start --debug

The `.env.<profile>` files include both `CODESCOUT_EMBEDDER_*` (retrieval stack)
and `LIBRARIAN_EMBED_*` (artifact embedder), so one file configures both. Real
environment variables still override the file.
