//! Combat Arms' options screen (specs/combat_arms/options.md P, K, W, L):
//! a modal screen with four tabs over the menus, on the game's layouts
//! when the install has them (`popup_option.xml` and the four tab files,
//! `install::OPTION_FILES`), else on ours (`data/ui/options*.xml`, the
//! same control names, flat stand-ins). The screen is a view of
//! `game::options` (the profile, the settings, the bindings): each frame
//! the controls are set from it, and a click or a drag calls its flow
//! (`flow::edit`, `ok`, `cancel`, `default_tab`, `begin_capture`).
//!
//! - L: the frame (`popup_option`: the tab buttons `tab_graphic`,
//!   `tab_sound`, `tab_control`, `tab_other`, `button_ok`,
//!   `button_cancel`, `button_close`, `button_default`) and the four
//!   windows (`GraphicWnd`, `SoundWnd`, `ControlWnd`, `OtherWnd`) are five
//!   trees of one desk, 1:1, centred on the screen as one; a control is
//!   found by its name in whichever tree has it, and a name a layout
//!   lacks is skipped. `BUTTONS` says what each button does (the check
//!   and radio pairs set a variable to a value, the Custom block's click
//!   rules, the social switches, cycling a drop-down's entries).
//! - Sliders (`SLIDERS`, W): the slider widget's maths
//!   (`game::options::slider`) on the layout's track; a click on the
//!   track or a drag moves the thumb; showing the screen reads each
//!   stored value back through the thumb (Quirk 2).
//! - K3: a left-click on a binding row (`list_control`'s rows, by row
//!   control name, their `ValueText` the row's text) starts a capture;
//!   the frame's keyboard, mouse-button, wheel and motion events are fed
//!   to it (up to 8), the click that began it not among them.
//! - The tab files' windows show and hide with the tab, `detail` with
//!   Custom quality, the Sound tab's `general_window` and `voice_window`
//!   with their buttons, `spectatormode_wnd` for game masters.
//!
//! `CaOptionsStatePlugin` is the screen without a window (tests/ui/
//! options.rs); `CaOptionsPlugin` adds the drawing. The `options` console
//! command opens it (`game::options::flow`).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The install's five layouts were not read (the spec names only their
//!   files and controls): they are bound by those names, placed by their
//!   own positions (the frame's box centred as ours is, the windows with
//!   it), a slider's track from its children by position (left arrow,
//!   thumb, right arrow), a missing control skipped; the check-button
//!   states, the combo boxes and the scroll bars are not drawn (the
//!   combo boxes are cycled by a click on their button or text, the
//!   binding list scrolls with the wheel inside the Control window, with no
//!   bar), and the music list, the language
//!   drop-down and the block list's rows are not built (the list shows
//!   the names in one text).
//! - "Register" takes the name from `OptionsScreen::name_box` (set by
//!   whoever has an edit box); there is no message box yet.
//! - The Steam client's window-mode switches (`check_full`,
//!   `check_window`) are shown (the spec's Open question 6).
//! - Esc does nothing on the screen (the spec gives it no meaning there)
//!   and the key capture takes it, unbound, as the spec has it.

use std::collections::BTreeMap;

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::PrimaryWindow,
};

use super::{
    CaUi,
    desk::{self, Desk, Event},
    draw::{self, CaFaces},
    layout::{Kind, Layout, PxRect, Sound, xml},
    menu::MenuPointer,
};
use crate::{
    client::{binds, canvas::WidgetId, fonts::UiFonts},
    game::options::{
        CaAccount, CaOptions, CaSocial, CaVoice, RawEvent, Tab, bindings, flow, keys,
        slider::{self, Slider},
        social::{self, Notes},
        video::{self, CaMachine, Mode, Quality},
    },
};

/// Our layouts (the game's format).
pub const LAYOUTS: [&str; 5] = [
    include_str!("../../data/ui/options.xml"),
    include_str!("../../data/ui/options_graphic.xml"),
    include_str!("../../data/ui/options_sound.xml"),
    include_str!("../../data/ui/options_control.xml"),
    include_str!("../../data/ui/options_other.xml"),
];

/// The window of each tab (L), in tab order.
pub const WINDOWS: [(&str, Tab); 4] = [
    ("GraphicWnd", Tab::Graphic),
    ("SoundWnd", Tab::Sound),
    ("ControlWnd", Tab::Control),
    ("OtherWnd", Tab::Other),
];

/// The thumb's width in our sliders, px.
pub const THUMB_W: i32 = 10;

/// Drop-down cycles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    Resolution,
    AntiAlias,
    Microphone,
    Speaker,
}

/// The block list's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockOp {
    Register,
    Clear,
    Sort,
    CheckAll,
}

/// What a button does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Show(Tab),
    Accept,
    Cancel,
    Defaults,
    /// The Sound tab's sub-page: true the Voice one.
    Page(bool),
    /// A check or radio of a pair: the variable gets the value.
    Set(&'static str, i32),
    /// A Custom block switch (`flow::custom_click`).
    Custom(&'static str, i32),
    /// An Other-tab switch.
    Social(&'static str, bool),
    /// A switch that shows a fixed value and cannot be changed.
    Fixed(i32),
    Drop(Cycle),
    TestMic,
    Block(BlockOp),
}

use Op as O;

/// Every button of the five layouts that does something (L).
pub const BUTTONS: &[(&str, Op)] = &[
    ("tab_graphic", O::Show(Tab::Graphic)),
    ("tab_sound", O::Show(Tab::Sound)),
    ("tab_control", O::Show(Tab::Control)),
    ("tab_other", O::Show(Tab::Other)),
    ("button_ok", O::Accept),
    ("button_cancel", O::Cancel),
    ("button_close", O::Cancel),
    ("button_default", O::Defaults),
    ("button_general", O::Page(false)),
    ("button_voice", O::Page(true)),
    // Graphic
    ("check_full", O::Set("windowed", 0)),
    ("check_window", O::Set("windowed", 1)),
    ("drop_resolution", O::Drop(Cycle::Resolution)),
    ("check_16bit", O::Set("ScreenDepthSetting", 16)),
    ("check_32bit", O::Set("ScreenDepthSetting", 32)),
    ("drop_antialias", O::Drop(Cycle::AntiAlias)),
    ("check_low", O::Set("PerformanceSetting", 0)),
    ("check_mid", O::Set("PerformanceSetting", 1)),
    ("check_high", O::Set("PerformanceSetting", 2)),
    ("check_custom", O::Set("PerformanceSetting", 3)),
    ("check_emptycartridges_off", O::Set("ShellCasings", 0)),
    ("check_emptycartridges_on", O::Set("ShellCasings", 1)),
    ("check_viewflare_off", O::Set("Tracers", 0)),
    ("check_viewflare_on", O::Set("Tracers", 1)),
    ("check_shadow_off", O::Fixed(0)),
    ("check_shadow_low", O::Fixed(1)),
    ("check_shadow_high", O::Fixed(2)),
    ("check_anisotropic_off", O::Custom("anisotropic", 0)),
    ("check_anisotropic_on", O::Custom("anisotropic", 1)),
    ("check_trilinear_off", O::Custom("trilinear", 0)),
    ("check_trilinear_on", O::Custom("trilinear", 1)),
    ("check_envmap_off", O::Set("EnvironmentalDetail", 0)),
    ("check_envmap_low", O::Set("EnvironmentalDetail", 1)),
    ("check_envmap_high", O::Set("EnvironmentalDetail", 2)),
    ("check_chardetail_low", O::Fixed(0)),
    ("check_chardetail_mid", O::Fixed(1)),
    ("check_chardetail_high", O::Fixed(2)),
    ("check_shader_off", O::Custom("shader", 0)),
    ("check_shader_on", O::Custom("shader", 1)),
    ("check_bloom_off", O::Custom("bloom", 0)),
    ("check_bloom_on", O::Custom("bloom", 1)),
    ("check_fxlevel_low", O::Custom("effects", 0)),
    ("check_fxlevel_mid", O::Custom("effects", 1)),
    ("check_fxlevel_high", O::Custom("effects", 2)),
    ("check_blood_off", O::Set("Gore", 0)),
    ("check_blood_on", O::Set("Gore", 1)),
    // Sound
    ("bgm_on", O::Set("enablemusic", 1)),
    ("bgm_off", O::Set("enablemusic", 0)),
    ("bgm_rotation", O::Set("bgmrotation", 1)),
    ("bgm_random", O::Set("bgmrotation", 0)),
    ("sound_on", O::Set("enablesound", 1)),
    ("sound_off", O::Set("enablesound", 0)),
    ("voicechat_on", O::Set("EnableVoiceChat", 1)),
    ("voicechat_off", O::Set("EnableVoiceChat", 0)),
    ("gamesoundforvoice_on", O::Set("SoundAdjustVolume", 100)),
    ("gamesoundforvoice_off", O::Set("SoundAdjustVolume", 0)),
    ("btn_testmic", O::TestMic),
    ("mic_device_combo", O::Drop(Cycle::Microphone)),
    ("speaker_device_combo", O::Drop(Cycle::Speaker)),
    // Control
    ("InvertMouse_On", O::Set("MouseInvertY", 1)),
    ("InvertMouse_Off", O::Set("MouseInvertY", 0)),
    ("mapRotaion_On", O::Set("rotateradar", 1)),
    ("mapRotaion_Off", O::Set("rotateradar", 0)),
    // Other
    ("blockwhisper_on", O::Social("block_whispers", true)),
    ("blockwhisper_off", O::Social("block_whispers", false)),
    ("blockinvite_on", O::Social("block_invites", true)),
    ("blockinvite_off", O::Social("block_invites", false)),
    ("cursefilter_on", O::Social("chat_filter", true)),
    ("cursefilter_off", O::Social("chat_filter", false)),
    ("spectatormode_on", O::Social("spectator_mode", true)),
    ("spectatormode_off", O::Social("spectator_mode", false)),
    ("recivenote_on", O::Social("notes", true)),
    ("recivenote_off", O::Social("notes", false)),
    ("jobautopopup_on", O::Social("daily_job_popup", true)),
    ("jobautopopup_off", O::Social("daily_job_popup", false)),
    ("button_regist", O::Block(BlockOp::Register)),
    ("button_clear", O::Block(BlockOp::Clear)),
    ("sort_name", O::Block(BlockOp::Sort)),
    ("check_all", O::Block(BlockOp::CheckAll)),
];

/// The sliders (W): the track, its value text and the variable.
pub const SLIDERS: &[(&str, &str, &str)] = &[
    ("SliderBar_Mousesens", "SliderBar_value", "MouseSensitivity"),
    ("sliderbar_gamma", "sliderbar_value_gamma", "GammaR"),
    ("bgm_volume_sliderbar", "bgm_volume_value", "musicvolume"),
    ("sound_volume_sliderbar", "sound_volume_value", "soundvolume"),
    ("mic_volume_sliderbar", "mic_volume_value", "MicVolume"),
    ("speaker_volume_sliderbar", "speaker_volume_value", "SpeakerVolume"),
];

/// The drop-downs' texts.
pub const DROP_TEXTS: &[(&str, Cycle)] = &[
    ("droptext_resolution", Cycle::Resolution),
    ("droptext_antialiasing", Cycle::AntiAlias),
    ("droptext_microphone", Cycle::Microphone),
    ("droptext_speaker", Cycle::Speaker),
];

/// One wheel notch of the Control tab's scroll, px.
const CONTROL_SCROLL_STEP: i32 = 32;

/// Voice controls other than the on/off switch.
const VOICE_CONTROLS: &[&str] = &[
    "gamesoundforvoice_on",
    "gamesoundforvoice_off",
    "btn_testmic",
    "mic_volume_sliderbar",
    "mic_device_combo",
    "speaker_volume_sliderbar",
    "speaker_device_combo",
];

/// The modes offered when the renderer has not reported any (G2).
pub const COMMON_MODES: &[(u32, u32)] = &[(1920, 1080), (1600, 900), (1280, 720), (1024, 768), (800, 600)];

/// The anti-aliasing counts offered when the renderer cannot say (G2: the
/// list is then empty and disabled; ours is the usual ones).
pub const SAMPLE_COUNTS: &[i32] = &[0, 2, 4, 8];

/// The screen: its desk, its sub-page and sliders, and what happened.
#[derive(Resource, Clone, Debug)]
pub struct OptionsScreen {
    pub desk: Desk,
    /// The sheets: the frame, then the four windows.
    pub sheets: [usize; 5],
    /// Each sheet's first control and its position as the layout has it.
    pub roots: [(WidgetId, IVec2); 5],
    pub installed: bool,
    pub screen: IVec2,
    /// The Sound tab shows its Voice page.
    pub voice: bool,
    /// Sliders by variable, with the thumb as the screen holds it.
    pub sliders: BTreeMap<String, Slider>,
    /// A slider being dragged: its variable.
    pub drag: Option<String>,
    /// Buttons fired, by name; sounds played (for tests and the log).
    pub fired: Vec<String>,
    pub played: Vec<Sound>,
    /// The name the block list's Register button adds.
    pub name_box: Option<String>,
    /// The Control tab's scroll, px: its content moves up by this much
    /// inside its window (the rows would run past it otherwise).
    pub scroll: i32,
    /// The screen was open last frame.
    was_open: bool,
    /// The press that began a capture is not an event of it.
    skip_input: bool,
    last: Vec<super::layout::Draw>,
    /// Redraw.
    pub changed: bool,
}

impl OptionsScreen {
    pub fn new(ui: &CaUi) -> Self {
        let install = ui.install.as_ref().and_then(|i| i.options.clone());
        let installed = install.is_some();
        let layouts: Vec<Layout> = install.unwrap_or_else(|| {
            LAYOUTS
                .iter()
                .map(|t| xml::read(t).expect("data/ui/options*.xml"))
                .collect()
        });
        let mut desk = Desk::default();
        let mut sheets = [0; 5];
        let mut roots = [(0, IVec2::ZERO); 5];
        for (i, l) in layouts.into_iter().enumerate() {
            if !installed {
                l.check_fonts(&ui.fonts).expect("the options screen's fonts");
            }
            let first = l.tree.get(0).children.first().copied().unwrap_or(0);
            roots[i] = (first, l.tree.get(first).pos);
            let names = ["popup_option", "GraphicWnd", "SoundWnd", "ControlWnd", "OtherWnd"];
            sheets[i] = desk.add(names[i], l, true);
        }
        let mut s = Self {
            desk,
            sheets,
            roots,
            installed,
            screen: IVec2::new(1024, 768),
            voice: false,
            sliders: BTreeMap::new(),
            drag: None,
            fired: Vec::new(),
            played: Vec::new(),
            name_box: None,
            scroll: 0,
            was_open: false,
            skip_input: false,
            last: Vec::new(),
            changed: true,
        };
        s.place();
        s
    }

    /// Centre the frame's box on the screen and move the windows with it.
    pub fn place(&mut self) {
        let (id, base) = self.roots[0];
        let size = self.desk.sheets[self.sheets[0]].layout.tree.get(id).size;
        let at = crate::client::canvas::centre(self.screen, size);
        let offset = at - base;
        for i in 0..5 {
            let (id, base) = self.roots[i];
            self.desk.sheets[self.sheets[i]].layout.tree.get_mut(id).pos = base + offset;
        }
    }

    /// The Control tab's visible region (its window's box, screen px) and
    /// how far its content can scroll: the content's bottom below the
    /// region's, 0 when it fits.
    pub fn control_view(&self, ui: &CaUi) -> (PxRect, i32) {
        let l = self.layout(3);
        let mut clip = l.hit_rect(self.roots[3].0, &ui.ctx());
        // The rect carries the scroll as the hit offset; the window's own
        // place is without it.
        clip.y0 += self.scroll;
        clip.y1 += self.scroll;
        let bottom = l
            .draw(&ui.ctx())
            .iter()
            .map(|d| match d {
                super::layout::Draw::Quad(_, q) => q.cover.y1,
                super::layout::Draw::Text(t) => t.rect.y1,
            })
            .max()
            .unwrap_or(0);
        (clip, (bottom - clip.y1).max(0))
    }

    /// Scroll the Control tab by `delta` px (clamped); true when it moved.
    pub fn scroll_control(&mut self, ui: &CaUi, delta: i32) -> bool {
        let (_, max) = self.control_view(ui);
        let to = (self.scroll + delta).clamp(0, max);
        self.apply_scroll(to)
    }

    fn apply_scroll(&mut self, to: i32) -> bool {
        let moved = to != self.scroll;
        self.scroll = to;
        let sheet = self.sheets[3];
        self.desk.sheets[sheet].layout.hit_offset.y = -to;
        moved
    }

    pub fn layout(&self, i: usize) -> &Layout {
        &self.desk.sheets[self.sheets[i]].layout
    }

    /// A control by name: its sheet index (0 the frame, 1 to 4 the
    /// windows) and id.
    pub fn find(&self, name: &str) -> Option<(usize, WidgetId)> {
        (0..5).find_map(|i| self.layout(i).find(name).map(|id| (i, id)))
    }

    fn widget_mut(&mut self, name: &str) -> Option<&mut super::layout::Widget> {
        let (i, id) = self.find(name)?;
        Some(self.desk.sheets[self.sheets[i]].layout.widget_mut(id))
    }

    /// Set a text control's string.
    pub fn set_text(&mut self, name: &str, s: &str) {
        if let Some(w) = self.widget_mut(name)
            && let Kind::Text(t) = &mut w.kind
            && t.set.as_deref() != Some(s)
        {
            t.set = Some(s.to_string());
        }
    }

    pub fn text_of(&self, name: &str, ui: &CaUi) -> Option<String> {
        let (i, id) = self.find(name)?;
        let l = self.layout(i);
        l.widget(id).text().map(|_| l.string_of(id, &ui.strings))
    }

    pub fn set_selected(&mut self, name: &str, on: bool) {
        if let Some(w) = self.widget_mut(name) {
            w.selected = on;
        }
    }

    pub fn set_enabled(&mut self, name: &str, on: bool) {
        if let Some(w) = self.widget_mut(name) {
            w.enabled = on;
        }
    }

    pub fn enabled(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).widget(id).enabled)
    }

    pub fn selected(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).widget(id).selected)
    }

    pub fn set_shown(&mut self, name: &str, on: bool) {
        if let Some((i, id)) = self.find(name) {
            let n = self.desk.sheets[self.sheets[i]].layout.tree.get_mut(id);
            if n.show != on {
                n.show = on;
            }
        }
    }

    pub fn shown(&self, name: &str) -> Option<bool> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).tree.get(id).show)
    }

    /// A slider's track on its layout, px (W).
    pub fn track_of(&self, name: &str) -> Option<i32> {
        let (i, id) = self.find(name)?;
        let l = self.layout(i);
        let n = l.tree.get(id);
        let mut kids: Vec<_> = n.children.iter().map(|&c| l.tree.get(c)).collect();
        kids.sort_by_key(|k| k.pos.x);
        Some(if kids.len() >= 3 {
            slider::track_width(kids[0].pos.x, kids[kids.len() - 1].pos.x, kids[0].size.x, kids[1].size.x)
        } else {
            n.size.x - THUMB_W
        })
    }

    /// A slider's rect, screen px.
    pub fn rect_of(&self, name: &str, ui: &CaUi) -> Option<PxRect> {
        let (i, id) = self.find(name)?;
        Some(self.layout(i).hit_rect(id, &ui.ctx()))
    }

    /// A row's `ValueText` (K1): the text control under the row.
    fn row_text_mut(&mut self, row: &str) -> Option<&mut super::layout::Widget> {
        let (i, id) = self.find(row)?;
        let l = &mut self.desk.sheets[self.sheets[i]].layout;
        let t = l.tree.find_from(id, "ValueText")?;
        Some(l.widget_mut(t))
    }

    pub fn row_text(&self, row: &str, ui: &CaUi) -> Option<String> {
        let (i, id) = self.find(row)?;
        let l = self.layout(i);
        let t = l.tree.find_from(id, "ValueText")?;
        Some(l.string_of(t, &ui.strings))
    }
}

/// The op of a button by name.
pub fn op_of(name: &str) -> Option<Op> {
    BUTTONS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, o)| *o)
}

// ---------------------------------------------------------------------------
// What the controls show

fn value(world: &mut World, cvar: &str) -> f64 {
    flow::control_value(world, cvar).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

fn modes_of(o: &CaOptions, m: &CaMachine) -> Vec<Mode> {
    if o.modes.is_empty() {
        COMMON_MODES
            .iter()
            .flat_map(|&(w, h)| [32, 16].map(|depth| Mode { width: w, height: h, depth, format: 0, hardware_tnl: true }))
            .collect()
    } else {
        video::mode_list(&o.modes, m.hardware_tnl)
    }
}

/// The resolution drop-down's list (G2).
pub fn resolutions(o: &CaOptions, m: &CaMachine) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = Vec::new();
    for m in modes_of(o, m) {
        if !v.contains(&(m.width, m.height)) {
            v.push((m.width, m.height));
        }
    }
    v
}

/// The Resolution text: the string table's format (width and height).
pub fn resolution_text(w: u32, h: u32) -> String {
    format!("{w} x {h}")
}

/// An anti-aliasing entry's text: "NX" (G1), off for none.
pub fn antialias_text(n: i32) -> String {
    if n == 0 { "OFF".into() } else { format!("{n}X") }
}

/// Whether a button is selected, and enabled, from the world's state.
fn state_of(world: &mut World, op: Op, name: &str) -> (Option<bool>, bool) {
    let account = world.resource::<CaAccount>().clone();
    let machine = *world.resource::<CaMachine>();
    let voice = world.resource::<CaVoice>().clone();
    let custom = world.resource::<CaOptions>().custom;
    let social = world.resource::<CaOptions>().social_pending.clone().unwrap_or_else(|| world.resource::<CaSocial>().clone());
    let mut enabled = true;
    let selected = match op {
        O::Set(cvar, v) => {
            if cvar == "Gore" && !video::gore_allowed(account.low_violence, account.region, account.age) {
                enabled = false;
                Some(v == 0)
            } else {
                Some(value(world, cvar) as i32 == v)
            }
        }
        O::Custom(which, v) => {
            enabled = match which {
                "shader" => custom.shader_enabled(),
                "bloom" => custom.bloom_enabled(),
                "effects" => custom.effects_enabled(v),
                _ => true,
            };
            Some(match which {
                "anisotropic" => custom.anisotropic as i32 == v,
                "trilinear" => custom.trilinear as i32 == v,
                "shader" => custom.shader as i32 == v,
                "bloom" => custom.bloom as i32 == v,
                _ => custom.effects == v,
            })
        }
        O::Fixed(v) => {
            // Shadows are always shown off and character detail High, and
            // neither can be changed.
            enabled = false;
            let shown = if name.contains("chardetail") { 2 } else { 0 };
            Some(v == shown)
        }
        O::Social(which, on) => {
            if which == "notes" {
                enabled = social.notes != Notes::Unknown;
            }
            let now = match which {
                "block_whispers" => social.block_whispers,
                "block_invites" => social.block_invites,
                "chat_filter" => social.chat_filter,
                "spectator_mode" => social.spectator_mode,
                "daily_job_popup" => social.daily_job_popup,
                _ => social.notes == Notes::Friends,
            };
            Some(now == on)
        }
        _ => None,
    };
    if name == "check_16bit" {
        let o = world.resource::<CaOptions>();
        enabled = modes_of(o, &machine).iter().any(|m| m.depth == 16);
    }
    if VOICE_CONTROLS.contains(&name) || name.starts_with("voicechat") {
        let on = value(world, "EnableVoiceChat") != 0.0;
        enabled = voice.built && (name.starts_with("voicechat") || on);
    }
    (selected, enabled)
}

/// Set every control from the world (each frame while the screen shows).
pub fn sync(world: &mut World, screen: &mut OptionsScreen) {
    let tab = world.resource::<CaOptions>().tab;
    for (name, t) in WINDOWS {
        screen.set_shown(name, t == tab);
    }
    screen.set_shown("general_window", !screen.voice);
    screen.set_shown("voice_window", screen.voice);
    let custom_quality = value(world, "PerformanceSetting") as i32 == Quality::Custom.number();
    screen.set_shown("detail", custom_quality);
    let master = world.resource::<CaAccount>().game_master;
    screen.set_shown("spectatormode_wnd", master);
    // Buttons.
    for &(name, op) in BUTTONS {
        if screen.find(name).is_none() {
            continue;
        }
        let (selected, enabled) = state_of(world, op, name);
        if let Some(s) = selected {
            screen.set_selected(name, s);
        }
        screen.set_enabled(name, enabled);
    }
    screen.set_selected("tab_graphic", tab == Tab::Graphic);
    screen.set_selected("tab_sound", tab == Tab::Sound);
    screen.set_selected("tab_control", tab == Tab::Control);
    screen.set_selected("tab_other", tab == Tab::Other);
    screen.set_selected("button_general", !screen.voice);
    screen.set_selected("button_voice", screen.voice);
    // Sliders: the thumb follows the value unless it is being dragged.
    let voice = world.resource::<CaVoice>().clone();
    for &(bar, text, cvar) in SLIDERS {
        let shown = flow::value_to_slider(cvar, world);
        let on = !VOICE_CONTROLS.contains(&bar) || (voice.built && value(world, "EnableVoiceChat") != 0.0);
        screen.set_enabled(bar, on);
        let Some(track) = screen.track_of(bar) else { continue };
        let s = screen.sliders.entry(cvar.to_string()).or_insert_with(|| Slider::new(track));
        if s.track != track {
            *s = Slider::new(track);
            s.show(shown);
        } else if s.value() != shown && screen.drag.as_deref() != Some(cvar) {
            s.show(shown);
        }
        let v = s.value();
        screen.set_text(text, &v.to_string());
    }
    // Drop-downs.
    let (w, h) = (value(world, "ScreenWidthSetting") as u32, value(world, "ScreenHeightSetting") as u32);
    screen.set_text("droptext_resolution", &resolution_text(w, h));
    screen.set_text("droptext_antialiasing", &antialias_text(value(world, "AntiAliasFSOverSample") as i32));
    screen.set_text(
        "droptext_microphone",
        &social::device_text(&voice.microphones, value(world, "MicDeviceIndex") as i32, voice.ready),
    );
    screen.set_text(
        "droptext_speaker",
        &social::device_text(&voice.speakers, value(world, "SpeakerDeviceIndex") as i32, voice.ready),
    );
    // The binding rows (K2).
    {
        let o = world.resource::<CaOptions>();
        let empty = super::layout::Strings::default();
        let strings = world.get_resource::<CaUi>().map_or(&empty, |u| &u.strings);
        let names = o.key_names.resolved(|id| strings.resolve(id));
        let capturing = o.capture.as_ref().map(|c| c.action);
        let texts: Vec<(usize, String)> = bindings::ACTIONS
            .iter()
            .enumerate()
            .filter(|(_, a)| a.row.is_some())
            .map(|(i, _)| {
                let t = if capturing == Some(i) { String::new() } else { o.bindings.entries[i].row_text(&names) };
                (i, t)
            })
            .collect();
        for (i, t) in texts {
            let row = bindings::ACTIONS[i].row.unwrap();
            if let Some(w) = screen.row_text_mut(row)
                && let Kind::Text(look) = &mut w.kind
                && look.set.as_deref() != Some(&t)
            {
                look.set = Some(t);
            }
        }
    }
    // The block list's names, in one text.
    let list = world.resource::<CaOptions>().block_list.names.iter().map(|(n, c)| format!("[{}] {n}", if *c { "x" } else { " " })).collect::<Vec<_>>().join("   ");
    screen.set_text("item_name", &list);
}

// ---------------------------------------------------------------------------
// Input

/// What a click did on the screen's buttons.
fn do_op(world: &mut World, screen: &mut OptionsScreen, op: Op) {
    match op {
        O::Show(t) => flow::show_tab(world, t),
        O::Accept => {
            let _ = flow::ok(world);
        }
        O::Cancel => flow::cancel(world),
        O::Defaults => {
            let tab = world.resource::<CaOptions>().tab;
            flow::default_tab(world, tab);
            // The sliders read their values again.
            screen.sliders.clear();
        }
        O::Page(v) => screen.voice = v,
        O::Set(cvar, v) => flow::edit(world, cvar, &v.to_string()),
        O::Custom(which, v) => flow::custom_click(world, which, v),
        O::Social(which, on) => flow::edit_social(world, which, on),
        O::Fixed(_) | O::TestMic => {}
        O::Drop(c) => cycle(world, c),
        O::Block(b) => {
            let name = screen.name_box.take();
            let mut o = world.resource_mut::<CaOptions>();
            match b {
                BlockOp::Register => {
                    if let Some(n) = name {
                        let _ = o.block_list.register(&n);
                    }
                }
                BlockOp::Clear => {
                    o.block_list.clear_checked();
                }
                BlockOp::Sort => o.block_list.sort(),
                BlockOp::CheckAll => {
                    let all = o.block_list.names.iter().all(|(_, c)| *c);
                    o.block_list.check_all(!all);
                }
            }
        }
    }
}

/// A drop-down's next entry.
fn cycle(world: &mut World, c: Cycle) {
    match c {
        Cycle::Resolution => {
            let list = {
                let o = world.resource::<CaOptions>();
                let m = *world.resource::<CaMachine>();
                resolutions(o, &m)
            };
            if list.is_empty() {
                return;
            }
            let cur = (value(world, "ScreenWidthSetting") as u32, value(world, "ScreenHeightSetting") as u32);
            let next = list[list.iter().position(|e| *e == cur).map_or(0, |i| (i + 1) % list.len())];
            flow::edit(world, "ScreenWidthSetting", &next.0.to_string());
            flow::edit(world, "ScreenHeightSetting", &next.1.to_string());
        }
        Cycle::AntiAlias => {
            let cur = value(world, "AntiAliasFSOverSample") as i32;
            let i = SAMPLE_COUNTS.iter().position(|&n| n == cur).map_or(0, |i| (i + 1) % SAMPLE_COUNTS.len());
            flow::edit(world, "AntiAliasFSOverSample", &SAMPLE_COUNTS[i].to_string());
        }
        Cycle::Microphone | Cycle::Speaker => {
            let voice = world.resource::<CaVoice>().clone();
            let (cvar, n) = if c == Cycle::Microphone { ("MicDeviceIndex", voice.microphones.len()) } else { ("SpeakerDeviceIndex", voice.speakers.len()) };
            if voice.built && n > 0 {
                let cur = value(world, cvar) as usize;
                flow::edit(world, cvar, &((cur + 1) % n).to_string());
            }
        }
    }
}

/// The slider under the pointer, if any: its variable and the offset of
/// the pointer from the track's start.
fn slider_at(screen: &OptionsScreen, ui: &CaUi, p: IVec2) -> Option<(String, i32)> {
    for &(bar, _, cvar) in SLIDERS.iter().rev() {
        let Some((i, id)) = screen.find(bar) else { continue };
        let l = screen.layout(i);
        if !l.tree.visible(id) || screen.enabled(bar) == Some(false) {
            continue;
        }
        let r = l.hit_rect(id, &ui.ctx());
        if r.contains(p) {
            return Some((cvar.to_string(), p.x - r.x0));
        }
    }
    None
}

fn move_slider(world: &mut World, screen: &mut OptionsScreen, cvar: &str, x: i32) {
    let Some(s) = screen.sliders.get_mut(cvar) else { return };
    let v = s.drag(x);
    flow::edit_slider(world, cvar, v);
}

/// The frame's events from the pointer (a press, a release, a move).
fn pointer(world: &mut World, screen: &mut OptionsScreen, ui: &CaUi) {
    let Some(p) = world.resource::<MenuPointer>().0 else { return };
    let (down, up, held) = {
        let m = world.resource::<ButtonInput<MouseButton>>();
        (m.just_pressed(MouseButton::Left), m.just_released(MouseButton::Left), m.pressed(MouseButton::Left))
    };
    let ctx = ui.ctx();
    let before: Vec<_> = (0..5).map(|i| states(screen.layout(i))).collect();
    // A scrolled-off row outside the Control window is not under the
    // pointer: its sheet hits nothing there.
    let hidden = world.resource::<CaOptions>().tab == Tab::Control && !screen.control_view(ui).0.contains(p);
    let sheet = screen.sheets[3];
    let kept = screen.desk.sheets[sheet].layout.hit_offset;
    if hidden {
        screen.desk.sheets[sheet].layout.hit_offset = IVec2::splat(-100_000);
    }
    screen.desk.pointer_moved(p, &ctx);
    if down {
        // A slider takes the press before a button under it.
        if let Some((cvar, x)) = slider_at(screen, ui, p) {
            screen.drag = Some(cvar.clone());
            move_slider(world, screen, &cvar, x);
        } else {
            screen.desk.left_down(p, &ctx);
        }
    } else if held && let Some(cvar) = screen.drag.clone() {
        let bar = SLIDERS.iter().find(|(_, _, c)| *c == cvar).map(|(b, ..)| *b);
        if let Some(r) = bar.and_then(|b| screen.rect_of(b, ui)) {
            move_slider(world, screen, &cvar, p.x - r.x0);
        }
    }
    if up {
        screen.desk.left_up();
        screen.drag = None;
    }
    screen.desk.sheets[sheet].layout.hit_offset = kept;
    let after: Vec<_> = (0..5).map(|i| states(screen.layout(i))).collect();
    if before != after {
        screen.changed = true;
    }
}

fn states(l: &Layout) -> Vec<(bool, bool)> {
    l.tree.order().into_iter().map(|i| (l.widget(i).hover, l.widget(i).held)).collect()
}

/// What the desk's events do.
fn act(world: &mut World, screen: &mut OptionsScreen) {
    if screen.desk.events.is_empty() {
        return;
    }
    let events = screen.desk.take_events();
    screen.played.extend(desk::sounds(&events));
    for e in events {
        let Event::Fired { name, .. } = e else { continue };
        screen.fired.push(name.clone());
        if let Some(op) = op_of(&name) {
            do_op(world, screen, op);
        } else if let Some(row) = bindings::by_row(&name) {
            // K3: a left-click on a row starts a capture (not while one
            // waits); the click itself is not one of its events.
            if flow::begin_capture(world, row) {
                screen.skip_input = true;
            }
        }
        screen.changed = true;
    }
}

/// The frame's raw input for a capture (K3): keys, buttons, the wheel
/// and motion; up to eight.
fn capture_events(world: &World) -> Vec<RawEvent> {
    let mut out = Vec::new();
    let keys_in = world.resource::<ButtonInput<KeyCode>>();
    for (code, name) in binds::KEY_NAMES {
        let Some(n) = keys::keyboard_token(name).and_then(|t| keys::number(&t)) else { continue };
        if keys_in.just_pressed(*code) {
            out.push(RawEvent::Key { code: n, down: true });
        }
        if keys_in.just_released(*code) {
            out.push(RawEvent::Key { code: n, down: false });
        }
    }
    // Escape is a key like the others for the capture (reserved).
    if keys_in.just_pressed(KeyCode::Escape) {
        out.push(RawEvent::Key { code: 1, down: true });
    }
    let mouse = world.resource::<ButtonInput<MouseButton>>();
    for (b, name) in binds::MOUSE_NAMES {
        let Some(n) = keys::mouse_token(name).and_then(|t| keys::number(&t)) else { continue };
        if mouse.just_pressed(*b) {
            out.push(RawEvent::Button { code: n, down: true });
        }
        if mouse.just_released(*b) {
            out.push(RawEvent::Button { code: n, down: false });
        }
    }
    if let Some(s) = world.get_resource::<AccumulatedMouseScroll>()
        && s.delta.y != 0.0
    {
        out.push(RawEvent::Wheel { up: s.delta.y > 0.0 });
    }
    if let Some(m) = world.get_resource::<AccumulatedMouseMotion>()
        && m.delta != Vec2::ZERO
    {
        out.push(RawEvent::MouseMove);
    }
    out.truncate(bindings::MAX_EVENTS);
    out
}

/// The screen's frame: shown and hidden with the options, its controls
/// set, its input.
pub fn update(world: &mut World) {
    if !world.contains_resource::<CaUi>() {
        return;
    }
    world.resource_scope(|world, mut screen: Mut<OptionsScreen>| {
        let screen = screen.bypass_change_detection();
        let open = world.resource::<CaOptions>().open;
        if open != screen.was_open {
            screen.was_open = open;
            screen.changed = true;
            for i in 0..5 {
                let s = screen.sheets[i];
                if open {
                    screen.desk.show(s, false);
                } else {
                    screen.desk.hide(s);
                }
            }
            if open {
                // Showing the screen reads every slider through its thumb
                // (Quirk 2), the Graphic tab first.
                screen.voice = false;
                screen.sliders.clear();
                for &(bar, _, cvar) in SLIDERS {
                    if let Some(track) = screen.track_of(bar) {
                        let s = flow::open_slider(world, cvar, track);
                        screen.sliders.insert(cvar.to_string(), s);
                    }
                }
            }
        }
        if !open {
            return;
        }
        // Cloned only while the screen is up: the install's string table
        // makes it milliseconds a frame (docs/performance.md, "Lucker
        // Party").
        let ui = world.resource::<CaUi>().clone();
        pointer(world, screen, &ui);
        act(world, screen);
        // A capture waiting takes the frame's input.
        if world.resource::<CaOptions>().capture.is_some() {
            if screen.skip_input {
                screen.skip_input = false;
            } else {
                let events = capture_events(world);
                if flow::capture_frame(world, &events) {
                    screen.changed = true;
                }
            }
        }
        // `flow::ok` and `cancel` close the screen; the next frame hides it.
        sync(world, screen);
        // The Control tab's scroll: the wheel over its window, kept inside
        // what the content overflows by.
        if world.resource::<CaOptions>().tab == Tab::Control {
            let (clip, max) = screen.control_view(&ui);
            let mut to = screen.scroll.clamp(0, max);
            if world.resource::<CaOptions>().capture.is_none()
                && let Some(w) = world.get_resource::<AccumulatedMouseScroll>()
                && w.delta.y != 0.0
                && world.resource::<MenuPointer>().0.is_some_and(|p| clip.contains(p))
            {
                to = (to - (w.delta.y.signum() * CONTROL_SCROLL_STEP as f32) as i32).clamp(0, max);
            }
            if screen.apply_scroll(to) {
                screen.changed = true;
            }
        }
        let draws = (0..5)
            .filter(|&i| screen.desk.sheets[screen.sheets[i]].shown)
            .flat_map(|i| screen.layout(i).draw(&ui.ctx()))
            .collect::<Vec<_>>();
        if draws != screen.last {
            screen.last = draws;
            screen.changed = true;
        }
    });
}

/// The screen without a window.
pub struct CaOptionsStatePlugin;

impl Plugin for CaOptionsStatePlugin {
    fn build(&self, app: &mut App) {
        let ui = app.world().get_resource::<CaUi>().cloned().unwrap_or_default();
        // The fire-mode key is a named action the weapon frame reads
        // (`game::data::FIRE_MODE_ACTION`), not a command to run.
        crate::client::binds::BindsAppExt::bound_action(app, crate::game::data::FIRE_MODE_ACTION);
        app.insert_resource(OptionsScreen::new(&ui))
            .insert_resource(ui)
            .init_resource::<MenuPointer>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, update.in_set(super::menu::MenuSystems));
    }
}

/// The screen with the window: the pointer, the drawing.
pub struct CaOptionsPlugin;

impl Plugin for CaOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CaOptionsStatePlugin)
            .add_systems(Startup, spawn)
            .add_systems(Update, window_size.before(super::menu::MenuSystems))
            .add_systems(Update, draw_screen.after(super::menu::MenuSystems));
    }
}

fn window_size(window: Query<&Window, With<PrimaryWindow>>, mut screen: ResMut<OptionsScreen>) {
    let Ok(w) = window.single() else { return };
    let size = IVec2::new(w.resolution.width() as i32, w.resolution.height() as i32);
    if size.x > 0 && size.y > 0 && screen.screen != size {
        screen.screen = size;
        screen.place();
        screen.changed = true;
    }
}

#[derive(Component)]
struct ScreenRoot;

fn spawn(mut commands: Commands) {
    commands.spawn((
        ScreenRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: percent(100.0),
            height: percent(100.0),
            ..default()
        },
        // Over the HUD and the in-game menu.
        GlobalZIndex(70),
        Visibility::Hidden,
    ));
}

#[allow(clippy::too_many_arguments)]
fn draw_screen(
    options: Res<CaOptions>,
    mut screen: ResMut<OptionsScreen>,
    ui: Res<CaUi>,
    fonts: Option<Res<UiFonts>>,
    faces: Option<Res<CaFaces>>,
    art: Option<Res<draw::CaArt>>,
    root: Single<(Entity, &mut Visibility), With<ScreenRoot>>,
    mut commands: Commands,
) {
    let redraw = screen.changed || faces.as_ref().is_some_and(|f| f.is_changed()) || art.as_ref().is_some_and(|a| a.is_changed());
    if !redraw {
        return;
    }
    screen.bypass_change_detection().changed = false;
    let (root, mut vis) = root.into_inner();
    *vis = if options.open { Visibility::Visible } else { Visibility::Hidden };
    commands.entity(root).despawn_related::<Children>();
    if !options.open {
        return;
    }
    let order = screen.desk.layers.order().to_vec();
    commands.entity(root).with_children(|p| {
        for sheet in order {
            let s = &screen.desk.sheets[sheet];
            if !s.shown {
                continue;
            }
            let draws = s.layout.draw(&ui.ctx());
            if sheet == screen.sheets[3] {
                // The Control window clips its content, scrolled.
                let (clip, _) = screen.control_view(&ui);
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(clip.x0 as f32),
                    top: px(clip.y0 as f32),
                    width: px((clip.x1 - clip.x0) as f32),
                    height: px((clip.y1 - clip.y0) as f32),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(Node {
                        position_type: PositionType::Absolute,
                        left: px(-clip.x0 as f32),
                        top: px((-clip.y0 - screen.scroll) as f32),
                        width: px(clip.x1 as f32 + 1.0),
                        height: px(100_000.0),
                        ..default()
                    })
                    .with_children(|c| draw::spawn_draws(c, &draws, &ui.stand_ins, art.as_deref(), fonts.as_deref(), faces.as_deref()));
                });
                continue;
            }
            draw::spawn_draws(p, &draws, &ui.stand_ins, art.as_deref(), fonts.as_deref(), faces.as_deref());
        }
        // The sliders: a track and a thumb over each (the widget has no
        // art of its own).
        for &(bar, _, cvar) in SLIDERS {
            let Some((i, id)) = screen.find(bar) else { continue };
            let l = screen.layout(i);
            if !l.tree.visible(id) {
                continue;
            }
            let r = l.hit_rect(id, &ui.ctx());
            if i == 3 {
                let (clip, _) = screen.control_view(&ui);
                if r.y0 < clip.y0 || r.y1 > clip.y1 {
                    continue;
                }
            }
            let thumb = screen.sliders.get(cvar).map_or(0, |s| s.thumb);
            let on = screen.enabled(bar) != Some(false);
            let bg = |x: i32, w: i32, c: Color| {
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(x as f32),
                        top: px(r.y0 as f32 + if w == THUMB_W { 0.0 } else { (r.y1 - r.y0) as f32 / 2.0 - 1.0 }),
                        width: px(w as f32),
                        height: px(if w == THUMB_W { (r.y1 - r.y0) as f32 } else { 2.0 }),
                        ..default()
                    },
                    BackgroundColor(c),
                )
            };
            p.spawn(bg(r.x0, r.x1 - r.x0, Color::srgb_u8(90, 98, 108)));
            let c = if on { Color::srgb_u8(255, 198, 0) } else { Color::srgb_u8(110, 110, 110) };
            p.spawn(bg(r.x0 + thumb, THUMB_W, c));
        }
    });
}
