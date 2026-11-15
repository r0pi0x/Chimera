//! Tag class codes (specs/halo_ce/cache_file.md, "Bytes" and "Tag
//! classes"): `ClassCode`, a four-character code stored as a u32 whose
//! big-endian spelling is the code, and the disc's classes with their
//! names and parents as a table (`CLASSES`, `class_info`), with a constant
//! per class named after it (`WEAPON` is `weap`).

use std::fmt;

/// A tag class code: four ASCII characters (`weap`, `snd!`, `sky `,
/// `DeLa`), case-sensitive. Built from a literal (`ClassCode::new(b"weap")`,
/// usable in `const`), compared as a u32, printed as its four characters.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassCode(u32);

/// The stored value of an absent class (a parent slot of a class without
/// one).
pub const NONE_CLASS: u32 = 0xFFFF_FFFF;

impl ClassCode {
    /// The code spelled by `code` (`b"weap"`).
    pub const fn new(code: &[u8; 4]) -> Self {
        ClassCode(u32::from_be_bytes(*code))
    }

    /// The code spelled by a four-character string (`"weap"`); panics
    /// (at compile time in a `const`) on another length.
    pub const fn from_text(code: &str) -> Self {
        let b = code.as_bytes();
        assert!(b.len() == 4, "a class code is four characters");
        ClassCode(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// The code from its stored u32 (as a little-endian field reads it).
    pub const fn from_u32(v: u32) -> Self {
        ClassCode(v)
    }

    /// A stored parent slot: `None` for NONE (all bits set).
    pub const fn from_slot(v: u32) -> Option<Self> {
        if v == NONE_CLASS { None } else { Some(ClassCode(v)) }
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// The four characters, in reading order.
    pub const fn bytes(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }

    /// The disc's table row for this code, if it is one of the disc's
    /// classes.
    pub fn info(self) -> Option<&'static ClassInfo> {
        class_info(self)
    }
}

impl fmt::Display for ClassCode {
    /// The four characters; a byte outside printable ASCII as `\xNN`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.bytes() {
            if (0x20..0x7F).contains(&b) {
                write!(f, "{}", b as char)?;
            } else {
                write!(f, "\\x{b:02x}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for ClassCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClassCode(\"{self}\")")
    }
}

/// One row of the spec's class table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassInfo {
    pub code: ClassCode,
    /// The tag group's name (`weapon`, `shader_transparent_generic`).
    pub name: &'static str,
    pub parent: Option<ClassCode>,
    pub grandparent: Option<ClassCode>,
    /// Appears only as a parent on the disc, never as a tag's own class
    /// (`obje`, `unit`, `item`, `devi`, `shdr`).
    pub abstract_only: bool,
}

macro_rules! classes {
    ($( $konst:ident $code:literal $name:literal $($p:ident)? $(> $g:ident)? $(; $abs:ident)? ,)*) => {
        $(
            #[doc = concat!("`", $code, "`: ", $name, ".")]
            pub const $konst: ClassCode = ClassCode::from_text($code);
        )*
        /// Every class code in the tag index of any of the disc's 24 maps,
        /// with the parents the index stores for it (the spec's "Classes
        /// on the disc").
        pub const CLASSES: &[ClassInfo] = &[
            $(ClassInfo {
                code: $konst,
                name: $name,
                parent: classes!(@opt $($p)?),
                grandparent: classes!(@opt $($g)?),
                abstract_only: classes!(@abs $($abs)?),
            },)*
        ];
    };
    (@opt) => { None };
    (@opt $p:ident) => { Some($p) };
    (@abs) => { false };
    (@abs $a:ident) => { true };
}

classes! {
    OBJECT "obje" "object" ; abstract_,
    UNIT "unit" "unit" OBJECT ; abstract_,
    ITEM "item" "item" OBJECT ; abstract_,
    DEVICE "devi" "device" OBJECT ; abstract_,
    SHADER "shdr" "shader" ; abstract_,
    BIPED "bipd" "biped" UNIT > OBJECT,
    VEHICLE "vehi" "vehicle" UNIT > OBJECT,
    WEAPON "weap" "weapon" ITEM > OBJECT,
    EQUIPMENT "eqip" "equipment" ITEM > OBJECT,
    GARBAGE "garb" "garbage" ITEM > OBJECT,
    DEVICE_MACHINE "mach" "device_machine" DEVICE > OBJECT,
    DEVICE_CONTROL "ctrl" "device_control" DEVICE > OBJECT,
    DEVICE_LIGHT_FIXTURE "lifi" "device_light_fixture" DEVICE > OBJECT,
    PROJECTILE "proj" "projectile" OBJECT,
    SCENERY "scen" "scenery" OBJECT,
    SOUND_SCENERY "ssce" "sound_scenery" OBJECT,
    SHADER_ENVIRONMENT "senv" "shader_environment" SHADER,
    SHADER_MODEL "soso" "shader_model" SHADER,
    SHADER_TRANSPARENT_GENERIC "sotr" "shader_transparent_generic" SHADER,
    SHADER_TRANSPARENT_CHICAGO "schi" "shader_transparent_chicago" SHADER,
    SHADER_TRANSPARENT_GLASS "sgla" "shader_transparent_glass" SHADER,
    SHADER_TRANSPARENT_METER "smet" "shader_transparent_meter" SHADER,
    SHADER_TRANSPARENT_PLASMA "spla" "shader_transparent_plasma" SHADER,
    SHADER_TRANSPARENT_WATER "swat" "shader_transparent_water" SHADER,
    SCENARIO "scnr" "scenario",
    SCENARIO_STRUCTURE_BSP "sbsp" "scenario_structure_bsp",
    SKY "sky " "sky",
    GLOBALS "matg" "globals",
    MODEL "mode" "model",
    MODEL_COLLISION_GEOMETRY "coll" "model_collision_geometry",
    MODEL_ANIMATIONS "antr" "model_animations",
    PHYSICS "phys" "physics",
    POINT_PHYSICS "pphy" "point_physics",
    BITMAP "bitm" "bitmap",
    SOUND "snd!" "sound",
    SOUND_LOOPING "lsnd" "sound_looping",
    SOUND_ENVIRONMENT "snde" "sound_environment",
    EFFECT "effe" "effect",
    DAMAGE_EFFECT "jpt!" "damage_effect",
    PARTICLE "part" "particle",
    PARTICLE_SYSTEM "pctl" "particle_system",
    CONTRAIL "cont" "contrail",
    LIGHT "ligh" "light",
    LENS_FLARE "lens" "lens_flare",
    GLOW "glw!" "glow",
    LIGHT_VOLUME "mgs2" "light_volume",
    DECAL "deca" "decal",
    ANTENNA "ant!" "antenna",
    FLAG "flag" "flag",
    FOG "fog " "fog",
    WIND "wind" "wind",
    MATERIAL_EFFECTS "foot" "material_effects",
    DETAIL_OBJECT_COLLECTION "dobc" "detail_object_collection",
    ITEM_COLLECTION "itmc" "item_collection",
    ACTOR "actr" "actor",
    ACTOR_VARIANT "actv" "actor_variant",
    DIALOGUE "udlg" "dialogue",
    CAMERA_TRACK "trak" "camera_track",
    COLOR_TABLE "colo" "color_table",
    FONT "font" "font",
    STRING_LIST "str#" "string_list",
    UNICODE_STRING_LIST "ustr" "unicode_string_list",
    HUD_MESSAGE_TEXT "hmt " "hud_message_text",
    HUD_GLOBALS "hudg" "hud_globals",
    HUD_NUMBER "hud#" "hud_number",
    GRENADE_HUD_INTERFACE "grhi" "grenade_hud_interface",
    UNIT_HUD_INTERFACE "unhi" "unit_hud_interface",
    WEAPON_HUD_INTERFACE "wphi" "weapon_hud_interface",
    METER "metr" "meter",
    UI_WIDGET_DEFINITION "DeLa" "ui_widget_definition",
    UI_WIDGET_COLLECTION "Soul" "ui_widget_collection",
    VIRTUAL_KEYBOARD "vcky" "virtual_keyboard",
    MULTIPLAYER_SCENARIO_DESCRIPTION "mply" "multiplayer_scenario_description",
}

/// Classes the spec lists as known to the game but on no disc map
/// (inferred, without parents). A map naming one is accepted. The disc
/// disagrees for two: `elec` (lightning) and `rain` are tags of the
/// campaign maps b30, b40, c10, c20, c40 and d40, with no parents (the
/// heavy test `all_24_maps` lists them).
pub const ABSENT_CLASSES: &[(ClassCode, &str)] = &[
    (ClassCode::new(b"cdmg"), "continuous_damage_effect"),
    (ClassCode::new(b"elec"), "lightning"),
    (ClassCode::new(b"rain"), "weather_particle_system"),
    (ClassCode::new(b"plac"), "placeholder"),
    (ClassCode::new(b"boom"), "spheroid"),
    (ClassCode::new(b"devc"), "input_device_defaults"),
    (ClassCode::new(b"ngpr"), "preferences_network_game"),
    (ClassCode::new(b"tagc"), "tag_collection"),
];

/// The disc's table row for `code`.
pub fn class_info(code: ClassCode) -> Option<&'static ClassInfo> {
    CLASSES.iter().find(|c| c.code == code)
}

/// Whether this port reads maps holding `code`: one of the disc's classes
/// or a known absent one. The PC releases' own classes (`mod2`, `scex`,
/// ...) are refused.
pub fn is_known(code: ClassCode) -> bool {
    class_info(code).is_some() || ABSENT_CLASSES.iter().any(|(c, _)| *c == code)
}
