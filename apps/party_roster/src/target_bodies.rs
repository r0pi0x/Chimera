//! The bodies a sandbox's practice target may wear, for the guest games
//! that register none of their own (`chimera_sim::minigame::targets`,
//! `TargetCharacter`): Halo's Spartan (`halo_ce:spartan`, any Halo gun;
//! worn, it takes off a campaign body), then Halo's campaign characters
//! (`halo_ce:elite`, `halo_ce:grunt`, `halo_ce:jackal`: Halo's own
//! registrations, `halo_ce::game::campaign::targets`, once their units
//! loaded) and TF2's nine classes (`tf2:scout` ... `tf2:engineer`, the guns each
//! class may hold by TF2's class table, `classes::Classes::of_gun_all`;
//! worn as TF2's rung class, `classes::RungClass`, which the class a held
//! gun makes its holder follows when the gun can be it). Combat Arms, CS:S and HL2:DM
//! offer their `<game>:default`, the body
//! their guns give, registered here for its numbers. Each body's fresh
//! health and second bar (`TargetCharacter::vitals`, normalized: 1.0 a
//! standard player's 100): a Spartan 1.0 and a full shield (1 / its
//! incoming in the party's cross-game table, `cross_game::shield_unit`), a TF2 class its
//! table's health / 100 and none, Combat Arms 1.0 and its default 100
//! points of gear (docs/DESIGN.md, "Armour when the identity switches"),
//! CS:S and HL2:DM 1.0 and none (kevlar is bought; HL2:DM's suit starts
//! empty). The games' full names are their `GameInfo::name`.
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

fn tf2_vitals(world: &World, character: &str) -> (f32, f32) {
    let fresh;
    let table = match world.get_resource::<Classes>() {
        Some(t) => t,
        None => {
            fresh = tf2::game::classes::classes();
            &fresh
        }
    };
    (table.get(class_name(character)).map_or(1.0, |c| c.health / 100.0), 0.0)
}

fn halo_vitals(world: &World, id: &str) -> (f32, f32) {
    let scale = halo_ce::game::cross_game::incoming_in(world, id);
    (1.0, halo_ce::game::cross_game::shield_unit(1.0, scale))
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
        wear: halo_ce::game::campaign::targets::wear_spartan,
        vitals: halo_vitals,
        second_bar: None,
    });
    // Halo's campaign characters, after the Spartan (a Halo gun alone
    // gives the Spartan).
    for c in halo_ce::game::campaign::targets::characters() {
        app.target_character(c);
    }
    for (id, name) in TF2_CLASSES {
        app.target_character(TargetCharacter { id, name, game: "tf2", holds: tf2_holds, wear: tf2_wear, vitals: tf2_vitals, second_bar: None });
    }
    for (game, name, shield) in [("combat_arms", "CA default", 1.0), ("cs_source", "CS:S default", 0.0), ("hl2dm", "HL2:DM default", 0.0)] {
        let id: &'static str = match game {
            "combat_arms" => "combat_arms:default",
            "cs_source" => "cs_source:default",
            _ => "hl2dm:default",
        };
        let vitals: fn(&World, &str) -> (f32, f32) = if shield > 0.0 { |_, _| (1.0, 1.0) } else { |_, _| (1.0, 0.0) };
        app.target_character(TargetCharacter {
            id,
            name,
            game,
            holds: |_, c, gun| gun.split_once(':').is_some_and(|(g, _)| c.starts_with(g) && c[g.len()..].starts_with(':')),
            wear: |_, _, _| {},
            vitals,
            second_bar: None,
        });
    }
}
