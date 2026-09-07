---
id: c0ecdba1574b68bb
kind: bug
status: open
title: 'BUG: a file whose name matches `@ack_<8hex>` is unreachable by `run_command`, with no escape'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
topic: run_command handle interpolation
closed: ''
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

## Hypotheses tried

1. **Hypothesis:** quoting escapes the check.
   **Test:** `ls -l "./@ack_639fc11a"`.
   **Verdict:** rejected — identical refusal. The check runs before the shell sees the
   string, so quoting cannot reach it.

## Fix

Not started. The class is `CLAUDE.md` § *Parsers Over a Namespace*, which says a parser
over a namespace owes an escape **and** a disambiguator. Options:

- **A — anchor the match.** Require the handle to be the entire command (`^@ack_…$`),
  which is the only form the hint tells callers to use. Narrowest change; the ack feature
  is documented as `run_command("@ack_<id>")` standalone, so an interior occurrence is
  already not a supported invocation.
- **B — add an escape.** Honour a backslash or a `./` prefix. More faithful to shell
  intuition but requires the check to do enough parsing to know what it is looking at,
  which is the thing that made this wrong.
- **C — say so at the refusal site.** If no escape is affordable, the error should name
  the limitation and the `find -exec` workaround instead of pointing at a handle that does
  not exist.

**Recommendation: A, plus C's wording fix.** A removes the collision entirely for the
documented usage; C repairs a hint that currently sends the reader somewhere useless.

## Tests added

None yet. A regression test is cheap: create a file named `@ack_deadbeef` in a temp
project and assert `ls` on it succeeds. Note the test must assert on the **success**
direction — an assertion that the refusal fires is monotone under keeping the bug.

## Workarounds

Never write the token. Use a glob that stops short of the hex:

```
find . -maxdepth 1 -name '@ack*' -exec <cmd> {} \;
```

Native `Bash` is also unaffected — the check lives in codescout's `run_command` only.

## Resume

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
