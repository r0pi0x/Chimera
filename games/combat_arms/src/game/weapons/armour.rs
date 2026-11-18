use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{character::CharacterPartsExt, later::Later};
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        Damage, DamageKind, DamageRules, Damageable, FirstTimePredicted, Health, Hitgroup, Intent, MovementState, NetRole,
        Predict, PredictedAppExt, PropSurface, SOLID_LAYERS, SimClock, Velocity,
    },
    map::{SurfaceNames, sound::SurfaceGrid},
    weapon::{
        AltModes, Burst, CarriedAppExt, Hitscan, ParamsAppExt, Inventory, NetPart, SpreadShape, ViewBackPush, ViewPunch,
        WeaponEvent,
        Armor, ArmorPieces, WeaponEventKind, WeaponFrame, Zoom, aim_of,
        hits,
        data::{WeaponData, WeaponDataAppExt},
        lagcomp,
        random::{Rng, shared_random},
    },
};

use crate::game::characters;
use crate::game::attachments::{CaNpc, CaSpecialRoll, DamageMods, RecoilMods, SpecialKind, SpreadMods, Victim};

pub use self::{
    classes::{ALT_FIRE, CaAltFire, CaAltVolley, CaSpin, CarryOver, PelletCarryOver, SPIN, SpinParams, SpinState, reported_hits},
    zoom::{CaFocusBlur, CaZoom, CaZoomSound, Scope, Sway, ZOOM, ZoomParams},
};

use super::*;

// ---------------------------------------------------------------------------
// Surfaces

/// One shoot-through surface (data/surfaces.toml), metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePass {
    /// Most material passed (0: always).
    pub thickness: f32,
    /// Exit direction offset, thousandths per axis.
    pub perturb: f32,
    pub range: f32,
    pub damage: f32,
    /// Its surface type number (`effects.toml`'s ids).
    pub id: u16,
}

/// Shoot-through surfaces by name (lower case).
#[derive(Resource, Clone, Debug, Default)]
pub struct CaSurfaces(pub Vec<(String, SurfacePass)>);

impl CaSurfaces {
    pub fn get(&self, name: &str) -> Option<SurfacePass> {
        let name = name.to_lowercase();
        self.0.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
    }

    /// Parse a surfaces document (data/surfaces.toml's format).
    pub fn parse(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Units {
            length: f32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Row {
            names: Vec<String>,
            ids: Vec<u16>,
            thickness: f32,
            perturb: f32,
            range: f32,
            damage: f32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Doc {
            units: Units,
            surface: Vec<Row>,
        }
        let doc: Doc = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in doc.surface {
            if row.ids.len() != row.names.len() {
                return Err(format!("{:?}: one id per name", row.names));
            }
            for (n, id) in row.names.into_iter().zip(row.ids) {
                out.push((
                    n.to_lowercase(),
                    SurfacePass {
                        thickness: row.thickness * doc.units.length,
                        perturb: row.perturb,
                        range: row.range,
                        damage: row.damage,
                        id,
                    },
                ));
            }
        }
        Ok(Self(out))
    }

    /// The shoot-through surfaces' type numbers, ascending.
    pub fn ids(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self.0.iter().map(|(_, s)| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// The built-in table (data/surfaces.toml).
    pub fn built_in() -> &'static Self {
        static SET: OnceLock<CaSurfaces> = OnceLock::new();
        SET.get_or_init(|| {
            Self::parse(include_str!("../../../data/surfaces.toml"))
                .unwrap_or_else(|e| panic!("Combat Arms surfaces.toml: {e}"))
        })
    }
}

