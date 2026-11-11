//! The front end's per-frame cost doesn't grow with the string table
//! while its screens are down: the options screen, the lobby and the chat
//! panel each cloned `CaUi` (the install's string table with it) every
//! frame, about 16 ms a frame in Lucker Party (docs/performance.md,
//! "Lucker Party"). Headless on `CaSim` with the screens' state plugins;
//! frames with a table of 200 000 extra strings against frames with ours,
//! by the frames' instructions (`metrics::work::now`, the same on every
//! run, idle or loaded). Skips the bound without hardware counters.

use chimera_core::metrics::work;

use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    harness::CaSim,
    ui::{CaUi, lobby::CaLobbyStatePlugin, menu::CaMenuStatePlugin, options::CaOptionsStatePlugin, talk::CaTalkStatePlugin},
};

const FRAMES: u32 = 20;

fn frames(extra_strings: usize) -> Option<u64> {
    let mut s = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaMenuStatePlugin, CaLobbyStatePlugin, CaOptionsStatePlugin, CaTalkStatePlugin));
    });
    {
        let mut ui = s.app.world_mut().resource_mut::<CaUi>();
        for i in 0..extra_strings {
            ui.strings.values.insert(format!("Neutral_String_{i}"), format!("Some text number {i}"));
        }
    }
    s.app.update();
    let before = work::now()?;
    for _ in 0..FRAMES {
        s.app.update();
    }
    Some(work::now()?.saturating_sub(before))
}

#[test]
fn screens_that_are_down_cost_nothing_per_string() {
    let (Some(ours), Some(big)) = (frames(0), frames(200_000)) else {
        eprintln!("skipping: no hardware counters ({})", chimera_core::metrics::hw_status());
        return;
    };
    eprintln!("{FRAMES} frames: {big} instructions with a big string table, {ours} with ours");
    // Three clones of the big table a frame took ~50 ms (1 s for the
    // frames); a frame without them is the same either way.
    assert!(
        big < ours + ours / 2,
        "{FRAMES} frames: {big} instructions with a big string table, {ours} with ours: something copies the table \
         every frame"
    );
}
