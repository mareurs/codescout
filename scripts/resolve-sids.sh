#!/usr/bin/env bash
# Sourced, never executed: defines only `resolve_sids`.
#
# Callers: scripts/pre-push-foreign-session-guard.sh and scripts/hold-publish.sh. Both
# source this file so the liveness answer for a session id cannot drift between them.
#
# Usage: resolve_sids <sid>...  -> one line per sid: <sid>, LIVE|gone|?, uds:socket or empty,
# tab-separated.

# RESOLVE EACH FOREIGN SID TO A LIVE ADDRESS HERE, rather than telling the reader to go and
# run peer-sessions.sh. The dead-author case then answers itself at the point of refusal
# instead of costing a round trip, and the live case arrives with somewhere to send it.
#
# Liveness is the three-part conjunction src/librarian/session_registry.rs:292-323 settled on:
# the messaging socket exists, /proc/<pid> exists, and /proc/<pid>/stat field 22 STRING-equals
# the row's procStart. The third part is what closes pid reuse, and it is a string compare on
# purpose.
#
# Parsed with python3 rather than sed. A registry row carries `formerNames`, a LIST OF OBJECTS
# with their own keys, so a greedy sed binds to the LAST match and can silently read a nested
# value -- the defect already recorded against the peer-enumeration regex. python3 is already
# on the hook path (scripts/pre-commit-ledger-counts.py runs from pre-commit), and this is ONE
# process for all sids rather than one per sid.
#
# DEGRADES TO THE OLD BEHAVIOUR, never to a wrong answer: no python3, no registry, or an
# unreadable row yields `?` and the banner prints the bare sid as it always did. Three-valued
# on purpose -- LIVE / gone / ? -- because collapsing "cannot tell" into "gone" would print
# "unowned, push it" about a session that is running.
resolve_sids() {
    command -v python3 >/dev/null 2>&1 || { printf '%s\t?\t\n' "$@"; return; }
    printf '%s\n' "$@" | python3 -c '
import glob, json, os, sys
want = [l.strip() for l in sys.stdin if l.strip()]
rows = {}
for f in glob.glob(os.path.expanduser("~/.claude*/sessions/*.json")):
    try:
        d = json.load(open(f))
    except Exception:
        continue
    sid = d.get("sessionId")
    if sid not in want:
        continue
    pid, sock, ps = d.get("pid"), d.get("messagingSocketPath"), d.get("procStart")
    state, addr = "gone", ""
    if pid and sock and ps and os.path.exists(sock):
        try:
            with open("/proc/%d/stat" % int(pid)) as fh:
                if fh.read().rsplit(")", 1)[1].split()[19] == str(ps):
                    state, addr = "LIVE", "uds:" + sock
        except Exception:
            state = "?"
    if rows.get(sid, ("gone",))[0] != "LIVE":
        rows[sid] = (state, addr)
for sid in want:
    st, ad = rows.get(sid, ("?", ""))
    print("%s\t%s\t%s" % (sid, st, ad))
' 2>/dev/null || printf '%s\t?\t\n' "$@"
}
