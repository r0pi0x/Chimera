#!/usr/bin/env bash
# Give the agents' shared jobserver back the tokens it has lost.
#
# tools/agent-env.sh keeps one GNU-make-style jobserver FIFO for every
# agent's cargo on the box (one token per CPU thread). A cargo takes a
# token for each compile job beyond its first and puts it back when the job
# ends; a cargo that is killed mid-build (a tool timeout, Ctrl-C, an
# out-of-memory kill) never does, so its tokens are gone for good. After
# enough kills none are left and every build on the box, of every agent,
# runs one rustc at a time: found drained to 0 of 12 on 2026-11-10, 27
# hours after the FIFO was made (docs/performance.md, "Iteration speed").
#
# The check: tokens in the FIFO plus the tokens the running builds hold
# must add up to the full set. What a build holds is estimated from
# outside: a cargo attached to the FIFO holds one token per running job
# beyond its first (its child processes minus one); any other attached
# process (a rustc run without sccache, a make) at most one per thread.
# With no build attached the FIFO is set to exactly the full set (extra
# tokens, if an estimate was ever short, go too); with builds attached only
# the tokens that are surely missing are added, never removed. The count
# is read with FIONREAD, so nothing is taken out of the FIFO to look.
#
# Usage: tools/jobserver-heal.sh <fifo> <tokens> [holder pid]
# (agent-env.sh runs it when sourced, under its lock, at most once every
# AGENT_JS_HEAL_EVERY seconds, default 20; AGENT_JS_HEAL_EVERY=0 always.)
# Prints one line on stderr when it changes the count.
set -u
fifo=${1:-} jobs=${2:-} holder=${3:-0}
[ -p "$fifo" ] && [ "${jobs:-0}" -gt 0 ] 2>/dev/null || exit 0
command -v python3 >/dev/null 2>&1 || exit 0
every=${AGENT_JS_HEAL_EVERY:-20}
stamp="$(dirname "$fifo")/heal.stamp"
if [ "$every" -gt 0 ] 2>/dev/null && [ -e "$stamp" ]; then
  age=$(( $(date +%s) - $(stat -c %Y "$stamp" 2>/dev/null || echo 0) ))
  [ "$age" -lt "$every" ] && exit 0
fi
python3 - "$fifo" "$jobs" "$holder" <<'PY'
import fcntl, os, struct, sys, termios

fifo, jobs, holder = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
me = os.getpid()
real = os.path.realpath(fifo)


def attached():
    """Pids (other than the holder and this script) with the FIFO open."""
    pids = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) in (holder, me):
            continue
        try:
            for fd in os.listdir(f"/proc/{pid}/fd"):
                try:
                    if os.readlink(f"/proc/{pid}/fd/{fd}") in (fifo, real):
                        pids.append(int(pid))
                        break
                except OSError:
                    pass
        except OSError:
            pass
    return pids


def held(pid):
    """Tokens `pid` may hold: a cargo's running jobs beyond its first,
    anything else one per thread (an upper bound)."""
    try:
        comm = open(f"/proc/{pid}/comm").read().strip()
        tasks = os.listdir(f"/proc/{pid}/task")
    except OSError:
        return 0
    if comm != "cargo":
        return len(tasks)
    kids = 0
    for tid in tasks:
        try:
            kids += len(open(f"/proc/{pid}/task/{tid}/children").read().split())
        except OSError:
            pass
    return max(0, kids - 1)


# Read-write so the open never blocks and a write never fails for want of
# a reader; non-blocking so a drain can't hang.
fd = os.open(fifo, os.O_RDWR | os.O_NONBLOCK)
have = struct.unpack("i", fcntl.ioctl(fd, termios.FIONREAD, b"\0" * 4))[0]
users = attached()
in_use = sum(held(p) for p in users)
if not users:
    want = jobs
else:
    want = max(have, jobs - in_use)
if want > have:
    os.write(fd, b"+" * (want - have))
    print(
        f"jobserver-heal: {have} of {jobs} tokens in the jobserver, {in_use} held by "
        f"{len(users)} running build(s): {want - have} were lost (killed builds); put back",
        file=sys.stderr,
    )
elif want < have:
    try:
        os.read(fd, have - want)
    except BlockingIOError:
        pass
    print(f"jobserver-heal: {have} tokens in the jobserver, {jobs} wanted and no build running: took {have - want} out", file=sys.stderr)
os.close(fd)
PY
: >"$stamp" 2>/dev/null || true
