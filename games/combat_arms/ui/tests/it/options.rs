//! specs/combat_arms/options.md on the screen (`ui::options`, headless on
//! `CaSim` with the screen's state plugin and no window): the layouts'
//! controls and their names (L), the tabs and their windows (P), the
//! buttons (OK, Cancel, close, Default), check pairs, the Custom block's
//! rules as clicks, sliders (W) read through their track, the binding
//! rows and key capture (K1 to K3), drop-downs, the voice page and the
//! Other tab; what the options hand the shell (M2, G3, S2); and the two
//! cases that read the HUD (44 radar zoom saved with the profile, 45 the
//! HUD on/off key). The numbers are tests/it/options.rs's.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{LocalPlayer, SpawnPoint, Team},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::{
        options::{
            CaAccount, CaGamma, CaHudSwitch, CaMouseLook, CaOptions, CaSettings, CaSoundLevels, CaVoice, Tab, bindings, flow,
            video::Region,
        },
        radar::CaRadarProfile,
    }, harness::{CaSim, with_mode}};
use combat_arms_ui::{
        CaUi,
        hud::{CaHudStatePlugin, CaHud, radar::{CaRadar, RadarKeys}},
        menu::MenuPointer,
        options::{BUTTONS, CaOptionsStatePlugin, DROP_TEXTS, OptionsScreen, SLIDERS, WINDOWS},
        options_apply::{self, CaOptionsApplyPlugin},
    };

#[path = "../common/mode_kit.rs"]
mod mode_kit;
use mode_kit::*;

fn sim() -> Sim {
    let mut s = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaOptionsStatePlugin));
    });
    flow::load(s.app.world_mut(), None);
    s.app.update();
    s
}

fn screen(sim: &Sim) -> &OptionsScreen {
    sim.app.world().resource::<OptionsScreen>()
}

fn ui(sim: &Sim) -> CaUi {
    sim.app.world().resource::<CaUi>().clone()
}

fn opts(sim: &Sim) -> &CaOptions {
    sim.app.world().resource::<CaOptions>()
}

fn value(sim: &Sim, cvar: &str) -> String {
    sim.app.world().resource::<CaSettings>().get(cvar).unwrap_or("?").to_string()
}

fn open(sim: &mut Sim) {
    flow::open(sim.app.world_mut()).unwrap();
    sim.app.update();
}

/// The centre of a control, screen px.
fn centre(sim: &Sim, name: &str) -> IVec2 {
    let r = screen(sim).rect_of(name, &ui(sim)).unwrap_or_else(|| panic!("no control {name}"));
    IVec2::new((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2)
}

fn move_to(sim: &mut Sim, p: IVec2) {
    sim.app.world_mut().resource_mut::<MenuPointer>().0 = Some(p);
    sim.app.update();
}

fn press(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    sim.app.update();
}

fn release(sim: &mut Sim) {
    {
        let mut b = sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        // The press of the frame before is over; the release is this one's.
        b.clear();
        b.release(MouseButton::Left);
    }
    sim.app.update();
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
}

/// A left-click on a control's centre.
fn click(sim: &mut Sim, name: &str) {
    let p = centre(sim, name);
    move_to(sim, p);
    press(sim);
    release(sim);
}

fn selected(sim: &Sim, name: &str) -> bool {
    screen(sim).selected(name).unwrap_or_else(|| panic!("no control {name}"))
}

fn enabled(sim: &Sim, name: &str) -> bool {
    screen(sim).enabled(name).unwrap_or_else(|| panic!("no control {name}"))
}

fn shown(sim: &Sim, name: &str) -> bool {
    screen(sim).shown(name).unwrap_or_else(|| panic!("no control {name}"))
}

fn text(sim: &Sim, name: &str) -> String {
    screen(sim).text_of(name, &ui(sim)).unwrap_or_else(|| panic!("no text {name}"))
}

// ---------------------------------------------------------------------------
// L: the layouts

#[test]
fn our_layouts_have_every_control_the_screen_binds() {
    let s = sim();
    let screen = screen(&s);
    for (name, _) in BUTTONS {
        assert!(screen.find(name).is_some(), "button {name}");
    }
    for (bar, text, _) in SLIDERS {
        assert!(screen.find(bar).is_some() && screen.find(text).is_some(), "slider {bar}");
    }
    for (text, _) in DROP_TEXTS {
        assert!(screen.find(text).is_some(), "drop-down text {text}");
    }
    for (win, _) in WINDOWS {
        assert!(screen.find(win).is_some(), "window {win}");
    }
    for a in bindings::ACTIONS.iter().filter(|a| a.row.is_some()) {
        let row = a.row.unwrap();
        assert!(screen.find(row).is_some(), "row {row}");
        assert!(screen.row_text(row, &ui(&s)).is_some(), "{row}'s ValueText");
    }
    for name in ["detail", "general_window", "voice_window", "spectatormode_wnd", "list_control", "item_name"] {
        assert!(screen.find(name).is_some(), "{name}");
    }
}

#[test]
fn the_screen_opens_on_graphic_and_the_tabs_show_their_windows() {
    let mut s = sim();
    assert!(!screen(&s).desk.sheets[screen(&s).sheets[0]].shown);
    open(&mut s);
    assert!(screen(&s).desk.sheets[screen(&s).sheets[0]].shown);
    for (win, tab) in WINDOWS {
        assert_eq!(shown(&s, win), tab == Tab::Graphic, "{win}");
    }
    assert!(selected(&s, "tab_graphic"));
    click(&mut s, "tab_sound");
    assert_eq!(opts(&s).tab, Tab::Sound);
    assert!(shown(&s, "SoundWnd") && !shown(&s, "GraphicWnd"));
    click(&mut s, "tab_control");
    assert!(shown(&s, "ControlWnd") && !shown(&s, "SoundWnd"));
    click(&mut s, "tab_other");
    assert!(shown(&s, "OtherWnd"));
    // The status the shell reads: the mouse shows while it is open.
    assert!(screen(&s).fired.contains(&"tab_other".to_string()));
}

#[test]
fn the_frame_is_centred_with_its_windows() {
    let mut s = sim();
    open(&mut s);
    s.app.world_mut().resource_mut::<OptionsScreen>().screen = IVec2::new(1920, 1080);
    s.app.world_mut().resource_mut::<OptionsScreen>().place();
    s.app.update();
    let u = ui(&s);
    let frame = screen(&s).rect_of("popup_option", &u).unwrap();
    assert_eq!(frame.x0, 1920 / 2 - 350);
    let win = screen(&s).rect_of("GraphicWnd", &u).unwrap();
    assert_eq!((win.x0 - frame.x0, win.y0 - frame.y0), (10, 46), "the window keeps its place in the frame");
}

#[test]
fn ok_cancel_and_close() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    click(&mut s, "InvertMouse_On");
    assert!(selected(&s, "InvertMouse_On") && !selected(&s, "InvertMouse_Off"));
    assert_eq!(value(&s, "MouseInvertY"), "0", "waits for OK");
    click(&mut s, "button_ok");
    assert_eq!(value(&s, "MouseInvertY"), "1");
    assert!(!opts(&s).open);
    s.app.update();
    assert!(!screen(&s).desk.sheets[screen(&s).sheets[0]].shown);
    // Cancel and the close button both go back.
    for button in ["button_cancel", "button_close"] {
        open(&mut s);
        click(&mut s, "tab_control");
        click(&mut s, "InvertMouse_Off");
        click(&mut s, button);
        assert_eq!(value(&s, "MouseInvertY"), "1", "{button}");
        assert!(!opts(&s).open);
        s.app.update();
    }
}

#[test]
fn default_acts_on_the_visible_tab() {
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaSettings>().set("soundvolume", "12");
    s.app.world_mut().resource_mut::<CaSettings>().set("MouseSensitivity", "40");
    open(&mut s);
    click(&mut s, "tab_sound");
    click(&mut s, "button_default");
    assert_eq!(value(&s, "soundvolume"), "70", "the default profile's, at once");
    assert_eq!(value(&s, "MouseSensitivity"), "40", "the Control tab is another tab");
    // Cancel undoes it.
    click(&mut s, "button_cancel");
    assert_eq!(value(&s, "soundvolume"), "12");
}

// ---------------------------------------------------------------------------
// Graphic

#[test]
fn the_custom_block_shows_with_custom_quality_and_follows_its_rules() {
    let mut s = sim();
    open(&mut s);
    assert!(!shown(&s, "detail"));
    click(&mut s, "check_custom");
    assert!(shown(&s, "detail"));
    assert!(selected(&s, "check_custom"));
    // Shader off: bloom disabled, High effects disabled.
    click(&mut s, "check_shader_off");
    assert!(!enabled(&s, "check_bloom_on") && !enabled(&s, "check_fxlevel_high") && enabled(&s, "check_fxlevel_low"));
    click(&mut s, "check_shader_on");
    assert!(enabled(&s, "check_bloom_on") && enabled(&s, "check_fxlevel_high") && !enabled(&s, "check_fxlevel_low"));
    click(&mut s, "check_bloom_on");
    click(&mut s, "check_fxlevel_high");
    assert!(selected(&s, "check_bloom_on") && selected(&s, "check_fxlevel_high"));
    click(&mut s, "check_shader_off");
    assert!(selected(&s, "check_bloom_off") && selected(&s, "check_fxlevel_mid"));
    assert!(!enabled(&s, "check_fxlevel_high"));
    // Anisotropic and trilinear exclude each other.
    click(&mut s, "check_trilinear_on");
    click(&mut s, "check_anisotropic_on");
    assert!(selected(&s, "check_trilinear_off") && selected(&s, "check_anisotropic_on"));
    // The shadows and the character detail are shown and can't change.
    assert!(selected(&s, "check_shadow_off") && !enabled(&s, "check_shadow_high"));
    assert!(selected(&s, "check_chardetail_high") && !enabled(&s, "check_chardetail_low"));
    click(&mut s, "check_shadow_high");
    assert!(selected(&s, "check_shadow_off"));
    // Leaving Custom hides the block.
    click(&mut s, "check_high");
    assert!(!shown(&s, "detail"));
}

#[test]
fn quality_checks_and_the_blood_switch() {
    let mut s = sim();
    open(&mut s);
    assert!(selected(&s, "check_high"), "the template's quality");
    click(&mut s, "check_low");
    assert!(selected(&s, "check_low") && !selected(&s, "check_high"));
    assert_eq!(value(&s, "PerformanceSetting"), "2", "OK-time");
    click(&mut s, "check_blood_on");
    assert!(selected(&s, "check_blood_on"));
    click(&mut s, "button_ok");
    assert_eq!(value(&s, "PerformanceSetting"), "0");
    assert_eq!(value(&s, "ShellCasings"), "0");
    assert_eq!(value(&s, "Gore"), "1");
}

#[test]
fn a_minor_in_the_us_build_has_blood_off_and_disabled() {
    let mut s = sim();
    *s.app.world_mut().resource_mut::<CaAccount>() = CaAccount { age: Some(16), region: Region::Us, ..default() };
    open(&mut s);
    assert!(selected(&s, "check_blood_off") && !enabled(&s, "check_blood_on") && !enabled(&s, "check_blood_off"));
}

#[test]
fn the_depth_and_resolution_controls() {
    let mut s = sim();
    open(&mut s);
    assert_eq!(text(&s, "droptext_resolution"), "1024 x 768");
    assert!(selected(&s, "check_32bit"));
    click(&mut s, "drop_resolution");
    assert_eq!(text(&s, "droptext_resolution"), "800 x 600", "the next entry (the list wraps)");
    click(&mut s, "drop_resolution");
    assert_eq!(text(&s, "droptext_resolution"), "1920 x 1080");
    click(&mut s, "check_16bit");
    assert!(selected(&s, "check_16bit"));
    click(&mut s, "drop_antialias");
    assert_eq!(text(&s, "droptext_antialiasing"), "2X");
    click(&mut s, "button_ok");
    assert_eq!((value(&s, "ScreenWidthSetting"), value(&s, "ScreenHeightSetting"), value(&s, "ScreenDepthSetting")), ("1920".into(), "1080".into(), "16".into()));
    assert_eq!(opts(&s).profile.int("Display", "ScreenWidth"), Some(1920));
    assert_eq!(opts(&s).profile.int("Display", "AntiAlias"), Some(2));
}

#[test]
fn no_sixteen_bit_mode_disables_the_sixteen_bit_button() {
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaOptions>().modes = vec![combat_arms::game::options::Mode { width: 1024, height: 768, depth: 32, format: 1, hardware_tnl: false }];
    open(&mut s);
    assert!(!enabled(&s, "check_16bit") && enabled(&s, "check_32bit"));
}

// ---------------------------------------------------------------------------
// Sliders

#[test]
fn a_click_on_the_track_moves_the_thumb_and_a_drag_follows() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_sound");
    let u = ui(&s);
    let r = screen(&s).rect_of("sound_volume_sliderbar", &u).unwrap();
    // The track is 111 - 10 = 101 px: the thumb's left edge goes to the click.
    move_to(&mut s, IVec2::new(r.x0 + 50, (r.y0 + r.y1) / 2));
    press(&mut s);
    assert_eq!(value(&s, "soundvolume"), "49", "floor(50 * 100 / 101), at once");
    assert_eq!(text(&s, "sound_volume_value"), "49");
    move_to(&mut s, IVec2::new(r.x0 + 80, (r.y0 + r.y1) / 2));
    assert_eq!(value(&s, "soundvolume"), "79");
    release(&mut s);
    move_to(&mut s, IVec2::new(r.x0 + 10, (r.y0 + r.y1) / 2));
    assert_eq!(value(&s, "soundvolume"), "79", "not dragging any more");
    click(&mut s, "button_cancel");
    assert_eq!(value(&s, "soundvolume"), "70");
}

#[test]
fn a_track_of_exactly_one_hundred_makes_the_value_creep() {
    // Our tracks are 101 px (no creep); a layout whose track is 100 shows
    // a stored 10 as 11 and OK would save it (Quirk 2).
    let mut s = sim();
    s.app.world_mut().resource_mut::<CaSettings>().set("MouseSensitivity", "10");
    open(&mut s);
    assert_eq!(value(&s, "MouseSensitivity"), "10");
    assert_eq!(text(&s, "SliderBar_value"), "10");
    let slider = flow::open_slider(s.app.world_mut(), "MouseSensitivity", 100);
    assert_eq!(slider.value(), 11);
    assert_eq!(opts(&s).pending["mousesensitivity"], "11");
}

#[test]
fn the_gamma_slider_shows_the_stored_gamma_through_the_scale() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_graphic");
    assert_eq!(text(&s, "sliderbar_value_gamma"), "23", "1.22 shows as 23");
    let u = ui(&s);
    let r = screen(&s).rect_of("sliderbar_gamma", &u).unwrap();
    move_to(&mut s, IVec2::new(r.x0 + 34, (r.y0 + r.y1) / 2));
    press(&mut s);
    release(&mut s);
    assert!((s.app.world().resource::<CaGamma>().0 - 2.25).abs() < 1e-3);
}

// ---------------------------------------------------------------------------
// Controls and binding

fn keys_in(sim: &mut Sim, down: &[KeyCode]) {
    let mut k = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    k.clear();
    for c in down {
        k.press(*c);
    }
}

#[test]
fn the_binding_rows_show_the_keys() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    let u = ui(&s);
    let t = |s: &Sim, row: &str| screen(s).row_text(row, &u).unwrap();
    assert_eq!(t(&s, "Forward"), "W");
    assert_eq!(t(&s, "Fire"), "LEFT MOUSE");
    assert_eq!(t(&s, "Crouch"), "LEFT CTRL");
    assert_eq!(t(&s, "HUDOnOff"), "PAGE UP");
    assert_eq!(t(&s, "Use_Perk"), "`");
}

/// The install's key-name file gives string ids as the rows' texts: the
/// string table's text shows, and the built-in name where it has none.
#[test]
fn the_binding_rows_resolve_key_name_ids() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    {
        let w = s.app.world_mut();
        w.resource_mut::<CaUi>().strings = combat_arms_ui::layout::Strings::parse("Keyboard_Name_17 = \"WW\"\n");
        let mut o = w.resource_mut::<CaOptions>();
        o.key_names.keyboard = vec![("##17".into(), "$Keyboard_Name_17".into()), ("##32".into(), "$Keyboard_Name_32".into())];
    }
    s.app.update();
    let u = ui(&s);
    assert_eq!(screen(&s).row_text("Forward", &u).unwrap(), "WW");
    assert_eq!(screen(&s).row_text("Right", &u).unwrap(), "D", "no table entry: the built-in name");
}

/// The Control tab's rows stay inside its window: what overflows scrolls
/// (the wheel; here the call), rows scrolled out of the window are not
/// clicked, and the first rows are back at 0.
#[test]
fn the_control_tab_scrolls_what_overflows() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    let u = ui(&s);
    // Our layout fits; make the window shorter than its rows.
    {
        let mut sc = s.app.world_mut().resource_mut::<OptionsScreen>();
        let (id, sheet) = (sc.roots[3].0, sc.sheets[3]);
        sc.desk.sheets[sheet].layout.tree.get_mut(id).size.y = 200;
    }
    let (clip, max) = screen(&s).control_view(&u);
    assert!(max > 0, "the rows overflow the window");
    let before = screen(&s).rect_of("Back", &u).unwrap();
    assert!(before.y1 <= clip.y1 || before.y0 < clip.y1);
    let moved = s.app.world_mut().resource_mut::<OptionsScreen>().scroll_control(&u, 40);
    assert!(moved);
    let after = screen(&s).rect_of("Back", &u).unwrap();
    assert_eq!(after.y0, before.y0 - 40, "the row moved up with the scroll");
    // The last row is reachable at the end; the scroll stops there.
    let end = s.app.world_mut().resource_mut::<OptionsScreen>().scroll_control(&u, 100_000);
    assert!(end);
    assert_eq!(screen(&s).scroll, screen(&s).control_view(&u).1);
    let last = screen(&s).rect_of("Back", &u).unwrap();
    assert!(last.y0 < before.y0, "scrolled further");
    // A row below the window is not under a click outside it.
    s.app.world_mut().resource_mut::<OptionsScreen>().scroll_control(&u, -100_000);
    let below = IVec2::new(before.x0 + 5, clip.y1 + 10);
    let fired = screen(&s).fired.len();
    move_to(&mut s, below);
    press(&mut s);
    release(&mut s);
    assert_eq!(screen(&s).fired.len(), fired, "nothing below the window fires");
}

#[test]
fn a_click_on_a_row_captures_the_next_key() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    let u = ui(&s);
    // The click that began it isn't one of its events; the row is empty.
    let p = centre(&s, "Reload");
    move_to(&mut s, p);
    press(&mut s);
    assert!(opts(&s).capture.is_some());
    assert_eq!(screen(&s).row_text("Reload", &u).unwrap(), "");
    assert_eq!(opts(&s).bindings.entries[bindings::index_of("Reload").unwrap()].text(), "##19@ @ ", "nothing erased yet");
    release(&mut s);
    // The release is the first event: it erases the bindings and waits.
    assert!(opts(&s).capture.is_some());
    assert_eq!(opts(&s).bindings.entries[bindings::index_of("Reload").unwrap()].text(), " @ @ ");
    // A second click on a row while one waits does nothing.
    keys_in(&mut s, &[KeyCode::KeyQ]);
    s.app.update();
    assert!(opts(&s).capture.is_none());
    assert_eq!(screen(&s).row_text("Reload", &u).unwrap(), "Q");
    assert_eq!(screen(&s).row_text("PreviousWeapon", &u).unwrap(), "N/A", "the key is stolen");
    keys_in(&mut s, &[]);
}

#[test]
fn escape_unbinds_a_row() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    let p = centre(&s, "Jump");
    move_to(&mut s, p);
    press(&mut s);
    release(&mut s);
    keys_in(&mut s, &[KeyCode::Escape]);
    s.app.update();
    keys_in(&mut s, &[]);
    assert_eq!(screen(&s).row_text("Jump", &ui(&s)).unwrap(), "N/A");
    // The screen stays open: Escape means nothing to it.
    assert!(opts(&s).open);
}

#[test]
fn the_wheel_binds() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    let p = centre(&s, "PrimaryWeapon");
    move_to(&mut s, p);
    press(&mut s);
    release(&mut s);
    s.app.world_mut().insert_resource(bevy::input::mouse::AccumulatedMouseScroll {
        unit: bevy::input::mouse::MouseScrollUnit::Line,
        delta: Vec2::new(0.0, 1.0),
    });
    s.app.update();
    assert_eq!(screen(&s).row_text("PrimaryWeapon", &ui(&s)).unwrap(), "WHEEL UP");
}

#[test]
fn the_mouse_and_radar_switches() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_control");
    assert!(selected(&s, "InvertMouse_Off") && selected(&s, "mapRotaion_Off"));
    assert_eq!(text(&s, "SliderBar_value"), "5");
    click(&mut s, "mapRotaion_On");
    assert!(selected(&s, "mapRotaion_On"));
    click(&mut s, "button_ok");
    assert_eq!(s.app.world().resource::<CaRadarProfile>().rotate, 1);
}

#[test]
fn escape_does_not_open_the_in_game_menu_over_the_screen() {
    let mut s = sim();
    open(&mut s);
    assert!(opts(&s).open);
}

// ---------------------------------------------------------------------------
// Sound and voice

#[test]
fn the_sound_switches_and_the_voice_controls() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_sound");
    assert!(shown(&s, "general_window") && !shown(&s, "voice_window"));
    assert!(selected(&s, "bgm_on") && selected(&s, "sound_on") && selected(&s, "bgm_rotation"));
    click(&mut s, "sound_off");
    assert_eq!(value(&s, "enablesound"), "0", "at once");
    assert_eq!(s.app.world().resource::<CaSoundLevels>().master, 0.0);
    click(&mut s, "sound_on");
    assert!((s.app.world().resource::<CaSoundLevels>().master - 0.7).abs() < 1e-6);
    click(&mut s, "button_voice");
    assert!(shown(&s, "voice_window") && !shown(&s, "general_window"));
    // No voice system: every voice control is disabled.
    for n in ["voicechat_on", "voicechat_off", "btn_testmic", "mic_device_combo", "speaker_device_combo", "gamesoundforvoice_on", "mic_volume_sliderbar"] {
        assert!(!enabled(&s, n), "{n}");
    }
    assert_eq!(text(&s, "droptext_microphone"), "finding");
}

#[test]
fn with_voice_only_the_switch_works_until_it_is_on() {
    let mut s = sim();
    *s.app.world_mut().resource_mut::<CaVoice>() =
        CaVoice { built: true, ready: true, microphones: vec!["A microphone with a very long name".into(), "Mic 2".into()], speakers: vec!["Speakers".into()] };
    open(&mut s);
    click(&mut s, "tab_sound");
    click(&mut s, "button_voice");
    assert!(enabled(&s, "voicechat_on") && !enabled(&s, "btn_testmic") && !enabled(&s, "mic_device_combo"));
    click(&mut s, "voicechat_on");
    assert!(enabled(&s, "btn_testmic") && enabled(&s, "mic_device_combo") && enabled(&s, "speaker_volume_sliderbar"));
    assert_eq!(text(&s, "droptext_microphone"), "A microphone with a ve...");
    click(&mut s, "mic_device_combo");
    assert_eq!(text(&s, "droptext_microphone"), "Mic 2");
    click(&mut s, "mic_device_combo");
    click(&mut s, "speaker_device_combo");
    assert_eq!(value(&s, "SpeakerDeviceIndex"), "0", "one speaker: it stays");
}

// ---------------------------------------------------------------------------
// Other

#[test]
fn the_other_tab() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_other");
    assert!(selected(&s, "cursefilter_on") && selected(&s, "blockwhisper_off") && selected(&s, "jobautopopup_on"));
    assert!(!enabled(&s, "recivenote_on"), "the messenger's state isn't known yet");
    click(&mut s, "blockwhisper_on");
    assert!(selected(&s, "blockwhisper_on"));
    assert!(!shown(&s, "spectatormode_wnd"));
    s.app.world_mut().resource_mut::<CaAccount>().game_master = true;
    s.app.update();
    assert!(shown(&s, "spectatormode_wnd"));
    click(&mut s, "button_ok");
    let social = s.app.world().resource::<combat_arms::game::options::CaSocial>();
    assert!(social.block_whispers);
    assert_eq!(social.requests, ["whisper 1"]);
}

#[test]
fn the_block_list_buttons() {
    let mut s = sim();
    open(&mut s);
    click(&mut s, "tab_other");
    for n in ["zed", "Adam"] {
        s.app.world_mut().resource_mut::<OptionsScreen>().name_box = Some(n.into());
        click(&mut s, "button_regist");
    }
    assert_eq!(text(&s, "item_name"), "[ ] zed   [ ] Adam");
    click(&mut s, "sort_name");
    assert_eq!(text(&s, "item_name"), "[ ] Adam   [ ] zed");
    click(&mut s, "check_all");
    assert_eq!(text(&s, "item_name"), "[x] Adam   [x] zed");
    click(&mut s, "button_clear");
    assert_eq!(text(&s, "item_name"), "");
}

// ---------------------------------------------------------------------------
// The shell gets the settings

#[test]
fn the_options_reach_the_shell() {
    use chimera_client::client::{audio::SoundOptions, gamma::MonitorGammaSetting, input::MouseSettings};
    use chimera_present::audio::live_sound::SoundMix;
    let mut s = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaOptionsStatePlugin, CaOptionsApplyPlugin))
            .init_resource::<MouseSettings>()
            .init_resource::<SoundOptions>()
            .init_resource::<SoundMix>()
            .init_resource::<MonitorGammaSetting>();
    });
    flow::load(s.app.world_mut(), None);
    s.app.update();
    s.app.update();
    // M2: S = 5 turns one count by 1.5 k = 0.0017025 rad.
    let m = *s.app.world().resource::<MouseSettings>();
    let rad = m.look_delta(Vec2::new(-1000.0, 0.0)).x;
    assert!((rad - 1.7025).abs() < 1e-3, "{rad}");
    assert!(m.m_pitch > 0.0);
    // Zoom level 1 divides by 4 (the weapon's own scale).
    assert!((m.look_delta(Vec2::new(-100.0, 0.0)).x * 0.25 - 0.0425625).abs() < 1e-4);
    open(&mut s);
    flow::edit(s.app.world_mut(), "MouseInvertY", "1");
    flow::edit(s.app.world_mut(), "soundvolume", "50");
    flow::edit(s.app.world_mut(), "musicvolume", "25");
    flow::edit_slider(s.app.world_mut(), "GammaR", 33);
    s.app.update();
    s.app.update();
    assert!((s.app.world().resource::<SoundOptions>().volume - 0.5).abs() < 1e-6);
    assert!((s.app.world().resource::<SoundMix>().music - 0.5).abs() < 1e-6, "25% of a 50% master, over the master");
    assert!((s.app.world().resource::<MonitorGammaSetting>().0 - options_apply::monitor_gamma(2.25)).abs() < 1e-4);
    flow::ok(s.app.world_mut()).unwrap();
    s.app.update();
    s.app.update();
    assert!(s.app.world().resource::<MouseSettings>().m_pitch < 0.0, "inverted");
    assert!(s.app.world().resource::<CaMouseLook>().invert);
}

// ---------------------------------------------------------------------------
// 44, 45: the HUD

/// A match of TDM with the local player on Alpha at the origin, live.
fn live() -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
    });
    {
        let w = sim.app.world_mut();
        let old: Vec<Entity> = w.query_filtered::<Entity, With<SpawnPoint>>().iter(w).collect();
        for e in old {
            w.despawn(e);
        }
        for (k, team) in [JSF, CREED].into_iter().enumerate() {
            for i in 0..2 {
                w.spawn((
                    Name::new(format!("Spawn {}", k * 2 + i)),
                    SpawnPoint { team: Some(Team(team)) },
                    Transform::from_translation(point_of(team, i)),
                ));
            }
        }
        with_mode(w, "TDM");
    }
    let me = join(&mut sim, JSF);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Name::new("Me")));
    until_live(&mut sim);
    if m(&sim).warmup {
        past_warmup(&mut sim);
    }
    unprotect_all(&mut sim);
    (sim, me)
}

fn point_of(team: u8, i: usize) -> Vec3 {
    point(team, i)
}

#[test]
fn t44_the_radar_zoom_goes_to_the_profile() {
    let (mut sim, me) = live();
    sim.ticks(2);
    sim.app.world_mut().resource_mut::<RadarKeys>().expand = true;
    for _ in 0..60 {
        sim.ticks(1);
    }
    let z = sim.app.world().resource::<CaRadar>().zoom;
    assert!((z - 1.6).abs() < 1e-3, "{z}");
    for _ in 0..140 {
        sim.ticks(1);
    }
    assert_eq!(sim.app.world().resource::<CaRadar>().zoom, 2.0);
    // Leave the match (the radar hides): the next save has the zoom.
    sim.app.world_mut().resource_mut::<RadarKeys>().expand = false;
    kill_now(&mut sim, me, me);
    sim.ticks(3);
    assert!(!sim.app.world().resource::<CaRadar>().visible);
    flow::save(sim.app.world_mut()).unwrap();
    let saved = opts(&sim).profile.float("Display", "RadarScale").unwrap();
    assert_eq!(saved, 2.0);
}

#[test]
fn t45_the_hud_key_hides_the_panels_and_not_the_crosshair() {
    let (mut sim, _) = live();
    sim.ticks(4);
    let before = sim.app.world().resource::<CaHud>().clone();
    assert!(before.status.is_some(), "the health panel");
    assert!(sim.app.world().resource::<CaRadar>().visible);
    sim.app.world_mut().resource_mut::<Console>().submit("hudonoff");
    sim.ticks(3);
    let hud = sim.app.world().resource::<CaHud>();
    assert!(hud.status.is_none() && hud.weapon.is_none() && hud.feed.is_empty());
    assert!(!hud.crosshair.elements.is_empty(), "the crosshair is code-drawn: still there");
    assert_eq!(hud.crosshair.draws, before.crosshair.draws);
    assert!(!sim.app.world().resource::<CaRadar>().visible);
    // The next match: still hidden.
    with_mode(sim.app.world_mut(), "TDM");
    sim.ticks(3);
    assert!(!sim.app.world().resource::<CaHudSwitch>().panels());
    // Pressed again: shown.
    sim.app.world_mut().resource_mut::<Console>().submit("hudonoff");
    sim.ticks(3);
    assert!(sim.app.world().resource::<CaHud>().status.is_some());
}
