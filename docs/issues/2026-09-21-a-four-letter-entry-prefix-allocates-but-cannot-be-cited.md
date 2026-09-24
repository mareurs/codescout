---
id: '219027f500266ee4'
kind: bug
status: open
title: 'BUG: a four-letter entry prefix allocates cleanly and cannot be cited, and one is in active mandated use'
tags:
- cluster/addressing-without-an-escape-hatch
topic: tracker-entry-identity
opened: 2026-09-21
owner: marius
related: []
severity: medium
---

## Summary

The entry-token grammar is `\b[A-Z]{1,3}-\d+\b`, so a prefix of four letters is
**unrepresentable as a citation**. The allocator does not share that bound: it accepts a prefix of
any length, mints ids under it and commits its high-water mark. `DCTX` — four letters — is the
prefix `CLAUDE.md:423` mandates capture into until 2026-10-02, so entries are being written under a
namespace no citation can address. Nothing is lost and nothing errors; the citation simply cannot
be expressed, and because the scanner never tokenizes `DCTX-1` at all it is not reported dangling
either. Invisible in both directions.

## Symptom (Effect)

`docs/issues/2026-09-20-observation-window-yields-zero-prospective-samples.md` cites three entry
ids on three consecutive table rows, in byte-identical syntactic positions (lines 36–38):

```
| DCTX-1 | Historical seed — recipient-specific guide delivery | `historical-seed / …` |
| DWF-1 | Historical seed — discriminate an edit-miss hypothesis | `historical-seed / …` |
| DCS-1 | Collection setup — partial-session coverage | setup |
```

That file contains **zero** markdown links (`grep -n '](' …` → no output), so every edge it
produces comes from an entry token. `librarian(action="link_scan", write=false)` derives five
`edges_missing` rows from it:

```
IC-15-accepted-parameter-silently-dropped.md      <- IC-15
skill-frictions.md                                <- SKF-22
deep-agent-workflow-observations.md               <- DWF-1 / DCS-1
observer-blindness.md                             <- OB-7
IC-3-declared-not-wired.md                        <- IC-3
```

`docs/trackers/deep-agent-context-observations.md` — the file that defines the entry cited on
line 36 — is **absent**. Two three-letter siblings on the adjacent rows yield an edge; the
four-letter one yields nothing.

No finding names it either. Over the full report (286,408 bytes, `findings_limit=1000`:
`ambiguous` 636, `dangling` 687, `cross_repo` 84, `prefix_conflicts` 3) `grep -c 'DCTX'` returns
**0** — so it is in no findings array and `prefix_conflicts` does not hold it. `librarian(action="doctor")`
returns 193 violations; `grep -c 'DCTX'` over them is **0** as well.

## Reproduction

1. `git rev-parse HEAD` → `0f11fc40` (branch `experiments`), main checkout.
2. Apply the two live regexes to the token:

   ```
   python3 - <<'PY'
   import re
   entry = re.compile(r"\b[A-Z]{1,3}-\d+\b")
   defre = re.compile(r"^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+")
   for s in ["DCTX-1","DWF-1","DCS-1"]:
       print(repr(s), entry.findall(s), bool(defre.match(s + " — title")))
   PY
   ```

   →

   ```
   'DCTX-1' [] False
   'DWF-1' ['DWF-1'] True
   'DCS-1' ['DCS-1'] True
   ```

3. `librarian(action="link_scan", write=false, findings_limit=1000)`, then
   `grep -c 'DCTX' <buffer>` → `0`.
4. `librarian(action="doctor")`, then `grep -c 'DCTX' <buffer>` → `0`.
5. Confirm the allocator disagrees: `head -20 docs/trackers/deep-agent-context-observations.md`
   shows `entry_prefix: [DCTX]` and `entry_high_water_DCTX: 1`. That key is written by the
   allocator inside its own transaction, never by a caller
   (`src/librarian/catalog/augmentation.rs:1106-1110`), so its presence is server-side evidence
   the four-letter prefix was accepted.

**Never `write=true`** on a shared checkout — it prunes.

## Environment

codescout `experiments` @ `0f11fc40`, main checkout (not a worktree), Linux, MCP over the release
binary. Measured 2026-09-21T15:48Z–15:54Z UTC. The catalog reported
`provenance.head_commit: 170eac15` for a `doc(action="get")` in the same window while
`git rev-parse HEAD` read `0f11fc40`; noted, not investigated, and not load-bearing here.

## Root cause

**Three readers of one namespace, and only one of them is bounded at three characters.**

| reader | `path:line` | accepts `DCTX`? |
|---|---|---|
| citation scanner (`entry_re`) | `src/librarian/tools/link_scan/extract.rs:253-256` | **no** — `\b[A-Z]{1,3}-\d+\b` |
| definition scanner (`def_re`) | `src/librarian/tools/link_scan/extract.rs:319-322` | **no** — `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`, anchored, so it cannot slide onto `CTX-1` |
| declared-prefix reader, guard side (`clean_prefix`) | `src/util/librarian_guard.rs:343-347` | **no** — `p.len() <= 3 && all ascii_uppercase` |
| declared-prefix reader, allocator side | `src/librarian/catalog/augmentation.rs:1209-1223` | **yes** — any non-empty trimmed string, no length or case rule |
| body-index reader (`body_claimed_indices`) | `src/librarian/catalog/augmentation.rs:2030-2040` | **yes** — `regex::escape(id_prefix)` of any length |

`def_re` is anchored at `^\s*`, which is why the slide that rescues `cross_repo_re` cannot help
here: `[A-Z]{1,3}` consumes `DCT`, the next byte is `X` not `-`, backtracking to `DC` and `D`
fails the same way, and there is no later start position to try. `entry_re` is unanchored but
every interior position of `DCTX` is preceded by a word character, so no `\b` exists to start a
match at `CTX-1`. The token is not mis-parsed; it does not exist.

So an id under `DCTX` allocates correctly, is written to a heading in the exact shape
`def_re` requires, and defines nothing. There is no qualifier, no escape, and no
`<stem>:<TOKEN>` form that recovers it — the qualified regexes at `:287-289` and `:308-313`
end in the same `[A-Z]{1,3}-\d+`.

**The bound is documented in exactly one place, and it is a place a ledger author does not
read.** `src/librarian/tools/artifact.rs:301` — the `to` parameter of `rekey_prefix` — says
*"Max 3 uppercase ASCII: the token grammar is `[A-Z]{1,3}-\d+`, so a longer prefix is honoured by
the allocator but INVISIBLE to the citation scanner."* That sentence describes this defect
precisely. It is attached to the one action nobody performs when *creating* a ledger, and
`append_entry`'s own `id_prefix` description says nothing about length. Verified by reading both
descriptions at `:301` and `:303-305`.

**And the test that exists to catch this exact disagreement cannot.**
`both_entry_prefix_readers_agree_on_every_yaml_form`
(`src/librarian/catalog/augmentation.rs:4105-4168`) exists because the two `entry_prefix` readers
must agree, and its own doc comment names the failure direction: *"the allocator honours a form the
guard is blind to, so entries in that ledger can be hand-written past the allocator with no error
anywhere."* That is the state on disk today. Its thirteen fixtures use `R`, `HY`, `F`, `W`, or an
empty/absent value — **no fixture exceeds two characters**, so the length rule is never exercised
and the disagreement is unreachable by the assertion. `CLAUDE.md` § *Testing Discipline*: a
population selected so no member can falsify.

Mechanism read at the bytes and confirmed against the live tools for the *consequences* (link_scan
and doctor both silent). The Rust-level disagreement between the two `entry_prefix` readers is
**derived from the two function bodies**, not observed by running the test with a four-letter
fixture — that would require building, which was out of scope for this session.

## Evidence

### The three-adjacent-rows control

The discriminator is not an absence argument. Lines 36, 37 and 38 of
`docs/issues/2026-09-20-observation-window-yields-zero-prospective-samples.md` are table rows of
identical shape, differing only in the prefix. Two produce a `cites` edge, one produces nothing,
and the file has no markdown link that could confound the attribution. The positive control and
the negative case are in the same sentence of the same file.

### Which prefixes in this corpus exceed three letters

Exactly one. `docs/trackers/deep-agent-context-observations.md` declares `entry_prefix: [DCTX]`
(4). Its companion `docs/trackers/deep-agent-workflow-observations.md` declares
`entry_prefix: [DWF, DCS]` — **both three letters, both fine**, which is why the workflow half of
the same mandate is unaffected. The `DCS` receipts `CLAUDE.md:435` asks for are citable; the
`DCTX` samples `CLAUDE.md:423` asks for are not.

### The declaration route is closed too

`docs/issues/archive/2026-09-04-a-namespace-owned-outside-the-corpus-cannot-declare-itself.md` records
that `cited_prefix_with_no_definer` has two silence conditions — a heading definer, or an
`entry_prefix` declaration. Neither is available here: the heading cannot define
(`def_re` rejects it) and the declaration is discarded by `clean_prefix`. The check is silent for a
third reason as well — it counts *citations* of a prefix, and this prefix produces zero.

### Not the archived prefix-gated bug

`docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md`
(`status: fixed`) is the same *symptom* one layer up: `WIN-N` — three letters, inside the grammar —
had 129 citations reclassified as prose because the prefix had no definition. Its remedy (add
defining headings; `cited_prefix_with_no_definer`) is reachable for a three-letter prefix and
unreachable for a four-letter one, because there are no tokens to count.

## Hypotheses tried

1. **Hypothesis:** the tokens are extracted and reported dangling, just not as edges.
   **Test:** `link_scan` at `findings_limit=1000` (the full 687-row `dangling` array, not the
   50-row cut), `grep -c 'DCTX'`. **Verdict:** rejected — `0`. The earlier `findings_limit=50`
   run was a cut list and its own summary says *"absence from a cut list is not evidence"*; the
   full run is what settles it.
2. **Hypothesis:** `DCTX` shows up in `prefix_conflicts`. **Test:** same grep over the full
   report, which holds all 3 `prefix_conflicts` rows. **Verdict:** rejected — the prefix is
   invisible to the scanner entirely, which is the point rather than an incidental miss.
3. **Hypothesis:** `def_re` slides and matches `CTX-1`, so the entry defines a *different* token.
   **Test:** the regex run in § Reproduction step 2. **Verdict:** rejected — `def_re` is anchored
   and `entry_re`'s `\b` cannot open inside `DCTX`. No token, not a wrong token.
4. **Hypothesis:** already filed. **Test:** `grep -rln 'DCTX'` over `docs/ src/ tests/ scripts/
   CLAUDE.md` → four trackers plus `CLAUDE.md`, no bug file; and
   `grep -rlniE 'prefix.*(too long|length|four|invisible to the citation)|A-Z\]\{1,3\}'` over
   `docs/issues/` → 17 files, every one about zero-padding, qualifier truncation, prefix collision
   or the dangling gate, none about the length bound. **Verdict:** rejected — not filed.
5. **Hypothesis:** the allocator refuses a four-letter `id_prefix`, so nothing was ever written
   under it. **Test:** `entry_high_water_DCTX: 1` is in committed frontmatter, and that key is
   written by `allocate_entry_id` inside its own transaction
   (`src/librarian/catalog/augmentation.rs:1106-1110`). **Verdict:** rejected — the allocator
   accepted it. Not re-tested by calling `append_entry`, deliberately: that would add a row to a
   live observation ledger as a side effect of investigating it.

## Fix

Not fixed here. Three directions, and they are not equivalent:

- **Refuse at the allocator.** Reject an `id_prefix` the citation grammar cannot express, at
  `append_entry` and at `rekey_prefix` alike, with a message naming the three-character bound.
  This is the only direction that makes the correct path end in a safe state — a ledger cannot be
  born uncitable. It does not repair the ledger that already exists.
- **Widen the grammar** to `[A-Z]{1,4}` (or more) in `entry_re`, `def_re`, `cross_repo_re` and
  `double_qualified_re`. Note the cost before reaching for it: every widening enlarges the set of
  prose strings read as citations, which is the mechanism behind
  `docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`.
- **Rename the namespace** — `doc(action="rekey_prefix", from="DCTX", to="DCX")` (or similar), which
  moves the params ids, the schema pattern, the defining headings, the in-body citations and the
  `entry_cite` rows in one dry-run-by-default transaction. This is the cheapest repair for the live
  ledger and the one the schema note at `src/librarian/tools/artifact.rs:301` is written for. It
  leaves the class open: the next author can pick a four-letter prefix again.

Whichever lands, **add a four-letter fixture to
`both_entry_prefix_readers_agree_on_every_yaml_form`** and observe it red first. Its thirteen
fixtures are the reason this shipped, and a fix that leaves that population unchanged is guarded by
nothing.

## Tests added

None — nothing was fixed. The regression test this needs is a red on
`both_entry_prefix_readers_agree_on_every_yaml_form` with an `entry_prefix: DCTX` fixture, plus one
asserting that a four-letter `id_prefix` is refused (or, if the grammar is widened instead, that
`DCTX-1` produces a `Definition` and a `Citation`).

## Workarounds

Cite the ledger by **path** and the entry by **prose label**, not by token:
`docs/trackers/deep-agent-context-observations.md` § *Historical seed — recipient-specific guide
delivery*. A markdown link to the file does produce a `cites` edge; only the entry-grain address is
unavailable. Writing `DCTX-1` is not an error and not a citation — it is prose.

## Resume

Decide between refuse-at-the-allocator and rekey, then do it before the window closes on
2026-10-02, because the ledger keeps growing uncitable entries until then. If rekey is chosen, run
`doc(action="rekey_prefix", id="0cc578bbc332d699", from="DCTX", to="DCX")` dry-run first and read
which citing files it reports as needing prose repointing — `link_scan` re-derives their edges only
after their prose moves. Do not run `librarian(action="link_scan", write=true)` on a shared
checkout to verify; re-run the `write=false` grep from § Reproduction step 3 and expect `DCTX` to
appear once the prefix is three characters.

## References

- `docs/trackers/deep-agent-context-observations.md` — the `DCTX` ledger (artifact `0cc578bbc332d699`)
- `docs/trackers/deep-agent-workflow-observations.md` — `DWF` / `DCS`, both three letters, both fine
- `CLAUDE.md` § *Deep-agent observation window* — the mandate, lines 423 and 435
- `docs/issues/2026-09-20-observation-window-yields-zero-prospective-samples.md` — the file whose
  three adjacent table rows are the control
- `docs/issues/archive/2026-09-04-a-namespace-owned-outside-the-corpus-cannot-declare-itself.md` — the
  sibling: a namespace that cannot declare itself because its authority is outside the corpus. Here
  it is inside the corpus and the declaration is discarded by a length rule.
- `docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md` — the opposite
  polarity of the same class, and the reason widening the grammar is not free
- `docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md`
  — same symptom, three-letter prefix, fixed
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md` — the class
