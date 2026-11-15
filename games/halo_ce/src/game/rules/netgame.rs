//! The scenario's multiplayer data the game types read, from the map to the
//! simulation (specs/halo_ce/scenario.md "Player starting locations",
//! "Netgame flags", "Starting equipment"; multiplayer_rules.md 4.3, 4.6,
//! 6.4 to 6.7): every player start with its four game type slots and team
//! index, every netgame flag (capture the flag spots, ball spawns, hills,
//! race checkpoints) and every starting equipment entry with its item
//! collections' weighted items, as map entities (so the converted-map
//! cache keeps them, as the pickups' entries are kept), and back as the
//! rules' resource (`Netgame`).
//!
//! The loader (`add_entities`) writes them only when the map's game is the
//! gameplay's (`map::KitLoad::pickups`): a party app's Halo map has none,
//! so Halo's rules never see a party's map. Positions are the engine's
//! metres (`maps::space::point`, the feet for a start), headings the
//! engine's yaw (`maps::space::yaw`); the rules measure in world units
//! (`WU`).

use bevy::prelude::*;
use blam_formats::{
    cache::{MapFile, class},
    scenario::{GameType, GameTypes, ItemCollection, NetgameFlagType, Scenario},
};

use super::super::maps::space;
use crate::map::{MapData, MapEntities, MapEntity, entities};

pub const START_CLASS: &str = "halo_ce_player_start";
pub const FLAG_CLASS: &str = "halo_ce_netgame_flag";
pub const EQUIPMENT_CLASS: &str = "halo_ce_starting_equipment";

/// Metres in a world unit.
pub const WU: f32 = space::METERS_PER_UNIT;

/// A collection's (weight, item tag) list as one keyvalue: `w=tag|w=tag`.
fn items_text(file: &MapFile, collection: &blam_formats::cache::TagRef) -> Option<String> {
    let c = blam_formats::weapon::ref_id(collection)
        .and_then(|id| file.tag(id))
        .filter(|t| t.class == class::ITEM_COLLECTION)
        .and_then(|t| ItemCollection::read(file, t).ok())?;
    let items: Vec<String> = c
        .permutations
        .iter()
        .filter_map(|p| {
            let tag = blam_formats::weapon::ref_id(&p.item).and_then(|id| file.tag(id))?;
            Some(format!("{}={}", p.weight, tag.path))
        })
        .collect();
    Some(items.join("|"))
}

fn slots(g: &GameTypes) -> String {
    format!("{} {} {} {}", g.0[0], g.0[1], g.0[2], g.0[3])
}

fn origin(p: Vec3) -> String {
    let o = entities::engine_to_entity(p, 1.0);
    format!("{} {} {}", o.x, o.y, o.z)
}

/// The map's starts, flags and starting equipment as entities (module
/// doc), when `rules` (the map's game is the gameplay's).
pub fn add_entities(file: &MapFile, scenario: &Scenario, map: &mut MapData, rules: bool) {
    if !rules {
        return;
    }
    for (i, s) in scenario.player_starts.iter().enumerate() {
        map.entities.push(MapEntity {
            keyvalues: vec![
                ("classname".into(), START_CLASS.into()),
                ("index".into(), i.to_string()),
                ("origin".into(), origin(space::point(s.position))),
                ("yaw".into(), space::yaw(s.facing).to_string()),
                ("team".into(), s.team_index.to_string()),
                ("game_types".into(), slots(&s.game_types)),
            ],
            ..Default::default()
        });
    }
    for (i, f) in scenario.netgame_flags.iter().enumerate() {
        map.entities.push(MapEntity {
            keyvalues: vec![
                ("classname".into(), FLAG_CLASS.into()),
                ("index".into(), i.to_string()),
                ("origin".into(), origin(space::point(f.position))),
                ("yaw".into(), space::yaw(f.facing).to_string()),
                ("type".into(), f.flag_type.code().to_string()),
                ("team".into(), f.team_index.to_string()),
            ],
            ..Default::default()
        });
    }
    for (i, e) in scenario.starting_equipment.iter().enumerate() {
        let collections: Vec<String> = e
            .item_collections
            .0
            .iter()
            .take(5)
            .map(|c| items_text(file, c).unwrap_or_default())
            .collect();
        map.entities.push(MapEntity {
            keyvalues: vec![
                ("classname".into(), EQUIPMENT_CLASS.into()),
                ("index".into(), i.to_string()),
                ("flags".into(), e.flags.to_string()),
                ("game_types".into(), slots(&e.game_types)),
                ("collections".into(), collections.join(";")),
            ],
            ..Default::default()
        });
    }
}

// ---------------------------------------------------------------------------
// The simulation's side.

/// A player start: feet (metres), engine yaw, team index, game type slots.
#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    pub at: Vec3,
    pub yaw: f32,
    pub team: i16,
    pub game_types: GameTypes,
}

/// A netgame flag: where (metres), engine yaw, its type and team index.
#[derive(Clone, Debug, PartialEq)]
pub struct Flag {
    pub at: Vec3,
    pub yaw: f32,
    pub kind: NetgameFlagType,
    pub team: i16,
}

/// A starting equipment entry: grenade flags, game type slots, up to five
/// collections of (weight, item tag).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Equipment {
    pub flags: u32,
    pub game_types: GameTypes,
    pub collections: Vec<Vec<(f32, String)>>,
}

impl Equipment {
    pub const NO_GRENADES: u32 = 1;
    pub const PLASMA_GRENADES: u32 = 2;
}

/// The map's multiplayer data, in block order (empty on a map without
/// it: the greybox, a party's map).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Netgame {
    pub starts: Vec<Start>,
    pub flags: Vec<Flag>,
    pub equipment: Vec<Equipment>,
    /// The map entities they came from.
    pub seen: usize,
}

impl Netgame {
    /// The flags of `kind`, in block order.
    pub fn flags_of(&self, kind: NetgameFlagType) -> impl Iterator<Item = &Flag> {
        self.flags.iter().filter(move |f| f.kind == kind)
    }

    /// The first capture the flag spot of team index `team`.
    pub fn flag_spot(&self, team: i16) -> Option<&Flag> {
        self.flags_of(NetgameFlagType::CaptureTheFlag).find(|f| f.team == team)
    }

    /// This data kept whatever the map's entities say (a test's own).
    pub fn pinned(mut self) -> Self {
        self.seen = usize::MAX;
        self
    }

    /// The first starting equipment entry eligible for `game`.
    pub fn equipment_for(&self, game: GameType) -> Option<&Equipment> {
        self.equipment.iter().find(|e| e.game_types.eligible_for(game))
    }
}

fn parse_slots(text: Option<&str>) -> GameTypes {
    let mut out = [0i16; 4];
    for (i, n) in text.unwrap_or("").split_whitespace().take(4).enumerate() {
        out[i] = n.parse().unwrap_or(0);
    }
    GameTypes(out)
}

/// A collection's text (`w=tag|w=tag`) as its list.
pub fn parse_items(text: &str) -> Vec<(f32, String)> {
    text.split('|')
        .filter_map(|i| {
            let (w, t) = i.split_once('=')?;
            Some((w.parse().ok()?, t.to_string()))
        })
        .collect()
}

/// The map's data from its entities.
pub fn from_entities(list: &[MapEntity]) -> Netgame {
    let num = |e: &MapEntity, k: &str| e.get(k).and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(0.0);
    let at = |e: &MapEntity| entities::entity_to_engine(e.vector("origin"), 1.0);
    let mut out = Netgame::default();
    for e in list {
        match e.classname() {
            START_CLASS => out.starts.push(Start {
                at: at(e),
                yaw: num(e, "yaw") as f32,
                team: num(e, "team") as i16,
                game_types: parse_slots(e.get("game_types")),
            }),
            FLAG_CLASS => {
                let Some(kind) = NetgameFlagType::from_code(num(e, "type") as i16) else { continue };
                out.flags.push(Flag { at: at(e), yaw: num(e, "yaw") as f32, kind, team: num(e, "team") as i16 });
            }
            EQUIPMENT_CLASS => out.equipment.push(Equipment {
                flags: num(e, "flags") as u32,
                game_types: parse_slots(e.get("game_types")),
                collections: e.get("collections").unwrap_or("").split(';').map(parse_items).collect(),
            }),
            _ => {}
        }
    }
    out
}

/// The resource follows the map's entities (made again when they change).
pub(crate) fn follow_map(entities: Option<Res<MapEntities>>, netgame: Option<ResMut<Netgame>>, mut commands: Commands) {
    let Some(map) = entities else { return };
    let key = std::sync::Arc::as_ptr(&map.entities) as usize;
    if netgame.as_ref().is_some_and(|n| n.seen == key || n.seen == usize::MAX) {
        return;
    }
    let mut n = from_entities(&map.entities);
    n.seen = key;
    commands.insert_resource(n);
}
