//! The view-model activity driver (any game): which sequence each
//! character's first-person view model plays (`map::ViewAnimator`) as its
//! weapon is drawn, fired, reloaded and zoomed, and the effects the
//! animation's events ask for (`map::ViewModelEvent`). Per character and
//! frame (`drive`, in `map::DriveAnimation`, simulation side so it runs
//! headless; its data's procedural bones after it, `view_bones`): show the active weapon's view model (drawing it when it
//! changes, or when its model comes in after the weapon did; nothing when
//! dead), react to the weapon's `WeaponEvent`s,
//! advance the animation and send the effects of the events its cycle
//! passed, and idle once the sequence has played out (a looping attack
//! sequence, TF2's `ft_fire`: one length after the last attack,
//! `ViewFrame::sustain`), no grenade is held
//! primed, no reload runs and the weapon's idle delay has passed (its
//! data's `[anim]`: `idle_after`, `idle_interval`, `idle_after_draw`). A
//! view model behind a scope (`Zoomed::scope`) is hidden. An animation
//! event the rules read as a sound (`ViewModelEventKind::Sound`: Source's
//! 5004) is played for this machine's own player only, and a reload's for
//! the others too when the game's rules say so (`ViewAnimRules::others_reload`;
//! `play_view_sounds`). An owner's sound event whose entry has no wave
//! here (its file missing from the install or the asset store) is said
//! once per entry and map and, in a reload, stands in as the gun's world
//! reload.
//!
//! What plays when is the weapon's:
//!
//! - A weapon whose data has `[anim.sequences]` (action -> sequence names,
//!   docs/weapon-data.md) plays those by name (`data_event`): `draw` on
//!   the draw, `idle`, `fire` on each round (`Fired`; `fire_last` for the
//!   round that empties the clip), `reload` on a magazine reload,
//!   `reload_start`/`reload_loop`/`reload_end` on a shell-by-shell one,
//!   `zoom_in`/`zoom_out` when the owner's `Zoomed` comes and goes.
//!   Each action names one or more sequences (one at random); a missing
//!   action plays nothing (the sequence playing goes on).
//! - Any other weapon asks its game's `ViewAnimRules` (draw and idle
//!   choices, a reaction per event, the effects of animation events): the
//!   entry in `GameViewAnimRules` of the game its id names (`game:name`);
//!   CS:S picks Source activities by its weapons' rules. Without rules for
//!   its game, such a weapon's view model shows its reference pose. A
//!   weapon's `ViewPose` (a game's own pose number) reaches the rules as
//!   `ViewState::pose` (TF2's spun Minigun idles in its spun sequence).
//!
//! Games add `ViewAnimPlugin`, register their `ViewAnimRules` under their
//! game id (`app.view_anim_rules(game, rules)`; several games' rules live
//! side by side, a game registering again replaces its own) and insert
//! `map::ViewModelSettings` (field of view, hand). A game that drives a
//! character's view model itself (its `ViewAnimator`, e.g. Combat Arms'
//! own state machine) puts `OwnViewAnim(true)` on it and the driver leaves it
//! alone.
//!
//! **A view model's variant** (`ViewVariant`, on the character): a game
//! whose weapon looks different in some holders' hands (TF2's Shotgun on
//! each class's arms) loads those view models keyed `<weapon>#<variant>`
//! (`variant_key`) and puts the holder's variant on; the driver shows that
//! key when the map has it, else the weapon's own, and the rules see it
//! (`ViewState::variant`).

use bevy::prelude::*;

use super::{
    data::{WeaponCatalog, WeaponData},
    AltModes, Burst, Inventory, Magazine, ShellReload, Weapon, WeaponEvent, WeaponEventKind, WeaponState, ViewModelHidden, Zoomed,
    grenade::Throwable,
};
use crate::{
    core::Health,
    map::{
        DriveAnimation, EffectSettings, GameViewModelFovs, ViewAnimator, ViewModelEvent, ViewModelEventKind, ViewModelSettings,
        ViewModels,
        anim::{AnimEvent, Sequence},
    },
};

/// The data actions (`[anim.sequences]` keys) the driver plays.
pub const ACTIONS: &[&str] = &[
    "draw",
    "idle",
    "fire",
    "fire_last",
    "reload",
    "reload_start",
    "reload_loop",
    "reload_end",
    "zoom_in",
    "zoom_out",
];

/// The driver and its messages; `ViewModelSettings` and `EffectSettings`
/// at their defaults unless the game inserted its own.
pub struct ViewAnimPlugin;

impl Plugin for ViewAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WeaponEvent>()
            .add_message::<ViewModelEvent>()
            .add_message::<crate::map::PlaySound>()
            .init_resource::<ViewModelSettings>()
            .init_resource::<EffectSettings>()
            .add_systems(Update, (drive, super::view_bones::turn, play_view_sounds).chain().in_set(DriveAnimation));
    }
}

/// The view models' sound events (`ViewModelEventKind::Sound`) played from
/// their owner for this machine's own player alone (specs/tf2/
/// viewmodel_sounds.md 1.1: as the animation passes them, nobody else
/// hears them: others' view models, which nobody here draws, play theirs
/// silently), and a reload's, when the game's rules say others hear them
/// (`ViewAnimRules::others_reload`, `ViewModelEvent::others`), from the
/// reloading character's place (specs/cs_source/sounds.md 5.1), the later
/// ones after their lag.
///
/// An owner's event whose entry the map's table holds without a wave (its
/// files missing from the install or the asset store: a gun's set
/// published before the entry joined it) is said once per entry and map
/// (`unplayable_view_sound`) and, for a reload's sequence, stands in as the
/// gun's world reload (`WeaponSounds::world_reload`) when that plays: a
/// reload heard, if not the game's own, rather than none.
#[allow(clippy::type_complexity)]
pub fn play_view_sounds(
    time: Res<Time>,
    mut effects: MessageReader<ViewModelEvent>,
    owners: Query<(&Transform, Has<crate::core::LocalPlayer>, Option<&Inventory>)>,
    guns: Query<&super::WeaponSounds>,
    bank: Option<Res<crate::map::sound::SoundBank>>,
    mut play: MessageWriter<crate::map::PlaySound>,
    (mut later, mut said): (Local<Vec<(f64, Entity, String)>>, Local<bevy::platform::collections::HashSet<String>>),
) {
    let now = time.elapsed_secs_f64();
    if bank.as_ref().is_some_and(|b| b.is_changed()) {
        said.clear();
    }
    let mut say = |owner: Entity, entry: String, play: &mut MessageWriter<crate::map::PlaySound>| {
        if let Ok((at, ..)) = owners.get(owner) {
            play.write(crate::map::PlaySound {
                source: Some(owner),
                ..crate::map::PlaySound::at(entry, at.translation)
            });
        }
    };
    let mut i = 0;
    while i < later.len() {
        if later[i].0 <= now {
            let (_, owner, entry) = later.remove(i);
            say(owner, entry, &mut play);
        } else {
            i += 1;
        }
    }
    // Whether the map's table holds `entry` with no wave to play.
    let waveless = |entry: &str| bank.as_ref().and_then(|b| b.0.entry(entry)).is_some_and(|e| e.waves.is_empty());
    let playable = |entry: &str| bank.as_ref().and_then(|b| b.0.entry(entry)).is_some_and(|e| !e.waves.is_empty());
    for ev in effects.read() {
        let ViewModelEventKind::Sound { entry } = &ev.kind else { continue };
        let Ok((_, local, inventory)) = owners.get(ev.owner) else { continue };
        if local && waveless(entry) {
            let stand_in = ev
                .reload
                .then(|| inventory.and_then(|i| i.active).and_then(|w| guns.get(w).ok()))
                .flatten()
                .and_then(|s| s.world_reload.clone())
                .filter(|w| playable(w));
            if said.insert(entry.to_lowercase()) {
                warn!("{}", unplayable_view_sound(entry, stand_in.as_deref()));
            }
            if let Some(w) = stand_in {
                say(ev.owner, w, &mut play);
            }
            continue;
        }
        match ev.others {
            _ if local => say(ev.owner, entry.clone(), &mut play),
            Some(lag) if lag > 0.0 => later.push((now + lag as f64, ev.owner, entry.clone())),
            Some(_) => say(ev.owner, entry.clone(), &mut play),
            None => {}
        }
    }
}

/// What a view model's sound event that can't play says (`play_view_sounds`):
/// its entry's waves are missing, and what is heard instead.
pub fn unplayable_view_sound(entry: &str, stand_in: Option<&str>) -> String {
    let instead = stand_in.map_or_else(|| "not heard".to_string(), |w| format!("the gun's world reload {w} plays instead"));
    format!(
        "view-model sound {entry}: none of its waves could be read (missing from the install or the asset store: \
         a gun's set published before the entry joined it needs publishing again); {instead}"
    )
}

/// A console variable for a game's view-model field of view (horizontal
/// degrees at 4:3), kept within `min..=max` and saved in the config: the
/// game's entry in `GameViewModelFovs` when the app has one (an app of
/// several games), else `ViewModelSettings::fov`.
pub fn fov_cvar(app: &mut App, name: &str, help: &str, game: &'static str, (min, max): (f32, f32)) {
    use crate::console::ConsoleAppExt;
    let read = move |w: &mut World| -> f32 {
        let own = w.get_resource::<GameViewModelFovs>().and_then(|g| g.0.iter().find(|f| f.game == game).map(|f| f.fov));
        own.or_else(|| w.get_resource::<ViewModelSettings>().map(|s| s.fov)).unwrap_or(min)
    };
    let default = app
        .world()
        .get_resource::<GameViewModelFovs>()
        .and_then(|g| g.0.iter().find(|f| f.game == game).map(|f| f.fov))
        .or_else(|| app.world().get_resource::<ViewModelSettings>().map(|s| s.fov))
        .unwrap_or(min);
    app.console_cvar(
        name,
        help,
        &default.to_string(),
        move |w| Some(read(w).to_string()),
        move |w, v| {
            let v: f32 = v.trim().parse().map_err(|_| format!("bad value \"{v}\""))?;
            let v = v.clamp(min, max);
            if let Some(mut g) = w.get_resource_mut::<GameViewModelFovs>()
                && let Some(f) = g.0.iter_mut().find(|f| f.game == game)
            {
                f.fov = v;
            } else if let Some(mut s) = w.get_resource_mut::<ViewModelSettings>() {
                s.fov = v;
            }
            Ok(())
        },
    );
    let mut console = app.world_mut().resource_mut::<crate::console::Console>();
    console.archive(name);
    console.set_range(name, min, max);
}

/// On a character whose view model the game drives itself (true): the
/// driver skips it. Once on, it stays, false while another game's gun is
/// in hand: put on and off, it was an archetype for every combination of
/// the character's other parts (docs/DESIGN.md, "ECS style").
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct OwnViewAnim(pub bool);

/// Sequences to play: the first activity any sequence has (a weighted
/// pick among its sequences), or one of the named sequences at random.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Activities(Vec<String>),
    Names(Vec<String>),
}

impl Pick {
    pub fn activities(list: &[&str]) -> Self {
        Self::Activities(list.iter().map(|s| s.to_string()).collect())
    }

    /// Whether `s` is one of these (an activity by prefix: its variants).
    pub fn matches(&self, s: &Sequence) -> bool {
        match self {
            Self::Activities(a) => a.iter().any(|a| s.activity.starts_with(a.as_str())),
            Self::Names(n) => n.iter().any(|n| s.name.eq_ignore_ascii_case(n)),
        }
    }
}

/// Per character: when it last attacked, drew and idled, and its
/// sequence-choice dice.
#[derive(Component, Debug, Clone)]
pub struct ViewModelPlay {
    pub last_attack: f64,
    /// When the weapon in hand was drawn, and when its idle last started.
    pub drawn_at: f64,
    pub idle_started: f64,
    /// When the burst playing started (games' rules: its later rounds
    /// don't restart it).
    pub burst_started: f64,
    /// Whether the owner was zoomed last frame (`zoom_in`/`zoom_out`).
    pub zoomed: bool,
    /// Games' rules' count of a series of attacks (HL2's pistol counts
    /// shots close together).
    pub series: u32,
    /// The looping sequence the last attack started (TF2's Flame
    /// Thrower's `ft_fire`): it counts as played out one length after the
    /// last attack (`ViewFrame::sustain` keeps it going), as a Source
    /// weapon's idle time is the attack sequence's duration.
    pub attacked: Option<usize>,
    /// The sequence a reload event (start, a shell, the finish) last
    /// started: its sound events are the reload's (`ViewModelEvent::others`).
    pub reload_seq: Option<usize>,
    /// A draw played faster than its own rate (`ViewAnimRules::draw_rate`)
    /// holds its last frame until then: its length at its own rate after
    /// the draw (Source's idle time; an attack ends the hold).
    pub draw_held_until: f64,
    rng: u32,
}

impl ViewModelPlay {
    pub fn new(owner: Entity) -> Self {
        Self {
            last_attack: f64::MIN,
            drawn_at: f64::MIN,
            idle_started: f64::MIN,
            burst_started: f64::MIN,
            zoomed: false,
            series: 0,
            attacked: None,
            reload_seq: None,
            draw_held_until: f64::MIN,
            rng: owner.to_bits() as u32 ^ 0x9e37_79b9,
        }
    }

    /// A roll in [0, 1).
    pub fn roll(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Play `pick` from the start; false when the model has none of it.
pub fn play_pick(view: &mut ViewAnimator, dice: &mut ViewModelPlay, pick: &Pick, now: f64) -> bool {
    let Some(animator) = view.animator.as_mut() else {
        return false;
    };
    let s = match pick {
        Pick::Activities(list) => {
            let roll = dice.roll();
            list.iter().find_map(|a| animator.set.pick_activity(a, roll))
        }
        Pick::Names(names) => {
            let found: Vec<usize> = names.iter().filter_map(|n| animator.set.sequence(n)).collect();
            if found.is_empty() {
                None
            } else {
                Some(found[((dice.roll() * found.len() as f32) as usize).min(found.len() - 1)])
            }
        }
    };
    let Some(s) = s else { return false };
    animator.restart(s, now);
    true
}

/// A game's pose for a weapon's view model other than its usual (0): the
/// rules read it as `ViewState::pose` to idle in another sequence (TF2's
/// spun Minigun idles lowered, aiming). Kept on the weapon from its
/// attach, changed in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewPose(pub u8);

/// A character's view-model variant (module doc): the view model keyed
/// `variant_key(weapon, variant)` is shown in place of the weapon's own
/// when the map has it.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewVariant(pub String);

/// The key of `weapon`'s view model in `variant`'s hands
/// (`<weapon>#<variant>`).
pub fn variant_key(weapon: &str, variant: &str) -> String {
    format!("{weapon}#{variant}")
}

/// What the rules see of a character's weapon in hand.
pub struct ViewState<'a> {
    pub weapon: &'static str,
    /// The holder's `ViewVariant` (None without).
    pub variant: Option<&'a str>,
    /// Its data (None: the app's catalog lacks it).
    pub data: Option<&'static WeaponData>,
    /// Its `AltModes` mode (0 without).
    pub mode: u8,
    /// Its burst in the current mode.
    pub burst: Option<&'a Burst>,
    pub clip: Option<u32>,
    /// A grenade held with its pin out.
    pub primed: bool,
    /// Reloads shell by shell.
    pub shells: bool,
    pub reloading: bool,
    pub zoomed: Option<&'a Zoomed>,
    /// Shots since the trigger was last released (`WeaponState::burst`).
    pub shots: u32,
    /// Its `ViewPose` (0 without).
    pub pose: u8,
}

/// One character's view model this frame, for the rules' reactions.
pub struct ViewFrame<'a, 's> {
    pub owner: Entity,
    pub state: ViewState<'s>,
    pub view: &'a mut ViewAnimator,
    pub dice: &'a mut ViewModelPlay,
    pub now: f64,
    /// A draw played this frame.
    pub drawn: bool,
    /// A fire sequence started this frame (later pellets don't restart it).
    pub fired: bool,
    /// Something restarted this frame (its events from cycle 0).
    pub restarted: bool,
}

impl ViewFrame<'_, '_> {
    /// Play `pick` from the start.
    pub fn play(&mut self, pick: &Pick) -> bool {
        let played = play_pick(self.view, self.dice, pick, self.now);
        self.restarted |= played;
        played
    }

    /// Play `pick` as an attack (the idle delay counts from now).
    pub fn attack(&mut self, pick: &Pick) -> bool {
        self.dice.last_attack = self.now;
        self.dice.draw_held_until = f64::MIN;
        let played = self.play(pick);
        if played {
            self.dice.attacked = self.view.animator.as_ref().and_then(|a| a.main);
        }
        played
    }

    /// A looping attack sequence already playing goes on (a shot of a gun
    /// whose fire sequence loops): its idle delay counts from now, and it
    /// runs out one length after the last of these (`ViewModelPlay::attacked`).
    pub fn sustain(&mut self) {
        self.dice.last_attack = self.now;
    }
}

/// A game's view-model rules, for its weapons without `[anim.sequences]`
/// (registered per game: `GameViewAnimRules`).
#[derive(Clone, Copy)]
pub struct ViewAnimRules {
    /// What plays when the weapon is drawn.
    pub draw: fn(&ViewState) -> Pick,
    /// What it idles with.
    pub idle: fn(&ViewState) -> Pick,
    /// The reaction to one of the owner's weapon events (but `Deployed`,
    /// the driver's).
    pub event: fn(&mut ViewFrame, &WeaponEventKind),
    /// What an animation event shows (muzzle flash, brass); for every view
    /// model.
    pub effect: fn(&AnimEvent) -> Option<ViewModelEventKind>,
    /// The draw's playback rate (1: its own). Faster, it then holds its
    /// last frame until its length at its own rate has passed before the
    /// idle (TF2's 1.34: specs/tf2/viewmodel.md 6, a 0.67 s draw over a
    /// 0.5 s deploy).
    pub draw_rate: f32,
    /// Others hear the sound events of a reload's sequence
    /// (`ViewModelEvent::others`): the later ones this many seconds behind
    /// the sequence as this machine plays it (CS:S's 0.075, specs/cs_source/
    /// sounds.md 5.1: the first, at cycle 0, as the reload's start arrives).
    /// None: the holder alone hears them.
    pub others_reload: Option<f32>,
}

/// After a draw played: its rate, and the hold of its last frame
/// (`ViewModelPlay::draw_held_until`).
fn draw_rate(view: &mut ViewAnimator, dice: &mut ViewModelPlay, rate: f32, now: f64) {
    let Some(a) = view.animator.as_mut() else { return };
    if rate == 1.0 || rate <= 0.0 {
        return;
    }
    let Some(main) = a.main else { return };
    a.playback = rate;
    dice.draw_held_until = now + a.set.duration(main) as f64;
}

/// Each game's `ViewAnimRules`, by game id (the prefix of its weapons'
/// ids, `game:name`). The driver asks the held weapon's game's.
#[derive(Resource, Clone, Default)]
pub struct GameViewAnimRules(Vec<(&'static str, ViewAnimRules)>);

impl GameViewAnimRules {
    /// `game`'s rules (ids compare ignoring case).
    pub fn get(&self, game: &str) -> Option<&ViewAnimRules> {
        self.0.iter().find(|(g, _)| g.eq_ignore_ascii_case(game)).map(|(_, r)| r)
    }

    /// The rules of the game weapon `id` belongs to (`game:name`).
    pub fn for_weapon(&self, id: &str) -> Option<&ViewAnimRules> {
        self.get(id.split_once(':')?.0)
    }

    /// `game`'s rules from now on (replacing its own, if it had some).
    pub fn set(&mut self, game: &'static str, rules: ViewAnimRules) {
        match self.0.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(game)) {
            Some(entry) => entry.1 = rules,
            None => self.0.push((game, rules)),
        }
    }

    /// The games with rules, in registration order.
    pub fn games(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().map(|(g, _)| *g)
    }
}

/// Registering a game's `ViewAnimRules`.
pub trait ViewAnimAppExt {
    /// `game`'s view-model rules (`GameViewAnimRules::set`).
    fn view_anim_rules(&mut self, game: &'static str, rules: ViewAnimRules) -> &mut Self;
}

impl ViewAnimAppExt for App {
    fn view_anim_rules(&mut self, game: &'static str, rules: ViewAnimRules) -> &mut Self {
        self.world_mut().get_resource_or_init::<GameViewAnimRules>().set(game, rules);
        self
    }
}

/// The data's sequences for `action` (None: the data has no table).
fn data_pick(data: Option<&WeaponData>, action: &str) -> Option<Pick> {
    data?.anim.sequences.get(action).map(|n| Pick::Names(n.clone()))
}

/// A data-driven weapon's reaction to an event (module doc).
pub fn data_event(frame: &mut ViewFrame, kind: &WeaponEventKind) {
    let data = frame.state.data;
    let pick = |a: &str| data_pick(data, a);
    match kind {
        WeaponEventKind::Fired { .. } if !frame.fired => {
            let last = frame.state.clip == Some(0);
            let p = last.then(|| pick("fire_last")).flatten().or_else(|| pick("fire"));
            if let Some(p) = p {
                frame.fired = frame.attack(&p);
            }
        }
        WeaponEventKind::ReloadStarted => {
            let p = if frame.state.shells {
                pick("reload_start").or_else(|| pick("reload"))
            } else {
                pick("reload")
            };
            if let Some(p) = p {
                frame.attack(&p);
            }
        }
        WeaponEventKind::ShellInserting => {
            if let Some(p) = pick("reload_loop") {
                frame.attack(&p);
            }
        }
        WeaponEventKind::Reloaded if frame.state.shells => {
            if let Some(p) = pick("reload_end") {
                frame.attack(&p);
            }
        }
        _ => {}
    }
}

fn no_effect(_: &AnimEvent) -> Option<ViewModelEventKind> {
    None
}

#[allow(clippy::type_complexity)]
pub fn drive(
    time: Res<Time>,
    models: Option<Res<ViewModels>>,
    rules: Option<Res<GameViewAnimRules>>,
    catalog: Option<Res<WeaponCatalog>>,
    mut characters: Query<
        (
            Entity,
            &Inventory,
            Option<&Health>,
            Option<&mut ViewAnimator>,
            Option<&mut ViewModelPlay>,
            Option<&Zoomed>,
            Has<ViewModelHidden>,
            Option<&ViewVariant>,
            Option<&OwnViewAnim>,
        ),
    >,
    weapons: Query<(
        &Weapon,
        Option<&AltModes>,
        Option<&Burst>,
        Option<&Magazine>,
        Option<&Throwable>,
        (Option<&WeaponState>, Option<&ShellReload>, Option<&ViewPose>),
    )>,
    mut events: MessageReader<WeaponEvent>,
    mut effects: MessageWriter<ViewModelEvent>,
    mut commands: Commands,
) {
    let (dt, now) = (time.delta_secs(), time.elapsed_secs_f64());
    let rules = rules.as_deref();
    let events: Vec<&WeaponEvent> = events.read().filter(|e| e.shown()).collect();
    for (e, inventory, health, view, dice, zoomed, holstered, variant, own) in &mut characters {
        if own.is_some_and(|o| o.0) {
            continue;
        }
        let (Some(mut view), Some(mut dice)) = (view, dice) else {
            commands
                .entity(e)
                .insert((ViewAnimator::default(), ViewModelPlay::new(e)));
            continue;
        };
        let alive = health.is_none_or(|h| h.current > 0.0);
        let parts = inventory.active.filter(|_| alive).and_then(|w| weapons.get(w).ok());
        let weapon = parts.map(|(w, ..)| w.id);
        let variant = variant.map(|v| v.0.as_str());
        // The holder's variant of it, when the map has one.
        let shown = weapon.map(|w| {
            variant
                .map(|v| variant_key(w, v))
                .filter(|k| models.as_ref().is_some_and(|m| m.get(k).is_some()))
                .unwrap_or_else(|| w.to_string())
        });
        // Snipers hide the view model behind the scope.
        view.hidden = holstered || zoomed.is_some_and(|z| z.scope);
        let state = parts.map(|(w, alt, burst, magazine, throwable, (ws, shells, pose))| {
            let mode = alt.map_or(0, |m| m.current);
            ViewState {
                weapon: w.id,
                variant,
                data: catalog.as_ref().and_then(|c| c.get(w.id)),
                mode,
                burst: burst.filter(|b| b.mode == mode),
                clip: magazine.map(|m| m.clip),
                primed: throwable.is_some_and(|t| t.pin),
                shells: shells.is_some(),
                reloading: ws.is_some_and(|s| s.reloading()),
                zoomed,
                shots: ws.map_or(0, |s| s.burst),
                pose: pose.map_or(0, |p| p.0),
            }
        });
        let by_data = state.as_ref().is_some_and(|s| s.data.is_some_and(|d| !d.anim.sequences.is_empty()));
        // The held weapon's game's rules.
        let rules = weapon.and_then(|w| rules?.for_weapon(w)).copied();
        let effect = rules.map_or(no_effect as fn(&AnimEvent) -> _, |r| r.effect);
        let draw = |s: &ViewState| {
            if by_data {
                data_pick(s.data, "draw").or_else(|| data_pick(s.data, "idle"))
            } else {
                rules.map(|r| (r.draw)(s))
            }
        };
        let idle = |s: &ViewState| {
            if by_data {
                data_pick(s.data, "idle")
            } else {
                rules.map(|r| (r.idle)(s))
            }
        };
        // A new weapon in hand draws it.
        let mut drawn = false;
        if view.show(shown.as_deref(), models.as_deref()) {
            if let Some(p) = state.as_ref().and_then(draw) {
                drawn = play_pick(&mut view, &mut dice, &p, now);
            }
            dice.draw_held_until = f64::MIN;
            if drawn && !by_data {
                draw_rate(&mut view, &mut dice, rules.map_or(1.0, |r| r.draw_rate), now);
            }
            dice.drawn_at = now;
            dice.attacked = None;
            dice.zoomed = zoomed.is_some();
        }
        let Some(state) = state else { continue };
        // Where the sequence was, to fire the events passed this frame.
        let before = view.animator.as_ref().and_then(|a| Some((a.main?, a.cycle)));
        let mut frame = ViewFrame {
            owner: e,
            state,
            view: &mut view,
            dice: &mut dice,
            now,
            drawn,
            fired: false,
            restarted: drawn,
        };
        for ev in events.iter().filter(|ev| ev.owner == e) {
            let restarted_before = frame.restarted;
            let seq_before = frame.view.animator.as_ref().and_then(|a| a.main);
            match &ev.kind {
                WeaponEventKind::Deployed => {
                    if !frame.drawn {
                        if let Some(p) = draw(&frame.state) {
                            frame.drawn = frame.play(&p);
                            if frame.drawn && !by_data {
                                draw_rate(frame.view, frame.dice, rules.map_or(1.0, |r| r.draw_rate), now);
                            }
                        }
                        frame.dice.drawn_at = now;
                        frame.dice.attacked = None;
                    }
                }
                kind if by_data => data_event(&mut frame, kind),
                kind => {
                    if let Some(r) = rules {
                        (r.event)(&mut frame, kind);
                    }
                }
            }
            // What this event started: a reload's sequence, or not.
            let main = frame.view.animator.as_ref().and_then(|a| a.main);
            if frame.restarted && (!restarted_before || main != seq_before) {
                let reload = matches!(
                    ev.kind,
                    WeaponEventKind::ReloadStarted | WeaponEventKind::ShellInserting | WeaponEventKind::Reloaded
                );
                frame.dice.reload_seq = main.filter(|_| reload);
            }
        }
        // Zooming in and out (data actions).
        let is_zoomed = frame.state.zoomed.is_some();
        if by_data && is_zoomed != frame.dice.zoomed {
            if let Some(p) = data_pick(frame.state.data, if is_zoomed { "zoom_in" } else { "zoom_out" }) {
                frame.attack(&p);
            }
        }
        frame.dice.zoomed = is_zoomed;
        let restarted = frame.restarted;
        let state = frame.state;
        let Some(animator) = view.animator.as_mut() else {
            continue;
        };
        animator.advance(dt, now);
        // Events the cycle passed: a restart re-arms them, so those at
        // cycle 0 fire on its first frame.
        if let Some(s) = animator.main {
            let from = before.filter(|(m, _)| *m == s && !restarted).map(|(_, c)| c);
            let reload = dice.reload_seq == Some(s);
            for ev in animator.set.sequences[s].events_between(from, animator.cycle) {
                if let Some(kind) = effect(ev) {
                    let others = rules
                        .and_then(|r| r.others_reload)
                        .filter(|_| reload)
                        .map(|lag| if ev.cycle <= 0.0 { 0.0 } else { lag });
                    effects.write(ViewModelEvent { owner: e, kind, others, reload });
                }
            }
        }
        // Idle once the sequence has played out and the weapon's idle delay
        // has passed; a non-looping idle starts over after its interval,
        // holding its last frame meanwhile.
        let Some(idle) = idle(&state) else { continue };
        let (delay, interval, after_draw) = state.data.map_or((0.0, 0.0, false), |d| {
            (d.anim.idle_after, d.anim.idle_interval, d.anim.idle_after_draw)
        });
        let since = if after_draw {
            dice.last_attack.max(dice.drawn_at)
        } else {
            dice.last_attack
        };
        let idling = animator.main.is_some_and(|m| idle.matches(&animator.set.sequences[m]));
        // A looping attack sequence (a held flame's) plays out one length
        // after the last attack: it never reaches its end, and kept looping
        // after the release (the Flame Thrower's view model shaking in
        // `ft_fire` until the next draw).
        let looped_out = !idling
            && dice.attacked.is_some_and(|a| {
            animator.main == Some(a)
                && animator.set.sequences.get(a).is_some_and(|q| q.looping)
                && now - dice.last_attack >= animator.set.duration(a) as f64
        });
        if (animator.finished() || looped_out)
            && !state.primed
            && !state.reloading
            && now - since >= delay as f64
            && now >= dice.draw_held_until
            && (!idling || now - dice.idle_started >= interval as f64)
            && play_pick(&mut view, &mut dice, &idle, now)
        {
            dice.idle_started = now;
        }
    }
}
