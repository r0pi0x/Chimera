//! Stamps the build with its git commit (`CHIMERA_GIT`) and version
//! (`CHIMERA_VERSION`): `hl2dm_app::BUILD` and `VERSION`, for bug
//! reports and the `version` command. The script every app shares
//! (chimera/core/build_stamp.rs), as the CS:S app's.

include!("../../chimera/core/build_stamp.rs");

fn main() {
    stamp();
}
