---
id: f41bfeb963dcee56
kind: bug
status: fixed
title: probe_guide_section_use.py's MECHANISM_TOOLS omits the renamed doc tool and goes blind monotonically
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-03
opened: 2026-09-03
owner: marius
related: []
severity: medium
unverified: 'No regression test. `scripts/` has no test harness in this repo, so nothing gates `MECHANISM_TOOLS` against the next rename — the same defect can recur exactly as it did here, and the fix''s evidence is an observed before/after rather than a guard. The union is also unbounded in principle: a THIRD name would go undetected the same way, and the durable remedy named in the Fix section (derive the list from the served registry) is NOT implemented. Separately, the documented `"doc"` substring greediness is recorded at the site but not enforced — a future `mcp__codescout__docs_*` tool would be silently counted as mechanism operation, and only a reader of the comment would know.'
---

# BUG: `MECHANISM_TOOLS` omits `doc`, so the section-use probe goes blind as the rename propagates

## Summary
`scripts/probe_guide_section_use.py` decides whether a tool call counts as guide-section engagement by substring-matching the tool name against `MECHANISM_TOOLS = ("artifact", "librarian")`. The 2026-09-02 tool collapse renamed `artifact` / `artifact_event` / `artifact_augment` / `artifact_refresh` to **`doc`**, which matches neither entry. Every `doc` call is therefore invisible to the probe. The blindness is small today, grows **monotonically** as sessions pick up the new binary, and runs in exactly the direction that manufactures apparent "waste".

## Symptom (Effect)
No error. The probe reports engagement figures that silently omit every `doc` call, so a session that *did* engage a served section scores as having engaged nothing. Effect is a downward bias on engagement — i.e. an upward bias on "never engaged" — which is the number the probe exists to produce.

## Reproduction

```
git rev-parse HEAD                                  # 88311708 at filing
grep -n "MECHANISM_TOOLS" scripts/probe_guide_section_use.py
```

```
103:MECHANISM_TOOLS = ("artifact", "librarian")
191:    return any(t in name for t in MECHANISM_TOOLS)
```

Corpus exposure, measured 2026-09-03 across all three profile roots:

```
$ grep -rl "mcp__codescout__doc" ~/.claude*/projects | wc -l
9
$ grep -rl "mcp__codescout__artifact" ~/.claude*/projects | wc -l
1373
```

## Environment
Linux, Python 3, codescout `experiments` @ `88311708`. Reads Claude Code transcripts from `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`.

## Root cause

`is_mechanism_tool` (`scripts/probe_guide_section_use.py:190-191`) is a substring test over `MECHANISM_TOOLS` (`:103`). The tuple predates the 2026-09-02 collapse in which `artifact*` became `doc` (`6e8b4170`, `dac1068a`). `"artifact" in "mcp__codescout__doc"` is false and `"librarian" in "mcp__codescout__doc"` is false, so the call is dropped before any section attribution runs.

`librarian` still matches, so the probe is **partially** blind rather than dead — which is worse for a reader, because output still looks well-formed and differentiated.

measured 2026-09-03: the two `grep -rl … | wc -l` counts above — 9 of 1,382 relevant transcripts already carry the new name.

**The exposure is time-dependent and one-directional.** A session keeps its old binary until `cargo rb` + `/mcp`, so the cutover propagates slowly; but every future session moves the ratio the same way. A run today is nearly correct; the same command in a month is materially wrong, with no signal that anything changed.

This is `cluster/selector-narrower-than-its-population` — `MECHANISM_TOOLS` is a selector whose name claims "the mechanism tools" while its membership is a hard-coded pair. It is the same class as its sibling filed the same day, and both are **author-written selectors**, which IC-18's remedy column names as the unremedied half: *"partial — nothing reaches an author-written selector."*

## Evidence

### `docs/PROBES.md` names this trap on a different row
The `probe_tool_surface.py` row carries an explicit rename warning:

> *"`usage.db` `tool_name` cuts over on the ship date of the 2026-09-02 collapse: `artifact` → `doc` … A 30-day join spanning that date MUST union both names, or every per-tool figure for the renamed six is a floor that reads like a total."*

The identical hazard exists in this probe and is documented on **neither** its PROBES row nor in the script. The knowledge was in the repo; it just was not where this instrument's reader would look.

## Hypotheses tried

1. **Hypothesis:** the omission is harmless because the corpus is almost entirely pre-rename.
   **Test:** counted transcripts by tool name across all three profiles.
   **Verdict:** rejected as a reason not to fix — 9/1,382 is small **today**, but the ratio only moves one way and nothing surfaces the drift. Confirmed as a reason the current numbers are not badly wrong yet.
   **Evidence link:** § *Reproduction*.

## Fix

**Shipped 2026-09-03.**

- **SHA:** `d4ee86da` on **`experiments`** — positional; it dies when `experiments` is rebased.
- **patch-id:** `28ea0c83152f6b093b8bfb2d8935e28ab4ee307b` — content hash of the diff; survives rebase and cherry-pick. `git show d4ee86da | git patch-id --stable`.

`MECHANISM_TOOLS` is now `("artifact", "doc", "librarian")`, with the union requirement and its reasoning recorded at the site rather than in this file: the corpus is historical, a session keeps its old binary until `cargo rb` + `/mcp`, so dropping either name is blindness in one direction with no signal.

**Verified by observed before/after, not by assertion** — the pre-fix tuple was evaluated against the post-fix one in the same run:

| tool | pre-fix | fixed |
|---|---|---|
| `mcp__codescout__doc` | **False** | **True** |
| `mcp__codescout__artifact` | True | True |
| `mcp__codescout__librarian` | True | True |
| `mcp__codescout__grep` | False | False |
| `mcp__codescout__read_file` | False | False |

The last two matter as much as the first: they establish the documented `"doc"` greediness has not over-widened into ordinary tools. The two middle rows establish the historical half of the union still resolves.

The original plan below is preserved as written.

---

Plan (not yet implemented): add `"doc"` to `MECHANISM_TOOLS`. Keep `"artifact"` — the corpus is historical and both names must be unioned, exactly as the `probe_tool_surface.py` PROBES row prescribes for `usage.db`.

Note the substring test makes `"doc"` slightly greedy: it would also match a hypothetical `mcp__codescout__docs_*`. No such tool exists in the 21-tool registry today; if one is added, tighten `is_mechanism_tool` to match the full `mcp__codescout__<name>` form rather than widening the tuple further.

Better still, derive the list from the served registry instead of hand-maintaining it — that is what would actually reach this class of defect, and it is IC-18's open remedy.

## Tests added
None — filed, not fixed.

## Workarounds

Numbers produced today are a **floor** on engagement (equivalently, a ceiling on "never engaged"). Do not quote this probe's output for any window that includes post-2026-09-02 sessions without unioning both tool names by hand.

## Resume

Edit `scripts/probe_guide_section_use.py:103` to `MECHANISM_TOOLS = ("artifact", "doc", "librarian")`, re-run `python3 scripts/probe_guide_section_use.py`, and diff the never-engaged percentages against the pre-fix run to size the correction. If the delta is under ~1% today, say so in the PROBES row rather than leaving the reader to assume it is zero. Then add the rename caveat to this probe's `docs/PROBES.md` row.

## References

- `scripts/probe_guide_section_use.py:103` (`MECHANISM_TOOLS`), `:190-191` (`is_mechanism_tool`)
- `docs/PROBES.md` — `probe_tool_surface.py` row, which documents the identical rename trap for `usage.db`
- `6e8b4170`, `dac1068a` — the 2026-09-02 tool collapse that renamed `artifact*` to `doc`
- `docs/trackers/issue-clusters.md` — IC-18
- Sibling filed the same day: `docs/issues/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md`
