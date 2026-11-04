"""The HL2:DM probe's cases beyond movement (measure.py): respawn
(specs/hl2dm/rules.md P2-P4), armour (suit.md P1-P2, tests 1-9), weapon
timings and spread (weapons.md P1, P2, P5-P10, P13, P16, P17), head
hits (P15, the bolt's), projectiles (projectiles.md P1-P4, P9, the
frag's drag), the gravity gun's punts and launches (gravity_gun.md
P2-P5, hanging crates), pickups (maps_entities.md P1, P2),
chargers (suit.md P10, P11), swimming and drowning (movement.md rows
59-61, suit.md P6), TDM (rules.md P5, P7, P8, friendly fire, team
spawns: `tdm`, `tdm_spawns` change the map and come back to DM), crates
dropped on and punted into a bot (gravity_gun.md P10: `crate_hits`), punts
of a crate on the floor (`floor_punts`), `+use` carry with
`hl2mp_allow_pickup` (`use_carry`) and the cvars every spec lists. Prints each case's
numbers beside the spec's.

    python3 tools/hl2dm_probe/cases.py [case ...]   # dm_lockdown cases
    python3 tools/hl2dm_probe/cases.py swim drown   # after `changelevel dm_underpass`

Timers a player's command starts (a frag's fuse, a missile's ignition, an
item's or weapon's respawn) run on the command's clock, the bot's tick
base, which is 4 ticks ahead of the server's (`sv_clockcorrection_msecs`
60); counted in server ticks from the event they land 4-5 ticks after
the specs' numbers.
"""

import math
import re
import sys
import time

import measure
import probe
from probe import ATTACK, ATTACK2, DUCK, FORWARD, JUMP, RELOAD, USE, hold

SPOT = measure.SPOT
CRATE = "models/props_junk/wood_crate001a.mdl"
BULLET, BLAST = 2, 64


def show(name, got, want):
    print(f"{name}: got {got} want {want}")


def diffs(xs):
    return [b - a for a, b in zip(xs, xs[1:])]


def arm(r, *classes, ammo=250):
    r.exec("probe_strip")
    for c in classes:
        r.exec(f"probe_give {c}")
    for t in range(32):
        r.exec(f"probe_ammo {t} {ammo}")


def shots(out):
    return [int(t[0]) for t in out["te"] if t[1] == "bullets"]


def clip_changes(out):
    w = out["weapon"]
    return [int(b[0]) for a, b in zip(w, w[1:]) if a[2] != b[2] and a[1] == b[1]]


ROOF = measure.ROOF


def fall_death(r, ticks):
    """Dies from a fall (health 5, 512 units) on input row 85, inside a
    tick; returns (death row, ticks to the respawn or None)."""
    for _ in range(3):
        time.sleep(0.5)
        probe.wait_alive(r)
        r.exec("probe_set 5 0 100")
        rows = probe.run_full(r, (ROOF[0], ROOF[1], ROOF[2] + 512, 0, 180, 0, 0, 0), ticks)["rows"]
        d = next((i for i, x in enumerate(rows) if x["life"] != 0), None)
        if d is not None and rows[0]["life"] == 0:
            break
    a = next((i for i, x in enumerate(rows) if i > d and x["life"] == 0), None)
    return d, (a - d if a else None), sorted(set(int(x["obs"]) for x in rows))


def respawn(r):
    d, t, obs = fall_death(r, hold(600))
    show("rules P3 forced respawn after an in-tick death", (t, obs), "334, observer mode 0 throughout")
    show("rules P4 button held throughout", fall_death(r, hold(600, ATTACK))[1], "never")
    show("rules P4 release, press at +150 and +201", fall_death(r, hold(96, ATTACK) + hold(140) + hold(1, ATTACK)
         + hold(50) + hold(1, ATTACK) + hold(300))[1], "202")
    show("rules P4 press at +199", fall_death(r, hold(285) + hold(1, ATTACK) + hold(300))[1], "334 (too early)")
    show("rules P4 press at +200", fall_death(r, hold(286) + hold(1, ATTACK) + hold(300))[1], "201")


def hurt(r, armour, damage, kind, health=100):
    r.exec("probe_setprop %s data m_flDamageAccumulator float 0" % bot(r))
    r.exec(f"probe_set {health} {armour} 100")
    s = r.exec(f"probe_hurt {damage} {kind}").split()
    return int(s[4]), int(s[6])


def bot(r):
    m = re.search(r"player (\d+) fake 1", r.exec("probe_info"))
    return m.group(1) if m else "1"


def armour(r):
    probe.wait_alive(r)
    show("suit test 1 (100 armour, 30 bullet)", hurt(r, 100, 30, BULLET), (94, 76))
    show("suit test 2 (100 armour, 30 blast)", hurt(r, 100, 30, BLAST), (94, 52))
    lost = [(d, 100 - hurt(r, 100, d, BULLET)[1]) for d in (1, 2, 3, 5, 6, 7, 8, 13, 24, 63)]
    show("armour lost per bullet damage on 100", lost, "trunc(0.8 D), at least 1 (spec: ceil)")
    lost = [(d, 100 - hurt(r, 100, d, BLAST)[1]) for d in (1, 2, 6, 8)]
    show("armour lost per blast damage on 100", lost, "trunc(1.6 D), at least 1")
    show("armour running out (10, 30 bullet / blast; 5, 7)", (hurt(r, 10, 30, BULLET), hurt(r, 10, 30, BLAST),
         hurt(r, 5, 7, BULLET)), "(80, 0), (75, 0), (98, 0)")
    b = bot(r)
    r.exec(f"probe_setprop {b} data m_flDamageAccumulator float 0")
    r.exec("probe_set 100 0 100")
    for _ in range(4):
        r.exec("probe_hurt 0.2 2")
    r.exec("probe_kill")
    time.sleep(0.5)
    probe.wait_alive(r)
    kept = r.exec(f"probe_get {bot(r)} data m_flDamageAccumulator float").split()[-1]
    r.exec("probe_set 100 50 100")
    show("suit P2 remainder across death, then one 1-bullet hit", (kept, r.exec("probe_hurt 1 2").split()[4]),
         "0.8, 99")


def weapons(r):
    probe.wait_alive(r)
    for cls, buttons, want in [("weapon_pistol", ATTACK, "33/34"), ("weapon_357", ATTACK, "50"),
                               ("weapon_smg1", ATTACK, "5 (±1)"), ("weapon_ar2", ATTACK, "7-7-6"),
                               ("weapon_shotgun", ATTACK, "59"), ("weapon_shotgun", ATTACK2, "70")]:
        arm(r, cls)
        out = probe.run_full(r, measure.start(), hold(80, select=cls) + hold(250, buttons))
        show(f"weapons P6 {cls} {buttons} held: shot intervals", diffs(shots(out))[:10], want)
    arm(r, "weapon_pistol")
    ticks = hold(80, select="weapon_pistol") + [(ATTACK if k % 2 == 0 else 0, 0, 0, 0, 180) for k in range(170)]
    out = probe.run_full(r, measure.start(), ticks)
    bul = [t for t in out["te"] if t[1] == "bullets"]
    show("weapons P7 pistol clicks: intervals", diffs(shots(out))[:8], "8")
    show("weapons P1 pistol spread per shot (sin of the half angle)", [b[11] for b in bul][:11],
         "0.00872 rising 0.00538 a click to 0.05234")
    arm(r, "weapon_357")
    out = probe.run_full(r, measure.start(), hold(80, select="weapon_357") + (hold(1, ATTACK) + hold(132)) * 3)
    show("weapons P2 .357 spread", [t[11] for t in out["te"] if t[1] == "bullets"], "0")
    for cls, want in [("weapon_pistol", 96), ("weapon_smg1", 100), ("weapon_ar2", 105), ("weapon_357", 245),
                      ("weapon_shotgun", 34)]:
        arm(r, cls)
        out = probe.run_full(r, measure.start(), hold(80, select=cls) + hold(1, ATTACK) + hold(100) + hold(1, RELOAD)
                             + hold(300))
        got = [t - 182 for t in clip_changes(out) if t > 182][:3]
        show(f"weapons P8 {cls} reload: ticks to the clip change", got, want)
    arm(r, "weapon_pistol", "weapon_smg1")
    for a, b, want in [("weapon_smg1", "weapon_pistol", 54), ("weapon_pistol", "weapon_smg1", 56)]:
        out = probe.run_full(r, measure.start(), hold(80, select=a) + hold(1, 0, select=b) + hold(120, ATTACK))
        show(f"weapons P9 switch to {b}: first shot", [s - 80 for s in shots(out) if s >= 80][:1], want)
    arm(r, "weapon_smg1", "weapon_pistol")
    for wait, want in [(207, 45), (190, 38)]:
        out = probe.run_full(r, measure.start(), hold(60, select="weapon_smg1") + hold(35, ATTACK)
                             + hold(1, select="weapon_pistol") + hold(wait) + hold(1, select="weapon_smg1") + hold(30))
        show(f"weapons P13 SMG1 back after {wait} ticks: clip", out["weapon"][-1][2], want)
    r.exec("probe_cmd drop")
    show("weapons P17 `drop`: SMG1 still held", "weapon_smg1" in r.exec("probe_inv"), True)
    r.exec("probe_kill")
    time.sleep(0.5)
    probe.wait_alive(r)
    inv = r.exec("probe_inv")
    show("weapons P5 spawn ammo (pistol 3, SMG1 4, .357 5, buckshot 7, grenade 10)",
         re.findall(r"ammo (\d+) = (\d+)", inv), "150, 45, 6, 6, 2")


def drops(r):
    for cls in ["weapon_pistol", "weapon_shotgun", "weapon_crowbar", "weapon_physcannon"]:
        probe.wait_alive(r)
        r.exec(f"probe_give {cls}")
        out = probe.run_full(r, measure.start(), hold(40, select=cls))
        pos = [out["rows"][-1][k] for k in "xyz"]
        r.exec("probe_kill")
        time.sleep(0.3)
        near = []
        for line in r.exec(f"probe_find {cls}").splitlines():
            p = line.split()
            if len(p) > 5 and math.dist(map(float, p[3:6]), pos) < 64:
                near.append(p[1])
        show(f"weapons P10 {cls} held at death: dropped", near, "pistol/shotgun yes, crowbar/physcannon no")


def pushes(r):
    """weapons P16 / projectiles P7: a second bot's velocity after hits."""
    probe.setup(r, bots=2)
    time.sleep(1)
    ids = re.findall(r"player (\d+) fake 1", r.exec("probe_info"))
    a, b = ids[0], ids[1]
    r.exec(f"probe_bot {a}")
    r.exec("probe_track clear")
    r.exec(f"probe_track {b}")
    for cls, dist, pitch, want in [("weapon_pistol", 200, 8, "40 (5 per damage)"), ("weapon_357", 200, 3, "375"),
                                   ("weapon_crowbar", 50, 0, "≈125")]:
        r.exec(f"probe_bot {b}")
        r.exec("probe_set 100 0 100")
        r.exec(f"probe_bot {a}")
        r.exec(f"probe_tp {b} {SPOT[0] - dist} {SPOT[1]} {SPOT[2]} 0 0")
        arm(r, cls)
        out = probe.run_full(r, measure.start(pitch=pitch), hold(70, 0, 0, 0, pitch, 180, select=cls)
                             + hold(1, ATTACK, 0, 0, pitch, 180) + hold(6, 0, 0, 0, pitch, 180))
        v = [round(math.hypot(float(e[6]), float(e[7])), 1) for e in out["ents"] if e[1] == b]
        hits = [(x[5], x[6], x[3]) for x in out["traced"] if x[1] == b]
        show(f"weapons P16 {cls} hit: victim speed, (hitbox, hitgroup, damage)", (max(v), hits), want)
    r.exec("probe_track clear")
    r.exec(f"probe_tp {b} {ROOF[0]} {ROOF[1]} {ROOF[2]} 0 0")
    r.exec(f"probe_bot {a}")


def two_bots(r):
    """(driven, victim): two bots, the first driven."""
    probe.setup(r, bots=2)
    time.sleep(1)
    ids = re.findall(r"player (\d+) fake 1", r.exec("probe_info"))
    r.exec(f"probe_bot {ids[0]}")
    return ids[0], ids[1]


def shot_at(r, a, b, cls, pitch, yaw, dist=200, armour=0, wait=30):
    """One `cls` attack from SPOT (yaw 180 + `yaw`) at bot `b` placed
    `dist` ahead facing it (health 5000): its traced hits (damage,
    hitbox, hitgroup) and damage taken (damage, type, health after)."""
    r.exec(f"probe_bot {a}")
    r.exec("probe_set 100 0 100")
    arm(r, cls)
    r.exec(f"probe_tp {b} {SPOT[0] - dist} {SPOT[1]} {SPOT[2]} 0 0")
    r.exec(f"probe_setprop {b} data m_iHealth int 5000")
    r.exec(f"probe_setprop {b} data m_ArmorValue int {armour}")
    out = probe.run_full(r, measure.start(), hold(70, 0, 0, 0, pitch, 180 + yaw, select=cls)
                         + hold(1, ATTACK, 0, 0, pitch, 180 + yaw) + hold(wait, 0, 0, 0, pitch, 180 + yaw))
    mine = lambda rows: [x for x in rows if x[1] == b]
    return [(x[3], x[5], x[6]) for x in mine(out["traced"])], [(x[4], x[5], x[6]) for x in mine(out["damage"])]


def heads(r):
    """weapons P15, projectiles test 39: a stopped bot doesn't animate, its
    head hitbox sits low and to one side: from 200 units, pitch 1.25 and
    yaw +1 hit it (hitbox 0, hitgroup 1); pitch 3 the chest, 8.5 a leg."""
    a, b = two_bots(r)
    for cls, aim, dist, want in [("weapon_pistol", (1.25, 1.0), 200, "head: 8 traced, 24 taken"),
                                 ("weapon_357", (1.25, 1.0), 200, "head: 75 traced, 225 taken"),
                                 ("weapon_crowbar", (4.5, 5.0), 48, "25, no multiplier"),
                                 ("weapon_crossbow", (1.25, 1.0), 200, "spec 300; real: 100, hitgroup 0"),
                                 ("weapon_crossbow", (8.5, 0.0), 200, "legs: 100, hitgroup 0")]:
        show(f"weapons P15 / bolt {cls} at {aim}, {dist}", shot_at(r, a, b, cls, *aim, dist=dist), want)
    r.exec(f"probe_tp {b} {ROOF[0]} {ROOF[1]} {ROOF[2]} 0 0")


def frag_drag(r):
    """projectiles 9.4: a frag thrown from above the roof (deployed on the
    floor first): its velocity fitted over 15-tick windows past its
    spin (its origin circles its mass centre)."""
    r.exec("probe_track clear")
    r.exec("probe_track npc_grenade_frag")
    for buttons, want in [(ATTACK, "1200 forward, 120 up, slowing 0.176 of its speed a second"),
                          (ATTACK2, "350 forward, 50 up, slowing 0.10 a second")]:
        probe.wait_alive(r)
        arm(r, "weapon_frag")
        probe.run_full(r, measure.start(), hold(70, 0, 0, 0, 0, 180, select="weapon_frag"))
        out = probe.run_full(r, (ROOF[0], ROOF[1], ROOF[2] + 400, 0, 180, 0, 0, 0), hold(1, buttons, 0, 0, 0, 180)
                             + hold(80, 0, 0, 0, 0, 180))
        e = entities(out, "npc_grenade_frag")
        row = out["rows"][int(e[0][0])]
        eye = (row["x"], row["y"], row["z"] + row["eye"])
        pts = [[float(x[3 + k]) - eye[k] for k in range(3)] for x in e]
        fits = [round(-window_velocity(pts[s:s + 15])[0]) for s in (0, 15, 30, 45)]
        show(f"frag {buttons} forward speed over 15-tick windows", fits, want)
    r.exec("probe_track clear")


def window_velocity(pts, g=600.0):
    """The mean velocity of points 0.015 s apart, fitting a circle at one
    rate (scanned) on top of a line (and gravity's parabola on z)."""
    ts = [i * 0.015 for i in range(len(pts))]
    best = None
    for k in range(200, 4000, 5):
        w = k * 0.01
        rows = [[1.0, t, math.cos(w * t), math.sin(w * t)] for t in ts]
        sol, res = [], 0.0
        for ax in range(3):
            ys = [p[ax] + (0.5 * g * t * t if ax == 2 else 0.0) for t, p in zip(ts, pts)]
            x, e = least_squares(rows, ys)
            sol.append(x[1])
            res += e
        if best is None or res < best[0]:
            best = (res, sol)
    return best[1]


def least_squares(rows, ys):
    n = len(rows[0])
    m = [[sum(r[i] * r[j] for r in rows) for j in range(n)] + [sum(r[i] * y for r, y in zip(rows, ys))]
         for i in range(n)]
    for c in range(n):
        p = max(range(c, n), key=lambda q: abs(m[q][c]))
        m[c], m[p] = m[p], m[c]
        for q in range(n):
            if q != c and m[c][c]:
                f = m[q][c] / m[c][c]
                m[q] = [x - f * y for x, y in zip(m[q], m[c])]
    x = [m[i][n] / m[i][i] if m[i][i] else 0.0 for i in range(n)]
    return x, sum((sum(a * b for a, b in zip(r, x)) - y) ** 2 for r, y in zip(rows, ys))


def hanging_punt(r, pitch, dist, height, mass=30):
    """A crate hanging motion-disabled (flags 8 + 64) above the roof,
    `dist` ahead and its centre `height` above the bot's feet, punted:
    its velocity and pitch rate (degrees/s) a tick after."""
    probe.wait_alive(r)
    arm(r, "weapon_physcannon")
    r.exec("probe_track clear")
    probe.run_full(r, measure.start(pitch=pitch), hold(30, 0, 0, 0, pitch, 180, select="weapon_physcannon"))
    z = ROOF[2] + 200
    idx = r.exec(f"probe_prop prop_physics {CRATE} {ROOF[0] - dist} {ROOF[1]} {z + height} {mass} 72").split()[-1]
    out = probe.run_full(r, (ROOF[0], ROOF[1], z, pitch, 180, 0, 0, 0), hold(1, ATTACK, 0, 0, pitch, 180)
                         + hold(4, 0, 0, 0, pitch, 180))
    e = [x for x in out["ents"] if x[1] == idx]
    r.exec(f"probe_fire {idx} Kill")
    a, b = e[1], e[2]
    return [round(x) for x in speed(a, b)], round((float(b[12]) - float(a[12])) / 0.015)


def entities(out, cls):
    """The rows of the first `cls` entity created in the run."""
    made = [c[1] for c in out["created"] if c[2] == cls]
    return [e for e in out["ents"] if made and e[1] == made[0]]


def speed(a, b):
    return [(float(b[3 + j]) - float(a[3 + j])) / 0.015 for j in range(3)]


def projectiles(r):
    r.exec("probe_track clear")
    for c in ["npc_grenade_frag", "grenade_ar2", "rpg_missile", "crossbow_bolt"]:
        r.exec(f"probe_track {c}")
    eye = (SPOT[0], SPOT[1], SPOT[2] + 64)
    for cls, buttons, ent, want in [("weapon_frag", ATTACK, "npc_grenade_frag", "eye + (18, -8, 0) (1200, 0, 120)"),
                                    ("weapon_frag", ATTACK2, "npc_grenade_frag", "(350, 0, 50)"),
                                    ("weapon_smg1", ATTACK2, "grenade_ar2", "1000, gravity 400"),
                                    ("weapon_rpg", ATTACK, "rpg_missile", "(300, 0, 128), 1500 from tick 20"),
                                    ("weapon_crossbow", ATTACK, "crossbow_bolt", "3500, gravity 30")]:
        for _ in range(2):
            probe.wait_alive(r)
            arm(r, cls)
            out = probe.run_full(r, measure.start(), hold(70, 0, 0, 0, 0, 180, select=cls)
                                 + hold(1, buttons, 0, 0, 0, 180) + hold(260, 0, 0, 0, 0, 180))
            e = entities(out, ent)
            if len(e) >= 3:
                break
        if len(e) < 3:
            show(f"projectiles {cls} {buttons}", "no projectile", want)
            continue
        rel = [round(float(e[0][3 + k]) - eye[k], 1) for k in range(3)]
        # Toss projectiles stay put on their first row; the frag moves.
        k = 0 if e[0][3:6] != e[1][3:6] else 1
        v = [round(x, 1) for x in speed(e[k], e[k + 1])]
        sp = [round(math.sqrt(sum(x * x for x in speed(a, b)))) for a, b in zip(e[k:], e[k + 1:])]
        jump = next((i + 1 + k for i, (a, b) in enumerate(zip(sp, sp[1:])) if b - a > 500), None)
        sounds = [int(s[0]) - int(e[0][0]) + 1 for s in out["ent_sounds"] if s[1] == e[0][1] and "tick" in s[6]]
        show(f"projectiles {cls} {buttons}: start rel eye, first velocity, ignition, blips, life",
             (rel, v, jump, sounds, len(e)), want)
    arm(r, "weapon_crossbow")
    out = probe.run_full(r, measure.start(), hold(70, 0, 0, 0, 0, 180, select="weapon_crossbow")
                         + hold(400, ATTACK, 0, 0, 0, 180))
    show("projectiles P9 crossbow held: ticks between bolts", diffs([int(c[0]) for c in out["created"]
                                                                     if c[2] == "crossbow_bolt"]),
         "168 (spec); 130 (Chimera, from this server)")
    r.exec("probe_track clear")


def punt(r, mass, pitch=0.0, dist=80, height=64, launch=False):
    probe.wait_alive(r)
    arm(r, "weapon_physcannon")
    r.exec("probe_track clear")
    probe.run_full(r, measure.start(pitch=pitch), hold(30, 0, 0, 0, pitch, 180, select="weapon_physcannon"))
    idx = r.exec(f"probe_prop prop_physics {CRATE} {SPOT[0] - dist} {SPOT[1]} {SPOT[2] + height} {mass}").split()[-1]
    pre = hold(70, ATTACK2, 0, 0, pitch, 180) if launch else []
    out = probe.run_full(r, measure.start(pitch=pitch), pre + hold(1, ATTACK, 0, 0, pitch, 180)
                         + hold(8, 0, 0, 0, pitch, 180))
    e = [x for x in out["ents"] if x[1] == idx]
    held = round(float(e[len(pre) - 1][3]) - SPOT[0], 1) if launch else None
    v = [speed(a, b) for a, b in zip(e[len(pre):], e[len(pre) + 1:])]
    first = next(x for x in v if math.hypot(x[0], x[1]) > 300)
    r.exec(f"probe_fire {idx} Kill")
    return round(math.hypot(first[0], first[1])), round(first[2]), held


def gravity(r):
    show("gravity P2 level punts 10/30/100/250 kg (first tick, after a tick of drag)",
         [punt(r, m)[0] for m in (10, 30, 100, 250)], "2000, 1100, 750, 660")
    show("gravity P3 punt looking 45 down, 30 kg (horizontal, up)", punt(r, 30, pitch=45, dist=40, height=24)[:2],
         "up ≈ 506")
    show("gravity P3 hanging crate, 45 down (velocity, pitch rate)", hanging_punt(r, 45, 40, 24),
         "(-778, 0, 506) less a tick of drag and gravity")
    show("gravity P3 hanging crate, 21.8 down at its face", hanging_punt(r, 21.8, 100, 24),
         "(-1007, 0, 253); pitching 2964/s (the lever to the box's centre where it stopped)")
    for rz in (5, 10, 15):
        show(f"gravity P3 level punt {rz} above a hanging crate's centre: pitch rate",
             hanging_punt(r, 0, 100, 64 - rz)[1], f"{176 * rz}/s (inertia 0.714 of a solid box's)")
    for m, want in [(30, "1500; held ≈ 66.6 ahead"), (250, "≈ 1327")]:
        h, _, held = punt(r, m, launch=True)
        show(f"gravity P4/P5 launch held {m} kg (speed, held x offset)", (h, held), want)


def pickups(r):
    """dm_lockdown's kit 489 and shotgun 391 by position (indices differ
    by load: found by class and spot)."""
    def find(cls, x, y):
        for line in r.exec(f"probe_find {cls}").splitlines():
            p = line.split()
            if len(p) > 5 and math.hypot(float(p[3]) - x, float(p[4]) - y) < 32:
                return p[0]
    def drawn(index):
        deadline = time.time() + 40
        while time.time() < deadline and "= 0" not in r.exec(f"probe_get {index} send m_fEffects"):
            time.sleep(0.5)
    probe.wait_alive(r)
    kit = find("item_healthkit", -2839.33, 2204.99)
    drawn(kit)
    r.exec("probe_set 50 0 100")
    out = probe.run_full(r, (-2839.33, 2204.99, 8.0, 0, 0, 0, 0, 0), hold(2100))
    h = [(int(x["tick"]), x["health"]) for x in out["rows"]]
    show(f"maps P1 kit {kit} taken on row 0, taken again on row", [b for a, b in zip(h, h[1:]) if a[1] != b[1]],
         "2000 (+4-5: the tick base)")
    gun = find("weapon_shotgun", -3487.13, 2056.31)
    drawn(gun)
    arm(r, "weapon_crowbar")
    r.exec("probe_track clear")
    r.exec("probe_track weapon_shotgun")
    out = probe.run_full(r, (-3487.13, 2056.31, 8.0, 0, 0, 0, 0, 0), hold(1400))
    copies = [e for e in out["ents"] if e[1] != gun and e[2] == "weapon_shotgun"]
    shown = next((int(e[0]) for e in copies if not int(e[11]) & 32), None)
    show(f"maps P2 shotgun {gun} taken on row 0: copy drawn on row", shown, "1333 (+4-5: the tick base)")
    r.exec("probe_track clear")


def charger(r, pos, yaw, target, n, setup, column):
    probe.wait_alive(r)
    r.exec(setup)
    dz = target[2] - (pos[2] + 64)
    pitch = -math.degrees(math.atan2(dz, math.hypot(target[0] - pos[0], target[1] - pos[1])))
    out = probe.run_full(r, (pos[0], pos[1], pos[2], pitch, yaw, 0, 0, 0), hold(20, 0, 0, 0, pitch, yaw)
                         + hold(n, USE, 0, 0, pitch, yaw))
    vals = [(int(x["tick"]), x[column], x["health"]) for x in out["rows"]]
    ch = [b for a, b in zip(vals, vals[1:]) if a[1:] != b[1:]]
    return len(ch), sorted(set(diffs([c[0] for c in ch]))), ch[:2], vals[-1]


def chargers(r):
    show("suit P10 suit charger (charges, steps, first, last)",
         charger(r, (-3423.26, 1816.0, 8.0), 90.0, (-3423.26, 1856.0, 46.76), 300, "probe_set 100 0 100", "armor"),
         "pool sk_suitcharger (75 in the spec; this server: 30), every 7 ticks")
    show("suit P10 health charger", charger(r, (-2768.0, 2993.0, 136.0), 180.0, (-2808.0, 2993.0, 179.85), 500,
                                            "probe_set 1 0 100", "health"), "pool 50, every 7 ticks")
    show("suit P11 megacharger (flag 8192)", charger(r, (-3256.0, 4760.0, 136.0), 90.0, (-3256.0, 4800.0, 183.0), 720,
                                                     "probe_set 50 0 100", "armor"),
         "pool 200, +2 armour +5 health a charge, armour to 200")


# dm_underpass: water from about -697 to -510 here.
WATER = (-1728.0, -2240.0, -640.0)


def swim(r):
    st = (WATER[0], WATER[1], WATER[2], 0, 0, 0, 0, 0)
    show("movement row 61 sink vz 1, 5, 9", [round(x["vz"], 3) for x in probe.run(r, st, hold(10))[0][:9:4]],
         "-7.2, -31.931, -48")
    show("movement row 59 jump vz", [round(x["vz"], 3) for x in probe.run(r, st, hold(4, JUMP))[0][1:3]],
         "116.8, 104.2")
    rows = probe.run(r, st, hold(4, FORWARD, 450, 0, 0, 0))[0]
    show("movement row 60 forward speed 1-3", [round(math.hypot(x["vx"], x["vy"]), 3) for x in rows[1:4]],
         "22.8, 44.232, 64.378")


def drown(r):
    probe.wait_alive(r)
    r.exec("probe_set 100 0 100")
    # A jump tap every 200 ticks keeps the bot under without reaching the
    # floor (whose bottom leaves the feet out of the water: level 0).
    ticks = [(JUMP if k % 200 == 0 else 0, 0, 0, 0, 0) for k in range(2400)]
    out = probe.run_full(r, (WATER[0], WATER[1], WATER[2] - 20, 0, 0, 0, 0, 0), ticks)
    rows = out["rows"]
    a0 = next(i for i, x in enumerate(rows) if x["aux"] <= 0.0)
    hp = [(int(x["tick"]), x["health"]) for x in rows]
    ch = [b[0] for a, b in zip(hp, hp[1:]) if a[1] != b[1]]
    show("suit P6 aux per tick, aux 0 (s), hits (s)", (round(rows[1]["aux"] - rows[2]["aux"], 4), a0 * 0.015,
         [round(t * 0.015, 3) for t in ch[:4]]), "0.1005, ≈14.9, 21.9 then every 1.005")


def change_map(r, name, teamplay):
    """`changelevel` with `mp_teamplay` set, then the probe set up again.
    `hl2mp_bot_join_after_player 0` keeps the bots in TDM (with its
    default 1 the bot manager kicks every bot while no human plays, which
    looked like TDM kicking them)."""
    r.exec(f"mp_teamplay {teamplay}")
    r.exec("hl2mp_bot_join_after_player 0")
    r.exec(f"changelevel {name}")
    time.sleep(12)
    probe.setup(r, bots=0)


def tdm_bots(r):
    """Three bots: 1 and 2 Combine, 3 Rebels (`hl2mp_bot_add <team>`;
    a bot that first joined the other team moves, which costs it a death
    charged to its new team). Returns their client numbers."""
    r.exec("hl2mp_bot_kick all")
    time.sleep(2)
    for team in ("combine", "combine", "rebels"):
        r.exec(f"hl2mp_bot_add {team}")
        time.sleep(1.5)
    time.sleep(6)
    players, _ = probe.score(r)
    combine = sorted(c for c, p in players.items() if p[0] == 2)
    rebels = sorted(c for c, p in players.items() if p[0] == 3)
    return combine[0], combine[1], rebels[0]


def wait_bot_alive(r, bot, timeout=12.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if probe.score(r)[0].get(bot, (0, 0, 0, 1))[3] == 0:
            return True
        time.sleep(0.25)
    return False


def timeline(r, bot, ticks, cmds):
    """`cmds` (row, command; "{other}" and "{same}": the teams other than
    and of the bot's at the start) run by `bot` while it runs `ticks`
    (alive and 5 s past its last death first): its deaths, spawns and
    team changes by row, and the score's change (frags, deaths; team
    points). The bot manager moves a bot to another team on some of its
    spawns (a team change of its own, not the rules'): rows after a spawn
    the case didn't cause aren't the rules'."""
    for _ in range(4):
        wait_bot_alive(r, bot)
        time.sleep(5.2)
        a = probe.score(r)
        if a[0][bot][3] == 0:
            break
    r.exec(f"probe_bot {bot}")
    same = a[0][bot][0]
    other = 5 - same
    cmds = [(row, bot, c.format(other=other, same=same)) for row, c in cmds]
    out = probe.run_full(r, None, ticks, cmds=cmds)
    b = probe.score(r)
    events = [(int(v[0]), v[1], v[3:]) for v in out["events"] if v[2] == str(bot)]
    frags = tuple(y - x for x, y in zip(a[0][bot][1:3], b[0][bot][1:3]))
    teams = {t: b[1][t] - a[1][t] for t in (2, 3)}
    return same, events, frags, teams


def tdm(r):
    """rules.md P5, P7, P8, tests 10-13, friendly fire (rules.md "Friendly
    fire", tests 50) on dm_lockdown in TDM; back to DM after."""
    change_map(r, "dm_lockdown", 1)
    c1, c2, rb = tdm_bots(r)
    show("rules TDM teams after the adds (team, frags, deaths, life)", probe.score(r), "1, 2 on 2; 3 on 3")
    show("rules P5/test 10: (start team, events, frags/deaths, team points) alive, jointeam the other team, "
         "then (dead) back 1 s later, then the same team",
         timeline(r, c1, hold(300), [(10, "jointeam {other}"), (77, "jointeam {same}"), (144, "jointeam {same}")]),
         "death on 10 charged to the new team (frag -1); the dead change free; same team nothing")
    show("rules test 11: jointeam twice 0.15 s apart",
         timeline(r, c1, hold(60), [(10, "jointeam {other}"), (20, "jointeam {same}")]), "the second ignored (0.3 s)")
    r.exec("mp_forcerespawn 0")
    # With forced respawn off the bot may already be dead here (a death
    # the bot manager's own team change caused): then the kill is nothing
    # and the change on 410 is a long-dead player's.
    show("rules: kill, then dead 6 s later change team (press at 420)",
         timeline(r, c1, hold(420) + hold(1, ATTACK) + hold(30), [(10, "kill"), (410, "jointeam {other}")]),
         "no death on 410 (a dead player's change is free)")
    show("rules test 12: kill, button respawn at 3.08 s, change team at 3.9 s; back at 5.5 s",
         timeline(r, c1, hold(215) + hold(1, ATTACK) + hold(300),
                  [(10, "kill"), (270, "jointeam {other}"), (380, "jointeam {same}")]),
         "alive at 270 (within 5 s of the suicide); death at 380, the new team -1")
    r.exec("mp_forcerespawn 1")
    show("rules test 13: spectate alive", timeline(r, c2, hold(30), [(10, "spectate")]),
         "death, frag given back (0, +1), old team -1")
    for ff in (0, 1):
        r.exec(f"mp_friendlyfire {ff}")
        for victim, other in ((c2, rb), (rb, c2)):
            wait_bot_alive(r, victim)
            r.exec(f"probe_tp {other} {ROOF[0]} {ROOF[1]} {ROOF[2]} 0 0")
            show(f"friendly fire {ff}: pistol {c1} at {victim} (traced, taken)",
                 shot_at(r, str(c1), str(victim), "weapon_pistol", 3, 0), "a teammate with ff 0 takes nothing")
    for ff, victim, want in [(1, c2, "killer -1 frag, its team +1"), (1, rb, "killer +1, its team +1"),
                             (0, c2, "refused")]:
        r.exec(f"mp_friendlyfire {ff}")
        wait_bot_alive(r, victim)
        r.exec(f"probe_bot {victim}")
        r.exec("probe_set 100 0 100")
        a = probe.score(r)
        r.exec(f"probe_hurt 500 2 {c1}")
        time.sleep(0.2)
        b = probe.score(r)
        show(f"rules P7: ff {ff}, {c1} kills {victim} (frags, deaths of each; team points)",
             ({c: tuple(y - x for x, y in zip(a[0][c][1:3], b[0][c][1:3])) for c in (c1, victim)},
              {t: b[1][t] - a[1][t] for t in (0, 2, 3)}), want)
    wait_bot_alive(r, c2)
    r.exec(f"probe_bot {c2}")
    r.exec("probe_set 100 0 100")
    a = probe.score(r)
    r.exec("probe_hurt 500 32")
    time.sleep(0.2)
    b = probe.score(r)
    show("rules: a world kill (fall) in TDM: team points", {t: b[1][t] - a[1][t] for t in (0, 2, 3)},
         "team 0 +1 (the world's: not shown), 2 and 3 unchanged")
    r.exec("mp_friendlyfire 0")
    r.exec(f"probe_bot {c1}")
    change_map(r, "dm_lockdown", 0)


def tdm_spawns(r, rounds=12):
    """rules.md "Spawn-point choice": TDM on dm_runoff (8 Combine, 6 Rebel
    spots): each spawn's nearest spot class by team; back to DM after."""
    change_map(r, "dm_runoff", 1)
    tdm_bots(r)
    spots = {}
    for cls in ("info_player_combine", "info_player_rebel", "info_player_deathmatch"):
        for line in r.exec(f"probe_find {cls}").splitlines():
            p = line.split()
            if len(p) >= 6:
                spots[(cls[12:], int(p[0]))] = tuple(float(v) for v in p[3:6])
    teams = {c: v[0] for c, v in probe.score(r)[0].items()}
    seen = {}
    for _ in range(rounds):
        for c in teams:
            r.exec(f"probe_bot {c}")
            r.exec("probe_kill")
        time.sleep(5.6)
        for line in r.exec("probe_info").splitlines():
            m = re.match(r"player (\d+) fake 1 at (\S+) (\S+) (\S+)", line)
            if m:
                pos = [float(m.group(k)) for k in (2, 3, 4)]
                k = min(spots, key=lambda k: sum((a - b) ** 2 for a, b in zip(spots[k], pos)))
                key = (teams[int(m.group(1))], k[0])
                seen[key] = seen.get(key, 0) + 1
    show("rules TDM spawns on dm_runoff: (team, nearest spot class): count", seen, "2 combine, 3 rebel only")
    change_map(r, "dm_lockdown", 0)


def drop_on(r, a, b, mass, height):
    """A crate let fall from `height` above bot `b`'s head (5000 health) on
    the roof: its crush hits (tick, amount), its step (units/s) on the
    ticks around the first and its centre's height above the feet the
    tick before."""
    probe.wait_alive(r)
    r.exec(f"probe_bot {a}")
    r.exec("probe_track clear")
    r.exec(f"probe_tp {b} {ROOF[0]} {ROOF[1]} {ROOF[2]} 0 0")
    r.exec(f"probe_setprop {b} data m_iHealth int 5000")
    r.exec(f"probe_setprop {b} data m_ArmorValue int 0")
    idx = r.exec(f"probe_prop prop_physics {CRATE} {ROOF[0]} {ROOF[1]} {ROOF[2] + 92 + height} {mass}").split()[-1]
    out = probe.run_full(r, measure.start(), hold(int(100 + 3 * math.sqrt(height))))
    z = {int(x[0]): float(x[5]) for x in out["ents"] if x[1] == idx}
    hits = [(int(x[0]), round(float(x[4]))) for x in out["damage"] if x[1] == b and x[5] == "1"]
    r.exec(f"probe_fire {idx} Kill")
    if not hits:
        return hits, [], None
    t = hits[0][0]
    steps = [round((z[k] - z[k - 1]) / 0.015) for k in range(t - 2, t + 3) if k in z and k - 1 in z]
    return hits, steps, round(z[t - 1] - ROOF[2], 1)


def punt_into(r, a, b, mass, dist=200, ahead=50, cls="prop_physics"):
    """A crate (`cls`) hanging at eye height `ahead` of bot `a`, punted level into
    bot `b` `dist` ahead: b's crush hits (tick, amount, attacker) and the
    crate's step at the hit."""
    probe.wait_alive(r)
    r.exec(f"probe_bot {a}")
    arm(r, "weapon_physcannon")
    r.exec("probe_track clear")
    probe.run_full(r, measure.start(), hold(30, 0, 0, 0, 0, 180, select="weapon_physcannon"))
    r.exec(f"probe_tp {b} {SPOT[0] - dist} {SPOT[1]} {SPOT[2]} 0 0")
    r.exec(f"probe_setprop {b} data m_iHealth int 5000")
    r.exec(f"probe_setprop {b} data m_ArmorValue int 0")
    idx = r.exec(f"probe_prop {cls} {CRATE} {SPOT[0] - ahead} {SPOT[1]} {SPOT[2] + 64} {mass} 72").split()[-1]
    out = probe.run_full(r, measure.start(), hold(1, ATTACK, 0, 0, 0, 180) + hold(60, 0, 0, 0, 0, 180))
    e = {int(x[0]): x for x in out["ents"] if x[1] == idx}
    hits = [(int(x[0]), round(float(x[4])), x[2]) for x in out["damage"] if x[1] == b]
    r.exec(f"probe_fire {idx} Kill")
    t = hits[0][0] if hits else None
    step = [round(-x) for x in speed(e[t - 1], e[t])[:1]] if t and t in e and t - 1 in e else []
    return hits, step


def crate_hits(r):
    """gravity_gun.md P10, 9: crates dropped on a bot's head and punted
    into one."""
    a, b = two_bots(r)
    for mass, h, want in [(30, 75, "5 + 10"), (30, 150, "5 + 10"), (30, 300, "10 + 20 (spec test 39: 500)"),
                          (30, 600, "10 + 50"), (10, 300, "10"), (100, 300, "20 + 100, a 5 later"),
                          (100, 600, "100 + 500"), (250, 300, "100 + 500")]:
        show(f"gravity P10 {mass} kg dropped {h} onto a head: hits (tick, amount), steps t-2..t+2, centre z at t-1",
             drop_on(r, a, b, mass, h), want)
    for m, want in [(10, "20 + 10"), (30, "20 + 10 + 5"), (100, "50 + 10 + 10, a 5 later"),
                    (250, "100 + 5 + 10, a 20 later")]:
        show(f"gravity P10 {m} kg punted into a bot 150 ahead: hits (tick, amount, attacker), forward step",
             punt_into(r, a, b, m), want)
    r.exec(f"probe_tp {b} {ROOF[0]} {ROOF[1]} {ROOF[2] + 100} 0 0")


def floor_punt(r, mass, pitch, dist, settle=60):
    """A crate settled on the floor `dist` ahead, punted looking `pitch`
    down: its steps (forward, side, up) and pitch rates on the ticks
    after."""
    probe.wait_alive(r)
    arm(r, "weapon_physcannon")
    r.exec("probe_track clear")
    probe.run_full(r, measure.start(pitch=pitch), hold(30, 0, 0, 0, pitch, 180, select="weapon_physcannon"))
    idx = r.exec(f"probe_prop prop_physics {CRATE} {SPOT[0] - dist} {SPOT[1]} {SPOT[2] + 22} {mass}").split()[-1]
    out = probe.run_full(r, measure.start(pitch=pitch), hold(settle, 0, 0, 0, pitch, 180)
                         + hold(1, ATTACK, 0, 0, pitch, 180) + hold(8, 0, 0, 0, pitch, 180))
    e = [x for x in out["ents"] if x[1] == idx]
    r.exec(f"probe_fire {idx} Kill")
    rows = []
    for a, b in zip(e[settle:settle + 4], e[settle + 1:settle + 5]):
        v = speed(a, b)
        rows.append(([round(-v[0]), round(v[1]), round(v[2])], round((float(b[12]) - float(a[12])) / 0.015)))
    return rows


def floor_punts(r):
    """gravity_gun.md P3 on the floor: the 30 kg crate resting 40 ahead."""
    show("gravity P3 floor, 45 down, 40 ahead: (step, pitch rate) per tick", floor_punt(r, 30, 45, 40),
         "(773, 0, 898), then (695, 20, 871); Chimera (773, 728) then (761, 792)")
    show("gravity P3 floor, 30 down, 60 ahead", floor_punt(r, 30, 30, 60), "-")


def carry(r, mass, after, pitch=40):
    """`hl2mp_allow_pickup 1`: +use at a crate on the floor 45 ahead
    (looking `pitch` down), then level for 40 ticks, then `after`: the
    crate's centre relative to the eye and its step by row, the sounds,
    the weapon rows."""
    probe.wait_alive(r)
    arm(r, "weapon_physcannon", "weapon_smg1")
    r.exec("probe_track clear")
    probe.run_full(r, measure.start(pitch=pitch), hold(40, 0, 0, 0, pitch, 180, select="weapon_smg1"))
    idx = r.exec(f"probe_prop prop_physics {CRATE} {SPOT[0] - 45} {SPOT[1]} {SPOT[2] + 22} {mass}").split()[-1]
    out = probe.run_full(r, measure.start(pitch=pitch), hold(40, 0, 0, 0, pitch, 180) + hold(1, USE, 0, 0, pitch, 180)
                         + hold(40, 0, 0, 0, 0, 180) + after)
    r.exec(f"probe_fire {idx} Kill")
    e = [x for x in out["ents"] if x[1] == idx]
    rows = out["rows"]
    at = {}
    for t in range(1, min(len(e), len(rows))):
        eye = (rows[t]["x"], rows[t]["y"], rows[t]["z"] + rows[t]["eye"])
        at[t] = ([round(-(float(e[t][3]) - eye[0]), 1), round(float(e[t][4]) - eye[1], 1),
                  round(float(e[t][5]) - eye[2], 1)], [round(-x) if k == 0 else round(x)
                                                       for k, x in enumerate(speed(e[t - 1], e[t]))],
                 round(math.hypot(rows[t]["vx"], rows[t]["vy"])))
    return at, [(int(x[0]), x[5]) for x in out["sounds"]], out["weapon"]


def use_carry(r):
    """gravity_gun.md 8, P9: `+use` at props with `hl2mp_allow_pickup` 0
    and 1."""
    r.exec("hl2mp_allow_pickup 0")
    at, sounds, _ = carry(r, 20, [])
    show("gravity P9 allow 0, 20 kg: crate 20 ticks after +use (rel eye), sounds", (at[60][0], sounds),
         "unmoved, common/wpn_select.wav")
    r.exec("hl2mp_allow_pickup 1")
    at, sounds, _ = carry(r, 40, [])
    show("gravity 8 allow 1, 40 kg", (at[60][0], sounds), "unmoved, common/wpn_denyselect.wav")
    at, sounds, weapon = carry(r, 20, hold(1, ATTACK, 0, 0, 0, 180) + hold(10, 0, 0, 0, 0, 180))
    show("gravity 8 allow 1, 20 kg: carried (rel eye on rows 42, 45, 80), sounds", ([at[t][0] for t in (42, 45, 80)],
         sounds), "66.5 ahead at eye height (24 + 22.6 + its 20 depth), one click")
    show("gravity 8 throw 20 kg (steps on the rows after the press)", [at[t][1] for t in range(82, 86)], "200 along f")
    show("gravity 8 the weapon's next attack around the throw", [(w[0], w[5]) for w in weapon if 80 <= int(w[0]) <= 84],
         "deployed again at the throw (SMG1 0.8333)")
    for m, want in [(10, "279.3 less a tick of drag"), (1, "620.7 less drag")]:
        at, _, _ = carry(r, m, hold(1, ATTACK, 0, 0, 0, 180) + hold(10, 0, 0, 0, 0, 180))
        show(f"gravity 8 throw {m} kg", [at[t][1] for t in range(82, 86)], want)
    at, _, _ = carry(r, 20, hold(40, 0, 0, 0, -30, 180) + hold(40, 0, 0, 0, 30, 180))
    show("gravity 8 carried looking up 30, down 30 (rel eye)", (at[110][0], at[150][0]),
         "(63.9, 0, 37.0), (63.8, 0, -36.8): the hold along the view, the crate level (yaw-only frame)")
    for name, after in [("+use again", hold(1, USE, 0, 0, 0, 180)), ("secondary", hold(1, ATTACK2, 0, 0, 0, 180))]:
        at, _, _ = carry(r, 20, after + hold(30, 0, 0, 0, 0, 180))
        show(f"gravity 8 drop by {name}: steps after", [at[t][1] for t in range(82, 86)], "falls from rest")
    at, _, _ = carry(r, 20, hold(60, probe.SPEED | FORWARD, 450, 0, 0, 180))
    show("gravity 8 sprint held while carrying: player speed", at[130][2], "190 (sprint off)")
    r.exec("hl2mp_allow_pickup 0")


def cvars(r):
    names = ("sk_player_head sk_plr_dmg_pistol mp_weaponstay sv_hl2mp_weapon_respawn_time sv_hl2mp_item_respawn_time "
             "sk_auto_reload_time phys_pushscale sk_healthkit sk_healthvial sk_battery sk_suitcharger "
             "sk_suitcharger_citadel sk_suitcharger_citadel_maxarmor sk_healthcharger mp_flashlight player_old_armor "
             "sv_stickysprint sk_weapon_ar2_alt_fire_duration sk_weapon_ar2_alt_fire_radius "
             "sk_weapon_ar2_alt_fire_mass sk_combine_ball_search_radius old_radiusdamage sk_satchel_radius "
             "physcannon_tracelength physcannon_maxmass physcannon_minforce physcannon_maxforce physcannon_pullforce "
             "physcannon_cone physcannon_ball_cone physcannon_chargetime player_throwforce hl2mp_allow_pickup "
             "phys_impactforcescale sv_turbophysics breakable_multiplayer g_ragdoll_maxcount "
             "sk_env_headcrabcanister_shake_amplitude sk_env_headcrabcanister_shake_radius "
             "sv_clockcorrection_msecs").split()
    for n in names:
        m = re.search(r'"%s" = "([^"]*)"' % n, r.exec(n))
        print(f"{n} = {m.group(1) if m else '?'}")


CASES = {"respawn": respawn, "armour": armour, "weapons": weapons, "drops": drops, "pushes": pushes,
         "projectiles": projectiles, "gravity": gravity, "heads": heads, "frag_drag": frag_drag, "pickups": pickups, "chargers": chargers, "cvars": cvars,
         "swim": swim, "drown": drown, "tdm": tdm, "tdm_spawns": tdm_spawns, "crate_hits": crate_hits,
         "floor_punts": floor_punts, "use_carry": use_carry}
LOCKDOWN = ["cvars", "respawn", "armour", "weapons", "drops", "projectiles", "frag_drag", "gravity", "pickups",
            "chargers", "pushes", "heads"]

if __name__ == "__main__":
    r = probe.connect()
    probe.setup(r)
    for name in sys.argv[1:] or LOCKDOWN:
        CASES[name](r)
