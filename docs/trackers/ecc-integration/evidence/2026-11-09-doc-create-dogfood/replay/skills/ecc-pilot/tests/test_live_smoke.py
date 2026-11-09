"""Guard native smoke evidence against message mentions and failed tool calls."""
import unittest

from live_smoke import require_completed_tools


class LiveEvidenceTests(unittest.TestCase):
    def test_agent_text_is_not_tool_execution(self):
        events = [{"type": "item.completed", "item": {
            "type": "agent_message", "text": "I used references and call_graph."}}]
        with self.assertRaises(AssertionError):
            require_completed_tools(events, {"references", "call_graph"})

    def test_failed_mcp_call_is_not_execution_evidence(self):
        for result in ({"isError": True}, {"content": [
                {"type": "text", "text": '{"ok": false, "error": "unavailable"}'}]}):
            event = {"type": "item.completed", "item": {
                "type": "mcp_tool_call", "server": "codescout",
                "tool": "references", "status": "completed", "result": result}}
            with self.subTest(result=result), self.assertRaises(AssertionError):
                require_completed_tools([event], {"references"})

    def test_only_completed_calls_from_codescout_qualify(self):
        def event(server, status, tool):
            return {"type": "item.completed", "item": {
                "type": "mcp_tool_call", "server": server, "tool": tool,
                "status": status, "result": {"content": [
                    {"type": "text", "text": "caller result"}]}}}
        events = [event("other", "completed", "references"),
                  event("codescout", "failed", "call_graph"),
                  event("codescout", "completed", "references")]
        with self.assertRaises(AssertionError):
            require_completed_tools(events, {"references", "call_graph"})
        events.append(event("codescout", "completed", "call_graph"))
        calls = require_completed_tools(events, {"references", "call_graph"})
        self.assertEqual([c["tool"] for c in calls], ["references", "call_graph"])


if __name__ == "__main__":
    unittest.main()
