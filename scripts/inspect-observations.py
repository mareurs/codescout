#!/usr/bin/env python3
"""Read-only diagnostic inspector; it never claims execution completeness."""
import argparse
from dataclasses import asdict
import json
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from observation_store import read_observations
from operation_observations import reconcile_observations

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace",type=Path,required=True)
    parser.add_argument("--json",action="store_true")
    args=parser.parse_args()
    batch=read_observations(args.workspace)
    result=reconcile_observations(batch.records,[])
    value=dict(schema_version=1,enabled=batch.enabled,store_epoch=batch.store_epoch,coverage="unknown",
               record_count=len(batch.records),writes=[asdict(w) for w in result.writes],pairs=result.pairs,
               aliases=result.aliases,gaps=[dict(source_id=g.source_id,reason=g.reason,workspace=str(g.workspace) if g.workspace is not None else None,paths=g.paths) for g in result.gaps],diagnostics=sorted(set(batch.diagnostics+result.diagnostics)))
    print(json.dumps(value,ensure_ascii=False,sort_keys=True,indent=None if args.json else 2))
    return 0

if __name__=="__main__":raise SystemExit(main())
