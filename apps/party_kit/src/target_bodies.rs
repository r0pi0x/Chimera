//! The bodies a sandbox's practice target may wear, for the guest games
//! that register none of their own (`chimera_sim::minigame::targets`,
//! `TargetCharacter`): Halo's Spartan (`halo_ce:spartan`, any Halo gun)
//! and TF2's nine classes (`tf2:scout` ... `tf2:engineer`, the guns each
//! class may hold by TF2's class table, `classes::Classes::of_gun_all`;
//! worn as TF2's rung class, `classes::RungClass`, which the class a held
//! gun makes its holder follows when the gun can be it). A game without
//! an entry here or of its own (Combat Arms, CS:S, HL2:DM) offers its
//! `<game>:default`, the body its guns give.
//!
//! Known shortcuts: Combat Arms' looks (`CaLook`: bodies, heads, camo) are
//! not offered yet; the TF2 class is put on again after each arming
//! (`targets` does), as a rung's own drawn class replaces it there.

use bevy::prelude::*;
use chimera_sim::minigame::targets::{TargetCharacter, TargetsAppExt};
use tf2::game::classes::{Classes, RungClass};

/// TF2's classes as bodies: (id, display name).
const TF2_CLASSES: [(&str, &str); 9] = [
    ("tf2:scout", "Scout"),
    ("tf2:soldier", "Soldier"),
    ("tf2:pyro", "Pyro"),
    ("tf2:demoman", "Demoman"),
    ("tf2:heavy", "Heavy"),
    ("tf2:engineer", "Engineer"),
    ("tf2:medic", "Medic"),
    ("tf2:sniper", "Sniper"),
    ("tf2:spy", "Spy"),
];

fn class_name(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, c)| c)
}

fn tf2_holds(world: &World, character: &str, gun: &str) -> bool {
    let fresh;
    let table = match world.get_resource::<Classes>() {
        Some(t) => t,
        None => {
            fresh = tf2::game::classes::classes();
            &fresh
        }
    };
    table.of_gun_all(gun).iter().any(|c| c.name == class_name(character))
}

fn tf2_wear(world: &mut World, e: Entity, character: &str) {
    let class = class_name(character).to_string();
    if let Ok(mut ent) = world.get_entity_mut(e)
        && ent.get::<RungClass>().is_none_or(|c| c.0 != class)
    {
        ent.insert(RungClass(class));
    }
}

/// The registrations (`PartyPartsPlugin`).
pub fn add(app: &mut App) {
    app.target_character(TargetCharacter {
        id: "halo_ce:spartan",
        name: "Spartan",
        game: "halo_ce",
        holds: |_, _, gun| gun.starts_with("halo_ce:"),
        wear: |_, _, _| {},
    });
    for (id, name) in TF2_CLASSES {
        app.target_character(TargetCharacter { id, name, game: "tf2", holds: tf2_holds, wear: tf2_wear });
    }
}
