"""The intervention judge: reads the operator's packet and answers with the operator's labels.

Model-free until `judge` is called with a real channel. It sits beside `judge.py` and loads it, and `label.py`,
as siblings, reusing their helpers; it edits neither. Spec:
docs/superpowers/specs/2026-10-24-intervention-judge-design.md. Plan:
docs/superpowers/plans/2026-10-24-intervention-judge.md.
"""
import collections
import concurrent.futures as cf
import dataclasses
import hashlib
import importlib.util
import pathlib
import re

_HERE = pathlib.Path(__file__).resolve().parent


def _load_sibling(name):
    spec = importlib.util.spec_from_file_location(f"measure_{name}", _HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# `_judge` is exposed on purpose: a caller that needs `ToolCallError` must take the class from THIS copy of
# judge.py, because a second copy loaded elsewhere in the process is a different class.
_judge = _load_sibling("judge")
_label = _load_sibling("label")

SILENT_LABELS = ("none", "unresolved")


@dataclasses.dataclass
class Verdict:
    labels: list | None = None
    delivery: str | None = None
    action: str = ""
    quote: str = ""
    flags: list = dataclasses.field(default_factory=list)


def _valid_labels(x):
    if not isinstance(x, list) or not x or not all(isinstance(i, str) for i in x):
        return False
    if len(set(x)) != len(x):
        return False
    return x in (["none"], ["unresolved"]) or all(i in _label.LABELS for i in x)


def parse_vote(raw, pre_evidence):
    """One reply to a Verdict. A missing field is flagged `missing:<field>` and a bad value
    `invalid:<field>`; neither is ever defaulted. `invalid:pairing` is the operator's label/delivery
    rule (`label.validate`); both values are kept. A non-silent vote whose quote is not a verbatim
    span of `pre_evidence` is flagged `quote_not_found`. Nothing here retries or drops a vote."""
    obj = _judge._extract_json(raw)
    if obj is None:
        return Verdict(flags=["unparseable"])
    flags = []
    v = Verdict(flags=flags)

    if "labels" not in obj:
        flags.append("missing:labels")
    elif _valid_labels(obj["labels"]):
        v.labels = list(obj["labels"])
    else:
        flags.append("invalid:labels")

    if "delivery" not in obj:
        flags.append("missing:delivery")
    elif isinstance(obj["delivery"], str) and obj["delivery"] in _label.DELIVERIES:
        v.delivery = obj["delivery"]
    else:
        flags.append("invalid:delivery")

    for f in ("action", "quote"):
        if f not in obj:
            flags.append(f"missing:{f}")
        elif isinstance(obj[f], str):
            setattr(v, f, obj[f])
        else:
            flags.append(f"invalid:{f}")

    if v.labels is not None and v.delivery is not None:
        try:
            _label.validate(v.labels, v.delivery)
        except ValueError:
            flags.append("invalid:pairing")

    if v.labels is not None and v.labels not in (["none"], ["unresolved"]):
        if _judge._verified_span(v.quote, pre_evidence) is None:
            flags.append("quote_not_found")
    return v


PROMPT_PATH = _HERE / "intervention_prompt.md"


def bars_text():
    """The labelling banner's question, bars and delivery rule (steps 2 to 4 of `label._BANNER_BODY`),
    verbatim, so that the judge and the operator's labels read the same instructions."""
    body = _label._BANNER_BODY
    start, end = body.find("2. The question"), body.find("5. Recall")
    if start < 0 or end < 0 or end < start:
        raise ValueError("label._BANNER_BODY no longer has the '2. The question' and '5. Recall' markers")
    return body[start:end].rstrip("\n")


def render_prompt(packet_text, bars=None):
    """The prompt around one packet. Comments are stripped from the template first and the values are
    filled in one pass, so text inside the packet is never re-scanned for placeholders."""
    template = _judge._COMMENT.sub("", PROMPT_PATH.read_text(encoding="utf-8"))
    values = {"BARS": bars_text() if bars is None else bars, "PACKET": _judge._data(packet_text)}

    def fill(m):
        if m.group(1) not in values:
            raise KeyError(f"intervention_prompt.md names an unknown placeholder {m.group(0)}")
        return values[m.group(1)]

    return _judge._PLACEHOLDER.sub(fill, template)


def prompt_sha256():
    return hashlib.sha256(PROMPT_PATH.read_bytes()).hexdigest()


def build_input(packet):
    """One judge input. `pre_evidence` is the packet text unchanged: it is what the quote check reads.
    The case id and the hash stay on the input and never enter the prompt."""
    return {"id": packet.case_id, "packet_sha256": packet.sha256, "pre_evidence": packet.text,
            "prompt": render_prompt(packet.text)}


def _flag_key(flag):
    """A `call_failed:<detail>` or `tool_call:<detail>` flag is keyed by its kind alone, so exception
    text never splits the count; every other flag is its own key."""
    kind = flag.split(":", 1)[0]
    return kind if kind in ("call_failed", "tool_call") else flag


def judge(packet, votes=3, complete=None, log_dir=None):
    """`votes` votes on one packet, at most `MAX_CONCURRENT_CALLS` at once, as `judge.judge` does. An
    attempt that is `call_failed`, `unparseable` or `tool_call` is re-issued, up to
    `MAX_ATTEMPTS_PER_VOTE` attempts; a pairing violation or an unverified quote is flagged, counted and
    NOT re-issued, because the rate at which the model breaks those rules is itself a result.
    `complete(prompt, log_path) -> str` is injectable; the default is the Codex channel. The record has
    no consensus field: the rule for a set-valued answer is the gate spec's to choose."""
    inp = build_input(packet)
    if complete is None:
        complete = _judge.codex_channel().complete
    if log_dir is not None:
        log_dir = pathlib.Path(log_dir)
        log_dir.mkdir(parents=True, exist_ok=True)
    safe = re.sub(r"[^A-Za-z0-9._-]+", "_", inp["id"])

    def attempt(k, a):
        log_path = None if log_dir is None else log_dir / f"{safe}.vote{k + 1}.try{a + 1}.log"
        try:
            raw = complete(inp["prompt"], log_path)
        except _judge.ToolCallError as exc:
            return Verdict(flags=[f"tool_call:{exc}"]), ""
        except Exception as exc:  # noqa: BLE001 -- recorded on the vote, never silently dropped
            return Verdict(flags=[f"call_failed:{type(exc).__name__}: {str(exc)[:200]}"]), ""
        return parse_vote(raw, inp["pre_evidence"]), raw

    def one(k):
        tries = []
        for a in range(_judge.MAX_ATTEMPTS_PER_VOTE):
            v, raw = attempt(k, a)
            tries.append({"attempt": a + 1, "failure": _judge._failure(v), "flags": list(v.flags),
                          "raw": raw})
            if tries[-1]["failure"] is None:
                break
        return v, raw, tries

    with cf.ThreadPoolExecutor(max_workers=max(1, min(_judge.MAX_CONCURRENT_CALLS, votes))) as ex:
        results = list(ex.map(one, range(votes)))

    label_votes = {label: 0 for label in _label.LABELS + SILENT_LABELS}
    delivery_votes = {d: 0 for d in _label.DELIVERIES}
    flag_counts = collections.Counter()
    n_uncounted = 0
    for v, _raw, _tries in results:
        for label in v.labels or ():
            label_votes[label] += 1
        if v.delivery is not None:
            delivery_votes[v.delivery] += 1
        if v.labels is None or v.delivery is None:
            n_uncounted += 1
        flag_counts.update({_flag_key(f) for f in v.flags})
    return {"id": inp["id"], "packet_sha256": inp["packet_sha256"],
            "votes": [dict(dataclasses.asdict(v), raw=raw, attempts=tries) for v, raw, tries in results],
            "label_votes": label_votes, "delivery_votes": delivery_votes,
            "flag_counts": dict(flag_counts), "n_uncounted": n_uncounted}
