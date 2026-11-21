# Options with Games

The Options screen (party_kit `settings`, the Signal look) rebuilt from
the approved mockup "Options with Games" and the user's review of it
(2026-11-21): tabs General · Video · Audio · Controls · Games, shared
actions bound once under Controls, and a Games tab where each game's own
keys, gameplay and look live, declared by the game in data.

## Decisions (user review, 2026-11-21)

1. Tabs and nav: grey text goes white when hovered or selected, with
   no underline (the mockup's back nav), kit-wide.
2. Toggles: the mockup's bigger, wider ones, with the kit's animation.
3. A per-setting reset key in the row; where a value comes from
   ("Same as General", "TF2 default") goes in the help pane, not the row.
4. The key-capture chip: blinking "Press a key", Esc cancels.
5. A generic count **badge** in the kit, used on tabs and anywhere.
6. Every control in one column of one width; a choice set that fits
   shows as a segmented control, otherwise a dropdown.
7. One marker per row: the row alone gets the lighter fill and the blue
   left bar; the control inside gets no focus line of its own.
8. Settings that are ours (not in the original game) carry a small
   Chimera mark, and the help pane says so; each option declares its
   origin in data (`origin = "game" | "chimera"`).
9. The game rail at the left if it fits; progressive disclosure if not.
10. No "Keys in hand" card for now. No blocking animations (onboarding
    review): everything is usable at once.

## Steps

1. **Kit** (`chimera_ui`, sizes in the theme's `[layout.kit]`):
   `badge`, the big toggle (its slide kept), the key-capture chip
   (blinking light, Esc), `setting_row` (label, Chimera mark, the control
   column, the reset key), tabs grey to white without the strip, the
   segmented-or-dropdown rule (`choice`), one marker per row. Gallery
   page `ui_gallery kit` (`ui_scene kit`), tests.
2. **Layout**: options.toml's tabs become General (the Party rows, the
   Network row), Video, Audio, Controls (the mouse and the shared binds),
   Games. The old look's pages follow the data.
3. **Games**: each game crate ships `data/options.toml` (its actions:
   label, group, command, notes; its options: a cvar of its own or an
   override of a shared one, page, origin, help), named by its
   `GameBundle::options`. The Games tab: the rail (badges = the game's
   changes), sub-tabs Keys / Gameplay / Look and sound (badges), rows
   with reset. TF2's Auto reload and Auto switch when empty move to its
   Gameplay page (same cvars). Per-game overrides of shared cvars: the
   console's game cvar layers (`setgame`, `resetgame`, saved in the
   config like `bindgame`), applied while that game is the player's.
   Binds stay in the existing game layers: no config migration needed,
   nobody loses binds (a test loads an old config).
4. **Screenshots**: `ui_scene settings <tab>` (and `games:<game>:<page>`)
   at 1920x1080 and 1280x720; OBSERVABILITY says how.

## Progress

- 2026-11-21: plan written.
- 2026-11-21: steps 1-3 built. Kit: `chimera_ui::form` (badge, big
  toggle, setting row with the reset key and the Chimera mark, `choice`,
  the blinking capture chip in `key_field`), tabs grey to white with
  `tabs_badged`, one marker a row (`row`'s hover bar), icons `reset` and
  `chimera`, `ui_gallery form`. Console: game cvar layers (`setgame`,
  `resetgame`, `cvar_base`; `apply_game_cvars` in `run_queue`, the game
  played from `follow_scope`). Options: General / Video / Audio /
  Controls / Games; each game's `data/player_options.toml`
  (`GameBundle::player_options`, party_kit `game_options`): CS:S, CA,
  HL2:DM, Halo (its HUD scaling moved from Video), TF2 (its actions, the
  planned ones too; Auto reload and Auto switch when empty moved from
  Party, same cvars); every game a sensitivity and a field-of-view
  override. Binds stay in the game layers (no migration needed).
- Next: screenshots at 1080 and 720 (step 4) in the scratchpad; TF2's
  alt-fire actions share `+attack2` (per-gun keys need the input to know
  the gun's action); taunt, inspect, call for Medic and special attack
  are planned rows (no game code yet).
