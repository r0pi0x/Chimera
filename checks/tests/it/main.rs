//! `repo_checks`' integration tests, as one test binary: the architecture
//! tests (the repository's layering, asset, spec and docs rules), the
//! simulation's determinism ratchets and the ECS-style checks (callback
//! registries, per-tick exclusive systems) and tests of the shell tools that
//! guard the disk (`scripts`). They run
//! on every selection of `affected`, docs-only changes included, and build
//! in seconds (no workspace crate under them). `cargo test -p repo_checks`.

mod architecture;
mod determinism;
mod ecs;
mod scripts;
