---
kind: bug
status: fixed
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

Fixed on `experiments` — SHA `41d5a1e30f93c8f25ebc682bd1628a20572cf7ab`, patch-id
`b16161c458d3fd1219418309946b027ccbfc47c5`.

**⚠ THE FRAMING BELOW WAS TOO NARROW, and the correction changed what got built.** This section
read *"the value is that the predicate stops computing a confident wrong reason"* — a no-behaviour-
change tidy-up. Reading `segment_reads_project_source`'s own doc comment while fixing it shows the
stakes are higher: that function was added 2026-08-16 **specifically** to stop refusing
out-of-project paths with a remedy that *"could not be followed"*, measuring **25 of 111**
`il3_shell_on_source` refusals that named a path outside the project, and calling that *"a worse
failure than a strict gate"*. **This bug is that fix's residual** — the same unfollowable refusal,
reappearing through a token the resolver cannot read. So the REMEDY TEXT is the load-bearing half,
not the predicate.

Two changes, sharing **one** predicate:

1. `path_is_within_project` reaches its conservative branch explicitly when the token carries
   `$VAR` / `${VAR}` / `$(cmd)` / a backtick, instead of falling through to
   `project_root.join("$SP/x.sh").starts_with(project_root)` — the right answer for a reason the
   code never had.
2. `check_source_file_access` says so in the refusal **and names the action that works**: write the
   path out literally. Probe B above proves that is checked and allowed, so the caller is sent
   somewhere they can actually go.

`has_unexpanded_expansion` is shared by both on purpose: a second copy of the test would be free to
disagree with the verdict it describes. It is deliberately crude (`$` or backtick anywhere)
because both callers use it only to reach the BLOCKING verdict they would have reached anyway — it
must never be used to open the gate.

**Still not permissive, and that part of the original plan stands.** Resolving `$SP` would make the
verdict depend on the environment, which the sibling `cd`-target fix rejected on hermeticity
grounds (this module reads `HOME` directly and has no `EnvGuard`).
## Tests added

Three, in `src/util/path_security.rs`. The filing said a regression test *"needs the branch to be
observable"* — it is now, because the fix makes the refusal TEXT differ, so the reason is
assertable without exposing the branch itself.

- `an_unexpanded_expansion_says_the_path_was_never_resolved` — **observed RED before the fix**, and
  its panic printed the defect verbatim: `cat $SP/probe.sh` answered *"use read_file(path,
  start_line, end_line) or symbols(path)…"*, neither of which can serve a path outside the project.
- `a_resolved_in_project_read_carries_no_unresolved_caveat` — **THE CONTROL, and the reason the
  first test discriminates.** A note emitted unconditionally would satisfy that assertion while
  saying nothing, so an ordinary RESOLVED read is pinned to carry no caveat.
- `an_unexpanded_expansion_still_blocks` — the verdict must not move, on all three expansion forms
  (`$VAR`, `${VAR}`, backtick). This fix makes the gate honest, not more permissive.

`source_file_access_allows_a_source_read_outside_the_project` — the 2026-08-16 carve-out this fix
builds on — was checked green by name too, so the thing being extended is not regressed.

**GATE GREEN ON AN ISOLATED WORKTREE, stated rather than implied.** The shared checkout could not be
made green: this was the fifth in-flight blocker of the session, a `RecoverableError`/`anyhow`
mismatch in a peer's uncommitted `src/librarian/tools/mod.rs`. Verified on `git worktree add
--detach … HEAD` with only this diff applied — `cargo fmt --check` clean, clippy `--workspace
--all-targets --features local-embed` clean, lean lane **3807**, default lane **5830**, default
last. All four tests above read green BY NAME rather than by total.

The control that the isolation did not simply skip the broken area: **1155** `librarian::tools`
tests ran in that lane. The peer's breakage lived only in their uncommitted copy.
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
