"""Strict, content-free operation observation v1 contract."""
import hashlib
import json
import re

EVENTS = ["request-observed","dispatch-observed","result-observed","hook-pre","hook-post","lifecycle","observer-diagnostic"]
OUTCOMES = ["requested","dispatched","returned-success","refused","error-possible-effects","missing-terminal","unknown"]
DIAGNOSTICS = ["missing-root","invalid-context","context-conflict","unsupported-tool","unsupported-event","incomplete-input","opaque-effects","missing-terminal","unavailable","disabled","quota","torn-record","checksum","sequence-gap","conflicting-duplicate","ambiguous-pair","incompatible-schema","checkout-mismatch","reset","unsupported-fingerprint"]
TOOLS = ["create_file","edit_file","edit_code","run_command","read_file","symbols","grep","semantic_search","workspace","doc","memory","find_file","references","call_graph","symbol_at","get_guide","onboarding","index","execute","exec_command","shell","code_mode"]
FIELDS = ["schema_version","observed_at_unix_ms","observation_id","producer_instance","sequence","store_epoch","event","producer_kind","workspace","caller","tool","local_invocation","native_call_id","turn_id","fingerprints","targets","outcome","diagnostics"]

def _fail():
    raise ValueError("invalid observation")

def _scalar(s):
    if not isinstance(s, str) or any(0xD800 <= ord(c) <= 0xDFFF for c in s):
        _fail()
    return s

def _text(s, maximum=1024):
    _scalar(s)
    if not s.strip() or len(s.encode()) > maximum or any(ord(c) < 32 for c in s):
        _fail()

def _exact(v, keys):
    if not isinstance(v, dict) or set(v) != set(keys):
        _fail()

def _optional(v, check=_text):
    if v is not None:
        check(v)

def _safe(n):
    return type(n) is int and 0 <= n <= 9007199254740991

def _path(v):
    _text(v, 4096)
    if not v.startswith("/"):
        _fail()

def _uuid(v):
    _text(v)
    if not re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}",v):
        _fail()

def canonical_json(value, depth=0):
    if depth > 32:
        _fail()
    if value is None:
        return "null"
    if type(value) is bool:
        return "true" if value else "false"
    if type(value) is int and abs(value) <= 9007199254740991:
        return str(value)
    if isinstance(value,str):
        _scalar(value)
        return '"' + "".join('\\"' if c=='"' else '\\\\' if c=='\\' else '\\u%04x' % ord(c) if ord(c)<32 else c for c in value) + '"'
    if isinstance(value,list):
        return "[" + ",".join(canonical_json(v,depth+1) for v in value) + "]"
    if isinstance(value,dict):
        for k in value:
            _scalar(k)
        return "{" + ",".join(canonical_json(k,depth+1)+":"+canonical_json(value[k],depth+1) for k in sorted(value)) + "}"
    _fail()

def fingerprint(value):
    return {"domain":"canonical-json-safe-v1","digest":hashlib.sha256(canonical_json(value).encode()).hexdigest()}

def _fingerprint(v):
    _exact(v,["domain","digest"])
    if v["domain"] not in ["canonical-json-safe-v1","bytes-v1"] or not isinstance(v["digest"],str) or not re.fullmatch("[0-9a-f]{64}",v["digest"]):
        _fail()

def decode_observation(v):
    _exact(v,FIELDS+["native_cwd"] if "native_cwd" in v else FIELDS)
    if type(v["schema_version"]) is not int or v["schema_version"] != 1 or not _safe(v["sequence"]) or not _safe(v["observed_at_unix_ms"]):
        _fail()
    _text(v["observation_id"])
    _uuid(v["producer_instance"])
    _uuid(v["store_epoch"])
    _path(v["workspace"])
    _optional(v.get("native_cwd"),_path)
    if v["event"] not in EVENTS or v["producer_kind"] not in ["codex-hook","codescout-server"] or v["outcome"] not in OUTCOMES:
        _fail()
    c=v["caller"]
    _exact(c,["harness","principal","thread_id","agent_id"])
    if c["harness"] not in ["codex", "unknown"] or (v["producer_kind"]=="codex-hook" and c["harness"]!="codex"):
        _fail()
    if c["harness"]=="unknown" and any(c[k] is not None for k in ["principal","thread_id","agent_id"]):
        _fail()
    if c["principal"] is not None:
        _text(c["principal"])
        if not c["principal"].startswith("codex:") or c["principal"]=="codex:":
            _fail()
    _optional(c["thread_id"])
    _optional(c["agent_id"])
    if v["tool"] is not None and v["tool"] not in TOOLS:
        _fail()
    for k in ["local_invocation","native_call_id","turn_id"]:
        _optional(v[k])
    for k,limit in [("fingerprints",4),("targets",64),("diagnostics",32)]:
        if not isinstance(v[k],list) or len(v[k])>limit:
            _fail()
    for f in v["fingerprints"]:
        _fingerprint(f)
    for t in v["targets"]:
        _exact(t,["path","phase","bytes_len","fingerprint"])
        _path(t["path"])
        if t["phase"] not in ["attempted","committed","uncertain"] or not _safe(t["bytes_len"]):
            _fail()
        _optional(t["fingerprint"],_fingerprint)
    if any(d not in DIAGNOSTICS for d in v["diagnostics"]):
        _fail()
    if len(json.dumps(v,ensure_ascii=False,separators=(",",":")).encode())>65536:
        _fail()
    return v
