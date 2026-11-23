//! What a playing minigame tells the local player's HUD (D2, L2): data, not
//! drawing. `MinigameHud` is rebuilt every frame by `update` from the
//! active minigame: its name (`{map}` in it: the map played, `shown_name`), the status lines its rules module gives the
//! local player (`MinigameStatus`, a component the rules' system writes:
//! the ladder's rung, the next gun, the leader), the match's phase and time, and brief notices made of the
//! match's `MinigameEvent`s (the module's `notice` for its own events,
//! `match_won` for every minigame; no HUD draws them now: each says a
//! moment the callout below says big, and drawn too they were a second
//! banner). Empty while no minigame plays. Any
//! game's HUD may show it (Combat Arms' and CS:S's do); it names nobody
//! and no weapon itself: a line's text has `{player}` and `{weapon}` where
//! the HUD puts the player's name and the weapon's display name, which
//! only the HUD's game knows (`HudLine`). The big moments for the viewer
//! (`Callout`: a rung up or down, the last rung, yours or another's, the
//! lead taken, lost or tied, the match won or lost) are kept apart
//! (`MinigameHud::callout`): the client shell draws them centre screen
//! and plays their cue's sound (`chimera_client::client::callouts`), or
//! the HUD up draws them in its own looks (Combat Arms'). The callouts of one frame merge into one: the highest
//! level's lines, then the others' first lines (at most `CALLOUT_LINES`),
//! with its cue; it replaces the one showing unless that one has a
//! higher level and is younger than `CALLOUT_HOLD`. `update` also tells the shell
//! when the dead local player comes back (`core::RespawnDue`, from
//! `runtime::respawn_due`): the shell's countdown and death view read it,
//! and then the match's moments for a music director (`moments::update`).
//!
//! **The rung hint** (`MinigameHud::rung_hint`, `RungHint`): while the
//! viewer's rung declares one (`ladder::rung_hint`: its weapon data's
//! opt-in `rung_hint`, Halo's grenade rung only so far), the action to
//! name and its verb, shown `RUNG_HINT_ARRIVAL` seconds from the arrival
//! on the rung (a rung change or a respawn onto it) and again
//! `RUNG_HINT_DRY_FIRE` seconds from each dry fire of the viewer's own
//! (its predicted `WeaponEventKind::DryFire`: on a network client too,
//! from its own input; nothing new crosses the network), or the whole
//! time on the rung while alive when its data says `always` (Halo's
//! grenade rung), with its title, whether the fire button does it too
//! and, with `count`, what is left to throw (the held weapon's rounds).
//! The HUD resolves the keys bound when it draws (`chimera_client::
//! client::rung_hint`).
//!
//! Known shortcuts: the notices (`RulesModule::notice`) are still made
//! though no HUD draws them any more (only tests read them); drop them,
//! or give them a place that is no banner (a log), when one is wanted.

use bevy::{ecs::message::MessageCursor, prelude::*};

use super::{ActiveMinigame, MinigameDef, MinigameEvent};
use crate::{
    core::{LocalPlayer, RespawnAt, RespawnDue, SimClock},
    mode::{Match, Phase},
};

/// How long a notice stays up (s).
pub const NOTICE_SECONDS: f64 = 4.0;

/// How long a callout stays up (s).
pub const CALLOUT_SECONDS: f64 = 3.0;

/// How long a callout keeps a lower-level one from replacing it (s).
pub const CALLOUT_HOLD: f64 = 1.5;

/// How long the rung hint shows on arriving on its rung, and on a dry
/// fire there (s).
pub const RUNG_HINT_ARRIVAL: f64 = 5.0;
pub const RUNG_HINT_DRY_FIRE: f64 = 3.0;

/// The viewer's rung hint (module doc): "Press <the action's key> to
/// <verb>", shown until `until` (`Time`'s seconds).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RungHint {
    pub action: String,
    pub verb: String,
    pub until: f64,
    /// The rung and whether alive when last seen (an arrival changes it).
    pub seen: (usize, bool),
    /// The data's title ("Grenade rung").
    pub title: Option<String>,
    /// The fire button does it too (named beside the action's key).
    pub fire: bool,
    /// Shown the whole time on the rung while alive (`RungHintData::always`).
    pub always: bool,
    /// What is left to throw (the held weapon's rounds), when the data asks.
    pub count: Option<u32>,
}

impl RungHint {
    /// Whether it shows at `now`.
    pub fn shown(&self, now: f64) -> bool {
        (self.always && self.seen.1) || now < self.until
    }
}

/// The most lines a merged callout has.
pub const CALLOUT_LINES: usize = 3;

/// A big moment for the viewer: lines (the first big), a cue key for
/// the sound (`chimera_core::cues`: `rung_up`, `last_rung`, ...) and a
/// level (a higher one wins).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Callout {
    pub lines: Vec<HudLine>,
    pub cue: Option<&'static str>,
    pub level: i32,
}

impl Callout {
    pub fn new(level: i32, cue: &'static str, lines: Vec<HudLine>) -> Self {
        Self {
            lines,
            cue: Some(cue),
            level,
        }
    }
}

/// The callout showing: since when (`Time`'s seconds) and its number
/// (each new one counts up: the shell plays its cue once).
#[derive(Clone, Debug, PartialEq)]
pub struct ShownCallout {
    pub callout: Callout,
    pub since: f64,
    pub id: u32,
}

/// One frame's callouts merged into one (module doc); None for none.
pub fn merge(mut callouts: Vec<Callout>) -> Option<Callout> {
    callouts.sort_by_key(|c| -c.level);
    let mut it = callouts.into_iter();
    let mut first = it.next()?;
    for c in it {
        if first.lines.len() >= CALLOUT_LINES {
            break;
        }
        first.lines.extend(c.lines.into_iter().next());
    }
    first.lines.truncate(CALLOUT_LINES);
    Some(first)
}

/// One line of the minigame's HUD: text with `{player}` (the HUD's name
/// for `player`) and `{weapon}` (the weapon's display name) in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudLine {
    pub text: String,
    pub player: Option<Entity>,
    pub weapon: Option<&'static str>,
}

impl HudLine {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..default()
        }
    }

    pub fn player(mut self, e: Entity) -> Self {
        self.player = Some(e);
        self
    }

    pub fn weapon(mut self, id: &'static str) -> Self {
        self.weapon = Some(id);
        self
    }
}

/// A player's status lines from the rules in play (the ladder's rung, the
/// next gun, the leader), written by the rules' own system
/// (`ladder::write_status`) when a line changes, on the host and on each
/// client alike; `update` reads the viewer's.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct MinigameStatus(pub Vec<HudLine>);

/// What the minigame shows the local player this frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MinigameHud {
    /// The minigame's name; empty while none plays.
    pub title: String,
    /// Its status, top to bottom.
    pub lines: Vec<HudLine>,
    /// Brief notices, oldest first, each with when (`Time`'s seconds) it
    /// goes.
    pub notices: Vec<(HudLine, f64)>,
    /// The finished match's results while the match is decided.
    pub results: Option<super::results::MatchResults>,
    /// The shell's match-end screen shows the results over the whole
    /// screen (`chimera_client::client::match_end` sets it): a game's
    /// panel leaves the results and notices out meanwhile.
    pub results_shown: bool,
    /// The callout showing, for `CALLOUT_SECONDS` (module doc).
    pub callout: Option<ShownCallout>,
    /// The last callout's number.
    pub callouts: u32,
    /// The viewer's rung's hint while it has one (module doc).
    pub rung_hint: Option<RungHint>,
}

impl MinigameHud {
    /// Whether a minigame is playing.
    pub fn playing(&self) -> bool {
        !self.title.is_empty()
    }
}

/// The minigame's name as players see it: the definition's `name`, with
/// `{map}` (if it has one) replaced by the display name of the map
/// actually played (`map::LoadedMapName`, else the pool's first map), so a
/// rotating pool's title follows the map; an arena slice of it in play
/// (`arena::PlayArea`), the slice's label (`vote::map_label`: "Blood
/// Gulch: West Base"). A name without `{map}` is as written.
pub fn shown_name(world: &World, def: &MinigameDef) -> String {
    if !def.name.contains("{map}") {
        return def.name.clone();
    }
    let map = world
        .get_resource::<chimera_world::map::LoadedMapName>()
        .map(|m| m.0.as_str())
        .filter(|m| !m.is_empty() && *m != "greybox")
        .or(def.maps.first().map(String::as_str))
        .unwrap_or("");
    // A slice of it in play: the slice's label (`vote::map_label`).
    if let Some(a) = world.get_resource::<super::arena::PlayArea>().filter(|a| a.slice.map.eq_ignore_ascii_case(map)) {
        return def.name.replace("{map}", &super::vote::map_label(world.get_resource(), &a.slice.key()));
    }
    def.name.replace("{map}", &map_display_name(map))
}

/// A map id (`cs_source:de_aztec`, `combat_arms:warehouse`) as a name to
/// read: no game part, no CS-style mode prefix (`de_`, `cs_`, ...),
/// underscores as spaces, words capitalised, a trailing number apart
/// (`de_dust2`: "Dust 2"). An arena slice (`game:map@id`) reads
/// "<map's name>: <slice's id as words>" (`halo_ce:bloodgulch@west_base`:
/// "Bloodgulch: West Base").
pub fn map_display_name(id: &str) -> String {
    if let Some((map, slice)) = id.split_once('@') {
        let words = slice
            .split('_')
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut cs = w.chars();
                cs.next().map(|c| c.to_uppercase().chain(cs).collect::<String>()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        return format!("{}: {words}", map_display_name(map));
    }
    let id = id.rsplit(':').next().unwrap_or(id);
    let id = ["de_", "cs_", "as_", "es_", "aim_", "ar_", "dm_", "gg_", "fy_", "zm_"]
        .iter()
        .find_map(|p| id.strip_prefix(p).filter(|r| !r.is_empty()))
        .unwrap_or(id);
    id.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let digits = w.trim_end_matches(|c: char| c.is_ascii_digit()).len();
            let (word, num) = w.split_at(digits);
            let mut cs = word.chars();
            let head = cs.next().map(|c| c.to_uppercase().chain(cs).collect::<String>()).unwrap_or_default();
            match (head.is_empty(), num.is_empty()) {
                (_, true) => head,
                (true, false) => num.to_string(),
                _ => format!("{head} {num}"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The match's own line: waiting for players, the time left, the end.
fn phase_line(world: &World) -> Option<HudLine> {
    let m = world.resource::<Match>();
    match m.phase {
        Phase::Waiting | Phase::Starting => Some(HudLine::new("Waiting for players")),
        Phase::Live => {
            let now = world.get_resource::<SimClock>()?.now;
            let left = (m.round_ends? - now).max(0.0).ceil() as u32;
            Some(HudLine::new(format!("Time {}:{:02}", left / 60, left % 60)))
        }
        Phase::RoundOver { .. } | Phase::Intermission { .. } | Phase::MatchOver => Some(HudLine::new("Match over")),
        Phase::Setup { .. } => None,
    }
}

/// `core::RespawnDue`, written only when it changes.
fn set_respawn_due(world: &mut World, due: Option<RespawnAt>) {
    match world.get_resource_mut::<RespawnDue>() {
        Some(mut r) => {
            r.set_if_neq(RespawnDue(due));
        }
        None if due.is_some() => world.insert_resource(RespawnDue(due)),
        None => {}
    }
}

/// `MinigameHud` for this frame (an exclusive system, `Update`), and the
/// local player's `core::RespawnDue` (`runtime::respawn_due`).
pub fn update(world: &mut World, mut cursor: Local<MessageCursor<MinigameEvent>>, mut shots: Local<MessageCursor<crate::weapon::WeaponEvent>>) {
    let events: Vec<MinigameEvent> = world
        .get_resource::<Messages<MinigameEvent>>()
        .map(|m| cursor.read(m).cloned().collect())
        .unwrap_or_default();
    let dry: Vec<Entity> = world
        .get_resource::<Messages<crate::weapon::WeaponEvent>>()
        .map(|m| shots.read(m).filter(|e| !e.replay && matches!(e.kind, crate::weapon::WeaponEventKind::DryFire)).map(|e| e.owner).collect())
        .unwrap_or_default();
    let Some(active) = world.get_resource::<ActiveMinigame>().copied() else {
        if world.get_resource::<MinigameHud>().is_some_and(MinigameHud::playing) {
            world.insert_resource(MinigameHud::default());
        }
        set_respawn_due(world, None);
        super::moments::update(world);
        return;
    };
    let now = world.resource::<Time>().elapsed_secs_f64();
    let viewer = chimera_core::query_cache::first::<With<LocalPlayer>>(world);
    let due = viewer.and_then(|v| {
        let at = super::runtime::respawn_due(world, v)?;
        Some(RespawnAt {
            at,
            delay: active.def.spawn.respawn.delay as f64,
        })
    });
    set_respawn_due(world, due);
    let mut hud = world.remove_resource::<MinigameHud>().unwrap_or_default();
    hud.title = shown_name(world, active.def);
    hud.notices.retain(|(_, until)| *until > now);
    if hud.callout.as_ref().is_some_and(|c| now - c.since >= CALLOUT_SECONDS) {
        hud.callout = None;
    }
    hud.lines.clear();
    hud.results = world.get_resource::<super::results::MatchResults>().cloned();
    hud.rung_hint = viewer.and_then(|v| rung_hint(world, v, hud.rung_hint.take(), dry.contains(&v), now));
    if let Some(v) = viewer {
        hud.lines.extend(world.get::<MinigameStatus>(v).map(|s| s.0.clone()).unwrap_or_default());
        hud.lines.extend(phase_line(world));
        let mut callouts = Vec::new();
        for event in &events {
            callouts.extend(match event.name {
                "match_won" => Some(match event.player {
                    Some(p) if p == v => Callout::new(5, "match_won", vec![HudLine::new("You win!")]),
                    Some(p) => Callout::new(5, "match_lost", vec![HudLine::new("{player} wins!").player(p)]),
                    None => Callout {
                        lines: vec![HudLine::new("Match over")],
                        cue: None,
                        level: 5,
                    },
                }),
                _ => (active.rules.callout)(world, v, event),
            });
            let notice = match event.name {
                "match_won" => Some(match event.player {
                    Some(p) if p == v => HudLine::new("You win the match!"),
                    Some(p) => HudLine::new("{player} wins the match").player(p),
                    None => HudLine::new("Match over"),
                }),
                _ => (active.rules.notice)(world, v, event),
            };
            if let Some(n) = notice {
                hud.notices.push((n, now + NOTICE_SECONDS));
            }
        }
        if let Some(c) = merge(callouts) {
            let held = hud.callout.as_ref().is_some_and(|s| s.callout.level > c.level && now - s.since < CALLOUT_HOLD);
            if !held {
                hud.callouts = hud.callouts.wrapping_add(1);
                hud.callout = Some(ShownCallout {
                    callout: c,
                    since: now,
                    id: hud.callouts,
                });
            }
        }
    }
    world.insert_resource(hud);
    super::moments::update(world);
}

/// The viewer's rung hint this frame (module doc): `last` the one before,
/// `dry` the viewer dry-fired.
fn rung_hint(world: &World, v: Entity, last: Option<RungHint>, dry: bool, now: f64) -> Option<RungHint> {
    let (rung, hint) = super::ladder::rung_hint(world, v)?;
    let alive = world.get::<crate::core::Health>(v).is_some_and(|h| h.current > 0.0);
    let seen = (rung, alive);
    let mut out = last.filter(|l| l.action == hint.action && l.verb == hint.verb).unwrap_or(RungHint {
        action: hint.action.clone(),
        verb: hint.verb.clone(),
        until: f64::MIN,
        seen: (usize::MAX, false),
        title: hint.title.clone(),
        fire: hint.fire,
        always: hint.always,
        count: None,
    });
    out.count = hint.count.then(|| held_rounds(world, v)).flatten();
    if alive && out.seen != seen {
        out.until = out.until.max(now + RUNG_HINT_ARRIVAL);
    }
    if alive && dry {
        out.until = out.until.max(now + RUNG_HINT_DRY_FIRE);
    }
    out.seen = seen;
    Some(out)
}

/// The rounds of `v`'s weapon in hand (`Magazine::clip`).
fn held_rounds(world: &World, v: Entity) -> Option<u32> {
    let w = world.get::<crate::weapon::Inventory>(v)?.active?;
    world.get::<crate::weapon::Magazine>(w).map(|m| m.clip)
}

#[cfg(test)]
mod name_tests {
    use super::map_display_name;

    #[test]
    fn map_ids_read_as_names() {
        assert_eq!(map_display_name("cs_source:de_aztec"), "Aztec");
        assert_eq!(map_display_name("de_dust2"), "Dust 2");
        assert_eq!(map_display_name("combat_arms:warehouse"), "Warehouse");
        assert_eq!(map_display_name("cs_source:cs_office"), "Office");
        assert_eq!(map_display_name("cs_source:de_cbble"), "Cbble");
        assert_eq!(map_display_name("halo_ce:bloodgulch@west_base"), "Bloodgulch: West Base");
    }

    #[test]
    fn a_title_on_a_slice_names_the_slice() {
        use crate::minigame::{MinigameDef, arena};
        let text = "[[slice]]\nmap = \"bloodgulch\"\nid = \"west_base\"\nname = \"West Base\"\nmap_name = \"Blood Gulch\"\n\
                    points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]\nseed = [5.0, 1.0, 5.0]\n";
        let def = MinigameDef::parse(
            "t",
            "id = \"t:t\"\nname = \"Gun Game: {map}\"\nplayers = { min = 1, max = 4 }\nmaps = [\"halo_ce:bloodgulch\"]\n\
             teams = \"free_for_all\"\ntime_limit = 60\n[rules]\nmodule = \"chimera:ladder\"\nladder = [\"x:y\"]\n",
        )
        .unwrap();
        let mut world = bevy::prelude::World::new();
        world.insert_resource(chimera_world::map::LoadedMapName("halo_ce:bloodgulch".into()));
        assert_eq!(super::shown_name(&world, &def), "Gun Game: Bloodgulch");
        let defs = arena::parse("halo_ce", text).unwrap();
        let slice = defs[0].slice.clone();
        world.insert_resource(arena::PlayArea { polygon: slice.polygon().unwrap(), slice, put_backs: 0 });
        assert_eq!(super::shown_name(&world, &def), "Gun Game: Bloodgulch: West Base", "no data: the id's");
        world.insert_resource(arena::ArenaSlices(defs));
        assert_eq!(super::shown_name(&world, &def), "Gun Game: Blood Gulch: West Base");
    }
}
