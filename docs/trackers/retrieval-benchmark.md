---
id: cc4843e5c1a020bd
kind: tracker
status: active
title: Retrieval Benchmark — pinned 25-TC log
owners:
- '@mareurs'
tags:
- retrieval
- benchmark
- qdrant
- embedding
expects_augmentation: docs/augmentations/docs-trackers-retrieval-benchmark.yaml
---

## Why this tracker exists

The 20-TC numbers scattered across `docs/research/2026-04-03-embedding-model-benchmark.md`
(23–41/60 range) are **not comparable to each other**. Each run used a different codebase
HEAD, different chunking, different fusion config, different embedder protocol — and the
harness recorded none of those. The "best" historical result (41/60 for CodeRankEmbed
hybrid on 2026-05-02) cannot be reproduced from the artifacts that exist.

This tracker is the canonical log going forward. Every run is anchored to:

- A **pinned worktree** at `.worktrees/bench` (detached HEAD at `baseline_sha`)
- A **dedicated qdrant collection** (`bench_<model>_code_chunks`) so models coexist
- A **config block** in the JSON output (model, boost, sparse on/off, project_sha)
- The **25-TC suite** (20 legacy tiers + 5 T5 real-usage-shape from external `usage.db`)
- The **host** — machine, GPU, VRAM, and container set. Added 2026-08-16, after the table
  turned out to already span at least three machines without ever recording which. See the
  2026-08-16 history entry.

If the table above ever needs a new `baseline_sha`, treat all prior rows as compromised
and start a new section here.


## Machines

Runs in this log were produced on two different machines. Record the **label**, never "this
machine" — a tracker that says "this machine" is false the moment it is read on the other one.

| label | hardware | accelerators | how the stack is served |
|---|---|---|---|
| `desktop-threadripper` | AMD Ryzen Threadripper PRO 3975WX, 32 cores | **RTX A5000 24 GB** (CUDA) **and** **RX 7800 XT 16 GB** (ROCm) — both present | `-amd` containers on the Radeon (dense/rerank/sparse at :48081/:48083/:48084); the A5000 has also hosted `llama-server` directly (e.g. `:43302` for the nomic run) |
| `laptop` | — | single **6 GiB** card | the `-gpu` container names used in § Prerequisites; TEI at `--dtype float16` could not warm up here, which forced the GGUF/llama-server reranker swap |

Two consequences worth internalising before comparing any two rows:

- **A dual-GPU host is not one environment.** `desktop-threadripper` can serve the same model
  from CUDA/A5000 or from ROCm/Radeon, with different latency and different VRAM ceilings.
  Recording only the machine is not enough — record which accelerator served the run.
- **The 6 GiB ceiling changed scores, not just latency.** It is why `.env.amd` carried
  `CODESCOUT_DISABLE_SPARSE=1`, which config layer 2 then fed to every profile, making the
  2026-07-28 reranker A/B dense-only — the axis its author called *"the one I was least aware
  of while measuring"*.
## How to run a bench

### Prerequisites

```bash
# 1. Retrieval stack containers must be up
docker ps --format '{{.Names}}' | grep -E "qdrant|embedder|reranker"
# Expect: codescout-qdrant (:6334), codescout-embedder-gpu (:48081),
#         codescout-embedder-sparse-gpu (:48084), codescout-reranker-gpu (:48083)

# NOTE (2026-08-16): the container NAMES above are host-specific. They are the
# `-gpu` set from an NVIDIA host. On the Threadripper/RX 7800 XT desktop the same
# ports are served by codescout-dense-amd (:48081, CodeRankEmbed-Q4_K_M),
# codescout-reranker-amd (:48083, bge-reranker-v2-m3-Q4_K_M) and
# codescout-sparse-amd (:48084, Splade_PP_en_v1) — all llama.cpp/TEI in Docker,
# so no separate llama-server on :43300 is needed there. Check what you have
# before following the Run sections verbatim.

# 2. Pinned bench worktree must exist at the baseline commit
git worktree list | grep .worktrees/bench
# It will be missing on a fresh host — it is NOT in git and was deleted 2026-08-16.
# Recreate it: git worktree add --detach .worktrees/bench <baseline_sha>

# 3. Build release binary
cargo build --release
```

### Run 1 — jina-v2 (fusion at default boost=3.0)

```bash
# Sync once (per (model, collection_prefix) combination)
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_jinav2_ \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench codescout

# Then bench (no re-sync needed for boost sweeps on same model)
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=jina-embeddings-v2-base-code \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_jinav2_ \
  --label "jina-v2-bm25-3.0" \
  > /tmp/bench-jinav2.json
```

### Run 2 — CodeRankEmbed via llama-server

```bash
# Start llama-server (foreground for sanity, then move to background)
nohup llama-server -m ~/models/CodeRankEmbed-Q4_K_M.gguf \
  --embeddings --port 43300 --host 127.0.0.1 \
  -ngl 99 -c 16384 -b 4096 -ub 4096 --parallel 1 \
  > /tmp/llama-coderank.log 2>&1 &

# Wait for ready
until curl -s http://127.0.0.1:43300/v1/models >/dev/null; do sleep 1; done

# Sync
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_coderank_ \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench codescout

# Bench
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=CodeRankEmbed-Q4_K_M \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_coderank_ \
  --label "coderank-bm25-3.0" \
  > /tmp/bench-coderank.json
```

### Run variants

- **Dense-only control:** add `CODESCOUT_DISABLE_SPARSE=1` to the bench command (sync is shared).
- **Boost sweep:** vary `CODESCOUT_BM25_BOOST=<0.5|1.0|2.0|5.0>` on the bench command;
  no re-sync needed (boost is query-time only).
- **New model:** pick a unique `CODESCOUT_QDRANT_COLLECTION_PREFIX` (e.g. `bench_new_`),
  sync into it, then bench. The live `code_chunks` collection is never touched.

### Scoring

Score per TC: 3 if all expected paths in top-5, 2 if all in top-10 or majority in top-5,
1 if at least one in top-10, 0 otherwise. Path match: `r == exp` or
`r.endswith("/" + exp)` or `exp.endswith("/" + r)`. **Caveat:** basename collisions
(`embedder.rs` in two crates) defeat the matcher when the expected list is workspace-
relative — known gap.

## Findings so far (2026-05-12, baseline `ede25e69`)

- **Fusion helps by +2** on both jina-v2 and CodeRankEmbed (sparse on vs off at boost=3.0).
- **Boost sweep on CodeRankEmbed-Q4 (no prefix):** 0.5→32, 1.0→33, 2.0→34, 3.0→34,
  **5.0→35** (peak), 7.0→34, 10.0→33, 15.0→33, 20.0→33.
- **Query prefix × quantization:** prefix +3 on f16, prefix −4 on Q4_K_M. Q4 collapses
  the asymmetric subspace. Authoritative spec confirmed locally in
  `~/models/CodeRankEmbed-hf/config_sentence_transformers.json` (`prompts.query` only,
  no doc prefix). Researcher unneeded.
- **f16+prefix peaks at 34/75** (boost ∈ {2,3,5,7}), one point below Q4 no-prefix.
- **CodeRankEmbed wins on env-var / identifier-bag queries** (TC-02, TC-11, TC-12)
  where code-specific training surfaces identifier semantics jina misses.
- **T5 reached 7/15 on both models after fixing 4 wrong-expected truth lists** (originally
  cited the wrong file paths for ToolContext, EmbedderHttp, artifact-augment, MockLspClient).
- **TC-24 still 0/3 across all configs.** Top-10 is dominated by `.md` plans/specs/trackers
  that mention the augment feature in natural language; the actual `tools/augment.rs` and
  `catalog/augmentation.rs` are never surfaced. **Real retrieval failure mode**: code is
  losing to descriptive prose. Next levers: md-vs-code score balancing, or a `kind:code`
  filter on `semantic_search`.
- **TC-24 went 0/3 → 3/3 in code-mode** — augment.rs and augmentation.rs surface
  to ranks #1 and #3 when .md plans/specs are filtered out.
- **Champion (2026-05-12): CodeRankEmbed Q4_K_M, no prefix, bm25=5.0, mode=code → 37/75**
  (49.3% — total matches full-mode but T5 jumps from 7/15 to 10/15, signaling
  better real-user query handling).
## Caveats and known gaps

- **No CodeRankEmbed query prefix.** Historical 41/60 hybrid run used the
  `Represent this query for searching relevant code:` prefix. The retrieval stack's
  `EmbedderHttp` does not add a query-side prefix. Adding `CODESCOUT_QUERY_PREFIX`
  support is the single highest-leverage win for CodeRankEmbed.
- **Scoring matcher needs basename flexibility** for T5. Either rewrite TC-23/24/25
  expected lists to use unambiguous paths, or extend the matcher with crate-aware
  matching.
- **No latency baseline for dense-only p95** in this row set — the harness emitted
  `p95=0` when bench length was too short for the 5% tail. Re-run with `--limit 20` if
  comparing tails matters.

## History

### 2026-09-02 — artifact-path baseline

First instrument for `artifact(find, semantic=)`. The 25-TC suite scores `bench_<model>_code_chunks` and never touched this path. Baseline on first-chunk-only: **hits@5 0/12, MRR 0.0** — no result carries a line range, so no case can score. `search_live: true` (positive control — at least one query returned non-empty `items`, so the 0/12 reflects the missing line-range field, not a dead search path). Suite: `scripts/tc-suites/artifact-entries.json`.

### 2026-09-04 (late) — the harness now says WHY a case missed, and one case could never have passed

**The score was understating retrieval by a full point, for a reason that is not
retrieval.** `AE-9` expected `IC-16` in `docs/trackers/issue-clusters.md`. That
ledger has since been split into one file per cluster, so `IC-16` is defined at
`docs/trackers/issue-clusters/IC-16-assertion-that-cannot-fail.md` and the old path
defines **zero** `IC-N` headings. The case could not have scored under any
retrieval quality whatever — and it did not read as broken, because a stale
ground truth and a genuine miss produce the byte-identical `rank: null`. It has
been charged to retrieval in every number recorded since the split.

Neither existing guard could see it. `search_live` covers a dead search path;
the returncode check covers a dead binary. Both were healthy. **The ground truth
was the unchecked input** — the harness validated its tool and its transport and
never its own question. Second instance in this tracker: `2026-05-12 — T5
expected-path fix` found 4 of 5 TCs with wrong truth on the *other* suite, and
the lesson did not travel to this one.

**Instrument changes** (`scripts/run-artifact-bench.py`), all additive to the
recorded JSON schema, so older consumers reading `rank` / `hits_at_5` still work:

- `defines_entry()` pre-flight per case → `unscorable` class, named loudly on
  stderr with the resulting cap on `hits@5`, and counted on **stdout** beside the
  score, because the score is what gets copied into a tracker and a stderr warning
  is not.
- every result recorded (`top`: rank, path, line, resolved entry) rather than only
  the matching one.
- `file_hits_at_5` alongside the entry-level `hits_at_5`. The file-level figure
  was previously derived by hand and quoted next to the entry-level one as though
  they were the same metric.
- five miss classes: `hit` / `preamble` / `wrong_entry` / `wrong_file` /
  `unscorable`.

**Measured, `target/release/codescout` at `28de2827`, chunk grain (default):**

| | hits@5 | file-hits@5 | MRR | classes |
|---|---|---|---|---|
| before repoint | 3/12 | 6/12 | 0.1875 | hit=3 unscorable=1 preamble=3 wrong_file=5 |
| after repoint | **4/12** | **7/12** | 0.2708 | hit=4 preamble=3 wrong_file=5 |

The `+1` is attributable rather than noise: `unscorable` is a deterministic file
check, not a retrieval outcome, and the other eleven cases kept their classes
across both runs.

**The diagnosis the classes buy, which nine `rank: null`s could not.** Of the
eight misses at `limit=5`: **three are the preamble attractor, five are genuine
retrieval loss, zero are ordinary intra-file ranking.** All three near-misses
return the right file — twice at rank 1 — with a chunk whose line maps to **no
entry token at all**, i.e. the file's index/preamble, a section that summarises
every entry and so out-scores each specific one on any query about that file.
With `max_per_artifact=1` it then evicts the real answer. That is `BL-72`'s
territory, now with a count attached instead of an impression.

Worth recording that the harness corrected its own author here: the first run
labelled those three `wrong_entry`, and only the new `entry` field showed every
one of them resolving to `None`. "Right file, wrong place in it" is two findings
with opposite fixes.

**Caveat with teeth — the corpus is live and shared.** Two runs twenty minutes
apart returned 2/12 and 3/12; `artifact_chunk` went 35,508 → 35,533 rows and
1,850 → 1,851 artifacts *within a single turn* as peer sessions committed and
reindexed. Three back-to-back runs agreed exactly (4/12, 7/12, 0.2708, identical
classes), so the movement is corpus drift rather than jitter — but a delta of ±1
across runs minutes apart on this checkout is not evidence of anything. Pin the
comparison to one run window, or record the chunk-row count beside the score.

### 2026-09-04 — the two grains measured head to head; the default flips to ON

**This supersedes the ruling below it.** That entry recorded chunk grain as a
20× cost for better ranking, and therefore the project's call. Measuring the two
grains against each other refuted the premise: it is not better-versus-worse, it
is working-versus-not.

Method: build a real per-file Qdrant collection (`bench_artifact_grain_codescout`,
1,475 points, one vector per artifact over the frontmatter-stripped body), then
run the same 12 suite queries against both collections with the same embedder and
the same query prefix. Scored at **file level** — did the right document come
back — because that is the only metric fair to both grains; the suite's own
entry-level metric is unscorable at artifact grain by construction.

| | chunk grain | artifact grain |
|---|---|---|
| file-level hits@5 | **6/12** (MRR 0.396) | **0/12** (MRR 0.000) |
| entry-level hits@5 | 3/12 | 0/12 — structurally impossible |
| vectors | 29,138 | 1,475 (19.8× cheaper) |
| artifacts the embedder REFUSED | 0 | **473 of 1,475 (32%)** |

**The 0/12 passed a positive control, which is the only reason it is quoted.** A
zero is the shape a broken collection also produces. Querying the artifact-grain
collection with a document's own *title* ranks that document #1 every time, with
clear margin: 0.5743 vs 0.5072, 0.5087 vs 0.3346, 0.4248 vs 0.3891. The
collection retrieves correctly; the grain cannot answer the question.

**Mechanism.** A per-artifact vector represents the document's opening ~2,048
characters. For a ledger that is frontmatter, an index table and conventions
boilerplate. `W-81` sits at line 7,956 of a 10,752-line file and has *no*
representation in that vector. Artifact grain answers *"which document is this?"*
and cannot answer *"which document says this?"* — and every real query is the
second kind.

**The 32% is the finding that made the decision one-sided rather than a
trade.** The embedder rejects oversized input with HTTP 500 instead of
truncating, and nothing in the librarian embed path clips first, so artifact
grain leaves a third of this corpus permanently vectorless in the absorbing state
`2026-09-02-indexer-stamps-content-seen-before-it-embeds` describes. Filed as
`618fcd89dd2c5e24`. It was visible three times before it was seen —
`embed_error_count` read 2, 1 and 1 across the preceding day and was dismissed as
incidental each time, because at chunk grain only a handful of chunks exceed the
limit.

**Ruling:** chunk grain becomes the default; `[librarian] chunk_grain = false`
opts out. The silence around a mistyped key now fails SAFE — a typo costs
embedding time and leaves search working, where under the previous default the
same typo cost a third of the corpus its vectors.

**Also measured, and not yet acted on:** the page policy is worth more than the
grain debate. Simulated over raw kNN, with `cap=1 limit=5` reproducing the
shipped 3/12 exactly as a positive control:

| policy | entry-level hits@5 |
|---|---|
| cap=1 limit=5 (shipped) | 3/12 |
| cap=1 limit=10 · cap=2 limit=5 | 5/12 |
| **cap=2 limit=10** | **7/12** |
| cap=3 limit=10 | 6/12 |
| cap=∞ limit=10 | 5/12 |

`cap=2` is a genuine optimum — both directions are worse, because uncapping lets
one artifact flood the page. Not changed here: `find.rs:800-805` carries a
standing warning that both side maps are keyed by artifact id and whoever raises
the cap owns restructuring them.

**What the residual 5 misses are, at the best policy:** 1 unscorable (`AE-9`
wants `IC-16`, which has no `## IC-16 —` heading — a suite defect), 2
mis-specified ground truth (`AE-11`/`AE-12` point at 256- and 372-byte evidence
stubs rather than the prose that answers the query; for `AE-11` the correct FILE
already ranks 1), and 2 genuine ranking losses (`AE-4`, `AE-7`). Re-point or
retire those three before quoting any number from this suite again.

### 2026-09-03 — the ruling: chunk grain ships OFF by default, opt in per project

**Shipped** `4f172f70` (patch-id `991386342baded3dccbc6f59b7b578fb114851db`), on `experiments`, gate green including the `--features server-stack` lane.

`[librarian] chunk_grain = true` in `<project>/.codescout/project.toml` opts in.
Default is one vector per artifact. **codescout itself opts IN** — 12 minutes is
acceptable on this machine, and this is where the feature is measured.

**Why the decision is the project's and not ours — the cost, measured 2026-09-03:**

| | |
|---|---|
| artifacts | 1,457 |
| chunks | 28,612 |
| mean chunks/artifact | 19.6 |
| median | 16 |
| max | 565 |
| top 6 artifacts' share of cost | **6%** |
| full re-embed | 27,762 vectors, ~12m10s, ~38/sec, 2 failures |

**The distribution is BROAD, not skewed, and that is the load-bearing half.** An
earlier note in this tracker called it skewed, generalising from one 564-chunk
file; the median is 16 and the top six artifacts are 6% of the total. A skewed
distribution would have admitted a targeting rule — chunk the big trackers, leave
the rest — and a broad one does not. There is no cheap subset, so the only
available lever is per-project on/off, which is what shipped. Carry the
retraction, not the original claim.

**Artifact grain is not a neutral cheap mode.** It is the ranking behaviour of
`docs/issues/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md`:
one vector on a 512-token embedder represents a document's first ~2,048
characters and no more. Two things differ from that defect and **neither recovers
the ranking** — the whole body is stored as one `artifact_chunk` row, so `matched`
reports the document's real span instead of a wrong one, and a larger-context
embedder would improve it for free. It is a hardware concession. Anyone reading
this to decide a default for their own project should read it as "off means you
keep the 2026-09-02 defect's ranking, on purpose, to save 20x the vectors".

**What this entry does NOT claim.** That 3/12 (the run below) justifies the 20x.
Twelve hand-written queries against prose entries is the first honest read of
whether chunk grain earns its cost, and 25% is not encouraging — but it bounds
nothing tightly, and the switch shipped so that the trade can be made per machine
rather than resolved by that number. A larger suite is owed before anyone quotes
3/12 as a verdict on the feature.

**One silent failure mode, recorded because nothing reports it.**
`.codescout/project.toml` is gitignored, so the opt-in is **machine-local** — the
same is already true of its sibling `[librarian] vector_backend`. A clone on a
second machine reindexes at artifact grain until someone sets the key there too,
and both grains produce a populated index and a plausible ranking. The difference
shows up only as worse retrieval, which reads like a model problem.

### 2026-09-03 (late) — the first genuine RANKING measurement: 3/12

**hits@5 3/12, MRR 0.1875, `search_live=true`**, on a fully populated chunk-grain
index. Every previous number from this suite was coverage wearing a ranking's
clothes; this one is not.

| run | hits@5 | MRR | what the number was actually about |
|---|---|---|---|
| baseline (Task 1) | 0/12 | 0.0 | — |
| after Tasks 2–11 | 0/12 | 0.0 | correct artifact ranked #1, span published wrong |
| after `36afd405` | 2/12 | 0.1667 | **coverage**: 2 of 2 targets that had chunk rows |
| after `f74f25ec` | 1/12 | 0.0833 | **coverage**: editing an artifact removed it from the index |
| **after `6f032dbd` + full re-embed** | **3/12** | **0.1875** | **ranking** — 12 of 12 targets indexed |

**The denominator is real this time, and it was checked rather than assumed.**
Every one of the twelve suite targets holds chunk rows (565, 97, 76, 39, 451, 35,
95, 150, 38, 175, 19, 23 — zero targets at zero). So no case can fail for want of
being in the index, and 3/12 measures what the ranker does.

**A correction to this tracker's own previous entry, which flattered the system.**
The 2026-09-03 earlier entry reported `2/12` as "2 of 2 reachable" and read that
as retrieval being perfect on what it could see. True as stated, and misleading:
AE-10 ranked **1** when only two artifacts were indexed and ranks **nothing** now
that it competes against a full corpus. A near-empty index makes ranking trivial,
so a hit rate computed over a tiny reachable set is not evidence about a full one.
The generalisable form: **a ratio whose denominator is small because the system is
broken cannot be extrapolated to the fixed system** — the same population that
makes the ratio flattering is the one the fix removes.

**Per-case ranks:** AE-1 **1**, AE-2 **4**, AE-6 **1**; AE-3/4/5/7/8/9/10/11/12
absent from the top 5. AE-10 is the regression described above.

**Config.** Host `ripper`, `experiments` @ `6f032dbd` (patch-id
`2605fac14725020fcd4fcb66e5a22d6d21d85f9a`), `target/release/codescout`
(`server-stack` → Qdrant), collection
`artifact_chunks_codescout_dc6a871595179329` — per-project, chunk-keyed, both
`chunk_id` and `artifact_id` in the payload. Embeddings `CodeRankEmbed` @
127.0.0.1:48081. Suite `scripts/tc-suites/artifact-entries.json`, harness
`scripts/run-artifact-bench.py --bin target/release/codescout`.

**The re-embed that produced it.** `librarian(reindex, reembed=true)`: **27,762
vectors written, 2 failures** (embedder HTTP 500, oversized input), down from 169
refusals before the fix. ~12m10s wall clock at ~38 vectors/sec, holding the
project write lock throughout — which blocked every other session in the
checkout and is being filed separately.

**Cost, measured, because it is now a product decision.** 1,457 artifacts →
**28,612 chunks**: mean 19.6, median 16, max 565. The cost is **broad, not
skewed** — the top six artifacts are only 6% of it, and capping at 32 chunks per
artifact still keeps 87%. It is not concentrated by kind either (bug 38.7%, plan
21.9%, tracker 18.2%, spec 10.1%), so "chunk only the ledgers" saves just 41%.
There is no targeting rule that makes this cheap; chunk-grain is ~20x
artifact-grain and the only real lever is on/off. Ruling 2026-09-03: **default
off, opt in per project**; codescout itself opts in.

**What 3/12 does and does not settle.** It is the first honest read of whether
chunk-grain earns its cost, and 25% is not a strong result — but the suite is
twelve hand-written queries against prose entries, so it bounds nothing tightly.
Do not read it as a verdict on the feature; read it as the first number that was
ever *about* the feature.

### 2026-09-03 — after the coordinate fix: 2/12, and the denominator is the finding

**hits@5 2/12, MRR 0.1667, `search_live=true`.** The first non-zero score this
instrument has produced. Up from 0/12 / MRR 0.0 on both prior runs.

| run | hits@5 | MRR | what changed |
|---|---|---|---|
| baseline (Task 1) | 0/12 | 0.0 | — |
| after Tasks 2–11 | 0/12 | 0.0 | chunk-grain shipped; ranks correct artifact #1, publishes the wrong span |
| **after `36afd405`** | **2/12** | **0.1667** | chunk ranges are file lines; 47 artifacts' stored ranges migrated |

**Config.** Host `ripper`, `experiments` @ `36afd405` (patch-id
`6ba7ae81ba07d8fde8870fc6162c6330093159b8`), `target/release/codescout`
(`server-stack` → Qdrant, `artifacts` collection, 5386 points — artifact-grain
by contract; the Task 7 guard now refuses chunk ids outright),
embeddings `CodeRankEmbed` @ 127.0.0.1:48081. Suite
`scripts/tc-suites/artifact-entries.json`, harness
`scripts/run-artifact-bench.py`.

**Read the denominator before the numerator — 2/12 is 2 of 2 reachable.**
The suite's twelve targets were checked against `artifact_chunk` after the run:

| target | chunk rows | rank |
|---|---|---|
| `docs/trackers/bug-fix-session-log.md` (AE-1) | 559 | **1** |
| `docs/trackers/prompt-surface-measurement-session-log.md` (AE-10) | 175 | **1** |
| the other ten (AE-2…9, 11, 12) | **0** | none |

The two targets carrying chunk rows are exactly the two that scored, both at
rank 1. The other ten hold **zero** chunk rows, so they are absent from the
semantic index entirely and no ranking change could reach them. Retrieval is
2-for-2 on the population it can see; the residual is **index coverage**, not
ranking — 55 of 1430 codescout artifacts (3.8%) have chunk rows at all. A
reader taking `2/12` at face value would go tune the ranker.

**Two instrument facts, both still true.** The harness must be run with
`--bin target/release/codescout`: the default `target/debug` is a lean build
whose `ArtifactBackend::resolve` returns `SqliteVec`, reads an empty
`artifact_vec_v2`, and returns `count: 0, hints: {}` at exit 0 — a clean zero
that looks like a retrieval verdict. And `docs/trackers/retrieval-benchmark.md`
is in `.codescout/project.toml`'s `ignored_paths`, so this tracker cannot
contaminate its own suite.

**What blocks the remaining ten — and it is NOT a backfill problem.** This
deployment resolves to the **Qdrant** backend, and chunk-grain retrieval is
implemented on **sqlite-vec only**. A `librarian(action="reindex")` run
2026-09-03 returned `embedded: 0, embed_error_count: 59`, every one of them:

> `QdrantArtifactStore is artifact-grain and was handed a non-artifact id
> "1fda5a94-…". Chunk-grain retrieval is implemented on the sqlite-vec backend
> only; set the artifact backend to sqlite-vec, or implement chunk-grain
> Qdrant.`

That is **Task 7's guard working as specified**, not a defect. The plan's
§ *Deferred* already names this and says in terms: *"before shipping, establish
which backend this deployment actually uses — and if it is Qdrant, this plan
does not apply to it yet."* It is Qdrant. So the chunk-grain feature is inert
here by design, the 55 artifacts holding `artifact_chunk` rows hold them
because `embed_queue_items` writes chunk ROWS as a side effect of queueing
even when the vector upsert is then refused, and the benchmark's live hits are
artifact-grain Qdrant hits hydrated against those rows. Which is exactly why
only the two suite targets carrying chunk rows can produce a `matched` block
at all.

**A false lead, recorded so it is not re-walked.** `codescout backfill-chunks`
was briefly suspected of writing to a store the backend never reads, and a bug
file was drafted. It is wrong: `SqliteVecArtifactStore::upsert`
(`src/librarian/artifact_store.rs:242`) calls the same `write_embeddings_v2`
the backfill calls directly, so on the one backend where chunk-grain works the
two paths have an identical target. The draft was withdrawn unpushed. The
lesson is the ordinary one: the claim came from reading three call sites, and
running the tool once refuted it.

**So the real fork is a deployment decision, not a fix**: either point this
project at sqlite-vec (`[librarian] vector_backend = "sqlite-vec"` in
`.codescout/project.toml`, or `CODESCOUT_ARTIFACT_BACKEND`), or implement
Qdrant chunk-grain parity — the plan's open question 4, recorded as undecided.
**Do not read a later run's delta as a ranking change** until that is settled;
it will be coverage arriving.

**Migration note, for anyone comparing against an older run.** The 47
artifacts whose stored ranges were shifted from body- to file-relative kept
their `chunk_id`s and vectors: `content_hash` is over `content` alone, so the
coordinate change preserves every hash. Nothing was re-embedded, so this run's
delta is attributable to the coordinate fix and to nothing else. Three
artifacts were refused by the migration's round-trip check because their chunk
rows are stale against the current file (17/41, 2/20 and 3/10 chunks
reproducing) — a separate defect that only a re-chunk cures.

### 2026-09-02 — artifact-path after chunk-grain (Tasks 2–11): **still 0/12, for a different reason**

`hits@5 **0/12**, MRR **0.0**, `search_live: true`` — numerically identical to the baseline
above, and **not** the same result. The baseline scored 0 because no hit carried a line
range at all. This run scores 0 because the range it now carries is in the **wrong
coordinate space**.

| | baseline | after |
|---|---|---|
| hits@5 | 0/12 | 0/12 |
| MRR | 0.0 | 0.0 |
| `search_live` | true | true |
| cause | no `start_line` on any hit | `matched.start_line` is body-relative, published as a file line |

**Config, because a run without it is not comparable to anything.** Host `ripper`, model
`CodeRankEmbed` (`LIBRARIAN_EMBED_MODEL`), backend **Qdrant** (the `server-stack` release
default; `CODESCOUT_ARTIFACT_BACKEND` unset), tree `experiments` at `488192e8`, suite
`scripts/tc-suites/artifact-entries.json`, result
`~/.claude/jobs/ffb95976/tmp/artifact-bench-after.json`.

**Two instrument facts this run established, both of which changed the number's meaning:**

1. **`--bin` must be the RELEASE binary.** The harness defaults to `target/debug/codescout`,
   which is a lean build: no `server-stack`, so `ArtifactBackend::resolve` returns
   sqlite-vec, whose `artifact_vec_v2` is empty on this machine. The debug binary returns
   `count: 0` with `hints: {}` and exit 0 — no error, no hint, indistinguishable from "the
   corpus does not cover this query". Every future run of this suite must pass
   `--bin target/release/codescout` or it measures the wrong process.
2. **The scorer read the wrong field, and its positive control could not tell.** It looked
   for a top-level `start_line`; Task 10 ships the range under `matched`. `search_live`
   proves the search path is alive, not that the scored FIELD exists — so a 0/12 from a
   field-name mismatch is indistinguishable from a retrieval finding. Fixed to accept both
   shapes, and the fix did **not** move the number, which is what makes the remaining 0/12
   a real result rather than a repaired one.

**What the 0/12 actually says.** Retrieval is working: `AE-1` ranks the correct artifact
**#1**. Its reported span is `7793`, the expected entry `W-81` begins at file line `7808`,
and the artifact's frontmatter closes at line `15` — the offset is exactly the frontmatter
length, so the published range lands inside the *previous* entry. Filed as
`docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`.
The suite cannot score above 0 until that is fixed, and it should be re-run immediately
afterwards — this is the first run where a non-zero number is even reachable.
### 2026-08-30 — D2 resolved: prefix removed, boost 3.0→5.0 — and the precedence list below was missing a layer

**Changed, in `~/.claude/settings.json` § `env`:**

```diff
-  "CODESCOUT_QUERY_PREFIX": "Represent this query for searching relevant code: ",
-  "CODESCOUT_BM25_BOOST": "3.0",
+  "CODESCOUT_BM25_BOOST": "5.0",
```

Backed up to `settings.json.bak-20260830-173902`; JSON re-validated (21 top-level
keys, `env` 13→12); `diff` shows exactly these two lines.

**Why.** Both deltas are measured, not argued. Prefix is **−4 on Q4_K_M** (2026-05-12)
and was re-measured as **37 no-prefix vs 32 with** (2026-07-28) — same direction
twice — against a live-verified `CodeRankEmbed-Q4_K_M.gguf` at `:48081`. Boost 5.0 is
the sweep peak (3.0→34, 5.0→35). Prefix is query-side only, so **no re-index**.

#### The precedence list in the 2026-07-28 entry is incomplete — and that is why its fix did not stick

That entry names three layers: the MCP `env` block in `<profile>/.claude.json`, then
`$CODESCOUT_ENV_FILE`/`<global_config_dir>/.env`, then hardcoded defaults. There is a
fourth, and it outranks all of them for a Claude Code–spawned server:

> **`<profile>/settings.json` § `env`** — injected by Claude Code into every MCP
> server it spawns. Not the shell's environment: the parent `claude` process
> (pid 801487) carries **no** `CODESCOUT_QUERY_PREFIX`, while its child server
> (3881801) does.

The 2026-07-28 fix removed the prefix from `.claude-sdd` and `.claude-kat`'s
**`.claude.json`**, and recorded `~/.claude` as clean — correctly, for the file it
checked. The setting was in `settings.json` the whole time. So the entry's
"**real — now fixed**" verdict was true for two profiles and false for the third, for
one month.

**Measured consequence:** six live `codescout` servers, **three with the prefix and
three without**, all querying one shared Qdrant index. The split is exactly the
profile boundary — `~/.claude` sessions had it, the other two did not.

#### Still inconsistent after this change, deliberately

| source | `BM25_BOOST` | `QUERY_PREFIX` |
|---|---|---|
| `~/.claude/settings.json` | **5.0** (changed) | **absent** (removed) |
| `~/.claude-sdd/.claude.json` | 3.0 | absent |
| `~/.claude-kat/.claude.json` | 3.0 | absent |
| `.env.gpu` (layer 2, symlinked) | 3.0 | commented out |

Only this profile was moved. `CLAUDE.md`'s three-instance rule argues for aligning
the other two, and that is a live decision, not an oversight — left to the operator
because `.env.gpu:124-125` explicitly cautions *"3.0 the long-standing default.
Re-sweep with `scripts/sweep-bm25-boost.sh` before trusting either."*

#### What this entry does NOT claim

No local re-measurement was taken. Both deltas are inherited from 2026-05-12 /
2026-07-28 runs, and this tracker's own history is a catalogue of numbers that did
not transfer across machines, corpora and reranker hosts. The current reranker is
`llama-server` (~13× the TEI p50 the 37/75 champion used), so **do not expect 37/75
and do not cite one if it appears.** The claim here is narrow: two settings now match
the best-measured configuration on record, and the drift that hid one of them for a
month is named.

#### Verified after the reconnect — and only half of it took

The check above was run rather than skipped, and it is the reason this section
exists. In the server spawned 11 s after `/mcp`:

| | on disk | in the new server |
|---|---|---|
| `CODESCOUT_BM25_BOOST` | `5.0` | **`5.0`** — applied |
| `CODESCOUT_QUERY_PREFIX` | **absent** | **still set** — not applied |

One edit, one reconnect, opposite outcomes: **an updated value lands, a deleted key
does not.** Filed as
`docs/issues/2026-08-30-mcp-reconnect-applies-env-updates-but-not-env-deletions.md`.

**The boost is the control that makes this a finding rather than a stale cache.**
`5.0` exists in no other layer — both sibling profiles say `3.0`, `.env.gpu` says
`3.0`, and the parent `claude` process carries no such variable at all. It could only
have come from the file edited moments earlier, so the reconnect **did** re-read
`settings.json` and still produced the old key.

**Why this nearly went unnoticed: the half that worked confirms the half that did
not.** The natural check after a config edit is to look at what you changed. Any edit
containing an update passes that check, and a reader concludes the reconnect applied
the whole edit. Only separately probing a key you *deleted* distinguishes the two, and
that is not an obvious thing to do.

**Applied workaround — `CODESCOUT_QUERY_PREFIX: ""` rather than absent.** An empty
value survives the merge because it is an update, and it is *exactly* equivalent to
unset here, verified in the consuming code rather than assumed:
`EmbedderHttp::new` reads the var with `unwrap_or_default()`, and `remote_dense`
maps `query_prefix.is_empty()` → `QueryPrefix::Suppressed` — the same state an
absent var produces. (That method's own doc comment independently cites this
tracker's numbers: *"37 without the prefix and 34 with it"*.)

So the settled state on disk is `BM25_BOOST=5.0` and `QUERY_PREFIX=""`. **Takes
effect on the next MCP restart**; a full Claude Code restart would also allow the
empty entry to be replaced by a clean deletion, which is cosmetic — the two are
behaviourally identical.

### 2026-08-16 (later) — first run attempt on `desktop-threadripper`: corpus indexed, harness scores 0/75, NOT root-caused

**No row was appended to `params.runs`.** There is no valid score to log; a 0/75 from a
wiring fault is not a measurement, and writing it into the table would poison exactly the
comparability this tracker exists to protect.

**Setup actually used** (differs from § Prerequisites, which is `laptop`-shaped):

| | value |
|---|---|
| host / accelerator | `desktop-threadripper`, stack on the **RX 7800 XT**; A5000 idle |
| dense | `codescout-dense-amd` :48081 — CodeRankEmbed-Q4_K_M via llama.cpp (**no separate `llama-server` on :43300 needed here**) |
| sparse | `codescout-sparse-amd` :48084 — Splade_PP_en_v1 |
| rerank | `codescout-reranker-amd` :48083 — bge-reranker-v2-m3-Q4_K_M |
| corpus | `.worktrees/bench` recreated pristine at `ede25e69`, 851/851 files |
| collection | `bench_coderank_code_chunks`, dim 768 + sparse w/ IDF — byte-identical config to production `code_chunks` |
| project id | `bench` (basename default; no `project.toml` on either side) |

**Indexing succeeded and was fast:** `+19216 -0 ~0 chunks in 323862 ms` — 5.4 min, against
1239 s for a comparable index on `laptop`. Roughly 4×, which is the clearest
host-difference datapoint the log now has.

**But the corpus is 11% smaller than the chunk count says.** Qdrant holds **17,108**
points against 19,216 chunks reported added. `chunk_id` is `{project}:{path}:{content_hash}`
with no ordinal (`src/retrieval/sync.rs:77`) and the point id derives from it, so two
identical chunks in one file are the same point and the second overwrites the first. Both
ends report success — the writer says `+19216`, and a no-op re-sync says `+0 -0 ~0`. Filed
as `docs/issues/archive/2026-08-16-chunk-id-omits-index-so-duplicate-chunks-collapse.md` (fixed). The 64-bit
truncation in `chunk_id_to_point_id` (`src/retrieval/qdrant.rs:28`) is NOT the cause — at
19k items its expected collision count is ~1e-11 — and ruling that out is what located the
real one. **Any future score on this collection has an unknown recall ceiling below 100%.**

**The blocker: every TC returns `top10_files: []`.** Zero hits, no exception, no `[WARN]`
line — `semantic_search` returns successfully and empty. Ruled out by measurement:

| candidate | how it was excluded |
|---|---|
| `mode` | `code` and `full` both 0/75 |
| reranking | on and off both 0/75 |
| project-id mismatch (the 2026-05-12 failure) | payloads carry `project_id: "bench"`; activation resolves `bench`; no `project.toml` on either side |
| collection shape | dense 768 + sparse/IDF, identical to production |
| response parsing | `semantic_search` returns clean JSON with a `results` array, and the same query against the production project returns hits |

**Lead for the next session — sub-project topology.** Activating the bench corpus resolves
**8** workspace projects (`bench`, `codescout-embed`, `librarian-mcp`, and 5
`tests/fixtures/*` libraries); today's tree resolves **2**. `sync_project` wrote every chunk
under the single id `bench`. If the query path scopes by sub-project, everything under
`crates/**` and `tests/fixtures/**` is unreachable under that id — and TC-01's expected
`src/tools/core/types.rs` is not, so this hypothesis does not explain the *whole* zero on
its own. Test it before building on it.

**Harness gaps that made this expensive, worth fixing before the next attempt:**

- `run-tc-benchmark.py` spawns the server with `stderr=subprocess.DEVNULL`, so every
  server-side error is discarded. Capturing it would very likely have answered this in one
  run instead of a dozen probes.
- A prerequisite assertion is missing: the harness should refuse to run when the collection
  is empty, when `points_count` disagrees with the index state, or when a first smoke query
  returns nothing — rather than dutifully scoring 25 zeros.

**Two config facts the old rows do not pin, now recorded:**

- **`CODESCOUT_RERANK` is opt-in and defaults OFF** (`src/retrieval/config.rs:4`,
  `src/retrieval/search.rs:10`). Reranking does not happen unless explicitly set, so any
  historical row that does not state the flag is ambiguous about whether it reranked.
- `.env.amd` (symlinked as the global `.env`) now has **both** `CODESCOUT_DISABLE_SPARSE`
  and `CODESCOUT_QUERY_PREFIX` commented out — the 2026-07-28 drift is fixed.

**Un-filed minor quirk:** `sync_project` has no argument parsing — `./target/release/sync_project --help`
treats `--help` as the project path and syncs a project named `--help`, creating a `./--help/`
directory. Harmless (removed), but it is a silent do-the-wrong-thing on the most natural
discovery command. Not filed as a bug; noted here.

### 2026-08-16 — the pinned table spans two machines, and never said so; bench worktree was a foreign-host leftover and is now deleted

Started as "let's run the benchmark" and ended without a run, because the preconditions did
not hold and one of them invalidates comparison itself.

**1. Host is a comparability axis this tracker never anchored.** The section above pins every
run to a worktree, a collection, and a config block. It does not pin the machine. Two are
involved, now named in § Machines:

| entry | hardware, as its own text records it | machine |
|---|---|---|
| 2026-05-12 nomic-embed-code | *"llama-server (CUDA, **RTX A5000 24GB**)"* | `desktop-threadripper`, on its A5000 |
| 2026-07-28 reranker swap | *"this **6 GiB** card"*; TEI at float16 could not warm up | `laptop` |
| 2026-08-16 (this entry) | Threadripper PRO 3975WX; **A5000 24 GB idle**, stack served from **RX 7800 XT 16 GB** via `-amd` containers | `desktop-threadripper` |

**Corrected 2026-08-16, same day.** The first version of this entry said "at least three
machines" and treated the A5000 as a third host. Wrong: the A5000 and the Radeon are **both in
the desktop** (`nvidia-smi` and `rocm-smi` each report one card, and `lspci` shows both on the
same bus). The error came from a GPU probe written as `rocm-smi || nvidia-smi` — ROCm answered,
so the NVIDIA branch never ran, and a single-GPU picture was reported from a check structurally
incapable of seeing the second card. Same failure shape as the retracted bug below: the
measurement was real, but it could not observe the thing the claim was about.

This is not cosmetic. Every p50/p95 in the table is a property of its host. Worse, VRAM
pressure changed **scores** too: the 6 GiB box is why `.env.amd` carried
`CODESCOUT_DISABLE_SPARSE=1`, which fed every profile through config layer 2 and made that
session's entire reranker A/B dense-only — the axis its author called *"the one I was least
aware of while measuring"*. `host` is now in the anchored list above.

**Consequence — revised after the correction above.** The champion row (**37/75**, CodeRank Q4
no-prefix boost=5.0, dense+sparse+rerank) *was* measured on `desktop-threadripper`, so it is far
closer to comparable than first stated. What still differs is the **accelerator and serving
path**: 2026-05-12 ran embedders as bare `llama-server` processes (the nomic arm explicitly on
CUDA/A5000), whereas today the stack is `-amd` containers on the Radeon. Latency is therefore
not comparable; scores plausibly are, but nothing pins which card served the 2026-05-12
CodeRank arm, so that remains an inference rather than a record.

**Therefore: a run here is logged as a new baseline section carrying an explicit
`host: desktop-threadripper` + accelerator field, and may be *compared* to the 37/75 row with
that caveat stated — not silently appended to the table as if it continued it.**

**2. `.worktrees/bench` was a foreign-host leftover — deleted.** Its `.git` file read
`gitdir: /home/marius/work/claude/code-explorer/.git/worktrees/bench`, a repo path that does
not exist on this machine, so the worktree was orphaned from git entirely: `git -C` failed and
`git worktree list` omitted it. `git worktree repair` cannot fix that (the referenced repo is
gone), so the admin dir was reconstructed by hand purely to inspect the contents before
deciding its fate. Findings:

- Created **2026-05-12** — the date of every pinned run.
- Corpus **complete and correct**: all **851** files of the `ede25e69` tree present, zero missing.
- Diverged from the baseline in exactly **two** files — `crates/codescout-embed/src/remote.rs`
  (adds `nomic-embed-code` to `query_prefix_for`) and `src/retrieval/embedder.rs` (an
  `CODESCOUT_EMBEDDER_QUERY_PREFIX` prototype).
- **Both are dead.** The nomic-embed-code entry below already records them as *"redundant
  patches … because main already had `CODESCOUT_QUERY_PREFIX` support"*, and that experiment's
  own verdict was **"drop nomic-embed-code-7B from consideration"**. Verified independently:
  main's `query_prefix_for` (`crates/codescout-embed/src/remote.rs:105`) matches `coderank`
  only, and main's `EmbedderHttp` reads `CODESCOUT_QUERY_PREFIX` (`src/retrieval/embedder.rs:292`)
  — a different, more complete implementation than the prototype's.

Nothing was worth merging, and the stale copy actively caused a false bug report earlier the
same day (see below), so it was removed with `git worktree remove --force`. 174 MB, of which
163 MB was regenerable `.codescout` index state. **Recreating it is one command** —
`git worktree add --detach .worktrees/bench ede25e694b63219e1382f359d7ba242f66a516a5` — and a
fresh checkout is strictly better than the diverged one.

**3. A false bug, and the reason it was false.** Earlier the same day a bug was filed claiming
`run-tc-benchmark.py`'s `expected` lists cite five deleted files, making several TCs unpassable.
Wrong: the harness scores against the **pinned corpus**, never against current HEAD, and all
five resolve at `ede25e69` (`git cat-file -e`, plus the 851-file walk above). The check had been
run against the working tree the session happened to be standing in. Retracted in
`docs/issues/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`, whose
Hypotheses-tried keeps the mistake on purpose. The rule it earns: **check the corpus the
instrument actually reads, not the one you are standing in.**

**4. Bench collections are gone.** Qdrant on this host holds only `memories`, `code_chunks`
(579,834 points, dense 768 + sparse w/ IDF), and `artifacts`. Both `bench_coderank_code_chunks`
and `bench_jinav2_code_chunks` are absent, so any run here starts with a full re-index of the
pinned corpus (~20-30 min at the rates recorded below).

**5. Config drift flagged on 2026-07-28 is already fixed.** `~/.config/codescout/.env` →
`.env.amd` now has **both** `CODESCOUT_DISABLE_SPARSE=1` and `CODESCOUT_QUERY_PREFIX` commented
out. Sparse is on; no query prefix. That matches what the log says it should be.

**What a run here still needs, for whoever picks this up.** Recreate the worktree; re-index into
a `bench_*` collection; then run the A/B the 2026-07-28 entry asked for — boost=5.0, mode=code,
sparse ON, with reranker arms that differ **only** in server. Note the one blocker for that last
part: isolating server from model needs the *same* model on TEI, and this host's `tei-rerank`
(:30083) serves `cross-encoder/ms-marco-MiniLM-L-6-v2`, not `bge-reranker-v2-m3`. A TEI arm on
it would change model and server together, which is exactly the confound the arm exists to
remove. Stand up a TEI with bge-reranker-v2-m3 first, or drop that arm and say so.

### 2026-07-28 — TEI→llama-server reranker swap costs ~13× p50; prefix rediscovered; scores are load-dependent

Three findings, one of which **corrects a claim I made earlier in the session** and one of
which is a rediscovery of the 2026-05-12 prefix result below.

**Setup.** Fresh `bench_base_` collection: 24,923 chunks, 809 indexable files (486 md / 254 rs),
indexed in 1239 s. `mode=full`, `--limit 10`, `boost=3.0`, **`CODESCOUT_DISABLE_SPARSE=1`**
(set by a concurrent session for VRAM reasons). Binary `5d3142e0`.

**1. The reranker's latency regressed ~13×, and it is my swap that did it — not the reranker.**

| | 2026-05-12 champion | 2026-07-28 |
|---|---|---|
| reranker | `bge-v2-m3` via **TEI** `:48083` | same model as **Q4_K_M GGUF on llama-server** `:48083` |
| p50 | ~141-157 ms (boost sweep rows) | **1994-2176 ms** |

Same model, different server. The GGUF/llama-server swap was made 2026-07-27 to fix a CUDA OOM
(`docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md`) — TEI at `--dtype float16` could not warm
up on this 6 GiB card. That fix traded **VRAM for latency**, and the latency cost was never
measured until now. Mechanism is unchanged in either server: `SearchOpts::new` sets
`overfetch: limit * 2` and `search_in` reranks every candidate, so `limit=10` means 20
cross-encoder passes per query — TEI batches those; llama-server appears not to.

**This reframes `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`**, which
I filed as "the reranker costs 42×". The correct statement is narrower and more useful: *the
GGUF/llama-server reranker costs ~13× what the TEI one did.* Worth trying before abandoning
reranking: a TEI reranker with `--max-batch-tokens` capped low enough to fit 6 GiB, or
`bge-reranker-base` on TEI.

**2. My reranker score A/B is NOT comparable to the 2026-05-12 row.** I measured 32/75 with
rerank vs 35/75 without, and read that as "the reranker hurts". The recorded champion is
`coderank Q4 + bge-v2-m3 (TEI) = 37/75`. Four config dimensions differ between the two runs —
sparse (off vs on), boost (3.0 vs 5.0), mode (`full` vs `code`), and reranker server — so the
comparison establishes nothing about the reranker's value. Do not cite the −3 as a reranker
verdict.

**3. Scores are load-dependent — my "zero variance" claim was conditional on a quiet box.**
Four runs per arm on an idle machine gave *exactly* 32/32/32/32 and 35/35/35/35, and I concluded
retrieval is deterministic. Re-running the same command while a background `codescout index` was
saturating the GPU and writing to Qdrant produced **37 then 35 for the identical arm**. So:

- determinism holds only with no concurrent index and no Qdrant write load
- **any bench run must first assert `pgrep -af 'codescout index'` is empty**, and the harness
  should arguably refuse to run otherwise
- the p50 figures from loaded runs (4173-4845 ms) are contaminated and were discarded

**4. Query prefix — rediscovery, consistent with 2026-05-12.** Measured 37 (no prefix) vs 32
(with) on one pass, then 35 (no prefix) on a second, loaded pass. Direction matches the recorded
−2 to −4 exactly; magnitude is at or above the top of that range and the loaded pass makes my
number untrustworthy. **The finding that matters is not the magnitude but the location:** the
live MCP server config at `/home/marius/.claude-sdd/.claude.json` →
`mcpServers/codescout/env` *sets* `CODESCOUT_QUERY_PREFIX`, while `.env.gpu` deliberately
comments it out with the "37, champion" note. The two config sources have drifted, and the live
one carries the setting this tracker recorded as harmful 2.5 months ago.

**Env drift in the live config — CORRECTED 2026-07-28 after reading the resolution code.** The
first pass of this entry got two of four items wrong by inspecting only two of the **three**
config layers. Precedence, from `src/config/global.rs:109-160`:

1. **MCP `env` block** in `<profile>/.claude.json` — the real process environment. Always wins.
2. **`$CODESCOUT_ENV_FILE`, else `<global_config_dir>/.env`** — `startup_env_assignments` filters
   to `!is_set(key)`, so this layer fills *only* keys layer 1 left unset. On this host
   `~/.config/codescout/.env` is a **symlink to `.env.amd`**.
3. **Hardcoded defaults** in `RetrievalConfig::from_env`.

`load_startup_env` never reads the CWD by design — *"a user-scoped server must not absorb an
arbitrary repo's `.env`"* — so **no repo-root `.env.*` is read at all**. `.env.gpu` is loaded by
nothing; `.env.amd` is loaded only because it is symlinked into the global config dir.

| var | verdict | detail |
|---|---|---|
| `CODESCOUT_QUERY_PREFIX` | **real — now fixed** | Set in `.claude-sdd` + `.claude-kat` MCP env, absent from `~/.claude`. Removed from both 2026-07-28; takes effect on MCP restart. Harmful *for our model specifically*: prefix is +3 on f16 but −4 on Q4_K_M, and we run `CodeRankEmbed-Q4_K_M.gguf`. |
| `CODESCOUT_DISABLE_SPARSE` | **my claim was wrong** | I wrote "`.env.gpu` sets it; live server does not". It *is* set live: `.env.amd` carries `CODESCOUT_DISABLE_SPARSE=1` and layer 2 supplies it because no profile's MCP env defines it. Sparse has been **off** on all three live servers. |
| `CODESCOUT_RETRIEVAL_PROFILE=amd` | **inert, not stale** | Read into `RetrievalConfig.profile` and then never consumed — a project-wide search for `.profile` finds only `src/util/path_security.rs` / `src/config/project.rs` (unrelated `SecurityProfile`) and two `tests/retrieval_unit.rs` assertions. A dead field, so the amd-vs-gpu mismatch has zero behavioural effect. |
| `CODESCOUT_RERANKER_PROTOCOL=infinity` | **not drift** | `infinity`, `cohere`, `llama-server`, `llama_server`, `llamacpp` all map to `Protocol::Infinity` (`src/retrieval/reranker.rs:18-27`). `.claude-kat` sets none, but layer 2 supplies `llama-server` from `.env.amd`. All three profiles resolve to the same variant. |

So **one** of the four was a real problem. `.env.gpu` being a template nothing sources still
holds, and the reason is sharper than "sourcing cannot unset": the server never looks at the repo
at all. That mechanism plus the symlink-target drift were **already filed** by a concurrent
session in `docs/issues/archive/2026-07-25-env-copy-flow-stale-model-dir.md` § *"2026-07-28 — the symlink
flow itself drifted"* — this is a rediscovery, not a second bug.

**Open, for the next session.** Re-run the reranker A/B on a quiet box against a
sparse-**enabled** collection at boost=5.0 / mode=code, so it is comparable to the 2026-05-12
champion row, and include a TEI-hosted reranker arm to isolate server from model. Until then the
only settled facts are the ~13× p50 regression and the config drift (whose prefix half is now
fixed).

**Comparability warning for every number in this entry.** Because `.env.amd` sets
`CODESCOUT_DISABLE_SPARSE=1` and layer 2 feeds it to every profile, *all* of this session's
measurements — the reranker A/B included — ran **dense-only**. The 2026-05-12 champion row
(37/75, p50 ~150 ms) was dense+sparse+rerank. That is a fourth axis of non-comparability on top
of boost, mode, and reranker host, and it is the one I was least aware of while measuring.


### 2026-05-12 — query prefix experiment (negative result) + extended boost sweep

Added `CODESCOUT_QUERY_PREFIX` env to `EmbedderHttp::embed()` (query side only;
`embed_batch()` doc-side untouched). Tested `"Represent this query for searching
relevant code: "` against CodeRankEmbed across boost ∈ {1, 2, 3, 5}:

| variant | no prefix | with prefix | Δ |
|---|---|---|---|
| dense-only | 32 | 30 | **−2** |
| fusion boost=1.0 | 33 | 30 | **−3** |
| fusion boost=2.0 | 34 | 31 | **−3** |
| fusion boost=3.0 | 34 | 32 | −2 |
| fusion boost=5.0 | **35** | 31 | **−4** |

**Prefix consistently hurts** by 2–4 points. Hypotheses (not yet validated):

1. Q4_K_M quantization may have collapsed the prefix-conditioned subspace.
2. Our docs were indexed without the doc-side training distribution (raw code only).
   If the model was trained with explicit `search_document:` style doc prefix
   (Nomic family convention), then prefix-asymmetry without re-indexing docs WITH
   the doc prefix breaks the asymmetric calibration.
3. Re-indexing docs with `search_document: ` doc-side prefix may recover the win.
   Not tested yet — would require a fresh `bench_coderank_qp_` collection.

Extended boost sweep (no prefix):

| boost | score | p50 ms |
|---|---|---|
| 0.5 | 32 | 143 |
| 1.0 | 33 | 141 |
| 2.0 | 34 | 146 |
| 3.0 | 34 | 152 |
| **5.0** | **35** | 148 |
| 7.0 | 34 | 146 |
| 10.0 | 33 | 150 |
| 15.0 | 33 | 145 |
| 20.0 | 33 | 157 |

**Boost peak is 5.0** on the 25-TC pinned bench. Beyond 5.0, BM25 starts crowding
out the dense candidates that actually carry signal. Plateau is broader than Phase 6
saw (it stopped at 3.0).

### 2026-05-12 — f16 vs Q4 quantization × query prefix

Tested the quantization-collapsed-prefix hypothesis on f16 weights at `bench_coderank_f16_`
(re-indexed separately). Spec lookup in `~/models/CodeRankEmbed-hf/config_sentence_transformers.json`
confirmed:

- **Query prefix:** `"Represent this query for searching relevant code: "` (exact, trailing space)
- **Doc prefix:** none — `prompts.query` is the only entry. Docs go in raw.

So our prior tests used the correct prefix. The doc-side hypothesis is invalidated.

| variant | Q4_K_M | f16 |
|---|---:|---:|
| boost=5.0, no prefix | **35** | 31 |
| boost=5.0, +prefix | 31 | 34 |

**On Q4: prefix hurts by 4. On f16: prefix helps by 3.** Quant hypothesis confirmed —
the asymmetric prefix subspace does not survive Q4_K_M.

f16 + prefix boost sweep (search-side prefix only):

| boost | 1.0 | 2.0 | 3.0 | 5.0 | 7.0 |
|---|---:|---:|---:|---:|---:|
| f16 +prefix | 33 | 34 | 34 | 34 | 34 |

f16+prefix plateaus at **34/75** from boost=2.0 onward — close to but **not exceeding**
Q4 no-prefix 35/75. Spec-conformant ≠ best.

**Working theory:** Q4_K_M's coarser dense signal lets BM25 dominate ranking on the
T5 keyword-bag tier (which contributes most of the score variance). f16's sharper
asymmetric vectors are mathematically "more correct" but BM25 already covers what
they'd recover, leaving net wash.

**Current champion: CodeRankEmbed-Q4_K_M, no prefix, fusion @ bm25_boost=5.0 → 35/75.**

### 2026-05-12 — T5 expected-path fix (4 of 5 TCs had wrong truth)

Inspected top-10 for each T5 query against the original expected list and found
**4 of 5 expected lists were authored against the wrong files**:

| TC | Original expected | Actual symbol location |
|---|---|---|
| TC-21 (ToolContext) | `src/tools/mod.rs` | `src/tools/core/types.rs` |
| TC-22 (ActiveProject) | `src/agent/mod.rs` | correct ✓ |
| TC-23 (EmbedderHttp) | `crates/codescout-embed/src/embedder.rs` | `src/retrieval/embedder.rs` |
| TC-24 (artifact augment) | `crates/librarian-mcp/src/tools/mod.rs` | `tools/augment.rs` + `catalog/augmentation.rs` |
| TC-25 (MockLspClient + circuit breaker) | `src/lsp/ops.rs`, `client.rs` | `mock.rs`, `client.rs`, `manager.rs` |

Re-ran champion config (CodeRank Q4 no-prefix boost=5.0) and jina baseline
(boost=3.0) on the corrected suite. T5 jumped **4/15 → 7/15** for both models:

| Run | T5 before | T5 after | Total before | Total after |
|---|---:|---:|---:|---:|
| CodeRank Q4 no-prefix b=5.0 | 4 | **7** | 35 | **37** |
| jina-v2 b=3.0 | 4 | **7** | 32 | **35** |

**New champion: CodeRankEmbed-Q4_K_M, no prefix, bm25_boost=5.0 → 37/75** (49.3%).

#### Remaining T5 failure: TC-24

`artifact augment params merge librarian tracker` still scores 0/3 — top-10 is
**all `.md` plans/specs/trackers**, no `.rs` makes it in. The actual implementation
(`tools/augment.rs`, `catalog/augmentation.rs`) is being out-ranked by every plan
document that discusses the augment feature in natural language. Real failure
mode worth surfacing — likely needs either md-vs-code score balancing or
query-side hints that lean toward code (e.g. `pub fn augment`, `impl Augmentation`).

### 2026-05-12 — code/full search modes (default to code)

`semantic_search` now accepts `mode: "code" | "full"`. Default is `code`, which
applies a Qdrant `must_not: language == markdown` filter to drop md/mdx chunks
from results. `full` reverts to prior behavior (all indexed sources).

Implementation: new `exclude_languages: Vec<String>` on `SearchOpts`, plumbed
through `search_in` to `hybrid_query` which builds a `Filter { must, must_not }`.

Re-ran champion configs on pinned bench:

| Run | Total | T5 | Notes |
|---|---:|---:|---|
| coderank b=5.0, mode=full (prior) | 37/75 | 7/15 | |
| **coderank b=5.0, mode=code (new default)** | **37/75** | **10/15** | T5 +3, T1-T4 −3 |
| jina b=3.0, mode=full (prior) | 35/75 | 7/15 | |
| **jina b=3.0, mode=code (new default)** | **36/75** | **11/15** | +1 net, T5 +4 |

**TC-24 went from 0/3 → 3/3 in code-mode.** The two expected files
(`crates/librarian-mcp/src/tools/augment.rs`, `crates/librarian-mcp/src/catalog/augmentation.rs`)
moved to ranks #1 and #3 with the .md plans/specs filtered out.

The total-score wash on coderank reflects a real trade-off: queries whose expected
answer IS a `.md` doc (TC-02 backend config, TC-05 PROGRESSIVE_DISCOVERABILITY,
TC-17 routing-plugin guide) lose points. This is the right behavior for the
common LLM use case (finding implementations) but users who want docs must
explicitly pass `mode="full"`.

**Updated champion: coderank Q4 no-prefix bm25=5.0 mode=code → 37/75**, with
the meaningful T5 improvement (10/15 vs prior 7/15) signaling better real-user
query handling.

### 2026-05-12 — initial pinned bench

Built `.worktrees/bench`, refactored 7 hard-coded collection literals to use
`config.collection(<kind>)` with `CODESCOUT_QDRANT_COLLECTION_PREFIX` override.
Added 5 T5 real-usage-shape TCs sampled from external `usage.db`. First 8 runs
land in the table above. CodeRankEmbed @ boost=5.0 is the current leader at 35/75.


### 2026-05-12 — legacy-natural reconstruction (settling the 41/60 question)

User asked why champion config scored 37/75 vs historical 41/60. Investigation:

1. **Inspected commit `a55f1458`**: it rewrote both queries *and* expected paths of
   multiple legacy TCs (natural-language → keyword-stuffed; pre-refactor paths →
   post-refactor paths). Methodology change, not bugfix.
2. **Extracted pre-`a55f1458` TC defs** into `scripts/tc-suites/legacy-natural.json`
   (20 TCs, natural queries). Remapped 10 expected paths (workflow.rs → run_command/mod.rs,
   markdown.rs → markdown/edit_markdown.rs, symbol.rs → symbol/edit_code.rs, etc.) so
   they exist at the pinned SHA. Verified zero missing.
3. **Ran both suites** at jina-v2 bm25=5.0 mode=code on pinned worktree:

   | Suite | Score | T5 |
   |---|---|---|
   | legacy-natural (20-TC, natural) | 25/60 | — |
   | legacy-keyword (20-TC subset of full suite) | 25/60 | — |
   | full 25-TC (legacy-keyword + T5) | 36/75 | 11/15 |

**Conclusion: 41/60 is not reproducible.** Natural and keyword queries scored
identical (25/60 each), so the query-style rewrite is innocent. The gap to 41/60
must come from one or more of: pre-pin chunking config, different bm25 boost (Phase 6
used 3.0), or a stale `code_chunks` collection that happened to align with the
pre-refactor expected paths. None of those states are reachable any more.

The honest baseline going forward is **25/60 legacy-natural / 36/75 25-TC** at the
pinned worktree. The `legacy-natural.json` suite is now committed so future runs
can keep this comparison alive without recomputing it from git history.


### 2026-05-12 — Reranker A/B/C: bge-v2-m3 (TEI) vs jina-rerank-v2 (Infinity)

Spun up Infinity 0.0.77 on `:48085` to host `jinaai/jina-reranker-v2-base-multilingual`
(TEI can't load it — custom XLM-R-flash architecture lacks standard `model_type` in
config.json). Added `CODESCOUT_RERANKER_PROTOCOL=tei|infinity` toggle to
`RerankerHttp` so codescout speaks both wire shapes (TEI uses `{texts, score}`,
Infinity/Cohere use `{documents, results.relevance_score}`).

Four configurations, all at bm25=5.0 / mode=code on pinned worktree:

| Embedder | Reranker | natural 20-TC | full 25-TC | T5 |
|---|---|---|---|---|
| jina-v2 | bge-v2-m3 (TEI) | 25/60 | 36/75 | — |
| jina-v2 | jina-rerank-v2 (Infinity) | 23/60 | **38/75** | 11/15 |
| coderank Q4 | bge-v2-m3 (TEI) | not measured | 37/75 | 10/15 |
| coderank Q4 | jina-rerank-v2 (Infinity) | 23/60 | 36/75 | 11/15 |

**Findings.**

- T5 (real-usage tier) is the cleanest signal: jina-rerank-v2 lifts it 10→11 on both
  embedders. bge-v2-m3 caps at 10/15. The +1 is the same TC every time — TC-25
  (LSP circuit breaker) flips 1→2.
- jina-v2 + jina-rerank-v2 wins on the full suite (38/75) but loses on
  legacy-natural (23/60). General-purpose multilingual reranker is keyword-friendly
  but loses on long natural-language queries.
- coderank Q4 + jina-rerank-v2 doesn't compound: 36/75 vs 37/75 for the bge baseline.
  Two code-tuned components don't stack — likely because both already over-fit to
  the same code patterns and add noise to each other.
- **Recommendation:** keep coderank Q4 + bge-v2-m3 (TEI:48083) as champion for
  full-suite stability, but consider jina-rerank-v2 swap when T5 improvement
  matters more than legacy parity. The protocol toggle makes this a one-env-var
  switch, no rebuild required.

Teardown: stopped Infinity container; `bench_jinav2_*` and `bench_coderank_*` Qdrant
collections preserved for future re-runs.


### 2026-05-12 — Golden-set audit + post-fix re-baseline

Audited both suites end-to-end. Findings:

**Structural (clean):** all required fields present, no dup ids/queries, tier ranges valid,
all 116 expected paths exist at the pinned SHA.

**Stale expectations (fixed):**
- TC-01 (both suites): `src/tools/mod.rs` → `src/tools/core/types.rs`. `mod.rs` is now
  only `pub mod foo;` declarations after the tools/ refactor; `RecoverableError` lives in
  `core/types.rs`.
- TC-14 (both suites): same fix.

**Filename-token bias:** 14/25 keyword TCs had the literal expected-file basename appearing
in the query (e.g. `path_security` ↔ `path_security.rs`, `MockLspClient` ↔ `mock.rs`).
Rewrote 7 queries to drop blatant cheats while preserving the underlying concept. Remaining
8/25 tokens are concept words (`client`, `output`, `schema`, `index`, `server`, `augment`,
`usage`) that real users would naturally type.

**Re-baseline at champion config** (bm25=5.0, mode=code, TEI reranker `bge-v2-m3`):

|  | natural 20-TC | full 25-TC |
|---|---|---|
| jina-v2 + bge | 26/60 (was 25) | 35/75 (was 36) |
| coderank Q4 + bge | 26/60 | **37/75** (champion confirmed) |

The TC-01/TC-14 expected-path fix gives +1 on natural. The bias removal costs jina-v2
−1 on full (less BM25 lift) but leaves coderank unchanged at 37 — code-aware embedder is
more robust to query rephrasing, which is the *desired* signal we couldn't see before
because BM25 was masking it.

The 41/60 historical claim remains unreproducible; **26/60 natural / 37/75 full** is the
honest post-audit baseline going forward.

### 2026-05-12 — Tavily stack (sqlite-vec + tantivy) + CodeRank, no reranker

**Goal:** Settle the "is the retrieval backend itself the bottleneck?" question.
Reproduce the May-2 31/60 ceiling by going back to the pre-Qdrant stack with the
best dense embedder we have today (CodeRankEmbed-Q4_K_M on llama-server-rocm :43300).

**Setup**

- Worktree pinned at `0795b208e8bab76705d6582f43431e39fcccedf4` (the 31/60 commit) → `.worktrees/bench-legacy/`
- Binary: `.worktrees/bench-legacy/target/release/codescout` (default features: `local-embed`, `remote-embed`, `dashboard`, `http`, `librarian`)
- Index target: `.worktrees/bench` (current code at `ede25e69`) — so TC paths still align with the post-refactor layout
- `[embeddings]` config: `model = "CodeRankEmbed-Q4_K_M.gguf"`, `url = "http://127.0.0.1:43300/v1"`
- Stack: dense via HTTP (coderank, dim 768) + tantivy BM25 in-process; **no reranker** (didn't exist in 0795b208)
- 730 files / 18 229 chunks indexed in ~47 s

**Result: 28/60 on legacy-natural** (p50 93 ms, p95 110 ms)

| Stack | Embedder | Sparse | Rerank | Legacy-natural |
|---|---|---|---|---|
| historical (May 2) | jina-v1-base-code | tantivy | — | 31/60 |
| **tavily + coderank** | **CodeRankEmbed-Q4** | **tantivy** | **—** | **28/60** |
| Qdrant + reranker (today's best) | jina-v2-base-code | splade-cocondenser | bge-v2-m3 | ~30/60 (38/75) |

**Per-TC scores:** TC-01 1/3, TC-02 2/3, TC-03 3/3, TC-04 2/3, TC-05 2/3, TC-06 1/3,
TC-07 2/3, TC-08 2/3, TC-09 2/3, TC-10 1/3, TC-11 0/3, TC-12 1/3, TC-13 1/3,
TC-14 1/3, TC-15 2/3, TC-16 1/3, TC-17 1/3, TC-18 2/3, TC-19 1/3, TC-20 0/3.

**Conclusions**

1. **Ceiling confirmed.** No matter which retrieval engine we swap in (Qdrant vec0,
   sqlite-vec, tantivy, fastembed-bm25, splade, splade-pp), top-10 hit-rate on the
   legacy-natural suite plateaus at 28–30/60. The bottleneck is upstream of the
   retrieval engine — query phrasing, chunk granularity, or embedding-model
   recall on code identifiers.
2. **Latency wins for the legacy stack.** 93 ms p50 vs Qdrant's ~300–500 ms p50 (with rerank).
   No HTTP hop to Qdrant, no second HTTP hop to a reranker.
3. **31/60 isn't a phantom but isn't repeatable either.** Within 3 points across
   completely different backends — this is noise band for a 20-TC suite where each
   TC is 0/3..3/3. The historical value was real but not load-bearing.
4. **No retrieval-stack reason to keep tantivy/sqlite-vec.** Drop them. The
   architecture decision is now justified: ship a thin codescout binary that talks
   HTTP to an external retrieval stack (Qdrant + sparse + dense + rerank).

**Caveats (recorded for honesty)**

- The harness env block records `embedder_url=:48081`, `sparse_embedder_url=:48084`,
  `reranker_url=:48083`, `qdrant_url=:6334`, `embed_model=jina-embeddings-v2-base-code`
  — these are env vars at harness invocation time; the legacy binary **ignored**
  all of them and read its own `[embeddings]` block. The recorded config block
  misrepresents the actual stack. Followup: add `backend = "stack" | "tavily"`
  detection in the harness (e.g. probe `codescout version` for a "retrieval-backend"
  field, or inspect `[embeddings].url` in project.toml).
- `codescout_build_sha` / `codescout_version` are empty because the 0795b208 binary
  predates the build-SHA bake-in (`ad7e7e7a`). `codescout_repo_head_sha` = 0795b208
  (recorded from `git rev-parse HEAD` in the legacy worktree).
- Index built with `--force`; chunks differ slightly (18 229 vs current 17 827+).

### 2026-05-12 — nomic-embed-code-7B Q4 (claimed CoIR SOTA) — negative result

**Goal:** Test whether a much larger code-specific embedder breaks the 28-31/60 ceiling.
Hypothesis: `nomic-ai/nomic-embed-code` (7B, Qwen2.5-Coder-7B-Instruct base, claimed SOTA
on CoIR per Nomic's blog) should outperform 137M-class models if the bottleneck is dense
recall on code identifiers.

**Setup**

- Model: `bartowski/nomic-ai_nomic-embed-code-GGUF` Q4_K_M (~4.1 GB, dim 3584, 32k ctx)
- Server: `llama-server` (CUDA, RTX A5000 24GB) on `:43302` with `--embeddings --pooling last`
- Query prefix: `"Represent this query for searching relevant code: "` (asymmetric, doc side raw)
- Codescout: `ad7e7e7a` (main binary). Env: `CODESCOUT_EMBEDDER_PROTOCOL=openai`,
  `CODESCOUT_EMBEDDER_MODEL_NAME=nomic-embed-code`, `CODESCOUT_MODEL_DIM=3584`,
  `CODESCOUT_QUERY_PREFIX=...`, `CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_nomic_`
- Collection: `bench_nomic_code_chunks` — dim 3584, 21 371 points
- Indexing: 24 923 chunks in 29 minutes (~14 chunks/sec — 7B fwd pass dominates)
- Rerank: bge-reranker-v2-m3 on `:48083` (unchanged)
- Sparse: splade-cocondenser on `:48084` (unchanged)
- Suite: `legacy-natural.json` (20 TCs, max 60)

**Result: 24/60 (worse than current stack and tavily+coderank)**

| BM25 boost | Score | p50 latency |
|---|---|---|
| 5.0 | 24/60 | 178 ms |
| 3.0 | 24/60 | 177 ms |
| 1.5 | 24/60 | 175 ms |
| 0.5 | 25/60 | 173 ms |

**Comparison**

| Stack | Embedder (params) | Sparse | Rerank | Legacy-natural |
|---|---|---|---|---|
| Qdrant + jina-v2 + bge-rerank | jina-v2-base-code (137M) | splade | bge-v2-m3 | 28/60 |
| Tavily (sqlite-vec + tantivy) | CodeRankEmbed-Q4 (137M) | tantivy | — | 28/60 |
| **Qdrant + nomic-embed-code-Q4 + bge-rerank** | **nomic-embed-code (7B)** | **splade** | **bge-v2-m3** | **24/60** |

**Findings**

1. **Bigger is not better here.** A 50× parameter model with a SOTA CoIR claim
   scored 4 points below jina-v2 on our TC suite. Indexing was 35× slower.
2. **BM25 fusion weight is irrelevant.** Sweeping 0.5–5.0 moves the score by 1
   point. The signal is in dense + rerank; fusion barely shifts top-10.
3. **Ceiling is genuinely upstream.** Across radically different dense embedders
   (jina-v2, CodeRankEmbed, nomic-embed-code, nomic-embed-code-7B), retrieval
   backends (Qdrant, sqlite-vec), sparse models (splade, splade-pp, tantivy
   BM25), rerankers (bge-v2-m3, jina-rerank-v2, none), and fusion weights, the
   top-10 hit-rate on legacy-natural sits in 24–28/60. **The bottleneck is the
   TC suite phrasing and/or chunking, not the retrieval stack.**
4. **Q4 quantization probably hurts but isn't the whole story.** We didn't run
   f16 (14GB VRAM, would have to evict other services) — but the spread on
   smaller models between Q4 and f16 was ≤1 point, so we'd expect 24→25 at
   best, not a breakthrough.

**Caveats**

- Project-id mismatch caused a 0/60 first run (sync used `bench_nomic` as id,
  search uses `p.config.project.name` from project.toml = `codescout`).
  Fixed by temporarily renaming project.toml; restored after the run.
- Discovered that `src/retrieval/embedder.rs::EmbedderHttp::embed` did not
  apply the asymmetric query prefix in older builds — main since fixed via
  `CODESCOUT_QUERY_PREFIX` env var. Earlier Qdrant-stack runs with CodeRankEmbed
  may have silently underperformed because the prefix wasn't applied on the
  query path (legacy `RemoteEmbedder` had it; new `EmbedderHttp` lacked it).
- Bench worktree at `ede25e69` was modified with redundant patches during this
  experiment (query_prefix_for + EmbedderHttp query_prefix). The patches are
  redundant because main already had `CODESCOUT_QUERY_PREFIX` support. Bench
  worktree is now slightly diverged from `ede25e69`; treat the canonical pinned
  bench as the main binary at `ad7e7e7a` going forward.

**Conclusion: drop nomic-embed-code-7B from consideration.** The
infrastructure cost (29-min reindex, 24 GB VRAM on the AMD or 6 GB on NVIDIA,
2× search latency) buys negative quality on our suite. If we want to break the
ceiling, the next levers are **TC suite phrasing audit** (drop or rephrase the
8/20 TCs that flatline at 0/3 across all configs) and **chunking strategy**
(node-aware chunks vs char-bounded splits).

### 2026-05-12 — Bench-doc pollution + mode=code blind spot (+6 points, no infra change)

**Goal:** After concluding the retrieval stack is not the bottleneck, audit the
TC suite itself. Look at zero-score TCs across the latest jina-v2 + bge-rerank
baseline and identify systematic issues.

**Findings (two no-cost bugs in the bench, not the stack)**

1. **`mode=code` post-filter blind spot.** The harness called `semantic_search`
   with default `mode="code"`, which post-filters out all markdown candidates.
   Several TCs have markdown-only expected files (TC-05's
   `docs/PROGRESSIVE_DISCOVERABILITY.md`, TC-17's docs in
   `docs/manual/src/concepts/`) — these TCs returned empty top-10 lists not
   because retrieval failed, but because every candidate was filtered out.
   Switching to `mode="full"` lifted score 26 → 29 (+3) on identical
   collection.

2. **Bench-doc data leak.** The TC queries are stored verbatim in
   `docs/research/2026-04-03-embedding-model-benchmark.md`,
   `docs/research/2026-05-06-retrieval-stack-benchmark.md`,
   `docs/trackers/retrieval-benchmark.md`, and `scripts/run-tc-benchmark.py`.
   Semantic search legitimately ranked those highest because they contain the
   exact query strings. **15 of 60 top-3 slots (25%) were pollution.** Deleting
   their chunks from Qdrant: 29 → 32 (+3). Combined with `mode=full`: 26 → 32
   (**+6 points = +23% relative**).

**Result: 32/60 on legacy-natural** with no infra change — beats the 31/60
"mythical historical ceiling" we couldn't reproduce, using the same
jina-v2-base-code + splade-cocondenser + bge-reranker-v2-m3 stack we already
had.

**Per-TC delta (jina-v2 + bge-rerank, both runs)**

| TC | mode=code | mode=full + depollute | Δ |
|---|---:|---:|---:|
| TC-01 | 1 | 2 | +1 |
| TC-02 | 0 | 2 | +2 |
| TC-03 | 2 | 2 | 0 |
| TC-04 | 2 | 2 | 0 |
| TC-05 | 0 | **3** | **+3** |
| TC-06 | 1 | 2 | +1 |
| TC-07 | 2 | 0 | -2 (now caught by audit, see below) |
| TC-08 | 2 | 2 | 0 |
| TC-09 | 2 | 2 | 0 |
| TC-10 | 1 | 1 | 0 |
| TC-11 | 2 | 2 | 0 |
| TC-12 | 0 | 2 | +2 |
| TC-13 | 1 | 2 | +1 |
| TC-14 | 1 | 2 | +1 |
| TC-15 | 2 | 2 | 0 |
| TC-16 | 1 | 0 | -1 (audit needed) |
| TC-17 | 1 | 2 | +1 |
| TC-18 | 2 | 1 | -1 (audit needed) |
| TC-19 | 1 | 1 | 0 |
| TC-20 | 0 | 0 | 0 (audit needed) |
| **Total** | **26** | **32** | **+6** |

**Fixes committed**

- `scripts/run-tc-benchmark.py`: new `--mode {code,full}` CLI flag, default
  `full`. Default change is reasonable because the suite mixes code and
  markdown expected files; `full` is the superset.
- `.codescout/project.toml`: added the four bench-doc paths plus
  `.codescout/projects/**` (codescout's own per-project memories — pure
  internal noise) and `scripts/tc-suites/**` to `[ignored_paths] patterns`.
  Existing chunks for these were deleted from Qdrant `code_chunks` via
  `points/delete` filter on `chunk_id` (text-match).

**Three remaining zero-score TCs — root causes, not pollution**

- **TC-07** "section boundary detection in markdown editing"
  Top-3 are all `chunker.rs` (text chunker that *also* detects section
  boundaries — semantically right, not the expected
  `src/tools/markdown/edit_markdown.rs`). The expected file's chunks just
  don't surface for this phrasing. Likely fix: rephrase to
  `"edit_markdown section heading replace insert_after action"` or accept
  `chunker.rs` as a valid match.

- **TC-16** "how a semantic search query flows from input through embedding
  to KNN ranked results"
  Top-10 dominated by design specs (`auto-reindex-on-edit-design.md`,
  `hybrid-retrieval-design.md`, `library-indexing-redesign.md`). Design docs
  legitimately answer "how does X flow?" better than the implementation
  files. Either rephrase to be implementation-anchored
  (`semantic_search call_tool semantic_search.rs RetrievalClient`) or
  broaden truth set to include the matching design docs.

- **TC-20** "three prompt surfaces consistent when tools are renamed"
  Top-1 is `CLAUDE.md` — which **is** the canonical place for the three-
  prompt-surfaces doctrine. The expected files
  (`src/prompts/server_instructions.md`, `onboarding_prompt.md`) are the
  prompts themselves, not the meta-discussion the query asks about. **The
  truth set is wrong.** Either add `CLAUDE.md` to expected, or rephrase to
  ask about prompt content rather than the consistency pattern.

**Implication for the retrieval-stack design**

- 28/60 was never the true ceiling — it was 26/60 with hidden mode and
  pollution bugs. Honest current ceiling on jina-v2-base-code (137M, default
  stack) is **32/60**, with 3 TCs requiring TC-suite repair (not retrieval
  fixes) to potentially reach 36-38/60.
- Reinforces the earlier conclusion: **don't strip-and-ship the stack
  optimization-side without first repairing the TC suite**, otherwise the
  bench remains an unreliable signal for future retrieval improvements.
- Concretely: before locking docker-compose profiles, run a "clean bench"
  (post-fix) sweep across the jina-v2 / CodeRankEmbed / nomic-embed-code
  matrix to see whether the model-size ranking changes once the noise is
  removed. The earlier negative result for nomic-embed-code (24/60) is
  suspect because it inherited the same mode and pollution bugs.
