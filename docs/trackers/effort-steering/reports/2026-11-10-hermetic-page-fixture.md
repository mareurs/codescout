---
id: d39a2a30be949b85
kind: report
status: active
title: Hermetic page fixture — native discrimination and source parity
tags:
- effort-steering
- evaluation
- source-parity
topic: effort-steering
---

# Hermetic page fixture and source parity

## Full331controls — 2026-11-10

[Fullv4registration](../evidence/full-obscura-fixture-v4-20261110/registration.json) preserves original2F2P/329P2P and literal commands. Actual base3fail/reference1fail,0missing,46.0s each: the extra P2P is resolver_does_not_ssrf_block_public_host. Mapping example.com toloopback changed the hostname used by that unchanged security test. Complete streams,source/service bytes and cleanup pass; the fixture itself fails compatibility. Both controls remain retained; reference-repeat explicitly notexecuted after this diagnosed configured collision. [Issue](../../../issues/2026-11-10-page-fixture-hostname-collides-with-ssrf-test.md).

[Fullv5registration](../evidence/full-obscura-fixture-v5-20261110/registration.json) uses fixture.example.com for the local page and leaves example.com untouched. All assertions/expected arrays/literal commands remain unchanged; child-only private-network opt-in retained. Actual v5base:exactly2originalF2Pfail,329P2Ppass,0missing,44.6s. Reference:331/331pass,0missing,47.0s. Both source/service bytes,complete streams and cleanup pass. Reference-repeat:331/331pass,0missing,43.6s. Root verification confirms identical reference/repeat selected maps,all original commands/assertions/expected names,exact source/service/runtime,complete streams and fresh absence. Common actor binding and independent whole-admission review remain open. Original v4failures are not rescored.

## Native fixture discrimination — 2026-11-10

**Historical v3diagnostic:** the16-test root-host mapping below is superseded for full-suite use by v5fixture.example.com after v4exposed the SSRFhostnamecollision. Retain these control outcomes; use Full331controls for the current fixture.

Fresh known-reference16MCPcontrols use fixed local HTTP HTML,networknone and example.com→127.0.0.1 only inside their own containers. Serviceoff:13pass/3fail,0missing,25.2s. Correct page:16pass,0missing,20.0s. Wrong page without h1:15pass/selectorfail,0missing,22.3s. Every original assertion is unchanged; only7navigation URL literals and spawned MCP child's local-network opt-in differ. Cargo and unrelated tests receive no global private-network opt-in. This HTTP fixture does not qualify TLS behavior.

[Registration](../evidence/hermetic-page-fixture-v3-20261110/registration.json) binds source,test/service/helper/image and exact commands. Root verification reparses all3logs,requalifies complete streams and checks actual final source/service bytes,registered memory/CPU/init/capability/pids,networknone,private/etc/hosts,global opt-in absence,no host binds,OOM0 and fresh container absence. Service request logs contain0/8/8entries respectively; the8include the controller readiness request. The page is static and never derived from candidate/test outcomes.

## Source-image mismatch and explicit baseline

V2 had the expected3functional failures with serviceoff but correctly refused final source bytes. Actual image base already differs from public preparation. A phase probe records bootstrap bytes before reference application,after the ORIGINAL reference and after a successful rebuild: build does not change the source. Public preparation exported canonical base_commit with git archive, whereas the task image has a2195byte tracked delta:3NetworkInformationmethods and runtime.rs test-helper run_page_init calls. [Issue and exact evidence](../../../issues/2026-11-10-obscura-evaluator-source-byte-binding-mismatch.md).

V3 explicitly derives expected bytes from canonical public source + this immutable-image public delta + ORIGINAL reference. No evaluator production code is changed. Final hashes now match in every fixture control, including both image-dirty source files and the public MCP fixture. A read-only full indexed-path audit measures133Obscuraimagepaths:130match old preparation,3differ(.gitignore metadata plus2sourcefiles). The public preparation's extra gitignore entry is .codescout/private-memories/. Untracked/transitive runtime files are not covered by this predicate.

Initial Lean inventories stopped on mode160000 gitlink aur/lean-ctx-bin before producing a count; their refusals remain preserved. The later Gitlink-aware inventory and separate canonical-lineage calibration below now cover that empty Gitlink and all canonical physical paths. This does not qualify untracked/transitive runtime or common actor context.


## Checked diagnostic custody

[Closure](../evidence/hermetic-page-fixture-v3-20261110/closure.json) preserves all source/test/service/controller versions,complete native v2/v3streams,readiness/request/runtime observations,actual image/apply/build bytes,read-only tracked-diff and indexed-path audits,failed registration and unsupported-Gitlink refusals. verification.tar.gz contains160regular members,1128565bytes,SHA25691b2a7f3ae3ae72d837829fe0cd43f170ffdf6c1bd2f7047daefcb5748af6d61. Every member reopened byte-equal and original bytes unchanged during sealing. The initial source-probe absence predicate's false result remains preserved alongside an independently verified absence follow-up. No source product gate was repeated for this private-only slice; no product source changed and whole admission/product integration remain pending.


## Checked full-population custody — 2026-11-10

[Fullv5closure](../evidence/full-obscura-fixture-v5-20261110/closure.json) preserves all3full331controls and failedv4controls/explicitstoppingrecord,source/test/service versions,source/parity inventories,Gitlink-aware Lean follow-up and original-reference application. verification.tar.gz contains214regular members,1797388bytes,SHA256ef33dbccbe796dfbdb22ea6baeef603ecbb9955da6fa2b58836c607505e57b20. All members reopened byte-equal and originals unchanged during seal. A separate image-identity follow-up checks actual immutable image IDs in12inspectionrecords across the6Lean+Obscurafullcontrols; this is not whole transitive actor qualification.

The repaired Lean inventory reads2637indexedpaths,2636match oldpreparation and only.gitignore metadata differs. One emptyGitlink aur/lean-ctx-bin is included as logical metadata; a separate native Git probe calibrates its index/HEAD object against registered canonical base9910a9f08627360ab6c6c784bd496ca4d5cfbee8. All2636physical canonical export paths equal the image physical-path set; Gitlink is empty both sides and pointerb1d40ed0265616739f1d6d330ff7d8de3758ae22matches original base. Untracked/transitive-runtime scope remains excluded. Earlier unsupported-mode refusals stay preserved.

## Rulings and remaining qualification

Preserve the actual image baseline as an explicit public delta shared by future actors/evaluator. A new public preparation/index/runtime version must include that delta and the SAME page fixture; evaluator-only success cannot admit a paid arm. Reuse existing source-only onboarding instead of repeating LLM onboarding; changed-file/index/source binding still needs verification. Original source preparation,dataset,task pair,expected arrays and old failed controls remain intact.

The [plan](../plans/2026-11-10-hermetic-page-fixture.md) Task1 is verified. Task2 remains open: full331original expected base/reference/reference-repeat; common public actor binding; whole-admission independent review. HTTP substitution explicitly narrows TLS scope; no private oracle/gold goes to actors. Host resource floor,F6linker classification and all-arm/controller/accounting remain parent blockers. No new experiment model repair or paid admission.
