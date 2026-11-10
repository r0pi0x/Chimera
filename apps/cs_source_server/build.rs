//! Stamps the build with its git commit (`CHIMERA_GIT`) and version
//! (`CHIMERA_VERSION`): `cs_source_server::VERSION`, for its banner and
//! the `version` command. The script every app shares
//! (chimera/core/build_stamp.rs).

include!("../../chimera/core/build_stamp.rs");

fn main() {
    stamp();
}
