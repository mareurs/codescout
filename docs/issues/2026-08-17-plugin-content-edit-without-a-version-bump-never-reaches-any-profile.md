---
status: mitigated
opened: 2026-08-17
closed: 2026-08-17
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


### An existing detector reported fully green throughout

`claude-plugins/docs/trackers/version-bump-checklist.md` (`cc8cb9e23ab5cc67`, 49 refreshes)
exists precisely to catch cross-profile plugin drift. Its Phase-A gather collects three
checks per (plugin, profile):

| Check | Value during the bug window |
|---|---|
| `installed` == `canonical` | `1.16.7` == `1.16.7` → ✅ |
| `cache_dir_exists` | the `1.16.7` dir was present → ✅ |
| `install_path_matches_profile` | each profile used its own cache root → ✅ |

All three were satisfied for all three profiles while the cached `SKILL.md` was stale. So
the tracker would have rendered an all-✅ table for the entire window in which the change
was inert — not because it was wrong, but because **every check it runs is about version
identity and none is about content identity.** A same-version content edit is invisible to
all three by construction.

That is the sharp form of this bug: it is not that drift detection was missing, it is that
the existing detector's checks are all satisfiable while the served content is wrong.
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

**Immediate: done 2026-08-17.** `codescout-companion` bumped to `1.16.8`; all three
profiles re-synced. Verified rather than assumed:

```
~/.claude      installPath -> .../codescout-companion/1.16.8   1.16.8 step5: HAS new step 5
~/.claude-sdd  installPath -> .../codescout-companion/1.16.8   1.16.8 step5: HAS new step 5
~/.claude-kat  installPath -> .../codescout-companion/1.16.8   1.16.8 step5: HAS new step 5
source plugin.json: "version": "1.16.8"
```

And the refresh route is now established rather than inferred: `1.16.7`'s cached copy is
**untouched**, still carrying its original mtime (`2026-08-16 19:59:54`). Nothing rewrote a
version in place — the new content arrived as a new `1.16.8` directory, and `installPath`
moved to it. `/reload-plugins` materializes and re-points; it does not refresh a
same-version copy. That was the load-bearing assumption in § Root cause and it now has
evidence behind it.

**Status is `mitigated`, not `fixed`,** because only this instance is repaired. The trap —
nothing gates the bump — is untouched, and the evidence in § Root cause is that remembering
failed one commit after succeeding.

**Durable.** Corrected after finding the existing detector (§ Evidence, *An existing
detector reported fully green throughout*) — the cheapest fix is not new infrastructure,
it is **one more field on a tracker that already runs**:

- **a. Add a content check to `version-bump-checklist`'s gather** — the smallest change
  with the widest coverage. Its augmentation prompt already walks every (plugin, profile)
  pair; add a fourth per-pair field beside `cache_dir_exists`:

  > `content_matches_source`: true iff the cached plugin tree at
  > `$HOME/<profile>/plugins/cache/sdd-misc-plugins/<plugin>/<installed>/` matches the
  > repo's `<plugin>/` tree. A recursive `diff -r` (or a digest over both trees) suffices;
  > report false with the first differing path, since that path names the change that is
  > not live.

  This catches the whole class — stale cache, partial sync, hand-edited cache — not just
  the missing-bump cause, and it reuses a refresh cycle that has run 49 times. It detects
  rather than prevents, which is the trade-off against (b).

- **b. A pre-commit hook in `claude-plugins`** refusing a commit that touches
  `<plugin>/**` without also changing that plugin's `.claude-plugin/plugin.json` version.
  Prevents rather than detects, and fires while the author can still fix it. The repo
  already carries hooks (`5f6b336` is an `il3-hook` change), so this is a new rule in an
  existing place.

- **c. A ship-sequence line in CLAUDE.md** § *Three Claude Code Instances*, which already
  owns cross-profile plugin discipline. Needed regardless, so the guard above has a
  written reason and does not get removed later as mysterious. On its own it is another
  rule to remember, and the evidence in § Root cause is that remembering failed one commit
  after succeeding.

Recommend **a first** (one gather field, immediate coverage of the whole class), then
**c**, and **b** only if the same miss recurs — a hook that refuses commits is the kind of
friction worth adding once detection has shown it is needed.

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

The immediate staleness is repaired and verified (§ Fix). What remains is the durable
guard, and it is why this file stays open at `mitigated`.

Next action: add the `content_matches_source` gather field to
`claude-plugins/docs/trackers/version-bump-checklist.md` (`cc8cb9e23ab5cc67`) — Fix (a).
It is an augmentation-prompt edit, so it goes through
`artifact_augment(id="cc8cb9e23ab5cc67", merge=true, prompt=…)`, and the params_schema
needs the field added to each profile object's `required` list alongside
`cache_dir_exists`. That repo is on `main`, so the commit is the user's.

Then re-refresh the tracker and confirm it reports ✅ for the current 1.16.8 state — which
is a real check, not a formality: a content comparison that cannot go red on a known-good
tree is not measuring anything.

Do NOT close this file on the strength of the bump alone. The bump cleared the symptom; the
next content-only commit reproduces the bug exactly, and the existing detector will again
report green while it does.
## References
- `claude-plugins` @ `9d9ecc2` — the content edit that is committed but not live
- `claude-plugins` @ `9877b9d` — the version bump one commit earlier, proving the practice
  exists
- `~/<profile>/plugins/installed_plugins.json` — the `installPath` per profile
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow
- `docs/issues/archive/2026-08-17-mcp-reconnect-does-not-refresh-server-instructions.md`
  (`50038111`, still open) — the sibling "committed but not live" defect, one surface over.
  Worth reading as a pair: both are cases where the artifact on disk is correct and the
  copy actually being served is not, and in both the author is the least able to notice.
