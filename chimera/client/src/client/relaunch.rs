//! The game starts again when a join bound the player's asset token to
//! the server's asset service (`net::assets::AssetRelaunch`): the same
//! program and options, joining that server (`relaunch_args`), and this one
//! quits, since the games' mounts are opened at startup. Registered by
//! `net::ClientNetPlugin`.

use bevy::prelude::*;

/// Start the game again joining the server whose asset service the token
/// was just bound to, and quit.
pub fn relaunch(asked: Res<crate::net::assets::AssetRelaunch>, mut exit: MessageWriter<AppExit>) {
    let args = relaunch_args(std::env::args().skip(1), asked.0);
    let started = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(&args).spawn());
    match started {
        Ok(child) => {
            info!("starting again with the server's game files (process {})", child.id());
            exit.write(AppExit::Success);
        }
        Err(e) => error!("couldn't start the game again ({e}): start it yourself and join again"),
    }
}

/// The options a game started again gets: these, without an earlier
/// `+connect` (replaced by `server`'s) and without the token (kept already).
pub fn relaunch_args(args: impl IntoIterator<Item = String>, server: Option<std::net::SocketAddr>) -> Vec<String> {
    let token = format!("+{}", crate::net::assets::TOKEN_COMMAND);
    let mut out = Vec::new();
    let mut it = args.into_iter().peekable();
    while let Some(a) = it.next() {
        if a.eq_ignore_ascii_case("+connect") || a.eq_ignore_ascii_case(&token) {
            while it.peek().is_some_and(|n| !n.starts_with('+') && !n.starts_with("--")) {
                it.next();
            }
            continue;
        }
        out.push(a);
    }
    if let Some(server) = server {
        out.extend(["+connect".to_string(), server.to_string()]);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_game_started_again_joins_the_server_without_the_token() {
        let args = [
            "--offscreen",
            "1280x720",
            "+chimera:asset_token",
            "chimera1_x",
            "+connect",
            "1.2.3.4:5",
            "+name",
            "a",
        ];
        let out = super::relaunch_args(args.map(String::from), Some("9.8.7.6:27015".parse().unwrap()));
        assert_eq!(
            out,
            ["--offscreen", "1280x720", "+name", "a", "+connect", "9.8.7.6:27015"]
        );
    }
}
