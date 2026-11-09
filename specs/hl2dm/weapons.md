# Half-Life 2: Deathmatch: weapons, inventory, bullets and melee (H3)

Source basis: the public Source SDK 2013 multiplayer game code — the shared
weapon base (frame, timers, deploy, holster, reload, empty handling), the
HL2:DM weapon bases (combat weapon, machine gun, bludgeon) and the pistol,
.357, SMG1, AR2, shotgun, crowbar and stunstick, the shared bullet routine
and spread helper, the HL2:DM player (bullet wrapper, loadout, pickup,
drop, lag-compensation filter), the HL2 player's ammo and damage paths,
the base player's damage and armour path, view punch and its decay, the
multiplayer and HL2:DM rules (ammo table, weapon choice, respawn, weapon
stay, object relocation), the weapon and item respawn code, the client's
weapon-selection HUD logic and the HL2:DM bullet temp entity — read in
place in a separate spec session. From the game's files on this box
(game build 24600434, dedicated server build 24600426): the plain weapon
scripts in `hl2mp/hl2mp_pak_dir.vpk`, `hl2mp/cfg/*.cfg`, and the sequence
lengths of the view models (`hl2/hl2_misc_dir.vpk`, `hl2mp`'s VPK for the
stunstick, physcannon and S.L.A.M.), read for numbers only. Nothing was
launched.
Status: draft

## Summary

HL2:DM's weapons run on the same per-command weapon frame as CS:S
(specs/cs_source/weapons.md §2–3: three timers, view-model sequence
lengths for draw and reload, lag compensation, seeded spread). What
differs is almost everything above that frame: a shot's damage is the
script's whole-number `damage` key **against every target**, scaled only by
the hitgroup cvars (head 3); no penetration, no range falloff, no
per-weapon armour ratio (the suit takes 80 %); spread is a fixed cone per
weapon except the pistol's 1°–6° accuracy ramp; the punch angle moves the
shot as well as the view; the machine guns kick harder the longer the
trigger is held; the shotgun is a pump with shell-by-shell reloading and a
12-pellet double shot; melee is a 75-unit line with a 32-unit box fallback
that only counts when the target's **origin** is within 45° of the view.
Inventory is by bucket and weight; weapons and items on the map respawn
after 20 s and 30 s; a dead player drops only the weapon in hand, and
melee weapons and the gravity gun are never dropped.

## Units and conventions

- Source units (inches), Z up, X forward, Y left; degrees; pitch positive
  looks **down**. Time in seconds; one server tick = 0.015 s.
- Every timer is an absolute game time compared with the current time; an
  action due at time T happens on the first tick whose time is ≥ T. A
  delay of d seconds therefore lands ⌈d / 0.015⌉ ticks later; where
  d / 0.015 is a whole number the float rounding decides between n and
  n + 1 ticks (Open questions 3). Tick counts below use ⌈d / 0.015⌉.
- "Same as CS:S §x" points into specs/cs_source/weapons.md.
- Impulse in kg·in/s, as specs/cs_source/physics_props.md.
- Steam builds read: game 24600434, dedicated server 24600426.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tick | 0.015 | s | server tick |
| shot_length | 56755.84 | in | every bullet weapon's trace length |
| spread_1deg / 3 / 5 / 6 / 10 | 0.00873 / 0.02618 / 0.04362 / 0.05234 / 0.08716 | – | spread vector components (same on both axes) |
| pistol_refire_held | 0.5 | s | accumulating fire interval with attack held |
| pistol_click_unlock | 0.1 | s | after a shot, releasing attack for longer than this allows the next press to fire at once |
| pistol_dry_unlock | 0.2 | s | same, after a dry fire |
| pistol_penalty_per_shot | 0.2 | s | accuracy penalty added per shot |
| pistol_penalty_max | 1.5 | s | penalty at which the cone reaches 6° |
| pistol_shot_window | 0.5 | s | shots closer than this count as a series (animation choice only) |
| magnum_refire | 0.75 | s | .357, fixed |
| smg1_refire | 0.075 | s | accumulating |
| ar2_refire | 0.1 | s | accumulating |
| smg1_kick_max / slide | 1 / 2 | deg / s | machine-gun kick parameters |
| ar2_kick_max / slide | 8 / 5 | deg / s | |
| kick_min (pitch, yaw, roll) | 0.2, 0.2, 0.1 | deg | machine-gun kick floor |
| kick_clip (pitch, yaw, roll) | 24, 3, 1 | deg | punch angle limits after a machine-gun kick |
| kick_reset_tolerance | 10 | deg, deg/s | punch reset before a machine-gun kick when √(|punch|² + |punch vel|²) ≤ this |
| shotgun_pellets / double | 7 / 12 | – | |
| pellet_box | ±3 | in | odd-numbered pellets of a multi-bullet shot are box traces |
| melee_range_crowbar / stunstick | 75 / 75 | in | |
| melee_refire_crowbar / stunstick | 0.4 / 0.8 | s | |
| melee_damage_crowbar / stunstick | 25 / 40 | hp | |
| melee_hull | ±16 | in | fallback box |
| melee_hull_pullback | 27.712 | in | 1.732 × 16; the box sweep ends this much short of the range |
| melee_facing_cos | 0.70721 | – | box hit counts only if target origin direction · view ≥ this |
| melee_force_per_hp | 300 | kg·in/s per hp | 75 kg × 4 in/s |
| holster_autoreload | 3 | s | `sk_auto_reload_time` |
| empty_click_interval | 0.5 | s | generic |
| underwater_refusal | 0.2 | s | generic |
| autoswitch_hold | 0.3 | s | old weapon's next primary after an empty auto-switch |
| drop_offset | −12 | in (Z) | from the eye |
| drop_speed_cap | 400 | in/s | |
| drop_spin | (200, 200, 200) | – | angular impulse of a dropped weapon |
| drop_touchable_after | 1.0 | s | |
| drop_removed_after | 30 | s | after becoming touchable (31 s after the drop) |
| weapon_respawn | 20 | s | `sv_hl2mp_weapon_respawn_time` |
| item_respawn | 30 | s | `sv_hl2mp_item_respawn_time` |
| relocate_distance | 64 | in | placed weapon/item farther than this from its spawn point is sent back |
| entity_margin | 100 | entities | "limit in world" weapons wait while fewer than this many entity slots are free |
| player_physics_mass | 85 | kg | divides the death force for the dropped weapon |
| punch_damping / spring | 9 / 65 | 1/s, 1/s² | same as CS:S (shared spring) |
| punch_impulse_scale | 20 | (deg/s) per deg | same as CS:S |

## Behavior

### 1. Weapon data and inventory

**Scripts.** `scripts/weapon_<name>.txt` from `hl2mp/`'s VPK, as in
source_deltas.md's "Weapon scripts" table, which this session re-read and
confirms (pistol: no `default_clip`, so 18; shotgun: no `default_clip`, so
6). Keys used here: `bucket`, `bucket_position`, `clip_size`,
`clip2_size`, `default_clip` (missing = `clip_size`), `default_clip2`,
`primary_ammo`, `secondary_ammo`, `weight`, `item_flags`, `autoswitchto`
and `autoswitchfrom` (missing = 1), `damage` (missing = 0). `item_flags`
bits: 1 select when empty, 2 no auto-reload, 4 no auto-switch when empty,
8 limit in world, 16 exhaustible (HUD history only), and a missing key
means 8. **Every HL2:DM script sets `item_flags` and none has bit 8**:
crowbar, stunstick, pistol, .357, SMG1, AR2, shotgun, crossbow, RPG 0;
physcannon and S.L.A.M. 2; frag 18 (2 + 16).

**Inventory.** One weapon per class, up to 48, kept in the order they
were acquired (that order breaks ties below). Buckets in use 0–4 (six
exist; `slot6`, `slot7` find nothing):

| Bucket | Positions (0, 1, 2) | Weights |
|---|---|---|
| 0 | crowbar, stunstick, physcannon | 0, 0, 0 |
| 1 | pistol, .357 | 2, 7 |
| 2 | SMG1, AR2 | 3, 5 |
| 3 | shotgun, crossbow | 4, 6 |
| 4 | frag, RPG, S.L.A.M. | 1, 0, 1 |

An empty weapon is never removed from the inventory (the "exhaustible"
bit only affects the pickup history HUD).

**Selectable.** A weapon can be selected when it uses no ammo type at all
(crowbar, stunstick, physcannon), has bit 1, or has any of: clip > 0,
reserve of its primary ammo > 0, secondary clip > 0, reserve of its
secondary ammo > 0. So an SMG1 with no bullets but an SMG1 grenade, or an
AR2 with only a ball, stays selectable. The server refuses a switch to a
weapon that has none of its ammo (clip, primary reserve, secondary), and a
weapon with no ammo whose `autoswitchfrom` is 1 refuses to deploy.

**Selection commands** (client side; the HUD is H8's):

- `hud_fastswitch` 0 (default): `slotN` opens the selection on bucket N−1
  at the lowest position that can be selected; pressing the same slot
  again moves to the next selectable position in that bucket, wrapping;
  `+attack` or `+attack2` confirms (the button press is swallowed and
  does not fire). Confirming a weapon that can't be selected plays
  `Player.DenyWeaponSelection` and keeps the menu open.
- `hud_fastswitch` 1: `slotN` switches at once to the next selectable
  weapon in that bucket after the current one (wrapping to the first);
  if there is none different, `Player.DenyWeaponSelection`.
- `invnext` / `invprev`: next or previous selectable weapon ordered by
  (bucket, position) across all buckets, wrapping, starting from the
  weapon in hand; with `hud_fastswitch` 0 it only moves the highlight.
- `lastinv`: switches at once to the last weapon (the one in hand before
  the latest successful switch), refused like any switch if it has no
  ammo or the current weapon can't be put away.
- `phys_swap`: with the physcannon in hand, drops what it holds and does
  `lastinv`; otherwise selects `weapon_physcannon`. Does nothing with no
  weapon in hand.
- `use <classname>`: selects that weapon if owned.
- Selection is refused for observers and frozen or dead players; a
  selection of the weapon already in hand does nothing (it redeploys only
  if the weapon is hidden).
- **There is no drop command**: neither the SDK nor the shipped binds
  have one; a player loses a weapon only by dying (§9).

### 2. Switching, draw and holster

- A switch (from any of the commands above, a pickup auto-switch or an
  empty auto-switch) puts the current weapon away, then deploys the new
  one **in the same tick**. Putting away cancels a reload (no ammo
  moved) and stops suit zoom.
- HL2:DM's view models do have a holster sequence (pistol 0.2667 s, .357
  0.3333, AR2 0.3, shotgun 0.2667, crowbar 0.3333, stunstick 0.3333,
  physcannon 0.2667, crossbow 0.4; SMG1, frag and RPG have none), but the
  deploy that follows in the same tick overwrites both the view model and
  the "next attack" time: **the switch time is the new weapon's draw
  length only**, as in CS:S §3.6.
- Deploy: the draw activity `ACT_VM_DRAW` is played and the player's
  next attack, the weapon's next primary and next secondary all become
  now + draw length.

Draw lengths (`(frames − 1) / fps` of the `ACT_VM_DRAW` sequence; they
come from the view-model files, which Chimera reads at run time; the
numbers are for tests):

| Weapon | View model | Draw (s) | Ticks |
|---|---|---|---|
| pistol | `v_pistol.mdl` | 0.8000 | 54 |
| .357 | `v_357.mdl` | 1.0000 | 67 |
| SMG1 | `v_smg1.mdl` | 0.8333 | 56 |
| AR2 | `v_irifle.mdl` | 0.8333 | 56 |
| shotgun | `v_shotgun.mdl` | 0.7333 | 49 |
| crowbar | `v_crowbar.mdl` | 0.9333 | 63 |
| stunstick | `v_stunstick.mdl` (hl2mp) | 0.9333 | 63 |
| physcannon | `v_physcannon.mdl` (hl2mp) | 0.3222 | 22 |
| frag | `v_grenade.mdl` | 1.2667 | 85 |
| RPG | `v_rpg.mdl` | 1.1000 | 74 |
| crossbow | `v_crossbow.mdl` | 1.0000 | 67 |
| S.L.A.M. | `v_slam.mdl` (hl2mp) | 1.0000 (throw/detonator draw; tripmine draw 0.8333) | 67 |

- The SMG1 and AR2 reset their consecutive-shot count on deploy
  (animation choice only). The AR2 can't be put away while its secondary
  is charging (H4). The stunstick plays `Weapon_StunStick.Activate` and
  sparks on deploy, `Weapon_StunStick.Deactivate` on holster.
- HL2's "lowered weapon" posture never occurs in DM (it needs a friendly
  NPC or a map global); ignore it.

**Auto-switch on pickup** (`cl_autowepswitch`, a client cvar sent to the
server, default 1): after a new weapon is added, the player switches to
it when all hold: it can be switched to (above); and either the player
has no weapon in hand, or [`cl_autowepswitch` ≥ 1 or the player is a bot]
and the new weapon's `autoswitchto` is 1 and the weapon in hand's
`autoswitchfrom` is 1 and the new weapon's `weight` is **strictly
greater** than the one in hand. Consequences: holding the physcannon
(`autoswitchfrom` 0) never auto-switches; the physcannon (`autoswitchto`
0) is never switched to automatically.

**Next best weapon** (used when the weapon in hand is empty, after it is
dropped, and on the ammo rule below): refused if the weapon in hand has
`autoswitchfrom` 0 or can't be put away. Otherwise, going through the
inventory in order and skipping weapons with `autoswitchto` 0 (when a
weapon is in hand) and the weapon in hand itself: the first other weapon
with the **same weight** that has any ammo and can be switched to wins at
once; failing that, the heaviest weapon with any ammo (first in inventory
order among equals). Melee weapons always "have ammo".

**Switch on ammo pickup** (HL2 player rule, not gated by
`cl_autowepswitch`): when ammo of a type the player had none of arrives
and the next best weapon's primary ammo is that type, the player switches
to it. A pickup refused because the type is full sends the HUD an
"ammo denied" notice (H8).

### 3. Spawn loadout

On every spawn, in this order (H0 1.3, with one correction):

1. Suit on.
2. Reserve ammo: 255 Pistol (clamped to its limit: **150**), 45 SMG1,
   1 grenade, 6 Buckshot, 6 357.
3. Weapons, each created at the player's feet and picked up through the
   normal pickup path (§8): `weapon_stunstick` for a Combine-soldier or
   metro-police model, `weapon_crowbar` for a citizen model (exactly one
   of the two); then `weapon_pistol`, `weapon_smg1`, `weapon_frag`,
   `weapon_physcannon`. Weapons given this way never respawn (§10).
4. Pickup ammo from step 3: the pistol and SMG1 arrive with full clips (18
   and 45, no extra reserve); **`weapon_frag` gives its `default_clip` of
   1 as reserve, so the player starts with 2 grenades, not 1** (H0's
   test 19 says 1: Open questions 2).
5. Auto-switch during step 3 (no weapon in hand → first weapon; then by
   weight): crowbar/stunstick → pistol → SMG1; frag (1) and physcannon
   (`autoswitchto` 0) don't switch. Then the player switches to the
   owned weapon named by the client's `cl_defaultweapon` (default
   `weapon_physcannon`), or to the physcannon if they own none of that
   name. With the defaults the physcannon ends in hand (draw 22 ticks)
   and the **last weapon is the SMG1** (so `lastinv` right after spawning
   gives the SMG1).

Resulting reserves with defaults: Pistol 150, SMG1 45, grenade 2,
Buckshot 6, 357 6, everything else 0; clips pistol 18, SMG1 45.

### 4. Ammo pools and carry limits

The ammo table of source_deltas.md ("Ammo types") is confirmed: AR2 60,
AR2AltFire 3, Pistol 150, SMG1 225, 357 12, XBowBolt 10, Buckshot 30,
RPG_Round 3, SMG1_Grenade 3, Grenade 5, slam 5. Names match the scripts'
ammo names ignoring case (`grenade`, `rpg_round`). Giving ammo adds
min(count, limit − current); nothing (and no pickup sound) if that is
< 1. A weapon's clip is separate from the pool and not counted against
the limit. The registered cvars `sk_max_*` exist (all 0) but HL2:DM's
table does not read them.

### 5. Bullets: the shot

Common to pistol, .357, SMG1, AR2 and shotgun (primary and double):

- **Origin**: the eye position after this tick's movement.
- **Aim**: forward vector of (view angles **+ current punch angle**). The
  punch is the value after this tick's decay and before this shot's own
  kick. (specs/cs_source/weapons.md §4.1 says HL2 code applies punch
  to the view only; for HL2:DM it moves the shot. Auto-aim is off on any
  server with more than one slot.)
- **Spread**: the generic HL2 rule of CS:S §4.3 with bias 1 (flatness
  0.5, rejection to the unit disc), spread vector (s, s) from the table
  below; direction = f + x·s·r + y·s·u. The seed for pellet i is
  (command seed & 255) + i, CS:S §4.2.
- **Trace**: 56755.84 units, mask "shot" (solid, moveable, monster,
  window, debris, hitbox); the shooter ignored. When a call fires more
  than one bullet (shotgun pellets, or a machine gun catching up two
  bullets in one tick), **every odd-numbered bullet (1, 3, 5 …) is a
  6 × 6 × 6 box** (±3) instead of a line.
- **No penetration**: the bullet stops at the first thing it hits, except
  glass on a `func_breakable` without its "no bullet penetration" flag,
  which it passes as in CS:S §4.4. No range falloff.
- **Water**: a shot ending in water traces again for the surface (splash,
  H9) and **still damages** what is under the water.

| Weapon | Spread s | Bullets per shot | Ammo type | Fires under water (eyes under) | Tracer every |
|---|---|---|---|---|---|
| pistol | ramp 0.00873 → 0.05234 (§6) | 1 | Pistol | yes | 2nd bullet |
| .357 | 0 (exact) | 1 | 357 | no | 4th |
| SMG1 | 0.04362 (5°) | 1 (more if catching up) | SMG1 | no | 2nd |
| AR2 | 0.02618 (3°) | 1 (more if catching up) | AR2 | no | 2nd |
| shotgun | 0.08716 (10°) | 7 / 12 | Buckshot | no | 4th (per pellet) |

The tracer counter is one counter for the whole server, advanced once per
bullet or pellet; a bullet gets a tracer when the counter, before it is
advanced, is a multiple of the weapon's number (and it did not pass
through glass). Other clients are only told "this shot has tracers" and
then draw one for **every** pellet of that shot (H9 draws them; H0 1.10
named only SMG1 and AR2 as every-2nd: the pistol is too).

A weapon that "can't fire under water" (eyes under water, water level 3):
pressing attack plays its empty sound, sets next primary = now + 0.2
(14 ticks) and ends the weapon's frame for this tick; no ammo is used.

**Order against the empty-clip rules** (answered 2026-11-09; .357, SMG1
and AR2 primaries, all on the generic frame of §7.6): the attack branch
runs only when attack is held and next primary ≤ now, and inside it the
**empty test comes first**. Clip ≤ 0 → the generic fired-on-empty step
(CS:S §3.4: first tick the empty click, at most every 0.5 s, and the
"fired on empty" mark; a later held tick reloads or auto-switches), and
the frame then **continues** to the reload-key and no-button rules as on
land. Only a weapon with rounds in the clip reaches the water test, and
that refusal **ends the frame** at once: the reload key is not looked at
on that tick. Consequences: an empty .357 under water clicks and then
reloads exactly as on land (a reload under water is allowed); holding
attack under water with rounds in the clip re-refuses every 0.2 s, and
since next primary is then always in the future or the frame ends, a
reload pressed together with attack never starts while attack is held
(release attack to reload). The shotgun keeps its own order (§7.4: clip
0 handled before the water test, the same answer).

**Secondary fire under water** (correction 2026-11-09 to §7.3's "dry-fire
animation … +0.5 s"): neither HL2:DM machine gun sets "alt-fires under
water", so the generic frame refuses the secondary **before** the weapon's
own secondary code runs, whenever attack2 is held (no timer test in front
of it): empty sound, next secondary = now + 0.2, frame ends (no primary,
no reload that tick). With no grenades or balls the earlier "no secondary
ammo" step answers first instead (empty sound at most every 0.5 s, next
secondary = now + 0.5, the frame goes on). So the weapon's own dry-fire
branch (`ACT_VM_DRYFIRE`, 0.5 s) is never reached in HL2:DM. Because the
water refusal has no timer in front of it, holding attack2 under water
plays the empty sound on **every** tick (each replaces the last on the
weapon's channel; the client plays it on its first prediction only).
Keep (parity).

### 6. Bullets: damage and force

- **Damage per bullet = the active weapon's script `damage`**, against
  players **and against everything else** (props, breakables, the world):
  pistol 8, .357 75, SMG1 5, AR2 11, shotgun 9 per pellet. The damage
  kind is "bullet" (the shotgun adds "buckshot"), plus "always gib" when
  the damage is above 16 (.357) else "never gib" (cosmetic).
- This resolves H0's open question 6 and **corrects H0 1.3** ("damage to
  anything that isn't a player comes from the ammo type"): the HL2:DM
  server writes the script damage into both the player and the general
  damage of every bullet a player fires, so the ammo table's damage
  columns (all 0 for every type) are never consulted for player bullets.
  The HL2 cvars `sk_plr_dmg_*` and `sk_npc_dmg_*` are registered (default
  0) but HL2:DM's ammo table does not refer to them and no shipped cfg
  sets them: changing them has no effect. A target's own damage rules
  still apply (prop and breakable multipliers: specs/source/prop_damage.md
  and breakables.md).
- **Players**: the hitgroup of the hitbox hit scales the bullet:
  head × `sk_player_head`, chest × `sk_player_chest`, stomach ×
  `sk_player_stomach`, arms × `sk_player_arm`, legs ×
  `sk_player_leg`, generic × 1. Registered defaults 2, 1, 1, 1, 1;
  `cfg/skill.cfg` (run through `skill_manifest.cfg`, identical in the
  game and the server) sets **head 3**, the others 1. Team damage refused
  per `mp_friendlyfire` / `mp_teamplay` before any blood (H6).
- **One damage event per target per shot**: all bullets of one shot that
  hit the same target are summed (each pellet scaled by its own hitgroup
  first) and applied once (CS:S §4.5 accumulation). Then the suit rule
  (H0 "Behavior", H2): health part = D × 0.2, armour part = D × 0.8 (at
  least 1); armour loses that part truncated toward zero as an integer
  subtraction (armour − 6.4 from 100 leaves 93); if the armour part
  exceeds the armour left, the armour absorbs only what is left and the
  rest goes to health; health subtracts the whole part of the damage and
  carries the fraction until it adds up to 1 (CS:S §4.5). Death at
  health ≤ 0.
- **Force** along the normalised shot direction, at the hit point:
  impulse of the ammo type × `phys_pushscale` (1), where impulse =
  ft/s × 12 × (grains × 0.002285 / 16 lb in kg) × 3.5:

| Ammo | Grains, ft/s | Impulse (kg·in/s) | Δv of a 30 kg prop (in/s) |
|---|---|---|---|
| Pistol, SMG1, AR2 | 200, 1225 | 666.57 | 22.22 |
| 357 | 800, 5000 | 10882.81 | 362.76 |
| Buckshot (per pellet) | 400, 1200 | 1305.94 | 43.53 |
| XBowBolt (H4) | 800, 8000 | 17412.50 | 580.42 |

- Bullets do not push players (no such code in the SDK; Open questions
  6). A victim's received forces within one tick are summed and become
  the force of their death (ragdoll H7, dropped weapon §9).

### 7. The bullet weapons

#### 7.1 Pistol (`weapon_pistol`)

- **Fire timing.** Uses the generic frame and the generic primary attack
  with the accumulating rule (CS:S §3.2) at 0.5 s: holding attack fires
  every 0.5 s (33 or 34 ticks; long-run exactly 2 per second). A fresh
  press resets next primary to now, so the first shot of a press is
  immediate.
- **Click refire.** After the generic frame, each tick: if attack is
  **not** held and now > (last shot + 0.1), next primary is set to now
  − 0.1, i.e. the next press fires at once. With 0.015 ticks the fastest
  clicking is: shot on tick 0, release on tick 1, unlock on tick 7
  (0.105 > 0.1), press and shot on tick 8 — **8 ticks (0.12 s) between
  shots**, 8.33 shots per second.
- **Attack2 blocks the pistol**: while attack2 is held (and no reload),
  next primary, the click-unlock time and the "last shot" time are all
  pushed to now + 0.1 every tick.
- **Dry fire.** With attack held, clip ≤ 0, next primary already passed
  and not reloading: empty sound `Weapon_Pistol.Empty`, `ACT_VM_DRYFIRE`
  (0.3667 s), next primary = now + 0.3667, click-unlock = now + 0.2. The
  generic empty rule runs too (§7.6), so the next held tick after the dry
  fire's 0.3667 s starts the reload (or the auto-switch).
- **Accuracy penalty** P (seconds): +0.2 after every shot (added after
  the shot, so a shot uses the penalty from before it; the add is not
  clamped, P can exceed 1.5); once per tick, while attack is not held and
  now > last shot + 0.1, P −= 0.015 and P is clamped to [0, 1.5] (the
  clamp happens only here); a reload that starts sets P = 0. The decay
  runs whether or not the player is busy (draw, reload).
- **Spread from P** (the pistol does have spread; **correction to H0
  1.3, Q7**: the HL2:DM server asks the weapon for its cone through a
  path that ends in the pistol's ramp, scaled by 1.0, and the client
  predicts the same):

```math
\text{ramp} = \operatorname{clamp}\!\left(\frac{P}{1.5},\,0,\,1\right),\qquad
s = 0.00873 + \text{ramp}\,(0.05234 - 0.00873)
```

  i.e. 1° after a rest, 6° once P ≥ 1.5 (8 held shots, or about 8 quick
  clicks).
- **Kick**: before every shot the punch angle and its velocity are reset
  to zero; after the shot, view punch (pitch U(0.25, 0.5), yaw
  U(−0.6, 0.6), roll 0) — pitch positive, so the view dips **down**.
  Shared random per command with its own label per component (CS:S
  §4.2).
- **Reload**: `ACT_VM_RELOAD` 1.4333 s (96 ticks), magazine swapped at
  the end (generic, CS:S §3.4); `Weapon_Pistol.Reload`.
- Shot-series animation: shots less than 0.5 s apart count up and pick
  `ACT_VM_PRIMARYATTACK`, `ACT_VM_RECOIL1`, `…2`, `…3` (each 0.4 s);
  cosmetic.
- **Fires under water** (eyes under, water level 3): the pistol is the
  only clip weapon that does (§5's table; answered 2026-11-09, the
  "can't fire under water" line below belongs to the .357, §7.2).

#### 7.2 .357 (`weapon_357`)

- Fire: one bullet, no spread; next primary and next secondary = now +
  0.75 (50 ticks, Open questions 3), the "set" rule (no accumulation;
  holding attack fires every 0.75 s). The fire sequence is 0.8 s, longer
  than the refire.
- Kick: view punch (−8, U(−2, 2), 0) — 8° up; the yaw comes from the
  plain random stream right after the shot (seeded by the shot, so it
  follows the command; not a labelled shared random). **Client only**,
  on the first prediction of the shot: the client's real view angles
  change by a whole random −1, 0 or +1 degree in pitch and in yaw — a
  permanent nudge of the aim, not a punch. Because the client draws two
  extra numbers for it, its predicted punch yaw differs from the
  server's (the server's punch wins on the next update).
- Empty: generic (§7.6); sound `Weapon_Pistol.Empty`.
- Reload: `ACT_VM_RELOAD` 3.6667 s (245 ticks); its animation events
  (loader sounds) are effects only.
- The .357 can't fire under water (§5's rule).

#### 7.3 Machine guns: SMG1 (`weapon_smg1`) and AR2 (`weapon_ar2`)

- Fire timing: accumulating at 0.075 s (SMG1, 5 ticks, 13.33 per
  second) and 0.1 s (AR2, ticks 7, 7, 6 repeating, 10 per second). All
  bullets due in a tick (normally one) are fired in one call, limited by
  the clip; the clip check happens first — an empty clip fires nothing.
- **Kick per call** (after the bullets), with t = time attack has been
  held (the generic fire-duration counter: +0.015 every tick attack is
  held, reset when released, on a reload start and on an empty-trigger
  reload; it is already 0.015 on the first shot), M = max vertical kick,
  L = slide limit:

```math
k = \frac{\min(t, L)}{L},\qquad
\Delta = \left(-(0.2 + Mk),\; \pm\frac{0.2 + Mk}{3},\; \pm\left(0.1 + \frac{Mk}{8}\right)\right)
```

  1. If √(|punch|² + |punch velocity|²) ≤ 10, punch and punch velocity
     are reset to 0 first.
  2. Signs: the generator is seeded with (command seed & 255) and two
     whole numbers are drawn from {−1, 0, 1}; the yaw term is negated
     when the first is ≥ 0, the roll term when the second is ≥ 0 (each
     flipped with probability 2/3).
  3. Δ is clipped so that punch + Δ stays within ±24 pitch, ±3 yaw, ±1
     roll.
  4. View punch Δ × 0.5 (punch velocity += Δ × 10 deg/s).

  SMG1: M 1, L 2 s; AR2: M 8, L 5 s. First shot SMG1: Δ = (−0.2075,
  ±0.06917, ±0.10094) → velocity (−2.075, ±0.6917, ±1.0094) deg/s. After
  2 s held: Δ = (−1.2, ±0.4, ±0.225). AR2 after 5 s: Δ = (−8.2,
  ±2.7333, ±1.1) before the clip (roll clipped to the 1° limit).
- SMG1 reload: `ACT_VM_RELOAD` 1.5 s (100 ticks, Open questions 3). It
  sets only the weapon's next primary; the player's next attack and the
  weapon's next secondary keep their earlier values, so the weapon keeps
  running its full frame during the reload (the grenade can be fired and
  cancels the reload with no ammo moved; H4).
- AR2 reload: 1.5667 s (105 ticks), generic (player busy). The AR2 can't
  reload while its secondary is charging (H4).
- Secondary fire (SMG1 grenade, AR2 ball): H4. Both use their secondary
  ammo type; both refuse under water through the generic frame (§5,
  "Secondary fire under water": empty sound, next secondary +0.2 s, every
  tick attack2 is held; corrected 2026-11-09, was "dry-fire animation,
  +0.5 s").
- Empty sound: `Weapon_SMG1.Empty`, `Weapon_IRifle.Empty` (AR2).
- Animation by consecutive shots (reset when attack is released and on
  deploy): `ACT_VM_PRIMARYATTACK` for the first two, then
  `ACT_VM_RECOIL1`, `…2`, `…3`; the AR2's vent pose parameter
  follows the count 0–5; cosmetic.
- **AR2 vents** (answered 2026-11-09): `v_irifle.mdl` has one pose
  parameter, `VentPoses`, range 0 to 1, not looping, and a 2-blend
  sequence of the same name (no activity; flags delta, autoplay and
  "post", i.e. an additive layer the model always plays on top of every
  view-model sequence) that blends along it. Value = clamp(n / 5, 0, 1),
  n = the consecutive-shot count above (0 after a release or a deploy,
  +1 per primary call that fires, before the bullets): 0, 0.2, 0.4, 0.6,
  0.8, then 1 from the 5th shot on. It is written each tick at the start
  of the AR2's frame, before the generic frame (so before that tick's
  shot and before that tick's release reset): it shows the count as it
  stood at the end of the previous tick, one tick late, and drops to 0 on
  the tick after the one where attack was released. It jumps, there is no
  easing or decay over time. Not written while the player is busy
  (reload, draw), so the vents hold their last value then; a reload does
  not reset the count, only release and deploy do. Both the server and
  the predicting client set it on the view model. Which of the two blends
  is "closed" is the model's (the sequence stores its pose range as
  1 → 0; Open questions 12).

#### 7.4 Shotgun (`weapon_shotgun`)

The shotgun replaces the generic frame with its own. Its timer is the
weapon's next primary; the player's next attack is set only by the pump.
Sequences (`v_shotgun.mdl`): fire 0.3333, double 0.5, pump 0.5333,
reload start 0.5, insert 0.4, reload finish 0.4333, dry fire 0.3333.
Each tick, in order:

1. Reload held while a pump is pending → remember "reload after pump".
2. **While reloading**: attack held, clip ≥ 1, no pump pending → stop
   reloading (no shell added now) and remember "fire 1"; else attack2
   held, clip ≥ 2, no pump pending → stop and remember "fire 2"; else,
   when next primary has passed: reserve 0 → finish; clip < 6 → **insert
   one shell at once** (clip +1, reserve −1), play the insert, next
   primary = now + 0.4, end the frame; clip full → finish. "Finish" plays
   the finish sequence, next primary = now + 0.4333, ends the frame.
3. Pump pending and next primary passed → pump: if "reload after pump",
   start a reload first; then pump sequence and sound `Weapon_Shotgun.Special1`,
   player next attack = next primary = now + 0.5333; end the frame.
4. ("fire 2" or attack2 held) and next primary passed:
   clip 1 → a normal 7-pellet shot; clip 0 → reserve 0: dry fire, else
   start a reload; under water → empty sound, next primary +0.2, end;
   else the **double shot**: 12 pellets, clip −2, next primary = now +
   0.5, punch pitch U(−5, 5) (up or down), yaw 0; attack2 is cleared from
   this tick's buttons.
5. Else ("fire 1" or attack held) and next primary passed: clip 0 →
   reserve 0: dry fire (empty sound, next primary = now + 0.3333), else
   start a reload; under water → as above; else the **primary**: 7
   pellets, clip −1, next primary = now + 0.3333, punch (U(−2, −1),
   U(−2, 2), 0).
   Either shot sets "pump pending".
6. Reload held and not reloading → start a reload: refused when a pump
   is pending, the reserve is 0 or the clip is full; else start
   sequence, next primary = now + 0.5. Otherwise: if the shotgun has no
   ammo at all and next primary is past, switch to the next best weapon
   (old weapon's next primary +0.3); else if the clip is 0 and next
   primary is past, start a reload (auto-reload, also while attack is
   held); then idle.

Timing (ticks at 0.015): primary held: shot, 23 ticks to the pump, pump
36 ticks → **59 ticks (0.885 s) per shot**; double held: 34 + 36 = **70
ticks (1.05 s)**. Reload from clip 0 with ≥ 6 in reserve: start at tick 0;
shells enter on ticks 34, 61, 88, 115, 142, 169 (start 0.5 s, then one
every 0.4 s — the shell goes in when its insert animation **starts**);
finish begins on tick 196; can fire on tick 225. Firing during a reload
waits for the current start or insert sequence to end (the shell of a
running insert is already in).

A shotgun put away with a pump pending keeps it: after the next draw it
pumps (0.5333 s) before it can fire.

#### 7.5 Damage per weapon (players, generic hitgroups ×1, head ×3)

Shots to kill a player at 100 health (computed with the suit rule of §6,
float arithmetic as the game; "body" = any non-head hitgroup):

| Weapon | Body dmg | Head dmg | Body, armour 0 | Body, armour 100 | Head, armour 0 | Head, armour 100 |
|---|---|---|---|---|---|---|
| pistol | 8 | 24 | 13 | 24 | 5 | 9 |
| .357 | 75 | 225 | 2 | 3 | 1 | 1 |
| SMG1 | 5 | 15 | 20 | 40 | 7 | 14 |
| AR2 | 11 | 33 | 10 | 18 | 4 | 7 |
| shotgun, all 7 pellets | 63 | 189 | 2 | 4 | 1 | 2 |
| shotgun double, all 12 | 108 | 324 | 1 | 2 | 1 | 1 |
| crowbar | 25 (no head) | – | 4 | 8 | – | – |
| stunstick | 40 (no head) | – | 3 | 5 | – | – |

With armour the counts double or more because only 20 % reaches health
until the armour is gone; the per-hit traces are in test cases 27–31.

#### 7.6 Generic empty, reload and holster rules (all clip weapons except where noted)

Same as CS:S §3.4–3.5 (empty click at most every 0.5 s and "fired on
empty"; holding attack on an empty clip then reloads or auto-switches;
with no buttons held, a weapon with no ammo at all auto-switches once
both its timers are strictly in the past and its old next primary
becomes now + 0.3; a clip-0 weapon with reserve auto-reloads once both
timers are strictly past; reload refused with an empty reserve or a full
clip; magazine moved at the end; holding attack through the end fires
on the completion tick). HL2:DM addition:

- **Holstered auto-reload** (`sk_auto_reload_time`, 3): every tick, for
  each carried weapon that is not in hand: if now − (time it was last put
  away) > 3 s, its clip is filled from the reserve with no animation
  (min(clip size − clip, reserve)) and the "put away" time becomes now
  (so it repeats every 3 s while holstered). The shotgun does the same
  with its own code (same numbers). A weapon picked up and never yet in
  hand has a "put away" time of 0, so it is **topped up on the next
  tick** (Quirks). Melee weapons and the physcannon have no clip;
  nothing happens.

### 8. Pickup

Walking into a weapon (touch box: the weapon's bounds grown 36 units on
X and Y, 18 up, CS:S §3.9; weapons are triggers, players never collide
with them, and `+use` does not pick them up) runs, in order:

1. Refused if the weapon has an owner, is dissolving, or has the "no
   player pickup" flag; refused if `mp_weaponstay` > 0 and the player
   already owns a weapon of that class (every HL2:DM weapon lacks the
   "limit in world" bit, so this applies to all of them).
2. Refused if the weapon can't see the player (a line from the weapon to
   the player's eye, blocked by solid, moveable, window, monster and
   grate contents — windows block).
3. **Already owns that class**: the weapon's clip (or, for a weapon
   without a clip, the ammo count the weapon entity carries) is given to
   the reserve up to the limit, likewise its secondary. If anything was
   taken the weapon is **removed entirely**, even if rounds were left in
   it, and (if it is a respawning map weapon) its respawn starts (§10);
   if nothing was taken (reserve full) it stays and the touch is refused.
4. **New class**: added to the inventory (stops being a world object,
   follows the player, next primary and secondary = now). Ammo: a
   weapon without a clip gives `default_clip` to the reserve; a clip
   weapon whose `default_clip` > `clip_size` gets a **full clip** and
   the excess as reserve (AR2: clip 30, +30 reserve), **replacing
   whatever clip it had**; otherwise its clip stays as it was. Secondary
   likewise. The respawn of a map weapon starts. Then the auto-switch
   rule (§2).

A map-placed weapon's ammo is its spawn ammo: clip = `default_clip`
(AR2 60 in the clip of the world object). What a pickup gives:

| Weapon | Taken new (clip, + reserve) | Taken by an owner (+ reserve) |
|---|---|---|
| pistol | 18, +0 | +18 Pistol |
| .357 | 6, +0 | +6 357 |
| SMG1 | 45, +0 | +45 SMG1 |
| AR2 | 30, +30 | +60 AR2 (limit 60) |
| shotgun | 6, +0 | +6 Buckshot |
| crossbow, frag, RPG, S.L.A.M. | H4 (same rule: crossbow clip 1 + 4 bolts; frag +1 grenade; RPG +3 rounds) | same rule |

### 9. Drop on death

- When a player dies the weapon in hand is first put away (reload
  cancelled), then **only that weapon is dropped**; every other weapon,
  all reserve ammo and the suit are removed shortly after (the dead
  player's next think, about 0.1 s later).
- **Never dropped**: `weapon_crowbar`, `weapon_stunstick` and
  `weapon_physcannon` delete themselves instead (the physcannon first
  lets go of what it holds). A frag primed with attack or attack2 held
  is dropped live instead of the weapon (H4).
- Placement: 12 units below the eye, facing the player's yaw. Velocity:
  the sum of damage forces the player received this tick (§6; melee
  force §11) divided by the player's physics mass (85 kg), capped at 400
  in/s; plus a (200, 200, 200) spin. E.g. a pistol kill gives 666.57 /
  85 = 7.84 in/s, a .357 kill 128.0 in/s, a crowbar kill 88.2 in/s. It
  is a physics object from then on; bullets push it.
- It keeps its clip (the dead player's reserve is not added). A
  clip-less weapon keeps the ammo count it was created with (frag 1,
  RPG 3), whatever the dead player had.
- It never respawns; it becomes touchable 1.0 s after the drop and is
  removed 30 s after that (31 s after the drop) unless picked up.
- With no weapon to switch to after a drop the view model is hidden.

### 10. Respawn of placed weapons and items, `mp_weaponstay`

- **Placed weapons** (from the map, not given at spawn, not dropped):
  they fall to the floor, and once on the ground become physics objects
  (bullets and explosions push them). When one is taken (as a new class
  or for ammo), a **new copy** is created at the original's spawn point
  (oriented as the taken one was at that moment), invisible and not
  touchable, and appears after `sv_hl2mp_weapon_respawn_time` (20 s): it
  becomes visible, plays `AlyxEmp.Charge`, shows a muzzle-flash effect,
  becomes a physics object and touchable, with fresh default ammo.
- **`mp_weaponstay` > 0**: the delay is **0** for weapons without the
  "limit in world" bit (all of HL2:DM's), so the copy appears at once
  (next think): the weapon seems to stay; and owners of that class can't
  take it (§8.1), so it never gives ammo. Weapons with the bit (none in
  stock HL2:DM) keep the 20 s delay and, when they are due, wait another
  20 s at a time while fewer than 100 entity slots are free.
- **Items** (`item_*`: kits, batteries, ammo boxes; H2/H10): the taken
  item itself hides, goes back to its original spot and angles, and
  reappears after `sv_hl2mp_item_respawn_time` (30 s) with
  `AlyxEmp.Charge` and a muzzle-flash effect. Items with the "no
  respawn" spawnflag are removed instead.
- **Relocation**: the rules check every placed weapon and item once per
  20 s (weapons) or 30 s (items) each, starting at map start: if it is
  more than 64 units from its original spawn point and at rest (physics
  asleep, or on the ground), it is moved back to its original origin and
  angles with `AlyxEmp.Charge`. A weapon leaves this list when picked up;
  its respawned copy joins it when it appears.

### 11. Melee: crowbar and stunstick

Both use one shared melee frame instead of the generic one (no reload,
no empty rules, no fire-duration counter). Each tick: attack held and
next primary passed → swing; else attack2 held and next secondary passed
→ secondary (the crowbar's and stunstick's do nothing at all — no timers
change); else idle.

**Swing** (primary; other players lag-compensated around it):

1. f = forward of the view angles (no punch); start = eye; line trace to
   start + 75 f with the "shot hull" mask (solid, moveable, monster,
   window, debris, grate — **no hitbox bit**). Triggers along the line
   that react to damage get a club hit (CS:S §4.4).
2. If the line missed: sweep a box ±16 from start to start + (75 −
   27.712) f = 47.288 units. If the box hits an entity (the world counts
   as an entity, Quirks), take d = normalise(entity **origin** − start);
   if d · f < 0.70721 it is a miss; else re-trace a line to twice the
   box's hit distance and, if that misses, lines to the 8 corners of a
   ±16 box at that doubled end point, keeping the nearest hit; if all of
   those miss, the box hit itself is used.
3. Swing sound (`Weapon_Crowbar.Single` / `Weapon_StunStick.Swing`).
4. Miss: `ACT_VM_MISSCENTER` (0.6667 s), splash if the 75-unit line
   enters water. Hit: damage 25 (crowbar) / 40 (stunstick), kind "club",
   force = damage × 300 along f at the hit point; applied through the
   target's trace-attack. Because the trace has no hitbox bit, a player
   is hit on their **collision box with hitgroup generic: melee is never
   scaled by the head cvar**. Stunstick plays `Weapon_StunStick.Melee_Hit`
   on any hit (the crowbar's hit sound comes from the surface impact
   only); impact effect and decal (the stunstick's own spark effect 4
   units off the surface; H9). Hit animation `ACT_VM_HITCENTER`
   (0.5 s).
5. Next primary = now + refire (crowbar 0.4 s = 27 ticks, stunstick 0.8 s
   = 54 ticks), the same for hit and miss; next secondary = now + the
   played sequence's length.

- **No view punch** on either swing in HL2:DM (CS:S §6.1's crowbar punch
  U(1,2)/U(−2,−1) belongs to code HL2:DM never calls).
- **Stunstick extras in DM**: only the deploy/holster sounds and sparks,
  the hit sound and impact effect above. Its knock-back, red screen fade
  and view punch on players belong to NPC wielders only.
- Melee works under water.

### 12. Lag compensation and prediction

Same as CS:S §4.6: other players are rewound around each bullet call and
each primary melee swing. HL2:DM picks whom to rewind: only if this
command holds attack or the player fired within the last 5 commands;
then each other player whose origin is within 1.5 × (their current max
speed) × `sv_maxunlag` of the shooter, or whose origin lies within 45°
(cos 0.707107) of the command's view direction. Weapons are predicted on
the shooter's client (seed, timers, kicks); other clients draw the
shot's tracers and impacts from one message with the shot's origin,
direction, seed, bullet count and spread, re-tracing it themselves (H9).

## Per-tick order

One user command on the server (the client predicts the same):

1. Time and the command's seed (CS:S §2).
2. A weapon selection in the command: put away, deploy (§2).
3. Button edges.
4. Pre-think: `+use` handling; **every carried weapon not in hand runs
   its holstered auto-reload check** (§7.6); if not busy, the weapon in
   hand's pre-frame (pistol penalty decay).
5. Movement: punch decays first (shared spring), then the move; touched
   weapon and item triggers are processed (pickups, §8).
6. Post-think: if busy (now < player next attack) the busy frame (pistol
   penalty decay only); else the weapon's frame: generic (§7.6 order:
   fire duration → reload completion → attack2 → attack → reload key →
   no-button auto rules), or the pistol's extra click/dry step after it,
   the machine gun's shot-count reset before it, the shotgun's own
   (§7.4) or melee's (§11). A shot: lag compensation on, origin = eye,
   aim = view + punch, bullets traced and summed per target, damage
   applied, compensation off; then that weapon's kick.
7. Tick base +1.

Outside the commands, every server frame: rule think (relocation, §10),
entity thinks (respawn copies appearing, dropped weapons becoming
touchable or removed, items reappearing).

## Edge cases

- Pressing attack and attack2 together: the shotgun takes attack2 first;
  the generic frame gives attack2 priority; the pistol is blocked by
  attack2 (§7.1).
- A shot whose trace starts in solid hits at its start (CS:S).
- Two machine-gun bullets due in one tick (after a hitch): the second is a
  ±3 box trace.
- Shotgun with clip 1 and attack2: a normal 7-pellet shot.
- Shotgun reload started by attack on an empty clip (with reserve); the
  same tick does not fire.
- Switching away mid-reload loses nothing; the clip is filled 3 s later
  while holstered.
- Ammo given to a full pool: nothing, and the "owner" pickup is refused
  so the weapon stays.
- A weapon picked up while dead or frozen: dead players never touch
  pickups; frozen ones (intermission) cannot select.
- `cl_defaultweapon` naming a weapon not owned: the physcannon.
- A weapon in hand with no ammo at all and the physcannon is the only
  other weapon: the physcannon is never chosen by "next best" while a
  weapon is in hand (`autoswitchto` 0) — the empty weapon stays in hand
  unless a melee weapon or another weapon has ammo.
- Placed weapon knocked away and never picked up: sent back within 20 s
  once at rest > 64 units from its spot.
- Melee box fallback against a crouched or lower player: the origin
  (feet) direction usually fails the 45° test unless looking down at
  them.

## Quirks

Keep all of these.

- **Punch moves the shot** (§5); the pistol's punch is downward (§7.1).
- **The pistol has spread** after all (1°–6°), while the .357 is exact
  (§7.1, §7.2). H0's "no spread" is wrong for the pistol.
- **Script damage hits props and breakables too** (§6); H0's ammo-type
  fallback does not happen.
- **Melee ignores hitgroups** (§11): a crowbar to the face does 25.
- **Melee box test uses the target's origin**: for players that is the
  feet, so the 32-unit fallback mostly misses standing players unless the
  attacker looks down; for the world (origin at the map's 0, 0, 0) a box
  hit on a wall counts only when the map origin happens to lie within 45°
  of the view — then the swing "hits" the wall (sound, decal) without the
  line reaching it.
- **Owner pickups delete the weapon** even when rounds remain in it.
- **A dropped AR2 refills**: taken new it always gives clip 30 + 30
  reserve, whatever was in it.
- **Never-deployed weapons top up on the next tick** (holster time 0).
- **Spawn gives 2 grenades** (1 from the ammo grant, 1 from the frag
  weapon), per the SDK; to be confirmed (Open questions 2).
- Holding the physcannon blocks every pickup auto-switch.
- Tracers: one server-wide counter; other clients see tracers on all
  pellets of a shotgun shot that earned one.
- The .357 nudges the client's real view angles by ±1° (client only).

## Test cases

Implementer checks, no game needed (tick 0.015, `sk_player_head` 3,
health 100, armour 0 unless said, defaults otherwise). "Tick n" counts
from the action at tick 0.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | spawn, DM, citizen model, defaults | – | spawn tick | weapons crowbar, pistol, SMG1, frag, physcannon (order of acquisition); in hand physcannon; last weapon SMG1; reserves Pistol 150, SMG1 45, grenade 2, Buckshot 6, 357 6; clips pistol 18, SMG1 45 |
| 2 | spawn with `models/police.mdl` or `models/combine_soldier.mdl` | – | – | stunstick instead of crowbar, otherwise as 1 |
| 3 | as 1, `cl_defaultweapon weapon_smg1` | – | – | SMG1 in hand (no redeploy needed); first shot allowed 56 ticks after the SMG1's deploy |
| 4 | pistol in hand, idle | select SMG1 at tick 0 | – | first SMG1 shot possible at tick 56; holster sequence does not add time |
| 5 | pistol, clip 18, reserve 150 | hold attack | until empty | 18 shots; first at tick 0, last 8.5 s later (tick 567 ± 1); clip 0 |
| 6 | pistol | press tick 0, release tick 1, press tick 8, release 9, … | – | a shot on every press: 8-tick intervals; 18 shots in 136 ticks; a press on tick 7 would not fire |
| 7 | pistol, P = 0 | shots as in 6 | before 3rd shot | P after shot 1 = 0.2; decay on tick 7 → 0.185; shot 2 (tick 8) → 0.385; tick 15 → 0.37 (shot 3 uses s = 0.00873 + 0.24667 × 0.04361 = 0.01949); after shot 3: 0.57 |
| 8 | pistol, P = 0 | hold attack 8 shots (0.5 s apart) | after shot 8 | P = 1.6 (no decay while held; s = 0.05234); after release, decay starts once now > last shot + 0.1; the first decay gives 1.585, clamped to 1.5; P reaches 0 about 100 decay ticks later (1.5 s) |
| 9 | pistol, P = 1.2, clip 5, reserve 10 | reload | – | P = 0 at the reload start; clip 15 at tick 96 |
| 10 | pistol, clip 0, reserve 0, SMG1 with ammo | hold attack | – | tick 0: empty click + dry fire (next primary +0.3667); tick 25: auto-switch to SMG1 (next best, weight 3); pistol's next primary = tick-25 time + 0.3 |
| 11 | .357, clip 6 | hold attack | – | shots every 50 ticks; 6 shots in 250 ticks (3.75 s); no spread (direction = view + punch exactly) |
| 12 | .357, punch 0 | one shot at tick 0 | – | punch velocity (−160, 20·y, 0) with y ∈ [−2, 2]; pitch punch −2.4 at tick 1, −4.4409 at tick 2, peak −10.47 at tick 9, back through 0 at tick 31 |
| 13 | .357, clip 2, reserve 6 | reload | – | clip 6, reserve 2 at tick 245 |
| 14 | SMG1, clip 45 | hold attack | – | 45 shots, last 3.3 s after the first (220 ticks; individual intervals 5 ticks, ±1 from float rounding, total within ±1) |
| 15 | SMG1, punch 0, first shot | – | – | Δ = (−0.2075, ±0.069167, ±0.100938); punch velocity += (−2.075, ±0.69167, ±1.00938) |
| 16 | SMG1, attack held 2.5 s, punch and velocity 0 | a shot | – | k = 1; Δ = (−1.2, ±0.4, ±0.225); velocity += (−12, ±4, ±2.25) |
| 17 | AR2, attack held 6 s, punch (11, 0, 0.5), punch velocity 0 (√121.25 > 10, so no reset) | a shot | – | k = 1; Δ before clip (−8.2, ±2.7333, ±1.1); pitch 11 − 8.2 = 2.8 inside ±24, yaw inside ±3; roll: +1.1 gives 1.6 → clamped to 1, Δroll = +0.5; −1.1 gives −0.6, Δroll = −1.1; punch velocity += Δ × 10, e.g. (−82, ±27.333, +5 or −11) |
| 18 | AR2, clip 30 | hold attack | – | 30 shots within 2.9 s (tick 193 or 194 for the last); intervals 7, 7, 6 repeating |
| 19 | AR2 clip 0, reserve 60 | reload | – | clip 30, reserve 30 at tick 105 |
| 20 | shotgun, clip 6 | hold attack | – | shots at ticks 0, 59, 118, 177, 236, 295; pump starts 23 ticks after each shot |
| 21 | shotgun, clip 6 | hold attack2 | – | double shots at ticks 0, 70, 140 (clip 6 → 4 → 2 → 0); 12 pellets each |
| 22 | player 100/100 armour | .357 head (225) | – | health part 45, armour part 180 > 100 → armour 0, health takes 125 → dead (1 shot) |
| 23 | shotgun, clip 0, reserve 10, no buttons | – | from tick 0 (auto-reload start) | shells at ticks 34, 61, 88, 115, 142, 169; finish at 196; fire possible at 225; reserve 4 |
| 24 | as 23 | press attack on tick 70 | – | reload stops after shell 2 (clip 2); shot at tick 88; clip 1; pump at tick 111 |
| 25 | shotgun pellets | 7-pellet shot | – | pellets 1, 3, 5 are ±3 box traces, 0, 2, 4, 6 lines; double: pellets 1, 3, 5, 7, 9, 11 boxes |
| 26 | target 100 health, no armour | each weapon, body / head | – | shots to kill as the table of §7.5 (pistol 13 / 5, .357 2 / 1, SMG1 20 / 7, AR2 10 / 4, shotgun 2 / 1, double 1, crowbar 4, stunstick 3) |
| 27 | target 100 health, armour 100 | pistol body | 1 shot | health 99 (1.6: 1 now, 0.6 carried), armour 93 |
| 28 | as 27 | pistol body ×24 | – | dead on shot 24; armour reaches 0 on shot 15 (2 left after 14 shots, then absorbs 2, health takes 6) |
| 29 | as 27 | crowbar ×8 | – | health 95, 90, 85, 80, 75 (armour 80, 60, 40, 20, 0), then 50, 25, 0: dead on hit 8 |
| 30 | as 27 | stunstick ×5 | – | health 92, 84, 76 (armour 68, 36, 4), then 40 (armour 0), then 0: dead on hit 5 |
| 31 | as 27 | shotgun, all 7 pellets | per shot | health 88 / armour 49; 74 / 0; 11; dead on shot 4 |
| 32 | crowbar | hold attack against a 100-health player | – | hits at ticks 0, 27, 54, 81: dead at tick 81 (1.215 s) |
| 33 | stunstick | as 32 | – | hits at 0, 54, 108: dead at tick 108 (1.62 s) |
| 34 | attacker eye (0, 0, 64) looking +X; player origin (50, 0, 0) (box 34..66, −16..16, 0..72) | crowbar swing | – | line hits the box at x = 34; damage 25 exactly (generic hitgroup) |
| 35 | as 34, player origin (50, 20, 0) | swing | – | line misses (y 0 outside 4..36); box sweep hits; origin direction · f = 50 / 83.64 = 0.598 < 0.70721 → miss, no damage, miss animation |
| 36 | as 34, a 30 kg box prop ±10 centred at (50, 24, 64), origin at its centre | swing | – | line misses; box hits; d · f = 50 / 55.46 = 0.9015 → hit, 25 club damage, impulse 7500 along +X → Δv 250 in/s |
| 37 | 30 kg prop at its centre of mass | one bullet of each type | – | Δv along the shot: Pistol/SMG1/AR2 22.22, 357 362.76, Buckshot 43.53 per pellet (in/s) |
| 38 | `func_breakable` health 50 | pistol | – | 7 shots (8 each, script damage); shotgun with ≥ 6 pellets on it: 1 shot (sum ≥ 54) |
| 39 | owns SMG1, reserve 200 | touch a placed `weapon_smg1` (clip 45) | – | reserve 225; the touched weapon is removed (20 rounds left in it are lost); a copy appears at its spot after 20 s with `AlyxEmp.Charge` |
| 40 | owns no AR2 | touch a placed `weapon_ar2` | – | AR2 clip 30, AR2 reserve 30 |
| 41 | owns an AR2, AR2 reserve 0 | touch a placed `weapon_ar2` | – | reserve 60 (clip of the world object 60, limit 60) |
| 42 | owns SMG1 with reserve 225 | touch a placed `weapon_smg1` | – | refused; the weapon stays; the HUD gets an "ammo denied" notice for SMG1 on each touch attempt (H8) |
| 43 | player dies holding the pistol (clip 7), killed by one pistol bullet | – | – | a `weapon_pistol` at eye − 12 Z, speed 7.84 in/s along the bullet, clip 7; touchable from +1.0 s; removed at +31 s if untouched; all other weapons removed |
| 44 | player dies holding the crowbar / the physcannon | – | – | nothing dropped |
| 45 | a non-owner touches the pistol of 43 at +2 s | – | – | pistol clip 7, Pistol reserve unchanged |
| 46 | dead player's AR2 had clip 4; a non-owner takes it | – | – | clip 30, +30 reserve |
| 47 | `mp_weaponstay 1`, owns a shotgun | touch a placed shotgun | – | refused, no ammo |
| 48 | `mp_weaponstay 1`, owns none | touch it | – | taken; a copy is visible and touchable at the original spot on the next think |
| 49 | SMG1 clip 10, reserve 100 | switch to pistol at tick 0, keep pistol | – | SMG1 clip 45, reserve 65 at the first tick more than 3 s after the switch (tick 201); no animation; if reserve was 30: clip 40, reserve 0 |
| 50 | owns no shotgun, Buckshot 6, physcannon in hand | touch a dropped shotgun with clip 2 | – | no auto-switch (physcannon `autoswitchfrom` 0); next tick the holstered shotgun tops up: clip 6, Buckshot 2 |
| 51 | pistol in hand, `cl_autowepswitch 1` | pick up a shotgun | – | switch to shotgun (4 > 2); with `cl_autowepswitch 0`: no switch; a bot: switch |
| 52 | SMG1 in hand, eyes under water | press attack | – | empty sound, no shot, next primary +0.2 (14 ticks); pistol under water fires |
| 53 | tracer counter at 0 | 4 pistol shots, then 4 .357 shots | – | pistol tracers on shots 1 and 3; .357: counter 4 → tracer on its 1st shot only (4 % 4 = 0), next at counter 8 |
| 54 | SMG1 empty, no grenades; inventory crowbar, pistol (with ammo), physcannon; no buttons | – | timers past | switch to pistol (heaviest with ammo, physcannon skipped) |
| 55 | placed `item_battery` | taken at t | – | hidden; visible and touchable again at t + 30 s at its original spot |
| 56 | placed weapon pushed 100 units away, at rest, not taken | – | – | moved back to its spawn point at its next 20 s check, `AlyxEmp.Charge` |
| 57 | `phys_swap` with the pistol in hand | – | – | physcannon selected; `phys_swap` again → pistol (last weapon) |
| 58 | .357 clip 0, reserve 12, eyes under water | hold attack | – | empty click and "fired on empty" on the first tick, then a reload starts (as on land); no water refusal |
| 59 | SMG1 clip 10, eyes under water | hold attack and reload together | 1 s | no shot, no reload; empty sound every 0.2 s; release attack → the reload starts on the next tick |
| 60 | SMG1 with 1 grenade, eyes under water | hold attack2 for 3 ticks | – | empty sound on each of the 3 ticks; next secondary = now + 0.2; no dry-fire animation; grenade count unchanged |
| 61 | AR2, attack held | 7 shots, then release | – | `VentPoses` 0, 0, 0.2, …, written one tick behind the count; 1.0 from the tick after the 5th shot; 0 two ticks after the release tick |

Checks on the real game or the probe server (game build 24600434,
dedicated server build 24600426), each marked **probe**:

| # | How | Expected |
|---|---|---|
| P1 probe | 20 pistol shots at a wall 512 units away, clicking as fast as allowed, punch frozen or accounted | first shot within 4.5 units of the aim point; later shots spread up to ≈ 26.8 units (6°); **if all land on one point, H0 was right and §7.1's spread is wrong** |
| P2 probe | 10 .357 shots 2 s apart at 512 units | one point (no spread) |
| P3 probe | RCON `sk_player_head`, `sk_plr_dmg_pistol`, `mp_weaponstay`, `sv_hl2mp_weapon_respawn_time`, `sv_hl2mp_item_respawn_time`, `sk_auto_reload_time`, `phys_pushscale` | 3, 0, 0, 20, 30, 3, 1 |
| P4 probe | pistol at a `func_breakable` and a `prop_physics` of known health (with no damage multipliers) | 8 per bullet on both (Q6 of H0 settled) |
| P5 probe | spawn; read ammo counts | Pistol 150, SMG1 45, grenade **2** (or 1: then the spawn path differs, Open questions 2), Buckshot 6, 357 6 |
| P6 probe | log each shot's tick with attack held: pistol, .357, SMG1, AR2, shotgun primary and double | 33/34-tick, 50 (or 51), 5, 7-7-6, 59, 70 tick intervals |
| P7 probe | fastest clicks with the pistol (bot pressing every other tick) | 8-tick intervals |
| P8 probe | reload ticks from the reload command to the clip change: pistol, .357, SMG1, AR2; shotgun shells | 96, 245, 100 (or 101), 105; shotgun 34 then every 27 |
| P9 probe | weapon switch: tick of select → first possible shot, pistol and SMG1 | 54, 56 |
| P10 probe | kill a player holding each weapon; inspect the world | pistol/.357/SMG1/AR2/shotgun dropped with their clip; crowbar, stunstick, physcannon not; dropped weapon gone 31 s later |
| P11 probe | take a placed weapon; time its reappearance; same with an item | 20 s, 30 s |
| P12 probe | `mp_weaponstay 1`: touch a placed weapon you own / don't own | refused / taken and a copy remains |
| P13 probe | holster an SMG1 at clip 10, wait 3.1 s, switch back | clip 45 with no reload animation |
| P14 probe | crowbar a standing player whose box is 20 units to the side (line misses) | no hit; looking down at their feet from close: hit |
| P15 probe | crowbar and pistol at a player's head | 25 (no multiplier), 24 |
| P16 probe | shoot a standing player with the .357 (and the shotgun) and read their velocity | unchanged (bullets don't push players) |
| P17 probe | type `drop` in the console | unknown command / no effect |

## Open questions

1. **H0 corrections, to confirm on the probe**: (a) the pistol's spread
   (P1; H0 1.3 and Q7 said none); (b) damage to non-players is the script
   damage (P4; H0 1.3 and Q6 said the ammo type's); (c) the pistol draws
   a tracer every 2nd bullet (H0 1.10 named only SMG1 and AR2).
2. **Spawn grenades**: the SDK gives 1 from the ammo grant plus 1 from
   `weapon_frag`'s `default_clip`, i.e. 2; H0's test 19 says 1. P5
   decides; the SDK version may differ from build 24600434 here.
3. **Exact-multiple timers**: delays that are whole multiples of the
   tick (.357 0.75 s = 50 ticks, SMG1 refire 0.075 = 5, SMG1 reload 1.5
   = 100) can land a tick late from float rounding of the stored time.
   Chimera should compute in ticks with a small epsilon so these land on
   the exact multiple, unless P6/P8 show the extra tick.
4. **Exact random values**: the spread and kick patterns depend on the
   engine's uniform generator (CS:S Q1) and, for the shared-random
   punches, on per-use label texts not reproduced here. Tests should
   check ranges and seeding (same seed → same result), not the
   game's exact numbers.
5. **Box pellets against hitboxes**: the ±3 box traces use the engine's
   swept-box test against hitboxes (not in the SDK); assume a hit when
   the swept box touches a hitbox, hitgroup of that hitbox (H7).
6. **Bullets and players' velocity**: no code in the SDK pushes a living
   player on bullet or melee damage; P16 confirms.
7. **The world-origin melee quirk** (Quirks): follows from the code
   (the world is the hit entity, its origin is 0, 0, 0); confirm on a map
   where the origin lies in front of a wall (P14 variant).
8. **Holster time 0 of never-deployed weapons** relies on new server
   entities starting zeroed; confirm with test 50 on the probe.
9. **`.ctx` vs `.txt`** (H0 Q10): numbers here are from the `.txt`
   files.
10. **Does the dedicated server run `skill_manifest.cfg`?** H0 and this
    spec assume `sk_player_head` 3; P3 reads it. If the server leaves it
    at 2, every head number above changes (pistol head 16, five shots
    become seven).
11. **Probe results, round 2 (2026-11-03, dedicated server build
    24600426, `tools/hl2dm_probe/cases.py heads`).** A bot whose
    behaviour is stopped doesn't animate; its head hitbox sits low and to
    one side, so it is aimed at directly. P15: a pistol head hit 8 × 3 =
    24, a .357's 225; the crowbar 25 on the head (generic hitgroup, no
    multiplier). P14: at 40 ahead the crowbar still hits a player whose
    centre is 26 to the side (the box reaches) and misses at 36 (the box
    20 short); looking down at the feet from 30: a hit. P12: with
    `mp_weaponstay 1` a placed shotgun is taken while a copy stays at the
    spot; touched again while owned, it gives nothing. P4: bullets on a
    wood crate (`prop_physics`, health 30) take its prop data's bullet
    scale (pistol 8 → 6, SMG1 5 → 3 points), so P4's "8 on both" needs a
    prop without multipliers; `func_breakable` not checked. P16 (from
    round 1): bullets do push players, 5 a point (projectiles.md 3).
12. **AR2 vent blend direction.** `v_irifle.mdl`'s `VentPoses` sequence
    stores its own pose range as 1 → 0 while the pose parameter runs
    0 → 1; whether pose 0 shows closed vents (expected) depends on how the
    engine maps the sequence range, which is not in the SDK. Look at an
    AR2 after 5 held shots and at rest: vents open only while firing.
