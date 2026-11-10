//! CS:S's weapons as data (docs/weapon-data.md): one TOML file per weapon
//! in games/cs_source/data/weapons (built in, parsed at run time by
//! `weapon::data`: `cs_weapons`), with the values from the CS:S weapon
//! scripts and view models (specs/cs_source/weapons.md, "Weapon data") and
//! the rules measured on the game ("CS:S values (measured)"). The scripts
//! are encrypted in the install (their key isn't ours to ship), so the data
//! holds the spec's tables, which were read from the user's install, and
//! nothing is read from the scripts at run time; cs_guns.rs checks the data
//! against the spec's tables. Sound entries and model paths are the
//! install's own names (its sound scripts and model files), checked by
//! `games/cs_source/tests/it/heavy/map_de_dust2.rs`.
//!
//! What data can't say is Rust here, registered as the hooks the data names
//! (`register_hooks`): `cs_source:accuracy` (`Inaccuracy`, the accuracy
//! penalty), `cs_source:recoil` (`Recoil`, view punch per shot),
//! `cs_source:hit_slowdown` (`hit_reaction::HitSlowdown`, measured M18),
//! `cs_source:alternate_hands` (the Elites), `cs_source:grenade`
//! (`grenades::throwable`) and `cs_source:c4` (`objectives::c4_part`);
//! the gun game's power curve reads the accuracy's numbers as a spread
//! (`power_accuracy`).
//! `CsGunsPlugin` is the weapons alone (data, hooks, accuracy, recoil,
//! penetration materials: what a mashup offers in another game's slots);
//! `CsWeaponsPlugin` is CS:S's game, with the shop, rules and effects.
//! The punch decays only while CS:S's recoil owns it
//! (`weapon::impact::RecoilGame`: a CS:S weapon in hand).
//! Bullet penetration is the shared `Penetration` part with CS:S's numbers
//! (data/penetration.toml: `pass_materials`, measured M13; each ammo type's
//! power and reach in ammo.toml). The shop is data too
//! (data/economy.toml, docs/economy-data.md: `cs_economy`): the items that
//! aren't weapons, the buy menu's layout, the spawn kit and the shop's
//! sounds, loaded after the weapons.
//!
//! Known shortcuts (docs/tech-debt.md): The CS:S guns (all 24, data in
//! `games/cs_source/data/weapons`): unmeasured parts are marked `UNMEASURED`
//! there: the 556MM and 9MM penetration powers/distances and 45ACP's distance
//! are community values (the same table's 45ACP, 50AE, 762MM and 338MAG powers
//! match M13); 357SIG, 57MM and BUCKSHOT take 45ACP's power and distance,
//! 556MM_BOX 556MM's; recoil is measured only for the AK-47, M4A1 and the
//! USP/Deagle/AWP/Glock: FAMAS, Galil, AUG, SG552 and the SMGs use the M4A1's
//! sets, the M249 the AK-47's, the scout, SG550, G3SG1, P228, Five-SeveN and
//! Elites the semi-automatics' 2° with no cap, the shotguns the SDK template's
//! random 4-6° (8-11° airborne), once per shot, decaying by the AK-47's
//! measured rule, bullets along view + 2 × punch (the template's and the
//! measured rifles'; shotgun kick, decay and aim factor unmeasured); the M4A1's
//! moving/airborne kicks are its standing set; scoped, silenced and burst
//! recoil are the normal ones (scoped measured so: AWP and scout 2° up, SG550
//! and G3SG1 their random kicks; a burst kicks on its first round only,
//! measured); silenced damage and range are unchanged (M16 not
//! measured); the Glock's burst refire is 0.5 s; reloading unzooms; the SG552's
//! zoomed speed is unchanged and the G3SG1's is the SG550's 150. Shotguns: the
//! shell-by-shell reload is measured (M9: 0.5 s start, 0.5 s a shell for
//! both; the models' durations differ), pellets share one inaccuracy
//! offset per shot and each draws its own Spread part (Q12), every pellet is a
//! line trace. The Elites alternate hands by the clip's parity (left on odd
//! rounds left, ours). Bots' buying weights (`buy.bot_weight`) are ours. Zoomed
//! mouse speed is `zoom_sensitivity_ratio` (1.2) × zoom FOV / 90 as the
//! community documents it, not measured; the scope overlay uses the game's
//! `sprites/scope_arc` and `overlays/scope_lens` laid out by eye (quarters
//! mirrored over the screen-height square, 1-pixel cross hairs), not compared
//! against the game. Timers compare with a 10 µs slack (`TIME_SLACK`) so
//! exact-tick durations fire on that tick as CS:S's 32-bit times do. Why it's
//! OK for now: Measured behaviour is all in; the rest needs the probe (list in
//! plans/active/weapons.md).

use std::sync::OnceLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::{
    core::{Intent, MovementState, Velocity},
    weapon::{
        AltModes, CarriedAppExt, Hitscan, Inventory, NetPart, PassMaterials, SpreadShape, StartingWeapons, ViewPunch,
        Weapon, WeaponEvent, WeaponEventKind, WeaponFrame,
        data::{WeaponData, WeaponDataAppExt, WeaponSet},
        economy_data::{EconomyData, EconomyDataAppExt},
    },
};

const UNIT: f32 = 0.0254;

macro_rules! data_file {
    ($name:literal) => {
        ($name, include_str!(concat!("../../data/weapons/", $name)))
    };
}

/// Script damage is in hit points; ours is normalized (100 hp = 1.0).
pub(super) const HP: f32 = 0.01;

/// CS:S's weapon data (games/cs_source/data/weapons; the format:
/// docs/weapon-data.md), built into the game and parsed at run time, in
/// registration order (network messages name weapons by registry index:
/// knife, guns, grenades, C4, as before the data).
const DATA: &[(&str, &str)] = &[
    data_file!("_base.toml"),
    data_file!("_gun.toml"),
    data_file!("ammo.toml"),
    data_file!("knife.toml"),
    data_file!("ak47.toml"),
    data_file!("m4a1.toml"),
    data_file!("awp.toml"),
    data_file!("usp.toml"),
    data_file!("glock.toml"),
    data_file!("deagle.toml"),
    data_file!("famas.toml"),
    data_file!("galil.toml"),
    data_file!("aug.toml"),
    data_file!("sg552.toml"),
    data_file!("scout.toml"),
    data_file!("sg550.toml"),
    data_file!("g3sg1.toml"),
    data_file!("mac10.toml"),
    data_file!("tmp.toml"),
    data_file!("mp5navy.toml"),
    data_file!("ump45.toml"),
    data_file!("p90.toml"),
    data_file!("m249.toml"),
    data_file!("p228.toml"),
    data_file!("fiveseven.toml"),
    data_file!("elite.toml"),
    data_file!("m3.toml"),
    data_file!("xm1014.toml"),
    data_file!("hegrenade.toml"),
    data_file!("flashbang.toml"),
    data_file!("smokegrenade.toml"),
    data_file!("c4.toml"),
];

/// CS:S's weapons as data, parsed once per process (a read-only table; an
/// app's `WeaponCatalog` lists it once its plugin adds it).
pub fn cs_weapons() -> &'static WeaponSet {
    static SET: OnceLock<&'static WeaponSet> = OnceLock::new();
    SET.get_or_init(|| {
        let set = WeaponSet::parse(DATA, &[]).unwrap_or_else(|e| panic!("CS:S weapon data: {e}"));
        Box::leak(Box::new(set))
    })
}

/// A CS:S weapon's data by short name (`usp`, `weapon_usp`) or full ID.
pub fn data_named(name: &str) -> Option<&'static WeaponData> {
    let set = cs_weapons();
    set.get(name).or_else(|| {
        set.weapons.iter().find(|w| {
            let short = w.id.rsplit(':').next().unwrap_or(&w.id);
            short == name || short.strip_prefix("weapon_") == Some(name)
        })
    })
}

/// A CS:S weapon's data by ID.
pub fn data(id: &str) -> Option<&'static WeaponData> {
    cs_weapons().get(id)
}

/// Every CS:S gun (a weapon that fires bullets), for tables and tests.
pub fn guns() -> impl Iterator<Item = &'static WeaponData> {
    cs_weapons().weapons.iter().filter(|w| w.hitscan.is_some())
}

/// Every CS:S weapon's display name by weapon id, from a localisation
/// table (lower-case keys, `radio::localization`): the text of its data's
/// `hud.name` token (`Cstrike_WPNHUD_Glock18`). A weapon whose token the
/// table lacks is left out.
pub fn names_in(strings: &std::collections::BTreeMap<String, String>) -> Vec<(String, String)> {
    cs_weapons()
        .weapons
        .iter()
        .filter_map(|w| {
            let token = w.hud.name.as_deref()?.trim_start_matches('#').to_lowercase();
            let text = strings.get(&token)?.trim();
            (!text.is_empty()).then(|| (w.id.clone(), text.to_string()))
        })
        .collect()
}

/// `names_in` for the install's own strings (`resource/cstrike_english.txt`
/// on the mount): what an app showing CS:S's guns under another game's
/// HUD calls them. Empty without the file.
pub fn display_names(mount: &crate::mount::Mount) -> Vec<(String, String)> {
    use super::radio::{decode, localization};
    let path = crate::mount::normalize("resource/cstrike_english.txt");
    let mut strings = std::collections::BTreeMap::new();
    for bytes in mount.read_every(&path) {
        for (k, v) in localization(&decode(&bytes)) {
            strings.entry(k).or_insert(v);
        }
    }
    names_in(&strings)
}

/// The hooks CS:S's weapon data names (`[hooks."<name>"]`).
pub const ACCURACY: &str = "cs_source:accuracy";
pub const RECOIL: &str = "cs_source:recoil";
pub const HIT_SLOWDOWN: &str = "cs_source:hit_slowdown";
pub const ALTERNATE_HANDS: &str = "cs_source:alternate_hands";
pub const GRENADE: &str = "cs_source:grenade";
pub const C4_HOOK: &str = "cs_source:c4";

/// A hook that reads no parameters (`[hooks."<name>"]` empty).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// `cs_source:accuracy`: `Inaccuracy`'s keys, recovery times and the
/// alternate mode's keys.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccuracyParams {
    pub spread: f32,
    pub crouch: f32,
    pub stand: f32,
    pub jump: f32,
    pub land: f32,
    pub fire: f32,
    pub movement: f32,
    pub recovery_crouch: f32,
    pub recovery_stand: f32,
    #[serde(default)]
    pub alt: Option<AccuracyKeys>,
}

impl AccuracyParams {
    pub fn keys(&self) -> AccuracyKeys {
        AccuracyKeys::new([
            self.spread,
            self.crouch,
            self.stand,
            self.jump,
            self.land,
            self.fire,
            self.movement,
        ])
    }
}

/// `cs_source:recoil`: the kick sets (`Recoil`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoilParams {
    pub standing: Kick,
    pub crouched: Kick,
    pub moving: Kick,
    pub airborne: Kick,
}

/// On a weapon whose data names `cs_source:alternate_hands` (the
/// Elites): shots alternate hands by the clip's parity
/// (`elite_right_hand`), in the view and body animations.
#[derive(Component, Clone, Copy, Debug)]
pub struct AlternateHands;

/// Whether the weapon `id` fires hand by hand (`AlternateHands`).
pub fn alternate_hands(id: &str) -> bool {
    data(id).is_some_and(|d| d.has_hook(ALTERNATE_HANDS))
}

/// A shot's spread for the gun game's power curve
/// (`chimera_sim::minigame::power`): the first shot's `Spread` plus the
/// standing inaccuracy, the follow-ups' plus the firing penalty of four
/// times the curve's `firing` shots (0.25: one shot's); a gun with an
/// alternate mode (a scope) takes the tighter of the two, as a player at
/// range would.
fn power_accuracy(
    p: &chimera_sim::minigame::power::HookParams,
    at: &chimera_sim::minigame::power::ReadAt,
    s: &mut chimera_sim::minigame::power::GunStats,
) {
    let Ok(a) = p.clone().try_into::<AccuracyParams>() else { return };
    let tighter = |f: &dyn Fn(AccuracyKeys) -> f32| a.alt.map_or(f(a.keys()), |alt| f(a.keys()).min(f(alt)));
    s.spread = tighter(&|k| k.spread + k.stand);
    s.follow = Some(tighter(&|k| k.spread + k.stand + k.fire * 4.0 * at.firing));
}

/// Register the behaviour CS:S's weapon data names.
pub fn register_hooks(app: &mut App) {
    use chimera_sim::minigame::power::PowerAppExt;
    app.add_power_reader(ACCURACY, power_accuracy);
    app.register_weapon_hook::<AccuracyParams>(ACCURACY, |e, _, p| {
        let acc = Inaccuracy::new(p.keys(), p.alt, (p.recovery_crouch, p.recovery_stand));
        if let Some(mut scan) = e.get_mut::<Hitscan>() {
            scan.spread = SpreadShape::Disc {
                inaccuracy: acc.value,
                spread: p.spread,
            };
        }
        e.insert(acc);
    })
    .register_weapon_hook::<RecoilParams>(RECOIL, |e, _, p| {
        e.insert(Recoil {
            standing: p.standing,
            crouched: p.crouched,
            moving: p.moving,
            airborne: p.airborne,
            ..default()
        });
    })
    .register_weapon_hook::<super::hit_reaction::HitSlowdown>(HIT_SLOWDOWN, |e, _, p| {
        e.insert(*p);
    })
    .register_weapon_hook::<NoParams>(ALTERNATE_HANDS, |e, _, _| {
        e.insert(AlternateHands);
    })
    .register_weapon_hook::<super::grenades::GrenadeParams>(GRENADE, super::grenades::throwable)
    .register_weapon_hook::<NoParams>(C4_HOOK, |e, _, _| super::objectives::c4_part(e));
}

/// World models (held by characters): each weapon's.
pub fn world_models() -> Vec<(&'static str, &'static str)> {
    cs_weapons()
        .weapons
        .iter()
        .filter_map(|w| Some((w.id.as_str(), w.models.world.as_deref()?)))
        .collect()
}

/// World models with the silencer on (the data's `world_alt`), held under
/// `silenced_key(id)` while the silencer is on.
pub fn silenced_world_models() -> Vec<(&'static str, &'static str)> {
    cs_weapons()
        .weapons
        .iter()
        .filter_map(|w| Some((w.id.as_str(), w.models.world_alt.as_deref()?)))
        .collect()
}

/// View models (seen by the local player) and whether each is built
/// right-handed (spec view_models.md 2). The scripts are encrypted, so
/// handedness comes from the models: the AK is held left of the eye
/// (left-handed), the knife right of it
/// (`games/cs_source/tests/it/heavy/map_de_dust2.rs::view_model_handedness_in_the_files`).
/// With the default `cl_righthand 1` the left-handed ones are mirrored, so
/// all end up in the right hand, as CS:S shows them.
pub fn view_models() -> Vec<(&'static str, &'static str, bool)> {
    cs_weapons()
        .weapons
        .iter()
        .filter_map(|w| Some((w.id.as_str(), w.models.view.as_deref()?, w.models.view_right_handed)))
        .collect()
}

pub const KNIFE: &str = "cs_source:weapon_knife";
pub const AK47: &str = "cs_source:weapon_ak47";
pub const M4A1: &str = "cs_source:weapon_m4a1";
pub const AWP: &str = "cs_source:weapon_awp";
pub const USP: &str = "cs_source:weapon_usp";
pub const GLOCK: &str = "cs_source:weapon_glock";
pub const DEAGLE: &str = "cs_source:weapon_deagle";
pub const FAMAS: &str = "cs_source:weapon_famas";
pub const GALIL: &str = "cs_source:weapon_galil";
pub const AUG: &str = "cs_source:weapon_aug";
pub const SG552: &str = "cs_source:weapon_sg552";
pub const SCOUT: &str = "cs_source:weapon_scout";
pub const SG550: &str = "cs_source:weapon_sg550";
pub const G3SG1: &str = "cs_source:weapon_g3sg1";
pub const MAC10: &str = "cs_source:weapon_mac10";
pub const TMP: &str = "cs_source:weapon_tmp";
pub const MP5NAVY: &str = "cs_source:weapon_mp5navy";
pub const UMP45: &str = "cs_source:weapon_ump45";
pub const P90: &str = "cs_source:weapon_p90";
pub const M249: &str = "cs_source:weapon_m249";
pub const P228: &str = "cs_source:weapon_p228";
pub const FIVESEVEN: &str = "cs_source:weapon_fiveseven";
pub const ELITE: &str = "cs_source:weapon_elite";
pub const M3: &str = "cs_source:weapon_m3";
pub const XM1014: &str = "cs_source:weapon_xm1014";

/// The held-model key of a weapon with its silencer on.
pub fn silenced_key(id: &str) -> String {
    format!("{id}#silenced")
}

/// Sound entries the weapons use, for precaching with the map's sounds:
/// `SOUNDS`, every weapon's (its data's) and the shop's (`cs_economy`).
pub fn sounds() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = SOUNDS.to_vec();
    out.extend(super::grenades::SOUNDS);
    out.extend(super::objectives::SOUNDS);
    // The shop's: buying ammo, a weapon, an item; night vision on and off.
    out.extend(cs_economy().sounds.entries());
    for w in &cs_weapons().weapons {
        let swings = w
            .melee
            .iter()
            .flat_map(|m| [Some(&m.primary), m.secondary.as_ref()])
            .flatten();
        let swung = swings
            .flat_map(|s| [&s.sound_hit, &s.sound_hit_world, &s.sound_miss])
            .flatten();
        for s in w
            .sounds
            .iter()
            .flat_map(|s| s.entries())
            .chain(swung.map(String::as_str))
        {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// Sound entries other than the guns' own.
pub const SOUNDS: &[&str] = &[
    "Weapon_Knife.Deploy",
    "Weapon_Knife.Hit",
    "Weapon_Knife.HitWall",
    "Weapon_Knife.Slash",
    "Weapon_Knife.Stab",
    "Default.Zoom",
    "Bounce.PistolShell",
    "Bounce.RifleShell",
    "Bounce.ShotgunShell",
];

/// CS:S's other data (the formats: docs/economy-data.md, and
/// docs/weapon-data.md for penetration), built in and parsed at run time:
/// the shop (items, buy menu, spawn kit, its sounds) and how bullets pass
/// materials (measured M13).
const ECONOMY_DATA: (&str, &str) = ("economy.toml", include_str!("../../data/economy.toml"));
const PENETRATION_DATA: (&str, &str) = ("penetration.toml", include_str!("../../data/penetration.toml"));

/// CS:S's economy document, parsed once per process.
pub fn cs_economy() -> &'static EconomyData {
    static DOC: OnceLock<EconomyData> = OnceLock::new();
    DOC.get_or_init(|| {
        EconomyData::parse(ECONOMY_DATA.0, ECONOMY_DATA.1).unwrap_or_else(|e| panic!("CS:S economy data: {e}"))
    })
}

/// How CS:S's bullets pass things (games/cs_source/data/penetration.toml).
pub fn pass_materials() -> PassMaterials {
    PassMaterials::parse(PENETRATION_DATA.0, PENETRATION_DATA.1).unwrap_or_else(|e| panic!("CS:S penetration data: {e}"))
}

/// CS:S's weapons alone, for another game's gameplay (a mashup offering
/// them in its slots): the weapon data with the hooks it names, the
/// accuracy penalty and recoil around the weapon frame (predicted) and how
/// bullets pass materials. None of CS:S's shop, spawn kit, pickup and
/// spray rules, impact sounds and effects, grenades', fire's and
/// objectives' systems or animation: `CsWeaponsPlugin` (CS:S's own game)
/// adds those after it.
pub struct CsGunsPlugin;

impl Plugin for CsGunsPlugin {
    fn build(&self, app: &mut App) {
        register_hooks(app);
        // A CS:S rung's player model (`rungs`).
        app.add_plugins(super::rungs::CsRungsPlugin);
        // Its surfaces by class, for impacts across games (`map::impact`).
        crate::map::impact::SurfaceClassesAppExt::register_surface_classes(app, super::impacts::SURFACES);
        if let Err(e) = app.add_weapon_set(cs_weapons()) {
            panic!("CS:S weapon data: {e}");
        }
        app.add_message::<WeaponEvent>()
            // Inaccuracy, punch and recoil are predicted with the weapon
            // frame (`core::Predict`).
            .add_systems(
                crate::core::Predict::Weapons,
                (before_shots.before(WeaponFrame), after_shots.after(WeaponFrame)),
            )
            .predicted_part::<Inaccuracy>()
            .predicted_part::<Recoil>()
            .insert_resource(pass_materials());
    }
}

/// CS:S's weapons as its game plays them: `CsGunsPlugin` and the rest
/// around them (the shop and spawn kit, pickup and spray rules, impact
/// sounds and effects, grenades, fire, objectives, hit reactions, pain
/// sounds, the server's body animation).
pub struct CsWeaponsPlugin;

impl Plugin for CsWeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CsGunsPlugin);
        // The burst guns' switch notices for HUDs without the install's
        // strings (the minimal HUD, Combat Arms' with a CS:S gun in hand).
        {
            let mut words = app.world_mut().get_resource_or_insert_with(crate::weapon::notice::NoticeStrings::default);
            for (token, text) in [
                ("#Cstrike_TitlesTXT_Switch_To_BurstFire", "Switched to Burst-Fire mode"),
                ("#Cstrike_TitlesTXT_Switch_To_SemiAuto", "Switched to semi-automatic"),
                ("#Cstrike_TitlesTXT_Switch_To_FullAuto", "Switched to automatic"),
            ] {
                words.add(token, text);
            }
        }
        // The shop after the weapons it names; a spawn kit the app set
        // before the game (tests) stays.
        let kept = app
            .world()
            .get_resource::<StartingWeapons>()
            .filter(|s| !s.is_empty())
            .cloned();
        if let Err(e) = app.add_economy_data(&[ECONOMY_DATA]) {
            panic!("CS:S economy data: {e}");
        }
        if let Some(kit) = kept {
            app.insert_resource(kit);
        }
        // Touching a gun of a type carried takes nothing (measured: M20).
        app.insert_resource(crate::weapon::drop::SameTypeAmmo(false));
        app.insert_resource(crate::weapon::spray::SprayPictureSource(Some(
            source_engine::effects::spray_pictures::picture_of,
        )));
        app.insert_resource(crate::weapon::spray::SprayRules {
            sound: Some(SPRAY_SOUND.into()),
            ..default()
        })
        .add_plugins((
            super::impacts::ImpactSoundsPlugin,
            super::impact_effects::ImpactEffectsPlugin,
            super::grenades::GrenadesPlugin,
            super::hit_reaction::HitReactionPlugin,
            super::fire::FirePlugin,
            super::objectives::CsObjectivesPlugin,
            // A network server's per-tick body animation, for hitboxes.
            super::player_anim::SimAnimPlugin,
        ));
        if !app.is_plugin_added::<super::pain::PainSoundsPlugin>() {
            app.add_plugins(super::pain::PainSoundsPlugin);
        }
    }
}

/// A spray (`impulse 201`; game_sounds.txt: `player/sprayer.wav`).
pub const SPRAY_SOUND: &str = "SprayCan.Paint";

/// Which hand of the dual Elites fires the shot that leaves `clip` rounds:
/// they alternate (spec weapons.md, view-model table), the right one on
/// even counts (ours: the full 30 fires left first).
pub fn elite_right_hand(clip: u32) -> bool {
    clip.is_multiple_of(2)
}

/// The script's accuracy keys for one mode (`Spread`, `Inaccuracy*`; the
/// alternate mode's are the `*Alt` keys), tangent units.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccuracyKeys {
    pub spread: f32,
    pub crouch: f32,
    pub stand: f32,
    pub jump: f32,
    pub land: f32,
    pub fire: f32,
    pub movement: f32,
}

impl AccuracyKeys {
    /// In the spec table's column order: Spread, InaccuracyCrouch,
    /// InaccuracyStand, InaccuracyJump, InaccuracyLand, InaccuracyFire,
    /// InaccuracyMove (InaccuracyLadder is not used yet).
    pub const fn new(v: [f32; 7]) -> Self {
        Self {
            spread: v[0],
            crouch: v[1],
            stand: v[2],
            jump: v[3],
            land: v[4],
            fire: v[5],
            movement: v[6],
        }
    }
}

/// No cap on the punch was seen for the semi-automatics (UNMEASURED).
const NO_CAP: f32 = 90.0;

/// CS:S's accuracy penalty (measured M1/M2; values are the weapon script's
/// `Inaccuracy*`, `RecoveryTime*` and `Spread`, in tangent units). Rests
/// at `stand` (`crouch` when ducked); the excess falls to 10 % in the
/// recovery time; a shot adds `fire`, a jump `jump`, a landing `land` x
/// the fall speed / 301.99 u/s. Moving adds `movement` x
/// clamp((v - vmax/3) / (2 vmax/3)) at shot time. In an alternate mode
/// (`AltModes`: silenced, burst, scoped) the `*Alt` keys apply, with the
/// same recovery times (the scripts have no Alt ones).
#[derive(Component, Clone, Debug)]
pub struct Inaccuracy {
    pub keys: AccuracyKeys,
    pub alt: Option<AccuracyKeys>,
    pub recovery_stand: f32,
    pub recovery_crouch: f32,
    /// The current penalty.
    pub value: f32,
    on_ground: bool,
    /// Vertical speed on the previous tick, u/s (for landings).
    fall_speed: f32,
}

impl Inaccuracy {
    /// From the keys and (`RecoveryTimeCrouch`, `RecoveryTimeStand`); starts
    /// at rest standing.
    pub fn new(keys: AccuracyKeys, alt: Option<AccuracyKeys>, recovery: (f32, f32)) -> Self {
        Self {
            keys,
            alt,
            recovery_crouch: recovery.0,
            recovery_stand: recovery.1,
            value: keys.stand,
            on_ground: true,
            fall_speed: 0.0,
        }
    }

    /// The keys for `mode` (0 = normal).
    pub fn keys(&self, mode: u8) -> AccuracyKeys {
        match (mode, self.alt) {
            (1.., Some(alt)) => alt,
            _ => self.keys,
        }
    }
}

impl NetPart for Inaccuracy {
    /// The penalty, on the ground last tick, last tick's vertical speed.
    type Net = (f32, bool, f32);
    fn net(&self) -> Self::Net {
        (self.value, self.on_ground, self.fall_speed)
    }
    fn set_net(&mut self, (value, on_ground, fall_speed): Self::Net) {
        self.value = value;
        self.on_ground = on_ground;
        self.fall_speed = fall_speed;
    }
}

impl NetPart for Recoil {
    /// Shots so far, the last one's time, the sideways direction.
    type Net = (u32, f64, f32);
    fn net(&self) -> Self::Net {
        (self.shots, self.last_shot, self.direction)
    }
    fn set_net(&mut self, (shots, last_shot, direction): Self::Net) {
        self.shots = shots;
        self.last_shot = last_shot;
        self.direction = direction;
    }
}

/// Airborne decay of the penalty's excess per 0.015 s tick (measured M1:
/// toward the crouched rest after a jump, 10 % left after 1.05 s; a fall
/// without a jump stays at the standing rest, so the rest is a floor).
const AIR_DECAY_PER_TICK: f32 = 0.96752;

/// A landing adds `land` x |fall speed| / this (u/s; measured M1, about the
/// jump speed).
const LAND_SPEED: f32 = 301.99;

/// One recoil kick set (measured M3), degrees: up `up` for the first shot
/// and `up + up_step·n` after (n = shots so far), sideways likewise; the
/// punch is clamped at `up_cap` (pitch) and `side_cap` (yaw).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Kick {
    pub up: f32,
    pub up_step: f32,
    pub side: f32,
    pub side_step: f32,
    pub up_cap: f32,
    pub side_cap: f32,
    /// Above `up`: each shot kicks up a whole number of degrees from `up`
    /// to this at random instead (the shotguns, measured: M3 4-6, XM1014 3-5).
    pub up_max: f32,
    /// Above 0: each shot kicks up `up` plus a random amount in
    /// `0..up_spread` and sideways a random amount in `±side_spread`
    /// instead of the set's steps (the G3SG1 and SG 550, measured).
    pub up_spread: f32,
    pub side_spread: f32,
}

impl Kick {
    pub const fn new(up: (f32, f32), side: (f32, f32), caps: (f32, f32)) -> Self {
        Self {
            up: up.0,
            up_step: up.1,
            side: side.0,
            side_step: side.1,
            up_cap: caps.0,
            side_cap: caps.1,
            up_max: 0.0,
            up_spread: 0.0,
            side_spread: 0.0,
        }
    }

    /// Straight up by a random whole number of degrees in `lo..=hi`.
    pub const fn random_up(lo: f32, hi: f32) -> Self {
        let mut k = Self::new((lo, 0.0), (0.0, 0.0), (NO_CAP, NO_CAP));
        k.up_max = hi;
        k
    }
}

/// CS:S's recoil (measured M3): view punch per shot from the kick set for
/// the shooter's state on the shot tick: airborne, else moving (any
/// horizontal speed), else crouched, else standing. The sideways kick
/// starts in a random direction and flips with a 1/8 chance per shot.
#[derive(Component, Clone, Debug, Default)]
pub struct Recoil {
    pub standing: Kick,
    pub crouched: Kick,
    pub moving: Kick,
    pub airborne: Kick,
    shots: u32,
    last_shot: f64,
    direction: f32,
}

/// Faster than this (u/s) counts as moving for recoil (measured: 5 u/s
/// moves; this only keeps float noise at rest out).
const MOVING_SPEED: f32 = 0.1;

/// The shot count resets after this long without a shot (measured: it
/// holds about 0.45 s, then falls within 0.1 s).
const SHOTS_RESET: f64 = 0.5;

/// Punch decay per second: |p| -= (10 + 0.5 |p|) degrees/s (M3), over
/// all three angles together (M18: a head flinch's (-12, 0, 9) is
/// (-11.79, 0, 8.8425) a tick later).
pub fn decay_punch(p: Vec3, dt: f32) -> Vec3 {
    let len = p.length().to_degrees();
    if len <= 0.0 {
        return Vec3::ZERO;
    }
    let next = (len - (10.0 + 0.5 * len) * dt).max(0.0);
    p * (next / len)
}

/// Before the weapon frame: punch decays, the penalty recovers, and the
/// active weapon's spread is set from the penalty and the movement term.
fn before_shots(
    mut owners: Query<
        (
            &Inventory,
            &MovementState,
            &Velocity,
            &Intent,
            Option<&mut ViewPunch>,
            Option<&crate::weapon::impact::RecoilGame>,
        ),
        Without<crate::map::interp::NetDrawn>,
    >,
    mut weapons: Query<(&Weapon, &mut Inaccuracy, &mut Hitscan, Option<&AltModes>)>,
    clock: Res<crate::core::SimClock>,
) {
    let dt = clock.dt();
    for (inv, state, vel, _intent, punch, recoil_game) in &mut owners {
        // CS:S's punch decay only while its recoil owns the punch (another
        // game's weapon in hand returns it by its own rules:
        // `weapon::impact::RecoilGame`).
        let ours = crate::weapon::impact::RecoilGame::is(recoil_game, super::GAME)
            || inv.active.is_some_and(|a| weapons.contains(a));
        if let Some(mut p) = punch.filter(|_| ours) {
            let next = decay_punch(p.0, dt);
            if p.0 != next {
                p.0 = next;
            }
        }
        let Some(active) = inv.active else { continue };
        let Ok((weapon, mut acc, mut scan, modes)) = weapons.get_mut(active) else {
            continue;
        };
        let keys = acc.keys(modes.map_or(0, |m| m.current));
        let rest = if state.crouching { keys.crouch } else { keys.stand };
        let recovery = if state.crouching {
            acc.recovery_crouch
        } else {
            acc.recovery_stand
        };
        if acc.on_ground && !state.on_ground && vel.0.y > 0.0 {
            acc.value += keys.jump;
        } else if !acc.on_ground && state.on_ground {
            // Scaled by the vertical speed on the tick before (M1).
            acc.value += keys.land * acc.fall_speed.abs() / LAND_SPEED;
        }
        acc.on_ground = state.on_ground;
        acc.fall_speed = vel.0.y / UNIT;
        acc.value = if !state.on_ground {
            let target = keys.crouch;
            (target + (acc.value - target) * AIR_DECAY_PER_TICK.powf(dt / 0.015)).max(rest.min(acc.value))
        } else if acc.value > rest {
            rest + (acc.value - rest) * 0.1f32.powf(dt / recovery)
        } else {
            rest
        };
        let speed = vel.0.xz().length() / UNIT;
        let vmax = weapon.max_speed.unwrap_or(250.0 * UNIT) / UNIT;
        let moving = ((speed - vmax / 3.0) / (vmax * 2.0 / 3.0)).clamp(0.0, 1.0) * keys.movement;
        scan.spread = SpreadShape::Disc {
            inaccuracy: acc.value + moving,
            spread: keys.spread,
        };
    }
}

/// After the weapon frame: each shot adds to the penalty and kicks the
/// view. A shotgun shot is one `Shot` event per pellet but kicks once.
fn after_shots(
    mut events: MessageReader<WeaponEvent>,
    mut weapons: Query<(
        Option<&mut Inaccuracy>,
        Option<&mut Recoil>,
        Option<&AltModes>,
        Option<&Hitscan>,
        Option<(&crate::weapon::Burst, &crate::weapon::WeaponState)>,
    )>,
    mut owners: Query<(&MovementState, &Velocity, &Intent, Option<&mut ViewPunch>)>,
    mut commands: Commands,
    clock: Res<crate::core::SimClock>,
    mut traces: Local<Vec<(Entity, u32)>>,
) {
    let now = clock.now;
    traces.clear();
    for e in events.read() {
        if !matches!(e.kind, WeaponEventKind::Shot { .. }) {
            continue;
        }
        let Ok((acc, recoil, modes, scan, burst)) = weapons.get_mut(e.weapon) else {
            continue;
        };
        // A burst's second and third rounds don't kick (CS:S, measured
        // 2026-10-18, weapons.md Q3: the FAMAS's first round kicked 0.625
        // up and 0.375 aside standing, the two after nothing).
        let follow_up = burst.is_some_and(|(b, st)| {
            modes.map_or(0, |m| m.current) == b.mode && st.burst_left + 1 < b.count
        });
        // The n-th trace of this weapon this tick: a new shot every
        // `pellets` traces.
        let n = match traces.iter_mut().find(|(w, _)| *w == e.weapon) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                traces.push((e.weapon, 0));
                0
            }
        };
        if !n.is_multiple_of(scan.map_or(1, |s| s.pellets.max(1))) {
            continue;
        }
        if let Some(mut acc) = acc {
            // (A sniper shot has already unzoomed: the AWP's fire key is
            // the same in both modes.)
            acc.value += acc.keys(modes.map_or(0, |m| m.current)).fire;
        }
        let Some(mut r) = recoil else { continue };
        if follow_up {
            continue;
        }
        let Ok((state, vel, intent, punch)) = owners.get_mut(e.owner) else {
            continue;
        };
        if now - r.last_shot > SHOTS_RESET {
            r.shots = 0;
        }
        r.last_shot = now;
        r.shots += 1;
        // Shared randoms of the shot's command (spec 4.2): the same on a
        // re-run; the shot count tells several shots of one tick apart.
        let mut dice = crate::weapon::random::shared_random(intent.command, "recoil", r.shots as i32);
        let roll = dice.next();
        if r.shots == 1 {
            r.direction = if roll < 0.5 { -1.0 } else { 1.0 };
        } else if roll < 0.125 {
            r.direction = -r.direction;
        }
        let n = if r.shots == 1 { 0.0 } else { r.shots as f32 };
        let kick = if !state.on_ground {
            r.airborne
        } else if vel.0.xz().length() / UNIT > MOVING_SPEED {
            // Crouched and moving isn't measured: the moving set.
            r.moving
        } else if state.crouching {
            r.crouched
        } else {
            r.standing
        };
        let (mut up, mut side) = (kick.up + kick.up_step * n, kick.side + kick.side_step * n);
        let mut direction = r.direction;
        if kick.up_max > kick.up {
            // A second roll from the same dice.
            let u = dice.next();
            up = (kick.up + (u * (kick.up_max - kick.up + 1.0)).floor()).min(kick.up_max);
        }
        if kick.up_spread > 0.0 || kick.side_spread > 0.0 {
            // Random both ways, from the same dice.
            up = kick.up + dice.next() * kick.up_spread;
            side = (dice.next() * 2.0 - 1.0) * kick.side_spread;
            direction = 1.0;
        }
        let old = punch.as_ref().map_or(Vec3::ZERO, |p| p.0);
        let kicked = Vec3::new(
            (old.x + up.to_radians()).min(kick.up_cap.to_radians()),
            (old.y + side.to_radians() * direction).clamp(-kick.side_cap.to_radians(), kick.side_cap.to_radians()),
            old.z,
        );
        match punch {
            Some(mut p) => p.0 = kicked,
            None => {
                commands.entity(e.owner).insert(ViewPunch(kicked));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punch_decays_linearly_plus_proportionally() {
        // |p| 5 degrees: -(10 + 2.5) x 0.015 = -0.1875 per tick.
        let p = Vec3::new(5f32.to_radians(), 0.0, 0.0);
        let next = decay_punch(p, 0.015).x.to_degrees();
        assert!((next - 4.8125).abs() < 1e-4, "{next}");
        assert_eq!(decay_punch(Vec3::new(0.001, 0.0, 0.0), 0.015), Vec3::ZERO);
    }

    /// Every file in data/weapons is built in, and the data loads with
    /// CS:S's hooks.
    #[test]
    fn the_data_is_built_in_and_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1), Some(text.as_str()), "{name} not in `DATA`");
        }
        assert_eq!(files.len(), DATA.len());
        let mut app = App::new();
        register_hooks(&mut app);
        app.add_weapon_set(cs_weapons()).unwrap();
        assert_eq!(cs_weapons().weapons.len(), 29);
        assert_eq!(guns().count(), 24);
        let knife = data(KNIFE).unwrap();
        assert_eq!(knife.anim.idle_after, super::super::view_anim::KNIFE_IDLE);
        assert!(alternate_hands(ELITE) && !alternate_hands(USP));
    }

    #[test]
    fn display_names_come_from_the_strings_by_the_datas_token() {
        let strings = std::collections::BTreeMap::from([
            ("cstrike_wpnhud_glock18".to_string(), "GLOCK 18".to_string()),
            ("cstrike_wpnhud_awp".to_string(), " ".to_string()),
        ]);
        let names = names_in(&strings);
        assert_eq!(names, [(GLOCK.to_string(), "GLOCK 18".to_string())], "the AWP's blank text and the rest's missing ones are left out");
    }
}
