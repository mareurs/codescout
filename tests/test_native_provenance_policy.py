import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from native_provenance import VerifiedPrefix, ProofError
from native_provenance_policy import certify_execution

class ExecutionPolicyTests(unittest.TestCase):
    def setUp(self):
        self.identity = {"root":"root","thread":"root"}
        self.rows = [(1, {"type":"session_meta","payload":{"session_id":"root","id":"root","cwd":"/repo"}}),
                     (2, {"type":"response_item","payload":{"type":"function_call","name":"exec","call_id":"wrapper","arguments":"{}"}})]
        self.events = [
            ("header",self.identity),
            ("received",{"call_id":"wrapper","scope":"wrapper","tool":"exec","arguments":{},"arguments_complete":True,"cwd":"/repo","source":{"kind":"direct"}}),
            ("dispatch",{"call_id":"wrapper"}),
            ("cell_start",{"cell_id":"cell","call_id":"wrapper"}),
            ("cell_callback",{"cell_id":"cell","runtime_id":"one","sequence":1}),
            ("received",{"call_id":"writer","scope":"operation","tool":"mcp__alias__create_file","arguments":{"path":"a.rs"},"arguments_complete":True,"cwd":"/repo","source":{"kind":"code_mode","cell_id":"cell","runtime_id":"one"}}),
            ("dispatch",{"call_id":"writer"}),
            ("mcp_dispatch",{"call_id":"writer","server":"alias","tool":"create_file","arguments":{"path":"a.rs"},"fence_capability":True}),
            ("mcp_outcome",{"call_id":"writer","result":{"content":[],"isError":False}}),
            ("outcome",{"call_id":"writer","outcome":"completed"}),
            ("cell_callback",{"cell_id":"cell","runtime_id":"two","sequence":2}),
            ("received",{"call_id":"formatter","scope":"operation","tool":"mcp__alias__run_command","arguments":{"command":"bash scripts/fmt-mine.sh"},"arguments_complete":True,"cwd":"/repo","source":{"kind":"code_mode","cell_id":"cell","runtime_id":"two"}}),
            ("dispatch",{"call_id":"formatter"}),
            ("mcp_dispatch",{"call_id":"formatter","server":"alias","tool":"run_command","arguments":{"command":"bash scripts/fmt-mine.sh"},"fence_capability":True}),
            ("checkpoint",{"call_id":"formatter"}),
        ]
    def certify(self):
        prefix = VerifiedPrefix(self.identity, "formatter",
            tuple({"seq":i,"kind":k,"data":v} for i,(k,v) in enumerate(self.events)), "0"*64)
        return certify_execution(prefix,self.rows)
    def test_exact_active_formatter_and_completed_writer_are_certified(self):
        proof = self.certify()
        self.assertEqual(proof.wrapper_call_ids, frozenset({"wrapper"}))
        self.assertEqual([call[0]["call_id"] for call in proof.operations], ["writer"])
        self.assertEqual(proof.operations[0][1]["arguments"], {"path":"a.rs"})
        self.assertEqual(proof.operations[0][2], {"content":[],"isError":False})
    def test_missing_result_pending_cancel_and_unsupported_connection_refuse(self):
        for kind in ("mcp_outcome","outcome"):
            with self.subTest(kind=kind):
                saved = copy.deepcopy(self.events)
                self.events = [(k,v) for k,v in self.events if not (k==kind and v.get("call_id")=="writer")]
                with self.assertRaises(ProofError): self.certify()
                self.events=saved
        self.events[7][1]["fence_capability"]=False
        with self.assertRaises(ProofError): self.certify()
    def test_missing_issued_slot_unmatched_callback_and_reused_runtime_refuse(self):
        for change in ("hole","extra","reused"):
            saved=copy.deepcopy(self.events)
            if change=="hole": self.events[10][1]["sequence"]=3
            elif change=="extra": self.events.insert(-1,("cell_callback",{"cell_id":"cell","runtime_id":"unreceived","sequence":3}))
            else: self.events[10][1]["runtime_id"]="one"
            with self.subTest(change=change), self.assertRaises(ProofError): self.certify()
            self.events=saved
    def test_only_exact_formatter_is_exempt_from_pending(self):
        self.events[13][1]["arguments"]["command"]="bash scripts/fmt-mine.sh; touch peer.rs"
        with self.assertRaises(ProofError): self.certify()
    def test_unjournaled_history_wrapper_and_external_lineage_refuse(self):
        self.rows.append((3,{"type":"response_item","payload":{"type":"function_call","name":"exec","call_id":"old","arguments":"{}"}}))
        with self.assertRaises(ProofError): self.certify()
        self.rows.pop()
        self.rows[0][1]["payload"]["forked_from_id"]="parent"
        with self.assertRaises(ProofError): self.certify()
    def test_initial_world_state_does_not_mean_history_was_compacted(self):
        self.rows.insert(1,(3,{"type":"world_state","payload":{"full":True,"state":{}}}))
        self.assertEqual(self.certify().wrapper_call_ids,frozenset({"wrapper"}))

    def test_actual_compaction_envelopes_and_completed_items_refuse(self):
        for row in ({"type":"compacted","payload":{}},
                    {"type":"event_msg","payload":{"type":"item_completed","item":{"type":"ContextCompaction"}}}):
            self.rows.append((3,row))
            with self.subTest(row=row),self.assertRaises(ProofError): self.certify()
            self.rows.pop()

    def test_incomplete_late_retirement_unknown_events_and_other_active_cells_refuse(self):
        for event in (("incomplete",{}),("future_event",{}),
                      ("cell_closed",{"cell_id":"cell","final_sequence":3}),
                      ("cell_start",{"cell_id":"sibling","call_id":"wrapper"})):
            saved=copy.deepcopy(self.events)
            self.events.insert(-1,event)
            with self.subTest(event=event), self.assertRaises(ProofError): self.certify()
            self.events=saved

if __name__ == "__main__": unittest.main()
