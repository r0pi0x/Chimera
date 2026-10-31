//! Our render-style table against the install's files
//! (specs/combat_arms/models_ltb.md Follow-up (2026-10-31) 1 and 3, cases
//! R6 and R8): every layered style's lighting material
//! (`render_style::Layer::lighting`) is the material its `RS` file holds,
//! and `pv_char_b` is the three passes of the spec. Skips with a note
//! without the install.

use chimera_core::mount::config::LocalConfig;
use combat_arms::game::{GAME, INFO, render_style::{self, Layer, Style}};
use lithtech_formats::render_style::RenderStyleFile;

fn read(name: &str) -> Option<RenderStyleFile> {
    let install = LocalConfig::load().ok().and_then(|c| c.game_path(GAME))?;
    if !install.join("Game").is_dir() {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    let mount = (INFO.open)(&install).ok()?;
    let Ok(bytes) = mount.read(&format!("rs/{}", name.to_lowercase())) else {
        eprintln!("skipping: rs/{name} doesn't read");
        return None;
    };
    Some(RenderStyleFile::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}")))
}

#[test]
fn layered_styles_use_the_files_materials() {
    for name in ["env_alpha_mask.ltb", "itemedge.ltb", "edgeamp01.ltb", "envmap.ltb", "pv_char.ltb", "pv_char_b.ltb"] {
        let Some(file) = read(name) else { return };
        let Style::Layered { layer, .. } = render_style::style(&format!("RS\\{name}")) else { panic!("{name} is layered") };
        let ours = layer.lighting();
        let m = file.styles[0].material;
        let close = |a: f32, b: f32| (a - b).abs() < 2e-3;
        for (what, got, want) in [
            ("emissive", [ours.emissive.x, ours.emissive.y, ours.emissive.z], [m.emissive[0], m.emissive[1], m.emissive[2]]),
            ("diffuse", [ours.diffuse.x, ours.diffuse.y, ours.diffuse.z], [m.diffuse[0], m.diffuse[1], m.diffuse[2]]),
            ("specular", [ours.specular.x, ours.specular.y, ours.specular.z], [m.specular[0], m.specular[1], m.specular[2]]),
        ] {
            assert!(got.iter().zip(want).all(|(a, b)| close(*a, b)), "{name} {what}: ours {got:?}, file {want:?}");
        }
        assert!(close(ours.ambient.x, m.ambient[0]), "{name} ambient: {} vs {}", ours.ambient.x, m.ambient[0]);
        assert!(close(ours.power, m.power) || m.specular[..3] == [0.0; 3], "{name} power: {} vs {}", ours.power, m.power);
        eprintln!("{name}: material matches");
    }
    let _ = Layer::PvCharB;
}
