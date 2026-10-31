//! A rung's swap with `party_swap_effect` (`chimera_client::client::
//! swap_effect`) on the party's simulation: the local player's gun going
//! from CS:S's to Combat Arms' flashes once per swap; off (the default)
//! nothing.

use chimera_client::client::swap_effect::{SwapEffectPlugin, SwapEffectSetting, SwapEffectView};
use chimera_core::core::LocalPlayer;
use chimera_sim::minigame;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use cs_source::game::weapons as cs_weapons;
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::gun_game::{held, soldier};

const M4: &str = "combat_arms:m4a1";

fn swaps(on: bool) -> u32 {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin, SwapEffectPlugin));
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.app.world_mut().resource_mut::<SwapEffectSetting>().on = u8::from(on);
    let a = soldier(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    assert_eq!(sim.app.world().resource::<SwapEffectView>().swaps, 0, "joining is no swap");
    for gun in [M4, cs_weapons::GLOCK] {
        chimera_sim::weapon::give(sim.app.world_mut(), a, gun);
        sim.seconds(1.0);
        assert_eq!(held(&sim, a), Some(gun));
    }
    sim.app.world().resource::<SwapEffectView>().swaps
}

#[test]
fn a_swap_flashes_once_when_on() {
    assert_eq!(swaps(false), 0, "off: nothing");
    assert_eq!(swaps(true), 2, "on: one flash per swap (CS:S to Combat Arms and back)");
}

/// Each game's swap sound (`swap_effect::swap_cue` on the party's cue
/// packs): a swap into CS:S keeps CS:S's pickup sound, a swap into
/// Combat Arms gets Combat Arms' weapon change sound.
#[test]
fn a_swap_sounds_the_new_games_own_swap_cue() {
    use chimera_client::client::swap_effect::{SWAP_CUE, swap_cue};
    use chimera_core::cues::CuePacks;
    let sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    let packs = sim.app.world().resource::<CuePacks>();
    let sound = |game: &str| swap_cue(packs, None, game).map(|p| (p.id.clone(), p.sound(SWAP_CUE).unwrap().to_string()));
    assert_eq!(sound("cs_source"), Some(("cs_source".into(), "sound/items/battery_pickup.wav".into())));
    assert_eq!(sound("combat_arms"), Some(("combat_arms".into(), "UI_SND/VWRM/WEAPONRACE_CHANGE.WAV".into())));
}
