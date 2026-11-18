use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    character::CAPSULE_HEIGHT,
    character::spawn_character,
    console::{Command, Console, resource_cvar},
    core::{Health, Intent, MapBrush, MapBrushes, MovementState, SimSet, SpawnPoint, Team},
    map::nav::{NavLadder, NavMesh, STEP_HEIGHT, flags},
    weapon::{Inventory, grenade::Projectile},
};

use super::*;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn think(
    mut bots: Query<(
        Entity,
        &mut Bot,
        &mut Intent,
        &Transform,
        &MovementState,
        &Team,
        &Health,
        Option<&crate::core::Blinded>,
        Option<&mut Inventory>,
        &crate::core::Velocity,
        Option<&crate::weapon::ViewPunch>,
    )>,
    others: Query<(Entity, &Transform, &Team, &Health), With<Intent>>,
    teamless: Query<(&Transform, &Health), (With<Intent>, Without<Team>)>,
    arms: grenades::Arms,
    triggers: Query<&crate::weapon::Trigger>,
    projectiles: Query<(Entity, &Transform, &Projectile)>,
    smoke: Query<&crate::core::SightBlocker>,
    breakable: Query<(), With<crate::core::Damageable>>,
    spatial: SpatialQuery,
    nav: Option<Res<NavMesh>>,
    brushes: Option<Res<MapBrushes>>,
    cfg: Res<BotConfig>,
    time: Res<Time>,
    tactics: Res<Tactics>,
    vis: Option<Res<crate::map::vis::ActiveVisibility>>,
    round: (
        Option<Res<crate::core::FreezeTime>>,
        Option<Res<crate::objectives::RoundOpen>>,
        Query<&crate::weapon::Hitscan>,
        Query<
            (Entity, &Transform, &Team, &Health, &Intent, &MovementState, Option<&crate::core::Velocity>),
            Without<Bot>,
        >,
    ),
) {
    let brushes = brushes.as_deref().map_or(&[][..], |b| &b.0[..]);
    // Freeze time: it may look around, not walk (the rules hold its
    // intent), so standing still isn't being stuck. Grenades only while
    // the round is open (not frozen, not over).
    let frozen = round.0.is_some_and(|f| f.0);
    let open = round.1.is_none_or(|o| o.0) && !frozen;
    let dt = time.delta_secs();
    let now = time.elapsed_secs_f64();
    // World traces for flash sight (a line) and grenade arcs (the
    // grenade's box swept, half size given): characters aren't in the way.
    let no_nav = NavMesh::default();
    let filter = SpatialQueryFilter::default().with_mask(crate::core::SOLID_LAYERS);
    let not_character = |e: Entity| !others.contains(e);
    let world_trace = |a: Vec3, b: Vec3| -> Option<(Vec3, Vec3)> {
        let d = b - a;
        let dir = Dir3::new(d).ok()?;
        spatial
            .cast_ray_predicate(a, dir, d.length(), true, &filter, &not_character)
            .map(|h| (a + *dir * h.distance, h.normal))
    };
    let box_trace = |a: Vec3, b: Vec3, half: f32| -> Option<(Vec3, Vec3)> {
        let d = b - a;
        let dir = Dir3::new(d).ok()?;
        let config = ShapeCastConfig {
            max_distance: d.length(),
            ignore_origin_penetration: true,
            ..ShapeCastConfig::DEFAULT
        };
        let shape = Collider::cuboid(half * 2.0, half * 2.0, half * 2.0);
        spatial
            .cast_shape_predicate(&shape, a, Quat::IDENTITY, dir, &config, &filter, &not_character)
            .map(|h| (a + *dir * h.distance, h.normal1.normalize_or_zero()))
    };
    // Where every living character looks (teammates a flash must spare).
    // (With the bot's number, 0 for a person: who gives way in a door.)
    let view = |e: Entity, t: &Transform, team: &Team, intent: &Intent, state: &MovementState, v: Vec3, number: u32| {
        (
            e,
            *team,
            grenades::MateView {
                eye: t.translation + state.eye_offset,
                look: intent.look_rotation() * Vec3::NEG_Z,
                velocity: v,
                person: number == 0,
            },
            number,
        )
    };
    let views: Vec<(Entity, Team, grenades::MateView, u32)> = bots
        .iter()
        .filter(|b| b.6.current > 0.0)
        .map(|(e, b, intent, t, state, team, _, _, _, v, _)| view(e, t, team, intent, state, v.0, b.number.max(1)))
        .chain(
            round
                .3
                .iter()
                .filter(|p| p.3.current > 0.0)
                .map(|(e, t, team, _, intent, state, v)| view(e, t, team, intent, state, v.map_or(Vec3::ZERO, |v| v.0), 0)),
        )
        .collect();
    // What every living bot sees: the nearest living enemy in sight, into
    // `Bot::sighted` (taken in the loop below).
    //
    // Nearest first, so the first one in sight is the one: a bot in a crowd
    // traced to every enemy each tick (a full server's lobby on a minigame
    // map, all in each other's PVS: multiplayer.md "Capacity"). The same pick
    // as testing them all (ties in the query's order). An enemy whose
    // visibility cluster the bot's can't see (the map's PVS, which is
    // conservative) is hidden by the world without a trace.
    //
    // The traces are the server's capacity limit; each bot's are read-only
    // and independent of the others' results, so the bots spread over the
    // compute task pool (`par_iter_mut`; docs/performance.md, "Bots' sight
    // in parallel"). Each writes only its own `sighted`, from state nothing
    // in this system changes, so the result is what a serial loop gives
    // whatever the order the threads run in. (Part of `think`, not a system
    // of its own: another system would reorder the unordered ones around it.)
    let vis = vis.as_deref().map(|v| &*v.0);
    let clusters: bevy::ecs::entity::EntityHashMap<u32> = vis
        .map(|v| {
            others
                .iter()
                .filter(|o| o.3.current > 0.0)
                .filter_map(|(e, ot, ..)| Some((e, v.cluster_at(ot.translation + Vec3::Y * AIM_HEIGHT)?)))
                .collect()
        })
        .unwrap_or_default();
    bots.par_iter_mut().for_each(|(me, mut bot, _, t, state, team, health, blinded, _, _, _)| {
        bot.sighted = None;
        if health.current <= 0.0 || blinded.is_some_and(|b| b.alpha_at(now) > BLIND_ALPHA) {
            return;
        }
        let eye = t.translation + state.eye_offset;
        let eye_cluster = vis.and_then(|v| v.cluster_at(eye));
        let mut candidates: Vec<(Entity, Vec3, f32)> = Vec::new();
        for (e, ot, oteam, oh) in &others {
            if e == me || oteam == team || oh.current <= 0.0 {
                continue;
            }
            if let (Some(v), Some(a), Some(b)) = (vis, eye_cluster, clusters.get(&e))
                && v.visible.get(a as usize).is_some_and(|row| !row.is_empty())
                && !v.sees(a, *b)
            {
                continue;
            }
            let aim = ot.translation + Vec3::Y * AIM_HEIGHT;
            candidates.push((e, aim, (aim - eye).length()));
        }
        candidates.sort_by(|a, b| a.2.total_cmp(&b.2));
        let filter = SpatialQueryFilter::from_excluded_entities([me]).with_mask(crate::core::SOLID_LAYERS);
        for (e, aim, dist) in candidates {
            let Ok(dir) = Dir3::new(aim - eye) else { continue };
            let visible = spatial
                .cast_ray(eye, dir, dist, true, &filter)
                .is_none_or(|h| h.entity == e || h.distance >= dist - 0.05);
            // Nor through smoke (specs/cs_source/grenades.md Q16).
            if visible && !smoke.iter().any(|s| s.blocks(eye, aim)) {
                bot.sighted = Some((e, aim, dist));
                break;
            }
        }
    });
    for (me, mut bot, mut intent, t, state, team, health, blinded, mut inv, velocity, punch) in &mut bots {
        if health.current <= 0.0 {
            bot.target = None;
            bot.toss = None;
            bot.avert = None;
            bot.dead = true;
            bot.activity = Activity::Dead;
            continue;
        }
        if bot.dead {
            // Respawned.
            bot.dead = false;
            bot.new_life();
        }
        // The enemy it remembers is dead (or gone): nothing left there.
        if bot
            .lead_who
            .is_some_and(|who| !others.get(who).is_ok_and(|(_, _, _, h)| h.current > 0.0))
        {
            bot.lead = None;
            bot.lead_who = None;
        }
        let eye = t.translation + state.eye_offset;
        // A flashed bot sees nothing until the white is mostly gone, and
        // aims worse while it fades.
        let white = blinded.map_or(0.0, |b| b.alpha_at(now));
        let blind = white > BLIND_ALPHA;
        // The nearest living enemy in sight (the pass above).
        let best = bot.sighted.take();

        // Flashes in flight it sees: now and then look away until they pop.
        bot.watched.retain(|g| projectiles.contains(*g));
        if !blind {
            for (g, gt, p) in &projectiles {
                let flash = matches!(p.effect, crate::weapon::grenade::GrenadeEffect::Flash(_));
                let at = gt.translation;
                if !flash
                    || p.thrower == Some(me)
                    || bot.watched.contains(&g)
                    || at.distance(eye) > grenades::FLASH_WATCH
                    || world_trace(eye, at).is_some()
                    || smoke.iter().any(|s| s.blocks(eye, at))
                {
                    continue;
                }
                bot.watched.push(g);
                if bot.rand() < grenades::FLASH_REACT {
                    let left = (p.fuse - p.ticks as f32 * dt).max(0.0);
                    bot.avert = Some(grenades::Avert {
                        from: at,
                        until: now + left as f64 + 0.3,
                        grenade: Some(g),
                    });
                }
            }
        }
        if let Some(a) = bot.avert.as_mut() {
            match a.grenade.map(|g| projectiles.get(g)) {
                Some(Ok((_, gt, _))) => a.from = gt.translation,
                // Gone: it popped.
                Some(Err(_)) => a.until = now,
                None => {}
            }
            if now >= a.until {
                bot.avert = None;
            }
        }
        if bot.toss.is_none() && bot.plan.as_ref().is_some_and(|(_, until)| now > *until) {
            bot.plan = None;
        }
        // An enemy in sight before the pin is out: forget the throw.
        if best.is_some()
            && let Some(toss) = bot.toss.as_ref()
            && toss.pulled.is_none()
        {
            let previous = toss.previous;
            bot.toss = None;
            if let Some(inv) = inv.as_deref_mut() {
                inv.wanted = previous
                    .filter(|p| inv.weapons.contains(p))
                    .or_else(|| grenades::best_weapon(inv, &arms));
            }
        }

        // Semi-automatics fire once per press: let go every other tick.
        let pressed = intent.fire;
        let semi = inv
            .as_deref()
            .and_then(|i| i.active)
            .and_then(|w| triggers.get(w).ok())
            .is_some_and(|t| !t.automatic);
        // How much of the punch its bullets take.
        let punch_scale = inv
            .as_deref()
            .and_then(|i| i.active)
            .and_then(|w| round.2.get(w).ok())
            .map_or(0.0, |h| h.punch_scale);
        intent.fire = false;
        match best {
            Some((e, aim, _)) => {
                bot.contact = now;
                // Remember where they were (feet) for after they hide.
                bot.lead = Some((
                    aim - Vec3::Y * (AIM_HEIGHT + CAPSULE_HEIGHT / 2.0),
                    time.elapsed_secs_f64(),
                ));
                bot.lead_who = Some(e);
                if bot.target == Some(e) {
                    bot.seen += dt;
                } else {
                    bot.target = Some(e);
                    bot.seen = 0.0;
                }
                bot.wobble.1 -= dt;
                if bot.wobble.1 <= 0.0 {
                    let (a, r) = (bot.rand() * std::f32::consts::TAU, bot.rand().sqrt());
                    let error = cfg.aim_error * (1.0 + FLASHED_AIM * white);
                    bot.wobble = (Vec2::from_angle(a) * r * error.to_radians(), AIM_REROLL);
                }
                let to = aim - eye;
                // Recoil control: aimed so the bullets (eye angles plus
                // the weapon's share of the punch) stay on the target,
                // pulling down against the kick.
                let kick = punch.map_or(Vec2::ZERO, |p| p.0.truncate()) * punch_scale * cfg.recoil_control;
                let want_yaw = (-to.x).atan2(-to.z) + bot.wobble.0.x - kick.y;
                let want_pitch = to.y.atan2(to.xz().length()) + bot.wobble.0.y - kick.x;
                let step = cfg.turn_rate.to_radians() * dt;
                let dyaw = wrap(want_yaw - intent.yaw);
                intent.yaw = wrap(intent.yaw + dyaw.clamp(-step, step));
                intent.pitch += (want_pitch - intent.pitch).clamp(-step, step);
                let off = wrap(want_yaw - intent.yaw).abs().max((want_pitch - intent.pitch).abs());
                intent.fire = cfg.dont_shoot == 0
                    && !bot.frozen
                    && bot.seen >= cfg.reaction
                    && off.to_degrees() < FIRE_CONE_DEG
                    && !(semi && pressed);
            }
            None => {
                bot.target = None;
                bot.seen = 0.0;
            }
        }

        // The round closed (won, or frozen) before the pin came out: no
        // throw.
        if !open && bot.toss.as_ref().is_some_and(|t| t.pulled.is_none()) {
            let previous = bot.toss.take().and_then(|t| t.previous);
            if let Some(inv) = inv.as_deref_mut() {
                inv.wanted = previous
                    .filter(|p| inv.weapons.contains(p))
                    .or_else(|| grenades::best_weapon(inv, &arms));
            }
        }
        // A throw in progress takes over.
        if bot.toss.is_some() {
            bot.activity = Activity::Throwing;
            match inv.as_deref_mut() {
                Some(inv) => {
                    grenades::step_toss(&mut bot, &mut intent, inv, &arms, &cfg, now, dt);
                }
                None => bot.toss = None,
            }
            continue;
        }
        // Not throwing: a grenade in hand goes back for a gun.
        if let Some(inv) = inv.as_deref_mut()
            && inv
                .active
                .is_some_and(|a| arms.get(a).is_ok_and(|(_, t)| t.is_some_and(|t| !t.primed())))
            && inv.wanted.is_none()
            && let Some(gun) = grenades::best_weapon(inv, &arms)
        {
            inv.wanted = Some(gun);
            intent.fire = false;
        }
        // Looking away from a flash: turn the back to it and wait.
        if let Some(a) = bot.avert {
            let away = eye - a.from;
            let want_yaw = (-away.x).atan2(-away.z);
            let step = cfg.turn_rate.to_radians() * dt;
            intent.yaw = wrap(intent.yaw + wrap(want_yaw - intent.yaw).clamp(-step, step));
            intent.pitch -= intent.pitch.clamp(-step, step);
            intent.fire = false;
            intent.move_axis = Vec2::ZERO;
            intent.jump = false;
            bot.activity = Activity::Averting;
            continue;
        }

        // Strafe in place, switching direction every 0.4-1.2 s.
        bot.strafe_left -= dt;
        if bot.strafe_left <= 0.0 {
            bot.strafe_left = 0.4 + 0.8 * bot.rand();
            let r = bot.rand();
            bot.strafe = if r < 0.4 {
                -1.0
            } else if r < 0.8 {
                1.0
            } else {
                0.0
            };
        }
        intent.move_axis = if cfg.stop == 0 && !bot.frozen && bot.target.is_some() {
            Vec2::new(bot.strafe, 0.0)
        } else {
            Vec2::ZERO
        };
        intent.jump = false;
        intent.crouch = false;
        if bot.target.is_some() {
            bot.route.clear();
            bot.activity = Activity::Fighting;
            continue;
        }
        let feet = t.translation - Vec3::Y * CAPSULE_HEIGHT / 2.0;
        // What was seen or heard goes stale, or is where it went.
        if bot
            .lead
            .is_some_and(|(at, when)| now - when > MEMORY || at.distance(feet) < ARRIVED)
        {
            bot.lead = None;
        }
        if let Some((_, when)) = bot.lead {
            bot.contact = bot.contact.max(when);
        }
        if bot
            .assist
            .is_some_and(|(at, when)| now - when > ASSIST_TIME || at.distance(feet) < ARRIVED * 2.0)
        {
            bot.assist = None;
        }
        if bot.roam.is_some_and(|at| at.distance(feet) < ARRIVED) {
            bot.roam = None;
        }
        if cfg.stop != 0 || bot.frozen {
            bot.route.clear();
            bot.goal = None;
            bot.activity = Activity::Idle;
            continue;
        }
        // Without a mesh nothing walks, but throws still happen.
        let nav = nav.as_deref().unwrap_or(&no_nav);
        if let Some(here) = nav.area_at(feet + Vec3::Y * 0.1)
            && bot.area != Some(here)
        {
            bot.prev_area = bot.area;
            bot.area = Some(here);
            bot.entered = feet;
        }
        let plan = tactics.team(*team);
        let mates: Vec<Vec3> = others
            .iter()
            .filter(|(e, _, oteam, oh)| *e != me && *oteam == team && oh.current > 0.0)
            .map(|(_, ot, ..)| ot.translation - Vec3::Y * CAPSULE_HEIGHT / 2.0)
            .collect();
        let goal = choose_goal(&mut bot, me, feet, now, &tactics, plan, nav, &mates);
        bot.goal = goal;
        // Now and then a grenade at the remembered enemy or the site.
        if let Some(inv) = inv.as_deref()
            && cfg.grenades > 0
            && open
            && now >= bot.next_toss_check
        {
            bot.next_toss_check = now + grenades::TOSS_CHECK;
            if now >= bot.next_toss {
                // Gathering before the site: flashes onto where the group
                // comes onto it, smokes between there and the defenders
                // watching it (`tactics::smoke_spot`).
                let staging = plan.is_some_and(|p| p.staging && p.site == bot.site);
                let site = bot.site.and_then(|s| tactics.sites.get(s));
                let approach = |s: &tactics::Site| {
                    s.approaches[0]
                        .iter()
                        .copied()
                        .enumerate()
                        .min_by(|a, b| a.1.distance(feet).total_cmp(&b.1.distance(feet)))
                };
                let smoke = |s: &tactics::Site| {
                    approach(s).map_or(s.point, |(i, a)| s.smokes.get(i).copied().flatten().unwrap_or(a))
                };
                let objective = match bot.activity {
                    _ if staging => site.map(|s| (approach(s).map_or(s.point, |a| a.1), smoke(s))),
                    Activity::ToSite | Activity::Following | Activity::Waiting => site.map(|s| (s.point, smoke(s))),
                    Activity::Roaming if bot.roam_objective => bot.roam.map(|r| (r, r)),
                    _ => None,
                };
                let mates: Vec<grenades::MateView> = views
                    .iter()
                    .filter(|v| v.0 != me && v.1 == *team)
                    .map(|v| v.2)
                    .collect();
                let sight = |a: Vec3, b: Vec3| world_trace(a, b).is_none();
                consider_throw(
                    &mut bot, inv, &arms, &cfg, eye, feet, objective, staging, &mates, &sight, now, dt, &box_trace,
                );
            }
        }
        if bot.toss.is_some() {
            intent.move_axis = Vec2::ZERO;
            continue;
        }
        let params = path::CostParams {
            danger: plan.map(|p| p.danger.as_slice()).filter(|d| d.len() == nav.areas.len()),
            exposure: Some(tactics.exposure.as_slice()).filter(|e| e.len() == nav.areas.len()),
            team_seed: plan.map_or(0, |p| p.seed),
            bot_seed: bot.seed,
            stuck: &tactics.failed_links,
        };
        // Never left hanging on a ladder: with nowhere to go, it finishes
        // the climb (its route's end).
        let goal = goal.or_else(|| state.on_ladder.then(|| bot.route.last().copied()).flatten());
        if frozen {
            // Held still: no progress expected, nothing in the way.
            bot.progress = (feet, 0.0);
            bot.toward.1 = now;
            bot.blocked = 0.0;
        }
        let step = match goal {
            Some(goal) => walk_route(&mut bot, nav, feet, goal, &params, dt, now, state.on_ladder, state.on_ground, brushes),
            // Waiting keeps its route (for comparing with the leader's).
            None if bot.activity == Activity::Waiting => None,
            None => {
                bot.route.clear();
                bot.route_areas.clear();
                None
            }
        };
        // Nowhere left to go on a ladder (a goal reached from it, a step
        // above the floor): step off it.
        let step = step.or_else(|| state.on_ladder.then(|| off_ladder(nav, feet)).flatten());
        // Look at what matters (`choose_look`), turning a little slower
        // than in a fight; walk wherever the route goes, whatever the
        // view.
        let looking = intent.look_rotation() * Vec3::NEG_Z;
        let los = |a: Vec3, b: Vec3| world_trace(a, b).is_none() && !smoke.iter().any(|s| s.blocks(a, b));
        let mut look = choose_look(&mut bot, nav, eye, feet, step.map(|s| s.dir), looking, now, &los);
        let mut pitch_limit = LOOK_PITCH;
        if let Some(step) = step {
            if let Some(r) = step.rungs {
                // A ladder: square to it. Up: looking up, pushing in.
                // Down: level (the movement grabs a ladder along the
                // view), pushing in to grab it, then backing off it.
                let up = step.climb.is_some_and(|c| c.y > feet.y);
                look = eye + r.face + Vec3::Y * if up { 6.0 } else { 0.0 };
                pitch_limit = LADDER_PITCH;
            } else if let Some(rung) = step.climb {
                // A climb or drop with no ladder: face it, looking up (or
                // down) the way to go.
                look = rung + Vec3::Y * (rung.y - feet.y).signum() * 2.0;
                pitch_limit = LADDER_PITCH;
            } else if step.dismount {
                // Off a ladder's top: level, the way out (on the ladder:
                // square to it, `rungs`, above).
                look = eye + step.dir * 2.0;
            } else if bot.blocked > USE_AFTER {
                // Stuck: face the way (a door to open, a ledge to jump).
                look = eye + step.dir * 2.0;
            }
        }
        bot.look_at = Some(look);
        let to = look - eye;
        let want_yaw = (-to.x).atan2(-to.z);
        let want_pitch = to.y.atan2(to.xz().length()).clamp(-pitch_limit, pitch_limit);
        let turn = cfg.turn_rate.to_radians() * SCAN_TURN * dt;
        intent.yaw = wrap(intent.yaw + wrap(want_yaw - intent.yaw).clamp(-turn, turn));
        intent.pitch += (want_pitch - intent.pitch).clamp(-turn, turn);
        if let Some(step) = step {
            // Keep off teammates and other teamless characters
            // (hostages) close by (they block each other in doorways
            // otherwise).
            let mut dir = step.dir;
            let near = others
                .iter()
                .filter(|(e, _, oteam, oh)| *e != me && *oteam == team && oh.current > 0.0)
                .map(|(_, ot, ..)| ot)
                .chain(teamless.iter().filter(|(_, h)| h.current > 0.0).map(|(t, _)| t));
            for ot in near {
                let away = (t.translation - ot.translation).with_y(0.0);
                let d = away.length();
                if d < PERSONAL_SPACE && (ot.translation.y - t.translation.y).abs() < 1.5 {
                    let push = if d > 1e-3 { away / d } else { Vec3::X };
                    dir += push * (1.0 - d / PERSONAL_SPACE) * SEPARATION;
                }
            }
            // Teammates in the way (`give_way`): keep right of one coming
            // the other way (and wait for it, giving way), step round one
            // standing there, don't run into one walking ahead.
            let mut pace = step.pace;
            if step.rungs.is_none() && step.climb.is_none() {
                let mine = views.iter().find(|v| v.0 == me).map_or(0, |v| v.3);
                let feet_eye = t.translation + state.eye_offset;
                for v in views.iter().filter(|v| v.0 != me && v.1 == *team) {
                    let rel = v.2.eye - feet_eye;
                    let Some(way) = give_way(step.dir, rel, v.2.velocity, mine, v.3, bot.number) else {
                        continue;
                    };
                    dir += way.side;
                    pace = pace.min(way.pace);
                }
            }
            let dir = dir.with_y(0.0).normalize_or(step.dir);
            let rot = intent.yaw_rotation();
            let (fwd, right) = (rot * Vec3::NEG_Z, rot * Vec3::X);
            let axis = Vec2::new(dir.dot(right), dir.dot(fwd));
            // Full speed whichever way (the larger key fully pressed); on
            // a ladder, straight at it.
            intent.move_axis = match step.rungs {
                // Stepping off its top sideways: climbing on, and the
                // sideways key toward the way out.
                Some(r) if state.on_ladder && step.dismount => {
                    let perp = Vec3::Y.cross(r.normal);
                    let across = step.target.map_or(0.0, |t| (t - feet).dot(perp));
                    let side = if across.abs() > LADDER_CENTRE {
                        (perp * across.signum()).dot(right).signum()
                    } else {
                        0.0
                    };
                    if fwd.with_y(0.0).normalize_or_zero().dot(r.face) < LADDER_FACING {
                        Vec2::ZERO
                    } else {
                        Vec2::new(side, 1.0)
                    }
                }
                // On the ladder: straight on, stepping sideways back to
                // its middle line.
                Some(r) if state.on_ladder => {
                    let perp = Vec3::Y.cross(r.normal);
                    let off = (feet - r.foot).dot(perp) - r.shift;
                    // (Exactly on a line off the middle: `Bot::ladder_shift`.)
                    let centre = if r.shift == 0.0 { LADDER_CENTRE } else { LADDER_ON_SHIFT };
                    // (Not near the top going up: the ladder lets go with
                    // the sideways speed still on, flinging it off the top.)
                    let near_top = step.climb.is_some_and(|c| c.y > feet.y && c.y - feet.y < LADDER_TOP_STEADY);
                    let side = if off.abs() > centre && !near_top {
                        -(perp * off.signum()).dot(right).signum()
                    } else {
                        0.0
                    };
                    let up = step.climb.is_some_and(|c| c.y > feet.y);
                    if fwd.with_y(0.0).normalize_or_zero().dot(r.face) < LADDER_FACING {
                        // Turned away (aiming at something): hang there
                        // until square again (keys move it along the view).
                        Vec2::ZERO
                    } else {
                        Vec2::new(side, if up { 1.0 } else { -1.0 })
                    }
                }
                // A drop with no mesh ladder: straight on, over the edge.
                // A climb: straight on once facing it (still turning,
                // toward it: forward alone circles it).
                _ if step.rungs.is_none()
                    && step.climb.is_some_and(|c| c.y < feet.y || axis.y > CLIMB_FACING) =>
                {
                    Vec2::Y
                }
                _ => axis / axis.abs().max_element().max(1e-3) * pace,
            };
            // Walking into something: press use now and then (doors),
            // jump it every so often (ledges).
            if velocity.0.xz().length() < BLOCKED_SPEED && !frozen {
                bot.blocked += dt;
            } else {
                bot.blocked = 0.0;
            }
            let b = bot.blocked;
            intent.use_key = b > USE_AFTER && (b / USE_PERIOD) as u32 % 2 == 0;
            let blocked = b > BLOCKED_JUMP && (b - BLOCKED_JUMP) % JUMP_PERIOD < dt * 1.5;
            // Jump is a fresh press each time (held, it jumps once), and
            // never on a ladder (that lets go of it); blocked at one (slow,
            // pressing in or sidestepping at its foot) it doesn't jump
            // either (the jump and the duck after it keep it off).
            let want_jump = ((step.jump && step.climb.is_none()) || (blocked && step.rungs.is_none())) && !state.on_ladder;
            intent.jump = want_jump && !bot.jump_held;
            bot.jump_held = intent.jump;
            if intent.jump && state.on_ground {
                bot.jumped = now;
            }
            // Duck in the air after a jump (a duck-jump clears higher
            // ledges, as CS:S bots do), and under a low ceiling ahead
            // (vents).
            let airborne = !state.on_ground && now - bot.jumped > 0.05 && now - bot.jumped < DUCK_JUMP;
            let ahead = feet + step.dir * LOW_AHEAD;
            let low = |from: Vec3| {
                // Not from inside something (a stair ahead).
                spatial
                    .cast_ray_predicate(
                        from + Vec3::Y * LOW_FROM,
                        Dir3::Y,
                        LOW_CEILING - LOW_FROM,
                        false,
                        &filter,
                        &not_character,
                    )
                    .is_some_and(|h| h.distance > 0.02)
            };
            // (Not for what's ahead when getting on a ladder starting above
            // the floor: that is the underside of what it climbs, a train
            // car, and a ducked box doesn't reach the ladder.)
            let high_ladder = step.rungs.is_some_and(|r| r.foot.y - feet.y > STEP_HEIGHT);
            let low_ceiling = !state.on_ladder && (low(feet) || (low(ahead) && !high_ladder));
            intent.crouch = step.crouch || airborne || low_ceiling;
            // Something breakable in the way (a vent grille, a window):
            // shoot it.
            // How long it has been walking to this route point.
            if let Some(target) = step.target
                && bot.toward.0.distance(target) > 0.3
            {
                bot.toward = (target, now);
            }
            let slow = now - bot.toward.1 > BREAK_SLOW;
            if (b > BREAK_AFTER || slow || step.dismount)
                && cfg.dont_shoot == 0
                && !bot.frozen
                && let Some(target) = step.target
            {
                // Along the way at a few heights, and at the route point.
                let ahead = feet + step.dir * 0.6;
                let aims = [
                    ahead + Vec3::Y * 0.1,
                    ahead + Vec3::Y * 0.5,
                    ahead + Vec3::Y * 1.0,
                    ahead + Vec3::Y * 1.5,
                    target + Vec3::Y * 0.3,
                ];
                let filter = SpatialQueryFilter::from_excluded_entities([me]).with_mask(crate::core::SOLID_LAYERS);
                // (No further than the route point: not a window past it.)
                let reach = BREAK_REACH.min(target.distance(eye) + BREAK_PAST);
                let hit = aims.into_iter().find_map(|aim| {
                    let dir = Dir3::new(aim - eye).ok()?;
                    spatial
                        .cast_ray(eye, dir, reach, true, &filter)
                        .filter(|h| breakable.contains(h.entity))
                        .map(|h| eye + *dir * h.distance)
                });
                if let Some(at) = hit {
                    let d = at - eye;
                    intent.yaw = (-d.x).atan2(-d.z);
                    intent.pitch = d.y.atan2(d.xz().length());
                    intent.fire = !(semi && pressed);
                    bot.look_at = Some(at);
                    if state.on_ladder {
                        // Hang on the ladder until it breaks.
                        intent.move_axis = Vec2::ZERO;
                    }
                }
            }
        } else {
            bot.blocked = 0.0;
            intent.use_key = false;
        }
    }
}

/// Teammates within this ahead (flat, m), and within this of the line
/// walked, are in the way (`give_way`).
pub(super) const WAY_AHEAD: f32 = 2.2;
pub(super) const WAY_WIDE: f32 = 0.9;
/// Faster than this toward the walker: coming the other way; slower than
/// this: standing (m/s).
pub(super) const ONCOMING: f32 = 1.0;
pub(super) const STANDING: f32 = 0.6;
/// How hard it steps aside, and the share of full speed it keeps while
/// giving way, behind someone slower, or passing one standing.
pub(super) const SIDESTEP: f32 = 1.2;
pub(super) const YIELD_PACE: f32 = 0.15;
pub(super) const BEHIND_PACE: f32 = 0.5;

/// How a walker gives way to a teammate (ours; CS:S's bots do too, how
/// isn't public).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Way {
    /// Added to the walking direction (flat).
    pub side: Vec3,
    /// Share of full speed at most.
    pub pace: f32,
}

/// What a bot walking `dir` (flat, unit) does about a teammate at `rel`
/// (its eye less the bot's, so the same height counts) moving at
/// `velocity`: nothing if it isn't in the way; coming the other way, both
/// keep right, and the one who gives way (to a person always, else to the
/// lower bot number; `mine` and `theirs` are bot numbers, 0 a person)
/// nearly stops; standing there, it steps round on the side with more of
/// the way (`number`'s parity breaks a tie); walking ahead the same way,
/// it slows down rather than shove.
pub fn give_way(dir: Vec3, rel: Vec3, velocity: Vec3, mine: u32, theirs: u32, number: u32) -> Option<Way> {
    if rel.y.abs() > 1.2 {
        return None;
    }
    let flat = rel.with_y(0.0);
    let ahead = flat.dot(dir);
    let right = dir.cross(Vec3::Y);
    let across = flat.dot(right);
    if ahead <= 0.0 || ahead > WAY_AHEAD || across.abs() > WAY_WIDE {
        return None;
    }
    let v = velocity.with_y(0.0);
    let toward = -v.dot(dir);
    if toward > ONCOMING {
        let yields = theirs == 0 || (mine != 0 && mine > theirs);
        return Some(Way {
            side: right * SIDESTEP,
            pace: if yields { YIELD_PACE } else { 1.0 },
        });
    }
    if v.length() < STANDING {
        let side = if across.abs() > 0.05 {
            -across.signum()
        } else if number % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        return Some(Way {
            side: right * side * SIDESTEP,
            pace: 1.0,
        });
    }
    Some(Way {
        side: Vec3::ZERO,
        pace: if ahead < WAY_AHEAD * 0.6 { BEHIND_PACE } else { 1.0 },
    })
}

/// A climb with no mesh ladder is walked straight on once the way there
/// is within this cosine of the view.
pub(super) const CLIMB_FACING: f32 = 0.9;
/// Seconds a bot answers a teammate's call.
pub(super) const ASSIST_TIME: f64 = 20.0;
/// Teammates closer than this push a walking bot aside, this hard, m.
pub(super) const PERSONAL_SPACE: f32 = 1.2;
pub(super) const SEPARATION: f32 = 1.5;
/// Seconds after a jump it ducks while in the air.
pub(super) const DUCK_JUMP: f64 = 0.8;
/// A ceiling lower than this above the feet, here or this far ahead,
/// makes a walking bot crouch (traced from `LOW_FROM` up), m.
pub(super) const LOW_CEILING: f32 = 73.0 * 0.0254;
pub(super) const LOW_AHEAD: f32 = 0.6;
pub(super) const LOW_FROM: f32 = 0.5;
/// Blocked this long: shoot a breakable within this in the way, s and m.
pub(super) const BREAK_AFTER: f32 = 0.5;
/// Walking to one route point this long also looks for one, s.
pub(super) const BREAK_SLOW: f64 = 2.0;
pub(super) const BREAK_REACH: f32 = 2.5;
/// ... and no more than this past the route point it walks to, m.
pub(super) const BREAK_PAST: f32 = 0.5;
/// Slower than this while walking for this long: jump, m/s and s.
pub(super) const BLOCKED_SPEED: f32 = 0.5;
pub(super) const BLOCKED_JUMP: f32 = 0.4;
pub(super) const JUMP_PERIOD: f32 = 0.8;
/// Blocked this long: face the way and press use every other
/// `USE_PERIOD` (doors open on a press), s.
pub(super) const USE_AFTER: f32 = 0.25;
pub(super) const USE_PERIOD: f32 = 0.15;
/// A route point at least this much above or below, this close (flat),
/// is up or down a ladder, m.
pub(super) const LADDER_RISE: f32 = 1.2;
pub(super) const LADDER_REACH: f32 = 1.5;
/// Going up a ladder starts from in front of it: the direction to its
/// foot within this (cosine) of straight at it; else to the point this
/// far out in front, m (spec: 0.9, 2 × half hull).
pub(super) const LADDER_LINED_UP: f32 = -0.9;
/// Pressing at a ladder's foot this long (s) without getting on, within
/// this distance (m), a bot steps this far (m) to one side of its middle,
/// switching sides every `LADDER_SIDESTEP_EACH` seconds.
pub(super) const LADDER_SIDESTEP_AFTER: f32 = 0.6;
pub(super) const LADDER_SIDESTEP_NEAR: f32 = 0.6;
pub(super) const LADDER_SIDESTEP: f32 = 0.25;
pub(super) const LADDER_SIDESTEP_EACH: f32 = 1.2;
pub(super) const LADDER_MOUNT: f32 = 32.0 * 0.0254;
/// A ladder's mount point (at its foot's height) counts as reached from
/// the floor below it up to this far below, m (a standing box reaches a
/// ladder starting up to 72 units up).
pub(super) const MOUNT_ABOVE: f32 = 72.0 * 0.0254;
/// Going down, it backs out over the ladder's top until this far beyond
/// it, then presses in (falling past it, it catches it), m.
pub(super) const LADDER_OVER: f32 = 0.45;
/// On a ladder, a route point is reached with the feet this close to its
/// height. Feet moving no more than `LADDER_RISING` in `LADDER_TOPPED`
/// seconds short of that: stuck (under something going up: it sidesteps,
/// `Bot::ladder_shift`), m and s.
pub(super) const LADDER_TOP_NEAR: f32 = 0.3;
pub(super) const LADDER_RISING: f32 = 0.01;
pub(super) const LADDER_TOPPED: f32 = 0.25;
/// Off a ladder's top toward a point within this (cosine) of straight
/// over it, it turns and walks there; else it steps off sideways.
pub(super) const LADDER_OVER_TOP: f32 = 0.3;
/// On a ladder, it climbs only while its view is within this (cosine) of
/// square to it.
pub(super) const LADDER_FACING: f32 = 0.85;
/// Climbing up within this of the top, it no longer steps sideways, m.
pub(super) const LADDER_TOP_STEADY: f32 = 0.6;
/// Pitch limit looking up or down a ladder (radians).
pub(super) const LADDER_PITCH: f32 = 1.4;
/// On a ladder further than this from its middle line (or this from a
/// line off it it keeps to): step sideways, m.
pub(super) const LADDER_CENTRE: f32 = 0.1;
pub(super) const LADDER_ON_SHIFT: f32 = 0.04;
/// Sidestepping at a ladder's foot: this close to the line it gets on
/// along, it presses in, m.
pub(super) const LADDER_ACROSS: f32 = 0.03;
/// Within this of that line it slows, down to this share of full speed.
pub(super) const LADDER_SLOW: f32 = 0.2;
pub(super) const LADDER_PACE: f32 = 0.2;
/// Half the box's width, and how far past a flank's face it keeps the box
/// on a ladder boxed in by flush faces, m.
pub(super) const HALF_WIDTH: f32 = 16.0 * 0.0254;
pub(super) const LADDER_CLEAR: f32 = 2.0 * 0.0254;
/// Seconds without contact a bot holds before moving on: attackers on a
/// site they took go to the next one, defenders go hunting.
pub(super) const ATTACK_PATIENCE: (f64, f64) = (20.0, 35.0);
pub(super) const DEFEND_PATIENCE: (f64, f64) = (45.0, 80.0);
/// Attackers chase what they saw or heard within this; holders within
/// this of their spot, m.
pub(super) const CHASE_ATTACK: f32 = 25.0;
pub(super) const CHASE_HOLD: f32 = 15.0;
/// A follower farther than this from the leader goes to the leader; one
/// with this much less route left than the leader, and this far from it,
/// waits, m.
pub(super) const FOLLOW_FAR: f32 = 10.0;
pub(super) const AHEAD: f32 = 4.0;
pub(super) const AHEAD_DIST: f32 = 5.0;
/// Gathering before the site, followers come this close to the leader, m.
pub(super) const STAGE_CLOSE: f32 = 3.5;
/// At its hold spot within this, m.
pub(super) const HOLD_REACHED: f32 = 0.75;
/// Within this of its spot, a bot holds where it is when a teammate
/// stands within `HOLD_TAKEN` of the spot or it has been blocked for
/// `HOLD_BLOCKED` seconds (bots shoulder each other otherwise), m.
pub(super) const HOLD_SHARE: f32 = 2.5;
pub(super) const HOLD_TAKEN: f32 = 1.0;
pub(super) const HOLD_BLOCKED: f32 = 1.0;
/// Looks at a remembered enemy within this for this long, m and s.
pub(super) const LEAD_LOOK: (f32, f64) = (40.0, 8.0);
/// Looks this far ahead along its route with nothing else to look at, m.
pub(super) const LOOK_AHEAD: f32 = 6.0;
/// The view stays within this of the walking direction (radians).
pub(super) const LOOK_SPREAD: f32 = 2.0;
/// Pitch limit when not fighting (radians), and the turn rate's share.
pub(super) const LOOK_PITCH: f32 = 0.45;
pub(super) const SCAN_TURN: f32 = 0.6;
/// Eye height looked at on watched approaches and the route, m.
pub(super) const WATCH_LIFT: f32 = 1.3;

/// Where a bot goes with nobody in sight, by its orders (None: stay),
/// setting its `activity`.
#[allow(clippy::too_many_arguments)]
pub(super) fn choose_goal(
    bot: &mut Bot,
    me: Entity,
    feet: Vec3,
    now: f64,
    tactics: &Tactics,
    plan: Option<&tactics::TeamPlan>,
    nav: &NavMesh,
    mates: &[Vec3],
) -> Option<Vec3> {
    if let Some(at) = bot.move_to {
        bot.activity = Activity::Sent;
        // (Not from on top of something beside it: a lip by a ladder's top.)
        return ((at - feet).xz().length() > ARRIVED * 0.5 || (at.y - feet.y).abs() > SENT_RISE).then_some(at);
    }
    // The bomb to plant or defuse comes first (`objectives`).
    if let Some(at) = bot.objective {
        bot.activity = Activity::ToSite;
        return Some(at);
    }
    // A spot the bomb keeps it at: there, watching; an enemy remembered
    // near it, after them.
    bot.guard_at = false;
    if let Some((g, act)) = bot.guard.clone() {
        if let Some((at, _)) = bot.lead
            && at.distance(g.spot) < CHASE_HOLD
        {
            bot.activity = Activity::Chasing;
            return Some(at);
        }
        bot.activity = act;
        if (g.spot - feet).xz().length() > objectives::GUARD_NEAR || (g.spot.y - feet.y).abs() > 1.5 {
            // A teammate keeps it from the spot (one defusing in a
            // corner): the next spot.
            if bot.blocked > HOLD_BLOCKED && mates.iter().any(|m| m.distance(feet) < 1.6) {
                bot.guard_skip += 1;
                bot.blocked = 0.0;
            }
            return Some(g.spot);
        }
        bot.guard_at = true;
        return None;
    }
    // A teammate's radio command.
    if let Some(goal) = radio::order_goal(bot, feet) {
        return goal;
    }
    let role = bot.orders.role;
    let site = bot.site.and_then(|s| tactics.sites.get(s));
    // A remembered enemy, if the role lets it go there.
    if let Some((at, _)) = bot.lead {
        let chase = match (role, &bot.hold) {
            _ if bot.hunting || site.is_none() => true,
            (Role::Roam, _) => true,
            (_, Some(h)) => at.distance(h.spot) < CHASE_HOLD,
            (Role::Attack, None) => at.distance(feet) < CHASE_ATTACK,
            (Role::Defend, None) => true,
        };
        if chase {
            bot.activity = Activity::Chasing;
            return Some(at);
        }
    }
    if let Some((at, _)) = bot.assist {
        bot.activity = Activity::Assisting;
        return Some(at);
    }
    let Some(site) = site.filter(|_| !bot.hunting && role != Role::Roam) else {
        // Roam: mostly to the objectives.
        if bot.roam.is_none() && !nav.areas.is_empty() {
            let objectives = &tactics.objectives;
            let at_objective = !objectives.is_empty() && bot.rand() < OBJECTIVE_SHARE;
            bot.roam = Some(if at_objective {
                let k = (bot.rand() * objectives.len() as f32) as usize % objectives.len();
                objectives[k]
            } else {
                let i = (bot.rand() * nav.areas.len() as f32) as usize % nav.areas.len();
                nav.areas[i].center
            });
            bot.roam_objective = at_objective;
            bot.repath = 0.0;
        }
        bot.activity = if bot.hunting {
            Activity::Hunting
        } else {
            Activity::Roaming
        };
        return bot.roam;
    };
    if bot.hold.is_none() {
        if role == Role::Attack && feet.distance(site.point) >= tactics::ARRIVE_RADIUS {
            // On the way, as a group (told to go: without waiting).
            if let Some(plan) = plan.filter(|p| p.site == bot.site && p.leader.is_some() && !bot.urgent(now)) {
                if plan.leader == Some(me) {
                    if plan.leader_wait {
                        bot.activity = Activity::Waiting;
                        return None;
                    }
                } else if let Some(lf) = plan.leader_feet {
                    let d = lf.distance(feet);
                    if plan.staging {
                        // Gathering: close to the leader, then wait.
                        if d > STAGE_CLOSE {
                            bot.activity = Activity::Following;
                            return Some(lf);
                        }
                        bot.activity = Activity::Waiting;
                        return None;
                    }
                    if d > FOLLOW_FAR {
                        bot.activity = Activity::Following;
                        return Some(lf);
                    }
                    if d > AHEAD_DIST && !bot.route.is_empty() && bot.route_left() + AHEAD < plan.leader_left {
                        bot.activity = Activity::Waiting;
                        return None;
                    }
                }
            }
            bot.activity = Activity::ToSite;
            return Some(site.point);
        }
        // There (or a defender with no spot): hold somewhere on it.
        let rank = bot.orders.rank;
        bot.hold = tactics::attacker_hold(site, rank)
            .filter(|_| role == Role::Attack)
            .or(Some(Hold {
                spot: site.point,
                watch: site.approaches[0].clone(),
            }));
        bot.hold_since = None;
    }
    let spot = bot.hold.as_ref().map(|h| h.spot)?;
    let reach = if bot.hold_since.is_some() {
        HOLD_REACHED * 3.0
    } else {
        HOLD_REACHED
    };
    if (spot - feet).xz().length() > reach || (spot.y - feet.y).abs() > 1.5 {
        // Nearly there, but a teammate stands on the spot or keeps it
        // from being reached: hold from here instead.
        let close = (spot - feet).xz().length() < HOLD_SHARE && (spot.y - feet.y).abs() < 1.0;
        let taken = mates.iter().any(|m| (*m - spot).xz().length() < HOLD_TAKEN);
        if close && (taken || bot.blocked > HOLD_BLOCKED) {
            if let Some(h) = bot.hold.as_mut() {
                h.spot = feet;
            }
        } else {
            bot.activity = Activity::ToHold;
            return Some(spot);
        }
    }
    let since = *bot.hold_since.get_or_insert(now);
    if now - since.max(bot.contact) > bot.patience {
        // Nothing happens here: move on.
        bot.hold = None;
        bot.hold_since = None;
        if role == Role::Attack && !tactics.sites.is_empty() {
            bot.site = bot.site.map(|s| (s + 1) % tactics.sites.len());
        } else {
            bot.hunting = true;
        }
        bot.contact = now;
    }
    bot.activity = Activity::Holding;
    None
}

/// Where a bot looks with nobody in sight: a remembered enemy nearby;
/// holding, the approaches it watches (switching now and then); else a
/// hiding spot coming into view (`look::corner`), else a few meters
/// ahead along its route, kept within `LOOK_SPREAD` of the walking
/// direction.
#[allow(clippy::too_many_arguments)]
pub(super) fn choose_look(
    bot: &mut Bot,
    nav: &NavMesh,
    eye: Vec3,
    feet: Vec3,
    walk: Option<Vec3>,
    looking: Vec3,
    now: f64,
    los: &dyn Fn(Vec3, Vec3) -> bool,
) -> Vec3 {
    let ahead = |bot: &Bot| -> Option<Vec3> {
        let mut left = LOOK_AHEAD;
        let mut at = feet;
        for p in bot.route.iter().skip(bot.next) {
            let d = at.distance(*p);
            if d >= left {
                return Some(at + (*p - at) * (left / d) + Vec3::Y * WATCH_LIFT);
            }
            left -= d;
            at = *p;
        }
        (at != feet).then_some(at + Vec3::Y * WATCH_LIFT)
    };
    let within_spread = |p: Vec3| {
        walk.is_none_or(|w| {
            let d = (p - eye).with_y(0.0).normalize_or_zero();
            d == Vec3::ZERO || d.angle_between(w) <= LOOK_SPREAD
        })
    };
    if let Some((at, when)) = bot.lead
        && now - when < LEAD_LOOK.1
        && at.distance(feet) < LEAD_LOOK.0
    {
        let p = at + Vec3::Y * WATCH_LIFT;
        if within_spread(p) {
            return p;
        }
    }
    let watching = if bot.guard_at {
        bot.guard.as_ref().map(|g| &g.0)
    } else if bot.activity == Activity::Holding {
        bot.hold.as_ref()
    } else {
        None
    };
    if let Some(n) = watching.map(|h| h.watch.len())
        && n > 0
    {
        if now >= bot.watch.1 {
            // Mostly the nearest approach.
            let i = if bot.rand() < 0.6 {
                0
            } else {
                (bot.rand() * n as f32) as usize % n
            };
            bot.watch = (i, now + 2.5 + 3.0 * bot.rand() as f64);
        }
        let h = if bot.guard_at { &bot.guard.as_ref().unwrap().0 } else { bot.hold.as_ref().unwrap() };
        return h.watch[bot.watch.0 % n] + Vec3::Y * WATCH_LIFT;
    }
    if let Some(spot) = look::corner(bot, nav, eye, feet, walk, looking, now, los) {
        let p = spot + Vec3::Y * look::SPOT_LIFT;
        if within_spread(p) {
            return p;
        }
    }
    if let Some(p) = ahead(bot) {
        return p;
    }
    match walk {
        Some(w) => eye + w * LOOK_AHEAD,
        None => eye + looking * LOOK_AHEAD,
    }
}

/// Aim error grows by this factor times the flash's whiteness (below the
/// blind threshold).
pub(super) const FLASHED_AIM: f32 = 6.0;

/// Maybe start a grenade throw: at the remembered enemy (an HE or a
/// flash), else at the objective being walked to (`objective`: where a
/// flash goes, and where a smoke goes), if an arc ends close enough to it.
/// A flash that would blind a teammate (`grenades::flashes_mate`) is aimed
/// deeper once, else not thrown.
#[allow(clippy::too_many_arguments)]
pub(super) fn consider_throw(
    bot: &mut Bot,
    inv: &Inventory,
    arms: &grenades::Arms,
    cfg: &BotConfig,
    eye: Vec3,
    feet: Vec3,
    objective: Option<(Vec3, Vec3)>,
    staging: bool,
    mates: &[grenades::MateView],
    sight: &dyn Fn(Vec3, Vec3) -> bool,
    now: f64,
    dt: f32,
    sweep: &dyn Fn(Vec3, Vec3, f32) -> Option<(Vec3, Vec3)>,
) {
    use crate::weapon::grenade::{GrenadeEffect, GrenadeKind};
    let held = grenades::held(inv, arms);
    if held.is_empty() {
        return;
    }
    let always = cfg.grenades >= 2;
    let in_range = |at: Vec3| {
        let d = (at - feet).xz().length();
        d >= grenades::TOSS_RANGE.0 && d <= grenades::TOSS_RANGE.1
    };
    // Only where an enemy was lately (it moves on, or died).
    let lead = bot.lead.filter(|(_, when)| now - when <= grenades::TOSS_LEAD_MEMORY);
    // What to throw where, in order of preference.
    let choices: [(GrenadeKind, Vec3); 2] = match (lead, objective) {
        (Some((at, _)), _) if in_range(at) => {
            if !(always || bot.rand() < grenades::TOSS_CHANCE_LEAD) {
                return;
            }
            // Mostly an HE when there's a choice.
            if bot.rand() < 0.65 {
                [(GrenadeKind::Blast, at), (GrenadeKind::Flash, at)]
            } else {
                [(GrenadeKind::Flash, at), (GrenadeKind::Blast, at)]
            }
        }
        (_, Some((at, smoke))) if staging && (in_range(at) || in_range(smoke)) => {
            if !(always || bot.rand() < grenades::TOSS_CHANCE_STAGING) {
                return;
            }
            [(GrenadeKind::Flash, at), (GrenadeKind::Smoke, smoke)]
        }
        (None, Some((at, smoke))) if in_range(at) || in_range(smoke) => {
            if !(always || bot.rand() < grenades::TOSS_CHANCE_OBJECTIVE) {
                return;
            }
            [(GrenadeKind::Smoke, smoke), (GrenadeKind::Flash, at)]
        }
        _ => return,
    };
    let Some((weapon, t, target)) = choices.iter().filter(|(_, at)| in_range(*at)).find_map(|(k, at)| {
        held.iter()
            .find(|(_, t)| t.effect.kind() == *k)
            .map(|(w, t)| (*w, *t, *at))
    }) else {
        return;
    };
    let kind = t.effect.kind();
    let plan_at = |target: Vec3| {
        grenades::plan_throw(
            kind,
            &t.throw,
            &t.flight,
            grenades::fuse_time(t.fuse, t.flight.check_interval, dt),
            kind == GrenadeKind::Smoke,
            eye,
            target + Vec3::Y * grenades::aim_lift(kind),
            &mut |a, b| sweep(a, b, t.flight.half),
        )
        .filter(|p| p.error <= grenades::tolerance(kind))
    };
    let mut plan = plan_at(target);
    if let GrenadeEffect::Flash(fl) = &t.effect {
        let blinds = |p: &grenades::GrenadePlan| grenades::flashes_mate(p, fl.model, mates, sight);
        if plan.as_ref().is_some_and(blinds) {
            // Deeper, popping farther from the team (often out of its
            // sight); else not at all.
            let deeper = target + (target - feet).with_y(0.0).normalize_or_zero() * grenades::FLASH_DEEPER;
            plan = plan_at(deeper).filter(|p| !blinds(p));
            if plan.is_none() {
                bot.flashes_held += 1;
                bot.next_toss = now + 1.0;
                return;
            }
        }
    }
    match plan {
        Some(plan) => {
            let (lo, hi) = grenades::TOSS_COOLDOWN;
            bot.next_toss = now + lo + (hi - lo) * bot.rand() as f64;
            bot.plan = Some((plan.clone(), now + grenades::PLAN_SHOWN));
            bot.toss = Some(grenades::Toss {
                weapon,
                previous: inv.active,
                plan,
                started: now,
                pulled: None,
                released: false,
            });
        }
        // No good arc from here: look again in a while.
        None => bot.next_toss = now + 2.0,
    }
}

/// A route point counts as reached within this distance (2D), m.
pub(super) const REACHED: f32 = 25.0 * 0.0254;
/// Seconds between route recomputations while walking.
pub(super) const REPATH_SECONDS: f32 = 1.0;
/// Without this much progress in `STUCK_SECONDS`, repath (and jump).
pub(super) const STUCK_DISTANCE: f32 = 0.5;
pub(super) const STUCK_SECONDS: f32 = 1.5;
/// That many times in a row: the link it is on costs more for
/// `STUCK_MEMORY` seconds (`path::STUCK_PENALTY`).
pub(super) const STUCK_MARK: u32 = 2;
pub(super) const STUCK_MEMORY: f64 = 25.0;

/// The nav link a bot is trying to cross: from the area it stands in to
/// the next different area its route leads into.
pub(super) fn stuck_link(bot: &Bot, nav: &NavMesh, feet: Vec3) -> Option<(usize, usize)> {
    let from = nav.area_at(feet + Vec3::Y * 0.1).or(bot.area)?;
    let to = bot.route_areas.iter().skip(bot.next).find(|a| **a != from)?;
    nav.areas[from].links.iter().any(|l| l.0 == *to).then_some((from, *to))
}

/// Crossing points on edges wider than this lean aside, up to this share
/// of the way to an end, per bot and edge (spreads a group out), m.
pub(super) const LEAN_WIDTH: f32 = 2.0;
pub(super) const LEAN: f32 = 0.45;

/// One tick's walking: direction (flat, unit), jump and crouch.
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub(super) dir: Vec3,
    /// The route point walked to.
    pub(super) target: Option<Vec3>,
    pub(super) jump: bool,
    pub(super) crouch: bool,
    /// The point up or down a ladder it climbs to.
    pub(super) climb: Option<Vec3>,
    /// The mesh's ladder it climbs.
    pub(super) rungs: Option<Rungs>,
    /// Stepping off a ladder's top toward `target`: still on the ladder
    /// it hangs there to shoot a breakable in the way (vent grilles).
    pub(super) dismount: bool,
    /// Share of full speed (lining up at a ladder's foot).
    pub(super) pace: f32,
}

/// A ladder being climbed: its foot (bottom going up, top going down),
/// flat outward normal, and the way to face.
#[derive(Clone, Copy, Debug)]
pub(super) struct Rungs {
    pub(super) foot: Vec3,
    pub(super) normal: Vec3,
    pub(super) face: Vec3,
    /// Climb this far off its middle line (along `Y × normal`), m.
    pub(super) shift: f32,
}

/// Walk the navigation mesh toward `goal` (feet positions) by the bot's
/// path cost: the direction to the next route point.
#[allow(clippy::too_many_arguments)]
pub(super) fn walk_route(
    bot: &mut Bot,
    nav: &NavMesh,
    feet: Vec3,
    goal: Vec3,
    params: &path::CostParams,
    dt: f32,
    now: f64,
    on_ladder: bool,
    on_ground: bool,
    brushes: &[MapBrush],
) -> Option<Step> {
    // Mid-ladder or in the air the route stays (a new one would start
    // from the area below or above, back at a ladder's foot or top),
    // unless it is stuck there.
    if on_ground && !on_ladder {
        bot.repath -= dt;
    }
    bot.progress.1 += dt;
    let mut stuck = false;
    if bot.progress.1 >= STUCK_SECONDS {
        stuck = feet.distance(bot.progress.0) < STUCK_DISTANCE;
        bot.progress = (feet, 0.0);
        if stuck {
            bot.stuck_count += 1;
            if bot.stuck_count >= STUCK_MARK
                && let Some(link) = stuck_link(bot, nav, feet)
            {
                // Still stuck after a repath and a jump: avoid the link
                // it keeps failing for a while.
                debug!("bot stuck at {feet:.1} on link {link:?}: walking around it");
                bot.stuck_links.retain(|l| l.0 != link);
                bot.stuck_links.push((link, now + STUCK_MEMORY));
                bot.stuck_count = 0;
            }
        } else {
            bot.stuck_count = 0;
        }
    }
    bot.stuck_links.retain(|l| l.1 > now);
    if bot.repath <= 0.0 || bot.next >= bot.route.len() || stuck {
        bot.repath = REPATH_SECONDS;
        let seed = bot.seed;
        let links: Vec<(usize, usize)> = bot
            .stuck_links
            .iter()
            .map(|l| l.0)
            .chain(params.stuck.iter().copied())
            .collect();
        let params = &path::CostParams { stuck: &links, ..*params };
        let found = nav
            .nearest_area(feet)
            .zip(nav.nearest_area(goal))
            .and_then(|(a, b)| nav.find_path_with(a, b, |from, to, via| path::step_cost(nav, from, to, via, params)));
        let points = found.map(|(areas, _)| {
            nav.points_along(&areas, feet, goal, |from, to, width| {
                if width < LEAN_WIDTH {
                    0.0
                } else {
                    (path::hash01(seed, from * 7919 + to) as f32 * 2.0 - 1.0) * LEAN
                }
            })
        });
        (bot.route, bot.route_areas) = points.unwrap_or_default().into_iter().unzip();
        bot.next = 0;
        if on_ladder {
            // Already on it: skip the point at its foot or top.
            while bot.next + 1 < bot.route.len() {
                let (a, b) = (bot.route[bot.next], bot.route[bot.next + 1]);
                let mount = (b.y - a.y).abs() > LADDER_RISE && (b - a).xz().length() < LADDER_REACH;
                if mount && (a - feet).xz().length() < LADDER_REACH {
                    bot.next += 1;
                } else {
                    break;
                }
            }
        }
    }
    // Just got on a ladder boxed in by flush faces (`LadderFit`): climb
    // it on the side it got on from, just clear of the far side's face
    // (drifting back to its middle lets go of it).
    if on_ladder && bot.climbed.is_none() {
        bot.ladder_shift = 0.0;
        let near = nav
            .ladders
            .iter()
            .enumerate()
            .filter(|(_, l)| feet.y > l.bottom.y.min(l.top.y) - 2.0 && feet.y < l.bottom.y.max(l.top.y) + 1.0)
            .min_by(|a, b| (a.1.bottom - feet).xz().length().total_cmp(&(b.1.bottom - feet).xz().length()))
            .filter(|(_, l)| (l.bottom - feet).xz().length() < LADDER_REACH);
        if let Some((k, l)) = near
            && let Some(fit) = fit_of(bot, brushes, k, l)
        {
            let perp = Vec3::Y.cross(l.normal.with_y(0.0).normalize_or_zero());
            let off = (feet - l.bottom).dot(perp) - fit.middle;
            bot.ladder_shift = fit.middle + fit.clear.copysign(off);
        }
    }
    // On a ladder: how long since the feet last moved up or down.
    bot.climbed = match bot.climbed {
        _ if !on_ladder => None,
        Some((last, still)) if (feet.y - last).abs() <= LADDER_RISING => Some((last, still + dt)),
        _ => Some((feet.y, 0.0)),
    };
    if !on_ladder {
        bot.ladder_shift = 0.0;
    }
    // Stuck short of the top: try off the middle line, one side then the
    // other (a hatch narrower than the ladder, off its middle: de_nuke's
    // ladder room); after both, take it as far as it goes.
    let still = bot.climbed.map_or(0.0, |c| c.1);
    let topped_out = still > LADDER_TOPPED + 2.0 * LADDER_SIDESTEP_EACH;
    // On a ladder, a point (its top or foot, or beyond) counts as reached
    // only with the feet level with it, or a step above: turning to the
    // next one any earlier steps off under the lip and falls, or hangs
    // mid-ladder.
    let short = |p: Vec3| {
        on_ladder && (p.y - feet.y > LADDER_TOP_NEAR || feet.y - p.y > STEP_HEIGHT) && !topped_out
    };
    let ladder_top = |p: Vec3| nav.ladders.iter().any(|l| l.top.distance(p) < 0.05);
    // The point in front of a ladder's foot it gets on from (the mesh puts
    // it at the foot's height, which can be well above the floor: ladders
    // up the side of de_train's cars start 1.5 m up). Reached in 2D from
    // the floor below it, as the spec's follower does.
    let mount = |i: usize| {
        let (Some(&p), Some(&top)) = (bot.route.get(i), bot.route.get(i + 1)) else {
            return false;
        };
        ladder_top(top) && top.y - p.y > LADDER_RISE && (top - p).xz().length() < LADDER_REACH
    };
    // The point behind a ladder's top it gets on from going down (spec:
    // the follower walks to the top's front instead, so boxes beside the
    // top don't matter): reached within `LADDER_REACH`.
    let entry = |i: usize| {
        let (Some(&p), Some(&foot)) = (bot.route.get(i), bot.route.get(i + 1)) else {
            return false;
        };
        nav.ladders.iter().any(|l| l.bottom.distance(foot) < 0.05)
            && p.y - foot.y > LADDER_RISE
            && (foot - p).xz().length() < LADDER_REACH
    };
    let near = |i: usize| if entry(i) && !on_ladder { LADDER_REACH } else { REACHED };
    while bot.next < bot.route.len()
        && (bot.route[bot.next] - feet).xz().length() < near(bot.next)
        && ((bot.route[bot.next].y - feet.y).abs() < LADDER_RISE
            || (mount(bot.next) && !on_ladder && bot.route[bot.next].y > feet.y && bot.route[bot.next].y - feet.y < MOUNT_ABOVE))
        && !short(bot.route[bot.next])
    {
        bot.next += 1;
    }
    let to_mount = mount(bot.next) && !on_ladder;
    if let Some(&p) = bot.route.get(bot.next)
        && on_ladder
        && p.y - feet.y > LADDER_TOP_NEAR
        && still > LADDER_TOPPED
    {
        let turn = ((still - LADDER_TOPPED) / LADDER_SIDESTEP_EACH) as i32;
        bot.ladder_shift = if turn % 2 == 0 { LADDER_SIDESTEP } else { -LADDER_SIDESTEP };
    }
    let point = *bot.route.get(bot.next)?;
    // Past a ladder's top, still on it: step off toward the next point.
    let dismount = on_ladder
        && bot.next > 0
        && ladder_top(bot.route[bot.next - 1])
        && (bot.route[bot.next - 1] - feet).xz().length() < LADDER_REACH
        && (point.y - feet.y).abs() < LADDER_RISE;
    let to = point - feet;
    // Up or down a ladder: both ends near one of the mesh's ladders.
    let near_ladder = |p: Vec3| {
        nav.ladders.iter().any(|l| {
            (p.xz() - l.bottom.xz()).length() < LADDER_REACH
                && p.y > l.bottom.y.min(l.top.y) - 1.0
                && p.y < l.bottom.y.max(l.top.y) + 1.0
        })
    };
    // (On a ladder, short of the point: still climbing, the last step's
    // height of it too; walking there presses sideways keys, which move it
    // sideways at full climbing speed and fling it off the top.)
    let ladder = !to_mount
        && (to.y.abs() > STEP_HEIGHT || (short(point) && !dismount))
        && to.xz().length() < LADDER_REACH
        && (to.y.abs() > LADDER_RISE || (near_ladder(point) && near_ladder(feet)));
    let mut step = Step {
        target: Some(point),
        climb: ladder.then_some(point),
        dir: to.with_y(0.0).normalize_or_zero(),
        // Jump up ledges higher than a step, and when stuck.
        jump: stuck || (to.y > STEP_HEIGHT && to.xz().length() < 1.5 && !to_mount),
        crouch: nav.area_at(feet).is_some_and(|a| nav.areas[a].has(flags::CROUCH)),
        rungs: None,
        dismount: false,
        pace: 1.0,
    };
    // One of the mesh's ladders (the route goes to its top or foot): get
    // on it from the front, square to it (spec "Path following", ladder
    // handling), and stay on its middle.
    let up = to.y > 0.0;
    let mesh_ladder = nav
        .ladders
        .iter()
        .find(|l| if up { l.top } else { l.bottom }.distance(point) < 0.05);
    if dismount && !ladder {
        step.jump = false;
        // Still rising below the top: climb on (spec "Path following":
        // mid-ladder, locomotion finishes the climb; turning toward the
        // next point first presses sideways or back keys, which slide it
        // off the ladder's side or down it). Then toward a next point
        // beside or behind it (or stopped under a lip): still facing it,
        // climbing and stepping sideways toward it (de_nuke's vents: out
        // through a grille in the shaft's side); else turning to walk.
        let top = bot.route[bot.next - 1];
        if let Some(l) = nav.ladders.iter().find(|l| l.top.distance(top) < 0.05) {
            let n = l.normal.with_y(0.0).normalize_or_zero();
            let rungs = Rungs {
                foot: l.bottom,
                normal: n,
                face: -n,
                shift: bot.ladder_shift,
            };
            if still <= LADDER_TOPPED && feet.y < top.y {
                step.dir = -n;
                step.climb = Some(top + Vec3::Y * LADDER_RISE);
                step.rungs = Some(rungs);
                return Some(step);
            }
            if step.dir.dot(-n) < LADDER_OVER_TOP {
                step.rungs = Some(rungs);
            }
        }
        step.dismount = true;
        return Some(step);
    }
    if ladder && let Some(l) = mesh_ladder {
        let n = l.normal.with_y(0.0).normalize_or_zero();
        let foot = if up { l.bottom } else { l.top };
        let to_foot = (foot - feet).with_y(0.0);
        if up && !on_ladder {
            // (Pressing at its foot, sidestepping, counts as lined up.)
            let pressing = bot.ladder_pressing > 0.0 && to_foot.length() < LADDER_SIDESTEP_NEAR + LADDER_SIDESTEP;
            let lined_up = pressing || to_foot.length() < 0.1 || to_foot.normalize().dot(n) < LADDER_LINED_UP;
            if !lined_up {
                // Round to the front first.
                let mount = l.bottom + n * LADDER_MOUNT;
                step.climb = None;
                step.jump = false;
                step.target = Some(mount);
                step.dir = (mount - feet).with_y(0.0).normalize_or(-n);
                return Some(step);
            }
        }
        // Pressing at its foot without getting on: try off its middle,
        // one side then the other (coincident clip faces beside a ladder
        // win ties on one side; de_nuke's vent ladders). A ladder boxed in
        // by flush faces (`LadderFit`) only takes a box clear of one of
        // them: at once, just that far off its middle.
        if up && !on_ladder && on_ground && to_foot.length() < LADDER_SIDESTEP_NEAR {
            bot.ladder_pressing += dt;
        } else if on_ladder || !up {
            bot.ladder_pressing = 0.0;
        }
        let k = nav.ladders.iter().position(|m| std::ptr::eq(m, l)).unwrap_or(0);
        let fit = if up && !on_ladder && bot.ladder_pressing > 0.0 {
            fit_of(bot, brushes, k, l)
        } else {
            None
        };
        let (after, middle, off) = match fit {
            Some(f) => (0.0, f.middle, f.clear),
            None => (LADDER_SIDESTEP_AFTER, 0.0, LADDER_SIDESTEP),
        };
        let tangent = Vec3::Y.cross(n).normalize_or_zero();
        // The line (off the mesh ladder's middle) to get on along.
        let line = (bot.ladder_pressing > after).then(|| {
            let turn = ((bot.ladder_pressing - after) / LADDER_SIDESTEP_EACH) as i32;
            middle + if turn % 2 == 0 { off } else { -off }
        });
        // Always facing it. Up: walk at its foot. Down: back out over
        // its top, and once past it (falling) press in to catch it.
        step.dir = if let Some(line) = line
            && up
            && !on_ladder
        {
            // Sidestepping at its foot: across to the line first, then
            // straight in (pressing in while moving across carries the
            // sideways speed onto the ladder and off its side).
            let across = line - (feet - foot).dot(tangent);
            if across.abs() > LADDER_ACROSS {
                step.pace = (across.abs() / LADDER_SLOW).clamp(LADDER_PACE, 1.0);
                tangent * across.signum()
            } else {
                -n
            }
        } else if up && !on_ladder && to_foot.length() < LADDER_SIDESTEP_NEAR {
            // Close: straight in (any sideways key held as it gets on
            // moves it sideways at full climbing speed).
            -n
        } else if up {
            if to_foot.length() > 0.1 { to_foot.normalize() } else { -n }
        } else {
            let beyond = (feet - foot).dot(n);
            let side = (feet - foot).with_y(0.0) - n * beyond;
            let back = if beyond > LADDER_OVER || !on_ground { -n } else { n };
            // Square to its middle on the way.
            (back - side * 2.0).normalize_or(back)
        };
        step.jump = false;
        step.rungs = Some(Rungs {
            foot,
            normal: n,
            face: -n,
            shift: bot.ladder_shift,
        });
    }
    Some(step)
}

/// A mesh ladder whose brush is narrower than the box and flush with
/// other solid faces beside it (maps box ladders in with player clip):
/// the probe from its middle can report a flank's face (spec: the first
/// brush reached through the BSP tree wins a tie), so a box gets on, and
/// stays on, only with one side clear of a flank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LadderFit {
    /// The brush's middle line off the mesh ladder's (along `Y ×
    /// normal`), m.
    middle: f32,
    /// How far off that middle the box clears a flank, m.
    clear: f32,
}

/// `ladder_fit` of mesh ladder `k`, remembered for the last one asked.
pub(super) fn fit_of(bot: &mut Bot, brushes: &[MapBrush], k: usize, l: &NavLadder) -> Option<LadderFit> {
    match bot.ladder_fit {
        Some((j, fit)) if j == k => fit,
        _ => {
            let fit = ladder_fit(brushes, l);
            bot.ladder_fit = Some((k, fit));
            fit
        }
    }
}

pub(super) fn ladder_fit(brushes: &[MapBrush], l: &NavLadder) -> Option<LadderFit> {
    const FLUSH: f32 = 0.03;
    let n = l.normal.with_y(0.0).normalize_or_zero();
    let perp = Vec3::Y.cross(n);
    let (lo, hi) = (l.bottom.y.min(l.top.y), l.bottom.y.max(l.top.y));
    // A brush's extent along a (compass) direction.
    let span = |b: &MapBrush, d: Vec3| {
        let (a, c) = (b.min.dot(d), b.max.dot(d));
        (a.min(c), a.max(c))
    };
    let at = |p: Vec3, d: Vec3| p.dot(d);
    let rung = brushes.iter().find(|b| {
        let (p0, p1) = span(b, perp);
        let (n0, n1) = span(b, n);
        let foot = at(l.bottom, perp);
        b.ladder
            && b.max.y > lo
            && b.min.y < hi + 0.5
            && foot > p0 - FLUSH
            && foot < p1 + FLUSH
            && at(l.bottom, n) > n0 - 0.3
            && at(l.bottom, n) < n1 + 0.3
    })?;
    let (p0, p1) = span(rung, perp);
    if p1 - p0 >= 2.0 * HALF_WIDTH {
        return None;
    }
    let face = span(rung, n).1;
    let flanked = brushes.iter().any(|b| {
        let (q0, q1) = span(b, perp);
        !b.ladder
            && b.max.y > rung.min.y
            && b.min.y < rung.max.y
            && (span(b, n).1 - face).abs() < FLUSH
            && ((q1 - p0).abs() < FLUSH || (q0 - p1).abs() < FLUSH)
    });
    flanked.then(|| LadderFit {
        middle: (p0 + p1) / 2.0 - at(l.bottom, perp),
        clear: HALF_WIDTH - (p1 - p0) / 2.0 + LADDER_CLEAR,
    })
}

/// Away from the mesh's ladder the feet are at.
pub(super) fn off_ladder(nav: &NavMesh, feet: Vec3) -> Option<Step> {
    let l = nav
        .ladders
        .iter()
        .filter(|l| feet.y > l.bottom.y - 1.0 && feet.y < l.top.y + 1.0)
        .min_by(|a, b| (a.bottom - feet).xz().length().total_cmp(&(b.bottom - feet).xz().length()))
        .filter(|l| (l.bottom - feet).xz().length() < LADDER_REACH)?;
    Some(Step {
        dir: l.normal.with_y(0.0).normalize_or_zero(),
        target: None,
        jump: false,
        crouch: false,
        climb: None,
        rungs: None,
        dismount: false,
        pace: 1.0,
    })
}

pub(super) fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

