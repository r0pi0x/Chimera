# Combat Arms: player characters

Source basis: reference material the user pointed the spec session to on the game's player characters: the shared character-model attribute manager and the per-player model selection record, the player object's model building, attachments, death and body code on the server, the client's character effect (aim twisting, hit-sphere display), the shared third-person animation driver and its animation-set tables, the client and server animation updaters, the body object and its states, and the client hit-sphere test. Data was read from a US-region attribute data set (the model, attachment, mark and weapon attribute files) and, where it differs, compared with a BR-region set. Model numbers were measured with a throwaway reader written from `models_ltb.md` and `animation.md` on the character archives of the US reference client (build 1501.04), whose character archives are not keyed. The Steam install on the Windows PC (manifest build id 25595979) keys its character archives (`V2_RESOURCES_KEY`), so those numbers are marked "install" in Test cases and must be re-measured there. Section 12 (who may hold which weapon) was read on the Steam install, build 26236938: weapon, item, model, AI and effect files, all mission files and every world's objects, with the client's item, equipment-slot and room-option code. A follow-up session (Open questions 13–22) added the server's reload, weapon-fire and hit-box code, the client's shot filter, emote and body effects, and measured the female numbers and third-person weapon sockets on an older extracted client release (its male files reproduce the US 1501.04 numbers). Section 13 (NPC and boss bodies, 2026-10-31) was read on the Steam install, build 26236938: the model, attachment, weapon, effect and AI attribute files, every world's objects, and every NPC body model and animation library in the character archives (measured with a throwaway reader written from `models_ltb.md` and `animation.md`), with the reference code that builds AI characters, their animation sets and trackers, and the pattern boss.
Status: draft

Sections: Summary; Units and conventions; Constants; Behaviour (1 character records and model choice, 2 assembling the model: pieces, skins, head and face, attachments, 3 skeleton and sockets, 4 trackers and layering, 5 deciding the animation state, 6 building the animation names, 7 starting, restarting, syncing, rates and blend times, 8 aim pitch and twist, 9 hit reactions, emotes, 10 death and bodies, 11 hit-sphere placement, 12 who may hold which weapon, 13 NPC and boss bodies: `AIModel#` records, the 947 world NPCs, body families, the NPC animation driver, hit tables 2–18 and sizes, powered suits and the HQ raid boss, a player-driven NPC body); Per-tick order; Edge cases; Quirks; Corrections (2026-10-22); Corrections (2026-10-30); Test cases; Open questions.

---

## Summary

- A player character is one skinned **body model** (with the shared animation library of its sex as child model), plus a **head piece** and optionally a **face piece** drawn as additional models on the body's skeleton, plus rigid **socket attachments**: the hand-held weapon (`RightHand`), a second gun for dual weapons (`LeftHand`), goggles and masks (`Eyes`), backpacks and flags (`1W_N`). Carried but unselected weapons are not shown.
- Which body is used comes from the player's own loadout (model, face, head piece, camouflage index, cosmetics), not from the team. The face can redirect a male body record to its female twin. Team only changes one texture slot (the "friendly sign").
- The hit spheres, death animations and ragdoll-free body are keyed on the model record's **skeleton** index (0 male, 1 female in the stock player set). Each sphere sits on a node of that name, or, if the model has no such node, on a socket of that name; missing ones are skipped. On the stock bodies the two 7.0 "shoulder pad" spheres of the male table never exist.
- Third-person animation runs on four trackers: the main tracker (the body's own bind-pose animation, never changed while alive), an upper-body tracker, a lower-body tracker and a crouch tracker whose weight is the crouch fraction. Names are built from a weapon-class code, a stance letter, an action and a direction (`URFRAmS`, `LRF`, `LC`...). A name the library lacks is silently skipped and the previous animation keeps playing.
- Playback rate is 1 for the legs whatever the ground speed; the upper body is rate 1 too in the stock data. Aim pitch and the upper-body twist are bone rotations applied after animation, spread over torso, neck, head and shoulders, with pitch clamped to ±45°.
- Death plays one non-looping animation picked from the last hit sphere's location, the shot direction, the damage type and a coin flip; the body is a separate object that stays until its owner respawns and is removed 5 s after that in the common modes. There is no ragdoll.

## Units and conventions

- Distance: LithTech world units ("u"). Up is +Y, the frame is left-handed: +X right, +Z forward (`models_ltb.md`). All model-space positions below are relative to the object position, which is the centre of the movement hull; the feet of the stock characters are at y = −50 when standing (the animations' root translation, `animation.md`).
- Angles: radians in the game; degrees in this spec where stated. Positive pitch looks down, positive yaw turns right (forward +Z towards +X). "Rotation about axis a by θ" means the usual right-hand-rule formula applied to column vectors in these coordinates; with it, +θ about +X tips +Y towards +Z (a forward bow) and +θ about +Y turns +Z towards +X.
- Time: integer milliseconds for animations, seconds for body lifetimes.
- Quaternions are (x, y, z, w). Node and socket offsets are in the parent node's space unless "model space" is said.
- "Male library" is `CHARS_M_BODY\ST_M_CHILD.LTB`, "female library" `CHARS_M_BODY\ST_W_CHILD.LTB` (the female library lives in the male body archive).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| stand half-extents (fallback) | (15, 50, 15) | u | Used if the model lacks `LST` or `LC` (`models_ltb.md`) |
| crouch half-extents (fallback) | (15, 40, 15) | u | Same |
| twist limit | ±45 (`PlayerTwistYawMax` 90 × 0.5, `PlayerTwistYawMin` −90 × 0.5) | ° | Upper-body twist range; also the third-person aim pitch clamp |
| aim byte scale | 127 / (π/2) | steps per rad | Pitch and twist sent in one signed byte each on the legacy path |
| torso yaw share (`LowerYawRatio`) | 7/17 | | Fraction of the twist applied at `Torso` |
| upper-torso yaw share (`UpperYawRatio`) | 10/17 | | Fraction applied at `Upper_torso` |
| pelvis yaw share (`PelvisYawRatio`) | 0 | | At `Right_pelvis` |
| pitch shares, looking up | Torso 0.15, Upper_torso 0.15, Neck 0.25, shoulders 0.45, Head 0.50 | | `LowerPitchUpRatio` ... `HeadPitchUpRatio` |
| pitch shares, looking down | Torso 0.35, Upper_torso 0.45, Neck 0.10, shoulders 0.10, Head 0.35 | | `LowerPitchDownRatio` ... `HeadPitchDownRatio` |
| hit-reaction pitch kick | 0.1 rad × a 1–5–0 counter, 0.5 rad peak | rad | Remote characters only, see 9 |
| body spawn lift | 0.1 | u | Body created this far above the dead player's (floor-snapped) position |
| body removal delay | 5 / 500 / 1000 | s | After the fade state starts and the death animation is done: most modes / Combat, clan default and ESL modes / Hunger Game mode |
| body lifetime console variable | `BodyLifetime` 30 | s | Only for bodies given a positive lifetime; players' bodies have none (−1) |
| left-hand weapon offsets | per weapon | u, ° | `LeftHHModelPosOffset`, `LeftHHModelRotOffset`, `LeftHHScale` |
| third-person frame lengths | `HHAniFrame_0` … `HHAniFrame_9`, `HHAniFrame_5_1` | ms | Keys of the weapon's own record in the weapon file; per posture index (7); default 0 = no rate change. Absent from both stock data sets read |
| server reload fallback | 500 | ms | Server reload timer when no third-person reload animation is found |
| server reload check time | 0.8 | — | Server configuration `ReloadCheckTime` (section `TimeWeight`); the server's reload rate is min(r, 2.0) × (2 − 0.8) = 1.2 × min(r, 2.0) |
| server frame | 20 Hz (shipped configuration) | — | The server advances every model's animation trackers once per server frame by that frame's time (`game_modes.md`) |
| client driver step | 15 | ms | The client runs the animation driver for its own character once per movement tick (`movement.md`), not per rendered frame |
| hit-sphere debug console variables | `ShowHitSphere`, `ShowLocalHitSphere` | | Draw spheres on remote / own character (non-final builds) |

## Behaviour

### 1. Character records and model choice

**Where the data lives (by role).**

| Data | File in the attribute archive | Records |
|---|---|---|
| Bodies, head pieces, faces, first-person hands, body types, hit skeletons, infection overrides | `MODELBUTES.TXT` | `Model#`, `Hair#`, `Face#`, `Hand#`, `FigureType#`, `Skeleton#`, `Overidable_Model#` |
| Goggles and masks, backpacks, flags | `ATTACHMENTS.TXT` | `Goggle#`, `Backpack#`, `Flag#` |
| Team mark textures | `MARKS.TXT` | `[TeamMark]` `TeamName#`, `RenderStyle#` |
| Third-person weapon model and animation class | `WEAPONS.TXT` | `HHModel`, `HHSkin#`, `HHRenderStyle#`, `LeftHHModel`, `AniType`, `LowerAniType`, `HHAniFrame_#` |
| Armour per gear id | `MODELBUTES.TXT` / gear records | `GearID` on models, head pieces and goggles (`weapon_firing.md`, Armour) |

A `Model#` record holds: `Name`, `ModelFile` (body), `RenderStyle0..`, `Skeleton` (hit table index), `FigureType` (body type record: sex, sounds, sprint and stamina stats; `attribute_data.md`), `GearID`, `MaxArmor`, `MovePenalty`, `DashSpeed`, `Mass`, `UpperSkin0..6`, `LowerSkin0..6`, `HandsSkin0..`, `HandIndex`, `FaceRelationModel0..`, `InfectedOveridable`, `HostInfectedOveridable`, screen-only animation and light names. A record with at least one `UpperSkin` is a "player body" (two-texture vest/legs scheme); other records use a plain `Skin#` list.

**The selection.** Each player's client sends, with its character info, a record of small integers: figure type, model id, head piece id, face id, clan mark, team mark, backpack id, goggle id, "default goggle" flag, camouflage index, infection-override index, clan emblem and clan background texture names, cosmetic character, helmet, goggle, vest and backpack ids, and colour indices for model, helmet, goggle and backpack. The server validates it (an unknown model id rejects the record; unknown face, head piece, backpack or goggle ids become "none"; a figure type whose sex does not match is reset to the default of that sex) and sends it to every client.

**Effective body.**

1. Start from the cosmetic character if it is ≥ 0, else the cosmetic vest if ≥ 0, else the model id.
2. **Face redirect:** if the face index f is valid and the record's `FaceRelationModel{f}` is ≥ 0, use that model instead. In the stock data the three standard bodies redirect faces 4–7 (the female faces) to their female twins: `Normal` (0) → `Female Normal` (7), `Light` (1) → 8, `Heavy` (2) → 9; `balance` (25) → 26, `balance_NX` (28) → 29. Faces 0–3 (male) keep the record. So gender is chosen by the face.
3. The body file, skins, render styles, scale and **skeleton** come from the redirected effective record. The **figure type** comes from the redirected *base* model (step 2 applied to the model id without cosmetics).

**Default.** A fresh selection is model 0 (`Normal`, `CHARS_M_BODY\CM_VST_NM_DF_Nuts_JS.ltb`, skeleton 0, figure type 0, gear 0), head piece 0 (`HBasic`, no model), face 0 (`Face_a`), no goggle, no backpack, camouflage 0. A model id outside the table becomes 0.

**Defaults when the selection carries nothing** (offline, a player whose lobby data is missing; bots fill their own record from their profile, `bots.md`). Every field not sent keeps the fresh value above, and three data rules then complete the look (Open question 16):

- **Face piece**: every stock body record leaves `AttachFace` unset, which means "attach a separate face", and names no default face, so the face model is the selected face (`Face_a`: `CHARS_M_FACE\Face_A.ltb`, skin `CHARS_T_FACE\Face_A.dtx`).
- **Head piece**: when the selected head piece has an empty model (`HBasic`), the face record's `HairIndex` (if it has one) names the head piece that is drawn instead. Male faces 0–3 have none (no head model); the female faces have: face 4 (`Female_Face_a`) → head piece 23 `Female_Hair_A` (`CHARS_M_HEAD\Hair_W_A.ltb`), face 5 → 28 (`Female_Hair_Dawson`), face 6 → 29 (`Female_Hair_Sophia`), face 7 → none.
- **Goggles**: "default goggle" (the client's −1) takes the face record's `GoggleIndex`; faces 0–7 have none, so no goggle. Backpack −1: none.
- **Skins**: camouflage index 0 → `UpperSkin0` / `LowerSkin0` of the effective body. The stock bodies' skin tables (index c = 0..6), by role:

| Body | Upper (vest) skins c = 0..6 | Lower (legs) skins c = 0..6 |
|---|---|---|
| 0 Normal | `Nomal_vest_body`, `_DE`, `_SN`, `_UR`, `_WL`, `_FK`, `CM_VST_NM_TG_00_JS` | `Nomal_vest_leg`, `_DE`, `_SN`, `_UR`, `_WL`, `_FK`, `CM_LG_NM_TG_00_JS` |
| 1 Light | `Light_vest_body`, `_DE`, `_SN`, `_UR`, `_WL`, `_FK`, `CM_VST_LT_TG_00_JS` | `Light_Heavy_leg`, `_DE`, `_SN`, `_UR`, `_WL`, `_FK`, `CM_LG_HV_TG_00_JS` |
| 2 Heavy | `Heavy_vest_body`, `_DE`, `_SN`, `_UR`, `_WL`, `_FK`, `CM_VST_HV_TG_00_JS` | same as Light |
| 7 Female Normal | `F_Upper_Lev01_OD`, `F_Upper_Lev01_DE`, `CW_VST_NM_SN_00_HM`, `F_Upper_Lev01_WD`, `F_Upper_Lev01_KR`, `CW_VST_NM_FK_00_HM`, `CW_VST_NM_TG_00_HM` | `F_Leg_Lev01_OD`, `_DE`, `CW_LG_NM_SN_00_HM`, `F_Leg_Lev01_WD`, `_KR`, `CW_LG_NM_FK_00_HM`, `CW_LG_NM_TG_00_HM` |
| 8 Female Light | `F_Upper_Lev02_OD`, `_DE`, `CW_VST_LT_SN_00_HM`, `F_Upper_Lev02_WD`, `_KR`, `CW_VST_LT_FK_00_HM`, `CW_VST_LT_TG_00_HM` | `F_Leg_Lev03_OD`, `_DE`, `CW_LG_HV_SN_00_HM`, `F_Leg_Lev03_WD`, `_KR`, `CW_LG_HV_FK_00_HM`, `CW_LG_HV_TG_00_HM` (the Heavy legs, as stored) |
| 9 Female Heavy | `F_Upper_Lev03_OD`, `_DE`, `CW_VST_HV_SN_00_HM`, `F_Upper_Lev03_WD`, `_KR`, `CW_VST_HV_FK_00_HM`, `CW_VST_HV_TG_00_HM` | same as Female Light |

  All in `CHARS_T_BODY\` with extension `.dtx` (case as stored); "`_DE`" after a name means that name with its last suffix replaced (`F_Upper_Lev02_OD`, `_DE` = `F_Upper_Lev02_DE`). Render styles are the same three for all six (2 below). The female bodies carry 8 `HandsSkin` entries (the male ones 4), see `view_models.md`.

**Stock player bodies** (US data set):

| Id | Name | Body file | Skeleton | Figure | GearID | Skins (camo variants) |
|---|---|---|---|---|---|---|
| 0 | Normal | `CHARS_M_BODY\CM_VST_NM_DF_Nuts_JS.ltb` | 0 | 0 | 0 | 7 |
| 1 | Light | `CHARS_M_BODY\CM_VST_LT_DF_Nuts_JS.ltb` | 0 | 0 | 1 | 7 |
| 2 | Heavy | `CHARS_M_BODY\CM_VST_HV_DF_Nuts_JS.ltb` | 0 | 0 | 2 | 7 |
| 7 | Female Normal | `CHARS_M_BODY\Nomal_W_vest.ltb` | 1 | 1 | 0 | 7 |
| 8 | Female Light | `CHARS_M_BODY\Light_W_vest.ltb` | 1 | 1 | 1 | 7 |
| 9 | Female Heavy | `CHARS_M_BODY\Heavy_W_vest.ltb` | 1 | 1 | 2 | 7 |

The US data has 431 `Model#` records in all (named mercenaries, cosmetics, NPCs). 260 pair skeleton 0 with figure type 0 and 158 skeleton 1 with figure 1; 13 do not (for example the `Nemexis_Enemy_Woman` records use skeleton 1 with figure type 0, and the powered suits use skeletons 5 and 6). The hit table follows the skeleton, the sounds and stats the figure type.

**Team.** The team never changes the body. It sets the team-mark texture (2, below) and nothing else on the model.

### 2. Assembling the model

**Body.** The body model is created with the effective record's file, skins and render styles. Its child (`ST_M_child.ltb` or `ST_W_Child.ltb`) provides the animations. The male player bodies (the three in the table above and the named male mercenaries) have 53 nodes and share one tree shape, the female ones 60; a few special bodies (powered suits, some infected) use other skeletons and libraries. The model's scale is the record's scale (1 for the stock bodies).

**Texture slots of a player body** (piece texture indices point here; `models_ltb.md`):

| Slot | Content |
|---|---|
| 0 | Upper body: `UpperSkin{c}` |
| 1 | Legs: `LowerSkin{c}` |
| 2 | Team mark (the `Friendly_Sign` piece) |
| 3 | Clan background (the client's clan background texture, else team mark 0) |
| 4 | Clan emblem (likewise) |
| 5 … | The record's additional skins, if any: slot 5 + k = `AdditionalSkins{k}` (Corrections (2026-10-30) 1) |

- c is the camouflage index; if c ≥ the number of `UpperSkin` entries, c = 0. If the model colour index selects a colour skin list that exists, it replaces the upper and lower lists, each on its own, and also the additional skins and the hands' skins (Corrections (2026-10-30) 1). The stock suffixes for c = 0..6 are none, `_DE`, `_SN`, `_UR`, `_WL`, `_FK` and a `TG` variant (Open question 3).
- **Team mark** (client side): if the character is on the local player's team, slot 2 gets mark 1 (`CHARS_T_BODY\RED_Tie.dtx`), otherwise mark 0 (`CHARS_T_BODY\Alpha.dtx`, drawn with the transparent mark render style, so enemies show nothing). In the infection mode every character gets mark 0. Dead bodies created by the server use mark 0.
- Render styles 0, 1, 2 of the stock bodies are `RS\default.ltb`, `RS\GLOW.LTB`, `RS\MARKTRANSPARENT.LTB`; the sign and clan pieces use style 2. An infection override record (`Overidable_Model#`) replaces the render styles, armour, hit points and hand model.

**Head piece** (`Hair#`): `ModelFile0` for skeleton 0, `ModelFile1` for skeleton 1 (indexed by the effective skeleton; an index past the list means no head model). An empty file name means no head piece of its own (`HBasic`); the face's default hair then stands in, if the face has one (Defaults above). The skin is chosen by the camouflage index (out of range → 0), or by figure type if the record says its skins are per figure type; colour skins override. The head piece is an **additional model**: it is skinned with the body's node transforms, its bones matched to the body's by name (as the first-person hands are, `animation.md`). It therefore follows every body animation and aim rotation and has no sockets of its own that the game uses.

**Face piece** (`Face#`): an additional model like the head piece, created only for records that allow a separate face; otherwise the face index is forced to 0. Its skin list is per face record (with colour overrides).

**Socket attachments** (rigid models fixed to a socket, following its animated transform):

| What | Socket | Model | Offset and rotation |
|---|---|---|---|
| Weapon in hand | `RightHand` | the current weapon's `HHModel` with `HHSkin#`, `HHRenderStyle#` | none (identity): the weapon model's own origin and axes are the grip |
| Second gun (dual weapons) | `LeftHand` | `LeftHHModel` | `LeftHHModelPosOffset` (u); an Euler rotation from `LeftHHModelRotOffset` with pitch = its second component, yaw = its first, roll = its third (as stored); the model is then scaled by `LeftHHScale` |
| Goggles, masks, beards | `Socket` of the `Goggle#` record (all stock records: `Eyes`) | `Model0` for figure type 0, `Model1` for figure type 1 | none; skins by camouflage index and colour |
| Backpack | `Socket` of the `Backpack#` record (all stock: `1W_N`) | per figure type | none; slots 7 and 8 get the clan emblem and background |
| Carried flag (flag modes) | `1W_N` (`Flag#` records) | | none |

- Only one weapon is shown: the right-hand attachment swaps its model when the weapon changes. There is no back or holster display of the other carried weapons (the character attachment list has positions such as `Back` and the foot and limb names, but the player's own list uses only the right hand; nothing is ever put on the others). The third-person model is the weapon record's `HHModel` (for example M4A1 `GUNS_M_HH\HH_AR_M4A1_SH.ltb`), attached to the character's `RightHand` socket with a zero offset and identity rotation; the weapon record's `HHModelScale` is not read (scale 1). The model's own `Flash` and `Breach` sockets are where others' muzzle flash and casings start (`effects_sounds.md`).
- **While an emote plays** (9) every piece of the hand-held weapon model and of the second gun is hidden, and shown again when it ends.
- Goggles, backpacks and the second gun are flagged "hide in first person, show in third person" and become invisible if their parent is.
- If the socket is missing the attachment is simply not attached (not drawn).

### 3. Skeleton and sockets

**Node roles.** Both sexes share the core names; the female tree has 7 more nodes (hair or ribbon bones under the head, two clavicle and two spine helper bones, two hip helpers) and uses `Bip01` names for neck, head and right clavicle. Bind positions are model-space, as stored (the male bind pose is centred on the hull, feet near y = −54; the female bind pose stands on y ≈ 2, which the animations' root translation corrects).

| Role | Male node (index, parent) | Male bind position | Female node (index, parent) | Female bind position |
|---|---|---|---|---|
| Root | `Scene Root` (0) | (0, 0, 0) | `Scene Root` (0) | (0, 0, 0) |
| Extra root child | `Object08` (1), `Object05` (52) | | `Bip_root01` (1, root) | (0, 2.27, 29.10) |
| Hips (animation root of the legs) | `_zN_LowerBody` (2, root) | (0, −7.70, 1.46) | `_zN_LowerBod` (2, 1) | (0, 55.24, 3.14) |
| Pelvis (left / right, chained) | `Left_pelvis` (3, 2), `Right_pelvis` (4, 3) | (0, −7.70, 0.75), (0, 1.45, 0.75) | (3, 2), (4, 3) | (0, 55.24, 3.14), (0, 60.95, 4.23) |
| Lower spine | `Torso` (5, 4) | (0, 5.14, 0.74) | `Torso` (5, 4) | (0.01, 63.51, 4.22) |
| Chest | `Upper_torso` (6, 5) | (0, 14.27, 0.74) | `Upper_torso` (6, 5) | (0.03, 71.80, 3.33) |
| Neck | `Neck` (7, 6) | (0, 29.31, −0.10) | `Bip01 Neck` (7, 6) | (0.07, 86.05, 1.30) |
| Head | `Head` (8, 7) | (0, 31.57, 0.99) | `Bip01 Head` (8, 7) | (0.08, 89.37, 1.94) |
| Face | `Face_Jaw` (9, 8), `Face_Eyelid` (10, 8) | (0, 33.49, 3.45), (0.03, 38.96, 7.91) | `Bip01 HeadNub` (9, 8) and hair bones 10–19 | |
| Shoulders | `Left_shoulder` (11, 7), `Right_shoulder` (24, 7) | (∓4.10, 27.95, −0.43) | `Left_shoulder` (20, 7), `Bip01 R Clavicle` (33, 7) | (−2.51, 82.15, 1.13), (2.62, 82.13, 1.13) |
| Upper arms | `Left_armu` (12), `Right_armu` (25) | (∓11.36, 24.91, −1.53) | (21), (34) | |
| Forearms | `Left_arml` (13), `Right_arml` (26) | (∓21.43, 14.33, −1.77) | (22), (35) | |
| Hands | `left_hand` (14), `Right_hand` (27) | (∓31.19, 4.32, 2.14) | (23), (36) | |
| Fingers | 15–23, 28–37 | | 24–32, 37–45 | |
| Back helper | `Bone06` (38, 6) | (0.27, 19.79, −11.88) | `Bone_r_Clavicle01`, `Bone_l_Clavicle01`, `spine_right`, `spine_left` (46–49, 6) | |
| Thighs, shins, feet, toes | `Left_legu` (39, 4), `Left_legl` (40), `Left_foot` (41), `Bip01 L Toe0` (42); right 43–46 | left foot (−9.72, −46.74, −2.14) | 50–53, 54–57 (thighs parented to `Left_pelvis`) | left foot (−8.34, 10.28, −0.20) |
| Pelvis helpers | `Bone01`–`Bone05` (47–51, 3) | | `Bone_LHip`, `Bone_RHip` (58, 59, 3) | |

The odd chain (right pelvis a child of the left pelvis, legs children of the right pelvis on the male) is data; keep it (`models_ltb.md` Quirks).

**Weight sets** (stored in the body): `upper`, `Lower`, `blend`. Male: `upper` 1 on nodes 5–38, 0 elsewhere; `Lower` 1 on 0–3 and 39–52, 0.7 on 4, 0.3 on 5, 0 elsewhere (as `animation.md`). Female: `upper` 1 on 5–49; `Lower` 1 on 0–3 and 50–59, 0.7 on 4, 0.3 on 5. `blend` is unused by players.

**Sockets the game uses.** Offsets are in the parent node's space; "model space" positions are for the bind pose. Same parents and offsets on every stock male body read; the female set is the stock female body's.

| Socket | Role | Male parent node, offset, rotation | Male bind model space | Female parent, offset, rotation | Female bind model space |
|---|---|---|---|---|---|
| `RightHand` | Hand-held weapon | `Right_hand`, (7.085, 1.661, 1.277), (0.0573, −0.7151, −0.0818, 0.6918) | (35.69, −1.14, 4.24) | `Right_hand`, same offset and rotation | (33.13, 53.21, 1.37) |
| `LeftHand` | Second gun of dual weapons | `left_hand`, (0, 0, 0), (0, 0, 0, 0.7071) (= identity) | (−31.19, 4.32, 2.14) | `left_hand`, (0, 0, 0), identity | (−29.16, 59.40, 0.02) |
| `Eyes` | Goggles and masks; the dead player's head camera (Open question 7) | `Face_Eyelid`, (3.011, 0, 1.314), (−0.9772, 0, 0.2122, 0) | (0.03, 41.38, 5.68) | `Bip01 Head`, (−85.255, 0.001, 18.748), (−0.0846, 0.0846, −0.7020, 0.7020) | (0.08, 2.14, 5.20) |
| `1W_N` | Backpacks and carried flags | `Upper_torso`, (−18.328, 0, 0), (0, 0, −0.7071, 0.7071) | (0, −3.98, 2.40) | same | (−0.02, 53.78, 6.65) |
| `NAME_socket` | Name-tag anchor (`hud_crosshair.md`) | `Head`, (16.5, 0, 0.906), identity | (0, 47.97, 3.05) | `Bip01 Head`, same | (0.08, 105.45, 5.78) |
| `P_BAG_N` | None (present, unused) | `Torso`, (0, 0, 0) | (0, 5.14, 0.74) | `Torso` | |
| Hit-sphere sockets | See 11 | | | | |

- Sockets and nodes are looked up by name without regard to case.
- **Dropped weapons** do not use a socket: they appear at the world position of the node `Left_foot` (`inventory.md` section 12).
- **Thrown grenades and bullets** do not use a character socket: their origin is the camera (first person) or the weapon model's position (third person; `grenades.md`, `weapon_firing.md`).
- **The live first-person camera** is not a node either: it is the hull centre plus the eye height of `movement.md`.
- **Female neck and right hand** (measured on an older extracted client release whose male body and library give the same numbers as the US 1501.04 ones in this spec): `Bip01 Neck` is node 7, parent `Upper_torso` (6), bind (0.069, 86.050, 1.303), `LST` t = 0 (1.74, 27.99, 9.61); the female table gives it radius 0 (no sphere), so it matters only for the aim rotations (8). `RightHand` on the female body: parent `Right_hand` (node 36), offset (7.08465, 1.66108, 1.27744), rotation (0.057341, −0.715141, −0.081756, 0.69181), the same as the male socket; bind (33.13, 53.21, 1.37), `LST` t = 0 (5.69, 23.62, 22.09).
- The female `Eyes` socket sits near the floor in model space; the stock female goggle models (`Model1`, for example `ATTACH_M\W_Glass.ltb`) are authored about 89 u up, in the female bind frame, so the pair lands on the face. A goggle model must be paired with the socket of its own figure type.

### 4. Trackers and layering

Each player model runs four trackers (`animation.md`, Layered playback), in this order:

| Tracker | Weight set | Multiplier | Plays |
|---|---|---|---|
| Main | ignored (sets the base pose) | | Entry 0 of the combined list: the body's own animation (`baseAnim`, 4800 ms, on the default male; `W_VST_NM`, 3333 ms, on the default female), looping. The game never changes it while the player is alive. |
| Upper | `Upper` (the model's `upper`) | 1 | Upper-body animation (`U...`) |
| Lower | `Lower` | 1 | Leg animation (`L...`). Also the tracker whose animation would set the hull box; for players the box setter accepts and ignores it (the hull comes from `movement.md`). |
| Crouch | `Lower` | the crouch fraction r ∈ [0, 1] each update | The crouch leg animation for the current direction |

Per node j, with the main pose (p₀, q₀), weights u_j (upper) and l_j (lower):

```math
(p,q) \leftarrow (p_0,q_0);\quad (p,q) \leftarrow \mathrm{blend}\big((p,q),\ U_j,\ u_j\big);\quad (p,q) \leftarrow \mathrm{blend}\big((p,q),\ L_j,\ l_j\big);\quad (p,q) \leftarrow \mathrm{blend}\big((p,q),\ C_j,\ l_j\, r\big)
```

where blend(a, b, f) lerps positions and slerps rotations by f and is skipped when f = 0. Consequences: the main pose shows only at node 4 (`Right_pelvis`, 30 %); node 5 (`Torso`) is 70 % upper, 30 % legs; fully crouched (r = 1) the legs, hips and root are entirely the crouch animation. The root translation (−50 for stand animations, −40 for crouch ones) lives on node 0 and therefore comes from the leg trackers, so the skeleton's feet stay at the bottom of whichever hull the crouch fraction implies.

The crouch fraction r is the movement code's crouch rate (`movement.md`). The server uses the client-reported value, sent as a byte n with r = n / 255.

**The drawn body against the hull.** The model is the hull object itself: it is drawn at the object position (the hull centre) with the object's yaw, with no offset of its own. The feet follow from the root translation alone: the stand animations put node 0 at y = −50 and the crouch animations at y = −40 (their bindings' dims are (15, 50, 15) and (15, 40, 15)), and with the crouch tracker's weight r on node 0 the root ends at y = −50 + 10 r. The hull's half-height is 50 − 10 r for the same r (`movement.md`, Hull height), so the skeleton's root stays exactly at the hull's bottom at every r, during the transition too (both are linear in r). When the hull shrinks in the air (crouch-jump) the centre stays and the hull bottom rises; the drawn feet rise with it. The only part that does not follow the hull is what the animations themselves do above the root (Quirk 7).

**When the pose is decided** (Open question 14):

- **Server**: the driver of 5–7 runs once per client movement frame received (Per-tick order), and the server engine advances every model's trackers once per server frame by that frame's time (20 Hz shipped). So the server does animate characters, and its node and socket positions are evaluated from that state whenever something asks for them.
- **Own character, client**: the driver runs once per movement tick (15 ms) inside the client's movement update, and only while the client drives its own object (the console variable `ClientObjectUpdateByNet` is 0, the default). The trackers advance with the engine's per-frame model advance (`animation.md`).
- **Other characters, client**: no driver runs. Their tracker animations and times come from the server through the engine's object updates; the client advances them each rendered frame and applies its own aim rotations (8, 9). What a player sees of another player is therefore the **server's** choice of animation (from the raw key flags the client sent), not the other client's own driver.

### 5. Deciding the animation state

Every update the driver is given four values, then picks animations from them, then resets them to their defaults (rifle class, aim, walk, no direction); so a value nobody sets this update means its default.

**Weapon class** = the current weapon's `AniType` (knife class 14 if there is no weapon); the leg set = its `LowerAniType` (0 normal, 1 infected, 2 infected with club).

**Posture** (upper-body action), from the weapon status the client reports (select, deselect, reloading, idle, hit) and the firing flags, evaluated in this order:

1. Status "idle" (a status the client weapon can report): posture aim, status cleared, stop.
2. If the weapon is reloading: when the reload may finish (server: its reload timer has expired; client: the weapon has left the reload state) or the reload animation is done, finish the reload and clear the status; otherwise posture reload (if the last posture was post-fire, only once that animation is done).
3. Then, overriding step 2: if the status is "select", or the post-fire animation is done, or a select animation is playing: posture select, except that a finished select clears the status. (After a finished post-fire with the status "none", step 4 replaces this select by aim or fire in the same update, so it has no visible effect.) Else if the status is "deselect": clear it. Else if the status is "hit" or a hit animation is playing: see 9.
4. Only if the status is now "none": the firing choice.
   - Sniper class (5) and land-mine class (20): fire while firing or while the fire animation plays; aim once the last animation is done.
   - Grenade class (11) and claymore class (21): a small sequence on the fire button: at aim with fire held → pre-fire; pre-fire done → fire; fire with the button up → post-fire; post-fire done → aim; anything else → aim.
   - Other classes: fire held (the client counts only a weapon in a firing state) or a fire animation still playing → fire, but if the last posture was reload, select or alt-fire, only once that animation is done. Else alt-fire held or playing → alt-fire (same wait after reload or select). Else aim.
   - If the current ammo count is 0 and the upper animation is done → aim.

**"Fire held" for the driver** (Open question 17). Both sides read the frame's fire control flag, but they build it differently:

- **Server** (what everyone else sees): the fire flag exactly as the client sent it in that movement frame. The client sets it while the fire command is held, unless the use key is also held, a popup has the input, or the client's own "ignore fire" flag is set (its uses were not traced); it does not look at the weapon's state. So on the server a player holding fire with an empty magazine, mid-reload or between semi-automatic shots still counts as "fire held"; the reload and select branches of steps 2–3 (and the "ammo count 0" rule) are what keep the fire pose off in those cases.
- **Own client** (its own third-person body): the same flag, cleared unless the current weapon is in its firing request (the fire button reached the weapon update and sprint did not block it) **and** the weapon's state is one of begin firing, firing, fired, end firing or alt-firing. Reloading, selecting, deselecting, idle and the dry-fire state do not count. A semi-automatic gun between shots is still in its firing state while its fire or post-fire animation runs, so it counts; once it returns to idle it does not.
- **Knife**: the same rule; the knife's swing states are firing states.
- **Grenade being cooked**: the grenade class does not use the rule above; it runs its own pre-fire / fire / post-fire sequence on the (server: raw, client: masked) fire flag, so holding fire after the pin pull keeps the fire posture (`UTRFiS_B`), and releasing it starts the throw (`UTRFiS_C`).

**Movement** (legs), from the movement flags:

- After a jump starts (the client reports "jumped") and while the crouch key is up: movement jump, direction kept from the previous update. When the jump animation is done the status becomes "falling" and nothing is set (so: walk, no direction). "Landed" clears the status.
- Otherwise: run if the run flag is set and the player is not zoomed, else walk; sprint overrides both while sprinting. The run flag is set at rest unless walking or crouched (`weapon_firing.md`, Open question 6), so standing still is "run, no direction".
- Direction from the keys: forward; backward; right strafe → right, right-forward or right-backward with forward or backward; left strafe likewise. With both strafes held the left one wins.
- The crouch key does not set a movement value; crouching is entirely the crouch tracker.

### 6. Building the animation names

**Legs** (looping unless marked), for the normal leg set:

| Movement | None | F | B | L | R | FL | FR | BL | BR |
|---|---|---|---|---|---|---|---|---|---|
| walk | `LST` | `LWF` | `LWB` | `LWL` | `LWR` | `LWFL` | `LWFR` | `LWBL` | `LWBR` |
| run | `LST` | `LRF` | `LRB` | `LRL` | `LRR` | `LRFL` | `LRFR` | `LRBL` | `LRBR` |
| crouch tracker | `LC` | `LCF` | `LCB` | `LCL` | `LCR` | `LCFL` | `LCFR` | `LCBL` | `LCBR` |
| jump (non-looping) | `LSJ` | `LJJ` | `LJB` | `LJL` | `LJR` | `LJJL` | `LJJR` | `LJBL` | `LJBR` |
| sprint | none | `LRRF` | | | | `LRRL` | `LRRR` | | |
| sprint jump (unreachable, Quirk 3) | | `LRRJF` | | | | `LRRJL` | `LRRJR` | | |

The infected leg sets prefix these names with `V` (`VLST`, `VLRF`, ...) and `VG` (`VGLST`, ...).

**Upper body.** Name = `U` + class code + stance + action + direction suffix, with these parts:

- **Class code** by `AniType`: 0 rifle `RF`; 1 bullpup rifle `RF` (its moving reload `URBRReS`); 2 rifle `RH`; 3 SMG `SM`; 4 SMG `SH`; 5 sniper `SN` (its select and reload reuse the rifle's `URFRSeS` / `URFRReS`); 6 automatic shotgun `SG`; 7 pump shotgun `G`; 8 machine gun `M`; 9 Vulcan `V`; 10 pistol `P`; 11 grenade `T`; 12 rocket launcher `RL`; 13 time bomb `TB`; 14 knife `K`; 15 bullpup shotgun `F`; 16 knife `BK`; 17 flamethrower `FT`; 18 heartbeat sensor `HS`; 19 rocket launcher `RW`; 20 land mine `LM`; 21 claymore `CM`; 22 claymore detonator `CT`; 23 rifle `RA`; 25 P90 `P9`; 28 chain gun `GG`. Classes 24 (revolver), 26 (FAMAS), 27 (double barrel) and 29 (snowball gun) reuse the pistol, SMG and shotgun names with a digit suffix (confirmed, Open question 18): revolver = the pistol entries with suffix 1 on the **reload only** (`UPRSReS1` standing, `UPRReS1` moving and walking, `UPRRReS1` sprinting; aim, fire, select and alt-fire are the plain pistol names); FAMAS = the SMG entries with suffix 2 on the reload only (`USMRSReS2`, `USMRReS2`, `USMRRReS2`); double barrel = the automatic-shotgun aim names, with suffix 1 on **fire, alt-fire, select and reload** (`USGRFiS1`, moving `USGRFiF1` / walk `USGWFiF1` and the `B`, `L`, `R` forms, sprint fire = `USGRRAmF`; select `USGRSSeS1`, `USGRSeS1`, `USGRRSeS1`; reload `USGRSReS1`, `USGRReS1`, `USGRRReS1`); snowball gun = the same as the double barrel with suffix 2. The crouch column of these entries is unused by players (Quirk 2).
- **Stance** letter: `R` for run (also standing still), `W` for walk, `C` for crouch, `RR` for sprint. Each table entry holds a run, a walk, a crouch and a sprint name; the crouch name is never used by players (no player movement is "crouch", Quirk 2).
- **Action**: `Am` aim (looping), `Fi` fire, `Se` select (draw), `Re` reload; with no direction the select and reload use the standing forms `SSe` and `SRe` (`URFRSSeS`, `URFRSReS`). Fire, select and reload are non-looping. Grenade and claymore pre-fire, fire and post-fire add `_A`, `_B`, `_C` to the fire name (`UTRFiS_A`, `UTRFiS_B`, `UTRFiS_C`). Knife-type alt fire adds `2` (`UKRFiS2`); knife fires use the standing name in every direction.
- **Direction**: `S` none, `F` forward (also both forward diagonals), `B` backward (also both backward diagonals), `L`, `R`.
- **Sprint**: the sprint name of aim and fire is the class's sprint aim (`URFRRAmF`); of select and reload `URFRRSeS`, `URFRRReS`.
- **Hit reactions** (posture hit / hit2) ignore class, direction and stance: `D_HIT` and `D_HITT` (see 9).

Which name of the entry plays: the run name, replaced by the walk name when the movement is walk and the walk name exists in the library, by the sprint name when sprinting and it exists. The full rifle table (`AniType` 0, the M4A1's class):

| Posture | None | F (and FL, FR) | B (and BL, BR) | L | R | Sprint |
|---|---|---|---|---|---|---|
| aim | `URFRAmS` / walk `URFWAmS` | `URFRAmF` / `URFWAmF` | `URFRAmB` / `URFWAmB` | `URFRAmL` / `URFWAmL` | `URFRAmR` / `URFWAmR` | `URFRRAmF` |
| fire | `URFRFiS` | `URFRFiF` / `URFWFiF` | `URFRFiB` / `URFWFiB` | `URFRFiL` / `URFWFiL` | `URFRFiR` / `URFWFiR` | `URFRRAmF` |
| select | `URFRSSeS` | `URFRSeS` | `URFRSeS` | `URFRSeS` | `URFRSeS` | `URFRRSeS` |
| reload | `URFRSReS` | `URFRReS` | `URFRReS` | `URFRReS` | `URFRReS` | `URFRRReS` |
| alt-fire | `URFRAmS` (aim) | `URFRAmS` | `URFRAmS` | `URFRAmS` | `URFRAmS` | `URFRAmS` |

**Names that do not exist** in a library are dropped when the tables are built (the library is searched once per body file, case-insensitively). In the US libraries the rifle's `URFWAmS` and all moving rifle fires (`URFRFiF`, `URFRFiB`, `URFRFiL`, `URFRFiR` and their walk forms) are missing, as are the whole `SH`, `G` and `M` families. A missing walk or sprint name falls back to the run name; a missing run name means nothing is started and the tracker keeps its current animation (Quirk 1).

### 7. Starting, restarting, syncing, rates and blend times

Each update, after the state is decided:

1. Both upper and lower trackers get their looping flag from the chosen entry.
2. If the body file just changed (new model): start the crouch, lower and upper animations.
3. Otherwise:
   - start the crouch tracker's animation and the lower animation (every update; starting the animation already playing does nothing unless it is done, see below);
   - the upper animation is started if the weapon class or posture changed, or if the name differs from the last one started and the posture is aim, fire or alt-fire (so moving changes the aim animation at once, but a reload or draw keeps the variant it began with);
   - if movement or direction changed, the upper tracker's time is set to the lower tracker's current time (no key strings fire), so legs and torso stay in step. This copy is skipped when the last posture was fire or alt-fire, and for pre-fire, post-fire, reload, select and the hits it happens only once that animation is done.
4. The crouch tracker's multiplier is set to r.
5. The four values are remembered as "last" and reset to defaults.

**Starting an animation** on a tracker: if a different animation is playing, switch to it (a transition over the new animation's own interpolation time, `animation.md`); if the same animation is playing and is done, restart it from 0 (this is how a fire animation repeats once per shot cycle while firing); if it is the same and still playing, nothing. The grenade class never restarts a finished animation this way.

**Rates.** The lower and crouch trackers always play at rate 1: ground speed does not drive the legs. When the upper animation is started with a new posture, the upper rate is set to

```math
r_{upper} = \begin{cases} \dfrac{L_{3p}\, r_w}{F} & F > 0 \\ 1 & \text{otherwise} \end{cases}
```

where L₃ₚ is the new animation's length, F the weapon's `HHAniFrame_{posture}` (posture index: 0 aim, 1 pre-fire, 2 fire, 3 post-fire, 4 select, 5 reload, 6–8 hits, 9 alt-fire; `HHAniFrame_5_1` for a double barrel reloading with rounds left) and r_w the weapon rate of that posture: reload 1 + mod reload bonuses + model reload bonus + set bonus (server: capped at 2.0 and multiplied by 2 − the server's reload check time 0.8, i.e. × 1.2); select 1 + mod, model and set select bonuses; fire, pre-fire and post-fire the fire rate of `weapon_firing.md`. The effect is that the third-person action lasts F / r_w ms. No `HHAniFrame` keys exist in the data sets read, so every upper rate is 1.

**The server's reload** (Open questions 2 and 19). When the server accepts a reload request it marks the player as reloading and, while that mark is set, **ignores every shot the client reports** with a magazine weapon (all weapons except shotguns, mines, rockets, the throwing, installing, medic, first-aid, M32 and crossbow kinds, and weapons with `CheckReloadAniTime` 0). It then times the reload itself:

- If the weapon has `HHAniFrame_5` (or `HHAniFrame_5_1` for a double barrel with rounds left) non-zero, no timer is set; the reload ends only when the third-person reload animation reports done (which, with the rate formula above, is after F / r_w ms).
- Otherwise it looks up the third-person reload animation for the player's current weapon class, direction and movement (the values the driver holds at that moment; between updates these are the reset defaults, so in practice the standing walk form, which falls back to the run name `..SReS`), and sets the timer to its length L ÷ the server reload rate (1.2 × min(r, 2), as above). If no such animation exists the timer is 500 ms.
- On each driver update the reload finishes at the **earlier** of the timer expiring and the upper reload animation being done. With r = 1 that is the timer: **L / 1.2**. Finishing refills the server's copy of the clip (for `CheckReloadAniTime` weapons), plays the reload call-out to the team (`effects_sounds.md`) and clears the mark, after which shots count again.

So the server's reload end is decided by the **third-person animation**, not by the client weapon's first-person reload. For the M4A1 (US male library `URFRSReS` 1866 ms) the server accepts shots again 1866 / 1.2 = 1555 ms after the request, while the client fills its clip when its 2300 ms first-person `reload` ends (`view_models.md`); the server is ready first, so no client shot is lost to it in normal play. The same holds at any reload rate r: up to r = 2 the client needs 2300 / r and the server 1866 / (1.2 r); above 2 the server stays at 1866 / 2.4 = 778 ms and the client's animation falls back to rate 1.0 (`view_models.md`, rate cap). An implementation that keeps one shared reload timer must end the server's "ignore shots" window no later than the client's refill.

**Blend times** are the animations' own interpolation times. In the US male library every locomotion, aim, fire, select, reload and death animation used here has 0 ms; `D_HIT` and `D_HITT` have 200 ms. In the US female library `LST` and `LC` have 200 ms, the rifle aim, select and reload animations 100 ms, fires, hits and deaths 0 ms. During a blend the new animation does not advance.

Measured lengths (US male library; female in brackets where different): `LST` 4000, `LC` 4000, `LWF` 1000, `LRF`/`LRB`/`LRL`/`LRR` 600, `LCF` 1400, `LSJ`/`LJJ`/`LJB` 666, `LRRF` 500 (533), `URFRAmS` 4000, `URFRFiS` 100, `URFRSSeS` 733, `URFRSReS` 1866, `UKRFiS` 500, `UKRFiS2` 800, `UTRFiS_A` 267 (266), `UTRFiS_B` 1, `UTRFiS_C` 900, `D_HIT` 467 (533), `D_HITT` 400 (466) ms.

**Footsteps** come from the leg animations' `FOOTSTEP_KEY` / `FOOTPRINT_KEY` strings (`animation.md`), for example `LRF` at 50 ms (`FOOTSTEP_KEY 2`) and 350 ms (`FOOTSTEP_KEY 1`).

### 8. Aim pitch and twist

Aim is applied as **bone rotations after the animation pose**, on seven nodes, parent before child, each change propagating to its children:

| Order | Node (male / female) | Yaw share | Pitch share up / down | Its "parent" for the yaw sum |
|---|---|---|---|---|
| 1 | `Right_pelvis` | 0 | 0 / 0 | none |
| 2 | `Torso` | 7/17 | 0.15 / 0.35 (server: 0 / 0) | 1 |
| 3 | `Upper_torso` | 10/17 | 0.15 / 0.45 | 2 |
| 4 | `Neck` / `Bip01 Neck` | 0 | 0.25 / 0.10 | 3 |
| 5 | `Head` / `Bip01 Head` | 0 | 0.50 / 0.35 | 4 |
| 6 | `Left_shoulder` | 0 | 0.45 / 0.10 | 4 |
| 7 | `Right_shoulder` / `Bip01 R Clavicle` | 0 | 0.45 / 0.10 | 4 |

Inputs: pitch φ (radians, positive down) clamped to ±45°, and twist τ (the upper-body twist of `weapon_firing.md`: the view yaw minus the body yaw, within ±45°, 0 while any movement key is held). For each node in order, with its global (world) transform G, its yaw share y and pitch shares (u, d):

```math
A_{node} = y\,\tau + A_{parent},\qquad G \leftarrow \mathrm{Rot}_{\,\hat Y,\ p}\!\left(y\,\tau\right) G,\qquad G \leftarrow \mathrm{Rot}_{\,\hat X(A_{node}),\ p}\!\left(\varphi\cdot\begin{cases}u & \varphi<0\\ d & \varphi\ge 0\end{cases}\right) G
```

where Rot_{a,p}(θ) is the rotation by θ about the world axis a through the node's own position p, Ŷ is world up, and X̂(A) is the character's right axis turned by A about up (the object's yaw plus the accumulated twist). The sums are what players see: the chest turns the full twist (7/17 at the lower spine, the rest at the chest); the arms and weapon pitch exactly φ (0.15 + 0.15 + 0.25 + 0.45 = 1 up, 0.35 + 0.45 + 0.10 + 0.10 = 1 down); the head over-rotates (1.05 φ up, 1.25 φ down); the legs never pitch.

- **Who applies it.** Each client applies it to every character it draws (its own third-person model with its own aim; remote characters with the aim sent by the server). The server applies the same rotations to its own copy of each player, except that `Torso` gets no pitch there, so the server's arms pitch only 0.85 φ up and 0.65 φ down (Open question 5). The zero is written explicitly for that one node each time the server sets the aim inputs (the pelvis node gets the real pitch but has zero shares), so it reads as a deliberate choice, not an accident of data; keep it.
- **Where the server's spheres matter.** Client-reported player hits are checked on the server against the victim's hull box only (`weapon_firing.md`), so the server's spheres and their pitch are used only for hits the server traces itself: projectile and crossbow-bolt direct hits on a character, and instant-hit shots fired by server-side characters (bots, `bots.md`). For those the struck node (and with it the zone factor and the death animation) comes from the server pose. A server-traced shot at a player from something that is neither a player, an AI nor a pattern boss (an auto turret, a world object) skips the sphere test and records table entry 0 as the struck node.
- **Transport (legacy update path):** pitch and twist travel as one signed byte each, b = trunc(θ · 127 / (π/2)), θ' = b · (π/2) / 127 (steps of about 0.709°). The newer update path carries them in the per-object update data (resolution not traced; Open question 6).
- **Dead characters:** the server zeroes the pitch at death.

### 9. Hit reactions, emotes

- **Hit reaction:** when the server applies damage to a player (and on the victim's own client when it is damaged) the weapon status becomes "hit". On the next update, if the character took damage (last damage > 0) and is not in spawn protection, the posture becomes hit or hit2 with equal chance (`D_HIT` or `D_HITT`, upper body only, non-looping), held until that animation is done; then the status clears. While the status is "hit" no fire or aim choice is made, so a hit interrupts the firing pose for its length. With the infected body sets the names are `V_HIT` / `V_HITT` (`VD_HIT` / `VD_HITT` for bodies without `D_HIT`).
- **Hit kick on remote characters:** each time a client is told another character was damaged by someone other than itself, the next aim update of that character raises its displayed pitch (negative) by an amount that depends on a per-character counter k of such events: 0.1 k rad for k = 1..5, 0.1 (10 − k) for k = 6..10, and on the 11th event nothing and the counter restarts. The offset lasts until the next aim update. Display only; seen on the paths where the client recomputes aim (Open question 6).
- The hit kick is triggered by the server's per-character damage-flags message (the one the client also uses for its hit marks), whenever its damager is not the local player; the kick is added after the client copies the character's received pitch, so it shows for exactly one aim update and adds no animation (Open question 20).
- **Emotes** (`DT`, `Insult`, `Surrender`, `Insult_Dance`, `Salute`) replace both the upper and the lower animation, non-looping, only while aiming with no direction and walking or running; they end when done or when that condition breaks. The US male library lacks `Insult` and `Salute` (nothing plays).
  - **Trigger** (Open question 20): only in the Hunted mode (`HG`). The `G` key opens an emote menu (when alive, not voting, not in the weapon-change step); choices 1–4 are `Insult`, `Surrender`, `Insult_Dance`, `Salute` (the `DT` "play dead" entry is disabled). A choice is sent only if no emote is running on the client. The server then plays one emote sound at the player (two variants per sex for the first three, one for the salute; files under `GUNS_SND_ETC\Emotion\`, picked by the clock-seeded coin), stores the emote and hides the weapon's pieces.
  - **Playing**: the server checks the emote once per server update; the client-side copy follows from the server (4, When the pose is decided). Both trackers are set non-looping to the emote's name at once (a switch with the emote's own blend time; no layering: the upper and lower trackers both play it, the crouch tracker keeps its weight r). How this interacts with the driver's per-movement-frame starts of the leg animation was not traced (Open question 20). It lasts the animation's length, or until the posture is no longer aim with no direction while walking or running (any movement key, fire, reload, draw or jump ends it at once). At the end the weapon is shown again and the client is told it may emote again.

### 10. Death and bodies

Order on the server when a player dies:

1. The player object itself is moved down onto the floor (dying in the air snaps the body to the ground below): a ray straight down from the hull centre, up to 10000 u, against solid world and objects; if the hit is farther below than the hull's half-height h, the object is placed with its centre at hit height + h + 0.1. So the dead player's own position (the one the server and every client see, the death camera's anchor) moves, not only the body's, the backpack, goggle, flag and second-gun attachments are removed from the player object, the player becomes non-solid, and the death animation is chosen and set on its main tracker, non-looping.
2. A **body** object is created at the player's position + (0, 0.1, 0) with the player's rotation, the same body file and skins (team mark 0), render styles, scale and hull dims, and the head and face pieces as additional models. It copies the main-tracker animation (the death animation) and plays it from 0, non-looping. It has no upper, lower or crouch tracker, no aim rotation, and is not damageable. Its client effect re-creates the goggles and backpack on their sockets. The held weapon is not transferred to it: the last-selected primary is dropped at `Left_foot` (`inventory.md`); every other weapon disappears.
3. **Bodies do not stop shots** (Open question 9, answered). The server gives a body a hit box object, but that box is ray-hittable only on the server and is never sent to clients; the client's own body effect never builds its hit box. The client's shot filter skips body objects outright, so a client shot passes through a corpse and can hit a player behind it. No hit spheres are drawn on bodies either (the debug sphere display covers live characters only).

**Death animation choice.** Inputs: the death damage type, the death direction d (the direction the killing damage travelled), the character's forward f and right r, the location of the last hit sphere (11), and a coin c = rand() mod 2 drawn right after seeding the generator with the current time in seconds. Let cf = d · f, cr = d · r; front ⇔ cf < 0; left ⇔ cr > 0 (the shot travels towards the character's right, so it came from the left); head ⇔ the last hit sphere's location is head (1); groin ⇔ its location is 5.

- Explosion, claymore and land-mine deaths ("blown off"): front and cf < −cos 45° → `D_BO_F`; back and cf > cos 45° → `D_BO_B`; otherwise left → `D_BO_L`, right → `D_BO_R`.
- Endless fall → `D_W`; poison → `D_P`; melee → `D_S`. These are the death's damage type: endless fall is the `EndlessFall` damage kind (the map's endless-fall volumes; it ignores god mode); poison is the `Poison` kind (gas ammo's lasting damage; the infection's poison is reported as poison for the damage flags but its death type stays the infection kind, so it does not pick `D_P`); melee is the `Melee` kind (knives). No other kind names a death animation.
- Otherwise:
  - front, strongly (cf < −cos 45°): head → `DH` (c = 1) or `DH_1` (c = 0); else `DT` / `DT_1`;
  - front, side: left → `DLSH` (head or not, Quirk 5); right → head `DRSH`, else `DRT`;
  - back, strongly (cf > cos 45°): head → `DHB` / `DHB_1`; else `DTB` / `DTB_1`;
  - back, side: left → head `DLSH`, else `DLT`; right → head `DRSH`, else `DRT`;
  - the crouched variants (`DC`, `DC_1`, `DCH`, `DCH_1`, `DCB`, `DCB_1`, `DCBH`, `DCBH_1`) are chosen only if the last movement was "crouch", which no player sets (Quirk 2);
  - groin overrides all of these: `NS1` (c = 1) or `NS2` (c = 0).
- If the player was in the jump state at death, c is forced to 0 and the body is first turned to yaw = atan(d_x / d_z) (+π if d_z > 0; ±π/2 if d_z = 0 by the sign of d_x).
- If the chosen name does not exist in the library, `DC` plays. A model record may name a special death animation, which then always plays. In the snowball mode a frozen player plays `SD`.

Lengths (US male): `DH` 1433, `DH_1` 1700, `DT` 3033, `DT_1` 1333, `DHB` 1566, `DTB` 1766, `DLSH` 1766, `DLT` 1333, `DRSH` 1233, `DRT` 1433, `DC` 1900, `D_S` 3033, `D_P` 5633, `D_W` 7800, `D_BO_F` 1500, `NS1` 5566, `NS2` 3166 ms. The female library has no `NS1` / `NS2` (a groin death falls back to `DC`).

**No ragdoll.** A ragdoll set-up exists in the client but is never enabled; the body is pure keyframe animation and holds the last key of the death animation.

**How long the body stays.** A player's body has no lifetime of its own. When its owner respawns, the body enters the fade state. In the fade state, on the first update where its animation is done, it tells clients to fade (the client fade is disabled, so the body stays opaque) and schedules removal: 5 s later in team deathmatch, VIP elimination, clan team deathmatch, capture the flag (and clan), free-for-all, VIP weapon race, last man standing, snowball fight, survival (and clan), sabotage and headquarters; 500 s in Combat, clan default and ESL modes; 1000 s in Hunger Game mode; never in other modes. A fast respawn removes it at once. So in the common modes a body disappears 5 s after its owner respawns (or 5 s after the death animation ends, if that is later).

### 11. Hit-sphere placement

The hit tables are `Skeleton#` records: for each table index n, `Node{n}Name`, `Node{n}Radius` (u), `Node{n}Location` (1 head, 2 torso, 3 arm, 4 leg, 5 groin), `Node{n}DamageFactor` and per-node death and recoil animation names (unused by players). A character uses table `Skeleton` of its effective record. For every entry with radius > 0:

1. If the model has a **node** of that name, the sphere centre is the node's animated position (no offset).
2. Else, if it has a **socket** of that name, the centre is the socket's animated position: node position + node rotation × socket offset.
3. Else the entry is skipped.

Positions are the full pose as drawn: all four trackers and the aim rotations (8). Spheres therefore follow crouching, aiming and the walking legs (Open question 13, answered). They are **not** a fixed pose: each test asks the model for the node or socket's current world transform, evaluated from the trackers' current animations and times plus the aim rotations, at the moment of the test.

- **Client test** (the shot a player reports): the shooter's client evaluates the target's model as that client is animating it (server-chosen animations, client-advanced times, client-applied aim and hit kick) after its movement update has moved the other characters to the server time it is showing (a back-up and restore around the movement tick; its details were not traced here). All four trackers count: the legs' walk or run cycle, the upper body's weapon, fire, reload and draw animations, the crouch weight and the aim pitch and twist.
- **Server test** (8, Where the server's spheres matter): the server's model state, advanced once per server frame, with the server aim (no `Torso` pitch).
- Bodies have no spheres (10).

An implementation that uses one fixed pose (for example `LST` t = 0, crouched by squashing heights) is an approximation: the hands and arms swing tens of units with the weapon animations and the feet ±25 u with the leg cycle (Test cases 30, 31).

**Male, table 0, on the default male body** (positions in model space; "LST 0" = the single animation `LST` at t = 0 applied to all nodes, root translation included):

| # | Name | r | Loc | Factor | On | Parent, offset | Bind centre | LST 0 centre |
|---|---|---|---|---|---|---|---|---|
| 3 | `Left_pelvis` | 8.0 | 4 | 0.9 | node | | (0.00, −7.70, 0.75) | (−0.27, −6.89, −0.69) |
| 4 | `Right_pelvis` | 8.5 | 4 | 0.9 | node | | (0.00, 1.45, 0.75) | (−0.27, 2.26, −0.69) |
| 5 | `Torso` | 9.0 | 2 | 1.0 | node | | (0.00, 5.14, 0.74) | (−0.08, 5.88, −0.02) |
| 6 | `Upper_torso` | 10.0 | 2 | 1.0 | node | | (0.00, 14.27, 0.74) | (1.15, 14.52, 2.69) |
| 7 | `Neck` | 3.0 | 2 | 1.0 | node | | (0.00, 29.31, −0.10) | (5.22, 27.27, 9.60) |
| 8 | `Head` | 1.0 | 1 | 6.0 | node | | (0.00, 31.57, 0.99) | (6.45, 29.03, 10.90) |
| 9 | `Face_Jaw` | 3.0 | 1 | 6.0 | node | | (0.00, 33.49, 3.45) | (6.89, 30.82, 13.42) |
| 10 | `Face_Eyelid` | 0 | 1 | 6.0 | (unhittable) | | | |
| 11 | `Left_shoulder` | 5.5 | 3 | 0.9 | node | | (−4.10, 27.95, −0.43) | (1.94, 25.53, 11.84) |
| 12 | `Left_armu` | 4.5 | 3 | 0.9 | node | | (−11.36, 24.91, −1.53) | (−4.11, 25.07, 16.96) |
| 13 | `Left_arml` | 4.0 | 3 | 0.9 | node | | (−21.43, 14.33, −1.77) | (0.44, 12.73, 23.33) |
| 14 | `left_hand` | 3.2 | 3 | 0.9 | node | | (−31.19, 4.32, 2.14) | (10.35, 21.70, 28.98) |
| 24 | `Right_shoulder` | 5.5 | 3 | 0.9 | node | | (4.10, 27.95, −0.43) | (7.19, 27.15, 5.74) |
| 25 | `Right_armu` | 4.5 | 3 | 0.9 | node | | (11.36, 24.91, −1.53) | (14.03, 25.76, 1.95) |
| 26 | `Right_arml` | 4.0 | 3 | 0.9 | node | | (21.43, 14.33, −1.77) | (22.98, 14.22, 2.33) |
| 27 | `Right_hand` | 3.2 | 3 | 0.9 | node | | (31.19, 4.32, 2.14) | (15.43, 18.88, 13.81) |
| 39 | `Left_legu` | 8 | 4 | 0.9 | node | | (−6.14, −7.70, 0.75) | (−4.67, −6.89, 3.58) |
| 40 | `Left_legl` | 6.0 | 4 | 0.75 | node | | (−7.91, −25.25, 1.87) | (−5.43, −20.85, 14.39) |
| 41 | `Left_foot` | 5 | 4 | 0.5 | node | | (−9.72, −46.74, −2.14) | (−9.95, −42.12, 11.46) |
| 42 | `Bip01 L Toe0` | 3.0 | 4 | 0.5 | node | | (−11.68, −54.32, 6.67) | (−8.48, −49.85, 20.24) |
| 43 | `Right_Legu` | 8 | 4 | 0.9 | node | | (6.14, −7.70, 0.75) | (4.14, −6.89, −4.96) |
| 44 | `Right_legl` | 6.0 | 4 | 0.75 | node | | (7.91, −25.25, 1.87) | (14.02, −21.52, −4.22) |
| 45 | `Right_foot` | 5 | 4 | 0.5 | node | | (9.72, −46.74, −2.14) | (12.62, −41.96, −12.09) |
| 46 | `Bip01 R Toe0` | 3.0 | 4 | 0.5 | node | | (11.68, −54.32, 6.67) | (20.91, −49.78, −9.09) |
| 53 | `Face_Eyelid1` | 4.6 | 1 | 6.0 | socket | `Face_Eyelid`, (0, 0, 7) | (0.03, 36.67, 1.29) | (7.48, 34.01, 11.37) |
| 54 | `Left_armu1` | 4.0 | 3 | 0.9 | socket | `Left_armu`, (7.25, 0, 0) | (−16.36, 19.66, −1.65) | (−1.85, 18.95, 20.12) |
| 55 | `Left_arml1` | 4.0 | 3 | 0.9 | socket | `Left_arml`, (7.25, 0, 0) | (−26.31, 9.33, 0.18) | (5.39, 17.21, 26.15) |
| 56 | `Right_armu1` | 4.0 | 3 | 0.9 | socket | `Right_armu`, (7.25, 0, 0) | (16.36, 19.66, −1.65) | (18.47, 20.04, 2.14) |
| 57 | `Right_arml1` | 4.0 | 3 | 0.9 | socket | `Right_arml`, (7.25, 0, 0) | (26.31, 9.33, 0.18) | (19.21, 16.55, 8.07) |
| 58 | `Left_legu1` | 7 | 4 | 0.9 | socket | `Left_legu`, (8.8, 0, 0) | (−7.02, −16.44, 1.31) | (−5.05, −13.84, 8.96) |
| 59 | `Left_legl1` | 6 | 4 | 0.75 | socket | `Left_legl`, (11, 0, 0) | (−8.81, −36.03, −0.14) | (−7.70, −31.52, 12.92) |
| 60 | `Right_Legu1` | 7 | 4 | 0.9 | socket | `Right_Legu`, (8.8, 0, 0) | (7.02, −16.44, 1.31) | (9.06, −14.18, −4.59) |
| 61 | `Right_legl1` | 6 | 4 | 0.75 | socket | `Right_legl`, (11, 0, 0) | (8.81, −36.03, −0.14) | (13.32, −31.77, −8.17) |
| 62 | `Left_Shld` | 7.0 | 3 | 0.9 | absent | no such node or socket on any stock body | | |
| 63 | `Right_Shld` | 7.0 | 3 | 0.9 | absent | likewise | | |
| 64 | `Nuts` | 3.0 | 2 (US) / 5 (BR) | 1.0 | socket | `Left_pelvis`, (2, 0, 6) | (0.00, −5.70, 6.75) | (3.91, −4.89, 3.61) |

The `Nuts` socket exists on 43 of the 61 bodies with the 53-node male skeleton (all `CM_VST_..._Nuts...` bodies have it, including the three standard male ones; `CM_HV_UR_Brandon_JS.ltb`, the `MT_*` and the older `*_VEST.LTB` bodies do not); on the others the groin sphere does not exist.

**Female, table 1, on the default female body:**

| # | Name | r | Loc | Factor | On | Parent, offset | Bind centre | LST 0 centre |
|---|---|---|---|---|---|---|---|---|
| 3 | `Left_pelvis` | 8.0 | 4 | 0.9 | node | | (0.00, 55.24, 3.14) | (−1.44, 0.48, −1.66) |
| 4 | `Right_pelvis` | 8.5 | 4 | 0.9 | node | | (0.00, 60.95, 4.23) | (−1.21, 6.28, −1.42) |
| 5 | `Torso` | 9.0 | 2 | 1.0 | node | | (0.01, 63.51, 4.22) | (−1.65, 8.81, −1.55) |
| 6 | `Upper_torso` | 10.0 | 2 | 1.0 | node | | (0.03, 71.80, 3.33) | (−1.09, 16.67, 1.19) |
| 7 | `Bip01 Neck` | 0 | 2 | 1.0 | (unhittable) | | | |
| 8 | `Bip01 Head` | 3 | 1 | **1.0** | node | | (0.08, 89.37, 1.94) | (2.96, 31.08, 10.21) |
| 20 | `Left_shoulder` | 5.5 | 3 | 0.9 | node | | (−2.51, 82.15, 1.13) | (−1.27, 24.52, 8.72) |
| 21 | `Left_armu` | 4.5 | 3 | 0.9 | node | | (−7.74, 81.26, 1.30) | (−4.82, 23.56, 12.54) |
| 22 | `Left_arml` | 4.0 | 3 | 0.9 | node | | (−17.54, 71.54, −0.89) | (−7.02, 13.33, 21.82) |
| 23 | `left_hand` | 3.2 | 3 | 0.9 | node | | (−29.16, 59.40, 0.02) | (2.96, 23.41, 30.87) |
| 33 | `Bip01 R Clavicle` | 5.5 | 3 | 0.9 | node | | (2.62, 82.13, 1.13) | (2.21, 26.17, 5.33) |
| 34 | `Right_armu` | 4.5 | 3 | 0.9 | node | | (7.85, 81.21, 1.30) | (7.00, 24.90, 3.42) |
| 35 | `Right_arml` | 4 | 3 | 0.9 | node | | (17.65, 71.50, −0.90) | (13.56, 12.62, 2.18) |
| 36 | `Right_hand` | 3.2 | 3 | 0.9 | node | | (29.27, 59.36, 0.02) | (7.23, 20.85, 15.42) |
| 46 | `Bone_r_Clavicle01` | 3 | 2 | 1.0 | node | | (7.99, 76.65, 1.76) | (6.47, 20.79, 1.12) |
| 47 | `Bone_l_Clavicle01` | 3 | 2 | 1.0 | node | | (−7.81, 76.69, 1.76) | (−5.16, 19.46, 10.92) |
| 48 | `spine_right` | 3 | 2 | 1.0 | node | | (4.31, 76.34, 5.43) | (4.56, 19.91, 2.12) |
| 49 | `spine_left` | 3 | 2 | 1.0 | node | | (−3.94, 76.36, 5.43) | (−1.04, 17.26, 7.56) |
| 50 | `Left_legu` | 8 | 4 | 0.9 | node | | (−5.75, 55.19, 3.14) | (−5.64, −0.22, 2.21) |
| 51 | `Left_legl` | 6.0 | 4 | 0.75 | node | | (−7.00, 33.52, 1.55) | (−5.24, −16.63, 16.49) |
| 52 | `Left_foot` | 5.0 | 4 | 0.5 | node | | (−8.34, 10.28, −0.20) | (−7.73, −39.56, 12.91) |
| 53 | `Bip01 L Toe0` | 3.0 | 4 | 0.5 | node | | (−8.87, 2.33, 3.17) | (−7.22, −47.59, 16.09) |
| 54 | `Right_Legu` | 8 | 4 | 0.9 | node | | (5.75, 55.29, 3.13) | (2.76, 1.17, −5.53) |
| 55 | `Right_legl` | 6.0 | 4 | 0.75 | node | | (7.00, 33.62, 1.54) | (7.47, −19.86, −2.56) |
| 56 | `Right_foot` | 5 | 4 | 0.5 | node | | (8.34, 10.38, −0.21) | (2.90, −39.47, −14.37) |
| 57 | `Bip01 R Toe0` | 3 | 4 | 0.5 | node | | (8.87, 2.43, 3.16) | (5.52, −47.55, −12.69) |
| 60 | `Face_Eyelid1` | 0 | 1 | 6.0 | (unhittable) | | | |
| 61 | `Left_armu1` | 4.0 | 3 | 0.9 | socket | `Left_armu`, (7.25, 0, 0) | (−12.82, 76.22, 0.16) | (−5.96, 18.25, 17.35) |
| 62 | `Left_arml1` | 4.0 | 3 | 0.9 | socket | `Left_arml`, (7.25, 0, 0) | (−22.55, 66.31, −0.50) | (−2.72, 17.67, 25.72) |
| 63 | `Right_armu1` | 4.0 | 3 | 0.9 | socket | `Right_armu`, (7.25, 0, 0) | (12.93, 76.17, 0.16) | (10.40, 18.53, 2.78) |
| 64 | `Right_arml1` | 4.0 | 3 | 0.9 | socket | `Right_arml`, (7.25, 0, 0) | (22.66, 66.27, −0.50) | (10.84, 16.17, 7.88) |
| 65 | `Left_legu1` | 7 | 4 | 0.9 | socket | `Left_legu`, (8.8, 0, 0) | (−6.26, 46.42, 2.50) | (−5.48, −6.86, 7.99) |
| 66 | `Left_legl1` | 6 | 4 | 0.75 | socket | `Left_legl`, (11, 0, 0) | (−7.63, 22.57, 0.73) | (−6.41, −27.44, 14.80) |
| 67 | `Right_Legu1` | 7 | 4 | 0.9 | socket | `Right_Legu`, (8.8, 0, 0) | (6.26, 46.52, 2.49) | (4.66, −7.33, −4.33) |
| 68 | `Right_legl1` | 6 | 4 | 0.75 | socket | `Right_legl`, (11, 0, 0) | (7.63, 22.67, 0.72) | (5.31, −29.10, −8.12) |
| 69 | `Headfuck` | 4.7 | 1 | 6.0 | socket | `Bip01 Head`, (3.219, 0, 1.031) | (0.08, 92.36, 3.53) | (4.12, 33.37, 12.42) |
| 70 | `Headfuck2` | 4.7 | 1 | 6.0 | socket | `Bip01 Head`, (4.453, 0, −0.562) | (0.08, 93.86, 2.18) | (4.76, 35.01, 11.44) |

The female table has no neck sphere, no groin sphere and no 7.0 shoulder spheres, and has four 3 u torso spheres the male lacks (Open question 1).

### 12. Who may hold which weapon

Read on the Steam install (build 26236938): the weapon file, the item file (`ITEMS.TXT`), the model file, the three AI files (`AIBUTES*.TXT`), the effect file (`FX.TXT`), all 820 mission files (`GMS.rez`, `missions.md`) and the objects of every world; plus the client's item, equipment-slot and room-option code. "No link" below means none of these sources names the weapon for any holder. Weapon ids are `Weapon#` indices; our ids are the slugs of their `Name` (`combat_arms:` + lower case, other characters `_`).

#### 12.1 Holder records

| Kind | Where | Key | Notes |
|---|---|---|---|
| Player body | model file `Model#` | index | 592 records; item records of characters use the same number as `ItemID` |
| Infection override | model file `Overidable_Model#` | index | the look of an infected player (1, `InfectedOveridable`) |
| NPC body | model file `AIModel#` | `Name` | 101 records; never a player body. Hit table (`Skeleton`) 0 or 1 = humanoid; 2 = infected tanker and captains; 3 = infected assassin; 5 = NPC powered suit; 7 = EOD boss and `GXL-4209`; 8 = tutorial dummies; 9–16, 18 = boss-battle bodies; bodies and animations in 13 |
| Boss model | effect file `PatternFX#` | `Name` | one record, `HQRaid_Boss` (`CHARS_M_BODY\boss_model.ltb`, skeleton 4): not a character |

#### 12.2 How a weapon reaches a holder

Seven kinds of link exist in the data. Nothing else ties a weapon to a body.

- **L1, NPC hands in a world.** Every NPC is a `CAIHuman` world object (947 in 23 worlds). Its `ModelTemplate` names an `AIModel` (every value matches one), its `RightHand` and `LeftHand` name the weapons it holds (`<none>` = empty), `Brain` names its AI record and `NPCType` its class (`NPCDefault`, `Zombie`, `AbnormalZombie`, `ClubZombie`, `SpecialBugs`, `SpecialBoss`, `AbnormalBoss`, `BonusCoin`, `PrisonOfficer`, `SpecialZ`). `RightHand2` and `LeftHand2` are `<none>` on all 947. Holder = the `AIModel`.
- **L2, the HQ raid boss.** `PatternFX0` (`HQRaid_Boss`) lists `Weapon0..2` with a socket and a chance each: 460 on `highgun` (100), 462 on `Missile_` (50), 463 on `bomb_` (50). The weapon file's `[PatternWeapon0..3]` hold the boss's fire patterns for 460, 461, 462, 463 (`WeaponId`, `WeaponType` `NormalWeapon` or `SpecialWeapon`, timings). Worlds place the boss as a `PatternObject` with `ModelName` `HQRaid_Boss` (NemexisHQ with pattern NPC `HQRaid_Boss`, NemexisLab_CP with `LABRaid_Boss`, both `[PatternNPC#]` in the AI files).
- **L3, suit sets given by a mode.** A mission file's `[ChangeCharInfo_n]` (`ModelIndex` a, `WeaponIndex` b) pairs `[ChangeCharModel_a]` (`ModelId`: a `Model#` index; `HUDIndex`) with `[ChangeCharWeapon_b]` (`Primary<i>`, `Secondary<i>`: weapon ids). `WeaponIndex` is the number of a weapon set, not a weapon id. Five files have them: NEMEXISLAB_CP, ROADKILL_RTDM and SNOWVALLEY_RTDM (sets 0–4), DESERTHOLE_RTDM and TESTING_T (sets 0–9, the B and R coloured suits). VIP Escort files use the same shape (`VIPChangeCharInfo`, `VIPChangeCharModel_0`, `VIPChangeCharWeapon_0`) for the VIP (Model 87 `VIP_Melissa`, or 289 `VIP_Hauser`; weapons 91 and 0). The mode code that reads these sections is not in the reference code read (Open question 25).
- **L4, infected loadouts of a mode.** Quarantine (`missions.md` F4.1): `[InfectedModelSetting]` `MeleeWeapon0` 191, `[InfectedHostModelSetting]` 190, in all 33 `_QRT` files; NOXIOUSDAMNATION_LLHG gives its infected 1346. Holder = any player in the infected state, drawn with its body's infection override.
- **L5, infected mercenaries.** Use items of `SubType` 20 (infected mercenary) carry `InfectedModelNo` (a `Model#`) and `InfectedWeaponNo`: item 54 → Model 56 `Infected Merc Subtype-S` with 521; item 55 → Model 57 `Infected Merc Subtype-P` with 522; items 148 and 150 → Model 513 `Infected theshinning` with 1734. Holder = that body (`game_modes.md` M8: "a mercenary character uses its own infected model and weapon").
- **L6, NPC skills.** `[NpcSkill#]` `UseWeaponName` in the AI files is a weapon a skill fires; a `[Brain#]` lists skills in `NpcSkillName<i>`, and a `CAIHuman` names its brain. Holder = the `AIModel`s of the objects whose brain lists that skill.
- **L7, equipment slots of a player's loadout** (12.3). This is the only rule that limits what a player body may carry.

Pickups anyone may take are not holder links: co-op `[WeaponSpawn]` (M32_FT, Minigun_Bounty_FT, Airstrike_Napalm_FT in 8 files), Hunted map spawn tables (533 in NOXIOUSDAMNATION_LHG and _LLHG), the Hired Guns turret (558).

#### 12.3 Specialists and assassins

**The game's own test.** A character counts as a specialist when its `Character#` record in the item file has `ItemType` 30, an assassin with `ItemType` 37 (36 is "assassin weapon"); the room option "no specialists" (weapon-use option 9) refuses a loadout whose character has either type. In this install all 303 `Character#` records have `ItemType` 22 (mercenary), and no item of any kind has type 30, 36 or 37. **By data, no body is a specialist or an assassin.**

**By name only** (no data marks them): specialist bodies `Model` 22 `Specialist01_M`, 23 `Specialist01_W`, 27 `Specialist02_M`, 38 `Specialist04`, 43 `Brazilian_Specialist`, 44 `Specialist02_Rebalance`, 45 `Specialist01_Rebalance`, 46 `Brazil_Speical01_Rebalance`, 47 `Specialist04_Rebalance`, 48 `Specialist01_W_Rebalance`, 236, 237, 252, 253 (the `_recruit` copies), 486 (`Specialist01_M` cosmetic); assassin bodies 63 `Assassin_F`, 64 `Assassin_M`, 244, 245 (`_recruit`). `Nemexis_Enemy_Man_Special`, `Nemexis_Enemy_Man_Assassin` and the woman pair (82–85) are named alike; no data ties them to specialist or assassin rules either.

**What restricts a weapon: the equip-slot type.** A `WeaponItem#` may carry `EquipBackpackType` (0 normal, 1 special, 2 perk, 3 assassin, 4 special with three slots; default 0) and `EquipOnlyBackpack` 1. Such a weapon can only go into a backpack slot of the same type; if the player has none, the equip screen refuses it ("equip only in the special backpack", "… assassin backpack"). A player's backpack slots and their types come from:

1. the character item: `AddSlot` slots, all of type `SlotType`; this type is also the character's own slot type;
2. the equipped backpack item, the same way;
3. use items of `SubType` 18 (extend special backpack), `AddSlot` slots of type `SlotType` (or `SlotType_0`, `SlotType_1`, … per slot), counted only when the character's own slot type is not 0, and then given the character's type.

Install values: 116 characters have `AddSlot` (1 or 2), every one with `SlotType` 0; four extend items (36, 52: one slot of type 1; 76: types 1 and 0; 77: two of type 1) never count, since no character has a non-normal type. Weapons with a slot type: 1 (special) on 173 `Claymore_Controller`, 174 `Satellite_Scanner`, 290 `M32_FT`, 291 `Minigun_Bounty_FT`, 293 `Airstrike_Napalm_FT`, 447 `Mobile_Grenade_Controller`, 558 `HG2_Auto_Turret`; 3 (assassin) on 536 `Nerve_Confuse_Mine`. So these 8 are the specialist-only (7) and assassin-only (1) weapons, and with this install's data **no loadout can hold them**; players get some of them only from the mode pickups above.

The assassin crossbows (533 `Assassin_Bowgun_Nor`, 534 `Assassin_Bowgun_Expl`) are item type 8 (rocket launcher) with no slot type: any body may carry them. 535 `Assassin_Bowgun_Expl_Controller` is 534's controller (534's `SubControlWeaponName`; 535 has `NoSelect` 1): it comes with 534 and is never chosen on its own.

The only data link between player bodies and particular weapons is L3 (the powered suits, 12.4) and L5 (infected mercenaries, 12.6).

#### 12.4 Boss and NPC weapons (the 48 candidates)

The 48 weapons whose id contains `npc`, `boss`, `hqraid`, `powered_suit` or `assassin`, by link. "AIModel" names are `AIModel#` `Name`s, with the record number. "Item" = the weapon has a `WeaponItem#` record (only those can be in a player's inventory).

| Weapon | Link | Holder record(s) | Worlds or files |
|---|---|---|---|
| 62 `AK-47_npc` | L1 right | SH_Rifle 0, SH_Recon 1, SH_MGunner 2, SH_Sniper 3, SH_HeavyGunner 4, SH_RoyalGuard 6, Agent_Z 12, SubMission_Z_1 76, SubMission_Z_2 79 | DesertFox_CP, Sandhog_CP1, TwinTower, SnowStorm |
| 63 `IMI_UZI_npc` | L1 right | SH_Rifle 0, SH_Recon 1, SH_MGunner 2, SH_Sniper 3, SH_HeavyGunner 4, SH_RoyalGuard 6 | DesertFox_CP, Sandhog_CP1 |
| 64 `M24_npc` | L1 right | SH_Recon 1, SH_Sniper 3, SH_RoyalGuard 6 | DesertFox_CP, Sandhog_CP1 |
| 65 `M60_npc` | L1 right (once left) | SH_Recon 1, SH_MGunner 2, SH_RoyalGuard 6 | DesertFox_CP, Sandhog_CP1, TwinTower |
| 66 `LAW_npc` | L1 right | SH_HeavyGunner 4 | DesertFox_CP, Sandhog_CP1 |
| 67 `Minigun_npc` | none | — | — |
| 68 `M67_npc` | L1 left | SH_Rifle 0, SH_Recon 1, SH_MGunner 2, SH_Sniper 3, SH_HeavyGunner 4, SH_RoyalGuard 6, Agent_Z 12, Squad_0–9, 11, 12 (13–22, 38, 39), HQRAIDNPC_1–3 (32–34), Powered_Suit_Npc_Melee 35, Powered_Suit_Npc_Smg 36 | DesertFox_CP, Sandhog_CP1, TwinTower, BloodMoney, HiredGuns2, NemexisHQ, NemexisLab_CP |
| 69 `Dynamite_npc` | L1 left | SH_Recon 1, SH_MGunner 2, SH_Sniper 3, SH_HeavyGunner 4, SH_RoyalGuard 6 | DesertFox_CP, Sandhog_CP1 |
| 283 `VH_Normal_NPC` | L1 right | Normal_Infecter_Common_D 11 | BlackLung, CabinFever, DeathRoom_CP, RailwayDefence |
| 286 `VH_Explode_NPC` | L1 right | Normal_Infecter_Explode_C 10 | same four |
| 287 `VH_Heavy_NPC` | L1 right | Heavy_Infecter_Gas_A 8 | same four |
| 288 `VH_Light_NPC` | L1 right | Light_Infecter_Speed_B 9 | same four |
| 366 `Quarantine_Stick_NPC` | L1 right | Normal_Infecter_Common_D 11 (brain `Infected_Stick`, `ClubZombie`) | BlackLung |
| 416 `MP5A4_NPC` | L1 right; item | Squad_3 15, Squad_4 16, Squad_12 39 | BloodMoney, HiredGuns2 |
| 417 `M4A1_NPC` | L1 right; item | Squad_1 13, Squad_2 14, Squad_11 38 | BloodMoney, HiredGuns2 |
| 418 `P90_NPC` | L1 right; item | Squad_5 17, Squad_6 18, Squad_10 37 | BloodMoney, HiredGuns2 |
| 419 `M416_NPC` | L1 right; item | Squad_7 19, Squad_8 20 | BloodMoney, HiredGuns2 |
| 420 `MG21E_NPC` | L1 right; item | Squad_9 21, Squad_0 22 | BloodMoney, HiredGuns2 |
| 421 `Watergun_NPC` | none | — | — |
| 457 `HQRAIDNPC_1_MELEE` | L1 right | HQRAIDNPC_1 32 | NemexisHQ, NemexisLab_CP |
| 458 `HQRAIDNPC_2_SMG` | L1 right | HQRAIDNPC_2 33, Powered_Suit_Npc_Melee 35, Powered_Suit_Npc_Smg 36, SubMission_Z_1 76, SubMission_Z_2 79 | NemexisHQ, NemexisLab_CP, SnowStorm |
| 459 `HQRAIDNPC_3_SPECIAL` | L1 right | HQRAIDNPC_3 34 | NemexisHQ, NemexisLab_CP |
| 460 `HQRAID_BOSS_MINIGUN` | L2 | `PatternFX0` `HQRaid_Boss` (`Weapon0`, `highgun`) | NemexisHQ, NemexisLab_CP |
| 461 `HQRAID_BOSS_FLAME` | L2 (pattern only) | `HQRaid_Boss` by `[PatternWeapon1]`; not mounted by `PatternFX0` | same |
| 462 `HQRAID_BOSS_MISSILE` | L2 | `HQRaid_Boss` (`Weapon1`, `Missile_`) | same |
| 463 `HQRAID_BOSS_POISONBOMB` | L2 | `HQRaid_Boss` (`Weapon2`, `bomb_`) | same |
| 526 `Powered_Suit_Basic_SR` | L3 set 3 (8); item | Model 59 `Powered_Suit_SR`, 580 `_SRB`, 585 `_SRR` | 5 files |
| 527 `Powered_Suit_Basic_SMG` | L3 set 2 (7); item | Model 58 `Powered_Suit_SMG`, 579, 584 | 5 files |
| 528 `Powered_Suit_Boss_MG` | L3 set 0 (5); item | Model 62 `Powered_Suit_Boss`, 583, 588 | 5 files |
| 529 `Powered_Suit_Basic_Pistol` | L3 sets 1, 4 (6, 9); item | Model 60 `Powered_Suit_Melee`, 581, 586, and 61 `Powered_suit_medic`, 582, 587 | 5 files |
| 530 `Powered_Suit_Basic_Melee` | L3 set 1 (6); item | Model 60, 581, 586 | 5 files |
| 531 `Powered_Suit_Boss_Launcher` | L3 set 0 (5); item | Model 62, 583, 588 | 5 files |
| 532 `Powered_Suit_Basic_Medic` | L3 set 4 (9); item | Model 61, 582, 587 | 5 files |
| 533 `Assassin_Bowgun_Nor` | item only | any body (12.3) | Hunted spawns |
| 534 `Assassin_Bowgun_Expl` | item only | any body | — |
| 535 `Assassin_Bowgun_Expl_Controller` | with 534 | whoever holds 534 | — |
| 537 `NPC_Powered_Suit_Basic_SMG` | none | — (AIModel 36 holds 458 instead) | — |
| 538 `NPC_Powered_Suit_Basic_Melee` | none | — (AIModel 35 holds 458 instead) | — |
| 560 `Ravens_G36C_NPC` | none | — | — |
| 561 `Souzas_Parafal_NPC` | none | — | — |
| 562 `PP19_MOD_Scorpion_NPC` | none | — | — |
| 676 `InfectedS_Captain_Hand_Boss` | L1 right | InfectedS_Captain_Boss 50, InfectedS_W_Captain_Boss 70, InfectedS_Captain 40 (TowerDefence only) | InfectedShip_CP, TowerDefence |
| 858 `Abnormal_Boss_Minigun` | L1 left | Abnormal_Boss 75, Abnormal_Du_Boss 89, Abnormal_Du_Boss_A 90, Abnormal_Boss_Fortress 92, Abnormal_Boss_Beetle 94, Abnormal_Boss_GroundThorn 98 | BossBattle_CP, BossBattle3_CP, BossBattle4_CP, BossRush, BossShip, SnowStorm, DarkThanDark |
| 863 `Abnormal_Boss_Minigun_Left` | none | — | — |
| 864 `Abnormal_IceGas_boss` | L6 only | none: `[NpcSkill16]` `P_Boss_IceGas` fires it, but no `[Brain#]` lists that skill | — |
| 1142 `VH_Normal_NPC_CFS` | L1 right | Normal_Infect_CFS 82, Normal_Infect_Carnival_CFS 83 | CostaRecon_CP |
| 1143 `VH_Explode_NPC_CFS` | L1 right | Explode_Infect_CFS 84, Explode_Infect_Carnival_CFS 85 | CostaRecon_CP |
| 1144 `VH_Heavy_NPC_CFS` | L1 right | Hauser_Infect_CFS 86 | CostaRecon_CP |

Counts: 25 by L1, 4 by L2 (461 by pattern only), 7 by L3, 3 crossbow family (no holder restriction), 1 by an unused skill, 8 with no link. Item records exist for 14 (416–420, 526–534); the other 34 never appear in a player's inventory.

The L3 sets in full (NEMEXISLAB_CP; the R/B files repeat them for the coloured suits, sets 5–9 = 0–4):

| Set | Body (`Model#`) | Weapons |
|---|---|---|
| 0 | 62 `Powered_Suit_Boss` (583 B, 588 R) | primary 528, secondary 531 |
| 1 | 60 `Powered_Suit_Melee` (581, 586) | primary 530, secondary 529 |
| 2 | 58 `Powered_Suit_SMG` (579, 584) | primary 527 |
| 3 | 59 `Powered_Suit_SR` (580, 585) | primary 526 |
| 4 | 61 `Powered_suit_medic` (582, 587) | primary 532, secondary 529 |

Powered-suit bodies are skeleton 5 (the boss suits 6) with `HumanModel` 0.

#### 12.5 Weapon fields

No field of the weapon records (297 keys in all) names a body, skeleton, figure type, team or character type, and none limits who may hold a weapon. The fields that come closest, and what they do:

- `LowerAniType` 1 or 2: the infected leg set (4, 5); 40 weapons. It is the only body-class signal in the weapon data: these weapons are animated for infected legs.
- `Hidden` true (21 weapons, all infected hands): the third-person weapon model is not attached.
- `NoSelect` 1 (173, 447, 535): the weapon cannot be selected; each is the controller named by another weapon's `SubControlWeaponName`.
- `AIPrimaryWeapon` true (457, 538): an NPC treats this melee weapon as a ranged one when choosing attacks.
- `[DeditWeapon]` (54 names: infected and NPC hands, boss weapons, the CFS event weapons, 7 modifiable guns): a list in the weapon file not read by the game code read; a hint that the names are NPC or special weapons, nothing more.

On the item side: `EquipBackpackType` and `EquipOnlyBackpack` (12.3); `CanRestrict` (45 weapon items, the room's weapon restriction); a character's `RestrictedSlot<i>` lists item types the character cannot change (11 head gear, 12 face attachment, 14 clothes, 15 class kit, 16 backpack, 39–43 cosmetic slots), never a weapon type.

#### 12.6 Infected hands

The 40 weapons with `LowerAniType` 1 or 2 (all `Guntype` 0, knife class 39, except 366: class 47 and leg set 2) and who holds them. "Own type" is the infected type the hand's name and data both point to; "also" lists other holders the worlds give it.

| Weapon | Link | Own type | Also |
|---|---|---|---|
| 190 `Quarantine_Host_Hand` | L4 | any infected player, host | L1 left on AIModels 8, 9, 10, 11 (BlackLung, CabinFever, RailwayDefence, TowerDefence) |
| 191 `Quarantine_Virus_Hand` | L4 | any infected player | — |
| 283, 286, 287, 288 `VH_*_NPC` | L1 | 12.4 table | — |
| 344 `Mutation_Quarantine_Virus_Hand` | none (has an item record, type 7) | — | — |
| 366 `Quarantine_Stick_NPC` | L1 | AIModel 11 as club zombie | — |
| 521 `Quarantine_SubtypeS_Hand` | L5 | Model 56 | — |
| 522 `Quarantine_SubtypeP_Hand` | L5 | Model 57 | — |
| 654 `InfectedS_Captain_Hand` | L1 | InfectedS_Captain 40, InfectedS_W_Captain 60 | DarkThanDark: InfectedS_Man01_Host 41, _Man02_Host 43, _W_Man01_Host 61, _Man01_Virus 42, SubMission_Z_1 76; RailwayDefence: Infecter_Abnormal_Suicide 73 |
| 655 `InfectedS_Man01_Host_Hand` | L1 | InfectedS_Man01_Host 41, _W_Man01_Host 61 | Normal_Infecter_Common_D 11 (DarkThanDark), Infecter_Abnormal_Suicide 73 (TowerDefence), InfectedS_Captain 40 (ZombieWar) |
| 656 `InfectedS_Man01_Virus_Hand` | L1 | 42, 62 | — |
| 657 `InfectedS_Man02_Host_Hand` | L1 | 43, 63 | — |
| 658 `InfectedS_Man02_Virus_Hand` | L1 | 44, 64 | — |
| 659 `InfectedS_Woman_Host_Hand` | L1 | 45, 65 | Infecter_Abnormal_Suicide 73 (TowerDefence) |
| 660 `InfectedS_Woman_Virus_Hand` | L1 | 46, 66 | — |
| 661 `InfectedS_New_Man01_Hand` | L1 | 47, 67 | Infecter_Abnormal_Suicide 73 (TowerDefence) |
| 662 `InfectedS_New_Man02_Hand` | L1 | 48, 68 | — |
| 663 `InfectedS_New_Woman_Hand` | L1 | 49, 69, InfectedS_New_Woman_StunUp 52, InfectedS_New_Woman_Block 59 | — |
| 674 `InfectedS_Man02_Host_Hand_NSee` | none | (by name: InfectedS_Man02_Host_NSee 51, not placed in any world) | — |
| 675 `InfectedS_New_Woman_Hand_StunUp` | L1 | 52 | — |
| 676 `InfectedS_Captain_Hand_Boss` | L1 | 50, 70 | InfectedS_Captain 40 (TowerDefence) |
| 678 `InfectedS_Captain_1_Hand` | none | (by name: the captains 40, 60) | — |
| 684 `InfectedS_Man01_Host_B_Hand` | L1 | InfectedS_Man01_Host_B 53 | — |
| 685 `InfectedS_Man02_Host_B_Hand` | L1 | InfectedS_Man02_Host_B 54 | InfectedS_New_Man02_B 57 |
| 686 `InfectedS_Woman_Host_B_Hand` | L1 | InfectedS_Woman_Host_B 55 | — |
| 687 `InfectedS_New_Man01_B_Hand` | L1 | InfectedS_New_Man01_B 56 | — |
| 688 `InfectedS_New_Man02_B_Hand` | L1 | 57 | — |
| 689 `InfectedS_New_Woman_B_Hand` | L1 | InfectedS_New_Woman_B 58 | — |
| 854 `Abnormal_Normal` | L1 | Infecter_Abnormal_Normal 71, Infecter_Abnormal_Normal_boss 80 | Infecter_Abnormal_Suicide 73 (SnowStorm); bosses Abnormal_Du_Boss_S 91, Abnormal_Boss_Callus 95, Abnormal_Boss3_Bugs_A 96, _B 99 |
| 859 `Abnormal_Explode` | L1 | Infecter_Abnormal_Explode 72 | — |
| 860 `Abnormal_Blood` | L1 | Infecter_Abnormal_Blood 74, Infecter_Abnormal_Blood_boss 81 | — |
| 861 `Bonus_Abnormal_Explode` | L1 | Bonus_Abnormal_Explode 77 | — |
| 862 `Bonus_Abnormal_Blood` | L1 | Bonus_Abnormal_Blood 78 | — |
| 1142, 1143, 1144 `VH_*_NPC_CFS` | L1 | 12.4 table | — |
| 1346 `Quarantine_LC_Melee` | L4 | any infected player in NOXIOUSDAMNATION_LLHG | — |
| 1734 `The Shining_Hand` | L5 | Model 513 | — |

The `InfectedS_W_*` records (60–70) are the same types as their `InfectedS_*` twins and share their hands. Counts: 31 hands by L1, 3 by L4 (190 also by L1), 3 by L5, 3 with no link. The `InfectedS_*` worlds are InfectedShip_CP (and DarkThanDark, TowerDefence, ZombieWar for the "also" cases); the `Abnormal_*` ones SnowStorm, TowerDefence, ZombieRoom and the boss maps; `VH_*` 12.4.

What it means for a rule "infected hands only on their own infected type": the player-side infected bodies are the infection overrides of the player bodies (190 and 191, and 1346 in its mode) and the three infected mercenaries (521, 522, 1734); every other hand belongs to an `AIModel` NPC type, never to a player body.

### 13. NPC and boss bodies (added 2026-10-31)

Read on the Steam install, build 26236938 (model, attachment, weapon, effect and AI attribute files, every world's objects, and the body models and their animation libraries in the character archives), with the reference code that builds, layers and animates AI characters and the pattern boss. Everything here is what an implementation needs to draw an NPC or boss body as a playable character's look; how the AI itself moves and decides is out of scope (`bots.md` 11).

#### 13.1 What "skeleton" means

The `Skeleton` number of a model record names a **hit table** (`Skeleton#` in the model file: sphere names, radii, locations, damage factors, 11), not a set of bones. The bones, sockets, weight sets and animations come from the body file and its animation library. Two records with the same skeleton number can have different bone trees (skeleton 7 is used by two unrelated bosses), and two records with different numbers can share one tree. Section 13.4 therefore describes **body families** (one bone tree and one animation library each) and 13.6, separately, the hit tables 2–18.

#### 13.2 The `AIModel#` records

The model file holds 101 `AIModel#` records (0–100). The game keeps them in the same list as the player `Model#` records, after them (an NPC's model id is the `Model#` count plus its `AIModel` index); worlds and AI data name them only by `Name`. A record is read like a `Model#` record (1) with these differences:

| Field | Default | Meaning for an NPC body |
|---|---|---|
| `Name` | — | The key worlds use (`ModelTemplate` of a `CAIHuman` object) |
| `ModelFile` | a placeholder model | Body file; the animation library is its child (13.3) |
| `Skeleton` | none (invalid) | Hit table (13.1, 13.5) |
| `Skin0..` | — | Texture slot i = `Skin{i}`, in order; no camouflage index, no colour lists, no team mark, no clan slots (the player-body slot layout of 2 does not apply) |
| `RenderStyle0..` | — | Render style per slot index, as for players |
| `AttachFace` | 1 | 1: draw a separate face piece (below); 0: none |
| `AIFace` | the name `AIFace0` | `Name` of the face piece: looked up case-insensitively among the `Face#` records, then the `AIFace#` records (59 records, 0–58, the same fields as `Face#`). The default name matches no record, so it means "no face" |
| `AIHair` | the name `AIhair0` | `Name` of a `Hair#` record drawn as the head piece (skeleton-0 or -1 file by the record's skeleton, as in 2); the default matches nothing, so no head piece |
| `AIGoggle` | the name `AIGoggle0` | `Name` of a `Goggle#` record (attachment file) put on its socket (`Eyes`), file by figure type, as in 2; the default matches nothing |
| `AIBackpack` | the name `AIBackpack0` | Same for `Backpack#`; no record sets it |
| `FigureType` | 0 | Body type (sex, sounds); set to 1 on the female squad records only |
| `AIOnly` | 1 | 1: the AI animates the body with its own NPC tables (13.4); 0: with the player tables. Only records 35 and 36 (the NPC powered suits) set 0 |
| `HumanModel` | 1 | 0 on 35 and 36 (as on the player suits, 12.4): not a human (no dropped weapon, robot HUD) |
| `ModelScale` | (1, 1, 1) | Object scale; **no record in the install sets it** (neither `Model#` nor `AIModel#`), so every NPC body is drawn at scale 1 |
| `UseSpecialDeathAni` | none | Death animation that always plays (`VDIE` on 8, 9, 40, 50, 60, 70, 100; `death` on 87, 88) |
| `ClientFX0..` | none | Effect groups attached to the body (eye glows, markers; `game_modes.md`) |
| `MovingFX`, `BodyExplodFX` | none | Effect while moving (12 `Agent_Z`), effect on death (35, 36 `Robot_Exploding`) |
| `BodySurfaceType`, `HeadSurfaceType` | — | Impact surface of body and head hits |
| `LFootprintCustomSnd`, `RFootprintCustomSnd` | none | Footstep sound records replacing the figure type's |
| `AIName` | `Hero` | The AI template the record pairs with (AI files); not used for the look |
| `GearID`, `Mass`, `MaxArmor`, `MovePenalty`, class marks, screen animations | | As on `Model#`; not part of the look |

Not read for `AIModel` records: `UpperSkin#`, `LowerSkin#`, colour skins, `FaceRelationModel#`, `HandIndex`, `AdditionalSkins#`, infection overrides.

The resolved face, head and goggle pieces (all records that set them resolve):

| AIModel | Face piece file (`CHARS_M_FACE\` unless shown) | Head piece | Goggle |
|---|---|---|---|
| 0, 1 | `MT_NO_FACE.ltb` | — | — |
| 2, 3, 4, 5, 6 | `MT_GG_FACE`, `MT_SP_FACE`, `MT_RPG_FACE`, `MT_BM_FACE`, `MT_ELT_FACE` (`.ltb`) | — | — |
| 10, 84, 85, 86 | `CM_FC_NM_Host_YK.ltb` | — | — |
| 11, 73, 82, 83 | `CM_FC_NM_Virus_HM.ltb` | — | — |
| 12 | `CHARS_M_HEAD\CM_FC_NM_SpecialZ_HM.ltb` | — | — |
| 13, 14, 15, 16 | `Face_A`, `Face_W_A`, `Face_B`, `CW_FC_NM_DF_Dawson_JS` | `Cap` (Hair3) | `Chemical_Gassmask`, `Russian_Gasmask`, `Inline_Goggle`, `Anti-FlashGoggles` |
| 17, 18 | `Mei_Face`, `Deckland_Face` | `Black_Beret` (Hair24) | `Orange_AntiFlash_Goggle`, `Metal_AntiFlash` |
| 19, 20 | `CW_FC_NM_DF_HANA_JS`, `Zadan_Face` | `Skull_Helmet` (Hair12) | `Bandana Scarf` |
| 21, 22 | `CW_FC_HV_DF_Chloe_JS`, `CM_FC_HV_UR_Brandon_JS` | `Force_Warrior_Helmet` (Hair61) | — |
| 32, 33, 34 | the three `..._Ninjas_...` faces | — | — |
| 37, 38, 39 | `Face#` 29, 33, 34 (the specialist faces) | 37: `Specialist04_hair` (Hair79), 38: `Brazilian_Specialist_Cap` (Hair86) | — |
| 41–49, 51–59, 61–69 | the matching `CM_FC_infected_ship_...` / `CW_FC_Infected_Ship_...` face | — | — |
| 71, 72, 74, 76–81 | the `CM_FC_Cabin_Fever2_...` faces | — | — |
| 23–31 | the three office faces (`Dead_Dev_0..2_Face`) | — | — |
| 87, 88 | `FC_Tutorial_Dummy_KI.ltb` | — | — |
| 7, 8, 9, 35, 36, 40, 50, 60, 70, 75, 89–100 | none (`AttachFace` 0) | — | — |

Two names exist in both lists: `InfectedS_New_Man01_B` and `InfectedS_New_Woman_B` (`Face#` 56, 58 and 55, 57; `AIFace#` 41, 43). The first match wins, so AIModels 56 and 58 get `Face#` 56 and 55: the same face file, skins and render styles as the `AIFace` records, plus the face's eye effect `Eye_NewType_Tail` (which the two AIModel records list themselves anyway). The face and head pieces are additional models skinned to the body's nodes by name, exactly as for players (2).

#### 13.3 The 947 world NPCs, by skeleton

Every NPC in a world is a `CAIHuman` object whose `ModelTemplate` names an `AIModel` (12.2 L1). 947 objects in 23 worlds; 86 of the 101 records are placed. "Family" is the body family of 13.4: M = the player male tree and library, W = the player female tree and library, the others as named there.

| Skeleton | AIModel | Body file (`CHARS_M_BODY\`) | Family | Placed | Worlds |
|---|---|---|---|---|---|
| 0 | 0 `SH_Rifle` | `MT_AR_BODY.ltb` | M | 83 | desertfox_cp, sandhog_cp1, twintower |
| 0 | 1 `SH_Recon` | `MT_NO_BODY.ltb` | M | 67 | desertfox_cp, sandhog_cp1 |
| 0 | 2 `SH_MGunner` | `MT_GG_BODY.ltb` | M | 83 | desertfox_cp, sandhog_cp1, twintower |
| 0 | 3 `SH_Sniper` | `MT_SP_BODY.ltb` | M | 69 | desertfox_cp, sandhog_cp1 |
| 0 | 4 `SH_HeavyGunner` | `MT_RPG_BODY.ltb` | M | 33 | desertfox_cp, sandhog_cp1 |
| 0 | 5 `SH_Assualt` | `MT_BM_BODY.ltb` | M | 0 | |
| 0 | 6 `SH_RoyalGuard` | `MT_ELT_BODY.ltb` | M | 66 | desertfox_cp, sandhog_cp1 |
| 0 | 10 `Normal_Infecter_Explode_C` | `MT_BM_BODY.ltb` | M | 11 | blacklung, cabinfever, deathroom_cp, railwaydefence, towerdefence |
| 0 | 11 `Normal_Infecter_Common_D` | `MT_AR_BODY.ltb` | M | 32 | blacklung, cabinfever, darkthandark, deathroom_cp, railwaydefence, towerdefence |
| 0 | 12 `Agent_Z` | `CM_Body_NM_SpecialZ_HM.LTB` | M | 1 | desertfox_cp |
| 0 | 13 `Squad_1` | `CM_Body_HV_NX_YK.ltb` | M | 2 | bloodmoney, hiredguns2 |
| 0 | 15 `Squad_3` | `CM_Body_LT_NX_YK.ltb` | M | 2 | bloodmoney, hiredguns2 |
| 0 | 18 `Squad_6` | `CM_VST_LT_DF_Nuts_Deckland_JS.ltb` | M | 2 | bloodmoney, hiredguns2 |
| 0 | 20 `Squad_8` | `CM_VST_NM_DE_Nuts_Zadan_JS.ltb` | M | 2 | bloodmoney, hiredguns2 |
| 0 | 22 `Squad_0` | `CM_VST_HV_UR_Nuts_Brandon_JS.ltb` | M | 1 | bloodmoney |
| 0 | 23 `Dead_Dev_0`, 29 `Atk_Dev_0`, 31 `Z_Dev_0` | `Light_W_vest.ltb` | **W** (Quirk 12) | 0 | |
| 0 | 24 `Dead_Dev_1`, 27 `Atk_Dev_1`, 30 `Z_Dev_1` | `CM_VST_LT_DF_Nuts_JS.ltb` | M | 0 | |
| 0 | 25 `Dead_Dev_2`, 26 `Atk_Dev_2`, 28 `Z_Dev_2` | `CM_VST_HV_DF_Nuts_JS.ltb` | M | 0 | |
| 0 | 32 `HQRAIDNPC_1` | `CM_Body_NPC_Ninjas_Melee_SH.ltb` | M | 20 | nemexishq, nemexislab_cp |
| 0 | 33 `HQRAIDNPC_2` | `CM_Body_NPC_Ninjas_SMG_SH.ltb` | M | 99 | nemexishq, nemexislab_cp |
| 0 | 34 `HQRAIDNPC_3` | `CM_Body_NPC_Ninjas_SP2_SH.ltb` | M | 15 | nemexishq, nemexislab_cp |
| 0 | 38 `Squad_11` | `CM_Body_NM_Brazil_Speical_SH.ltb` | M | 1 | hiredguns2 |
| 0 | 39 `Squad_12` | `CM_Body_NM_Special_BC.ltb` | M | 1 | hiredguns2 |
| 0 | 41 `InfectedS_Man01_Host` | `CM_Body_infected_ship_man01_host_KE.ltb` | M | 23 | darkthandark, infectedship_cp |
| 0 | 42 `InfectedS_Man01_Virus` | same | M | 6 | darkthandark, infectedship_cp |
| 0 | 43 `InfectedS_Man02_Host` | `CM_Body_infected_ship_man02_host_KE.ltb` | M | 6 | darkthandark, infectedship_cp |
| 0 | 44 `InfectedS_Man02_Virus` | same | M | 5 | infectedship_cp |
| 0 | 47 `InfectedS_New_Man01` | `CM_Body_Infected_Ship_New_Type_Man01_KE.ltb` | M | 6 | infectedship_cp |
| 0 | 48 `InfectedS_New_Man02` | `CM_Body_Infected_Ship_New_Type_Man02_KE.ltb` | M | 6 | infectedship_cp |
| 0 | 51 `InfectedS_Man02_Host_NSee` | `CM_Body_infected_ship_man02_host_KE.ltb` | M | 0 | |
| 0 | 53 `InfectedS_Man01_Host_B` | `CM_Body_infected_ship_man01_host_B_KE.ltb` | M | 4 | infectedship_cp |
| 0 | 54 `InfectedS_Man02_Host_B` | `CM_Body_infected_ship_man02_host_B_KE.ltb` | M | 5 | infectedship_cp |
| 0 | 56 `InfectedS_New_Man01_B` | `CM_Body_Infected_Ship_New_Type_Man01_B_KE.ltb` | M | 7 | infectedship_cp |
| 0 | 57 `InfectedS_New_Man02_B` | `CM_Body_Infected_Ship_New_Type_Man02_B_KE.ltb` | M | 8 | infectedship_cp |
| 0 | 61, 62 `InfectedS_W_Man01_Host` / `_Virus` | `CM_Body_infected_ship_man01_host_KE.ltb` | M | 2, 1 | darkthandark, infectedship_cp |
| 0 | 63, 64 `InfectedS_W_Man02_Host` / `_Virus` | `CM_Body_infected_ship_man02_host_KE.ltb` | M | 1, 1 | infectedship_cp |
| 0 | 67, 68 `InfectedS_W_New_Man01` / `02` | the `New_Type_Man01` / `02` bodies | M | 1, 1 | infectedship_cp |
| 0 | 71 `Infecter_Abnormal_Normal` | `CM_Body_Cabin_Fever2_Basic_AB.ltb` | M | 43 | snowstorm, towerdefence, zombieroom |
| 0 | 72 `Infecter_Abnormal_Explode` | `CM_Body_Cabin_Fever2_Explosive_AB.ltb` | M | 7 | snowstorm, towerdefence |
| 0 | 73 `Infecter_Abnormal_Suicide` | `CM_Body_Cabin_Fever2_Suiside_AB.ltb` | M | 10 | railwaydefence, snowstorm, towerdefence |
| 0 | 74 `Infecter_Abnormal_Blood` | `CM_Body_Cabin_Fever2_Blood_AB_KE.ltb` | M | 5 | snowstorm, towerdefence |
| 0 | 76 `SubMission_Z_1` | `CM_Body_NM_SpecialZ_HM.ltb` | M | 3 | darkthandark, snowstorm |
| 0 | 77 `Bonus_Abnormal_Explode` | `CM_Body_Cabin_Fever2_Explosive_AB.ltb` | M | 3 | snowstorm, towerdefence |
| 0 | 78 `Bonus_Abnormal_Blood` | `CM_Body_Cabin_Fever2_Blood_AB_KE.ltb` | M | 3 | snowstorm, towerdefence |
| 0 | 79 `SubMission_Z_2` | `CM_Body_NM_SpecialZ_HM.ltb` | M | 2 | snowstorm |
| 0 | 80 `Infecter_Abnormal_Normal_boss` | `CM_Body_Cabin_Fever2_Basic_AB.ltb` | M | 6 | snowstorm, towerdefence, zombieroom |
| 0 | 81 `Infecter_Abnormal_Blood_boss` | `CM_Body_Cabin_Fever2_Blood_AB_KE.ltb` | M | 3 | railwaydefence, snowstorm, towerdefence |
| 0 | 82, 83 `Normal_Infect_CFS`, `Normal_Infect_Carnival_CFS` | `MT_AR_BODY.ltb` | M | 2, 2 | costarecon_cp |
| 0 | 84, 85 `Explode_Infect_CFS`, `Explode_Infect_Carnival_CFS` | `MT_BM_BODY.ltb` | M | 2, 2 | costarecon_cp |
| 0 | 86 `Hauser_Infect_CFS` | `CM_Body_NM_M_YK.ltb` | M | 2 | costarecon_cp |
| 1 | 7 `SH_Sandhog` | `boss_model.ltb` | Pattern boss (13.6) | 0 | |
| 1 | 14 `Squad_2`, 16 `Squad_4` | `CW_Body_HV_NX_YK.ltb`, `CW_Body_LT_NX_YK.ltb` | W | 1, 1 | bloodmoney |
| 1 | 17 `Squad_5` | `Mei_BODY.ltb` | W | 2 | bloodmoney, hiredguns2 |
| 1 | 19 `Squad_7` | `CW_VST_NM_DF_HANA_JS.ltb` | W | 2 | bloodmoney, hiredguns2 |
| 1 | 21 `Squad_9` | `CW_HV_DF_Chloe_JS.ltb` | W | 2 | bloodmoney, hiredguns2 |
| 1 | 37 `Squad_10` | `CW_Body_NM_Special04_JP.ltb` | W | 1 | hiredguns2 |
| 1 | 45, 46 `InfectedS_Woman_Host` / `_Virus` | `CW_Body_Infected_Ship_Woman_Virus_KE.ltb` | W | 5, 5 | infectedship_cp |
| 1 | 49 `InfectedS_New_Woman`, 52 `_StunUp`, 59 `_Block` | `CW_Body_Infected_Ship_New_Type_Woman_KE.ltb` | W | 6, 2, 1 | infectedship_cp |
| 1 | 55 `InfectedS_Woman_Host_B` | `CW_Body_Infected_Ship_Woman_Virus_B_KE.ltb` | W | 6 | infectedship_cp |
| 1 | 58 `InfectedS_New_Woman_B` | `CW_Body_Infected_Ship_New_Type_Woman_B_KE.ltb` | W | 9 | infectedship_cp |
| 1 | 65, 66 `InfectedS_W_Woman_Host` / `_Virus`, 69 `InfectedS_W_New_Woman` | as 45, 46 and 49 | W | 1, 1, 1 | infectedship_cp |
| 2 | 8 `Heavy_Infecter_Gas_A` | `VIM_F_NM_DF_Tanker_SH.ltb` | Tanker | 11 | blacklung, cabinfever, deathroom_cp, railwaydefence, towerdefence |
| 2 | 40 `InfectedS_Captain` | `CM_Body_Infected_Ship_Captain_KE.ltb` | Tanker | 53 | darkthandark, infectedship_cp, towerdefence, zombiewar |
| 2 | 50 `InfectedS_Captain_Boss` | `CM_Body_Infected_Ship_Captain_B_KE.ltb` | Tanker | 1 | infectedship_cp |
| 2 | 60 `InfectedS_W_Captain`, 70 `InfectedS_W_Captain_Boss` | as 40, 50 | Tanker | 1, 1 | infectedship_cp |
| 3 | 9 `Light_Infecter_Speed_B` | `VIW_F_NM_DF_Assassin_CH.ltb` | Assassin | 11 | blacklung, cabinfever, deathroom_cp, railwaydefence, towerdefence |
| 3 | 100 `The Shining` | `CW_VST_TheShining.ltb` | Assassin | 0 | |
| 5 | 35 `Powered_Suit_Npc_Melee`, 36 `Powered_Suit_Npc_Smg` | `Suit_small_Base.ltb` | Small suit | 6, 12 | nemexislab_cp |
| 7 | 75 `Abnormal_Boss` | `PV_CabinFever2_EOD_Boss_KE.ltb` | EOD boss | 3 | bossrush, snowstorm |
| 7 | 89 `Abnormal_Du_Boss` | `GXL-4209.ltb` | Boss (own) | 4 | bossbattle_cp, bossrush, darkthandark |
| 8 | 87 `Dummy`, 88 `Dummy2` | `Body_Tutorial_Dummy_KI.ltb` (**not in the install**) | — | 0 | |
| 9 | 90 `Abnormal_Du_Boss_A` | `GXL-4209B.ltb` | Boss (own) | 1 | bossbattle_cp |
| 10 | 91 `Abnormal_Du_Boss_S` | `GXL-X.ltb` | Boss (own) | 1 | bossbattle_cp |
| 11 | 93 `Abnormal_Boss_Robot` | `Mob_Robot.ltb` | Boss (own) | 2 | bossbattle4_cp, bossship |
| 12 | 92 `Abnormal_Boss_Fortress` | `Boss_Fortress.ltb` | Boss (own) | 4 | bossbattle4_cp, bossrush, bossship |
| 13 | 95 `Abnormal_Boss_Callus` | `Callus.ltb` | Boss (own) | 4 | bossbattle3_cp, bossbattle4_cp, bossrush, darkthandark |
| 14 | 94 `Abnormal_Boss_Beetle` | `Beetle.ltb` | Boss (own) | 4 | bossbattle3_cp, bossbattle4_cp, bossrush |
| 15 | 97 `Abnormal_Boss_Scorpion` | `Scorpion.ltb` | Boss (own) | 3 | bossbattle3_cp, bossbattle4_cp, bossrush |
| 16 | 98 `Abnormal_Boss_GroundThorn` | `Thorn.ltb` | Boss (own) | 5 | bossbattle3_cp, bossbattle4_cp, bossrush, darkthandark |
| 18 | 96 `Abnormal_Boss3_Bugs_A`, 99 `Abnormal_Boss3_Bugs_B` | `Bugs_Explosive.ltb`, `Bugs_MeleeHeavy.ltb` | Boss (own) | 3, 3 | bossbattle3_cp, bossbattle4_cp, bossrush |

Totals by skeleton: 0: 768; 1: 46; 2: 67; 3: 11; 5: 18; 7: 7; 9: 1; 10: 1; 11: 2; 12: 4; 13: 4; 14: 4; 15: 3; 16: 5; 18: 6 (947). Skeleton 4 is the pattern boss's (13.6), 6 the big player suit's (13.7), 17 is used by no record. By family: M 768, W 46, Tanker 67, Assassin 11, small suit 18, EOD boss 3, own-animation bosses 34. So **814 of the 947 NPCs (86 %) wear a player bone tree and the player animation library**.

#### 13.4 Body families (bone sets and animation libraries)

The `AIModel` records name 64 body files (ignoring case); 63 are in the install. Each belongs to one family. Animation names below are exactly as stored; lookups ignore case (6).

| Family | Body files | Nodes | Library (child 1) | Weight sets | Aim nodes present (8) | `RightHand` socket |
|---|---|---|---|---|---|---|
| M | 33 (`MT_*_BODY`, the squad, ninja, infected-ship man, Cabin Fever 2 and `CM_VST_*` bodies) | 53, the stock male tree (same names, order and parents as `CM_VST_NM_DF_Nuts_JS.ltb`) | `ST_M_child.ltb` (1,543–1,545 entries with the body's own) | `upper`, `Lower` (most also `blend`) | all | as the stock male (3) |
| W | 11 (the `CW_*`, `Mei_BODY`, `Light_W_vest` bodies) | 60, the stock female tree | `ST_W_Child.ltb` (1,340 entries) | `upper`, `blend`, `Lower` | all (female names) | as the stock female |
| Tanker | `VIM_F_NM_DF_Tanker_SH`, `CM_Body_Infected_Ship_Captain_KE`, `..._Captain_B_KE` | 43 | `Ani_VI_Tanker_SH.ltb` (103) | `upper`, `lower`, `blend` | all on the tanker; on the captains `Neck` is `Bip01 Neck` and `Left_shoulder` is `Bip01 L Clavicle` | `Right_hand`, the player offset and rotation |
| Assassin | `VIW_F_NM_DF_Assassin_CH`, `CW_VST_TheShining` | 71 | `ANI_VI_Assassin_CH.ltb` (149) | `upper`, `lower`, `blend` (Shining: none of its own; Quirk 13) | female names | `Right_hand`, player offset |
| Small suit | `Suit_small_Base` | 44 | `small_anibase.ltb` (272) | `upper`, `Lower` | no `Upper_torso` | `Right_hand`, offset 0, identity |
| Big suit | `Suit_BIG_Base` (player records only, 13.7) | 43 | `BIG_anibase.ltb` (122) | `upper`, `Lower` | no `Upper_torso` | `Right_hand`, (−12.109, 0, 0), identity |
| EOD boss | `PV_CabinFever2_EOD_Boss_KE` | 35 | `Ani_CabinFever2_EOD_Boss_KE.ltb` (20) | `upper`, `Lower`, `blend`, `BODY`, `AL` | only `Bip01 Head` | `Dummy003`, (37, 6.031, 2.031), rotation (0.5, 0.5, 0.5, 0.5) |
| Pattern boss | `boss_model` | 66 | `boss_anibase.ltb` (14) | none | none | none (13.6) |
| Own-animation bosses | `GXL-4209`, `GXL-4209B`, `GXL-X`, `Boss_Fortress`, `Mob_Robot`, `Beetle`, `Callus`, `Scorpion`, `Thorn`, `Bugs_Explosive`, `Bugs_MeleeHeavy` | 9–152 | none: 4–18 animations in the body itself | `Upper` (every node 1) and `Lower` (every node 0) | at most a `Head` | `Boss_Fortress`: `Gun_R` (100, 0, 0); `Mob_Robot`: `Bip01 L Forearm` (40, 0, 0); others none |

**M and W.** Bone tree, sockets, weight sets and library are the player's, so 3–11 apply unchanged. Socket differences from the stock bodies: the seven `MT_*_BODY` files and `CM_Body_NM_Special_BC` have no `Nuts`; `MT_BM_BODY` has no `Right_arml1`; `MT_GG_BODY` has no `Eyes` socket (it has one named `Face_Eyelid` instead, so goggles cannot attach) and no animation of its own (its main tracker plays the library's first entry). Some bodies carry extra sockets (`eye_l`, `eye_r` on the Cabin Fever 2 blood body) used only by effects. The libraries also hold the NPC-only upper and lower animations (names ending `_NP`, `_Np`, `_Tb`; for example `URFRFiS_NP`, `URFST_Np`, `LRFWF_Np`, `UTRFiS_Np`) that the human AI uses (13.5); a player-driven body never needs them.

**Tanker** (skeleton 2). Nodes by role: 0 `Scene Root`, 1 `Dummy_all`, 2 `_zN_LowerBody` (bind y 53.03), 3 `Left_pelvis`, 4 `Right_pelvis` (child of 3), 5 `Torso`, 6 `Upper_torso`, 7 `Neck`, 8 `Head` (bind y 98.75), 9 `Left_shoulder` (child of 7), 10–12 left arm to `left_hand`, 13–18 left fingers, 19–21 left rig helpers (`Rig_wrist_Left`, `Rig_Shoulder_Left01`, `Rig_Shoulder_Left`), 22 `Right_shoulder`, 23–25 right arm to `Right_hand`, 26–31 right fingers, 32–34 right rig helpers, 35–38 left leg (`Left_legu` child of 4, `Left_legl`, `Left_foot`, `Bip01 L Toe0`), 39–42 right leg. The bind pose stands on y ≈ 0 (feet 7.47, toes 0.77); the animations' root translation (−50) puts it on the hull bottom, as for the female. Weight sets: `upper` 0.7 on 4 and 1 on 5–34; `lower` 1 on 0–3 and 35–42, 0.7 on 4; `blend` 0.3 on 4 and 5. Sockets: the player names (`RightHand`, `LeftHand`, `1W_N`, `P_BAG_N`, `NAME_socket` 16.5 above `Head`, the eight limb hit sockets, `Face_Eyelid1`), plus `Tanker_hand_L ` (a trailing space in the name), `eye_l`, `eye_r`; no `Eyes`. The two captain bodies have the **same tree shape** and use the same library, but four nodes carry other names: 7 `Bip01 Neck`, 9 `Bip01 L Clavicle`, 37 `Bip01 L Foot` (the left foot) and 42 `Left_foot` (in the tree's place of the right toe; Quirk 14). Library (103 entries, interpolation 100 ms unless said): body animations `BaseAnim`, `I_info_Tank`, `VD_hit` 333, `VDIE` 3733 (0 ms); legs `VLST`, `VLC` and the eight `VLC*` 1333, `VLW*` (eight) 1200, `VLR*` (eight) 800, `VLRRF`, `VLRRFL`, `VLRRFR` 667, `VLJS`, `VLJJ`, `VLJJL`, `VLJJR`, `VLJB`, `VLJBL`, `VLJBR`, `VLJL`, `VLJR`, `VLRRJF`, `VLRRJL`, `VLRRJR` 666; upper infected-hand class `VUM...` (`VUMRAmS` and the five `VUMCAm*` 1333, `VUMWAm{F,B,L,R}` 1200, `VUMRAm{F,B,L,R}` 800, `VUMRRAmF` 667, `VUMJAm{S,F,B,L,R}`, `VUMRRJAmF`, `VUMRSeS`, `VUMRSeF`, `VUMCSeS`, `VUMWSeF`, `VUMRRSeF`, `VUMRFIS` 666, the last 0 ms); the same 27 names with `VUT` (the infected grenade class). Every entry has dims (15, 50, 15) and root translation (0, −50, 0), the crouch ones included (the tanker does not lower its root when crouching).

**Assassin** (skeleton 3). The female player tree with seven extra hair bones (`Bone01`–`Bone07` under `Bip01 Head`), finger end nodes (`... Nub`) and two hip and spine helpers: 0 `Scene Root`, 1 `Bip_root01`, 2 `_zN_LowerBod`, 3 `Left_pelvis`, 4 `Right_pelvis`, 5 `Torso`, 6 `Upper_torso`, 7 `Bip01 Neck`, 8 `Bip01 Head`, 9 `Bip01 HeadNub`, 10–16 hair, 17 `Left_shoulder` … 20 `left_hand`, 21–35 left fingers, 36 `Bip01 R Clavicle` … 39 `Right_hand`, 40–54 right fingers, 55 `spine_right`, 56 `spine_left`, 57 `Bone08`, 58 `Bone09`, 59–63 left leg, 64–68 right leg, 69 `Bone_LHip`, 70 `Bone_RHip`. Weight sets: `upper` 1 on 5–56; `lower` 1 on 0–5 and 59–70 (node 5 is in both at 1; layered after the upper tracker, the legs' animation wins at the torso); `blend` 0.3 on 4, 5. Sockets: the female player set (`Eyes` included, `Headfuck`, `Headfuck2`) plus `eye_l`, `eye_r`. Library (149 entries, 100 ms interpolation): `VLST` 5666, `VLW*` (eight) 1000, `VLR*` (eight) 600, `VLRRF`, `VLRRL`, `VLRRR` 533, `VLSJ` and the jumps 666, `VLC` 5700 and the eight `VLC*` 1400 (crouch entries dims (15, 40, 15), root −40); `VUM...` complete (`VUMRAMS` 5666, `VUMRFIS` and every `VUM{R,W,C}FI{S,F,B,L,R}` and their `2`-suffixed alternates 1000, `VUMRSSES`, `VUMRSES`, `VUMRSEF`, `VUMCSES`, `VUMRRSES`, `VUMRSEL` 800, `VUMRRAMF` 533); the `VUT` set; `VD_HIT`, `VD_HITT` 533, `VDIE` 3000, `VDIE1` 3933, `I_INFO_ASS`.

**Small suit** (skeleton 5; also the player suits, 13.7). Nodes: 0 `Scene Root`, 1 `_zN_LowerBody`, 2 `Left_pelvis`, 3 `Right_pelvis`, 4 `Torso`, 5 `Neck`, 6 `Head`, 7 `Face_Jaw`, 8 `Left_shoulder` … 11 `left_hand` and four left finger nodes, 16 `Right_shoulder` … 19 `Right_hand` and four right finger nodes, 24 `Left_legu`, 25 `Left_legl`, 26 `Left_foot`, 27 toe, 28–33 the left leg's wheels and pistons (`r_l_wheel`, `r_r_wheel`, `l_legu_up_d`, `l_legu_up_u`, `l_legu_b`, `l_legu_front`), 34–43 the same for the right leg. Weight sets `upper` 1 on 4–23, `Lower` 1 on 0–2 and 24–43, 0.7 on 3. Sockets: `RightHand` and `LeftHand` on the hands with zero offset, `1W_N` on `Torso` (12.047, 0, 0), `P_BAG_N`, `NAME_socket` on `Head` (31.219, 0, −19.359) (bind 169.93 up), limb hit sockets at 16–22 u along the limbs; no `Eyes`. Library (272 entries, 50 ms interpolation): legs `LST` 1333, `LWF`, `LWB`, `LWL`, `LWR` 1200, `LRF`, `LRB`, `LRL`, `LRR` 1000, `LRRF` 533, `LC` 1333, `LCF`, `LCB`, `LCL`, `LCR` 1200, `LSJ`, `LJJ`, `LJB`, `LJL`, `LJR`, `LRRJF` 666 (20 of the 38 player leg names: **no diagonals**, no sprint strafes); stand entries dims (80, 100, 50) root (0, −100, 0), crouch entries (80, 70, 50) root (0, −70, 0); deaths: every player death name (`DH` … `D_BO_R`, `NS1`, `NS2`, `SD`, `VDIE`, also `D_L`, `D_DU`, `D_DA`) 2666; `D_HIT`, `D_HITT` 833; upper classes `USMG...` (SMG suit), `UP...` (pistol), `USN...` (sniper), `UME...` (melee, `1`-suffixed alternates), `URE...` (medic), `UK...` (knife), each with `_AI` copies of its fires for the NPC suits, and `USMGABORD`, `UMEABORD` 1500 (boarding).

**Big suit** (skeleton 6). The small suit's tree minus two finger nodes, plus `Face_Eyelid` (8); same roles. `Lower` 0.7 on 3. Sockets as the small suit's plus `Eyes` on `Head` (8.281, 0, 7.891), `Nuts` on `Left_pelvis`, `Face_Eyelid1`; `RightHand` (−12.109, 0, 0). Library (122 entries): the same 20 leg names and every death name, stand dims (128, 150, 70) root (0, −150, −40), crouch dims (128, 120, 80) root (0, −120, −20); upper `UMG...` (machine-gun suit) and `UFG...` (launcher suit), `UMGABORD`.

**EOD boss** (skeleton 7, `Abnormal_Boss`). A biped with `Bip001` names: 0 `Scene Root`, 1 `Bip001`, 2 `Bip001 Pelvis`, 3–5 spine, 6 `Bip001 Neck`, 7 `Bip01 Head`, 8–11 left arm, 12 `Bone_Lgun`, 13 `Dummy002`, 14 `Dummy001`, 15–18 right arm, 19 `Bone_Rgun`, 20 `Dummy004`, 21 `Dummy003`, 22 `Bone_AL01`, 23 `Bone_AL02` (the weak point), 24–28 left leg (`Left_foot` 28), 29–33 right leg (`Right_foot` 33), 34 `Bone_B`. Weight sets: `upper` 1 on 2–23, `Lower` 1 on 0–3 and 24–34, `AL` 1 on 22, 23, `BODY`, `blend`. Sockets: `RightHand` on `Dummy003`, `LeftHand` on `Dummy001` (0, −5.625, −2.578) rotation (−0.5, −0.5, 0.5, 0.5), `breach`, `breach01`, `EYE_L`, `Foot_R`, `Foot_L`, `AL`. Library (20, all 0 ms interpolation, dims (70, 100, 55), root −100): `ilde` (sic) 1667, `Walk` 1367, `AL` 1667, `Death` 4667, `Damage_ST` 1567, `Damage_LP` 1433, `Damage_ED` 2366, `Foot_Attack` 2800, `All_Attack` 7033, `Run_Attack_ST` 667, `Run_Attack_LP` 867, `Run_Attack_ED` 867, `Run_Attack_ED_D` 2034, `Attack_L_ST`, `Attack_R_ST` 800, `Attack_L_LP`, `Attack_R_LP` 533, `Attack_L_ED`, `Attack_R_ED` 1667.

**Own-animation bosses.** No library; the animations are in the body. Every one has `Upper` (1 on every node) and `Lower` (0 on every node) and 200 ms interpolation (the two bug bodies 100 and 0). Names, lengths (ms) and the stand dims and root translation of the first entry:

| Body | Animations | Dims, root |
|---|---|---|
| `GXL-4209` (66 nodes, `Bip001` biped with tongue and pustule bones) | `Idle` 1666, `Appear` 3533, `Attack` 2366, `Walk` 1333, `Rush_ST` 1233, `Rush_LP` 100, `Rush_ED` 1133, `Grab_ST` 1833, `Grab_LP` 400, `Grab_ED` 1133, `Grab` 5333, `Die` 1900 | (100, 80, 100) idle, (100, 100, 100) others; (0, −75, 0) |
| `GXL-4209B` (55, tentacle bones `Bone_CS_*`, `Bone_PD_*`, eyes `YJ_*`) | `Idle`, `Walk`, `Idle_R`, `Idle_L` 1666, `Appear` 5766, `Strafe` 4500, `Strafe_L` 2400, `Strafe_R` 2433, `Strafe_Pro`, `_L`, `_R` 4166, `Collide`, `_L`, `_R`, `Die` 2966 | (120, 160, 120) idle, (120, 180, 120); (0, −160, 0) |
| `GXL-X` (19) | `Idle` 1666, `Walk` 1000, `Attack` 1500, `Die` 1600 | (30, 15, 30), (30, 20, 30); (0, −15, 0) |
| `Boss_Fortress` (32, `Bip001` biped with `Gun_L/R`, `Missile_L/R`, `Bullet_L/R`) | `Idle` 800, `Idle_02` 166, `Appear` 3400, `Walk` 1333, `Jump` 4000, `Melee` 2566, `Missile_ST` 1233, `Missile_LP` 533, `Missile_ED` 1234, `Missile_Random`, `Missile_Target` 2833, `Gatling_ST` 633, `Gatling_LP` 2567, `Gatling_ED` 1100, `Gatling_0_ST` 1466, `Gatling_0_LP` 266, `Gatling_0_ED` 1234, `Die` 2400 | (120, 100, 120); (0, −100, −50) |
| `Mob_Robot` (25, `Bip01` biped, no left hand) | `Idle` 800, `Walk` 1200, `Laser` 1800, `Melee` 2100, `Dash_ST` 733, `Dash_LP` 66, `Dash_ED` 1200, `Die` 2000 | (50, 35, 50); (0, −30, 0) |
| `Beetle` (152) | `Idle` 1733, `Appear` 3833, `Walk` 1333, `Attack` 2733, `Immobility` 2066, `Defense_ST` 2333, `Defense_LP` 1733, `Defense_ED` 1000, `Rush_ST` 2000, `Rush_LP` 100, `Rush_ED` 1000, `Die` 3166 | (200, 120, 250); (0, −120, 70) |
| `Callus` (32) | `Idle` 1000, `Breed` 1733, `Hit` 333, `Die` 1433 | (70, 70, 70); (0, −70, −10) |
| `Scorpion` (122) | `Idle` 2000, `Appear` 3500, `Walk` 2000, `Attack` 3133, `Melee` 3333, `Toxic` 2000, `Bury_ST` 700, `Bury_LP` 3000, `Bury_ED` 1633, `Die` 2333 | (240, 140, 320); (0, −140, −50) |
| `Thorn` (9) | `Idle` 1033, `AppearAtk_ST_LP` 1033, `AppearAtk_ST` 233, `AppearAtk_LP` 1033, `AppearAtk_ED` 366 | (60, 200, 60); (0, −525, 0) idle, −200 others |
| `Bugs_Explosive` (26) | `Idle`, `Walk` 500, `Attack` 2433, `Die` 1300 | (35, 35, 40); (0, 0, 0) |
| `Bugs_MeleeHeavy` (38) | `Idle` 2000, `Walk` 1000, `Attack` 1100, `Die` 2500 | (20, 25, 40); (0, −25, 0) |

None of these has a stand (`LST`), crouch, jump, strafe or backward animation, an aim pose or a pitch-capable spine with player names. Their AI (the `SpecialBoss` and `SpecialBugs` NPC types the worlds give them) is not in the reference code read (Open question 33).

#### 13.5 How the original animates NPC bodies

The AI runs the same animation driver as the player (4–7) with these differences:

- **Trackers.** Main, an upper tracker on weight set `Upper` and a lower tracker on `Lower`; **no crouch tracker** (crouched NPCs play the crouch leg names on the lower tracker instead). The lower tracker's entry sets the AI's hull: its dims × the object's scale (13.6). The `SpecialBugs`-type NPCs ask for a weight set `Whole` on both trackers, which no body has (Open question 34). The abnormal-boss type adds a third tracker on weight set `AL` that loops the animation `AL` (the weak point's pulse).
- **Which table.** By NPC type and record: records with `AIOnly` 0 (the NPC suits) use the player tables exactly, after mapping the NPC-only postures to player ones (NPC fire variants and "appear" → fire, the third hit → hit, the idle variants → aim; loose walking → walk; investigating, hiding and guarding → run; the NPC melee class → knife). Human NPCs (`NPCDefault`, `SpecialZ`, `PrisonOfficer`) use the player tables but replace some entries with the `_NP` / `_Np` / `_Tb` animations (standing NPC fires with their own leg animations, loose walking, investigating, hiding behind cover, grenade throws). Zombie types (`Zombie`, `AbnormalZombie`, `BonusCoin`) always use the infected-hand class 39 (`VUM...`) upper names unless the class is the NPC grenade, with the leg set of the held weapon's `LowerAniType`; `ClubZombie` uses class 47 (`VGUM...`). The abnormal boss maps its skills to the EOD names: idle `ilde`, moving `Walk`, rush `Run_Attack_ST/LP/ED`, strafing fire `Attack_L_*`, `Attack_R_*`, stomp `Foot_Attack`, volley `All_Attack`, stagger `Damage_ST/LP/ED`, upper and lower tracker both playing the same name.
- **Rates.** Lower rate 1 and upper rate 1 at each posture change; then, in the aim posture only, an AI skill rate (the AI template's) on both trackers, and a melee-attack rate on the upper tracker. Not needed for a player-driven body.
- **Death.** The record's `UseSpecialDeathAni` if set, else the player choice of 10 (which needs the player death names; `DC` fallback).
- **Aim.** The player aim rotations of 8 need the seven player node names; NPCs that lack them simply do not pitch those nodes.

#### 13.6 Hit spheres, sizes and eye height

**Spheres.** The rule of 11 applies to every body: each entry of the record's table with radius > 0 sits on the node of that name, else on the socket of that name, else is skipped. The tables 2–18 (L = location: 0 unknown, which damage counts as torso (`weapon_firing.md`), 1 head, 2 torso, 3 arm, 4 leg):

| Table | Spheres (radius > 0) | Head spheres (radius, factor) | Other factors | Radii | On its body: node / socket / absent |
|---|---|---|---|---|---|
| 2 (tanker, captains) | 45: L0 2 (`_zN_LowerBody`, `Right_pelvis`), L1 1, L2 4, L3 26 (fingers and rig helpers included), L4 12 | `Head` 5.5, ×3 | all 1.0 | 1–11 | tanker 41 / 4 / 0; captains 38 / 4 / 3 (`Neck`, `Left_shoulder`, `Bip01 R Toe0`) |
| 3 (assassin) | 66: L1 1, L2 12, L3 41, L4 12 | `Head` 5.5, ×1 | all 1.0 | 1–7 | 57 / 8 / 1 (`Head`; the body's head is `Bip01 Head`, so it has **no head sphere**) |
| 4 (pattern boss) | 41, all L0 | — | 1.0; `du_ca` 2.0 | 40–80 | 41 / 0 / 0 |
| 5 (small suit) | 33: as table 0's layout | `Head` 25, `Face_Jaw` 6, `Face_Eyelid1` 10, all ×1 | 0.9 limbs, 0.75 shins, 0.5 feet | 6–60 | 22 / 8 / 3 (`Face_Eyelid1`, `Left_Shld`, `Right_Shld`) |
| 6 (big suit) | 33 | `Face_Eyelid1` 4.6, ×1 (`Head` and `Face_Jaw` are torso here) | as 5 | 4–90 | 22 / 9 / 2 |
| 7 (EOD boss; also `GXL-4209`) | 32: L1 2, L2 5, L3 23, L4 2 | `Bone_AL01` 19, `Bone_AL02` 13, both ×20 (the weak point) | 1.0 | 3–27 | EOD 30 / 0 / 2 (`Bip001 Head`, `Nuts`); `GXL-4209` 19 / 0 / 13 |
| 8 (dummy) | 31 | `Bip001 Head` 6, ×6 | 1.0, 0.9 | 3–9 | body missing |
| 9 (`GXL-4209B`) | 48: 46 L0 with factor 0 (no damage), 2 L1 | `YJ_A_02`, `YJ_B_02` 20, ×1 | 0 | 10–100 | 48 / 0 / 0 |
| 10 (`GXL-X`) | 19, all L0 | — | 1.0 | 2–40 | 18 / 0 / 1 (`Bone004`) |
| 11 (`Mob_Robot`) | 24, all L0 | — | 1.5 `Head`, 1.0 body, 0.75 arms, 0.5 legs, 0.25 toes and fingers | 2–25 | 24 / 0 / 0 |
| 12 (`Boss_Fortress`) | 28, all L0 | — | 1.5 pelvis and head, 0.3–1.0 elsewhere, 0 the root and bullet bones | 5–30 | 28 / 0 / 0 |
| 13 (`Callus`) | 2, L0 | — | 1.0 | 20, 100 | 2 / 0 / 0 |
| 14 (`Beetle`) | 95: 61 L0 ×0.5 or ×1, 34 L1 (the `weak-*` bones, `Head`) ×1.25 | 34 weak points | | 10–120 | 82 / 0 / 13 |
| 15 (`Scorpion`) | 99, L0 ×0.75 | — | | 18–80 | 98 / 0 / 1 (`UNUSE`) |
| 16 (`Thorn`) | 9: 2 L1 ×0.75 (`Scene Root`, `Bone001`), 7 L0 (`Bone007`, `Bone008` ×1.5) | | | 43–53 | 9 / 0 / 0 |
| 17 | 1 (`Dummy010` 40) | — | | | used by no record |
| 18 (the two bugs) | 20, L0 ×1 (the names of table 10 plus `Bip01 Spine1`) | — | | 2–48 | `Bugs_Explosive` 1 / 0 / 19; `Bugs_MeleeHeavy` 2 / 0 / 18 (table and bodies do not match) |

On the M and W NPC bodies, tables 0 and 1 resolve as on the stock bodies (11), less the missing `Nuts` and `Right_arml1` sockets above: 33 spheres on most male NPC bodies, 32 on the `MT_*` and `Special_BC` bodies, 31 on `MT_BM_BODY`, 35 on every female NPC body.

**Hull.** The original uses two rules:

- **As a player body** (the powered suits, 13.7; and what a player-driven NPC body follows): standing half-extents = the dims of the body's `LST` entry and crouched = those of `LC`, if both names exist; otherwise (15, 50, 15) and (15, 40, 15) (`movement.md`, Constants). Eye height above the hull centre = 0.66 × the standing half-height (`movement.md`, Eye height).
- **As an AI**: the dims of the lower tracker's current entry, times the object scale (1), set whenever the lower animation changes (so a running zombie and a walking one can differ).

Per family, with the player rule:

| Family | Stand half-extents | Crouch half-extents | Eye above centre | Root translation stand / crouch |
|---|---|---|---|---|
| M, W | (15, 50, 15) | (15, 40, 15) | 33 | −50 / −40 |
| Tanker, Assassin | (15, 50, 15) (no `LST`/`LC`: fallback) | (15, 40, 15) | 33 | tanker −50 / −50; assassin −50 / −40 |
| Small suit | (80, 100, 50) | (80, 70, 50) | 66 | −100 / −70 |
| Big suit | (128, 150, 70) | (128, 120, 80) | 99 | (0, −150, −40) / (0, −120, −20) |
| EOD boss | fallback (15, 50, 15) | fallback | 33 | −100 (its animations' own dims are (70, 100, 55)) |
| Own-animation bosses | fallback | fallback | 33 | see 13.4 |

The fallback hull of the EOD boss and the bosses does not fit their animations (their root is 100–525 u below the centre while the hull's bottom is 50 below): such a body drawn as a player would sink into the floor (Open question 35).

#### 13.7 The powered suits and the pattern boss

**Powered suits as player bodies** (`Model#` 58–62 and the coloured copies 579–588, 12.4). The suits are ordinary player bodies run by the player driver of 4–8, with a body from 13.4 (small suit for 58–61, big suit for 62), the player hull rule (13.6) and their own weapon classes. Their weapons' `AniType` picks one of the robot classes, whose names (run / walk / crouch / sprint form, as in 6) are:

| Class (`AniType`) | Weapon | Aim | Fire | Select | Reload | Alt-fire |
|---|---|---|---|---|---|---|
| 58 SMG suit | 527 `Powered_Suit_Basic_SMG` | none `USMGRAmS` (walk the same, crouch `USMGCAmS`); F `USMGRAmF` / `USMGWAmF` / `USMGCAmF` / `USMGRRAmF`; B `USMGRAmB` / `USMGWAmB` / `USMGCAmB` / `USMGRRAmF`; L **and R** `USMGRAmL` / `USMGWAmL` / `USMGCAmL` | none `USMGRFiS` / `USMGWFiS` / `USMGCFiS`; F and B `USMGRFiF` (sprint `USMGRRAmF`); L and R `USMGRFiL` / `USMGWFiL` / `USMGCFiL` | standing `USMGRSSeS` (crouch `USMGCSeS`, sprint `USMGRRSeS`); moving `USMGRSeS` / `USMGWSeS` | standing `USMGRSReS` (`USMGCReS`, `USMGRRReS`); moving `USMGRReS` / `USMGWReS` | the aim names |
| 59 pistol suit | 529 `Powered_Suit_Basic_Pistol` | as 58 | none `UPRFiS` / `UPWFiS` / `UPCFiS`; F, B `UPRFiF` (walk the same, crouch `UPCFiS`, sprint `USMGRRAmF`); L, R `UPRFiL` / `UPWFiL` / `UPCFiL` | as 58 | none `UPRReS` / `UPWReS` / `UPCReS` / `UPRRReS`; moving `UPRReL` / `UPWReS` | as 58 |
| 60 sniper suit | 526 `Powered_Suit_Basic_SR` | as 58 | none `USNRFiS` / `USNWFiS` / `USNCFiS`; F, B `USNRFiF`; L, R `USNRFiL` / `USNWFiL` / `USNCFiL` | as 58 | none `USNRReS` / `USNWReS` / `USNCReS` / `USNRRReS`; F, B `USNRReF`; L, R `USNRReL` / `USNWReL` | as 58 |
| 61 melee suit | 530 `Powered_Suit_Basic_Melee` | `UMERAm*` as 58's pattern (`UMEWAm*`, `UMECAm*`, `UMERRAmF`) | none `UMERFiS` / `UMEWFiS` / `UMECFiS`; F, B `UMERFiF`; L, R `UMERFiL` | `UMERSSeS` / `UMERSeS` pattern | `UMERSReS` / `UMERReS` pattern | `UMERFiS1`, `UMERFiF1`, `UMERFiL1` (and `W`, `C` forms) |
| 62 medic suit | 532 `Powered_Suit_Basic_Medic` | as 61 | none `URERFiS` / `UREWFiS` / `URECFiS`; F, B `URERFiF`; L, R `URERFiL` | as 61 | as 61 | the 61 aim names |
| 63 big-suit MG | 528 `Powered_Suit_Boss_MG` | `UMGRAm*` in 58's pattern | `UMGRFiS` / `UMGWFiS` / `UMGCFiS`; F, B `UMGRFiF`; L, R `UMGRFiL` | `UMGRSSeS` / `UMGRSeS` pattern | `UMGRSReS` / `UMGRReS` pattern | aim |
| 64 big-suit launcher | 531 `Powered_Suit_Boss_Launcher` | 58's names (`USMG...`, which the big suit lacks) | `UFGRFiS` / `UFGWFiS` / `UFGCFiS`; F, B `UFGRFiF`; L, R `UFGRFiL` | 58's names (absent) | none `UFGRReS` / `UFGWReS` / `UFGCReS` / `UFGRRReS`; moving `UFGRReL` (walk the same, crouch `UFGCReL`) | aim |
| 66, 67 NPC suit SMG, melee | 537, 538 (no holder, 12.4) | as 58 / 61 | the `_AI` copies (`USMGRFiS_AI`, `UMERFiS_AI`, …) | as 58 / 61 | | 67: `UMERFiS1_AI` … |

Directions group as in 6 (F with both forward diagonals, B with both backward ones), but the robot classes give **right the left-strafe names** (`USMGRAmL` for both; the suits' `USMGRAmR` and `UMERAmR` entries are never played; Quirk 15). Hits are `D_HIT` / `D_HITT`. On the big suit the launcher's aim and select names come from the SMG class and do not exist, so a launcher-holding big suit keeps its previous upper animation in those postures (6, Quirk 1). Both suits' leg libraries lack the diagonal and sprint-strafe names (13.4): moving diagonally, the legs keep the previous animation (Quirk 1 again). Weapons mount on `RightHand` as for players (the `HHModel` of 526–532, `HH_*_Powered_Suit_Basic_*_SH.ltb`); second guns would use `LeftHand`.

**The HQ raid boss** (`PatternFX0` `HQRaid_Boss`, in the effect file; placed as a `PatternObject` in NemexisHQ and NemexisLab_CP). Not a character: a flying object (a dragon-like gunship).

- **Model**: `CHARS_M_BODY\boss_model.ltb`, skin `CHARS_T_BODY\Boss_SD.dtx`, render style `RS\default.ltb`, scale 1, model radius 500. 66 nodes: the body (`du_boss_main`), two wings (`du_wing_left/right` with `da_wing_*` segments), lights, smoke points, six missile pods `du_missile_1..6`, the guns `du_firegun_01` and `du_minigun_01`, an `eye`, a back wing, seven bomb bays `du_boom_1..7`, and the hit bones `da_body_*`, `da_head_*`, `da_tail_*`. No weight sets: one main tracker only.
- **Animations** (`boss_anibase.ltb`, all 200 ms interpolation, dims (450, 200, 500), root 0): `idle` 333; and for each of `left`, `right`, `up`, `down`: `<dir>_start` 333 (334), `<dir>` 133–134, `<dir>_end` 500. The object plays them non-looping on the main tracker and looks again only when the current one is done: after `<dir>_start` comes `<dir>`, which then holds its last key; `<dir>_end` plays when the move is about to end (its length before the move's end time); any other animation is followed by `<dir>_start` of the current state. Hovering uses `idle` for all three, so it repeats.
- **Weapons** are not models: each fires from sockets. `Weapon0` 460 (`HQRAID_BOSS_MINIGUN`) from `highgun` (on `du_minigun_01`, 113.484 forward); `Weapon1` 462 (missiles) from the sockets `Missile_1` … `Missile_6`; `Weapon2` 463 (poison bombs) from `bomb_1` … `bomb_7`. Rule: if a socket with the listed name exists it is the only one; otherwise the names `<name>1`, `<name>2`, … are taken in order until one is missing. A shot leaves the socket's position along the socket's **negative** forward axis (its sockets are rotated 180° about up). `[PatternWeapon#]` gives the fire type per weapon: 0 random socket (460), 1 sockets in order (462), 2 all sockets at once (463). 461 (flame) has a fire pattern but no `WeaponN` entry, so it is never mounted.
- **Size**: the object's box and hit box are the animation dims (450, 200, 500); its hit table is skeleton 4 (13.6). Effects ride on sockets `light_left/right/back`, `smoke_left/right` (`AttachFX`), `Breach_left/right/center` (damage effects at 75, 55, 25 % health), `Flash_eye`, `Lowgun`.

`AIModel` 7 `SH_Sandhog` also names `boss_model.ltb` (with skeleton 1); no world places it.

#### 13.8 A player-driven NPC body

What a playable character wearing an NPC or boss look needs, per family. "Driver" is the player driver of 4–8 run unchanged on the body's own library.

- **M and W (814 NPCs).** Everything of 2–11 applies as for a player body: same trackers (crouch tracker included), names, rates, aim nodes, sockets, death names, hull and eye. Differences: skins from `Skin#` (13.2) with no team mark slot (an NPC body shows no friendly mark); face, head and goggle from 13.2; scale 1. The NPC guns (12.4) use ordinary player classes: `AniType` 0 rifle (62, 65, 67, 417, 419, 420, 560, 561, 858), 3 SMG (63, 416, 562), 5 sniper (64), 19 `RW` launcher (66, 462), 11 grenade (68, 69, 463), 25 P90 (418), 2 `RH` rifle (458), 16 `BK` knife (457), 27 double barrel (459), 28 chain gun (460), 40 flamethrower `UFS` (461), 10 pistol (421), rifle also 863, grenade also 864; every one of these classes' names exists in both libraries.
- **Tanker (67) and Assassin (11).** Driver with the infected leg set (`V` prefix) and the infected-hand upper class 39 (`VUM...`) **whatever the held weapon's class**, as the original's zombie NPCs do (the bodies have no rifle, pistol or other class animations: with any other class nothing would play and the body would stay in its first pose). Hits use `VD_HIT` / `VD_HITT` (the bodies have no `D_HIT`); death always `VDIE` (the records' special death; the assassin also has `VDIE1`). Not in the tanker library, so they keep the previous animation: standing jump `VLSJ` (the library has `VLJS`, a misnamed copy; an implementation may map the standing jump to it, flagged), sprint strafes `VLRRL` / `VLRRR` (it has `VLRRFL` / `VLRRFR`), standing draw `VUMRSSeS` and sprint draw `VUMRRSeS`, the second hit `VD_HITT`. The tanker's crouch leg animations keep the root at −50, so a crouching tanker's feet stand 10 u below the crouched hull's bottom. Captains: aim pitch skips `Neck` and `Left_shoulder` (their nodes have other names), and a dropped weapon spawns at the right toe (Quirk 14).
- **Small suit (18 NPCs) and big suit.** The player suits' own rule (13.7): driver with the robot classes. An NPC suit holding a non-suit weapon (the world gives 35 and 36 `HQRAIDNPC_2_SMG`, class 2) has no matching upper names: map the holder to the SMG-suit class 58 (melee record 35: class 61) for animation, and mount the held gun's `HHModel` on `RightHand` (flagged: the data can't say how such a gun should look on a suit hand. In the original the NPC suits hold 458 with player tables (13.5), find no `URH...` names in the suit library and so never change their upper animation; the `_AI` fire names belong to classes 66 and 67, whose weapons 537 and 538 no world gives out; Open question 36).
- **EOD boss (3).** No player names at all. Minimal mapping onto its names, both trackers playing the same name, plus the looping `AL` tracker: standing → `ilde`; any movement, every direction → `Walk` (rate 1); fire held → `Attack_L_LP` (or `_R_LP`) looping while held, `_ST` before and `_ED` after (the boss's strafing fire); alt-fire or melee → `Foot_Attack`; hit → `Damage_ST`; death → `Death`. No crouch, jump, draw, reload or aim pitch; its guns mount on `RightHand` (`Dummy003`) and `LeftHand` (`Dummy001`), its world weapon (858) on both. Hull per 13.6 (fallback). Flagged: the data supplies no aim pose, no backward or strafe walk and no jump.
- **Own-animation bosses (34).** Minimal mapping: standing → `Idle`; moving → `Walk` (`GXL-4209B`: `Walk`; `Thorn`, `Callus`: none, stay on `Idle`); fire → `Attack` (`Boss_Fortress`: `Gatling_ST` / `_LP` / `_ED`; `Mob_Robot`: `Laser`; `Scorpion`: `Attack`; `Beetle`: `Attack`; `Callus`: `Breed`; `Thorn`: `AppearAtk_ST` / `_LP` / `_ED`; `GXL-4209B`: `Strafe`); melee → `Melee` where it exists; death → `Die`. Both trackers can play the same name (the `Lower` set is empty, so the upper tracker's pose is the whole pose). Weapons mount only on `Boss_Fortress` (`Gun_L`, `Gun_R`), `Mob_Robot` (both sockets on the left forearm) and `Scorpion` (`LeftHand` on the body). Flagged: no aim, crouch, jump, strafe, draw or reload animations; hulls far larger than a player's (13.6); several bosses are not upright walkers (`Thorn` rises from the ground, `Callus` does not move).
- **Pattern boss.** Not wearable as a walking body (no legs, no character animations, a 900 × 400 × 1000 u box). If used, the only mapping the data gives: hovering → `idle`; strafing left or right → `left_*` / `right_*`; rising (jump) → `up_*`; sinking (crouch) → `down_*`; firing from its sockets as above. Flagged.
- **Dummy** (skeleton 8): its body file is not in the install; not usable.

## Per-tick order

**Server, per client movement frame received** (each frame of the client's update packet, in sequence):

1. Apply the frame's control flags, input-allowed flag, third-person flag, sprint state, firing and alt-firing flags, zoom, dot sight and bomb-planting flags.
2. Animation: weapon class from the current weapon; posture and movement from the statuses and flags (5); then pick, start, sync and weight the animations (7). Skipped (and the state just reset) while dead or frozen.
3. Movement for that frame (`movement.md`).

**Server, each server update:** recompute the weapon class, posture and movement values from the latest flags (no animation start), apply the aim rotations' inputs (pitch, twist), and, if dead, set the death animation and create the body once (10).

**Client, own character, each movement tick (15 ms)** (corrected: earlier drafts said "each frame"): control flags → crouch rate update → the same animation driver on the client's copy of its own model (only while the client drives its own object) → hull dims → movement. Engine animation advance happens once per rendered frame (`animation.md`); aim rotations are applied when node transforms are next evaluated.

**Client, other characters, each frame:** tracker states come from the server; the client takes the latest pitch and twist (from the update data, or from the aim messages on the legacy path), applies the hit kick (9), and sets the aim rotations; hit spheres are evaluated from that pose when a ray test needs them.

## Edge cases

- **Switching weapons mid-reload:** the client reports "select"; the select branch overrides the reload posture on the next update, so the new weapon's select animation plays at once (class changed, so the upper animation restarts). The reload itself is abandoned as `weapon_firing.md` says.
- **Firing during reload or draw:** while the weapon status is reloading or selecting no firing choice is made; the upper body keeps the reload or draw animation. The fire pose starts only after the status clears.
- **Firing while moving with a class whose moving fire names are missing** (the rifle class in the US libraries): no fire animation plays; the aim animation for the direction keeps playing (Quirk 1).
- **Dying crouched:** the crouched death animations are never chosen; a standing death animation plays from the crouched pose (the body copies the dead player's crouched hull dims).
- **Dying in the air:** the player is snapped to the floor first, so the body never falls; if the player was in the jump state the body is turned to face away from the shot and c = 0 is used.
- **Sprint with a direction that has no sprint leg animation** (backward, pure strafe): the lookup fails, the driver returns without starting anything or recording the state, and the previous animations continue (Open question 4).
- **Jumping while holding crouch:** the jump branch is skipped; normal locomotion with the crouch tracker plays in the air.
- **Walking off a ledge:** no "jumped" status, so the run or walk animation keeps playing while falling.
- **A model missing a socket:** weapon (`RightHand`), second gun (`LeftHand`), goggles (`Eyes`) or backpack (`1W_N`) are not attached; the name tag falls back to the object position (`hud_crosshair.md`); a missing hit-sphere socket or node removes that sphere; a missing `Left_foot` node has no fallback in the reference (`inventory.md` gives ours).
- **A missing animation name** anywhere (lower, crouch, upper, death): nothing new is started; death falls back to `DC`.
- **A model without the `Lower` or `Upper` weight sets:** those trackers cannot be enabled and keep no weight set; with the engine's rule that an extra tracker without a valid set freezes the pose (`animation.md`, Quirk 5) the character would not animate. Every stock player body has both.
- **Changing body mid-life** (cosmetic swap, infection): all trackers are removed and re-added, the tables are looked up for the new file, and the first update starts all three animations unconditionally.
- **Shots at corpses:** client shots pass through bodies (10, step 3); a player standing behind a fresh corpse can be hit through it.

## Quirks

1. **Missing names keep the old animation.** A name absent from the library is dropped, and an entry whose run name is absent starts nothing. With the US libraries the M4A1 class shows no fire animation while moving, and SMG-H, pump-shotgun and machine-gun classes show nothing new at all. Keep (it is the shipped look); an implementation must not substitute another animation.
2. **Players are never "crouching" to the animation driver.** Crouch exists only as the crouch tracker, so the upper body never plays its `...C...` crouch animations and the crouched death animations never play. Keep.
3. **Sprint jump is unreachable.** The test for it requires the last direction to be forward, left-forward and right-forward at once, so `LRRJF` / `LRRJL` / `LRRJR` never play; a sprinting jump plays `LJJ`. Keep.
4. **Hit status without damage sticks.** If the "hit" status arrives while the character has no recorded damage (or is spawn-protected), no hit animation starts and the status is not cleared, so the firing choice stays suspended (aim pose) until another status replaces it. Keep, but see Open question 12.
5. **Front-left body hits use the head animation.** A front side hit from the left plays `DLSH` whether or not the head was hit (from behind it is `DLT`). Keep.
6. **The coin is reseeded every death** from the clock in seconds, so deaths within the same second pick the same variant. Keep the behaviour (or use any RNG; nothing visible depends on it beyond correlation).
7. **The crouch lowers the female head more than the hull.** Fully crouched with the rifle aim, the default female's head node drops 25.2 u (from 80.6 to 55.4 above the feet) while the hull top drops 20 u; the default male's drops 18.1 u (79.7 to 61.6). The female upper-face spheres follow (82.8 → 57.2). Keep: it changes head hitboxes.
8. **Spheres outside the box.** The left hand and forearm sit far forward of the hull: standing with the rifle, the left hand sphere's front reaches z = 31.5 (28.3 + 3.2), past the 2×-widened first-test box (half-depth 30) of a character facing along a world axis; crouched it reaches 33.8. The parts outside the box cannot be hit. Keep.
9. **The head node sphere is tiny and low.** On the male the head-location spheres are a 1.0 u sphere at the base of the skull (`Head`), the 3.0 jaw and the 4.6 upper-face socket, which sits 7 u behind the eyelid node inside the head. Keep.
10. **Server and client aim poses differ** (the server does not pitch `Torso`), so the server's own sphere tests see arms and head pitched less than drawn. Keep, and see Open question 5.
11. **Upper and lower trackers resync on every movement change** (the torso's time is set to the legs'), which restarts the aim cycle when the legs change animation. Keep.
12. **A female body on the male hit table.** `AIModel` 23, 29 and 31 pair `Light_W_vest.ltb` (the female tree) with table 0: only 28 of the 35 spheres exist (no `Neck`, `Head`, `Face_Jaw`, `Right_shoulder`, `Nuts`, no 7.0 shoulders), the head left with `Face_Eyelid1` alone. No world places them. Keep the data rule (11) if they are ever used.
13. **The Shining has no weight sets.** `CW_VST_TheShining.ltb` (player `Model` 513 and `AIModel` 100) stores none; the trackers' sets cannot be found, which by `animation.md` (Layered playback, Quirk 5) leaves no pose. Its tree and library are the assassin's, so use `VIW_F_NM_DF_Assassin_CH.ltb`'s `upper` and `lower` sets for it (Open question 32).
14. **Captain node names.** The two captain bodies share the tanker's tree and library but name node 9 `Bip01 L Clavicle`, node 37 `Bip01 L Foot` and node 42 (the right toe's place) `Left_foot`. So the aim rotations skip their left shoulder and neck, table 2's `Neck`, `Left_shoulder` and `Bip01 R Toe0` spheres do not exist, its `Left_foot` sphere sits on the right toe, and a weapon dropped "at `Left_foot`" appears at the right toe. Keep (by name, as the data says).
15. **Robot classes strafe right with the left animations.** The powered-suit classes 58–64 give the right direction the left names (`USMGRAmL`, `USMGRFiL`, `UMERFiL` …); the suit libraries' `...R` entries never play. Keep.

## Corrections (2026-10-22)

From a comparison pass over the reference's shared third-person driver and its client and server feeds, the player object's death, body and model-skin code and the client character effect, against this spec and our implementation (`games/combat_arms/src/game/characters.rs` and `characters/`, at the commit this section was written on). Ordered by what a player notices. Where a correction contradicts a section above, the correction wins.

1. **A held fire button does not repeat the third-person fire animation** (corrects 7, "Starting an animation", the sentence "this is how a fire animation repeats once per shot cycle while firing", and Test case 12).
   - Reference: the upper tracker is only (re)started when the class or posture changed, or the chosen upper name differs from the last one started while the posture is aim, fire or alt-fire. The restart-when-done rule belongs to that start; it is never reached while posture, class and name stay the same. So with the fire button held continuously (the server's raw flag), the fire animation plays once, then the upper tracker holds its last frame until the posture or name changes (button released for at least one driver update, a move that changes the name, a reload, a draw, a hit). The legs, by contrast, are started every update and do restart when done (looping names never are).
   - Ours: `characters/driver.rs:737-744` calls the start for aim, fire and alt-fire postures every update with restart allowed, so a finished `URFRFiS` (and `UKRFiS`, `USNRFiS`, ...) restarts on the next update while fire is held.
   - Expect: M4A1, standing, fire held 1000 ms on the server copy: upper `URFRFiS` starts once at t = 0, reports done at 101 ms and is still `URFRFiS` at its end time at 1000 ms (no second start). Release for one update and press again: it starts again from 0. Knife held 2 s: `UKRFiS` once, held at its last frame. Running forward with fire held keeps `URFRAmF` looping (unchanged).

2. **A front-left body hit plays `DC`, not `DLSH`** (corrects 10, the "front, side: left" bullet, Quirk 5 and Test case 38).
   - Reference: in the front-side-left branch, the head case picks `DLSH`, but the non-head case looks up `DLSH` and then returns the character's cached handle for `DLT`. That cache is cleared every time the player is (re)spawned and is only filled by a back-side-left body death of the same life, which cannot have happened, so the handle is always unset and the "missing animation" fallback plays `DC`.
   - Ours: `characters/death.rs:107` returns `DLSH` for every front-left side hit.
   - Expect: bullet death, d = (0.94, 0, −0.34) (from the left, slightly in front), last sphere `Upper_torso` → `DC` (1900 ms, US male); same d with `Face_Jaw` → `DLSH`; d = (0.94, 0, 0.34) unchanged (`DLT` / `DLSH`); right-side cases unchanged.

3. **Walking off a real drop shows standing legs in the air** (corrects Edge cases, "Walking off a ledge").
   - Reference: on any movement tick where the client did not jump, is not already falling, is not on a ladder, in a free-movement volume or spectating, and the footing test of `view_models.md` 6 fails (no floor within 150 u below the hull centre, i.e. 100 u below the standing feet), the client marks itself falling and sends the "falling" motion status. The driver then sets nothing for the legs (walk, no direction: `LST`) and the upper body uses the no-direction aim until the "landed" status arrives. Drops shallower than 100 u below the feet never fall, so the run legs continue there.
   - Ours: the driver's jumped/landed come only from the jump flag (`characters.rs:832-833`), so walking off any ledge keeps `LRF` (and `URFRAmF`) through the fall.
   - Expect: run forward off a 300 u drop: from the first tick with no floor within 150 u below the hull centre until the landing tick, lower `LST`, upper `URFRAmS`; after landing `LRF` / `URFRAmF` again. Off an 80 u step (80 u below the feet): `LRF` throughout.

4. **The sniper and land-mine fire pose follows the shots, not the button** (corrects 5 step 4, first bullet, and the "Fire held" paragraph for these two classes).
   - Reference: for classes 5 and 20 the per-update choice is called with "not firing"; the fire posture comes only from the server's per-shot fire notice (each shot the server accepts, and each throw notice), which sets the posture to fire for the next update. It is kept while that fire animation plays and becomes aim once it is done. The held button plays no part.
   - Ours: `characters/driver.rs:613-619` uses the held fire flag (the raw one on the server), so holding the trigger of a bolt-action sniper keeps the fire posture between shots (and, with item 1's restart, replays the fire animation continuously).
   - Expect: bolt sniper, trigger held 3 s, shots accepted at 0 and 1.5 s: upper fire animation starts at 0 and at 1.5 s (one start per shot), aim (`USNRAmS`) in between once each fire animation is done.

5. **No fire pose while sprinting** (missing from 5; the build defines the option that disables firing poses during sprint, in the US, EU and BR configurations alike).
   - Reference: after the fire / alt-fire choice, if the lower tracker is still playing the sprint leg animation (last movement sprint, not done; sprint legs loop, so: while sprinting), the posture is forced to aim.
   - Ours: `characters/driver.rs:629-637` keeps fire or alt-fire while sprinting.
   - Expect: sprint forward with the fire flag held (the server's raw flag) → posture aim, upper `URFRRAmF` **looping**; the last posture recorded is aim, so a direction change during the sprint copies the legs' time to the torso (7 step 3).

6. **The driver's "ammo count 0" is the weapon's total ammunition, not the clip** (corrects 5 step 4, last bullet).
   - Reference: both the client's and the server's value is the player's whole ammunition count for the current weapon (the reserve pool that includes the clip).
   - Ours: `characters.rs:830` uses the clip.
   - Expect: M4A1 with clip 0 and 90 in reserve, fire flag held before the reload status arrives → posture stays fire (item 1: held at the last frame), not aim. With total 0 → aim once the upper animation is done.

7. **Crouch in the air clears the jump and fall status** (corrects 5, "Movement").
   - Reference: when the motion status is jumped or falling and the crouch key is held, the status is reset to none and the normal locomotion of the keys is used; it does not come back when crouch is released in the air (only a new "jumped" sets it).
   - Ours: `characters/driver.rs:650-660` keeps "jumped" while crouch is held (so releasing crouch mid-air brings the jump legs back) and plays the falling `LST` whatever the crouch key.
   - Expect: jump forward at t = 0, crouch held from 200 to 400 ms, still airborne at 400 ms: legs `LRF` (run forward, crouch tracker by r) from 200 ms on, and no `LJJ` after 400 ms. Walking off a drop (item 3) then pressing crouch: locomotion legs at once.

8. **Forward and backward held together give backward** (corrects 5, "Direction from the keys").
   - Reference: the keys are applied in order forward, backward (overrides), right strafe, left strafe; within a strafe the forward key is tested before the backward one.
   - Ours: `characters/driver.rs:95-96` lets forward win.
   - Expect: forward + back → `LRB`, `URFRAmB`; forward + back + right → `LRFR`; forward + back + left → `LRFL`; left + right → left (unchanged).

9. **An emote plays on the upper body only; the legs return at once** (answers Open question 20's open detail, as read; corrects 9 "Playing").
   - Reference: the emote is put on both trackers once, but the driver keeps running on every client movement frame and starts its leg animation each time, which replaces the emote on the lower tracker on the next movement frame. The upper tracker keeps the emote because the driver's upper name and posture have not changed. When the emote ends, nothing restarts the upper tracker either: it keeps the emote's last pose until the driver next starts an upper animation (a posture, class or upper-name change).
   - Ours: `characters/driver.rs:700-716` holds both trackers on the emote and restarts the trackers when it ends.
   - Expect: Hunted, standing still, `Insult_Dance`: lower tracker back on `LST` within one movement frame; upper on `Insult_Dance` until done (weapon hidden); after it ends the upper still shows the emote's final frame until the player moves, fires, reloads or switches. Check live (Open question 20's measurement).

10. **A death "in a jump" means the jump legs, and only for non-blast deaths** (corrects 10, the jump bullet).
    - Reference: the jump test is the driver's movement being the jump (status "jumped", crouch key up, the jump leg animation not yet done: the first ~667 ms of a jump), not "airborne". The coin is forced to 0 and the body turned only in the non-blast branch; explosion, claymore and land-mine deaths are never turned.
    - Ours: `characters.rs:611` uses the airborne flag for the whole flight and `characters.rs:637` turns the body for every kind.
    - Expect: bullet death 300 ms after a jump starts → turned away from the shot, c = 0; 800 ms after (falling) → not turned, coin as drawn; grenade death 300 ms after → not turned.

11. **No team mark at all in Quarantine** (2, "Team mark": the infection mode is the Quarantine game type).
    - Reference: in Quarantine every character's slot 2 is mark 0 (transparent), teammates included, on the client and on server-made bodies.
    - Ours: `characters/models.rs:301` always passes "not infection", and `characters.rs:1024` gives teammates the tie in every mode.
    - Expect: Quarantine, a teammate's slot 2 = `CHARS_T_BODY\Alpha.dtx` (nothing drawn); any other team mode = `RED_Tie.dtx`.

12. **The corpse is a separate object that outlives the respawn** (10; restates for implementers a shortcut that players see).
    - Reference: at death the player object itself is made invisible and loses its attachments; the body object shows the death animation at the death spot and stays until 5 s after its owner respawns in the common modes (500 s in Combat, ESL and clan-default modes).
    - Ours: bodies (`CaBody`) are not drawn; the dead player's own model plays the death animation, so the corpse vanishes the moment its owner respawns.
    - Expect: TDM, death at 0 with `DT`, respawn at 10 s → one body model visible at the death spot until 15 s while the respawned player is drawn at its spawn.

13. **Missing crouch names fall back to the leg animation** (4 and 6, minor).
    - Reference: if the leg set has no crouch name for the direction, the crouch tracker is given the lower tracker's animation instead of being left alone.
    - Ours: `characters/driver.rs:726-729` starts nothing.
    - Expect: a library without `LCFR`: crouch tracker plays the lower name (`LRFR` or `LWFR`) with weight r.

## Corrections (2026-10-30)

From the shared model-attribute reader's skin assembly and the client's equipment and cosmetic screens. Answers "Body skins" 1 to 4 of the Combat Arms plan (render styles: `models_ltb.md` "Render styles and sprite skins (added 2026-10-30)").

1. **Additional skins and model colour** (2, texture slots).
   - Reference: when the record has any `AdditionalSkins#`, slots 5, 6, … get `AdditionalSkins0`, `AdditionalSkins1`, … in order, the same for every camouflage index and figure type (nothing indexes them); the record's `AdditionalRenderStyles#` likewise fill render-style slots 5, 6, …. The **model colour** index m (−1 = none, at most 5 lists, 0..4) is a per-player value: the colour the player chose for the equipped cosmetic **character** item, overridden by the equipped cosmetic **vest** item's colour if one is equipped (items processed in that order), stored per owned item on the account and sent to the other clients with the player's model information. Default −1. When m ≥ 0, each of `ColorSkin{m}_UpperSkin#`, `ColorSkin{m}_LowerSkin#`, `ColorSkin{m}_AdditionalSkins#` and `ColorSkin{m}_HandsSkin#` that has at least one entry replaces its base list (upper and lower still indexed by the camouflage index c; additional skins by k); an empty colour list leaves the base list. The `Ghillie_Suit_Color_A/B/C` records are separate model records; their colour comes from their own lists, not from m.
   - Ours: slot 5 + k from `AdditionalSkins{k}` (matches); no model colour (matches the default −1).
   - Expect: test cases 58 and 59.

2. **The ghillie leaves and other `RS\MARKTRANSPARENT.LTB` pieces** are **alpha-tested**, not blended: a texel is drawn (at its full texture colour, unlit) when its alpha is at least 160/255 and discarded otherwise; no sorting is needed.
   - Ours: blended by the texture's alpha, sorted per piece.
   - Expect: test case 60.

3. **A body record whose file is missing** (plan "Body skins" 5; Steam build 26236938, from the engine's model-file loading on client and server). `Model279` `Medium_NM` and `Model280` `medium_NM_female` name `CHARS_M_BODY\CM_Body_Medium_vest_KE.ltb` and `CHARS_M_BODY\CW_Body_Medium_Female_KE.ltb`; build 26236938 still ships neither (the body archive's other `_KE` bodies are there). When a model object's file does not load, the engine loads `models\default.ltb` in its place (both the server's and the client's model set-up do this), so such a character is drawn as that placeholder: one untextured piece of 52 triangles about 6 u across with one animation `x` (6566 ms, dimensions (3, 3, 3)), not as a body (whether anything earlier in the character set-up avoids such a record was not traced).
   - Ours: a rung that draws one of these two records keeps the body its player already wore.
   - Expect: test case 61.

## Test cases

"Install" marks values that depend on the character archives: they were measured on the US reference client 1501.04 and must be re-checked on the Steam install, build 25595979, after decryption of `CHARS_M_BODY.rez` (the Steam body files have the same sizes as the US ones, 429,442 and 237,832 bytes, but the Steam libraries differ: `ST_M_CHILD.LTB` 66,900,324 vs 63,225,013 bytes, `ST_W_CHILD.LTB` 95,232,495 vs 91,210,809). "Data" marks values from the US attribute data set. Positions in u, model space, left-handed; tolerance 0.01. The "stand" pose is the layered pose of 4 at t = 0 for every tracker: main entry 0, upper `URFRAmS`, lower `LST`, crouch `LC` with multiplier 0; "crouch" is the same with multiplier 1.

**Model choice (data)**

1. Selection model 0, face 0 → body `CHARS_M_BODY\CM_VST_NM_DF_Nuts_JS.ltb`, skeleton 0, figure type 0. Same with face 5 → model 7, `CHARS_M_BODY\Nomal_W_vest.ltb`, skeleton 1, figure type 1. Model 2 with face 4 → model 9 (`Heavy_W_vest.ltb`).
2. Missing or out-of-range model id → model 0. Head piece 0 → no head model.
3. Model 0, camouflage index 3 → slot 0 `CHARS_T_BODY\Nomal_vest_body_UR.dtx`, slot 1 `CHARS_T_BODY\Nomal_vest_leg_UR.dtx`; index 9 → `Nomal_vest_body.dtx`, `Nomal_vest_leg.dtx`.
4. Team mark on a client: a teammate's slot 2 = `CHARS_T_BODY\RED_Tie.dtx`; an enemy's = `CHARS_T_BODY\Alpha.dtx`; any character in the infection mode = `Alpha.dtx`.
5. Head piece `Combat_Helmet` (Hair2): skeleton 0 → `CHARS_M_HEAD\Combat_Helmet.ltb`, skeleton 1 → `CHARS_M_HEAD\W_Combat_Helmet.ltb`. Goggle0 on figure type 1 → `ATTACH_M\W_Glass.ltb` on socket `Eyes`.

**State → animation (install for names and lengths; rates data)**

6. M4A1 (class 0), standing still, run flag on → lower `LST` (looping, rate 1), upper `URFRAmS` (looping, rate 1), crouch tracker `LC` at weight r.
7. Walk key and forward → movement walk → lower `LWF`, upper `URFWAmF`. Same with no direction → upper `URFRAmS` (the walk name is missing).
8. Run forward-left → lower `LRFL` (600 ms), upper `URFRAmF`. Backward-right → `LRBR`, `URFRAmB`. Strafe right → `LRR`, `URFRAmR`.
9. Sprint forward → lower `LRRF` (500 ms), upper `URFRRAmF`. Sprint right-forward → `LRRR`.
10. Crouched (r = 1) and still → lower `LST`, crouch tracker `LC` at weight 1, upper `URFRAmS`. Crouched moving forward → lower `LWF` (walk, run flag off), crouch `LCF` (1400 ms), upper `URFWAmF`.
11. Jump from standing → lower `LSJ`, non-looping, 666 ms; it reports done after 667 ms; next update the status is falling and the lower animation becomes `LST`. Jump while running forward (direction forward last update) → `LJJ`. Sprinting jump → `LJJ`, never `LRRJF`.
12. Fire held, standing, M4A1 → upper `URFRFiS` (100 ms, non-looping); while fire stays held it restarts on the first update after it reports done. Fire held while running forward → the upper animation stays `URFRAmF`.
13. Reload standing → `URFRSReS` (1866 ms); reload started while moving → `URFRReS`, kept even if the player stops. Draw standing → `URFRSSeS` (733 ms); moving → `URFRSeS`; sprinting → `URFRRSeS`.
14. Pistol (class 10) fire while running forward → `UPRFiF`; knife (14) fire → `UKRFiS` (500 ms) in every direction; knife alt fire → `UKRFiS2` (800 ms).
15. Grenade (class 11): at rest press fire → `UTRFiS_A` (267 ms); when it is done → `UTRFiS_B` (1 ms); fire released → `UTRFiS_C` (900 ms); when done (status "none") → `UTRAmS`; if the client has meanwhile reported "select" (next grenade drawn) → `UTRSSeS`.
16. Damage received, not in spawn protection → upper `D_HIT` (467 ms, 200 ms blend) or `D_HITT` (400 ms, 200 ms blend), 50 % each; fire held meanwhile → no fire animation until it is done.
17. Rates: any upper animation with weapon data lacking `HHAniFrame_n` → 1.0. A weapon with `HHAniFrame_5` = 2300, reload rate 1, third-person reload length 1866 → upper rate 0.8113 (lasts 2300 ms). The legs play at 1.0 at any speed.

**Aim (data for shares; install for positions)**

18. Twist 45° → `Torso` turns 18.53°, `Upper_torso` 26.47° more (chest total 45°); neck, head and shoulders add nothing.
19. Pitch shares: arms total exactly φ up and down; head 1.05 φ up, 1.25 φ down. Server: arms 0.85 φ up, 0.65 φ down.
20. Pitch 60° down → clamped to 45°. Byte transport: 30° → byte 42 → 29.764°; −45° → −63 → −44.646°.
21. Default male, stand pose: no aim → `Head` (5.66, 29.72, 9.65), `RightHand` (12.54, 22.90, 19.93); pitch +30° (down) → `Head` (5.66, 24.68, 16.72), `RightHand` (12.54, 13.34, 22.19), `Face_Eyelid1` (6.62, 28.50, 19.96); pitch −30° (up) → `Head` (5.66, 30.98, 6.23); twist +45° → `Head` (10.30, 29.72, 3.59), `RightHand` (22.43, 22.90, 6.00); server, pitch +30° → `Head` (5.66, 27.45, 13.02).
22. Default female, stand pose, pitch +30° → `Bip01 Head` (2.95, 24.74, 17.70), `Headfuck` (4.07, 25.08, 20.87).

**Sockets (install)**

23. `RightHand` → parent `Right_hand`, offset (7.085, 1.661, 1.277), rotation (0.0573, −0.7151, −0.0818, 0.6918); male bind (35.69, −1.14, 4.24); male stand (12.54, 22.90, 19.93).
24. `NAME_socket` → `Head`, (16.5, 0, 0.906); male bind (0.00, 47.97, 3.05), `LST` 0 (9.70, 45.02, 13.52), stand (8.63, 45.87, 11.51), crouch (6.13, 37.57, 15.21). Female (`Bip01 Head`): bind (0.08, 105.45, 5.78), stand (9.28, 43.71, 19.18). A tag over a male standing at object position P is anchored at P + (8.63, 45.87, 11.51) rotated by the object's yaw.
25. `Eyes` → male `Face_Eyelid`, (3.011, 0, 1.314), bind (0.03, 41.38, 5.68); female `Bip01 Head`, (−85.255, 0.001, 18.748), bind (0.08, 2.14, 5.20).
26. `1W_N` → `Upper_torso`, (−18.328, 0, 0), rotation (0, 0, −0.7071, 0.7071); male bind (0.00, −3.98, 2.40). `LeftHand` → `left_hand`, (0, 0, 0); male bind (−31.19, 4.32, 2.14).
27. Death drop origin = `Left_foot`: male stand (−10.02, −41.49, 13.52), female stand (−7.73, −39.56, 12.91).

**Hit spheres (install)**

28. Male bind pose: every centre in the table of 11 ("Bind centre"), for example `Upper_torso` (0.00, 14.27, 0.74) r 10, `Face_Eyelid1` (0.03, 36.67, 1.29) r 4.6, `Nuts` (0.00, −5.70, 6.75) r 3, `Left_armu1` (−16.36, 19.66, −1.65) r 4.
29. Male `LST` at t = 0 (single animation): every "LST 0 centre", for example `Head` (6.45, 29.03, 10.90), `Left_foot` (−9.95, −42.12, 11.46).
30. Male `LRF` at t = 300 (single animation): `Left_foot` (−3.53, −33.27, −23.60), `Right_foot` (3.44, −40.27, 18.95), `Head` (2.70, 28.39, 13.53), `Right_legl1` (4.26, −30.65, 13.80).
31. Male stand and crouch poses: `Head` (5.66, 29.72, 9.65) → (3.21, 21.61, 12.05); `Torso` (−0.15, 5.91, −0.22) → (−0.69, −1.65, 0.15); `Face_Eyelid1` (6.62, 34.74, 9.89) → (4.24, 26.57, 12.72); `left_hand` (8.84, 23.46, 28.27) → (1.39, 14.69, 30.63).
32. Female bind and `LST` 0: every centre of the female table, for example `Headfuck` (0.08, 92.36, 3.53) / (4.12, 33.37, 12.42), `Bip01 Head` (0.08, 89.37, 1.94) / (2.96, 31.08, 10.21).
33. Female stand → crouch: `Bip01 Head` (2.95, 30.63, 11.32) → (2.48, 15.39, 15.58); above the feet 80.63 → 55.39 (Quirk 7).
34. Default male: table entries 62 and 63 (`Left_Shld`, `Right_Shld`) produce no sphere; 35 entries of table 0 have radius > 0, 33 exist on the body. Default female: 35 entries of table 1 have radius > 0, all 35 exist.
35. A body without the `Nuts` socket (for example `CHARS_M_BODY\CM_HV_UR_Brandon_JS.ltb`, which has no `Nuts` socket in the US archive): no groin sphere.

**Death and bodies (data and logic)**

36. Bullet death, shot travelling along the victim's −forward (cf = −1), last hit sphere `Face_Jaw` (head), c = 1 → `DH`; c = 0 → `DH_1`. Same with `Upper_torso` → `DT` / `DT_1`.
37. Shot along +forward (cf = 1), torso → `DTB` / `DTB_1`; head → `DHB` / `DHB_1`.
38. Shot with d = (0.94, 0, −0.34) (from the left, slightly from the front; cf = −0.34 > −cos 45°): torso → `DLSH`, head → `DLSH`. d = (0.94, 0, 0.34) (from the left, behind): torso → `DLT`, head → `DLSH`. d = (−0.94, 0, −0.34): torso → `DRT`, head → `DRSH`.
39. Explosion with cf = −1 → `D_BO_F`; melee → `D_S`; poison → `D_P`; endless fall → `D_W`; female groin death → `DC` (no `NS1`). With BR data, male groin (`Nuts`, location 5) → `NS1` / `NS2`; with US data (location 2) → the torso choices.
40. Death crouched, front, torso → `DT` / `DT_1` (never `DC`).
41. Owner dies at t = 0 with `DT` (3033 ms), respawns at t = 2.0 s in team deathmatch → the body is removed at t = 3.033 + 5 = 8.04 s (first update after). Respawning at t = 10 s → removed at 15 s. In Combat mode, respawn at 10 s → removed at 510 s.
42. Body creation: dead player at hull centre (100, 200, 300) after the floor snap → body at (100, 200.1, 300), no weapon attachment.

**Follow-up cases** (answers to Open questions 2, 5, 9, 13–22). "Old release" = measured with this repo's LithTech readers on an older extracted client release whose male body and male library reproduce the US 1501.04 numbers above (for example `LST` t = 0 `Head` (6.45, 29.03, 10.90)); re-check on the Steam install (build 25595979) like the other install cases.

43. **(old release)** Default female body, `LST` t = 0 alone: `Bip01 Neck` (node 7, parent 6) at (1.74, 27.99, 9.61); `RightHand` socket (parent `Right_hand`, offset (7.08465, 1.66108, 1.27744)) at (5.69, 23.62, 22.09); bind `Bip01 Neck` (0.069, 86.050, 1.303), bind `RightHand` (33.13, 53.21, 1.37).
44. **Body against the hull** (logic). Hull centre at y = 0. r = 0: half-height 50, skeleton root at y = −50 (hull bottom −50). r = 0.5: half-height 45, root −45. r = 1: half-height 40, root −40. The root is at the hull bottom for every r.
45. **Defaults (data).** Selection with only model 0 and face 4: effective body 7 (`Nomal_W_vest.ltb`), head piece drawn = 23 (`CHARS_M_HEAD\Hair_W_A.ltb`) although the head piece id is 0; face model `CHARS_M_FACE\Face_W_A.ltb`; no goggle, no backpack. Model 0, face 0: no head model, face model `CHARS_M_FACE\Face_A.ltb`. Face 7 with head piece 0: no head model.
46. **Skins (data).** Model 8, camouflage 0 → upper `CHARS_T_BODY\F_Upper_Lev02_OD.dtx`, lower `CHARS_T_BODY\F_Leg_Lev03_OD.dtx`; camouflage 2 → `CW_VST_LT_SN_00_HM.DTX`, `CW_LG_HV_SN_00_HM.dtx`. Model 1, camouflage 6 → `CM_VST_LT_TG_00_JS.DTX`, `CM_LG_HV_TG_00_JS.dtx`.
47. **Digit suffixes (data).** Revolver (24): standing reload `UPRSReS1`, standing fire `UPRFiS`, standing draw `UPRSSeS`. FAMAS (26): moving reload `USMRReS2`, standing fire `USMRFiS`. Double barrel (27): standing fire `USGRFiS1`, standing draw `USGRSSeS1`, walking forward fire `USGWFiF1`, alt fire = fire. Snowball gun (29): standing reload `USGRSReS2`, running forward fire `USGRFiF2`.
48. **Server reload, no frame key (data and logic).** M4A1, reload rate 1, third-person reload `URFRSReS` 1866 ms (US library): the server ignores reported shots from the request until 1555 ms later (1866 / 1.2 = 1555.0), then refills its clip. Reload rate 2.5 → server rate 2.4 → 1866 / 2.4 = 777.5 → timer 777 ms (whole ms, truncated).
49. **Server reload with a frame key.** A weapon with `HHAniFrame_5` = 2300, rate 1, third-person reload 1866 ms: no timer; the upper rate is 1866 / 2300 = 0.8113; the server reload ends on the first driver update after the animation reports done (2300 ms plus at most one movement frame).
50. **Server reload without an animation.** A weapon class whose reload name is missing from the library → timer 500 ms.
51. **Own-client fire mask (logic).** Own third-person body, fire key held while the client weapon is reloading → no fire posture (the masked flag is off). Same key held on the server copy of that player → the server driver stays in the reload posture (step 2) while the reload mark is set, then plays fire.
52. **Hit kick (logic).** Ten damage messages for a remote character from other shooters: displayed pitch offsets (radians, upward) 0.1, 0.2, 0.3, 0.4, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0; the 11th adds nothing and resets the counter; each offset lasts one aim update.
53. **Emote (logic).** Hunted, alive, standing still with the rifle: `G`, then choice 2 → both trackers play `Surrender`, weapon pieces hidden, one surrender sound at the player; pressing forward ends it on the next server update and shows the weapon. The same key in any other mode does nothing.
54. **Floor snap (logic).** Player dies in the air with hull centre y = 300, half-height 50, floor 200 u below the centre (y = 100) → the player object moves to y = 150.1; the body is created at y = 150.2. Hull centre 40 u above the floor (less than the half-height) → no move.
55. **Corpses (logic).** A client shot along a line through a fresh body's spheres and then a living enemy's head 50 u behind → hits the enemy's head; the body is not hit and receives no decal.
56. **Death types (logic).** Killed by an `EndlessFall` volume → `D_W`; killed by a gas grenade's lasting `Poison` damage → `D_P`; killed by an infected's attack in Quarantine (infection kind) → the bullet-style choice (for example `DT` from the front), not `D_P`.
57. **Third-person weapon (data, old release).** M4A1 in hand: model `GUNS_M_HH\HH_AR_M4A1_SH.ltb` on `RightHand` with zero offset; its `Flash` socket sits on node `Bone01` at offset (5.17188, 0, 27.25005) (weapon-model space at bind (0, 2.685, 24.440)); no other weapon of the loadout is drawn.
58. **Additional skins (data).** A body record with `AdditionalSkins0` a and `AdditionalSkins1` b, camouflage index 3 → slot 5 a, slot 6 b (not affected by c).
59. **Model colour (logic).** m = 1 with `ColorSkin1_UpperSkin0..` present and `ColorSkin1_LowerSkin#` absent → slot 0 from the colour list (index c), slot 1 from `LowerSkin{c}`; m = −1 → base lists.
60. **Leaves cut-out (logic).** A ghillie leaves texel with alpha 0.62 (158/255) → not drawn; 0.63 (161/255) → drawn opaque, unlit texture colour.
61. **Missing body file (data, Steam build 26236938).** Records 279 and 280 → their body files are not in the install; loading either → the model is `models\default.ltb` (1,720 bytes, 1 piece, 52 triangles, animation `x` 6566 ms), drawn untextured.

**Who may hold which weapon (data, Steam build 26236938; 12)**

62. The weapon file has 2386 `Weapon#` records; 48 have an id containing `npc`, `boss`, `hqraid`, `powered_suit` or `assassin`. By link (12.4): 25 world NPC hands, 4 HQ raid boss (461 by pattern only), 7 suit sets, 3 crossbow family, 1 skill-only (864), 8 none (67, 421, 537, 538, 560, 561, 562, 863).
63. 40 weapons have `LowerAniType` 1 or 2; only 366 `Quarantine_Stick_NPC` has 2. 21 have `Hidden` true, all infected hands; `NoSelect` 1 on 173, 447, 535; `AIPrimaryWeapon` true on 457, 538.
64. Worlds: 947 `CAIHuman` objects in 23 worlds; every `ModelTemplate` equals the `Name` of one of the 101 `AIModel#` records; `RightHand2` and `LeftHand2` are `<none>` on all.
65. `CAIHuman` with `RightHand` `VH_Normal_NPC` → `ModelTemplate` always `Normal_Infecter_Common_D`, in BlackLung, CabinFever, DeathRoom_CP and RailwayDefence only. `HQRAIDNPC_2_SMG` → HQRAIDNPC_2, Powered_Suit_Npc_Melee, Powered_Suit_Npc_Smg, SubMission_Z_1, SubMission_Z_2. No world object names 537 or 538.
66. `PatternFX0` (`HQRaid_Boss`): `Weapon0` 460 on `highgun` chance 100, `Weapon1` 462 on `Missile_` 50, `Weapon2` 463 on `bomb_` 50; `[PatternWeapon0..3]` `WeaponId` 460, 461, 462, 463.
67. NEMEXISLAB_CP: `ChangeCharInfo_2` → `ChangeCharModel_2` `ModelId` 58, `ChangeCharWeapon_2` `Primary0` 527; set 0 → Model 62 with 528 and 531. DESERTHOLE_RTDM: 10 sets, set 5 → Model 588 with 528 and 531. `WeaponIndex` 2 is set 2, not weapon 2.
68. Use item 54 (`SubType` 20) → `InfectedModelNo` 56, `InfectedWeaponNo` 521; item 55 → 57, 522; items 148 and 150 → 513, 1734.
69. Item file: 303 `Character#` records, all `ItemType` 22; no record of any kind has `ItemType` 30, 36 or 37. 116 characters have `AddSlot`, all with `SlotType` 0; `Character14` has `ItemID` 22 and Model 22 is `Specialist01_M`.
70. `WeaponItem#` with `EquipBackpackType`: 8, all with `EquipOnlyBackpack` 1: type 1 on weapons 173, 174, 290, 291, 293, 447, 558; type 3 on 536. Use items with `SubType` 18: 36, 52, 76, 77.
71. Equip rule (logic): character with `AddSlot` 2, `SlotType` 0 plus an equipped extend item (`SubType` 18, one slot of type 1) → two normal backpack slots and no special one; equipping 290 `M32_FT` is refused. A character with `SlotType` 1 and the same item → three special slots; 290 goes in, and an ordinary weapon (slot type 0) cannot go into those slots: a weapon only enters a backpack slot whose type equals its own.
72. The 33 `_QRT` mission files: infected melee 191, host melee 190; NOXIOUSDAMNATION_LLHG infected melee 1346.
73. `[NpcSkill16]` `P_Boss_IceGas` uses `Abnormal_IceGas_boss`; no `[Brain#]` in the three AI files lists `P_Boss_IceGas`.
74. Weapon items exist for exactly 14 of the 48 (416–420, 526–534) and for one infected hand (344).

**NPC and boss bodies (data, Steam build 26236938; 13)**

75. The model file has 101 `AIModel#` records (0–100) and 59 `AIFace#` records (0–58). `AIOnly` 0 and `HumanModel` 0 only on `AIModel` 35 and 36. No `Model#` or `AIModel#` record has a `ModelScale` key. `UseSpecialDeathAni` `VDIE` on 8, 9, 40, 50, 60, 70, 100 and `death` on 87, 88.
76. Worlds: 947 `CAIHuman` objects naming 86 distinct `AIModel`s. By the record's skeleton: 0: 768, 1: 46, 2: 67, 3: 11, 5: 18, 7: 7, 9: 1, 10: 1, 11: 2, 12: 4, 13: 4, 14: 4, 15: 3, 16: 5, 18: 6; none for 4, 6, 8, 17. Most placed: `HQRAIDNPC_2` 99, `SH_Rifle` and `SH_MGunner` 83 each.
77. `AIModel` 13 `Squad_1`: body `CHARS_M_BODY\CM_Body_HV_NX_YK.ltb` with 5 skins in slots 0–4; face `Squad_1_Face` → `AIFace10` `CHARS_M_FACE\Face_A.ltb`; head `Cap` → `Hair3` `CHARS_M_HEAD\CAP.ltb`; goggle `Chemical_Gassmask` → `Goggle18` on `Eyes`. `AIModel` 0 `SH_Rifle`: face `MT_RF_FACE` → `AIFace6` (`CHARS_M_FACE\MT_NO_FACE.ltb`), no head piece, no goggle. `AIModel` 8: `AttachFace` 0, no face. `AIModel` 58: face `InfectedS_New_Woman_B` → `Face55` (not `AIFace43`).
78. **(install)** The `AIModel`s name 64 body files (ignoring case); `Body_Tutorial_Dummy_KI.ltb` is not in the install. 33 have 53 nodes with the names, order and parents of `CM_VST_NM_DF_Nuts_JS.ltb` and child `ST_M_child.ltb`; 11 have the stock female 60-node tree and `ST_W_Child.ltb`; 19 others. `MT_GG_BODY.ltb` has no `Eyes` socket and no animation of its own; the seven `MT_*_BODY` files have no `Nuts` socket.
79. **(install)** `VIM_F_NM_DF_Tanker_SH.ltb`: 43 nodes, 17 sockets, weight sets `upper`, `lower`, `blend`, child `Ani_VI_Tanker_SH.ltb`, 103 entries in all. `VLST` 1333 ms, `VLRF` 800, `VLRRF` 667, `VUMRFIS` 666 (interpolation 0), `VDIE` 3733; no `VLSJ` (there is `VLJS`), no `VLRRL`, `VLRRR`, `VUMRSSeS`, `D_HIT`, `VD_HITT`. `CM_Body_Infected_Ship_Captain_KE.ltb`: the same parent of every node as the tanker; node 7 `Bip01 Neck`, 9 `Bip01 L Clavicle`, 37 `Bip01 L Foot`, 42 `Left_foot`.
80. **(install)** `VIW_F_NM_DF_Assassin_CH.ltb`: 71 nodes, 19 sockets (`Eyes` on `Bip01 Head`), child `ANI_VI_Assassin_CH.ltb`, 149 entries; `VLST` 5666 ms, `VLC` 5700 with dims (15, 40, 15) and root (0, −40, 0), `VD_HITT` 533, `VDIE1` 3933. `CW_VST_TheShining.ltb`: same tree and library, no weight sets.
81. **(install)** `Suit_small_Base.ltb`: 44 nodes, 13 sockets (no `Eyes`, no `Upper_torso` node), child `small_anibase.ltb`, 272 entries; `LST` dims (80, 100, 50) root (0, −100, 0); `LC` (80, 70, 50) root (0, −70, 0); no `LWFL`, `LRFL`, `LRRL`; `USMGRFiS` 200 ms, `USMGRSReS` 1933, `D_HIT` 833, `DT` 2666, all 50 ms interpolation. As a player body: standing half-extents (80, 100, 50), crouched (80, 70, 50), eye 66 u above the centre.
82. **(install)** `Suit_BIG_Base.ltb`: 43 nodes, 16 sockets, child `BIG_anibase.ltb`, 122 entries; `LST` (128, 150, 70) root (0, −150, −40); `LC` (128, 120, 80) root (0, −120, −20); `RightHand` on `Right_hand` at (−12.109, 0, 0); `Eyes` on `Head` at (8.281, 0, 7.891); eye 99 u. No `USMGRAmS`.
83. **(install)** `PV_CabinFever2_EOD_Boss_KE.ltb`: 35 nodes, 8 sockets, weight sets `AL` (nodes 22, 23), `BODY`, `Lower`, `blend`, `upper`; 20 entries, all dims (70, 100, 55) root (0, −100, 0): `ilde` 1667, `Walk` 1367, `All_Attack` 7033, `Death` 4667. `RightHand` on `Dummy003` (37, 6.031, 2.031), `LeftHand` on `Dummy001`.
84. **(install)** Own-animation bosses, entry counts: `GXL-4209` 12, `GXL-4209B` 15, `GXL-X` 4, `Boss_Fortress` 18, `Mob_Robot` 8, `Beetle` 12, `Callus` 4, `Scorpion` 10, `Thorn` 5, `Bugs_Explosive` 4, `Bugs_MeleeHeavy` 4; each has a `Lower` weight set of all zeros and an `Upper` of all ones. `Boss_Fortress` `Gatling_LP` 2567 ms; `LeftHand` on `Gun_L` at (100, 0, 0).
85. **(install)** Hit-table entries with radius > 0 that resolve (node / socket / absent): table 2 on the tanker 41 / 4 / 0, on a captain 38 / 4 / 3; table 3 on the assassin 57 / 8 / 1 (`Head`); table 5 on the small suit 22 / 8 / 3; table 6 on the big suit 22 / 9 / 2; table 7 on the EOD boss 30 / 0 / 2, on `GXL-4209` 19 / 0 / 13; table 18 on `Bugs_Explosive` 1 / 0 / 19; table 0 on `Light_W_vest.ltb` 19 / 9 / 7; table 4 on `boss_model.ltb` 41 / 0 / 0.
86. **(install)** `boss_model.ltb`: 66 nodes, 24 sockets, no weight sets, child `boss_anibase.ltb`, 14 entries, dims (450, 200, 500): `idle` 333 ms, `left` 133, `left_start` 333, `left_end` 500. Sockets `Missile_1` … `Missile_6` and `bomb_1` … `bomb_7` exist, `Missile_` and `bomb_` do not; `highgun` on `du_minigun_01` at (0, 0, 113.484). So 460 fires from 1 socket, 462 from 6 in turn, 463 from all 7 at once.
87. **(logic)** Powered suits: SMG suit (class 58) aiming while strafing right → `USMGRAmL`; pistol suit (59) aiming still → `USMGRAmS`, firing still → `UPRFiS`; big suit with 531 (class 64) aiming still → no new upper animation (`USMGRAmS` absent); any suit moving forward-left → the lower tracker keeps its previous animation (`LRFL` absent).
88. **(logic)** Player-driven tanker holding an M4A1 (class 0): upper names from class 39 → standing `VUMRAmS`, firing `VUMRFIS`; legs from the `V` set → running forward `VLRF`; standing jump → no new leg animation (`VLSJ` absent).
89. **(install)** The Steam male library has `Insult` and `Salute` (9 says the US library lacks them); the female library lacks `DHB_1`, `NS1`, `NS2`. Both libraries have every upper name the NPC guns' classes need: `URHRAmS`, `UBKRAmS`, `USGRFiS1`, `UGGRAmS`, `URWRAmS`, `UFSRAmS`, `UP9RAmS`.

## Open questions

1. **Contradictions with `weapon_firing.md`'s hit-zone table.** (a) The male "Shoulders: 5.5 each, plus 7.0 each" — the 7.0 spheres (`Left_Shld`, `Right_Shld`) are table entries with no node or socket on any of the 100 bodies read, so they never exist; only the 5.5 shoulder spheres do. (b) Groin location: the US data gives `Nuts` location 2 (torso), the BR data 5 (groin); `weapon_firing.md` uses 5 (no armour, groin death animation). Which does the Steam data use? Check the decrypted Steam `MODELBUTES.TXT`. (c) "The female skeleton has the same body and limb spheres": it has no neck sphere and no groin sphere, and has four extra 3 u torso spheres (clavicle and spine helpers). (d) The tables key on the model's `Skeleton` value, not on `FigureType`: 13 US records pair them differently (for example female-shaped enemy NPCs on skeleton 1 with figure type 0). Update `weapon_firing.md` after a review.
2. **Reload timing on the server.** The server finishes a player's reload (and, for weapons with `CheckReloadAniTime`, default true, refills its copy of the clip) when its timer of third-person reload length ÷ reload rate expires, or when the third-person reload animation is done. For the M4A1 that is about 1866 ms (US library), while `weapon_firing.md` gives 2300 ms from the first-person animation. Does the server's clip become full before the client's? Check against the Steam libraries' `URFRSReS` length and with a live test (fire right after 1.9 s of reloading).
   **Answered:** yes, the server is ready first. Its reload rate is min(r, 2) × (2 − `ReloadCheckTime` 0.8) = 1.2 × min(r, 2), so the M4A1 timer is 1866 / 1.2 = 1555 ms; the 500 ms fallback applies only when the class has no third-person reload animation; a weapon with `HHAniFrame_5` gets no timer and ends with its animation. While the server's reload runs it drops every reported shot of a magazine weapon, so the end that matters for gameplay is the **server's third-person** one; the client's clip refill is the first-person animation's (`view_models.md`). Applied in 7 (The server's reload); cases 48–50. Still worth one live check on Steam (its library may change `URFRSReS`): reload an M4A1 with a stopwatch on the shot counter of a second client, and fire at 1.6 s and 1.9 s after the key; both shots should register.
3. **Camouflage index.** It arrives with the character info; its source on the client (presumably the equipped uniform item) was not traced, nor the meaning of skin index 5 (`_FK`) and 6 (`TG`).
4. **Sprint directions.** Can a player sprint with a backward or pure strafe direction? If so the animation driver stops updating (Edge cases). Check `movement.md`'s sprint conditions or in game.
5. **Server pitch at `Torso`.** The server pitches `Torso` by 0 while clients use 0.15 / 0.35. This affects only server-side sphere tests (shots the client reported as non-player hits, `weapon_firing.md`). Confirm it is intended, and whether our implementation should keep the mismatch.
   **Answered:** the zero is an explicit per-node assignment on the server each time the aim is applied (the other six nodes get the real pitch), so it is deliberate; keep it. The server's spheres are used only by hits the server traces itself: projectile and crossbow direct hits and bot (AI) instant-hit shots; client-reported player hits are validated on the hull box only. Applied in 8 (Who applies it, Where the server's spheres matter).
6. **Aim precision on the current update path.** The newer per-object update data carries aim pitch and yaw for remote characters; its encoding was not traced. Test: look 0.3° up and down repeatedly in front of another client and watch whether its model moves.
7. **`Eyes` as the death camera.** The client can attach the camera to the dead body's `Eyes` socket; on the female body that socket is near the floor. Is the head-attached death camera used in this build? If so, the female death view starts at floor height.
8. **Steam libraries.** The Steam animation libraries are larger than the US ones (more animations). Re-measure every "install" case, and re-check which names exist (Quirk 1: the moving rifle fires may exist on Steam).
9. **Corpses and shots.** The client ray test also tests dead bodies' spheres, and bodies keep a hit box. Do bodies stop bullets for the client test (making the shooter report a "body" instead of a player behind it)? Check in game by shooting through a fresh corpse at a player.
   **Answered:** no. The body's hit box exists only on the server and is not sent to clients, the client never builds its own body hit box, and the client's shot filter refuses body objects, so client shots pass through corpses (and bodies show no debug spheres). The earlier reading ("bodies keep hit spheres for the client test") was wrong; 10 step 3 and Edge cases are corrected; case 55.
10. **Shotgun per-round reloads.** The automatic-shotgun tables contain separate pre-reload, per-round and post-reload names (`USGRReS-A`, `-B`, `-C`), but the posture set has only "reload"; which of them the reload posture uses for players was not traced beyond the main reload entry (`USGRSReS` standing, `USGRReS-B` moving). The `USGRReS-*` names are also missing from the US female library.
11. **Weight-set name case.** The trackers ask for `Upper` and `Lower`; the bodies store `upper` and `Lower`. This works only if the engine's weight-set lookup ignores case (assumed here, as for all other names).
12. **Stuck hit status.** Quirk 4 follows from the decision order read; check in game whether a player who is hit while spawn-protected keeps an aim pose while firing until the next weapon status (switch or reload).
13. **Hit spheres and animation.** Does the server place the hit spheres on the animated skeleton every update (which trackers count) or on a fixed pose? Female `Bip01 Neck` and the female right-hand socket.
    **Answered:** on the animated skeleton, evaluated at the moment of each test: all four trackers (walking or running legs, the upper body's weapon, fire, reload and draw animations, the crouch weight) plus the aim rotations, on the server (server pose, no `Torso` pitch) and on the shooting client (its own animation of the target). Not a fixed pose. Female numbers measured on an older release (3, Female neck and right hand; case 43). Applied in 11.
14. **Update rate.** Per server frame, per movement tick or per rendered frame?
    **Answered:** server: the driver per client movement frame received, tracker advance per server frame (20 Hz shipped), so yes, the server animates characters; own client: driver per 15 ms movement tick, advance per rendered frame; other characters on a client: no driver, server-chosen animations advanced per rendered frame. Applied in 4 (When the pose is decided), Constants, Per-tick order.
15. **The drawn body against the hull** (standing, crouched, during the transition).
    **Answered:** no offset; the model is the hull object, and the root translation (−50 stand, −40 crouch, blended by r) keeps the skeleton's root on the hull bottom for every r, since the half-height is 50 − 10 r. Applied in 4; case 44.
16. **Defaults without lobby data** (head piece, goggle, backpack, face, skin per stock body; skin tables).
    **Answered:** fresh selection values, plus the face's default hair when the head piece is empty, the face's default goggle when "default goggle" is asked (none for faces 0–7), no backpack, camouflage 0. Skin tables of all six stock bodies by role in 1 (Defaults when the selection carries nothing); cases 45, 46. Bots build their own record from the profile (`bots.md`).
17. **"Firing" for the driver.**
    **Answered:** server = the raw fire flag of the movement frame; own client = the flag masked by the weapon being in a firing state (begin, firing, fired, end, alt). Grenades use their own sequence on the same flag. Applied in 5 ("Fire held" for the driver); case 51.
18. **Upper-body rates and digit suffixes.**
    **Answered:** `HHAniFrame_0`..`_9` and `HHAniFrame_5_1` are keys of each weapon record in the weapon file, read with default 0 (= rate 1). Suffixes: revolver 1 and FAMAS 2 on reloads only; double barrel 1 and snowball 2 on fire, alt-fire, select and reload. Applied in 6, 7; case 47.
19. **Which reload ends the server's reload** (with Open question 2).
    **Answered:** the third-person one (timer L / (1.2 min(r, 2)) or the animation's end, whichever is first). Applied in 7.
20. **Emotes and the hit kick on remote characters.**
    **Answered** (except one detail): the hit kick comes from the server's per-character damage-flags message when the damager is not the local player and lasts one aim update (9). Emotes exist only in Hunted, opened with `G`, four choices, one sound, weapon hidden, both trackers play the emote non-looping until done or until the player moves, fires or reloads (9); case 53. **Still open:** whether the driver's per-movement-frame leg start replaces the emote on the lower tracker. Measure: in Hunted, start `Insult_Dance` standing still and watch a second client: do the legs dance for the whole animation?
21. **Death choice details.**
    **Answered:** `D_W` = the `EndlessFall` damage kind, `D_P` = `Poison`, `D_S` = `Melee`; the coin (Quirk 6) is a fresh C-library generator seeded with the clock in **seconds** before every death choice; the floor snap moves the player object itself. Applied in 10; cases 54, 56.
22. **Third-person weapons.**
    **Answered:** `HHModel` of the weapon record, on `RightHand`, zero offset, identity rotation, scale 1 (`HHModelScale` unread); no back or holster display. Applied in 2; case 57.
23. **Left open by the 2026-10-22 corrections.** (a) A jump-pad launch sends its own motion status, which the driver treats like "falling" (standing legs until landing); whether the client also reports an ordinary "jumped" for it was not traced, so whether a pad launch shows the jump legs is open. (b) Item 9's frozen upper pose after an emote follows from the start rules as read; confirm live. (c) Item 1: whether the stock fire animations' last frame differs visibly from the aim pose (how a held trigger looks to others) needs the libraries.

24. **Specialists in this build.** No character is typed specialist (30) or assassin (37) and no character gives a special or assassin slot, so by the client data the 8 slot-typed weapons (12.3) can never be equipped. Does the lobby server's own item data differ (types or slot types), or are specialist slots really gone in the Classic? Check in game: buy a specialist character, try to equip `M32_FT`. Until then the "specialists" are named bodies only (12.3 list) and the specialist-only weapons are those 8.
25. **The suit-set mode.** The mode that reads `ChangeCharInfo_` / `ChangeCharModel_` / `ChangeCharWeapon_` (co-op NemexisLab and the `_RTDM` files) is not in the reference code read: how a player picks a set (`HUDIndex`), whether a suit can carry other weapons, and whether suit weapons ever reach a non-suit body.
26. **Weapons with no link** (67 `Minigun_npc`, 421 `Watergun_NPC`, 537, 538, 560 `Ravens_G36C_NPC`, 561 `Souzas_Parafal_NPC`, 562 `PP19_MOD_Scorpion_NPC`, 863 `Abnormal_Boss_Minigun_Left`, 864, and hands 344, 674, 678). By name only: 537/538 → AIModels 36/35, 863 → the same bosses as 858, 674 → AIModel 51, 678 → the captains 40/60, 560–562 → the named mercenaries Raven, Souza and Scorpion (player bodies); 67, 421, 344 → none. These are guesses for our data, not the game's.
27. **Cross-type hands.** Worlds give some hands to other types (12.6 "also": a captain's hand on male hosts in DarkThanDark, `Quarantine_Host_Hand` on the co-op zombies, `M9` on `Normal_Infecter_Common_D`). For "own infected type only", take the "Own type" column; for "any holder the game used", add the "also" column.
28. **Answered (2026-10-31)** in 13: the `AIModel#` records, their bodies and libraries, hit tables 2–18, the powered suits and the pattern boss, and a mapping for player-driven NPC bodies; new questions 31–39. Was: **NPC bodies as rung bodies.** Every NPC and boss weapon belongs to `AIModel#` bodies (or the boss `PatternFX0` model), never to a `Model#` player body; skeletons 2–18 and their animation libraries were not specified.
29. **`[DeditWeapon]`.** What reads this list (an editor list for NPC hands is assumed from the name) was not found.
30. **Older data.** In the US and BR data sets read for this spec's earlier sections, were the specialist and assassin characters typed 30 / 37 with special or assassin slots, and the crossbows typed 36? If so, the older rule was: crossbows and the slot-typed weapons only on those characters.
31. **`VLJS` on the tanker library.** The tanker's library has `VLJS` where the driver asks for `VLSJ` (standing jump), so the original plays no standing jump on tanker bodies. Is `VLJS` that animation under a typo? A player-driven tanker could alias it; parity says not.
32. **The Shining without weight sets** (Quirk 13). How does `Model` 513 (an infected mercenary players can be) animate in game, given that its body has no `upper` / `lower` sets? Watch an infected Shining mercenary run: if it animates, the engine falls back to something the read code does not show.
33. **Boss-battle AI.** The worlds give the eleven own-animation bosses the NPC types `SpecialBoss` and `SpecialBugs`, which the reference code's NPC type list does not contain (it knows `NPCDefault`, `Trigger Boss`, `Last Boss`, `Bugs`, `Zombie`, `ClubZombie`, `AbnormalZombie`, `SpecialZ`, `AbnormalBoss`, `PrisonOfficer`, `BonusCoin`). How these bosses choose animations, where their weapons appear (`Beetle` holds 858 in both hands with no hand socket) and how their hulls are sized is not in the code read; 13.8's mapping for them is ours.
34. **The `Whole` weight set.** Bug-type NPCs ask for a weight set `Whole` on both trackers; no body has one. Does the pose freeze (`animation.md` Quirk 5) or does the engine fall back? Only matters for copying NPC behaviour, not for player-driven bodies.
35. **Hull of large bodies.** The player rule gives the EOD boss and the own-animation bosses the fallback (15, 50, 15) box while their animations put the feet 100–525 u below the centre (13.6). For a player-driven boss body, use the AI rule (its current lower animation's dims) or a fixed box from its idle animation? Also check `movement.md`'s crouch eye term (−13 r) and crouch speed for the suits, whose crouch lowers the hull by 30 u rather than 10.
36. **NPC suits' guns.** `AIModel` 35 and 36 hold `HQRAIDNPC_2_SMG` (class 2), and the suit library has no `URH...` names. In game, do the NPC suits keep one upper pose while firing? Which class should a player-driven suit use for a non-suit gun (13.8 proposes the SMG-suit class 58, and the melee-suit class 61 for the melee record 35)?
37. **Missing head spheres.** The assassin body names its head `Bip01 Head` while table 3 asks for `Head`, so `Light_Infecter_Speed_B` has no head sphere (13.6); the captains lack `Neck`. Confirm in game (headshots on a Speed-B infected should register as body hits).
38. **Table 18 and the bug bodies.** Table 18's node names are `GXL-X`'s, not the bugs': `Bugs_Explosive` keeps 1 sphere and `Bugs_MeleeHeavy` 2. Are those bugs hittable only on the root sphere in game?
39. **Steam libraries against the US measurements.** Section 13 was measured on the Steam install, the earlier sections' "install" numbers on the US client; the male library differs at least by `Insult` and `Salute` (test case 89). Re-measure 7's lengths on Steam.
