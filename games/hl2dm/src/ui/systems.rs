//! The Bevy side of HL2:DM's HUD: the local player's state into `Inputs`
//! each frame, the game's messages (damage taken, kills, pickups) into the
//! HUD's events, the weapon selection's commands, and the HUD's `Frame`
//! into UI nodes (`sync`). Run only while HL2:DM's HUD is the gameplay's
//! (`gameplay::hud_is`); the look is the map's (`map::hud::ActiveHud`, read
//! by `game::hud`), so a map without it leaves the shell's HUD up.

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use chimera_client::client::{
    binds, fonts::UiFonts, hud::{GameDraws, HudRoot}, hud_text::{GlyphParams, HudGlyphMaterial, HudText}, kill_feed::KillFeed, video::PlayerFov,
    weapon_menu::{WeaponMenu, WeaponMenuTimes, menu_input, pick_of},
};
use chimera_core::{
    console::{Console, resource_cvar},
    core::{Dead, DamageKind, DamageTaken, Health, LocalPlayer},
};
use chimera_net::net::NetCharacter;
use chimera_sim::weapon::{Armor, Inventory, Magazine, Weapon, data::WeaponCatalog};
use chimera_world::map::{PlaySound, hud::ActiveHud};

use super::{
    damage::{DamageMsg, Kinds, damage_angle},
    frame::{Draw, Frame, Screen},
    hud::{Hl2Hud, Inputs, Owned, WeaponNow},
    look::Look,
    scoreboard::{Board, BoardPlayer},
    selection::Action,
};
use crate::game::{
    GAME,
    hud::{ANIMATIONS, animations, icon_key},
    rules::deaths::notice_name,
    standing::{Hl2MatchView, Hl2Team},
    suit::Suit,
    weapons::inventory::{Hl2Ammo, Hl2Item},
};

/// What this HUD draws instead of the shell's.
pub const DRAWS: GameDraws = GameDraws { crosshair: true, status: true, hit_marker: false, killfeed: true, callout: false };

/// HL2:DM's HUD cvars (`hud_fastswitch`...).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HudCvars {
    pub fastswitch: i32,
    pub quickinfo: u8,
    pub crosshair: u8,
    pub show_empty_slots: u8,
}

impl Default for HudCvars {
    fn default() -> Self {
        HudCvars { fastswitch: 0, quickinfo: 1, crosshair: 1, show_empty_slots: 1 }
    }
}

/// The HUD, its look, this frame's inputs and the frame it drew.
#[derive(Resource, Default)]
pub struct Hl2dmHud {
    pub hud: Option<Hl2Hud>,
    pub look: Option<Look>,
    pub inputs: Inputs,
    pub frame: Frame,
    pub board: Option<Board>,
    seen: Seen,
}

#[derive(Default)]
struct Seen {
    weapons: HashSet<Entity>,
    ammo: HashMap<String, i32>,
    health: i32,
    armour: i32,
    alive: bool,
    kill: u64,
    started: bool,
}

pub fn register_cvars(app: &mut App) {
    app.init_resource::<HudCvars>();
    for (name, help, field) in [
        ("hl2dm:hud_fastswitch", "0: weapon selection buckets; 1: a slot key switches at once.", (|r: &mut HudCvars| &mut r.fastswitch) as fn(&mut HudCvars) -> &mut i32),
    ] {
        resource_cvar::<HudCvars, i32>(app, name, help, field);
    }
    for (name, help, field) in [
        ("hl2dm:hud_quickinfo", "1: the quick info brackets show health and clip.", (|r: &mut HudCvars| &mut r.quickinfo) as fn(&mut HudCvars) -> &mut u8),
        ("hl2dm:crosshair", "1: the crosshair and quick info show.", |r| &mut r.crosshair),
        ("hl2dm:hud_showemptyweaponslots", "1: empty boxes show in the open bucket.", |r| &mut r.show_empty_slots),
    ] {
        resource_cvar::<HudCvars, u8>(app, name, help, field);
    }
}

/// The HUD's look has this game's script in it.
fn is_ours(hud: &ActiveHud) -> bool {
    hud.0.menus.as_ref().is_some_and(|m| m.strings.contains_key(ANIMATIONS))
}

pub fn build(
    hud: Res<ActiveHud>,
    windows: Query<&Window>,
    mut state: ResMut<Hl2dmHud>,
    mut draws: Option<ResMut<GameDraws>>,
    mut commands: Commands,
) {
    let Some(w) = windows.iter().next() else { return };
    if !is_ours(&hud) {
        state.hud = None;
        state.look = None;
        return;
    }
    let look = Look::new(hud.0.clone(), Screen::new(w.width(), w.height()));
    state.hud = Some(Hl2Hud::new(animations(&hud.0), &look));
    state.look = Some(look);
    state.seen = Seen::default();
    if let Some(d) = draws.as_mut() {
        **d = DRAWS;
    }
    let _ = &mut commands;
}

pub fn teardown(mut state: ResMut<Hl2dmHud>) {
    state.hud = None;
    state.look = None;
}

/// The weapon in hand as the HUD reads it.
fn weapon_now(w: &Weapon, mag: Option<&Magazine>, item: Option<&Hl2Item>) -> WeaponNow {
    let class = crate::game::scripts::class_of(w.id).to_string();
    let uses_primary = item.map_or(mag.is_some(), |i| i.ammo.is_some());
    WeaponNow {
        class,
        uses_primary,
        clip: mag.filter(|m| m.size > 0).map(|m| m.clip as i32),
        max_clip: mag.map_or(0, |m| m.size as i32),
        reserve: mag.map_or(0, |m| m.reserve as i32),
        // M5's secondary ammo (the grenades, the AR2 ball, the S.L.A.M.)
        // comes with its weapons' data.
        secondary: None,
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn collect(
    mut state: ResMut<Hl2dmHud>,
    time: Res<Time>,
    windows: Query<&Window>,
    player: Option<Single<(Entity, &Health, Option<&Armor>, Option<&Suit>, Option<&Inventory>, Has<Dead>, Option<&Hl2Team>, Option<&Transform>, Option<&Hl2Ammo>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>, Option<&Hl2Item>)>,
    catalog: Option<Res<WeaponCatalog>>,
    mut taken: MessageReader<DamageTaken>,
    feed: Option<Res<KillFeed>>,
    views: Query<&Hl2MatchView>,
    teams: Query<&Hl2Team>,
    cvars: Res<HudCvars>,
    fov: Option<Res<PlayerFov>>,
    mut play: MessageWriter<PlaySound>,
) {
    let st = &mut *state;
    let (Some(hud), Some(look)) = (st.hud.as_mut(), st.look.as_mut()) else {
        taken.clear();
        return;
    };
    if let Some(w) = windows.iter().next() {
        let screen = Screen::new(w.width(), w.height());
        if look.screen != screen {
            look.screen = screen;
            hud.anim.screen_units = screen.units_wide();
        }
    }
    let now = time.elapsed_secs_f64();
    let Some((_me, health, armor, suit, inventory, dead, team, at, ammo)) = player.map(|p| *p) else {
        taken.clear();
        return;
    };
    let view = views.iter().next();
    let (alive, points) = (!dead && health.current > 0.0, health.current * 100.0);
    // The spawn's HUD reset: the first frame and every return to life.
    if !st.seen.started || (alive && !st.seen.alive) {
        st.seen.started = true;
        hud.reset(look);
        st.seen.weapons.clear();
        st.seen.ammo.clear();
        st.seen.health = points.ceil() as i32;
        st.seen.armour = armor.map_or(0, |a| (a.amount * 100.0).ceil() as i32);
    }
    st.seen.alive = alive;

    let mut inp = Inputs {
        alive,
        health: points,
        armour: armor.map_or(0.0, |a| a.amount * 100.0),
        aux: suit.map_or(100.0, |s| s.aux),
        sprinting: suit.is_some_and(|s| s.sprinting),
        breathing: suit.is_some_and(|s| s.devices & crate::game::suit::BREATHER != 0),
        zoomed: suit.is_some_and(|s| s.zoom.is_some_and(|z| z.to < z.from)),
        fov: fov.map_or(75.0, |f| f.0),
        teamplay: view.is_some_and(|v| v.teamplay),
        team: team.map_or(0, |t| t.0),
        crosshair: cvars.crosshair != 0,
        quickinfo: cvars.quickinfo != 0,
        hide_empty_slots: cvars.show_empty_slots == 0,
        ..Inputs::default()
    };
    if inp.zoomed {
        inp.fov = inp.fov.min(crate::game::suit::ZOOM_FOV);
    }
    // The weapons: in hand, and every one owned for the selection.
    if let Some(inv) = inventory {
        inp.active = inv.active;
        for &e in &inv.weapons {
            let Ok((w, mag, item)) = weapons.get(e) else { continue };
            let data = catalog.as_ref().and_then(|c| c.get(w.id));
            let has_ammo = mag.is_none_or(|m| m.clip > 0 || m.reserve > 0);
            inp.owned.push(Owned {
                entity: e,
                class: crate::game::scripts::class_of(w.id).to_string(),
                bucket: w.slot,
                position: item.map_or(0, |i| i.position),
                selectable: has_ammo,
                print_name: data.and_then(|d| d.hud.name.clone()).unwrap_or_default(),
            });
        }
        if let Some(active) = inv.active.and_then(|a| weapons.get(a).ok()) {
            let mut now_weapon = weapon_now(active.0, active.1, active.2);
            // The carried count of a type no clip holds (the frag, the
            // RPG): the player's loose ammo.
            if now_weapon.clip.is_none()
                && let (Some(a), Some(item)) = (ammo, active.2)
                && let Some(t) = item.ammo
            {
                now_weapon.reserve = a.0.iter().find(|(n, _)| n == t).map_or(now_weapon.reserve, |(_, c)| *c as i32);
            }
            inp.weapon = Some(now_weapon);
        }
    }

    // Pickups: a weapon new to the inventory, ammo that rose, a battery or
    // a health kit (a whole step up in armour or health).
    if alive {
        for o in &inp.owned {
            if st.seen.weapons.insert(o.entity) {
                let icon = look.glyph(&icon_key(&o.class, "weapon")).map(|(f, c)| (super::numeric::static_font(&f), c));
                let no_ammo = !o.selectable;
                let exhaustible = o.class == "weapon_frag";
                hud.history.weapon(o.entity, icon, no_ammo, exhaustible, now);
            }
        }
        let mut carried: HashMap<String, i32> = HashMap::new();
        for e in inventory.map_or(&[][..], |i| &i.weapons[..]) {
            if let Ok((w, Some(m), Some(i))) = weapons.get(*e).map(|(w, m, i)| (w, m, i))
                && let Some(t) = i.ammo
                && w.slot < 6
            {
                let c = carried.entry(t.to_string()).or_default();
                *c = (*c).max(m.reserve as i32);
            }
        }
        for (t, c) in ammo.map_or(&[][..], |a| &a.0[..]).iter().map(|(t, c)| (t, *c as i32)) {
            let e = carried.entry(t.clone()).or_default();
            *e = (*e).max(c);
        }
        for (t, c) in &carried {
            let old = st.seen.ammo.get(t).copied();
            if let Some(old) = old
                && *c > old
            {
                let icon = inp
                    .owned
                    .iter()
                    .filter_map(|o| weapons.get(o.entity).ok().map(|(_, _, i)| (o, i)))
                    .find(|(_, i)| i.is_some_and(|i| i.ammo == Some(t.as_str())))
                    .and_then(|(o, _)| look.glyph(&icon_key(&o.class, "ammo")))
                    .map(|(f, c)| (super::numeric::static_font(&f), c));
                hud.history.ammo(t, *c - old, icon, now);
            }
            st.seen.ammo.insert(t.clone(), *c);
        }
        let (health_i, armour_i) = (points.ceil() as i32, inp.armour.ceil() as i32);
        let item_icon = |name: &str| look.glyph(name).map(|(f, c)| (super::numeric::static_font(&f), c));
        if armour_i - st.seen.armour == 15 {
            hud.history.item(item_icon("item_battery"), now);
        }
        if health_i - st.seen.health == 25 {
            hud.history.item(item_icon("item_healthkit"), now);
        }
        st.seen.health = health_i;
        st.seen.armour = armour_i;
    }

    // Damage taken.
    for t in taken.read() {
        let d = &t.damage;
        let kinds = Kinds { drown: d.kind == DamageKind::Drown, burn: d.kind == DamageKind::Burn, ..Kinds::default() };
        let angle = at.and_then(|tf| {
            // The hit's direction is where it travelled; the inflictor is
            // behind it.
            let to = (-d.dir).normalize_or_zero();
            let forward = Vec3::new(tf.forward().x, 0.0, tf.forward().z).normalize_or_zero();
            let left = Vec3::new(tf.left().x, 0.0, tf.left().z).normalize_or_zero();
            damage_angle(to, forward, left)
        });
        let msg = DamageMsg { health: d.amount * 100.0, armour: 0.0, kinds, angle, world: !t.directed };
        hud.damage_message(&msg, points);
    }

    // Deaths: the shell's kill feed has them (a client's from the
    // server's `Killed`), named and with the killing weapon.
    if let Some(feed) = feed {
        let from = st.seen.kill;
        for e in feed.entries.iter().filter(|e| e.id >= from) {
            let team_of = |ent: Entity| teams.get(ent).map_or(0, |t| t.0);
            let victim = (e.victim.name.clone(), team_of(e.victim.entity));
            let killer = e.killer.as_ref().map(|k| (k.name.clone(), team_of(k.entity)));
            let weapon = e.weapon.map_or_else(|| "world".to_string(), notice_name);
            hud.death_notice(killer.filter(|_| !e.suicide), victim, &weapon);
            st.seen.kill = e.id + 1;
        }
    }

    hud.think(&inp, now);
    for s in std::mem::take(&mut hud.quick.sounds) {
        play.write(PlaySound::ui(s));
    }
    st.frame = hud.frame(look, &inp);
    st.inputs = inp;
}

/// The scoreboard's rows: every player with its team, score, ping and name
/// (a client's from the server's `NetScore` and `NetCharacter`).
#[allow(clippy::type_complexity)]
pub fn collect_board(
    mut state: ResMut<Hl2dmHud>,
    players: Query<(
        Entity,
        Option<&Name>,
        Option<&NetCharacter>,
        Option<&chimera_core::core::Score>,
        Option<&chimera_net::net::NetScore>,
        Option<&Hl2Team>,
        Has<LocalPlayer>,
        Has<chimera_core::core::Bot>,
    ), With<chimera_core::core::Intent>>,
    views: Query<&Hl2MatchView>,
    settings: Option<Res<chimera_net::net::NetSettings>>,
    held: Res<chimera_client::client::console::HeldActions>,
    (console, ui, keys, mouse): (
        Option<Res<Console>>,
        Option<Res<chimera_client::client::console::ConsoleUi>>,
        Res<ButtonInput<KeyCode>>,
        Option<Res<ButtonInput<MouseButton>>>,
    ),
) {
    if state.hud.is_none() {
        return;
    }
    let view = views.iter().next();
    let mut rows: Vec<BoardPlayer> = players
        .iter()
        .map(|(e, name, character, score, net, team, local, bot)| {
            let s = score.copied().unwrap_or_default();
            let (kills, deaths) = net.map_or((s.kills, s.deaths), |n| (n.kills, n.deaths));
            BoardPlayer {
                name: chimera_client::client::shown_name(e, local, character, name, settings.as_deref()),
                team: team.map_or(0, |t| t.0),
                frags: kills,
                deaths,
                ping: net.map_or(0, |n| n.ping),
                bot: bot || net.is_some_and(|n| n.flags & chimera_net::net::score_flags::BOT != 0),
                local,
                index: e.index().index(),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.index);
    let board = Board {
        teamplay: view.is_some_and(|v| v.teamplay),
        server_name: String::new(),
        players: rows,
        team_scores: view.map_or([0; 2], |v| v.scores),
    };
    // Held with `+showscores`, and up at the intermission.
    let typing = ui.is_some_and(|c| c.open);
    let tab = match (&console, &mouse) {
        (Some(c), Some(m)) => binds::pressed(&c.binds, &keys, m, "+showscores"),
        _ => false,
    };
    let show = (tab && !typing) || held.showscores || view.is_some_and(|v| v.over);
    if show && let Some(look) = state.look.clone() {
        state.frame.0.extend(super::scoreboard::frame(&look, &board).0);
    }
    state.board = Some(board);
}

/// Selection commands (`slotN`, `invnext`, `invprev`, attack) before the
/// shell's weapon menu reads them: they are ours while this HUD is up.
#[allow(clippy::too_many_arguments)]
pub fn selection_commands(
    mut state: ResMut<Hl2dmHud>,
    mut menu: ResMut<WeaponMenu>,
    times: Res<WeaponMenuTimes>,
    cvars: Res<HudCvars>,
    console: Option<Res<Console>>,
    (keys, mouse): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    captured: Option<Res<binds::CapturedKeys>>,
    cursor: Query<&bevy::window::CursorOptions>,
    mut play: MessageWriter<PlaySound>,
) {
    menu.takes_slots = false;
    let st = &mut *state;
    let Some(hud) = st.hud.as_mut() else { return };
    let pressed = std::mem::take(&mut menu.pressed);
    let steps = std::mem::take(&mut menu.steps);
    if !st.inputs.alive || st.inputs.hide_selection {
        return;
    }
    let (owned, active) = (st.inputs.owned.clone(), st.inputs.active);
    let mut actions = Vec::new();
    for slot in pressed {
        actions.extend(hud.select_slot(slot, &owned, active, cvars.fastswitch));
    }
    for step in steps {
        actions.extend(hud.step(step as i32, &owned, active, cvars.fastswitch));
    }
    // Attack while the selection is open confirms, and fires nothing.
    let empty = std::collections::BTreeMap::new();
    let binds_map = console.as_ref().map_or(&empty, |c| &c.binds);
    let playing = cursor.iter().next().is_some_and(chimera_client::client::input::cursor_grabbed);
    let attack = playing && binds::just_pressed_free(binds_map, &keys, &mouse, captured.as_deref(), "+attack");
    let attack_held = playing && binds::pressed_free(binds_map, &keys, &mouse, captured.as_deref(), "+attack");
    if !attack_held {
        menu.swallow_fire = false;
    }
    if hud.selection.open && cvars.fastswitch == 0 {
        if attack || (menu.swallow_fire && attack_held) {
            if attack {
                actions.extend(hud.confirm(&owned));
            }
            menu.swallow_fire = true;
        }
        if keys.just_pressed(KeyCode::Escape) {
            actions.extend(hud.cancel());
        }
    }
    let held: Vec<(Entity, u8)> = owned.iter().map(|o| (o.entity, o.bucket)).collect();
    for a in actions {
        match a {
            Action::Sound(s) => {
                play.write(PlaySound::ui(s));
            }
            Action::Switch(w) => {
                if let Some(pick) = pick_of(&held, w) {
                    menu.select = Some((pick, times.select_hold));
                }
            }
        }
    }
}

/// `menu_input` runs after this and finds nothing to do.
pub fn run_before_menu() -> impl bevy::ecs::schedule::IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    selection_commands.before(menu_input)
}

/// One pooled node of a frame's draws.
#[derive(Component)]
pub struct Pooled(pub u8);

const KIND_BOX: u8 = 0;
const KIND_TEXT: u8 = 1;
const KIND_SPRITE: u8 = 2;

/// The frame's draws as UI nodes: pooled entities per kind, each kept at
/// its draw's index in the stack (`GlobalZIndex`), extras hidden.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync(
    state: Res<Hl2dmHud>,
    roots: Query<Entity, With<Root>>,
    mut pool: Local<[Vec<Entity>; 3]>,
    mut nodes: Query<(&mut Node, &mut Visibility, &mut GlobalZIndex)>,
    mut boxes: Query<(&mut BackgroundColor, Option<&mut BackgroundGradient>)>,
    mut texts: Query<&mut HudText>,
    mut sprites: Query<&mut MaterialNode<HudGlyphMaterial>>,
    (hud_fonts, active, mut materials): (Option<Res<UiFonts>>, Option<Res<ActiveHud>>, ResMut<Assets<HudGlyphMaterial>>),
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {
    let Some(root) = roots.iter().next() else { return };
    let (Some(look), Some(active)) = (&state.look, active) else {
        for e in pool.iter().flatten() {
            if let Ok((_, mut v, _)) = nodes.get_mut(*e) {
                v.set_if_neq(Visibility::Hidden);
            }
        }
        return;
    };
    let _ = hud_fonts;
    let mut used = [0usize; 3];
    for (z, d) in state.frame.0.iter().enumerate() {
        let (kind, rect): (u8, Rect) = match d {
            Draw::Box { rect, .. } | Draw::Fade { rect, .. } => (KIND_BOX, *rect),
            Draw::Ring { centre, radius, .. } => (KIND_BOX, Rect::from_center_half_size(*centre, Vec2::splat(*radius))),
            Draw::Text { clip, .. } => (KIND_TEXT, clip.unwrap_or(Rect::new(0.0, 0.0, look.screen.w, look.screen.h))),
            Draw::Sprite { rect, .. } => (KIND_SPRITE, *rect),
        };
        let slot = used[kind as usize];
        used[kind as usize] += 1;
        let entity = match pool[kind as usize].get(slot) {
            Some(e) => *e,
            None => {
                let mut e = commands.spawn((
                    Pooled(kind),
                    Node { position_type: PositionType::Absolute, ..default() },
                    Visibility::Hidden,
                    GlobalZIndex(0),
                    ChildOf(root),
                ));
                match kind {
                    KIND_BOX => {
                        e.insert(BackgroundColor(Color::NONE));
                    }
                    KIND_TEXT => {
                        e.insert(Node { position_type: PositionType::Absolute, overflow: Overflow::clip(), ..default() });
                    }
                    _ => {}
                }
                let id = e.id();
                pool[kind as usize].push(id);
                if kind == KIND_TEXT {
                    // The text itself, inside its clipping wrapper.
                    commands.spawn((
                        Pooled(KIND_TEXT),
                        Node { position_type: PositionType::Absolute, ..default() },
                        HudText {
                            font: chimera_client::client::hud_text::GlyphFont { data: std::sync::Arc::new(Vec::new()), additive: false },
                            tall: 1.0,
                            text: String::new(),
                            color: Color::NONE,
                            at: Vec2::ZERO,
                            align: chimera_client::client::hud_text::Align::Left,
                        },
                        ChildOf(id),
                    ));
                }
                id
            }
        };
        let Ok((mut node, mut vis, mut zi)) = nodes.get_mut(entity) else { continue };
        zi.set_if_neq(GlobalZIndex(100 + z as i32));
        vis.set_if_neq(Visibility::Inherited);
        let place = |node: &mut Node, r: Rect| {
            node.left = px(r.min.x);
            node.top = px(r.min.y);
            node.width = px(r.width());
            node.height = px(r.height());
        };
        let rgba = |c: [u8; 4]| Color::srgba_u8(c[0], c[1], c[2], c[3]);
        match d {
            Draw::Box { rect, color, round } => {
                let radius = if *round { (look.screen.scaled(8.0) / 2.0).max(8.0) } else { 0.0 };
                node.border_radius = BorderRadius::all(px(radius));
                place(&mut node, *rect);
                if let Ok((mut bg, grad)) = boxes.get_mut(entity) {
                    bg.set_if_neq(BackgroundColor(rgba(*color)));
                    if grad.is_some() {
                        commands.entity(entity).remove::<BackgroundGradient>();
                    }
                }
            }
            Draw::Fade { rect, color, from, to } => {
                node.border_radius = BorderRadius::ZERO;
                place(&mut node, *rect);
                let c = |k: f32| rgba([color[0], color[1], color[2], (color[3] as f32 * k) as u8]);
                if let Ok((mut bg, _)) = boxes.get_mut(entity) {
                    bg.set_if_neq(BackgroundColor(Color::NONE));
                }
                commands.entity(entity).insert(BackgroundGradient::from(LinearGradient {
                    angle: std::f32::consts::FRAC_PI_2,
                    stops: vec![ColorStop::new(c(*from), Val::Percent(0.0)), ColorStop::new(c(*to), Val::Percent(100.0))],
                    ..default()
                }));
            }
            Draw::Ring { radius, color, .. } => {
                node.border_radius = BorderRadius::MAX;
                node.border = UiRect::all(px(1.0));
                place(&mut node, rect);
                let _ = radius;
                commands.entity(entity).insert(BorderColor::all(rgba(*color)));
            }
            Draw::Text { font, tall, text, color, at, align, clip } => {
                place(&mut node, rect);
                let origin = clip.map_or(Vec2::ZERO, |c| c.min);
                if let Some(f) = look.font(font) {
                    commands.queue({
                        let (text, color, at, align, tall, glyph) = (text.clone(), rgba(*color), *at - origin, *align, *tall, f.font);
                        move |w: &mut World| {
                            let child = w.get::<Children>(entity).and_then(|c| c.first().copied());
                            if let Some(child) = child
                                && let Some(mut t) = w.get_mut::<HudText>(child)
                            {
                                let want = HudText { font: glyph, tall, text, color, at, align };
                                if *t != want {
                                    *t = want;
                                }
                            }
                        }
                    });
                }
                let _ = &mut texts;
            }
            Draw::Sprite { sprite, rect, color, additive, flip } => {
                place(&mut node, *rect);
                let Some(sp) = look.hud.sprites.get(sprite) else { continue };
                let Some(image) = active.1.get(&sp.texture) else { continue };
                // Additive sheets draw as they are: the pipeline adds them (black is
                // nothing), also when the texture is block compressed.
                let image = image.clone();
                let image = &image;
                let size = images.get(image).map_or(Vec2::ONE, |i| i.size().as_vec2());
                let [x, y, w, h] = sp.rect;
                let (mut u0, mut v0, mut du, mut dv) = (x / size.x, y / size.y, w / size.x, h / size.y);
                if flip[0] {
                    u0 += du;
                    du = -du;
                }
                if flip[1] {
                    v0 += dv;
                    dv = -dv;
                }
                let want = HudGlyphMaterial {
                    params: GlyphParams { color: rgba(*color).to_linear(), uv: Vec4::new(u0, v0, du, dv) },
                    coverage: image.clone(),
                    additive: *additive,
                };
                match sprites.get_mut(entity) {
                    Ok(m) => {
                        if materials.get(&m.0).is_none_or(|cur| cur.params != want.params || cur.coverage != want.coverage || cur.additive != want.additive) {
                            let _ = materials.insert(&m.0, want);
                        }
                    }
                    Err(_) => {
                        commands.entity(entity).insert(MaterialNode(materials.add(want)));
                    }
                }
            }
        }
    }
    // The pool's extras.
    for (kind, list) in pool.iter().enumerate() {
        for e in list.iter().skip(used[kind]) {
            if let Ok((_, mut v, _)) = nodes.get_mut(*e) {
                v.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

/// The HUD's root node: everything above hangs from it.
#[derive(Component)]
pub struct Root;

pub fn spawn_root(mut commands: Commands) {
    commands.spawn((
        Root,
        HudRoot(GAME),
        Node { position_type: PositionType::Absolute, left: px(0.0), top: px(0.0), width: percent(100.0), height: percent(100.0), ..default() },
        Visibility::Inherited,
        Pickable::IGNORE,
    ));
}
