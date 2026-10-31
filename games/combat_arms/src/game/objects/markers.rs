//! `TargetTrigger` markers on the client (specs/combat_arms/
//! world_objects.md Follow-up (2026-10-31b) 1.6): their icons come from
//! the install's trigger resource table (`ATTRIBUTES\TRIGGERRESOURCE.TXT`,
//! records `Trigger0`, `Trigger1`, ...), the first record whose `Name` is
//! the trigger's name exactly (case counts), not from the trigger's own
//! texture properties. A viewer is an ally when their team is the
//! marker's; with the record's `CheckTeam` an ally takes the `Ally...`
//! picture and anyone else the `Enemy...` one (each when non-empty),
//! otherwise the plain one. The `...LargeMapText` picture is the radar
//! and large-map icon, the `...GuideTex` one the on-screen guide marker,
//! which shows the distance as whole metres (⌊d / 40⌋ `M`) with
//! `ShowDist`, fully opaque or at alpha 125 when its line is blocked.
//!
//! `CaMarkers` is what a client shows: each marker that is on and exists
//! in the mode playing (`events::Marker::exists_in`, the server sends
//! only those: `net`), with its record's icons for the local player.
//!
//! Known shortcuts: the line test that dims a guide marker isn't in the
//! material (`Guide::blocked` is the caller's); the guide markers aren't
//! hidden during videos or while players are frozen (no videos here).

use bevy::prelude::*;
use lithtech_formats::attributes::Document;

use super::events::MarkerTeam;

/// The trigger resource table in the install's attribute archive.
pub const FILE: &str = "attributes/triggerresource.txt";
/// A guide marker's alpha when its line is blocked (of 255).
pub const BLOCKED_ALPHA: u8 = 125;
/// World units per metre in the guide's distance.
pub const UNITS_PER_METRE: f32 = 40.0;

/// One record of the trigger resource table (the target-marker keys).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriggerResource {
    pub name: String,
    pub check_team: bool,
    pub show_dist: bool,
    pub guide: String,
    pub map: String,
    pub ally_guide: String,
    pub ally_map: String,
    pub enemy_guide: String,
    pub enemy_map: String,
}

/// The table (empty without the install).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TriggerResources(pub Vec<TriggerResource>);

impl TriggerResources {
    pub fn parse(doc: &Document) -> Self {
        let text = |s: &lithtech_formats::attributes::Section, k: &str| {
            s.string(k).map(|b| String::from_utf8_lossy(b).trim().to_string()).unwrap_or_default()
        };
        Self(
            doc.records("Trigger")
                .into_iter()
                .map(|s| TriggerResource {
                    name: text(s, "Name"),
                    check_team: s.int("CheckTeam", 0) != 0,
                    show_dist: s.int("ShowDist", 0) != 0,
                    guide: text(s, "TargetGuideTex"),
                    map: text(s, "TargetLargeMapText"),
                    ally_guide: text(s, "AllyTargetGuideTex"),
                    ally_map: text(s, "AllyTargetLargeMapText"),
                    enemy_guide: text(s, "EnemyTargetGuideTex"),
                    enemy_map: text(s, "EnemyTargetLargeMapText"),
                })
                .collect(),
        )
    }

    /// The install's table (empty without it).
    pub fn install() -> Self {
        let Some(mount) = super::super::INFO.mount() else { return Self::default() };
        let Ok(bytes) = mount.read(FILE) else { return Self::default() };
        match Document::parse(&bytes) {
            Ok(doc) => Self::parse(&doc),
            Err(e) => {
                warn!("combat_arms markers: {FILE}: {e}; no marker icons");
                Self::default()
            }
        }
    }

    /// The first record named exactly `name`.
    pub fn find(&self, name: &str) -> Option<&TriggerResource> {
        self.0.iter().find(|r| r.name == name)
    }
}

/// What a viewer sees of a marker: its guide picture and its map icon
/// (None: nothing), and whether the guide shows the distance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Icons {
    pub guide: Option<String>,
    pub map: Option<String>,
    pub show_dist: bool,
}

/// Whether a viewer of team `viewer` (`rules::objects::team_named`'s ids)
/// is an ally of a marker of `team`.
pub fn ally(team: &MarkerTeam, viewer: Option<u8>) -> bool {
    match team {
        MarkerTeam::All => true,
        MarkerTeam::Nobody => false,
        MarkerTeam::Team(t) => viewer.is_some() && super::super::rules::objects::team_named(t) == viewer,
    }
}

/// The icons record `r` gives a viewer of team `viewer` for a marker of
/// `team`.
pub fn icons(r: &TriggerResource, team: &MarkerTeam, viewer: Option<u8>) -> Icons {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    let (guide, map) = if r.check_team {
        if ally(team, viewer) {
            (some(&r.ally_guide).or_else(|| some(&r.guide)), some(&r.ally_map).or_else(|| some(&r.map)))
        } else {
            (some(&r.enemy_guide).or_else(|| some(&r.guide)), some(&r.enemy_map).or_else(|| some(&r.map)))
        }
    } else {
        (some(&r.guide), some(&r.map))
    };
    Icons {
        guide,
        map,
        show_dist: r.show_dist,
    }
}

/// The guide's distance text for `units` from the local player.
pub fn distance_text(units: f32) -> String {
    format!("{} M", (units / UNITS_PER_METRE).floor().max(0.0) as i64)
}

/// A guide marker's alpha (of 255).
pub fn guide_alpha(blocked: bool) -> u8 {
    if blocked { BLOCKED_ALPHA } else { 255 }
}

/// One marker a client shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub id: u32,
    pub name: String,
    /// Where (the engine's axes and metres).
    pub at: Vec3,
    pub icons: Icons,
}

/// The markers a client shows now, for its local player.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaMarkers(pub Vec<Shown>);

/// Each frame: the markers to show for the local player (`CaMarkers`).
/// On a network client the server's word decides which are on (it sends
/// only those of the mode playing); elsewhere the marker's own state and
/// the mode playing.
#[allow(clippy::too_many_arguments)]
pub fn show(
    role: Option<Res<crate::core::NetRole>>,
    settings: Option<Res<super::super::rules::CaRuleSettings>>,
    local: Query<&crate::core::Team, With<crate::core::LocalPlayer>>,
    table: Option<Res<TriggerResources>>,
    objects: Res<super::CaObjects>,
    mut markers: ResMut<CaMarkers>,
    mut commands: Commands,
) {
    let Some(table) = table else {
        commands.insert_resource(TriggerResources::install());
        return;
    };
    let client = role.as_deref() == Some(&crate::core::NetRole::Client);
    let family = settings
        .as_ref()
        .and_then(|s| super::super::rules::mode_by_key(&s.mode))
        .and_then(|m| m.spawns.family);
    let viewer = local.iter().next().map(|t| t.0);
    let next = shown(&objects, &table, client, family, viewer);
    if *markers != next {
        *markers = next;
    }
}

/// The markers shown to a viewer of team `viewer`: those on (and, unless
/// `client`, existing in mode family `family`) with a record in `table`.
pub fn shown(o: &super::CaObjects, table: &TriggerResources, client: bool, family: Option<&str>, viewer: Option<u8>) -> CaMarkers {
    let mut shown = Vec::new();
    for (i, ob) in o.world.objects.iter().enumerate() {
        let super::runtime::Kind::Trigger(t) = &ob.kind else { continue };
        let Some(m) = &t.marker else { continue };
        if ob.removed || !m.on || !(client || m.exists_in(family)) {
            continue;
        }
        let Some(r) = table.find(&ob.name) else { continue };
        shown.push(Shown {
            id: i as u32,
            name: ob.name.clone(),
            at: super::geometry::to_engine(ob.pose.pos, o.scale),
            icons: icons(r, &m.team, viewer),
        });
    }
    CaMarkers(shown)
}
