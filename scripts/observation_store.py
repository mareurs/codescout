"""Bounded read-only counterpart to the Linux cooperative Rust observation store."""
from contextlib import ExitStack
from dataclasses import dataclass, field
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time
import uuid

from observation_contract import canonical_json, decode_observation, DIAGNOSTICS

RECORD_BYTES=64*1024
SHARD_BYTES=8*1024*1024
TOTAL_BYTES=64*1024*1024
MAX_SHARDS=4096
POLICY="codescout-observation-policy-v1.json"

@dataclass
class ObservationBatch:
    records: list[dict]=field(default_factory=list)
    diagnostics: list[str]=field(default_factory=list)
    enabled: bool=False
    store_epoch: str|None=None

class _Unavailable(Exception):
    pass

def _object(pairs):
    result={}
    for key,value in pairs:
        if key in result:raise ValueError("duplicate field")
        result[key]=value
    return result

def _json(data):
    return json.loads(data,object_pairs_hook=_object)

def _keys(value,keys):
    if not isinstance(value,dict) or set(value)!=set(keys):raise _Unavailable("incompatible-schema")

def _uuid(value):
    if not isinstance(value,str) or str(uuid.UUID(value))!=value:raise _Unavailable("incompatible-schema")

def _git(root,flag):
    env={k:v for k,v in os.environ.items() if not k.startswith("GIT_")}
    out=subprocess.run(["git","-C",str(root),"rev-parse",flag],env=env,capture_output=True,
                       timeout=2,check=True).stdout
    return Path(os.fsdecode(out.rstrip(b"\n"))).resolve(strict=True)

def _owned(fd,directory=False,private=True):
    meta=os.fstat(fd)
    kind=stat.S_ISDIR(meta.st_mode) if directory else stat.S_ISREG(meta.st_mode)
    if not kind or meta.st_uid!=os.getuid() or private and meta.st_mode&0o077:
        raise _Unavailable("unavailable")
    return meta

def _directory(stack,path=None,parent=None,name=None,private=False):
    flags=os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC
    fd=os.open(str(path) if path is not None else name,flags,dir_fd=parent)
    stack.callback(os.close,fd)
    _owned(fd,True,private)
    return fd

def _file(stack,parent,name,maximum):
    fd=os.open(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC|os.O_NONBLOCK,dir_fd=parent)
    stack.callback(os.close,fd)
    if _owned(fd).st_size>maximum:raise _Unavailable("quota")
    data=bytearray()
    while len(data)<=maximum:
        chunk=os.read(fd,min(65536,maximum+1-len(data)))
        if not chunk:break
        data.extend(chunk)
    if len(data)>maximum:raise _Unavailable("quota")
    return fd,bytes(data)

def _supported(fd):
    # The initial contract supports the same measured Linux filesystems as Rust.
    info=ctypes.create_string_buffer(256)
    if ctypes.CDLL(None,use_errno=True).fstatfs(fd,ctypes.byref(info))!=0:
        raise _Unavailable("unavailable")
    magic=ctypes.c_long.from_buffer(info).value&0xffffffff
    if magic not in (0xef53,0x01021994,0x9123683e,0x58465342,0x794c7630):
        raise _Unavailable("unavailable")

def _lock(fd):
    import fcntl
    deadline=time.monotonic()+0.1
    while True:
        try:
            fcntl.flock(fd,fcntl.LOCK_SH|fcntl.LOCK_NB)
            return
        except BlockingIOError:
            if time.monotonic()>=deadline:raise _Unavailable("unavailable")
            time.sleep(0.002)

def read_observations(root):
    batch=ObservationBatch()
    try:
        root=Path(root).resolve(strict=True)
        try:
            actual=_git(root,"--show-toplevel")
        except subprocess.CalledProcessError:
            # The legacy scan API also accepts non-Git fixtures. No state means no opt-in;
            # the real formatter independently requires a Git checkout.
            if not os.path.lexists(root/".git") and not os.path.lexists(root/".codescout/observations") and not (root/".codescout").is_symlink():
                batch.diagnostics.append("disabled")
                return batch
            raise
        if actual!=root:raise _Unavailable("checkout-mismatch")
        if sys.platform!="linux":
            gitdir=_git(root,"--absolute-git-dir")
            if not os.path.lexists(gitdir/POLICY) and not os.path.lexists(root/".codescout/observations"):
                batch.diagnostics.append("disabled")
                return batch
            raise _Unavailable("unavailable")
        with ExitStack() as stack:
            git=_directory(stack,path=_git(root,"--absolute-git-dir"))
            policy=None
            policy_bytes=0
            try:
                _,raw=_file(stack,git,POLICY,4096)
                _supported(git)
                policy_bytes=len(raw)
                policy=_json(raw)
                _keys(policy,("schema_version","workspace","first_epoch"))
                _uuid(policy["first_epoch"])
                if policy["schema_version"]!=1 or policy["workspace"]!=str(root):raise _Unavailable("checkout-mismatch")
            except FileNotFoundError:pass
            rfd=_directory(stack,path=root)
            try:
                cs=_directory(stack,parent=rfd,name=".codescout")
                observations=_directory(stack,parent=cs,name="observations",private=True)
                if _file(stack,observations,".gitignore",2)[1]!=b"*\n":raise _Unavailable("unavailable")
                directory=_directory(stack,parent=observations,name="v1",private=True)
                _supported(directory)
                os.stat("control.json",dir_fd=directory,follow_symlinks=False)
            except FileNotFoundError:
                batch.diagnostics.append("reset" if policy else "disabled")
                return batch
            lock,_=_file(stack,directory,"quota.lock",0)
            _lock(lock)
            _,raw=_file(stack,directory,"control.json",512*1024)
            control_bytes=len(raw)
            control=_json(raw)
            _keys(control,("schema_version","workspace","epoch","enabled","producer_sequences"))
            _uuid(control["epoch"])
            expected=control["producer_sequences"]
            if not isinstance(expected,dict) or len(expected)>MAX_SHARDS:raise _Unavailable("incompatible-schema")
            for producer,count in expected.items():
                _uuid(producer)
                if type(count) is not int or not 1<=count<=2**53-1:raise _Unavailable("incompatible-schema")
            if control["schema_version"]!=1 or type(control["enabled"]) is not bool:raise _Unavailable("incompatible-schema")
            if control["workspace"]!=str(root):raise _Unavailable("checkout-mismatch")
            if policy is None or policy["first_epoch"]!=control["epoch"]:batch.diagnostics.append("reset")
            batch.enabled=control["enabled"];batch.store_epoch=control["epoch"]
            _,raw=_file(stack,directory,"health.json",4096)
            health=_json(raw);_keys(health,("diagnostics",))
            if not isinstance(health["diagnostics"],list) or len(health["diagnostics"])>32 or any(d not in DIAGNOSTICS for d in health["diagnostics"]):
                raise _Unavailable("unavailable")
            batch.diagnostics.extend(health["diagnostics"])
            seen={};sequences={};total=policy_bytes+2+control_bytes+len(raw)
            if total>TOTAL_BYTES:raise _Unavailable("quota")
            # scandir is consumed lazily; a hostile directory cannot create an unbounded list.
            with os.scandir(directory) as entries:
                for count,entry in enumerate(entries):
                    if count>=MAX_SHARDS+3:raise _Unavailable("quota")
                    if entry.name in ("control.json","quota.lock","health.json"):continue
                    if not re.fullmatch(r"[0-9a-f-]{36}-[0-9]{6,}\.jsonl",entry.name):
                        batch.diagnostics.append("unavailable");continue
                    with ExitStack() as shard:
                        _,data=_file(shard,directory,entry.name,SHARD_BYTES)
                        if not data:batch.diagnostics.append("torn-record")
                        total+=len(data)
                        if total>TOTAL_BYTES:raise _Unavailable("quota")
                        for line in data.splitlines(keepends=True):
                            if len(line)>RECORD_BYTES or not line.endswith(b"\n"):
                                batch.diagnostics.append("torn-record");continue
                            try:
                                wrapper=_json(line);_keys(wrapper,("record","checksum"))
                                record=decode_observation(wrapper["record"])
                                if hashlib.sha256(canonical_json(record).encode()).hexdigest()!=wrapper["checksum"]:
                                    raise _Unavailable("checksum")
                                if record["workspace"]!=str(root) or record["store_epoch"]!=control["epoch"]:
                                    raise _Unavailable("checkout-mismatch")
                            except _Unavailable as error:
                                batch.diagnostics.append(str(error));continue
                            except (ValueError,TypeError,KeyError,RecursionError,UnicodeError):
                                batch.diagnostics.append("incompatible-schema");continue
                            old=seen.get(record["observation_id"])
                            if old is not None:
                                if old!=line:batch.diagnostics.append("conflicting-duplicate")
                                continue
                            seen[record["observation_id"]]=line
                            sequences.setdefault(record["producer_instance"],[]).append(record["sequence"])
                            batch.records.append(record)
            if set(sequences)!=set(expected):batch.diagnostics.append("sequence-gap")
            for producer,values in sequences.items():
                count=expected.get(producer)
                # Compare length first: hostile metadata never allocates range(count).
                if count!=len(values) or sorted(values)!=list(range(len(values))):
                    batch.diagnostics.append("sequence-gap")
            batch.records.sort(key=lambda r:(r["producer_instance"],r["sequence"]))
    except _Unavailable as error:batch.diagnostics.append(str(error))
    except (OSError,ValueError,TypeError,KeyError,RecursionError,UnicodeError,subprocess.SubprocessError):
        batch.diagnostics.append("unavailable")
    batch.diagnostics=sorted(set(batch.diagnostics))
    return batch
