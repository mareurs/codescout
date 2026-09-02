---
id: 421564ab7890f8f8
kind: bug
status: fixed
title: 'BUG: `read_only: true` is inert at every root — home swallows it, foreign already implies it, and the one caller needing the state bypasses the API'
tags:
- cluster/accepted-parameter-silently-dropped
- workspace
- read-only
- api-contract
- tool-quirk
closed: 2026-09-02
opened: 2026-09-02
owner: marius
severity: medium
unverified: 'FIX LANDED at 1559daa5 (experiments), patch-id 3e86d303136e5192d1761b91058b65bdeb3612df — SHA + patch-id recorded, so nothing is owed later. NOT ARCHIVED YET, deliberately: the move re-points paths AND the 16-hex id, and this file is cited from four surfaces committed minutes ago (src/agent/mod.rs, src/server.rs, docs/superpowers/specs/2026-06-01-peer-delegation-protocol-design.md, and by slug in docs/trackers/issue-clusters.md). src/server.rs is peer-contended, so the citation sweep is a separate bounded task, not a drive-by. GATE CAVEAT: fmt 0, clippy 0, lean 0; `cargo test --workspace` returned 101 with ONE red — run_exits_after_idle_timeout_with_no_connections (ee9d8d80ad5ecdc8, known load-sensitive), 7/7 green in isolation and excluded from this diff by mechanism (every changed line in src/peer/server.rs is a /// comment). The gate also ran on a shared checkout carrying several peers'' uncommitted work, with HEAD moving ~10 commits during the session, so a clean whole-lane green was never observable here. RESOLVED by this commit: the TOOL_SURFACE_CHAR_BUDGET coupling described in the capture ledger — src/tools/config/mod.rs landed, so HEAD''s 56_497 is again the exact measured total rather than carrying 31 chars of slack.'
---

## Summary

`workspace(action="activate", read_only=true)` never changes any outcome, at any root.
`build_workspace`'s match arms make `Some(true)` indistinguishable from omitting the
parameter everywhere: on the **home** root both yield a **writable** workspace, and on a
**foreign** root both yield read-only. The only value of `read_only` that affects anything
is `false`. The one in-tree caller that genuinely needs "home, but read-only" — peer-serve —
reaches past the API and mutates the field directly.

## Symptom (Effect)

Asking for a read-only home workspace returns a writable one. Live probe, 2026-09-02
~07:12Z, against the running MCP server binary:

```
workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=true)
→ { "status": "ok", "project": "codescout", "read_only": false, ... }
```

The response *does* echo `read_only: false`, so the drop is visible to a caller who reads
the echoed field. Nothing warns, and no schema or guide text says `true` may be ignored.

## Reproduction

Deterministic — one call, no timing or concurrency involved.

1. Any codescout MCP session whose home project is `P`.
2. `workspace(action="activate", path="<P>", read_only=true)`.
3. Read `read_only` in the response → `false`.

Source at branch `experiments`, HEAD `8fb5f6385dc4a0043525b0841b865a109b1385d2`. The probe
ran against the live MCP server binary (last `cargo rb`), not a build of that SHA.

## Environment

- Branch `experiments`, HEAD `8fb5f638` at time of filing.
- codescout MCP over stdio, Claude Code profile `~/.claude-kat`.
- Not environment-sensitive: the decision is a pure `match` over two inputs.

## Root cause

`AgentInner::build_workspace` (`src/agent/mod.rs:187-191`) — read at HEAD `8fb5f638`, and
confirmed live by the probe above:

```rust
let effective_read_only = match read_only {
    Some(false) => false,
    _ if is_home => false,   // swallows Some(true)
    _ => true,
};
```

Enumerating the whole domain shows the parameter's `true` value is inert, not merely
mishandled:

| `read_only` | root | `effective_read_only` |
|---|---|---|
| `Some(true)` | home | **`false`** ← asked for read-only, got writable |
| `None` | home | `false` |
| `Some(true)` | foreign | `true` |
| `None` | foreign | `true` |

`Some(true)` and `None` agree on every row. So `read_only: true` is indistinguishable from
omitting the parameter at every root — the guard it advertises can never be requested.

**Two live decision sites, not one.** The same expression is duplicated in
`Agent::activate_within_workspace` (`src/agent/mod.rs:1011-1014`). A third copy at
`:974-977` computes the value and explicitly discards it
(`let _ = effective_read_only_snapshot; // recomputed under write lock below`) — a fix that
changes only one of the three leaves the others disagreeing, and per this repo's
mutate-per-site law a single kill would say nothing about the rest.

**The behavior is relied upon elsewhere, which is why this is a contract question rather
than a one-arm fix.** `src/peer/server.rs:54` names it as intent:

> `Agent::new` makes `root` the home, and home is read-write by default
> (`build_workspace`'s `is_home => rw` invariant); peer-serve is a pure reader, so we
> override that here.

And `build_server_for_with_env` (`src/peer/server.rs:64-88`) does exactly that — it cannot
express "home, read-only" through `activate`, so it bypasses it:

```rust
if read_only {
    agent.with_project_at_mut(None, |p| { p.read_only = true; Ok(()) }).await?
}
```

That is the tell: the sole in-tree consumer of the state the parameter names routes around
the parameter.

## Evidence

### The parameter's `true` value has no in-tree caller

`grep(pattern="activate\\(.*Some\\(true\\)|read_only.*Some\\(true\\)", glob="**/*.rs")` at
HEAD `8fb5f638` → **0 matches** (hidden dirs not searched). Every one of the 26
`Agent::activate` call sites passes `None` or `Some(false)`. So no test pins the home arm in
either direction, and the `Some(true)` path is exercised only from the MCP surface — where,
per the table above, it cannot be distinguished from `None`.

### The project's own guide warns against the no-op this is

`get_guide("workspace-state")` § *Anti-patterns*:

> **Treating `read_only=true` as a no-op.** It blocks mutations at the agent layer; tools
> that try to write will fail with `RecoverableError`. Use it deliberately for scout-only
> work.

The advice is correct for a foreign root and false for the home root, and the guide draws no
distinction — while § *The home/foreign distinction* on the same page states the defaults
correctly. The guide is right about defaults and wrong about whether the parameter can
override them.

## Hypotheses tried

1. **Hypothesis:** the drop is a rendering artifact of the response, not the stored state.
   **Test:** read `build_workspace`'s body at HEAD and enumerate the match domain;
   `read_only: effective_read_only` is the field actually stored on `ActiveProject`
   (`src/agent/mod.rs:221`). **Verdict:** rejected — the state itself is writable.
2. **Hypothesis:** a test pins home-plus-read-only as intended behavior.
   **Test:** grep for `Some(true)` at any `activate` call site → 0 matches; the four
   `is_home_*` tests (`src/agent/mod.rs:2775-2812`) assert `is_home()` truthiness only.
   **Verdict:** rejected — nothing pins it either way.


3. **Hypothesis:** the `is_home` arm is deliberate policy, so the flag should be *refused*
   rather than honoured (direction 2 below). **Test:** read the original design artifacts, and
   compare the sibling code path. **Verdict:** rejected, on two independent grounds. The
   2026-03-15 plan (`docs/superpowers/plans/2026-03-15-activate-project-read-only.md:205-209`)
   does write the arm with the comment `// home project always writable` — but its own tool
   schema describes `read_only` as *"default: true for non-home projects, false for home"*, a
   **default**, and its commit message discusses only `read_only: false`. An explicit `true`
   was never considered, so there was no policy to override. Independently,
   `activate_within_workspace`'s already-activated branch applies `read_only` directly and
   therefore *already* honoured `Some(true)` — one function, two branches, disagreeing. Nobody
   intended an invariant.
## Impact

Medium. A caller asking for the read-only guard on its own home project silently does not
get it, and the guard's whole purpose is to be a backstop against writes the caller did not
intend — so its failure mode is the one it exists to prevent. Three things hold severity
below high: the response echoes `read_only: false`, so the drop is detectable without extra
calls; a foreign root (the common scouting case) already defaults to read-only, so the
advertised protection is present where it is most used; and no in-tree caller passes `true`.

The lasting cost is that `(home, read-only)` is **unexpressible** through the public API,
which is why peer-serve had to reach past it — a state the codebase needs and the interface
cannot say.

## Fix

**IMPLEMENTED 2026-09-02 — direction 1 (honour the flag).**

**SHA:** `1559daa533140214f007ce61fcf1b49505e20559` (label: **`experiments`**).
**patch-id:** `3e86d303136e5192d1761b91058b65bdeb3612df`.

The rule is now one function, `AgentInner::resolve_read_only` (`src/agent/mod.rs`), expressed
as `read_only.unwrap_or(!is_home)` rather than a `match`. That form was chosen so the defect
is **unrepresentable**: there is no guard arm above the explicit case for it to shadow. All
three former copies call it — `build_workspace`, `activate_within_workspace`, and the
discarded snapshot — so the divergent-copy hazard goes with them.

Both defaults are unchanged (`None` at home → rw, `None` elsewhere → ro). The only behavior
that moves is `Some(true)`, which nothing in-tree passed (0 grep matches) and which no caller
in `claude-plugins/codescout-companion` passes against a home root either — its explore
injector uses `read_only=true` on a *foreign* root, where the default was already `true`.
Blast radius across both repos: nil.

**Two things this fix deliberately does NOT do.**

- **`peer/server.rs` keeps `with_project_at_mut`.** The plan was to drop it as a bypass now
  that the flag works. Reverted after checking: `activate` also sets
  `project_chosen_this_session`, which gates `guard_worktree_write` and `worktree_read_notice`
  — session intent peer-serve must not assert, having been *given* its root rather than
  choosing it. The direct mutation is the narrow, correct call; only its doc comment changed,
  to say so instead of blaming the (now fixed) inert flag.
- **`ActivateProject`'s schema text is unchanged, and needed no change** — it already stated
  both defaults correctly. Only `Workspace`'s was wrong (`"(default: false)"`), and correcting
  it cost 31 chars against **3** of surface headroom (measured baseline 56_516), so
  `TOOL_SURFACE_CHAR_BUDGET` was raised 56_519 → 56_547 and recorded as debt at the constant,
  following the 2026-08-28 precedent there.

Not attempted. This is a contract decision, not an arm to flip, and the two directions differ
in what they promise:

1. **Honour it.** `Some(true) => true` regardless of `is_home`. Closes the hole and lets
   peer-serve drop its `with_project_at_mut` bypass. Must land at all three sites
   (`:187`, `:1011`, and the discarded snapshot at `:974`) or they disagree. Risk: anything
   relying on the `is_home => rw` invariant named at `src/peer/server.rs:54` — which must be
   re-read as a claim about `Agent::new`'s default, not about `activate`'s override, before
   this is safe.
2. **Refuse it.** If home-is-always-writable is genuinely invariant, `activate` should
   **reject** `read_only=true` on the home root with a `RecoverableError` naming the
   limitation, rather than accept and ignore it. Per this repo's *Parsers Over a Namespace*
   rule: where no escape is affordable, say so at the refusal site — a documented limitation
   and a silent reinterpretation cost a reader very different amounts.

**The guide has already been corrected to describe today's behavior, so either direction must
UPDATE it rather than add to it.** Three passages in
`src/prompts/guides/workspace-state.md` now assert that `read_only=true` is inert, and
direction 1 makes all three false:

- § *The home/foreign distinction* — the "`read_only` only ever lowers the guard" paragraph,
  which cites this file by path.
- § *Anti-patterns* — the "Expecting `read_only=true` to protect the home project" bullet.
- § *Cross-project workflow pattern* — the note that the flag changes no behavior on a
  foreign root.

The tool schema for `read_only` (`src/tools/config/mod.rs`) also needs to state which
direction was chosen; it currently documents neither. Corrected 2026-09-02 in the session
that filed this file, so the guide and the code agree **today** and will disagree the moment
the bug is fixed: `grep -rn 'read-only-true-is-inert' src/prompts/guides/` before closing.

## Tests added

Three in `src/agent/mod.rs`, each with its mutation reasoned before it was written:

- `activate_home_with_read_only_true_is_honoured` — the live defect. Watched **RED** against
  the old arms, **GREEN** after.
- `resolve_read_only_covers_its_whole_domain` — all six rows at the single site. The
  discriminating row is `Some(true) + home`; a foreign-root-only test is monotone under this
  defect, because deleting the explicit case leaves both foreign rows correct.
- `activate_within_workspace_honours_read_only_true_on_the_root_project` — GREEN both before
  and after, deliberately. It pins the agreement between `activate_within_workspace`'s two
  branches, whose *disagreement* is the evidence that settled intent (see Hypotheses tried 3).

`activate_home_always_writable` was renamed to `activate_home_defaults_to_writable`: it only
ever exercised `None`, and "always" stopped being true with this change.
## Workarounds

None at the API level — the state cannot be requested. In-process Rust callers can do what
peer-serve does (`with_project_at_mut(None, |p| p.read_only = true)`), which is not reachable
from an MCP client.

For the practical goal of "do not write to this project", per-call pinning
(`workspace=<path>`) plus not issuing writes is the available discipline, and
`get_guide("workspace-state")` § *Per-call workspace pinning* already recommends pinning over
activation for scoped work.

## Resume

Decide direction 1 (honour) vs direction 2 (refuse) — it is a contract call, not a
measurement. Before implementing direction 1, re-read `src/peer/server.rs:44-88` and confirm
the `is_home => rw` comment describes `Agent::new`'s default rather than a guarantee
`activate` must preserve; if it is the latter, direction 2 is the only safe one. Then change
all three sites (`src/agent/mod.rs:187`, `:1011`, `:974`) in one commit and add the home-arm
test described under *Tests added*.

## References

- `docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md`
  (`c752708c2757e139`, `investigating`) — same two functions, the adjacent defect: `activate`
  reassigns `default_workspace_root` process-wide. Found while verifying that file's code
  claims.
- `src/peer/server.rs:44-88` — the caller that needs `(home, read-only)` and bypasses the API.
- `get_guide("workspace-state")` § *The home/foreign distinction*, § *Anti-patterns*.
