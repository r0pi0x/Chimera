"""Ours (Chimera project). The DoD:S probe's measurement cases
(specs/dods/movement.md, weapons.md, damage.md; tools/dods_probe/README.md),
on dod_avalanche's open flat ground. Prints the numbers the specs quote.

    python3 tools/dods_probe/measure.py [case ...]   # no case: all

Cases: sprint caps regen threshold jump crouch getup slow guns fov
fireselect mg frag fragspeed rocket riflegren.
"""

import math
import sys

import probe
from probe import ATTACK, ATTACK2, BACK, DUCK, FORWARD, JUMP, MOVERIGHT, SPEED, hold, props, run_full

ALT1 = 1 << 14  # prone
RELOAD = 8192
# dod_avalanche: flat open ground, about 1,800 units clear towards yaw 315
# and nothing overhead (a spot 200 units back had something above that
# cut jumps short and refused them after a placement).
START = (1683.0, 2473.0, 128.031, 0.0, 315.0, 0.0, 0.0, 0.0)
YAW = 315.0
R = None


def go(ticks, start=START, cmds=()):
    out = run_full(R, start, ticks, cmds=cmds)
    by = props(out)
    for row in out["rows"]:
        row.update(by.get(int(row["tick"]), {}))
        row["hs"] = math.hypot(row["vx"], row["vy"])
    return out


def stamina(value, row):
    return (row, 0, f"probe_setprop 1 send m_flStamina float {value}")


def standing():
    """The bot standing on START, not prone or ducked (a deployed MG keeps
    it prone, so prone means: strip, then get up)."""
    for _ in range(3):
        probe.watch(R, probe.DODS)
        row = go(hold(3))["rows"][-1]
        if row.get("m_bProne") == 1.0 or row["eye"] < 20:
            R.exec("probe_strip")
            R.exec("probe_give weapon_colt")
            go(hold(5, select="weapon_colt") + hold(1, ALT1) + hold(100))
        elif row["ducked"] == 1.0:
            go(hold(30))
        else:
            return


def gun(w, other="weapon_amerknife"):
    """Strips the bot, gives `other` and weapon_<w> with 200 spare rounds;
    returns the gun's entity index."""
    standing()
    R.exec("probe_strip")
    R.exec(f"probe_give {other}")
    idx = int(R.exec(f"probe_give weapon_{w}").split(":")[-1])
    for line in R.exec("probe_inv").splitlines():
        p = line.split()
        if p[:3] == ["weapon", str(idx), f"weapon_{w}"] and int(p[-1]) >= 0:
            R.exec(f"probe_ammo {p[-1]} 200")
    return idx


def sprint():
    standing()
    rows = go(hold(3) + hold(400, FORWARD | SPEED, 400, 0, 0, YAW))["rows"]
    for x in rows[3:20] + rows[20::20]:
        print(int(x["tick"]), f"speed {x['hs']:.2f} stamina {x['m_flStamina']:.2f} sprinting {x['m_bIsSprinting']:.0f}")
    for name, b, f, s, y in [("back", BACK | SPEED, -400, 0, YAW + 180), ("side", MOVERIGHT | SPEED, 0, 400, YAW + 90),
                             ("diagonal", FORWARD | MOVERIGHT | SPEED, 400, 400, YAW + 45),
                             ("ducked", FORWARD | SPEED | DUCK, 400, 0, YAW)]:
        standing()
        x = go(hold(3) + hold(60, b, f, s, 0, y))["rows"][-1]
        print(name, f"speed {x['hs']:.1f} sprinting {x['m_bIsSprinting']:.0f}")


def caps():
    for s0 in (90, 50, 10):
        for b, name in ((FORWARD, "run"), (FORWARD | SPEED, "sprint")):
            standing()
            x = go(hold(5) + hold(120, b, 400, 0, 0, YAW), cmds=[stamina(s0, k) for k in range(5, 125, 2)])["rows"][-1]
            print(name, "stamina held at", s0, f"speed {x['hs']:.2f} (want {120 + s0 if b == FORWARD else 230 + s0})")


def regen():
    def rate(name, ticks, at):
        standing()
        rows = go(ticks, cmds=[stamina(20, at)])["rows"]
        seq = [x["m_flStamina"] for x in rows[at + 1:at + 12]]
        print(name, "per tick", sorted({round(b - a, 3) for a, b in zip(seq, seq[1:])}))
    rate("still", hold(10) + hold(40), 10)
    rate("ducked still", hold(10) + hold(80, DUCK), 60)
    rate("walking 220", hold(10) + hold(80, FORWARD, 400, 0, 0, YAW), 40)
    rate("walking 50", hold(10) + hold(80, FORWARD, 50, 0, 0, YAW), 40)
    rate("ducked moving", hold(10) + hold(80, DUCK | FORWARD, 400, 0, 0, YAW), 60)
    rate("prone still", hold(5) + hold(1, ALT1) + hold(130), 110)
    rate("prone crawling", hold(5) + hold(1, ALT1) + hold(90) + hold(60, FORWARD, 400, 0, 0, YAW), 140)
    rate("in the air", hold(5) + hold(1, JUMP) + hold(60), 8)


def threshold():
    for fm in (75, 79, 80, 100, 101, 110):
        standing()
        rows = go(hold(3) + hold(60, FORWARD | SPEED, fm, 0, 0, YAW))["rows"]
        st = next((int(x["tick"]) for x in rows if x["m_bIsSprinting"] == 1.0), None)
        d = round(rows[-1]["m_flStamina"] - rows[-2]["m_flStamina"], 3)
        print("wish", fm, "sprint from tick", st, "previous speed", round(rows[st - 1]["hs"], 2) if st else None,
              "stamina per tick at the end", d)


def jump():
    for name, pre, b, f, s0 in [
        ("still", hold(20), 0, 0, None), ("still, stamina 0", hold(20), 0, 0, 0),
        ("running", hold(5) + hold(40, FORWARD, 400, 0, 0, YAW), FORWARD, 400, None),
        ("sprinting", hold(5) + hold(40, FORWARD | SPEED, 400, 0, 0, YAW), FORWARD | SPEED, 400, None),
        ("ducked", hold(10) + hold(40, DUCK), DUCK, 0, None), ("duck in the air", hold(20), DUCK, 0, None)]:
        standing()
        cmds = [stamina(s0, len(pre) - 2)] if s0 is not None else []
        rows = go(pre + hold(1, b | JUMP, f, 0, 0, YAW) + hold(80, b, f, 0, 0, YAW), cmds=cmds)["rows"]
        j = len(pre)
        land = next((int(x["tick"]) for x in rows[j + 2:] if x["ground"] == 1), None)
        print(name, f"vz after {rows[j + 1]['vz']:.2f} apex {max(x['z'] for x in rows[j:]) - rows[j]['z']:.2f}",
              "air ticks", land - j - 1 if land else None,
              "stamina", [round(x["m_flStamina"], 2) for x in rows[j - 1:j + 2]])


def crouch():
    standing()
    rows = go(hold(10) + hold(40, DUCK) + hold(25))["rows"]
    print("unduck (released on tick 51):", [(int(x["tick"]), round(x["eye"], 3), int(x["top"])) for x in rows[50:66]])


def getup():
    for name, b in (("alt1", ALT1), ("duck", DUCK), ("jump", JUMP)):
        standing()
        rows = go(hold(2) + hold(1, ALT1) + hold(85) + hold(1, b) + hold(110, FORWARD, 400, 0, 0, YAW))["rows"]
        print(name, [(int(x["tick"]), round(x["eye"], 2), int(x["top"]), int(x["m_bProne"]), round(x["hs"], 1))
                     for x in rows[86:200:6]])


def slow():
    for name, b, f in (("running", FORWARD, 400), ("sprinting", FORWARD | SPEED, 400), ("ducked", FORWARD | DUCK, 400),
                       ("walking 100", FORWARD, 100)):
        standing()
        rows = go(hold(3) + hold(140, b, f, 0, 0, YAW),
                  cmds=[(50, 0, "probe_settime 1 send m_flSlowedUntilTime 0.5")])["rows"]
        print(name, [(int(x["tick"]), round(x["hs"], 1)) for x in rows[48:100:3]])


GUNS = ("garand k98 spring k98_scoped thompson mp40 bar mp44 30cal mg42 colt p38 c96 m1carbine "
        "bazooka pschreck spade").split()


def guns(names=GUNS):
    for w in names:
        cls = f"weapon_{w}"
        idx = gun(w)
        probe.watch(R, [("bot", "send", "m_flNextAttack", "float")])
        t = (hold(1, select="weapon_amerknife") + hold(60) + hold(1, select=cls) + hold(250) + hold(1, ATTACK)
             + hold(60) + hold(1, RELOAD) + hold(400))
        out = go(t, cmds=[(200, 0, f"probe_setprop {idx} send m_iClip1 int 1")])
        W = out["weapon"]
        na = {int(x["tick"]): x.get("m_flNextAttack", 0) - x["gt"] for x in out["rows"]}
        sw = next(int(x[0]) for i, x in enumerate(W) if x[1] == cls and i > 0 and W[i - 1][1] != cls)
        rl = 374
        seq = [(int(x[0]), int(x[2]), float(x[5])) for x in W if x[1] == cls and int(x[0]) >= rl - 1]
        refill = next((c[0] for c in seq if c[1] > 1), None)
        print(f"{w:12s} draw {max(na.get(sw, 0), na.get(sw + 1, 0)):.4f} s  empty reload "
              f"{max([c[2] for c in seq[:3]]):.4f} s, refilled +{(refill - rl) if refill else None} ticks")


def fov():
    for w in ("garand", "k98", "spring", "k98_scoped"):
        gun(w)
        probe.watch(R, [("bot", "send", "m_iFOV", "int"), ("bot", "send", "m_flFOVRate", "float")])
        rows = go(hold(1, select=f"weapon_{w}") + hold(120) + hold(1, ATTACK2) + hold(60) + hold(1, ATTACK2) + hold(30))["rows"]
        seq = [(int(x["tick"]), x.get("m_iFOV"), x.get("m_flFOVRate")) for x in rows]
        print(w, "(alt fire on ticks 122 and 183)", [s for i, s in enumerate(seq) if i == 0 or s[1:] != seq[i - 1][1:]])


def fireselect():
    for w in ("bar", "mp44"):
        gun(w)
        probe.watch(R, [("weapon", "send", "m_bSemiAuto", "int")])
        out = go(hold(1, select=f"weapon_{w}") + hold(120) + hold(1, ATTACK2) + hold(60) + hold(60, ATTACK))
        clips = [(int(x[0]), int(x[2]), float(x[5])) for x in out["weapon"]]
        shots = [c for i, c in enumerate(clips[1:], 1) if c[1] < clips[i - 1][1]]
        print(w, "semi-auto from tick", next(int(x["tick"]) for x in out["rows"] if x.get("m_bSemiAuto") == 1.0),
              "shots while holding fire", shots)


def mg():
    for w in ("30cal", "mg42"):
        idx = gun(w)
        probe.watch(R, probe.DODS + [("weapon", "send", "m_bDeployed", "int"), ("bot", "send", "m_flNextAttack", "float")])
        t = (hold(1, select=f"weapon_{w}") + hold(110) + hold(1, ALT1) + hold(90) + hold(1, ATTACK2) + hold(100)
             + hold(1, RELOAD) + hold(500) + hold(1, ATTACK2) + hold(120, ATTACK))
        out = go(t, cmds=[(250, 0, f"probe_setprop {idx} send m_iClip1 int 0")])
        W = {int(q[0]): q for q in out["weapon"]}
        prev = None
        print(w, "(alt fire on 203, reload on 304, alt fire again on 805)")
        for x in out["rows"]:
            t_ = int(x["tick"])
            cur = (x.get("m_bDeployed"), x.get("m_bProne"), W[t_][2] if t_ in W else None,
                   x.get("m_flDeployedYawLimitLeft"), x.get("m_flDeployedYawLimitRight"))
            if cur != prev and (prev is None or cur[:2] != prev[:2] or cur[3:] != prev[3:] or abs(int(cur[2]) - int(prev[2])) > 1):
                print("  ", t_, cur, "next attack", round(x.get("m_flNextAttack", 0) - x["gt"], 4), "next shot", W[t_][5] if t_ in W else None)
            prev = cur


def frag():
    # the fuse runs from the press: held 335 ticks it goes off in the hand
    for cook in (1, 60, 150):
        standing()
        probe.watch(R, [])
        R.exec("probe_strip")
        R.exec("probe_give weapon_frag_us")
        R.exec("probe_track clear")
        R.exec("probe_track grenade")
        out = go(hold(1, select="weapon_frag_us") + hold(100) + hold(cook, ATTACK) + hold(10)
                 + hold(max(0, 340 - cook), FORWARD, 400, 0, 0, YAW + 180) + hold(40),
                 cmds=[(150, 0, "probe_set 1000 0")])
        print("held", cook, "ticks: pressed on 102, thrown on", out["created"], "removed (exploded)", out["removed"])
    # blast against distance: in the air, the victim (client 2) beside it
    for d in (0, -75, -150, -250, -350):
        standing()
        R.exec("probe_strip")
        R.exec("probe_give weapon_frag_us")
        R.exec("probe_track clear")
        R.exec("probe_track grenade")
        cmds = []
        for row in range(430, 452):
            cmds += [(row, 0, f"probe_tprel 2 0 {d} -36"), (row, 0, "probe_setprop 2 data m_iHealth int 1000"),
                     (row, 0, "probe_set 1000 0")]
        out = go(hold(1, select="weapon_frag_us") + hold(100) + hold(327, ATTACK) + hold(10)
                 + hold(20, FORWARD, 400, 0, 0, YAW + 180) + hold(40), cmds=cmds)
        print("victim offset", d, "damage (tick victim attacker inflictor amount type health)",
              [x for x in out["damage"] if x[1] == "2"])


def fragspeed():
    for aim in (-80, -45, -30, -15, 0, 15, 30):
        standing()
        probe.watch(R, [])
        R.exec("probe_strip")
        R.exec("probe_give weapon_frag_us")
        R.exec("probe_track clear")
        R.exec("probe_track grenade")
        out = go(hold(1, select="weapon_frag_us") + hold(100, 0, 0, 0, aim, YAW) + hold(1, ATTACK, 0, 0, aim, YAW)
                 + hold(3, 0, 0, 0, aim, YAW))
        E = [e for e in out["ents"] if e[2].startswith("grenade")][:3]
        a, b = ([float(v) for v in e[3:6]] for e in E[:2])
        h = math.hypot(b[0] - a[0], b[1] - a[1]) / 0.015
        v = (b[2] - a[2]) / 0.015 + 420 * 0.0075
        print(f"aim {aim:4d}: speed {math.hypot(h, v):6.1f} elevation {math.degrees(math.atan2(v, h)):6.2f} (first step)")
        go(hold(380, FORWARD, 400, 0, 0, YAW + 180), start=None)


def rocket():
    for w in ("bazooka", "pschreck"):
        standing()
        probe.watch(R, [("tracked", "data", "m_vecAbsVelocity", "vec"), ("tracked", "data", "m_flGravity", "float")])
        R.exec("probe_strip")
        R.exec(f"probe_give weapon_{w}")
        R.exec("probe_track clear")
        R.exec("probe_track rocket")
        vx, vy = 1683 + 400 * math.cos(math.radians(YAW)), 2473 + 400 * math.sin(math.radians(YAW))
        out = go(hold(1, select=f"weapon_{w}") + hold(100) + hold(1, ATTACK2) + hold(80) + hold(1, ATTACK) + hold(60),
                 cmds=[(150, 0, "probe_set 1000 0"), (150, 0, f"probe_tp 2 {vx:.2f} {vy:.2f} 128.031 0 0"),
                       (150, 0, "probe_setprop 2 data m_iHealth int 1000")])
        print(w, "first rows", [p for p in out["props"]][:2], "removed", out["removed"],
              "damage", [x for x in out["damage"] if x[1] == "2"])


def riflegren():
    for w, rifle in (("riflegren_us", "garand"), ("riflegren_ger", "k98")):
        standing()
        probe.watch(R, [("tracked", "data", "m_flDamage", "float"), ("tracked", "data", "m_DmgRadius", "float")])
        R.exec("probe_strip")
        R.exec(f"probe_give weapon_{rifle}")
        R.exec(f"probe_give weapon_{w}")
        R.exec("probe_track clear")
        R.exec("probe_track grenade")
        out = go(hold(151, 0, 0, 0, -10, YAW) + hold(1, ATTACK, 0, 0, -10, YAW) + hold(300, 0, 0, 0, -10, YAW),
                 cmds=[(0, 1, f"use weapon_{w}"), (100, 0, "probe_set 1000 0")])
        E = [e for e in out["ents"] if e[2].startswith("grenade")]
        a, b = ([float(v) for v in e[3:6]] for e in E[:2])
        print(w, "first step speed", round(math.dist(a, b) / 0.015, 1), "created", out["created"], "removed",
              out["removed"], "props", out["props"][:2])


CASES = {f.__name__: f for f in (sprint, caps, regen, threshold, jump, crouch, getup, slow, guns, fov, fireselect, mg,
                                 frag, fragspeed, rocket, riflegren)}

if __name__ == "__main__":
    R = probe.connect()
    probe.setup(R, bots=2)
    for name in sys.argv[1:] or CASES:
        print(f"== {name}")
        CASES[name]()
