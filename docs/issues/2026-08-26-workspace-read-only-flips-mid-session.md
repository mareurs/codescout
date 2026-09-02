---
id: c752708c2757e139
kind: bug
status: mitigated
title: Workspace read_only flipped to true twice mid-session with no activate call from this session, silently blocking every write
tags:
- cluster/shared-resource-carries-no-owner
- workspace
- read-only
- concurrency
- multi-session
- tool-quirk
closed: 2026-09-02
opened: 2026-08-26
owner: marius
related:
- '4574d18db7aacec8'
- '3be6b587a9c92a7a'
severity: high
unverified: 'MITIGATED, NOT FIXED — the default_workspace_root clobber in Agent::activate is untouched and unpinned callers remain exposed. Diagnosis is closed at three levels (00948381, 76e287f8, 3ccfefb2, the last live-verified 2026-08-28 on binary 404f4622) and per-call pinning is a complete, probe-verified escape, which is what makes mitigated honest. Prevention was DECLINED 2026-09-02 rather than deferred: option 2 as written recreates the wrong-project symptom in the opposite direction, and a take_default param was rejected as a second axis on a one-axis call — see the Reclassified section for both derivations. Route A re-verified 2026-09-02 (rmcp still 1.3.0, no per-caller identity in RequestContext). NOT established: whether any occurrence exists in which pinning was unavailable as an escape — that is the re-open trigger and nothing currently watches for it.'
---

## Summary

Twice in one long session the active project (`codescout`) went `read_only`, blocking every
write path, **without this session issuing any `workspace(activate)` call between the last
successful write and the failure.** Both times the remedy was
`workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)`,
which succeeded immediately and restored writes.

The failure is loud at the call site but silent as a state change: nothing announces the
flip, so the first symptom is an unrelated-looking write refusal.

## Symptom (Effect)

```
File writes are disabled for this project. If this project was activated in
read-only mode, call workspace(action='activate', read_only: false) to enable writes.
```

The message is accurate and its hint is the correct remedy. The problem is not the message —
it is that the state changed underneath a session that never asked for it.

## Reproduction

Not reproduced on demand. Two observations from session `b02898c3` on 2026-08-26:

1. **Occurrence 1** — `create_file` to an absolute path in the session scratchpad
   (*outside* the project root). Immediately followed the completion of a background
   implementer subagent. Many `edit_markdown`, `artifact(update)` and `create_file` writes
   had succeeded earlier in the same session.
2. **Occurrence 2** — `edit_markdown` on `.superpowers/sdd/…/progress.md`, immediately after
   dispatching another implementer subagent. Between the two occurrences this session had
   completed several successful writes and two git commits.

In both cases the very next call — `workspace(activate, read_only=false)` — returned
`"read_only": false` and the retried write succeeded unchanged.

## Environment

- codescout MCP server shared across three Claude Code profiles (`~/.claude`,
  `~/.claude-sdd`, `~/.claude-kat`).
- **A peer session was live on this machine throughout** (`lang-pal-engine-3a`), and
  committed `20d5d43f` to codescout's `experiments` branch during this session — so it was
  not merely open, it was actively working in the same repo.
- This session had background subagents running against a *different* repo
  (`prompt-engineering`), pinned via the `workspace=` parameter rather than by activation.

## Hypothesis (NOT established)

The nearest prior art is `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`
(`4574d18db7aacec8`, **fixed**): *"shared codescout server has one process-global active
project — concurrent activations silently cross-contaminate reads."* That fix addressed the
project **identity** being process-global. The plausible reading here is that the
`read_only` **flag** shares the same scope, so a peer session activating codescout read-only
flips it for every session on the process.

This is a hypothesis with a motive and an opportunity, and no evidence tying an activate call
to any particular caller. It should be checked, not assumed — see `unverified:`.


## Update (2026-08-26) — cross-session hypothesis refuted, cause narrowed

The cross-session/shared-server hypothesis above is **refuted**, not just unconfirmed.
`docs/manual/src/concepts/cross-process-write-serialization.md` documents that each Claude
Code session runs its **own separate** codescout server process; cross-session coordination
is a `.codescout/write.lock` flock for write ordering only, not shared memory. A different
OS process (e.g. a `lang-pal-engine-3a`-style peer) cannot mutate this session's in-memory
`read_only` field — there is no shared `AgentInner` across sessions.

The real mechanism is **within-session**, and it is not new: `Agent::activate`
(`src/agent/mod.rs:542-567`) still unconditionally runs `inner.workspaces.clear()` and
reassigns `default_workspace_root`, for any caller sharing this session's one process —
exactly the mechanism diagnosed in `3be6b587a9c92a7a`
(`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`,
status `mitigated`). That bug's own fix was documentation-only (a stronger imperative in
`docs/architecture/companion-plugin.md` § *Concurrent multi-workspace*) and its `unverified:`
field explicitly predicted this: "a subagent that ignores the briefing can still reproduce
the original failure." `explore-inject.mjs` was checked directly (source read) and is not
the cause — it correctly injects `workspace="<root>"` pin guidance, never an `activate` call.

This file is very likely a **rediscovery** of `3be6b587a9c92a7a`'s residual risk, one day
after that bug closed. The open question is no longer "what mechanism" (answered) but
"which specific call" — same as the archived bug's own unresolved gap. See that bug's Fix
section (options 1–3) for the real remediation choices; option 1 (docs) was already tried
and just failed to hold. Options 2 (declare unpinned-concurrent unsupported, documented) and
3 (structural code guard — blocked on MCP `RequestContext` having no per-caller identity)
remain undecided and are the actual open work, not anything specific to this file.

**Confirmation, not new discovery (downgraded 2026-08-26 after checking against
`get_guide("workspace-state")`, which already states the home/foreign read-only default
split verbatim in its § *The home/foreign distinction* — home: `read_only=false`, foreign:
`read_only=true`).** What's actually new here is tying that already-documented default to
the *parent-clobbering* mechanism above, not the default itself — codescout-8f's original
framing overstated it as a fresh finding. The byte-level trace still has standalone value as
proof the implementation matches the docs: `AgentInner::build_workspace`
(`src/agent/mod.rs:156-249`):

```rust
let is_home = self.home_root.as_ref().map(|h| h.as_path() == root).unwrap_or(true);
let effective_read_only = match read_only {
    Some(false) => false,
    _ if is_home => false,
    _ => true,
};
```

Activating any root that is not `home_root`, without explicitly passing `read_only=false`,
yields `effective_read_only = true` by design (a safety default for foreign roots). Combined
with `activate` clearing the registry and reassigning `default_workspace_root`, the full
mechanism is: a subagent calling `workspace(activate, path=<foreign project>)` without
`read_only=false` makes the **parent session's default workspace a read-only foreign
project** — not just a wrong one. This matches both occurrences (subagent lifecycle-adjacent,
subagents on non-home `prompt-engineering`) and the recovery (`activate(codescout,
read_only=false)` — the one arm that beats a non-home root). It also means the diagnosis cost
is worse than the archived bug's own framing: the write refusal names a cause ("activated in
read-only mode") the parent never enacted, inviting the wrong kind of debugging. A narrower,
cheaper partial mitigation this suggests: whether `activate` should infer `read_only` at all
when invoked with a foreign root, independent of the two structural options above.

**Occurrence 4 (codescout-8f, 2026-08-26, same day) — the more dangerous sibling form.**
Trigger precisely named this time: resuming a subagent via `SendMessage` (not dispatching
one). Between the parent's last codescout-relative call and the next, nothing else
happened — the subagent (working in `prompt-engineering`) had activated its own project,
and the parent inherited it. This produced the **wrong-project** form, not the read-only
form: `symbols`/`grep`/`read_file` on `src/agent/mod.rs` returned confident, correct-for-
`prompt-engineering` negative answers ("file not found", "0 matches") for a file that is
tracked and 131 KB in `codescout`. `workspace(action="status")` was the only thing that
surfaced the actual active root; nothing else pointed at it, and the negative results were
indistinguishable from a genuine absence until that one call was made.

**Why this raises severity (medium → high):** read-only *refuses* — loud, one wasted call,
obvious remedy. Wrong-project *succeeds against the wrong tree* — silent, and can manufacture
plausible-looking wrong findings about the very codebase under investigation. This session came
within one message of filing a false high-severity bug against codescout's own `grep`/
`read_file`/`symbols` ("blind to a tracked source file") before `workspace(status)` revealed
the real cause.

**Trigger list, broadened:** not just dispatching a subagent into a foreign root — *resuming*
one (`SendMessage` to an existing subagent) reproduces the identical mechanism, since nothing
about resumption changes which process/registry the subagent's tool calls land on.

**Diagnostic gap, independent of any structural fix:** `workspace(action="status")` resolves
both symptom forms instantly, and this project's OWN `get_guide("workspace-state")` already
documents the gap verbatim, pre-dating this bug: “Caller has no way to detect this without an
extra `workspace(status)` call.” Neither the read-only refusal message (“File writes are
disabled for this project...”) nor a silent wrong-project read points at it. Cheapest possible
fix, orthogonal to options 1–3 above: have the write-refusal hint suggest `workspace(action=
"status")` explicitly (“your active project may have been changed by a subagent”). Does not
help the silent wrong-project form (nothing errors to hang the hint on), but closes the
read-only form's diagnosis gap for near-zero cost.

**IMPLEMENTED 2026-08-26.** `check_tool_access` (`src/util/path_security.rs:568`) now appends
“If you didn't expect this, a subagent may have changed the active project — call
`workspace(action='status')` to check.” to the write-refusal message. Regression test
`file_write_disabled_message_points_to_workspace_status` (same file, added via TDD — watched
RED against the old message, GREEN after the change). Full gate green: `cargo fmt --check`,
`cargo clippy --lib -- -D warnings`, `cargo test --lib` (4360 passed, 0 failed, 8 ignored).
Committed `experiments` (label: **experiments**) sha `00948381d3ef06448e03552ed001d64e5499a1ab`,
patch-id `d7d6bc55f292fb3983613c57f7812dc74d6b880b`. The two structural options (2/3) remain
undecided and unimplemented — this only closes the diagnosis gap for the read-only form, not
the underlying clobber. Not archiving this file: root cause is still open.

**IMPLEMENTED 2026-08-27 — the silent form's diagnosis gap is closed too.** The section
above concluded the wrong-project form could not get the same treatment as the read-only
form because "nothing errors to hang the hint on." That is true of `grep` and `symbols`
returning zero. It is **not** true of `read_file` / `read_markdown`: a not-found *is* an
error, and it was discarding the one fact that separates the two readings.

Both sites built their message from the caller's **relative** argument and threw away
`resolved` — the absolute path naming the tree — which was in scope at each failure site
and simply unused:

```
before:  file not found: 'src/agent/mod.rs'
         hint: Check the path with tree, or use tree with `glob` to locate the file

after:   file not found: 'src/agent/mod.rs' (searched /home/.../other-repo/src/agent/mod.rs)
         hint: ... If the root above is not the project you meant, a subagent sharing this
               session's process may have changed the active project — call
               workspace(action='status') to check.
```

The old hint was worse than silent: it routed the caller to `tree`, which runs against the
same wrong root and **confirms** the absence. Occurrence 4 is exactly this shape —
`read_file` on a 131 KB tracked file answering "not found," with `workspace(status)` the
only call that could surface why.

Two independent arms (`read_file_text`, `resolve_markdown_source`), so both are pinned:
`file_not_found_names_the_root_it_searched` and
`read_markdown_file_not_found_names_the_root_it_searched`. The second passes a RELATIVE
path deliberately — an absolute one makes the assertion vacuous, since the caller's own
argument would already carry the root — and was mutation-checked: reverting only the
message while leaving the new hint in place fails it on the root assertion specifically.
Both messages keep the `file not found:` prefix that
`src/usage/db.rs::normalize_err_family` keys on.

`grep` and `symbols` plain zeros are deliberately unchanged. They carry no error to hold
the fact, and `completeness_warning`'s contract makes a trustworthy bare zero load-bearing
— attaching a root to every empty result is the noise that stops it being read. One
surface naming the root is enough to break the illusion, and this is the surface that
already errors.

Fix `76e287f8` on `experiments`, patch-id `f328f65909ef74d80768f7657d3d6e86d1bf4268`.
Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 4612 passed /
0 failed / 47 ignored.

**Still not archiving.** This closes the second and last *diagnosis* gap; the underlying
clobber is untouched. Options 2 (declare unpinned-concurrent unsupported) and 3
(structural guard, blocked on MCP `RequestContext` having no per-caller identity) remain
undecided and are the open work.
## Hypotheses tried

- **"A subagent did it."** Plausible on timing — both occurrences sit next to subagent
  lifecycle events. But my subagents were told to pin with `workspace=`, not to activate, and
  they were operating on `prompt-engineering`, not codescout. Neither confirmed nor excluded.
- **"The scratchpad path is outside the project, so the guard is right."** Refuted by
  occurrence 2: `.superpowers/…` is inside the project root and was refused identically.

## Impact

Medium. Every write path refuses, and the refusal names a cause (`if this project was
activated in read-only mode`) that reads as speculative rather than diagnostic — so the
natural response is to doubt the path or the tool rather than the workspace state. The
recovery is one call and is stated in the hint, which is why this is not high severity. The
cost is a wasted call plus the risk that an agent mid-task interprets it as a permissions
problem and works around it, or a subagent silently gives up on a write it was asked to make.

## Fix

Not attempted. Two things worth checking before any change:

1. ~~Whether `read_only` is per-session or process-global in the current build.~~ **ANSWERED
   2026-08-26**: process-global-per-session (i.e. shared by everything on one session's
   process, not isolated per-caller). Not the unfinished half of `4574d18db7aacec8`
   (project *identity* resolution) — it's the already-known, still-unaddressed
   `default_workspace_root` clobber from `3be6b587a9c92a7a`. See the Update section above.
2. Whether the state change can be made **legible** regardless of scope — an activation that
   flips `read_only` for a session that did not request it should be visible in that
   session's next call, not discovered by a write refusal.

### Done 2026-08-28 — Fix item 2, at the point of failure

**SHA:** `3ccfefb2` (`experiments`). **patch-id:**
`5e6cd821540336f76930737b7be91d2c1f2af2a9`.

Item 2 asked whether the state change can be made legible regardless of scope. It can, and
the cheapest place is the refusal itself — no per-caller identity needed, so none of
option 3's blocker applies.

`PathSecurityConfig` now carries `write_block { root, cause }`, and the refusal states which
of the two causes it is:

> File writes are disabled: the active project is `/work/some-other-repo` and it was
> activated read-only. Call `workspace(action='activate', path='/work/some-other-repo',
> read_only: false)`… If that is not the project you expected to be in, something else
> sharing this process activated it…

**`root` is the load-bearing half.** The failure this file describes is
`default_workspace_root` being reassigned by something else in the process, so the single
most useful fact is *which project answered*. A session that believes it is in `codescout`
and reads a different path has its diagnosis in one line, with no `workspace(status)` round
trip — which is what the Impact section's "reads as speculative rather than diagnostic"
was costing.

**Precedence is a named, tested rule** (`WriteBlockCause::classify`), extracted rather than
left inline because inside `project_security_config` it needs a whole `ActiveProject` to
exercise, and getting it wrong fails silently — by shipping confident, actionable, WRONG
advice. `ConfiguredOff` outranks `ActivatedReadOnly`: telling someone whose `project.toml`
disables writes to re-activate writable is a call that succeeds and changes nothing, so
that message now says re-activating **will not** clear it.

A builder with no root in scope keeps the original hedged wording. The change's whole point
is that the refusal stops asserting causes, so a path that cannot attribute one must not
gain a confident message.

**What this does NOT do.** The clobber is untouched — `Agent::activate` still clears the
registry and reassigns the default for everything in the process, and options 2 and 3 stand
as written. This makes the consequence diagnosable in one read, not impossible. It also does
not log *which call* did it; naming the project turns out to answer the practical question
("is my default still mine?") without needing the trigger, so per-incident trigger logging
is no longer the cheapest next step — the structural remedy is.

#### The test that mattered was the one measuring whether the feature was wired at all

Seven tests, each mutation-verified with its blast radius predicted first. Four unit tests
covered the message and the precedence — and **all four passed with the derivation deleted**,
full suite green at 4598, because every one of them builds `PathSecurityConfig` by hand. The
feature would have shipped completely inert.

That is the **third** instance of this class in this repo, all found within one day:
`link_scan`'s `cross_repo_file_qualified` bucket (0 findings in every real repo under a
passing unit test), and this codebase's own `indexed_with_model` reader, which outlived its
producer by three and a half months behind a test that hand-built the response it asserted
on. `project_security_config` therefore has three tests of its own; deleting the assignment
now fails two of them.

One method note worth keeping: a mutation of mine **silently did not apply**, and its test
passed — character-identical to a test that cannot fail. Mutations are only evidence when the
edit is applied under an asserted occurrence count.

#### Live-verified 2026-08-28 06:2xZ on binary `404f4622`

Staged the real condition rather than asserting from the unit tests: activated a foreign
root read-only, then attempted a write **to a path under `codescout`**.

```
create_file("/home/marius/work/claude/codescout/docs/issues/_probe.md")
→ File writes are disabled: the active project is /tmp/…/foreign-root and it was
  activated read-only. Call workspace(action='activate', path='/tmp/…/foreign-root',
  read_only: false) to enable writes. If that is not the project you expected to be in,
  something else sharing this process activated it — a subagent, or another caller on
  this session — because activation replaces the default for everyone in the process.
```

That is the whole point, visible in one line: **the path written and the project blamed are
different**, and the message says so. The old wording — "File writes are disabled for this
project" — would have named neither, which is why four occurrences went by before anyone
suspected the activation rather than the path or the tool.

Restored with `workspace(activate, path=<codescout>, read_only=false)`; writes resumed
immediately, and this edit is the proof.

### Measured 2026-08-28 — two escape routes from the structural remedy, both closed

Before accepting "blocked", both plausible ways around it were tested. Neither works, and
knowing *why* narrows the remaining design to one shape.

**Route A — is option 3's blocker actually true?** Yes, measured rather than assumed.
`rmcp 1.3.0`'s `RequestContext<R>` carries exactly `{ ct, id, meta, extensions, peer }`
(`service.rs:654`). Of those: `id` is a per-request counter, `extensions` is server-side and
never populated here, `peer` is the single stdio connection so a subagent and its parent
share one, and `meta` is read in exactly one place in this codebase — `server.rs:1434`, for
`get_progress_token()`. A progress token is per-request, not per-caller. **So there is no
caller identity to key a per-caller default on**, and the file's blocker stands as written.

**Route B — could the `workspaces.clear()` be the harm instead?** `activate`'s own comment
calls the clear legacy (*"mirrors the previous single-slot drop-and-replace"*) and notes
`ensure_resident` adds without clearing, which suggested a no-identity fix: stop clearing,
and callers pinned with `workspace=` survive someone else's activate.

Tested live. Activated `/home/marius/work/mirela` (clearing the registry and moving the
default), then issued a **pinned** write back to codescout:

```
create_file(path="docs/issues/_pin_probe.md",
            workspace="/home/marius/work/claude/codescout")   → "ok"
```

It succeeded. **Pinning already survives the clobber** — the pinned path is re-resolved on
demand rather than read out of the registry — so the clear harms nothing and removing it
would fix nothing. That is a cheap fix eliminated by one probe rather than by argument.

**What the two results leave.** The entire harm is the `default_workspace_root`
reassignment, and only for **unpinned** callers. "Which default is mine?" is a per-caller
question by construction, so route B cannot reach it and route A has nothing to key it on.
The remaining shapes are therefore genuinely a decision, not a task: change what `activate`
MEANS (option 2 — e.g. a read-only activation stays resident but does not take the default),
or wait for a transport that carries caller identity (option 3). Both are the operator's
call, now on measured rather than presumed grounds.


### Reclassified `mitigated` 2026-09-02 — prevention is a decision that was made, not a task still open

The structural remedy was considered and **declined**, on measured grounds rather than
deferred a third time:

- **Option 2 as written — "a read-only activation stays resident but does not take the
  default" — recreates the class it fixes.** Because `read_only: true` was inert
  (`docs/issues/2026-09-02-read-only-true-is-inert-at-every-root.md`), *every* foreign
  activation without an explicit `read_only=false` **is** a read-only activation. So option 2
  means no foreign activation ever takes the default unless the caller requests **write**
  access to a tree it only wants to read — while `get_guide("workspace-state")` §
  *Cross-project workflow pattern* documents the opposite as the supported way to work in a
  sibling. Unpinned reads after `activate(sibling)` would silently resolve to **home**: this
  file's own wrong-project symptom, pointing the other way.
- **A `take_default` parameter was rejected as the remedy.** It bolts a second axis onto a
  call that already has one. `read_only` answers *"may I write here?"*; the clobber question
  is *"whose default is this?"*. Overloading either flag with the other's axis is precisely
  how the inert slot came to exist, so the fix spent its effort on making the existing flag
  honest instead.
- **Routes A and B stay closed** (§ *Measured 2026-08-28*), and route A was re-verified
  2026-09-02: `Cargo.toml` still pins `rmcp = "1.3"` and `Cargo.lock` resolves `1.3.0`, so
  `RequestContext` still carries no per-caller identity. That measurement is current, not
  inherited.

What makes `mitigated` honest rather than a shrug: the **escape is complete and measured** —
route B's probe showed a pinned call survives the clobber, and pinning needs no prior
residency — and the **diagnosis is closed at three levels** (`00948381`, `76e287f8`,
`3ccfefb2`), so the consequence is one read away rather than four occurrences away. What is
**not** fixed is the `default_workspace_root` reassignment itself, and only for unpinned
callers.

**Re-open trigger:** a transport that carries per-caller identity (route A becomes available),
or an occurrence in which per-call pinning was *not* an available escape.
## Workarounds

`workspace(action="activate", path="<project root>", read_only=false)` restores writes
immediately. Cheap, and safe to issue whenever the refusal appears.

## References

- `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md` — the fixed
  process-global active-project race; same architecture, different field.
- `prompt-surface-measurement-session-log:F-3` — *"A subagent's `workspace(activate)` mutated
  the parent's active project"*, still `open`. Same surface, and the reason "a subagent did
  it" could not be dismissed out of hand here.
