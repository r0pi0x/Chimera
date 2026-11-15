//! Kill call-outs (specs/combat_arms/effects_sounds.md 11 "Kill
//! call-outs", Follow-up (2026-11-14) 7 and 8): the team voice lines a
//! death sends, read from the install's team auto-cast records.
//!
//! The data (`CaTeamCallouts`): the install's `attributes/
//! autocastsound.txt`, records `[AutoCastTeams3DSound0]`,
//! `[AutoCastTeams3DSound1]`, ... (0 `EnemyDown`, 1 `ManDown`; the index
//! is what travels), each with its `3DsoundRadius` and its files per
//! sound template (`<Template>_Snd0`, `_Snd1`, ...; templates 0 `Male`,
//! 1 `Female`, 2 `Q_Male`, 3 `Q_Female`, 4 `Q_Ptype`, 5 `Q_Stype`, 6
//! `VIP_M`). Read at start-up when the install is there; without it, a
//! built-in `EnemyDown` of the male file `data/effects.toml` names
//! (`EventSounds::enemy_down`) and nothing else (`ManDown` is empty in
//! the stock data too).
//!
//! The server (`send`, on each `Died`): the killer is the attacker, or
//! for a squad AI (`rules::mercs::CaMerc`) its leader (the leader's voice
//! from the AI's place); with no player behind the kill (a fall, the
//! world, a non-squad AI) the killer's team is 0, the voice the male
//! template's, from where the damage was dealt. `EnemyDown` goes to the
//! killer's team when that team differs from the victim's and the killer
//! is not the victim (`rules::callouts`): team 0 has no players, so a
//! world death is heard by nobody; in free-for-all every player is a team
//! of their own, so only the killer hears it. `ManDown` goes to the
//! victim's team, spoken by the victim's living teammate nearest the
//! victim, strictly closer than 1000 u (none when nobody is). Each is
//! sent to the readers on that team (`chat::readers`: this machine's
//! player directly, a remote one as a server message). `EnemyDown` also
//! goes to other readers who asked for others' voice lines
//! (`party_hears`: `party_voice_lines_3d`, the party apps' default),
//! within 2R of where it is spoken, so in a free-for-all those near the
//! killer hear it too.
//!
//! The client (`hear`): the speaker's template's files in that record,
//! one at random; nothing when the list is empty (the infected `Q_`
//! templates, every `ManDown`) or the record's radius is 0. 3D at the
//! speaker (it follows the speaker while it plays when the speaker is
//! known here), the record's radius, the call-out channel.
//!
//! Known shortcuts: the sound's place is fixed when it starts (Chimera's
//! one-shots: `effects::sounds`), not moved with the speaker; the
//! priority ("high") isn't modelled.

use bevy::{ecs::entity::MapEntities, prelude::*};
use lithtech_formats::attributes::Document;
use serde::{Deserialize, Serialize};

use super::{
    data::CaFxTables,
    rules,
    sounds::{CHANNEL_CALLOUT, CaSound},
};
use super::super::{
    chat,
    inventory::UNIT,
    movement::CaInfected,
    rules::{escort::CaVip, mercs::CaMerc},
};
use crate::core::{Died, Health, Npc, Team};

/// The auto-cast attribute file in the attributes archive.
pub const FILE: &str = "attributes/autocastsound.txt";
/// The team 3D records' prefix.
pub const RECORDS: &str = "AutoCastTeams3DSound";
/// The records a death sends (their index travels).
pub const ENEMY_DOWN: u8 = 0;
pub const MAN_DOWN: u8 = 1;

/// The sound templates, by number (Follow-up (2026-11-14) 8).
pub const TEMPLATES: [&str; 7] = ["Male", "Female", "Q_Male", "Q_Female", "Q_Ptype", "Q_Stype", "VIP_M"];
pub const MALE: u8 = 0;
pub const FEMALE: u8 = 1;
pub const Q_MALE: u8 = 2;
pub const Q_FEMALE: u8 = 3;
pub const VIP_M: u8 = 6;

/// One team call-out record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TeamCallout {
    pub name: String,
    /// Outer radius, u (0: silent).
    pub radius: f32,
    /// Files by template number.
    pub files: Vec<Vec<String>>,
}

/// The team call-out records in use.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaTeamCallouts(pub Vec<TeamCallout>);

impl CaTeamCallouts {
    /// Read the records from a document (until one is missing).
    pub fn read(doc: &Document) -> Self {
        let text = |b: Vec<u8>| b.iter().map(|&c| c as char).collect::<String>();
        Self(
            doc.records(RECORDS)
                .into_iter()
                .map(|s| TeamCallout {
                    name: s.string("Name").map(|b| text(b.to_vec())).unwrap_or_default(),
                    radius: s.number("3DsoundRadius", 0.0) as f32,
                    files: TEMPLATES
                        .iter()
                        .map(|t| {
                            s.string_list(&format!("{t}_Snd"))
                                .into_iter()
                                .map(text)
                                .filter(|f| !f.trim().is_empty())
                                .collect()
                        })
                        .collect(),
                })
                .collect(),
        )
    }

    /// Parse a document's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        Document::parse(text.as_bytes()).map(|d| Self::read(&d)).map_err(|e| e.to_string())
    }

    /// Without the install: `EnemyDown` in the male voice
    /// (`EventSounds::enemy_down`), an empty `ManDown`.
    pub fn builtin(tables: &CaFxTables) -> Self {
        let mut files = vec![Vec::new(); TEMPLATES.len()];
        files[MALE as usize] = tables.sounds.enemy_down.clone();
        Self(vec![
            TeamCallout {
                name: "EnemyDown".into(),
                radius: tables.sounds.enemy_down_radius,
                files,
            },
            TeamCallout {
                name: "ManDown".into(),
                radius: tables.sounds.enemy_down_radius,
                files: vec![Vec::new(); TEMPLATES.len()],
            },
        ])
    }

    /// The install's records when they can be read, else the built-in.
    pub fn load(data: &crate::game::data::CaDataInner) -> Self {
        if !data.builtin_only()
            && let Some(mount) = data.mount()
            && let Ok(bytes) = mount.read(FILE)
        {
            match Document::parse(&bytes) {
                Ok(d) => {
                    let records = Self::read(&d);
                    if !records.0.is_empty() {
                        return records;
                    }
                }
                Err(e) => warn!("combat_arms call-outs: {FILE}: {e}; using the built-in"),
            }
        }
        Self::builtin(CaFxTables::built_in())
    }

    /// The files `template` speaks in `record` and the record's radius;
    /// None when it says nothing (no files, radius 0, no such record).
    pub fn voice(&self, record: u8, template: u8) -> Option<(&[String], f32)> {
        let r = self.0.get(record as usize)?;
        let files = r.files.get(template as usize)?;
        (!files.is_empty() && r.radius > 0.0).then_some((files.as_slice(), r.radius))
    }
}

/// Server to a team: a call-out spoken by `speaker` (None: nobody's, a
/// world kill's) from `at` (metres).
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, MapEntities)]
pub struct CaTeamCallout {
    #[entities]
    pub speaker: Option<Entity>,
    /// The record (`ENEMY_DOWN`, `MAN_DOWN`).
    pub record: u8,
    /// The speaker's sound template.
    pub template: u8,
    pub at: [f32; 3],
}

/// A character's sound template: the infected ones (`Q_Male`,
/// `Q_Female`), the male VIP's, else the body's (`Male`, `Female`).
pub fn template_of(world: &World, e: Entity) -> u8 {
    let female = crate::game::grenades::callout::is_female(world, e);
    if world.get::<CaInfected>(e).is_some() {
        return if female { Q_FEMALE } else { Q_MALE };
    }
    if female {
        FEMALE
    } else if world.get::<CaVip>(e).is_some() {
        VIP_M
    } else {
        MALE
    }
}

/// The call-outs of one death: (team, message) pairs.
pub fn of_death(world: &World, d: &Died) -> Vec<(u8, CaTeamCallout)> {
    let victim = d.entity;
    let (Some(victim_team), Some(victim_at)) = (
        world.get::<Team>(victim).map(|t| t.0),
        world.get::<Transform>(victim).map(|t| t.translation),
    ) else {
        return Vec::new();
    };
    // The killer: the attacker, a squad AI's leader, or nobody (team 0).
    let damager_at = d.attacker.and_then(|a| world.get::<Transform>(a)).map_or(d.damage.point, |t| t.translation);
    let player = |e: Entity| world.get::<Team>(e).is_some() && world.get::<Npc>(e).is_none();
    let killer = d.attacker.and_then(|a| match world.get::<CaMerc>(a) {
        Some(m) => player(m.leader).then_some(m.leader),
        None => player(a).then_some(a),
    });
    let mut out = Vec::new();
    let (killer_team, template, speaker) = match killer {
        Some(k) => (world.get::<Team>(k).map_or(0, |t| t.0), template_of(world, k), Some(k)),
        None => (0, MALE, None),
    };
    let u = |p: Vec3| p / UNIT;
    // Rules (11): EnemyDown when the teams differ and it isn't a suicide.
    let living: Vec<(Entity, u8, Vec3)> = {
        let mut q = world.try_query::<(Entity, &Team, &Transform, &Health)>().expect("registered");
        q.iter(world)
            .filter(|(e, _, _, h)| *e != victim && h.current > 0.0 && world.get::<Npc>(*e).is_none())
            .map(|(e, t, tr, _)| (e, t.0, u(tr.translation)))
            .collect()
    };
    let killer_entry = (speaker.unwrap_or(Entity::PLACEHOLDER), killer_team, u(damager_at));
    let (enemy_down, man_down) = rules::callouts(Some(killer_entry), (victim, victim_team, u(victim_at)), &living);
    if enemy_down.is_some() && speaker != Some(victim) {
        out.push((
            killer_team,
            CaTeamCallout {
                speaker,
                record: ENEMY_DOWN,
                template,
                at: damager_at.to_array(),
            },
        ));
    }
    if let Some(m) = man_down {
        let at = world.get::<Transform>(m).map_or(victim_at, |t| t.translation);
        out.push((
            victim_team,
            CaTeamCallout {
                speaker: Some(m),
                record: MAN_DOWN,
                template: template_of(world, m),
                at: at.to_array(),
            },
        ));
    }
    out
}

/// The server: each death's call-outs, to the readers on their teams
/// (the sending queued as a command: it needs the world's readers).
pub fn send(mut died: MessageReader<Died>, mut commands: Commands) {
    let deaths: Vec<Died> = died.read().cloned().collect();
    if deaths.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        let calls: Vec<(u8, CaTeamCallout)> = deaths.iter().flat_map(|d| of_death(world, d)).collect();
        if calls.is_empty() {
            return;
        }
        let readers = chat::readers(world);
        // `EnemyDown`'s outer radius (u), for others' voice lines.
        let radius = world
            .get_resource::<CaTeamCallouts>()
            .and_then(|r| r.0.get(ENEMY_DOWN as usize))
            .map_or(0.0, |r| r.radius);
        for (team, call) in calls {
            for (target, reader) in &readers {
                if world.get::<Team>(*reader).is_some_and(|t| t.0 == team) || party_hears(world, *reader, &call, radius) {
                    chat::send(world, *target, call.clone());
                }
            }
        }
    });
}

/// Whether `reader`, beyond the call-out's team, gets `call` as one of
/// others' voice lines (`map::sound::PartyVoices`, docs/DESIGN.md "Others'
/// voice lines in 3D"): `EnemyDown` only, spoken by a player (a world
/// kill's has no speaker), to a reader who asked within the server's send
/// reach of the record's radius `radius` (u), as the grenade call-out
/// (`grenades::callout::party_hears`). `ManDown` stays the victim's team's.
pub fn party_hears(world: &World, reader: Entity, call: &CaTeamCallout, radius: f32) -> bool {
    call.record == ENEMY_DOWN
        && radius > 0.0
        && call
            .speaker
            .is_some_and(|s| crate::game::grenades::callout::party_hears(world, reader, s, Vec3::from(call.at), radius))
}

/// The client: play the call-outs that arrived.
pub fn hear(
    mut calls: MessageReader<CaTeamCallout>,
    records: Res<CaTeamCallouts>,
    characters: Query<&Transform>,
    mut draws: Local<u32>,
    mut sounds: MessageWriter<CaSound>,
) {
    for c in calls.read() {
        let Some((files, radius)) = records.voice(c.record, c.template) else {
            continue;
        };
        // One of the template's files, uniformly.
        *draws = draws.wrapping_add(1);
        let pick = (crate::weapon::random::shared_random(*draws, "combat_arms:kill_callout", 0).next() * files.len() as f32) as usize;
        let file = &files[pick.min(files.len() - 1)];
        let at = c.speaker.and_then(|s| characters.get(s).ok()).map_or(Vec3::from(c.at), |t| t.translation);
        let sound = CaSound::at(file, at, radius);
        sounds.write(match c.speaker {
            Some(s) => sound.from_source(s, CHANNEL_CALLOUT),
            None => sound,
        });
    }
}
