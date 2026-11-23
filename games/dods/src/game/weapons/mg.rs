//! DoD:S's machine guns' own part (`dods:mg`, `DodsMg`; specs/dods/weapons.md,
//! "Alt fire"): deployed only while prone (`stance`: the alt fire deploys,
//! the next shot waits `deploy_time`, 1.2 s, measured), its spread then the
//! script's `SecondaryAccuracy` (`dods:gun`'s alt accuracy: 0.01 the .30
//! cal, 0.025 the MG42), else its `Accuracy` (0.2); fired undeployed, each
//! shot costs its holder `stamina_per_shot` stamina (`stamina`).

use bevy::prelude::*;
use serde::Deserialize;

/// `dods:mg`'s parameters.
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DodsMg {
    /// After deploying, seconds before the first shot (measured ≈1.2).
    pub deploy_time: f32,
    /// Stamina a shot fired undeployed costs (measured: 100 to 85 on the
    /// first; weapons.md, open question 4).
    pub stamina_per_shot: f32,
}
