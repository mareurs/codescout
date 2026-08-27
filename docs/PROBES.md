---
kind: tracker
status: active
tags: ["probes", "measurement", "instruments"]
title: PROBES — the measurement instruments in this repo
---

# PROBES — the measurement instruments in this repo

A single-page index of everything that **measures** this project: standalone scripts,
built-in `librarian` scan actions, and skill-driven analyses. Use it when you are about to
answer a question with a number and want to know whether an instrument already exists.

This page is the index, not the spec — follow the links for each probe's own
documentation. Sibling of [`TAXONOMY.md`](TAXONOMY.md), which indexes ID prefixes rather
than instruments.

## What counts as a probe

A probe **reports on the system's state or history** and is expected to be re-run and
compared over time. That is the whole test, and it is deliberately narrow:

- ✅ measures something and prints/returns a result (`friction-probe.py`, `doctor`)
- ✅ scores or ranks against a corpus (`run-tc-benchmark.py`, `legibility_scan`)
- ❌ **builds, installs, fetches, or captures** — `build-windows.sh`, `install-lsp.sh`,
  `install-ollama.sh`, `fetch-models.sh`, `retrieval-stack.sh`, `capture-screenshot.sh`,
  `capture-dashboard.js` are infrastructure, not instruments. They are in `scripts/` and
  are deliberately **not** listed below.

---

## Before you trust any probe on this page

Four rules, each earned by a measured failure in this repo. They cost more to skip than
to follow.

1. **Calibrate against a known-good instrument on the overlap before extending one.**
   `friction-probe.py` was checked against TU-7's published immediate-repeat figures first
   — ratios 0.9 / 0.82 / 0.63 with the ordering preserved — and only then trusted for the
   families TU-7 never measured. A ratio near 1 licenses the extension; a large one
   localises a defect in the *new* instrument.
2. **Name what the predicate literally counts, then compare that to your question.**
   `cc.py` summed `message.usage` per JSONL line for months, inflating every cost figure
   **2.1–2.6×**, because one completion is written as 2–3 lines each carrying an identical
   usage dict (fixed 2026-08-20, `llm-proxy:38d80eb`). It returned a plausible number and
   no error, which is why nothing raised.
3. **A truncated read is a floor, never a count**, and **a zero is evidence about the
   search, not about the world.** Check whether your filter selects *against* the
   population you are characterising — it has, here, more than once.

4. **The predicate you supplied from memory is the one that fails silently — run a
   positive control.** Rules 1–3 are about reading output someone else produced. This one
   is about the instrument you just wrote: a path, a pattern, a sort key, a field name.
   Three in one session, 2026-08-21 — a version-keyed plugin cache probed at a flat path
   (empty output, **not** `0`, and nearly reported as one); a backtick eaten by
   `grep -c "$s"`, returning `0` for a string present exactly once; and `sort` over
   `ps lstart`, which orders by **weekday name** and ranked two-day-old processes as the
   newest on a machine whose newest was 17 seconds old. **The third returned no zero at
   all** — a complete, well-formatted, entirely wrong answer — which is why rule 3 does
   not reach it. Before believing a result, make the instrument find or rank one case
   whose answer you already know. (`reconnaissance-patterns:R-104`, widened.)

The long form, with the incident list, is the **Measurement** iron rule in `CLAUDE.md`.

---

## Standalone scripts

| Probe | Measures | Invoke | Know before you run it |
|---|---|---|---|
| [`scripts/friction-probe.py`](../scripts/friction-probe.py) | Agent friction from `.codescout/usage.db`: `err_family` coverage by tool, immediate-repeat rate (TU-7's discriminator), calls-to-recovery, consecutive-error runs, and the unclassified population | `python3 scripts/friction-probe.py [--days N] [--calibrate] [--null-detail] [--json]` | Defaults to `session_id` (per-process, correct), **not** `cc_session_id` (wrong on ~31% of rows). `--calibrate` compares against TU-7 before you believe anything else. Bounded by the 30-day retention sweep — every count is a floor |
| same, compare mode | Before/after across a cutoff, **mix-adjusted** | `python3 scripts/friction-probe.py --split-at "<UTC>" --clean-only` | Handles workload mix (direct standardisation), dirty builds (`--clean-only`), and reports reconnect blending. Prints `do not quote the delta until the power line is satisfied` — heed it: as of 2026-08-20 a 24h window is **4.6× short** of the data needed |
| [`scripts/run-tc-benchmark.py`](../scripts/run-tc-benchmark.py) · [`.sh`](../scripts/run-tc-benchmark.sh) | Retrieval quality — the TC suite, Qdrant stack only since Phase 7 | `./scripts/run-tc-benchmark.sh > /tmp/stack.json`, or the `.py` with `--binary` / `--project-path` | Scores against expected-file lists naming **codescout's own tree**, so any other corpus yields meaningless numbers. Results log: [`trackers/retrieval-benchmark.md`](trackers/retrieval-benchmark.md) |
| [`scripts/sweep-bm25-boost.sh`](../scripts/sweep-bm25-boost.sh) · [`sweep-bm25-cr1200.sh`](../scripts/sweep-bm25-cr1200.sh) | Score table across `CODESCOUT_BM25_BOOST` values; the `cr1200` variant sweeps the current best cell and indexes once | `./scripts/sweep-bm25-boost.sh [binary]` | **The expected-file lists are relative to the PINNED bench corpus** (`.worktrees/bench`, detached at `ede25e69`) — not to current HEAD. Several of those paths have since been deleted or merged on `experiments`, so checking them against HEAD tells you nothing |
| [`scripts/probe_entry_attribution.py`](../scripts/probe_entry_attribution.py) | Entry-grain citation attribution — whether a `PREFIX-N` citation belongs to the entry whose heading precedes it. Compares the naive nearest-preceding-heading rule against a section-bounded one and reports the disagreement rate | `python3 scripts/probe_entry_attribution.py --calibrate --fence-audit [--ground-truth N] [--json]` | The headline precision is **agreement between two algorithms**, with the bounded rule assumed correct — *not* a labelled set. `--ground-truth N` prints disagreement rows to check by hand. Scope is ledgers declaring `entry_prefix` under `docs/trackers/` excluding `archive/`, so it says nothing about specs, plans, bug files or ADRs, which also carry `PREFIX-N` tokens. Fence detection is a naive ``` toggle — an unbalanced fence silently inverts every line after it, so run `--fence-audit`. Baseline 2026-08-20: **87.9%**, errors 89% concentrated in four ledgers |
| [`scripts/stale-servers.sh`](../scripts/stale-servers.sh) | Which running codescout servers execute a **deleted** binary — i.e. which Claude Code sessions are still serving a previous build's guides, prompt surfaces and guide routing. The process axis of the freshness law, made countable | `./scripts/stale-servers.sh` | Counts `/proc/<pid>/exe` resolving to `… (deleted)`; that is **not** a version comparison — a byte-identical rebuild also reads STALE, because the inode changed. `pgrep -x codescout` matches argv[0] exactly, so a server behind a wrapper is invisible. A deleted inode carries no version, so this says *not current*, never *which build*. Baseline 2026-08-21: **21 of 26 stale, oldest 17 days** |
| [`scripts/extract-kotlin-tcs.py`](../scripts/extract-kotlin-tcs.py) | Mines `(query, expected_files)` ground-truth pairs from a project's `usage.db`: for each `semantic_search`, the files touched by `read_file`/`edit_file`/`symbols`/`edit_code` within 300s in the same session | `python3 scripts/extract-kotlin-tcs.py …` → JSON list | **The name is misleading** — nothing in it is Kotlin-specific; it reads any project's `usage.db`. The 300s same-session window is a *behavioural* ground truth, i.e. a heuristic, not a labelled set |
| [`scripts/probe_entry_read_grain.py`](../scripts/probe_entry_read_grain.py) | Where entry-grain reads land: the two exactly-attributed paths (`artifact(get, heading=…)`, `librarian(context, anchor_id=<slug>:<local>)`), the two-hop buffer **leak** Layer 5a was scheduled to close, and the artifact-grain population beside it — all bucketed into three eras. Holds the trigger that would reopen 5a | `python3 scripts/probe_entry_read_grain.py [--window-hours 30] [--trigger] [--collisions] [--integrity] [--json]`; `--trigger` exits **1** when the reopen condition fires | The leak count is a **floor**: only two-hop chains are followed, and usage.db is retention-swept. Every join is scoped to `session_id` (not `cc_session_id`, wrong on ~31% of rows) because `store_tool` mints handles from a truncated 32-bit timestamp *and* dedups by content hash — run `--collisions` rather than trusting that; baseline 2 of 253. `names_entry_token` matches the token word-shape, so a heading merely *mentioning* `R-91` counts as an entry read — it over-counts, never under. Buckets are wall-clock, not workload: read the ratio against `artifact_calls`, never the raw count. Baseline 2026-08-21 (recent 30h / prior 30h / older ≈16d): leaked entry-grain **4 / 4 / 30**, anchored entry-context **4 / 0 / 0** — and all four are one anchor from Layer 4's own smoke tests, so the trigger reads `holds`. Calibrated against hand-written SQL on the same window: exact on every era-invariant metric |
| [`scripts/probe_import_rename_density.py`](../scripts/probe_import_rename_density.py) | How often real Python reaches a symbol under a **different name** (`from X import Y as Z`, `Z != Y`) — the CHASE_REQUIRED mechanism of the blast-radius eval. AST-based, reports the population rate *and* the rate conditional on symbols renamed at least once | `python3 scripts/probe_import_rename_density.py ROOT [ROOT …]`; `--help` lists the baseline corpora | **The headline is the CONDITIONAL rate and the two differ by 11×** — 3.31% of all binding sites, 38.1% restricted to symbols renamed somewhere. Quoting the population rate answers a different question. "Renamed at least once" is corpus-size-dependent, so a larger corpus qualifies more symbols by construction. Counts binding **sites**, not files. Star-imports (691 at baseline) hide symbol identity entirely. Baseline 2026-08-26: 8,517 files / 72,968 sites, conditional **38.1%**, `__init__.py` re-export surface 42.6%. Kotlin comparison, by grep: **16 aliased of 16,703 imports (0.10%)** |
| [`scripts/probe_string_dispatch.py`](../scripts/probe_string_dispatch.py) | How often a callable is reached by a **string** — `getattr(x,"n")`, `D["n"]()`, `globals()["n"]`, or a config key — the LEXICAL_ONLY mechanism. Unit is the **file** | `python3 scripts/probe_string_dispatch.py ROOT [ROOT …]` | Restricted to **DISTINCTIVE** names (len ≥ 8, snake_case, defined exactly once) because bare-name reference matching is invalid otherwise: the unrestricted first version collected **495,054 "references" across 1,209 names** (~409 each — `get` matching every `.get(`) and reported 0.62%, wrong by 14×. The restriction excludes short and common names, where string dispatch may well be commoner. Config scan is word-boundary regex; requiring quotes silently dropped YAML/TOML bare keys and undercounted 5× (13 → 66 names). Baseline 2026-08-26: **8.5%** of dependent files reach a callable *only* by string, while **18.5%** of all files contain string dispatch somewhere — those answer different questions, do not swap them |
| [`scripts/probe_librarian_scope.py`](../scripts/probe_librarian_scope.py) | How the librarian's `scope` surface is actually used: every action bucketed by how it treats scope (`scope_aware` / `scope_adhoc` / `id_addressed` / `machine_wide`), the explicit-vs-omitted rate, **requested → APPLIED** read from each response's own `scope` block, cross-repo yield cross-tabbed against the scope passed, `scope_fallback` fires, and (`--counter-test`) what the narrow default costs — zero-result rate by scope plus widening retries | `python3 scripts/probe_librarian_scope.py [DB…] [--crosstab] [--counter-test] [--since TS] [--json]`; with no DB args it scans `~/work` + `~/agents` | **The routing table is STATIC**, transcribed from `src/librarian/tools/*.rs` — if those files change it lies; re-verify with `references(symbol="apply_scope", …)`. Two ways it has already been wrong, both reporting clean numbers: usage.db stores the **MCP content envelope**, so walking it without `unwrap()` finds no paths at all and reports a 0% cross-repo rate that reads as perfect isolation; and counting *any* absolute path as foreign made codescout its own largest foreign root (1407 hits), because `create`/`doctor`/`get` render absolute for in-project rows too. Worktree roots are normalised via `repo_root_of` — without it the overlay's main-checkout arm reads as 5 phantom default-scope leaks. Cross-repo detection is path-string based, so `get`-by-id — the main cross-repo vector — is structurally **under**-counted. The widening-retry count is a **ceiling** (pairs on session+time, not on question). One db per server root: a single-db run is one repo's view, not the machine's. Retention-swept ⇒ every count is a floor. Baseline 2026-08-27 (13 dbs, 6,445 calls, 24d): 76.6% `id_addressed`, 84.0% of scope-accepting calls omitted `scope`, `find`+omitted → **`repo`** 822×, `link_scan`+omitted → `project`, and `project == repo root` in **100%** of 923 scope blocks |
| [`scripts/probe_dependency_vectors.py`](../scripts/probe_dependency_vectors.py) | Prevalence of six dependency mechanisms a name-based search cannot follow: assembled `getattr` names, monkey-patching, string-keyed decorator registries, entry points, callbacks (inverted direction), inheritance | `python3 scripts/probe_dependency_vectors.py ROOT [ROOT …]` | `monkeypatch` requires the assignment target's root to be an **imported** name; without that filter it matches every `self.foo = bar` and reads **38.4% instead of 2.6%**. `callback` counts any locally-defined function passed as a bare `Name` argument, so `sorted(x, key=f)` counts — cross-file callback *registration* is lower. `inherit` counts library base classes, not intra-project ones. `entrypoint` sees only the Python-side `entry_points()` call, never the `pyproject.toml` declaration, so 0.2% is a floor. Baseline 2026-08-26 (8,379 files): inherit **48.0%**, callback **21.6%**, assembled **9.4%**, registry **5.2%**, monkeypatch **2.6%**, entrypoint **0.2%** |
| [`scripts/probe_guide_injection.py`](../scripts/probe_guide_injection.py) | Guide **auto-injection delivery** across Claude Code transcripts: bytes pushed per session, per-topic share, duplicate share, which tool triggered each topic, and the dev/normal and pre/post-fix slices | `python3 scripts/probe_guide_injection.py [--main-only] [--fix-date YYYY-MM-DD] [--json]` | **Delivery, never use** — the use half is not scriptable (see [`evals/2026-08-27-guide-injection-use.md`](evals/2026-08-27-guide-injection-use.md), n=81 injections, 66.7% unused). Counts **auto-injections only**, not explicit `get_guide()` fetches — so a topic reading 0 here may still be fetched. Two traps already burned: every injection carries **two** markers (opening + closing) so naive counting doubles, and guide files **grow** (`tracker-conventions` 10,377 → 34,333 B in ten days), so applying today's file size to a historical injection overstated the corpus **1.17×** — this measures the bytes actually between the markers. Guard rejects marker-bearing `tool_result`s whose trigger is not `mcp__codescout__*` or which carry >2 markers (a transcript being *read*, not an injection); 3 of ~4,200 at baseline. Dedupes session-ids present under two profiles (49 of 1,705). Subagent transcripts **share the parent's ledger**, so `--main-only` is the session-level view. Baseline 2026-08-27: 1,705 unique sessions, post-fix clean slice **31,899 B/session, median 3 injections, 8% duplicate**; `librarian`+`tracker-conventions` = **72%** of bytes; **4 topics never auto-injected** (28,186 B, 27% of the corpus) |

## Built-in `librarian` scans

Read-only by default. These are the probes you already have without leaving the tool.

| Action | Measures | Note |
|---|---|---|
| `librarian(action="doctor")` | Catalog drift: `abs_path` form, ADS colons, `..` segments, missing-on-disk files, worktree-scoped rows, frontmatter-id vs catalog-id mismatch | Read-only unless you pass `fix=…`. Opt-in repairs are individually gated and dry-run by default |
| `librarian(action="audit_doc_refs")` | Stale code references in markdown — paths, symbols, line refs, link targets — against the filesystem + LSP index | Manual; run before a doc-heavy merge. **Many findings are false positives** by design: any backticked token that looks like a path is checked, so `message.id`, `usage.db` and `/tmp/...` all register. Read the severities, not the count |
| `index(action="verify")` | Semantic-index integrity: coverage against the indexer's **own** walk (`expected_files` vs `stored_files`, `missing_*`, `orphan_*`), `empty_eligible_dirs` (an eligible top-level dir with files on disk and none indexed), `chunks_without_vectors`, and `git_sync` — reduced to one `verdict`: `complete` / `stale` / `incomplete` | Read-only; never prunes, because a bad walk reporting everything as an orphan would otherwise delete a live index. **Use this instead of `sqlite3` on `.codescout/embeddings/*.db`** — that file may belong to a backend that is not in use (`CODESCOUT_VECTOR_BACKEND` unset defaults to Qdrant under `server-stack`), and it answers plausibly anyway: `bug-fix-session-log:F-66`. **`chunks_without_vectors` is a measurement only under sqlite-vec**; Qdrant returns 0 structurally, since a point carries payload and vector together. `missing_*` is EXPECTED to be non-zero whenever `git_sync` is behind — that is what the `stale` verdict means, and reading it as breakage is the false alarm the arm ordering exists to prevent. Samples cap at 20; counts are exact |
| `librarian(action="legibility_scan")` | Refactor candidates, ranked by *observed* cost — joins `usage.db` friction to the AST symbol index | Consumes `friction_target`, which is NULL on ~96.6% of rows and only ever populated on friction rows (errors/overflows), so its input is sparse by construction |
| `librarian(action="link_scan")` | Citation graph health: dangling and ambiguous `PREFIX-N` tokens, `cites` edges | `write=false` reports, `write=true` materialises. Idempotent |

## Skill-driven

| Skill | Measures | Note |
|---|---|---|
| `analyze-usage` | Nine canonical SQL queries over `usage.db`: tool popularity, error breakdown, overflow, latency buckets, slow commands, per-session summary, LSP starts and failures | **No error-taxonomy query keyed on `err_family`** — a gap worth knowing when you are asked to "measure frictions"; use `friction-probe.py` for that axis |
| `claude-traces` | Session cost, message timeline, tool sequences (`cc.py`, JSONL) and token/profile/per-call detail (`lf.py`, Langfuse) | `cc.py` reads **only `~/.claude`** — it ignores `CLAUDE_CONFIG_DIR`, so it is blind to `~/.claude-sdd` and `~/.claude-kat` (filed: `llm-proxy:docs/issues/2026-07-10-ccpy-config-dir-hardcoded-and-path-encoding.md`). Pass explicit directories |

## Cross-repo

| Probe | Measures | Note |
|---|---|---|
| `llm-proxy` · `lf.py mismatches --check` | Requested-vs-served model divergence (silent fallback detection) | Drives a standing systemd `--user` tripwire (`llm-mismatch-watch.timer`); a reroute leaves the oneshot `failed` |
| `llm-proxy` · `scripts/record_shim.py` | What a Claude Code client actually **sent**: the request's `thinking` object, `stream` flag, `anthropic-beta` header, model and tool count — keyed by session id. Run it on `:8099`, it forwards to the real proxy on `:8082` | The proxy does **not** log any of this (`build_langfuse_input` keeps system/messages/tool_names/tool_count only), so no request-shape question is answerable from past traces — the observation must be made fresh. **An export cannot point a client at it**: Claude Code resolves `ANTHROPIC_BASE_URL` as `CLI --settings > profile settings.json > shell env`, so use `--settings '{"env":{"ANTHROPIC_BASE_URL":"http://localhost:8099"}}'`. Match records to runs by `session_id`, never by prompt text |
| `llm-proxy` · `scripts/thinking_by_session.py` | How much thinking **text** reached Langfuse per session — blocks, chars, signed count | `0 blocks` (model did not think) and `1 block / 0 chars / signed` (capture failure) are **different results**; do not merge them. `traces=0` right after a run is ingestion **lag** — re-query in ~30 s before concluding absence; both were observed 2026-08-27. Pairs with the shim: separately each is compatible with several causes of `thinking: ""`, together they discriminate |

---

## Open questions on this page

- **TC-suite size disagrees with itself.** `run-tc-benchmark.py`'s docstring says a
  **20-TC** suite; `trackers/retrieval-benchmark.md` is titled a pinned **25-TC** log.
  Recorded rather than resolved — whoever next runs the benchmark should settle which is
  current and correct the loser.

## Adding a probe

1. Put it in `scripts/` if it is standalone. Give it a `--help` and a module docstring that
   states **what each predicate literally counts**, not just what it reports.
2. Document its blind spots *in the code*, next to the predicate. `friction-probe.py` is
   the worked example: every metric's docstring says what it cannot see, and the compare
   mode refuses to hand over a delta that its own power check does not support.
3. Add a row here. If it has a caveat that would make a reader mis-trust a number — a
   pinned corpus, a hardcoded path, a heuristic window — the caveat belongs in the
   *"Know before you run it"* column, not in a footnote nobody reaches.
4. If it produces a durable series, point it at a tracker under `docs/trackers/` so the
   numbers accumulate somewhere queryable instead of scrolling past in a terminal.
5. If it lives in a sibling repo, add it to **Cross-repo** above *and* give that repo a
   tracker holding the usage and the traps — a probe reachable only by remembering it
   exists is not indexed. The worked example is
   `llm-proxy:docs/trackers/observability-instruments.md`, which the codescout umbrella
   reaches via `artifact(action="find", scope="umbrella", …)`.
