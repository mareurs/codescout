# Jev + Claude Code = The Cheapest Agentic Coding Loop Yet

- **Source:** https://www.youtube.com/watch?v=ScvXFi4MUSc
- **Creator:** Ray Amjad
- **Length / published:** 27:28, 2026-09-18
- **Form:** summary + short attributed quotes (not a full transcript — see `docs/media/README.md`)

## Summary

Overview of **Jev** (TypeSafe.ai), a small/fast "classifier" model positioned as a *system-one*
complement to slow, deliberate *system-two* coding agents (Claude Opus, GPT-family "Astra"),
borrowing the Kahneman framing from *Thinking, Fast and Slow*.

- **What Jev is.** Doesn't generate free text — given a prompt, some data, and a fixed set of
  possible answers, it returns calibrated probabilities over those answers in ~100–300ms. Three
  primitives: `bool` (true/false with a rubric), `score` (0–10 against a spectrum), `choice` (up
  to 255 options). Cheap enough to run continuously in the background.
- **System one / system two pairing.** The pitch: let a fast, cheap classifier make constant
  small judgment calls, while an expensive deliberate model (Opus/Astra) sets strategy and only
  gets invoked for the decisions that matter. Demoed live by having Jev play Minecraft
  move-to-move while GPT "Astra" set higher-level goals and periodically reviewed progress.
- **Claude Code-relevant use cases** (the reason this is worth having in this repo):
  - **Skill routing.** Cited from the Jev cookbook: with 182 skills installed, an agent alone
    loads the wrong skill 17% of the time; with Jev pre-filtering, that drops to 7.3% (tested with
    Haiku 4.5) — pitched as a way to cut ~10k tokens of skill-description overhead from context.
  - **Cheap verification loops.** A Jev-powered browser-use agent found a specific flight search
    result in ~7 seconds for a fraction of a cent, framed as a pattern for validating hundreds of
    user flows per PR cheaply — parallels codescout's own "verification before completion" framing,
    just implemented as a classifier instead of a full agent.
  - **Comment-quality / linting.** Demoed asking Jev to flag code comments that are accurate but
    not useful (e.g. "multiply by two" on a line that already says `* 2`): 150 comments scored in
    9.3 seconds for ~1 cent; full-codebase estimate ~$0.57 for ~1,700 flagged comments.
  - **Semantic "AI linters."** Framed as a replacement for brittle static-analysis rules — e.g.
    "does this log statement contain a secret or PII" as a natural-language rubric instead of a
    regex-based linter rule.
  - **Code-smell / code-review triage.** A full-codebase code-smell pass was quoted at 28M input
    tokens for $1.19; a Jev-based code-review layer is pitched as routing ~100 cheap yes/no/score
    questions per diff and only escalating the highest-severity findings to a full coding agent —
    claimed ~10x reduction in review token cost. A cited Sentry engineer's use of a similar small
    classifier on a security pipeline claims 5x cost/latency improvement over other small models
    at comparable accuracy.
- **Caveat:** the video is partly a promotional vehicle for the creator's paid "Agents Engineer
  Cohort" course — multiple mid-video plugs. The technical claims (skill-routing accuracy, cost
  figures) are sourced from Jev's own cookbook/marketing and third-party tweets, not independently
  verified here.

## Notable quotes

> "One way of thinking about it is that Jev does not generate any text... you can imagine it as a
> really smart switch statement."
> — Ray Amjad, ~1:01, describing the core mechanic

> "They find that with the agent alone, it would load the wrong skill 17% of the time, and with
> Jev suggesting it, it would load the wrong skill 7.3% of the time."
> — ~13:23, citing the Jev cookbook's skill-routing benchmark

> "We managed to do 150 comments in 9.3 seconds for about 1 cent."
> — ~19:02, live demo of comment-quality scoring

> "Your output tokens are 10% of the cost... if I can reduce the amount of tokens for my code
> review by 10 times, that is like a huge win on cost alone."
> — ~23:09

> "These are the results of using Jev on one of our security pipelines... this model does it over
> five times cheaper, faster, and whilst maintaining a high accuracy."
> — ~25:14, quoting a Sentry engineer's tweet

## Why it might be worth citing

The "cheap classifier pre-filters, expensive agent only handles escalations" pattern is directly
relevant to codescout's own progressive-disclosure and verification philosophy — worth a
skeptical look if codescout ever explores tiered/cheap-first classification (e.g. pre-filtering
`grep`/`semantic_search` results, or triaging findings before a full review pass). Treat the
specific benchmark numbers as marketing claims to verify independently, not settled facts.
