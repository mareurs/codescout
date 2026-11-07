"""Contract tests: prefix integrity is independent of ownership clearance."""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
from native_provenance import ProofError, verify_checkpoint

def encoded(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)

class NativeProofTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.history = Path(self.tmp.name) / "rollout.jsonl"
        self.history.write_text(encoded({"type":"session_meta","payload":{"session_id":"root","id":"thread","cwd":self.tmp.name}}) + "\n")
        binary = os.environ.get("NATIVE_PROVENANCE_FIXTURE_BINARY")
        if binary:
            self.checkpoint = json.loads(subprocess.check_output([binary, str(self.history)], text=True))
            self.path = Path(self.checkpoint["path"])
            self.lines = self.path.read_text().splitlines()
        else:
            prefix = self.history.read_bytes()
            identity = dict(schema=1, root="root", thread="thread", epoch="epoch",
                            history=str(self.history), history_bytes=len(prefix),
                            history_sha256=hashlib.sha256(prefix).hexdigest())
            self.path = Path(str(self.history) + ".provenance-epoch.journal")
            events = [
                ("header", identity),
                ("received", {"call_id":"writer","tool":"create_file","arguments":{"path":"src/main.rs"}}),
                ("dispatch", {"call_id":"writer"}),
                ("outcome", {"call_id":"writer","outcome":"completed"}),
                ("received", {"call_id":"formatter","tool":"run_command","arguments":{}}),
                ("dispatch", {"call_id":"formatter"}),
                ("checkpoint", {"call_id":"formatter"}),
            ]
            self.lines, previous = [], "0" * 64
            for seq, (kind, data) in enumerate(events):
                body = encoded(dict(seq=seq, previous=previous, kind=kind, data=data))
                previous = hashlib.sha256(body.encode()).hexdigest()
                self.lines.append(encoded(dict(body=body, sha256=previous)))
            self.path.write_text("\n".join(self.lines) + "\n")
            self.checkpoint = dict(identity=identity, path=str(self.path), seq=6,
                                   sha256=previous, call_id="formatter")

    def verify(self, checkpoint=None, **overrides):
        args = dict(checkpoint=checkpoint or self.checkpoint,
                    native_identity={"root":"root","thread":"thread"},
                    history_path=self.history, invocation_id="formatter")
        args.update(overrides)
        return verify_checkpoint(**args)

    def rewrite(self, lines):
        self.path.write_text("\n".join(lines) + "\n")

    def test_genuine_ordered_prefix_binds_both_calls_and_formatter(self):
        proof = self.verify()
        self.assertEqual(proof.call_id, "formatter")
        self.assertEqual([event["kind"] for event in proof.events],
                         ["header","received","dispatch","outcome","received","dispatch","checkpoint"])
        self.assertEqual(proof.events[1]["data"]["call_id"], "writer")

    def test_later_unfinished_tail_does_not_certify_earlier_prefix(self):
        with self.path.open("a") as stream:
            stream.write('{"body":"unfinished')
        self.assertEqual(len(self.verify().events), 7)

    def test_truncated_required_line_refuses(self):
        self.path.write_bytes(self.path.read_bytes()[:-2])
        with self.assertRaises(ProofError): self.verify()

    def test_corrupt_body_refuses(self):
        self.rewrite([s.replace("writer", "peer") for s in self.lines])
        with self.assertRaises(ProofError): self.verify()

    def test_missing_event_refuses(self):
        self.rewrite(self.lines[:2] + self.lines[3:])
        with self.assertRaises(ProofError): self.verify()

    def test_wrong_identity_epoch_history_or_invocation_refuses(self):
        for field, value in [("root","peer"), ("thread","child"), ("epoch","other"),
                             ("schema",2), ("history_bytes",True),
                             ("history",str(Path(self.tmp.name) / "other.jsonl"))]:
            with self.subTest(field=field):
                checkpoint = copy.deepcopy(self.checkpoint)
                checkpoint["identity"][field] = value
                with self.assertRaises(ProofError): self.verify(checkpoint)
        with self.assertRaises(ProofError): self.verify(invocation_id="sibling")
        with self.assertRaises(ProofError): self.verify(native_identity={"root":"peer","thread":"thread"})

    def test_wrong_checkpoint_frontier_refuses(self):
        for field, value in [("seq",True), ("seq",-1), ("seq",9999),
                             ("sha256","0" * 64), ("call_id","writer")]:
            with self.subTest(field=field):
                checkpoint = copy.deepcopy(self.checkpoint)
                checkpoint[field] = value
                with self.assertRaises(ProofError): self.verify(checkpoint)

    def test_changed_history_prefix_refuses_but_append_is_allowed(self):
        original = self.history.read_bytes()
        with self.history.open("ab") as stream: stream.write(b'{"type":"event_msg"}\n')
        self.assertEqual(len(self.verify().events), 7)
        self.history.write_bytes(original.replace(b"root", b"peer"))
        with self.assertRaises(ProofError): self.verify()

    def test_an_environment_path_alone_cannot_supply_a_checkpoint(self):
        with self.assertRaises(ProofError): self.verify(checkpoint={"path":str(self.path)})

    def test_substituted_or_linked_journal_refuses(self):
        other = Path(self.tmp.name) / "substitute.jsonl"
        other.write_bytes(self.path.read_bytes())
        checkpoint = copy.deepcopy(self.checkpoint)
        checkpoint["path"] = str(other)
        with self.assertRaises(ProofError): self.verify(checkpoint)
        self.path.unlink()
        self.path.symlink_to(other)
        with self.assertRaises(ProofError): self.verify()

    def test_duplicate_json_keys_refuse_even_when_values_match(self):
        last = self.lines[-1]
        self.rewrite(self.lines[:-1] + [last[:-1] + ',"sha256":"' + self.checkpoint["sha256"] + '"}'])
        with self.assertRaises(ProofError): self.verify()

    def test_checkpoint_at_noncheckpoint_record_refuses(self):
        checkpoint = copy.deepcopy(self.checkpoint)
        checkpoint["seq"] = 5
        checkpoint["sha256"] = json.loads(self.lines[5])["sha256"]
        with self.assertRaises(ProofError): self.verify(checkpoint)

if __name__ == "__main__":
    unittest.main()
