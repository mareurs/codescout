# Guard refusals resolve the artifact id instead of printing `<id>`

> **For agentic workers:** REQUIRED SUB-SKILL: `superpowers:subagent-driven-development` or
> `superpowers:executing-plans`.

**Goal:** every `guard_not_librarian_managed` refusal hands back a copy-pasteable
`doc(...)` call carrying the real artifact id, instead of the literal placeholder `<id>`.

**Architecture:** extend the already-injected `AugmentedArtifactOracle` trait with an
`id_for` method rather than calling `crate::librarian` from `crate::util`. The librarian
side supplies librarian knowledge; the feature boundary is respected by construction.

**Spec:** none — this arose from a design question during the tool-surface collapse
(2026-09-02). The rejected alternative and the reason are recorded below, because the
reason is the part that transfers.

## Why this, and not routing

The proposal that prompted it was larger: have `edit_file` **route** a librarian-managed
write to `doc(action="update")` instead of refusing it, the same way Task 8 makes it route
markdown to `markdown::edit` instead of refusing it. That was rejected, and the asymmetry
is the point.

Markdown routing is **semantics-preserving**: `read_file` and `markdown::read` are the same
operation on the same bytes, differing only in addressing. `doc(action="update")` is a
**different operation** — it takes the catalog lock, emits a `field_patch` event, syncs
frontmatter, applies a shrink guard. Routing there silently converts a file write into an
audited catalog transaction, and this guard's refusal is load-bearing safety that has
already been breached twice: `docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md`
and its read twin, found and fixed in Task 7's fix round.

And one of the three refusal reasons **cannot** be routed at all. `guard_with_oracle`
refuses for three independent reasons (`src/util/librarian_guard.rs:121-234`):

| reason | routable? |
|---|---|
| `stamped` + `FrontmatterWrite` | yes — intent is unambiguous |
| `augmented` | technically, but the body is a rendered snapshot; a body edit is wrong whichever tool performs it |
| `ledger` | **no** |

The ledger hint deliberately offers **two** destinations — `append_entry` (goes through the
id allocator) and `update` (ordinary prose edit) — because an `edit_file` call carries no
signal about which the caller means. Inferring "is this a new entry?" from whether the
content looks like a `## PREFIX-N` heading is exactly the `IC-6` trap: content that looks
like a heading versus content that is one. Guessing wrong burns an id or corrupts a counter.

So the refusal stays. What changes is that following it costs one call instead of two.

## Global constraints

- `crate::librarian` is `#[cfg(feature = "librarian")]` (`src/lib.rs:39-40`); `crate::util`
  is unconditional. **`src/util/librarian_guard.rs` must not reference `crate::librarian`.**
- `artifact_id_from_abs` (`src/librarian/ids.rs:17-23`) is a **pure function of the path** —
  `sha256(RepoPath::from(abs))`, truncated to 16 hex. No catalog lookup, so the id is free
  at the refusal site. **Do not re-implement it**; two implementations of an id derivation
  is a drift source.
- `id = sha256(abs_path)`, so in a git worktree the derived id is correct-by-construction
  but **may have no catalog row** — observed 2026-09-02, when `artifact(action="find")`
  scoped to `.worktrees/tool-collapse` returned main-checkout `abs_path`s. The hint must
  therefore present the id as derived, not assert that it resolves.

## Task 1: `id_for` on the oracle, interpolated into all three hints

**Files:**
- Modify: `src/util/librarian_guard.rs:71-74` (trait), `:121-234` (`guard_with_oracle`)
- Modify: the librarian's `AugmentedArtifactOracle` implementor (find it with
  `references(symbol="AugmentedArtifactOracle")` — do not guess the path)
- Test: `src/util/librarian_guard.rs` `mod tests` (`:372`)

**Interfaces:**
- Produces: `AugmentedArtifactOracle::id_for(&self, abs_path: &Path) -> Option<String>`,
  **with a default body returning `None`** so no existing implementor breaks — several test
  oracles in this file implement the trait inline.

### Design notes the implementer must not re-derive

1. **Three hint branches, not one.** `guard_with_oracle` builds `hint` in three arms —
   `ledger && !augmented && !stamped`, `stamped_only`, and the generic `else`. All three
   contain `id="<id>"`. Interpolate in all three; a fix that lands in one is the
   "mutate once per guarded SITE" law failing in the small.
2. **Resolve once, above the branches:**
   `let id = abs_path.zip(oracle).and_then(|(p, o)| o.id_for(p)).unwrap_or_else(|| "<id>".into());`
   Keeping the fallback string means a lean build, or a missing oracle, degrades to
   today's behaviour rather than printing an empty id.
3. **The lean lane keeps `<id>`, and that is correct** — no oracle is installed there
   (`install_augmented_oracle` is called by the librarian runtime), and `doc` is not a
   registered tool in that build, so a resolved id would name a call the caller cannot make.

### Test design — read this before writing the tests

The obvious test installs a fixture oracle whose `id_for` returns a known string and
asserts the hint contains it. **That test asserts about its own fixture**, which is the
failure `CLAUDE.md` § *Testing Discipline* names: *"a second level asserting about its own
re-implementation is indistinguishable from coverage until you break the thing that ships"*.
One such guard in this repo survived its own detector being disabled, 24 green.

Write **both** halves:

- **Fixture half** — an oracle returning `Some("deadbeefdeadbeef")` proves the guard
  *plumbs* whatever the oracle gives it, in all three arms. Also assert the negative:
  an oracle returning `None` still yields `<id>`, so the fallback is exercised.
- **Production half** — assert the *real* librarian oracle's `id_for` agrees with
  `artifact_id_from_abs` for a given path. This is the half that breaks if someone
  re-implements the hash. It is `#[cfg(feature = "librarian")]`-gated by necessity, so it
  runs in the **default lane only** — say so in a comment on the test, because a lean-lane
  green tells you nothing about it. (Measured 2026-09-02: a stale
  `TOOL_SURFACE_CHAR_BUDGET` survived precisely this way — the lean lane passed it with
  27,594 chars of headroom while the default lane, the only one that could fire, kept not
  completing.)

**Mutation, required:** replace `o.id_for(p)` with `None` and confirm the fixture half goes
RED in all three arms; then make the librarian implementor return a *different* 16-hex
string and confirm the production half goes RED. Paste both observed failures. A passing
assertion is not evidence.

### Steps

- [ ] **Step 1** — write the four tests above. Run them; expect RED with the placeholder
      still present.
- [ ] **Step 2** — add `id_for` to the trait with the `None` default body.
- [ ] **Step 3** — resolve `id` once above the branches and interpolate into all three
      hints. Run: expect the fixture half GREEN, the production half still RED.
- [ ] **Step 4** — implement `id_for` on the librarian oracle, delegating to
      `crate::librarian::ids::artifact_id_from_abs`. Run: expect all GREEN.
- [ ] **Step 5** — run both mutations above, paste the observed REDs, restore.
- [ ] **Step 6** — gate: `cargo fmt`; `cargo clippy --workspace --all-targets --features
      local-embed -- -D warnings`; `cargo test --workspace --no-default-features` ;
      `cargo test --workspace`. **Chain the two test lanes with `;`, never `&&`.**
- [ ] **Step 7** — commit. Stage exact paths by name; **never `git add -A`** on this shared
      checkout.

## Revisit-when

If a third caller ever needs an artifact id inside `crate::util`, move
`artifact_id_from_abs` into `util` and have `librarian::ids` re-export it, rather than
widening the oracle trait again. Its only non-`util` dependency is `sha2` — `RepoPath` is
already `crate::util::fs` — so the move is mechanical. Not done now under rule-of-three:
one caller does not justify relocating the catalog's id derivation.
