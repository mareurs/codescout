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

Three rules, each earned by a measured failure in this repo. They cost more to skip than
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
| [`scripts/extract-kotlin-tcs.py`](../scripts/extract-kotlin-tcs.py) | Mines `(query, expected_files)` ground-truth pairs from a project's `usage.db`: for each `semantic_search`, the files touched by `read_file`/`edit_file`/`symbols`/`edit_code` within 300s in the same session | `python3 scripts/extract-kotlin-tcs.py …` → JSON list | **The name is misleading** — nothing in it is Kotlin-specific; it reads any project's `usage.db`. The 300s same-session window is a *behavioural* ground truth, i.e. a heuristic, not a labelled set |

## Built-in `librarian` scans

Read-only by default. These are the probes you already have without leaving the tool.

| Action | Measures | Note |
|---|---|---|
| `librarian(action="doctor")` | Catalog drift: `abs_path` form, ADS colons, `..` segments, missing-on-disk files, worktree-scoped rows, frontmatter-id vs catalog-id mismatch | Read-only unless you pass `fix=…`. Opt-in repairs are individually gated and dry-run by default |
| `librarian(action="audit_doc_refs")` | Stale code references in markdown — paths, symbols, line refs, link targets — against the filesystem + LSP index | Manual; run before a doc-heavy merge. **Many findings are false positives** by design: any backticked token that looks like a path is checked, so `message.id`, `usage.db` and `/tmp/...` all register. Read the severities, not the count |
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
