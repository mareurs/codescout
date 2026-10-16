# Plan-limit hazard in subscription `claude -p` trials (built 2026-10-11)

Related: `research/effort-steering` (the study), prompt-engineering OP-11 (the original report, 2026-08-21).
Code: `claude-plugins/effort-steering/spikes/plan-limit.mjs` (pure detector, imports nothing),
wired into `spikes/s5/run.mjs` and `spikes/s4-run.mjs`; guard `spikes/plan-limit.guard.test.mjs`.
Commits b673b9d..f026c11 on claude-plugins `main` (not pushed). Briefs, reports and Opus reviews:
`/home/marius/work/claude/s5-work/briefs/plan-limit-*.md` (scratch, not committed).

## The hazard
A spent subscription does not make `claude -p` fail visibly. The CLI prints a refusal ("You've hit your limit ·
resets 3pm") that a harness grades as a model failure. S5's `xhigh` arms burn the most plan usage, so a hit would
fabricate a loss for exactly the arms under test. `~/.claude-sdd` is a Team plan, so org spend limits apply too.

## Verified in the 2.1.292 and 2.1.296 binaries (read-only grep, binary never run)
- The CLI has its own classifier `wms()`: 12 `startsWith` prefixes + one regex (`Fable|Haiku|Mythos|Opus|Sonnet ...
  requires usage credits.`). The detector mirrors it. Text identical in both versions; only minified names differ
  (`syr`/`Cuo` in 2.1.292, `qTr`/`Wvo` in 2.1.296). Re-extract recipe is in the detector header.
- Typed kinds: `api_error` enum includes `usage_limit_reached` and `model_requires_usage_credits`; the transcript
  record carries `apiError` + `apiErrorParams.rate_limit_info` only when the CLI's classifier matched.
- Wrap-up notes: `isMeta` user records with `usageLimitNote` = `wrap_up` | `release` | `near_limit` (flag-gated,
  default off). The run then ends with a normal-looking final text although the plan cut it short.
- Not refusals: "Approaching your 5-hour usage limit", "Server is temporarily limiting requests (not your usage
  limit)", "Context limit reached", "Fast mode requires usage credits", a 401 "Failed to authenticate". `subtype` is
  "success" even on a 401: never key on it.

## NOT verified (say so wherever it matters)
- The real envelope and transcript of an actual plan-limit hit. None exists on this host. Every plan-limit test
  record is labelled SYNTHETIC. Whether `--output-format json` emits `api_error`, whether `-p` text mode prints the
  refusal on stdout or stderr, whether the transcript persists `usageLimitNote`.
- A refusal that matches no pattern and has no typed `apiError` would show as `api_error` attempts, then
  `infra_exhausted`, then `infra_streak`: loud but misattributed; silent if returned as an ordinary answer.

## Design rules that cost thought
- Fail closed: a false hit stops a resumable batch; a false miss silently corrupts the study. Wide prefixes (`You've
  reached your ...`) can match short agent prose at index 0; accepted, pinned by tests (A2/F7).
- A `plan_limit` row in S5 is neutral: not graded, `final:false`, never retried in the batch, stops the batch, and it
  is NOT counted in `attemptsUsed` or the infra streak (else three plan windows would write `infra_exhausted` and drop
  the task from all arms, R149). A partial run with no printed cost is cost-estimated like `interrupted`.
- A RECURRING false stop blocks the batch (no cap, no override): each resume pays one full run. Not built: an
  accept flag, a preflight check of prompt/hook text. Study prompts and steer phrases were checked: no marker start.
- `batchPreflight` refuses a `cli.version` not in `VERIFIED_CLI_VERSIONS` (['2.1.292','2.1.296']); adding a version is
  a deliberate edit after re-running the grep. S4 has no such gate and never passes the transcript to the detector
  (documented in its header).
- Python/shell launchers (7 probe `.sh`, `dose/run-calibration.py`) are NOT wired; they sit in the guard's UNGUARDED
  map on purpose.
- Run effort-steering suites from the REPO ROOT: `s5/hook-copy.test.mjs` fails with cwd `effort-steering/` (old bug).

## Which account pays for S5 (decided 2026-10-11)

- The user ran `claude setup-token` for `~/.claude` (the personal account, `ailinca.marius@gmail.com`), not for `~/.claude-sdd` (Stefanini org). The user chose the personal token on purpose after being shown both accounts.
- The token is stored at `~/.config/s5/token` (mode 600, directory 700). The token was pasted into a chat session, so it is in a transcript: treat it as exposed and revoke or replace it when the study ends.
- Consequence: plan limits are the personal subscription windows (`five_hour`, `seven_day`), not Stefanini org spend limits. A long batch can stop with `plan_limit` and a reset hint; resume after the reset.
- Not verified: that the token belongs to the personal account (no model call was made).
