"""Validate the inherited live formatter lease without taking or upgrading a lock."""
import os
from pathlib import Path
import stat
import struct
import sys
from native_provenance import ProofError, _json, _regular, _require

def _verify_lease(environ, prefix, root, before):
    _require(type(before) is int and 0 <= before < (1 << 64)-1
             and environ.get(prefix+"_GENERATION") == str(before+1)
             and environ.get(prefix+"_ROOT") == str(root), "native lease generation/root differs")
    descriptor = environ[prefix+"_FD"]
    _require(isinstance(descriptor,str) and descriptor.isascii() and descriptor.isdecimal(),
             "native lease descriptor is invalid")
    fd = int(descriptor)
    current = os.fstat(fd)
    _require(stat.S_ISREG(current.st_mode), "native lease descriptor is not a regular file")
    with _regular(root/".codescout/native-provenance-v1.lock") as stream:
        expected = os.fstat(stream.fileno())
        _require((current.st_dev,current.st_ino) == (expected.st_dev,expected.st_ino),
                 "native lease descriptor points at another file")
    info = Path("/proc/self/fdinfo")/str(fd)
    locks = [line.split()[2:] for line in info.read_text().splitlines() if line.startswith("lock:")]
    _require(len(locks) == 1 and len(locks[0]) == 7 and locks[0][:3] == ["FLOCK","ADVISORY","WRITE"]
             and locks[0][-2:] == ["0","EOF"], "native formatter does not hold an exclusive whole-file lease")
    with _regular(root/".codescout/native-provenance-v1.counter") as stream:
        counter = stream.read(9)
    _require(len(counter) == 8 and struct.unpack("<Q",counter)[0] == before+1,
             "native lease was replayed or another effect intervened")

def _coordination_root(certificate, environ):
    # This path is copied by the trusted server from its account-wide domain,
    # after checking the native producer's exact domain binding. It is not an
    # arbitrary history path or a model-facing tool result.
    value=certificate["coordination_root"]
    _require(isinstance(value,str) and Path(value).is_absolute()
             and value==environ.get("CODESCOUT_PROVENANCE_COORDINATOR_ROOT"),
             "native coordination root differs")
    path=Path(value)
    metadata=path.lstat()
    _require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid==os.geteuid()
             and metadata.st_mode & 0o077==0 and path.resolve(strict=True)==path,
             "native coordination directory is not private and owned")
    try:
        (path/".codescout/unverified-shell-v1").lstat()
    except FileNotFoundError:
        return path
    raise ProofError("UNKNOWN: uncertified shell descendants remain possible")

def verify_fence(environ, root):
    try:
        _require(sys.platform == "linux", "live native fence proof is unsupported on this platform")
        root = Path(root).resolve(strict=True)
        certificate = _json(environ["CODESCOUT_NATIVE_PROVENANCE"])
        _require(isinstance(certificate,dict) and set(certificate) ==
                 {"checkpoint","invocation_id","fence_root","fence_generation","coordination_generation","coordination_root"},
                 "native fence certificate fields are missing or unsupported")
        _require(certificate["fence_root"] == str(root), "native fence checkout differs")
        checkpoint = certificate["checkpoint"]
        invocation = certificate["invocation_id"]
        _require(isinstance(checkpoint,dict) and isinstance(invocation,str) and invocation
                 and checkpoint.get("call_id") == invocation, "native fence invocation differs")
        _verify_lease(environ,"CODESCOUT_PROVENANCE_COORDINATOR",_coordination_root(certificate,environ),
                      certificate["coordination_generation"])
        _verify_lease(environ,"CODESCOUT_PROVENANCE_FENCE",root,certificate["fence_generation"])
        return certificate
    except ProofError:
        raise
    except (OSError,KeyError,TypeError,ValueError,OverflowError,UnicodeError) as error:
        raise ProofError("native formatter fence is missing or unreadable") from error
