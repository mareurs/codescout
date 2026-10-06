---
id: d5f1945653f935aa
kind: bug
status: open
title: 'BUG: the path_security docstring calls ionice an unlisted wrapper, but the wrapper list names it'
tags:
- path-security
- doc-drift
- il3
- cluster/doc-contradicted-by-code
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-30-source-gate-is-bypassed-by-background-ampersand-sudo-xargs-and-command-substitution.md
severity: low
---

# BUG: the `path_security` docstring says `ionice` is a wrapper "not named here", and the same function names it

## Summary

The doc comment on the wrapper-skipping rule in `src/util/path_security.rs` says that a wrapper not on the list, and gives `ionice` as the example, "falls to bounded". `ionice` is on the list. Commit `98c6d798` added it, and the docstring was not updated.

## Symptom (Effect)

Two statements about one function disagree.

`src/util/path_security.rs:1541-1543` says:

```
/// **A closed wrapper list, and the limit is stated rather than hidden:** a wrapper not named
/// here (`ionice`, a shell function) still reads as its own name and falls to bounded
```

The same doc block names it at line 1525: `` `doas [flags]`, `ionice [flags]` ``. The option table at lines 1597-1603 has an `"ionice"` arm, and the match at lines 1628-1629 lists `"ionice"`. The known-limits doc at line 2015 says `` `sudo`, `doas`, `ionice` and `xargs` were closed here ``.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

Run `grep -n ionice src/util/path_security.rs`. Read lines 1525, 1541-1543, 1597-1603, 1628-1629 and 2015.

## Environment

Linux, `experiments` at `10e935e3`.

## Root cause

The sentence was written when `ionice` was an open bypass. `98c6d798` (2026-10-01) closed it, and nobody re-read the sentence. The archived record `docs/issues/archive/2026-09-30-source-gate-is-bypassed-by-background-ampersand-sudo-xargs-and-command-substitution.md` says the same bypass was "closed after the re-measurement" and covers the code change. It does not mention this docstring.

Read from the source — not measured at runtime. The behaviour is not in doubt: the unit tests named in that record pass.

## Evidence

Quoted above. A reader who trusts line 1542 would conclude that `ionice cat src/main.rs` is allowed, when the gate refuses it.

## Hypotheses tried

None needed.

## Fix

Not started. Reword the sentence so its example is a wrapper that is still unlisted, for example `parallel`, which the known-limits doc names as a measured bypass. Keep "a shell function".

## Tests added

N/A — not fixed. A comment cannot be pinned by a behaviour test.

## Workarounds

None needed.

## Resume

One-line comment edit.

## References

- `docs/issues/archive/2026-09-30-source-gate-is-bypassed-by-background-ampersand-sudo-xargs-and-command-substitution.md`.
- Cluster `IC-11`: the prose was true when written, and the code later gained the capability.
