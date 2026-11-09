"""Opt-in native CLI/MCP acceptance: supply the freshly built isolated executable."""
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time

import pytest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"scripts"))
from observation_store import read_observations
from operation_observations import reconcile_observations

BINARY=os.environ.get("CODESCOUT_OBSERVATION_TEST_BINARY")
pytestmark=pytest.mark.skipif(not BINARY,reason="native acceptance requires an explicitly freshly built CODESCOUT_OBSERVATION_TEST_BINARY")

class Server:
    def __init__(self,root,env):
        self.root=root;self.env=env;self.n=0;self.messages=queue.Queue()
        self.errors=(root.parent/"mcp-errors.log").open("w")
        self.process=subprocess.Popen([BINARY,"start","--project",str(root)],cwd=root,env=env,
                                      stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.errors,text=True)
        def incoming():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        self.reader=threading.Thread(target=incoming,daemon=True);self.reader.start()
        self.request("initialize",dict(protocolVersion="2024-11-05",capabilities={},
                     clientInfo=dict(name="codex-mcp-client",version="0.167.0")))
        self.send(dict(jsonrpc="2.0",method="notifications/initialized"))

    def send(self,message):
        self.process.stdin.write(json.dumps(message)+"\n");self.process.stdin.flush()

    def begin(self,name,arguments,meta=None):
        self.n+=1
        params=dict(name=name,arguments=arguments)
        if meta is not None:params["_meta"]=meta
        self.send(dict(jsonrpc="2.0",id=self.n,method="tools/call",params=params))
        return self.n

    def receive(self,ident,timeout=20):
        deadline=time.monotonic()+timeout
        while True:
            value=self.messages.get(timeout=max(0.01,deadline-time.monotonic()))
            if value.get("id")==ident:
                assert "error" not in value,value
                return value["result"]

    def request(self,method,params):
        self.n+=1;self.send(dict(jsonrpc="2.0",id=self.n,method=method,params=params))
        return self.receive(self.n)

    def call(self,name,arguments,meta=None):
        return self.receive(self.begin(name,arguments,meta))

    def close(self):
        self.process.stdin.close()
        try:self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.terminate();self.process.wait(timeout=5)
        self.errors.close()

def meta(call="call",root="native-root",child="native-child"):
    value=dict(threadId=child,callId=call,**{"x-codex-turn-metadata":dict(thread_id=child,turn_id="turn")})
    if root is not None:
        value["sessionId"]=root;value["x-codex-turn-metadata"]["session_id"]=root
    return value

@pytest.fixture
def native(tmp_path):
    root=tmp_path/"checkout";root.mkdir()
    subprocess.run(["git","init","-q",str(root)],check=True)
    home=tmp_path/"home";home.mkdir()
    env={k:v for k,v in os.environ.items() if not k.startswith(("GIT_","CLAUDE_CODE_SESSION","CODEX_SESSION","CODEX_THREAD","CODESCOUT_CONTEXT"))}
    env.update(HOME=str(home),XDG_STATE_HOME=str(home/"state"),XDG_CONFIG_HOME=str(home/"config"),
               CODESCOUT_SQLITE_DIR=str(tmp_path/"db"),CODESCOUT_ALLOW_TEMP_WORKSPACE="1",
               CODESCOUT_VECTOR_BACKEND="sqlite-vec",CODESCOUT_EMBEDDER_URL="http://127.0.0.1:1",
               CODEX_SESSION_ID="ambient-wrong-root")
    server=Server(root,env)
    try:yield root,env,server
    finally:server.close()

def cli(root,env,action):
    result=subprocess.run([BINARY,"observations",action,"--workspace",str(root),"--json"],env=env,
                           text=True,capture_output=True,check=True)
    return json.loads(result.stdout)

def committed(root):
    batch=read_observations(root)
    return batch,reconcile_observations(batch.records,[])

def test_cli_server_actual_targets_unknown_root_refusal_and_hook_pair(native):
    root,env,server=native
    cli(root,env,"enable")
    payload=dict(workspace=str(root),path="native.txt",content="NATIVE_PRIVATE_BODY")
    result=server.call("create_file",payload,meta())
    assert not result.get("isError")
    batch,reconciliation=committed(root)
    writes=[w for w in reconciliation.writes if w.path==str(root/"native.txt")]
    assert len(writes)==1 and writes[0].principal=="codex:native-root"
    assert writes[0].thread_id=="native-child"
    assert writes[0].fingerprint["digest"]==hashlib.sha256(b"NATIVE_PRIVATE_BODY").hexdigest()
    api=server.call("create_file",dict(workspace=str(root),path="api.txt",content="API_PRIVATE_BODY"),meta("api",None,"api-child"))
    assert not api.get("isError")
    _,reconciliation=committed(root)
    write=next(w for w in reconciliation.writes if w.path==str(root/"api.txt"))
    assert write.principal is None and write.thread_id=="api-child"
    refusal=server.call("create_file",payload,meta("refused"))
    assert refusal.get("isError") is False
    batch,_=committed(root)
    assert any(r["native_call_id"]=="refused" and r["outcome"]=="refused" for r in batch.records)
    assert not any(r["native_call_id"]=="refused" and any(t["phase"]=="committed" for t in r["targets"]) for r in batch.records)
    serialized=json.dumps(batch.records)
    assert "PRIVATE_BODY" not in serialized and "ambient-wrong-root" not in serialized

def test_arbitrary_shell_effect_read_partial_error_and_cancellation_are_opaque(native):
    root,env,server=native
    cli(root,env,"enable")
    result=server.call("run_command",dict(workspace=str(root),command="printf private-shell-body > partial.txt; exit 7",effect="read"),meta("partial"))
    assert (root/"partial.txt").read_text()=="private-shell-body"
    batch,reconciliation=committed(root)
    assert any(r["native_call_id"]=="partial" and "opaque-effects" in r["diagnostics"] for r in batch.records)
    assert any("opaque" in gap.reason for gap in reconciliation.gaps)
    assert not any(w.path==str(root/"partial.txt") for w in reconciliation.writes)
    ident=server.begin("run_command",dict(workspace=str(root),command="printf started > cancel-start.txt; sleep 4; printf ended > cancel-end.txt",effect="read"),meta("cancel"))
    deadline=time.monotonic()+10
    while not (root/"cancel-start.txt").exists() and time.monotonic()<deadline:time.sleep(0.02)
    assert (root/"cancel-start.txt").exists()
    server.send(dict(jsonrpc="2.0",method="notifications/cancelled",params=dict(requestId=ident,reason="fixture cancellation")))
    deadline=time.monotonic()+3
    while time.monotonic()<deadline:
        batch=read_observations(root)
        if any(r["native_call_id"]=="cancel" and r["outcome"]=="missing-terminal" for r in batch.records):break
        time.sleep(0.02)
    assert any(r["native_call_id"]=="cancel" and r["outcome"]=="missing-terminal" for r in batch.records)
    assert "private-shell-body" not in json.dumps(batch.records)
    # Cancellation is not proof that descendants cannot finish later.

def test_default_disabled_and_observer_failure_preserve_real_effects(native):
    root,env,server=native
    server.call("create_file",dict(workspace=str(root),path="disabled.txt",content="works"),meta("disabled"))
    assert (root/"disabled.txt").read_text()=="works"
    assert not (root/".codescout/observations").exists()
    cli(root,env,"enable")
    (root/".codescout/observations/v1/quota.lock").unlink()
    result=server.call("create_file",dict(workspace=str(root),path="unavailable.txt",content="still works"),meta("unavailable"))
    assert result.get("isError") is False
    assert (root/"unavailable.txt").read_text()=="still works"
    assert "unavailable" in read_observations(root).diagnostics

def test_full_capture_store_preserves_the_original_tool_success(native):
    root,env,server=native
    cli(root,env,"enable")
    budget=root/".codescout/observations/v1/budget-fixture.bin"
    with budget.open("wb") as stream:stream.truncate(64*1024*1024)
    budget.chmod(0o600)
    result=server.call("create_file",dict(workspace=str(root),path="full-store.txt",content="original success"),meta("full"))
    assert result.get("isError") is False
    assert (root/"full-store.txt").read_text()=="original success"
    assert "quota" in read_observations(root).diagnostics

def test_cli_disable_torn_and_removed_state_remain_diagnostic(native):
    root,env,server=native
    cli(root,env,"enable")
    server.call("create_file",dict(workspace=str(root),path="before.txt",content="before"),meta("before"))
    cli(root,env,"disable")
    batch=read_observations(root);assert batch.records and not batch.enabled
    shard=next((root/".codescout/observations/v1").glob("*.jsonl"))
    with shard.open("ab") as stream:stream.write(b'{"record":')
    assert "torn-record" in read_observations(root).diagnostics
    shutil.rmtree(root/".codescout/observations")
    assert "reset" in read_observations(root).diagnostics
    cli(root,env,"enable")
    assert "reset" in read_observations(root).diagnostics

def test_real_node_adapter_cli_default_disabled_then_ingests_allowlisted_hook(native):
    root,env,server=native
    plugin=os.environ.get("CODESCOUT_OBSERVATION_PLUGIN_ROOT")
    if not plugin:pytest.skip("cross-repo Node adapter requires CODESCOUT_OBSERVATION_PLUGIN_ROOT")
    adapter=Path(plugin)/"hooks/codex-observation.mjs"
    payload=dict(session_id="native-root",agent_id="native-child",turn_id="turn",cwd=str(root),
                 hook_event_name="PreToolUse",tool_name="mcp__codescout__create_file",tool_use_id="call",
                 tool_input=dict(workspace=str(root),path="paired.txt",content="HOOK_PRIVATE_BODY"),
                 secret="EXTRA_PRIVATE_CANARY")
    args=["node",str(adapter),"--event","PreToolUse","--codescout",BINARY]
    nested=root/"nested";nested.mkdir()
    def invoke(cwd=root):
        result=subprocess.run(args,cwd=cwd,env=env,input=json.dumps(payload),capture_output=True,text=True,timeout=3)
        assert result.returncode==0 and result.stdout=="{}\n" and result.stderr==""
    invoke();assert not (root/".codescout/observations").exists()
    cli(root,env,"enable");invoke()
    payload["cwd"]=str(nested/".."/"nested");invoke(nested)
    server.call("create_file",payload["tool_input"],meta())
    batch,reconciliation=committed(root)
    hooks=[r for r in batch.records if r["producer_kind"]=="codex-hook"]
    assert len(hooks)==2
    assert {r["workspace"] for r in hooks}=={str(root)}
    assert {r["native_cwd"] for r in hooks}=={str(root),str(nested/".."/"nested")}
    assert reconciliation.pairs
    assert "PRIVATE" not in json.dumps(batch.records)
