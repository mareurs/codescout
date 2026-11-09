"""Cross-language observation boundaries; unknown evidence is not clearance."""
import copy
import hashlib
import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from observation_contract import canonical_json, fingerprint, decode_observation

FIXTURE = json.loads((Path(__file__).parent / "fixtures/operation-observations-v1.json").read_text())

def test_canonical_vectors():
    for case in FIXTURE["canonical"]:
        assert canonical_json(case["input"]) == case["canonical"]
        assert fingerprint(case["input"]) == {"domain": "canonical-json-safe-v1", "digest": hashlib.sha256(case["canonical"].encode()).hexdigest()}
    for value in [1.5, 9007199254740992, "\ud800"]:
        with pytest.raises(ValueError):
            canonical_json(value)

def test_strict_record():
    assert decode_observation(FIXTURE["record"]) == FIXTURE["record"]
    for patch in [{"schema_version":2}, {"body":"private-canary"}, {"sequence":-1}, {"native_call_id":"\0"}, {"native_call_id":"a"*1025}, {"tool":"credential-canary"}]:
        with pytest.raises(ValueError):
            decode_observation({**FIXTURE["record"], **patch})

def test_unknown_root_preserves_child():
    record=copy.deepcopy(FIXTURE["record"])
    record["caller"]={"harness":"codex", "principal":None, "thread_id":"child", "agent_id":None}
    assert decode_observation(record)["caller"]["principal"] is None
    record["caller"]["principal"]="child"
    with pytest.raises(ValueError):
        decode_observation(record)
def test_unknown_harness_never_claims_principal():
    record=copy.deepcopy(FIXTURE["record"])
    record["caller"]={"harness":"unknown","principal":None,"thread_id":None,"agent_id":None}
    assert decode_observation(record)["caller"]["principal"] is None
    record["caller"]["principal"]="codex:root"
    with pytest.raises(ValueError):
        decode_observation(record)
@pytest.mark.parametrize("cwd,valid", [(None,True),("/repo/src/../nested",True),("relative/path",False),("/bad\x00cwd",False),("/"+"a"*4096,False),(42,False)])
def test_native_cwd_is_optional_bounded_and_separate(tmp_path,cwd,valid):
    record=copy.deepcopy(FIXTURE["record"])
    record["native_cwd"]=cwd
    if valid:
        assert decode_observation(record)["native_cwd"]==cwd
        assert record["workspace"]==FIXTURE["record"]["workspace"]
    else:
        with pytest.raises(ValueError):decode_observation(record)
