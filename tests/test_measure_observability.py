"""Stage 1c tests: the observability map (Task 13, fix round 1).

Mirrors tests/test_measure_join.py's shape: unittest classes, modules loaded by path through
importlib (never a real package import), fixture helpers copied from that file rather than
imported (each test file owns its own copies, per convention).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_observability.py -v
"""
import importlib.util
import json
import pathlib
import sqlite3
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


def _multi_tool_use_entry(uuid, ts, sid, names):
    # R65: one entry, N tool_use blocks -- join.py's _tool_use_candidates_from_entry emits one
    # candidate row per block, so this is what "the context signal cell counts tool_use BLOCKS,
    # not entries" actually needs to exercise.
    blocks = [
        {"type": "tool_use", "id": f"tu-{uuid}-{i}", "name": name, "input": {}}
        for i, name in enumerate(names)
    ]
    return _entry(uuid, ts, sid, type_="assistant", content=blocks)


def _interrupt_entry(uuid, ts, sid):
    return _entry(uuid, ts, sid, type_="user", content="[Request interrupted by user]")


# R61: a bounds-less manifest used to resolve both windows to (None, None), which zeroed every
# window-gated field regardless of real data -- confirmed a real bug in the round-0 fixture
# helper. Default both windows wide open so a fixture that doesn't care about window bounds
# still gets real, non-spuriously-downgraded counts.
_DEFAULT_BOUNDS = {
    "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
    "decision": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
}


def _make_corpus(tmp_path, repos=None, bounds=None):
    corpus_dir = tmp_path / "corpus"
    (corpus_dir / "manifest.json").parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "corpus_id": "c-test", "created_utc": "2026-09-26T00:00:00Z",
        "bounds": bounds if bounds is not None else _DEFAULT_BOUNDS,
        "files": {}, "counts": {}, "versions": {},
        "repos": repos or {}, "exclusions": [],
    }
    (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))
    return corpus_dir


def _load_manifest(corpus_dir):
    return json.loads((pathlib.Path(corpus_dir) / "manifest.json").read_text())


def _session_dir(corpus_dir, profile_dir, project_slug):
    return corpus_dir / "transcripts" / profile_dir / project_slug


def _subagent_dir(corpus_dir, profile_dir, project_slug, sid):
    return _session_dir(corpus_dir, profile_dir, project_slug) / sid / "subagents"


def _write_usage_db(corpus_dir, rows, db_name="usage.db"):
    # Exact tool_calls column set per join.py's _load_usage_rows SELECT: id, tool_name,
    # called_at, started_at, cc_session_id, agent_id, input_json, output_json, deliveries_json,
    # tool_use_id, latency_ms.
    db_dir = pathlib.Path(corpus_dir) / "usage_dbs"
    db_dir.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_dir / db_name))
    try:
        conn.execute(
            "CREATE TABLE tool_calls (id TEXT, tool_name TEXT, called_at TEXT, started_at TEXT, "
            "cc_session_id TEXT, agent_id TEXT, input_json TEXT, output_json TEXT, "
            "deliveries_json TEXT, tool_use_id TEXT, latency_ms INTEGER)"
        )
        for i, row in enumerate(rows):
            conn.execute(
                "INSERT INTO tool_calls (id, tool_name, called_at, started_at, cc_session_id, "
                "agent_id, input_json, output_json, deliveries_json, tool_use_id, latency_ms) "
                "VALUES (?,?,?,?,?,?,?,?,?,?,?)",
                (
                    row.get("id", f"row-{i}"), row.get("tool_name"), row.get("called_at"),
                    row.get("started_at"), row.get("cc_session_id"), row.get("agent_id"),
                    row.get("input_json"), row.get("output_json"), row.get("deliveries_json"),
                    row.get("tool_use_id"), row.get("latency_ms"),
                ),
            )
        conn.commit()
    finally:
        conn.close()


def _parse_outcome_table(rendered, outcome):
    """Parses render_map's `## {outcome}` 6-column table into {link: {label, decision, retained,
    population, basis}}. Used by RenderedTableIntegrity to assert on the RENDERED text itself,
    not just build_cells -- mutants #9/#10 drop a column or every row from render_map, and only a
    test that parses the actual rendered string can see either.
    """
    lines = rendered.splitlines()
    header = f"## {outcome}"
    start = None
    for i, line in enumerate(lines):
        if line.strip() == header:
            start = i
            break
    if start is None:
        return {}
    i = start + 1
    while i < len(lines) and not lines[i].startswith("| link |"):
        i += 1
    i += 2  # header row + separator row
    table = {}
    while i < len(lines) and lines[i].startswith("|"):
        cells = [c.strip() for c in lines[i].strip().strip("|").split("|")]
        if len(cells) == 6:
            link, label, decision, retained, population, basis = cells
            table[link] = {
                "label": label, "decision": decision, "retained": retained,
                "population": population, "basis": basis,
            }
        i += 1
    return table


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

            manifest = _load_manifest(corpus_dir)
            rendered = observability.render_map(cov, manifest)
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
            self.assertEqual(cov["fields"]["deliveries_total"]["retained"], 0)
            self.assertEqual(cov["fields"]["subagent_turns"]["retained"], 0)
            self.assertEqual(cov["fields"]["tool_events_total"]["retained"], 1)

            cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}

            # Positive control: a base "measurable now" cell whose field IS nonzero here stays
            # "measurable now" -- proves the downgrade below is about the zero, not a blanket
            # rewrite of every cell.
            positive = cells[("context", "signal/request")]
            self.assertEqual(positive["label"], "measurable now")
            self.assertEqual(positive["retained_number"], 1)

            # The cell under test: base "measurable now", but its coverage field is 0 in this
            # corpus, so it must be downgraded, with a basis that says why.
            downgraded = cells[("context", "delivery/action")]
            self.assertEqual(downgraded["label"], "needs adjudication")
            self.assertEqual(downgraded["retained_number"], 0)
            self.assertIn("0", downgraded["basis"])

            # General invariant, over every cell: a zero coverage number in the outcome's
            # DECISION-BEARING window (R62: decision for mistakes, retained for the rest) is
            # never rendered as "measurable now".
            for cell in cells.values():
                decision_bearing = (
                    cell["decision_number"] if cell["outcome"] == "mistakes" else cell["retained_number"]
                )
                if decision_bearing == 0:
                    self.assertNotEqual(cell["label"], "measurable now")


class Rediscovery(unittest.TestCase):
    def test_rediscovery_is_needs_adjudication(self):
        self.assertEqual(observability._REDISCOVERY_SPEC["label"], "needs adjudication")

        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
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

    def test_r58_base_table_fields_match_the_ruling(self):
        # Kills mutant #2: mistakes/opportunity swapped to a different coverage field (e.g.
        # "tool_use_turns", which the fixed coverage() doesn't even declare).
        expected_fields = {
            ("mistakes", "opportunity"): "assistant_text_turns",
            ("mistakes", "signal/request"): "prompt_interrupt_turns",
            ("mistakes", "delivery/action"): None,
            ("mistakes", "observed use"): None,
            ("mistakes", "checked outcome"): None,
            ("context", "opportunity"): "assistant_text_turns",
            ("context", "signal/request"): "tool_events_total",
            ("context", "delivery/action"): "deliveries_total",
            ("context", "observed use"): None,
            ("context", "checked outcome"): None,
            ("transfer", "opportunity"): None,
            ("transfer", "signal/request"): None,
            ("transfer", "delivery/action"): "transfer_filtered_deliveries",
            ("transfer", "observed use"): None,
            ("transfer", "checked outcome"): None,
            ("background-worker", "opportunity"): "tool_events_total",
            ("background-worker", "signal/request"): "agent_tool_uses",
            ("background-worker", "delivery/action"): "subagent_turns",
            ("background-worker", "observed use"): None,
            ("background-worker", "checked outcome"): None,
        }
        for key, field in expected_fields.items():
            self.assertEqual(observability._BASE_TABLE[key].get("field"), field, msg=str(key))


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


class ExclusionsMismatchRaisesOnPersistedMetaCorruption(unittest.TestCase):
    def test_persisted_meta_disagreeing_with_recompute_raises_even_when_turns_agree(self):
        # Isolates R57's OWN mismatch check (line ~533) from R64's two turns-membership checks.
        # ExclusionsMismatchRaises's fixture forces spec_sid KEPT at build time (excluded_sids=
        # set()) while coverage() always force-excludes it via join.SPEC_EXCLUDED_SIDS -- so
        # spec_sid ends up with a turns row that the recomputed excluded_cids also claims,
        # which trips R64's "turn_sids subset of kept_cids" check (line 548) FIRST, independently
        # of whether R57's own check still exists. That masks removal of R57's check under this
        # mutation (round0_4): assertRaises(ValueError) alone can't tell which guard fired.
        #
        # Here the build uses the DEFAULT excluded_sids (join.SPEC_EXCLUDED_SIDS, exactly what
        # coverage() recomputes), so turns.sid stays a subset of the recomputed kept cids and the
        # recomputed excluded cids stay disjoint from turns.sid -- R64's two checks are admitted
        # and stay silent. Only events_meta's own persisted counters are corrupted afterward, by
        # a direct UPDATE, with no session-set change at all -- so only R57's check can fire.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "normal-sid.jsonl", [
                _entry("n1", "2026-09-20T10:00:00Z", "normal-sid"),
            ])

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            conn = sqlite3.connect(str(events_db))
            persisted_kept = int(
                conn.execute(
                    "SELECT value FROM events_meta WHERE key = 'sessions_kept'"
                ).fetchone()[0]
            )
            conn.execute(
                "UPDATE events_meta SET value = ? WHERE key = 'sessions_kept'",
                (str(persisted_kept + 1),),
            )
            conn.commit()
            conn.close()

            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn("exclusions mismatch", str(ctx.exception))


class ExclusionsByReason(unittest.TestCase):
    def test_exclusions_by_reason_grouping(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            sdd_proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            kat_proj = _session_dir(corpus_dir, "01-.claude-kat", "p")

            _write_lines(sdd_proj / "headless-sid.jsonl", [
                _entry("h1", "2026-09-20T10:00:00Z", "headless-sid", entrypoint="sdk-cli"),
            ])
            _write_lines(sdd_proj / "normal-sid.jsonl", [
                _entry("n1", "2026-09-20T10:00:00Z", "normal-sid"),
            ])

            # Two divergent-duplicate pairs with DIFFERENT keepers/sids -- their reasons are
            # "divergent-duplicate-of:<keeper-a>" and "divergent-duplicate-of:<keeper-b>",
            # distinct full strings that must still collapse into ONE base-class key
            # ("divergent-duplicate-of") rather than two singleton keys. A mutant that drops the
            # reason.split(":", 1)[0] grouping (base = reason instead) is INVISIBLE on a fixture
            # using only a static, colon-free reason like "sdk-cli" alone -- this pair is what
            # actually exercises the split (round0_5).
            for tag in ("a", "b"):
                sid = f"div-sid-{tag}"
                kat_lines = [_entry(f"{tag}u{i}", f"2026-09-20T10:00:0{i}Z", sid) for i in range(5)]
                sdd_lines = kat_lines[:3] + [_entry(f"{tag}vX", "2026-09-20T10:00:09Z", sid)]
                _write_lines(kat_proj / f"{sid}.jsonl", kat_lines)
                _write_lines(sdd_proj / f"{sid}.jsonl", sdd_lines)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            cov = observability.coverage(events_db, corpus_dir)
            by_reason = cov["exclusions_by_reason"]
            self.assertEqual(by_reason.get("sdk-cli"), 1)
            self.assertEqual(by_reason.get("divergent-duplicate-of"), 2)
            self.assertTrue(
                all(":" not in k for k in by_reason),
                f"a dynamic reason leaked its full colon-suffixed form as a key: {by_reason!r}",
            )


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


class MissingCoverageFieldRaises(unittest.TestCase):
    """R60/mutant #1: a coverage field-name typo must RAISE, not silently read as 0."""

    def _win(self):
        return {
            "turns": {},
            "tool_events": {
                "total": observability._zero_flat_window(),
                observability.AGENT_TOOL_NAME: observability._zero_flat_window(),
            },
            "deliveries": {
                "total": observability._zero_flat_window(),
                "transfer_filtered": observability._zero_flat_window(),
                "with_tool_use_id": observability._zero_flat_window(),
            },
            "data_min": None, "data_max": None, "skipped_ts": 0,
        }

    def test_known_field_resolves(self):
        dec, ret, pop = observability._field_window_count(self._win(), "tool_events_total")
        self.assertEqual((dec, ret, pop), (0, 0, "all"))

    def test_unknown_field_raises_keyerror(self):
        # "tool_use_turns" is the exact stale field name the round-0 map/tests used before this
        # fix round -- it has no dispatcher branch in the fixed coverage().
        with self.assertRaises(KeyError) as ctx:
            observability._field_window_count(self._win(), "tool_use_turns")
        self.assertIn("no window-count dispatcher branch", str(ctx.exception))


class R61RetainedWindowDerivedFromData(unittest.TestCase):
    def test_retained_start_after_earliest_entry_raises(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2026-09-25T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
                "decision": {"start_utc": "2026-09-25T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn("R61: manifest.bounds.retained.start_utc", str(ctx.exception))

    def test_observed_ts_after_retained_end_raises(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2026-09-20T00:00:00Z"},
                "decision": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2026-09-20T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-25T10:00:00Z", "sid1", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn("R61: an observed ts", str(ctx.exception))


class DecisionBearingWindowDowngrade(unittest.TestCase):
    """R62: downgrade-on-zero reads the outcome's decision-bearing window -- decision for
    mistakes, retained for the other three."""

    def test_decision_window_downgrade_for_mistakes(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
                "decision": {"start_utc": "2026-09-25T00:00:00Z", "end_utc": "2026-09-26T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _assistant_text_entry("a1", "2026-09-20T10:00:00Z", "sid1"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}
            cell = cells[("mistakes", "opportunity")]
            self.assertEqual(cell["label"], "needs adjudication")
            self.assertIn(
                "downgraded: the decision-window coverage number for this link is 0",
                cell["basis"],
            )

    def test_retained_window_gates_non_mistakes_outcomes(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
                "decision": {"start_utc": "2026-09-25T00:00:00Z", "end_utc": "2026-09-26T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _agent_tool_use_entry("t1", "2026-09-20T10:00:00Z", "sid1"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}
            cell = cells[("context", "signal/request")]
            self.assertEqual(cell["label"], "measurable now")
            self.assertEqual(cell["retained_number"], 1)
            self.assertEqual(cell["decision_number"], 0)


class TransferDeliveryFilterMembers(unittest.TestCase):
    """R63: the transfer delivery filter is ONE named constant, one fixture per member."""

    def test_each_transfer_marker_member_is_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "obs-sid.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "obs-sid", content="hi"),
            ])
            rows = [
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:01Z",
                 "output_json": json.dumps([{"type": "text", "text": "<!-- operator-rule OP-1 -->\nbody"}])},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:02Z",
                 "output_json": json.dumps(
                     [{"type": "text", "text": "<!-- auto-injected get_guide('topic') -->\nbody"}]
                 )},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:03Z",
                 "deliveries_json": json.dumps(
                     [{"engine": "operator-rules", "ledger_keys": ["k"], "blocks": []}]
                 )},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:04Z",
                 "deliveries_json": json.dumps(
                     [{"engine": "guide-sections", "ledger_keys": ["k"], "blocks": []}]
                 )},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:05Z",
                 "deliveries_json": json.dumps(
                     [{"engine": "session-opener", "ledger_keys": ["k"], "blocks": []}]
                 )},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:06Z",
                 "deliveries_json": json.dumps(
                     [{"engine": "craft-skills", "ledger_keys": ["k"], "blocks": []}]
                 )},
            ]
            _write_usage_db(corpus_dir, rows)
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(
                cov["fields"]["deliveries_total"],
                {"decision": 6, "retained": 6, "population": "all"},
            )
            self.assertEqual(
                cov["fields"]["transfer_filtered_deliveries"],
                {"decision": 5, "retained": 5, "population": "all"},
            )


class R64SwapFixtureRaises(unittest.TestCase):
    def test_swap_fixture_raises_on_turn_sid_not_subset_of_kept(self):
        spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / f"{spec_sid}.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", spec_sid, content="hi"),
            ])
            _write_lines(proj / "other.jsonl", [
                _entry("u2", "2026-09-20T10:00:00Z", "other", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db, excluded_sids={"other"})
            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn(
                "R64: turns.sid contains cid(s) not in the recomputed kept set:",
                str(ctx.exception),
            )


class KeptSessionsWithZeroTurns(unittest.TestCase):
    def test_zero_turn_kept_session_is_explained_by_its_owning_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            big_sid, small_sid = "big-sid", "small-sid"
            # big has strictly more uuids (5) than small (3), and small's uuids are a strict
            # subset of big's -- attribute_entries()'s (-count, first_ts, cid) tie-break sorts
            # big first, so it claims every shared uuid; small ends up owning none of its own.
            big_lines = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", big_sid) for i in range(5)]
            small_lines = [_entry(f"u{i}", f"2026-09-20T10:00:0{i}Z", small_sid) for i in range(3)]
            _write_lines(proj / f"{big_sid}.jsonl", big_lines)
            _write_lines(proj / f"{small_sid}.jsonl", small_lines)

            sessions_list = transcripts.sessions(corpus_dir)
            excl = transcripts.exclusions(sessions_list, join.SPEC_EXCLUDED_SIDS)
            self.assertEqual(excl, {})  # different sids -- Stage D never compares them

            attribution = transcripts.attribute_entries(sessions_list, excl)
            big_cid = f".claude-sdd/{big_sid}"
            small_cid = f".claude-sdd/{small_sid}"
            self.assertEqual(attribution["u0"], big_cid)

            conn = sqlite3.connect(":memory:")
            try:
                conn.row_factory = sqlite3.Row
                conn.execute("CREATE TABLE turns (sid TEXT)")
                conn.execute("INSERT INTO turns (sid) VALUES (?)", (big_cid,))
                conn.commit()

                result = observability._kept_sessions_with_zero_turns(
                    sessions_list, excl, conn, attribution
                )
            finally:
                conn.close()
            self.assertEqual(result["count"], 1)
            self.assertEqual(result["cids"], [small_cid])
            self.assertIn(f"its uuids are owned by {big_cid}", result["detail"])


class ContextSignalCountsToolUseBlocks(unittest.TestCase):
    """R65: the context signal cell counts tool_use BLOCKS, not entries."""

    def test_one_entry_two_tool_use_blocks_counts_two(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _multi_tool_use_entry("t1", "2026-09-20T10:00:00Z", "sid1", ["Bash", "Read"]),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(
                cov["fields"]["tool_events_total"],
                {"decision": 2, "retained": 2, "population": "all"},
            )


class BackgroundWorkerSignalsNeverSummed(unittest.TestCase):
    """R66: background-worker signal's label coverage is Agent tool_uses; delegation turns
    render beside it as a second, separately named number -- never summed."""

    def test_agent_tool_uses_and_delegation_turns_are_separate_numbers(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            sid = "parent-sid"
            _write_lines(proj / f"{sid}.jsonl", [
                _agent_tool_use_entry("t1", "2026-09-20T10:00:00Z", sid),
            ])
            sub_dir = _subagent_dir(corpus_dir, "00-.claude-sdd", "p", sid)
            _write_lines(sub_dir / "sub1.jsonl", [
                _entry("s1", "2026-09-20T10:00:01Z", sid, content="do the thing"),
                _entry("s2", "2026-09-20T10:00:02Z", sid, content="do another thing"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)

            self.assertEqual(
                cov["fields"]["agent_tool_uses"],
                {"decision": 1, "retained": 1, "population": "all"},
            )
            self.assertEqual(
                cov["fields"]["delegation_turns"],
                {"decision": 2, "retained": 2, "population": "all"},
            )
            deleg = cov["fields"]["delegation_turns"]
            self.assertEqual(
                cov["cell_extra"][("background-worker", "signal/request")],
                "delegation turns (separate, never summed) -- decision: "
                f"{deleg['decision']}, retained: {deleg['retained']}",
            )


class SubagentTurnsRealNonzero(unittest.TestCase):
    """Mutant #4: subagent_turns forced to 0 -- assert the real non-zero value, not `== 0`."""

    def test_subagent_turns_reflects_real_subagent_population(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            sid = "parent-sid"
            _write_lines(proj / f"{sid}.jsonl", [
                _agent_tool_use_entry("t1", "2026-09-20T10:00:00Z", sid),
            ])
            sub_dir = _subagent_dir(corpus_dir, "00-.claude-sdd", "p", sid)
            _write_lines(sub_dir / "sub1.jsonl", [
                _entry("s1", "2026-09-20T10:00:01Z", sid, content="do the thing"),
                _entry("s2", "2026-09-20T10:00:02Z", sid, content="do another thing"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertGreater(cov["fields"]["subagent_turns"]["retained"], 0)
            self.assertEqual(cov["fields"]["subagent_turns"]["population"], "subagent")


class UsageRowsUnmappedRendersRulingSentence(unittest.TestCase):
    def test_unmapped_usage_rows_render_the_ruling_sentence(self):
        spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "obs-keep.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "obs-keep", content="hi"),
            ])
            rows = [
                {"cc_session_id": "obs-keep", "called_at": "2026-09-20T10:00:01Z",
                 "output_json": json.dumps([{"type": "text", "text": "no markers here"}])},
                {"cc_session_id": "ghost-sid", "called_at": "2026-09-20T10:00:02Z",
                 "output_json": json.dumps([{"type": "text", "text": "no markers here"}])},
                {"cc_session_id": spec_sid, "called_at": "2026-09-20T10:00:03Z",
                 "output_json": json.dumps([{"type": "text", "text": "no markers here"}])},
            ]
            _write_usage_db(corpus_dir, rows)
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["usage_rows_total"], 3)
            self.assertEqual(cov["usage_rows_unmapped"], 2)
            self.assertEqual(cov["usage_rows_unmapped_spec_excluded"], 1)

            manifest = _load_manifest(corpus_dir)
            rendered = observability.render_map(cov, manifest)
            self.assertIn(
                "usage rows with no kept session: 2 of 3 usage rows read "
                "(of which 1 from the spec-excluded session).",
                rendered,
            )


class SkippedTimestampDoesNotCrash(unittest.TestCase):
    def test_null_or_unparseable_ts_is_skipped_and_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", None, "sid1", content="hi"),
                _entry("u2", "2026-09-20T10:00:00Z", "sid1", content="hi again"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertGreaterEqual(cov["skipped_ts_count"], 1)


class ExcludedBySpecDetail(unittest.TestCase):
    def test_a_spec_sid_present_in_two_profiles_is_explained(self):
        spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj_sdd = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            proj_kat = _session_dir(corpus_dir, "01-.claude-kat", "p")
            _write_lines(proj_sdd / f"{spec_sid}.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", spec_sid, content="hi"),
            ])
            _write_lines(proj_kat / f"{spec_sid}.jsonl", [
                _entry("u2", "2026-09-20T10:00:00Z", spec_sid, content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertIn(f"sid {spec_sid} present in 2 profile(s)", cov["excluded_by_spec_detail"])


class MinorRulingSentencesRender(unittest.TestCase):
    def test_latency_share_and_join_coverage_sentences_render(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            manifest = _load_manifest(corpus_dir)
            rendered = observability.render_map(cov, manifest)
            self.assertIn("A1.5's latency share is measurable only for joined calls", rendered)
            self.assertIn("share carrying a tool_use_id", rendered)


class SessionsPerProjectWindowMembership(unittest.TestCase):
    """Mutant #3: the window-membership check replaced with `if True` -- today
    sessions_per_project has ZERO tests. A session with a ts only inside the wider retained
    window must NOT count toward the narrower decision window."""

    def test_membership_is_ts_based_not_vacuously_true(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2099-01-01T00:00:00Z"},
                "decision": {"start_utc": "2026-09-25T00:00:00Z", "end_utc": "2026-09-26T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid-in-decision.jsonl", [
                _entry("u1", "2026-09-25T12:00:00Z", "sid-in-decision", content="hi"),
            ])
            _write_lines(proj / "sid-retained-only.jsonl", [
                _entry("u2", "2026-09-01T12:00:00Z", "sid-retained-only", content="hi"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            entry = cov["sessions_per_project"]["p"]
            self.assertEqual(entry["decision"], 1)
            self.assertEqual(entry["retained"], 2)


class InterruptsCountTowardMistakesSignal(unittest.TestCase):
    """Mutant #7: interrupts dropped from the mistakes signal/request field."""

    def test_interrupts_and_prompts_both_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("p1", "2026-09-20T10:00:00Z", "sid1", content="a real prompt"),
                _interrupt_entry("i1", "2026-09-20T10:00:01Z", "sid1"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(
                cov["fields"]["prompt_interrupt_turns"],
                {"decision": 2, "retained": 2, "population": "all"},
            )


class ConstantsMatchRuling(unittest.TestCase):
    def test_agent_tool_name_is_agent_not_task(self):
        # Mutant #5: AGENT_TOOL_NAME = "Task".
        self.assertEqual(observability.AGENT_TOOL_NAME, "Agent")

    def test_transfer_delivery_markers_match_ruling_r63(self):
        # Mutant #6: the op/guide filter emptied.
        self.assertEqual(
            observability.TRANSFER_DELIVERY_MARKERS,
            frozenset(
                {"operator-rule", "get_guide", "operator-rules", "guide-sections", "session-opener"}
            ),
        )


class FirstExactJoinUsesExactMethod(unittest.TestCase):
    """Mutant #8: the first-exact-ts query using 'none' instead of 'exact'."""

    def test_first_exact_tool_event_ts_is_populated_by_an_exact_join(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry(
                    "t1", "2026-09-20T10:00:00Z", "sid1", type_="assistant",
                    content=[{
                        "type": "tool_use", "id": "tu-exact-1",
                        "name": "mcp__codescout__symbols", "input": {},
                    }],
                ),
            ])
            _write_usage_db(corpus_dir, [
                {"cc_session_id": "sid1", "tool_name": "symbols", "tool_use_id": "tu-exact-1",
                 "called_at": "2026-09-20T10:00:01Z"},
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            # No "none"-joined tool_events exist in this fixture at all, so a mutant that
            # substitutes join_method='none' into the MIN(ts) query would find zero matching
            # rows and return None here.
            self.assertEqual(cov["tool_events_by_method"].get("exact"), 1)
            self.assertIsNotNone(cov["first_exact_tool_event_ts"])
            self.assertIn("2026-09-20", cov["first_exact_tool_event_ts"])


class RenderedTableIntegrity(unittest.TestCase):
    """Mutants #9/#10: render_map dropping the label column, or dropping every row. Today's
    "4x5 labelled cells" test asserts on build_cells only -- this parses the RENDERED text."""

    def test_every_cell_label_and_number_survives_rendering(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("p1", "2026-09-20T10:00:00Z", "sid1", content="do something"),
                _assistant_text_entry("a1", "2026-09-20T10:00:01Z", "sid1"),
                _agent_tool_use_entry("t1", "2026-09-20T10:00:02Z", "sid1"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            manifest = _load_manifest(corpus_dir)
            rendered = observability.render_map(cov, manifest)

            cells = {(c["outcome"], c["link"]): c for c in observability.build_cells(cov)}
            for outcome in observability._OUTCOMES:
                table = _parse_outcome_table(rendered, outcome)
                expected_links = {link for (o, link) in observability._BASE_TABLE if o == outcome}
                if outcome == "transfer":
                    expected_links.add("rediscovery")
                self.assertEqual(set(table.keys()), expected_links, msg=outcome)
                for link, row in table.items():
                    cell = cells[(outcome, link)]
                    self.assertIn(row["label"], _VALID_LABELS)
                    self.assertEqual(row["label"], cell["label"])
                    expected_dec = (
                        "n/a" if cell["decision_number"] is None else str(cell["decision_number"])
                    )
                    expected_ret = (
                        "n/a" if cell["retained_number"] is None else str(cell["retained_number"])
                    )
                    self.assertEqual(row["decision"], expected_dec)
                    self.assertEqual(row["retained"], expected_ret)


if __name__ == "__main__":
    unittest.main()
