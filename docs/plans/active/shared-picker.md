# Plan: one shared picker for maps and guns

Status: in progress (2026-11-20). Design approved by the user (below).
Related: docs/plans/active/ugg-ui.md ("Signal revision 8" stage 4, the
Maps picker; M11, the sandbox's screens).

## The design (approved)

- **One component** (`chimera_ui::picker`). Items grouped by game in
  collapsible sections; a section's header has a chevron and the game's
  name, a click on it folds or opens it. A search box and filter chips
  (weapon type, game, map size and the like) over the list.
- **Two modes.**
  - *Multi-select* (host settings): each item and each game section has
    an on / off / some box. Only the box toggles (with a little hit
    slop); the rest of a header folds. The rule the Maps dialog follows
    since f51426ab3.
  - *Single pick* (Sandbox): a click on a gun equips it or puts it on a
    quick-bar slot, a click on a map changes the range's map. No boxes.
- **One component, two frames.** The same picker in a dialog (host
  settings) and filling the screen (Sandbox). The container gives the
  width; the picker picks its columns and card size from it (fewer
  columns, smaller cards in a dialog).
- **Data-driven.** Lists come from the existing catalogs (maps: the
  minigame's pool through `MapCatalogue`; guns: the weapon catalog),
  each item an id, a name, an icon, a game and tags. A new picker
  (characters) is new data, not new UI.

## Shape in code

- `chimera_ui::picker` (the kit; no game, no simulation):
  - **Data**: `PickerGame { id, name, groups }`, `PickerGroup { name,
    items }`, `PickerItem { id, name, icon, parent, tags }` (`parent`: an
    item listed under another one step in, e.g. an arena slice under its
    map). Facets for the chips come from the items' tags.
  - **State** (`PickerState`): the mode, the kept set and the working
    copy (multi: the ids off, the stored form with `+id` for a child kept
    on under a parent off), the current pick (single), the folded games
    and groups, the cursor, the search text and the ticked chips.
  - **Rows**: games, groups, items after search and chips; the tri-state
    box of any row; toggle, fold, ←/→ as the Maps dialog had them.
  - **Build**: `picker::spawn(..)` lays out the search and chips (when the
    data has them) and the kit's recycling list: rows (`PickerLook::
    List`) or cards (`Cards`: columns and card size from the frame's
    width and `[layout.picker]`). Slots are bound by one kit system for
    every open picker (`Pickers` by list key).
  - **Dialog frame**: `picker::dialog(..)` (scrim, plate, title, counts,
    legend, keys, actions), what the Maps dialog was.
- Numbers in the theme data (`[layout.picker]` in the kit's base theme,
  chimera/ui/data/shell.toml, which apps/ultimate_gun_game/data/theme.toml
  lays over: every app and test theme has them), not in code.
- The gallery: `ui_gallery picker` (both modes, both frames, made-up
  data).

## Steps

1. [x] The component in `chimera_ui::picker`, a gallery page; the Maps
   dialog moved onto it with no visible change (before/after pixel
   compare at 1920x1080 and 1280x720). Tests: the kit's picker logic
   (`chimera/ui/tests/it/picker.rs`), the Maps dialog's existing tests.
2. [x] Host settings' guns. Finding (2026-11-20): Host had no gun
   selector (only Maps); this step adds one: the Host page's Guns field
   (main_menu.toml `guns = true`) opens `guns_picker` (multi, cards, the
   class chips) over `LadderCatalogue` (`auto::report` and
   `auto::drawable_all`: the guns a drawn ladder may deal), and Start
   sends `mp_guns_off` (`chimera_sim::minigame::auto::GunsOff`: drawn
   ladders leave them out unless none would be left).
3. [ ] The Sandbox's Change gun (B) on the picker (single, full screen,
   cards), the quick bar kept (picking the gun in hand does nothing:
   acb27eb50, `rung_command::select_held`).
4. [ ] A Sandbox map selector on the picker (single), changing map
   through the sandbox's entry path (f2132a134): the map is changed by
   submitting `sandbox <map>` (apps/ultimate_gun_game/src/sandbox.rs, the
   console command the main menu's Sandbox entry runs), so the entry's
   no-lobby, no-end-screen switch applies.

Notes for step 3: Change gun (chimera/client/src/client/sandbox/
change_gun.rs, 1,467 lines) has more than the picker does today: a
games column with per-game download state and facets (TF2's classes), a
tag popover of eight groups with its own search, the detail pane (model,
stats, hits to kill, variants list, Play as), the pinned bar with drag
and drop, tiles with download bars, the grid keys (`\\` folds, I, R,
Ctrl+1-6). The picker needs, before it can carry it: typing in its search
field (the kit's text field state, as `GunSearch`), a per-card foot
slot (download bar, slot number), family tiles (a count beside the
name), the vlist grid keys (the picker's cursor through `VListState::
nav`), and a side slot for the detail pane. Plan: add those to
`chimera_ui::picker` as data/hooks, then move the grid and the search
onto it, keeping the games column, popover, detail and pin bar as the
screen's own around it.

Each step: UI tests (as apps/party_kit/tests/it/signal_pages.rs), offscreen
captures at 1920x1080 and 1280x720, a commit, post-push tests.

## Log

- 2026-11-20: plan written.
- 2026-11-20, step 1: `chimera_ui::picker` (data, state, rows, both modes,
  both looks, `dialog`, `bind_slots` in the kit's plugin), the numbers in
  shell.toml's `[layout.picker]`; `ui_gallery picker`; the Maps dialog on
  it (`maps_picker` is now the maps' data and the dialog's words). Before
  and after at 1920x1080 and 1280x720: the dialog pixel-identical (the
  only differing pixels were the bar's avatar, whose colour is drawn per
  run). Tests: chimera/ui/tests/it/picker.rs; party_kit's Maps tests.
- 2026-11-20, step 2: the Host page's Guns field and dialog on the picker
  (cards, class chips), `mp_guns_off` on the server (`auto::GunsOff`,
  applied in `auto::drawable`; `drawable_all` lists before it). Tests:
  party_kit `the_guns_picker_sets_the_guns_off`, chimera_sim
  `the_hosts_guns_off_are_left_out_unless_none_would_be_left`. Captures:
  `ui_scene play guns` at 1920x1080 and 1280x720 (2,090 guns listed with
  the installs here; the recycling grid keeps it cheap).
- Next session: step 3 (the notes above), then step 4.
