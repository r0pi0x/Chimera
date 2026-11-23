//! The Grenade Launcher's grenade (specs/tf2/weapons_more.md 4, 6), on
//! Chimera's flyers: `chimera:projectile` launches it (1200 units/s along
//! the aim and 200 up, from 16 forward, 8 right and 6 down of the eye,
//! gravity 800, through its owner's teammates, its touches reported, gone
//! after 2.1 s: the 2.0 s fuse at its tenth 0.2 s check, each 14 ticks);
//! `tf2:grenade` (`blast::BlastParams`) answers (`touch`, the server's,
//! in `FlyerSet::Touch`):
//!
//! - touching an enemy before it has touched anything else (`Rolled`
//!   marks one that has), it explodes there, the enemy at full splash;
//! - anything else bounces it (`bounce`: the normal part of its velocity
//!   reversed at `RESTITUTION`, the rest kept at `FRICTION`; resting on
//!   the ground it rolls, its normal speed dropped), with its bounce sound;
//! - at 2.1 s (the flyer's life ends: `FlyerGone::expired`) it explodes
//!   where it is (`fuse`) at `fuse_scale` (60 %) of its damage to others.
//!
//! Where (effects.md 13): a direct hit's centre is 1 unit off what a
//! 64-unit line through the pipe along its flight meets (the pipe's
//! centre, in the air, when it meets nothing: `BlastWorld::pipe_hit`); a
//! fused pipe's snaps onto a floor within 24 units below it
//! (`BlastWorld::fused`); the scorch on what that line met.
//!
//! The explosion is the shared one (`blast`): 100 x the splash (60 on the
//! fuse), no distance modifier; the Demoman's own the base 100 x the
//! splash to 146 units x0.75 grounded or airborne, pushed at scale 9
//! (weapons_more.md 6 and its corrections from the SDK). A pipe an
//! airblast turned (`airblast::Deflected`) mini-crits (7.4).
//!
//! Known shortcuts: the restitution and friction are ours (the physics
//! engine's in the game: open question 7), as is the grenade's 2-unit box
//! (the game's 4-unit cube); the fuse is the flyer's life, not a check
//! every 0.2 s; no random launch jitter (+-10 right and up); its angular
//! drag is ours (open question 22; its launch spin and drag are
//! `chimera:projectile`'s `spin` and `drag`, weapons_more.md 4.1, 4.4);
//! no speed cap (open question 23); teammates are passed for its whole
//! flight (the game: 0.25 s) and enemies after a bounce aren't bounced
//! off; a grenade doesn't explode on buildings (there are none).

use bevy::prelude::*;

use chimera_sim::weapon::projectile::{Flyer, FlyerGone, FlyerHit, reflect};

use super::{
    blast::{Blast, BlastWorld},
    damage::U,
    rocket::RocketRules,
};

/// The normal part of a bounce's velocity kept (ours).
pub const RESTITUTION: f32 = 0.45;
/// The tangential part kept by a bounce (ours).
pub const FRICTION: f32 = 0.8;
/// Below this normal speed (units/s) a touch is resting contact: no
/// bounce, the roll goes on (ours).
pub const REST: f32 = 60.0;
/// The tangential speed kept per resting touch (a roll's slowing; ours).
pub const ROLL: f32 = 0.97;
/// A bounce faster than this (units/s) is heard.
pub const LOUD: f32 = 100.0;

/// The grenades' rules by weapon (`tf2:grenade`).
#[derive(Resource, Clone, Debug, Default)]
pub struct GrenadeRules(pub RocketRules);

/// On a grenade that has touched something: it no longer explodes on
/// enemies.
#[derive(Component, Clone, Copy, Debug)]
pub struct Rolled;

/// The velocity after touching a surface of normal `n` (unit) going `v`
/// (m/s).
pub fn bounce(v: Vec3, n: Vec3) -> Vec3 {
    let into = v.dot(n);
    if into >= 0.0 {
        return v;
    }
    let tangent = v - n * into;
    if -into < REST * U {
        return tangent * ROLL;
    }
    let r = reflect(v, n);
    let normal = n * r.dot(n);
    normal * RESTITUTION + (r - normal) * FRICTION
}

#[allow(clippy::too_many_arguments)]
pub fn touch(
    mut hits: MessageReader<FlyerHit>,
    rules: Res<GrenadeRules>,
    mut flyers: Query<(&mut Flyer, &Transform, Has<Rolled>, Has<super::airblast::Deflected>)>,
    mut world: BlastWorld,
    mut gone: MessageWriter<FlyerGone>,
    mut commands: Commands,
) {
    for hit in hits.read() {
        let Some((p, _)) = rules.0.get(hit.weapon) else { continue };
        let Ok((mut f, t, rolled, deflected)) = flyers.get_mut(hit.flyer) else { continue };
        let enemy = hit.character && hit.other.is_some_and(|o| hit.owner.is_none_or(|w| !world.mates(w, o)));
        if enemy && !rolled {
            // Its 64-unit line through its centre (effects.md 13).
            let at = world.pipe_hit(t.translation, hit.velocity);
            let blast = Blast { params: p, radius: p.radius, ramp: None, scale: 1.0, mini_crit: deflected };
            world.detonate(&blast, &at, hit.owner, hit.weapon, hit.other);
            gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: at.centre, expired: false });
            commands.entity(hit.flyer).despawn();
            continue;
        }
        if p.shatter {
            // Gone in a puff, no blast (item_attributes.md 4.5).
            gone.write(FlyerGone { flyer: hit.flyer, weapon: hit.weapon, owner: hit.owner, at: t.translation, expired: false });
            commands.entity(hit.flyer).despawn();
            continue;
        }
        f.velocity = match p.first_touch_keep {
            // No bounce: its first world touch keeps a share (4.5).
            Some(k) if !rolled && hit.world => hit.velocity * k,
            _ => bounce(hit.velocity, hit.normal),
        };
        if !rolled {
            commands.entity(hit.flyer).insert(Rolled);
        }
        if -hit.velocity.dot(hit.normal) > LOUD * U
            && let Some(s) = &p.bounce_sound
        {
            world.sound(s, hit.point);
        }
    }
}

/// The fuse: a grenade whose life ended explodes where it is.
pub fn fuse(mut ends: MessageReader<FlyerGone>, rules: Res<GrenadeRules>, deflected: Query<(), With<super::airblast::Deflected>>, mut world: BlastWorld) {
    for end in ends.read() {
        if !end.expired {
            continue;
        }
        let Some((p, _)) = rules.0.get(end.weapon) else { continue };
        let blast = Blast { params: p, radius: p.radius, ramp: None, scale: p.fuse_scale.unwrap_or(1.0), mini_crit: deflected.contains(end.flyer) };
        // Snapped onto a floor within 24 units below (effects.md 13).
        let at = world.fused(end.at, None);
        world.detonate(&blast, &at, end.owner, end.weapon, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bounce_loses_speed_and_a_roll_keeps_rolling() {
        let v = Vec3::new(10.0, -10.0, 0.0);
        let b = bounce(v, Vec3::Y);
        assert!(b.y > 0.0 && b.y < 10.0 && b.x < 10.0 && b.x > 0.0, "{b}");
        let rolling = bounce(Vec3::new(5.0, -0.1, 0.0), Vec3::Y);
        assert_eq!(rolling.y, 0.0);
        assert!(rolling.x > 4.8);
    }
}
