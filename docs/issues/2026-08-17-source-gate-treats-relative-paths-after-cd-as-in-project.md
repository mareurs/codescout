---
id: '70cd189fa2590af3'
kind: bug
status: fixed
title: 'BUG: the shell-on-source gate counts every relative path as in-project, so `cd <outside> && awk x.rs` is refused with a hint that cannot be followed — the residue of 433100bd'
tags:
- iron-law
- run-command
- path-security
- il3
- gate-firing
closed: 2026-08-18
opened: 2026-08-17
owner: marius
related:
- docs/trackers/2026-08-16-iron-law-gate-firing-audit.md
severity: low
---

## Summary

`path_is_within_project` returns `true` for **every** relative path token, on the stated
assumption that "`run_command` executes with the project root as its cwd". A command may
`cd` elsewhere first, and nothing tracks that — so `cd <outside-the-project> && awk '…'
file.rs` is refused as shell-access-to-project-source, with a hint routing to
`symbols`/`read_file`, which cannot serve a file outside the active project.

This is the residual slice of GF-3, fixed one day earlier in `433100bd` precisely to stop
refusals whose remedy is unfollowable.

## Symptom (Effect)

2026-08-17, writing a scratch copy of a schema into `$CLAUDE_JOB_DIR/tmp` (outside the
project) and running `awk` over it:

```
run_command("cd /home/marius/.claude-kat/jobs/44c01c0f/tmp && awk '…' artifact_head.rs")

→ shell access to source files is blocked
  hint: use read_file(path, start_line, end_line), symbols(path),
        symbols(name=..., include_body=true), or grep(regex) instead.
        Re-run with acknowledge_risk: true if you need raw shell access.
```

Neither `read_file` nor `symbols` can serve that path — it is not in the project, so the
index does not cover it. The refusal's own IL-3 note states the opposite of what happened:

```
IL-3 source condition: … A path outside the project root is allowed, because
`symbols`/`read_file` resolve against the active project and cannot serve it.
```

Copying the byte-identical file to `artifact_head.txt` and re-running the *same* command
succeeded.

## Reproduction

Branch `experiments`, HEAD `637b9d37`. Live MCP.

1. `run_command("mkdir -p /tmp/outside && cp <any .rs> /tmp/outside/x.rs")` — or use any
   directory outside the active project root.
2. `run_command("cd /tmp/outside && awk 'END{print NR}' x.rs")`
3. Observe `shell access to source files is blocked`.
4. `run_command("awk 'END{print NR}' /tmp/outside/x.rs")` — absolute path, same file.
5. Observe it **succeeds**. The absolute form takes the membership branch and is correctly
   allowed; only the relative form is misclassified.

Step 4/5 is the discriminating pair: identical file, identical reader, verdict flips on
whether the path is written relative or absolute.

## Environment

codescout MCP server, `run_command` tool, IL-3 source gate. Rust.
`src/util/path_security.rs`. Branch `experiments`, project `codescout`.

## Root cause

Measured 2026-08-17: the `.rs`/`.txt` pair above, then the guard read at `637b9d37`.

`check_source_file_access` (`src/util/path_security.rs:1211-1277`) is a two-part
heuristic — a blocked command name **and** a source extension — and since `433100bd` also
requires project membership via `segment_reads_project_source`
(`src/util/path_security.rs:1295-1299`):

```rust
shell_tokens(seg)
    .iter()
    .any(|tok| ext_re.is_match(tok) && path_is_within_project(tok, project_root))
```

`path_is_within_project` (`src/util/path_security.rs:1307-1320`) short-circuits on
relative paths, and its doc comment names the assumption outright:

> *"Conservative in the blocking direction: anything that cannot be resolved counts as
> inside … Relative paths are inside by construction — `run_command` executes with the
> project root as its cwd."*

```rust
if expanded.is_relative() {
    return true;
}
expanded.starts_with(project_root)
```

`run_command` does *start* at the project root; the assumption is that the command cannot
move. `cd <path> &&` moves it. `check_source_file_access` already splits the command on
`&&`, `||`, `;` and `|` — so the `cd` is present in the very segment list the decision
walks, and is simply not consulted.

**The extension was necessary, not sufficient.** The `.txt` rename passed because it failed
the extension half, not because location was re-evaluated — so "the gate discriminates on
extension, not location" is a wrong reading of the same evidence. See Hypotheses tried #1.

## Evidence

### The two commands, and the verdict flip

All three run live, 2026-08-17, against the same file in the same directory outside the
project:

```
cd <outside> && awk '…' artifact_head.rs   → blocked   (relative token)
cd <outside> && awk '…' artifact_head.txt  → ran       (extension miss, not location)
awk '…' /abs/outside/artifact_head.rs      → ran       (membership branch reached)
```

The third is the discriminating one, and it is **measured, not inferred**:

```
run_command("awk 'END{print \"lines:\", NR}' /home/marius/.claude-kat/jobs/44c01c0f/tmp/artifact_head.rs")
→ exit_code: 0
  stdout: lines: 316
```

Identical file, identical reader, identical extension. The only difference between the
blocked call and this one is whether the path was written relative-after-`cd` or absolute —
which isolates the relative-path short-circuit as the cause and rules out both the
extension and the directory.
### What GF-3 was for

`segment_reads_project_source`'s own doc comment, added by `433100bd`:

> *"The gate's remedy is 'use symbols / read_file instead', and both resolve against the
> **active project** — they cannot serve a path the index does not cover. Until 2026-08-16
> the extension match alone decided, so reading a dependency's source under
> `~/.cargo/registry`, a sibling repo, or a file in `~/.config` was refused with a
> suggestion that could not be followed. That is a worse failure than a strict gate: a
> strict gate at least leaves a correct path open. Measured: **25 of 111**
> `il3_shell_on_source` refusals in codescout's own `usage.db` named a path outside the
> project."*

Every clause applies to the `cd`-then-relative case unchanged. The fix closed the
absolute-path population and left this one open.

### A second trigger: no `cd`, an absolute out-of-project root, and a glob

2026-08-17, second session, hit while sweeping a sibling repo for references before
deleting a hook:

```
run_command("grep -rn 'il3-warn' /home/marius/work/claude/claude-plugins \
             --include='*.json' --include='*.mjs' --include='*.sh' --include='*.md'")

→ shell access to source files is blocked
  hint: use grep(pattern, path) codescout tool instead.
```

This matters because **the title of this bug under-scopes it**. There is no `cd` here.
The search root is *absolute* and unambiguously outside the project. The only relative
token in the command is the glob `--include=*.mjs`, which is not a path at all — it is a
filter pattern naming an extension. `ext_re` matches it, `path_is_within_project` sees a
relative token and returns `true` by construction, and the segment is refused.

So the failing condition is not "a `cd` moved the cwd". It is: **any token in the segment
that contains a source extension and is not an absolute path forces the in-project
verdict**, regardless of every other path in the command. A single glob argument is
enough.

Discriminating probes run the same minute, same directory, same command shape:

| Command | Verdict |
|---|---|
| `grep -rn 'il3-warn' <abs-sibling-repo> --include='*.mjs'` | **refused** |
| `grep -rn 'il3-warn' <abs-sibling-repo> --include='*.json'` | allowed |
| `grep -rln 'il3-warn' <abs-sibling-repo>` (no `--include`) | allowed |

The only variable is whether a glob names a source extension. That isolates the cause to
the token scan, independent of the `cd` path in the original report — and rules out the
reading that the absolute search root was somehow mis-resolved.

The hint is unfollowable here for the same reason as the original case, but one step
worse: `grep(pattern, path)` resolves against the **active project**, so it cannot search
a sibling repo at all. The suggested remedy has no correct invocation, not merely an
inconvenient one.

**Consequence for the fix.** Filtering on "tokens that look like paths" is not enough —
`--include=*.mjs` would still qualify under most such tests. A token carrying an `=`
prefix, or consisting of a bare glob with no directory separator, is an option argument
rather than a file operand, and treating it as a path is what produces this instance.

## Hypotheses tried

1. **Hypothesis:** the gate decides on file extension alone and ignores project
   membership — the `.rs`→`.txt` rename flipping the verdict is the proof.
   **Test:** read `check_source_file_access`, `segment_reads_project_source`,
   `path_is_within_project`.
   **Verdict:** rejected. Membership *is* checked; the relative-path short-circuit is what
   made this token look in-project. The rename is consistent with both explanations, so it
   discriminates nothing — the absolute-path call in Reproduction step 4 is the test that
   actually separates them, and it succeeds.
   **Evidence link:** *The two commands, and the verdict flip*.
   **Note:** this hypothesis was one write away from shipping as the entry's stated
   mechanism. It was rejected only because the guard source was opened.

2. **Hypothesis:** `$CLAUDE_JOB_DIR/tmp` is special-cased or otherwise treated as in-scope.
   **Test:** the absolute-path form of the same file under the same directory (step 4).
   **Verdict:** rejected — it runs. The directory is irrelevant; the path *form* is
   everything.

## Fix

**Shipped 2026-08-18, `be2d7781` (experiments).** Both causes, one predicate.

**1 — the shell can move.** `check_source_file_access` now splits twice: sequential
operators (`&&`, `||`, `;`, newline) bound a *run*, the unit a `cd` can move, and a
pipeline inside a run shares one cwd. `cd x | cmd` puts the `cd` in a subshell that
cannot affect the other stage, so propagating cwd across a pipe would be a bypass
rather than a carve-out. A new `Cwd` starts at `At(project_root)` — which is where
`run_command` actually starts — so the old "relative is inside by construction" rule
is now written down as state instead of assumed.

`cd_effect` yields `Cwd::At` only for a target it resolves completely. A variable,
`cd -`, a `..` component, and a relative `cd` from an already-unknown base all yield
`Cwd::Unknown`, which keeps the old blocking verdict.

**Deviation from §3 of the original proposal, deliberate.** Bare `cd` and `cd ~` mean
`$HOME`; this file proposed treating them as *resolved*. They are left **unresolved**
instead. Resolving them makes the gate's verdict depend on the environment, and this
module has no `EnvGuard` and reads `HOME` directly — so a test pinning that branch
would be non-hermetic. The only cost is that a rare command keeps being refused,
which is the safe direction and consistent with the blocking bias the rest of the fix
keeps.

**2 — an option can carry an extension without naming a file.** `--include='*.mjs'`
is a filter pattern; nothing is read from it. The carve-out is keyed on **positive
evidence** — an operand that is absolute and outside the root — and *not* on "options
are not paths".

That distinction is load-bearing and was nearly missed. The tidier rule opens a hole:
`grep -rn x src/ --include='*.rs'` genuinely reads project source, and the glob is the
**only** token that says so, because `src/` carries no extension. Skipping option
tokens outright would have silently stopped blocking it. The mutation below proves
it rather than asserting it.
## Tests added

Six, in `src/util/path_security.rs`, alongside the existing
`check_source_file_access_at_root` fixture.

**Two were RED before the change, four were green throughout.** The split matters:
the green four are controls, not restatements — they pin behaviour a careless fix
breaks, and a suite where every new test starts red would have had none.

| Test | Before |
|---|---|
| `a_cd_out_of_the_project_makes_a_relative_source_read_reachable_again` | **RED** |
| `an_option_glob_does_not_force_the_in_project_verdict` | **RED** |
| `a_cd_that_stays_inside_the_project_still_blocks` | green |
| `a_cd_the_gate_cannot_resolve_keeps_blocking` | green |
| `a_cd_inside_a_pipeline_does_not_move_the_shell_for_other_stages` | green |
| `an_option_glob_still_counts_without_evidence_of_an_outside_target` | green |

**Three mutations, each producing exactly one distinct failure** — which is what
shows the tests discriminate rather than merely co-occur with the fix:

| Mutation | Red |
|---|---|
| `cd` tracking neutered (resolve, then discard the result) | `a_cd_out_of_the_project_…` only |
| option carve-out disabled | `an_option_glob_does_not_force_…` only |
| option carve-out made unconditional — *the naive fix* | `an_option_glob_still_counts_…` only |

The third is the one worth keeping. It is not a mutation of the shipped code so much
as a test of the design alternative, and it fails on the exact command the tidier
rule would have stopped guarding.
## Workarounds

Write the path **absolutely** — `awk '…' /home/…/tmp/x.rs` instead of `cd /home/…/tmp &&
awk '…' x.rs`. The absolute form reaches the membership branch and is correctly allowed.

`acknowledge_risk: true` also bypasses, but it is the wrong tool here: nothing risky is
being acknowledged, and it suppresses the gate for genuinely in-project reads in the same
command. Renaming the file to a non-source extension works but teaches the wrong lesson.

## Resume

Code is done and gated (`cargo fmt --check`, `clippy --workspace --all-targets -D
warnings`, `cargo test --workspace` — 4116 passed, 0 failed, 50 ignored).

One step left, and it needs a rebuild the session cannot do to itself: **`cargo rb`
then `/mcp`**, and re-run the reproduction from *Symptom* against the live server.
Until then the running MCP binary still carries the old predicate, so probing this
session's own `run_command` will show the OLD behaviour and read as a failed fix —
the same trap `src/prompts/README.md` was corrected for on 2026-08-17.

The two commands to run after the rebuild, and the verdict each must give:

- `cd /tmp/scratch && awk '{print}' head.rs` → runs (was refused)
- `grep -rn 'x' <abs-sibling-repo> --include='*.mjs'` → runs (was refused)
- `cat src/main.rs` → still refused — the control; without it the first two are
  equally consistent with the gate having been removed.
## References

- `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` — GF-3, the audit this is the
  residue of.
- `433100bd` — "fix(il3): stop blocking source reads outside the project" (the partial fix).
  Sibling gate work: `be4a679b` (`wc` returns a count, not content), `90c5aea1` (the
  refusal states the predicate).
- `docs/trackers/codescout-usage-frictions.md` — U-43 (this friction, from the caller's side).
- `src/util/path_security.rs` — `check_source_file_access`,
  `segment_reads_project_source`, `path_is_within_project`.
