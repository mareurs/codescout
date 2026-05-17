# Observations: Claude + codescout in Practice

A living document capturing real-world observations from using codescout as an MCP
server during Claude-assisted development. Intended to inform the user manual and
showcase the tool's value as open-source software.

---

## Autonomy & Plan Completion

**Observation:** When using structured planning (e.g. superpowers planning skills),
implementation plans tend to complete themselves with significantly less manual
intervention than typical AI-assisted development.

**Why it happens:** The combination of flawless codebase exploration (semantic search,
symbol navigation, LSP-backed references) and precise code editing tools (replace_symbol,
insert_code) removes the two main friction points that normally stall
agentic workflows:

1. *Finding the right place* — Claude locates the exact symbol, file, or concept without
   guess-and-check browsing
2. *Making the right edit* — Symbol-level edits are surgical; no diff noise, no
   accidental overwrites, no re-reading context to figure out line numbers

The result is that structured plans (spec → plan → implement → review) execute with
fewer human course-corrections. What normally requires multiple back-and-forth rounds
to clarify context becomes a nearly autonomous run.

**Implication for advertising:** codescout is not just a "better code search" tool —
it changes the *completion rate* of agentic tasks. The side-effect of good tooling is
that plans become self-executing.

---

## Progressive Disclosure Keeps the Context Window Useful

**Observation:** Claude's context window stays clean and navigable throughout long
sessions, even when exploring large codebases. The model doesn't get "lost" in its own
context the way it does with naive tool designs that dump full file contents.

**Why it happens:** The `exploring` / `focused` two-mode output pattern (enforced via
`OutputGuard`) means that broad queries return compact summaries — names, locations,
counts — rather than thousands of lines of code. Claude reads the map first, then zooms
in only on what it needs. A call to `list_symbols` costs ~50 tokens; the
equivalent `read_file` on a large module might cost 3000+.

The overflow hints ("showing 47 of 312 — narrow with a file path") actively guide the
next query rather than silently truncating, so the model always knows what it hasn't
seen yet.

**Compounding effect:** Because each tool call is token-efficient, Claude can make more
calls within the same context budget. More exploration steps fit in a session, which
means deeper understanding before any edit is made — and fewer hallucinated edits based
on incomplete information.

**Implication for advertising:** Progressive disclosure is a force multiplier on context
window size. Users get effectively longer, more coherent sessions without needing a
larger model or a bigger context window.

---

## The Plugin Closes the Loop

**Observation:** codescout as an MCP server is only half the story. The
`codescout-companion` plugin (available in the claude-plugins marketplace) is what
makes the tooling truly seamless — Claude uses codescout tools correctly without
any per-session prompting or reminders.

**What the plugin does:**

- **SessionStart hook** — injects the tool selection decision tree and progressive
  disclosure rules into every session automatically, so Claude always knows when to use
  `semantic_search` vs `find_symbol` vs `list_symbols`
- **SubagentStart hook** — propagates the same guidance into all spawned subagents and
  Plan agents, so parallel workstreams don't fall back to naive file reading
- **PreToolUse hook** — actively intercepts Grep/Glob/Read calls on source files and
  redirects them to the appropriate codescout equivalent before they execute

**Why the interception matters:** Without the routing hook, Claude will occasionally
default to `grep` or `cat` out of habit — especially subagents that start with a blank
slate. The hook enforces the right tool at the call site, not just in the system prompt.

**The combined effect:** MCP server + plugin = a self-reinforcing system. The server
provides the capability; the plugin ensures that capability is always routed to
correctly. The user gets the benefits without having to manage tool selection themselves.

> Reference: `codescout-companion` plugin will be published in the
> [claude-plugins marketplace](https://github.com/mareurs/claude-plugins) alongside
> the codescout MCP server release. Install with:
> `/plugin install codescout-companion@sdd-misc-plugins`

---

## When the Substrate Catches Itself

**Observation:** On 2026-05-17, codescout's tool-usage observability layer ([Pika scanner](https://github.com/mareurs/claude-plugins/tree/main/buddy/skills/codescout-pika)) detected 50 Iron Law violations in a single 559-call session. While auditing the evidence, the scanner detected its own SQL as a violation: its `LIKE '%|%'` discriminator matched literally on Pika's own `INSERT INTO pika_observations …` writes. Five observations turned out to be Pika observing Pika. The recursion was fixed in-place: the discriminator was rewritten, the five self-matches were retroactively deleted, the trackers citing them were rectified, and a substrate-level warn hook was shipped — all in the same session, all with paper trail.

**The recursion, numbered:**

1. **Observe.** A scoped Pika scan of session `753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce` recorded 50 Iron Law 3 violations (`run_command` output piped to `head`/`tail`/`grep`/`wc` instead of querying the `@cmd_*` buffer).
2. **Observe the observer.** Auditing the 50, five rows turned out to be Pika's own scan SQL — its `LIKE '%|%'` pattern matched any string containing `'`…`|`, including Pika's own discriminator queries and `INSERT` statements. The observability layer was logging itself as a violation.
3. **Correct.** The discriminator was rewritten to `INSTR(input_json, '''%|') = 0 AND INSTR(input_json, 'pika_observations') = 0` — literal substring containment, not SQL wildcard. The fix was mirrored to all three Claude Code instances (`~/.claude/`, `~/.claude-sdd/`, `~/.claude-kat/`) with md5 verification (`670836e7`).
4. **Rectify the ledger.** Five observation rows (ids 35, 36, 48, 49, 50, all `sqlite3` invocations) were retroactively deleted. The two operational trackers citing them ([`codescout-usage-frictions.md`](trackers/codescout-usage-frictions.md) U-1 and [`codescout-usage-hookify.md`](trackers/codescout-usage-hookify.md) H-1) were patched atomically: recurrence counts updated 50→45, shape tables refreshed from the post-cleanup database, a *Post-cleanup note* added explaining the delta. The audit trail survives.
5. **Promote.** The pattern was promoted from observation (U-1) to enforcement candidate (H-1), and a warn-mode `PreToolUse` hook was shipped to the [`codescout-companion`](https://github.com/mareurs/claude-plugins/tree/main/codescout-companion) plugin. The hook fires on `(cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle) … | (head|tail|grep|wc|less|sed|awk|cut|sort|uniq|tr|fmt)`, injecting an `additionalContext` line on the next turn so Claude self-corrects without blocking the call. Verified live post-`/reload-plugins` + `/mcp` reconnect.

**The numbers:**

| Metric | Value |
|---|---|
| Session tool calls scanned | 559 |
| Initial IL3 violations recorded | 50 |
| Self-matches discovered | 5 (all `sqlite3` family) |
| Real violations after cleanup | 45 |
| Shipped hook predicate catches | ~18% of observed slips (build-tool path) |
| CC instances synced with discriminator fix | 3 (md5 `670836e7`) |
| Trackers rectified atomically | 2 (`U-1`, `H-1`) |
| Time from detection to hook ship | one session |

**The substrate that did this work — sidebar.** The Pika scanner did not act alone. The story used three specialists from the [`buddy`](https://github.com/mareurs/claude-plugins/tree/main/buddy) plugin (a sibling of `codescout-companion` in the same marketplace):

- **[codescout-pika](https://github.com/mareurs/claude-plugins/tree/main/buddy/skills/codescout-pika)** — the observability scanner. Watches the meadow, whistles on Iron Law violations, escalates same-turn repeats. *"I called early — read the call before reading the rocks."*
- **[docs-lotus-frog](https://github.com/mareurs/claude-plugins/tree/main/buddy/skills/docs-lotus-frog)** — wrote this section. *"Say less. Say it once. Say it where the reader will find it."*
- **[architecture-snow-lion](https://github.com/mareurs/claude-plugins/tree/main/buddy/skills/architecture-snow-lion)** — constrained the placement decision ("two-concretes threshold: don't create a directory for one file"). *"I have seen this shape before. Let me tell you how it ends."*

Each specialist has a voice, operating principles, and a yields-to convention. They are summoned via `/buddy:summon <name>` and persist for the rest of the session. The deeper story — how each specialist is structured, how memories are routed, how the gates compose — belongs in the claude-plugins repo, not here. The point on this page is structural: the substrate that caught codescout dogfooding badly lives in a separate plugin, observes this one, and produced verifiable changes here.

**Why it matters:** The recursion claim *codescout uses codescout to grade codescout* has, until now, been provable only via the eval framework (nav-eval, edit-eval grading themselves against a live MCP server). This event is a stronger proof point: a different layer — observability, not evaluation — caught a different failure mode — observer self-reference, not eval miscalibration — and the correction loop closed inside one session with the evidence ledger updated retroactively. Three levels of recursion in one trace: the observability layer observed the codebase, the audit observed the observability layer, and the doc you are reading observes the audit.

**Implication for advertising:** Most observability tooling needs an external operator to notice when the observer is wrong. codescout's substrate is structured so that the operator role is filled by another piece of the same ecosystem: the `buddy` plugin specialists watch for the patterns that codescout's `tool_calls` table surfaces, and the patterns trip on the observability scanner just as readily as they trip on user code. A self-reference bug in the observer is not a special case — it is just another row that gets the same treatment as any other.

The shipped IL3 warn hook is narrow on purpose (build-tool prefix only, ~18% of observed slips). The remaining 82% — `git`, `find`, `ls`, `grep`, `cat`, `diff`, other — stays observational while the build-tool path collects telemetry. The decision to broaden waits on a second-session datapoint, exactly the cross-session validation the H-1 entry's `Promote-when` clause encodes. Narrow-first is the deliberate posture.

---

*Add new observations below as they emerge during development.*
