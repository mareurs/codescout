---
id: '4ddfe91d022d5919'
kind: research-note
status: archived
title: Fixed checker loop verifier — 2026-10-31
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/fixed-checker-loop-verifier
original_sha256: 887f66bf7b036229fe8bbdfe9f617b65df1d2e2c429684dbb1dbb71d36f92787
---

# Fixed checker loop verifier — 2026-10-31

This is the exact independently rerun closure program captured in the U task-2 completion ledger. Run it with argv `verify` only, using Python -B. It reads registered runtime/source/model/grade/cost and complete physical/symlink/Git manifests; it launches no model. Never call seal again or alter the closed root. The native task-2 log recorded verified-closed for608 physical files,7098 symlink path-to-target records and12 committed files. OldS overlap19841 files+9 Git blobs, ratio1; oldN separately verified18239+8.

Copy this code to a temporary path outside the sealed root if needed; command: python3 -B /tmp/fc31-verify.py verify. The worktree and prior roots must remain available at their recorded paths. Current branch HEAD need not equal the old commit during verify; committed object bytes/parent remain checked.

```python
import hashlib, importlib.util, json, os, subprocess, sys
from decimal import Decimal as D
from pathlib import Path
R=Path("/home/marius/.cache/codescout-resident/codescout/fixed-checker-loop-20261031")
U=Path("/home/marius/.codex/worktrees/9369/codescout")
sys.path.insert(0,str(R))
spec=importlib.util.spec_from_file_location("fc_close_workflow",R/"workflow.py")
w=importlib.util.module_from_spec(spec);spec.loader.exec_module(w);e=w.e
account=w.load("fc_close_raw_account",w.OLD/"analysis/account.py")
git=lambda *args:subprocess.run(["git","-C",str(U),*args],capture_output=True,check=True).stdout
sha=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()

def scan(root):
    files={};links={}
    for path in sorted(root.rglob("*",recurse_symlinks=False)):
        rel=str(path.relative_to(root))
        if "__pycache__" in path.parts or rel in ("final-outcome.json","seal-verification.log"):continue
        if path.is_symlink():links[rel]=os.readlink(path)
        elif path.is_file():files[rel]=sha(path)
    return files,links

def reconcile():
    e.frozen()
    analysis=e.read(R/"analysis.json")
    account.ORDER=w.ORDER
    charges=account.charges(R)
    states=e.states()
    assert charges==analysis["charges"] and states==analysis["states"]
    assert w.summary(states)==analysis["result"]
    assert sum(map(D,charges.values()),D(0))==D(analysis["phase_usd"])
    assert analysis["result"]["supported"] and analysis["result"]["budget_valid"] and analysis["result"]["evidence_valid"]
    return analysis

def prior_calibration():
    old=e.read(w.OLD/"final-outcome.json")
    files,_=scan(w.OLD)
    common={path:value for path,value in files.items() if Path(path).name not in ("final-outcome.json","seal-verification.log")}
    assert common==old["files"],"Prior physical evidence changed"
    extras=sorted(set(files)-set(old["files"]))
    assert all(Path(path).name in ("final-outcome.json","seal-verification.log") for path in extras)
    for path,value in old["git_files"].items():
        assert hashlib.sha256(git("show",old["commit"]+":"+path)).hexdigest()==value
    assert git("rev-parse",old["commit"]+"^").decode().strip()==old["parent"]
    return dict(prior_physical_files=len(common),prior_git_files=len(old["git_files"]),overlap_ratio="1",
                extra_nested_files_in_new_predicate=extras,prior_seal_sha256=sha(w.OLD/"final-outcome.json"),
                prior_registered_source_states_recomputed=e.read(R/"preceding-S-summary-verified.json")["evidence_valid"])

def verify():
    seal=e.read(R/"final-outcome.json")
    files,links=scan(R)
    assert files==seal["files"] and links==seal["symlinks"],"Evidence population or bytes changed"
    assert reconcile()==seal["analysis"]
    assert prior_calibration()==seal["prior_calibration"]
    assert git("rev-parse",seal["commit"]+"^").decode().strip()==seal["parent"]
    for path,value in seal["git_files"].items():
        assert hashlib.sha256(git("show",seal["commit"]+":"+path)).hexdigest()==value
    print(json.dumps(dict(status="verified-closed",commit=seal["commit"],patch_id=seal["patch_id"],
        verified_physical_files=len(files),verified_symlink_targets=len(links),verified_git_files=len(seal["git_files"]),
        cumulative_client_usd=seal["analysis"]["cumulative_client_usd"],
        remaining_client_usd=seal["analysis"]["remaining_client_usd"])))

def seal():
    assert not (R/"final-outcome.json").exists(),"Never overwrite a closed phase"
    analysis=reconcile();prior=prior_calibration()
    e.a.save(R/"prior-physical-calibration.json",prior)
    publication=e.read(R/"postcommit-verification.json")
    assert git("rev-parse","HEAD").decode().strip()==publication["commit"]
    assert not git("status","--porcelain") and not git("diff","--cached","--name-only")
    assert publication["parent"]=="c707df401d19df3bf0ca3c5117653ea054ff333b"
    assert git("diff-tree","--no-commit-id","--name-only","-r",publication["commit"]).decode().splitlines()==list(publication["files"])
    files,links=scan(R)
    value=dict(status="closed",analysis=analysis,analysis_sha256=sha(R/"analysis.json"),
        cumulative_client_usd=analysis["cumulative_client_usd"],remaining_client_usd=analysis["remaining_client_usd"],
        commit=publication["commit"],parent=publication["parent"],patch_id=publication["patch_id"],
        native_sid=publication["native_sid"],git_files=publication["files"],files=files,symlinks=links,
        prior_calibration=prior,
        exclusions="Root final-outcome.json and repeatable root seal-verification.log; __pycache__ directories. Symlink target strings recorded separately; selected runtime/input/source bytes independently bound by registration and captures.")
    e.a.save(R/"final-outcome.json",value)
    verify()

seal() if sys.argv[1]=="seal" else verify()

```



## Ownership and re-check — 2026-11-02

Ownership: no session claimed this file. At the operator's request, session `1a35461f-23c2-41e6-ba35-052481f7dd96` (Claude, `.claude-stef`) took it over on 2026-11-02. That session did not write the program.

Verified on 2026-11-02 with a separate read-only script. That script did NOT run the program below, because the program executes `workflow.py` from the sealed directory.
- `final-outcome.json` has status `closed`, commit `e3daa380d87dcf663cc0f32e099afe199eba2bee`, parent `c707df401d19df3bf0ca3c5117653ea054ff333b`.
- 608 physical files and 7,098 symlink records are present, with 0 hash mismatches and 0 target mismatches. The counts equal the ones above.
- 12 git blobs match the seal, and the parent matches.
- `workflow.py` finds the prior root as `R.parent/'snapshot-workflow-20261031'`. That directory exists at the new location, `/home/marius/.cache/codescout-resident/codescout/`. The worktree `/home/marius/.codex/worktrees/9369/codescout` exists.

Not done: the program's own `verify` run, the overlap numbers (oldS 19841 files + 9 blobs, oldN 18239 + 8), and the "native task-2 log" that the first paragraph cites.
