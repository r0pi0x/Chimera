//! Combat Arms' player characters (specs/combat_arms/characters.md;
//! section numbers below are its): which body a player's selection draws,
//! the third-person animation state, the posed skeleton with its hit
//! spheres and sockets, death animations and bodies, and the network's
//! copy of it all for other players.
//!
//! - `records`: data/characters.toml (bodies, head pieces, goggles, team
//!   marks, each weapon's `AniType`, the skeletons with their hit tables
//!   and sockets) and the model choice (1, 2): cosmetics, the face
//!   redirect, skins by camouflage index, team marks, head pieces and
//!   goggles by skeleton and figure type. A player's selection is
//!   `CaLook`: the local player's `CaOwnLook` (the console's `ca_model`,
//!   `ca_face`, `ca_head`, `ca_camo`), a bot's drawn from the loadout
//!   pool (`CharacterData::bot_look`), else the default; a ladder rung's
//!   look over any of them (`CaRungLook`, `game::rungs`). A body no stock
//!   group has loads when first worn (`bodies_on_demand`). The same choice
//!   is the player's gear (gear_perks.md, `gear`): the body record is the
//!   vest, the head piece the helmet, the goggle and backpack as such.
//! - `driver`: the animation driver (4-7, 9) on LithTech's millisecond
//!   trackers: posture and movement from the weapon status, the fire
//!   buttons and the movement flags; names from the class code, stance,
//!   action and direction; starting, restarting, the torso kept in step
//!   with the legs, the upper rate from `HHAniFrame`; hit reactions.
//! - `skeleton`: the four trackers layered by the body's weight sets (4),
//!   the aim's bone rotations (8), hit-sphere centres on nodes or sockets
//!   (11) and sockets (3); without a body model, the spec's skeleton as
//!   measured (`skeleton::fallback`).
//! - `death`: the death animation (10), a jumping body's turn, the body's
//!   removal time.
//! - `models`: the install's bodies and third-person weapons (2) as
//!   Chimera's character and held models, and their rigs.
//!
//! Each tick (`FixedUpdate`, after the weapons: the spec's server runs the
//! driver per movement frame) every living character's `CaAnim` advances
//! by the tick and decides (a network client only its own player), then
//! its skeleton is posed into `CaPose`: hit-sphere centres and the sockets
//! the game uses (`NAME_socket` for the name tag, `Left_foot` for drops,
//! `RightHand` for the weapon and the third-person flash), in the
//! character's frame. `weapons` hits the spheres, `inventory` drops at the
//! left foot, the HUD anchors name tags at the name socket. With a body
//! model (`models::rig` has the selection's body file) the pose is the
//! model's own, drawn through Chimera's body (`map::BodyName`,
//! `Animator::fixed`) with the weapon held at `RightHand` (`map::Held`).
//! After the pose (`hitboxes`) a Combat Arms character's spheres are also
//! Chimera's hitboxes (`core::Hitboxes`, marked `CaHitboxes`: spheres with
//! their region, `Node#Priority` and zone factor as the body's own scale;
//! docs/DESIGN.md "Hits across games"), so other games' guns hit the
//! drawn body by region.
//!
//! A character whose identity is another game's (`slots::GameIdentity`:
//! Lucker Party's players follow their weapon's game) keeps its trackers,
//! pose and hit spheres (our guns, knives and grenades hit it so while it
//! has no hitboxes of its own game's; with them, those: `weapons::
//! their_shapes`; its name tag floats over that body's hitboxes, not our
//! hidden skeleton's socket) but wears no look of
//! ours (`dress` takes `BodyName`, `BodyNumber` and `BodySkin` off, `drive`
//! leaves its `Animator` and visibility alone) and leaves no body of ours
//! when it dies.
//!
//! On death (`core::Died`) the driver plays the death animation chosen by
//! `death::choose` from the killing hit, and a body (`CaBody`) is left at
//! the player's floor-snapped position + 0.1 u; it goes 5 s after its
//! owner respawned and its animation ended in the modes that remove
//! bodies (data's `body_removal`), or at a new round. With no mode of ours
//! (`ca_mode none`) but a minigame playing (Lucker Party), as our like
//! mode: a free-for-all as One Man Army, teams as Team Deathmatch.
//!
//! Network (`CaCharactersNetPlugin`, part `combat_arms_characters`): each
//! client sends its own look (`CaLookRequest`, from `CaOwnLook`: when its
//! player appears and whenever it changes); the server validates it
//! (`CharacterData::validated`: an unknown body refused, unknown pieces
//! none), keeps it on the client's entity (`CaClientLook`) and dresses
//! that client's player in it (`wear_looks`). The server sends each
//! character's selection (`CaLook`: players', bots'), its trackers' names
//! and start ticks with the crouch, pitch and twist bytes (`CaAnimNet`,
//! 8's legacy transport), and bodies (`CaBody`); a client plays other
//! players' trackers from it and poses them with the received aim, as the
//! spec's clients do.
//!
//! The follow-up answers (characters.md Open questions 2, 5, 9, 13-22):
//! the hit spheres sit on the animated pose (every tracker and the aim)
//! where a body model is loaded; the server's own pose (no `Torso`
//! pitch: `CaPose::server_spheres`) takes the hits the server traces
//! itself (bots' rounds, `weapons`); the skeleton's root follows the
//! crouch to the hull's bottom; the defaults without lobby data are the
//! fresh selection with the face's own hair; the digit classes carry
//! their digit as 6 says; "fire held" is the raw flag on the server and
//! the masked one on a client's own body (`driver::fire_held`); the
//! server times its reload from the third-person reload (`driver::
//! server_reload_timer`: L / 1.2 min(r, 2), 500 ms without one, none with
//! `HHAniFrame_5`); the death types `D_W` and `D_P` come from the endless
//! fall's volume damage and a thrown weapon's lasting damage, the coin
//! from the clock in seconds (`death::coin`, the C library's generator);
//! the dead player itself comes down onto the floor (`snap`); corpses
//! neither stop shots nor have spheres; a remote character's displayed
//! pitch kicks on others' hits (`driver::hit_kick`); emotes (Hunted,
//! `ca_emote`) play on the upper tracker with the weapon hidden.
//!
//! The 2026-10-22 corrections (characters.md Corrections 1-13): the upper
//! tracker starts only on a class or posture change, an accepted shot
//! (sniper, land mine: `CaAnim::shot`, from the weapon's `Fired` /
//! `Thrown`) or a different name while aiming or firing, so a held fire
//! button plays once and holds its last frame; no fire pose while the
//! legs sprint; "ammo 0" is clip plus reserve (`driver::ammo_exhausted`);
//! walking off a drop the 150 u footing ray misses reports falling
//! (`fell`: standing legs and the no-direction aim until the landing);
//! crouching in the air clears the status; forward with back is backward;
//! an emote is upper body only (the legs replace the lower copy at the
//! next update, the upper keeps its last pose after it ends); a front-left
//! body death plays `DC`; "in a jump" at death is the jump legs' first
//! ~667 ms and a blast never turns the body; Quarantine shows no team mark
//! (`models::shows_mark`); a leg set without a crouch name crouches on the
//! legs' animation; the body is a drawn object (`corpses`: `map::Corpse`,
//! the death animation at its age), and the dead player itself is hidden.
//!
//! Known shortcuts (docs/tech-debt.md; the most literal reading where the
//! spec is unclear, listed for its open questions):
//! - No V2 key: no body, head piece, goggle or third-person weapon model
//!   loads (the unkeyed corpses and zombie bodies on the stock trees
//!   aren't player records; `heavy::characters` poses them; characters
//!   are the shell's capsules) and the library is the spec's names and
//!   measured lengths (`driver::spec_library`; names the spec doesn't
//!   measure get their kind's length). The skeleton is then the spec's
//!   measured `LST` t = 0 pose for nodes and spheres and the "stand" pose
//!   for sockets, crouched by squashing heights towards the feet (the
//!   hull's height over the standing one), turned by the aim: so without
//!   a model the spheres don't follow the walking legs or the weapon's
//!   aim pose (Open question 13 says they do).
//! - With models: the six stock bodies load (`models::LOADED`), one
//!   character model per sex: the first body with its library, the
//!   other bodies (the Light and Heavy vests), the faces of the loadout
//!   pool and the head pieces they may wear merged onto its skeleton as
//!   body-part choices (`models::BodyGroup`, `map::BodyNumber`), so the
//!   Light and Heavy bodies pose on the Normal one's library; another
//!   selection's body stays a capsule on the spec's skeleton, a face or
//!   head piece outside the pool shows the body's default; the pose is
//!   computed per tick (the server's driver runs per movement frame, its
//!   trackers per server frame: Open question 14) and drawn as is; with
//!   the default head piece (`HBasic`, no model) a body wears the head
//!   piece its face names (`HairIndex`); goggles and backpacks (the
//!   console's `ca_goggle`, `ca_backpack`; the bots' pool) are body-part
//!   choices too, rigid on their socket's node at its bind transform
//!   (`models::attach`), only the pool's (another shows none), and not
//!   hidden for a first-person owner (bodies aren't drawn there anyway);
//!   cosmetics and the clan pieces aren't loaded (nothing selects them);
//!   of an infection override (`records::InfectionRecord`, read from the
//!   install) only its face is worn (a gun game's infected rung, and
//!   Quarantine's infected, `movement::CaInfected`, in `wear_looks`:
//!   `CharacterData::infected_look`), not its render styles or hands; a face's skins by camouflage index (its
//!   colour overrides not read); render styles as `render_style` reads
//!   them; a corpse (`CaBody`) is drawn by Chimera's `map::Corpse` body
//!   from the same models, posed by `corpses` at its age on the receiving
//!   machine's own clock, hung `STAND_HEIGHT / 2` under its origin (a
//!   crouched death stands) and without the install's models only the
//!   data exists; the dead player itself is hidden only where it has a
//!   body model. Bots' looks are ours (the
//!   spec has no loadouts for them). The look a client sends is its
//!   selection's ids only; without the install's models file a face is
//!   known when the body's face redirect covers it and a head piece when
//!   ours or the bots' pool names it (`validated`). A weapon names its
//!   third-person model only with the V1 key (the install's `HHModel`,
//!   `data`, on `RightHand` with no offset); ours name none.
//! - The driver: `HHAniFrame` is absent from our data (every upper rate
//!   1; the install's weapons file gives it with the V1 key); the
//!   weapons' classes are ours by kind (data/characters.toml) unless the
//!   install's weapons file (V1) gives their `AniType`; the server's
//!   reload rate is 1 (no reload bonuses read); the footing ray (Corrections
//!   3) and the shot notice (4) are taken from the character's own tick,
//!   not the client's movement frame; the hit kick shows on a
//!   network client's remote characters only (a single player's world is
//!   the server's); an emote plays no sound (the emotion files aren't
//!   named by role) and is asked for on the local player only (no menu,
//!   no network message).
//! - Aim: pitch and twist from the look and the weapon's body yaw
//!   (`weapons::CaSpread::body_yaw`); the client's shares everywhere but
//!   the bots' rounds.
//! - Death: the coin's clock is the game's, not the wall clock; the
//!   floor snap skips a player standing on the ground.

pub mod bodies_on_demand;
pub mod death;
pub mod driver;
pub mod held_on_demand;
pub mod models;
pub mod records;
pub mod skeleton;

use std::sync::Arc;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use self::{
    driver::{Driver, Inputs, Status},
    records::{CharacterData, Selection},
    skeleton::{Posed, Rig},
};
use super::{
    data::CaData,
    movement::CaMovement,
    rules::spawns::CaProtected,
    weapons::{CaSpread, FEMALE, HitSpheres, Location, MALE, zoom::CaZoom},
};
use crate::{
    core::{
        self, Damage, Dead, Died, Health, Hitgroup, Intent, LocalPlayer, MovementState, NetRole, RoundRestarts,
        SimClock, SimSet, Team,
    },
    map::{BodyName, BodyNumber, BodySkin, CharacterModels, Corpse, Held, anim::Animator},
    net::{NetAppExt, game::ServerTime, ordered::OrderedAppExt, server::Player},
    slots::GameIdentity,
    weapon::{Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind, WeaponState},
};

/// A player's model selection (1); the default (model 0, face 0, no
/// gear) when a character has none.
pub type CaLook = Selection;

/// The standing hull's height in units (the stock 2 x 50).
pub const STAND_HEIGHT: f32 = 100.0;
/// A body is created this far above the dead player (u).
pub const BODY_LIFT: f32 = 0.1;

/// One character's animation state and what it was told since the last
/// update.
#[derive(Component, Clone, Debug)]
pub struct CaAnim {
    pub driver: Driver,
    /// The effective body record and its skeleton (hit table) index.
    pub body: i32,
    pub skeleton: usize,
    /// The body's rig (None: no model, the spec's skeleton).
    pub rig: Option<Arc<Rig>>,
    /// A weapon status reported since the last update.
    pub pending: Option<Status>,
    /// The last damage was above 0.
    pub damaged: bool,
    /// A shot or throw was accepted since the last update.
    pub shot: bool,
    /// In a jump at the last update (for "jumped" and "landed").
    pub in_jump: bool,
    /// The aim as posed (spec frame: pitch positive down, twist positive
    /// right; radians).
    pub pitch: f32,
    pub twist: f32,
    /// The death animation playing (None: alive).
    pub death: Option<String>,
    /// The hit kick (9): damage notices counted, the pitch offset for the
    /// next aim update (radians, upward).
    pub kick_count: u32,
    pub kick: f32,
    /// An emote asked for (`ca_emote`: 0..3, `driver::EMOTES`).
    pub pending_emote: Option<usize>,
    /// The server's reload in progress: when it started (s) and its timer
    /// (ms; None: the animation ends it), 7.
    pub server_reload: Option<(f64, Option<u32>)>,
    dice: u32,
}

impl CaAnim {
    pub fn new(body: i32, skeleton: usize, rig: Option<Arc<Rig>>) -> Self {
        let list = rig.as_ref().map_or_else(|| driver::spec_library(skeleton == 1), |r| Arc::new(r.list.clone()));
        Self {
            driver: Driver::new(list),
            body,
            skeleton,
            rig,
            pending: None,
            damaged: false,
            shot: false,
            in_jump: false,
            pitch: 0.0,
            twist: 0.0,
            death: None,
            kick_count: 0,
            kick: 0.0,
            pending_emote: None,
            server_reload: None,
            dice: 0x9e37_79b9,
        }
    }

    /// The next coin (true: 1).
    fn coin(&mut self) -> bool {
        self.dice ^= self.dice << 13;
        self.dice ^= self.dice >> 17;
        self.dice ^= self.dice << 5;
        self.dice & 1 == 1
    }

    /// The upper, lower and crouch trackers' animation names.
    pub fn playing(&self) -> (&str, &str, &str) {
        let d = &self.driver;
        (d.name_of(&d.upper), d.name_of(&d.lower), d.name_of(&d.crouch))
    }

    /// The posed skeleton now, with aim (`server`: the server's shares).
    pub fn posed(&self, crouch: f32, squash: f32, server: bool) -> Posed {
        let mut posed = self.unaimed(crouch, squash);
        posed.aim(&records::builtin().skeleton(self.skeleton).aim, self.pitch, self.twist, server);
        posed
    }

    /// `posed` for the client's view and the server's (`posed(.., false)`,
    /// `posed(.., true)`), the layers posed once: only the aim differs.
    pub fn posed_both(&self, crouch: f32, squash: f32) -> (Posed, Posed) {
        let aim = &records::builtin().skeleton(self.skeleton).aim;
        let mut seen = self.unaimed(crouch, squash);
        let mut server = seen.clone();
        seen.aim(aim, self.pitch, self.twist, false);
        server.aim(aim, self.pitch, self.twist, true);
        (seen, server)
    }

    /// The pose before the aim.
    fn unaimed(&self, crouch: f32, squash: f32) -> Posed {
        let d = &self.driver;
        let skel = records::builtin().skeleton(self.skeleton);
        match &self.rig {
            Some(rig) => {
                let pose = if self.death.is_some() {
                    // A body has only its main tracker (10).
                    (0..rig.model.nodes.len()).map(|j| d.main.pose(&rig.list, j)).collect()
                } else {
                    rig.layered(&d.main, &d.upper, &d.lower, &d.crouch, crouch)
                };
                rig.posed(&pose, crouch)
            }
            None => skeleton::fallback(skel, squash),
        }
    }
}

/// The posed skeleton's results in the character's frame (right-handed,
/// facing -Z, +X its right, feet at the origin; world units), turned by
/// `body_yaw` in the world.
#[derive(Component, Clone, Debug, Default)]
pub struct CaPose {
    /// Per entry of the skeleton's hit table: its centre (None: the body
    /// has no node or socket of that name, or its radius is 0).
    pub spheres: Vec<Option<Vec3>>,
    /// The same on the server's pose (no `Torso` pitch, 8): the hits the
    /// server traces itself (bots' shots).
    pub server_spheres: Vec<Option<Vec3>>,
    /// The skeleton (hit table) index.
    pub skeleton: usize,
    /// `NAME_socket`, `Left_foot`, `RightHand`.
    pub name_tag: Option<Vec3>,
    pub left_foot: Option<Vec3>,
    pub right_hand: Option<Vec3>,
    /// `EYE_L` and `EYE_R`, where the face's looping groups sit (the
    /// infected's eyes: game_modes.md Corrections (2026-10-30) 7).
    pub eyes: [Option<Vec3>; 2],
    /// The legs' yaw (as `Intent::yaw`).
    pub body_yaw: f32,
    /// From a body model (not the spec's skeleton).
    pub modelled: bool,
}

impl CaPose {
    /// A point of the pose in the world for a character at `t` with
    /// `state`'s hull, `unit` metres per unit.
    pub fn world(&self, t: &Transform, state: &MovementState, unit: f32, p: Vec3) -> Vec3 {
        let feet = t.translation + Vec3::Y * if state.hull_max != state.hull_min { state.hull_min.y } else { -50.0 * unit };
        feet + Quat::from_rotation_y(self.body_yaw) * (p * unit)
    }

    /// The sockets effect groups attach to on a character (`EYE_L`,
    /// `EYE_R`, `RightHand`, `NAME_socket`), in the character's frame.
    pub fn sockets(&self) -> impl Iterator<Item = (&'static str, Vec3)> + '_ {
        [
            ("EYE_L", self.eyes[0]),
            ("EYE_R", self.eyes[1]),
            ("RightHand", self.right_hand),
            // The held weapon's flash socket, at the hand (the held
            // model's own isn't posed here).
            ("Flash", self.right_hand),
            ("Flash01", self.right_hand),
            ("NAME_socket", self.name_tag),
        ]
        .into_iter()
        .filter_map(|(n, p)| Some((n, p?)))
    }
}

/// What other players are told of a character (the server's): its
/// trackers' names with the server tick each one's time 0 fell on, the
/// crouch fraction, pitch and twist bytes (8's legacy transport), the
/// death animation.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaAnimNet {
    pub upper: (String, f64),
    pub lower: (String, f64),
    pub crouch: (String, f64),
    pub r: u8,
    pub pitch: i8,
    pub twist: i8,
    pub death: Option<String>,
    pub body_yaw: f32,
}

/// A dead player's body (10): where it lies, facing, its owner, the death
/// animation and when it started (s, the simulation's clock), when its
/// owner respawned.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CaBody {
    pub owner: Entity,
    pub look: Selection,
    pub at: Vec3,
    pub yaw: f32,
    pub animation: String,
    pub since: f64,
    /// The animation's length, s.
    pub length: f64,
    pub respawned: Option<f64>,
}

/// Combat Arms' characters.
pub struct CaCharactersPlugin;

impl Plugin for CaCharactersPlugin {
    fn build(&self, app: &mut App) {
        CaData::of(app);
        app.add_message::<WeaponEvent>()
            .add_message::<Damage>()
            .add_message::<Died>()
            .net_part("combat_arms_characters", register)
            .add_systems(
                FixedUpdate,
                (setup, listen, die.after(core::apply_damage), drive, bodies.run_if(core::authoritative))
                    .chain()
                    .after(SimSet::Weapons),
            )
            .init_resource::<CaOwnLook>()
            .add_systems(FixedUpdate, dress.after(drive).after(bodies))
            .add_systems(FixedUpdate, hitboxes.after(drive))
            .add_systems(FixedUpdate, wear_looks.before(setup).run_if(core::authoritative))
            .add_systems(FixedUpdate, client_clock.before(drive).run_if(core::shows_others))
            .add_systems(
                Update,
                (held_on_demand::load_on_demand, bodies_on_demand::load_on_demand)
                    .run_if(resource_exists::<super::view_model::on_demand::LoadOnDemand>),
            );
        look_cvars(app);
        use crate::console::ConsoleAppExt;
        app.console_command(
            "ca_emote",
            "Hunted only: play emote 1 Insult, 2 Surrender, 3 Insult_Dance or 4 Salute (characters.md 9; the G menu's).",
            |w, args| {
                let n: usize = args.first().and_then(|a| a.parse().ok()).filter(|n| (1..=4).contains(n)).ok_or("ca_emote 1-4")?;
                if w.get_resource::<super::rules::CaRuleSettings>().is_none_or(|s| s.mode != "HG") {
                    return Ok(Some("emotes only in Hunted".into()));
                }
                let me = w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).next().ok_or("no player")?;
                if let Some(mut a) = w.get_mut::<CaAnim>(me) {
                    a.pending_emote = Some(n - 1);
                }
                Ok(None)
            },
        );
    }
}

/// The protocol: the selection, the animation state and bodies, from the
/// server.
fn register(app: &mut App) {
    app.replicate::<Selection>()
        .ordered::<Selection>()
        .replicate::<CaRungLook>()
        .ordered::<CaRungLook>()
        .replicate::<CaAnimNet>()
        .ordered::<CaAnimNet>()
        .replicate::<CaBody>()
        .ordered::<CaBody>()
        .add_client_message::<CaLookRequest>(Channel::Ordered)
        .add_systems(
            PreUpdate,
            receive_looks
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            PostUpdate,
            send_look.before(ClientSystems::Send).run_if(resource_equals(NetRole::Client)),
        );
}

/// A client's own look, sent to the server with its character info
/// (characters.md 1, "The selection").
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CaLookRequest(pub Selection);

/// The look a connected client chose (the server's, on the client's
/// entity, validated: `CharacterData::validated`); its player's
/// characters wear it (`wear_looks`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaClientLook(pub Selection);

/// A client sends its look when its player appears and whenever it
/// changes (`CaOwnLook`, the console's `ca_*`).
fn send_look(
    loadout: Res<CaOwnLook>,
    me: Query<Entity, With<LocalPlayer>>,
    mut sent: Local<Option<(Entity, Selection)>>,
    mut out: MessageWriter<CaLookRequest>,
) {
    let Ok(e) = me.single() else {
        *sent = None;
        return;
    };
    if *sent != Some((e, loadout.0)) {
        *sent = Some((e, loadout.0));
        out.write(CaLookRequest(loadout.0));
    }
}

/// The server keeps each client's look, validated (an unknown body is
/// refused: the client keeps its previous look).
fn receive_looks(
    mut requests: MessageReader<FromClient<CaLookRequest>>,
    clients: Query<(), With<Player>>,
    data: Res<CaData>,
    mut commands: Commands,
) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        if clients.get(client).is_err() {
            continue;
        }
        match data.characters().validated(data.gear(), r.message.0) {
            Some(look) => {
                commands.entity(client).insert(CaClientLook(look));
            }
            None => debug!("look refused: body record {} unknown", r.message.0.model),
        }
    }
}

/// Metres per world unit.
fn unit() -> f32 {
    super::movement::movement_data().meters_per_unit
}

/// The selection's body record, skeleton and rig.
fn body_of(data: &CaData, look: &Selection) -> (i32, usize, Option<Arc<Rig>>) {
    let (record, _) = data.characters().effective(look);
    (record.id, record.skeleton, models::rig(&record.file))
}

/// The local player's own look (characters.md 1, "The selection": what
/// its client sends; ours from the console, `ca_model`, `ca_face`,
/// `ca_head`, `ca_camo`: the record ids and the camouflage index).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaOwnLook(pub Selection);

impl Default for CaOwnLook {
    fn default() -> Self {
        Self(Selection::default())
    }
}

/// The console's look settings (`CaOwnLook`).
pub fn look_cvars(app: &mut App) {
    use crate::console::resource_cvar;
    resource_cvar::<CaOwnLook, i32>(app, "ca_model", "Your body record (0 Normal, 1 Light, 2 Heavy).", |l| &mut l.0.model);
    resource_cvar::<CaOwnLook, i32>(app, "ca_face", "Your face (0-3 male, 4-7 female).", |l| &mut l.0.face);
    resource_cvar::<CaOwnLook, i32>(app, "ca_head", "Your head piece (0: none).", |l| &mut l.0.head);
    resource_cvar::<CaOwnLook, i32>(app, "ca_camo", "Your camouflage index (0-6).", |l| &mut l.0.camouflage);
    resource_cvar::<CaOwnLook, i32>(app, "ca_goggle", "Your goggles or mask (-1: none).", |l| &mut l.0.goggle);
    resource_cvar::<CaOwnLook, i32>(app, "ca_backpack", "Your backpack (-1: none).", |l| &mut l.0.backpack);
}

/// A seed for a bot's look: its name's (stable across rounds), else its
/// entity's.
fn look_seed(name: Option<&Name>, e: Entity) -> u64 {
    match name {
        Some(n) => n.as_str().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)),
        None => e.to_bits(),
    }
}

/// A look every player on a ladder rung wears in place of their own
/// (`game::rungs`: the rung's character, the same for all on it); the
/// server's, replicated with the `Selection` it puts on, so every side's
/// gear reads the rung's baseline vest for it (`gear::vest_of`).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaRungLook(pub Selection);

/// A bot's own look (`CharacterData::bot_look` by its name's seed): what
/// it wears again off a rung's look.
pub fn bot_look_of(name: Option<&Name>, e: Entity) -> Selection {
    records::builtin().bot_look(look_seed(name, e))
}

/// Looks (the server's): a rung's look first (`CaRungLook`); a bot
/// without one draws its own from the
/// loadout pool (`CharacterData::bot_look`); the local player wears
/// `CaOwnLook` (changed from the console at once); a client's player the
/// look its client sent (`CaClientLook`); others keep the default.
#[allow(clippy::type_complexity)]
#[allow(clippy::type_complexity)]
fn wear_looks(
    loadout: Res<CaOwnLook>,
    data: Res<CaData>,
    clients: Query<(&Player, &CaClientLook)>,
    characters: Query<
        (
            Entity,
            Option<&Selection>,
            Option<&Name>,
            Has<core::Bot>,
            Has<LocalPlayer>,
            Option<&CaRungLook>,
            Has<super::movement::CaInfected>,
        ),
        (With<Intent>, With<MovementState>),
    >,
    mut commands: Commands,
) {
    for (e, look, name, bot, local, rung, infected) in &characters {
        let sent = clients.iter().find(|(p, _)| p.character == e).map(|(_, l)| l.0);
        let own = if let Some(r) = rung {
            Some(r.0)
        } else if local {
            Some(loadout.0)
        } else if let Some(sent) = sent {
            Some(sent)
        } else if bot && look.is_none() {
            Some(records::builtin().bot_look(look_seed(name, e)))
        } else {
            None
        };
        // Quarantine's infected wear their body's infection override's
        // look (its face, so its eyes: game_modes.md M8, Corrections
        // (2026-10-30) 5, 7).
        let want = match (own, infected) {
            (Some(own), true) => data.characters().infected_look(own.model),
            (None, true) => match look {
                Some(l) => {
                    let i = data.characters().infected_look(l.model);
                    if i.face == l.face { *l } else { i }
                }
                None => continue,
            },
            (Some(own), false) => own,
            (None, false) => continue,
        };
        if look != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// Characters get a selection (the default) and an animation state; a
/// changed selection, a new driver (a new body file).
#[allow(clippy::type_complexity)]
fn setup(
    data: Res<CaData>,
    characters: Query<(Entity, Option<&Selection>, Option<&CaAnim>), (With<Intent>, With<MovementState>)>,
    females: Option<ResMut<crate::core::FemaleBodies>>,
    mut commands: Commands,
) {
    // The bodies' sex, for what is gendered (the announcer's groin-shot
    // voice): kept only where something reads it.
    let mut females = females;
    if let Some(f) = females.as_mut() {
        f.0.retain(|e| characters.contains(*e));
    }
    for (e, look, anim) in &characters {
        let look = look.copied().unwrap_or_default();
        let (body, skeleton, rig) = body_of(&data, &look);
        if let Some(f) = females.as_mut()
            && f.0.contains(&e) != (skeleton == 1)
        {
            if skeleton == 1 {
                f.0.insert(e);
            } else {
                f.0.remove(&e);
            }
        }
        let same = anim.is_some_and(|a| a.body == body && a.rig.as_ref().map(Arc::as_ptr) == rig.as_ref().map(Arc::as_ptr));
        if !same {
            let table = if skeleton == 1 { *FEMALE } else { *MALE };
            commands
                .entity(e)
                .insert((look, CaAnim::new(body, skeleton, rig), CaPose::default(), HitSpheres(table)));
        }
    }
}

/// Weapon statuses (a draw starts: "select") and damage ("hit") since
/// the last update; damage by someone other than the local player kicks
/// the character's displayed pitch (9).
fn listen(
    mut events: MessageReader<WeaponEvent>,
    mut damage: MessageReader<Damage>,
    mut anims: Query<&mut CaAnim>,
    local: Query<Entity, With<LocalPlayer>>,
) {
    let me = local.iter().next();
    for ev in events.read().filter(|e| e.shown()) {
        if matches!(ev.kind, WeaponEventKind::Deployed)
            && let Ok(mut a) = anims.get_mut(ev.owner)
        {
            a.pending = Some(Status::Select);
        }
        // Each accepted shot or throw (Corrections 4).
        if matches!(ev.kind, WeaponEventKind::Fired { .. } | WeaponEventKind::Thrown)
            && let Ok(mut a) = anims.get_mut(ev.owner)
        {
            a.shot = true;
        }
    }
    for d in damage.read() {
        if let Ok(mut a) = anims.get_mut(d.target) {
            a.damaged = d.amount > 0.0;
            a.pending = Some(Status::Hit);
            if d.attacker.is_none() || d.attacker != me {
                let (kick, count) = driver::hit_kick(a.kick_count);
                a.kick = kick;
                a.kick_count = count;
            }
        }
    }
}

/// The location of a hit group (the weapons' sphere locations).
fn location_of(h: Hitgroup) -> Option<Location> {
    match h {
        Hitgroup::Head => Some(Location::Head),
        Hitgroup::Chest => Some(Location::Torso),
        Hitgroup::Stomach => Some(Location::Groin),
        Hitgroup::LeftArm | Hitgroup::RightArm => Some(Location::Arm),
        Hitgroup::LeftLeg | Hitgroup::RightLeg => Some(Location::Leg),
        Hitgroup::Generic => None,
    }
}

/// The spec frame's forward and right of a character facing `yaw` (ours),
/// in the world.
fn axes(yaw: f32) -> (Vec3, Vec3) {
    let q = Quat::from_rotation_y(yaw);
    (q * Vec3::NEG_Z, q * Vec3::X)
}

/// Death (10): the death animation on the main tracker, and the body.
#[allow(clippy::type_complexity)]
fn die(
    mut died: MessageReader<Died>,
    mut characters: Query<(&mut CaAnim, &Transform, &Intent, Option<&CaPose>, &Selection, Option<&GameIdentity>)>,
    catalog: Res<crate::weapon::data::WeaponCatalog>,
    clock: Res<SimClock>,
    authoritative: Option<Res<NetRole>>,
    mut commands: Commands,
) {
    let server = authoritative.is_none_or(|r| *r != NetRole::Client);
    for d in died.read() {
        let Ok((mut anim, t, intent, pose, look, identity)) = characters.get_mut(d.entity) else {
            continue;
        };
        // Another game's character for now leaves its own body (its
        // ragdoll), not ours.
        let ours = GameIdentity::allows(identity, super::GAME);
        let yaw = pose.map_or(intent.yaw, |p| p.body_yaw);
        let (forward, right) = axes(yaw);
        let kind = death::kind_of(&catalog, d.damage.kind, d.damage.weapon);
        let location = location_of(d.damage.hitgroup);
        // Quirk 6 (Open question 21): a fresh C-library generator seeded
        // with the clock in whole seconds, one draw.
        let coin = death::coin(clock.now.floor() as u32);
        // "In a jump" is the legs' jump animation (the first ~667 ms of a
        // jump), not the whole flight (Corrections 10).
        let drv = &anim.driver;
        let jumping = drv.last.movement == driver::Movement::Jump && !drv.lower.done(&drv.list);
        let inputs = death::DeathInputs {
            kind,
            dir: d.damage.dir,
            forward,
            right,
            head: location == Some(Location::Head),
            groin: location == Some(Location::Groin),
            coin,
            jumping,
            special: None,
        };
        let list = anim.driver.list.clone();
        let name = death::choose(&inputs, |n| list.find(n).is_some());
        if let Some(entry) = list.find(&name) {
            let main = &mut anim.driver.main;
            main.start(entry);
            main.looping = false;
        }
        anim.death = Some(name.clone());
        anim.pitch = 0.0;
        if !server || !ours {
            continue;
        }
        // The body: the dead player's own yaw, or turned away from the
        // shot when it died in a jump (spec yaw is ours negated).
        let body_yaw = if jumping && kind != death::DeathKind::Explosion {
            let dir = Quat::from_rotation_y(-yaw) * d.damage.dir;
            yaw - death::jump_yaw(Vec3::new(dir.x, dir.y, -dir.z))
        } else {
            yaw
        };
        let length = list.find(&name).map_or(0.0, |e| list.entries[e].length() as f64 / 1000.0);
        let owner = d.entity;
        let look = *look;
        let at = t.translation;
        let since = clock.now;
        commands.queue(move |w: &mut World| {
            // The player object itself comes down onto the floor (10 step
            // 1, Open question 21), the body 0.1 u above it.
            let snapped = floor_snapped(w, owner, at);
            if snapped != at
                && let Ok(mut e) = w.get_entity_mut(owner)
            {
                if let Some(mut t) = e.get_mut::<Transform>() {
                    t.translation = snapped;
                }
                if let Some(mut p) = e.get_mut::<avian3d::prelude::Position>() {
                    p.0 = snapped;
                }
            }
            let at = snapped + Vec3::Y * BODY_LIFT * unit();
            w.spawn((
                Name::new("Body"),
                Replicated,
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(body_yaw)),
                CaBody {
                    owner,
                    look,
                    at,
                    yaw: body_yaw,
                    animation: name,
                    since,
                    length,
                    respawned: None,
                },
            ));
        });
    }
}

/// Where the hull centre `at` of `owner` comes to rest on the floor
/// below (10, step 1): the same when standing or nothing is below.
fn floor_snapped(world: &mut World, owner: Entity, at: Vec3) -> Vec3 {
    let Some(state) = world.get::<MovementState>(owner).cloned() else { return at };
    if state.on_ground || state.hull_max == state.hull_min {
        return at;
    }
    let half = -state.hull_min.y;
    let mut system_state: bevy::ecs::system::SystemState<avian3d::prelude::SpatialQuery> =
        bevy::ecs::system::SystemState::new(world);
    let Ok(spatial) = system_state.get(world) else { return at };
    let filter = avian3d::prelude::SpatialQueryFilter::from_mask(core::SOLID_LAYERS).with_excluded_entities([owner]);
    let reach = FLOOR_RAY * unit();
    match spatial.cast_ray(at, Dir3::NEG_Y, reach, true, &filter) {
        Some(hit) => snap(at, hit.distance, half, BODY_LIFT * unit()),
        None => at,
    }
}

/// 10 step 1's snap: a centre `at` whose floor is `below` straight down,
/// for a hull of half-height `half`: unmoved when the floor is within the
/// half-height, else the centre at the floor + `half` + `lift`.
pub fn snap(at: Vec3, below: f32, half: f32, lift: f32) -> Vec3 {
    if below <= half { at } else { Vec3::new(at.x, at.y - below + half + lift, at.z) }
}

/// How far down the floor snap looks (u).
pub const FLOOR_RAY: f32 = 10_000.0;

/// The player object's visibility change, if any: hidden while dead (only
/// where the body is ours), shown again when alive. Showing doesn't ask
/// whose identity it is: a bot that died as a Combat Arms soldier and
/// respawned holding a CS:S gun (the gun game's `identity = "weapon"`)
/// stayed hidden for good, and its CS:S body with it.
pub fn object_visibility(ours: bool, alive: bool, hidden: bool) -> Option<Visibility> {
    match (alive, hidden) {
        (false, false) if ours => Some(Visibility::Hidden),
        (true, true) => Some(Visibility::Inherited),
        _ => None,
    }
}

/// Each tick: every living character's driver (a client's own player
/// only; others follow the server's `CaAnimNet`), then its pose.
#[allow(clippy::type_complexity)]
fn drive(
    data: Res<CaData>,
    clock: Res<SimClock>,
    role: Option<Res<NetRole>>,
    server_time: Option<Res<ClientClock>>,
    mut characters: Query<(
        Entity,
        (&mut CaAnim, &mut CaPose, Option<&mut CaAnimNet>),
        (&Intent, &MovementState, Option<&CaMovement>, &Health, Has<Dead>, Has<LocalPlayer>),
        (Option<&Inventory>, Option<&CaProtected>, Option<&Visibility>),
        (Option<&mut Animator>, Option<&GameIdentity>, Option<&Selection>),
    )>,
    weapons: Query<(&Weapon, Option<&WeaponState>, Option<&Magazine>, Option<&CaZoom>, Option<&CaSpread>)>,
    mut commands: Commands,
) {
    let client = role.as_deref().is_some_and(|r| *r == NetRole::Client);
    let ms = (clock.delta.as_secs_f64() * 1000.0).round() as u32;
    let unit = unit();
    for (e, (mut anim, mut pose, net), (intent, state, movement, health, dead, local), (inventory, protected, visibility), (animator, identity, look)) in
        &mut characters
    {
        // Our body only on our characters (another game's identity wears
        // its own); the trackers, pose and hit spheres run for every one.
        let ours = GameIdentity::allows(identity, super::GAME);
        let alive = health.current > 0.0 && !dead;
        if alive && anim.death.is_some() {
            // Respawned: the trackers start over.
            let list = anim.driver.list.clone();
            anim.driver.set_list(list);
            anim.death = None;
        }
        // The player object itself is invisible while dead: the body object
        // shows the death (Corrections 12).
        if anim.rig.is_some()
            && let Some(v) = object_visibility(ours, alive, visibility == Some(&Visibility::Hidden))
        {
            commands.entity(e).insert(v);
        }
        let weapon = inventory.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let r = movement.map_or(0.0, |m| m.crouch_rate);
        let height = (state.hull_max.y - state.hull_min.y) / unit;
        let squash = if height > 1.0 { (height / STAND_HEIGHT).min(1.0) } else { 1.0 };
        let followed = client && !local;
        if !alive {
            let list = anim.driver.list.clone();
            anim.driver.main.update(&list, ms, &mut |_| {});
            anim.pending = None;
            anim.shot = false;
        } else if followed {
            if let (Some(net), Some(clock)) = (net.as_deref(), server_time.as_deref()) {
                follow(&mut anim, net, clock.0, clock.1);
            }
            // The hit kick shows for one aim update (display only).
            let kick = std::mem::take(&mut anim.kick);
            anim.pitch = skeleton::clamp_aim(anim.pitch - kick);
        } else {
            anim.kick = 0.0;
            if let Some(n) = anim.pending_emote.take()
                && let Some(name) = driver::EMOTES.get(n)
            {
                anim.driver.start_emote(name);
            }
            let mut inputs = inputs(data.characters(), &mut anim, intent, movement, weapon, protected, r, !client, clock.now);
            inputs.no_footing = movement.is_some_and(|m| m.no_footing);
            inputs.grounded = state.on_ground;
            anim.driver.update(ms, &inputs);
            // The aim: pitch from the look, the twist from the body yaw.
            anim.pitch = skeleton::clamp_aim(-intent.pitch);
            let body = weapon.and_then(|w| w.4).and_then(|s| s.body_yaw).unwrap_or(intent.yaw);
            anim.twist = skeleton::clamp_aim(-wrap(intent.yaw - body));
        }
        let body_yaw = if followed {
            net.as_deref().map_or(intent.yaw, |n| n.body_yaw)
        } else {
            intent.yaw + anim.twist
        };
        // The worn face's sockets (its eyes), on the body's nodes.
        let face = look.and_then(|l| data.characters().worn_face(l)).and_then(|f| models::face_sockets(f.id));
        let face = face.as_deref().map(Vec::as_slice);
        let posed = if client {
            let posed = anim.posed(r, squash, false);
            *pose = pose_of(&anim, &posed, body_yaw, squash, face);
            posed
        } else {
            let (posed, server) = anim.posed_both(r, squash);
            *pose = pose_of(&anim, &posed, body_yaw, squash, face);
            pose.server_spheres = pose_of(&anim, &server, body_yaw, squash, face).spheres;
            posed
        };
        if let (true, Some(mut animator)) = (anim.rig.is_some() && ours, animator) {
            animator.fixed = Some(Arc::new(posed.locals_rh()));
            if animator.main.is_none() {
                animator.main = Some(0);
            }
            animator.yaw = Some(body_yaw);
        }
        if !client {
            let next = net_of(&anim, &clock, r, body_yaw);
            match net {
                Some(mut n) => {
                    if *n != next {
                        *n = next;
                    }
                }
                None => {
                    commands.entity(e).insert(next);
                }
            }
        }
    }
}

/// An angle into (-pi, pi].
fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = a.rem_euclid(TAU);
    if a > PI { a - TAU } else { a }
}

/// The driver's inputs from the character's state (5). "Fire held"
/// (Open question 17): the server's (`server`) is the frame's raw fire
/// flag (cleared only while use is held); a client's own body's is masked
/// by its weapon being in a firing state (not reloading, not empty).
fn inputs(
    characters: &CharacterData,
    anim: &mut CaAnim,
    intent: &Intent,
    movement: Option<&CaMovement>,
    weapon: Option<(&Weapon, Option<&WeaponState>, Option<&Magazine>, Option<&CaZoom>, Option<&CaSpread>)>,
    protected: Option<&CaProtected>,
    r: f32,
    server: bool,
    now: f64,
) -> Inputs {
    let look = characters.weapon(weapon.map(|w| w.0.id));
    let reloading = weapon.and_then(|w| w.1).is_some_and(|s| s.reloading());
    // The driver's "ammo count 0" is the weapon's total ammunition
    // (Corrections 6), not the clip.
    let empty = weapon.and_then(|w| w.2).is_some_and(|m| driver::ammo_exhausted(m.clip, m.reserve));
    let magazine = weapon.is_some_and(|w| w.2.is_some());
    let in_jump = movement.is_some_and(|m| m.in_jump);
    let (jumped, landed) = (in_jump && !anim.in_jump, !in_jump && anim.in_jump);
    anim.in_jump = in_jump;
    let coin = anim.coin();
    let m = movement.cloned().unwrap_or_default();
    // The server's own reload timer (7): from the third-person reload
    // animation's length at the reload's start; the reload may finish
    // when it runs out.
    if !reloading {
        anim.server_reload = None;
    } else if server && anim.server_reload.is_none() {
        let (name, _, _) = driver::upper_names(look.ani_type, driver::Posture::Reload, driver::Direction::None);
        let length = anim.driver.find(&name).map(|e| anim.driver.list.entries[e].length());
        let frame = look.frames.get(driver::Posture::Reload.frame_index()).copied().unwrap_or(0);
        anim.server_reload = Some((now, driver::server_reload_timer(length, 1.0, frame)));
    }
    let may_finish = server
        && anim
            .server_reload
            .is_some_and(|(since, timer)| timer.is_some_and(|t| (now - since) * 1000.0 + 1e-6 >= t as f64));
    Inputs {
        class: look.ani_type,
        legs: look.lower_ani_type,
        status: anim.pending.take(),
        reloading,
        reload_may_finish: may_finish,
        fire: driver::fire_held(intent.fire, intent.use_key, server, magazine, reloading, empty),
        alt_fire: intent.secondary && matches!(look.ani_type, 14 | 16),
        empty,
        run: m.run,
        sprint: m.sprint && m.moving,
        zoomed: weapon.and_then(|w| w.3).is_some_and(CaZoom::zoomed),
        jumped,
        landed,
        shot: std::mem::take(&mut anim.shot),
        no_footing: false,
        grounded: false,
        crouch_key: m.crouch,
        forward: m.forward,
        back: m.back,
        left: m.left,
        right: m.right,
        crouch: r,
        damaged: anim.damaged,
        protected: protected.is_some(),
        coin,
        frames: look.frames,
        rates: Default::default(),
    }
}

/// The pose's results in the character's frame.
/// `face`: the worn face piece's sockets (`models::piece_sockets`), for
/// the eyes when the body has none of theirs.
pub fn pose_of(anim: &CaAnim, posed: &Posed, body_yaw: f32, squash: f32, face: Option<&[(String, String, Vec3, Quat)]>) -> CaPose {
    let skel = records::builtin().skeleton(anim.skeleton);
    let body = |p: Vec3| skeleton::to_body(p, posed.feet);
    let left_foot = if anim.rig.is_some() {
        posed.node("Left_foot").map(|j| posed.globals[j].0)
    } else {
        // The spec's stand pose, squashed (the aim never moves the legs).
        let [x, y, z] = skel.left_foot;
        Some(Vec3::new(x, skeleton::STAND_FEET + (y - skeleton::STAND_FEET) * squash, z))
    };
    CaPose {
        spheres: skel
            .spheres
            .iter()
            .map(|s| (s.radius > 0.0).then(|| posed.sphere(&s.name)).flatten().map(body))
            .collect(),
        server_spheres: Vec::new(),
        skeleton: anim.skeleton,
        name_tag: posed.place("NAME_socket").map(|p| body(p.0)),
        left_foot: left_foot.map(body),
        right_hand: posed.place("RightHand").map(|p| body(p.0)),
        eyes: ["EYE_L", "EYE_R"].map(|n| {
            posed
                .place(n)
                .map(|p| p.0)
                .or_else(|| {
                    let (_, node, offset, _) = face?.iter().find(|s| s.0.eq_ignore_ascii_case(n))?;
                    let (p, q) = *posed.globals.get(posed.node(node)?)?;
                    Some(p + q * *offset)
                })
                .map(body)
        }),
        body_yaw,
        modelled: anim.rig.is_some(),
    }
}

/// The server's animation state for the network: each tracker's name
/// and the server tick its time 0 fell on.
fn net_of(anim: &CaAnim, clock: &SimClock, r: f32, body_yaw: f32) -> CaAnimNet {
    let d = &anim.driver;
    let step = clock.delta.as_secs_f64().max(1e-9);
    let at = |t: &lithtech_formats::anim::Tracker| {
        let start = clock.tick as f64 - t.time as f64 / 1000.0 / (t.rate.max(1e-3) as f64) / step;
        (d.name_of(t).to_string(), start.round())
    };
    CaAnimNet {
        upper: at(&d.upper),
        lower: at(&d.lower),
        crouch: at(&d.crouch),
        r: (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        pitch: skeleton::aim_byte(anim.pitch),
        twist: skeleton::aim_byte(anim.twist),
        death: anim.death.clone(),
        body_yaw,
    }
}

/// A client's view of the server's tick (now) and tick length, for
/// other players' trackers.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ClientClock(pub f64, pub f64);

/// Other players' trackers from the server's state (a client).
fn follow(anim: &mut CaAnim, net: &CaAnimNet, tick: f64, step: f64) {
    let list = anim.driver.list.clone();
    let d = &mut anim.driver;
    for (t, (name, start)) in [(&mut d.upper, &net.upper), (&mut d.lower, &net.lower), (&mut d.crouch, &net.crouch)] {
        let Some(entry) = list.find(name) else { continue };
        if t.current != entry {
            t.start(entry);
        }
        let ms = ((tick - start) * step * 1000.0).max(0.0) as u32;
        t.set_time(&list, ms);
    }
    d.crouch.multiplier = net.r as f32 / 255.0;
    anim.pitch = skeleton::from_aim_byte(net.pitch);
    anim.twist = skeleton::from_aim_byte(net.twist);
}

/// A client keeps `ClientClock` from the network's server time.
pub fn client_clock(world: &mut World) {
    if let Some(t) = ServerTime::of(world) {
        world.insert_resource(ClientClock(t.tick, t.step));
    }
}

/// Bodies: their owner's respawn noted, removed when their time comes or
/// at a new round.
fn bodies(
    mut bodies: Query<(Entity, &mut CaBody)>,
    owners: Query<(&Health, Has<Dead>)>,
    clock: Res<SimClock>,
    restarts: Res<RoundRestarts>,
    mut round: Local<Option<u32>>,
    settings: Option<Res<super::rules::CaRuleSettings>>,
    minigame: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    mut commands: Commands,
) {
    let new_round = round.is_some_and(|r| r != restarts.0);
    *round = Some(restarts.0);
    let delay = match settings.and_then(|s| super::rules::mode_by_key(&s.mode)) {
        Some(m) => records::builtin().body_removal(m.id),
        // No mode of ours: a minigame's match (Lucker Party's) removes
        // bodies as our like mode does, a free-for-all as One Man Army
        // (FFA), teams as Team Deathmatch: 5 s.
        None => minigame
            .map(|m| match m.def.teams {
                chimera_sim::minigame::def::TeamsDef::FreeForAll => super::rules::ONE_MAN_ARMY,
                _ => super::rules::TDM,
            })
            .and_then(|id| records::builtin().body_removal(id)),
    };
    for (e, mut b) in &mut bodies {
        if new_round {
            commands.entity(e).despawn();
            continue;
        }
        let alive = owners.get(b.owner).is_ok_and(|(h, dead)| h.current > 0.0 && !dead);
        if alive && b.respawned.is_none() {
            b.respawned = Some(clock.now);
        }
        // "Done" one millisecond after its last key.
        let end = b.since + b.length + 0.001;
        if death::removal_time(end, b.respawned, delay).is_some_and(|t| clock.now + 1e-9 >= t) {
            commands.entity(e).despawn();
        }
    }
}

/// The corpse (10, Corrections 12): the body object is a drawn model of
/// its own, in the dead player's skin, posed by the death animation at the
/// body's age, and stays where the player fell when its owner respawns.
/// Without the install's models (or the selection's body) nothing is
/// drawn; the body is still there. Run by `dress` (a system of its own
/// would add ambiguous pairs with the exclusive rule systems).
#[allow(clippy::type_complexity)]
fn dress_corpses(
    data: &CaData,
    models: &CharacterModels,
    now: f64,
    bodies: &Query<(
        Entity,
        &CaBody,
        (Option<&BodyName>, Option<&BodyNumber>, Option<&BodySkin>, Option<&Corpse>),
        Option<&Animator>,
    )>,
    commands: &mut Commands,
) {
    for (e, b, (name, number, skin, corpse), animator) in bodies {
        let Some((want, body)) = models::drawn(data, &b.look) else { continue };
        if !models.0.iter().any(|m| m.name.as_deref() == Some(want.as_str())) {
            continue;
        }
        let mut entity = commands.entity(e);
        if name.is_none_or(|n| n.0 != want) {
            entity.insert(BodyName(want));
        }
        if number.is_none_or(|n| n.0 != body) {
            entity.insert(BodyNumber(body));
        }
        // No team mark on a corpse: every mark is transparent (11).
        let family = BodySkin(models::family(b.look.camouflage, false));
        if skin != Some(&family) {
            entity.insert(family);
        }
        let corpse_here = Corpse {
            feet: -STAND_HEIGHT * 0.5 * unit(),
        };
        if corpse != Some(&corpse_here) {
            entity.insert(corpse_here);
        }
        let Some(rig) = body_of(data, &b.look).2 else { continue };
        let age = ((now - b.since).max(0.0) * 1000.0) as u32;
        // Posed once its animation has ended and held.
        let ended = animator.is_some_and(|a| a.fixed.is_some()) && f64::from(age) > b.length * 1000.0 + 100.0;
        if ended {
            continue;
        }
        let mut pose = Animator::default();
        let entry = rig.list.find(&b.animation);
        let mut tracker = lithtech_formats::anim::Tracker::on(entry.unwrap_or(0), false);
        tracker.set_time(&rig.list, age);
        let locals: Vec<_> = (0..rig.model.nodes.len()).map(|j| tracker.pose(&rig.list, j)).collect();
        pose.fixed = Some(Arc::new(rig.posed(&locals, 0.0).locals_rh()));
        pose.main = Some(0);
        entity.insert(pose);
    }
}

/// The held model a character's weapon wants with its fitted parts on it
/// (`view_model::parts::view_key`; None: the bare gun's): what
/// `held_on_demand` loads, `Held` until it is there.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct WantHeld(pub Option<String>);

/// Characters wear their body model when the map has it (`BodyName`,
/// with the body number for their body, face and head piece:
/// `models::drawn`), with their skin family for the local player
/// (camouflage, a teammate's mark), and hold their weapon (`Held`).
#[allow(clippy::type_complexity)]
fn dress(
    data: Res<CaData>,
    models: Option<Res<CharacterModels>>,
    characters: Query<(
        Entity,
        &Selection,
        Option<&Team>,
        (Option<&BodyName>, Option<&BodyNumber>, Option<&BodySkin>),
        Option<&Inventory>,
        (Option<&Held>, Option<&GameIdentity>, Option<&WantHeld>),
    )>,
    local: Query<Option<&Team>, With<LocalPlayer>>,
    weapons: Query<(&Weapon, Option<&super::attachments::CaMods>)>,
    late: Option<Res<crate::map::LateHeldModels>>,
    anims: Query<&CaAnim>,
    view: Option<Res<super::rules::CaMatchView>>,
    clock: Res<SimClock>,
    bodies: Query<(
        Entity,
        &CaBody,
        (Option<&BodyName>, Option<&BodyNumber>, Option<&BodySkin>, Option<&Corpse>),
        Option<&Animator>,
    )>,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    dress_corpses(&data, &models, clock.now, &bodies, &mut commands);
    let quarantine = view.is_some_and(|v| v.mode == super::gear::QUARANTINE);
    let mine = local.iter().next().flatten().copied();
    for (e, look, team, (name, number, skin), inventory, (held, identity, want_held)) in &characters {
        // Another game's character for now (`GameIdentity`: the weapon it
        // holds): our look comes off; that game dresses it.
        if !GameIdentity::allows(identity, super::GAME) {
            if name.is_some() || number.is_some() || skin.is_some() {
                commands.entity(e).remove::<(BodyName, BodyNumber, BodySkin)>();
            }
            continue;
        }
        let Some((want, body)) = models::drawn(&data, look) else { continue };
        if !models.0.iter().any(|m| m.name.as_deref() == Some(want.as_str())) {
            continue;
        }
        if name.is_none_or(|n| n.0 != want) {
            commands.entity(e).insert(BodyName(want));
        }
        if number.is_none_or(|n| n.0 != body) {
            commands.entity(e).insert(BodyNumber(body));
        }
        let teammate = models::shows_mark(mine.is_some() && team.copied() == mine, quarantine);
        let family = BodySkin(models::family(look.camouflage, teammate));
        if skin != Some(&family) {
            commands.entity(e).insert(family);
        }
        // An emote hides the weapon in hand (2).
        let emoting = anims.get(e).is_ok_and(|a| a.driver.emote.is_some());
        let active = inventory.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()).filter(|_| !emoting);
        // With drawn parts, the model with them (`held_on_demand` makes it
        // from `WantHeld`), the bare gun's until it is there.
        let full = active.map(|(w, mods)| super::view_model::parts::view_key(w.id, mods));
        let with_parts = full.filter(|k| active.is_some_and(|(w, _)| k != w.id));
        let id = match &with_parts {
            Some(k) if late.as_ref().is_some_and(|l| l.has(k)) => Some(k.clone()),
            _ => active.map(|(w, _)| w.id.to_string()),
        };
        if want_held.is_none_or(|w| w.0 != with_parts) {
            commands.entity(e).insert(WantHeld(with_parts));
        }
        if held.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(Held(id));
        }
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::object_visibility;
    use bevy::prelude::Visibility;

    /// A bot that died as a Combat Arms soldier (its object hidden for
    /// the death body) and respawned holding a CS:S gun is seen again; one
    /// that dies holding a CS:S gun is not hidden by Combat Arms.
    #[test]
    fn a_living_character_is_never_left_hidden() {
        // (ours, alive, hidden) -> change
        assert_eq!(object_visibility(true, false, false), Some(Visibility::Hidden), "ours, dead: hidden");
        assert_eq!(object_visibility(true, true, true), Some(Visibility::Inherited), "ours, back alive");
        assert_eq!(
            object_visibility(false, true, true),
            Some(Visibility::Inherited),
            "another game's identity now, alive: shown (it stayed hidden)"
        );
        assert_eq!(object_visibility(false, false, false), None, "another game's dead body isn't ours to hide");
        assert_eq!(object_visibility(true, true, false), None);
        assert_eq!(object_visibility(true, false, true), None);
    }
}

/// Combat Arms' bodies as Chimera's hitboxes (docs/DESIGN.md "Hits across games"),
/// so other games' guns hit the drawn body by region: after the pose,
/// every hit sphere of a Combat Arms character's table where its own
/// guns test it (`weapons::body_geometry`, a body model's pose by
/// `weapons::posed_geometry`), as a `core::Hitbox` sphere in the frame
/// shots turn it by (feet at its box's bottom, the look's yaw), with its
/// region (`HitSphere::region`), `Node#Priority` and zone factor as the
/// body's own scale. Characters of another game's identity wear that
/// game's body and hitboxes.
#[allow(clippy::type_complexity)]
fn hitboxes(
    mut characters: Query<(
        Entity,
        (&Transform, &Intent, &MovementState, Option<&avian3d::prelude::ColliderAabb>),
        (&HitSpheres, Option<&CaPose>, Option<&GameIdentity>),
        (Option<&mut core::Hitboxes>, Has<CaHitboxes>),
    )>,
    mut commands: Commands,
) {
    for (e, (t, intent, state, aabb), (table, pose, identity), (current, made)) in &mut characters {
        if !GameIdentity::allows(identity, super::GAME) {
            // Now another game's: its body's hitboxes (or none), not ours.
            // Checked when the command runs: that game's body may have put
            // its own in this tick (`map::bodies::attach_hitboxes`).
            if made {
                commands.entity(e).queue(|mut ent: EntityWorldMut| {
                    let ours = ent.get::<core::Hitboxes>().is_some_and(|h| h.0.iter().all(|b| b.radius.is_some()));
                    if ours {
                        ent.remove::<core::Hitboxes>();
                    }
                    ent.remove::<CaHitboxes>();
                });
            }
            continue;
        }
        let data = table.0;
        let Some((min, _, mut world)) = super::weapons::body_geometry(t, intent, state, aabb, data) else {
            continue;
        };
        if let Some(p) = pose.filter(|p| p.modelled && p.skeleton == data.first().map_or(0, |s| s.skeleton)) {
            world = super::weapons::posed_geometry(t.translation, min, p.body_yaw, p, data);
        }
        let feet = aabb.map_or(t.translation, |b| t.translation.with_y(b.min.y));
        let turn = intent.yaw_rotation().inverse();
        let shapes: Vec<core::Hitbox> = data
            .iter()
            .zip(&world)
            .map(|(s, (c, r))| core::Hitbox {
                priority: s.priority,
                scale: Some(s.factor),
                ..core::Hitbox::sphere(turn * (*c - feet), *r, s.region())
            })
            .collect();
        match current {
            Some(mut h) => h.0 = shapes,
            None => {
                commands.entity(e).insert(core::Hitboxes(shapes));
            }
        }
        if !made {
            commands.entity(e).insert(CaHitboxes);
        }
    }
}

/// On a character whose `core::Hitboxes` Combat Arms makes (`hitboxes`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CaHitboxes;
