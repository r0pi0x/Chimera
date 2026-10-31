//! The character records (data/characters.toml: specs/combat_arms/
//! characters.md sections 1, 2, 3 and 11 as our data) and the model
//! choice: which body record a player's selection draws (1: cosmetics,
//! the face redirect, the default), its skins by camouflage index, the
//! team mark, head pieces, goggles and backpacks by skeleton and figure
//! type, faces
//! and the head piece a face comes with (2). With the V1 key the
//! install's models and attachments files give every head piece's,
//! face's, goggle's and backpack's files and
//! skins (`merge_install_pieces`), and its body records we have none of
//! join ours (its specialists and mercenaries: `rung_bodies` is the
//! wearable player bodies); a client's selection is checked by
//! `CharacterData::validated`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::data::CaDataInner;

/// The model colour every player has (characters.md Corrections
/// (2026-10-30) 1: the colour of the equipped cosmetic character or vest
/// item; we have no cosmetic items, so none).
pub const NO_MODEL_COLOUR: i32 = -1;

/// The install's colour lists as skins (entries that aren't textures are
/// empty slots).
fn colour_lists(lists: &[crate::game::data::attributes::ColourSkins]) -> Vec<crate::game::data::attributes::ColourSkins> {
    let dtx = |l: &[String]| -> Vec<String> {
        l.iter().map(|s| if super::super::data::is_skin_file(s) { s.clone() } else { String::new() }).collect()
    };
    lists
        .iter()
        .map(|c| crate::game::data::attributes::ColourSkins {
            upper: dtx(&c.upper),
            lower: dtx(&c.lower),
            additional: dtx(&c.additional),
            hands: dtx(&c.hands),
        })
        .collect()
}

/// A body record (`Model#`).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ModelRecord {
    pub id: i32,
    pub name: String,
    /// The body file in the install's archives.
    pub file: String,
    /// The hit table and skeleton (`Skeleton`): index into `skeleton`.
    pub skeleton: usize,
    /// The body type (`FigureType`): sounds and stats.
    pub figure: usize,
    pub gear: i32,
    /// `FaceRelationModel#` by face index (-1: keep this record).
    #[serde(default)]
    pub faces: Vec<i32>,
    /// `UpperSkin#` and `LowerSkin#` by camouflage index.
    #[serde(default)]
    pub upper_skins: Vec<String>,
    #[serde(default)]
    pub lower_skins: Vec<String>,
    /// `AdditionalSkins#`: texture slot 5 + k (characters.md 2: "the
    /// record's additional skins"), the same for every camouflage index.
    #[serde(default)]
    pub additional_skins: Vec<String>,
    /// `RenderStyle#`: render-style files by a piece's render-style
    /// index (none given: `CharacterData::body_render_styles`).
    #[serde(default)]
    pub render_styles: Vec<String>,
    /// `FootprintSndVolume`: the share added to the body's footstep volume
    /// (-0.5 for the ghillie-type models; effects_sounds.md Corrections
    /// 2026-10-22, 6). Only the install's models file gives it.
    #[serde(default)]
    pub footprint_volume: f32,
    /// `InfectedOveridable`: its infection override (`CharacterData::
    /// infection` id; -1 none). Only the install's models file gives it.
    #[serde(default = "no_override")]
    pub infected: i32,
    /// The model colour lists by colour m (characters.md Corrections
    /// (2026-10-30) 1; `skins_in`). Only the install's models file gives
    /// them.
    #[serde(default)]
    pub colour_skins: Vec<crate::game::data::attributes::ColourSkins>,
}

fn no_override() -> i32 {
    -1
}

/// An infection override (`Overidable_Model#`, characters.md 2): the
/// face an infected body wears (`FaceIndex`), its render styles and
/// first-person hands (read, not drawn).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct InfectionRecord {
    pub id: i32,
    pub name: String,
    #[serde(default = "no_override")]
    pub face: i32,
    #[serde(default = "no_override")]
    pub hand: i32,
    #[serde(default)]
    pub render_styles: Vec<String>,
}

/// A head piece (`Hair#`): its file per skeleton index and its skins by
/// camouflage index.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeadRecord {
    pub id: i32,
    pub name: String,
    pub files: Vec<String>,
    #[serde(default)]
    pub skins: Vec<String>,
    /// `RenderStyle#` (none: the engine's default style).
    #[serde(default)]
    pub render_styles: Vec<String>,
}

/// A face (`Face#`): its file, its skins and the head piece it comes
/// with (`HairIndex`, the install's).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FaceRecord {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub skins: Vec<String>,
    #[serde(default)]
    pub hair: Option<i32>,
    /// `RenderStyle#` (none: the engine's default style).
    #[serde(default)]
    pub render_styles: Vec<String>,
    /// `ClientFX#`: effect groups worn looping on the character, seen
    /// in third person (game_modes.md Corrections (2026-10-30) 4, 7: the
    /// infected faces' eyes).
    #[serde(default)]
    pub client_fx: Vec<String>,
}

/// A socket attachment a player wears (characters.md 2): goggles or a
/// mask (`Goggle#`) or a backpack (`Backpack#`): its socket, its file
/// per figure type, its skins by camouflage index and render styles.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GoggleRecord {
    pub id: i32,
    #[serde(default)]
    pub name: String,
    pub socket: String,
    pub files: Vec<String>,
    #[serde(default)]
    pub skins: Vec<String>,
    /// `RenderStyle#` (none: the engine's default style).
    #[serde(default)]
    pub render_styles: Vec<String>,
}

impl GoggleRecord {
    /// Its skin for camouflage index `c` (index 0 past the list; None:
    /// no skins).
    pub fn skin(&self, c: i32) -> Option<&str> {
        let i = usize::try_from(c).ok().filter(|&i| i < self.skins.len()).unwrap_or(0);
        self.skins.get(i).map(String::as_str).filter(|s| !s.is_empty())
    }
}

/// The team-mark textures (`[TeamMark]`).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Marks {
    pub enemy: String,
    pub teammate: String,
    /// `RenderStyle#`: mark 0's (the enemy's) and mark 1's styles, drawn
    /// over the mark piece's own (none: the piece's alone).
    #[serde(default)]
    pub render_styles: Vec<String>,
}

/// A weapon's third-person data (`AniType`, `LowerAniType`, `HHSkin#`,
/// `HHAniFrame_#`).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct WeaponLook {
    pub ani_type: i32,
    #[serde(default)]
    pub lower_ani_type: i32,
    #[serde(default)]
    pub skins: Vec<String>,
    /// `HHAniFrame_0..9`, ms (0 or missing: none).
    #[serde(default)]
    pub frames: Vec<u32>,
    /// `HHRenderStyle#` (none: the engine's default style).
    #[serde(default)]
    pub render_styles: Vec<String>,
}

/// Ours: the looks bots draw from (`CharacterData::bot_look`). The spec
/// gives each player's look from their own loadout (characters.md 1);
/// bots in the player-versus-player modes have none, so theirs are drawn
/// from these lists, one entry each.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Loadouts {
    pub models: Vec<i32>,
    pub faces: Vec<i32>,
    pub heads: Vec<i32>,
    pub camouflages: Vec<i32>,
    /// Goggles and backpacks (-1: none).
    #[serde(default)]
    pub goggles: Vec<i32>,
    #[serde(default)]
    pub backpacks: Vec<i32>,
}

/// A skeleton node the game uses, and where the spec measured it at `LST`
/// t = 0 (the fallback pose).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NodeRecord {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub at: Option<[f32; 3]>,
}

/// Where a socket sits: its parent node and offset in that node's space.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SocketPlace {
    pub parent: String,
    pub offset: [f32; 3],
}

/// One hit-table entry (`Node#Name`, `Radius`, `Location`, `DamageFactor`).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SphereRecord {
    pub index: u32,
    pub name: String,
    pub radius: f32,
    /// 1 head, 2 torso, 3 arm, 4 leg, 5 groin.
    pub location: u8,
    pub factor: f32,
    /// On a socket of the stock body, not a node.
    #[serde(default)]
    pub socket: Option<SocketPlace>,
    /// No stock body has it.
    #[serde(default)]
    pub absent: bool,
    #[serde(default)]
    pub bind: Option<[f32; 3]>,
    /// Its centre at `LST` t = 0 (the fallback pose).
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    /// `Node#Priority` (attribute_data.md Corrections 1): which sphere the
    /// server's own walk picks (the highest; a tie, the later node). None:
    /// the skeleton's `hit_priority` for its location.
    #[serde(default)]
    pub priority: Option<f32>,
}

/// A skeleton's hit priority by location, for spheres that give none
/// (the stock values are an open question, weapon_firing.md Corrections
/// 5: ours rank the torso first).
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct HitPriority {
    pub head: f32,
    pub torso: f32,
    pub arm: f32,
    pub leg: f32,
    pub groin: f32,
}

impl HitPriority {
    /// The priority of location code `location` (1 head, 2 torso, 3 arm,
    /// 4 leg, 5 groin; anything else counts as torso).
    pub fn of(&self, location: u8) -> f32 {
        match location {
            1 => self.head,
            3 => self.arm,
            4 => self.leg,
            5 => self.groin,
            _ => self.torso,
        }
    }
}

/// A socket the game uses.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SocketRecord {
    pub name: String,
    pub parent: String,
    pub offset: [f32; 3],
    pub rotation: [f32; 4],
    pub bind: [f32; 3],
    /// Its position in the spec's "stand" pose (the fallback), when given.
    #[serde(default)]
    pub at: Option<[f32; 3]>,
}

/// A skeleton and its hit table (`Skeleton#`).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SkeletonRecord {
    pub name: String,
    /// Section 8's seven aim nodes, in order.
    pub aim: Vec<String>,
    pub nodes: Vec<NodeRecord>,
    pub spheres: Vec<SphereRecord>,
    pub sockets: Vec<SocketRecord>,
    /// The `Left_foot` node in the "stand" pose (the drop point).
    pub left_foot: [f32; 3],
    /// The hit priority of spheres without their own.
    #[serde(default)]
    pub hit_priority: HitPriority,
}

impl SkeletonRecord {
    pub fn socket(&self, name: &str) -> Option<&SocketRecord> {
        self.sockets.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn sphere(&self, name: &str) -> Option<&SphereRecord> {
        self.spheres.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// The whole file.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CharacterData {
    pub model: Vec<ModelRecord>,
    pub head: Vec<HeadRecord>,
    #[serde(default)]
    pub face: Vec<FaceRecord>,
    pub goggle: Vec<GoggleRecord>,
    #[serde(default)]
    pub backpack: Vec<GoggleRecord>,
    pub marks: Marks,
    /// The stock bodies' render styles (characters.md 2), for a record
    /// that names none.
    pub body_render_styles: Vec<String>,
    pub loadouts: Loadouts,
    pub weapons: BTreeMap<String, WeaponLook>,
    /// Seconds a body stays after its owner respawned, by mode key.
    pub body_removal: BTreeMap<String, f32>,
    pub skeleton: Vec<SkeletonRecord>,
    /// The infection overrides (`Overidable_Model#`, the install's).
    #[serde(default)]
    pub infection: Vec<InfectionRecord>,
}

/// The built-in character data without any install, a read-only table
/// for what the install doesn't change (the skeletons, the bot loadout
/// pools, the bodies' removal times); what it does (`weapon`,
/// `effective`, `validated`) comes from the app's `CaData::characters`.
pub fn builtin() -> &'static CharacterData {
    super::super::data::CaData::builtin_static().characters()
}

impl CharacterData {
    /// Combat Arms' character data (data/characters.toml) with `ca`'s
    /// install over it (`CaData::characters` keeps it).
    pub fn load(ca: &CaDataInner) -> Self {
        let mut data: CharacterData =
            toml::from_str(include_str!("../../../data/characters.toml")).expect("data/characters.toml");
        merge_install_weapons(&mut data, ca);
        merge_install_pieces(&mut data, ca);
        for m in &mut data.model {
            if m.render_styles.is_empty() {
                m.render_styles = data.body_render_styles.clone();
            }
        }
        data
    }
}

/// The install's marks file over our team marks (their textures and
/// render styles) and its models file over our bodies, head pieces and
/// faces:
/// each `Model#` of ours by its id (file, skins, render styles, face
/// redirects), each `Hair#` and `Face#` record by its index, its files,
/// skins, render styles and (a face) `HairIndex` and `ClientFX#` (characters.md sections
/// 1 and 2); ours not in it stay.
fn merge_install_pieces(data: &mut CharacterData, ca: &CaDataInner) {
    if let Some(m) = ca.attributes().and_then(|a| a.marks.as_ref()) {
        if let [enemy, teammate, ..] = &m.textures[..] {
            data.marks.enemy = enemy.clone();
            data.marks.teammate = teammate.clone();
        }
        data.marks.render_styles = m.render_styles.clone();
    }
    // The attachments file's goggles and backpacks by record index.
    if let Some(a) = ca.attributes().and_then(|a| a.attachments.as_ref()) {
        for (list, records) in [(&mut data.goggle, &a.goggles), (&mut data.backpack, &a.backpacks)] {
            for (i, r) in records.iter().enumerate() {
                let id = i as i32;
                let rec = GoggleRecord {
                    id,
                    name: r.name.clone(),
                    socket: r.socket.clone(),
                    files: r.files.clone(),
                    skins: super::super::data::skin_paths(&r.skins),
                    render_styles: r.render_styles.clone(),
                };
                match list.iter_mut().find(|g| g.id == id) {
                    Some(g) => *g = rec,
                    None => list.push(rec),
                }
            }
        }
    }
    let Some(models) = ca.attributes().and_then(|a| a.models.as_ref()) else {
        return;
    };
    // Our body records (`Model#` by id): the install's file, skins,
    // render styles and face redirects where it gives them.
    for m in &mut data.model {
        let Some(r) = usize::try_from(m.id).ok().and_then(|i| models.bodies.get(i)) else { continue };
        if !r.file.is_empty() {
            m.file = r.file.clone();
        }
        // As written (texture lookups ignore case and slashes); entries
        // that aren't textures are empty slots.
        let dtx = |l: &[String]| -> Vec<String> {
            l.iter()
                .map(|s| if super::super::data::is_skin_file(s) { s.clone() } else { String::new() })
                .collect()
        };
        if !r.upper_skins.is_empty() {
            m.upper_skins = dtx(&r.upper_skins);
            m.lower_skins = dtx(&r.lower_skins);
        }
        if !r.additional_skins.is_empty() {
            m.additional_skins = dtx(&r.additional_skins);
        }
        if !r.render_styles.is_empty() {
            m.render_styles = r.render_styles.clone();
        }
        if !r.faces.is_empty() {
            m.faces = r.faces.clone();
        }
        m.footprint_volume = r.footprint_volume;
        m.infected = r.infected;
        m.colour_skins = colour_lists(&r.colour_skins);
    }
    // The install's other bodies (named mercenaries and specialists,
    // cosmetics, NPCs: characters.md 1 counts 431 records), each `Model#`
    // we have no record of, by its index: wearable (`model`, `effective`,
    // `validated`) and in the rungs' pool when a player body
    // (`rung_bodies`).
    for (i, r) in models.bodies.iter().enumerate() {
        let id = i as i32;
        if r.file.is_empty() || data.model.iter().any(|m| m.id == id) {
            continue;
        }
        let wear = models.models.get(i);
        let dtx = |l: &[String]| -> Vec<String> {
            l.iter()
                .map(|s| if super::super::data::is_skin_file(s) { s.clone() } else { String::new() })
                .collect()
        };
        data.model.push(ModelRecord {
            id,
            name: wear.map(|w| w.name.clone()).unwrap_or_default(),
            file: r.file.clone(),
            skeleton: usize::try_from(r.skeleton).unwrap_or(0),
            figure: wear.and_then(|w| usize::try_from(w.figure_type).ok()).unwrap_or(0),
            gear: r.gear,
            faces: r.faces.clone(),
            upper_skins: dtx(&r.upper_skins),
            lower_skins: dtx(&r.lower_skins),
            additional_skins: dtx(&r.additional_skins),
            render_styles: r.render_styles.clone(),
            footprint_volume: r.footprint_volume,
            infected: r.infected,
            colour_skins: colour_lists(&r.colour_skins),
        });
    }
    data.infection = models
        .overrides
        .iter()
        .enumerate()
        .map(|(i, o)| InfectionRecord {
            id: i as i32,
            name: o.name.clone(),
            face: o.face,
            hand: o.hand,
            render_styles: o.render_styles.clone(),
        })
        .collect();
    for (i, r) in models.hair_pieces.iter().enumerate() {
        let id = i as i32;
        let head = HeadRecord {
            id,
            name: r.name.clone(),
            files: r.files.clone(),
            skins: super::super::data::skin_paths(&r.skins),
            render_styles: r.render_styles.clone(),
        };
        match data.head.iter_mut().find(|h| h.id == id) {
            Some(h) => *h = head,
            None => data.head.push(head),
        }
    }
    for (i, r) in models.faces.iter().enumerate() {
        let id = i as i32;
        let face = FaceRecord {
            id,
            name: r.name.clone(),
            file: r.files.first().cloned().filter(|f| !f.is_empty()),
            skins: super::super::data::skin_paths(&r.skins),
            hair: r.hair,
            render_styles: r.render_styles.clone(),
            client_fx: r.client_fx.clone(),
        };
        match data.face.iter_mut().find(|f| f.id == id) {
            Some(f) => *f = face,
            None => data.face.push(face),
        }
    }
}

/// The install's weapons file over our weapon looks: each install
/// weapon (ours and the ones it adds, by the id the weapon merge gives
/// it: `data::install_weapons_by_id`) takes its `AniType`, `LowerAniType`,
/// `HHSkin#` and `HHRenderStyle#` (characters.md's "Third-person weapon
/// model and animation class"); `HHAniFrame_#` isn't read (our data has
/// none either).
fn merge_install_weapons(data: &mut CharacterData, ca: &CaDataInner) {
    let Some(file) = ca.attributes().and_then(|a| a.weapons.as_ref()) else {
        return;
    };
    for (id, r) in super::super::data::install_weapons_by_id(file) {
        let look = data.weapons.entry(id).or_default();
        look.ani_type = r.ani_type;
        look.lower_ani_type = r.lower_ani_type;
        if !r.hh_skins.is_empty() {
            look.skins = super::super::data::skin_paths(&r.hh_skins);
        }
        if !r.hh_render_styles.is_empty() {
            look.render_styles = r.hh_render_styles.clone();
        }
    }
}

/// A weapon's class when it has no record: the weapons file's `AniType`
/// default (attribute_data.md).
pub const DEFAULT_ANI_TYPE: i32 = 5;
/// The class with no weapon at all (5): the knife's.
pub const NO_WEAPON_ANI_TYPE: i32 = 14;

/// A player's model selection (1, "The selection"): the record a client
/// sends with its character info; -1 is "none".
#[derive(bevy::prelude::Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub model: i32,
    pub head: i32,
    pub face: i32,
    pub goggle: i32,
    pub backpack: i32,
    pub camouflage: i32,
    /// Cosmetic character and vest ids (-1: none).
    pub cosmetic_character: i32,
    pub cosmetic_vest: i32,
}

impl Default for Selection {
    /// "A fresh selection is model 0, head piece 0, face 0, no goggle, no
    /// backpack, camouflage 0."
    fn default() -> Self {
        Self {
            model: 0,
            head: 0,
            face: 0,
            goggle: -1,
            backpack: -1,
            camouflage: 0,
            cosmetic_character: -1,
            cosmetic_vest: -1,
        }
    }
}

impl CharacterData {
    pub fn model(&self, id: i32) -> Option<&ModelRecord> {
        self.model.iter().find(|m| m.id == id)
    }

    /// The bodies a ladder rung may dress its players in
    /// (`game::rungs`): every player body record (one with upper skins,
    /// the two-texture vest and legs scheme, characters.md 1) on the two
    /// player skeletons (0 male, 1 female), the install's specialists and
    /// mercenaries with it; without the install's models file, ours (the
    /// stock six).
    pub fn rung_bodies(&self) -> Vec<i32> {
        self.model
            .iter()
            .filter(|m| !m.file.is_empty() && m.skeleton <= 1 && m.upper_skins.iter().any(|s| !s.is_empty()))
            .map(|m| m.id)
            .collect()
    }

    /// A body's skin for texture slot `slot` past the five fixed ones
    /// (characters.md 2: 5 on are the record's additional skins,
    /// `AdditionalSkins{slot - 5}`); None past its list or empty.
    pub fn additional_skin<'a>(&self, record: &'a ModelRecord, slot: i32) -> Option<&'a str> {
        let k = usize::try_from(slot.checked_sub(5)?).ok()?;
        let colour = usize::try_from(NO_MODEL_COLOUR).ok().and_then(|m| record.colour_skins.get(m));
        let list = colour.map(|c| &c.additional).filter(|l| !l.is_empty()).unwrap_or(&record.additional_skins);
        list.get(k).map(String::as_str).filter(|s| !s.is_empty())
    }

    /// The player bodies a gun game's infected rung may draw
    /// (`game::rungs`): the rung bodies with an infection override (all
    /// of them without one, our stock six).
    pub fn infected_bodies(&self) -> Vec<i32> {
        let all = self.rung_bodies();
        let with: Vec<i32> = all.iter().copied().filter(|&b| self.infection_of(b).is_some()).collect();
        if with.is_empty() { all } else { with }
    }

    /// Body `body`'s infection override (`InfectedOveridable`).
    pub fn infection_of(&self, body: i32) -> Option<&InfectionRecord> {
        let id = self.model(body)?.infected;
        self.infection.iter().find(|r| r.id == id && id >= 0)
    }

    /// The look body `body` has infected: its override's face (game_modes.md
    /// M8, characters.md 2), else its default face. The override's render
    /// styles and hands aren't drawn (docs/plans/active/combat-arms.md).
    pub fn infected_look(&self, body: i32) -> Selection {
        let face = self
            .infection_of(body)
            .map(|r| r.face)
            .filter(|f| self.face.iter().any(|x| x.id == *f))
            .unwrap_or_else(|| self.default_face(body));
        Selection {
            model: body,
            face,
            ..Selection::default()
        }
    }

    /// Model 0, the default.
    pub fn default_model(&self) -> &ModelRecord {
        self.model(0).unwrap_or(&self.model[0])
    }

    /// `record` after the face redirect (a valid face whose
    /// `FaceRelationModel` is set).
    fn redirect<'a>(&'a self, record: &'a ModelRecord, face: i32) -> &'a ModelRecord {
        usize::try_from(face)
            .ok()
            .and_then(|f| record.faces.get(f))
            .filter(|&&m| m >= 0)
            .and_then(|&m| self.model(m))
            .unwrap_or(record)
    }

    /// "Effective body": the record a selection draws (file, skins,
    /// skeleton) and its figure type (from the redirected base model).
    pub fn effective(&self, s: &Selection) -> (&ModelRecord, usize) {
        let start = if s.cosmetic_character >= 0 {
            s.cosmetic_character
        } else if s.cosmetic_vest >= 0 {
            s.cosmetic_vest
        } else {
            s.model
        };
        let base = self.model(s.model).unwrap_or(self.default_model());
        let own = self.model(start).unwrap_or(base);
        (self.redirect(own, s.face), self.redirect(base, s.face).figure)
    }

    /// The body's upper and leg skins for camouflage index `c` (index 0
    /// past the list), with no model colour (ours: every player's is -1).
    pub fn skins<'a>(&self, record: &'a ModelRecord, c: i32) -> (Option<&'a str>, Option<&'a str>) {
        self.skins_in(record, c, NO_MODEL_COLOUR)
    }

    /// `skins` in model colour `m` (characters.md Corrections (2026-10-30)
    /// 1): each of the colour's upper and lower lists that has an entry
    /// replaces its base list, on its own; -1 or a colour without lists:
    /// the base lists.
    pub fn skins_in<'a>(&self, record: &'a ModelRecord, c: i32, m: i32) -> (Option<&'a str>, Option<&'a str>) {
        let colour = usize::try_from(m).ok().and_then(|m| record.colour_skins.get(m));
        let list = |base: &'a [String], own: Option<&'a Vec<String>>| own.filter(|l| !l.is_empty()).map_or(base, Vec::as_slice);
        let upper = list(&record.upper_skins, colour.map(|c| &c.upper));
        let lower = list(&record.lower_skins, colour.map(|c| &c.lower));
        let pick = |list: &'a [String]| {
            let i = usize::try_from(c).ok().filter(|&i| i < list.len()).unwrap_or(0);
            list.get(i).map(String::as_str)
        };
        (pick(upper), pick(lower))
    }

    /// The team mark on a body's slot 2 as a client sees it: a teammate's
    /// tie, else nothing (the transparent mark); in the infection mode
    /// always the latter.
    pub fn team_mark(&self, teammate: bool, infection: bool) -> &str {
        if teammate && !infection {
            &self.marks.teammate
        } else {
            &self.marks.enemy
        }
    }

    /// A head piece's file for `skeleton` (None: no head model).
    pub fn head_file(&self, head: i32, skeleton: usize) -> Option<&str> {
        let r = self.head.iter().find(|h| h.id == head)?;
        r.files.get(skeleton).map(String::as_str).filter(|f| !f.is_empty())
    }

    /// A head piece's skin for camouflage index `c` (index 0 past the
    /// list; None: no skins).
    pub fn head_skin(&self, head: i32, c: i32) -> Option<&str> {
        let r = self.head.iter().find(|h| h.id == head)?;
        let i = usize::try_from(c).ok().filter(|&i| i < r.skins.len()).unwrap_or(0);
        r.skins.get(i).map(String::as_str).filter(|s| !s.is_empty())
    }

    pub fn face(&self, id: i32) -> Option<&FaceRecord> {
        self.face.iter().find(|f| f.id == id)
    }

    /// The face record a selection wears for its looping groups and
    /// sockets: the face it names, else its body's default.
    pub fn worn_face(&self, look: &Selection) -> Option<&FaceRecord> {
        self.face(look.face).or_else(|| self.face(self.default_face(look.model)))
    }

    /// The face a body record is drawn with by default: the first face
    /// that, on the default selection (model 0), draws this body (the
    /// face redirect: 0 for the male bodies, 4, the first female face,
    /// for the female twin); 0 when none does.
    pub fn default_face(&self, body: i32) -> i32 {
        (0..8)
            .find(|&f| {
                let s = Selection {
                    face: f,
                    ..Selection::default()
                };
                self.effective(&s).0.id == body
            })
            .unwrap_or(0)
    }

    /// The head piece drawn with `head` and `face` on a body: the
    /// selection's head piece when it has a model for `skeleton`, else
    /// the one the face comes with (its `HairIndex`), else `head`.
    pub fn worn_head(&self, head: i32, face: i32, skeleton: usize) -> i32 {
        if self.head_file(head, skeleton).is_some() {
            return head;
        }
        self.face(face).and_then(|f| f.hair).unwrap_or(head)
    }

    /// A bot's look, drawn from the loadout pool (`Loadouts`) by `seed`:
    /// one body, face, head piece and camouflage index (the same seed,
    /// the same look); an empty list keeps the default's.
    pub fn bot_look(&self, seed: u64) -> Selection {
        let mut x = seed;
        let mut next = |list: &[i32], fallback: i32| {
            // splitmix64
            x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            if list.is_empty() { fallback } else { list[(z % list.len() as u64) as usize] }
        };
        let d = Selection::default();
        let l = &self.loadouts;
        Selection {
            model: next(&l.models, d.model),
            face: next(&l.faces, d.face),
            head: next(&l.heads, d.head),
            camouflage: next(&l.camouflages, d.camouflage),
            goggle: next(&l.goggles, d.goggle),
            backpack: next(&l.backpacks, d.backpack),
            ..d
        }
    }

    /// A goggle's socket and file for `figure` (None: none).
    pub fn goggle_file(&self, goggle: i32, figure: usize) -> Option<(&str, &str)> {
        let r = self.goggle.iter().find(|g| g.id == goggle)?;
        let file = r.files.get(figure).map(String::as_str).filter(|f| !f.is_empty())?;
        Some((r.socket.as_str(), file))
    }

    /// A backpack's socket and file for `figure` (None: none).
    pub fn backpack_file(&self, backpack: i32, figure: usize) -> Option<(&str, &str)> {
        let r = self.backpack.iter().find(|g| g.id == backpack)?;
        let file = r.files.get(figure).map(String::as_str).filter(|f| !f.is_empty())?;
        Some((r.socket.as_str(), file))
    }

    /// The server's check of a client's selection (1, "The selection"):
    /// None for an unknown body record (refused); unknown face, head
    /// piece, goggle, backpack or cosmetic ids become none (-1). Known
    /// faces are the face records and the indices the body's face
    /// redirect covers (ours: without the install's models file our data
    /// names only face 0); known head pieces the records and the bots'
    /// pool (likewise).
    pub fn validated(&self, gear: &crate::game::gear::records::GearData, s: Selection) -> Option<Selection> {
        // A vest only the gear data knows (gear_perks.md 3) is a valid
        // choice too: it plays as itself and is drawn as the default body.
        let base = self.model(s.model).or_else(|| gear.vests.contains_key(&s.model).then(|| self.default_model()))?;
        let known = |ok: bool, id: i32| if ok { id } else { -1 };
        let face_ok = self.face(s.face).is_some() || usize::try_from(s.face).is_ok_and(|f| f < base.faces.len());
        let head_ok = self.head.iter().any(|h| h.id == s.head) || self.loadouts.heads.contains(&s.head) || gear.heads.contains_key(&s.head);
        Some(Selection {
            model: s.model,
            face: known(face_ok, s.face),
            head: known(head_ok, s.head),
            goggle: known(self.goggle.iter().any(|g| g.id == s.goggle) || gear.goggles.contains_key(&s.goggle), s.goggle),
            backpack: known(self.backpack.iter().any(|g| g.id == s.backpack) || gear.backpacks.contains_key(&s.backpack), s.backpack),
            camouflage: s.camouflage,
            cosmetic_character: known(self.model(s.cosmetic_character).is_some(), s.cosmetic_character),
            cosmetic_vest: known(self.model(s.cosmetic_vest).is_some(), s.cosmetic_vest),
        })
    }

    /// A weapon's third-person data (the attribute default without one).
    pub fn weapon(&self, id: Option<&str>) -> WeaponLook {
        match id {
            None => WeaponLook {
                ani_type: NO_WEAPON_ANI_TYPE,
                ..Default::default()
            },
            Some(id) => self.weapons.get(id).cloned().unwrap_or(WeaponLook {
                ani_type: DEFAULT_ANI_TYPE,
                ..Default::default()
            }),
        }
    }

    pub fn skeleton(&self, index: usize) -> &SkeletonRecord {
        self.skeleton.get(index).unwrap_or(&self.skeleton[0])
    }

    /// Seconds a body stays after its owner respawned in mode `mode`
    /// (None: until a new round).
    pub fn body_removal(&self, mode: &str) -> Option<f32> {
        self.body_removal.get(mode).copied()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_reads() {
        let d = crate::game::data::CaData::builtin();
        let d = d.characters();
        assert_eq!(d.skeleton.len(), 2);
        for s in &d.skeleton {
            assert_eq!(s.aim.len(), 7);
            for n in &s.nodes {
                if let Some(p) = &n.parent {
                    assert!(s.nodes.iter().any(|m| &m.name == p), "{}: parent {p}", s.name);
                }
            }
            for sp in &s.spheres {
                if let Some(sock) = &sp.socket {
                    assert!(s.nodes.iter().any(|m| m.name == sock.parent), "{}", sp.name);
                }
                if sp.radius > 0.0 && !sp.absent {
                    assert!(sp.at.is_some() && sp.bind.is_some(), "{}", sp.name);
                }
            }
        }
    }
}
