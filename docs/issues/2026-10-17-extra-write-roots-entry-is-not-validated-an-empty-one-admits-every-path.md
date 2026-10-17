---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: medium
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md, docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: an `extra_write_roots` entry is not validated, so an empty one admits every path

## Summary

`SecuritySection::to_path_security_config` (`src/config/project.rs`) mapped each
`extra_write_roots` string straight to a `PathBuf`, and `validate_list_limits` checked only the
count. In `classify_write_path_in` (`src/util/path_security.rs`) each entry went through
`canonicalize_through_existing_ancestor`, which returns an empty path unchanged, and
`Path::starts_with("")` is true for every path. So `extra_write_roots = [""]` made every path
writable except the deny list, and `["."]` made the server's cwd a write root. Fixed on branch
`fix/write-allowlist-port-2026-10-17`: the config refuses such an entry when it loads, and the
classifier skips it as well.

## Class fit

No existing class names a list entry that matches everything because an empty string is a prefix
of all strings; it is the same mechanism as the empty-`TMPDIR` case closed in the related issue,
found in a second place. Filed unclassified, and that second sighting is the reason to revisit.

## Symptom (Effect)

With `extra_write_roots = [""]` in `.codescout/project.toml`, a write to `/etc/probe-x` is
`Allowed` where the same call without the entry is `OutsideRoot`.

## Reproduction

Measured 2026-10-17 with `pathsec_probe` against the real library (`classify_write_path`). `before`
is `experiments` 27efb11d, `after` is the branch head with item 6. Targets are written by absolute
path; cwd `fx/plain` unless stated.

| scenario | target | before | after |
|---|---|---|---|
| F9a `extra_write_roots=[""]` | `~/.bashrc` | ALLOWED | outside |
| F9a | `~/.claude/settings.json`, `~/.config/codescout/.env` | ALLOWED | outside |
| F9a | `/etc/probe-x`, `/home/otheruser/x`, `/probe-x` | ALLOWED | outside |
| F9a | `~/.ssh/authorized_keys` | denied | denied |
| F9b `extra_write_roots=["."]`, cwd `~` | `~/.bashrc`, `~/.claude/settings.json`, `~/.config/codescout/.env` | ALLOWED | outside |
| F9b2 `extra_write_roots=["."]`, cwd `/etc` | `/etc/probe-x` | ALLOWED | ALLOWED (the cwd itself is an implicit write root; unchanged) |

Entries that ARE absolute are honoured as before (probed on the branch head, deny list still wins):

| `extra_write_roots` | `/probe-x` | `~/.bashrc` | `~/.ssh/authorized_keys` |
|---|---|---|---|
| `["/"]` | ALLOWED | ALLOWED | denied |
| `["/home"]` | outside | ALLOWED | denied |
| `["/home/marius"]` | outside | ALLOWED | denied |

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

No validation of the entries: empty and relative ones were accepted. The file is the project's own
config, so this needs a bad config line rather than an attacker, but a typo
(`extra_write_roots = [""]`, or an unset variable expanded into an empty string by a generator) is
enough. A relative entry resolved against the server's cwd, not the project.
Measured 2026-10-17: the first table.

## Evidence

Before the change:

```
an_empty_extra_write_root_admits_nothing:  an empty extra root must not become a root, got: Allowed(".../x.txt")
a_relative_extra_write_root_admits_nothing: the relative extra root "." must not become a root, got: Allowed(".../ce_relative_extra_root_probe.txt")
validate_list_limits_rejects_an_extra_write_root_that_is_not_absolute: "" is not an absolute path: ()
```

## Hypotheses tried

None.

## Fix

Commit e80bc432 (item 6), patch-id 003d5ccf8a212122ae0defd36c6122acf05da61a:

- `ProjectConfig::validate_list_limits` (next to `MAX_EXTRA_WRITE_ROOTS`) refuses an entry that is
  not an absolute path, naming the field, the index and the entry (`security.extra_write_roots
  entry 1 ("") is not an absolute path; use a full path (a leading '~' is not expanded)`). The
  project then fails to load, as it already does for an oversized list.
- `classify_write_path_in` ignores any entry that is not absolute, because a config can change
  after it was validated. Only narrows.
- `docs/manual/src/concepts/security.md` says entries must be absolute.

Behaviour change to expect: a project whose `project.toml` carries a relative or `~` entry stops
loading until the entry is made absolute. No such entry exists in the repo's docs or tests.

NOT changed, a design question: an absolute entry may be as broad as `/` or a home (table above);
that is an explicit operator choice, the deny list still wins, and narrowing it is not obviously
right.

## Tests added

`src/config/project.rs`: `validate_list_limits_rejects_an_extra_write_root_that_is_not_absolute`
(red before). `src/util/path_security.rs`: `an_empty_extra_write_root_admits_nothing`,
`a_relative_extra_write_root_admits_nothing` (both red before),
`an_absolute_extra_write_root_beside_an_unusable_one_still_works`.

### Mutation results (one guard per mutant, 2026-10-17)

Same method as in `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`.

| mutant | killed by |
|---|---|
| m6a config accepts a non-absolute entry | `validate_list_limits_rejects_an_extra_write_root_that_is_not_absolute`: KILLED |
| m6b classifier keeps non-absolute entries | `an_empty_extra_write_root_admits_nothing`, `a_relative_extra_write_root_admits_nothing`: KILLED |
| m6c config checks the first entry only | same config test (the `entry 1` assertion): KILLED |
| m6d config test inverted (rejects absolute) | same config test: KILLED |

## Workarounds

Do not write an empty or relative entry.

## Resume

Decide whether an absolute entry that is `/`, a home or above one should be refused or warned
about; nothing else is open.

## References

- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
