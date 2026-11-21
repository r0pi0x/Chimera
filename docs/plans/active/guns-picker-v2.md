# Host Guns picker, version 2

The host's Guns dialog rebuilt from the approved mockup ("Host Guns Picker"
v2, 2026-11-21; the review's notes and the mockup's "what changed since
v1" list are its brief): a preset at the top, the ladder's shape under it,
rules that fold, a browser of gun families for single exceptions, a help
tile. Today's dialog (one grid of about 2,100 cards, class chips that only
filter the view, `mp_guns_off`) goes. The kit pieces come from
`chimera_ui::form` (badge, big toggle, setting rows, segmented or
dropdown) and the user's kit preferences.

## Decisions

- **Families.** A gun family is the guns that are one gun in hand: Combat
  Arms' skins, editions and levels. Its signal is the install record's
  first-person model (`PVModelNormal`), not the name: the copies'
  documents get `family = "combat_arms:<model stem>"` when they are made
  (`games/combat_arms/src/game/data.rs`, hashed into the converter's
  version, so the data is generated and versioned with it). A family
  whose members name different classes splits by class
  (`<stem>.<class>`). Every other gun is its own family (its id). Ids are
  stable: the model's file stem, lower case.
- **Effective class.** A gun's class for the rules is its data's class
  (Combat Arms' copies: their template's), a pistol that hits like a
  heavy pistol counted as one (`power::band_of`). Its stage is its power
  band; a card says "plays as" when the band differs from its class's.
- **Rules are server cvars**, applied at each draw to what is there then,
  so new installs, downloaded skins and joiners are handled without a
  saved list going stale:
  - `mp_guns_games_off`, `mp_guns_classes_off`: what is off (empty: all on),
    so a game or class that appears later is on. Solo is "every other
    one off".
  - `mp_guns_skins`: `one` (default: a family is one candidate, its skin
    drawn at the deal) or `all` (every skin a candidate, the old draw).
  - `ugg_rungs`: 5 to 40, default 13 (`[rungs]` in
    chimera/sim/data/power_curve.toml holds the bounds and the default; a
    value outside snaps back with a note). Replaces the `RUNGS` constant
    as the source.
  - `mp_guns_short`: `shorter` (default; today's), `repeat` (the short
    rungs dealt again from the stages' guns, spread out, never the same gun
    twice in a row while another is possible), `borrow` (from classes the
    rules turned off, only in games that are on, the nearest stage first;
    never a banned or unavailable gun; none left: shorter). Applies only
    once the stages' own hand-off (a short stage gives its rungs to the
    nearest stage with guns to spare) has nothing left to give.
  - `mp_guns_ban`, `mp_guns_allow`: family ids or weapon ids (a weapon id
    bans or allows one skin). `mp_guns_off` stays as a ban list (old
    configs, the old picker) and is read as bans.
- **Precedence**: Unavailable (a game not every player has, data
  exclusions, no damage) then Ban then Allowed then Rules (games, classes,
  skins). An Allowed gun of a game that is off is drawn as one shared deck
  turn with the other such guns (`ALLOWED_GROUP`), so a single allowed gun
  doesn't take a whole game's share.
- **No silent fallback.** Rules that leave no gun at all: the draw logs a
  warning, plays every available gun and keeps the note
  (`GunRulesNote`), which the Host page shows. The dialog prevents it
  (red pool tile, Done disabled).
- **Presets** are small TOML files in the user's config folder
  (`console::cfg_dir()/guns/<name>.toml`): the rule values and stable
  family ids, nothing else. Built-ins live in the app's data. A dedicated
  server loads one with `guns_preset <name>`. Copy as text / Paste use the
  same TOML.

## Steps

1. Data: family ids from the converter, effective class, family listing
   (`auto::families`).
2. Server rules: the cvars, `auto::GunRules`, the draw's When short and
   skins, the allowed group, the logged fallback; tests of each rule and the
   precedence.
3. The dialog per the mockup v2 (preset dropdown with Save, Save as, Copy
   as text, Paste and an undo toast; the ladder-shape strip; folding rule
   sections with summaries and badges; Solo; the Rungs stepper; the
   browser with search, Group by, power sort, Show; family cards with Ban
   / Allowed and chance; the help tile; keys and controller; two columns
   below 1600 px). `ui_scene` stagings and screenshots at 1920x1080 and
   1280x720.
4. Host page: the Guns field as the preset dropdown and a summary line
   (and the fallback note).

## Progress

- 2026-11-21: plan written.
- Step 1 done: `family` in `WeaponData`; Combat Arms' install documents
  get it from `PVModelNormal` (`game::data::families`, in the converter's
  hashed sources); `auto::family_of`, `auto::effective_class`. The heavy
  test `families_install` checks the install's counts: 2,061 classed
  weapons in 484 families (the AK-47's, `combat_arms:pv_ar_ak47_sh`, holds
  34).
- Step 2 done: `minigame::gun_rules` (the cvars, `apply` with the
  precedence, `Pool`, `GunRulesNote`, `LadderRungs`), `auto::draw_pool`
  and `auto::ladder_rules` (families one candidate, Shorter / Repeat /
  Borrow, the allowed group), `ugg_rungs` with the curve's `[rungs]`
  bounds; used by the Ultimate Gun Game and every `ladder = "auto"`.
  Presets: `gun_rules::Preset` (TOML, built-ins in
  chimera/sim/data/guns_presets.toml, the host's in `cfg/guns/`),
  `guns_preset [name]` and `guns_preset_save <name>` on any server.
  Tests: gun_rules and auto unit tests, apps/ultimate_gun_game/tests/it/
  gun_rules.rs.
- Step 4, first part: the Host page's summary pane shows the Guns line
  (`gun_rules::summary`: the preset or what differs, the rungs, the
  fallback note).
- Next: step 3, the dialog (the old card picker still opens from the Guns
  field and sends `mp_guns_off`, read as bans, so nothing regressed), then
  the Guns field as the preset dropdown (step 4). The dialog's pieces
  already in data and code: the presets (`Preset::builtin`, `saved_in`,
  `to_text`/`from_text` for Copy as text and Paste), the families
  (`family_of`), effective classes, stage counts (`power::band_counts`),
  the pool per stage (`draw_pool` then `powers`).
