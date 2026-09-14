---
id: '9024da758e0870ee'
kind: bug
status: fixed
title: 'BUG: read_only blocks five tool names, not the writes it promises — doc, memory and onboarding write through it'
tags:
- cluster/guard-narrower-than-its-name
- workspace-state
- path-security
- librarian
closed: 2026-09-14
---

## Summary

`check_tool_access` (`src/util/path_security.rs:637-686`) gates file writes on a
hand-maintained list of **five tool names** — `approve_write | create_file | edit_file |
edit_code | library` — and ends `_ => {} // All other tools are always allowed`. Every other
write tool passes the gate.

`Tool::is_write(input)` already answers the same question authoritatively, per-input, at the
same dispatch point (`CodeScoutServer::is_write_call`, `src/server.rs:602-606`), and it
classifies `doc`, `memory`, `onboarding` and `index(action="build")` as writes. The two
enumerations disagree, and the narrower one is the one that guards.

So a project activated `read_only: true` still accepts `doc(action="update" / "create" /
"move" / "delete" / "link" / "graft" / "append_entry" / "update_entry" / "event_create" /
"augment")`, `memory(action="write" / "remember" / "forget" / "delete" / "refresh_anchors")`
and `onboarding`. The catalog is machine-global and `doc` resolves its target file from the
artifact id rather than from the active project, so those writes reach both the filesystem and
`catalog.db`.

## Symptom (Effect)

`read_only` is stated as a **total** property on the surface a session actually reads.
`get_guide("workspace-state")` § *Anti-patterns*:

> **Treating `read_only=true` as advisory.** It is enforced at the agent layer on any root,
> home included — tools that try to write fail with `RecoverableError`.

The refusal text itself generalises the same way — *"pass `workspace='<absolute path>'` on this
call — **every mutating tool takes it**"* — which is true of the pin and false of the block
that is explaining itself.

**The manual is the one surface that is accurate, and it reads as an incomplete list rather
than an exhaustive one:** `docs/manual/src/tools/activate-project-read-only.md` says *"All
write tools (`edit_file`, `create_file`, `edit_code`) are blocked"*. Three names, correct.
A reader takes that as an illustration.

## Reproduction

**NOT RUN, deliberately — and the reason is the finding's own subject matter.** The probe is:

1. `workspace(action="activate", path=<any>, read_only=true)`
2. `create_file(...)` → expect refusal (the control: proves the gate is armed)
3. `doc(action="create", rel_path=<an existing path>)` → the discriminator. A read-only
   refusal means the gate covers it; a *"file already exists"* validation error means the call
   reached the tool body.

Step 3 mutates nothing either way, which is what makes it a safe discriminator. **Step 1 is
the problem:** activation is process-wide, and this checkout had a live peer
(pid `2596854`, profile `~/.claude-sdd`, 20 sockets on the machine) at the time of writing.
Flipping the default read-only under a working peer is the exact hazard
`docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
records, and the refusal message this bug is about warns about it in its own text. So the claim
here is **code-derived, not probe-derived**, and it is a two-line read of a 50-line pure
function rather than an inference chain: `doc` and `memory` appear in neither guarded arm, and
the final arm is `_ => {}`.

Run the probe on a checkout with no live peer, or from a fresh server with a throwaway project
root, and record the result here.

## Environment

codescout `experiments` @ `36eec498`, worktree dirty. Read at
`src/util/path_security.rs:637-686`, `src/server.rs:602-606`, `:652-658`, `:742-789`,
`:1256-1277`, `src/librarian/adapter.rs:351-406`, `src/agent/mod.rs:436-465`.

## Root cause

**Two enumerations of "what is a write", and only one of them learned the lesson.**

`LibrarianAdapter::is_write` (`src/librarian/adapter.rs:351-406`) carries a doc comment that
states the principle explicitly, having paid for it twice:

> **Reads are the closed set; everything else is a write.** The inverse — enumerating the
> writes — is what got this wrong twice. […] An unlisted **read** is merely over-serialised […]
> an unlisted **write** races.

`check_tool_access` is precisely the inverse form — an enumeration of writes — and it never
received that correction, because the two live in different files and nothing ties them
together.

**Origin, and it was correct when written.**
`docs/superpowers/specs/2026-03-15-activate-project-read-only-design.md` § *What stays
unchanged* declares:

> `check_tool_access` in … — unchanged (already reads `file_write_enabled`)

The basename is elided from that quotation on purpose: the spec writes it bare, three files in
this tree carry that basename, and reproducing it verbatim manufactures an ambiguous citation
that `audit_doc_refs` scores `med` while no reader gains anything (CLAUDE.md § *Parsers Over a
Namespace* — a path has no syntax for *mention*, so a quotation and a claim are the same token).

**And the spec's pointer is itself now split, which is part of why the list was never
revisited.** There are two `check_tool_access` today: the method
`CodeScoutServer::check_tool_access` (`src/server.rs:652-658`), a thin wrapper that resolves the
pinned security config and delegates — and the free function
(`src/util/path_security.rs:637-686`) that actually holds the five-name `match`. The spec's
*"unchanged"* is true of the wrapper, which has been maintained (it gained pin-awareness in
2026-07 and subagent-attribution text in 2026-08). The allowlist sits in the other one, and
nothing routes a reader from the maintained half to the frozen half.

At 2026-03-15 the write surface *was* the three file tools, so a name list was a complete
selector. The population grew — the librarian tools, `memory`, `onboarding` — and the selector
did not. Nothing reds when a new write tool is added, because the guard's failure mode is
silence.

## Evidence

`project_security_config` (`src/agent/mod.rs:436-465`) correctly derives the block: `p.read_only
→ config.file_write_enabled = false` plus an attributed `WriteBlock { root, cause:
ActivatedReadOnly }`. That half is well-tested — `project_security_config_attributes_a_read_only_project_to_its_root`
even documents that 4598 tests stayed green with the wiring deleted, so it earned its own test.

The gap is entirely downstream of that, in who consults it. `check_tool_access`'s write arm:

```rust
"approve_write" | "create_file" | "edit_file" | "edit_code" | "library"
    if !config.file_write_enabled => { ... }
"semantic_search" | "index" if !config.indexing_enabled => { ... }
_ => {} // All other tools are always allowed
```

`index` appears only under `indexing_enabled`; `index(action="build")` is `is_write == true`
but is not gated by `file_write_enabled` at all.

**Every existing test of this feature asserts on a `PathSecurityConfig` constructed by hand and
a tool name from the covered five** (`read_only_refusal_names_the_project_and_the_remedy`,
`read_only_refusal_offers_the_per_call_pin_before_reactivation`,
`configured_off_refusal_rejects_the_reactivation_remedy`,
`an_unattributed_block_falls_back_to_the_original_wording`). They pin the *message*; none asks
which tools reach it. The uncovered population is untested by construction — CLAUDE.md
§ *Testing Discipline*, an assertion computed over the covered members cannot verify a claim
about the uncovered ones.


### Blast radius, measured 2026-09-14 — 23 write calls

The § *Reproduction* probe above was never run (live peer). This number comes from the other
direction, which needs no activation and so has no peer hazard: build the fix, then **mutate the
production arm back to the five-name list** and run the population guard. It enumerates every
`(tool, input)` pair that is `is_write == true` yet reaches the tool body under a write block.

**23 calls leak**, across five tools:

| tool | leaking calls |
|---|---|
| `doc` | 11 — bare `{}`, `create`, `update`, `move`, `delete`, `graft`, `link`, `append_entry`, `update_entry`, `event_create`, `augment` |
| `memory` | 5 — `write`, `delete`, `remember`, `forget`, `refresh_anchors` |
| `librarian` | 5 — bare `{}`, `reindex`, `audit_doc_refs`, `legibility_scan`, `merge_worktree` |
| `onboarding` | 1 — unconditional |
| `index` | 1 — `action="build"` |

**The unit and the tree, per CLAUDE.md § *Testing Discipline*:** the unit is a `(tool, input)`
pair — *not* a tool, which would read 5, and not a librarian action, which would read more — and
the inputs are each tool's own `action` enum, so the count moves when a tool gains an action.
Measured against worktree `36eec498` + this change, uncommitted.

**`doc {}` and `librarian {}` are in the list and are not noise.** `LibrarianAdapter::is_write`
treats reads as the closed set, so an absent `action` falls through to write — deliberately. A
malformed call therefore leaked too, not only a well-formed mutation.

**What this number is not.** It is a count of *classified-write call shapes that the gate failed
to refuse*, which is an upper bound on exposure and not a count of observed data loss. Whether
any of the 23 ever ran against a read-only activation in practice is not measured here, and the
probe that would answer it is the one the peer hazard blocks.
## Hypotheses tried

1. **A second read-only check exists elsewhere on the librarian path.** Not found. The write
   guard (`acquire_write_guard_if_writing`, `src/server.rs:742-789`) does run for `doc` — it
   takes the lock — but it is a *concurrency* guard and consults `write_lock`/`file_lock`, never
   `file_write_enabled`. Taking the lock and then writing is the observed path.
2. **`with_project_at_mut` is the write path and gates there.** Refuted — its own doc comment
   says write tools that mutate via interior mutability go through the **read**
   `with_project_at`, and it has 4 call sites. The mut/non-mut split tracks Rust borrow
   mutability, not write semantics.

## Fix

**Implemented 2026-09-14, gate green, NOT yet committed** — so there is no SHA or patch-id to
record and the record stays non-terminal. See § *Resume*.

`check_tool_access` now takes the dispatcher's own answer instead of re-deriving it:

```rust
pub fn check_tool_access(tool_name: &str, is_write: bool, config: &PathSecurityConfig) -> Result<()>
```

- **The write arm became `_ if is_write && !config.file_write_enabled`.** No tool names. A write
  tool added later is gated on the day it is added.
- **The indexing arm moved FIRST, and the order is load-bearing.** `index` is also a write under
  `action="build"`, so had the write arm claimed it, a caller with indexing disabled would be
  told their project is read-only and sent to re-activate — a call that succeeds, changes
  nothing, and leaves indexing off. Same reasoning as `WriteBlockCause` precedence: state the
  cause whose remedy works.
- **`call_tool_inner` computes `is_write` once** and threads it to both consumers — the
  pinned-residency upgrade at `src/server.rs:1269-1277` and the gate. Deriving it twice is how
  the two halves drift apart, which is this bug one level up.
- `CodeScoutServer::check_tool_access` gained the parameter and forwards it.

**Safety property checked before writing a line, because getting it wrong is a deadlock:**
`workspace` does **not** override `Tool::is_write` (only 11 files in `src/` do, and
`src/tools/config/` is not among them), so it defaults to `false` and stays reachable under a
write block. Every refusal message prescribes `workspace(action='activate', read_only: false)`
as the escape; if `workspace` ever gains an `is_write == true`, that escape becomes unreachable
from inside a read-only project and every one of those messages starts prescribing a call the
gate refuses. `read_tools_always_allowed` now carries that reasoning on the fixture line.

**Behaviour change, as predicted, and it is the point rather than a side effect.** `read_only:
true` is the default for a foreign activation, so activating a sibling repo and appending to its
trackers now refuses, naming the project and offering `workspace=<path>`. That is the ruled
semantics (`open-issue-work-queue:BL-46`, 2026-09-14).

## Fix provenance

- **SHA:** `a13b31c659c7502120535400a675ea169c10120f` (`a13b31c6`), 2026-09-14, on `experiments`.
- **patch-id:** `16e5289ce733a192d7faae30803863f65febab88`

Recorded as a **pair at fix time**, per CLAUDE.md § *Git Workflow*: the SHA is positional and
orphans when `experiments` is rebased after a ship, while the patch-id is a content hash of the
diff and survives both rebase and cherry-pick. Not a merge commit, so the value is citable — a
clean merge emits no diff and would have returned empty, and a conflicted one would have returned
a well-formed value hashing only the resolution hunks.

The commit carries **four coupled files** — the two source files, this record, and IC-14's
`**Members:**` entry. That coupling is enforced: `pre-commit` refuses a class gaining a member its
ledger does not name, and it refused this commit once for exactly that. The refusal was correct
and the omission was mine.

**Not yet archived, and the reason is a citation rather than a doubt.** `id = sha256(abs_path)`,
so `doc(action="move")` mints a new id — and `open-issue-work-queue:BL-46`'s `status_note` cites
this record by its **id**, not its path. Archiving therefore has to repoint that citation in the
same commit. IC-14 is safe either way: it cites the **slug**, which survives the move.

## Tests added

Two, deliberately at different grains — a readable statement and a population guard.

- **`a_write_tool_outside_the_legacy_name_list_is_still_refused`**
  (`src/util/path_security.rs`) — asserts `doc`, `librarian`, `memory`, `onboarding` are
  refused. **Observed RED before the fix**, panicking on `doc`, with `1 failed; 0 passed; 5530
  filtered out` — the filtered count is quoted because a filter matching nothing reports
  success here (`docs/issues/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md`),
  so a bare `ok` would not have shown the test ran.
- **`every_write_call_is_refused_under_a_write_block`** (`src/server.rs`) — walks the real
  registry, takes each tool's own `is_write` as the oracle and each tool's own `action` enum as
  the inputs. A name list here would drift exactly as the production list did and stay green
  while a new write tool went unguarded, so it deliberately has none. Carries anti-vacuity
  assertions in **both** directions: `refused >= 10` (a truncated registry cannot pass) and
  `reads_allowed > 0` (a gate that refused reads too would strand a caller inside a read-only
  project — the deadlock above).

**The population guard was mutation-tested against the PRODUCTION path, not its own inputs.**
Reverting the arm to the five-name list turned it RED and enumerated all 23 leaking calls
(§ *Evidence*). That is what the § *Blast radius* table is derived from — the number is a
by-product of demanding an observed red, not a separate count.

**Gate:** all four commands green in the documented order, `;`-chained with per-command exit
codes (`FMT_EXIT=0 CLIPPY_EXIT=0 LEAN_EXIT=0 DEFAULT_EXIT=0`). The explicit echoes are not
decoration: the gate ends in `echo`, so a chained exit status reads 0 whatever happened. Both
new tests were read out **by name** from the default lane rather than inferred from a total, and
both appear twice — they compile in the lean lane too, unlike librarian code.
## Workarounds

Do not rely on `read_only: true` to protect catalog or memory state. It protects
`create_file` / `edit_file` / `edit_code` / `library` / `approve_write` and nothing else.

## Resume

**The cost of the new refusals is counted: zero documented workflows break.** § *Fix* predicted
this would surface as new refusals in cross-repo flows. Swept the skills, plugin docs and specs
for foreign activation followed by a write, 2026-09-14 — every one that writes already passes
`read_only=false`, and every one that does not is read-only by design:

| workflow | state |
|---|---|
| `codescout-companion:tracker-hygiene`, foreign sweep | already requires `read_only=false` **and confirms the response** (`SKILL.md:417-418`) |
| companion sibling-repo work | `read_only=false` (`docs/architecture/companion-plugin.md:90,92`) |
| both generated `onboarding-prompt.md` | `read_only=false` |
| `explore-project` | `read_only: true` + *"Do NOT write or modify any files"* — the change **enforces** what its spec already instructs |

The tracker-hygiene entry is the load-bearing one: `claude-plugins:docs/trackers/prompt-hamsa-audit-log.md`
records that a foreign read-only activation already blocked a hygiene sweep's Phase 5, and the
skill was amended to activate writable. That block was hit on a **file** tool during the ledger
bootstrap, not on the librarian — consistent with only the five names being gated — and it means
the one workflow that would have been affected had already adapted for a different reason.

**Scope of that sweep, so the zero is readable:** grep for `read_only` across this repo's docs,
`../claude-plugins/`, and the plugin caches. It cannot see a workflow that activates foreign and
writes without ever writing `read_only` down — an agent improvising rather than following a
skill. That is the residual, and it is bounded by the refusal being informative: it names the
project and offers the pin.

**Still not run:** the § *Reproduction* probe. It needs a process-wide read-only activation and
the checkout had a live peer throughout (pid `2596854`, `~/.claude-sdd`), plus a peer holding the
write lock for ~2 minutes mid-session. The fix is verified by **mutation** instead — stronger for
the gate's own behaviour, weaker for the end-to-end claim: it proves the gate refuses, not that a
real `activate(read_only=true)` session reaches that gate. Run it on a peerless checkout and
record the result here.
## References

- `docs/trackers/open-issue-work-queue.md` § `BL-46` — the write-root split. The 2026-09-14
  ruling ("refuse, name the project, offer the pin") makes `last_writable_root` unnecessary and
  makes **this** the remaining work.
- `docs/trackers/bug-ledger-resume-2026-08-28.md` § *Two decisions waiting on a human*,
  Decision 2.
- `docs/superpowers/specs/2026-03-15-activate-project-read-only-design.md` § *What stays
  unchanged* — where the name list was frozen.
- `docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`
  — the same class, in `is_write` itself, fixed by inverting the enumeration. That fix is the
  one `check_tool_access` still needs.
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
  — why the probe was not run.
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`
