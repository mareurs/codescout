---
status: fixed
opened: 2026-08-15
closed: 2026-08-16
severity: medium
owner: marius
related: []
tags: [get_guide, prompt-surfaces, doc-drift, external-report]
kind: bug
---

# BUG: `get_guide("iron-laws-detail")` states `cat src/foo.rs` is allowed; the gate refuses it

## Summary

The Iron Law 3 section of `get_guide("iron-laws-detail")` tells the reader that
`cat src/foo.rs` is allowed on bounded files. It is not — any shell command naming a
source path is refused. The sentence also contradicts its own bolded lead one clause
earlier. A guide that is wrong is worse than a guide that is missing: it converts a
checkable doubt into a false certainty.

## Symptom (Effect)

The guide body contains, verbatim:

```
**Read-mode for source code is blocked.** `cat src/foo.rs` is
allowed on bounded files but the broader "shell on source" pattern
is intercepted with a hint to route through `symbols`.
```

The bolded lead says blocked; the sentence says allowed.

## Reproduction

Reproduced 2026-08-15 at `821f9d0d`:

```
run_command("ls -1 src/tools/read_file.rs src/tools/core/types.rs … && sed -n '322p' src/tools/core/types.rs")
```

→

```
shell access to source files is blocked
hint: use read_file(path, start_line, end_line), symbols(path),
symbols(name=..., include_body=true), or grep(regex) instead.
Re-run with acknowledge_risk: true if you need raw shell access.
```

`sed` and `ls` are both on the guide's **own** bounded-LHS allowlist, stated one
paragraph earlier in the same section. So bounded-ness is not the deciding predicate —
the presence of a source path is.

## Environment

Reported on macOS against `experiments @ d7988aca`; reproduced on Linux at `821f9d0d`
via the live MCP server.

## Root cause

The gate is path-based, not command-based. `src/tools/run_command/inner.rs:305-315`:

```rust
// --- Step 2.5: Source file access block ---
if !buffer_only && !acknowledge_risk {
    if let Some(hint) = crate::util::path_security::check_source_file_access(resolved_command) {
        return Err(RecoverableError::with_hint(
            "shell access to source files is blocked", &hint).into());
    }
}
```

`check_source_file_access` inspects the resolved command for source paths. There is no
command allowlist and no bounded-file carve-out, so the guide's exception describes
behaviour that does not exist.

*Measured 2026-08-15: the `run_command` above was executed against the live server and
returned the quoted error.*

## Evidence

### Why this one outranks its size

The external reporter made the point himself, and it is the sharpest observation in his
report: this is the exact gate he got wrong in his own D3 write-up. He never read the
guide — **but had he read it, it would have confirmed his error.**

He arrived at "`wc` is in the block list" (a command-list model) rather than "shell on
source paths is blocked" (the actual path model). The guide teaches precisely that
wrong model.

## Hypotheses tried

1. **Hypothesis:** the carve-out exists but only for `cat` specifically.
   **Test:** read `inner.rs` Step 2.5 and look for a command allowlist.
   **Verdict:** rejected — the predicate is `check_source_file_access(resolved_command)`
   with no per-command branch.

## Fix

Shipped in `43fac6c8` (**experiments** — fast-forward promotion path available at fix
time, `git rev-list --left-right --count master...experiments` = 0/737, so this SHA
is final; no master-side SHA is owed). The Iron Law 3 paragraph in
`src/prompts/guides/iron-laws-detail.md` now states the block by its real predicate
(path, not command), lists refused commands regardless of boundedness, and names the
`acknowledge_risk: true` override the old text omitted.

The Iron Law 1 `force=true` over-promise flagged here as a companion fix was
addressed independently the same day by the IL1 overlap-condition commit
(`a926fdf5`), which scopes the claim to line ranges — the remaining `force`
whole-file semantics decision stays with
`docs/issues/2026-08-15-read-file-force-ignored-on-full-reads.md`.

Eval evidence that motivated priority: the false sentence, used as a planted-belief
trap (prompt-engineering `scenarios/conclude-last`, t2), measured **0/10** unaided
survival — and 0/5 even under a VERIFIED/INFERRED claim-tag contract. A wrong guide
converts a checkable doubt into false certainty; only unconditional-imperative
guidance arms survived it (5/5).
## Tests added

`prompts::redesign_invariants::iron_laws_detail_gate_claim_matches_path_predicate`
(`src/prompts/mod.rs`) — asserts the phantom carve-out phrase ("allowed on bounded
files") stays gone and that the guide keeps stating the path predicate and the
`acknowledge_risk` override. Green in `43fac6c8` (`cargo test --lib`: 3691 passed).
## Workarounds

Use `read_file(path, start_line, end_line)`, `symbols`, or `grep`. For genuine raw
shell on a source path, pass `acknowledge_risk: true`.

## Resume

N/A — fixed and archived. The B-6 decision (whether read-only metadata *should* get
a carve-out) remains open in
`docs/issues/2026-08-15-read-only-metadata-commands-blocked-on-source-paths.md`;
this fix supplies its "option B minimum" (the guide no longer implies the carve-out
exists).
## References

- `docs/trackers/bistriceanu/index.md` § B-9
- `docs/trackers/bistriceanu/index.md` § B-6 — the misdiagnosis this guide caused
- `src/tools/run_command/inner.rs:305-315` — the actual predicate
