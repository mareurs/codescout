---
status: open
opened: 2026-08-17
closed:
severity: medium
owner: marius
related: []
tags: [companion-plugin, skills, ship-sequence, three-profiles, silent-failure]
kind: bug
---

# BUG: a plugin content edit without a version bump is committed, correct, and live nowhere

## Summary

The plugin cache is keyed by the version in `plugin.json`, and each profile keeps one
real directory per version. Editing a skill's content without bumping that version
leaves the cache key unchanged, so nothing re-syncs and every profile keeps serving the
pre-edit copy. The change is committed, reviewed, and in force nowhere — with no error,
no warning, and nothing in `git status` to suggest it.

## Symptom (Effect)

Measured 2026-08-17, immediately after `9d9ecc2` in `claude-plugins` landed the
three-sub-step archive procedure into `codescout-companion/skills/tracker-hygiene/SKILL.md`:

```
~/.claude:      STALE — no three-sub-step text
~/.claude-sdd:  STALE — no three-sub-step text
~/.claude-kat:  STALE — no three-sub-step text
source of truth: has it
```

The cached copy still carries the two-step form:

```
134,135c134,154
< 5. **Archive through the catalog:** `artifact(update, patch={status:"archived"})`
<    then `artifact(move, new_rel_path="docs/trackers/archive/<name>.md")`.
---
> 5. **Archive through the catalog, in three steps — not two.** Per
>    `archive-cadence-policy` § 3 (amended, ratified 2026-08-17):
```

So any session invoking `/codescout-companion:tracker-hygiene` today reads the archive
step that omits the citation repoint — the exact omission the commit was written to
close.

## Reproduction

`claude-plugins` @ `9d9ecc2`; codescout @ `f6140205`.

1. Edit any file under `codescout-companion/` without touching
   `codescout-companion/.claude-plugin/plugin.json`.
2. Commit it.
3. `grep` the edited text in
   `~/<profile>/plugins/cache/sdd-misc-plugins/codescout-companion/<version>/...`
   for each of the three profiles → absent in all three.

## Environment

Linux. Three Claude Code profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`), all
three resolving `codescout-companion@sdd-misc-plugins` to their **own** cache root — no
cross-profile drift, so the 2026-05-16 `installPath` note in CLAUDE.md is not implicated
here.

## Root cause

The cache is a **version-keyed copy**, not a link to source. *Measured 2026-08-17:*

- `ls -ld` on the cache dir shows a real directory (`drwxr-xr-x`), and its content
  differs from source — so it is a copy, not a symlink to the plugin repo;
- sibling version dirs `1.16.2 … 1.16.7` all exist side by side, one per released
  version — confirming the version string is the cache key;
- source `codescout-companion/.claude-plugin/plugin.json` still reads
  `"version": "1.16.7"`, which is a key that **already exists** locally.

Unchanged key → nothing to fetch → the pre-edit copy stays authoritative. There is no
content hash in the path, so a same-version content change is unrepresentable in the
cache.

**The practice that prevents this already exists and is simply not enforced.** The
commit immediately before the failing one is `9877b9d`,
*"chore(codescout-companion): 1.16.6 -> 1.16.7 for the il3 git-hook sync"* — a bump whose
entire purpose was to publish a content change. So the ship sequence is known and was
followed one commit earlier; nothing gated it on the next, and it failed silently.

That is what makes this worth a bug rather than a note: the failure mode is invisible at
every checkpoint a careful author would look at. `git status` is clean, the diff is
correct, the tests (which read the source tree, not the cache) pass, and the skill file
on disk says the right thing.

## Evidence

### The three checkpoints that all report success

| Checkpoint | Reports | Actually |
|---|---|---|
| `git status` in `claude-plugins` | clean | change committed |
| reading `skills/tracker-hygiene/SKILL.md` | new text | source only |
| a session invoking the skill | — | **serves the old text** |

Only the third matters, and it is the only one nobody inspects.

## Hypotheses tried

1. **Hypothesis:** the cache is a symlink to the plugin source (directory-source
   install), so a source edit is live immediately.
   **Test:** `ls -ld` the cache dir and diff its SKILL.md against source.
   **Verdict:** rejected — real directory, content differs. `scripts/discover-specialists.sh`
   in the `buddy` plugin does note that some installs point `installPath` at a source dir,
   which is presumably where this hypothesis comes from; it is not the case here.
2. **Hypothesis:** `/mcp` reconnect re-syncs plugins.
   **Verdict:** rejected — `/mcp` restarts MCP servers. Plugin skills are not MCP
   resources, and the caches were re-checked after a reconnect and remained stale. Cf.
   `50038111`, which established the neighbouring fact that `/mcp` refreshes tool schemas
   but not server instructions.

## Fix

**Immediate:** bump `codescout-companion/.claude-plugin/plugin.json` to `1.16.8` and
commit, then let each profile re-sync. One line, in `claude-plugins` on `main`.

**Durable, and the actual ask:** make the bump non-optional rather than remembered.
Options, cheapest first:

- **a. A pre-commit hook in `claude-plugins`** that refuses a commit touching
  `<plugin>/**` unless that plugin's `plugin.json` version also changed. Mechanical, and
  it fires at the moment the author can still fix it.
- **b. A ship-sequence line in CLAUDE.md** § *Three Claude Code Instances*, which already
  owns cross-profile plugin discipline. Necessary documentation either way, but on its
  own it is another rule to remember — and the evidence above is that remembering failed
  one commit after doing it correctly.
- **c. A staleness check** comparing each profile's cached copy against source at session
  start. Catches every drift cause, not just this one, but it is the most machinery and
  it reports the problem after the fact rather than preventing it.

Recommend **a + b**: the hook prevents it, the CLAUDE.md line explains why the hook
exists so nobody deletes it.

## Tests added

None — this is not codescout code. The equivalent guard is Fix (a), whose own test is
that a content-only commit to `claude-plugins` is refused.

## Workarounds

Bump the version with every content change. To verify a change is actually live rather
than merely committed:

```
grep -l '<new text>' ~/.claude*/plugins/cache/sdd-misc-plugins/<plugin>/*/skills/**/SKILL.md
```

Three hits — one per profile — means live. Fewer means the bump is missing or a profile
has not re-synced.

## Resume

Bump `codescout-companion` to `1.16.8` in `claude-plugins` (on `main`, so the user's
call), then re-run the workaround grep above and confirm three hits. After that, decide
between Fix (a) and (c) for the durable guard; (b) should land regardless.

## References
- `claude-plugins` @ `9d9ecc2` — the content edit that is committed but not live
- `claude-plugins` @ `9877b9d` — the version bump one commit earlier, proving the practice
  exists
- `~/<profile>/plugins/installed_plugins.json` — the `installPath` per profile
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow
- `docs/issues/2026-08-17-mcp-reconnect-does-not-refresh-server-instructions.md`
  (`50038111`, still open) — the sibling "committed but not live" defect, one surface over.
  Worth reading as a pair: both are cases where the artifact on disk is correct and the
  copy actually being served is not, and in both the author is the least able to notice.
