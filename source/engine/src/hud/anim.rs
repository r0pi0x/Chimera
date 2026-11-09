//! Source's HUD animation scripts (`scripts/hudanimations.txt`;
//! specs/hl2dm/hud.md 3, shared by every Source game's HUD): a script is
//! a list of named events, each a list of commands; an `Animator` runs
//! them over the variables of named panels (`HudHealth` `FgColor`).
//!
//! - `Script::parse`: the file's events. Colour targets given as scheme
//!   colour names are resolved at load (`colour`), positions and sizes
//!   keep their `r`/`c` anchors until an animation starts.
//! - `Animator`: `start` an event (it first removes what the same event
//!   started earlier), `update` each frame (queued commands due by now
//!   run, then every running animation applies in the order started), `get`
//!   a variable. An animation takes as its start value what the variable
//!   holds on the first frame at or after its start time; Linear, Accel
//!   and Deaccel interpolators; colours interpolate per channel and are
//!   truncated to whole numbers when applied; panel and variable names
//!   match ignoring case. `StopAnimation` removes only the first running
//!   animation of the panel's variable that belongs to a different event.
//!
//! A game's HUD owns one `Animator`, sets the variables its elements
//! start from (`set`), starts events by name as its elements' state
//! changes and reads the variables when drawing.

use bevy::platform::collections::HashMap;

use chimera_world::map::hud::HudCoord;
use source_formats::keyvalues::{condition_holds, tokens, Escapes};

/// How an animation moves from its start value to its target: with x the
/// share of the time done, Linear x, Accel x squared, Deaccel its root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Linear,
    Accel,
    Deaccel,
}

impl Interp {
    fn of(name: &str) -> Interp {
        match name.to_ascii_lowercase().as_str() {
            "accel" => Interp::Accel,
            "deaccel" => Interp::Deaccel,
            _ => Interp::Linear,
        }
    }

    pub fn apply(self, x: f32) -> f32 {
        match self {
            Interp::Linear => x,
            Interp::Accel => x * x,
            Interp::Deaccel => x.sqrt(),
        }
    }
}

/// A variable's value: a number (`Blur`, `Alpha`), a colour (RGBA 0..255,
/// kept fractional while it moves) or a pair (a position or size, in
/// units, the left edge `x` counted from the screen's left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Number(f32),
    Colour([f32; 4]),
    Pair([f32; 2]),
}

impl Value {
    pub fn colour(c: [u8; 4]) -> Value {
        Value::Colour(c.map(f32::from))
    }

    /// The colour as written to a panel: whole numbers.
    pub fn rgba(&self) -> Option<[u8; 4]> {
        match self {
            Value::Colour(c) => Some(c.map(|v| v.clamp(0.0, 255.0) as u8)),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f32> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn pair(&self) -> Option<[f32; 2]> {
        match self {
            Value::Pair(p) => Some(*p),
            _ => None,
        }
    }

    fn mix(&self, to: &Value, k: f32) -> Value {
        let lerp = |a: f32, b: f32| a + (b - a) * k;
        match (self, to) {
            (Value::Number(a), Value::Number(b)) => Value::Number(lerp(*a, *b)),
            (Value::Colour(a), Value::Colour(b)) => Value::Colour([lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2]), lerp(a[3], b[3])]),
            (Value::Pair(a), Value::Pair(b)) => Value::Pair([lerp(a[0], b[0]), lerp(a[1], b[1])]),
            // A different kind: the target at once.
            _ => *to,
        }
    }
}

/// What an `Animate` command moves a variable to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Number(f32),
    Colour([f32; 4]),
    /// Position or size, with its anchors, resolved when the animation
    /// starts.
    Pair([HudCoord; 2]),
}

/// One command of an event.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Animate {
        panel: String,
        var: String,
        target: Target,
        interp: Interp,
        start: f32,
        duration: f32,
    },
    RunEvent { event: String, at: f32 },
    StopEvent { event: String, at: f32 },
    StopAnimation { panel: String, var: String, at: f32 },
    StopPanelAnimations { panel: String, at: f32 },
}

/// A script's events by lower-case name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Script {
    pub events: HashMap<String, Vec<Command>>,
}

fn is_pair_var(var: &str) -> bool {
    matches!(var, "position" | "size")
}

impl Script {
    /// The events in `text`. `colour` resolves a scheme colour name
    /// (`BrightFg`) to RGBA.
    pub fn parse(text: &str, colour: &dyn Fn(&str) -> Option<[u8; 4]>) -> Script {
        let mut events: HashMap<String, Vec<Command>> = HashMap::new();
        let mut current: Option<String> = None;
        let mut depth = 0;
        // Braces on lines of their own, wherever the file puts them.
        let text = text.replace('{', "\n{\n").replace('}', "\n}\n");
        for line in text.lines() {
            let line = line.split("//").next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut t = tokens(line, Escapes::Off);
            // A platform condition at the end of the line.
            if t.last().is_some_and(|s| s.starts_with('[') && s.ends_with(']')) {
                let c = t.pop().unwrap_or_default();
                if !condition_holds(&c) {
                    // The line is dropped; a block opener still counts.
                    if t.first().is_some_and(|s| s.eq_ignore_ascii_case("event")) {
                        current = None;
                    }
                    continue;
                }
            }
            let Some(first) = t.first().cloned() else { continue };
            match first.as_str() {
                "{" => depth += 1,
                "}" => {
                    depth -= 1;
                    if depth <= 0 {
                        current = None;
                        depth = 0;
                    }
                }
                _ if first.eq_ignore_ascii_case("event") && t.len() >= 2 => {
                    current = Some(t[1].to_ascii_lowercase());
                    events.entry(t[1].to_ascii_lowercase()).or_default();
                }
                _ => {
                    let Some(name) = &current else { continue };
                    if let Some(c) = command(&t, colour) {
                        events.entry(name.clone()).or_default().push(c);
                    }
                }
            }
        }
        Script { events }
    }
}

fn command(t: &[String], colour: &dyn Fn(&str) -> Option<[u8; 4]>) -> Option<Command> {
    let num = |i: usize| t.get(i).and_then(|s| s.trim().parse::<f32>().ok());
    let name = |i: usize| t.get(i).map(|s| s.to_ascii_lowercase());
    match t.first()?.to_ascii_lowercase().as_str() {
        "animate" => {
            let (panel, var) = (name(1)?, name(2)?);
            let raw = t.get(3)?;
            // What the variable holds decides: a position or size is a
            // pair, a single number a number, three or four numbers (or a
            // scheme colour's name) a colour.
            let parts: Vec<f32> = raw.split_whitespace().filter_map(|p| p.parse().ok()).collect();
            let words = raw.split_whitespace().count();
            let target = if is_pair_var(&var) {
                let mut p = raw.split_whitespace().map(HudCoord::parse);
                Target::Pair([p.next()??, p.next()??])
            } else if words == 1 && parts.len() == 1 {
                Target::Number(parts[0])
            } else {
                match parts.as_slice() {
                    [r, g, b, a] if words == 4 => Target::Colour([*r, *g, *b, *a]),
                    [r, g, b] if words == 3 => Target::Colour([*r, *g, *b, 255.0]),
                    _ => Target::Colour(colour(raw)?.map(f32::from)),
                }
            };
            let interp = Interp::of(t.get(4)?);
            // `start` and `duration` are the last two numbers (an
            // interpolator may carry a parameter before them).
            let n = t.len();
            let (start, duration) = (num(n.checked_sub(2)?)?, num(n - 1)?);
            Some(Command::Animate { panel, var, target, interp, start, duration })
        }
        "runevent" => Some(Command::RunEvent { event: name(1)?, at: num(2).unwrap_or(0.0) }),
        "stopevent" => Some(Command::StopEvent { event: name(1)?, at: num(2).unwrap_or(0.0) }),
        "stopanimation" => Some(Command::StopAnimation { panel: name(1)?, var: name(2)?, at: num(3).unwrap_or(0.0) }),
        "stoppanelanimations" => Some(Command::StopPanelAnimations { panel: name(1)?, at: num(2).unwrap_or(0.0) }),
        _ => None,
    }
}

/// An animation under way.
#[derive(Clone, Debug)]
struct Running {
    event: String,
    panel: String,
    var: String,
    target: Value,
    interp: Interp,
    begin: f64,
    duration: f32,
    from: Option<Value>,
}

/// A command waiting for its time.
#[derive(Clone, Debug)]
struct Queued {
    event: String,
    at: f64,
    command: Command,
}

/// The variables of a HUD's panels and the animations moving them.
#[derive(Clone, Debug, Default)]
pub struct Animator {
    pub script: Script,
    vars: HashMap<(String, String), Value>,
    running: Vec<Running>,
    queued: Vec<Queued>,
    /// The virtual screen's width in units (a position `r150` is this
    /// minus 150).
    pub screen_units: f32,
}

impl Animator {
    pub fn new(script: Script, screen_units: f32) -> Animator {
        Animator { script, screen_units, ..Animator::default() }
    }

    /// Set a variable now (what an element starts from).
    pub fn set(&mut self, panel: &str, var: &str, value: Value) {
        self.vars.insert((panel.to_ascii_lowercase(), var.to_ascii_lowercase()), value);
    }

    /// A variable's current value.
    pub fn get(&self, panel: &str, var: &str) -> Option<Value> {
        self.vars.get(&(panel.to_ascii_lowercase(), var.to_ascii_lowercase())).copied()
    }

    pub fn number(&self, panel: &str, var: &str) -> Option<f32> {
        self.get(panel, var).and_then(|v| v.number())
    }

    pub fn rgba(&self, panel: &str, var: &str) -> Option<[u8; 4]> {
        self.get(panel, var).and_then(|v| v.rgba())
    }

    /// Whether any animation or queued command is left.
    pub fn busy(&self) -> bool {
        !self.running.is_empty() || !self.queued.is_empty()
    }

    /// Whether `event` has animations or commands under way.
    pub fn is_running(&self, event: &str) -> bool {
        let e = event.to_ascii_lowercase();
        self.running.iter().any(|r| r.event == e) || self.queued.iter().any(|q| q.event == e)
    }

    /// Start `event` at `now`: what it started before is removed first. An
    /// event the script lacks does nothing.
    pub fn start(&mut self, event: &str, now: f64) {
        let name = event.to_ascii_lowercase();
        let Some(commands) = self.script.events.get(&name).cloned() else { return };
        self.running.retain(|r| r.event != name);
        self.queued.retain(|q| q.event != name);
        for c in commands {
            match c {
                Command::Animate { panel, var, target, interp, start, duration } => {
                    let target = match target {
                        Target::Number(n) => Value::Number(n),
                        Target::Colour(c) => Value::Colour(c),
                        Target::Pair([x, y]) => {
                            let w = self.screen_units;
                            Value::Pair([x.resolve(w, 1.0), y.resolve(480.0, 1.0)])
                        }
                    };
                    self.running.push(Running {
                        event: name.clone(),
                        panel,
                        var,
                        target,
                        interp,
                        begin: now + f64::from(start),
                        duration,
                        from: None,
                    });
                }
                Command::RunEvent { at, .. }
                | Command::StopEvent { at, .. }
                | Command::StopAnimation { at, .. }
                | Command::StopPanelAnimations { at, .. } => {
                    self.queued.push(Queued { event: name.clone(), at: now + f64::from(at), command: c });
                }
            }
        }
    }

    /// Stop every animation and queued command of `event`.
    pub fn stop_event(&mut self, event: &str) {
        let e = event.to_ascii_lowercase();
        self.running.retain(|r| r.event != e);
        self.queued.retain(|q| q.event != e);
    }

    /// Cancel everything and run each animation to its end at once (the
    /// spawn's reset): the running ones' targets are set, queued commands
    /// dropped.
    pub fn finish_all(&mut self) {
        self.queued.clear();
        for r in std::mem::take(&mut self.running) {
            self.vars.insert((r.panel, r.var), r.target);
        }
    }

    /// Cancel everything without moving the variables.
    pub fn cancel_all(&mut self) {
        self.queued.clear();
        self.running.clear();
    }

    /// One frame: queued commands due by `now` run (in the order queued),
    /// then every running animation applies in the order started.
    pub fn update(&mut self, now: f64) {
        let mut i = 0;
        while i < self.queued.len() {
            if self.queued[i].at <= now {
                let q = self.queued.remove(i);
                self.run(q, now);
                // A command may have queued or removed others.
                i = 0;
            } else {
                i += 1;
            }
        }
        let mut done = Vec::new();
        for (n, r) in self.running.iter_mut().enumerate() {
            if now < r.begin {
                continue;
            }
            let key = (r.panel.clone(), r.var.clone());
            let from = *r.from.get_or_insert_with(|| self.vars.get(&key).copied().unwrap_or(r.target));
            let x = if r.duration <= 0.0 { 1.0 } else { ((now - r.begin) as f32 / r.duration).clamp(0.0, 1.0) };
            let value = if x >= 1.0 { r.target } else { from.mix(&r.target, r.interp.apply(x)) };
            self.vars.insert(key, value);
            if x >= 1.0 {
                done.push(n);
            }
        }
        for n in done.into_iter().rev() {
            self.running.remove(n);
        }
    }

    fn run(&mut self, q: Queued, now: f64) {
        match q.command {
            Command::RunEvent { event, .. } => self.start(&event, now),
            Command::StopEvent { event, .. } => self.stop_event(&event),
            Command::StopAnimation { panel, var, .. } => {
                if let Some(n) = self.running.iter().position(|r| r.panel == panel && r.var == var && r.event != q.event) {
                    self.running.remove(n);
                }
            }
            // Not the stopping event's own: an event that stops a panel and
            // animates it in one list (hud.md 4.6's `WeaponUsesSecondaryAmmo`)
            // must keep what it just started, or the panel never shows.
            Command::StopPanelAnimations { panel, .. } => self.running.retain(|r| r.panel != panel || r.event == q.event),
            Command::Animate { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours(name: &str) -> Option<[u8; 4]> {
        match name {
            "FgColor" => Some([255, 220, 0, 100]),
            "BrightFg" => Some([255, 220, 0, 255]),
            _ => None,
        }
    }

    /// `StopPanelAnimations` stops others' animations of the panel, not the
    /// ones its own event started.
    #[test]
    fn a_panel_stop_keeps_its_own_events_animations() {
        let script = Script::parse(
            "event Other\n{\n\tAnimate P Blur \"9\" Linear 0 10\n}\nevent Show\n{\n\tStopPanelAnimations P 0\n\tAnimate P Alpha \"255\" Linear 0 0.1\n}\n",
            &colours,
        );
        let mut a = Animator::new(script, 640.0);
        a.set("P", "Alpha", Value::Number(0.0));
        a.set("P", "Blur", Value::Number(0.0));
        a.start("Other", 0.0);
        a.update(0.0);
        a.update(1.0);
        let blur = a.number("P", "Blur");
        a.start("Show", 1.0);
        a.update(1.0);
        a.update(1.2);
        assert_eq!(a.number("P", "Alpha"), Some(255.0), "its own animation ran");
        a.update(5.0);
        assert_eq!(a.number("P", "Blur"), blur, "the other event's stopped");
    }

    const SCRIPT: &str = r#"
event HealthDamageTaken
{
	Animate HudHealth	FgColor		"BrightFg"	Linear	0.0		0.25
	Animate HudHealth	FgColor		"FgColor"	Linear	0.3		0.75
	Animate HudHealth	Blur		"3"		Linear	0.0		0.1
	Animate HudHealth	Blur		"0"		Deaccel	0.1		2.0
}

event Slide
{
	Animate HudAmmo		Position	"r150 432"	Deaccel	0.0		0.4
	Animate HudAmmo		Size		"132 36"	Deaccel	0.0		0.4
	RunEvent Slide2 1.0
	StopAnimation HudAmmo Blur 0.5
}

event Slide2
{
	Animate HudAmmo	Alpha "5" Accel 0 1
}
"#;

    fn anim() -> Animator {
        let mut a = Animator::new(Script::parse(SCRIPT, &colours), 640.0);
        a.set("HudHealth", "FgColor", Value::colour([255, 220, 0, 100]));
        a
    }

    #[test]
    fn the_script_reads_its_events_and_resolves_colour_names() {
        let s = Script::parse(SCRIPT, &colours);
        assert_eq!(s.events.len(), 3);
        let e = &s.events["healthdamagetaken"];
        assert_eq!(e.len(), 4);
        assert_eq!(
            e[0],
            Command::Animate {
                panel: "hudhealth".into(),
                var: "fgcolor".into(),
                target: Target::Colour([255.0, 220.0, 0.0, 255.0]),
                interp: Interp::Linear,
                start: 0.0,
                duration: 0.25
            }
        );
        // Positions keep their anchors.
        let Command::Animate { target, .. } = &s.events["slide"][0] else { panic!() };
        assert_eq!(*target, Target::Pair([HudCoord::End(150.0), HudCoord::Start(432.0)]));
    }

    /// Spec test 5: a Deaccel animation 0 to 200 over 1 s is 100 at 0.25
    /// s; Accel is 12.5 (12 whole).
    #[test]
    fn interpolators() {
        let s = Script::parse("event A { Animate P Alpha \"200\" Deaccel 0 1 }\nevent B { Animate P Alpha \"200\" Accel 0 1 }", &colours);
        for (event, want) in [("A", 100.0), ("B", 12.5)] {
            let mut a = Animator::new(s.clone(), 640.0);
            a.set("P", "Alpha", Value::Number(0.0));
            a.start(event, 0.0);
            a.update(0.0);
            a.update(0.25);
            assert_eq!(a.number("P", "Alpha"), Some(want), "{event}");
        }
    }

    /// Spec test 6: the damage flash goes to `BrightFg` over 0.25 s and
    /// back; halfway back it is 255 - 77.5 -> 177 alpha.
    #[test]
    fn a_flash_goes_up_and_comes_back() {
        let mut a = anim();
        a.start("HealthDamageTaken", 10.0);
        a.update(10.0);
        a.update(10.25);
        assert_eq!(a.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 255]));
        a.update(10.675);
        assert_eq!(a.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 177]));
        a.update(12.2);
        assert_eq!(a.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 100]));
        // Done: nothing is left running.
        assert!(!a.busy());
    }

    /// Spec test 7: an event started again drops the first run's
    /// animations and queued commands.
    #[test]
    fn restarting_an_event_replaces_its_first_run() {
        let mut a = anim();
        a.start("Slide", 0.0);
        a.update(0.2);
        a.start("Slide", 0.2);
        a.update(0.2);
        let before = a.running.len();
        // Two animations and the two queued commands, once each.
        assert_eq!(before, 2);
        assert_eq!(a.queued.len(), 2);
        // The queued RunEvent fires at 1.2, not at 1.0.
        a.update(1.1);
        assert!(!a.is_running("Slide2"));
        a.update(1.2);
        assert!(a.is_running("Slide2"));
    }

    #[test]
    fn positions_resolve_against_the_screen_width() {
        let mut a = anim();
        a.start("Slide", 0.0);
        a.update(0.0);
        a.update(0.4);
        assert_eq!(a.get("HudAmmo", "Position"), Some(Value::Pair([490.0, 432.0])));
        assert_eq!(a.get("HudAmmo", "Size"), Some(Value::Pair([132.0, 36.0])));
    }

    #[test]
    fn a_start_value_is_whatever_the_variable_holds_then() {
        // The second line starts where the first ended.
        let mut a = anim();
        a.start("HealthDamageTaken", 0.0);
        a.update(0.0);
        a.update(0.3);
        let at_third = a.number("HudHealth", "Blur").unwrap();
        assert!(at_third > 0.0 && at_third < 3.0, "{at_third}");
        // A variable nobody set starts from its target.
        let mut b = Animator::new(Script::parse("event E { Animate P Alpha \"9\" Linear 0 1 }", &colours), 640.0);
        b.start("E", 0.0);
        b.update(0.5);
        assert_eq!(b.number("P", "Alpha"), Some(9.0));
    }

    #[test]
    fn stop_animation_takes_only_another_events() {
        let s = Script::parse(
            "event A { Animate P Blur \"1\" Linear 0 5 }\nevent B { Animate P Blur \"2\" Linear 0 5\n StopAnimation P Blur 0 }",
            &colours,
        );
        let mut a = Animator::new(s, 640.0);
        a.start("A", 0.0);
        a.start("B", 0.0);
        a.update(0.0);
        // B's command removed A's animation, not B's own.
        assert_eq!(a.running.len(), 1);
        assert_eq!(a.running[0].event, "b");
    }

    #[test]
    fn an_event_the_script_lacks_does_nothing() {
        let mut a = anim();
        a.start("HintMessageLower", 0.0);
        assert!(!a.busy());
    }

    #[test]
    fn platform_conditions_drop_their_lines() {
        let s = Script::parse(
            "event A\n{\n Animate P Blur \"1\" Linear 0 1 [$WIN32]\n Animate P Blur \"2\" Linear 0 1 [$DECK]\n}",
            &colours,
        );
        assert_eq!(s.events["a"].len(), 1);
    }

    #[test]
    fn the_spawn_reset_runs_everything_to_its_end() {
        let mut a = anim();
        a.start("HealthDamageTaken", 0.0);
        a.update(0.1);
        a.finish_all();
        assert_eq!(a.rgba("HudHealth", "FgColor"), Some([255, 220, 0, 100]));
        assert_eq!(a.number("HudHealth", "Blur"), Some(0.0));
        assert!(!a.busy());
    }
}
