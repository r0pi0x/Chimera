//! Ultimate Gun Game's sandbox: `sandbox [map]` (console) and
//! `+sandbox [map]` (command line) play a local match on every game's guns
//! and grenades (the whole ladder, in the games' order, no draw) where
//! nothing advances and nothing ends, to try a gun with `rung` (chimera_sim's
//! `minigame::rung_command`). A person alone on the greybox (or the map
//! named, `sandbox <map>`; `--map` too), cheats on (`sv_cheats 1`: `god`,
//! `noclip`), no bots until `ca_bot_add`; practice targets of any game with
//! `target` (chimera_sim's `minigame::targets`: placed in front of the
//! player, still, strafing or walking, firing back or not, protected or
//! not, measured), every blast measured, and props with `prop` (bottles and
//! tin cans to knock off a shelf, a target board keeping the shot group:
//! `minigame::props`).
//!
//! The definition (`SANDBOX`, `definition`) is the automatic gun game's
//! without its draw: every gun and grenade of the playable games as the
//! ladder, `refill_ammo`, no knife finale, no time to speak of
//! (`TIME_LIMIT`), no rotation or vote, one player enough, a quick respawn.
//! No option of the ladder module stops a kill from advancing or the last
//! rung's from winning, so the rule is `rung_command::HoldRungs` (inserted
//! by the plugin): a player stays on the rung they were put on, and the
//! ladder's last rung is a sentinel (a copy of the one before) that the
//! ladder's win lands on instead of ending the match.
//!
//! Known shortcuts: rung extras (a game's per-rung draws: attachments,
//! characters) are the ladder's own draw, so a rung character (Combat Arms'
//! infected, its turret hands) is there only when the draw gives one; the
//! sentinel counts in the HUD's rung total.

use bevy::prelude::*;
use chimera_core::console::{Console, Level};
use chimera_sim::minigame::{self, MinigameDef, auto::Capabilities};

/// The sandbox minigame's id.
pub const SANDBOX: &str = "ultimate_gun_game:sandbox";

/// The match's length, seconds (more than a day: it doesn't end).
pub const TIME_LIMIT: f32 = 1.0e7;

/// The sandbox's help (`sandbox help`).
pub const HELP: &str = "sandbox [map]: a match alone on the greybox (or `map`), nothing advances or ends.\n\
    rung <n | gun id or part | next | prev | list> [player | all]: put yourself on a rung (numbered from 1).\n\
    ca_bot_add [alpha|bravo]: add a bot; ca_bot_kick: remove them; ca_bot_stop 1: they stand still;\n\
    ca_bot_dont_shoot 1: they never fire.\n\
    target add <rung n | gun id | body id> [distance m] [still|strafe|walk] [shoots] [protect]: a practice target\n\
    in front of you (any game's body; it never fires unless `shoots`; `target characters` lists the bodies);\n\
    target set <n|all> <shoots|noshoot|protect|noprotect|still|strafe|walk|gun <id>|character <id>|respawn <s>|distance <m>>;\n\
    target remove <n|all>; target list. Hits on targets and their kills are logged (HitMeasured, KillMeasured),\n\
    and every blast (BlastMeasured: centre, inner and outer radius, each victim's damage).\n\
    prop add <bottles|tin_cans|target_board> [distance m]: bottles or cans on a shelf to knock off, or a board\n\
    that keeps your shot group; prop remove <n|all>; prop list; prop group <n>; prop reset <n|all>.\n\
    host_timescale 0.5: half speed (measured times stay in game seconds).\n\
    god, noclip: no damage, fly through walls (sv_cheats is 1 here).\n\
    guns never run dry (the ladder refills them); kills and deaths never move your rung.\n\
    `+sandbox [map]` on the command line starts it at launch; `+rung <id>` there waits for the match.\n\
    ugg_new or `minigame <id>` leaves the sandbox.";

fn q(s: &str) -> String {
    format!("{s:?}")
}

fn list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| q(s)).collect::<Vec<_>>().join(", "))
}

/// The sandbox's definition text: every playable game's guns and grenades as
/// the ladder (`caps`, `installed`: as `auto::definition`), the base game's
/// slots, and the sentinel rung last.
pub fn definition(caps: &[Capabilities], base: &str, installed: &dyn Fn(&str) -> bool) -> Result<String, String> {
    let games: Vec<&Capabilities> = caps.iter().filter(|c| c.playable() && installed(c.game)).collect();
    let mut ladder: Vec<String> = Vec::new();
    for c in &games {
        for id in c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()) {
            if !ladder.iter().any(|l| l == id) {
                ladder.push(id.to_string());
            }
        }
    }
    let Some(last) = ladder.last().cloned() else {
        return Err("no game lends guns and movement that every player has".into());
    };
    // The sentinel: the win of the last rung lands on it (module doc).
    ladder.push(last);
    let base_caps = caps.iter().find(|c| c.game == base);
    let fallback = base_caps.and_then(|b| b.knife);
    let mut sets: Vec<String> = Vec::new();
    let mut kit: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for id in &ladder {
        let game = id.split_once(':').map_or(id.as_str(), |(g, _)| g).to_string();
        if seen.contains(&game) {
            continue;
        }
        seen.push(game.clone());
        if !sets.contains(&game) {
            sets.push(game.clone());
        }
        let own = caps.iter().find(|c| c.game == game).and_then(|c| c.knife);
        if let Some(knife) = own.or(fallback) {
            kit.push(format!("{} = {}", q(&game), list(&[knife.to_string()])));
            let of = knife.split_once(':').map_or(knife, |(g, _)| g).to_string();
            if !sets.contains(&of) {
                sets.push(of);
            }
        }
    }
    let mut pool = vec!["chimera:greybox".to_string()];
    for m in minigame::auto::map_pool(caps, installed) {
        if !pool.contains(&m) {
            pool.push(m);
        }
    }
    let movement = base_caps.and_then(|b| b.movement).map(|m| format!("movement = {}\n", q(m))).unwrap_or_default();
    let hud = base_caps.and_then(|b| b.hud).unwrap_or(chimera_core::game_parts::MINIMAL_HUD);
    let hud = if hud == chimera_core::game_parts::MINIMAL_HUD { String::new() } else { format!("hud = {}\n", q(hud)) };
    Ok(format!(
        "# The Ultimate Gun Game sandbox (apps/ultimate_gun_game/src/sandbox.rs).\n\
         id = {id}\nname = \"Sandbox\"\n\
         description = \"Try any game's gun: `rung` picks it; nothing advances or ends.\"\n\
         players = {{ min = 1, max = 16 }}\nmaps = {maps}\nteams = \"free_for_all\"\n\
         time_limit = {time}\nrotate = false\n\n\
         [slots]\n{hud}binds = {base}\ntick = {base}\nbots = {base}\n{movement}\
         identity = \"weapon\"\nweapons = {sets}\n\n\
         [spawn]\nrespawn = {{ delay = \"1s\", protection = \"1s\" }}\npoints = \"scored\"\n\
         min_distance = 20.32\nkit = {{ {kit} }}\n\n\
         [pickups]\nweapons = false\n\n\
         [rules]\nmodule = \"chimera:ladder\"\nladder = {ladder}\nadvance_on = \"kill\"\n\
         demote_on = \"none\"\nknife_finale = false\nrefill_ammo = true\nwin = {{ kind = \"ladder_end\" }}\n",
        id = q(SANDBOX),
        maps = list(&pool),
        time = TIME_LIMIT,
        base = q(base),
        sets = list(&sets),
        kit = kit.join(", "),
        ladder = list(&ladder),
    ))
}

/// The sandbox added as `SANDBOX` from the games' parts in `world` (every
/// gun, not the ladder's draw).
pub fn generate(world: &mut World, base: &str) -> Result<&'static MinigameDef, String> {
    let caps = minigame::auto::report(world);
    let games: Vec<&'static str> = caps.iter().map(|c| c.game).collect();
    let installed = chimera_net::minigame_net::installed_by_all(world, &games);
    let text = definition(&caps, base, &|g| installed.contains(&g))?;
    minigame::add(world, SANDBOX, &text)
}

/// `sandbox [map | help]`: start it.
pub fn command(w: &mut World, a: &[String], base: &str) -> Result<Option<String>, String> {
    if a.first().is_some_and(|h| h.eq_ignore_ascii_case("help")) {
        return Ok(Some(HELP.to_string()));
    }
    if w.get_resource::<minigame::Minigames>().and_then(|m| m.get(SANDBOX)).is_none() {
        generate(w, base)?;
    }
    let map = a.first().map_or("greybox", String::as_str);
    let said = chimera_net::minigame_net::change_match(w, SANDBOX, Some(map))?;
    let mut console = w.resource_mut::<Console>();
    console.submit("sv_cheats 1");
    console.print(Level::Info, "sandbox: `sandbox help` lists what to try");
    Ok(Some(said))
}

/// A command line's `+sandbox [map]` made into what the app starts with:
/// the sandbox minigame on that map (the greybox without one), with no bots, with
/// cheats; other lines (`+rung`) stay. Without `+sandbox` the args are as
/// given.
#[cfg(feature = "present")]
pub fn args(mut args: chimera_client::client::Args) -> chimera_client::client::Args {
    let Some(at) = args
        .console
        .iter()
        .position(|c| c.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("sandbox")))
    else {
        return args;
    };
    let line = args.console.remove(at);
    let map = line.split_whitespace().nth(1).map(|m| m.trim_matches('"').to_string());
    // `--map` can't ask for the greybox (the app's default map takes its
    // place), so the match starts where the app does and `sandbox` moves it.
    args.console.insert(0, "MaxPlayer 0".into());
    args.console.insert(1, "sv_cheats 1".into());
    args.console.insert(2, format!("minigame {SANDBOX}"));
    args.console.insert(3, format!("sandbox {}", map.or_else(|| args.map.clone()).unwrap_or_else(|| "greybox".into())));
    args
}
