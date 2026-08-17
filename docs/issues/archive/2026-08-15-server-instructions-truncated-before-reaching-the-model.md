---
kind: bug
status: fixed
tags:
- prompt-surfaces
- server_instructions
- get_guide
- external-report
closed: 2026-08-16
opened: 2026-08-15
owner: marius
related: []
severity: high
---

# BUG: `server_instructions` arrives at the model truncated mid-word, dropping the guide pointers

# Summary

The `server_instructions` surface is capped at 2,200 chars server-side, but the client
cuts it below that — so the surface arrives truncated mid-word, ending
`- "iron-… [truncated]`. The `iron-laws-detail` and `symbol-navigation` guide pointers
and the trailing content never reach the model. A model that never learns those topics
exist cannot call `get_guide` for them.

## Symptom (Effect)

Observed directly in this session's own system prompt. The MCP server-instructions
block ends:

```
Before deeper work in an area below, call get_guide(topic) FIRST:
- "librarian"               — artifacts, filters, trackers
- "tracker-conventions"     — frontmatter, archive, status
- "progressive-disclosure"  — output budgets, @ref buffers
- "error-handling"          — RecoverableError vs anyhow::bail
- "workspace-state"         — activate, home/foreign, reset
- "iron-… [truncated]
```

The list is cut in the middle of the `iron-laws-detail` entry.

## Reproduction

Reproduces on every session on this host at `821f9d0d` — read the MCP server
instructions block in the system prompt and observe the trailing `[truncated]` marker.
The external reporter observed the identical cut on macOS, and measured the slice at
**2,203 bytes** against a real client cut of roughly 2 KB.

## Environment

Reported on macOS against `experiments @ d7988aca` (Claude Code). Reproduced on Linux
at `821f9d0d`, Claude Code, stdio transport. Cross-platform and cross-machine — this is
not a local misconfiguration.

## Root cause

`MAX_INSTRUCTIONS_CHARS = 2200` (`src/prompts/mod.rs:1193`). The adjacent comment reads
*"The 2200 cap gives ~200 bytes …"* of headroom against the channel limit documented in
`docs/architecture/mcp-channel-caps.md`.

The cap is therefore set **at** the cliff rather than below it, and the reporter's
measurement (2,203 bytes emitted) says the real content can exceed even the nominal
2,200. Whatever headroom was intended is not being realised in practice.

The failure is invisible from inside the server: the cut happens client-side, after
emission, so no server-side test or assertion observes it.

*Measured 2026-08-15: this session's own system prompt, read directly. The 2,203-byte
figure is the external reporter's measurement on his host, not re-measured here.*

## Evidence

### What is lost

The truncated tail contains the `get_guide` topic pointers. In this session the
`iron-laws-detail` entry is cut mid-token, and any entries after it are gone entirely,
along with the Project Status block the reporter also notes as missing.

### Why the loss is self-concealing

The pointers are the mechanism by which a model discovers that deeper guidance exists.
Losing them is silent: nothing signals absence, and the model has no way to know a
topic it was never told about is callable. The reporter's phrasing is exact — *the
instruction telling the model not to trust truncated output was itself truncated before
it arrived*.

### Compounding interaction

`docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`
records that the `iron-laws-detail` guide contains a false claim. This bug means most
sessions never reach the pointer to that guide at all — so the two defects mask each
other: the guide is rarely read, and when read, misleads.

## Hypotheses tried

1. **Hypothesis:** the truncation is a display artifact of the transcript rather than
   the real prompt. **Test:** compare against the reporter's independent observation on
   a different OS and checkout. **Verdict:** rejected — identical cut point
   (`- "iron-…`) on two hosts, two platforms, two checkouts.

## Fix

Fixed on `experiments`. The filing's three steps were all taken, and step 1 — *measure the
real client cut point* — changed what steps 2 and 3 had to be.

### 1. The measurement

Taken from a live session's own system prompt, by locating its truncation marker inside the
rendered slice:

| quantity | value |
|---|---|
| `build_server_instructions(None)` | 2127 bytes / **2081 chars** |
| delivered prefix ended at | `- "symbol-navigatio` |
| that offset | byte 2092 / **char 2048** |
| `build_server_instructions(Some(&status))` — what production emits | **2350 bytes** |

**The limit is 2048 characters.** 2^11, not a coincidence.

### 2. Three defects, where the filing named one

- **The cap was above the cliff.** 2200 vs 2048.
- **The cap counted the wrong unit.** Named `MAX_INSTRUCTIONS_CHARS`, compared against
  `String::len()`, which is **bytes**. The same slice is 2127 bytes and 2081 chars — the
  surface is dense with em-dashes and arrows. So the gate was wrong twice over, in the same
  line, while staying green.
- **The test measured a string nobody receives.** `source_md_under_cap` renders
  `build_server_instructions(None)`. Every production call passes `Some(&status)`. Bare 2127
  (green against 2200); production 2350.

That third one is R-86 reached from a second direction — *name every deployment mode the
component has and ask which one the test constructed and which one production runs*. The
filing already spotted the cap-vs-client version of it; the `None`-vs-`Some` version was
underneath and unnamed.

### 3. What shipped

- `CLIENT_INSTRUCTIONS_CHAR_LIMIT = 2048` and `CHANNEL_SAFETY_MARGIN = 48`, both in
  **characters**, both carrying the measurement in their doc comments.
- `STATIC_SLICE_CHAR_BUDGET = 1900` replaces `MAX_INSTRUCTIONS_CHARS = 2200`, measured with
  `chars().count()`.
- `build_project_status_block` splits the dynamic suffix out so its length can be *measured
  before* it is appended, rather than discovered too long by a client that says nothing.
- `fit_dynamic_block` **guarantees** the total fits: the static slice is never sacrificed,
  and whatever does not fit is dropped from the dynamic tail at a **line boundary** with an
  explicit note. This inverts the old behaviour — a fixed char count, mid-token, silent.
- The memories list is capped at 8 names + `+N more`; it was the one unbounded field.
- Static slice trimmed 2081 → ~1880 chars: the `## Workspace gate` section removed (fully
  covered by `get_guide("workspace-state")`, which is in the pointer list, with a one-line
  bullet kept in the quickref), the guide pointer list de-aligned (pure padding), and Iron
  Laws 1 and 6 compressed.

### 4. What did NOT fit, and the decision taken

The arithmetic is unforgiving: 2048 − 48 margin − ~1880 static leaves ~120 chars, and
`KOTLIN_KNOWN_ISSUES`, the `## Workspace Projects` table, and user `## Custom Instructions`
each exceed that alone. **No ordering fixes this** — which is what separates it from the
tail-cut siblings this file cites.

Decision (2026-08-16, explicit): ship the guarantee here and file the channel problem
separately rather than expand this bug into a redesign. → **BL-37**,
`docs/issues/2026-08-16-project-status-blocks-do-not-fit-the-instructions-channel.md`.

Note what changed and what did not: those blocks were *already* being lost, cut client-side
mid-token with no signal. They are now dropped producer-side with a note. Same content gone,
but the agent learns it went.
## Tests added

The old `source_md_under_cap` is rewritten to count **characters** against
`STATIC_SLICE_CHAR_BUDGET`, and its failure message now names the unit explicitly — the
bytes/chars confusion is the trap most likely to be re-introduced.

**`production_render_fits_the_client_channel`** is the gate the old one was not. It builds a
deliberately hostile status (30 memory topics, three languages including Kotlin, a long
custom prompt), renders through `build_server_instructions(Some(&status))` — the call
production actually makes — and asserts three things: the total is under the channel limit;
the **final static line survives intact** (`"symbol-navigation"`, precisely what was being
cut); and the trim **announces itself**, since the silent loss was the whole defect.

**`a_status_block_that_fits_is_left_alone`** — the negative twin. A small status must not be
touched and must carry no note; a trim marker on every session would be noise that teaches
nothing.

Three pre-existing tests were **retargeted, not weakened**:
`build_with_kotlin_project_includes_kotlin_warnings`,
`build_with_workspace_appends_project_table`, and
`build_with_system_prompt_appends_custom_section` now assert on
`build_project_status_block` — the renderer, whose behaviour is unchanged and still worth
pinning — rather than on the delivered surface, which no longer carries them. Each carries a
comment saying why, and together they are BL-37's standing reproduction.

The `server_instructions.md` snapshot was regenerated (`UPDATE_PROMPT_SNAPSHOTS=1`).

Gate: **3975 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

Obsolete for the pointer list — the `get_guide` topics now arrive whole, so
`get_guide("iron-laws-detail")` no longer needs to be called blind.

Still relevant for what BL-37 covers: Kotlin known issues are also in codescout memory
`gotchas`; a project's custom instructions live in its own `system_prompt`.
## Resume

None for this bug — the limit is measured, the cap is correct in both value and unit, the
gate measures the production render, and the static slice is guaranteed to arrive.

One successor: **BL-37** for the blocks that cannot fit at any ordering.

And one thing worth keeping. This file's own § *Tests added* said *"a green suite is positive
evidence of nothing, and has been the whole time."* That was right, and the reason turned out
to be **two** independent scope errors in a single assertion — the wrong unit and the wrong
render. Both were invisible to any amount of re-reading and took one measurement each.
## References

- `docs/trackers/bistriceanu/index.md` § B-10
- `src/prompts/README.md` — the 2200-byte slice cap and the shared-branch verify hazard
- `docs/architecture/mcp-channel-caps.md` — cited by the cap's own comment
- **`docs/trackers/reconnaissance-patterns.md` § R-86** — this bug is that rule,
  independently rediscovered from outside. R-86 was written 2026-08-15 about an LSP fix
  that shipped inert because *"the end-to-end test drives `LspClient::start` with
  `mux: false` — it exercised the one transport on which the defect cannot appear."*
  Its rule: **"name every transport / deployment mode the component has and ask which
  one the test constructed and which one production runs. If they differ, the test is a
  smoke test."**

  `source_md_under_cap` (`src/prompts/mod.rs:1199`) constructs the *server-side render*
  and measures it against the *server's own constant*. Production is the client
  channel, which cuts lower. Same shape, different subsystem, found on the same day by
  an outside user who had never heard of R-86 — which is decent evidence the rule
  generalises past LSP transports to any surface with a cap on both sides of a boundary.
