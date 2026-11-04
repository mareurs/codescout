---
id: c7aa0fa96d0484fd
kind: research
status: active
title: Worker context verification — actual Claude transport and native MCP
tags:
- effort-steering
- context
- validity
topic: effort-steering
---

# Worker context verification

## Readiness follow-up — 2026-11-04

**LATEST:** controlled root rebuild from clean build scopes at e6f07e9a6 completed after a fresh full gate (all lane exits0). The initial release call was cached; a second call invalidated only build.rs mtime and observed root lib/bin rustc invocations. All527 inventoried tracked build-input files stayed byte-identical, and archived source entries match. Rebuilt raw ELF SHA8f567f170b07de559b0dfc89e0250d41bd80f9e52c0e650b1b77a795f1454dea matches the tested artifact exactly; private guest package SHAcdc53728d28a659b5a697048e6e5b719149693df2cf57cb8554ee0751ebf51bb passed all four native store/marker cells. Existing compiler/dependency caches were reused: this proves a controlled root rebuild, not an independent hermetic dependency rebuild or general byte reproducibility.

Actual Codex app-server reload accepted `{}`. A subsequent live workspace call still returned OLD PID1745637/build439eb06e/deleted executable during this active turn. **Live deployment remains pending:** query the actual answering server on the next active turn and hash its `/proc/<pid>/exe` against8f567f17 before declaring completion. No server process was killed, no profile/config file changed, and no standalone probe was labeled live. [Official API documentation](https://learn.chatgpt.com/docs/app-server) describes the queued reload.

Canonical issue5faa9c6e3fd2a6f9 holds results and remaining deployment caveat; private cleanup-verifier false negative3e9647481c8c7d1b is corrected with independently absent four exact owned names and sixteen matching source/config instances. [Build/source/gate/native/reload evidence](data/2026-11-04-native-build-provenance.tar.gz): SHA7b9cc96c195aa38f1f6ae3598a09df83a59cb67a738fbe9adebac271f0ce12ad,5889757bytes/72verifiedentries, including all527 source entries in a separately verified inner archive. No paid model/onboarding calls, credential reads, product source-content edits or worker quality admission. Original [earlier binary recheck](data/2026-11-04-native-readiness-recheck.tar.gz) and historical checkpoint remain preserved.

## Result and scope

Actual pinned Claude-client serialization and native MCP transport are verified on two fresh common-source clones, with effort fields low and xhigh. Both carry the visible global rules, complete project CLAUDE.md, complete Recon body, explicitly read Codescout guidance and root architecture memory. This is a transport result, not model adherence, quality parity or full paid-worker admission.

The Anthropic-compatible endpoint was a loopback recorder returning scripted replies. No real API credential was read or supplied; no provider model was invoked. Token usage and zero client cost returned by that endpoint are fixtures, not a new spend measurement. Existing client cumulative29.221154299999999834/40 and remaining10.778845700000000166 are unchanged. No onboarding author was repeated.

Operator requested issues immediately in the main checkout; all seven findings below are recorded there. Research implementation worktree remains /home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101. No product source was changed or published.

## Readiness contradiction — actual cause

Agent::project_status checks whether .codescout/embeddings/project.db exists; the bootstrap maps that boolean to Built/Not built. Actual IndexStatus resolves the configured retrieval backend and reads its stored chunks. The current research source and the pinned binary's recorded base commit both contain that legacy-path check; because the package build was dirty, the runtime controls establish behavior independently.

| Actual selected store | Zero-byte legacy project.db | Actual indexed/queryable | Bootstrap says | Returned query results |
|---|---|---|---|---|
| Empty | absent | false / unspecified | Not built | 0 |
| Empty | present | false / unspecified | Built | 0 |
| Populated | absent | true / true | Not built | 3, capped |
| Populated | present | true / true | Built | 3, capped |

All four expected cross-predicate observations occurred in actual native calls. Adding a zero-byte marker changes the banner but supplies no index; rebuilding a real index without that legacy file does not repair the banner. [Main issue](../issues/2026-11-04-semantic-bootstrap-disagrees-with-queryable-index.md) remains open. Prior research-worktree issue56a9630d26ddbb2d is the historical record; main issue5faa9c6e3fd2a6f9 is canonical for this continuation.

## Initial real-client failures

The original private-HOME launch exposed gaps not exercised by direct handcrafted MCP initialization:

- Claude sends server/discover before legacy initialization. The rmcp server closes, Claude respawns, and a retained fixed Docker name conflicts with its first stopped container. The request contains a failed-MCP notice and no native MCP tools. [Lifecycle issue](../issues/2026-11-04-claude-mcp-discovery-retry-reuses-container-name.md).
- Skill invocation explicitly returns Unknown skill: codescout-companion:reconnaissance. Host/root skill availability did not load the isolated worker's plugin. [Recon registration issue](../issues/2026-11-04-isolated-worker-recon-plugin-not-loaded.md).
- Instruction-source headers include the private global snapshot, host /home/marius/.claude/CLAUDE.md and the project CLAUDE.md. Private HOME/config alone did not isolate ancestor discovery. [Inherited instruction issue](../issues/2026-11-04-isolated-home-still-loads-host-claude-instructions.md).
- Oversized .codescout/system-prompt.md was trimmed from initialize instructions and never explicitly read in the earlier direct native sequence. Existing file presence did not establish delivery. This is the private worker bootstrap's gap against a known channel limit, not a product channel redesign. [Guidance delivery issue](../issues/2026-11-04-native-worker-omits-trimmed-codescout-guidance.md).

The initial CLI exited0 after the scripted responder ended the diagnostic. That exit did not mean MCP or skill success. Full failed payloads/debug/launch/cleanup are retained.

## Positive counterparts on fresh common clones

Private configuration uses a unique owned Docker name per stdio connection attempt with auto-removal; explicit pinned companion plugin loading; client CWD outside the host-home ancestor chain; and an explicit native read of the frozen common guidance. Plugin hooks are deliberately disabled, so this does not qualify their effects.

Both final clients use clean Git baseline898002898161832a7c718527d8a75bf8b73428a7 and immutable snapshotc3b559399714f35b65383540f617eaa7ea84db608be80a6a93b41088ade561d4. All1450source/knowledge manifest entries were checked before and after each diagnostic, including both literal source links. Writable source/cache/home are separate. Existing stopped semantic cache and shared read-only model cache were reused.

| Measured property | low | xhigh |
|---|---|---|
| Actual serialized request records | 9 | 9 |
| Every actual output_config.effort equals requested value | yes | yes |
| Ordered visible global-rule text complete | yes | yes |
| Complete project CLAUDE.md | yes | yes |
| Complete Recon body after actual Skill invocation | yes | yes |
| Complete Codescout guidance after explicit native read | yes | yes |
| Complete root architecture memory after native read | yes | yes |
| Entire instruction-source header population | private global + clone project | private global + clone project |
| Additional host-global header | absent | absent |
| Native index status | indexed/queryable;0missing vectors | indexed/queryable;0missing vectors |
| Source/knowledge population and clean baseline unchanged | yes | yes |

Native host Read/Bash/Write/Edit/Agent tools are not advertised in these actual request populations. This observation is not a full authority proof for every other tool or future worker configuration. The scripted responder requested each tool call, so successful transport is not evidence that a real low/xhigh model would choose or obey the same sequence.

An actual host canary file was created and read positively on the host, outside every container mount. Native read requests for that absolute host path return a nested ok:false file-not-found result; its unique content is absent from all captured final payload text. This is one excluded-host-file namespace witness, not a universal/adversarial oracle-isolation proof or qualification of an actual held-out checker.

Exactly11generated names in this diagnostic's recorded launch population independently inspect absent, including discovery/reconnect attempts. These are attempted names, not a count of successfully created containers or a global Docker census.

## Measurement correction and preparation failure

The initial exact whole-global-file substring predicate returned false. Complete source/readback inspection showed only three nonempty source lines omitted, all HTML comments; the visible rules were present. Ordered visible-text comparison after removing only HTML comments/blank lines matches the full actual isolated-global block, and deliberately deleting the visible ALWAYS VERIFY imperative is rejected. The earlier user-facing claim was corrected; raw false result remains as evidence of the failed instrument. [Audit issue](../issues/2026-11-04-context-audit-mistakes-html-comment-stripping-for-rule-loss.md).

A separate first Git-bound clone overlay attempted to recreate existing LICENSE links and failed before any final client ran. Two follow-up launches were incorrectly issued before checking preparation success and failed on a missing diagnostic script. Those failures are retained; the final two fresh clones instead verify existing source entries and copy only missing knowledge, then launch after successful preparation. [Preparation issue](../issues/2026-11-04-context-clone-overlay-recreates-existing-symlinks.md). No product exporter/importer changed.

## Preservation and verification

[Complete evidence bundle](data/2026-11-04-worker-context-verification.tar.gz): SHA256341fc013894825a8676169f362d6ffe4dc66f22124eaa392a528139821f63f22,5715233bytes,1619verified archive entries (1618payload plus manifest),6literal links (2source links and4debug-log pointers). Every archived file hash or literal link target was independently verified by rereading the archive. Absolute debug pointer targets are retained literally; log bytes are separately included, so verification does not follow those pointers.

The bundle contains complete real serialized request/reply populations, client debug/launch/child/cleanup receipts, readiness controls, failed attempts, full common source/knowledge snapshot, verified private Git baseline bundle, frozen global rules, exact Recon file and cache manifest. It excludes profiles/credentials, Git administrative directories, duplicate actor/home trees, database/model/tool payloads. Endpoint replies/usage are explicitly fixtures.

Private active root: /home/marius/.cache/codescout-resident/codescout/worker-context-verification-20261104. Final fresh actors are recorded in client-final2-low/actor-path.json and client-final2-xhigh/actor-path.json. Preserve old sealed experiments and original author receipts; do not rerun onboarding. Pinned Claude SHA and native binary/runtime manifests are inherited from the prior preserved setup; this transport result does not establish source equivalence between a dirty packaged binary and current product HEAD.

Fresh actual controls, full source/context comparisons and archive verification pass for their stated scope. No product source edits; prior full research-worktree gate remains the unchanged-code checkpoint with its published exclusions. No repeated full product suite, new product fix or independent review is claimed here.

## Next gate and end goal

Transport gaps are mitigated in the private diagnostic configuration. The generic product readiness predicate and whitespace-only coverage false positives remain open. Common guidance also contains historical onboarding-time tooling remarks; refresh capability statements from current observations without repeating source onboarding.

Before a paid quality comparison: apply/verify the readiness correction, qualify the actual worker authority and task-specific checks, isolate the real held-out oracle, freeze common effective inputs, then use fresh tasks and fresh xhigh controls within the existing budget. Review any load-bearing implementation with actual applied mutations. Do not promote these scripted calls to a paid controller or count them as evaluated model attempts.

The end goal—making low reliably preserve xhigh-quality behavior at lower whole-workflow cost—remains open.
