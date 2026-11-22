#!/usr/bin/env bash
# Report (and, only on request, fix) an sccache server that has leaked
# jobserver tokens.
#
# NEVER stops a server on its own: other agents' builds run through it, and a
# restart kills them (exit 101, no message: 2026-11-21). Without --force this
# only says what it sees; `tools/sccache-heal.sh --force` restarts it, which
# a human or an agent must do on purpose, with no build running.
#
# sccache's server keeps its own jobserver (one token per CPU thread) and
# hands it to the rustc it runs. When a client is killed mid-build (a tool
# timeout), the server kills that rustc (kill_on_drop), and tokens the rustc
# held are gone for good. Once all are gone every later request waits forever
# at 0% CPU, which looks like a hung server. Whenever the server has no
# children, its pipe must hold every token; fewer, on two looks 3 s apart,
# means a leak and nothing real is in flight: reported (--force restarts).
# Usage: tools/sccache-heal.sh [--force]   (agent-env.sh runs it without
# --force when sourced)
set -u
force=0; [ "${1:-}" = --force ] && force=1
bin="${RUSTC_WRAPPER:-sccache}"
port="${SCCACHE_SERVER_PORT:-4226}"
pid=$(ss -ltnpH "sport = :$port" 2>/dev/null | sed -n 's/.*pid=\([0-9]*\).*/\1/p' | head -1)
[ -n "$pid" ] || exit 0
# One check at a time: a look takes the tokens out of the pipe for a moment,
# which a second concurrent look would take for a leak.
exec 8>"${XDG_RUNTIME_DIR:-/tmp}/sccache-heal.$port.lock"
flock 8

# Prints "<tokens in the server's jobserver pipe> <running children>".
look() {
  python3 - "$pid" <<'PY'
import os, sys
pid = sys.argv[1]
fds = {}
for fd in os.listdir(f"/proc/{pid}/fd"):
    try:
        t = os.readlink(f"/proc/{pid}/fd/{fd}")
    except OSError:
        continue
    if t.startswith("pipe:"):
        flags = int(open(f"/proc/{pid}/fdinfo/{fd}").read().split("flags:")[1].split()[0], 8) & 3
        fds.setdefault(t, {})["w" if flags == 1 else "r"] = fd
pair = next(((v["r"], v["w"]) for v in fds.values() if "r" in v and "w" in v), None)
tokens = -1
if pair:
    r = os.open(f"/proc/{pid}/fd/{pair[0]}", os.O_RDONLY | os.O_NONBLOCK)
    n = 0
    try:
        while True:
            b = os.read(r, 4096)
            if not b:
                break
            n += len(b)
    except BlockingIOError:
        pass
    if n:
        w = os.open(f"/proc/{pid}/fd/{pair[1]}", os.O_WRONLY)
        os.write(w, b"+" * n)
    tokens = n
kids = 0
try:
    kids = len(open(f"/proc/{pid}/task/{pid}/children").read().split())
except OSError:
    pass
print(tokens, kids)
PY
}

want=$(nproc)
read -r t k < <(look 2>/dev/null)
{ [ "${k:-1}" -ne 0 ] || [ "${t:--1}" -lt 0 ] || [ "$t" -ge "$want" ]; } && exit 0
sleep 3
read -r t k < <(look 2>/dev/null)
{ [ "${k:-1}" -ne 0 ] || [ "${t:--1}" -lt 0 ] || [ "$t" -ge "$want" ]; } && exit 0
if [ "$force" = 1 ]; then
  echo "sccache-heal: server $pid has $t of $want jobserver tokens and no children; restarting it (--force)" >&2
  "$bin" --stop-server >/dev/null 2>&1
  env -u MAKEFLAGS -u CARGO_MAKEFLAGS -u MFLAGS "$bin" --start-server >/dev/null 2>&1
else
  echo "sccache-heal: server $pid has $t of $want jobserver tokens and no children (hung?); left running. With no build in flight: tools/sccache-heal.sh --force" >&2
fi
