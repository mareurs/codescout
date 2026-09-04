# Artifact Chunk-Grain Retrieval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `artifact(find, semantic=…)` return the *entry* that matched — file, line range, matched text and citable `PREFIX-N` token — instead of ranking whole artifacts by their preamble.

**Architecture:** Re-key `artifact_vec` from artifact ids to opaque chunk ids (no `vec0` schema change — the column is already `TEXT PRIMARY KEY`), add an `artifact_chunk` side table holding line ranges and entry tokens, and port the shape `code_chunk`/`code_vec` already uses for 33,032 markdown chunks in the same process. One new parameter, `max_per_artifact`, serves both consumers: `context` passes 1 (artifact grain, behaviour preserved), `find` passes 3 (chunk grain, no ledger monopolises a page).

**Tech Stack:** Rust, `rusqlite` 0.39 (bundled SQLite + `sqlite-vec`), `uuid` 1.20 (v4), `codescout-embed` chunker, `pulldown-cmark`.

**Spec:** `docs/superpowers/specs/2026-09-02-artifact-chunk-grain-retrieval-design.md` (committed `47ac0937`, patch-id `e4a55730ced544ba355c5b4281722ca132e05b14`)

**Bug:** `docs/issues/archive/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md` (artifact `154848bbd55e7768`)

## Execution status — read this before dispatching any task

**Tasks 1–6 are shipped, and the checkboxes did not say so.** Two sessions executed this
plan concurrently and neither ticked a box, so on 2026-09-02 the plan read 0 of 71 steps
done against six finished tasks. A subagent dispatched from the unamended plan would have
re-implemented all six. The ticks below Task 1–6 were added retroactively on 2026-09-02
from the evidence in this table — they are a *record*, not a live trace.

Authorship is by the `Session-Id:` trailer on each commit, which is a **recorded field**.
It is not adjacency (`git diff --stat` names insertions and no author) and not a
self-reported name (a name is registry-minted and re-minted by compaction or by a restart
under another profile; the sessionId survives both).

| Task | Commits | `Session-Id:` |
|---|---|---|
| 1 | `ee111a3d` | `19e0e253` |
| 2 | `aebb285c`, `304a4d8a` | `19e0e253` |
| 3 | `be043276`, `51a441a3` | `19e0e253` |
| 4 | `a3d2aba3`, `79cafa16` | `19e0e253` |
| 5 | `e811ffd6`, `22845220`, `dbfd1d0c`, `9e71f25e` | `19e0e253` |
| 6 | `411a6523`; hardening in `de434ca5` | `19e0e253`; `ffb95976` |
| 7 | `04444ba2` | `ffb95976` |
| 8 | `9e2b93d2`, `dcf3940a` | `ffb95976` |
| 9 | `e67c3221` | `ffb95976` |
| 10 | `95b77262` | `ffb95976` |
| 11 | `98eb5adc`, `488192e8` — **backfill + fix (c) only; the swap is NOT written** | `ffb95976` |
| 12 | `36afd405`, `6ff477b8` — **Steps 1–4 done; 5 and 7 held on open question 4** | `ffb95976` |

`19e0e253-6b26-4a74-a201-33c92fbd0b30` is session `codescout-20` (profile `~/.claude`).
`ffb95976-dc89-4cca-87aa-c026544faf2f` is the session that wrote this block.

**What a retroactive tick is evidence for, and what it is not.** Every task's deliverable
was read on disk on 2026-09-02 and every commit above was resolved from `git log`. That
covers each step which leaves a durable artifact — write the tests, implement, gate and
commit. It does **not** cover the `Step 2: Run to verify they fail` reds: a red leaves
nothing in the tree, so those ticks rest on the commit sequence rather than on an
observation. Read a tick as *"this task shipped"*, never as *"every step was witnessed"*.
Task 6 Step 5 is the one exception — it was run and the red observed; see its annotation.

**Tasks 7, 8, 9 and 10 shipped on 2026-09-02**, and unlike the rows above them those four
ticks *are* a live trace — written by the session that made them, as each landed. **Task 8
closes the outage**: between Tasks 6 and 8 artifact semantic search returned an empty page
for every re-indexed artifact.

**Task 11 is HALF done and Task 12 has not started.** Task 11's backfill and its
mandatory fix (c) shipped; `swap_artifact_vec` was deliberately not written, and its
Step 5 is not runnable as written — read that task's own block before touching either.
The negative verification that used to stand here for 7–12
has been consumed by those four tasks and is not re-derivable:
`src/librarian/catalog/find.rs` now contains `max_per_artifact`, and Task 7's `gc.rs` half
was **withdrawn as redundant** rather than performed — the trigger plus the FK cascade
already collect chunk vectors, and the plan aimed the change at `apply_rehome`, which is not
a delete site. Read Task 7's own block before treating its absence from `gc.rs` as evidence
of anything.

**Read each shipped task's own block before reusing its Files list or its `git add` line.**
Three of the four were wrong about where the code lives or what a test could detect, and
Task 10's was wrong in **both** places consistently — which is why cross-checking a plan
against itself does not catch this and reading the code does.

Two carried rulings for whoever picks them up:

- **Read `8b48b8f1` before starting Task 7.** It amends Task 7, which as originally written
  would have shipped dead code.
- **Task 11's swap stays blocked** on the diagnosis in `2bc70c26` — the indexer stamps
  content-seen before it embeds, trapping 729 artifacts. Task 11's *backfill* half is not
  blocked. The standing ruling is stop-and-surface; never backfill into the hole.

## Global Constraints

- **The chunk budget does NOT change.** It stays `512` tokens / 2,048 chars. `chunk_size_for_model` returns a *ceiling*, not a target; `AST_CHUNK_TARGET = 3000` and the benchmark-backed `STACK_CHUNK_TARGET = 1200` are the project's deliberate window. A task that raises it is wrong — see the spec's § *Retraction*.
- **Next schema version is 11.** `schema_version` is already at 10 (`src/librarian/catalog/mod.rs:244`). Migrations are additive and idempotent inside `apply_migrations_in_txn` (`mod.rs:131`), using `CREATE TABLE IF NOT EXISTS` then `INSERT OR IGNORE INTO schema_version (version) VALUES (11)`.
- **Gate before every commit**, in this order, the two test lanes chained with `;` **never** `&&`:
  ```
  cargo fmt
  cargo clippy --workspace --all-targets --features local-embed -- -D warnings
  cargo test --workspace --no-default-features ; cargo test --workspace
  ```
- **Git on this repo has 3 worktrees.** Bare `git commit` is blocked by a guard. Always `git -C /home/marius/work/claude/codescout commit …`.
- **Never `git add -A`/`-u`/`.`** — this checkout is shared with other live sessions. Stage by explicit path only; a pre-commit hook refuses an index carrying another session's paths.
- **Iron Laws:** use `symbols`/`edit_code` for Rust, `read_markdown`/`edit_markdown` for markdown, never an unbounded `run_command` pipe.
- **Any bug noticed goes in `docs/issues/`** the moment it is noticed, with exactly one `cluster/<slug>` tag written through the catalog (`artifact(action="update", …, patch={tags:[…]})`), never a raw frontmatter edit.

## Entry condition — NOT a task in this plan

**P1 — the vector-coverage hole. DIAGNOSED 2026-09-02.** The original text of this section is superseded and quoted below, because its derivation is what changed:

> *"1,406 of 4,525 catalogued artifacts have a vector (31.1%); within codescout, the repo with a configured embedder, 717 of 1,357 have none (52.8%). The cause is **not established**. The pattern fits `docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md` … but that is a hypothesis, not a reproduction."*

The reproduction was run. `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` (artifact `a766aad35b0b7610`, severity high) establishes the mechanism: `index_repo_sync` commits an artifact's `file_sha256` at `src/librarian/indexer.rs:302` **unconditionally**, then at `:309` decides whether to embed by reading that same stamp. A run reaching `:302` without reaching `:309`'s true branch records the content as seen while leaving it unembedded, and every later run then computes `content_unchanged == true` and skips it. The state is **absorbing**: `index_repo` calls `index_repo_sync(…, false, false)`, hardcoding both `force_rewalk` and `force_embed`, so no automatic caller can supply the lever that escapes.

**729 of codescout's 1373 catalog artifacts (53%) have no searchable representation of any kind** — no legacy per-artifact vector, no chunk rows — and no ordinary reindex can give them one. The derivation, including the `sha256sum -c` pass proving those 729 are byte-identical to what was indexed, is in the bug file. Read it there; do not restate the number from here.

**Two mechanisms, not one — and the archived file is the other half, not a duplicate.** `2026-07-25-reindex-reembed-noop-without-force.md` is a **policy** finding: reindex will not re-embed unchanged content without `force`. The new file is a **bookkeeping** finding: `:302` manufactures the "unchanged" condition on content that was never embedded. The archive describes the escape hatch; this describes the hole it has to be used on. Neither supersedes the other, and citing only one of them describes half the system.

**What is still NOT established, and it bears directly on Task 11.** The bug's `unverified:` field records that the *entry* path is unknown — measurement proves the 729 are trapped and cannot leave, not which run stamped each one. A run with no embedder, a per-artifact embed error, and an artifact predating embeddings are not distinguishable after the fact, because the catalog keeps no per-artifact embed-attempt record. Task 11 does not need the entry path: a backfill needs the *exit*. But this is exactly why fix (c) in that file — an `IndexReport` count of `want_embeddings && !has_vector` — is the part that must ship under § *Observer Blindness*, and not merely the tidy one. Without it the next occurrence is as unreconstructable as this one.

**So the prerequisite changed shape: it is now a fix plus a backfill, not an investigation.** Tasks 1–10 and 12 still do not depend on it. Task 11 does, and its blocker text is amended to say which half is lifted.

---


## Deferred — named so nobody reads its absence as coverage

**AMENDED 2026-09-02 — the original text of this section was wrong twice, and
both errors pointed the same way: they made a silent breakage read as a
deliberate deferral.** It said *"`src/librarian/artifact_store.rs:216` is the
Qdrant write path and is left untouched, so on a Qdrant-backed catalog
`artifact(find, semantic=)` keeps its present artifact-grain, first-chunk-only
behaviour."* Verified against the code:

- **`:216` is not Qdrant.** It is the last line of `SqliteVecArtifactStore::upsert`
  (`impl` at `:210`, `upsert` at `:211`). `QdrantArtifactStore`'s `upsert` is at
  `:161` (`impl` at `:160`). The guard named the wrong backend, and so implied the
  sqlite write path was handled — when the sqlite write path is the exact one the
  plan leaves broken (see Task 7's amendment).
- **Qdrant does not keep its present behaviour.** `ArtifactVectorStore::upsert`
  carries exactly **one** id, and after Task 6 that id is a *chunk* id for every
  backend. `QdrantArtifactStore::upsert` (`:161`) passes it to
  `artifact_upsert` (`src/retrieval/artifact.rs:77`), which derives the point id
  from it *and* writes it into the payload as `artifact_id` (`:85`, `:89`). So
  Qdrant is already storing chunk-keyed points whose payload claims they are
  artifact ids. Nothing about its behaviour is unchanged.

**What is actually deferred: Qdrant CHUNK-GRAIN PARITY** — carrying both ids
(chunk id for line ranges, artifact id for hydration) through the trait, which
needs a signature widening that reaches Task 8's read shape too. This matches
the spec's open question 4, which records the decision as undecided rather than
made.

**What is NOT deferrable, and is now a required step in Task 7: the Qdrant path
must not silently receive chunk ids.** A deferral means "this backend keeps
working as before"; today it means "this backend returns nothing, slowly, and
says so nowhere". Task 7 Step 3b specifies the guard. Until that guard exists,
the deferral is not a decision — it is an undetected outage on the *default*
backend of the server build (`ArtifactBackend::resolve`, `:42`).

**Before shipping, establish which backend this deployment actually uses** —
and if it is Qdrant, this plan does not apply to it yet, and Task 7's guard is
what tells the operator that rather than leaving them to discover it from an
empty result.
## File Structure

| File | Responsibility |
|---|---|
| `crates/codescout-embed/src/chunker.rs` | **Modify.** Add heading-depth parameter to `split_markdown`. |
| `src/librarian/entry_token.rs` | **Create.** Pure parsing: which `PREFIX-N` entry encloses a line. Fenced-code aware. |
| `src/librarian/catalog/mod.rs` | **Modify.** Schema v11 in `apply_migrations_in_txn`. |
| `src/librarian/catalog/chunk.rs` | **Create.** `artifact_chunk` CRUD, chunk-id allocation, per-artifact replace. |
| `src/librarian/indexer.rs` | **Modify.** Emit every chunk; write chunk rows; backfill runner. |
| `src/librarian/catalog/gc.rs` | **Modify.** Vector cascade fans out over an artifact's chunks. |
| `src/librarian/catalog/find.rs` | **Modify.** `semantic_find` gains `max_per_artifact`; hits carry chunk fields. |
| `src/librarian/tools/context.rs` | **Modify.** Pass `max_per_artifact = 1`. |
| `src/librarian/tools/artifact.rs` | **Modify.** Chunk-grain result shape for `find(semantic=)`. |
| `scripts/tc-suites/artifact-entries.json` | **Create.** 12 artifact-retrieval test cases with entry-token ground truth. |
| `scripts/run-artifact-bench.py` | **Create.** Scores the suite against the live artifact path. |

---

## Task 1: Artifact-TC benchmark suite and BASELINE capture

Runs first, before any behaviour changes, because a before/after comparison needs the "before" measured on the unmodified implementation. `scripts/run-tc-benchmark.py`'s 25-TC suite scores `bench_<model>_code_chunks` only — the path being changed has no instrument today.

**Files:**
- Create: `scripts/tc-suites/artifact-entries.json`
- Create: `scripts/run-artifact-bench.py`
- Modify: `docs/trackers/retrieval-benchmark.md` (via `artifact` tools — it is augmented, do NOT edit the file directly)

**Interfaces:**
- Produces: `scripts/run-artifact-bench.py --suite <path> --out <json>` writing `{"suite": str, "n": int, "hits_at_5": int, "mrr": float, "cases": [{"id","query","expect_entry","expect_path","rank"}]}`

- [x] **Step 1: Write the suite file with 12 cases**

Ground truth is an entry token plus its defining file. Pick queries whose answer is unambiguous and lives **after** the first heading — that is the property under test.

```json
{
  "suite": "artifact-entries",
  "derived": "2026-09-02, entries verified present by `grep -n '## W-81' docs/trackers/bug-fix-session-log.md`",
  "cases": [
    {"id": "AE-1", "query": "choosing where a gate lives by how fast it reports",
     "expect_entry": "W-81", "expect_path": "docs/trackers/bug-fix-session-log.md"},
    {"id": "AE-2", "query": "the parameter your own context supplies for free",
     "expect_entry": "OB-1", "expect_path": "docs/trackers/observer-blindness.md"},
    {"id": "AE-3", "query": "no Git for Windows means no commands at all",
     "expect_entry": "WIN-36", "expect_path": "docs/trackers/windows-platform-support.md"},
    {"id": "AE-4", "query": "augmented tracker params are a citation surface",
     "expect_entry": "SD-11", "expect_path": "docs/trackers/structural-debt-refactor.md"}
  ]
}
```

**Before writing the remaining 8 cases, verify each entry exists and sits after the first heading:**

```bash
for e in W-81 OB-1 WIN-36 SD-11; do
  grep -rn "^#\{2,4\} $e —" docs/trackers/ | head -1
done
```

Add 8 more the same way, spread across at least 4 distinct files, at least 2 of them `docs/issues/` bug files. A case whose entry is in the first chunk is inert for this suite — annotate any you keep as inert so nobody credits it with coverage.

- [x] **Step 2: Write the scorer**

```python
#!/usr/bin/env python3
"""Score artifact-path retrieval against known entry tokens.

Distinct from run-tc-benchmark.py, which scores bench_<model>_code_chunks.
This one exercises artifact(find, semantic=) — the path with no instrument.
"""
import argparse, json, re, subprocess, sys

ENTRY = re.compile(r'^#{2,4}\s+([A-Z]{1,3}-\d+)\s+[—–-]\s')

def entry_at(path, line):
    """The PREFIX-N entry enclosing 1-indexed `line`, or None."""
    tok = None
    with open(path, encoding="utf-8") as fh:
        for i, text in enumerate(fh, 1):
            if i > line:
                break
            m = ENTRY.match(text)
            if m:
                tok = m.group(1)
    return tok

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--bin", default="target/debug/codescout")
    args = ap.parse_args()

    suite = json.load(open(args.suite))
    cases, hits, rr = [], 0, 0.0
    for c in suite["cases"]:
        proc = subprocess.run(
            [args.bin, "artifact", "find", "--semantic", c["query"], "--limit", "5", "--json"],
            capture_output=True, text=True)
        results = json.loads(proc.stdout or "{}").get("items", [])
        rank = None
        for i, r in enumerate(results, 1):
            path = r.get("rel_path") or r.get("abs_path", "")
            if not path.endswith(c["expect_path"]):
                continue
            # Pre-change the tool returns artifacts, not chunks: a path match with
            # no line range cannot prove the ENTRY was found, so it is not a hit.
            line = r.get("start_line")
            if line is not None and entry_at(path, line) == c["expect_entry"]:
                rank = i
                break
        if rank:
            hits += 1
            rr += 1.0 / rank
        cases.append({**c, "rank": rank})

    out = {"suite": suite["suite"], "n": len(cases),
           "hits_at_5": hits, "mrr": round(rr / max(len(cases), 1), 4), "cases": cases}
    json.dump(out, open(args.out, "w"), indent=2)
    print(f"{out['suite']}: hits@5 {hits}/{len(cases)}  MRR {out['mrr']}")

if __name__ == "__main__":
    sys.exit(main())
```

- [x] **Step 3: Run it against the CURRENT implementation to capture the baseline**

```bash
cargo build
python3 scripts/run-artifact-bench.py \
  --suite scripts/tc-suites/artifact-entries.json \
  --out /tmp/claude-1000/artifact-bench-baseline.json
```

**Expected: `hits@5 0/12`, `MRR 0.0`.** Today's results carry no `start_line`, so no case can score. A non-zero baseline means the scorer's hit condition is wrong — fix the scorer, not the expectation.

- [x] **Step 4: Record the baseline in the benchmark tracker**

The tracker is augmented — write through the catalog, never the file:

```
artifact(action="find", filter={"rel_path": {"contains": "retrieval-benchmark"}})
artifact(action="update", id="<id>", patch={body_edits: [{
  heading: "## History", action: "insert_after", at: "after-heading-line",
  content: "\n### 2026-09-02 — artifact-path baseline\n\nFirst instrument for `artifact(find, semantic=)`. The 25-TC suite scores `bench_<model>_code_chunks` and never touched this path. Baseline on first-chunk-only: **hits@5 0/12, MRR 0.0** — no result carries a line range, so no case can score. Suite: `scripts/tc-suites/artifact-entries.json`.\n"}]})
```

- [x] **Step 5: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add scripts/tc-suites/artifact-entries.json scripts/run-artifact-bench.py docs/trackers/retrieval-benchmark.md
git -C /home/marius/work/claude/codescout commit -m "test(bench): artifact-path retrieval suite, and its 0/12 baseline"
```

---

## Task 2: `split_markdown` heading-depth parameter

`chunk_markdown` breaks on heading levels 1–6; `split_markdown` on 1–3 only (`chunker.rs:93`). 64 of this corpus's 1,482 defined entries sit at `####`. Do **not** change `split_markdown`'s default — its chunk ids encode `start_line`, so re-chunking would invalidate all 33,032 code-index chunks.

**Files:**
- Modify: `crates/codescout-embed/src/chunker.rs:83-121`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub fn split_markdown_with_depth(source: &str, chunk_size: usize, chunk_overlap: usize, max_heading_depth: usize) -> Vec<RawChunk>`
- Produces: `pub fn split_markdown(source, chunk_size, chunk_overlap) -> Vec<RawChunk>` — unchanged signature, delegates with `max_heading_depth = 3`

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn split_markdown_default_depth_ignores_h4() {
    // LOAD-BEARING: `#### D` must NOT start a chunk at the default depth.
    // The code index's chunk_ids encode start_line, so changing this default
    // silently invalidates 33,032 existing chunks.
    let src = "# A\n\ntext\n\n#### D\n\nmore\n";
    let chunks = split_markdown(src, 10_000, 0);
    assert_eq!(chunks.len(), 1, "h4 must not split at default depth");
}

#[test]
fn split_markdown_with_depth_6_splits_on_h4() {
    let src = "# A\n\ntext\n\n#### D\n\nmore\n";
    let chunks = split_markdown_with_depth(src, 10_000, 0, 6);
    assert_eq!(chunks.len(), 2, "h4 must split at depth 6");
    assert!(chunks[1].content.starts_with("#### D"));
    assert_eq!(chunks[1].start_line, 5, "line numbers stay 1-indexed and file-relative");
}

#[test]
fn split_markdown_with_depth_3_equals_the_default() {
    let src = "# A\n\nx\n\n## B\n\ny\n\n### C\n\nz\n\n#### D\n\nw\n";
    let a = split_markdown(src, 10_000, 0);
    let b = split_markdown_with_depth(src, 10_000, 0, 3);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.content, y.content);
        assert_eq!((x.start_line, x.end_line), (y.start_line, y.end_line));
    }
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test -p codescout-embed split_markdown_with_depth`
Expected: FAIL — `cannot find function split_markdown_with_depth`

- [x] **Step 3: Implement**

Replace the body of `split_markdown` at `chunker.rs:83`:

```rust
/// Split markdown by heading boundaries, then apply character limits.
///
/// Heading levels 1..=3 start a new section. For a caller that needs deeper
/// headings to split — the librarian's entry ledgers define entries at `####` —
/// use [`split_markdown_with_depth`]. The default is 3 and MUST stay 3: the
/// code index's `chunk_id` encodes `start_line`, so widening it here silently
/// invalidates every existing code chunk.
pub fn split_markdown(source: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<RawChunk> {
    split_markdown_with_depth(source, chunk_size, chunk_overlap, 3)
}

/// [`split_markdown`], with the heading depth that starts a new section made
/// explicit. `max_heading_depth` is clamped to 1..=6.
pub fn split_markdown_with_depth(
    source: &str,
    chunk_size: usize,
    chunk_overlap: usize,
    max_heading_depth: usize,
) -> Vec<RawChunk> {
    if source.is_empty() {
        return vec![];
    }
    let depth = max_heading_depth.clamp(1, 6);

    let lines: Vec<&str> = source.lines().collect();
    let mut sections: Vec<(usize, usize)> = vec![];
    let mut section_start = 0;

    for (i, line) in lines.iter().enumerate() {
        if i > 0 && heading_level(line).is_some_and(|l| l <= depth) {
            sections.push((section_start, i));
            section_start = i;
        }
    }
    sections.push((section_start, lines.len()));

    let mut chunks = vec![];
    for (start, end) in sections {
        let section_text = lines[start..end].join("\n");
        if section_text.len() <= chunk_size {
            chunks.push(RawChunk {
                content: section_text,
                start_line: start + 1,
                end_line: end,
                metadata: None,
            });
        } else {
            let sub_chunks = split(&section_text, chunk_size, chunk_overlap);
            for mut sc in sub_chunks {
                sc.start_line += start;
                sc.end_line += start;
                chunks.push(sc);
            }
        }
    }
    chunks
}

/// ATX heading level of `line` (1..=6), or `None` when it is not a heading.
/// Requires the space after the hashes, so `#hashtag` is not a heading.
fn heading_level(line: &str) -> Option<usize> {
    let stripped = line.trim_start_matches('#');
    let hashes = line.len() - stripped.len();
    (1..=6).contains(&hashes).then_some(hashes).filter(|_| stripped.starts_with(' '))
}
```

- [x] **Step 4: Run to verify they pass**

Run: `cargo test -p codescout-embed split_markdown`
Expected: PASS, including the four pre-existing `chunk_markdown_*` tests.

- [x] **Step 5: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add crates/codescout-embed/src/chunker.rs
git -C /home/marius/work/claude/codescout commit -m "feat(embed): split_markdown_with_depth, default 3 unchanged"
```

---

## Task 3: Entry-token extraction

A chunk hit must name a **citable** entry (`bug-fix-session-log:W-81`), which is the unit this project's conventions run on. The token grammar is fixed by the resolver: `[A-Z]{1,3}-\d+`, defined only by a heading of the form `## <ID> — <title>` (token, whitespace, dash, whitespace, text). A table row defines nothing, and a heading inside a fenced code block is an example, not a definition.

**Files:**
- Create: `src/librarian/entry_token.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod entry_token;`)

**Interfaces:**
- Produces: `pub fn entry_tokens_by_line(source: &str) -> Vec<Option<String>>` — one entry per **1-indexed line**, index 0 unused, each holding the token of the innermost entry heading at or above that line.

- [x] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_defines_the_token_for_every_line_below_it() {
        let src = "# Log\n\npreamble\n\n## W-81 — a title\n\nbody\n\n## F-3 — another\n\nmore\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[3], None, "preamble is outside every entry");
        assert_eq!(by_line[5].as_deref(), Some("W-81"), "the heading line itself");
        assert_eq!(by_line[7].as_deref(), Some("W-81"), "body below it");
        assert_eq!(by_line[11].as_deref(), Some("F-3"), "next entry takes over");
    }

    #[test]
    fn only_the_dash_form_defines_an_entry() {
        // Mirrors get_guide("tracker-conventions") § Entry headings exactly.
        let src = "## R-91\n\na\n\n### A-9 Addendum\n\nb\n\n| R-5 | row |\n\nc\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[3], None, "no title, no dash -> defines nothing");
        assert_eq!(by_line[7], None, "no dash -> a section ABOUT A-9");
        assert_eq!(by_line[11], None, "a table row never defines");
    }

    #[test]
    fn a_heading_inside_a_fenced_block_is_an_example_not_a_definition() {
        // LOAD-BEARING: docs teaching the syntax quote real-looking headings.
        // Counting one would make every guide a ledger. This is the IC-6
        // "parsers over a namespace owe an escape" case; the fence IS the escape.
        let src = "# Guide\n\n```\n## W-99 — not real\n```\n\nafter\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[4], None, "inside the fence");
        assert_eq!(by_line[7], None, "and it must not leak past the fence");
    }

    #[test]
    fn h4_entries_are_recognised() {
        // 64 of 1,482 entries in this corpus are defined at ####.
        let src = "# T\n\n#### BL-71 — deep\n\nbody\n";
        assert_eq!(entry_tokens_by_line(src)[5].as_deref(), Some("BL-71"));
    }
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib entry_token`
Expected: FAIL — `unresolved module` / `cannot find function entry_tokens_by_line`

- [x] **Step 3: Implement**

```rust
//! Which ledger entry (`W-81`, `BL-71`) encloses a given line of a tracker.
//!
//! The grammar is the citation resolver's, not a new one: a token is
//! `[A-Z]{1,3}-\d+`, and it is DEFINED only by a heading shaped
//! `## <ID> — <title>` — token, whitespace, dash (— – -), whitespace, text.
//! A heading with no title defines nothing; a table row defines nothing. See
//! `get_guide("tracker-conventions")` § Entry headings.
//!
//! Fenced code blocks are skipped. Documentation that teaches this syntax
//! quotes real-looking headings, and counting one would make every guide a
//! ledger — the fence is the escape this parser owes its namespace.

/// The entry token in scope at each 1-indexed line. Index 0 is unused padding
/// so callers can index by line number directly.
pub fn entry_tokens_by_line(source: &str) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None];
    let mut current: Option<String> = None;
    let mut fence: Option<usize> = None;

    for line in source.lines() {
        let trimmed = line.trim_start();
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        if ticks >= 3 {
            match fence {
                // A closing fence must be at least as long as the opener, so a
                // ``` inside a ```` block does not close it.
                Some(open) if ticks >= open => fence = None,
                Some(_) => {}
                None => fence = Some(ticks),
            }
            out.push(current.clone());
            continue;
        }
        if fence.is_none() {
            if let Some(tok) = heading_defines_entry(line) {
                current = Some(tok);
            }
        }
        out.push(current.clone());
    }
    out
}

/// The token this line DEFINES, if it is an entry-defining heading.
fn heading_defines_entry(line: &str) -> Option<String> {
    let rest = line.strip_prefix("##")?;
    let rest = rest.trim_start_matches('#');
    let rest = rest.strip_prefix(' ')?;
    let (token, tail) = rest.split_once(char::is_whitespace)?;

    let (alpha, digits) = token.split_once('-')?;
    if alpha.is_empty() || alpha.len() > 3 || !alpha.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    // A dash with text after it is what separates a definition from a mention.
    let tail = tail.trim_start();
    let tail = tail
        .strip_prefix('—')
        .or_else(|| tail.strip_prefix('–'))
        .or_else(|| tail.strip_prefix('-'))?;
    (!tail.trim().is_empty()).then(|| token.to_string())
}
```

Add to `src/librarian/mod.rs`:

```rust
pub mod entry_token;
```

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib entry_token`
Expected: PASS, 4 tests.

- [x] **Step 5: Verify against the real corpus (not just fixtures)**

```bash
cargo run --quiet --bin codescout -- --version >/dev/null
```

Then, in a scratch test, assert the parser finds **1,482** defined entries across `docs/trackers` and `docs/issues` — the number the spec derived. A different number means the parser and the spec's population disagree; reconcile before continuing, and state which is right.

- [x] **Step 6: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/entry_token.rs src/librarian/mod.rs
git -C /home/marius/work/claude/codescout commit -m "feat(librarian): entry_tokens_by_line — fence-aware, definition-rule exact"
```

---

## Task 4: Schema v11 — `artifact_chunk` and `artifact_vec_v2`

**Files:**
- Modify: `src/librarian/catalog/mod.rs:244` (immediately after the v10 stamp, inside `apply_migrations_in_txn`)
- Test: `src/librarian/catalog/mod.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: tables `artifact_chunk` and `artifact_vec_v2`; `schema_version` max becomes 11.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn v11_creates_the_chunk_table_and_stamps_the_version() {
    let cat = Catalog::open_in_memory().unwrap();
    let v: i64 = cat.conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 11);
    let n: i64 = cat.conn
        .query_row("SELECT COUNT(*) FROM artifact_chunk", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn v11_is_idempotent() {
    let cat = Catalog::open_in_memory().unwrap();
    apply_migrations_in_txn(&cat.conn, None).unwrap();
    let v: i64 = cat.conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 11, "re-running must not advance or duplicate");
}

#[test]
fn deleting_an_artifact_cascades_its_chunk_rows() {
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
    cat.conn.execute(
        "INSERT INTO artifact_chunk
           (chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token, content, content_hash)
         VALUES ('c1','a',0,1,9,NULL,'body','h')", []).unwrap();
    cat.conn.execute("DELETE FROM artifact WHERE id='a'", []).unwrap();
    let n: i64 = cat.conn
        .query_row("SELECT COUNT(*) FROM artifact_chunk", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "FK cascade must remove the chunk rows");
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib v11_creates_the_chunk_table`
Expected: FAIL — `assertion failed: left == right` (10 vs 11), then `no such table: artifact_chunk`

- [x] **Step 3: Implement**

Insert directly after the `VALUES (10)` stamp in `apply_migrations_in_txn`:

```rust
// v11: chunk-grain artifact embeddings.
//
// `artifact_vec` needs no schema change — its `id` is already TEXT PRIMARY KEY
// and nothing requires it to DENOTE an artifact. v11 adds the side table and a
// second vec table; Task 11 backfills v2 and swaps. Keeping both alive is what
// avoids a dark window over ~90,500 embeds.
//
// `chunk_id` is an OPAQUE uuid, deliberately not derived from artifact_id:
// `id = sha256(abs_path)`, so archiving re-keys an artifact, and a derived
// chunk id would make every archive move an O(chunks) loop through
// `gc::migrate_vec_id` (which exists only because vec0 rejects UPDATE ... SET id).
conn.execute(
    "CREATE TABLE IF NOT EXISTS artifact_chunk (
       chunk_id     TEXT PRIMARY KEY,
       artifact_id  TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
       chunk_ix     INTEGER NOT NULL,
       start_line   INTEGER NOT NULL,
       end_line     INTEGER NOT NULL,
       entry_token  TEXT,
       content      TEXT NOT NULL,
       content_hash TEXT NOT NULL,
       UNIQUE (artifact_id, chunk_ix)
     )",
    [],
)?;
conn.execute(
    "CREATE INDEX IF NOT EXISTS idx_artifact_chunk_artifact
       ON artifact_chunk(artifact_id)",
    [],
)?;
conn.execute(
    "CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec_v2 USING vec0(
       id        TEXT PRIMARY KEY,
       embedding FLOAT[768]
     )",
    [],
)?;
conn.execute(
    "INSERT OR IGNORE INTO schema_version (version) VALUES (11)",
    [],
)?;
```

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib v11`
Expected: PASS, 3 tests. Also run the full catalog module: `cargo test --lib librarian::catalog`

- [x] **Step 5: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/catalog/mod.rs
git -C /home/marius/work/claude/codescout commit -m "feat(catalog): schema v11 — artifact_chunk + artifact_vec_v2"
```

---

## Task 5: `artifact_chunk` write path

**Files:**
- Create: `src/librarian/catalog/chunk.rs`
- Modify: `src/librarian/catalog/mod.rs` (add `pub mod chunk;`)

**Interfaces:**
- Consumes: `entry_token::entry_tokens_by_line` (Task 3), `split_markdown_with_depth` (Task 2)
- Produces:
  ```rust
  pub struct ChunkRow {
      pub chunk_id: String,
      pub artifact_id: String,
      pub chunk_ix: usize,
      pub start_line: usize,
      pub end_line: usize,
      pub entry_token: Option<String>,
      pub content: String,
      pub content_hash: String,
  }
  pub fn build_chunks(artifact_id: &str, body: &str, chunk_size: usize) -> Vec<ChunkRow>;
  pub fn replace_chunks(cat: &Catalog, artifact_id: &str, rows: &[ChunkRow]) -> Result<Vec<ChunkRow>>;
  pub fn chunks_for(cat: &Catalog, artifact_id: &str) -> Result<Vec<ChunkRow>>;
  ```
  `replace_chunks` returns the rows **as stored** — chunk ids for unchanged `(artifact_id, chunk_ix, content_hash)` triples are preserved, so a re-index does not churn vectors.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn build_chunks_carries_line_ranges_and_entry_tokens() {
    let body = "# Log\n\npreamble\n\n## W-81 — a title\n\nbody text\n";
    let rows = build_chunks("a", body, 2048);
    assert!(rows.len() >= 2, "preamble and entry are separate chunks");
    assert_eq!(rows[0].entry_token, None, "the preamble is inside no entry");
    let w = rows.iter().find(|r| r.entry_token.as_deref() == Some("W-81")).unwrap();
    assert!(w.start_line <= 5 && w.end_line >= 7, "range brackets the entry");
}

#[test]
fn replace_chunks_preserves_ids_for_unchanged_chunks() {
    // This is what stops a re-index re-embedding an untouched 766 KB tracker.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let first = build_chunks("a", "# T\n\nx\n\n## W-1 — t\n\ny\n", 2048);
    let stored1 = replace_chunks(&cat, "a", &first).unwrap();
    let stored2 = replace_chunks(&cat, "a", &first).unwrap();
    assert_eq!(
        stored1.iter().map(|r| &r.chunk_id).collect::<Vec<_>>(),
        stored2.iter().map(|r| &r.chunk_id).collect::<Vec<_>>(),
        "identical content must keep identical chunk ids"
    );
}

#[test]
fn replace_chunks_drops_chunks_that_no_longer_exist() {
    // Absence assertion — pair it with the positive leg below, or a
    // replace that deletes EVERYTHING also passes.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let long = build_chunks("a", "# T\n\n## A-1 — x\n\na\n\n## A-2 — y\n\nb\n", 2048);
    replace_chunks(&cat, "a", &long).unwrap();
    let short = build_chunks("a", "# T\n\n## A-1 — x\n\na\n", 2048);
    replace_chunks(&cat, "a", &short).unwrap();
    let stored = chunks_for(&cat, "a").unwrap();
    assert_eq!(stored.len(), short.len(), "shrunk body drops the trailing chunks");
    assert!(
        stored.iter().any(|r| r.entry_token.as_deref() == Some("A-1")),
        "and KEEPS the surviving one — without this the test passes on total deletion"
    );
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib catalog::chunk`
Expected: FAIL — `unresolved module`

- [x] **Step 3: Implement**

```rust
//! `artifact_chunk` rows: the line-anchored, entry-tagged pieces an artifact is
//! embedded as. One row per chunk; `artifact_vec_v2` is keyed by `chunk_id`.

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::librarian::catalog::Catalog;
use crate::librarian::entry_token::entry_tokens_by_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRow {
    pub chunk_id: String,
    pub artifact_id: String,
    pub chunk_ix: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub entry_token: Option<String>,
    pub content: String,
    pub content_hash: String,
}

/// Chunk `body` at the librarian's grain: heading depth 6, so `####`-defined
/// entries start their own chunk. `chunk_size` is the CHARACTER budget and
/// stays 2,048 (512 tokens) — see the plan's Global Constraints before
/// touching it.
pub fn build_chunks(artifact_id: &str, body: &str, chunk_size: usize) -> Vec<ChunkRow> {
    let tokens = entry_tokens_by_line(body);
    codescout_embed::chunker::split_markdown_with_depth(body, chunk_size, 0, 6)
        .into_iter()
        .enumerate()
        .map(|(ix, raw)| {
            let mut hasher = Sha256::new();
            hasher.update(raw.content.as_bytes());
            ChunkRow {
                // Placeholder — replace_chunks assigns or preserves the real id.
                chunk_id: String::new(),
                artifact_id: artifact_id.to_string(),
                chunk_ix: ix,
                entry_token: tokens.get(raw.start_line).cloned().flatten(),
                start_line: raw.start_line,
                end_line: raw.end_line,
                content: raw.content,
                content_hash: format!("{:x}", hasher.finalize()),
            }
        })
        .collect()
}

/// Replace an artifact's chunk rows, preserving `chunk_id` wherever
/// `(chunk_ix, content_hash)` is unchanged so untouched chunks keep their
/// vectors. Returns the rows as stored.
pub fn replace_chunks(
    cat: &Catalog,
    artifact_id: &str,
    rows: &[ChunkRow],
) -> Result<Vec<ChunkRow>> {
    let existing = chunks_for(cat, artifact_id)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let reuse = existing
            .iter()
            .find(|e| e.chunk_ix == row.chunk_ix && e.content_hash == row.content_hash)
            .map(|e| e.chunk_id.clone());
        let mut stored = row.clone();
        stored.chunk_id = reuse.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        out.push(stored);
    }

    cat.conn
        .execute("DELETE FROM artifact_chunk WHERE artifact_id = ?1", [artifact_id])?;
    let mut stmt = cat.conn.prepare(
        "INSERT INTO artifact_chunk
           (chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token, content, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for r in &out {
        stmt.execute(rusqlite::params![
            r.chunk_id, r.artifact_id, r.chunk_ix as i64, r.start_line as i64,
            r.end_line as i64, r.entry_token, r.content, r.content_hash
        ])?;
    }
    Ok(out)
}

/// An artifact's chunk rows, ordered by `chunk_ix`.
pub fn chunks_for(cat: &Catalog, artifact_id: &str) -> Result<Vec<ChunkRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token,
                content, content_hash
           FROM artifact_chunk WHERE artifact_id = ?1 ORDER BY chunk_ix",
    )?;
    let rows = stmt
        .query_map([artifact_id], |r| {
            Ok(ChunkRow {
                chunk_id: r.get(0)?,
                artifact_id: r.get(1)?,
                chunk_ix: r.get::<_, i64>(2)? as usize,
                start_line: r.get::<_, i64>(3)? as usize,
                end_line: r.get::<_, i64>(4)? as usize,
                entry_token: r.get(5)?,
                content: r.get(6)?,
                content_hash: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

Add `pub mod chunk;` to `src/librarian/catalog/mod.rs`.

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib catalog::chunk`
Expected: PASS, 3 tests.

- [x] **Step 5: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/catalog/chunk.rs src/librarian/catalog/mod.rs
git -C /home/marius/work/claude/codescout commit -m "feat(catalog): artifact_chunk write path, id-preserving on re-index"
```

---

## Task 6: Indexer emits every chunk

**Files:** *(addressed by symbol, not by line — see the note below)*
- Modify: `src/librarian/indexer.rs` — `embed_queue_items` (the plan drafted it singular as
  `embed_queue_item`; it shipped **plural**), the two
  `embed_queue.extend(embed_queue_items(cat, &id, title, body)?)` enqueue sites inside
  `index_repo_sync`, and the `EmbedQueueItem` type alias
- Test: `src/librarian/indexer.rs` `#[cfg(test)] mod tests`

> **Why no line numbers here.** This line originally read
> `` `:54-75` … `:249-253` and `:284-288` … `:22` ``. By the time the task shipped, the four
> coordinates had drifted to `70-102`, `277`, `310` and `24` — **non-uniformly**, by `+2`, `+16`,
> `+28` and `+26`, because the edits between them inserted different amounts. No single offset
> repairs a set that drifts unevenly, so a reader correcting one coordinate learns nothing about
> the next three. A symbol name survives every insertion above it; `symbols(name="embed_queue_items",
> path="src/librarian/indexer.rs")` resolves it at whatever line it currently occupies.

**Interfaces:**
- Consumes: `catalog::chunk::{build_chunks, replace_chunks, ChunkRow}` (Task 5)
- Produces: `pub type EmbedQueueItem = (String, Option<String>, String);` **unchanged shape**, but element 0 is now a `chunk_id` rather than an artifact id, and there are N per artifact.
- Produces: `fn embed_queue_items(cat: &Catalog, id: &str, title: Option<String>, body: &str) -> Result<Vec<EmbedQueueItem>>`

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn embed_queue_items_emits_every_chunk_not_just_the_first() {
    // The regression test for this whole plan. Mutating the implementation back
    // to `.next()` must fail HERE.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let body = "# Log\n\npreamble\n\n## W-1 — first\n\nalpha\n\n## W-2 — second\n\nbeta\n";
    let items = embed_queue_items(&cat, "a", Some("Log".into()), body).unwrap();
    assert!(items.len() >= 3, "preamble + two entries, got {}", items.len());
    let texts: Vec<&str> = items.iter().map(|(_, _, t)| t.as_str()).collect();
    assert!(texts.iter().any(|t| t.contains("alpha")), "W-1's body must be embedded");
    assert!(texts.iter().any(|t| t.contains("beta")), "W-2's body must be embedded");
}

#[test]
fn embed_queue_items_keys_on_chunk_ids_that_exist_in_artifact_chunk() {
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let items = embed_queue_items(&cat, "a", None, "# T\n\n## W-1 — t\n\nx\n").unwrap();
    for (chunk_id, _, _) in &items {
        let n: i64 = cat.conn
            .query_row("SELECT COUNT(*) FROM artifact_chunk WHERE chunk_id = ?1",
                       [chunk_id], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "every queued id must be a real chunk row");
    }
}

#[test]
fn a_whitespace_only_section_is_dropped_without_dropping_the_batch() {
    // The embedder's guard bails the WHOLE batch on one empty input
    // (archive/2026-05-17-reindex-embedding-dim-mismatch.md). With N chunks the
    // filter has to be per-chunk, or one blank section aborts a full reindex.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let body = "# T\n\n## W-1 — real\n\ncontent\n\n##    \n\n\n\n## W-2 — also real\n\nmore\n";
    let items = embed_queue_items(&cat, "a", None, body).unwrap();
    assert!(!items.is_empty(), "the real chunks survive");
    assert!(items.iter().all(|(_, _, t)| !t.trim().is_empty()),
            "no empty text may reach the embedder");
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib indexer::tests::embed_queue_items`
Expected: FAIL — `cannot find function embed_queue_items`

- [x] **Step 3: Implement**

Replace `embed_queue_item` (`indexer.rs:54-75`) with:

```rust
/// Build the embed-queue entries for `body`: ONE PER CHUNK, keyed by chunk id.
///
/// Writes the artifact's `artifact_chunk` rows as a side effect, because the
/// chunk ids the queue is keyed on are assigned there — the queue and the rows
/// cannot be built independently without the two disagreeing.
///
/// Empty/whitespace-only chunks are filtered PER CHUNK, not per artifact: the
/// embedder's guard bails the WHOLE batch on a single empty input (see
/// `docs/issues/archive/2026-05-17-reindex-embedding-dim-mismatch.md`), and
/// with N chunks per artifact one blank section would otherwise abort an entire
/// bulk reindex.
///
/// Shared by both enqueue sites in [`index_repo_sync`] — the changed-content
/// path and the forced-re-embed path through the unchanged-row early return.
/// Keeping it in one place is what stops those two from drifting apart.
fn embed_queue_items(
    cat: &Catalog,
    id: &str,
    title: Option<String>,
    body: &str,
) -> Result<Vec<EmbedQueueItem>> {
    // 2048 chars = 512 tokens. Do NOT swap this for chunk_size_for_model:
    // that returns a CEILING (2048 tokens for CodeRankEmbed), and this project
    // deliberately chunks below it for ranking sharpness. See
    // docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md.
    const CHUNK_CHARS: usize = 512 * 4;

    let built = crate::librarian::catalog::chunk::build_chunks(id, body, CHUNK_CHARS);
    let stored = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built)?;
    Ok(stored
        .into_iter()
        .filter(|r| !r.content.trim().is_empty())
        .map(|r| {
            // Give a MID-entry chunk its entry's identity. `## W-81 — Choose a
            // gate's surface` may be thousands of characters upstream, so a
            // chunk from the middle of a five-chunk entry would otherwise embed
            // with no idea what it belongs to. Skipped when the chunk already
            // opens with its own heading, which is the common case.
            let text = match &r.entry_token {
                Some(tok) if !r.content.trim_start().starts_with("#") => {
                    format!("{tok}\n\n{}", r.content)
                }
                _ => r.content,
            };
            (r.chunk_id, title.clone(), text)
        })
        .collect())
}
```

Update **both** enqueue sites. At `indexer.rs:249-253`:

```rust
            if want_embeddings && force_embed {
                embed_queue.extend(embed_queue_items(cat, &id, title.clone(), body)?);
            }
```

At `indexer.rs:284-288`:

```rust
        if want_embeddings && (!content_unchanged || force_embed) {
            embed_queue.extend(embed_queue_items(cat, &id, title.clone(), body)?);
        }
```

Update the type alias doc at `indexer.rs:22`:

```rust
/// Items queued for embedding: `(chunk_id, title, chunk_text)`. One per CHUNK,
/// not per artifact — `chunk_id` keys `artifact_vec_v2`.
pub type EmbedQueueItem = (String, Option<String>, String);
```

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib indexer`
Expected: PASS. Existing indexer tests that count `artifact_vec` rows will still pass — nothing writes to `artifact_vec_v2` until Task 7.

- [x] **Step 5: Mutation check — this is the plan's central guard**

Temporarily restore `.into_iter().next()` on the chunk list. Run `cargo test --lib indexer`.
**Expected: `embed_queue_items_emits_every_chunk_not_just_the_first` FAILS.** If it passes, the test is not discriminating — fix the test before reverting the mutation.

> **RUN 2026-09-02 by session `ffb95976`. The guard holds, and it over-delivers.**
>
> | Point | `exit_code` | Result |
> |---|---|---|
> | Baseline, before mutating | `0` | 31 passed, 0 failed |
> | Mutated | `101` | **2 FAILED**, 29 passed |
> | After revert | `0` | 31 passed, 0 failed |
> | `git diff --stat -- src/librarian/indexer.rs` | — | empty |
>
> **Deviation from the step as written: the mutation applied was `.into_iter().take(1)`, not
> `.into_iter().next()`.** `.next()` yields `Option<ChunkRow>`, which cannot chain into the
> `.filter().map().collect()` that follows without a second `.into_iter()` — so the step's literal
> text does not compile. `.take(1)` is the type-correct realization of the same "first chunk only"
> defect, and is what the mutation must be written as if this step is ever re-run.
>
> **Two tests died, in different directions** — the step predicted one:
>
> ```
> ---- ..._emits_every_chunk_not_just_the_first stdout ----
> panicked at src/librarian/indexer.rs:1454:9: preamble + two entries, got 1
>
> ---- ...a_whitespace_only_section_is_dropped_without_dropping_the_batch stdout ----
> panicked at src/librarian/indexer.rs:1531:9: the real chunks survive
> ```
>
> The first is a **count** assertion, the second a **content** assertion. That is redundancy in two
> directions rather than the same test twice, which is what § *Testing Discipline*'s monotonicity
> law asks for: a count assertion alone is monotone under a change that keeps one correct chunk.
>
> **Both kills are assertion panics that name their test.** On a shared checkout that distinction
> is the whole point of the three-point sandwich: a bare `cargo test` red is one bit conflating an
> assertion firing (wanted), a compile error from a peer's in-flight edit, and an unrelated panic.
> The green baseline discriminates the compile error, the named panic discriminates the third, and
> the empty `git diff` proves no residue was left behind. A single red would have been worthless
> here.
>
> **CORRECTION, same session.** This annotation first read *"the mutation window was announced to
> the one peer sharing this tree beforehand, so a red observed inside it would have had an
> author."* **That was false, and the way it was false is the useful part.** The announcement was
> sent, and `SendMessage` returned `{"success": true, …  → codescout-20}` — which reads as
> delivered. It was **held for the recipient user's approval** and never reached that session; the
> `[Cross-session delivery notice]` saying so arrived **asynchronously, after this text was
> committed**. So the red *would* have been the anonymous kind recorded in
> `docs/issues/2026-09-01-un-wired-function-reds-the-shared-build-with-no-author.md`, and the
> author believed otherwise on the strength of a success return.
>
> The practice is still right — announce before mutating a shared tree — but **treat the send as
> unconfirmed until a reply arrives.** A `success` return means *accepted for delivery*, not
> *delivered*, and there is no synchronous signal that distinguishes them. This is why
> `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 5* is
> titled "an announce channel that was used and did not help" — that instance recorded the
> symptom; this one names the mechanism.

- [x] **Step 6: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/indexer.rs
git -C /home/marius/work/claude/codescout commit -m "fix(librarian): embed every chunk, not only the first — the core defect"
```

---

## Task 7: Write vectors to `artifact_vec_v2`, and fan out the delete cascade

> **DONE 2026-09-02 at `04444ba2`** by session `ffb95976`. Outcome, including the three
> ways the task as written did not survive contact with the code:
>
> | | |
> |---|---|
> | Shipped | `write_embeddings_v2`, `delete_chunk_vectors`, all three writers re-pointed, `SqliteVecArtifactStore::delete` reaching v2, the Qdrant grain guard |
> | Dropped | the `gc.rs` edit — redundant *and* aimed at a non-delete site; see the correction in § *Step 3* |
> | Moved | Step 1b's hydration test → Task 8 Step 1 |
> | Mutation | run and **killed** — `chunk_ix = 0` produced `must report every vector it removed, left: 1, right: 3`, an assertion panic naming the test, not a compile error. Reverted, diff clean |
>
> **The plan's own code failed to compile, twice over.** `delete_chunk_vectors` as written
> returns the `collect()` from a block, keeping a temporary alive past `stmt`'s drop
> (E0597) — the same class as Task 6's `.into_iter().next()`, and the second time this plan
> has shipped Rust that does not build. And `write_embeddings_v2` claims to mirror
> `write_embeddings_with`'s dim guard while omitting one of its three parts: the
> **intra-batch** consistency loop, which exists for the F-6b case (an embedder returning a
> 1-element error sentinel inside an otherwise good batch). Added, with a test.
>
> **An existing test was the independent detector.** `concurrent_embed_queue_completes_all`
> counted rows in `artifact_vec` and went red on the re-point — it had been pinning
> `index_repo`'s target table without saying so. It now asserts `v2 == 16` **and**
> `v1 == 0`, because the first alone is monotone under a writer that writes *both* tables,
> which is exactly what a half-finished re-point looks like.
>
> **Step 3a's exit criterion is NOT satisfiable in Task 7, and that is correct.** It asks
> that `grep artifact_vec` return only v1-migration paths and tests; `SqliteVecArtifactStore::knn`
> still reads v1, and moving it is **Task 8's** by name (that task's Files list says
> *"`knn` reads `artifact_vec_v2`"*). Re-check the criterion at the end of Task 8, not here.
>
> **Interim state, stated plainly: artifact semantic search stays broken until Task 8.** It
> was already broken before this task — Task 6 re-keyed the queue to chunk ids and
> `find_by_ids_filtered` hydrates against the `artifact` table — and Task 7 moves the data
> to the right table without closing the loop. Writers now target v2, `knn` still reads v1,
> and hydration still cannot map a chunk id to its artifact. **Task 8 closes all three.**
>
> **A guard no gate compiles.** The Qdrant guard and its test are `cfg(feature = "server-stack")`,
> which is not in `default`, so **no lane of the four-command gate builds either one** — not
> the lean lane, not the default lane, and not `clippy --features local-embed` (which adds to
> default rather than replacing it). Verified separately: `cargo test --features server-stack
> --lib librarian::artifact_store` → 6 passed. Recorded rather than fixed, because widening
> the gate is a change to a sentence `CLAUDE.md` pins byte-for-byte.

**AMENDED 2026-09-02 — this task originally produced `write_embeddings_v2` and
never called it.** Verified: the only production call of `write_embeddings_v2`
anywhere in this plan was Task 11's one-shot backfill, and Task 11 is blocked on
prerequisite P1. Meanwhile Task 6 already re-keyed the embed queue to chunk ids,
and every live consumer still writes them into the **artifact-keyed** `artifact_vec`.
That is not an interim window between Tasks 6 and 8 — as written it was the
plan's permanent end state. Re-pointing the consumers is now part of this task.

**Line numbers below are as-of 2026-09-02 and Task 6 has already moved some of
them. Resolve each site by SYMBOL, not by line — `symbols(name=…, include_body=true)`
— and treat a mismatch as a signal the substrate moved, not a rounding error.**

**Files:**
- Modify: `src/librarian/indexer.rs` — `write_embeddings`, `write_embeddings_with`
  (~`:502-608`); **and the embed-flush consumer, which is `index_repo`
  (`:676-727`) — NOT `index_repo_sync`.** An earlier draft of this amendment said
  `index_repo_sync`; that is wrong and was corrected by reading the symbol table.
  `index_repo_sync` spans `:115-388` and contains no `write_embeddings` call at
  all. `index_repo` holds **two** of them, `:708` and `:721`, and they are the
  same shape — the `else` arm of `if let Some(s) = store { s.upsert(…) }`, once
  for the every-100 batch flush and once for the trailing partial batch. Change
  both or the bug survives in whichever you miss, silently, for any repo whose
  artifact count is not a multiple of 100.
- Also modify: `src/librarian/indexer.rs` — `embed_queue_items` (`:70-102`) must
  become `pub(crate)`, so Step 1b's test can reach it from `catalog::find`'s test
  module. This is a visibility change, not a behaviour one, and it is the only
  production edit Step 1b needs.
- Modify: `src/librarian/artifact_store.rs` — `SqliteVecArtifactStore::upsert`
  (~`:211`) and `::delete` (~`:219`); **and a guard on `QdrantArtifactStore::upsert`
  (~`:161`)**, per Step 3b
- **`src/librarian/catalog/gc.rs` — NO CHANGE. Scouted 2026-09-02 before implementing; the plan's instruction here was wrong in two independent ways.** See the correction block in § *Step 3*.
- Read-only, but confirm no change is needed: `src/librarian/tools/reindex.rs`
  (~`:359-376`). It reaches the vector store through the **trait**, so fixing
  `SqliteVecArtifactStore` fixes it. Verify this rather than assuming it —
  if it grew a direct `write_embeddings` call, it is a fourth site.
- Test: all three files' test modules, plus the cross-seam test in Step 1b

**Interfaces:**
- Consumes: `EmbedQueueItem` keyed by chunk id (Task 6)
- Produces: `pub fn write_embeddings_v2(cat: &Catalog, embeddings: &[(String, Vec<f32>)]) -> Result<()>` — writes to `artifact_vec_v2`, same dim guard as `write_embeddings_with`
- Produces: `pub fn delete_chunk_vectors(cat: &Catalog, artifact_id: &str) -> Result<usize>`
- **Re-points:** every production writer of artifact embeddings onto
  `artifact_vec_v2`. After this task, `grep` for `write_embeddings(` and
  `artifact_vec` must return only v1-migration and v1-test call sites — a
  production write to `artifact_vec` is a defect, not a leftover.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn deleting_an_artifact_removes_all_its_chunk_vectors_not_only_the_first() {
    // Mutation target: `WHERE chunk_ix = 0`. The FK cascade empties
    // artifact_chunk, so the vector delete must read it FIRST.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let rows = chunk::build_chunks("a", "# T\n\n## W-1 — x\n\na\n\n## W-2 — y\n\nb\n", 2048);
    let stored = chunk::replace_chunks(&cat, "a", &rows).unwrap();
    assert!(stored.len() >= 3, "fixture must have >1 chunk or this proves nothing");
    let vecs: Vec<(String, Vec<f32>)> =
        stored.iter().map(|r| (r.chunk_id.clone(), vec![0.5f32; 768])).collect();
    write_embeddings_v2(&cat, &vecs).unwrap();

    delete_chunk_vectors(&cat, "a").unwrap();
    let n: i64 = cat.conn
        .query_row("SELECT COUNT(*) FROM artifact_vec_v2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "every chunk vector must go, not just chunk_ix 0");
}

#[test]
fn write_embeddings_v2_still_refuses_a_dim_mismatch() {
    let cat = Catalog::open_in_memory().unwrap();
    write_embeddings_v2(&cat, &[("c1".into(), vec![0.1f32; 768])]).unwrap();
    let err = write_embeddings_v2(&cat, &[("c2".into(), vec![0.1f32; 384])]).unwrap_err();
    assert!(err.to_string().contains("dim"), "got: {err}");
}
```

- [x] **Step 1b: The cross-seam test — RETARGETED 2026-09-02**

> **CORRECTION, found by scouting before implementing: the test below cannot go green in Task 7, and this step asserted that it would.** It requires a chunk id to *hydrate through `semantic_find`*. Hydration means mapping a chunk id to its artifact, and `src/librarian/catalog/find.rs` contains **zero** references to `artifact_chunk` — `find_by_ids_filtered` looks candidate ids up in the `artifact` table directly. **Task 8 owns that work by name** (`semantic_find`, `SemanticHit`, and `knn` reading `artifact_vec_v2`), so no edit available inside Task 7 turns this red green.
>
> Worse, the test never touches the writers Step 3a re-points — it upserts straight into `InMemoryArtifactStore` — so **Step 3a is not even on its path.** Left in place it would block Step 6's gate, which requires a green `cargo test --workspace`.
>
> **So the test below MOVES to Task 8 Step 1**, where hydration lands and it can go green. Task 7 keeps a cross-seam test of its own, specified after it — one that goes red on *this* task's defect.
>
> *Authored in `8b48b8f1` earlier the same day by the session that then found this. That amendment was right that Task 7 would otherwise ship dead code, and wrong about which step proves it — which is the argument for scouting a plan you wrote yourself.*

**Why this step exists.** Every existing `semantic_find` test (`find.rs:421`,
`:444`, `:795`) hand-feeds the vector store an **artifact** id:
`store.upsert("proj", "a", …)`. So all three stayed green through Task 6, which
re-keyed the embed queue to **chunk** ids and broke hydration end-to-end. The
defect lives in the seam between the production *writer* and the production
*reader*, and no test in the tree crosses it. That is this project's own law —
*mutate the PRODUCTION path, not the test's inputs* — and the tests above are the
second level asserting about their own re-implementation.

**The load-bearing property: no id in this test is written by hand.** Every id
comes out of `embed_queue_items`. Replace `queue[0].0` with a literal and the
test silently rejoins the population that missed the bug. Annotate that on the
fixture line, per § *Testing Discipline*.

Add to `src/librarian/catalog/find.rs`'s test module:

```rust
#[tokio::test]
async fn an_id_from_the_production_embed_queue_hydrates_through_semantic_find() {
    // LOAD-BEARING: the ids fed to the store MUST come from embed_queue_items,
    // never from a literal. Every other semantic_find test here hand-feeds an
    // artifact id, which is exactly why all of them stayed green when Task 6
    // re-keyed the queue to chunk ids and hydration broke outright.
    use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
    use crate::librarian::artifact_store::ArtifactVectorStore;

    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();

    let queue = crate::librarian::indexer::embed_queue_items(
        &cat,
        "a",
        Some("T".into()),
        "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
    )
    .unwrap();
    // Without >1 chunk the grain bug is UNREPRESENTABLE by this fixture: a
    // single-chunk artifact's chunk id and artifact id fail in the same way,
    // so the test would pass under both the broken and the fixed writer.
    assert!(queue.len() > 1, "fixture must yield >1 chunk, got {}", queue.len());

    let store = InMemoryArtifactStore::default();
    for (id, _, _) in &queue {
        store.upsert("proj", id, &[1.0, 0.0]).await.unwrap();
    }

    let cat = parking_lot::Mutex::new(cat);
    let page = semantic_find(&store, &cat, Some("proj"), &[1.0, 0.0], None, 10, 0, 0)
        .await
        .unwrap();
    assert_eq!(
        page.hits.iter().map(|h| h.row.id.as_str()).collect::<Vec<_>>(),
        vec!["a"],
        "a chunk id from the production queue must hydrate to its ARTIFACT, exactly once"
    );
    // The widening loop is the second half of the symptom: on the broken path
    // `enough` is false and `store_exhausted` is false, so semantic_find climbs
    // toward K_CAP=2000 and re-queries several times before returning empty.
    // An empty page is the visible failure; the burn is the expensive one.
    assert_eq!(page.widenings, 0, "hydration must not need to widen");
}
```

**↑ That test belongs to Task 8 Step 1. Below is Task 7's own cross-seam test.**

The seam Task 7 owns is **production writer → storage table**, not writer → reader. Add to `src/librarian/artifact_store.rs`'s test module:

```rust
#[tokio::test]
async fn the_sqlite_store_writes_a_chunk_id_into_v2_and_never_into_v1() {
    // LOAD-BEARING: this goes through SqliteVecArtifactStore::upsert — the
    // PRODUCTION writer, Step 3a site 3 — and NOT through write_embeddings_v2
    // directly. A test that calls the new function proves the function works
    // and says nothing about whether anything calls it, which is exactly how
    // this task would have shipped write_embeddings_v2 as dead code.
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
    let queue = crate::librarian::indexer::embed_queue_items(
        &cat, "a", Some("T".into()),
        "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n").unwrap();
    // Without >1 chunk the grain bug is UNREPRESENTABLE by this fixture: a
    // single-chunk artifact's chunk id and artifact id fail the same way.
    assert!(queue.len() > 1, "fixture must yield >1 chunk, got {}", queue.len());

    let cat = std::sync::Arc::new(parking_lot::Mutex::new(cat));
    let store = SqliteVecArtifactStore::new(cat.clone());
    for (id, _, _) in &queue {
        store.upsert("proj", id, &vec![0.5f32; 768]).await.unwrap();
    }

    let cat = cat.lock();
    for (id, _, _) in &queue {
        let n: i64 = cat.conn.query_row(
            "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "chunk {id} must reach artifact_vec_v2");
    }
    // BOTH halves are required. The first is monotone under a writer that
    // writes v2 AND v1 — which is exactly what a half-finished re-point looks
    // like, and is a state this task passes through.
    let v1: i64 = cat.conn
        .query_row("SELECT COUNT(*) FROM artifact_vec", [], |r| r.get(0)).unwrap();
    assert_eq!(v1, 0, "no chunk id may reach the artifact-keyed v1 table");
}
```

**Expected RED before Step 3a:** the v1 count is non-zero, because `SqliteVecArtifactStore::upsert` still delegates to `write_embeddings`. That red names *this task's* defect, and Step 3a is what turns it green — which is the property the relocated test did not have.

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib deleting_an_artifact_removes_all_its_chunk_vectors`
Expected: FAIL — `cannot find function write_embeddings_v2`

Run: `cargo test --lib an_id_from_the_production_embed_queue_hydrates`
Expected: FAIL — the page is empty, because the chunk ids were written to
`artifact_vec` (v1) and `find_by_ids_filtered` hydrates against the `artifact`
table. **Record the observed failure text.** "Empty page" and "function not
found" are different reds; only the first one is evidence about this defect.

- [x] **Step 3: Implement**

Add next to `write_embeddings_with` in `indexer.rs`:

```rust
/// Write pre-computed chunk vectors into `artifact_vec_v2`.
///
/// Mirrors [`write_embeddings_with`]'s dim guard: a mismatch against existing
/// rows is a loud, safe stop rather than a silent partial write.
pub fn write_embeddings_v2(cat: &Catalog, embeddings: &[(String, Vec<f32>)]) -> Result<()> {
    if embeddings.is_empty() {
        return Ok(());
    }
    let batch_dim = embeddings[0].1.len();
    if batch_dim == 0 {
        anyhow::bail!("embedding dim is 0 — embedder produced an empty vector");
    }
    let existing_dim: Option<i64> = cat
        .conn
        .query_row("SELECT length(embedding) FROM artifact_vec_v2 LIMIT 1", [], |r| r.get(0))
        .optional()?;
    if let Some(bytes) = existing_dim {
        let stored = (bytes / 4) as usize;
        if stored != batch_dim {
            anyhow::bail!(
                "embedding dim mismatch: artifact_vec_v2 holds {stored}, batch has {batch_dim}"
            );
        }
    }
    let mut del = cat.conn.prepare("DELETE FROM artifact_vec_v2 WHERE id = ?1")?;
    let mut ins = cat
        .conn
        .prepare("INSERT INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)")?;
    for (chunk_id, vec) in embeddings {
        let blob: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
        del.execute([chunk_id])?;
        ins.execute(rusqlite::params![chunk_id, blob])?;
    }
    Ok(())
}

/// Delete every chunk vector belonging to `artifact_id`.
///
/// MUST run BEFORE the artifact row is deleted: `artifact_chunk` has
/// `ON DELETE CASCADE`, so once the artifact is gone the chunk ids that name
/// these vectors are gone too and the vectors are unreachable orphans. vec0
/// has no FK, so nothing else would ever collect them.
pub fn delete_chunk_vectors(cat: &Catalog, artifact_id: &str) -> Result<usize> {
    let ids: Vec<String> = {
        let mut stmt = cat
            .conn
            .prepare("SELECT chunk_id FROM artifact_chunk WHERE artifact_id = ?1")?;
        stmt.query_map([artifact_id], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    };
    let mut del = cat.conn.prepare("DELETE FROM artifact_vec_v2 WHERE id = ?1")?;
    for id in &ids {
        del.execute([id])?;
    }
    Ok(ids.len())
}
```

> **CORRECTION 2026-09-02, found by scouting before implementing — the `gc.rs` instruction was wrong twice, independently.** It read: *"In `gc.rs`, call `delete_chunk_vectors(cat, id)?` immediately before every `DELETE FROM artifact WHERE id = …`, at the site already commented … (`gc.rs:482`)."*
>
> **(1) The delete is already handled, so the call would be redundant.** `delete_chunk_vectors`'s own doc-comment justifies itself with *"vec0 has no FK, so nothing else would ever collect them."* That was true when this plan was written, and **Task 4 falsified it.** Verified at the bytes: `artifact_chunk.artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE` (`catalog/mod.rs:262`) → `CREATE TRIGGER artifact_vec_v2_cascade_delete AFTER DELETE ON artifact_chunk BEGIN DELETE FROM artifact_vec_v2 WHERE id = OLD.chunk_id; END` (`:294-298`) → `PRAGMA foreign_keys = ON` on all three `Catalog` constructors (`:481`, `:497`, `:524`). The trigger's own comment records that cascade-fires-trigger was *verified, not assumed*, under `recursive_triggers` both OFF and ON, and `deleting_an_artifact_cascades_its_vec_v2_row_via_the_chunk_trigger` (`:771`) pins it.
>
> **(2) `gc.rs:482` is not a delete site at all.** It sits inside `apply_rehome`, which **changes** an artifact id rather than deleting it — `migrate_vec_id` exists only because vec0 rejects `UPDATE … SET id`. Grepping `DELETE FROM artifact WHERE id` across `gc.rs` matches **that comment and no statement**. And rehome needs no v2 work *by design*: `artifact_vec_v2` is keyed by `chunk_id`, which does not change when the artifact id does — given as the reason for that key at `catalog/mod.rs:257-258`.
>
> **So `delete_chunk_vectors` survives for exactly ONE caller:** `SqliteVecArtifactStore::delete`, which removes an artifact's vectors **without deleting the artifact row**, so no `artifact_chunk` delete occurs and the trigger is unavailable. Keep the function and its Step 1 test; drop the `gc.rs` edit entirely. **Rewrite its doc-comment** — the hazard is not *"nothing would collect them"* but *"this path deletes vectors without deleting chunks, so the trigger cannot fire here."* A doc-comment stating a falsified rationale is worse than none: it is the thing the next reader checks against.
>
> **A live bug was found beside this and is filed, NOT fixed here.** `apply_rehome` updates `artifact.id` and never touches `artifact_chunk.artifact_id` (zero `artifact_chunk` references in `gc.rs`); the FK carries no `ON UPDATE` clause, so it is `NO ACTION`, and `PRAGMA defer_foreign_keys = ON` (`gc.rs:453`) defers that check to COMMIT rather than skipping it. Out of scope for Task 7.

- [x] **Step 3a: Re-point the production writers — the step the original plan omitted**

Without this, `write_embeddings_v2` is dead code and the branch ships the
defect. There are **three** production sites that write a chunk id into the
artifact-keyed v1 table, reached by two different routes:

| # | Site | Route | Change |
|---|---|---|---|
| 1 | `indexer.rs:708` — `index_repo`, every-100 batch flush | direct, when `store` is `None` | `write_embeddings` → `write_embeddings_v2` |
| 2 | `indexer.rs:721` — `index_repo`, trailing partial batch | direct, when `store` is `None` | same |
| 3 | `artifact_store.rs:216` — `SqliteVecArtifactStore::upsert` | through the trait, when `store` is `Some(sqlite-vec)` | same |

`reindex.rs:359-376` is **read-only for this task and the plan's note about it is
correct** — verified: it reaches the store only through `store.upsert(…)` at
`:368` and has no `write_embeddings` fallback at all, so fixing site 3 fixes it.
Confirm this by reading the symbol rather than trusting this line; if it has
grown a direct call, it is a fourth site.

`SqliteVecArtifactStore::delete` (`:219`) must delete from `artifact_vec_v2`
too. It is not on the write path, so nothing reds when it is missed — the cost
is an accumulating orphan set, which is the same failure mode this task's
doc-comment on `delete_chunk_vectors` describes.

**Exit criterion, checkable in one command:** `grep` for `write_embeddings(` and
for `artifact_vec\b` across `src/` must return only v1-migration paths
(`write_embeddings_with`, `rebuild_artifact_vec_at_dim`, `gc.rs`'s
`migrate_vec_id`) and test modules. A production write to `artifact_vec` after
this step is a defect, not a leftover.

- [x] **Step 3b: Guard `QdrantArtifactStore::upsert` — refuse a grain it cannot honour**

Chunk-grain Qdrant is **deferred** (see § *Deferred*), and deferral is only
honest if the deferred path *refuses* the input it cannot handle. Today it
accepts it: `ArtifactVectorStore::upsert` carries exactly one id, so a chunk id
reaches `QdrantArtifactStore::upsert` (`:161`) and is written as **both** the
point id and the payload's claimed `artifact_id`
(`src/retrieval/artifact.rs:85` and `:89`). Nothing downstream can distinguish
that from a real artifact id. There is no error, no log line, and no observer —
which is § *Observer Blindness*'s test for a defect that care will not catch.

The two id spaces are **shape-distinguishable**, so the guard is exact rather
than a heuristic. Both halves verified at their mint sites:

- **artifact id** — `src/librarian/ids.rs:17-23`, `artifact_id_from_abs` =
  `sha256(abs_path)` hex `[..16]`: exactly 16 lowercase hex chars.
  Independently encoded as `\b[0-9a-f]{16}\b` by the citation extractor
  (`link_scan/extract.rs:258-262`), whose doc-comment states the same invariant.
  Two instruments of *different kinds* — a mint and a parser — so this is
  corroboration rather than one blind spot counted twice.
- **chunk id** — `src/librarian/catalog/chunk.rs:136` and `:141`,
  `uuid::Uuid::new_v4().to_string()`: 36 chars, four dashes. It cannot satisfy
  the artifact-id predicate.

Add at the top of `QdrantArtifactStore::upsert`, before `self.ensure(…)`:

```rust
// Chunk-grain Qdrant is deferred (see the plan's § Deferred). Deferral only
// holds if this path REFUSES the grain it cannot represent: a chunk id here
// becomes the point id AND the payload's claimed `artifact_id`
// (retrieval/artifact.rs:85,89), indistinguishable downstream from a real one.
// Artifact ids are sha256(abs_path)[..16] — 16 lowercase hex (librarian/ids.rs:17).
// Chunk ids are UUID v4 (catalog/chunk.rs:136). Refuse the shape.
if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
    anyhow::bail!(
        "QdrantArtifactStore is artifact-grain and was handed a non-artifact id {id:?}. \
         Chunk-grain retrieval is implemented on the sqlite-vec backend only; \
         set the artifact backend to sqlite-vec, or implement chunk-grain Qdrant."
    );
}
```

**Name the observer, or this guard is decoration** (§ *Testing Discipline*: an
alarm nothing reaches is exactly as informative as no alarm). The reaching caller
is `index_repo`'s `s.upsert(project_id, id, vec)` at `:705` and `:718` whenever
`ArtifactBackend::resolve` returns Qdrant — **which is the default on the server
build** (`artifact_store.rs:42`), so this fires for anyone running the shipped
binary against a re-indexed corpus. The error surfaces through
`index_repo`'s `?` into the reindex report. Write the test with that caller in
mind, not with a direct call to `upsert`.

Test it against the real id shapes, both directions:

```rust
#[tokio::test]
async fn qdrant_store_refuses_a_chunk_id_and_accepts_an_artifact_id() {
    // Both fixtures are REAL shapes from their mint sites, not hand-typed
    // look-alikes: change either constructor and this test must be re-derived.
    let chunk_like = uuid::Uuid::new_v4().to_string();
    let artifact_like = crate::librarian::ids::artifact_id_from_abs(
        std::path::Path::new("/test/a.md"),
    );
    assert_eq!(artifact_like.len(), 16, "artifact id shape moved — re-derive the guard");
    // …assert upsert(chunk_like) is Err and names the grain; upsert(artifact_like)
    // reaches `ensure` (a connection error is a PASS here — it means the guard
    // let it through; only a grain-refusal error is a FAIL).
}
```

That last parenthesis is the load-bearing half: without it the accept-direction
assertion is satisfied by *any* error, including the guard wrongly firing, and
the test is monotone in the direction it exists to check.

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib librarian::indexer ; cargo test --lib librarian::catalog::gc`
Expected: PASS.

- [x] **Step 5: Mutation check**

Change `delete_chunk_vectors`'s query to `... WHERE artifact_id = ?1 AND chunk_ix = 0`.
**Expected: `deleting_an_artifact_removes_all_its_chunk_vectors_not_only_the_first` FAILS.** Revert.

- [x] **Step 6: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/indexer.rs src/librarian/catalog/gc.rs
git -C /home/marius/work/claude/codescout commit -m "feat(librarian): artifact_vec_v2 writes + per-artifact chunk-vector cascade"
```

---

## Task 8: `semantic_find` returns chunk hits, capped per artifact

> **DONE 2026-09-02** by session `ffb95976`. **This is the task that closes the outage** —
> writers (Task 7) and reader now name the same table, and hydration maps a chunk id back to
> its artifact. Mutation run and **both** predicted tests killed: disabling the cap gave
> `capped at 3, got 6` and `must yield distinct artifacts` (7 hits collapsing to 2 distinct
> artifacts — exactly the regression the distinctness assertion exists for). Reverted.
>
> **Four ways the task as written did not survive contact with the code.**
>
> **1. `ids.dedup()` is wrong and would have hydrated duplicates.** `dedup()` collapses only
> *adjacent* duplicates, and two ledgers' chunks interleave in KNN order as a matter of
> course — which is the normal case, not an edge one. Replaced with a first-appearance
> `HashSet` filter, preserving best-chunk order.
>
> **2. "update their call sites to pass `max_per_artifact`" is not enough for the three
> existing `semantic_find` tests.** They hand-feed **artifact** ids to the store, so after
> this change every candidate is unresolvable and the page comes back empty. They needed
> chunk rows, via a new `one_chunk` helper. Note the failure direction: an unresolvable
> candidate is skipped as *stale* rather than erroring, so a test left in the old shape goes
> **silently empty** rather than red-with-a-reason.
>
> **3. Raising the `k` floor to 200 silently invalidated an annotated load-bearing fixture.**
> `a_filter_that_excludes_the_nearest_matches_reports_starvation` seeded 150 rows *because*
> the old floor was 100, and said so in its doc comment. At the new floor the store returns
> fewer rows than `k`, `store_exhausted` fires on the first pass, and the test would have
> gone **green while asserting nothing about starvation**. Seed raised to 250 **and the
> annotation rewritten** — leaving the stale "(100)" would have been the exact silent decay
> § *Testing Discipline* warns about.
>
> **4. A second seeding site the Files list does not name.** `tools/find.rs`'s `seed_vec`
> helper wrote into `artifact_vec` keyed by artifact id, plus one test that hand-rolled the
> same INSERT twice. Both re-pointed; the hand-rolled pair now routes through `seed_vec`, so
> the next grain change has one site rather than three.
>
> **A test the plan does not have, and it is the one that pins the outage.**
> `the_real_sqlite_path_writes_and_reads_the_same_table` drives the production writer
> (`SqliteVecArtifactStore::upsert`) and the production reader (`knn`) and names **no table
> at all**. Every other test here supplies its own `InMemoryArtifactStore`, which covers
> hydration and is structurally blind to writer-and-reader-on-different-tables — the actual
> defect between Tasks 7 and 8. A test that supplies the store cannot see which table the
> store uses.
>
> **Both production callers pass `max_per_artifact = 1`**, which *preserves* today's result
> shape rather than changing it: the store was artifact-keyed until Task 7, so every artifact
> appeared at most once. `context.rs` gets 1 because its value is breadth across artifacts
> (Task 9 formalises that contract); `tools/find.rs` gets 1 until Task 10 decides whether
> `artifact(find, semantic=)` should surface several chunks per artifact.

**Files:**
- Modify: `src/librarian/catalog/find.rs:242-245` (`SemanticHit`), `:257-265` (`SemanticPage`), `:283-357` (`semantic_find`)
- Modify: `src/librarian/artifact_store.rs:228-252` (`knn` reads `artifact_vec_v2`)
- Test: `src/librarian/catalog/find.rs` test module

**Interfaces:**
- Consumes: `chunk::chunks_for` (Task 5)
- Produces:
  ```rust
  pub struct ChunkHit { pub chunk_id: String, pub chunk_ix: usize,
                        pub start_line: usize, pub end_line: usize,
                        pub entry_token: Option<String>, pub content: String }
  pub struct SemanticHit { pub row: ArtifactRow, pub distance: f32, pub chunk: Option<ChunkHit> }
  ```
  `SemanticPage` gains `pub cap_suppressed: usize`.
  `semantic_find(..., max_per_artifact: usize, ...)` — a new parameter placed after `filter`.

- [x] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn a_hit_names_the_chunk_that_matched_not_the_preamble() {
    // The whole point of the plan, at the retrieval layer.
    let (cat, store) = fixture_with_two_entry_artifact().await;
    let page = semantic_find(&store, &cat, None, &query_vec_for("beta"), None, 3, 10, 0, 0)
        .await.unwrap();
    let hit = &page.hits[0];
    let chunk = hit.chunk.as_ref().expect("chunk-grain hit");
    assert_eq!(chunk.entry_token.as_deref(), Some("W-2"));
    assert!(chunk.start_line > 1, "must not be the preamble chunk");
    assert!(chunk.content.contains("beta"));
}

#[tokio::test]
async fn max_per_artifact_caps_without_emptying_the_page() {
    // Absence assertion — a cap that drops EVERYTHING also satisfies "no more
    // than 3". All three clauses are required.
    let (cat, store) = fixture_with_one_big_and_one_small_artifact().await;
    let page = semantic_find(&store, &cat, None, &q(), None, 3, 10, 0, 0).await.unwrap();
    let from_big = page.hits.iter().filter(|h| h.row.id == "big").count();
    assert_eq!(from_big, 3, "capped at 3");
    assert!(page.cap_suppressed > 0, "and it reports what it suppressed");
    assert!(page.hits.iter().any(|h| h.row.id == "small"),
            "a lower-ranked chunk from another artifact must still make the page — \
             without this clause a cap that drops everything passes");
}

#[tokio::test]
async fn max_per_artifact_one_yields_distinct_artifacts() {
    // context.rs's contract. Assert DISTINCTNESS: a count of N is satisfied by
    // N chunks of one ledger, which is the regression this prevents.
    let (cat, store) = fixture_with_one_big_and_one_small_artifact().await;
    let page = semantic_find(&store, &cat, None, &q(), None, 1, 10, 0, 0).await.unwrap();
    let mut ids: Vec<&str> = page.hits.iter().map(|h| h.row.id.as_str()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), before, "every hit must be a distinct artifact");
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test --lib catalog::find::tests::a_hit_names_the_chunk`
Expected: FAIL — `this function takes 8 arguments but 9 were supplied`

- [x] **Step 3: Implement**

In `artifact_store.rs`, point `knn`'s SQL at the new table:

```rust
        let mut stmt = cat.conn.prepare(
            "SELECT id, distance FROM artifact_vec_v2 WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2",
        )?;
```

In `find.rs`, extend the types and the loop. The candidate ids returned by `knn` are now **chunk ids**, so map them to artifact ids before hydrating:

```rust
/// The chunk that matched, when the store is chunk-keyed.
#[derive(Debug, Clone)]
pub struct ChunkHit {
    pub chunk_id: String,
    pub chunk_ix: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub entry_token: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct SemanticHit {
    pub row: ArtifactRow,
    pub distance: f32,
    pub chunk: Option<ChunkHit>,
}
```

Inside the loop, replace the `by_id` construction and the hit-building block:

```rust
        // knn returns CHUNK ids. Resolve each to its artifact so the catalog
        // filter — which is artifact-level — still applies, and keep the
        // chunk alongside so the hit can name the entry that matched.
        let chunk_rows = {
            let cat = catalog.lock();
            crate::librarian::catalog::chunk::rows_by_chunk_ids(
                &cat,
                &candidates.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            )?
        };

        // KNN order is the ranking; preserve it, and apply the per-artifact cap
        // in that order so the cap keeps each artifact's BEST chunks.
        let mut seen_per_artifact: std::collections::HashMap<String, usize> = HashMap::new();
        let mut ordered: Vec<(ChunkHit, String, f32)> = Vec::new();
        let mut cap_suppressed = 0usize;
        for (chunk_id, distance) in &candidates {
            let Some(row) = chunk_rows.get(chunk_id) else { continue };
            let n = seen_per_artifact.entry(row.artifact_id.clone()).or_insert(0);
            if *n >= max_per_artifact {
                cap_suppressed += 1;
                continue;
            }
            *n += 1;
            ordered.push((
                ChunkHit {
                    chunk_id: row.chunk_id.clone(),
                    chunk_ix: row.chunk_ix,
                    start_line: row.start_line,
                    end_line: row.end_line,
                    entry_token: row.entry_token.clone(),
                    content: row.content.clone(),
                },
                row.artifact_id.clone(),
                *distance,
            ));
        }

        let candidate_ids: Vec<String> = {
            let mut ids: Vec<String> = ordered.iter().map(|(_, a, _)| a.clone()).collect();
            ids.dedup();
            ids
        };
```

Hydrate + filter on `candidate_ids` exactly as before, then build hits by walking `ordered` and keeping those whose artifact survived the filter. `target` counts **surviving hits after the cap**, not raw candidates.

Add to `chunk.rs`:

```rust
/// Chunk rows for a set of chunk ids, keyed by `chunk_id`. Ids with no row are
/// simply absent — a vector whose chunk row is gone is stale, not an error.
pub fn rows_by_chunk_ids(
    cat: &Catalog,
    chunk_ids: &[String],
) -> Result<std::collections::HashMap<String, ChunkRow>> {
    if chunk_ids.is_empty() {
        return Ok(Default::default());
    }
    let placeholders = std::iter::repeat_n("?", chunk_ids.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token,
                content, content_hash
           FROM artifact_chunk WHERE chunk_id IN ({placeholders})"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(chunk_ids.iter()), |r| {
            Ok(ChunkRow {
                chunk_id: r.get(0)?, artifact_id: r.get(1)?,
                chunk_ix: r.get::<_, i64>(2)? as usize,
                start_line: r.get::<_, i64>(3)? as usize,
                end_line: r.get::<_, i64>(4)? as usize,
                entry_token: r.get(5)?, content: r.get(6)?, content_hash: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows.into_iter().map(|r| (r.chunk_id.clone(), r)).collect())
}
```

Add `cap_suppressed: usize` to `SemanticPage` with this doc:

```rust
    /// Hits dropped by `max_per_artifact`. Distinct from `exhausted`: the page
    /// is full and relevant, but one artifact had more to say than it was
    /// allowed. A caller that cannot see this cannot tell a capped page from a
    /// corpus that simply had nothing else — the same silent-partial defect
    /// this whole change exists to fix, one level up.
    pub cap_suppressed: usize,
```

Raise the widening constants — with a cap, candidates collapse before counting:

```rust
    let mut k = (target * 5 * max_per_artifact.max(1)).max(200);
    const K_CAP: usize = 8000;
```

- [x] **Step 4: Run to verify they pass**

Run: `cargo test --lib catalog::find`
Expected: PASS, including the pre-existing `semantic_find` tests (update their call sites to pass `max_per_artifact`).

- [x] **Step 5: Mutation check**

Replace the cap check with `if false {`.
**Expected: `max_per_artifact_caps_without_emptying_the_page` AND `max_per_artifact_one_yields_distinct_artifacts` both FAIL.** Revert.

- [x] **Step 6: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/catalog/find.rs src/librarian/catalog/chunk.rs src/librarian/artifact_store.rs
git -C /home/marius/work/claude/codescout commit -m "feat(librarian): chunk-grain semantic_find with max_per_artifact"
```

---

## Task 9: `librarian(context)` keeps artifact grain

> **DONE 2026-09-02** by session `ffb95976`. Commit `e67c3221`. **Step 3 was already in the
> tree** — Task 8 (`9e2b93d2`) landed the `max_per_artifact = 1` at this call site, so Task 9
> reduced to writing its guard. Writing the guard is what found the problem.
>
> **The test as specified would have passed with the cap removed**, in three independent
> ways, and only the third survives careful transcription.
>
> **1. It reads a field the tool does not emit.** `out["candidate_ids"]` — `context` returns
> `included_ids`. `.as_array().unwrap()` on `None` panics, so this one at least is loud.
>
> **2. `fixture_with_one_ledger_of_many_chunks()` and `run_context()` do not exist**, and no
> fixture in this module supplies an embedder or a store: `TestToolContextBuilder` has both
> setters, and every one of the 31 pre-existing context tests leaves both `None`.
>
> **3. The assertion cannot fail — this is the one a careful transcription still ships.**
> With no embedder or no store, `context`'s topic branch falls back to a `title|topic
> contains` filter. That fallback is **artifact-grain**, so it satisfies a distinctness
> assertion while exercising none of the code under test. Distinctness is monotone under
> narrowing besides: an empty page satisfies it perfectly. The plan's test would have been
> green on a tree with the cap deleted.
>
> **So the discriminator has to live in the FIXTURE, not the assertion.** The topic string
> here is a substring of no artifact's title or topic, so the fallback returns **zero** rows —
> which turns a skipped semantic path into a red on the "note is present" assertion rather
> than a pass in silence.
>
> **Two assertions, two disjoint mutation kills, both observed.** At `max_per_artifact = 2`
> the note still fits and only distinctness fires: `["r/ledger.md", "r/ledger.md",
> "r/note.md"]`. At `51` the ledger's 61 chunks fill every candidate slot and only the note's
> absence fires — `included_ids` came back as **50 copies of one artifact**, the entire token
> budget spent on one body, while `candidates_capped` reported `capped`. A plausible answer,
> not an error. Neither assertion subsumes the other, and the docstring says so, because
> trimming either one as redundant halves the guard silently.
>
> **Also shipped:** `realign_abs_paths`, hoisted out of `mk_ctx` so a store-backed fixture can
> reuse it. The packing loop reads every candidate body off **disk** and `continue`s past a
> path it cannot open, so an unrealigned row is skipped in silence — indistinguishable from
> "the search matched nothing".

**Files:**
- Modify: `src/librarian/tools/context.rs:679-695`
- Test: `src/librarian/tools/context.rs` test module

**Interfaces:**
- Consumes: `semantic_find(..., max_per_artifact, ...)` (Task 8)

- [x] **Step 1: Write the failing test** — rewritten; see the block above for why the
      specified one could not fail.

```rust
#[tokio::test]
async fn context_candidates_are_distinct_artifacts_after_the_chunk_change() {
    // context ranks by the link graph and only needs ids. Before the cap
    // existed, 51 chunk hits could be 8 artifacts — a bundle that packed 50
    // candidates would pack 8, with candidates_capped still reporting "capped".
    let ctx = fixture_with_one_ledger_of_many_chunks().await;
    let out = run_context(&ctx, json!({"topic": "gate latency", "scope": "project"})).await;
    let ids = out["candidate_ids"].as_array().unwrap();
    let mut seen: Vec<&str> = ids.iter().map(|v| v.as_str().unwrap()).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), before, "context must never receive two chunks of one artifact");
}
```

- [x] **Step 2: Run to verify it fails** — not reachable as written: Step 3 shipped with
      Task 8, so the red was produced by mutating the production line instead (Step 4).

Run: `cargo test --lib context_candidates_are_distinct_artifacts`
Expected: FAIL — duplicate ids, because `semantic_find` now returns chunk hits.

- [x] **Step 3: Implement** — already in the tree from `9e2b93d2`; the call-site comment now
      names the pinning test and both mutation values.

At `context.rs:679`, pass the cap:

```rust
            let page = crate::librarian::catalog::find::semantic_find(
                store.as_ref(),
                &ctx.catalog,
                project_id.as_deref(),
                &vec,
                scoped_filter.as_ref(),
                // 1 = artifact grain. context() ranks by the link graph and needs
                // 50 DISTINCT artifacts; a chunk-grain page would silently hand it
                // 8 artifacts wearing 51 hits, with candidates_capped still true.
                1,
                51,
                0,
                cutoff_ms,
            )
            .await?;
```

- [x] **Step 4: Run to verify it passes** — 32/32 `librarian::tools::context` green, and the
      two mutations above each killed exactly one assertion.

Run: `cargo test --lib librarian::tools::context`
Expected: PASS.

- [x] **Step 5: Gate and commit** — `e67c3221`. fmt/clippy clean; lean 3386 passed / 1 failed
      (a peer mid-write of `issue-clusters.md`), default 5008 passed / 2 failed (a peer's
      committed ceiling correction, and the known-flaky peer idle-timeout test).

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/tools/context.rs
git -C /home/marius/work/claude/codescout commit -m "fix(librarian): context() pins artifact grain with max_per_artifact=1"
```

---

## Task 10: `artifact(find, semantic=)` result shape

> **DONE 2026-09-02** by session `ffb95976`. Commit `95b77262`.
>
> **The Files list was half wrong, and Step 6 would have committed a lie.** It named
> `artifact.rs` for both the response builder and the param description, and staged
> `artifact.rs src/server.rs`. Only the *description* is in `artifact.rs`; the response
> builder is in **`tools/find.rs`**, which that `git add` line does not name. Followed
> literally, Task 10 ships a schema advertising `matched` and a build that never emits it —
> and the two halves of the plan **agree with each other**, so no amount of re-reading the
> plan catches it. Only reading the code does.
>
> **Two departures from the specified code, both in the same direction.**
>
> **1. The snippet had no truncation marker.** `chars().take(480)` is
> `cluster/capped-result-presented-as-complete` by name — a 480-char snippet is
> indistinguishable from a chunk that happens to end there. It now appends
> `… [snippet truncated — read the span with artifact(get)]`, and the test asserts **both**
> directions, because a marker is only informative if its absence is too.
>
> **2. `chunk_by_id` is keyed by ARTIFACT id**, like the `distance_by_id` it sits beside —
> correct only while this caller passes `max_per_artifact = 1`. Raise the cap and a second
> chunk silently overwrites the first; `rows` collapses the same way, being one
> `ArtifactRow` per hit. Annotated at the site, naming who owns the restructure, because the
> failure mode is a wrong `matched` span on a plausible-looking item rather than an error.
>
> **Four tests, four kills, none overlapping.** Hardcoding `matched.start_line` to 1 — an
> artifact-grain answer wearing a chunk-grain shape — gave `left: Number(1), right:
> Number(9)`. Dropping the marker killed the snippet test. `cap_suppressed > 100` killed the
> suppression test. Firing the hint **unconditionally** killed its *control*, which emitted
> `0 further chunk(s) belonging to artifacts already on this page were dropped` — precisely
> the noise that control exists to prevent.
>
> **A wording fix the mutants surfaced.** `semantic_exhausted_hint` said *"this is every
> matching ROW that exists, not a truncated page"* — unambiguous while hits were
> artifact-grain, unit-ambiguous the moment they were not, and now read beside
> `cap_suppressed`, where chunks *were* truncated away. Changed to **ARTIFACT**.
>
> **Budget: 56_497 → 56_735**, exact measured total with a log entry, per the constant's own
> rule. 30 bytes were repaid by tightening the draft first, logged as *prose-golf rather
> than a correctness fix* so a later sweep does not credit it as one of the good ones.

**Files:**
- Modify: `src/librarian/tools/artifact.rs` (the `find` response builder and the `semantic` param description)
- Test: `src/librarian/tools/artifact.rs` test module

**Interfaces:**
- Consumes: `SemanticHit.chunk`, `SemanticPage.cap_suppressed` (Task 8)
- Produces: each `items[]` entry gains `matched: {start_line, end_line, entry_token, snippet}` when the hit is chunk-grain; response gains `hints.cap_suppressed` when non-zero.

- [x] **Step 1: Write the failing test** — four tests, not one: `matched` fields, the
      suppression hint, its silent control, and the truncation marker in both directions.
      `run_artifact` / `fixture_with_two_entry_artifact` do not exist; a `seed_entry_chunks`
      helper does the job in `tools/find.rs`'s own module.

```rust
#[tokio::test]
async fn semantic_find_results_carry_the_matched_span_and_entry() {
    let ctx = fixture_with_two_entry_artifact().await;
    let out = run_artifact(&ctx, json!({"action": "find", "semantic": "beta", "limit": 3})).await;
    let m = &out["items"][0]["matched"];
    assert!(m["start_line"].as_u64().unwrap() > 1, "not the preamble");
    assert_eq!(m["entry_token"], "W-2");
    assert!(m["snippet"].as_str().unwrap().len() <= 480, "snippet is bounded");
}
```

- [x] **Step 2: Run to verify it fails** — done as a mutation run against the production
      path; see the block above for the four kills and their exact messages.

Run: `cargo test --lib semantic_find_results_carry_the_matched_span`
Expected: FAIL — `matched` is null.

- [x] **Step 3: Implement** — in `tools/find.rs`, NOT `artifact.rs`; plus the truncation
      marker the specified snippet lacked.

In the `find` response builder, when `hit.chunk` is `Some`:

```rust
        // Bounded snippet, not the whole chunk: a 2 KB chunk x 10 hits is a
        // 20 KB response. The caller reads the full span with
        // artifact(get, id=…, start_line=…, end_line=…), which is why the range
        // travels alongside. See docs/PROGRESSIVE_DISCLOSURE.md.
        const SNIPPET_CHARS: usize = 480;
        if let Some(chunk) = &hit.chunk {
            let snippet: String = chunk.content.chars().take(SNIPPET_CHARS).collect();
            item["matched"] = json!({
                "start_line": chunk.start_line,
                "end_line": chunk.end_line,
                "entry_token": chunk.entry_token,
                "snippet": snippet,
            });
        }
```

And after the items loop:

```rust
    if page.cap_suppressed > 0 {
        hints["cap_suppressed"] = json!(page.cap_suppressed);
        hints["cap_hint"] = json!(
            "further chunks from artifacts already in this page were suppressed; \
             read a full artifact with artifact(action=\"get\", id=…)"
        );
    }
```

Update the `semantic` param description in the tool schema:

```rust
                    "description": "find: natural-language query for semantic search (requires embedder). Hits are CHUNK-grain: each item carries `matched` with the line range, the enclosing entry token (e.g. `W-81`) and a bounded snippet."
```

- [x] **Step 4: Run to verify it passes** — 39/39 `librarian::tools::find` green.

Run: `cargo test --lib librarian::tools::artifact`
Expected: PASS.

- [x] **Step 5: Check the prompt-surface gates** — both green after the raise;
      `prompt_surfaces_reference_only_real_tools` unaffected.

Run: `cargo test --lib prompt_surfaces_reference_only_real_tools ; cargo test --lib tool_surface_char_budget`
The description grew — if the budget test fails, **raising `TOOL_SURFACE_CHAR_BUDGET` is allowed** (it is a ratchet, not a ceiling). Add a dated log entry stating what bought the bytes.

- [x] **Step 6: Gate and commit** — `95b77262`, and **the staging line here is wrong**: it
      omits `src/librarian/tools/find.rs`, where the implementation lives. Committed by
      pathspec over all three paths, a peer holding ~20 staged paths in the shared index.

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/tools/artifact.rs src/server.rs
git -C /home/marius/work/claude/codescout commit -m "feat(librarian): semantic find returns matched span, entry token and snippet"
```

---

## Task 11: Backfill and swap

> **PARTIALLY DONE 2026-09-02** by session `ffb95976`. Commits `98eb5adc` (fix (c)) and
> `488192e8` (the backfill). **The swap is deliberately NOT implemented.**
>
> **Fix (c) shipped first, as this task requires.** `IndexReport` gains
> `vectorless: Option<usize>`, surfaced by `reindex` as `vectorless` +
> `vectorless_note`. `Option`, not a bare count, because `0` would read identically for
> "measured, no hole" and "embeddings are off, so the number is noise". It counts **chunk
> rows, not vectors** — a defect caught while writing the second test: `index_repo` writes
> through `store.upsert` when an external store is configured and leaves `artifact_vec_v2`
> empty, so a vector-based count would false-alarm at **100%**. `embed_queue_items` writes
> chunk rows as a side effect of queueing, so "no chunk rows" is the absorbing state's exact
> signature on every backend.
>
> **The backfill's key test asserts the fix does NOT close the trap.** After the backfill
> the indexer still refuses to queue the artifact — without that control, a backfill doing
> what an ordinary reindex would have done looks identical and is not worth a function.
>
> **Three departures from the specified interfaces.** `BackfillReport` gains
> `missing_file`: a file that is *gone* and one that is *blank* have different remedies and
> `skipped_empty` would say the same word about both. The cursor **clears on completion**
> rather than persisting as a watermark — parked at the end of the corpus it would skip any
> artifact that becomes vectorless later with an earlier timestamp, which is exactly what a
> re-formed hole looks like; mutating it to a watermark gives `left: 0, right: 2`, and *0
> artifacts visited* reads as "nothing to do". And it takes the **`Mutex`**, not a
> `&Catalog`, so the lock is released across each embedding await — clippy's
> `await_holding_lock` caught the first shape and the honest fix was the restructure.
>
> **Wired as a CLI (`codescout backfill-chunks`), not an MCP action**, and both halves of
> that are deliberate: an unwired function is `cluster/declared-not-wired`, and a tool
> action would hold the catalog through thousands of remote round-trips, degrading every
> session sharing it.
>
> ⚠ **STEP 5 IS DANGEROUS AS WRITTEN — do not run it.** It sets
> `LIBRARIAN_CATALOG=/tmp/…` to point the binary at a copy. **That variable does not exist
> anywhere in this codebase** (verified by grep 2026-09-02), so the command opens the
> **live** catalog and back-fills it — the exact thing the step's own text forbids, on a
> machine where several sessions share that file. A real dry run needs a catalog argument
> the CLI does not yet take.
>
> **Why the swap was not written, rather than written-and-not-run.** The plan blocks
> *running* it, which would leave a function nobody may call — `cluster/declared-not-wired`,
> the class whose canonical instance in this repo is two tools that implemented `Tool`,
> were registered nowhere, and carried a passing suite for months. Shipping the swap now
> would instantiate it knowingly. It stays for whoever lands fix (a) or (b).
>
> **And the plan and the bug file disagree about whether the backfill is blocked at all.**
> The bug file's Resume says Task 11 *"backfills chunk rows from artifacts the indexer
> declines to process, so running it first would backfill into the hole and report
> success."* That is true of a backfill routed through the indexer and false of this one,
> which consults `content_unchanged` nowhere — resolved on the mechanism, not on which
> document is newer.

**BLOCKED ON P1 — AMENDED 2026-09-02: diagnosed, not fixed.** The vector-coverage hole now has a reproduction and a named mechanism (`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`, artifact `a766aad35b0b7610`); see § *Entry condition*. That changes what the block means rather than lifting it:

- **The backfill itself is unblocked.** It needs the exit from the absorbing state, and the exit is the established half; the entry path is the half still unknown.
- **The swap is not.** Do not run it on a real catalog until fix (a) or (b) from the bug file has landed. `:302` still stamps content as seen before `:309` decides to embed it, so a backfill against an unfixed indexer fills the hole once and the next `librarian(reindex)` starts digging it again — with the freshly written chunk rows now making the affected artifacts *look* covered.
- **Fix (c) is mandatory here, not optional hygiene.** An `IndexReport` count of `want_embeddings && !has_vector` is the only thing that would make a re-formed hole observable; without it the next occurrence is as unreconstructable as this one.

**Files:**
- Modify: `src/librarian/indexer.rs` (add the backfill runner next to `write_embeddings` — **resolve by symbol, not by line**; see the coordinate note in Step 3)
- Test: `src/librarian/indexer.rs` test module

**Interfaces:**
- Produces: `pub async fn backfill_chunk_vectors(cat: &Catalog, svc: &EmbeddingService, batch: usize) -> Result<BackfillReport>` where `BackfillReport { embedded: usize, skipped_empty: usize, artifacts: usize }`
- Produces: `pub fn swap_artifact_vec(cat: &Catalog) -> Result<()>`

- [ ] **Step 1: Write the failing tests** — the two SWAP tests. Not written; see the block
      above for why writing them now would ship an unwired function.

```rust
#[test]
fn the_swap_replaces_the_table_and_recreates_the_trigger() {
    // migrate_v6.rs:202 records the trap: "DROP TABLE implicitly drops the
    // artifact_vec_cascade_delete trigger". A swap that forgets it leaves the
    // catalog silently accumulating orphan vectors.
    let cat = Catalog::open_in_memory().unwrap();
    swap_artifact_vec(&cat).unwrap();
    let n: i64 = cat.conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name='artifact_vec_cascade_delete'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "the cascade trigger must survive the swap");
    let v2: i64 = cat.conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE name='artifact_vec_v2'", [], |r| r.get(0)).unwrap();
    assert_eq!(v2, 0, "v2 is renamed away, not left as a second table");
}

#[test]
fn the_swap_is_re_runnable() {
    // The catalog is machine-local and gitignored, so every checkout pays this
    // migration independently and an interrupted run must be resumable.
    let cat = Catalog::open_in_memory().unwrap();
    swap_artifact_vec(&cat).unwrap();
    swap_artifact_vec(&cat).unwrap();
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib the_swap_replaces_the_table`
Expected: FAIL — `cannot find function swap_artifact_vec`

- [x] **Step 3: Implement** — the backfill runner and fix (c). `swap_artifact_vec` is NOT
      implemented and stays for whoever lands fix (a) or (b).

```rust
/// Replace `artifact_vec` with the chunk-keyed `artifact_vec_v2`.
///
/// Re-runnable: a catalog already swapped has no `artifact_vec_v2` and returns
/// early. Reuses the DROP+CREATE shape in `rebuild_artifact_vec_at_dim`, and re-creates the
/// cascade trigger — `migrate_v6.rs:202` records that DROP TABLE takes the
/// trigger with it, which is silent and leaves orphan vectors behind.
pub fn swap_artifact_vec(cat: &Catalog) -> Result<()> {
    let has_v2: i64 = cat.conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE name = 'artifact_vec_v2'",
        [], |r| r.get(0))?;
    if has_v2 == 0 {
        return Ok(());
    }
    cat.conn.execute_batch(
        "BEGIN;
         DROP TABLE IF EXISTS artifact_vec;
         ALTER TABLE artifact_vec_v2 RENAME TO artifact_vec;
         CREATE TRIGGER IF NOT EXISTS artifact_vec_cascade_delete
         AFTER DELETE ON artifact
         BEGIN
           DELETE FROM artifact_vec WHERE id = OLD.id;
         END;
         COMMIT;",
    )?;
    Ok(())
}
```

For the backfill runner, walk artifacts in `updated_at` order, call `embed_queue_items`, embed in batches of `batch` (default 100, matching the `batch.len() >= 100` flush inside `index_repo`), and `write_embeddings_v2`. Record progress in `catalog_meta` (v10, `catalog/mod.rs:238`) under key `chunk_backfill_cursor`, via the `get_meta` / `set_meta` pair `gc.rs` already provides, so an interrupted run resumes.

**Do not consult `content_unchanged` at all.** The unchanged-content early return — the `if !force_rewalk && content_unchanged && meta_unchanged` block in `index_repo_sync`, whose embed branch is gated on `force_embed` — is where an already-trapped artifact is *sealed* on every later run. But note this is only the second half of P1's mechanism: the trap is *entered* at the `:302`/`:309` pair (§ *Entry condition*), so a backfill that skirts the early return escapes the seal and leaves the entry wide open. That is the whole reason the swap stays blocked on the fix while the backfill does not.

> **Coordinate note — verified 2026-09-02 at `8b48b8f1`.** All four `indexer.rs` line numbers this task originally carried are stale; the one `migrate_v6.rs` citation is exact (`:202` is verbatim `-- DROP TABLE implicitly drops the artifact_vec_cascade_delete trigger.`). That split is the signal: `indexer.rs` is the file this plan keeps moving. **No bulk shift is available** — the four corrections are +23, +22, +50 and −22 — so resolve every site by symbol:
>
> | cited | actually | at |
> |---|---|---|
> | `:479` "`write_embeddings`" | `write_embeddings` | `:502` |
> | `:640` "DROP+CREATE shape" | inside `rebuild_artifact_vec_at_dim` | `:662-663` |
> | `:652` "batch of 100" | `batch.len() >= 100`, inside `index_repo` | `:702` |
> | `:284` "unchanged-content early return" | `let now = …`, the line *after* the block; the block is `:262-282`, embed gate `:276` | — |
>
> Task 7 was re-verified against the live file today and its coordinates hold. This task's never were — which is the entire difference, and the reason the instruction is "resolve by symbol" rather than "use these numbers".

- [x] **Step 4: Run to verify they pass** — 38/38 `librarian::indexer`, 18/18
      `librarian::tools::reindex`. Mutation: cursor-as-watermark gave `left: 0, right: 2`.

Run: `cargo test --lib librarian::indexer`
Expected: PASS.

- [ ] ⚠ **Step 5: Dry run on a COPY, never the live catalog** — NOT RUN, and **not runnable
      as written**: `LIBRARIAN_CATALOG` does not exist in this codebase, so the command
      redirects nothing and opens the LIVE catalog. Needs a CLI catalog argument first.

```bash
cp ~/.local/share/librarian/catalog.db /tmp/claude-1000/catalog-backfill-test.db
LIBRARIAN_CATALOG=/tmp/claude-1000/catalog-backfill-test.db \
  cargo run --release --bin codescout -- librarian backfill-chunks --batch 100
sqlite3 /tmp/claude-1000/catalog-backfill-test.db \
  "SELECT COUNT(*) FROM artifact_chunk;"
```

Expected: on the order of 90,500 rows corpus-wide (~26,530 for codescout alone). A number near 4,500 means only one chunk per artifact was written — stop and re-check Task 6.

- [x] **Step 6: Gate and commit** — `98eb5adc` + `488192e8`. The staging line here names
      only `indexer.rs`; the CLI wiring also touches `src/cli/backfill_chunks.rs`,
      `src/cli/mod.rs` and `src/main.rs`, and fix (c) touches `tools/reindex.rs`.

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add src/librarian/indexer.rs
git -C /home/marius/work/claude/codescout commit -m "feat(librarian): resumable chunk backfill and transactional vec swap"
```

---

## Task 12: Re-run the benchmark, record the delta, close the bug

> **TASK 12 RAN 2026-09-03 — Steps 1–4 DONE, 5 and 7 held, and the result
> reframes the plan.** hits@5 **0/12 → 2/12**, MRR 0.0 → 0.1667, first non-zero
> this instrument has produced. Recorded at `6ff477b8` in
> `docs/trackers/retrieval-benchmark.md`.
>
> **Read the denominator: 2/12 is 2 of 2 reachable.** The two suite targets
> holding `artifact_chunk` rows both rank **1**; the other ten hold zero and are
> absent from the index, so no ranking change reaches them.
>
> **The gate on Steps 1–3 was a defect this session found and fixed.** Chunk
> ranges were numbered against the frontmatter-stripped body and published as
> file lines, so every hit on a frontmatter-carrying document landed inside the
> PREVIOUS entry. Fixed at `36afd405` (patch-id
> `6ba7ae81ba07d8fde8870fc6162c6330093159b8`), two guards each observed RED under
> its own mutation. 47 artifacts' stored ranges were migrated in place; nothing
> was re-embedded, because `content_hash` is over `content` alone — so the whole
> 0→2 delta is attributable to the coordinate fix and to nothing else.
>
> **§ *Deferred* turned out to be the live constraint, not a footnote.** This
> deployment resolves to **Qdrant**, and a reindex returned `embedded: 0,
> embed_error_count: 59`, every one Task 7's guard refusing a chunk id. That
> section already said *"if it is Qdrant, this plan does not apply to it yet"*.
> It is. So chunk-grain is inert here by design, and the 55 artifacts carrying
> chunk rows carry them because `embed_queue_items` writes the ROWS as a side
> effect of queueing even when the vector upsert is then refused.
>
> **Step 5 (archive) and Step 7 (commit the archive) are deliberately held.**
> Bug `7a37f1179d2f0e21` is now `status: fixed` with an `unverified:` field
> naming exactly what is not established. Archiving it would file a shipped
> feature that is unreachable on the default server backend. The decision it
> waits on is the plan's **open question 4**: point this project at sqlite-vec,
> or implement Qdrant chunk-grain parity. That is a deployment call, not a fix.
>
> Three artifacts were refused by the migration's round-trip check — their chunk
> rows are stale against the current file (17/41, 2/20, 3/10 chunks reproducing).
> A separate defect; only a re-chunk cures it.

**Files:**
- Modify: `docs/trackers/retrieval-benchmark.md` (via `artifact` tools — augmented)
- Modify: `docs/issues/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md` (via `artifact` tools — stamped `id:`, guarded)

- [ ] **Step 1: Re-run the suite**

```bash
python3 scripts/run-artifact-bench.py \
  --suite scripts/tc-suites/artifact-entries.json \
  --out /tmp/claude-1000/artifact-bench-after.json
```

- [ ] **Step 2: Compare against the baseline**

```bash
python3 -c "
import json
b=json.load(open('/tmp/claude-1000/artifact-bench-baseline.json'))
a=json.load(open('/tmp/claude-1000/artifact-bench-after.json'))
print(f\"hits@5 {b['hits_at_5']}/{b['n']} -> {a['hits_at_5']}/{a['n']}\")
print(f\"MRR    {b['mrr']} -> {a['mrr']}\")
for x,y in zip(b['cases'],a['cases']):
    if x['rank']!=y['rank']: print(f\"  {x['id']}: {x['rank']} -> {y['rank']}\")
"
```

**Report whatever it says.** A result below 8/12 is a finding, not a failure to hide — record it and investigate before claiming the change improved retrieval. The point of building the instrument was to be able to be wrong.

- [ ] **Step 3: Record the run in the benchmark tracker**

Same `artifact(action="update", patch={body_edits: […]})` form as Task 1 Step 4. Include the host, the model (`CodeRankEmbed`), the `baseline_sha`, and both numbers — the tracker's own § *Why this tracker exists* says a run without its config is not comparable to anything.

- [ ] **Step 4: Close the bug file**

```
artifact(action="update", id="7a37f1179d2f0e21", patch={
  status: "fixed",
  body_edits: [{heading: "## Tests added", action: "replace", content: "\n- `split_markdown_default_depth_ignores_h4` / `split_markdown_with_depth_6_splits_on_h4` / `split_markdown_with_depth_3_equals_the_default` (Task 2)\n- `entry_tokens_by_line` × 4, including the fenced-block case (Task 3)\n- `v11_creates_the_chunk_table_and_stamps_the_version`, `v11_is_idempotent`, `deleting_an_artifact_cascades_its_chunk_rows` (Task 4)\n- `replace_chunks_preserves_ids_for_unchanged_chunks`, `replace_chunks_drops_chunks_that_no_longer_exist` (Task 5)\n- **`embed_queue_items_emits_every_chunk_not_just_the_first`** — the core regression guard; killed by restoring `.next()`\n- `a_whitespace_only_section_is_dropped_without_dropping_the_batch` (Task 6)\n- `deleting_an_artifact_removes_all_its_chunk_vectors_not_only_the_first` — killed by `AND chunk_ix = 0` (Task 7)\n- `a_hit_names_the_chunk_that_matched_not_the_preamble`, `max_per_artifact_caps_without_emptying_the_page`, `max_per_artifact_one_yields_distinct_artifacts` — the last two killed together by `if false` on the cap (Task 8)\n- `context_candidates_are_distinct_artifacts_after_the_chunk_change` (Task 9)\n- `the_swap_replaces_the_table_and_recreates_the_trigger`, `the_swap_is_re_runnable` (Task 11)\n\nThree mutations were run and each killed a DIFFERENT test, per the once-per-site law.\n"}]
})
```

Record in `## Tests added` the actual test names from Tasks 2–11 and the three mutations that killed them. Fill `closed:` with today's date. Cite the fix by **SHA and patch-id** — `git show <sha> | git patch-id --stable` — because `experiments` is rebased after every ship and the SHA alone does not survive it.

- [ ] **Step 5: Archive through the catalog**

```
artifact(action="move", id="7a37f1179d2f0e21",
         new_rel_path="docs/issues/archive/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md")
```

Read `id_changed` / the new id from the response — the old id stops resolving immediately. Then re-point every citation of the old path **and** the old 16-hex id in the same commit, and verify with a scoped `audit_doc_refs` (0 high findings as the gate). The spec at `docs/superpowers/specs/2026-09-02-artifact-chunk-grain-retrieval-design.md` cites both and will need updating.

- [ ] **Step 6: Update the cluster count**

The bug carries `cluster/capped-result-presented-as-complete`. Archiving does not change membership, but confirm the count surfaces in `docs/trackers/issue-clusters.md` still agree with the corpus:

```bash
cargo test --test issue_clusters
```

Expected: 18 passed.

- [ ] **Step 7: Gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git -C /home/marius/work/claude/codescout add docs/issues/archive/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md docs/trackers/retrieval-benchmark.md docs/superpowers/specs/2026-09-02-artifact-chunk-grain-retrieval-design.md
git -C /home/marius/work/claude/codescout commit -m "docs(issues): chunk-grain retrieval shipped — measured delta, bug archived"
```
