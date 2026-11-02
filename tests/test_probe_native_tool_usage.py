"""Behavioral acceptance for scripts/probe-native-tool-usage.py, on small synthetic transcript trees.

Each case is a trap met while measuring native-tool use over 6,038 real transcripts (2026-11-02): a resumed session
copies history (44,430 records, 34,660 distinct ids), some copies carry no timestamp or cwd, a symlinked transcript
is a second path to the same file, and the guard's config is read at the EXACT cwd."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

PROBE = Path(os.environ.get("NATIVE_USAGE_PROBE", Path(__file__).resolve().parents[1] / "scripts/probe-native-tool-usage.py"))
MAIN = "/work/codescout"
BLOCKED = "This call is blocked because codescout has a faster path for source files.\n\nFile: x"
DISABLED = "<tool_use_error>Error: No such tool available: Write. Write is disabled</tool_use_error>"
POINTER = "PreToolUse:Read hook error: BLOCKED (see previous message)"


def tool_use(tid, name, inp, ts=None, cwd=None):
    d = {"type": "assistant", "message": {"content": [{"type": "tool_use", "id": tid, "name": name, "input": inp}]}}
    if ts:
        d["timestamp"] = ts
    if cwd:
        d["cwd"] = cwd
    return json.dumps(d, separators=(",", ":"))


def tool_result(tid, text="ok", err=False):
    d = {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": tid, "is_error": err,
                                                  "content": [{"type": "text", "text": text}]}]}}
    return json.dumps(d, separators=(",", ":"))


class NativeUsageProbe(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.profile = Path(self.tmp.name) / "prof"
        self.proj = self.profile / "projects" / "-work-codescout"
        self.proj.mkdir(parents=True)

    def write(self, rel, lines):
        p = self.proj / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("\n".join(lines) + "\n")
        return p

    def run_probe(self, *extra):
        r = subprocess.run([sys.executable, str(PROBE), "--profiles", str(self.profile), "--main-checkout", MAIN,
                            "--jobs", "1", "--json", *extra], capture_output=True, text=True)
        return r

    def report(self, *extra):
        r = self.run_probe(*extra)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def read(self, tid, path, ts="2026-10-01T10:00:00.000Z", cwd=MAIN, text="ok", err=False):
        return [tool_use(tid, "Read", {"file_path": path}, ts, cwd), tool_result(tid, text, err)]

    # --- the resumed-session copy -------------------------------------------------------------------------------

    def test_a_resumed_copy_counts_once_and_keeps_the_ts_and_cwd_of_the_copy_that_has_them(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        # the copy has the same id but NO timestamp and NO cwd: picking it would drop the call from every dated cut
        self.write("sess2.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs", ts=None, cwd=None))
        rep = self.report("--since", "2026-10-01")
        self.assertEqual(rep["records_before_dedup"], 2)
        self.assertEqual(rep["distinct_calls_total"], 1)
        self.assertEqual(rep["population"]["distinct_native_calls"], 1)
        self.assertEqual(rep["read_grep_glob_by_cwd_class"], {"main_checkout": {"ok": 1}})

    def test_the_first_copy_scanned_may_be_the_one_without_ts_and_cwd(self):
        # "sess0" sorts, and is scanned, BEFORE "sess1": the base copy has neither a ts nor a cwd, and both must come from
        # the later copy. (With the stamped copy first, as in the test above, a probe that never merges would still pass.)
        self.write("sess0.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs", ts=None, cwd=None))
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        rep = self.report("--since", "2026-10-01")
        self.assertEqual(rep["population"]["distinct_native_calls"], 1)
        self.assertEqual(rep["read_grep_glob_by_cwd_class"], {"main_checkout": {"ok": 1}})


    def test_the_earliest_timestamp_among_the_copies_is_the_calls_time(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs", ts="2026-10-05T10:00:00.000Z"))
        self.write("sess2.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs", ts="2026-10-20T10:00:00.000Z"))
        self.assertEqual(self.report("--since", "2026-10-01")["population"]["distinct_native_calls"], 1)
        self.assertEqual(self.report("--since", "2026-10-10")["population"]["distinct_native_calls"], 0)

    def test_a_main_session_copy_outranks_a_subagent_copy(self):
        self.write("sess1/subagents/agent-a.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        self.write("sess9.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        ok = self.report()["ok_source_in_main_checkout"]
        self.assertEqual(ok["main_vs_subagent"], {"main": 1})

    def test_a_call_without_a_timestamp_in_any_copy_is_in_no_window_and_is_reported(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs", ts=None))
        rep = self.report("--since", "2026-10-01")
        self.assertEqual(rep["population"]["distinct_native_calls"], 0)
        self.assertEqual(self.report()["population"]["without_ts_or_cwd"], 1)

    def test_a_symlinked_transcript_is_not_scanned_twice(self):
        real = self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        (self.proj / "sess2.jsonl").symlink_to(real)
        rep = self.report()
        self.assertEqual(rep["files_scanned"], 1)
        self.assertEqual(rep["records_before_dedup"], 1)

    # --- outcome classes ---------------------------------------------------------------------------------------

    def test_outcome_classes_come_from_the_observed_strings(self):
        lines = (self.read("toolu_ok", MAIN + "/a.rs")
                 + self.read("toolu_bl", MAIN + "/b.rs", text=BLOCKED, err=True)
                 + self.read("toolu_pt", MAIN + "/c.rs", text=POINTER, err=True)
                 + self.read("toolu_er", MAIN + "/d.rs", text="File does not exist.", err=True)
                 + [tool_use("toolu_w", "Write", {"file_path": MAIN + "/e.rs"}, "2026-10-01T10:00:00.000Z", MAIN),
                    tool_result("toolu_w", DISABLED, True)])
        self.write("sess1.jsonl", lines)
        out = self.report()["outcomes_by_tool"]
        self.assertEqual(out["Read"], {"ok": 1, "blocked": 2, "other_error": 1})
        self.assertEqual(out["Write"], {"disabled": 1})
        self.assertEqual(self.report()["blocked"]["pointer_only"], 1)

    def test_an_unknown_error_wording_is_other_error_never_blocked(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/a.rs", text="Some new refusal wording", err=True))
        self.assertEqual(self.report()["outcomes_by_tool"]["Read"], {"other_error": 1})

    def test_a_call_with_no_result_is_no_result_not_ok(self):
        self.write("sess1.jsonl", [tool_use("toolu_1", "Read", {"file_path": MAIN + "/a.rs"}, "2026-10-01T10:00:00.000Z", MAIN)])
        self.assertEqual(self.report()["outcomes_by_tool"]["Read"], {"no_result": 1})

    # --- where, and what counts as source ----------------------------------------------------------------------

    def test_the_cwd_class_is_the_exact_cwd_not_the_project_root(self):
        lines = []
        for i, cwd in enumerate([MAIN, MAIN + "/src", MAIN + "/.worktrees/x", MAIN + ".worktrees/y", "/other/proj"]):
            lines += self.read(f"toolu_{i}", MAIN + "/a.rs", cwd=cwd)
        self.write("sess1.jsonl", lines)
        got = self.report()["read_grep_glob_by_cwd_class"]
        self.assertEqual(sorted(got), ["main_checkout", "other_dir", "repo_subdir", "sibling_worktree", "worktree_in_repo"])
        self.assertTrue(all(v == {"ok": 1} for v in got.values()), got)

    def test_source_is_the_guards_extension_list_and_a_directory_grep_is_unscoped(self):
        lines = (self.read("toolu_rs", MAIN + "/a.rs")                      # source
                 + self.read("toolu_md", MAIN + "/docs/x.md")               # not source
                 + [tool_use("toolu_g1", "Grep", {"path": MAIN, "pattern": "x", "type": "rust"}, "2026-10-01T10:00:00.000Z", MAIN),
                    tool_result("toolu_g1"),
                    tool_use("toolu_g2", "Grep", {"path": MAIN + "/src", "pattern": "x"}, "2026-10-01T10:00:00.000Z", MAIN),
                    tool_result("toolu_g2"),
                    tool_use("toolu_g3", "Grep", {"path": MAIN + "/docs/a.md", "pattern": "x"}, "2026-10-01T10:00:00.000Z", MAIN),
                    tool_result("toolu_g3")])
        self.write("sess1.jsonl", lines)
        ok = self.report()["ok_source_in_main_checkout"]
        self.assertEqual(ok["total"], 3)             # the .rs Read, the typed Grep, the directory-only Grep
        self.assertEqual(ok["by_tool"], {"Read": 1, "Grep": 2})

    # --- who ---------------------------------------------------------------------------------------------------

    def test_a_subagent_is_attributed_by_its_file_and_its_meta(self):
        self.write("sess1/subagents/agent-a.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        (self.proj / "sess1" / "subagents" / "agent-a.meta.json").write_text(
            json.dumps({"agentType": "general-purpose", "description": "Implement Task 13: observability map"}))
        ok = self.report()["ok_source_in_main_checkout"]
        self.assertEqual(ok["main_vs_subagent"], {"sub": 1})
        self.assertEqual(ok["subagent_type"], {"general-purpose": 1})
        self.assertEqual(ok["subagent_description_kind"], {"implement": 1})

    def test_codescout_calls_are_counted_by_distinct_id_across_copies(self):
        cs = [tool_use("toolu_c1", "mcp__codescout__symbols", {}, "2026-10-01T10:00:00.000Z", MAIN),
              tool_use("toolu_c2", "mcp__codescout__run_command", {}, "2026-10-01T10:00:00.000Z", MAIN)]
        self.write("sess1.jsonl", cs)
        self.write("sess2.jsonl", cs)   # the resumed copy
        pop = self.report()["population"]
        self.assertEqual(pop["distinct_codescout_calls"], 2)
        self.assertEqual(pop["codescout_read_ish"], 1)          # symbols is a read-ish tool, run_command is not

    def test_a_transcript_with_codescout_calls_is_not_counted_as_without(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/a.rs")
                   + [tool_use("toolu_c1", "mcp__codescout__symbols", {}, "2026-10-01T10:00:00.000Z", MAIN)])
        self.write("sess2.jsonl", self.read("toolu_2", MAIN + "/b.rs"))
        conc = self.report()["concentration"]
        self.assertEqual(conc["in_transcripts_with_no_codescout_call"], 1)

    # --- blocked, then what ------------------------------------------------------------------------------------

    def test_a_blocked_read_is_classed_by_what_happened_to_the_same_path_afterwards(self):
        self.write("sess1.jsonl",
                   self.read("toolu_a", MAIN + "/a.rs", ts="2026-10-01T10:00:00.000Z", text=BLOCKED, err=True)
                   + self.read("toolu_a2", MAIN + "/a.rs", ts="2026-10-01T10:05:00.000Z")                      # later ok
                   + self.read("toolu_b", MAIN + "/b.rs", ts="2026-10-01T10:00:00.000Z", text=BLOCKED, err=True)
                   + self.read("toolu_b2", MAIN + "/b.rs", ts="2026-10-01T10:05:00.000Z", text=BLOCKED, err=True)  # again
                   + self.read("toolu_c", MAIN + "/c.rs", ts="2026-10-01T10:00:00.000Z", text=BLOCKED, err=True))  # never
        b = self.report()["blocked"]
        self.assertEqual((b["reads_later_succeeded_same_path"], b["reads_retried_blocked_again"], b["reads_not_retried_natively"]), (1, 1, 2))
        self.assertEqual(b["total"], 4)

    # --- the command line --------------------------------------------------------------------------------------

    def test_no_transcripts_is_exit_2_never_a_clean_empty_report(self):
        empty = Path(self.tmp.name) / "empty"
        empty.mkdir()
        r = subprocess.run([sys.executable, str(PROBE), "--profiles", str(empty), "--jobs", "1"], capture_output=True, text=True)
        self.assertEqual(r.returncode, 2)
        self.assertIn("no transcripts found", r.stderr)

    def test_the_text_report_runs(self):
        self.write("sess1.jsonl", self.read("toolu_1", MAIN + "/src/lib.rs"))
        r = subprocess.run([sys.executable, str(PROBE), "--profiles", str(self.profile), "--main-checkout", MAIN, "--jobs", "1"],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("distinct native calls 1", r.stdout)


if __name__ == "__main__":
    unittest.main()
