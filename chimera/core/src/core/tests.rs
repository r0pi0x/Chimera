use super::*;

#[cfg(test)]
mod remainder_tests {
    use super::DamageRemainder;

    /// specs/hl2dm/suit.md "Fractional damage": 1.6-point hits take 1, 2,
    /// 1, 2; 0.2-point hits nothing four times, then a point.
    #[test]
    fn whole_points_and_the_carried_fraction() {
        let mut r = DamageRemainder::default();
        let taken: Vec<Option<f32>> = (0..4).map(|_| r.take(1.6)).collect();
        assert_eq!(taken, [Some(1.0), Some(2.0), Some(1.0), Some(2.0)]);
        let mut r = DamageRemainder::default();
        let taken: Vec<Option<f32>> = (0..5).map(|_| r.take(0.2)).collect();
        assert_eq!(taken, [None, None, None, None, Some(1.0)]);
        let mut r = DamageRemainder::default();
        assert_eq!(r.take(62.5), Some(62.0));
        assert!((r.0 - 0.5).abs() < 1e-6);
    }
}

#[cfg(test)]
mod scale_tests {
    use bevy::prelude::*;

    use super::{Damage, DamageKind, DamageScale, DamageTaken, Died, Health, Hitgroup, apply_damage};

    fn hit(target: Entity, amount: f32) -> Damage {
        Damage {
            target,
            attacker: None,
            amount,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        }
    }

    /// `DamageScale` multiplies what health takes (and what `Died` and
    /// `DamageTaken` carry); without it, the damage as it comes.
    #[test]
    fn a_damage_scale_multiplies_what_health_takes() {
        let mut app = App::new();
        app.add_message::<Damage>().add_message::<Died>().add_message::<DamageTaken>().add_systems(Update, apply_damage);
        let plain = app.world_mut().spawn(Health::default()).id();
        let scaled = app.world_mut().spawn((Health::default(), DamageScale(1.5))).id();
        for e in [plain, scaled] {
            app.world_mut().write_message(hit(e, 0.35));
        }
        app.update();
        let health = |app: &App, e| app.world().get::<Health>(e).unwrap().current;
        assert!((health(&app, plain) - 0.65).abs() < 1e-6);
        assert!((health(&app, scaled) - 0.475).abs() < 1e-6);
        let taken: Vec<f32> = app.world().resource::<Messages<DamageTaken>>().iter_current_update_messages().map(|t| t.damage.amount).collect();
        assert!((taken[1] - 0.525).abs() < 1e-6, "{taken:?}");
        // 0.35 x 1.5 is lethal on what is left; unscaled it isn't.
        for e in [plain, scaled] {
            app.world_mut().write_message(hit(e, 0.35));
        }
        app.update();
        assert!(health(&app, plain) > 0.0);
        assert_eq!(health(&app, scaled), 0.0);
        let died: Vec<Entity> = app.world().resource::<Messages<Died>>().iter_current_update_messages().map(|d| d.entity).collect();
        assert_eq!(died, [scaled]);
    }
}


#[cfg(test)]
mod brush_grid_tests {
    use super::*;

    /// The grid's candidates are exactly the brushes whose bounds overlap
    /// the box (what testing every brush finds), in index order.
    #[test]
    fn the_grid_finds_what_testing_every_brush_finds() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut rand = move |lo: f32, hi: f32| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            lo + (hi - lo) * (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut brushes = Vec::new();
        for i in 0..2000 {
            let at = Vec3::new(rand(-300.0, 300.0), rand(-50.0, 50.0), rand(-300.0, 300.0));
            // Some long walls and floors (the grid's "big" list), and an
            // empty brush (bounds inverted, as a brush with no volume has).
            if i % 97 == 0 {
                brushes.push(MapBrush::from_box(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)));
                continue;
            }
            let size = if i % 50 == 0 {
                Vec3::new(rand(50.0, 600.0), rand(0.1, 10.0), rand(50.0, 600.0))
            } else {
                Vec3::new(rand(0.1, 8.0), rand(0.1, 8.0), rand(0.1, 8.0))
            };
            brushes.push(MapBrush::from_box(at, at + size));
        }
        let grid = BrushGrid::new(&brushes);
        let mut near = Vec::new();
        for _ in 0..500 {
            // Boxes inside and outside the brushes' extent.
            let lo = Vec3::new(rand(-1000.0, 1000.0), rand(-60.0, 60.0), rand(-1000.0, 1000.0));
            let hi = lo + Vec3::new(rand(0.0, 20.0), rand(0.0, 5.0), rand(0.0, 20.0));
            grid.near(&brushes, lo, hi, &mut near);
            let all: Vec<u32> = brushes
                .iter()
                .enumerate()
                .filter(|(_, b)| !(b.max.cmplt(lo).any() || b.min.cmpgt(hi).any()))
                .map(|(i, _)| i as u32)
                .collect();
            assert_eq!(near, all);
        }
    }
}

#[cfg(test)]
mod hitbox_bounds_tests {
    use super::*;

    /// The bounds hold every shape: a sphere by its radius, a turned box
    /// by its corners.
    #[test]
    fn bounds_hold_spheres_and_turned_boxes() {
        let turned = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
        let shapes = Hitboxes(vec![
            Hitbox::sphere(Vec3::new(0.0, 1.0, 0.0), 0.1, BodyRegion::Chest),
            Hitbox::cuboid(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.1, 0.1, 0.1), turned, BodyRegion::Head),
        ]);
        let (min, max) = shapes.bounds().unwrap();
        let r = 0.1 * std::f32::consts::SQRT_2;
        assert!((max.y - (2.0 + r)).abs() < 1e-5 && (min.y - 0.9).abs() < 1e-5, "{min} {max}");
        assert!((max.x - r).abs() < 1e-5 && (max.z - 0.1).abs() < 1e-5, "{min} {max}");
        assert!(Hitboxes::default().bounds().is_none());
    }

    /// Combat Arms' rules on boxes (another game's body): a segment
    /// through a small head box that goes on through a big box (a
    /// shoulder-high chest) hits the head, though the big box's bounding
    /// sphere would have won (|p|^2 - r^2: 0.045 against the head's
    /// 0.06), and the server's line test needs the box itself. Spheres
    /// keep |p|^2 - r^2.
    #[test]
    fn combat_arms_rules_on_boxes_take_the_box_itself() {
        let head = Hitbox::cuboid(Vec3::new(0.0, 1.6, 0.0), Vec3::splat(0.1), Quat::IDENTITY, BodyRegion::Head);
        let chest = Hitbox::cuboid(Vec3::new(0.15, 1.3, 0.0), Vec3::new(0.25, 0.35, 0.25), Quat::IDENTITY, BodyRegion::Generic);
        let shapes = Hitboxes(vec![chest, head]);
        let start = Vec3::new(-0.3, 1.6, 0.0);
        let end = start + Vec3::X * 3.0;
        assert!(chest.segment_passes(start, end) && head.segment_passes(start, end));
        assert_eq!(shapes.by_power(start, end), Some(1), "the head");
        assert!((head.power_along(start, end) - 0.2 * 0.4).abs() < 1e-5);
        // A line just past the head's box misses it, though it passes its
        // bounding sphere.
        let past = Vec3::new(0.0, 1.6, 0.0) + Vec3::new(0.11, 0.11, 0.0);
        assert!(!head.line_passes(past, Vec3::Z));
        assert!(head.line_passes(Vec3::new(0.0, 1.65, -2.0), Vec3::Z));
        // A sphere's power is its own.
        let s = Hitbox::sphere(Vec3::new(0.0, 1.0, 0.0), 0.2, BodyRegion::Chest);
        let from = Vec3::new(0.0, 1.0, -2.0);
        assert!((s.power_along(from, from + Vec3::Z * 5.0) - (4.0 - 0.04)).abs() < 1e-5);
    }
}
