---
id: 92398b90d9d86891
kind: bug
status: fixed
title: 'BUG: a file whose name matches `@ack_<8hex>` is unreachable by `run_command`, with no escape'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
topic: run_command handle interpolation
closed: 2026-09-07
opened: 2026-09-02
owner: marius
related: []
severity: low
---

# BUG: a file whose name matches `@ack_<8hex>` is unreachable by `run_command`, with no escape

## Summary

`run_command` refuses any command whose text contains `@ack_[0-9a-f]{8}` anywhere, so a
real file with that name cannot be listed, read, moved or deleted through the tool.
Quoting and a `./` prefix do not help — the check runs on the raw command string before
any shell parsing. Found live: a peer session left a 4 KB commit message at
`./@ack_639fc11a` in the repo root, and no `run_command` invocation can name it.

## Symptom (Effect)

```
run_command("ls -l @ack_639fc11a")
run_command("ls -l \"./@ack_639fc11a\"")
```

Both return, without executing anything:

```
ack handle cannot be used for interpolation
hint: Use run_command("@ack_<id>") directly to execute a pending acknowledgment.
```

The hint is actively misleading here: invoking the handle "directly" would attempt to
execute a *pending acknowledgment* that does not exist — the string is a filename, not a
handle.

## Reproduction

```
git rev-parse HEAD          # d5d37b250aefe40f8817f0a0d092003bf5e3def2
touch '@ack_deadbeef'
```

Then any of these is refused rather than run:

- `run_command("ls -l @ack_deadbeef")`
- `run_command("ls -l './@ack_deadbeef'")`
- `run_command("cat @ack_deadbeef")`
- `run_command("rm @ack_deadbeef")`

**Workaround that does work** — never name the token:

```
find . -maxdepth 1 -name '@ack*' -exec head -c 400 {} \;
```

Verified live 2026-09-02 against `./@ack_639fc11a`; the `find` form read the file
contents where every direct form was refused.

## Environment

codescout MCP, branch `experiments`, Linux. Not environment-sensitive — the check is a
pure string match on the command text.

## Root cause

`src/tools/output_buffer.rs:620-630`:

```rust
static ACK_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
if ACK_RE
    .get_or_init(|| Regex::new(r"@ack_[0-9a-f]{8}").expect("valid regex"))
    .is_match(command)
{
    return Err(RecoverableError::with_hint(
        "ack handle cannot be used for interpolation",
        "Use run_command(\"@ack_<id>\") directly to execute a pending acknowledgment.",
    )
    .into());
}
```

`is_match` over the **whole command string**, unanchored. Three consequences, in
increasing order of severity:

1. The pattern matches **anywhere**, not just where an argument sits, so a mention in a
   quoted string, a heredoc body, a commit message or a comment counts.
2. There is **no escape**. The check precedes shell parsing, so quoting, `./`, `$'…'` and
   backslashes are all invisible to it — every form a shell user would reach for is
   defeated identically.
3. The refusal is **whole-command**, not per-argument, so a pipeline is rejected in full
   because of one token.

Measured 2026-09-02: both the bare and the quoted-`./` form refused; `find -name '@ack*'`
succeeded on the same file in the same session.

## Evidence

The live instance is a peer's in-flight commit message, not a synthetic file:

```
./@ack_639fc11a  4009 bytes  2026-09-02 22:37
feat(librarian): a resumable chunk backfill that escapes the indexer's absorbing state
…
```

It sits untracked in the repo root and is invisible to any `run_command` that names it,
including a cleanup `rm`.

**Second sighting, 2026-09-07, by sessionId `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`, which
was not looking for it.** Stronger than the original for that reason: this file's own
filing session had to name the case deliberately, whereas the rediscovery arrived during
unrelated arithmetic — counting untracked paths out of `git status` to settle which of them
belonged to one authorship cluster. The refused call was an eight-line script in which the
token appeared on a single `stat` line among seven others; the whole script was refused and
nothing ran. So the defect is reached by ordinary composition, not only by naming the file
as a bare argument, and a session with no knowledge of this bug meets it while doing
something else entirely.

## Hypotheses tried

1. **Hypothesis:** quoting escapes the check.
   **Test:** `ls -l "./@ack_639fc11a"`.
   **Verdict:** rejected — identical refusal. The check runs before the shell sees the
   string, so quoting cannot reach it.

## Fix

Fixed at `bd3d0973`, patch-id `31fc06775f8341fe77b3b367fc9ec25346debb18`
(`git show <sha> | git patch-id --stable`), by sessionId
`ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`.

**What shipped: none of A/B/C — a live-handle lookup.** The refusal is now gated on
`self.get_dangerous(token).is_some()` rather than on the token's shape. That is the same
disambiguator the sibling `REF_RE` arm below already applies to its own namespace, so the
two arms finally agree on what a handle is. A token matching a live pending ack is still
refused (the guard's real purpose, `cat @ack_xxx` meaning "interpolate this"); a token that
matches nothing is a filename and passes through byte-identical.

**A was the recorded recommendation and would have been DEAD CODE — do not retry it.**
`run_command`'s early dispatch calls `looks_like_ack_handle(command)` and returns
(`src/tools/run_command/mod.rs:185`) *before* `resolve_refs` is ever reached, and
`resolve_refs` has exactly one production caller (`mod.rs:215`, after that dispatch). So by
the time this guard runs, the command is never a bare handle. Anchoring to `^@ack_…$` would
therefore have matched nothing, made every test of the guard vacuous, and "fixed" the bug by
silently deleting the feature. The recommendation was written without tracing the caller —
which is the § *Testing Discipline* point about a guard nothing reaches, arrived at from the
fix side rather than the test side.

B (an escape) was unnecessary once the lookup existed: the ambiguity is not lexical, so no
escape syntax was owed. C's wording fix shipped as part of the same change — the hint now
names the token, says how to execute it alone, and states that a file of that name is not
refused, so both branches of the reader's question are answered at the refusal site.

**Known residual ambiguity, stated rather than hidden:** a file named identically to a
*live* pending ack is still refused. That case is genuinely ambiguous, refusing is the safe
side, and the hint now says so.
## Tests added

Two, both in `src/tools/output_buffer.rs`, both asserting the **success** direction — an
assertion that the refusal fires is monotone under keeping the bug.

- `resolve_refs_distinguishes_a_live_ack_handle_from_a_file_of_the_same_name` — drives both
  branches with **one token**, varying only whether the ack is stored. This is the load-bearing
  one: two tests using two *different* tokens would let a shape-only reimplementation keep
  passing, because shape cannot separate the two cases. Mutating the fix back to `is_match`
  fails its second half.
- `resolve_refs_allows_a_script_that_merely_mentions_an_ack_shaped_filename` — a multi-line
  script with the token on one `stat` line, pinning that the refusal was whole-command and no
  longer is.

The pre-existing `resolve_refs_rejects_ack_handle_interpolation` still passes unchanged: it
stores a real handle via `store_dangerous`, so it exercises the branch the fix preserves.
Observed RED before the fix and GREEN after — both new tests failed with the exact reported
error text.

Gate green 2026-09-07 at `bd3d0973`: FMT=0, CLIPPY=0, LEAN=0 (3627 tests), DEFAULT=0 (5571
tests, 0 failures).

**Not yet verified against a running MCP server.** The fix is in source; the live server
runs a release binary built before it, so the defect still reproduces in-session until
`cargo rb` and an `/mcp` reconnect. Nothing in the tests depends on that.
## Workarounds

Never write the token. Use a glob that stops short of the hex:

```
find . -maxdepth 1 -name '@ack*' -exec <cmd> {} \;
```

A single-character `?` glob is lighter and works positionally, so the path goes straight to
an ordinary command instead of through `find -exec`:

```
stat -c '%y  %s bytes  %n' ./?ack_639fc11a
head -12 ./?ack_639fc11a
```

`?` matches the `@` and keeps the literal token out of the command text. Both escapes route
*around* the name rather than through it, which is this cluster's signature: neither is
provided by the parser, and a caller who does not already know the bug has no way to derive
either one.

Native `Bash` is also unaffected — the check lives in codescout's `run_command` only.

## Resume

**SUPERSEDED — fixed at `bd3d0973`; this section is retained for its fixture note only.**
The A/B/C decision below was not taken: see § *Fix* for why anchoring would have been dead
code. **The fixture warning is also discharged.** `./@ack_639fc11a` is no longer this bug's
only reproduction — two unit tests now cover both branches without touching the filesystem,
so that file may be deleted by whoever owns it without silently disarming anything. It is
not deleted here: its authorship is open (its content is the commit message of `488192e8`,
from a different work stream) and it is not this session's to remove.

Decide between A/B/C in § *Fix*, then change `src/tools/output_buffer.rs:620-630`. The
same block has a sibling `REF_RE` at `:632` for `@cmd_*` / `@tool_*` / `@file_*` handles —
**check whether it has the same shape before fixing only the ack arm**, since those
namespaces are far likelier to collide with a real filename than `@ack_` is.

Do **not** delete `./@ack_639fc11a` — **it is this bug's only reproduction fixture.** § *Symptom*
and the `**Test:**` line both address that exact filename, and there is no other file in the tree
whose name matches `@ack_<8hex>`. Delete it and every check in this file silently becomes
unrunnable while the bug stays open — the file would still read as complete.

**The reason this line originally gave has expired, and that is why it now gives a different
one.** It read: *"as of 2026-09-02 22:37 it is a peer session's in-flight commit message."* That
was true and is no longer — the commit landed as `488192e8` (`feat(librarian): a resumable chunk
backfill that escapes the indexer's absorbing state`), which is an ancestor of HEAD as of
2026-09-07. A reader who checked the stated reason would have found it discharged and deleted the
fixture, correctly following the note. The durable reason was in this file the whole time, one
section away, and unstated here.

(`CLAUDE.md` § *Testing Discipline*: annotate a fixture's load-bearing detail on the fixture line,
saying what breaks if it goes. A bare "do not delete" — or one resting on a fact with a shorter
half-life than the fixture — does not survive the tidy-up it exists to prevent.)

**Verified still live 2026-09-07** at `4b30601c`: `src/tools/output_buffer.rs:620-630` still
rejects unconditionally with no escape, and the sibling `REF_RE` at `:632` still has the same
shape, so the § *Fix* note about checking both arms together still applies.

## References

- `CLAUDE.md` § *Parsers Over a Namespace* — the class, and the "owe an escape" rule
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- `docs/issues/archive/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md` —
  a different ack-handle defect (handle unresolvable), not this collision
- `docs/issues/archive/2026-08-31-dangerous-command-gate-scans-heredoc-body.md` — the same
  "scanner reads data as syntax" shape in a sibling gate
