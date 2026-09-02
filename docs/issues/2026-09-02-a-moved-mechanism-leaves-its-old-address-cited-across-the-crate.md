---
kind: bug
status: open
tags:
- cluster/doc-contradicted-by-code
- doc-drift
- engines
- audit-doc-refs
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
---

# BUG: a moved mechanism leaves its old address cited across the crate, and the grep that checks shares a scope with the fix

## Summary

Plan 3's wiring commit (`a8d2e0d9`) moved the session-opener trigger out of
`Tool::call_content` and into `engines::emitters::emit_session_opener`, rewriting
it from `!emitted.contains(SESSION_OPENING_GUIDE)` to `ledger.contains(topic)`.
**Ten Rust doc comments and code comments across four files still cite the old
form** — wrong file, wrong function, wrong expression. One is production code.
One is the remediation text inside a failure message.

## Symptom (Effect)

`src/server.rs:1157`, in a production filter closure (not a test):

```rust
// `notice_once`, not `insert`: under the opener's
// trigger (the `!emitted.contains(SESSION_OPENING_GUIDE)`
// check in `Tool::call_content`, `src/tools/core/types.rs`)
```

Neither the expression, the function, nor the file is correct any more.

## Reproduction

```
git log --oneline -1                       # a8d2e0d9 or later
grep -rn 'emitted.contains(SESSION_OPENING_GUIDE)' src/
```

**11 hits, of which 10 are instances.** The eleventh, `src/engines/coordinator.rs`,
is a *documentation example* — prose added in `9f1d92be` that quotes the stale form
to teach this very lesson. It is the "documentation example counted as a real
citation" case `CLAUDE.md` § *Parsers Over a Namespace* names, met while counting
this population.

| file | sites |
|---|---|
| `src/server.rs` | 4 — `:1157` (production), `:6503`, `:7249`, `:7673` |
| `src/tools/guide_ledger.rs` | 4 — `:56`, `:165`, `:357`, `:1045` |
| `src/prompts/mod.rs` | 1 — `:611` |
| `src/tools/config/mod.rs` | 1 — `:104` |

Two carry argumentative weight rather than being incidental: `guide_ledger.rs:56`
is *why* `notices` is a separate set, and `:357` is *why* `persist` deletes rather
than writing `{}`. A reader checking those arguments now checks them against a
mechanism that no longer exists where the comment says.

**Ranked separately — the same staleness inside a failure message**,
`src/server.rs:8623-8632`:

```
"SESSION_OPENING_GUIDE ('{}') now declares sections, but the opener branch
 in `call_content` still keys it as a bare topic … Update the opener
 branch to route through `guide_blocks_for` … before removing this assertion."
```

That is the text someone reads *at the moment the gate fires*, prescribing work on
a branch `a8d2e0d9` deleted. A stale comment misinforms; a stale remediation
**dispatches**.

## Root cause

Not the move — the move was correct and reviewed faithful. The defect is that
nothing relates a citation to its target:

1. **`audit_doc_refs` is markdown-only by default.** `DEFAULT_AUDIT_CODE_GLOBS`
   covers source files for *path* refs, but nothing scans a `.rs` doc comment for a
   cited **symbol, module or expression** that no longer resolves. The LSP index
   already loaded could answer exactly that question.
2. **The corrective sweep's completeness is unfalsifiable, and this instance shows
   why in a measurable way.** Three review rounds each named one site; I repaired
   the named site and offered a greppable sweep of `src/engines/`. **The grep and
   the fix shared a scope**, so the grep confirmed the fix *because of the blind
   spot they had in common* — two instruments agreeing about one blind spot, which
   `CLAUDE.md` § *Observer Blindness* names as indistinguishable from corroboration
   at the point of use.
3. **Quoting the expression is not a remedy.** All ten sites cite the trigger
   verbatim in backticks with no line numbers — the form I had just written into
   `coordinator.rs` as the durable one — and all ten rotted anyway, because the
   refactor rewrote the expression itself. A quoted expression is a claim about a
   revision exactly as a line number is; it fails later and less visibly, since a
   stale quote still reads as precise. Corrected at `9f1d92be`.

Measured 2026-09-02 by the anchored grep above, plus a reviewer's independent
enumeration that agreed on all ten.

## Hypotheses tried

1. **Hypothesis:** the class is "a third representation in `mod.rs`" — what
   `prompt-surface-measurement-session-log:F-49` recorded after three rounds.
   **Verdict: too narrow.** F-49 is true about the *repair* pattern, but the
   population is crate-wide and the representations are not co-located. Filed here
   as the wider class; F-49 stands as the process finding.
2. **Hypothesis:** every stale citation is in `src/engines/`, where the move
   happened. **Verdict: rejected** — zero of the ten are, and that is what a
   scope-sharing grep cannot see.

## Fix

Two halves, and only the second prevents recurrence.

- **The sites.** Rewrite the ten to name `emitters::emit_session_opener` and the
  *property* ("the opener fires when its topic is absent from the ledger") rather
  than the expression. Fix `server.rs:8623`'s remediation text first — it is the
  only one that dispatches. **Not done here on purpose:** four of the ten are in
  `src/server.rs`, where a peer is actively committing, and this was found while
  finishing an unrelated task.
- **The instrument.** Extend `audit_doc_refs` (or a sibling check) to resolve
  symbol/module citations inside `.rs` doc comments against the LSP index. That is
  the half that makes the sweep falsifiable; without it the next move repeats this.

## Tests added

None. The regression test for the second half is the check itself.

## Workarounds

When reading a doc comment that cites a mechanism, confirm the cited symbol exists
before trusting the argument built on it. `symbols(name=…)` answers in one call.

## Resume

Start with `src/server.rs:8623-8632`'s failure text — it is the only site that
prescribes action. Then the four `guide_ledger.rs` sites, two of which
(`:56`, `:357`) carry the *reason* for a design choice. Leave `src/server.rs`'s
comment sites until `git status --short src/server.rs` is clean; a pathspec commit
there captures a peer's in-flight hunk.

## References

- `9f1d92be` — the falsified "cite the expression" lesson, corrected in
  `src/engines/coordinator.rs`
- `prompt-surface-measurement-session-log:F-49` — the repair-pattern half
- `a8d2e0d9` — the move; reviewed faithful, not itself a defect
