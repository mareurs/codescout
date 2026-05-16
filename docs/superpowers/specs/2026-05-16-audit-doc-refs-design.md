# Audit Doc Refs — Design (2026-05-16)

Spec for a new `librarian` action that audits markdown files for stale code
references and surfaces findings as an `audit_issues`-archetype tracker.

Companion to `2026-05-08-researcher-tracker-design.md` (tracker patterns) and
motivated by the markdown ↔ code drift class documented in
`MRV-poc/CLAUDE.md` § Rules:

> "Real-world example: 2026-05-16 MA-41 spec referenced 'Streamlit /
> chat_app.py' 11 days after Streamlit was retired in the 2026-05-05
> vanilla-JS migration. Pattern-matching on familiar landmarks beat
> truth-checking."

## Goal

Provide a librarian action that:

1. Scans markdown files in scope for code references (file paths, line
   refs, symbol refs, link targets, dotted module paths).
2. Resolves each reference against the filesystem and the LSP symbol
   index.
3. Emits findings as an `audit_issues` tracker that lives in the repo,
   refreshes idempotently across runs, and surfaces decay via the
   existing `artifact_refresh_stale` query path.

Trackers solve fact-vs-prose drift. This action closes the second drift
class — markdown body vs live code symbols — that trackers do not
address today.

| Drift class | Solution today | Solution post-spec |
|---|---|---|
| Tracker prose vs live state | augmented trackers + `artifact_refresh_stale` | unchanged |
| Markdown body vs live code symbols | none — manual reading only | this action |

## Audience

- codescout maintainers reviewing a follow-on PR (estimated ~600 LOC, post-dissolve).
- Agents working in repos with heavy markdown ↔ code coupling
  (specs/, CLAUDE.md, design docs, ADRs).

## Prerequisites — parallel workstreams

This spec assumes the librarian crate has been dissolved into the root crate.
Two upstream PRs in flight at design time:

1. **`2026-05-16-artifact-cli-design.md` (PR-B)** — landing first. Adds
   `codescout artifact <verb>` CLI surface. Currently references
   `librarian_mcp::build_tool_context()`; will be path-swept by PR-A.
2. **`dissolve-librarian-crate` (PR-A, mechanical, no design doc)** — moves
   `crates/librarian-mcp/src/**` to `src/librarian/**`, folds Cargo deps,
   keeps `librarian` feature flag as a module gate.

This spec (PR-C) lands third, on the post-dissolve layout. All paths below
cite the post-dissolve location.

## Out of scope

- **CLI command resolution** in fenced bash blocks (`mrv ingest`,
  `cargo test`, etc.). Requires a command registry codescout does not
  maintain. Deferred to Phase 2.
- **Auto-fix.** Findings are surfaced; humans patch prose. Auto-rewriting
  markdown on symbol rename is ergonomic gold but lifecycle-dangerous.
- **Cross-repo references.** v1 is single active project; multi-repo is
  a Phase 2 candidate when `scope=umbrella` ships across librarian.
- **Watch mode / live re-audit on symbol-index change.** Pull-based audit
  only in v1.
- **Subtle line-drift detection** (line still in file but cited construct
  has moved). v1 detects `line_oob` only.
- **CI integration.** Cadence is manual in v1. `fail_on` schema present
  for opt-in by downstream repos; codescout itself does not wire CI in v1.

## Approach (decision summary)

1. **New librarian action**, not a new tool. Sits next to existing
   `context`, `reindex`, `tracker_design`, `workspace_state_at` in
   `src/librarian/tools/librarian.rs` (post-dissolve).
2. **Parse only inline-code spans, fenced blocks, and link targets.**
   Plain prose is not scanned. The false-positive rate on prose nouns
   that happen to be class names explodes otherwise. Backticks and
   code blocks are the user's signal of "this is a code identifier"
   — honor that signal.
3. **Output as an `audit_issues` tracker** (existing archetype, no new
   archetype needed — verified today at
   `crates/librarian-mcp/src/tools/tracker_design.rs:155`, post-dissolve at
   `src/librarian/tools/tracker_design.rs:155`). Reuses the F-N lifecycle —
   `open` / `in-progress` / `fixed` / `wontfix` — with automatic
   `open` ↔ `fixed` transitions on rescan.
4. **Merge by `(md_file, raw_ref)` primary key.** Numbering (`n`)
   assigned at first-seen and never renumbered. Same rule the
   `failure_table` archetype uses for F-N.
5. **Severity is config-overridable** with a sensible default map.
   Every finding carries a `severity_reason` (`policy_default`,
   `archive_drop`, `memory_drop`, `issues_drop`, `override:<key>`).
6. **`fail_on` flag** controls exit-code behavior. Default `never`
   (diagnostic). Pre-commit hooks can opt into `high`; not wired in v1.
7. **Manual cadence** — agent or human invokes when drift is suspected
   or before a doc-heavy PR merges. No CI gate, no scheduled refresh
   in v1.
8. **Parser is `pulldown-cmark`** — already a root dep post-dissolve. No
   new crate. `tree-sitter-markdown` evaluated and rejected (Open Q4
   resolved).
9. **All five `ref_kind`s ship.** `module_path` narrowed to inline-code-span
   and fenced-block contexts only (never bare prose, which is moot
   since prose is never parsed).

## Motivation — three concrete drift cases

### Case 1 — MRV-poc Streamlit retirement

`MRV-poc/docs/superpowers/specs/2026-05-16-section-feedback-design.md`
referenced `src/mrv/chat_app.py` — the Streamlit entry point retired
11 days earlier (2026-05-05) when the UI migrated to vanilla HTML+JS at
`demo/static/chat.js`. The spec was authored by an agent that
pattern-matched on a familiar landmark rather than re-verifying.

This audit would have flagged the reference as `file_path` → `missing`
at severity `high` at the next scan after commit.

### Case 2 — Renamed function symbol

`docs/manual/src/concepts/lsp-startup-stats.md` cites
`prewarm_lsp_background()`. If the function were renamed to
`prewarm_lsp_async`, the markdown would silently lie until a human
re-read the page.

This audit would flag the symbol as `file_symbol` → `symbol_missing`.

### Case 3 — Line-number reference drift

CLAUDE.md sections frequently cite `path:line`. As files evolve,
line numbers drift even when the symbol is intact. v1 detects only
`line_oob` (cited line past EOF); subtler "line still in file but no
longer the cited construct" drift is Phase 2.

## Architecture

Single new module dispatched from the existing librarian action enum:

```
src/librarian/
├── tools/
│   ├── librarian.rs            # extend Action enum + dispatch (one arm)
│   └── audit_doc_refs.rs       # NEW — parser + resolver + merger (~600 LOC)
└── (everything else unchanged from dissolved crate)

tests/librarian/audit_doc_refs/
├── fixtures/
│   ├── clean_repo/             # all refs resolve
│   ├── drift_repo/             # known verdict distribution
│   ├── regression_repo/        # fixed → open flip
│   ├── wontfix_repo/           # wontfix preservation
│   ├── archive_drop_repo/      # severity-drop policy
│   └── parse_recovery_repo/    # malformed fence recovery
└── corpus.rs
```

Reuses (all in-crate, post-dissolve):

- **Symbol resolution:** `src/lsp/symbols.rs` via the existing `symbols`
  plumbing. No dependency cycle (was a blocker pre-dissolve).
- **FS catalog:** `src/librarian/catalog/` for file existence + line counts.
- **Tracker upsert:** `src/librarian/catalog/artifact.rs::upsert` +
  `src/librarian/tools/augment.rs` for the `artifact_augment(merge=true)`
  write path.
- **Markdown parsing:** `pulldown-cmark` 0.13 (already a root dep).
- **OutputGuard:** `src/tools/output.rs` — `cap_items` + `OverflowInfo.by_file`.

## Tool surface

Extends `librarian` action enum.

### `Librarian::description` extension

The composite tool's description gets a new bullet so the LLM-facing
surface advertises the new action:

```
- audit_doc_refs: scan markdown for stale code refs (file paths, symbols,
  line refs, link targets). Surfaces broken references against current
  filesystem + LSP symbol index. Manual cadence — run when a doc-heavy
  PR is about to merge or when drift is suspected. Output is an
  `audit_issues` tracker.
```

### Input schema

```json
{
  "action": "audit_doc_refs",
  "scope": "project | repo | umbrella | all",
  "paths": ["docs/**/*.md", "CLAUDE.md"],
  "emit_tracker": true,
  "tracker_id": "abc123...",
  "severity_overrides": {
    "file_path": { "missing": "med" },
    "memory_globs": [".buddy/memory/**", "**/buddy/memory/**"]
  },
  "fail_on": "never | high | any"
}
```

| Field | Default | Notes |
|---|---|---|
| `scope` | `"project"` | Standard librarian scope. `umbrella` and `all` deferred to Phase 2. |
| `paths` | `audit_doc_refs::DEFAULT_AUDIT_GLOBS` (see below) | Visible constant; no hidden config. |
| `emit_tracker` | `true` | When `false`, returns findings inline only — used by pre-commit hooks. |
| `tracker_id` | none | If unset and `emit_tracker=true`, the action creates `docs/trackers/doc-ref-audit.md` (and registers it as an augmented artifact). |
| `severity_overrides` | none | Per-ref-kind, per-verdict overrides + optional `memory_globs` override. Merges over the default severity map. |
| `fail_on` | `"never"` | Diagnostic by default. `high` returns nonzero exit for CI gates. `any` includes `unknown`. Not wired to codescout's own CI in v1. |

```rust
pub const DEFAULT_AUDIT_GLOBS: &[&str] = &[
    "docs/**/*.md",
    "CLAUDE.md",
    "**/CLAUDE.md",
    "**/README.md",
];
```

### Output shape

```json
{
  "n_files_scanned": 138,
  "n_refs_found": 642,
  "n_refs_resolved": 583,
  "n_refs_broken": 17,
  "n_refs_unknown": 42,
  "tracker_id": "abc123...",
  "tracker_path": "docs/trackers/doc-ref-audit.md",
  "findings": [
    {
      "n": 1,
      "md_file": "docs/superpowers/specs/2026-05-16-foo.md",
      "md_line": 42,
      "raw_ref": "src/mrv/chat_app.py",
      "ref_kind": "file_path",
      "verdict": "missing",
      "severity": "high",
      "severity_reason": "policy_default",
      "status": "open"
    }
  ],
  "overflow": {
    "shown": 50,
    "total": 59,
    "by_file": { "docs/superpowers/specs/2026-05-16-foo.md": 12 },
    "hint": "narrow with paths=[...] or read full tracker at docs/trackers/doc-ref-audit.md"
  },
  "parse_warnings": [
    { "md_file": "docs/old.md", "line": 14, "reason": "unterminated code fence" }
  ],
  "scan_meta": {
    "degraded": false,
    "lsp_languages_offline": []
  },
  "exit_code": 0
}
```

`findings` is capped at 50 inline via `OutputGuard::cap_items`. The full
list always lives in the tracker. `overflow.by_file` matches the standard
`OverflowInfo` shape used elsewhere in codescout (project invariant —
see `docs/PROGRESSIVE_DISCOVERABILITY.md`).

## Reference taxonomy

Parser extracts code-identifier candidates from three syntactic positions:

1. **Inline code spans** — `` `path/to/file.py` ``.
2. **Fenced code blocks** — content between ` ``` ` fences. Optional
   info-string is honored for language hints but not required.
3. **Link targets** — the URL position in `[label](target)`.

Each candidate is classified into one of these `ref_kind`s:

| ref_kind | Pattern | Example | Context constraint |
|---|---|---|---|
| `file_path` | extension-bearing path, no `:` | `` `src/mrv/chat_app.py` `` | any of the three positions |
| `file_line` | path with `:NN` suffix | `` `scripts/eval_chunking.py:807` `` | any |
| `file_symbol` | path with `:Class/method` or `:fn_name` | `` `src/mrv/cli.py:cmd_generate` `` | any |
| `module_path` | dotted ident, ≥1 dot, lowercase, no `/`, no spaces | `` `mrv.chat_app` `` | **inline-code-span or fenced-block only** |
| `link` | URL position in `[…](…)` | `[foo](src/foo.py)` | link-target only |

Order matters — disambiguation is "most-specific wins."
`scripts/eval_chunking.py:807` is `file_line`, not `file_path`.

Anything else inside a code span is classified `unknown` and reported
at severity `low`; the agent never sees a hard failure for an
ambiguous identifier.

## Resolution semantics

| ref_kind | Resolver | Verdict set |
|---|---|---|
| `file_path` | catalog `repo_root.join(path).exists()` | `resolved` / `missing` |
| `file_line` | file exists + line count ≥ cited line | `resolved` / `missing` / `line_oob` |
| `file_symbol` | `symbols(path, name)` LSP call against the active project | `resolved` / `symbol_missing` / `file_missing` |
| `module_path` | candidate resolved via `symbols(name=...)` against active project | `resolved` / `unknown` |
| `link` (fs scheme) | as `file_path` / `file_line` | as above |
| `link` (http/https) | not resolved | `external` (informational, dropped from tracker) |
| `link` (anchor `#section`) | local markdown heading lookup | `resolved` / `anchor_missing` |

`unknown` is distinct from `missing`. `unknown` means the parser
identified a candidate but resolution was ambiguous: symbol-index lag,
multiple matches, polyglot identifier. Surface in the tracker at
severity `low`; never block a build on `unknown`.

## Severity policy

### Default map

| Verdict | Default severity | severity_reason |
|---|---|---|
| `missing` (file) | `high` | `policy_default` |
| `symbol_missing` | `high` | `policy_default` |
| `file_missing` (path of file_symbol) | `high` | `policy_default` |
| `anchor_missing` | `med` | `policy_default` |
| `line_oob` | `med` | `policy_default` |
| `unknown` | `low` | `policy_default` |
| `external` | (dropped from tracker) | — |

### Severity drop rules

| Match location | Drop | severity_reason |
|---|---|---|
| `docs/archive/**` or `*.archive.md` | one level (`high` → `med`, `med` → `low`) | `archive_drop` |
| Memory files (see globs below) | two levels (`high` → `low`) | `memory_drop` |
| `docs/issues/**` | one level | `issues_drop` |

**Memory globs** — match this project's real layout:

```
.buddy/memory/**
**/.buddy/memory/**
**/buddy/memory/**
**/projects/**/memory/**
```

Overridable via `severity_overrides.memory_globs`.

Rationale: archive is meant to rot; memory is temporally pinned by
design; issue trackers document historical states that may
intentionally reference retired symbols.

## Tracker integration — merge semantics

Output tracker is an `audit_issues` artifact. Params shape:

```json
{
  "issues": [
    {
      "n": 1,
      "title": "src/mrv/chat_app.py — missing",
      "severity": "high",
      "severity_reason": "policy_default",
      "status": "open",
      "owner": "",
      "ref_kind": "file_path",
      "md_file": "docs/superpowers/specs/2026-05-16-foo.md",
      "md_line": 42,
      "raw_ref": "src/mrv/chat_app.py",
      "first_seen_commit": "abc1234",
      "first_seen_at": "2026-05-16T14:23:00Z",
      "last_verified_at": "2026-05-17T08:00:00Z",
      "notes": ""
    }
  ],
  "scan_meta": {
    "last_scan_at": "2026-05-17T08:00:00Z",
    "last_scan_commit": "def5678",
    "n_files_scanned": 138,
    "n_refs_found": 642,
    "degraded": false,
    "lsp_languages_offline": []
  },
  "parse_warnings": [
    { "md_file": "docs/old.md", "line": 14, "reason": "unterminated code fence" }
  ]
}
```

### Render template

```jinja
**Last scan:** {{ scan_meta.last_scan_at }} ({{ scan_meta.last_scan_commit }}) —
{{ issues|selectattr("status","equalto","open")|list|length }} open /
{{ issues|length }} total
{% if scan_meta.degraded %} — ⚠ degraded ({{ scan_meta.lsp_languages_offline|join(", ") }} offline){% endif %}

| # | severity | reason | status | ref | found in |
|---|---|---|---|---|---|
{% for i in issues %}| {{ i.n }} | {{ i.severity }} | {{ i.severity_reason }} | {{ i.status }} | `{{ i.raw_ref }}` | {{ i.md_file }}:{{ i.md_line }} |
{% endfor %}

{% if parse_warnings %}
### Parse warnings ({{ parse_warnings|length }})

| file | line | reason |
|---|---|---|
{% for w in parse_warnings %}| {{ w.md_file }} | {{ w.line }} | {{ w.reason }} |
{% endfor %}
{% endif %}
```

### Merge rules (load-bearing)

1. **Primary key:** `(md_file, raw_ref)`. Numbering (`n`) assigned at
   first-seen and never renumbered.
2. **Currently-open issue whose ref now resolves** → auto-transition
   `open` → `fixed` with `notes: "auto-resolved at <commit>"`.
3. **Currently-fixed issue whose ref breaks again** → auto-transition
   `fixed` → `open`. Notes line carries the regression trail.
4. **`wontfix` status is never auto-flipped.** Humans own that flag.
5. **Severity escalates only.** If a finding moves from `low` to
   `high` between runs, severity updates. Going the other way
   (severity downgrade) requires a human edit or a severity-policy
   change.
6. **`first_seen_commit` is immutable.** `last_verified_at` updates
   on every rescan that observes the issue (open or fixed).
7. **Unknown fields preserved per-issue** (additive merge — see failure
   mode 4g).

Two back-to-back runs against an unchanged repo MUST produce
byte-identical tracker params. This is the property that
`tests::idempotent_merge` enforces.

## Error handling — named failure modes

### 4a — Markdown parse failure

`pulldown-cmark` returns a best-effort event stream for malformed input
(unterminated code fence, BOM at offset N, mixed line endings). The
parser MUST proceed with what was emitted, log a structured warning,
and surface `parse_warnings: [{md_file, line, reason}]` in BOTH the
inline response and the tracker's `scan_meta`. Never silently drop a file.

### 4b — Symbol-index unavailable

`symbols(...)` call fails because the LSP for the language hasn't
started or has died. Resolver MUST return `unknown` for every
`file_symbol` / `module_path` candidate in that language for this
scan. Tracker is annotated with `scan_meta.degraded: true` and
`lsp_languages_offline: ["<lang>"]`. Subsequent scans recover
automatically.

### 4c — Symbol-index lag

LSP is alive but its view is stale relative to the on-disk markdown
(e.g. file was just renamed; reindex hasn't completed). Resolver MUST
prefer the on-disk truth — a `symbols` call returning "not found"
combined with a successful `file_path` `exists` check yields
`file_symbol` → `symbol_missing` (not `unknown`).

### 4d — Tracker write conflict

Two parallel `audit_doc_refs` runs (CI + local) attempt to write the
same tracker. Reuse the existing `artifact_augment(merge=true)`
cross-process write serialization via `src/librarian/tools/augment.rs`.

### 4e — Path outside active project

Markdown reference points outside the project root (e.g.
`../other-repo/src/foo.py`). Resolver emits `unknown` with notes
`"path outside active project; scope=umbrella required"`. Never
attempts to traverse outside the project sandbox.

### 4f — Glob explosion

`paths=["**/*"]` against a large repo. Hard cap at 10000 files
per scan; over-cap returns a `RecoverableError` suggesting a tighter
glob. Cap is configurable via env `LIBRARIAN_AUDIT_MAX_FILES`.

### 4g — Tracker schema lock violation

User has hand-edited tracker params to add fields the merger doesn't
know about. Merger MUST preserve unknown fields per-issue (additive
merge), never delete them. The schema lock is upheld only over
required fields.

### 4h — Repo without `docs/trackers/` directory

`emit_tracker=true` and no destination configured. Action creates
`docs/trackers/doc-ref-audit.md` and registers it as an augmented
artifact. If the directory itself does not exist, the action creates
it (with a single `.gitkeep`) and proceeds.

### Error classification

| Class | Routing |
|---|---|
| Sibling-input-driven (bad glob, path outside project, repo missing trackers dir) | `RecoverableError` → `isError: false` |
| Genuine bugs (catalog DB corrupt, write to read-only FS, parser panic) | `anyhow::bail!` → `isError: true` |

## Testing strategy

### Tier 1 — Unit & schema tests (inline `#[cfg(test)] mod tests`)

| Test | Verifies |
|---|---|
| `parser_resolves_simple_file_path` | `` `src/foo.py` `` in span → `file_path` candidate |
| `parser_ignores_prose_outside_code_spans` | "We use Pydantic" not parsed; `` `Pydantic` `` is |
| `parser_classifies_file_line_over_file_path` | `` `src/foo.py:42` `` → `file_line` |
| `parser_classifies_file_symbol_over_file_line` | `` `src/foo.py:Bar/baz` `` → `file_symbol` |
| `parser_module_path_requires_code_context` | `mrv.chat_app` in prose → no candidate; in `` ` `` → `module_path` |
| `parser_recovers_from_unterminated_fence` | partial event stream + `parse_warning` emitted |
| `resolver_resolved_for_existing_path` | path exists → `resolved` |
| `resolver_missing_for_absent_path` | path absent → `missing`, severity `high`, reason `policy_default` |
| `resolver_line_oob_for_short_file` | line past EOF → `line_oob`, severity `med` |
| `resolver_symbol_missing_for_renamed_symbol` | mock LSP returns none → `symbol_missing` |
| `resolver_unknown_when_lsp_offline` | LSP unavailable → `unknown`, `scan_meta.degraded=true` |
| `resolver_prefers_disk_truth_on_lsp_lag` | file exists + LSP not found → `symbol_missing` NOT `unknown` |
| `resolver_external_for_https_link` | `https://...` → `external`, dropped from tracker |
| `severity_drops_one_level_in_archive` | `docs/archive/` ref → `med`, reason `archive_drop` |
| `severity_drops_two_levels_in_memory` | matches `.buddy/memory/**` etc. → `low`, reason `memory_drop` |
| `severity_reason_populated_for_every_finding` | every `Finding.severity_reason` is `Some(_)` |
| `idempotent_merge` | two back-to-back runs → byte-identical params |
| `lifecycle_open_to_fixed` | break → audit → fix → audit: status flips |
| `lifecycle_fixed_to_open` | fix → audit → break → audit: regression caught |
| `wontfix_never_auto_flipped` | wontfix + ref resolves → status stays `wontfix` |
| `unknown_field_preserved_across_merge` | human-added per-issue field survives rewrite (4g) |
| `paths_glob_filter_respected` | `paths=["docs/specs/**"]` skips everything else |
| `glob_explosion_returns_recoverable` | `paths=["**/*"]` over cap → `RecoverableError` |
| `outputguard_caps_findings_inline` | 51 findings → 50 emitted + `OverflowInfo.by_file` populated |

### Tier 2 — Behavior tests (fixture-driven)

`tests/librarian/audit_doc_refs/` corpus with:

- `fixtures/clean_repo/` — 5 markdown files, all refs resolve. Audit
  yields zero findings.
- `fixtures/drift_repo/` — 8 markdown files seeded with 3
  `file_path` misses, 2 `symbol_missing`, 1 `line_oob`, 2
  `module_path:unknown`. Audit yields exactly that distribution.
- `fixtures/regression_repo/` — golden tracker pre-seeded with a
  `fixed` issue. Audit re-injects the broken ref and verifies
  `fixed` → `open` flip.
- `fixtures/wontfix_repo/` — golden tracker with `wontfix`; audit
  with resolving ref preserves `wontfix`.
- `fixtures/archive_drop_repo/` — identical drift in `docs/specs/` vs
  `docs/archive/`; severities differ by one level.
- `fixtures/parse_recovery_repo/` — one file with malformed code
  fence; other files scanned; `parse_warnings` populated.

Tests run via `cargo test --test audit_doc_refs`.

### Tier 3 — Eval (`#[ignore]`-marked, run on demand)

`tests/librarian/audit_doc_refs/eval_on_codescout_self.rs` — runs the
action on codescout's own `docs/` tree. Golden output committed and
manually reviewed before each release; regressions in finding count
flagged. Equivalent to the existing `edit-eval-round-*` series.

**Acceptance threshold:** ≤5 findings on `master`. Higher count is
either a regression in the action OR doc drift to fix. The golden file
lists current findings explicitly so the diff makes the cause obvious.

Second eval target (deferred, NOT a v1 ship gate): runs against
MRV-poc repo. Expects Streamlit reference (Case 1) at `high`. Lives in
MRV-poc's own CI; not committed in codescout.

### Not tested (and why)

- **Wall-clock performance on huge repos.** 10000-file cap (failure
  mode 4f) makes the worst-case bounded; exhaustive performance is a
  separate concern.
- **`pulldown-cmark` upstream parser bugs.** Out of our control;
  failure mode 4a covers the partial-tree case.
- **Tracker render template MiniJinja edge cases.** Library code;
  tested by the librarian's existing render tests.
- **Real LSP integration in Tier 1.** Tier 1 uses `MockLspClient`
  from `src/lsp/mock.rs`. Real LSP exercised in Tier 3 eval only.

## Implementation order

### Phase 1 — Parser only

`audit_doc_refs.rs::parse_refs(text, path) -> Vec<RefCandidate>` using
`pulldown-cmark`'s `into_offset_iter()`. All five `ref_kind` classifiers,
including the `module_path` code-span guard. Tier-1 parser tests only.
Internal helper, not wired to dispatch.

**Ship gate:** all parser tests pass; corpus fixtures yield expected
candidate counts.

### Phase 2 — Resolver

Add `resolve_ref(candidate, ctx) -> Resolution` plus Tier-1 resolver
tests. Hooked into `parse_refs` to produce `Vec<Finding>`. FS check via
`src/librarian/catalog/`, LSP check via direct call to `src/lsp/symbols.rs`
(no trait indirection — possible only because PR-A dissolved the crate).
Severity + `severity_reason` assignment. `degraded` flag plumbing.
Still no tracker write — output is returned inline.

**Ship gate:** all resolver tests pass; Tier-2 `drift_repo` corpus
yields exact expected verdict counts.

### Phase 3 — Tracker integration + dispatch wire

Add `merge_into_tracker(findings, prior, now, commit) -> TrackerParams`
plus merge tests and lifecycle tests. Wire into `Librarian::call`
dispatch (one new action arm). Update `Librarian::description` text.
Auto-create of `docs/trackers/doc-ref-audit.md` with the augmentation
persistent prompt + render template. OutputGuard wiring. Manual page
`docs/manual/src/concepts/audit-doc-refs.md`.

**Ship gate:** all Tier-1 and Tier-2 tests pass; idempotency property
test holds across 100 random orderings of the same input.

### Phase 4 — Eval + golden

Tier-3 eval running on codescout's own docs. Golden output committed.
PR template updated to mention running the action on spec-heavy
branches.

**Ship gate:** Tier-3 eval committed with golden output; release notes
mention the action.

## Open questions — resolved at design time

| Q | Resolution |
|---|---|
| Memory severity drop too generous? | Keep `low` default. Revisit if a stale-memory drift case bites within one quarter (revisit-by 2026-08-16). |
| `unknown` ever block? | `fail_on=any` schema present; default `never`. No CI hook in v1; revisit when a CI consumer appears. |
| One global vs per-doc-root tracker? | One global `docs/trackers/doc-ref-audit.md`. Per-root partitioning revisit when active findings exceed ~100. |
| `tree-sitter-markdown` cost? | Dropped — `pulldown-cmark` already in deps and sufficient. |
| `rewrite_doc_refs` batch tool? | Out of scope. Lifecycle-dangerous; own spec if pursued. |

## Validation after implementation

After Phase 4 ships:

1. Run `librarian(action="audit_doc_refs", scope="repo")` against
   codescout itself. Expect ≤5 findings on `master`; any higher count
   is a regression in either the action or the docs.
2. Run against MRV-poc repo. Expect to surface the Streamlit
   reference (Case 1) as `high` and at least 2-3 other latent drifts
   we have not yet noticed. Use the output to seed a docs-cleanup PR
   in that downstream repo.
3. Refresh the tracker via `artifact_refresh_stale` after one week of
   commits. Verify staleness query returns the audit tracker and that
   re-running the action zeroes the age clock.
4. After PR-B (artifact-cli) ships, smoke-test the tracker is queryable
   via `codescout artifact find --tag doc-ref-audit --status open`.
   Synergy check; not a ship gate for PR-C.

If validation passes on (1)-(3), the action graduates from experimental
to stable per the standard graduation lifecycle. (4) is additive
verification once artifact-cli's CLI surface is available.
