//! HL2:DM's suit (specs/hl2dm/suit.md, H2): every HL2:DM character wears
//! it (`Suit`, put on with the movement: `wear`). Health keeps fractional
//! damage in a remainder that survives death (`core::DamageRemainder`).
//! Armour (`weapon::Armor`, 0 at spawn) takes 80 % of every damage class
//! but fall, drown, poison and radiation, at one armour point per point
//! absorbed (two against blasts), whole points lost (`SUIT`, through the
//! shared armour step `weapon::hits::absorb`: lift C2). Aux power (0-100)
//! feeds the devices: the sprint (25/s while moving, `super::movement::
//! select_speed`) and the breather (6.7/s with the eyes under water);
//! switching one off costs a tenth of its drain at once, and it recharges
//! at 12.5/s half a second after the last one stops. While aux power lasts
//! the player breathes under water; then the air lasts 7 s and drowning
//! takes 10 a second (`drowning`), given back 10 every 2 s after
//! surfacing. The flashlight (`impulse 100`: `weapon::PlayerImpulse`, a
//! network client's through `net`'s `ImpulseRequest`, or the `Flashlight`
//! message) costs nothing and needs `hl2dm:mp_flashlight` (0 by default);
//! it replicates (`FlashlightOn`) and shows as a spot light along the
//! view (`flashlight_lights`). The suit zoom (`+zoom`) narrows the view to
//! 25 over 0.4 s (`weapon::ViewZoom`) and blocks firing while held;
//! `hl2dm:toggle_zoom` works it without holding a key. It has one owner:
//! not while the crossbow's scope holds the view, nor that while it does.
//!
//! Known shortcuts:
//! - Of the devices a player dies with only the sprint is kept to the
//!   respawn (measured, P9); the breather stuck on after a death under
//!   water (suit.md Quirks) isn't measured and is cleared.
//! - The suit zoom's FOV is stepped per tick on the simulation's clock
//!   (the client evaluates it at its render time).
//! - The flashlight's light starts at the eye, not the model's right-hand
//!   attachment suit.md gives for others, for every player. The spec gives
//!   no brightness, colour, cone or reach: ours are picked to look right
//!   (white, 2 lightmap units, a 45 degree cone, 750 units).
//! - Drowning's sounds play from the player; the server's hurt event
//!   isn't sent. A `trigger_hurt`'s poison and radiation types are their
//!   own kinds (`DamageKind::Poison`, `Radiation`) and bypass the suit
//!   (dm_underpass's 75); the other types it deals are generic.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use source_engine::source_movement::{SourceMovement, SourceSpeed, StepSounds};

use crate::{
    core::{
        ActionRegistry, Damage, DamageKind, DamageRemainder, Dead, Health, Hitgroup, Intent, SimClock,
        SimSet,
    },
    map::PlaySound,
    slots::MovementSlot,
    weapon::{Armor, ArmorPieces, Inventory, OwnZoom, PlayerImpulse, ViewZoom, Zoomed, hits},
};

/// Device bits (`Suit::devices`).
pub const SPRINT: u8 = 1;
pub const BREATHER: u8 = 2;
pub const FLASHLIGHT: u8 = 4;
/// A horizontal velocity component at most this (units/s) is "not moving".
pub const STILL: f32 = 0.001;
pub const AUX_MAX: f32 = 100.0;
const RECHARGE: f32 = 12.5;
const RECHARGE_DELAY: f64 = 0.5;
const SPRINT_DRAIN: f32 = 25.0;
const BREATHER_DRAIN: f32 = 6.7;
/// Below this after a drain, aux power is 0.
const EMPTY: f32 = 0.01;
/// Air after the aux power runs out, seconds; drowning per hit, points;
/// between hits; given back per check; between checks.
pub const AIR: f64 = 7.0;
pub const DROWN_DAMAGE: f32 = 10.0;
const DROWN_INTERVAL: f64 = 1.0;
const RESTORE: f32 = 10.0;
const RESTORE_INTERVAL: f64 = 2.0;
/// The suit zoom: FOV, in and out times; the default FOV (`fov_desired`).
pub const ZOOM_FOV: f32 = 25.0;
pub const ZOOM_IN: f32 = 0.4;
pub const ZOOM_OUT: f32 = 0.2;
pub const DEFAULT_FOV: f32 = 75.0;
/// The zoom key (`+zoom`), a named action.
pub const ZOOM_ACTION: &str = "zoom";
/// Health is normalized: 1.0 is 100 points.
const POINTS: f32 = 100.0;
/// Two times this close are the same (the clock's float noise).
const SAME: f64 = 1e-6;

/// The suit's armour as coverage (suit.md "Armour"): 80 % of every class
/// on every part, one armour point per point taken, except the classes
/// that bypass it; blasts at two points per point.
pub fn suit_armour() -> ArmorPieces {
    let all = |class, wear, except| hits::Coverage {
        class,
        head: 0.8,
        torso: 0.8,
        arm: 0.8,
        leg: 0.8,
        groin: 0.8,
        wear,
        except,
    };
    let bypass = [
        hits::DamageClass::FALL,
        hits::DamageClass::DROWN,
        hits::DamageClass::POISON,
        hits::DamageClass::RADIATION,
        hits::DamageClass::BLAST,
    ];
    ArmorPieces {
        pieces: vec![hits::ArmorPiece {
            entries: vec![
                all(None, 1.0, hits::Coverage::except_of(&bypass)),
                all(Some(hits::DamageClass::BLAST), 2.0, 0),
            ],
        }],
        depletion: hits::Depletion::Cost,
        // Whole armour points lost; health keeps the fraction (its
        // remainder).
        armour_quantum: Some(0.01),
    }
}

/// The suit (suit.md "Aux power", "Sprint"), predicted.
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct Suit {
    pub worn: bool,
    pub aux: f32,
    /// Active devices (`SPRINT`, `BREATHER`).
    pub devices: u8,
    /// The tick the last device stopped.
    pub off_tick: u64,
    pub sprinting: bool,
    /// `+speed` was held last tick.
    pub speed_held: bool,
    /// `+zoom` was held last tick, and the zoom now: from `from` at
    /// `since` (seconds) to `to` over `time`.
    pub zoom_held: bool,
    pub zoom: Option<ZoomMove>,
}

impl Default for Suit {
    fn default() -> Self {
        Self {
            worn: true,
            aux: AUX_MAX,
            devices: 0,
            off_tick: 0,
            sprinting: false,
            speed_held: false,
            zoom_held: false,
            zoom: None,
        }
    }
}

/// A FOV transition (suit.md "Suit zoom").
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZoomMove {
    pub from: f32,
    pub to: f32,
    pub since: f64,
    pub time: f32,
}

impl ZoomMove {
    /// The FOV at `now`: a smooth step from `from` to `to`.
    pub fn fov(&self, now: f64) -> f32 {
        let x = (((now - self.since) / f64::from(self.time)) as f32).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * (3.0 * x * x - 2.0 * x * x * x)
    }
}

fn drain(device: u8) -> f32 {
    match device {
        SPRINT => SPRINT_DRAIN,
        BREATHER => BREATHER_DRAIN,
        _ => 0.0,
    }
}

impl Suit {
    /// The active devices' drain, per second.
    pub fn load(&self) -> f32 {
        [SPRINT, BREATHER].iter().filter(|d| self.devices & **d != 0).map(|d| drain(*d)).sum()
    }

    /// Switch a device on (free).
    pub fn attach(&mut self, device: u8) {
        if self.worn && self.devices & device == 0 {
            self.devices |= device;
        }
    }

    /// Switch a device off: a tenth of its drain at once; with none left,
    /// the recharge delay starts.
    pub fn detach(&mut self, device: u8, tick: u64, infinite: bool) {
        if !self.worn || self.devices & device == 0 {
            return;
        }
        if !infinite {
            self.aux = (self.aux - 0.1 * drain(device)).max(0.0);
            if self.aux < EMPTY {
                self.aux = 0.0;
            }
        }
        self.devices &= !device;
        if self.devices == 0 {
            self.off_tick = tick;
        }
    }

    /// The tick's recharge or drain (`moving`: the tick started moving
    /// horizontally; a standing sprint drains nothing).
    pub fn update_aux(&mut self, moving: bool, tick: u64, dt: f32, infinite: bool) {
        if self.devices == 0 {
            let since = tick.saturating_sub(self.off_tick) as f64 * f64::from(dt);
            if self.aux < AUX_MAX && since >= RECHARGE_DELAY - SAME {
                self.aux = (self.aux + RECHARGE * dt).min(AUX_MAX);
            }
            return;
        }
        let mut load = self.load();
        if self.devices & SPRINT != 0 && !moving {
            load -= SPRINT_DRAIN;
        }
        if infinite {
            return;
        }
        self.aux -= load * dt;
        if self.aux < EMPTY {
            self.aux = 0.0;
        }
    }
}

/// `sv_infinite_aux_power` (a cheat): drains do nothing.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InfiniteAux(pub bool);

/// `mp_flashlight`: the flashlight may be switched on.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct FlashlightAllowed(pub bool);

/// A player's breath (suit.md "Breath under water"), server-side.
#[derive(Component, Clone, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Breath {
    /// When the air runs out, and when the next drowning hit may come
    /// (seconds).
    pub air_deadline: f64,
    pub pain_deadline: f64,
    /// Drowning damage taken and given back, points.
    pub taken: f32,
    pub returned: f32,
    /// Giving it back, and the last time-based check.
    pub recovering: bool,
    pub last_check: f64,
    started: bool,
}

/// `impulse 100`: toggle `player`'s flashlight.
#[derive(Message, Clone, Copy, Debug)]
pub struct Flashlight(pub Entity);

/// `toggle_zoom` asked (the command's, on the server): the zoom system
/// starts or stops the suit zoom next tick.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZoomToggle;

/// The flashlight is on (its dim light; others see a beam, H9).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashlightOn;

/// The model's footsteps until the player models come (M8): a citizen's.
pub fn citizen_steps() -> StepSounds {
    StepSounds {
        left: "NPC_Citizen.RunFootstepLeft".into(),
        right: "NPC_Citizen.RunFootstepRight".into(),
    }
}

pub struct SuitPlugin;

impl Plugin for SuitPlugin {
    fn build(&self, app: &mut App) {
        use crate::core::{ActionAppExt, PredictedAppExt};
        use chimera_net::net::parts::NetAppExt;
        app.init_resource::<InfiniteAux>()
            .init_resource::<FlashlightAllowed>()
            .add_message::<Flashlight>()
            .add_message::<PlayerImpulse>()
            .add_message::<PlaySound>()
            .add_message::<Damage>()
            .intent_action(ZOOM_ACTION)
            .predicted_net::<Suit>()
            .net_part("hl2dm_flashlight", |app| {
                use bevy_replicon::prelude::AppRuleExt;
                app.replicate::<FlashlightOn>();
            })
            .add_systems(Update, flashlight_lights)
            .add_systems(FixedUpdate, wear.before(SimSet::Movement))
            .add_systems(
                crate::core::Predict::Movement,
                (breath_devices, zoom)
                    .chain()
                    .before(super::movement::select_speed)
                    .before(source_engine::source_movement::SourceMovementStep),
            )
            .add_systems(
                FixedUpdate,
                (spawned, drowning, flashlight)
                    .chain()
                    .after(wear)
                    .before(SimSet::Movement)
                    .run_if(crate::core::authoritative),
            );
        crate::console::ConsoleAppExt::console_command(
            app,
            &format!("{}:toggle_zoom", super::GAME),
            "Start or stop the suit zoom (unlike +zoom, firing stays allowed).",
            |w, _| {
                let e = crate::console::command_player(w)?;
                w.entity_mut(e).insert(ZoomToggle);
                Ok(None)
            },
        );
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .player_command(&format!("{}:toggle_zoom", super::GAME));
        for (name, help, get, set) in [
            (
                "mp_flashlight",
                "1: players may use the flashlight (0 by default)",
                (|w: &World| w.get_resource::<FlashlightAllowed>().map(|f| f.0)) as fn(&World) -> Option<bool>,
                (|w: &mut World, v: bool| w.insert_resource(FlashlightAllowed(v))) as fn(&mut World, bool),
            ),
            (
                "sv_infinite_aux_power",
                "1: aux power never drains (a cheat)",
                |w: &World| w.get_resource::<InfiniteAux>().map(|f| f.0),
                |w: &mut World, v: bool| w.insert_resource(InfiniteAux(v)),
            ),
        ] {
            crate::console::ConsoleAppExt::console_cvar(
                app,
                &format!("{}:{name}", super::GAME),
                help,
                "0",
                move |w| get(w).map(|b| if b { "1" } else { "0" }.to_string()),
                move |w, v| {
                    let n = v.trim().trim_matches('"').parse::<f32>().map_err(|_| format!("{name}: not a number: {v}"))?;
                    set(w, n != 0.0);
                    Ok(())
                },
            );
        }
    }
}

/// HL2:DM's characters (on its movement) wear the suit: `Suit`, its armour
/// (`SUIT`), the health remainder, its speed and footsteps; a character
/// that moves another game's way takes them off (the remainder stays: it
/// is the character's).
#[allow(clippy::type_complexity)]
fn wear(
    mut commands: Commands,
    on: Query<
        (
            Entity,
            &MovementSlot,
            Has<Suit>,
        ),
        Or<(Changed<MovementSlot>, Added<Suit>)>,
    >,
) {
    for (e, slot, suit) in &on {
        let mut ent = commands.entity(e);
        if slot.0 == super::movement::ID {
            // The parts missing, each kept where it is (a network client's
            // suit may come first with the server's state), in one move:
            // each step is an archetype of its own.
            ent.insert_if_new((
                Suit::default(),
                Breath::default(),
                suit_armour(),
                SourceSpeed(190.0),
                citizen_steps(),
                Armor::default(),
                DamageRemainder::default(),
            ));
        } else if suit {
            ent.remove::<(Suit, Breath, ArmorPieces, SourceSpeed, StepSounds, ViewZoom, FlashlightOn)>();
        }
    }
}

/// A player back in play (no longer `Dead`): health 100 (`revive`),
/// armour 0, aux power 100, the suit on, the view's zoom and the
/// flashlight off, fresh air, unreturned drowning forgotten; the
/// remainder kept (suit.md "Death and spawn"). A sprint running at the
/// death is still on (suit.md Quirks, measured: the real server's
/// respawned sprinter pays the 2.5 detach on its first tick, aux 97.5).
fn spawned(
    mut revived: RemovedComponents<Dead>,
    mut q: Query<(&mut Suit, &mut Breath, &mut Armor)>,
    mut commands: Commands,
    clock: Res<SimClock>,
) {
    for e in revived.read() {
        let Ok((mut suit, mut breath, mut armor)) = q.get_mut(e) else { continue };
        *suit = Suit {
            off_tick: clock.tick,
            devices: suit.devices & SPRINT,
            sprinting: suit.sprinting,
            speed_held: suit.speed_held,
            ..Suit::default()
        };
        *breath = Breath::default();
        armor.amount = 0.0;
        commands.entity(e).remove::<(ViewZoom, FlashlightOn)>();
    }
}

/// The breather (suit.md "Breath under water"): on while the eyes are
/// under water (the last movement's water level 3), off (costing 0.67)
/// when they come out; predicted.
pub fn breath_devices(
    mut q: Query<(&SourceMovement, &mut Suit, &Health)>,
    clock: Res<SimClock>,
    infinite: Option<Res<InfiniteAux>>,
) {
    let infinite = infinite.is_some_and(|i| i.0);
    for (me, mut suit, health) in &mut q {
        if health.current <= 0.0 {
            continue;
        }
        let under = me.water_level >= 3;
        let on = suit.devices & BREATHER != 0;
        if under && !on {
            suit.attach(BREATHER);
        } else if !under && on {
            suit.detach(BREATHER, clock.tick, infinite);
        }
    }
}

/// Breathing, drowning hits and giving them back (suit.md "Breath under
/// water"), before this tick's movement drains the aux power.
fn drowning(
    mut q: Query<(Entity, &SourceMovement, &Suit, &mut Breath, &mut Health, &Transform)>,
    clock: Res<SimClock>,
    mut damage: MessageWriter<Damage>,
    mut play: MessageWriter<PlaySound>,
) {
    let now = clock.now;
    for (e, me, suit, mut b, mut health, t) in &mut q {
        let mut sound = |entry: &str| {
            play.write(PlaySound {
                entry: entry.into(),
                at: Some(t.translation),
                volume: None,
                pitch: None,
                source: Some(e),
                channel: None,
            });
        };
        if !b.started {
            b.air_deadline = now + AIR;
            b.started = true;
        }
        if health.current <= 0.0 {
            continue;
        }
        let breathing = me.water_level < 3 || suit.aux > 0.0;
        if breathing {
            if now > b.air_deadline + SAME {
                sound("Player.DrownStart");
            }
            b.air_deadline = now + AIR;
            if b.taken > b.returned {
                b.recovering = true;
            }
        } else {
            b.recovering = false;
            if now > b.air_deadline + SAME && now > b.pain_deadline + SAME {
                let first = b.taken <= b.returned;
                damage.write(Damage {
                    target: e,
                    attacker: None,
                    amount: DROWN_DAMAGE / POINTS,
                    point: t.translation,
                    dir: Vec3::ZERO,
                    hitgroup: Hitgroup::Generic,
                    kind: DamageKind::Drown,
                    weapon: None,
                    force: Vec3::ZERO,
                });
                b.pain_deadline = now + DROWN_INTERVAL;
                b.taken += DROWN_DAMAGE;
                sound(if first { "Player.DrownStart" } else { "Player.DrownContinue" });
            }
        }
        // The time-based check: at most once every 2 s while giving back.
        if b.recovering && (now - b.last_check).abs() >= RESTORE_INTERVAL - SAME {
            b.last_check = now;
            let give = RESTORE.min(b.taken - b.returned);
            b.returned += give;
            health.current = (health.current + give / POINTS).min(health.max);
            if b.returned >= b.taken {
                b.recovering = false;
            }
        }
    }
}

/// A lit flashlight's spot light, following `.0`'s view.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashlightBeam(pub Entity);

/// How far the flashlight reaches (units; picked, the spec has none).
pub const FLASHLIGHT_REACH: f32 = 750.0;
/// Its light at the source, in lightmap units (picked; the map's shaders
/// read colour x intensity / 4 pi as those, `dynamic_light` in world.wesl).
pub const FLASHLIGHT_LIGHT: f32 = 2.0;
/// Its cone, the whole angle (degrees; picked).
pub const FLASHLIGHT_CONE: f32 = 45.0;

/// One light for each player with the flashlight on (suit.md
/// "Flashlight"): at the eye, along the view (the others' as drawn).
#[allow(clippy::type_complexity)]
fn flashlight_lights(
    lit: Query<(Entity, &Transform, &crate::core::MovementState, &Intent, Option<&crate::map::interp::RenderedView>, Has<crate::core::LocalPlayer>), With<FlashlightOn>>,
    mut beams: Query<(Entity, &FlashlightBeam, &mut Transform), Without<FlashlightOn>>,
    mut commands: Commands,
) {
    let mut seen: Vec<Entity> = Vec::new();
    for (e, beam, mut t) in &mut beams {
        let Ok((_, at, state, intent, view, local)) = lit.get(beam.0) else {
            commands.entity(e).despawn();
            continue;
        };
        seen.push(beam.0);
        let look = crate::map::interp::eye_view(if local { None } else { view }, intent, state);
        t.translation = at.translation + look.eye_offset;
        t.rotation = crate::weapon::aim_of(look.yaw, look.pitch);
    }
    for (player, at, state, intent, view, local) in &lit {
        if seen.contains(&player) {
            continue;
        }
        let look = crate::map::interp::eye_view(if local { None } else { view }, intent, state);
        commands.spawn((
            Name::new("Flashlight"),
            FlashlightBeam(player),
            SpotLight {
                intensity: FLASHLIGHT_LIGHT * 4.0 * std::f32::consts::PI,
                range: FLASHLIGHT_REACH * super::weapons::launch::U,
                outer_angle: FLASHLIGHT_CONE.to_radians() / 2.0,
                inner_angle: (FLASHLIGHT_CONE * 0.6).to_radians() / 2.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(at.translation + look.eye_offset).with_rotation(crate::weapon::aim_of(look.yaw, look.pitch)),
        ));
    }
}

/// `impulse 100` (suit.md "Flashlight"): off always works (silent when
/// dead); on needs `mp_flashlight` and a living player. Death turns it
/// off silently.
#[allow(clippy::too_many_arguments)]
fn flashlight(
    mut toggles: MessageReader<Flashlight>,
    mut impulses: MessageReader<PlayerImpulse>,
    q: Query<(&Health, &Transform, Has<FlashlightOn>), With<Suit>>,
    lit: Query<(Entity, &Health), With<FlashlightOn>>,
    allowed: Res<FlashlightAllowed>,
    mut commands: Commands,
    mut play: MessageWriter<PlaySound>,
) {
    for (e, h) in &lit {
        if h.current <= 0.0 {
            commands.entity(e).remove::<FlashlightOn>();
        }
    }
    let asked: Vec<Entity> = toggles
        .read()
        .map(|f| f.0)
        .chain(impulses.read().filter(|i| i.impulse == 100).map(|i| i.player))
        .collect();
    for e in &asked {
        let Ok((h, t, on)) = q.get(*e) else { continue };
        let alive = h.current > 0.0;
        let entry = if on {
            commands.entity(*e).remove::<FlashlightOn>();
            alive.then_some("HL2Player.FlashlightOff")
        } else if allowed.0 && alive {
            commands.entity(*e).insert(FlashlightOn);
            Some("HL2Player.FlashlightOn")
        } else {
            None
        };
        if let Some(entry) = entry {
            play.write(PlaySound {
                entry: entry.into(),
                at: Some(t.translation),
                volume: None,
                pitch: None,
                source: Some(*e),
                channel: None,
            });
        }
    }
}

/// The suit zoom (suit.md "Suit zoom"): `+zoom` pressed narrows the view
/// to 25 over 0.4 s, released widens it back over 0.2 s, from the FOV of
/// the moment truncated to a degree; a weapon switch or death ends it;
/// while `+zoom` is held attack and attack2 do nothing. The view's zoom is
/// `ViewZoom` while it differs from the default. `toggle_zoom`
/// (`ZoomToggle`) starts or stops it without holding the key. One zoom
/// owner: the suit zoom can't start while the crossbow's scope (a weapon
/// with `OwnZoom`, its owner `Zoomed`) holds the view, and while the suit
/// zoom holds, such a weapon's attack2 (its zoom toggle) is refused.
#[allow(clippy::type_complexity)]
fn zoom(
    mut q: Query<(
        Entity,
        &mut Suit,
        &mut Intent,
        &Health,
        Option<&Inventory>,
        Option<&ViewZoom>,
        Has<Zoomed>,
        Has<ZoomToggle>,
    )>,
    own_zooms: Query<(), With<OwnZoom>>,
    registry: Res<ActionRegistry>,
    clock: Res<SimClock>,
    mut last_weapon: Local<bevy::platform::collections::HashMap<Entity, Option<Entity>>>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut suit, mut intent, health, inv, view, zoomed, toggle) in &mut q {
        let held = intent.action(&registry, ZOOM_ACTION) && health.current > 0.0;
        let active = inv.and_then(|i| i.active);
        let switched = last_weapon.insert(e, active).is_some_and(|w| w != active);
        let current = suit.zoom.map_or(DEFAULT_FOV, |z| z.fov(now));
        let zoomed_in = suit.zoom.is_some_and(|z| z.to < DEFAULT_FOV);
        // Another owner (the crossbow's scope) holds the zoom.
        let scoped = active.is_some_and(|a| own_zooms.contains(a));
        let other_owner = zoomed && scoped && !zoomed_in;
        if toggle {
            commands.entity(e).remove::<ZoomToggle>();
        }
        let start = ((held && !suit.zoom_held) || (toggle && !zoomed_in)) && health.current > 0.0;
        if health.current <= 0.0 && suit.zoom.is_some() {
            suit.zoom = None;
        } else if start && !other_owner {
            suit.zoom = Some(ZoomMove {
                from: current.trunc(),
                to: ZOOM_FOV,
                since: now,
                time: ZOOM_IN,
            });
        } else if zoomed_in && ((!held && suit.zoom_held) || switched || toggle) {
            suit.zoom = Some(ZoomMove {
                from: current.trunc(),
                to: DEFAULT_FOV,
                since: now,
                time: ZOOM_OUT,
            });
        }
        suit.zoom_held = held;
        if held {
            intent.fire = false;
            intent.secondary = false;
        }
        // The crossbow can't zoom while the suit zoom holds.
        if scoped && suit.zoom.is_some_and(|z| z.to < DEFAULT_FOV) {
            intent.secondary = false;
        }
        let fov = suit.zoom.map_or(DEFAULT_FOV, |z| z.fov(now));
        let want = (fov < DEFAULT_FOV).then_some(ViewZoom(fov));
        if view.copied() != want {
            match want {
                Some(v) => commands.entity(e).insert(v),
                None => commands.entity(e).remove::<ViewZoom>(),
            };
        }
        if suit.zoom.is_some_and(|z| z.to >= DEFAULT_FOV && now - z.since >= f64::from(z.time)) {
            suit.zoom = None;
        }
    }
}

/// Is the line from `from` to `to` (engine space) clear of the world and
/// props (characters and `ignore` aside)?
pub fn clear_line(spatial: &SpatialQuery, from: Vec3, to: Vec3, ignore: &[Entity]) -> bool {
    let Ok((dir, len)) = Dir3::new_and_length(to - from) else { return true };
    let filter = SpatialQueryFilter::from_excluded_entities(ignore.iter().copied()).with_mask(crate::core::SOLID_LAYERS);
    spatial.cast_ray(from, dir, len, true, &filter).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_follows_a_smooth_step() {
        let z = ZoomMove {
            from: 75.0,
            to: 25.0,
            since: 0.0,
            time: 0.4,
        };
        for (t, fov) in [(0.1, 67.1875), (0.2, 50.0), (0.3, 32.8125), (0.4, 25.0), (0.5, 25.0)] {
            assert!((z.fov(t) - fov).abs() < 1e-3, "{t}: {}", z.fov(t));
        }
    }
}
