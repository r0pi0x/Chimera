"""Ours (Chimera project). DoD:S probe driver (tools/dods_probe/README.md): writes an input
file into the server's dod/ folder, runs `probe_run` over RCON and
returns the logged rows.

    import probe; r = probe.connect(); probe.setup(r); probe.run_full(r, ...)

The server's folder is $CHIMERA_DODS_SERVER (default ~/games/dods_ds);
RCON on 127.0.0.1:$CHIMERA_PROBE_PORT (default 27050), password
chimera-probe (chimera_probe.cfg).
"""

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "css_probe"))
import rcon  # noqa: E402

SERVER = os.path.expanduser(os.environ.get("CHIMERA_DODS_SERVER", "~/games/dods_ds"))
PORT = int(os.environ.get("CHIMERA_PROBE_PORT", "27050"))

# Source's buttons.
ATTACK, JUMP, DUCK, FORWARD, BACK, USE = 1, 2, 4, 8, 16, 32
RELOAD, ATTACK2 = 8192, 2048
MOVELEFT, MOVERIGHT, SPEED, WALK, ZOOM = 512, 1024, 131072, 262144, 524288


def connect():
    return rcon.Rcon(("127.0.0.1", PORT), "chimera-probe")


def setup(r, bots=1):
    """At least `bots` of the game's own puppet bots in the game, alive
    (`bot`; the plugin drives the first living one), and the driven one
    picked again."""
    r.exec("sv_cheats 1")
    have = r.exec("status").count("BOT")
    for _ in range(max(0, bots - have)):
        r.exec("bot")
    if have < bots:
        time.sleep(1.0)
    # The second bot is the victim: on the other team (Axis), alive.
    if bots > 1 and " team 3 " not in r.exec("probe_score"):
        r.exec("probe_bot 2")
        r.exec("probe_cmd jointeam 3")
        r.exec("probe_cmd joinclass 0")
        time.sleep(6.0)
    r.exec("probe_bot 1")


def run(r, start, ticks, name="probe"):
    """`start`: (x, y, z, pitch, yaw, vx, vy, vz) or None; `ticks`: a list
    of (buttons, forward, side, pitch, yaw[, select[, impulse]]). Returns
    (rows, sounds, hurts): rows as dicts by column name; the other line
    kinds (README) of the run are in `LAST`."""
    full = run_full(r, start, ticks, name)
    return full["rows"], full["sounds"], full["hurts"]


LAST = {}


def run_full(r, start, ticks, name="probe", cmds=()):
    """`run`, returning every line kind by name: rows, sounds, hurts,
    weapon (W), ents (E), damage (D), traced (A), te (T), created (C), removed (X),
    events (V), ent_sounds (s), commands run (K), watched properties (P:
    tick, name, value...; `watch`). `cmds`: (row, client,
    command) run on that input row by the client (0: the server)."""
    folder = os.path.join(SERVER, "dod")
    inp, out = f"{name}_in.txt", f"{name}_out.txt"
    with open(os.path.join(folder, inp), "w") as f:
        if start:
            f.write("start " + " ".join(f"{v:.4f}" for v in start) + "\n")
        for t in ticks:
            extra = "".join(f" {v}" for v in t[5:])
            f.write(f"{t[0]} {t[1]:.3f} {t[2]:.3f} {t[3]:.4f} {t[4]:.4f}{extra}\n")
        for row, client, cmd in cmds:
            f.write(f"cmd {row} {client} {cmd}\n")
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
             "X": "removed", "V": "events", "s": "ent_sounds", "A": "traced", "K": "commands", "P": "props"}
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


def score(r):
    """`probe_score`: ({client: (team, frags, deaths, life)}, {team: score})."""
    players, teams = {}, {}
    for line in r.exec("probe_score").splitlines():
        p = line.split()
        if p[:1] == ["player"]:
            players[int(p[1])] = (int(p[3]), int(p[5]), int(p[7]), int(p[9]))
        elif p[:1] == ["team"]:
            teams[int(p[1])] = int(p[3])
    return players, teams


def wait_alive(r, timeout=12.0):
    """Waits until a living bot is in the game (forced respawn: up to 5 s)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        if " fake 1 " in r.exec("probe_info"):
            return True
        time.sleep(0.25)
    return False


def watch(r, specs):
    """Replaces the watched properties: `specs` as (who, kind, prop, type[,
    element]) with who "bot", "weapon" or an entity index."""
    r.exec("probe_watch clear")
    for spec in specs:
        r.exec("probe_watch " + " ".join(str(v) for v in spec))


# DoD:S's player fields worth logging every tick (send props, m_Shared).
DODS = [
    ("bot", "send", "m_flStamina", "float"),
    ("bot", "send", "m_bIsSprinting", "int"),
    ("bot", "send", "m_bProne", "int"),
    ("bot", "send", "m_flGoProneTime", "float"),
    ("bot", "send", "m_flUnProneTime", "float"),
    ("bot", "send", "m_flSlowedUntilTime", "float"),
    ("bot", "send", "m_flDeployChangeTime", "float"),
    ("bot", "send", "m_flDeployedHeight", "float"),
    ("bot", "send", "m_flDeployedYawLimitLeft", "float"),
    ("bot", "send", "m_flDeployedYawLimitRight", "float"),
    ("bot", "send", "m_vecPunchAngle", "vec"),
    ("bot", "send", "m_iFOV", "int"),
    ("bot", "send", "m_flDucktime", "float"),
    ("bot", "send", "m_bDucked", "int"),
    ("bot", "send", "m_bDucking", "int"),
    ("bot", "data", "m_flMaxspeed", "float"),
    ("bot", "send", "m_flFriction", "float"),
    ("bot", "data", "m_surfaceFriction", "float"),
]


def props(out):
    """Watched values by tick: {tick: {name: value or tuple}}."""
    by = {}
    for p in out["props"]:
        if len(p) < 3:
            continue
        vals = [float(v) for v in p[2:]]
        by.setdefault(int(p[0]), {})[p[1].split("[")[0]] = vals[0] if len(vals) == 1 else tuple(vals)
    return by
