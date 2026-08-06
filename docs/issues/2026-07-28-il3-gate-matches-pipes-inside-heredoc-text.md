---
id: '56640fb8a33f2bb0'
kind: bug
status: fixed
title: IL3 pipe gate scans heredoc content, so a commit message that describes a pipe is blocked
tags:
- run_command
- tooling
- false-positive
- progressive-disclosure
topic: run_command guards
---

# BUG: IL3 pipe gate scans heredoc content, so a commit message describing a pipe is blocked

## Summary

`run_command`'s IL3 guard rejects an unbounded command piped to a log-trimmer. It decides
by pattern-matching the command **string**, and that string includes the body of a
heredoc. A `git commit -F -` whose message *describes* a piped command is therefore
refused, even though the pipe appears only inside quoted document text that the shell
never interprets.

Low severity, entirely recoverable, and worth recording because of what it blocks: writing
down a piping mistake in the commit that fixes it.

## Symptom (Effect)

```
run_command("git commit -q -F - <<'EOF'
… five distinct errors this session — a liveness filter whose output was then
reused as the population, pgrep | tail -1 picking an orphan, a SHA census
matching frontmatter id: values, ls | wc -l counting .anchors.toml sidecars …
EOF")
```

→

```
IL3 violation — piped `git commit -q -F - <<'EOF' … ` to a log-trimmer. BLOCKED.
```

The `git` at the head of the command supplies the "unbounded LHS", and `| tail -1` inside
the message supplies the "trimmer RHS". Neither is a pipe the shell would execute: the
heredoc is `<<'EOF'`, single-quoted, so not even parameter expansion applies to it.

## Reproduction

```
git rev-parse HEAD    # 0f6815f8, branch experiments
```

1. `run_command` any `<<'EOF'`-heredoc command whose first word is an unbounded producer
   (`git`, `cargo`, …).
2. Put the two characters `| ` followed by `tail`, `head`, `grep`, or `sort` anywhere in
   the heredoc body.
3. The call is refused with an IL3 violation naming a pipe that does not exist.

## Environment

Linux, codescout `experiments` @ `0f6815f8`, `run_command` via the MCP server built at
14:08. Also echoed by the companion plugin's advisory `PreToolUse` hook, which prints the
same diagnosis — so the two surfaces agree, and both are looking at the same string.

## Root cause

**Read 2026-07-29.** `detect_il3_violation` (`src/util/path_security.rs`) analysed the
command as a flat string:

```rust
let mut stages = command.split('|');
let pre_pipe = stages.next().unwrap_or("");
if !stages.any(stage_trims) { return None; }
…
if !is_unbounded_lhs(pre_pipe) { return None; }
```

One `split('|')` over the whole string, with no notion of where a command begins or ends.
A heredoc body is part of that string, so its text becomes pipeline stages.

### A second false positive from the same line

Writing the fix surfaced one the original report missed, with the same root and a wider
blast radius. `pre_pipe` is *everything before the first `|` in the entire command* — not
the left-hand side of that pipe. So in

```
git status --short; …; ls docs/issues/*.md | wc -l; …; rocm-smi … | grep -v "^$"
```

`pre_pipe` is `git status --short; …; ls docs/issues/*.md `, which "starts with `git`" and
therefore reads as an unbounded LHS — while the trimmer that completes the violation
(`grep -v`) lives three segments further on, in an unrelated command. Nothing here is
piped to anything unbounded: the only piping segments are `ls … | wc -l` (bounded LHS,
pure aggregator) and `rocm-smi … | grep -v` (LHS not on the unbounded list). Blocked
anyway.

That one bites far more often than the heredoc case, because `;`-chaining short probes
into one call is the normal way to use `run_command` under a per-call budget.
## Evidence

The same command committed successfully once the two literal pipe characters were replaced
with prose — `pgrep piped to "tail -1"`, and `an ls count that included .anchors.toml
sidecars`. Nothing else changed: same producer at the head, same heredoc form, same
length. So the trigger is the pipe glyphs in the document body and nothing else.

## Hypotheses tried

1. **Hypothesis:** the heredoc itself is the problem — the guard rejects `<<` outright.
   **Test:** the identical command with the pipes reworded committed fine.
   **Verdict:** rejected. The heredoc form is accepted; only its content matters.

## Fix

Fixed on `experiments` — option 1 from below, plus the segment fix the second false
positive required. The gate now looks at shell *structure* rather than raw text, in two
cheap passes that stop short of parsing quoting:

1. **`strip_heredoc_bodies`** — drops everything between a `<<DELIM` / `<<'DELIM'` /
   `<<-DELIM` opener and its terminator line, terminator included. The opener's own line
   survives, so a real pipe there is still seen. `<<<` (here-string) takes no body and is
   left alone — it would otherwise match the opener pattern starting one byte in.
2. **`pipeline_segments`** — splits at top-level `;`, `&&`, `||`, and analyses each
   segment independently. A single `|` is a pipe and stays inside its segment. The
   lookahead keeps `2>&1` from reading as `&&` and a lone `|` from reading as `||`.

The per-segment check is the old body, extracted as `il3_offending_lead` and returning the
offending left-hand side. The hint now names *that segment* rather than the whole chain,
which also makes the error actionable: previously it printed a `pre_pipe` the reader could
not find a pipe after.

**Not done — the companion plugin's mirror.**
`../claude-plugins/codescout-companion/hooks/il3-deny-hook.sh` carries the same regex and
will keep emitting a spurious advisory line on both shapes. It is an echo, not the
enforcer (its own output says so), so the false positive is now cosmetic rather than
blocking. Cross-repo change, tracked separately.
## Tests added

Five in `src/util/path_security.rs`, alongside the 30 existing IL3 tests — all of which
still pass untouched, which is the evidence the restructuring is behaviour-preserving:

- `il3_allows_a_pipe_that_only_appears_inside_a_heredoc_body` — the reported case, using
  a real `<<'EOF'` whose body contains two prose pipes.
- `il3_still_blocks_a_real_pipe_on_the_line_that_opens_a_heredoc` — the paired negative.
  Stripping bodies must not blind the gate to the opener's own command; without this,
  "fix the false positive" could have been satisfied by dropping too much.
- `il3_treats_a_here_string_as_having_no_body` — `<<<` must not swallow the lines after it.
- `il3_analyses_each_semicolon_separated_command_on_its_own` — the second false positive,
  **plus** its negative: a genuine violation in a later segment still blocks, and the hint
  must name that segment. Otherwise the segment split would be a way to smuggle a pipe
  past the gate by prefixing it with `ls;`.
- `il3_segment_split_does_not_confuse_redirection_or_a_lone_pipe` — `2>&1` is not `&&`,
  `||` is a separator, `|` is not.

Every new test that relaxes the gate is paired with one that proves it did not open a
hole. Full gate: 18 binaries, 3463 passed, 0 failed, 44 ignored; clippy clean.
## Workarounds

None needed once this reaches the release binary. Until then (`cargo rb` + `/mcp`),
reword the pipe out of heredoc text — `pgrep piped to "tail -1"` rather than the glyph —
or split a `;`-chained probe into separate `run_command` calls.

The companion plugin's advisory hook will keep printing its warning on both shapes even
after the rebuild; it is noise, not a block.
## Resume

Two follow-ons:

1. **Mirror the fix in the companion plugin's hook**
   (`../claude-plugins/codescout-companion/hooks/il3-deny-hook.sh`), whose regex this
   function's doc comment says it mirrors. Cross-repo, and cosmetic — the hook advises,
   the server enforces — so it can wait, but the two drifting apart is its own small
   hazard: the doc comment claims they match.
2. **Master-side SHA** after cherry-pick.

Worth noting for whoever extends this: neither pass tracks quoting. A `;` or `|` inside a
quoted argument still reads as a separator. That is a deliberate stopping point — the two
shapes fixed here are the ones observed, and full quote-state scanning is the option 2 this
file originally listed. If a third false positive turns up from quoting, that is the
trigger to do it.
## References

- `docs/PROGRESSIVE_DISCOVERABILITY.md` — the IL3 rule this guard enforces, and why it exists
- `src/util/text.rs` — `literal_continuation_mask`, a scanner of the shape option 2 needs
