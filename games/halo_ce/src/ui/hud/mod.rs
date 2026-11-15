//! Halo's in-game HUD (specs/halo_ce/hud.md; M3 of
//! docs/plans/active/halo-ce.md), drawn from the disc's HUD tags by the
//! game's rules on its 640 by 480 design canvas: the shield bar with its
//! hit mark and overshield stages and the health segments top right, the
//! weapon's plate, ammunition numbers (the HUD number tag's glyphs),
//! loaded-rounds bar, heat bar and battery percentage top left with the
//! flashing by the tag's cutoffs, the grenade plate beside it, the
//! crosshair by weapon and zoom state with the warning words, and the
//! motion tracker bottom left (its disc, cone, sweep and the blips of
//! every unit that moves or fires within 20 world units: friends yellow,
//! enemies red by the party's teams, the viewer's own orange).
//!
//! - `layout`: the canvas, the safe frame, anchors, rectangles, the
//!   number's glyphs (and the widescreen rule: scale by height, anchor to
//!   the safe edges).
//! - `rules`: flashes, meters, the shield's bookkeeping, the weapon's
//!   states and crosshair states, the tracker's blips and sweep.
//! - `data`: the HUD read from a map file (`HudData`).
//! - `draw`: the draw list (`Quad`s) from the data, the unit and the state.
//! - `material` (hud.wesl): the pictures' blend, the meter shader, the
//!   sweep.
//! - `nodes`: the Bevy side.
//!
//! **Where it shows.** Under Halo's own root (`client::hud::HudRoot`),
//! scaled by its canvas (node sizes from it), never Bevy's global UI
//! scale, and only while Halo's HUD is the gameplay's (`hud_is`): in
//! Halo's app always, in a party app while the local player holds a Halo
//! gun. It draws instead of the shell's fallback crosshair and status
//! (`DRAWS`); it draws no kill feed (its declared feed, `feed_lines`, is
//! always empty: the party's shared feed is the shell's) and nothing of the
//! shell's overlay. The data is the donor map's (`body::load::DONOR_MAP`),
//! read on a thread the first time the HUD is up; without the disc it
//! draws nothing.
//!
//! Known shortcuts: see `draw`; the motion tracker ignores `hud_radar`
//! (the party apps' radar setting is about map overviews, and the tracker
//! reads none). The grenade HUD is the selected type's (hud.md 10:
//! `grenades::Throw::selected`, changed by the rotate-grenades button),
//! with its count from the local player's `HaloHands`.

pub mod data;
pub mod draw;
pub mod layout;
pub mod material;
pub mod nodes;
pub mod rules;
pub mod score;

use bevy::prelude::*;
use chimera_client::client::hud::{GameDraws, HudDraws};
use chimera_core::gameplay::hud_is;

use crate::game::GAME;

/// What Halo's HUD draws instead of the shell's fallback HUD.
pub const DRAWS: GameDraws = GameDraws { crosshair: true, status: true, hit_marker: false, killfeed: false };

/// Halo's own kill feed's lines: none (kill messages are the shell's).
pub fn feed_lines(_: &World) -> usize {
    0
}

/// The layout the HUD draws in (`layout::HudLayout`): the disc's unless
/// the party's front end (`ui::party_app`) sets the party's.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloHudLayout(pub layout::HudLayout);

/// Halo's HUD: its root, the data's load, the state and the nodes (no
/// renderer needed: the material's assets are used when there).
pub struct HaloHudPlugin;

impl Plugin for HaloHudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().get_resource_or_init::<HudDraws>().0.push((GAME, DRAWS));
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, feed_lines);
        app.init_resource::<nodes::HudLoad>()
            .init_resource::<nodes::HudImages>()
            .init_resource::<nodes::HaloHud>()
            .add_systems(Startup, nodes::spawn_root)
            .add_systems(Update, (nodes::load, nodes::collect, nodes::sync).chain().run_if(hud_is(GAME)));
        super::kill_icons::plugin(app);
    }
}
