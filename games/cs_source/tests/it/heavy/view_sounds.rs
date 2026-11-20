//! specs/cs_source/view_models.md 6, sounds.md ("Model animation sound
//! events"), mdl_v48.md (the AWP's): every CS:S gun's view-model sound
//! events (event 5004) as the install's models carry them, at their time
//! into each sequence (cycle x length), and every entry named in the gun's
//! data (`[sounds] view`, what a map loads) as the owner hears them. The
//! timed reload, draw and bolt sounds the data used to hold were these.
//! Skips without a CS:S install.

use std::collections::BTreeSet;

use cs_source::game::{INFO, material::MaterialLoader, view_anim, weapons};

/// (gun, sequence, [(seconds into it, entry)]): each sound event of the
/// guns' view models (the C4's clicks are the bomb's, `view_anim::
/// event_effect`; the grenades' pin pull at cycle 0.6923, grenades.md 2).
const EVENTS: &[(&str, &str, &[(f32, &str)])] = &[
    ("ak47", "ak47_draw", &[(0.3667, "Weapon_AK47.BoltPull")]),
    ("ak47", "ak47_reload", &[(0.3514, "Weapon_AK47.Clipout"), (1.5405, "Weapon_AK47.Clipin")]),
    ("m4a1", "reload", &[(0.6757, "Weapon_M4A1.Clipout"), (1.4324, "Weapon_M4A1.Clipin"), (2.3784, "Weapon_M4A1.Boltpull")]),
    ("m4a1", "draw", &[(0.0250, "Weapon_M4A1.Deploy"), (0.4250, "Weapon_M4A1.Boltpull")]),
    ("m4a1", "add_silencer", &[(0.9333, "Weapon_M4A1.Silencer_On")]),
    ("m4a1", "reload_unsil", &[(0.6757, "Weapon_M4A1.Clipout"), (1.4324, "Weapon_M4A1.Clipin"), (2.3784, "Weapon_M4A1.Boltpull")]),
    ("m4a1", "draw_unsil", &[(0.0250, "Weapon_M4A1.Deploy"), (0.4250, "Weapon_M4A1.Boltpull")]),
    ("m4a1", "detach_silencer", &[(0.7000, "Weapon_M4A1.Silencer_Off")]),
    ("awp", "awm_fire", &[(0.5000, "Weapon_AWP.Bolt"), (0.6333, "Weapon_AWP.Bolt"), (1.0667, "Weapon_AWP.Bolt")]),
    ("awp", "awm_reload", &[(0.8667, "Weapon_AWP.Clipout"), (1.6667, "Weapon_AWP.Clipin"), (2.9667, "Weapon_AWP.Bolt")]),
    ("usp", "reload", &[(0.0000, "Weapon_USP.Slideback2"), (0.4595, "Weapon_USP.Clipout"), (1.0811, "Weapon_USP.Clipin"), (2.2162, "Weapon_USP.Sliderelease")]),
    ("usp", "draw", &[(0.5417, "Weapon_USP.Slideback")]),
    ("usp", "add_silencer", &[(1.0270, "Weapon_USP.AttachSilencer")]),
    ("usp", "reload_unsil", &[(0.0000, "Weapon_USP.Slideback2"), (0.4595, "Weapon_USP.Clipout"), (1.0811, "Weapon_USP.Clipin"), (2.2162, "Weapon_USP.Sliderelease")]),
    ("usp", "draw_unsil", &[(0.5417, "Weapon_USP.Slideback")]),
    ("usp", "detach_silencer", &[(0.7838, "Weapon_USP.DetachSilencer")]),
    ("glock", "glock_draw", &[(0.3778, "Weapon_Glock.Sliderelease")]),
    ("glock", "glock_reload", &[(0.0000, "Weapon_Glock.Slideback"), (0.4000, "Weapon_Glock.Clipout"), (1.0857, "Weapon_Glock.Clipin"), (1.8286, "Weapon_Glock.Sliderelease")]),
    ("deagle", "reload", &[(0.0000, "Weapon_DEagle.Slideback"), (0.4667, "Weapon_DEagle.Clipout"), (1.1333, "Weapon_DEagle.Clipin")]),
    ("deagle", "draw", &[(0.0333, "Weapon_DEagle.Deploy")]),
    ("famas", "reload", &[(0.5185, "Weapon_FAMAS.Clipout"), (1.4815, "Weapon_FAMAS.Clipin"), (2.3333, "Weapon_FAMAS.Forearm")]),
    ("galil", "reload", &[(0.3810, "Weapon_Galil.Clipout"), (1.3095, "Weapon_Galil.Clipin"), (2.1667, "Weapon_Galil.Boltpull")]),
    ("aug", "draw", &[(0.3000, "Weapon_AUG.Forearm")]),
    ("aug", "reload", &[(0.2857, "Weapon_AUG.Boltpull"), (1.4286, "Weapon_AUG.Clipout"), (2.5143, "Weapon_AUG.Clipin"), (3.2000, "Weapon_AUG.Boltslap")]),
    ("sg552", "reload", &[(0.4324, "Weapon_SG552.Clipout"), (1.6486, "Weapon_SG552.Clipin"), (2.4324, "Weapon_SG552.Boltpull")]),
    ("sg552", "draw", &[(0.3243, "Weapon_SG552.Boltpull")]),
    ("scout", "shoot", &[(0.3714, "Weapon_Scout.Bolt")]),
    ("scout", "reload", &[(0.4333, "Weapon_Scout.Clipout"), (1.2333, "Weapon_Scout.Clipin"), (1.7333, "Weapon_Scout.Bolt")]),
    ("sg550", "reload", &[(0.7857, "Weapon_SG550.Clipout"), (1.6429, "Weapon_SG550.Clipin"), (2.9286, "Weapon_SG550.Boltpull")]),
    ("g3sg1", "reload", &[(0.5333, "Weapon_G3SG1.Slide"), (1.8000, "Weapon_G3SG1.Clipout"), (2.8333, "Weapon_G3SG1.Clipin"), (3.8667, "Weapon_G3SG1.Slide")]),
    ("mac10", "mac10_reload", &[(0.6286, "Weapon_MAC10.Clipout"), (1.5714, "Weapon_MAC10.Clipin"), (2.4857, "Weapon_MAC10.Boltpull")]),
    ("tmp", "reload", &[(0.4800, "Weapon_TMP.Clipout"), (1.2800, "Weapon_TMP.Clipin")]),
    ("tmp", "draw", &[(0.5000, "Weapon_TMP.Deploy")]),
    ("mp5navy", "reload1", &[(0.3158, "Weapon_MP5Navy.Clipout"), (1.1579, "Weapon_MP5Navy.Clipin"), (2.2632, "Weapon_MP5Navy.Slideback")]),
    ("mp5navy", "draw", &[(0.3714, "Weapon_MP5Navy.Slideback")]),
    ("ump45", "reload", &[(0.6970, "Weapon_UMP45.Clipout"), (1.7879, "Weapon_UMP45.Clipin"), (2.6061, "Weapon_UMP45.Boltslap")]),
    ("p90", "reload", &[(0.4250, "Weapon_P90.Cliprelease"), (0.8750, "Weapon_P90.Clipout"), (1.8750, "Weapon_P90.Clipin"), (2.7000, "Weapon_P90.Boltpull")]),
    ("p90", "draw", &[(0.3000, "Weapon_P90.Boltpull")]),
    ("m249", "reload", &[(0.4667, "Weapon_M249.Boxout"), (1.5000, "Weapon_M249.Coverup"), (2.6667, "Weapon_M249.Boxin"), (3.3000, "Weapon_M249.Chain"), (4.5000, "Weapon_M249.Coverdown")]),
    ("p228", "reload", &[(0.0000, "Weapon_P228.Slideback"), (0.6857, "Weapon_P228.Clipout"), (1.4000, "Weapon_P228.Clipin"), (2.3143, "Weapon_P228.Sliderelease")]),
    ("p228", "draw", &[(0.5000, "Weapon_P228.Slidepull")]),
    ("fiveseven", "reload", &[(0.0000, "Weapon_FiveSeven.Slideback"), (0.5000, "Weapon_FiveSeven.Clipout"), (1.3667, "Weapon_FiveSeven.Clipin"), (2.5000, "Weapon_FiveSeven.Sliderelease")]),
    ("fiveseven", "draw", &[(0.4333, "Weapon_FiveSeven.Slidepull")]),
    ("elite", "reload", &[(0.0000, "Weapon_ELITE.Reloadstart"), (0.4800, "Weapon_ELITE.Clipout"), (1.4400, "Weapon_ELITE.Rclipin"), (2.3600, "Weapon_ELITE.Lclipin"), (3.2400, "Weapon_ELITE.Sliderelease")]),
    ("elite", "draw", &[(0.0333, "Weapon_ELITE.Deploy")]),
    ("m3", "insert", &[(0.0000, "Weapon_M3.Insertshell")]),
    ("m3", "after_reload", &[(0.3000, "Weapon_M3.Pump")]),
    ("m3", "draw", &[(0.3667, "Weapon_M3.Pump")]),
    ("xm1014", "insert", &[(0.0222, "Weapon_XM1014.InsertShell")]),
    ("xm1014", "draw", &[(0.0333, "Weapon_DEagle.Deploy")]),
    ("hegrenade", "pullpin", &[(0.6585, "Default.PullPin_Grenade")]),
    ("hegrenade", "pullpin2", &[(0.6585, "Default.PullPin_Grenade")]),
    ("hegrenade", "pullpin3", &[(0.6585, "Default.PullPin_Grenade")]),
    ("hegrenade", "pullpin4", &[(0.6585, "Default.PullPin_Grenade")]),
    ("flashbang", "pullpin", &[(0.6585, "Default.PullPin_Grenade")]),
    ("flashbang", "pullpin2", &[(0.6585, "Default.PullPin_Grenade")]),
    ("flashbang", "pullpin3", &[(0.6585, "Default.PullPin_Grenade")]),
    ("flashbang", "pullpin4", &[(0.6585, "Default.PullPin_Grenade")]),
    ("smokegrenade", "pullpin", &[(0.6585, "Default.PullPin_Grenade")]),
    ("smokegrenade", "pullpin2", &[(0.6585, "Default.PullPin_Grenade")]),
    ("smokegrenade", "pullpin3", &[(0.6585, "Default.PullPin_Grenade")]),
    ("smokegrenade", "pullpin4", &[(0.6585, "Default.PullPin_Grenade")]),
];

#[test]
fn the_view_models_sound_events_are_the_guns_reload_draw_and_bolt_sounds() {
    let Some(mount) = INFO.mount() else {
        eprintln!("skipping: no CS:S install configured");
        return;
    };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let mut wrong = Vec::new();
    let mut seen = BTreeSet::new();
    for (id, path, rh) in weapons::view_models() {
        let v = source_engine::content::props::load_view_model(&mut materials, path, id, rh).unwrap_or_else(|e| panic!("{id}: {e}"));
        let set = v.animations.as_ref().unwrap();
        let short = id.trim_start_matches("cs_source:weapon_");
        let data = weapons::guns().find(|g| g.id == id);
        let mut named = BTreeSet::new();
        for (i, q) in set.sequences.iter().enumerate() {
            let len = set.duration(i);
            let got: Vec<(f32, String)> = q
                .events
                .iter()
                .filter_map(|e| match view_anim::event_effect(e) {
                    Some(chimera_world::map::ViewModelEventKind::Sound { entry }) => Some((e.cycle * len, entry)),
                    _ => None,
                })
                .collect();
            let want = EVENTS.iter().find(|(g, s, _)| *g == short && *s == q.name).map_or(&[][..], |r| r.2);
            if got.len() != want.len() || got.iter().zip(want).any(|((t, e), (wt, we))| (t - wt).abs() > 0.006 || e != we) {
                wrong.push(format!("{short} {}: {got:?}, expected {want:?}", q.name));
            }
            named.extend(got.into_iter().map(|(_, e)| e));
            seen.insert((short.to_string(), q.name.clone()));
        }
        // The grenades' pin pull is loaded with the grenades' sounds.
        named.remove("Default.PullPin_Grenade");
        // Every entry is in the gun's data, so a map loads it; the data
        // names no other.
        let listed: BTreeSet<String> = data.and_then(|d| d.sounds.as_ref()).map(|s| s.view.iter().cloned().collect()).unwrap_or_default();
        if listed != named {
            wrong.push(format!("{short}: [sounds] view {listed:?}, the events {named:?}"));
        }
        // The data times none of them any more.
        if let Some(s) = data.and_then(|d| d.sounds.as_ref())
            && (!s.reload.is_empty() || !s.draw.is_empty() || !s.shot.is_empty() || !s.finish.is_empty() || !s.modes.is_empty())
        {
            wrong.push(format!("{short}: the data still times view-model sounds"));
        }
    }
    for (g, s, _) in EVENTS {
        if !seen.contains(&(g.to_string(), s.to_string())) {
            wrong.push(format!("{g} {s}: no such sequence"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
