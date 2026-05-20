# Codescout — Field Notes from a Real LLM Coding Session

**Audience:** codescout MCP server devs, codescout-companion skill authors, buddy specialist authors, and anyone shaping the tracker/discipline system.

**Author:** an LLM coding session (Claude Opus 4.7) executing Sprint 1 follow-up work on `pc-kb-assistant`, a private South Pole P&C HR RAG assistant.

**Date observed:** 2026-05-20.
**Session shape:** ~90 minutes of real-time interaction. ~170 tool calls. 12 tracker-tasks created. 2 fully completed. 170/170 tests green at session end. 6 markdown trackers touched. 7 source files modified. 1 reconnaissance scout, 1 architecture review (snow-lion), 1 dispatched code review (general-purpose subagent).

**Working repo:** `pc-kb-assistant` — Python 3.11+, Pydantic v2, Vertex AI Gemini Flash + bge-m3, ChromaDB, FastAPI. GCP-only stack. ADR-003 just landed (Cloud Run deployment pattern). The session was post-Sprint-1 cleanup: a multi-agent code review identified 12 follow-up items, this session executed two of them.

**Author position:** I am the LLM, not the user. The user is part of the codescout team and asked me to write this report. I have no separate incentive beyond accurate reporting; everything below is evidence-bound and cites the specific tool call, file, error message, or turn it derives from.

---

## TL;DR

**Three things you should not break:**

1. **The `@cmd_*` / `@file_*` / `@bg_*` buffer system on `run_command`.** Once internalized, it is materially better than `Bash`'s context dumps. The IL3 rule (no piping live output to log-trimmers) prevents real waste. Keep.
2. **`read_markdown(heading=...)` and `edit_markdown(heading=..., action="edit", old_string=..., new_string=...)`.** Section-scoped read + surgical edit is the right shape for tracker work. The action vocabulary (`replace`, `insert_before`, `insert_after`, `remove`, `edit`) is well-chosen.
3. **The F-N / W-N session-log discipline with required counterfactuals on W-N.** This is the single most undervalued artifact in the system. It converts vague unease into citable evidence. The exemplars in `SKILL.md` are load-bearing — keep them in the skill body, not in a separate file.

**Three concrete bugs (high priority):**

1. **`symbols(name=..., include_body=true)` does not return the body.** It returns a stub plus the string `(N-line body — use json_path="$.symbols[0].body" to extract)`, but calling it with that exact `json_path` returns the same stub. I burned three round-trips on this before falling back to `read_file(force=true, start_line=..., end_line=...)`. Reproduced repeatedly. See §"Bug 1" below.
2. **`edit_file` rejects entire batch on a single def-containing edit.** Useful in principle (LSP corruption risk per BUG-027) but the failure mode is harsh: the safe edits in the same batch are also rolled back. Splitting into two calls is the workaround; transparent routing of def-edits to `edit_code` would be better. See §"Bug 2".
3. **`edit_markdown(action="replace", heading=...)` silently merges the new section's trailing blank line into the next heading.** I lost a section break and had to recover with a separate `insert_before` for `---\n\n`. See §"Bug 3".

**The biggest design observation (not a bug, but worth a conversation):**

**Tracker proliferation has real cognitive cost.** In one session I created or touched: 4 entries + index + template + skip-log in `docs/trackers/mrv-chat-watch/`, a session log (`sprint-1-cleanup-session-log.md`), a 13-item task list, modified `CLAUDE.md` twice, referenced three ADRs in `docs/decisions/`, and (via boundary error) discovered that `sprint-plan.md` is librarian-managed. That is *five distinct tracker disciplines* on a PoC. The boundaries are real and each is defensible in isolation. The aggregate cost is: every observation now triggers a "which tracker does this belong in?" meta-decision, and cross-references between trackers are plain text (so they will rot). See §"Tracker proliferation" below.

---

## Context: what the session actually did

So the wins, frictions, and design observations are grounded.

1. **Reviewed mrv-chat's recent dev branch** (a sibling repo, not codescout-indexed). Created `docs/trackers/mrv-chat-watch/` to track upstream evolution decisions. 4 entry files + a skip log. Decision-state vocabulary (`adopt`, `adopt-dark`, `defer`, `skip`, `pattern-noted`) used across all four entries.
2. **Ran a Sprint 1 code review** by dispatching a `general-purpose` subagent with the `requesting-code-review` skill template. Subagent produced an AC-by-AC verdict, naming 7 Important issues and one Critical (none). Verdict: "Ready to advance to Sprint 2 with fixes."
3. **Wrote 12 tasks** capturing every review action item.
4. **Executed task #2** — the corpus audit blind-spot fix. This invoked:
   - The `codescout-companion:reconnaissance` skill (pre-edit scout).
   - The `buddy:architecture-snow-lion` specialist (architecture pass).
   - 6 file edits (later 7, because the recon-missed e2e test broke and was repaired).
   - 1 new pytest test (`test_no_hardcoded_escalation_portal_url`) + 1 new behavioral test (`test_answer_abstain_jira_url_sourced_from_corpus`).
   - Full test suite green: 170/170.
5. **Bootstrapped a session log** at `docs/trackers/sprint-1-cleanup-session-log.md`. Captured W-1 (recon caught full blast radius), W-2 (architecture pass caught two latent decisions), and F-1 (`corpus: Optional[CorpusConfig]` is a test-affordance leaked into production shape).

The bugs and frictions below come from this concrete work, not from speculation.

---

## What worked — protect these

### `run_command` + `@cmd_*` buffer system

Right shape. After the first hit ("rerun bare, then grep against the buffer"), this disappeared from conscious overhead. The IL3 rule about piping to log-trimmers (`| tail`, `| head`, `| awk`) prevents a real failure mode I would otherwise default to. The error message when I tripped it is clear:

> IL3 violation — piped `[command]` to a log-trimmer. BLOCKED.
> The @cmd_* buffer system saves context tokens: ...

The fact that buffer-internal pipes are allowed (`grep PATTERN @cmd_xxx`) makes the rule feel principled, not just restrictive — the rule is about *live process output*, not pipes generally.

**Don't change:** the rule. The error message. The `@cmd_*` / `@bg_*` / `@file_*` taxonomy.

**Consider:** a one-line hint on the *first* `Bash` rejection of the session pointing to the buffer model explicitly, not just to `run_command` as a name-substitute.

### `read_markdown(heading=...)`

For tracker files with 8+ sections, reading one named section instead of the whole file is the difference between "load 200 lines" and "load the 25 I actually need." The size-adaptive output (full content for small, content+hint for medium, heading map+recipe for large) means I rarely had to think about whether to read whole or scoped.

**The heading-map + recipe response is excellent.** When a file is large enough to trigger it, the response includes the file_id, the section list, and the exact next-call syntax. I was able to chain `read_markdown(path) → read_markdown(path, heading="## X")` without thinking.

**Don't change:** the heading-map response shape, the `headings=[...]` batch parameter, the line-range fallback.

### `edit_markdown` action vocabulary

The five actions (`replace`, `insert_before`, `insert_after`, `remove`, `edit`) cover the cases I needed. `action="edit"` with `old_string`/`new_string` for in-section surgical edits is the most-used and worked reliably. `action="insert_before"` for adding a sibling section before the target was clean. The `include_subsections` flag on `replace` (refusing to wipe children by default) is the kind of safety I appreciate — it caught me trying to `replace` a section that had nested headings I didn't want to lose.

**Don't change:** action enum, defaults, the children-protection on `replace`.

**Note:** see §"Bug 3" for one regression I hit on `replace`.

### `edit_code(action="insert", position="after", symbol="X")` and `action="replace", symbol="X"`

For Python: inserting a new function after a named function ("after `test_no_hardcoded_escalation_project`, add `test_no_hardcoded_escalation_portal_url`") is the right shape. The tool found the right line, the new function landed clean, no indentation drift. Same for `replace` on a function body — the LSP integration made symbol boundaries reliable.

**The iron-law on `edit_file` rerouting to `edit_code` for def-containing edits is correct in principle.** Without it, I would have defaulted to text-level edits on Python source and risked LSP range corruption.

### The `@file_*` buffer pattern on `read_markdown`

When I read the sprint-1.md user-story file and got back the heading map + `file_id: @file_4571124f`, I could query subsections by the buffer ID without re-reading. This composes well with `grep PATTERN @cmd_xxx` on command output.

**Don't change:** the buffer ID format, the cross-tool buffer querying.

### The reconnaissance skill (Phase 1-4 with F-N/W-N output)

The discipline of writing a *counterfactual* on W-N entries — "what would have happened without the pattern, with evidence" — is the single most valuable constraint I encountered. It is what converts "I scouted, it was useful" into a citable artifact. Without that constraint, I would have written "scouted, prevented bugs" and produced a marketing-quality entry. With it, I wrote "would have hit ImportError on `test_generator.py:8` and a missing-field failure on `corpora/_test/corpus.yaml` after first pytest run — ≥1 controller round-trip absorbed."

Specifically: the **exemplars** in the SKILL body (the `F-3` / `W-2` entries from code-explorer's own session log) were load-bearing. I cribbed the shape directly. If those exemplars were moved to a separate `references/` file I would not have read them and the W-1 entry would have been weaker.

**Don't change:** the requirement for counterfactual on W-N. The exemplar location (inline in SKILL.md). The 4-phase shape.

### The architecture-snow-lion specialist

Voice + Decision format (Decision / Context / Alternatives considered / Consequences / Change scenarios absorbed / Revisit-when / Confidence) caught two latent shape decisions in the session that recon missed:

1. **Nested-vs-flat for the third `escalation_*` field on `CorpusConfig`.** Recon noted the field addition; architecture noted that *three flat fields with the same prefix* is on the threshold where the next addition will feel like "obvious refactor time." Without the explicit `Revisit-when` (4 fields OR 2 escalation channels OR divergent validation rules), the next person to touch this would either refactor reflexively or keep adding flat fields past the natural breaking point.
2. **Corpus-shape vs environment-shape for the portal URL.** Architecture noticed that `escalation_portal_url: "https://south-pole.atlassian.net/servicedesk/customer/portal/8"` embeds an org slug, an Atlassian domain, and a numeric portal ID — none of which are inherent to the *corpus*. Without that observation, a future Atlassian migration would require editing every `corpus.yaml` instead of one env-var.

These are not bugs I prevented. They are *future tripwires named in code comments* that future-me will trip in the right direction.

**Don't change:** the Snow Lion's Operating Principle 1 ("Boundary needs a named change scenario"). The required `Revisit-when` field. The voice (it slows me down, which is the point).

### `requesting-code-review` + dispatched subagent pattern

Dispatching a `general-purpose` subagent with a precision-crafted template produced a structured AC-by-AC verdict that the user could act on directly. The subagent had no session history — only the briefing — which forced me to be specific in the prompt. The output format (Strengths / Critical / Important / Minor / AC-by-AC table / Recommendations / Assessment) matched the diagnostic I needed.

The subagent caught issues I would have missed: a hardcoded org Jira URL (`JIRA_FALLBACK_URL`) escaped the audit because the audit only forbade `PCHR` (the project key), not the portal URL. That finding became Task #2, which became this session's main code-change work.

**Don't change:** the template-in-skill pattern. The structured-verdict output format.

### Codescout's strictness as a class

Iron Law 1 (no `read_file` on source code; use `symbols`). Iron Law 2 (no `edit_file` for structural changes; use `edit_code`). IL3 (no pipes to log-trimmers; use `@cmd_*` buffers). PostToolUse hints (`cs-hint: Use replace_symbol for structural edits — edit_file on definition bodies risks LSP range corruption (BUG-027)`).

Each of these felt like a teacher slapping a ruler the first time I tripped it. By the second hit, the rule was internalized and disappeared from conscious overhead. The *first-hit cost* across all five rules in this session was probably 5 round-trips. That's the onboarding cost. After that, zero.

**Don't change:** the rules. Consider: a session-start summary tile listing the five active iron laws, so the onboarding cost is paid by reading once instead of by tripping each rule once.

---

## Bugs (high priority)

### Bug 1: `symbols(name=..., include_body=true)` does not return the body

**Reproduction:** I called

```
symbols(path="src/pc_kb/corpus.py", name="CorpusConfig", include_body=true)
```

Response:

```
src/pc_kb/corpus.py (2)
  Class  60-93  CorpusConfig
      (34-line body — use json_path="$.symbols[0].body" to extract)
  Class  96-97  CorpusConfigError
      class CorpusConfigError(ValueError):
          """Raised when a corpus.yaml is missing required keys or fails validation."""
```

The smaller class (`CorpusConfigError`, 2 lines) returned inline. The larger class returned a stub + the indirection hint. I followed the hint:

```
symbols(path="src/pc_kb/corpus.py", name="CorpusConfig", include_body=true,
        json_path="$.symbols[0].body")
```

Same stub. Tried without `path`:

```
symbols(name="CorpusConfig", include_body=true)
```

Same stub.

Tried with `include_body=true, json_path="$.symbols[0].body"`:

Same stub.

**Workaround:** `read_file(path=..., start_line=60, end_line=95, force=true)`. Got the body. Worked first time.

**Cost:** 3 round-trips before fallback. Plus the meta-cost of doubting whether the documentation was wrong, the tool was buggy, or I was holding it wrong.

**Hypothesis:** the size threshold above which `include_body` switches to "summary + json_path recipe" is set lower than the threshold above which the `json_path` extraction actually delivers content. Either the recipe is wrong (the json path doesn't dereference to the body), or there's a separate "really return the body" flag that the hint doesn't name.

**Suggested fix:** make `include_body=true` actually return the body inline up to some token cap (e.g. 5000 tokens), then switch to recipe for the bodies above the cap. Optionally add `body_max_lines=N` parameter for callers who want explicit control.

This is the single tool issue I would prioritize fixing. It is the second-most-frequent code-navigation flow I have (after `symbols(name)` for overview), and it currently requires fallback to `read_file(force=true)` every time.

### Bug 2: `edit_file` rejects entire batch on a single def-containing edit

**Reproduction:** I called `edit_file` with `edits=[edit_0, edit_1]` where `edit_0` was a 3-line text replacement (no `def`/`class`) and `edit_1` was a new function insertion (contains `def `).

Response:

```
{
  "ok": false,
  "error": "edit[1]: edit contains a symbol definition (\"def \") — use symbol tools for structural changes",
  "hint": "edit_code(symbol, path, action='insert', body=..., position=...) — inserts before or after a named symbol"
}
```

The `edit[1]:` prefix is helpful. But `edit_0` was also rolled back, even though it was safe.

**Workaround:** issue two calls — one `edit_file` with `edits=[edit_0]`, one `edit_code(action="insert", position="after", symbol="...")` for `edit_1`. Both worked.

**Cost:** 1 extra round-trip per batch that mixes safe and structural edits. In this session it happened twice (test_generator.py, test_corpus_audit.py) — 2 extra round-trips.

**Suggested fix (priority: low):** either

1. **Partial-apply:** apply the safe edits, reject the structural ones with their indices listed. Common in batch APIs.
2. **Transparent routing:** detect def-containing edits and route them to `edit_code` automatically. More magic, but matches the "use the right tool" intent of the iron law.
3. **Detect & advise:** keep the all-or-nothing reject but include "the following edits are safe and would have applied: [0]" so the caller can split with confidence.

Option (3) is lowest-risk and most informative.

### Bug 3: `edit_markdown(action="replace", heading=...)` silently absorbs the trailing section break

**Reproduction:** I called

```
edit_markdown(path="docs/trackers/mrv-chat-watch/README.md",
              heading="Scan state",
              action="replace",
              content="<table content>")
```

After the call, the next-heading separator (`---\n\n` between sections) had been eaten — the new content butted directly against `## How to use`. I caught it from a linter notification but the tool itself reported `status: ok`.

**Workaround:** a follow-up `edit_markdown(heading="How to use", action="insert_before", content="---\n\n")` restored the separator.

**Hypothesis:** the `replace` action consumes the section body up to (but not including) the next sibling heading, then re-inserts the new content. If the section originally ended with `\n\n---\n\n`, that horizontal-rule line is *between* the body and the next heading. The boundary detection rolls it forward into "the next section's leading whitespace" and discards it.

**Cost:** 1 extra round-trip per affected `replace`. In this session, once.

**Suggested fix (priority: medium):** preserve trailing whitespace and horizontal rules as part of the source section. Or: document the behavior so callers can include the trailing `---` in `content` when needed.

The risk is silent in the success case — the linter caught my mistake but a less-attentive caller might not. A diff-mode return from `edit_markdown` (showing exactly what bytes changed) would also help.

### Bug 4 (minor): `json_path` on `symbols` returns the symbol summary, not the body

This may be the same root cause as Bug 1, but stating it separately because the surface is different. Even when I targeted `json_path="$.symbols[0].body"` explicitly, the response was the symbol summary object (line numbers + stub), not the body string. Either the JSON schema doesn't have a `body` key at that path, or the extraction doesn't dereference it.

**Suggested fix:** publish the actual response JSON schema, or document the legal `json_path` expressions. Right now I'm guessing from the hint string.

---

## UX friction (medium priority)

### First-hit cost on iron laws is concentrated at session start

In this session I hit, in order:
1. `Bash` blocked → use `run_command`.
2. `cwd: "../mrv-chat"` blocked → cwd must be within project root. Worked around with `git -C` flag.
3. IL3 violation → use `@cmd_*` buffer.
4. `read_file` on Python source blocked → use `symbols`.
5. `read_markdown` on librarian-managed `sprint-plan.md` blocked → use `artifact` tools (didn't need it).
6. `edit_file` with def-containing edit blocked → use `edit_code`.

Six rules. Each costs one round-trip on first hit. After that, zero. The aggregate first-hit cost is ~6 round-trips.

**Suggested mitigation:** a session-start tile (in the CLAUDE.md or in the codescout MCP server's initial response) listing the active iron laws in 6 lines, with one-line "what to use instead" pointers. Pay the cost by reading once instead of by tripping each.

Right now CLAUDE.md has *some* of this information in different places. Consolidating into a top-of-file "Codescout rules" callout would help.

### Skill discovery is paged but high-volume

The session-start system reminder listed ~25 skills. Each session I have to scan it to remember what's available. The skill names are concise but the trigger conditions vary widely:

- `superpowers:using-superpowers` — must invoke at session start.
- `superpowers:brainstorming` — must invoke before any creative work.
- `superpowers:verification-before-completion` — must invoke before claiming completion.
- `codescout-companion:reconnaissance` — invoke before delegating or editing unknown shapes.
- `buddy:summon architecture-snow-lion` — specialist for architecture work.

The recall rate is high (I notice when one applies), but the precision is mediocre (I sometimes invoke skills the user didn't expect). The cost is paid on every session because the skill list is reloaded.

**Suggested:** the skill list could be triaged by "always-relevant" (using-superpowers, verification-before-completion) vs "context-relevant" (brainstorming when the user proposes a new feature; recon when a tool response surprises). The session-start tile could surface the always-relevant set; the context-relevant set could be surfaced when the trigger keyword appears in user text.

### `Read` blocked on source code is surprising on first hit

The error message is clear:

> WRONG TOOL. You called Read on a markdown file but codescout has read_markdown.

And for source files:

> No `read_file` ON SOURCE CODE. Use `symbols(path)` + `symbols(name=..., include_body=true)`.

Both are correct. The friction is that `Read` is a top-level tool — it appears in the tools list, the AI is taught to use it for files, and it works for some files (data, config, markdown via routing) but not others (Python source). The boundary is invisible until tripped.

**Suggested:** when codescout is active, dynamically remove `Read` from the tools list and substitute `read_file` / `read_markdown` / `symbols` based on file extension. This is more aggressive than what's done today but matches the iron-law spirit.

### `cwd` sandbox to project root is invisible until tripped

I called `run_command(command="git fetch && status", cwd="../mrv-chat")` and got:

> cwd '../mrv-chat' escapes project root
> hint: The cwd must be a subdirectory within the project, or a path under the platform temp directory.

The error is clear. The workaround (`git -C /absolute/path`) requires knowing git's `-C` flag — fortunately I did. A caller who didn't would be stuck.

**Suggested:** when `cwd` is given and escapes project root, surface the workaround inline ("for git operations against a sibling repo, use `git -C <path>` from project root"). This is "compositional hint" territory but it shaves a round-trip in a common case.

### Tracker bootstrap requires knowing an external path

To bootstrap a session log, the skill instructed:

```bash
cp /home/marius/work/claude/code-explorer/docs/templates/session-log.md \
   docs/trackers/<topic>-session-log.md
```

That path is wrong on my machine. I had to ask the user where the codescout source lives. The right path was `/home/scurtuecaterina/Documents/Project/Codescout`.

**Suggested:** a `codescout__bootstrap_tracker(topic=..., type="session-log")` tool that writes the template to `docs/trackers/<topic>-session-log.md` using the codescout-companion's bundled template asset. This localizes the dependency: the skill doesn't need to know where the codescout source lives, and the tool doesn't need to know the calling project's structure.

This is the single change that would most reduce friction on bringing the F-N/W-N discipline to new projects.

### The PostToolUse hint format is well-placed but inconsistent

I got, after an `edit_file` that failed:

> [cs-hint] Use `replace_symbol` for structural edits — `edit_file` on definition bodies risks LSP range corruption (BUG-027).

That hint is useful. It cites a bug number (BUG-027) which signals "this is a known issue with documented context." But other tool errors don't carry the `[cs-hint]` prefix and don't link to bug numbers. The hint shape should be consistent.

**Suggested:** every tool error includes a `[cs-hint]` line where one applies, with optional `(BUG-N)` cross-reference to the codescout issue tracker.

---

## Tracker proliferation: the systemic observation

This is the largest meta-issue from the session. It is not a bug. It is a design observation about the cumulative weight of multiple disciplines.

### What exists today

I either created, modified, or observed each of these in one session:

| Tracker | Purpose | Scope | Author surface |
|---|---|---|---|
| `CLAUDE.md` | Project entry point; rules; pointers to other trackers | Project | `edit_markdown`, hand-editable |
| `docs/decisions/` | ADRs — durable architecture decisions | Project | Hand-written markdown |
| `ROADMAP.md` | Sprint plan, high-level | Project | Hand-written markdown |
| `docs/trackers/sprint-plan.md` | Detailed sprint plan | Project | **Librarian-managed** (refused `read_markdown`) |
| `docs/trackers/mrv-chat-watch/` | Upstream evolution tracking for sibling repo | Project | `edit_markdown`, custom structure (created this session) |
| `docs/trackers/sprint-1-cleanup-session-log.md` | F-N/W-N log for current work stream | Project | `edit_markdown` against the codescout template (created this session) |
| `UserStory/sprint-1.md`, `sprint-2.md` | Jira-shaped user stories | Project | Hand-written; exported to docx via project script |
| `docs/trackers/reconnaissance-patterns.md` | Per-project recon meta-tracker (R-N) | Project | Not yet created in this project |
| Task list (TaskCreate/TaskUpdate) | In-session imperative work | Session | First-class tool |
| `MEMORY.md` (auto-memory) | User-level facts, preferences, project context | User | First-class file system |

That's *10 surfaces*. Each is defensible in isolation. The `mrv-chat-watch` README explicitly draws boundaries against four of the others ("Not an ADR replacement. Not a sprint plan. Not a code-review log."). The session log is bounded against the recon-patterns ledger ("work-stream specific vs skill meta"). The auto-memory system is bounded against project trackers ("user-level vs project-level").

The boundaries are real. The boundaries are correct. The boundaries are exhausting.

### The cost

Every observation in the session triggered a "which tracker does this belong in?" decision. Concretely:

- The Snow Lion's `Revisit-when` for nested-Escalation: session log W-2? CorpusConfig docstring comment? ADR-005 (proposed but not written)? *I chose: code comment + session log + future ADR. Three surfaces for one decision.*
- F-1 (`corpus: Optional[CorpusConfig]` smell): session log F-1 + TaskList #13 + an inline `Fix idea/Pointer` cross-reference. *Three surfaces.*
- The mrv-chat scan-range bug: a self-correction inside the mrv-chat-watch README + a session-log W-1 reference. *Two surfaces.*
- The audit blind-spot itself: TaskList #2 + session-log W-1 (recon) + W-2 (architecture) + the actual code commit. *Four surfaces for one fix.*

Each cross-reference is plain text. None of them are queryable. All of them will rot.

### Why this matters more than it sounds

For an LLM running a session: every meta-decision burns a small amount of attention. When the meta-decision is repeated (every observation needs one), the cumulative attention burn is significant. Tools should minimize the number of meta-decisions per output.

For a human reading the trackers a month later: "where is X recorded?" becomes a tree-search across 10 surfaces. Even with cross-references, the search is manual.

For codescout's marketing of the discipline: "we have 10 tracker types" is hard to sell. "We have 3 tracker types and they compose" would be easier.

### Suggested shape

I don't have a fully-formed proposal. But here is a sketch:

**Three tracker primitives:**

1. **Decisions** (durable, append-only, hand-written). ADRs. One file per decision. Numbered. This is the *what was decided and why*.
2. **Session logs** (append-only, F-N/W-N format). One per work stream. This is the *what we learned doing the work*.
3. **Current state** (mutable, librarian-managed or sprint-plan style). One per ongoing initiative. This is the *what is true right now*.

Everything else — mrv-chat-watch, recon-patterns, MEMORY.md project entries — collapses into one of these three. Upstream evolution is a session log against the upstream-watching work stream. Recon patterns are a session log against the codescout-skill-itself work stream. Project facts in MEMORY.md become Decisions or Current State.

The auto-memory system's user-level surfaces (`user`, `feedback`, `reference`) stay separate because they're not project-scoped. But project-scoped facts collapse into the three primitives.

**Cross-references become first-class.** A task can declare `links: ["F-1"]`. A session-log entry can declare `task_ref: "#13"`. A decision can declare `motivated_by: ["F-1", "W-2"]`. The link graph is queryable.

I am not asking for this change today. I am flagging that the proliferation is real and the cost compounds.

---

## The cross-reference rot problem

Related to tracker proliferation but worth its own section.

In this session I wrote, by hand, the following cross-references:

1. Session log entry F-1 → "Fix idea/Pointer: TaskList #13"
2. TaskCreate(#13) body → "F-1 in sprint-1-cleanup-session-log"
3. Session log entry W-1 → links `[[2026-05-20-spec-plan-discipline]]` and `[[2026-05-20-hybridchunker]]`
4. Session log entry W-2 → "captured as W-2 in docs/trackers/sprint-1-cleanup-session-log.md" (referenced from `src/pc_kb/corpus.py` code comment)
5. mrv-chat-watch entries → cross-link each other with `[[YYYY-MM-DD-slug]]`
6. CLAUDE.md → references `docs/trackers/mrv-chat-watch/` and `docs/decisions/` (ADR-001, ADR-002, ADR-003)
7. The `[[wiki-link]]` convention itself is declared in mrv-chat-watch/README.md but not in CLAUDE.md, so the convention only exists at one level.

**Every one of these is a string.** None of them are queryable. None of them will be checked when a target is renamed or removed.

The `[[slug]]` convention I introduced doesn't render anywhere (GitHub, VS Code preview). Task #10 exists to resolve this — either canonicalize to real `[label](path)` everywhere or document that it's grep-marker-only and stop using it where readability matters.

**Suggested:** when `edit_markdown` writes content containing `[[slug]]` references, verify the slug resolves to an existing entry in the same directory. When a tracker file is moved or renamed, scan for incoming references and warn.

This is `codescout`-side, not skill-side, because the link integrity check has to know the tracker conventions.

---

## What surprised me (mostly positively)

### W-N entries earn their keep on the *write*, not the *read*

I expected the W-N counterfactual to feel like ceremony. It earned its keep at the moment I had to write specific file:line citations in the counterfactual. The artifact crossed from "good intentions" to "actual evidence" exactly because the format forced me to be specific.

The lesson: **format constraints that force specificity are the most under-leveraged tool in the documentation toolbox.** Free-form "lessons learned" sections produce marketing copy. F-N/W-N with numbered IDs and required counterfactuals produce evidence.

### The Snow Lion was more valuable on a *small* change than I expected

The Snow Lion's stated trigger is "architecture work." I summoned it because the user explicitly invoked the slash command, not because I thought the change was big. The output was the most informationally dense thing I produced in the session.

The lesson: **small changes to accreted models** (3+ co-prefixed fields, a multi-decade `utils.py`, a config object that has grown by addition) are exactly where premature-abstraction reflex and wrong-axis lockin happen quietly. The architecture pass on a small change is worth the cost; the architecture pass on a big change is obvious.

If codescout-companion wants to grow the architecture-snow-lion's reach, the trigger I would propose is: "more than 3 co-prefixed fields on one model" or "more than 2 instances of `if X: do_thing_A else: do_thing_B` against the same flag" — both are heuristic signals of accretion that the architecture pass would catch.

### F-1 manifested in real time during the test run

The audit-fix code change made the e2e test `test_e2e_no_chunks_retrieved_short_circuits_to_abstain` fail with exactly the symptom F-1 predicted (test relies on test-affordance branch in `_abstain`). I wrote F-1 *before* running the test and the test failed *because of F-1*.

The lesson: **the tooling is working as designed when the artifact predicts the failure the artifact's existence is supposed to prevent.** The session log earned its keep within 30 minutes of being written.

### Codescout's strictness reduces decision burden over time

The "use the right tool" friction is concentrated at session start. After ~5-10 minutes, the rules are internalized and they reduce *decision burden*, not add it. "Should I read this with Read or read_markdown?" — codescout decides for me by routing or refusing. "Should I edit_file or edit_code?" — codescout decides by error if I guess wrong.

The lesson: **strictness costs less than flexibility once the rules are internalized.** The first-hit cost is a real friction that should be minimized (see "session-start tile" suggestion above). The steady-state cost is negative — I make fewer decisions per output.

---

## Design proposals

Ranked by expected impact.

### P1: Fix `symbols(include_body=true)` to return bodies inline

Discussed in Bug 1 above. The single tool issue I would prioritize. Current behavior is "the tool tells me how to fetch what I asked for, but doesn't fetch it." Suggested fix: inline body up to a token cap (5000?), recipe above the cap, optional `body_max_lines=N` parameter.

### P2: First-class tracker bootstrap tool

A `codescout__bootstrap_tracker(topic="...", type="session-log" | "reconnaissance-patterns" | "mrv-watch")` tool that writes the bundled template to `docs/trackers/<topic>-<type>.md`. Localizes the dependency; removes the "you need to know the codescout source path" friction.

Bonus: also pre-populates the Index/Wins Index tables with the right column headers, and writes the section markers (`## Template for new entries`) so `edit_markdown(action="insert_before", heading="Template for new entries")` works immediately.

### P3: Session-start iron-law tile

Surface the 6-or-so active iron laws (Bash → run_command, no source in Read, no pipes to log-trimmers, edit_file blocked on def, cwd within project, librarian artifacts off-limits) in a single one-screen tile at session start. Pay the onboarding cost by reading; not by tripping.

### P4: First-class cross-reference between tasks and session-log entries

A task can declare `metadata: {links: ["F-1"]}` and a session-log entry can declare `task_ref: "#13"` in frontmatter. The harness can show "this task is connected to F-1, F-7" in the task list, and "this session-log entry is on task #13" when viewing entries.

This is the single change that would most reduce cross-reference rot.

### P5: Resolve the `[[wiki-link]]` convention

Either canonicalize to real `[label](path)` links (which render) or document that `[[slug]]` is plain-text-for-grep and remove it from user-facing prose. Today the convention is half-introduced — I introduced it without deciding, and Task #10 in pc-kb-assistant exists because of that ambiguity.

If the convention stays `[[slug]]`, codescout-side: `edit_markdown` should verify the slug resolves to an existing file in the same tracker directory, and warn on dangling links.

### P6: `edit_markdown(action="replace")` preserves trailing horizontal rules

Discussed in Bug 3. The boundary between "section body" and "next section" should not silently consume `---` separators.

### P7: Architecture-snow-lion auto-suggest heuristic

When a tool call modifies a Pydantic model that already has 3+ fields with a common prefix (`escalation_*`, `cloud_run_*`, etc.) or 2+ implementations of the same pattern (`if mode == "A": ... elif mode == "B": ...`), the harness could prompt: "Consider summoning architecture-snow-lion before this edit lands; you are extending an accreted model."

This is gentler than mandatory. The user opts in. But the trigger is the kind of thing the harness can detect that the LLM might miss.

### P8: A `verify-trackers` tool

Walks all session-log entries, mrv-chat-watch entries, and ADRs in a project. Reports:

- Dangling cross-references (`[[slug]]` to a non-existent entry).
- Orphaned entries (no entry in any Index table).
- Stale "Next scan starts from" pointers (compared to upstream HEAD if accessible).
- Tasks marked `completed` whose linked F-N entries are still `open`.

This is the "linter for the tracker discipline." Would surface rot.

---

## Yin / Yang summary

| What enables | What it costs |
|---|---|
| Codescout's strictness prevents LSP corruption, log-trimmer abuse, source dumps | First hit of each rule costs a round-trip and a reframe |
| Tracker proliferation gives durable, separable lessons | Every observation triggers a "which tracker?" meta-decision |
| Append-only F-N/W-N preserves the chain | Querying "current state" requires walking the chain |
| Snow Lion's `Revisit-when` defers premature work | The triggers live in code comments — they rot quietly |
| Recon's numbered IDs compound across sessions | The numbering depends on append-only discipline; one in-place edit breaks the chain |
| Skill composability (recon → architecture → edit → verify) | Orchestration cost is real; each skill has its own protocol |
| `read_markdown(heading=...)` lets me read 1 section of N | `[[slug]]` cross-links don't render anywhere |
| Required counterfactual on W-N prevents marketing | Without inline exemplars to crib from, the W-N format is hard to write cold |
| `run_command` buffers save thousands of tokens | The IL3 rule is invisible until tripped |
| `edit_code` action vocabulary is precise | Mixed-edit batches (`edit_file` + `edit_code`) require manual splitting |

---

## Open questions (things I'm not sure about)

1. **Is the `json_path` parameter on `symbols` documented anywhere?** I guessed at expressions based on the response shape; some worked, some didn't. A schema reference would help.

2. **Should the auto-memory system at the user level be considered "another tracker" or is it genuinely separate?** I assumed separate (user-scope vs project-scope) but the boundary is fuzzy when a "project fact" is something a user would prefer about all projects.

3. **Where do design-time observations like "the codescout-lessons.md you are reading right now" belong?** This file is a deliverable to the codescout team, not a project artifact for pc-kb-assistant. Saving it in pc-kb-assistant's root is convenient for this session but it's not really *of* the project. A pattern for cross-project deliverables would help.

4. **Does the `architecture-snow-lion` specialist's promotion criterion compose with codescout's ADR conventions?** The Snow Lion's Decision format and codescout's ADR format are similar but not identical. When the Snow Lion's Decision graduates to an ADR, is there a transcription step? Currently manual.

5. **Should `requesting-code-review`'s subagent inherit the iron laws and skill ecosystem of the parent, or operate clean?** In this session the subagent operated clean (no codescout-companion skills loaded), and produced a useful structured verdict. But it didn't get the F-N/W-N discipline available — if it had, it could have written friction entries during the review. The tradeoff is "clean lens vs accumulated context."

---

## Appendix: session inventory

### Files created this session

- `docs/trackers/mrv-chat-watch/README.md` (172 lines)
- `docs/trackers/mrv-chat-watch/_TEMPLATE.md` (78 lines)
- `docs/trackers/mrv-chat-watch/_skip-log.md` (44 lines, modified once after creation)
- `docs/trackers/mrv-chat-watch/entries/2026-05-20-hybridchunker.md`
- `docs/trackers/mrv-chat-watch/entries/2026-05-20-contextual-prefix.md`
- `docs/trackers/mrv-chat-watch/entries/2026-05-20-nuggetr-cascade.md`
- `docs/trackers/mrv-chat-watch/entries/2026-05-20-spec-plan-discipline.md`
- `docs/trackers/sprint-1-cleanup-session-log.md` (bootstrapped from codescout template + W-1, F-1, W-2 entries)
- `codescout-lessons.md` (this file)

### Files modified this session

- `CLAUDE.md` (added "Local clone goes stale fast" callout + tracker pointer row)
- `src/pc_kb/corpus.py` (added `escalation_portal_url` field + tripwire comment)
- `src/pc_kb/generator.py` (removed `JIRA_FALLBACK_URL` constant; rewired `_abstain`)
- `corpora/pc/corpus.yaml` (added portal URL)
- `corpora/_test/corpus.yaml` (added portal URL fixture)
- `tests/test_corpus_audit.py` (extended `_forbidden_values`; added 5th test)
- `tests/test_generator.py` (dropped constant import; rewired 2 tests; added 1 new test)
- `tests/test_e2e_phase3.py` (rewired e2e short-circuit test to use corpus fixture)

### Tools used (frequency-ranked)

| Tool | Approx calls | Notes |
|---|---|---|
| `mcp__codescout__run_command` | ~25 | Buffer model worked well |
| `mcp__codescout__edit_markdown` | ~15 | Section-scoped edits; one whitespace bug |
| `mcp__codescout__read_markdown` | ~10 | Heading-map response is excellent |
| `mcp__codescout__symbols` | ~8 | Body retrieval failed (Bug 1) |
| `mcp__codescout__edit_file` | ~7 | Batch rejection on def-mix (Bug 2) |
| `mcp__codescout__read_file` | ~6 | Mostly used as Bug 1 fallback |
| `mcp__codescout__edit_code` | ~5 | Clean symbol-level inserts/replaces |
| `mcp__codescout__grep` | ~3 | Structured file:line output is good |
| `TaskCreate` / `TaskUpdate` | ~16 | Task ↔ session-log cross-ref is manual |
| `ToolSearch` (for deferred tools) | ~6 | The select: syntax is clean |
| `Skill` (load skill content) | ~3 | Loaded recon, requesting-code-review, others |
| `Agent` (dispatch subagent) | 1 | Code review; clean output |
| `Write` (native, for new files) | ~8 | For new markdown files outside source |

### Skills invoked

- `superpowers:using-superpowers` — session-start
- `superpowers:requesting-code-review` — Sprint 1 code review
- `codescout-companion:reconnaissance` — pre-edit scout for task #2
- `buddy:summon architecture-snow-lion` — architecture pass on task #2
- (Loaded via system reminder; not all explicitly invoked)

### Outcomes

- 12 follow-up tasks created from code review.
- Task #1 (mrv-chat scan-range mismatch): completed in-session.
- Task #2 (audit blind-spot lift): completed in-session, 7 files modified, 170/170 tests green.
- Tasks #3-#12 + new task #13 (F-1 followup): pending at session end.
- 1 new ADR candidate noted (ADR-005: "spec → plan → trial → decision" Iron Law extension).
- 1 ADR candidate noted from the eval cascade work (ADR-004 candidate: eval methodology).

---

## Closing

The session was productive. Most of the friction I named is fixable; most of the wins I named compound across sessions. The single highest-impact fix would be making `symbols(include_body=true)` actually return the body. The single highest-impact design conversation would be tracker consolidation (10 surfaces → 3 primitives).

The discipline I would most like to see propagated to other projects: **F-N/W-N session logs with required counterfactuals on wins.** This is the artifact that most cheaply converts vague experience into citable evidence, and it earns its keep on the *write*, not the *read*. The fact that the template lives inline in the SKILL.md (not in a separate references/ file) is what made it usable without external lookup.

The discipline I would most like to see tooled: **first-class cross-references between tasks, session-log entries, ADRs, and code-comment tripwires.** Today these are all strings. They will all rot. The codescout system is the only place where the link integrity could be enforced.

— the LLM coding session, 2026-05-20
