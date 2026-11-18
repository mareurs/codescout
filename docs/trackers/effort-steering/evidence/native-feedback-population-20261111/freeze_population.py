"""Freeze public order/exposure/procedure before any new task qualification or actor."""
import collections
import hashlib
import json
from pathlib import Path
import tarfile

MAIN=Path("/home/marius/work/claude/codescout")
OUT=MAIN/"docs/trackers/effort-steering/evidence/native-feedback-population-20261111"
OLD=Path("/var/tmp/ep11-01a15/paid-campaign-headroom-v3/registration.json")
DATA=Path("/home/marius/work/claude/setup-ab-data")
SEED="native-public-feedback-20261111-v1"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    old_raw=OLD.read_bytes();old=json.loads(old_raw)
    descriptor=old["admission"]["population"]
    archive=Path(descriptor["path"]);packed_raw=archive.read_bytes()
    assert sha(packed_raw)==descriptor["sha256"]
    with tarfile.open(archive) as tar:
        original={role:tar.extractfile(name).read() for role,name in descriptor["members"].items()}
    metadata=json.loads(original["metadata"])["tasks"]
    old_status=json.loads(original["status"])
    exposure=set(old["exposure"])
    exposure.update(row["id"] for row in old["selected"])
    sources=[{"path":str(OLD),"sha256":sha(old_raw),"role":"closed pilot selected IDs and original exposure"},
             {"path":str(archive),"sha256":sha(packed_raw),"role":"original public metadata/status/exposure"}]
    payload={"original-metadata.json":original["metadata"],"original-status.json":original["status"],
             "original-exposure.json":original["exposure"]}
    registry_names=("fresh-build/records.jsonl","fresh-build-v2/pool/records.jsonl",
                    "fresh-screen-v2/live-screen/screen-plan.json",
                    "fresh-screen-v2-cs/live-screen-cs/screen-plan.json")
    snapshots=[]
    for name in registry_names:
        path=DATA/name;raw=path.read_bytes()
        if path.suffix==".jsonl":
            records=[json.loads(line) for line in raw.splitlines() if line.strip()]
            ids=[row["instance_id"] for row in records]
        else:
            records=json.loads(raw);ids=[row[0] for row in records]
        assert ids and all(isinstance(x,str) and x for x in ids)
        exposure.update(ids)
        snapshot={"path":str(path),"sha256":sha(raw),"listed_rows":len(records),
                  "exact_ids":sorted(set(ids)),"rule":"conservatively exclude all listed IDs,without reading actor outcomes"}
        snapshots.append(snapshot);sources.append({k:snapshot[k] for k in ("path","sha256")})
    aliases={"Tokio-rs__tokio-4384":"tokio-rs__tokio-4384",
             "sab-arm-pola-rs__polars-28484":"pola-rs__polars-28484",
             "sab-grade-pola-rs__polars-28484-gold-36":"pola-rs__polars-28484",
             "sab-grade-nushell__nushell-18863":"nushell__nushell-18863",
             "sab-grade-h4ckf0r0day__obscura-447-empty-57134719":"h4ckf0r0day__obscura-447",
             "sab-grade-h4ckf0r0day__obscura-447-gold-3173":"h4ckf0r0day__obscura-447"}
    exposure.update(value for key,value in aliases.items() if key in exposure)
    allowed={"id","repo","base_commit","image","language","public_issue_sha256","created_at"}
    assert len(metadata)==171 and len({r["id"] for r in metadata})==171
    assert all(set(r)<=allowed and {"id","repo","base_commit","image","language","public_issue_sha256"}<=set(r)
               for r in metadata)
    status={row["id"]:({"eligible":False,"reason":"retained earlier preparation exclusion: "+old_status[row["id"]]["reason"]}
            if old_status[row["id"]]["eligible"] is False else
            {"eligible":None,"reason":"unassessed under native-public-feedback-v1"}) for row in metadata}
    ranked=sorted(metadata,key=lambda r:(sha((SEED+"\0"+r["id"]).encode()),r["id"]))
    decisions=[{"id":r["id"],"repo":r["repo"],"rank_sha256":sha((SEED+"\0"+r["id"]).encode()),
                "reason":"research_exposure" if r["id"] in exposure else
                status[r["id"]]["reason"] if status[r["id"]]["eligible"] is False else "qualification_pending"}
               for r in ranked]
    rule={"schema":"native-feedback-public-selection-v1","seed":SEED,"order_seed":20261111,"task_count":4,
          "unit":"four canonical task IDs from distinct casefolded repositories",
          "rank":"ascending SHA256(seed+NUL+canonical_id),then canonical_id",
          "exposure":"original36IDs plus closed pair plus all listed peer-pool/study IDs;only explicit aliases",
          "selection":"qualify in frozen order;after a repository qualifies,skip later tasks of that repository;stop after four repositories",
          "ordinary_argv":["cargo","check","--offline","--workspace"],
          "broader_argv":["cargo","check","--offline","--workspace","--all-targets"],
          "workspace":"root Cargo.toml first;otherwise a unique top-level Rust workspace/package from public manifests/config;ambiguous is unsupported",
          "preparation":"source-only onboarding once per project/version;independent physical actor copies;actual CLI/global/project/Recon/tool/language/LSP/index delivery qualification",
          "acceptance_controls":"independent fixed private baseline-negative/reference/reference-repeat;both public compile scopes on baseline and reference",
          "preparation_exclusions":"retain each unsupported source/ambiguous workspace/failed baseline or reference/oracle reason before actors",
          "infrastructure":"disk/RAM/noexec/transport/toolchain crash/common harness failure stops preparation;do not resample around it",
          "dependency_bootstrap":"one source-bound generic preparation step may fetch locked dependencies/install declared toolchain before frozen offline checks;record exact commands/images/resource results",
          "forbidden":"selection by model success/failure,issue-keyword difficulty,gold implementation shape,or private repair feedback",
          "if_unavailable":"no shrink/replacement after paid outcomes;report if four repositories cannot qualify and explicitly version any population revision",
          "paid_admission":False}
    payload.update({"registry-exposure-snapshots.json":(json.dumps(snapshots,indent=2)+"\n").encode(),
                    "explicit-aliases.json":(json.dumps(aliases,indent=2)+"\n").encode(),
                    "exposure.json":(json.dumps({"tasks":sorted(exposure)},indent=2)+"\n").encode(),
                    "status-initial.json":(json.dumps(status,indent=2)+"\n").encode(),
                    "selection-rule.json":(json.dumps(rule,indent=2)+"\n").encode(),
                    "ranked-decisions.json":(json.dumps(decisions,indent=2)+"\n").encode()})
    for item in sources:
        assert sha(Path(item["path"]).read_bytes())==item["sha256"], item["path"]
    OUT.mkdir(exist_ok=False)
    for name,raw in payload.items():
        (OUT/name).write_bytes(raw)
        assert (OUT/name).read_bytes()==raw
    result={"schema":"native-feedback-population-policy-freeze-v1","state":"qualification_pending",
            "metadata_rows":len(metadata),"repositories":len({r["repo"].casefold() for r in metadata}),
            "exposure_ids":len(exposure),"universe_exposure_exclusions":sum(r["id"] in exposure for r in metadata),
            "initial_status_counts":dict(collections.Counter(str(r["eligible"]) for r in status.values())),
            "decision_counts":dict(collections.Counter(r["reason"] for r in decisions)),
            "sources":sources,"payload_sha256":{name:sha(raw) for name,raw in payload.items()},
            "procedure_source_sha256":sha(Path(__file__).read_bytes()),
            "mechanism_design_sha256":sha((MAIN/"docs/trackers/effort-steering/native-public-feedback-design-20261111.md").read_bytes()),
            "experimental_model_calls":0,"new_private_repair_outcomes_read":False,"paid_admitted":False}
    (OUT/"freeze.json").write_text(json.dumps(result,sort_keys=True,indent=2)+"\n")
    (OUT/"freeze_population.py").write_bytes(Path(__file__).read_bytes())
    first=[r for r in ranked if r["id"] not in exposure and status[r["id"]]["eligible"] is not False][:8]
    print(json.dumps({"freeze":{k:v for k,v in result.items() if k not in ("sources","payload_sha256")},
                      "first_unassessed_public_candidates":first},indent=2))


if __name__=="__main__":
    main()
