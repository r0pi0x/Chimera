use super::*;
    use super::*;
    use crate::bot::path::tests::two_ways;

    #[test]
    fn teammates_give_way_in_doors() {
        let dir = Vec3::NEG_Z;
        let right = dir.cross(Vec3::Y);
        let ahead = Vec3::new(0.0, 0.0, -1.5);
        // Coming the other way: both keep right; the higher number (and any
        // bot facing a person) nearly stops.
        let toward = Vec3::Z * 5.0;
        let w = give_way(dir, ahead, toward, 3, 2, 3).unwrap();
        assert!(w.side.dot(right) > 0.0 && w.pace < 0.5, "{w:?}");
        let w = give_way(dir, ahead, toward, 2, 3, 2).unwrap();
        assert!(w.side.dot(right) > 0.0 && w.pace == 1.0, "{w:?}");
        assert!(give_way(dir, ahead, toward, 1, 0, 1).unwrap().pace < 0.5, "people first");
        // Standing in the way, a little to the left: round it on the right.
        let w = give_way(dir, ahead + Vec3::X * -0.2, Vec3::ZERO, 2, 3, 2).unwrap();
        assert!(w.side.dot(right) > 0.0, "{w:?}");
        let w = give_way(dir, ahead + Vec3::X * 0.2, Vec3::ZERO, 2, 3, 2).unwrap();
        assert!(w.side.dot(right) < 0.0, "{w:?}");
        // Walking ahead the same way, close: slow down, no sidestep.
        let w = give_way(dir, Vec3::new(0.0, 0.0, -0.9), Vec3::NEG_Z * 2.0, 2, 3, 2).unwrap();
        assert_eq!(w.side, Vec3::ZERO);
        assert!(w.pace < 1.0);
        // Behind, beside, far ahead or a floor up: not in the way.
        for rel in [Vec3::Z * 1.0, Vec3::X * 1.5, Vec3::NEG_Z * 4.0, Vec3::new(0.0, 2.0, -1.0)] {
            assert_eq!(give_way(dir, rel, Vec3::ZERO, 2, 3, 2), None, "{rel}");
        }
    }

    /// A bot that keeps failing to cross a link walks around it, and
    /// goes back to the short way once the memory runs out.
    #[test]
    fn stuck_bot_repaths_around_the_link() {
        let nav = two_ways(0);
        let mut bot = Bot::default();
        let params = path::CostParams::default();
        // Standing in area 1 (x 4..8, z 0..4), going to area 3's middle.
        let feet = Vec3::new(7.0, 0.0, 2.0);
        let goal = nav.areas[3].center;
        let dt = 1.0 / 60.0;
        let mut now = 0.0;
        walk_route(&mut bot, &nav, feet, goal, &params, dt, now, false, true, &[]);
        assert!(
            bot.route_areas.contains(&3) && !bot.route_areas.contains(&5),
            "{:?}",
            bot.route_areas
        );
        // Not moving at all (a wall the mesh doesn't know about).
        let mut jumped = false;
        while now < 3.0 * STUCK_SECONDS as f64 + 0.1 {
            now += dt as f64;
            let step = walk_route(&mut bot, &nav, feet, goal, &params, dt, now, false, true, &[]).unwrap();
            jumped |= step.jump;
        }
        assert!(jumped, "a stuck bot jumps first");
        assert_eq!(bot.stuck_links.iter().map(|l| l.0).collect::<Vec<_>>(), [(1, 3)]);
        // The new route goes round through areas 2 and 5.
        assert!(
            bot.route_areas.contains(&5) && bot.route_areas.contains(&2),
            "{:?}",
            bot.route_areas
        );
        // Later it is forgotten.
        now += STUCK_MEMORY + 1.0;
        bot.repath = 0.0;
        bot.progress = (Vec3::splat(100.0), 0.0);
        walk_route(&mut bot, &nav, feet, goal, &params, dt, now, false, true, &[]);
        assert!(bot.stuck_links.is_empty());
        assert!(!bot.route_areas.contains(&5), "{:?}", bot.route_areas);
    }

