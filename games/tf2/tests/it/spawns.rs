//! TF2's spawn rule (`maps::team_spawn`, specs/tf2/install.md 2.3) on
//! made-up entities: RED (2) is our team 1, BLU (3) our team 2; disabled,
//! triggered and team-0 spawns are left out; a spawn naming a control
//! point is used while its team owns the point at round start. And the
//! free-for-all's doors (`maps::free_for_all_doors`): a door a trigger
//! opens on touch starts open, the others as they are.

use chimera_core::core::Team;
use chimera_world::map::MapEntity;
use tf2::game::maps::{free_for_all_doors, team_spawn};

fn entity(keys: &[(&str, &str)]) -> MapEntity {
    MapEntity {
        keyvalues: keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        hulls: Vec::new(),
        mover: false,
        physics: None,
    }
}

fn spawn(team: &str, more: &[(&str, &str)]) -> MapEntity {
    let mut keys = vec![("classname", "info_player_teamspawn"), ("TeamNum", team)];
    keys.extend_from_slice(more);
    entity(&keys)
}

#[test]
fn red_and_blu_are_teams_one_and_two() {
    assert_eq!(team_spawn(&spawn("2", &[]), &[]), Some(Some(Team(1))));
    assert_eq!(team_spawn(&spawn("3", &[]), &[]), Some(Some(Team(2))));
    // Keys in any case, as Hammer writes them.
    let lower = entity(&[("classname", "info_player_teamspawn"), ("teamnum", "3")]);
    assert_eq!(team_spawn(&lower, &[]), Some(Some(Team(2))));
}

#[test]
fn others_are_not_spawns() {
    assert_eq!(team_spawn(&spawn("0", &[("MatchSummary", "1")]), &[]), None, "the match summary's");
    assert_eq!(team_spawn(&spawn("2", &[("StartDisabled", "1")]), &[]), None);
    assert_eq!(team_spawn(&spawn("3", &[("SpawnMode", "1")]), &[]), None, "triggered");
    assert_eq!(team_spawn(&entity(&[("classname", "info_player_start")]), &[]), None);
    assert_eq!(team_spawn(&entity(&[("classname", "info_player_teamspawn")]), &[]), None, "no team");
}

#[test]
fn a_control_points_spawns_are_its_starting_owners() {
    let point = |name: &str, owner: &str| {
        entity(&[("classname", "team_control_point"), ("targetname", name), ("point_default_owner", owner)])
    };
    let all = [point("cap_red_1", "2"), point("cap_center", "0"), point("cap_blue_2", "3")];
    let at = |team: &str, cp: &str| team_spawn(&spawn(team, &[("controlpoint", cp)]), &all);
    assert_eq!(at("2", "cap_red_1"), Some(Some(Team(1))), "RED's home");
    assert_eq!(at("2", "cap_center"), None, "neutral at the start");
    assert_eq!(at("2", "cap_blue_2"), None, "BLU's");
    assert_eq!(at("3", "cap_blue_2"), Some(Some(Team(2))));
    // A point the map doesn't hold leaves the spawn as it is.
    assert_eq!(at("3", "no_such_point"), Some(Some(Team(2))));
}

#[test]
fn doors_a_trigger_opens_on_touch_start_open() {
    let door = |name: &str| entity(&[("classname", "func_door"), ("targetname", name), ("spawnflags", "0")]);
    let mut all = vec![
        // A spawn room's: its trigger filtered to a team (a filter class
        // the logic doesn't know, so it lets anyone through), a wildcard.
        entity(&[
            ("classname", "trigger_multiple"),
            ("filtername", "filter_red_team"),
            ("onstarttouchall", "door_red1_1_*,Open,,0,-1"),
            ("onendtouchall", "door_red1_1_*,Close,,0,-1"),
        ]),
        door("door_red1_1_left"),
        door("door_red1_1_right"),
        // One the logic opens (a relay, a round timer): left closed.
        entity(&[("classname", "logic_relay"), ("ontrigger", "gate,Open,,0,-1")]),
        door("gate"),
        // Opened on touch by a trigger_once, by OnTrigger.
        entity(&[("classname", "trigger_once"), ("OnTrigger", "hatch\u{1b}Open\u{1b}\u{1b}0\u{1b}-1")]),
        entity(&[("classname", "func_door_rotating"), ("targetname", "hatch"), ("spawnpos", "0")]),
        // Closed on touch: as it is.
        entity(&[("classname", "trigger_multiple"), ("onstarttouch", "shutter,Close,,0,-1")]),
        door("shutter"),
    ];
    free_for_all_doors(&mut all);
    let open = |i: usize| all[i].get("spawnpos") == Some("1");
    assert!(open(1) && open(2), "the spawn room's doors");
    assert!(!open(4), "a door the logic opens");
    assert!(open(6), "the hatch");
    assert!(!open(8), "a door a trigger closes");
    assert_eq!(all[6].keyvalues.iter().filter(|(k, _)| k == "spawnpos").count(), 1, "the key set, not added twice");
}
