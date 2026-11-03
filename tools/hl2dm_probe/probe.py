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
ATTACK, JUMP, DUCK, FORWARD, BACK, USE = 1, 2, 4, 8, 16, 32
RELOAD, ATTACK2 = 8192, 2048
MOVELEFT, MOVERIGHT, SPEED, WALK, ZOOM = 512, 1024, 131072, 262144, 524288


def connect():
    return rcon.Rcon(("127.0.0.1", PORT), "chimera-probe")


def setup(r, bots=1):
    """Makes the server safe to probe: the bots' NextBot behaviour stopped
    (it crashes the server when a bot dies), forced respawn on (the
    dedicated server resets the config's value at the map load), and at
    least `bots` bots in the game."""
    r.exec("sv_cheats 1")
    r.exec("nb_player_stop 1")
    r.exec("mp_forcerespawn 1")
    r.exec("probe_bot 0")
    have = r.exec("status").count("BOT")
    for _ in range(max(0, bots - have)):
        r.exec("hl2mp_bot_add")
    if have < bots:
        time.sleep(1.0)


def run(r, start, ticks, name="probe"):
    """`start`: (x, y, z, pitch, yaw, vx, vy, vz) or None; `ticks`: a list
    of (buttons, forward, side, pitch, yaw[, select[, impulse]]). Returns
    (rows, sounds, hurts): rows as dicts by column name; the other line
    kinds (README) of the run are in `LAST`."""
    full = run_full(r, start, ticks, name)
    return full["rows"], full["sounds"], full["hurts"]


LAST = {}


def run_full(r, start, ticks, name="probe"):
    """`run`, returning every line kind by name: rows, sounds, hurts,
    weapon (W), ents (E), damage (D), traced (A), te (T), created (C), removed (X),
    events (V), ent_sounds (s)."""
    folder = os.path.join(SERVER, "hl2mp")
    inp, out = f"{name}_in.txt", f"{name}_out.txt"
    with open(os.path.join(folder, inp), "w") as f:
        if start:
            f.write("start " + " ".join(f"{v:.4f}" for v in start) + "\n")
        for t in ticks:
            extra = "".join(f" {v}" for v in t[5:])
            f.write(f"{t[0]} {t[1]:.3f} {t[2]:.3f} {t[3]:.4f} {t[4]:.4f}{extra}\n")
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
    cols = ("tick x y z vx vy vz ground ducked top eye buttons water movetype aux health armor maxspeed life obs gt server_tick tickbase"
            .split())
    kinds = {"S": "sounds", "H": "hurts", "W": "weapon", "E": "ents", "D": "damage", "T": "te", "C": "created",
             "X": "removed", "V": "events", "s": "ent_sounds", "A": "traced"}
    out = {k: [] for k in kinds.values()}
    out["rows"] = []
    with open(path) as f:
        for l in f.read().splitlines():
            p = l.split()
            if not p:
                continue
            if p[0] in kinds:
                out[kinds[p[0]]].append(p[1:])
            else:
                out["rows"].append({c: float(v) for c, v in zip(cols, p)})
    LAST.clear()
    LAST.update(out)
    return out


def hold(n, buttons=0, forward=0.0, side=0.0, pitch=0.0, yaw=0.0, select=None):
    """`n` ticks of the same input; `select` switches weapon on the first."""
    ticks = [(buttons, forward, side, pitch, yaw)] * n
    if select and n:
        ticks[0] = (buttons, forward, side, pitch, yaw, select)
    return ticks


def wait_alive(r, timeout=12.0):
    """Waits until a living bot is in the game (forced respawn: up to 5 s)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        if " fake 1 " in r.exec("probe_info"):
            return True
        time.sleep(0.25)
    return False
