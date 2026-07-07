# Workspace Gotchas

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
See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

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

## RemoteEmbedder Dimensions

`RemoteEmbedder.dimensions()` returns `0` until after the first successful `embed()` call
(uses `AtomicUsize` cached lazily). Callers needing a guaranteed non-zero dimension must
embed a sample text first.

## Cherry-Pick SHA Discipline

Always record the **master-side SHA** after cherry-pick, not the experiments-side original.
After `git rebase master`, experiments-side originals become orphans — `git branch --contains`
returns empty. Use `git log master --oneline --grep="<subject>"` to recover master SHA if needed.

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