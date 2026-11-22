//! The action message's bomb prompts and its life (hud_crosshair.md D9,
//! Corrections 11): after `update` has asked for the pickup prompt, the
//! defuse prompt (`IDS_HUD_BOMBDEFUSE_MSG`: aiming at a bomb target the
//! local player's side defuses, no defuse under way) and the plant prompt
//! (`IDS_HUD_BOMBSET_MSG` with the bomb in hand, else
//! `IDS_HUD_BOMBSELECT_MSG`: while the player may plant and isn't
//! planting); then the message lives 100 OS ms after its last request
//! (`model::ActionMessage`), its text the string table's (`CaUi`).
//!
//! Known shortcuts: "aiming at a bomb target" is the view ray entering a
//! bomb site's box before any wall (our sites are boxes, not objects);
//! Bombing Run's defuse prompt is our reading of "a target owned by the
//! other team": a site the local player's team defends (the other team
//! plants there); fixed messages (mode notices) aren't done, so nothing
//! keeps an ordinary message out.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{ActionView, CaHud, HudMemory, clocks, model::bomb_prompt};
use crate::{
    client::aim,
    core::{Dead, Health, Intent, LocalPlayer, MovementState, SimClock, Team},
    game::rules::{
        CaMatchView, CaRuleSettings, Phase,
        objectives::{self, ActionKind, BOMB, CaAction, CaObjectives},
        objects::{BombZone, CaMapObjects, ObjBox},
    },
    map::interp::RenderedView,
    CaUi,
    weapon::{Inventory, Weapon},
};

/// The plant margin when the rules' settings aren't here (game_modes.md
/// M3: 50 u).
const PLANT_MARGIN: f32 = 50.0;

/// Mode `key`'s bomb site family (S&D's and Go4CA's `Doobic`, Bombing
/// Run's `Sabotage`), and whether only Bravo defuses there.
fn family(key: &str) -> Option<(&'static str, bool)> {
    match key {
        "TMM" | "CMM" | "ESL" => Some(("Doobic", true)),
        "SBT" => Some(("Sabotage", false)),
        _ => None,
    }
}

/// The sites of `family`; every site when the map has none of it (as
/// the rules pick them).
fn sites<'a>(objects: &'a CaMapObjects, family: &str) -> Vec<&'a BombZone> {
    let mine: Vec<_> = objects.bomb_zones.iter().filter(|z| z.family == family).collect();
    if mine.is_empty() { objects.bomb_zones.iter().collect() } else { mine }
}

/// Where a ray from `eye` along `dir` enters `b` (metres), if it does.
fn entry(b: &ObjBox, eye: Vec3, dir: Vec3) -> Option<f32> {
    let (lo, hi) = (b.centre - b.half, b.centre + b.half);
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        if dir[i].abs() < 1e-9 {
            if eye[i] < lo[i] || eye[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - eye[i]) / dir[i], (hi[i] - eye[i]) / dir[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0 <= t1).then_some(t0)
}

type Local<'a> = (
    Entity,
    &'a Transform,
    &'a Intent,
    &'a MovementState,
    Option<&'a RenderedView>,
    Option<&'a Team>,
    Option<&'a Inventory>,
    Option<&'a CaAction>,
    Has<Dead>,
    &'a Health,
);

/// The bomb prompts, then the action message into `CaHud::action`.
#[allow(clippy::too_many_arguments)]
pub(super) fn prompts(
    (time, real, sim): (Res<Time>, Res<Time<Real>>, Option<Res<SimClock>>),
    mut memory: ResMut<HudMemory>,
    mut hud: ResMut<CaHud>,
    ui: Res<CaUi>,
    (view, objects, objectives, settings): (
        Res<CaMatchView>,
        Option<Res<CaMapObjects>>,
        Option<Res<CaObjectives>>,
        Option<Res<CaRuleSettings>>,
    ),
    local: Option<Single<Local, With<LocalPlayer>>>,
    weapons: Query<&Weapon>,
    spatial: SpatialQuery,
) {
    let (ms, _) = clocks(&real, &time, sim.as_deref());
    if let (Some(l), Some(objects), Some((family, bravo_defuses))) = (local.as_deref(), objects.as_deref(), family(&view.mode)) {
        let (me, at, intent, state, rendered, team, inv, action, dead, health) = *l;
        let alive = !dead && health.current > 0.0;
        let team = team.map_or(0, |t| t.0);
        // The plant prompt only while a plant could start: the round
        // counts and is undecided (game_modes.md M3).
        let open = view.phase == Phase::Live && !view.warmup && view.winner.is_none();
        if alive {
            let sites = sites(objects, family);
            // Defuse: the view on a site this player's side defuses,
            // before any wall.
            let ray = aim::view_ray(at, intent, state, rendered);
            let filter = SpatialQueryFilter::from_excluded_entities([me]).with_mask(crate::core::SOLID_LAYERS);
            let wall = spatial
                .cast_ray(ray.eye, ray.dir, super::AIM_RANGE * super::UNIT, true, &filter)
                .map_or(f32::MAX, |h| h.distance);
            let defends = |z: &BombZone| if bravo_defuses { team == 2 } else { z.defusers == team };
            let aimed = sites
                .iter()
                .filter(|z| defends(z))
                .any(|z| entry(&z.area, ray.eye, *ray.dir).is_some_and(|t| t <= wall));
            let defusing = action.is_some_and(|a| a.kind == ActionKind::Defuse && a.held > 0.0);
            // Plant: carrying the bomb inside a site the other team
            // defends (with the plant margin), none planted yet.
            let ids = |w: Entity| weapons.get(w).ok().map(|w| w.id);
            let carries = inv.is_some_and(|i| i.weapons.iter().any(|w| ids(*w) == Some(BOMB)));
            let in_hand = inv.and_then(|i| i.active).and_then(ids) == Some(BOMB);
            let margin = objectives::units(settings.as_deref().map_or(PLANT_MARGIN, |s| s.objectives.plant_margin));
            let planted = objectives.as_deref().is_some_and(|o| o.planted.is_some());
            let may_plant = open
                && carries
                && !planted
                && sites.iter().any(|z| z.defusers != team && z.area.contains(at.translation, margin));
            let planting = action.is_some_and(|a| a.kind == ActionKind::Plant);
            if let Some(id) = bomb_prompt(aimed, defusing, may_plant, planting, in_hand) {
                memory.message.request(id, ms);
            }
        }
    }
    let shown = memory.message.frame(ms).map(|id| ActionView {
        id: id.to_string(),
        text: ui.strings.resolve(id),
    });
    if hud.action != shown {
        hud.action = shown;
    }
}
