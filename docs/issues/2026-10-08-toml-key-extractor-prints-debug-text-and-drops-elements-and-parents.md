---
id: '52efc05f9241f73f'
kind: bug
status: open
title: 'BUG: toml_key prints Rust debug text for a scalar and returns only the first element, the child without its parent, and the whole file as line_range'
tags:
- cluster/unclassified
- read_file
- toml_key
opened: 2026-10-08
owner: marius
related:
- docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md
severity: low
---

# BUG: toml_key prints Rust debug text for a scalar and returns only the first element, the child without its parent, and the whole file as line_range

## Summary

`extract_toml_key` (`src/tools/file_summary/file_summary.rs`, about line 1588) gives wrong or partial text for
four shapes of key. They are older than the 2026-10-08 value-paging change, which pages whatever the extractor
returns and did not touch it.

## Symptom (Effect)

1. A scalar key comes back as Rust debug text: `t = "t"` read with `toml_key=t` returns `String("t")`.
   `toml::to_string_pretty` fails for a scalar and the code falls back to `{current:?}`. A multi-line string
   becomes ONE line of escaped text, which paging cannot read.
2. `toml_key=k` on an array of tables (`[[k]]` three times) returns only the first element.
3. `toml_key=a` on `[a.b]` returns `[b]` and its body, without the parent name.
4. On the full-parse path `line_range` is the whole file, not the lines of the key.

## Reproduction

Live binary, 2026-10-08 (cases 1 to 3 reproduced by the controller; 4 reproduced by an implementer in a test):

- `t = "t"\nn = 1\n`, `toml_key=t` gives the text `String("t")`.
- `[[k]]\nname = "a"\n\n[[k]]\nname = "b"\n\n[[k]]\nname = "c"\n`, `toml_key=k` gives `[[k]]\nname = "a"\n`.
- `top = 1\n\n[a.b]\ny = 2\n`, `toml_key=a` gives `[b]\ny = 2\n`.
- `top = 1\nk.x = 1\nk.y = 2\nz = 3\n`, `toml_key=k` gives `x = 1\ny = 2\n` with `line_range` `[1,4]`.

## Environment

codescout on `experiments`. Linux. The same output on the baseline before the value-paging change.

## Root cause

Unknown — under investigation. The first read of `extract_toml_key` points at the serializer fallback for
case 1; cases 2 to 4 have not been traced.

## Evidence

The four calls above.

## Hypotheses tried

None yet.

## Fix

Not started.

## Tests added

N/A — not fixed.

## Workarounds

Read the file by line range, or use `grep` for the key.

## Resume

Read `extract_toml_key` and `extract_yaml_key`. Decide what a key means for an array of tables and for a
parent table, then pin each case with a test.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
