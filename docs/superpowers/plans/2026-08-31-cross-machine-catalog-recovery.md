# Cross-Machine Catalog Recovery — Implementation Plan (Unit 1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recover the catalog rows that `CM-1`, `CM-2` and `CM-3` record as unrecoverable, make them durable in git, and bring both hosts' catalogs into agreement without either side losing data.

**Architecture:** Data movement only — no Rust changes, no new tests. The 38 `PV-*` rows that exist solely in the desktop's `catalog.db` are projected into a committed archive companion (durability), then restored to params on both hosts (queryability). Four sidecar shape applies that are currently blocked by schema validation are unblocked by fixing the underlying params first. Every task's gate is a **measured before/after** — the honest analogue of a failing test for a task that ships no code.

**Tech Stack:** `sqlite3` CLI against `~/.local/share/librarian/catalog.db`; the `codescout artifact-augment` CLI (`--params @<file> --merge`); codescout MCP `artifact` / `librarian` tools; `ssh marius@192.168.1.162` for laptop-side steps.

**Spec:** `docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md`

## Global Constraints

- **Host labels are mandatory.** Every task is marked `[DESKTOP]`, `[LAPTOP]` or `[BOTH]`. The desktop is `/home/marius/work/claude/codescout` on this machine; the laptop is `marius@192.168.1.162`, same path. Running a step on the wrong host is the primary failure mode of this plan.
- **Never `git add -A`.** Stage explicit paths only, and re-read `git status --short` immediately before every commit. Peer sessions share this checkout; `bug-fix-session-log:F-60` records a peer's routine commit absorbing another session's uncommitted writes.
- **Never `git commit -m` with a body.** Write the message to a file and use `git commit -F <file>`. Backticks inside a double-quoted `-m` are live command substitution; measured 2026-08-07 (F-16), a word vanished from a message and `git commit` still exited 0.
- **Never `cp` the catalog.** Use `sqlite3 <db> ".backup '<path>'"`. The MCP server holds the database open in WAL mode and a plain copy can tear mid-write.
- **A params patch REPLACES the array it names.** `apply_merge_patch` replaces arrays wholesale, so every params write in this plan passes the **complete** array under its key. A partial array silently drops the rest.
- **Do not run `librarian(action="doctor", fix="prune_missing")`** at any point. The catalog is machine-global and holds rows for other repos.
- **Archive/move through the catalog**, never `git mv` — `id = sha256(abs_path)`, so a hand-move orphans the row's events and augmentation.
- **Existing backup:** `~/.local/share/librarian/catalog.db.bak-20260831-preintegration` (39,854,080 bytes, `quick_check: ok`, contains all 68 `PV` rows). This is the rollback for every desktop task.

## Artifact ids (stable, used throughout)

| artifact | id |
|---|---|
| `docs/trackers/provenance-subsystem.md` | `e12cd7e0060ed9b8` |
| `docs/trackers/tool-usage-patterns.md` | `f2ecdd76a6189efb` |
| `docs/research/README.md` | `5086e3c7c0b9d83c` |
| `docs/trackers/fable-tuning-findings.md` | `35de33286cd34f87` |
| `docs/trackers/resume-cross-machine-catalog-restore.md` | `f4923e5e894de62f` |

## File Structure

| file | responsibility | task |
|---|---|---|
| `docs/trackers/archive/provenance-subsystem-recovered-entries.md` | **Create.** The durable record of all 38 recovered `PV` rows. This is the artifact that makes `CM-2` non-recurring. | 1 |
| `docs/augmentations/docs-trackers-provenance-subsystem.yaml` | **Modify.** Widen the `status` enum to admit `superseded`. | 2 |
| `docs/trackers/provenance-subsystem.md:1044-1047` | **Modify.** Replace `PV-9`'s authored title; drop `PV-11`'s AUTHORED caveat. | 5 |
| `docs/trackers/resume-cross-machine-catalog-restore.md` | **Modify.** Correct `CM-1`, close `CM-2`, update `CM-3`. | 6 |
| Both hosts' `catalog.db` | **Modify.** Params only, via the CLI. Not a tracked file. | 2, 3, 4 |

## Pre-flight (run once, before Task 1) `[BOTH]`

- [ ] **Step 1: Confirm both hosts are at the same commit**

```bash
git rev-parse HEAD
ssh marius@192.168.1.162 'cd ~/work/claude/codescout && git rev-parse HEAD'
```

Expected: identical SHAs.

**If they differ, do not abort reflexively — this check's stated reason was measured false
once already, on 2026-08-31.** The reason it gives is that a divergent laptop invalidates
the row comparisons in Tasks 3 and 4. That is only true if the divergence *touched* this
plan's dependency files. Test it before deciding:

```bash
git fetch origin
git diff --name-only HEAD...origin/experiments | grep -E \
  'provenance-subsystem|augmentations/|tool-usage-patterns|fable-tuning-findings|research/README|resume-cross-machine'
```

Empty output means the baselines hold and the divergence is irrelevant to this plan;
re-measure the four laptop row counts (Task 2 Step 1, Task 3 Step 1, Task 4 Step 1) to
confirm, then proceed. Non-empty means STOP for real.

On 2026-08-31 the laptop was 12 and then 20 commits ahead, touching only
`observer-blindness.md` and `reconnaissance-patterns.md`; all four baselines were unchanged
to the byte, and execution correctly continued.

Baselines were taken at `2f434fba` plus three doc commits, which the same day's rebase
rewrote to `9c6fd5cf` / `4d2e5e58` / `9ac9e6d5` (patch-ids `3e97fd8b…`, `3687655c…`,
`513492b5…`). Cite the patch-ids if the SHAs no longer resolve.

- [ ] **Step 2: Confirm the laptop has no uncommitted catalog-affecting work**

```bash
ssh marius@192.168.1.162 'cd ~/work/claude/codescout && git status --short'
```

Expected: at most edits under `src/` and `docs/issues/`. Uncommitted edits to
`docs/trackers/**` or `docs/augmentations/**` mean the laptop is mid-write on
state this plan reads — STOP and reconcile with that session first.

- [ ] **Step 3: Take a laptop-side backup (the desktop already has one)**

```bash
ssh marius@192.168.1.162 'sqlite3 ~/.local/share/librarian/catalog.db \
  ".backup ~/.local/share/librarian/catalog.db.bak-20260831-preintegration" \
  && sqlite3 ~/.local/share/librarian/catalog.db.bak-20260831-preintegration "PRAGMA quick_check;"'
```

Expected: `ok`

- [ ] **Step 4: Record the baseline `doctor` counts**

```
librarian(action="doctor")
read_file("<returned buffer>", json_path="$.summary.by_check")
```

Expected, and write these down — later tasks assert movement from them:
`sidecar_shape_drift: 4`, `params_behind_body: 3`, `params_status_drift: 3`,
`entry_without_definition: 1`, `augmentation_declared_but_absent: 0`.

---

### Task 1: Archive companion for the 38 recovered PV rows `[DESKTOP]`

The durability fix, and the only task that is time-sensitive: until it lands, the
sole copy of these rows is a gitignored file on one machine.

**Files:**
- Create: `docs/trackers/archive/provenance-subsystem-recovered-entries.md`
- Read (not modified): desktop `~/.local/share/librarian/catalog.db`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: a committed file containing one `#### PV-N — <title>` section per
  recovered row. Task 6 cites its path when closing `CM-2`.

- [ ] **Step 1: Measure the baseline — how many of the 38 exist in git today**

```bash
git grep -ohE '^#{2,4} PV-[0-9]+ ' -- docs/trackers/ | wc -l
```

Expected: `30` — the live body defines 30 entries and nothing defines the other 38.
This is the number Step 6 must move.

- [ ] **Step 2: Generate the companion body from the catalog**

Writes the file mechanically from params; no hand-authoring, so no invented content.

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad; mkdir -p $SP
python3 - <<'PY' > $SP/companion-body.md
import sqlite3, json, os
db = sqlite3.connect(os.path.expanduser('~/.local/share/librarian/catalog.db'))
(params,) = db.execute(
    "SELECT g.params FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id "
    "WHERE a.abs_path LIKE '%/codescout/docs/trackers/provenance-subsystem.md'").fetchone()
items = json.loads(params)['items']
body = open('docs/trackers/provenance-subsystem.md').read()
defined = {l.split()[1] for l in body.splitlines()
           if l.startswith('#') and len(l.split()) > 2 and l.split()[1].startswith('PV-')
           and l.split()[2] in ('—', '–', '-')}
recovered = [i for i in items if i['id'] not in defined]
recovered.sort(key=lambda i: int(i['id'].split('-')[1]))
print(f"<!-- {len(recovered)} recovered entries -->")
for i in recovered:
    print(f"\n#### {i['id']} — {i['title']}\n")
    bits = [f"`{i['type']}`", f"**{i['status']}**"]
    if i.get('priority'): bits.append(f"priority `{i['priority']}`")
    print(" · ".join(bits) + "  ")
    print("*Provenance: RECOVERED-VERBATIM from the desktop `catalog.db`, 2026-08-31.*\n")
    if i.get('detail'):    print(i['detail'] + "\n")
    if i.get('evidence'):  print(f"**Evidence:** {i['evidence']}\n")
    if i.get('gated_on'):  print(f"**Gated on:** {i['gated_on']}\n")
PY
head -12 $SP/companion-body.md
grep -c '^#### PV-' $SP/companion-body.md
```

Expected: the count prints `38`.

- [ ] **Step 3: Assemble the file with its frontmatter and header**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
mkdir -p docs/trackers/archive
{
cat <<'EOF'
---
kind: tracker
status: archived
title: Provenance Subsystem — recovered entries (PV-N)
owners:
  - marius
tags:
  - provenance
  - archive
  - cross-machine
topic: provenance-attribution
---

# Provenance Subsystem — recovered entries (PV-N)

Companion to `docs/trackers/provenance-subsystem.md`. Holds the 38 `PV-N` entries
that existed only in one machine's `catalog.db` and that `CM-2` recorded as
**permanently lost**. They were not lost: `CM-2`'s own `Next:` line named the
recovery path — *"If the desktop's `catalog.db` still exists, a targeted export of
`artifact_augmentation.params` for this one id is the only real recovery path"* —
and the desktop's catalog did still exist. Recovered 2026-08-31.

Every section below is **RECOVERED-VERBATIM**: projected mechanically from
`artifact_augmentation.params`, never re-authored from the prose that cites these
ids. Source snapshot:
`~/.local/share/librarian/catalog.db.bak-20260831-preintegration`
(39,854,080 bytes, `quick_check: ok`).

**`status: archived` is deliberate.** Where two artifacts define one token the sole
*active* definer wins, so if a live stub for any of these is ever added to the
parent tracker it takes precedence automatically and no ambiguous token is created.

**No stubs were added to the live body, on purpose.** The parent tracker's
§ *Defining sections for cited entries* sets a measured policy against
mass-promotion. Checked 2026-08-31: of these 38, **22 appear cited from outside but
0 carry a load-bearing content citation** — the 22 are three bookkeeping files that
enumerate these ids *as lost*, a citation-count table, a doc-comment example of
citation syntax, and a test fixture using `PV-3`/`PV-9` as arbitrary tokens.

EOF
cat $SP/companion-body.md
} > docs/trackers/archive/provenance-subsystem-recovered-entries.md
wc -l docs/trackers/archive/provenance-subsystem-recovered-entries.md
```

- [ ] **Step 4: Verify all 38 ids landed, and that none duplicates a live definer**

```bash
grep -oE '^#### PV-[0-9]+' docs/trackers/archive/provenance-subsystem-recovered-entries.md \
  | sed 's/^#### //' | sort -u > /tmp/recovered.txt
wc -l < /tmp/recovered.txt
grep -oE '^#{2,4} PV-[0-9]+ ' docs/trackers/provenance-subsystem.md \
  | awk '{print $2}' | sort -u > /tmp/live.txt
comm -12 /tmp/recovered.txt /tmp/live.txt
```

Expected: count is `38`, and `comm -12` prints **nothing** — no id is defined in
both files, so no ambiguous token is created even before the archived-status rule
applies.

- [ ] **Step 5: Register in the catalog and confirm no link-graph regression**

```
librarian(action="reindex")
librarian(action="link_scan")
```

Expected: `reindex` reports `added: 1`. `link_scan` shows `edges_missing` and
`edges_stale` at `0` — if either is non-zero, run `link_scan(write=true)` then
re-run to confirm the fixpoint before committing.

- [ ] **Step 6: Confirm the measured baseline moved**

```bash
git add docs/trackers/archive/provenance-subsystem-recovered-entries.md
git diff --cached --numstat
git grep --cached -ohE '^#{2,4} PV-[0-9]+ ' -- docs/trackers/ | wc -l
```

Expected: the count is now `68` (was `30` at Step 1). All 68 `PV` entries are
defined somewhere in git for the first time.

- [ ] **Step 7: Commit**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
cat > $SP/msg.txt <<'EOF'
docs(trackers): recover the 38 PV rows CM-2 recorded as permanently lost

CM-2's own Next: line named the recovery path and conditioned it on the
desktop's catalog.db still existing. It does. All 38 ids it enumerates were
present there, matching its list exactly, 38 of 38.

Projected mechanically from artifact_augmentation.params into an archived
companion — never re-authored from the prose that cites them, which CM-2
explicitly warned is a citation and not a record.

Archived status is deliberate: the sole active definer wins a token, so a live
stub added later takes precedence with no ambiguity. No stubs were added to the
live body, because the parent tracker's own measured policy forbids
mass-promotion and 0 of the 38 carry a load-bearing content citation once
bookkeeping files, a citation-count table, a doc-comment syntax example and a
test fixture are excluded from the apparent 22.

All 68 PV entries are now defined in git; previously 30 were.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
git status --short
git commit -F $SP/msg.txt
```

---

### Task 2: Widen the `status` enum and restore the 38 rows to the laptop `[BOTH]`

**Files:**
- Modify: `docs/augmentations/docs-trackers-provenance-subsystem.yaml`
- Modify: both hosts' `catalog.db` (params + shape)

**Interfaces:**
- Consumes: Task 1's committed companion (referenced in the commit message only).
- Produces: `provenance-subsystem` params holding 68 rows on **both** hosts, and a
  sidecar whose `status` enum admits `superseded`. Task 6 asserts this when closing `CM-2`.

- [ ] **Step 1: Measure the baseline on both hosts**

```bash
Q="SELECT count(*) FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.items')) WHERE a.abs_path LIKE '%provenance-subsystem.md';"
sqlite3 ~/.local/share/librarian/catalog.db "$Q"
ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"$Q\""
```

Expected: desktop `68`, laptop `30`.

- [ ] **Step 2: Confirm the enum is the only schema blocker**

Already measured 2026-08-31, re-confirm before changing anything:

```bash
sqlite3 ~/.local/share/librarian/catalog.db "
SELECT json_extract(value,'\$.status') s, count(*)
FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.items'))
WHERE a.abs_path LIKE '%provenance-subsystem.md' GROUP BY s;"
```

Expected: `settled 43, open 17, carried 4, descoped 2, killed 1, superseded 1`.
Only `superseded` is outside the enum
`["settled","open","blocked","descoped","carried","killed"]`. All 68 rows carry the
four required fields, all `type` values are in enum, and all ids match
`^PV-[0-9]+$` — verified 2026-08-31, so nothing else can reject.

- [ ] **Step 3: Widen the enum in the catalog, which writes through to the sidecar**

`artifact_augment` with `merge=true` overlays the field and preserves the rest;
the write-through updates the committed YAML so the change travels.

```
artifact_augment(id="e12cd7e0060ed9b8", merge=true, params_schema={
  "type": "object",
  "required": ["items"],
  "properties": {"items": {"type": "array", "items": {
    "type": "object",
    "required": ["id", "type", "status", "title"],
    "properties": {
      "id":     {"type": "string", "pattern": "^PV-[0-9]+$"},
      "type":   {"type": "string", "enum": ["finding","gap","decision","hazard","task"]},
      "status": {"type": "string", "enum": ["settled","open","blocked","descoped","carried","killed","superseded"]},
      "title":  {"type": "string"},
      "detail": {"type": ["string","null"]},
      "note":   {"type": "string"}
    }}}}
})
```

- [ ] **Step 4: Verify the sidecar changed in git, and only in the enum**

```bash
git diff docs/augmentations/docs-trackers-provenance-subsystem.yaml
```

Expected: exactly one added enum member, `superseded`. Any other change means the
merge overlaid something it should not have — revert and re-check the call.

- [ ] **Step 5: Export the full 68-row array for the laptop**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
sqlite3 ~/.local/share/librarian/catalog.db \
  "SELECT json_object('items', json_extract(g.params,'\$.items'))
   FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id
   WHERE a.abs_path LIKE '%provenance-subsystem.md';" > $SP/pv-items.json
python3 -c "import json;d=json.load(open('$SP/pv-items.json'));print(len(d['items']),'items')"
scp $SP/pv-items.json marius@192.168.1.162:/tmp/pv-items.json
```

Expected: `68 items`. The payload is the **complete** array — a params patch
replaces the array it names, so a partial one would delete the remainder.

- [ ] **Step 6: Apply on the laptop**

The laptop must first pull, so its sidecar carries the widened enum — otherwise
its own schema rejects the `superseded` row exactly as the desktop's did.

```bash
ssh marius@192.168.1.162 'cd ~/work/claude/codescout && git pull --ff-only && \
  ./target/release/codescout artifact-augment e12cd7e0060ed9b8 --merge \
    --params @/tmp/pv-items.json 2>&1 | tail -1'
```

Expected: `"ok"`. An `Error: merged params violate params_schema` here means the
pull did not bring the widened enum — check `git log -1` on the laptop.

- [ ] **Step 7: Verify both hosts agree, id for id**

```bash
Q="SELECT json_extract(value,'\$.id') FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.items')) WHERE a.abs_path LIKE '%provenance-subsystem.md';"
sqlite3 ~/.local/share/librarian/catalog.db "$Q" | sort > /tmp/d.txt
ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"$Q\"" | sort > /tmp/l.txt
diff /tmp/d.txt /tmp/l.txt && echo "IDENTICAL"; wc -l < /tmp/d.txt
```

Expected: `IDENTICAL`, `68`.

- [ ] **Step 8: Verify the acceptance query works on both hosts**

```
artifact(action="get", id="e12cd7e0060ed9b8", entry_filter={"id": {"eq": "PV-48"}})
```

Expected: one entry returned, title beginning *"74 tool calls carry 51.3% of all
information-bearing context…"*. Run the equivalent on the laptop via
`ssh … './target/release/codescout artifact get e12cd7e0060ed9b8'` and confirm the
row is present.

- [ ] **Step 9: Confirm `sidecar_shape_drift` decremented**

```
librarian(action="doctor")
```

Expected: `sidecar_shape_drift: 3` (was `4`). `provenance-subsystem` no longer drifts.

- [ ] **Step 10: Commit**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
cat > $SP/msg.txt <<'EOF'
docs(trackers): widen the provenance status enum to the vocabulary its rows use

PV-48 carries status `superseded`, which the committed schema's enum did not
admit — so restoring the recovered rows failed validation on both hosts. Same
move and same reason as 2a8decc5, which widened the work queue's enum to the
vocabulary its bodies already used.

Measured before changing anything: of 68 rows, only this one value falls outside
the enum. All 68 carry the four required fields, every `type` is in enum, and
every id matches ^PV-[0-9]+$ — so the enum was the sole blocker.

With it widened, both hosts' catalogs now hold all 68 rows and agree id for id.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
git status --short
git add docs/augmentations/docs-trackers-provenance-subsystem.yaml
git commit -F $SP/msg.txt
```

---

### Task 3: Field-level union for `tool-usage-patterns` `[BOTH]`

The one place a naive row-level copy loses data in **either** direction. The
laptop's id set is a superset; its field content is not.

**Files:**
- Modify: both hosts' `catalog.db` (params for `f2ecdd76a6189efb`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: 32 observation rows on both hosts, retaining the desktop's 10 verdicts.

- [ ] **Step 1: Re-measure the divergence (do not trust this plan's numbers blind)**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
cat > $SP/obs.sql <<'SQL'
SELECT json_extract(value,'$.id')||'|'||coalesce(json_extract(value,'$.verdict'),'<none>')
FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id,
     json_each(json_extract(g.params,'$.observations'))
WHERE a.abs_path LIKE '%/codescout/docs/trackers/tool-usage-patterns.md';
SQL
sqlite3 ~/.local/share/librarian/catalog.db < $SP/obs.sql | sort > $SP/obs_d.txt
ssh marius@192.168.1.162 'sqlite3 ~/.local/share/librarian/catalog.db' < $SP/obs.sql | sort > $SP/obs_l.txt
echo "desktop $(wc -l < $SP/obs_d.txt)  laptop $(wc -l < $SP/obs_l.txt)"
join -t'|' $SP/obs_d.txt $SP/obs_l.txt -o 0,1.2,2.2 | awk -F'|' '$2!=$3'
```

Expected: `desktop 30  laptop 32`, and exactly 11 differing rows —
`T-005, T-008, T-011, T-012, T-19, T-20, T-22` (desktop `wrong-tool`, laptop
`<none>`), `T-17, T-18, T-21` (desktop `legitimate`, laptop `<none>`), and `T-30`
(desktop `legitimate, shape-mismatched`, laptop `legitimate`).

Note the id forms are **not** uniform — `T-005`/`T-008`/`T-011`/`T-012` are
zero-padded and the rest are not. Match them literally; a normalising comparison
will mis-join.

If the shape differs from this, STOP — the laptop has moved and the union below
is computed against a stale premise.

- [ ] **Step 2: Build the union**

Laptop rows are the base (they carry `T-31`/`T-32` and the schema-valid `T-30`);
the desktop's non-null `verdict` fills any laptop row that lacks one.

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"SELECT json_extract(g.params,'\\\$.observations') FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id WHERE a.abs_path LIKE '%tool-usage-patterns.md';\"" > $SP/obs_laptop.json
sqlite3 ~/.local/share/librarian/catalog.db "SELECT json_extract(g.params,'\$.observations') FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id WHERE a.abs_path LIKE '%tool-usage-patterns.md';" > $SP/obs_desktop.json
python3 - <<'PY'
import json, os
SP = "/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad"
lap = json.load(open(f"{SP}/obs_laptop.json"))
desk = {r["id"]: r for r in json.load(open(f"{SP}/obs_desktop.json"))}
filled = []
for row in lap:
    d = desk.get(row["id"])
    if d and not row.get("verdict") and d.get("verdict"):
        row["verdict"] = d["verdict"]; filled.append(row["id"])
    for k, v in (d or {}).items():                 # desktop-only fields, never overwrite
        if k not in row and v is not None:
            row[k] = v
json.dump({"observations": lap}, open(f"{SP}/obs-union.json", "w"), indent=1)
print("rows:", len(lap), "| verdicts filled from desktop:", len(filled), filled)
PY
```

Expected: `rows: 32 | verdicts filled from desktop: 10` listing the ten ids from
Step 1. `T-30` is **not** in the filled list — the laptop's value is non-null and
therefore wins, which is the intended normalisation.

- [ ] **Step 3: Apply on the desktop**

```
artifact_augment(id="f2ecdd76a6189efb", merge=true, params_path="/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad/obs-union.json")
```

Expected: `"ok"`. `params_path` is read server-side, which is required here — the
payload exceeds the inline budget.

- [ ] **Step 4: Apply on the laptop**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
scp $SP/obs-union.json marius@192.168.1.162:/tmp/obs-union.json
ssh marius@192.168.1.162 'cd ~/work/claude/codescout && \
  ./target/release/codescout artifact-augment f2ecdd76a6189efb --merge \
    --params @/tmp/obs-union.json 2>&1 | tail -1'
```

Expected: `"ok"`.

- [ ] **Step 5: Verify both hosts are now byte-identical on this collection**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
sqlite3 ~/.local/share/librarian/catalog.db < $SP/obs.sql | sort > $SP/after_d.txt
ssh marius@192.168.1.162 'sqlite3 ~/.local/share/librarian/catalog.db' < $SP/obs.sql | sort > $SP/after_l.txt
diff $SP/after_d.txt $SP/after_l.txt && echo IDENTICAL
grep -c 'wrong-tool\|legitimate' $SP/after_d.txt
```

Expected: `IDENTICAL`, and the verdict count is **at least 11** — the 10 recovered
plus `T-30`. A count of 1 means the union was not applied and the laptop's
verdict-less rows overwrote the desktop's.

- [ ] **Step 6: Apply the blocked sidecar shape and confirm the decrement**

Extract the three fields to files and pass them by reference — the CLI accepts
`@<file>` for each, and `--merge` preserves `entry_collection`, which the CLI has
no flag for and would otherwise be lost.

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
python3 - <<'PY'
import yaml, json
SP = "/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad"
s = yaml.safe_load(open("docs/augmentations/docs-trackers-tool-usage-patterns.yaml"))
open(f"{SP}/tup.prompt", "w").write(s["prompt"])
open(f"{SP}/tup.render", "w").write(s["render_template"])
open(f"{SP}/tup.schema", "w").write(json.dumps(s["params_schema"]))
PY
./target/release/codescout artifact-augment f2ecdd76a6189efb --merge \
  --prompt @$SP/tup.prompt --render-template @$SP/tup.render \
  --params-schema @$SP/tup.schema 2>&1 | tail -1
```

Expected: `"ok"`. Then:

```
librarian(action="doctor")
```

Expected: `sidecar_shape_drift: 2` (was `3`).

- [ ] **Step 7: Commit**

No tracked file changes if the sidecar write-through is byte-identical — confirm
with `git status --short`. If it is clean, record the work as an artifact event
instead of an empty commit:

```
artifact_event(action="create", artifact_id="f2ecdd76a6189efb", kind="note",
  payload={"text": "Field-level union across hosts 2026-08-31: laptop's 32 rows as base, 10 verdicts restored from the desktop (T-005/008/011/012/019/020/022 wrong-tool; T-17/18/21 legitimate). T-30 normalised to the laptop's schema-valid `legitimate`. Both catalogs now identical."})
```

---

### Task 4: Unblock and apply the last two sidecar shapes `[BOTH]`

**Files:**
- Modify: both hosts' `catalog.db` (params for `5086e3c7c0b9d83c`, `35de33286cd34f87`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `sidecar_shape_drift: 0`, the gate for declaring Section 1 shape-complete.

- [ ] **Step 1: Confirm the entry counts still match, so the copy is safe**

```bash
for pair in "research/README.md:entries" "trackers/fable-tuning-findings.md:findings"; do
  f=${pair%%:*}; c=${pair##*:}
  Q="SELECT count(*) FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.$c')) WHERE a.abs_path LIKE '%/codescout/docs/$f';"
  echo "$f  desktop=$(sqlite3 ~/.local/share/librarian/catalog.db "$Q")  laptop=$(ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"$Q\"")"
done
```

Expected: `research/README.md desktop=15 laptop=15` and
`fable-tuning-findings.md desktop=18 laptop=18`. Equal counts are what make a
wholesale copy safe here — unlike Task 3. **If either pair is unequal, STOP** and
run Task 3's union procedure against that collection instead.

- [ ] **Step 2: Confirm the laptop's rows are the strictly-better ones**

```bash
Q="SELECT count(*) FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.entries')) WHERE a.abs_path LIKE '%/codescout/docs/research/README.md' AND json_extract(value,'\$.path') IS NOT NULL;"
echo "desktop rows WITH path: $(sqlite3 ~/.local/share/librarian/catalog.db "$Q")"
echo "laptop  rows WITH path: $(ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"$Q\"")"
```

Expected: desktop `0`, laptop `15`. The laptop populated the `path` field the
schema now requires; the desktop's rows predate it. Same relationship holds for
`title` on `fable-tuning-findings`.

- [ ] **Step 3: Copy each collection from laptop to desktop**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"SELECT json_object('entries', json_extract(g.params,'\\\$.entries')) FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id WHERE a.abs_path LIKE '%/codescout/docs/research/README.md';\"" > $SP/readme-entries.json
ssh marius@192.168.1.162 "sqlite3 ~/.local/share/librarian/catalog.db \"SELECT json_object('findings', json_extract(g.params,'\\\$.findings')) FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id WHERE a.abs_path LIKE '%/codescout/docs/trackers/fable-tuning-findings.md';\"" > $SP/fable-findings.json
python3 -c "import json;print('entries',len(json.load(open('$SP/readme-entries.json'))['entries']))"
python3 -c "import json;print('findings',len(json.load(open('$SP/fable-findings.json'))['findings']))"
```

Expected: `entries 15`, `findings 18`.

- [ ] **Step 4: Apply both on the desktop**

```
artifact_augment(id="5086e3c7c0b9d83c", merge=true, params_path="/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad/readme-entries.json")
artifact_augment(id="35de33286cd34f87", merge=true, params_path="/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad/fable-findings.json")
```

Expected: `"ok"` from each.

- [ ] **Step 5: Apply the two blocked sidecar shapes**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
python3 - <<'PY'
import yaml, json
SP = "/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad"
for stem, tag in (("docs-research-README", "readme"),
                  ("docs-trackers-fable-tuning-findings", "fable")):
    s = yaml.safe_load(open(f"docs/augmentations/{stem}.yaml"))
    open(f"{SP}/{tag}.prompt", "w").write(s["prompt"])
    open(f"{SP}/{tag}.render", "w").write(s["render_template"])
    open(f"{SP}/{tag}.schema", "w").write(json.dumps(s["params_schema"]))
PY
./target/release/codescout artifact-augment 5086e3c7c0b9d83c --merge \
  --prompt @$SP/readme.prompt --render-template @$SP/readme.render \
  --params-schema @$SP/readme.schema 2>&1 | tail -1
./target/release/codescout artifact-augment 35de33286cd34f87 --merge \
  --prompt @$SP/fable.prompt --render-template @$SP/fable.render \
  --params-schema @$SP/fable.schema 2>&1 | tail -1
```

Expected: `"ok"` from each. Both failed before Step 4 with
`"path" is a required property` / `"title" is a required property`; they succeed
now because the copied params satisfy the schema.

- [ ] **Step 6: Confirm the gate**

```
librarian(action="doctor")
```

Expected: **`sidecar_shape_drift: 0`** (was `4` at pre-flight). Both hosts now hold
the same augmentation shape for all nine sidecar-backed artifacts.

- [ ] **Step 7: Verify no tracked file drifted, then record**

```bash
git status --short
```

Expected: clean — the write-throughs reproduce the committed sidecars byte for
byte. If any `docs/augmentations/*.yaml` shows a diff, the applied values did not
match the file: inspect before committing anything.

---

### Task 5: Correct the two authored titles `[DESKTOP]`

**Files:**
- Modify: `docs/trackers/provenance-subsystem.md:1044-1047`

**Interfaces:**
- Consumes: nothing.
- Produces: corrected body text that Task 6 cites when updating `CM-3`.

- [ ] **Step 1: Record the baseline**

```bash
sed -n '1044,1047p' docs/trackers/provenance-subsystem.md
grep -c 'title AUTHORED from the round-2 table' docs/trackers/provenance-subsystem.md
```

Expected: the two headings as written below, and a count of `2`.

- [ ] **Step 2: Confirm the canonical titles are still what the catalog holds**

```bash
sqlite3 ~/.local/share/librarian/catalog.db "
SELECT json_extract(value,'\$.id')||' :: '||json_extract(value,'\$.title')
FROM artifact_augmentation g JOIN artifact a ON a.id=g.artifact_id, json_each(json_extract(g.params,'\$.items'))
WHERE a.abs_path LIKE '%provenance-subsystem.md' AND json_extract(value,'\$.id') IN ('PV-9','PV-11');"
```

Expected:
```
PV-9 :: DONE — M6 measured: spec churn is same-session, not long-horizon drift
PV-11 :: RESOLVED — `unrecorded` dominates only at whole-repo scope, NOT at working-diff scope
```

- [ ] **Step 3: Replace `PV-9`'s title and provenance line**

This artifact is augmented, so the librarian guard refuses `edit_markdown` —
the edit must go through `artifact(action="update", patch={body_edits: …})`.

```
artifact(action="update", id="e12cd7e0060ed9b8", patch={"body_edits": [
 {"heading": "### Defining sections for cited entries", "action": "edit",
  "old_string": "#### PV-9 — M6 stale-drift: specs rarely change after code derives from them, at any horizon that matters\n`gap` · **settled** · repo-gated weak NO · heading added 2026-08-28; title AUTHORED from the round-2 table, not recovered — see the row's `note`",
  "new_string": "#### PV-9 — DONE — M6 measured: spec churn is same-session, not long-horizon drift\n`gap` · **settled** · repo-gated weak NO · heading added 2026-08-28; title RECOVERED-VERBATIM 2026-08-31 from the desktop catalog, replacing an AUTHORED title that asserted a different claim (\"at any horizon that matters\" vs same-session churn)"},
 {"heading": "### Defining sections for cited entries", "action": "edit",
  "old_string": "`gap` · **settled** · resolves to PV-17 · heading added 2026-08-28; title AUTHORED from the round-2 table, not recovered — see the row's `note`",
  "new_string": "`gap` · **settled** · resolves to PV-17 · heading added 2026-08-28; title confirmed RECOVERED-VERBATIM 2026-08-31 — the 2026-08-28 authored text matched the canonical in substance"}
]})
```

- [ ] **Step 4: Verify the correction landed and the caveats are gone**

```bash
sed -n '1044,1047p' docs/trackers/provenance-subsystem.md
grep -c 'title AUTHORED from the round-2 table' docs/trackers/provenance-subsystem.md
```

Expected: `PV-9`'s heading now reads *"DONE — M6 measured: spec churn is
same-session, not long-horizon drift"*, and the count is `0` (was `2`).

- [ ] **Step 5: Commit**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
cat > $SP/msg.txt <<'EOF'
docs(trackers): replace PV-9's authored title with the recovered canonical one

CM-3 authored titles for PV-9 and PV-11 on 2026-08-28 because "no canonical
title survived the catalog loss". Both canonical titles were in the desktop's
catalog the whole time.

PV-9's matters: the authored text claims specs rarely change "at any horizon
that matters", while the canonical says the churn is same-session. Those are
different claims, and the invented one was the committed record.

PV-11's authored text matched its canonical in substance, so only the AUTHORED
caveat is dropped there.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
git status --short
git add docs/trackers/provenance-subsystem.md
git commit -F $SP/msg.txt
```

---

### Task 6: Correct the CM ledger `[DESKTOP]`

**Files:**
- Modify: `docs/trackers/resume-cross-machine-catalog-restore.md`

**Interfaces:**
- Consumes: Tasks 1–5 (cites the companion path, the 68-row parity, the title fix).
- Produces: a corrected ledger. Terminal task.

- [ ] **Step 1: Record the baseline**

```bash
grep -n 'permanently\|no provenance anywhere\|only invention' docs/trackers/resume-cross-machine-catalog-restore.md
```

Expected: hits in `CM-1` and the `CM-2` heading — the three claims this task retires.

- [ ] **Step 2: Close `CM-2`**

`CM-2`'s `Status:` is `open` and `Valid: invariant`. The invariant is false, so
both change. Note the heading itself says "permanently" and must change too.

```
artifact(action="update", id="f4923e5e894de62f", patch={"body_edits": [
 {"heading": "## CM-2 — provenance-subsystem is missing 38 rows, permanently",
  "action": "edit",
  "old_string": "**Status:** open\n**Valid:** invariant",
  "new_string": "**Status:** fixed 2026-08-31 — recovered in full from the desktop catalog\n**Valid:** dated 2026-08-31"},
 {"heading": "## CM-2 — provenance-subsystem is missing 38 rows, permanently",
  "action": "edit",
  "old_string": "**Next:** do not fabricate them from the prose that cites them",
  "new_string": "**Done 2026-08-31.** All 38 recovered, ids matching this entry's enumeration exactly, 38 of 38. Projected verbatim from `artifact_augmentation.params` into `docs/trackers/archive/provenance-subsystem-recovered-entries.md`; both hosts' catalogs now hold all 68 rows and agree id for id. The `Next:` line below named this exact path and conditioned it on the desktop's `catalog.db` still existing — it did. Retained for the record:\n\n**Superseded Next:** do not fabricate them from the prose that cites them"}
]})
```

- [ ] **Step 3: Correct `CM-1`'s factual premise without overturning its judgement**

```
artifact(action="update", id="f4923e5e894de62f", patch={"body_edits": [
 {"heading": "## CM-1 — 13 trackers still unaugmented, by decision, not by omission",
  "action": "edit",
  "old_string": "Five of the 13 have **no provenance anywhere**",
  "new_string": "**Correction 2026-08-31 — the premise below is false.** All five have live augmentations in the desktop's `catalog.db`: `structural-debt-refactor` (11 `items`, 25,455 bytes of params), `test-escape-hardening` (7 `interventions`), `retrieval-benchmark`, `code-dupes-backlog`, `2026-08-15-tool-usage-investigation`. Restoration is therefore possible and would **not** be invention. This does not overturn the entry's decision — the argument that restoring an augmentation purely to clear a check converts a precise signal into a false all-clear still stands, and the `[LIVE]` block is still read by every agent meeting the tracker cold. Only the impossibility claim is withdrawn. Original text follows.\n\nFive of the 13 have **no provenance anywhere**"}
]})
```

- [ ] **Step 4: Update `CM-3`**

```
artifact(action="update", id="f4923e5e894de62f", patch={"body_edits": [
 {"heading": "## CM-3 — four PV rows have no defining heading", "action": "edit",
  "old_string": "**Valid:** dated 2026-08-28",
  "new_string": "**Valid:** dated 2026-08-31\n\n**Update 2026-08-31.** The AUTHORED titles this entry describes were replaceable after all — PV-9's and PV-11's canonical titles were in the desktop catalog. PV-9's authored text asserted a materially different claim and has been replaced; PV-11's matched in substance and only its caveat was dropped. The VERBATIM/AUTHORED/DERIVED distinction this entry introduced is what made the divergence findable, and it stays."}
]})
```

- [ ] **Step 5: Verify all three corrections landed**

```bash
grep -c 'Status:\*\* fixed 2026-08-31' docs/trackers/resume-cross-machine-catalog-restore.md
grep -c 'Correction 2026-08-31 — the premise below is false' docs/trackers/resume-cross-machine-catalog-restore.md
grep -c 'Update 2026-08-31' docs/trackers/resume-cross-machine-catalog-restore.md
grep -c 'Superseded Next:' docs/trackers/resume-cross-machine-catalog-restore.md
```

Expected: `1` from each of the four.

The original phrases ("permanently", "no provenance anywhere", "only invention")
are **deliberately still present** — each entry was correct on the evidence it
had, so the corrections annotate rather than rewrite. Do not grep for their
absence.

- [ ] **Step 6: Final gate — the whole of Section 1**

```
librarian(action="reindex")
librarian(action="link_scan")
librarian(action="doctor")
```

Expected against the pre-flight baseline:

| check | before | after |
|---|---:|---:|
| `sidecar_shape_drift` | 4 | **0** |
| `params_behind_body` | 3 | **≤ 1** (`open-issue-work-queue` may persist — out of scope) |
| `augmentation_declared_but_absent` | 0 | 0 |
| `PV` entries defined in git | 30 | **68** |
| `link_scan` | fixpoint | fixpoint (`edges_missing[0]`, `edges_stale[0]`) |

- [ ] **Step 7: Commit**

```bash
SP=/tmp/claude-1000/-home-marius-work-claude-codescout/scratchpad
cat > $SP/msg.txt <<'EOF'
docs(trackers): close CM-2 as recovered, correct CM-1's impossibility claim

Three CM entries rested on one premise — that the desktop's catalog was gone —
and all three were reachable from it.

CM-2 closes: 38 of 38 recovered, matching its own enumeration exactly. Its
Next: line named this path and conditioned it on the desktop catalog existing.

CM-1 keeps its decision and loses its impossibility claim. All five trackers it
called "no provenance anywhere, restoration is not possible, only invention"
hold live augmentations on the desktop. The argument against restoring purely to
clear a check is untouched; only the claim that it could not be done is
withdrawn.

CM-3 gains the outcome of its own provenance labelling: marking PV-9's title
AUTHORED is what made it findable as wrong.

Original text is retained in every case rather than rewritten — these entries
were correct on the evidence available to them.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
git status --short
git add docs/trackers/resume-cross-machine-catalog-restore.md
git commit -F $SP/msg.txt
```

---

## Rollback

Every desktop task is reversible from
`~/.local/share/librarian/catalog.db.bak-20260831-preintegration`, the laptop from
its pre-flight twin. To restore a host:

```bash
# stop the MCP server first (a live server holds the DB open in WAL mode)
cp ~/.local/share/librarian/catalog.db.bak-20260831-preintegration \
   ~/.local/share/librarian/catalog.db
```

`cp` is correct **here** and nowhere else in this plan: the backup is a static
file with no writer attached. Committed files roll back with `git revert`.

## Out of scope (recorded, not done)

- Restoring the 14 desktop-only augmentations. `CM-1`'s decision stands; only its
  factual premise is corrected. Revisit when someone writes an `entry_filter`
  query against one of them.
- `open-issue-work-queue`'s 24 `params_behind_body` ids (`BL-45`…). The laptop
  holds all 68 and the desktop 44; this is the same class as Task 4 but was not
  measured id-by-id, so it needs its own comparison before any copy.
- Everything in Section 2 of the spec — units 2 and 3 get their own plans.
