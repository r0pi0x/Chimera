//! specs/combat_arms/options.md's test cases that need no window
//! (`t01_` is test case 1): the mouse, the slider widget, brightness, the
//! sound volumes, the graphics presets and rules, key binding and the
//! profile's life, headless on `CaSim` with the options' state
//! (`game::options`). The screen's layout and clicks are
//! tests/ui/options.rs (with cases 44 and 45, which read the HUD); the
//! install's key names and layouts are `heavy::options`.

use bevy::prelude::*;
use chimera_core::{console::Console, profile::Profile};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::options::{
        CaAccount, CaGamma, CaMachine, CaMouseLook, CaNotices, CaOptions, CaSettings, CaSoundLevels, RawEvent, Tab,
        bindings::{self, Binding, Slot},
        flow, keys, mouse,
        slider::Slider,
        sound::{self, Class, Multipliers},
        video::{self, Custom, Mode, Quality, Region},
    },
    harness::CaSim,
};

fn sim() -> Sim {
    Sim::new(CaSim, GreyboxMapPlugin)
}

/// A profile that has the template's version and `body` after it.
fn profile(body: &str) -> Profile {
    Profile::parse(&format!("[Version]\nMajor = 1\n{body}"))
}

fn load(sim: &mut Sim, p: Option<Profile>) {
    flow::load(sim.app.world_mut(), p);
    sim.app.update();
}

fn value(sim: &Sim, cvar: &str) -> String {
    sim.app.world().resource::<CaSettings>().get(cvar).unwrap_or("?").to_string()
}

fn number(sim: &Sim, cvar: &str) -> f64 {
    value(sim, cvar).parse().unwrap()
}

fn open(sim: &mut Sim) {
    flow::open(sim.app.world_mut()).unwrap();
}

fn ok(sim: &mut Sim) {
    flow::ok(sim.app.world_mut()).unwrap();
    sim.app.update();
}

fn edit(sim: &mut Sim, cvar: &str, v: &str) {
    flow::edit(sim.app.world_mut(), cvar, v);
    sim.app.update();
}

fn opts(sim: &Sim) -> &CaOptions {
    sim.app.world().resource::<CaOptions>()
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} (+-{tol})");
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ca_options_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("player.txt")
}

fn action(name: &str) -> usize {
    bindings::index_of(name).unwrap()
}

fn binding(sim: &Sim, name: &str) -> String {
    opts(sim).bindings.entries[action(name)].text()
}

fn key(code: u16) -> RawEvent {
    RawEvent::Key { code, down: true }
}

/// Capture `name` and feed one frame of `events`.
fn capture(sim: &mut Sim, name: &str, events: &[RawEvent]) {
    let w = sim.app.world_mut();
    assert!(flow::begin_capture(w, action(name)));
    assert!(flow::capture_frame(w, events), "the capture ended");
}

// ---------------------------------------------------------------------------
// Mouse

#[test]
fn t01_a_thousand_counts_at_sensitivity_five() {
    let (yaw, pitch) = mouse::view_delta((1000.0, 0.0), 5.0, 0, false, false);
    close(yaw, 1.7025, 1e-9);
    assert_eq!(pitch, 0.0);
    close(mouse::axis_scale(5.0), 0.001135, 1e-12);
}

#[test]
fn t02_zoom_divides_the_turn() {
    close(mouse::view_delta((100.0, 0.0), 5.0, 1, false, false).0, 0.0425625, 1e-9);
    close(mouse::view_delta((100.0, 0.0), 5.0, 2, false, false).0, 0.0243214, 1e-7);
}

#[test]
fn t03_counts_for_a_full_turn() {
    close(mouse::counts_per_turn(10.0), 1853.4, 0.2);
    close(mouse::counts_per_turn(1.0), 17824.6, 0.5);
    close(mouse::counts_per_turn(100.0), 186.1, 0.1);
}

#[test]
fn t04_sensitivity_zero() {
    close(mouse::axis_scale(0.0), 0.00001, 1e-12);
    close(mouse::view_delta((1000.0, 0.0), 0.0, 0, false, false).0, 0.015, 1e-9);
}

#[test]
fn t05_invert_flips_the_pitch() {
    close(mouse::view_delta((0.0, 10.0), 5.0, 0, false, false).1, 0.017025, 1e-9);
    close(mouse::view_delta((0.0, 10.0), 5.0, 0, true, false).1, -0.017025, 1e-9);
}

#[test]
fn t06_no_look_under_a_popup() {
    assert_eq!(mouse::view_delta((500.0, 500.0), 5.0, 0, false, true), (0.0, 0.0));
}

#[test]
fn t07_the_sensitivity_keys() {
    let mut s = sim();
    load(&mut s, None);
    s.app.world_mut().resource_mut::<CaSettings>().set("MouseSensitivity", "5");
    let run = |s: &mut Sim, line: &str| {
        s.app.world_mut().resource_mut::<Console>().submit(line);
        s.app.update();
    };
    run(&mut s, "sensitivityinc");
    assert_eq!(value(&s, "MouseSensitivity"), "6");
    close(s.app.world().resource::<CaMouseLook>().scale(), 0.00136, 1e-12);
    let n = s.app.world().resource::<CaNotices>().lines.last().unwrap().clone();
    assert_eq!(n.life_ms, 5000.0);
    assert_eq!(n.colour, "mouse_sensitivity");
    assert!(n.text.contains('6'), "{}", n.text);
    // At 99 it stays, and the line is still shown.
    s.app.world_mut().resource_mut::<CaSettings>().set("MouseSensitivity", "99");
    let before = s.app.world().resource::<CaNotices>().lines.len();
    run(&mut s, "sensitivityinc");
    assert_eq!(value(&s, "MouseSensitivity"), "99");
    assert_eq!(s.app.world().resource::<CaNotices>().lines.len(), before + 1);
    // Down at 1 stays.
    s.app.world_mut().resource_mut::<CaSettings>().set("MouseSensitivity", "1");
    run(&mut s, "sensitivitydec");
    assert_eq!(value(&s, "MouseSensitivity"), "1");
}

#[test]
fn t08_missing_and_large_sensitivity() {
    let mut s = sim();
    load(&mut s, Some(profile("[Controls]\nInvertMouse = 1\n")));
    assert_eq!(s.app.world().resource::<CaMouseLook>().sensitivity, 5, "a profile without the key");
    assert!(s.app.world().resource::<CaMouseLook>().invert);
    load(&mut s, Some(profile("[Controls]\nSensitivity = 150\n")));
    close(s.app.world().resource::<CaMouseLook>().scale(), 0.03376, 1e-9);
}

// ---------------------------------------------------------------------------
// Sliders

#[test]
fn t09_showing_a_stored_value() {
    let mut s = Slider::new(100);
    assert_eq!(s.show(5), 6);
    assert_eq!(s.thumb, 6);
    let mut s = Slider::new(140);
    assert_eq!(s.show(5), 5);
    assert_eq!(s.thumb, 8);
}

#[test]
fn t10_dragging_and_arrows() {
    let mut s = Slider::new(200);
    assert_eq!(s.drag(99), 49);
    assert_eq!(s.drag(100), 50);
    assert_eq!(s.arrow(1), 51);
    assert_eq!(s.thumb, 102);
}

#[test]
fn t11_a_narrow_track_skips_values() {
    let mut s = Slider::new(80);
    let mut read = Vec::new();
    for d in 0..=8 {
        read.push(s.drag(d));
    }
    assert_eq!(read, [0, 1, 2, 3, 5, 6, 7, 8, 10]);
    s.drag(3);
    s.arrow(1);
    assert_eq!(s.thumb, 4, "an arrow click moves one px");
}

#[test]
fn opening_a_slider_in_the_screen_creeps_by_the_tracks_width() {
    let mut s = sim();
    load(&mut s, Some(profile("[Controls]\nSensitivity = 10\n")));
    open(&mut s);
    // A 140 px track shows 10 as 11 (thumb 15 -> 10.7 -> 10? it reads 10).
    let slider = flow::open_slider(s.app.world_mut(), "MouseSensitivity", 100);
    assert_eq!(slider.value(), 11);
    assert_eq!(opts(&s).pending["mousesensitivity"], "11", "OK would save the read-back");
    flow::cancel(s.app.world_mut());
    assert_eq!(value(&s, "MouseSensitivity"), "10");
}

// ---------------------------------------------------------------------------
// Brightness

#[test]
fn t12_slider_to_gamma() {
    for (s, g) in [(0.0, 0.5), (4.0, 0.75), (16.0, 1.0), (33.0, 2.25), (50.0, 6.0), (100.0, 31.519)] {
        close(video::gamma_of_slider(s), g, 1e-3);
    }
}

#[test]
fn t13_gamma_to_slider_is_lossy() {
    assert_eq!(video::slider_of_gamma(1.22), 23);
    close(video::gamma_of_slider(23.0), 1.21194, 1e-5);
    assert_eq!(video::slider_of_gamma(1.155709), 22);
    close(video::gamma_of_slider(22.0), 1.155709, 1e-5);
    assert_eq!(video::slider_of_gamma(2.25), 33);
}

#[test]
fn t14_the_display_ramp() {
    let r = video::ramp(2.0);
    assert_eq!((r[64], r[128], r[255]), (32831, 46431, 65535));
    assert_eq!(video::ramp(1.22)[128], 37249);
}

#[test]
fn the_gamma_slider_moves_the_gamma_at_once_and_cancel_restores_it() {
    let mut s = sim();
    load(&mut s, Some(profile("[Display]\nScreenGamma = 1.22\n")));
    open(&mut s);
    flow::edit_slider(s.app.world_mut(), "GammaR", 33);
    s.app.update();
    close(s.app.world().resource::<CaGamma>().0, 2.25, 1e-6);
    flow::cancel(s.app.world_mut());
    s.app.update();
    close(s.app.world().resource::<CaGamma>().0, 1.22, 1e-6);
}

// ---------------------------------------------------------------------------
// Sound

fn m() -> Multipliers {
    Multipliers::default()
}

#[test]
fn t15_full_volume() {
    let h = |class, r| (sound::level(r, class, 100, &m()), sound::heard(true, 100, r, class, &m()));
    let (l, heard) = h(Class::Weapons, 100.0);
    close(l as f64, 1.27, 1e-6);
    close(heard as f64, 1.27, 1e-6);
    close(h(Class::Default, 100.0).0 as f64, 0.95, 1e-6);
    close(h(Class::Speech, 100.0).0 as f64, 1.27, 1e-6);
}

#[test]
fn t16_half_volume() {
    close(sound::heard(true, 50, 100.0, Class::Weapons, &m()) as f64, 0.315, 1e-6);
    close(sound::heard(true, 50, 100.0, Class::Default, &m()) as f64, 0.23, 1e-6);
    close(sound::heard(true, 50, 100.0, Class::Speech, &m()) as f64, 0.635, 1e-6);
}

#[test]
fn t17_seventy_truncates_to_sixty_nine() {
    close(sound::level(100.0, Class::Weapons, 70, &m()) as f64, 0.87, 1e-6);
    close(sound::heard(true, 70, 100.0, Class::Weapons, &m()) as f64, 0.609, 1e-6);
    close(sound::level(100.0, Class::Default, 70, &m()) as f64, 0.66, 1e-6);
    close(sound::heard(true, 70, 100.0, Class::Default, &m()) as f64, 0.462, 1e-6);
}

#[test]
fn t18_volume_one() {
    assert_eq!(sound::level(100.0, Class::Weapons, 1, &m()), 0.0);
    assert_eq!(sound::level(100.0, Class::Default, 1, &m()), 0.0);
    close(sound::heard(true, 1, 100.0, Class::Speech, &m()) as f64, 0.0127, 1e-6);
}

#[test]
fn t19_effects_off_and_on() {
    let mut s = sim();
    load(&mut s, Some(profile("[Sound]\nsoundvolume = 80\nsoundenable = FALSE\n")));
    assert_eq!(s.app.world().resource::<CaSoundLevels>().master, 0.0);
    open(&mut s);
    edit(&mut s, "enablesound", "1");
    close(s.app.world().resource::<CaSoundLevels>().master as f64, 0.8, 1e-6);
}

#[test]
fn t20_a_negative_stored_volume_is_full() {
    assert_eq!(sound::master(true, -5), 1.0);
    let mut s = sim();
    load(&mut s, Some(profile("[Sound]\nsoundvolume = -5\nsoundenable = TRUE\n")));
    assert_eq!(s.app.world().resource::<CaSoundLevels>().master, 1.0);
}

#[test]
fn t21_music() {
    let mut s = sim();
    load(&mut s, Some(profile("[Sound]\nmusicenable = TRUE\nmusicvolume = 40\n")));
    close(s.app.world().resource::<CaSoundLevels>().music as f64, 0.4, 1e-6);
    load(&mut s, Some(profile("[Sound]\nmusicenable = FALSE\nmusicvolume = 40\n")));
    assert_eq!(s.app.world().resource::<CaSoundLevels>().music, 0.0);
    load(&mut s, Some(profile("[Sound]\nmusicenable = TRUE\n")));
    assert_eq!(s.app.world().resource::<CaSoundLevels>().music, 0.0, "a profile without musicvolume");
    // Over 100: passed on as is, stored as 100.
    close(sound::music_level(true, 150) as f64, 1.5, 1e-6);
    assert_eq!(sound::music_stored(150), 100);
}

// ---------------------------------------------------------------------------
// Graphics

fn value_i(sim: &Sim, cvar: &str) -> i32 {
    number(sim, cvar) as i32
}

#[test]
fn t22_low_to_mid() {
    let mut s = sim();
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 0\n")));
    assert_eq!(value_i(&s, "ShellCasings"), 0);
    open(&mut s);
    edit(&mut s, "PerformanceSetting", "1");
    ok(&mut s);
    for (cvar, v) in [("ShellCasings", 1), ("Tracers", 1), ("ShadowDetail", 1), ("FXDetail", 1), ("EnvironmentalDetail", 1), ("DetailLevel", 2), ("PerformanceSetting", 1)] {
        assert_eq!(value_i(&s, cvar), v, "{cvar}");
    }
    let d = opts(&s).derived;
    assert_eq!(
        d.shadows,
        Some(video::Shadows { blur: 0, texture_size: 128, back_face_cull: 0, per_frame: 0 })
    );
    assert_eq!((d.client_fx_level, d.debris_fx_level), (1, 1));
    assert_eq!(d.snow_density, 1.0);
    assert_eq!(d.texture_offsets, [1, 1, 2]);
    let p = &opts(&s).profile;
    assert_eq!(p.int("Settings", "PerformanceSetting"), Some(1));
    assert_eq!(p.int("Details", "XLargeTex"), Some(2));
}

#[test]
fn t23_high_forces_the_effect_levels_to_one_on_a_shader_machine() {
    let mut s = sim();
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 0\n")));
    open(&mut s);
    edit(&mut s, "PerformanceSetting", "2");
    ok(&mut s);
    assert_eq!((value_i(&s, "Shader"), value_i(&s, "FXDetail")), (1, 2));
    let d = opts(&s).derived;
    assert_eq!((d.client_fx_level, d.debris_fx_level), (1, 1));
    // Without shaders the levels follow FXDetail.
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaMachine>().shader = false;
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 0\n")));
    open(&mut s);
    edit(&mut s, "PerformanceSetting", "2");
    ok(&mut s);
    assert_eq!((opts(&s).derived.client_fx_level, opts(&s).derived.debris_fx_level), (2, 2));
}

#[test]
fn t24_low_environment_is_sparse_snow() {
    let mut s = sim();
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 1\n")));
    open(&mut s);
    edit(&mut s, "PerformanceSetting", "0");
    ok(&mut s);
    assert_eq!(value_i(&s, "EnvironmentalDetail"), 0);
    assert_eq!(opts(&s).derived.snow_density, 0.75);
}

#[test]
fn t25_anisotropic_and_trilinear_exclude_each_other() {
    let mut c = Custom::new(CaMachine::default());
    c.click_trilinear(true);
    assert!(c.trilinear);
    c.click_anisotropic(true);
    assert!(c.anisotropic && !c.trilinear);
    c.click_trilinear(true);
    assert!(c.trilinear && !c.anisotropic);
}

#[test]
fn t26_shader_off_takes_bloom_and_high_effects_with_it() {
    let mut c = Custom::new(CaMachine::default());
    c.click_shader(true);
    assert!(c.bloom_enabled() && !c.effects_enabled(0) && c.effects_enabled(2));
    c.click_bloom(true);
    c.click_effects(2);
    assert_eq!((c.bloom, c.effects), (true, 2));
    c.click_shader(false);
    assert!(!c.bloom && !c.bloom_enabled());
    assert_eq!(c.effects, 1);
    assert!(!c.effects_enabled(2) && c.effects_enabled(0));
    // No shader support: the switches are disabled, High too.
    let c = Custom::new(CaMachine { shader: false, ..default() });
    assert!(!c.shader_enabled() && !c.effects_enabled(2));
}

#[test]
fn custom_clicks_reach_the_variables_on_ok() {
    let mut s = sim();
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 3\nTrilinearFiltering = 1\n")));
    open(&mut s);
    flow::custom_click(s.app.world_mut(), "anisotropic", 1);
    assert_eq!(opts(&s).pending["trilinear"], "0");
    ok(&mut s);
    assert_eq!((value_i(&s, "anisotropic"), value_i(&s, "trilinear")), (1, 0));
}

#[test]
fn t27_custom_always_stores_no_shadows() {
    let mut s = sim();
    load(&mut s, Some(profile("[Settings]\nPerformanceSetting = 2\n")));
    assert_eq!(value_i(&s, "ShadowDetail"), 2);
    open(&mut s);
    edit(&mut s, "PerformanceSetting", "3");
    ok(&mut s);
    assert_eq!(value_i(&s, "ShadowDetail"), 0);
    assert_eq!(opts(&s).profile.int("Settings", "ShadowDetail"), Some(0));
    // The values off the custom block keep what they were (DetailLevel,
    // Shader stays as the High preset left it).
    assert_eq!(value_i(&s, "Shader"), 1);
}

fn mode(w: u32, h: u32, depth: u32) -> Mode {
    Mode { width: w, height: h, depth, format: 1, hardware_tnl: true }
}

fn modes() -> Vec<Mode> {
    vec![
        mode(1024, 768, 32),
        mode(640, 480, 32),
        mode(1280, 720, 32),
        mode(1920, 1080, 16),
        mode(1920, 1080, 32),
        mode(1920, 1080, 32),
        Mode { format: 2, ..mode(1600, 900, 32) },
    ]
}

#[test]
fn t28_an_unlisted_mode_loads_as_the_menus() {
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().modes = modes();
    load(&mut s, Some(profile("[Display]\nScreenWidth = 1600\nScreenHeight = 900\nScreenBitDepth = 32\n")));
    assert_eq!(
        (value_i(&s, "ScreenWidthSetting"), value_i(&s, "ScreenHeightSetting"), value_i(&s, "ScreenDepthSetting")),
        (1024, 768, 32)
    );
    load(&mut s, Some(profile("[Display]\nScreenWidth = 1280\nScreenHeight = 720\nScreenBitDepth = 32\n")));
    assert_eq!(value_i(&s, "ScreenWidthSetting"), 1280);
}

#[test]
fn t29_the_resolution_list() {
    let list = video::mode_list(&modes(), true);
    let shown: Vec<_> = list.iter().map(|m| (m.width, m.height, m.depth)).collect();
    assert_eq!(shown, [(1920, 1080, 32), (1920, 1080, 16), (1280, 720, 32), (1024, 768, 32)]);
    // No 16-bit mode: a stored 16 becomes 32.
    let no16 = video::mode_list(&[mode(1024, 768, 32)], false);
    assert_eq!(video::load_mode(&no16, (1024, 768, 16)), (1024, 768, 32));
}

#[test]
fn t30_blood_off_for_a_minor_in_the_us_build() {
    assert!(!video::gore_allowed(false, Region::Us, Some(16)));
    assert!(video::gore_allowed(false, Region::Europe, Some(16)));
    assert!(video::gore_allowed(false, Region::Us, Some(18)));
    assert!(!video::gore_allowed(true, Region::Europe, None), "a low-violence build");
    let mut s = sim();
    *s.app.world_mut().resource_mut::<CaAccount>() = CaAccount { age: Some(16), region: Region::Us, ..default() };
    load(&mut s, None);
    open(&mut s);
    edit(&mut s, "Gore", "1");
    ok(&mut s);
    assert_eq!(value_i(&s, "Gore"), 0);
    assert_eq!(opts(&s).profile.int("Game", "Gore"), Some(0));
    assert_eq!(Quality::of(3), Quality::Custom);
}

// ---------------------------------------------------------------------------
// Bindings

#[test]
fn the_table_matches_the_default_profile() {
    let default = Profile::parse(combat_arms::game::options::DEFAULT_PROFILE);
    for a in bindings::ACTIONS {
        assert!(default.string("Bindings", a.name).is_some(), "{} has a stock binding", a.name);
    }
    // The stock keys of K1 that matter to other specs.
    let t = bindings::load(&default, &default, true);
    for (name, text) in [
        ("Forward", "##17@ @ "),
        ("Fire", " @##3@ "),
        ("ZoomIn", " @##4@ "),
        ("Dash", "##42@ @ "),
        ("Speed", "##46@ @ "),
        ("DuckLock", "##29@ @ "),
        ("HUDOnOff", "##201@ @ "),
        ("Prev_Weapon", "##16@ @ "),
    ] {
        assert_eq!(t.get(name).unwrap().text(), text, "{name}");
    }
    // The game's default binds agree with the stock keys they name.
    let mut s = sim();
    load(&mut s, None);
    let binds = &s.app.world().resource::<Console>().binds;
    for (k, cmd) in combat_arms::game::DEFAULT_BINDS {
        // Keys the spec doesn't list as actions: the scoreboard, clan chat
        // and voice output (Esc, Pause and F8 can't be captured).
        if ["tab", "f4", "f8"].contains(k) {
            continue;
        }
        assert!(binds.get(*k).is_some_and(|l| l.split(';').any(|p| p.trim() == *cmd)), "{k} {cmd}: {:?}", binds.get(*k));
    }
}

#[test]
fn t31_stealing_a_key_and_the_fallback_that_puts_it_back() {
    let path = temp("t31");
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().path = Some(path.clone());
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "Reload", &[key(16)]);
    assert_eq!(binding(&s, "Reload"), "##16@ @ ");
    assert_eq!(binding(&s, "Prev_Weapon"), " @ @ ");
    ok(&mut s);
    let saved = chimera_core::profile::read(&path).unwrap();
    assert_eq!(saved.string("Bindings", "Prev_Weapon").as_deref(), Some(" @ @ "));
    // A restart: Prev_Weapon is Q again, and Q fires both.
    let mut s2 = sim();
    s2.app.world_mut().resource_mut::<CaOptions>().path = Some(path.clone());
    load(&mut s2, chimera_core::profile::read(&path));
    assert_eq!(binding(&s2, "Prev_Weapon"), "##16@ @ ");
    assert_eq!(binding(&s2, "Reload"), "##16@ @ ");
    assert_eq!(s2.app.world().resource::<Console>().binds["q"], "+reload; lastinv");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn t32_escape_unbinds_the_action_for_good() {
    let path = temp("t32");
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().path = Some(path.clone());
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "Jump", &[key(1)]);
    assert_eq!(binding(&s, "Jump"), " @ @ ");
    assert_eq!(opts(&s).bindings.entries[action("Jump")].row_text(&keys::KeyNames::default()), "N/A");
    ok(&mut s);
    let mut s2 = sim();
    load(&mut s2, chimera_core::profile::read(&path));
    assert_eq!(binding(&s2, "Jump"), " @ @ ");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn t33_the_wheel() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "PrimaryWeapon", &[RawEvent::Wheel { up: true }]);
    assert_eq!(binding(&s, "PrimaryWeapon"), " @#U@ ");
    ok(&mut s);
    assert_eq!(s.app.world().resource::<Console>().binds["mwheelup"], "slot1");
    // Another action taking the wheel takes it away.
    open(&mut s);
    capture(&mut s, "Jump", &[RawEvent::Wheel { up: true }]);
    assert_eq!(binding(&s, "PrimaryWeapon"), " @ @ ");
}

#[test]
fn t34_the_right_button_is_stolen_from_zoom() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "Fire", &[RawEvent::Button { code: 4, down: true }]);
    assert_eq!(binding(&s, "Fire"), " @##4@ ");
    assert_eq!(binding(&s, "ZoomIn"), " @ @ ");
}

/// The install's key-name file gives string ids (`$Keyboard_Name_4`) as
/// the shown text: resolved through the string table, and the built-in
/// name where the table has none (never the raw id).
#[test]
fn key_names_given_as_string_ids_resolve_or_fall_back() {
    let names = keys::KeyNames::parse(
        &lithtech_formats::attributes::Document::parse(
            b"[Keyboard0]\nRealName = \"##42\"\nTriggerName = \"$Keyboard_Name_4\"\n[Keyboard1]\nRealName = \"##57\"\nTriggerName = \"$Keyboard_Name_nope\"\n",
        )
        .unwrap(),
    );
    let table = |id: &str| if id == "$Keyboard_Name_4" { "LSH".to_string() } else { id.to_string() };
    let r = names.resolved(table);
    assert_eq!(r.keyboard_text("##42"), "LSH");
    assert_eq!(r.keyboard_text("##57"), "SPACE");
    assert!(!names.resolved(|id| id.to_string()).keyboard_text("##42").starts_with('$'));
}

#[test]
fn t35_a_new_key_leaves_the_old_one() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "Forward", &[key(200)]);
    assert_eq!(binding(&s, "Forward"), "##200@ @ ");
    let names = keys::KeyNames::parse(
        &lithtech_formats::attributes::Document::parse(b"[Keyboard0]\nRealName = \"##200\"\nTriggerName = \"Up Arrow \"\n").unwrap(),
    );
    assert_eq!(opts(&s).bindings.entries[action("Forward")].row_text(&names), "Up Arrow");
    ok(&mut s);
    assert!(!s.app.world().resource::<Console>().binds.contains_key("w"), "W is no longer bound");
    assert_eq!(s.app.world().resource::<Console>().binds["uparrow"], "+forward");
}

#[test]
fn k3_the_first_frame_with_any_event_clears_and_the_first_key_down_decides() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    let w = s.app.world_mut();
    assert!(flow::begin_capture(w, action("Jump")));
    assert!(!flow::begin_capture(w, action("Fire")), "not while a capture waits");
    // No events: nothing happens, the old binding stays.
    assert!(!flow::capture_frame(w, &[]));
    assert_eq!(w.resource::<CaOptions>().bindings.entries[action("Jump")].text(), "##57@ @ ");
    // A mouse move erases (all three devices) and waits; so does a key-up.
    assert!(!flow::capture_frame(w, &[RawEvent::MouseMove, RawEvent::JoyAxis, RawEvent::Key { code: 30, down: false }]));
    assert_eq!(w.resource::<CaOptions>().bindings.entries[action("Jump")].text(), " @ @ ");
    // Up to 8 events a frame: a ninth isn't looked at.
    let mut events = vec![RawEvent::MouseMove; 8];
    events.push(key(30));
    assert!(!flow::capture_frame(w, &events));
    assert!(flow::capture_frame(w, &[RawEvent::Key { code: 30, down: false }, key(44), key(45)]));
    assert_eq!(w.resource::<CaOptions>().bindings.entries[action("Jump")].text(), "##44@ @ ");
}

#[test]
fn k3_a_key_the_engine_already_binds_cannot_be_taken() {
    let mut s = sim();
    load(&mut s, None);
    s.app.world_mut().resource_mut::<Console>().binds.insert("tab".into(), "+showscores".into());
    open(&mut s);
    capture(&mut s, "Jump", &[key(15)]);
    assert_eq!(binding(&s, "Jump"), " @ @ ", "the capture ended with nothing written");
}

#[test]
fn k3_one_key_per_device() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    capture(&mut s, "Fire", &[key(30)]);
    assert_eq!(binding(&s, "Fire"), "##30@ @ ", "the first capture erased the mouse button");
    let mut b = Binding::parse("##30@##3@pad1");
    assert_eq!(b.row_text(&keys::KeyNames::default()), "A, LEFT MOUSE, pad1");
    b.joy.clear();
    b.mouse.clear();
    b.kbd.clear();
    assert!(b.is_empty());
}

#[test]
fn t36_a_blank_hud_key_gets_page_up_back_when_it_is_free() {
    let mut s = sim();
    load(&mut s, Some(profile("[Bindings]\nHUDOnOff = \" @ @ \"\n")));
    assert_eq!(binding(&s, "HUDOnOff"), "##201@ @ ");
    // Page Up bound to Forward: it stays unbound.
    load(&mut s, Some(profile("[Bindings]\nForward = \"##201@ @ \"\nHUDOnOff = \" @ @ \"\n")));
    assert_eq!(binding(&s, "HUDOnOff"), " @ @ ");
    assert_eq!(binding(&s, "Forward"), "##201@ @ ");
}

#[test]
fn t37_a_missing_entry_on_a_taken_key_restores_the_whole_section() {
    let mut s = sim();
    load(&mut s, Some(profile("[Bindings]\nForward = \"##201@ @ \"\nJump = \"##30@ @ \"\n")));
    assert_eq!(binding(&s, "Forward"), "##17@ @ ", "the default profile's [Bindings]");
    assert_eq!(binding(&s, "Jump"), "##57@ @ ");
    assert_eq!(binding(&s, "HUDOnOff"), "##201@ @ ");
}

#[test]
fn t38_the_sensitivity_keys_have_no_fallback() {
    let mut s = sim();
    load(&mut s, Some(profile("[Bindings]\nForward = \"##17@ @ \"\nHUDOnOff = \"##201@ @ \"\nVideoCapture = \"##209@ @ \"\nUseMutationItem = \"##56@ @ \"\nCommandNPCShop = \"##34@ @ \"\nCommandNPCOrder = \"##35@ @ \"\nCommandUsePerk = \"##41@ @ \"\nQuestState = \"##36@ @ \"\n")));
    assert_eq!(binding(&s, "SensitivityInc"), " @ @ ");
    assert_eq!(binding(&s, "SensitivityDec"), " @ @ ");
}

#[test]
fn t39_the_number_keys_also_choose() {
    let mut s = sim();
    load(&mut s, None);
    // Even when the number keys are rebound.
    open(&mut s);
    capture(&mut s, "PrimaryWeapon", &[key(30)]);
    ok(&mut s);
    let binds = s.app.world().resource::<Console>().binds.clone();
    for n in 1..=6 {
        let line = &binds[&n.to_string()];
        assert!(line.split(';').any(|p| p.trim() == format!("choose_{n}")), "{n}: {line}");
    }
    assert_eq!(binds["2"], "slot2; choose_2");
    assert!(!binds["1"].contains("slot1"), "slot1 moved to A: {}", binds["1"]);
    s.app.world_mut().resource_mut::<Console>().submit("choose_3");
    s.app.update();
    assert_eq!(s.app.world().resource::<combat_arms::game::options::derive::CaChoose>().0, [3]);
    // Not saved: config.cfg is shared with apps that have no `choose_N`
    // (the HL2:DM playtest's "Unknown command" on every press); the keys'
    // own commands are.
    let saved = chimera_core::console::config_text(s.app.world_mut());
    assert!(!saved.contains("choose_"), "{saved}");
    assert!(saved.lines().any(|l| l.starts_with("bind") && l.contains("slot2")), "{saved}");
}

#[test]
fn a_key_two_actions_hold_runs_both_and_keeps_the_polled_reads() {
    let mut s = sim();
    load(&mut s, Some(profile("[Bindings]\nForward = \"##17@ @ \"\nBackward = \"##17@ @ \"\n")));
    let line = s.app.world().resource::<Console>().binds["w"].clone();
    assert_eq!(line, "+forward; +back");
    assert_eq!(Slot::Keyboard, Slot::Keyboard);
}

// ---------------------------------------------------------------------------
// Storage and flow

#[test]
fn t40_no_profile_is_made_from_the_template() {
    let path = temp("t40");
    let mut s = sim();
    {
        let mut o = s.app.world_mut().resource_mut::<CaOptions>();
        o.path = Some(path.clone());
        o.default_profile = Profile::parse("[Version]\nMajor = 1\n[Sound]\nsoundvolume = 33\n");
    }
    load(&mut s, chimera_core::profile::read(&path));
    assert!(opts(&s).first_play);
    assert_eq!(value(&s, "MouseSensitivity"), "5", "the template lacks Sensitivity");
    assert_eq!(value(&s, "soundvolume"), "33");
    assert!(!path.exists(), "written the first time it is saved");
    flow::save(s.app.world_mut()).unwrap();
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn t41_a_new_template_version_deletes_the_profile() {
    let path = temp("t41");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "[Version]\nMajor = 8\n[Controls]\nSensitivity = 77\n").unwrap();
    let mut s = sim();
    {
        let mut o = s.app.world_mut().resource_mut::<CaOptions>();
        o.path = Some(path.clone());
        o.default_profile = Profile::parse("[Version]\nMajor = 9\n[Controls]\nSensitivity = 12\n");
    }
    load(&mut s, chimera_core::profile::read(&path));
    assert!(!path.exists(), "deleted");
    assert!(opts(&s).first_play);
    assert_eq!(value(&s, "MouseSensitivity"), "12");
    // A profile without [Version] counts as major 0.
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().default_profile = Profile::parse("[Version]\nMajor = 1\n[Controls]\nSensitivity = 12\n");
    load(&mut s, Some(Profile::parse("[Controls]\nSensitivity = 77\n")));
    assert_eq!(value(&s, "MouseSensitivity"), "12");
    // The same version is kept.
    load(&mut s, Some(Profile::parse("[Version]\nMajor = 1\n[Controls]\nSensitivity = 77\n")));
    assert_eq!(value(&s, "MouseSensitivity"), "77");
    assert!(!opts(&s).first_play);
}

#[test]
fn t42_cancel_takes_the_previews_back() {
    let path = temp("t42");
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().path = Some(path.clone());
    load(&mut s, Some(profile("[Sound]\nsoundvolume = 50\nsoundenable = TRUE\n")));
    open(&mut s);
    let file = std::fs::read_to_string(&path).unwrap();
    edit(&mut s, "soundvolume", "20");
    close(s.app.world().resource::<CaSoundLevels>().master as f64, 0.2, 1e-6);
    flow::cancel(s.app.world_mut());
    s.app.update();
    close(s.app.world().resource::<CaSoundLevels>().master as f64, 0.5, 1e-6);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), file, "the file is unchanged");
    assert!(!opts(&s).open);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn p_settings_that_wait_for_ok_wait() {
    let mut s = sim();
    load(&mut s, Some(profile("[Controls]\nSensitivity = 5\n[Game]\nGore = 0\n")));
    open(&mut s);
    edit(&mut s, "MouseSensitivity", "30");
    assert_eq!(s.app.world().resource::<CaMouseLook>().sensitivity, 5, "OK-time");
    assert_eq!(flow::control_value(s.app.world_mut(), "MouseSensitivity").as_deref(), Some("30"));
    ok(&mut s);
    assert_eq!(s.app.world().resource::<CaMouseLook>().sensitivity, 30);
    assert_eq!(opts(&s).profile.int("Controls", "Sensitivity"), Some(30));
    assert!(!opts(&s).open);
}

#[test]
fn p_the_screen_does_not_open_in_a_match() {
    use combat_arms::game::rules::{CaRuleSettings, state::{CaMatch, Phase}};
    let mut s = sim();
    load(&mut s, None);
    assert!(flow::open(s.app.world_mut()).is_ok());
    flow::cancel(s.app.world_mut());
    s.app.world_mut().resource_mut::<CaRuleSettings>().mode = "TDM".into();
    s.app.world_mut().resource_mut::<CaMatch>().phase = Phase::Live;
    assert!(flow::open(s.app.world_mut()).is_err());
    assert!(!opts(&s).open);
    // The command refuses too; `options force` opens it all the same.
    s.app.world_mut().resource_mut::<Console>().submit("options");
    s.app.update();
    assert!(!opts(&s).open);
    s.app.world_mut().resource_mut::<Console>().submit("options force");
    s.app.update();
    assert!(opts(&s).open);
}

#[test]
fn t43_default_on_the_control_tab_leaves_the_sound_alone() {
    let mut s = sim();
    load(
        &mut s,
        Some(profile("[Sound]\nsoundvolume = 33\n[Controls]\nSensitivity = 40\n[Bindings]\nJump = \" @ @ \"\nForward = \"##30@ @ \"\n")),
    );
    open(&mut s);
    flow::default_tab(s.app.world_mut(), Tab::Control);
    s.app.update();
    assert_eq!(binding(&s, "Jump"), "##57@ @ ");
    assert_eq!(s.app.world().resource::<CaMouseLook>().sensitivity, 5, "the mouse values, at once");
    ok(&mut s);
    let default = Profile::parse(combat_arms::game::options::DEFAULT_PROFILE);
    for a in bindings::ACTIONS {
        assert_eq!(opts(&s).profile.string("Bindings", a.name), default.string("Bindings", a.name), "{}", a.name);
    }
    assert_eq!(opts(&s).profile.int("Sound", "soundvolume"), Some(33));
}

#[test]
fn p_default_on_the_sound_tab_is_a_preview_that_cancel_undoes() {
    let mut s = sim();
    load(&mut s, Some(profile("[Sound]\nsoundvolume = 33\n")));
    open(&mut s);
    flow::default_tab(s.app.world_mut(), Tab::Sound);
    assert_eq!(value(&s, "soundvolume"), "70");
    flow::cancel(s.app.world_mut());
    assert_eq!(value(&s, "soundvolume"), "33");
}

#[test]
fn p_default_on_the_other_tab_uses_the_profiles_own_defaults() {
    let mut s = sim();
    load(&mut s, Some(profile("[OtherOption]\nWhisper = TRUE\nBlockInvite = TRUE\nCurseFilter = FALSE\nAllowNote = FALSE\nDailyQuest = FALSE\n")));
    s.app.world_mut().resource_mut::<CaAccount>().messenger = true;
    open(&mut s);
    flow::edit_social(s.app.world_mut(), "block_whispers", false);
    flow::default_tab(s.app.world_mut(), Tab::Other);
    let p = opts(&s).social_pending.clone().unwrap();
    assert!(p.block_whispers && p.block_invites && !p.chat_filter && !p.daily_job_popup);
    ok(&mut s);
    let live = s.app.world().resource::<combat_arms::game::options::CaSocial>();
    assert!(live.block_whispers);
    assert!(live.requests.iter().any(|r| r == "whisper 1"), "{:?}", live.requests);
    assert!(live.requests.iter().any(|r| r == "dailyquest 1"), "the popup bit is stored inverted: {:?}", live.requests);
}

#[test]
fn t46_the_radar_rotation_option_reaches_the_console_on_ok() {
    let mut s = sim();
    load(&mut s, None);
    open(&mut s);
    edit(&mut s, "rotateradar", "1");
    assert_eq!(s.app.world().resource::<combat_arms::game::radar::CaRadarProfile>().rotate, 0);
    ok(&mut s);
    assert_eq!(s.app.world().resource::<combat_arms::game::radar::CaRadarProfile>().rotate, 1);
    assert_eq!(opts(&s).profile.int("Controls", "RotateRadar"), Some(1));
}

#[test]
fn t44_the_radar_zoom_is_saved_with_the_profile() {
    let mut s = sim();
    load(&mut s, None);
    s.app.world_mut().resource_mut::<combat_arms::game::radar::CaRadarProfile>().scale = 1.6;
    flow::save(s.app.world_mut()).unwrap();
    close(opts(&s).profile.float("Display", "RadarScale").unwrap() as f64, 1.6, 1e-6);
    let saved = opts(&s).profile.clone();
    load(&mut s, Some(saved));
    close(s.app.world().resource::<combat_arms::game::radar::CaRadarProfile>().scale as f64, 1.6, 1e-6);
}

#[test]
fn f_unmanaged_lines_survive_a_save() {
    let mut s = sim();
    load(&mut s, Some(profile("// my note\n[Display]\nCrossHairColor = 255\n[Bindings]\nChoose_7 = \"##8@ @ \"\n")));
    flow::save(s.app.world_mut()).unwrap();
    let text = opts(&s).profile.to_text();
    assert!(text.contains("// my note"));
    assert!(text.contains("CrossHairColor = 255"));
    assert!(text.contains("Choose_7 = \"##8@ @ \""));
}

#[test]
fn f_restore_defaults_reloads_every_section() {
    let mut s = sim();
    load(&mut s, Some(profile("[Controls]\nSensitivity = 77\n")));
    s.app.world_mut().resource_mut::<CaSettings>().set("RestoreDefaults", "1");
    flow::load(s.app.world_mut(), Some(profile("[Controls]\nSensitivity = 77\n")));
    assert_eq!(value(&s, "MouseSensitivity"), "5");
    assert_eq!(value(&s, "RestoreDefaults"), "0");
}

#[test]
fn f_the_options_have_console_variables() {
    let mut s = sim();
    load(&mut s, None);
    s.app.world_mut().resource_mut::<Console>().submit("soundvolume 15");
    s.app.update();
    assert_eq!(value(&s, "soundvolume"), "15");
    close(s.app.world().resource::<CaSoundLevels>().master as f64, 0.15, 1e-6);
    // Tenths and inverted variables.
    load(&mut s, Some(profile("[Game]\nWeaponSway = 10\nMessageDur = 15\nDirectSound = 0\n")));
    assert_eq!(value(&s, "WeaponSway"), "1");
    assert_eq!(value(&s, "MessageDuration"), "1.5");
    assert_eq!(value(&s, "DisableHardwareSound"), "1");
    // A profile without them: weapon sway and messages are 0 (a quirk).
    load(&mut s, Some(profile("")));
    assert_eq!(value(&s, "WeaponSway"), "0");
    assert_eq!(value(&s, "MessageDuration"), "0");
    assert_eq!(value(&s, "soundvolume"), "0", "a profile without soundvolume is silent");
}

#[test]
fn t45_the_hud_switch_starts_on_and_flips() {
    let mut s = sim();
    load(&mut s, None);
    assert!(s.app.world().resource::<combat_arms::game::options::CaHudSwitch>().panels());
    s.app.world_mut().resource_mut::<Console>().submit("hudonoff");
    s.app.update();
    assert!(!s.app.world().resource::<combat_arms::game::options::CaHudSwitch>().panels());
    // Kept across a new match (nothing resets it); a new game starts on.
    combat_arms::harness::with_mode(s.app.world_mut(), "TDM");
    s.ticks(3);
    assert!(!flow::panels_shown(s.app.world()));
    let fresh = sim();
    assert!(flow::panels_shown(fresh.app.world()));
}

#[test]
fn h_the_help_key_toggles_except_for_game_masters() {
    let mut s = sim();
    load(&mut s, None);
    let run = |s: &mut Sim| {
        s.app.world_mut().resource_mut::<Console>().submit("helpscreen");
        s.app.update();
    };
    run(&mut s);
    assert!(s.app.world().resource::<combat_arms::game::options::HelpPopup>().open);
    run(&mut s);
    assert!(!s.app.world().resource::<combat_arms::game::options::HelpPopup>().open);
    s.app.world_mut().resource_mut::<CaAccount>().game_master = true;
    run(&mut s);
    assert!(!s.app.world().resource::<combat_arms::game::options::HelpPopup>().open);
}

#[test]
fn o_the_block_list() {
    use combat_arms::game::options::social::{BlockList, Refused};
    let mut l = BlockList::default();
    assert_eq!(l.register("GMBob"), Err(Refused::GameMaster));
    l.register("zed").unwrap();
    l.register("Adam").unwrap();
    l.register("mike").unwrap();
    assert_eq!(l.register("ADAM"), Err(Refused::AlreadyThere));
    l.sort();
    assert_eq!(l.names.iter().map(|n| n.0.as_str()).collect::<Vec<_>>(), ["Adam", "mike", "zed"]);
    l.sort();
    assert_eq!(l.names.iter().map(|n| n.0.as_str()).collect::<Vec<_>>(), ["zed", "mike", "Adam"]);
    assert!(!l.clear_checked(), "nothing was checked");
    l.check_all(true);
    l.names[1].1 = false;
    assert!(l.clear_checked());
    assert_eq!(l.names.len(), 1);
    assert_eq!(BlockList::from_profile(&l.to_profile()).names[0].0, "mike");
    assert_eq!(BlockList::file_name("me"), "me_BlackList.txt");
}

#[test]
fn the_machine_limits_music_and_triple_buffering() {
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaMachine>().music = false;
    s.app.world_mut().resource_mut::<CaMachine>().triple_buffer = false;
    load(&mut s, Some(profile("[Sound]\nmusicvolume = 60\n[Settings]\nPerformanceSetting = 2\n")));
    assert_eq!(value(&s, "musicvolume"), "0");
    assert_eq!(value(&s, "triplebuffer"), "0");
    assert_eq!(value(&s, "MusicActive"), "0");
    assert_eq!(s.app.world().resource::<CaSoundLevels>().music, 0.0);
}

/// Saving the profile at exit only looks at the exit message: the app's
/// runner still reads it, so `--screenshot` and `quit` end the run.
#[test]
fn saving_at_exit_leaves_the_exit_message_for_the_runner() {
    let mut s = sim();
    let dir = std::env::temp_dir().join(format!("chimera_ca_exit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    combat_arms::game::options::use_user_profile(&mut s.app, Some(dir.join("profile.txt")));
    s.app.world_mut().write_message(bevy::app::AppExit::Success);
    s.app.update();
    let kept = !s.app.world().resource::<Messages<bevy::app::AppExit>>().is_empty();
    let saved = dir.join("profile.txt").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(kept, "the exit message was drained");
    assert!(saved, "the profile is saved at exit");
}

/// Bind migration (architecture review X5): CS:S's polled commands moved
/// out of Chimera's `POLLED` into CS:S's UI. Combat Arms' stock binds use
/// none of them, so its binds are read exactly as before.
#[test]
fn the_stock_binds_use_none_of_the_commands_that_moved_to_cs_source() {
    let all = combat_arms::game::DEFAULT_BINDS.iter().chain(combat_arms::game::OLD_DEFAULT_BINDS);
    for (k, line) in all {
        for part in line.split(';').map(|p| p.trim().to_lowercase()) {
            let moved = ["buymenu", "chooseteam", "radio1", "radio2", "radio3"];
            assert!(!moved.contains(&part.as_str()), "{k} {line}");
        }
    }
}
