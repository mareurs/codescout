---
id: '3cb3b89d2ed359ea'
kind: spec
status: draft
title: Codex publish holds
---

# Codex publish holds

Extend recorded publish holds to native Codex root sessions without requiring a Claude identity. Reuse `session_context.resolve_context`; root identity, explicit-context validation and conflict detection stay authoritative. Child thread identity is metadata.

Claude keeps bare `Session-Id: <sid>` and `refs/holds/<sid>`. Codex writes canonical `Session-Id: codex:<sid>` and `refs/holds/_codex/<sid>`. Qualified `claude-code:<sid>` is an explicit alias for a legacy Claude hold. IDs remain restricted to `[A-Za-z0-9-]+` at the publish boundary. No automatic ref migration or history rewriting. A bare UUID always denotes Claude, even when a Codex caller explicitly releases it; implicit release uses the caller's own harness.

Set, refresh, release, list, pre-commit refusal and push scanning must agree on that mapping. Same UUID across harnesses must remain distinct. Codex liveness/contact is unknown; it must never borrow a matching Claude registry row. Existing Claude message shapes and manual/no-identity behavior remain compatible. Conflicting or unsupported identified context refuses state-changing commands and hooks; list and explicitly named release remain available to an operator without identity. An explicit release of another actor remains an operator decision. Codex push/release use show-ref --exists to distinguish missing refs from failed lookups and refuse failures without claiming absence. Legacy Claude push/release error policy remains unchanged.

Codex trailers are stamped through Git's trailer parser with the existing idempotent replay policy. Qualified tokens pass through foreign-session acknowledgements without conflating harnesses. A hold remains stronger than `CODESCOUT_PUSH_ACK=all`. Branch, tag, raw-SHA and worktree paths retain their existing range rules.

No installed hooks, profiles, global configuration, real holds, shared checkout index or branch are modified. Delivery is an isolated branch/worktree, validated by hermetic integration tests, existing shell discriminations, applied mutation review and the full gate. Manual pushes, unstamped historical commits, unrecorded withholding and skipped hooks remain outside the mechanism.
