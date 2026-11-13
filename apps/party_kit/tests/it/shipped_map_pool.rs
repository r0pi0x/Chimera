//! The shipped map pool data (each game's data/map_pool.toml and
//! data/map_fitness.toml, `chimera_sim::minigame::map_fit`;
//! docs/plans/active/map-pool-fitness.md): it reads, the pool at 4, 8
//! and 12 players is what the plan says, the deny lists hold what the user
//! asked to keep out, and the maps of the pool before stay allowed.

use chimera_sim::minigame::map_fit::{FitSettings, MapPool, PoolRules, parse_fitness};

/// Every game's rules and measurements, as the apps register them.
fn shipped() -> MapPool {
    let mut p = MapPool::default();
    for (game, rules, fitness) in [
        ("cs_source", cs_source::game::parts::POOL_RULES, cs_source::game::parts::MAP_FITNESS),
        ("combat_arms", combat_arms::game::parts::POOL_RULES, combat_arms::game::parts::MAP_FITNESS),
        ("hl2dm", hl2dm::game::parts::POOL_RULES, hl2dm::game::parts::MAP_FITNESS),
    ] {
        p.add_rules(game, PoolRules::parse(rules).unwrap_or_else(|e| panic!("{game} map_pool.toml: {e}")));
        p.add_fitness(game, parse_fitness(fitness).unwrap_or_else(|e| panic!("{game} map_fitness.toml: {e}")));
    }
    p
}

/// Every measured map, `game:name`.
fn measured() -> Vec<String> {
    let mut out = Vec::new();
    for (game, fitness) in [
        ("cs_source", cs_source::game::parts::MAP_FITNESS),
        ("combat_arms", combat_arms::game::parts::MAP_FITNESS),
        ("hl2dm", hl2dm::game::parts::MAP_FITNESS),
    ] {
        out.extend(parse_fitness(fitness).unwrap().into_keys().map(|m| format!("{game}:{m}")));
    }
    out
}

#[test]
fn the_shipped_pool_by_count() {
    let pool = shipped();
    let s = FitSettings::shipped();
    let maps = measured();
    assert_eq!(maps.len(), 222, "the 2026-11-13 sweep's maps");
    let in_at = |n: usize| -> Vec<&String> { maps.iter().filter(|m| pool.judge(m, n, &s).weight() > 0.0).collect() };
    let count = |n: usize, game: &str| in_at(n).iter().filter(|m| m.starts_with(game)).count();
    for (n, total, by_game) in [(4, 104, [48, 50, 6]), (8, 122, [58, 57, 7]), (12, 122, [58, 57, 7])] {
        assert_eq!(in_at(n).len(), total, "{n} players: {}", pool.lines(&maps, n, &s));
        assert_eq!([count(n, "cs_source:"), count(n, "combat_arms:"), count(n, "hl2dm:")], by_game, "{n} players");
    }
    // Small maps: about an eighth of the draw at 4 players, less at more.
    let small_share = |n: usize| {
        let (mut small, mut all) = (0.0, 0.0);
        for m in &maps {
            if let chimera_sim::minigame::map_fit::Judged::In { weight, small: s, .. } = pool.judge(m, n, &s) {
                all += weight;
                if s {
                    small += weight;
                }
            }
        }
        small / all
    };
    for (n, lo, hi) in [(4, 0.08, 0.18), (8, 0.03, 0.10), (12, 0.05, 0.14)] {
        let share = small_share(n);
        assert!((lo..hi).contains(&share), "{n} players: small maps take {share:.2} of the draw");
    }
}

#[test]
fn the_deny_lists_hold_the_users_choices_and_keep_the_old_pool() {
    let pool = shipped();
    let out = |game: &str, map: &str| pool.listed(game, map).is_err();
    for m in ["bhop_addict_v2_3xl", "kz_11342", "surf_kismet", "mg_wipeout", "test_hardware", "cs_militia", "cs_havana"] {
        assert!(out("cs_source", m), "cs_source:{m}");
    }
    for m in [
        "bossbattle_cp", "sandhog_cp1", "bossrush", "zombiewar", "infectedship_cp", "railwaydefence", "training",
        "testing_t", "snowstorm_qrt", "grayhammerq", "vertigo_q", "costarecon_q", "darkthandark", "deserthole",
        "desertholezone", "16rooms", "nemexishq", "hgoilrig", "sector25", "castle", "battleship", "cargoship",
        "twinship", "killland", "lastvegas", "twotowers", "crystaltower", "rapidstream", "waverider", "downhill",
        "grayhammer",
    ] {
        assert!(out("combat_arms", m), "combat_arms:{m}");
    }
    assert!(out("hl2dm", "halls3"));
    // The pool before this stays allowed (its claimed maps and CS:S's
    // de_ maps but those above).
    for (g, m) in [
        ("combat_arms", "warehouse"), ("combat_arms", "junk_flea"), ("combat_arms", "darkhills"),
        ("combat_arms", "bloodmoney"), ("cs_source", "de_dust2"), ("cs_source", "cs_office"),
        ("cs_source", "de_nuke"), ("hl2dm", "dm_lockdown"), ("hl2dm", "dm_overwatch"),
    ] {
        assert!(!out(g, m), "{g}:{m}");
    }
    // A big stock map waits for more players; a small one is always in.
    let s = FitSettings::shipped();
    assert_eq!(pool.judge("cs_source:de_nuke", 4, &s).weight(), 0.0);
    assert!(pool.judge("cs_source:de_nuke", 8, &s).weight() > 0.0);
    assert!(pool.judge("combat_arms:darkhills", 12, &s).weight() > 0.0);
}
