---
id: bb794aff72744ead
kind: bug
status: fixed
title: 'BUG: file-provenance matches a relocating verb that is not a command invocation — a subcommand, or a heredoc body'
tags:
- cluster/addressing-without-an-escape-hatch
- attribution
- file-provenance
- shared-checkout
topic: authorship attribution
closed: 2026-09-15
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

**FIXED 2026-09-15** in `2f32faa3` — patch-id `d24cade6bdd1633ffb186d5be27960e53d12b0b3`.

Shipped as the prescribed **command-position anchor**. `RELOCATORS` now opens with
`(?:^|[;&|(\n])\s*`, so a relocating verb is only a relocation when it *opens a command* —
preceded by start-of-input, a newline, or `;` `&` `|` `(`.

That is a third bound, and it is about **position** where the other two are about **shape**,
which is why neither could stand in for it: in both defects the whitespace around the verb is
real.

**Measured on the live corpus**, the previous shipped version (`b59a035d`) against this one,
over the same transcripts in one pass: **943 transcript files, 927,720 records, 60,528
`Bash`/`run_command` calls**.

| | |
|---|---|
| records **withdrawn** | **866**, over 448 distinct targets |
| records **added** | **0**, over 0 distinct targets |

Both directions were computed, because subtractive-only is the claim and an unmeasured zero
is not evidence for it. Of the 448 withdrawn targets, **392 are not path-shaped at all** —
ordinary English words (`the`, `a`, `in`, `and`) lifted out of prose sitting inside command
text, which resolved to no file and were harmless. The remaining **56 look like paths**, and
those are the reason this was worth shipping: `src/lsp/client.rs`, a real source file, was
being attributed **6** times.

## Known loss, accepted deliberately

A **wrapper word hides the verb behind it, exactly as `cargo` does** — `sudo mv a b` no longer
resolves, and the same holds for `time`, `env`, `xargs`, `nohup`. Nothing distinguishes a
wrapper from a program-with-subcommands without a list of one or the other, and a list is the
same no-escape problem one level in.

The miss degrades to **UNKNOWN**, which is this file's safe direction — a confident wrong name
is the thing it exists to avoid. It is asserted in the suite rather than only commented, so
that widening it later is a deliberate edit to a test and not an accident.

## Residue this does NOT close

A relocation at the **start of a line inside a heredoc body** is, to a line-oriented scanner,
indistinguishable from a command. Closing it needs heredoc-extent tracking — a parser, not a
tighter pattern — and is not obviously worth it for an instrument whose failure direction
should be tuned toward UNKNOWN anyway. Stated here and at the pattern in the source, per the
rule that a documented limitation and a silent reinterpretation cost a reader very different
amounts.

## Tests added

`tests/file-provenance.sh`, in the *relocating verbs* section — **eleven assertions**, taking
the suite 129 → 140. CI already runs this file (`.github/workflows/ci.yml`).

Every case is paired with a control, because each assertion here is an **absence** assertion
and absence is monotone under removal — deleting a verb from the alternation satisfies all of
them at once:

| case | control that keeps it a discrimination |
|---|---|
| `cargo install ripgrep` → nothing | a real `install -m 755 a b` still writes its destination, and its source stays UNKNOWN |
| a heredoc body's quoted relocation → nothing | the command **carrying** the heredoc still resolves its own relocation |
| a line **opening** with `mv.rs` → nothing | a real relocation opening a line still resolves |
| `sudo mv a b` → nothing | asserted as a **known loss**, labelled as such |

**Two fixtures were vacuous on first writing and `verify-RED` caught both** — recorded because
they were passing for reasons that had nothing to do with the defect:

- the heredoc case asserted on the **second** operand, which the tool lifts as
  `src/…beta.rs")]` with the Python syntax attached, so it resolves to no file and the
  assertion was green against the unfixed tool. The first operand comes back clean.
- the wrapper case used `/etc/…`, which `normalize()` discards as out-of-tree **before any
  verb logic runs** — the same trap the section's own `cp source` comment already warned
  about, from 2026-09-01.

## Mutation — four sites, four kills

Run in an isolated copy; control and revert both 140/0.

| mutation | verdict | killed by |
|---|---|---|
| drop the command-position prefix | KILLED | 5 assertions |
| drop `(?=\s)` | KILLED | 2 — the line-opening pair |
| drop the `\n` tail bound | KILLED | 3 |
| widen the position class to admit `.` `"` `/` | KILLED | 2 — the heredoc pair |

**The second row is the finding, and it did not start that way.** Dropping `(?=\s)` first
**SURVIVED**: the two cases written the same day to isolate that bound both put the
verb-shaped filename after a `/`, which the new position anchor already rejects. **Adding a
bound silently un-guarded a site an older bound still owned**, and the suite went green
through all of it. The case that restores the isolation puts the verb-shaped token at the
start of a line, where position is satisfied and only the whitespace bound can refuse it.

The fourth row exists so the position class cannot be quietly widened back — it is the
over-broad direction, which no `drop the bound` mutation reaches.

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

N/A — fixed. Two things a later reader should not re-derive:

- The wrapper-word loss and the heredoc-line-start residue are **known and deliberate**, each
  named at the pattern in `scripts/file-provenance.py` and asserted or documented in the
  suite. Neither is an oversight to re-file.
- If a fourth bound is ever added here, **re-run the full per-site mutation set, not just one
  for the new bound**. This fix is the measured instance of why: a new bound can subsume an
  older one's test cases and leave that older site unguarded with the suite green.

## References

- `docs/issues/archive/2026-09-15-file-provenance-reads-mv-inside-a-filename-and-attributes-a-write.md` (`76c83a43c2d1752f`) — the predecessor, fixed in `b59a035d`.
- CLAUDE.md § *Parsers Over a Namespace* — the heredoc tell, and the four earlier instances.
