"""Thin CLI for the System-1 base-rate measurement pipeline (R126).

Task 9 adds the `gate` subcommand; later tasks add theirs.

    run.py gate --dry --out <file>     build all 81 gate inputs, render every prompt, report
                                       prompt sizes; makes NO model call (R121, Task 9a)
    run.py gate --out <file> --log-dir <dir>
                                       the real gate (Task 9b, only after the prompt's sha256 is
                                       registered in the spec's Amendments); exit 1 when it fails.
                                       It refuses to start with --votes other than 3, with
                                       --any-population, with a --log-dir inside the repository,
                                       or on any codex other than codex-cli 0.154.0 (R133, R138)

Run from the repo root with ~/work/claude/prompt-engineering/.venv/bin/python.
"""
import argparse
import contextlib
import json
import pathlib
import sys

_HERE = pathlib.Path(__file__).resolve().parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))
import judge  # noqa: E402  (sibling module, per miner.py's idiom)


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


def main(argv=None, complete=None):
    """`complete(prompt, log_path) -> str` is injectable (tests); None means the Codex channel,
    which a dry run never constructs."""
    args = _parser().parse_args(argv)
    if args.cmd == "gate":
        return _gate(args, complete)
    return 2


if __name__ == "__main__":
    sys.exit(main())
