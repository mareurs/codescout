---
id: '3b65b107c4ad877e'
kind: bug
status: open
title: 'BUG: file-provenance matches a relocating verb that is not a command invocation — a subcommand, or a heredoc body'
tags:
- cluster/addressing-without-an-escape-hatch
- attribution
- file-provenance
- shared-checkout
topic: authorship attribution
closed: null
opened: 2026-09-15
severity: low
---

# BUG: file-provenance matches a relocating verb that is not a command invocation

## Summary

`RELOCATORS` in `scripts/file-provenance.py` decides a token is a relocating verb from its
**shape alone**, with no notion of command **position**. Two consequences, both measured: a
verb that is a *subcommand of another program* (`cargo install`, `apt install`) is read as a
relocation, and a **heredoc body** — text whose entire purpose is to mean *"this is data, not
syntax"* — is scanned as command text. Both attribute writes to sessions that performed none.

This is the residue of `76c83a43c2d1752f`, filed separately because it survives that fix and
needs a different remedy.

## Symptom (Effect)

The same wrong output as its predecessor: `file-provenance.py` names an author for a path
nobody wrote, and the `run_command` hook that attaches authorship to a build red repeats it.

## Reproduction

At `b59a035d`, against the production function, via `importlib`:

```python
spec = importlib.util.spec_from_file_location('fp', 'scripts/file-provenance.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
list(m.relocation_targets(cmd))
```

| `cmd` | result |
|---|---|
| `cargo install ripgrep` | `['ripgrep']` |
| `cargo install --path .` | `['.']` |
| `apt install vim` | `['vim']` |
| a heredoc whose body contains `"mv src/alpha.rs src/beta.rs"` | `['src/alpha.rs', 'src/beta.rs")]']` |
| `install -m 755 build/x /usr/local/bin/x` — **control** | `['/usr/local/bin/x']` |

The fourth row is the sharp one: the captured operand carries **Python syntax** (`")]`),
which is positive evidence the matched text was never a command. The control is what keeps
this a discrimination — a real `install` invocation must keep resolving.

## Environment

`experiments` at `b59a035d`, Python 3, this checkout. Independent of profile and of session.

## Root cause

`scripts/file-provenance.py`, the `RELOCATORS` pattern. Its only left anchor is `\b`, which
is satisfied anywhere a word begins — including after `cargo `, and including inside quoted
string data in a heredoc body. Nothing in the scanner distinguishes *text that a shell will
execute* from *text a shell will pass through as data*.

Measured 2026-09-15 with the table above; the mechanism is read off the pattern and confirmed
at runtime, not inferred.

## Evidence

Self-inflicted, and that is the part worth keeping. The mutation probes run **in this
session** while fixing the predecessor bug embedded relocation verbs inside `python3 - <<'PY'`
heredocs as test fixtures. Every one of those wrote a transcript record asserting the session
relocated files it never touched — so the instrument was polluted by the act of repairing it.

CLAUDE.md § *Parsers Over a Namespace* names four independent shell gates in this process that
each separately decided a heredoc body was command text: IL-3's pipe limiter, the
dangerous-command gate, the source-file gate, and `run_command`'s pipe instrumentation. **This
is the fifth**, found the same way the section predicts — by using the thing, not by reading it.

## Hypotheses tried

None needed; the mechanism was visible in one probe. Recorded so a later reader does not
re-derive it: the predecessor's `(?=\s)` bound does **not** help here, because `cargo install
ripgrep` and a heredoc's `mv a b` both have genuine whitespace after the verb. The two defects
share a namespace and not a remedy.

## Fix

Not attempted. The available anchor is **command position**: require the verb to be preceded
by start-of-input, a newline, `;`, `&`, `|`, or a subshell opener, rather than by any word
boundary.

That answers the subcommand case outright, and most of the heredoc case as a side effect — the
`mv` matched above is preceded by `"`, not by a command separator. **The residue it does not
answer:** a relocation verb at the *start of a line inside* a heredoc body is, to a
line-oriented scanner, indistinguishable from a command. Closing that requires tracking
heredoc extents, which is a real parser rather than a tighter regex, and is likely not worth it
for an instrument whose failure direction should be tuned toward UNKNOWN anyway.

## Tests added

None yet — `tests/file-provenance.sh` is where they belong, in the *relocating verbs* section
that now holds the predecessor's nine. The shape:

- `cargo install ripgrep` yields nothing, **and** `install -m 755 a b` still yields `b`. The
  pair is the discrimination; the first assertion alone is satisfied by deleting `install`
  from the alternation.
- A heredoc-shaped body's verbs yield nothing, **and** the enclosing command's own verbs still
  resolve.

Note both pairs are one absence assertion plus one existence assertion, because absence alone
is monotone under removal and a suppressed matcher would pass it.

## Workarounds

Unchanged from the predecessor: read the hook's attribution as *"this sid's transcript mentions
this path"*, never as *"this sid wrote this path"*. To decide whether a peer holds a file, use
the positive instruments — `git diff --stat` on the shared checkout, `git status --porcelain`
in their worktree, and the `Session-Id` trailer for committed work.

## Classification

`cluster/addressing-without-an-escape-hatch` (`IC-6`), the **no-escape-for-mention** half. A
heredoc is *the* escape construct of the shell namespace, and this scanner has no escape for
it — which is the class stated almost verbatim.

## Resume

Decide whether this is worth fixing at all before writing the anchor. The failure is toward
**over**-attribution, and the tool's whole design bias is that UNKNOWN beats a confident wrong
name — so the fix is cheap and the argument for it is about that bias, not about the two
commands in the table. If it is taken: anchor command position in `RELOCATORS`, add both
assertion pairs above, and mutate each separately — the predecessor's lesson was that two
bounds addressing one symptom guard each other's cases and neither's site.

## References

- `docs/issues/archive/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md` (`76c83a43c2d1752f`) — the predecessor, fixed in `b59a035d`.
- CLAUDE.md § *Parsers Over a Namespace* — the heredoc tell, and the four earlier instances.
