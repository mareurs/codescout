"""Generate synthetic contrastive pairs from the seed manifest, with the registered construction checks.

    python3 generate_synthetic.py --set pilot|train|tsyn-in|tsyn-cross --out DIR [--rules r1,r2] [--workers N]

Registered in the synthetic amendment and its corrections:
  - pilot, train, tsyn-in: Claude Sonnet 5 via `claude -p` on the clean judge channel
    (JUDGE_CONFIG_DIR; refused if `dirty_reasons` finds plugins, hooks or a CLAUDE.md);
  - tsyn-cross: Codex gpt-6-astra/medium via `codex exec`, a fresh CODEX_HOME, run outside the
    repository in a directory holding only the prompt and the call's seeds;
  - 5 seeds per call; a call that errors or returns no parsable JSON is retried once with the
    same seeds, then its seeds are construction failures; no replacement seed is drawn;
  - construction checks (check_pair) decide `ok`; a failing pair is kept with its reasons.

Output in DIR: raw/<call>.txt (each raw reply), pairs.jsonl (one row per seed), summary.txt.
"""
import argparse, collections, concurrent.futures as cf, importlib.util, json, os, pathlib, re, shutil, subprocess, sys, tempfile

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]


def _load(name: str, path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
    return mod


mp = _load("mine_pairs", HERE / "mine_pairs.py")
seg = _load("segment", HERE / "segment.py")
sc = _load("score_dp1", ROOT / "scripts/phase2-score-dp1.py")

CLAUDE_MODEL = "claude-sonnet-5"
CODEX_MODEL, CODEX_EFFORT = "gpt-6-astra", "medium"
PER_CALL = 5
BANNED = re.compile(r"\b(violation|violates|rule|incorrect|correction|corrected|wrong|fix)\b", re.I)
FIELDS = ("seed_id", "rule", "paragraph", "violating_sentence", "fixed_sentence", "why")
SET_USE = {"pilot": "pilot", "train": "train", "tsyn-in": "tsyn", "tsyn-cross": "tsyn"}


def check_pair(pair: dict, rule: str, seed_text: str, banned_sh: set) -> list[str]:
    """Every registered construction check that `pair` fails; [] means it passes.

    `banned_sh` is the union of the held-out and T-row shingles; the seed's own shingles are
    added here. Freeze-time filters (across folds, against T-syn) are not run here."""
    if not all(isinstance(pair.get(f), str if f != "seed_id" else int) for f in FIELDS):
        return ["missing or mistyped field"]
    fails = []
    para, v, f = pair["paragraph"], pair["violating_sentence"], pair["fixed_sentence"]
    if pair["rule"] != rule:
        fails.append("rule mismatch")
    if para.count(v) != 1:
        fails.append("violating sentence not exactly once in paragraph")
    if seg.norm(v) == seg.norm(f):
        fails.append("fixed sentence equals violating sentence")
    if not 60 <= len(para.split()) <= 200:
        fails.append("paragraph length outside 60-200 words")
    if any(BANNED.search(t) for t in (para, v, f)):
        fails.append("banned word")
    substituted = para.replace(v, f, 1)
    if not seg.is_one_unit(v, para):
        fails.append("violating sentence is not one segmenter unit")
    if not seg.is_one_unit(f, substituted):
        fails.append("fixed sentence is not one segmenter unit")
    sh = mp.shingles(para) | mp.shingles(substituted)
    if sh & mp.shingles(seed_text):
        fails.append("shares an 8-token shingle with its seed")
    if sh & banned_sh:
        fails.append("shares an 8-token shingle with a held-out text or T row")
    return fails


def prompt_for(rule: str, seeds: list[dict]) -> str:
    body = (HERE / "synthetic-generation-prompt.md").read_text()
    return (f"{body}\n\n## The rule: `{rule}`\n\nLAW: {mp.sel.RULES[rule]}\n\nUSUAL SHAPE: {mp.sel.SPECS[rule]}\n\n"
            "## Seeds\n\n" + "\n".join(json.dumps({"seed_id": s["id"], "text": s["text"]}, ensure_ascii=False)
                                       for s in seeds))


def parse(raw: str) -> list[dict]:
    out = []
    for ln in raw.splitlines():
        ln = ln.strip().strip("`")
        if ln.startswith("{"):
            try:
                out.append(json.loads(ln))
            except json.JSONDecodeError:
                pass
    return out


class ClaudeGen(sc.SubscriptionJudge):
    SYSTEM = "You write data exactly as instructed. Output only what the instructions ask for."


def codex_complete(prompt: str, home: pathlib.Path, log_path: pathlib.Path | None = None) -> str:
    """One `codex exec` call in a throwaway directory. `log_path`, when given, receives the
    call's full stdout+stderr: the only record of whether Codex ran shell commands, since the
    directory is deleted and `--ephemeral` keeps no session. The T-syn-cross run of
    2026-09-25 predates this parameter and has no such record."""
    work = pathlib.Path(tempfile.mkdtemp(prefix="syn-cross-"))
    try:
        (work / "task.md").write_text(prompt)
        env = {k: v for k, v in os.environ.items() if k not in ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL")}
        env["CODEX_HOME"] = str(home)
        p = subprocess.run(
            ["codex", "exec", "--skip-git-repo-check", "--sandbox", "read-only", "-c", "approval_policy=never",
             "-C", str(work), "--ephemeral", "-o", str(work / "last.txt"),
             "Read task.md in this directory and do exactly what it asks. Reply with only the JSON lines it "
             "specifies, one per seed or item it lists, and nothing else. Use no other file."],
            capture_output=True, text=True, timeout=1200, stdin=subprocess.DEVNULL, env=env)
        if log_path is not None:
            log_path.write_text(p.stdout + "\n--- stderr ---\n" + p.stderr)
        if p.returncode != 0:
            raise RuntimeError(f"codex exec exit {p.returncode}: {p.stderr.strip()[-200:]}")
        return (work / "last.txt").read_text()
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--set", required=True, choices=SET_USE)
    ap.add_argument("--out", required=True)
    ap.add_argument("--rules")
    ap.add_argument("--workers", type=int, default=4)
    a = ap.parse_args()
    out = pathlib.Path(a.out); (out / "raw").mkdir(parents=True, exist_ok=True)

    cross = a.set == "tsyn-cross"
    if cross:
        home = pathlib.Path(tempfile.mkdtemp(prefix="codex-syn-home-"))
        (home / "auth.json").symlink_to(pathlib.Path.home() / ".codex/auth.json")
        (home / "config.toml").write_text(f'model = "{CODEX_MODEL}"\nmodel_reasoning_effort = "{CODEX_EFFORT}"\n')
        complete, generator = (lambda pr, cid=None: codex_complete(pr, home, out / "raw" / f"{cid}.codex.log" if cid else None)), f"codex:{CODEX_MODEL}/{CODEX_EFFORT}"
    else:
        cfg = os.environ.get("JUDGE_CONFIG_DIR", "")
        if not cfg or (dirty := sc.dirty_reasons(cfg)):
            sys.exit(f"refused: JUDGE_CONFIG_DIR is not the clean channel ({cfg or 'unset'}: {dirty if cfg else ''})")
        gen = ClaudeGen(CLAUDE_MODEL, cfg, timeout=900)
        complete, generator = (lambda pr, cid=None: gen.complete(pr)[0]), f"claude:{CLAUDE_MODEL}"

    manifest = [json.loads(l) for l in (HERE / "seed-manifest.jsonl").read_text().splitlines()]
    want = SET_USE[a.set]
    rules = sorted({r["use"].split(":")[1] for r in manifest if r["use"] and r["use"].startswith(want + ":")})
    if a.rules:
        rules = [r for r in rules if r in a.rules.split(",")]
    split = [json.loads(l) for l in (HERE / "t-split.jsonl").read_text().splitlines()]
    rows = [json.loads(l) for l in (HERE / "mined-candidates.jsonl").read_text().splitlines()]
    banned_sh = set().union(*mp.held_out().values())
    for s in split:
        if s["split"] == "T":
            for fld in ("positive", "twin", "context_before", "context_after"):
                banned_sh |= mp.shingles(rows[s["id"]].get(fld) or "")

    calls = []
    for rule in rules:
        seeds = [r for r in manifest if r["use"] == f"{want}:{rule}"]
        for k in range(0, len(seeds), PER_CALL):
            calls.append((f"{rule}-{k // PER_CALL:02d}", rule, seeds[k:k + PER_CALL]))

    def run(call):
        cid, rule, seeds = call
        pr, got, err = prompt_for(rule, seeds), [], None
        for attempt in (1, 2):
            try:
                raw = complete(pr, f"{cid}.a{attempt}")
            except Exception as e:                      # noqa: BLE001 -- recorded, then retried once
                err = f"attempt {attempt}: {e}"; continue
            (out / "raw" / f"{cid}.a{attempt}.txt").write_text(raw)
            got = parse(raw)
            if got:
                break
            err = f"attempt {attempt}: no parsable JSON line"
        by_seed = {g.get("seed_id"): g for g in got if isinstance(g, dict)}
        res = []
        for s in seeds:
            base = {"pair_id": f"{a.set}-{rule}-{s['id']}", "set": a.set, "rule": rule, "seed_id": s["id"],
                    "seed_group": s["group"], "side": s["side"], "fold": s["fold"], "source": "synthetic",
                    "generator": generator, "claude_generated": not cross}
            g = by_seed.get(s["id"])
            if g is None:
                res.append({**base, "ok": False, "fails": ["call failed" if not got else "seed missing from output"],
                            "error": err})
                continue
            fails = check_pair(g, rule, s["text"], banned_sh)
            res.append({**base, **{k: g.get(k) for k in FIELDS if k not in ("seed_id", "rule")},
                        "ok": not fails, "fails": fails})
        return res

    results = []
    with cf.ThreadPoolExecutor(a.workers) as ex:
        for res in ex.map(run, calls):
            results.extend(res)
    with open(out / "pairs.jsonl", "w") as fh:
        for r in results:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    per_rule = collections.defaultdict(lambda: [0, 0])
    fails = collections.Counter()
    for r in results:
        per_rule[r["rule"]][0] += r["ok"]; per_rule[r["rule"]][1] += 1
        fails.update(r["fails"])
    lines = [f"set {a.set}  generator {generator}  calls {len(calls)}  pairs {len(results)}  "
             f"ok {sum(r['ok'] for r in results)}",
             "failures: " + json.dumps(dict(fails)),
             *(f"  {rule:22} {ok}/{n}" for rule, (ok, n) in sorted(per_rule.items()))]
    (out / "summary.txt").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
