"""The HL2:DM probe's movement and suit cases (specs/hl2dm/movement.md
P2-P9, P12-P13; suit.md P3-P5), on dm_lockdown at the first deathmatch
spawn's open floor. Prints each case's numbers beside the spec's.

    python3 tools/hl2dm_probe/measure.py [case ...]
"""

import math
import sys

from probe import DUCK, FORWARD, JUMP, SPEED, WALK, connect, hold
from probe import run as probe_run


def run(r, *args, **kw):
    """`probe.run` with the bot's health, armour and aux power reset."""
    r.exec("probe_set 100 0 100")
    return probe_run(r, *args, **kw)

# dm_lockdown: open flat floor at z 0 beside a deathmatch spawn; -x
# (yaw 180) is clear and flat for about 200 units.
SPOT = (-2842.78, 2100.0, 0.03125)


def start(z=0.03125, yaw=180.0, v=(0.0, 0.0, 0.0), pitch=0.0):
    return (SPOT[0], SPOT[1], z, pitch, yaw, v[0], v[1], v[2])


def speed(row):
    return math.hypot(row["vx"], row["vy"])


def show(name, got, want):
    print(f"{name}: got {got} want {want}")


def accel(r):
    for label, buttons, want in [
        ("run", 0, [28.5, 51.0, 73.5, 96.0, 118.5, 139.89, 160.0, 178.9, 190.0]),
        ("walk", WALK, [22.5, 39.0, 55.5, 72.0, 88.5, 105.0, 121.2, 136.43, 150.0]),
        ("sprint", SPEED, [48, 90, 132, 172.08, 209.76, 245.17, 278.46, 309.75, 320]),
    ]:
        rows, _, _ = run(r, start(), hold(3) + hold(12, FORWARD | buttons, 450, 0, 0, 180))
        show(f"P6 {label}", [round(speed(x), 2) for x in rows[4:13]], want)
    rows, _, _ = run(r, start(v=(-190, 0, 0)), hold(28, 0, 0, 0, 0, 180))
    # The placement tick already runs the first move with the start velocity.
    sp = [round(speed(x), 2) for x in rows]
    show("P6 friction 1/10/11/12/26/27", [sp[0], sp[9], sp[10], sp[11], sp[25], sp[26]], [178.6, 102.34, 96.2, 90.2, 6.2, 0])


def jump(r):
    for label, ducked in [("standing", False), ("ducked", True)]:
        pre = hold(40, DUCK) if ducked else hold(10)
        b = DUCK if ducked else 0
        rows, sounds, _ = run(r, start(), pre + hold(1, JUMP | b) + hold(45, b))
        base = rows[len(pre)]
        z0 = base["z"]
        traj = rows[len(pre) + 1 :]
        zs = [round(x["z"] - z0, 4) for x in traj]
        apex = max(zs)
        ground = next((i + 1 for i, x in enumerate(traj) if i > 0 and x["ground"]), None)
        show(f"P2/P3 {label} jump: first vz, apex (tick), grounded tick",
             (round(traj[0]["vz"], 3), apex, zs.index(apex) + 1, ground),
             "146.5, 20.145 (17), 34" if not ducked else "151.0, 21.33 (18), 35")
        if not ducked:
            show("P2 feet above floor when grounded", zs[ground - 1] if ground else None, 1.275)


def duck(r):
    rows, _, _ = run(r, start(), hold(10) + hold(1, JUMP) + hold(8) + hold(10, DUCK))
    before, after = rows[19], rows[20]
    show("P4 air duck: feet lift, top, eye", (round(after["z"] - before["z"], 3), after["top"], after["eye"]),
         "36 + move, 36, 28")
    rows, _, _ = run(r, start(), hold(40, DUCK) + hold(20, DUCK | FORWARD, 450, 0, 0, 180))
    show("P4 ducked on ground: top, eye", (rows[39]["top"], rows[39]["eye"]), "36, 28")
    show("P5 ducked accel", [round(speed(x), 3) for x in rows[41:44]] + [round(speed(rows[56]), 3), round(speed(rows[57]), 3)],
         [9.5, 13.0, 16.5, 62.0, 63.333])
    rows, _, _ = run(r, start(), hold(10) + hold(30, DUCK))
    # eyes[k]: k ticks after the press (eyes[0], the press tick).
    eyes = [round(x["eye"], 3) for x in rows[11:]]
    show("row 21 eye at 13/26/27 after the press", (eyes[13], eyes[26], eyes[27]), (46.675, 28.066, 28))


def aux(r):
    rows, sounds, _ = run(r, start(), hold(3) + hold(12, FORWARD | SPEED, 450, 0, 0, 180))
    show("P8/P3 aux from rest, ticks 1-5", [round(x["aux"], 4) for x in rows[4:9]], [100, 99.625, 99.25, 98.875, 98.5])
    show("sprint sounds", [s for s in sounds if "sprint" in " ".join(s).lower()][:3], "SprintStart once")
    rows, _, _ = run(r, start(), hold(3) + hold(67, SPEED))
    show("P8 standing sprint 67 ticks: aux", rows[-1]["aux"], 100)
    # Tap: moving, +speed one tick, then the recharge 34 ticks after.
    rows, _, _ = run(r, start(yaw=180), hold(3) + hold(20, FORWARD, 450, 0, 0, 180) + hold(1, FORWARD | SPEED, 450, 0, 0, 180)
                     + hold(60, 0, 0, 0, 0, 180))
    a = [round(x["aux"], 4) for x in rows[23:]]
    show("suit P4 tap: tick 1, 2, 35, 36, 51", (a[1], a[2], a[35], a[36], a[51]), (99.625, 97.125, 97.125, 97.3125, 100))
    # Running dry: sprint in a circle (turning 6 degrees a tick).
    ticks = [(FORWARD | SPEED, 450, 0, 0, 180 + 6.0 * k) for k in range(290)]
    rows, sounds, _ = run(r, start(), hold(3) + ticks)
    a = [x["aux"] for x in rows[4:]]
    empty = next(i + 1 for i, v in enumerate(a) if v == 0)
    show("suit P4 run dry: last aux > 0 and first 0 (ticks)", (round(a[empty - 2], 4), empty), (0.25, 268))
    sp = [round(speed(x), 1) for x in rows[4:]]
    show("speed around running dry", sp[empty - 3:empty + 4], "320 then down to 190")


def sprint_duck(r):
    rows, _, _ = run(r, start(), hold(3) + hold(1, SPEED) + hold(30, SPEED | DUCK) + hold(12, SPEED | DUCK | FORWARD, 450, 0, 0, 180))
    sp = [round(speed(x), 3) for x in rows[35:]]
    show("P9 sprint kept ducked: speed 1/10/11", (sp[0], sp[9], sp[10]), (16.0, 106.0, 106.667))
    show("P9 aux draining ducked", [round(x["aux"], 3) for x in rows[36:40]], "falling 0.375 a tick")


# A roof at z 272 near the first spawn, open above: the drops.
ROOF = (-4113.8, 3134.79, 272.03125)


def falls(r):
    for h, want in [(236.9, "no damage, step 0.85"), (245, "10 damage, step 1.0, Player.FallDamage"),
                    (512, "10 damage (fixed)")]:
        rows, sounds, hurts = run(r, (ROOF[0], ROOF[1], ROOF[2] + h, 0, 180, 0, 0, 0), hold(130))
        t = next(i for i, x in enumerate(rows) if x["ground"])
        land = [(s[1], s[2], s[5]) for s in sounds if abs(int(s[0]) - t) <= 1]
        show(f"P12 drop {h}: fall speed, damage, armour, sounds", (round(-rows[t - 1]["vz"] + 4.5, 1), 100 - rows[-1]["health"],
             rows[-1]["armor"], land), want)
    r.exec("mp_falldamage 1")
    rows, _, _ = run(r, (ROOF[0], ROOF[1], ROOF[2] + 512, 0, 180, 0, 0, 0), hold(130))
    show("P12 progressive from 512: health", rows[-1]["health"], "38 (62 of 62.5)")
    r.exec("mp_falldamage 0")


def steps(r):
    rows, sounds, _ = run(r, start(yaw=180), hold(3) + hold(70, FORWARD, 450, 0, 0, 180))
    show("P13 run steps (tick, volume, sample)", [(int(s[0]) - 3, s[2], s[5]) for s in sounds][:4], "5, 32, 59 at 0.2 left/right")
    rows, sounds, _ = run(r, start(yaw=180), hold(40, DUCK) + hold(70, DUCK | FORWARD, 450, 0, 0, 180))
    show("P13 ducked steps", sounds, "none")


CASES = {"accel": accel, "jump": jump, "duck": duck, "aux": aux, "sprint_duck": sprint_duck, "falls": falls, "steps": steps}

if __name__ == "__main__":
    r = connect()
    for name in sys.argv[1:] or list(CASES):
        CASES[name](r)
