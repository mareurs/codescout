---
entry_prefix:
  - F
  - W
entry_high_water_F: 10
entry_high_water_W: 2
---
# Session Log — local-embed Opt-In Work Stream

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...",
>          index_row="| {id} | <date> | <sev> | <cat> | open | **<title>** — … |",
>          index_after_line="<the table line your row goes AFTER>")
> ```
>
> One call, one write — **and the index row belongs in that same call.**
> The server allocates the next id, formats the heading as `## F-N —
> <title>` (the only shape `link_scan` accepts as a definition), records
> the ledger's high-water mark, stamps `**Valid:** dated <today>` unless
> your body declares a class, and writes `index_row` with `{id}` replaced
> by the id it just allocated. The indexes are the eval surface, the
> sections are the evidence.
>
> **A row needs an anchor from SOMEWHERE, and there are two sources.**
> Best, once per ledger: declare `snapshot_anchor` in this file's own
> frontmatter, naming the index table's header line verbatim —
> `doc(action="update", id=…, patch={"extra": {"snapshot_anchor": "| ID |
> Date | … |"}})`. After that, `index_row` **alone** places every row at
> the block's **TAIL**, which is where a newest-last ledger wants it, and
> nobody chooses an anchor again. Otherwise pass `index_after_line` per
> call; it WINS when both are given. They are **not** symmetric —
> `index_after_line` with no `index_row` is refused, and passing neither
> anchor with a row is refused by name.
>
> **Choosing that per-call anchor fails silently in both directions, which
> is the reason to prefer the declaration above.** It inserts after the
> **first** line equal to what you pass: a non-unique line writes your row into the wrong
> table, and a line that does not exist writes nothing at all and
> allocates no id. Neither raises an error. **Prefer the target table's
> last existing row** — unique by construction, because ids are — over
> the `|---|---|` separator, which anchors to the table's **top** and is
> repeated 9 times in one live ledger. Top is right for a newest-first
> ledger like `bug-fix-session-log.md` and wrong for the oldest-first
> ones; the corpus does not agree, so your ledger decides. A declared
> `snapshot_anchor` sidesteps this whole paragraph.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`index_row` is not a pre-written row, and that warning does not reach
> it.** The server substitutes `{id}` after allocating, in the same write
> as the section, so no instant exists at which the row names an id the
> section does not. The warning is about a row you type yourself ahead of
> the append — that one still consumes the id it names.
>
> **A copy of this file is a ledger from its first moment.** The frontmatter
> above declares `entry_prefix: [F, W]`, which is why the recipe at the top
> is accepted on a fresh copy with nothing to declare first — and, because
> a declared file is a ledger, the librarian guard refuses direct edits to
> it from the start (`edit_file` says so by name). Retitle it and edit its
> prose and index tables with `doc(action="update", id=<the copy's artifact
> id>, patch={body_edits: [...]})` once the copy is catalogued; add entries
> only with `append_entry`, never by hand.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (`docs/conventions/`, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-<n> | YYYY-MM-DD | low/med/high | <category> | open | <one-line title> |
| F-10 | 2026-11-09 | med | instrument-blindness | mitigated | **A new untracked file escaped a `git ls-files` guard, so the gate was green before the commit and red after** — `every_cap_constant_is_classified` never saw `holders.rs` until it was tracked |
| W-2 | 2026-11-09 | — | pattern | validated | **Read the gate before filing the bug** — the root's leniency in `doc` is deliberate; the filed bug is the narrow `dry_run` hazard |
| F-9 | 2026-11-09 | low | self-friction | mitigated | **I put an invented timestamp in a message to a peer** — corrected by a follow-up message |
| F-8 | 2026-11-09 | low | tooling | promoted-to-bug-tracker | **`fmt-mine.sh` was refused five times by a provenance lock** — the rust-analyzer mux holds an inherited shared copy for hours; bug `91b5ffce06ede31a` (fixed 2026-11-10, `1ccf33e7b`) |
| F-7 | 2026-11-09 | med | tool-surface | fixed-verified | **`doc update` accepted `dry_run=true` and wrote** — a parameter of a sibling tool, read by nothing here; bug `b284cd70c19eee0f`, fixed in `53a7d8ec6` |
| F-6 | 2026-11-09 | low | stale-doc | fixed-verified | **The legacy-retrieval tracker says memory queries use `search_memories`, and nothing calls it** — deleted; memory recall uses `SemanticMemoryStore`; tracker corrected |
| F-5 | 2026-11-09 | med | stale-plan | fixed-verified | **The tracker's plan for L-12 named a method over a collection nothing writes to** — `search_libraries` reads `library_chunks`; the library index writes `code_chunks` under `lib:<name>` |
| F-4 | 2026-11-08 | low | plan-prose | fixed-verified | **The manual contradicted itself about whether local-embed is a default** — two pages disagreed with each other and with Cargo.toml |
| F-3 | 2026-11-08 | low | self-friction | fixed-verified | **A doc claim about the HTTP fix was written from code reading, then run** — `url` alone sends the default model's bare name; Ollama needs `ollama:<name>` |
| F-2 | 2026-11-08 | med | measurement | fixed-verified | **cargo bloat measured a different quantity than the size claim** — unstripped, with-feature build; direct with/without builds gave 40.3 vs 61.9 MiB |
| F-1 | 2026-11-08 | med | architectural | fixed-verified | **The feature-lane guard vouched for local-embed through a YAML comment** — scanner read a comment as a test lane; now skips comments and reads matrix flags |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-<n> | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |
| W-1 | 2026-11-08 | high | read the history of what you reverse | three stale surfaces, a red guard, and the default-model decision made by default | validated |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Put the matching Index
row in the same call, as `index_row` (see *How to use* above); writing it
afterwards is the second write that call exists to remove. Do not allocate the id
yourself.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (`docs/conventions/<topic>.md`, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into a `docs/conventions/` page, an ADR, a skill, or another permanent surface (a `CLAUDE.md` line only when it passes that file's residency test). Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — The feature-lane guard vouched for local-embed through a YAML comment

**Valid:** dated 2026-11-08

**Observed:** 2026-11-08, scouting the removal of `local-embed` from `default`.

**When:** Checking whether `every_declared_feature_has_a_lane_or_a_reason` (tests/feature_lanes.rs) would still pass once `local-embed` left `default`.

**Expected:** The CI `test` matrix entry `flags: "--features local-embed --no-default-features"` counts as the lane that runs the `local-embed` tests.

**Got:** `features_from_text` scans line by line. It needs `cargo test` or `cargo nextest run` on the same line as `--features`. The matrix `flags:` line has no verb. The verb line (`cargo nextest run --workspace --no-fail-fast ${{ matrix.config.flags }}`) has no `--features`. The only line that met both conditions was a YAML comment, `.github/workflows/ci.yml:511`: `# \`cargo test --features local-embed --no-default-features\` shows …`. While `default` covered `local-embed`, nothing needed the lane, so the accident was invisible.

**Probable cause:** The scanner reads text, not structure. It did not skip comments and did not know about matrix flags.

**Workaround:** `features_from_text` now skips `#` lines. It also reads `flags:` entries when some line in the same text both runs tests and names `matrix.config.flags`. New fixture test: `the_workflow_scan_ignores_comments_and_reads_matrix_flags`. Mutation check against the live files: removing `--features local-embed` from the matrix entry reds `every_declared_feature_has_a_lane_or_a_reason` and names `local-embed`. Re-adding `local-embed` to `default` reds `the_onnx_backends_are_opt_in_not_default`. Both mutations were reverted.

**Severity:** med. Editing a comment would have turned the guard red with a message about a missing lane that exists.

**Status:** fixed-verified

## F-2 — cargo bloat measured a different quantity than the size claim

**Valid:** dated 2026-11-08

**Observed:** 2026-11-08, writing the "why" for dropping `local-embed` from `default`.

**When:** I had put `cargo bloat` figures into `Cargo.toml`, `tests/feature_lanes.rs` (doc comment and assertion message), the manual and the changelog: "`ort_sys` 12.3 MiB of the 40.3 MiB `.text`, 30.6%".

**Expected:** Those figures describe what a user saves by not linking ONNX Runtime.

**Got:** They describe something else. `cargo bloat` builds with `strip` overridden, so its binary was unstripped (its own line: "the file size is 90.1MiB"). The `.text` share is also taken from a build that already had `local-embed` on, so it says nothing about the build without it. Building the real artifact twice (`cargo build --release -p codescout --bin codescout`, stripped per `[profile.release]`) gave 42,262,960 bytes (40.3 MiB) without `local-embed` and 64,857,688 bytes (61.9 MiB) with it: 21.6 MiB, or 53%, more. The lean binary's size and the unstripped build's `.text` are both 40.3 MiB by coincidence, which would have made the old text read as consistent with the new measurement while meaning something else. The 21.6 MiB agrees with the "~21MB" recorded in `.cargo/config.toml` on 2026-09-15.

**Probable cause:** I quoted the first instrument that returned a number, and I did not check what it measured against the claim it supported. The tool itself says its numbers are "guesswork".

**Workaround:** Replaced the figures in all six places with the direct measurement. `cargo bloat` is still cited once, for the `ort_sys` share (12.3 MiB of `.text`), with its caveat.

**Severity:** med. Six places would have carried a wrong-quantity number, in a comment that a failing test prints.

**Fix idea:** For a "this feature costs N bytes" claim, build the artifact with and without the feature and compare file sizes. Use an attribution tool only to say where the bytes are.

**Status:** fixed-verified

## F-3 — A doc claim about the HTTP fix was written from code reading, then run

**Valid:** dated 2026-11-08

**Observed:** 2026-11-08, writing the troubleshooting entry for the new failure of a default build.

**When:** I wrote "set `[embeddings].url` … Ollama works: `url = "http://127.0.0.1:11434"`" as the HTTP fix for a project with no `[embeddings]` config.

**Expected:** `url` alone is enough to embed over HTTP.

**Got:** On review I saw that `create_embedder_with_config` strips routing prefixes (`bare_model_name`) when `url` is set, and that `guard_local_model_with_url` says "url set, model unset" resolves to `local:AllMiniLML6V2Q`. So the server would receive the default model's bare name. That was an inference from code, so I ran it: a throwaway HTTP server on 127.0.0.1, `codescout index` on a fresh project with `CODESCOUT_EMBEDDER_URL` set and no model. The request body was `{"model":"AllMiniLML6V2Q","input":[…]}`. Ollama needs an exact model name, so `url` alone fails there. The documented Ollama route is `model = "ollama:nomic-embed-text"`.

**Probable cause:** I described a fix from the shape of the error hint ("set `[embeddings].url`") without checking what the client sends once the URL is set.

**Workaround:** `docs/manual/src/troubleshooting.md` and `docs/manual/src/getting-started/installation.md` now give the Ollama route as `model = "ollama:nomic-embed-text"`, and say that `url` alone sends the default model's bare name, which only a server that ignores the name will accept. The capture script is `capture_embed_request.py` in the session scratchpad.

**Severity:** low. Caught before it shipped, but it was a checkable fact written into user-facing docs from reading alone.

**Status:** fixed-verified

## F-4 — The manual contradicted itself about whether local-embed is a default

**Valid:** dated 2026-11-08

**Observed:** 2026-11-08, scouting which docs name `local-embed`.

**When:** Grepping the manual and `CONTRIBUTING.md` before changing the default.

**Expected:** The manual states one consistent position on whether local ONNX embedding is a default feature.

**Got:** Two pages disagreed with each other and with `Cargo.toml`. `docs/manual/src/getting-started/installation.md` said `local-embed` "is not the default". `Cargo.toml` had put it in `default` on 2026-09-17. `docs/manual/src/configuration/embedding-backends.md` listed "Local (fastembed, **default**)" with setup "Bundled — no setup needed", while the same page's Local section said it needs a source build. After the change this page would have been wrong in the other direction.

**Probable cause:** The 2026-09-17 change updated `Cargo.toml`, `.cargo/config.toml`, `scripts/build-windows.sh`, `scripts/gate.sh`, `CLAUDE.md` and two issue files. It did not touch the manual.

**Workaround:** `embedding-backends.md` (comparison row, opt-in note, error section), `installation.md` (feature table row and the why-section), a new `troubleshooting.md` entry and the changelog now agree with `Cargo.toml`. The error block in the manual was compared with the live output of a rebuilt lean binary: same text.

**Severity:** low

**Status:** fixed-verified

## W-1 — Reading the issue that introduced the default exposed everything the reversal had to unwind

**Valid:** dated 2026-11-08

**Observed:** 2026-11-08, before editing `Cargo.toml` to drop `local-embed` from `default`.

**Pattern:** Read the issue and commit that introduced the thing you are about to reverse, and list its recorded consequences before the first edit. The 2026-09-17 issue (`docs/issues/archive/2026-09-17-the-default-build-cannot-run-the-default-embedding-model.md`) and commit `87e434dd4` named three consequences and one rejected alternative.

**What the scout surfaced:**
- The operator's 2026-09-17 ruling rejected "lean default plus a remote default model" because it leaves no zero-config path. The reversal was a decision for the user. I asked, with a recommendation, and the answer was to keep `local:AllMiniLML6V2Q`.
- `cargo rb` inherited `local-embed`, and `.cargo/config.toml` described that as superseded.
- `scripts/build-windows.sh` had a hand-written default list with `local-embed-dynamic` swapped in.
- `scripts/gate.sh` exported `CODESCOUT_SKIP_ONNX_TESTS=1` and printed "report PASS while skipped".
- `src/prompts/mod.rs` and `tests/test_python_runner.py` pin the gate's printed lines and command list, so those lines had to keep their needles.
- `every_declared_feature_has_a_lane_or_a_reason` would have passed only by accident (see `local-embed-opt-in-session-log:F-1`).

**Counterfactual:** Editing the one word in `Cargo.toml` would have left three surfaces stating false facts (the `cargo rb` note, the gate's skip comment and printed limit, the Windows script's comment). It would have left the old guard test red, and it would have left the lane guard vouching for `local-embed` through a comment. The decision about the default model would have been made by default, not by the user.

**Confidence:** high. Each point above was checked by reading the file or running the test.

**Status:** validated

## F-5 — The tracker's plan for L-12 named a method over a collection nothing writes to

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. The audit left `search_libraries` and `search_memories` as "dead, but the open tracker `docs/trackers/2026-05-07-legacy-retrieval-removal.md` plans to wire them in" (rows L-12 and L-13), so I kept both. When I started L-12, I read what `index(scope="lib:<name>")` writes before I wired anything: it stores a library under the project id `lib:<name>` in the `code_chunks` collection (`src/tools/semantic/index.rs`, `sync_project` and `project_index_stats`). `search_libraries` queries a `library_chunks` collection with project id `*`. `git grep library_chunks` finds that name only in `src/retrieval/search.rs` and in the May design spec.

**Plan said:** wire `RetrievalClient::search_libraries` into the stack path (L-12).

**Reality:** that method searches a collection nothing writes to, so wired in as planned it returns an empty page for every library.

**Counterfactual:** a green build, two passing refusal tests, and a `lib:<name>` search that always answers "0 matches" while the manual says it works.

**Resolution:** `ef7fe2ee9` validates the library against the registry and runs `search_code("lib:<name>", …)`; `search_libraries` is deleted. The happy path (an indexed library returning hits) has no test. It needs a populated vector store.


**Status:** fixed-verified for the refusals (tests pass). The happy path is covered only below the tool: `a_library_file_under_dist_is_indexed_and_found_by_its_lib_id` (`src/retrieval/sync.rs`) syncs a library with `SyncOpts::for_library` and searches its `lib:<name>` id on a real sqlite store. Mutating `for_library` to ignore `dist` turns it red. It does not pin the collection name (the sqlite store ignores it) or the id that `semantic_search` passes; that wiring has no test. A first version of this test passed under a wrong-collection mutation, which is how I learned it could not see that. Fix commit `ef7fe2ee9`.

## F-6 — The legacy-retrieval tracker says memory queries use search_memories, and nothing calls it

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. `docs/trackers/2026-05-07-legacy-retrieval-removal.md` marks L-04 and L-13 as done: "memory queries route through `RetrievalClient::search_memories`". `references` and a text search over all Rust sources find no caller of `search_memories`; the only other mention of the `memories` collection is `Agent::semantic_memory_store` in `src/agent/mod.rs`, which is a different store.

**Reality:** the claim is stale, or the routing exists somewhere I did not read. I did not establish which: I did not read the memory recall path.

**Consequence:** `search_memories` stays in the tree as an apparently planned piece with no caller. I kept it for that reason, so the audit's "delete the dead items" step is incomplete by one item that I could neither delete nor justify.

**Resolved 2026-11-09:** memory recall goes through `SemanticMemoryStore::search` (`Memory::call`, `src/tools/memory/mod.rs`); the Qdrant implementation has its own `memory_search_dense` (`src/retrieval/memory.rs`). `RetrievalClient::search_memories` had no caller, and under the default sqlite store it would have searched the code index: that store ignores the collection name and keys by project id. I did not test that last point by running it; it follows from `SqliteVecCodeStore::query` taking `_collection` unused. The method is deleted, and the tracker's rows L-04 and L-13 are corrected. L-13 stays open as a feature, with the right method named.

**Status:** fixed-verified by build and tests (see the commit).

## F-7 — doc update accepted dry_run=true and performed the write

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. I sent `doc(action="update", dry_run=true, …)` expecting a preview, because `dry_run` is a parameter of `librarian(merge_worktree)`. The call returned `{"updated": true}` and replaced the `## Fix` section of a bug record with `x`. I caught it in `git diff` before committing and repaired it. A second probe on a throwaway file showed that `bogus_param_xyz=true` is dropped the same way.

**Cost:** one repaired record, no commit carried it. The shrink guard did not fire because the edit was small.

**Reading before filing paid off:** the first framing was "unknown top-level keys are ignored". The gate `every_schema_object_promising_to_refuse_unknown_keys_actually_refuses` says the root is excluded on purpose, so the filed bug is the narrow one (`dry_run` on an action that does not implement it). See W-2.

**Status:** fixed-verified. Bug `b284cd70c19eee0f` (archived; the archive move re-minted its id), fixed in `53a7d8ec6` with the narrow refusal. Wire check done on the rebuilt server 2026-11-09: `dry_run=true` on `doc update` is refused and the file is unchanged.

## F-8 — fmt-mine.sh was refused three times by a provenance lock

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. Three calls of `./scripts/fmt-mine.sh` through `run_command` were refused before the script ran, with `native provenance coordination refused dispatch: lock acquisition failed because the operation would block`. The first overlapped a background clippy that I had just started. The other two ran after that clippy had ended. Other `run_command` calls, including `./scripts/gate.sh`, ran normally in the same period.

**Cause (as first written, superseded by the Status line below):** not found. I did not read the dispatch code, and I did not check whether a peer session held the lock.

**Workaround used:** `rustfmt --edition 2021` on the named files. `rustfmt` follows `mod` declarations, so I checked `git diff --stat` afterwards: only my files had changed. The pre-commit hook then ran `rustfmt --check` on the committed bytes, which passed.

**Cost:** one avoided formatter run, and a manual check that `fmt-mine.sh` exists to make unnecessary.

**Status:** promoted-to-bug-tracker. Cause found 2026-11-09 and filed as bug `91b5ffce06ede31a` (archived; the archive move gave it this new id): the formatter takes the provenance lock exclusively, and the `codescout mux` for rust-analyzer (with its children) holds a shared copy through inherited descriptors, about three hours after the call and the server that took it. The refusal names no holder. Measured with `/proc/locks`, `/proc/<pid>/fdinfo`, `lsof` and `ps`. A `flock -n -x` that I also ran from inside `run_command` exited 1, but that call holds a shared lock itself while it runs, so it was confounded and is not evidence; the bug record says so. Not fixed at the root: the fence is another effort's design, and the bug lists two options. **Option 2 is done** (`4eceee5e7`, `0d47f5898`): a refusal now names its holders and marks a cut list. The bug stays open for option 1.

**Update 2026-11-10.** Fixed by option 3 (`1ccf33e7b`): only a formatter with a native proof takes the leases exclusively, so a leftover holder no longer refuses `fmt-mine` for a Claude session. Verified live while an `adb` server and a dev server held shared leases. Option 1 was measured and is not recommended; the bug is archived.

**Update 2026-11-09, later in the session:** a fourth direct call was refused with the same lock message. Separately, inside `./scripts/gate.sh` the formatter lane exited `FMT=1`, `FMT_STATE=ownership-refused`: `fmt-mine` named `src/retrieval/search.rs` as `SHARED` and refused to format it, citing coverage gaps (`unreadable Claude history directory`, `Claude inherited history has an unverified native root enqueue boundary`). `git diff` showed the only uncommitted change in that file was my own deletion, so the attribution was recoverable from the diff and not from the scan. This is a different refusal from the lock one, but it shares the cause class: the provenance scan cannot see enough history to attribute a file. Both were worked around with `rustfmt --edition 2021` on the one file, after reading its diff.

## F-9 — I put an invented timestamp in a message to a peer

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. In a message to a peer session about their uncommitted `src/librarian/util.rs`, I wrote that the gate found the clippy error "at about 12:00". I did not know the time of the run, and no tool had given it to me. I sent a one-line correction right after.

**Why it matters:** a made-up detail in a message to another agent is a claim the receiver may act on or cite. This one was small and harmless, but it is the same failure as an unverified number in a doc, and it happened in a message that was otherwise built from tool output.

**Rule I am applying:** a time, a count or a name goes into a message only if a tool result in this session gave it to me. Otherwise I leave it out.

**Status:** mitigated. The correction was delivered to the peer's inbox. I have no sign that the peer read it.

## W-2 — Reading the existing gate before filing changed the bug that was filed

**Valid:** dated 2026-11-09

**What happened:** before filing the `dry_run` bug (F-7), I looked for an existing gate on unknown parameters. `src/librarian/tools/artifact.rs:1723` says the root object of `doc`'s schema is excluded from `additionalProperties:false` enforcement on purpose, because the dispatcher passes one shared argument blob to every action, and an earlier attempt at `deny_unknown_fields` on the root "broke every `doc(update)` call".

**Counterfactual:** the bug I was about to write asked for unknown top-level keys to be refused. That is the change the comment says was tried and reverted. The filed bug instead names the narrow hazard (`dry_run`, a real parameter of a sibling tool, ignored on an action that writes), cites the `grep` precedent for refusing one named key, and says plainly what is not established: whether a check on the schema's declared `properties` would avoid the earlier breakage.

**Cost of the pattern:** two reads (a `grep` for `additionalProperties`, then the comment). It paid for itself in one filing.

**Status:** validated.

## F-10 — A new untracked file escaped a git ls-files guard, so the gate was green before the commit and red after

**Valid:** dated 2026-11-09

**Observed:** 2026-11-09. I added a new file, `src/tools/native_provenance_fence/holders.rs`, with `const MAX_LISTED: usize = 12;`. The full gate before I committed it was green. The gate after the commit was red: `result_caps::every_cap_constant_is_classified` named the constant as an unclassified cap. HEAD (`4eceee5e7`) carried a red guard test until the fix commit (`0d47f5898`).

**Cause:** the guard reads its population from `git ls-files src`, so it sees tracked files only. A new file is untracked until its first commit, and every gate run before that commit passes over it. This is a property of the guard's selector, the class `IC-18` (selector narrower than its population), and it applies to every `git ls-files`-based guard in `tests/`, not only this one. Listed 2026-11-09 by `grep` for `ls-files` in `tests/`: `result_caps.rs`, `issue_clusters.rs`, `committed_paths.rs`, `inline_gates.rs`, `issue_frontmatter.rs`, `buffer_stream_policy.rs` and `e2e/eval_common/proc.rs`. I read the selector's rationale only in `result_caps.rs:644`, not in the other six.

**Cost here:** HEAD red for one commit, caught by the very next gate. I also nearly relied on a focused test run (the fence tests) that did not include `result_caps`.

**Rule I am applying:** after the first commit that adds a file under `src/`, run `result_caps` again, or the whole gate, before treating the change as verified; and run the gate once with the new file staged (`git add -N`) so that `git ls-files` lists it. I did not test that `git add -N` makes the guard see the file.

**Not fixed, and probably should not be.** `tests/result_caps.rs:644` says the selector is deliberate: "an untracked file is a peer's in-flight work and gating on it lets one session red another's build". Reading `--others` would remove that protection, so the earlier idea of changing the guard is withdrawn. What stays is a rule for the author of a new file: stage the file by explicit path before the gate. That has a price, because the staged file is then visible to a peer's gate too. The rule is now recorded in `docs/conventions/gate-ordering.md`, section "What the gate cannot see", with the `git add -N` check run in a scratch repository (`git ls-files` lists the file after `add -N`; the guards themselves were not run against it).

**Status:** mitigated.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...",
              index_row="| {id} | <date> | <sev> | <cat> | open | **<t>** — … |",
              index_after_line="<the table line your row goes AFTER>")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark, stamps `**Valid:** dated
     <today>` AND writes the index row with `{id}` filled in — one write.
     A row needs an anchor from SOMEWHERE: `index_after_line` per call, or
     a `snapshot_anchor` declared once in this file's frontmatter, after
     which `index_row` alone lands the row at the block's TAIL. Passing
     `index_after_line` with no `index_row` is refused; they are not
     symmetric. A per-call anchor must be UNIQUE (first match wins,
     silently) and decides top vs bottom — prefer the last existing row.
     Do not hand-allocate ids; do not type the row yourself ahead of the
     call. -->
