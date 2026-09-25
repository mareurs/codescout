"""Deterministic seed extractor for the synthetic pairs (synthetic amendment, corrections 4 and 5).

    python3 extract_seeds.py

Writes, next to this file:
  seed-manifest.jsonl       every eligible paragraph: id, path, paragraph index, group, side,
                            fold, word count, sha1 of its text; `use` and `text` on drawn seeds
  fold-assignment.jsonl     every doc group's side and fold (mined and seed-only groups)
  seed-manifest-summary.txt counts at each step

Registered rules, in order:
  - files: `docs/**/*.md` tracked at COMMIT, read from git, not the working tree;
  - whole-file exclusion: path matches the miner's HELD_OUT_DOC_RE or `review-model-vs-context`,
    or content contains any of CONTENT_TOKENS, or basename is one of T's doc groups;
  - paragraphs: frontmatter, fenced blocks and HTML comments removed; blank-line paragraphs;
    heading and table lines removed; >= 60 whitespace words;
  - a paragraph sharing an 8-token shingle with a held-out text (mine_pairs.held_out) or with
    any field of a mined T row is dropped;
  - side: a group with no mined rows goes to S with p = 0.3, Random(20260929), alphabetical;
  - folds: non-T mined components (draw_t's construction) plus training-side seed-only groups
    as singletons; val p = 0.15, cal p = 0.15, else train; Random(20260930), alphabetical by
    each component's first group;
  - draws: one Random(20260931): pilot 110 from training-side ids (5 per rule, rules sorted),
    then 80 per rule from the remaining training-side ids, then 15 per rule from S ids; no
    reuse; a shortfall scales that side's per-rule count down (floor) and is reported.
"""
import collections, hashlib, importlib.util, itertools, json, math, pathlib, random, re, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]
COMMIT = "3cfda138"          # the synthetic amendment's commit
CONTENT_TOKENS = ("rule-tell", "rule_tell", "phase1-local-classifier", "phase1-span-selector")
EXTRA_PATH_RE = re.compile(r"review-model-vs-context")
MIN_WORDS = 60
PILOT_PER_RULE, TRAIN_PER_RULE, TSYN_PER_RULE = 5, 80, 15

spec = importlib.util.spec_from_file_location("mine_pairs", HERE / "mine_pairs.py")
mp = importlib.util.module_from_spec(spec); spec.loader.exec_module(mp)

COMMENT = re.compile(r"<!--.*?-->", re.S)
FENCED = re.compile(r"^\s*(```|~~~).*?^\s*\1[^\n]*$", re.S | re.M)
FRONTMATTER = re.compile(r"\A---\n.*?\n---\n", re.S)


def git(*a: str) -> str:
    return subprocess.run(["git", "-C", str(ROOT), *a], check=True, capture_output=True,
                          text=True).stdout


def paragraphs(text: str) -> list[str]:
    text = FRONTMATTER.sub("", text)
    text = COMMENT.sub("", text)
    text = FENCED.sub("", text)
    out = []
    for block in re.split(r"\n\s*\n", text):
        kept = [l for l in block.splitlines() if l.strip() and not l.lstrip().startswith(("#", "|"))]
        p = "\n".join(kept).strip()
        if len(p.split()) >= MIN_WORDS:
            out.append(p)
    return out


def components(rows: list[dict]) -> list[list[str]]:
    """draw_t.py's construction: doc groups joined by a shared 8-token shingle."""
    by_group = collections.defaultdict(set)
    for r in rows:
        by_group[r["doc_group"]] |= mp.shingles(r["positive"]) | mp.shingles(r["twin"] or "")
    owners = collections.defaultdict(set)
    for g, ss in by_group.items():
        for s in ss:
            owners[s].add(g)
    pairs = {p for gs in owners.values() if len(gs) > 1 for p in itertools.combinations(sorted(gs), 2)}
    assert len(pairs) == 25, len(pairs)
    parent = {g: g for g in by_group}
    def find(g):
        while parent[g] != g:
            parent[g] = parent[parent[g]]; g = parent[g]
        return g
    for a, b in pairs:
        parent[find(a)] = find(b)
    comps = collections.defaultdict(list)
    for g in by_group:
        comps[find(g)].append(g)
    return sorted((sorted(m) for m in comps.values()), key=lambda m: m[0])


def main() -> int:
    rules = sorted(mp.sel.RULES)
    rows = [json.loads(l) for l in (HERE / "mined-candidates.jsonl").read_text().splitlines()]
    split = [json.loads(l) for l in (HERE / "t-split.jsonl").read_text().splitlines()]
    t_groups = {s["doc_group"] for s in split if s["split"] == "T"}
    mined_groups = {r["doc_group"] for r in rows}
    assert len(t_groups) == 75 and len(mined_groups) == 248

    ho = set().union(*mp.held_out().values())
    t_sh = set()
    for s in split:
        if s["split"] == "T":
            r = rows[s["id"]]
            for f in ("positive", "twin", "context_before", "context_after"):
                t_sh |= mp.shingles(r.get(f) or "")

    n = collections.Counter()
    paras = []                                   # (path, index, text)
    for path in sorted(git("ls-tree", "-r", "--name-only", COMMIT, "--", "docs").split("\n")):
        if not path.endswith(".md"):
            continue
        n["md files"] += 1
        base = pathlib.PurePosixPath(path).name
        if mp.HELD_OUT_DOC_RE.search(path) or EXTRA_PATH_RE.search(path):
            n["excluded: campaign path"] += 1; continue
        if base in t_groups:
            n["excluded: T doc group"] += 1; continue
        text = git("show", f"{COMMIT}:{path}")
        if any(tok in text for tok in CONTENT_TOKENS):
            n["excluded: campaign content"] += 1; continue
        for k, p in enumerate(paragraphs(text)):
            n["paragraphs >= 60 words"] += 1
            sh = mp.shingles(p)
            if sh & ho:
                n["dropped: held-out shingle"] += 1; continue
            if sh & t_sh:
                n["dropped: T-row shingle"] += 1; continue
            paras.append((path, k, p))

    groups = sorted({pathlib.PurePosixPath(p).name for p, _, _ in paras} | mined_groups)
    side = {}
    rng = random.Random(20260929)
    for g in groups:
        if g in t_groups:
            side[g] = "T"
        elif g not in mined_groups and rng.random() < 0.3:
            side[g] = "S"
        else:
            side[g] = "training"

    comps = [m for m in components(rows) if m[0] not in t_groups]
    assert all(not (set(m) & t_groups) for m in comps)
    comps += [[g] for g in groups if g not in mined_groups and side[g] == "training"]
    comps.sort(key=lambda m: m[0])
    fold = {g: ("T" if side[g] == "T" else "S" if side[g] == "S" else None) for g in groups}
    rng = random.Random(20260930)
    for m in comps:
        r = rng.random()
        f = "val" if r < 0.15 else "cal" if r < 0.30 else "train"
        for g in m:
            fold[g] = f
    assert all(v is not None for v in fold.values())

    ids = list(range(len(paras)))
    gid = [pathlib.PurePosixPath(p).name for p, _, _ in paras]
    train_ids = [i for i in ids if side[gid[i]] == "training"]
    s_ids = [i for i in ids if side[gid[i]] == "S"]
    use = {}
    rng = random.Random(20260931)

    def draw(pool: list[int], per_rule: int, tag: str) -> tuple[list[int], int]:
        k = min(per_rule, len(pool) // len(rules))
        picked = rng.sample(pool, k * len(rules))
        for j, rule in enumerate(rules):
            for i in picked[j * k:(j + 1) * k]:
                use[i] = f"{tag}:{rule}"
        taken = set(picked)
        return [i for i in pool if i not in taken], k

    train_ids, k_pilot = draw(train_ids, PILOT_PER_RULE, "pilot")
    train_ids, k_train = draw(train_ids, TRAIN_PER_RULE, "train")
    _, k_tsyn = draw(s_ids, TSYN_PER_RULE, "tsyn")

    with open(HERE / "seed-manifest.jsonl", "w") as fh:
        for i, (path, k, p) in enumerate(paras):
            row = {"id": i, "path": path, "para": k, "group": gid[i], "side": side[gid[i]],
                   "fold": fold[gid[i]], "words": len(p.split()),
                   "sha1": hashlib.sha1(p.encode()).hexdigest(), "use": use.get(i)}
            if i in use:
                row["text"] = p
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
    with open(HERE / "fold-assignment.jsonl", "w") as fh:
        for g in groups:
            fh.write(json.dumps({"group": g, "mined": g in mined_groups, "side": side[g], "fold": fold[g]}) + "\n")

    fc = collections.Counter(fold[g] for g in groups)
    summary = [f"commit {COMMIT}", *(f"{k}: {v}" for k, v in n.items()),
               f"eligible paragraphs: {len(paras)} (training side {len(train_ids) + (k_pilot + k_train) * len(rules)}, S {len(s_ids)})",
               f"groups: {len(groups)} ({dict(collections.Counter(side.values()))}); folds {dict(fc)}",
               f"per rule drawn: pilot {k_pilot} (of {PILOT_PER_RULE}), train {k_train} (of {TRAIN_PER_RULE}), T-syn {k_tsyn} (of {TSYN_PER_RULE})",
               f"seeds drawn: {len(use)}"]
    (HERE / "seed-manifest-summary.txt").write_text("\n".join(summary) + "\n")
    print("\n".join(summary))
    return 0


if __name__ == "__main__":
    sys.exit(main())
