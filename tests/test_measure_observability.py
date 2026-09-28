"""Stage 1c tests: the observability map (Task 13, fix round 3).

Mirrors tests/test_measure_join.py's shape: unittest classes, modules loaded by path through
importlib (never a real package import), fixture helpers copied from that file rather than
imported (each test file owns its own copies, per convention).

Run: ~/work/claude/prompt-engineering/.venv/bin/python -m pytest tests/test_measure_observability.py -v
"""
import importlib.util
import json
import pathlib
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone

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
        # R70: render_map reads counts.usage_rows with a subscript, so a fixture manifest must
        # carry it; freeze-built fixtures (ProvenanceHeader, FinalizeBounds) carry real counts.
        "files": {}, "counts": {"transcripts": 0, "subagent_transcripts": 0, "usage_rows": 0},
        "versions": {},
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


# R68: render_map reads git when code_version is None. Fixture renders pass this fixed value so
# they are hermetic; RenderingCodeVersionDefault is the one test of the git path.
_CODE_VERSION = {"head": "f" * 40, "dirty": False}


def _sections(rendered):
    """{"## X" or "### X" heading: [its body lines]} for the rendered map. A body runs to the
    next heading of ANY level, so each appendix section is isolated from its neighbours."""
    out = {}
    current = None
    for line in rendered.splitlines():
        if line.startswith("## ") or line.startswith("### "):
            current = line
            out[current] = []
        elif current is not None:
            out[current].append(line)
    return out


def _table_rows(body):
    """The rows of the FIRST markdown table in `body`, as lists of stripped cells, header row
    first, separator row dropped."""
    rows = []
    started = False
    for line in body:
        if line.startswith("|"):
            started = True
            if set(line.replace("|", "").strip()) <= {"-"}:
                continue
            rows.append([c.strip() for c in line.strip().strip("|").split("|")])
        elif started:
            break
    return rows


def _prose(body):
    """The non-blank, non-table lines of a section body."""
    return [line for line in body if line.strip() and not line.startswith("|")]


def _freeze_fixture(tmp, profiles, usage_rows=(), corpus_id="fx-corpus"):
    """A corpus built by the REAL archive.freeze, so its manifest (files keys, counts,
    created_utc) has the shape Task 12's will. `profiles` is {profile_name: {name: [entries]}};
    a name "<sid>/subagents/<agent>" writes a subagent file. Every profile's project dir is "p".
    Returns (corpus_dir, manifest)."""
    archive = _load("archive")
    src = pathlib.Path(tmp) / "src"
    dirs = []
    for profile, files in profiles.items():
        project = src / profile / "projects" / "p"
        project.mkdir(parents=True, exist_ok=True)
        for name, entries in files.items():
            _write_lines(project / f"{name}.jsonl", entries)
        dirs.append(str(project))
    stage = pathlib.Path(tmp) / "stage"
    _write_usage_db(stage, list(usage_rows))
    manifest = archive.freeze(
        corpus_id,
        {
            "transcript_dirs": dirs,
            "usage_dbs": [str(stage / "usage_dbs" / "usage.db")],
            "repos": {},
            # Placeholder bounds, far from any fixture ts: finalize_bounds must replace both.
            "bounds": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2000-01-02T00:00:00Z"},
        },
        str(pathlib.Path(tmp) / "out"),
    )
    return pathlib.Path(tmp) / "out" / corpus_id, manifest


def _plus_one_second(created):
    """The test's own R75 end: a second-precision "...Z" created_utc plus one second."""
    dt = datetime.strptime(created, "%Y-%m-%dT%H:%M:%SZ") + timedelta(seconds=1)
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


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
            rendered = observability.render_map(cov, manifest, _CODE_VERSION)
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

    def test_background_worker_opportunity_basis_names_what_is_not_computed(self):
        # Minor ruling, fix round 2: the basis must not promise an A1.5 family split the map
        # never computes.
        self.assertEqual(
            observability._BASE_TABLE[("background-worker", "opportunity")]["basis"],
            "tool calls (the A1.5 task-family split is not computed in this map)",
        )


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
                # R70: every join method is PRESENT per day; absent ones are a present 0.
                self.assertEqual(
                    cov["tool_events_by_day"][day],
                    {"exact": 0, "heuristic": 0, "none": 0, "not_codescout": 1},
                )
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

    def test_observed_ts_exactly_at_retained_end_raises(self):
        # R71: the window is half-open, so a ts EQUAL to retained.end_utc is outside it. A
        # closed-interval check (`>` instead of `>=`) passes this fixture silently.
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2026-09-20T10:00:00Z"},
                "decision": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2026-09-20T10:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                _entry("u0", "2026-09-20T09:59:59Z", "sid1", content="hi"),
                _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="exactly at the end"),
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn("is at or after manifest.bounds.retained.end_utc", str(ctx.exception))


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
            # Minor ruling: the owned FRACTION, not "its uuids are owned by".
            self.assertEqual(result["detail"], f"{small_cid}: 3 of 3 uuids owned by {big_cid}")


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
            # Exact, in BOTH windows: a decision-only zeroing survived `> 0` on retained alone
            # (fix round 2's mutant04b).
            self.assertEqual(
                cov["fields"]["subagent_turns"],
                {"decision": 2, "retained": 2, "population": "subagent"},
            )


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
            rendered = observability.render_map(cov, manifest, _CODE_VERSION)
            self.assertIn(
                "usage rows with no kept session: 2 of 3 usage rows read "
                "(of which 1 from the spec-excluded session).",
                rendered,
            )


class SkippedTimestampDoesNotCrash(unittest.TestCase):
    """R71: NULL and unparseable ts rows are skipped AND counted, per table and per cause, in
    coverage() and in render_map -- and neither a None day key nor an unparseable FIRST entry
    (which becomes Session.first_ts) crashes anything. LOAD-BEARING: the two causes have
    DIFFERENT counts in every table and in the earliest-entry scan, so a swap of the cause labels
    anywhere (the round-2 reviewer's X1-X4) changes a number; equal counts made every swap
    invisible."""

    def test_null_or_unparseable_ts_is_skipped_and_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [
                # FIRST in file order, so it is Session.first_ts: round 1's
                # min(join.utc(first_ts)) raised ValueError on exactly this.
                _entry("u-bad", "not-a-timestamp", "sid1", content="hi"),
                _entry("u-null", None, "sid1", content="hi again"),
                _agent_tool_use_entry("t-null", None, "sid1"),
                _agent_tool_use_entry("t-bad", "garbage-ts", "sid1"),
                _agent_tool_use_entry("t-bad2", "garbage-2", "sid1"),
                _agent_tool_use_entry("t-ok", "2026-09-20T10:00:01Z", "sid1"),
                _entry("u-ok", "2026-09-20T10:00:00Z", "sid1", content="a valid one"),
                # Session-state records: no uuid (so no turn) and no ts field at all.
                {"type": "last-prompt", "sessionId": "sid1", "lastPrompt": "x"},
                {"type": "last-prompt", "sessionId": "sid1", "lastPrompt": "y"},
                {"type": "mode", "sessionId": "sid1", "mode": "normal"},
            ])
            marker = json.dumps([{"type": "text", "text": "<!-- operator-rule OP-1 -->\nbody"}])
            _write_usage_db(corpus_dir, [
                {"cc_session_id": "sid1", "called_at": None, "output_json": marker},
                {"cc_session_id": "sid1", "called_at": "", "output_json": marker},
                {"cc_session_id": "sid1", "called_at": "bogus", "output_json": marker},
                {"cc_session_id": "sid1", "called_at": "2026-09-20T10:00:02Z", "output_json": marker},
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)

            self.assertEqual(cov["skipped_ts"], {
                "turns": {"null": 2, "unparseable": 3},        # u-null, t-null; u-bad, t-bad, t-bad2
                "tool_events": {"null": 1, "unparseable": 2},  # t-null; t-bad, t-bad2
                "deliveries": {"null": 2, "unparseable": 1},   # called_at None and ""; "bogus"
            })
            # Skipped rows stay in the whole-DB tables; only the parseable one has a day.
            self.assertEqual(cov["tool_events_by_method"]["not_codescout"], 4)
            self.assertEqual(
                cov["tool_events_by_day"],
                {"2026-09-20": {"exact": 0, "heuristic": 0, "none": 0, "not_codescout": 1}},
            )
            # The scan: 10 top-level entries; 5 carry no ts (u-null, t-null and the 3 records),
            # 3 an unparseable one (u-bad, t-bad, t-bad2).
            self.assertEqual(cov["earliest_kept_top_level"], {
                "ts": "2026-09-20T10:00:00Z",
                "entries_total": 10,
                "entries_without_ts": 5,
                "entries_without_ts_by_type": {"last-prompt": 2, "mode": 1, "user": 1,
                                               "assistant": 1},
                "entries_unparseable_ts": 3,
            })
            # Only parseable rows are windowed: 1 of 4 deliveries, 1 of 4 tool_events.
            self.assertEqual(cov["fields"]["deliveries_total"]["retained"], 1)
            self.assertEqual(cov["fields"]["tool_events_total"]["retained"], 1)

            rendered = observability.render_map(cov, _load_manifest(corpus_dir), _CODE_VERSION)
            windows = _prose(_sections(rendered)["## Windows"])
            self.assertIn(
                "Rows skipped from every window, day and span for a NULL or empty ts -- "
                "turns: 2, tool_events: 1, deliveries: 2.",
                windows,
            )
            self.assertIn(
                "Rows skipped from every window, day and span for an unparseable ts -- "
                "turns: 3, tool_events: 2, deliveries: 1.",
                windows,
            )
            by_day = _sections(rendered)["### A1.6 -- joins by method (by day)"]
            self.assertIn(
                "Window: retained, by UTC day of the tool_use ts; the 3 tool_events rows with a "
                "NULL or unparseable ts appear in the overall table only.",
                _prose(by_day),
            )
            self.assertEqual(_table_rows(by_day)[1:], [["2026-09-20", "0", "0", "0", "1"]])
            provenance = _prose(_sections(rendered)["## Provenance"])
            self.assertIn(
                "- earliest kept top-level entry ts: 2026-09-20T10:00:00Z (the minimum over the "
                "kept top-level entries that carry a parseable ts)",
                provenance,
            )
            self.assertIn(
                "- 5 of 10 kept top-level entries carry no ts field (absent, null or empty) and 3 "
                "carry an unparseable ts; the no-ts entries by entry type: last-prompt 2, "
                "assistant 1, mode 1, user 1",
                provenance,
            )


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
            rendered = observability.render_map(cov, manifest, _CODE_VERSION)
            self.assertIn(
                "A1.5's latency share is measurable only for joined calls: 0 of 0 tool_events are "
                "joined (exact + heuristic), and 0 of those joined rows carry latency_ms on their "
                "usage row.",
                rendered,
            )
            self.assertIn("delivered items carrying a tool_use_id", rendered)


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
            rendered = observability.render_map(cov, manifest, _CODE_VERSION)

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


def _codescout_tool_use_entry(uuid, ts, sid, tool, tool_use_id):
    return _entry(
        uuid, ts, sid, type_="assistant",
        content=[{"type": "tool_use", "id": tool_use_id, "name": f"mcp__codescout__{tool}",
                  "input": {}}],
    )


def _attachment_entry(uuid, ts, sid, atype, text, hook_event, hook_name):
    e = _entry(uuid, ts, sid, type_="attachment")
    del e["message"]
    e["attachment"] = {
        "type": atype, "content": text, "hookEvent": hook_event, "hookName": hook_name,
    }
    return e


# R69: a REAL-shaped deliveries_json payload. Shape measured on the 2026-09-27 snapshot's usage DB
# (probes/task13-fix2-real.txt): every record is {engine: str, ledger_keys: [str], blocks:
# [{sha256: str, bytes: int}], hint: bool}, with as many blocks as keys. LOAD-BEARING: the
# non-empty `blocks` are what make join.py emit the key-NULL block rows; with `blocks: []` (the
# R63 fixture) there are none, and a count of table rows equals a count of deliveries.
_REAL_SHAPED_DELIVERIES = [
    {"engine": "guide-sections", "ledger_keys": ["librarian#Filter Syntax", "tracker-conventions"],
     "blocks": [{"sha256": "a" * 64, "bytes": 1812}, {"sha256": "b" * 64, "bytes": 944}],
     "hint": False},
    {"engine": "session-opener", "ledger_keys": ["project-activation-bootstrap"],
     "blocks": [{"sha256": "c" * 64, "bytes": 3001}], "hint": True},
]

_OP_RULE_OUTPUT = json.dumps([{"type": "text", "text": "<!-- operator-rule OP-1 -->\nbody"}])
_NO_MARKER_OUTPUT = json.dumps([{"type": "text", "text": "no markers here"}])

# The rich fixture's windows. LOAD-BEARING: retained.start_utc EQUALS the earliest KEPT top-level
# entry (q1's), while the spec-excluded session is earlier still -- so counting excluded sessions
# into the earliest-entry scan (mutant N7) trips R61's start check.
_RICH_BOUNDS = {
    "retained": {"start_utc": "2026-09-19T12:00:00Z", "end_utc": "2026-09-23T00:00:00Z"},
    "decision": {"start_utc": "2026-09-21T00:00:00Z", "end_utc": "2026-09-22T00:00:00Z"},
}


def _rich_corpus(tmp):
    """One fixture that reaches every appendix row with a known, non-trivial value. Returns
    (corpus_dir, events_db, spec_sid). Every count asserted in AppendixRenderedIntegrity is
    derived in the comments here."""
    spec_sid = next(iter(join.SPEC_EXCLUDED_SIDS))
    corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=_RICH_BOUNDS)
    p = _session_dir(corpus_dir, "00-.claude-sdd", "p")
    q = _session_dir(corpus_dir, "01-.claude-kat", "q")
    _write_lines(p / "k1.jsonl", [
        _entry("k1-p1", "2026-09-20T10:00:00Z", "k1", content="please fix the parser"),  # prompt
        _interrupt_entry("k1-i1", "2026-09-20T10:00:01Z", "k1"),                         # interrupt
        _assistant_text_entry("k1-a1", "2026-09-20T10:00:02Z", "k1"),                    # assistant_text
        _codescout_tool_use_entry("k1-t1", "2026-09-20T10:00:03Z", "k1", "symbols", "tu-exact"),
        _codescout_tool_use_entry("k1-t2", "2026-09-21T10:00:00Z", "k1", "grep", "tu-heur-a"),
        _codescout_tool_use_entry("k1-t3", "2026-09-21T11:00:00Z", "k1", "grep", "tu-heur-b"),
        _codescout_tool_use_entry("k1-t4", "2026-09-21T12:00:00Z", "k1", "tree", "tu-none"),
        _agent_tool_use_entry("k1-t5", "2026-09-21T13:00:00Z", "k1"),                    # not_codescout
        # h1 is a hac; h2 is its hook_success twin (same event, same text, 1 s apart) -> dropped;
        # h3 has no twin -> kept as hook_success_only. So 2 transcript_hook rows.
        _attachment_entry("k1-h1", "2026-09-21T13:00:05Z", "k1", "hook_additional_context",
                          "ctx text", "SessionStart", "SessionStart:startup"),
        _attachment_entry("k1-h2", "2026-09-21T13:00:06Z", "k1", "hook_success",
                          "ctx text", "SessionStart", "SessionStart:startup"),
        _attachment_entry("k1-h3", "2026-09-21T13:00:30Z", "k1", "hook_success",
                          "other text", "PostToolUse", "PostToolUse:Bash"),
    ])
    _write_lines(_subagent_dir(corpus_dir, "00-.claude-sdd", "p", "k1") / "agent-1.jsonl", [
        _entry("k1-s1", "2026-09-21T13:00:01Z", "k1", content="do the subtask"),  # delegation
        _assistant_text_entry("k1-s2", "2026-09-21T13:00:02Z", "k1"),             # assistant_text
    ])
    # small: a KEPT copy whose 2 uuids are a strict subset of k1's, so k1 owns both and small
    # gets zero turns -- "2 of 2 uuids owned by .claude-sdd/k1".
    _write_lines(p / "small.jsonl", [
        _entry("k1-p1", "2026-09-20T10:00:00Z", "small", content="please fix the parser"),
        _interrupt_entry("k1-i1", "2026-09-20T10:00:01Z", "small"),
    ])
    _write_lines(p / "headless.jsonl", [
        _entry("h-1", "2026-09-20T09:00:00Z", "headless", entrypoint="sdk-cli"),  # sdk-cli
    ])
    _write_lines(p / f"{spec_sid}.jsonl", [
        _entry("spec-1", "2026-09-19T00:00:00Z", spec_sid),  # excluded-by-spec, earliest of all
    ])
    _write_lines(q / "q1.jsonl", [
        _entry("q1-p1", "2026-09-19T12:00:00Z", "q1", content="hello"),  # the earliest KEPT entry
    ])
    _write_usage_db(corpus_dir, [
        # exact join of k1-t1; latency_ms set
        {"cc_session_id": "k1", "tool_name": "symbols", "tool_use_id": "tu-exact",
         "called_at": "2026-09-20 10:00:04", "latency_ms": 12},
        # heuristic join of k1-t2 via started_at (1 s); latency_ms set
        {"cc_session_id": "k1", "tool_name": "grep", "started_at": "2026-09-21 10:00:01",
         "called_at": "2026-09-21 10:00:01", "latency_ms": 7},
        # heuristic join of k1-t3 via called_at (started_at NULL, 2 s); NO latency_ms
        {"cc_session_id": "k1", "tool_name": "grep", "called_at": "2026-09-21 11:00:02"},
        # 1 usage_output_json item, carrying a tool_use_id; no candidate claims "tu-d1"
        {"cc_session_id": "k1", "tool_use_id": "tu-d1", "called_at": "2026-09-21 12:30:00",
         "output_json": _OP_RULE_OUTPUT},
        # 3 usage_deliveries_json items (3 ledger keys) in 6 rows (plus 3 block rows)
        {"cc_session_id": "k1", "called_at": "2026-09-21 12:31:00",
         "deliveries_json": json.dumps(_REAL_SHAPED_DELIVERIES)},
        # unmapped: a session not in the corpus, and the spec-excluded session
        {"cc_session_id": "ghost-sid", "called_at": "2026-09-21 12:32:00",
         "output_json": _NO_MARKER_OUTPUT},
        {"cc_session_id": spec_sid, "called_at": "2026-09-21 12:33:00",
         "output_json": _NO_MARKER_OUTPUT},
    ])
    events_db = pathlib.Path(tmp) / "events.db"
    join.build_events(corpus_dir, events_db)
    return corpus_dir, events_db, spec_sid


class AppendixRenderedIntegrity(unittest.TestCase):
    """R70: parse-level tests of the rendered APPENDIX, like RenderedTableIntegrity for the
    label tables -- every section, every row and every value, against the rich fixture's known
    coverage. Kills N2 (Am.4(a) count), N4 (by-day rows dropped), N5 (heuristic_rest not
    subtracted), N6 (first exact ts), N8 (zero-turn explanation) and N9 (exclusions rows)."""

    def test_every_appendix_section_row_and_value(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, events_db, spec_sid = _rich_corpus(tmp)
            cov = observability.coverage(events_db, corpus_dir)
            rendered = observability.render_map(cov, _load_manifest(corpus_dir), _CODE_VERSION)
            sections = _sections(rendered)
            whole_db = (
                "Window: retained, as the whole events DB (rows skipped above for their ts "
                "included)."
            )

            appendix = [h for h in sections if h.startswith("### ")]
            self.assertEqual(appendix, [
                "### A1.6 -- joins by method (overall)",
                "### A1.6 -- joins by method (by day)",
                "### A1.6 -- delivery coverage by source",
                "### A1.6 -- sessions per project and window",
                "### A1.6 -- exclusions by reason",
                "### A1.6 -- turns by kind",
                "### A1.6 -- divergent-duplicate unowned uuids",
                "### A1.6 -- kept sessions with zero turns",
            ])

            body = sections["### A1.6 -- joins by method (overall)"]
            self.assertEqual(_table_rows(body), [
                ["method", "count"],
                ["exact", "1"], ["heuristic", "2"], ["none", "1"], ["not_codescout", "1"],
            ])
            self.assertEqual(_prose(body), [
                whole_db,
                "Amendment 4(a): tool_use_id was NULL on every usage row until 6f6349ca; it "
                "appears per session from that session's /mcp. Usage rows with a non-NULL "
                "tool_use_id: 2. First exact join ts (the transcript tool_use ts): "
                "2026-09-20T10:00:03.000Z.",
                "Heuristic joins: 2 total, of which 1 matched via called_at (started_at NULL) "
                "and 1 matched via started_at directly.",
                "A1.5's latency share is measurable only for joined calls: 3 of 5 tool_events "
                "are joined (exact + heuristic), and 2 of those joined rows carry latency_ms on "
                "their usage row.",
            ])

            body = sections["### A1.6 -- joins by method (by day)"]
            self.assertEqual(_table_rows(body), [
                ["day", "exact", "heuristic", "none", "not_codescout"],
                ["2026-09-20", "1", "0", "0", "0"],
                ["2026-09-21", "0", "2", "1", "1"],
            ])
            self.assertEqual(_prose(body), [
                "Window: retained, by UTC day of the tool_use ts; the 0 tool_events rows with a "
                "NULL or unparseable ts appear in the overall table only.",
            ])

            body = sections["### A1.6 -- delivery coverage by source"]
            self.assertEqual(_table_rows(body), [
                ["source", "delivered items", "table rows", "unit"],
                ["transcript_hook", "2", "2",
                 "one per hook injection (a hook_success or hook_additional_context row)"],
                ["usage_deliveries_json", "3", "6",
                 "one per ledger key of a deliveries_json engine record; its block rows (key "
                 "NULL) are digests of the same deliveries and are not counted"],
                ["usage_output_json", "1", "1",
                 "one per operator-rule or get_guide marker match in output_json"],
            ])
            self.assertEqual(_prose(body), [
                whole_db,
                "hook_success_only: 1; hook_success_twins_dropped: 1.",
                "usage rows with no kept session: 2 of 7 usage rows read (of which 1 from the "
                "spec-excluded session).",
            ])

            body = sections["### A1.6 -- sessions per project and window"]
            self.assertEqual(_table_rows(body), [
                ["project", "retained", "decision"], ["p", "1", "1"], ["q", "1", "0"],
            ])
            self.assertEqual(_prose(body), [
                "Window: both, as columns; a kept session is in a window if any of its turns' "
                "ts is.",
            ])

            body = sections["### A1.6 -- exclusions by reason"]
            self.assertEqual(_table_rows(body), [
                ["reason", "count"], ["excluded-by-spec", "1"], ["sdk-cli", "1"],
            ])
            self.assertEqual(_prose(body), [
                "Window: retained, as every session in the corpus.",
                f"excluded-by-spec, data-wise: sid {spec_sid} present in 1 profile(s): "
                f".claude-sdd/{spec_sid}",
            ])

            body = sections["### A1.6 -- turns by kind"]
            self.assertEqual(_table_rows(body), [
                ["kind", "count"],
                ["assistant_text", "2"], ["assistant_thinking", "0"], ["delegation", "1"],
                ["interrupt", "1"], ["meta", "0"], ["prompt", "2"], ["tool_result", "0"],
                ["tool_use", "5"],
            ])
            self.assertEqual(_prose(body), [whole_db])

            body = sections["### A1.6 -- divergent-duplicate unowned uuids"]
            self.assertEqual(_table_rows(body), [])
            self.assertEqual(_prose(body), [
                "Window: retained, as every divergent-duplicate copy in the corpus.", "none",
            ])

            body = sections["### A1.6 -- kept sessions with zero turns"]
            self.assertEqual(_table_rows(body), [])
            self.assertEqual(_prose(body), [
                whole_db,
                "count: 1.",
                ".claude-sdd/small: 2 of 2 uuids owned by .claude-sdd/k1",
            ])

            windows = sections["## Windows"]
            self.assertEqual(_prose(windows), [
                "Both windows are half-open, [start, end).",
                "- retained: 2026-09-19T12:00:00Z to 2026-09-23T00:00:00Z",
                "- decision: 2026-09-21T00:00:00Z to 2026-09-22T00:00:00Z",
                "Data span observed across turns, tool_events and deliveries: "
                "2026-09-19T12:00:00.000Z to 2026-09-21T13:00:30.000Z.",
                "Rows skipped from every window, day and span for a NULL or empty ts -- "
                "turns: 0, tool_events: 0, deliveries: 0.",
                "Rows skipped from every window, day and span for an unparseable ts -- "
                "turns: 0, tool_events: 0, deliveries: 0.",
            ])

    def test_every_label_cell_value_on_the_rich_fixture(self):
        # Every cell's label, both windows' numbers and population, and the full basis of every
        # cell that carries a number -- derived in _rich_corpus's comments against _RICH_BOUNDS
        # (decision = 2026-09-21 only). Pins decision-window values that aggregate or retained-
        # only assertions leave free.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, events_db, _spec_sid = _rich_corpus(tmp)
            cov = observability.coverage(events_db, corpus_dir)
            rendered = observability.render_map(cov, _load_manifest(corpus_dir), _CODE_VERSION)
        na = ("n/a", "n/a", "n/a")
        unit = (
            "counted as delivered items: one per output_json marker match, one per "
            "deliveries_json ledger key (block rows not counted), one per transcript hook "
            "injection"
        )
        expected = {
            "mistakes": {
                "opportunity": ("measurable now", "1", "2", "all",
                                "decision points = assistant_text turns (of which top-level -- "
                                "decision: 0, retained: 1)"),
                "signal/request": ("needs adjudication", "0", "3", "all",
                                   "prompt + interrupt rows are the candidate population; "
                                   "whether each is a correction is judged (of which top-level "
                                   "-- decision: 0, retained: 3)"),
                "delivery/action": ("needs adjudication",) + na,
                "observed use": ("needs adjudication",) + na,
                "checked outcome": ("needs adjudication",) + na,
            },
            "context": {
                "opportunity": ("measurable now", "1", "2", "all",
                                "assistant_text turns (of which top-level -- decision: 0, "
                                "retained: 1)"),
                "signal/request": ("measurable now", "4", "5", "all",
                                   "tool_use blocks (tool_events total)"),
                "delivery/action": ("measurable now", "6", "6", "all",
                                    f"deliveries by source, {unit} (delivered items carrying a "
                                    "tool_use_id -- decision: 1 of 6; retained: 1 of 6)"),
                "observed use": ("needs adjudication",) + na,
                "checked outcome": ("needs adjudication",) + na,
            },
            "transfer": {
                "opportunity": ("needs adjudication",) + na,
                "signal/request": ("needs adjudication",) + na,
                "delivery/action": ("measurable now", "4", "4", "all",
                                    "deliveries whose engine_or_hook names a transfer-carrying "
                                    "engine (TRANSFER_DELIVERY_MARKERS: operator-rule, get_guide, "
                                    f"operator-rules, guide-sections, session-opener), {unit}"),
                "observed use": ("needs adjudication",) + na,
                "checked outcome": ("needs adjudication",) + na,
                "rediscovery": ("needs adjudication",) + na,
            },
            "background-worker": {
                "opportunity": ("measurable now", "4", "5", "all",
                                "tool calls (the A1.5 task-family split is not computed in this "
                                "map)"),
                "signal/request": ("measurable now", "1", "1", "all",
                                   "Agent tool_uses (delegation turns (separate, never summed) -- "
                                   "decision: 1, retained: 1)"),
                "delivery/action": ("measurable now", "2", "2", "subagent",
                                    "subagent turns (agent_path set)"),
                "observed use": ("needs adjudication",) + na,
                "checked outcome": ("unobservable",) + na,
            },
        }
        for outcome, rows in expected.items():
            table = _parse_outcome_table(rendered, outcome)
            self.assertEqual(set(table), set(rows), msg=outcome)
            for link, want in rows.items():
                got = table[link]
                self.assertEqual(
                    (got["label"], got["decision"], got["retained"], got["population"]),
                    want[:4], msg=f"{outcome}/{link}",
                )
                if len(want) == 5:
                    self.assertEqual(got["basis"], want[4], msg=f"{outcome}/{link}")


class DeliveredItemsNotRows(unittest.TestCase):
    """R69: a delivery cell counts DELIVERED ITEMS. For usage_deliveries_json that is the key
    rows only -- the block rows are digests of the same deliveries."""

    def test_a_real_shaped_engine_record_counts_its_keys_not_its_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "obs-sid.jsonl", [
                _entry("u1", "2026-09-20T10:00:00Z", "obs-sid", content="hi"),
                _attachment_entry("h1", "2026-09-20T10:00:01Z", "obs-sid",
                                  "hook_additional_context", "ctx", "SessionStart",
                                  "SessionStart:startup"),
            ])
            _write_usage_db(corpus_dir, [
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:02Z",
                 "deliveries_json": json.dumps(_REAL_SHAPED_DELIVERIES)},
                {"cc_session_id": "obs-sid", "called_at": "2026-09-20T10:00:03Z",
                 "tool_use_id": "tu-9", "output_json": _OP_RULE_OUTPUT},
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)

            # The fixture is load-bearing only if join.py really emitted block rows for it.
            conn = sqlite3.connect(str(events_db))
            try:
                rows = conn.execute(
                    "SELECT key IS NULL, COUNT(*) FROM deliveries "
                    "WHERE source = 'usage_deliveries_json' GROUP BY key IS NULL"
                ).fetchall()
            finally:
                conn.close()
            self.assertEqual(sorted(rows), [(0, 3), (1, 3)])

            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["deliveries_by_source"], {
                "transcript_hook": {"rows": 1, "items": 1},
                "usage_deliveries_json": {"rows": 6, "items": 3},
                "usage_output_json": {"rows": 1, "items": 1},
            })
            # 3 ledger keys + 1 marker + 1 hook; never the 8 table rows.
            self.assertEqual(
                cov["fields"]["deliveries_total"],
                {"decision": 5, "retained": 5, "population": "all"},
            )
            # guide-sections x2 + session-opener + operator-rule; the SessionStart hook is not.
            self.assertEqual(
                cov["fields"]["transfer_filtered_deliveries"],
                {"decision": 4, "retained": 4, "population": "all"},
            )

            rendered = observability.render_map(cov, _load_manifest(corpus_dir), _CODE_VERSION)
            unit = (
                "counted as delivered items: one per output_json marker match, one per "
                "deliveries_json ledger key (block rows not counted), one per transcript hook "
                "injection"
            )
            context = _parse_outcome_table(rendered, "context")["delivery/action"]
            self.assertEqual((context["decision"], context["retained"]), ("5", "5"))
            self.assertIn(unit, context["basis"])
            self.assertIn(
                "delivered items carrying a tool_use_id -- decision: 1 of 5; retained: 1 of 5",
                context["basis"],
            )
            transfer = _parse_outcome_table(rendered, "transfer")["delivery/action"]
            self.assertEqual((transfer["decision"], transfer["retained"]), ("4", "4"))
            self.assertIn(unit, transfer["basis"])


class HalfOpenWindows(unittest.TestCase):
    """R71: every window membership test is half-open, [start, end): a row AT the decision
    window's start is in it, a row AT its end is not -- for turns, tool_events, deliveries and
    sessions-per-project alike."""

    def test_start_is_inside_and_end_is_outside(self):
        with tempfile.TemporaryDirectory() as tmp:
            bounds = {
                "retained": {"start_utc": "2026-09-20T00:00:00Z", "end_utc": "2026-09-23T00:00:00Z"},
                "decision": {"start_utc": "2026-09-21T00:00:00Z", "end_utc": "2026-09-22T00:00:00Z"},
            }
            corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "s1.jsonl", [
                _assistant_text_entry("a-start", "2026-09-21T00:00:00Z", "s1"),
                _agent_tool_use_entry("t-start", "2026-09-21T00:00:00Z", "s1"),
                _assistant_text_entry("a-end", "2026-09-22T00:00:00Z", "s1"),
                _agent_tool_use_entry("t-end", "2026-09-22T00:00:00Z", "s1"),
            ])
            # s2's ONLY turn is exactly at the decision end: in retained, not in decision.
            _write_lines(proj / "s2.jsonl", [
                _entry("s2-p", "2026-09-22T00:00:00Z", "s2", content="late"),
            ])
            _write_usage_db(corpus_dir, [
                {"cc_session_id": "s1", "called_at": "2026-09-21 00:00:00",
                 "output_json": _OP_RULE_OUTPUT},
                {"cc_session_id": "s1", "called_at": "2026-09-22 00:00:00",
                 "output_json": _OP_RULE_OUTPUT},
            ])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            for field in ("assistant_text_turns", "tool_events_total", "agent_tool_uses",
                          "deliveries_total", "transfer_filtered_deliveries"):
                self.assertEqual(
                    (cov["fields"][field]["decision"], cov["fields"][field]["retained"]),
                    (1, 2), msg=field,
                )
            self.assertEqual(cov["sessions_per_project"]["p"], {"retained": 2, "decision": 1})
            # The top-level subsets ride the same predicate: a-start is top-level and in the
            # decision window; s2-p (a prompt AT the end) is retained-only.
            self.assertEqual(
                cov["cell_extra"][("mistakes", "opportunity")],
                "of which top-level -- decision: 1, retained: 2",
            )
            self.assertEqual(
                cov["cell_extra"][("mistakes", "signal/request")],
                "of which top-level -- decision: 0, retained: 1",
            )


class StrictReadsRaise(unittest.TestCase):
    """R70: no `.get(..., default)` on a coverage, events_meta or closed-set key. A missing key
    RAISES; only a PRESENT 0 reads 0. Kills N1 (a kind misspelt) and N3 (a counter misspelt)
    by construction, and the general rule for every other key."""

    def _simple(self, tmp):
        corpus_dir = _make_corpus(pathlib.Path(tmp))
        proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
        _write_lines(proj / "sid1.jsonl", [
            _entry("u1", "2026-09-20T10:00:00Z", "sid1", content="hi"),
            _assistant_text_entry("a1", "2026-09-20T10:00:01Z", "sid1"),
            _agent_tool_use_entry("t1", "2026-09-20T10:00:02Z", "sid1"),
        ])
        _write_usage_db(corpus_dir, [
            {"cc_session_id": "sid1", "called_at": "2026-09-20T10:00:03Z",
             "output_json": _OP_RULE_OUTPUT},
        ])
        events_db = pathlib.Path(tmp) / "events.db"
        join.build_events(corpus_dir, events_db)
        return corpus_dir, events_db

    def test_every_events_meta_counter_read_is_strict(self):
        for key in ("sessions_kept", "sessions_excluded", "tool_events_total",
                    "tool_events_heuristic", "heuristic_via_called_at", "hook_success_only",
                    "hook_success_twins_dropped", "deliveries_unmapped_session",
                    "tool_events_exact", "tool_events_none", "tool_events_not_codescout"):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as tmp:
                corpus_dir, events_db = self._simple(tmp)
                conn = sqlite3.connect(str(events_db))
                conn.execute("DELETE FROM events_meta WHERE key = ?", (key,))
                conn.commit()
                conn.close()
                with self.assertRaises(KeyError) as ctx:
                    observability.coverage(events_db, corpus_dir)
                self.assertIn(key, str(ctx.exception))

    def test_every_coverage_key_render_map_reads_is_strict(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, events_db = self._simple(tmp)
            cov = observability.coverage(events_db, corpus_dir)
            manifest = _load_manifest(corpus_dir)
            baseline = observability.render_map(cov, manifest, _CODE_VERSION)
            # "meta" is returned for callers and never rendered; proven unread below rather
            # than assumed.
            for key in sorted(set(cov) - {"meta"}):
                broken = dict(cov)
                del broken[key]
                with self.subTest(key=key), self.assertRaises(KeyError):
                    observability.render_map(broken, manifest, _CODE_VERSION)
            broken = dict(cov)
            del broken["meta"]
            self.assertEqual(observability.render_map(broken, manifest, _CODE_VERSION), baseline)

    def test_a_missing_turn_kind_raises_and_a_present_zero_reads_zero(self):
        def _win(drop=None):
            return {
                "turns": {
                    k: observability._zero_pop_window()
                    for k in observability.TURN_KINDS if k != drop
                },
                "tool_events": {
                    "total": observability._zero_flat_window(),
                    observability.AGENT_TOOL_NAME: observability._zero_flat_window(),
                },
                "deliveries": {
                    "total": observability._zero_flat_window(),
                    "transfer_filtered": observability._zero_flat_window(),
                    "with_tool_use_id": observability._zero_flat_window(),
                },
            }
        self.assertEqual(
            observability._field_window_count(_win(), "assistant_text_turns"), (0, 0, "all")
        )
        for field, kind in (("assistant_text_turns", "assistant_text"),
                            ("prompt_interrupt_turns", "prompt"),
                            ("prompt_interrupt_turns", "interrupt"),
                            ("delegation_turns", "delegation")):
            with self.subTest(field=field, kind=kind), self.assertRaises(KeyError):
                observability._field_window_count(_win(drop=kind), field)

    def test_an_undeclared_closed_set_value_raises(self):
        for sql, value in (("UPDATE turns SET kind = ?", "bogus-kind"),
                           ("UPDATE tool_events SET join_method = ?", "bogus-method"),
                           ("UPDATE deliveries SET source = ?", "bogus-source")):
            with self.subTest(value=value), tempfile.TemporaryDirectory() as tmp:
                corpus_dir, events_db = self._simple(tmp)
                conn = sqlite3.connect(str(events_db))
                conn.execute(sql, (value,))
                conn.commit()
                conn.close()
                with self.assertRaises(ValueError) as ctx:
                    observability.coverage(events_db, corpus_dir)
                self.assertIn(value, str(ctx.exception))


class ProvenanceHeader(unittest.TestCase):
    """R68: the header is rendered from data -- corpus_id, the sources from the manifest's
    files keys, the usage-DB row count, the kept earliest ts, the freeze instant, the rendering
    code's version -- plus ONE fixed sentence, verbatim."""

    def test_header_is_data_plus_the_one_fixed_sentence(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, manifest = _freeze_fixture(tmp, {
                ".claude": {
                    "s-a": [_entry("a1", "2026-09-20T09:00:00Z", "s-a", content="hi")],
                    "s-a/subagents/agent-x": [
                        _entry("a2", "2026-09-20T09:00:01Z", "s-a", content="sub"),
                    ],
                },
                ".claude-sdd": {
                    "s-b": [_entry("b1", "2026-09-20T10:00:00Z", "s-b", content="hi")],
                    "s-c": [_entry("c1", "2026-09-20T11:00:00Z", "s-c", content="hi")],
                },
            }, usage_rows=[
                {"cc_session_id": "s-a", "called_at": "2026-09-20 09:00:02"},
                {"cc_session_id": "s-b", "called_at": "2026-09-20 10:00:02"},
            ])
            observability.finalize_bounds(corpus_dir)
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            final = _load_manifest(corpus_dir)
            created = manifest["created_utc"]

            rendered = observability.render_map(
                cov, final, {"head": "c0de" * 10, "dirty": True}
            )
            lines = rendered.splitlines()
            self.assertEqual(lines[0], "# Observability map -- fx-corpus")
            self.assertEqual(
                lines[2],
                "A map from any corpus other than the Task 12 freeze is provisional; Task 12 "
                "regenerates this file from the frozen real corpus.",
            )
            provenance = _sections(rendered)["## Provenance"]
            self.assertEqual(_prose(provenance), [
                "- corpus_id: fx-corpus",
                f"- freeze instant (manifest created_utc): {created}",
                "- rendering code: git HEAD " + "c0de" * 10 + " at render time; scripts/measure "
                "has uncommitted changes",
                "- repos at freeze (manifest repos): none",
                "- earliest kept top-level entry ts: 2026-09-20T09:00:00Z (the minimum over the "
                "kept top-level entries that carry a parseable ts)",
                "- 0 of 3 kept top-level entries carry no ts field (absent, null or empty) and 0 "
                "carry an unparseable ts; the no-ts entries by entry type: none",
                "- usage DBs: 1 file(s) in the manifest's files, holding 2 usage rows "
                "(manifest counts)",
                "Transcript sources, from the manifest's files:",
            ])
            self.assertEqual(_table_rows(provenance), [
                ["profile dir", "project dir", "top-level transcripts", "subagent transcripts"],
                ["00-.claude", "p", "1", "1"],
                ["01-.claude-sdd", "p", "2", "0"],
            ])
            windows = _prose(_sections(rendered)["## Windows"])
            end = _plus_one_second(created)
            self.assertIn(f"- retained: 2026-09-20T09:00:00Z to {end}", windows)

            clean = observability.render_map(cov, final, {"head": "c0de" * 10, "dirty": False})
            self.assertIn(
                "- rendering code: git HEAD " + "c0de" * 10 + " at render time; scripts/measure "
                "clean",
                clean.splitlines(),
            )

    def test_render_map_refuses_a_manifest_the_coverage_was_not_computed_from(self):
        # Each half of the refusal separately (the round-2 reviewer's X6 dropped the corpus_id
        # half and survived a bounds-only case): every other manifest field is the real one.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [_entry("u1", "2026-09-20T10:00:00Z", "sid1")])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            manifest = _load_manifest(corpus_dir)
            observability.render_map(cov, manifest, _CODE_VERSION)  # control: the real pair renders

            other_bounds = dict(manifest)
            other_bounds["bounds"] = {
                "retained": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2098-01-01T00:00:00Z"},
                "decision": {"start_utc": "2000-01-01T00:00:00Z", "end_utc": "2098-01-01T00:00:00Z"},
            }
            other_corpus = dict(manifest)
            other_corpus["corpus_id"] = "some-other-corpus"
            for name, other in (("bounds", other_bounds), ("corpus_id", other_corpus)):
                with self.subTest(differs=name):
                    with self.assertRaises(ValueError) as ctx:
                        observability.render_map(cov, other, _CODE_VERSION)
                    self.assertIn("render_map: coverage was computed for corpus", str(ctx.exception))


class RenderingCodeVersionDefault(unittest.TestCase):
    """R68: with no code_version, render_map reads the RENDERING code's version at render time:
    `git -C <repo> rev-parse HEAD` plus a dirty flag from `git status --porcelain --
    scripts/measure`."""

    def test_default_code_version_is_git_head_and_dirty_flag_at_render_time(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "sid1.jsonl", [_entry("u1", "2026-09-20T10:00:00Z", "sid1")])
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            rendered = observability.render_map(cov, _load_manifest(corpus_dir))
        head = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "status", "--porcelain", "--", "scripts/measure"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        state = "scripts/measure has uncommitted changes" if dirty else "scripts/measure clean"
        self.assertIn(
            f"- rendering code: git HEAD {head} at render time; {state}", rendered.splitlines()
        )

    def test_dirty_flag_reads_git_status_of_scripts_measure_only(self):
        # A throwaway repo pins both halves of the flag in every state of THIS checkout: the
        # test above can only observe whichever state the real tree happens to be in.
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp)

            def git(*args):
                return subprocess.run(
                    ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t",
                     *args],
                    capture_output=True, text=True, check=True,
                ).stdout.strip()

            git("init", "-q")
            (repo / "scripts" / "measure").mkdir(parents=True)
            (repo / "scripts" / "measure" / "m.py").write_text("x = 1\n")
            (repo / "README").write_text("r\n")
            git("add", "-A")
            git("commit", "-q", "-m", "c")
            head = git("rev-parse", "HEAD")

            self.assertEqual(
                observability.rendering_code_version(repo), {"head": head, "dirty": False}
            )
            (repo / "README").write_text("changed outside scripts/measure\n")
            self.assertEqual(
                observability.rendering_code_version(repo), {"head": head, "dirty": False}
            )
            (repo / "scripts" / "measure" / "m.py").write_text("x = 2\n")
            self.assertEqual(
                observability.rendering_code_version(repo), {"head": head, "dirty": True}
            )


class FinalizeBounds(unittest.TestCase):
    """R71: the pipeline owns the bounds. finalize_bounds sets retained = [earliest KEPT
    top-level entry ts, created_utc) and decision = [created_utc - 7 days, created_utc),
    rewrites manifest.json, and coverage() checks against the SAME earliest-entry function."""

    def test_bounds_come_from_kept_sessions_and_the_freeze_instant(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, manifest = _freeze_fixture(tmp, {".claude-sdd": {
                # The earliest entry is NOT first in file order: the scan is a min over every
                # entry, not Session.first_ts.
                "keep-a": [
                    _entry("ka-1", "2026-09-20T10:00:05Z", "keep-a", content="later"),
                    _entry("ka-0", "2026-09-20T10:00:00Z", "keep-a", content="earlier"),
                ],
                # LOAD-BEARING: an EXCLUDED session (sdk-cli) predating every kept entry pins
                # "kept" -- an unfiltered scan returns 2026-09-19T08:00:00Z (mutant N7).
                "headless": [
                    _entry("h-0", "2026-09-19T08:00:00Z", "headless", entrypoint="sdk-cli"),
                ],
            }})
            created = manifest["created_utc"]
            # R75: end = floor(created_utc) + 1 s; decision = the 7 days before that end.
            end = _plus_one_second(created)
            end_dt = datetime.fromisoformat(end.replace("Z", "+00:00"))
            expected = {
                "retained": {"start_utc": "2026-09-20T10:00:00Z", "end_utc": end},
                "decision": {
                    "start_utc": (end_dt - timedelta(days=7)).astimezone(timezone.utc)
                    .strftime("%Y-%m-%dT%H:%M:%SZ"),
                    "end_utc": end,
                },
            }
            self.assertEqual(
                observability.earliest_kept_top_level_ts(corpus_dir), "2026-09-20T10:00:00Z"
            )

            bounds = observability.finalize_bounds(corpus_dir)
            self.assertEqual(bounds, expected)
            on_disk = _load_manifest(corpus_dir)
            self.assertEqual(on_disk["bounds"], expected)
            # Nothing but the bounds changed.
            for key in set(manifest) - {"bounds"}:
                self.assertEqual(on_disk[key], manifest[key], msg=key)

            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["earliest_kept_top_level"]["ts"], "2026-09-20T10:00:00Z")

            # coverage() checks the SAME earliest-entry value: a start one second later raises.
            on_disk["bounds"]["retained"]["start_utc"] = "2026-09-20T10:00:01Z"
            (corpus_dir / "manifest.json").write_text(json.dumps(on_disk, indent=2, sort_keys=True))
            with self.assertRaises(ValueError) as ctx:
                observability.coverage(events_db, corpus_dir)
            self.assertIn("R61: manifest.bounds.retained.start_utc", str(ctx.exception))

    def test_no_parseable_kept_entry_raises(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, _manifest = _freeze_fixture(tmp, {".claude-sdd": {
                "keep-a": [_entry("ka-0", "not-a-ts", "keep-a")],
            }})
            with self.assertRaises(ValueError):
                observability.finalize_bounds(corpus_dir)

    def test_a_row_in_the_freezes_final_second_is_inside_the_window(self):
        # R75: archive._format_iso floors created_utc to the second, so a row written during
        # the freeze's final second carries a ts >= created_utc. end = floor + 1 s keeps it in.
        # Deterministic stand-in for the reviewer's probe_floor.py (which raced the real clock):
        # created_utc is set, and the late row sits 750 ms after it, inside the same second.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))  # created_utc 2026-09-26T00:00:00Z
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "s1.jsonl", [
                _entry("e-0", "2026-09-25T23:59:00Z", "s1", content="earlier"),
                _entry("e-late", "2026-09-26T00:00:00.750Z", "s1", content="same second"),
            ])
            bounds = observability.finalize_bounds(corpus_dir)
            self.assertEqual(bounds, {
                "retained": {"start_utc": "2026-09-25T23:59:00Z", "end_utc": "2026-09-26T00:00:01Z"},
                "decision": {"start_utc": "2026-09-19T00:00:01Z", "end_utc": "2026-09-26T00:00:01Z"},
            })
            events_db = pathlib.Path(tmp) / "events.db"
            join.build_events(corpus_dir, events_db)
            cov = observability.coverage(events_db, corpus_dir)  # R61's end raise stays silent
            self.assertEqual(cov["fields"]["prompt_interrupt_turns"]["retained"], 2)

    def test_the_end_floors_a_fractional_created_utc(self):
        # R75's floor: archive never writes a fraction, but a manifest that carries one must not
        # push the end past floor + 1 s.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir = _make_corpus(pathlib.Path(tmp))
            manifest = _load_manifest(corpus_dir)
            manifest["created_utc"] = "2026-09-26T00:00:00.400Z"
            (corpus_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))
            proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
            _write_lines(proj / "s1.jsonl", [_entry("e-0", "2026-09-25T23:59:00Z", "s1")])
            bounds = observability.finalize_bounds(corpus_dir)
            self.assertEqual(bounds["retained"]["end_utc"], "2026-09-26T00:00:01Z")
            self.assertEqual(bounds["decision"]["start_utc"], "2026-09-19T00:00:01Z")

    def test_an_earliest_entry_not_before_the_end_raises(self):
        # The empty-window guard (the round-2 reviewer's X5): the earliest kept entry AT the end
        # (half-open, so outside) and AFTER it must both refuse, and nothing may be written.
        for ts in ("2026-09-26T00:00:01Z", "2026-09-27T00:00:00Z"):
            with self.subTest(ts=ts), tempfile.TemporaryDirectory() as tmp:
                corpus_dir = _make_corpus(pathlib.Path(tmp))  # created_utc 2026-09-26T00:00:00Z
                before = (corpus_dir / "manifest.json").read_text()
                proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
                _write_lines(proj / "s1.jsonl", [_entry("e-0", ts, "s1")])
                with self.assertRaises(ValueError) as ctx:
                    observability.finalize_bounds(corpus_dir)
                self.assertIn("is not before the window end", str(ctx.exception))
                self.assertEqual((corpus_dir / "manifest.json").read_text(), before)


class RowsBeforeRetainedStartRaise(unittest.TestCase):
    """R76 / Amendment 6(a): coverage() raises when an events-DB row predates retained.start --
the half of "data falls outside the window" that R61's top-level scan cannot see. The
reviewer's probe_before_start.py shape: a KEPT session whose subagent turn and usage-row
delivery predate every top-level entry, with retained.start = the earliest top-level entry."""

    def _corpus(self, tmp, subagent_ts, called_at):
        bounds = {
            "retained": {"start_utc": "2026-09-20T10:00:00Z", "end_utc": "2026-09-23T00:00:00Z"},
            "decision": {"start_utc": "2026-09-21T00:00:00Z", "end_utc": "2026-09-22T00:00:00Z"},
        }
        corpus_dir = _make_corpus(pathlib.Path(tmp), bounds=bounds)
        proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
        _write_lines(proj / "k1.jsonl", [
            _entry("k1-p1", "2026-09-20T10:00:00Z", "k1", content="hello"),
            _assistant_text_entry("k1-a1", "2026-09-20T10:00:01Z", "k1"),
        ])
        _write_lines(_subagent_dir(corpus_dir, "00-.claude-sdd", "p", "k1") / "agent-1.jsonl", [
            _entry("k1-s1", subagent_ts, "k1", content="brief"),
        ])
        _write_usage_db(corpus_dir, [
            {"cc_session_id": "k1", "called_at": called_at, "output_json": _OP_RULE_OUTPUT},
        ])
        events_db = pathlib.Path(tmp) / "events.db"
        join.build_events(corpus_dir, events_db)
        return corpus_dir, events_db

    def test_a_subagent_turn_or_a_delivery_before_the_start_raises(self):
        for name, sub_ts, called_at in (
            ("subagent turn", "2026-09-20T09:00:00Z", "2026-09-20 10:30:00"),
            ("usage delivery", "2026-09-20T10:30:00Z", "2026-09-20 09:30:00"),
        ):
            with self.subTest(early=name), tempfile.TemporaryDirectory() as tmp:
                corpus_dir, events_db = self._corpus(tmp, sub_ts, called_at)
                # The top-level half is silent: retained.start IS the earliest top-level entry.
                self.assertEqual(
                    observability.earliest_kept_top_level_ts(corpus_dir), "2026-09-20T10:00:00Z"
                )
                with self.assertRaises(ValueError) as ctx:
                    observability.coverage(events_db, corpus_dir)
                self.assertIn("R76: an events-DB row's ts", str(ctx.exception))

    def test_rows_at_the_start_are_inside(self):
        # Half-open control: a subagent turn and a delivery EXACTLY at retained.start pass.
        with tempfile.TemporaryDirectory() as tmp:
            corpus_dir, events_db = self._corpus(tmp, "2026-09-20T10:00:00Z", "2026-09-20 10:00:00")
            cov = observability.coverage(events_db, corpus_dir)
            self.assertEqual(cov["data_min_ts"], "2026-09-20T10:00:00.000Z")
            self.assertEqual(cov["fields"]["subagent_turns"]["retained"], 1)
            self.assertEqual(cov["fields"]["deliveries_total"]["retained"], 1)


class JoinMethodCountsMatchMeta(unittest.TestCase):
    """R77: the joins-by-method table's row counts must equal events_meta's
tool_events_exact / _heuristic / _none / _not_codescout; a mismatch raises. Each counter is
corrupted in turn, so a check covering only some methods is caught."""

    def test_each_corrupted_join_method_counter_raises(self):
        for method in ("exact", "heuristic", "none", "not_codescout"):
            with self.subTest(method=method), tempfile.TemporaryDirectory() as tmp:
                corpus_dir = _make_corpus(pathlib.Path(tmp))
                proj = _session_dir(corpus_dir, "00-.claude-sdd", "p")
                _write_lines(proj / "sid1.jsonl", [
                    _agent_tool_use_entry("t1", "2026-09-20T10:00:00Z", "sid1"),
                ])
                events_db = pathlib.Path(tmp) / "events.db"
                join.build_events(corpus_dir, events_db)
                observability.coverage(events_db, corpus_dir)  # control: consistent meta passes
                conn = sqlite3.connect(str(events_db))
                conn.execute(
                    "UPDATE events_meta SET value = value + 1 WHERE key = ?",
                    (f"tool_events_{method}",),
                )
                conn.commit()
                conn.close()
                with self.assertRaises(ValueError) as ctx:
                    observability.coverage(events_db, corpus_dir)
                self.assertIn(f"events_meta.tool_events_{method}", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
