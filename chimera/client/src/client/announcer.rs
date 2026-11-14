//! The announcer's banner and voice, drawn by the shell for any game
//! (specs/combat_arms/kill_streaks.md A2-A6; the pack and the facts are
//! `chimera_core::announcer`): whatever the local player's game, HUD or
//! gun, the server's pack (`KillFacts::pack`) announces the local
//! player's enemy kills, so Ultimate Gun Game shows Combat Arms'
//! announcer to a CS:S soldier with a CS:S gun, and plain CS:S (no pack)
//! nothing.
//!
//! - **Which callout** (`requests`, A2, A6): one per kill, by priority:
//!   first kill, revenge, then at count 1 a headshot or groin shot, else
//!   the step of the multi-kill count. The skull or nut goes over the
//!   banner unless the shot is the event itself; at most one voice: the
//!   event's own, else the headshot's, else the groin shot's by the
//!   killer's body. A weapon skin's art and voices replace the pack's by
//!   key. Failing all of those, a plain kill (`plain_kill`, B2a: count 1,
//!   no flags) is the "KILL" / "KILLS" banner by the killer's match kill
//!   total (`KillFacts::total`), level 1, no top image, `one`'s voice only.
//! - **Digits** (`digit_slots`, B2a): a request's number N in up to three
//!   fixed slots (ones always, tens and hundreds as needed, thousands
//!   dropped), one image per digit; each slot is scaled about its own
//!   centre with its copy's scale and alpha, and shakes with the main
//!   copy. N = 0 draws none.
//! - **One banner** (`BannerPlayer`, A4): a request is dropped while a
//!   banner with a higher level shows; otherwise it replaces it at once.
//!   No queue. Every callout's voice plays whether its banner showed or
//!   not, overlapping (A6).
//! - **The animation** (`main_copy`, `echo_copy`, A5): slam-in from the
//!   pack's start size with an ease-in quartic, an echo copy growing and
//!   fading while the main copy shakes, hold, blow-out; scaled about the
//!   layout rectangle's centre (`rect_on_screen`), each axis stretched
//!   from the pack's reference screen.
//! - `chimera_announce <count> [first] [head|groin] [revenge] [female] [total=<n>]`
//!   shows a kill's callouts at once (a test, for screenshots).
//! - `AnnouncerView` is the state, headless too (tests read it); drawing
//!   (Bevy UI images, `<game>://` install files through the asset
//!   server) and the voices (`SoundAsset`s as local interface sounds) run
//!   where there is an asset server. A new map hides the banner.
//!
//! Known shortcuts: the plain kill's number is the server's count of the
//! killer's enemy kills for the map (`KillFacts::total`), not a scoreboard
//! entry (no training-round or Bounty Hunter exceptions, no scoreboard
//! updates); no VIP kill or ice crush events; the phases start on their
//! exact times (not a frame late, Quirk 1); no co-op digits ("N KILLS",
//! "COMBO"), side log (A7), lucky shot or the "+N Points" line. The voice is an interface sound at the
//! effects volume times 1 (A6): the mixer's master (`GlobalVolume`, which
//! each game's sound options set) with gains of 1, as the spec says. A
//! voice's audio entity carries `AnnouncerVoice` (an app's interface
//! sounds duck under it: `ui_sound`).

use std::{collections::BTreeMap, sync::Arc};

use bevy::{prelude::*, window::PrimaryWindow};
use chimera_core::announcer::{AnnouncerPack, AnnouncerPacks, Callout, KillAnnounced, KillFacts, Rect, SpecialShot, Timing};

use crate::{
    audio::{
        clip_asset::SoundAsset,
        live_sound::{Gains, LiveClip},
    },
    core::LocalPlayer,
    map::LoadedMapName,
};

pub struct AnnouncerPlugin;

impl Plugin for AnnouncerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnnouncerView>()
            .init_resource::<AnnouncerPacks>()
            .add_message::<KillAnnounced>()
            .add_systems(Update, announce);
        crate::console::ConsoleAppExt::console_command(
            app,
            "chimera:announce",
            "Announce a kill of yours as the server's pack would (a test): chimera_announce <multi-kill count> [first] [head|groin] [revenge] [female] [total=<kills>].",
            test_announce,
        );
        if app.world().contains_resource::<AssetServer>() {
            app.init_resource::<AnnouncerAssets>()
                .add_systems(
                    Update,
                    (
                        draw_banner.run_if(resource_exists::<Assets<Image>>),
                        play_voices.run_if(resource_exists::<Assets<SoundAsset>>),
                    )
                        .after(announce),
                );
        }
    }
}

/// A banner request and its voice (one callout of a kill).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub key: String,
    pub name: String,
    pub image: String,
    pub top: Option<String>,
    pub voice: Option<String>,
    pub level: i32,
    /// A number drawn in digits (the plain kill's total; None: none).
    pub number: Option<u32>,
}

/// A callout as requested, with a skin's replacements.
fn request(c: &Callout, female: bool, skin: Option<&chimera_core::announcer::Skin>) -> Request {
    let image = |key: &str, default: Option<&String>| {
        skin.and_then(|s| s.images.get(key)).or(default).cloned()
    };
    let voice_key = if female && c.voice_female.is_some() {
        format!("{}_female", c.key)
    } else {
        c.key.clone()
    };
    let voice_default = if female { c.voice_female.as_ref().or(c.voice.as_ref()) } else { c.voice.as_ref() };
    Request {
        key: c.key.clone(),
        name: c.name.clone(),
        image: image(&c.key, Some(&c.banner)).unwrap_or_default(),
        top: image(&format!("{}_top", c.key), c.top.as_ref()),
        voice: skin.and_then(|s| s.voices.get(&voice_key)).or(voice_default).cloned(),
        level: c.level,
        number: None,
    }
}

/// The digit slots a number shows (B2a), as (slot, digit): slot 0 the ones
/// (shown for any N > 0), 1 the tens (when the tens or hundreds digit is
/// not 0), 2 the hundreds (when not 0); thousands are dropped. N = 0
/// shows none.
pub fn digit_slots(n: u32) -> Vec<(usize, u8)> {
    let (ones, tens, hundreds) = ((n % 10) as u8, (n / 10 % 10) as u8, (n / 100 % 10) as u8);
    let mut out = Vec::new();
    if n > 0 {
        out.push((0, ones));
        if tens > 0 || hundreds > 0 {
            out.push((1, tens));
        }
        if hundreds > 0 {
            out.push((2, hundreds));
        }
    }
    out
}

/// The one callout of a kill (A2, A6): its single event by priority
/// (first kill, revenge, then at count 1 a headshot or groin shot, else
/// the step of the count; the pack lacking an event's callout passes to
/// the next), the banner with the skull or nut over it when that flag is
/// not the event itself (Quirk 7), and at most one voice: the event's
/// own, else the headshot's, else the groin shot's by the killer's body.
pub fn requests(pack: &AnnouncerPack, facts: &KillFacts, weapon: Option<&str>) -> Option<Request> {
    if facts.multi == 0 {
        return None;
    }
    let skin = weapon.and_then(|w| pack.skin(w));
    let female = facts.killer_female;
    let (head, groin) = (facts.shot == SpecialShot::Head, facts.shot == SpecialShot::Groin);
    // Headshot and groin shot are events only on a single, plain kill.
    let single_shot = || if head { pack.headshot.as_ref() } else if groin { pack.groin.as_ref() } else { None };
    let found = [
        (facts.first.then_some(pack.first_kill.as_ref()).flatten(), false),
        (facts.revenge.then_some(pack.revenge.as_ref()).flatten(), false),
        (if facts.multi == 1 { single_shot() } else { pack.step(facts.multi) }, facts.multi == 1),
    ]
    .into_iter()
    .find_map(|(c, consumed)| c.map(|c| (c, consumed)));
    let Some((event, consumed)) = found else {
        // B2a: no event and no flag at all is a plain kill.
        let plain = pack.plain_kill.as_ref()?;
        if facts.multi != 1 || facts.first || facts.revenge || head || groin {
            return None;
        }
        let one = facts.total <= 1;
        let mut r = request(if one { &plain.one } else { &plain.many }, female, skin);
        r.number = Some(facts.total);
        return Some(r);
    };
    let mut r = request(event, female, skin);
    let shot = match facts.shot {
        SpecialShot::Head => pack.headshot.as_ref(),
        SpecialShot::Groin => pack.groin.as_ref(),
        SpecialShot::None => None,
    }
    .map(|c| request(c, female, skin));
    if consumed {
        r.top = None;
    } else {
        r.top = shot.as_ref().and_then(|s| s.top.clone());
        if r.voice.is_none() {
            r.voice = shot.and_then(|s| s.voice);
        }
    }
    Some(r)
}

/// The banner showing: its request and when it started.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub request: Request,
    pub since: f64,
}

/// The one banner area (A4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BannerPlayer {
    pub shown: Option<Shown>,
}

impl BannerPlayer {
    /// Show `r` from `now` unless a banner of a higher level shows
    /// (equal levels replace). Whether it shows.
    pub fn request(&mut self, r: Request, now: f64) -> bool {
        if self.shown.as_ref().is_some_and(|s| s.request.level > r.level) {
            return false;
        }
        self.shown = Some(Shown { request: r, since: now });
        true
    }

    /// Hidden on the first update at or past the total time.
    pub fn update(&mut self, now: f64, timing: &Timing) {
        if self.shown.as_ref().is_some_and(|s| now - s.since >= timing.total as f64) {
            self.shown = None;
        }
    }

    /// The showing banner's time since its start.
    pub fn t(&self, now: f64) -> Option<f64> {
        self.shown.as_ref().map(|s| now - s.since)
    }
}

/// Alpha of a copy at scale `s` going from 1 (opaque) to `far`
/// (transparent), truncated.
fn alpha_at(s: f64, far: f64) -> u8 {
    if far <= 1.0 {
        return 255;
    }
    (255.0 - (s - 1.0) * 255.0 / (far - 1.0)).clamp(0.0, 255.0) as u8
}

/// The main copy at `t`: scale and alpha (A5).
pub fn main_copy(t: f64, tm: &Timing) -> (f64, u8) {
    let (s0, l) = (tm.start_size as f64, tm.last_size as f64);
    let (start, c, end, d) = (tm.start as f64, tm.creating as f64, tm.end as f64, tm.destroying as f64);
    if t >= end && t - end < d {
        let s = l - (l - 1.0) * ((d - (t - end)) / d).powi(4);
        return (s, alpha_at(s, l));
    }
    if t >= end + d {
        return (l, 0);
    }
    if t < start {
        return (s0, 0);
    }
    if t - start < c {
        let s = s0 - (s0 - 1.0) * ((t - start) / c).powi(4);
        return (s, alpha_at(s, s0));
    }
    (1.0, 255)
}

/// Whether `t` is in the echo phase (and the main copy shakes).
pub fn in_echo(t: f64, tm: &Timing) -> bool {
    let tau = t - (tm.start + tm.creating) as f64;
    (0.0..tm.blowing as f64).contains(&tau)
}

/// The echo copy at `t`: scale and alpha (0 outside its phase).
pub fn echo_copy(t: f64, tm: &Timing) -> (f64, u8) {
    if !in_echo(t, tm) {
        return (1.0, 0);
    }
    let (b, big) = (tm.blowing as f64, tm.blow_size as f64);
    let tau = t - (tm.start + tm.creating) as f64;
    let s = big - (big - 1.0) * ((b - tau) / b).powi(4);
    (s, alpha_at(s, big))
}

/// A layout rectangle on a `screen` (each axis stretched from the
/// `reference`), scaled by `scale` about its centre: x, y, width, height.
pub fn rect_on_screen(rect: Rect, reference: [f32; 2], screen: Vec2, scale: f64) -> [f64; 4] {
    let (sx, sy) = (screen.x as f64 / reference[0] as f64, screen.y as f64 / reference[1] as f64);
    let [x, y, w, h] = rect.map(|v| v as f64);
    let (cx, cy) = ((x + w / 2.0) * sx, (y + h / 2.0) * sy);
    let (w, h) = (w * sx * scale, h * sy * scale);
    [cx - w / 2.0, cy - h / 2.0, w, h]
}

/// The announcer's state: the pack it shows, the banner, the voices it
/// played (newest last, the last 32) and those to play.
#[derive(Resource, Default)]
pub struct AnnouncerView {
    pub pack: Option<Arc<AnnouncerPack>>,
    pub player: BannerPlayer,
    pub voices: Vec<String>,
    pub to_play: Vec<String>,
    map: Option<String>,
}

/// The local player's kills into banners and voices.
fn announce(
    mut kills: MessageReader<KillAnnounced>,
    packs: Res<AnnouncerPacks>,
    me: Query<Entity, With<LocalPlayer>>,
    map: Option<Res<LoadedMapName>>,
    time: Res<Time>,
    mut view: ResMut<AnnouncerView>,
) {
    let now = time.elapsed_secs_f64();
    let map_name = map.map(|m| m.0.clone());
    if view.map != map_name {
        view.map = map_name;
        view.player = BannerPlayer::default();
    }
    for k in kills.read() {
        if !me.contains(k.killer) {
            continue;
        }
        let Some(pack) = packs.get(&k.facts.pack).cloned() else { continue };
        if let Some(r) = requests(&pack, &k.facts, k.weapon) {
            if let Some(v) = &r.voice {
                view.voices.push(v.clone());
                view.to_play.push(v.clone());
            }
            view.player.request(r, now);
        }
        if view.voices.len() > 32 {
            let extra = view.voices.len() - 32;
            view.voices.drain(..extra);
        }
        view.pack = Some(pack);
    }
    if let Some(pack) = view.pack.clone() {
        view.player.update(now, &pack.timing);
    }
}

/// `chimera_announce`: a kill of the local player's with these facts,
/// in the server's pack (else the first registered).
fn test_announce(world: &mut World, args: &[String]) -> Result<Option<String>, String> {
    let me = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next()
        .ok_or("no local player")?;
    let pack = world
        .get_resource::<chimera_core::announcer::ActiveAnnouncer>()
        .and_then(|a| a.0.clone())
        .or_else(|| world.get_resource::<AnnouncerPacks>()?.0.first().cloned())
        .ok_or("no announcer pack")?;
    let multi = args.first().and_then(|a| a.parse().ok()).unwrap_or(1u8).max(1);
    let has = |w: &str| args.iter().any(|a| a.eq_ignore_ascii_case(w));
    let total = args.iter().find_map(|a| a.strip_prefix("total=")?.parse().ok()).unwrap_or(1u32);
    let facts = KillFacts {
        pack: pack.id.clone(),
        multi,
        first: has("first"),
        shot: if has("head") {
            SpecialShot::Head
        } else if has("groin") {
            SpecialShot::Groin
        } else {
            SpecialShot::None
        },
        revenge: has("revenge"),
        total,
        killer_female: has("female"),
    };
    world.write_message(KillAnnounced {
        victim: me,
        killer: me,
        weapon: None,
        facts,
    });
    Ok(None)
}

/// The pack's files as assets (loaded ahead, kept while it is shown), the
/// voices waiting for their files, and the banner's nodes.
#[derive(Resource, Default)]
struct AnnouncerAssets {
    pack: Option<String>,
    images: BTreeMap<String, Handle<Image>>,
    sounds: BTreeMap<String, Handle<SoundAsset>>,
    waiting: Vec<(Handle<SoundAsset>, f64)>,
    root: Option<Entity>,
    shake: u32,
}

impl AnnouncerAssets {
    /// Load every file of `pack` once it is the one shown.
    fn follow(&mut self, pack: &AnnouncerPack, server: &AssetServer, sounds: bool) {
        if self.pack.as_deref() == Some(pack.id.as_str()) {
            return;
        }
        let path = |f: &str| chimera_present::assets::install_path(&pack.game, f);
        let (images, voices) = pack.files();
        self.images = images.into_iter().map(|f| (f.clone(), server.load(path(&f)))).collect();
        self.sounds = if sounds {
            voices.into_iter().map(|f| (f.clone(), server.load(path(&f)))).collect()
        } else {
            BTreeMap::new()
        };
        self.pack = Some(pack.id.clone());
    }
}

/// What a banner image shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Banner,
    Top,
    /// A digit slot: 0 ones, 1 tens, 2 hundreds.
    Digit(usize),
}

/// One of the banner's images: the echo copy or the main one, the banner,
/// its top image or a digit.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct BannerImage {
    echo: bool,
    part: Part,
}

/// The banner on screen (A5).
fn draw_banner(
    view: Res<AnnouncerView>,
    server: Res<AssetServer>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut assets: ResMut<AnnouncerAssets>,
    mut images: Query<(&BannerImage, &mut Node, &mut ImageNode, &mut Visibility)>,
    sounds: Option<Res<Assets<SoundAsset>>>,
    mut commands: Commands,
) {
    if let Some(pack) = &view.pack {
        assets.follow(pack, &server, sounds.is_some());
    }
    if assets.root.is_none() {
        let root = commands
            .spawn((
                Name::new("announcer"),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
                GlobalZIndex(35),
            ))
            .id();
        // Main copy first, the echo on top (A5).
        let parts = [Part::Banner, Part::Top, Part::Digit(0), Part::Digit(1), Part::Digit(2)];
        for echo in [false, true] {
            for part in parts {
                commands.spawn((
                BannerImage { echo, part },
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ImageNode::default(),
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(root),
                ));
            }
        }
        assets.root = Some(root);
        return;
    }
    let now = time.elapsed_secs_f64();
    let shown = view.pack.as_ref().zip(view.player.shown.as_ref());
    let screen = window.single().map_or(Vec2::new(1024.0, 768.0), |w| w.size());
    // The shake: a fresh whole-pixel offset each frame (xorshift).
    let mut draw = || {
        let mut x = assets.shake.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        assets.shake = x;
        x
    };
    let shake = shown.map(|(p, _)| p.timing.shake.max(0)).unwrap_or(0);
    let span = (2 * shake + 1) as u32;
    let offset = Vec2::new(
        (draw() % span) as f32 - shake as f32,
        (draw() % span) as f32 - shake as f32,
    );
    for (part, mut node, mut image, mut vis) in &mut images {
        let Some((pack, s)) = shown else {
            *vis = Visibility::Hidden;
            continue;
        };
        let t = now - s.since;
        let digit_file = |slot: usize| {
            let digit = digit_slots(s.request.number?).into_iter().find(|(i, _)| *i == slot)?.1;
            Some(pack.plain_kill.as_ref()?.digit_file(digit))
        };
        let file = match part.part {
            Part::Top => s.request.top.clone(),
            Part::Banner => Some(s.request.image.clone()),
            Part::Digit(i) => digit_file(i),
        };
        let handle = file.and_then(|f| assets.images.get(&f));
        let (scale, alpha) = if part.echo { echo_copy(t, &pack.timing) } else { main_copy(t, &pack.timing) };
        let (Some(handle), true) = (handle, alpha > 0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let rect = match part.part {
            Part::Top => pack.layout.top,
            Part::Banner => pack.layout.banner,
            Part::Digit(i) => match &pack.plain_kill {
                Some(p) => p.digits[i],
                None => {
                    *vis = Visibility::Hidden;
                    continue;
                }
            },
        };
        let [x, y, w, h] = rect_on_screen(rect, pack.layout.reference, screen, scale);
        let shift = if !part.echo && in_echo(t, &pack.timing) { offset } else { Vec2::ZERO };
        node.left = px(x as f32 + shift.x);
        node.top = px(y as f32 + shift.y);
        node.width = px(w as f32);
        node.height = px(h as f32);
        if image.image != *handle {
            image.image = handle.clone();
        }
        image.color = Color::srgba(1.0, 1.0, 1.0, alpha as f32 / 255.0);
        *vis = Visibility::Inherited;
    }
}

/// The voices, as local interface sounds, once their files are loaded
/// (given up after 3 s).
/// An announcer voice playing (its audio entity, gone when it ends): the
/// `ui` bus ducks under it (`ui_sound`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AnnouncerVoice;

fn play_voices(
    mut view: ResMut<AnnouncerView>,
    time: Res<Time>,
    mut assets: ResMut<AnnouncerAssets>,
    sounds: Option<Res<Assets<SoundAsset>>>,
    clips: Option<ResMut<Assets<LiveClip>>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    for v in std::mem::take(&mut view.to_play) {
        if let Some(h) = assets.sounds.get(&v).cloned() {
            assets.waiting.push((h, now));
        }
    }
    let (Some(sounds), Some(mut clips)) = (sounds, clips) else { return };
    assets.waiting.retain(|(h, since)| {
        if let Some(sound) = sounds.get(h) {
            let gains = Arc::new(Gains::new(1.0, 1.0));
            let clip = clips.add(LiveClip::new(sound.0.clone(), gains, default()));
            commands.spawn((AudioPlayer(clip), PlaybackSettings::DESPAWN, AnnouncerVoice));
            return false;
        }
        now - since < 3.0
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK: &str = r#"
id = "stand_in"
name = "Stand-in"
game = "stand_in"
window_ms = 5000
revenge_after = 2
steps = [
    { count = 2, key = "double", name = "DOUBLE", banner = "double.img", voice = "double.snd", level = 5 },
    { count = 3, key = "multi", name = "MULTI", banner = "multi.img", voice = "multi.snd", level = 5 },
    { count = 4, key = "ultra", name = "ULTRA", banner = "ultra.img", voice = "ultra.snd", level = 5 },
    { count = 5, key = "fantastic", name = "FANTASTIC", banner = "fantastic.img", voice = "fantastic.snd", level = 5 },
    { count = 6, key = "unbelievable", name = "UNBELIEVABLE", banner = "unbel.img", voice = "unbel.snd", level = 5 },
    { count = 7, key = "unbelievable_plus", name = "UNBELIEVABLE+", banner = "unbel_plus.img", voice = "unbel.snd", level = 5 },
]
first_kill = { key = "first", name = "FIRST KILL", banner = "first.img", voice = "first.snd", level = 6 }
headshot = { key = "head", name = "HEADSHOT", banner = "head.img", top = "skull.img", voice = "head.snd", level = 5 }
groin = { key = "groin", name = "NUTSHOT", banner = "groin.img", top = "nut.img", voice = "groin_m.snd", voice_female = "groin_f.snd", level = 5 }
revenge = { key = "revenge", name = "REVENGE", banner = "revenge.img", level = 6 }
[plain_kill]
one = { key = "kill", name = "KILL", banner = "kill.img", voice = "kill.snd", level = 1 }
many = { key = "kills", name = "KILLS", banner = "kills.img", level = 1 }
digit_prefix = "num_"
digits = [[425.0, 175.0, 64.0, 128.0], [385.0, 175.0, 64.0, 128.0], [345.0, 175.0, 64.0, 128.0]]
[layout]
reference = [1024.0, 768.0]
banner = [256.0, 76.0, 512.0, 256.0]
top = [256.0, 93.0, 512.0, 256.0]
[timing]
total = 1.4
start = 0.0
creating = 0.1
blowing = 0.15
end = 1.2
destroying = 0.2
start_size = 5.0
blow_size = 3.0
last_size = 5.0
shake = 5
[[skins]]
name = "psy"
weapons = ["kit:psy_gun"]
images = { double = "double_psy.img", revenge = "revenge_psy.img", kill = "kill_psy.img" }
voices = { revenge = "revenge_psy.snd" }
[[skins]]
name = "robo"
weapons = ["kit:robo_gun"]
images = { head = "head_robo.img", head_top = "skull_robo.img" }
voices = { head = "head_robo.snd" }
[[skins]]
name = "phantom"
weapons = ["kit:phantom_gun"]
images = { head = "head_phantom.img" }
"#;

    fn pack() -> AnnouncerPack {
        AnnouncerPack::parse(PACK).unwrap()
    }

    fn facts(multi: u8) -> KillFacts {
        KillFacts {
            pack: "stand_in".into(),
            multi,
            ..default()
        }
    }

    fn plain(total: u32) -> KillFacts {
        KillFacts { total, ..facts(1) }
    }

    /// B2a, cases 1, 42-47, 53, 54: the plain kill is "KILL" / "KILLS" by
    /// the total at level 1 with the number, no top image; its voice only
    /// for "KILL" and only if the pack names one.
    #[test]
    fn the_plain_kill() {
        let p = pack();
        let r = requests(&p, &plain(1), None).unwrap();
        assert_eq!((r.name.as_str(), r.level, r.number, r.top.as_deref(), r.voice.as_deref()), ("KILL", 1, Some(1), None, Some("kill.snd")));
        let r = requests(&p, &plain(2), None).unwrap();
        assert_eq!((r.name.as_str(), r.number, r.voice.as_deref()), ("KILLS", Some(2), None));
        let r = requests(&p, &plain(0), None).unwrap();
        assert_eq!((r.name.as_str(), r.number), ("KILL", Some(0)), "N = 0: KILL, no digits");
        assert!(digit_slots(0).is_empty());
        // Not a plain kill: a flag or a step.
        for f in [
            KillFacts { first: true, ..plain(1) },
            KillFacts { revenge: true, ..plain(1) },
            KillFacts { shot: SpecialShot::Head, ..plain(1) },
            facts(2),
        ] {
            assert!(requests(&p, &f, None).is_none_or(|r| r.number.is_none()), "{f:?}");
        }
        // The head kill is HEADSHOT, no digits.
        assert_eq!(requests(&p, &KillFacts { shot: SpecialShot::Head, ..plain(1) }, None).unwrap().name, "HEADSHOT");
        // A skin replaces the image; the digits stay.
        let r = requests(&p, &plain(1), Some("kit:psy_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.number), ("kill_psy.img", Some(1)));
        // A pack without the banner announces no plain kills.
        let mut bare = pack();
        bare.plain_kill = None;
        assert!(requests(&bare, &plain(1), None).is_none());
    }

    /// Cases 26-28, 42-46: the digit slots.
    #[test]
    fn digit_slots_by_number() {
        assert_eq!(digit_slots(1), [(0, 1)]);
        assert_eq!(digit_slots(5), [(0, 5)]);
        assert_eq!(digit_slots(7), [(0, 7)]);
        assert_eq!(digit_slots(12), [(0, 2), (1, 1)]);
        assert_eq!(digit_slots(120), [(0, 0), (1, 2), (2, 1)]);
        assert_eq!(digit_slots(105), [(0, 5), (1, 0), (2, 1)]);
        assert_eq!(digit_slots(1234), [(0, 4), (1, 3), (2, 2)]);
        assert_eq!(digit_slots(1005), [(0, 5)], "hundreds and tens are both 0");
        assert_eq!(digit_slots(10), [(0, 0), (1, 1)]);
    }

    /// Case 49: at t = 0.05 the digit is scaled about its own slot's
    /// centre, not the banner's: the ones digit is a 304 x 608 rect
    /// centred on (457, 239); alpha 15 like the banner's.
    #[test]
    fn a_digit_scales_about_its_slot() {
        let p = pack();
        let tm = p.timing;
        let (s, a) = main_copy(0.05, &tm);
        let slot = p.plain_kill.as_ref().unwrap().digits[0];
        let [x, y, w, h] = rect_on_screen(slot, p.layout.reference, Vec2::new(1024.0, 768.0), s);
        assert!((w - 304.0).abs() < 1e-3 && (h - 608.0).abs() < 1e-3, "{w} {h}");
        assert!((x + w / 2.0 - 457.0).abs() < 1e-3 && (y + h / 2.0 - 239.0).abs() < 1e-3);
        assert_eq!(a, 15);
    }

    /// Case 51, 52: level 1 yields to a showing FIRST KILL and is replaced
    /// by a lucky shot; it replaces a level 0 pickup.
    #[test]
    fn the_plain_kill_is_level_one() {
        let mut b = BannerPlayer::default();
        b.request(req("FIRST KILL", 6), 0.0);
        assert!(!b.request(req("KILL", 1), 0.5));
        let mut b = BannerPlayer::default();
        b.request(req("HISEC", 0), 0.0);
        assert!(b.request(req("KILL", 1), 0.1));
        assert!(b.request(req("LUCKY", 11), 0.2));
    }

    #[test]
    fn pack_files_include_the_digits() {
        let (images, _) = pack().files();
        assert!(images.contains(&"num_0.dtx".to_string()) && images.contains(&"num_9.dtx".to_string()));
        assert!(images.contains(&"kill.img".to_string()) && images.contains(&"kills.img".to_string()));
    }

    fn names(r: &[Request]) -> Vec<&str> {
        r.iter().map(|r| r.name.as_str()).collect()
    }

    /// Cases 1, 2, 6: no step at 1, the steps 2 to 7, Unbelievable+ again
    /// above with Unbelievable's voice.
    #[test]
    fn steps_and_voices_by_count() {
        let p = pack();
        assert!(requests(&p, &facts(1), None).is_some_and(|r| r.level == 1), "count 1 is the plain kill, not a step");
        let got: Vec<(String, Option<String>)> = (2..=8)
            .map(|n| {
                let r = requests(&p, &facts(n), None).unwrap();
                (r.name.clone(), r.voice.clone())
            })
            .collect();
        let want = [
            ("DOUBLE", "double.snd"),
            ("MULTI", "multi.snd"),
            ("ULTRA", "ultra.snd"),
            ("FANTASTIC", "fantastic.snd"),
            ("UNBELIEVABLE", "unbel.snd"),
            ("UNBELIEVABLE+", "unbel.snd"),
            ("UNBELIEVABLE+", "unbel.snd"),
        ];
        assert_eq!(got, want.map(|(n, v)| (n.to_string(), Some(v.to_string()))));
        assert!(requests(&p, &KillFacts::default(), None).is_none(), "not an enemy kill");
    }

    fn shot(multi: u8, shot: SpecialShot) -> KillFacts {
        KillFacts { shot, ..facts(multi) }
    }

    fn summary(r: Option<Request>) -> (String, Option<String>, Option<String>) {
        let r = r.expect("a callout");
        (r.name, r.top, r.voice)
    }

    fn s(name: &str, top: Option<&str>, voice: Option<&str>) -> (String, Option<String>, Option<String>) {
        (name.into(), top.map(Into::into), voice.map(Into::into))
    }

    /// Cases 7-9: first kill, headshot and groin shot on a single kill
    /// (no top image: Quirk 7), the groin voice by the killer's body.
    #[test]
    fn single_kill_events() {
        let p = pack();
        let first = requests(&p, &KillFacts { first: true, multi: 2, ..facts(2) }, None);
        assert_eq!(summary(first), s("FIRST KILL", None, Some("first.snd")), "the step ignored");
        assert_eq!(summary(requests(&p, &shot(1, SpecialShot::Head), None)), s("HEADSHOT", None, Some("head.snd")));
        let groin = |female| requests(&p, &KillFacts { killer_female: female, ..shot(1, SpecialShot::Groin) }, None);
        assert_eq!(summary(groin(true)), s("NUTSHOT", None, Some("groin_f.snd")));
        assert_eq!(summary(groin(false)), s("NUTSHOT", None, Some("groin_m.snd")));
    }

    /// Cases 35-39: one event and at most one voice per kill; the skull
    /// or nut over a step, first kill or revenge; revenge falls back to
    /// the head voice.
    #[test]
    fn one_event_and_one_voice() {
        let p = pack();
        let head2 = requests(&p, &shot(2, SpecialShot::Head), None);
        assert_eq!(summary(head2), s("DOUBLE", Some("skull.img"), Some("double.snd")), "case 35");
        let first_head = KillFacts { first: true, ..shot(3, SpecialShot::Head) };
        assert_eq!(summary(requests(&p, &first_head, None)), s("FIRST KILL", Some("skull.img"), Some("first.snd")), "case 36");
        let revenge_head = KillFacts { revenge: true, ..shot(2, SpecialShot::Head) };
        let r = requests(&p, &revenge_head, None).unwrap();
        assert_eq!((r.name.as_str(), r.top.as_deref(), r.voice.as_deref(), r.level), ("REVENGE", Some("skull.img"), Some("head.snd"), 6), "case 37");
        let revenge = KillFacts { revenge: true, ..facts(1) };
        assert_eq!(summary(requests(&p, &revenge, None)), s("REVENGE", None, None), "case 38");
        let nut3 = requests(&p, &shot(3, SpecialShot::Groin), None);
        assert_eq!(summary(nut3), s("MULTI", Some("nut.img"), Some("multi.snd")), "case 39");
        // Revenge beats first kill? No: first kill comes first.
        let both = KillFacts { first: true, revenge: true, ..facts(1) };
        assert_eq!(requests(&p, &both, None).unwrap().name, "FIRST KILL");
    }

    /// Cases 8, 9, 29-31, 41: a skin's art and voices by key, the pack's
    /// for the keys it lacks; a skin's skull only over a step.
    #[test]
    fn skins() {
        let p = pack();
        let r = requests(&p, &facts(2), Some("kit:psy_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.voice.as_deref()), ("double_psy.img", Some("double.snd")));
        let r = requests(&p, &shot(1, SpecialShot::Head), Some("kit:robo_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.top.as_deref(), r.voice.as_deref()), ("head_robo.img", None, Some("head_robo.snd")));
        let r = requests(&p, &shot(2, SpecialShot::Head), Some("kit:robo_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.top.as_deref(), r.voice.as_deref()), ("double.img", Some("skull_robo.img"), Some("double.snd")));
        let r = requests(&p, &shot(1, SpecialShot::Head), Some("kit:phantom_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.top.as_deref(), r.voice.as_deref()), ("head_phantom.img", None, Some("head.snd")));
        let r = requests(&p, &shot(2, SpecialShot::Head), Some("kit:phantom_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.top.as_deref(), r.voice.as_deref()), ("double.img", Some("skull.img"), Some("double.snd")));
        let r = requests(&p, &KillFacts { revenge: true, ..facts(1) }, Some("kit:psy_gun")).unwrap();
        assert_eq!((r.image.as_str(), r.voice.as_deref()), ("revenge_psy.img", Some("revenge_psy.snd")), "case 41");
    }

    fn req(name: &str, level: i32) -> Request {
        Request {
            key: name.into(),
            name: name.into(),
            image: String::new(),
            top: None,
            voice: None,
            level,
            number: None,
        }
    }

    /// Cases 10, 17-21, 40: shown for 1.4 s; a lower level dropped, equal
    /// or higher replace at once; the last of a frame's requests shows.
    #[test]
    fn one_banner_by_level() {
        let tm = pack().timing;
        let mut b = BannerPlayer::default();
        assert!(b.request(req("DOUBLE", 5), 0.0));
        b.update(1.399, &tm);
        assert!(b.shown.is_some());
        b.update(1.4, &tm);
        assert!(b.shown.is_none(), "hidden at T");
        b.request(req("FIRST KILL", 6), 0.0);
        assert!(!b.request(req("DOUBLE", 5), 0.5), "dropped under a higher level");
        assert_eq!(b.shown.as_ref().unwrap().request.name, "FIRST KILL");
        b.update(1.39, &tm);
        assert!(b.shown.is_some(), "FIRST KILL plays on");
        let mut b = BannerPlayer::default();
        b.request(req("DOUBLE", 5), 0.0);
        assert!(b.request(req("MULTI", 5), 0.6));
        assert_eq!(b.t(0.6), Some(0.0), "restarted");
        let mut b = BannerPlayer::default();
        b.request(req("REVENGE", 6), 0.0);
        assert!(!b.request(req("DOUBLE", 5), 0.2), "case 19");
        assert!(b.request(req("LUCKY", 11), 0.3));
        let mut b = BannerPlayer::default();
        b.request(req("HEADSHOT", 5), 0.0);
        assert!(b.request(req("DOUBLE", 5), 0.4), "case 40: equal levels replace");
        let mut b = BannerPlayer::default();
        b.request(req("DOUBLE", 5), 0.0);
        b.request(req("MULTI", 5), 0.0);
        assert_eq!(b.shown.unwrap().request.name, "MULTI");
    }

    /// Cases 11-15: the copies' scale and alpha at sample times.
    #[test]
    fn animation_at_sample_times() {
        let tm = pack().timing;
        let (s, a) = main_copy(0.05, &tm);
        assert!((s - 4.75).abs() < 1e-4 && a == 15, "{s} {a}");
        assert_eq!(main_copy(0.5, &tm), (1.0, 255));
        let (s, a) = main_copy(1.3, &tm);
        assert!((s - 4.75).abs() < 1e-4 && a == 15, "{s} {a}");
        let (s, a) = echo_copy(0.1 + 0.075, &tm);
        assert!((s - 2.875).abs() < 1e-4 && a == 15, "{s} {a}");
        assert_eq!(echo_copy(0.05, &tm).1, 0);
        assert_eq!(echo_copy(0.5, &tm).1, 0);
        assert!(in_echo(0.2, &tm) && !in_echo(0.5, &tm) && !in_echo(0.05, &tm), "the shake's phase");
    }

    /// Cases 24, 25: the banner's rectangle stretched and scaled about
    /// its centre.
    #[test]
    fn banner_rect() {
        let l = pack().layout;
        let [x, y, w, h] = rect_on_screen(l.banner, l.reference, Vec2::new(1920.0, 1080.0), 1.0);
        assert!((x - 480.0).abs() < 1e-3 && (y - 106.875).abs() < 1e-3 && (w - 960.0).abs() < 1e-3 && (h - 360.0).abs() < 1e-3);
        let [x, y, w, h] = rect_on_screen(l.banner, l.reference, Vec2::new(1024.0, 768.0), 4.75);
        assert!((w - 2432.0).abs() < 1e-3 && (h - 1216.0).abs() < 1e-3);
        assert!((x + w / 2.0 - 512.0).abs() < 1e-3 && (y + h / 2.0 - 204.0).abs() < 1e-3);
    }
}
