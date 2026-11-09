"""Reconcile cooperative facts without manufacturing history bodies or permissions."""
from dataclasses import dataclass, field
from datetime import datetime, timezone
import os
from pathlib import Path

from observation_contract import canonical_json, decode_observation, fingerprint
from provenance_records import CoverageGap, ProvenanceScan

OPAQUE={"run_command","exec_command","execute","shell","code_mode"}
WRITERS={"create_file","edit_file","edit_code","doc","memory"}

@dataclass(frozen=True)
class ObservedWrite:
    observation_id: str
    local_invocation: str|None
    workspace: str
    path: str
    principal: str|None
    thread_id: str|None
    native_call_id: str|None
    turn_id: str|None
    tool: str|None
    fingerprint: dict|None
    bytes_len: int
    observed_at_unix_ms: int
    outcome: str

@dataclass
class Reconciliation:
    writes: list[ObservedWrite]=field(default_factory=list)
    pairs: list[tuple[str,str]]=field(default_factory=list)
    aliases: dict[str,list[str]]=field(default_factory=dict)
    diagnostics: list[str]=field(default_factory=list)
    gaps: list[CoverageGap]=field(default_factory=list)

def _key(record):
    return (record["caller"]["principal"],record["caller"]["thread_id"],record["native_call_id"],
            record["turn_id"],record["workspace"],record["tool"])

def _fingerprints(record):
    return {(f["domain"],f["digest"]) for f in record["fingerprints"] if f["domain"]=="canonical-json-safe-v1"}

def _bounded_add(index,key,value):
    """Retain enough distinct candidates to disprove uniqueness, without unbounded buckets."""
    values=index.setdefault(key,[])
    if value not in values and len(values)<2:values.append(value)


def reconcile_observations(records,history_operations):
    result=Reconciliation()
    unique={};valid=[]
    for record in records:
        try:
            decode_observation(record)
            encoded=canonical_json(record)
        except (ValueError,TypeError,KeyError,RecursionError):
            result.diagnostics.append("incompatible-schema");continue
        old=unique.get(record["observation_id"])
        if old is not None:
            if old!=encoded:result.diagnostics.append("conflicting-duplicate")
            continue
        unique[record["observation_id"]]=encoded;valid.append(record)
    groups={};pending={}
    for record in sorted(valid,key=lambda r:(r["producer_instance"],r["sequence"])):
        result.diagnostics.extend(record["diagnostics"])
        workspace=Path(record["workspace"])
        if record["producer_kind"]=="codescout-server" and record["local_invocation"] is not None:
            groups.setdefault(record["local_invocation"],[]).append(record)
        for target in record["targets"]:
            boundary=(record["producer_instance"],record["local_invocation"],_key(record),
                      target["path"],target["bytes_len"],canonical_json(target["fingerprint"]))
            if target["phase"]=="attempted":
                pending.setdefault(boundary,[]).append((record,target));continue
            if target["phase"]=="committed":
                if record["local_invocation"] is not None and pending.get(boundary):pending[boundary].pop(0)
                result.writes.append(ObservedWrite(record["observation_id"],record["local_invocation"],
                    record["workspace"],target["path"],record["caller"]["principal"],record["caller"]["thread_id"],
                    record["native_call_id"],record["turn_id"],record["tool"],target["fingerprint"],
                    target["bytes_len"],record["observed_at_unix_ms"],record["outcome"]))
            else:
                result.gaps.append(CoverageGap(record["observation_id"],"stock write phase is not committed",workspace,(target["path"],)))
        if record["tool"] in OPAQUE:
            result.gaps.append(CoverageGap(record["observation_id"],"opaque command or wrapper may have additional effects",workspace))
        if record["outcome"] in ("missing-terminal","error-possible-effects") or record["event"]=="observer-diagnostic":
            result.gaps.append(CoverageGap(record["observation_id"],"stock observation has incomplete effect coverage",workspace))
    for unresolved in pending.values():
        for record,target in unresolved:
            result.gaps.append(CoverageGap(record["observation_id"],"stock write phase is not committed",Path(record["workspace"]),(target["path"],)))
    representatives={};invalid_invocations=set()
    full_identity=set();partial_identity=set();full_fingerprints={};partial_fingerprints={}
    for invocation,group in groups.items():
        terminal_outcomes={r["outcome"] for r in group if r["event"]=="result-observed"}
        if len(terminal_outcomes)>1 or len({_key(r) for r in group})!=1 or len({frozenset(_fingerprints(r)) for r in group})!=1:
            invalid_invocations.add(invocation)
            result.diagnostics.append("context-conflict")
            result.gaps.extend(CoverageGap(r["observation_id"],"conflicting local invocation context",Path(r["workspace"])) for r in group)
            continue
        if not terminal_outcomes:
            result.diagnostics.append("missing-terminal")
            result.gaps.append(CoverageGap(group[0]["observation_id"],"stock invocation has no terminal result",Path(group[0]["workspace"])))
        representative=next((r for r in group if r["event"]=="request-observed"),group[0])
        representatives[invocation]=representative
        key=_key(representative)
        if all(value is not None for value in key):
            partial=key[:1]+key[2:]
            full_identity.add(key);partial_identity.add(partial)
            for fp in _fingerprints(representative):
                _bounded_add(full_fingerprints,(key,fp),invocation)
                _bounded_add(partial_fingerprints,(partial,fp),invocation)
        if representative["tool"] in WRITERS and not any(r["targets"] for r in group):
            terminal=next((r for r in group if r["event"]=="result-observed"),None)
            if terminal is None or terminal["outcome"]!="refused":
                result.gaps.append(CoverageGap(representative["observation_id"],"stock writer has no observed target boundary",Path(representative["workspace"])))
    for hook in (r for r in valid if r["producer_kind"]=="codex-hook" and r["event"] in ("hook-pre","hook-post")):
        key=_key(hook)
        if key[1] is None:
            key=key[:1]+key[2:];index=partial_fingerprints;identities=partial_identity
        else:index=full_fingerprints;identities=full_identity
        candidates=set()
        for fp in _fingerprints(hook):candidates.update(index.get((key,fp),()))
        if len(candidates)==1:
            representative=representatives[next(iter(candidates))]
            result.pairs.append((hook["observation_id"],representative["observation_id"]))
        else:
            if len(candidates)>1:result.diagnostics.append("ambiguous-pair")
            elif key in identities:result.diagnostics.append("unsupported-fingerprint")
            if hook["tool"] in WRITERS or hook["tool"] in OPAQUE:
                result.gaps.append(CoverageGap(hook["observation_id"],"unjoined hook is not server effect coverage",Path(hook["workspace"])))
    # Index rich-history keys once; never scan every history body for every observation.
    history={}
    for operation in history_operations:
        if not operation.name.startswith("mcp__codescout__"):continue
        key=(operation.principal,operation.native_thread_id,operation.call_id,
             operation.native_turn_id,operation.native_workspace,operation.name.removeprefix("mcp__codescout__"))
        if any(value is None for value in key):continue
        try:
            value=fingerprint(operation.input)
            fp=(value["domain"],value["digest"])
        except (ValueError,TypeError,RecursionError):continue
        _bounded_add(history,(key,fp),operation.source_id)
    for record in valid:
        if record["local_invocation"] in invalid_invocations:continue
        key=_key(record);aliases=set();candidates=set()
        for fp in _fingerprints(record):
            aliases.update(history.get((key,fp),()))
            candidates.update(full_fingerprints.get((key,fp),()))
        if len(candidates)>1:
            result.diagnostics.append("ambiguous-pair");continue
        if len(aliases)==1:result.aliases[record["observation_id"]]=sorted(aliases)
        elif aliases:result.diagnostics.append("ambiguous-pair")
    result.diagnostics=sorted(set(result.diagnostics))
    if any(d!="disabled" for d in result.diagnostics):
        for workspace in sorted({r["workspace"] for r in valid}):
            result.gaps.append(CoverageGap("stock-observations","stock diagnostics: "+", ".join(result.diagnostics),Path(workspace)))
    return result

def _relative(path,root):
    try:return str(Path(os.path.normpath(path)).relative_to(root))
    except (ValueError,TypeError):return None

def merge_scan_observations(scan,reconciliation,root,store_diagnostics=()):
    root=Path(root)
    owners={p:list(values) for p,values in scan.owners.items()}
    evidence={k:(list(v) if v is not None else None) for k,v in scan.evidence.items()}
    streams={p:{who:set(agents) for who,agents in values.items()} for p,values in scan.streams.items()}
    gaps=list(scan.gaps)
    # Keep the baseline authority channels intact. A stock timestamp must not move
    # a previously window-excluded own write into an authorizing window.
    for gap in reconciliation.gaps:
        if gap.workspace is not None and gap.workspace!=root:continue
        paths=tuple(rel for raw in (gap.paths or ()) if (rel:=_relative(raw,root)) is not None)
        if gap.paths and not paths:continue
        gaps.append(CoverageGap(gap.source_id,gap.reason,root,paths))
    for write in reconciliation.writes:
        if write.workspace!=str(root):continue
        rel=_relative(write.path,root)
        if rel is None:continue
        known={who for who,_ in owners.get(rel,[])}
        if write.principal is None:
            reason="observed write has unknown native root"
        elif write.principal not in known:
            reason="additional observed writer; stock facts cannot establish formatter permission"
        else:
            continue
        gaps.append(CoverageGap(write.observation_id,reason,root,(rel,)))
    diagnostics=sorted(set(store_diagnostics)|set(reconciliation.diagnostics))
    damaged=[d for d in store_diagnostics if d!="disabled"]
    if damaged:gaps.append(CoverageGap("stock-store","stock observation store: "+", ".join(sorted(set(damaged))),root))
    return ProvenanceScan(owners,evidence,gaps,streams,
                          list(scan.observed_writes)+reconciliation.writes,diagnostics,
                          dict(scan.observation_aliases)|reconciliation.aliases)

def observed_foreign_principals(scan,rel,root,me):
    return sorted({w.principal for w in scan.observed_writes
        if w.workspace==str(root) and _relative(w.path,Path(root))==rel
        and w.principal is not None and w.principal!=me})
