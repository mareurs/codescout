---
id: '6cbcb90689a08f18'
kind: research-note
status: archived
title: Latest checkpoint — external adapter committed; source-only setup in flight
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/external-prepared-entry-20261105
original_sha256: 72452207cad06fa621969ad6215aac40147c147b5f0e28dd005594a6074a137e
---

# Latest checkpoint — external adapter committed; source-only setup in flight

2026-11-05, root Codex 01a15ca9-f939-7603-acb5-01c6b02f68e2. Supersedes next-work sections in external-transfer-admission-20261105 and action-budget-live-20261105; historical closed evidence remains unchanged. Generic low-quality mechanism goal OPEN. No external repair attempt has started or been admitted. Do not rerun already closed scripts or repeat onboarding.

## Source and verification

V /home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101, now HEAD c1d51c446c5712b6a09c82e358f5ffbda834923c. Stable patch-id c9c3d68dfc083d1c570c9a2ba5143b0b1e542f93. Six exact paths committed; V and index clean after commit. No merge/push. MAIN remains shared and its existing foreign staged bug-triage blob f4ad12b9cf7b8b830ac35cc341afcde702d0a56c was independently unchanged after commit. MAIN research notes/data remain local under known provenance issue421ed73df25f795d; no foreign ack or hook bypass.

New generic tools/agent_lifecycle/external.py validates prepared inputs before credentials/launch and routes actual pinned CLI through full user,project,local context and wildcard Codescout permissions; artifacts.py captures staged binary Git diff/new/deleted files against controller baseline even when actor changes HEAD. workflow.execute optional required callback runs before native/client cleanup. Capture failure blocks workflow completion while retaining known cost. Excludes .codescout,.buddy,.superpowers,.claude,target,node_modules.

MAIN issue d06194e58297a768 filed immediately: equals-form conflicting Docker options bypassed draft split-token guard. Seven regression subcases observed RED at fake credential boundary, then full normalized parser/duplicate/unsupported-option refusal fixed them. Issue records unmerged branch fix; not archived as fixed on MAIN. Correction reviewer /root/cold_recovery_review accepted six exact source hashes, eleven actually applied mutations: ten assertion kills, one independent production-error rejection (pathspec mutation hits edited-path guard), zero survivors/invalid instruments/harness errors/timeouts. Original rejected review remains separate. Artifacts /tmp/external-capture-review-01a15ca9-20261105/{review.md,aggregate.json,review-binding.json,correction/...}. Reviewed final hashes match committed bytes.

Full gate @bg_000000a0 exited0: PYTHON_LIGHT0 FMT0 CLIPPY0 LEAN0 DEFAULT0; normal heavy-Python/server-stack/local-ONNX exclusions, librarian DEFAULT-only. Record /tmp/codescout-bg/455879-a720241d-bg_000000a0.json; full record/log durable at BASE/external-onboarding-once-20261105/admission/. Separately71 lifecycle,30 pilot,24 workflow unittest methods passed. Gate --check and git diff --check clean. Actual MAIN publish_identity resolver returned canonical root; V predates that script. Peer enumeration18:06:27UTC had no self marker; no peer count or occupancy inference.

## Actual native and capture qualification

BASE=/home/marius/.cache/codescout-resident/codescout.
BASE/external-entry-qualification-20261105: actual pinned CLI generic external.launch, both immutable selected images, synthetic replies only. Per case13 raw requests/11 tool-use IDs with results; full global/project request audit, actual Recon body, one native job delivered to same-session continuation, candidate captured BEFORE teardown and applied to a fresh exact image. Exact recorded containers independently absent;0 provider/credential reads/cost for these synthetic checks. Limits: fixtures, not efficacy.

Original Nushell audit expected only probe-file edit and failed: capture correctly included automatic .gitignore private-memories line. Original script/log/closed receipt preserved. Resume script audited SAME receipt and applied SAME captured patch, then launched rust-analyzer once; did not repeat Nushell CLI.

Actor synthetic Git baselines: NU5ec462b99bb0773843538cb545a741db84d0da92; RA3678da48aebe17797d2f792bde07a46f5bf4a827. Upstream baselines remain different as documented. Images NUsha256:3767f31b9b1a3288ad7132740dcff712737d5e855029079a4171a8065a2040ef; RAsha256:0d74e4f14c68a438dce6cf9e4a02c626432ce12b866f55ac69c6cc687f7fff7a.

BASE/external-native-capabilities-20261105 configured generated project languages before boot:21 actual tools including references/symbol_at/call_graph. Initial unconfigured18-tool list from old phase was not full configured tool availability. BASE/external-native-references-20261105 preserves initially incomplete/cold reference warnings; BASE/external-native-warm-references-20261105 later same-connection calls return cross-file Value/AbsPathBuf references. First requests limit5, later requests limit20 with30s intervals: no controlled timing/cause estimate, only later positive named-reference witness. All these containers closed/absent.

## Semantic and private controls

Jina source-only index attempt BASE/external-workflow-setup-20261105: Nushell timeout1800s, no OOM, exact container removed/absent; partial DB retained, no LLMonboarding/provider calls. Not a quality comparison of embedding models.

Generated-default local:AllMiniLML6V2Q attempt @bg_0000009a, script /tmp/external-default-index-01a15ca9-20261105.py, root BASE/external-default-index-20261105. NU completed exit0 in1827.057s, no OOM, exact container absent; RA then started sequentially and still indexing at checkpoint. This uses existing /home/marius/.cache/codescout/fastembed readonly, no network/model-provider/LLMonboarding. Source-only config selection before any repair outcome. Choose same successful config/cache for both arms.

NU clone tests BASE/external-semantic-ready-20261105 (@bgaa) and external-semantic-verify-20261105 (@bgab) both closed exit0/cleanup. Actual semantic query on cloned store returns source snippets in nu-command conversions/into/bool.rs; config and stored model match MiniLM. Native index(verify) reports complete CODE coverage: expected/stored1980files, missing0, orphan0, vectors_missing0, git level. Those are indexer's own eligible-file/store predicates, not total repository file counts. Nested memories reports generated onboarding/language-patterns2 on disk and0 vectors: pending source-scaffold migration, not fully qualified memory recall. Never cite top-level complete as all-store readiness. After author, migrate/embed authored memories deterministically (no LLMonboarding) and positively test recall; check artifact/doc semantic lane separately if used. Native verify hint names codescout migrate-memories --in-place; inspect actual CLI arguments before invoking.

Fresh existing private live_grader controls @bg_000000a7, script /tmp/external-private-controls-01a15ca9-20261105.py, root BASE/external-private-controls-20261105. Uses unchanged MAIN tools/setup-ab-eval modules in fresh subprocess; pins all source hashes before/after. Actual grader profile4CPU/8GiB, not actor-public24GiB. Baseline-negative NU finished: expected8622, not_passing2 exactly selected F2P, missing0, no OOM, exact container independently absent,841.4s. Gold and gold-repeat then RA negative/gold/repeat still in progress. No provider/credential/model calls. Do not mix run.py namespaces from pilot and setup-ab. Keep expected sets unchanged.

## Author registration and live jobs

ROOT=BASE/external-onboarding-once-20261105. registration.json raises finite tracked-known-series ceiling to150USD under operator's explicit broader-budget authorization. Known prior repair/continuation35.255714099999999604; reserve2 authors*20 (CLIcap19),4 repairs*8. Unallocated42.744285900000000396 against this KNOWN series. Not all historical research/review/setup/CPU/invoice spend. Author ongoing cost UNKNOWN until raw terminal reconciliation; don't report zero new spend or35.255... as current all-in spend.

Fixed future repair order:NUlow,RAxhigh,NUxhigh,RAlow. Same full workflow and identical isolated authored knowledge; no task-specific hint/authority treatment. Current global74a3eb181f6ec2ba5c3275dda9da35102144fe8cc665abaf67a778a29771d8c6 remains previous E policy. Authority factorial still separately unmeasured.

Both source-only author inputs prepared, no repair task/problem/gold/testfeedback. Existing CLAUDE/AGENTS frozen byte-for-byte; source/manifests/tests/Gitmetadata edits prohibited. One sequential author context per workspace; no repeated project explorers. Generated scaffolding only initially; author may use generated grep fallback while independent index finishes. Only authored Codescout facts/system prompt shared later, no run-specific diagnostics. File check is presence/min40bytes over required topics/systemprompt ONLY, not fact quality. Inspect substantive authored facts and candidate before freeze.

author-admission.json binds registration, code hashes, accepted review, completed gate, actual native/wire qualification, complete exact native mount allowlist and readonly input inventory. Only binary/runtime/LSP/cache readonly and isolated nativehome/knowledge writable; no host repo, Docker socket, credential or oracle mounts. Source-only author admitted; repairs FALSE.

First NU author running @bg_000000a8; record /tmp/codescout-bg/455879-a720241d-bg_000000a8.json; command python3 /tmp/external-author-once-01a15ca9-20261105.py launch nushell__nushell-18863. Approved token internally read from /home/marius/.config/s5/token-claude-stef; NEVER print/read token through tools. Real requests/tool calls observed; no final receipt/cost yet. Root6 new topics and several member topics being authored; do not infer completion or fact quality from counts.

Automatic finite continuation waiting @bg_000000ac, record ...bg_000000ac.json; script /tmp/external-setup-continuations-01a15ca9-20261105.py second-author. Waits first author job PID, requires completed workflow + reconciled known cost + candidate edits subset{.gitignore}, checks pinned launch script, then starts the already registered RA author once. If incomplete/source changed, records refusal and stops, retaining all facts. Do not also launch RA manually while this waiter exists. This does NOT launch repairs.

Automatic source-only semantic continuation @bg_000000ad, record ...bg_000000ad.json, same continuation script semantic-rust-analyzer. Waits full default-index job PID, requires both index results0/absence/noOOM, then executes /tmp/external-semantic-verify-01a15ca9-20261105.py rust-lang__rust-analyzer-22992. Does not spend or repeat index/model onboarding. Both watchers use tail --pid, no polling loop. No Codex auto-wake is guaranteed; read records after actual expected completion and don't report root goal complete.

## Preservation and next work

Closed immutable MAIN archive docs/research/data/2026-11-05-external-prepared-entry-evidence.tar.gz SHA b79a6b03bb17580bd219b426b9f1dcc5dc48bdb6dbc32dbef0add4e10b2764fa.326 actual regular payload files re-read and hash-verified;1,638,301archivebytes. Matching preservation.json. Contains committed worker source/dependencies, original/correction review, native/wire/capture/reference proof, gate and static author registration/preparation/scripts; excludes ongoing author/default-index/private-control results, model weights, images, live DBs and profiles. PRIVATE CONTROLLER archive NEVER actor mount. Preserve later closure in a NEW supplement; don't mutate this sealed archive. Scripts are also in archive for exact restoration if /tmp is cleaned; static native/image/cache paths remain under BASE, so retain pinned images and caches.

Still-open plan ae0b584f43fc96e8 (MAIN docs/superpowers/plans/2026-11-05-external-workflow-transfer-admission.md) updated; three admission tasks remain incomplete until their actual full criteria close.

Next concrete work: (1) Read active results/receipts and reconcile author cost via existing pure driver.reconcile before any further paid action; watchers already own RA launch. If censored, continue preserved facts, never restart onboarding/rewrite completed topics. (2) Review substantive KB/source-only candidate, then freeze portable logical knowledge; reuse existing snapshot_reuse.py where compatible. It exports source+logicalMD/TOML, not live DBs. Separate completed semantic cache reuse must close all writers and copy physically with manifest/positive queries; no hardlinks/shared runtime DB/WAL/config/session state between arms. Embed/migrate memories once after author, then prove recall and named-reference/code readiness. (3) Complete unchanged private negative/gold/repeat controls both cases. (4) Prepare matched4 manifests from existing task/public-check contracts and same frozen knowledge; bind full context, actual generic capture and cost/reserves; repairs not yet authorized by admission. Run both arms regardless first quality result. Use existing private live_grader on fresh images after candidate extraction; no gold/oracle before final artifacts. (5) Compare functional outcomes and complete workflow costs; if low fails/xhigh succeeds inspect preserved missing observable behavior before generic intervention, validate later heldout. If both pass, record ceiling/cost, not causal proof of recon/hints. No cosmetic prompt variants or exposed-task reruns.
