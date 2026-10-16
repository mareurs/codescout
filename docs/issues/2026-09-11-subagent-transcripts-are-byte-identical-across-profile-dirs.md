---
id: '1d944ec938b7ba2d'
kind: bug
status: mitigated
title: '228 subagent transcripts exist as byte-identical copies across 2-3 of this machine''s three CC profile dirs, origin unestablished'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- subagents
- transcripts
- cross-profile
topic: subagent guide delivery / transcript corpus hygiene
unverified: 'Origin of the cross-profile copies is not established (measured 2026-10-16: still occurring, mtimes preserved); only the probe''s handling of them is fixed.'
---

## Symptom

While re-deriving the parent/subagent guide-injection join for
`docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md` (Investigation
2026-09-11), a corpus scan across this machine's three Claude Code profile dirs
(`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) found **228 of 1,606 distinct
`(parent-session-uuid, agent-transcript-filename)` keys present under 2 or 3 of the three
profile roots simultaneously**, with **byte-identical content** (verified by md5, not just
matching size) and **different inodes** (verified via `ls -li` — not a hardlink or symlink;
genuinely separate copies on disk).

Example: `agent-a4328088388be50da.jsonl` under parent session
`c95ba99b-17b2-4fe8-a8a5-afed8deb49c6`, project `-home-marius-work-claude-codescout`, exists
identically (13,039,494 B, md5 `43c6dd7e...`) under all three of
`~/.claude/projects/.../subagents/`, `~/.claude-sdd/projects/.../subagents/`, and
`~/.claude-kat/projects/.../subagents/`.

## Why this matters

Any corpus-scale measurement that walks all three profile dirs (as
`scripts/probe_guide_injection.py` and the ad-hoc join in the 2026-09-11 investigation both
do) triple-counts these 228 sessions unless explicitly deduped by
`(parent-session-uuid, filename)` rather than by path. For the guide-injection join this did
not materially change the final rate (98.1% raw vs 98.4% deduped), but that is not
guaranteed for every future measurement over this corpus, and nothing currently warns a
reader that raw per-profile counts include this duplication.

## What is NOT established

- **Origin unknown.** Whether this is a deliberate backup/sync step, a manual copy done at
  some point, a Claude Code behavior under some condition (e.g. profile migration), or
  something else. Not investigated — out of scope for the guide-injection bug this was
  found while working on.
- **Scope unknown beyond subagent transcripts.** Whether main-session transcripts
  (`<project>/<uuid>.jsonl`, no `/subagents/`) show the same duplication was not checked.
- **Whether this is still happening now** (i.e., an active ongoing sync) or is residue from
  a past one-time event. Not checked — would need mtime comparison across the duplicate
  triples, which was not done here.

## Reproduction

```python
# dedupe key: (parent_uuid, agent_filename); group by key across the three profile roots;
# report keys with >1 distinct profile; verify content equality via md5, not size.
```
Full script used: ad-hoc, not committed — see the investigation section above for the
exact logic (group subagent transcripts by `(parts[idx-1], p.name)` where `idx` is the
`"subagents"` path component; compare md5 across the group).

## Measured 2026-10-16 (answers two of the three "NOT established" items)

Every `*.jsonl` under the three profiles' `projects/` (4,528 files), grouped by basename:

- 619 names occur in more than one location (other than the two generic names below). Subagent
  files: 584 groups with identical size, 6 whose copies differ in size, every size-differing
  pair a byte-prefix of the larger (one copy was taken earlier in the file's growth). Main-session
  files: 26 prefix-related, 1 not a prefix (same name, diverged content). Profile sets: 226
  `~/.claude`+`~/.claude-kat`, 169 `~/.claude`+`~/.claude-sdd`, 146 all three, 73
  `~/.claude-kat`+`~/.claude-sdd`.
- So duplication is **not** subagent-only: 29 main-session names are duplicated too.
- Still happening: the newest duplicated file's mtime is the day of measurement, and 495 of 617
  groups carry an identical `st_mtime_ns` across copies, i.e. the copy preserves mtime. 3
  agent ids appear under two different parent sessions inside a single profile as hardlinks,
  which is not a cross-profile copy at all.
- Origin is still **not** established. Preserved mtimes say the copier preserves them; they do
  not say who copies.
- Two basenames are not sessions: `session.jsonl` (573 files, one per prompt-test directory)
  and `skill-injections.jsonl` (6). Keying a collapse on the basename merges them.

## Resolution (probe side)

`scripts/probe_guide_injection.py` already collapsed copies, but keyed on the basename alone, and
said nothing about how many it dropped. Now:

- `session_key` keys `<uuid>.jsonl` and `agent-<id>.jsonl` on the basename (the same agent id under
  two parents is one subagent, so `(parent, filename)` would double-count those 3) and any other
  name on its path under `projects/`, so 573 `session.jsonl` files stay 573 sessions.
- `collect` prints `duplicate copies collapsed: N (M differ in size)` on its summary line. Today:
  `unique sessions: 3171  duplicate copies collapsed: 763 (39 differ in size)`.
- The corpus figures are unchanged: the old and new scripts, run back to back on today's corpus,
  produce byte-identical `--json` output (no `session.jsonl` has an assistant model line, so none
  is parsed). The defect was latent; the report line is the lasting change.

Tests: `tests/test_probe_guide_injection_dedupe.py` (8 cases; 4 red before the fix: generic
filenames collapsed, report line absent, empty-profile line absent; controls for uuid/subagent
copies, unreadable transcript, missing profile).

Not done: the origin of the copies (a process, a sync, or a harness behaviour) and whether the
one non-prefix main-session pair is a fork. Those need a look at what writes into the other
profiles' `projects/` dirs, not at the probe.
