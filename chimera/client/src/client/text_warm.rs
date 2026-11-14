//! Glyph atlases made ahead of the text that needs them: Bevy makes a glyph
//! atlas (a 512 x 512 RGBA image, 1 MB: 15.7 M instructions to fill) the
//! first time any text lays out a font at a size, and rasterizes each glyph
//! the first time it is drawn at it (about 0.3 M at 30 px), all on the main
//! thread in `text_system` on the frame the text first shows: 20-60 M
//! instructions for a HUD text at a size not shown before
//! (docs/performance.md, "Nothing blocks the main thread").
//!
//! Each HUD declares its fonts and sizes ahead (`TextWarmAppExt::
//! warm_texts`: a source, a system returning `WarmText`s, from its own
//! layout data and scheme fonts at the window's size), and every
//! text that exists but has laid out nothing yet (a hidden HUD's empty
//! label) counts too. Each new font and size is laid out once, with the
//! printable ASCII characters and the source's own (a game's weapon names),
//! by a hidden UI text off screen (`WarmNode`) that Bevy's own pipeline lays
//! out as any other, so the glyphs are the ones the real text finds (same
//! face, physical size, hinting, smoothing and target); the node goes once
//! laid out. Its atlas is not made on the frame: empty atlas images are
//! filled on a task ahead (`AtlasPool`), and one is put in Bevy's
//! `FontAtlasSet` under the key Bevy will look the font up by (`atlas_key`:
//! the font's data id, its physical size, no variations, the hinting and
//! smoothing) before the node lays out; a key that guessed wrong is counted
//! (`TextWarm::missed`) and its atlas dropped, and Bevy makes its own on
//! that node. A font whose turn comes while the pool is empty waits for
//! the task, frames that depend on the machine's load: what counts frames
//! (the tests) fills the pool first (`fill_pool`).
//!
//! When: behind the loading screen (`map::MapHooks::settle`, `settle`,
//! `GLYPHS_PER_SETTLE_FRAME` a frame), and in play a slice a frame
//! (`GLYPHS_PER_FRAME`; the command line's `--map` has no loading screen),
//! the sources asked again when the window, the fonts or the map's HUD
//! change and every `RECOLLECT_FRAMES` frames. A font and size is warmed once
//! per scale factor (`TextWarm::known`); Bevy keeps a font's atlases until
//! the font is dropped.
//!
//! Known shortcuts:
//! - Every font and size declared or waiting in an empty text gets its atlas,
//!   shown later or not: 1 MB in memory and 1 MB on the GPU each.
//! - Characters outside printable ASCII are warmed only when a source names
//!   them; a string with others makes their glyphs on its first frame (no
//!   new atlas while the atlas has room).
//! - The glyphs themselves are still rasterized on the main thread (on the
//!   warm nodes, a slice a frame); only the atlas images are made on a task.
//! - `atlas_key` repeats how Bevy 0.20 keys its atlases (`TextPipeline::
//!   update_text_layout_info`); a font with variations, a font collection's
//!   later face or a size not in pixels isn't guessed (Bevy makes those
//!   atlases on the warm node).

use std::collections::{HashSet, VecDeque};

use bevy::{
    asset::RenderAssetUsages,
    ecs::system::{RunSystemOnce, SystemId, SystemParam},
    image::{DynamicTextureAtlasBuilder, ImageSampler, TextureAtlasLayout},
    platform::{collections::HashMap, hash::FixedHasher},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
    text::{FontAtlas, FontAtlasKey, FontAtlasSet, FontHinting, FontSmoothing, TextLayoutInfo},
    ui::ComputedUiRenderTargetInfo,
    window::PrimaryWindow,
};

/// Glyphs laid out ahead per frame in play.
pub const GLYPHS_PER_FRAME: usize = 16;
/// Glyphs laid out ahead per frame behind the loading screen (with the
/// settling's other slices a frame stays under 33 ms: 256 made 10 ms of
/// rasterizing alone, docs/performance.md, "A map's swap in the real app").
pub const GLYPHS_PER_SETTLE_FRAME: usize = 128;
/// Frames between two askings of the sources in play when nothing changed.
pub const RECOLLECT_FRAMES: u32 = 120;
/// Frames a warm node waits for its layout before it goes anyway (no UI
/// camera).
const NODE_FRAMES: u8 = 4;
/// Bevy's glyph atlases' side (the smallest it makes) and the padding it
/// keeps around each glyph (`bevy_text::font_atlas`).
const ATLAS_SIDE: u32 = 512;
const ATLAS_PADDING: u32 = 2;
/// Empty atlases kept ready (`AtlasPool`).
const POOL: usize = 4;

/// The characters every font and size is warmed with: printable ASCII.
pub fn ascii() -> impl Iterator<Item = char> {
    (0x20u8..0x7f).map(char::from)
}

/// A font and size a HUD will draw, and the characters beyond printable
/// ASCII it will draw in it (a game's names).
#[derive(Clone, Debug)]
pub struct WarmText {
    pub font: TextFont,
    pub extra: String,
}

impl From<TextFont> for WarmText {
    fn from(font: TextFont) -> Self {
        Self { font, extra: String::new() }
    }
}

/// Where a HUD's fonts and sizes come from: a system run when the warm-up
/// asks, returning each font the HUD draws at the window's size.
pub type WarmSource = SystemId<(), Vec<WarmText>>;

/// A font and size waiting, the characters still to lay out, and whether
/// its atlas is still to be put in.
struct Job {
    font: TextFont,
    chars: Vec<char>,
    fresh: bool,
}

/// The text warm-up's sources and progress.
#[derive(Resource, Default)]
pub struct TextWarm {
    sources: Vec<WarmSource>,
    /// Fonts and sizes queued or warmed at `scale` (`key`).
    known: HashSet<String>,
    queue: VecDeque<Job>,
    scale: f32,
    /// The window's logical size when the sources were last asked.
    size: Vec2,
    since: u32,
    /// Fonts and sizes queued in this session.
    pub warmed: usize,
    /// Atlases put in ahead (from `AtlasPool`), and those Bevy didn't look
    /// up (a key guessed wrong: Bevy made its own).
    pub seeded: usize,
    pub missed: usize,
    /// Atlases Bevy made for a text other than a warm node's: fonts and
    /// sizes no HUD declared (each logged, `notice_unwarmed`).
    pub unwarmed: usize,
    /// The atlas keys seen.
    seen: HashSet<FontAtlasKey>,
}

impl TextWarm {
    /// Nothing queued.
    pub fn idle(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Empty glyph atlas images, filled on tasks ahead of the fonts that take
/// them: making one on the frame is 15.7 M instructions.
#[derive(Resource, Default)]
pub struct AtlasPool {
    ready: Vec<Image>,
    making: Vec<Task<Image>>,
}

impl AtlasPool {
    /// Images done taken in, and more started until `POOL` are ready or
    /// being made.
    fn tend(&mut self) {
        let ready = &mut self.ready;
        self.making.retain_mut(|task| match block_on(poll_once(task)) {
            Some(image) => {
                ready.push(image);
                false
            }
            None => true,
        });
        while self.ready.len() + self.making.len() < POOL {
            self.making.push(AsyncComputeTaskPool::get().spawn(async { empty_atlas() }));
        }
    }
}

/// Wait for the empty atlases being made (`AtlasPool`: `POOL` ready after
/// it). For what counts frames, not time (tests, offscreen runs): a font
/// waits a frame for each frame the pool's task hasn't run, as many as the
/// machine's load makes it, and then the frames to the warm-up's end are
/// the same on every run. Not for a frame in play: it is the 15.7 M
/// instructions the task is there to keep off it.
pub fn fill_pool(world: &mut World) {
    let mut pool = world.get_resource_or_insert_with(AtlasPool::default);
    pool.tend();
    for task in std::mem::take(&mut pool.making) {
        pool.ready.push(block_on(task));
    }
}

/// An atlas image as Bevy's `FontAtlas::new` makes it: transparent, kept
/// in the main world too (glyphs are added to it there).
fn empty_atlas() -> Image {
    Image::new_fill(
        Extent3d { width: ATLAS_SIDE, height: ATLAS_SIDE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// A hidden text laying out a font ahead, its frames out, and the atlas key
/// put in for it.
#[derive(Component)]
pub struct WarmNode(u8, Option<FontAtlasKey>);

/// Registering a HUD's fonts and sizes (`TextWarm`).
pub trait TextWarmAppExt {
    fn warm_texts<M>(&mut self, source: impl IntoSystem<(), Vec<WarmText>, M> + 'static) -> &mut Self;
}

impl TextWarmAppExt for App {
    fn warm_texts<M>(&mut self, source: impl IntoSystem<(), Vec<WarmText>, M> + 'static) -> &mut Self {
        self.init_resource::<TextWarm>();
        let id = self.register_system(source);
        self.world_mut().resource_mut::<TextWarm>().sources.push(id);
        self
    }
}

pub struct TextWarmPlugin;

impl Plugin for TextWarmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TextWarm>().init_resource::<AtlasPool>();
        app.init_resource::<crate::map::MapHooks>();
        app.world_mut().resource_mut::<crate::map::MapHooks>().settle.push(settle);
        app.add_systems(
            Update,
            in_play
                .run_if(resource_exists::<FontAtlasSet>)
                .run_if(resource_exists::<Assets<Image>>)
                .run_if(resource_exists::<Assets<Font>>),
        );
    }
}

/// The primary window (none headless).
pub type PrimaryWindows<'w, 's> = Query<'w, 's, &'static Window, With<PrimaryWindow>>;

/// The primary window's logical height (720 without one: headless).
pub fn window_height(window: &PrimaryWindows) -> f32 {
    window.iter().next().map_or(720.0, |w| w.height())
}

/// What sets a font's atlas: its face, its size (to the physical pixel),
/// weight, style and smoothing.
fn key(font: &TextFont, scale: f32) -> String {
    let size = match font.font_size {
        FontSize::Px(px) => format!("{}", px * scale),
        other => format!("{other:?}"),
    };
    format!(
        "{:?}|{size}|{:?}|{:?}|{:?}|{:?}",
        font.font, font.weight, font.width, font.style, font.font_smoothing
    )
}

/// The key Bevy's text pipeline files `font`'s glyphs under at `scale`,
/// when it can be told ahead: a face by handle (its data's id, its first
/// face, no variations), a size in pixels (times the scale, as Parley
/// sizes a run), UI text's hinting.
fn atlas_key(fonts: &Assets<Font>, font: &TextFont, scale: f32) -> Option<FontAtlasKey> {
    use std::hash::BuildHasher;
    let bevy::text::FontSource::Handle(handle) = &font.font else { return None };
    let FontSize::Px(px) = font.font_size else { return None };
    let data = fonts.get(handle)?;
    let coords: &[i16] = &[];
    Some(FontAtlasKey {
        id: data.data.id() as u32,
        index: 0,
        font_size_bits: (px * scale).to_bits(),
        variations_hash: FixedHasher.hash_one(coords),
        hinting: FontHinting::Enabled,
        font_smoothing: font.font_smoothing,
    })
}

/// The texts that exist but have laid out nothing yet (a hidden HUD's
/// empty label), and the UI's scale factor (a laid-out node's target's).
fn empty_texts(
    texts: Query<(&TextFont, Option<&TextLayoutInfo>), Without<WarmNode>>,
    targets: Query<&ComputedUiRenderTargetInfo>,
    window: PrimaryWindows,
    ui_scale: Option<Res<UiScale>>,
) -> (Vec<WarmText>, f32, Vec2) {
    let found = texts
        .iter()
        .filter(|(_, laid)| laid.is_none_or(|l| l.glyphs.is_empty()))
        .map(|(f, _)| f.clone().into())
        .collect();
    let w = window.iter().next();
    let scale = targets
        .iter()
        .map(|t| t.scale_factor())
        .find(|s| *s > 0.0)
        .unwrap_or_else(|| w.map_or(1.0, |w| w.scale_factor()) * ui_scale.map_or(1.0, |u| u.0));
    (found, scale, w.map_or(Vec2::ZERO, |w| w.size()))
}

/// Every source's fonts and the empty texts', the new ones queued.
fn collect(world: &mut World) {
    let Ok((mut found, scale, size)) = world.run_system_once(empty_texts) else { return };
    let sources = world.resource::<TextWarm>().sources.clone();
    for source in sources {
        if let Ok(more) = world.run_system(source) {
            found.extend(more);
        }
    }
    let mut warm = world.resource_mut::<TextWarm>();
    if (warm.scale - scale).abs() > 1e-4 {
        warm.known.clear();
        warm.queue.clear();
        warm.scale = scale;
    }
    warm.size = size;
    warm.since = 0;
    let before = warm.queue.len();
    for t in found {
        if warm.known.insert(key(&t.font, scale)) {
            let mut chars: Vec<char> = ascii().collect();
            for c in t.extra.chars() {
                if !c.is_control() && !chars.contains(&c) {
                    chars.push(c);
                }
            }
            warm.queue.push_back(Job { font: t.font, chars, fresh: true });
            warm.warmed += 1;
        }
    }
    let added = warm.queue.len() - before;
    if added > 0 {
        info!("text warm: {added} fonts and sizes to lay out ahead ({} in all)", warm.warmed);
    }
}

/// What laying out ahead reads and writes.
#[derive(SystemParam)]
struct Warming<'w, 's> {
    commands: Commands<'w, 's>,
    warm: ResMut<'w, TextWarm>,
    pool: ResMut<'w, AtlasPool>,
    set: ResMut<'w, FontAtlasSet>,
    images: ResMut<'w, Assets<Image>>,
    fonts: Res<'w, Assets<Font>>,
    nodes: Query<'w, 's, (Entity, &'static mut WarmNode, Option<&'static TextLayoutInfo>)>,
}

impl Warming<'_, '_> {
    /// Put an empty atlas from the pool in under `font`'s key, unless Bevy
    /// has one there. `None`: the pool is empty, wait; `Some(None)`: nothing
    /// to put in (the key can't be told, or Bevy has its atlas already).
    fn seed(&mut self, font: &TextFont) -> Option<Option<FontAtlasKey>> {
        let Some(key) = atlas_key(&self.fonts, font, self.warm.scale) else { return Some(None) };
        if self.set.get(&key).is_some_and(|a| !a.is_empty()) {
            return Some(None);
        }
        let mut image = self.pool.ready.pop()?;
        if font.font_smoothing == FontSmoothing::None {
            image.sampler = ImageSampler::nearest();
        }
        let size = UVec2::splat(ATLAS_SIDE);
        let atlas = FontAtlas {
            texture: self.images.add(image),
            texture_atlas: TextureAtlasLayout::new_empty(size),
            glyph_to_atlas_index: HashMap::default(),
            dynamic_texture_atlas_builder: DynamicTextureAtlasBuilder::new(size, ATLAS_PADDING),
        };
        self.set.entry(key).or_default().push(atlas);
        self.warm.seeded += 1;
        self.warm.seen.insert(key);
        Some(Some(key))
    }

    /// Atlases made since the last frame other than for warm nodes (whose
    /// glyphs are in none of them): a text showed a font and size no HUD
    /// declared (logged with its size, so a playtest's log names what is
    /// left; `TextWarm::unwarmed`). Not the app's first texts.
    fn notice_unwarmed(&mut self) {
        if self.set.len() == self.warm.seen.len() {
            return;
        }
        let ours: HashSet<AssetId<Image>> = self
            .nodes
            .iter()
            .filter_map(|(_, _, l)| l)
            .flat_map(|l| l.glyphs.iter().map(|g| g.atlas_info.texture))
            .collect();
        let first = self.warm.seen.is_empty();
        let new: Vec<FontAtlasKey> = self.set.keys().filter(|k| !self.warm.seen.contains(*k)).copied().collect();
        let theirs: Vec<f32> = new
            .iter()
            .filter(|k| !self.set[*k].iter().any(|a| ours.contains(&a.texture.id())))
            .map(|k| f32::from_bits(k.font_size_bits))
            .collect();
        self.warm.seen.extend(new);
        if first || theirs.is_empty() {
            return;
        }
        self.warm.unwarmed += theirs.len();
        info!(
            "text warm: {} glyph atlases made on a text's first frame (no HUD declared their font at {theirs:?} px)",
            theirs.len()
        );
    }

    /// Warm nodes laid out (or out too long) despawned, a seeded atlas Bevy
    /// didn't use dropped; then up to `budget` glyphs of the queue laid out
    /// by new ones.
    fn step(&mut self, budget: usize) {
        self.pool.tend();
        let mut gone = Vec::new();
        for (e, mut n, laid) in &mut self.nodes {
            n.0 += 1;
            if laid.is_some_and(|l| !l.glyphs.is_empty()) || n.0 > NODE_FRAMES {
                gone.push((e, n.1));
            }
        }
        for (e, seeded) in gone {
            self.commands.entity(e).despawn();
            let Some(key) = seeded else { continue };
            if self.set.get(&key).is_some_and(|a| a.iter().all(|a| a.glyph_to_atlas_index.is_empty())) {
                self.set.remove(&key);
                self.warm.missed += 1;
                debug!("text warm: Bevy filed a font's glyphs under another key than {key:?}");
            }
        }
        let mut left = budget;
        while left > 0 {
            let Some(job) = self.warm.queue.pop_front() else { break };
            let mut seeded = None;
            if job.fresh {
                match self.seed(&job.font) {
                    Some(k) => seeded = k,
                    None => {
                        // No empty atlas yet: this font waits for the pool.
                        self.warm.queue.push_front(job);
                        break;
                    }
                }
            }
            let Job { font, mut chars, .. } = job;
            let take = chars.len().min(left);
            let text: String = chars.drain(..take).collect();
            left -= take;
            if !chars.is_empty() {
                self.warm.queue.push_front(Job { font: font.clone(), chars, fresh: false });
            }
            self.commands.spawn((
                WarmNode(0, seeded),
                Text::new(text),
                font,
                TextLayout::no_wrap(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-100_000.0),
                    top: px(-100_000.0),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        }
    }
}

fn settle_step(mut w: Warming) -> bool {
    w.step(GLYPHS_PER_SETTLE_FRAME);
    w.warm.idle() && w.nodes.is_empty()
}

/// Behind the loading screen (`map::MapHooks::settle`): the sources asked,
/// then `GLYPHS_PER_SETTLE_FRAME` a frame until all are laid out.
pub fn settle(world: &mut World) -> bool {
    if !world.contains_resource::<FontAtlasSet>()
        || !world.contains_resource::<TextWarm>()
        || !world.contains_resource::<Assets<Image>>()
        || !world.contains_resource::<Assets<Font>>()
    {
        return true;
    }
    world.init_resource::<AtlasPool>();
    collect(world);
    world.run_system_once(settle_step).unwrap_or(true)
}

/// In play: the sources asked when the window, the fonts or the map's HUD
/// changed (and every `RECOLLECT_FRAMES`), then `GLYPHS_PER_FRAME` a frame.
fn in_play(
    mut w: Warming,
    window: PrimaryWindows,
    fonts: Option<Res<super::fonts::UiFonts>>,
    hud: Option<Res<crate::map::hud::ActiveHud>>,
) {
    let size = window.iter().next().map_or(Vec2::ZERO, |w| w.size());
    w.warm.since += 1;
    let changed = fonts.is_some_and(|f| f.is_changed()) || hud.is_some_and(|h| h.is_changed()) || w.warm.size != size;
    if changed || w.warm.since >= RECOLLECT_FRAMES {
        w.warm.since = 0;
        w.commands.queue(collect);
    }
    w.notice_unwarmed();
    w.step(GLYPHS_PER_FRAME);
}
