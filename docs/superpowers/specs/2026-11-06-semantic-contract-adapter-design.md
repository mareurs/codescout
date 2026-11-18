---
id: fb7d21b6e59730b3
kind: spec
status: active
title: Paid workflow adapter with public classification and owned runtime supervision
tags:
- effort-steering
- controller
- runtime
---

# Semantic contract adapter design

## Intent and authority

Complete the actual host adapter needed to run the fixed semantic-contract pilot. User delegated autonomous implementation and said go after private qualification. Method choices below are ours; no new approval round or merge/push. This implements the remaining boundary of spec28aab59cb5842630, without changing prompts/tasks/model/efforts/common instructions or private oracle bytes.

## Evidence reused

Runtime source36fec1201b0f809d6f21cc31082def4d651b6cf8 is qualified for4CPU/32GiB, exec4GiB /tmp, networknone. Grader policy867c680924cb75575bec7ccd7068b8dfd7fd5889 is qualified for the two private4CPU/8GiB tasks; reporta812bb15c2d91b80/archive2bc2fc8706b4b4d7c3d302e6b6cdd0b832704903db3bd4fe4ba4b23e7cf17fbc. Existing closed campaign.py engine already enforces exact six attempts, cost reconciliation before candidate/cleanup faults and private-outcome embargo. Readiness deliberately has no paid launch. Old onboarding, six delivery cases and NUlow replay are closed; do not rerun them.

## Approaches

1. Validate only final receipts: insufficient because the client may already have continued after infrastructure failure.
2. Patch global Popen/native-check functions privately: preserves old product bytes but obscures ownership and provider/continuation call sites.
3. Chosen: explicit optional supervisor hooks in session/workflow/external entry and a provider request guard. The private adapter supplies health/classification and reuses the existing engine/launch/reconcile/capture code. Defaults preserve current callers. Cost: small generic API delta requiring call-site behavioral and applied-mutation tests.

## Runtime interfaces

session.run_session gains guard=None and client_observer=None. Call guard before client creation and on every remaining-time boundary (including sends and pending-job waits); notify observer with the actual returned Popen before first prompt. Existing finally owns/reaps that exact process group.

workflow.execute and external.launch gain supervisor=None. The supplied object binds out/image/server before launch; methods check(), client_started(proc), before_public(), after_public(command,log,code), finish(). Public callbacks execute around the actual native_check inside PublicVerifier. Invalid health/classification raises before feedback can be returned. finish runs once after background/candidate preservation and before client/native teardown, including error paths. Default None adds no effect.

transport.Proxy checks an optional server.request_guard at request entry and immediately before each actual upstream forwarding. This closes the gap between periodic health failure and a new paid request. In-flight requests cannot be unspent; incomplete cost remains explicit and stops the series.

## Supervisor and paid adapter

Host-only supervisor, no hidden expected-set inputs. Sample root/tmp/MemAvailable at start, nominal10second intervals and public/final boundaries; floors80GiB/4GiB/48GiB. Discover only this launch's native records, require exact image plus codescout.hfa name label and immutable CID, native4CPU/32GiB/networknone and exec4GiB /tmp config. Read cgroup memory.events; unknown/missing/OOM or any floor/probe fault invalidates and stops. Abort only the actual observed client Popen group and verified owned native CID; never process-name matching or arbitrary recorded PID.

A public boundary requires before/after health observations, complete untruncated log (64MiB maximum) and classify_public from the committed policy. Functional nonzero remains feedback; infrastructure invalidity is never model feedback. Save raw code/log and separate classification. Final health and independent daemon absence remain mandatory. Sampling is not a resource reservation or proof between samples.

PaidBackend extends existing Readiness: launch uses external.launch with supervisor; reconcile calls the already calibrated driver.reconcile; valid_public checks every classification and final supervision proof; cleanup independently queries daemon for all observed CIDs. The engine retains one-shot attempt markers, all known/unknown charges and source/input drift stops. No private grade before all six repair attempts/costs/candidates close.

## Inputs, budgets and admission

New native-managed worktree from36fec1201; preserve old runtime worktree and grader80boundpaths. Bind new host/runtime/adapter sources AND unchanged native worker source, full plugin identity, fixed six prepared inputs/physical writable clones, qualified private source/control receipt. New registration supersedes no old artifact. Reuse actual authored knowledge; no new onboarding or task-specific authoring.

Known prior58.180531399999999104USD, ceiling200USD, each20USD reserve/19USD CLI cap, exact order from campaign.POLICY. Decimal accounting. Token path remains the already authorized claude-stef file; never print token or request a replacement as routine setup. Provider/model calls remain disabled throughout implementation and zero-provider qualification.

## Verification and completion

TDD for each hook/guard and the full native call-site chain using fake Docker/process boundaries; direct owned-subprocess abort test. One fresh whole-change reviewer on strongest available model applies at least12actual load-bearing mutations on copies, reports observed survivors; one offline fix pass/no second reviewer. Run targeted lifecycle/transport/adapter suites and full repo gate. Commit only reviewed product paths in isolated branch, never push/merge.

After source freeze, one new disposable actual-CLI scripted common integration smoke proves the new adapter/public supervision path without provider/credential access; old six wire cases remain untouched. Revalidate full plugin/source/image/inputs and actual resource/cleanup proofs. Archive raw evidence with each payload reopened/hashverified. Deliver implemented adapter plus truthful fresh admission; actual paid pilot activation uses the existing finite cap and fixed six order only once when all registered gates are satisfied. No efficacy claim before that pilot and later held-out validation.
