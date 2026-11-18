---
id: '7a34f7ff8947dfb7'
kind: plan
status: active
title: Hermetic public page fixture for Obscura acceptance controls
tags:
- effort-steering
- evaluation
- dependency-fixture
topic: effort-steering
---

# Hermetic Page Fixture Implementation Plan

> **For agentic workers:** execute inline under the parent evidence-protocol plan. Standing delegation opts out of recurring approval rounds. No commit,merge,push,global change,deletion or model/provider call. This is an unqualified diagnostic until all recorded conditions pass; whole admission review stays under parent Task4.

**Goal:** Remove the changing external webpage from Obscura control outcomes while preserving every original expected test and assertion.

**Architecture:** Fresh evaluator container, network none, loopback-only fixed HTTP service with a declared public example-page body. Add example.com→127.0.0.1 only inside that container and parameterize the public MCP test input URL to http://example.com:18080. Keep the domain/name and all original assertions. Only the spawned test MCP child receives OBSCURA_ALLOW_PRIVATE_NETWORK=1. Production browser/selector/HTTP code and gold structured-clone patch unchanged. This HTTP fixture does not qualify HTTPS/TLS behavior.

**Tech Stack:** existing task image, Python standard-library HTTPServer, original Rust MCP integration tests and complete stream capture.

**Spec:** [external fixture failure](../../../issues/2026-11-10-obscura-reference-network-dependent-p2p.md), [parent plan](../evidence-protocol-plan-20261109.md). Actual current page has no h1; opening networking fixes2of3tests but cannot provide stable page semantics.

## Approaches and ruling

Public egress still depends on changing content and already fails the required h1. Transport mocking would edit production client code and risk masking the candidate's behavior. Choose an explicit local page/input fixture: it exercises actual navigation,DOM,evaluation,snapshot and selector APIs with fixed HTML, with all assertions retained. The URL scheme changes from HTTPS to HTTP and is disclosed; no HTTPS correctness claim. This is an implementer choice under standing delegation. Actor common preparation/context/runtime must later receive the SAME public fixture version before any paid arm; private acceptance files/gold remain hidden.


## Source-parity ruling from actual image audit — 2026-11-10

The actual pinned image has a2195byte tracked-source delta versus canonical base:3NetworkInformationmethods in bootstrap.js and run_page_init corrections in runtime.rs test helpers. Public preparation used git archive BASE and omitted both. A read-only image git diff and actual image/apply/build captures establish this; build leaves the source unchanged. Preserve the REAL image working-tree baseline as an explicit public delta rather than silently resetting benchmark test helpers. Native v3 expected bytes derive from canonical public files + declared image delta + ORIGINAL reference patch; include both dirty files and the public MCP fixture in final source binding. No original dataset/test assertion/expected name changes. The corresponding public baseline+fixture must still reach common actor preparation/index/runtime under a NEW version before paid admission; no second LLM onboarding. Old canonical-source preparation and v2source-invalid control remain preserved.

This is a generic parity requirement: the actual tracked source supplied to actors and evaluators must match the same declared baseline, including image working-tree edits. It is not a task-specific repair prompt or private expected-result patch. Parent whole-admission review must judge that public overlay and its privacy before payment.

## Global constraints

- Original dataset/expected arrays/task pair and old failed verdicts unchanged. Derived test inputs only; every original assertion remains byte-identical.
- Supplemental public-test path disjoint from gold production path; verify source hashes inside each final evaluator.
- UID0/4CPU/8GiB/swap8GiB/init/networknone,capdropALL,pids4096,900sperstep. No host bind,token,home,public egress or global DNS/firewall change.
- Fixture only serves static registered bytes, no response derived from test names/statuses/candidate. Ready/HTTP transcript retained; fixtureoff must not pass the3dependent tests.
- No process-wide allow-private setting in cargo or unrelated tests. Spawned MCP child only; original security tests preserved.
- Reuse existing source-only onboarding; no second LLM onboarding. A later public fixture overlay requires explicit source/index/runtime rebinding before actor admission.
- New private fixture must be reviewed with the whole admission change before paid_ready can become true.

## Review focus

1. Dropping an assertion or expected name must be refused by record construction/preflight.
2. Serviceoff and wrong page must produce genuine functional failures; no compile/hash refusal counted.
3. Original MCP child alone gets local-network opt-in; unrelated SSRF tests retain their environment.
4. Container stays networknone; example.com resolves only to loopback inside its private/etc/hosts, no Internet fallback.
5. New fixture is public preparation input common to every future arm; private oracle/gold never reaches actors. HTTP substitution explicitly narrows TLS scope.

### Task 1: Preregister and run native fixture discrimination

**Files:** private loopback_page.py,helper,derived record,registration,complete controls in a new grader-owned scratch directory. Public MCP test file may be modified only in fresh evaluator containers. No production edits.
**Interfaces:** fixture serves literal HTML title/h1 Example Domain on127.0.0.1:18080; runner owns readiness/start/stop and request logs. Derived record preserves original expected arrays and literal rebuild/test/print commands for full controls; a separate3test filtered record is diagnostic only.

- [x] Verify original MCP source/args,URL occurrences,original assertions and gold paths; freeze exact test-input patch and fixture bytes before native runs.
- [x] Run filtered known-reference with serviceoff:3dependent tests must fail,0missing,full streams; no setup error.
- [x] Run filtered known-reference with serviceon:3dependent tests must pass,0missing,full streams,actual source/runtime/hosts/environment bindings.
- [x] Run wrong-page negative control (no h1):selector must fail while navigation/title still pass. Keep all outcome/status records.

### Task 2: Full population and admission handoff

**V4compatibility failure and v5correction:** fullv4base/reference expose an extra original SSRFP2Pfailure because example.com was mapped globally toloopback. Preserve bothcompletedcontrols and explicitrepeat-notexecutedrecord; do notrescore. [V5registration](../evidence/full-obscura-fixture-v5-20261110/registration.json) moves ONLY the public fixture to fixture.example.com,keeping originalassertions/expectednames/literalcommands/child-onlyoptin. Actualv5base has exactly2originalF2Pfail and329P2Ppass; reference331pass; repeat331pass(43.6s),0missing. Detailed rootverification/custody and commonactorbinding remain open; the fullobservations do not clear paidadmission. [Collision issue](../../../issues/2026-11-10-page-fixture-hostname-collides-with-ssrf-test.md).

[Full331v4registration](../evidence/full-obscura-fixture-v4-20261110/registration.json) was written before native execution. Original2F2P/329P2P and literal rebuild/test/print lists are unchanged; same public fixture and declared image delta as v3. Base/reference are running; reference-repeat will use the same registration. No expected-name filtering or dropped assertion; paid_readyfalse. Common actor/index/runtime alignment remains separate and required.

- [x] Once fixture discrimination passes, preregister full331expected base/reference/reference-repeat. Baseline must fail only2F2P; reference/repeat all331pass with identical maps. Preserve all failures if expectations unmet.
- [x] Verify unchanged assertions/expected population and final production/test/service hashes,networknone,readiness/requests,OOM/cleanup; preserve checked archive and exact runtime dependencies.
- [ ] Bind this PUBLIC fixture to common actor preparation/runtime and independent evaluation. No paid admission from evaluator-only success; parent all-arm qualification/review remains required.

## Self-review

Source inspected: McpClient::spawn launches only mcp; test_navigate_and_snapshot asserts domain,Example Domain and URL; env_allows_private_network accepts the child-only opt-in. Gold changes a separate JS bootstrap file. No claim yet that the local fixture compiles or passes; native discrimination is required. The plan introduces no candidate-production correction or task-specific repair prompt.


## Registered native discrimination — 2026-11-10

V2 serviceoff observed13pass/3fail but refused source-byte binding, retained as invalid for admission. [V3 registration](../evidence/hermetic-page-fixture-v3-20261110/registration.json) explicitly declares canonical public source + immutable-image tracked delta; no evaluator production edit. Native v3:serviceoff13pass/3fail(25.2s),correctpage16pass(20.0s),wrongpage15pass/selectorfail(22.3s),all0missing. All3complete streams/source/service bytes/runtime limits/networknone/global-private-env-absence/loopbackhosts/OOM0/fresh absence independently verified. Requests recorded0/8/8 respectively, including controller readiness. Task1 completion command:python3 .superpowers/sdd/2026-11-10-hermetic-page-fixture/verify_task1.py. Full331andcommonactorbinding remain Task2.

The first registration attempt stopped on a misspelled runner constant before any native run; failed helper preserved. Corrected NEXT contract and explicit empty stderr_files were read from actual source. A separate all-indexed-path audit measured all133Obscuraimagepaths:130match,3differ(.gitignore plus2sourcefiles). Lean audit refused gitlink160000 aur/lean-ctx-bin before producing a count; it does not establish broad Lean source parity. All such failed instruments and their exact outputs remain retained.
