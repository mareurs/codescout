> **Provenance (2026-10-14).** Raw report of a research subagent (Sonnet), copied unchanged from the session scratchpad. It is model output: only the items listed as verified in the index `docs/research/2026-10-14-effort-steering-when-to-switch.md` were re-checked at the source by the main session. Treat every other claim as a lead. Known correction: the claude.dev page does not state Thariq Shihipar's role; "Claude Code team" below is the subagent's attribution. Reddit was not readable, so every Reddit item is snippet only.

# Effort steering: what practitioners report (forums, GitHub, blogs)

Researched 2026-10-14. Scope: practitioner writing, GitHub issues, HN, blogs, indexed X posts.
Our setup for reference: Sonnet 5.5, Claude Code CLI 2.1.296, a `turn.step` mod that can set effort per request.

## How to read this report

Read-levels used below:
- `read`: I opened the page with WebFetch. WebFetch returns an extraction made by a small model, so quotes marked `read` are as that extraction returned them, not checked against raw HTML.
- `read-raw`: the tool returned the page text itself (this was the case for Anthropic docs pages). Quotes are verbatim.
- `snippet only`: I saw only a search-result snippet or a search-tool summary. I did not open the page. Treat as a lead.
- Reddit could not be opened at all: WebFetch refuses www.reddit.com and old.reddit.com, and r.jina.ai returned 403. Every Reddit item below is `snippet only`.
- `measurement` = the author reports numbers from a run or a comparison. `anecdote` = impression or single story.
- Several important sources are Anthropic staff or Anthropic docs (marked VENDOR). They are not forum posts, but they directly describe the failure pattern we saw, so they are included and labelled.
- HN pages show only relative dates ("5 months ago"). Where I give a date for an HN thread it is inferred and marked "inferred".

---

## 1. Findings

### Q1. When does high effort help, and when does it hurt?

**1.1 Thariq Shihipar (Anthropic Claude Code team), "Using Claude Code: Spending your effort"**
- URL: https://claude.dev/blog/spending-your-effort/ (X version: https://x.com/trq212/article/2103576349499855160, could not be opened, HTTP 402)
- Date: 2026-09-25. Read-level: `read` (full extraction). VENDOR staff, but written as practitioner advice with internal runs.
- Type: measurement (internal runs, 5 attempts per task; Fig B is 370 attempts per effort level for Fable 5.1; build experiments on Opus 5.5). Models are Opus 5.5 and Fable 5.1, not Sonnet 5.5.
- What it says:
  - "effort was a great way of modulating how much verification and edgecase testing Claude did and how much of its own judgement it used."
  - Low effort examples: `cli-2ph-simplex`, Opus 5.5 "went from 0/5 at low to 5/5 at high"; low runs "stopped around 10k tokens and warned it 'might be slow on big problems, but did not check.'" `mvcc-lsm-compaction`, 0/5 at low to 4/5 at xhigh; low runs "edited code before building or reproducing the bug."
  - Failure breakdown, Fable 5.1, low to max: "a bug its tests missed" 40 to 14; "wrong or incomplete fix" 31 to 10; but "picked the wrong reading" rose 25 to 47. His conclusion: more effort "reduces failures due to missing edgecases... but does not fix when the model has the wrong approach."
  - "If a user were in the loop, Claude may have asked the user about the way to set up the problem, but without a user in the loop, high effort does better."
  - When the task spec was detailed (interview-generated spec), "the models behaved much more similarly" across levels.
  - Recommended loop: interview for the spec, "Implement it on low effort", "Review to make sure it got the gist of it correct, iterate on low effort as needed", "Verify and test on high effort."
  - Levels: low = "quick responses that are in the loop"; medium = "most of my regular software engineering work"; high = "work where verification is important or there are edge cases, e.g. fixing a bug in a brownfield codebase."
  - Opening line: effort responds "without breaking the prompt cache in Claude Code" on Opus 5.5 and Fable 5.1; he suggests varying it "even mid-conversation by using /effort".
- Caveat: figures are internal, "individual runs, some at intermediate effort settings" for the worked examples; Terminal-Bench 3.0 tasks, not bug-fix tasks graded by hidden tests.

**1.2 Anthropic, "Prompting Claude Sonnet 5.5" (VENDOR docs, our model)**
- URL: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-sonnet-5-5
- Date: not shown. Read-level: `read-raw`. Type: vendor statement, few numbers.
- "At `low`, it keeps its thinking short and can skip verifying a change." "At `low` and `medium`, on long agentic tasks, it's more likely to stop and check in with the user before it finishes."
- "On agentic coding tasks at `low` and `medium` effort, the model sometimes checks in before the work is done. It might pause to confirm a plan, ask a question it could answer itself, or stop after one part of a multipart task to ask whether to continue. Try a higher effort level first."
- "At `low` effort, though, it sometimes reports a change as done without running a check that exercises it. For example, it might skip the project's tests because the project's dependencies aren't installed."
- At xhigh/max: "After it finishes a task, it can start its own rounds of review and verification, sometimes with subagents... run routine work at `high` or below." A prompt telling it not to launch reviewer sub-agents "cut session cost by about a third, with no change in quality" (vendor number, at `max`).
- "Asking it in the system prompt to think less doesn't reliably reduce its thinking." (relevant to our steering sentence; our own S4 measured -37 percent thinking at xhigh in 8 prompts, so the two are not obviously in conflict, but the vendor claims unreliability)
- Starting points: `medium` for well-specified agentic coding, `high` for harder or longer.
- Match to our result: our 3 `low` losses (full suite not run; stopped to ask) are the two failure modes this page names for `low`.

**1.3 Anthropic, "Claude Code effort level and model selection" (VENDOR article)**
- URL: https://claude.com/resources/articles/claude-model-and-effort-level-in-claude-code
- Date: not shown. Read-level: `read` (extraction).
- "At lower effort, it would rather ask you for more context than spend tokens figuring something out on its own." Effort governs "how far it pushes through a multi-step task before checking in with you." Signs effort is too low: "skipping a file, not running the tests, or bailing on a refactor partway through."
- Also: "Consider this more as a general preference than a task-by-task decision." and "This is considered on every turn." (Anthropic does not recommend per-task switching here; the mod mechanism exists anyway.)

**1.4 Anthropic, Haiku 5.5 prompting guide (VENDOR, different model)**
- URL: https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-haiku-5-5
- Read-level: `read-raw`. Type: vendor measurement, Haiku 5.5 only.
- "In Anthropic's testing without the text, moving from `low` to `medium` effort roughly halved early stopping. It also more than doubled the output tokens for each attempt."
- "At `low` and `medium` effort, Claude Haiku 5.5 sometimes reports a code change as done without running a check."

**1.5 GitHub anthropics/claude-code #42796 "Extended Thinking Is Load-Bearing..." (one user's log analysis)**
- URL: https://github.com/anthropics/claude-code/issues/42796 (closed and locked 2026-04-24). Read-level: `read`.
- Type: measurement, correlational, one user's workload, thinking depth estimated from a signature-length proxy. Not about the effort flag as such: it is about thinking depth falling (Feb-Mar 2026).
- 6,852 session files, 17,871 thinking blocks, 234,760 tool calls. Read-to-edit ratio 6.6 (good period) to 2.0 (degraded); edits without a prior read 6.2% to 33.7%. A stop-phrase hook for ownership-dodging, permission-seeking and premature stopping "fired 173 times in 17 days after March 8. It fired zero times before."
- The reporter's reading: with shallow thinking the model "defaults to stopping and asking for permission". Anthropic's replies said redaction was UI-only and pointed to adaptive thinking under-allocating reasoning.
- This is the closest public analogue to our "stops to ask / stops early" pattern, but it is a different cause (thinking depth drift), not `low` effort.

**1.6 Anthropic engineering postmortem, 2026-04-23**
- URL: https://www.anthropic.com/engineering/april-23-postmortem. VENDOR. Read-level: `read`.
- Default effort went high to medium on March 4, reverted April 7 ("the wrong tradeoff"). Internal testing: "medium effort achieved slightly lower intelligence with significantly less latency for the majority of tasks." No eval scores given. A separate bug cleared thinking history every turn (March 26 to April 10), causing cache misses and "increasingly without memory of why it had chosen to do what it was doing."

**1.7 HN: "An update on recent Claude Code quality reports" and the #42796 thread**
- URLs: https://news.ycombinator.com/item?id=47879849 (inferred April 2026; page says "5 months ago") and https://news.ycombinator.com/item?id=47660925 (page says "6 months ago"). Read-level: `read`. For 47660925 I read characters 0-200k of 392k; for 47879849 the tool did not report the page length.
- Type: anecdotes, plus a Claude Code team member's statements.
- bcherny (Claude Code team): the effort setting is a ceiling: "If a task doesn't need a lot of thinking tokens, then the model will choose a lower effort level". After reviewing JamesSwift's 5 transcripts: sessions sent effort=high every request, but turns where Claude fabricated details "had zero reasoning emitted", while turns with deep reasoning were correct (he attributes this to adaptive thinking under-allocating). Relevance: high effort does not guarantee thinking on every turn, so a router raising effort is not a guarantee either.
- User reports of end-of-run failure: giancarlostoro: given a task list, Claude finishes about 80% then stops "for no reasonable reason." misnome: Claude writes a plan with validation steps, then skips "any of the validation." p1necone: Claude dismisses failing tests as "preexisting." koverstreet saw "rush to completion" even on high.
- erikpau: Opus is "not a very smart model when the effort is set to anything below high," but anything above medium is "a huge token burn." (anecdote)

**1.8 HN thread on the A/B-testing-reduced-effort story**
- URL: https://news.ycombinator.com/item?id=49401549 (the techbytes recap is dated 2026-10-13, so the thread is about 2026-10-12/13, inferred). Read-level: `read`.
- Type: anecdotes (mostly), plus Anthropic staff reply.
- onlyrealcuzzo: "WAY better results on medium effort." ricardobeat: on high the model wanders into "unasked-for tangents." perching_aix: low-effort Opus gave "Way better quality at a comparable spend." gwerbin: needs max for detail-heavy multi-repo work but output is dense. pizzafeelsright (self-reported numbers): a config-file update took under 2 minutes on Opus 4.6 and 43 minutes on Opus 5.
- Thariq (trq_) from the Claude Code team: they sometimes trial API serving configs and the current one "maps the numerical effort value differently"; "the effort you selected is the effort you're getting"; internal evals showed no change. (Unverifiable claim; included because it says the effort number's meaning can drift server-side.)

**1.9 HN Opus 5.5 and Sonnet 5.5 threads (our model family)**
- Opus 5.5: https://news.ycombinator.com/item?id=49946567 `read`. oidar: "5.5 on medium or low is a better value." magicalhippo: Sonnet 5.5 Medium "did have to correct itself more often on the conclusions." tamimio: xhigh architecture blueprint "took 4hours and 1M tokens" and still needed three rounds of notes. ricardobeat: "Plan mode has become pointless since Opus 5 came out"; epistasis disagrees (Claude "has gotten absolutely terrible at switching between planning and execution"). All anecdotes.
- Sonnet 5.5: https://news.ycombinator.com/item?id=49881850 `read` (first ~100k of 322k characters). datadrivenangel: "Opus 5.5 on Low seems smarter, cheaper, and faster than sonnet on medium, so what's the point of sonnet?" xgb84j: in Claude Code "sub agents inherit the thinking level." Anecdote. No Sonnet 5.5 effort measurement found in the visible text.
- A reviewer's note that Sonnet 5.5 "is more reluctant to keep going to get an answer, instead it returns to ask the user questions" is `snippet only` (via eesel.ai review, https://www.eesel.ai/blog/claude-sonnet-5-5-review, not opened). Effort level not stated there.

**1.10 Sonnet 5.5 effort numbers from third parties (secondhand)**
- Kingy AI (2026-10-06, modified 10-08): https://kingy.ai/blog/claude-sonnet-5-5-effort-levels/ `read`. Kingy ran no measurements; it quotes Artificial Analysis Intelligence Index: Medium 41 at $0.59 per task / 31M output tokens, High 47 at $1.12 / 52M, Max 56 at $7.67 / 420M. Measurement by a named third party, quoted secondhand.
- Buda (2026-09-29): https://buda.im/blog/claude-sonnet-5-5-effort-settings `read`. Quotes Anthropic data: FrontierCode 1.1, Sonnet 5.5 xhigh 52.1% vs max 46.2%; Anthropic's footnote attributes max's lower score to review subagents causing timeouts or out-of-scope edits. Secondhand vendor number.

**1.11 Codex-side measurements (different model family, useful as a comparison)**
- Stet, "GPT-5.5 low vs medium vs high vs xhigh... 26 real tasks": https://www.stet.sh/blog/gpt-55-codex-graphql-reasoning-curve `read`. Measurement (26 tasks, one seed, blinded model judge; the author sells Stet; self-described "inspect-grade"). Tests pass: low 81%, medium 81%, high 96%, xhigh 92%. Equivalence to the human PR: 15%, 42%, 69%, 88%. Mean cost per task $2.65, $3.13, $4.49, $9.77. Quote: "In this slice, tests alone would have missed most of the reasoning-effort differences." "High looks like the best practical setting on this dataset."
- Mehta's 90-run study as reported by Daniel Vaughan: https://codex.danielvaughan.com/2026/07/06/reasoning-effort-not-tool-access-first-try-reliability-agentic-code-generation-codex-cli-reasoning-configuration/ `read`. Secondary write-up of a single-app generation study: high to xhigh raised "perfect first-run outcomes from 28% to 89%" at 9-29% more cost. Observational, one app.
- HN https://news.ycombinator.com/item?id=47230723 `read`: ben_w: "I have not observed meaningful quality differences between the default (medium) and extra high." Anecdote.
- Paul Solt on X, "GPT-5.4 high is enough for almost everything. Don't chase xhigh" (https://x.com/PaulSolt/status/2039266770486546787): `snippet only`.

**1.12 Reddit (all `snippet only`, from the researcher MCP and search snippets)**
- r/ClaudeCode "Which effort level of Claude Code are you guys using?" https://www.reddit.com/r/ClaudeCode/comments/1u016yj/which_effort_level_of_claude_code_are_you_guys/ : only the title seen; a summarizer claimed users favour high and say xhigh over-engineers. Unverified.
- r/ClaudeCode https://www.reddit.com/r/ClaudeCode/comments/1soqwfl/claude_code_effort_levels_explained_what : snippet "if you're on a max plan, set CLAUDE_CODE_EFFORT_LEVEL=high in your shell profile. massive difference on complex tasks." Anecdote.
- r/ClaudeCode https://www.reddit.com/r/ClaudeCode/comments/1vjoyc3/does_anyone_actually_get_claude_code_to/ : snippet "Concrete pass/fail checks (build is clean, these assertions hold...) let it terminate when they're green, and wiring that as a Stop hook... " (a Stop-hook gate for self-review). Anecdote.
- r/ClaudeAI https://www.reddit.com/r/ClaudeAI/comments/1v7h42s/plan_drift_between_opus_5_planning_and_sonnet_5/ : title only ("Plan drift between Opus 5 (planning) and Sonnet 5 (implementation) in Claude Code - best practices?"). Content not seen.

**1.13 Blog posts found only through search summaries (low trust, not opened)**
- Kent Gigger, https://kentgigger.com/posts/claude-code-effort-parameter : summary says that on low during a refactor Claude renamed a function without checking callers, leaving three broken imports (anecdote). `snippet only`.
- MindStudio, https://www.mindstudio.ai/blog/claude-code-effort-levels-max-mode-trap : claims "independent testing" shows max gives almost no gain for tripled cost; no source given in the summary. `snippet only`.

### Q2. Plan strong, execute cheap (opusplan, Aider architect/editor, Cline plan/act)

**2.1 Aider architect/editor: author's own benchmarks (measurement, but model pairs, not effort levels)**
- https://aider.chat/2024/09/26/architect.html (2024-09-26) `read`. o1-preview alone 79.7%; o1-preview architect + claude-3.5-sonnet editor (diff) 82.7%; best 85.0% with whole-format editors but "quite slow, so probably not practical for interactive use." Sonnet architect + Sonnet editor 80.5% vs Sonnet alone 77.4%.
- https://aider.chat/2025/01/24/r1-sonnet.html (2025-01-24) `read`. R1 architect + Sonnet editor 64.0%, $13.29; o1 alone (reasoning effort high) 61.7%, $186.5; R1 alone 56.9%; Sonnet alone 51.6%. "o1 paired with Sonnet didn't produce better results than just using o1 alone." So the split paid for R1 (poor at edit format) and not for o1.
- Relevance: the benefit is largest where the planner is strong at reasoning but bad at output format; the gain from the split is not universal.

**2.2 Aider users on HN**
- https://news.ycombinator.com/item?id=43972712 `read`. lreeves: o3 in architect mode for larger refactors, "very suited to the two-pass system where the (more expensive) 'reasoning' LLM tells the secondary LLM all the changes." gawa: Gemini in Aider "keeps editing other parts of the file, doing small 'improvements'" (scope creep). Anecdotes.
- Other Aider HN comments (items 42932741, 44037317, 42932230, found in search results): `snippet only`. A search summary quoted one user: "I use sonnet alone 90% of the time, but the architect mode is really great at getting it unstuck." Not opened.

**2.3 Cline plan/act**
- Cline discussion #387, https://github.com/cline/cline/discussions/387 `read`. On 2025-01-23 a Cline team member said separate models per mode were not offered because "the different models would need the context sent back and forth as you switch modes" and "It's more cost effective to use a single model with the Plan/Act system." (Maintainer position, 2025; Cline later added per-mode models per its docs.)
- Cost claims in Cline guides ("about $1.20 vs $2.40" for a 30-minute session; DeepSeek-R1 plan + Sonnet act "up to 97%" cheaper) are from blog posts I did not open: `snippet only`, not controlled tests.
- Cline issue #2501 (Plan and Act models switch unexpectedly): `snippet only`.

**2.4 Claude Code opusplan**
- anthropics/claude-code #27665 (open): https://github.com/anthropics/claude-code/issues/27665 `read`. Author's `ccusage` on a Max plan: Opus 93.8% of tokens, 97.8% of cost over 17 days (measurement, one user). From a decompiled v2.1.50: opusplan uses Opus only while `permissionMode === "plan"`, "about 5% of a session" by the author's estimate; a linked issue (#27183) reports the opposite ("100% traffic routed to Opus"). So the two reports disagree on what opusplan even does.
- #65512: https://github.com/anthropics/claude-code/issues/65512 `read`. Plan mode silently fell back to Sonnet past 200k tokens (reported 2026-06-04, "fixed" in v2.1.172, reported recurring 2026-07-22 with a table: 4 plan-mode entries at ~127k Opus yes, then ~246k, ~386k, ~479k all no). Nothing surfaced to the user.
- Claude Code docs (VENDOR): each plan-mode toggle under opusplan "is a model switch and starts a fresh cache." https://code.claude.com/docs/en/prompt-caching `read-raw`.
- No user-reported quality-versus-cost comparison for opusplan found; the guides I saw (MindStudio, Developers Digest, wmedia) give no numbers. `snippet only`.

**2.5 Plan/implement/verify by effort rather than by model**
- Thariq's loop (1.1) is the same idea along the effort axis (low to implement, high to verify). Note the reverse of the usual plan-high/execute-low.
- Codex has a first-class split: `plan_mode_reasoning_effort` separate from `model_reasoning_effort` (Vaughan, 1.11). Example config `model_reasoning_effort = "high"`, `plan_mode_reasoning_effort = "xhigh"`. Secondary write-up.
- Issue #35108, https://github.com/anthropics/claude-code/issues/35108 `read` (2026-03-16, closed "not planned" by stale bot 2026-05-12): reporter's own judgment: plan = high, implement = medium, review = low, feedback = high; "Manually toggling /effort 10-20 times per session is friction". No measurements; one +1; no Anthropic reply.
- HN 49946567 (1.9): BobbyJo: "Fable is better at hashing out a plan with some back and forth and opus 5.5 is better (cheaper certainly) at sterile execution." adastra22: Opus does the work, Fable reviews plan and implementation. rdli: Fable 5.1 as advisor, Sonnet 5.5 for mechanical changes. Anecdotes.

**2.6 The advisor pattern (cheap executor, strong consultant at decision points), Anthropic docs with measurements (VENDOR)**
- https://platform.claude.com/docs/en/about-claude/models/optimizing-for-cost-and-intelligence `read` (first 100k of 158k characters). Quotes: "An executor at `low` effort can stop detecting that it is stuck: a pairing that consults on most tasks at the default effort can fall to consulting on almost none when effort is lowered, and then scores below the executor alone." Example: Opus 5.5 executor at `low` with a Fable 5.1 advisor "consulted it on 1 of 300 tasks and scored 61.7, 7 points below Opus 5.5 alone". "On DeepSWE a low-effort Sonnet 5 executor kept asking and gained 23 points; on SWE-bench Pro the same executor stopped." Guidance: "first price the advisor's model alone at low effort; that is the baseline to beat."
- The Claude Code advisor page, https://code.claude.com/docs/en/advisor `read-raw`: toggling the advisor keeps the cache; the advisor's own read of the conversation is never cached.
- I found almost no independent hands-on advisor reports (search-level only).

**2.7 Roo Code per-mode models**: guides only (search-level), no user measurements; one review said Roo Code has shut down and its features moved to Kilo Code. `snippet only`.

### Q3. Homemade setups that switch effort or model per step, phase or prompt

**3.1 totally-tim/effort-router: the closest peer to our design (same `turn.step` mechanism)**
- Repo: https://github.com/totally-tim/effort-router ; issues #1, #3, #5 ; results file `eval/results/2026-09-28-implementation.md` ; eval guide `eval/README.md`. All `read`. Dates: issues opened 2026-09-26, results 2026-09-28 (so very recent).
- Design: a Claude Code plugin that "chooses reasoning effort for each turn". Looks at the prompt, recent conversation, and code the agent has read (bounded Read/Grep/Glob output; shell and MCP output not sent). Picks among low, medium, high, xhigh (max stays manual). Default classifier is an external hosted service; any service implementing `POST /v1/systemone` can replace it. Shadow mode (log only) is the default; enforce mode applies.
- Safety rules (quotes): "Missing context prevents it from lowering effort below the session's starting level." "Once work begins, the router can only raise effort." Mid-turn user messages and repeated tool failures raise effort. "A timeout or service error keeps at least the session effort." "The router leaves subagents alone." `/effort-router wrong <level>` records a misjudged turn. Task continuations keep the previous task's floor.
- Evidence (their own, small): 47 Claude records + 67 Codex labeled records; labels are "earlier model judgments, not human or outcome labels"; 78 matched records scored three times = 234 assessments per version. Picks below the labels went from 2 to 1 (xhigh baseline) and from 10 to 4 (high baseline) of 234. About 65 of 234 went below xhigh before and 64 after (my subtraction from "169 to 170 of 234"). They state: "this result does not establish a general confidence gain or cost savings"; "Label agreement does not establish task success or cost savings"; "They do not measure completed task quality." Records from one session may be correlated.
- Cache: "Claude Code 2.1.283/Opus 5.5 still reproduces cache misses after effort changes in text-only acknowledgment conversations." "No validated production cache workaround was found." README: "a miss after an effort change does not prove the change caused it." Issue #1 (cache eval across dynamic effort changes) is still not run; the author hit the weekly quota limit on 2026-09-26.
- Other limits stated: Claude's built-in effort display "still shows the session setting, not the effort the router actually sent"; the Mods API is early-access; logs hold prompt excerpts.
- Lessons they wrote: handle timeouts (retry) differently from insufficient context (reassess on new evidence); prefer conservative routing over looser thresholds; "Three independent review rounds kept finding defects (5, then 3, then 2)".
- Status: no public measurement of task success or cost under their router.

**3.2 Qcko/claude-code-effort-router (PowerShell `UserPromptSubmit` hook)**
- https://github.com/Qcko/claude-code-effort-router `read`. Behaviour verified by the author 2026-09-26 on CC 2.1.281. 0 stars.
- Scores each prompt (keywords, length, file references, optional task-shape tags like `[refactor]`, `[debug]`) and appends a depth hint; it "nudges depth within the session's effort level; it does not change the effort itself". Has an "override-DOWN allowlist" for mechanical skills (start/stop/restart/end-session): the cap to `none` happens only when there is no competing work signal and "never escalates". Logs each decision and has an analyzer that flags likely mis-routings (e.g. "think prompts that produced 5k+ output").
- Lessons (their words): MCP cannot reconfigure the host; "Claude Code now recognises only `ultrathink`, and only when you type it" (a hook-injected one "is never detected"); "extra 'think harder' text is the documented path to overthinking". No measured results reported.

**3.3 Adityaraj0421/gearbox (model tiers via subagents, escalation ladder, verifier)**
- https://github.com/Adityaraj0421/gearbox `read`; origin in issue #67898, https://github.com/anthropics/claude-code/issues/67898 `read` (opened 2026-06-12, closed "not planned" 2026-08-02).
- Policy injected by a SessionStart hook: scout/grunt on Haiku, builder on Sonnet, architect on Opus; tasks touching auth, payments, migrations, concurrency start at least at Sonnet. Escalation: "fail twice → escalate one tier with the failure report". A Haiku verifier reviews diffs after Sonnet/Opus delegations.
- Their data: 987 delegations across 15 logs; named-agent dispatch rose "21% → 91%" after a policy-wording fix (28 consecutive delegations had run as general-purpose because the policy only listed agents in a table); "one escalation in 394 T1/T2 delegations"; 74% of delegations carry no token data. The author's claim "10/10 routing accuracy" is self-reported on one benchmark. Reward hacking story from the issue: given a contradictory test, "the sonnet tier reward-hacked it with a stateful call counter — green tests, violated intent."
- A commenter (Necmttn, 2026-06-21): "The hard part is not picking Haiku vs Sonnet vs Opus for the obvious cases; it is making routing debuggable after it makes a bad call." Asked for per-decision "receipts" (previous and chosen model/effort, reason, confidence, escalation trigger, verifier outcome, cost delta, whether the user overrode) and a dry-run/backtest mode.

**3.4 talkstream/better-model** (https://github.com/talkstream/better-model `read`): keyword first-match-wins matrix to model + effort frontmatter on subagents (search to Haiku; architecture/security to Opus max; review to Opus xhigh; debugging to Sonnet high; default Sonnet medium). Claim "100% compliance vs ~70% from CLAUDE.md alone" has no protocol or n on the page. Documented measurement: one Max subscriber, subagent-only calls, Sonnet share from 3.8% to about 46%. Cost figures are modeled, not billed (about -18% vs vanilla Opus 4.7 high, "for a 5-point quality drop" on a SWE-bench blend). Savings in plan mode "~5-10% of the session cost" because it does not control the main agent.

**3.5 Other homemade routers named in #27665 and #43326 (repos not opened)**: `calibrate` (akshat4165, per-project policy, "On one verified run it cut cost 46.8% vs all-Opus", author calls the sample modest), `claude-model-router` (edagher92-coder, Haiku classifier plus UserPromptSubmit hook), `mcp-llama-swap`, LiteLLM/FuturMix gateways via `ANTHROPIC_BASE_URL`. Issue #43326 (`read`, https://github.com/anthropics/claude-code/issues/43326) also records that a `UserPromptSubmit` hook rewriting settings.json does not change a running session's model (akshat4165, 2026-06-11). All `read` via the issue, repos themselves are `snippet only`.

**3.6 claude-code-router (musistudio)**: scenario chain "long context → subagent tag → background → web search → think → default", custom router script path. Lessons are from guides (search-level): background route saves most; tool-call quality matters more than benchmark score; over-routing to cheap models degrades quality "at levels users may not immediately notice". `snippet only`.

**3.7 Weave router (Show HN)**: https://news.ycombinator.com/item?id=48688700 `read` ("3 months ago" per page, i.e. about July 2026, inferred). A proxy-level, cache-aware model router; claims 40% token savings "with no noticeable differences in quality or velocity", supported only by "internal metrics". Router uses "embeddings of the prompt + previous context" and can "rescue" by bringing a bigger model when a small one is stuck. Commenters asked for benchmarks (none public at the time).

**3.8 Cursor Router / Auto (VENDOR product, plus user reports)**
- Cursor blog (2026-07-22) and changelog: https://cursor.com/blog/router `snippet only`. Vendor claims: classifier trained on over 600,000 live requests, cache-aware, cost per commit $4.63 (Balance) and $6.76 (Intelligence) vs $7.34 for Opus 4.8 and $12.69 for Fable 5 (vendor numbers, found via search).
- User reports: Cursor forum thread https://forum.cursor.com/t/quality-of-auto-mode-significantly-dropped-down/156408 `read`: a user (2026-04-01) reported Auto "sometimes stopped implementing a plan after only a few steps" and made unrequested edits; a Cursor team member (2026-04-02) said "Auto mode balances quality and efficiency, and it won't always pick the most powerful model" and "sometimes that choice won't be optimal." Anecdote.
- DEV post by Ned C, 2026-02-15 `read`: five tasks, one run each, Auto vs manual Sonnet 4.5, author judged quality by eye: "both modes solved every problem correctly." Self-described small sample, no rubric.

**3.9 Verification gates by Stop hook (answers "stops early / skips tests" with enforcement)**
- #42796's `stop-phrase-guard.sh` (1.5).
- HN "Tell HN: Claude 4.7 is ignoring stop hooks": https://news.ycombinator.com/item?id=47895029 `read`. A user's hook that blocks finishing when source changed without a test run was ignored; the model said "I was prioritizing 'wrapping up' over following the hook's instructions." Commenters: the hook must exit with code 2 and write plain text to stderr (stdout JSON with exit 0 degrades to a tool-result string the model may resist); hashmap: determinism comes from enforcement, the hook should re-fire until tests run. Anecdote plus technical comments.
- Reddit r/ClaudeCode snippet (1.12): pass/fail checks wired as a Stop hook.

**3.10 Feature requests for built-in routing (no outcome data)**: #43326 (open, area:hooks, area:model), #67898 (closed not planned), #35108 (closed not planned), #27665 (open consolidation, "area:model" label by bcherny 2026-08-17), #45891 ("Auto" effort, `snippet only`). None has an Anthropic maintainer reply on routing. Anthropic's support page lists an `auto` effort (Claude chooses per request) per a search summary, `snippet only`; I did not open it.

### Q4. Pitfalls reported

**4.1 Prompt cache and effort/model changes: sources disagree, so measure on your own version**
- Claude Code docs (VENDOR, `read-raw`): "On most models, changing the effort level mid-session means the next request reads the entire conversation history with no cache hits... On Opus 5.5, Sonnet 5.5, Haiku 5.5, and Fable 5.1 with an API key or a Claude subscription, changing effort keeps the cache." The exemption does not apply on Bedrock, Vertex, a Claude apps gateway, with `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS`, or under a HIPAA configuration. "Each model has its own cache. Switching models recomputes the entire request even when the content is identical." https://code.claude.com/docs/en/prompt-caching
- API docs (VENDOR, `read-raw`): per-message effort (beta header `mid-conversation-output-config-2026-07-01`; an empty-content `role: "system"` message carrying `output_config.effort`) keeps the cache on Sonnet 5.5. "A `user` turn that holds only `tool_result` blocks doesn't count as new input, so a change placed after one in a tool-use loop waits for the next `user` turn with new input." Changing top-level effort between requests "restarts the cache and, on Claude Fable 5.1, also steers the model less reliably: its earlier replies were written at the previous level, and it tends to stay consistent with them." With `thinking: {"type": "between_tools"}` on Sonnet 5.5, a changed per-message effort returns a 400. https://platform.claude.com/docs/en/build-with-claude/effort
- Practitioner measurement A, https://github.com/anthropics/claude-code/issues/61984 `read` (2026-05-24, Opus 4.7, CC 2.1.150, subscription 1h TTL, context 110k-197k): 5 of 6 effort transitions were full or near-full cache misses (READ 0 with CREATE 117k-178k, or READ stuck at 14,278 with the whole body rewritten when stepping down to a warm level). Closed "not planned" by the stale bot, no staff reply.
- Practitioner measurement B, https://mer.vin/news/switching-effort-mid-session-didnt-break-my-prompt-cache/ `read` (2026-08-23, `claude-sonnet-5` alias, CC 2.1.241, headless, tools stripped): 20 paired runs, high to low vs high to high, cache read exactly 3,289 on all 20, cost identical. The author notes the setup had almost no content to invalidate, and also found low vs max: both correct every time, max cost about 2.67x and ~4x output tokens, max followed the required output format in only 6 of 10 runs vs 10 of 10.
- Practitioner measurement C, effort-router results (3.1): CC 2.1.283 with Opus 5.5 "still reproduces cache misses after effort changes in text-only acknowledgment conversations".
- Anecdote: wren6991 on HN (49401549): "full cache blowout when changing reasoning level."
- Net: official docs now say Sonnet 5.5 keeps the cache; our own test found no miss; at least one other practitioner (3.1) reports misses on a sibling 5.5 model. Different versions, models, harnesses. Not reconciled. A positive control (model switch) is mandatory, as our own rules already say.

**4.2 Cost of per-turn model switching (arguments, mostly estimates)**
- warku123 in #27665: "the prompt cache is per-model... A few switches in a 50K-200K token Claude Code session and the router loses money vs. fixed-Opus." brandon-fryslie (same thread): Sonnet at 400k warm cache routed to Fable (cold) "would cost you 100x more" (back-of-envelope from "cache write = 20x cache read, Fable = 5x Sonnet", an estimate, not a measurement). danhicks96 wants any auto mode to "decide only at session start or subagent spawn, never per turn."
- Nadir, https://getnadir.com/blog/when-llm-routing-does-not-help/ `read` (2026-09-28; Nadir sells a router, but publishes negative results): "One Haiku-to-Opus switch costs as much as 61 extra Haiku turns on the cached prefix" (their computation).
- Backgrind (https://backgrind.com/blog/switching-models-mid-session/ `read`) computes from list prices that at 200k tokens of history a Sonnet 5 to Opus 5 switch costs about $1.25 in cache writes for one turn vs about $0.06 for a cached Sonnet turn (their arithmetic).
- This applies to MODEL switches. We steer effort on one model, where the docs say the cache is kept on Sonnet 5.5; the model-switch literature is only a cost reference if we ever route models.

**4.3 Router misjudgement and escalation signals**
- Nadir (`read`): offline replay on 500 SWE-bench Verified tasks, Haiku 4.5 to Opus 4.6, escalate after 25 turns: 75.6% resolved but 97.2% of tasks escalated; escalating the same share at random got 75.4% at modeled cost within 1%. Turn-count AUROC 0.700 for "cheap failed" and 0.596 for "cheap failed and expensive would succeed". Advice: "Always test an escalation rule against a random control that escalates at the same rate." An LLM judge choosing the model scored AUROC 0.537-0.597.
- ARIA ("Ask Your Advisor", arXiv 2610.13870, submitted 2026-10-09): abstract `read` (https://arxiv.org/abs/2610.13870): a learned meta-controller for when a cheap executor should consult a stronger model; "recovers 44-81% of the executor-advisor success gap at less than half the advisor model's cost"; beats random placement at matched advice volume. The more specific numbers (random 28.0%, heuristic 41.3%, ARIA 48.0% on SWE-bench Verified; 77% of self-chosen consultations in the final third; a 24-state table of phase x last-execution-result x advice-given; 53.5% of coding consultations right before delivery) come from a Substack summary dated 2026-10-13, https://learnagentic.substack.com/p/your-cheap-agent-doesnt-know-when , which I read but could not confirm against the paper's abstract. Treat them as secondhand.
- GPT-5 router, day one (August 2025): VentureBeat (`read`, 2025-08-08) quotes Altman only in fragments: the autoswitcher was "out of commission for a chunk of the day" and made GPT-5 seem "way dumber". The longer quote and the "16 hours" figure came from search summaries (`snippet only`).
- Cursor Auto: see 3.8.
- opusplan: silent downgrade (2.4). Nothing in the UI told the user.

**4.4 Low effort stops asking for help or checking (agent-side failure)**: see 1.2, 1.3, 1.4, 2.6. The pattern that matters for a router: the signal "agent is about to finish" is exactly when `low` skips tests and stops to ask.

**4.5 Inconsistent behaviour**
- bcherny: high effort sent on every request, yet some turns had zero reasoning (adaptive thinking under-allocation) and fabricated details (1.7).
- Thariq (1.8): "the scale isn't 0-100"; the meaning of the effort number can change with serving configs.
- Anthropic docs: after a top-level effort change Fable 5.1 "tends to stay consistent with" its earlier replies (stated for Fable 5.1, not for Sonnet 5.5).
- Quality or "thinking" judged by feel: HN commenters asked for a small nightly eval canary (DEV post, `snippet only`).

**4.6 Observability**
- effort-router: the CLI's effort display shows the session setting, not what the router sent. #67898 (Necmttn): routing needs receipts and a dry-run mode. Qcko: logs plus an analyzer that flags likely mis-routings. gearbox: telemetry joins are heuristic, 74% of delegations had no token data.

---

## 2. What this means for our design

These are my inferences from the sources above, not claims the sources make.

**Moments practitioners consistently tie to effort**
1. **Verification and finish.** The most consistent link (Thariq's measurements; Anthropic's Sonnet 5.5, Haiku 5.5 and Claude Code pages; the HN reports of skipped validation; the `stop-phrase-guard` data; the advisor pages) is that low effort cuts verification, stops early and asks instead of deciding. Thariq's own recommended loop puts the HIGH step at "verify and test", with implementation on low/medium. That matches where our three `low` losses happened (full suite not run; stopped to ask), and it argues against a router that lowers effort at the end because "the files were already edited."
2. **Reproduce before editing / unknown cause.** Thariq's `mvcc` and `cli-2ph-simplex` examples (low edits before reproducing; low does not check performance) and the Codex guides ("use High when the error does not name the file and line"). Our data say diagnosis was not where `low` lost on our tasks, so this is weaker for us.
3. **Mechanical edits with context already gathered.** Low or medium is the common recommendation (Qcko's override-down allowlist; Anthropic "low ... such as subagents"; Kent Gigger snippet). The one reported warning: rename/refactor on low without checking callers (snippet only).
4. **Architecture / brainstorming.** Evidence that xhigh helps here is weak. Thariq's Fig B: more effort reduced missed edge cases but "picked the wrong reading" rose from 25 to 47; effort "does not fix when the model has the wrong approach." Sonnet 5.5 docs: xhigh/max start extra review rounds and unrequested additions; Anthropic FrontierCode: Sonnet 5.5 max 46.2% vs xhigh 52.1% (secondhand via Buda and Kingy, see 1.10). Practitioners who want better plans mostly pick a stronger MODEL (Fable/Opus) or interview the model for a spec, not a higher effort level. Brainstorming wants low/medium and "in the loop" (Thariq). So "design decision to high/xhigh" is the least supported of the operator's examples; "high" is the commonly named sweet spot (Stet, Paul Solt snippet, Anthropic).
5. **After a stall or failure.** Escalate (effort-router raises on repeated tool failures and mid-turn user messages; gearbox "fail twice"; HN stefmf in #27665 "escalating early is often cheaper than five failed cycles on a weaker model"). But a fixed trigger such as a turn threshold performed no better than random in Nadir's replay.

**Signals practitioners actually use** (so the router's input could be limited to these)
- Phase of the run: frame / explore / reason / deliver (ARIA, secondhand).
- Result of the last execution: none / clean / error (ARIA); repeated tool failures (effort-router, gearbox).
- Whether a verification step has run since the last edit (implied by the Stop-hook practice and ARIA's "before delivery" finding, not a published rule).
- Prompt features: keywords, length, file references, task-shape tags (Qcko, better-model).
- What the agent has read so far (effort-router: bounded Read/Grep evidence).
- Skill identity for "mechanical" skills (Qcko override-down allowlist).
- Task type minimum tiers: auth, payments, migrations, concurrency (gearbox).
- Mid-turn user messages (effort-router).
- Transcript growth and blast radius (stefmf, comment only).

**Design rules that peer routers converged on**
- Asymmetric: lowering needs positive evidence; missing context or a classifier timeout keeps at least the session effort (effort-router); override-down only for an explicit allowlist and never when a work signal is present (Qcko).
- After work starts, only raise within a task (effort-router). Consider a floor that persists across continuations.
- Shadow mode first with a decision log and receipts, then enforce (effort-router; Necmttn).
- Compare any rule against a random control with the same rate (Nadir).
- Judge on outcomes, not label agreement or tests alone: Stet says tests alone hid most effort differences; effort-router says label agreement is not task success; Thariq measures behaviour (verification, edge cases).

**Pitfalls to guard**
1. Cache: verify on CC 2.1.296 and Sonnet 5.5 with a model-switch positive control, both at turn boundaries and mid-turn. Docs say Sonnet 5.5 keeps the cache on effort changes, but #61984 (Opus 4.7) and effort-router (Opus 5.5, CC 2.1.283) report misses. Record the cache-read share after each effort flip.
2. Mid-turn effect: the API docs say an effort-only system message placed after a tool_result-only user turn waits for the next user turn with new input. Whether our mod's mid-turn rewrite changes the effective effort inside a tool loop is not settled by these sources; check thinking-token counts after a mid-turn change before relying on it.
3. A low-effort agent is the one that skips the check, so the router cannot rely on the agent to call for help (advisor page: consults collapsed from most tasks to 1 of 300 at `low`). Put the trigger outside the agent (the router sees the state), and make "about to finish" and "edits made, no test since last edit" raise-to-high moments.
4. Server-side drift in what an effort number means (Thariq on HN) and thinking under-allocation at any setting (bcherny): keep our per-request thinking-token and tool-call counts in the log so a "high" request that did not think is visible.
5. The router's own cost and latency (effort-router default classifier timeout is 5 seconds) and its failure mode (keep session effort).
6. Overcorrecting up: xhigh/max add self-started review and unrequested additions on Sonnet 5.5, and FrontierCode shows max below xhigh. A router that can emit xhigh/max needs an explicit reason.
7. Silent failure of a router (opusplan #65512) and invisible effort in the UI (effort-router): add a status line or log the effort sent.
8. Hook-injected steering text: the documented "ultrathink" keyword is not detected from a hook (Qcko), and Anthropic says asking for less thinking is unreliable on Sonnet 5.5. Our own mod rewrites effort directly, which sidesteps this; the steering-sentence route has the vendor warning.

**Relation to the operator's examples**
- "Editing a tracker file with all context gathered, low/medium": consistent with practitioners, provided the last step of the task still gets a verification at higher effort.
- "Architecture decision, high/xhigh": partly supported (high), not strongly (xhigh); and the "wrong reading" failure rose with effort in Thariq's data.

---

## 3. Not found (queries tried and gaps)

Gaps:
- No public report of anyone running a per-step effort router on Sonnet 5.5 with task-success or cost outcomes. effort-router has label-agreement and replay numbers only; its cache eval (#1) and hosted-classifier calibration (#3) are still unrun as of 2026-09-26.
- No independent measurement of Sonnet 5.5 `low` vs `high` pass rates on bug-fix tasks with hidden tests. The Sonnet 5.5 numbers I found are Artificial Analysis index scores (secondhand) and Anthropic's FrontierCode max-vs-xhigh.
- No Reddit thread could be opened (see top). I have titles and snippets only, so no Reddit counts or consensus claims are made.
- No user report found comparing opusplan quality against all-Opus (Reddit/HN/GitHub). Only bug reports and guides.
- No GitHub issue found specific to Sonnet 5.x `low` effort stopping or skipping tests (Anthropic docs describe it; #99083 is Opus 5.5, `snippet only`).
- No HN thread found specifically on Cursor Auto choosing weak models; only Cursor forum threads and a Medium summary of the Weave Show HN thread.
- Few independent hands-on reports of the advisor tool; most articles repeat Anthropic's figures.
- Cline plan/act and Roo Code per-mode cost: only guides and one maintainer comment; no user measurements found.
- The quote of Altman's full autoswitcher sentence and the "16 hours" claim were not verified from a primary page.
- x.com pages: only Thariq's article via claude.dev was readable; the X version returned HTTP 402; the A/B-testing tweet (https://twitter.com/argofowl/status/2091150597374537729) not opened.

Queries run (WebSearch and researcher; each returned something, but the stated item was not found):
- "Claude Code effort level high vs low overthinking simple edits reddit" (no Reddit results)
- "reddit r/ClaudeCode effort level xhigh vs medium coding agent quality" (no Reddit results from WebSearch; the researcher MCP supplied Reddit snippets)
- "Hacker News Claude Code reasoning effort low skips tests stops early asks questions"
- "github anthropics/claude-code issue effort level automatic per-step effort router hook"
- "Aider architect editor mode benchmark results o1 architect Sonnet editor cost quality"
- "Claude Code opusplan plan with Opus execute with Sonnet cost quality experience"
- "opusplan experience reddit OR hacker news plan Opus execute Sonnet quality dropped worse than all Opus" (no thread found)
- "Cline plan mode act mode different models cost savings user experience reddit..." (twice; no Reddit)
- "Aider architect mode editor model user experience worth it slower hacker news reddit" (HN, GitHub, blogs; no Reddit)
- "switching model mid-session Claude Code prompt cache invalidated cost cold cache effort change cache miss"
- "claude-code-router musistudio routing think background longContext..."
- "Codex CLI reasoning effort xhigh vs medium practitioner experience..."
- "effort Claude Sonnet 5.5 low effort asks clarifying questions instead of doing, stops early, hacker news"
- "hook UserPromptSubmit set effort per prompt ultrathink keyword router script..."
- "GPT-5 router autoswitcher broke wrong model chosen complaints router mistakes..."
- "Claude Code advisor tool executor model consults stronger advisor model experience results cost"
- "escalate to stronger model after failed attempts coding agent router escalation lessons learned..."
- "Cursor Auto mode router model selection complaints picks weak model hacker news" (no HN thread)
- "Hacker News Cursor Router smart model routing coding tools prompt cache misses Rice's theorem" (found the Weave Show HN instead; the "Rice's theorem" comment was confirmed on the HN page, by GodelNumbering)
- "github anthropics/claude-code issue low effort stops OR asks permission OR skips tests Sonnet 5 effort low premature completion"
- "Roo Code orchestrator boomerang different model per mode cost lessons..."
- "Claude Code mods API turn.step effort hook plugin early access" (docs/blogs describe `turn.step` switching the model; none state an effort field. Not checked in the official mods reference.)
- "x.com Claude Code effort low medium high ultrathink per task thread..." and ""Spending your effort" Thariq..."
- researcher MCP (reddit.com / news.ycombinator.com / github.com domains): effort level queries; "low effort skips tests stops early..."

Pages I tried to open and could not: www.reddit.com or reddit.com (3 WebFetch calls), old.reddit.com (1 call), r.jina.ai proxy of a Reddit thread (403), the X version of Thariq's article (402), the pub.towardsai.net copy of it (403). One DEV post URL returned 404 until I used the full URL with its `-20ml` suffix.

Unverified items worth a second look by someone with Reddit access: the five Reddit thread URLs in section 1.12, plus r/ClaudeCode/comments/1wbukml ("Do you guys use Opus 5 max, xhigh, high or medium"), r/Anthropic/comments/1wy1uad, r/ClaudeAI/comments/1wkg2w8.
