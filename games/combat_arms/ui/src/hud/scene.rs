//! What the radar and the full-screen map know (specs/combat_arms/
//! radar_minimap.md R6, F2, F4, F5): one list of things per frame, in
//! file coordinates, each with the icon the radar gives it and the icon
//! the map gives it (a heartbeat contact is on the radar only; a dead ally
//! is on the map only), read from the world: every player and AI
//! character, the mode's objectives and items (`game::rules::objects`,
//! `objectives`), the map's markers that are on (`game::objects::markers`:
//! their trigger resource records' large-map icons), the death marks
//! (`Died`).
//!
//! Headless (the tests read `CaScene`); `radar` and `full_map` turn it
//! into pixels.
//!
//! Known shortcuts (the spec's most literal reading): AI characters
//! (`core::Npc`) are drawn as co-op NPCs when they have no team or the
//! viewer's (an enemy's are left out: Hired Guns' mercenaries are the only
//! AI on a versus team and the spec has no icon for them); the "goal"
//! icon is every activate trigger the map passes through (Seize and Secure
//! excepted), the mission weapons and targets of co-op aren't there (the
//! modes aren't built); Quarantine's infecting kills still leave a death
//! mark; a player's yaw is its `Intent`'s (the body's turn on a client
//! that has no intent for it: the transform's); Arms Race's last stage
//! is the server's `arms_race::CaLadder`, which a network client isn't
//! sent (no last-stage marks there).

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{
    core::{Dead, Died, Health, Intent, LocalPlayer, MapLoads, Npc, Team},
    game::{
        radar::{
            self, CaRadarRound, CaScanner, CaSeen,
            look::{self, Look, Subject, Viewer},
            maths,
            scan::{self, View},
        },
        rules::{
            CaMatchView, CaStats, arms_race, bombing_run,
            objectives::{CaCarry, CaObjectives, FlagAt},
            objects::CaMapObjects,
            search_destroy,
            escort::CaVip,
        },
    },
};

/// How many death marks the full map keeps (F4).
pub const DEATH_MARKS: usize = 20;

/// The kinds of thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A player or AI character: a wedge shows its facing.
    Character,
    /// An objective or item.
    Item,
}

/// One thing on the radar and the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Thing {
    pub entity: Option<Entity>,
    pub kind: Kind,
    /// File X, Z and the height (Y) in world units.
    pub at: (f64, f64),
    pub y: f64,
    /// The spec's yaw (from north, turning right), for a character's wedge.
    pub yaw: f64,
    /// The radar's icon (None: not on the radar) and the map's.
    pub radar: Option<Look>,
    pub map: Option<Look>,
    /// The player's name, under its icon on the map.
    pub name: Option<String>,
    /// An item gets an edge arrow when out of range (R6's arrows).
    pub arrow: bool,
}

/// The local player, as the radar sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Me {
    pub entity: Entity,
    pub at: (f64, f64),
    pub y: f64,
    pub yaw: f64,
    pub alive: bool,
    pub team: u8,
    pub carry: CaCarry,
    pub name: Option<String>,
}

/// A death mark (F4): where a player was killed by another player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeathMark(pub (f64, f64));

/// What the radar and the map draw from this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaScene {
    pub me: Option<Me>,
    /// In draw order: items and objectives, AI, other players.
    pub things: Vec<Thing>,
    /// The team spawn markers (F5): the icon, the point and the pixels it
    /// is moved down.
    pub spawns: Vec<(Look, (f64, f64), i32)>,
    /// The mode's room key and whether the round is a warm-up.
    pub mode: String,
    pub aspect: f32,
}

/// The death marks of this map (F4): a ring of 20 where the 21st reuses
/// the oldest place.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DeathMarks {
    pub marks: Vec<DeathMark>,
    pub next: usize,
    /// The map load they belong to.
    pub loads: Option<u32>,
}

impl DeathMarks {
    pub fn push(&mut self, m: DeathMark) {
        if self.marks.len() < DEATH_MARKS {
            self.marks.push(m);
        } else {
            self.marks[self.next] = m;
            self.next = (self.next + 1) % DEATH_MARKS;
        }
    }
}

/// Where each dead player was when its kill message arrived (F4, 2026-10-22
/// correction 4): a dead player's mark stays there, it does not follow the
/// body. Dropped when the player is alive again.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DeadSpots(pub Vec<(Entity, (f64, f64))>);

type Chars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Health,
        Option<&'static Team>,
        Option<&'static CaCarry>,
        Option<&'static CaSeen>,
        Option<&'static CaStats>,
        Option<&'static Intent>,
        Option<&'static Name>,
        Has<Dead>,
        Has<Npc>,
        Has<CaVip>,
    ),
>;

#[derive(SystemParam)]
pub struct SceneInputs<'w, 's> {
    chars: Chars<'w, 's>,
    local: Query<'w, 's, Entity, With<LocalPlayer>>,
    scanner: Query<'w, 's, &'static CaScanner, With<LocalPlayer>>,
    view: Option<Res<'w, CaMatchView>>,
    objects: Option<Res<'w, CaMapObjects>>,
    objectives: Option<Res<'w, CaObjectives>>,
    round: Option<Res<'w, CaRadarRound>>,
    screen: Option<Res<'w, super::HudScreen>>,
    /// Arms Race: each player's place on the ladder (the last stage).
    ladders: Query<'w, 's, &'static arms_race::CaLadder>,
    /// The map's `TargetTrigger` markers that are on (their map icons).
    markers: Option<Res<'w, crate::game::objects::markers::CaMarkers>>,
}

/// The spec's yaw of a character: its intent's, else its body's.
fn yaw_of(intent: Option<&Intent>, t: &Transform) -> f64 {
    match intent {
        Some(i) => radar::spec_yaw(i.yaw),
        None => {
            let f = t.rotation * Vec3::NEG_Z;
            (f.x as f64).atan2(-f.z as f64)
        }
    }
}

/// The room keys whose bomb sites are shown, and the sites' family.
fn site_family(mode: &str) -> Option<&'static str> {
    match mode {
        "TMM" | "ESL" | "CMM" => Some(search_destroy::FAMILY),
        "SBT" => Some(bombing_run::FAMILY),
        _ => None,
    }
}

/// Seize and Secure's rooms (the goal icon is left out).
fn seize_mode(mode: &str) -> bool {
    matches!(mode, "HQM" | "SQM2")
}

pub fn update_scene(inputs: SceneInputs, mut scene: ResMut<CaScene>, mut spots: ResMut<DeadSpots>) {
    let mode = inputs.view.as_ref().map(|v| v.mode.clone()).unwrap_or_default();
    let warmup = inputs.view.as_ref().is_some_and(|v| v.warmup);
    let mut next = CaScene {
        mode: mode.clone(),
        aspect: inputs.screen.as_ref().map_or(4.0 / 3.0, |s| s.0.w / s.0.h.max(1.0)),
        ..default()
    };
    // The local player.
    let local = inputs.local.single().ok();
    if let Some(l) = local
        && let Ok((e, t, h, team, carry, _, _, intent, name, dead, ..)) = inputs.chars.get(l)
    {
        next.me = Some(Me {
            entity: e,
            at: radar::file_point(t.translation),
            y: radar::file_height(t.translation),
            yaw: yaw_of(intent, t),
            alive: !dead && h.current > 0.0,
            team: team.map_or(0, |t| t.0),
            carry: carry.copied().unwrap_or_default(),
            name: name.map(|n| n.as_str().to_string()),
        });
    }
    let viewer = next.me.as_ref().map_or(Viewer::default(), |m| Viewer {
        team: m.team,
        intel: m.carry.intel > 0,
    });
    // The local scanner's view, for its own heartbeat contacts (S1).
    let my_scan = inputs.scanner.single().ok().filter(|s| s.on && s.option != scan::SATELLITE).copied();
    let scan_view = local.and_then(|l| inputs.chars.get(l).ok()).map(|(_, t, _, _, _, _, _, intent, ..)| View {
        eye: t.translation,
        yaw: intent.map_or(0.0, |i| i.yaw),
        pitch: intent.map_or(0.0, |i| i.pitch),
        fov_v: scan::FOV_VERTICAL,
        aspect: next.aspect,
    });
    // Arms Race: on the ladder's last step (`arms_race::CaLadder`).
    let arms_last = |e: Entity| mode == "VWRM" && inputs.ladders.get(e).is_ok_and(|l| l.last_stage);

    let mut items: Vec<Thing> = Vec::new();
    let mut npcs: Vec<Thing> = Vec::new();
    let mut players: Vec<Thing> = Vec::new();
    let mut still_dead: Vec<Entity> = Vec::new();
    for (e, t, h, team, carry, seen, _stats, intent, name, dead, npc, vip) in &inputs.chars {
        if Some(e) == local {
            continue;
        }
        let alive = !dead && h.current > 0.0;
        let mut at = radar::file_point(t.translation);
        let y = radar::file_height(t.translation);
        let yaw = yaw_of(intent, t);
        if !alive {
            still_dead.push(e);
            if let Some((_, p)) = spots.0.iter().find(|(s, _)| *s == e) {
                at = *p;
            }
        }
        let team_id = team.map_or(0, |t| t.0);
        let allied = viewer.team != 0 && team_id == viewer.team;
        if npc {
            if (allied || team_id == 0) && alive {
                let l = look::npc(false);
                npcs.push(Thing {
                    entity: Some(e),
                    kind: Kind::Character,
                    at,
                    y,
                    yaw,
                    radar: Some(l.clone()),
                    map: Some(l),
                    name: None,
                    arrow: false,
                });
            }
            continue;
        }
        let seen = seen.copied().unwrap_or_default();
        // A teamless player is an enemy (no team ids: the skeleton).
        let subject = Subject {
            ally: allied,
            alive,
            carry: carry.copied().unwrap_or_default(),
            vip,
            arms_race: mode == "VWRM",
            last_stage: arms_last(e),
            shared_scan: seen.scanned_by(viewer.team),
            contact: match (&my_scan, &scan_view) {
                (Some(s), Some(v)) => alive && scan::contact(v, t.translation, s.scan_dist),
                _ => false,
            },
            team_contact: seen.contact_of(viewer.team),
        };
        let radar_look = look::radar_player(&viewer, &subject);
        let map_look = look::map_player(&viewer, &subject);
        if radar_look.is_none() && map_look.is_none() {
            continue;
        }
        let arrow = radar_look.as_ref().is_some_and(|l| l.arrow);
        players.push(Thing {
            entity: Some(e),
            kind: Kind::Character,
            at,
            y,
            yaw,
            radar: radar_look,
            map: map_look,
            name: name.map(|n| n.as_str().to_string()),
            arrow,
        });
    }

    // The map's markers (world_objects.md Follow-up (2026-10-31b) 1.6):
    // their records' large-map pictures.
    if let Some(markers) = &inputs.markers {
        for m in &markers.0 {
            let Some(icon) = &m.icons.map else { continue };
            let l = Look {
                icon: look::map_icon_key(icon),
                view: None,
                badge: None,
                arrow: false,
            };
            items.push(Thing {
                entity: None,
                kind: Kind::Item,
                at: radar::file_point(m.at),
                y: radar::file_height(m.at),
                yaw: 0.0,
                radar: Some(l.clone()),
                map: Some(l),
                name: None,
                arrow: false,
            });
        }
    }
    // Objectives and items.
    if let (Some(objects), Some(objectives)) = (&inputs.objects, &inputs.objectives) {
        let pt = |p: Vec3| (radar::file_point(p), radar::file_height(p));
        let mut item = |p: Vec3, texture: String, arrow: bool| {
            let (at, y) = pt(p);
            let l = Look {
                icon: texture,
                view: None,
                badge: None,
                arrow,
            };
            items.push(Thing {
                entity: None,
                kind: Kind::Item,
                at,
                y,
                yaw: 0.0,
                radar: Some(l.clone()),
                map: Some(l),
                name: None,
                arrow,
            });
        };
        // Bomb sites: found at the first round that counts.
        if !warmup && let Some(family) = site_family(&mode) {
            let mine: Vec<_> = objects.bomb_zones.iter().filter(|z| z.family == family).collect();
            let zones = if mine.is_empty() { objects.bomb_zones.iter().collect() } else { mine };
            for z in zones {
                item(z.area.centre, look::bomb_site(z.number, mode == "SBT"), true);
            }
        }
        if !seize_mode(&mode) && !mode.is_empty() {
            for t in &objects.triggers {
                item(t.area.centre, look::goal(), true);
            }
        }
        // Capture the flag: each flag zone shows its team's flag.
        if !objectives.flags.is_empty() {
            for z in &objects.flag_zones {
                let home = objectives.flag(z.team).is_none_or(|f| f.at == FlagAt::Home);
                item(z.area.centre, look::flag_base(z.team, home), true);
            }
            for f in &objectives.flags {
                if let FlagAt::Dropped { at, .. } = f.at {
                    item(at, look::loose_flag(f.team), true);
                }
            }
        }
        for p in &objectives.intel {
            item(*p, look::intel(), true);
        }
        if let Some(p) = objectives.loose_bomb {
            item(p, look::loose_bomb(), true);
        }
        for b in &objectives.bullion {
            item(b.at, look::bullion(), false);
        }
    }

    // The team spawn markers (F5): the round's data.
    if let Some(round) = &inputs.round {
        let escort = inputs.objectives.as_ref().map(|o| o.escort).filter(|e| *e != 0);
        for (i, s) in round.spawns.iter().enumerate() {
            let Some(p) = s else { continue };
            let team = i as u8 + 1;
            // Hired Guns: moved down by the icon's height and a gap (37 ref px).
            let down = if mode == "SQM" { 37 } else { 0 };
            next.spawns.push((
                Look {
                    icon: look::respawn(team, escort),
                    view: None,
                    badge: None,
                    arrow: false,
                },
                (p[0] as f64, p[1] as f64),
                down,
            ));
        }
    }

    spots.0.retain(|(e, _)| still_dead.contains(e));
    next.things.extend(items);
    next.things.extend(npcs);
    next.things.extend(players);
    if *scene != next {
        *scene = next;
    }
}

/// The death marks (F4): every kill of a player by another player; cleared
/// when a world is entered.
pub fn track_deaths(
    mut died: MessageReader<Died>,
    victims: Query<(&Transform, Option<&Team>, Has<Npc>), With<Health>>,
    killers: Query<(), (With<Health>, With<Intent>)>,
    loads: Option<Res<MapLoads>>,
    mut marks: ResMut<DeathMarks>,
    mut spots: ResMut<DeadSpots>,
) {
    let l = loads.map_or(0, |l| l.0);
    if marks.loads != Some(l) {
        *marks = DeathMarks {
            loads: Some(l),
            ..default()
        };
    }
    for d in died.read() {
        if let Ok((t, _, false)) = victims.get(d.entity) {
            let at = radar::file_point(t.translation);
            match spots.0.iter_mut().find(|(e, _)| *e == d.entity) {
                Some(s) => s.1 = at,
                None => spots.0.push((d.entity, at)),
            }
        }
        let Some(a) = d.attacker else { continue };
        if a == d.entity || !killers.contains(a) {
            continue;
        }
        let Ok((t, _, npc)) = victims.get(d.entity) else { continue };
        if npc {
            continue;
        }
        marks.push(DeathMark(radar::file_point(t.translation)));
    }
}

/// The maths' frame for the local player at the radar's zoom.
pub fn frame(me: &Me, zoom: f32, rotate: bool) -> maths::Frame {
    maths::Frame {
        x: me.at.0,
        z: me.at.1,
        y: me.y,
        yaw: me.yaw,
        zoom: zoom as f64,
        rotate,
    }
}
