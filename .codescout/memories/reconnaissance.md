# Reconnaissance — promoted rules (distilled from docs/trackers/reconnaissance-patterns.md)

One-line imperatives promoted at their R-N promote-when thresholds. The ledger
entry holds the full narrative; this file carries only the rule.

- Before editing any tool description or enumerated prompt surface (get_guide
  topics, Iron Laws, tool lists), enumerate and run its budget/count gates
  first: grep `server::tests` for the tool name (300-char description budget)
  and grep the surface's tests for hardcoded counts (`len() == N`) — convert
  those to derive from the canonical const while there. (R-28 + R-37)

- Never conclude a codescout tool dropped an input, returned nothing, or
  ignored a flag from its **summary** — read the `@ref` buffer. Every buffered
  response carries `buffered_bytes` and names the ref: if that number is larger
  than the summary could account for, the summary is not the result. Checked
  twice against `symbols(path=<dir>, include_body=true)`, whose 43 bodies sat in
  a 56 KB buffer while its line-oriented summary showed none. (R-50)

- A green check proves nothing until you confirm it *can* fail. Before trusting
  it, ask what it would do if the guarded thing were broken; if the answer is
  "the same thing", it is decorative. Three instances in one session, each green
  for months: a single-value jq probe (the trailing CRLF is stripped, so it
  cannot see the defect), a `sleep 5 && touch x` cancellation test (`sleep` does
  not exist under cmd.exe, so it short-circuited and never exercised the kill
  path — hiding a total absence of Windows process-tree kill), and a 134-test
  suite that structurally could not reach `inject_tee`'s path validator.
  (claude-plugins `windows-shell-env-session-log.md` W-2)

- A red gate is not a finding until HEAD has been measured: `git stash`, re-run,
  compare. Attribute by measurement, never by reading the diff and reasoning
  about plausibility. This kept a clippy suggestion from deleting a `return`
  that is load-bearing on Unix (it only looks needless on Windows, where every
  following statement is `cfg(unix)`) and refuted a plausible self-blame for
  three EDR-quarantined test binaries. Corollary for flaky-vs-real: re-run the
  single test in isolation — deterministic 5/5 failure is real, passes-alone /
  fails-under-load is a timing flake. (same log, W-3)

- Shell-pipeline scouts must read exit status AND bytes, not output emptiness.
  A non-zero exit with non-empty output is common (jq with several inputs,
  compilers with `-k`) and defeats `${x:-default}` guards — codescout's own
  `_merge_cache` consumed a partial-success jq document and silently swapped the
  session JSON for a cache file. (claude-plugins reconnaissance-patterns R-4)
