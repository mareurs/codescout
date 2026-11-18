---
id: d39d69657657fa9b
kind: plan
status: done
title: Implement and qualify the semantic-contract paid workflow adapter
tags:
- effort-steering
- controller
- runtime
---

# Semantic contract adapter implementation plan

> **For agentic workers:** Execute inline with superpowers:executing-plans under the user's explicit autonomous-work delegation. TDD, one fresh whole-change review with applied mutations, one offline fix pass; no approval pause or second review loop.

**Goal:** Implement and qualify actual public-classification/resource-supervision/cost-capture-cleanup integration for the fixed six-arm pilot.

**Architecture:** Optional generic supervisor hooks in lifecycle/session/external plus provider request guard. Private supervisor/paid backend consume those hooks and the already committed public classifier, existing campaign engine, external.launch and calibrated cost reconciliation. Frozen old source/controls/prepared arms remain untouched.

**Tech Stack:** Python stdlib/pytest, Docker, existing scripted CLI transport, Decimal, SHA256.

**Spec:** docs/superpowers/specs/2026-11-06-semantic-contract-adapter-design.md (fb7d21b6e59730b3), overarching28aab59cb5842630.

## Global Constraints

- Native-managed isolatedWT/home/marius/.codex/worktrees/ca35/codescout, branchresearch/semantic-contract-adapter-20261106 from36fec1201. OldruntimeWT andgrader867c680924cb75575bec7ccd7068b8dfd7fd5889/80boundpaths unchanged. No merge/push.
- Hostfloors80GiBroot/4GiBtmp/48GiBMemAvailable; nominal10second samples/public/final boundaries. Actor4CPU32GiB/networknone/privateexec4GiBtmpfs. Privategrader4CPU8GiB remains separatelyqualified. Unknownruntime/capture/absence invalidates.
- Knownprior58.180531399999999104, ceiling200, each20reserve/19CLI; exact campaign.POLICY order, sixattemptsonce, no resampling. All repaircosts/candidatesclosebeforeprivategrades. Provider/credentialsdisabledthroughthisimplementation/qualification stage.
- No newonboarding/protocol edits/hidden feedback; reusefrozenknowledge/fullplugin. New source registration before qualification; oldsourcehashes do not admit newhooks. Initial requests/inflightcosts cannot be erased by a resource stop.
- Save allraw logs/charges/probes and archive everypayload hash; ROOT/home/marius/.cache/codescout-resident/codescout/semantic-contract-adapter-20261106. BugsMAINdocs/issues immediately.

## Review Focus

- A floor/OOM/probe fault during idle client output or background wait must prevent a later provider request and continuation; kill only owned Popen group and verified CID.
- A public functional failure must still reach the same session as feedback; a fatal/storage/capture fault must raise before feedback, even when CLI terminal status succeeded.
- Final supervision must run before native deletion on success/error and retain cost/candidate/background evidence; missing final proof cannot become healthy defaults.
- Backend methods must invoke actual external.launch/driver.reconcile/policy/daemon checks, not fabricate admission or cleanup booleans; source/input/plugin drift must stop before credential/provider contact.
- Cost unknown/transport/context/capture/cleanup faults stop the fixed sequence with all charges preserved; no private grade before six repair candidates/costs close.

### Task 1: Generic lifecycle and provider guard hooks

**Files:** Modify tools/agent_lifecycle/session.py,workflow.py,external.py; tools/hint_authority_pilot/transport.py. Create tools/agent_lifecycle/test_supervision.py; extend existing external-entry/transport tests where their real boundaries are used.

**Interfaces:** session.run_session(...,guard=None,client_observer=None); workflow.execute(...,supervisor=None); external.launch(...,supervisor=None). Supervisor methods bind(out,image,server),check(),client_started(proc),before_public(),after_public(command,log,code),finish(). Optional server.request_guard is checked on request entry and actual upstream forwarding.

- [x] Write behavioral tests first: guard blocks spawn; actual Popen observer precedes prompt; failure during live idle/background wait stops the owned client; functional feedback continues; supervisor invalidates before second prompt; finish precedes native cleanup on success and exception; default callers retain behavior. Run and observe intended RED, not import/fixture errors.
- [x] Implement only these hooks/order. Public/native/candidate commands and receipt interpretation otherwise unchanged. Add actual local-server transport guard regression proving no upstream contact after guard failure, including count_tokens/scripted paths.
- [x] Run full lifecycle and relevant transport suites. Expected allpass. Baseline lifecycle76tests96subtests alreadyobserved. Ledger result; final source commit after whole-change gate/review to keep coupled changes together.

### Task 2: Owned supervisor and concrete paid backend

**Files:** Create ROOT/supervision.py,test_adapter_supervision.py,paid_backend.py,test_paid_backend.py. Reuse existing source-bound campaign.py/real_backend.py via imports; preserve old files. No paid main entry executed here.

**Interfaces:** Supervisor(policy,out/image context bound by runtime) supplies Task1 methods and final receipts. PaidBackend(Readiness).launch(row,reserve,cli)->receipt; reconcile(row,receipt,reserve)->Decimal; valid_public(row,receipt); cleanup(row)->daemon/absence proof. Existing validate/resources methods retained.

- [x] TDD host floors individually, unknown/malformed cgroup and groupOOM, exactimage/label/CID, final sample before cleanup, bounded log/truncation, actual public classifier distinction, ownedPopen subprocess abort, no arbitrary process/CID kill. Docker/HTTP boundaries may be faked; call real supervisor/runtime chain, not wholeattempt fakes.
- [x] TDD concrete backend calls through actual external.launch boundary under zero-provider fixtures, calibratedreconcile, actual candidate capture/health proofs and daemonabsence; source/inputdrift beforelaunch; injectedlateinfra stops campaign beforesecondattempt while preserving knowncost; no earlygrading method.
- [x] Implement new private modules and run allnew+relatedsuites. Read actual prepared configuration/profile/CLI metadata without credentials. Preserve5886realpreparedpaths/unstartedmarkers using existing population hashes.

### Task 3: Review, source freeze and actual common integration qualification

- [x] Run full required repo gate and targeted suites. Fresh whole-change reviewer on strongest available model applies at least12focusedmutations to isolatedcopies, reports observed summaries/exits/survivors; fileissues immediately. One offline RED→GREEN fix pass and exactmutationreapplication, no rereview.
- [x] Commit only reviewed productpaths by exactpathspec/Session-Id after gate. Freeze new runtime/privateadapter/nativeworker/policy/plugin/source/input hashes; preserve oldfrozenregistrations andresults.
- [x] One NEW disposable actual-CLI scripted common smoke through PaidBackend/Supervisor proves new call sites, public classification/continuation and native32GiB health/cleanup; raisingupstream/NoCredential enforce0provider. Reuse oldfixture scaffolding but not oldoutputdirs; no repeat of6oldwirecases/privatecontrols/onboarding. Ifinvalidretain+stop, no hiddenretry.
- [x] Archive allsource/tests/review/mutations/newnativeevidence with payload-by-payload verification, update report/memory and all admissiongates from actual artifacts. Deliver implemented adapter and truthful fresh readiness. Actual fixed-six paid activation and final private grading follow only when all required gates resolve; a missing gate leaves paidfalse, not a fabricated pass.


Closure2026-11-07: original148 preserved INVALID/archive76d403de...; no relabeling. Separately registered compatibility delta82c73e8aba7113bd closed its single new actual CLI14c and independent audit with healthy owned native proof, complete candidate, public failure->pass continuation and0provider/credential calls. New archive dc72f66c... verifies323payloads+manifest. Final paid-only registration/admission/preflight at /home/marius/.cache/codescout-resident/codescout/semantic-contract-paid-20261107 links every closed gate plus exact fe8 product/v2 private source. Full preflight passes with all six original configs/clones untouched and whole-cohort budget reserved. No repair efficacy conclusion; next is fixed-six execution, all costs/candidates before private grades.

## Rulings and execution method

User has opted out of implementation-method questions and delegated the end goal. Continue inline; don't turn routine choices into a permission request. Explicit supervisor API avoids global monkey-patching. Resident scratch replaces in-repo .superpowers helper paths to avoid earlier watcher/disk issues; keep explicit briefs/ledger/manifests. No automatic deletion of sealed evidence or old worktrees. Any scope gap is ledgered with its cost and resolved before launch.



Checkpoint2026-11-06: Tasks1/2 implemented; lifecycle85,transport32,supervisor12,backend19discoveredtestsPASS (backend11newmethods+8importedoldentrytests). Onefreshreview20actualmutants10killed10survived; oneofflinefixpassaddedtests only for those ten core guards, exact20reapplied20killed0surviving with verified import/byte provenance. Sixqualification-helpertestsPASS. Two scaffold issues corrected: post-fix finalnativegate/source-bytebinding, and actualPaidBackend.reconcile (scripted receipt deliberatelyrejected plus fourpreservedpaidreceiptpositiveoverlap/16protectedfiles). Gate13e stillrunning; finalsource-boundqualification, commit, freeze and one NEWactualCLIcommon smoke remain pending. No provider/credential/model/onboarding/realrepairlaunch. Newunion5899uniqueprotectedpaths exactlyold5886+CLI+12tempcontextfiles; allmatching/unstarted.
