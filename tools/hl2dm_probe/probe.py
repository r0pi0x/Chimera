"""HL2:DM probe driver (tools/hl2dm_probe/README.md): writes an input
file into the server's hl2mp/ folder, runs `probe_run` over RCON and
returns the logged rows.

    python3 tools/hl2dm_probe/probe.py <case> [...]   # cases: see CASES

The server's folder is $CHIMERA_HL2DM_SERVER (default ~/games/hl2dm-ds);
RCON on 127.0.0.1:$CHIMERA_PROBE_PORT (default 27040), password
chimera-probe (chimera_probe.cfg).
"""

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "css_probe"))
import rcon  # noqa: E402

SERVER = os.path.expanduser(os.environ.get("CHIMERA_HL2DM_SERVER", "~/games/hl2dm-ds"))
PORT = int(os.environ.get("CHIMERA_PROBE_PORT", "27040"))

# Source's buttons.
JUMP, DUCK, FORWARD, BACK, USE = 2, 4, 8, 16, 32
MOVELEFT, MOVERIGHT, SPEED, WALK, ZOOM = 512, 1024, 131072, 262144, 524288


def connect():
    return rcon.Rcon(("127.0.0.1", PORT), "chimera-probe")


def run(r, start, ticks, name="probe"):
    """`start`: (x, y, z, pitch, yaw, vx, vy, vz) or None; `ticks`: a list
    of (buttons, forward, side, pitch, yaw). Returns (rows, sounds, hurts):
    rows as dicts by column name."""
    folder = os.path.join(SERVER, "hl2mp")
    inp, out = f"{name}_in.txt", f"{name}_out.txt"
    with open(os.path.join(folder, inp), "w") as f:
        if start:
            f.write("start " + " ".join(f"{v:.4f}" for v in start) + "\n")
        for t in ticks:
            f.write(f"{t[0]} {t[1]:.3f} {t[2]:.3f} {t[3]:.4f} {t[4]:.4f}\n")
    path = os.path.join(folder, out)
    if os.path.exists(path):
        os.remove(path)
    print(r.exec(f"probe_run {inp} {out}").strip(), file=sys.stderr)
    # Done when the file has every row (the plugin closes it at the end).
    deadline = time.time() + 10 + len(ticks) * 0.03
    rows = []
    while time.time() < deadline:
        time.sleep(0.5)
        if not os.path.exists(path):
            continue
        with open(path) as f:
            lines = f.read().splitlines()
        rows = [l for l in lines if l and l[0].isdigit()]
        if len(rows) > len(ticks):
            break
    cols = "tick x y z vx vy vz ground ducked top eye buttons water movetype aux health armor maxspeed".split()
    parsed = []
    sounds = []
    hurts = []
    with open(path) as f:
        for l in f.read().splitlines():
            p = l.split()
            if not p:
                continue
            if p[0] == "S":
                sounds.append(p[1:])
            elif p[0] == "H":
                hurts.append(p[1:])
            else:
                parsed.append({c: float(v) for c, v in zip(cols, p)})
    return parsed, sounds, hurts


def hold(n, buttons=0, forward=0.0, side=0.0, pitch=0.0, yaw=0.0):
    return [(buttons, forward, side, pitch, yaw)] * n
