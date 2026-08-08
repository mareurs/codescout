# Changelog

All notable changes to codescout are documented here.

## [Unreleased]

### Added

- **`librarian(action="doctor")` gains an `abs_path_outside_managed_roots` check.** Reports
  catalog rows whose `abs_path` falls under no configured root — the drift class behind the
  Windows `containing_root` failure below. Rows outside the active project's roots are
  EXPECTED on a catalog spanning several workspaces, so the emitted list is capped at a
  10-row sample with the elision announced in the hint rather than applied silently.
  `summary.total` and `summary.by_check` both count every violation and partition each
  other; `summary.shown` carries the post-cap emitted count.

- **`CODESCOUT_RERANK` — the cross-encoder reranker is now opt-in and OFF by default.**
  Measured on the AMD profile (bge-reranker-v2-m3 Q4_K_M over llama-server) with sparse
  fusion on and a freshly rebuilt index, two arms differing in exactly one dimension:
  reranking scored 23/75 at a 1559 ms warm median against 26/75 at 990 ms without it —
  about 569 ms per query for a result that did not improve. Set `CODESCOUT_RERANK=1` to
  restore the previous behaviour. The cost is serving-runtime specific (the same weights
  over TEI measure ~80 ms), so this is a knob rather than a removal; measure on your own
  stack with `scripts/run-tc-benchmark.py`. Details:
  `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`.

- **Opt-in idle shutdown for the stdio server** (`CODESCOUT_IDLE_SHUTDOWN_SECS`).
  Exits after the given number of seconds with no tool call. Closes the accumulation in
  `docs/issues/archive/2026-07-28-mcp-servers-outlive-their-clients.md`, where a client that is
  abandoned but never exited holds stdin open and stays alive — so neither EOF nor a signal
  ever arrives, and one server per stale session piles up (18 measured, oldest 93.5 h,
  ~1 GiB RSS aggregate). Time since the last tool call is the only observable separating an
  abandoned session from a quiet one; `list_tools`, resource reads and pings deliberately do
  not count, or a client polling capabilities would pin the server open. **Disabled unless
  set, with no default** — how long a server may sit idle is a property of the operator's
  workflow, and exiting on a merely-quiet client costs a `/mcp` reconnect. The LSP mux's
  180 s is not a precedent: a mux is re-dialled transparently on the next navigation call,
  whereas an MCP server exiting is user-visible.
- **Worktree sessions now overlay the artifact catalog instead of forking it.**
  A session running from a linked git worktree reads the main checkout's
  artifacts live until it writes one. The first mutating call (`append_entry`,
  `update`, `artifact_event`, `artifact_augment`, `link`) forks that artifact —
  seeding a shadow row at the worktree path, a `worktree_fork` event carrying
  the fork-time base, and a `worktree_of` lineage link. `find`/`get` dedup
  shadow against main, annotating the winner `"overlay": true`. Fold a session
  back with `librarian(action="merge_worktree", root=…, dry_run=true)`, which
  applies each shadow's *delta* against its fork-time base rather than
  overwriting the main row; `abandon=true` drops the shadows instead.
  `delete`/`move` against a main-root artifact from a worktree session are
  refused — act from the main checkout. `doctor`'s `worktree_scoped_row` check
  and `fix=reseat_worktree` are now the legacy fallback, for rows with no
  active registration.
- **Catalog garbage collection with a grace period (schema v10).** A row whose
  file has vanished is stamped `missing_since` rather than dropped, hidden from
  `find` once past the grace window (listing and search only — `get` and
  `doctor` still see it), and reconciled back to visible if the file returns.
  `doctor` reports a `catalog_health` block (`hidden_rows`, `move_candidates`,
  `move_candidates_detail`), where move candidates are detected by commit-hash
  overlap between a dead row and a live one — i.e. the file was moved, not
  deleted. Reconciliation also runs throttled and best-effort on ordinary
  librarian calls, so a stale catalog heals without a manual pass.
- **`doctor` gained three repair modes.** `fix="prune_missing"` batch-drops rows
  under a dead root, `fix="rehome"` rewrites a moved root's ids while preserving
  every child row (events, observations, links, augmentation) via a deferred-FK
  id-rewrite, and `fix="reseat_worktree"` re-points worktree-scoped rows. All
  three are dry-run by default and require `confirm=true` to write.
- **`artifact(action="append_entry")` — atomic entry allocation for monotonic-ID
  trackers.** Assigns the next `<id_prefix>-<n>` from the live maximum across
  both the augmentation's `entry_collection` rows *and* the ids the markdown
  body already claims, so a body that ran ahead of params cannot reissue an id
  (the response carries a `warning` when params lags). Replaces the
  read-then-write dance for F-N / W-N / T-N logs.
- **Entry-grain citations (`entry_cite`, schema v9).** Artifacts gained a lazy,
  deduped, immutable `slug`, and entries can cite other entries: pass `cites`
  to `append_entry` with 16-hex artifact ids, `<slug>:<local>` entry ids, or
  unique rel_paths, and the edges are created atomically — an unresolvable or
  ambiguous ref aborts the whole call rather than guessing. `artifact(action=
  "get", include_links=true)` surfaces them. Not supported from a worktree
  checkout.
- **`librarian(action="link_scan")` — derive citation edges from prose.** Parses
  artifact bodies, resolves citation tokens against their defining headings, and
  materializes or prunes scanner-owned `rel="cites"` edges. Ambiguous tokens are
  reported, never guessed. `write=false` (the default) reports; `write=true`
  applies. Idempotent, and the repair path after moves or reindex — the catalog's
  abs_path pre-clean cascade-drops a moved artifact's links, and a re-scan heals
  them. First live run on this repo: 755 artifacts, 430 edges from zero.
- **`artifact(action="graft")` — merge one artifact into another.** Re-points the
  source's events, observations and links onto the destination, merges
  augmentation params with collision renumbering, flags near-duplicate entries
  for review, and deletes the source last.
- **Constitution trackers and `codescout constitution-check`.** A new tracker
  archetype holds project rules with optional path scoping. The CLI subcommand
  is a read-only, fast query for hook consumption: with `--path` it returns the
  rules matching that path (for a `PreToolUse` hook), without it the global
  path-less rules (for `UserPromptSubmit`). Always exits 0 and prints `[]` on
  any internal error, so a hook can never block on it.
- **`edit_markdown` now explains why an edit missed.** A failed `old_string`
  match returns a tiered diagnostic instead of a bare miss: nearest-match
  similarity scoring, whitespace and invisible-character classification rendered
  visibly, and detection of whether the target line sits inside a fenced code
  block. The hint adapts to which tier fired, and survives into `body_edits` on
  augmented artifacts. Batch edits also gained overlap detection and an
  end-to-start apply engine, so a multi-section batch cannot corrupt its own
  later offsets.
- **Opt-in startup dotenv, so the MCP launcher needs no env injection.** At
  startup codescout loads `$CODESCOUT_ENV_FILE`, or else
  `<global_config_dir>/.env` (e.g. `~/.config/codescout/.env`). A variable
  already present in the process environment always wins over the file. A
  missing default path is a silent no-op; a missing *explicit*
  `$CODESCOUT_ENV_FILE` is a surfaced warning. The current working directory is
  never read — a user-scoped server must not absorb an arbitrary repo's `.env`.
- **`get_guide("project-activation-bootstrap")`, auto-injected on activate.** A
  main-agent exploration protocol (load project memories and the open-bug ledger
  before exploring; route lookups by what you already know; verify at the bytes)
  that arrives with the first `workspace(action="activate")` of a session rather
  than waiting to be asked for.
- **`[ignored_paths] force_include` in `.codescout/project.toml`.** A list of
  globs the librarian indexer walks even though the repo's ignore files exclude
  them — for a `docs/trackers/` tracked only on a local branch. The
  supplemental walk is scoped to each glob's literal directory prefix rather
  than re-walking the repo with ignores disabled, and is best-effort: an absent
  or unparseable config resolves to no force-includes, never an error.
- **`librarian(action="reindex", reembed=true)`.** Content that had not changed
  was previously never re-embedded, so enabling embeddings or switching
  model/backend left the corpus silently un-vectorized. `reembed` requeues it,
  independent of `force`. The response now also reports `embedded` and an
  `embed_note`, because `unchanged: N` alone read identically whether N files
  needed no work or N files were skipped by mistake — which is exactly how the
  no-op stayed invisible.
- **A minimum chunk size for AST-split code (`AST_CHUNK_MIN`, 250 chars).** Inner-node
  decomposition emitted one embedding per declaration however small it was; on a
  real Kotlin corpus 5,133 of 43,582 chunks were single lines. Runs of adjacent,
  contiguous, undersized declarations now coalesce into one chunk. Only
  contiguous chunks merge, which keeps reconstruction byte-exact, and gap chunks
  are excluded so a container's declarations keep their own metadata headers.
  **Existing semantic indexes chunk differently after this change and should be
  rebuilt.**
- **Per-project index lock**, so two concurrent index builds on one project can
  no longer interleave writes.
- **Embed batch size is discovered from the sparse backend's `/info`** instead of
  being assumed, and sub-batches are pipelined order-preservingly.
  `CODESCOUT_EMBED_BATCH` overrides it.
- **Opt-in `artifact_vec` dimension migration**, for switching to an embedding
  model with a different vector width without discarding the catalog.
- **`grep` gained ripgrep-style flags and modes.** `ignore_case`, `whole_word`,
  `glob`, and `include_hidden` params; `mode="files"` for ranked per-file
  match counts instead of line-level hits; hits carry their enclosing symbol
  when the result set is small.
- **`ignored_paths` config is now honored by the code index.** The embedding
  indexer, background auto-index, and manual `index(action="build")` all skip
  configured ignore patterns, matching the file-tree walker's existing
  behavior.
- **Out-of-scope writes return a resumable acknowledgment instead of a hard
  denial.** `create_file`, `edit_file`, and `edit_markdown` now capture the
  attempted content behind an `@ack_*` handle when a write targets a path
  outside the active project; re-invoking with the handle replays the
  original write after `approve_write`.
- **`artifact` create/update accept custom frontmatter.** New `extra` param
  round-trips arbitrary YAML keys; `time_scope` is now wired through both
  `create` and `update` (previously silently dropped on `update`).
- **Background auto-index now gates on a heartbeat**, tagging the background
  indexing operation so a crash mid-index is distinguishable in OOM forensics
  from other work.
- **`contrib/pi/`** — codescout&lt;-&gt;Pi coding-agent integration: MCP config, a
  `codescout-mode` extension that promotes a codescout tool hot-set and
  disables Pi's native `edit`, `AGENTS.md` tool-map guidance, and an
  idempotent installer.

### Changed

- **`run_command` executes through Git Bash on Windows, not `cmd.exe`.** Commands now run
  under a POSIX shell on every platform, which is what makes the documented buffer-query
  idioms (`grep`, `tail`, `awk`, `tee`) work there at all — under `cmd /C` none of those
  binaries exist, so the documented workflow was simply unrunnable on Windows.
  **This makes Git for Windows a hard runtime requirement**: there is no `cmd.exe`
  fallback. The shell is resolved from `CODESCOUT_BASH`, then `CLAUDE_CODE_GIT_BASH_PATH`,
  then the standard Git install roots, then `PATH` — deliberately skipping
  `%SystemRoot%\System32`, whose `bash.exe` is the WSL launcher and would silently run
  every command in a different filesystem namespace. A host with no Git Bash now gets a
  `RecoverableError` naming the requirement instead of a bare `program not found`.
  Paths in commands follow POSIX rules on both platforms as a result (`C:/Users/...`, not
  `C:\Users\...`); buffer refs are substituted forward-slashed automatically. See
  `docs/manual/src/tools/workflow-and-config.md` and
  `docs/issues/2026-08-08-run-command-unusable-without-git-bash.md`.

- **Path parameters are aliased consistently across every tool.** `file_path`
  works wherever `path` does, and vice versa; a wrong-but-obvious param name is
  repaired and the call proceeds, with the hint naming the canonical form. The
  same repair-and-continue treatment covers inverted filter-AST leaves — the
  `{"eq": {"field": …, "value": …}}` shape is accepted and corrected rather than
  rejected.
- **The IL3 shell gate parses shell structure instead of matching raw text.** The
  unbounded-LHS-piped-to-a-trimmer check no longer trips on a pipe character
  inside a heredoc body or a quoted string.
- **`symbols` advertises body availability in overview mode**, not only in search
  results, so the follow-up call that fetches a body is discoverable from the
  first response.
- **`memory(action="read", sections=…)` follows the memory's shallowest heading
  level** rather than assuming `###`, so section filtering works on memories
  written with `##` sections.
- **Embedding sub-batch inflight defaults to 1**, not 4 — the higher default
  saturated modest backends and cost more in retries than it won in throughput.
- **`usage` tags errors with a tool-scoped `err_family`**, so recurring failure
  shapes are aggregatable per tool rather than a flat error string.
- **Dense embeddings are now always OpenAI-compatible.** Removed the
  `DenseProtocol::Tei` path and the `CODESCOUT_EMBEDDER_PROTOCOL` env var —
  every shipped profile (cpu/gpu/amd) already used the OpenAI-compatible
  `/v1/embeddings` endpoint (llama-server, vLLM, Ollama, OpenAI, or a
  corporate gateway). The dense leg is one code path now; nothing to
  configure. The reranker protocol toggle
  (`CODESCOUT_RERANKER_PROTOCOL=tei|infinity`) is unchanged.
- **Output buffers (`@tool_*` handles) dedupe by content hash (C-1)**,
  avoiding redundant storage when a tool call reproduces an identical large
  result.
- **`run_command` background-process heartbeats are now durable and
  per-operation-tagged**, surviving a `SIGKILL` so OOM forensics can identify
  which background operation was running at time of death.

### Fixed

- **The shipped build could prepend a dependency's diagnostic to `--json` output.**
  `qdrant-client`'s version-compatibility probe is on by default, and when it cannot
  reach a server it reports that with `println!` — onto codescout's **stdout**, ahead
  of the JSON envelope. Every `--json` CLI command that builds the librarian tool
  context was affected whenever Qdrant was down (Qdrant is the default artifact backend
  on a `server-stack` build): a correct payload, a zero exit code, and output no parser
  accepts. The librarian's own degradation path was never at fault — it catches the
  unreachable-Qdrant error and reports it through `tracing`; the `println!` fires inside
  the dependency, before any of our error handling exists to intercept it. The probe is
  now disarmed at both client-construction sites, which also removes a blocking health
  check from a call already wrapped in `spawn_blocking` + `timeout` to survive blocking.
  Only `cargo rb` — the configuration we ship, and the one no CI lane compiled until this
  cohort — was affected; the `server-stack` lane added in this cohort caught it on its
  first run. See
  `docs/issues/archive/2026-08-08-qdrant-compat-check-printlns-to-stdout.md`.
- **The chunk metadata header reached neither the embedder nor the payload.**
  `build_metadata_header` computes an identity line for every chunk —
  `src/foo.rs :: impl Bar :: fn baz(…)` — and has nine tests pinning its shape, yet
  nothing outside the chunker consumed it: `flush_pending` embedded `payload.content`
  alone and `stream_index` wrote `ast_header` as `String::new()`. Measured against the
  live collection: empty on **579,311 of 579,311** points, across every project and
  language. The legacy `embed::index` path *did* prepend it — its migration comment
  reads *"searchable header prepended before embedding"* — and the test pinning
  `{metadata}\n{content}` was deleted along with that module in `66db4c70`, so nothing
  failed when the surviving path turned out not to implement the contract. What a chunk
  looks like to the embedder is now `embed_text`'s decision and no one else's. Fixing it
  surfaced a second defect it would have shipped: the chunker was handed the *absolute*
  file path, so prepending the header as-was would have embedded the machine's checkout
  location into every vector — it now carries the same forward-slashed relative path the
  payload stores, which is what all 31 of the chunker's own call sites already passed.
  Reaching an existing corpus needs `index --force`: chunk ids are content-addressed and
  content is unchanged, so a normal sync skips every chunk by id. The retrieval benchmark
  has **not** been run — this restores a declared contract and is not yet evidence of
  better retrieval. See
  `docs/issues/archive/2026-08-08-metadata-header-computed-but-never-embedded-or-stored.md`.

- **`edit_code(action="remove")` now verifies what it wrote, and rolls back an edit that
  breaks the file.** `remove` — the one action whose entire purpose is deleting a range —
  had **no post-edit verification at all**: it wrote and returned `status: "ok"`, while
  `replace` next door checked for dropped symbols and rolled back. A removal whose range
  overshot by two lines took the closing `)` and `}` of the *preceding* function with it
  and reported success; the file was left syntactically invalid. `remove` now runs the same
  check as `replace`, and both gained a `SyntaxBroken` verdict for damage the symbol-level
  checks structurally cannot see — dropping a delimiter loses no symbol *name*, so the file
  was reported clean. The check only fires when the file parsed **before** the edit, so
  editing an already-broken file is never refused. See
  `docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md`.

- **`edit_code(action="rename")` no longer reports a partial rename as a success.** When
  the language server resolves no reference outside the declaration file — measured and
  reproducible on Kotlin for a top-level `object`, in a tree that compiled green — the
  rename edited only that file and returned `status: "ok"`. The call sites it missed were
  already in the same response under `textual_matches`, neither edited nor flagged, and
  `verify_hint` pointed the wrong way: it warned about over-reach *in changed files* while
  the failure was under-reach in *unchanged* ones. The rename now returns
  `status: "incomplete"` with `completeness_warning` and `uncovered_source_files` whenever
  the LSP reached only the declaration while other source files still name the symbol —
  the same disagreement check `references` already performed on the read path. The compact
  output is also corrected: it summed renamed and un-renamed occurrences into one "sites"
  figure, so a rename that changed 2 sites and missed 45 displayed as `47 sites`. See
  `docs/issues/archive/2026-08-08-edit-code-rename-under-reaches-and-reports-ok.md`.

- **`artifact`'s `extra` can no longer silently unclassify the artifact it writes.**
  `extra` is for frontmatter keys the schema does not model, but nothing stopped it naming
  one that it does. Because the writer emits the typed fields first and then appends
  `extra` verbatim, `extra={"kind": "bug"}` produced **two `kind:` lines in one YAML
  mapping** — which does not parse, so the artifact lost its kind, status, title, owners
  and tags together and dropped out of `artifact(find, kind=…)` entirely. The file still
  read correctly to a human and no gate noticed; one bug file sat outside the ledger for a
  working day. `create` and `update` now refuse an `extra` key that names a modelled field
  and name the parameter that owns it, and the writer additionally drops such a key rather
  than emitting a document that cannot be read back. See
  `docs/issues/archive/2026-08-08-artifact-extra-key-collision-unclassifies-silently.md`.

- **The `run_command` safety layer now reads a command the way the shell will run it.**
  Since the switch to executing through a POSIX shell on both platforms (`sh -c`, Git Bash
  `bash -c`), the gates still split on whitespace and matched raw substrings, so quoting or
  escaping could hide what a command actually does. `is_dangerous_command` now matches
  every pattern against the raw string **and** a shell-normalized form — a union, so
  nothing it caught before slips through — closing evasions such as `r''m -rf /tmp/x` and
  `rm -r\f /tmp/x`. The six pipeline and source-file helpers (`stage_trims`,
  `grep_is_counting`, `is_unbounded_lhs`, `has_recursive_flag`, `extract_grep_pattern`,
  `check_source_file_access`) now tokenize the same way, closing the sharper bypass:
  `'cat' src/main.rs` used to skip the source-file block entirely, because the quoted
  command name matched no blocked command.

  **This can flag commands that previously ran.** A flag is not a refusal — re-invoke with
  the returned `@ack_*` handle. One check got *looser* in the same pass: a quoted `-c` no
  longer makes a counting `grep` look like a log-trimmer, so `… | grep '-c' pattern` is
  allowed again. See
  `docs/issues/archive/2026-08-08-security-layer-tokenizes-unlike-the-shell.md` and
  `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md`.

- **`librarian` path matching could never succeed on Windows.** `containing_root` compared
  a catalog `abs_path` (stored forward-slashed and `//?/`-prefixed) against a root spelled
  with backslashes, so `artifact(action="move")` and `artifact(action="delete")` answered
  `no managed root contains …` for every row on that platform. Both sides now go through a
  normalized comparable form, and the match is segment-aware — a sibling directory sharing
  a name prefix still does not match. See
  `docs/issues/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md`.

- **MSYS argument conversion is no longer disabled on Windows.** `MSYS_NO_PATHCONV` and
  `MSYS2_ARG_CONV_EXCL` are now explicitly removed from the child environment rather than
  set, so MSYS-form paths (`/c/Users/...`) resolve for native binaries such as `git.exe`,
  and a value exported in the parent shell cannot change how commands resolve. See
  `docs/issues/2026-08-07-msys-pathconv-optout-breaks-native-exe-paths.md`.

- **`librarian(action="context", anchor_id=…)` no longer starves the anchor's
  neighbours.** The packing loop's "always include the first item" guard
  admitted an oversized anchor unconditionally and exhausted the token budget
  before any neighbour was considered. Anchor mode now reserves half the budget
  for neighbours and truncates the anchor's own section when it exceeds the
  reserve; topic-search behaviour is untouched.
- **`link_scan` no longer swallows every other tracker entry.** It enabled
  pulldown_cmark's `ENABLE_YAML_STYLE_METADATA_BLOCKS`, which pairs *any* two
  bare `---` lines as a metadata block — and session-log trackers separate
  entries with a bare `---`, so roughly every other entry heading was never
  registered as a link target. Frontmatter is now skipped by an explicit
  byte-offset guard instead. The post-fix rescan surfaced 16 previously
  invisible edges and pruned 8 stale ones.
- **`edit_code` with `action="replace"` preserves the symbol's doc comments**
  instead of dropping them with the rest of the lead region.
- **`edit_code` reindent no longer shifts the contents of multi-line string
  literals**, and samples its indentation base from a validated non-blank line.
  An unclosed `"` at end of line is now treated as opening a multi-line literal.
- **Tool state is never indexed.** Internal state files no longer enter the
  semantic index as if they were project content.
- **Disk-backed catalog connections set `busy_timeout`**, so a concurrent writer
  yields instead of failing the call outright.
- **The LSP mux dedups `cached_capabilities` by registration id**, fixing
  duplicate capability entries after repeated client registration.
- **Test lock files no longer leak into the per-user runtime directory**, where
  they outlived the test run and confused later sessions.
- **Memory recall no longer makes a wasted sparse-embedding HTTP call.**
  `HttpDenseEmbedder` (memory / dense-only retrieval) now hits only the
  dense endpoint via `EmbedderHttp::dense_query`, instead of computing a
  sparse vector it immediately discarded.
- **`edit_code` `insert`/`after` into a Kotlin class with backtick-named
  `@Test` methods** (`` fun `test name`() ``) now resolves the correct AST
  end line via `name_path` first, fixing a false "AST parse failed" on the
  annotation-to-`fun`-line gap.
- **kotlin-lsp's JVM heap is now capped (`-Xmx2g`)**, closing an
  unbounded-growth path surfaced during a 2026-06-19 MCP server OOM
  investigation.
- **`sync_project` now streams instead of loading the whole project into
  memory**, bounding indexing at O(batch) — the other fix from the same OOM
  investigation.
- **Heartbeat log pruning no longer evicts a recent crash victim's log**
  before it can be inspected — pruning is now age-floored.
- **`read_file`'s `json_path` supports quoted keys**, so dotted object keys
  (e.g. `"a.b.c"`) are reachable without being misparsed as a path
  separator.
- **`grep` decodes escaped newlines in `@tool_*` buffers**, so multi-line
  field values are searchable instead of collapsing to one line.

### Removed

- `CodePayload::ast_kind`. Declared, serialized, deserialized — and written as `""` at
  every construction site in the tree. It had no producer anywhere, so populating it
  would have meant inventing a value; a permanently-empty payload field is worse than no
  field. Points written before 2026-08-08 still carry the key and it is no longer read.

- **Benchmark matrix scaffolding** — `docker-compose.matrix.yml` and
  `scripts/chunk-model-matrix.py`. The tuned defaults they produced (chunk
  size, `CODESCOUT_BM25_BOOST`) remain baked in; the orchestration harness
  is recoverable from git history.
- **`shell_enabled` config flag and the vestigial `shell_output_limit_bytes`**
  — redundant with `shell_command_mode = "disabled"` and dead code
  respectively.

### Docs

- **Seven new manual pages** for subsystems that had none: worktree overlay,
  catalog GC &amp; repair, entry citations, `link_scan`, `artifact(action="graft")`,
  constitution trackers, and `edit_markdown` miss diagnostics. Also corrected
  `concepts/librarian-embedded.md`, which still advertised "15 librarian tools"
  including a standalone `workspace_state_at` — there are five, and
  `workspace_state_at` folded into `librarian(action="workspace_state_at")`.
- **Three new ADRs**: `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`
  (why a malformed-but-unambiguous tool input is repaired and continued rather
  than rejected), `docs/adrs/2026-07-20-artifact-vec-shared-catalog-boundary.md`,
  and `docs/adrs/2026-07-25-embedding-transport-boundary.md`.
- **`CLAUDE.md` cut from 42KB to 12.5KB** (677→184 lines), relocating Git
  release/ship procedures to `docs/RELEASE.md`, companion-plugin hook
  inventory to `docs/architecture/companion-plugin.md`, and prompt-surface
  operational rules to `src/prompts/README.md`. New gate
  `claude_md_contains_no_deprecated_tool_names` keeps it tool-name-consistent
  with the server-instructions prompt surface going forward.
- **`docs/SECURITY.md`** — vulnerability reporting process and an honest
  threat-model summary, including previously-undocumented limitations (the
  dashboard has no built-in auth; the default security profile is a
  deny-list, not a containment sandbox).
- Corrected a self-contradicting tool count in `docs/manual/src/architecture.md`
  and a stale `run_command` background-output example; removed documentation
  for a `denied_read_patterns` config option that was actually removed from
  the code in an earlier release.
## [0.14.0] — 2026-05-25

### Added

- **Surgical body editing for `artifact(update)` — `patch={body_edits: [...]}`.**
  Mirrors `edit_markdown`'s batch shape: each entry is `{heading, action,
  content? | old_string+new_string?, at?, replace_all?, include_subsections?}`,
  applied atomically. Mutually exclusive with `patch={body}`. Removes the
  temptation to write a partial body in the first place — the anti-pattern
  that cost a real ~600-line tracker on 2026-05-25 (`artifact(get,
  heading=…)` returns *a section*, but `patch={body: <section>}` *replaces
  the entire body*). See
  [docs/architecture/augmented-artifacts.md § Body editing surfaces](docs/architecture/augmented-artifacts.md)
  and the live MCP guide at `src/prompts/guides/librarian.md § Body Editing
  Surfaces`. Refs `docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md`.

### Changed

- **Body-write safety net: 50% shrink guard + `force=true` escape hatch.**
  Any `patch={body}` (or `edit_markdown`) write that would reduce the file
  by more than 50% is refused with
  `RecoverableError("body-shrink guard: …")`. The error hint names both
  `body_edits[]` and `force=true`. Exemptions: files under 200 bytes (the
  percentage is meaningless for shells), and augmentations with
  `append_mode + history_cap` set (legitimate history trimming).

- **Strict patch deserializer on `UpdatePatch`.** Misspelled keys like
  `body_prepend_section` now return `RecoverableError` listing the valid
  fields instead of silently no-opping. (Strictness applies to the patch
  body only; the outer `Args` still accepts dispatcher-injected fields.)

- **Forensic body-mutation trail.** Every body write emits a `field_patch`
  event with `payload={field: "body", prev_bytes, new_bytes, edits_count,
  mode, forced}`. Query via `artifact_event(action="list", artifact_id=X)`
  — a body write that shouldn't have happened is now reconstructable from
  the event timeline.

### Fixed

- **`call_graph` depth > 1 timeout.** BFS now parallelizes per-level LSP
  one-hop queries via `futures::try_join_all`; previous serial loop
  saturated at ~6s × N siblings, hitting the 60s tool timeout on depth=2
  walks across realistic call sites.

- **`call_graph` spurious self-edges in the tree-sitter fallback.** When
  rust-analyzer cannot model a symbol (e.g. `#[test]` fns), the
  `references()` fallback could return locations inside the symbol's own
  body, which the AST walk-up resolved back to the symbol itself. Filtered
  at the edge construction site.

- **Test isolation regressions.** `retrieval_unit` env tests now use the
  `temp-env` crate (drop `serial_test` markers; concurrent test runs no
  longer leak env into sibling tests). `symbols_no_body_start_line` pins
  the new auto-inline contract.

### Docs

- New section in `docs/architecture/augmented-artifacts.md` covering the
  body / `body_edits` / `force` surfaces, the shrink-guard exemptions, the
  forensic trail, and the anti-pattern story.
- New `## Body Editing Surfaces` section in the live MCP guide
  (`src/prompts/guides/librarian.md`).
- `librarian` guide action table now includes `doctor` and `audit_doc_refs`.
- `progressive-disclosure` guide documents the path-relative annotation
  convention.

## [0.12.1] — 2026-05-16

### Added

- **`audit_doc_refs` librarian action** — scans markdown for stale code refs
  (file paths, line refs, symbols, module paths, link targets) against the
  current filesystem and LSP symbol index. Emits findings as an `audit_issues`
  tracker at `docs/trackers/doc-ref-audit.md` (auto-created). Manual cadence
  in v1; `fail_on` flag available for downstream CI integration. See
  [docs/manual/src/concepts/audit-doc-refs.md](docs/manual/src/concepts/audit-doc-refs.md).

### Changed

- **LSP pool default capacity bumped 5 → 10** to support multi-worktree swarm
  workflows. Each `(language, project_root)` pair gets its own pooled LSP
  client; with 5 slots, a swarm of 3-4 worktrees touching 2 languages would
  thrash via LRU eviction and pay cold-start cost on every switch. 10 covers
  realistic parallel-agent setups while staying well under per-process memory
  ceilings. ([#5](https://github.com/mareurs/codescout/issues/5))

### Docs

- **README compacted and rewired to the manual.** Dropped detail-heavy sections
  (full retrieval-stack tables, Kotlin specifics, embedding model rundown) in
  favour of short summaries that link to the corresponding pages on
  [mareurs.github.io/codescout](https://mareurs.github.io/codescout/). Added a
  prominent **Artifacts** section advertising the embedded librarian — what it
  is, why it matters, and a four-call usage example.
- Manual is now advertised on the repo landing page: 3 badges (docs /
  crates.io / license), a 📖 callout linking to the manual root, `homepage`
  and `documentation` fields populated in `Cargo.toml` (so crates.io shows
  them), and 9 GitHub topic tags (`mcp`, `mcp-server`, `claude-code`, etc.)
  for discovery.
- Plugin documentation consolidated: there is only one plugin
  (`codescout-companion`), not two. Removed the stale "companion-plugin.md"
  duplicate and merged content into the single canonical page.

### Publishing

- **First publish of `codescout-embed` (0.1.0) and `librarian-mcp` (0.1.0)**
  to crates.io. Both were previously workspace-internal path dependencies and
  blocked `codescout` itself from publishing past 0.9.0. Now public.
- License unified to MIT across the workspace (was inconsistently Apache-2.0
  in `[workspace.package]` while the repo `LICENSE` file is MIT).

## [0.12.0] — 2026-05-16

### Breaking changes

- **Retrieval substrate replaced.** The in-process sqlite-vec + Tantivy index
  is gone. Semantic search now talks to a network-attached retrieval stack:
  Qdrant (`:6334`), a dense embedder service (`:48081`), a sparse SPLADE
  service (`:48084`), and a cross-encoder reranker (`:48083`). All four ship
  as a single `docker-compose.yml` with `cpu` and `gpu` profiles. See
  [`docs/manual/src/concepts/retrieval-stack.md`](docs/manual/src/concepts/retrieval-stack.md)
  for setup, Ollama/llama.cpp/OpenAI integration, and the benchmark we used
  to pick defaults.
- **`local-embed` is no longer the default Cargo feature.** Default build
  drops `local-embed` from defaults; `cargo install codescout` produces a
  network-only binary. Use `--features local-embed` to re-enable the
  in-process ONNX path; note that it skips the rerank + sparse fusion
  pipeline (benchmark penalty ~9 points on the 75-query suite).
- **Memory IDs are UUIDs now.** `memory.recall` returns string UUIDs (UUIDv5
  of `(project_id, bucket, title)`); the prior integer rowids no longer
  apply. `memory.forget` accepts UUID strings.

### Added

- **`codescout migrate-memories` subcommand** for moving legacy
  `.codescout/embeddings/project.db` content into Qdrant. `--dry-run` previews;
  the active-project banner shows a `⚠ LEGACY INDEX` hint when it detects an
  old file.
- **`CODESCOUT_RERANKER_PROTOCOL=tei|infinity`** env knob for swapping
  between TEI-protocol (default `bge-reranker-v2-m3`) and Infinity-protocol
  (e.g. `jina-rerank-v2`) rerankers without rebuilding.
- **`CODESCOUT_QUERY_PREFIX`** env for asymmetric retrieval models that
  require a query-side prefix (e.g. Nomic, BGE-large).
- **`semantic_search` mode parameter** — `code` (default) excludes markdown
  chunks for implementation queries; `full` includes all indexed content.
- **`call_edges` cache extracted to `.codescout/call_edges.db`** — call-graph
  data is now in its own SQLite file rather than co-housed with the deleted
  chunk storage.
- **Nav-tool eval harness** (`tests/e2e/nav_eval_harness.rs`,
  `tests/fixtures/nav-eval-rust/`). Library-level adversarial eval grading
  action-correctness of `symbols`, `symbol_at`, `references`, and `call_graph`
  on hand-authored Rust ambiguity traps. Run via
  `cargo test --test e2e_tests -- --ignored run_nav_eval`. Verdicts for
  rounds 1-4 under `docs/superpowers/specs/2026-05-15-nav-eval-round-*.md`.
- **Adversarial library-level eval for `edit_code`** — 14 cases across
  replace/insert/remove/rename, graded via composite (return + on-disk content
  + cargo check exit). Shared `eval_common` module factored out of nav-eval.
- **`call_graph` TS same-file fallback (LIMIT-001 Phase A).**
  `CachedResolver::lookup_pos` now falls back to a tree-sitter scan of the
  seed file when both pre-seeded positions and LSP `workspace_symbols` are
  unavailable, rescuing depth-≥2 BFS in LSP-down scenarios. Closes nav-eval
  C-11 (a→b→c→a cycle, depth=5).

### Changed

- **AMD ROCm profile in `docker-compose.yml`.** New `amd` profile alongside
  `cpu` and `gpu`. Dense embedder and cross-encoder reranker run as
  `rocm/llama.cpp:b6652_rocm7.0.0_ubuntu24.04_server` containers with
  `/dev/kfd` + `/dev/dri` passthrough; sparse SPLADE stays on CPU (no llama.cpp
  MLM path). Ports `48081`/`48083`/`48084`/`6334` are profile-agnostic — only
  the underlying container changes. Codescout ships `.env.amd` setting
  `CODESCOUT_EMBEDDER_PROTOCOL=llama-server` and
  `CODESCOUT_RERANKER_PROTOCOL=llama-server`. See
  [`docs/manual/src/concepts/retrieval-stack.md`](docs/manual/src/concepts/retrieval-stack.md)
  → "AMD ROCm profile".
- **Protocol aliases.** `CODESCOUT_EMBEDDER_PROTOCOL` and
  `CODESCOUT_RERANKER_PROTOCOL` now accept `llama-server` (and `llamacpp`,
  `llama_server`) as aliases for the OpenAI/Cohere-compatible shapes, so users
  configuring against llama-server's `/v1/embeddings` and `--reranking`
  endpoints can use the mental model of the backend rather than its wire
  protocol.
- **Binary size 54 MiB → 30 MiB (–44%).** Dropped `ort_sys`, `tokenizers`,
  `hf-hub`, `image`/`ravif` (~22 MiB) by moving `local-embed` out of defaults.
  Dropped `aws_lc_sys` (~2 MiB) by switching `rustls` to the `ring` crypto
  provider. Trimmed `qdrant-client` to `default-features = false, features =
  ["serde"]` to drop a duplicate `reqwest` dep tree.
- **`fs2` → `fs4`** for cross-process file locking (fs2 unmaintained since
  2018).
- **Tool hint strings** updated to current names (`index_project` →
  `index(action='build')`, `Run index_project()` → `Run index(action='build')`).
- **`symbols` `scope="project"`** now filters out stdlib/dependency matches
  whose path is outside the project root (only when scope is strictly
  `Project`, not `All`). Found via nav-eval round 3.
- **`call_graph` BFS** no longer aborts when a non-seed node's resolver
  returns `RecoverableError` — skips that hop and continues, matching the
  existing `lookup_pos` behavior. Found via nav-eval round 3.

### Removed

- `src/embed/index.rs` (~4900 LOC), `src/embed/drift.rs`, `src/embed/bm25.rs`,
  `src/embed/chunker.rs`, `src/embed/local.rs`, `src/embed/remote.rs`.
- `sqlite-vec` and `tantivy` dependencies (still pinned in `[workspace.dependencies]`
  for librarian-mcp, which retains its own sqlite-vec store).
- `percent-encoding` dep (URL handling is fully covered by the `url` crate).

### Fixed

- `semantic_search` now classifies retrieval-stack errors into actionable
  hints (which service is down, how to start it) rather than opaque
  transport errors.
- `memory.delete` (unified-tool path) correctly removes the anchor sidecar
  file alongside the memory entry.
- `create_semantic_anchors` now uses the cross-encoder reranker for anchor
  selection, raising precision.
- `edit_code(replace)` no longer strips preceding `///` doc comments or
  `#[...]` attributes when the new body omits them. The walk-back extension
  introduced for BUG-031 was unconditional; it now narrows back forward past
  decorator lines when `new_body` does not lead with one. Regression:
  `tests/symbol_lsp.rs::replace_symbol_preserves_doc_when_new_body_has_no_doc_comment`
  and edit-eval R-08 (BUG-055).
- Nav-eval runner grades transient LSP `-32801 / content modified` errors as
  SilentWrong (retryable) instead of Panic; the existing retry-on-warmup loop
  now absorbs LSP reindex races.

---
## [0.11.0] — 2026-05-06

### Breaking changes — tool consolidation

- **`replace_symbol`, `insert_code`, `rename_symbol`, `remove_symbol`
  consolidated into `edit_code`** with `action: replace|insert|remove|rename`.
  The four standalone tools are no longer registered. Migration:
    - `replace_symbol(name_path, path, new_body)` → `edit_code(symbol, path, action="replace", body=...)`
    - `insert_code(name_path, path, code, position)` → `edit_code(symbol, path, action="insert", body=..., position=...)`
    - `rename_symbol(name_path, path, new_name)` → `edit_code(symbol, path, action="rename", new_name=...)`
    - `remove_symbol(name_path, path)` → `edit_code(symbol, path, action="remove")`
- **GitHub tools unregistered** (`github_identity`, `github_issue`,
  `github_pr`, `github_file`, `github_repo`). Code remains in `src/tools/github.rs`
  but tools are not exposed to MCP clients. Use the `gh` CLI via `run_command`
  for GitHub operations.

### Added

- **`edit_code`** unified structural-edit tool (consolidates four prior tools).
- **`call_graph`** — transitive caller/callee traversal with `direction` and
  `max_depth` for impact analysis before refactoring.
- **`approve_write`** — session-scoped grant for writes outside the project
  root (e.g. user's home dotfiles).
- **`read_file` source-range gate** — line-range reads that overlap a named
  symbol redirect to `symbols(include_body=true)`. Bypass with `force=true`.
- **JVM pre-warm on activation** for Java/Kotlin projects — the JDTLS / Kotlin
  LSP process starts in the background during `workspace(action="activate")`
  rather than on first symbol query.
- **9 experimental features graduated** to stable docs (`concepts/` from
  `experimental/`): security profiles, diagnostic logging, memory sections
  filter, call_graph, auto-reindex, hybrid search, librarian guide resource,
  artifact_refresh list_stale, augmentation templates.
- **`symbol_at`** + **`references`** added to the prompt's pre-edit
  navigation strategy.

### Changed

- **Librarian-mcp default-on** — the embedded librarian indexer is compiled
  in by default. Runtime registration remains opt-in via `LIBRARIAN_ENABLED=1`
  or `[librarian] enabled = true` in `project.toml`.
- **Librarian tool collapse (16 → 5)** — `artifact`, `artifact_event`,
  `artifact_augment`, `artifact_refresh`, `librarian` cover what 16 individual
  tools did before.
- **`edit_code` rename/replace caller-check hint** appended to success
  responses so the LLM verifies call sites without a separate `references`
  call.

### Fixed

- `edit_code` propagates the caller hint through `format_compact` for large
  rename results (was previously dropped on overflow).
- Stale `replace_symbol` tip in `read_file` blocking error.
- LSP tool enforcement gaps where `symbols` / `references` could be skipped
  in favour of `Read` / `Grep`.

---

## [0.2.2] — 2026-03-11

### Added

- **Hardware-aware embedding model selection** — `onboarding` now picks the best
  available embedding model based on detected hardware (GPU/CPU/Apple Silicon),
  writing the optimal `embedding_model` into `project.toml` automatically.
- **`index_project` progress reporting** — live progress output during indexing
  (files processed, ETA) so long runs are no longer silent.
- **`project_status` trimmed output** — cleaner, more scannable status view with
  memory staleness section (`stale` / `fresh` / `untracked`).
- **Memory staleness detection** — `memory` tool tracks path anchors and semantic
  anchors; `project_status` reports which memories have drifted from their
  source files since last write.
- **Pre-onboarding semantic index gate** — prevents `semantic_search` from
  returning empty results on a freshly cloned project before indexing completes.
- **`language-patterns` shared memory** — curated per-language anti-patterns and
  correct idioms for 7 languages, consulted automatically before code changes.
- **CWD awareness in Agent** — `home_root` tracking so tools resolve relative
  paths correctly when invoked from a subdirectory.
- **LSP `RequestCancelled` retry** — LSP `-32800` errors are now retried
  automatically instead of surfacing as failures.

### Fixed

- **UTF-8 byte-slice crash in onboarding** (BUG-026) — preference text was
  sliced at a byte offset that could fall inside a multi-byte character, causing
  a panic. Now uses `char_indices` for safe truncation.
- **Stale `@bg_*` refs** — background command refs that have expired now return
  a `RecoverableError` with a descriptive hint instead of an opaque failure.
- **`.env` accidentally tracked** — added `.env` to `.gitignore`.

---

## [0.2.1] — 2026-03-09

### Fixed

- **`github_file` schema** — `files` array parameter now includes a proper `items`
  schema (`{ path, content }` object with required fields). VS Code and other
  spec-compliant MCP clients rejected the tool with *"tool parameters array type must
  have items"* because JSON Schema requires `items` on every `array` type.

---

## [0.2.0] — 2026-03-09


> **TL;DR:** The project was renamed from `code-explorer` to `codescout`. If you're
> migrating, update your MCP config and any scripts that reference the old binary name.
> [Full story and migration guide →](docs/manual/src/history.md)

### Breaking changes

- **Binary renamed:** `code-explorer` → `codescout`
- **MCP server ID renamed:** `code-explorer` → `codescout` — update `.mcp.json` or Claude Code settings accordingly
- **Tool renames** (API consistency):

| Old name | New name |
|---|---|
| `get_symbols_overview` | `list_symbols` |
| `find_referencing_symbols` | `find_references` |
| `replace_symbol_body` | `replace_symbol` |
| `insert_before_symbol` + `insert_after_symbol` | `insert_code` (+ `position` param) |
| `execute_shell_command` | `run_command` |
| `create_text_file` | `create_file` |
| `search_for_pattern` | `search_pattern` |
| `search_code` | `semantic_search` |
| `index_stats` | `index_status` |
| `get_current_config` | `get_config` |
| `check_onboarding_performed` | `onboarding` |

- **Tool consolidations** — `insert_before_symbol` + `insert_after_symbol` merged into
  `insert_code(position)`, `is_onboarded` folded into `onboarding(force)`

### Added

#### New tools
- `goto_definition` — LSP-backed jump to symbol definition; auto-discovers libraries
- `hover` — LSP type info and doc comments at a given position
- `insert_code` — insert code before or after a named symbol (replaces the separate
  `insert_before_symbol` / `insert_after_symbol` tools via a `position` parameter)
- `list_libraries` — list all registered external libraries and their index status
- `memory` semantic actions — `remember`, `recall`, `forget`, `refresh_anchors` added
  to the unified `memory` tool for vector-backed episodic memory
- `github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo` — five
  new GitHub tools backed by the `gh` CLI

> **Note:** `edit_lines` (line-splice editing) and `index_library` (separate library
> index tool) were drafted during this cycle but not shipped. Library indexing is covered
> by `index_project(scope: "lib:<name>")` instead.

#### Library search
- Symbol tools (`list_symbols`, `find_symbol`, `find_references`, `goto_definition`,
  `hover`) and `semantic_search` now accept a `scope` parameter: `"project"` (default),
  `"libraries"`, `"all"`, or `"lib:<name>"` for a specific library
- `LibraryRegistry` — persistent registry; libraries auto-registered via `goto_definition`
  when definitions resolve outside the project
- Manifest discovery auto-registers `Cargo.toml` / `package.json` / `go.mod` paths as
  named libraries

#### Semantic search improvements
- Incremental index: hash-based change detection (git diff → mtime → SHA-256 fallback);
  only changed files are re-indexed
- Semantic drift detection in `index_status` — surfaces files whose content has drifted
  significantly from their indexed embeddings
- sqlite-vec extension replaces hand-rolled Rust cosine loop for distance computation
- AST-aware chunker splits files by symbol boundaries before embedding
- `local-embed` feature flag: fastembed-rs LocalEmbedder for CPU-only inference,
  no Ollama required
- CPU fallback: automatically switches to local model when Ollama is unreachable
- Concurrent embedding with single-transaction writes for faster indexing
- `.cjs` and `.mjs` files now indexed as JavaScript

#### Progressive disclosure
- `OutputGuard` module enforces two output modes across all list/search tools:
  - **Exploring** (default): compact, capped at 200 items, overflow hint included
  - **Focused**: full detail with `detail_level: "full"` + `offset`/`limit` pagination
- `read_file` capped at 200 lines in exploring mode; explicit `start_line`/`end_line`
  bypasses the cap
- `next_offset` field in overflow JSON for seamless pagination

#### Robustness & DX
- Recoverable errors (`RecoverableError`) return `isError: false` with a `hint` field —
  sibling parallel tool calls are not aborted when one tool returns an expected error
- Dynamic server instructions injected into the MCP `initialize` response so Claude
  sees guidance before the first tool call
- `system_prompt` field in `.codescout/project.toml` for project-specific guidance
- Auto-detect project root from the server's working directory on startup
- Configurable per-language LSP init timeout via `lsp_init_timeout_secs`
- `text_sweep` helper: after `rename_symbol`, scans for residual textual occurrences
  (comments, strings, docs) that LSP rename cannot reach
- JetBrains official `kotlin-lsp` replaces community `kotlin-language-server`
- TSX/JSX LSP support via `typescript-language-server`
- tree-sitter support for Java, Kotlin, and TSX
- E2E test fixture projects with TOML-driven data harness
- Windows support: path separators, home directory in security checks, cmd.exe shell

#### Security
- Path sandboxing: all reads/writes validated against project root
- Tool category access controls (read, write, git, index, shell) configurable per-project
- Platform-specific deny-list (SSH keys, `/etc/passwd`, Windows credential stores)

### Changed

- `read_file` now rejects source code files (`.rs`, `.py`, `.ts`, etc.) — forces use of
  symbol tools; pass `start_line`/`end_line` only on non-source files
- `onboarding` redesigned: produces richer project context and memory-creation guidance
- Tool count: 33 → 30 (consolidation of insert tools, removal of git_log/git_diff)
- Default embedding model: `ollama:mxbai-embed-large`

### Removed
- `git_log` tool — use `run_command` with `git log` for file history
- `git_diff` tool — use `run_command` with `git diff` for diffs
- `replace_content` tool — superseded by `replace_symbol` and `edit_lines`

### Fixed
- Ghost blank lines in `replace_symbol` and `insert_code` when replacement body contains
  a trailing newline (`.push(body)` → `.extend(body.lines())`)
- `write_lines` empty-output guard: no longer writes `"\n"` when result is empty
- 1-indexed line numbers in all symbol/AST tool outputs (`start_line`, `end_line`)
- Concurrent `semantic_search` deadlock when multiple calls hit a cold LSP simultaneously
- LSP thundering-herd race condition on cold start (watch-channel barrier)
- LSP deadlock in waiter-retry path and excessive lock hold during shutdown
- Graceful LSP shutdown prevents orphaned language server processes
- `search_pattern` returns `RecoverableError` for invalid regex (not a hard error)
- Char-safe truncation in drift snippets (prevented panic on multibyte Unicode)
- HTTP timeout wired through to embedding client
- Hidden directories (`.worktrees`, `.claude`) excluded from all file walkers
- `git_blame` reads committed content correctly; better error for dirty files
- `SecuritySection::default()` now enables write/git/indexing tools (was too restrictive)

---

## [0.1.0] — 2026-02-25

### Added

#### Core MCP server
- Rust MCP server (`rmcp` 0.1) with 29 tools across 8 categories
- Stdio and HTTP/SSE dual transport — stdio for Claude Code, HTTP for multi-session use
- Library + binary split (`src/lib.rs`) enabling integration tests and external use
- Release profile: `opt-level 3`, thin LTO, symbol stripping

#### File tools (3)
- `read_file` — read files with optional line range
- `list_dir` — directory listing, recursive mode
- `search_for_pattern` — regex search across project files

#### Workflow tools (3)
- `execute_shell_command` — run shell commands in project root
- `create_text_file` — create or overwrite files
- `find_file` — glob pattern file discovery

#### Symbol tools — LSP-backed (7)
- `get_symbols_overview` — hierarchical symbol tree for a file or directory
- `find_symbol` — workspace-wide symbol search by name pattern
- `find_referencing_symbols` — find all usages of a symbol
- `rename_symbol` — rename across the whole workspace
- `replace_symbol_body` — replace the body of a symbol
- `insert_before_symbol` / `insert_after_symbol` — precise code insertion
- JSON-RPC 2.0 LSP client with async stdio transport, 30s timeout, crash recovery
- Language server configs for 9 languages: Rust, Python, TypeScript/JS, Go, Java,
  Kotlin, C/C++, C#, Ruby

#### AST tools — tree-sitter offline (2)
- `list_functions` — extract all function/method signatures
- `extract_docstrings` — extract doc comments with associated symbol names
- **Rust** (`tree-sitter-rust`): functions, structs, enums, traits, impl methods,
  modules, constants — `///` and `//!` doc comments
- **Python** (`tree-sitter-python`): functions, classes, methods, decorated definitions
  — triple-quoted docstrings
- **Go** (`tree-sitter-go`): functions, methods with receiver type, structs, interfaces
  — `//` and `/* */` comments
- **TypeScript** (`tree-sitter-typescript`): functions, classes, interfaces, enums,
  type aliases, export statements — JSDoc `/** */`
- **TSX** (`tree-sitter-typescript` LANGUAGE_TSX): full JSX grammar — same extraction
  as TypeScript
- **Java** (`tree-sitter-java`): classes, interfaces, enums, records, methods,
  constructors, fields, enum constants — Javadoc `/** */`
- **Kotlin** (`tree-sitter-kotlin-ng`): classes, objects, functions, properties, enums,
  companion objects, type aliases, enum entries — KDoc `/** */`

#### Git tools (3)
- `git_blame` — per-line blame with commit SHA, author, timestamp
- `git_log` — file commit history
- `git_diff` — working tree or commit-range diff

#### Semantic search tools (3)
- `search_code` — vector similarity search over indexed codebase
- `index_project` — build/update embedding index (chunked, content-hashed)
- `index_status` — show index stats and coverage

#### Memory tools (4)
- `write_memory` — store named notes per project
- `read_memory` — retrieve a note by topic
- `list_memories` — list all stored topics
- `delete_memory` — remove a note

#### Config tools (2)
- `activate_project` — switch active project root
- `get_current_config` — show config and project root

#### Onboarding tools (2)
- `onboarding` — project discovery: detect languages, structure, create config
- `check_onboarding_performed` — check if onboarding has run

### Infrastructure
- `.mcp.json` — Claude Code MCP config for using codescout on its own source
- 141 tests: 136 unit + 5 end-to-end integration tests
- Integration tests cover: read→search→replace, AST analysis, memory+config roundtrip,
  git history creation, onboarding+explore


## [0.10.0] — 2026-05-01

### Breaking changes — tool surface compression (L3)

| Old name | New name |
|----------|----------|
| `find_symbol`, `list_symbols` | `symbols` |
| `find_references` | `references` |
| `goto_definition`, `hover` | `symbol_at(fields: ["def", "hover"])` |
| `list_dir`, `find_file` | `tree` |
| `activate_project`, `project_status` | `workspace(action: activate / status / list_projects)` |
| `list_libraries`, `register_library` | `library(action: list / register)` |
| `index_project`, `index_status` | `index(action: build / status)` |

Added: `call_graph` stub (implementation tracked in item A).

---
