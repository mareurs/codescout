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

## Amendment 2026-09-10 — the path family repairs silently; and stop advertising the aliases

- **Status:** accepted. **Decider:** Marius. **Trigger:** the second Revisit-when
  above, pulled by the operator: *"expose one if they are the same, but then in
  the code we correct if we receive any of the other and get back a hint that we
  overwrote a wrong param."*

**What was wrong.** This ADR's law was half-implemented and nobody had recorded
it. `find.rs` and `update.rs` do the whole law — repair, then
`response["corrections"] = {keyed, hint}`. The **path family repairs and never
notes**, and the reason is structural rather than an oversight:
`get_path_param` (`src/fs/mod.rs:233`), `require_path_param` (`:251`) and
`require_str_param_or_hint` (`src/tools/core/params.rs:139`) return
`Option<&str>` / `Result<&str>`. **A function returning `&str` cannot report a
correction to its caller**, so the note was unreachable from the helper, not
merely missing. The Sites list below calls those tools *"path aliases + teaching
hints"* — and the hint rides the **error** path, which a successful repair means
you never reach.

**Amendment 1 — the note becomes universal, at the chokepoint.** Alias repair
moves to `Tool::call_content` (`src/tools/core/types.rs`), ahead of everything
that reads the input, and emits `corrections` there. It does **not** go per-tool:
a side-effect tied to "whenever a call arrives" belongs in that call's one
chokepoint, and two handlers doing it locally is what earned the centralization.
The per-call helpers keep their alias fallbacks as a redundant second layer —
many existing tests invoke `call()` directly and bypass the boundary entirely.

**The field is `corrections`, and this is the load-bearing half of the
amendment.** A first draft of this work chose a new `warning` field, derived from
the `Guidance` enum's doc comment. That reasoning is wrong at the root:
`Guidance` attaches to a `RecoverableError`, and this ADR governs precisely the
path where no error is returned. Shipping it would have put a **third**
vocabulary on one concept — `filter_warnings`, `corrections`, `warning`.
One concept, one field name: `corrections`, shaped as `find.rs` shapes it.

**Amendment 2 — stop ADVERTISING the aliases, which this ADR never addressed.**
This ADR governs *acceptance*. It says nothing about whether an accepted synonym
should also be a schema property, and 23 of them are, across 8 tools, each
described `Alias for path`. That gap had a measured cost: co-equal alias
properties make a flat `required: ["path"]` a false claim, the only construct
that states the true requirement is a top-level `anyOf`, and **the Anthropic
Messages API rejects that outright** — so the client drops the whole tool before
sending. Seven tools were unreachable for eight days behind four green schema
gates
(`docs/issues/archive/2026-09-10-seven-tools-carried-an-api-illegal-top-level-anyof-and-were-dropped-client-side.md`,
fixed `2735df73`).

So: **advertise exactly one name per concept.** With one advertised name,
`required: ["path"]` becomes *true* and the pressure that produced the illegal
schema is removed rather than worked around. Nested combinators stay legal; only
the root is restricted, which leaves the per-action-schema question open rather
than closed.

**Scope of amendment 2 — path family only.** Deliberately excludes:

- `symbols`' `name`/`query` and `name_path`/`symbol`. They are true synonyms, but
  `symbols` **carries no top-level `required` at all**, so no false claim exists
  there and the change scenario that funds the path collapse is provably absent.
  Revisit if a `usage.db` split on those four names shows a retry or error
  asymmetry — an ambiguity cost is measurable, and consistency alone is not a
  change scenario.

  **OVERTAKEN 2026-09-11 by `8b396343`, and this exclusion is kept rather than
  deleted because a reader would act wrongly without it** — it is the only thing
  explaining why a decision recorded here looks contradicted by the shipped code.
  That commit collapsed exactly these four names to two under a separate
  directive, so the revisit condition above is spent: there is no four-name surface
  left for a `usage.db` split to measure. It was **not** overturned on the
  ambiguity-cost argument this paragraph asks for. It was overturned by a finding
  that argument never contemplated: the four keys fed a precedence chain producing
  the PATTERN, while the exact-vs-substring MODE came from a separate key-PRESENCE
  test, so a key that lost the race — contributing no value at all — still flipped
  the mode, and `symbols(query="Tool|Doc", symbol="x")` returned `0` matches with
  the regex refusal suppressed. `query` and `name_path` are now declared
  `param_aliases()`, rewritten before `call()` runs, so `call()` can observe only
  two keys. **What this leaves for the two exclusions still standing below:** the
  cost that funds a collapse need not be the cost the exclusion names, so "no
  measurable ambiguity cost" is not on its own a durable reason to keep a synonym.
- `librarian`'s `root`/`old_root` — unverified whether `root` means the same
  parameter for `merge_worktree` and `doctor` as it does for `rehome`. Not
  amended on an assumption.
- `read_file`'s `offset`/`limit`. A second calling convention, not a rename:
  `limit` is a count folded into `end_line`. Collapsing it would compute rather
  than rename, and a wrong computation is silent.

**Consequences.** Now easier: one advisory grammar an agent learns once; the
schema stops making a claim it cannot state honestly; a new synonym lands in one
list and is announced everywhere. Now harder: `corrections` becomes a
response-contract field on object-shaped responses, and it must be threaded
through **all four** of `call_content`'s render paths. Re-derived 2026-09-11 by
reading every consumer of `param_corrections` in `src/tools/core/types.rs`, not
transcribed from this paragraph's earlier form:

1. the **buffered envelope** — `corrections` written wholesale onto the overflow
   envelope;
2. the **`OutputForm::Text` compact render** — a `⚠`-prefixed hint STRING and no
   `corrections` key at all, because a text renderer cannot carry an object;
3. the **pretty-JSON value** — `merge_param_corrections` into the returned value.
   ONE site serving TWO emitters (an `OutputForm::Json` tool, and an
   `OutputForm::Text` tool whose `format_compact` returned `None`), which is why it
   counts once here and why a reader re-deriving from branch arms instead of from
   consumption sites gets five;
4. the **error path** — `attach_param_corrections_to_error` on the `Err` arm of
   `self.call()`. Added by `295a928e`; it is the one this paragraph, the
   implementation plan and three task reviews all missed, each having checked that
   the three they knew about were covered.

Path 4 is not itself flat, and the distinction is what the next reader needs:
`route_tool_error` (`src/server.rs`) dispatches to **three outcomes** —
`RecoverableError`, the LSP-transient `-32800`/`-32801` arm, and the fatal `else` —
which render as **two shapes**: an object body, taking the advisory at
`corrections.param_aliases`, and plain text, taking a `⚠ {hint}` prefix. Those
three counts move independently, and only the shape count bounds the advisory's
addresses. Conflating them produced
`docs/issues/archive/2026-09-11-the-alias-advisory-is-dropped-on-the-lsp-transient-error-branch.md`,
where the middle outcome composed its own body and consulted no advisory at all.
Since that fix the advisory is attached ONCE after arm selection, so a fourth
outcome cannot be added without carrying it.

Path 2 already ate this exact bug once
(`docs/issues/2026-09-02-the-worktree-notice-is-injected-then-discarded-by-every-compact-renderer.md`):
`format_compact` renders only the fields the tool knows about, so a
framework-added key is dropped unless re-attached at the render site. A
`corrections` key set on the value and nowhere else is silent on the two paths
most callers actually receive.

**Round 3 note (2026-09-10) — where the advisory lands, as implemented.** On the
**object-shaped** render paths — the pretty-JSON value and the buffered envelope
— the alias-correction advisory lands at `corrections.param_aliases`,
**unconditionally**, including when the tool wrote no `corrections` of its own.
An earlier implementation kept that no-op case flat (`corrections` = the advisory
directly) to avoid touching existing pinned assertions; that gave the advisory two
addresses depending on a fact the caller cannot see, so it was changed to nest
in every case. When a tool's own `call()` already returned a `corrections`
value, the framework nests its advisory into it rather than overwriting: an
object-shaped `corrections` gets a `param_aliases` key added; a non-object
`corrections` (e.g. a bare array) is promoted to `{"tool": <original>,
"param_aliases": <advisory>}` so both reach the caller.

**Two paths this does NOT cover, and both matter more than the sentence above.**
The `OutputForm::Text` compact render carries the advisory as a `⚠`-prefixed
hint STRING with **no `corrections` key at all** — a text renderer cannot carry an
object, so that asymmetry is by design, but it means **six of the ten**
alias-declaring tools deliver no key on the path they mostly take. Re-derived
2026-09-11 from the code, both halves:

- **Ten tools override `param_aliases()` with a non-empty map** — `create_file`,
  `edit_file`, `grep`, `read_file`, `edit_code`, `references`, `symbol_at`,
  `call_graph`, `symbols`, and `doc` (declared on `LibrarianAdapter`, keyed on the
  inner tool's name, in `src/librarian/adapter.rs`). Pinned per tool by
  `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL` (`src/server.rs`): 35 pairs over those ten.
- **`OutputForm::Json` is the trait default and every `output_form()` override in
  the tree returns `OutputForm::Text`**, so the split is exactly "does the tool
  override it". Six of the ten do — `grep`, `read_file`, `references`, `symbol_at`,
  `call_graph`, `symbols`; four do not — `create_file`, `edit_file`, `edit_code`,
  `doc`. (`tree`, `library` and `memory` override to `Text` too but declare no
  aliases, which is why an `output_form()` grep alone over-counts this side.)

The earlier form of this sentence named a smaller population, and the two tools it
omitted were added by this work stream itself after the sentence was written — the
document and the code have independent rates of change and nothing couples them,
which is the mechanism to expect here rather than an authoring slip to find.

And the
buffered envelope, while it does carry the framework's advisory, drops a
tool's OWN `corrections` entirely — a separate open defect tracked in
`docs/issues/archive/2026-09-10-the-buffered-envelope-drops-the-tools-own-corrections.md`.

**Unchanged by this amendment — the DECISION, whose factual claim has since gone
stale.** The decision stands: **no response is reshaped in order to carry the
note**, because the round-trip saving is in the repair rather than the note and
reshaping ~40 responses is not worth it.

What is no longer true is the sentence that used to state it — *"`json!("ok")`
write tools still repair silently."* Corrected 2026-09-10, found independently
twice: `annotate_write_path` (`src/tools/core/types.rs`) promotes a bare
`json!("ok")` to an object for its own unrelated purpose, the write-path
annotation, and it runs BEFORE the corrections insert. So the two `json!("ok")`
write tools that declare aliases — `create_file` and `edit_file` — do carry
`corrections.param_aliases`. The exception above was purchased with a cost that
is not being paid for them: the shape change was already paid for by something
else. `json!("ok")` write tools that are NOT promoted still repair silently, and
that is the clause's live scope.

**Revisit-when (added):** a repaired alias call is observed where the
`corrections` note did **not** reach the caller — that is a render-path hole, not
a repair failure.

**Do not read this trigger's own count as the search space.** Both times it has
fired, the hole was at a site the then-current count did not contain: `295a928e`
found the error path while the count said three, and `4629a95b` found an outcome
INSIDE that path while the count said four. The sites today are **four render
paths, the fourth holding three outcomes across two shapes**, enumerated under
Consequences above and re-derived 2026-09-11. The first move on a report is to
re-derive that enumeration — from `src/tools/core/types.rs`'s consumers of
`param_corrections` and from `src/server.rs`'s `route_tool_error` — rather than to
check the four you were handed and stop.

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
