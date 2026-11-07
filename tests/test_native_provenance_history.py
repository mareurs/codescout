import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"scripts"))
import test_native_provenance_policy as policy_fixture
from codex_history import read_codex_history
from native_provenance_history import apply_checkpoint
from test_native_provenance_fence import FenceProofTests as _FenceFixture
del _FenceFixture

@unittest.skipUnless(sys.platform=="linux","native lease proof is Linux-only")
class HistoryCheckpointTests(unittest.TestCase):
    def setUp(self):
        import test_native_provenance_fence as fences
        self.fence=fences.FenceProofTests()
        self.fence.setUp()
        self.addCleanup(self.fence.doCleanups)
        self.root=self.fence.root
        self.env=dict(self.fence.env,CODEX_SESSION_ID="root",CODEX_THREAD_ID="root")
        self.fixture=policy_fixture.ExecutionPolicyTests()
        self.fixture.setUp()
        self.rows=self.fixture.rows
        self.rows[0][1]["payload"]["cwd"]=str(self.root)
        self.history=self.root/"rollout.jsonl"
        self.history.write_text("".join(json.dumps(row)+"\n" for _,row in self.rows))
        self.events=self.fixture.events
        for _,data in self.events:
            if "cwd" in data: data["cwd"]=str(self.root)
        self.write_journal()
    def write_journal(self):
        prefix=self.history.read_bytes()
        identity=dict(schema=1,root="root",thread="root",epoch="epoch",history=str(self.history),
            history_bytes=len(prefix),history_sha256=hashlib.sha256(prefix).hexdigest())
        self.events[0]=("header",identity)
        path=Path(str(self.history)+".provenance-epoch.journal")
        lines=[]; previous="0"*64
        for seq,(kind,data) in enumerate(self.events):
            body=json.dumps(dict(seq=seq,previous=previous,kind=kind,data=data))
            previous=hashlib.sha256(body.encode()).hexdigest()
            lines.append(json.dumps(dict(body=body,sha256=previous)))
        path.write_text("\n".join(lines)+"\n")
        checkpoint=dict(identity=identity,path=str(path),seq=len(lines)-1,sha256=previous,call_id="formatter")
        cert=json.loads(self.env["CODESCOUT_NATIVE_PROVENANCE"])
        cert["checkpoint"]=checkpoint
        self.env["CODESCOUT_NATIVE_PROVENANCE"]=json.dumps(cert)
    def apply(self):
        return apply_checkpoint(self.history,read_codex_history(self.history),self.env,self.root)
    def test_actual_checkpoint_removes_only_bound_wrapper_gap_and_recovers_inner_write(self):
        import fcntl
        baseline=read_codex_history(self.history)
        self.assertEqual(len(baseline.gaps),1)
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        batch=self.apply()
        self.assertEqual(batch.gaps,[])
        self.assertEqual([(op.name,op.input,op.principal) for op in batch.operations],
            [("mcp__codescout__create_file",{"path":"a.rs"},"codex:root")])
    def test_missing_live_fence_cannot_clear_the_wrapper(self):
        batch=self.apply()
        self.assertTrue(any("wrapper" in gap.reason for gap in batch.gaps))
        self.assertTrue(any("checkpoint" in gap.reason for gap in batch.gaps))
        self.assertEqual(batch.operations,[])
    def test_unrelated_syntax_gap_is_preserved(self):
        import fcntl
        with self.history.open("a") as stream: stream.write("bad json\n")
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        batch=self.apply()
        self.assertTrue(any("malformed" in gap.reason for gap in batch.gaps))
    def test_unrelated_reason_on_the_same_certified_wrapper_locator_is_preserved(self):
        import fcntl
        from provenance_records import CoverageGap, HistoryBatch
        baseline=read_codex_history(self.history)
        unrelated=CoverageGap(f"{self.history}:2","conflicting native replay at wrapper locator",self.root)
        batch=HistoryBatch(baseline.operations,baseline.gaps+[unrelated])
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        result=apply_checkpoint(self.history,batch,self.env,self.root)
        self.assertEqual(result.gaps,[unrelated])
        self.assertEqual(len(result.operations),1)

    def test_unknown_shell_write_keeps_opaque_history(self):
        import fcntl
        self.events[7][1].update(tool="run_command",arguments={"command":"bash unknown.sh"})
        self.write_journal()
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        self.assertTrue(self.apply().gaps)
    def test_atomic_refusal_is_preserved_as_no_write(self):
        import fcntl
        self.events[8][1]["result"]={"content":[{"type":"text","text":'{"ok":false}'}],"isError":False}
        self.write_journal()
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        self.assertEqual(self.apply().operations[0].outcome,"no-write-refused")
    def test_foreign_history_does_not_borrow_current_checkpoint(self):
        foreign=self.root/"foreign.jsonl"
        foreign.write_bytes(self.history.read_bytes())
        batch=read_codex_history(foreign)
        self.assertEqual(apply_checkpoint(foreign,batch,self.env,self.root),batch)
    def test_real_scanner_consumes_live_proof_but_does_not_cache_clearance(self):
        import fcntl
        script=Path(__file__).resolve().parents[1]/"scripts/file-provenance.py"
        spec=importlib.util.spec_from_file_location("native_scanner_test",script)
        scanner=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(scanner)
        env=dict(self.env,HOME=str(self.root),
            FILE_PROVENANCE_CODEX_ROOTS_JSON=json.dumps([str(self.history)]),
            FILE_PROVENANCE_ROOTS=str(self.root/"absent-claude"),
            FILE_PROVENANCE_CACHE_DIR=str(self.root/"history-cache"))
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        result=scanner.scan_provenance(self.root,env)
        self.assertEqual(result.owners,{"a.rs":[("codex:root",None)]})
        self.assertEqual(result.gaps,[])
        fcntl.flock(self.fence.fd,fcntl.LOCK_UN)
        second=scanner.scan_provenance(self.root,env)
        self.assertEqual(second.owners,{})
        self.assertTrue(second.gaps)
    def test_real_formatter_writes_only_while_the_certified_lease_is_live(self):
        import fcntl
        import shutil
        import subprocess
        scripts=Path(__file__).resolve().parents[1]/"scripts"
        shutil.copytree(scripts,self.root/"scripts",ignore=shutil.ignore_patterns("__pycache__"))
        (self.root/"Cargo.toml").write_text('[package]\nname="native-proof-fixture"\nversion="0.1.0"\nedition="2021"\n[lib]\npath="a.rs"\n')
        path=self.root/"a.rs"
        path.write_text("pub fn sample() {}\n")
        subprocess.run(["git","init","-q",str(self.root)],check=True)
        subprocess.run(["git","-C",str(self.root),"add","Cargo.toml","a.rs"],check=True)
        subprocess.run(["git","-C",str(self.root),"-c","user.name=Fixture","-c","user.email=x@example.invalid",
                        "commit","-qm","base"],check=True)
        raw="pub fn sample(){println!(\"own\");}\n"
        path.write_text(raw)
        env=dict(os.environ)
        for key in ("CLAUDE_CODE_SESSION_ID","CLAUDE_CONFIG_DIR","CODESCOUT_CONTEXT_FILE"):
            env.pop(key,None)
        env.update(self.env,HOME=str(self.root),
        RUSTUP_HOME=os.environ.get("RUSTUP_HOME",str(Path.home()/".rustup")),
        FILE_PROVENANCE_ROOTS=str(self.root/"absent-claude"),
            FILE_PROVENANCE_CODEX_ROOTS_JSON=json.dumps([str(self.history)]),
            FILE_PROVENANCE_CACHE_DIR="")
        command=["bash","scripts/fmt-mine.sh"]
        refused=subprocess.run(command,cwd=self.root,env=env,pass_fds=(self.fence.fd,self.fence.coordination_fd),capture_output=True,text=True)
        self.assertNotEqual(refused.returncode,0)
        self.assertEqual(path.read_text(),raw)
        fcntl.flock(self.fence.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        allowed=subprocess.run(command,cwd=self.root,env=env,pass_fds=(self.fence.fd,self.fence.coordination_fd),capture_output=True,text=True)
        self.assertEqual(allowed.returncode,0,allowed.stdout+allowed.stderr)
        self.assertEqual(path.read_text(),'pub fn sample() {\n    println!("own");\n}\n')

if __name__=="__main__": unittest.main()
