//! The command-string language (specs/combat_arms/world_objects.md 13):
//! what map objects store in their command properties, key commands and
//! `msg` arguments, run by the server's command manager.
//!
//! - Syntax: statements separated by `;`, words by whitespace;
//!   parentheses (and double quotes) group words into one argument, the
//!   outer parentheses removed when used (`tokens`). Command words are
//!   case-insensitive with exact argument counts (`ARG_COUNTS`); a
//!   string whose first statement isn't valid is ignored whole (also the
//!   bodies of `delay`, `repeat`, `loop`, `rand`), a later statement with
//!   the wrong count is skipped silently, one that fails
//!   (an unknown variable, a bad delay or `rand` chance, ...) stops the
//!   rest of its string (Quirk 11).
//! - `run` executes a string against a `CommandHost` (the object world:
//!   it delivers `msg`s synchronously, names objects, rolls dice).
//! - `CommandState` is the manager's state: integer and object variables
//!   (512), pending `when`s (32) and the delayed, repeating and looping
//!   commands (512 slots, each counted down by the frame's real time and
//!   run in slot order: `tick`). A command queued while the table is full
//!   runs at once; a delayed one longer than 255 bytes is cut; ids keep
//!   15 characters.
//!
//! A run that finishes inside a statement clears the outer string's
//! sender and default target for what follows (world_objects.md
//! Corrections (2026-10-22) 8); `tick` reaches commands queued into a later
//! slot during its walk (13).
//!
//! Known shortcuts: an ignored (invalid) string doesn't count as a finished
//! run for the context clearing, and `IF`/`WHEN` bodies are checked like any
//! string (our readings); an unknown command word is skipped like a wrong
//! argument count (the spec names only the failures above); `SHOWVAR` and
//! `LISTCOMMANDS` write to the host's log (no on-screen display); nested
//! runs deeper than `MAX_DEPTH` stop (ours: a map whose objects message
//! each other in a loop would otherwise never return).

use std::collections::HashMap;

/// Pending delayed, repeating and looping commands at once (cmd_pending_max).
pub const PENDING_MAX: usize = 512;
/// Longest stored delayed command, bytes (cmd_length_max): 255 kept.
pub const LENGTH_MAX: usize = 256;
/// Longest command id (cmd_id_max): 15 characters kept.
pub const ID_MAX: usize = 16;
/// Command-manager variables (cmd_vars_max).
pub const VARS_MAX: usize = 512;
/// Pending `when` conditions (cmd_when_max).
pub const WHEN_MAX: usize = 32;
/// Ours: how deep commands may run inside each other's deliveries.
pub const MAX_DEPTH: u32 = 64;

/// An object of the world running the commands (its index).
pub type ObjId = u32;

/// Who a command acts for: its sender.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Who {
    #[default]
    Nobody,
    /// A map object.
    Object(ObjId),
    /// A character (a player or a bot): its entity's bits.
    Character(u64),
}

/// The sender and default target a command string runs with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ctx {
    pub sender: Who,
    pub target: Option<ObjId>,
    /// A character as the default target (a trigger's `CommandTouch`: the
    /// toucher; Corrections (2026-10-22) 9).
    pub target_char: Who,
}

/// What a command manager needs of the world around it.
pub trait CommandHost {
    fn commands(&mut self) -> &mut CommandState;
    /// Deliver `message` now to every target `targets` names (one name or
    /// several, whitespace-separated; `player`, `ActivePlayer`,
    /// `OtherPlayers`, `ai`, `Sender`, empty for the default target).
    fn send(&mut self, targets: &str, message: &str, ctx: Ctx);
    /// Deliver `message` to one object.
    fn send_to(&mut self, object: ObjId, message: &str, ctx: Ctx);
    /// The first object named `name`.
    fn named(&self, name: &str) -> Option<ObjId>;
    /// Uniform in [0, 1).
    fn random(&mut self) -> f32;
    /// A debug line (`SHOWVAR`, `LISTCOMMANDS`).
    fn log(&mut self, _line: String) {}
}

/// One argument of a statement: its text (a group without its outer
/// parentheses or quotes) and whether it was a group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub group: bool,
}

/// `s`'s top-level statements (split on `;` outside groups), trimmed,
/// empty ones left out.
pub fn statements(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quoted = false;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ';' if depth <= 0 && !quoted => {
                let t = cur.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

/// A statement's arguments: whitespace-separated words, a parenthesised
/// or quoted group one argument (its outer parentheses or quotes removed).
pub fn tokens(s: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '(' {
            let mut depth = 0;
            let start = i + 1;
            let mut quoted = false;
            while i < chars.len() {
                match chars[i] {
                    '"' => quoted = !quoted,
                    '(' if !quoted => depth += 1,
                    ')' if !quoted => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let end = i.min(chars.len());
            out.push(Token {
                text: chars[start..end].iter().collect::<String>().trim().to_string(),
                group: true,
            });
            i = end + 1;
            continue;
        }
        if c == '"' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            out.push(Token {
                text: chars[start..i.min(chars.len())].iter().collect(),
                group: true,
            });
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != '"' {
            i += 1;
        }
        out.push(Token {
            text: chars[start..i].iter().collect(),
            group: false,
        });
    }
    out
}

/// The command words and their argument counts (the word included);
/// `RAND2`..`RAND8` take N + 1.
pub const ARG_COUNTS: &[(&str, usize)] = &[
    ("MSG", 3),
    ("VMSG", 3),
    ("DELAY", 3),
    ("DELAYID", 4),
    ("DELAYVARIABLE", 3),
    ("REPEAT", 6),
    ("REPEATID", 7),
    ("LOOP", 4),
    ("LOOPID", 5),
    ("ABORT", 2),
    ("RAND", 4),
    ("INT", 3),
    ("OBJ", 3),
    ("SET", 3),
    ("ADD", 3),
    ("SUB", 3),
    ("IF", 4),
    ("WHEN", 4),
    ("SHOWVAR", 3),
    ("LISTCOMMANDS", 1),
];

/// A command word's argument count, None when it isn't one.
pub fn arg_count(word: &str) -> Option<usize> {
    let w = word.to_ascii_uppercase();
    if let Some(n) = w.strip_prefix("RAND").and_then(|n| n.parse::<usize>().ok())
        && (2..=8).contains(&n)
    {
        return Some(n + 1);
    }
    ARG_COUNTS.iter().find(|(c, _)| *c == w).map(|(_, n)| *n)
}

/// Whether `s` starts with a command word.
pub fn is_command(s: &str) -> bool {
    tokens(s).first().is_some_and(|t| !t.group && arg_count(&t.text).is_some())
}

/// A variable's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Int(i32),
    Obj(Option<ObjId>),
}

/// A stored `when`.
#[derive(Clone, Debug, PartialEq)]
pub struct When {
    pub expression: String,
    pub commands: String,
    /// The variables it reads (lower case).
    pub vars: Vec<String>,
    pub ctx: Ctx,
}

/// How a pending command repeats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Repeats {
    Once,
    /// Runs left after this one, and the wait range.
    Times(u32, f32, f32),
    Forever(f32, f32),
}

/// A delayed, repeating or looping command.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub id: Option<String>,
    pub delay: f32,
    pub command: String,
    pub ctx: Ctx,
    pub repeats: Repeats,
    /// The tick count when it was queued: not counted down in that tick.
    pub born: u64,
}

/// The command manager's state.
#[derive(Clone, Debug)]
pub struct CommandState {
    pub vars: HashMap<String, Var>,
    pub whens: Vec<When>,
    pub pending: Vec<Option<Pending>>,
    pub ticks: u64,
    pub depth: u32,
    /// How many command-string runs have finished (a run that finishes
    /// inside a statement clears the outer string's sender and target:
    /// Corrections (2026-10-22) 8).
    pub finished: u64,
    /// The slot whose command the manager is running (a command queued
    /// meanwhile takes another slot: Corrections (2026-10-22) 13).
    pub running: Option<usize>,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            vars: HashMap::new(),
            whens: Vec::new(),
            pending: vec![None; PENDING_MAX],
            ticks: 0,
            depth: 0,
            finished: 0,
            running: None,
        }
    }
}

impl CommandState {
    /// An integer variable's value.
    pub fn int(&self, name: &str) -> Option<i32> {
        match self.vars.get(&name.to_ascii_lowercase()) {
            Some(Var::Int(v)) => Some(*v),
            _ => None,
        }
    }

    /// How many commands are pending.
    pub fn pending_count(&self) -> usize {
        self.pending.iter().flatten().count()
    }
}

/// Why a statement failed (it stops the rest of its string).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure(pub String);

fn fail<T>(why: impl Into<String>) -> Result<T, Failure> {
    Err(Failure(why.into()))
}

/// Run the command string `text` with `ctx`: each statement in turn,
/// stopping at the first that fails (its reason returned).
///
/// A string whose first statement isn't valid (its first word a command
/// word, its argument count exact) is ignored whole and fails
/// (Corrections (2026-10-22) 6). A run that finishes inside a statement
/// (a receiver's own commands, a `rand` branch, a message that is a
/// command) clears the sender and default target for the statements
/// after it (Corrections (2026-10-22) 8).
pub fn run(host: &mut impl CommandHost, text: &str, ctx: Ctx) -> Result<(), Failure> {
    if host.commands().depth >= MAX_DEPTH {
        return fail("commands nested too deep");
    }
    let list = statements(text);
    let Some(first) = list.first() else { return Ok(()) };
    if !valid_statement(first) {
        return fail(format!("not a command: {first}"));
    }
    host.commands().depth += 1;
    let mut ctx = ctx;
    let mut result = Ok(());
    for s in list {
        let before = host.commands().finished;
        let r = statement(host, &s, ctx);
        if host.commands().finished != before {
            ctx = Ctx::default();
        }
        if let Err(e) = r {
            result = Err(e);
            break;
        }
    }
    let state = host.commands();
    state.depth -= 1;
    state.finished += 1;
    result
}

/// Whether `s` (one statement) starts with a command word and has its
/// exact argument count.
fn valid_statement(s: &str) -> bool {
    let t = tokens(s);
    t.first().is_some_and(|w| !w.group && arg_count(&w.text) == Some(t.len()))
}

/// Whether the first statement of the stored command string `s` is valid
/// (the bodies of `delay`, `repeat`, `loop`: Corrections (2026-10-22) 6).
fn valid_string(s: &str) -> bool {
    statements(s).first().is_some_and(|f| valid_statement(f))
}

/// Number argument.
fn number(t: &Token) -> Result<f32, Failure> {
    t.text
        .trim()
        .parse::<f32>()
        .or_else(|_| fail(format!("not a number: {}", t.text)))
}

/// Integer argument (a number's whole part).
fn integer(t: &Token) -> Result<i32, Failure> {
    number(t).map(|x| x as i32)
}

/// The id of `DELAYID` and the others: 15 characters kept.
fn id(t: &Token) -> String {
    t.text.chars().take(ID_MAX - 1).collect()
}

/// Uniform in [a, b].
fn uniform(host: &mut impl CommandHost, a: f32, b: f32) -> f32 {
    a + (b - a) * host.random()
}

/// Uniform integer in a..=b.
fn uniform_int(host: &mut impl CommandHost, a: i32, b: i32) -> i32 {
    if b <= a {
        return a;
    }
    (a + (host.random() * (b - a + 1) as f32) as i32).min(b)
}

/// Queue `command` after `delay` s (at once when the table is full).
fn queue(
    host: &mut impl CommandHost,
    id: Option<String>,
    delay: f32,
    command: &str,
    ctx: Ctx,
    repeats: Repeats,
) -> Result<(), Failure> {
    if !valid_string(command) {
        return fail(format!("not a command: {command}"));
    }
    let mut command = command.to_string();
    if command.len() >= LENGTH_MAX {
        let mut cut = LENGTH_MAX - 1;
        while !command.is_char_boundary(cut) {
            cut -= 1;
        }
        command.truncate(cut);
    }
    let state = host.commands();
    let born = state.ticks;
    let running = state.running;
    match state.pending.iter_mut().enumerate().find(|(i, p)| p.is_none() && Some(*i) != running) {
        Some((_, slot)) => {
            *slot = Some(Pending {
                id,
                delay,
                command,
                ctx,
                repeats,
                born,
            });
            Ok(())
        }
        None => {
            // The table is full: it runs at once, and that counts as a
            // failure (Corrections (2026-10-22) 7).
            let _ = run(host, &command, ctx);
            fail("command table full")
        }
    }
}

/// One statement.
fn statement(host: &mut impl CommandHost, s: &str, ctx: Ctx) -> Result<(), Failure> {
    let t = tokens(s);
    let Some(word) = t.first().filter(|w| !w.group) else {
        return Ok(());
    };
    let Some(count) = arg_count(&word.text) else {
        return Ok(());
    };
    if t.len() != count {
        return Ok(());
    }
    let w = word.text.to_ascii_uppercase();
    match w.as_str() {
        "MSG" => {
            message(host, &t[1].text, &t[2].text, ctx);
            Ok(())
        }
        // An unknown, non-object or empty variable sends nothing and the
        // string goes on (Corrections (2026-10-22) 10).
        "VMSG" => {
            if let Some(Var::Obj(Some(o))) = host.commands().vars.get(&t[1].text.to_ascii_lowercase()).copied() {
                for part in statements(&t[2].text) {
                    host.send_to(o, &part, ctx);
                }
            }
            Ok(())
        }
        "DELAY" => {
            let d = number(&t[1])?;
            queue(host, None, d, &t[2].text, ctx, Repeats::Once)
        }
        "DELAYID" => {
            let d = number(&t[2])?;
            queue(host, Some(id(&t[1])), d, &t[3].text, ctx, Repeats::Once)
        }
        "DELAYVARIABLE" => {
            let Some(d) = host.commands().int(&t[1].text) else {
                return fail(format!("no integer variable {}", t[1].text));
            };
            queue(host, None, d as f32, &t[2].text, ctx, Repeats::Once)
        }
        "REPEAT" | "REPEATID" => {
            let (ident, a) = if w == "REPEATID" { (Some(id(&t[1])), &t[2..]) } else { (None, &t[1..]) };
            let (n0, n1, s0, s1) = (integer(&a[0])?, integer(&a[1])?, number(&a[2])?, number(&a[3])?);
            // A roll of 0 (or less) still runs once (Corrections
            // (2026-10-22) 12).
            let n = uniform_int(host, n0, n1).max(1);
            let wait = uniform(host, s0, s1);
            queue(host, ident, wait, &a[4].text, ctx, Repeats::Times(n as u32 - 1, s0, s1))
        }
        "LOOP" | "LOOPID" => {
            let (ident, a) = if w == "LOOPID" { (Some(id(&t[1])), &t[2..]) } else { (None, &t[1..]) };
            let (s0, s1) = (number(&a[0])?, number(&a[1])?);
            let wait = uniform(host, s0, s1);
            queue(host, ident, wait, &a[2].text, ctx, Repeats::Forever(s0, s1))
        }
        "ABORT" => {
            // Ids are kept to 15 characters but compared in full, so a
            // longer one is never found; nothing found fails
            // (Corrections (2026-10-22) 7).
            let want = &t[1].text;
            let state = host.commands();
            match state
                .pending
                .iter_mut()
                .find(|p| p.as_ref().is_some_and(|p| p.id.as_ref().is_some_and(|i| i.eq_ignore_ascii_case(want))))
            {
                Some(slot) => {
                    *slot = None;
                    Ok(())
                }
                None => fail(format!("nothing pending as {want}")),
            }
        }
        "RAND" => {
            let p = number(&t[1])?;
            if !(0.001..=1.0).contains(&p) {
                return fail(format!("rand chance out of range: {p}"));
            }
            let pick = if host.random() < p { &t[2] } else { &t[3] };
            // A failing branch fails the statement (Corrections
            // (2026-10-22) 11).
            let pick = pick.text.clone();
            run(host, &pick, ctx)
        }
        _ if w.starts_with("RAND") => {
            let n = t.len() - 1;
            let k = ((host.random() * n as f32) as usize).min(n - 1);
            run(host, &t[1 + k].text, ctx)
        }
        "INT" => {
            let name = t[1].text.to_ascii_lowercase();
            let state = host.commands();
            if state.vars.contains_key(&name) {
                return fail(format!("variable {name} exists"));
            }
            if state.vars.len() >= VARS_MAX {
                return fail("too many variables");
            }
            let v = value_int(state, &t[2].text)?;
            state.vars.insert(name, Var::Int(v));
            Ok(())
        }
        "OBJ" => {
            let name = t[1].text.to_ascii_lowercase();
            if host.commands().vars.contains_key(&name) {
                return fail(format!("variable {name} exists"));
            }
            if host.commands().vars.len() >= VARS_MAX {
                return fail("too many variables");
            }
            let o = host.named(&t[2].text);
            host.commands().vars.insert(name, Var::Obj(o));
            Ok(())
        }
        "SET" | "ADD" | "SUB" => {
            let name = t[1].text.to_ascii_lowercase();
            let current = host.commands().vars.get(&name).copied();
            let new = match (current, w.as_str()) {
                (None, _) => return fail(format!("no variable {name}")),
                (Some(Var::Int(_)), "SET") => {
                    if !t[2].text.starts_with(|c: char| c.is_ascii_digit()) {
                        match host.commands().vars.get(&t[2].text.to_ascii_lowercase()) {
                            Some(Var::Int(v)) => Var::Int(*v),
                            _ => return fail(format!("bad value {}", t[2].text)),
                        }
                    } else {
                        Var::Int(integer(&t[2])?)
                    }
                }
                (Some(Var::Obj(_)), "SET") => match host.commands().vars.get(&t[2].text.to_ascii_lowercase()) {
                    Some(Var::Obj(o)) => Var::Obj(*o),
                    _ => Var::Obj(host.named(&t[2].text)),
                },
                (Some(Var::Int(v)), "ADD") => Var::Int(v.wrapping_add(value_int(host.commands(), &t[2].text)?)),
                (Some(Var::Int(v)), _) => Var::Int(v.wrapping_sub(value_int(host.commands(), &t[2].text)?)),
                (Some(Var::Obj(_)), _) => return fail(format!("{name} is an object variable")),
            };
            host.commands().vars.insert(name.clone(), new);
            changed(host, &name);
            Ok(())
        }
        "IF" | "WHEN" => {
            if !t[2].text.eq_ignore_ascii_case("THEN") {
                return fail("IF without THEN");
            }
            let truth = evaluate(host.commands(), &t[1].text)?;
            if truth {
                run_then(host, &t[3], ctx);
            } else if w == "WHEN" {
                let state = host.commands();
                if state.whens.len() >= WHEN_MAX {
                    return fail("too many whens");
                }
                state.whens.push(When {
                    expression: t[1].text.clone(),
                    commands: t[3].text.clone(),
                    vars: names_in(&t[1].text),
                    ctx,
                });
            }
            Ok(())
        }
        "SHOWVAR" => {
            let name = t[1].text.to_ascii_lowercase();
            let v = host.commands().vars.get(&name).copied();
            host.log(format!("showvar {name} = {v:?}"));
            Ok(())
        }
        "LISTCOMMANDS" => {
            let lines: Vec<String> = host
                .commands()
                .pending
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.as_ref().map(|p| format!("[{i}] {:.2} s {:?}: {}", p.delay, p.id, p.command)))
                .collect();
            for l in lines {
                host.log(l);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The then-part of `IF`/`WHEN`: one command, or groups each a command.
fn run_then(host: &mut impl CommandHost, t: &Token, ctx: Ctx) {
    let parts = tokens(&t.text);
    if parts.len() > 1 && parts.iter().all(|p| p.group) {
        for p in parts {
            let _ = run(host, &p.text, ctx);
        }
    } else {
        let _ = run(host, &t.text, ctx);
    }
}

/// `MSG`: the message to the targets now; a message that is itself a
/// command runs as one with the first object of the target name as its
/// default target; several `;`-separated parts are sent in order.
pub fn message(host: &mut impl CommandHost, targets: &str, text: &str, ctx: Ctx) {
    if is_command(text) {
        let first = targets.split_whitespace().next().unwrap_or("");
        let target = host.named(first).or(ctx.target);
        let _ = run(host, text, Ctx { target, target_char: Who::Nobody, ..ctx });
        return;
    }
    for part in statements(text) {
        host.send(targets, &part, ctx);
    }
}

/// An integer value: a number (starting with a digit or a sign) or an
/// integer variable.
fn value_int(state: &CommandState, s: &str) -> Result<i32, Failure> {
    let s = s.trim();
    if s.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
        return s
            .parse::<f32>()
            .map(|v| v as i32)
            .or_else(|_| fail(format!("not a number: {s}")));
    }
    state.int(s).ok_or_else(|| Failure(format!("no integer variable {s}")))
}

/// The variable names an expression reads (lower case).
fn names_in(expression: &str) -> Vec<String> {
    let mut out = Vec::new();
    for t in tokens(expression) {
        if t.group {
            out.extend(names_in(&t.text));
            continue;
        }
        let w = t.text.to_ascii_uppercase();
        let op = matches!(
            w.as_str(),
            "==" | "!=" | ">" | "<" | ">=" | "<=" | "AND" | "OR" | "&&" | "||" | "EQUALS" | "NOT_EQUALS"
                | "GREATER_THAN" | "LESS_THAN" | "GREATER_THAN_OR_EQUAL_TO" | "LESS_THAN_OR_EQUAL_TO"
        );
        if !op && !t.text.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
            out.push(t.text.to_ascii_lowercase());
        }
    }
    out
}

/// Evaluate an expression: `a op b`, or parenthesised expressions joined
/// by `AND`/`&&`, `OR`/`||` (left to right).
pub fn evaluate(state: &CommandState, expression: &str) -> Result<bool, Failure> {
    let t = tokens(expression);
    if t.len() == 1 && t[0].group {
        return evaluate(state, &t[0].text);
    }
    let joins = |w: &str| matches!(w.to_ascii_uppercase().as_str(), "AND" | "&&" | "OR" | "||");
    if t.len() >= 3 && t.iter().skip(1).step_by(2).all(|w| !w.group && joins(&w.text)) && t[0].group {
        let mut acc = evaluate(state, &t[0].text)?;
        let mut i = 1;
        while i + 1 < t.len() {
            let rhs = evaluate(state, &t[i + 1].text)?;
            acc = match t[i].text.to_ascii_uppercase().as_str() {
                "AND" | "&&" => acc && rhs,
                _ => acc || rhs,
            };
            i += 2;
        }
        return Ok(acc);
    }
    if t.len() != 3 {
        return fail(format!("bad expression: {expression}"));
    }
    let op = t[1].text.to_ascii_uppercase();
    // Object variables compare by identity (== and != only).
    let obj = |s: &str| match state.vars.get(&s.to_ascii_lowercase()) {
        Some(Var::Obj(o)) => Some(*o),
        _ => None,
    };
    if let (Some(a), Some(b)) = (obj(&t[0].text), obj(&t[2].text)) {
        return match op.as_str() {
            "==" | "EQUALS" => Ok(a == b),
            "!=" | "NOT_EQUALS" => Ok(a != b),
            _ => fail("objects compare by == and != only"),
        };
    }
    let a = value_int(state, &t[0].text)?;
    let b = value_int(state, &t[2].text)?;
    Ok(match op.as_str() {
        "==" | "EQUALS" => a == b,
        "!=" | "NOT_EQUALS" => a != b,
        ">" | "GREATER_THAN" => a > b,
        "<" | "LESS_THAN" => a < b,
        ">=" | "GREATER_THAN_OR_EQUAL_TO" => a >= b,
        "<=" | "LESS_THAN_OR_EQUAL_TO" => a <= b,
        _ => return fail(format!("bad operator {op}")),
    })
}

/// A variable changed: the stored `when`s reading it that are now true
/// run once (and are dropped).
fn changed(host: &mut impl CommandHost, var: &str) {
    let mut i = 0;
    while i < host.commands().whens.len() {
        let (reads, expression) = {
            let w = &host.commands().whens[i];
            (w.vars.iter().any(|v| v == var), w.expression.clone())
        };
        if !reads {
            i += 1;
            continue;
        }
        if evaluate(host.commands(), &expression).unwrap_or(false) {
            let w = host.commands().whens.remove(i);
            run_then(
                host,
                &Token {
                    text: w.commands,
                    group: true,
                },
                w.ctx,
            );
        } else {
            i += 1;
        }
    }
}

/// One frame of the manager: every pending command's delay less the
/// frame's real time `dt`; those at 0 or less run (in slot order), then
/// repeat, loop or are freed. A command queued during the walk into a
/// later slot is reached in the same walk (so `delay 0` queued by a
/// running command runs this frame); the running command's own slot
/// stays taken while it runs (Corrections (2026-10-22) 13). Commands
/// queued outside the walk are first counted down in the next one.
pub fn tick(host: &mut impl CommandHost, dt: f32) {
    host.commands().ticks += 1;
    for i in 0..PENDING_MAX {
        let due = {
            let state = host.commands();
            let Some(p) = state.pending[i].as_mut() else { continue };
            p.delay -= dt;
            // (Float error aside: a delay of whole frames ends on its frame.)
            if p.delay > 1e-5 {
                continue;
            }
            (p.command.clone(), p.ctx, p.repeats)
        };
        let (command, ctx, repeats) = due;
        let once = match repeats {
            Repeats::Times(left, s0, s1) if left > 0 => {
                let wait = uniform(host, s0, s1);
                if let Some(p) = host.commands().pending[i].as_mut() {
                    p.delay = wait;
                    p.repeats = Repeats::Times(left - 1, s0, s1);
                }
                false
            }
            Repeats::Forever(s0, s1) => {
                let wait = uniform(host, s0, s1);
                if let Some(p) = host.commands().pending[i].as_mut() {
                    p.delay = wait;
                }
                false
            }
            _ => true,
        };
        host.commands().running = Some(i);
        let _ = run(host, &command, ctx);
        let state = host.commands();
        state.running = None;
        if once {
            state.pending[i] = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host that records deliveries.
    #[derive(Default)]
    struct Host {
        state: CommandState,
        sent: Vec<(String, String)>,
        names: Vec<String>,
        rolls: Vec<f32>,
    }

    impl CommandHost for Host {
        fn commands(&mut self) -> &mut CommandState {
            &mut self.state
        }
        fn send(&mut self, targets: &str, message: &str, _ctx: Ctx) {
            for t in targets.split_whitespace() {
                self.sent.push((t.to_string(), message.to_string()));
            }
        }
        fn send_to(&mut self, object: ObjId, message: &str, _ctx: Ctx) {
            self.sent.push((self.names[object as usize].clone(), message.to_string()));
        }
        fn named(&self, name: &str) -> Option<ObjId> {
            self.names.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|i| i as u32)
        }
        fn random(&mut self) -> f32 {
            if self.rolls.is_empty() { 0.5 } else { self.rolls.remove(0) }
        }
    }

    #[test]
    fn tokens_group_and_strip() {
        let t = tokens("msg (A B) (visible 0)");
        assert_eq!(t.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["msg", "A B", "visible 0"]);
        assert!(t[1].group && !t[0].group);
        assert_eq!(statements("a; (b; c); d"), ["a", "(b; c)", "d"]);
        let t = tokens("if ((Check == 0) AND (TRound == 1)) then (set TRound 0)");
        assert_eq!(t.len(), 4);
        assert_eq!(t[1].text, "(Check == 0) AND (TRound == 1)");
    }

    #[test]
    fn wrong_counts_are_skipped_and_failures_stop() {
        let mut h = Host::default();
        // A wrong count after the first statement: skipped, the rest runs.
        run(&mut h, "msg B on; msg A; msg C on", Ctx::default()).unwrap();
        assert_eq!(h.sent, [("B".to_string(), "on".to_string()), ("C".to_string(), "on".to_string())]);
        // Corrections (2026-10-22) 6: a wrong first statement ignores the
        // whole string.
        h.sent.clear();
        assert!(run(&mut h, "msg A; msg B on", Ctx::default()).is_err());
        assert!(run(&mut h, "mgs A on; msg B on", Ctx::default()).is_err());
        assert!(h.sent.is_empty());
        // Case 20: an undeclared variable stops the string.
        assert!(run(&mut h, "set Undeclared 1; msg A on", Ctx::default()).is_err());
        assert!(h.sent.is_empty());
        // `SET x -1` fails (a value must start with a digit).
        run(&mut h, "int x 0", Ctx::default()).unwrap();
        assert!(run(&mut h, "set x -1; msg A on", Ctx::default()).is_err());
        assert!(h.sent.is_empty());
    }

    #[test]
    fn rand_bounds() {
        let mut h = Host::default();
        assert!(run(&mut h, "rand 0 (msg A on) (msg B on)", Ctx::default()).is_err());
        h.rolls = vec![0.3];
        run(&mut h, "rand 0.5 (msg A on) (msg B on)", Ctx::default()).unwrap();
        assert_eq!(h.sent, [("A".to_string(), "on".to_string())]);
    }
}
