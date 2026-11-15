//! The saved game variant and the stock game type table
//! (specs/halo_ce/multiplayer_rules.md, sections 1 and 2): the record's
//! layout in both directions, the enumerations' tables, the rejections and
//! the 26 stock rows as the table lists them.

use blam_formats::game_type::{
    BallSpeed, BallType, BaseGameType, GameFlags, GameTypeOptions, GameVariant, ObjectiveIndicator, OddballTrait,
    RaceType, STOCK_COUNT, STOCK_GAME_TYPES, TeamScoring, VARIANT_SIZE, VehicleSet, WeaponSet,
};

fn record() -> Vec<u8> {
    let mut b = vec![0u8; VARIANT_SIZE];
    for (i, u) in "Custom".encode_utf16().enumerate() {
        b[2 * i..2 * i + 2].copy_from_slice(&u.to_le_bytes());
    }
    let mut put = |o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    put(0x18, 3);
    put(0x20, 0x23);
    put(0x24, 1);
    put(0x2C, 150);
    put(0x30, 300);
    put(0x34, 450);
    put(0x38, 3);
    put(0x3C, 1.5f32.to_bits());
    put(0x40, 5);
    put(0x44, 2);
    put(0x48, 4);
    put(0x50, 2);
    put(0x54, 1);
    put(0x58, 3);
    put(0x5C, 2);
    put(0x60, 16);
    b[0x1C] = 1;
    b[0x28] = 1;
    b[0x4C] = 1;
    b[0x64] = 1;
    b
}

#[test]
fn reads_an_oddball_record() {
    let v = GameVariant::from_bytes(&record()).unwrap();
    assert_eq!(v.name, "Custom");
    let r = v.rules;
    assert_eq!(r.base(), BaseGameType::Oddball);
    assert!(r.team_play && r.odd_man_out);
    assert_eq!(r.flags.bits(), 0x23);
    assert!(r.flags.contains(GameFlags::GENERIC_STARTING_EQUIPMENT));
    assert!(!r.flags.contains(GameFlags::NO_SHIELDS));
    assert_eq!(r.objective_indicator, ObjectiveIndicator::NavPoints);
    assert_eq!(
        (r.respawn_time_growth, r.respawn_time, r.suicide_penalty),
        (150, 300, 450)
    );
    assert_eq!((r.number_of_lives, r.health, r.score_to_win), (3, 1.5, 5));
    assert_eq!((r.weapon_set, r.vehicle_set), (WeaponSet::Rifles, VehicleSet::Scorpion));
    assert_eq!(r.variant_flags, 1);
    assert_eq!(
        r.options,
        GameTypeOptions::Oddball {
            random_start: true,
            ball_spawn_delay: false,
            speed_with_ball: BallSpeed::Fast,
            trait_with_ball: OddballTrait::Invisible,
            trait_without_ball: OddballTrait::DamageResistant,
            ball_type: BallType::Juggernaut,
            ball_spawn_count: 16,
        }
    );
    // And back, byte for byte.
    assert_eq!(v.to_bytes().to_vec(), record());
}

#[test]
fn reads_each_base_types_options() {
    let mut b = record();
    b[0x18] = 1;
    b[0x4C..0x54].copy_from_slice(&[1, 0, 1, 1, 0x58, 0x02, 0, 0]);
    let v = GameVariant::from_bytes(&b[..0x68]).unwrap();
    assert_eq!(
        v.rules.options,
        GameTypeOptions::CaptureTheFlag {
            assault: true,
            reset_on_capture: false,
            flag_must_reset: true,
            flag_at_home_to_score: true,
            single_flag_time: 0x258,
        }
    );
    let mut b = record();
    b[0x18] = 2;
    b[0x4C..0x4F].copy_from_slice(&[0, 1, 1]);
    assert_eq!(
        GameVariant::from_bytes(&b).unwrap().rules.options,
        GameTypeOptions::Slayer {
            no_death_bonus: false,
            no_kill_penalty: true,
            kill_in_order: true
        }
    );
    let mut b = record();
    b[0x18] = 4;
    assert_eq!(
        GameVariant::from_bytes(&b).unwrap().rules.options,
        GameTypeOptions::KingOfTheHill { moving_hill: true }
    );
    let mut b = record();
    b[0x18] = 5;
    b[0x4C] = 2;
    b[0x50] = 2;
    assert_eq!(
        GameVariant::from_bytes(&b).unwrap().rules.options,
        GameTypeOptions::Race {
            race_type: RaceType::Rally,
            team_scoring: TeamScoring::Sum
        }
    );
}

#[test]
fn refuses_what_is_outside_the_tables() {
    assert!(GameVariant::from_bytes(&record()[..0x60]).is_err());
    for (offset, value, what) in [
        (0x18usize, 0u8, "base game type"),
        (0x18, 6, "base game type"),
        (0x24, 3, "objective indicator"),
        (0x44, 14, "weapon set"),
        (0x48, 5, "vehicle set"),
        (0x50, 3, "speed with ball"),
        (0x5C, 3, "ball type"),
    ] {
        let mut b = record();
        b[offset] = value;
        let err = GameVariant::from_bytes(&b).unwrap_err();
        assert!(err.to_string().contains(what), "{what}: {err}");
    }
    // An unpaired surrogate in the name is not text.
    let mut b = record();
    b[0] = 0x00;
    b[1] = 0xD8;
    assert!(GameVariant::from_bytes(&b).is_err());
    // A long name is cut to 11 units on the way out.
    let mut v = GameVariant::from_bytes(&record()).unwrap();
    v.name = "abcdefghijklmnop".into();
    assert_eq!(GameVariant::from_bytes(&v.to_bytes()).unwrap().name, "abcdefghijk");
}

#[test]
fn the_stock_table_is_the_specs() {
    assert_eq!(STOCK_GAME_TYPES.len(), STOCK_COUNT);
    let names: Vec<&str> = STOCK_GAME_TYPES.iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        [
            "Slayer",
            "Slayer Pro",
            "Elimination",
            "Phantoms",
            "Endurance",
            "Rockets",
            "Snipers",
            "Oddball",
            "Reverse Tag",
            "Accumulate",
            "Juggernaut",
            "Stalker",
            "King",
            "King Pro",
            "Crazy King",
            "Race",
            "Rally",
            "CTF",
            "Invasion",
            "Iron CTF",
            "CTF Pro",
            "Team Race",
            "Team Rally",
            "Team Ball",
            "Team King",
            "Team Slayer"
        ]
    );
    // (teams, flags, respawn s, growth s, suicide s, lives, health, score,
    // indicator, weapon set, vehicle set, odd man out) per row.
    use ObjectiveIndicator::{MotionTracker as M, NavPoints as N, None as X};
    type Row = (
        bool,
        u32,
        i32,
        i32,
        i32,
        i32,
        f32,
        i32,
        ObjectiveIndicator,
        i16,
        i16,
        bool,
    );
    let rows: [Row; 26] = [
        (false, 3, 0, 0, 10, 0, 1.0, 15, M, 0, 2, false),
        (false, 35, 0, 0, 15, 0, 1.0, 25, M, 0, 2, false),
        (false, 3, 0, 0, 10, 1, 1.0, 25, M, 0, 2, false),
        (false, 18, 5, 0, 5, 0, 1.0, 10, N, 0, 2, false),
        (false, 3, 0, 10, 10, 5, 1.0, 10, M, 0, 2, true),
        (false, 34, 0, 0, 10, 0, 1.0, 25, N, 6, 2, false),
        (false, 34, 0, 5, 10, 0, 1.0, 15, N, 4, 2, false),
        (false, 3, 5, 0, 5, 0, 1.0, 2, N, 0, 1, false),
        (false, 2, 5, 0, 5, 0, 1.0, 2, N, 0, 1, false),
        (false, 2, 5, 0, 5, 0, 1.0, 5, X, 0, 1, false),
        (false, 3, 5, 0, 5, 0, 1.0, 10, N, 0, 2, false),
        (false, 2, 5, 0, 5, 0, 1.0, 10, M, 0, 2, false),
        (false, 3, 5, 0, 5, 0, 1.0, 2, N, 0, 2, false),
        (false, 35, 10, 0, 15, 0, 1.0, 2, N, 0, 2, false),
        (false, 3, 0, 0, 5, 0, 1.0, 2, N, 0, 2, false),
        (false, 3, 0, 0, 10, 0, 1.0, 3, N, 0, 2, false),
        (false, 3, 0, 0, 10, 0, 1.0, 15, N, 0, 2, false),
        (true, 3, 10, 0, 5, 0, 1.0, 3, N, 0, 2, false),
        (true, 3, 0, 0, 5, 5, 1.0, 3, N, 0, 2, false),
        (true, 3, 15, 0, 5, 0, 2.0, 3, N, 0, 4, false),
        (true, 35, 10, 0, 15, 0, 1.0, 3, N, 0, 2, false),
        (true, 3, 0, 0, 10, 0, 1.0, 3, N, 0, 2, false),
        (true, 3, 0, 0, 10, 0, 1.0, 5, N, 0, 2, false),
        (true, 35, 10, 0, 5, 0, 1.0, 2, N, 0, 1, false),
        (true, 3, 10, 0, 5, 0, 1.0, 2, N, 0, 2, false),
        (true, 3, 10, 0, 10, 0, 1.0, 50, M, 0, 2, false),
    ];
    for (s, row) in STOCK_GAME_TYPES.iter().zip(rows) {
        let r = s.rules;
        let got = (
            r.team_play,
            r.flags.bits(),
            r.respawn_time / 30,
            r.respawn_time_growth / 30,
            r.suicide_penalty / 30,
            r.number_of_lives,
            r.health,
            r.score_to_win,
            r.objective_indicator,
            r.weapon_set.code(),
            r.vehicle_set.code(),
            r.odd_man_out,
        );
        assert_eq!(got, row, "{}", s.name);
        assert_eq!(r.respawn_time % 30, 0);
        assert_eq!(r.variant_flags, 1, "{}: system default", s.name);
    }
    // Only Team Ball's flags are uncertain.
    let uncertain: Vec<&str> = STOCK_GAME_TYPES
        .iter()
        .filter(|s| s.flags_uncertain)
        .map(|s| s.name)
        .collect();
    assert_eq!(uncertain, ["Team Ball"]);
    // Base types by row.
    use BaseGameType::*;
    let bases = [
        Slayer,
        Slayer,
        Slayer,
        Slayer,
        Slayer,
        Slayer,
        Slayer,
        Oddball,
        Oddball,
        Oddball,
        Oddball,
        Oddball,
        KingOfTheHill,
        KingOfTheHill,
        KingOfTheHill,
        Race,
        Race,
        CaptureTheFlag,
        CaptureTheFlag,
        CaptureTheFlag,
        CaptureTheFlag,
        Race,
        Race,
        Oddball,
        KingOfTheHill,
        Slayer,
    ];
    for (s, b) in STOCK_GAME_TYPES.iter().zip(bases) {
        assert_eq!(s.rules.base(), b, "{}", s.name);
    }
}

#[test]
fn the_stock_table_options() {
    let o = |i: usize| STOCK_GAME_TYPES[i].rules.options;
    let slayer = |bonus_off: bool, order: bool| GameTypeOptions::Slayer {
        no_death_bonus: bonus_off,
        no_kill_penalty: bonus_off,
        kill_in_order: order,
    };
    assert_eq!(o(0), slayer(false, false));
    assert_eq!(o(1), slayer(true, false));
    assert_eq!(o(3), slayer(true, true));
    assert_eq!(o(25), slayer(false, false));
    let ball = |ty, n, speed, with, without| GameTypeOptions::Oddball {
        random_start: false,
        ball_spawn_delay: false,
        speed_with_ball: speed,
        trait_with_ball: with,
        trait_without_ball: without,
        ball_type: ty,
        ball_spawn_count: n,
    };
    use OddballTrait::{DamageResistant, ExtraDamage, Invisible, None as T0};
    assert_eq!(o(7), ball(BallType::Normal, 1, BallSpeed::Slow, T0, T0));
    assert_eq!(o(8), ball(BallType::ReverseTag, 1, BallSpeed::Slow, T0, T0));
    assert_eq!(o(9), ball(BallType::ReverseTag, 16, BallSpeed::Slow, T0, T0));
    assert_eq!(o(10), ball(BallType::Juggernaut, 1, BallSpeed::Slow, ExtraDamage, T0));
    assert_eq!(
        o(11),
        ball(BallType::Juggernaut, 1, BallSpeed::Fast, Invisible, DamageResistant)
    );
    assert_eq!(o(23), ball(BallType::Normal, 1, BallSpeed::Slow, T0, T0));
    for (i, moving) in [(12, false), (13, false), (14, true), (24, true)] {
        assert_eq!(o(i), GameTypeOptions::KingOfTheHill { moving_hill: moving });
    }
    for (i, ty) in [
        (15, RaceType::Normal),
        (16, RaceType::Rally),
        (21, RaceType::Normal),
        (22, RaceType::Rally),
    ] {
        assert_eq!(
            o(i),
            GameTypeOptions::Race {
                race_type: ty,
                team_scoring: TeamScoring::Minimum
            }
        );
    }
    let ctf = |assault, reset, home| GameTypeOptions::CaptureTheFlag {
        assault,
        reset_on_capture: false,
        flag_must_reset: reset,
        flag_at_home_to_score: home,
        single_flag_time: 0,
    };
    assert_eq!(o(17), ctf(false, false, false));
    assert_eq!(o(18), ctf(true, false, false));
    assert_eq!(o(19), ctf(false, true, false));
    assert_eq!(o(20), ctf(false, false, true));
    // Every stock type survives the record.
    for s in &STOCK_GAME_TYPES {
        let v = s.variant();
        assert_eq!(GameVariant::from_bytes(&v.to_bytes()).unwrap(), v, "{}", s.name);
    }
    assert_eq!(STOCK_GAME_TYPES[19].rules.vehicle_set, VehicleSet::Scorpion);
}
