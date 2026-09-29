"""Thin CLI for the System-1 base-rate measurement pipeline (R126).

Task 9 adds the `gate` subcommand; the labelled-sample subcommands (Task 5 of
docs/superpowers/plans/2026-09-29-system1-labelled-sample.md) are below it.

    run.py gate --dry --out <file>     build all 81 gate inputs, render every prompt, report
                                       prompt sizes; makes NO model call (R121, Task 9a)
    run.py gate --out <file> --log-dir <dir>
                                       the real gate (Task 9b, only after the prompt's sha256 is
                                       registered in the spec's Amendments); exit 1 when it fails.
                                       It refuses to start with --votes other than 3, with
                                       --any-population, with a --log-dir inside the repository,
                                       or on any codex other than codex-cli 0.154.0 (R133, R138)

    run.py preflight --corpus C [--excluded-out PATH]
                                       builds a packet for EVERY frame unit and prints COUNTS only (units,
                                       built, refused, refusals by exception type / stratum / kind); with
                                       --excluded-out, writes the refusing units' case_keys (a JSON list,
                                       mode 0600, new file, outside the repo). Run it BEFORE `frame`: the
                                       main draw is one-shot, so a refusal found at render is unrecoverable
    run.py frame --corpus C --out FRAME.json [--exclude-units FILE]
                                       counts + four code hashes (sampler, packet, estimate, run), no text;
                                       may be written anywhere. --exclude-units removes those case_keys
                                       BEFORE counting and records {count, sha256} of the file. Editing
                                       run.py (or any hashed file) after `frame` invalidates the frame file
    run.py draw --corpus C --set DIR --seed S --substantive N --routine M [--exclude-set DIR2]
                [--frame FRAME.json --exclude-units FILE]
                                       stratified draw into a NEW private (mode 700) set directory
                                       OUTSIDE the repo: key.json (private, holds session ids) and
                                       draw.json; prints counts only (incl. distinct sessions per stratum).
                                       --exclude-units is refused unless FILE's sha256 is the one FRAME.json
                                       registered; pilot and main draws use the same file
    run.py render --corpus C --set DIR builds every blinded packet into DIR/packets and fills
                                       draw.json's sha256s; a token-shaped string aborts it with
                                       nothing written; prints counts only
    run.py estimate --set DIR --frame FRAME.json --seed S --out RESULT.json
                                       aggregates only (no case id, note or per-case label); S must be
                                       the seed the set was drawn with and the frame's four code hashes
                                       must match the files on disk; each stratum block carries
                                       n_sessions (distinct sessions among its labelled units). PARTIAL,
                                       with no decision, until every drawn substantive case is labelled.
                                       Refuses to overwrite --out: a re-run needs a new path
    run.py export --set DIR --out-dir D
                                       the committable record: case ids, packet hashes, strata,
                                       kinds and labels WITHOUT notes; D must be INSIDE the repo
                                       (the opposite of the private writers above). Re-runnable: the
                                       draw manifest must stay byte-identical, an exported label file
                                       may only grow; every committed value is validated

Run from the repo root with ~/work/claude/prompt-engineering/.venv/bin/python.
"""
import argparse
import contextlib
import hashlib
import json
import os
import pathlib
import random
import re
import sys

_HERE = pathlib.Path(__file__).resolve().parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))
import judge  # noqa: E402  (sibling module, per miner.py's idiom)
import archive  # noqa: E402
import estimate as estimator  # noqa: E402
import label  # noqa: E402
import packet  # noqa: E402
import sampler  # noqa: E402

BOOTSTRAP_SAMPLES = 10000
STRATA = sampler.STRATA
# sampler.frame emits exactly these two unit kinds as string literals (no constant exists there); a test
# pins this tuple to what frame() really produces so it cannot drift.
KINDS = ("top", "handback")
MAX_SECONDS = 86400 * 7  # a labelling time of a week or more is not a labelling time
_HEX64 = re.compile(r"[0-9a-f]{64}")
LABEL_EXPORT_FIELDS = ("case_id", "packet_sha256", "labels", "delivery", "recall", "seconds")
# A set directory looks like 2026-09-29-labelled-main; that name becomes the committed file names, so it
# may not be a bare session id, nor contain a UUID-shaped run (which the pattern alone would let through).
_SET_NAME = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}-[a-z][a-z0-9-]{0,40}")
_UUID_RUN = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
SEED_LIMIT = 2 ** 32
_CASE_ID = re.compile(r"[0-9a-f]{10}")
# The code whose behaviour the registered decision depends on: what a draw and a packet are (sampler, packet),
# how the bootstrap and the rule run (estimate), and the constants and wiring around them (run:
# BOOTSTRAP_SAMPLES, the strata blocks). `frame` records their sha256; `estimate` refuses if any differs. run.py
# is hashed from the file on disk, so EDITING run.py AFTER `frame` invalidates the frame file -- which is the point.
CODE_FILES = ("sampler.py", "packet.py", "estimate.py", "run.py")


def _parser():
    ap = argparse.ArgumentParser(prog="run.py", description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gate", help="the judge's validity gate (spec § Judge protocol)")
    g.add_argument("--dry", action="store_true",
                   help="build every input and render every prompt; make NO model call")
    g.add_argument("--repo", default=str(judge.REPO_ROOT))
    g.add_argument("--rtd-doc", help=f"default: <repo>/{judge.RTD_DOC}")
    g.add_argument("--controls-doc", help=f"default: <repo>/{judge.CONTROLS_DOC}")
    g.add_argument("--global-claude-md", default=str(judge.GLOBAL_CLAUDE_MD),
                   help="the operator's global CLAUDE.md (R118's undated lessons)")
    g.add_argument("--votes", type=int, default=3,
                   help="votes per item; a live gate refuses anything but 3 (R138)")
    g.add_argument("--log-dir", help="one Codex log per vote attempt (live runs); outside the repo")
    g.add_argument("--out", help="write the report here (default: stdout)")
    g.add_argument("--json-out", help="also write the full result as JSON")
    g.add_argument("--any-population", action="store_true",
                   help="skip the spec's 21/8/4/52 population check (dry runs and synthetic "
                        "fixtures only; a live gate refuses it, R138)")
    fr = sub.add_parser("frame", help="freeze the frame counts (no text)")
    fr.add_argument("--corpus", required=True)
    fr.add_argument("--out", required=True)
    fr.add_argument("--exclude-units", help="JSON list of case_keys (preflight --excluded-out) removed from the "
                                            "frame BEFORE counting; its count and sha256 are recorded")
    pf = sub.add_parser("preflight", help="build a packet for every frame unit; print refusal counts only")
    pf.add_argument("--corpus", required=True)
    pf.add_argument("--excluded-out", help="write the refusing units' case_keys here (new file, mode 0600, "
                                           "outside the repo)")
    dr = sub.add_parser("draw", help="draw a stratified sample into a new private set directory")
    dr.add_argument("--corpus", required=True)
    dr.add_argument("--set", required=True, help="new directory, outside the repo")
    dr.add_argument("--seed", type=int, required=True)
    dr.add_argument("--substantive", type=int, required=True)
    dr.add_argument("--routine", type=int, required=True)
    dr.add_argument("--exclude-set", help="another set directory whose units are removed from the pool")
    dr.add_argument("--frame", help="the frame file; required with --exclude-units, and refused when it "
                                    "records an exclusion the draw does not apply")
    dr.add_argument("--exclude-units", help="the same file frame was given; refused unless its sha256 is the "
                                            "one the frame file registered")
    rd = sub.add_parser("render", help="build every blinded packet of a set")
    rd.add_argument("--corpus", required=True)
    rd.add_argument("--set", required=True)
    es = sub.add_parser("estimate", help="aggregates only; PARTIAL until every substantive case is labelled")
    es.add_argument("--set", required=True)
    es.add_argument("--frame", required=True)
    es.add_argument("--seed", type=int, required=True, help="must equal the seed the set was drawn with")
    es.add_argument("--out", required=True,
                    help="a NEW file: estimate refuses to overwrite an existing --out, so re-running "
                         "after a relabel needs a new path")
    ex = sub.add_parser("export", help="the committable record of a set (no key, no notes, no packets)")
    ex.add_argument("--set", required=True)
    ex.add_argument("--out-dir", required=True, help="must be INSIDE the repo")
    return ap


def _gate(args, complete):
    # Reserve both destinations before any model call. Keep reservations on failure:
    # an interrupted attempt must not silently reuse or overwrite its evidence.
    paths = {key: pathlib.Path(value).resolve()
             for key, value in (("text", args.out), ("json", args.json_out)) if value}
    if len(set(paths.values())) != len(paths):
        raise ValueError("--out and --json-out must be different paths")
    for path in paths.values():
        if path.exists():
            raise FileExistsError(f"gate output already exists: {path}")
    with contextlib.ExitStack() as stack:
        outputs = {}
        for key, path in paths.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            outputs[key] = stack.enter_context(path.open("x"))
            outputs[key].write('{"state": "reserved; gate has not completed"}\n')
            outputs[key].flush()
        res = judge.run_gate(
            complete=complete, dry=args.dry, repo=args.repo, rtd_doc=args.rtd_doc,
            controls_doc=args.controls_doc, global_claude_md=args.global_claude_md,
            votes=args.votes, log_dir=args.log_dir,
            population=None if args.any_population else judge.GATE_POPULATION)
        text = judge.format_gate(res)
        for key, output in outputs.items():
            output.seek(0)
            output.truncate()
            output.write(text if key == "text" else json.dumps(res, indent=1, default=str))
        if not args.out:
            sys.stdout.write(text)
    if args.dry:
        return 0
    return 0 if res["passed"] else 1


# ---------------------------------------------------------------- labelled-sample subcommands

class Refused(ValueError):
    """A labelled-sample subcommand refuses; main() prints the message to stderr and exits 1.
    Messages name paths and opaque case ids, never packet text or a session id."""


def _read_json(path):
    return json.loads(pathlib.Path(path).read_text(encoding="utf-8"))


def _write_new(path, data, mode=0o600):
    """Create `path` (str or bytes), refusing an existing one (FileExistsError)."""
    if isinstance(data, str):
        data = data.encode("utf-8")
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, "wb") as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())


def _write_atomic(path, data, mode=0o600):
    """Replace `path` with `data` via a temp file and os.replace: a crash leaves the old file whole."""
    path = pathlib.Path(path)
    tmp = path.with_name(path.name + ".tmp")
    tmp.unlink(missing_ok=True)
    try:
        _write_new(tmp, data, mode)
        os.replace(tmp, path)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise


def _private_dir(path):
    """A set directory is private data: refuse one inside the repo."""
    try:
        archive._refuse_if_inside_repo(pathlib.Path(path))
    except ValueError:
        raise Refused(f"refusing {path}: it is inside the repository; label sets hold private data and live "
                      f"outside it") from None


def _load_set(set_dir):
    set_dir = pathlib.Path(set_dir)
    _private_dir(set_dir)
    try:
        draw, key = _read_json(set_dir / "draw.json"), _read_json(set_dir / "key.json")
    except OSError as e:
        raise Refused(f"{set_dir} is not a label set: {e.strerror}: {pathlib.Path(e.filename).name}") from None
    for c in draw["cases"]:
        if not _CASE_ID.fullmatch(c["case_id"]) or c["case_id"] not in key:
            raise Refused(f"{set_dir}: draw.json names a case_id key.json does not hold")
    drawn_ids = {c["case_id"] for c in draw["cases"]}
    if len(draw["cases"]) != len(drawn_ids):
        raise Refused(f"{set_dir}: draw.json lists a case more than once")
    if set(key) != drawn_ids:
        raise Refused(f"{set_dir}: key.json holds {len(set(key) - drawn_ids)} case(s) draw.json does not; "
                      f"the two must name the same cases")
    # key.json values feed estimate's messages and export's committed manifest: check them here, once, for
    # render, estimate and export alike, by field name and case_id only. A stratum is not a kind and a kind
    # is not a stratum, so each is checked against its own vocabulary.
    for cid, entry in key.items():
        for field, vocabulary in (("stratum", STRATA), ("kind", KINDS)):
            if entry[field] not in vocabulary:
                raise Refused(f"{set_dir}: key.json: case {cid}: field {field} is invalid")
    return set_dir, draw, key


def _frame_units(corpus):
    """sampler.frame, with any failure reduced to its TYPE: the exception text may quote a session id."""
    try:
        return sampler.frame(corpus)
    except Exception as e:
        raise Refused(f"the corpus could not be read ({type(e).__name__}); nothing written") from None


def _check_population(labels, relabels, tail):
    """The population rules estimate and export share: a case is labelled once, relabelled at most once,
    and only a labelled case is relabelled. Messages name the case_id, which is opaque; `tail` says what
    did not happen ("nothing exported" / "refusing to estimate")."""
    seen = set()
    for r in labels:
        if r["case_id"] in seen:
            raise Refused(f"duplicate label for case_id '{r['case_id']}'; {tail}")
        seen.add(r["case_id"])
    again = set()
    for r in relabels:
        if r["case_id"] not in seen:
            raise Refused(f"orphan relabel for case_id '{r['case_id']}' that was never labelled; {tail}")
        if r["case_id"] in again:
            raise Refused(f"duplicate relabel for case_id '{r['case_id']}'; {tail}")
        again.add(r["case_id"])


def _valid_seed(v):
    """A registered seed: an int (not a bool) in [0, 2**32). Anything wider could carry a session id."""
    return isinstance(v, int) and not isinstance(v, bool) and 0 <= v < SEED_LIMIT


def _valid_set_name(name):
    return bool(_SET_NAME.fullmatch(name)) and not _UUID_RUN.search(name)


def _valid_seconds(v):
    """A real, non-bool number in [0, MAX_SECONDS). NaN, inf and 10**400 all fail the comparison itself."""
    return not isinstance(v, bool) and isinstance(v, (int, float)) and 0 <= v < MAX_SECONDS



def _code_hashes():
    return {name: hashlib.sha256((_HERE / name).read_bytes()).hexdigest() for name in CODE_FILES}


def _private_file(path):
    """A file listing private case keys: refuse one inside the repo."""
    try:
        archive._refuse_if_inside_repo(pathlib.Path(path))
    except ValueError:
        raise Refused(f"refusing {path}: it is inside the repository; a file of case keys is private data and "
                      f"lives outside it") from None


def _load_exclusion(path):
    """(set of case_keys, sha256 of the file's bytes) for a --exclude-units file. ONE read, so the hash is of
    the bytes that were parsed. Refusals name the flag, never a value of the file."""
    try:
        data = pathlib.Path(path).read_bytes()
    except OSError as e:
        raise Refused(f"--exclude-units {pathlib.Path(path).name}: cannot be read ({e.strerror})") from None
    try:
        keys = json.loads(data.decode("utf-8"))
    except ValueError:
        keys = None
    if not (isinstance(keys, list) and all(isinstance(k, str) for k in keys)):
        raise Refused("--exclude-units: the file must be a JSON list of case_key strings")
    if len(set(keys)) != len(keys):
        raise Refused("--exclude-units: the file lists a case_key more than once")
    return set(keys), hashlib.sha256(data).hexdigest()


def _apply_exclusion(units, keys):
    absent = keys - {u.case_key for u in units}
    if absent:
        raise Refused(f"--exclude-units holds {len(absent)} case_key(s) that are not in this frame; the file "
                      f"belongs to another corpus")
    return [u for u in units if u.case_key not in keys]


def _cmd_frame(args):
    corpus = pathlib.Path(args.corpus)
    units = _frame_units(corpus)
    excluded = None
    if args.exclude_units:
        keys, digest = _load_exclusion(args.exclude_units)
        units = _apply_exclusion(units, keys)
        excluded = {"count": len(keys), "sha256": digest}
    if not units:
        raise Refused(f"{corpus} has no units; refusing to freeze an empty frame")
    counts = sampler.frame_counts(units)
    rec = {"corpus_id": corpus.resolve().name, "counts": counts, "n_units": len(units),
           "code_sha256": _code_hashes()}
    if excluded is not None:
        rec["excluded_units"] = excluded
    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    _write_new(out, json.dumps(rec, indent=1) + "\n", 0o644)
    print(json.dumps(counts, sort_keys=True))
    return 0


def _cmd_preflight(args):
    """Build a packet for EVERY frame unit and report only counts. The one-shot main draw has no redraw,
    so a unit whose packet cannot be built must be found (and registered as excluded) before the draw.
    COST: build_packet re-reads and re-parses its unit's transcript on every call, so a unit costs one parse of
    its transcript plus a scan of the entries before it; a transcript with u units and e entries costs ~u*e.
    Units are built grouped by transcript so the re-reads hit the OS page cache."""
    corpus = pathlib.Path(args.corpus)
    out = None
    if args.excluded_out:
        out = pathlib.Path(args.excluded_out)
        _private_file(out)
        if out.exists():
            raise Refused(f"--excluded-out {out} already exists; nothing written")
    units = _frame_units(corpus)
    if not units:
        raise Refused(f"{corpus} has no units; nothing to check")
    refused = []  # (exception type name, unit): the exception itself is dropped, its text may quote a session id
    for unit in sorted(units, key=lambda u: (u.transcript, u.first_entry_index)):
        try:
            packet.build_packet(corpus, unit, "preflight")
        except Exception as e:  # every kind: a refusal of any type is a unit the render would abort on
            refused.append((type(e).__name__, unit))

    def tally(values):
        counts = {}
        for v in values:
            counts[v] = counts.get(v, 0) + 1
        return counts

    print(json.dumps({"units": len(units), "built": len(units) - len(refused), "refused": len(refused),
                      "by_type": tally(t for t, _ in refused),
                      "by_stratum": tally(u.stratum for _, u in refused),
                      "by_kind": tally(u.kind for _, u in refused)}, sort_keys=True))
    if out is not None:
        out.parent.mkdir(parents=True, exist_ok=True)
        _write_new(out, json.dumps(sorted(u.case_key for _, u in refused)) + "\n", 0o600)
    return 0


def _excluded_case_keys(exclude_set):
    path = pathlib.Path(exclude_set) / "key.json"
    if not path.is_file():
        raise Refused(f"--exclude-set {exclude_set} holds no key.json; nothing to exclude")
    try:
        return {v["case_key"] for v in _read_json(path).values()}
    except (ValueError, KeyError, TypeError, AttributeError):
        raise Refused(f"--exclude-set {exclude_set}: key.json is malformed (not a case_id -> case_key map)") \
            from None


def _registered_exclusion(args):
    """The exclusion `draw` applies (a set of case_keys, empty when none), after checking it against the frame
    file: the treatment of the units preflight found is registered by the sha256 the frame recorded."""
    recorded = None
    if args.frame:
        try:
            frame = _read_json(args.frame)
        except (OSError, ValueError):
            raise Refused(f"--frame {pathlib.Path(args.frame).name} cannot be read as a frame file") from None
        recorded = frame.get("excluded_units") if isinstance(frame, dict) else None
    if args.exclude_units:
        if not args.frame:
            raise Refused("--exclude-units needs --frame: the frame file registers the exclusion by its sha256")
        if not isinstance(recorded, dict):
            raise Refused("the frame file records no exclusion; --exclude-units is refused")
        keys, digest = _load_exclusion(args.exclude_units)
        if digest != recorded.get("sha256"):
            raise Refused("--exclude-units does not match the frame's registered exclusion (sha256 differs)")
        return keys
    if recorded is not None:
        raise Refused("the frame file records an exclusion; pass --exclude-units with the registered file")
    return set()


def _cmd_draw(args):
    if not _valid_seed(args.seed):
        raise Refused("--seed must be an integer from 0 to 2**32 - 1")
    set_dir = pathlib.Path(args.set)
    _private_dir(set_dir)
    if not _valid_set_name(set_dir.name):
        raise Refused("set name is not allowed: it must look like 2026-09-29-labelled-main (a date, then a "
                      "lowercase name) and hold no session-id-shaped run")
    unfit = _registered_exclusion(args)
    exclude = _excluded_case_keys(args.exclude_set) if args.exclude_set else set()
    units = _frame_units(args.corpus)
    if unfit:
        units = _apply_exclusion(units, unfit)
    absent = exclude - {u.case_key for u in units}
    if absent:
        raise Refused(f"--exclude-set holds {len(absent)} case_key(s) that are not in this frame; the pilot must "
                      f"come from the same frame, or disjointness is not guaranteed")
    removed = sum(1 for u in units if u.case_key in exclude)
    try:
        drawn = sampler.draw(units, {"substantive": args.substantive, "routine": args.routine}, args.seed,
                             frozenset(exclude))
    except ValueError as e:
        raise Refused(str(e)) from None
    key = {}
    for u in drawn:
        cid = packet.case_id_for(u, args.seed)
        if cid in key:
            raise Refused("two drawn units share a case_id; choose another seed")
        key[cid] = {"case_key": u.case_key, "stratum": u.stratum, "kind": u.kind, "copy_id": u.copy_id,
                    "reasons": list(u.reasons)}
    order = list(key)
    random.Random(args.seed + 1).shuffle(order)
    try:
        set_dir.mkdir(parents=True, mode=0o700)
    except FileExistsError:
        raise Refused(f"set directory already exists: {set_dir}") from None
    _write_new(set_dir / "key.json", json.dumps(key, indent=1) + "\n")
    draw = {"set_id": set_dir.name, "seed": args.seed, "cases": [{"case_id": cid, "sha256": None} for cid in key],
            "order": order}
    _write_new(set_dir / "draw.json", json.dumps(draw, indent=1) + "\n")
    n_sub = sum(v["stratum"] == "substantive" for v in key.values())
    # aggregates only: how many distinct sessions the drawn units of each stratum come from
    sessions = {s: len({v["copy_id"] for v in key.values() if v["stratum"] == s}) for s in STRATA}
    print(f"drawn substantive={n_sub} routine={len(key) - n_sub} excluded={removed} "
          f"sessions_substantive={sessions['substantive']} sessions_routine={sessions['routine']}")
    return 0


def _cmd_render(args):
    set_dir, draw, key = _load_set(args.set)
    if any(c["sha256"] for c in draw["cases"]):
        raise Refused(f"{set_dir} is already rendered; labels bind to its packet hashes, so it is not re-rendered")
    by_key = {u.case_key: u for u in _frame_units(args.corpus)}
    built = []
    for c in draw["cases"]:
        cid = c["case_id"]
        unit = by_key.get(key[cid]["case_key"])
        if unit is None:
            raise Refused(f"case {cid}: its unit is not in the corpus")
        if (unit.stratum, unit.kind) != (key[cid]["stratum"], key[cid]["kind"]):
            raise Refused(f"case {cid}: the unit no longer classifies as drawn (has the sampler changed?)")
        try:
            built.append(packet.build_packet(args.corpus, unit, cid))
        except packet.TokenFound:
            raise Refused(f"case {cid}: a token-shaped string is in its packet; render aborted, nothing written") \
                from None
        except Exception as e:  # e.g. packet.py's ValueError embeds the unit's case_key (a session id)
            raise Refused(f"case {cid}: the packet could not be built ({type(e).__name__}); render aborted, "
                          f"nothing written") from None
    packets = set_dir / "packets"
    packets.mkdir(mode=0o700, exist_ok=True)
    for p in built:
        _write_atomic(packets / f"{p.case_id}.md", p.text.encode("utf-8"))
    sha = {p.case_id: p.sha256 for p in built}
    for c in draw["cases"]:
        c["sha256"] = sha[c["case_id"]]
    _write_atomic(set_dir / "draw.json", json.dumps(draw, indent=1) + "\n")
    print(f"rendered {len(built)} packets")
    return 0


def _annotate(figs, corpus_id, note):
    """Attach the corpus id and the population to every figure block of one branch."""
    for stratum in STRATA:
        blk = figs[stratum]
        if blk:
            blk["corpus_id"] = corpus_id
            blk["population"] = f"{stratum} stratum{note}, n={blk['n']}"
    blk = figs["overall"]
    if blk:
        active = [s for s in STRATA if s in blk["weights"]]
        blk["corpus_id"] = corpus_id
        blk["population"] = (f"overall, frame-weighted over {'+'.join(active)} strata{note}, "
                             f"n={sum(figs[s]['n'] for s in active)}")
    figs["median_seconds"] = {
        "value": figs["median_seconds"], "corpus_id": corpus_id,
        "population": (f"median labelling time in seconds over all labelled cases of both strata{note}, "
                       f"n={sum(figs[s]['n'] for s in STRATA if figs[s])}")}


def _check_code_hashes(frame):
    """The frame file must have been frozen by the code that is about to run: all of CODE_FILES, by sha256."""
    recorded = frame.get("code_sha256") if isinstance(frame, dict) else None
    if not isinstance(recorded, dict):
        recorded = {}
    changed = [name for name, digest in _code_hashes().items() if recorded.get(name) != digest]
    if changed:
        raise Refused(f"the code changed since the frame was frozen ({', '.join(changed)}); re-run frame, or "
                      f"estimate with the code that was registered")


def _cmd_estimate(args):
    if not _valid_seed(args.seed):
        raise Refused("--seed must be an integer from 0 to 2**32 - 1")
    set_dir, draw, key = _load_set(args.set)
    frame = _read_json(args.frame)
    sha = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    if not all(sha.values()):
        raise Refused(f"{set_dir} is not rendered (draw.json holds no packet hashes); run render first")
    labels = label._read_jsonl(set_dir / "labels.jsonl")
    relabels = label._read_jsonl(set_dir / "relabels.jsonl")
    rows = labels + relabels
    stray = sum(1 for r in rows if not (isinstance(r.get("case_id"), str) and r["case_id"] in sha))
    if stray:
        raise Refused(f"{stray} label record(s) name a case_id that draw.json does not hold; refusing to estimate")
    bad = sum(1 for r in rows if r.get("packet_sha256") != sha[r["case_id"]])  # sha is non-null: render was checked
    if bad:
        raise Refused(f"{bad} label record(s) do not match draw.json's packet hashes; refusing to estimate")
    drawn = {s: sum(1 for c in draw["cases"] if key[c["case_id"]]["stratum"] == s) for s in STRATA}
    labelled = {s: sum(1 for r in labels if key[r["case_id"]]["stratum"] == s) for s in STRATA}
    if labelled["substantive"] > drawn["substantive"]:  # the invariant the `==` below relies on, stated
        raise Refused(f"{labelled['substantive']} substantive label records for {drawn['substantive']} substantive "
                      f"cases drawn (a case labelled twice?); refusing to estimate")
    _check_population(labels, relabels, "refusing to estimate")
    # The bootstrap streams from --seed, so a seed other than the registered one is a different draw of the
    # bootstrap and could move a boundary case: bind it to the seed the set was drawn with (public, an aggregate).
    if not _valid_seed(draw["seed"]):
        raise Refused("draw.json: field seed is invalid; refusing to estimate")
    if args.seed != draw["seed"]:
        raise Refused(f"--seed {args.seed} is not the seed this set was drawn with ({draw['seed']}); "
                      f"estimate must use the registered seed")
    _check_code_hashes(frame)
    try:
        est = estimator.estimate(labels, key, frame["counts"], args.seed, relabels or None, b=BOOTSTRAP_SAMPLES)
    except ValueError as e:
        # Unreachable by construction (labels, key.json strata/kinds and case ids are all validated above), so
        # a fixed message naming only the type: no estimator message may ever carry a value out.
        raise Refused(f"the estimator refused the labels ({type(e).__name__}); refusing to estimate") from None
    complete = labelled["substantive"] == drawn["substantive"]
    # `kept` mirrors estimate.estimate()'s recall filter (recall != "y"): the second branch's labelled units.
    kept = [r for r in labels if r["recall"] != "y"]
    for branch, note, recs in (("all_cases", "", labels),
                               ("without_recall_flagged", ", recall-flagged cases excluded", kept)):
        _annotate(est[branch], frame["corpus_id"], note)
        for stratum in STRATA:
            blk = est[branch][stratum]
            if blk:  # an aggregate: how many distinct sessions the labelled units of this stratum come from
                blk["n_sessions"] = len({key[r["case_id"]]["copy_id"] for r in recs
                                         if key[r["case_id"]]["stratum"] == stratum})
        sub = est[branch]["substantive"]
        if sub and not complete:
            del sub["decision"]
            sub["decision_withheld"] = "PARTIAL"
    if est["self_agreement"]:
        est["self_agreement"]["corpus_id"] = frame["corpus_id"]
        est["self_agreement"]["population"] = f"relabelled cases of the main set, n={est['self_agreement']['n']}"
    result = {"status": "COMPLETE" if complete else "PARTIAL", "corpus_id": frame["corpus_id"],
              "seed": args.seed, "bootstrap_samples": BOOTSTRAP_SAMPLES, "drawn": drawn, "labelled": labelled,
              "results": est}
    text = json.dumps(result, indent=1, sort_keys=True) + "\n"
    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    _write_new(out, text, 0o644)
    if not complete:
        print(f"PARTIAL: {labelled['substantive']} of {drawn['substantive']} substantive cases labelled; "
              f"no decision", file=sys.stderr)
    sys.stdout.write(text)
    return 0


def _check_record(fname, i, r, sha):
    """One label/relabel record as it may be committed: the whitelisted fields, each validated by VALUE.
    Refusals name the file, the case_id (already proven to be a case of this draw) and the FIELD, never a
    value: a bad value is exactly what may be a session id."""
    if not isinstance(r, dict):
        raise Refused(f"{fname}: record {i} is not an object; nothing exported")
    cid = r.get("case_id")
    # Deliberate redundancy on the only seam into committed data: the 10-hex shape below is implied by
    # `cid in sha` (_load_set shape-checks every draw.json id), so it is mutation-inert by construction.
    if not (isinstance(cid, str) and _CASE_ID.fullmatch(cid) and cid in sha):
        raise Refused(f"{fname}: record {i}: field case_id is not a case of this draw; nothing exported")

    def bad(field):
        raise Refused(f"{fname}: case {cid}: field {field} is invalid; nothing exported")

    if r.get("packet_sha256") != sha[cid]:
        bad("packet_sha256")
    if r.get("delivery") not in label.DELIVERIES:
        bad("delivery")
    labels = r.get("labels")
    # Deliberate redundancy on the same seam: label.validate below also rejects any label outside
    # LABEL_KEYS, so the subset test is mutation-inert by construction (the isinstance(list) part is not).
    if not isinstance(labels, list) or any(l not in estimator.LABEL_KEYS for l in labels):
        bad("labels")
    try:
        label.validate(labels, r["delivery"])
    except ValueError:
        bad("labels")
    if r.get("recall") not in ("y", "n"):
        bad("recall")
    if not _valid_seconds(r.get("seconds")):
        bad("seconds")
    return {k: r[k] for k in LABEL_EXPORT_FIELDS}


def _existing_records(path):
    try:
        return [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]
    except ValueError:
        return None


def _cmd_export(args):
    out_dir = pathlib.Path(args.out_dir).resolve()
    try:
        out_dir.relative_to(archive.REPO_ROOT.resolve())
    except ValueError:
        raise Refused(f"refusing {out_dir}: it is outside the repository; an export is committed data and "
                      f"must be inside it; nothing exported") from None
    try:
        set_dir, draw, key = _load_set(args.set)
    except Refused as e:
        raise Refused(f"{e}; nothing exported") from None
    set_id = draw["set_id"]
    if not (set_id == set_dir.name and _valid_set_name(set_id)):  # equality also implies set_id is a str
        raise Refused("draw.json's set_id must equal the set directory's name, which must look like "
                      "2026-09-29-labelled-main and hold no session-id-shaped run; nothing exported")
    sha = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    if not all(sha.values()):
        raise Refused(f"{set_dir} is not rendered (draw.json holds no packet hashes); run render first; "
                      f"nothing exported")
    # The draw manifest's own values are copied into committed data, so they are validated like the records.
    # (stratum and kind come from key.json and were validated by _load_set, the one place that does it.)
    if not _valid_seed(draw["seed"]):
        raise Refused("draw.json: field seed is invalid; nothing exported")
    for cid, digest in sha.items():
        if not (isinstance(digest, str) and _HEX64.fullmatch(digest)):
            raise Refused(f"draw.json: case {cid}: field sha256 is invalid; nothing exported")
    exported = {}
    for name in ("labels", "relabels"):
        fname = f"{set_id}-{name}.jsonl"
        recs = label._read_jsonl(set_dir / f"{name}.jsonl")
        exported[fname] = [_check_record(fname, i, r, sha) for i, r in enumerate(recs)]
    _check_population(exported[f"{set_id}-labels.jsonl"], exported[f"{set_id}-relabels.jsonl"], "nothing exported")
    draw_name = f"{set_id}-draw.json"
    plan = {draw_name: json.dumps(
        {"set_id": set_id, "seed": draw["seed"],
         "cases": [{"case_id": c["case_id"], "sha256": c["sha256"], "stratum": key[c["case_id"]]["stratum"],
                    "kind": key[c["case_id"]]["kind"]} for c in draw["cases"]]}, indent=1) + "\n"}
    for fname, recs in exported.items():
        if recs or fname.endswith("-labels.jsonl") or (out_dir / fname).exists():
            plan[fname] = "".join(json.dumps(r) + "\n" for r in recs)
    for fname, text in plan.items():
        path = out_dir / fname
        if not path.exists():
            continue
        if fname == draw_name:
            if path.read_bytes() != text.encode("utf-8"):
                raise Refused(f"{path} differs from the new draw manifest; the draw manifest is immutable once "
                              f"committed; nothing exported")
        else:
            old = _existing_records(path)
            if old is None or [json.loads(l) for l in text.splitlines()][:len(old)] != old:
                raise Refused(f"{path} is not a prefix of the new export; an exported label file only grows; "
                              f"nothing exported")
    out_dir.mkdir(parents=True, exist_ok=True)
    for fname, text in plan.items():
        _write_atomic(out_dir / fname, text.encode("utf-8"), 0o644)
    n_l, n_r = (len(exported[f"{set_id}-{n}.jsonl"]) for n in ("labels", "relabels"))
    print(f"exported {len(draw['cases'])} cases, {n_l} labels, {n_r} relabels")
    return 0


_SAMPLE_COMMANDS = {"frame": _cmd_frame, "preflight": _cmd_preflight, "draw": _cmd_draw, "render": _cmd_render,
                    "estimate": _cmd_estimate, "export": _cmd_export}



def main(argv=None, complete=None):
    """`complete(prompt, log_path) -> str` is injectable (tests); None means the Codex channel,
    which a dry run never constructs."""
    args = _parser().parse_args(argv)
    if args.cmd == "gate":
        return _gate(args, complete)
    if args.cmd in _SAMPLE_COMMANDS:
        try:
            return _SAMPLE_COMMANDS[args.cmd](args)
        except (Refused, FileExistsError) as e:
            print(f"error: {e}", file=sys.stderr)
            return 1
    return 2


if __name__ == "__main__":
    sys.exit(main())
