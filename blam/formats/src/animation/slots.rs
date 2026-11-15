//! The animation graph's index tables as typed slots (specs/halo_ce/
//! animations.md, "Index tables"): one enum per table whose variants are
//! the slots in the spec's order (`UnitSlot`, `WeaponClassSlot`,
//! `WeaponTypeSlot`, `WeaponSlot`, `VehicleSlot`, `DeviceSlot`,
//! `FirstPersonSlot`, and `UnitDamageSlot` for the damage table's
//! type x direction x part layout), and `IndexTable<S>`, the stored table
//! whose `get(slot)` gives the animation index or `None`: a slot beyond
//! the stored count, or stored as NONE, is none. Game code asks for
//! `FirstPersonSlot::Ready`, never a bare number.

use std::marker::PhantomData;

/// A slot of an index table: its position in the table.
pub trait Slot: Copy {
    /// Position in the table (the spec's slot number).
    fn index(self) -> usize;
}

/// A table of animation indices (indices into the graph's animations
/// block), one per slot of `S`. The stored count may be shorter than the
/// slot enumeration.
#[derive(Debug)]
pub struct IndexTable<S> {
    entries: Vec<Option<usize>>,
    slot: PhantomData<fn() -> S>,
}

impl<S> Clone for IndexTable<S> {
    fn clone(&self) -> Self {
        IndexTable {
            entries: self.entries.clone(),
            slot: PhantomData,
        }
    }
}

impl<S> PartialEq for IndexTable<S> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<S> Default for IndexTable<S> {
    fn default() -> Self {
        Self::from_entries(Vec::new())
    }
}

impl<S> IndexTable<S> {
    /// A table of the stored entries (`None` = NONE).
    pub fn from_entries(entries: Vec<Option<usize>>) -> Self {
        IndexTable {
            entries,
            slot: PhantomData,
        }
    }

    /// The stored entries, in slot order.
    pub fn entries(&self) -> &[Option<usize>] {
        &self.entries
    }

    /// The stored count (may be less than the number of slots).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<S: Slot> IndexTable<S> {
    /// The animation index in `slot`, or `None` when the slot is empty or
    /// beyond the stored count.
    pub fn get(&self, slot: S) -> Option<usize> {
        self.entries.get(slot.index()).copied().flatten()
    }
}

/// Declares a slot enum: variants in table order with their spec labels.
macro_rules! slots {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $label:literal),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            /// Every slot, in table order.
            pub const ALL: &'static [$name] = &[$($name::$variant),*];
            /// The number of slots.
            pub const COUNT: usize = Self::ALL.len();

            /// The slot at `index` of the table.
            pub fn from_index(index: usize) -> Option<$name> {
                Self::ALL.get(index).copied()
            }

            /// The spec's name of the slot (`move-front`).
            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label),*
                }
            }
        }

        impl Slot for $name {
            fn index(self) -> usize {
                self as usize
            }
        }
    };
}

slots! {
    /// The unit seat's table (30 slots).
    UnitSlot {
        AirborneDead = "airborne-dead",
        LandingDead = "landing-dead",
        AccFrontBack = "acc-front-back",
        AccLeftRight = "acc-left-right",
        AccUpDown = "acc-up-down",
        Push = "push",
        Twist = "twist",
        Enter = "enter",
        Exit = "exit",
        Look = "look",
        Talk = "talk",
        Emotions = "emotions",
        Unused12 = "unused",
        User0 = "user0",
        User1 = "user1",
        User2 = "user2",
        User3 = "user3",
        User4 = "user4",
        User5 = "user5",
        User6 = "user6",
        User7 = "user7",
        User8 = "user8",
        User9 = "user9",
        FlyingFront = "flying-front",
        FlyingBack = "flying-back",
        FlyingLeft = "flying-left",
        FlyingRight = "flying-right",
        Opening = "opening",
        Closing = "closing",
        Hovering = "hovering",
    }
}

slots! {
    /// The weapon class's table (55 slots).
    WeaponClassSlot {
        Idle = "idle",
        Gesture = "gesture",
        TurnLeft = "turn-left",
        TurnRight = "turn-right",
        DiveFront = "dive-front",
        DiveBack = "dive-back",
        DiveLeft = "dive-left",
        DiveRight = "dive-right",
        MoveFront = "move-front",
        MoveBack = "move-back",
        MoveLeft = "move-left",
        MoveRight = "move-right",
        SlideFront = "slide-front",
        SlideBack = "slide-back",
        SlideLeft = "slide-left",
        SlideRight = "slide-right",
        Airborne = "airborne",
        LandSoft = "land-soft",
        LandHard = "land-hard",
        Unused19 = "unused",
        ThrowGrenade = "throw-grenade",
        Disarm = "disarm",
        Drop = "drop",
        Ready = "ready",
        PutAway = "put-away",
        AimStill = "aim-still",
        AimMove = "aim-move",
        SurpriseFront = "surprise-front",
        SurpriseBack = "surprise-back",
        Berserk = "berserk",
        EvadeLeft = "evade-left",
        EvadeRight = "evade-right",
        SignalMove = "signal-move",
        SignalAttack = "signal-attack",
        Warn = "warn",
        StunnedFront = "stunned-front",
        StunnedBack = "stunned-back",
        StunnedLeft = "stunned-left",
        StunnedRight = "stunned-right",
        Melee = "melee",
        Celebrate = "celebrate",
        Panic = "panic",
        MeleeAirborne = "melee-airborne",
        Flaming = "flaming",
        ResurrectFront = "resurrect-front",
        ResurrectBack = "resurrect-back",
        MeleeContinuous = "melee-continuous",
        Feeding = "feeding",
        LeapStart = "leap-start",
        LeapAirborne = "leap-airborne",
        LeapMelee = "leap-melee",
        Zapping = "zapping",
        Unused52 = "unused",
        Unused53 = "unused",
        Unused54 = "unused",
    }
}

slots! {
    /// The weapon type's table (10 slots).
    WeaponTypeSlot {
        Reload1 = "reload-1",
        Reload2 = "reload-2",
        Chamber1 = "chamber-1",
        Chamber2 = "chamber-2",
        Fire1 = "fire-1",
        Fire2 = "fire-2",
        Charged1 = "charged-1",
        Charged2 = "charged-2",
        Melee = "melee",
        Overheat = "overheat",
    }
}

slots! {
    /// A weapon's own table (11 slots, its third-person graph).
    WeaponSlot {
        Idle = "idle",
        Ready = "ready",
        PutAway = "put-away",
        Reload1 = "reload-1",
        Reload2 = "reload-2",
        Chamber1 = "chamber-1",
        Chamber2 = "chamber-2",
        Charged1 = "charged-1",
        Charged2 = "charged-2",
        Fire1 = "fire-1",
        Fire2 = "fire-2",
    }
}

slots! {
    /// A vehicle's table (8 slots, all overlays).
    VehicleSlot {
        Steering = "steering",
        Roll = "roll",
        Throttle = "throttle",
        Velocity = "velocity",
        Braking = "braking",
        GroundSpeed = "ground-speed",
        Occupied = "occupied",
        Unoccupied = "unoccupied",
    }
}

slots! {
    /// A device's table (2 slots, overlays).
    DeviceSlot {
        Position = "position",
        Power = "power",
    }
}

slots! {
    /// The first-person weapon table (28 slots). Slots 23 to 25 are the
    /// shotgun's one-shell reload; 26 and 27 the overheat exit and the
    /// overheat with supercharge. Note the crossed flashlight slots: the
    /// light turning on plays `LightOff`.
    FirstPersonSlot {
        Idle = "idle",
        Posing = "posing",
        Fire1 = "fire-1",
        Moving = "moving",
        Overlays = "overlays",
        LightOff = "light-off",
        LightOn = "light-on",
        ReloadEmpty = "reload-empty",
        ReloadFull = "reload-full",
        Overheated = "overheated",
        Ready = "ready",
        PutAway = "put-away",
        Overcharged = "overcharged",
        Melee = "melee",
        Fire2 = "fire-2",
        OverchargedJitter = "overcharged-jitter",
        ThrowGrenade = "throw-grenade",
        Ammunition = "ammunition",
        Misfire1 = "misfire-1",
        Misfire2 = "misfire-2",
        ThrowOverheated = "throw-overheated",
        Overheating = "overheating",
        OverheatingAgain = "overheating-again",
        Enter = "enter",
        ExitEmpty = "exit-empty",
        ExitFull = "exit-full",
        OverheatedExit = "o-h-exit",
        OverheatedSuperchargeEnter = "o-h-s-enter",
    }
}

slots! {
    /// The kind of a damage reaction (the damage table's first factor).
    DamageKind {
        SoftPing = "s-ping",
        HardPing = "h-ping",
        SoftKill = "s-kill",
        HardKill = "h-kill",
    }
}

slots! {
    /// The direction of a damage reaction.
    DamageDirection {
        Front = "front",
        Left = "left",
        Right = "right",
        Back = "back",
    }
}

slots! {
    /// The body part of a damage reaction.
    BodyPart {
        Gut = "gut",
        Chest = "chest",
        Head = "head",
        LeftArm = "l-arm",
        LeftHand = "l-hand",
        LeftLeg = "l-leg",
        LeftFoot = "l-foot",
        RightArm = "r-arm",
        RightHand = "r-hand",
        RightLeg = "r-leg",
        RightFoot = "r-foot",
    }
}

/// A slot of the unit damage table: `(kind x 4 + direction) x 11 + part`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitDamageSlot {
    pub kind: DamageKind,
    pub direction: DamageDirection,
    pub part: BodyPart,
}

/// The number of slots of the unit damage table.
pub const UNIT_DAMAGE_SLOTS: usize = 176;

impl Slot for UnitDamageSlot {
    fn index(self) -> usize {
        (self.kind as usize * DamageDirection::COUNT + self.direction as usize) * BodyPart::COUNT + self.part as usize
    }
}

impl UnitDamageSlot {
    pub fn new(kind: DamageKind, direction: DamageDirection, part: BodyPart) -> Self {
        UnitDamageSlot { kind, direction, part }
    }

    /// The slot at `index` of the table (below `UNIT_DAMAGE_SLOTS`).
    pub fn from_index(index: usize) -> Option<Self> {
        if index >= UNIT_DAMAGE_SLOTS {
            return None;
        }
        let part = BodyPart::from_index(index % BodyPart::COUNT)?;
        let rest = index / BodyPart::COUNT;
        Some(UnitDamageSlot {
            kind: DamageKind::from_index(rest / DamageDirection::COUNT)?,
            direction: DamageDirection::from_index(rest % DamageDirection::COUNT)?,
            part,
        })
    }
}
