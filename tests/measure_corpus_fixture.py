"""Synthetic frozen-corpus builder shared by the labelled-sample tests (sampler, packet, run).

Writes the layout transcripts.sessions() reads, and nothing real:
    <root>/transcripts/<NN-profile>/<slug>/<sid>.jsonl
    <root>/transcripts/<NN-profile>/<slug>/<sid>/subagents/<name>.jsonl
"""
import json
import pathlib


def assistant(uuid, ts, mid, text=None, tool_uses=(), stop=None, model="claude", sidechain=False):
    """One assistant transcript entry. `tool_uses` is a sequence of (name, input_dict).
    A message spread over several entries is built by calling this repeatedly with one `mid`.

    tool_use ids are f"{uuid}-tu{i}" (i = position within THIS entry), so they are unique per entry
    even when sibling entries share one `mid`; a tool_result caller names the id as
    f"<that entry's uuid>-tu<i>"."""
    content = []
    if text is not None:
        content.append({"type": "text", "text": text})
    for i, (name, inp) in enumerate(tool_uses):
        content.append({"type": "tool_use", "id": f"{uuid}-tu{i}", "name": name, "input": inp})
    return {
        "type": "assistant",
        "uuid": uuid,
        "timestamp": ts,
        "isSidechain": sidechain,
        "message": {"id": mid, "model": model, "stop_reason": stop, "content": content},
    }


def user_prompt(uuid, ts, text):
    return {
        "type": "user",
        "uuid": uuid,
        "timestamp": ts,
        "message": {"role": "user", "content": text},
    }


def tool_result(uuid, ts, tool_use_id, content, is_error=False):
    return {
        "type": "user",
        "uuid": uuid,
        "timestamp": ts,
        "message": {
            "role": "user",
            "content": [{"type": "tool_result", "tool_use_id": tool_use_id,
                         "content": content, "is_error": is_error}],
        },
    }


def _write(path, entries, sid, sidechain=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for e in entries:
            e = dict(e)
            if sidechain:
                e["isSidechain"] = True  # forced: a real subagent file is all isSidechain
            e.setdefault("sessionId", sid)
            # entrypoint "cli": transcripts.exclusions() drops "sdk-cli" (headless) copies.
            e.setdefault("entrypoint", "cli")
            f.write(json.dumps(e) + "\n")


def build_corpus(root, sessions):
    """Write `sessions` under `root` and return `root`.

    Each session is {"sid", "profile", "slug", "entries": [...], "subagents": {name: [entries]}}
    (`subagents` optional). Profiles get a two-digit numeric prefix in ALPHABETICAL order,
    like the real corpus (`01-work`), which transcripts._profile_from_dir strips.
    Every subagent entry is written with isSidechain=True (overwritten, not defaulted), as in a
    real subagent file.
    """
    root = pathlib.Path(root)
    profiles = sorted({s["profile"] for s in sessions})
    for s in sessions:
        pdir = f"{profiles.index(s['profile']) + 1:02d}-{s['profile']}"
        base = root / "transcripts" / pdir / s["slug"]
        _write(base / f"{s['sid']}.jsonl", s["entries"], s["sid"])
        for name, entries in (s.get("subagents") or {}).items():
            _write(base / s["sid"] / "subagents" / f"{name}.jsonl", entries, s["sid"], sidechain=True)
    return root
