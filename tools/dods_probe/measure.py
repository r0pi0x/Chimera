"""Ours (Chimera project). The DoD:S probe's measurement cases
(specs/dods/movement.md, weapons.md, damage.md; tools/dods_probe/README.md),
on dod_avalanche's open flat ground. Prints the numbers the specs quote.

    python3 tools/dods_probe/measure.py [case ...]   # no case: all

Cases: sprint caps regen threshold jump crouch getup slow guns fov
fireselect mg frag fragspeed rocket riflegren; added 2026-11-24 (the
bullets message's spread, melee, prone and MG edges): spread nofire
stock reach backstab partial launchers rocketworld throw nadetiming
bounce shield mgyaw mgside ledge proneedge ceiling stillregen; added
later that day (physics props, the stance-change cap, the sighted start,
backstabs on low victims, the MG's yaw limit): mpprops propmass propsurvey
pronecap sightstart backstab2 mgsweep; and the spread pattern and the
helmet (specs/dods/spread.md, bodygroups.md): pattern helmet.
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


# --- 2026-11-24: spread, melee, reloads, explosives, prone and MG edges ---
# A "T tick firebullets ..." line is logged while the server runs input
# row `tick`; that row's movement shows in the logged row tick + 1.

ALT_YAW = 270.0  # open ground towards -y from START (a wall stands 4 units towards +x +y)
CRATE = "models/props_crates/static_crate_64.mdl"  # a 64-unit cube, origin at its base centre


def shots(out):
    """(tick, speed, eye, seed, spread) of each bullets message."""
    rows = {int(r["tick"]): r for r in out["rows"]}
    res = []
    for t in out["te"]:
        if t[1] == "firebullets":
            k = int(t[0])
            r = rows.get(k + 1, {})
            res.append((k, round(r.get("hs", 0), 1), r.get("eye"), int(t[9]), float(t[10])))
    return res


def crate(x, y, zbase):
    """A frozen crate (a physics prop, motion disabled); returns its index."""
    return int(R.exec(f"probe_prop prop_physics_override {CRATE} {x} {y} {zbase} 1000 8").split()[-1])


def one_shot(w, pre=(), b=0, f=0.0, settle=60):
    gun(w)
    t = hold(1, select=f"weapon_{w}") + hold(150) + list(pre) + hold(settle, b, f, 0, 0, YAW) + hold(1, b | ATTACK, f, 0, 0, YAW) + hold(10, b, f, 0, 0, YAW)
    return [(s[1], s[4]) for s in shots(go(t))]


def spread():
    sight = hold(1, ATTACK2) + hold(60)
    prone = hold(1, ALT1) + hold(100)
    for w in GUNS:
        if w in ("bazooka", "pschreck", "spade"):
            continue
        row = {"still": one_shot(w), "moving": one_shot(w, (), FORWARD, 400), "ducked": one_shot(w, hold(30, DUCK), DUCK),
               "prone": one_shot(w, prone)}
        if w in ("garand", "k98", "spring", "k98_scoped", "bar", "mp44"):
            row["alt"] = one_shot(w, sight)
            row["alt moving"] = one_shot(w, sight, FORWARD, 400)
        if w in ("30cal", "mg42"):
            row["deployed"] = one_shot(w, prone + hold(1, ATTACK2) + hold(120))
        print(w, row)
    for fm in (44, 44.9, 45.1, 46):
        print("thompson at a steady", fm, "u/s:", one_shot("thompson", (), FORWARD, fm, 100))
    gun("thompson")
    print("thompson burst:", [s[4] for s in shots(go(hold(1, select="weapon_thompson") + hold(150) + hold(60, ATTACK)))])


def nofire():
    gun("thompson")
    probe.watch(R, probe.DODS)
    R.exec("probe_tp 2 0 0 -5000 0 0")
    out = go(hold(1, select="weapon_thompson") + hold(100) + hold(60, FORWARD | SPEED, 400, 0, 0, YAW)
             + hold(30, FORWARD | SPEED | ATTACK, 400, 0, 0, YAW) + hold(10, FORWARD | ATTACK, 400, 0, 0, YAW))
    print("fire held sprinting 161-190, running 191-200: shots on", [s[0] for s in shots(out)])
    gun("thompson")
    out = go(hold(1, select="weapon_thompson") + hold(100) + hold(1, JUMP | ATTACK) + hold(60, ATTACK))
    land = next(int(r["tick"]) for r in out["rows"][103:] if r["ground"] == 1)
    print("fire held through a jump: shots on", [s[0] for s in shots(out)], "landed (row)", land)
    gun("thompson")
    out = go(hold(1, select="weapon_thompson") + hold(100) + hold(1, ALT1) + hold(100) + hold(20, FORWARD, 400, 0, 0, YAW)
             + hold(30, FORWARD | ATTACK, 400, 0, 0, YAW) + hold(10) + hold(10, ATTACK))
    print("fire held crawling 222-251, then still from 262: shots on", [s[0] for s in shots(out)])


def melee(w, d, victim_yaw, button=ATTACK, lateral=0.0, pitch=0.0):
    """Swing at a standing victim (client 2) placed d units ahead and
    `lateral` to the left, facing victim_yaw. Returns (damage, traced, next-attack)."""
    gun(w)
    R.exec("probe_set 1000 0")
    c, s_ = math.cos(math.radians(YAW)), math.sin(math.radians(YAW))
    x, y = START[0] + d * c - lateral * s_, START[1] + d * s_ + lateral * c
    cmds = [(r, 0, f"probe_tp 2 {x:.2f} {y:.2f} {START[2]} 0 {victim_yaw}") for r in range(140, 190)]
    cmds.append((140, 0, "probe_setprop 2 data m_iHealth int 1000"))
    out = go(hold(1, select=f"weapon_{w}") + hold(150, 0, 0, 0, pitch, YAW) + hold(1, button, 0, 0, pitch, YAW)
             + hold(40, 0, 0, 0, pitch, YAW), cmds=cmds)
    R.exec("probe_tp 2 0 0 -5000 0 0")
    nxt = [float(x[5]) for x in out["weapon"] if int(x[0]) == 152]
    return ([(x[0], x[4]) for x in out["damage"] if x[1] == "2"], [(x[0], x[3], x[6]) for x in out["traced"] if x[1] == "2"], nxt)


def stock():
    for w in ("thompson", "mp40"):
        print(w, "alt fire, victim facing (tick, damage), (tick, pre-zone damage, group), next attack:", melee(w, 45, YAW + 180, ATTACK2, 0, 20))
        print(w, "alt fire, victim facing away:", melee(w, 45, YAW, ATTACK2, 0, 20)[0])


def reach():
    for w, b in (("amerknife", ATTACK), ("spade", ATTACK), ("thompson", ATTACK2)):
        for d in (80, 90, 92, 94, 100):
            print(w, d, "units:", melee(w, d, YAW + 180, b, 0, 0)[1])


def backstab():
    for pitch, offs in ((0, (72, 72.5, 73)), (20, (60, 70, 71, 71.5, 75, -70)), (40, (66, 68))):
        for off in offs:
            print("pitch", pitch, "victim facing", off, "deg off the attacker's yaw:", melee("amerknife", 40, YAW + off, ATTACK, 0, pitch)[0])
    for vy in (105, -60):
        print("victim 40 left, facing", vy, "off the attacker's yaw:", melee("amerknife", 40, YAW + vy, ATTACK, 40, 20)[0])


def partial():
    for w, n in (("thompson", 10), ("mp40", 10), ("bar", 10), ("mp44", 10), ("colt", 3), ("p38", 3), ("c96", 10),
                 ("m1carbine", 7), ("k98", 2), ("spring", 2), ("k98_scoped", 2)):
        idx = gun(w)
        out = go(hold(1, select=f"weapon_{w}") + hold(150) + hold(1, RELOAD) + hold(400),
                 cmds=[(140, 0, f"probe_setprop {idx} send m_iClip1 int {n}")])
        refill = next(int(x[0]) for x in out["weapon"] if int(x[0]) > 152 and int(x[2]) > n)
        reserve = [x[3] for x in out["weapon"] if int(x[0]) == refill]
        print(w, "clip", n, "reload pressed on 151: refilled after", refill - 152, "ticks", f"({(refill - 152) * 0.015:.3f} s), reserve", reserve)


def launchers():
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for w in ("bazooka", "pschreck"):
        for at in (120, 200):
            gun(w)
            out = go(hold(1, select=f"weapon_{w}") + hold(100) + hold(1, ATTACK2) + hold(80) + hold(1, ATTACK) + hold(at) + hold(1, RELOAD) + hold(400))
            press = 183 + at
            na = [float(x[5]) for x in out["weapon"] if int(x[0]) == 183]
            refill = next((int(x[0]) for x in out["weapon"] if int(x[0]) > press and x[2] == "1"), None)
            print(w, "next attack after the shot", na, "reload pressed", at, "ticks after: refilled after",
                  refill - press - 1 if refill else None, "ticks")
    for w, rifle in (("riflegren_us", "garand"), ("riflegren_ger", "k98")):
        standing()
        R.exec("probe_strip")
        R.exec(f"probe_give weapon_{rifle}")
        idx = int(R.exec(f"probe_give weapon_{w}").split(":")[-1])
        for line in R.exec("probe_inv").splitlines():
            p = line.split()
            if p[:3] == ["weapon", str(idx), f"weapon_{w}"] and int(p[-1]) >= 0:
                R.exec(f"probe_ammo {p[-1]} 5")
        out = go(hold(151) + hold(1, ATTACK) + hold(200) + hold(1, RELOAD) + hold(400), cmds=[(0, 1, f"use weapon_{w}")])
        refill = next(int(x[0]) for x in out["weapon"] if int(x[0]) > 352 and x[1] == f"weapon_{w}" and x[2] == "1")
        print(w, "reload pressed on 352: refilled after", refill - 353, "ticks")


def rocketworld():
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for w in ("bazooka", "pschreck"):
        for pitch in (45, 10, 3, 0):
            standing()
            probe.watch(R, [])
            R.exec("probe_strip")
            R.exec(f"probe_give weapon_{w}")
            R.exec("probe_track clear")
            R.exec("probe_track rocket")
            out = go(hold(1, select=f"weapon_{w}") + hold(100, 0, 0, 0, pitch, YAW) + hold(1, ATTACK2, 0, 0, pitch, YAW)
                     + hold(80, 0, 0, 0, pitch, YAW) + hold(1, ATTACK, 0, 0, pitch, YAW) + hold(300, 0, 0, 0, pitch, YAW))
            E = [e for e in out["ents"] if e[2].startswith("rocket")]
            print(w, "pitch", pitch, "created", out["created"], "removed", out["removed"], "last", E[-1][3:6] if E else None,
                  "damage to the thrower", [x[4:6] for x in out["damage"] if x[1] == "1"])


def first_step(out):
    E = [e for e in out["ents"] if e[2].startswith("grenade")]
    a, b = ([float(v) for v in e[3:6]] for e in E[:2])
    return [(b[0] - a[0]) / 0.015, (b[1] - a[1]) / 0.015, (b[2] - a[2]) / 0.015 + 420 * 0.0075]


def nade(count=1):
    standing()
    probe.watch(R, [])
    R.exec("probe_strip")
    R.exec("probe_give weapon_frag_us")
    R.exec(f"probe_ammo 13 {count}")
    R.exec("probe_track clear")
    R.exec("probe_track grenade")


def throw():
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for aim in (-20, -25, -30, -35, -40, -50, -60, -70, -80, -89):
        nade()
        v = first_step(go(hold(1, select="weapon_frag_us") + hold(100, 0, 0, 0, aim, YAW) + hold(1, ATTACK, 0, 0, aim, YAW) + hold(4, 0, 0, 0, aim, YAW)))
        print(f"aim {aim}: speed {math.hypot(*v):.1f} elevation {math.degrees(math.atan2(v[2], math.hypot(v[0], v[1]))):.2f}")
        go(hold(380, FORWARD, 400, 0, 0, YAW + 180), start=None)
    for name, b, f, s_ in (("forward", FORWARD, 400, 0), ("back", BACK, -400, 0), ("right", MOVERIGHT, 0, 400)):
        nade()
        v = first_step(go(hold(1, select="weapon_frag_us") + hold(100) + hold(40, b, f, s_, 0, YAW) + hold(1, b | ATTACK, f, s_, 0, YAW) + hold(4, b, f, s_, 0, YAW)))
        c, sn = math.cos(math.radians(YAW)), math.sin(math.radians(YAW))
        print(f"running {name} at 220: along the aim {v[0] * c + v[1] * sn:.1f}, across {-v[0] * sn + v[1] * c:.1f}, up {v[2]:.1f}")
        go(hold(380), start=None)


def nadetiming():
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for held in (1, 30, 60, 100):
        nade(2)
        out = go(hold(1, select="weapon_frag_us", yaw=ALT_YAW) + hold(100, 0, 0, 0, 0, ALT_YAW) + hold(held, ATTACK, 0, 0, 0, ALT_YAW) + hold(150, 0, 0, 0, 0, ALT_YAW))
        W = out["weapon"]
        resets = [int(W[i][0]) for i in range(1, len(W)) if float(W[i][5]) > float(W[i - 1][5]) + 0.1]
        print("pressed on 101, held", held, ": thrown on", [c[0] for c in out["created"]], "weapon blocked anew on", resets)
        go(hold(300, 0, 0, 0, 0, ALT_YAW), start=None)
    nade(2)
    out = go(hold(1, select="weapon_frag_us", yaw=ALT_YAW) + hold(100, 0, 0, 0, 0, ALT_YAW) + hold(40, FORWARD | SPEED, 400, 0, 0, ALT_YAW)
             + hold(1, FORWARD | SPEED | ATTACK, 400, 0, 0, ALT_YAW) + hold(30, FORWARD | SPEED, 400, 0, 0, ALT_YAW)
             + hold(1, FORWARD | ATTACK, 400, 0, 0, ALT_YAW) + hold(10, FORWARD, 400, 0, 0, ALT_YAW))
    print("pressed sprinting on 141, running on 172: thrown on", [c[0] for c in out["created"]])
    go(hold(300, 0, 0, 0, 0, ALT_YAW), start=None)


def bounce():
    for aim in (89, 45, 0):
        nade()
        out = go(hold(1, select="weapon_frag_us") + hold(100, 0, 0, 0, aim, YAW) + hold(1, ATTACK, 0, 0, aim, YAW) + hold(120, 0, 0, 0, aim, YAW))
        P = [(int(e[0]), [float(v) for v in e[3:6]]) for e in out["ents"] if e[2].startswith("grenade")]
        print("aimed", aim, "down: (tick, z, horizontal step)",
              [(t, round(p[2], 2), round(math.hypot(p[0] - q[0], p[1] - q[1]), 2)) for (t, p), (_, q) in zip(P[1:], P)][:70])
        go(hold(300), start=None)


def shield():
    """A cooked frag going off in the air; the victim 150 units out, the
    thrower aside or between them."""
    for name, victim, thrower in (("aside", (0, -150, -36), (80, -75, -36)), ("between at 40", (0, -150, -36), (0, -40, -36)),
                                  ("between at 75", (0, -150, -36), (0, -75, -36)), ("between at 110", (0, -150, -36), (0, -110, -36))):
        nade()
        cmds = []
        for row in range(430, 452):
            cmds += [(row, 0, "probe_tprel 2 %g %g %g" % victim), (row, 0, "probe_setprop 2 data m_iHealth int 1000"),
                     (row, 0, "probe_set 1000 0"), (row, 0, "probe_tprel 1 %g %g %g" % thrower)]
        out = go(hold(1, select="weapon_frag_us") + hold(100) + hold(327, ATTACK) + hold(10) + hold(20, FORWARD, 400, 0, 0, YAW + 180) + hold(40), cmds=cmds)
        print("thrower", name, ": victim (amount, type)", [x[4:6] for x in out["damage"] if x[1] == "2"],
              "thrower", [x[4:6] for x in out["damage"] if x[1] == "1"])


MG = [("weapon", "send", "m_bDeployed", "int"), ("bot", "send", "m_bProne", "int"), ("bot", "send", "m_bDucked", "int"),
      ("bot", "send", "m_flDeployedHeight", "float"), ("bot", "send", "m_flDeployedYawLimitLeft", "float"),
      ("bot", "send", "m_flDeployedYawLimitRight", "float"), ("bot", "data", "v_angle", "vec")]


def deploy_prone():
    return (hold(1, select="weapon_mg42", yaw=ALT_YAW) + hold(110, 0, 0, 0, 0, ALT_YAW) + hold(1, ALT1, 0, 0, 0, ALT_YAW)
            + hold(90, 0, 0, 0, 0, ALT_YAW) + hold(1, ATTACK2, 0, 0, 0, ALT_YAW) + hold(40, 0, 0, 0, 0, ALT_YAW))


def mgyaw():
    for name, turn in (("yaw left", [(0, 0, 0, 0, ALT_YAW + i) for i in range(1, 71)]),
                       ("yaw right", [(0, 0, 0, 0, ALT_YAW - i) for i in range(1, 71)]),
                       ("pitch down", [t for p in range(40, 58) for t in hold(5, 0, 0, 0, p, ALT_YAW)]),
                       ("pitch up", [t for p in range(-54, -72, -3) for t in hold(5, 0, 0, 0, p, ALT_YAW)])):
        gun("mg42")
        probe.watch(R, MG)
        by = probe.props(go(deploy_prone() + turn))
        print(name, "(tick, view pitch, yaw, deployed):", [(k, by[k].get("v_angle", (None, None))[:2], by[k].get("m_bDeployed")) for k in sorted(by) if k >= 243 and k % 5 == 0][:20])


def mgside():
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for a, d in ((90, 42), (90, 50), (90, 70), (90, 110), (-90, 50), (120, 60), (60, 60), (45, 60), (30, 70), (35, 78)):
        gun("mg42")
        probe.watch(R, MG)
        ang = math.radians(ALT_YAW + a)
        c = crate(START[0] + d * math.cos(ang), START[1] + d * math.sin(ang), START[2] - 40)
        by = probe.props(go(deploy_prone()))
        R.exec(f"probe_fire {c} Kill")
        p = by.get(240, {})
        print(f"crate (top 24 above the ground) {a} deg left, {d} away: deployed {p.get('m_bDeployed')} limits {p.get('m_flDeployedYawLimitLeft')} {p.get('m_flDeployedYawLimitRight')}")


def ledge():
    for stance, tops, gap in (("standing", (10, 30, 50, 54, 56), 2), ("ducked", (8, 14, 38, 44), 2), ("standing", (40,), 12), ("standing", (40,), 22)):
        for top in tops:
            gun("mg42")
            probe.watch(R, MG)
            c = crate(START[0], START[1] - 16 - 32 - gap, START[2] + top - 64)
            b = DUCK if stance == "ducked" else 0
            out = go(hold(1, select="weapon_mg42", yaw=ALT_YAW) + hold(140, b, 0, 0, 0, ALT_YAW) + hold(1, b | ATTACK2, 0, 0, 0, ALT_YAW) + hold(80, b, 0, 0, 0, ALT_YAW))
            R.exec(f"probe_fire {c} Kill")
            r = out["rows"][-1]
            p = probe.props(out).get(int(r["tick"]), {})
            print(f"{stance}, a ledge {top} high {gap} units ahead: deployed {p.get('m_bDeployed')} height {p.get('m_flDeployedHeight')} eye {r['eye']:.2f} ducked {p.get('m_bDucked')}")


def proneedge():
    for name, t in (("prone pressed in the air", hold(5) + hold(1, JUMP) + hold(5) + hold(1, ALT1) + hold(110)),
                    ("prone pressed sprinting", hold(5) + hold(40, FORWARD | SPEED, 400, 0, 0, YAW) + hold(1, FORWARD | SPEED | ALT1, 400, 0, 0, YAW) + hold(100, FORWARD | SPEED, 400, 0, 0, YAW)),
                    ("prone pressed ducked", hold(5) + hold(30, DUCK) + hold(1, DUCK | ALT1) + hold(100, DUCK)),
                    ("getting up (pressed on 93)", hold(2) + hold(1, ALT1) + hold(90) + hold(1, ALT1) + hold(90))):
        standing()
        probe.watch(R, probe.DODS)
        out = go(t)
        by = probe.props(out)
        print(name, "(tick, z, eye, hull top, ground, speed, prone, sprinting, stamina):",
              [(int(x["tick"]), round(x["z"], 1), round(x["eye"], 2), int(x["top"]), int(x["ground"]), round(x["hs"], 1),
                by.get(int(x["tick"]), {}).get("m_bProne"), by.get(int(x["tick"]), {}).get("m_bIsSprinting"),
                round(by.get(int(x["tick"]), {}).get("m_flStamina", 0), 1)) for x in out["rows"][::4]])


def ceiling():
    for base, key, name in ((50, ALT1, "alt1"), (50, DUCK, "duck"), (40, DUCK, "duck"), (30, ALT1, "alt1")):
        standing()
        probe.watch(R, probe.DODS)
        go(hold(2) + hold(1, ALT1) + hold(90))
        c = crate(START[0], START[1], START[2] + base)
        out = go(hold(5) + hold(1, key) + hold(120), start=None)
        R.exec(f"probe_fire {c} Kill")
        by = probe.props(out)
        r = out["rows"][-1]
        print(f"prone under a ceiling {base} up, {name} pressed: after 120 ticks eye {r['eye']:.1f} hull top {int(r['top'])} prone {by[int(r['tick'])]['m_bProne']} ducked {by[int(r['tick'])]['m_bDucked']}")
        go(hold(1, ALT1) + hold(100), start=None)


def stillregen():
    for v in (150, 30):
        standing()
        probe.watch(R, probe.DODS)
        start = list(START)
        start[4] = ALT_YAW
        start[6] = -v
        out = go(hold(30, 0, 0, 0, 0, ALT_YAW), start=tuple(start), cmds=[(0, 0, "probe_setprop 1 send m_flStamina float 20")])
        by = probe.props(out)
        print("sliding from", v, "with no input (tick, speed, stamina gained):",
              [(int(r["tick"]), round(r["hs"], 1), round(by[int(r["tick"])]["m_flStamina"] - by[int(r["tick"]) - 1]["m_flStamina"], 3)) for r in out["rows"][2:20]])


# --- 2026-11-24 (3): physics props, prone from a sprint, sighted start ---
DRUM = "models/props_fortifications/fueldrum.mdl"  # a stock multiplayer prop of dod_avalanche; upright it rests with its origin 17.9 up


def walk_into(cls, mass="-", flags=0, kv="", d=120.0, ticks=150, b=FORWARD, fm=400.0, settle=60, every=None):
    """The bot runs from START along YAW into a prop placed d units ahead
    on the ground; prints (tick, player's distance along, speed, height, prop's
    distance along, prop z) and the prop's collision group before and after."""
    standing()
    R.exec("probe_track clear")
    c, s_ = math.cos(math.radians(YAW)), math.sin(math.radians(YAW))
    e = int(R.exec(f"probe_prop {cls} {DRUM} {START[0] + d * c:.2f} {START[1] + d * s_:.2f} {START[2] + 17.9} {mass} {flags} {kv}").split()[-1])
    go(hold(settle))
    g0 = R.exec(f"probe_get {e} send m_CollisionGroup int").split()[-1]
    out = go(hold(3) + hold(ticks, b, fm, 0, 0, YAW))
    g1 = R.exec(f"probe_get {e} send m_CollisionGroup int").split()[-1]
    R.exec(f"probe_fire {e} Kill")
    along = lambda x, y: (x - START[0]) * c + (y - START[1]) * s_
    ents = {int(x[0]): x for x in out["ents"] if int(x[1]) == e}
    res = []
    for r in out["rows"]:
        t = int(r["tick"])
        en = ents.get(t)
        res.append((t, round(along(r["x"], r["y"]), 1), round(r["hs"], 1), round(r["z"] - START[2], 1),
                    round(along(float(en[3]), float(en[4])), 1) if en else None, round(float(en[5]), 1) if en else None))
    print(f"{cls} mass {mass} spawnflags {flags} {kv}: collision group {g0} -> {g1}")
    for x in res[::every or 1]:
        print("  ", x)
    return res


def mglimit(fwd, left):
    """The MG42 deployed prone at START facing ALT_YAW with a 64-unit crate
    (top 24 above the ground) centred `fwd` ahead and `left` to the left;
    returns the yaw limit fields (left, right)."""
    gun("mg42")
    probe.watch(R, MG)
    ang = math.radians(ALT_YAW)
    x = START[0] + fwd * math.cos(ang) - left * math.sin(ang)
    y = START[1] + fwd * math.sin(ang) + left * math.cos(ang)
    c = crate(x, y, START[2] - 40)
    by = probe.props(go(deploy_prone()))
    R.exec(f"probe_fire {c} Kill")
    p = by.get(240, {})
    return p.get("m_bDeployed"), p.get("m_flDeployedYawLimitLeft"), p.get("m_flDeployedYawLimitRight")


def mgsweep():
    """What narrows the deployed yaw limit: the crate's near face moved in
    2-unit steps beside the player (its centre 90 deg left) and ahead."""
    R.exec("probe_tp 2 0 0 -5000 0 0")
    for d in range(40, 74, 2):
        print("crate beside, near face", d - 32, "left of the eye:", mglimit(0, d))
    for f in (-20, 20, 40):
        for d in range(40, 74, 4):
            print("crate beside, near face", d - 32, "left, centre", f, "ahead:", mglimit(f, d))


def mpprops():
    """Multiplayer props (physics_props.md): settled, spawned asleep (mode
    1 and 2) and a slow walk; the collision group before and after."""
    R.exec("probe_tp 2 0 0 -5000 0 0")
    walk_into("prop_physics_multiplayer", "-", 0, "physicsmode=1", every=10)
    for m in ("10", "50", "150", "400", "1000"):
        walk_into("prop_physics_multiplayer", m, 1, "physicsmode=1", settle=5, every=10 if m != "1000" else 1)
    walk_into("prop_physics_multiplayer", "1000", 1, "physicsmode=2", settle=5, every=10)
    walk_into("prop_physics_multiplayer", "10", 1, "physicsmode=1", settle=5, fm=60, ticks=250, every=10)


def propmass():
    """prop_physics of each mass: a wall (no shadow with turbo physics)."""
    R.exec("probe_tp 2 0 0 -5000 0 0")
    print("sv_turbophysics", R.exec("sv_turbophysics").split('"')[3], "sv_friction", R.exec("sv_friction").split('"')[3],
          "sv_stopspeed", R.exec("sv_stopspeed").split('"')[3])
    for m in ("1", "5", "10", "30", "50", "100", "150", "300", "340", "360", "400"):
        r = walk_into("prop_physics", m, 0, "", every=1000)
        print("  ", [x for x in r if x[0] in (33, 40, 80, 150)])


def propsurvey():
    """The map's multiplayer props: (collision group, physics mode, spawn flags) counts."""
    import collections
    n = collections.Counter()
    for line in R.exec("probe_find prop_physics_multiplayer").splitlines():
        if line[:1].isdigit():
            i = line.split()[0]
            n[tuple(R.exec(f"probe_get {i} {k} {f} int").split()[-1] for k, f in
                    (("send", "m_CollisionGroup"), ("data", "m_iPhysicsMode"), ("data", "m_spawnflags")))] += 1
    for k, v in sorted(n.items()):
        print("group, mode, spawnflags", k, v)


def pronecap():
    """The speed cap while going prone or getting up: A + (B - A) x smoothstep(k/80)."""
    ss = lambda t: 3 * t * t - 2 * t ** 3
    for name, pre, b0, after in (("sprinting, held", hold(60, FORWARD | SPEED, 400, 0, 0, YAW), FORWARD | SPEED, FORWARD | SPEED),
                                 ("running, held", hold(60, FORWARD, 400, 0, 0, YAW), FORWARD, FORWARD),
                                 ("sprinting, +speed released", hold(60, FORWARD | SPEED, 400, 0, 0, YAW), FORWARD | SPEED, FORWARD),
                                 ("sprinting, all released", hold(60, FORWARD | SPEED, 400, 0, 0, YAW), FORWARD | SPEED, 0),
                                 ("still, forward from the press", hold(60), 0, FORWARD),
                                 ("ducked, held", hold(60, DUCK | FORWARD, 400, 0, 0, YAW), DUCK | FORWARD, DUCK | FORWARD)):
        standing()
        probe.watch(R, probe.DODS)
        out = go(hold(5) + pre + hold(1, b0 | ALT1, 400 if b0 else 0, 0, 0, YAW) + hold(100, after, 400 if after else 0, 0, 0, YAW))
        by = probe.props(out)
        res = []
        for x in out["rows"][66:150]:
            k = int(x["tick"]) - 66
            p = by.get(int(x["tick"]) - 1, {})
            a = (230 if p.get("m_bIsSprinting") == 1.0 else 120) + p.get("m_flStamina", 0)
            a = a if p.get("m_bIsSprinting") == 1.0 else min(220, a)
            res.append((k, round(x["hs"], 2), round(a + (50 - a) * ss(min(k / 80, 1)), 2)))
        print(name, "(k, speed, smoothstep cap):", res[::3])
    for name, b, key in (("getting up, forward", FORWARD, ALT1), ("getting up, sprint", FORWARD | SPEED, ALT1),
                         ("to a crouch, forward", DUCK | FORWARD, DUCK)):
        standing()
        probe.watch(R, probe.DODS)
        out = go(hold(2) + hold(1, ALT1) + hold(90) + hold(1, key | b, 400, 0, 0, YAW) + hold(100, b, 400, 0, 0, YAW))
        print(name, "(k, speed, eye):", [(int(x["tick"]) - 94, round(x["hs"], 2), round(x["eye"], 1)) for x in out["rows"][93:190:3]])


def sightstart():
    """Sighted acceleration from rest (Garand, K98)."""
    for w in ("garand", "k98"):
        for settle in (60, 100):
            gun(w)
            probe.watch(R, probe.DODS)
            p = 152 + settle
            out = go(hold(1, select=f"weapon_{w}") + hold(150) + hold(1, ATTACK2) + hold(settle) + hold(140, FORWARD, 400, 0, 0, YAW))
            by = probe.props(out)
            print(w, settle, "ticks after the sights, FOV", by.get(p, {}).get("m_iFOV"),
                  [(int(x["tick"]) - p, round(x["hs"], 2)) for x in out["rows"][p:p + 140] if (int(x["tick"]) - p) % 10 in (1, 2)])


def backstab2():
    """The backstab on crouched (bot_crouch) and prone victims, and its timing."""
    st = lambda: (R.exec("probe_get 2 send m_bDucked int").split()[-1], R.exec("probe_get 2 send m_bProne int").split()[-1])
    for off in (0, 180):
        d = melee("amerknife", 40, YAW + off, ATTACK, 0, 20)
        print("standing victim facing", off, "off: (tick, damage)", d[0], "next attack", d[2][:1])
    R.exec("bot_crouch 1")
    for pitch, offs in ((20, (0, 71, 71.5)), (40, (66, 68))):
        for off in offs:
            print("crouched victim, pitch", pitch, "facing", off, "off:", melee("amerknife", 40, YAW + off, ATTACK, 0, pitch)[0], "victim (ducked, prone)", st())
    R.exec("bot_crouch 0")
    for d_, pitch, off in ((40, 45, 0), (40, 55, 0), (50, 45, 0), (50, 45, 60), (50, 45, 66), (50, 45, 70), (50, 45, 180)):
        if st()[1] != "1":
            R.exec("probe_bot 2")
            go(hold(5) + hold(1, ALT1) + hold(95))
            R.exec("probe_bot 1")
        r = melee("amerknife", d_, YAW + off, ATTACK, 0, pitch)
        print("prone victim", d_, "ahead, pitch", pitch, "facing", off, "off:", r[0], "next attack", r[2][:1], "victim", st())
    R.exec("probe_bot 2")
    go(hold(5) + hold(1, ALT1) + hold(95))
    R.exec("probe_bot 1")


# --- 2026-11-24: the spread pattern and the helmet (spread.md, bodygroups.md) ---
# The second bot stands 80 units ahead of START facing the first with
# 100000 health; aimed at its chest (`VICTIM_PITCH`) every bullet hits it,
# and its "B" line's force gives the bullet's direction.
VICTIM = (START[0] + 80 * math.cos(math.radians(YAW)), START[1] + 80 * math.sin(math.radians(YAW)), START[2])
VICTIM_PITCH = math.degrees(math.atan2(18.0, 80.0))
PLACE_VICTIM = [(0, 0, "probe_tp 2 {:.3f} {:.3f} {:.3f} 0 {}".format(*VICTIM, YAW - 180)),
                (0, 0, "probe_setprop 2 send m_iHealth int 100000")]


def aim_frame(pitch, yaw):
    """Forward, right and up of view angles (degrees, pitch down)."""
    p, y = math.radians(pitch), math.radians(yaw)
    f = (math.cos(p) * math.cos(y), math.cos(p) * math.sin(y), -math.sin(p))
    r = (math.sin(y), -math.cos(y), 0.0)
    u = (math.sin(p) * math.cos(y), math.sin(p) * math.sin(y), math.cos(p))
    return f, r, u


def offsets(out, pitch):
    """[(tick, seed, spread, [(x, y) of each bullet hitting the victim])]:
    x, y the bullet's tangent offsets right and up of the aim."""
    f, r, u = aim_frame(pitch, YAW)
    hits = {}
    for b in out["bullets"]:
        d = [float(v) for v in b[8:11]]
        n = math.sqrt(sum(c * c for c in d))
        if b[1] != "2" or n == 0:
            continue
        df = sum(a * c for a, c in zip(d, f)) / n
        hits.setdefault(int(b[0]), []).append((sum(a * c for a, c in zip(d, r)) / n / df,
                                               sum(a * c for a, c in zip(d, u)) / n / df))
    return [(int(t[0]), int(t[9]), float(t[10]), hits.get(int(t[0]), [])) for t in out["te"] if t[1] == "firebullets"]


def seeded(w, seeds, every, pitch=VICTIM_PITCH):
    """One shot of weapon_<w> at the victim every `every` ticks, each command's seed forced."""
    gun(w)
    probe.watch(R, [])
    ticks = hold(5, select=f"weapon_{w}") + hold(80)
    cmds = list(PLACE_VICTIM)
    for s in seeds:
        cmds.append((len(ticks), 0, f"probe_seed {s}"))
        ticks += hold(1, ATTACK, 0, 0, pitch, YAW) + hold(every - 1, 0, 0, 0, pitch, YAW)
    out = go(ticks, cmds=cmds)
    R.exec("probe_seed -1")
    return offsets(out, pitch)


def pattern():
    """Each forced seed's bullet in units of the spread (spread.md's test cases)."""
    for w, seeds, every in (("thompson", range(30), 7), ("garand", range(30, 38), 26), ("c96", range(100, 110), 6),
                            ("mg42", range(1, 12), 8)):
        for k, seed, spread, bs in seeded(w, seeds, every):
            print(w, "seed", seed, "spread", spread, [f"{x / spread:.4f} {y / spread:.4f}" for x, y in bs])
    same = {(s, tuple(round(v, 5) for v in b[0])) for _, s, _, b in seeded("thompson", [12345] * 7, 7) if b}
    print("seed 12345 seven times:", same)


def helmet():
    """Body and skin of every player (probe_model), then the victim's body
    group around Garand hits on its chest and head, set back to 0 before
    each (bodygroups.md)."""
    print(R.exec("probe_model"))
    gun("garand")
    probe.watch(R, [("2", "send", "m_nBody", "int"), ("2", "send", "m_nSkin", "int")])
    ticks = hold(5, select="weapon_garand") + hold(1, ATTACK2) + hold(40)
    cmds = list(PLACE_VICTIM)
    for z in (165, 188, 189, 190, 191):
        pitch = math.degrees(math.atan2(START[2] + 58 - z, 80.0))
        cmds.append((len(ticks), 0, "probe_setprop 2 send m_nBody int 0"))
        ticks += hold(2, 0, 0, 0, pitch, YAW) + hold(1, ATTACK, 0, 0, pitch, YAW) + hold(28, 0, 0, 0, pitch, YAW)
    out = go(ticks, cmds=cmds)
    body = {int(p[0]): p[2] for p in out["props"] if p[1].startswith("m_nBody")}
    for a in out["traced"]:
        print("hit, hitgroup", a[6], "body before", body.get(int(a[0])), "after", body.get(int(a[0]) + 1))


CASES = {f.__name__: f for f in (sprint, caps, regen, threshold, jump, crouch, getup, slow, guns, fov, fireselect, mg,
                                 frag, fragspeed, rocket, riflegren, spread, nofire, stock, reach, backstab, partial,
                                 launchers, rocketworld, throw, nadetiming, bounce, shield, mgyaw, mgside, ledge,
                                 proneedge, ceiling, stillregen, mpprops, propmass, propsurvey, pronecap,
                                 sightstart, backstab2, mgsweep, pattern, helmet)}

if __name__ == "__main__":
    R = probe.connect()
    probe.setup(R, bots=2)
    for name in sys.argv[1:] or CASES:
        print(f"== {name}")
        CASES[name]()
