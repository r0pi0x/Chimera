//! Ultimate Gun Game's sandbox: `sandbox [map]` (console) and
//! `+sandbox [map]` (command line) play a local match on every game's guns
//! and grenades (the whole ladder, in the games' order, no draw) where
//! nothing advances and nothing ends, to try a gun with `rung` (chimera_sim's
//! `minigame::rung_command`). A person alone on the range (`chimera_world::
//! range`: distance lines, cover, a shelf, a target board, a platform), or
//! the map named (`sandbox <map>`, `sandbox greybox`; `--map` too), cheats
//! on (`sv_cheats 1`: `god`, `noclip`), no bots until `ca_bot_add`;
//! practice targets of any game with `target` (chimera_sim's
//! `minigame::targets`: placed in front of the player, still, strafing or
//! walking, firing back or not, protected or not, measured), every blast
//! measured, and props with `prop` (bottles and tin cans to knock off a
//! shelf, a target board keeping the shot group: `minigame::props`).
//! `sandbox repeat` puts the last layout of targets and props back in
//! front of the player (`minigame::layout`); `sandbox <god|ammo|timescale|
//! thirdperson|camdist|freeze> <value>` and `sandbox conditions` are the
//! conditions (`minigame::conditions`; the camera's two are the client's,
//! kept from its camera by `camera_state`).
//!
//! The definition (`SANDBOX`, `definition`) is the automatic gun game's
//! without its draw: every gun, grenade and melee weapon (`melee_weapons`:
//! TF2's nine, the knives) of the playable games as the ladder, `refill_ammo`, no knife finale, no time to speak of
//! (`TIME_LIMIT`), no rotation or vote, no music (`music = "none"`: no
//! game's music director plays its moments), no pre-match lobby (`lobby =
//! false`: entered at once from a match, with no end screen, lobby or
//! Start, and without waiting for the server's own game files; who goes
//! along is the map change's as before: everyone on the server), one player enough, a quick respawn at the map's spawn points (the range's firing line).
//! No option of the ladder module stops a kill from advancing or the last
//! rung's from winning, so the rule is `rung_command::HoldRungs` (inserted
//! by the plugin): a player stays on the rung they were put on, and the
//! ladder's last rung is a sentinel (a copy of the one before) that the
//! ladder's win lands on instead of ending the match.
//!
//! The host's games off (`mp_guns_games_off`, `gun_rules::GunRules`) leave
//! the ladder too (all games back when they would leave no gun), and the
//! definition is made again as each sandbox match starts
//! (`minigame::MatchStarting`, `regenerate_on_start`), so a games-off on
//! the command line (`+mp_guns_games_off hl2dm +sandbox +rung 1`, in any
//! order: the match starts once its map is in) shapes the ladder `rung`
//! reads.
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

/// The map without one named: the range (`chimera_world::range::RANGE`).
pub const DEFAULT_MAP: &str = "range";

/// The match's length, seconds (more than a day: it doesn't end).
pub const TIME_LIMIT: f32 = 1.0e7;

/// The sandbox's help (`sandbox help`).
pub const HELP: &str = "sandbox [map]: a match alone on the range (or `map`: greybox, a game's map), nothing advances or ends.\n\
    sandbox restart [map]: start it again (plain `sandbox` does nothing when it already plays on that map).
\
    rung <n | gun id or part | next | prev | list> [player | all]: put yourself on a rung (numbered from 1).\n\
    ca_bot_add [alpha|bravo]: add a bot; ca_bot_kick: remove them; ca_bot_stop 1: they stand still;\n\
    ca_bot_dont_shoot 1: they never fire.\n\
    target add <rung n | gun id | body id> [distance m] [still|strafe|walk] [shoots] [protect] [platform]: a practice target\n\
    in front of you, on the range on that distance line at your side (`platform`: on the range's platform, standing still; any game's body; it never fires unless `shoots`; `target characters` lists the bodies);\n\
    target set <n|all> <shoots|noshoot|protect|noprotect|still|strafe|walk|platform|line|gun <id>|character <id>|respawn <s>|distance <m>>;\n\
    target remove <n|all>; target list. Hits on targets and their kills are logged (HitMeasured, KillMeasured),\n\
    and every blast (BlastMeasured: centre, inner and outer radius, each victim's damage).\n\
    prop add <bottles|tin_cans|target_board> [distance m]: bottles or cans on a shelf to knock off, or a board\n\
    that keeps your shot group, under the crosshair (with a distance: on that line; on the range: its own shelf and board first); prop remove <n|all>; prop list; prop group <n>; prop reset <n|all>.\n\
    host_timescale 0.5: half speed (measured times stay in game seconds).\n\
    god, noclip: no damage, fly through walls (sv_cheats is 1 here).\n\
    guns never run dry (the ladder refills them); kills and deaths never move your rung.\n\
    sandbox repeat (also target repeat): clear, then put back the last layout of targets and props you made,\n\
    each on its own line and distance from where you stand and look now (kept across maps and restarts; one made on the range comes back there exactly).\n\
    rung <gun id> as <body id>: that gun as that body (TF2's shotgun as tf2:soldier, a Halo gun as halo_ce:elite).\n\
    sandbox god on|off, ammo on|off, timescale 1|0.5|0.25, thirdperson on|off, camdist <units>, freeze on|off:\n\
    the conditions (`sandbox conditions` says them; freeze holds every target still). No spread or recoil switch exists.\n\
    `+sandbox [map]` on the command line starts it at launch; `+rung <id>` there waits for the match.\n\
    ugg_new or `minigame <id>` leaves the sandbox.";

fn q(s: &str) -> String {
    format!("{s:?}")
}

fn list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| q(s)).collect::<Vec<_>>().join(", "))
}

/// The sandbox's definition text: every playable game's guns, grenades and
/// melee weapons (`melee`: ids, `melee_weapons`; each game's knife too) as
/// the ladder (`caps`, `installed`: as `auto::definition`), the base game's
/// slots, and the sentinel rung last. A game `game_on` refuses lends no
/// rung, unless that would leave none (then every game does).
pub fn definition(
    caps: &[Capabilities],
    base: &str,
    installed: &dyn Fn(&str) -> bool,
    melee: &[&str],
    game_on: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    let playable: Vec<&Capabilities> = caps.iter().filter(|c| c.playable() && installed(c.game)).collect();
    let on: Vec<&Capabilities> = playable.iter().copied().filter(|c| game_on(c.game)).collect();
    let games = if on.iter().any(|c| !c.guns.is_empty()) { on } else { playable };
    let mut ladder: Vec<String> = Vec::new();
    for c in &games {
        let prefix = format!("{}:", c.game);
        let own_melee = c.knife.into_iter().chain(melee.iter().copied().filter(|m| m.starts_with(&prefix)));
        for id in c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()).chain(own_melee) {
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
        // Its own knife only (none: no melee slot; never another game's).
        if let Some(knife) = caps.iter().find(|c| c.game == game).and_then(|c| c.knife) {
            kit.push(format!("{} = {}", q(&game), list(&[knife.to_string()])));
        }
    }
    let mut pool = vec!["chimera:range".to_string(), "chimera:greybox".to_string()];
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
         time_limit = {time}\nrotate = false\nmusic = \"none\"\nlobby = false\n\n\
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
/// gun, not the ladder's draw; but no record that is no player's,
/// `auto::drop_not_players`).
pub fn generate(world: &mut World, base: &str) -> Result<&'static MinigameDef, String> {
    let mut caps = minigame::auto::report(world);
    // No NPC, boss, turret or test record (the Change gun list).
    minigame::auto::drop_not_players(world, &mut caps);
    let games: Vec<&'static str> = caps.iter().map(|c| c.game).collect();
    let installed = chimera_net::minigame_net::installed_by_all(world, &games);
    let melee = melee_weapons(world);
    let rules = world.get_resource::<minigame::gun_rules::GunRules>().cloned().unwrap_or_default();
    let text = definition(&caps, base, &|g| installed.contains(&g), &melee, &|g| rules.game_on(g))?;
    // The same text again (each start makes it): the one there stays.
    let made = world.get_resource::<minigame::Minigames>().filter(|m| m.text(SANDBOX) == Some(text.as_str()));
    if let Some(def) = made.and_then(|m| m.get(SANDBOX)) {
        return Ok(def);
    }
    minigame::add(world, SANDBOX, &text)
}

/// A sandbox match about to start (`minigame::MatchStarting`, before its
/// definition is read): the definition made again with the host's Guns
/// rules as they are now (module doc).
pub fn regenerate_on_start(world: &mut World, id: &str, base: &str) {
    if id != SANDBOX {
        return;
    }
    if let Err(e) = generate(world, base) {
        world.resource_mut::<Console>().print(Level::Error, format!("sandbox: {e}"));
    }
}

/// Every melee weapon of the catalogue (a `[melee]` swing, no class, no
/// round: TF2's nine, a game's knives and crowbars), in its order: the
/// games' parts lend guns and grenades only, and the sandbox may hand any
/// of these too (Change gun lists them as Melee).
pub fn melee_weapons(world: &World) -> Vec<&'static str> {
    let Some(catalog) = world.get_resource::<chimera_sim::weapon::data::WeaponCatalog>() else { return Vec::new() };
    catalog.weapons().iter().filter(|d| d.melee.is_some() && d.class.is_none() && d.hitscan.is_none()).map(|d| d.id.as_str()).collect()
}

/// The words `sandbox <word> ...` takes for a condition
/// (`minigame::conditions`), not a map.
const CONDITIONS: [&str; 7] = ["god", "ammo", "timescale", "thirdperson", "camdist", "freeze", "conditions"];

/// `sandbox [map | restart [map] | help | repeat | <condition> <value>]`:
/// start it (nothing when it already plays on that map: `restart` starts
/// it again), or repeat the last layout, or set a condition.
pub fn command(w: &mut World, a: &[String], base: &str) -> Result<Option<String>, String> {
    let first = a.first().map(|s| s.to_ascii_lowercase());
    match first.as_deref() {
        Some("help") => return Ok(Some(HELP.to_string())),
        Some("repeat") => return minigame::layout::repeat_command(w),
        Some("conditions") => return minigame::conditions::command(w, &["list".to_string()]),
        Some(c) if CONDITIONS.contains(&c) => return minigame::conditions::command(w, a),
        _ => {}
    }
    // `sandbox restart [map]` starts it again whatever plays.
    let restart = first.as_deref() == Some("restart");
    let a = if restart { &a[1..] } else { a };
    if w.get_resource::<minigame::Minigames>().and_then(|m| m.get(SANDBOX)).is_none() {
        generate(w, base)?;
    }
    let map = a.first().map_or(DEFAULT_MAP, String::as_str);
    // From the main menu no game is in (the map "in play" is only a
    // name, the greybox's, or the greybox left behind a game left, its
    // match held): the map is loaded, and its load takes the front end
    // into the game as any map change does.
    #[cfg(feature = "present")]
    let at_menu = w.get_resource::<party_roster::main_menu::PartyMenu>().is_some_and(|m| m.shown());
    #[cfg(not(feature = "present"))]
    let at_menu = false;
    // Already playing there (or anywhere, with no map named: the range is
    // only where it starts): nothing to do (a start costs a hitch, the
    // start sound and the layout's bodies; a control that fires twice
    // must not restart it). Never at the menu: a sandbox left there is
    // not playing.
    let here = chimera_net::net::current_map(w);
    let there = a.first().is_none() || here == chimera_net::net::normalize_map(map);
    if !restart && !at_menu && minigame::rung_command::holds(w) && there {
        return Ok(Some(format!("sandbox: already playing on {here} (`sandbox restart` starts it again)")));
    }
    let said = if at_menu {
        chimera_net::minigame_net::change_match_loading(w, SANDBOX, Some(map))?
    } else {
        chimera_net::minigame_net::change_match(w, SANDBOX, Some(map))?
    };
    let mut console = w.resource_mut::<Console>();
    console.submit("sv_cheats 1");
    console.print(Level::Info, "sandbox: `sandbox help` lists what to try");
    Ok(Some(said))
}

/// The camera's conditions from the client's camera (`minigame::
/// conditions`: third person and its distance are the client's).
#[cfg(feature = "present")]
pub fn camera_state(
    camera: Option<Res<chimera_client::client::view::CameraMode>>,
    conditions: Option<ResMut<minigame::conditions::SandboxConditions>>,
) {
    let (Some(camera), Some(mut c)) = (camera, conditions) else { return };
    if c.third_person != camera.third_person || c.camera_distance != camera.ideal_dist {
        c.third_person = camera.third_person;
        c.camera_distance = camera.ideal_dist;
        info!("{}", c.line());
    }
}

/// A command line's `+sandbox [map]` made into what the app starts with:
/// the sandbox minigame on that map (the range without one), with no bots, with
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
    args.console.insert(3, format!("sandbox {}", map.or_else(|| args.map.clone()).unwrap_or_else(|| DEFAULT_MAP.into())));
    args
}
