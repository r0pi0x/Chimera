"""The HL2:DM probe's cases beyond movement (measure.py): respawn
(specs/hl2dm/rules.md P2-P4), armour (suit.md P1-P2, tests 1-9), weapon
timings and spread (weapons.md P1, P2, P5-P10, P13, P16, P17),
projectiles (projectiles.md P1-P4, P9), the gravity gun's punts and
launches (gravity_gun.md P2-P5), pickups (maps_entities.md P1, P2),
chargers (suit.md P10, P11), swimming and drowning (movement.md rows
59-61, suit.md P6) and the cvars every spec lists. Prints each case's
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
         "projectiles": projectiles, "gravity": gravity, "pickups": pickups, "chargers": chargers, "cvars": cvars,
         "swim": swim, "drown": drown}
LOCKDOWN = ["cvars", "respawn", "armour", "weapons", "drops", "projectiles", "gravity", "pickups", "chargers",
            "pushes"]

if __name__ == "__main__":
    r = probe.connect()
    probe.setup(r)
    for name in sys.argv[1:] or LOCKDOWN:
        CASES[name](r)
