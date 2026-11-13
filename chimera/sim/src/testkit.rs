//! The Chimera test kit (feature `testkit`; docs/OBSERVABILITY.md,
//! section 1): what a contract test of the engine, or a game's generic
//! test, builds on instead of a game's data. A weapon set as TOML in round
//! numbers (`kit:pistol`, a semi-automatic with a 0.5 s cycle, an
//! 8-round clip and 20 hit points of 100 a hit; `kit:rifle`, an automatic
//! with a 0.1 s cycle, a 30-round clip and an armour ratio of 0.5; `kit:shotgun`, four pellets a
//! shot, shell by shell), the engine's base
//! (`SimBasePlugins`) on the greybox map (code-built, with spawn points),
//! and small readers: `active`, `active_id`, `feet`, `aim_at`.
//!
//! `kit_sim()` is a `harness::Sim` on the base with the kit's weapons
//! loaded; `with_player` adds a character on the placeholder movement
//! holding one. A game's own tests use the same pieces on their base
//! (`add_weapons`). `STAND_IN` and `add_stand_in_game`: the kit as a
//! game that lends only guns and movement, for the fallbacks of an app
//! made of several games. Nothing here names a game.

use bevy::prelude::*;
use chimera_core::{
    character::CAPSULE_HEIGHT,
    game_parts::{GameParts, GamePartsAppExt, GunClass, GunPart},
    slots::RegisterSlots,
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};

use crate::{
    SimBasePlugins,
    movement::placeholder,
    weapon::{Inventory, Weapon, data::WeaponDataAppExt, give},
};

/// The kit's weapons.
pub const PISTOL: &str = "kit:pistol";
pub const RIFLE: &str = "kit:rifle";
pub const SHOTGUN: &str = "kit:shotgun";
/// The kit's ammo type (the shotgun's is `kit:shell`).
pub const ROUND: &str = "kit:round";

const BASE: &str = r#"
id = "kit:base"
abstract = true
units = { length = 1.0, damage = 0.01 }

[ammo."kit:round"]
power = 10.0
max = 60
box_price = 10
box_rounds = 10

[ammo."kit:shell"]
power = 5.0
max = 24
"#;

const GUN: &str = r#"
id = "kit:gun"
base = "kit:base"
abstract = true

[trigger]
timing = "carry_over"

[hitscan]
pellets = 1
punch_scale = 0.0

[damage]
falloff = 1.0
falloff_step = 100.0
hitgroups = { head = 2.0, chest = 1.0, stomach = 1.0, arm = 1.0, leg = 1.0 }
impulse = 0.0
"#;

const PISTOL_DOC: &str = r#"
id = "kit:pistol"
base = "kit:gun"
slot = 1
draw = 0.5

[buy]
price = 100

[trigger]
automatic = false
cycle = 0.5

[magazine]
clip = 8
ammo = "kit:round"
reload = 1.5

[hitscan]
range = 100.0

[damage]
amount = 20.0
"#;

const RIFLE_DOC: &str = r#"
id = "kit:rifle"
base = "kit:gun"
slot = 0
draw = 1.0

[buy]
price = 1000

[trigger]
automatic = true
cycle = 0.1

[magazine]
clip = 30
ammo = "kit:round"
reload = 2.0

[hitscan]
range = 100.0

[damage]
amount = 10.0
armor_ratio = 0.5

[sounds]
fire = "kit.rifle_fire"
"#;

const SHOTGUN_DOC: &str = r#"
id = "kit:shotgun"
base = "kit:gun"
slot = 0
draw = 1.0

[buy]
price = 800

[trigger]
automatic = false
cycle = 1.0

[magazine]
clip = 6
ammo = "kit:shell"
reload = 0.5
shells = { start = 0.4, insert = 0.5, finish = 0.3 }

[hitscan]
range = 50.0
pellets = 4

[damage]
amount = 8.0
"#;

/// The kit's weapon documents, for `WeaponDataAppExt::add_weapon_data`.
pub fn weapon_docs() -> [(&'static str, &'static str); 5] {
    [
        ("base", BASE),
        ("gun", GUN),
        ("pistol", PISTOL_DOC),
        ("rifle", RIFLE_DOC),
        ("shotgun", SHOTGUN_DOC),
    ]
}

/// Load the kit's weapons into `app` (on any base that has the weapon
/// framework, `SimBasePlugins` among it).
pub fn add_weapons(app: &mut App) {
    app.add_weapon_data(&weapon_docs())
        .expect("the kit's weapon data loads");
}

/// The kit as a stand-in game (`game = "kit"`) that lends only guns and
/// movement (`chimera_core::game_parts`): no knife, characters, HUD or
/// maps, so an app made of several games falls back for each.
pub static STAND_IN: GameParts = GameParts {
    game: "kit",
    guns: &[
        GunPart {
            id: PISTOL,
            class: GunClass::Pistol,
        },
        GunPart {
            id: SHOTGUN,
            class: GunClass::Shotgun,
        },
        GunPart {
            id: RIFLE,
            class: GunClass::Rifle,
        },
    ],
    knife: None,
    movement: Some(STAND_IN_MOVEMENT),
    characters: None,
    hud: None,
    maps: &[],
    announcer: None,
    match_won_sound: None,
    match_lost_sound: None,
    music: false,
    cues: None,
};

/// The stand-in game's movement: the placeholder's, under the kit's name.
pub const STAND_IN_MOVEMENT: &str = "kit:movement";

/// The kit as a stand-in game in `app` (on a base with the weapon
/// framework and the placeholder movement): its weapons, its movement and
/// its parts (`STAND_IN`).
pub fn add_stand_in_game(app: &mut App) {
    add_weapons(app);
    app.register_movement::<placeholder::PlaceholderMovement>(STAND_IN_MOVEMENT)
        .register_parts(&STAND_IN);
}

/// A headless `Sim` on the engine's base and the greybox, the kit's
/// weapons loaded.
pub fn kit_sim() -> Sim {
    let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
    add_weapons(&mut sim.app);
    sim
}

/// A character on the placeholder movement at the greybox's first spawn
/// (or `at`), holding `weapon`; the sim has settled and drawn it.
pub fn with_player(sim: &mut Sim, at: Option<Vec3>, weapon: &str) -> Entity {
    let p = sim.spawn_character(at.unwrap_or(greybox::SPAWNS[0]), placeholder::ID);
    give(sim.app.world_mut(), p, weapon).expect("the kit's weapon exists");
    // Fall, settle and finish drawing (the slowest draw is 1 s).
    sim.seconds(1.6);
    p
}

/// `p`'s weapon in hand.
pub fn active(world: &World, p: Entity) -> Option<Entity> {
    world.get::<Inventory>(p)?.active
}

/// The ID of `p`'s weapon in hand.
pub fn active_id(world: &World, p: Entity) -> Option<&'static str> {
    Some(world.get::<Weapon>(active(world, p)?)?.id)
}

/// Where a standing character's feet are.
pub fn feet(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) - Vec3::Y * (CAPSULE_HEIGHT / 2.0)
}

/// Turn `p`'s intent to look from its eye at `target`.
pub fn aim_at(sim: &mut Sim, p: Entity, target: Vec3) {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let d = target - eye;
    let mut i = sim.intent(p);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kit_loads_and_arms_a_character() {
        let mut sim = kit_sim();
        let p = with_player(&mut sim, None, RIFLE);
        assert_eq!(active_id(sim.app.world(), p), Some(RIFLE));
    }
}
