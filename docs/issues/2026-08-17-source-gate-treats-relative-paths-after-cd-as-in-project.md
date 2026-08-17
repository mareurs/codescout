---
id: '70cd189fa2590af3'
kind: bug
status: open
title: 'BUG: the shell-on-source gate counts every relative path as in-project, so `cd <outside> && awk x.rs` is refused with a hint that cannot be followed — the residue of 433100bd'
tags:
- iron-law
- run-command
- path-security
- il3
- gate-firing
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

Not yet implemented. Resolve a relative token against the segment's **effective** cwd
rather than the project root:

- In `check_source_file_access`, the segments are already split on `&&`/`||`/`;`/`|`.
  Track a leading `cd <path>` per segment (and across segments joined by `&&`, since
  `cd x && cmd` is the common form) and pass that as the base for
  `path_is_within_project`'s relative branch.
- **Keep the blocking-direction bias.** No `cd` seen → relative still means inside. A `cd`
  whose target is a variable, unresolvable, or itself relative-and-ambiguous → also inside.
  The gate should only *open* on a `cd` it fully understands; the point is to stop
  unfollowable refusals, not to widen shell access to source.
- `cd` with no argument (`cd`, `cd ~`) resolves to `$HOME`, which is outside the project
  for every real layout — treat it as a resolved target, not an unknown one.

## Tests added

None yet. Planned, alongside the existing `check_source_file_access_at_root` fixture
(`src/util/path_security.rs:2233-2235`):

- `cd_outside_project_then_relative_source_read_is_allowed` — RED today.
- `cd_inside_project_then_relative_source_read_is_still_blocked` — guards the bias; must
  stay green throughout, and is the one a careless fix breaks.
- `unresolvable_cd_target_keeps_blocking` — `cd $DIR && cat x.rs` stays refused.
- Mutation check: drop the `cd`-tracking and confirm only the first test goes red.

## Workarounds

Write the path **absolutely** — `awk '…' /home/…/tmp/x.rs` instead of `cd /home/…/tmp &&
awk '…' x.rs`. The absolute form reaches the membership branch and is correctly allowed.

`acknowledge_risk: true` also bypasses, but it is the wrong tool here: nothing risky is
being acknowledged, and it suppresses the gate for genuinely in-project reads in the same
command. Renaming the file to a non-source extension works but teaches the wrong lesson.

## Resume

Edit `src/util/path_security.rs`: thread an effective-cwd parameter into
`segment_reads_project_source` / `path_is_within_project`, derived from a leading `cd` in
the segment chain that `check_source_file_access` already computes. Write
`cd_outside_project_then_relative_source_read_is_allowed` first and watch it fail with
`shell access to source files is blocked`.

Then re-measure the population the way GF-3 did — count `il3_shell_on_source` refusals in
`usage.db` whose command contains a `cd` to a path outside the project — so the entry
records a size rather than an anecdote. GF-3's own number (25 of 111) came from that query.

## References

- `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` — GF-3, the audit this is the
  residue of.
- `433100bd` — "fix(il3): stop blocking source reads outside the project" (the partial fix).
  Sibling gate work: `be4a679b` (`wc` returns a count, not content), `90c5aea1` (the
  refusal states the predicate).
- `docs/trackers/codescout-usage-frictions.md` — U-43 (this friction, from the caller's side).
- `src/util/path_security.rs` — `check_source_file_access`,
  `segment_reads_project_source`, `path_is_within_project`.
