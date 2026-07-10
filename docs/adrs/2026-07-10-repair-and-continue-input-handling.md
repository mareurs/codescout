# ADR: Repair-and-continue input handling

- **Date:** 2026-07-10
- **Status:** accepted
- **Deciders:** Marius (with the Architecture Snow Lion)
- **Commits:** `e92529e8` (unified path aliases + teaching hints), `19fb6b88`
  (filter-inversion repair + grep path alias) — both on `experiments`.

## Decision

When a tool can **deterministically** infer the intended input from a malformed
one, it repairs the input, executes, returns the result, and attaches an
advisory correction note — instead of returning a `RecoverableError`.
`RecoverableError` is reserved for input that is genuinely **missing or
ambiguous**.

## Context / forces

- Every `RecoverableError` forces the agent to retry, which is a **second full
  LLM inference** (latency + tokens + cost). The retry is the expensive part,
  not the error object.
- A usage.db sweep across 72 project DBs (~152k calls, 5.9% error rate) showed a
  large share of errors are deterministically-repairable shape/synonym
  mistakes: `file_path` for `path`, inverted filter leaves `{op:{field,value}}`,
  buffer handles under `output_id`, `file_path` on `grep`.
- codescout is **agent-agnostic** — the server must guide any MCP client without
  host hooks. Repairing in-process and noting the correction teaches every
  client at the moment of the mistake, which is why this needs no
  `server_instructions` change (self-describing when it happens).

## The boundary (load-bearing)

**Repair + note** when there is exactly ONE correct interpretation:

- synonym: `file_path` / `relative_path` / `file` → `path`; `regex` / `query` → `pattern`
- mechanical shape inversion: `{op:{field,value}}` → `{field:{op:value}}`
- coercible scalar: bool / int-as-string (pre-existing)

**Keep `RecoverableError`** (a teaching hint, never a guess) when input is:

- absent (no path, no pattern) — "there is no implicit current file"
- an unknown field that isn't an op; an uncoercible value
- ambiguous — more than one plausible reading

**Asymmetry — writes get a higher bar than reads.** Auto-accepting an *explicit*
write target (`create_file(file_path=…)`) is safe; auto-*guessing* a write
target must still hard-error. A wrong guess on a read wastes a query; on a
destructive write it is unrecoverable.

## Mechanism

- Repair at the tool's **input boundary**; keep core validators strict as
  defense-in-depth (`filter::compile` still errors on a truly-unknown field;
  `filter::repair_inverted_leaves` runs at the `find` handler *before* compile).
- Advisory feedback rides only on **object-shaped responses** (find / read /
  grep / …), reusing the `filter_warnings` / `corrections` shape. `json!("ok")`
  write tools repair **silently** — the round-trip is already saved; a note
  there would force reshaping ~40 tool responses for marginal teaching gain.
- Shared helpers: `crate::fs::PATH_PARAM_ALIASES`, `require_str_param_or_hint`
  (teaching hint on the error path), `filter::repair_inverted_leaves`.

## Alternatives considered

- **Error + teaching hint everywhere** (prior behavior) — every mistake costs a
  retry. Rejected: the retry is the expensive part.
- **Repair + notes everywhere, incl. reshaping `json!("ok")` tools** — maximal
  teaching, but response-contract churn + caller/test updates across ~40 tools
  for marginal gain. Rejected for now (Revisit-when below).
- **Surface the guidance in `server_instructions`** — rejected: the correction
  note and teaching hints are self-describing at the moment of the mistake; a
  prompt-surface addition is redundant weight.

## Consequences

- **Easier:** fewer round-trips (cheaper/faster flow); the agent still learns
  via the note; works for any MCP client.
- **Harder:** object responses grow a `corrections` field; the deterministic-only
  boundary must be honored or "save a call" becomes "silently do the wrong
  thing"; two inversion-detection sites (boundary repair + `compile` fallback)
  is mild duplication accepted as defense-in-depth.

## Change scenarios absorbed

- A new synonym an agent reaches for → add to `PATH_PARAM_ALIASES` (one place).
- A new deterministic shape mistake → a repair fn at that tool's boundary + note.

## Revisit-when

- Telemetry shows a repaired mistake was *mis*-repaired (a wrong
  single-interpretation assumption) → tighten the signature or revert to error.
- The `json!("ok")` silent-repair tools accumulate enough missed-teaching that
  reshaping their responses earns its cost.

## Confidence

**High** on the boundary + mechanism — verified live (inverted filter repaired +
noted; grep alias scoped). **Medium** on the notes-where-cheap split; may extend
notes to more responses if teaching value proves higher than noise.

## Sites (initial)

- `src/fs/mod.rs` — `PATH_PARAM_ALIASES`, `require_path_param` / `get_path_param`
- `src/tools/core/params.rs` — `require_str_param_or_hint`
- `src/tools/{create_file,edit_file,read_file,ast}.rs`,
  `src/tools/markdown/{read_markdown,edit_markdown}.rs` — path aliases + teaching hints
- `src/tools/grep.rs` — path alias (optional path)
- `src/librarian/filter.rs` — `repair_inverted_leaves`;
  `src/librarian/tools/find.rs` — boundary repair + `corrections` note
