//! TF2's spawn rule (`maps::team_spawn`, specs/tf2/install.md 2.3) on
//! made-up entities: RED (2) is our team 1, BLU (3) our team 2; disabled,
//! triggered and team-0 spawns are left out; a spawn naming a control
//! point is used while its team owns the point at round start.

use chimera_core::core::Team;
use chimera_world::map::MapEntity;
use tf2::game::maps::team_spawn;

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
