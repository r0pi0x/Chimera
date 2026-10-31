//! The view model's attached effects in play (specs/combat_arms/
//! view_models.md 3, case 44; `view_model::attached`) and the chainsaw's
//! sounds (special_weapons.md 5 and case 37, effects_sounds.md 11,
//! view_models.md 8): copies made from an install written here (a
//! chainsaw and an XK-2-like bomb, the names ours), their view models and
//! lengths written in the test (the stock files' keys, read by
//! `heavy::attached_fx`), the effect file's idle group written here.
//!
//! - The chainsaw's idle group exists while it is drawn, shows with the
//!   model, loops its engine sound (one long-lived looping start, no
//!   restarts), hides behind a scope, goes with a holster (the sound
//!   stopped).
//! - The bomb's five groups: hidden by `HIDE_PVATTACHFX 1`, still hidden
//!   after a scope, shown again by `SHOW_PVATTACHFX 1`.
//! - The secondary held 2 s: one `sound_key 1` (`ReloadSnd1`) per 900 ms
//!   swing and one fire sound per hit key (7 a swing), not one a tick.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Health, LocalPlayer};
use chimera_sim::weapon::{self, Inventory, data::WeaponDataAppExt};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{SoundControl, ViewModels},
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        data::{attributes::WEAPONS_FILE, merge_weapon_docs, read_attributes},
        effects::{
            self,
            library::set_library,
            sounds::CaSound,
        },
        movement,
        view_model::{
            attached::{CaAttachedFx, CaLoopSound},
            machine::AnimInfo,
        },
        weapons::{
            CaZoom,
            zoom::ZoomParams,
            timing::{AnimTimes, ViewTimes},
        },
    },
    harness::CaSim,
};
use toml::{Table, Value};

use super::{attribute_data::meters_per_unit, view_model::view_model};

const INSTALL: &str = r#"
[Ammo0] Name="knife" Type=1 MaxAmount=1000 InstDamage=45 AltInstDamage=75
[Ammo1] Name="saw" Type=1 MaxAmount=1000 InstDamage=75 AltInstDamage=15
[Weapon0] Name="M9" Guntype=0 AmmoName0="knife" PVModelNormal="GUNS_M_PV_MELEE\PV_Melee_M9_SH.ltb"
[Weapon1] Name="Test Saw" Guntype=0 AmmoName0="saw" Range=150 PVModelNormal="TEST\Saw\PV_Saw.ltb"
    FireSnd="TEST_SND\Saw\fire.wav" Alt_FireSnd="TEST_SND\Saw\fire.wav" ReloadSnd1="TEST_SND\Saw\rev.wav"
    SelectSnd="TEST_SND\Saw\select.wav" PVAttachFXName0="Test_Saw_Idle" PVAttachFXAlwaysShow=1
[Weapon2] Name="Test Bomb" Guntype=0 AmmoName0="knife" PVModelNormal="TEST\Bomb\PV_Bomb.ltb"
    PVAttachFXName0="Test_Green" PVAttachFXName1="Test_Red" PVAttachFXName2="Test_Yellow1"
    PVAttachFXName3="Test_Yellow2" PVAttachFXName4="Test_Yellow3"
"#;

const SAW: &str = "combat_arms:test_saw";
const BOMB: &str = "combat_arms:test_bomb";
const IDLE_WAV: &str = "test_snd/saw/idle.wav";

/// The idle group: one looping, local engine sound over its 3 s (the
/// stock `Chainsaw_PV_idle`'s shape).
pub(super) const FX: &str = "Groups: 1\r\n\tGroupName: Test_Saw_Idle\r\n\tFxInGroup: 1\r\n\tPhase: 3000\r\n\
\t\tFxName: PlaySound;\r\n\t\tFxID: 1\r\n\t\tLinked: 0\r\n\t\tLinkID: 0\r\n\t\tLinkNode:\r\n\
\t\tStartTime: 0.0\r\n\t\tEndTime: 3.0\r\n\t\tRepeat: 1\r\n\t\tTrackID: 0\r\n\t\tMinScale: 0\r\n\
\t\tMaxScale: 1\r\n\t\tProperties: 6\r\n\
\t\t\tPropName: UpdatePos\r\n\t\t\tType: 3\r\n\t\t\tCOMBO: 6,Fixed,Follow\r\n\
\t\t\tPropName: AttachName\r\n\t\t\tType: 0\r\n\t\t\tSTRING: motiontrail\r\n\
\t\t\tPropName: Sound\r\n\t\t\tType: 7\r\n\t\t\tPATH: wav|TEST_SND\\SAW\\IDLE.WAV\r\n\
\t\t\tPropName: Loop\r\n\t\t\tType: 3\r\n\t\t\tCOMBO: 1,No,Yes\r\n\
\t\t\tPropName: PlayLocal\r\n\t\t\tType: 3\r\n\t\t\tCOMBO: 1,No,Yes\r\n\
\t\t\tPropName: Volume\r\n\t\t\tType: 1\r\n\t\t\tINT: 100\r\n";

fn a(name: &str, length: u32, keys: &[(u32, &str)]) -> AnimInfo {
    AnimInfo::new(name, length, keys)
}

/// The stock chainsaw's view model (`heavy::attached_fx`'s census):
/// `alt_fire_0` 900 ms, `sound_key 1` at 0 and seven `ALT_FIRE_KEY`s.
fn saw_anims() -> Vec<AnimInfo> {
    vec![
        a("select", 1000, &[(0, "sound_key 4")]),
        a("idle_0", 1833, &[]),
        a("idle_1", 1833, &[]),
        a("prefire", 0, &[]),
        a("postfire", 0, &[]),
        a("fire_0", 600, &[(257, "FIRE_KEY")]),
        a("alt_fire_0", 900, &[
            (0, "sound_key 1"),
            (334, "ALT_FIRE_KEY"),
            (386, "ALT_FIRE_KEY"),
            (437, "ALT_FIRE_KEY"),
            (489, "ALT_FIRE_KEY"),
            (540, "ALT_FIRE_KEY"),
            (591, "ALT_FIRE_KEY"),
            (643, "ALT_FIRE_KEY"),
        ]),
        a("PreRush", 266, &[]),
        a("rush", 633, &[]),
        a("PostRush", 267, &[]),
    ]
}

/// A bomb whose swing hides light 1 and shows it again.
fn bomb_anims() -> Vec<AnimInfo> {
    vec![
        a("select", 300, &[]),
        a("idle_0", 2000, &[]),
        a("prefire", 0, &[]),
        a("postfire", 0, &[]),
        a("fire_0", 600, &[(0, "HIDE_PVATTACHFX 1"), (200, "FIRE_KEY")]),
        a("alt_fire_0", 600, &[(0, "SHOW_PVATTACHFX 1"), (200, "ALT_FIRE_KEY")]),
    ]
}

fn times(app: &mut App) {
    let mut t = app.world_mut().get_resource_or_init::<ViewTimes>();
    t.set("test/saw/pv_saw.ltb", AnimTimes {
        fire: Some(0.6),
        alt_fire: Some(0.9),
        select: Some(1.0),
        fire_keys: vec![0.257],
        alt_fire_keys: vec![0.334, 0.386, 0.437, 0.489, 0.540, 0.591, 0.643],
        ..default()
    });
    t.set("test/bomb/pv_bomb.ltb", AnimTimes {
        fire: Some(0.6),
        alt_fire: Some(0.6),
        select: Some(0.3),
        fire_keys: vec![0.2],
        alt_fire_keys: vec![0.2],
        ..default()
    });
}

fn copies() -> Vec<(String, String)> {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(INSTALL.as_bytes().to_vec()),
        _ => Ok(Vec::new()),
    });
    let weapons = attrs.weapons.as_ref().unwrap_or_else(|| panic!("{problems:?}"));
    let ours = [("m9.toml", include_str!("../../data/weapons/m9.toml"))];
    let docs: Vec<(String, Table)> = merge_weapon_docs(&ours, weapons, meters_per_unit())
        .unwrap()
        .into_iter()
        .map(|(n, t)| (n, t.parse().unwrap()))
        .collect();
    [SAW, BOMB]
        .iter()
        .map(|id| {
            let t = &docs
                .iter()
                .find(|(_, t)| t.get("id").and_then(Value::as_str) == Some(id))
                .unwrap_or_else(|| panic!("{id}"))
                .1;
            (id.to_string(), toml::to_string(t).unwrap())
        })
        .collect()
}

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>, Vec<CaLoopSound>, Vec<SoundControl>);

fn sim() -> (Sim, Entity) {
    // One library for every test of the binary (they run at once).
    set_library(Some(super::attached_effects::test_library()));
    let copies = copies();
    let mut sim = Sim::with(CaSim, move |app| {
        app.add_plugins(GreyboxMapPlugin);
        times(app);
        let refs: Vec<(&str, &str)> = copies.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        app.add_weapon_data(&refs).unwrap();
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.insert_resource(ViewModels(Arc::new(vec![
        view_model(SAW, &saw_anims()),
        view_model(BOMB, &bomb_anims()),
    ])));
    sim.app.init_resource::<Heard>();
    sim.app.add_systems(
        Last,
        |mut s: MessageReader<CaSound>, mut l: MessageReader<CaLoopSound>, mut c: MessageReader<SoundControl>, mut h: ResMut<Heard>| {
            h.0.extend(s.read().cloned());
            h.1.extend(l.read().cloned());
            h.2.extend(c.read().cloned());
        },
    );
    let p = sim.spawn_character(Vec3::new(30.0, 1.0, 30.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert(Health { current: 1.0, max: 1.0 });
    (sim, p)
}

fn heard(sim: &mut Sim) -> Heard {
    std::mem::take(&mut *sim.app.world_mut().resource_mut::<Heard>())
}

fn attached(sim: &Sim, p: Entity) -> CaAttachedFx {
    sim.app.world().get::<CaAttachedFx>(p).cloned().unwrap_or_default()
}

fn shown(sim: &Sim, p: Entity) -> Vec<bool> {
    attached(sim, p).effects.iter().map(|e| e.shown).collect()
}

/// The weapon zoomed to `level` (a melee weapon has no zoom of its own:
/// one is put on it), as a scope does (11: the model hides from level 1).
fn scope(sim: &mut Sim, w: Entity, level: u8) {
    let mut z = CaZoom::new(ZoomParams::default());
    z.level = level;
    sim.app.world_mut().entity_mut(w).insert(z);
}

fn draw(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w));
    w
}

/// The chainsaw's idle group: made with the view model, shown from the
/// draw (`PVAttachFXAlwaysShow`), its engine sound one looping start that
/// isn't restarted while it shows; hidden behind a scope (the sound
/// stopped, started again after); gone with a holster.
#[test]
fn the_chainsaw_idles_while_drawn() {
    let (mut sim, p) = sim();
    let saw = draw(&mut sim, p, SAW);
    let fx = attached(&sim, p);
    assert_eq!(fx.weapon, Some(saw));
    assert_eq!(fx.effects.len(), 1);
    assert_eq!(fx.effects[0].group, "Test_Saw_Idle");
    assert_eq!(shown(&sim, p), vec![true], "shown with the model");
    let h = heard(&mut sim);
    let starts: Vec<&CaLoopSound> = h.1.iter().filter(|l| l.sound.is_some()).collect();
    assert_eq!(starts.len(), 1, "one looping start: {:?}", h.1);
    let s = starts[0].sound.as_ref().unwrap();
    assert_eq!((s.file.as_str(), s.at), (IDLE_WAV, None), "local");
    // Still idling 3 s later (past the group's 3 s phase): no restart, no stop.
    sim.seconds(3.0);
    let h = heard(&mut sim);
    assert!(h.1.is_empty(), "nothing restarted or stopped: {:?}", h.1);
    assert_eq!(shown(&sim, p), vec![true]);
    // A scope hides the model and its effects (11); the sound stops.
    scope(&mut sim, saw, 1);
    sim.ticks(2);
    assert_eq!(shown(&sim, p), vec![false], "hidden with the model");
    let h = heard(&mut sim);
    assert!(h.1.iter().any(|l| l.sound.is_none()), "stopped: {:?}", h.1);
    scope(&mut sim, saw, 0);
    sim.ticks(2);
    assert_eq!(shown(&sim, p), vec![true], "shown again");
    assert_eq!(heard(&mut sim).1.iter().filter(|l| l.sound.is_some()).count(), 1, "started again");
    // Holstered: the effects go with the model, the sound stops.
    let bomb = draw(&mut sim, p, BOMB);
    let fx = attached(&sim, p);
    assert_eq!(fx.weapon, Some(bomb));
    assert!(fx.effects.iter().all(|e| !e.group.eq_ignore_ascii_case("Test_Saw_Idle")));
    let h = heard(&mut sim);
    assert!(h.1.iter().any(|l| l.sound.is_none()), "the idle stopped: {:?}", h.1);
    assert!(h.1.iter().all(|l| l.sound.is_none()), "the bomb's groups have no sound here");
}

/// Case 44 in play: the bomb's five groups show with the model; a
/// `HIDE_PVATTACHFX 1` key hides the second, which stays hidden through a
/// scope until a `SHOW_PVATTACHFX 1` key.
#[test]
fn t44_the_bombs_lights_follow_their_keys() {
    let (mut sim, p) = sim();
    let bomb = draw(&mut sim, p, BOMB);
    let fx = attached(&sim, p);
    assert_eq!(
        fx.effects.iter().map(|e| e.group.as_str()).collect::<Vec<_>>(),
        ["Test_Green", "Test_Red", "Test_Yellow1", "Test_Yellow2", "Test_Yellow3"]
    );
    assert_eq!(shown(&sim, p), vec![true; 5]);
    sim.intent(p).fire = true;
    sim.ticks(10);
    sim.intent(p).fire = false;
    sim.seconds(1.0);
    assert_eq!(shown(&sim, p), vec![true, false, true, true, true], "light 1 hidden by its key");
    scope(&mut sim, bomb, 1);
    sim.ticks(2);
    assert_eq!(shown(&sim, p), vec![false; 5], "all hidden with the model");
    scope(&mut sim, bomb, 0);
    sim.ticks(2);
    assert_eq!(shown(&sim, p), vec![true, false, true, true, true], "still hidden after the scope");
    sim.intent(p).secondary = true;
    sim.ticks(10);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    assert_eq!(shown(&sim, p), vec![true; 5], "shown by its key");
}

/// The chainsaw's secondary held 2 s: swings every 900 ms; each plays
/// `sound_key 1` (its `ReloadSnd1`) once at its start and the fire sound
/// once per hit key (7), never once per tick.
#[test]
fn the_chainsaws_secondary_sounds_per_swing_and_hit() {
    let (mut sim, p) = sim();
    let saw = draw(&mut sim, p, SAW);
    heard(&mut sim);
    sim.intent(p).secondary = true;
    sim.seconds(2.0);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    let h = heard(&mut sim);
    let count = |f: &str| h.0.iter().filter(|s| s.file == f).count();
    let (revs, fires) = (count("test_snd/saw/rev.wav"), count("test_snd/saw/fire.wav"));
    let swings = 3; // at 0, 0.9 and 1.8 s (the third started before the release)
    assert_eq!(revs, swings, "sound_key 1 once a swing: {:?}", h.0);
    assert_eq!(fires, swings * 7, "the fire sound once a hit: {:?}", h.0);
    // One fire voice (the attacker's with this weapon): each hit's
    // replaces the last (no pile-up of the stock chainsaw's 2.9 s file).
    for s in h.0.iter().filter(|s| s.file == "test_snd/saw/fire.wav") {
        assert_eq!((s.source, s.channel), (Some(saw), Some(effects::sounds::CHANNEL_FIRE)));
    }
    // The idle loop wasn't restarted by the swings.
    assert!(h.1.is_empty(), "{:?}", h.1);
}
