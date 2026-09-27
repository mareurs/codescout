"""Stage 1c tests: the observability map (Task 13).

Mirrors tests/test_measure_join.py's shape: unittest classes, modules loaded by path through
importlib (never a real package import), fixture helpers copied from that file rather than
imported (each test file owns its own copies, per convention).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_observability.py -v
"""
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
MEASURE = REPO_ROOT / "scripts" / "measure"
sys.path.insert(0, str(MEASURE))


def _load(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", MEASURE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


transcripts = _load("transcripts")
join = _load("join")
observability = _load("observability")


# --- transcripts.py-fixture-shape helpers, mirrored verbatim from tests/test_measure_join.py ---


def _write_lines(path, dicts):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for d in dicts:
            f.write(json.dumps(d) + "\n")


def _entry(uuid, ts, sid, entrypoint="cli", type_="user", content="hello",
           is_meta=None, is_compact=None):
    e = {
        "type": type_,
        "uuid": uuid,
        "timestamp": ts,
        "sessionId": sid,
        "entrypoint": entrypoint,
        "cwd": "/x",
        "gitBranch": "main",
        "message": {"content": content},
    }
    if is_meta is not None:
        e["isMeta"] = is_meta
    if is_compact is not None:
        e["isCompactSummary"] = is_compact
    return e


def _agent_tool_use_entry(uuid, ts, sid, tool_use_id=None):
    # An assistant entry whose one content block is a tool_use named "Agent" -- the real,
    # empirically confirmed name for a subagent-launching tool call. Not MCP_TOOL_PREFIX-named,
    # so join.py inserts it into tool_events with join_method="not_codescout" directly, no
    # usage.db join required (join.py:_join_tool_events).
    return _entry(
        uuid, ts, sid, type_="assistant",
        content=[{"type": "tool_use", "id": tool_use_id or f"tu-{uuid}", "name": "Agent", "input": {}}],
    )


def _assistant_text_entry(uuid, ts, sid, text="hello"):
    return _entry(uuid, ts, sid, type_="assistant", content=[{"type": "text", "text": text}])


def _make_corpus(tmp_path, repos=None, bounds=None):
    corpus_dir = tmp_path / "corpus"
    (corpus_dir / "manifest.json").parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "corpus_id": "c-test", "created_utc": "2026-09-26T00:00:00Z",
        "bounds": bounds or {}, "files": {}, "counts": {}, "versions": {},
        "repos": repos or {}, "exclusions": [],
    }
    (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return corpus_dir


def _session_dir(corpus_dir, profile_dir, project_slug):
    return corpus_dir / "transcripts" / profile_dir / project_slug


_VALID_LABELS = {"measurable now", "needs adjudication", "unobservable"}


class ObservabilityMapShape(unittest.TestCase):
    def test_every_outcome_and_chain_link_has_a_label(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="hi"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            cells = observability.build_cells(cov)

            # 4 outcomes x 5 chain links, plus the one extra rediscovery row under transfer.
            self.assertEqual(len(cells), 4 * 5 + 1)
            for cell in cells:
                self.assertIn(cell["label"], _VALID_LABELS)
                self.assertTrue(cell["basis"])

            rendered = observability.render_map(cov)
            for outcome in ("mistakes", "context", "transfer", "background-worker"):
                self.assertIn(outcome, rendered)


class ZeroCoverageDowngrade(unittest.TestCase):
    def test_a_link_with_zero_coverage_is_never_labelled_measurable_now(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("p1", "2026-09-20T10:00:00Z", "sid1", content="do something"),
                _assistant_text_entry("a1", "2026-09-20T10:00:01Z", "sid1"),
                _agent_tool_use_entry("t1", "2026-09-20T10:00:02Z", "sid1"),
            ])
            # No subagent file and no deliveries at all in this corpus -- deliveries_total and
            # subagent_turns are genuinely 0, not merely unpopulated.

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["fields"]["deliveries_total"], 0)
            self.assertEqual(cov["fields"]["subagent_turns"], 0)
            self.assertEqual(cov["fields"]["tool_use_turns"], 1)

            cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}

            # Positive control: a base "measurable now" cell whose field IS nonzero here stays
            # "measurable now" -- proves the downgrade below is about the zero, not a blanket
            # rewrite of every cell.
            positive = cells[("context", "signal/request")]
            self.assertEqual(positive["label"], "measurable now")
            self.assertEqual(positive["number"], 1)

            # The cell under test: base "measurable now" (R58), but its coverage field is 0 in
            # this corpus, so it must be downgraded, with a basis that says why.
            downgraded = cells[("context", "delivery/action")]
            self.assertEqual(downgraded["label"], "needs adjudication")
            self.assertEqual(downgraded["number"], 0)
            self.assertIn("0", downgraded["basis"])

            # General invariant, over every cell: a zero-coverage number is never rendered as
            # "measurable now".
            for cell in cells.values():
                if cell["number"] == 0:
                    self.assertNotEqual(cell["label"], "measurable now")


class Rediscovery(unittest.TestCase):
    def test_rediscovery_is_needs_adjudication(self):
        self.assertEqual(observability._REDISCOVERY_SPEC["label"], "needs adjudication")

        cov = {"fields": {}}
        cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}
        self.assertEqual(cells[("transfer", "rediscovery")]["label"], "needs adjudication")


class BaseTableMatchesRuling(unittest.TestCase):
    def test_r58_base_table_labels_match_the_ruling(self):
        # Transcribed verbatim from task-13-context.md's R58 table (controller ruling).
        expected = {
            ("mistakes", "opportunity"): "measurable now",
            ("mistakes", "signal/request"): "needs adjudication",
            ("mistakes", "delivery/action"): "needs adjudication",
            ("mistakes", "observed use"): "needs adjudication",
            ("mistakes", "checked outcome"): "needs adjudication",
            ("context", "opportunity"): "measurable now",
            ("context", "signal/request"): "measurable now",
            ("context", "delivery/action"): "measurable now",
            ("context", "observed use"): "needs adjudication",
            ("context", "checked outcome"): "needs adjudication",
            ("transfer", "opportunity"): "needs adjudication",
            ("transfer", "signal/request"): "needs adjudication",
            ("transfer", "delivery/action"): "measurable now",
            ("transfer", "observed use"): "needs adjudication",
            ("transfer", "checked outcome"): "needs adjudication",
            ("background-worker", "opportunity"): "measurable now",
            ("background-worker", "signal/request"): "measurable now",
            ("background-worker", "delivery/action"): "measurable now",
            ("background-worker", "observed use"): "needs adjudication",
            ("background-worker", "checked outcome"): "unobservable",
        }
        self.assertEqual(len(observability._BASE_TABLE), len(expected))
        for key, label in expected.items():
            self.assertEqual(observability._BASE_TABLE[key]["label"], label, msg=str(key))
        self.assertEqual(observability._REDISCOVERY_SPEC["label"], "needs adjudication")


class ExclusionsMismatchRaises(unittest.TestCase):
    def test_mismatch_between_recomputed_and_persisted_exclusions_raises(self):
        # join.SPEC_EXCLUDED_SIDS' sole member is this very session's own sid -- R44's spec-level
        # force-exclude. Forcing it KEPT at build time (excluded_sids=set()) disagrees with
        # coverage()'s hardcoded recompute, which always uses the real join.SPEC_EXCLUDED_SIDS.
        spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / f"{spec_sid}.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", spec_sid, content="hi"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids=set())

            with self.assertRaises(ValueError):
                observability.coverage(events_db, corpus_dir)


class ExclusionsByReason(unittest.TestCase):
    def test_exclusions_by_reason_grouping(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "headless-sid.jsonl", [
                _entry("h1", "2026-09-20T10:00:00Z", "headless-sid", entrypoint="sdk-cli"),
            ])
            _write_lines(proj / "normal-sid.jsonl", [
                _entry("n1", "2026-09-20T10:00:00Z", "normal-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["exclusions_by_reason"].get("sdk-cli"), 1)


class DivergentDuplicateUnowned(unittest.TestCase):
    def test_divergent_duplicate_unowned_uuid_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            # kat: 5-entry timeline -> the longer copy, so it is the keeper (same fixture
            # template as tests/test_measure_join.py's SubagentSiblingEdgeCases
            # .test_a_divergent_duplicate_sibling_is_still_unioned).
            kat_lines = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", "div-sid") for i in range(5)]
            # sdd shares kat's first 3 entries but its 4th DIVERGES (different uuid/ts) rather
            # than simply stopping early -- exclusions() labels it "divergent-duplicate-of:",
            # not "duplicate-prefix-of:".
            sdd_lines = kat_lines[:3] + [_entry("vX", "2026-09-20T10:00:09Z", "div-sid")]
            _write_lines(kat_proj / "div-sid.jsonl", kat_lines)
            _write_lines(sdd_proj / "div-sid.jsonl", sdd_lines)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            unowned = set()
            for uuids in cov["divergent_duplicate_unowned"].values():
                unowned.update(uuids)
            # u0/u1/u2 are owned by kept kat; "vX" is unique to the excluded sdd copy and no
            # kept transcript owns it -- Amendment 3's documented cost of the collapse.
            self.assertEqual(unowned, {"vX"})


class ToolEventsByDay(unittest.TestCase):
    def test_tool_events_by_day_split(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _agent_tool_use_entry("t1", "2026-09-20T10:00:00Z", "sid1"),
                _agent_tool_use_entry("t2", "2026-09-21T10:00:00Z", "sid1"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(set(cov["tool_events_by_day"].keys()), {"2026-09-20", "2026-09-21"})
            for day in ("2026-09-20", "2026-09-21"):
                self.assertEqual(cov["tool_events_by_day"][day], {"not_codescout": 1})
            self.assertEqual(cov["tool_events_by_method"]["not_codescout"], 2)


if __name__ == "__main__":
    unittest.main()
