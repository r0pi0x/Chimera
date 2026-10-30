//! Weapons as composable parts (README, "Weapons and items"): a trigger, a
//! cost, a delivery and an effect. A weapon is an entity carrying those
//! parts and its timers; a character's `Inventory` lists its weapons. Game
//! plugins build weapons from the parts and register them by ID. The frame
//! below runs them the way Source's shared weapon base does
//! (specs/cs_source/weapons.md, sections 3 and 4), which is general enough
//! for other games' weapons: per-game feel goes in extra parts.
//!
//! Weapons can be data: `data` builds the parts from TOML documents a game
//! ships or an app adds, with behaviour data can't express as hooks games
//! register by name (docs/weapon-data.md).
//!
//! Units: meters, seconds, damage normalized like `Health`.
//!
//! Weapons as parts (README, "Weapons and items"): `Trigger` (a per-pull
//! limit; `latency`: updates from entering the firing state to a pull's
//! first round, which goes even if let go; `postfire`: a closing part
//! after a pull's last cycle that reloads and an automatic's next round
//! wait for, a semi-automatic's press cuts short), `Magazine` (cost; `ShellReload`: one round at a time, a shot stops
//! it, optionally back to back with a closing part; `auto_reload`: an empty
//! clip reloads, held or not; `WeaponState::reload_held`: a game holds
//! reloads), `Hitscan` (pellets
//! share a shot's inaccuracy offset; without a `DamageEffect` only its `Fired`
//! event, for a game that delivers rounds itself)/`Melee` (delivery), `Penetration` with
//! `PassMaterials` (bullets through walls and characters), `DamageEffect`,
//! `WeaponSounds`, attack2 modes (`AltModes`, with `Zoom` (owner gets `Zoomed`,
//! max speed; a game zooming its own way marks the weapon `OwnZoom` and sets
//! `Zoomed` itself, with its own mouse scale) and `Burst`); `Inventory`, `WeaponState` timers,
//! `DamageWeapons` (the gun each attacker's damage came from, for kill
//! notices), `WeaponRegistry`/`StartingWeapons` (per team; a character's own `OwnKit`
//! instead), `give`; selection (or a
//! game's own slot-key rules: `OwnSelection`) and deploy before movement, the Source-style weapon frame in `SimSet::Weapons` (spec
//! weapons.md 3–4; both in `core::Predict` schedules, timed by
//! `core::SimClock`; characters a network client draws from snapshots
//! (`map::interp::NetDrawn`) never run it), `WeaponEvent`s (`replay`: written
//! while a client re-ran a command; effects skip those; `Fired`: a round's
//! origin, angles, spread seed and spread, enough to draw it again:
//! `pellet_dirs`); `BulletTarget` (a target's own hitgroup multipliers, hits on
//! more than one of its boxes, exact damage); damage, armour and pushes only
//! where `core::authoritative`, armour only from hits that land (`HitArmor`,
//! the one place bullets, knives and blasts take it, through the shared
//! armour step `hits::absorb`: `Armor`'s coverage, `KEVLAR` and `HELMET`, or
//! a game's own pieces worn with it, `ArmorPieces`: `coverage`),
//! with a `BulletHit` for each bullet hit on something with health (games'
//! reactions: CS:S's slowdown and flinch); `ViewPunch` (pitch, yaw, roll) on
//! the view and, scaled, on the shots.
//!
//! Known shortcuts (docs/tech-debt.md): Penetration (M13): a player's budget
//! cost is a fixed 21.5 units (a fit to the stop cases, Q7), not its thickness;
//! metal and sand damage factors and every other material class (tile, glass,
//! grate, ...: metal's scale 1.0, factor 0.5) are unmeasured guesses; the range
//! halving after players is applied, not after walls (unchecked); the 762MM
//! distance limit is unbounded (passes at 4000, nothing further measured); open
//! surfaces (displacements, anything not a closed solid) stop bullets; the
//! player-just-before-a-wall quirk isn't reproduced. Shots after passing
//! something are `ShotContinued` events: impact sounds, decals and particles
//! mark every surface a bullet enters (the probe logs entry impacts only; exit
//! marks unchecked), but tracers cover only the first segment. Why it's OK for
//! now: Matches every measured damage and pass/stop case the spec lists.

pub mod data;
mod deliver;
pub mod drop;
pub mod economy;
pub mod economy_data;
pub mod equip;
pub mod grenade;
pub mod hits;
pub mod impact;
pub mod lagcomp;
pub mod notice;
pub mod params;
mod pass_data;
pub mod random;
pub mod remote;
pub mod spray;
pub mod sync;
pub mod view_anim;

use std::sync::Arc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
pub use params::ParamsAppExt;
pub use sync::{CarriedAppExt, NetPart};

pub use deliver::{aim_of, armor_split, falloff, hitgroup_at, pellet_dirs, quantize, spread_dir};
pub use hits::{Absorbed, Coverage, DamageClass, Depletion, RegionScale, absorb};

use crate::{
    console::{Command, Console, ConsoleAppExt},
    core::{
        FirstTimePredicted, Health, Hitgroup, Intent, LocalPlayer, MaxSpeed, MovementState, Predict, PredictedAppExt, SimClock,
        SimSet, Team, run_predicted,
    },
    map::{PlaySound, interp::NetDrawn},
};

/// The weapon each attacker held when its latest damage was dealt (a hit
/// whose `Damage::weapon` is None: the held one's), noted before the
/// tick's damage lands (by `map_damage_armor`): a kill can change what
/// the killer holds in the same tick (the gun game's next rung), so a
/// kill notice or the network's `Killed` asks this, not the hand, which
/// gun killed (`of`).
#[derive(Resource, Default, Debug)]
pub struct DamageWeapons(pub bevy::platform::collections::HashMap<Entity, &'static str>);

impl DamageWeapons {
    /// The weapon that dealt `d` (a kill's hit): its own, else what its
    /// attacker held then.
    pub fn of(&self, d: &crate::core::Damage) -> Option<&'static str> {
        d.weapon.or_else(|| self.0.get(&d.attacker?).copied())
    }
}


/// Source's weapon sound channel: a new shot cuts off the last one's tail.
pub const CHAN_WEAPON: u8 = 1;
/// Reload part sounds and other item noises.
pub const CHAN_ITEM: u8 = 3;

pub struct WeaponPlugin;

/// The weapon frame within `SimSet::Weapons`: games order their weapon
/// feel (inaccuracy, recoil) before or after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WeaponFrame;

/// Weapon selection (`Inventory::wanted` applied, deploys): brains that
/// pick weapons run before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectWeapons;

/// On a character whose `lastinv` goes by weapon slot, most recently
/// drawn first (`Inventory::slot_mru`), not by weapon: a swap of the
/// weapon in a slot (the gun game's rungs) keeps the order. Set on both
/// sides for a character that follows its weapon (`identity`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SlotLastInv;

/// A game that turns slot keys and `lastinv` (`Intent::select`,
/// `Intent::last_weapon`) into switches by its own rules, with a system
/// before `SelectWeapons` setting `Inventory::wanted` (Combat Arms' class
/// keys): selection then only applies `wanted`.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct OwnSelection;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponRegistry>()
            .init_resource::<economy::Prices>()
            .init_resource::<economy::BuyWindow>()
            .init_resource::<drop::DeathDrops>()
            .init_resource::<drop::UsePickup>()
            .init_resource::<drop::DebugPickup>()
            .init_resource::<drop::DebugPickupSeen>()
            .add_message::<drop::UsedPickup>()
            .add_message::<drop::PickedUp>()
            .register_type::<economy::Money>()
            .register_type::<economy::DefuseKit>()
            .init_resource::<PassMaterials>()
            .init_resource::<StartingWeapons>()
            .init_resource::<equip::SpawnEquipment>()
            .init_resource::<crate::map::entities::EntityAnchors>()
            .add_message::<crate::core::Equip>()
            .add_message::<WeaponEvent>()
            .init_resource::<notice::NoticeStrings>()
            .init_resource::<notice::CenterNotice>()
            .add_systems(Update, notice::mode_notice)
            .add_message::<BulletHit>()
            .add_message::<crate::map::RagdollShot>()
            .add_message::<PlaySound>()
            // The rules' sounds (`map::GameSound`), played here.
            .add_message::<crate::map::GameSound>()
            .add_message::<crate::map::PredictedSound>()
            .add_systems(FixedLast, crate::map::sound::relay_game_sounds)
            .insert_resource(crate::core::MapDamageArmor)
            .init_resource::<DamageWeapons>()
            .add_systems(
                FixedUpdate,
                map_damage_armor
                    .after(SimSet::Weapons)
                    .before(crate::core::apply_damage)
                    .run_if(crate::core::authoritative),
            )
            .add_message::<crate::map::ViewKick>()
            .add_systems(FixedUpdate, view_kicks.before(SimSet::Weapons))
            .add_systems(
                FixedUpdate,
                (
                    (
                        // What a character carries is the server's to change
                        // (a client hears it with its own player's state).
                        (
                            equip::map_equipment,
                            // Their world work (the removals, the kits, the
                            // map's equips) queued in this order and applied
                            // at the one sync point before `pick_up`, as the
                            // exclusive pollers ran before the equips: no
                            // sync point between them, so the rest of the
                            // tick keeps its order (plan ecs-cleanup step 5).
                            (equip::remove_killed_weapons, give_starting_weapons, equip::apply_equips)
                                .chain_ignore_deferred(),
                            drop::pick_up,
                            drop::use_pick_up,
                            drop::debug_pickups,
                        )
                            .chain()
                            .run_if(crate::core::authoritative),
                        run_predicted(Predict::Select).in_set(SelectWeapons),
                    )
                        .chain()
                        .before(SimSet::Movement),
                    drop::drop_on_death
                        .after(SimSet::Weapons)
                        .run_if(crate::core::authoritative),
                    equip::anchor_weapons
                        .in_set(crate::map::entities::AnchorSet)
                        .after(SimSet::Movement)
                        .before(SimSet::Weapons)
                        .run_if(crate::core::authoritative),
                    (run_predicted(Predict::Weapons).in_set(WeaponFrame), ragdoll_shots)
                        .chain()
                        .in_set(SimSet::Weapons),
                ),
            )
            // What prediction re-runs (`core::Predict`): selection, and the
            // weapon frame with its timed sounds and zoom.
            .add_systems(Predict::Select, select_weapons.in_set(SelectWeapons))
            .add_systems(
                Predict::Weapons,
                (weapon_frame.in_set(WeaponFrame), timed_sounds, apply_zoom).chain(),
            )
            // What a character carries and its weapons' state go to the
            // client predicting it (`sync`).
            .predicted::<Inventory>()
            .predicted_codec(sync::name(), sync::encode, sync::decode)
            .predicted_part::<WeaponState>()
            .predicted_part::<Magazine>()
            .predicted_part::<AltModes>()
            .predicted::<Hitscan>()
            .predicted_net::<ViewPunch>()
            .predicted_net::<Zoomed>();
        impact::plugin(app);
        lagcomp::plugin(app);
        remote::plugin(app);
        grenade::plugin(app);
        spray::plugin(app);
        economy::plugin(app);
        app.init_resource::<Console>();
        app.world_mut().resource_mut::<Console>().add_command(Command {
            name: "give".into(),
            help: "Give the local player a weapon by ID (e.g. give cs_source:weapon_ak47).".into(),
            run: Arc::new(|w, args| {
                server_only(w)?;
                let id = args.first().ok_or("usage: give <weapon id>")?;
                let player = local_player(w)?;
                let def = w
                    .resource::<WeaponRegistry>()
                    .find(id)
                    .ok_or_else(|| format!("unknown weapon {id}"))?;
                let id = def.id;
                give(w, player, id);
                Ok(None)
            }),
            complete: Some(Arc::new(|w, _| {
                w.resource::<WeaponRegistry>()
                    .0
                    .iter()
                    .map(|d| d.id.to_string())
                    .collect()
            })),
        });
        for slot in 0..5u8 {
            app.console_command(
                &format!("slot{}", slot + 1),
                &format!("Select the weapon in slot {} (again: the next one there).", slot + 1),
                move |w, _| {
                    select_local(w, |inv, weapons| {
                        let in_slot: Vec<Entity> = inv
                            .weapons
                            .iter()
                            .copied()
                            .filter(|e| weapons.iter().any(|(we, s)| we == e && *s == slot))
                            .collect();
                        match inv.active.and_then(|a| in_slot.iter().position(|w| *w == a)) {
                            Some(i) => Some(in_slot[(i + 1) % in_slot.len()]),
                            None => in_slot.first().copied(),
                        }
                    })
                },
            );
        }
        app.init_resource::<PushScale>();
        crate::console::resource_cvar::<PushScale, f32>(
            app,
            "phys_pushscale",
            "Multiplies the push of every hit and blast on physics objects (specs/cs_source/physics_props.md; \
             minigame maps raise it for football).",
            |s| &mut s.0,
        );
        crate::console::resource_cvar::<drop::UsePickup, u8>(
            app,
            "chimera_usepickup",
            "1: +use on a dropped weapon you look at takes it, dropping the one in its slot (CS:GO's; CS:S has none).",
            |u| &mut u.0,
        );
        app.init_resource::<drop::AutoWeaponSwitch>();
        crate::console::resource_cvar::<drop::AutoWeaponSwitch, u8>(
            app,
            drop::AUTO_SWITCH_KEY,
            "1: walking over a weapon that weighs more (its script's weight) than the one in hand draws it (sent to the server as userinfo).",
            |u| &mut u.0,
        );
        {
            let mut console = app.world_mut().resource_mut::<Console>();
            console.archive(drop::AUTO_SWITCH_KEY);
            console.userinfo(drop::AUTO_SWITCH_KEY);
        }
        app.init_resource::<economy::BuyLists>();
        crate::console::resource_cvar::<economy::BuyLists, String>(
            app,
            economy::AUTOBUY_KEY,
            "What autobuy buys, in order (CS:S's cfg/autobuy.txt aliases: m4a1 ak47 ... primammo vesthelm; sent to the server as userinfo).",
            |l| &mut l.autobuy,
        );
        crate::console::resource_cvar::<economy::BuyLists, String>(
            app,
            economy::REBUY_KEY,
            "The order rebuy buys in (CS:S's cfg/rebuy.txt categories: PrimaryWeapon PrimaryAmmo Defuser Armor ...; sent to the server as userinfo).",
            |l| &mut l.rebuy,
        );
        {
            let mut console = app.world_mut().resource_mut::<Console>();
            for key in [economy::AUTOBUY_KEY, economy::REBUY_KEY] {
                console.archive(key);
                console.userinfo(key);
            }
        }
        crate::console::resource_cvar::<drop::DebugPickup, u8>(
            app,
            "chimera_debug_pickup",
            "1: print why the loose weapon nearest you is or isn't picked up, each time that changes (2: for every character; pickups are the server's).",
            |u| &mut u.0,
        );
        app.add_message::<drop::DropRequested>()
            .add_message::<economy::BuyRequested>();
        app.console_command("drop", "Drop the weapon you hold (G); walk over one to pick it up.", |w, _| {
            let player = local_player(w)?;
            if client(w) {
                // The server drops it; what we carry follows.
                w.write_message(drop::DropRequested);
                return Ok(None);
            }
            drop::drop_weapon(w, player, true).ok_or("nothing to drop")?;
            Ok(None)
        });
        app.console_command("lastinv", "Switch to the previously held weapon.", |w, _| {
            select_local(w, |inv, _| inv.last)
        })
        .console_command(
            "use",
            "use <weapon>: draw a weapon you carry by name, e.g. use weapon_smokegrenade (Source's).",
            |w, a| {
                let name = a.first().ok_or("use <weapon>")?.to_lowercase();
                let named: Vec<Entity> = w
                    .query::<(Entity, &Weapon)>()
                    .iter(w)
                    .filter(|(_, x)| x.id == name || x.id.rsplit(':').next() == Some(name.as_str()))
                    .map(|(e, _)| e)
                    .collect();
                select_local(w, |inv, _| inv.weapons.iter().copied().find(|e| named.contains(e)))
            },
        )
        .console_command(
            "impulse",
            "impulse 101: every weapon, full clips and reserves, kevlar and helmet (a cheat); impulse 201: spray your logo (T).",
            |w, a| {
                match a.first().map(|s| s.trim()) {
                    Some("201") => {
                        let player = local_player(w)?;
                        return spray::spray_or_ask(w, player);
                    }
                    Some("101") => {}
                    _ => return Err("only impulse 101 (all weapons and ammo) and 201 (spray) exist".into()),
                }
                server_only(w)?;
                let player = local_player(w)?;
                let held: Vec<&'static str> = held_ids(w, player);
                let all: Vec<&'static str> = w.resource::<WeaponRegistry>().0.iter().map(|d| d.id).collect();
                for id in all.into_iter().filter(|id| !held.contains(id)) {
                    give(w, player, id);
                }
                w.entity_mut(player).insert(Armor {
                    amount: 1.0,
                    helmet: true,
                });
                let weapons = w
                    .get::<Inventory>(player)
                    .map(|i| i.weapons.clone())
                    .unwrap_or_default();
                for e in weapons {
                    if let Some(mut m) = w.get_mut::<Magazine>(e) {
                        m.clip = m.size;
                        m.reserve = m.reserve_max;
                    }
                }
                Ok(None)
            },
        )
        .console_command(
            "buy",
            "buy <weapon>|vest|vesthelm|defuser, e.g. buy ak47 (costs money when you have some).",
            |w, a| {
                let name = a.first().ok_or("buy <weapon>")?.clone();
                let player = local_player(w)?;
                economy::buy_or_ask(w, player, &name)
            },
        );
        // CS:S's F1 and F2 (and the buy menu's Autobuy and Rebuy buttons).
        for (name, help) in [
            ("autobuy", "Buy what cl_autobuy lists, in order (one primary, one secondary; in a buy zone, in the buy time)."),
            ("rebuy", "Buy again what you carried after your last buy, in cl_rebuy's order."),
        ] {
            app.console_command(name, help, move |w, _| {
                let player = local_player(w)?;
                economy::buy_or_ask(w, player, name)
            });
        }
        // CS:S's ammo commands (`,` and `.` run buyammo1/2 by default);
        // `buy primammo`/`buy secammo` fill the reserve instead.
        for (name, slot, _) in economy::AMMO_BUYS.into_iter().filter(|(_, _, fill)| !fill) {
            let gun = if slot == 0 { "primary" } else { "secondary" };
            app.console_command(
                name,
                &format!("Buy a box of ammo for your {gun} weapon (in a buy zone, in the buy time)."),
                move |w, _| {
                    let player = local_player(w)?;
                    economy::buy_or_ask(w, player, name)
                },
            );
        }
    }
}

/// Whether this is a network client (what it carries is the server's).
fn client(w: &World) -> bool {
    w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client)
}

/// Refused on a network client: only the server changes what players
/// carry.
fn server_only(w: &World) -> Result<(), String> {
    if client(w) {
        return Err("only the server can do that in a network game".into());
    }
    Ok(())
}

/// The player a command acts on (a network client's on its server).
fn local_player(w: &mut World) -> Result<Entity, String> {
    crate::console::command_player(w)
}

/// IDs of the weapons `owner` carries.
fn held_ids(w: &World, owner: Entity) -> Vec<&'static str> {
    let Some(inv) = w.get::<Inventory>(owner) else {
        return Vec::new();
    };
    inv.weapons
        .iter()
        .filter_map(|e| w.get::<Weapon>(*e).map(|x| x.id))
        .collect()
}

/// Queue a weapon switch for the local player.
fn select_local(
    w: &mut World,
    pick: impl Fn(&Inventory, &[(Entity, u8)]) -> Option<Entity>,
) -> Result<Option<String>, String> {
    let player = w
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(w)
        .next()
        .ok_or("no local player")?;
    let weapons: Vec<(Entity, u8)> = w
        .query::<(Entity, &Weapon)>()
        .iter(w)
        .map(|(e, x)| (e, x.slot))
        .collect();
    let mut inv = w.get_mut::<Inventory>(player).ok_or("no inventory")?;
    if let Some(next) = pick(&inv, &weapons) {
        inv.wanted = Some(next);
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Registry

/// What builds a weapon's parts on a fresh entity.
pub type BuildWeapon = Arc<dyn Fn(&mut EntityWorldMut) + Send + Sync>;

/// One weapon a game offers (from its data, `data`, or code): `build`
/// inserts its parts on a fresh entity.
pub struct WeaponDef {
    /// Namespaced, e.g. `cs_source:weapon_ak47`.
    pub id: &'static str,
    pub build: BuildWeapon,
}

#[derive(Resource, Default)]
pub struct WeaponRegistry(pub Vec<WeaponDef>);

impl WeaponRegistry {
    /// The position of the weapon with this exact ID (the same on every
    /// build with the same games: network messages name weapons by it).
    pub fn index(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|d| d.id == id)
    }

    /// By full ID, or by the part after the namespace (`weapon_ak47`).
    pub fn find(&self, id: &str) -> Option<&WeaponDef> {
        let id = id.to_lowercase();
        self.0
            .iter()
            .find(|d| d.id == id)
            .or_else(|| self.0.iter().find(|d| d.id.rsplit(':').next() == Some(id.as_str())))
    }
}

pub trait RegisterWeapons {
    fn register_weapon(
        &mut self,
        id: &'static str,
        build: impl Fn(&mut EntityWorldMut) + Send + Sync + 'static,
    ) -> &mut Self;
}

impl RegisterWeapons for App {
    fn register_weapon(
        &mut self,
        id: &'static str,
        build: impl Fn(&mut EntityWorldMut) + Send + Sync + 'static,
    ) -> &mut Self {
        self.init_resource::<WeaponRegistry>()
            .world_mut()
            .resource_mut::<WeaponRegistry>()
            .0
            .push(WeaponDef {
                id,
                build: Arc::new(build),
            });
        self
    }
}

/// Weapons every new character gets, the last one given drawn (Source
/// draws the best weapon): its team's list (`team`: the entry for its
/// team, else the one with no team), then `all`.
#[derive(Resource, Default, Clone)]
pub struct StartingWeapons {
    pub team: Vec<(Option<u8>, Vec<&'static str>)>,
    pub all: Vec<&'static str>,
}

impl StartingWeapons {
    /// What a character of `team` starts with, in giving order.
    pub fn for_team(&self, team: Option<u8>) -> Vec<&'static str> {
        let own = self
            .team
            .iter()
            .find(|(t, _)| t.is_some() && *t == team)
            .or_else(|| self.team.iter().find(|(t, _)| t.is_none()));
        own.map(|(_, ids)| ids.clone())
            .unwrap_or_default()
            .into_iter()
            .chain(self.all.iter().copied())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty() && self.team.iter().all(|(_, ids)| ids.is_empty())
    }
}

/// Give `owner` the weapon `id`; draws it if they hold nothing. Returns the
/// weapon entity.
pub fn give(world: &mut World, owner: Entity, id: &str) -> Option<Entity> {
    world.get_entity(owner).ok()?;
    let weapon = spawn_weapon(world, id, owner)?;
    let Ok(mut o) = world.get_entity_mut(owner) else {
        world.despawn(weapon);
        return None;
    };
    if o.get::<Inventory>().is_none() {
        o.insert(Inventory::default());
    }
    let mut inv = o.get_mut::<Inventory>().unwrap();
    inv.weapons.push(weapon);
    inv.wanted = Some(weapon);
    Some(weapon)
}

/// A new weapon entity `id` (built from the registry) belonging to
/// `owner`, not yet in its inventory.
pub fn spawn_weapon(world: &mut World, id: &str, owner: Entity) -> Option<Entity> {
    let build = world.resource::<WeaponRegistry>().find(id)?.build.clone();
    let mut e = world.spawn((Name::new(id.to_string()), WeaponState::default()));
    build(&mut e);
    let weapon = e.id();
    if let Some(mut w) = world.get_mut::<Weapon>(weapon) {
        w.owner = Some(owner);
    }
    // A network client: the server's parameters over what the build derived.
    params::apply_pushed(world, weapon);
    Some(weapon)
}

/// A character's own starting weapons, given in place of
/// `StartingWeapons` (and of a map's spawn equipment) whenever it has no
/// inventory: a bot's kit (Combat Arms' item sets).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct OwnKit(pub Vec<&'static str>);

/// A new character's weapons (one with an `Intent` and `Health` but no
/// `Inventory` yet): its `OwnKit`, else the map's spawn equipment, else
/// `StartingWeapons` for its team. Found by a declared query (an
/// archetype filter: nothing to visit unless such a character exists) in
/// the first tick after it appears; the giving builds weapon entities, so
/// it is world work queued in the query's order and applied at the sync
/// point before `pick_up` (`plugin`'s chain), before the map's equips
/// queued by `apply_equips` and before this tick's selection draws the
/// last one (the tick the exclusive poller gave them,
/// plan ecs-cleanup step 5; an observer at the spawn would give them a
/// tick earlier).
#[allow(clippy::type_complexity)]
fn give_starting_weapons(
    new: Query<(Entity, Option<&Team>, Option<&OwnKit>), (With<Intent>, With<Health>, Without<Inventory>)>,
    mut commands: Commands,
) {
    for (owner, team, kit) in &new {
        let (team, kit) = (team.map(|t| t.0), kit.cloned());
        commands.queue(move |world: &mut World| give_kit(world, owner, team, kit));
    }
}

fn give_kit(world: &mut World, owner: Entity, team: Option<u8>, kit: Option<OwnKit>) {
    let Ok(mut e) = world.get_entity_mut(owner) else { return };
    if e.contains::<Inventory>() {
        return;
    }
    e.insert(Inventory::default());
    if let Some(kit) = kit {
        for id in kit.0 {
            give(world, owner, id);
        }
        return;
    }
    // A map's spawn equipment replaces the starting weapons.
    if let Some(items) = world.resource::<equip::SpawnEquipment>().0.clone() {
        for (name, count) in &items {
            equip::give_item(world, owner, name, *count);
        }
        return;
    }
    for id in world.resource::<StartingWeapons>().for_team(team) {
        give(world, owner, id);
    }
}

// ---------------------------------------------------------------------------
// Components

/// `phys_pushscale`: what every damage push on a physics object is
/// multiplied by (specs/cs_source/physics_props.md 5; default 1).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PushScale(pub f32);

impl Default for PushScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// What a character carries, and the player-level weapon timers.
#[derive(Component, Default, Clone, Debug)]
pub struct Inventory {
    pub weapons: Vec<Entity>,
    pub active: Option<Entity>,
    /// The previously active weapon (Source `lastinv`).
    pub last: Option<Entity>,
    /// The weapon slots drawn, most recent first (every deploy moves its
    /// weapon's slot to the front): `lastinv` by slot for a character that
    /// follows its weapon (`slot_target`), so a swap of the weapon in a
    /// slot (the gun game's rungs) keeps the order.
    pub slot_mru: Vec<u8>,
    /// Switch to this weapon at the start of the next tick.
    pub wanted: Option<Entity>,
    /// No weapon logic before this time (Source's player "next attack").
    pub next_attack: f64,
    /// Last tick's buttons, for press and release edges.
    prev_fire: bool,
    prev_secondary: bool,
    prev_select: Option<u8>,
    prev_last: bool,
    /// Last tick's use key (`drop::use_pick_up`).
    prev_use: bool,
}

impl Inventory {
    /// `slot` drawn just now: first in the order.
    pub fn touch_slot(&mut self, slot: u8) {
        self.slot_mru.retain(|s| *s != slot);
        self.slot_mru.insert(0, slot);
    }

    /// A weapon slot's place taken over by another (a rung's gun in
    /// another class, a knife of the other game): the order stays.
    pub fn rename_slot(&mut self, old: u8, new: u8) {
        if old == new {
            return;
        }
        for s in &mut self.slot_mru {
            if *s == old {
                *s = new;
            }
        }
        let mut seen = Vec::new();
        self.slot_mru.retain(|s| {
            let fresh = !seen.contains(s);
            seen.push(*s);
            fresh
        });
    }

    /// `lastinv` by slot: a weapon of the most recently drawn slot other
    /// than the active weapon's that holds one (the one last in hand when
    /// several share it). `slot_of` gives a weapon's slot.
    pub fn slot_target(&self, slot_of: impl Fn(Entity) -> Option<u8>) -> Option<Entity> {
        let here = self.active.and_then(&slot_of);
        self.slot_mru.iter().filter(|s| Some(**s) != here).find_map(|s| {
            let held = || self.weapons.iter().copied().filter(|w| slot_of(*w) == Some(*s));
            self.last.filter(|l| held().any(|w| w == *l)).or_else(|| held().next())
        })
    }

    /// Attack2 as the weapon frame last saw it (the last tick's, before
    /// this tick's frame): a game's own press edges (Combat Arms' zoom).
    pub fn prev_secondary(&self) -> bool {
        self.prev_secondary
    }
}

/// A weapon's identity and handling.
#[derive(Component, Clone, Debug)]
pub struct Weapon {
    pub id: &'static str,
    /// Slot key (0 = first) used by weapon selection.
    pub slot: u8,
    pub owner: Option<Entity>,
    /// Seconds from drawing to the first allowed attack.
    pub draw_time: f32,
    /// Max movement speed while held, m/s (None: the movement's own).
    pub max_speed: Option<f32>,
}

/// A weapon's auto-switch priority (Source weapon scripts' `weight`): a
/// player with `cl_autowepswitch` on draws a weapon it walks over when it
/// weighs more than the one in hand (`drop::auto_switch`). None counts
/// as 0.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchWeight(pub i32);

/// Trigger: how the primary attack is activated.
#[derive(Component, Clone, Debug)]
pub struct Trigger {
    /// Holding fires again; otherwise every shot needs a fresh press.
    pub automatic: bool,
    /// Seconds between shots.
    pub cycle: f32,
    pub timing: FireTiming,
    /// Most shots per trigger pull, 0 for no limit (Combat Arms'
    /// `RapidFireCount` above 1): holding stops after this many until the
    /// trigger is let go. A semi-automatic (`automatic` false) is 1.
    pub per_press: u32,
    /// Weapon updates from the one that starts firing to the one that
    /// spends the first round (0: that same update, Source). Combat Arms'
    /// view-model chain (specs/combat_arms/weapon_firing.md, Corrections
    /// 10): the update that sees the request (or the end of a draw or
    /// reload with fire held) only enters the firing state, the pre-fire
    /// and the fire animation take one each, so its first round comes 3
    /// updates later (2 without a pre-fire). Once the chain has started its
    /// round goes even if the trigger is let go; while firing (fire held,
    /// or the last cycle not over) later rounds keep the cycle alone.
    pub latency: u32,
    /// Seconds of a closing part after the last cycle of a pull (0: none;
    /// Combat Arms' `postfire` animation, specs/combat_arms/view_models.md
    /// 7 and Corrections 1): it starts on the update that sees the cycle
    /// over with the trigger let go (or the clip empty) and keeps the
    /// firing state. A press during it fires at once on a semi-automatic
    /// (`automatic` false: the press cancels it, no latency); an automatic
    /// waits for its end and fires on the update that sees it over, with
    /// no latency either. Reloads wait for it (`WeaponState::postfire_end`).
    pub postfire: f32,
}

/// How refire times are kept (spec 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireTiming {
    /// One shot, then next = now + cycle (fire rate quantised to ticks).
    Set,
    /// next += cycle per shot; several shots per tick if behind.
    Accumulate,
    /// CS:S (measured, spec M4): a fresh press sets next = now + cycle;
    /// while held, next = previous next + cycle, at most one shot a tick
    /// (the AK-47's 0.1 s fires 7, 7, 7, 6 ticks apart).
    CarryOver,
}

/// Cost: a magazine. Rounds move from the reserve when a reload ends.
#[derive(Component, Clone, Debug)]
pub struct Magazine {
    pub clip: u32,
    pub size: u32,
    pub reserve: u32,
    /// Most rounds the reserve holds (CS:S: `ammo_<type>_max`).
    pub reserve_max: u32,
    /// Seconds from starting a reload to the swap.
    pub reload_time: f32,
    /// Holding attack on an empty clip reloads as soon as allowed (the
    /// SDK); otherwise it dry-fires once and the reload waits for the
    /// buttons to be released (CS:S, measured M10).
    pub reload_while_held: bool,
    /// An empty clip with rounds in reserve reloads as soon as the weapon
    /// may fire again, buttons held or not, with no dry fire (Combat
    /// Arms: specs/combat_arms/weapon_firing.md, "Ammo").
    pub auto_reload: bool,
}

/// Magazine add-on: rounds go in one at a time (shotguns; spec 3.4,
/// "reload one at a time", the CS-derived form). A reload first takes
/// `start` (attacks wait for it), then each round `insert`, the next one
/// starting a tick after the last went in (`back_to_back`: as the last
/// goes in, timed from when it was due), until the clip is full or the
/// reserve empty, then `finish` (0: none; still reloading, a shot ends
/// it). Firing with rounds in the clip stops it; switching away too.
/// Reload sounds (`WeaponSounds::reload`) play from each insert.
#[derive(Component, Clone, Debug)]
pub struct ShellReload {
    pub start: f32,
    pub insert: f32,
    /// The closing part after the last round (Combat Arms' post-reload).
    pub finish: f32,
    /// No tick between rounds (Combat Arms' reload loop).
    pub back_to_back: bool,
}

/// Where a one-at-a-time reload is (`ShellReload`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellStage {
    #[default]
    Idle,
    /// The start sequence (or the tick between two inserts) until
    /// `WeaponState::shell_next`.
    Starting,
    /// A round going in, in at `shell_next`.
    Inserting,
    /// The closing part (`ShellReload::finish`) until `shell_next`.
    Finishing,
}

/// Delivery: instant traces from the eye. Without a `DamageEffect` the
/// frame traces nothing itself: each round is only its `Fired` event, and
/// the game delivers it (Combat Arms' own hit test and damage).
#[derive(Component, Clone, Debug)]
pub struct Hitscan {
    pub range: f32,
    pub pellets: u32,
    pub spread: SpreadShape,
    /// Shots go along the view plus this times the owner's `ViewPunch`.
    pub punch_scale: f32,
}

/// Delivery add-on: bullets that pass through things (Source's bullet
/// penetration). Everything passed, walls and characters alike, counts as
/// one object and uses part of a shared budget: a wall uses its thickness
/// along the path divided by its material's scale (`PassMaterials`), a
/// character a fixed amount. The bullet leaves an object only while the
/// budget stays at or above zero and fewer than `objects` were passed;
/// either way it still hits the next thing. Damage falls off again at
/// every hit (by the distance from the eye) and each object passed scales
/// what's carried on by its material's damage factor.
#[derive(Component, Clone, Debug)]
pub struct Penetration {
    /// Budget, meters of a scale-1 material.
    pub power: f32,
    /// Objects it can pass.
    pub objects: u32,
    /// Objects this far from the eye or farther are not passed, meters.
    pub max_distance: f32,
}

/// How bullets pass each kind of thing (see `Penetration`). Games set it.
#[derive(Resource, Clone, Debug)]
pub struct PassMaterials {
    /// By material class (`MapSurface::game_material`).
    pub by_class: Vec<(char, PassMaterial)>,
    /// Classes not listed, and surfaces whose class isn't known.
    pub default: PassMaterial,
    /// Characters (anything with `Intent` or `Hitboxes`).
    pub character: CharacterPass,
    /// An entity (a map prop entity or a moving body, not the world or a
    /// static prop) passed for this far or farther along the path scales
    /// the damage by its material's factor twice, meters (CS:S, measured
    /// M13: an oil drum's 25-28-unit chords). Infinite: never.
    pub entity_twice_from: f32,
}

impl Default for PassMaterials {
    fn default() -> Self {
        Self {
            by_class: Vec::new(),
            default: PassMaterial {
                scale: 1.0,
                damage: 0.5,
            },
            character: CharacterPass {
                cost: f32::INFINITY,
                damage: 0.5,
                range_scale: 1.0,
            },
            entity_twice_from: f32::INFINITY,
        }
    }
}

impl PassMaterials {
    pub fn get(&self, class: Option<char>) -> PassMaterial {
        class
            .and_then(|c| self.by_class.iter().find(|(k, _)| *k == c))
            .map_or(self.default, |(_, m)| *m)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassMaterial {
    /// Meters of this material a bullet passes per meter of budget.
    pub scale: f32,
    /// Damage carried on after passing, as a factor.
    pub damage: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterPass {
    /// Budget one character uses, meters.
    pub cost: f32,
    /// Damage carried on after passing, as a factor (of the hit's damage
    /// before its hitgroup multiplier).
    pub damage: f32,
    /// After a character, the range left beyond its exit is scaled by this.
    pub range_scale: f32,
}

/// How shots scatter around the aim, as offsets along its right and up
/// vectors (tangent units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpreadShape {
    /// SDK template: each axis U(-.5,.5)+U(-.5,.5), times the scale.
    Template(f32),
    /// CS:S (measured M2): a uniform radius up to `inaccuracy` at a random
    /// angle, plus one up to `spread` at another. Game code updates
    /// `inaccuracy` as it changes.
    Disc { inaccuracy: f32, spread: f32 },
}

/// Recoil and flinch on a character's view (Source's punch angle: pitch
/// up, yaw left, roll as `MovementState::view_roll`; radians): added to
/// the camera, and its pitch and yaw to shots by `Hitscan::punch_scale`.
/// Games that model recoil or flinching add and decay it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewPunch(pub Vec3);

/// A bullet hit on something with health, as the weapon layer dealt it
/// (the server's, once per target per shot: a shotgun's pellets on one
/// target are one, with the last pellet's hitgroup): for games that react
/// to being hit (CS:S's slowdown and flinch). `damage` is what went out
/// (rules may still refuse it); `raw` the damage before armour and
/// truncation (normalized like `Health`); `covered`: armour covered the
/// hitgroup and took its share; `helmet`: the target had armour with a
/// helmet before the hit.
#[derive(Message, Clone, Debug)]
pub struct BulletHit {
    pub damage: crate::core::Damage,
    pub weapon: Entity,
    pub raw: f32,
    pub covered: bool,
    pub helmet: bool,
}

/// Secondary attack: step through the weapon's modes (0 = normal; scope
/// levels, silencer on, burst). Other parts read `current`: `Zoom`,
/// `Burst`, `WeaponSounds::fire_alt`, games' accuracy. Holding the button
/// steps again every `toggle_time`.
#[derive(Component, Clone, Debug)]
pub struct AltModes {
    /// Modes including the normal one.
    pub count: u8,
    pub current: u8,
    /// Seconds until the secondary attack again (and the primary too when
    /// `blocks_primary`, e.g. while a silencer is screwed on).
    pub toggle_time: f32,
    pub blocks_primary: bool,
    /// Played on every step.
    pub sound: Option<String>,
}

impl AltModes {
    pub fn new(count: u8, toggle_time: f32, blocks_primary: bool) -> Self {
        Self {
            count,
            current: 0,
            toggle_time,
            blocks_primary,
            sound: None,
        }
    }
}

/// What the owner reads on stepping to each `AltModes` mode (a centre
/// print, as game string tokens by mode; CS:S's burst weapons: "Switched
/// to burst-fire mode"). Games' HUDs show it.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ModeTexts(pub Vec<&'static str>);

/// Modes 1.. of `AltModes` look through a scope: the owner's view narrows
/// to `fov[mode - 1]` (horizontal degrees at 4:3, like Source's `fov`) and
/// it moves at most `max_speed`. Switching away and reloading unzoom.
#[derive(Component, Clone, Debug)]
pub struct Zoom {
    pub fov: Vec<f32>,
    pub max_speed: Option<f32>,
    /// A shot unzooms; the zoom comes back when the weapon can fire again
    /// (CS:S's AWP and scout).
    pub unzoom_after_shot: bool,
    /// Drawn as a sniper scope (overlay, no view model).
    pub scope: bool,
}

/// While the weapon is in mode `mode`, one trigger pull fires `count`
/// rounds `interval` seconds apart, and the next pull may come `refire`
/// seconds after the first round.
#[derive(Component, Clone, Debug)]
pub struct Burst {
    pub mode: u8,
    pub count: u32,
    pub interval: f32,
    pub refire: f32,
    /// One burst per trigger pull: holding after a burst fires nothing
    /// until the trigger is let go (Combat Arms); otherwise holding fires
    /// a burst every `refire`.
    pub once_per_press: bool,
}

/// On a character looking through a zoomed weapon: its field of view
/// (horizontal degrees at 4:3) and whether that is a sniper scope.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Zoomed {
    pub fov: f32,
    pub scope: bool,
    /// The mouse turn multiplier the game wants (Combat Arms' by zoom
    /// level); none: the client's FOV rule (`zoom_sensitivity_ratio`).
    pub sensitivity: Option<f32>,
}

/// On a weapon: the game keeps its owner's `Zoomed` itself while it is
/// drawn (Combat Arms' scopes, with their transitions); `apply_zoom`
/// leaves it alone then.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OwnZoom;

/// Delivery: melee swings, one per attack button.
#[derive(Component, Clone, Debug)]
pub struct Melee {
    pub primary: Swing,
    pub secondary: Option<Swing>,
}

#[derive(Clone, Debug)]
pub struct Swing {
    pub range: f32,
    /// Half size of the box swept when the line misses (spec 6.1); none: no
    /// fallback.
    pub hull: Option<f32>,
    /// How far the box is swept (the SDK: range minus the box's corner
    /// distance; CS:S's knife: the full range).
    pub hull_reach: f32,
    /// The box only hits targets within this cosine of the view.
    pub facing_cos: f32,
    pub damage: f32,
    /// (window, damage): this damage instead when the previous swing of
    /// this kind was less than `window` seconds ago (CS:S slash: 15 within
    /// 0.9 s of the last slash).
    pub follow_up: Option<(f32, f32)>,
    /// Damage when hitting from behind, if different: the target faces
    /// within acos(`BACKSTAB_COS`) of the attacker-to-target bearing.
    pub backstab: Option<f32>,
    /// Hits scale by hitgroup (the SDK); CS:S's knife reports generic.
    pub hitgroups: bool,
    /// Seconds until either attack after a hit.
    pub hit_refire: f32,
    /// After a miss: until this swing again, and until the other one.
    pub miss_refire: f32,
    pub miss_refire_other: f32,
    /// Impulse per hit, kg·m/s (CS:S's knife: 300 kg·in/s for a slash or
    /// a stab alike, measured).
    pub force: f32,
    /// The damage type its hits deal (CS:S's knife: bullet, measured).
    pub kind: crate::core::DamageKind,
    /// As `DamageEffect::armor_ratio`; damage truncation as its `quantum`.
    pub armor_ratio: Option<f32>,
    pub quantum: f32,
    pub sound_hit: Option<String>,
    pub sound_hit_world: Option<String>,
    pub sound_miss: Option<String>,
}

/// Effect: damage on hit, for hitscan deliveries.
#[derive(Component, Clone, Debug)]
pub struct DamageEffect {
    pub amount: f32,
    /// Damage × falloff^(distance / falloff_step).
    pub falloff: f32,
    pub falloff_step: f32,
    /// The gun's damage per body region (its game's table; from the
    /// data's hitgroups, `RegionScale::from_hitgroups`).
    pub regions: hits::RegionScale,
    /// Impulse given to physics objects per shot, kg·m/s.
    pub impulse: f32,
    /// Damage dealt is truncated to whole multiples of this (CS:S: one hit
    /// point, 0.01); 0 keeps fractions.
    pub quantum: f32,
    /// How much of a hit armour lets through to health (`Armor`); None:
    /// armour doesn't apply.
    pub armor_ratio: Option<f32>,
}

/// Body armour, one per character whatever its game, normalized like
/// `Health` (1.0 = 100): the default kit (CS:S's kevlar and helmet) or a
/// game's own pieces worn with it (`ArmorPieces`: Combat Arms' gear). The
/// default kit's coverage per body part (`entries`: `KEVLAR`, and
/// `HELMET` with a helmet) goes through the shared armour step
/// (`hits::absorb`): hits it covers (CS:S, measured M7: chest, stomach,
/// arms, generic, and the head only with a helmet) take `damage x ratio x
/// 0.5` from health and half the rest from armour, both truncated.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Armor {
    pub amount: f32,
    pub helmet: bool,
}

/// CS:S's kevlar as coverage: half of every class on the torso (generic,
/// chest, stomach, the groin too), the arms; armour points lose half of
/// what it takes.
pub const KEVLAR: hits::Coverage = hits::Coverage {
    class: None,
    head: 0.0,
    torso: 0.5,
    arm: 0.5,
    leg: 0.0,
    groin: 0.5,
    wear: 0.5,
};

/// CS:S's helmet: the head, as kevlar covers the body.
pub const HELMET: hits::Coverage = hits::Coverage {
    class: None,
    head: 0.5,
    torso: 0.0,
    arm: 0.0,
    leg: 0.0,
    groin: 0.0,
    wear: 0.5,
};

impl Armor {
    /// The coverage worn.
    pub fn entries(&self) -> &'static [hits::Coverage] {
        if self.helmet { &[KEVLAR, HELMET] } else { &[KEVLAR] }
    }

    /// How it runs out (CS:S: by what it would lose).
    pub fn depletion(&self) -> hits::Depletion {
        hits::Depletion::Cost
    }

    /// Whether it covers a hit on `group` (it has points and coverage
    /// there).
    pub fn covers(&self, group: Hitgroup) -> bool {
        self.covers_region(crate::core::BodyRegion::from_hitgroup(group))
    }

    /// Whether it covers a hit on `region`.
    pub fn covers_region(&self, region: crate::core::BodyRegion) -> bool {
        self.amount > 0.0 && self.entries().iter().any(|e| e.at(region.part()) > 0.0)
    }
}

/// A game's own armour pieces worn with `Armor` (Combat Arms' gear):
/// their coverage and how they run out, in place of the default kit
/// (`KEVLAR`, and `HELMET` with a helmet: CS:S's). The game whose armour
/// it is puts it on its own characters and takes it off when one becomes
/// another game's (`slots::GameIdentity`); `Armor`'s amount stays. So a
/// character has one armour value, and what covers it follows its
/// identity (docs/DESIGN.md "Hits across games").
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArmorPieces {
    pub pieces: Vec<hits::ArmorPiece>,
    pub depletion: hits::Depletion,
}

/// What `armor` covers (a game's `pieces` when worn, else its default
/// kit) and how it runs out.
pub fn coverage<'a>(armor: &Armor, pieces: Option<&'a ArmorPieces>) -> (impl Iterator<Item = &'a hits::Coverage>, hits::Depletion) {
    let (kit, own): (&'static [hits::Coverage], &'a [hits::ArmorPiece]) = match pieces {
        Some(p) => (&[], &p.pieces),
        None => (armor.entries(), &[]),
    };
    let depletion = pieces.map_or(armor.depletion(), |p| p.depletion);
    (kit.iter().chain(own.iter().flat_map(|p| p.entries.iter())), depletion)
}

/// What armour did to one hit (`HitArmor::apply`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmorHit {
    /// Armour covered the hitgroup (it had some, and a helmet for the
    /// head) and the weapon's ratio applies: health got its share only.
    /// True for a refused hit as well, which takes no armour (CS:S: a
    /// noclipping player's covered hit doesn't flinch, as a walking one's).
    pub covered: bool,
    /// The target wore armour with a helmet before the hit.
    pub helmet: bool,
}

/// The one place hits take armour: bullets, knives (`deliver`) and blasts
/// (`grenade`). A hit takes armour only when it lands (`core::DamageRules::lands`):
/// a teammate's hit with friendly fire off, a noclipping or `God` target,
/// a map's invulnerability or damage filter leave it whole (CS:S, measured
/// 2026-10-17 for teammates and noclip: specs/cs_source/weapons.md
/// "Refused hits").
#[derive(bevy::ecs::system::SystemParam)]
pub struct HitArmor<'w, 's> {
    armor: Query<'w, 's, (&'static mut Armor, Option<&'static ArmorPieces>)>,
    rules: crate::core::DamageRules<'w, 's>,
}

impl HitArmor<'_, '_> {
    /// Whether `damage` lands (`core::DamageRules::lands`).
    pub fn lands(&self, damage: &crate::core::Damage) -> bool {
        self.rules.lands(damage)
    }

    /// Split `raw` damage (before armour, normalized like `Health`) of
    /// `damage` between armour and health (the shared armour step,
    /// `hits::absorb`, with the weapon's `ratio`, truncated to `quantum`),
    /// on the region of its hitgroup (`BodyRegion::from_hitgroup`): sets
    /// `damage.amount` to health's share when armour covers it, and takes
    /// armour's from the target if the hit lands.
    pub fn apply(&mut self, damage: &mut crate::core::Damage, raw: f32, ratio: Option<f32>, quantum: f32) -> ArmorHit {
        let region = crate::core::BodyRegion::from_hitgroup(damage.hitgroup);
        self.apply_region(damage, region, raw, ratio, quantum)
    }

    /// `apply` on a body region (the damage's class by its kind,
    /// `hits::DamageClass::of`).
    pub fn apply_region(
        &mut self,
        damage: &mut crate::core::Damage,
        region: crate::core::BodyRegion,
        raw: f32,
        ratio: Option<f32>,
        quantum: f32,
    ) -> ArmorHit {
        let lands = self.rules.lands(damage);
        let Ok((mut armor, pieces)) = self.armor.get_mut(damage.target) else {
            return ArmorHit::default();
        };
        let (entries, depletion) = coverage(&armor, pieces);
        let mut hit = ArmorHit {
            covered: false,
            helmet: armor.amount > 0.0 && armor.helmet,
        };
        let a = hits::absorb(
            raw,
            region.part(),
            hits::DamageClass::of(damage.kind),
            ratio,
            quantum,
            armor.amount,
            entries,
            depletion,
        );
        if a.covered {
            damage.amount = a.to_health;
            hit.covered = true;
            if lands {
                armor.amount = (armor.amount - a.taken).max(0.0);
            }
        }
        hit
    }
}

/// A map's damage to a player (`core::MapDamage`): armour takes its part as
/// from a weapon of armour ratio 1 (health half, armour half the rest,
/// measured on trigger_hurt: 20 a pass on 100 armour takes 10 health and
/// 5 armour, 2.5 takes 1 and 1, 20 on 1 armour takes 18 and 1), when the
/// kind allows it; health takes whole points.
///
/// It also notes, from the tick's weapon damage so far, the gun each
/// attacker held (`DamageWeapons`): it runs where that damage is all in
/// and hasn't landed yet (a system of its own there reordered the
/// schedule's unordered pairs between server and client).
pub fn map_damage_armor(
    mut map: MessageReader<crate::core::MapDamage>,
    mut armor: HitArmor,
    mut out: ResMut<Messages<crate::core::Damage>>,
    mut cursor: Local<bevy::ecs::message::MessageCursor<crate::core::Damage>>,
    (inventories, weapons, mut noted): (Query<&Inventory>, Query<&Weapon>, ResMut<DamageWeapons>),
) {
    for d in cursor.read(&out) {
        let Some(a) = d.attacker.filter(|_| d.weapon.is_none()) else { continue };
        if let Some(w) = inventories.get(a).ok().and_then(|i| i.active).and_then(|w| weapons.get(w).ok()) {
            noted.0.insert(a, w.id);
        }
    }
    for m in map.read() {
        let mut d = m.damage.clone();
        let raw = d.amount;
        if m.armour {
            armor.apply(&mut d, raw, Some(1.0), 0.01);
        }
        d.amount = crate::core::whole_points(d.amount);
        if d.amount > 0.0 {
            out.write(d);
        }
    }
    // Ours aren't weapons'.
    cursor.clear(&out);
}

/// Damage multiplier per hitgroup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitgroupScale {
    pub head: f32,
    pub chest: f32,
    pub stomach: f32,
    pub arm: f32,
    pub leg: f32,
}

impl Default for HitgroupScale {
    /// The SDK's generic multipliers (sk_player_*).
    fn default() -> Self {
        Self {
            head: 2.0,
            chest: 1.0,
            stomach: 1.0,
            arm: 1.0,
            leg: 1.0,
        }
    }
}

impl HitgroupScale {
    pub fn get(&self, g: Hitgroup) -> f32 {
        match g {
            Hitgroup::Generic => 1.0,
            Hitgroup::Head => self.head,
            Hitgroup::Chest => self.chest,
            Hitgroup::Stomach => self.stomach,
            Hitgroup::LeftArm | Hitgroup::RightArm => self.arm,
            Hitgroup::LeftLeg | Hitgroup::RightLeg => self.leg,
        }
    }
}

/// How bullets treat a target other than by the weapon's numbers (CS:S's
/// hostages, specs/cs_source/objectives.md Q8). Games put it on the
/// target; without it the weapon's rules apply.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BulletTarget {
    /// Its own hitgroup multipliers, instead of the weapon's
    /// (`DamageEffect::hitgroups`).
    pub hitgroups: Option<HitgroupScale>,
    /// A bullet that passes one of its hitboxes goes on inside it from
    /// that box's exit and may hit another of its boxes (or another box of
    /// the same group): each box passed is charged and scales the damage
    /// carried on as a passed character does (`PassMaterials::character`),
    /// and falloff applies again at every hit. Without it a passed
    /// character is left whole.
    pub hits_inside: bool,
    /// Its damage is kept exact (health a float), not truncated to the
    /// weapon's `DamageEffect::quantum`.
    pub exact: bool,
}

/// Sound entries a weapon plays.
#[derive(Component, Clone, Debug, Default)]
pub struct WeaponSounds {
    pub fire: Option<String>,
    /// Instead of `fire` in any alternate mode (`AltModes`), e.g. silenced.
    pub fire_alt: Option<String>,
    pub empty: Option<String>,
    pub deploy: Option<String>,
    /// Entries at times after a reload starts (view-model animation events).
    pub reload: Vec<(f32, String)>,
    /// Entries at times after the weapon is drawn.
    pub draw: Vec<(f32, String)>,
    /// Entries at times after each shot (a bolt worked by the fire
    /// animation).
    pub shot: Vec<(f32, String)>,
    /// Entries at times after a one-at-a-time reload (`ShellReload`) ends.
    pub finish: Vec<(f32, String)>,
    /// (mode, time, entry): played that long after stepping to that
    /// `AltModes` mode with attack2 (e.g. the silencer going on or off).
    pub modes: Vec<(u8, f32, String)>,
}

/// Source's per-weapon timers and flags.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeaponState {
    pub next_primary: f64,
    pub next_secondary: f64,
    /// When the reload in progress swaps the magazine.
    pub reload_end: Option<f64>,
    /// Seconds attack has been held.
    pub fire_duration: f32,
    pub fired_on_empty: bool,
    pub next_empty_sound: f64,
    /// Shots since the trigger was last released.
    pub burst: u32,
    /// When each swing (primary, secondary) was last made.
    pub last_swing: [Option<f64>; 2],
    /// Sounds waiting for their time (reload parts).
    pending: Vec<(f64, String)>,
    /// Rounds of the current burst still to fire, and when the next goes.
    pub burst_left: u32,
    pub next_burst_round: f64,
    /// The zoom mode to return to once the weapon can fire again.
    pub rezoom: Option<u8>,
    /// A one-at-a-time reload (`ShellReload`) and when its stage ends.
    pub shells: ShellStage,
    pub shell_next: f64,
    /// The game holds reloads (the key's and the automatic ones) while
    /// set: Combat Arms zooms out first, then reloads.
    pub reload_held: bool,
    /// In the firing state (`Trigger::latency`): entered by the update
    /// that starts the chain, left when the trigger is let go with no
    /// round pending and the last cycle over, and by a draw or a reload.
    pub firing: bool,
    /// Updates of the chain still to go before its first round, and
    /// whether that round is still to come.
    pub arming: u32,
    pub armed_round: bool,
    /// The trigger's closing part (`Trigger::postfire`) plays until then.
    pub postfire_end: Option<f64>,
}

impl WeaponState {
    /// A reload of either kind is in progress.
    pub fn reloading(&self) -> bool {
        self.reload_end.is_some() || self.shells != ShellStage::Idle
    }
}

/// What weapons did this tick, for HUDs, effects and tests.
#[derive(Message, Clone, Debug)]
pub struct WeaponEvent {
    pub owner: Entity,
    pub weapon: Entity,
    pub kind: WeaponEventKind,
    /// Written while a network client re-ran a command after a correction
    /// (`core::FirstTimePredicted` false): what the weapon does again
    /// (recoil reads it), not something to show again. Effects, sounds,
    /// HUDs and animation skip these.
    pub replay: bool,
}

impl WeaponEvent {
    /// Something to show (not a replay; `replay`).
    pub fn shown(&self) -> bool {
        !self.replay
    }
}

#[derive(Clone, Debug)]
pub enum WeaponEventKind {
    Deployed,
    /// One round of a hitscan weapon left the barrel (before its traces,
    /// one `Shot` per pellet): from `origin` along `aim_of(yaw, pitch)`
    /// with spread seed `seed` and this spread, in `AltModes` mode `mode`.
    /// Enough to draw the same shot again elsewhere (`pellet_dirs`): a
    /// network server sends it to the other clients. `id`: the weapon's ID,
    /// kept here because the gun game swaps the gun on the kill, and its
    /// entity is gone when the event is read.
    Fired {
        id: &'static str,
        origin: Vec3,
        yaw: f32,
        pitch: f32,
        seed: u32,
        spread: SpreadShape,
        mode: u8,
    },
    /// One trace: from the eye to where it stopped.
    Shot {
        from: Vec3,
        to: Vec3,
        hit: Option<Entity>,
        /// Surface normal where it hit.
        normal: Option<Vec3>,
    },
    /// The same bullet going on after passing something (`Penetration`):
    /// from where it left that to where it stopped next.
    ShotContinued {
        from: Vec3,
        to: Vec3,
        hit: Option<Entity>,
        normal: Option<Vec3>,
    },
    /// Damage dealt to something with health (summed per firing call).
    Hit {
        target: Entity,
        amount: f32,
        hitgroup: Hitgroup,
    },
    /// A melee swing (hit or not).
    Swing {
        hit: bool,
        secondary: bool,
        /// Where it hit: point, surface normal and what was hit.
        at: Option<(Vec3, Vec3, Entity)>,
        /// The swing's line trace: from the eye to where it stopped (the
        /// range's end when it hit nothing).
        line: (Vec3, Vec3),
    },
    DryFire,
    /// The weapon stepped to this `AltModes` mode (attack2, or a sniper
    /// shot unzooming and re-zooming).
    ModeChanged {
        mode: u8,
    },
    ReloadStarted,
    /// One round started going in (`ShellReload`).
    ShellInserting,
    /// A reload finished (the magazine swapped, or the last round in).
    Reloaded,
    /// A grenade's pin came out (`grenade::Throwable`).
    PinPulled,
    /// A grenade was released (it leaves the hand a moment later).
    Thrown,
    /// Arming a bomb began (the press-button animation, the third-person
    /// gesture): `objectives::bomb`.
    ArmingStarted,
    /// Arming stopped short (let go, moved off): back to idle.
    ArmingStopped,
}

// ---------------------------------------------------------------------------
// Selection and deploy

pub(crate) fn select_weapons(
    mut owners: Query<(Entity, &Intent, &mut Inventory, &Transform, Has<SlotLastInv>), Without<NetDrawn>>,
    mut weapons: Query<(
        &Weapon,
        &mut WeaponState,
        Option<&WeaponSounds>,
        Option<&mut AltModes>,
        Option<&Zoom>,
    )>,
    mut commands: Commands,
    mut events: MessageWriter<WeaponEvent>,
    mut play: MessageWriter<PlaySound>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
    own: Option<Res<OwnSelection>>,
) {
    let now = clock.now;
    let keys = own.is_none();
    for (owner, intent, mut inv, at, by_slot) in &mut owners {
        inv.weapons.retain(|w| weapons.contains(*w));
        // Slot keys on press: the first weapon in that slot, or the next one
        // after the active weapon when several share it; a pick (the HUD's
        // selection) names its weapon's place in the slot.
        if keys
            && intent.select != inv.prev_select
            && let Some(select) = intent.select
        {
            let (slot, nth) = Intent::picked(select);
            let in_slot: Vec<Entity> = inv
                .weapons
                .iter()
                .copied()
                .filter(|w| weapons.get(*w).is_ok_and(|(w, ..)| w.slot == slot))
                .collect();
            let next = match nth.and_then(|n| in_slot.get(n as usize)) {
                Some(w) => *w,
                None => match inv.active.and_then(|a| in_slot.iter().position(|w| *w == a)) {
                    Some(i) => in_slot[(i + 1) % in_slot.len()],
                    None => match in_slot.first() {
                        Some(w) => *w,
                        None => {
                            inv.prev_select = intent.select;
                            continue;
                        }
                    },
                },
            };
            inv.wanted = Some(next);
        }
        inv.prev_select = intent.select;
        if keys && intent.last_weapon && !inv.prev_last {
            inv.wanted = if by_slot {
                inv.slot_target(|w| weapons.get(w).ok().map(|(w, ..)| w.slot))
            } else {
                inv.last.filter(|l| inv.weapons.contains(l))
            };
        }
        inv.prev_last = intent.last_weapon;

        let Some(want) = inv.wanted.take() else { continue };
        // Selecting what you hold changes nothing (spec 3.6).
        if inv.active == Some(want) {
            continue;
        }
        // Holster: cancels a reload, a burst and the zoom; CS:S models have
        // no holster animation.
        if let Some(old) = inv.active
            && let Ok((_, mut st, _, modes, zoom)) = weapons.get_mut(old)
        {
            st.reload_end = None;
            st.pending.clear();
            st.fire_duration = 0.0;
            st.burst_left = 0;
            st.rezoom = None;
            st.shells = ShellStage::Idle;
            if let (Some(mut modes), Some(_)) = (modes, zoom) {
                modes.current = 0;
            }
        }
        let Ok((w, mut st, sounds, ..)) = weapons.get_mut(want) else {
            continue;
        };
        // Deploy (spec 3.3).
        let ready = now + w.draw_time as f64;
        inv.next_attack = ready;
        st.next_primary = ready;
        st.next_secondary = ready;
        st.fired_on_empty = false;
        inv.last = inv.active;
        inv.active = Some(want);
        inv.touch_slot(w.slot);
        match w.max_speed {
            Some(s) => commands.entity(owner).insert(MaxSpeed(s)),
            None => commands.entity(owner).remove::<MaxSpeed>(),
        };
        if let Some(s) = sounds.and_then(|s| s.deploy.clone())
            && first.0
        {
            play.write(owner_sound(s, owner, at.translation, CHAN_ITEM));
        }
        if let Some(s) = sounds {
            st.pending = s.draw.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
        }
        events.write(WeaponEvent {
            owner,
            weapon: want,
            kind: WeaponEventKind::Deployed,
            replay: !first.0,
        });
    }
}

/// A sound from the weapon's owner at `at`.
fn owner_sound(entry: String, owner: Entity, at: Vec3, channel: u8) -> PlaySound {
    PlaySound {
        pitch: None,
        entry,
        at: Some(at),
        volume: None,
        source: Some(owner),
        channel: Some(channel),
    }
}

// ---------------------------------------------------------------------------
// The weapon frame (spec 3.1)

#[derive(bevy::ecs::query::QueryData)]
#[query_data(mutable)]
struct WeaponParts {
    entity: Entity,
    weapon: &'static Weapon,
    state: &'static mut WeaponState,
    trigger: Option<&'static Trigger>,
    magazine: Option<&'static mut Magazine>,
    hitscan: Option<&'static Hitscan>,
    melee: Option<&'static Melee>,
    effect: Option<&'static DamageEffect>,
    penetration: Option<&'static Penetration>,
    sounds: Option<&'static WeaponSounds>,
    modes: Option<&'static mut AltModes>,
    zoom: Option<&'static Zoom>,
    burst: Option<&'static Burst>,
    shells: Option<&'static ShellReload>,
}

impl WeaponPartsItem<'_, '_> {
    fn mode(&self) -> u8 {
        self.modes.as_ref().map_or(0, |m| m.current)
    }

    /// Change mode, telling the HUD and view model.
    fn set_mode(&mut self, mode: u8, owner: Entity, w: &mut deliver::World) {
        let Some(m) = self.modes.as_mut() else { return };
        if m.current == mode {
            return;
        }
        m.current = mode;
        w.event(owner, self.entity, WeaponEventKind::ModeChanged { mode });
    }

    /// Fire up to `rounds` from the clip: traces, sounds; returns how many.
    fn fire_rounds(&mut self, ctx: &mut deliver::Shot, rounds: u32) -> u32 {
        let mut shots = rounds;
        if let Some(mag) = self.magazine.as_mut() {
            shots = shots.min(mag.clip);
            mag.clip -= shots;
        }
        let alt = self.mode() > 0;
        for i in 0..shots {
            if let Some(scan) = self.hitscan {
                ctx.seed = ctx.seed.wrapping_add(i);
                let fired = WeaponEventKind::Fired {
                    id: self.weapon.id,
                    origin: ctx.eye,
                    yaw: ctx.yaw,
                    pitch: ctx.pitch,
                    seed: ctx.seed,
                    spread: scan.spread,
                    mode: self.mode(),
                };
                ctx.w.event(ctx.owner, self.entity, fired);
                // Without an effect the game delivers it (`Hitscan`).
                if let Some(effect) = self.effect {
                    ctx.fire(scan, effect, self.penetration);
                }
            }
            let sound = self.sounds.and_then(|s| {
                if alt && s.fire_alt.is_some() {
                    s.fire_alt.clone()
                } else {
                    s.fire.clone()
                }
            });
            if let Some(s) = sound {
                ctx.w.sound(owner_sound(s, ctx.owner, ctx.eye, CHAN_WEAPON));
            }
        }
        self.state.burst += shots;
        if shots > 0
            && let Some(s) = self.sounds.filter(|s| !s.shot.is_empty())
        {
            let now = ctx.now;
            self.state.pending = s.shot.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
        }
        shots
    }
}

/// Seconds between dry-fire clicks (spec: empty_sound_interval).
const EMPTY_SOUND_INTERVAL: f64 = 0.5;

/// Timers that come due exactly on a tick fire on that tick: CS:S does
/// (measured M4/M8: the M4A1's 0.975 s draw takes 65 ticks, its 0.09 s
/// cycle 6, the pistols' 0.15 s 10), while our f32 durations widened to
/// f64 land a hair past the tick. Times are compared with this slack.
const TIME_SLACK: f64 = 1e-5;

#[allow(clippy::too_many_arguments)]
fn weapon_frame(
    mut owners: Query<(
        Entity,
        &Intent,
        &mut Inventory,
        &Transform,
        &MovementState,
        Option<&Health>,
        Option<&ViewPunch>,
        Option<&lagcomp::ViewTick>,
    ), Without<NetDrawn>>,
    mut weapons: Query<WeaponParts>,
    mut world: deliver::World,
    clock: Res<SimClock>,
    (histories, lag, mut lag_stats): (
        Query<(Entity, &lagcomp::HitHistory, &Health)>,
        Res<lagcomp::LagCompSettings>,
        ResMut<lagcomp::LagCompStats>,
    ),
) {
    // The command's time (Source's curtime).
    let now = clock.now;
    // Compare timers against this; record new ones from `now`.
    let due = now + TIME_SLACK;
    let dt = clock.dt();
    for (owner, intent, mut inv, transform, state, health, punch, view) in &mut owners {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let pressed = intent.fire && !inv.prev_fire;
        let released2 = !intent.secondary && inv.prev_secondary;
        inv.prev_fire = intent.fire;
        inv.prev_secondary = intent.secondary;
        let Some(active) = inv.active else { continue };
        let Ok(mut w) = weapons.get_mut(active) else { continue };
        if !intent.fire {
            w.state.burst = 0;
        }
        // A sniper shot's zoom comes back when the weapon can fire again
        // (measured M15).
        if let Some(mode) = w.state.rezoom
            && due >= w.state.next_primary
        {
            w.state.rezoom = None;
            w.set_mode(mode, owner, &mut world);
        }
        // The firing state (`Trigger::latency`) ends with a draw or a
        // reload: the update that sees either end starts the chain again.
        let latency = w.trigger.map_or(0, |t| t.latency);
        let postfire = w.trigger.map_or(0.0, |t| t.postfire);
        if (latency > 0 || w.state.postfire_end.is_some())
            && (due < inv.next_attack || w.state.reloading())
            && (w.state.firing || w.state.armed_round || w.state.postfire_end.is_some())
        {
            w.state.firing = false;
            w.state.arming = 0;
            w.state.armed_round = false;
            w.state.postfire_end = None;
        }
        // The closing part plays (`Trigger::postfire`): reloads wait, and
        // an automatic's next round.
        let in_postfire = |st: &WeaponState| st.postfire_end.is_some_and(|end| end > due);
        // Busy (drawing, reloading): only the timers run.
        if due < inv.next_attack {
            continue;
        }
        // The chain's updates before its first round.
        if w.state.arming > 0 {
            w.state.arming -= 1;
        }
        let eye = transform.translation + state.eye_offset;
        // Shots follow the view plus the weapon's share of the recoil.
        let kick = punch.map_or(Vec2::ZERO, |p| p.0.truncate()) * w.hitscan.map_or(0.0, |h| h.punch_scale);
        let (yaw, pitch) = (intent.yaw + kick.y, intent.pitch + kick.x);
        // A remote player's shots and swings hit others where it saw them
        // (lag compensation, server only: `ViewTick` is set per command).
        let may_trace = (intent.fire && w.state.next_primary <= due)
            || (intent.secondary && w.melee.is_some() && w.state.next_secondary <= due)
            || (w.state.burst_left > 0 && due >= w.state.next_burst_round);
        let rewound = match view.and_then(|v| v.0) {
            Some(view_tick) if lag.unlag != 0 && may_trace && world.authoritative() => {
                lagcomp::rewind(
                    owner,
                    view_tick,
                    clock.tick,
                    clock.delta.as_secs_f64(),
                    &lag,
                    histories
                        .iter()
                        .filter(|(_, _, h)| h.current > 0.0)
                        .map(|(e, history, _)| (e, history)),
                    &mut lag_stats,
                )
            }
            _ => Vec::new(),
        };
        let mut ctx = deliver::Shot {
            owner,
            weapon: active,
            eye,
            aim: deliver::aim_of(yaw, pitch),
            yaw,
            pitch,
            // Spec 4.2: from the command number.
            seed: random::shot_seed(intent.command),
            now,
            rewound,
            w: &mut world,
        };

        // 1. Fire duration.
        w.state.fire_duration = if intent.fire { w.state.fire_duration + dt } else { 0.0 };

        // 2. Reload completion.
        if let Some(end) = w.state.reload_end
            && due >= end
        {
            if let Some(mag) = w.magazine.as_mut() {
                let n = (mag.size - mag.clip).min(mag.reserve);
                mag.clip += n;
                mag.reserve -= n;
            }
            w.state.reload_end = None;
            w.state.next_primary = now;
            w.state.next_secondary = now;
            ctx.w.event(owner, active, WeaponEventKind::Reloaded);
        }

        // One-at-a-time reloads step on (spec 3.4; T25): the start, then a
        // round per insert, the next insert a tick after.
        if w.state.shells != ShellStage::Idle && due >= w.state.shell_next {
            let (insert, finish, back_to_back) =
                w.shells.map_or((0.0, 0.0, false), |s| (s.insert as f64, s.finish as f64, s.back_to_back));
            // Back to back, each part runs from when the last was due.
            let from = if back_to_back { w.state.shell_next } else { now };
            let room = w.magazine.as_ref().is_some_and(|m| m.clip < m.size && m.reserve > 0);
            match w.state.shells {
                ShellStage::Starting if room => {
                    w.state.shells = ShellStage::Inserting;
                    w.state.shell_next = from + insert;
                    if let Some(s) = w.sounds {
                        w.state.pending = s.reload.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
                    }
                    ctx.w.event(owner, active, WeaponEventKind::ShellInserting);
                }
                ShellStage::Inserting => {
                    if let Some(mag) = w.magazine.as_mut()
                        && mag.reserve > 0
                        && mag.clip < mag.size
                    {
                        mag.clip += 1;
                        mag.reserve -= 1;
                    }
                    w.state.shells = ShellStage::Starting;
                    w.state.shell_next = now;
                    let room = w.magazine.as_ref().is_some_and(|m| m.clip < m.size && m.reserve > 0);
                    if room && back_to_back {
                        // The next round starts as this one goes in.
                        w.state.shells = ShellStage::Inserting;
                        w.state.shell_next = from + insert;
                        if let Some(s) = w.sounds {
                            w.state.pending = s.reload.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
                        }
                        ctx.w.event(owner, active, WeaponEventKind::ShellInserting);
                    } else if !room {
                        if let Some(s) = w.sounds {
                            w.state.pending = s.finish.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
                        }
                        if finish > 0.0 {
                            w.state.shells = ShellStage::Finishing;
                            w.state.shell_next = from + finish;
                        } else {
                            w.state.shells = ShellStage::Idle;
                            ctx.w.event(owner, active, WeaponEventKind::Reloaded);
                        }
                    }
                }
                _ => {
                    w.state.shells = ShellStage::Idle;
                    ctx.w.event(owner, active, WeaponEventKind::Reloaded);
                }
            }
        }

        // The rest of a burst fires on its own, button or not (M16).
        if w.state.burst_left > 0 && due >= w.state.next_burst_round {
            w.state.burst_left -= 1;
            w.state.next_burst_round += w.burst.map_or(0.0, |b| b.interval) as f64;
            if w.fire_rounds(&mut ctx, 1) == 0 {
                w.state.burst_left = 0;
            }
        }

        // 3. Secondary attack has priority: a melee swing or the next mode.
        let mut blocked = false;
        if intent.secondary
            && let Some(swing) = w.melee.and_then(|m| m.secondary.clone())
            && w.state.next_secondary <= due
        {
            let swing = follow_up(&swing, w.state.last_swing[1], now);
            let hit = ctx.swing(&swing, true);
            let (own, other) = swing_refire(&swing, hit, now, &mut w.state.last_swing[1]);
            w.state.next_secondary = own;
            w.state.next_primary = other;
            blocked = true;
        } else if intent.secondary
            && let Some(modes) = w.modes.as_ref().map(|m| (**m).clone())
            && w.state.next_secondary <= due
        {
            // Each step sets only the secondary timer (measured M15), but a
            // silencer blocks both (M16).
            w.state.rezoom = None;
            w.state.next_secondary = now + modes.toggle_time as f64;
            if modes.blocks_primary {
                w.state.next_primary = w.state.next_secondary;
            }
            if let Some(s) = modes.sound {
                ctx.w.sound(owner_sound(s, owner, eye, CHAN_ITEM));
            }
            let next = (modes.current + 1) % modes.count.max(1);
            if let Some(s) = w.sounds {
                let timed = s.modes.iter().filter(|(m, ..)| *m == next);
                w.state.pending = timed.map(|(_, t, e)| (now + *t as f64, e.clone())).collect();
            }
            w.set_mode(next, owner, ctx.w);
            blocked = true;
        }

        // An empty clip reloading by itself (`Magazine::auto_reload`).
        if !w.state.reload_held
            && w.magazine.as_ref().is_some_and(|m| m.auto_reload && m.clip == 0 && m.reserve > 0)
            && w.state.next_primary <= due
            && w.state.next_secondary <= due
            && !w.state.reloading()
            && !in_postfire(&w.state)
        {
            try_reload(&mut w, &mut inv, now, owner, ctx.w);
        }

        // 4. Primary attack (or the chain's round, `Trigger::latency`).
        // An automatic waits for the closing part; a semi-automatic's press
        // cuts it short.
        let held_by_postfire = in_postfire(&w.state) && w.trigger.is_some_and(|t| t.automatic);
        if (intent.fire || w.state.armed_round)
            && !blocked
            && w.state.next_primary <= due
            && w.state.arming == 0
            && !held_by_postfire
        {
            let chain_round = std::mem::take(&mut w.state.armed_round);
            let automatic = w.trigger.is_none_or(|t| t.automatic);
            let per_press = w.trigger.map_or(0, |t| t.per_press);
            let empty = w.magazine.as_ref().is_some_and(|m| m.clip == 0);
            let once = w
                .burst
                .is_some_and(|b| b.once_per_press && b.mode == w.mode() && b.count > 0);
            if (!automatic || once) && w.state.burst > 0 {
                // Semi-automatic (or a burst per pull): wait for a fresh
                // press.
            } else if per_press > 0 && w.state.burst >= per_press {
                // As many shots as one pull allows.
            } else if w.state.shells != ShellStage::Idle && empty {
                // Loading the first round: the reload goes on.
            } else if w.magazine.as_ref().is_some_and(|m| m.clip == 0) {
                if w.state.fired_on_empty {
                    if w.magazine.as_ref().is_some_and(|m| m.reload_while_held) {
                        try_reload(&mut w, &mut inv, now, owner, ctx.w);
                    }
                } else {
                    // Fire on empty (spec 3.5).
                    w.state.fired_on_empty = true;
                    if now >= w.state.next_empty_sound {
                        w.state.next_empty_sound = now + EMPTY_SOUND_INTERVAL;
                        if let Some(s) = w.sounds.and_then(|s| s.empty.clone()) {
                            ctx.w.sound(owner_sound(s, owner, eye, CHAN_ITEM));
                        }
                    }
                    w.state.next_primary = now + 0.2;
                    ctx.w.event(owner, active, WeaponEventKind::DryFire);
                }
            } else if latency > 0 && !w.state.firing {
                // Enter the firing state: the round comes `latency`
                // updates on, the trigger held or not; a shell reload
                // stops now.
                w.state.firing = true;
                w.state.arming = latency;
                w.state.armed_round = true;
                w.state.shells = ShellStage::Idle;
            } else {
                if pressed || released2 || chain_round {
                    // A fresh press drops any accumulated lag (spec 3.2);
                    // so does the chain's first round.
                    w.state.next_primary = now;
                }
                w.state.postfire_end = None;
                // A shot stops a one-at-a-time reload.
                w.state.shells = ShellStage::Idle;
                if let Some(swing) = w.melee.map(|m| m.primary.clone()) {
                    let swing = follow_up(&swing, w.state.last_swing[0], now);
                    let hit = ctx.swing(&swing, false);
                    let (own, other) = swing_refire(&swing, hit, now, &mut w.state.last_swing[0]);
                    w.state.next_primary = own;
                    w.state.next_secondary = other;
                    w.state.burst += 1;
                } else if let Some(burst) = w.burst.filter(|b| b.mode == w.mode() && b.count > 0).cloned() {
                    // A burst: the first round now, the rest on their own.
                    if w.fire_rounds(&mut ctx, 1) > 0 {
                        w.state.burst_left = burst.count - 1;
                        w.state.next_burst_round = now + burst.interval as f64;
                    }
                    w.state.next_primary = now + burst.refire as f64;
                    w.state.next_secondary = w.state.next_primary;
                } else if let Some(trigger) = w.trigger.cloned() {
                    let mut shots = 0;
                    match trigger.timing {
                        FireTiming::Set => {
                            shots = 1;
                            w.state.next_primary = now + trigger.cycle as f64;
                            w.state.next_secondary = w.state.next_primary;
                        }
                        FireTiming::Accumulate => {
                            while w.state.next_primary <= due {
                                shots += 1;
                                w.state.next_primary += trigger.cycle as f64;
                            }
                        }
                        FireTiming::CarryOver => {
                            // A fresh press reset the timer to now above.
                            shots = 1;
                            w.state.next_primary += trigger.cycle as f64;
                            w.state.next_secondary = w.state.next_primary;
                        }
                    }
                    let fired = w.fire_rounds(&mut ctx, shots);
                    // A sniper shot unzooms until it can fire again (M15).
                    let mode = w.mode();
                    if fired > 0 && mode > 0 && w.zoom.is_some_and(|z| z.unzoom_after_shot) {
                        w.state.rezoom = Some(mode);
                        w.set_mode(0, owner, ctx.w);
                    }
                }
            }
        }

        // The firing state ends once the trigger is let go, no chain
        // round is pending and the last cycle is over; with a closing
        // part (`Trigger::postfire`) when that is over too (it starts
        // here, also with the trigger held on an empty clip).
        let empty = w.magazine.as_ref().is_some_and(|m| m.clip == 0);
        let let_go = !intent.fire || (postfire > 0.0 && empty);
        if w.state.firing && let_go && !w.state.armed_round && w.state.next_primary <= due {
            if postfire > 0.0 {
                match w.state.postfire_end {
                    None => w.state.postfire_end = Some(now + postfire as f64),
                    Some(end) if end <= due => {
                        w.state.firing = false;
                        w.state.postfire_end = None;
                    }
                    Some(_) => {}
                }
            } else {
                w.state.firing = false;
            }
        }

        // 5. Reload key.
        if intent.reload
            && !w.state.reload_held
            && w.state.next_primary <= due
            && !w.state.reloading()
            && !in_postfire(&w.state)
        {
            try_reload(&mut w, &mut inv, now, owner, ctx.w);
            w.state.fire_duration = 0.0;
        }

        // 6. No buttons: automatic reload of an empty clip.
        if !intent.fire && !intent.secondary && !intent.reload {
            w.state.fired_on_empty = false;
            if w.magazine.as_ref().is_some_and(|m| m.clip == 0)
                && !w.state.reload_held
                && w.state.next_primary <= due
                && w.state.next_secondary <= due
                && !w.state.reloading()
                && !in_postfire(&w.state)
            {
                try_reload(&mut w, &mut inv, now, owner, ctx.w);
            }
        }
    }
}

/// The swing with its follow-up damage when the previous one was recent.
fn follow_up(swing: &Swing, last_swing: Option<f64>, now: f64) -> Swing {
    match (swing.follow_up, last_swing) {
        (Some((window, damage)), Some(t)) if now - t < window as f64 => Swing {
            damage,
            ..swing.clone()
        },
        _ => swing.clone(),
    }
}

/// Next times for (this swing, the other) after a swing, recording it.
fn swing_refire(swing: &Swing, hit: bool, now: f64, last_swing: &mut Option<f64>) -> (f64, f64) {
    *last_swing = Some(now);
    if hit {
        let t = now + swing.hit_refire as f64;
        (t, t)
    } else {
        (now + swing.miss_refire as f64, now + swing.miss_refire_other as f64)
    }
}

/// Start a reload (spec 3.4): refused when nothing would move.
fn try_reload(w: &mut WeaponPartsItem, inv: &mut Inventory, now: f64, owner: Entity, events: &mut deliver::World) {
    let Some(mag) = w.magazine.as_ref() else { return };
    if (mag.size - mag.clip).min(mag.reserve) == 0 {
        return;
    }
    let reload_time = mag.reload_time;
    // Reloading lowers the scope (UNMEASURED: CS:S's snipers do).
    if w.zoom.is_some() {
        w.state.rezoom = None;
        w.set_mode(0, owner, events);
    }
    w.state.burst_left = 0;
    w.state.fired_on_empty = false;
    if let Some(shells) = w.shells {
        // One at a time: attacks wait for the start only (spec 3.4).
        let end = now + shells.start as f64;
        w.state.shells = ShellStage::Starting;
        w.state.shell_next = end;
        w.state.next_primary = end;
        w.state.next_secondary = end;
        events.event(owner, w.entity, WeaponEventKind::ReloadStarted);
        return;
    }
    let end = now + reload_time as f64;
    inv.next_attack = end;
    w.state.next_primary = end;
    w.state.next_secondary = end;
    w.state.reload_end = Some(end);
    if let Some(s) = w.sounds {
        w.state.pending = s.reload.iter().map(|(t, e)| (now + *t as f64, e.clone())).collect();
    }
    events.event(owner, w.entity, WeaponEventKind::ReloadStarted);
}

/// Characters look through their active weapon's zoom (`Zoomed`; a weapon
/// with `OwnZoom` leaves it to the game) and move at its zoomed speed.
#[allow(clippy::type_complexity)]
fn apply_zoom(
    owners: Query<(Entity, &Inventory, Option<&Zoomed>, Option<&MaxSpeed>), Without<NetDrawn>>,
    weapons: Query<(&Weapon, Option<&AltModes>, Option<&Zoom>, Has<OwnZoom>)>,
    mut commands: Commands,
) {
    for (owner, inv, zoomed, speed) in &owners {
        let Some((weapon, modes, zoom, own)) = inv.active.and_then(|a| weapons.get(a).ok()) else {
            if zoomed.is_some() {
                commands.entity(owner).remove::<Zoomed>();
            }
            continue;
        };
        let mode = modes.map_or(0, |m| m.current) as usize;
        let now = zoom.filter(|_| mode > 0).and_then(|z| {
            Some((
                Zoomed {
                    fov: *z.fov.get(mode - 1)?,
                    scope: z.scope,
                    sensitivity: None,
                },
                z.max_speed.or(weapon.max_speed),
            ))
        });
        let want_speed = now.map_or(weapon.max_speed, |(_, s)| s);
        // The game's own zoom (`OwnZoom`) keeps its `Zoomed` itself.
        if !own && zoomed != now.as_ref().map(|(z, _)| z) {
            match now {
                Some((z, _)) => commands.entity(owner).insert(z),
                None => commands.entity(owner).remove::<Zoomed>(),
            };
        }
        if speed.map(|s| s.0) != want_speed {
            match want_speed {
                Some(s) => commands.entity(owner).insert(MaxSpeed(s)),
                None => commands.entity(owner).remove::<MaxSpeed>(),
            };
        }
    }
}

/// Play sounds whose time has come (reload parts), from the owner.
/// Kicks to players' views from the map (env_viewpunch) add to their view
/// punch, which the game's recoil decays.
fn view_kicks(mut kicks: MessageReader<crate::map::ViewKick>, mut punches: Query<&mut ViewPunch>, mut commands: Commands) {
    for k in kicks.read() {
        match punches.get_mut(k.player) {
            Ok(mut p) => p.0 += k.angles,
            Err(_) => {
                if let Ok(mut e) = commands.get_entity(k.player) {
                    e.insert(ViewPunch(k.angles));
                }
            }
        }
    }
}

/// Each bullet path (a shot and its continuations through walls) is also
/// traced against ragdolls, which it passes through but pushes
/// (specs/cs_source/ragdolls.md 6.2).
fn ragdoll_shots(mut events: MessageReader<WeaponEvent>, mut shots: MessageWriter<crate::map::RagdollShot>) {
    for e in events.read().filter(|e| e.shown()) {
        if let WeaponEventKind::Shot { from, to, .. } | WeaponEventKind::ShotContinued { from, to, .. } = e.kind {
            shots.write(crate::map::RagdollShot::bullet(from, to));
        }
    }
}

fn timed_sounds(
    mut weapons: Query<(&Weapon, &mut WeaponState)>,
    owners: Query<&Transform>,
    mut play: MessageWriter<PlaySound>,
    clock: Res<SimClock>,
    first: Res<FirstTimePredicted>,
) {
    let now = clock.now;
    for (w, mut st) in &mut weapons {
        let Some(owner) = w.owner else { continue };
        if st.pending.is_empty() {
            continue;
        }
        let at = owners.get(owner).map_or(Vec3::ZERO, |t| t.translation);
        st.pending.retain(|(t, entry)| {
            if *t <= now {
                if first.0 {
                    play.write(owner_sound(entry.clone(), owner, at, CHAN_ITEM));
                }
                false
            } else {
                true
            }
        });
    }
}
