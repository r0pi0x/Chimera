//! The third-person seat camera against specs/halo_ce/vehicles.md's test
//! cases: the track's cubic through four control points, the command (the
//! track point in the look's heading frame from the focus), the pull-in
//! against a wall, and the worked examples of the drivers' cameras from the
//! data documents (the vehicle level, the look along its
//! forward; positions in the model frame from the model origin).

use bevy::math::Vec3;
use halo_ce::game::vehicles::camera::{command, pull_in, track_point};
use halo_ce::game::vehicles::data::VehicleDef;

fn near(a: Vec3, b: Vec3, tol: f32, what: &str) {
    assert!((a - b).abs().max_element() <= tol, "{what}: {a} against the spec's {b}");
}

fn track(d: &VehicleDef, name: &str) -> Vec<[f32; 2]> {
    d.tracks[name].points.clone()
}

#[test]
fn the_track_passes_through_its_points() {
    let d = VehicleDef::warthog();
    let p = track(&d, "warthog_driver_loose");
    let at = |i: usize| Vec3::new(p[i][0], 0.0, p[i][1]);
    near(track_point(&p, 0.0), at(4), 1e-6, "pitch 0: point 4");
    near(track_point(&p, -std::f32::consts::FRAC_PI_2), at(0), 1e-6, "pitch -90: point 0");
    near(track_point(&p, std::f32::consts::FRAC_PI_2), at(8), 1e-5, "pitch +90: point 8");
    for (i, deg) in [(1usize, -67.5f32), (3, -22.5), (5, 22.5), (7, 67.5)] {
        near(track_point(&p, deg.to_radians()), at(i), 1e-5, "every control point");
    }
}

#[test]
fn the_command_and_the_pull_in() {
    let o = Vec3::new(-2.0, 0.0, 1.0);
    let c = command(Vec3::ZERO, o, 0.0, 0.0);
    assert!((c.depth - 2.2361).abs() < 1e-4);
    near(c.probe, Vec3::new(0.2361, 0.0, 1.0), 1e-4, "probe");
    near(c.camera, Vec3::new(-2.0, 0.0, 1.0), 1e-5, "camera");
    near(c.look, Vec3::X, 1e-6, "looking +x");
    let none = |_: Vec3, _: Vec3| None;
    near(pull_in(&c, &none), c.camera, 0.0, "no obstruction");
    // Heading +y.
    let c90 = command(Vec3::ZERO, o, std::f32::consts::FRAC_PI_2, 0.0);
    near(c90.camera, Vec3::new(0.0, -2.0, 1.0), 1e-5, "heading +y");
    near(c90.look, Vec3::Y, 1e-6, "looking +y");
    // A wall plane facing the camera at x = -1.
    let wall = |a: Vec3, b: Vec3| {
        let (da, db) = (a.x + 1.0, b.x + 1.0);
        (da >= 0.0 && db < 0.0).then(|| da / (da - db))
    };
    let at = pull_in(&c, &wall);
    assert!((at.x + 0.9891).abs() < 1e-3, "pulled in to {at}");
}

#[test]
fn the_worked_examples() {
    // Warthog driver: focus the centre of mass; o and C per look pitch.
    let d = VehicleDef::warthog();
    let c = Vec3::from(d.physics.center_of_mass);
    let p = track(&d, "warthog_driver_loose");
    for (deg, o, cam) in [
        (0.0f32, Vec3::new(-2.7524, 0.0, 0.6827), Vec3::new(-2.6455, 0.0, 1.0045)),
        (-30.0, Vec3::new(-2.4078, 0.0, 1.8922), Vec3::new(-2.3009, 0.0, 2.2140)),
        (30.0, Vec3::new(-2.2864, 0.0, 0.0048), Vec3::new(-2.1795, 0.0, 0.3266)),
        (-15.0, Vec3::new(-2.6878, 0.0, 1.2900), Vec3::new(-2.5809, 0.0, 1.6118)),
    ] {
        let t = deg.to_radians();
        near(track_point(&p, t), o, 2e-4, &format!("Warthog o at {deg} degrees"));
        let cmd = command(c, track_point(&p, t), 0.0, t);
        near(cmd.camera, cam, 2e-4, &format!("Warthog camera at {deg} degrees"));
    }
    // Scorpion driver.
    let d = VehicleDef::scorpion();
    let c = Vec3::from(d.physics.center_of_mass);
    let p = track(&d, "scorpion_driver_loose");
    near(track_point(&p, 0.0), Vec3::new(-4.6130, 0.0, 2.1571), 1e-4, "Scorpion o level");
    near(
        command(c, track_point(&p, 0.0), 0.0, 0.0).camera,
        Vec3::new(-4.6468, 0.0, 2.5807),
        2e-4,
        "Scorpion camera level",
    );
    near(
        track_point(&p, (-30f32).to_radians()),
        Vec3::new(-4.0234, 0.0, 3.5954),
        2e-4,
        "Scorpion o 30 degrees down",
    );
}
