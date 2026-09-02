---
status: open
opened: 2026-09-02
closed:
severity: low
owner: marius
related: []
tags: ["cluster/unclassified"]
kind: bug
---

# BUG: `a_documented_call_names_a_live_tool` counts (call × param) pairs and calls them "documents"

## Summary

`tests/doc_tool_refs.rs`'s two guards share one extractor whose grain fits only one of
them. `anchored_cites()` emits **one `Cite` per named argument**; the parameter test
consumes that correctly, but the *tool-name* test asks a per-tool question of the same
per-param list without deduplicating. One offending call is reported once per argument it
carries, and the failure message labels the total `"N present-tense document(s)"` — a
count of (call × param) pairs presented as a count of documents.

## Symptom (Effect)

One stale call on one line of one file, reported as two:

```
thread 'a_documented_call_names_a_live_tool' panicked at tests/doc_tool_refs.rs:417:5:
2 present-tense document(s) call a tool that does not exist.

  src/prompts/guides/librarian.md:335
      `artifact_event(…)` names no registered tool.
      line: `artifact_event(action="list", artifact_id=X)`.

  src/prompts/guides/librarian.md:335
      `artifact_event(…)` names no registered tool.
      line: `artifact_event(action="list", artifact_id=X)`.
```

Two identical blocks, same file, same line, same tool. The call carries two named
arguments (`action`, `artifact_id`), so it produced two `Cite`s.

## Reproduction

On `experiments` (or this branch), introduce a call to a retired tool name carrying two or
more named arguments in any scanned present-tense surface, then:

```
cargo test --test doc_tool_refs a_documented_call_names_a_live_tool
```

The finding count equals the argument count, not the call count. A one-argument call
reports once; a five-argument call reports five times.

## Environment

codescout `experiments`, worktree `.worktrees/tool-collapse`, Rust stable, Linux.
Observed while the tool-surface-collapse branch tripped the guard on a call that Task 4's
`artifact_event → doc` fold had left behind in `src/prompts/guides/librarian.md`.

## Root cause

`anchored_cites()` (`tests/doc_tool_refs.rs:307-347`) walks each line, captures the tool
name, extracts the balanced depth-1 argument span, then **pushes one `Cite` per named
argument** — `for a in arg.captures_iter(&span) { out.push(Cite { ..., param: a[1] ... }) }`
(`tests/doc_tool_refs.rs:336-344`). Every `Cite` from one call shares `file`, `line`,
`tool` and `text`, differing only in `param`.

`a_documented_tool_parameter_exists_on_that_tool` (`:358`) is a **per-parameter** question,
so that grain is correct for it — one finding per bad parameter is the right report.

`a_documented_call_names_a_live_tool` (`:398`) is a **per-tool** question. It filters on
`c.tool` alone (`:405-410`) and pushes a finding per surviving `Cite` (`:411-414`) with no
dedup on `(file, line, tool)`. Its assertion message then interpolates `bad.len()` into
`"{} present-tense document(s)"` (`:419`, `:423`), so the number is neither documents nor
calls but (call × param) pairs.

Read out of `tests/doc_tool_refs.rs:307-425` — the miscount itself was **observed** in the
gate output quoted above (2026-09-02), the mechanism is read from source.

## Evidence

### The extractor's grain (`tests/doc_tool_refs.rs:336-344`)

```rust
let span = quoted.replace_all(&span, "");
for a in arg.captures_iter(&span) {
    out.push(Cite {
        file: rel.clone(),
        line: i + 1,
        tool: tool.clone(),
        param: a[1].to_string(),
        text: line.trim().chars().take(110).collect(),
    });
}
```

### The per-tool consumer, undeduplicated (`tests/doc_tool_refs.rs:405-414`)

```rust
if names.contains(&c.tool) || allowed.contains(c.tool.as_str()) {
    continue;
}
bad.push(format!(
    "  {}:{}\n      `{}(…)` names no registered tool.\n      line: {}",
    c.file, c.line, c.tool, c.text
));
```

### The same class already debugged one function up

The comment block immediately above the extraction loop records a *prior* miscount in this
very function — billing a neighbouring call's arguments to the current one:

> Running to the last `)` on the line billed a neighbouring call's arguments to this one —
> `artifact_augment(merge=…)` sitting after `artifact(…)` produced a phantom
> `artifact(merge=`, and the report was 20 violations of which 14 were that.

That fix corrected *which* arguments are attributed. It did not touch the grain mismatch
between the extractor and the per-tool consumer, so the population count stayed wrong in a
second way.

## Hypotheses tried

1. **Hypothesis:** the scan reads the same file twice (e.g. once from disk and once via an
   `include_str!`'d copy), producing duplicate findings.
   **Test:** read the walk in `anchored_cites()` and the per-argument push loop.
   **Verdict:** rejected — the duplication is per named argument within one line, not per
   file. Both `Cite`s carry `line: 335`.
   **Evidence link:** *The extractor's grain* above.

2. **Hypothesis:** the extractor emits one `Cite` per named argument, and the per-tool test
   consumes that list without dedup.
   **Verdict:** confirmed. The offending line carries exactly two named arguments and
   produced exactly two findings.
   **Evidence link:** *The extractor's grain* + *The per-tool consumer, undeduplicated*.

## Fix

Not yet applied. Two independent one-liners, and they are worth keeping separate because
they fail differently:

1. **Grain** — dedup by `(file, line, tool)` in `a_documented_call_names_a_live_tool`
   before pushing, so one call is one finding. The parameter test must NOT be deduplicated;
   its grain is already right.
2. **Unit** — the message says `document(s)` while counting findings. Even after dedup the
   two differ: one document can carry several stale calls. Name the unit the number
   actually has (`stale call(s)`), per `CLAUDE.md` § *Testing Discipline* — *"a count of a
   defect population must arrive with its unit or not at all"*.

Fixing only (1) leaves a number that is right today and mislabelled; fixing only (2) leaves
an honest label on an inflated number.

## Tests added

None yet. A regression test wants a fixture surface carrying one retired-tool call with
≥2 named arguments, asserting the failure names it **once**. Note that the natural
assertion here is a count, and this file's own `the_scan_is_not_reading_an_empty_corpus`
exists because `is_empty()` assertions are monotone under removal — a dedup that
over-collapses (say, keying on `file` alone) would also produce a smaller number and
satisfy a naive "reports once" check. Key the assertion on the emitted text, not the count
alone.

## Workarounds

Read the finding blocks, not the headline number. Duplicate blocks with an identical
`file:line` are one call, and the multiplicity is its argument count.

## Resume

Apply both fixes in `tests/doc_tool_refs.rs`: dedup `(file, line, tool)` in
`a_documented_call_names_a_live_tool` (`:398-426`), and change `"present-tense
document(s)"` to a unit matching what is counted (`:419`). Leave
`a_documented_tool_parameter_exists_on_that_tool` (`:358-390`) alone — its per-parameter
grain is correct. Then add the regression fixture described in *Tests added*, asserting on
emitted text rather than on the count alone.

## References

- `tests/doc_tool_refs.rs:307-347` (extractor), `:358-390` (parameter test), `:398-426`
  (tool-name test).
- `CLAUDE.md` § *Testing Discipline* — the unit-of-count law.
- `docs/trackers/issue-clusters.md` — `IC-11` (`cluster/doc-contradicted-by-code`) is what
  this file guards; the defect here is in the guard, not the corpus.
- Observed in the tool-surface-collapse branch gate, 2026-09-02, after Task 4's
  `artifact_event → doc` fold left a call behind in `src/prompts/guides/librarian.md:335`.
