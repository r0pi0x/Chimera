# Gun display names

Started 2026-11-13. Question (the user, after an Ultimate Gun Game
session): guns show very long names (Combat Arms' skins and editions,
some with underscores). Is that how the original games show them? This
plan answers that, measures every gun a drawn ladder can deal, and
proposes short names for the places with little room. Steps 1 to 5 are
done (2026-11-13, below); step 6 waits for the user. Parent plan:
[ultimate-gun-game.md](ultimate-gun-game.md).

Data: [gun-display-names.csv](gun-display-names.csv), one row per gun
a drawn ladder could deal on 2026-11-13 (`auto::report` then
`auto::drawable` on the party's simulation with the Combat Arms install's
data; 2,074 guns and grenades: Combat Arms 2,044, CS:S 25, HL2:DM 5).
Columns: `game`, `weapon_id`, `shown_now` (what our UI shows today),
`game_display_name` (the game's own display string; empty when it has
none), `length` (of `shown_now`, characters), `underscore`,
`skin_or_edition_word` (the split below found a skin before the model or
a variant after it), `over_16`, `over_24`, `split_short` and
`split_variant` (the automatic split below, Combat Arms only) and `fix`
(`ok`, `split` or `override`, below). Names and ids only. Six rows whose
name holds a word the push trace check refuses show "(withheld: see id)",
and one of those ids is written with `*` for the same reason.

## Answer: we already show each game's own display string

| Game | Where the name comes from | What the original shows |
|---|---|---|
| Combat Arms | the shop item file's `WeaponItem` record whose `ItemID` is the weapon's record index; its `ItemName` string id resolved in the install's language table (`ui::install::weapon_names`, filling `ItemNames`) | the same: the weapon panel's name is the item table's name, not a weapon-data field (specs/combat_arms/hud_crosshair.md D4, Corrections 16; ui_system.md R1) |
| CS:S | the weapon data's `hud.name` token (`#Cstrike_WPNHUD_*`) looked up in the install's `resource/cstrike_english.txt` (`weapons::display_names`, CS:S's `WeaponNames`) | the same token, the script's `printname` (specs/cs_source/weapons.md 1) |
| HL2:DM | the `hud.name` token (`#HL2_*`) in `resource/hl2mp_english.txt`, its line break as a space (`weapons::display_names`) | the same token, drawn on two lines (specs/hl2dm/hud.md 5.1) |

So **no gun shows an internal record name where the game has a display
string**: all 2,006 Combat Arms guns with a shop item and all 30 CS:S
and HL2:DM guns show the game's own string. The long and underscored
names are the games' own: Combat Arms' string table really holds
`Glock23_Champion`, `HG2_Auto_Turret`, `UnderSea_AK-47` and
`Sun God Ra AR-15 Double Barrel (Mythic)`, and its HUD fits them in a
250-unit right-aligned box. Using display strings fixes **0** more guns.

The other 38 Combat Arms guns have **no** shop item, so no display
string; we show the id's last part in capitals (`AUTO TURRET INFECTEDS
01`, `HQRAID BOSS MINIGUN`, `DEVIL SOUL P901`). They are NPC, boss,
turret and unreleased records the original never hands a player
(`*_npc`, `hqraid*`, `abnormal_*`, `fort_missle_*`, `auto-turret_infecteds_*`,
`rpg-7_robot`, `devil_soul_p90*`, `acr_contempt*`, `g36e_ugly`,
`glock_23_org`). Whether they belong on a ladder at all is a separate
question (a rule "no shop item, no rung" in `ladder_exclusions.toml`'s
style would drop them); until then their record `Name` with `_` as
spaces (`Devil Soul P901`) reads better than the id in capitals.

Where the shell names a gun (all through
`minigame_scoreboard::gun_name`: the app's `GunNames`, which
party_kit fills from CS:S's `WeaponNames` and Combat Arms' `ItemNames`,
the latter also holding CS:S's and HL2:DM's names; then the data's
plain `hud.name`; then the id's last part in capitals):

- the minigame panel's `{weapon}` (rung and next gun; CS:S's
  `WeaponNames` first),
- the Tab scoreboard's gun column (clipped to its fixed width),
- the shell's kill feed when a gun has no icon,
- the callouts (rung up, lead, "Level down: <gun>"),
- the match-end screen's rows,
- CS:S's HUD notices.

Combat Arms' own HUD names its weapon panel and death screen through
`ui::hud::weapon_name` (`ItemNames`, then `hud.name`, then the id), and
another game's gun in its kill feed through `gun_name` cut to 10
characters. The earlier example "Auto-Turret_InfectedS_02_Topgun" is a
turret's own gun; those no longer reach a ladder (their copies name no
class, `game::data::merge_weapon_docs`).

## Counts

| | Guns |
|---|---|
| Drawable guns and grenades | 2,074 |
| Fine as shown (16 characters or fewer, no `_`) | 1,146 |
| Longer than 16 or with `_` | 928 (907 over 16, 245 over 24, 26 with `_`) |
| fixed by the game's display string (not already used) | 0 |
| fixed by the automatic split (short name 16 or fewer) | 561 (151 of them over 24 today) |
| needing an override (or the layout net) | 367: Combat Arms 358, CS:S 7, HL2:DM 2; 94 over 24, 273 at 17-24 |

Most of the 273 overrides at 17 to 24 characters fit the wider places
as they are (the panel, the callouts, the match-end screen) and only
need the safety net in the narrow ones; the 94 over 24 are where
overrides pay.

## Proposal

### 1. Source of truth

The game's display string first, as today: Combat Arms' item table,
CS:S's and HL2:DM's localisation tokens. It is the **full name**, shown
where there is room and never changed by the steps below. A gun with no
display string uses its record name (`_` as spaces) before the id. Both
short and full names are per gun id, so a skin keeps its own full name.

### 2. Automatic base-plus-variant split (short name)

For Combat Arms only (CS:S's 25 and HL2:DM's 5 names are few and short:
overrides). Applied to the full name, in order:

1. **Clean.** `_` to a space; full-width brackets `（）` to `( )`; curly
   apostrophes to `'`; a lone ` - ` dropped; spaces collapsed; ends
   trimmed.
2. **Variant tail.** From the end, while more than one word is left,
   move to the variant: a bracketed group (`(Mythic)`, `(Event)`,
   `(limit)`), a level (`lv.3`, `LV3`, `Lv.2(limit)`, `Level 2`), an
   ordinal (`5th`), a bare number up to 4 digits (`2020`, `01`),
   `Mark II` / `Mk III`, or an edition word: `SE`, `CORE`, `CORE+`,
   `Edition`, `Event`, `Limited`, `limit`, `Champion(s)`, `PCM`,
   `Anniversary`, `MOD`, `Custom`, `Camo`, `Gold`, `Gold-Plated`,
   `Silver`, `Chrome`, `Common`, `Uncommon`, `Rare`, `Epic`, `Legendary`,
   `Mythic`, `Premium`, `Elite`, `Deluxe`, `Special`, `NPC`, `Org`,
   `TAU`, `SPW`, `ECO`, `Plus`, `GP`, `Black`, `White`, `Ugly`.
3. **Model.** The first word that is a model: letters and digits with
   at least one digit (`AK-47`, `M4A1`, `SG50`, `P220`, `DSR-1`, `.38`
   not alone), not an ordinal; or a known model word (`Kriss`, `USP`,
   `Thompson`, `Makarov`, `Minigun`, `UZI`, `FAMAS`, `Saiga`, `SCAR-L`,
   `Dragunov`, `Anaconda`, `Cobra`, `Skorpion`, `ParaFAL`, `Jackhammer`,
   `ACR`, `RPK`, `LAW`, `AUG`, `UMP`, `TMP`, `SVD`, ...; the list lives
   in data). A `Dual` / `Twin` right before it stays with it.
4. **Skin.** The words before the model (`Sun God Ra`, `Kagutsuchi's
   Bloom`, `Poseidon's Wrath`) are the skin: kept in the full name, left
   out of the short one.
5. **Condense.** In the short name: `Double Barrel(ed)` to `DB`,
   `Double Shot` to `DS`, `Machine Gun` / `Machinegun` to `MG`,
   `Tactical` to `Tac.`.
6. **Accept** the model and what follows it (`AR-15 DB`, `USP Tac.`,
   `M4A1 Frostbite`) as the short name when it is 1 to 16 characters;
   else the gun needs an override (or the safety net).

Measured on the CSV: 561 of the 928 long names get a short name this
way. Sample results: `Kagutsuchi's Bloom SCAR-L lv.1(limit)` to `SCAR-L`,
`Field Camo USP Tactical SE CORE` to `USP Tac.`, `Glock23_Champion` to
`Glock23`, `M4A1 Frostbite lv.2(limit)` to `M4A1 Frostbite`. A refinement
worth trying: keep one skin word when it still fits (`Bloom SCAR-L`,
`Ivory DSR-1`): 329 of the 561 would.

### 3. Overrides: `display_names.toml`, per game

Data, not code, in the style of `ladder_exclusions.toml`: one file per
game crate (`games/<game>/data/display_names.toml`), read at start up
and registered with the minigame layer (as `add_draw_exclusions` does),
winning over the split. Each entry names the ids, an optional `short`
and an optional `full` (a `full` replaces the game's string only where
that string is broken, e.g. a stray ` - ` or mojibake):

```toml
# Combat Arms guns whose automatic short name is wrong or too long.
[[name]]
weapons = ["combat_arms:double_barreled_ar-15_godra_common", "combat_arms:double_barreled_ar-15_godra_rare"]
short = "Ra AR-15 DB"

[[name]]
weapons = ["hl2dm:weapon_ar2"]    # in hl2dm's own file
short = "PULSE-RIFLE"
```

A console command (`gun_names [game]`) prints id, full, short and where
each came from, and a heavy test per game keeps every drawable gun's
short name at 16 characters or fewer, so a new install's guns show up as
failures to add.

### 4. Where short and full names are used

| Place | Room | Name |
|---|---|---|
| Minigame panel: rung, next gun | wide, one line | full; short when over the line's width |
| Callouts (rung up, lead, level down) | wide, large font | short |
| Tab scoreboard gun column | fixed narrow column | short |
| Shell kill feed (no icon) | narrow | short |
| Combat Arms kill feed, another game's gun (no icon) | 10 characters | short, then the safety net |
| Combat Arms weapon panel, death screen | the game's 250-unit box | full (the original's) |
| CS:S HUD notices | wide | short |
| Match-end screen rows | medium | short; full in the winner's line |
| Console, logs (`ladder`, `ladder_skipped`) | any | full and id |

`GunNames` would carry both (`full`, `short`); `gun_name` keeps its
meaning (full) and a `gun_short_name` beside it serves the narrow places.

### 5. Layout safety net

Where a name still doesn't fit its box (measured with the font, not
counted): first condense (step 2.5's abbreviations, then drop the
variant tail, then the skin), then truncate at a word boundary with
`…`, as Combat Arms' rung line already does (`rungs::label`,
`LABEL_MAX`). Never wrap, never overlap the next column.

## The 30 worst offenders: proposed short names

The longest names, one row per family (a family's levels or rarities
share the short name; the level stays in the full name).

| Weapon id | Shown now (chars) | Short |
|---|---|---|
| combat_arms:m419_combat_april | M419 Combat CQB SE CAMO Black MOD Mark II (41) | M419 CQB |
| combat_arms:double_barreled_ar-15_godra_* (4) | Sun God Ra AR-15 Double Barrel (Mythic) (39) | Ra AR-15 DB |
| combat_arms:malevolent_beauty_ar-15_double_barrel | Malevolent Beauty AR-15 Double Barreled (39) | Malevolent AR-15 |
| combat_arms:roaring_white_dragon_ar-15 | Roaring White Dragon AR-15 Double Shot (38) | Dragon AR-15 DS |
| hl2dm:weapon_ar2 | OVERWATCH STANDARD ISSUE (PULSE-RIFLE) (38) | PULSE-RIFLE |
| combat_arms:kagutsuchi___s_bloom_scar-l_lv.*_limit_ (5) | Kagutsuchi's Bloom SCAR-L lv.1(limit) (37) | Bloom SCAR-L |
| combat_arms:double_barreled_ar-15_snowreaper | Snow Camo AR-15 Double Barrel CORE+ (35) | Snow AR-15 DB |
| combat_arms:p220_se_variantsdragon | P220 (SE) - Majestic Dragon Loong (35) | Dragon P220 |
| combat_arms:visionary_ak-47_gold_event | Visionary AK-47 Gold-Plated (Event) (35) | Gold AK-47 |
| combat_arms:l96a1_bm_cac_5th | L96A1-Black Magnum 5th Anniversary (34) | L96A1 BM 5th |
| combat_arms:ghost_hunter_parafal_frankenstein | Ghost Hunter ParaFAL Frankenstein (33) | Ghost ParaFAL |
| combat_arms:kingofseasg50_lv*___limit___ (5) | Poseidon's Wrath SG50 LV.1 (Limit) (33) | Poseidon SG50 |
| combat_arms:stadium_legends_ak-47_lv.*_limit_ (5) | Stadium Legends AK-47 lv.1(limit) (33) | Stadium AK-47 |
| combat_arms:valofe_limited_kriss_specops_epic | VALOFE Limited Kriss SpecOps Epic (33) | Kriss SpecOps |
| combat_arms:visionary_m4a1_desert_aim_pcm | Visionary M4A1 Desert Warrior PCM (33) | Desert M4A1 |
| combat_arms:brazil_dual-kriss | Brazilian Dual Tropical Shooters (32) | Dual Kriss BR |
| combat_arms:harlequin_s_dual_pistol | Harlequin's Dual Pistol Red/Blue (32) | Harlequin Duals |
| combat_arms:mechanical_scannning_usp | Mechanical Scanning USP Tactical (32) | Scanning USP |
| combat_arms:an-94_winter_lv.*___limit___ (5) | Obsidian Gild AN-94 Lv.1 (limit) (31) | Obsidian AN-94 |
| combat_arms:blacklist_conviction_five-seven | Blacklist Conviction Five-Seven (31) | Blacklist 5-7 |
| combat_arms:double_barreled_ar-15_eclipse | Eclipsed Ra AR-15 Double Barrel (31) | Eclipse AR-15 DB |
| combat_arms:ivory_trigger_dsr-1_lv.*___limit___ (5) | Ivory Trigger DSR-1 lv.1(limit) (31) | Ivory DSR-1 |
| combat_arms:lighting_core_cobra_.38_special | Lighting Core Cobra .38 Special (31) | Cobra .38 Core |
| combat_arms:sv-98_winter_lv.*___limit___ (5) | Obsidian Gild SV-98 Lv.1 (limit) (31) | Obsidian SV-98 |
| combat_arms:thompson_labberpower | Mobster Tommy Gun The Godfather (31) | Godfather Tommy |
| combat_arms:usp_tactical_wild | Field Camo USP Tactical SE CORE (31) | Field Camo USP |
| combat_arms:arx-160_futuristic_lv.*___limit___ (5) | Red Dominion ARX60 Lv.1 (limit) (30) | Dominion ARX60 |
| combat_arms:bar | Browning Automatic Rifle (BAR) (30) | BAR |
| combat_arms:famas_prototype-01_lv.*_limit_ (5) | FAMAS Prototype-01 lv.1(limit) (30) | FAMAS Proto-01 |
| combat_arms:limit_judeworrior_m4a1_lv* | limit Imperial Spirit M4A1 lv1 (30) | Imperial M4A1 |

CS:S's seven for its override file (its own strings' words kept, no
real-world names): `weapon_xm1014` LEONE YG1265 AUTO SHOTGUN (LEONE
YG1265), `weapon_tmp` SCHMIDT MACHINE PISTOL (SCHMIDT MP), `weapon_m3`
LEONE 12 GAUGE SUPER (LEONE 12 GAUGE), `weapon_m4a1` MAVERICK M4A1
CARBINE (MAVERICK M4A1), `weapon_mp5navy` K&M SUB-MACHINE GUN (K&M SMG),
`weapon_awp` MAGNUM SNIPER RIFLE (MAGNUM SNIPER), `weapon_sg550` KRIEG
550 COMMANDO (KRIEG 550). HL2:DM's two: `weapon_smg1` SMG (SUBMACHINE
GUN) (SMG) and `weapon_ar2` (above).

## Steps

1. Done. `GunNames` carries `full` and `short`
   (`chimera_client::client::minigame_scoreboard`; `fill_short_names`
   fills the short ones from the games' rules); `gun_name` stays the full
   name, `gun_short_name` serves the callouts, the Tab scoreboard's gun
   column, the shell's kill feed, Combat Arms' kill feed for other games'
   guns (then fitted to its 10 characters), CS:S's HUD notices and the
   match-end rows (the winner's row in full). The minigame panel names the
   full one, and the short one on a status line wider than `LINE_MAX`.
   Combat Arms' weapon panel and death screen keep the original's name.
2. Done. `chimera_sim::weapon::names::split` / `auto_short`, its word
   lists in chimera/sim/data/gun_names.toml. One addition to section 2:
   a result over 16 loses words from the end down to the model (or, with
   no model, from the front), so most of the 358 "override" rows need no
   entry; the ones that then read badly (a run-together name, a first
   word that looks like a model) have one. On the 2026-11-13 install:
   2,044 Combat Arms guns, 120 overridden (by id patterns), 799 split,
   1,125 as they are.
3. Done. `games/<game>/data/display_names.toml` (`game`, `split`,
   `[[name]]` with `weapons`, a `*` pattern allowed, `short`, `full`),
   registered by each game's guns plugin (`add_display_names`, as
   `add_draw_exclusions`): the 30 above plus 10 Combat Arms run-together
   or misread names, CS:S's 7, HL2:DM's 2.
4. Done. `client::overlay::fit` (measured with the font, `UiFonts::width`;
   condense, drop the variant tail, drop the skin, then cut at a word with
   `…`): the scoreboard's and match-end screen's gun cells, the kill
   feed's gun name (`NAME_MAX`), the minigame panel's long status lines.
5. Done. `gun_names [game]` (id, full, short and where it came from) and
   `heavy::display_names` in combat_arms, cs_source and hl2dm (every
   drawable gun's short name at 16 or fewer, via
   `minigame::auto::drawable_short_names`).
6. Decide (the user): guns with no shop item (the 38 above) off the
   ladder?
