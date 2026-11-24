//! GoldSrc on Chimera: what every GoldSrc game shares, above the formats
//! (`goldsrc_formats`) and below the games (`cs16`; the `goldsrc` loader
//! layer, docs/ARCHITECTURE.md, "Crates"; the plan is
//! docs/plans/active/goldsrc.md, section 5). So far `content`: a BSP 30
//! map as Chimera's `MapData` and a studio model as a character body,
//! each converter versioned (`content::CONVERTER`, build.rs) and
//! deterministic.

pub mod content;

// `crate::map`, `crate::core` and `crate::mount` inside, as in the game
// crates (the architecture check reads `crate::<module>` against its
// layering table).
#[allow(unused_imports)]
use chimera_core::{core, mount};
#[allow(unused_imports)]
use chimera_world::map;
