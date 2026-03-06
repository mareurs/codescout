# Onboarding Memory: Architectural Invariants Design

**Date:** 2026-03-06
**Status:** Approved

## Problem

Onboarding memories capture *what* a codebase is (abstractions, data flows, layer structure)
but not *what rules govern it* (invariants, defaults). Without this, agents discover constraints
the hard way — by violating them — or treat every preference as equally breakable.

## Solution

Add two new sections to the `architecture` memory template:

- **Invariants** — hard rules that must never be broken, each with a *why* (concrete failure mode)
- **Strong Defaults** — preferred behaviors that can be overridden with deliberate reason

This is grounded in Design by Contract terminology (preconditions/postconditions/invariants)
and modern context engineering research (ContextCov, 2026; Theory of Code Space, 2026).

The key distinction:
- Invariants have a **specific, observable failure mode** when broken
- Defaults are strongly preferred but have **identified override conditions**
- If everything is an invariant, nothing is — keep invariants to ~5 entries max

## Changes

### 1. `src/prompts/onboarding_prompt.md` — architecture section template

Add after "Design Patterns":

```markdown
## Invariants
[Hard rules — ask "what concretely breaks if this is ignored?"]
[If the failure is vague, it's a default, not an invariant]

| Rule | Why it exists |
|---|---|
| [rule] | [specific failure if broken] |

## Strong Defaults
[Preferred behaviors that can be overridden with deliberate reason]

| Default | When it's okay to break it |
|---|---|
| [default] | [specific condition] |
```

Add to instructions: "For each invariant candidate, ask: what *specifically* breaks if
this is ignored? Vague answers → it's a default. Keep invariants to ~5 or fewer."

### 2. Codescout `architecture` memory — backfill

Add Invariants and Strong Defaults sections with the rules extracted from CLAUDE.md
and codebase knowledge.

**Invariants:**
| Rule | Why |
|---|---|
| `OutputGuard` is the only output limiter | Per-tool limits create inconsistency |
| Mutation tools return `json!("ok")`, never echo | Echoing wastes tokens, zero info gain |
| `RecoverableError` for user-fixable; `bail!` for real failures | Controls MCP `isError` — `bail!` aborts sibling parallel calls |
| All 3 prompt surfaces updated together on tool changes | Silent staleness corrupts agent guidance |
| New tools must be registered in `CodeScoutServer::new()` | Unregistered tools silently never run |

**Strong Defaults:**
| Default | When to break |
|---|---|
| Exploring mode (compact output) | After identifying targets via overflow hints |
| Lazy LSP startup | When diagnostics needed before first edit |
| `RecoverableError` includes a hint | When no corrective action exists |
| Tools live in their category file | When a tool genuinely spans multiple categories |

## Scope

- No new memory topics
- No new Rust code
- Two file changes: `onboarding_prompt.md`, architecture memory content
