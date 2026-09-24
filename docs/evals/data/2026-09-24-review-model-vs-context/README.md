# Review model-vs-context experiment, 2026-09-24 — data

Registration and results: `docs/evals/review-model-vs-context-2026-09-24.md`. Registered in `f446e759` before any call.

| file | what |
|---|---|
| `run_reviews.py` | the runner: one fresh worktree per run at the stimulus commit, the brief, both CLIs on their subscriptions with a clean channel |
| `check_reviews.py` | the per-defect checker: 11 defect definitions, the gate fixtures (the real Codex reviews, each with one defect's paragraph cut), Sonnet 5, majority of 3 |
| `primed-s1.md`, `primed-s2.md` | the author's own summaries given to PRIMED cells, excerpted from the authoring session's messages to the operator before each review |
| `gate.txt`, `gate.json` | checker gate, 22/22 |
| `runs/<stim>-<model>-<context>-<rep>.review.md` | each reviewer's final message, 24 runs |
| `runs/*.brief.txt` | the exact brief each run received |
| `runs/*.meta.json` | exit status, seconds, the model the CLI reported, review length, and whether the checkout was left dirty (none was) |
| `scores.json`, `score.txt` | per-(review, defect) verdicts with their three votes, and the per-cell recall |
| `findings-index.json`, `precision-sample.json` | the 206 numbered findings extracted by regex, and the seeded sample of 10 checked by hand |

**Not included:** the raw CLI stdout/stderr logs (`runs/*.log`). They carry session and account metadata. Nothing in the results rests on them beyond the model name, which each `meta.json` records.

Every file here was scanned for API keys, bearer and OAuth tokens and the operator's email address before commit; there were no hits.
