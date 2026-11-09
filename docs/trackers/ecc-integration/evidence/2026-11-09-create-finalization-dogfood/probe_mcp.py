"""Private real-MCP failure/normal controls; never opens the shared catalog."""
import hashlib
import importlib.util
from importlib.machinery import SourceFileLoader
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
helper_path=HERE.parent/"2026-11-09-doc-create-dogfood/transport-helper.py.txt"
loader=SourceFileLoader("finalization_mcp",str(helper_path))
spec=importlib.util.spec_from_loader(loader.name,loader)
helper=importlib.util.module_from_spec(spec);loader.exec_module(helper)
runtime=json.loads((HERE/"binary.json").read_text())
binary=Path(runtime["snapshot"])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==runtime["sha256"]
cases=[
("slug","BEFORE UPDATE OF slug ON artifact","simulated slug failure"),
("augmentation","BEFORE INSERT ON artifact_augmentation","simulated augmentation failure"),
("registration","BEFORE INSERT ON worktree_registration","simulated registration failure")]
records=[]
for name,trigger,marker in cases:
    with tempfile.TemporaryDirectory(prefix="create-finalization-",dir="/home/marius/.cache/codescout-ecc-tmp") as temporary:
        root=Path(temporary);main=root/"main";main.mkdir()
        subprocess.run(["git","init","-q",str(main)],check=True)
        config=main/".codescout";config.mkdir()
        (config/"project.toml").write_text('[project]\nname="finalization-fixture"\nlanguages=["python"]\n')
        (main/"fixture.py").write_text("VALUE = 1\n")
        subprocess.run(["git","add",".codescout/project.toml","fixture.py"],cwd=main,check=True)
        subprocess.run(["git","-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-qm","fixture"],cwd=main,check=True)
        project=main
        if name=="registration":
            project=root/"worktree"
            subprocess.run(["git","worktree","add","-q","-b","probe",str(project)],cwd=main,check=True)
        env=helper.isolated_server_env(root/"state")
        assert Path(env["LIBRARIAN_DB"]).is_relative_to(root)
        with helper.McpProbe(binary,project,env=env) as probe:
            conn=sqlite3.connect(env["LIBRARIAN_DB"])
            def state():
                return {table:conn.execute(f"SELECT count(*) FROM {table}").fetchone()[0] for table in ["artifact","artifact_augmentation","worktree_registration"]}
            before=state()
            conn.execute(f"CREATE TRIGGER fail_finalization {trigger} BEGIN SELECT RAISE(ABORT, '{marker}'); END;");conn.commit()
            args=dict(action="create",kind="tracker",title="Probe tracker",rel_path="pending/new.md",body="probe body",augment={"prompt":"maintain","params":{"entries":[]}},workspace=str(project))
            failed=probe.request("tools/call",{"name":"doc","arguments":args})
            after_failure=state()
            text="\n".join(b.get("text","") for b in failed.get("content",[]) if b.get("type")=="text")
            assert failed.get("isError") and marker in text,failed
            assert after_failure==before,(name,before,after_failure)
            assert not (project/"pending").exists(),name
            conn.execute("DROP TRIGGER fail_finalization");conn.commit()
            created=probe.request("tools/call",{"name":"doc","arguments":args})
            value=helper.payload(created)
            assert value.get("id") and not value.get("error"),value
            after_retry=state()
            assert after_retry["artifact"]==before["artifact"]+1
            assert after_retry["artifact_augmentation"]==before["artifact_augmentation"]+1
            assert after_retry["worktree_registration"]==before["worktree_registration"]+(1 if name=="registration" and not before["worktree_registration"] else 0)
            assert (project/"pending/new.md").is_file()
            read=probe.request("tools/call",{"name":"doc","arguments":dict(action="get",id=value["id"],full=True,workspace=str(project))})
            read_value=helper.payload(read)
            assert "probe body" in read_value.get("body",""),read_value
            slug=conn.execute("SELECT slug FROM artifact WHERE id=?",(value["id"],)).fetchone()[0]
            assert slug=="probe-tracker",slug
            records.append(dict(case=name,linked_worktree=name=="registration",before=before,after_failure=after_failure,after_retry=after_retry,file_absent_after_failure=True,successful_retry_read=True,slug=slug,responses=probe.responses))
            conn.close()
(HERE/"mcp-results.json").write_text(json.dumps(dict(binary=runtime,cases=records,scope="Current-source private binary. SQLite-trigger failure + clean retry controls; no no-ECC model run or shared catalog."),indent=2)+"\n")
print(json.dumps(dict(cases=len(records),failed_calls_rolled_back=3,retries_created_and_read=3)))
