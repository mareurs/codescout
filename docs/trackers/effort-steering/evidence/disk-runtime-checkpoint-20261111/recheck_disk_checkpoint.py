"""Independent closure of the existing checkpoint, accounting for tar's hard-link encoding."""
import hashlib
import json
from pathlib import Path
import os
import stat
import tarfile

DEST=Path("/home/marius/work/claude/codescout/docs/trackers/effort-steering/evidence/disk-runtime-checkpoint-20261111")
PRODUCER_LOG="integration/disk-checkpoint-seal.log"

def measure(stream):
    digest=hashlib.sha256()
    count=0
    while chunk:=stream.read(1024*1024):
        digest.update(chunk)
        count+=len(chunk)
    return count,digest.hexdigest()

def main():
    archive=DEST/"evidence.tar.gz"
    hardlinks=0
    changed=[]
    with tarfile.open(archive) as packed:
        members=packed.getmembers()
        manifest=json.loads(packed.extractfile("manifest.json").read())
        entries=manifest["entries"]
        by_name={m.name:m for m in members}
        assert len(by_name)==len(members)==len(entries)+1
        for member in members:
            if member.name=="manifest.json": continue
            row=entries[member.name]
            original=Path(row["source"])
            assert stat.S_IMODE(member.mode)==row["mode"]
            if row["kind"]=="file":
                assert member.isfile() or member.islnk(),(member.name,member.type)
                if member.islnk():
                    hardlinks+=1
                    assert member.linkname in entries and member.linkname!=member.name
                    target=entries[member.linkname]
                    assert target["kind"]=="file"
                    assert (target["size"],target["sha256"])==(row["size"],row["sha256"])
                with packed.extractfile(member) as stream:
                    assert measure(stream)==(row["size"],row["sha256"]),member.name
                with original.open("rb") as stream:
                    current=measure(stream)
                if current!=(row["size"],row["sha256"]):
                    changed.append(member.name)
            elif row["kind"]=="directory":
                assert member.isdir() and original.is_dir()
            else:
                assert member.issym() and member.linkname==row["target"]==os.readlink(original)
    assert changed==[PRODUCER_LOG],changed
    with archive.open("rb") as stream: size,digest=measure(stream)
    script=Path(__file__).resolve()
    with script.open("rb") as stream: _,verifier_sha=measure(stream)
    receipt={"schema":"disk-runtime-checkpoint-readback-v1","archive":archive.name,
        "bytes":size,"sha256":digest,"payload_entries":len(entries),
        "logical_regular_file_payloads":sum(r["kind"]=="file" for r in entries.values()),
        "tar_hardlink_payloads_resolved_and_hashed":hardlinks,
        "all_archive_payloads_reopened_and_size_hash_verified":True,
        "original_control_and_runtime_source_files_unchanged":True,
        "originals_changed_after_snapshot":changed,
        "producer_log_limit":"The original sealer archived its own empty live log, then its validation failed because tar encoded Cargo hard-linked files as hard-link entries. Its retained final traceback differs from that empty log snapshot. No other original differs. This independent verifier resolved and measured every logical file payload.",
        "verifier":str(script),"verifier_sha256":verifier_sha,
        "scope":manifest["scope"],"provider_forwarded":0,"paid_ready":False}
    with (DEST/"closure.json").open("x") as stream:
        json.dump(receipt,stream,indent=2,sort_keys=True)
        stream.write("\n")
    print(json.dumps(receipt),flush=True)

if __name__=="__main__":
    main()
