# Plan: Ultimate Gun Game's new UI ("Signal")

Status: in progress (M0, 2026-11-13). Design reference:
the Look Book IV prototype (published artifact) and its round V (menu
slot C backdrop, ambient shader background, tag styles, match-end
background: still open when this was written). The prototype's sizes,
timings and easings are the spec; its HTML, CSS and JS are never copied
or translated: everything is built again in Bevy from the numbers below.

Settled (user, look book rounds I-IV): palette Cobalt; UGG Display
Black/Bold/Regular and UGG Mono v0.4 (the wordmark in Black with `ss02`,
the plain A); one 12° slant; the logo pass between every screen (1.2 s,
the swap at 600 ms while covered); the lobby (no Surprises row); the HUD
(race chip and its leader variant, Tab opening the chip to the board,
kill feed marking your kills, the default health/ammo HUD only as the
fallback, the rank-up toast, the air-strike banner and edge pulse
folding into an effects chip); the Tab board with gun and K/D icons; the
match end (winner, standings with a hover climb chart, the map vote
along the bottom, one countdown, Leave, the next match on its own);
session wins; the main menu (lockup, four items, rotating backdrops plus
an ambient shader background), and an in-engine Showcase backdrop.

## 1. What exists (read before changing anything)

| Need | Have | Where |
|---|---|---|
| One engine-owned layer over every game's HUD, sized for 720 lines and scaled by the window's height, one face, tooltips, text fitting | `OverlayRoot`, `OverlayLayer`, `overlay::scale`, `overlay::font` (our text face, UGG Display), `overlay::fit`, `Tooltip` | `chimera/client/src/client/overlay.rs`; DESIGN.md "One HUD layer over several games' HUDs" |
| Games kept off that layer | `games_leave_the_overlay_alone` (a game crate naming `overlay` or `UiScale` fails) | `checks/tests/it/architecture.rs` |
| One layout at 16:9, 4:3, 21:9 under every game's HUD | `the_overlay_is_laid_out_alike_under_every_games_hud` | `apps/party_kit/tests/it/overlay_guard.rs` |
| Minigame panel (top centre), callouts, Tab board, match end, kill feed, respawn countdown, swap flash | `minigame_panel`, `callouts`, `minigame_scoreboard`, `match_end`, `kill_feed`, `respawn_countdown`, `swap_effect` | `chimera/client/src/client/` |
| Pre-match lobby and its F3 ready/start | `chimera_net::prematch` (`NetLobby`, `LobbyRow`), `client::prematch_lobby` (`LobbyView`, `LOBBY_KEY`) | `chimera/net/src/prematch.rs`, `chimera/client/src/client/prematch_lobby.rs` |
| Main menu, Esc menu, the menu's own camera, procedural backdrop, live background map | `main_menu` (`PartyMenu`, `MenuUiCamera`, `pick_background`, `fly`), `esc_menu`, `menu_backdrop` (+ `.wesl`) | `apps/party_kit/src/` |
| The fallback HUD | `hud_minimal` (`MINIMAL_HUD`, `HudRoot`), chosen by `PartsRegistry::fallback_hud` | `chimera/client/src/client/hud_minimal.rs` |
| Kill icons per game | `WeaponIcons` (CS:S and HL2:DM glyphs of their icon fonts, Combat Arms' images), `spawn_weapon_icon`, `kill_feed::icon_slot` | `chimera/client/src/client/weapon_icons.rs`; `games/cs_source/src/ui/game_hud.rs`, `games/combat_arms/src/ui/hud.rs` (`register_icons`), `games/hl2dm/src/ui/kill_icons.rs` |
| Short gun names | `GunNameRules::short`, `names::fit`, `gun_short_name` | `chimera/sim/src/weapon/names.rs`, `minigame_scoreboard.rs` |
| Gun class | `GunClass` (pistol ... grenade), from weapon data | `chimera/core/src/game_parts.rs` |
| Matches won | `wins::MatchWins` (by name, per server run), `MatchesWon` replicated; `ResultRow::wins` | `chimera/sim/src/minigame/wins.rs`, `results.rs` |
| Surprises | `surprise airstrike`, `surprise gravity` (the match's effects, `minigame::effects::MatchEffects`, since M3) | `apps/party_kit/src/surprise.rs` |
| Glyph atlases ahead | `client::text_warm` (`TextWarmAppExt::warm_texts`) | docs/performance.md "HUD texts' glyph atlases ahead" |
| Writes only on change | `ui_write::set_text`, `edit` | `chimera/client/src/client/ui_write.rs` |

Pinned Bevy 0.20 (read from the crate sources, not memory):

- `bevy_ui_render::UiMaterial` (`ui_material.rs`: `AsBindGroup + Asset`,
  `vertex_shader`, `fragment_shader`, `specialize`), `MaterialNode<M>`,
  `UiMaterialPlugin<M>` (`ui_material_pipeline.rs`). The fragment gets
  `UiVertexOutput` (`ui_vertex_output.wesl`: `uv`, `size` in physical
  pixels, border widths and radii) and the view and `globals` (time)
  uniforms (`ui_material.wesl`). The party menu's backdrop already is
  one (`menu_backdrop.rs`).
- `bevy_ui::UiTransform` (`ui_transform.rs`): translation (`Val2`),
  scale, rotation; **no skew**, so the slant is drawn by materials.
  `Overflow` clips to the node's axis-aligned rect only.
  `BackgroundGradient`/`LinearGradient` exist (`gradients.rs`);
  `BoxShadow`/`ShadowStyle`, `Outline`, `ImageNode`, `UiTargetCamera`
  exist. `TextShadow` has an offset and a colour, **no blur**.
  Layout rounds to physical pixels (`layout/ui_surface.rs`, taffy
  rounding on).
- `bevy_text` (Parley): `TextFont { font: FontSource (Handle, Family,
  Families, ...), font_size: FontSize, weight: FontWeight (1-1000),
  width, style, font_smoothing, font_features: FontFeatures,
  font_variations }`; `FontFeatures::builder().enable(tag)` (so `ss02`);
  `LetterSpacing::Px/Rem`, `LineHeight`, `FontHinting { Disabled,
  Enabled }`, `FontSmoothing { None, AntiAliased }` (no subpixel AA).
  `Font::from_bytes` assets are registered in Parley's collection under
  their embedded family name (`font.rs`,
  `load_font_assets_into_font_collection`), so `FontSource::Family("UGG
  Display")` with `FontWeight(900)` finds the Black face.
- `bevy_sprite::Text2d`, `Mesh2d` for the 2D backdrops.
- Renderer (for the Showcase's Studio look): `bevy_light::
  EnvironmentMapLight`, `GeneratedEnvironmentMapLight` (a cubemap
  filtered on the GPU at run time, `probe.rs`), `ShadowFilteringMethod`
  and `soft_shadows_enabled` on spot and point lights; `bevy_pbr::
  ScreenSpaceAmbientOcclusion` (recommends TAA), `ContactShadows` (depth
  prepass), `ScreenSpaceReflections` (needs the deferred prepass);
  `bevy_post_process::Bloom`; `bevy_anti_alias::TemporalAntiAliasing`
  (`Msaa::Off`).

## 2. Architecture

### 2.1 Where the kit lives

A new engine crate **`chimera_ui`** (`chimera/ui/`), the UI kit: no
game, no simulation, no network. It depends on Bevy (ui, ui_render,
text, sprite) and `chimera_core` (console cvars) only, so
`chimera_client` and any app use it, and its unit tests run in seconds
(`cargo test -p chimera_ui`). Modules:

- `theme`: `Theme` (resource) read from TOML: palette tokens, the type
  roles, motion durations and easings, layout numbers per screen (2.3).
- `fonts`: `KitFonts` (handles of the four faces, registered with
  `Font::from_bytes`; `install`), `Role` (`Wordmark`, `DisplayBlack`,
  `DisplayBold`, `DisplayRegular`, `Mono`, `Text`) to `TextFont` with
  weight, `ss02` where the role says, letter spacing in em, line height
  and case.
- `units`: `Cq` (the look book's unit, 1 % of a 16:9 frame's width =
  12.8 px at 720 lines) to overlay pixels, `overlay::scale` applied;
  `snap_px` (a logical position to whole physical pixels).
- `motion`: easings, `Tween`, `Timeline` (2.5).
- `materials`: `SlantMaterial`, `TickBarMaterial`, `PassMaterial`,
  `LinesMaterial` (2.5).
- `icons`: the fallback gun icons and the marks (section 4).
- `widgets`: small builders on Bevy UI: `tag(kind)` (YOU, BOT, HOST,
  RUNG; round V restyles them), `kbd`, `plus_mark`, `rolling_number`,
  `tick_bar`, `slab`, `chip`.
- `scroll` (M7): the scroll container, `Ui::scroll(key)`: a view, its
  content and a Signal bar (hairline track, a thumb the visible share
  long with 12° ends, hover `fg`, drag `acc`; drag it, click the track
  to page), shown only on overflow; the wheel eased to its target; the
  right button held pans 1:1 and flings on release (speed of the last
  0.1 s, exponential friction, an elastic stop at the edges, stopped by
  the next press); `ScrollIntoView` keeps a row in view; state by key in
  `ScrollViews` (screens rebuild); `[layout.scroll]` tunes it,
  `ReducedMotion` (`ui_reduce_motion`) cuts the ease and the fling.

Rows in `workspace_crates` (`checks/tests/it/architecture.rs`): an
engine crate; `chimera_client` may use it; games may not use it outside
their `ui` feature, and never its overlay screens (the existing check
stays as is). Run `cargo hakari generate` after adding it.

Screens stay where they are, restyled from the theme:

- **Shell overlays** (engine, `chimera_client`): lobby, race chip and
  board, kill feed, rank-up toast, effects chips and banner, match end,
  the logo pass, the respawn countdown. They name no game and serve any
  app with a minigame; the Signal look comes from the `Theme` the app
  inserts. Without one they keep today's colours (`Theme::shell_default`,
  in our faces since 2026-11-15), so Lucker Party and plain games keep
  their layout.
- **App screens** (`party_kit`, feature `present`): the main menu, the
  Esc menu, the backdrops, the Showcase. UGG-only data lives in
  `apps/ultimate_gun_game/` (theme, menu data); the faces are
  Chimera's (`chimera/ui/fonts/`).

DESIGN.md "HUD layers" changes in M0: the overlay's face is the app's
theme's (one set of faces for the whole layer, never a game's); since
2026-11-15 ours in every theme.

### 2.2 Game HUDs and the fallback HUD

- A game's own HUD keeps its look and scale under its `HudRoot`s
  (unchanged; parity first). The Signal overlay draws over it.
- The fallback HUD (`hud_minimal`) is restyled to the look book's HUD
  (health with slanted segments bottom left, ammo with bars bottom
  right, the crosshair) and stays a HUD (`HudRoot(MINIMAL_HUD)`), shown
  only when the gun's game lends none (`PartsRegistry::fallback_hud`).
  It reads the `Theme` but is no overlay.
- Overlap: the overlay's top-centre chip, top-right feed and top-left
  effects chips must keep clear of what a game HUD draws there. The
  overlay guard grows a test, `overlay_keeps_clear_of_game_huds`: under
  every game in `CLIENTS`, the overlay's visible rects must not
  intersect any visible node under a `HudRoot`. Where a game draws in
  one of those places, the game's piece is hidden under a minigame the
  way radars and kill feeds already are (`MinigameHud::playing`, a gate
  in the game's `ui`), never moved by the overlay.
- The current minigame panel (top centre) is replaced by the race chip
  in M2; the callouts' rank-up and lead lines move to the toast and the
  chip (callouts stay for the other events).

### 2.3 Theme as data

`apps/ultimate_gun_game/data/theme.toml`, read with `include_str!` like
the other data files, plus a dev override (`ui_theme_file <path>`,
`ui_theme_reload`, feature `dev`) for tuning without a rebuild:

```
[palette]            # Cobalt; every other colour derives from these
ink = "#06080C"  ink_2 = "#0A0D13"  ink_3 = "#111620"  ink_4 = "#1A212D"
fg = "#E8EDF5"  acc = "#3B7BFF"  on_acc = "#04060B"  acc_text = "#6A98FF"
threat = "#FF3B4E"
[alpha]              # fg_2 .66, fg_3 .42, fg_4 .15, fg_5 .07, acc_2 .16, plate .80 ...
[type]               # roles: face, weight, size in cq, tracking in em, case, features
[motion]             # named easings and per-piece durations (2.5)
[screen.toast]       # layout numbers in cq: top = 6.8, height = 3.1, ...
```

Colours are display-space values as the shell's other overlays already
treat them (`widgets::display_alpha`). A test, `theme_matches_the_look_book`,
pins a few derived values. Data-driven first: what a screen shows
(columns, which chips exist, menu entries) is data where it already is
(main_menu.toml) or becomes so; choreography stays in code, with its
numbers in the theme.

### 2.3b One token source, a style layer, BSN

- **Tokens**: the theme (shell.toml with Signal's theme.toml over it) is
  the one source of colours, sizes in cq, padding, gaps, type roles,
  durations and network thresholds. `repo_checks --bin ui_tokens` writes
  them as the look book's CSS custom properties
  (`apps/ultimate_gun_game/data/tokens.css`, and `--out` the look book's
  folder, whose gen.py appends it last so it wins);
  `ui_tokens_are_fresh` fails when the committed file is stale.
- **Style layer** (`chimera_ui::style`): `UiStyle` (`style!(row, gap: 0.6,
  pad: [0.7, 0.8], h: 2.15, bg: Tone::Plate)`: flex row/column, gap,
  padding/margin in CSS shorthand, sizes, absolute offsets, alignment,
  grow, a background tone, an outline), `UiText` (role, size in cq, tone)
  and `UiSlant` (a 12° fill), components resolved through `UiUnits` by
  the kit (again on a resize, a theme or font change). A node's px are the
  look book's `v cqw` rounded to whole physical pixels
  (`styled_px_follow_the_css_formula`, 1080p and 1440p).
- **BSN** (finding, 2026-11-14): pinned Bevy 0.20 ships `bevy_scene`'s
  `bsn!`/`bsn_list!` (on in our build through Bevy's `ui` feature; Bevy's
  own feathers widgets are written in it). It works for our UI trees,
  shown by `bsn_builds_styled_ui_trees`: styled rows from data (a
  `Vec<impl Scene>` as `Children [ {rows} ]`), text in the kit's roles,
  `UiSlant` nodes getting their `SlantMaterial`, and any marker beside
  them (a parity id per node). Notes: a runtime value goes in as a bare
  identifier bound before the macro (`let s = style!(..); bsn! { s }`),
  not as `{expr}` in component position (that form is for field values:
  `Text({name})`); components need `Default + Clone` (or
  `FromTemplate`); `spawn_scene` resolves at once, `queue_spawn_scene`
  waits for assets. Screens may use BSN or plain bundles; both read the
  same `UiStyle`/`UiText`.

### 2.4 Fonts as run-time assets

Our own fonts (OFL-1.1, authored for this project) may be committed:

- `chimera/ui/fonts/` (moved from `apps/ultimate_gun_game/fonts/` on
  2026-11-15: they are every app's faces now): `ugg-display-black.ttf`,
  `ugg-display-bold.ttf`, `ugg-display-regular.ttf`,
  `ugg-mono-regular.ttf` (TTF, 19-40 KB each; no WOFF2 needed),
  `OFL.txt` (the licence with our copyright line and the Reserved Font
  Name, if any) and `FONTLOG.txt` (version v0.5, what changed). v0.5
  adds a real lower case, the missing ASCII and Latin-1
  (`tools/fonts/lower.py`); the roles that must stay in capitals
  upper-case their own text (`TypeSpec::upper`; `style::case_texts`
  keeps a styled text in capitals however it is set later, `KeepCase`
  opts out). `UGG_FONTS_LOWER=0` builds the old caps-only mapping.
- Embedded in `chimera_ui` (`fonts::OWN`, `include_bytes!`) and loaded
  once per app by `KitFonts::install` (`ChimeraUiPlugin` and the
  client's `FontsPlugin`): `Font::from_bytes` into `Assets<Font>`, no
  file I/O, the same on Linux and Windows, offscreen and headless.
  Display Regular also replaces Bevy's default font, so text that names
  no font is ours.
- **Our faces everywhere** (user decision, 2026-11-15): every text the
  shell and the apps draw is in UGG Display or UGG Mono: the kit's roles
  in both themes (the shell's default too), the overlay layer
  (`overlay::font`), the console and its overlay lines, chat lines and
  hints, net_graph, the perf and debug readouts (and the egui debug UI),
  the kill feed, the party apps' GameUI-look menus (`widgets::Look::
  own_faces`). A game's own fonts stay where its own HUD or menu code
  draws them (CS:S's, HL2:DM's and Combat Arms' HUDs and icon fonts,
  GameUI's menus, VGUI panels, `game_text`), with the system faces their
  schemes name.
- **The sentence face** (`Role::Text`, `[type.text]`): Display Regular
  in its own case, tracking .01 em, lines 1.3 em (the descenders reach
  -200 units, 0.05 em under the faces' 850/-150 line box);
  `controls::SENTENCE` (.86 cq) for bodies and help, `NOTE` (.72 cq) for
  footnotes and status lines, `TYPED` (.74 cq) for typed text. Used for
  the Settings help pane, dialog bodies (report, crash, leave, keep
  display), notices and status lines, the Esc menu's paused and vote
  lines, Join's "is locked" line, the lobby's waiting line and player
  names, and typed text (`Ui::text_field`, `Ui::text_plain`; the old
  system-face fallback `style::PlainText` is gone). Console and cvar
  names show in lower-case Mono.
- THIRD_PARTY.md lists code we copy from others; our own fonts are not
  third party, so no entry there. The README's "Assets, code and legal"
  section gets one line: the UGG fonts are ours, OFL-1.1, in
  `chimera/ui/fonts/`. `no_game_assets_tracked` allows `.ttf`
  under 1 MB already; a new check, `own_fonts_carry_their_licence`,
  requires an `OFL.txt` beside any tracked font file.
- The font build scripts are ours too (open question 3).
- `text_warm`: every role and size the screens use is declared through
  `TextWarmAppExt::warm_texts` (sizes follow the window's height), so no
  atlas is made on a screen's first frame.

### 2.5 The motion kit

**Easings** (`chimera_ui::motion::Ease`, named in the theme; values as
the prototype computes them):

| Name | Curve |
|---|---|
| `expo_out` | `p == 1 ? 1 : 1 - 2^(-10p)` |
| `expo_in` | `p == 0 ? 0 : 2^(10p - 10)` |
| `cubic_in_out` | `p < .5 ? 4p^3 : 1 - (-2p + 2)^3 / 2` |
| `quint_in_out` | `p < .5 ? 16p^5 : 1 - (-2p + 2)^5 / 2` |
| `back(s)` | `q = p - 1; 1 + (s + 1)q^3 + s q^2` (the toast's roll: s = .9) |
| `hold` | `q = 2p - 1; .5 + .5 sign(q) abs(q)^3` (the logo pass) |
| `bezier(x1,y1,x2,y2)` | the CSS curves: out (.16,1,.3,1), in (.7,0,.84,0), in-out (.65,0,.35,1), pop (.34,1.56,.64,1) |
| `steps(n)` | the menu lockup's letter flicker, the board's sweep |
| `pulse` | `x < .08 ? x/.08 : exp(-(x - .08)/.32)` (the air strike's edge) |

**Timeline**: a `Timeline` component holds tracks (`Track { prop,
from, to, at, dur, ease }`) over a few properties: opacity, translate
x/y (cq), clip left/right (0-1), width/height (cq), scale y, letter
spacing (em), a material's parameter. One system steps all timelines
from `UiClock` (real time in play; stepped by hand in tests, so frames
are reproducible) and writes through `ui_write::edit` only on change.
Interruptions follow the prototype: a new rank-up during the toast
restarts the inner tracks without a new entry; Tab released mid-open
folds from where it is.

**Materials** (WGSL, `.wesl` beside the Rust as the repo does now):

- `SlantMaterial`: a parallelogram fill (tan 12° = 0.2126 from the
  theme), its edges anti-aliased over one physical pixel (from
  `UiVertexOutput::size`), optional accent bar at the left, horizontal
  wipe parameters (clip from the left and the right, along the slant),
  linear alpha ramp for the banner. Covers the menu's highlight bar,
  the toast's slanted wipe, the chip's meter ticks, the health segments,
  vote pips, the match-end winner slab, the backdrop swap's edge.
- `TickBarMaterial`: `n` ticks, `lit`, `hot` (accent from `hot` on),
  `pitch` and `tick` in physical pixels; the fragment picks a tick by
  integer pixel column, so ticks never blur.
- `PassMaterial`: the logo pass (four slanted bands and the mark, one
  full-screen node), or four `SlantMaterial` nodes plus a clipped mark
  image if that proves simpler.
- `LinesMaterial`: the match-end background (one system of 12° lines on
  a 24-column grid; far hairlines at 4 % in a 6-column 2+1 rhythm moving
  6 columns per 24 s loop; near half-column slabs at 2 % every 12
  columns at twice the speed, right to left), on `globals.time`.
- `AmbientMaterial`: the ambient background every between-match screen
  shares (look book V's shader, ported to WGSL: bands, facets, one
  bloom, composed in display space and written as linear light; per
  screen a calm column edge, or none, and the bloom's place:
  `AmbientMaterial::MENU`, `MATCH_END`).

**Pixel snapping** (`chimera_ui::units`): the prototype's tick-bar
layout, in physical pixels: `pitch = max(2, floor(box_w / n))`, `tick =
max(1, round(pitch * .36))`, `w = n * pitch - (pitch - tick)`, `h =
max(3, round(pitch * 1.5))` capped by the box; the node's left and top
rounded to whole physical pixels after layout (a `PostUpdate` system
after `UiSystems::Layout` sets a `UiTransform` translation of the
fraction). Same for hairlines (1 physical px, never scaled) and the
`+` marks.

**Look-book deviations** (Bevy has no blur): plates are more opaque
than the look book's (Signal's `alpha.plate` .92 for its .80 plus a
backdrop blur; tokens.css follows), so text keeps its contrast over a
bright map; text glow (`text-shadow` blur) is
dropped, or drawn as a soft `BoxShadow` behind big display text only if
the A/B screenshots miss it.

### 2.6 Sizes

The prototype measures in `cq` (1 % of a 16:9 frame's width). The
UI is sized by the largest 16:9 frame that fits the window
(`units::layout_height`: the height, or width x 9/16 when narrower), so
1 cq = 12.8 px at 720 lines times `overlay_scale` of that: identical at
16:9, wider margins at 21:9; at 4:3 or in a tall window the frame fits
the width (hotfix 7.1: the menu's frame was cropped at the sides). Smallest text: tags .42-.5 cq (5.4-
6.4 px at 720 lines, 8-10 px at 1080) gets a floor in physical pixels
from the theme (`min_text_px = 9`, as the prototype's `max(9px, ...)`)
(risk 1).

## 3. Screens and their specs (from the prototype)

- **Logo pass** (`client::logo_pass`, shell): bands at widths 1.1 cq
  accent (offset -2.6), 140 cq ink with the mark (offset 0), 10 cq
  accent (141), 5 cq fg (152), slanted 12°, at `x = -175 + 287 *
  hold(p)` cq over 1.2 s; the mark (13 x 9.33 cq, at 81.5 cq in the ink
  band) wipes in `expo_out((p - .33)/.16)` and out `expo_in((p - .70)/.12)`.
  The screen swap runs at p = .5 (600 ms) while covered. Its own root
  with the highest `GlobalZIndex`, moved to whichever UI camera draws
  (the menu's or the shell's, as the lobby is). API: `LogoPass::run(
  on_cover: impl FnOnce(&mut World))`; front-end moves (menu to loading,
  loading to lobby, lobby to play, match end to the next match, Leave)
  go through it. `ui_pass` (console) runs one for screenshots.
- **Lobby**: header (lockup at 1.05 cq, title 1.2 cq), rule at 7.6 cq;
  left column 50 cq: map and mode big (3 cq), the player table (rows
  3.2 cq, columns 2.6 / 1fr / 9 / 9.4 cq), tags HOST/BOT/YOU, the ready
  block that pops (`scale y 1 -> 1.9 -> 1`, .32 s, pop); right column:
  map tile, key/values, the countdown big with a 48-tick bar (hot from
  16/19). F3 as today (`LOBBY_KEY`). No Surprises row.
- **Race chip** (top centre, 21 x 3.05 cq at 2.2 cq from the top): your
  rung rolling, the gun icon, the meter (skewed ticks .66 x .32 cq; the
  leader's tick marked), the leader's name and rung. Leader variant:
  crown, the gap to the next. **Tab** opens it to the board (64 x 25.9
  cq) in .24 s (`expo_out`; chip content out in .08 s, board in from
  .06 s over .12 s, rows rising .45 cq each 25 ms apart, .16 s each);
  release folds back in .2 s (`cubic_in_out`). `+showscores` drives it.
- **Board rows** (in the chip; and the full-screen board, open question
  2): position, name and tags, rung big, gun icon and short name, K and
  D under their icons, the ladder ticks set apart by a hairline; the
  leader's accent bar; your row outlined.
- **Kill feed** (top right, 26 cq wide, rows 1.85 cq at a 2.15 cq pitch,
  newest at the bottom): a row enters from 2.4 cq right (`expo_out`
  .22 s), lives 5 s, leaves .25 s, older rows close the gap
  (`cubic_in_out` .25 s). Your kill: accent outline and bar, killer in
  accent text; your death: outlined, victim bright.
- **Rank-up toast** (top centre, 6.8 cq down, 3.1 cq tall): enters with
  a slanted wipe .2 s; the number rolls up from .08 s over .3 s
  (`back(.9)`); the gun wipes in from .12 s over .28 s; next gun from
  .2 s over .25 s; holds to .92 s; leaves with `expo_in` over .24 s
  (1.15 s in all). Line 1: the short name (`GunNameRules::short`, fitted:
  `overlay::fit`); line 2: class and game (`GunClass` name, the game's
  `GameInfo` name). A chain within the hold restarts inside the toast.
  The chip's meter tick pops as the number rolls.
- **Air strike**: edge glow pulses at .2/1.2/2.2/3.2 s and at impact
  (1.3x), a base of .2 during the warning; the banner (3.3 cq tall, 15.5
  cq down) opens from the centre at .25 s (.3 s), its letter spacing
  .3 to .08 em; folds at 2.0 s (`expo_in` .28 s) into the top-left
  effects chip (13.4 cq; threat bar; "Impact in N", then "Impact - stay
  down"); chips stack at 3.05 cq slots (`cubic_in_out` .24 s); the
  impact flash `.2 exp(-t/.12)`. Moon gravity is the second chip (ring
  countdown).
- **Match end**: header; winner (3.6 cq, slab sweeping across in .9 s
  from .55 s, the name shown at .9 s); standings (rows 2.62 cq, icons,
  K/D, session wins); hover or click a row: its climb chart (stepped
  rung line drawn in .7 s, kill and death ticks fading in along x over
  .52 s, the longest stall marked); the vote along the bottom (four map
  cards with pips, the leader's count in accent, yours outlined; one
  20 s countdown with a tick bar, hot at 17/20; then locked and the
  others dimmed); Leave; the logo pass into the next match.
- **Main menu**: lockup at (4.6, 5.6) cq, 3.3 cq; four items from 23.4
  cq down, 24 cq wide, 3.5 cq tall (index, label 1.36 cq, triangle);
  the highlight bar (slanted right edge .74 cq, accent bar .32 cq)
  wiping in .45 s at .62 s, items stepping in .52 s, 55 ms apart from
  .24 s; the ink fade (97 % to 88 % at 27 % to 0 at 46 %); keys line at
  the foot; backdrop tabs bottom right with a dwell bar; backdrops
  swapping every 16 s with a slanted wipe (.52 s, `cubic_in_out`, a
  3 px accent edge).

## 4. Gun icons and marks

- **Fallback set** (ours): the icon set's 13 shapes (pistol, revolver,
  SMG, rifle, LMG, shotgun, sniper, launcher, energy, crossbow, grenade,
  knife, blunt) as data, `chimera/ui/data/icons.toml` (polygons with
  whole-unit vertices on a 16-unit grid, holes by even-odd), committed:
  our own design. Drawn by **rasterizing on a task** into a coverage
  atlas per physical height (as glyph atlases: crisp at every size, one
  `ImageNode` tinted by the theme), cached by (icon, px). The Assembly
  backdrop needs them as meshes: our own ear clipping with hole bridging
  (`icons::tessellate`; the rules say rings never overlap) into `Mesh2d`.
- **Which icon**: `GunClass` to a shape (heavy -> lmg, carbine -> rifle,
  heavy_pistol -> revolver, marksman -> sniper, flamethrower ->
  launcher), melee by the weapon's slot (knife, else blunt), and a
  weapon's data may name one (`hud.icon = "crossbow"`, HL2:DM's crossbow
  and AR2): `chimera_ui::icons::IconFor` resolved by `chimera_client`.
- **Each game's own icons** when installed: the existing `WeaponIcons`
  registry (CS:S's and HL2:DM's icon-font glyphs, Combat Arms' images
  from its install's art, read at run time, never committed). Which
  places use them is open question 1.
- **Marks** as icon data too: the UGG mark (the logo pass, the lockup),
  crown (leader), trophy (session wins), kill and death (the K/D
  headers), warning (air strike), and a robot mark for bots if look book
  V replaces the BOT tag with it.

## 5. Milestones

Each one ships alone, behind nothing (the old look goes screen by
screen), with scenario tests on the harness (`Sim` with
`SimBasePlugins`, or the party's) and offscreen screenshots
(`--offscreen 1280x720`, `1920x1080`, `2560x1440`, `3840x2160`;
docs/OBSERVABILITY.md 2, "Offscreen") and **measured against the
prototype's frames before done**: `tools/ui-parity/run.sh <screen>`
(docs/OBSERVABILITY.md 6, "UI parity against the look book") with no
element past tolerance, or each one left listed with its reason in the
milestone's notes. A screen's nodes carry `UiParityId`s named as the look
book's `data-ui` ids. Every screen gets a console command that stages it
with made-up data (`ui_scene lobby|chip|board|toast|airstrike|matchend|
menu`, feature `dev`) for screenshots, and with the look book's data from
a fixture (`ui_scene <scene> tools/ui-parity/fixtures/<page>.json`) for
the parity check. Perf (section 7) is counted per
milestone.

### M0: tokens, fonts, the motion kit

Status (2026-11-14): done but for the items under "Left". In:
`chimera/ui` (`theme`, `fonts`, `units`, `motion`, `materials`: Slant,
TickBar, Pass, Lines, Ambient (the look book V shader since M4), `icons`: data/icons.toml with the
13 guns and the marks (UGG mark, crown, trophy, kill, death, warn),
rasterized per pixel height (`IconImages`) and tessellated, `IconFor`,
`choose` with `icons.prefer_game`; `widgets`: `tag`, `kbd`);
`ChimeraUiPlugin` and `ui_scene` (`client::ui_scene`, `proof`) in the
shell; Signal's theme and the v0.4 faces in `apps/ultimate_gun_game/`
(`signal`); the font sources in `tools/fonts/`;
`own_fonts_carry_their_licence`. Left: `overlay::font` from the theme's
roles (the screens' milestones switch their text to `fonts::text`), the
widgets `plus_mark`, `rolling_number`, `slab`, `chip`, the post-layout
pixel snap system, and `text_warm` declarations of the roles' sizes.

- `chimera_ui` crate (2.1) with `theme`, `fonts`, `units`, `motion`,
  `materials`, `icons`, `widgets`; the fonts and theme in
  `apps/ultimate_gun_game/`; `overlay::font` takes the theme's roles.
- Tests (`chimera/ui/tests/it/`): `easings_match_the_look_book`
  (sampled values), `timeline_is_frame_rate_independent` (60 vs 144
  steps end alike), `interrupted_tracks_continue_from_where_they_are`,
  `tick_bar_layout_snaps_to_physical_pixels` (1080p, 1440p, 4K: the
  prototype's numbers), `fonts_resolve_by_family_and_weight`,
  `wordmark_enables_ss02`, `theme_matches_the_look_book`,
  `icons_rasterize_with_holes`, `icons_tessellate_with_holes`.
  `repo_checks`: `own_fonts_carry_their_licence`, the crate's rows.
- Screenshot: `ui_scene proof` (type ramp, tick bars, slants, icons) at
  the four sizes.

### M1: the logo pass and the lobby

Status (2026-11-14): the lobby is in (`client::lobby_panel`, drawn on
the kit's style layer and Look Book V marks when the theme has
`[layout.lobby]`; the shell's panel otherwise; `LobbyView` gained the
map's game, the slots, the ladder's games and each row's ping;
`SlantMaterial::outline` for the unready block; tests
`chimera/client/tests/it/lobby_panel.rs`). The logo pass is in
(`client::logo_pass`: `Passes::ask(key)` and the `PassCovered`
event at the swap, which the screens observe; the swap in `PreUpdate`, reduced motion `ui_reduce_motion 1` as a 150 ms fade,
`ui_pass [p]` holding a pass for screenshots; its root moves to the
party menu's camera with the lobby and the console; the kit's
`pass.wesl` measures offsets at mid-height with `mark_x` the mark's
centre, so the mark holds centred at the swap) on the lobby coming up
and going (lobby to match) and the match end and the next match (tests
`chimera/client/tests/it/logo_pass.rs`, party_kit
`main_menu::logo_pass_covers_both_ui_cameras`). The party menu's
screens go through it too (`party_kit::main_menu::ShownScreen`: menu,
loading panel and game change at the swap, Leave and disconnect
included; test `main_menu::the_menu_changes_screens_at_the_pass_s_swap`).
Left: screenshots at the four sizes and of a real lobby-to-match pass
(so far 1080p, `ui_pass 0.2` and `0.5` in the game and over the menu).

- `client::logo_pass`; the front end's moves through it (2.5, 3).
- `prematch_lobby` rebuilt on the kit (HOST from `LobbyRow::id ==
  HOST_ID`, BOT from `bot()`, YOU from `LocalClientId`); no network
  change.
- Tests (`chimera/client/tests/it/`): `logo_pass_swaps_at_half_while_covered`,
  `logo_pass_covers_both_ui_cameras`, `lobby_rows_carry_host_bot_you`,
  `lobby_ready_block_flips`; the overlay guard covers the lobby root.
  The existing prematch tests keep passing.

### M2: race chip, Tab board, kill feed, fallback HUD

Status (2026-11-14): in, behind the theme's `flags.hud.signal` (only
Signal sets it; Lucker Party and the plain apps keep today's screens):
`client::race_chip` (the chip, its leader variant, Tab opening it into
the one board in 0.12 s and folding in 0.09 s from wherever it is),
`kill_feed::signal_feed`, `hud_minimal::signal_hud`, their pieces in
`client::signal` (tags, the fused YOU+HOST chip, the bot slot, the YOU
row, fades, gun icons: each game's own where `WeaponIcons` has one while
`flags.icons.prefer_game`, else the look book's set by `GunClass`; the
set is rasterized in `signal::icons` until the kit's `icons` lands).
Tests: `race_chip_opens_and_folds_on_showscores`,
`race_chip_needs_the_signal_theme`, `leader_chip_shows_the_gap`,
`board_rows_best_first_with_icons`, `kill_feed_marks_local_kills_and_deaths`,
`the_plain_feed_without_the_signal_theme` (chimera_client),
`fallback_hud_only_without_a_game_hud` (ultimate_gun_game). Left:
`overlay_keeps_clear_of_game_huds`, the match clock on the board's head
(the rolling rung and the tick pop came with M3's toast).

Parity (2026-11-14, `tools/ui-parity/run.sh`, 1920x1080): fixed the
board's ladder column (11.4 cq), its titles' padding and rule, the K/D
marks (.6 cq), the ping cell (.5 cq type, the meter .46 cq, the value
2.2 ch), the ticks (.58 cq at .74), the rows' rule as their border, the
head on one baseline, each text's tracking from the look book; the
chip's groups (.5 cq, the gap .4 cq), rungs at least 1.5 cq, the gun
slot 2.8 cq, the leader's plate 23 cq; the feed's rows rebuilt on a
resize (they kept their first layout, 720p's, when staged at start) and
its border inside the padding; the kit's text lines one font size tall.
Left past tolerance, with reasons: text widths from whole-pixel font
sizes (board 51 of 125, chip 6 of 11, leader chip 5 of 12; most are
names, guns, K/D at 1-4 px); the board's head has no match clock (its
`FIRST TO 14 • 06:41 • 8 PLAYERS`); the worst ping shows LAG where the
look book shows the loss mark (since M4: the loss mark when the line
loses packets, `score_flags::LOSS`, LAG without); the feed's rows were
2.3 cq apart where the look book's script moves them 2.15 cq apart
(since M4: 2.15, `layout.feed.pitch`). Widths that differ only by a
whole-pixel font size are reported as expected (`~`) since M4.

- `client::race_chip` replaces `minigame_panel` for a ladder (other
  minigames keep the panel); `minigame_scoreboard` becomes the chip's
  board; `kill_feed` restyled with fallback icons; `hud_minimal`
  restyled.
- Tests: `race_chip_opens_and_folds_on_showscores`,
  `leader_chip_shows_the_gap`, `board_rows_best_first_with_icons`,
  `kill_feed_marks_local_kills_and_deaths`,
  `fallback_hud_only_without_a_game_hud` (party_kit),
  `overlay_keeps_clear_of_game_huds` (overlay guard), `text_warm`
  extended: the first Tab and first kill make no atlas.
- No network change (rungs, wins, K/D are replicated already).

Dominations (decided 2026-11-14, for later): they apply to the gun
game. specs/cs_source/dominations.md has no free-for-all notes; its rule
flags only kills between enemies, and in a free-for-all everyone is an
enemy, so the 4th unanswered kill dominates and a kill back is a revenge
as in CS:S. The counting is the engine's (`chimera_sim::dominations`:
`count_kills`, enemies when teams differ or a player has none); the
gun game's rules would add it as CS:S's do (`DominationSet`), and the
server already sends a kill's flags (`net::Killed::marks`, written again
as `KillMark` on clients) and each player's relationships to its own
client. A natural fit for the Signal kill feed (a dominating or revenge
row) and the board (the YOU row's nemesis); not wired yet.

### M3: rank-up toast and the air strike

Status (2026-11-14): done. The toast: `client::rank_toast` (the pose a
function of the recent rung ups and `UiClock`; the plate's wipe a
`SlantMaterial` with a slanted cut; the chip's YOU rung a rolling number
and its tick popping in step; a chain within the hold or exit rolls
again; the rung-up callout's text left to it under Signal, its cue
kept); `ui_scene rankup [t]`. The effects: `chimera_sim::minigame::
effects::MatchEffects` (`Effect { id, kind, title, detail, countdown,
over, threat, color, starts, impact_at, ends, stopped }`), filled by
`party_kit::surprise` (no HUD text, no fade: decision 5), replicated as
`NetMinigame::effects` (**net49**); `client::match_effects` draws them:
under Signal the banner (every effect gets one; a threat's red with the
warning mark and the count), the edge pulse (a threat's; the kit's
`EdgeMaterial`, no fill and no impact flash), and the top-left chip stack
(threats first, slots sliding; a timed effect's `RingMaterial`
countdown); otherwise the plain text banner, flash and lines Lucker Party
had. `ui_scene airstrike [t]`. Tests: `rank_toast_rolls_and_holds_1_15_s`,
`rank_toast_chains_within_the_hold`, `the_pose_follows_the_look_book`,
`rank_toast_needs_the_signal_theme_and_takes_the_rung_up_callout`,
`air_strike_banner_folds_into_its_chip`, `effects_chips_stack_and_leave`,
`the_plain_look_says_it_as_text`, `the_edge_pulses_each_second`
(chimera_client); `match_effects_replicate` (chimera_net NetSim);
`effects_start_end_and_prune` (chimera_sim); the surprise tests
(party_kit) on `MatchEffects`.

- `client::rank_toast` from the local rung-up (`MinigameHud`'s event,
  the callout it replaces); `client::effects` (banner, edge, chips).
- **Network**: the surprises become structured, a generic
  `chimera_sim::minigame::effects::MatchEffects` (resource: a list of
  `Effect { kind, title, starts, impact_at, ends }`, replicated by
  `chimera_net::minigame_net`), filled by `party_kit::surprise` instead
  of HUD text (the text stays for older clients only if open question 5
  says so). **net48 to net49** (`NET_VERSION`,
  `chimera/net/src/net/mod.rs`).
- Tests: `rank_toast_rolls_and_holds_1_15_s`, `rank_toast_chains_within_the_hold`,
  `air_strike_banner_folds_into_its_chip`, `effects_chips_stack_and_leave`;
  `match_effects_replicate` (`chimera_net` NetSim).

### M4: match end and the vote

Status (2026-11-14): done on `ugg-ui-m4` (steps 1-3 below). The shared ambient
(`AmbientMaterial`, look book V's shader in WGSL; its own commit for M5).
The match end in the Signal look (`client::match_end_signal`, behind
`flags.hud.signal`; Lucker Party and the plain apps keep `match_end`'s
screen): the header lockup with MATCH OVER and map, game, minigame,
players; the winner with the accent slab sweeping across it and the
line (all the rungs in the time, the final kill: the winner's gun and
its last victim); the standings (HOST, fused YOU+HOST, the bot slot,
your row tinted and outlined, hover a faint fill (no tick since
2026-11-14), the selected row a stronger fill and a pointer outside the row, K/D
under their marks right-aligned, ping with the loss mark, session wins
with +1 on the winner's: the hook for M6); the climb chart (rung steps
against match time for the hovered row, else the selected one, the
winner's first; the others faint; kills and deaths as ticks; the longest
stall boxed (dashed since the polish pass, its label on an ink plate
clear of the line: `StallLabel`); drawn along its length in `climb_draw` with the ticks
fading in along x on a new choice); the games' HUDs hidden under it
(`client::hud::HudCover`, shared with M7's Esc menu since the polish
pass), the cursor free. Data: `ResultRow::climb` (`minigame::climb::Climb`: each
rung change, kill and death in seconds from the match going live, the
last victim), `MatchResults::length`, recorded on the authority
(`ClimbLog`); the scoreboard's `score_flags::LOSS`. **net50** (M3 took
net49). `ui_scene matchend [fixture]` (tools/ui-parity/fixtures/
matchend.json: the look book's data); parity screen `matchend`
(reference/end.html, the look book's frame with `data-ui` ids). Parity
(1920x1080, step 1): 89 matched, 13 past tolerance, 7 widths expected
from whole-pixel font sizes; left: the tags' boxes 1-2 px wide (font
rounding inside a box), the winner's line (the look book splits it into
bold runs the mockup measures apart), the fused chip 3 px, the loss
mark's cell, session wins 2 px; the rows' right columns sit .64 cq in
from the padding as the look book's grid does (`ROW_PAD_RIGHT`). Left:
the background and Leave (step 3).

Step 2 in: the vote band (`VoteCard`, `VoteCountdown`, `VoteTicks`): one
countdown, the vote's close and the next match's start at once (the
server sets `NextMatch::at` to the close plus the reveal when a vote
opens, `minigame_net::open_vote`), a 40-tick bar hot from 17 of 20 s;
four cards (the Ultimate Gun Game's `map_vote = { candidates = 4, time
= 20, reveal = 1.6 }`: the rotation's pick and three others from the
pool by the players present) with their game, eight pips (yours in the
accent), the count (the leader's in the accent), your vote outlined and
tagged; click or a number key votes; a tie goes to the pool's pick (the
earliest on the ballot, `MapVote::tally`); at 0 the cards lock (the
others at 35 %, the winner outlined and named under the countdown) and
the logo pass begins 1 s later, its swap on the change. Tests:
`match_end_has_one_countdown` (client), `vote_cards_lock_then_next_match`
(party_kit, NetSim). Parity (step 2): 119 matched, 22 past tolerance
(the step 1 list, the tags' boxes, pips 3 px, the way-out line that
waits for Leave); thumbnails are tinted gradients (no map shots yet).

Step 3 in: the ground (`EndGround`: the shared ambient with the match
end's bloom and the broad slanted planes over it, `LinesMaterial` now
look book V's planes rule: 16/32/64/128-unit planes every three widths,
one spacing per 96 s, `PLANES_ALPHA` .6 % a layer in display space) and
Leave (`LeaveButton`: `disconnect`, through the logo pass). Test:
`the_ground_and_leave`. Parity (step 3, 1920x1080): 120 matched, 22 past
tolerance, none missing: the tags' boxes (1-2 px), the winner's line and
the way-out line (the mockup measures a wrapped or split text's run),
the pips (3 px over eight), the fused chip, the loss mark's cell,
session wins (2 px). Left: map thumbnails (shots of the maps), the lock's
1 s reveal tuned in a playtest, Leave's confirmation for a listen host.

- `match_end` rebuilt: standings, climb chart, vote cards, one
  countdown, Leave (`disconnect` through the logo pass), the next match
  as today (`NextMatch`).
- **Data**: the climb chart needs each player's rung-up, kill and death
  times: `chimera_sim::minigame::results::ResultRow` gains `climb:
  Climb { ups: Vec<f32>, kills: Vec<f32>, deaths: Vec<f32> }`
  (seconds into the match, recorded by the ladder on the authority),
  and the vote four candidates instead of three (`map_vote` data: the
  rotation's pick and three others). The vote's close and the map
  change share one deadline (`MapVote` closes at `NextMatch::at`).
  **net49 to net50** (`MatchResults` changes shape: the climb; the vote keeps its shape).
- Tests: `climb_records_rungs_kills_deaths` (chimera_sim),
  `match_end_has_one_countdown`, `vote_cards_lock_then_next_match`,
  `climb_chart_follows_hover` (client), `results_with_climb_replicate`
  (net); the existing `map_vote` and `results_hud` tests keep passing.

### M5: main menu and backdrops

Status (2026-11-14): the first page is in (`party_kit::signal_menu`,
under a theme with `[layout.menu]`: the lockup wiping in with
ULTIMATE's letters flickering on, the entries stepping in on the slant,
the highlight wiping in and gliding between entries with the inverted
copy clipped to it, ↑↓ and Enter, the pointer; the account chip with an
inert Friends; the keys; the rule up the right edge; the backdrop tabs
with the dwell bar and caption; the ambient under everything; no live
map and no `MenuBackdrop` under it; `ui_scene menu [backdrop] [t]` and
`ui_scene menu enter <t>`; tests `apps/party_kit/tests/it/signal_menu.rs`).
The pages are Signal's (2026-11-14): Options opens M7's Settings, Play
and Join are `party_kit::signal_pages` (`[layout.pages]`; Settings' frame
over the ambient, the kit's controls with a new `text_field`; Play's
Server and Game tabs with a summary pane, Join's address, password,
recent addresses and the LAN server list; ↑↓/Tab, ←→, Q/E, F5, Enter,
Esc; `ui_scene play [game]`, `ui_scene join`; tests
`apps/party_kit/tests/it/signal_pages.rs`). Lucker Party and the plain
apps keep the VGUI frames.
The backdrops are in (`party_kit::menu_backdrops`): Ladder, Assembly and
the Ident (slot C until M5b's Showcase) painted on the CPU into a
vertex-coloured `Mesh2d` with `Text2d` labels per backdrop, drawn by a 2D
camera into an image the size of the backdrops' area (the right 64 %),
shown through `BackdropsMaterial`, which fades them toward the menu column,
runs the 12° swap with its accent edge and lays the Ident's sheen; the
Ladder uses the games' own kill icons when at hand. Cost (seeded,
headless, instructions a frame, the menu's systems): Ladder 0.3 M,
Assembly 1.2 M, Ident 2.5 M at its lock and 1.6 M under its sheen
(`menu_backdrops::the_menus_frame_stays_in_budget`, limit 3 M); a frozen
backdrop is painted once. Tests `apps/party_kit/tests/it/menu_backdrops.rs`.
The ground is the shared ambient (M4's `AmbientMaterial`, the `MENU`
preset: calm left of 30 % of the width, the bloom behind the backdrops;
still with reduced motion and in staged scenes; test
`the_ground_is_the_ambient_calm_behind_the_menu`). Parity (2026-11-14, `tools/ui-parity/run.sh menu`, 1920x1080, ids in
`reference/menu.html` from the look book generator's `--parity`): 25
matched, 14 within tolerance; left past it, with reasons: the tabs (slot
C reads IDENT here, SHOWCASE in the look book: 24 px narrower, the others
shift), text widths from whole-pixel font sizes (the caption 10 px over
51 letters, the keys 1-5 px, GUN GAME 63 for 63.4 px), the mark 1 px
wider (its image's width rounded up). Follow-ups (2026-11-14): CS:S's weapon selection row
(`HudWeaponSelection`, drawn on every deploy with fast switch,
`weapon_select::show_deploys`) no longer shows while a minigame plays
(the gun game swaps guns on every kill and the race chip, top centre
where the row draws, shows the gun; it overlapped the chip; test
`a_drawn_weapon_shows_the_row_except_under_a_minigame`); the logo pass's
mark measured against the look book at p = .4 and .5 (`ui_pass`, the
look book's lobby frame with its sting set to the same progress): the
same upright mark in the same place, edge pixels only differing, so the
pass needed no change. Left: the menu's sounds, M5b's Showcase in slot C, screenshots at
the four sizes beyond 1080p.

- `party_kit::main_menu` drawn with the kit (four items from
  main_menu.toml; Play, Join, Options, Quit), the lockup, the keys line,
  backdrop tabs. Pages (Play, Join, Options) restyled on the kit's
  widgets; the VGUI look goes from this menu only.
- Backdrops as 2D scenes (`Mesh2d`, `Text2d`) on a `RenderLayers`
  camera drawing into an image under the menu UI: **Ladder** (14 rungs
  in perspective, one step a 1.05 s with `quint_in_out` .48 s),
  **Assembly** (each gun drafted on the 16-unit grid, traced by a 12°
  scan, filed onto a shelf; 3.4 s a gun), slot C from look book V, and
  the ambient shader (`AmbientMaterial`) under them. They rotate every
  16 s; the live background map stays an option when at hand.
- Tests (`apps/party_kit/tests/it/main_menu.rs`): `menu_has_four_entries_from_data`,
  `backdrops_rotate_every_16_s`, `backdrop_tab_click_resets_the_dwell`;
  scripted runs (`--map`, `+connect`) skip the menu as before.

### M5b: the Showcase backdrop (in-engine)

A backdrop rendered in 3D: when games are installed or cached, a
character of one game holding a gun of another (the mashup idea) in its
idle animation on a slow turntable, or **a gun alone** under an inspect
camera, in a dark studio with a soft key light and a cobalt rim, framed
in the right two-thirds behind the menu's ink fade. Drag rotates, the
wheel zooms. Nothing installed or cached: the 2D backdrops only.

**Reuse.**

- Bodies and guns: `map::add_character_model` and `map::add_held_model`
  (`chimera/world/src/map/bodies.rs`), built off the main thread by
  `render::late_build`; drawn and posed by `chimera_present::render::
  bodies` (`attach_held`: the held model on the body's bone, its
  merged parts); animation by `map::anim::Animator`. Combat Arms' bodies
  and guns load from its install as the ladder's do
  (`combat_arms::game::characters::preload`: `bodies_on_demand`,
  `held_on_demand`); CS:S's and HL2:DM's come from their installs or
  the asset cache like a map's kit.
- The idle and the hand: each game lends them as data, a new optional
  `GameParts::showcase` (`chimera_core::game_parts`): its idle sequence
  per body family, its hand bone, and its held models' grip bone.
- **Cross-game hold**: `attach_held` finds the held model's bone by name
  on the body's skeleton (`joint_of`), so another game's gun finds none.
  The Combat Arms attach-sockets work (main, d3684716) fixed per-model
  part offsets within one game; on main there is no map between games.
  New: `map::socket_map` (data, `chimera/world/data/sockets.toml`): a
  held model's grip bone to each skeleton family's hand bone, with an
  offset and rotation per pair, and a test that a CS:S rifle sits in a
  Combat Arms hand (`socket_map_puts_a_cs_gun_in_a_ca_hand`). This also
  serves any later mashup of bodies and guns.
- The scene: the menu's background map machinery (`main_menu`'s
  `pick_background`, `BackgroundMaps::hold`, the local load) loads a
  **studio**, a map built in code like the greybox (`chimera_world::
  studio`: a floor disc and a dark cyclorama, no collision, no
  spawns; not the greybox, which the user ruled out behind the menu),
  with one showcase entity (body, held model, animator; no player, no
  intent).
- Pairs: `ShowcaseQueue` cycles character/gun pairs on each backdrop
  swap; sources in order: the server's coming ladder (its next rungs),
  the last match's winner's gun, then random from what is at hand.

**Loading without blocking.** The menu never waits: pairs are chosen
only from what is installed or cached (`background_at_hand`), models
load on tasks (`late_build`; `Assets::insert` only on the main thread,
its per-frame cap), the Showcase tab shows the 2D backdrop until its
models are in, then crossfades. Downloads stay held as for the
background map. The guard: no menu frame over 8 M main-thread
instructions while a showcase loads (as `late_build`'s test).

**Rendering.** A 3D camera on its own `RenderLayers`, drawing into an
`Image` (render-to-texture) shown by an `ImageNode` under the menu's UI
(so the ink fade, the tabs and the logo pass sit over it); its size the
backdrop area's physical size (two-thirds of the width; the full window
in gun-only mode, at full resolution). The camera is active only while
the menu shows the Showcase; when idle it renders at most 30 fps
(`Camera::is_active` toggled), 60 while dragged, none behind a page or
the game.

**Input.** Drag and wheel only while the cursor is over the backdrop
area and no menu node is hovered (Bevy picking on the `ImageNode`); the
menu's items keep keys and clicks. A drag eases back to the automatic
motion after 3 s idle.

**Gun-only mode.** One gun (any game's, at hand) on an invisible stand:
slow orbit, tilt up and down, a gentle float, an eased dolly; drag and
zoom as above. **Every swap feels different**: camera moves and light
rigs are data (`apps/ultimate_gun_game/data/showcase.toml`: `[[move]]`
orbit speed and arc, tilt range, float, dolly curve and easing, lens;
`[[rig]]` key, fill and rim directions, colours, intensities, the
floor's tone), and a seeded shuffle never repeats a move or a rig twice
in a row (`showcase_moves_never_repeat_back_to_back`).

**The look switch** (`menu_look game|studio`, archived, default `game`;
in Options):

- **Game** (default, parity): the models' own `PropMaterial` and shaders
  as in play. The studio lights them through what that material reads:
  the probe's ambient cube set from the rig, the key and the rim as
  `DynamicLight`s (clustered point lights), the blob shadow on the
  floor.
- **Studio** (opt-in): an enhanced layer. Per model, a `StandardMaterial`
  built from the same textures (base colour; normal map where the
  material has one; roughness and metal derived by rules per material
  class: from Source's `$envmapmask`/`$phongexponent`, Combat Arms'
  environment layer as a metal mask, else keywords of the material's
  name: metal, wood, cloth, skin, rubber, plastic), image-based light
  from a small studio cubemap we author ourselves as code
  (`GeneratedEnvironmentMapLight` over a procedural cube: softboxes on
  dark; no file), a spot key with soft shadows
  (`soft_shadows_enabled`, `ShadowFilteringMethod`), SSAO with TAA
  (`Msaa::Off`), `ContactShadows`, `Bloom` on the HDR camera for the
  rim, and maybe a subtle floor reflection (`ScreenSpaceReflections`
  needs the deferred prepass; a mirrored draw of the model may be
  cheaper).
- Before committing to Studio: an A/B screenshot set (the same pairs,
  moves and rigs in both looks, `ui_scene showcase <pair> <move> <rig>`)
  for the user to judge. Old diffuse-only textures may look plastic or
  wrong under PBR; if they do, Studio stays a dev option.

**Perf budget.** Main thread: the Showcase's systems under 1 M
instructions a frame (counted in a seeded scenario,
`showcase_frames_stay_in_budget`); loads as above (no frame over 8 M).
Render: Game look under 1.5 ms GPU at 1080p, Studio under 4 ms, on the
dev box's GPU; the menu frame cap (`fps_max_menu`, default 60; 15 while
the window is unfocused) applies to the whole menu.

**Tests**: `showcase_falls_back_to_2d_without_installs`,
`showcase_waits_for_models_without_blocking`,
`showcase_input_only_over_the_backdrop`,
`showcase_moves_never_repeat_back_to_back`,
`socket_map_puts_a_cs_gun_in_a_ca_hand`; heavy (with the installs):
`showcase_pairs_draw` (every pair at hand shows within N frames), and
offscreen screenshots per look. Nothing extracted is committed; all
models and textures load from the install or the cache at run time.

### M6: session wins

Status (2026-11-14): done on `ugg-ui-m6`, for the playtest. Counted on
the server when a match ends with a winner (`minigame::wins::MatchWins`,
in memory, per server run: the user's decision), by player name, bots
by theirs; it survives map changes, `change_match` and a reconnect under
the same name. Shown from the server's data everywhere: the Tab board
(`MatchesWon` replicated on the characters), the match end (the results'
`ResultRow::wins`, this match counted, "+1" on the winner's), and now
the lobby (`LobbyRow::wins`: `lobby_panel::LobbyWins`, the trophy and
count after the name, the leader in the accent; the shell's own lobby
as a "2 WON" tag). **net50 to net51** (`LobbyRow`). Tests:
`session_wins::session_wins_survive_a_map_change_and_a_rejoin`
(chimera_net NetSim: two matches, Ann's then the bot's, the lobby's rows
on the new map, Ann's reconnect), `lobby_panel::
session_wins_show_in_the_lobby`, `race_chip::
the_board_follows_the_replicated_wins`, and the earlier
`a_won_match_counts_for_its_winner_and_the_count_survives_a_restart`,
`every_board_row_has_a_gun_icon`, `match_end_signal::
standings_carry_the_marks_and_the_winner_sweeps_in`.

Left: the key stays the name (a rename resets, two players with one
name share): the client's connection id changes on every reconnect, so
it can't carry a count over one; a per-install player id in `Join`
would (later, with open question 4, a listen host's restart). The race
chip itself shows no wins (the board does).

Network bumps, in order: M3 net49 (match effects; net48 went to HL2:DM's
blood per viewer), M4 net50 (results'
climb, four vote cards), M6 net51 (lobby wins). Each bump in its own
commit with the test that needs it; a playtest's server and clients
must be one build.

### UI sound

Status (2026-11-14): in, for the playtest. The kit is ours (v7,
synthesized by `tools/sounds/synth.py` from code, no samples; CC0 1.0,
`apps/ultimate_gun_game/assets/sounds/ui/LICENSE.txt`; rebuild:
`tools/sounds/README.md`). 48 files, 14.3 MB of 48 kHz 16-bit WAV (mono
where the sound is: the slider, the ticks, Tab), embedded in the app's
binary (`signal::UI_SOUNDS`); the A/B variants, the backdrop swap and the
breakcore loop are not shipped. `own_sounds_carry_their_licence`
(repo_checks) allows WAVs only in an app's `assets/sounds/` beside a CC0
licence, 5 MB a file, 20 MB in all.

- **Engine**: `chimera_ui::sound` (`UiCue`, `UiMusic`, `SoundTable`);
  `client::ui_sound` (`UiSounds`, the `ui` bus: unpositioned `LiveClip`s,
  `PlaybackSettings::DESPAWN`, no room DSP; `snd_uivolume`, default 0.8,
  `SoundMix::ui`; voice limits; per-play pitch and gain spread; the
  slider's speed from its value, 0.5 to 2.0; a 4 dB duck while an
  `announcer::AnnouncerVoice` plays; the music loop on the music bus,
  `snd_musicvolume`, looped from the start of a seamless file);
  `client::ui_cues` (the lobby, the rank-up, Tab, the kill feed, the match
  end and its vote, read off their views). Only a theme with
  `flags.sound.ui` (Signal) plays anything.
- **Data**: `apps/ultimate_gun_game/data/ui_sounds.toml` (the event
  table); the Audio tab's "UI volume" row (`apps/party_kit/data/
  options.toml`, `when = "ui_sound"`: only in an app with UI sound,
  `esc_menu::OptionTabs`).
- **Hooked**: menu hover, select, back, denied (a press refused while
  loading, a failure box or a reconnect; a non-host's Host), slider (a
  range row's step) in the party's main and Escape menus
  (`party_kit::ui_cues`); the logo pass (`pass_sweep` once per pass
  begun, `LogoPass::runs`; not for the reduced-motion fade); the lobby's
  ready and unready, another player readying, the countdown (10 to 4, 3,
  2, 1, go); the rank-up (the nearest of the 17 renders, p = rung /
  (rungs - 1), once per rung up), the knife on the last rung, the chain
  within the toast's window (`motion.dur.toast` hold_to + leave, 1.16 s,
  on the UI clock); Tab and your kill-feed line (off in the table); the
  match end's reveal; the map vote's cast, others', last 10 s and lock;
  the match effects (M3's `MatchEffects` through `match_effects::
  EffectsView`): a threat's `air_alert` at its start, `air_pulse` on the
  edge's pulses, `hud_chip` as its banner folds, `air_impact` at its
  impact, another effect's `hud_chip` (`ui_cues::effect_cues`); under UI
  sound Combat Arms' strike siren is muted on the client
  (`chimera_present::audio::live_sound::MutedSounds`,
  `party_kit::ui_cues::mute_siren`); the sci-fi loop on the main menu in
  place of Combat Arms' playlist (Combat Arms' music silent meanwhile:
  `combat_arms::game::music::AppMusicPlays`). The backdrop swap stays
  off.
- **Remote clients** (audit 2026-11-14, after playtest 7's report of no
  rank-up sound for players): every cue reads state a remote client of a
  dedicated server has (the replicated `Rung` and ladder, `NetLobby`, the
  match's effects, results and vote), none a server-only event; on
  `NetSim` with a dedicated server and two clients each hears its own
  cues once (ultimate_gun_game `net_ui_sounds`: rank-up, knife, no cue on
  a rung down, lobby, countdown, air strike over a slow link and a late
  join, match end, vote, `snd_uivolume`). One fix: a remote voter's own
  vote, shown at once but counted when the server's count comes back,
  also played `vote_other` (`match_end_cues`' owed votes). The playtest's
  silence did not reproduce there; the bus is under the master
  `GlobalVolume`, which Combat Arms' unloaded profile set to 0 at every
  start of the party apps (`ShellOptions`, branch `pt7-ui`, fixes it).
- **The loop's level**: the music bus, `snd_musicvolume`, which the party
  apps take from Combat Arms' music option; a first play's profile (the
  template) has music 50 and effects 70, so a fresh player hears it
  (`t21_music`). Tool runs load no profile: master and music are 0 there
  (set them over the console).
- **Left**: M4's vote cards keep the cues (`ui_cues::match_end_cues`
  reads `MatchEndView`). The game's winner sound plays with
  `end_reveal` (the sound page's note: delay the game's 0.25 s or drop
  ours if they clash). Follow-up: a Rust port of `rank_up(p, seed)`
  rendering each match's ladder on a task at its start (no rank files),
  and a compressed format with gapless loops (Vorbis) for the loop.

### Names (2026-11-14)

A player's name is at most 20 characters (the user's decision): the
server cuts it at a join, a rename and in the userinfo, takes control
characters out, trims it, and numbers a name another player has ("Ann
2", within the 20; `chimera_net::net::server::NAME_MAX`, `clean_name`,
`unique_name`). Every Signal widget that shows a name fits it to its
place with `…` (`...` in the UGG faces v0.4, which have no `…` glyph;
`chimera_ui::text_fit`: `Ellipsize`, measured in the kit's faces by
`KitFonts::width` and corrected from Bevy's layout where kerning widens
it): the kill feed (7.25 cq a name: 8 less half of what the bigger
gun icons take more, so a row stays within the feed's 26 cq), the race
chip (the leader's 5.8 cq, the one you lead 4.8 cq: 7 and 6 less what
the 4 cq gun box takes more, so the plate stays within 23 cq), the Tab board's name
cell, the lobby's rows, the match end's winner (44 cq), standings, the
climb's head (12 cq) and the vote cards' maps, the rank-up toast's gun.
The feedback dialog shows no name. `ui_scene_name <name>` calls every
staged player `name` (screenshots); ultimate_gun_game `names_fit` lays
twenty W's out in each at 1280x720 and 1920x1080.

### M7: Settings and Esc menu (first drafts)

Status (2026-11-14): in progress, first drafts for the playtest (the
"UGG Components & Views" page's Settings and Esc menu frames and its
component table; only settings that exist). Step 1 in:
`chimera_ui::controls` (buttons primary/secondary/ghost/danger with key
caps, sub-tabs, rows and section headers, toggle, segmented, slider,
stepper, dropdown, key-bind field, key/value lines, modal plates; one
state vocabulary: hover a faint tint (the right-edge tick dropped
2026-11-14), focus the accent bar, selected a white fill, a
toggle on white, threat as a mark only; `Wipe`/`wipe_cover` for moves
inside a screen), `UiSlant::outline`; `ui_scene controls`; tests
`chimera/ui/tests/it/controls.rs`. Step 2 in: the Esc menu
(`party_kit::esc_signal`, `[layout.esc]`): a left panel over a gradient
shade (the chip and the feed stay readable), the lockup, the paused
line, the `[[escape]]` entries with the HOST chip, ↑↓ and Enter, the
selection a white slab wiping along the slant, a running vote's box
(question, counts, pips, F1/F2), the host's leave in a threat-barred
modal with Stay first, the keys and "Match live"; `ui_scene esc`; test
`esc_menu::signal_keys_move_the_selection_and_stay_comes_first`.
Step 3 in: Settings (`party_kit::settings`, `[layout.settings]`), one
screen for the menu's Options (over the ambient, `AmbientMaterial::MENU`),
the Esc menu's Options (over the match) and Host settings: sub-tabs
Video, Audio, Controls (options.toml's Mouse rows plus the Controls
page's binds with the Shared / Combat Arms / CS:S switch and each game's
gun view FOV), Party; rows by pick (slider, toggle, segmented, a
`widget = "dropdown"`, stepper; key fields); ↑↓ ←→ Enter Q/E Delete;
the help pane (TOML `help` else the cvar's, console name, default,
range); the Apply / Revert bar (Ctrl+S / Ctrl+Z) over a deferred model
(2026-11-14: a change is pending, its row marked in cobalt, until Apply
sets the cvars and writes the config; Revert drops them; volume and
sensitivity sliders preview live and are set back on Revert or Discard;
key binds join it, restored from a snapshot; leaving with changes
pending asks Apply / Discard / Stay in a modal; anything else closing
the page discards; Host settings stay immediate); an applied
display-mode change reverts after 15 s unless kept (the config written
again); a page change wipes the rows in on the slant;
the rows (Settings' every page, Play's) in the kit's scroll container
(`chimera_ui::scroll`: bar, eased wheel, right-button pan and fling, ↑↓
keeping the focused row in view; Join's server list keeps its 8-row
window); sliders that drag (a press jumps there; the value shows and a preview
follows while dragging, pending once at the release with one slider
cue; ←→ a cue a step) and whose value box takes a typed number (any
finite one, past the range too; options.toml's `hard_min`/`hard_max`
refuse the unsafe, why shown beside the box; Enter or a click away
keeps, Esc puts back: `controls::typed_number`, `Ui::slider_ex`,
`stepper_ex`); `ui_scene settings <tab>` (`settings_set <cvar> <value>` and
`settings_press <button>` stage changes and the prompts); tests
`party_kit` `settings::`. Polish
(2026-11-14): the games' own HUDs (HL2:DM's root, Combat Arms' roots
and death screen, CS:S's top-level parts: `client::hud::HudPart`) are
hidden under the Signal Esc menu and Settings over the match
(`client::hud::HudCover`, M4's match end uses it too; test
`esc_menu::signal_menu_covers_the_games_huds`); the Esc menu's shade is
`chimera_ui::ShadeMaterial` (eased, per pixel in float, dithered: the
gradient node's 8-bit stops banded it in wide stripes). Settings fixes
(2026-11-14, the user's feedback): a value box typed into keeps its box,
face and size, its text one node a character at rest and typing alike,
the caret an overlay that takes no room, a click placing it between
characters (`controls::TextHits`), a drag selecting, a double click
selecting all (`TextPress`), the same in the kit's `text_field` (Play,
Join, the password); no hover tick anywhere (rows, list lines,
buttons, fields: an element's own fill lifts; `widgets::hover_tick`
gone); the toggle's labels centred on their slabs, its slab sliding
over (140 ms, none under reduced motion), its focus a strip on the
track's bottom edge; the dropdown's mark the `chevron` icon, flipped up
open; the Party page without the hit marker, others' voice lines and
the radar (party defaults on, on, off); the hover cue once per row
(`ui_cues::CueItem`), 90 ms apart, none while a button is held, 4 dB
under select. Left: the Crosshair and Multiplayer
pages (no rows exist yet), help previews (FOV fan, crosshair), a key
bound twice flagged, the UI parity check for both screens (no look-book
reference frames in `tools/ui-parity/reference/` yet), 720p/1440p/4K
screenshots.

### M9: staging between matches

Status (2026-11-14): in on `lobby-staging2`. The user's report from
playtest 7: another game's HUD showed over the lobby, nothing in the
lobby could be clicked while the map loaded, and Esc gave the in-game
menu (Disconnect and the rest); the map was loaded, started, the player
given a gun and only then the lobby screen taken down.

**Before** (traced on `party_kit`'s `lobby_staging::
trace_match_end_to_next_match`, a two-rung ladder on two test maps, the
lobby's countdown 1 s, clients through the shell's own load; times are
the server's clock, s):

| s | What happened |
|---|---|
| 0.07 | first join: the client's character spawned dead (`Connecting`) |
| 0.10 | the client joined: the lobby shows over the game (screen `InGame`), HUD not covered, world drawn, mouse captured, Esc the match's menu |
| 0.14-0.17 | the lobby still up: the server took the player in and spawned it **alive with its rung-0 gun**, seen on the client |
| 1.14-1.19 | countdown over: respawned together, the clock starts |
| 11.16 | match over (results; with UGG's vote, 20 s of vote and 1.6 s reveal) |
| 19.17 | the hold, the server's `changelevel` |
| 19.21 | the client hears `ChangingLevel`: the loading panel (input to the menu); **the old map stays loaded** until the new one replaces it |
| 19.65 | the server's map in: the lobby begins and at 19.66 every player is **alive with a gun** on the new map |
| 19.68-19.74 | the client loads the map (the old one goes only now); its player alive with a gun at 19.70 |
| 19.74 | the client's load done: screen back to `InGame` under the lobby: the HUD over the lobby, mouse captured, Esc the match's menu, until the lobby ends at 20.82 |

**After** (the same trace):

| s | What happens |
|---|---|
| 0.07 | the joiner's character is staged (`minigame::Staged`: dead, no weapon, out of the game) |
| 0.10 | the client joined: the lobby owns the screen: HUDs covered, world's view off, input to the menus (mouse free), Esc the lobby's (since `lobby-menus`: closes what is open over it, else nothing) |
| 1.14 | countdown over: everyone let in (`prematch::let_in`), spawned on rung 1 with its gun at 1.16, the clock live at 1.19; the lobby leaves through the logo pass, the cameras and HUDs back |
| 19.21 | `ChangingLevel` after the match's end: the loading panel covers the world (`WorldCover` `MENU`) and the map left is unloaded under it (`staging::UnloadAtCover`) |
| 19.66 | the next lobby: everyone staged again (no gun, dead) while the clients load |
| 19.68-19.75 | the client loads and joins behind the lobby, which owns the screen throughout (no pass under it) |
| 20.79-20.84 | countdown over: everyone spawns on rung 1, the clock starts |

**The flow now**:

- **Server** (`minigame::runtime`, `chimera_net::prematch`): while
  `PreMatch` stands, every character (remote players, a listen host's,
  bots) is staged as it appears (`runtime::hold_out`, first in the
  rules' chain: `stage` takes its weapons, makes it dead, not solid and
  `Connecting`, out of the minigame); `join` takes nobody in; a client's
  `Loaded` during the lobby records its map but doesn't enter it
  (`server::EntryHeld`). The lobby's end (`prematch::end`) restarts
  the match and lets everyone in (`let_in`: the join rules' `enter`; a
  client still loading stays out until it has the map, as a late
  joiner), so all spawn together on the first rung and the clock
  starts; the simulation of the match is paused meanwhile (phases held,
  the end freeze on).
- **Client** (`chimera_client`): while the lobby shows
  (`prematch_lobby::lobby_shown`) the world's 3D cameras are off with a
  blank camera clearing the window (`staging::WorldCover`), the HUDs
  covered, our Signal HUD too (`hud::LOBBY_COVER`); its Ready, Settings,
  Report a bug and Leave are buttons (`LobbyAction`, `lobby_press`;
  they work while downloading or loading, the server keeping Ready,
  `LobbyAsked`); Settings and Leave go through the app's
  `LobbyAppAsked` (`LobbyAnswered`), Report a bug is the report dialog;
  an app's overlay over it (`LobbyOverlay`) hides the panel's header and
  key hints. A change of level once the match is over unloads the map
  left as soon as a cover is up (`staging::UnloadAtCover`, `MapLeft`).
- **Party apps** (`party_kit`): the lobby owns the screen whatever the
  menu's screen (`main_menu::owned_by_lobby`): drawn on the menu's camera
  with the shell's (HUDs') off, the mouse free (`AppMenu::open`), no pass
  for moves under it; the loading panel covers the world too
  (`cover_world`). A mid-match join has no lobby: straight in after
  loading, as before.
- **Lobby menus** (user's call, 2026-11-14, `lobby-menus`; replaces the
  lobby's Escape menu, whose `lobby = true` entries are gone): the
  themed lobby is a full screen with its own header buttons: Invite and
  Match settings (inert, as before), Settings, Report a bug, Leave.
  Settings opens as an overlay over the lobby (`esc_menu`'s
  `EscMenu::lobby`, Settings' `Context::Game` with "Lobby ▸ Settings",
  the 86 % shade, `settings::LOBBY_SHADE`): the lobby stays live under
  it (countdown, rows, Ready kept), every setting allowed; the match
  starting under it keeps it open (now the match's, Back to the
  Escape menu's first page, nothing pending lost; the logo pass
  doesn't close it). Leave asks first (Stay focused first) on the same
  flat shade, no panel. Esc only closes Settings, Leave's confirm and
  the report dialog; with nothing open it does nothing. While anything
  is over the lobby its header and key hints hide (no ghosted header,
  no doubled hints: the old lobby menu drew its match gradient over
  the lobby). Lucker Party's plain lobby keeps the match's Escape menu.
- No network change (NET_VERSION stays).

Tests (`apps/party_kit/tests/it/lobby_staging.rs`):
`nobody_plays_in_the_lobby_and_everyone_spawns_at_its_end`,
`escape_in_the_themed_lobby_only_closes_what_is_open`,
`escape_in_the_plain_lobby_opens_the_match_menu`,
`settings_over_the_lobby_keeps_it_live_and_ready_kept`,
`settings_stay_open_across_the_match_start`,
`the_lobby_buttons_work_while_a_client_still_loads`,
`the_map_left_is_unloaded_before_the_next_lobby_shows`, and the trace
(`--nocapture`). The prematch tests (`chimera/net/tests/it/
prematch_lobby.rs`) check the staged players.

Left: a replicated weapon's despawn reaches a client a few frames after
the staging (its player shows the old gun under the lobby for up to
~0.1 s, drawn nowhere); the plain (unthemed) lobby panel has no
buttons (F3 and Esc only); M8's loading screens (the "Next map" screen
06) would replace the VGUI loading panel between the match end and the
lobby; offscreen screenshots of a real level change (not just
`ui_scene lobby`).

### Signal revision 8 (2026-11-15)

The user approved the kit's design rules' revision 8 (the design
session's `spec.md`, sections 9-17; its mocks `r8-lobby`, `r8-menu`,
`r8-host`, `r8-dialog-icons`, `r7-load`, `r7-load-confirm`,
`r7-host-maps`, `r7-lobby-bar`, `r6-avatars`, `r6-join`). Implemented in
four stages, each merged on its own:

1. **The shell** (in): `chimera_ui::shell`: the persistent bar (the
   lockup on the root; elsewhere the stacked breadcrumb "‹ MAIN MENU"
   over the title, its Esc cap and the right mouse button, the back
   target); at the right one rack of square tiles (T1): "PARTY n/8",
   your avatar with its edge strip, the "+" invite slot, then Friends,
   Settings, Report a bug (F10); the Settings tile white while Settings
   is open; no Play/Join tabs. Right-click is Back on every shell screen
   (`ShellScreen`) except while a bind listens (`BackGuard`): it and a
   click on the breadcrumb press Esc for a frame, so each screen's Esc
   path does Back (the lobby's Esc is now Leave, which asks first).
   Margins per 11.4 (`[layout.shell]`: 2.6 cq, the bar from 1.8, the
   rule at 7.2, the window 8.4 to 4.6 from the bottom, the side panel
   31 cq). Play is shown as HOST, Options as SETTINGS. Unbuilt features
   are disabled (`shell::Disabled`: `fg_4`, no hover, no sound, a "Coming
   soon" tooltip): Friends, the "+" (and your avatar opens nothing yet:
   no Profile screen). The main menu's news card (`signal_menu::
   MenuNews`, hidden while None; `menu_news` sets it) waits for the asset
   service to send one. The hero (`Tone::Hero` #2D6BEB, white text) and
   the key-cap lockup came in with it (stage 3 finishes the controls).
2. **Avatars and status** (in, with stage 1): `chimera_ui::avatar` (4
   glyphs x 8 colours, icons.toml's `avatar_*` from the design's SVGs),
   `cl_avatar` (saved, userinfo, random on the first run: `chimera_client::
   client::profile`; the lobby's rows carry it, `LobbyRow::avatar`,
   NET_VERSION net55), the YOU frame, the edge strip (ready in the lobby,
   cobalt in a match, filling while loading, the leader's cap in a party
   of two or more), away (unfocused: the crossed eye; idle 5 min: grey;
   `presence` fakes them). The lobby already is revision 8's list (one
   list, you first, avatars, the progress bars, ping bars and ms, no
   ready-up: `mp_lobby_host_start`, the host's Start as the hero).
3. **Controls**: the hero (white on #2D6BEB; the dialog's hero bigger
   than its secondary), the thin caret, the progress component (variant
   B), ping bars and ms with the loss mark, the list-with-chevron field,
   STATUS beside status blocks, Q/E caps at the tab strips' ends,
   slanted segmented groups, Mono only for data, instant apply (display
   mode and resolution wait for Apply and the 15 s keep).
4. **Screens**: the main menu, Settings, Host (name, password, port
   first; the Maps field opening the picker dialog; "Start when: host
   presses Start"; the UPnP status block), Join (section 13's fixes), the
   lobby (one list, your party first, no ready-up, Start the host's
   hero), Loading (MB of MB, files, speed, time left; the leave dialog).

### M10: Chat

Status (2026-11-15): in on `ugg-chat`. The user: "add a chat to
Ultimate Gun Game ... centralize it, make sure it's consistent, and use
our new font with it."

**Audit (before)**. Chat-like text came from five places and drew in
three looks, none of them Signal:

| What | Fed by | Drawn by |
|---|---|---|
| CS:S chat (`messagemode` Y, `messagemode2` U; `say`, `say_team`) | `net::chat` (`SayRequest` to the server, which picks the readers and sends `ChatMessage`); a game of one's own in `client::chat::say` | `client::chat` at CS:S's `HudChat` place (its scheme's ChatFont, 12 s a line, `hud_saytext_time`), its "Say :" line under it; always on, under every game's HUD |
| Radio lines (CS:S Z/X/C) | `games/cs_source/ui/radio.rs` writes `ChatLine`s | the same box |
| Joins, leaves, renames, team joins, CS:S objectives, a dedicated server's `say` | `net::PlayerCame`, `NameChanged`, `ChatMessage::console`, `team_joins`, `objectives_hud` as `ChatLine`s | the same box |
| Refusals ("You are flooding the server") | `net::Notice` | `client::chat`'s hint box (`HudHintDisplay`) |
| Combat Arms' chat and radio (Enter, F2, F3; B, V, N) | its own protocol (`CaSayRequest`, `CaSayIn`, `CaRadioIn`) and log | `games/combat_arms/src/ui/talk.rs` (its panel, input bar with tabs, radio menu): always drawn, not under a `HudRoot`; in the party apps the shared binds are Combat Arms', so Enter opened this one and Y/U were unbound |
| Bot chat ("AI_Alpha_3 : Gotcha, friend.", seen in the playtest) | `combat_arms::game::bots::chat` (`CaBotChat`, only on a host: not sent to clients) | `ui::bot_chat` as a `ChatLine` in the CS:S box |
| HL2:DM | no chat of its own: its team colours in `client::chat::run_color` | the CS:S box |
| "X is on the last level!" | the minigame's callout (`last_rung_other`) | `client::callouts`, centre screen (not chat) |
| Votes | `net::vote::VoteView` | the Esc menu's vote box (no line) |
| Voice | `net::voice` (`VoiceActivity`) | Combat Arms' talk icon; no Signal indicator |
| The console | every `ChatLine` printed by `client::chat::collect` | the console |
| The lobby | no chat | - |

Moderation was already on the server (`net::chat`): `clean_say` (control
characters out, trimmed, 127 bytes), a flood limit (four lines in a
burst, then one a second; radio three, then one every 1.5 s; the sender
told), names cut at 20.

**After**:

- **Entries** (`client::chat::ChatEntry`): every player line, notice and
  word of the server is one structured message (kind, sender, name, team,
  text) carrying its line in the game's format. Without the Signal look
  `route_classic` turns it into the game's `ChatLine` and sound as before
  (CS:S, Combat Arms, HL2:DM, Lucker Party unchanged).
- **One feed** (`client::signal_chat`, on the kit's `chimera_ui::chat`)
  under the Signal look (`SignalChatActive`): players' lines (name in
  the player's colour, `name_color`, yours in the accent; BOT or HOST
  beside it; TEAM or RADIO before it), notices (a slanted accent mark, the
  sentence in `fg_2`: joins, leaves, renames, team joins and objectives,
  "X is on the last level!", votes called and ended) and the server's word
  (SERVER tag, `fg_3`: the dedicated server's console, refusals, the
  flood warning). Kills stay in the kill feed. Bottom left above the
  health (`[layout.chat]` left 2.2, bottom 8.4, w 30 cq), six lines, each
  held 8.5 s then faded over 1 s (`hold`, `fade`). The game's chat box,
  its typing line and hints stand down; Combat Arms' panel draws no
  queue or input (its radio menu stays), its radio and lines go to the
  feed (`talk::signal_entry`), its bot chat too (with the BOT tag).
- **Input**: Y and U (`messagemode`, `messagemode2`, now the app's own
  default binds, `APP_BINDS`; `APP_BINDS_MARK` 2 puts them on free keys
  of saved configs once) and Combat Arms' Enter, F2, F3 open one field:
  the kit's caret, selection, copy, cut and paste; Tab switches all and
  team; Enter sends, Esc cancels; 127 bytes; every key taken while open.
  Open, the history (12 lines, with session time stamps) and the field
  sit on one plate whose right edge leans 12° (the user's "slanted plate";
  the left edge straight, as rule 1 keeps text in straight edges).
- **Docked** in the lobby (under `LobbyRoot`) and on the match end
  (under `SignalEndRoot`): the history always shown, the field below
  ("Press Enter to chat"), Enter opens it there too; hidden while
  Settings or the report is over the lobby.
- **Mute**: `ignore <name>`, `unignore`, `ignorelist`, or a click on a
  name in the open history (people by user id, bots by name, for the
  session); past lines hide too. **`cl_chat`** (Settings, Party: On,
  Team only, Off; archived) for everyone's lines.
- **Sound**: `chat_in` (the lobby's peer tick, 6 dB under) for another's
  line, at most one in 1.5 s; none for your own, notices or the server.
- No network change (NET_VERSION stays): the existing messages carry
  everything.

Tests: `chimera_client` `signal_chat::` (the game's chat without the
look, notices apart, the fade, the keys taken and Enter/Esc/Tab, mutes,
`cl_chat` and the cue's limit, the lobby dock); `ultimate_gun_game`
`chat::` on `NetSim` (two clients' lines and cues, team lines within the
team and `cl_chat 2`, the flood limit and its warning, `ignore`, the
lobby's chat and joins as notices). Scenes: `ui_scene chat [open|notice]`,
`ui_scene chat_lobby`.

Lines and the field are set in the text face (`Role::Text`, with lower
case, at `controls::TYPED`) since `fonts-wire` merged. Left: a mute button on the Tab board (held open with the
cursor captured today); a talking indicator in the feed (voice); wall
clock stamps; bot chat for network clients (a host's only today).

## 6. Decisions (user, 2026-11-14)

1. Icons (revised 2026-11-14): each game's own kill icon everywhere it
   has one (kill feed, Tab board, rank-up toast, the Ladder backdrop):
   CS:S's and HL2:DM's are icon-font glyphs (vector), Combat Arms' are
   bitmaps sized so they stay sharp. Our fallback set (by `GunClass`,
   with per-weapon overrides for knives, crossbows, energy guns) only
   where a game has none: our own guns, games without icons, assets not
   downloaded yet. One theme key (`icons.prefer_game`) flips it; `chimera_ui::icons::choose` applies it.
2. Tab: one board, the chip opening into it. The board is big enough to
   carry everything the full scoreboard had (rung, gun icon and name,
   K/D, ladder strip, tags, session wins). Its open and close are fast
   enough to spam Tab (well under the prototype's 240/200 ms; aim for
   about 120 ms open, 90 ms close, no row stagger that delays reading).
   Spectators on the board: later (backlog).
3. The font build scripts are committed under `tools/fonts/` as the OFL
   source, with the TTFs.
4. Session wins: per server run.
5. No compatibility with older clients: everyone runs the latest build;
   drop the HUD-text air strike.
6. The menu's Showcase may show any installed or cached body and gun
   (the menu is outside any lobby); the coming ladder isn't a constraint.

7. Bigger game icons (user, playtest 7, 2026-11-14): the kill feed's
   gun icons were too small beside the names. A game's own icon is now
   fitted by its visible ink (`chimera_client::client::ink_icon` on
   `chimera_ui::icon_ink`: an image's lit ink rectangle, a glyph's
   outline rasterized at the node's pixel height, centred in its box)
   instead of by its whole art, everywhere the Signal HUD draws one: the
   feed's gun 1.25 cq tall in its 1.85 cq row (the look book's .78), its
   headshot mark 1 cq; the race chip's gun 1.3 cq in a 1.3 x 4 cq box
   (.9 x 2.8); the board's and the toast's at their old heights, by
   ink. A deliberate deviation from the look book: the parity
   references (`tools/ui-parity/reference/style.css`, `.kf .e .gi`,
   `.c0 .gi`, `.c0 .gw`) carry the new sizes. The feed has no other
   kill-modifier marks yet (no penetration flag: `kill_feed`'s
   shortcuts); a new one goes through the same ink fit.

Look Book V decisions (user, 2026-11-14): menu slot C is the Ident
(the mark signs on facet by facet on one 12° scan, holds with a sheen,
signs off); an ambient full-menu fragment shader (slanted bands, a
faceted field, one cobalt bloom, near-still behind the menu column) sits
under everything. Player marks: tags are solid fills (HOST cobalt, YOU
white), a player who is both gets one fused chip split on the 12° slant;
BOT is the antenna robot icon (16-unit grid) in a slot reserved left of
every name; YOU's row is tinted and outlined, visibly different from the
hover state on hoverable rows. Kill and death are real grid columns with
their marks right-aligned over the numbers. The match-end background is
being redrawn as wide, overlapping translucent slanted planes (fewer,
broader bars, speed tied to width).

## 7. Risks

- **Small text in Bevy**: Parley with grayscale anti-aliasing only (no
  subpixel), hinting off by default; the tags (.42-.62 cq) are 5-8 px
  at 720 lines. Mitigation: a physical-pixel floor per role, hinting on
  for Mono under 12 px (`FontHinting::Enabled`), compared in offscreen
  screenshots at 720p and 1080p; UGG Mono may need a hinted or bolder
  small-size cut.
- **Atlases**: every height-scaled size is a new glyph atlas (1 MB,
  15.7 M instructions on its first frame). All roles and sizes declared
  to `text_warm`; few distinct sizes (round to whole physical px); a
  window resize re-warms behind nothing (accepted in menus).
- **Performance** (docs/performance.md: per-system instruction counts
  in seeded scenarios, never wall clock): budgets per milestone, counted
  with the frame log on the offscreen app and in `chimera_client` tests
  counting `ui_write` writes and layouts per frame: the HUD in play
  (chip, feed, toast idle) adds under 0.5 M main-thread instructions a
  frame and no layout when nothing changes; a toast or Tab opening under
  2 M in its worst frame; the menu under 3 M a frame. Animations write
  only `UiTransform` and material parameters where they can (no
  re-layout); materials run on `globals.time` where nothing else changes.
- **4K and odd scale factors**: pixel snapping after layout, hairlines
  1 physical px, tick bars by integer columns; screenshots at 125 % and
  150 % Windows scale as well as 4K.
- **Windows**: WGSL through DX12 (the repo's `.wesl` shaders already
  build there), fonts from bytes (no font folders), no case-colliding
  file names; the Showcase's Studio effects (TAA, SSAO, SSR) unmeasured
  on the Windows PC and the playtesters' GPUs: Game look is the default
  for that reason.
- **Look book V** may still change the menu's slot C, the ambient
  background, tags and the match-end background: M5 waits for it; M0-M4
  build the parts it does not touch.
- **Showcase**: another game's gun in a hand needs the socket map and
  will look wrong until tuned per pair; animation retargeting is out of
  scope (each body plays its own game's idle); Studio's PBR from old
  diffuse-only textures may look plastic (the A/B check decides); a
  render-to-texture camera with HDR, TAA and SSAO costs memory and GPU
  time even while idle (hence the cap and `is_active`).
