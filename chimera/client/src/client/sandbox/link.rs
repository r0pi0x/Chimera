//! What the sandbox's screens read from and ask of its mechanics (owned by
//! the mechanics side: `chimera_sim::minigame::targets`), as client-side
//! resources and messages: the bridge (`bridge_requests`, `bridge_view`,
//! `bridge_facts`) carries them both ways while a sandbox plays; a fake
//! (`fake_bench`) answers them in the `ui_scene` stagings.
//!
//! - **Targets** (the bench): `BenchRequest` (spawn, set, remove, clear a
//!   target; add or remove a prop) in the shapes of the mechanics'
//!   `SpawnTarget`, `SetTarget`, `RemoveTarget` and `ClearTargets`
//!   (`TargetSpec`: the character, its gun, distance, motion, shoots back,
//!   protected, respawn seconds); `BenchView` the targets and props as the
//!   bench lists them (a row each, alive or the time it comes back:
//!   `TargetRespawn`), and the last refusal ("no floor there") to show.
//! - **Measurement**: `KillLog` (the last kills, newest last, from the
//!   mechanics' `KillMeasured` and its `HitMeasured` hits: `KillCard`), read
//!   by the Tab column's last-kill card; `LookPanel` (the target under the
//!   crosshair: name, game, the damage of the last hit and in all, shield or
//!   gear, health, its anchor in the world) for the range's panel.
//! - **Blasts** (not started on the mechanics side): `BlastMeasured`, an
//!   explosion's centre, its full-damage and no-damage radii and the damage
//!   each body took, drawn as rings while `SandboxOptions::blast_rings`.
//! - **Props** (not started): `PropSpec` (bottles, tin cans, a target board
//!   at a distance), listed in `BenchView::props`.
//! - **Options** (`SandboxOptions`): endless ammo (the sandbox's refill),
//!   take no damage (`god`), half speed (not built), third person (a
//!   command), blast rings (on by default). `SandboxOptions::command` is
//!   the console line each toggle sends.
//! - **The fake** (`fake_bench`, while `FakeBench` is in): applies the
//!   requests to `BenchView` at once (a spawn past 150 m is refused, as a
//!   wall would), so the bench works in a staging.
//!
//! - **The bridge** (no `FakeBench`): a request calls `targets::spawn`,
//!   `set`, `remove` or `clear` (an error is the refusal shown); every
//!   frame the targets (`Target`, `TargetRespawn`, `TargetVitals`) make
//!   `BenchView::targets`; the local player's `HitMeasured` and
//!   `KillMeasured` make `KillLog`'s cards; the target nearest the
//!   crosshair's line (within `targets::AIM_RADIUS`) makes `LookPanel`;
//!   `targets::characters` makes the picker's `CharacterCatalogue`.
//!
//! Known shortcuts: props and blasts have no mechanics yet (props stay a
//! client-side list; no blast is measured); half speed isn't built; the
//! look panel picks by the crosshair's line, not the game's own trace.

use bevy::prelude::*;

/// How a target moves (the mechanics' `TargetMotion`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TargetMotion {
    #[default]
    Still,
    Strafes,
    Walks,
}

impl TargetMotion {
    pub const ALL: [TargetMotion; 3] = [TargetMotion::Still, TargetMotion::Strafes, TargetMotion::Walks];

    pub fn label(self) -> &'static str {
        match self {
            TargetMotion::Still => "Still",
            TargetMotion::Strafes => "Strafes",
            TargetMotion::Walks => "Walks",
        }
    }
}

/// The bench's distances, m.
pub const DISTANCES: [f32; 6] = [5.0, 10.0, 20.0, 40.0, 100.0, 200.0];

/// A target as asked for (the mechanics' `SpawnTarget` fields).
#[derive(Clone, Debug, PartialEq)]
pub struct TargetSpec {
    /// The body (`halo_ce:spartan`, `cs_source:phoenix`).
    pub character: String,
    /// Its gun (None: the character's default).
    pub gun: Option<String>,
    pub distance_m: f32,
    pub motion: TargetMotion,
    pub shoots_back: bool,
    /// Never falls: hits land and are measured, its health and shield are
    /// put back each tick (the mechanics' `protected`).
    pub protected: bool,
    /// Its second bar (shield, gear, armour) on; off: health only (the
    /// mechanics' `armored`).
    pub armored: bool,
    pub respawn_secs: f32,
}

impl Default for TargetSpec {
    fn default() -> Self {
        TargetSpec {
            character: String::new(),
            gun: None,
            distance_m: 10.0,
            motion: TargetMotion::Still,
            shoots_back: false,
            protected: false,
            armored: true,
            respawn_secs: 3.0,
        }
    }
}

/// A prop for accuracy (not started on the mechanics side).
#[derive(Clone, Debug, PartialEq)]
pub struct PropSpec {
    /// `bottles`, `tin_cans`, `target_board`.
    pub kind: String,
    pub distance_m: f32,
}

/// The props the bench offers: kind, name, what it says.
pub const PROPS: [(&str, &str, &str); 3] = [
    ("bottles", "Bottles", "6 on a crate"),
    ("tin_cans", "Tin cans", "5, knocked over by a hit"),
    ("target_board", "Target board", "Shows your group"),
];

/// A target's or a prop's id on the bench (the mechanics' entity, as bits).
pub type BenchId = u64;

/// What the bench asks of the mechanics (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub enum BenchRequest {
    Spawn(TargetSpec),
    Set(BenchId, TargetSpec),
    Remove(BenchId),
    Clear,
    AddProp(PropSpec),
    RemoveProp(BenchId),
}

/// A target as the bench lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetRow {
    pub id: BenchId,
    pub spec: TargetSpec,
    /// Its character's name and game ("Spartan", "Halo: Combat Evolved").
    pub name: String,
    pub game: String,
    /// While down: when it comes back (the mechanics' `TargetRespawn::at`,
    /// sim seconds) and how long it waits in all.
    pub respawn: Option<(f64, f32)>,
    /// Its spot in the world (the mechanics' `Target::home`).
    pub home: Option<Vec3>,
}

/// A prop as the bench lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct PropRow {
    pub id: BenchId,
    pub spec: PropSpec,
}

/// The targets and props (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BenchView {
    pub targets: Vec<TargetRow>,
    pub props: Vec<PropRow>,
    /// The last spawn the mechanics refused, and why.
    pub refused: Option<String>,
}

/// The most targets at once.
pub const MAX_TARGETS: usize = 6;

/// One hit of a kill (from the mechanics' `HitMeasured`).
#[derive(Clone, Debug, PartialEq)]
pub struct HitLine {
    pub damage: f32,
    /// "Chest", "Head" ...
    pub group: String,
    /// Seconds after the first hit.
    pub t: f32,
}

/// A kill as the last-kill card shows it (the mechanics' `KillMeasured`).
#[derive(Clone, Debug, PartialEq)]
pub struct KillCard {
    pub gun: String,
    pub gun_name: String,
    pub gun_game_short: String,
    pub victim: String,
    /// "Shield and health", "Body".
    pub protection: String,
    pub distance_m: f32,
    pub motion: TargetMotion,
    pub shots: u32,
    pub hits: Vec<HitLine>,
    pub first_hit_to_kill: f32,
    /// When (the UI clock, s).
    pub at: f64,
}

/// The last kills, newest last (module doc); the card shows the last and
/// the four before it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct KillLog(pub Vec<KillCard>);

impl KillLog {
    pub const KEEP: usize = 5;

    pub fn push(&mut self, k: KillCard) {
        self.0.push(k);
        let n = self.0.len();
        if n > Self::KEEP {
            self.0.drain(..n - Self::KEEP);
        }
    }
}

/// The target under the crosshair (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LookPanel(pub Option<LookedAt>);

#[derive(Clone, Debug, PartialEq)]
pub struct LookedAt {
    pub name: String,
    pub game_short: String,
    /// The last hit's damage and the damage since it last came up.
    pub last: Option<f32>,
    pub total: f32,
    /// Shield (or gear) and health: now, most. None: it has none.
    pub shield: Option<(f32, f32)>,
    pub health: (f32, f32),
    /// Its head's top and its feet, in the world (the panel sits beside).
    pub top: Vec3,
    pub feet: Vec3,
    /// Recharging its shield.
    pub recharging: bool,
}

/// An explosion's measure (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct BlastMeasured {
    pub at: Vec3,
    /// Full damage inside `inner`, none past `outer` (m).
    pub inner: f32,
    pub outer: f32,
    /// Each body hit: where, and the damage it took.
    pub hits: Vec<(Vec3, f32)>,
}

/// The sandbox's options (module doc).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SandboxOptions {
    pub endless_ammo: bool,
    pub no_damage: bool,
    pub half_speed: bool,
    pub third_person: bool,
    pub blast_rings: bool,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        SandboxOptions { endless_ammo: true, no_damage: true, half_speed: false, third_person: false, blast_rings: true }
    }
}

/// An option's place in the lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxOption {
    EndlessAmmo,
    NoDamage,
    BlastRings,
    HalfSpeed,
    ThirdPerson,
}

impl SandboxOption {
    pub const ALL: [SandboxOption; 5] =
        [SandboxOption::EndlessAmmo, SandboxOption::NoDamage, SandboxOption::BlastRings, SandboxOption::HalfSpeed, SandboxOption::ThirdPerson];

    pub fn label(self) -> &'static str {
        match self {
            SandboxOption::EndlessAmmo => "Endless ammo",
            SandboxOption::NoDamage => "Take no damage",
            SandboxOption::BlastRings => "Blast rings",
            SandboxOption::HalfSpeed => "Half speed",
            SandboxOption::ThirdPerson => "Third person",
        }
    }
}

impl SandboxOptions {
    pub fn get(&self, o: SandboxOption) -> bool {
        match o {
            SandboxOption::EndlessAmmo => self.endless_ammo,
            SandboxOption::NoDamage => self.no_damage,
            SandboxOption::BlastRings => self.blast_rings,
            SandboxOption::HalfSpeed => self.half_speed,
            SandboxOption::ThirdPerson => self.third_person,
        }
    }

    pub fn set(&mut self, o: SandboxOption, on: bool) {
        match o {
            SandboxOption::EndlessAmmo => self.endless_ammo = on,
            SandboxOption::NoDamage => self.no_damage = on,
            SandboxOption::BlastRings => self.blast_rings = on,
            SandboxOption::HalfSpeed => self.half_speed = on,
            SandboxOption::ThirdPerson => self.third_person = on,
        }
    }

    /// The console line that turns `o` to `on` (None: client-side only,
    /// or not built yet).
    pub fn command(o: SandboxOption, on: bool) -> Option<String> {
        match o {
            // `god` toggles: sent only when it must change.
            SandboxOption::NoDamage => Some("god".into()),
            SandboxOption::ThirdPerson => Some(if on { "thirdperson" } else { "firstperson" }.into()),
            _ => None,
        }
    }

    /// The condition tags under the gun chip: the options away from a
    /// plain match (take no damage only while Tab is held: design rules
    /// section 21, Q2).
    pub fn tags(&self, tab: bool) -> Vec<&'static str> {
        let mut t = Vec::new();
        if self.endless_ammo {
            t.push("Endless ammo");
        }
        if self.no_damage && tab {
            t.push("Take no damage");
        }
        if self.half_speed {
            t.push("Half speed");
        }
        if self.third_person {
            t.push("Third person");
        }
        t
    }
}

/// The fake mechanics are in (stagings): `fake_bench` answers requests.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct FakeBench {
    next: BenchId,
}

/// A character's name and game for the fake (by its id's tail).
pub fn character_words(id: &str) -> (String, String) {
    let (g, n) = id.split_once(':').unwrap_or(("", id));
    let mut c = n.replace('_', " ");
    if let Some(f) = c.get(..1) {
        c = f.to_uppercase() + &c[1..];
    }
    (c, crate::client::sandbox::game_name(g))
}

/// The fake mechanics (module doc).
pub fn fake_bench(fake: Option<ResMut<FakeBench>>, mut requests: MessageReader<BenchRequest>, mut view: ResMut<BenchView>) {
    let Some(mut fake) = fake else { return };
    for r in requests.read() {
        let mut v = view.clone();
        match r {
            BenchRequest::Spawn(spec) => {
                if spec.distance_m > 150.0 {
                    v.refused = Some(format!("No room {} m out: a wall is in the way.", spec.distance_m));
                } else if v.targets.len() >= MAX_TARGETS {
                    v.refused = Some(format!("{MAX_TARGETS} targets at most."));
                } else {
                    fake.next += 1;
                    let (name, game) = character_words(&spec.character);
                    v.targets.push(TargetRow { id: fake.next, spec: spec.clone(), name, game, respawn: None, home: None });
                    v.refused = None;
                }
            }
            BenchRequest::Set(id, spec) => {
                if let Some(t) = v.targets.iter_mut().find(|t| t.id == *id) {
                    (t.name, t.game) = character_words(&spec.character);
                    t.spec = spec.clone();
                }
            }
            BenchRequest::Remove(id) => v.targets.retain(|t| t.id != *id),
            BenchRequest::Clear => {
                v.targets.clear();
                v.props.clear();
            }
            BenchRequest::AddProp(p) => {
                fake.next += 1;
                v.props.push(PropRow { id: fake.next, spec: p.clone() });
            }
            BenchRequest::RemoveProp(id) => v.props.retain(|p| p.id != *id),
        }
        if *view != v {
            *view = v;
        }
    }
}

// ---------------------------------------------------------------------
// The bridge to `chimera_sim::minigame::targets`.

use chimera_sim::minigame::targets as tg;

fn motion_of(m: TargetMotion) -> tg::TargetMotion {
    match m {
        TargetMotion::Still => tg::TargetMotion::Still,
        TargetMotion::Strafes => tg::TargetMotion::Strafes,
        TargetMotion::Walks => tg::TargetMotion::Walks,
    }
}

fn motion_from(m: tg::TargetMotion) -> TargetMotion {
    match m {
        tg::TargetMotion::Still => TargetMotion::Still,
        tg::TargetMotion::Strafes => TargetMotion::Strafes,
        tg::TargetMotion::Walks => TargetMotion::Walks,
    }
}

/// The bench's requests to the mechanics (module doc, "The bridge").
pub fn bridge_requests(world: &mut World) {
    if world.contains_resource::<FakeBench>() || !world.get_resource::<super::SandboxUi>().is_some_and(|s| s.on) {
        return;
    }
    let Some(mut msgs) = world.get_resource_mut::<Messages<BenchRequest>>() else { return };
    let reqs: Vec<BenchRequest> = msgs.drain().collect();
    for r in reqs {
        let said = match r {
            BenchRequest::Spawn(s) => tg::spawn(
                world,
                &tg::SpawnTarget {
                    character: (!s.character.is_empty()).then(|| s.character.clone()),
                    gun: s.gun.clone(),
                    rung: None,
                    distance_m: s.distance_m,
                    motion: motion_of(s.motion),
                    shoots_back: s.shoots_back,
                    protected: s.protected,
                    armored: s.armored,
                    respawn_secs: s.respawn_secs,
                    by: None,
                },
            )
            .map(|_| ()),
            BenchRequest::Set(id, s) => {
                let e = Entity::from_bits(id);
                let Some(old) = world.get::<tg::Target>(e).cloned() else { continue };
                let mut set = tg::SetTarget::of(e);
                if s.character != old.character {
                    set.character = Some(s.character.clone());
                }
                if s.gun.is_some() && s.gun.as_deref() != Some(old.gun.as_str()) {
                    set.gun = s.gun.clone();
                }
                if (s.distance_m - old.distance_m).abs() > 0.01 {
                    set.distance_m = Some(s.distance_m);
                }
                if motion_of(s.motion) != old.motion {
                    set.motion = Some(motion_of(s.motion));
                }
                if s.shoots_back != old.shoots_back {
                    set.shoots_back = Some(s.shoots_back);
                }
                if s.protected != old.protected {
                    set.protected = Some(s.protected);
                }
                if s.armored != old.armored {
                    set.armored = Some(s.armored);
                }
                tg::set(world, &set).map(|_| ())
            }
            BenchRequest::Remove(id) => {
                tg::remove(world, Entity::from_bits(id));
                Ok(())
            }
            BenchRequest::Clear => {
                tg::clear(world);
                world.resource_mut::<BenchView>().props.clear();
                Ok(())
            }
            // Props have no mechanics yet: a client-side list.
            BenchRequest::AddProp(p) => {
                let mut v = world.resource_mut::<BenchView>();
                let id = v.props.iter().map(|p| p.id).max().unwrap_or(1 << 40) + 1;
                v.props.push(PropRow { id, spec: p });
                Ok(())
            }
            BenchRequest::RemoveProp(id) => {
                world.resource_mut::<BenchView>().props.retain(|p| p.id != id);
                Ok(())
            }
        };
        let mut v = world.resource_mut::<BenchView>();
        let refused = said.err();
        if v.refused != refused {
            v.refused = refused;
        }
    }
}

/// The targets into `BenchView`, the picker's characters (module doc).
#[allow(clippy::type_complexity)]
pub fn bridge_view(
    ui: Res<super::SandboxUi>,
    fake: Option<Res<FakeBench>>,
    targets: Query<(Entity, &tg::Target, Option<&tg::TargetRespawn>)>,
    mut refused: Option<MessageReader<tg::TargetRefused>>,
    mut view: ResMut<BenchView>,
) {
    if fake.is_some() || !ui.on {
        return;
    }
    let mut rows: Vec<(u32, TargetRow)> = targets
        .iter()
        .map(|(e, t, r)| {
            (
                t.number,
                TargetRow {
                    id: e.to_bits(),
                    spec: TargetSpec {
                        character: t.character.clone(),
                        gun: Some(t.gun.clone()),
                        distance_m: t.distance_m,
                        motion: motion_from(t.motion),
                        shoots_back: t.shoots_back,
                        protected: t.protected,
                        armored: t.armored,
                        respawn_secs: t.respawn_secs,
                    },
                    name: t.character_name.clone(),
                    game: t.game_name.clone(),
                    respawn: r.map(|r| (r.at, t.respawn_secs)),
                    home: Some(t.home),
                },
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    let rows: Vec<TargetRow> = rows.into_iter().map(|r| r.1).collect();
    let why = refused.as_mut().and_then(|r| r.read().last().map(|r| r.reason.clone()));
    if view.targets != rows || why.is_some() {
        view.targets = rows;
        if why.is_some() {
            view.refused = why;
        }
    }
}

/// The bodies for the picker, made when a sandbox comes up.
pub fn bridge_characters(world: &mut World) {
    if world.contains_resource::<FakeBench>() {
        return;
    }
    let on = world.get_resource::<super::SandboxUi>().is_some_and(|s| s.on);
    let have = world.get_resource::<super::bench::CharacterCatalogue>().is_some_and(|c| !c.0.is_empty());
    if !on {
        if have {
            world.insert_resource(super::bench::CharacterCatalogue::default());
        }
        return;
    }
    if have {
        return;
    }
    let names = world.get_resource::<crate::client::minigame_scoreboard::GunNames>();
    let catalog = world.get_resource::<chimera_sim::weapon::data::WeaponCatalog>();
    let list: Vec<super::bench::CharacterEntry> = tg::characters(world)
        .into_iter()
        .map(|c| super::bench::CharacterEntry {
            id: c.id,
            name: c.name,
            game: c.game_name,
            shield: (c.shield_max * 100.0).round(),
            health: (c.health_max * 100.0).round(),
            default_gun: c.guns.first().map_or("Its default".into(), |g| crate::client::minigame_scoreboard::gun_name(g, names, catalog)),
            guns: c.guns,
            bytes: None,
        })
        .collect();
    if !list.is_empty() {
        world.insert_resource(super::bench::CharacterCatalogue(list));
    }
}

/// Hits so far on each target since its last kill.
#[derive(Resource, Default)]
pub struct HitsSoFar(pub std::collections::BTreeMap<u64, Vec<(f64, f32, String)>>);

/// The local player's measured hits and kills into `KillLog`, the last
/// hit on each target into the look panel (module doc).
#[allow(clippy::too_many_arguments)]
pub fn bridge_facts(
    ui: Res<super::SandboxUi>,
    clock: Option<Res<chimera_ui::UiClock>>,
    me: Query<Entity, With<crate::core::LocalPlayer>>,
    targets: Query<&tg::Target>,
    mut hits: Option<MessageReader<tg::HitMeasured>>,
    mut kills: Option<MessageReader<tg::KillMeasured>>,
    names: Option<Res<crate::client::minigame_scoreboard::GunNames>>,
    catalog: Option<Res<chimera_sim::weapon::data::WeaponCatalog>>,
    games: Option<Res<chimera_core::game_registry::Games>>,
    mut so_far: ResMut<HitsSoFar>,
    mut last_hit: ResMut<LastHits>,
    mut log: ResMut<KillLog>,
) {
    if !ui.on || ui.staged {
        return;
    }
    let me = me.iter().next();
    let now = clock.map_or(0.0, |c| c.now);
    if let Some(h) = hits.as_mut() {
        for h in h.read() {
            if h.attacker.is_none() || h.attacker != me {
                continue;
            }
            let group = format!("{:?}", h.hitgroup);
            so_far.0.entry(h.victim.to_bits()).or_default().push((h.time, h.damage * 100.0, group));
            let e = last_hit.0.entry(h.victim.to_bits()).or_default();
            e.0 = Some(h.damage * 100.0);
            e.1 += h.damage * 100.0;
        }
    }
    if let Some(k) = kills.as_mut() {
        for k in k.read() {
            if k.attacker.is_none() || k.attacker != me {
                continue;
            }
            let v = targets.get(k.victim).ok();
            let hits = so_far.0.remove(&k.victim.to_bits()).unwrap_or_default();
            let t0 = hits.first().map_or(k.time, |h| h.0);
            let gun = k.gun.unwrap_or("").to_string();
            let game = gun.split_once(':').map_or("", |(g, _)| g);
            log.push(KillCard {
                gun_name: crate::client::minigame_scoreboard::gun_name(&gun, names.as_deref(), catalog.as_deref()),
                gun_game_short: super::game_short(game, games.as_deref()),
                gun,
                victim: v.map_or("Target".into(), |t| t.character_name.clone()),
                protection: v.map_or(String::new(), |t| if t.protected { "Shield and health".into() } else { "Health only".into() }),
                distance_m: k.distance_m,
                motion: v.map_or(TargetMotion::Still, |t| motion_from(t.motion)),
                shots: k.shots_fired,
                hits: hits.iter().map(|(t, d, g)| HitLine { damage: *d, group: g.clone(), t: (t - t0) as f32 }).collect(),
                first_hit_to_kill: k.from_first_hit,
                at: now,
            });
            last_hit.0.remove(&k.victim.to_bits());
        }
    }
}

/// The last hit's damage and the damage in all on each target (points).
#[derive(Resource, Default)]
pub struct LastHits(pub std::collections::BTreeMap<u64, (Option<f32>, f32)>);

/// The target nearest the crosshair's line into `LookPanel`.
#[allow(clippy::type_complexity)]
pub fn bridge_look(
    ui: Res<super::SandboxUi>,
    camera: Query<&GlobalTransform, With<crate::client::FirstPersonCamera>>,
    targets: Query<(Entity, &tg::Target, &GlobalTransform, Option<&tg::TargetVitals>, Has<tg::TargetRespawn>)>,
    last: Res<LastHits>,
    games: Option<Res<chimera_core::game_registry::Games>>,
    mut panel: ResMut<LookPanel>,
) {
    if !ui.on || ui.staged {
        return;
    }
    let Some(cam) = camera.iter().next() else { return };
    let (eye, fwd) = (cam.translation(), cam.forward().as_vec3());
    let mut best: Option<(f32, LookedAt)> = None;
    for (e, t, g, vit, dead) in &targets {
        if dead {
            continue;
        }
        let chest = g.translation() + Vec3::Y * 1.0;
        let along = (chest - eye).dot(fwd);
        if along <= 0.0 {
            continue;
        }
        let off = (chest - (eye + fwd * along)).length();
        if off > tg::AIM_RADIUS || best.as_ref().is_some_and(|b| b.0 <= along) {
            continue;
        }
        let v = vit.copied().unwrap_or_default();
        let (l, total) = last.0.get(&e.to_bits()).copied().unwrap_or((None, 0.0));
        best = Some((
            along,
            LookedAt {
                name: t.character_name.clone(),
                game_short: super::game_short(&t.game, games.as_deref()),
                last: l,
                total,
                shield: (v.shield_max > 0.0).then_some((v.shield * 100.0, v.shield_max * 100.0)),
                health: (v.health * 100.0, v.health_max * 100.0),
                top: g.translation() + Vec3::Y * 1.9,
                feet: g.translation(),
                recharging: false,
            },
        ));
    }
    let next = LookPanel(best.map(|b| b.1));
    if *panel != next {
        *panel = next;
    }
}
