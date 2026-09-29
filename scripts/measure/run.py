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

    run.py frame --corpus C --out FRAME.json
                                       counts + two code hashes, no text; may be written anywhere
    run.py draw --corpus C --set DIR --seed S --substantive N --routine M [--exclude-set DIR2]
                                       stratified draw into a NEW private (mode 700) set directory
                                       OUTSIDE the repo: key.json (private, holds session ids) and
                                       draw.json; prints counts only
    run.py render --corpus C --set DIR builds every blinded packet into DIR/packets and fills
                                       draw.json's sha256s; a token-shaped string aborts it with
                                       nothing written; prints counts only
    run.py estimate --set DIR --frame FRAME.json --seed S --out RESULT.json
                                       aggregates only (no case id, note or per-case label);
                                       PARTIAL, with no decision, until every drawn substantive
                                       case is labelled
    run.py export --set DIR --out-dir D
                                       the committable record: case ids, packet hashes, strata,
                                       kinds and labels WITHOUT notes; D must be INSIDE the repo
                                       (the opposite of the private writers above)

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
STRATA = ("substantive", "routine")
LABEL_EXPORT_FIELDS = ("case_id", "packet_sha256", "labels", "delivery", "recall", "seconds")
_SAFE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
_CASE_ID = re.compile(r"[0-9a-f]{10}")


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
    dr = sub.add_parser("draw", help="draw a stratified sample into a new private set directory")
    dr.add_argument("--corpus", required=True)
    dr.add_argument("--set", required=True, help="new directory, outside the repo")
    dr.add_argument("--seed", type=int, required=True)
    dr.add_argument("--substantive", type=int, required=True)
    dr.add_argument("--routine", type=int, required=True)
    dr.add_argument("--exclude-set", help="another set directory whose units are removed from the pool")
    rd = sub.add_parser("render", help="build every blinded packet of a set")
    rd.add_argument("--corpus", required=True)
    rd.add_argument("--set", required=True)
    es = sub.add_parser("estimate", help="aggregates only; PARTIAL until every substantive case is labelled")
    es.add_argument("--set", required=True)
    es.add_argument("--frame", required=True)
    es.add_argument("--seed", type=int, required=True)
    es.add_argument("--out", required=True)
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
    return set_dir, draw, key


def _cmd_frame(args):
    corpus = pathlib.Path(args.corpus)
    units = sampler.frame(corpus)
    if not units:
        raise Refused(f"{corpus} has no units; refusing to freeze an empty frame")
    counts = sampler.frame_counts(units)
    rec = {"corpus_id": corpus.resolve().name, "counts": counts, "n_units": len(units),
           "code_sha256": {name: hashlib.sha256((_HERE / name).read_bytes()).hexdigest()
                           for name in ("sampler.py", "packet.py")}}
    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    _write_new(out, json.dumps(rec, indent=1) + "\n", 0o644)
    print(json.dumps(counts, sort_keys=True))
    return 0


def _excluded_case_keys(exclude_set):
    path = pathlib.Path(exclude_set) / "key.json"
    if not path.is_file():
        raise Refused(f"--exclude-set {exclude_set} holds no key.json; nothing to exclude")
    return {v["case_key"] for v in _read_json(path).values()}


def _cmd_draw(args):
    set_dir = pathlib.Path(args.set)
    _private_dir(set_dir)
    if not _SAFE_NAME.fullmatch(set_dir.name):
        raise Refused(f"set name {set_dir.name!r} is not a plain file name (letters, digits, . _ -)")
    exclude = _excluded_case_keys(args.exclude_set) if args.exclude_set else set()
    units = sampler.frame(args.corpus)
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
    print(f"drawn substantive={n_sub} routine={len(key) - n_sub} excluded={len(exclude)}")
    return 0


def _cmd_render(args):
    set_dir, draw, key = _load_set(args.set)
    if any(c["sha256"] for c in draw["cases"]):
        raise Refused(f"{set_dir} is already rendered; labels bind to its packet hashes, so it is not re-rendered")
    by_key = {u.case_key: u for u in sampler.frame(args.corpus)}
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


def _cmd_estimate(args):
    set_dir, draw, key = _load_set(args.set)
    frame = _read_json(args.frame)
    sha = {c["case_id"]: c["sha256"] for c in draw["cases"]}
    if not all(sha.values()):
        raise Refused(f"{set_dir} is not rendered (draw.json holds no packet hashes); run render first")
    labels = label._read_jsonl(set_dir / "labels.jsonl")
    relabels = label._read_jsonl(set_dir / "relabels.jsonl")
    bad = sum(1 for r in labels + relabels if sha.get(r["case_id"]) != r["packet_sha256"])
    if bad:
        raise Refused(f"{bad} label record(s) do not match draw.json's packet hashes; refusing to estimate")
    drawn = {s: sum(1 for c in draw["cases"] if key[c["case_id"]]["stratum"] == s) for s in STRATA}
    labelled = {s: sum(1 for r in labels if key[r["case_id"]]["stratum"] == s) for s in STRATA}
    try:
        est = estimator.estimate(labels, key, frame["counts"], args.seed, relabels or None, b=BOOTSTRAP_SAMPLES)
    except ValueError as e:  # a duplicate label or relabel: refuse, do not crash
        raise Refused(str(e)) from None
    complete = labelled["substantive"] == drawn["substantive"]
    for branch, note in (("all_cases", ""), ("without_recall_flagged", ", recall-flagged cases excluded")):
        _annotate(est[branch], frame["corpus_id"], note)
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


def _cmd_export(args):
    out_dir = pathlib.Path(args.out_dir).resolve()
    try:
        out_dir.relative_to(archive.REPO_ROOT.resolve())
    except ValueError:
        raise Refused(f"refusing {out_dir}: it is outside the repository; an export is committed data and "
                      f"must be inside it") from None
    set_dir, draw, key = _load_set(args.set)
    set_id = draw["set_id"]
    if not _SAFE_NAME.fullmatch(set_id):
        raise Refused("draw.json's set_id is not a plain file name")
    labels = label._read_jsonl(set_dir / "labels.jsonl")
    relabels = label._read_jsonl(set_dir / "relabels.jsonl")
    files = {f"{set_id}-draw.json": json.dumps(
        {"set_id": set_id, "seed": draw["seed"],
         "cases": [{"case_id": c["case_id"], "sha256": c["sha256"], "stratum": key[c["case_id"]]["stratum"],
                    "kind": key[c["case_id"]]["kind"]} for c in draw["cases"]]}, indent=1) + "\n"}
    for name, recs in (("labels", labels), ("relabels", relabels)):
        if recs or name == "labels":
            files[f"{set_id}-{name}.jsonl"] = "".join(
                json.dumps({k: r[k] for k in LABEL_EXPORT_FIELDS}) + "\n" for r in recs)
    for name in files:
        if (out_dir / name).exists():
            raise Refused(f"{out_dir / name} exists; an export is not overwritten")
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        _write_new(out_dir / name, text, 0o644)
    print(f"exported {len(draw['cases'])} cases, {len(labels)} labels, {len(relabels)} relabels")
    return 0


_SAMPLE_COMMANDS = {"frame": _cmd_frame, "draw": _cmd_draw, "render": _cmd_render, "estimate": _cmd_estimate,
                    "export": _cmd_export}



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
