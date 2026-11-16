//! The campaign's characters as bodies a party player wears on a Halo rung
//! (specs/halo_ce/characters.md; docs/plans/active/halo-ce.md, "Play as
//! the campaign's characters"). Party apps only (`game::BUNDLE`'s `sim`
//! adds `CampaignPlugin`): Halo's own app never offers these bodies. The
//! characters: the Elite, the Grunt (its camera at its chest, 0.28 WU, as
//! the game has it; 40 body, no shield) and the Jackal (60 body; its 100
//! shield is the disc on its arm: `shield_disc` hides the disc while it is
//! down and `shield_boxes` shrinks its hit box to the emitter, so shots
//! pass where it was; the disc's hits go to the shield material through
//! `body::hits::SHIELD_REGION`).
//!
//! - **The body kind** (`BodyKind`: Spartan, Elite; a component on the
//!   player, replicated to every client, net part `halo_ce_bodies`). Each
//!   kind but the Spartan is a row of data/campaign.toml (`CampaignData`:
//!   its biped, the campaign map its unit is cut from, and what differs
//!   from the Spartan's movement and vitality tables, merged key by key):
//!   adding the Grunt, the Jackal, the Hunter, the Marine or the Flood is
//!   a variant here, a row there and its unit (`unit`).
//! - **Its body**: every machine names the kind's model for the character
//!   (`name_bodies`: the engine's `map::BodyName`, `halo_ce:elite`; the
//!   Spartan none), so the engine picks it among Halo's models
//!   (`CharacterModels::for_identity`) once its unit has loaded (`unit`:
//!   on demand, in the background); until then, or without the unit, the
//!   Spartan's. The third-person model, its graph's animations (stand,
//!   move, crouch, aim and the held gun: `body::animate`, the same code as
//!   the Spartan's; its move animations at the body's ground speed over
//!   the speeds they show, `AnimSpeeds`, so the feet keep up; a direction
//!   it has no animation for plays its forward move, the air without an
//!   airborne animation holds the last pose; the body faces the aim at
//!   once), its hit boxes from its own collision model (`body::hits`), its
//!   deaths and flinches from its own graph (`body::death`,
//!   `body::flinch`: the corpse carries the kind) all follow from the
//!   model.
//! - **How it moves** (`movement_of`, the one place that decides it): the
//!   owner's rule (2026-11-16: player expression over fidelity for
//!   movement) is that a campaign body moves on the Spartan's mover, not
//!   by its animations as the game does (characters.md section 4): the
//!   Spartan's acceleration, air control, jump and gravity, every
//!   direction, no landing freeze, no limit on the aim or where shots go;
//!   only its speed differs, the Spartan's run and crouch speeds times
//!   1 + blend x (ratio - 1) (`CampaignData::speed_factor`: the shared
//!   blend 0.5 and the character's own ratio, the Elite's 1.15, so 107.5
//!   %). The kind's `MovementData` is the Spartan's table with the
//!   character's own pill and camera (the Elite's 0.8 / 0.5 WU, radius
//!   0.2, camera 0.6 / 0.3) and any key of data/movement.toml a later
//!   tuning pass sets (acceleration, air control, jump: data, not code),
//!   fitted to the map's envelope by `movement::body` as the Spartan's is.
//! - **Its colour**: the party's colour for the player (team or
//!   free-for-all, as the Spartan's) through the kind's own change colour
//!   slot (`BodyDef::colour_slot`: the Elite's A, all over its armour),
//!   the minor's blue for a player given none (`BodyDef::uncoloured`).
//! - **Its vitality** (`vitality_of`): the kind's `Spartan` table (the
//!   Elite's collision model: 100 body, 100 shield of type 30, a 12 s
//!   delay, 10 s recharge, its four materials with the arms at x0.5).
//! - **Choosing it** (`arrive`, the host's): with the host setting
//!   `halo_ce:campaign_bodies` 1 (`CampaignBodies`; default 0, off), each
//!   time a player arrives on a Halo rung (`ladder::OnRung` inserted) its
//!   body is drawn by the gun's weights (`CampaignData::weights`; a gun not
//!   listed: the Spartan) among the kinds whose unit is loaded here
//!   (`UnitsOnDemand::ready`), from the match's own randomness
//!   (`roll`: the ladder's seed, the ladders made, the player's seed, the
//!   rung and the player's count of draws; no process-wide state). Off, or
//!   on another game's rung, the player has no kind (the Spartan). Turning
//!   the setting on asks for every kind's unit at once, so the first draws
//!   find them loaded.
//! - **The developer command** `halo_ce:body <spartan | elite | grunt |
//!   jackal | draw> [player | all]` forces a body (`ForcedBody`, kept across
//!   rungs while on Halo's; `draw` lets the draw choose again) for
//!   screenshots and the sandbox (the player's own body there: worn at
//!   once on a Halo rung, else on the next); `status` lists the units and
//!   who wears what, `shield <fraction>` sets every campaign body's shield;
//!   a remote client's needs the server's `sv_cheats` 1.
//! - **In the sandbox** (`targets`): the kinds as practice targets
//!   (`target add halo_ce:elite 5`), offered once their units loaded,
//!   which the sandbox asks for as it starts.
//!
//! - **The gun in its hand**: each gun the kind may carry is a held
//!   model of its own (`view::load::body_held`, key `<gun>#<kind>`), placed
//!   by the kind's own hand marker for the gun's label (the Elite's `right
//!   hand elite`), made with the unit (`unit::held_on`); `view::stance`
//!   holds it once loaded (`view::OWN_HAND`; until then the Spartan's
//!   placement). The off hand is free (characters.md section 2 rule 4: no
//!   Covenant class pins it): the arm moves only as the graph's holding
//!   and aiming animations move it.
//!
//! Known shortcuts:
//! - **Its sounds** (`voice`): its own footsteps, jump and landing
//!   sounds from its unit, and its pain, death, scream and maimed lines
//!   from its voice set (none without it), heard by every player at the
//!   body.
//! - The move animations' speeds are the graph's means (characters.md
//!   section 4), not each frame's root motion.

pub mod targets;
pub mod unit;
pub mod voice;

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub use unit::UnitsOnDemand;

use super::{GAME, movement::MovementData, vitality::Spartan};
use crate::map::BodyName;

/// data/campaign.toml, compiled in.
pub const TABLE: &str = include_str!("../../../data/campaign.toml");

/// The host setting's console variable.
pub const CVAR: &str = "halo_ce:campaign_bodies";

/// The developer command.
pub const BODY_COMMAND: &str = "halo_ce:body";

/// A Halo player's body (module doc): replicated, the host's.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BodyKind {
    #[default]
    Spartan,
    Elite,
    Grunt,
    Jackal,
}

impl BodyKind {
    pub const ALL: [BodyKind; 4] = [BodyKind::Spartan, BodyKind::Elite, BodyKind::Grunt, BodyKind::Jackal];

    /// Its id in data/campaign.toml and the commands.
    pub fn id(self) -> &'static str {
        match self {
            BodyKind::Spartan => "spartan",
            BodyKind::Elite => "elite",
            BodyKind::Grunt => "grunt",
            BodyKind::Jackal => "jackal",
        }
    }

    pub fn from_id(id: &str) -> Option<BodyKind> {
        Self::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(id))
    }

    /// The name of its character model (`map::BodyName`); the Spartan's is
    /// Halo's unnamed model.
    pub fn model_name(self) -> Option<String> {
        (self != BodyKind::Spartan).then(|| format!("{GAME}:{}", self.id()))
    }
}

/// One character's row of data/campaign.toml.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyDef {
    /// Its biped tag.
    pub biped: String,
    /// The campaign map its unit is cut from (`maps/<name>.map`).
    pub donor_map: String,
    /// Keys of data/movement.toml that differ from the Spartan's.
    #[serde(default)]
    pub movement: toml::Table,
    /// Tables of data/vitality.toml that differ (merged key by key).
    #[serde(default)]
    pub vitality: toml::Table,
    /// The game's own moving speed over the Spartan's (1: the same).
    #[serde(default = "one")]
    pub speed_ratio: f32,
    /// The speeds its move animations show (`AnimSpeeds`).
    #[serde(default)]
    pub animation: Option<AnimSpeeds>,
    /// The change colour slot its armour takes the player's colour from
    /// (`A` to `D`; characters.md "The Elite's colours"); none: untinted.
    #[serde(default)]
    pub colour_slot: Option<char>,
    /// Its colour for a player given none (linear RGB 0..1).
    #[serde(default)]
    pub uncoloured: Option<[f32; 3]>,
    /// An arm shield's disc (the Jackal's): the text its meshes' shader
    /// names hold; drawn only while the shield is up, and its hit box
    /// shrinks to the emitter while it is down (`shield_disc`).
    #[serde(default)]
    pub shield_disc: Option<String>,
}

impl BodyDef {
    /// How its armour takes the player's colour (`body::load::Tint`).
    pub fn tint(&self) -> super::body::load::Tint {
        use blam_formats::shader::AnimationSource as S;
        let source = match self.colour_slot.map(|c| c.to_ascii_uppercase()) {
            Some('A') => S::A,
            Some('B') => S::B,
            Some('C') => S::C,
            Some('D') => S::D,
            _ => S::None,
        };
        super::body::load::Tint { source, uncoloured: self.uncoloured }
    }
}

fn one() -> f32 {
    1.0
}

fn half() -> f32 {
    0.5
}

/// The speeds a body's move animations show, WU/s (its graph's mean root
/// motion): its legs play at the ground speed over these
/// (`body::anims::Inputs::rate`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimSpeeds {
    pub front: f32,
    pub back: f32,
    pub side: f32,
    pub crouch_front: f32,
    pub crouch_back: f32,
    pub crouch_side: f32,
}

impl AnimSpeeds {
    /// As `Inputs::own_speeds` takes them.
    pub fn array(&self) -> [f32; 6] {
        [self.front, self.back, self.side, self.crouch_front, self.crouch_back, self.crouch_side]
    }
}

/// data/campaign.toml (module doc).
#[derive(Resource, Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignData {
    /// The share of a character's own speed ratio its body runs at (module
    /// doc, "How it moves").
    #[serde(default = "half")]
    pub speed_blend: f32,
    /// Per weapon id, each body's weight (by `BodyKind::id`).
    #[serde(default)]
    pub guns: BTreeMap<String, BTreeMap<String, f32>>,
    /// Per body id but the Spartan's, its row.
    #[serde(default)]
    pub bodies: BTreeMap<String, BodyDef>,
}

/// `over` merged into `base` key by key (tables recursively, anything else
/// replaced).
pub fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

impl Default for CampaignData {
    fn default() -> Self {
        Self { speed_blend: half(), guns: BTreeMap::new(), bodies: BTreeMap::new() }
    }
}

impl CampaignData {
    /// The table compiled in (a test checks it parses).
    pub fn builtin() -> Self {
        Self::parse(TABLE).expect("data/campaign.toml parses (a test checks it)")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let data: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        for id in data.bodies.keys().chain(data.guns.values().flat_map(|w| w.keys())) {
            if BodyKind::from_id(id).is_none() {
                return Err(format!("unknown body `{id}`"));
            }
        }
        Ok(data)
    }

    /// Body `kind`'s row (None: the Spartan, or not in the table).
    pub fn body(&self, kind: BodyKind) -> Option<&BodyDef> {
        self.bodies.get(kind.id())
    }

    /// The bodies that may carry `weapon`, with their weights (a gun not
    /// listed: the Spartan's alone).
    pub fn weights(&self, weapon: &str) -> Vec<(BodyKind, f32)> {
        match self.guns.get(weapon) {
            Some(w) => w.iter().filter_map(|(id, &x)| Some((BodyKind::from_id(id)?, x))).filter(|(_, x)| *x > 0.0).collect(),
            None => vec![(BodyKind::Spartan, 1.0)],
        }
    }

    /// The factor on the Spartan's speeds a body of speed ratio `ratio`
    /// runs at: 1 + blend x (ratio - 1).
    pub fn speed_factor(&self, ratio: f32) -> f32 {
        1.0 + self.speed_blend * (ratio - 1.0)
    }

    /// Body `kind`'s movement: `base` with its row's keys, its run and
    /// crouch speeds times `speed_factor` of its ratio.
    pub fn movement(&self, kind: BodyKind, base: &MovementData) -> Result<MovementData, String> {
        let Some(def) = self.body(kind) else { return Ok(base.clone()) };
        let mut t = toml::Table::try_from(base).map_err(|e| e.to_string())?;
        merge(&mut t, &def.movement);
        let mut m: MovementData = t.try_into().map_err(|e: toml::de::Error| format!("{} movement: {e}", kind.id()))?;
        let f = self.speed_factor(def.speed_ratio);
        for v in [
            &mut m.run_forward,
            &mut m.run_backward,
            &mut m.run_sideways,
            &mut m.sneak_forward,
            &mut m.sneak_backward,
            &mut m.sneak_sideways,
        ] {
            *v *= f;
        }
        Ok(m)
    }

    /// Body `kind`'s vitality: data/vitality.toml with its row's tables.
    pub fn vitality(&self, kind: BodyKind) -> Result<Spartan, String> {
        let mut t: toml::Table = toml::from_str(super::vitality::data::VITALITY).map_err(|e| e.to_string())?;
        if let Some(def) = self.body(kind) {
            merge(&mut t, &def.vitality);
        }
        t.try_into().map_err(|e: toml::de::Error| format!("{} vitality: {e}", kind.id()))
    }
}

/// Each kind's movement and vitality tables (made once from
/// `CampaignData`): what `movement_of` and `vitality_of` read.
#[derive(Resource, Clone, Debug, Default)]
pub struct BodyTables {
    pub movement: BTreeMap<BodyKind, MovementData>,
    pub vitality: BTreeMap<BodyKind, Spartan>,
    pub animation: BTreeMap<BodyKind, AnimSpeeds>,
}

impl BodyTables {
    pub fn build(data: &CampaignData, base: &MovementData) -> Result<Self, String> {
        let mut t = Self::default();
        for kind in BodyKind::ALL.into_iter().filter(|k| *k != BodyKind::Spartan) {
            if data.body(kind).is_none() {
                continue;
            }
            t.movement.insert(kind, data.movement(kind, base)?);
            t.vitality.insert(kind, data.vitality(kind)?);
            if let Some(a) = data.body(kind).and_then(|d| d.animation) {
                t.animation.insert(kind, a);
            }
        }
        Ok(t)
    }
}

/// How a body moves (module doc, "How it moves"): the movement table of
/// `kind`, the Spartan's (`base`) without a kind or a table of its own.
/// The one place this is decided: the animation-driven mover of the next
/// stage replaces what this returns for a kind without player physics.
pub fn movement_of<'a>(base: &'a MovementData, tables: Option<&'a BodyTables>, kind: Option<&BodyKind>) -> &'a MovementData {
    kind.and_then(|k| tables?.movement.get(k)).unwrap_or(base)
}

/// A body's vitality table (`spartan` without a kind or a table of its
/// own).
pub fn vitality_of<'a>(spartan: &'a Spartan, tables: Option<&'a BodyTables>, kind: Option<&BodyKind>) -> &'a Spartan {
    kind.and_then(|k| tables?.vitality.get(k)).unwrap_or(spartan)
}

/// The host setting: campaign bodies on for the match (module doc).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CampaignBodies(pub bool);

/// A body a developer forced on a player (`BODY_COMMAND`): every Halo rung
/// gives it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForcedBody(pub BodyKind);

/// How many bodies were drawn for a player (the next draw's input).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BodyDraws(pub u64);

/// The body `BodyName` came from here (`name_bodies` takes it off again).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NamedByKind;

/// A 64-bit mix of `parts` (splitmix64 over them): the draw's roll.
pub fn roll(parts: &[u64]) -> u64 {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for &p in parts {
        x ^= p;
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x = z ^ (z >> 31);
    }
    x
}

/// The body `roll` picks among `weights` whose kind is `ready` (none
/// ready: the Spartan).
pub fn draw(weights: &[(BodyKind, f32)], ready: impl Fn(BodyKind) -> bool, roll: u64) -> BodyKind {
    let pool: Vec<(BodyKind, f32)> = weights.iter().copied().filter(|(k, w)| *w > 0.0 && ready(*k)).collect();
    let total: f32 = pool.iter().map(|(_, w)| w).sum();
    if pool.is_empty() || total <= 0.0 {
        return BodyKind::Spartan;
    }
    // 24 bits of the roll as a fraction of the total (no float from the
    // platform's maths: a product and sums).
    let mut x = (roll >> 40) as f32 / (1u64 << 24) as f32 * total;
    for (k, w) in &pool {
        if x < *w {
            return *k;
        }
        x -= w;
    }
    pool[pool.len() - 1].0
}

/// Whether `weapon` is one of Halo's.
fn halo_gun(weapon: &str) -> bool {
    weapon.split_once(':').is_some_and(|(g, _)| g.eq_ignore_ascii_case(GAME))
}

/// The host: player `e` arrived on a rung (module doc, "Choosing it").
fn arrive(on: On<Insert<chimera_sim::minigame::ladder::OnRung>>, role: Option<Res<crate::core::NetRole>>, mut commands: Commands) {
    if role.is_some_and(|r| *r == crate::core::NetRole::Client) {
        return;
    }
    let e = on.event_target();
    commands.queue(move |world: &mut World| choose(world, e));
}

/// Player `e`'s body for the rung it is on (the host's): the forced one,
/// a draw, or none.
pub fn choose(world: &mut World, e: Entity) {
    use chimera_sim::minigame::ladder::{Ladder, LaddersMade, OnRung, RungSeed};
    let Some(index) = world.get::<OnRung>(e).map(|o| o.index) else { return };
    let weapon = world.get_resource::<Ladder>().and_then(|l| l.rungs.get(index).copied()).unwrap_or("");
    let kind = if !halo_gun(weapon) {
        None
    } else if let Some(f) = world.get::<ForcedBody>(e) {
        Some(f.0)
    } else if world.get_resource::<CampaignBodies>().is_some_and(|c| c.0) {
        let data = world.get_resource::<CampaignData>().cloned().unwrap_or_default();
        let draws = world.get::<BodyDraws>(e).copied().unwrap_or_default();
        let seed = world.get_resource::<RungSeed>().map_or(0, |s| s.0);
        let made = world.get_resource::<LaddersMade>().map_or(0, |m| m.0);
        let player = crate::core::Seed::of(world.get::<crate::core::Seed>(e), e);
        let r = roll(&[seed, made, player, index as u64, draws.0]);
        let units = world.get_resource::<UnitsOnDemand>();
        let k = draw(&data.weights(weapon), |k| units.is_some_and(|u| u.ready(k)), r);
        world.entity_mut(e).insert(BodyDraws(draws.0 + 1));
        Some(k)
    } else {
        None
    };
    let mut ent = world.entity_mut(e);
    match kind {
        Some(k) if ent.get::<BodyKind>() != Some(&k) => {
            ent.insert(k);
        }
        Some(_) => {}
        None => {
            ent.remove::<BodyKind>();
        }
    }
}

/// Every machine: each character's `BodyName` from its kind (module doc),
/// taken off again when the kind goes.
#[allow(clippy::type_complexity)]
pub fn name_bodies(
    q: Query<(Entity, Option<&BodyKind>, Option<&BodyName>, Has<NamedByKind>), Or<(With<BodyKind>, With<NamedByKind>)>>,
    mut commands: Commands,
) {
    for (e, kind, name, ours) in &q {
        match kind.and_then(|k| k.model_name()) {
            Some(want) => {
                if name.is_none_or(|n| n.0 != want) {
                    commands.entity(e).insert((BodyName(want), NamedByKind));
                }
            }
            None if ours => {
                commands.entity(e).remove::<(BodyName, NamedByKind)>();
            }
            None => {}
        }
    }
}

/// The host setting's console variable (default 0); turning it on asks
/// for every kind's unit (module doc).
fn campaign_cvar(app: &mut App) {
    use chimera_core::console::ConsoleAppExt;
    app.console_cvar(
        CVAR,
        "1: a player given a Halo rung may wear one of the campaign's characters (the Elite), drawn by \
         data/campaign.toml's weights (default 0: the Spartan). The host's.",
        "0",
        |w| Some(if w.get_resource::<CampaignBodies>().is_some_and(|c| c.0) { "1" } else { "0" }.to_string()),
        |w, v| {
            let on = match v.trim() {
                "0" => false,
                "1" => true,
                other => return Err(format!("bad value \"{other}\" (0 or 1)")),
            };
            w.insert_resource(CampaignBodies(on));
            if on {
                let kinds: Vec<BodyKind> =
                    BodyKind::ALL.into_iter().filter(|k| w.resource::<CampaignData>().body(*k).is_some()).collect();
                w.get_resource_or_init::<UnitsOnDemand>().wanted.extend(kinds);
            }
            Ok(())
        },
    );
}

/// `halo_ce:body <spartan | elite | draw> [player | all]` (module doc).
fn body_command(world: &mut World, a: &[String]) -> Result<Option<String>, String> {
    use chimera_core::console::{Cheats, CommandPlayer, command_player};
    let usage = "halo_ce:body <spartan | elite | grunt | jackal | draw | status | shield <fraction>> [player | all]";
    let what = a.first().ok_or(usage)?.to_ascii_lowercase();
    if what == "shield" {
        // Every campaign body's shield to a fraction (screenshots of the
        // Jackal's disc down and up).
        let f: f32 = a.get(1).and_then(|v| v.parse().ok()).ok_or("halo_ce:body shield <fraction>")?;
        let mut q = world.query_filtered::<&mut super::vitality::Vitality, With<BodyKind>>();
        let mut n = 0;
        for mut v in q.iter_mut(world) {
            v.shield = f;
            // Down: as a hit leaves it, the delay running.
            if f <= 0.0 {
                v.depleted = true;
                v.delay = 450;
            }
            n += 1;
        }
        return Ok(Some(format!("{n} shields at {f}")));
    }
    if what == "status" {
        let units = world.get_resource::<UnitsOnDemand>().map_or_else(|| "no units".into(), UnitsOnDemand::status);
        let mut q = world.query::<(
            Entity,
            Option<&Name>,
            Option<&BodyKind>,
            Option<&BodyName>,
            Option<&crate::map::BodyModel>,
            Option<&crate::map::Held>,
        )>();
        let rows: Vec<String> = q
            .iter(world)
            .filter(|r| r.2.is_some() || r.3.is_some())
            .map(|(e, n, k, b, m, h)| {
                let who = n.map_or_else(|| e.to_string(), |n| n.to_string());
                format!("{who}: {k:?} {:?} model {:?} holds {:?}", b.map(|b| &b.0), m.map(|m| m.0), h.and_then(|h| h.0.as_deref()))
            })
            .collect();
        return Ok(Some(format!("{units}
{}", rows.join("
"))));
    }
    let forced = if what == "draw" { None } else { Some(BodyKind::from_id(&what).ok_or(usage)?) };
    let remote = world.contains_resource::<CommandPlayer>();
    if remote && world.get_resource::<Cheats>().is_none_or(|c| c.0 == 0) {
        return Err(format!("Can't use cheat command {BODY_COMMAND} in multiplayer, unless the server has sv_cheats set to 1."));
    }
    let who: Vec<Entity> = match a.get(1).map(|t| t.to_ascii_lowercase()) {
        None => vec![command_player(world)?],
        Some(_) if remote => return Err(format!("a client's {BODY_COMMAND} acts on its own player alone")),
        Some(t) => {
            let players = chimera_sim::minigame::runtime::players(world);
            let found: Vec<Entity> = players
                .into_iter()
                .map(|(e, _)| e)
                .filter(|e| t == "all" || world.get::<Name>(*e).is_some_and(|n| n.as_str().to_ascii_lowercase().contains(&t)))
                .collect();
            if found.is_empty() {
                return Err(format!("no player {t:?}"));
            }
            found
        }
    };
    if let Some(k) = forced.filter(|k| *k != BodyKind::Spartan) {
        world.get_resource_or_init::<UnitsOnDemand>().wanted.insert(k);
    }
    for &e in &who {
        match forced {
            Some(k) => world.entity_mut(e).insert(ForcedBody(k)),
            None => world.entity_mut(e).remove::<ForcedBody>(),
        };
        choose(world, e);
        // Off a Halo rung, a character that is Halo's all the same (a
        // `halo_ce:dummy`) wears it at once; any other waits for the next
        // Halo rung.
        let halo = world.get::<chimera_core::slots::GameIdentity>(e).is_some_and(|i| i.0 == GAME);
        if let Some(k) = forced
            && halo
            && world.get::<BodyKind>(e).is_none()
        {
            world.entity_mut(e).insert(k);
        }
    }
    Ok(Some(format!("{} player(s): {}", who.len(), forced.map_or("drawn", |k| k.id()))))
}

/// Every machine: an arm shield's disc (`BodyDef::shield_disc`) drawn
/// while its shield is up, hidden while it is down (characters.md section
/// 2 rule 7: depleted, the region's permutation 1; back to 0 when the
/// recharge begins, which for the Jackal fills it at once).
pub fn shield_disc(
    q: Query<(Entity, &BodyKind, &super::vitality::Vitality, Option<&crate::map::BodyNumber>)>,
    data: Res<CampaignData>,
    mut commands: Commands,
) {
    for (e, kind, v, number) in &q {
        if data.body(*kind).is_none_or(|d| d.shield_disc.is_none()) {
            continue;
        }
        let want = crate::map::BodyNumber(i32::from(v.shield <= 0.0));
        if number != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

/// Half the size of an arm shield's emitter, metres (characters.md section
/// 2 rule 7: `~shield_off`'s BSP, 0.04 x 0.06 x 0.09 WU).
pub const EMITTER_HALF: Vec3 = Vec3::new(0.02 * 3.048, 0.03 * 3.048, 0.045 * 3.048);

/// After the hit boxes are posed: while an arm shield is down its node's
/// box is the emitter alone (shots pass where the disc was).
pub fn shield_boxes(mut q: Query<(&BodyKind, &super::vitality::Vitality, &mut crate::core::Hitboxes)>, data: Res<CampaignData>) {
    for (kind, v, mut boxes) in &mut q {
        if v.shield > 0.0 || data.body(*kind).is_none_or(|d| d.shield_disc.is_none()) {
            continue;
        }
        for h in boxes.0.iter_mut().filter(|h| h.region == super::body::hits::SHIELD_REGION) {
            h.half = h.half.min(EMITTER_HALF);
        }
    }
}

/// The kinds' replication (net part `halo_ce_bodies`).
pub fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    app.replicate::<BodyKind>();
}

/// The campaign's bodies in a party app (module doc).
pub struct CampaignPlugin;

impl Plugin for CampaignPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::console::ConsoleAppExt;
        use chimera_net::net::parts::NetAppExt;
        let data = CampaignData::builtin();
        let base = MovementData::builtin();
        let tables = BodyTables::build(&data, &base).expect("data/campaign.toml's tables merge (a test checks it)");
        app.insert_resource(data)
            .insert_resource(tables)
            .init_resource::<CampaignBodies>()
            .init_resource::<UnitsOnDemand>()
            .add_observer(arrive)
            .init_resource::<voice::CampaignSounds>()
            .add_message::<voice::RegionLost>()
            .add_systems(Update, (unit::load_on_demand, unit::keep_held, name_bodies, voice::join_banks, shield_disc))
            .add_systems(Update, targets::sandbox_wants.before(unit::load_on_demand))
            .add_systems(FixedUpdate, shield_boxes.after(crate::map::pose_hitboxes).before(crate::core::SimSet::Movement))
            .add_systems(FixedLast, shield_boxes.after(crate::map::SimPose))
            .add_systems(
                FixedUpdate,
                voice::speak
                    .after(crate::game::damage::deliver_hits)
                    .before(crate::core::apply_damage)
                    .run_if(crate::core::authoritative),
            )
            .net_part("halo_ce_bodies", register);
        campaign_cvar(app);
        app.console_command(
            BODY_COMMAND,
            "halo_ce:body <spartan | elite | grunt | jackal | draw | status | shield <fraction>> [player | all]: the \
             body a player (default: you) wears on Halo's rungs (a developer command for screenshots and the sandbox; draw: the draw chooses again; a client \
             needs the server's sv_cheats 1).",
            body_command,
        );
        if let Some(mut console) = app.world_mut().get_resource_mut::<chimera_core::console::Console>() {
            console.player_command(BODY_COMMAND);
        }
    }
}
