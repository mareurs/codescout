---
id: b62b6139a3039f83
kind: research-note
status: archived
title: Docker dependencies — operator-relayed peer response, 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/docker-peer-dependencies-20261106
original_sha256: 7134d1a5c5120edf9c752662b50bbb2bf06ec92ec32608586d5729d3de052548
---

# Docker dependencies — operator-relayed peer response, 2026-11-06

User relayed Claude session reply to earlier SAB dependency question. Treat these as peer-reported ownership/dependency facts, not freshly inspected Docker state and not permission to delete.

Peer says15sab-tpl-* volumes are not theirs: cs-p01,p04,p07,p08,p15,p19,ruff-15309,ruff-15626,ripgrep-2576,nushell-13831,bat-1892,axum-691,tokio-4384,tokio-6603,coreutils-6575. No sab-tpl image. Prior Stage1/3 bundles bundle.json and stage1-registration.json/stage3-registration.json reference templates; earlier codescout/full arms or comparisons/extensions need them. Rebuild cost unmeasured; retain.

Keep images for registered fresh pool: current NU18863 and RA22992 plus RA23586/23115 and NU19283/19172/18976. Registration pins run and arm image IDs (sab-task-/sab-agent-cs-/sab-run-/sab-arm-/sab-fresh- families).

Peer identifies unregistered candidate images: RA23096:pilot and dropped23096, NU18979, gate-failedRA22852/NU19394/NU19352. Rebuildable but costs hours; peer said safe for their work if operator chooses, not generic deletion approval. Inspect actual references/IDs and preserve required records before any removal.

Keep bases sab-base-rust-analyzer-rs1991:f9b0fc9f8 and sab-base-nushell-tc:f9b0fc9f8 for future fresh builds. No new Docker cleanup done during /tmp diagnosis; all templates/registered images/bases retained.
