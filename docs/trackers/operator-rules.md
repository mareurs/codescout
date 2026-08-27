---
id: fa21bfb35684794d
kind: tracker
status: active
title: Operator Rules (OP-N)
tags:
- operator-rules
- engine-5
- ledger
entry_prefix: OP
entry_high_water_OP: 1
---

# Operator Rules (OP-N)

Rules that hold across every project, tool and model for this operator. Compiled into each Claude Code profile's CLAUDE.md by `codescout operator-rules compile`.

Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

## Index

| ID | Binding | Covers | Evidence | Status |
|---|---|---|---|---|
| OP-1 | always | unverified-assertion | measured: conclude-last/b2 0% -> 100% (n=35) | active |

## OP-1 — Always verify before asserting

**Imperative:** Do not hypothesise — ALWAYS VERIFY.
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** measured: conclude-last/b2 0% -> 100% (n=35)
**Rests on:** prompt-hamsa-audit-log:A-21
**Status:** active

**Valid:** invariant

The active ingredient is an unconditional imperative that binds at every claim. A-21 measured 11 arms: b2 imperative-only scored 100.0%, beating the full paragraph at 93.3%, against 0% bare. Conditional guards gate on the doubt a planted belief suppresses, which is why the guard-shaped variants lost.

## Template for new entries

<!-- Insert new OP-N entries above this line. Use artifact(action="append_entry", id=<this artifact>, id_prefix="OP", anchor_heading="## Template for new entries", title=…, body=…) — never hand-format the heading. -->
