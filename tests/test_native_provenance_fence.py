import fcntl
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"scripts"))
from native_provenance import ProofError
from native_provenance_fence import verify_fence

@unittest.skipUnless(sys.platform=="linux","live descriptor proof requires Linux")
class FenceProofTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name)
        state=self.root/".codescout"
        state.mkdir()
        self.path=state/"native-provenance-v1.lock"
        self.fd=os.open(self.path,os.O_CREAT|os.O_RDWR,0o600)
        self.addCleanup(os.close,self.fd)
        (state/"native-provenance-v1.counter").write_bytes(struct.pack("<Q",4))
        domain=self.root/"coordination"
        domain.mkdir(mode=0o700,exist_ok=True)
        state=domain/".codescout"
        state.mkdir(mode=0o700,exist_ok=True)
        self.coordination_fd=os.open(state/"native-provenance-v1.lock",os.O_CREAT|os.O_RDWR,0o600)
        self.addCleanup(os.close,self.coordination_fd)
        fcntl.flock(self.coordination_fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        counter=state/"native-provenance-v1.counter"
        before=struct.unpack("<Q",counter.read_bytes())[0] if counter.exists() else 0
        counter.write_bytes(struct.pack("<Q",before+1))
        self.env={"CODESCOUT_PROVENANCE_COORDINATOR_FD":str(self.coordination_fd),
                  "CODESCOUT_PROVENANCE_COORDINATOR_ROOT":str(domain.resolve()),
                  "CODESCOUT_PROVENANCE_COORDINATOR_GENERATION":str(before+1),
                  "CODESCOUT_PROVENANCE_FENCE_FD":str(self.fd),
                  "CODESCOUT_PROVENANCE_FENCE_ROOT":str(self.root),
                  "CODESCOUT_PROVENANCE_FENCE_GENERATION":"4",
                  "CODESCOUT_NATIVE_PROVENANCE":json.dumps({"checkpoint":{"call_id":"formatter"},
                     "invocation_id":"formatter","fence_root":str(self.root),"fence_generation":3,
                     "coordination_generation":before,"coordination_root":str(domain.resolve())})}
    def test_live_exclusive_descriptor_and_generation_are_required(self):
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        self.assertEqual(verify_fence(self.env,self.root)["invocation_id"],"formatter")
    def test_unlocked_shared_wrong_descriptor_and_stale_generation_refuse(self):
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
        fcntl.flock(self.fd,fcntl.LOCK_SH|fcntl.LOCK_NB)
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        for key,value in (("CODESCOUT_PROVENANCE_FENCE_FD","999999"),
                          ("CODESCOUT_PROVENANCE_FENCE_GENERATION","3"),
                          ("CODESCOUT_PROVENANCE_FENCE_ROOT",str(self.root/"elsewhere"))):
            saved=self.env[key]; self.env[key]=value
            with self.subTest(key=key),self.assertRaises(ProofError): verify_fence(self.env,self.root)
            self.env[key]=saved
    def test_replaced_or_linked_lock_and_corrupt_counter_refuse(self):
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        self.path.unlink(); self.path.touch()
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
    def test_environment_certificate_alone_is_insufficient(self):
        env={"CODESCOUT_NATIVE_PROVENANCE":self.env["CODESCOUT_NATIVE_PROVENANCE"]}
        with self.assertRaises(ProofError): verify_fence(env,self.root)
    def test_checkout_lease_alone_cannot_replace_live_global_coordination(self):
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        fcntl.flock(self.coordination_fd,fcntl.LOCK_UN)
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
        fcntl.flock(self.coordination_fd,fcntl.LOCK_SH|fcntl.LOCK_NB)
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
        fcntl.flock(self.coordination_fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        counter=Path(self.env["CODESCOUT_PROVENANCE_COORDINATOR_ROOT"])/".codescout/native-provenance-v1.counter"
        current=struct.unpack("<Q",counter.read_bytes())[0]
        counter.write_bytes(struct.pack("<Q",current+1))
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
    def test_durable_unverified_shell_evidence_refuses_even_with_both_live_leases(self):
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        marker=Path(self.env["CODESCOUT_PROVENANCE_COORDINATOR_ROOT"])/".codescout/unverified-shell-v1"
        marker.write_text("unverified earlier shell")
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)

    def test_counter_and_invocation_binding_cannot_be_replayed(self):
        fcntl.flock(self.fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
        (self.root/".codescout/native-provenance-v1.counter").write_bytes(struct.pack("<Q",5))
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)
        (self.root/".codescout/native-provenance-v1.counter").write_bytes(struct.pack("<Q",4))
        cert=json.loads(self.env["CODESCOUT_NATIVE_PROVENANCE"])
        cert["checkpoint"]["call_id"]="earlier"
        self.env["CODESCOUT_NATIVE_PROVENANCE"]=json.dumps(cert)
        with self.assertRaises(ProofError): verify_fence(self.env,self.root)

if __name__=="__main__": unittest.main()
