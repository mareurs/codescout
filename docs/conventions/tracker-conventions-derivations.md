# tracker-conventions — the full wording behind the guide

`src/prompts/guides/tracker-conventions.md` was 55,261 B. A direct
`get_guide("tracker-conventions")` returned 56.6 KB, and Claude Code saved a result of that
size to `tool-results/*.json` and showed the model a 2 KB preview. The guide is now cut to the
rule and the trap of each section. This file keeps what the cut removed, under the guide's own
headings, so a rule can be traced to its reasoning.

Each block has three parts:

- **Kept in the guide** — the idea that stayed.
- **Also stated in** — the other pages that already say the same thing. The guide points at
  them instead of repeating them, and this file does not copy them.
- **Pre-cut text** — the removed wording, verbatim, from blob `8df4fd692a4de79118eaf1eb2d060862dc4188a1` (`git show 8df4fd692a4de79118eaf1eb2d060862dc4188a1`), with the
  source line numbers. Where a section appears here with no pre-cut text, the other page holds
  everything the section lost.

## Editing the guide: what pins it

- **Scope: generic mechanics only.** The guide names no artifact kind that one repo's pattern
  defines. A bug file's procedure belongs in that repo's template (codescout:
  `docs/issues/_TEMPLATE.md`), and `CLAUDE.md` § *Bug Tracking* says so.
  `tracker_conventions_names_no_repo_specific_artifact_kind` fails if the guide names
  `docs/issues`, `_TEMPLATE`, `kind: bug`, `bug file` or `Bug files`. To document another
  repo-specific kind, put it in that repo's template and leave a pointer in `CLAUDE.md`.
  `no_guide_names_a_path_or_field_only_this_repo_has` covers every guide the same way
  (`src/prompts/README.md` rule 10).
- **Pointers removed on 2026-11-11.** The guide named three things that exist only in this
  checkout. They were: this file, in the intro ("The full wording, the derivations and a
  pointer to every other page that states the same thing are in
  `docs/conventions/tracker-conventions-derivations.md`, under the same headings."); the restore
  procedure after `export_augmentations` ("Restore procedure:
  `docs/conventions/cross-machine-catalog-resume.md` § 7."); and the guard source after
  "Declare `entry_prefix`." ("(`src/util/librarian_guard.rs`)"). The Querying section also
  said "the session-passover pattern:" where it now says "trackers that carry cross-session
  behavior:", because that pattern moved to `docs/conventions/librarian-derivations.md`.
- **Headings.** `SECTION_WAIVERS` and the routing tests in `src/prompts/mod.rs`,
  `src/prompts/guide_index.rs` and `src/server.rs` key on the exact heading text, and other pages
  cite sections as `§ *Heading*`. Rename none. The three archive headings start `Archive —`:
  `librarian.md` cites `§ *Archive — stage*`, a unique prefix, to keep that section short.
- **`serves:` lines.** They decide which call receives which section. Each declared section must
  stay within `MAX_DECLARED_SECTION_BYTES` (2,500 B).
- **Section openers.** Server tests use the first words of a section as proof that it was
  delivered. Keep these strings: `Archive through the catalog`,
  `**And stage BOTH halves of the move together`, `**Then re-point the citations, in the same commit as the move`,
  `**Verifying the sweep afterward needs a different pattern`, `sees BOTH forms, and tells you which is`
  (on one line), `Be precise about *why* a green run proves nothing here`,
  `A tracker that accumulates review passes by *appending*`, `If an artifact is meant to carry an augmentation`,
  `**Before you augment:`, `replaces the WHOLE collection`, `Never suffix an id`, `Cite by **bare token**`,
  `**Cite by ID in prose.**`, `The canonical` and `The ladder is **live body`.
- **Fix-anchor tokens.** They are pinned in `docs/issues/_TEMPLATE.md`, no longer in the guide:
  `## Fix provenance`, `- **SHA:**`, `- **patch-id:**` and `no_fix_commit` must stay there
  (`bug_template_states_the_fix_anchor_grammar_doctor_enforces`).
- **Per-call share.** `tracker_write_shapes_ship_their_sections_and_only_theirs` requires that no call
  delivers half or more of the whole guide. It required a quarter until 2026-11-11, when the guide
  fell to about 15 KB and `append_entry` still delivered about 5.4 KB. What it proves is that a
  write never ships the whole guide.
- **Byte constants.** `WRITE_CEILING_*` and `WRITE_FLOOR_APPEND_ENTRY` in `src/prompts/guide_index.rs`
  hold the delivered sizes. Re-measure after an edit and ratchet them down.


## Claiming and releasing a bug

**Not in the guide since 2026-11-11:** it moved to `docs/issues/_TEMPLATE.md`. It said: claim `taken` with your own sessionId through the catalog; release to `investigating`.

**Also stated in:**
- `docs/issues/_TEMPLATE.md` (status semantics and the claim call)

**Pre-cut text:**

<!-- blob lines 54-61 -->

You already know your sessionId without any API call: the harness makes it a path
component of your scratchpad directory, `$TMPDIR/claude-<uid>/<project>/<SESSION-ID>/scratchpad`.

**Store the sessionId and nothing else.** Not the session name — that is registry-minted
with `nameSource: "derived"` and re-minted by compaction, resume, or a restart under
another profile, so a name frozen into frontmatter decays silently while the sessionId
does not. Not the pid or socket path either; both are derived at read time.

## The unverified caveat, made queryable / Settled caveats — STANDING and TRACKED

**Not in the guide since 2026-11-11:** it moved to `docs/issues/_TEMPLATE.md`. It said: `unverified:` records what the status overstates; discharge markers settle it; `STANDING` and `TRACKED` are the two non-discharge first words.

**Also stated in:**
- `docs/issues/_TEMPLATE.md` ("Optional `unverified:`": the markers, the leading-and-uppercase rule, `MEASURED`, `STANDING`, `TRACKED`)

**Pre-cut text:**

<!-- blob lines 105-110 -->

A blocker stated only in prose is unreachable by a query, so
`doc(action="find", kind="bug", status="open")` — the triage query — hides a `fixed`
record whose own body admits it has no test and does not prevent recurrence.

The system's scarce resource is not candour; it is **legibility**. Authors already write
the caveat. Write it where a query can read it.

## Trigger rules and capture discipline

**Not in the guide since 2026-11-11:** it moved to `docs/issues/_TEMPLATE.md`. It said: open a bug file for any bug noticed, at once; archive once the fix is verified on `experiments`; carry the labelled SHA.

**Also stated in:**
- `docs/issues/_TEMPLATE.md` (the yes/no cases and the archive trigger)

**Pre-cut text:**

<!-- blob lines 129-139 -->

**Archive trigger:** move the file into `docs/issues/archive/` once the fix is
verified on `experiments` — gate green (`./scripts/gate.sh`, which `CLAUDE.md` § *Development
Commands* prescribes) and a regression test in place. Reaching `master` is **not** required:
`experiments` is never deleted, so an unmerged fix is not at risk of being lost, and
holding the file back only grows a pile of `fixed`-but-unarchived bugs that no query
ever surfaces (`doc(action="find", kind="bug", status="open")` filters on
`status`, not on path).

The file MUST carry the fix SHA when archived experiments-only, because nothing
re-reads `archive/`: label it **`experiments`**, since an `experiments` SHA orphans on
rebase and an unlabelled one in `archive/` becomes an untraceable string.

## Closing a bug — patch-id and fix provenance

**Not in the guide since 2026-11-11:** it moved to `docs/issues/_TEMPLATE.md`, which now states the `## Fix provenance` block. It said: record a patch-id beside the SHA; write the pair as a `## Fix provenance` section; never cite a merge commit's patch-id.

**Also stated in:**
- `docs/RELEASE.md` § *Citing a fix: SHA + patch-id*
- `docs/issues/_TEMPLATE.md`
- test `bug_template_states_the_fix_anchor_grammar_doctor_enforces` (`src/prompts/mod.rs`) pins `## Fix provenance`, `- **SHA:**`, `- **patch-id:**` and `no_fix_commit` in the template

**Pre-cut text:**

<!-- blob lines 143-161 -->

**And record a `patch-id` next to it, because the sentence above describes a wound with
no remedy.** `git show <sha> | git patch-id --stable` is a content hash of the commit's
diff: it is invariant under rebase and cherry-pick, so it still finds the change after the
SHA dies. Resolve one with redirects, not
pipes — a pipe from an unbounded `git log -p` into a trimmer is refused by Iron Law 3 when the
capture cannot be kept (always in the background), and a capture keeps at most 4 MiB of it:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep <first-12-of-patch-id> /tmp/patch-ids.txt
```

**Write the pair as a `## Fix provenance` section — the SHAPE is what `doctor` reads, and
prose does not discharge it.** `librarian(action="doctor")`'s
`terminal_status_without_fix_anchor` refuses a `fixed` or `mitigated` record that declares
no pointer, and its sharpest measurement is that a loose hash in prose is **worse than
none**: a record carrying one does not merely lack an anchor, it *reads as* anchored, so a
reader scanning for provenance finds the commit the bug was OBSERVED at and stops looking.

<!-- blob lines 170-224 -->

**Plural, because a fix is not always one commit.** Repeat the pair; each `- **patch-id:**`
binds to the `- **SHA:**` above it, not to the first in the file. Do not use a table: it
is invisible to every check, so anchors recorded in one are verified by nothing.

**A fix that landed in another repository takes the `<repo>:<sha>` prefix** —
`- **SHA:** `codescout-companion:b8ffa8b`` — which the dead-pointer check skips rather than
reporting unresolvable, since it cannot see that repo's object DB.

**A fenced example is a quotation, not a declaration** — including the one just above, which
is why quoting the shape here costs nothing. The corollary is the trap: a block you fence
inside your own bug file declares *nothing*, and the record still reads as anchored to a
human.

**If the record genuinely closed with no commit** — a mitigation that was a doc note, a
defect that turned out to be environmental — say so in frontmatter rather than leaving the
section absent, which is indistinguishable from forgetting:

```yaml
no_fix_commit: "mitigation was a doc note; nothing was committed"
```

An empty value does not discharge it. The check wants the reason, not the field.

**A merge commit's patch-id is never citable — but it is not always EMPTY, and the
difference decides whether you notice.** For a **clean** merge `git show <merge>` emits
the message with no diff (the tree is derivable from the parents, so there is nothing to
print), and `git patch-id` given no patch prints **nothing and exits 0** — the pipeline
returns empty, with no error. For a merge that **resolved a conflict**, the tree is *not*
derivable from the parents, so `git show` emits a **combined diff** and the pipeline
returns a well-formed 40-hex value that hashes only the resolution hunks — not the change
the merge delivers, matching no constituent commit, and not stable across a different but
equally valid resolution of the same conflict.

**Both outcomes mean the same thing: cite the merged branch's constituent commits** by
SHA + patch-id; each has an ordinary patch-id that survives rebase normally. Never record
an empty patch-id field, never substitute the SHA for it, and never record the value a
conflicted merge hands you — the empty case you would have noticed, and that one you
would not. Nor manufacture one from `git diff <first-parent>..<merge>`: it hashes, but not to
the object a cherry-pick would reproduce, so it is a plausible value in a field that means
something else.

The fix commit carries the citation; name the merge separately as an event — *"Fixed
at `<sha>`, merged to `experiments` at `<merge-sha>`"*.

**There is no promotion path to check and no pending-master-SHA `## Resume` line.**
Recording the SHA *and* its patch-id replaces both. Whichever way the fix reaches
`master` — fast-forward or cherry-pick — the pair stays resolvable, so nothing is owed
later and no session has to come back and reconcile.

Check where a SHA currently lives with `git branch --contains <fix-sha>`.

An archived bug file that promises a master-side SHA "still to be recorded" is a stale
instruction, not open debt — do not sweep it.

When `doc(action="update")` sets a bug to `fixed` or `mitigated` and the file as written has no recoverable pair, the response carries `fix_anchor_advisory` naming what is missing. The write still happens; the advisory informs, and archiving stays refused until the pair or `no_fix_commit:` is present.

## Archive — through the catalog, never git mv

**Kept in the guide:** Archive with `doc(action="move")`, never `git mv`; the move re-keys the artifact.

**Also stated in:**
- the `new_rel_path` description of the `doc` tool

**Pre-cut text:**

<!-- blob lines 233-238 -->

**The move mints a new id, and reports it.** Because the id is derived from the
path, archiving necessarily re-keys the artifact; `move` grafts its events, links,
observations and augmentation onto the new id in the same call and returns
`{"id": <new>, "previous_id": <old>, "id_changed": true}`. Read the new id from the
response — the old one stops resolving immediately, and a later call with it
returns `unknown id`.

## Archive — stage both halves of the move

**Kept in the guide:** Stage both halves; never confirm on `R` alone.

**Also stated in:**
- `stage_hint` in every `move` response (`src/librarian/tools/mv.rs`)
- `librarian` guide § *Archiving / Moving Trackers*

**Pre-cut text:**

<!-- blob lines 243-261 -->

**And stage BOTH halves of the move together — `git add -- <old> <new>`.** At the
catalog layer the move is atomic; on disk it is a tracked *deletion* plus an untracked
*addition*, so every selector defined over index entries — `git add -u`, `git commit
-a` — enumerates only the deletion and commits an archive that silently un-happened.
There is no count of what was missed, because an untracked file is never a skipped
candidate: it is never enumerated. The positive confirmation is that **both halves are lettered in column 1** of `git
status --short` — either one `R` line, or a `D` plus an `A`; a leading space (` D`) or a
`??` is half-staged. **Do not confirm on the `R` alone.** `R` is git's *similarity*
verdict, so it is monotone in both directions and has failed in each: it can vanish
when the archive is most correct — writing the outcome, fix SHA and patch-id into a bug
file before moving it drops similarity below git's 50% default —
and it appears identically when the destination holds a **stale** copy of its source
(similar enough to pair — a broken move then renders the same as a good one).
**The threshold is straddled, not merely approached:** the same flow lands on either side
of 50%, so a present `R` is not evidence the archive was shallow. It answers
neither *"are both halves staged"* nor *"does the destination hold the bytes I just
wrote"*; for the second, check the destination for a token you wrote just before the
move. The `move`
response carries `stage_together` and `stage_hint` saying exactly this.

## Archive — re-point the citations

**Kept in the guide:** Re-point paths and ids in the same commit; the grep is the check.

**Pre-cut text:**

<!-- blob lines 266-295 -->

**Then re-point the citations, in the same commit as the move —
paths *and* ids.** The move changes both, and archiving is a bug file's *normal*
end state — so every citation of `docs/issues/<slug>.md`, and every citation of
that artifact's 16-hex id, is a scheduled break, and the move is the moment it
fires.

```
grep -rn 'docs/issues/<slug>.md' . \
  --include='*.md' --include='*.rs' --include='*.sh' --include='*.py' \
  --include='*.yml' --include='*.toml' --include='.env*'
```

**The `--include` list is itself a hypothesis about where citations live, and it is the
part that fails silently** — a filter that misses a file type returns a clean zero, which
reads exactly like "no citations to fix". Non-`.md`/`.rs` surfaces cite `docs/issues/`
too — `.github/workflows/ci.yml`,
`docker-compose.yml`, `scripts/build-windows.sh`, `scripts/fetch-models.sh`,
`scripts/friction-probe.py`, and a `tests/fixtures` `.toml`. If you are unsure, drop the
filters entirely and read the extension histogram; `.jsonl` and `.diff` hits are session
artefacts under `.buddy/` and `.superpowers/` and are not surfaces.

Fix the **live** surfaces — the manual, `CHANGELOG.md`, `README`s, active trackers, CI
workflows, `scripts/`, `.env*` templates, source doc comments. **Leave `docs/issues/archive/**` and
superseded session-log rounds alone**: those are historical snapshots, and
`apply_drops`' `archive_drop` exists precisely so a retired document citing a moved
path does not gate. Rewriting them would falsify the record to satisfy a linter that
is already ignoring them.

Note the gate only catches the subset that lands in a full-severity surface, so the grep
is the check — not a green CI run.

## Verifying the citation sweep

**Kept in the guide:** Verify with the full old path plus the archive-form control, never the slug.

**Pre-cut text:**

<!-- blob lines 300-315 -->

**Verifying the sweep afterward needs a different pattern than the sweep itself — grepping the
slug is the mistake to avoid.** An archive move keeps the filename and changes only the
directory, so the slug — the part everyone reaches for, because it is the memorable half — is
invariant under exactly the change being verified: `docs/issues/YYYY-MM-DD-<slug>.md` and
`docs/issues/archive/YYYY-MM-DD-<slug>.md` both contain `<slug>`, so a slug grep returns the same
hit whether a citation was fixed or not.

- **Verify with the full old path, never the slug.** `docs/issues/<name>.md` as a literal prefix
  cannot match the archive form; it is the only pattern that answers "is anything still
  dangling?"
- **Always pair it with the archive-form control.** `git grep -F 'docs/issues/archive/<name>.md'`
  must return non-zero, or a `0` from the first grep is indistinguishable from a typo'd pattern
  matching nothing.
- **Move before you write the citation, not after.** A Members line, tracker row or
  cross-reference written after the move already names the new path, so a slug-only
  verification has nothing to hide — the same zero is honest either way.

## Reading move's inbound_path_citations

**Kept in the guide:** The response separates both citation forms; `cleared` is sound and its complement is not a verdict.

**Pre-cut text:**

<!-- blob lines 320-342 -->

- **`doc(action="move")`'s `inbound_path_citations` sees BOTH forms, and tells you which is
  which.** A bare-slug citation — every `**Members:**` line in `docs/trackers/issue-clusters/`
  is one — is included, so do not grep the slug yourself to cover it.

  What the response separates, because the two forms take **opposite** remedies:

  | field | meaning |
  |---|---|
  | `inbound_path_citations` | the union, capped at 20, `null` if the scan could not run |
  | `inbound_citations_cleared` | the subset of those same entries that **cannot** cite the old path |
  | `citation_stem_preserved` | `false` ⇒ nothing is cleared, and why |

  **`cleared` is sound; its complement is not a verdict.** An entry is cleared when the file
  never mentions the old *dated* stem — and since every citation of `docs/issues/<dated>.md`
  contains that substring, such a file cannot hold one. Nothing is owed; editing it would
  convert a slug citation into a path one. An entry **not** cleared may hold a dead path
  citation *or* a bare dated stem that survives untouched, because a move changes only the
  directory. Look at those; the scan deliberately over-reports and will not guess for you.

  **`citation_stem_preserved: false` clears nothing, deliberately.** A slug citation survives
  a move only because the slug does, so a move that *renames* the stem breaks slug and path
  citations alike. The flag is why the list is empty, which is a different fact from "checked
  and found none".

## Citing an archive path — where the file is, not where it will go

**Kept in the guide:** Cite where the file is.

**Pre-cut text:**

<!-- blob lines 346-353 -->

**Cite where the file IS, never where the archive flow will put it.** The sweep above
repairs citations that *were* correct when written; it can never reach one that was wrong
on the day it was written. A citation is authored at **fix** time; the archive move is a
separate, **later** event — `fixed` is terminal, archiving is a manual
`doc(action="move")`, and a *partial* fix cancels it outright. So writing
`docs/issues/archive/<slug>.md` for a bug still sitting in `docs/issues/` schedules no
repair at all: no archive event ever fires, therefore no sweep is ever triggered and no
procedure owns the fix — it survives until a reader follows the link.

## Why a green gate proves nothing about the sweep

**Kept in the guide:** `audit_doc_refs` reports the citation at `Med` on purpose.

**Pre-cut text:**

<!-- blob lines 358-369 -->

`DEFAULT_AUDIT_GLOBS` lists both `CHANGELOG.md` and `CONTRIBUTING.md`. Refs inside
already-released `CHANGELOG.md` sections are capped by `released_history_boundary`, so a
shipped section cannot be "corrected" into falsehood.

Be precise about *why* a green run proves nothing here, because the plausible reason is
the wrong one. `audit_doc_refs` **does** scan source files, shell scripts included —
`DEFAULT_AUDIT_CODE_GLOBS` in `src/librarian/tools/audit_doc_refs/mod.rs` lists `**/*.sh`,
and `bash` has a grammar, so the citation *is* found. It is reported at **`Med`**, on
purpose: `scan_code_comments` forces that severity so someone archiving a bug file cannot
break the build via a comment they never touched. `--fail-on high` then passes, correctly.
So the finding exists and the gate is silent by design — which is exactly why the grep is
the check.

## Tracker artifacts (docs/trackers/)

**Kept in the guide:** Frontmatter shape, status vocabulary, and `draft` meaning the opposite on a plan.

**Pre-cut text:**

<!-- blob lines 393-394 -->

The catalog derives the id (`id = sha256(abs_path)`, `src/librarian/ids.rs`) on the next
`librarian(action="reindex")` if `id:` is omitted; a frontmatter `id:` is optional.

<!-- blob lines 405-419 -->

`done`, `in-progress`, etc. are NOT special-cased — they appear as
active. The frontmatter status drives librarian visibility.

**`draft` means the opposite thing on a plan, and one catalog column holds both.** The
row above defines it for **trackers**: *scoped / watching, not yet active*. In
`docs/plans/` this repo uses it to mean *shipped, deliberately unarchived because a named
residual is still open* — a plan says so in its own text (*"this plan stays active (do NOT
ARCHIVE) while 4b is open"*) and the residual lives in a companion `resume-*` tracker.
Both surfaces are catalogued with the same `status` column, so `doc(action="find")`
returns them side by side with nothing marking which vocabulary a row speaks.

The disambiguator is a `resume-*` tracker naming the plan, but the pointer runs one
way only, tracker → plan, so the plan is the row a query returns and the tracker is the row
that would correct it. **Before reading a `draft` as "not started", check whether a
`resume-*` tracker names it.**

## A `## Resume` states its own currency

**Kept in the guide:** Date the heading or open with a banner naming what superseded it.

**Pre-cut text:**

<!-- blob lines 438-465 -->

A tracker that accumulates review passes by *appending* leaves its oldest navigational
section — the one saying "start here / do this next" — at the **bottom**, below every
later correction, with nothing marking which is current. A reader told to start there
starts at the stalest text in the file.

**So a `## Resume` either dates its heading, or opens with a banner naming what
superseded it and where the current state lives.**

    ## Resume — state at compaction, <date>

    ## Resume — where this stands (<date>)

    > **CLOSED <date> — merged at `<sha>`.** Everything below this line is the
    > historical record. Read § *Integration* for the current state.

**The marker does not prevent staleness; it makes staleness legible** — and that is the
whole of it. A dated Resume whose facts have since moved is still doing its job, because
the reader can see the date and check. That is also why this is a convention rather than
a `doctor` check: no check can separate "old and still right" from "old and wrong", and
one keyed on age would fire on every correctly-dated section while catching nothing the
marker does not already cover.

**The device generalises past `Resume`.** Any section a
later one falsifies has the same problem: sections carry no dependency edges, so the falsifying
edit produces no diff hunk, no broken link and no check naming the section it falsifies — and
that section keeps reading as settled prose. Same repair, applied where you notice it: a
banner naming what superseded it. The blind party is the author adding the new section,
since nothing in the act of appending points backwards.

## Declaring an augmentation

**Kept in the guide:** Frontmatter declares the augmentation and names its sidecar; `merge=false` and `merge=true` both lose rows.

**Also stated in:**
- `docs/conventions/cross-machine-catalog-resume.md` § 7 (restore, export, `expects_augmentation`)

**Pre-cut text:**

<!-- blob lines 469-504 -->

**Before you augment: `merge=false` resets every field you omit, and under `merge=true` a `params`
array replaces the WHOLE collection** — a patch holding one row deletes the others, and the catalog
is not in git. Add and change rows with `append_entry` / `update_entry`, never by patching `params`.

If an artifact is meant to carry an augmentation, say so in **frontmatter** — and name
the committed sidecar holding its shape:

```yaml
expects_augmentation: docs/augmentations/docs-trackers-tool-usage-patterns.yaml
```

`expects_augmentation: true` is valid and means "declared, shape not
recorded". Anything else — a typo, an empty value, a bare word — is **reported** as
`augmentation_declaration_unparseable` rather than read as absent, because a
declaration that silently does not count is worse than none: its author believes they
are covered.

**What travels and what does not.** The sidecar carries `prompt`, `params_schema`,
`render_template`, `entry_collection`, `append_mode`, `history_cap` — the *shape*.
`params` deliberately stay catalog-only: they are live state that churns, and
committing them recreates params-vs-body drift. So
a restored tracker comes back **working and empty**, never holding another machine's
rows.

Write the sidecar with `librarian(action="doctor", fix="export_augmentations")`, which
also stamps the declaration. It can only export rows **this** catalog holds, so run it
on a machine that still has them.

**Export CREATES; it never refreshes.** It skips an artifact whose sidecar already
exists — that is what makes it idempotent — so it is not the way to update one after a
shape change. `doc(action="augment")` handles that itself: a call that changes `prompt`,
`params_schema`, `render_template`, `entry_collection`, `append_mode` or `history_cap`
writes through to an existing sidecar, and a params-only merge leaves the file
byte-identical. The catalog is authoritative and the sidecar is its committed
projection, so a shape change made locally is what gets published — read `git diff` on
`docs/augmentations/` before committing, as with any generated file.

## Declaring an augmentation — why the sidecar exists, and the check's remedy

**Kept in the guide:** The sidecar is the only on-disk form; a stale sidecar is worse than an absent one.

**Also stated in:**
- `docs/conventions/cross-machine-catalog-resume.md` § 7

**Pre-cut text:**

<!-- blob lines 509-537 -->

Without that wiring, export skipping, `reindex` attaching only when a row is *absent*, and
`doc(action="augment")` not touching the file would compose into a **one-way door**: after
the first export, no path
could update a sidecar, and a fresh clone's `reindex` would restore the old shape and report
success. **A stale sidecar is worse than
an absent one**: absence is reported by `augmentation_declared_but_absent`, staleness
restores clean.

**Why any of this is needed.** Augmentation is the one artifact state with no on-disk
form apart from the sidecar: rows, frontmatter and body all rebuild from disk on `reindex`,
but the catalog is machine-local and git-ignored. Its
absence is silent in every direction — `reindex` preserves augmentation keyed by id
rather than regenerating it, so it reports healthy after a loss and repairs nothing;
`doc(get)` returns `augmentation: null` without comment; the documented
`append_entry` / `update_entry` / `entry_filter` calls fail only at use, one caller at
a time.

`reindex` re-attaches a declared sidecar **whenever the row is absent, and never
overwrites a live one** — repair, not sync, because params move on independently of
the committed shape. It reports `augmentations_restored` so a run that repaired
nothing is distinguishable from one with nothing to repair.

The declaration is also what `doctor`'s `augmentation_declared_but_absent` reads. Like
`entry_prefix` it is **declared, never inferred**: guides, specs, plans and bug files
mention `[LIVE]` while *describing* the mechanism. Intent is not recoverable from prose.

The check splits by remedy. A declared sidecar that exists means run `reindex`. No
sidecar means the shape survives only in some other machine's catalog — export it
there and commit the result, or re-author with `doc(action="augment", id=…, augment={prompt: …, …})`.

## Declaring a ledger

**Kept in the guide:** A ledger declares `entry_prefix` in frontmatter; it is declared, never inferred.

**Also stated in:**
- `docs/manual/src/concepts/entry-citations.md` § *Ledgers, and entries that live in prose*

**Pre-cut text:**

<!-- blob lines 566-576 -->

**Frontmatter, because the catalog is machine-local and git-ignored.** A
declaration stored in the augmentation is absent in a fresh clone, so every
`append_entry` there fails. **The counter has to travel with the repo too** — see
*Entry ids* below for `entry_high_water_<PREFIX>`. A catalog-local mark is not safe even
though the allocator re-reads the committed body each time: the body's maximum is not
monotonic, because compaction moves entries out to an archive companion. Identity *and*
the counter travel; only the within-machine race guard stays local.

A ledger is **declared, never inferred**. A design doc that quotes `## R-4` in
prose is not a namespace, and inferring one from content would make every such doc
allocatable.

## A prefix has one owner per repository

**Kept in the guide:** The librarian refuses a second owner; use at most three letters.

**Pre-cut text:**

<!-- blob lines 581-601 -->

A bare `PREFIX-N` citation resolves to whichever ledger defines `PREFIX-N`, so two
ledgers owning one prefix bind each other's citations **silently** as wrong edges.
Longer prefixes do not prevent this, since
two authors thinking "TEST" collide at four letters too. Uniqueness does, so the librarian
enforces it:

- `doc(action="create")`, `doc(action="update")` and `rekey_prefix` **refuse** a prefix
  another artifact in the same repository already owns. The refusal names the owner and
  offers free alternatives. Nothing is written.
- **Both declarations own a prefix**: `entry_prefix` (a ledger allocating ids) and
  `external_prefix` (ids owned by a file outside markdown, e.g. a benchmark suite).
- **Archived ledgers still own theirs.** Citations of their entries still resolve to them.
- **`F` and `W` are the one shared family.** Every session log carries them, and they are
  cited qualified by file stem (`bug-fix-session-log:F-97`). No other prefix is shared.
- A hand-edited frontmatter bypasses all three write paths. `doctor`'s
  `entry_prefix_declared_twice` reports it after the fact.

Choosing one: prefer a distinctive two- or three-letter mnemonic, and **never more than
three**. The citation grammar is `[A-Z]{1,3}-\d+`, so a four-letter prefix allocates ids
nobody can cite.

## Entry ids

**Kept in the guide:** Never suffix an id; let the server allocate and write the section.

**Also stated in:**
- `docs/manual/src/concepts/entry-citations.md` § *The allocation problem*
- the `anchor_heading`, `index_row` and `id_prefix` descriptions of the `doc` tool

**Pre-cut text:**

<!-- blob lines 608-631 -->

- **Never suffix an id.** `R-72b`, `F-6a` are not valid tokens at all: digit→letter
  is not a word boundary, so a suffixed id can never be defined *or* cited. A
  collision-resolution scheme built on `a`/`b` suffixes produces ids the graph
  cannot represent. If two entries share a number, give the later one a **fresh**
  id.
- **Let the server allocate, and let it write the section too.**
  `doc(action="append_entry", id_prefix="R", anchor_heading="## Template for
  new entries", title=…, body=…, index_row="| {id} | … |")` assigns the next id atomically **and** writes
  `## R-N — <title>` — the only shape that defines a citable token — in the same
  file write. Passing `anchor_heading` + `title` + `body` together is what selects
  that path; omit any of the three and the call only reserves the id, leaving the
  section yours to write and a reserved-but-unwritten id as a live failure mode.
  This holds for an **augmented** ledger too, as long as it declares no
  `entry_collection` (its entries being body sections, not params rows).
  Hand-allocation races: a peer session in the same checkout can take the id
  between your scan and your write.
- **Put the index row in the same call, never before it.** `index_row` (`{id}` is
  filled in by the server; `index_after_line` or a declared `snapshot_anchor` says
  where it goes) lands in the same write as the section, so no commit can capture one
  without the other — a second write is the window this closes. And the allocator
  counts an id already claimed by an index row, so a row written ahead of its
  section consumes the number it names.
- If you must hand-allocate, scan **every** entry format the file uses, and re-scan
  in the same breath as the write — a max-id is a fact about an instant.

## Entry ids — refusals: main checkout only (prose ledgers)

**Kept in the guide:** Allocation is refused from a worktree session.

**Also stated in:**
- `docs/manual/src/concepts/entry-citations.md` § *From the main checkout only*

**Pre-cut text:**

<!-- blob lines 635-646 -->

- **From the MAIN checkout only.** `append_entry` refuses id allocation from a
  worktree session, on the same grounds it refuses `cites`: an entry id is
  ledger-wide state and must key to the main tracker. Left unguarded, the worktree's
  shadow row is a different `artifact_id`, so both trees issue the same number — and
  nothing repairs it at merge, because the renumber only covers params rows. If a
  worktree session must record something, write it to a worktree-local file and fold
  it into the ledger from the main checkout after the merge.
- **Unpushed ledger commits do not block allocation.** `append_entry` allocates
  optimistically. A peer at origin can still issue the same number: `pre-push`
  catches it on the merge commit that resolves a rejected push
  (`scripts/pre-push-foreign-session-guard.sh`). A ledger declaring an
  `entry_collection` takes the params path and is unguarded.

## Entry ids — where the counter lives

**Kept in the guide:** The committed high-water mark survives a fresh clone, a move and compaction.

**Also stated in:**
- `docs/manual/src/concepts/entry-citations.md` § *The counter travels with the repo* (same wording)

*No pre-cut text is kept here: the pages above hold it.*

## Entry ids — when two hosts allocate anyway

**Kept in the guide:** Diverged clones can issue the same id; `doctor` reports `entry_defined_twice`.

**Also stated in:**
- the `entry_defined_twice` finding text (`src/librarian/tools/doctor.rs`)

**Pre-cut text:**

<!-- blob lines 672-687 -->

diverged each hold a valid one and each issues the same next id; the merge then
lands two `## PREFIX-N` sections in one file, and every citation of that token
binds to whichever heading comes **first** in file order — silently, with no
dangling or ambiguous count moving, because the extractor de-duplicates and the
second definition is discarded before any consumer sees it.
`librarian(action="doctor")` reports this as `entry_defined_twice`, naming the
token and every line that defines it. It is a **detector, not a guard**: the state
it finds is already merged. It excludes a sub-heading that repeats its own entry's
token — `### A-28` nested inside `## A-28` is one entry with sub-sections, not a
collision — because a real cross-host merge lands both headings at the *same*
level, the allocator taking the level from the ledger's existing entries. Nothing
repairs it automatically, and the commonest cause is not the only one: a hand-edit
or a bad cherry-pick reach the same shape, so establish which definition is stale
before renumbering anything. Like the refusal above, it reads `## PREFIX-N`
headings only — a params ledger whose entries live as index-table rows is covered
by neither.

## Entry headings — the definition rule

**Kept in the guide:** Only `## <ID> — <title>` defines.

**Pre-cut text:**

<!-- blob lines 708-712 -->

An undefined-but-cited id becomes a **dangling** citation.

**Heading level is not the discriminator** in the table above — `### D-6 — …` and
`#### S-1 — …` both define. `### A-9 Addendum` fails on the missing dash, not the
level.

## Entry headings — a prefix with no definer, or owned outside markdown

**Kept in the guide:** No definer is a third state; an external prefix is declared with `external_prefix`.

**Pre-cut text:**

<!-- blob lines 716-744 -->

**A prefix with no definer ANYWHERE is a third state, distinct from dangling.**
Dangling means the prefix is known (something, somewhere, defines a sibling
entry or declares `entry_prefix`) but this specific id isn't. A prefix nobody
has ever defined or declared is not a citation candidate at all in the
resolver's terms, so it resolves as prose noise — the same gate that keeps
`UTF-8`/`SHA-256` silent — and is reported by neither `link_scan`'s
dangling/ambiguous buckets nor a ledger-scoped `doctor` check. `doctor`'s
`cited_prefix_with_no_definer` check exists specifically for this state: it
starts from the citation graph rather than a known ledger's claimed entries,
and fires only above a citation-volume threshold to stay quiet on incidental
prose.

**A prefix owned OUTSIDE markdown is a fourth state, and neither ledger remedy
fits it.** A benchmark suite, a JSON fixture or a code constant can own a
`PREFIX-N` namespace — `TC-N` is `scripts/tc-suites/legacy-natural.json`'s case
ids. Adding headings hands that namespace's allocation to the librarian *and*
makes the prefix known, so every cited id the authority lacks turns **dangling**.
`entry_prefix` is wrong too: it locks id allocation
to the librarian's allocator. Declare it instead, in any artifact:

```yaml
external_prefix:
  TC: scripts/tc-suites/legacy-natural.json   # path relative to the repo root
```

`cited_prefix_with_no_definer` then stays silent **only while that file exists
and holds a `TC-<n>` id** — the silence is re-checked every run, so a deleted or
rewritten suite re-arms the finding and names the stale declaration. `link_scan`
does not read the key and keeps treating the tokens as prose, which is the point.

## Citing an entry — bare, or qualified

**Kept in the guide:** Bare when one ledger owns the prefix; qualified by file stem otherwise.

**Also stated in:**
- `docs/manual/src/concepts/link-scan.md` § *What counts as a citation*

**Pre-cut text:**

<!-- blob lines 753-771 -->

Cite **qualified by file stem** when several files share a prefix. `F-N` and `W-N`
are namespaced per work stream, so each session log owns its own counter and
`F-1…F-5` are defined in every live log:

```
bug-fix-session-log:F-33      → resolves to that log's F-33
F-33                          → several definers, Ambiguous, resolves to nothing
```

The qualifier is the **file stem**, deliberately not `artifact.slug`: slugs are
lazily minted from `slugify(title)` with `-2` dedup, so they neither exist for most
artifacts nor can be predicted from the filename — and a citation an author cannot
predict is not a citation.

A qualifier naming no file in this repo is still a cross-repo reference
(`codescout:A-11`): reported, never turned into an edge, because edges cannot span
workspaces. A qualifier that *does* name a file which lacks that entry is
**dangling**, not ambiguous — the two need different fixes, so they are reported
separately.

## Citing an entry — a third shape, and mentioning without citing

**Kept in the guide:** The cross-repo file-qualified shape has no grammar; a fenced block is the only literal.

**Also stated in:**
- `docs/manual/src/concepts/link-scan.md`

**Pre-cut text:**

<!-- blob lines 776-800 -->

**A third shape — `<repo>:<file-stem>:<TOKEN>`, naming a file inside a *different*
repo — has no supported grammar at all.** `link_scan` treats any citation with 2+
qualifier segments before the token uniformly: it is retracted (never silently
collapsed to the inner `<file-stem>:<TOKEN>` form) and reported, never resolved —
same as the two-part cross-repo form above, it can never become an edge, edges
cannot span workspaces. Write it anyway when you mean it —
`claude-plugins:roster-audit-session-log:F-5` in a `## References` section is
understood by a human reader even though the resolver will not chase it — but know
it is prose only, permanently, not a gap waiting to be filled. To keep the report
legible, these land in their own `cross_repo_file_qualified` bucket rather than
the generic `malformed_qualifier` one whenever the outer segment does not match
the citing artifact's own repo name. A citation whose outer segment *is* the
citing repo's own name (`codescout:foo:F-2` cited from inside codescout itself)
is genuinely redundant, not cross-repo — that one stays in `malformed_qualifier`;
strip the prefix instead of writing it.

**Mentioning an id without citing it.** `link_scan` derives an edge from any entry
token in prose, and **inline backticks do not escape it** — inline code is scanned,
because that is how most real citations are written. A fenced code block is the only
place a token is a literal: use one when the sentence is about the identifier itself
(an id-allocation collision, say) rather than the entry it names. Left in prose, the
token becomes a `cites` edge, and `librarian(action="link_scan", write=true)` materializes it.

Why this matters: unqualified `F`/`W` citations from the **durable** ledgers are
ambiguous, so the permanent record loses the links to its own evidence.

## One entry format, never two

**Kept in the guide:** The headings are the index; a row defines nothing.

**Pre-cut text:**

<!-- blob lines 804-828 -->

Do not hand-maintain an index table *alongside* body sections. Two formats for one
entry is the defect that generates the rest: the index falls behind, and ids allocated
by scanning one format collide with the other.

**The headings are the index.** That is the shape — not one of two. It is also the only
shape in which entries are citable, because `link_scan` derives a definition from a
`## <ID> — <title>` heading and from nothing else. A table row defines no token, so a
ledger whose entries live only in rows has entries that **nothing can cite, ever**.

Do not render the index from `params` via `render_template` instead:

- **No `render_template` writes a body.** `render_params` has three callers: `context.rs`
  projects into the `librarian(action="context")` bundle, `legibility_scan` renders one
  compiled-in template keyed by file/symbol rather than by `PREFIX-N`, and a test. A
  template attached in the hope of a table writes nothing to the file.
- **Rows are uncitable**, as stated above.

Keeping a rendered row table *in addition to* headings is fine — a table is a good
reading surface. Just write it knowing it is hand-maintained and that the heading, not the
row, is what makes the entry reachable.

You do not have to take this on trust. `librarian(action="doctor")` reports
`entry_without_definition` and `ledger_defines_nothing` per artifact, and
`append_entry` / `update_entry` return `undefined_in_body` for an entry they just wrote
that no heading defines.

## Required fields / Required fields — Rests on, and detecting the lines

**Kept in the guide:** `Status`, `Valid` (three classes) and `Rests on`.

**Also stated in:**
- `docs/manual/src/concepts/statement-validity.md` (the classes, the refusals, `Rests on`, the four `doctor` checks, the stamping)
- `docs/TAXONOMY.md` § *`**Valid:**`*

*No pre-cut text is kept here: the pages above hold it.*

## Detecting these fields

**Kept in the guide:** Anchor on structure, never on a keyword.

**Pre-cut text:**

<!-- blob lines 876-879 -->

Anchor detection on **structure** — line-start, a key prefix — never on a keyword.
Prose and field share a vocabulary by construction, so `grep -c 'Status:'` also
counts sentences *about* Status, and a `/fired/` probe matches "the tell that should
have fired".

## Compaction and archival

**Kept in the guide:** Live body, then archived section with the heading kept; never reduce an entry to a row.

**Pre-cut text:**

<!-- blob lines 883-894 -->

The ladder is **live body → archived section (heading kept) → nothing further.**

- **Never reduce an entry to a bare row to "compact" it.** That destroys its
  definition and dangles every citation of it. Row-reduction is what created the
  dangling population described above.
- **Archival is safe for citations.** A unique definer resolves even when its
  artifact is `archived` — archived is not nonexistent. Where two artifacts define
  one token, the sole *active* one wins.
- **Archive into the ledger's existing archive artifact.** Check first —
  `doc(action="find", filter={"rel_path": {"contains": "archive/<ledger>"}},
  include_archived=true)`. Forking a second archive for one ledger splits the
  definitions and creates ambiguous tokens.

## Make the tracker guarded

**Kept in the guide:** Declare `entry_prefix`; do not stamp an `id:`.

**Pre-cut text:**

<!-- blob lines 899-920 -->

**Declare `entry_prefix`.** A ledger is guarded by that declaration alone — the guard
treats a declared `entry_prefix` as one of three independent reasons a file is
off-limits (`src/util/librarian_guard.rs`: augmented, stamped, or ledger), because the
`PREFIX-N` counter is state only the server may advance. Nothing else is needed, and any
YAML form works: `entry_prefix: R`, a block sequence, or `[F, W]`.

**Do NOT stamp `id: <16-hex>` into a tracker's frontmatter to guard it.**

- It guards on **catalog membership**, the axis the guard's own pinned test rejects.
  `a_catalogued_but_unaugmented_file_stays_directly_editable` argues that membership-
  guarding would refuse `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR — all catalog
  rows — and it names a prose tracker that `CLAUDE.md` documents direct heading-addressed
  editing for.
  Stamping an id converts a file by hand into the case that test exists to forbid.
- It breaks a documented workflow: a stamped `id:` in `reconnaissance-patterns.md`
  silently disables `docs/TAXONOMY.md`'s heading-addressed append path for R-N.

The guard reads the file's **own
text**, so a registered tracker whose frontmatter says nothing is invisible to it. The
remedy is the declaration, not the stamp. And **an artifact that owns no id namespace
needs no guard**: a convention doc, an ADR, a finished session log has nothing a hand-edit
can corrupt, and guarding it only makes routine edits ceremony.

## Querying with the librarian

**Kept in the guide:** The canonical find queries; filterable trackers.

**Pre-cut text:**

<!-- blob lines 932-972 -->

For bugs, swap the kind. **Constrain on all four non-terminal states, not just `open`**
— `status="open"` alone hides `taken` (a live session is on it — do not duplicate the
work), `investigating` (worked, no live owner) and `zombie` (recurring-but-unconfirmed,
kept open in case it comes back). `zombie` sits between "no longer observed" and "closed"
precisely because no other status fits — which is what makes it unreachable if the
canonical triage query omits it too:

```
doc(action="find", kind="bug",
         filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

A `zombie` hit in that query is a "has this recurred?" check, not a task to pick up —
most zombie records have no available work by design (see § Bug frontmatter and status vocabulary). Filter
it back out explicitly (`{"status": {"in": ["open", "taken", "investigating"]}}`) when you
specifically want only the actionable three.

`status="open"` alone remains right when you specifically mean *not yet started*.

Surface archived rows when needed:

```
doc(action="find", kind="tracker", include_archived=true)
```

Read a tracker's full body or one section:

```
doc(action="get", id="<id>", full=true)
doc(action="get", id="<id>", heading="## Foo")
```

**Filterable trackers** — augmented trackers that store structured rows in a params array
can be queried at entry grain via `entry_filter`. Call `doc(action="augment")` with
`augment={entry_collection: "<array-key>"}` to enable it, then pass `entry_filter={…}` (same AST as
`filter`) to `doc(action="get")`. Prose trackers need retrofit first.

For deeper artifact / augmentation / event mechanics see
`get_guide("librarian")`. For how augmented trackers carry cross-session
behavior — including the session-passover pattern — see
`get_guide("librarian-runtime")` § *Trackers as cross-session behavior*.

## Cross-linking (edges are derived — cite in prose)

**Kept in the guide:** Prose is the only write surface for citations; `link_scan` derives the edges.

**Also stated in:**
- `docs/manual/src/concepts/link-scan.md`
- `docs/manual/src/concepts/entry-citations.md` § *Relationship to `link_scan`*

**Pre-cut text:**

<!-- blob lines 979-1008 -->

- **Cite by ID in prose.** Entry IDs in their ledger's namespace
  (`A-11`, `F-3`, `BUG-40`), artifact ids (16-hex) or rel_paths across
  files, `<repo>:<ID>` across repos. Prose is the ONLY write surface for
  citations — never hand-create `cites` edges.
  **Entry IDs are stable; artifact ids are not.** A 16-hex artifact id is
  `sha256(abs_path)`, so it changes whenever the file moves — archiving one is
  the common case. Prefer an entry ID or a rel_path when the target is likely to
  be archived, and re-point 16-hex citations in the same commit as the move —
  `inbound_id_citations` in the move response names the files to fix, so this is
  a list to work through rather than a sweep to remember.
- **`link_scan` derives the edges.** `librarian(action="link_scan")` parses
  artifact bodies, resolves citations (entry tokens by their defining
  heading; archived definers lose ties to active ones; ambiguous tokens are
  reported, never guessed), and materializes/prunes scanner-owned
  `rel="cites"` edges. `write=false` (default) reports; `write=true`
  applies. Idempotent — safe to re-run any time, and the repair path after
  reindex. `doc(action="move")` grafts an artifact's links onto its
  new id itself; the scan is what heals the
  cases that bypass it — a bare `git mv`, or any other route that leaves a row
  whose id does not match its path for the abs_path pre-clean to cascade-drop.
- **Manual rels are few and deliberate:** `evidence-for`, `promoted-to`,
  `refutes` via `doc(action="link")` — use sparingly; a wrong edge
  pollutes context packing. `supersedes` is side-effectful (flips dst
  status, emits an event): archiving a tracker that has a successor
  REQUIRES a supersedes edge — created through `doc(action="link")`,
  never a bare status edit.
- **Where links pay off:** `doc(action="get", include_links=true)`,
  `doc(action="graph", depth=1-3)`, and
  `librarian(action="context", anchor_id=…)` all read the graph — a
  well-cited tracker gets neighborhood packing for free.

## Moved out: the bug-file procedure

On 2026-11-11 the guide stopped describing bug files. A bug file's directory, frontmatter,
status vocabulary, claim call, triggers, fix-provenance shape and archive timing are one
repo's pattern, and another repo keeps bugs elsewhere or not at all. They are also read by every
session that fetches the guide, including a session with no bug in front of it. The procedure
now lives in `docs/issues/_TEMPLATE.md` only. The guide keeps the mechanics every repo shares
(move through the catalog, stage both halves, re-point the citations), and the template points
at them.

What changed with it:

- **Routing.** `names_tracker_path` (`src/librarian/adapter.rs`) matches `docs/trackers/` only.
  A path under `docs/issues/` routes to `librarian`, and so does a bug file's move.
- **Doctor findings.** `terminal_status_with_caveat`, `claim_without_claimant` and the closing
  advisory name the repo's bug template (codescout: `docs/issues/_TEMPLATE.md`) instead of a
  guide heading. `closing_advisory` already states the shape it wants.
- **Fix-anchor tokens.** `bug_template_states_the_fix_anchor_grammar_doctor_enforces`
  (`src/prompts/mod.rs`) pins `## Fix provenance`, `- **SHA:**`, `- **patch-id:**` and `no_fix_commit`
  in the template.
- **The rule.** `tracker_conventions_names_no_repo_specific_artifact_kind` (`src/prompts/mod.rs`)
  fails if the guide names `docs/issues`, `_TEMPLATE`, `kind: bug`, `bug file` or `Bug files`.

The text below is the guide as it was at commit `24904379f`, from `## Bug files (docs/issues/)` up to
the first archive section. Where the template already says the same thing, the template is the
copy to edit. `docs/issues/_TEMPLATE.md` did not state the fix-provenance block, the claim release
call or the sweep pointer before this move: those were added to it.

~~~~~~markdown
## Bug files (docs/issues/)
<!-- serves: doc.create(path~docs/issues/) -->

One file per bug.

- **Path:** `docs/issues/YYYY-MM-DD-<slug>.md` while open;
  `docs/issues/archive/` once the fix is **verified on `experiments`** —
  reaching `master` is NOT required.
- **Slug:** short kebab-case noun-phrase (3–6 words), e.g.
  `edit-code-insert-mid-function`.

### Bug frontmatter and status vocabulary
<!-- serves: doc.create(path~docs/issues/), doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**Frontmatter:** every bug file has `kind: bug` plus a `status:` field.
The librarian classifier auto-recognizes the file on next reindex.

| Status | Meaning |
|---|---|
| `open` | Logged, investigation not started or paused |
| `taken` | A **live** session holds this right now. Requires `claimed_by: <sessionId>`; `doctor`'s `claim_liveness` resolves it |
| `investigating` | Worked, but **no live owner** — the residue of a claim whose session ended without concluding |
| `fixed` | Root cause addressed, regression test added, verified |
| `mitigated` | Workaround in place; root cause not addressed |
| `wontfix` | Intentionally not fixing; justification in the file |
| `zombie` | No longer observed but root cause unconfirmed. Pair with `last_observed:` and a re-open trigger |

### Claiming and releasing a bug
<!-- serves: doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**Claiming a bug.** Set `taken` with your own sessionId through the catalog — a raw
frontmatter edit does not reach it:

```
doc(action="update", id=…,
         patch={"status": "taken",
                "extra": {"claimed_by": "<sessionId>", "claimed_at": "YYYY-MM-DD"}})
```

Store the sessionId and nothing else (your scratchpad path carries it). **Release** with
the same call: `status: "investigating"` and `extra: {"claimed_by": null, "claimed_at": null}`.
`closed:` stays empty until the bug is `fixed` / `mitigated` / `wontfix`. The procedure
also lives in `docs/issues/_TEMPLATE.md`.

### The unverified caveat, made queryable
<!-- serves: doc.get(path~docs/issues/), doc.find(path~docs/issues/) -->

**`unverified:` — the caveat, made queryable.** Add it when `status` overstates what was
established. Absence means nothing outstanding, so never add it empty. A settled doubt keeps
its text under a DISCHARGE MARKER as the first word, uppercase: `CLEARED`, `REFUTED`,
`RESOLVED` or `WITHDRAWN`. Never mark an escalation: a caveat that got worse must keep
reporting. Examples: `docs/issues/_TEMPLATE.md`.

### Settled caveats — STANDING and TRACKED

`STANDING` marks a limit NO action can settle; it silences the record. `TRACKED <16-hex id>`
marks remaining WORK moved into an artifact whose status a query reads, and stays silent only
while that artifact is open. **Never write `STANDING` over work** — it deletes the only
surface still reporting it.

### Trigger rules and capture discipline
<!-- serves: doc.create(path~docs/issues/) -->

**Trigger rules — open a bug file for ANY bug noticed during work:** the moment you notice
it. The cases that do and do not qualify are listed in `docs/issues/_TEMPLATE.md`.

**Archive trigger:** move the file into `docs/issues/archive/` once the fix is verified on
`experiments` — gate green and a regression test in place. Archived experiments-only, the
file MUST carry the fix SHA labelled **`experiments`**.

### Closing a bug — patch-id and fix provenance

**Record a `patch-id` beside the SHA:** `git show <sha> | git patch-id --stable` hashes the
diff, so it survives rebase and cherry-pick. Redirect to a file, not a pipe (Iron Law 3);
the recipe is in `docs/RELEASE.md` § *Citing a fix: SHA + patch-id*.

**Write the pair as a `## Fix provenance` section.** `doctor`'s
`terminal_status_without_fix_anchor` reads the SHAPE and refuses a `fixed` or `mitigated`
record without it. A loose hash in prose is worse than none: the record then reads as
anchored.

````markdown
## Fix provenance

- **SHA:** `5a72304c` (`experiments`)
- **patch-id:** `e9f8df63b911`
````

- **Plural:** repeat the pair; each `- **patch-id:**` binds to the `- **SHA:**` above it.
  No tables — no check reads one.
- **Another repository:** `<repo>:<sha>`; the dead-pointer check skips it.
- **A fenced example is a quotation, not a declaration.**
- **No commit at all:** put `no_fix_commit: "<reason>"` in frontmatter. An empty value does
  not discharge it.

**Never cite a merge commit's patch-id** — it is empty for a clean merge and a hash of only
the resolution hunks for a conflicted one. Cite the merged branch's commits, and name the
merge as an event. `doc(action="update")` to `fixed` or `mitigated` without a recoverable
pair returns `fix_anchor_advisory`; archiving stays refused until the pair or
`no_fix_commit:` is present.
~~~~~~

The bug query and its note in `## Querying with the librarian`:

~~~~~~markdown
For bugs, constrain on all four non-terminal states, not just `open` — `status="open"` alone
hides `taken`, `investigating` and `zombie`:

```
doc(action="find", kind="bug",
         filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

A `zombie` hit is a "has this recurred?" check, not a task (see § Bug frontmatter and status
vocabulary). Add `include_archived=true` to surface archived rows. Read a tracker with:
~~~~~~

## Moved out: repo-specific references (second guard pass)

The guard test `no_guide_names_a_path_or_field_only_this_repo_has` gained three needles
(`docs/manual/`, `docs/RELEASE.md`, `resume-*`). `docs/plans/` is not a needle: the `librarian`
guide uses it as a generic example `rel_path`, and the first run of the test refused it for that. A read of the live
guide on 2026-11-11 found the text below still in `get_guide("tracker-conventions")`. It names
this repo's artifacts, so it left the guide verbatim and lives here.

- **`draft` on a plan.** Was under "Frontmatter shape", after the status table:

  > **`draft` means the opposite on a plan:** in `docs/plans/` it means *shipped, deliberately
  > unarchived because a named residual is open*. Before reading a `draft` as "not started",
  > check whether a `resume-*` tracker names it.

- **Concept pages for ledgers.** Was at the end of "Declaring a ledger":
  "Concept pages: `docs/manual/src/concepts/entry-citations.md`, `link-scan.md`,
  `statement-validity.md`."
- **Valid detail.** Was at the end of the `**Valid:**` bullet:
  "Detail: `docs/manual/src/concepts/statement-validity.md`."
- **Link-scan detail.** Was at the end of the `link_scan` bullet under "Cross-linking":
  "Detail: `docs/manual/src/concepts/link-scan.md`."
- **The guarded-tracker example.** The sentence read "which would refuse `docs/RELEASE.md` and
  every ADR". The guide now says "every plain doc and ADR". The two repo files that carry no
  namespace are `docs/RELEASE.md` and `CONTRIBUTING.md`; the guard's own comment says so
  (`src/util/librarian_guard.rs`).
- **The sidecar example.** The `expects_augmentation:` example named
  `docs/augmentations/docs-trackers-tool-usage-patterns.yaml`. That name is this repo's
  (`src/librarian/augmentation_sidecar.rs`, the injective-name test). The guide now shows
  `docs/augmentations/<slug>.yaml`.

## Size ledger

| Date | Bytes | Measured by |
|---|---|---|
| 2026-11-11, before | 55,261 | `wc -c` |
| 2026-11-11, after first pass | 39,246 | `wc -c` |
| 2026-11-11, after second pass | 29,942 | `wc -c` |
| 2026-11-11, after third pass (this file's split) | 21,928 | `wc -c` |
| 2026-11-11, after the bug-file procedure moved to `docs/issues/_TEMPLATE.md` | 16,545 | `wc -c` |

The harness threshold is bracketed: 29,942 B returned inline from a direct `get_guide` call on a
rebuilt server (2026-11-11), and 56.6 KB was saved to disk. The threshold is not one number across
tools and Claude Code versions
(`docs/issues/archive/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md`).
