---
name: ecc-spec-miner
description: Use when extracting behavioral specifications or invariants from an existing codebase in Codex with codescout, including brownfield modules without reliable specs.
license: MIT
---

# Mine observed behavior with codescout

Adapted from ECC spec-miner; upstream revision and license are in the plugin's
`upstream.json` and `LICENSE`.

Produce a bounded **observed-behavior draft**, not an assertion that the current
implementation is the desired product contract.

## Establish scope

Read the target repository's instructions. Resolve its absolute workspace and
selected capability; when scope is already clear, proceed. Otherwise inspect
entry points and propose a capability boundary before expanding.

Require the existing codescout MCP connection. If required tools are unavailable,
report the missing capability and return a bounded chat draft; do not install a
server or substitute native source-reading tools.

Use `workspace(action="status")` to identify the previous active root. If activation
is necessary, activate the target and restore the previous root when finished.
Pin subsequent calls with `workspace=<absolute target>`.

## Trace evidence

- Known code: `symbols(name=..., include_body=true)`; overview: `symbols(path=...)`.
- Follow use sites with `references` and `call_graph`. Exact strings use `grep`;
  concepts use `semantic_search` when the target index is available.
- Read ordinary docs by heading with `read_file`; managed artifacts use `doc`.
  Run commands through `run_command`, then query its output buffer.
- Read the actual enforcement body and relevant callers before writing a claim.
  A parameter value alone does not establish counting semantics: distinguish
  total attempts from retries by tracing the loop.
- Inspect test assertions. Record execution only after actually running the
  relevant command and observing its result. Static test presence is unexecuted
  evidence, not a passing result.

## Draft shape

Record workspace, observed commit, dirty-state limitations, capability boundary,
inspected symbols and uninspected/deferred paths. For each assertion use:

### Requirement: <observable trigger and outcome>

- **Behavior:** WHEN <verified condition>, THEN <observed outcome>.
- **Enforcement:** actual path and symbol; no invented identifier.
- **Evidence state:** code-inspected; test-inspected/not-run, executed-pass,
  executed-fail, or unavailable. Include the test anchor and command/result when known.
- **Contract status:** observed implementation; intended behavior unresolved unless
  separately supported by an authoritative requirement.
- **Unknowns:** missing callee, uninspected branch or unresolved semantics.

Use `### Invariant:` for an always-enforced property. Omit unknown metadata;
keep unknown behavior in **Open questions**, not as a Requirement or Invariant.
A sample supports only the inspected scope, never whole-module completeness.

## Persist through librarian

A request to extract a spec authorizes saving that draft; a read-only or chat-only
request keeps it in chat. Fetch `get_guide("librarian")` before artifact writes.
Find existing specs with `doc(action="find", kind="spec", ...)`, then read the
matching artifact using `doc(action="get", ...)`.

Follow the target repository's established spec location. If none exists, use
`docs/specs/<capability>-observed.md` and create a draft with
`doc(action="create", kind="spec", title=..., rel_path=..., body=...)`.
Update an existing artifact through `doc(action="update", patch={body_edits: [...]})`;
a partially read section is never a replacement for the entire body.
If persistence is unavailable, return the draft and the precise limitation.

Finish with the saved artifact/path or chat draft, scope covered, evidence state,
and unresolved questions. Durable lessons belong in codescout memory or trackers.
