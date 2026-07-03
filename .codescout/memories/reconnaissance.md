# Reconnaissance — promoted rules (distilled from docs/trackers/reconnaissance-patterns.md)

One-line imperatives promoted at their R-N promote-when thresholds. The ledger
entry holds the full narrative; this file carries only the rule.

- Before editing any tool description or enumerated prompt surface (get_guide
  topics, Iron Laws, tool lists), enumerate and run its budget/count gates
  first: grep `server::tests` for the tool name (300-char description budget)
  and grep the surface's tests for hardcoded counts (`len() == N`) — convert
  those to derive from the canonical const while there. (R-28 + R-37)
