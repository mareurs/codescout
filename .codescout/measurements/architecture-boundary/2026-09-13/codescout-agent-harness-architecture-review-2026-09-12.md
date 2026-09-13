# Codescout: architecture review for a more natural agent environment

Review dates: 2026-09-12–13. Scope: the codescout checkout and its live MCP tools. Starting HEAD was `b3534a25da3abc48606d08db5bc8b07ca89f8db8`; ending HEAD was `34a0beb1f7e32f0a549a51b1101bccbfdd2c7b65`. The shared working tree changed during the review. Citations refer to working-tree code inspected during this session, not an immutable release. Recommendations below are proposals, not accepted ADRs or implemented changes.

## Assessment

Codescout has the right capabilities for an effective coding environment. The most promising refactoring is to make their composition more dependable: resolve context once, preserve result meaning through every output form, give background work an explicit lifecycle, and centralize mutation mechanics.

The architectural pressure is concentrated at the boundaries between those capabilities. Tool authors and agents must currently account for workspace selection, rendering branches, guidance state, several write sequences, and separate persistence steps. The code contains thoughtful defenses against these problems, but some invariants are still maintained by repeated conventions and repairs.

My preferred experience would be: every result tells me what it describes and how complete it is; its addresses work directly in the next operation; every edit reports exactly what committed; every long operation has an observable terminal state. I should be able to spend attention on the user's problem while the runtime carries those facts.

This is an architecture review, not a measured latency study or a fault-injection audit. Confidence in the structural observations is higher than confidence in the relative return on each refactor.

## What the project is

Codescout implements the capability/runtime side of an agent harness. Its `Agent` type holds workspace and service state; the client supplies the model's reasoning loop. The MCP entry point resolves tools and controls their execution. Core tools provide file access, structural navigation and edits, commands, retrieval, and memory. The librarian contributes artifact operations through an adapter and a separate context.

```text
Agent client
    │ MCP request
    ▼
CodeScoutServer
    │ dispatch, access, write lock, cancellation, telemetry
    ▼
Tool::call_content
    │ parameter repair → Tool::call → annotation/guidance/render/buffer
    ├─ Code/file tools → LSP, AST, filesystem
    ├─ Search/memory → retrieval backends and stores
    ├─ Command tools → shell processes and output buffers
    └─ LibrarianAdapter → artifact tools → markdown + SQLite catalog
```

The actual registration is in [server.rs](/home/marius/work/claude/codescout/src/server.rs:330), the execution path in [call_tool_inner](/home/marius/work/claude/codescout/src/server.rs:1112), the shared wrapper in [Tool::call_content](/home/marius/work/claude/codescout/src/tools/core/types.rs:1185), and the librarian boundary in [LibrarianAdapter::call](/home/marius/work/claude/codescout/src/librarian/adapter.rs:258).

There are good boundaries to retain. [LspProvider](/home/marius/work/claude/codescout/src/lsp/ops.rs:77) separates tool logic from real and mock language servers. [RetrievalClient](/home/marius/work/claude/codescout/src/retrieval/client.rs:91) already holds code-store and embedder interfaces. Workspace pinning, cross-process write locking, syntax checks, result buffers, and guidance deduplication all solve concrete problems. The proposals below build on them.

## 1. Separate result meaning from rendering and transport

**Priority:** first. **Confidence:** high on the boundary; medium on the size of the migration.

**Evidence and context.** The shared tool contract returns `Result<Value>`, then `call_content` performs parameter correction, executes the tool, relativizes path fields, adds workspace/write annotations, coordinates guidance, chooses buffering, and formats text or JSON. Buffer and inline paths separately preserve correction metadata; the compact-text branch separately prefixes notices. This is visible in [types.rs](/home/marius/work/claude/codescout/src/tools/core/types.rs:1185), [the buffer branch](/home/marius/work/claude/codescout/src/tools/core/types.rs:1405), and [the compact branch](/home/marius/work/claude/codescout/src/tools/core/types.rs:1514).

Telemetry also depends on presentation: [classify_content_result](/home/marius/work/claude/codescout/src/usage/mod.rs:171) parses the first content block as JSON to recognize overflow and some error shapes. [server error handling](/home/marius/work/claude/codescout/src/server.rs:1248) uses an error-family classifier from the usage module to choose recovery guidance. These are concrete dependencies between presentation, measurement, and runtime behavior.

I encountered the multiple response forms directly during this review: plain symbol text, JSON search results, heading maps, and buffered JSON envelopes. They are individually useful; composing them requires remembering which representation a handle contains.

**Proposed decision.** Introduce a typed internal result envelope that separates outcome, payload, provenance, completeness, notices/corrections, and available continuations. Emit telemetry from that envelope and render it afterward. Keep a concise text representation for agents; structured internal data does not require verbose JSON everywhere. Use stable error codes to drive recovery and telemetry, with client-specific MCP error encoding at the transport edge.

A result handle should describe its payload kind and origin. A follow-up should carry a ready-to-use tool name and arguments where useful, rather than requiring the agent to reconstruct them from prose. Start with existing buffered reads and symbol-to-edit handoffs; a generic workflow language is unnecessary.

**Alternatives considered.** Merely splitting `types.rs` leaves the same branch-sensitive invariants. Making every response raw JSON preserves machine readability but imposes a readability and token cost. Adding more prose reminders leaves interpretation in the agent's context.

**Consequences.** Easier: preserve corrections and warnings independently of output size; support another client renderer; record semantic outcomes accurately. Harder: migrate tools and preserve existing response contracts during the transition. Keep an adapter from current `Value` results and move one tool family at a time.

**Change scenarios absorbed.** A new warning must survive compact and buffered output; a different client needs different failure encoding; a caller follows a result into another tool without translating field names.

**Revisit when.** The pilot envelope increases ordinary-call output or pushes tool-specific branching into a new universal renderer. The goal is common metadata ownership, not one formatter for all content.

**Validation required.** Exercise the same semantic result across small/large, text/JSON, and recoverable-error paths. Apply mutations that drop metadata at each rendering site and observe which tests fail. Measure end-to-end task success and delivered output size separately.

## 2. Resolve a request's project and identity once

**Priority:** alongside the result boundary. **Confidence:** high.

**Evidence and context.** [ToolContext](/home/marius/work/claude/codescout/src/tools/core/types.rs:66) contains the shared `Agent` and an optional workspace path. [with_project_at](/home/marius/work/claude/codescout/src/agent/mod.rs:703) resolves that selection each time it is called; [with_project](/home/marius/work/claude/codescout/src/agent/mod.rs:1208) still resolves the ambient default. The dispatcher, write-lock acquisition, tool body, result annotation, and usage recording make project-dependent decisions at different points. See [write guard acquisition](/home/marius/work/claude/codescout/src/server.rs:698), [dispatch](/home/marius/work/claude/codescout/src/server.rs:1140), and [result annotation](/home/marius/work/claude/codescout/src/tools/core/types.rs:1319).

Pinning already exists and is valuable. The remaining opportunity is to make the resolved project a property of the request, rather than a convention that every participant must repeat. This review did not reproduce a workspace race.

Identity has related boundaries: the server resolves a guide-ledger key separately from its usage-correlation identity, while write-lock attribution reads session environment variables. [Session-key resolution](/home/marius/work/claude/codescout/src/tools/session_key.rs:40) provides an existing foundation; preserve distinctions between conversation, caller, and process rather than collapsing them into a single string.

**Proposed decision.** At ingress, build a resolved request context containing the project/workspace handle, relevant policy snapshot, conversation/caller identity, and cancellation/deadline context. Pass that resolved handle to guards, tools, storage adapters, annotations, and telemetry. Activation changes defaults for future requests. Define explicitly which policy revocations, if any, can invalidate an in-flight request.

**Alternatives considered.** Requiring every caller to repeat `workspace=` improves explicitness but does not remove repeated internal resolution. Serializing all requests would reduce concurrency and avoid neither attribution drift nor confusing state lifetimes.

**Consequences.** Easier: concurrent workspaces, correct provenance, narrower tool dependencies, clear testing setup. Harder: handle lifetimes, project eviction, and policy-revocation semantics require deliberate design. A resolved handle need not become a broad service-locator trait.

**Change scenarios absorbed.** Another caller activates a workspace while a request awaits LSP; a child agent operates on a sibling checkout; a conversation changes while the MCP process remains alive.

**Revisit when.** A project handle retains expensive resources after work ends, or the design makes intentional policy revocation ineffective.

**Validation required.** Pause a request between project resolution and execution, activate another project, then verify its read target, write lock, output root, and telemetry destination. Mutate each consumer separately to use the ambient default and observe the tests fail.

## 3. Give background work job handles with terminal states

**Priority:** early agent-experience pilot. **Confidence:** high on the gap; medium on how much lifecycle machinery to share.

**Evidence and context.** [spawn_background_command](/home/marius/work/claude/codescout/src/tools/run_command/inner.rs:89) starts a child, drops its handle, waits through a cancellation-aware warm-up, then returns a log-buffer handle. [BufferInner](/home/marius/work/claude/codescout/src/tools/output_buffer.rs:122) stores background jobs as a mapping to log paths. Indexing uses a separate [Agent lifecycle and abort slot](/home/marius/work/claude/codescout/src/agent/mod.rs:54), and [index cancellation](/home/marius/work/claude/codescout/src/tools/semantic/index.rs:910) takes that shared slot and manually sets indexing state.

During this review, the background test runner gave me a log handle. I added explicit exit-status markers to the shell command so completion of both test lanes could be determined from the log. That is a useful local example of lifecycle information the runtime could expose directly; it is not a latency benchmark.

**Proposed decision.** Introduce a small job registry keyed by job ID, with workspace and caller ownership, running/completed/failed/cancelled state, terminal exit or domain outcome, and output references. Keep request cancellation distinct from cancelling an intentionally detached job. Route command jobs and indexing through shared lifecycle mechanics where their requirements actually match.

**Alternatives considered.** Polling logs leaves completion interpretation to callers. Keeping a single active indexing slot stays simple but does not provide independent addressability as concurrent work expands. A distributed queue is not justified by these local workloads.

**Consequences.** Easier: wait, cancel, resume observation after context loss, distinguish a quiet task from a completed one. Harder: process reaping, retention, shutdown policy, and reconnect semantics become explicit responsibilities. Start with process-lifetime tracking; persist jobs only where restart recovery is required.

**Change scenarios absorbed.** Tests and indexing overlap; an agent cancels one specific operation; a compacted conversation needs the terminal status without re-running work.

**Revisit when.** Job state starts duplicating domain state without a clear owner, or persistence requirements exceed what an in-process registry can reliably supply.

**Validation required.** Exercise instant success, instant failure, quiet running work, cancellation, concurrent jobs in different workspaces, and output eviction. Assert terminal state independently of log contents.

## 4. Share a prepared-change and commit mechanism across editors

**Priority:** after request context, or sooner if editor work is already planned. **Confidence:** high on consolidation; medium on the full design.

**Evidence and context.** Rename builds a plan and writes files with best-effort restoration of preceding files on failure ([edit_code.rs](/home/marius/work/claude/codescout/src/tools/symbol/edit_code.rs:487)). Replace writes the candidate and then re-extracts symbols and checks syntax ([replace](/home/marius/work/claude/codescout/src/tools/symbol/edit_code.rs:1085)); its corruption branches restore the original. Insert validates/repairs a candidate before an atomic write ([insert](/home/marius/work/claude/codescout/src/tools/symbol/edit_code.rs:1397)). Each path performs LSP notification, call-edge invalidation, and dirty tracking. These are existing safety mechanisms with different sequencing, not an absence of safety.

**Proposed decision.** Separate edit preparation from committing prepared changes. Represent target files, preimages or expected hashes, candidate content, and validation evidence. Centralize commit mechanics and post-commit invalidations. Keep language- and operation-specific validation separate. Use expected-version checks to detect stale preparation under cooperative concurrency; specify the remaining race boundary for external editors that do not honor locks.

Preparation can remain internal for normal edits. Optional preview is useful for broad changes, but this proposal does not require an extra approval round trip for every small edit. Multi-file replacement cannot be made crash-atomic by calling single-file atomic rename repeatedly; recovery needs an explicit journal if that guarantee is required.

**Alternatives considered.** More local rollback helpers leave orchestration duplicated. Forcing all edits through text replacement gives up structural targeting. Mandatory previews for every edit add latency without establishing a storage guarantee.

**Consequences.** Easier: consistent receipts, central invalidation, optional previews, stale-edit detection, and fault injection. Harder: defining partial completion and recovery, accommodating LSP-derived edits, and preserving existing syntax-repair behavior.

**Change scenarios absorbed.** A new editor action, multi-file rename, stale content between preparation and commit, or failure after one file changes.

**Revisit when.** A common change type accumulates language-specific exceptions. Keep the shared layer about versions, writes, and receipts rather than syntax semantics.

**Validation required.** Inject write failures at each target; test content changes before commit; verify successful and restored files receive the correct invalidation; test process interruption if durable recovery is part of the promised contract.

## 5. Make librarian file/catalog updates a recoverable operation

**Priority:** foundational reliability work. **Confidence:** high on the persistence boundary; medium on the recovery strategy.

**Evidence and context.** [doc update](/home/marius/work/claude/codescout/src/librarian/tools/update.rs:393) parses and repairs inputs, resolves an artifact, prepares markdown, validates parameter changes, then performs filesystem and catalog operations. The persistence sequence writes the file, upserts the catalog row, merges augmentation parameters, records a body event, and optionally commits refresh metadata ([update.rs](/home/marius/work/claude/codescout/src/librarian/tools/update.rs:631)). Prevalidation already prevents a class of partial updates. The remaining architectural fact is that the filesystem and SQLite are separate commit domains. This review did not inject a storage failure or claim observed data loss.

**Proposed decision.** Introduce an artifact mutation service that owns preparation, validation, file staging, database mutation, and recovery bookkeeping. Define authority by field: markdown-owned content, catalog-owned relationships/augmentation, and derived values need explicit reconciliation rules. Group database changes transactionally where possible, and use an operation journal or equivalent recovery protocol for the file/database boundary. Keep the public `doc` tool while its action handlers become thinner callers.

**Alternatives considered.** A SQLite transaction alone cannot roll back filesystem writes. More instructions to edit only through the librarian preserve the intended route but do not define interrupted-operation recovery. Moving everything into SQLite changes the git-friendly document model and is not justified by this review.

**Consequences.** Easier: explain and recover interrupted updates, support both CLI and MCP consistently, add mutation types without repeating persistence sequencing. Harder: recovery format, migration, idempotency, and the distinction between mandatory events and best-effort telemetry must be specified.

**Change scenarios absorbed.** Storage failure between file replacement and catalog update; CLI and MCP mutate the same artifact; a new augmentation update must be consistent with body metadata.

**Revisit when.** A single source of truth becomes a product requirement, or recovery records are too expensive relative to the actual write workload.

**Validation required.** Interrupt after each persistence phase, restart/reconcile, and inspect both markdown and catalog state. Test real production mutation paths rather than fixtures that reproduce the algorithm.

## How to determine whether these changes help agents

Keep the existing navigation/edit checks. [The edit eval harness](/home/marius/work/claude/codescout/tests/e2e/edit_eval_harness.rs:7) already classifies destructive outcomes and is explicitly ignored in an ordinary test run. This review did not run it or audit the entire eval corpus.

Add or extend workflow-level measurements around concrete tasks: locate and change a method; recover from an ambiguous symbol; follow a large buffered result; edit an artifact and read it through the other interface; observe a background failure; continue after compaction with another workspace active.

Measure task correctness, recovery success, tool round trips, delivered bytes/tokens with a named instrument, and wall-clock time separately. Fewer tool calls can hide a worse answer. Use paired tasks before and after a pilot; do not infer agent efficiency from helper-test counts or source-file size. Keep measurement output outside the corpus being measured.

## Recommended sequence

Pilot the result envelope and resolved request context on one read tool and one write tool. Preserve public spellings and existing recovery behavior during that pilot. Add observable command jobs for an immediate workflow improvement. Then migrate edit commit mechanics and artifact mutation operations against fault-injection tests.

The architecture skill shaped these proposals by requiring a concrete change scenario, actual code evidence, alternatives, costs, and a revisit condition for each boundary. No rewrite, service extraction, or additional orchestration product is established as necessary by this review.

## Verification record

Live read-only probes exercised workspace activation, memory reads, symbol navigation, semantic search, file/buffer reads, and commands. Editing and storage observations above are source inspections, not live destructive probes.

`./scripts/fmt-mine.sh` and `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` completed successfully. `cargo test --workspace --no-default-features` and `cargo test --workspace` ran in that order with independent exit capture and an unconditional transition to the default lane. The terminal log reported `review_lean_exit=0 review_default_exit=0`. The default lane ran last. These are baseline checks; they do not validate the unimplemented architectural proposals.

The local gate does not exercise `server-stack`. Its dedicated clippy/test lane is declared in [ci.yml](/home/marius/work/claude/codescout/.github/workflows/ci.yml:316); this review inspected that declaration but did not establish the latest remote CI result. Existing concurrent checkout changes limit any claim that the gate describes a frozen revision.
