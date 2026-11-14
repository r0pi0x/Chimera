//! The repository's own checks (crate `repo_checks`, checks/): the
//! architecture tests (checks/tests/it/architecture.rs: layering, the asset, spec
//! and docs rules; determinism.rs: the simulation crates' ratchets; ecs.rs:
//! callback registries on an allowed list, per-tick exclusive systems
//! ratcheted), `affected`, the test selection the git hooks run, and
//! `docs_index` (`docs_index.rs`), which generates the module and debt lists
//! from the sources' `//!` docs, and `ui_tokens` (`ui_tokens.rs`), the UI
//! theme's values as the look book's CSS custom properties.
//!
//! `affected`: from the files a change touches, the workspace packages
//! whose tests it can affect, through cargo's crate graph (`cargo
//! metadata`): a package's tests depend on the package, its dependencies
//! and its dev-dependencies, so a change to a crate selects the tests of
//! every package whose tests depend on it (docs/plans/active/engine-split.md,
//! "Test selection": no coverage-based selection, the crate graph picks).
//! `cargo run -p repo_checks --bin affected -- --help` for the command.

pub mod affected;
pub mod docs_index;
pub mod ui_tokens;
