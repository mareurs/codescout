"""Check literal write-result claims against the immutable private MCP binary."""
from pathlib import Path
import json,tempfile
from run_base import helper,fixture,static_and_nav,SCRATCH,HERE
binary=Path("/home/marius/work/claude/codescout/.codescout/ecc-routing-private-20261109/codescout")
static,nav=static_and_nav()
with tempfile.TemporaryDirectory(prefix="routing-write-shape-",dir=SCRATCH) as tmp:
    root=Path(tmp);project=root/"project";fixture(project,nav)
    with helper.McpProbe(binary,project,env=helper.isolated_server_env(root/"state")) as probe:
        created=probe.request("tools/call",{"name":"create_file","arguments":{"path":"witness.txt","content":"fixture-content-not-to-echo","workspace":str(project)}})
        doc=probe.request("tools/call",{"name":"doc","arguments":{"action":"create","kind":"note","title":"Write shape witness","rel_path":"docs/witness.md","body":"fixture-body-not-to-echo","workspace":str(project)}})
        records=probe.responses
    (HERE/"write-result-probe.json").write_text(json.dumps({"responses":records},indent=2)+"\n")
    assert not created.get("isError") and not doc.get("isError")
    value=json.loads(doc["content"][0]["text"])
    assert isinstance(value,dict) and "id" in value,value
    assert "fixture-body-not-to-echo" not in doc["content"][0]["text"]
    print(json.dumps({"create_file":created["content"][0]["text"],"doc_create_keys":sorted(value),"doc_create_is_metadata":True}))
