"""Stock observations are positive evidence, never completeness or formatter permission."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

import pytest

SCRIPTS=Path(__file__).resolve().parents[1]/"scripts"
sys.path.insert(0,str(SCRIPTS))
from observation_contract import canonical_json, fingerprint
from observation_store import read_observations
from operation_observations import reconcile_observations, merge_scan_observations
from provenance_records import Operation, ProvenanceScan, CoverageGap

EPOCH="11111111-1111-4111-8111-111111111111"
PRODUCER="22222222-2222-4222-8222-222222222222"
def record(root,sequence=0,**changes):
    value=dict(schema_version=1,observed_at_unix_ms=1794210000000,
        observation_id=f"{PRODUCER}:{sequence}",producer_instance=PRODUCER,sequence=sequence,store_epoch=EPOCH,
        event="dispatch-observed",producer_kind="codescout-server",workspace=str(root),
        caller=dict(harness="codex",principal="codex:root",thread_id="child",agent_id=None),
        tool="create_file",local_invocation="server:connection:0",native_call_id="call",turn_id="turn",
        fingerprints=[fingerprint({"path":"a.rs","content":"PRIVATE_BODY"})],
        targets=[],outcome="dispatched",diagnostics=[])
    value.update(changes)
    return value

def target(root,phase="committed"):
    return dict(path=str(root/"a.rs"),phase=phase,bytes_len=3,
                fingerprint=dict(domain="bytes-v1",digest=hashlib.sha256(b"abc").hexdigest()))

def hook(root,sequence=1,child=None,**changes):
    value=record(root,sequence,event="hook-pre",producer_kind="codex-hook",local_invocation=None,
                 caller=dict(harness="codex",principal="codex:root",thread_id=child,agent_id=child),outcome="unknown")
    value.update(changes)
    return value

def store(root,records,enabled=True):
    subprocess.run(["git","init","-q",str(root)],check=True)
    base=root/".codescout"/"observations"
    base.mkdir(parents=True,mode=0o700,exist_ok=True);base.chmod(0o700)
    (base/".gitignore").write_bytes(b"*\n");(base/".gitignore").chmod(0o600)
    directory=base/"v1";directory.mkdir(mode=0o700,exist_ok=True)
    marker=root/".git"/"codescout-observation-policy-v1.json"
    marker.write_text(json.dumps(dict(schema_version=1,workspace=str(root),first_epoch=EPOCH)));marker.chmod(0o600)
    for name,value in [("control.json",dict(schema_version=1,workspace=str(root),epoch=EPOCH,enabled=enabled,producer_sequences={p:max(r["sequence"]+1 for r in records if r["producer_instance"]==p) for p in {r["producer_instance"] for r in records}})),("health.json",dict(diagnostics=[]))]:
        (directory/name).write_text(json.dumps(value));(directory/name).chmod(0o600)
    (directory/"quota.lock").touch(mode=0o600)
    data=b"".join(json.dumps(dict(record=r,checksum=hashlib.sha256(canonical_json(r).encode()).hexdigest()),ensure_ascii=False,separators=(",",":")).encode()+b"\n" for r in records)
    shard=directory/f"{PRODUCER}-000000.jsonl";shard.write_bytes(data);shard.chmod(0o600)
    return directory,shard,marker

def test_committed_facts_survive_error_and_post_refusal(tmp_path):
    r=record(tmp_path,targets=[target(tmp_path)],outcome="error-possible-effects")
    post=hook(tmp_path,event="hook-post",outcome="refused")
    result=reconcile_observations([r,post],[])
    assert len(result.writes)==1
    assert result.writes[0].path==str(tmp_path/"a.rs")
    assert result.writes[0].principal=="codex:root"
    assert len(result.pairs)==1
@pytest.mark.parametrize("outcomes", [("returned-success", "refused"), ("refused", "returned-success"), ("returned-success", "error-possible-effects")])
def test_conflicting_terminal_outcomes_retain_writes_and_uncertainty(tmp_path, outcomes):
    committed=record(tmp_path,targets=[target(tmp_path)])
    terminals=[record(tmp_path,i+1,event="result-observed",outcome=outcome) for i,outcome in enumerate(outcomes)]
    result=reconcile_observations([committed,*terminals,hook(tmp_path,3)],[])
    assert len(result.writes)==1
    assert result.writes[0].path==str(tmp_path/"a.rs")
    assert "context-conflict" in result.diagnostics
    assert result.gaps
    assert result.pairs==[]


def test_write_without_terminal_result_preserves_fact_and_missing_terminal_gap(tmp_path):
    request=record(tmp_path,event="request-observed",outcome="requested")
    committed=record(tmp_path,1,targets=[target(tmp_path)])
    result=reconcile_observations([request,committed],[])
    assert len(result.writes)==1
    assert "missing-terminal" in result.diagnostics
    assert any("terminal" in gap.reason for gap in result.gaps)


def test_repeated_equal_terminal_outcomes_remain_pairable(tmp_path):
    committed=record(tmp_path,targets=[target(tmp_path)])
    terminals=[record(tmp_path,i,event="result-observed",outcome="returned-success") for i in (1,2)]
    result=reconcile_observations([committed,*terminals,hook(tmp_path,3)],[])
    assert len(result.writes)==1
    assert len(result.pairs)==1
    assert result.diagnostics==[]
    assert result.gaps==[]

def test_replays_conflicts_and_repeated_calls_are_not_hash_deduplicated(tmp_path):
    a=record(tmp_path,targets=[target(tmp_path)])
    b=record(tmp_path,1,local_invocation="server:connection:1",native_call_id="second",targets=[target(tmp_path)])
    result=reconcile_observations([a,copy.deepcopy(a),b],[])
    assert len(result.writes)==2
    conflict=copy.deepcopy(a);conflict["caller"]["principal"]="codex:foreign"
    damaged=reconcile_observations([a,conflict],[])
    assert "conflicting-duplicate" in damaged.diagnostics
    assert damaged.gaps

@pytest.mark.parametrize("field",["root","thread","turn","call","workspace","tool","fingerprint"])
def test_native_pair_key_discriminates_every_dimension(tmp_path,field):
    a=record(tmp_path);b=hook(tmp_path,child="child")
    if field=="root":b["caller"]["principal"]="codex:fork"
    elif field=="thread":b["caller"]["thread_id"]="sibling";b["caller"]["agent_id"]="sibling"
    elif field=="workspace":b[field]=str(tmp_path/"foreign")
    elif field=="tool":b[field]="edit_file"
    elif field=="fingerprint":b["fingerprints"]=[dict(domain="bytes-v1",digest="a"*64)]
    else:b[{"turn":"turn_id","call":"native_call_id"}[field]]="other"
    assert not reconcile_observations([a,b],[]).pairs

def test_missing_child_requires_exactly_one_compatible_candidate(tmp_path):
    a=record(tmp_path);b=record(tmp_path,1,local_invocation="server:other:0",caller=dict(harness="codex",principal="codex:root",thread_id="sibling",agent_id=None))
    h=hook(tmp_path,2)
    assert len(reconcile_observations([a,h],[]).pairs)==1
    ambiguous=reconcile_observations([a,b,h],[])
    assert not ambiguous.pairs
    assert "ambiguous-pair" in ambiguous.diagnostics

def test_history_alias_requires_native_turn_thread_pin_and_hash_and_retains_body(tmp_path):
    r=record(tmp_path,targets=[target(tmp_path)])
    inp={"path":"a.rs","content":"PRIVATE_BODY"}
    op=Operation("rich-source","call","codex:root","child","2026-11-09T00:00:00Z",tmp_path,
                 "mcp__codescout__create_file",inp,"possible-write",True,True,None,
                 native_thread_id="child",native_turn_id="turn",native_workspace=str(tmp_path))
    result=reconcile_observations([r],[op])
    assert result.aliases[r["observation_id"]]==["rich-source"]
    assert op.input is inp and op.input["content"]=="PRIVATE_BODY"
    assert "PRIVATE_BODY" not in json.dumps([w.__dict__ for w in result.writes])
    incomplete=copy.copy(op);object.__setattr__(incomplete,"native_turn_id",None)
    assert not reconcile_observations([r],[incomplete]).aliases

def test_history_identity_is_read_once_per_operation_not_per_observation(tmp_path):
    reads=[]
    class Counted:
        source_id="rich-source"
        principal="codex:root"
        agent_id="child"
        native_turn_id="turn"
        native_workspace=str(tmp_path)
        name="mcp__codescout__create_file"
        input={"path":"a.rs","content":"PRIVATE_BODY"}
        def __init__(self,call):self.call_id=call
        @property
        def native_thread_id(self):
            reads.append(self.call_id)
            return "child"
    operations=[Counted(f"call-{i}") for i in range(20)]
    records=[record(tmp_path,i,native_call_id=f"call-{i}",local_invocation=f"server:connection:{i}") for i in range(20)]
    result=reconcile_observations(records,operations)
    assert result.aliases
    assert len(reads)==len(operations),"history keys must be indexed, not rebuilt for every record"

def test_two_local_invocations_with_one_native_key_cannot_borrow_one_history_alias(tmp_path):
    inp={"path":"a.rs","content":"PRIVATE_BODY"}
    op=Operation("rich-source","call","codex:root","child",None,tmp_path,"mcp__codescout__create_file",
                 inp,"possible-write",True,True,None,native_thread_id="child",native_turn_id="turn",native_workspace=str(tmp_path))
    a=record(tmp_path)
    b=record(tmp_path,1,local_invocation="server:other:0")
    result=reconcile_observations([a,b],[op])
    assert not result.aliases
    assert "ambiguous-pair" in result.diagnostics

def test_scan_never_launders_gaps_or_authorizes_from_hash_only(tmp_path):
    gap=CoverageGap("wrapper","opaque wrapper",tmp_path)
    baseline=ProvenanceScan({"a.rs":[("codex:root",None)]},{("a.rs","codex:root",None):["rich-body"]},[gap])
    own=record(tmp_path,targets=[target(tmp_path)])
    enriched=merge_scan_observations(baseline,reconcile_observations([own],[]),tmp_path)
    assert gap in enriched.gaps and enriched.evidence==baseline.evidence
    foreign=record(tmp_path,1,caller=dict(harness="codex",principal="codex:foreign",thread_id="foreign",agent_id=None),targets=[target(tmp_path)])
    restricted=merge_scan_observations(baseline,reconcile_observations([foreign],[]),tmp_path)
    assert restricted.owners==baseline.owners
    assert {w.principal for w in restricted.observed_writes}=={"codex:foreign"}
    assert restricted.gaps and restricted.evidence==baseline.evidence
    unknown=copy.deepcopy(own);unknown["caller"]["principal"]=None
    assert merge_scan_observations(baseline,reconcile_observations([unknown],[]),tmp_path).gaps
    empty=ProvenanceScan({}, {}, [])
    alone=merge_scan_observations(empty,reconcile_observations([own],[]),tmp_path)
    assert alone.gaps and not alone.evidence
    assert baseline.gaps==[gap] and baseline.owners=={"a.rs":[("codex:root",None)]}

def test_opaque_missing_terminal_and_uncertain_targets_keep_gaps(tmp_path):
    records=[record(tmp_path,tool="run_command",outcome="returned-success"),
             record(tmp_path,1,local_invocation="server:other:0",targets=[target(tmp_path,"uncertain")]),
             record(tmp_path,2,local_invocation="server:missing:0",outcome="missing-terminal",diagnostics=["missing-terminal"])]
    result=reconcile_observations(records,[])
    assert result.gaps and not result.writes
    assert any("opaque" in g.reason for g in result.gaps)

def test_completed_boundary_resolves_only_its_own_matching_attempt(tmp_path):
    a=record(tmp_path,targets=[target(tmp_path,"attempted")])
    b=record(tmp_path,1,targets=[target(tmp_path)])
    result=reconcile_observations([a,b],[])
    assert len(result.writes)==1
    assert not any("phase" in g.reason for g in result.gaps)
    mismatch=copy.deepcopy(b);mismatch["targets"][0]["fingerprint"]["digest"]="0"*64
    assert any("phase" in g.reason for g in reconcile_observations([a,mismatch],[]).gaps)

def test_default_disabled_reader_does_not_write_and_disabled_store_retains_facts(tmp_path):
    subprocess.run(["git","init","-q",str(tmp_path)],check=True)
    before=set(tmp_path.rglob("*"))
    batch=read_observations(tmp_path)
    assert batch.diagnostics==["disabled"] and set(tmp_path.rglob("*"))==before
    r=record(tmp_path,targets=[target(tmp_path)])
    store(tmp_path,[r],enabled=False)
    batch=read_observations(tmp_path)
    assert not batch.enabled and batch.records==[r]

def test_legacy_non_git_scan_without_observation_state_stays_disabled(tmp_path):
    assert read_observations(tmp_path).diagnostics==["disabled"]
    assert list(tmp_path.iterdir())==[]

def test_unsupported_platform_is_disabled_only_when_no_capture_state_exists(tmp_path,monkeypatch):
    subprocess.run(["git","init","-q",str(tmp_path)],check=True)
    monkeypatch.setattr(sys,"platform","win32")
    assert read_observations(tmp_path).diagnostics==["disabled"]
    store(tmp_path,[])
    assert read_observations(tmp_path).diagnostics==["unavailable"]

@pytest.mark.parametrize("damage,expected",[
    ("checksum","checksum"),("tail","torn-record"),("sequence","sequence-gap"),
    ("duplicate","conflicting-duplicate"),("schema","incompatible-schema"),
    ("symlink","unavailable"),("public","unavailable"),("reset","reset"),("new_epoch","reset")])
def test_reader_degrades_on_damage_without_writing(tmp_path,damage,expected):
    r=record(tmp_path,targets=[target(tmp_path)])
    directory,shard,marker=store(tmp_path,[r])
    if damage=="checksum":
        row=json.loads(shard.read_text());row["checksum"]="0"*64;shard.write_text(json.dumps(row)+"\n")
    elif damage=="tail":shard.write_bytes(shard.read_bytes()+b'{"record":')
    elif damage=="sequence":store(tmp_path,[record(tmp_path,2)])
    elif damage=="duplicate":
        conflict=copy.deepcopy(r);conflict["caller"]["principal"]="codex:other";store(tmp_path,[r,conflict])
    elif damage=="schema":r["schema_version"]=2;store(tmp_path,[r])
    elif damage=="symlink":saved=shard.with_suffix(".saved");shard.rename(saved);shard.symlink_to(saved)
    elif damage=="public":shard.chmod(0o644)
    elif damage=="reset":
        import shutil
        shutil.rmtree(directory.parent)
    elif damage=="new_epoch":
        value=json.loads((directory/"control.json").read_text());value["epoch"]=str(uuid.uuid4());(directory/"control.json").write_text(json.dumps(value))
    before={str(p):(p.lstat().st_mtime_ns,p.lstat().st_size) for p in tmp_path.rglob("*")}
    batch=read_observations(tmp_path)
    after={str(p):(p.lstat().st_mtime_ns,p.lstat().st_size) for p in tmp_path.rglob("*")}
    assert expected in batch.diagnostics
    assert before==after

def test_empty_shard_after_writer_death_is_incomplete_even_without_a_receipt(tmp_path):
    _,shard,_=store(tmp_path,[])
    assert shard.exists() and shard.stat().st_size==0
    assert "torn-record" in read_observations(tmp_path).diagnostics

def test_inspector_is_bounded_read_only_and_never_claims_completeness(tmp_path):
    store(tmp_path,[record(tmp_path,targets=[target(tmp_path)])])
    before={str(p):(p.lstat().st_mtime_ns,p.lstat().st_size) for p in tmp_path.rglob("*")}
    run=subprocess.run([sys.executable,str(SCRIPTS/"inspect-observations.py"),"--workspace",str(tmp_path),"--json"],capture_output=True,text=True,check=True)
    value=json.loads(run.stdout)
    assert value["coverage"]=="unknown" and len(value["writes"])==1
    assert "PRIVATE_BODY" not in run.stdout
    assert before=={str(p):(p.lstat().st_mtime_ns,p.lstat().st_size) for p in tmp_path.rglob("*")}
