---
id: '45d1f6a31cd3f883'
kind: research-note
status: archived
title: Containerd/SAB storage — read-only check 2026-11-01 ~15:53 UTC
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/containerd-sab-storage-20261101
original_sha256: 7576d81f9ce260ee3e0607268b73435ef17c29b3ba44963a9cdaaeb35454df40
---

# Containerd/SAB storage — read-only check 2026-11-01 ~15:53 UTC

User supplied a du-based snapshot grouping: new>=22700 118/273GB; middle1000..22699 1307/286GB; old<1000 146/61GB; snapshot613GB plus content120GB≈containerd732GB. Treat those as operator-reported dated measurements; this check did NOT recount/verify that population or total. Numerical age cut does not itself identify job ownership or reclaimability. User's mappings22734/22737→gitbutler and22750/22757→rust-sdk were explicitly guesses.

Observed with current Docker/ctr read-only APIs: Docker29.9.2 uses containerd /run/containerd/containerd.sock namespace moby, overlayfs snapshotter. Direct ctr user socket permission denied; sudo -n ctr read-only works. No deletion, prune, mount, new container or snapshot/view creation performed. `ctr snapshots mounts <display-only-target> <activekey>` PRINTS a mount command; do not execute it. Calls for committed snapshot keys fail precondition (not active/view), and do not yield physical snapshot directory IDs. Error's moby/internal-ID is not proof it is a physical fs snapshot ID.

At observation containerbf7044b46e67279959059de62695436779468a55e9ccef1dc009575af4c82388 is RUNNING, named sab-grade-pola-rs__polars-28484-gold-36e76fdb, image sha256:7c3dc11159176d897e936c66d79807984fa4bc514a6427fd1c393a932a0f4278 (sab-arm-pola-rs__polars-28484:f9b0fc9f8). Its ctr mounts display lists upperdir snapshot23069 and lowerdirs23068,23065,23047,23046,23045,23044,23043,23042,23023,23022,23021,22736,22735,22734,21297,21296,21295,21294,21293. Therefore22734 is directly confirmed as a dependency of the active Polars container; attributing it solely to Gitbutler by size is unsupported. Other named physical IDs were not mapped here.

Three distinct Polars images: task9c2488cd0028 11RootFSlayers, agent/run d72dde099cfd17, arm7c3dc1115917 18. Comparison of actual ordered RootFS arrays: all11 task layers are exact prefix of agent, all17 agent layers exact prefix of arm. Agent-cs and run tags share SAME imageIDd72dde099cfd. Docker list85.0/85.8/85.9GB sizes cannot be added as independent storage. Physical unique/reclaimable space NOT measured by those list values or prefix count.

Read-only derived RootFS chain keys for all7 selected Polars/Gitbutler/RustSDK images match real ctr committed snapshot names/parents. Committed mounts don't expose their physical IDs, so full largest-snapshot mapping incomplete. No need to mutate live snapshot metadata to force it. No SAB image deletion licensed by zero current containers alone: scheduled future controls/arms may depend on them. Current setup-ab memory was older than live container activity; live check supersedes its 'no batch running' only for this observation, without editing another campaign's memory.

Dated df-h /home:188GiB available90%used (~15:53UTC), down from cleanup's earlier224GiB/88%; concurrent activity, no causal attribution of exact difference to SAB measured. Preserve active SAB/images and all raw evidence. Later cleanup should be by exact proven-obsolete Docker image/build refs after job-use coordination, never rm snapshot directories or broad prune on active environment. Existing own effort campaign remains stopped/sealed, no newpaidcall.
