"""Retain exact author mutation evidence, including the hash-recovered pre-regression test."""
import ast
import hashlib
import io
import json
from pathlib import Path
import shutil
import tarfile

MAIN=Path("/home/marius/work/claude/codescout")
SRC=MAIN/".superpowers/sdd/2026-11-11-native-public-feedback-transfer/native-closed-mutation"
OUT=MAIN/"docs/trackers/effort-steering/evidence/native-feedback-closure-probe-20261111"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    before=json.loads((SRC/"before.json").read_bytes())
    after=json.loads((SRC/"after.json").read_bytes())
    name="tools/effort_protocol/test_feedback_runner.py"
    data=(SRC/"clone"/name).read_bytes()
    lines=data.decode().splitlines(keepends=True)
    tree=ast.parse(data)
    node=next(n for n in ast.walk(tree) if isinstance(n,ast.FunctionDef) and n.name==
              "test_native_grading_barrier_consumes_sixteen_slot_closure_and_rejects_truncation")
    restored=None
    for start in range(max(0,node.lineno-4),node.lineno):
        for end in range(node.end_lineno,min(len(lines),node.end_lineno+4)):
            if any(s.strip() for s in lines[start:node.lineno-1]+lines[node.end_lineno:end]):
                continue
            candidate="".join(lines[:start]+lines[end:]).encode()
            if digest(candidate)==before["source_before_after"][name]:
                restored=candidate
    assert restored is not None, "cannot recover exact baseline69 test source"
    original=(SRC/"original-native-campaign.py").read_bytes()
    assert digest(original)==before["source_before_after"]["tools/effort_protocol/native_campaign.py"]
    assert original.decode().count(before["find"])==1
    mutant=original.decode().replace(before["find"],before["replacement"]).encode()
    payload={"before-test_feedback_runner.py":restored,"applied-mutant-native_campaign.py":mutant}
    for p in SRC.rglob("*"):
        if p.is_file() and "__pycache__" not in p.parts:
            payload[str(p.relative_to(SRC))]=p.read_bytes()
    payload["producer.py"]=(SRC.parent/"probe_native_closed.py").read_bytes()
    for name,expected in after["source_before_after"].items():
        assert digest(payload["clone/"+name])==expected, name
    OUT.mkdir(exist_ok=False)
    archive=OUT/"evidence.tar.gz"
    with tarfile.open(archive,"x:gz") as tar:
        for name,raw in sorted(payload.items()):
            entry=tarfile.TarInfo(name);entry.size=len(raw);entry.mode=0o644
            tar.addfile(entry,io.BytesIO(raw))
    with tarfile.open(archive) as tar:
        assert len(tar.getmembers())==len(payload)
        for entry in tar:
            assert tar.extractfile(entry).read()==payload[entry.name]
    result={"schema":"closure-callsite-author-mutation-custody-v1","scope":"one author-applied site;not independent review",
            "before":{"baseline_tests":69,"baseline_rc":0,"mutant_tests":69,"mutant_rc":0,"observed":"survived"},
            "after":{"baseline_tests":70,"baseline_rc":0,"mutant_tests":70,"mutant_rc":1,"observed":"killed"},
            "pre_regression_source":"reconstructed by deleting only the added test;exact original SHA-256 verified",
            "repository_probe":"refused before applying:CLAUDE_CODE_SESSION_ID absent;its instructed scratch-clone fallback used",
            "members":{k:digest(v) for k,v in payload.items()},"archive_bytes":archive.stat().st_size,
            "archive_sha256":digest(archive.read_bytes()),"producer_sha256":digest(Path(__file__).read_bytes())}
    (OUT/"manifest.json").write_text(json.dumps(result,sort_keys=True,indent=2)+"\n")
    shutil.copyfile(Path(__file__),OUT/"seal_closure_probe.py")
    print(json.dumps({k:v for k,v in result.items() if k!="members"}))


if __name__=="__main__":
    main()
