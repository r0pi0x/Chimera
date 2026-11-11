//! Each install weapon's own fire sequence, reload and draw from its own
//! view model's animations (docs/plans/active/ca-weapon-timings.md;
//! weapon_firing.md "Trigger and fire cycle" and "Reload",
//! weapon_classes.md "Typical weapons", 3.3, 4 and 5.5, view_models.md 7,
//! inventory.md 8.2, special_weapons.md 4): the cycle is `fire_0`'s
//! length, the magazine reload `reload`'s, a shell reload `PreReload`,
//! `reload` per shell and `PostReload`, the draw `select`'s, the trigger's
//! closing part `postfire`'s, all at rate 1, each found by name in the
//! model's combined list (its own animations, then its children's:
//! `model::load`). Also from the model:
//!
//! - the pre-fire: a gun whose class plays one (the template's
//!   `trigger.latency` 3: not pistols, knives or primary shotguns) and
//!   whose model has no `prefire` takes the first round an update sooner
//!   (latency 2; weapon_firing.md, "Trigger and fire cycle"); one with a
//!   `prefire` waits for its whole length (`prefire_latency`, at the
//!   simulation's tick when the weapon is built: weapon_classes.md
//!   Corrections (2026-10-30) 1);
//! - a spinning gun's (`CaSpin`, the merge's `combat_arms:spin` table
//!   where `AccumPreFire` is set): the spin-up `prefire`, the spin-down
//!   `postfire`, the loop `fire_0`, a round per `FIRE_KEY` before the
//!   loop's end (its cycle the loop over their count), latency 1, no
//!   trigger post-fire (the spin-down is it);
//! - a flame gun's (`special::CaFlame`): the pre-fire, the loop, its
//!   `FIRE_KEY`s (their count and mean spacing, the trigger's cycle) and
//!   the post-fire;
//! - a knife's (`inventory::Knife`): each swing's length (`fire_0`,
//!   `alt_fire_0`), hits (its `FIRE_KEY`s, `ALT_FIRE_KEY`s) and post-fire,
//!   at 1 + the record's `FireAniRate`.
//!
//! The `combat_arms:timing` hook (written by `data`'s merge on the
//! install's copies, never on our documents: their numbers are the
//! spec's) reads the weapon's `[models] view` when the weapon is built,
//! on whichever side builds it, once per model file per world
//! (`ViewTimes`, failures too). A
//! model that doesn't read, or a length it lacks, keeps the template's
//! number (a `postfire` it lacks is none). The rates on top are the other
//! modules': the cycle and post-fire are divided by
//! `attachments::fire_animation_rate` (the record's `FireAniRate`,
//! `CaItem::fire_rate`, + the parts' and the gear's), the draw by the
//! item's select rate; with an attachments table the lengths become its
//! `Stock` (the parts' and gear's rates applied there). The hook runs
//! after the attachments, damage, inventory, knife, flame and spin hooks
//! (hooks build in name order), so it rewrites what they took from the
//! template: the stock numbers and the server's reload window
//! (`CaDamage::reload_third_person`, when it was the template's reload).
//!
//! The server's numbers win (docs/plans/active/ca-weapon-timings.md, open
//! question 1): the hook leaves what it settled on the weapon
//! (`CaTimingValues`: the model's lengths where it read, the template's
//! where not), which the server sends a client once per weapon id
//! (`weapon::params`, codec `TIMING`, chimera_net's `net::weapon_params`);
//! the client's weapons of that id take them over their own (`push`),
//! so a client that can't read the model (no install, no key for its
//! archive) or reads another version predicts on the server's timing.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The first build of a weapon outside the match's ladder reads its view
//!   model on the frame it is built (up to about 100 M instructions, 30 ms: an
//!   LTB out of a keyed archive); a ladder's are read ahead (`read_ahead`).
//! - A client still reads the model on the first build of a weapon the
//!   server has sent the timing for (then takes the server's): one wasted
//!   read, or one warning when it can't.
//! - A non-spinning gun's pre-fire of non-zero length counts as present
//!   only (one update), not by its length (the plan's open questions); the
//!   spin's and the flame's lengths play at rate 1; a knife copy keeps its
//!   template's pre-fire; grenades' timings are their own hook's (the merge
//!   writes no timing table for them).

use std::collections::BTreeMap;

use bevy::prelude::*;
use lithtech_formats::anim::AnimList;
use serde::{Deserialize, Serialize};

use super::{
    super::{
        attachments::{CaAttachSlots, animation_rate, fire_animation_rate},
        inventory::{CaItem, Knife, played_rate},
        model,
        special::{CaFlame, FlameParams},
    },
    CaDamage,
    classes::CaSpin,
};
use super::super::attachments::{CaFitted, CaMods};
use crate::weapon::{Magazine, ShellReload, Trigger, Weapon, data::WeaponData};

/// The hook's name: after `combat_arms:attachments`, `:damage` and
/// `:inventory` in name order, so their components are there.
pub const TIMING: &str = "combat_arms:timing";

/// The hook's parameters: none (the model is the document's `[models]
/// view`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingParams {}

/// A view model's action lengths at rate 1, s (None: the model has no
/// such animation, or it is 0 ms long).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimTimes {
    pub fire: Option<f32>,
    pub reload: Option<f32>,
    pub pre_reload: Option<f32>,
    pub post_reload: Option<f32>,
    pub select: Option<f32>,
    /// Whether the model has a `prefire` (a 0 ms one counts), and its
    /// length.
    pub has_prefire: bool,
    pub prefire: Option<f32>,
    pub postfire: Option<f32>,
    /// `alt_fire_0` (a knife's stab).
    pub alt_fire: Option<f32>,
    /// The `FIRE_KEY` times in `fire_0` and the `ALT_FIRE_KEY` times in
    /// `alt_fire_0`, s.
    pub fire_keys: Vec<f32>,
    pub alt_fire_keys: Vec<f32>,
}

impl AnimTimes {
    /// The lengths in a model's combined animation list (first match by
    /// name, case ignored: view_models.md 7).
    pub fn of_list(list: &AnimList) -> AnimTimes {
        let len = |name: &str| {
            list.find(name)
                .map(|i| list.entries[i].length())
                .filter(|&ms| ms > 0)
                .map(|ms| ms as f32 / 1000.0)
        };
        let keys = |name: &str, word: &str| -> Vec<f32> {
            let Some(i) = list.find(name) else { return Vec::new() };
            list.entries[i]
                .animation
                .keys
                .iter()
                .filter(|k| {
                    lithtech_formats::anim::key_commands(&k.string)
                        .iter()
                        .any(|c| c.first().is_some_and(|w| w.eq_ignore_ascii_case(word)))
                })
                .map(|k| k.time as f32 / 1000.0)
                .collect()
        };
        AnimTimes {
            fire: len("fire_0"),
            reload: len("reload"),
            pre_reload: len("PreReload"),
            post_reload: len("PostReload"),
            select: len("select"),
            has_prefire: list.find("prefire").is_some(),
            prefire: len("prefire"),
            postfire: len("postfire"),
            alt_fire: len("alt_fire_0"),
            fire_keys: keys("fire_0", "FIRE_KEY"),
            alt_fire_keys: keys("alt_fire_0", "ALT_FIRE_KEY"),
        }
    }

    /// The fire keys before `fire_0`'s end (a loop's last key, at its
    /// end, is the next play's first: weapon_classes.md 3.3, the lower
    /// count).
    fn loop_keys(&self) -> Vec<f32> {
        let end = self.fire.unwrap_or(0.0);
        self.fire_keys.iter().copied().filter(|&k| k < end - 1e-6).collect()
    }
}

/// The view models' lengths this world has read, by archive path
/// (normalized; None: it didn't read). The weapons plugin inserts it;
/// tests set a model's lengths in it (`set`).
#[derive(Resource, Clone, Debug, Default)]
pub struct ViewTimes(pub BTreeMap<String, Option<AnimTimes>>);

impl ViewTimes {
    /// Model `path`'s lengths, as if read.
    pub fn set(&mut self, path: &str, times: AnimTimes) {
        self.0.insert(model::normalize(path), Some(times));
    }
}

/// View model `path`'s lengths, read from the install (`world::mount`);
/// None when it doesn't read.
pub fn read(path: &str) -> Option<AnimTimes> {
    let key = model::normalize(path);
    let read = super::super::world::mount()
        .ok_or_else(|| "no Combat Arms install".to_string())
        .and_then(|m| model::load(&key, &|p: &str| m.read(p).ok()));
    match read {
        Ok(l) => Some(AnimTimes::of_list(&l.list)),
        Err(e) => {
            warn!("Combat Arms: {key}: {e} (the template's timing stands)");
            None
        }
    }
}

/// A match's ladder's view models' lengths, read on tasks as soon as the
/// ladder is known (`characters::preload`; a front end's): `apply` runs when
/// a rung gives the weapon, in the middle of a tick, and a read there stops the
/// frame (docs/performance.md, "Nothing blocks the main thread").
pub fn read_ahead(
    ladder: Res<crate::game::characters::preload::LadderModels>,
    catalog: Option<Res<crate::weapon::data::WeaponCatalog>>,
    mut times: ResMut<ViewTimes>,
    mut pending: Local<BTreeMap<String, bevy::tasks::Task<Option<AnimTimes>>>>,
    wait: Option<Res<chimera_core::tasks::WaitForTasks>>,
) {
    pending.retain(|path, task| {
        let Some(read) = chimera_core::tasks::poll(task, wait.is_some()) else { return true };
        times.0.insert(path.clone(), read);
        false
    });
    if !ladder.is_changed() {
        return;
    }
    let Some(catalog) = catalog else { return };
    for key in &ladder.keys {
        // A key with parts is the weapon's own model's lengths too.
        let (id, _) = crate::game::view_model::parts::split_key(key);
        let Some(path) = catalog.get(id).and_then(|d| d.models.view.as_deref()).map(model::normalize) else { continue };
        if times.0.contains_key(&path) || pending.contains_key(&path) {
            continue;
        }
        let at = path.clone();
        pending.insert(path, bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { read(&at) }));
    }
}

/// The hook: weapon `d`'s parts take its view model's lengths.
pub fn apply(e: &mut EntityWorldMut, d: &'static WeaponData, _: &TimingParams) {
    let Some(path) = d.models.view.as_deref().map(model::normalize) else { return };
    let cached = e.get_resource_mut::<ViewTimes>().and_then(|c| c.0.get(&path).cloned());
    let times = cached.unwrap_or_else(|| {
        let t = read(&path);
        if let Some(mut c) = e.get_resource_mut::<ViewTimes>() {
            c.0.insert(path, t.clone());
        }
        t
    });
    if let Some(t) = times {
        apply_times(e, &t);
    }
    let settled = CaTimingValues::of(e);
    e.insert(settled);
}

/// The timing the hook settled on a weapon (its model's lengths where it
/// read, the template's where not): what the server sends a client
/// (`encode`, `push`). The cycle, reload and draw are the parts' values
/// before the attachments' rates (`stock`: the rate-1 lengths those
/// apply to).
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaTimingValues {
    pub cycle: Option<f32>,
    pub reload: Option<f32>,
    /// `ShellReload`'s start, insert, finish.
    pub shell: Option<(f32, f32, f32)>,
    pub draw: Option<f32>,
    /// `CaAttachSlots::stock`'s cycle, reload, draw, post-fire.
    pub stock: Option<(f32, f32, f32, f32)>,
    pub reload_third_person: Option<f32>,
    /// `Trigger::postfire` and `latency`.
    pub postfire: Option<f32>,
    pub latency: Option<u32>,
    /// `CaSpin`'s spin-up, spin-down, loop.
    pub spin: Option<(f32, f32, f32)>,
    /// `CaFlame`'s pre-fire, loop, keys, interval, post-fire.
    pub flame: Option<(f32, f32, u32, f32, f32)>,
    /// `Knife`'s played rate and each attack's pre-fire, length, hits and
    /// post-fire.
    pub knife: Option<(f32, Vec<(f32, f32, Vec<f32>, f32)>)>,
}

impl CaTimingValues {
    fn of(e: &EntityWorldMut) -> Self {
        CaTimingValues {
            cycle: e.get::<Trigger>().map(|t| t.cycle),
            reload: e.get::<Magazine>().map(|m| m.reload_time),
            shell: e.get::<ShellReload>().map(|s| (s.start, s.insert, s.finish)),
            draw: e.get::<Weapon>().map(|w| w.draw_time),
            stock: e
                .get::<CaAttachSlots>()
                .map(|s| (s.stock.cycle, s.stock.reload, s.stock.draw, s.stock.postfire)),
            reload_third_person: e.get::<CaDamage>().map(|d| d.reload_third_person),
            postfire: e.get::<Trigger>().map(|t| t.postfire),
            latency: e.get::<Trigger>().map(|t| t.latency),
            spin: e.get::<CaSpin>().map(|s| (s.params.prefire, s.params.postfire, s.params.loop_len)),
            flame: e.get::<CaFlame>().map(|f| {
                let p = f.params;
                (p.prefire, p.loop_len, p.keys, p.interval, p.postfire)
            }),
            knife: e.get::<Knife>().map(|k| {
                let attacks = k.attacks.iter().map(|a| (a.prefire, a.length, a.hits.clone(), a.postfire)).collect();
                (k.rate, attacks)
            }),
        }
    }
}

/// The server: weapon `e`'s settled timing (`weapon::params` codec).
pub fn encode(world: &World, e: Entity) -> Option<Vec<u8>> {
    world.get::<CaTimingValues>(e).and_then(|v| postcard::to_allocvec(v).ok())
}

/// A client: the server's timing over weapon `e`'s own; the attachments'
/// rates are applied again from the new stock (`attachments::apply_fitted`,
/// `apply_draw` run on the change).
pub fn push(world: &mut World, e: Entity, bytes: &[u8]) {
    let Ok(v) = postcard::from_bytes::<CaTimingValues>(bytes) else {
        warn!("Combat Arms: the server's weapon timing didn't read");
        return;
    };
    let Ok(mut e) = world.get_entity_mut(e) else { return };
    if let (Some(c), Some(mut t)) = (v.cycle, e.get_mut::<Trigger>()) {
        t.cycle = c;
    }
    if let (Some(r), Some(mut m)) = (v.reload, e.get_mut::<Magazine>()) {
        m.reload_time = r;
    }
    if let (Some((start, insert, finish)), Some(mut s)) = (v.shell, e.get_mut::<ShellReload>()) {
        (s.start, s.insert, s.finish) = (start, insert, finish);
    }
    if let (Some(shell), Some(mut s)) = (v.shell, e.get_mut::<CaAttachSlots>()) {
        s.stock.shell = Some(shell);
    }
    if let (Some(d), Some(mut w)) = (v.draw, e.get_mut::<Weapon>()) {
        w.draw_time = d;
    }
    if let (Some((cycle, reload, draw, postfire)), Some(mut s)) = (v.stock, e.get_mut::<CaAttachSlots>()) {
        (s.stock.cycle, s.stock.reload, s.stock.draw, s.stock.postfire) = (cycle, reload, draw, postfire);
    }
    if let Some(mut t) = e.get_mut::<Trigger>() {
        if let Some(p) = v.postfire {
            t.postfire = p;
        }
        if let Some(l) = v.latency {
            t.latency = l;
        }
    }
    if let (Some((prefire, postfire, loop_len)), Some(mut s)) = (v.spin, e.get_mut::<CaSpin>()) {
        (s.params.prefire, s.params.postfire, s.params.loop_len) = (prefire, postfire, loop_len);
    }
    if let (Some((prefire, loop_len, keys, interval, postfire)), Some(mut f)) = (v.flame, e.get_mut::<CaFlame>()) {
        f.params = FlameParams {
            prefire,
            loop_len,
            keys,
            interval,
            postfire,
        };
    }
    if let (Some((rate, attacks)), Some(mut k)) = (v.knife.clone(), e.get_mut::<Knife>()) {
        k.rate = rate;
        for (a, (prefire, length, hits, postfire)) in k.attacks.iter_mut().zip(attacks) {
            (a.prefire, a.length, a.hits, a.postfire) = (prefire, length, hits, postfire);
        }
    }
    if let (Some(r), Some(mut d)) = (v.reload_third_person, e.get_mut::<CaDamage>()) {
        d.reload_third_person = r;
    }
    if let Some(mut f) = e.get_mut::<CaFitted>() {
        f.set_changed();
    }
    if let Some(mut m) = e.get_mut::<CaMods>() {
        m.set_changed();
    }
    e.insert(v);
}

/// Parts take lengths `t` (rate 1; the rates applied as the module doc
/// says).
pub fn apply_times(e: &mut EntityWorldMut, t: &AnimTimes) {
    let (fire_rate, select_rate) = e.get::<CaItem>().map_or((0.0, 0.0), |i| (i.fire_rate, i.select_rate));
    let rate = fire_animation_rate(fire_rate, 0.0);
    // The weapon update's length (the simulation's tick).
    let tick = e
        .world()
        .get_resource::<Time<Fixed>>()
        .map_or(crate::game::TICK_INTERVAL, |t| t.timestep().as_secs_f64());
    if e.contains::<Knife>() {
        knife_times(e, t, fire_rate);
    }
    // The cycle at rate 1: `fire_0`; a spinning gun's a round per fire
    // key of its loop, a flame gun's its keys' spacing.
    let spin = e.contains::<CaSpin>();
    let flame = flame_times(e, t);
    let cycle = match (spin, flame) {
        (true, _) => t.fire.map(|f| f / t.loop_keys().len().max(1) as f32),
        (false, Some(interval)) => Some(interval),
        (false, None) => t.fire,
    };
    if spin && let Some(mut s) = e.get_mut::<CaSpin>() {
        s.params.prefire = t.prefire.unwrap_or(0.0);
        s.params.postfire = t.postfire.unwrap_or(0.0);
        if let Some(f) = t.fire {
            s.params.loop_len = f;
        }
    }
    // The trigger's closing part: a spinning gun's is its spin-down, a
    // flame gun's its own lock.
    let postfire = if spin || flame.is_some() { 0.0 } else { t.postfire.unwrap_or(0.0) };
    let old_reload = e.get::<Magazine>().map(|m| m.reload_time);
    if let Some(mut s) = e.get_mut::<CaAttachSlots>() {
        if let Some(c) = cycle {
            s.stock.cycle = c;
        }
        if let Some(r) = t.reload {
            s.stock.reload = r;
        }
        if let Some(sel) = t.select {
            s.stock.draw = sel;
        }
        s.stock.postfire = postfire;
    }
    if let Some(mut tr) = e.get_mut::<Trigger>() {
        if let Some(c) = cycle {
            tr.cycle = c / rate;
        }
        tr.postfire = postfire / rate;
        tr.latency = match tr.latency {
            _ if spin => 1,
            // A class that plays the pre-fire (3: the update that sees the
            // press, the pre-fire's, `fire_0`'s), without one in the model.
            3 if !t.has_prefire => 2,
            // With one: its whole length (weapon_classes.md Corrections
            // (2026-10-30) 1).
            3 => prefire_latency(t.prefire.unwrap_or(0.0) / rate, tick),
            l => l,
        };
    }
    if let (Some(r), Some(mut m)) = (t.reload, e.get_mut::<Magazine>()) {
        m.reload_time = r;
    }
    let shell = e.get_mut::<ShellReload>().map(|mut s| {
        if let Some(p) = t.pre_reload {
            s.start = p;
        }
        if let Some(r) = t.reload {
            s.insert = r;
        }
        if let Some(p) = t.post_reload {
            s.finish = p;
        }
        (s.start, s.insert, s.finish)
    });
    if let (Some(shell), Some(mut s)) = (shell, e.get_mut::<CaAttachSlots>()) {
        s.stock.shell = Some(shell);
    }
    if let (Some(sel), Some(mut w)) = (t.select, e.get_mut::<Weapon>()) {
        w.draw_time = sel / animation_rate(1.0 + select_rate);
    }
    if let (Some(sel), Some(mut d)) = (t.select, e.get_mut::<crate::game::inventory::DrawStock>()) {
        d.0 = sel;
    }
    // The server's reload window, when it was the template's reload.
    if let (Some(r), Some(old), Some(mut dmg)) = (t.reload, old_reload, e.get_mut::<CaDamage>())
        && dmg.reload_third_person == old
    {
        dmg.reload_third_person = r;
    }
}

/// The updates from the press to the first round of a weapon whose class
/// plays a pre-fire of `prefire` s (at its rate) with updates of `tick`
/// s: the update that sees the press, the pre-fire from the next, seen
/// done on the first update at least `prefire` + 1 ms later (where
/// `fire_0` starts), the round on the one after (weapon_classes.md
/// Corrections (2026-10-30) 1: a 0 ms pre-fire is 3, a 200 ms one at 60
/// updates a second 15).
pub fn prefire_latency(prefire: f32, tick: f64) -> u32 {
    let done = ((f64::from(prefire.max(0.0)) + 0.001) / tick - 1e-9).ceil().max(1.0);
    2 + done as u32
}

/// A flame gun's fire keys from `t` (special_weapons.md 4): the pre-fire,
/// the loop, its keys and their mean spacing, the post-fire. Returns the
/// spacing (the trigger's cycle at rate 1); None without `CaFlame`.
fn flame_times(e: &mut EntityWorldMut, t: &AnimTimes) -> Option<f32> {
    let mut f = e.get_mut::<CaFlame>()?;
    let keys = t.loop_keys();
    if let Some(p) = t.prefire {
        f.params.prefire = p;
    }
    if let Some(l) = t.fire {
        f.params.loop_len = l;
    }
    if !keys.is_empty() {
        f.params.keys = keys.len() as u32;
    }
    if keys.len() >= 2 {
        f.params.interval = (keys[keys.len() - 1] - keys[0]) / (keys.len() - 1) as f32;
    }
    f.params.postfire = t.postfire.unwrap_or(0.0);
    Some(f.params.interval)
}

/// A knife's swings from `t` (inventory.md 8.2, view_models.md 7 and C1):
/// the slash `fire_0` with its `FIRE_KEY`s, the stab `alt_fire_0` with its
/// `ALT_FIRE_KEY`s (a swing the model lacks, or keys it lacks, stay the
/// template's), the post-fire, at 1 + the record's `FireAniRate`.
fn knife_times(e: &mut EntityWorldMut, t: &AnimTimes, fire_rate: f32) {
    let Some(mut k) = e.get_mut::<Knife>() else { return };
    k.rate = played_rate(1.0 + fire_rate);
    let swings = [(t.fire, &t.fire_keys), (t.alt_fire, &t.alt_fire_keys)];
    for (a, (length, hits)) in k.attacks.iter_mut().zip(swings) {
        if let Some(l) = length {
            a.length = l;
        }
        if !hits.is_empty() {
            a.hits = hits.clone();
        }
        if let Some(p) = t.postfire {
            a.postfire = p;
        }
    }
}
