//! Bots: characters whose intent comes from a brain instead of a keyboard
//! (README, "Characters and control"), so they use the same movement and
//! weapons as players (the match's movement, `movement::match_movement`,
//! not the local player's current one). The brain turns toward the nearest enemy it can see
//! at a limited turn rate, strafes, and fires once it has seen them for a
//! reaction time and is on target. With nobody in sight it follows its
//! team's plan (`tactics`): attackers walk to a site as a group and hold
//! it, defenders hold spots at the sites watching the approaches; it goes
//! after enemies it saw or heard (sounds within their falloff range: shots
//! carry far, footsteps less) when its role allows, and to teammates who
//! call for help. It walks the navigation mesh with its own path cost
//! (`path`) and checks corners as it goes (`look`). Now and then it throws
//! a grenade it carries at a remembered enemy or at the site it walks to
//! (`grenades`), and it looks away from flashes about to go off.
//!
//! Bots: `Bot` brain writing `Intent` (nearest visible enemy, limited turn
//! rate, reaction time, strafing; not while `Blinded` (aim worse as the white
//! fades), nor through a `SightBlocker`; otherwise follows its orders, chases
//! what it saw or heard when its role allows, answers teammates' calls, walks
//! the `NavMesh` with its own cost and looks where enemies could be while
//! walking (`Activity` says what it does); runs before the rules
//! (`SimSet::Rules`: freeze time and death hold its intent; `core::FreezeTime`
//! keeps standing still from counting as stuck) and weapon selection
//! (`weapon::SelectWeapons`) since it picks weapons; while firing it pulls down
//! against the recoil so its bullets stay on the target (`bot_recoil_control`);
//! a remembered enemy (`Bot::lead`) is forgotten once dead; `tactics.rs` (runs
//! before the brain): the map's scenario (bomb sites, or hostage groups with
//! the sides swapped), each site's approaches from either spawn and ranked hold
//! spots (`choose_holds`: cover, seeing an approach from a distance), per-round
//! `TeamPlan`s and per-bot `Orders` (attackers: one site as a group behind a
//! waiting leader, then hold spots there; defenders: `split` over the sites at
//! hold spots), the attackers gathering before their site (`TeamPlan::staging`,
//! flashes and smokes thrown meanwhile), radio calls (enemy spotted, need
//! backup) sending the nearest free teammates and up to two defenders from the
//! other sites, danger per area where teammates died, nav links bots got stuck
//! on (`failed_links`, shared); `path.rs`: the bots' step cost (stock
//! crouch/jump penalties, AVOID areas, climbs, big drops, danger, exposure,
//! per-team and per-bot route noise, links it got stuck on); bots are numbered
//! (`Bot 3`: the lowest free number) and their `core::Seed` comes from number
//! and team, the dice restarting from it each round (never from entity ids);
//! route following mounts mesh ladders from the front (sidestepping at the foot
//! when pressing in doesn't attach), counts a ladder's top or foot reached only
//! with the feet level with it (stuck under something short of the top:
//! sidesteps, then takes it as the top), steps off the top toward the next
//! point, hanging on to shoot a breakable in the way first (de_nuke's vent
//! grilles), steps off a ladder when nothing is left to walk, crouches under
//! low ceilings, shoots breakables in the way and walks around a link it keeps
//! getting stuck on; `aside.rs`: on maps without a navigation mesh (most
//! minigame and course maps) bots step off spawns and teleport destinations and
//! out of teleport triggers to a clear spot nearby and wait, and the console
//! says once that the map has none; `look.rs`: corner checks (encounter spots
//! on v9 meshes, else hiding spots in view ahead); `grenades.rs`: throws (only
//! while `objectives::RoundOpen`; now and then an HE or flash at an enemy seen
//! or heard in the last 5 s, out of sight, a flash at the site it walks to or a
//! smoke at the site's `tactics::smoke_spot`, cutting a defender's sight line
//! onto the approach; never a flash that would blind a teammate,
//! `flashes_mate`): `plan_throw` tries view pitches with the grenade's
//! `ThrowRule`, follows each arc with gravity and `grenade::bounce` in segment
//! traces against the world (`follow`) and keeps the one ending nearest the
//! target; `Toss` draws the grenade, pulls the pin, holds, releases on aim and
//! draws the previous weapon; `Avert`: looks away from its own flash, and
//! sometimes from a flash it sees in flight, until it pops; `bot_add`,
//! `bot_kick [name]`, `bot_quota` and `bot_quota_mode` (normal, fill counting
//! humans, match; kept by a network server, `keep_quota`, not before a human
//! plays with `bot_join_after_player`; `bot_add` raises it, `bot_kick
//! [all|t|ct|<difficulty>|<name>]` lowers it), names from the game's bot
//! profiles at `bot_difficulty` (`map::bot_profiles`, with `bot_prefix`; "Bot
//! N" without them), `bot_stop`, `bot_dont_shoot`, `bot_reaction`,
//! `bot_turn_rate`, `bot_aim_error`, `bot_grenades`, `bot_give` (a weapon to
//! every bot), `bot_freeze` / `bot_unfreeze [name|number|all]` (ours: one bot
//! held still and not firing, `Bot::frozen`; the debug UI's per-bot row); bots let go of the trigger between semi-automatic shots;
//! `objectives.rs`: the bomb carrier leads the attackers to their site and
//! plants at its target; after the plant (`Retake`) defenders fall back to a
//! rally point, gather and go in together, one defusing while the others take
//! cover spots, and attackers guard the bomb from cover spots that see it
//! (`Bot::objective` and the spot it keeps, `Bot::guard`, come before its
//! orders); `buy.rs`: what each bot wants from the shop
//! (`weapon::economy::BotBuying`: the profile's weapon preferences, grenades by
//! role, a kit for the defusing team), bought by `economy::autobuy`; teammates
//! give way to each other in doors (`give_way`); `bot_goto` (`Bot::send_to`,
//! before anything but a fight); `radio.rs`: radio calls (`core::Radio`: enemy
//! spotted on first sight, enemy down after a kill, need backup when badly
//! hurt; and on their own (`speak`): the attackers' leader's go go go / stick
//! together team as the round opens and follow me as the group goes onto the
//! site, cover me while planting or defusing, sector clear after a fight, in
//! position when holding, regroup team when most of the team is down) with a
//! cooldown per bot and call, once a round for commands, quiet when a teammate
//! just said it and one call per team per `TEAM_GAP` (3 s), deterministic from
//! bot numbers and plan seeds; `Bot::last_call`; `bot_radio`; teammates' radio
//! commands (`obey`, after `tactics`): one or two of the nearest bots answer
//! "Roger that" (or "Negative" when busy) after a delay from their seed, and
//! take a `radio::Order` (`Bot::order`): follow the caller (follow me, cover
//! me, stick together), hold near the caller's spot (hold position, get in
//! position), regroup on it, fall back toward the team's spawn (fall back, get
//! out of there), or drop orders and press on without waiting for the group
//! (go, storm the front: defenders hunt; take the point); a person's need
//! backup or taking fire gets an answer from a bot `tactics` sends.
//!
//! Known shortcuts (docs/tech-debt.md): Bot awareness is ours, not CS:S's (not
//! in the SDK): sight, hearing by each sound's falloff gain, 15 s memory; route
//! following is a simple point follower (no gap jumps; mesh ladders mounted
//! square from the front (spec's 0.9 line-up, 32-unit mount point), up looking
//! up, down by backing out over the top until 0.45 m past it, pressing in level
//! to grab it and backing off it level; the mount point counts as reached in 2D
//! from the floor up to 72 units below it (de_train's car ladders start 1.5 m
//! up), and no jumping, ducking under what's ahead (a car's underside) or
//! sideways keys within 0.6 m of the foot; sidestepping 0.25 m off the middle
//! after 0.6 s pressing at the foot (across first at up to 0.2 of full speed,
//! then pressing in), or after 0.25 s stuck more than 0.3 m short of the top
//! (each side 1.2 s, then the top counts as reached); a ladder whose brush is
//! narrower than the box with solid faces flush beside it (`LadderFit`, from
//! the map's brushes) is got on at once 2 units clear of one flank, alternating
//! sides, and climbed on that line (within 0.04 m); a ladder's end counts as
//! reached within 0.3 m below or a step (18 units) above; still rising below
//! the top it climbs on (no sideways key in the last 0.6 m); then toward a
//! point over the top it turns and walks there level, toward one beside or
//! behind (or stopped under a lip) it stays square, climbing with the sideways
//! key toward it; it climbs only while facing within 32° of square; on a ladder
//! it hangs to shoot a breakable in the way (no further than 0.5 m past the
//! route point); going down, the point behind the top counts as reached within
//! 1.5 m (spec: the follower walks to the top's front); with nothing left to
//! walk on a ladder it backs off along the ladder's normal; a climb with no
//! mesh ladder is walked straight on once within 25° of it; no repath while on
//! a ladder or airborne; jump as fresh presses, never on a ladder; crouch under
//! a ceiling lower than 73 units here or 0.6 m ahead; shoots a `Damageable`
//! brush within 2.5 m in the way after 0.5 s blocked or 2 s on one route point
//! (vent grilles); doors by pressing use when blocked; teammates and hostages
//! pushed apart within 1.2 m; teammates in the way within 2.2 m ahead and 0.9 m
//! of the line walked (`bot::give_way`): one coming the other way faster than 1
//! m/s makes both keep right and the one giving way (to a person always, else
//! to the lower bot number) slow to 0.15 of full speed, one standing (under 0.6
//! m/s) is stepped round on the side with more room, one walking ahead closer
//! than 1.3 m slows it to half speed; jump-duck when blocked 0.4 s); stuck
//! recovery: no 0.5 m of progress in two 1.5 s checks in a row makes the link
//! it was crossing cost ×30 + 40 m for that bot for 25 s, and for every bot 60
//! s per report (for the rest of the map after 3 reports); a bot within 2.5 m
//! of its hold spot holds where it is when a teammate stands within 1 m of the
//! spot or it was blocked 1 s; maps without a `.nav` leave bots standing. Team
//! play is ours too (`bot::tactics`, `path`, `look`; nav spec open questions
//! 2-4): the path cost (stock crouch ×20/jump ×5, AVOID ×9, climbs ×2, drops
//! over 200 units ×21, danger ×3 per recent teammate death nearby halving every
//! 30 s, exposure up to ×0.6 from the visible-area count or hiding-spot flags,
//! route noise up to ×0.8 per team and round and ×0.25 per bot, crossing points
//! leaning up to 45% along edges over 2 m); attackers' group rules (the bomb
//! carrier leads, else the first member; the leader waits up to 5 s while
//! someone is over 12 m away, followers over 10 m go to the leader, ones ahead
//! wait; once, with 24 m of route left to the site, everyone gathers within 3.5
//! m of the leader until all are within 7 m (2-8 s) while those with flashes or
//! smokes throw them (50 % per half second) at the site's attacker approach
//! nearest them, then they go in without further waiting; the carrier leaves
//! the group for its target 15 m from the site; after the plant attackers hold
//! the bomb's site; after a plant (`bot::objectives::Retake`) the defenders
//! fall back to a rally point 15 m along the mesh from the bomb toward the mean
//! of the defenders more than 20 m from it, gather there (all within 6 m, then
//! 1 s), and go in together when gathered, when the bomb's time left is down to
//! the walk (straight distance x 1.4 at 6 m/s) plus the defuse plus 4 s, after
//! 25 s, or with one bot left; one standing within 4 m of the bomb with no
//! enemy seen or heard for 3 s defuses at once; the defuser is the nearest with
//! a kit (10 m head start), else the nearest, and the others take cover spots
//! (hiding spots within 18 m that see the bomb from 3 m or more, on the rally
//! point's side, ranked by `choose_holds` watching the attackers' approaches
//! and the first three guard spots), giving one up for the next after 1 s
//! blocked beside a teammate, and nobody but the defuser presses use at the
//! bomb; the planting team guards the bomb from such spots watching the
//! defenders' approaches and the bomb; both chase what they saw or heard within
//! 15 m of their spot; the terrorist nearest a dropped bomb fetches it), site
//! arrival at 7 m, approaches as where up to 3 routes from the spawn's mean
//! position (later ones avoiding earlier ones' areas ×6, at most 2.5× as long)
//! cross 16 m of path from the site, hold spots ranked by our score (cover, 18
//! m path radius, sees an approach 5-40 m away by a ray from 1.6 m to 1.2 m, 4
//! m apart); holding without contact 20-35 s (attackers: next site) or 45-80 s
//! (defenders: hunt); chasing only within 25 m (attackers on the way) or 15 m
//! of the hold spot; radio calls send the 1 (enemy spotted) or 2 (need backup,
//! taking fire) nearest free teammates within 8-60 m for 20 s; a defender's
//! "enemy spotted", "need backup" or "taking fire" within 27 m of its site
//! moves the nearest free defenders of the other sites there (two per round,
//! one always left behind); corner checks look at a spot 0.3 s or give up after
//! 2 s, remember it 10 s; a v16 area's visible set is its own list plus the one
//! it inherits from (spec silent). Recoil control is ours: a firing bot aims so
//! its bullets (view + the weapon's `punch_scale` × punch) stay on the target
//! (`bot_recoil_control` 1; how far CS:S bots pull down is unmeasured). A
//! remembered enemy is forgotten once dead (seen or heard: `Bot::lead`).
//! Grenade use is ours: buying chances by role (`bot::buy`: attackers HE 0.5,
//! flash 0.7, smoke 0.5; defenders and roamers 0.6, 0.5, 0.3; a second flash at
//! half the chance), when to throw (an enemy seen or heard in the last 5 s,
//! still alive and 4-30 m away, 20 % per half second; never in the freeze time
//! or after the round is decided (`objectives::RoundOpen`), a throw not yet
//! pinned dropped when it closes; the site walked to, 6 %; 6-12 s apart), the
//! arc planned with rays (not the grenade's box) and no carried velocity (the
//! bot stands still to throw), a 0.4 s pin hold, turning the back to its own
//! flash rather than throwing around corners on purpose, looking away from 60 %
//! of flashes it sees; a flash that would blind a teammate more than 1.5 s
//! (`bot::grenades::flashes_mate`, by the game's flash model, where the
//! teammate stands and where it walks to within 1.5 s; people taken as facing
//! it at least sideways, bots as they look) is aimed 3 m deeper once, else held
//! back; smokes go on the floor between an attackers' approach and the best
//! defender hold watching it (`tactics::smoke_spot`: half, a third or two
//! thirds of the way, 3-12 m from the approach, the cloud's middle 1 m up
//! within 2 m of the sight line), flashes on the approach (staging) or the
//! site; "Fire in the hole" is the throw's own radio (`weapon::grenade`, any
//! thrower). Why it's OK for now: Plays believable rounds for practice; CS:S's
//! own bot behaviour needs a behaviour spec.
//!
//! Known shortcuts (docs/tech-debt.md): Bots' sight in a crowd: each bot
//! traces to enemies nearest first until one is in sight (PVS first), but
//! in a minigame map's lobby, everyone in each other's PVS and in the way
//! of each other's rays, a bot traces to many and each ray tests many
//! capsules: mg_jacks_multigames_v1 with 64 bots in deathmatch ~25 ms a
//! tick (32: ~10 ms; measured before `think` spread the traces over the task
//! pool, which took a bot-full de_dust2's sight from 1.84 to 1.02 ms a tick at
//! 256 characters). CS:S's vision update rate and whether players block
//! bots' sight lines need a spec. Why it's OK for now: Humans don't cost it;
//! 32 bots fit.

pub mod aside;
pub mod buy;
mod grenades;
mod look;
mod navigation;
pub mod objectives;
pub mod path;
mod quota;
pub mod radio;
pub mod tactics;
#[cfg(test)]
mod tests;
mod think;

pub use navigation::*;
pub use quota::*;
pub use think::*;

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    character::CAPSULE_HEIGHT,
    character::spawn_character,
    console::{Command, Console, resource_cvar},
    core::{Health, Intent, MapBrush, MapBrushes, MovementState, SimSet, SpawnPoint, Team},
    map::nav::{NavLadder, NavMesh, STEP_HEIGHT, flags},
    weapon::{Inventory, grenade::Projectile},
};

pub use grenades::{GrenadePlan, MateView, flashes_mate, follow, plan_throw};
pub use tactics::{Hold, Role, Tactics};

pub struct BotPlugin;

impl Plugin for BotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BotConfig>()
            .init_resource::<BotQuota>()
            .init_resource::<radio::TeamCalls>()
            .init_resource::<Tactics>()
            .init_resource::<objectives::Retake>()
            .add_message::<crate::core::Radio>();
        // Bots are the server's (`core::authoritative`).
        app.add_systems(
            FixedUpdate,
            radio::speak
                .after(crate::core::apply_damage)
                .run_if(crate::core::authoritative)
                .run_if(crate::objectives::plays),
        );
        app.add_systems(
            FixedUpdate,
            (
                hear,
                tactics::update,
                radio::obey,
                objectives::goals,
                think,
                objectives::act,
                aside::step_aside,
            )
                .chain()
                // Before the rules: they hold everyone's intents in the
                // freeze time and the dead's (`rules::rounds::hold_frozen`).
                .before(SimSet::Rules)
                .before(crate::weapon::SelectWeapons)
                .run_if(crate::core::authoritative)
                .run_if(crate::objectives::plays),
        );
        // What they want from the shop (`weapon::economy::autobuy`).
        app.add_systems(
            FixedUpdate,
            buy::wishes.run_if(crate::core::authoritative).run_if(crate::objectives::plays),
        );
        // A network server keeps the bots at the quota (`keep_quota`).
        app.add_systems(
            Update,
            keep_quota
                .run_if(resource_equals(crate::core::NetRole::Server))
                .run_if(crate::objectives::plays),
        );
        resource_cvar::<BotQuota, u32>(
            app,
            "bot_quota",
            "Bots a server keeps in the game (bot_add raises it, bot_kick lowers it; see bot_quota_mode). \
             Single player adds and kicks bots only when told.",
            |q| &mut q.quota,
        );
        resource_cvar::<BotQuota, String>(
            app,
            "bot_quota_mode",
            "normal: bot_quota bots; fill: bots fill the game up to bot_quota players (humans count); \
             match: bot_quota bots per human.",
            |q| &mut q.mode,
        );
        resource_cvar::<BotQuota, u8>(
            app,
            "bot_join_after_player",
            "1: the bot quota adds no bots until a human player is in the game.",
            |q| &mut q.join_after_player,
        );
        resource_cvar::<BotQuota, u8>(
            app,
            "bot_difficulty",
            "Difficulty of bots added from now on: 0 easy, 1 normal, 2 hard, 3 expert (their names come with it).",
            |q| &mut q.difficulty,
        );
        resource_cvar::<BotQuota, String>(
            app,
            "bot_prefix",
            "Put before the names of bots added from now on.",
            |q| &mut q.prefix,
        );
        resource_cvar::<BotConfig, u8>(app, "bot_stop", "1: bots stand still.", |c| &mut c.stop);
        resource_cvar::<BotConfig, u8>(app, "bot_dont_shoot", "1: bots never fire.", |c| &mut c.dont_shoot);
        resource_cvar::<BotConfig, f32>(
            app,
            "bot_reaction",
            "Seconds a bot needs to see you before firing.",
            |c| &mut c.reaction,
        );
        resource_cvar::<BotConfig, u8>(
            app,
            "bot_grenades",
            "Bots throw grenades: 0 never, 1 now and then, 2 whenever they can.",
            |c| &mut c.grenades,
        );
        resource_cvar::<BotConfig, u8>(
            app,
            "bot_radio",
            "1: bots use the team radio (enemy spotted and down, need backup, commands such as follow me and cover me, sector clear, in position).",
            |c| &mut c.radio,
        );
        resource_cvar::<BotConfig, f32>(app, "bot_aim_error", "Bot aim wobble, degrees.", |c| &mut c.aim_error);
        resource_cvar::<BotConfig, f32>(
            app,
            "bot_recoil_control",
            "Share of the recoil bots pull down against while firing (1: bullets kept on the target, 0: none).",
            |c| &mut c.recoil_control,
        );
        resource_cvar::<BotConfig, f32>(app, "bot_turn_rate", "Bot turn speed, degrees per second.", |c| {
            &mut c.turn_rate
        });
        app.init_resource::<Console>();
        let mut console = app.world_mut().resource_mut::<Console>();
        console.add_command(Command {
            name: "bot_add".into(),
            help: "bot_add [team]: add a bot (team 1 by default; players are team 0).".into(),
            run: Arc::new(|w, a| {
                let team = match a.first() {
                    Some(t) => t.parse().map_err(|_| format!("bad team {t}"))?,
                    None => 1,
                };
                let e = add_bot(w, Team(team)).ok_or("no spawn point")?;
                let name = w.get::<Name>(e).map_or_else(|| e.to_string(), |n| n.as_str().to_string());
                Ok(Some(format!("added {name}")))
            }),
            complete: None,
        });
        console.add_command(Command {
            name: "bot_give".into(),
            help: "bot_give <weapon>: give every bot a weapon (e.g. weapon_hegrenade); bots keep their gun in hand."
                .into(),
            run: Arc::new(|w, a| {
                let name = a.first().ok_or("usage: bot_give <weapon>")?.to_lowercase();
                let wanted = if name.contains(':') || name.starts_with("weapon_") {
                    name
                } else {
                    format!("weapon_{name}")
                };
                let id = w
                    .resource::<crate::weapon::WeaponRegistry>()
                    .find(&wanted)
                    .map(|d| d.id)
                    .ok_or_else(|| format!("no weapon {wanted}"))?;
                let bots: Vec<Entity> = w.query_filtered::<Entity, With<Bot>>().iter(w).collect();
                for &b in &bots {
                    crate::weapon::give(w, b, id);
                }
                Ok(Some(format!("gave {id} to {} bots", bots.len())))
            }),
            complete: None,
        });
        console.add_command(Command {
            name: "bot_kick".into(),
            help: "bot_kick [all|t|ct|easy|normal|hard|expert|<name>]: remove every bot (bot_quota 0), those of a \
                   team or difficulty, or the one by that name (the quota lowered by those kicked)."
                .into(),
            run: Arc::new(|w, a| {
                let wanted = a.join(" ");
                let all = wanted.is_empty() || wanted.eq_ignore_ascii_case("all");
                let team = match wanted.to_ascii_lowercase().as_str() {
                    "t" => Some(Team(1)),
                    "ct" => Some(Team(2)),
                    _ => None,
                };
                let level = crate::map::bot_profiles::difficulty::of_name(&wanted);
                let bots: Vec<(Entity, String)> = w
                    .query::<(Entity, &Bot, Option<&Name>, Option<&Team>)>()
                    .iter(w)
                    .filter(|(_, b, n, t)| {
                        all || team.is_some_and(|x| t.copied() == Some(x))
                            || level.is_some_and(|l| b.difficulty & l != 0)
                            || n.is_some_and(|n| n.as_str().eq_ignore_ascii_case(&wanted))
                    })
                    .map(|(e, _, n, _)| (e, n.map_or_else(String::new, |n| n.as_str().to_string())))
                    .collect();
                if !all && bots.is_empty() {
                    return Err(format!("no bot \"{wanted}\""));
                }
                let n = bots.len();
                for (b, _) in bots {
                    kick_bot(w, b);
                }
                let mut quota = w.resource_mut::<BotQuota>();
                quota.quota = if all { 0 } else { quota.quota.saturating_sub(n as u32) };
                Ok(Some(format!("kicked {n} bots")))
            }),
            complete: None,
        });
        for (name, freeze) in [("bot_freeze", true), ("bot_unfreeze", false)] {
            console.add_command(Command {
                name: name.into(),
                help: format!(
                    "{name} [<name>|<number>|all]: {} (a bot's number is the one in \"Bot 3\"); no argument: every bot.",
                    if freeze { "hold a bot still and stop its fire" } else { "let a frozen bot move and fire again" }
                ),
                run: Arc::new(move |w, a| {
                    let wanted = a.join(" ");
                    let all = wanted.is_empty() || wanted.eq_ignore_ascii_case("all");
                    let by_number = wanted.parse::<u32>().ok();
                    let mut n = 0;
                    let mut q = w.query::<(&mut Bot, Option<&Name>)>();
                    for (mut b, nm) in q.iter_mut(w) {
                        if all
                            || by_number == Some(b.number)
                            || nm.is_some_and(|nm| nm.as_str().eq_ignore_ascii_case(&wanted))
                        {
                            b.frozen = freeze;
                            n += 1;
                        }
                    }
                    if n == 0 && !all {
                        return Err(format!("no bot \"{wanted}\""));
                    }
                    Ok(Some(format!("{} {n} bots", if freeze { "froze" } else { "unfroze" })))
                }),
                complete: None,
            });
        }
        console.add_command(Command {
            name: "bot_goto".into(),
            help: "bot_goto <x> <y> <z>: send every bot to a feet position (CS:S units, as getpos less the eye height); \
                   bot_goto alone lets them go back to their orders."
                .into(),
            run: Arc::new(|w, a| {
                let at = match &a[..] {
                    [] => None,
                    [x, y, z] => {
                        let n = |s: &String| s.parse::<f32>().map_err(|_| format!("bad number \"{s}\""));
                        Some(Vec3::new(n(x)?, n(z)?, -n(y)?) * 0.0254)
                    }
                    _ => return Err("bot_goto [<x> <y> <z>]".into()),
                };
                let mut q = w.query::<&mut Bot>();
                let mut n = 0;
                for mut b in q.iter_mut(w) {
                    b.send_to(at);
                    n += 1;
                }
                Ok(Some(format!("sent {n} bots")))
            }),
            complete: None,
        });
        // The server operator's: a network client can't run them.
        for name in SERVER_COMMANDS {
            console.server_command(&format!("{}:{name}", crate::game::GAME));
        }
    }
}

/// The bot commands that change the server's game: a network client can't
/// run them (`Console::server_command`).
pub const SERVER_COMMANDS: &[&str] =
    &["bot_add", "bot_add_t", "bot_add_ct", "bot_kick", "bot_give", "bot_goto", "bot_freeze", "bot_unfreeze"];

