---
kind: tracker
status: active
title: Codescout Usage Frictions — U-N Log
tags:
- pika
- iron-law
- usage
entry_high_water_U: 52
entry_prefix: U
expects_augmentation: docs/augmentations/docs-trackers-codescout-usage-frictions.yaml
---

# Codescout Usage Frictions — U-N Log

Observed tool-misuse violations. Each U-N is allocated by the Pika at scan
time. Format from `~/.claude/buddy/skills/codescout-pika/SKILL.md` § Tracker
Format. Backing rows live in `.codescout/usage.db::pika_observations`.

**Archive policy:** entries with terminal status (`fixed-shipped` to master,
`wontfix`, `by-design`, `substrate-caught`) graduate to
`docs/trackers/archive/codescout-usage-frictions-<YYYY>-q<n>.md` on the
quarterly archive pass. The active list keeps only currently-open items
plus closures still awaiting master cherry-pick. See
[`docs/trackers/archive-cadence-policy.md`](archive-cadence-policy.md).

**Archived to 2026 Q2** (pilot pass, 2026-05-24): U-4, U-9, U-16. See
[`archive/codescout-usage-frictions-2026-q2.md`](archive/codescout-usage-frictions-2026-q2.md).

**Archived to 2026 Q2** (second pass, 2026-11-02): U-1, U-2, U-3, U-5, U-6, U-7, U-8, U-10, U-11, U-14, U-15, U-17, U-19, U-20, U-21, U-22, U-23, U-24, U-25, U-26, U-27, U-30, U-31, U-33, U-35, U-36, U-39, U-42, U-43, U-44, U-45. Closed or verified fixed on `experiments`; the pass did not wait for master.

---
### U-12 — Recon SKILL body inline-pasted instead of lazy-loaded

**When:** 2026-05-23, user invoked `/codescout-companion:reconnaissance` early in session (turn 1 of this conversation, before the Pika summon).

**Iron Law / pattern:** static-prefix budget — every line in slash-command output joins the cached session prefix.

**Tool called (surface):** the slash command pastes ~300 lines of `reconnaissance/SKILL.md` inline into the user-message turn.

**Should have called:** debatable — slash commands trade lazy-load (Skill tool) for "always visible". For a *frequently invoked* skill like reconnaissance during a multi-task session, inline-paste is the right call (the body is referenced repeatedly). For a one-shot acknowledgment without follow-up scout work, lazy-load wins.

**Whistle delivered:** yes (chat U-9 → this tracker entry).

**Recurrence:** 1st.

**Severity:** low — design call, not a defect. Listed for awareness; not for immediate fix.

**Status:** open. Defer until usage data accumulates: query `.codescout/usage.db` for how often the recon body content gets actively referenced vs sits idle in the prefix. If reference rate is low, lazy-load wins.


### U-13 — Per-turn re-injection of output-style anchor

**When:** 2026-05-23, every assistant turn in this session.

**Iron Law / pattern:** per-turn hook design.

**Tool called (surface):** the CC harness re-injects `"Explanatory output style is active. Remember to follow the specific guidelines for this style."` as a `<system-reminder>` on every turn.

**Should have called:** by design — re-anchoring prevents style drift mid-session, especially under voice stacking (currently three layers in this session: Pika voice + Caveman + Explanatory output style). Listed only as a surface to be aware of when designing future hooks; the design tradeoff is "always anchored" vs "per-turn token cost".

**Whistle delivered:** yes (chat U-10 → this tracker entry).

**Recurrence:** every turn (by design).

**Severity:** info, not friction.

**Status:** open as design note. No fix expected.



---
### U-18 — Iron Law slips persist under deny-mode, session 2026-05-23 (×4)

**When:** audit_doc_refs noise investigation session, 2026-05-23 (this conversation).
Bound: continued from compacted cs-hint tracker session earlier the same day.

**Iron Law / pattern:** Mixed — Iron Law 1 (grep on source files) and Iron Law 3
(piped unbounded `run_command` output). Both caught by codescout-companion
PreToolUse hook in **deny mode**. Earlier U-3 (2026-05-18) was the warn-mode
baseline that promoted H-1 to deny; this is the first multi-strike post-deny
data point.

**Confirming data:** four strikes in a single session, all hook-blocked
and rerouted within the same turn:

1. `grep -rEn 'with_hint\b' src | wc -l` — recursive grep + pipe, blocked
   by source-file gate. Reroute → codescout `grep(pattern, path='src')`.
2. `grep -rEn 'to_string\(\)\.contains' src --include='*.rs' | grep ...`
   — recursive grep with file-type filter, blocked by source-file gate.
   Reroute → codescout `grep(pattern, path='src')`.
3. `cargo test --lib librarian::tools::audit_doc_refs:: 2>&1 | tail -30`
   — pipe to log-trimmer, blocked by IL3 gate. Reroute → run bare,
   `tail @cmd_xxx` on buffer.
4. `git log --oneline -- docs/trackers/doc-ref-audit.md 2>&1 | head -5; ...`
   — pipe to log-trimmer in chained command, blocked by IL3 gate.
   Reroute → run bare, `head @cmd_xxx` on buffer.

**Severity:** low — hook denied all 4 before any context cost. Each
recovery added one round-trip (~5-15s wall-clock). No cumulative drift
this session, in contrast to U-3's warn-mode 9-strike cost.

**Status:** open — the deny-mode substrate works as designed (zero
context bloat), but the reflex itself did not extinguish across 5+ days
since U-3. Habit persists; only the consequence changed.

**Diagnosis (introspection):**

- **Slips 1 and 2** (recursive grep): muscle memory from shell-first
  workflows. `grep -r` is a single token in mental shorthand for
  "search the tree"; codescout's `grep(pattern)` requires unpacking
  that into a tool-name. Under load (long investigation, many files to
  search), the unpack step gets skipped. Same root cause as U-3's
  "single round-trip" instinct, but the failure mode is *tool selection*,
  not output bounding.
- **Slips 3 and 4** (pipe to head/tail): exactly U-3's pattern, still
  active. Knowing the buffer exists doesn't override the reflex of
  bounding output at emission time. Tail-on-buffer requires two thoughts
  ("run bare" → "tail the buffer") where pipe-tail requires one ("just
  trim it inline").

**Pointer:** Deny-mode is the right substrate — it prevents context
bloat with zero ambiguity. The reflex persisting is bounded-cost (one
extra round-trip per slip) and arguably acceptable given habit-extinction
across sessions is slow. Open question for the H-N tracker: is there a
proactive nudge (per-turn first-call reminder, or skill-style "before
your first run_command this turn, consider …") that could shift the
reflex faster? Not blocking; capture as candidate, not priority.


---


### U-28 — `read_markdown` errors are untagged (`err_family` NULL), hiding ~23 live errors/week

**When:** 2026-06-21 Pika re-scan of `.codescout/usage.db` (`id > 6213`, 1,833 new calls, 34 sessions lifetime).

**Iron Law / pattern:** Observability gap, not an Iron Law. `recoverable_error` rows from `read_markdown` carry `err_family = NULL`, so they never appear in the recency rollup that gates every "live friction" verdict. 23 errors/week were invisible until drilled by hand.

**Tool called:** `read_markdown` — **51 lifetime / 23 in last 7d**, the #1 source of `(null)`-family errors (next: `artifact` 29, `symbols` 9, `edit_code` 8, `references`/`read_file`/`edit_markdown` 7 each).

**Sub-signatures (7d):**
- `file not found: 'CLAUDE.md'` ×10 — relative-path / moved-file reads (some against the just-restructured CLAUDE.md).
- `read_markdown only supports .md files` ×3 — wrong tool; should be `read_file`.
- `combined headings span N lines — exceeds inline threshold` ×4 — too many headings per call.
- `heading '…' not found` ×several — stale heading references.
- `missing 'path' parameter` ×1.

**Should have called / fix:** Two-sided.
- *Codescout-side (primary):* tag `read_markdown` errors with `err_family` (`md_file_not_found` / `non_md_file` / `heading_not_found` / `heading_span_over_threshold` / `missing_path`) so they surface in the rollup instead of hiding in `(null)`.
- *Agent-side:* verify path + heading exist before `read_markdown`; use `read_file` for non-`.md`; read fewer headings per call.

**Whistle delivered:** yes (this entry + `pika_observations` row, `subkind=read_markdown_untagged_errors`).

**Recurrence:** 51 lifetime / 23 in 7d — habit.

**Severity:** med — observability blind spot plus recurring retry-cost; no data loss.

**Status:** fixed-verified 2026-11-02 — `read_markdown` was folded into `read_file`, and `normalize_err_family` (`src/usage/db.rs`) tags its errors (`c76937ee3`). In `usage.db` since 2026-10-25, 279 of 286 non-success `read_file` rows carry a family (`path_not_found` 50, `heading_not_found` 5); the other 7 are untagged and were not examined. Earlier status: open. No hookify rule proposed — the fix is observability (`err_family` tagging), not a deny/warn gate.

### U-29 — Guards that reject AFTER accepting the payload cost a full re-transmission (×3 in one session)

**When:** 2026-08-04, provenance-measurement session (13 rounds, heavy file
authoring). Three instances of the same shape, two tools.

**Iron Law / pattern:** Not an Iron Law violation — every guard fired correctly.
The friction is that the guard's cost scales with the size of the content it
rejects, because validation happens after the argument payload is already in the
request.

**Tool called / instances:**
1. `create_file` → `file already exists` on a ~6 KB whole-file rewrite of a Python
   module. Correct refusal, good hint (`pass overwrite: true`), but the composed
   payload is discarded and complying costs a second full transmission. ~6k
   tokens.
2. `create_file` → `outside the project root` for a commit-message file in the
   job tmp dir. Returns `@ack_*`; re-invoking with the handle requires re-sending
   the entire content.
3. `create_file` → same, for a script written to the relocated artifact
   directory. Again a full re-send.

**Should have called / fix:** The agent-side fix is anticipation (pass
`overwrite: true` when the path may exist; write scratch inside the project or
ack first), and it is genuinely partial — existence and scope are not always
known ahead of the call.

The codescout-side fix is the one that generalises: on a rejection that the
caller can resolve by re-invoking, **buffer the submitted content server-side and
return a handle**, so the retry references it instead of re-sending. The `@ack_*`
pattern already does exactly this for the *approval token* — it just does not
carry the payload. Extending it would make the guard's cost constant rather than
proportional to content size, and the machinery is the same output-buffer system
that already backs `@cmd_*` / `@tool_*` / `@file_*`.

**Whistle delivered:** yes — this entry. Also logged as F-6 in
`docs/trackers/provenance-probe-session-log.md` (instance 1, before the pattern
was visible as a class).

**Recurrence:** 3 in a single session, all in file-authoring work. Expect it to
track how much whole-file writing a session does rather than being uniform.

**Severity:** med — pure token cost, no correctness impact, but it recurs for
every large rewrite and the cost is highest exactly when the content is most
expensive to have composed.

**Status:** open, partly fixed (checked 2026-11-02 by reading the code, not run). The outside-root write case is fixed: `resolve_write_or_capture` (`src/tools/core/write_ack.rs`) stores the full input as `pending_ack`, and the retry `create_file(path="@ack_*")` re-sends no content (`0452cf600`). The `file already exists` refusal in `src/tools/create_file.rs` still returns before anything is buffered, so that case still costs a full re-send. No hookify rule proposed — a hook cannot see the payload cost;
the fix belongs in the tool's rejection path.

---

### U-32 — `.buddy/` is read-exempt but not write-exempt, and the buddy summon flow tells you to use native tools

**When:** 2026-08-06, updating a buddy project memory after `/buddy:summon`.

**Expected:** `.buddy/` is guard-exempt — the summon command says so explicitly:
*"Read that one file first with native `Read`… the `.buddy/` path is guard-exempt,
and `read_markdown` would fragment a persona-sized file into a heading map."*
Native `Read` on the spilled payload worked exactly as documented.

**Got:** native `Write` to `.buddy/memory/docs-lotus-frog/<slug>.md` was **denied** —
*"codescout's create_file is the tracked path for new source files… The native Write
tool bypasses codescout's safety gates and file tracking."* Succeeded via
`create_file(overwrite=true)`.

The exemption is read-only, which is defensible — writes should be tracked. But the
asymmetry is undocumented on both sides: the buddy summon flow advertises native-tool
access for `.buddy/` without qualifying it as reads-only, and the memory protocol's
write steps (`git add .buddy/memory/<rel-path>`) don't name the write tool at all.

**Cost:** one round-trip. Trivial in isolation; recorded because it is a
prompt-surface disagreement between two active plugins, which is the class that
quietly wastes calls in every session that touches memory.

**Fix idea:** one clause in the summon command — *"native `Read` for the payload;
writes to `.buddy/` still go through `create_file` / `edit_file`"* — and the same
note in `data/memory-protocol.md` § Staging.

**Status:** open.

---

### U-34 — `edit_code action=insert` takes the ANCHOR in `symbol`, while an `anchor` param also exists

**When:** 2026-08-06. Inserting a new function after an existing one.

**Tried:** `edit_code(action="insert", path=…, anchor="is_module_path", position="after", body=…)`
→ `missing 'symbol' parameter`. Then
`edit_code(action="insert", symbol="is_path_segment", anchor="is_module_path", position="after", …)`
— reading `symbol` as "the symbol I am creating" — → `symbol not found: is_path_segment`.
Third attempt, `symbol="is_module_path", position="after"` with no `anchor`, succeeded.

**Got:** two failed calls to learn that for `insert`, `symbol` names the **existing
anchor**, and `anchor` is either an alias or inert. The error messages are individually
correct and jointly misleading: the first says a required param is missing without
saying what it should contain, and the second confirms `symbol` is looked up as an
existing symbol — which only makes sense once you already know the answer.

**Cost:** 2 round-trips, one of them re-transmitting a ~16-line body.

**Fix idea:** either drop `anchor` from the schema, or make `insert`'s `symbol`
description read "the EXISTING symbol to insert relative to". A one-line hint on the
`missing 'symbol'` error ("for action=insert, `symbol` is the anchor") would have cost
zero round-trips. Kin U-26 (action grammar learned via sequential errors).

**Status:** open, mostly fixed (checked 2026-11-02: schema and refusal read, tool not called). `anchor` is gone from the `edit_code` input schema, and the `missing 'symbol'` refusal now carries a hint with an example. Still open: the `symbol` description says only "Symbol name-path" and never says that for `insert` it is the EXISTING anchor, so the misread is still possible.

---
### U-37 — `edit_file(replace_all=true)` right after an insert rewrote the line inside the helper being introduced, making it call itself

**Observed:** 2026-08-06, de-saturating `scan_meta.degraded` in
`src/librarian/tools/audit_doc_refs/resolver.rs`.

**Sequence, and the order is the whole bug:**

1. `edit_code(action="insert")` added a helper whose body was
   `ctx.degraded_languages.borrow_mut().push(lang.to_string());` guarded by an
   `if lang != "unknown"`.
2. `edit_file(old_string="ctx.degraded_languages.borrow_mut().push(lang.to_string());",
   new_string="note_degraded(ctx, lang);", replace_all=true)` to convert the two real
   call sites.

Step 2 matched **three** occurrences, not two — the third being the line inside the helper
from step 1. Result:

```rust
fn note_degraded(ctx: &ResolveCtx<'_>, lang: &str) {
    if lang != "unknown" {
        note_degraded(ctx, lang);   // <- unbounded recursion
    }
}
```

**Got:** `cargo check` **passed** — infinite recursion is not a compile error, and clippy's
`only_used_in_recursion` did not fire on this shape either. It surfaced as
`has overflowed its stack / fatal runtime error: stack overflow, aborting` (SIGABRT) in
`resolver_unknown_when_lsp_offline`, i.e. only because a test happened to exercise that
path.

**Severity:** med — no wrong code shipped, but the failure mode is a hard process abort and
the only thing that caught it was test coverage on the affected branch. A helper introduced
on a path with no test would have compiled, passed clippy, and recursed in production.

**Root cause:** `replace_all` is scoped to the file *as it is at that moment*, which
includes anything the previous call inserted. "Replace every call site" and "the helper
contains a copy of the call site" are the same string, and nothing distinguishes them.

**Fix idea, in order of preference:**

1. **Do the `replace_all` FIRST, then insert the helper.** The helper cannot be caught by a
   replace that already ran. Zero tooling changes; purely ordering.
2. Write the helper body differently from the call sites it replaces — here, using a local
   binding or a different receiver expression would have made the strings non-identical.
3. Tool-side: `edit_file` could report the **match count** on `replace_all` ("replaced 3
   occurrences"), which would have made this visible immediately. It currently returns a
   bare `"ok"`, so the caller never learns the arity of what they just did. That is the
   cheap generalisable fix, and it helps every `replace_all` caller, not just this shape.

**Status:** open — (1) and (2) are discipline and cost nothing; (3) is a small tool change
worth doing because a silent arity is what turned a two-site edit into a three-site one.

### U-38 — `edit_file(replace_all=true)` matched 1 of 2 intended sites (different indentation) and reported plain `"ok"`

**Observed:** 2026-08-07, mutation-testing the new walk-error guard in
`src/tools/symbol/symbols.rs`.

**Tried:** neutralise both `Err`-arm counters at once —
`edit_file(old_string="                    audit.errors += 1;", replace_all=true)`.

**Got:** `"ok"`. One of the two sites changed. The two statements are byte-identical apart from
leading whitespace: the accepted-files walk sits at 16 spaces, the tree-sitter fallback walk at
20, because it is nested inside `if matches.is_empty()`. My `old_string` carried 20, so
`replace_all` correctly replaced *all* occurrences of that string — one — and said so in the
only way it can: not at all.

**Iron Law / pattern:** not an Iron Law breach; a **silent partial application**. The response
is a bare `"ok"` with no match count, so "replaced 2" and "replaced 1" are indistinguishable
from the caller's side. `replace_all` reads as an intent ("every occurrence") while it is
actually a predicate ("every occurrence *of this exact string*"), and in Rust the same statement
at two nesting depths is two different strings.

**Cost, and it is the interesting part.** The partial mutation left one counter live, so the
test under examination still passed — which read as *"the test is vacuous"*. The next step I
nearly took was rewriting a correct regression test for a real defect. Two cheap checks
separated the explanations instead: `id -u` plus a `chmod 000` probe confirmed the test's
precondition genuinely held, and grepping for the mutation marker showed it had landed at only
one site. Recorded as F-10 in `docs/trackers/release-promotion-session-log.md`.

**Fix idea:** have `edit_file` return the match count (`{"ok": true, "replaced": 2}`) rather than
a bare `"ok"`. That single number would have made the partial application self-evident, and it
is the same class of gap as U-37 — both are cases where `edit_file` did exactly what it was told
and told the caller nothing about what that was. Two datapoints now, one session apart, same
tool, same missing field.

**Recurrence:** 1st observed for the indentation shape; 2nd for the "`edit_file` reports no
match information" family (see U-37).

**Status:** open — the workaround (grep for the marker after mutating) is a habit, not a
mechanism. A `replaced` count in the response would retire both halves.

### U-40 — A multi-line `old_string` failed as "not found" on text that was verbatim present, and the error cannot tell a bad needle from a bad haystack

**Valid:** dated 2026-10-01

**Observed:** 2026-08-17, adding a paragraph to `src/prompts/guides/iron-laws-detail.md`
after the "Bounded LHS is allowed" block.

**Call:** `edit_markdown(action="edit", heading="## Iron Law 3: …", old_string=<three
lines copied out of the file>, new_string=<the same three lines plus a new paragraph>)`.

**Got:**

```
old_string not found in section '## Iron Law 3: `run_command` output → buffer, not pipe'.
The text must match exactly (whitespace-sensitive).
scoped_miss_tier: "no_close"
```

Those three lines had just been read twice — once by `grep` against the file, once by
`get_guide("iron-laws-detail")` off the wire. They were verbatim present.

**Cause — mine, not the tool's, and that is the point.** The `old_string` reached the
server carrying literal `\n` two-character sequences instead of newlines, so it genuinely
was not in the file. A **single-line** anchor succeeded on the next call.

**Why it is still a friction.** The message and the remedy point in opposite directions.
*"not found … must match exactly (whitespace-sensitive)"* describes a haystack that moved,
so the reflex is to re-read the file — which I did, twice, and both reads *confirmed* the
text was there, which made the tool look wrong rather than the query. Nothing in the
response separates "your needle is malformed" from "the file changed".
`scoped_miss_tier: "no_close"` is the one field that does separate them, and it is
undocumented: a near-miss means the file drifted; `no_close` on text you just read means
your string is corrupt. The signal exists and says nothing to the reader.

**Verified, not assumed.** Two rival hypotheses: (H1) `edit_markdown` rejects multi-line
`old_string`; (H2) the escaping was mine. Probe — a two-line `old_string` with real
newlines against a scratch `.md` outside the project. It succeeded and replaced both
lines, refuting H1. Filing H1 as a tool bug would have been a false report against a tool
that behaved correctly; the probe cost two calls and one scratch file, deleted after. See
R-101 for the general form — name what the rival hypothesis predicts before recording a
verdict.

**Fix idea.** Make `no_close` self-describing instead of a raw tier name: when the miss is
`no_close` **and** the `old_string` spans more than one line, say which of the two causes
is likelier and name both. Exactly the shape of U-39 one tool over — the zero was right,
the silence about *why* was the defect.

**Also observed in the same act.** `append_entry`'s `next_step` said the heading must be
`## U-40 — <title>` (H2). This ledger's own augmentation prompt mandates
`### U-N — <title>` (H3), all 36 existing entries use H3, and `docs/TAXONOMY.md` says H3.
The resolver's `def_re` does not anchor on `#`, so both levels resolve — but an agent that
follows the response boilerplate writes a heading inconsistent with every sibling. The
generic hint should defer to the ledger's own convention, or say "match the surrounding
entries".

**The heading half is fixed — `bf485a00` (experiments, patch-id `c38669e53483eae178d772e0ec84a03df2c68792`).** `allocate_entry_id` now returns
the level the ledger's own entries use — the *mode* of `^#{1,6} PREFIX-N` over the body it
already scans for `body_max`, so a stray heading at another depth cannot decide the level
for every future entry — and `append_entry` phrases the hint with it. Where the body heads
nothing there is no level to observe, and the hint now says its suggestion is a DEFAULT
instead of sounding certain. Promoted to a standing rule the same day: Anti-Pattern 5,
*Asserting a Convention the Tool Never Read*, in `docs/PROGRESSIVE_DISCOVERABILITY.md` —
the file `CLAUDE.md` requires reading before adding or modifying any tool.

**Re-checked 2026-10-01 — the `no_close` half is narrower than this status says, and not closed.** `diagnose_scoped_miss` (`src/tools/markdown/edit_markdown.rs`) now gives each cause its own tier and wording: `visible_drift` (a `want`/`have` diff headed *did it change since you read it?*), `whitespace_invisible`, `no_similar_match` (*nothing scored above 0.5 similarity*), `old_string_empty`, `old_string_too_large`, `old_string_longer_than_section`, and the two section caps. Of these, `visible_drift` and `no_similar_match` were observed in a live session the same day. So "a malformed needle and a moved haystack in identical words" is no longer true, and `no_close` is now spelled `no_similar_match`. What is still absent is the case this entry was bitten by: nothing detects literal two-character `\n` sequences in `old_string` (a search of that file for backslash or escaped-newline handling finds none), so that needle lands in `no_similar_match`, whose hint still says to verify the heading or re-read the section. `scoped_miss_tier` remains undocumented to the caller.

**Status:** open — the heading half is fixed. The `no_close` half, which is the primary
friction above, is partly fixed (checked 2026-11-02 by reading the code, not run): `diagnose_scoped_miss` (`src/tools/markdown/edit_markdown.rs`) now gives separate tiers (`old_string_empty`, `old_string_too_large`, `section_too_many_lines`, `old_string_longer_than_section`, `no_similar_match`, `whitespace_invisible`, `visible_drift`), and tests in `src/tools/markdown/tests.rs` pin some of them. Still open: nothing detects an `old_string` that carries a literal backslash-n, which was the actual cause here, so such a needle gets the generic wording that blames the haystack, and `scoped_miss_tier` remains undocumented to the model.
### U-41 — `snapshot_stale` asserts the row "still shows the PREVIOUS field values" without knowing which fields the row renders

**Observed:** 2026-08-17, updating `BL-37`'s `next` field in the open-issue work queue via
`artifact(action="update_entry")`.

**Got:**

```
snapshot_stale: This tracker renders a snapshot in its body, and its `BL-37` row still
shows the PREVIOUS field values — params changed, the file did not.
```

The body's snapshot table renders `id | phase | task | status | bug`. `next` is not one of
them, so the row showed nothing stale at all. Acting on the advice would have meant editing
a row that was already correct.

**Mechanism — read, not inferred.** `snapshot_stale_note`
(`src/librarian/catalog/augmentation.rs`) takes `artifact_id`, `entry_id` and the set of
claimed indices. It does **not** take `changed_fields`, so it cannot know whether any
changed field appears in the rendered row. What it establishes is "the body keeps a
snapshot, and this row is in it"; what it says is "the row still shows the PREVIOUS field
values". The gap between those two sentences is the whole friction.

The function's own doc comment is honest about the limit — *"no id comparison can see
that, so the signal is 'you changed fields the body still renders the old way'"* — which
makes this a **wording** defect rather than a logic one. The check is deliberately
imprecise and documented as such; the message is not.

**Why it matters more than a cosmetic quibble.** It is the same shape as Anti-Pattern 5 in
`docs/PROGRESSIVE_DISCOVERABILITY.md`, added earlier the same day: a tool stating something
specific it never verified. An advisory that cries wolf on every field change trains the
reader to skip it — and the case it exists for (a *rendered* column going stale, which no
id comparison can catch) is genuinely valuable and genuinely invisible otherwise.

**Fix idea.** Either pass `changed_fields` through and fire only when one of them appears
in the rendered row, or soften the message to what was actually checked: *"you changed
fields on a row this body renders — check whether the rendered columns are affected."* The
first is better and costs one parameter; the second costs nothing and is honest today.

**Not filed as a bug** deliberately: nothing is wrong, corrupt, or lost, and the advisory
is load-bearing in its true case. This is friction with the wording of a correct-but-broad
check. Same reasoning that kept U-40 out of `docs/issues/` once the probe showed the tool
had behaved correctly.

**Status:** open, wording fixed (checked 2026-11-02 by reading the code, not run) — `snapshot_stale_note` (`src/librarian/catalog/augmentation.rs`) now says "this check reads IDS ONLY ... UNVERIFIED" and points to declaring `snapshot_anchor` (`e9995df34`, 2026-09-14). Still open: passing `changed_fields` through, so the note stops firing when no changed field is rendered.

---
### U-48 — I tripped IL3 three times in the session whose subject matter was IL3

**Observed:** 2026-08-18, in a session that fixed the IL3 warn-hook (U-44), rewrote the IL3 row in `docs/architecture/companion-plugin.md`, and put the rule into two guides. Three of my own calls were refused by that same gate: `cargo clippy ... 2>&1 | tail -5`, `git -C <repo> check-ignore -v <paths> 2>&1 | head`, and `cargo rb 2>&1 | tail -3`.

**Got:** all three are **true positives**. The gate was right every time, the rule was in context every time, and I had just finished writing prose about it.

**Why this is a friction entry and not a confession.** It is a field replication of A-24 on a sample of one agent: A-24 measured immediate compliance at 96% against **per-session repeat of 47% for `il3_pipe` and 71% for `il1`** — agents obey every refusal and still cannot predict the next one, because the message teaches the CALL and not the PREDICATE. My three slips are that shape exactly. Each refusal was obeyed correctly and immediately; none made the next pipe predictable.

The three share a form worth naming: **each was a convenience trim appended to a long-output build or git command** (`| tail -3`, `| head`) reached for to keep the response small — the exact habit `@cmd_*` buffers exist to replace. Knowing the rule as prose is evidently a different representation from applying it while composing a command.

**Cost:** three wasted round-trips and no wrong output, since IL3 denies rather than warns. The evidential cost is larger: an agent that has just authored the rule is not a clean subject for judging whether the rule is learnable, so my own compliance is not evidence about the surface.

**Fix idea:** none for the gate — it works. This belongs with A-24's finding that the refusal should name the predicate, not just the call. Worth citing there as the n=1 in-the-wild replication.

**Status:** open — evidence for A-24, no independent fix.

---
### U-49 — `entry_filter={}` is rejected instead of meaning "every entry"

**Observed:** 2026-08-18, trying to read all entries of a params ledger to backfill headings.

```
artifact(action="get", id="52451519052d207c", entry_filter={})
→ leaf must have exactly one field, got 0
  hint: Each leaf has shape `{field: {op: value}}`. Wrap multiple fields with `and`/`or`.
```

**Got:** an error whose hint is about leaf *shape*, which is not the mistake. The intent — "no constraint, return everything" — is the obvious reading of an empty filter and the natural way to ask for a whole collection.

**Workaround found:** `entry_filter={"id": {"contains": "<PREFIX>-"}}`, which matches every row because every id carries its prefix. It works, and it is a tautology dressed as a filter — the sign of a missing affordance rather than a real constraint.

**Cost:** small per instance, paid four times in one session (once per ledger backfilled), each time re-deriving the same tautology. The `get` response already carries `entry_total` alongside `entries`, so "all rows" is clearly an expected shape.

**Fix idea:** treat `{}` as the identity filter and return the whole collection, or refuse it with a hint that names the tautology workaround. The first is better: an empty AST node has one sensible meaning, and the filter engine already reports `entry_total` for the unfiltered set.

**Status:** open.

---
### U-46 — `audit_doc_refs` reads `e.g` and `1.16.8` as module paths and `contains/prefix` as a file path — two FP mechanisms, 12 of 50 findings

**Observed:** 2026-08-17, running a scoped `audit_doc_refs` over the four bug files written for U-42–U-45 as the doc-side gate before committing.

**Got:** `exit_code: 0`, `n_refs_found: 109`, `n_refs_resolved: 52`, `n_refs_broken: 29`, `n_refs_unknown: 28`. Reading the 50 shown findings rather than trusting the zero — R-104 — **none** was a real broken reference. The breakdown by kind:

```
36  file_path
12  module_path
 2  file_line
```

Two distinct false-positive mechanisms, both triggered by punctuation in prose:

| `raw_ref` | Classified | Actually |
|---|---|---|
| `contains/prefix` | `file_path` | two op names joined by a slash, quoted from a schema description |
| `ls/cat/awk/sed/find` | `file_path` | a command list from the gate's own hint text |
| `e.g` | `module_path` | an abbreviation |
| `1.16.8` | `module_path` | a plugin version number |
| `ext_re.is_match`, `expanded.is_relative`, `expanded.starts_with` | `module_path` | Rust method calls on locals, inside quoted code |
| `corrections.filter` | `module_path` | a JSON response key path |
| `usage.db` | `file_path` | a filename with no directory |

The remainder are legitimate non-resolutions and correctly reported: deliberate repro fixtures (`foo.rs`, `x.rs` — `foo.rs` *not* existing is the point of U-45), cross-repo `claude-plugins/…` paths, and ephemeral session-scratch paths in evidence sections.

**Mechanism — partly inferred, and said so.** Not traced to a line this pass. The pattern in the data is that a slash makes a token a `file_path` candidate and a dot makes it a `module_path` candidate, with no test that the shape is *plausible* for the kind — `e.g` has no path-like structure beyond the dot, and `1.16.8` is three integers. The `module_path` half is the same family as U-15 ("mis-parses Rust `::` separator + classifies git refs as paths") and U-17 (39 FPs from instructional placeholders), which is why this is one entry rather than three: the classifier has a recall-first bias and no cheap plausibility filter.

**Severity is lower than it first looks, and the correction is the point.** These never gate. CI runs `--fail-on high` (`.github/workflows/ci.yml:370`), and every one of these is `Med` after `issues_drop` capping — `fail_on=high` ignores Med (`src/librarian/tools/audit_doc_refs/mod.rs:1168`, pinned by `fail_on_high_ignores_med_severity`). An earlier read of this same data claimed they "would gate at med in an ADR"; that was wrong in the direction that matters, because it inflates a precision issue into a CI risk. The real cost is **report noise**: 29 broken + 28 unknown, all benign, is a haystack a reader has to walk before finding a real drift — and the tool is documented as *manual*, run when drift is suspected, which is exactly when a 57-item benign list is most expensive.

It would gate for anyone running `--fail-on med`, which is the setting a team tightening doc hygiene reaches for first.

**Fix idea:** add a plausibility filter before emitting, not after — reject `module_path` candidates whose segments are all-numeric (`1.16.8`), whose final segment is a known English abbreviation (`e.g`, `i.e`, `etc`), or that resolve to a local-variable method call already inside a fenced code block; reject `file_path` candidates with no path-ish segment and a known-prose separator. Cheaper alternative, and probably the right first move: extend the existing `cap_code_comment` treatment so a ref extracted from **inside a fenced block** in a bug file drops to `Low` — most of the 12 `module_path` FPs here are quoted code, and quoted code is a citation of nothing.

**Not filed as a bug** — same reasoning as U-40 and U-41. Nothing is wrong, lost, or gated; the classifier's recall-first bias is a deliberate design stance and the capping layer is already absorbing the consequence. This is friction with the precision of a correct-by-design check, and it belongs with U-15 and U-17 as evidence for a future precision pass rather than as a fifth open bug.

**Status:** partially fixed 2026-08-18 — the `module_path` half of the 12 sampled FPs. Verified before fixing: static trace of `has_known_ext` disagreed with this entry's own table for `usage.db` (claimed `file_path`; the extension list has no `db`, so it cannot classify that way). A live repro fixture confirmed the trace, not the table — `usage.db` classifies `module_path`, and this entry's mechanism table is stale on that one row.

`is_module_path` gained two rejections: no dot-separated segment set where every segment lacks an alphabetic character (kills `1.16.8`, a version number, and generalises — any all-numeric dotted token is rejected, not just this one), and a small denylist for `e.g`/`i.e`. TDD: two failing tests first (`a_bare_version_number_is_not_a_module_path`, `common_latin_abbreviations_are_not_module_paths`), a positive-control test guarding against over-rejection (`the_new_filters_do_not_touch_real_module_paths`, pinning `os.path` and `commits.git_root`), then the minimal fix. Gate: fmt 0, clippy 0, 4198 passed (4 new) / 0 failed.

**Deliberately left open:** the `ext_re.is_match`/`expanded.is_relative`/`corrections.filter` shape and the `contains/prefix`/`ls/cat/awk/sed/find` shape. Both are syntactically identical to real citations — the first to a genuine Python-style module path (`os.path`, `mrv.chat_app`), the second to a genuine extensionless directory path (`crates/librarian-mcp`) — and neither has a discriminator that doesn't also reject the real case. The method-chain shape is actually the *documented* markdown default (`PathSyntax::for_language(None) == DottedModules`, deliberately unchanged since SD-1's sweep, per this file's own doc comment) rather than a bug: an inline code span carries no language, so the classifier cannot know `ext_re.is_match` is Rust field access rather than a Python-style name. Forcing either heuristic risks trading false positives for false negatives on real content, which is the same mistake this entry's own analysis warned against ("no test that the shape is plausible for the kind").

---
### U-47 — Between `EnterWorktree` and `workspace(activate)`, writes are blocked but reads are not — so git reconnaissance answers about the wrong checkout

**Observed:** 2026-08-17, twice in one session, opening a worktree to fix the newline gap.

**Got:** `EnterWorktree` moves the *session* cwd into the worktree, and its post-hook says:

```
WORKTREE DETECTED: codescout must switch to the worktree.
MCP write tools (edit_code, edit_file, edit_markdown, create_file) are BLOCKED
until workspace is called — they would otherwise silently write to the wrong repo.
```

Writes are blocked. **Reads are not.** In that window `run_command` still resolves against
the previously-active project, so this — run from inside the worktree — described the main
checkout:

```
run_command("git log -1 --format='%h %s'; git rev-list --left-right --count experiments...HEAD")
→ 3ecb8730 docs(prompts): correct the … claim
  0   0
```

I read that as "the worktree is based on `experiments`, no reset needed" and moved on. The
worktree was actually at `eca9902e` — **origin/master, 1091 commits behind** — because
`worktree.baseRef: fresh` branches from the default branch. The real state only surfaced
when `edit_code` said `symbol not found` and `wc -l` gave 2199 lines against the 3403 the
file actually has.

**The tool did warn me, in that same response.** The `_workspace_notice` field said it
exactly:

> *Reads are resolving against "/home/marius/work/claude/codescout". This repo also has
> linked git worktrees […] and no project has been explicitly activated, so results
> describe the main checkout even if you are working in a worktree.*

So this is not a missing signal. It is a **correct signal in a sibling field, next to a
confident-looking answer** — and the answer wins attention. The asymmetry is what makes it
dangerous: a blocked write fails loudly and is unmissable; a read that quietly answers
about another tree looks like a finished reconnaissance step.

**Second, smaller defect in the same hook:** it names the wrong path. It says to call
`workspace(action="activate", path="<MAIN root>")` when the session has just moved *into* a
worktree — activating main would leave every subsequent path resolving to the checkout you
deliberately left. The right call is the worktree root, plus `read_only=false`, since a
foreign activation defaults to read-only (`get_guide("workspace-state")` § home/foreign).
I overrode the hook both times and it was right to.

**Cost:** two wrong conclusions about which tree I was on. The first time (earlier session)
I caught it from a suspicious line count; the second time only from a failed `edit_code`.
Either could have ended with tests written against a 1091-commit-old file and a "why does
this symbol not exist" investigation.

**Fix idea, in order of preference:**

1. **Have `EnterWorktree` activate the worktree itself.** It knows the path — it just
   created it. The manual step exists only to open this window.
2. **Block reads too, not just writes,** until a project is chosen when linked worktrees are
   present. The precedent is `guard_worktree_write`, which already refuses *writes* on
   exactly this ambiguity; the same argument applies to a read whose answer names a tree.
   The bug that armed that guard
   (`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`) made
   this same case and was closed on the write half only.
3. **At minimum, promote the notice from a sibling field into the answer** — prefix the
   `stdout` rather than adding a key beside it, so it cannot be read past.

**Verify-first rule this yields, cheap and general:** the first command after entering a
worktree should be `pwd`, and its output compared to where you think you are. One token of
output, and it is the only thing in the response that cannot be about the wrong tree.

**Filed:** `docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md`.

**Status:** mitigated — fix idea 3 (prefix the notice into `stdout`, not just a sibling field) landed in `src/tools/core/types.rs`'s `inject_notice`. Fix idea 2 (guard reads outright) was deliberately rejected: `worktree_read_notice`'s own doc comment already argues against refusing reads mid-orientation, and that reasoning holds. Fix idea 1 (`EnterWorktree` self-activates) remains open and lives in `codescout-companion`, a different repo — not done here.
### U-50 — Nothing surfaces concurrent writers in a shared checkout; a peer's uncommitted refactor appeared in my working tree mid-task

**Valid:** conditional — a concurrent-writer signal ships (session-start or `workspace(action="status")`), or worktree-per-session becomes the default for agent sessions

**Rests on:** `docs/trackers/archive/multi-agent-concurrent-coordination.md` — the fault line, its two prior concretes, and Option C

**When:** 2026-08-27, executing `docs/superpowers/plans/2026-08-27-operator-rules-phase-1.md` under `subagent-driven-development` in `/home/marius/work/claude/codescout`.

**Got:** I ruled against an isolated worktree — `experiments` is the repo's designated experimental branch, and a worktree costs `merge_worktree` catalog reconciliation — and dispatched a Task 1 implementer into the main checkout. Three minutes later `git status` showed two files modified that I had not touched: `src/tools/file_summary/file_summary.rs` (+147/−30) and `src/tools/markdown/edit_markdown.rs` (+12), a `HeadingQuery` occurrence-selector refactor. `ListAgents` showed **two other codescout sessions live and busy**, started 9h and 8h earlier. Nothing in the session-start surface, the activation bootstrap, the companion hooks, or `workspace(action="status")` had mentioned them.

**Cost:** the implementer's `cargo test` would have run against a tree holding a peer's half-finished refactor, so a red suite would have been unattributable — mine or theirs. Any `git add -A` would have swept their work into an operator-rules commit. The tell surfaced only because the operator overrode my ruling and instructed a worktree, which prompted a `git status` I had no other reason to run. Absent that instruction the first signal would have been a confusing test failure inside a subagent.

**Why this is a missing projection, not missing data:** `ListAgents` already reports every live session on the machine. `workspace(action="status")` reports the active project, index state and memories — not peers. `librarian(action="doctor")` reports worktree-scoped catalog rows but nothing about live sessions. The data exists one tool call away, and no surface an agent reads by default carries it.

**This is a third concrete on a fault line already scoped — and then archived while still scoping.** `docs/trackers/archive/multi-agent-concurrent-coordination.md` names the fault line, *"shared resource accessed by ≥2 concurrent agents with no transaction between observation and action"*, records two concretes (a `git reset --soft HEAD~1` race on shared HEAD; an F-N allocator collision), and offers three options — of which **Option C is worktree-per-session**, the only one whose stated scope is the fault line rather than the symptoms. Its Promote-when fires on *"a third concrete instance of the same fault line (whatever the shared resource is)"* and on *"concurrent-session rate exceeds 1 per day, sustained"*. Today supplies both: the shared **working tree** is a third resource its conventions never covered, and three codescout sessions were live at once. Its decision table's bottom row reads *"Daily, multiple collisions / week → Option C"*, against a recorded *"Today's state: 1 concurrent session pair this calendar quarter"* — a figure that is now false.

Its own Stale-when says archive when Option A, B or C ships, or when concurrent work stops for ≥3 months. Neither happened. The criteria that should have fired today stopped being swept the moment the file moved into `archive/`.

**Fix idea (cheap, independent of the structural one):** have `workspace(action="status")` and the session-start surface name other live sessions sharing this `git_root`. One line — *"2 other codescout sessions are active in this checkout"* — turns a silent race into a visible precondition, and it is a projection of data `ListAgents` already returns.

**Fix idea (structural):** restore the coordination tracker to `active` and decide Option C against its own recorded criteria, rather than re-deriving them from scratch next time.

**Leverage claim — recorded as a hypothesis, NOT as a finding.** The operator's reason for filing this is that worktree-per-session would close several partially-done problems at once. A grep for `merge_worktree|worktree-scoped|reseat_worktree|concurrent session|ListAgents` returns 254 matches across 69 files; these are the candidates that look like the same fault line:

- `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md`
- `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`
- `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md` (fixed 2026-08-30 — all four directions shipped)
- `append_entry`'s refusal to allocate ids from a worktree, documented in `get_guide("tracker-conventions")` — itself a per-resource mitigation on this line

**Nobody has checked whether these actually collapse into one fix.** Counting and classifying that set is the first task of any promotion, not an assumption to build on: a fix that names a population asserts the population is non-empty and homogeneous, and this one is so far only eyeballed.

**Status:** open

### U-51 — `file-provenance`'s window floor moves on commit, so a verification re-run answers a different question

**Observed:** 2026-09-01, twice in one hour, by two sessions, on the same affordance.

**What happened.** `scripts/file-provenance.py <path>` answers *"who wrote this uncommitted
file?"* over a window whose floor is **the path's last commit** (`file-provenance.py:325`:
*"Default is 'since this path was last committed'"*). That floor **moves**. Committing the
file pushes it past every write that constituted the evidence, so the identical command
returns a different answer before and after.

Measured on `src/util/librarian_guard.rs`:

```
run A   floor 2026-08-26T15:40:15   -> PEER, written by 3e275c54     (in-window)
run B   floor 2026-09-01T11:48:06   -> PEER, written by 3e275c54     (in-window)
c26943b5 commits the file at 14:59:59                                <- floor jumps
run C   floor 2026-09-01T14:59:59   -> UNKNOWN, "38 writes predate the window"
```

**Why it bites rather than merely surprises: a verification re-run silently answers a
DIFFERENT QUESTION.** A reader checking a past provenance claim gets `UNKNOWN`, concludes the
claim was unsupported, and files a correction — when the claim was correct and *the commit
erased its own evidence*. That is exactly what happened: a peer charged this session with
`bug-fix-session-log:F-92` (*corroborating a verdict with evidence from outside its window*),
which was the precisely wrong charge, and withdrew it once the floor movement was shown. The
`Session-Id` trailer on `c26943b5` independently corroborates the original in-window reading,
so the tool was right both times and the two answers are to different questions.

**The worse half — the tool prints a guard rail and it does not survive being read.** Its
`UNKNOWN` output says verbatim: *"That is a statement about coverage, NOT about ownership —
Bash writes this tool's heuristics miss look identical. Do not read it as 'not mine'."* The
peer **quoted that line in the message** and used the result as a refutation in the same
breath. A warning that fails inside the message displaying it is a mechanism problem, not a
comprehension one, and no strengthening of the wording fixes it.

**Fix ideas**, unranked, none implemented:

- **Print the floor's anchoring commit, not just its timestamp** — `since c26943b5
  (2026-09-01 14:59:59)`. A reader re-running to check an older claim then sees the anchor
  changed, which is the fact they need and currently have to infer.
- **`UNKNOWN` + "N writes predate the window" should name the newest of them.** The count
  arrives without its date, so the reader cannot tell whether the evidence is one minute or
  one month outside the window — and those warrant opposite reactions.
- **A `--since <commit|time>` escape**, so a past claim can be re-run against the window it
  was actually taken under. Verification is the whole use case and it is the one the default
  cannot serve.

**Cross-cite:** `bug-fix-session-log:F-80` — *never close an authorship question by
elimination; identify positively from write history*. Its third instance fired in this same
exchange. The two are one root cause from opposite directions: `F-80` is reaching for the
wrong instrument; this is the right instrument answering a question you did not notice you
had changed.

**Status:** open

**Valid:** conditional — `file-provenance.py` gains a `--since` escape or prints its floor's
anchoring commit

**Rests on:** the default window remaining commit-relative, stated at
`scripts/file-provenance.py:325`, which is the whole mechanism.

### U-52 — Subagents natively `Read` source files 3.47 times per 100 calls, against 0.14 for parent sessions, mostly ranged reads in two worktrees

**Valid:** dated 2026-11-02

**Observed:** 2026-11-02, scanning 204 transcript files (21 parent sessions, 183 subagent files, deduplicated by name and size) of this project since 2026-10-25 across the four profiles. Subagents made 8,626 tool calls, 16.9% of them native tools, and 299 native `Read` calls on source files, 3.47 per 100 calls. The parent sessions made 16,988 calls with 23 such reads, 0.14 per 100. The 2026-10-24 usage report measured 0.45 per 100 for subagents.

**What the reads were.** In a second pass that counted every record type (445 reads, so the absolute number is not settled): 66% were Python files in the `setup-ab-eval` worktree, 20% Rust files in the `bg-log-reaper` worktree, 87% were ranged reads (`offset` or `limit`). The 5 subagents with the most reads carry 76 of them, so the reads are spread, not concentrated. 79 subagents natively read a file under `.worktrees/`; their codescout calls were 1,194 of 2,099 with a `workspace=` argument (57%).

**Not established:** why. One reading is that agents working in a worktree read natively because codescout reads resolve against the main checkout unless pinned; the data does not show it, since an agent with 281 codescout calls and no `workspace=` argument made only 14 native reads. The two window sizes and project sets differ from the 10-24 report, so the change from 0.45 to 3.47 is direction only.

**Cost:** each native `Read` of source skips `symbols` and its outline, and returns file text into the agent's context.

**Status:** open. No hookify rule proposed: the cause is not known, and a rule would block reads that may be the only way to see worktree files.

**Rests on:** `sweep-transcripts.py` and `sub-read-what.py` run on 2026-11-02 (scripts kept in the session job directory, not in the repo).

## Template for new entries

<!-- Insert new U-N entries above this line.

  ONE CALL allocates the id and writes the section:

    doc(action="append_entry", id="c43df94e69ca915f", id_prefix="U",
             anchor_heading="## Template for new entries",
             title="<title>", body="**Status:** ...")

  Passing `anchor_heading` + `title` + `body` TOGETHER is what selects that path.
  Omit any of the three and the call only reserves the id, leaving a
  reserved-but-unwritten number as a live failure mode.

  Entries here are `### U-N — <title>`. The dash-and-title is not decoration:
  `link_scan` defines a citable token in that shape and in no other, so a heading
  without it defines nothing and every citation of that id dangles.

  This ledger has no index table — the headings ARE the index. Never hand-grep the
  highest U-N: a max read is stale by the time you write, and the allocator already
  counts ids the body claims.

  `edit_file` is refused here — the ledger declares `entry_prefix`.

  ADDED 2026-09-10. `docs/TAXONOMY.md` had prescribed this exact call, naming this
  exact anchor, since before the heading existed — so the documented one-call path
  degraded silently to reserve-only. Found while fixing the same defect in the H-N
  ledger; 2 of the 6 trackers TAXONOMY names with a concrete artifact id were
  affected, the other 4 already had the heading.
-->
