# ECC for codescout — Codex pilot

Two focused workflows adapted from ECC: behavioral spec mining and silent-failure
review. The package is for Codex and expects an existing codescout MCP connection.
It adds no hooks, bundled servers, automatic delegation, model choices, or memory store.

Upstream: [affaan-m/ECC](https://github.com/affaan-m/ECC), pinned to
`d2ef2680594f9a17526a6e1a2e18b3d242c5d17b` (2.2.3).
Source hashes and adaptation notes are in [upstream.json](upstream.json);
the original MIT license is retained in [LICENSE](LICENSE).

## Try it in a separate Codex home

Run from this directory. This creates a separate plugin/cache/config state and
does not install into your normal Codex profile. Codex can still discover user
skills from `~/.agents/skills`; this is profile isolation, not a filesystem sandbox.

```bash
ECC_PILOT_HOME="$(mktemp -d -t ecc-codescout-pilot.XXXXXX)"
env CODEX_HOME="$ECC_PILOT_HOME" codex plugin marketplace add "$PWD"
env CODEX_HOME="$ECC_PILOT_HOME" codex plugin add ecc-codescout@ecc-codescout-pilot
env CODEX_HOME="$ECC_PILOT_HOME" codex mcp add codescout -- codescout start --project /absolute/path/to/target-repo
env CODEX_HOME="$ECC_PILOT_HOME" codex login
env CODEX_HOME="$ECC_PILOT_HOME" codex --cd /absolute/path/to/target-repo
```

`codescout` must be on PATH; otherwise use your existing server's absolute binary
path in the MCP add command. Login uses the separate profile. Existing project
instructions and any project configuration still apply in the target repository.
The target must have its codescout languages configured before relying on LSP
navigation: an uninitialized scratch project with an empty language list hides
references/call_graph even if a Python source file is present. The fixture diagnosis
in evidence/failure-review.md records the controlled check.

In the new session:

```text
Use $ecc-codescout:ecc-spec-miner to extract a bounded observed-behavior spec of <capability>.
Use $ecc-codescout:ecc-silent-failure-review to review <scope> for hidden failures. Keep the review read-only.
```

The native marketplace has a separate name, `ecc-codescout-pilot`, and is not
registered in the repository's Claude marketplace. The skill names above are the
namespaced names observed through Codex app-server `skills/list`.

## Workflow boundaries

- Spec mining traces symbols, callers and tests, then saves an observed-behavior
  draft through codescout librarian when requested. It names incomplete scope
  and distinguishes code inspection from test execution.
- Failure review separates confirmed defects from concerns that still need
  contract or caller evidence. A review request does not authorize edits.
- Applied mutation probes belong in an authorized isolated target. Record the
  mutation, actual command, observed outcome, and restoration; a proposed
  mutation is not an executed test.
- Durable facts and ongoing work stay in codescout memory and trackers.
  Existing repository instructions and workflows remain authoritative.

## Validate

Run all pilot tests from this directory:

```bash
python3 -m unittest discover -s tests -p 'test_*.py'
```

The tests use Python's standard library. With `codex` on PATH, package tests
perform a real native install and app-server skill discovery in a temporary
profile. With `codescout` on PATH, navigation tests start the actual server
against a temporary fixture copy. If needed, select its executable explicitly:

```bash
CODESCOUT_BIN=/absolute/path/to/codescout python3 -m unittest discover -s tests -p 'test_*.py'
```

Python caller tracing also requires `pyright-langserver`. Unavailable native
dependencies produce explicit skips; a skipped check does not establish native
compatibility. Routine tests make no model calls and never copy authentication.

The fixture includes a codescout Python configuration and a real `commit_sync`
caller. Navigation checks distinguish an unconfigured negative control from
configured tool advertisement, then assert the returned reference site and
caller graph. This checks navigation, not runtime propagation behavior.

For opt-in authenticated workflow checks:

```bash
python3 tests/live_smoke.py --live --output-dir /tmp/ecc-workflow-results \
  --codescout-binary /absolute/path/to/codescout
```

The output directory must be new. The runner uses the selected `CODEX_HOME`'s
existing file login and model settings in a private temporary profile, removed
after the run. Codescout uses a private catalog, workspace config, artifact
vector store and XDG state directories for the fixture. The temporary Codex
config passes those values explicitly to its MCP child. It runs both skills,
checks actual completed MCP calls and
librarian spec persistence, and retains transcripts and artifact evidence
separately from authentication. These calls consume model usage.

The fixture sources are intentionally mixed examples. The pilot has no benchmark
demonstrating better accuracy, cost, or latency than the existing setup.
See [verification evidence](evidence/verification.md) for observed results.

## Updating and removal

The native plugin installs a cache snapshot. After changing this worktree, use a
fresh temporary Codex home and reinstall to avoid testing an older snapshot.
Review upstream changes against the pinned source hashes; do not copy the full ECC
configuration over this adapter.

To remove the plugin from the pilot profile:

```bash
env CODEX_HOME="$ECC_PILOT_HOME" codex plugin remove ecc-codescout@ecc-codescout-pilot
```

Keep the temporary home while experimenting if its conversations or login are
needed. Removing this plugin does not remove the entire profile.
