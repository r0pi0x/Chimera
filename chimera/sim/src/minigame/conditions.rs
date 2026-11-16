//! The sandbox's conditions as plain on/off requests (Ultimate Gun Game's
//! `sandbox`, its bench's switches): each a `SetCondition` message, the
//! console's `condition <name> <value>` (the app's `sandbox <name>
//! <value>` too), and its current state in `SandboxConditions`, kept from
//! what each condition really is each frame.
//!
//! | Condition | Request | State field | Console | What it is |
//! |---|---|---|---|---|
//! | No damage to me | `Condition::God(bool)` | `god` | `condition god on\|off` | the `god` command (toggles: sent only when it must change); the state is the local player's `core::God` |
//! | Endless ammo | `Condition::EndlessAmmo(bool)` | `endless_ammo` | `condition ammo on\|off` | the ladder's `refill_ammo` refill (a held rung gun's reserve topped up, a rung's grenade given back); off, the reserve runs down. It never touched the magazine: one empties and reloads either way |
//! | Time scale | `Condition::TimeScale(f32)` | `time_scale` | `condition timescale <1\|0.5\|0.25>` | `host_timescale` (the client's command); the state is `Time<Virtual>`'s speed (0 while paused) |
//! | Third person | `Condition::ThirdPerson(bool)` | `third_person` | `condition thirdperson on\|off` | `thirdperson` / `firstperson` (the client's) |
//! | Its camera distance | `Condition::CameraDistance(f32)` | `camera_distance` | `condition camdist <units>` | `cam_idealdist` (the game's units, CS:S's inches) |
//! | Bots frozen | `Condition::BotsFrozen(bool)` | `bots_frozen` | `condition freeze on\|off` | every practice target moves as a still one (`targets`): it stands on its spot (walks back when pushed off) |
//!
//! The camera's two fields are the client's (`chimera_client`'s camera):
//! the app that has both keeps them from it (Ultimate Gun Game's
//! `sandbox::camera_state`); with no client they say what was last asked.
//! No spread and no recoil: the engine has no such switch (no game's
//! spread or recoil can be turned off), so there is no request for them.
//! Blast rings are the screens' own drawing.
//!
//! **Who may**: while the sandbox plays, or in a local match (not a
//! network client) with `sv_cheats 1`; elsewhere a request is refused
//! (`ConditionRefused { reason }`, the console's error). The endless-ammo
//! and frozen switches go back to on and off when another minigame starts.
//! Each change of the state is logged (`SandboxConditions god=... ammo=...`).
//! Nothing here is replicated.

use bevy::prelude::*;

use super::{ActiveMinigame, rung_command};
use crate::{
    console::{Cheats, Console, ConsoleAppExt},
    core::{God, LocalPlayer, NetRole},
};

/// One condition and the value asked for (module doc's table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Condition {
    God(bool),
    EndlessAmmo(bool),
    TimeScale(f32),
    ThirdPerson(bool),
    CameraDistance(f32),
    BotsFrozen(bool),
}

/// Set a condition (module doc).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct SetCondition(pub Condition);

/// A `SetCondition` refused, and why.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ConditionRefused {
    pub reason: String,
}

/// The conditions now (module doc's table), for the screens' switches.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SandboxConditions {
    pub god: bool,
    pub endless_ammo: bool,
    pub time_scale: f32,
    pub third_person: bool,
    /// `cam_idealdist`, the game's units.
    pub camera_distance: f32,
    pub bots_frozen: bool,
}

impl Default for SandboxConditions {
    fn default() -> Self {
        Self { god: false, endless_ammo: true, time_scale: 1.0, third_person: false, camera_distance: 150.0, bots_frozen: false }
    }
}

impl SandboxConditions {
    /// The log's line.
    pub fn line(&self) -> String {
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "SandboxConditions god={} ammo={} timescale={} thirdperson={} camdist={} frozen={}",
            on(self.god),
            on(self.endless_ammo),
            self.time_scale,
            on(self.third_person),
            self.camera_distance,
            on(self.bots_frozen)
        )
    }
}

/// Whether conditions may be set here now (module doc, "Who may").
pub fn allowed(world: &World) -> bool {
    rung_command::holds(world)
        || (world.get_resource::<NetRole>() != Some(&NetRole::Client) && world.get_resource::<Cheats>().is_some_and(|c| c.0 != 0))
}

/// Set `c` now; what was done.
pub fn set(world: &mut World, c: Condition) -> Result<String, String> {
    if !allowed(world) {
        return Err("conditions are the sandbox's (or a local match's with sv_cheats 1)".into());
    }
    let now = world.get_resource::<SandboxConditions>().copied().unwrap_or_default();
    let mut submit = |line: String| {
        if let Some(mut console) = world.get_resource_mut::<Console>() {
            console.submit(line);
        }
    };
    match c {
        Condition::God(on) => {
            if now.god != on {
                submit("god".into());
            }
        }
        Condition::TimeScale(s) => {
            if !(s.is_finite() && s > 0.0 && s <= 4.0) {
                return Err(format!("a time scale of {s}: between 0 and 4 (1, 0.5, 0.25)"));
            }
            submit(format!("host_timescale {s}"));
        }
        Condition::ThirdPerson(on) => submit(if on { "thirdperson" } else { "firstperson" }.into()),
        Condition::CameraDistance(d) => {
            if !(d.is_finite() && d >= 0.0) {
                return Err(format!("a camera distance of {d}"));
            }
            submit(format!("cam_idealdist {d}"));
        }
        Condition::EndlessAmmo(_) | Condition::BotsFrozen(_) => {}
    }
    let mut s = world.get_resource_or_init::<SandboxConditions>();
    let was = *s;
    match c {
        Condition::EndlessAmmo(on) => s.endless_ammo = on,
        Condition::BotsFrozen(on) => s.bots_frozen = on,
        // Until the client's state says otherwise (`keep`, the app's
        // camera mirror).
        Condition::ThirdPerson(on) => s.third_person = on,
        Condition::CameraDistance(d) => s.camera_distance = d,
        Condition::God(_) | Condition::TimeScale(_) => {}
    }
    if *s != was {
        info!("{}", s.line());
    }
    Ok(format!("{c:?}"))
}

fn parse(a: &[String]) -> Result<Condition, String> {
    let name = a.first().map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
    let v = a.get(1).map(|s| s.to_ascii_lowercase()).ok_or(USAGE)?;
    let on = || match v.as_str() {
        "on" | "1" | "true" => Ok(true),
        "off" | "0" | "false" => Ok(false),
        _ => Err(format!("{v:?}: on or off")),
    };
    let num = || v.parse::<f32>().map_err(|_| format!("{v:?}: a number"));
    Ok(match name.as_str() {
        "god" | "nodamage" => Condition::God(on()?),
        "ammo" | "endless_ammo" => Condition::EndlessAmmo(on()?),
        "timescale" | "time_scale" | "host_timescale" => Condition::TimeScale(num()?),
        "thirdperson" | "third_person" => Condition::ThirdPerson(on()?),
        "camdist" | "camera_distance" | "cam_idealdist" => Condition::CameraDistance(num()?),
        "freeze" | "frozen" | "bots_frozen" => Condition::BotsFrozen(on()?),
        _ => return Err(USAGE.into()),
    })
}

/// The console's words.
pub const USAGE: &str = "condition <god on|off | ammo on|off | timescale 1|0.5|0.25 | thirdperson on|off | camdist <units> | \
    freeze on|off> | condition list";

/// `condition ...` (and the app's `sandbox <condition> <value>`).
pub fn command(world: &mut World, a: &[String]) -> Result<Option<String>, String> {
    if a.first().is_some_and(|w| w.eq_ignore_ascii_case("list")) {
        return Ok(Some(world.get_resource::<SandboxConditions>().copied().unwrap_or_default().line()));
    }
    let c = parse(a)?;
    set(world, c).map(Some)
}

fn apply_requests(mut asks: MessageReader<SetCondition>, mut commands: Commands) {
    for r in asks.read().copied() {
        commands.queue(move |world: &mut World| {
            if let Err(reason) = set(world, r.0) {
                warn!("condition: {reason}");
                world.write_message(ConditionRefused { reason });
            }
        });
    }
}

/// Each frame: the state from what each condition is (god, the time
/// scale), the switches back to their defaults when another minigame
/// starts, a change logged.
fn keep(
    mut s: ResMut<SandboxConditions>,
    me: Query<Has<God>, With<LocalPlayer>>,
    time: Option<Res<Time<Virtual>>>,
    active: Option<Res<ActiveMinigame>>,
    mut playing: Local<Option<String>>,
) {
    let id = active.map(|a| a.def.id.as_str());
    let mut next = *s;
    if playing.as_deref() != id {
        *playing = id.map(String::from);
        next.endless_ammo = true;
        next.bots_frozen = false;
    }
    if let Some(god) = me.iter().next() {
        next.god = god;
    }
    if let Some(time) = time {
        next.time_scale = if time.is_paused() { 0.0 } else { time.relative_speed() };
    }
    if next != *s {
        info!("{}", next.line());
        *s = next;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SandboxConditions>()
        .add_message::<SetCondition>()
        .add_message::<ConditionRefused>()
        .add_systems(Update, (apply_requests, keep))
        .console_command("chimera:condition", &format!("The sandbox's conditions: {USAGE}"), command);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_words_parse() {
        let w = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(parse(&w("god on")), Ok(Condition::God(true)));
        assert_eq!(parse(&w("ammo off")), Ok(Condition::EndlessAmmo(false)));
        assert_eq!(parse(&w("timescale 0.25")), Ok(Condition::TimeScale(0.25)));
        assert_eq!(parse(&w("thirdperson 1")), Ok(Condition::ThirdPerson(true)));
        assert_eq!(parse(&w("camdist 120")), Ok(Condition::CameraDistance(120.0)));
        assert_eq!(parse(&w("freeze on")), Ok(Condition::BotsFrozen(true)));
        assert!(parse(&w("spread off")).is_err());
    }
}
