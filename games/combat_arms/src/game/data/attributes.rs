//! Combat Arms' attribute records: what the game builds from its attribute
//! files (read by `lithtech_formats::attributes`), per
//! specs/combat_arms/attribute_data.md. Each record starts from the
//! spec's built-in defaults ("Default" columns) and takes a file value
//! only when it has an accepted type ("Typed reads").
//!
//! - `WeaponsFile`: the weapons file (`ATTRIBUTES/WEAPONS.TXT`): armour
//!   (`Gear#`), ammo (`Ammo#`), weapons (`Weapon#`, ammo linked by name),
//!   `[WeaponOrder]` and `[Perturb]`, in the game's load order and with
//!   its load failures ("Weapons file: record kinds and load order").
//! - `ServerPlayer`: `[Player]` in the server attributes file
//!   (`SERVERBUTES.TXT`).
//! - `ModelsFile`: body types, characters, head pieces and faces
//!   (`MODELBUTES.TXT`; the pieces' files and skins as `PieceRecord`s).
//! - `ModFile`: the modification file (`MOD.TXT`,
//!   specs/combat_arms/attachments.md): attachment records (`Mod#`, as
//!   `attachments::ModRecord`) and each weapon's accepted list
//!   (`Weapon#`, unknown names dropped with their variant index).
//! - `ProjectileRecord`: the effect file's (`ATTRIBUTES/FX.TXT`)
//!   projectile records (`ProjectileFX#`: special_weapons.md "Projectile
//!   record"), kept on `WeaponsFile` and linked by the ammo's
//!   `ProjectileFXName`; `ImpactRecord` its impact records (`ImpactFX#`:
//!   effects_sounds.md "Effect tables in the attribute data"), linked by
//!   the ammo's `ImpactFXName`.
//! - `AttachmentsFile`: the character attachments file
//!   (`ATTACHMENTS.TXT`): goggles and backpacks (`Goggle#`, `Backpack#`:
//!   socket, model per figure type, skins, render styles).
//!
//! Names are kept as text (ASCII in shipped data; other bytes as
//! Latin-1); `$` strings are kept as they are.
//!
//! Known shortcuts:
//! - Only the fields the spec's tables list are read (weapon presentation
//!   strings and vectors, crosshair styles, `[BombExceptList]`,
//!   conversion, pattern and editor lists are skipped: not needed for the
//!   slice).
//! - `DashGauge` and `GaugeRec` have no stated default ("none"); like
//!   `Maxstamina` ("none (very negative)") they fall back to the smallest
//!   32-bit integer.
//! - `DamageRFacter` is stored as read; `WeaponRecord::damage_roll`
//!   applies the "outside [0, 1] becomes 0.2 when used" rule.
//! - Ammo `Type` has default "unknown": -1 here.

use lithtech_formats::attributes::{Document, Section, c_atof, c_atoi};

use crate::game::attachments::ModRecord;

/// The four files a loader for this spec needs, in-archive paths
/// ("Where the data sits"), as the mount names them.
pub const WEAPONS_FILE: &str = "attributes/weapons.txt";
pub const SERVER_FILE: &str = "attributes/serverbutes.txt";
pub const MODELS_FILE: &str = "attributes/modelbutes.txt";
pub const MOD_FILE: &str = "attributes/mod.txt";
pub const MARKS_FILE: &str = "attributes/marks.txt";
/// The effect tables (`FX.TXT`): the projectile records
/// (special_weapons.md, "Where the numbers live").
pub const FX_FILE: &str = "attributes/fx.txt";
pub const ATTACHMENTS_FILE: &str = "attributes/attachments.txt";
/// The perk and set item files (gear_perks.md).
pub const PERKS_FILE: &str = "attributes/perkskillinfo.txt";
pub const SETS_FILE: &str = "attributes/setitem.txt";

/// The "none" fallback of `Maxstamina`, `DashGauge`, `GaugeRec`.
pub const NO_INT: i32 = i32::MIN;

fn name_of(s: &Section) -> String {
    s.fixed_string("Name", "")
}

/// The case-insensitive, first-match name link ("Links between
/// records"): an empty name never matches.
pub fn find_by_name<'a, T>(items: &'a [T], name: &str, name_of: impl Fn(&T) -> &str) -> Option<(usize, &'a T)> {
    if name.is_empty() {
        return None;
    }
    items.iter().enumerate().find(|(_, t)| {
        let n = name_of(t);
        !n.is_empty() && n.eq_ignore_ascii_case(name)
    })
}

/// The string field `<base><n>` (empty when missing or not a string).
fn numbered(s: &Section, base: &str, n: usize) -> String {
    s.string(&format!("{base}{n}")).map(text).unwrap_or_default()
}

fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// `Gear<N>`: armour.
#[derive(Clone, Debug, PartialEq)]
pub struct GearRecord {
    pub name: String,
    pub armor: f64,
    pub health: f64,
    pub exclusive: i32,
    pub protection_type: Vec<i32>,
    /// Fractions absorbed, one per protection type, clamped to [0, 1].
    pub head: Vec<f64>,
    pub torso: Vec<f64>,
    pub arm: Vec<f64>,
    pub leg: Vec<f64>,
    pub consumable: Vec<i32>,
}

impl GearRecord {
    pub fn read(s: &Section) -> Self {
        let fractions = |k: &str| s.number_list(k).into_iter().map(|v| v.clamp(0.0, 1.0)).collect();
        Self {
            name: name_of(s),
            armor: s.number("Armor", 0.0),
            health: s.number("Health", 0.0),
            exclusive: s.int("Exclusive", 1),
            protection_type: s.int_list("ProtectionType"),
            head: fractions("NodeHeadProtection"),
            torso: fractions("NodeTorsoProtection"),
            arm: fractions("NodeArmProtection"),
            leg: fractions("NodelegProtection"),
            consumable: s.int_list("Consumable"),
        }
    }
}

/// The ammo's spread values ("Field table: ammo"), plain or `Zoomed`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AmmoPerturb {
    pub fire: f64,
    pub fire_move: f64,
    pub fire_duck: f64,
    pub fire_move_duck: f64,
    pub walk: f64,
    pub run: f64,
    pub jump: f64,
    pub recoil: f64,
}

impl AmmoPerturb {
    fn read(s: &Section, prefix: &str) -> Self {
        let n = |k: &str| s.number(&format!("{prefix}{k}"), 0.0);
        Self {
            fire: n("FirePerturb"),
            fire_move: n("FireMovePerturb"),
            fire_duck: n("FireDuckPerturb"),
            fire_move_duck: n("FireMoveDuckPerturb"),
            walk: n("WalkPerturb"),
            run: n("RunPerturb"),
            jump: n("JumpPerturb"),
            recoil: n("PerturbRecoil"),
        }
    }
}

/// `Ammo<N>`.
#[derive(Clone, Debug, PartialEq)]
pub struct AmmoRecord {
    pub name: String,
    pub kind: i32,
    pub priority: f64,
    pub max_amount: i32,
    pub spawned_amount: i32,
    pub selection_amount: i32,
    pub inst_damage: i32,
    pub alt_inst_damage: i32,
    pub inst_damage_type: i32,
    /// `Rangedamage0..`, converted from strings as decimals.
    pub range_damage: Vec<f64>,
    pub alt_range_damage: Vec<f64>,
    pub area_damage: i32,
    pub area_damage_radius: i32,
    pub area_damage_type: i32,
    pub prog_damage: f64,
    pub prog_damage_duration: f64,
    pub prog_damage_radius: f64,
    pub prog_damage_lifetime: f64,
    pub prog_damage_type: i32,
    pub stunned_time: f64,
    pub stunned_move_penalty: f64,
    pub can_adjust_inst_damage: i32,
    pub perturb: AmmoPerturb,
    pub zoomed_perturb: AmmoPerturb,
    /// ms.
    pub dec_delay: i32,
    /// ms.
    pub jitter_dec_delay: i32,
    pub jitter_wait_recover: f64,
    pub jitter_recover: f64,
    pub damage_move_penalty: f64,
    pub damage_move_penalty_duration: f64,
    pub health: f64,
    /// `ProjectileFXName`: the projectile record it fires (`FX.TXT`).
    pub projectile_fx: String,
    /// `ImpactFXName`: the impact record its hits and blasts show
    /// (`FX.TXT` `ImpactFX#`, effects_sounds.md); None when the key is
    /// missing (no effect), empty when present but empty (`BulletFmj`).
    pub impact_fx: Option<String>,
    /// `FireFXName` (casings, the shooter's muzzle group) and
    /// `TracerFXName`: None when the key is missing (no effect); a
    /// present but empty tracer name is `Standard`, an empty impact name
    /// `BulletFmj` (effects_sounds.md, "Effect tables").
    pub fire_fx: Option<String>,
    pub tracer_fx: Option<String>,
}

impl AmmoRecord {
    pub fn read(s: &Section) -> Self {
        let inst_damage = s.int("InstDamage", 0);
        let bands = |k: &str| s.string_list(k).iter().map(|v| c_atof(v)).collect();
        Self {
            name: name_of(s),
            kind: s.int("Type", -1),
            priority: s.number("Priority", -1.0),
            max_amount: s.int("MaxAmount", 0),
            spawned_amount: s.int("SpawnedAmount", 0),
            selection_amount: s.int("SelectionAmount", 0),
            inst_damage,
            alt_inst_damage: s.int("Alt_InstDamage", inst_damage),
            inst_damage_type: s.int("InstDamageType", -1),
            range_damage: bands("Rangedamage"),
            alt_range_damage: bands("Alt_Rangedamage"),
            area_damage: s.int("AreaDamage", 0),
            area_damage_radius: s.int("AreaDamageRadius", 0),
            area_damage_type: s.int("AreaDamageType", -1),
            prog_damage: s.number("ProgDamage", 0.0),
            prog_damage_duration: s.number("ProgDamageDuration", 0.0),
            prog_damage_radius: s.number("ProgDamageRadius", 0.0),
            prog_damage_lifetime: s.number("ProgDamageLifetime", 0.0),
            prog_damage_type: s.int("ProgDamageType", -1),
            stunned_time: s.number("StunnedTime", 0.0),
            stunned_move_penalty: s.number("StunnedMovePenalty", 0.0),
            can_adjust_inst_damage: s.int("CanAdjustInstDamage", 1),
            perturb: AmmoPerturb::read(s, ""),
            zoomed_perturb: AmmoPerturb::read(s, "Zoomed"),
            // A wrong type keeps an undefined value in the game: the
            // missing-key default here ("Edge cases").
            dec_delay: s.int("DecDelay", 150),
            jitter_dec_delay: s.int("JitterDecDelay", 600),
            jitter_wait_recover: s.number("JitterWaitRecover", 0.1),
            jitter_recover: s.number("JitterRecover", 0.3),
            damage_move_penalty: s.number("DamageMovePenalty", 0.0),
            damage_move_penalty_duration: s.number("DamageMovePenaltyDuration", -1.0),
            health: s.number("Health", 0.0),
            projectile_fx: s.string("ProjectileFXName").map(text).unwrap_or_default(),
            impact_fx: s.string("ImpactFXName").map(text),
            fire_fx: s.string("FireFXName").map(text),
            tracer_fx: s.string("TracerFXName").map(text),
        }
    }
}

/// A weapon's spread limits and rates (plain, zoomed or burst).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    pub min: i32,
    pub max: i32,
    pub inc: f64,
    pub dec: f64,
}

/// `Weapon<N>`.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponRecord {
    pub name: String,
    pub gun_type: i32,
    pub client_weapon_type: i32,
    pub ani_type: i32,
    pub lower_ani_type: i32,
    pub shots_per_clip: i32,
    /// Ammo record IDs, the names that resolved (first: the default).
    pub ammo: Vec<usize>,
    /// Whether `AmmoName0` existed at all (for "no resolvable ammo").
    pub names_ammo: bool,
    pub infinite_ammo: i32,
    pub reload_per_bullet: i32,
    /// `ReloadAfterFire` (weapon_classes.md: leave zoom after a zoomed
    /// shot; not in attribute_data.md's table: 0 when missing).
    pub reload_after_fire: i32,
    /// `AccumPreFire` (weapon_classes.md 3.3: a reversible spin; 0 when
    /// missing).
    pub accum_pre_fire: i32,
    /// `DefaultScopeAttach`: the scope fitted at spawn, by name (the
    /// modifications file's; empty when missing). The other slots'
    /// defaults: `DefaultMuzzleAttach`, `DefaultMagazineAttach`,
    /// `DefaultBulletAttach`, `DefaultMechSightAttach` (attachments.md).
    pub default_scope_attach: String,
    pub default_muzzle_attach: String,
    pub default_magazine_attach: String,
    pub default_bullet_attach: String,
    pub default_mech_sight_attach: String,
    /// `DefaultScopeVisible` (default 1).
    pub default_scope_visible: bool,
    pub vectors_per_round: i32,
    /// World units.
    pub range: i32,
    /// `Effectrange0..`: band limits in world units (strings truncated).
    pub effect_range: Vec<i32>,
    pub alt_effect_range: Vec<i32>,
    pub damage_r_facter: f64,
    /// ms, negative values already 0.
    pub fire_delay: i32,
    pub fire_ani_rate: f64,
    pub rapid_fire_count: i32,
    pub continuous_fire: i32,
    pub hud_fire_mode: i32,
    pub triple_shot_mode: i32,
    pub triple_shot_round_num: i32,
    pub triple_shot_delay: i32,
    pub triple_shot_select_delay: i32,
    pub triple_shot: Cone,
    pub triple_shot_max_cam_recoil_pitch: f64,
    pub triple_shot_base_cam_recoil_pitch: f64,
    pub perturb: Cone,
    pub duck_perturb: i32,
    pub move_duck_perturb: i32,
    pub zoomed: Cone,
    pub dynamic_perturb_factor: f64,
    pub dynamic_perturb_factor_run: f64,
    pub dynamic_perturb_factor_walk: f64,
    pub dynamic_perturb_factor_jump: f64,
    pub zoom_time: f64,
    pub move_penalty: f64,
    pub zoomed_move_penalty: f64,
    pub max_cam_recoil_pitch: f64,
    pub base_cam_recoil_pitch: f64,
    pub base_cam_recoil_accel_pitch: f64,
    pub max_cam_recoil_yaw: f64,
    pub base_cam_recoil_yaw: f64,
    pub max_cam_recoil_wag: f64,
    pub base_cam_recoil_wag_l: f64,
    pub base_cam_recoil_wag_r: f64,
    pub max_random_pitch: f64,
    pub max_random_wag: f64,
    pub base_cam_back_jitter: f64,
    pub back_jitter_recover: f64,
    pub camera_sway: [f64; 4],
    pub disable_crosshair: i32,
    pub cross_hair_type: i32,
    pub no_select: i32,
    pub grenade_speed: f64,
    pub grenade_attach: i32,
    pub wide_fire_max_num: i32,
    pub wide_fire_angle_degree: i32,
    /// `FireSnd` (empty: none).
    pub fire_snd: String,
    /// `SilencedFireSnd1..10` (silencer level l plays entry l - 1),
    /// `DryFireSnd`, `ReloadSnd1..3`, `SelectSnd`, `DeselectSnd` (empty
    /// when missing: none).
    pub silenced_fire_snd: [String; 10],
    pub dry_fire_snd: String,
    /// `Alt_FireSnd` (empty when missing).
    pub alt_fire_snd: String,
    pub reload_snd: [String; 3],
    pub select_snd: String,
    pub deselect_snd: String,
    /// `PVMuzzleFXName` and `HHMuzzleFXName` (empty: no flash), and the
    /// flash-hider groups `PVFlashHiderFXName1..10`, `HHFlashHiderFXName1..10`.
    pub pv_muzzle_fx: String,
    pub hh_muzzle_fx: String,
    pub pv_flash_hider_fx: [String; 10],
    pub hh_flash_hider_fx: [String; 10],
    pub fire_snd_radius: i32,
    pub reload_snd_radius: i32,
    pub select_snd_radius: i32,
    /// `MiscSound1..5` (view-model `sound_key` 10 to 14; the grenade's
    /// pin pull is the first; empty when the record has none or a
    /// non-string there, with no fallback by file name).
    pub misc_sounds: [String; 5],
    /// `PlayAutoThrowSound` (default on): a thrown weapon (`Guntype` 7)
    /// makes the thrower's team call-out at its release.
    pub play_auto_throw_sound: bool,
    /// `SilencedFireRadius1..10`.
    pub silenced_fire_radius: [i32; 10],
    pub drop_by_player_dead: bool,
    pub damage_weapon: bool,
    pub check_reload_ani_time: bool,
    pub hidden: bool,
    /// `PVModelNormal`: the first-person model, an archive path as stored
    /// (empty when missing; view_models.md 1: every stock character's
    /// player-view entry is `Normal`).
    pub pv_model: String,
    /// `PVSkin0..`: its skins by texture index.
    pub pv_skins: Vec<String>,
    /// `HHModel` and `HHSkin0..`: the third-person model and its skins.
    pub hh_model: String,
    pub hh_skins: Vec<String>,
    /// `PVRenderStyle0..` and `HHRenderStyle0..`: the render-style files
    /// of the two models (view_models.md 2, characters.md 2).
    pub pv_render_styles: Vec<String>,
    pub hh_render_styles: Vec<String>,
    /// `Weaponkillimage`: the kill feed's icon of the weapon, an archive
    /// path as stored (empty when missing).
    pub kill_image: String,
    /// `DotSightName`: the reticle image (empty: none).
    pub dot_sight_name: String,
    /// `Pos`, `MuzzlePos`, `BreachOffset` (camera space, world units;
    /// missing: zero).
    pub pos: [f64; 3],
    pub muzzle_pos: [f64; 3],
    pub breach_offset: [f64; 3],
    /// Where fitted parts sit at their sockets (attachments.md "View model
    /// and world model"), each (position, rotation) in the socket's frame:
    /// `MuzzlePVPosOff`/`RotOff`, `ScopePVPosOff`/`RotOff`,
    /// `MuzzleHHPosOff`/`RotOff`, `ScopeHHPosOff`/`RotOff` (missing: zero).
    pub muzzle_pv_off: [[f64; 3]; 2],
    pub scope_pv_off: [[f64; 3]; 2],
    pub muzzle_hh_off: [[f64; 3]; 2],
    pub scope_hh_off: [[f64; 3]; 2],
    /// `PVAttachFXName0..`: the view model's attached effect groups
    /// (view_models.md 3; the chainsaw's idle, the time bombs' lights),
    /// and `PVAttachFXAlwaysShow` (keys can't hide them).
    pub pv_attach_fx: Vec<String>,
    pub pv_attach_fx_always: bool,
}

impl WeaponRecord {
    /// Read a weapon, linking its `AmmoName#` list to `ammo` by name.
    pub fn read(s: &Section, ammo: &[AmmoRecord]) -> Self {
        let offsets = |k: &str| {
            [
                s.vector(&format!("{k}PosOff")).unwrap_or_default(),
                s.vector(&format!("{k}RotOff")).unwrap_or_default(),
            ]
        };
        let n = |k: &str, d: f64| s.number(k, d);
        let i = |k: &str, d: i32| s.int(k, d);
        let names = s.string_list("AmmoName");
        let ammo_ids = names
            .iter()
            .take(10)
            .filter_map(|name| find_by_name(ammo, &text(name), |a| &a.name).map(|(id, _)| id))
            .collect();
        let bands = |k: &str| s.string_list(k).iter().map(|v| c_atoi(v)).collect();
        let mut silenced = [0; 10];
        for (k, r) in silenced.iter_mut().enumerate() {
            *r = i(&format!("SilencedFireRadius{}", k + 1), 0);
        }
        Self {
            name: name_of(s),
            gun_type: i("Guntype", 0),
            client_weapon_type: i("ClientWeaponType", 1),
            ani_type: i("AniType", 5),
            lower_ani_type: i("LowerAniType", 0),
            shots_per_clip: i("ShotsPerClip", 100),
            ammo: ammo_ids,
            names_ammo: !names.is_empty(),
            infinite_ammo: i("InfiniteAmmo", 0),
            // Undefined in the game when missing ("Edge cases"): 0.
            reload_per_bullet: i("ReloadPerBullet", 0),
            reload_after_fire: i("ReloadAfterFire", 0),
            accum_pre_fire: i("AccumPreFire", 0),
            default_scope_attach: s.fixed_string("DefaultScopeAttach", ""),
            default_muzzle_attach: s.fixed_string("DefaultMuzzleAttach", ""),
            default_magazine_attach: s.fixed_string("DefaultMagazineAttach", ""),
            default_bullet_attach: s.fixed_string("DefaultBulletAttach", ""),
            default_mech_sight_attach: s.fixed_string("DefaultMechSightAttach", ""),
            default_scope_visible: s.int("DefaultScopeVisible", 1) != 0,
            vectors_per_round: i("VectorsPerRound", 1),
            range: i("Range", 3000),
            effect_range: bands("Effectrange"),
            alt_effect_range: bands("Alt_Effectrange"),
            damage_r_facter: n("DamageRFacter", 0.1),
            fire_delay: i("FireDelay", 30).max(0),
            fire_ani_rate: n("FireAniRate", 0.0),
            rapid_fire_count: i("RapidFireCount", 1),
            continuous_fire: i("ContinuousFire", 0),
            hud_fire_mode: i("HudFireMode", 0),
            triple_shot_mode: i("TripleShotMode", 0),
            triple_shot_round_num: i("TripleShotRoundNum", 3),
            triple_shot_delay: i("TripleShotDelay", 300),
            triple_shot_select_delay: i("TripleShotSelectDelay", 400),
            triple_shot: Cone {
                min: i("TripleShotMinPerturb", 7),
                max: i("TripleShotMaxPerturb", 100),
                inc: n("TripleShotPerturbIncSpeed", 1.0),
                dec: n("TripleShotPerturbDecSpeed", 1.0),
            },
            triple_shot_max_cam_recoil_pitch: n("TripleShotMaxCamRecoilPitch", 50.0),
            triple_shot_base_cam_recoil_pitch: n("TripleShotBaseCamRecoilPitch", 1.0),
            perturb: Cone {
                min: i("MinPerturb", 100),
                max: i("MaxPerturb", 100),
                inc: n("PerturbIncSpeed", 1.0),
                dec: n("PerturbDecSpeed", 1.0),
            },
            duck_perturb: i("DuckPerturb", 100),
            move_duck_perturb: i("MoveDuckPerturb", 100),
            zoomed: Cone {
                min: i("ZoomedMinPerturb", 1),
                max: i("ZoomedMaxPerturb", 500),
                inc: n("ZoomedPerturbIncSpeed", 3.0),
                dec: n("ZoomedPerturbDecSpeed", 3.0),
            },
            dynamic_perturb_factor: n("DynamicPerturbFactor", 1.0),
            dynamic_perturb_factor_run: n("DynnamicPerturbFactorRun", 1.0),
            dynamic_perturb_factor_walk: n("DynnamicPerturbFactorWalk", 1.0),
            dynamic_perturb_factor_jump: n("DynnamicPerturbFactorJump", 1.0),
            zoom_time: n("ZoomTime", 0.5),
            move_penalty: n("MovePenalty", 0.01),
            zoomed_move_penalty: n("ZoomedMovePenalty", 0.7),
            max_cam_recoil_pitch: n("MaxCamRecoilPitch", 10.0),
            base_cam_recoil_pitch: n("BaseCamRecoilPitch", 1.2),
            base_cam_recoil_accel_pitch: n("BaseCamRecoilAccelPitch", 0.0),
            max_cam_recoil_yaw: n("MaxCamRecoilYaw", 0.0),
            base_cam_recoil_yaw: n("BaseCamRecoilYaw", 0.0),
            max_cam_recoil_wag: n("MaxCamRecoilWag", 30.0),
            base_cam_recoil_wag_l: n("BaseCamRecoilWagL", -1.0),
            base_cam_recoil_wag_r: n("BaseCamRecoilWagR", 1.0),
            max_random_pitch: n("MaxRandomPitch", 0.032),
            max_random_wag: n("MaxRandomWag", 0.015),
            base_cam_back_jitter: n("BaseCamBackJitter", 0.5),
            back_jitter_recover: n("BackJitterRecover", 3.0),
            camera_sway: [
                n("CameraSwayXSpeed", 12.0),
                n("CameraSwayYSpeed", 2.0),
                n("CameraSwayXFreq", 6.0),
                n("CameraSwayYFreq", 20.0),
            ],
            disable_crosshair: i("DisableCrosshair", 0),
            cross_hair_type: i("CrossHairType", 0),
            no_select: i("NoSelect", 0),
            grenade_speed: n("GrenadeSpeed", 1.0),
            grenade_attach: i("GrenadeAttach", 0),
            wide_fire_max_num: i("WideFireMaxNum", 0),
            wide_fire_angle_degree: i("WideFireAngleDegree", 0),
            fire_snd: s.string("FireSnd").map(text).unwrap_or_default(),
            silenced_fire_snd: std::array::from_fn(|k| numbered(s, "SilencedFireSnd", k + 1)),
            dry_fire_snd: s.string("DryFireSnd").map(text).unwrap_or_default(),
            alt_fire_snd: s.string("Alt_FireSnd").map(text).unwrap_or_default(),
            reload_snd: std::array::from_fn(|k| numbered(s, "ReloadSnd", k + 1)),
            select_snd: s.string("SelectSnd").map(text).unwrap_or_default(),
            deselect_snd: s.string("DeselectSnd").map(text).unwrap_or_default(),
            pv_muzzle_fx: s.string("PVMuzzleFXName").map(text).unwrap_or_default(),
            hh_muzzle_fx: s.string("HHMuzzleFXName").map(text).unwrap_or_default(),
            pv_flash_hider_fx: std::array::from_fn(|k| numbered(s, "PVFlashHiderFXName", k + 1)),
            hh_flash_hider_fx: std::array::from_fn(|k| numbered(s, "HHFlashHiderFXName", k + 1)),
            fire_snd_radius: i("FireSndRadius", 1000),
            reload_snd_radius: i("ReloadSndRadius", 300),
            select_snd_radius: i("SelectSndRadius", 300),
            misc_sounds: std::array::from_fn(|k| {
                s.string(&format!("MiscSound{}", k + 1)).map(text).unwrap_or_default()
            }),
            play_auto_throw_sound: s.bool("PlayAutoThrowSound", true),
            silenced_fire_radius: silenced,
            drop_by_player_dead: s.bool("DropByPlayerDead", true),
            damage_weapon: s.bool("DamageWeapon", true),
            check_reload_ani_time: s.bool("CheckReloadAniTime", true),
            hidden: s.bool("Hidden", false),
            pv_model: s.string("PVModelNormal").map(text).unwrap_or_default(),
            pv_skins: s.string_list("PVSkin").iter().map(|v| text(v)).collect(),
            hh_model: s.string("HHModel").map(text).unwrap_or_default(),
            kill_image: s.string("Weaponkillimage").map(text).unwrap_or_default(),
            dot_sight_name: s.string("DotSightName").map(text).unwrap_or_default(),
            hh_skins: s.string_list("HHSkin").iter().map(|v| text(v)).collect(),
            pv_render_styles: s.string_list("PVRenderStyle").iter().map(|v| text(v)).collect(),
            hh_render_styles: s.string_list("HHRenderStyle").iter().map(|v| text(v)).collect(),
            pos: s.vector("Pos").unwrap_or_default(),
            muzzle_pos: s.vector("MuzzlePos").unwrap_or_default(),
            breach_offset: s.vector("BreachOffset").unwrap_or_default(),
            muzzle_pv_off: offsets("MuzzlePV"),
            scope_pv_off: offsets("ScopePV"),
            muzzle_hh_off: offsets("MuzzleHH"),
            scope_hh_off: offsets("ScopeHH"),
            pv_attach_fx: s.string_list("PVAttachFXName").iter().map(|v| text(v)).collect(),
            pv_attach_fx_always: s.bool("PVAttachFXAlwaysShow", false) || i("PVAttachFXAlwaysShow", 0) != 0,
        }
    }

    /// The default ammo's ID (the first that resolved).
    pub fn default_ammo(&self) -> Option<usize> {
        self.ammo.first().copied()
    }

    /// The random damage spread as used: outside [0, 1] it is 0.2.
    pub fn damage_roll(&self) -> f64 {
        if (0.0..=1.0).contains(&self.damage_r_facter) {
            self.damage_r_facter
        } else {
            0.2
        }
    }
}

/// The damage-by-range multiplier of a weapon with an ammo ("Damage by
/// range"): bands only when both lists have at least three elements.
pub fn range_multiplier(effect_range: &[i32], range_damage: &[f64], distance: f64) -> f64 {
    let (Some(e), Some(r)) = (effect_range.get(..3), range_damage.get(..3)) else {
        return 1.0;
    };
    if (0.0..=e[0] as f64).contains(&distance) {
        r[0]
    } else if distance > e[0] as f64 && distance <= e[1] as f64 {
        r[1]
    } else if distance > e[1] as f64 && distance <= e[2] as f64 {
        r[2]
    } else {
        1.0
    }
}

/// `[Perturb]`: global spread speeds, each read only if present.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerturbGlobals {
    pub movement_increase: f64,
    pub movement_decrease: f64,
    pub firing_increase: f64,
    pub firing_decrease: f64,
    pub rotation_effect: f64,
}

impl Default for PerturbGlobals {
    fn default() -> Self {
        Self {
            movement_increase: 3.0,
            movement_decrease: 0.7,
            firing_increase: 4.0,
            firing_decrease: 1.0,
            rotation_effect: 2.0,
        }
    }
}

/// The weapons file, built as the game builds it.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponsFile {
    pub gear: Vec<GearRecord>,
    pub ammo: Vec<AmmoRecord>,
    pub weapons: Vec<WeaponRecord>,
    /// `[WeaponOrder]`: each class name and its weapons' IDs in order.
    pub order: Vec<(String, Vec<usize>)>,
    /// `AutoSwitchWeapon0..` as weapon IDs (unknown names dropped).
    pub auto_switch: Vec<usize>,
    pub perturb: PerturbGlobals,
    /// The effect file's projectile records (`ProjectileFX#`; empty
    /// without the file), linked by the ammo's `ProjectileFXName`.
    pub projectiles: Vec<ProjectileRecord>,
    /// The effect file's impact records (`ImpactFX#`; empty without the
    /// file), linked by the ammo's `ImpactFXName`.
    pub impacts: Vec<ImpactRecord>,
    /// The effect file's fire records (`FireFX#`; empty without the
    /// file), linked by the ammo's `FireFXName`.
    pub fires: Vec<FireRecord>,
}

impl WeaponsFile {
    /// Build the records; `Err` where the game fails to load (no ammo, no
    /// weapons, an order list naming no weapon).
    pub fn read(doc: &Document) -> Result<Self, String> {
        let gear = doc.records("Gear").into_iter().map(GearRecord::read).collect();
        let ammo: Vec<_> = doc.records("Ammo").into_iter().map(AmmoRecord::read).collect();
        if ammo.is_empty() {
            return Err("no Ammo0 record".into());
        }
        let weapons: Vec<_> = doc.records("Weapon").into_iter().map(|s| WeaponRecord::read(s, &ammo)).collect();
        if weapons.is_empty() {
            return Err("no Weapon0 record".into());
        }
        let weapon_id = |name: &[u8]| find_by_name(&weapons, &text(name), |w| &w.name).map(|(id, _)| id);
        let mut order = Vec::new();
        let mut auto_switch = Vec::new();
        if let Some(s) = doc.section("WeaponOrder") {
            for class in s.string_list("Class") {
                let class = text(&class);
                let mut ids = Vec::new();
                for name in s.string_list(&class) {
                    ids.push(weapon_id(&name).ok_or_else(|| {
                        format!("[WeaponOrder] {class}: no weapon named \"{}\"", text(&name))
                    })?);
                }
                order.push((class, ids));
            }
            auto_switch = s.string_list("AutoSwitchWeapon").iter().filter_map(|n| weapon_id(n)).collect();
        }
        let mut perturb = PerturbGlobals::default();
        if let Some(s) = doc.section("Perturb") {
            for (key, field) in [
                ("MovementPerturbIncreaseSpeed", &mut perturb.movement_increase),
                ("MovementPerturbDecreaseSpeed", &mut perturb.movement_decrease),
                ("FiringPerturbIncreaseSpeed", &mut perturb.firing_increase),
                ("FiringPerturbDecreaseSpeed", &mut perturb.firing_decrease),
                ("PerturbRotationEffect", &mut perturb.rotation_effect),
            ] {
                *field = s.number(key, *field);
            }
        }
        Ok(Self {
            gear,
            ammo,
            weapons,
            order,
            auto_switch,
            perturb,
            projectiles: Vec::new(),
            impacts: Vec::new(),
            fires: Vec::new(),
        })
    }

    /// Every class's weapon IDs, one list in class order.
    pub fn order_flat(&self) -> Vec<usize> {
        self.order.iter().flat_map(|(_, ids)| ids.iter().copied()).collect()
    }

    /// A weapon's inventory class: the 1-based place of the order list it
    /// is in (`Class0` is 1, primary; ... 6, mission), 0 in none. Not its
    /// `Guntype`: the slot keys and the HUD's weapon panel read this.
    pub fn inventory_class(&self, weapon: usize) -> u8 {
        self.order
            .iter()
            .position(|(_, ids)| ids.contains(&weapon))
            .map_or(0, |i| (i + 1).min(u8::MAX as usize) as u8)
    }

    /// A weapon's command id: its place in all order lists in a row (the
    /// server's "better weapon" rule, inventory.md 11); None in no list.
    pub fn command_id(&self, weapon: usize) -> Option<usize> {
        self.order_flat().iter().position(|&w| w == weapon)
    }

    /// A weapon by name (the first match).
    pub fn weapon(&self, name: &str) -> Option<&WeaponRecord> {
        find_by_name(&self.weapons, name, |w| &w.name).map(|(_, w)| w)
    }

    /// A weapon's default ammo record.
    pub fn default_ammo(&self, weapon: &WeaponRecord) -> Option<&AmmoRecord> {
        weapon.default_ammo().map(|id| &self.ammo[id])
    }

    /// The projectile record `ammo` fires (its `ProjectileFXName`, the
    /// first record of that name).
    pub fn projectile(&self, ammo: &AmmoRecord) -> Option<&ProjectileRecord> {
        find_by_name(&self.projectiles, &ammo.projectile_fx, |p| &p.name).map(|(_, p)| p)
    }

    /// The fire record `ammo` names (its `FireFXName`, the first record
    /// of that name).
    pub fn fire(&self, ammo: &AmmoRecord) -> Option<&FireRecord> {
        let name = ammo.fire_fx.as_deref().filter(|n| !n.is_empty())?;
        find_by_name(&self.fires, name, |p| &p.name).map(|(_, p)| p)
    }

    /// The impact record `ammo` names (its `ImpactFXName`, the first
    /// record of that name; a present but empty name is `BulletFmj`).
    pub fn impact(&self, ammo: &AmmoRecord) -> Option<&ImpactRecord> {
        let name = ammo.impact_fx.as_deref()?;
        let name = if name.is_empty() { "BulletFmj" } else { name };
        find_by_name(&self.impacts, name, |p| &p.name).map(|(_, p)| p)
    }
}

/// `FireFX<N>` in the effect file (effects_sounds.md, "Effect tables in
/// the attribute data"): the group the shooter alone sees at the muzzle
/// and the casings.
#[derive(Clone, Debug, PartialEq)]
pub struct FireRecord {
    pub name: String,
    pub fx_name: String,
    pub eject_shells: bool,
    pub shell_model: String,
    pub shell_skin: String,
    pub shell_scale: f64,
    pub velocity: [f64; 3],
}

impl FireRecord {
    pub fn read(s: &Section) -> Self {
        Self {
            name: name_of(s),
            fx_name: s.string("FXName").map(text).unwrap_or_default(),
            eject_shells: s.number("EjectShells", 0.0) != 0.0,
            shell_model: s.string("ShellModel").map(text).unwrap_or_default(),
            shell_skin: s.string("ShellSkin").map(text).unwrap_or_default(),
            shell_scale: s.number("ShellScale", 1.0),
            velocity: s.vector("Velocity").unwrap_or_default(),
        }
    }

    /// Every `FireFX#` record of the effect file.
    pub fn read_all(doc: &Document) -> Vec<Self> {
        doc.records("FireFX").into_iter().map(Self::read).collect()
    }
}

/// `ImpactFX<N>` in the effect file (effects_sounds.md, "Effect tables in
/// the attribute data"): what an impact or a blast plays.
#[derive(Clone, Debug, PartialEq)]
pub struct ImpactRecord {
    pub name: String,
    /// `Sound`: a file, or `Surface` (by surface and ammo kind).
    pub sound: String,
    pub sound_radius: f64,
    pub create_mark: bool,
    /// `Mark`: `surface` (the surface's bullet hole) or a sprite; empty:
    /// none.
    pub mark: String,
    pub mark_scale: f64,
    pub mark_color_ratio: Option<f64>,
    /// `DoSurfaceFX`.
    pub do_surface_fx: bool,
    /// `FXName`: a group always played at the impact.
    pub fx_name: String,
}

impl ImpactRecord {
    pub fn read(s: &Section) -> Self {
        Self {
            name: name_of(s),
            sound: s.string("Sound").map(text).unwrap_or_default(),
            sound_radius: s.number("SoundRadius", 0.0),
            create_mark: s.int("CreateMark", 0) != 0,
            mark: s.string("Mark").map(text).unwrap_or_default(),
            mark_scale: s.number("MarkScale", 0.0),
            mark_color_ratio: s.number_opt("MarkColorRatio"),
            do_surface_fx: s.int("DoSurfaceFX", 0) != 0,
            fx_name: s.string("FXName").map(text).unwrap_or_default(),
        }
    }

    /// Every `ImpactFX#` record of the effect file.
    pub fn read_all(doc: &Document) -> Vec<Self> {
        doc.records("ImpactFX").into_iter().map(Self::read).collect()
    }
}

/// `ProjectileFX<N>` in the effect file (special_weapons.md, "Projectile
/// record"; grenades.md "Data"): the fields a thrown grenade reads. A
/// field the record lacks is `None` (the spec's data or ours stands).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileRecord {
    pub name: String,
    pub class: String,
    /// `Velocity` (u/s), `Lifetime` (s), `DropVelocity` (u/s).
    pub velocity: Option<f64>,
    pub lifetime: Option<f64>,
    pub drop_velocity: Option<f64>,
    /// `FireOffset`, u.
    pub fire_offset: Option<f64>,
    /// `CanHitSameKind`.
    pub can_hit_same_kind: Option<bool>,
    /// `Gravity` 1: the object gravity applies in flight.
    pub gravity: Option<bool>,
    /// `Model`: the world model it is drawn with; `ModelScale` (default
    /// 1 per axis); `Skin0..`.
    pub model: String,
    pub model_scale: [f64; 3],
    pub skins: Vec<String>,
    /// Its looks and sounds in flight (special_weapons.md Follow-up
    /// (2026-10-30) 1-3).
    pub look: ProjectileLookRecord,
}

/// A projectile record's looks and sounds in flight (special_weapons.md
/// Follow-up (2026-10-30) 1-3): the `FXName` group with `FXLoop` and
/// `FXSmoothShutdown`, the `FlySound` (a number reads as none) with
/// `SoundRadius`, the flare (`Flare` 1 with `FlareSprite`, `FlareScale`
/// in its two screen axes, `FlareYOffset`) and the smoke trail
/// (`SmokeTrail` 1, `SmokeTrailType`). `Light*` is never made, so not
/// read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectileLookRecord {
    pub fx: String,
    pub fx_loop: bool,
    pub fx_smooth: bool,
    pub fly_sound: String,
    pub sound_radius: f64,
    pub flare: Option<(String, [f64; 2], f64)>,
    pub smoke_trail: Option<i32>,
}

impl ProjectileLookRecord {
    pub fn read(s: &Section) -> Self {
        let on = |k: &str| s.number_opt(k).is_some_and(|v| v != 0.0);
        let string = |k: &str| s.string(k).map(text).unwrap_or_default();
        let flare_scale = match s.vector("FlareScale") {
            Some([x, y, _]) => [x, y],
            None => {
                let v = s.number("FlareScale", 1.0);
                [v, v]
            }
        };
        let sprite = string("FlareSprite");
        Self {
            fx: string("FXName"),
            fx_loop: on("FXLoop"),
            fx_smooth: on("FXSmoothShutdown"),
            fly_sound: string("FlySound"),
            sound_radius: s.number("SoundRadius", 0.0),
            flare: (on("Flare") && !sprite.is_empty()).then(|| (sprite, flare_scale, s.number("FlareYOffset", 0.0))),
            smoke_trail: on("SmokeTrail").then(|| s.int("SmokeTrailType", 1)),
        }
    }
}

impl ProjectileRecord {
    pub fn read(s: &Section) -> Self {
        Self {
            name: name_of(s),
            class: s.string("Class").map(text).unwrap_or_default(),
            velocity: s.number_opt("Velocity"),
            lifetime: s.number_opt("Lifetime"),
            drop_velocity: s.number_opt("DropVelocity"),
            fire_offset: s.number_opt("FireOffset"),
            can_hit_same_kind: s.number_opt("CanHitSameKind").map(|v| v != 0.0),
            gravity: s.number_opt("Gravity").map(|v| v != 0.0),
            model: s.string("Model").map(text).unwrap_or_default(),
            model_scale: s.vector("ModelScale").unwrap_or([1.0; 3]),
            skins: s.string_list("Skin").iter().map(|b| text(b)).collect(),
            look: ProjectileLookRecord::read(s),
        }
    }

    /// Every `ProjectileFX#` record of the effect file.
    pub fn read_all(doc: &Document) -> Vec<Self> {
        doc.records("ProjectileFX").into_iter().map(Self::read).collect()
    }
}

/// `[Player]` in the server attributes file. A missing or wrongly typed
/// speed reads as the smallest positive double, 0 as a float.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerPlayer {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub jump_speed: f32,
    pub ladder_speed: f32,
    pub walk_footstep_volume: f32,
    pub run_footstep_volume: f32,
    pub crouch_footstep_volume: f32,
    pub default_weapon: String,
}

impl ServerPlayer {
    pub fn read(doc: &Document) -> Option<Self> {
        let s = doc.section("Player")?;
        let n = |k: &str| s.number(k, f64::MIN_POSITIVE) as f32;
        Some(Self {
            walk_speed: n("WalkSpeed"),
            run_speed: n("RunSpeed"),
            jump_speed: n("JumpSpeed"),
            ladder_speed: n("LadderSpeed"),
            walk_footstep_volume: n("WalkFootstepVolume"),
            run_footstep_volume: n("RunFootstepVolume"),
            crouch_footstep_volume: n("CrouchFootstepVolume"),
            default_weapon: s.fixed_string("DefaultWeapon", ""),
        })
    }
}

/// The server's `[TimeWeight]` settings used by firing (attribute_data.md
/// Corrections 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerTimeWeight {
    /// `ReloadCheckTime` (built-in 0.8): the server's reload window is the
    /// third-person reload length / (rate x (2 - this)).
    pub reload_check_time: f64,
}

impl ServerTimeWeight {
    pub const BUILT_IN: f64 = 0.8;

    /// The section's values (no section: the built-in ones).
    pub fn read(doc: &Document) -> Self {
        Self {
            reload_check_time: doc
                .section("TimeWeight")
                .map_or(Self::BUILT_IN, |s| s.number("ReloadCheckTime", Self::BUILT_IN)),
        }
    }
}

/// A skeleton record's hit nodes (`Node<i>Name`, `Node<i>Radius`,
/// `Node<i>Location`, `Node<i>DamageFactor`, `Node<i>Priority`) from
/// node 0 until the first index without a name (attribute_data.md
/// Corrections 1: a missing priority reads as 0).
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonNode {
    pub name: String,
    pub radius: f64,
    pub location: i32,
    pub damage_factor: f64,
    pub priority: f64,
}

impl SkeletonNode {
    pub fn read_all(s: &Section) -> Vec<Self> {
        (0..)
            .map_while(|i| {
                let k = |f: &str| format!("Node{i}{f}");
                let name = text(s.string(&k("Name"))?);
                Some(Self {
                    name,
                    radius: s.number(&k("Radius"), 0.0),
                    location: s.int(&k("Location"), 0),
                    damage_factor: s.number(&k("DamageFactor"), 0.0),
                    priority: s.number(&k("Priority"), 0.0),
                })
            })
            .collect()
    }
}

/// `FigureType<N>`: a body type.

#[derive(Clone, Debug, PartialEq)]
pub struct FigureTypeRecord {
    pub name: String,
    pub female: bool,
    pub dash_speed: f64,
    pub max_stamina: i32,
    pub dash_gauge: i32,
    pub gauge_rec: i32,
    pub variable_jump: f64,
    pub variable_speed: f64,
    pub variable_damage: f64,
    pub variable_weight: f64,
    /// Walk, Run, Jump, Fire, FireDuck, FireMoveDuck, FireMove perturbs and
    /// Recoil.
    pub perturb: AmmoPerturb,
    pub attacked_push: f64,
    pub stiff_duration: f64,
    /// HP, Speed, Jump, Shooting, Damagedown, WeaponFenalty (truncated).
    pub stats: [i32; 6],
    pub hp_inc_percent: f64,
}

impl FigureTypeRecord {
    pub fn read(s: &Section) -> Self {
        let n = |k: &str| s.number(k, 0.0);
        let p = AmmoPerturb::read(s, "");
        Self {
            name: name_of(s),
            female: s.fixed_string("Sex", "Female").eq_ignore_ascii_case("female"),
            dash_speed: s.number("DashSpeed", 1.4),
            max_stamina: s.int("Maxstamina", NO_INT),
            dash_gauge: s.int("DashGauge", NO_INT),
            gauge_rec: s.int("GaugeRec", NO_INT),
            variable_jump: n("VariableJump"),
            variable_speed: n("VariableSpeed"),
            variable_damage: n("VariableDamage"),
            variable_weight: n("VariableWeight"),
            perturb: AmmoPerturb {
                recoil: n("Recoil"),
                ..p
            },
            attacked_push: n("AttackedPush"),
            stiff_duration: n("StiffDuration"),
            stats: ["HP", "Speed", "Jump", "Shooting", "Damagedown", "WeaponFenalty"].map(|k| n(k) as i32),
            hp_inc_percent: n("HPIncPercent"),
        }
    }
}

/// The movement keys of a `Model<N>` (character) or `Hair<N>` (head
/// piece).
#[derive(Clone, Debug, PartialEq)]
pub struct WearRecord {
    pub name: String,
    /// `FigureType` (characters only; 0 for head pieces).
    pub figure_type: i32,
    pub move_penalty: f64,
    pub dash_speed: f64,
    pub max_stamina: f64,
}

impl WearRecord {
    pub fn read(s: &Section) -> Self {
        Self {
            name: name_of(s),
            figure_type: s.int("FigureType", 0),
            move_penalty: s.number("MovePenalty", 0.0),
            dash_speed: s.number("DashSpeed", 0.0),
            max_stamina: s.number("MaxStamina", 0.0),
        }
    }
}

/// A model piece's files (characters.md section 2): a head piece
/// (`Hair<N>`: `ModelFile0`, `ModelFile1` by skeleton) or a face
/// (`Face<N>`: `ModelFile`), its skins (`Skin0..`, by camouflage index for
/// a head piece) and, for a face, the head piece it comes with
/// (`HairIndex`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PieceRecord {
    pub name: String,
    pub files: Vec<String>,
    pub skins: Vec<String>,
    pub hair: Option<i32>,
    /// `RenderStyle0..`.
    pub render_styles: Vec<String>,
    /// `ClientFX0..`: effect groups the piece plays looping on the
    /// character (game_modes.md Corrections (2026-10-30) 4, 7: faces'
    /// eyes).
    pub client_fx: Vec<String>,
}

impl PieceRecord {
    pub fn read(s: &Section) -> Self {
        let list = |prefix: &str| -> Vec<String> {
            (0..)
                .map_while(|i| s.string(&format!("{prefix}{i}")).map(text))
                .collect()
        };
        let mut files = list("ModelFile");
        if files.is_empty()
            && let Some(f) = s.string("ModelFile")
        {
            files.push(text(f));
        }
        Self {
            name: name_of(s),
            files,
            skins: list("Skin"),
            hair: s.get("HairIndex").map(|_| s.int("HairIndex", -1)).filter(|&h| h >= 0),
            render_styles: list("RenderStyle"),
            client_fx: list("ClientFX"),
        }
    }
}

/// A worn socket attachment (characters.md 1 and 2, attachments.md
/// "Character attachments"): goggles or a mask (`Goggle<N>`) or a
/// backpack (`Backpack<N>`): its name, socket (`Socket`), file per figure
/// type (`Model0`, `Model1`), skins (`Skin0..`) and render styles
/// (`RenderStyle0..`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WornRecord {
    pub name: String,
    pub socket: String,
    pub files: Vec<String>,
    pub skins: Vec<String>,
    pub render_styles: Vec<String>,
}

impl WornRecord {
    pub fn read(s: &Section) -> Self {
        let list = |prefix: &str| -> Vec<String> {
            (0..)
                .map_while(|i| s.string(&format!("{prefix}{i}")).map(text))
                .collect()
        };
        Self {
            name: name_of(s),
            socket: s.string("Socket").map(text).unwrap_or_default(),
            files: list("Model"),
            skins: list("Skin"),
            render_styles: list("RenderStyle"),
        }
    }
}

/// The character attachments file's goggles and backpacks, by record
/// index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttachmentsFile {
    pub goggles: Vec<WornRecord>,
    pub backpacks: Vec<WornRecord>,
}

impl AttachmentsFile {
    pub fn read(doc: &Document) -> Self {
        Self {
            goggles: doc.records("Goggle").into_iter().map(WornRecord::read).collect(),
            backpacks: doc.records("Backpack").into_iter().map(WornRecord::read).collect(),
        }
    }
}

/// The team marks (characters.md 2, `[TeamMark]`): mark i's texture
/// (`TeamName#`) and render style (`RenderStyle#`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarksFile {
    pub textures: Vec<String>,
    pub render_styles: Vec<String>,
}

impl MarksFile {
    pub fn read(doc: &Document) -> Option<Self> {
        let s = doc.section("TeamMark")?;
        let list = |prefix: &str| -> Vec<String> { s.string_list(prefix).iter().map(|v| text(v)).collect() };
        Some(Self {
            textures: list("TeamName"),
            render_styles: list("RenderStyle"),
        })
    }
}

/// A body record's look (`Model<N>`, characters.md 1 and 2): its file
/// (`ModelFile`), skins by camouflage index (`UpperSkin0..`,
/// `LowerSkin0..`), additional skins (`AdditionalSkins0..`: texture
/// slots 5 on), render styles (`RenderStyle0..`), face redirects
/// (`FaceRelationModel0..`, -1 none), skeleton (`Skeleton`) and gear
/// (`GearID`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodyLook {
    pub file: String,
    pub upper_skins: Vec<String>,
    pub lower_skins: Vec<String>,
    /// `AdditionalSkins0..`: texture slot 5 + k (characters.md 2), e.g.
    /// the ghillie suits' leaves.
    pub additional_skins: Vec<String>,
    pub render_styles: Vec<String>,
    pub faces: Vec<i32>,
    /// `FootprintSndVolume`: a share added to the footstep volume.
    pub footprint_volume: f32,
    /// `Skeleton`: the hit table and skeleton index (0 male, 1 female in
    /// the player set).
    pub skeleton: i32,
    /// `GearID`.
    pub gear: i32,
    /// `InfectedOveridable`: its infection override (`Overidable_Model#`
    /// index; -1 none).
    pub infected: i32,
    /// `ColorSkin{m}_UpperSkin#`, `_LowerSkin#`, `_AdditionalSkins#` and
    /// `_HandsSkin#` for model colours m = 0..4 (characters.md Corrections
    /// (2026-10-30) 1).
    pub colour_skins: Vec<ColourSkins>,
}

/// One model colour's skin lists (empty: the base list stays).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColourSkins {
    pub upper: Vec<String>,
    pub lower: Vec<String>,
    pub additional: Vec<String>,
    pub hands: Vec<String>,
}

/// At most this many model colour lists (0..4).
pub const MODEL_COLOURS: usize = 5;

/// `Overidable_Model<N>`: an infection override's look (characters.md 2:
/// it replaces render styles, armour, hit points and hands; its face).
#[derive(Clone, Debug, PartialEq)]
pub struct InfectionLook {
    pub name: String,
    /// `FaceIndex` (-1: none).
    pub face: i32,
    /// `HandIndex` (-1: none).
    pub hand: i32,
    pub render_styles: Vec<String>,
}

impl InfectionLook {
    pub fn read(s: &Section) -> Self {
        Self {
            name: name_of(s),
            face: s.int("FaceIndex", -1),
            hand: s.int("HandIndex", -1),
            render_styles: s.string_list("RenderStyle").iter().map(|v| text(v)).collect(),
        }
    }
}

impl BodyLook {
    pub fn read(s: &Section) -> Self {
        let list = |prefix: &str| -> Vec<String> { s.string_list(prefix).iter().map(|v| text(v)).collect() };
        Self {
            file: s.string("ModelFile").map(text).unwrap_or_default(),
            upper_skins: list("UpperSkin"),
            lower_skins: list("LowerSkin"),
            additional_skins: list("AdditionalSkins"),
            render_styles: list("RenderStyle"),
            faces: s.int_list("FaceRelationModel"),
            footprint_volume: s.number("FootprintSndVolume", 0.0) as f32,
            skeleton: s.int("Skeleton", 0),
            gear: s.int("GearID", -1),
            infected: s.int("InfectedOveridable", -1),
            colour_skins: (0..MODEL_COLOURS)
                .map(|m| ColourSkins {
                    upper: list(&format!("ColorSkin{m}_UpperSkin")),
                    lower: list(&format!("ColorSkin{m}_LowerSkin")),
                    additional: list(&format!("ColorSkin{m}_AdditionalSkins")),
                    hands: list(&format!("ColorSkin{m}_HandsSkin")),
                })
                .collect(),
        }
    }
}

/// The models file's body types, characters, head pieces and faces.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelsFile {
    pub figure_types: Vec<FigureTypeRecord>,
    pub models: Vec<WearRecord>,
    pub hair: Vec<WearRecord>,
    /// `Hair<N>`'s and `Face<N>`'s files, by record index.
    pub hair_pieces: Vec<PieceRecord>,
    pub faces: Vec<PieceRecord>,
    /// `Model<N>`'s looks, by record index.
    pub bodies: Vec<BodyLook>,
    /// `Overidable_Model<N>`, by record index.
    pub overrides: Vec<InfectionLook>,
}

impl ModelsFile {
    pub fn read(doc: &Document) -> Self {
        Self {
            figure_types: doc.records("FigureType").into_iter().map(FigureTypeRecord::read).collect(),
            models: doc.records("Model").into_iter().map(WearRecord::read).collect(),
            hair: doc.records("Hair").into_iter().map(WearRecord::read).collect(),
            hair_pieces: doc.records("Hair").into_iter().map(PieceRecord::read).collect(),
            faces: doc.records("Face").into_iter().map(PieceRecord::read).collect(),
            bodies: doc.records("Model").into_iter().map(BodyLook::read).collect(),
            overrides: doc.records("Overidable_Model").into_iter().map(InfectionLook::read).collect(),
        }
    }
}


// ---------------------------------------------------------------------------
// The modification file (specs/combat_arms/attachments.md, "How
// attachments are described in the data")

/// A special function's kind or target written as a name (any case); a
/// number or an unknown name is `None` (the spec doesn't say how numbers
/// map).
fn named<T: Copy>(s: &Section, key: &str, names: &[(&str, T)]) -> Option<T> {
    let v = text(s.string(key)?);
    names.iter().find(|(n, _)| n.eq_ignore_ascii_case(&v)).map(|(_, t)| *t)
}

/// `Mod<N>`: an attachment record, as our data's (`ModRecord`); fields
/// missing or of another type are 0 (empty).
pub fn read_mod(s: &Section) -> ModRecord {
    use crate::game::attachments::{Special, SpecialKind as K, Target as T};
    let n = |k: &str| s.number(k, 0.0) as f32;
    let i = |k: &str| s.int(k, 0);
    let opt_int = |k: &str| match s.get(k) {
        Some(lithtech_formats::attributes::Value::Int(v)) => Some(*v),
        _ => None,
    };
    let column = |k: &str| (0..10).map(|c| s.int(&format!("{k}{c}"), 0)).collect::<Vec<_>>();
    let kinds = [("InstDamage", K::InstDamage), ("Stun", K::Stun), ("Knockback", K::Knockback), ("Crit", K::Crit)];
    let targets = [
        ("All", T::All),
        ("Player", T::Player),
        ("Npc", T::Npc),
        ("Human", T::Human),
        ("Zombie", T::Zombie),
        ("PH", T::PH),
        ("PZ", T::PZ),
        ("NH", T::NH),
        ("NZ", T::NZ),
    ];
    let mut special = Vec::new();
    for j in 0.. {
        let ty = format!("SFType{j}");
        if s.get(&ty).is_none() {
            break;
        }
        let Some(kind) = named(s, &ty, &kinds) else { continue };
        let target_key = format!("SFTargetType{j}");
        let target = if s.get(&target_key).is_none() {
            T::All
        } else {
            match named(s, &target_key, &targets) {
                Some(t) => t,
                None => continue,
            }
        };
        special.push(Special {
            kind,
            target,
            value: n(&format!("SFValue{j}")),
            duration: n(&format!("SFDuration{j}")),
            probability: s.number(&format!("SFProbability{j}"), 1.0) as f32,
        });
    }
    let fov: Vec<f32> = (0..10).map_while(|l| s.number_opt(&format!("ZoomFov{l}")).map(|v| v as f32)).collect();
    let socket = s.fixed_string("Socket", "");
    let hh_socket = s.fixed_string("HHModelSocket", "");
    // Per variant c: `<model>c` and its skins `<skin>c_0`, `<skin>c_1`, ...
    let models = |k: &str| -> Vec<String> {
        let list: Vec<String> = (0..10).map(|c| s.string(&format!("{k}{c}")).map(text).unwrap_or_default()).collect();
        let used = list.iter().rposition(|m| !m.is_empty()).map_or(0, |i| i + 1);
        list[..used].to_vec()
    };
    let skins = |k: &str, n: usize| -> Vec<Vec<String>> {
        (0..n)
            .map(|c| (0..).map_while(|i| s.string(&format!("{k}{c}_{i}")).map(text)).collect())
            .collect()
    };
    let scale = |k: &str| s.vector(k).map(|v| v.map(|x| x as f32));
    let attach_models = models("AttachModel");
    let attach_hh_models = models("AttachHHModel");
    ModRecord {
        attach_skins: skins("AttachSkin", attach_models.len()),
        attach_hh_skins: skins("AttachHHSkin", attach_hh_models.len()),
        attach_models,
        attach_hh_models,
        attach_render_styles: s.string_list("AttachRenderStyle").iter().map(|v| text(v)).collect(),
        hh_render_styles: s.string_list("HHRenderStyle").iter().map(|v| text(v)).collect(),
        attach_scale: scale("AttachScale"),
        attach_hh_scale: scale("AttachHHModelScale"),
        hh_socket: (!hh_socket.is_empty()).then_some(hh_socket),
        name: name_of(s),
        kinds: opt_int("Kinds"),
        kind: opt_int("Type"),
        integrated: i("Integrated"),
        socket: (!socket.is_empty()).then_some(socket),
        silencer_level: i("SilencerLevel"),
        flash_hider_level: i("FlashHiderLevel"),
        zoom_level: i("ZoomLevel"),
        zoom_fov: fov,
        zoom_sprite: s.fixed_string("ZoomSpriteName", ""),
        zoom_sprite_scale: s.number("ZoomSpriteScale", 0.0) as f32,
        sight_style: i("SightSystemStyle"),
        target_range: i("TargetRange"),
        inst_damage: n("Instdamage"),
        inst_damage_per: n("InstDamagePer"),
        range_per: n("RangePer"),
        fire_ani_rate: n("Fireanirate"),
        reload_ani_rate: n("Reloadanirate"),
        select_ani_rate: n("Selectanirate"),
        perturb_inc_speed: n("PerturbIncSpeed"),
        perturb_dec_speed: n("PerturbDecSpeed"),
        zoomed_perturb_inc_speed: n("ZoomedPerturbIncSpeed"),
        zoomed_perturb_dec_speed: n("ZoomedPerturbDecSpeed"),
        walk_perturb: n("WalkPerturb"),
        run_perturb: n("RunPerturb"),
        fire_perturb: n("FirePerturb"),
        fire_duck_perturb: n("FireDuckPerturb"),
        zoomed_min_perturb: n("ZoomedMinPerturb"),
        zoomed_max_perturb: n("ZoomedMaxPerturb"),
        dynamic_perturb_factor: n("DynamicPerturbFactor"),
        min_perturb_per: n("MinPerturbPer"),
        base_cam_recoil_pitch: n("BaseCamRecoilPitch"),
        base_cam_recoil_pitch_per: n("BaseCamRecoilPitchPer"),
        max_cam_recoil_pitch_per: n("MaxCamRecoilPitchPer"),
        base_cam_recoil_accel_pitch: n("BaseCamRecoilAccelPitch"),
        max_random_pitch: n("MaxRandomPitch"),
        camera_sway: [
            n("CameraSwayXSpeed"),
            n("CameraSwayYSpeed"),
            n("CameraSwayXFreq"),
            n("CameraSwayYFreq"),
        ],
        vectors_per_round_per: n("VectorsPerRoundPer"),
        shots_per_clip_plus: column("ShotsPerClipPlus"),
        max_amount_plus: column("MaxAmountPlus"),
        selection_amount_plus: column("SelectionAmountPlus"),
        special,
    }
}

/// `Weapon<N>` of the modification file: the accepted list of the weapon
/// with the same N in the weapons file (Q13: by position).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModWeaponRecord {
    /// `Name` (informational).
    pub name: String,
    /// `ModName#` with its `ModModel#` (default 0); a name that matches no
    /// attachment is dropped with its index.
    pub accepts: Vec<(String, i32)>,
    /// `MuzzleOPtion`, `SightOption`, `MagazineOption`, `BulletOption`,
    /// `MechSightOption` (default -1): the lobby's per-slot state.
    pub options: [i32; 5],
}

/// The per-slot option keys of a `Weapon#` record, slot order.
pub const SLOT_OPTIONS: [&str; 5] = ["MuzzleOPtion", "SightOption", "MagazineOption", "BulletOption", "MechSightOption"];

/// The modification file (`MOD.TXT`): attachment records and per-weapon
/// accepted lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModFile {
    pub mods: Vec<ModRecord>,
    pub weapons: Vec<ModWeaponRecord>,
}

impl ModFile {
    pub fn read(doc: &Document) -> Self {
        let mods: Vec<ModRecord> = doc.records("Mod").into_iter().map(read_mod).collect();
        let weapons = doc
            .records("Weapon")
            .into_iter()
            .map(|s| {
                let accepts = s
                    .string_list("ModName")
                    .iter()
                    .enumerate()
                    .filter_map(|(k, name)| {
                        let (_, m) = find_by_name(&mods, &text(name), |m| &m.name)?;
                        Some((m.name.clone(), s.int(&format!("ModModel{k}"), 0)))
                    })
                    .collect();
                ModWeaponRecord {
                    name: name_of(s),
                    accepts,
                    options: SLOT_OPTIONS.map(|k| s.int(k, -1)),
                }
            })
            .collect();
        Self { mods, weapons }
    }
}
