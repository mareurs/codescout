---
id: '23421bbc5b226368'
kind: doc
status: draft
title: Understanding codescout — a teammate's guide to the tools and the system around them
owners:
- marius
tags:
- onboarding
- team
- guide
- trackers
- codescout-usage
topic: 'team onboarding: codescout tools, librarian knowledge system, capture habits'
---

This guide is for teammates who already have codescout wired into Claude Code but use maybe
10% of it. It explains what the system actually is, how to work with it day to day, and — the
part that is genuinely unusual — how the surrounding tracker system turns your frictions into
fixes that arrive before you need them. Twenty minutes of reading; written in plain language on
purpose.

## The two things called "codescout"

**Thing one: an MCP server that gives the model IDE-grade code intelligence.** Instead of
reading whole files and grepping, the model navigates by symbol (backed by LSP + tree-sitter),
searches by meaning (semantic index), and edits structurally. This makes sessions cheaper and
edits safer.

**Thing two: a working system built around it** — a librarian that catalogs project documents
(bug files, trackers, specs, ADRs), a memory store, guides the server injects when you need
them, and CI tests that enforce the bookkeeping. This is the part most people don't understand,
and it is where the compounding value lives. Short version: **the repo remembers what every past
session learned, and mechanisms — not good intentions — deliver those lessons to your session.**

## Five minutes of theory: the loop that works on your behalf

Here is the whole system in one story:

1. A session hits a problem — a tool misbehaves, an edit corrupts something, a green test lies.
   The rule is **capture on notice**: it gets its own file in `docs/issues/` immediately, not at
   task end.
2. Every bug file declares exactly one **defect class** (a `cluster/<slug>` tag from
   `docs/trackers/issue-clusters.md`). A CI test fails the build if the tag is missing or
   unknown — so classification cannot be skipped.
3. When a class accumulates **3+ instances across 2+ subsystems**, it gets promoted into a
   rule — in CLAUDE.md, a guide, or a dedicated tracker.
4. A rule is not considered done until it has a **mechanism**: a CI test, a hook, a gate, a
   server behavior. Prose alone is explicitly treated as "worklist item, not a rule", because
   the project measured that knowing about a defect class does not prevent committing it.
5. Some rules are additionally **tested for effect**: a sibling repo (`prompt-engineering`) runs
   A/B evals against headless Claude sessions to check whether a given piece of guidance
   actually changes model behavior. Guidance that measured as useless was deleted or never
   built.

The practical consequence for you: when a codescout tool blocks your command or an error message
redirects you, that is usually step 4 of this loop firing — a past session's bug, promoted into
a mechanism, catching the same mistake before it costs you an afternoon. **Work with the gates,
not around them.** Every refusal names its remedy in the message.

## Setting up on your machine — the part everyone gets wrong

The librarian's catalog (`~/.local/share/librarian/catalog.db`) is **machine-local and
gitignored**. A fresh clone or a big pull arrives silently missing three layers: the semantic
index, citation edges between documents, and tracker augmentations. Nothing errors — you just
quietly get less.

After cloning, or after pulling a large batch of changes someone else made:

1. Read `docs/conventions/cross-machine-catalog-resume.md` and follow it. Seriously — it exists
   because a real pull once left 21 of 23 memories invisible with zero error messages.
2. Minimum repair: `librarian(action="reindex")`, then `librarian(action="link_scan",
   write=true)`.
3. `librarian(action="doctor")` tells you what is still off; read its `missing_file` list after
   any pull that moved or archived files.

Build/verify commands, release flow, and the pre-commit gate live in `CLAUDE.md`
§ *Development Commands* — the four commands and **their order** are load-bearing; the file
explains why.

## Layer 1: code intelligence — the tools you'll use hourly

Route by what you already know:

| You know… | Use | Not |
|---|---|---|
| a symbol name | `symbols(name="Foo")`, then `symbols(name_path="Foo/bar", include_body=true)` | reading the whole file |
| a concept, not a name | `semantic_search("where retries are configured")` | guessing filenames |
| an exact string/regex | `grep(pattern, glob="*.rs")` | shell grep |
| who calls X | `references(symbol, path)` | grep — it also matches comments and strings |
| blast radius of a change | `call_graph(symbol, path, direction="callers")` | hoping |
| a structural code edit | `edit_code` (replace/insert/remove/rename via LSP) | `edit_file` on code |
| markdown | `read_markdown` / `edit_markdown` (heading-addressed) | `read_file` / `edit_file` |

Two mechanics that confuse everyone at first:

**Buffers.** Output too large to inline comes back as a handle like `@cmd_abc` or `@tool_xyz`
plus a summary. The full result is on the server — query it (`grep FAILED @cmd_abc`,
`read_file("@tool_xyz", json_path="$.items[*].id")`) instead of re-running the tool. The
response's `hint` field names the most useful follow-up call; trust it.

**Gates.** Shell commands that pipe unbounded output into a trimmer are blocked (run bare, then
query the buffer). Content-readers (`cat`, `grep`, `sed`) aimed at source files are blocked
(use `symbols`). Dangerous commands need an `@ack_*` handle. These are not permissions theater —
each gate exists because the pattern it blocks produced a specific documented failure (e.g. the
pipe gate exists partly because `cargo test | grep` once masked a non-zero exit and a truncated
buffer produced a valid-looking wrong hash). The refusal message always tells you the sanctioned
alternative.

## Layer 2: the knowledge system

**Memories** — `memory(action="list")`, then `memory(action="read", topic="architecture")`.
Read `architecture`, `conventions`, `gotchas` before deep work; they are the distilled
project knowledge, auto-listed at session start.

**Guides** — `get_guide(topic)` for deeper contracts: `tracker-conventions`, `error-handling`,
`progressive-disclosure`, `librarian`, `workspace-state`. The server also auto-injects the
relevant guide the first time a call of yours needs it.

**Artifacts** — every document under `docs/` (bug files, trackers, specs, ADRs, plans) is
indexed by the librarian. Enter through the catalog, not the filesystem:

```
doc(action="find", kind="bug", filter={"status": {"in": ["open","taken","investigating","zombie"]}})
artifact(action="find", kind="tracker")
artifact(action="get", id="<id>")                    # read
artifact(action="get", id="<id>", heading="## Foo")  # one section
```

**Ledgers** — some trackers own numbered entries (`F-N`, `W-N`, `T-N`, `IC-N`, …). The full map
of every prefix is one page: `docs/TAXONOMY.md`. Two hard rules:

- **Never hand-edit a managed ledger.** The server allocates entry ids; hand-editing races
  peer sessions and corrupts the counter. Append with
  `artifact(action="append_entry", id=…, id_prefix="F", anchor_heading=…, title=…, body=…)` —
  one call, the server writes the heading and assigns the id. The guard will refuse direct
  edits; that refusal is protecting you.
- **Archive through the catalog** — `artifact(action="move", …)`, never bare `git mv`. Identity
  is currently derived from the file path, so a hand-move orphans the document's history.

**Citations.** Writing an entry id in prose (`IC-6` for a prefix with one ledger, or qualified
with the file stem — `bug-fix-session-log:F-33` — when several files share a prefix) is how
documents link — a scanner
derives real graph edges from them. Cite ids and rel-paths, not 16-hex artifact ids (those
change when files move).

## What the system needs from you

It runs on capture. The three habits that matter, in order:

1. **Open a bug file the moment you notice a bug** — including bugs in codescout's own tools,
   bugs you won't fix, and misleading errors. Copy `docs/issues/_TEMPLATE.md`, one claim per
   file, add the `cluster/` tag through the catalog (`artifact(action="update", id=…,
   patch={tags:[…]})`). Not for typos or feature ideas.
2. **When you fix something, record the SHA and its patch-id**
   (`git show <sha> | git patch-id --stable`) in the bug file, then archive it via
   `artifact(action="move")`. The patch-id survives rebases; the SHA alone does not.
3. **When a tool annoys you, write it down** — skill frictions go to
   `docs/trackers/skill-frictions.md`, tool-usage observations to the Tool Usage Patterns
   tracker (see CLAUDE.md for the exact append calls). This feels like bureaucracy the first
   week. It is the raw material for step 3 of the loop, and the gates that save you time were
   somebody else's friction entries three weeks ago.

A useful mindset: **claims decay.** This project stamps claims with dates and derivations
(`**Valid:** dated …`, counts shipped with the command that produced them). When you write a
number into a document, include how you got it — the next reader re-runs the derivation instead
of trusting a stale cell.

## Reading error messages

codescout errors are designed to name the remedy. Examples you will actually meet:

- *"is a librarian-managed artifact — do not read or edit it directly"* → use
  `artifact(action="get", id=…)`; the error includes the id.
- *"IL3 violation — piped unbounded output"* → run the command bare, query the `@cmd_*` buffer.
- *"entry_filter set but this artifact is not augmented"* → the tracker's structured rows live
  in a machine-local augmentation your catalog doesn't have — see the cross-machine resume page,
  or read the body with `heading=`.
- *"shell access to source files is blocked"* → `symbols` / `read_file` for that file.

If an error genuinely misleads you — that's a bug. File it (habit 1). Misleading errors are
first-class citizens in the defect taxonomy here.

## Cheat sheet — where to look things up

| Question | Place |
|---|---|
| Which tracker/prefix takes my observation? | `docs/TAXONOMY.md` |
| How do bug files / trackers / statuses work? | `get_guide("tracker-conventions")` |
| Is there already an instrument for the number I need? | `docs/PROBES.md` |
| How do I add or change a tool? | `docs/PROGRESSIVE_DISCOVERABILITY.md` |
| Build/test gate, release flow | `CLAUDE.md`, `docs/RELEASE.md` |
| Known bugs before I re-discover one | `doc(action="find", kind="bug", filter={"status": {"in": ["open","taken","investigating","zombie"]}})` |
| What defect classes exist? | `docs/trackers/issue-clusters.md` (via `artifact(get)`) |
| Fresh machine / big pull | `docs/conventions/cross-machine-catalog-resume.md` |
| This guide's origin and the honest system critique | `docs/trackers/2026-09-01-fable-system-review.md` |
| The improvement backlog that critique feeds | `docs/trackers/system-retrospective-improvements.md` |

## FAQ

**Do I have to learn all the trackers to be productive?** No. Layer 1 (the code tools) makes
you productive on day one. Learn the capture habits in week one, `TAXONOMY.md` when you first
need to file something, and ignore the rest until an error message points you at it — the
system is built to teach itself to you at the moment of need.

**Why is CLAUDE.md so dense?** It is the promoted end of the loop — every paragraph is a rule
that earned its place through repeated measured failures, written with its evidence attached.
You are not expected to memorize it; the mechanisms enforce the parts that matter.

**What if I think a rule is wrong?** Good — rules here are claims with dates, not scripture.
Check the rule's cited evidence, and if reality moved, file the contradiction. There is a whole
tracker (`claim-decay`) for documented claims that stopped being true.

**Is all this bookkeeping worth it?** The honest current answer: the mechanisms demonstrably
pay (they catch real defects in CI and in-session); the prose corpus is large and its per-page
value varies; and the team is explicitly measuring which parts earn their tokens (see the
system review tracker). You are encouraged to be part of that measurement rather than take the
system on faith.
