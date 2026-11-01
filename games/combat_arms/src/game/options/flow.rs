//! The profile's life and the options screen's flow, on the world
//! (specs/combat_arms/options.md F, P, K3): loading at game start, saving,
//! and what opening the screen, a control moving, OK, Cancel, Default (on
//! one tab) and capturing a key do. The screen (`ui::options`) calls
//! these; the `options` console command is the same for scripts and
//! tests.
//!
//! The settings' live values are the console variables; a screen edit
//! waits in `CaOptions::pending` for OK unless the setting applies at once
//! (a preview). Cancel returns to the profile as it was saved when the
//! screen opened, so it undoes previews and Default too.

use std::collections::BTreeMap;

use bevy::{app::AppExit, prelude::*};

use super::{
    CaOptions,
    bindings::{self, Capture, RawEvent},
    derive::CaHudSwitch,
    settings::{self, Applies, CaSettings, Setting, Tab, WRITE_ORDER},
    social::{CaAccount, CaSocial, Notes},
    video::{self, CaMachine, Quality},
};
use crate::{
    console::{Console, ConsoleAppExt},
    profile::{self, Profile},
};

// ---------------------------------------------------------------------------
// Helpers

fn with_options<R>(world: &mut World, f: impl FnOnce(&mut World, &mut CaOptions) -> R) -> R {
    world.resource_scope(|world, mut o: Mut<CaOptions>| f(world, &mut o))
}

/// Whether a match is running (the screen cannot open in one).
pub fn in_match(world: &World) -> bool {
    let playing = world.get_resource::<super::super::rules::CaRuleSettings>().is_some_and(|s| s.mode != "none");
    playing
        && world
            .get_resource::<super::super::rules::state::CaMatch>()
            .is_some_and(|m| !matches!(m.phase, super::super::rules::state::Phase::Waiting | super::super::rules::state::Phase::Starting))
}

/// A setting's current value as a number.
fn num(world: &mut World, o: &CaOptions, cvar: &str) -> f64 {
    settings::get(world, &o.table, cvar).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

/// A control's value: what it waits to write, else the live value.
pub fn control_value(world: &mut World, cvar: &str) -> Option<String> {
    let o = world.resource::<CaOptions>();
    if let Some(v) = o.pending.get(&cvar.to_lowercase()) {
        return Some(v.clone());
    }
    let table = o.table.clone();
    settings::get(world, &table, cvar)
}

// ---------------------------------------------------------------------------
// F2: loading

/// Load the profile at game start (the app's `Startup`): the file at
/// `CaOptions::path`, else the template.
pub fn load_at_start(world: &mut World) {
    let found = world.resource::<CaOptions>().path.as_deref().and_then(profile::read);
    let key_names = super::keys::read_installed(&world.get_resource_or_insert_with(super::super::data::CaData::default));
    world.resource_mut::<CaOptions>().key_names = key_names;
    load(world, found);
    // The shell's startup lines (`binddefaults`, `config.cfg`) run from the
    // console's queue after this: the player's binds go in once more
    // behind them.
    world.resource_mut::<Console>().submit("options applybinds");
}

/// Save the profile when the game exits (once). The exit message is only
/// looked at, never drained: the app's runner reads it after this, and a
/// drained one left `--screenshot` runs and `quit` running for ever.
pub fn save_at_exit(world: &mut World, mut saved: Local<bool>) {
    let exit = world.get_resource::<Messages<AppExit>>().is_some_and(|m| !m.is_empty());
    if exit && !*saved {
        *saved = true;
        let _ = save(world);
    }
}

/// F2: load the player profile `found` (None: not found). A version
/// different from the template's deletes it; not found makes the profile
/// from the template (first play). Then every setting is applied.
pub fn load(world: &mut World, found: Option<Profile>) {
    with_options(world, |world, o| {
        let default = o.default_profile.clone();
        let major = |p: &Profile| p.int("Version", "Major").unwrap_or(0);
        let mut first = false;
        let player = match found {
            Some(p) if default.has_section("Version") && major(&default) != major(&p) => {
                if let Some(path) = &o.path {
                    let _ = std::fs::remove_file(path);
                }
                None
            }
            other => other,
        };
        let profile = player.unwrap_or_else(|| {
            first = true;
            default.clone()
        });
        o.profile = profile;
        o.first_play = first;
        apply_profile(world, o, first);
    });
}

/// Apply a profile's values (F2 steps 3 to 5): sound, display, controls
/// (the bindings by K4), game options, then the performance block.
fn apply_profile(world: &mut World, o: &mut CaOptions, first: bool) {
    let machine = *world.resource::<CaMachine>();
    let table = o.table.clone();
    // Every setting from the profile (or its code default).
    for x in &table {
        if let Some(v) = x.from_profile(&o.profile) {
            settings::set(world, &table, &x.cvar, &v);
        }
    }
    // Machine limits: no music support forces the volume to 0.
    if !machine.music {
        settings::set(world, &table, "musicvolume", "0");
    }
    // A mode that isn't among the reported ones: the menus' (G2).
    if !o.modes.is_empty() {
        let list = video::mode_list(&o.modes, machine.hardware_tnl);
        let get = |w: &mut World, k: &str| settings::get(w, &table, k).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let stored = (get(world, "ScreenWidthSetting"), get(world, "ScreenHeightSetting"), get(world, "ScreenDepthSetting"));
        let (w, h, d) = video::load_mode(&list, stored);
        settings::set(world, &table, "ScreenWidthSetting", &w.to_string());
        settings::set(world, &table, "ScreenHeightSetting", &h.to_string());
        settings::set(world, &table, "ScreenDepthSetting", &d.to_string());
    }
    // The bindings (K4), applied to the console.
    o.bindings = bindings::load(&o.profile, &o.default_profile, first);
    if let Some(mut c) = world.get_resource_mut::<Console>() {
        bindings::apply(&mut c, &o.bindings);
    }
    // The performance block: from [Settings] if present (the rows were
    // read above), otherwise the console's current values; then the
    // preset is applied.
    apply_quality(world, o);
    enforce_gore(world, &table);
    o.custom = custom_of(world, o, machine);
    // RestoreDefaults: every section from the default profile.
    let restore = settings::get(world, &table, "RestoreDefaults").is_some_and(|v| v.trim() == "1");
    if restore {
        o.profile = o.default_profile.clone();
        apply_profile_once(world, o);
    }
}

/// The values of the default profile after a `RestoreDefaults`, without
/// looking at that variable again.
fn apply_profile_once(world: &mut World, o: &mut CaOptions) {
    let table = o.table.clone();
    for x in &table {
        if let Some(v) = x.from_profile(&o.profile) {
            settings::set(world, &table, &x.cvar, &v);
        }
    }
    o.bindings = bindings::load(&o.profile, &o.default_profile, true);
    if let Some(mut c) = world.get_resource_mut::<Console>() {
        bindings::apply(&mut c, &o.bindings);
    }
    apply_quality(world, o);
    settings::set(world, &table, "RestoreDefaults", "0");
}

/// G5: blood stays off where the build or the account's age says so (the
/// switch shows off and is disabled; OK writes `Gore` 0).
pub fn enforce_gore(world: &mut World, table: &[Setting]) {
    let a = world.resource::<CaAccount>();
    if !video::gore_allowed(a.low_violence, a.region, a.age) {
        settings::set(world, table, "Gore", "0");
    }
}

/// The Custom block's switches as the variables hold them.
fn custom_of(world: &mut World, o: &CaOptions, machine: CaMachine) -> video::Custom {
    let mut c = video::Custom::new(machine);
    c.anisotropic = num(world, o, "anisotropic") != 0.0;
    c.trilinear = num(world, o, "trilinear") != 0.0;
    c.shader = num(world, o, "Shader") != 0.0;
    c.bloom = num(world, o, "EnableBloom") != 0.0;
    c.effects = num(world, o, "FXDetail") as i32;
    c.enforced()
}

// ---------------------------------------------------------------------------
// G4: the quality apply

/// Apply the quality choice (`PerformanceSetting`): a preset writes its
/// column of G4's table; Custom keeps the rows it doesn't own and writes
/// the custom block (ShadowDetail always 0). The derived renderer values
/// follow (`Derived`).
pub fn apply_quality(world: &mut World, o: &mut CaOptions) {
    let machine = *world.resource::<CaMachine>();
    let table = o.table.clone();
    let q = Quality::of(num(world, o, "PerformanceSetting") as i32);
    for r in o.presets.clone() {
        match r.value(q) {
            Some(v) => settings::set(world, &table, &r.cvar, &v.to_string()),
            None if r.name == "ShadowDetail" => settings::set(world, &table, &r.cvar, "0"),
            None => {}
        }
    }
    // Machine limits: triple buffering and music off where unsupported.
    if !machine.triple_buffer {
        settings::set(world, &table, "triplebuffer", "0");
    }
    if !machine.music {
        settings::set(world, &table, "MusicActive", "0");
    }
    let n = |w: &mut World, k: &str| settings::get(w, &table, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as i32;
    let (shadow, fx, env, detail) = (n(world, "ShadowDetail"), n(world, "FXDetail"), n(world, "EnvironmentalDetail"), n(world, "DetailLevel"));
    o.derived = video::derive(shadow, fx, env, detail, q != Quality::Custom, &machine, &o.derived);
}

// ---------------------------------------------------------------------------
// F3: saving

/// Write the live values and the bindings into the in-memory profile
/// (F3): every section the profile manages; other lines stay.
pub fn sync_to_profile(world: &mut World, o: &mut CaOptions) {
    let table = o.table.clone();
    for x in &table {
        let Some(mut v) = settings::get(world, &table, &x.cvar) else { continue };
        if x.cvar.eq_ignore_ascii_case("musicvolume") {
            v = super::sound::music_stored(v.parse().unwrap_or(0)).to_string();
        }
        o.profile.set(&x.section, &x.key, &x.to_stored(&v));
    }
    for (key, v) in ["RegularTex", "LargeTex", "XLargeTex"].into_iter().zip(o.derived.texture_offsets) {
        o.profile.set("Details", key, &v.to_string());
    }
    o.bindings.store(&mut o.profile);
}

/// F3: save the profile: sync, then write the file when it has a path.
pub fn save_options(world: &mut World, o: &mut CaOptions) -> Result<(), String> {
    sync_to_profile(world, o);
    o.saves += 1;
    match &o.path {
        Some(p) => profile::write(p, &o.profile).map_err(|e| format!("{}: {e}", p.display())),
        None => Ok(()),
    }
}

/// Save the profile (the options screen opening, OK, the game's exit).
pub fn save(world: &mut World) -> Result<(), String> {
    with_options(world, save_options)
}

// ---------------------------------------------------------------------------
// P: the screen

/// Open the screen (P): refused during a match. The profile is saved
/// first, so Cancel can go back to exactly this state.
pub fn open(world: &mut World) -> Result<(), String> {
    open_with(world, false)
}

/// `open`, or with `force` in a match too (the `options force` command:
/// there is no lobby to leave the match for).
pub fn open_with(world: &mut World, force: bool) -> Result<(), String> {
    if !force && in_match(world) {
        return Err("the options can't be opened during a match".into());
    }
    with_options(world, |world, o| {
        if o.open {
            return Ok(());
        }
        save_options(world, o)?;
        o.saved = Some(o.profile.clone());
        o.pending.clear();
        o.capture = None;
        o.social_pending = None;
        o.tab = Tab::Graphic;
        o.open = true;
        let machine = *world.resource::<CaMachine>();
        o.custom = custom_of(world, o, machine);
        Ok(())
    })
}

/// Show a tab (the buttons `tab_graphic` and the rest).
pub fn show_tab(world: &mut World, tab: Tab) {
    let mut o = world.resource_mut::<CaOptions>();
    o.tab = tab;
    o.capture = None;
}

/// A control moved: `value` is the variable's text. It waits for OK
/// unless the setting applies at once.
pub fn edit(world: &mut World, cvar: &str, value: &str) {
    with_options(world, |world, o| {
        let Some(x) = o.setting(cvar).cloned() else { return };
        o.pending.insert(cvar.to_lowercase(), value.trim().to_string());
        if x.applies == Applies::Now {
            let table = o.table.clone();
            settings::set(world, &table, &x.cvar, value);
        }
    });
}

/// A slider's value as the variable's: the gamma slider's (G3) maps to
/// gamma; the others' are the variable's own.
pub fn slider_to_value(cvar: &str, slider: i32) -> String {
    if cvar.eq_ignore_ascii_case("GammaR") {
        format!("{}", video::gamma_of_slider(slider as f64) as f32)
    } else {
        slider.to_string()
    }
}

/// The slider value a variable shows.
pub fn value_to_slider(cvar: &str, world: &mut World) -> i32 {
    let v = control_value(world, cvar).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    if cvar.eq_ignore_ascii_case("GammaR") {
        video::slider_of_gamma(v)
    } else {
        v.round() as i32
    }
}

/// Show a variable on a slider of `track` px (W): the thumb goes to
/// `floor((v + 1) W / 100)` and the value read back is the control's
/// (Quirk 2: it can be one more), applied like any move.
pub fn open_slider(world: &mut World, cvar: &str, track: i32) -> super::slider::Slider {
    let mut s = super::slider::Slider::new(track);
    let shown = value_to_slider(cvar, world);
    let read = s.show(shown);
    edit_slider(world, cvar, read);
    s
}

/// A slider moved to `slider` (0 to 100).
pub fn edit_slider(world: &mut World, cvar: &str, slider: i32) {
    edit(world, cvar, &slider_to_value(cvar, slider));
}

/// A Custom switch was clicked (the rules between them, G4): `which` is
/// `anisotropic`, `trilinear`, `shader`, `bloom` or `effects` (0 to 2).
pub fn custom_click(world: &mut World, which: &str, value: i32) {
    let c = {
        let mut o = world.resource_mut::<CaOptions>();
        let c = &mut o.custom;
        match which {
            "anisotropic" => c.click_anisotropic(value != 0),
            "trilinear" => c.click_trilinear(value != 0),
            "shader" => c.click_shader(value != 0),
            "bloom" => c.click_bloom(value != 0),
            "effects" => c.click_effects(value),
            _ => {}
        }
        o.custom
    };
    for (cvar, v) in [
        ("anisotropic", c.anisotropic as i32),
        ("trilinear", c.trilinear as i32),
        ("Shader", c.shader as i32),
        ("EnableBloom", c.bloom as i32),
        ("FXDetail", c.effects),
    ] {
        edit(world, cvar, &v.to_string());
    }
}

/// The Other tab's edits: a switch by name (`block_whispers`,
/// `block_invites`, `chat_filter`, `spectator_mode`, `daily_job_popup`)
/// or the notes (`notes`: 1 friends, 0 nobody).
pub fn edit_social(world: &mut World, which: &str, on: bool) {
    let live = world.resource::<CaSocial>().clone();
    let mut o = world.resource_mut::<CaOptions>();
    let s = o.social_pending.get_or_insert(live);
    match which {
        "block_whispers" => s.block_whispers = on,
        "block_invites" => s.block_invites = on,
        "chat_filter" => s.chat_filter = on,
        "spectator_mode" => s.spectator_mode = on,
        "daily_job_popup" => s.daily_job_popup = on,
        "notes" => s.notes = if on { Notes::Friends } else { Notes::Nobody },
        _ => {}
    }
}

/// OK (P): every tab's pending values to the engine (Graphic, Sound,
/// Control, Other), the quality apply, the key bindings, the values into
/// the profile, the file saved, the screen closed.
pub fn ok(world: &mut World) -> Result<(), String> {
    with_options(world, |world, o| {
        if !o.open {
            return Ok(());
        }
        let table = o.table.clone();
        let mut pending = std::mem::take(&mut o.pending);
        for tab in WRITE_ORDER {
            let mine: Vec<&Setting> = table.iter().filter(|x| x.page == Some(tab)).collect();
            for x in mine {
                if let Some(v) = pending.remove(&x.cvar.to_lowercase()) {
                    settings::set(world, &table, &x.cvar, &v);
                }
            }
            match tab {
                Tab::Graphic => apply_quality(world, o),
                Tab::Other => {
                    if let Some(next) = o.social_pending.take() {
                        let mut live = world.resource_mut::<CaSocial>();
                        send_changes(&mut live, &next);
                    }
                }
                _ => {}
            }
        }
        // Whatever else waited (a setting without a page).
        for (k, v) in pending {
            settings::set(world, &table, &k, &v);
        }
        enforce_gore(world, &table);
        if let Some(mut c) = world.get_resource_mut::<Console>() {
            bindings::apply(&mut c, &o.bindings);
        }
        o.capture = None;
        let r = save_options(world, o);
        o.saved = None;
        o.open = false;
        r
    })
}

/// The service gets the changes (O): each switch that differs is sent.
fn send_changes(live: &mut CaSocial, next: &CaSocial) {
    for (name, a, b) in [
        ("whisper", live.block_whispers, next.block_whispers),
        ("invite", live.block_invites, next.block_invites),
        ("chatfilter", live.chat_filter, next.chat_filter),
        ("spectator", live.spectator_mode, next.spectator_mode),
    ] {
        if a != b {
            live.send(name, b);
        }
    }
    if live.daily_job_popup != next.daily_job_popup {
        // Stored inverted: the bit set means the popup is off.
        live.send("dailyquest", !next.daily_job_popup);
    }
    if live.notes != next.notes && next.notes != Notes::Unknown {
        live.send("note", next.notes == Notes::Friends);
    }
    live.block_whispers = next.block_whispers;
    live.block_invites = next.block_invites;
    live.chat_filter = next.chat_filter;
    live.spectator_mode = next.spectator_mode;
    live.daily_job_popup = next.daily_job_popup;
    live.notes = next.notes;
}

/// Cancel and the close button (P): the profile saved when the screen
/// opened comes back and is applied again; previews, Default's changes
/// and the binding edits go.
pub fn cancel(world: &mut World) {
    with_options(world, |world, o| {
        if !o.open {
            return;
        }
        if let Some(saved) = o.saved.take() {
            o.profile = saved;
        }
        o.pending.clear();
        o.social_pending = None;
        o.capture = None;
        apply_profile(world, o, false);
        o.open = false;
    });
}

/// The settings a tab's Default button restores.
fn default_settings(o: &CaOptions, tab: Tab) -> Vec<Setting> {
    o.table
        .iter()
        .filter(|x| match tab {
            Tab::Graphic => {
                (x.page == Some(Tab::Graphic) && !x.cvar.eq_ignore_ascii_case("Gore"))
                    || x.section.eq_ignore_ascii_case("Display") && !x.ext
            }
            Tab::Sound => x.page == Some(Tab::Sound),
            Tab::Control => x.page == Some(Tab::Control),
            Tab::Other => false,
        })
        .cloned()
        .collect()
}

/// Default (P) on `tab`, without closing: that tab's values from the
/// default profile, applied at once; the Other tab's from the profile's
/// `[OtherOption]`. The values reach the file with OK.
pub fn default_tab(world: &mut World, tab: Tab) {
    with_options(world, |world, o| {
        let table = o.table.clone();
        let default = o.default_profile.clone();
        for x in default_settings(o, tab) {
            if let Some(v) = x.from_profile(&default) {
                settings::set(world, &table, &x.cvar, &v);
            }
            o.pending.remove(&x.cvar.to_lowercase());
        }
        match tab {
            Tab::Graphic => {
                apply_quality(world, o);
                let machine = *world.resource::<CaMachine>();
                o.custom = custom_of(world, o, machine);
            }
            Tab::Control => {
                o.bindings = bindings::load(&default, &default, true);
                if let Some(mut c) = world.get_resource_mut::<Console>() {
                    bindings::apply(&mut c, &o.bindings);
                }
                o.capture = None;
            }
            Tab::Other => {
                let messenger = world.resource::<CaAccount>().messenger;
                let mut next = o.social_pending.clone().unwrap_or_else(|| world.resource::<CaSocial>().clone());
                next.reset(&o.profile, messenger);
                o.social_pending = Some(next);
            }
            Tab::Sound => {}
        }
    });
}

// ---------------------------------------------------------------------------
// K3: capturing a key

/// Left-click a row: start recording (not while a capture waits).
pub fn begin_capture(world: &mut World, action: usize) -> bool {
    let mut o = world.resource_mut::<CaOptions>();
    if o.capture.is_some() || action >= bindings::ACTIONS.len() {
        return false;
    }
    o.capture = Some(Capture::start(action));
    true
}

/// One frame of recorded input while a capture waits (up to 8 events).
/// Returns whether the capture ended.
pub fn capture_frame(world: &mut World, events: &[RawEvent]) -> bool {
    with_options(world, |world, o| {
        let Some(mut cap) = o.capture.clone() else { return false };
        let console = world.get_resource::<Console>();
        let taken = |slot: bindings::Slot, token: &str| {
            console.is_some_and(|c| bindings::key_name(slot, token).is_some_and(|k| bindings::engine_takes(c, k)))
        };
        let ended = cap.frame(events, &mut o.bindings, &taken);
        o.capture = if ended { None } else { Some(cap) };
        ended
    })
}

/// The row's text (K2) for an action.
pub fn row_text(world: &World, action: usize) -> String {
    let o = world.resource::<CaOptions>();
    o.bindings.entries[action].row_text(&o.key_names)
}

// ---------------------------------------------------------------------------
// Console

pub fn register(app: &mut App) {
    app.console_command(
        "combat_arms:options",
        "options [open [force]|force|ok|cancel|default <tab>|tab <name>|save|applybinds|show]: Combat Arms' options screen state.",
        |w, a| {
            let tab_of = |s: Option<&String>| match s.map(|s| s.to_lowercase()).as_deref() {
                Some("graphic") => Ok(Tab::Graphic),
                Some("sound") => Ok(Tab::Sound),
                Some("control") => Ok(Tab::Control),
                Some("other") => Ok(Tab::Other),
                _ => Err("tab: graphic, sound, control or other".to_string()),
            };
            match a.first().map(|s| s.to_lowercase()).as_deref() {
                None | Some("open") => open_with(w, a.get(1).is_some_and(|f| f == "force")).map(|_| None),
                Some("force") => open_with(w, true).map(|_| None),
                Some("ok") => ok(w).map(|_| None),
                Some("cancel") => {
                    cancel(w);
                    Ok(None)
                }
                Some("default") => {
                    let t = tab_of(a.get(1)).or_else(|_| Ok::<_, String>(w.resource::<CaOptions>().tab))?;
                    default_tab(w, t);
                    Ok(None)
                }
                Some("tab") => {
                    show_tab(w, tab_of(a.get(1))?);
                    Ok(None)
                }
                Some("save") => save(w).map(|_| None),
                Some("applybinds") => {
                    let table = w.resource::<CaOptions>().bindings.clone();
                    bindings::apply(&mut w.resource_mut::<Console>(), &table);
                    Ok(None)
                }
                Some("show") => {
                    let o = w.resource::<CaOptions>();
                    Ok(Some(format!("open {} tab {:?} pending {:?}", o.open, o.tab, o.pending)))
                }
                Some(other) => Err(format!("unknown option \"{other}\"")),
            }
        },
    );
}

/// A snapshot of the pending values (for a screen to read).
pub fn pending(world: &World) -> BTreeMap<String, String> {
    world.resource::<CaOptions>().pending.clone()
}

/// The HUD switch's state (H): whether the layout-built panels are drawn.
pub fn panels_shown(world: &World) -> bool {
    world.get_resource::<CaHudSwitch>().is_none_or(|h| h.panels())
}

/// The live values (for tests and screens).
pub fn live(world: &World) -> &CaSettings {
    world.resource::<CaSettings>()
}
