//! The user's install (`[games.cs16] path` or `$CHIMERA_CS16_ROOT`):
//! it mounts. Passes with a note without one. Add a test here for each
//! file format the game reads (its maps, models, sounds), on the real
//! files, never on copies in the repository.

use cs16::game::INFO;

#[test]
fn the_install_mounts() {
    let Some(install) = INFO.install() else {
        eprintln!("no Counter-Strike 1.6 install ([games.cs16] path or CHIMERA_CS16_ROOT): skipped");
        return;
    };
    (INFO.open)(&install).unwrap_or_else(|e| panic!("{}: {e}", install.display()));
}
