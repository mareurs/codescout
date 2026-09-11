---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
- run-command
- path-security
- il3
closed: null
opened: 2026-09-10
owner: marius
related:
- docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md
severity: low
---

# BUG: the source gate joins an unexpanded `$VAR` path onto the project root, so a scratchpad write is refused as in-project source access

## Summary

`run_command("cat $SP/x.sh")` is refused as source-file access even when `$SP` expands
to a directory outside the project. The gate inspects pre-expansion command text, so the
token `$SP/x.sh` is `is_relative()` and gets joined onto the project root — the gate then
believes `<project_root>/$SP/x.sh` is a real in-project `.sh` file. The same command with
the path written out literally is allowed.

## Symptom (Effect)

```
run_command("SP=/tmp/.../scratchpad; cat $SP/probeA.sh; echo 'D ok'")
→ {"ok": false,
   "error": "shell access to source files is blocked",
   "hint": "use read_file(path, start_line, end_line) or symbols(path) + symbols(name=..., include_body=true) instead. Re-run with acknowledge_risk: true if you need raw shell access."}
```

**The first half of the hint cannot be followed for this path.** `read_file` and `symbols`
resolve against the active project, so neither can serve a file under `/tmp`. The second
half (`acknowledge_risk: true`) is performable, so the refusal does have a working exit —
this is a wrong reason and a dead first remedy, not a trap.

## Reproduction

Four probes, run 2026-09-10 at `96633d33`. `$SP` is the session scratchpad, outside the
project root:

| # | command shape | result |
|---|---|---|
| A | `echo hi > $SP/probeA.sh` | **allowed** — `echo` is not a content reader |
| B | `cat > /tmp/.../probeB.sh <<'EOS' … EOS` | **allowed** — literal absolute path resolves outside |
| C | `cat > $SP/probeC.sh <<'EOS' … EOS` | **refused** |
| D | `cat $SP/probeA.sh` | **refused** — no heredoc involved |
| E | `cat > $SP/probeE.txt <<'EOS' … EOS` | **allowed** — fails the extension half |

D and E are the load-bearing pair: D removes the heredoc and still refuses, E keeps the
heredoc and passes, so **the heredoc is not part of the mechanism.** An initial reading
attributed this to the known heredoc-as-command-text shape and was falsified by these two
probes.

## Environment

codescout MCP server `0.15.0`, `run_command`, IL-3 source gate,
`src/util/path_security.rs`. Branch `experiments`, project `codescout`. `bash` is one of
this project's indexed languages, which is why `.sh` is a source extension here.

## Root cause

`path_is_within_project` (`src/util/path_security.rs:1911`) expands only `~/`, then:

```rust
if expanded.is_relative() {
    let Cwd::At(base) = cwd else { return true; };
    if expanded.components().any(|c| matches!(c, Component::ParentDir)) { return true; }
    return base.join(&expanded).starts_with(project_root);
}
```

With `tok = "$SP/probeA.sh"` and no preceding `cd`, `cwd` is `Cwd::At(project_root)`, there
is no `..` component, so the call returns
`project_root.join("$SP/probeA.sh").starts_with(project_root)` → `true`. Paired with a
blocked reader name (`cat`) and a source extension (`.sh`), `check_source_file_access`
refuses. Measured 2026-09-10 by probes A–E above; predicate read at `96633d33`.

**Why this is a gap and not the documented conservatism.** The function's own doc comment
says *"anything that cannot be resolved counts as inside"*, and enumerates the cases that
take that path: `~/` with no `HOME`, a relative token under `Cwd::Unknown`, and a `..`
component. Each of those `return true` explicitly. A `$VAR` token is **not** detected as
unresolvable — it takes the *resolved* branch and produces a computed `starts_with`
verdict for a path it never resolved. The answer happens to coincide with the conservative
one, so the wrong reason is invisible in the outcome.

Sibling: `docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md`
(fixed, `be2d7781`) taught the gate that a `cd` can move the shell, and its fix
deliberately maps an unresolvable `cd` target — *including a variable* — to
`Cwd::Unknown`. That reasoning covers the **cwd** token and not the **operand** token,
which is why this survived it.

## Hypotheses tried

1. **Hypothesis** — the heredoc body is being read as command text, the known shape where
   four separate scanners each misread a heredoc. **Test** — probe D (no heredoc, still
   refused) and probe E (heredoc, `.txt`, allowed). **Verdict** — rejected.
2. **Hypothesis** — `.sh` files are refused outright regardless of location. **Test** —
   probe B, a literal absolute path to a `.sh` file outside the project. **Verdict** —
   rejected; location is evaluated, just wrongly for variable tokens.

## Fix

Not attempted. The shape consistent with the sibling's fix is to classify a token
containing an unexpanded expansion (`$`, `${`, backtick, `$(`) as unresolvable and
`return true` **on the conservative branch**, so the blocking verdict is reached for the
stated reason rather than by a false join. That leaves behaviour unchanged and makes the
predicate honest, which is the cheap half.

Making such a path *allowed* is the expensive half and probably wrong: resolving `$SP`
would make the gate's verdict depend on the environment, which the sibling's fix rejected
explicitly for `cd ~` on hermeticity grounds (`src/util/path_security.rs` has no
`EnvGuard` and reads `HOME` directly). The affordable remedy is a clearer refusal, not a
resolution.

## Tests added

None — not fixed. A regression test would assert
`path_is_within_project("$SP/x.sh", root, &Cwd::At(root))` reaches the conservative branch
rather than the join, which is a claim about the *reason* and so needs the branch to be
observable.

## Workarounds

Write the path literally (probe B), or pass `acknowledge_risk: true`. For files inside the
project, use `read_file`/`symbols`/`grep` as the hint says — that advice is correct there
and only wrong for the outside-the-project case this bug is about.

## Resume

Decide whether the conservative-branch reclassification is worth a commit on its own or
should ride along with the next `src/util/path_security.rs` change. No behaviour change
either way; the value is that the predicate stops computing a confident wrong reason.

## References

- `src/util/path_security.rs:1911` — `path_is_within_project`.
- `docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md`
  — the sibling that fixed the cwd half.
- `docs/issues/archive/2026-09-01-source-gate-refuses-the-whole-compound-command.md` — fixed
  2026-09-11, same gate: the refusal now names which clause offended, which is what made probes A–E
  necessary to localise this one.
