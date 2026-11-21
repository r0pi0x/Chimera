# Third-person camera: over the shoulder

Goal: an opt-in third-person camera for every game in the engine, as
polished as a modern third-person shooter's. The camera sits over a
shoulder, so the centre of the screen is free. Shots go where the
crosshair points. A second marker shows where a blocked shot would land.
The camera never clips through walls, moves smoothly and zooms in to aim.
It is a Chimera addition, not the original games' behaviour: its settings
carry the Chimera mark. Done when the milestones below are in and each
one's tests pass. A playtest is the final check.

User request: 2026-11-21. The references (Fortnite, The Division) stand
for the genre's general conventions only. Nothing of theirs is copied.

## 1. What exists today

- **Camera.** `chimera/client/src/client/view.rs`. `CameraMode` holds
  `third_person`, `ideal_dist` (`cam_idealdist`, 150 game units),
  `ideal_yaw` (`cam_idealyaw`), `pivot_up` (0) and `meters_per_unit`.
  The camera turns about the eye and sits straight back along the view
  (`third_person_offset`). A 0.2 m sphere is swept back from the eye, and
  the camera stops where it hits (`camera_offset_for_hull`). It moves at
  once: no easing, no smoothing. This is CS:S's camera, measured for
  parity.
- **Toggle.** The console commands `thirdperson` / `firstperson`. They
  are not archived, and no Options row exists. The sandbox has a
  `Condition::ThirdPerson` and a camera distance
  (`chimera/sim/src/minigame/conditions.rs`, `client/sandbox/mod.rs`).
  `chimera:watchbot` and `chimera:freecam` reuse the same offset, and so
  does the spectator's chase camera (`client/spectate.rs`).
- **Placing the camera.** `follow_eye` in `client/mod.rs` builds the
  look from `Intent` yaw/pitch plus the view punch and roll. It turns
  the look by `cam_idealyaw`, then calls `camera_offset_for_hull`. In
  third person, the stair smoothing and the shot's back push are skipped.
- **Own body and gun.** `show_local_body` sets `map::ShowLocalBody` in
  third person (and in the free cam, watch cam and map cameras). The view
  model hides (`present/render/view_model.rs`), and so does first-person
  brass (`render/shells.rs`). The world body and its gun are drawn.
- **Shots.** Shots trace from the eye
  (`transform.translation + eye_offset`) along
  `aim_of(intent.yaw + kick.y, intent.pitch + kick.x)`
  (`chimera/sim/src/weapon/mod.rs`, `deliver::Shot`). The kick is the
  weapon's share of the punch. Spread comes from the command's seed, so
  a client's predicted shot is the server's. Other clients redraw others'
  shots from the same angles (`weapon/remote.rs`). Projectiles
  (`weapon/projectile.rs`), grenades and the games' own deliveries
  (Combat Arms' rounds and knife, Halo's weapons, HL2:DM's physcannon)
  also read `intent.yaw/pitch`.
- **Server checks.** Lag compensation (`weapon/lagcomp.rs`) rewinds the
  others to the shooter's view tick, then traces from the server's own
  eye. The server never takes a position from the client. It only takes
  the look angles in the command.
- **HUD aim.** `client/aim.rs` `view_ray` is the eye along the drawn
  view. Target IDs and name tags use it.
- **Vehicles.** Halo's vehicles have their own seat cameras
  (`games/halo_ce/src/game/vehicles/camera.rs`). This plan leaves them
  alone.

So today, in third person, the crosshair marks the eye's ray. The
character stands in front of it, and nothing on screen marks where the
shot goes.

## 2. The key decision: aim angles, not a camera position

The camera has its own look (the mouse's). Each command's
`Intent.yaw/pitch` is set from the camera on the client: the angles
from the eye to the crosshair's aim point. The aim point is where the
camera's ray through the crosshair first hits.

- **No change on the server, and no new trust.** The server already
  traces from its own eye along the command's angles. Lag compensation,
  spread seeds, prediction, others' redrawn shots, projectiles, grenades
  and every game's own deliveries keep working unchanged, and so do
  demos. The client could always send any angles, so this gives it
  nothing new.
- **No shooting around corners.** The shot starts at the eye the server
  places. If cover stands between the eye and the target, the shot hits
  the cover, even when the camera sees past it. The blocked-shot marker
  (M1) shows this before the player fires.
- **Movement follows the camera.** `move_axis` is relative to
  `Intent.yaw`. The client turns it by (camera yaw - aim yaw), so W
  still walks where the camera faces. Bodies face the aim, which is what
  the genre does.
- **Peeking** (the camera seeing round a corner the eye can't) is the
  genre's accepted trade. Servers limit it with two settings: a host
  switch that allows or forbids the mode (`sv_allow_thirdperson`), and a
  cap on the camera's offset from the eye (`sv_thirdperson_maxoffset`),
  sent to clients and enforced there. A cheating client can show itself
  any view in any mode, so the cap is for fair defaults, not security.
- **The aim ray.** The camera ray starts at the point on it nearest the
  eye, not at the camera. Anything between the camera and the character
  (a pole behind you, your own body) is skipped. Your own character and
  what `CHARACTER_FILTER` skips are excluded. With no hit, the aim point
  is the weapon's range along the ray (8192 game units without one).
  Others are traced where they are drawn (interpolated): that is where
  the server's rewind puts them.
- **No classic camera.** The behind-the-eye camera is gone for your
  own view (user decision 2026-11-21): `thirdperson` turns on the
  shoulder camera. `cam_idealdist` / `cam_idealyaw` stay for the chase
  cameras (spectating, `chimera:watchbot`); `cam_idealyaw` on your own
  camera orbits it as a debug view (the mouse aims as in first person).
- **How the server checks the aim.** It doesn't need to: the command
  carries only angles, as it always has. The server places the eye
  itself (its own position and `eye_offset`), traces along those angles
  with lag compensation, and the walls between that eye and the target
  stop the shot. A forged aim point gives a cheater nothing a forged
  angle didn't already. What the camera may see (peeking) is the clients'
  to enforce (`chimera:allow_thirdperson`, `chimera:thirdperson_maxoffset`).

## 3. Milestones

### M1. Aim through the crosshair, the blocked-shot marker

- [x] Shoulder camera. A pivot above the eye, then a side offset (right
      shoulder by default) and a distance back. Values in metres, scaled
      by nothing: one feel for every game's character. The heights are
      fractions of the character's eye height, so a Halo Spartan and a
      CS:S player frame the same.
- [x] Shoulder swap. `chimera:shoulder` on middle mouse (section 5).
      The swap slides over about 0.15 s.
- [x] Camera look separate from `Intent` angles. The mouse turns the
      camera. The command's angles are the eye-to-aim-point angles,
      computed when the command is built. Movement is turned as in
      section 2.
- [x] One `third_person::aim_point(camera, eye, ...)` in Chimera's
      client, shared by the command builder and the marker.
- [x] The HUD's target IDs follow the crosshair (`aim::crosshair_ray`
      from `Shoulder::ray`; CS:S's and Combat Arms' name tags).
- [x] Blocked-shot marker. Each frame, trace from the eye to the aim
      point, using the shot trace's filter. If something is hit short of
      the aim point (by more than a small tolerance), draw a second
      marker at that point on screen (dimming the crosshair: M4, each
      game draws its own). Hide the
      marker when the line is clear. Melee and thrown weapons show none.
- [x] The marker's trace is the server's: the same eye
      (`eye_offset`), the same angles, the same layers. Spread is not
      part of it: it marks the centre of the cone.
- [x] Host settings: `chimera:allow_thirdperson` (0) and
      `chimera:thirdperson_maxoffset` (3 m), replicated to clients (old
      names `sv_allow_thirdperson`, `sv_thirdperson_maxoffset` work).
- [x] Right mouse: tap to the gun's own zoom in first person, hold for
      the over-the-shoulder aim zoom (section 5, 2).
- [x] The first/third switch, `chimera:toggle_thirdperson` (unbound).

### M2. Camera collision (spring arm)

- [x] Sweep from the pivot to the wanted camera spot (the existing sphere
      sweep, from the pivot instead of the eye). On a hit, pull in at
      once. When the way clears, ease back out over about 0.3 s.
- [x] The pivot itself is swept from the eye (side, then up), so a wall
      at the shoulder pushes the camera toward the centre, not through
      the wall.
- [x] Never inside the character's own hull (the pivot and the camera
      are swept as spheres, 0.2 m, from the eye: tested in a corner). The
      body fades before the lens could reach it.

### M3. Smoothing, aim zoom, body fade, recoil, transitions

- [ ] Follow lag. The pivot trails the character's movement a little
      (critically damped spring, `chimera:cam_smooth`, 0 off). Rotation is
      never smoothed: aim stays one-to-one with the mouse. Crouch and land
      height changes ease.
- [ ] Aim zoom. While aiming (a hold), the camera moves closer and higher
      on the shoulder and the FOV narrows. Both ease over about 0.15 s.
      Mouse sensitivity scales with the FOV, as the zoom does today
      (`zoom_sensitivity_ratio`). A weapon with a scope switches to first
      person while scoped and comes back after: scope overlays are
      first-person.
- [x] Body fade (M2). When the camera is closer to the body than a threshold
      (pulled in by a wall), the local body fades with a dither (the
      props' dither prepass), down to hidden. The gun fades with it.
- [x] Recoil (M2). The view punch shakes the camera (pitch and yaw) as it does
      the first-person view. The crosshair stays on the aim. The shots
      keep the weapon's `punch_scale` share, as now.
- [x] Transitions (M2). First to third person: the camera moves from the eye
      to its spot over about 0.25 s, the body fades in and the view model
      out. Back again in reverse. Death, spectating and map cameras take
      over at once, as now.

### M2b. Polish, built with M2 (2026-11-21)

- [x] The crosshair dims while the marker shows (the shared one;
      `Shoulder::dim`). Combat Arms', TF2's, HL2:DM's and Halo's own
      crosshairs aren't dimmed yet.
- [x] Camera distances scale with the standing eye height
      (`Shoulder::scale`, `REF_EYE`), so every game's body frames alike.
- [x] Options: Controls tab, "Third-person camera" (shoulder side,
      distance; Chimera mark), and "Keys, every game": the switch
      (unbound) and the shoulder swap (middle mouse; unbound where the
      game binds the key: TF2).
- [x] Network test: `chimera/client/tests/it/third_person_net.rs`.

### M4. Settings

- [ ] A data file of defaults and tuning (Chimera's client data:
      offsets, distances, aim-zoom offsets and FOV, spring-arm speeds,
      fade distances, marker look). The theme holds the marker's colours
      and shape.
- [ ] Archived cvars: `chimera:thirdperson` (the mode on or off),
      `chimera:cam_shoulder` (right or left), `chimera:cam_dist`,
      `chimera:cam_fov` (the third-person FOV), `chimera:cam_smooth`.
- [ ] A "Camera" group in `apps/party_kit/data/options.toml`. Each row is
      marked as a Chimera option (the UI kit's origin field: the Chimera
      icon, plus a line in the help tile saying the original game has no
      such setting).
- [ ] The sandbox's `ThirdPerson` condition turns on the new style.

## 4. Tests and checks

Unit tests (pure functions, in `client/view.rs` or a new
`client/third_person.rs`):

- M1: the shoulder offset for a look; the aim ray skips what is between
  the camera and the eye; the aim angles hit the aim point from the eye;
  the movement turn keeps the walk along the camera's facing.
- M2: the spring arm pulls in on the frame of the hit and eases out at
  the set rate.
- M3: the follow spring settles without overshoot; the aim zoom's offset
  and FOV at 0, half and full.

Scenario tests (`harness::Sim`, `chimera/client/tests/it/third_person.rs`):

- M1: a wall's edge between the eye and a target the camera sees. The
  marker shows on the wall and the crosshair dims; a shot hits the wall,
  not the target. Step out: the marker hides and the shot hits.
- M1: in the open, a third-person shot lands where the crosshair points,
  near and far.
- M2: back into a corner: the camera never ends inside a wall or the
  body.
- M3: switching modes leaves the camera at the eye in first person and
  at its spot in third, after the transition time.

Network tests (`NetSim`, `chimera/net/tests/it/third_person_shots.rs`):

- M1: a client in third person fires. The server's hit equals the
  client's predicted hit and the marker's point.
- M1: the corner case over the network: the client's camera sees the
  target, the server's shot hits the cover.
- M1: `sv_allow_thirdperson 0` turns a client's mode off;
  `sv_thirdperson_maxoffset` caps its camera.

Visual checks (offscreen, docs/OBSERVABILITY.md section 2):

- `--offscreen 1920x1080 --frames N --dump <dir>` with the mode on, at a
  fixed `setpos`/`setang` on de_dust2 and the greybox: right shoulder,
  left shoulder, aim zoom, against a wall (pulled in, body faded), the
  marker on a wall edge. Before and after shots with the
  screenshot-and-compare skill.
- One run per game's character (CS:S, Combat Arms, HL2:DM, TF2, Halo) to
  check the framing scales with eye height.

## 5. Decisions (user, 2026-11-21)

1. **Shoulder swap: middle mouse** (`mouse3`, `chimera:shoulder`), a
   Chimera bind placed only where the game leaves the key free. Not Q
   (previous weapon in CS:S, HL2:DM, TF2, Combat Arms; Halo's grenade
   switch). Not Left Alt: it is our `+freelook` (and PUBG's free look),
   HL2:DM's `+walk` and TF2's `+strafe`. Precedent: Ghost Recon
   Breakpoint and Wildlands swap on middle mouse; PUBG uses Q/E (its lean
   keys in first person), Gears of War: E-Day Left Shift, The Last of Us
   Tab, ARC Raiders X; Apex has no third person. Middle mouse is free in
   CS:S, Combat Arms, HL2:DM and Halo; TF2 binds it to `+attack3`, so
   there the swap is unbound until the player picks a key.
2. **Right mouse** (`+attack2`) on a gun whose right click aims or zooms
   (`weapon::Zoom`, or a game's `OwnZoom`: Combat Arms' sights, HL2:DM's
   crossbow, Halo's guns): a tap goes to first person and gives the gun
   its click (its own ADS or scope; first person while that zoom lasts,
   back to third when it ends); a hold zooms over the shoulder in third
   person (closer, narrower FOV, slower mouse) and the gun gets nothing.
   Guns whose right click is something else (TF2's Minigun spin and other
   secondary fire, a knife's stab, CS:S's burst and silencer) keep their
   own right click.
3. **Off on servers by default**: `chimera:allow_thirdperson 0` (alias
   `sv_allow_thirdperson`), the host's to turn on; not a cheat setting.
   Single player always may.
4. **The classic behind-the-character camera is removed** (no style
   switch).
5. **The first/third switch** (`chimera:toggle_thirdperson`) is bindable
   and unbound by default.
6. **Later (plan only): free look around your own or any player's
   model.** A mode that stops controlling the character and orbits a
   chosen player's model (your own, a teammate, anyone), with the mouse
   orbiting and the wheel zooming, for looking at models and outfits.
   It would build on `chimera:freecam` and the spectator's chase camera,
   and replace `cam_idealyaw` as the way to look at your own front.

## Progress

2026-11-21: plan written.

2026-11-21: open questions answered (section 5). M1 built, with M2's
sweeps and M3's aim zoom: `chimera/client/src/client/third_person.rs`,
scenario tests `chimera/client/tests/it/third_person.rs`. Left: the
target-ID ray, M2's "never inside the own hull" and near-plane margin,
M3's follow lag, body fade, recoil and transitions, M4's settings page.

2026-11-21 (M2 and polish): the target ID follows the crosshair, the
crosshair dims at the marker, the camera follows the recoil, the
distances scale with eye height, the body fades by a dither band on the
camera's distance (`VisibilityRange`, the props' mechanism), the camera
glides between eye and shoulder over 0.25 s (input and aim follow the
target mode at once; the view model hides at once on entering, shows when
the glide ends on leaving), Options rows, network tests. Left: M3's
follow lag and aim-zoom scope switch, M4's data file and cvars
(`cam_fov`, `cam_smooth`, the `thirdperson` cvar), the own crosshairs'
dim in Combat Arms, TF2, HL2:DM and Halo.

## Decision log

2026-11-21: the user's answers (section 5): middle mouse swaps, right
mouse taps to the gun's zoom and holds for the camera's, off on servers
by default, no classic camera, the switch unbound, free look later.

2026-11-21: aim by angles. The client sends the eye-to-aim-point angles
in the existing command. The server traces from its own eye, so it
needs no new data or checks, and cover still blocks shots. Chosen over
sending the camera's position, which the server would have to bound and
trust.
