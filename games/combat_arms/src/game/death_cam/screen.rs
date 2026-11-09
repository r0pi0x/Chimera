//! What the death screen shows (specs/combat_arms/death_killcam.md 8):
//! the values for the layout's controls by role, worked out from the kill
//! message, the last damage report, the watched player and the countdown,
//! and the tip choice from the guide data (`Guide`).

use lithtech_formats::attributes::Document;

use crate::core::{DamageKind, Hitgroup};

/// The last damage shown is clamped to this (8).
pub const DAMAGE_MIN: u32 = 1;
pub const DAMAGE_MAX: u32 = 999;

/// The hit-zone icons (the layout's `Observe_Mode` strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Head,
    Body,
    Arms,
    Leg,
    Groin,
}

impl Zone {
    /// The layout string that names the zone's icon.
    pub fn string(self) -> &'static str {
        match self {
            Zone::Head => "DamagePointIconHeadTex",
            Zone::Body => "DamagePointIconBodyTex",
            Zone::Arms => "DamagePointIconArmsTex",
            Zone::Leg => "DamagePointIconLegTex",
            Zone::Groin => "DamagePointIconNutTex",
        }
    }
}

/// The zone icon of a hit location; `groin` for a hit in the groin
/// sphere. Any other location uses the body's.
pub fn zone_of(hitgroup: Hitgroup, groin: bool) -> Zone {
    match hitgroup {
        Hitgroup::Head => Zone::Head,
        Hitgroup::LeftArm | Hitgroup::RightArm => Zone::Arms,
        Hitgroup::LeftLeg | Hitgroup::RightLeg => Zone::Leg,
        Hitgroup::Stomach if groin => Zone::Groin,
        _ => Zone::Body,
    }
}

/// Damage types the figure is doubled for: explosions, claymores,
/// infected explosions, burns and EMP (our blasts and burns).
pub fn doubled(kind: DamageKind) -> bool {
    matches!(kind, DamageKind::Blast | DamageKind::BlastSurface | DamageKind::Burn)
}

/// The last damage as shown: the amount, doubled for the types above,
/// truncated to an integer, then clamped to 1..=999. (The spec words it
/// "truncated, then doubled" but its case 22 has 37.8 give 75, which only
/// doubling first does: 74 the other way. The case is followed.)
pub fn damage_figure(amount: f32, doubled: bool) -> u32 {
    let n = (amount.max(0.0) * if doubled { 2.0 } else { 1.0 }).trunc() as u64;
    n.clamp(DAMAGE_MIN as u64, DAMAGE_MAX as u64) as u32
}

/// A killer's health or armour as three zero-padded digits (⌊value⌋,
/// at least 0).
pub fn three_digits(value: f32) -> String {
    format!("{:03}", value.max(0.0).floor() as u32)
}

/// The killer panel (`KillerInfo`).
#[derive(Clone, Debug, PartialEq)]
pub struct KillerView {
    pub name: String,
    /// `CamOn` (with health and armour) rather than `CamOff`: the enemy
    /// view is on or the killer is a teammate.
    pub cam_on: bool,
    pub hp: String,
    pub ap: String,
}

/// The killing weapon (`KillerWeapon`): its item name and id (the large
/// icon is the layout's art by the id).
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponView {
    pub name: String,
    pub id: String,
}

/// The last hit (`DamageWnd`).
#[derive(Clone, Debug, PartialEq)]
pub struct DamageView {
    pub text: String,
    pub zone: Zone,
}

/// The watched player (`Target`).
#[derive(Clone, Debug, PartialEq)]
pub struct TargetView {
    pub name: String,
    /// How many times this player has killed you in this match, when it
    /// has.
    pub kills_you: Option<u32>,
    /// Hunted: the free cameras' key help.
    pub free_cam_info: bool,
}

/// The tip banner.
#[derive(Clone, Debug, PartialEq)]
pub struct TipView {
    pub icon: String,
    pub text: String,
}

/// The respawn countdown's panel (`Respawn`, `Respawn_EXPass`).
#[derive(Clone, Debug, PartialEq)]
pub struct RespawnView {
    pub gauge: u32,
    pub timer: String,
    /// The bar art: Relentless's `RespawnGauge_Perk`, else `_Normal`.
    pub perk: bool,
    /// The fast-respawn modes: the premium pass's separate panel, or
    /// the free counts (remaining, maximum).
    pub fast: Option<FastView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastView {
    Premium,
    Free { left: u32, max: u32 },
}

/// Everything on the death screen this frame (None: hidden).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeathScreen {
    pub killer: Option<KillerView>,
    pub weapon: Option<WeaponView>,
    pub damage: Option<DamageView>,
    pub target: Option<TargetView>,
    pub banner: Option<TipView>,
    pub respawn: Option<RespawnView>,
}

// ---------------------------------------------------------------------------
// Tips

/// The game-type number of a room key (game_modes.md "Modes found"), as
/// the guide's mode conditions name modes.
pub fn game_type(key: &str) -> Option<i32> {
    Some(match key {
        "TDM" => 1,
        "TMM" => 2,
        "CMM" => 3,
        "CDM" => 4,
        "COOP" | "SCOP" => 5,
        "FFA" => 6,
        "CTF" => 7,
        "CCTF" => 8,
        "BNT" => 9,
        "SFM" => 10,
        "QRT" => 11,
        "TSV" => 12,
        "CTSV" => 13,
        "LMS" => 14,
        "SBT" => 15,
        "HQM" => 17,
        "SQM" => 19,
        "SQM2" => 20,
        "ESL" => 21,
        "VWRM" => 22,
        "ESC" => 23,
        "CESC" => 24,
        "VTDM" => 25,
        "IFS" => 26,
        "COOP_CF" => 27,
        "HG" => 29,
        _ => return None,
    })
}

/// What a tip is tested against at the death (the conditions after the
/// room's). Unknown numbers (the damage and item types, a weapon's
/// number) stay None: a tip that asks for one doesn't pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeathFacts {
    pub damage_type: Option<i32>,
    pub item_type: Option<i32>,
    pub weapon: Option<i32>,
    pub suicide: bool,
    pub infected_mode: bool,
    pub spy_mode: bool,
    /// The player's team is Alpha (for the infected and spy conditions).
    pub alpha: bool,
}

/// The room set-up the first conditions are tested against: the mode's
/// game-type number (None: unknown, a mode condition doesn't pass).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoomFacts {
    pub mode_type: Option<i32>,
}

/// A condition's kind, in the order the data stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionKind {
    UserType,
    Mode,
    Map,
    DamageType,
    ItemType,
    Weapon,
    Suicide,
    Infected,
    Spy,
}

impl ConditionKind {
    fn of(n: i32) -> Option<Self> {
        use ConditionKind::*;
        [UserType, Mode, Map, DamageType, ItemType, Weapon, Suicide, Infected, Spy].get(usize::try_from(n).ok()?).copied()
    }
}

/// A condition: its kind and the numbers it accepts (one `ConditionData<i>`
/// or the list `ConditionData<i>_<j>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    pub kind: ConditionKind,
    pub data: Vec<i32>,
}

/// One record of the guide data: `InGame<i>`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub enable: bool,
    pub order: u32,
    pub icon: String,
    /// String ids (`$IngameGuide0_4`) or texts.
    pub texts: Vec<String>,
    pub conditions: Vec<Condition>,
}

impl Tip {
    /// The room-level conditions hold (the pre-selection at set-up). The
    /// user type and the map aren't known here: those pass.
    pub fn room_ok(&self, room: &RoomFacts) -> bool {
        self.enable
            && self.conditions.iter().all(|c| match c.kind {
                ConditionKind::Mode => room.mode_type.is_some_and(|m| c.data.contains(&m)),
                _ => true,
            })
    }

    /// The death-level conditions hold.
    pub fn death_ok(&self, d: &DeathFacts) -> bool {
        let has = |v: Option<i32>, c: &Condition| v.is_some_and(|v| c.data.contains(&v));
        self.conditions.iter().all(|c| match c.kind {
            ConditionKind::DamageType => has(d.damage_type, c),
            ConditionKind::ItemType => has(d.item_type, c),
            ConditionKind::Weapon => has(d.weapon, c),
            ConditionKind::Suicide => d.suicide == c.data.iter().any(|v| *v != 0),
            ConditionKind::Infected => d.infected_mode && d.alpha == c.data.iter().any(|v| *v != 0),
            ConditionKind::Spy => d.spy_mode && d.alpha == c.data.iter().any(|v| *v != 0),
            _ => true,
        })
    }
}

/// The guide data's tips from its document: the records `InGame0`,
/// `InGame1`, ... with `Enable`, `Order` (255 by default), `DTXPath`, the
/// `Desc<i>` texts and the condition pairs `ConditionType<i>` (the kind's
/// number) with `ConditionData<i>` or the list `ConditionData<i>_<j>`.
pub fn parse_guide(doc: &Document) -> Vec<Tip> {
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
    doc.records("InGame")
        .into_iter()
        .map(|s| {
            let mut conditions = Vec::new();
            for i in 0.. {
                let Some(n) = s.get(&format!("ConditionType{i}")).map(|_| s.int(&format!("ConditionType{i}"), -1)) else { break };
                let mut data: Vec<i32> = (0..)
                    .map_while(|j| s.get(&format!("ConditionData{i}_{j}")).map(|_| s.int(&format!("ConditionData{i}_{j}"), 0)))
                    .collect();
                if data.is_empty() && s.get(&format!("ConditionData{i}")).is_some() {
                    data.push(s.int(&format!("ConditionData{i}"), 0));
                }
                if let Some(kind) = ConditionKind::of(n) {
                    conditions.push(Condition { kind, data });
                }
            }
            Tip {
                enable: match s.get("Enable") {
                    Some(lithtech_formats::attributes::Value::Bool(b)) => *b,
                    Some(lithtech_formats::attributes::Value::Int(i)) => *i != 0,
                    _ => true,
                },
                order: s.int("Order", 255).max(0) as u32,
                icon: s.string("DTXPath").map(text).unwrap_or_default(),
                texts: s.string_list("Desc").iter().map(|b| text(b)).filter(|t| !t.is_empty()).collect(),
                conditions,
            }
        })
        .collect()
}

/// The tip for a death: among the records passing both levels, those with
/// the lowest `Order`; one of them and one of its texts picked by
/// `random` (a uniform draw). None passing: no banner.
pub fn pick_tip(tips: &[Tip], room: &RoomFacts, death: &DeathFacts, random: u64) -> Option<TipView> {
    let passing: Vec<&Tip> = tips
        .iter()
        .filter(|t| t.room_ok(room) && t.death_ok(death) && !t.texts.is_empty())
        .collect();
    let lowest = passing.iter().map(|t| t.order).min()?;
    let best: Vec<&&Tip> = passing.iter().filter(|t| t.order == lowest).collect();
    let tip = best[(random % best.len() as u64) as usize];
    let text = &tip.texts[((random >> 32) % tip.texts.len() as u64) as usize];
    Some(TipView {
        icon: tip.icon.clone(),
        text: text.clone(),
    })
}
