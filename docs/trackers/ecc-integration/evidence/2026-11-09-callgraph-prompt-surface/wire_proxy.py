"""Transparent stdio MCP transport recorder for fixture-only routing evaluation."""
import hashlib,json,os,subprocess,sys,threading
from pathlib import Path
binary,project,record=sys.argv[1:]
child=subprocess.Popen([binary,"start","--project",project],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
pending={}; lock=threading.Lock()
def save(data):
    with lock,Path(record).open("a") as f: f.write(json.dumps(data)+"\n")
def incoming():
    for line in sys.stdin:
        try:
            data=json.loads(line)
            if "id" in data: pending[str(data["id"])]=data.get("method")
        except Exception: pass
        child.stdin.write(line);child.stdin.flush()
    child.stdin.close()
threading.Thread(target=incoming,daemon=True).start()
try:
    for line in child.stdout:
        try:
            data=json.loads(line); method=pending.get(str(data.get("id")))
            if method in ("initialize","tools/list"):
                save({"method":method,"result":data.get("result"),"error":data.get("error")})
        except Exception as exc: save({"recorder_error":str(exc)})
        sys.stdout.write(line);sys.stdout.flush()
finally:
    child.terminate()
    try: child.wait(timeout=5)
    except subprocess.TimeoutExpired: child.kill();child.wait()
