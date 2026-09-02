# Workspace Gotchas

## `.codescout/embeddings/*.db` May Belong To A Backend That Is Not In Use

**Read the resolved backend before quoting any number about "the index."**

`VectorBackend::resolve` (`src/retrieval/code_store.rs`) reads
`CODESCOUT_VECTOR_BACKEND`; when it is **unset** and the binary is built with
`server-stack`, it defaults to **Qdrant**. The sqlite-vec store files
(`.codescout/embeddings/<project>.db`) are then leftovers from an earlier lite-stack
run — and they keep answering queries perfectly.

Measured 2026-08-26 on this host, minutes apart:

| | live tool (Qdrant) | `codescout.db` |
|---|---|---|
| distinct files | 1611 | 1593 |
| chunks | 47 647 | 46 979 |
| paths absent from disk | 6 | 18 |

**Use `index(action="status")` or `index(action="verify")`, not `sqlite3`.** The
tool owns the backend resolution and cannot be wrong about its own substrate.

Three reasons nothing catches this on its own, so the check has to be deliberate:

- A bounded `sqlite3` read is correctly not IL-3-blocked.
- Every query succeeds and returns internally consistent numbers — no zero, no
  empty result, no error for `docs/PROBES.md` rules 3/4 to catch on.
- **A passing positive control does not help.** Validating that a predicate
  discriminates (a deliberately-broken join key returning all rows) says nothing
  about *which database* it discriminated in. Instrument and substrate are
  orthogonal; passing one while failing the other is the most confident wrong state
  available.

Also: `chunks_without_vectors` from `verify` is a real measurement only under
sqlite-vec. Qdrant returns 0 structurally, because a point carries its payload and
vector together.

Full account: `bug-fix-session-log:F-66`, `reconnaissance-patterns:R-116`,
`tool-usage-patterns:T-28`.

## Semantic Index — Fixture Projects Not Indexed

The semantic index is populated for `codescout` only. All fixture projects
(java-library, kotlin-library, python-library, rust-library, typescript-library,
nav-eval-rust, edit-eval-rust) have no separate semantic index.
**When searching within fixture projects:** skip `semantic_search`; use
`grep(pattern, path="tests/fixtures/<name>/src")` or `symbols(path="tests/fixtures/<name>/")` directly.

## Kotlin LSP Circuit-Breaker

`kotlin-language-server` circuit-breaker trips when two codescout instances target the same
Kotlin project concurrently. `symbols(include_body=true)` will fail with "circuit-breaker open".
**Workaround:** use `grep` as fallback.
See `docs/issues/archive/2026-04-24-find-symbol-kotlin-multi-session.md` (the "Multiple
editing sessions" lock contention and the 8 s per-language budget) and
`docs/issues/archive/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md` (the
per-instance system-path fix). The circuit-breaker landed in `dc44ac3d`.

## eval Fixture Workspace Isolation

`edit-eval-rust` and `nav-eval-rust` declare their own `[workspace]` tables and must
**never** be added as workspace members of codescout. Their `Cargo.lock` must stay separate.
`git restore tests/fixtures/edit-eval-rust/src` resets mutations — all `src/` files must be
git-tracked or restore silently no-ops and mutations leak between eval cases.

## MCP Binary Symlink

`~/.cargo/bin/codescout` is a symlink → `target/release/codescout`.
`cargo build` (dev profile) does NOT update the live binary. Only `cargo build --release` does.
After a release build, run `/mcp` to reconnect. If the symlink is missing after `cargo clean`,
recreate: `ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout`.

**But the reconnect refreshes the SERVER, not the LSP muxes it attaches to.** A mux is keyed by
project hash and shared across sessions, so a freshly started server attaches to whichever mux
already holds the socket. Sweep with `readlink /proc/<pid>/exe` over `pgrep -x codescout`: the
kernel appends ` (deleted)` when a running image has been replaced, and that marker is the only
signal here that does not read green in the broken world — binary mtime, this symlink, a clean
`git status`, the last source commit and the reconnect itself all report fresh while stale
images run. `$PPID` inside a `run_command` shell names your own **server** only; a mux is a
*descendant* of some server (possibly another session's), never an ancestor of your shell, so
it is reachable by the sweep alone. A stale mux self-heals once idle past its `--idle-timeout`
(180s Rust, 300s Kotlin), so the exposure is use-coupled — stale exactly while it is being
queried, healthy the moment nobody cares. (F-84 in `bug-fix-session-log`.)

**The self-heal above is a property of MUXES ONLY, and the sweep finds stale SERVERS too — for
those the argument inverts.** `codescout start` takes no idle-timeout option (checked against
`--help`, 2026-09-01), so a server lives exactly as long as the `claude` process that spawned
it. A stale server therefore never recycles: it is not use-coupled, it does not heal when
nobody cares, and only `/mcp` or that session exiting clears it — neither of which anything
prompts. **Discriminate by parent, not by the `(deleted)` marker, which is identical for both:**
`ps -o cmd= -p $(ps -o ppid= -p <pid>)` — a server's parent is `…/claude`, a mux's is another
`codescout`, and the mux command line also carries `mux --socket …`. **Prefer that last one — the
child's OWN cmdline — which is what the block below now uses.** The parent test carries a latent
false positive: it asks whether the *parent's* command line contains `claude`, and this repo lives
at `/home/marius/work/claude/codescout`, so a mux whose parent server was launched from
`target/release/codescout` classifies as a **server**. It happens not to fire only because live
servers run via the `~/.cargo/bin/codescout` symlink — an accident of launch path, not a property
of the test. Found by a peer 2026-09-01 by checking the snippet against every live mux instead of
assuming it; the `mux --socket` form needs no parent lookup and was verified to give the identical
partition (9 servers + 3 muxes) on the same population.

Measured 2026-09-01: a release build at 13:04:36 left **three** servers from 11:26–11:28 running
the deleted image, still alive at 13:40 — one in this repo, two in another project. The in-repo
one predated a 304-line `src/librarian/catalog/audit.rs` behaviour change, so its `artifact`
writes were recording audit rows the superseded way with nothing to signal it.
`/proc/<ppid>/environ` did **not** expose `CLAUDE_CODE_SESSION_ID`, so the sweep cannot name the
session — `readlink /proc/<ppid>/cwd` is the discriminator that does work, and it is enough to
tell a peer which window to reconnect.

**Nothing here is orphaned — do not reach for `pkill`.** Measured 2026-09-01: every one of the
13 processes the sweep returns had a **live parent**. A stale server is a live session's server
running a replaced image, not a leak; a stale mux is shared infrastructure with an idle timeout.
So the verb is *accumulate*, never *leak*, and the remedy is `/mcp` **in the owning session** —
there is nothing to kill here, and killing one would take a working session's server out from
under it. Read this before the counts below: a reader who meets "13 stale images" first reaches
for `pkill` and is wrong.

**They ACCUMULATE, which is the consequence of not self-healing and is easy to under-rate from a
single reading.** Re-measured at 15:2x the same day, after a few more rebuilds: **10 stale
servers and 2 stale muxes**, up from 3 servers ninety minutes earlier. The split is the whole
point — the 2 muxes recycle themselves once idle past their timeout; the 10 servers will not,
and each is a session silently running pre-rebuild code. Treat any single count as a floor
rather than a magnitude: it grows once per rebuild per live session and falls only when
sessions exit.

Count it directly, and note this is a `/proc` sweep rather than a session enumeration — it is
per-user and therefore profile-independent, where `ListAgents` is per-profile and under-reports:

```
for p in $(pgrep -x codescout); do
  case "$(readlink /proc/$p/exe 2>/dev/null)" in *" (deleted)")
    case "$(tr '\0' ' ' < /proc/$p/cmdline)" in *"mux --socket"*) echo "mux $p";; *) echo "server $p";; esac;;
  esac
done
```

**The fix's own AUTHOR is in the stale population by default, and this is the part that reads
green.** Measured 2026-09-01: two fixes committed at 14:59 and 15:12, release binary rebuilt at
15:35 — and the author's own server had started at **14:21**, so it could not contain either. The
author had already told peers to reconnect with `/mcp` and framed staleness as a fact about *other*
sessions. Nothing about a stale server is observable from inside it: every call succeeds, and the
sole asymmetry is that a path the fix changed still behaves the old way. Identify your own server
by walking up from a `run_command` shell (`sh → codescout → claude`); the `codescout` in that chain
is yours.

**So probe a code path the fix CHANGED — and check first that the probe is able to fail.** The
first probe here read a stamped-*looking* bug file and succeeded, which reads exactly like "fix is
live". That file carried no `id:` (`_TEMPLATE.md` writes none), so the guard arm under test never
fired: a non-discriminating probe returns success in **both** worlds. The valid probe used a file
verified to carry `id: <16-hex>` and no `entry_prefix` — pre-fix it refuses with the anonymous
*"do not read or edit it directly"*, post-fix it reads. Confirming the fixture is load-bearing is
the whole probe; without it you have measured nothing and feel informed.

**The refusal that makes that probe work is the same thing that HIDES the staleness in normal
use.** The anonymous *"do not read or edit it directly"* is a detector only once you are already
suspicious. Measured 2026-09-01 from the *receiving* side: a session hit several stamped-file
refusals (`issue-clusters.md`, an archived bug file), routed around each via `artifact(get)`, and
questioned none — because **a refusal reads as the guard working correctly**. Its workarounds
happened to be right, so nothing downstream broke and nothing prompted a re-check, while some of
those refusals had been fixed hours earlier. That is `R-89`'s process axis with a wrinkle worth
naming on its own: not merely that the served copy is stale, but that **the stale copy's error
text is what makes it invisible** — a plausible refusal rather than an error, which is precisely
the shape nothing downstream fires on.

**Freshness is per-TRANSPORT, not per-session — one session can be fresh on the CLI path and
stale on MCP in the same minute.** Measured 2026-09-01: a session's `doctor` runs all went
through `./target/debug/codescout doctor`, rebuilt before each run and therefore current, while
every `artifact()` / `edit_markdown()` call it made in the same window went through an MCP server
started six hours earlier. Its **measurements stand; its writes were stale.** The split falls the
dangerous way by default, because measurements tend to go through the CLI and mutations through
MCP — so the readings look fine and the *writes* are the stale half. Ask which transport a call
took before ruling a session clean or dirty; *"my session is fresh"* is not a well-formed claim.

*(Mirror of this section's opening case, observed 2026-09-01: mux `3934969` fresh at 19:37 under
server `62803` stale from 13:38. The opening paragraph has a fresh server attaching to a stale
mux; this is the inverse. The two layers are therefore demonstrably independent in both
directions, rather than merely described as separate.)*

**Mechanism candidate (not built).** The server can answer this about itself in one syscall —
`readlink("/proc/self/exe")` ending in `" (deleted)"` — and surface a one-line advisory in the
envelope. That is a check that runs when nobody is worried; the `/proc` sweep above runs only once
somebody already suspects, which is the state this whole section exists because nobody reaches.

*(Re-measured 15:4x, after the 15:35 build: **10 servers — 9 stale, 1 current — plus 3 stale
muxes**, against 10 servers + 2 muxes at 15:2x. Consistent with the floor claim; the mux count
moved because muxes are use-coupled, the server count did not because they are not.)*

*(Re-measured 2026-09-02 20:13:44, after a 20:06:36 release build: **17 servers — 11
stale, 6 current — and 3 muxes, 0 stale.** All four numbers taken in ONE sweep rather
than assembled from two readings, for the reason the last paragraph gives.*

*THE MUX HALF IS NOW DEMONSTRATED RATHER THAN DESCRIBED. 2026-09-01 15:4x found 3 of 3
muxes STALE; a day later the same population size is 3 of 3 FRESH. Same count, opposite
state — that is what use-coupled recycling looks like from outside, and no single
reading could have shown it. The server half held across a day boundary rather than
merely across ninety minutes: 10 → 17 total, 9 → 11 stale.*

*THE AUTHOR'S OWN SERVER WAS FRESH THIS TIME, which sharpens the paragraph above rather
than contradicting it: an explicit `/mcp` had just run. The rebuild is not what moves a
session out of the stale population — the reconnect is. "I just rebuilt" is not the
escape; "I just reconnected" is, and only for the session that ran it.*

*AND THE WINDOW MOVED UNDER THE INSTRUMENT, in miniature. A first reading five minutes
earlier returned 12 stale servers with no denominator captured. Re-running only to fetch
the denominator would have spliced two windows into one apparent observation, so the
whole sweep was retaken instead — and the stale count had already moved 12 → 11 by then.
Read every figure in this section as an instant, never a level.)*
## RemoteEmbedder Dimensions

`RemoteEmbedder.dimensions()` returns `0` until after the first successful `embed()` call
(uses `AtomicUsize` cached lazily). Callers needing a guaranteed non-zero dimension must
embed a sample text first.

## Cherry-Pick SHA Discipline

Record the fix **SHA and its patch-id** — the SHA alone is not durable, and both promotion
paths stay available without needing to check which one applies.

A SHA is positional. After `git rebase master`, experiments-side originals of cherry-picked
commits become orphans and `git branch --contains` returns empty. Measured 2026-08-19:
**10 of 63 archived bug files had already lost their fix pointer**, with the objects absent
from the object DB rather than merely unreferenced — so the reflog cannot help either.

`git show <sha> | git patch-id --stable` is a content hash of the diff and survives rebase
**and** cherry-pick. Zero genuine collisions across 3594 commits; all 104 duplicate
patch-ids were the same change appearing on two branches, which is the anchor working.

There is no promotion path to check and nothing owed later — record the pair once at fix
time. To recover an already-orphaned commit, resolve its patch-id with **redirects**, since
Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep <first-12-of-patch-id> /tmp/patch-ids.txt
```

`git log master --oneline --grep="<subject>"` is a weaker fallback: measured the same day,
subject probes returned between 2 and 153 candidates — a search, not a lookup.
## Cross-Repo Commit References

When a tracker cites a commit from a sibling repo, prefix: `<repo>:<sha>` (e.g. `codescout-companion:0b75991`).
A bare SHA implies the current repo. Unenforced by tooling — readers must notice the prefix.

## Artifact Freshness Requires a `reviewed` EVENT — Not a Refresh

`artifact(get).freshness` stays `"unknown"` no matter how many `commit_refresh=true`
updates land: `freshness::compute` (src/librarian/freshness.rs) returns Unknown whenever
`latest_reviewed_at` is absent. `commit_refresh` feeds the `provenance` keys
(`refreshed_at_commit`, `commits_behind_head`); freshness anchors on
`artifact_event(kind="reviewed")`. To flip a tracker fresh: emit a reviewed event (earn it —
an actual content review), THEN freshness computes fresh/stale from file mtime + commit
distance. Discovered on the W3 audit-log retrofit (2026-07-05).

## Link Graph Is Derived — Re-Run link_scan After Moves/Reindex

`artifact_link` rows do NOT durably survive: the reindex abs_path pre-clean
(catalog/artifact.rs upsert) CASCADE-drops a moved artifact's links when its id churns.
Never hand-curate `cites` edges — cite in prose and run
`librarian(action="link_scan", write=true)` (idempotent fixpoint; scanner owns rel="cites"
only, never touches manual rels/supersedes). `context(anchor_id=…)`'s large-hub neighbor
starvation is FIXED (2026-07-05, `src/librarian/tools/context.rs::call`) — the packing loop
now reserves half the budget for neighbors and truncates an oversized anchor rather than
letting it consume the whole budget; `artifact(graph)` + targeted `get(heading=…)` remains
a fine alternative but is no longer a required workaround.
## link_scan Can't See Augmented-Artifact Param Rows (Only Markdown Headings)

`link_scan`'s definition detector (`extract.rs::def_re`) only recognizes `## TOKEN — title`
markdown headings. Entry-tokens that exist ONLY as structured rows inside an augmented
artifact's `params` (e.g. `tool-usage-patterns.md`'s `T-N` rows, most of which have no
matching `### T-NNN` prose write-up) will always report as `dangling`, even though they
are genuinely "defined" from the tracker's own maintenance-contract perspective. This is an
architectural boundary, not a bug — `link_scan` scans prose/headings, not augmentation
params. Confirmed 2026-07-05: ~21 of a sampled dangling batch were `T-1`..`T-21` cited from
`docs/trackers/artifact-augmentation-followups.md`, all traced to this cause. No fix planned;
note it here so a future dangling-triage pass doesn't re-investigate from scratch.

## Short Tracker IDs (F-N/W-N) Are Locally-Scoped — Multi-Definer Is Expected

Nearly every `docs/trackers/*session-log.md` file runs its own independent `F-1`, `F-2`,
`W-1`... counter. `link_scan` correctly reports a generic short token (e.g. `F-8`, `W-2`)
cited without a qualifying tracker name as `ambiguous` whenever ≥2 trackers define it — this
was predicted by the pre-implementation design-validation memo ("multi-definer is the common
case, not rare") and confirmed live 2026-07-05: the large majority of a 213-entry ambiguous
sample was exactly this pattern, concentrated in `prompt-hamsa-audit-log.md`'s narrative
citing other trackers' F-N/W-N entries generically. Not a bug — the system correctly declines
to guess which tracker's `F-8` is meant rather than link to a possibly-wrong one.
## Onboarding Subagent Project-Scope Collision

During parallel force-onboarding, subagents may overwrite each other's memories in the
`codescout` project slot (last writer wins when multiple subagents share the focused project).
Verify `memory(action="read", project_id="codescout", topic="project-overview")` after onboarding
to confirm the content is actually about codescout and not a fixture crate.

## `symbols(path)` Routes to LSP When Available — Not Always Tree-Sitter

`symbols(path)` uses the **LSP** (rust-analyzer/gopls/tsserver) when a client is available for
that file, and falls back to the tree-sitter AST extractor
(`src/ast/parser.rs::extract_symbols_from_source`) only when none is. LSP clients start
**lazily**, so the same probe file can hit tree-sitter early in a session and the LSP later —
the output shape changes under you.

Tells the output came from the LSP, not the AST extractor:
- Rust: an `impl S [Object]` container symbol appears (the AST extractor emits NO impl symbol —
  it merges impl methods up to the parent level).
- Go: methods named `(*Stack[T]).Push` (LSP) vs the extractor's `Stack/Push` name_path.
- TS: arrow-fn consts reported as `Constant` incl. plain data consts (the extractor emits
  function-valued consts as `Function` and skips data consts).

**To verify a tree-sitter extractor fix:** do NOT trust `symbols(path)`. Either unit-test
`extract_symbols_from_source` directly, or run `edit_code` on a previously-dropped symbol —
`edit_code` resolves via LSP then **AST-confirms** the end line (`ast_confirmed_end_line` →
`extract_symbols_from_source` → `find_ast_end_line_in`), so a successful insert where pre-fix
returned "AST parse failed" is the proof. Datapoint: the 2026-06-04 extractor-coverage fixes
(nested types, namespaces/abstract classes, Rust assoc items/macros, TS arrow consts, Go generic
receivers) — `symbols` showed LSP output for all; `edit_code` on `my_macro` / impl `Output` was
the real proof.

## `artifact(create, augment={...})` Silently Drops `entry_collection`

The `augment` shortcut on `artifact(action="create")` only accepts `prompt` and `params`
(per its own input schema) — passing `entry_collection` (or `render_template`, `params_schema`,
etc.) alongside them inside `augment` is silently ignored, leaving the new artifact's
augmentation with `entry_collection: null`. Any code that filters on `entry_collection`
(e.g. `find_matching_rules`/`find_global_rules` in the constitution-tracker feature, which
skip any tracker whose `entry_collection != "rules"`) will then see the artifact as invisible,
with no error anywhere in the chain — it looks exactly like "no matching trackers exist."
Hit twice in one session (2026-07-06) creating throwaway test trackers for the constitution
archetype's `rules` entry_collection.
**Fix:** follow `artifact(create, augment={prompt, params})` with a separate
`artifact_augment(id=..., entry_collection="...", merge=true)` call — `create`'s `augment`
shortcut only ever gets you `prompt`+`params`; everything else needs the dedicated tool.

## Machine-Wide Startup-Env Symlink Can Silently Contradict the Running docker-compose Profile

`codescout::config::load_startup_env()` (`src/config/global.rs`) never reads a repo's own
`.env*` files — only `$CODESCOUT_ENV_FILE`, or else `$XDG_CONFIG_HOME/codescout/.env`
(a `$HOME`-scoped symlink, not a repo file, shared by every codescout process on the
machine). If `index(action="build")` fails on an embed_batch call (e.g.
`embed_batch sparse send`) but the configured URL responds fine to a direct `curl`, don't
assume the URL is wrong — check what that symlink actually resolves to
(`readlink -f ~/.config/codescout/.env`) and whether *its* `CODESCOUT_DISABLE_SPARSE` /
`CODESCOUT_SPARSE_EMBEDDER_URL` values match the docker-compose profile that's actually
running (`docker ps -a`), not the repo's own `.env`/`.env.gpu`/`.env.amd` files — this
loader never reads those regardless of which one looks current. The symlink target can
drift stale silently (e.g. left pointing at a profile whose compose services were later
removed) with nothing warning on mismatch. See
`docs/issues/archive/2026-08-23-index-build-fails-embed-batch-sparse-send.md` and
`bug-fix-session-log:F-59`.

## `cargo test` Fails From Native `Bash` But Passes Via `run_command` — Export `CODESCOUT_EMBEDDER_URL`

**RESOLVED 2026-08-28** — root cause found and removed. Kept for the mechanism and
for the two debugging lessons at the end, both of which cost real time.

**Symptom, as it presented.** Run `cargo test` from a native harness `Bash` call in
this repo and `tools::memory::tests::write_and_read_roundtrip` FAILS with
`semantic anchor creation failed: dense embed connect failed:
http://127.0.0.1:48080/v1/embeddings`. Nothing listens on 48080; the dense
embedder is `codescout-dense-amd` on **48081**. The same suite run through
codescout's `run_command` is 4731 passed / 0 failed. The test **asserted** strict
equality against `"ok"`, so an attached warning failed it — which made this read
exactly like a code regression when it was an environment difference. That
assertion was fixed 2026-08-27 (`src/tools/memory/tests.rs` now reads the status
out of either result shape), so the same misconfiguration today produces a passing
test with a warning attached rather than a red gate.

**Remedy — already installed on this machine (2026-08-27).** `~/.cargo/config.toml`
carries an `[env]` block, so every `cargo` invocation gets the var regardless of
shell and nothing needs remembering:

```toml
[env]
CODESCOUT_EMBEDDER_URL = { value = "http://127.0.0.1:48081", force = false }
```

Verified: the previously-failing test passes bare with the var unset in the shell,
and `force = false` is a fallback rather than a lock — an explicit
`CODESCOUT_EMBEDDER_URL=http://127.0.0.1:49999` still overrides it and still
fails. User-level config, outside every repo, so nothing ships to contributors.

**It is deliberately NOT in the repo's `.cargo/config.toml`.** That file is
tracked and carries the `cargo rb` alias CLAUDE.md names as the live-MCP build
command, plus `sccache` and the mold linker, and is referenced by
`.github/workflows/ci.yml` and `scripts/build-windows.sh`. Gitignoring it would
require untracking it, which deletes all of that from every other checkout and
from CI. Cargo's `include = [...]` is not an escape either — **a missing include
target makes cargo fail hard** (`exit 101`, "could not load Cargo configuration"),
measured 2026-08-27, so a gitignored sidecar breaks every clone that lacks it.

On a machine without that `[env]` block, do it by hand — either form works:

```bash
export CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081
# or load the whole startup env:
set -a; . ~/.config/codescout/.env; set +a
```

**Mechanism: ESTABLISHED 2026-08-28. Two files, same two variable names, opposite
values.** `settings.json` → `env` feeds the Claude Code process and therefore its
native `Bash` children; `.claude.json` → `.mcpServers.<server>.env` feeds the MCP
server and therefore its `run_command` children. In the `-sdd` and `-kat` profiles
the first carried a **stale** `CODESCOUT_EMBED_MODEL=all-minilm` /
`CODESCOUT_EMBED_URL=…:48080/v1`; the second carried the **correct** `CodeRankEmbed`
/ `…:48081/v1`. Both names are `apply_embed_overrides` inputs at highest
precedence, so two shells resolved different embedders from identical code and
identical `project.toml`.

The plausible story — "`load_startup_env()` runs at MCP server startup so
`run_command` children inherit the var and `Bash` children do not" — is what this
entry originally asserted, and it is **wrong twice**: it does not survive its own
measurements below, *and* `~/.config/codescout/.env` never sets the short-prefix
`CODESCOUT_EMBED_*` pair at all (it sets the `EMBEDDER_`-prefixed ones), so it was
never the rescuer. Do not re-propose it. The measurements that constrained the
search:

| Condition | Result |
|---|---|
| Bash tool, nothing set | FAIL (48080) |
| `CODESCOUT_EMBEDDER_URL=…:48081` | PASS |
| `LIBRARIAN_EMBED_URL=…:48081` alone | FAIL |
| `env -u CODESCOUT_EMBED_URL` | FAIL |
| `env -i HOME=… PATH=… bash -lc 'cargo test …'` | **PASS** |
| via `run_command` | PASS |

The last row is the refutation: a near-empty environment passes, so the failure is
caused by something **present** in the harness environment, not by the absent
var — yet stripping the obvious suspect (ambient
`CODESCOUT_EMBED_URL=http://127.0.0.1:48080/v1`) does not fix it. *Why that row
misled:* the other stale var, `CODESCOUT_EMBED_MODEL=all-minilm`, was
independently sufficient on its own, so removing either one alone changed nothing.

**Retracted 2026-08-28.** The clause *"set in the desktop session and present in
both the Bash shell and the MCP server's `/proc/*/environ`"* used to sit in the
sentence above, and it is the observation that wrongly cleared this variable — if
both sides have it, it cannot be the discriminator. Re-checked across 7 live
`codescout start` processes: three carry `CODESCOUT_EMBED_URL=…:48081/v1`, four
lack the var, and **none carries `48080/v1`**. The reading cannot be reproduced;
the likeliest source is a zombie server predating the `.claude.json` block
(`docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`).
The name was on both sides. The value never was. Also checked and excluded: `XDG_CONFIG_HOME` and `CODESCOUT_ENV_FILE` are
unset, no shell profile sets `CODESCOUT_EMBEDDER_URL`, and `48080` appears as a
literal nowhere in `src/`. Note `/proc/<pid>/environ` shows only the exec-time
environment, so a var set later by `std::env::set_var` is invisible there — which
is why that file cannot settle this either way.

**Fix applied 2026-08-28** — both stale vars deleted from
`~/.claude-sdd/settings.json` and `~/.claude-kat/settings.json` (backups
`*.bak-20260827`); `~/.claude`'s block was already correct and was left alone.
Verified after a Claude Code restart: **zero** `CODESCOUT_*` and zero
`EMBED`-named vars in native `Bash`, and the previously-failing test passes when
the test binary is invoked **directly** — which bypasses the
`~/.cargo/config.toml` pin, so the pass is not the pin propping it up. Full
write-up, including the retracted evidence line:
`docs/issues/archive/2026-08-27-cargo-test-fails-from-bash-passes-via-run-command.md`
(`fixed`, archived 2026-08-28).

**Two transferable debugging lessons.**

- **At a divergence, diff values — not names.** "The stale var is present in the
  MCP server's environ too" retired the correct suspect for a day. The *name* was
  on both sides; the *values* disagreed, and the disagreement was the whole
  mechanism. `grep -c` answers the wrong question here.
- **Bisect additively from a known-good base.** With two independently sufficient
  causes, one-at-a-time elimination returns a clean *negative* for a variable that
  genuinely is a cause — it did so three times running. `env -i` with `{HOME,PATH}`
  passes; add variables back until it breaks.

Measured 2026-08-27 at `c37c7c98` (diagnosis) and 2026-08-28 (resolution), across a
period when `shell_command_mode` moved from `"disabled"` back to `"warn"`.

**The symptom is session-scoped, not `Bash`-scoped — measured 2026-08-27 22:1x from
a concurrent session, `codescout-d3`.** That session's native `Bash` ran the full
suite at `12f21926` and got **4732 passed / 0 failed**, `write_and_read_roundtrip`
among them — from native `Bash`, and **before** the `~/.cargo/config.toml` `[env]`
pin existed (config mtime 21:59:43; the run finished by 21:45). It passed because
that session's `Bash` already carried `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081`
ambiently, and `48081` is the one listening port (`ss -ltn`). Its `run_command`
carried the identical value, so the two shells did **not** diverge there at all.

Two consequences for the bisection:

- The discriminator is the **environment Claude Code was launched from**, not the
  Bash-vs-`run_command` boundary. Two concurrent sessions on this machine differ:
  `CODESCOUT_EMBED_URL` (the short name, `48080`) was present in the failing
  session's env and is **absent** from `codescout-d3`'s — `env | grep -c
  '^CODESCOUT_EMBED_URL='` → `0` in both its `Bash` and its `run_command`.
  *Explained 2026-08-28:* that fingerprint — `CODESCOUT_EMBEDDER_URL=…:48081`
  present, short-prefix `CODESCOUT_EMBED_URL` absent — is exactly
  `~/.claude/settings.json`'s `env` block and exactly not `-sdd`'s, so `d3` was
  almost certainly launched from the `~/.claude` profile. (Inferred from the env
  fingerprint, not from the process itself, which is gone.) So "session-scoped" is
  more precisely **profile-scoped**: which `settings.json` supplied the block.
- One hypothesis is already dead, so nobody need re-run it: *"`env -i … bash -lc`
  passes only because `-l` re-sources profile files that set the var."* It does not
  — `env -i HOME=… PATH=… bash -lc 'env | grep -i EMBED'` prints nothing, and so
  does the non-login form. No profile, `bashrc`, `/etc/profile.d/` or
  `environment.d/` file on this machine sets any `CODESCOUT_EMBED*` var. The
  `env -i` row stands as a genuine refutation of the absent-variable story.

Before treating a green suite from native `Bash` as suspect, check the cheap thing
first: `env | grep -i EMBED` in the shell that ran it.

**Related, separately verified.** `usage.db` records only codescout MCP calls:
`tool_name` has never contained "bash", while `run_command` is 37% of 47,727
recorded calls (2026-08-03..08-27). So shell work routed through `Bash` does not
appear in `/analyze-usage`, `docs/trackers/tool-usage-patterns.md`, or the
`pika_observations` table. Native `Bash` also does not get `run_command`'s IL-3
unbounded-pipe block (piping `cargo test` to `grep` masked a non-zero exit here,
reporting success on a failing run) or its dangerous-command `@ack_*` gate.

**Which shell path this project should use is UNDER ACTIVE EVALUATION — do not
"settle" it in either direction as a drive-by.** Eval data showed Opus performing
better with native `Bash` than with `run_command`, which is why
`shell_command_mode` was set to `"disabled"` on 2026-08-27; it was returned to
`"warn"` (i.e. `run_command` ON) the same day, deliberately, while further evals
run. So the current setting is a **hold, not a verdict**, and the trade-offs above
are inputs to that evaluation rather than defects to fix. If you find
`shell_command_mode` set to something surprising, assume it is an eval arm and
ask before changing it.

The `~/.cargo/config.toml` `[env]` pin stays useful either way: it is inert when
work goes through `run_command` and load-bearing when it goes through `Bash`, so
it does not need touching as the setting moves.

Sibling of the section above, and easy to conflate: that one is about the
symlink's *contents* drifting from the running compose profile; this one is about
a shell-dependent difference in what reaches the process.

## A Fresh Machine Loses Three Catalog Layers, Each Silent Differently

`~/.local/share/librarian/catalog.db` is machine-local, git-ignored, and **machine-global**
(one DB spanning every repo on the host). A checkout never indexed here arrives missing its
semantic index, its `cites` edges, and every artifact augmentation — while markdown bodies
look perfect, because those are the half that travels. Nothing errors; you quietly get less.
Each layer is silent in a *different* way: `reindex` preserves augmentation keyed by id
rather than regenerating it, so it reports healthy and repairs nothing; `artifact(get)`
returns `augmentation: null` without comment; a missing edge is indistinguishable from an
artifact that cites nothing.

Full ordered process, with the measured numbers from the 2026-08-28 pull of 437 commits →
**`docs/conventions/cross-machine-catalog-resume.md`** (linked from CLAUDE.md § Docs). Do not
re-derive it. Load-bearing bits: a second `link_scan` reaching `edges_missing[0]` is what
proves the write landed (the write's own success is not); partition `doctor` output three
ways (machine drift / content debt that travels in git / other repos' rows) before touching
anything; never `fix=prune_missing` during a resume.

**`append_entry` does NOT need an augmentation.** `allocate_entry_id` keys on frontmatter —
`entry_prefix` plus `entry_high_water_<PREFIX>`, both committed — so prose ledgers keep
working on a fresh clone. `entry_collection` in `append_entry`'s signature is a CALLER
argument, not a lookup of stored augmentation. Measured: 10 unaugmented ledgers, all
functional. What augmentation loss actually costs a prose ledger is the `[LIVE]` prompt.

**Restore only what a documented query proves you need.** Extract each tracker's documented
`entry_filter` prescriptions and RUN them; the ones returning `entry_filter set but this
artifact is not augmented` are the whole worklist. 2026-08-28: 18 unaugmented trackers, **4**
with a failing documented query. Filling the other 14 would mean authoring standing
instructions (the `[LIVE]` block is seen by every agent meeting the tracker cold) purely to
clear a check — and `expects_augmentation` firing is a precise signal that reconstruction
converts into a false all-clear.

**Check for a code-owned augmentation first — it restores byte-for-byte.** A tracker written
by a codescout action carries its augmentation via `include_str!`, so it is in the repo:
`legibility-backlog` ← `src/librarian/tools/legibility_scan/render_prompt.md` +
`render_template.j2`, `entry_collection: "candidates"`. The owning action does NOT self-heal —
`legibility_scan(write=true)` on an existing-but-unaugmented tracker returns `ok: true` WITH
`tracker_error: "no augmentation … call artifact_augment first"`, because create-and-augment
only fires when the tracker does not exist. Attach the exact bytes, then re-run the action.

**Shape survives in prose; the prompt does not.** Archived bug files quote the original calls
verbatim, so `entry_collection`, field names and even whole forgotten fields are recoverable
by grepping the artifact id across `docs/issues/archive/`. Nothing routinely quotes a
`prompt`, so that half is always reconstruction — label it as such in a PROVENANCE paragraph
inside the prompt itself.

## Co-Occurrence Is Not Usage — Run The Query

Three separate greps misled during one 2026-08-28 pass, each returning a plausible **number**
rather than an error, which is what makes this class dangerous:

- Counting bare `---` to find duplicate frontmatter reported **22** affected trackers; all
  false, because session logs use `---` as an entry separator. Counting a frontmatter-only
  key (`kind:`) gives the true answer, **1**.
- Filtering `doctor` output by an absolute project path matched **nothing**, reading exactly
  like "this project is clean" — response paths are relativized, so project-internal ones are
  relative and only foreign repos stay absolute. Filter on the leading `/`.
- Ranking trackers by "documented query count" gave **opposite** answers on the same tracker:
  line-scoped grep said 0 (multi-line prescriptions split id and `entry_filter` across lines),
  file-scoped said 5 (the file's `entry_filter` mentions documented a bug against a *different*
  tracker's id). Executing the query was the only method that separated "queries this" from
  "mentions both".

Generalisation: when a proxy and the real call disagree, the proxy is measuring co-occurrence.
Prefer running the thing. Anchor detection on structure (line-start, key prefix), never on a
keyword, since prose and field share a vocabulary by construction.
