# Combat Arms: skeletal animation

Source basis: reference material the user pointed the spec session to on the engine's model loading and animation playback (the model reader's animation, node, weight-set, child-model and binding sections, the animation tracker, the node-transform evaluation and blending, the skinned and vertex-animated mesh render paths, the quaternion helpers, the client frame clock and the model-key callback), the game's first-person weapon animation logic, its character animator and character model-key handling, and the model attribute table of a US-region data set. Every number in "What the install contains" and in Test cases for the first-person weapon was measured on the Steam install on the Windows PC (Steam build 25595979), reading the models straight out of the unkeyed archives with a throwaway reader written from `rez_archive.md` and this spec. The character archives of the Steam install are V2-keyed and could not be read; the character numbers come from the US reference client (build 1501.04) and are marked as such.
Status: draft

Sections: Summary; Units and conventions; Constants; Where animations are stored; Byte layout (primitives, counts in the model header, child models, the animation section, one animation, node channels, vertex animation, weight sets, bindings); Building a model's animation list; Sampling one animation; Playback (time base and rate, key scan, looping, end of play, transitions); String keys (parsing, weapon keys, character keys); Layered playback (trackers and weight sets); Node evaluation order and the node transform; Skinning; First-person weapon animations; What the install contains; Per-frame order; Edge cases; Quirks; Test cases; Synthetic fixture; Open questions.

The rest of the model file (header, pieces, meshes, skeleton bind poses, sockets, levels of detail) belongs to `specs/combat_arms/models_ltb.md`, which is being written alongside this one. This spec describes the surrounding sections only as far as a reader needs them to find and decode the animation data. Where the two disagree about anything outside the animation data, `models_ltb.md` wins.

---

## Summary

- Animations live **inside model files** (`.ltb`), in an animation section after the skeleton. A model can name **child models** whose animations are appended to its own list and played on its skeleton by node index. Combat Arms uses this everywhere:
  - every first-person weapon is a mesh model `PV_*.LTB` with one bind-pose animation and one child `ANI_*.LTB` that carries the real animations (`select`, `idle_0`, `fire_0`, `reload`, ...);
  - every character body is a skin model with one animation (`baseAnim`) and a shared child (`ST_M_child.ltb` for men, `ST_W_child.ltb` for women) carrying about 1,500 animations.
- An animation is a list of **keyframes** (integer millisecond times, each with an optional text string) and, for every skeleton node, a position channel and a rotation channel. A channel stores no value (identity), one value (constant) or one value per keyframe. Values are 32-bit floats, or 16-bit integers (positions in 1/16 unit steps, quaternion components scaled by 32767), depending on the animation's compression code.
- Playback advances in whole milliseconds of real frame time, times a rate. Between keys, positions are linearly interpolated and rotations are spherically interpolated. Key strings fire when playback time passes their key. An animation's length is the time of its last key. A non-looping animation counts as done only once its time is **past** the last key (length + 1 ms).
- Switching animation can blend over the new animation's own **interpolation time** (stored per animation; 0 for all first-person weapon animations, 0 to 200 ms for characters). During the blend the new animation does not advance.
- Several **trackers** can play on one model at once. The first one sets the pose; each further tracker blends its pose in per node with a weight from a named **weight set** stored in the model (`upper`, `Lower`, ...). Players run a main tracker plus upper-body, lower-body and crouch trackers.
- Nodes are evaluated from the root, parent before child, as local translation-plus-rotation matrices multiplied down the tree. A skinned vertex is the weighted sum of its bones' (animated global × inverse bind global) transforms applied to the bind-pose vertex.
- Weapon timing values in other specs (draw time, fire cycle, reload time, sprint-in and sprint-out, knife hit times) are animation lengths and key times from the weapon's `ANI_*.LTB`.

## Units and conventions

- Integers are little-endian. "u8", "u16", "u32" are unsigned, "i16" and "i32" signed, "f32" an IEEE-754 single.
- **String**: a u16 byte count n followed by n bytes, with no terminator. n = 0 is the empty string. Names are 8-bit ASCII. The engine truncates strings to its buffer sizes (4095 bytes in general, 255 bytes for binding names); no file in the install comes close.
- **Vector**: 3 × f32 (x, y, z). **Quaternion**: 4 × f32 in the order (x, y, z, w), w being the scalar part.
- Distance: LithTech world units ("u"), Y up, left-handed (X right, Z forward), as in the other Combat Arms specs.
- Time: integer milliseconds. Animation time, key times, lengths and interpolation times are all in ms.
- Matrices: 4 × 4, row-major in the file and in this spec, for column vectors: $v' = M v$, translation in the fourth column ($m_{03}, m_{13}, m_{23}$).
- Quaternion product is the Hamilton product, $q^{-1}$ means the conjugate (all quaternions here are treated as unit). The rotation matrix of $q = (x, y, z, w)$ is

```math
s = \frac{2}{x^2+y^2+z^2+w^2},\qquad
R(q) = \begin{pmatrix}
1 - s(y^2+z^2) & s(xy - wz) & s(xz + wy)\\
s(xy + wz) & 1 - s(x^2+z^2) & s(yz - wx)\\
s(xz - wy) & s(yz + wx) & 1 - s(x^2+y^2)
\end{pmatrix}
```

  The division by $|q|^2$ makes slightly non-unit quaternions (from 16-bit storage or from interpolation) produce a proper rotation.
- "Node index" is a node's position in the skeleton's depth-first pre-order (root = 0, then each child subtree in file order). This is the order nodes appear in the file.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| pos16_scale | 1/16 | u per step | A 16-bit position component $c$ decodes to $c/16$ |
| quat16_scale | 1/32767 | per step | A 16-bit quaternion component $c$ decodes to $c/32767$ |
| min_model_version | 23 | | Model files with a lower version are refused |
| max_child_models | 32 | | Including the model itself |
| max_trackers_evaluated | 8 | | At most 8 trackers take part in a pose |
| default_interp_ms | 200 | ms | What the exporter writes as an animation's interpolation time unless the artist changes it |
| dims_default | (128, 16, 16) | u | Box of an animation entry that nothing ever set (never seen in practice) |
| rate_cap (game rule) | 2.0 | × | A first-person weapon animation rate above this is replaced by 1.0 (`weapon_firing.md`, `inventory.md`) |
| idle_min, idle_max | 5, 15 | s | Range of the random wait before a first-person full idle (`idle_1`) |
| motion_threshold | 0.1 | u/s | Above this speed, with the moving flag set, the player counts as moving and gets no full idle |
| `AniRateInterpolation` | 1 | | Console variable: 1 carries the fractional milliseconds of rate-scaled frame time to the next frame, 0 drops them |
| `ModelTransitionMS` | 200 | ms | Console variable that exists but does **not** affect playback; transitions use each animation's own interpolation time |

## Where animations are stored

### In the model file

A model file is a sequence of sections. The animation data is one of them; the sections before it must be walked (they have no offsets or sizes in the header) to reach it. In file order:

| # | Section | What this spec needs from it |
|---|---|---|
| 1 | Container header, 20 bytes | Byte 0 = 1 (model file). u16 at offset 2 = 9 (container version). The rest is reserved. |
| 2 | u32 model version at offset 0x14 | 23 or 25 in the install (see What the install contains). The animation data is laid out identically in both. |
| 3 | 15 × u32 counts | Field 0, 1, 13 and 14 concern animation (next table). |
| 4 | Command string, visibility radius, oriented boxes, pieces | `models_ltb.md`. Each piece level of detail carries a size-prefixed render object, so it can be skipped. |
| 5 | Node tree | Names, node index, flags, bind matrix, children; recursive. `models_ltb.md`. The tree's shape and order are what the animation channels follow. |
| 6 | Weight sets | Used by layered playback (below). |
| 7 | Child models | Names of the models whose animations are appended. |
| 8 | **Animations** | This spec. |
| 9 | Sockets | `models_ltb.md`. |
| 10 | Animation bindings | Per child model, per animation: box and root translation. |

The file ends right after the bindings in every model read for this spec.

### In child models

- A child model is an ordinary model file named in section 7 by a path **relative to the parent's folder** (the parent's archive path with its file name replaced). Examples: `Ani_PST_M92FS_SH.ltb` next to `PV_PST_M92FS_SH.LTB`; `ST_M_child.ltb` next to the character body. Names are matched without regard to case, as all archive paths are.
- A child model must not itself have child models (its child count must be 1); the engine refuses the parent otherwise.
- Only the child's animations (and their bindings) are used. Its pieces are not drawn as part of the parent, and its own skeleton is not used: the child's node channels are applied to the parent's node with the **same index**. The two skeletons must have the same node count and order; names are not compared. (In the M92FS pair, eight gun-part nodes have different names in the two files and still animate correctly.)
- If a child cannot be loaded, its slot stays empty and its animations are absent from the list; whether the parent then still loads or fails is Open question 4.

### Additional models (first-person hands)

The first-person hands are a separate skinned model attached to the weapon model as an "additional model". It has no animations of its own on screen; its bones are matched to the weapon model's nodes **by name** (case ignored) once when attached, and it is skinned with the weapon model's node transforms. A hand bone whose name the weapon model lacks keeps its own index. The hand model's file layout is `models_ltb.md`'s.

## Byte layout

### Counts in the model header that concern animation

The 15 u32 after the version are allocation counts for the whole file. Readers do not need them to parse, but they are a cheap consistency check:

| Field | Meaning | M92FS `ANI_` file |
|---|---|---|
| 0 | Total keyframes over all of this file's own animations | 92 |
| 1 | Number of this file's own animations | 13 |
| 13 | Bytes of vertex-animation vertex data (floats only, without the per-frame counts) | 0 |
| 14 | Bytes of node channel data over all animations (sum of all channel payloads, without counts) | 41,192 |

Fields 2 to 12 (nodes, pieces, child models, triangles, vertices, weights, levels of detail, sockets, weight sets, strings, string bytes) are `models_ltb.md`'s. Field 4 (child model count) equals the count in section 7.

### Child models (section 7)

| Size | Type | Meaning |
|---|---|---|
| 4 | u32 | C = number of child models **including the model itself**. Must be 1 ≤ C ≤ 32. |
| var | String × (C − 1) | File names of children 1 to C − 1. Child 0 is the model itself and has no name. |

### The animation section (section 8)

| Size | Type | Meaning |
|---|---|---|
| 4 | u32 | A = number of animations in this file |
| var | Animation × A | A records, back to back |

### One animation record

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 12 | Vector | Default box of this animation: half-extents (x, y, z) in u. Overridden by the binding (below). |
| 12 | 2 + n | String | Animation name. Names are looked up without regard to case. |
| var | 4 | u32 | Compression code k (table below) |
| +4 | 4 | u32 | Interpolation time into this animation, ms |
| +8 | 4 | u32 | N = keyframe count |
| +12 | var | Key × N | Per key: u32 time in ms, then a String (empty for most keys) |
| var | var | Node channels | One channel record per skeleton node, **in node order** (depth-first pre-order of the tree, the same order as the node section). There are exactly as many records as the file has nodes. |

- Key times are non-decreasing. Several keys may share a time (570 animations in the install do). The first key is at 0 in every animation of the install. The animation's **length** is the time of the last key, $L = t_{N-1}$, or 0 if N = 0.
- N = 0 and N = 1 are legal; such an animation never moves (see Edge cases).

**Compression codes:**

| k | Position values | Rotation values | Channel counts present | Used in the install for |
|---|---|---|---|---|
| 0 | f32 × 3 | f32 × 4 | No: every channel holds N values (or the node is vertex-animated) | Characters, props, FX, bind-pose animations |
| 1 | f32 × 3 | f32 × 4 | Yes | One FX model |
| 2 | i16 × 3, ÷ 16 | i16 × 4, ÷ 32767 | Yes | Some props and FX, the default placeholder model |
| 3 | f32 × 3 | i16 × 4, ÷ 32767 | Yes | All first-person weapon animations |

Any other code makes the model fail to load.

### Node channel record, compression 0

| Size | Type | Meaning |
|---|---|---|
| 1 | u8 | Vertex-animation flag. 0 = transform channels follow; non-zero = vertex lists follow (next section). |
| 12 × N | Vector × N | Flag 0 only: position at each key |
| 16 × N | Quaternion × N | Flag 0 only: rotation at each key |

All positions come first, then all rotations. A vertex-animated node has no transform channels: its position is (0, 0, 0) and its rotation identity at every key.

### Node channel record, compressions 1, 2 and 3

| Size | Type | Meaning |
|---|---|---|
| 4 | u32 | P = number of stored positions |
| P × s_p | | Positions: s_p = 12 (three f32) for k = 1 and 3; s_p = 6 (three i16) for k = 2 |
| 4 | u32 | Q = number of stored rotations |
| Q × s_q | | Rotations: s_q = 16 (four f32) for k = 1; s_q = 8 (four i16) for k = 2 and 3 |

**Filling missing keys.** A channel's value at key i is:

| Stored count | Value at key i |
|---|---|
| 0 | Position (0, 0, 0); rotation (0, 0, 0, 1) |
| 1 | The single stored value, at every key |
| N | The i-th stored value |
| anything else | Never written by the exporter and not present in the install. The engine would index past the stored values. Our reader rejects the file. |

How the exporter chooses the count, which explains what is in the files: it drops consecutive exact repeats; if one value is left it stores 1, otherwise it stores all N values (repeats included). There is no per-key time table and no run-length or keyframe reduction beyond "constant or full".

**Decoding 16-bit values:**

```math
p = \left(\frac{c_x}{16},\ \frac{c_y}{16},\ \frac{c_z}{16}\right),\qquad
q = \left(\frac{c_x}{32767},\ \frac{c_y}{32767},\ \frac{c_z}{32767},\ \frac{c_w}{32767}\right)
```

- Positions are fixed point with 4 fractional bits: steps of 0.0625 u, range −2048 to +2047.9375 u. The exporter truncates toward zero.
- Quaternion components are truncated toward zero by the exporter. The decoded quaternion is **not** renormalised; interpolation runs on the raw values and the matrix formula above normalises.

### Vertex animation (compression 0, flag non-zero)

| Size | Type | Meaning |
|---|---|---|
| (4 + 12·V_i) per key | | For each of the N keys: u32 V_i = vertex count, then V_i × Vector (positions) |

- The vertex lists belong to a vertex-animated mesh piece that names this node (the piece's render object stores the node index; `models_ltb.md`). Vertex v of the piece at key i is the v-th vector of list i. All lists of a node have the same count in the install.
- Only the **first tracker** (the main one) drives vertex animation. At render, each vertex position is the linear interpolation between the list of the tracker's previous key and of its current key with the tracker's interpolation fraction (see Sampling), in the piece's model space. If the current key's list is missing, the previous one is used for both.
- The dedicated server skips these lists.
- 38 FX models in the install use it (mines, med kits, screens, tornadoes). No character or weapon does.

### Weight sets (section 6)

| Size | Type | Meaning |
|---|---|---|
| 4 | u32 | W = number of weight sets |
| per set | | String name; u32 count (must equal the node count, else the model fails to load); f32 × count, one weight per node in node order |

Weight values are normally in [0, 1]. The value **2.0** exactly means "additive" for that node (see Layered playback). The default character's sets use only values in [0, 1]. In the readable archives 2.0 appears only in a set named `twitch` that 24 one-node FX debris models carry as part of a stock template (`Null`, `Upper`, `Lower`, `blink`, `twitch`, `Morph0`, `Morph100`); the game's dead-body twitch tracker asks for a set of that name.

### Animation bindings (section 10)

| Size | Type | Meaning |
|---|---|---|
| per child slot 0 .. C − 1 | | u32 B = number of bindings for that slot, then B × binding |
| binding | | String animation name; Vector box (half-extents, u); Vector root translation (u) |

- Slot 0 lists the file's own animations, in order, one binding each (B = A). Slots for real children list nothing in every file of the install (B = 0), because each child carries its own slot-0 bindings.
- The box and translation of a binding replace the animation record's default box and a translation of (0, 0, 0). The record's own box always equals its binding's box in the install.

## Building a model's animation list

When a model and its children are loaded, the model's playable list is built once:

1. Entries 0 .. A_self − 1: the model's own animations, in file order. Each entry takes box and root translation from the model's slot-0 binding of the same position.
2. Then, for each child slot 1 .. C − 1 in order whose child loaded: all entries of the child's own list (built the same way, so with the child's slot-0 bindings), in the child's order. The first child entry's index is that child's **animation offset**.
3. Bindings listed in the parent's slot for that child (none in the install) would then override box and translation of the child entries, matched by name.

Lookup by name returns the **first** entry whose name matches without regard to case. A child animation with the same name as an own animation is therefore shadowed (both the weapon `PV_` and `ANI_` files and the character body and child contain such pairs only for the bind-pose animation names, which nothing looks up).

Animation indices are positions in this combined list. The game resolves names to indices once per model and plays by index.

The **box** of an entry is what the game reads as an animation's dimensions: the movement code takes the standing hull from the `LST` entry and the crouching hull from the `LC` entry (`movement.md`: 15 × 50 × 15 and 15 × 40 × 15 half-extents for the default character).

## Sampling one animation

Given an animation with keys $t_0 \le \dots \le t_{N-1}$, the pose of node j at time t (0 ≤ t ≤ L) as the playback code arrives at it:

1. Let $a$ be the last key with $t_a < t$ (if t = 0, use $a = 0$ and $b = 0$). Let $b = a + 1$ (if $a = N - 1$, $b = a$).
2. The fraction is

```math
u = \begin{cases} \dfrac{t - t_a}{t_b - t_a} & t_b > t_a \\ \text{see Playback} & \text{otherwise}\end{cases}
```

3. With $p_a, q_a$ and $p_b, q_b$ the channel values at keys a and b:

```math
p(t) = p_a + (p_b - p_a)\,u,\qquad q(t) = \mathrm{slerp}(q_a, q_b, u)
```

4. **Slerp** (no renormalisation of the result):

```math
c = q_a \cdot q_b;\quad \text{if } c < 0:\ c \leftarrow -c,\ q_b \leftarrow -q_b
```
```math
\text{if } 1 - c > 0.0001:\ \ \Omega = \arccos c,\ \ w_a = \frac{\sin((1-u)\Omega)}{\sin\Omega},\ \ w_b = \frac{\sin(u\Omega)}{\sin\Omega};\qquad \text{else } w_a = 1 - u,\ w_b = u
```
```math
\mathrm{slerp}(q_a, q_b, u) = w_a\,q_a + w_b\,q_b
```

5. If node j is the **root node** (node 0), add the root translation of the animation's entry to the position. During a transition between two entries, the two translations are interpolated with the transition fraction:

```math
p_{root} \mathrel{+}= T_{prev} + (T_{cur} - T_{prev})\,u
```

  First-person weapons: every `ANI_` animation of the M92FS has root translation (−0.4, −3.6, −1.2). Characters: every child animation has (0, −50, 0), so the root sits 50 u below the model origin (the origin is the hull centre, the feet are at −50).

At $t = t_b$ exactly the result equals key b. Because $a$ is the last key **strictly** before t, a run of keys sharing one time is passed over in one step.

## Playback

Playback state lives in a **tracker**. A model has one main tracker and may have more (Layered playback). A tracker holds: the current entry and the previous entry (equal unless a transition is running), the current time t (ms, integer), the key indices a and b, the fraction u, flags (playing, looping), a rate r (default 1.0), a rate carry c (default 0), the transition elapsed time (or "not transitioning"), a weight set and a weight multiplier (default 1.0).

A new tracker starts on entry 0, playing, **looping**, at t = 0, not transitioning.

### Time base and rate

- The client measures frame time with a high-resolution clock converted to whole milliseconds since start; a frame's Δ is the difference of two such readings, so no time is lost over many frames. Model animation is updated once per client frame, only if Δ > 0, before the frame's transforms are evaluated. Paused models are not updated. Trackers that are not playing do not advance.
- Rate-scaled step, with `AniRateInterpolation` = 1 (default):

```math
\Delta' = \lfloor \Delta \cdot r + c \rfloor,\qquad c \leftarrow (\Delta \cdot r + c) - \Delta'
```

  With r = 1 the step is exactly Δ. With `AniRateInterpolation` = 0, $\Delta' = \lfloor \Delta \cdot r\rfloor$ and the fraction is lost every frame. Setting a rate resets c to 0.
- The game sets first-person weapon rates per state (`weapon_firing.md`, `inventory.md`); a rate above 2.0 becomes 1.0.

### Key scan (one update of one tracker, after any transition step)

1. If the entry has N ≤ 1: t = 0, u = 0, nothing else happens.
2. t ← t + Δ'. If not looping, t ← min(t, L + 1).
3. Walk the keys from the tracker's next unvisited key: for each key i with $t_i < t$, fire key i's string (if non-empty, see String keys), then set a = i, b = i + 1 (or a = b = i at the last key). Stop at the first key with $t_i \ge t$.
4. If looping and every key has been visited: t ← t mod L (t ← 0 if L = 0), and the next unvisited key becomes key 0.
5. Fraction: if a ≠ b, $u = (t - t_a)/(t_b - t_a)$. If a = b (the last key was visited): looping → $u = \min(t / t_{N-1}, 1)$ (0 if $t_{N-1} = 0$), which has no visible effect because both keys are the same; not looping → u = 1.

Setting a time directly (the game does this to keep upper and lower body in step) re-scans from key 0 **without firing strings**.

### Looping

- A looping tracker never counts as done.
- When t passes L the last key's string fires, t wraps by one modulo, and the key walk restarts at key 0 **on the next update**. Key 0's string (for example a weapon's `FIRE_KEY` at 0) therefore fires once per loop, on the first update after the wrap.
- For that one update the pose shows the last key (a = b = last), although t has already wrapped (Quirks).
- Only one wrap per update: if Δ' > 2L, keys of the skipped loops do not fire.

### End of play

A non-looping tracker is **done** when $t > L$, which needs $t = L + 1$ (the clamp). An entry with N ≤ 1 is always done. The game polls "done" to chain animations (`weapon_firing.md`: the fire cycle restarts on the update after the fire animation reports done). The last key's string fires on the same update that makes the tracker done.

### Starting an animation and transitions

Starting entry e on a tracker (the game always asks for a transition):

1. Previous ← current (entry, keys, time). Current ← e at t = 0, a = b = 0, u = 0, next unvisited key 0. Transition elapsed ← 0.
2. On the next update, before the key scan, with $I$ = the **new** entry's interpolation time:
   - If $I = 0$: the transition ends at once (previous ← current) and the key scan runs with the full Δ'.
   - Else elapsed ← elapsed + Δ'. If elapsed > I: the tracker resets to t = 0 at key 0, the transition ends, and the key scan runs with the leftover Δ' = elapsed − I. Otherwise u ← elapsed / I and the update ends here: **time does not advance and no strings fire** while blending.
3. While a transition runs, the pose of each node blends the **previous** entry at its key b (the key it was heading for when it was interrupted, not its interpolated pose) with the **new** entry at key 0, using u: positions linearly, rotations by slerp, root translations as in Sampling.

"Reset" (the game's restart of the current animation) sets previous ← current entry, t = 0, keys 0, u = 0, but leaves a running transition's elapsed time alone.

First-person weapon animations all have I = 0, so weapon switches between animations are instant. Character animations have I = 0 (1,019 of 1,490), 10, 20, 50, 100 (363) or 200 ms (96) in the US reference build.

## String keys

### How a key string becomes events

- A key with an empty string does nothing. Strings fire only from the key scan of a normal update, for every tracker of the model, in tracker order; the game sees them on the client only for models that asked for key notifications (view models and characters do).
- The string is split into **commands** at `;` and at any control character (bytes 0 to 31 and 127). Each command is split into **arguments** at spaces. A double quote or an opening parenthesis starts a grouped argument that runs to the matching close (spaces kept, the delimiters dropped); `%` makes the next character literal. Each command with at least one argument is delivered as (argument count, arguments). Commands and keywords are compared without regard to case.
- If a handler changes the tracker's animation while handling a key, the rest of that key's processing is skipped.

### First-person weapon keys

Delivered to the current weapon only when the key comes from its own view model.

| Keyword | Arguments | Effect |
|---|---|---|
| `FIRE_KEY` | | Raises one shot or melee hit (`weapon_firing.md`, `inventory.md`). Fire animations put it at 0 ms; knife swings put one per hit. |
| `ALT_FIRE_KEY` | | Same, for the left gun of dual pistols |
| `SOUND_KEY` | id | Plays one of the weapon's sounds and tells other players: 1–3 reload sounds 1–3, 4 select sound, 5 deselect sound, 10–14 misc sounds 1–5. Other ids do nothing. |
| `BUTE_SOUND_KEY` | name | Plays the named sound locally only |
| `LOOP_SOUND_KEY` | id or `STOP` | Starts a looping weapon sound (ids as above, plus 6 fire, 7 dry fire, 8 alt fire, 9 silenced fire by the muzzle mod's silencer level), replacing a different one; `STOP` ends it |
| `FX` | names | Spawns the named weapon effects |
| `FIREFX_KEY` | names | Raises a shot if a fire animation is playing and the weapon is firing, and spawns the effects |
| `HIDE_PIECE_KEY` / `SHOW_PIECE_KEY` | piece names | Hides or shows those view-model pieces |
| `PIECE_VISIBLE` | | Piece visibility (attachment pieces) |
| `SHELL_CASING` | | Ejects a shell casing (first person, visible weapon only) |
| `HIDE_PVATTACHFX` / `SHOW_PVATTACHFX` | index | Hides or shows a view-model attached effect |
| `HIDE_PVATTACHMENT` / `SHOW_PVATTACHMENT` | index | Hides or shows a view-model attachment |
| `DEFLECT` | seconds | Tells the server the player is deflecting for that long |
| `RS` | n name [`RS` n name ...] | Sets render style n of the view model |

Unknown keywords are ignored. The M92FS `ANI_` file uses `sound_key 1`/`2`/`3`, `FIRE_KEY` and `dryfire_key`; the last one matches nothing and does nothing.

### Character keys (third person)

| Keyword | Arguments | Effect |
|---|---|---|
| `FOOTSTEP_KEY` | [foot] | Footstep sound and effects. Argument `1` = right foot, any other value = left foot, none = alternate feet. |
| `FOOTPRINT_KEY` | [foot] | Footprint decal, same foot rule |
| `MOVE_LOUD_KEY` / `MOVE_QUIET_KEY` | | Movement sounds |
| `SHOW_ATTACHFX` / `HIDE_ATTACHFX` | n | Attached effects |
| `FX` | name | Effect |
| `PIECE_VISIBLE` | attachment, piece | Shows a piece of an attachment |
| `RS` | n name | Render style |
| `FIRE`, `THROW` | | Used by the server's computer-controlled soldiers only (moment of firing or throwing) |

The male child animations carry 115 `FOOTSTEP_KEY`, 96 `FOOTPRINT_KEY`, 253 `FIRE` and 3 `THROW` keys (US reference build).

## Layered playback (trackers and weight sets)

- A model's trackers form a list: the main tracker first, then added trackers in the order they were added. All advance every frame (in list order) with the same Δ but their own rate, and each fires its own key strings. At most 8 take part in a pose; with more, no pose is computed.
- Every tracker after the first must have a weight set that exists in the model; if any has none or an invalid one, the pose is not computed at all for that frame (Quirks). The game "disables" a tracker by asking for a weight set named `Null`. The character models have no such set, so for players and soldiers the request fails and the tracker keeps its old set; only some template-based props have a `Null` set (all weights 0).
- Weight sets are always the **model's own** (the body's), indexed by node, never the child's.
- Per node j, the local pose is:
  1. $p, q$ ← the main tracker's sampled pose (its weight set is ignored).
  2. For each further tracker i in list order, with $m_i$ its weight multiplier and $w = W_i[j]$ its set's weight for j:
     - if $m_i = 0$ or $w = 0$: skip;
     - if $w = 2.0$ (additive): let $(p_t, q_t)$ be the tracker's sampled pose and $(p_0, q_0)$ the tracker's previous entry at key 0; then

```math
q \leftarrow q \cdot \left(q_0^{-1}\, q_t\right),\qquad p \leftarrow p + (p_t - p_0)
```

     - otherwise, with $f = w\, m_i$ and $(p_t, q_t)$ the tracker's sampled pose (root translation included):

```math
q \leftarrow \mathrm{slerp}(q, q_t, f),\qquad p \leftarrow p + (p_t - p)\, f
```

- Weights are not clamped; the multiplier does not apply to additive nodes.

**How players use it** (details of which animation goes where are `characters.md`'s): the player's model gets, after the main tracker, an `Upper` tracker (weight set `upper`: 1 on the torso, arms and head, 0 elsewhere), a `Lower` tracker (set `Lower`: 1 on the root, hips and legs, 0.7 and 0.3 on the two nodes between hips and torso, 0 elsewhere) and a crouch tracker that also uses `Lower` and whose multiplier is set every frame to the crouch fraction. Upper-body animations are named `U*`, leg animations `L*` (for example `LST` stand, `LC` crouch, `LWF` walk forward, `LRF` run forward). The game copies the lower tracker's time to the upper tracker when the movement changes, so legs and torso stay in step. The lower tracker's entry gives the hull box.

The default character's sets (node index: name: upper / Lower): 0–3 (root, mesh node, lower body, left pelvis): 0 / 1; 4 (right pelvis): 0 / 0.7; 5 (torso): 1 / 0.3; 6–38 (upper torso, neck, head, face, shoulders, arms, hands, fingers, one helper node): 1 / 0; 39–52 (legs, feet, toes and six helper nodes): 0 / 1. A third set `blend` holds 0.3 on nodes 4 and 5 and 0 elsewhere; the player animator does not use it.

**How NPCs use it** (`characters.md` 13.5): the same upper and lower trackers on the body's `upper` / `Lower` sets, but no crouch tracker. Two NPC kinds differ: bug-type NPCs ask for a set named `Whole` on both trackers (no body has one), and the abnormal boss adds a third tracker on its body's `AL` set (nodes `Bone_AL01`, `Bone_AL02`) that loops the animation `AL`. The boss-battle bodies carry an `Upper` set of all ones and a `Lower` set of all zeros, so whatever the upper tracker plays is the whole pose.

## Node evaluation order and the node transform

- Nodes are evaluated depth-first from the root (node 0), parent before children, children in file order, which is node-index order. The engine evaluates lazily: a request for one node (a socket, a hit sphere) evaluates only the path from the root to it; drawing evaluates every node. Results are cached until the next animation update or tracker change.
- Local matrix of node j from the blended pose $(p_j, q_j)$:

```math
L_j = \begin{pmatrix} R(q_j) & p_j \\ 0 & 1 \end{pmatrix}
```

  If the node's flags have bit 1 (value 2, "rotation only") set, $p_j$ is replaced by the node's bind offset from its parent: the translation part of $B_{parent}^{-1} B_j$, where B are the bind-pose global matrices from the node section. 26 nodes in the readable archives set that flag; none in the M92FS pair or the default character.
- Global matrix:

```math
G_{root} = M_{obj}\, L_{root},\qquad G_j = G_{parent(j)}\, L_j
```

  $M_{obj}$ is the object's world matrix (position, rotation, scale) for drawing; for the first-person weapon it is the view-model placement (`L12`'s spec). Game code can hook individual nodes after their global matrix is computed (aim pitch, head turning; `characters.md`); hooked changes propagate to the children.
- Animation channels hold **local** (parent-relative) transforms. The bind matrices in the file are model-space globals; the bind local of node j is $B_{parent}^{-1} B_j$.

## Skinning

- Rendering transform of node j:

```math
S_j = G_j\, B_j^{-1}
```

  so that a vertex in bind pose follows its bone from the bind pose to the animated pose.
- A skinned vertex with bind-pose position $v$, bones $j_1 \dots j_n$ ($n \le 4$) and stored weights $w_1 \dots w_{n-1}$ (the last weight is implied):

```math
w_n = 1 - \sum_{k=1}^{n-1} w_k,\qquad
v' = \sum_{k=1}^{n} w_k\, S_{j_k}\, v
```

  Normals use the same blend of the matrices' rotation parts (the engine uses the fixed-function vertex-blending pipeline; no non-uniform scale occurs in the install). Which bones a triangle group uses and how weights and indices are stored per vertex is `models_ltb.md`'s.
- A rigid piece attached to one node j: $v' = S_j v$.
- An additional model (hands) is skinned with the host's $S_j$, after its bone indices have been remapped by name (Where animations are stored).

## First-person weapon animations

The view model is the weapon's `PV_` model (its attribute `PVModelNormal`, for example `GUNS_M_PV_Pistol\PV_PST_M92FS_SH.ltb`), whose child `ANI_` model supplies the animations. The weapon looks these names up once (case ignored); a missing one leaves that step out:

| Name | Role | Played |
|---|---|---|
| `select` | Draw | On switching to the weapon. Draw time = its length / rate (`inventory.md`). |
| `deselect` | Holster | Only if present; none of the 146 readable `ANI_` files has it, so holstering is instant. |
| `idle_0` | Subtle idle | Whenever nothing else plays and no full idle is due; restarted each time it finishes (played non-looping). |
| `idle_1` | Full idle | When a random wait of 5 to 15 s has passed, the current animation is done and the player is not moving. A list of idles (index ≥ 1) is supported; only `idle_1` exists. |
| `prefire` | Before the first shot | Only if non-empty (`weapon_firing.md`); a 2-key 0 ms `prefire` lasts one update. |
| `fire_0` | One shot or swing | Restarted per round; cycle = its length / rate. Its `FIRE_KEY` raises the shot. |
| `alt_fire_0` | Alternate attack | Knife stab, dual pistols' left gun |
| `postfire` | After the last shot | If present (`inventory.md`: pistol 150 ms) |
| `dryFire` | Empty trigger | Matches `dryfire` in the files |
| `reload` | Magazine reload | Clip filled when it is done; its length / rate is the reload time |
| `reload_1` | Second reload | Double-barrel guns with rounds left in the clip |
| `PreReload`, `PostReload` | Per-round reloads | Shotguns: `PreReload`, then `reload` once per round, `PostReload` at the end |
| `PreRush`, `rush`, `PostRush` | Sprint in, sprint loop, sprint out | `movement.md` / `inventory.md` sprint timings |

The readable `ANI_` files also contain `fire_1` (22 files), `alt_prefire`, `alt_fire_1`, `alt_postfire`, `alt_dryfire` (4 each) and two misspelt `_fire_0`/`_fire_1`; the weapon code read for this spec never looks these names up (Open question 9).

All weapon animations are played non-looping, from t = 0, with interpolation time 0. Values that other specs take from these lengths and keys:

| Spec value | Comes from |
|---|---|
| Draw time | `select` length |
| Fire cycle | `fire_0` length (plus `prefire`/`postfire` around a burst) |
| Reload time | `reload` length (or per-round lengths) |
| Sprint-in, sprint-out | `PreRush`, `PostRush` lengths |
| Knife hit times | `FIRE_KEY` times in the knife's fire and alt-fire animations |
| Reload and draw sounds | `SOUND_KEY` times |

Each of these is observed at frame resolution and needs one more millisecond than the length to report done (Quirks).

## What the install contains

Steam build 25595979, all **unkeyed** archives (the keyed ones, which hold characters, most first-person guns, knives and grenades, could not be read):

- 815 binary model files read: 161 of version 23 and 654 of version 25. The animation data of both versions is identical in layout. One more FX model (`FX\WATERFALL2.LTB`) has version 18, below the engine's minimum, and cannot load; six FX files under the `.LTB` extension use a different text-headed format and are not models the engine reads (`models_ltb.md`). 65 render-style files share the extension but have container byte 0 = 5.
- 2,622 animations: compression 0: 806, 1: 1, 2: 115, 3: 1,700. Interpolation times: 0 (1,955), 10 (27), 50 (3), 200 (637).
- Node channel counts: 1 or N everywhere, plus 0 only on the 244 vertex-animated nodes (38 files). Every animation starts at time 0; 407 animations have 0 or 1 key.
- 181 models have child models; 26 have weight sets (24 of them the FX debris template above).
- 146 readable first-person `ANI_` files. Every one has `select`; 142 have the full set `idle_0`, `idle_1`, `prefire`, `fire_0`, `postfire`, `reload`, `PreReload`, `PostReload`, `PreRush`, `rush`, `PostRush` (140 also `dryfire`). The M92FS pair: version 25, compression 3, interpolation 0, box (2, 2, 2).
- The character files are in the V2-keyed body archive. The default character body (`CM_VST_NM_DF_NUTS_JS.LTB`) has the same size in the Steam install as in the US reference build (429,442 bytes); the male child `ST_M_CHILD.LTB` does not (66,900,324 bytes on Steam, 63,225,013 in the US build), so the Steam animation set is newer.

## Per-frame order

1. The client clock ticks; Δ = whole milliseconds since the previous frame.
2. If Δ > 0, for every model that is not paused, for each of its trackers in list order: if playing, compute Δ', run the transition step, then the key scan (firing strings, which the game handles right away), then the loop wrap and the fraction.
3. Cached node transforms are marked stale.
4. Game logic for the frame runs (weapon state machine polls "done", starts animations, sets rates and times; `weapon_firing.md`, `inventory.md`).
5. Transforms are evaluated on demand (sockets, hit spheres) and for drawing; skinning and vertex animation happen at draw.

## Edge cases

- **N = 0 or 1**: the entry never advances, its pose is key 0 (or identity with N = 0), it is always "done", and its strings never fire.
- **Zero-length animation with 2+ keys** (all keys at 0, such as the pistol `prefire`): non-looping, the first update clamps t to 1, fires every key's string and reports done. Looping, t stays 0 after each wrap and the keys fire once per update.
- **Several keys at one time**: all fire in key order in the same update; sampling passes over the run.
- **A key exactly at the current time** has not fired yet; it fires on the update that takes t past it. A key at 0 fires on the first update with Δ' > 0.
- **Very large Δ' on a non-looping tracker**: clamped to L + 1; all remaining strings fire in one update.
- **Rate 0**: no advance; the carry keeps any fraction.
- **Transition into an animation with I = 0**: none; the new animation advances by the full Δ' of the next update.
- **Transition longer than the frame**: the new animation's strings, including key 0's, fire only after the transition has ended.
- **Child missing**: its animations are absent from the list; name lookups for them fail and the weapon or character skips those steps.
- **Additive weight on a tracker playing a transition**: the base is key 0 of the tracker's previous entry.

## Quirks

1. **Done needs L + 1 ms.** A 650 ms draw is reported done only once 651 ms have accumulated, so at 16 ms frames it ends on the 41st update (656 ms of frame time; the tracker's time clamps to 651). Keep: other specs' frame-rounding statements already assume "the first frame after the animation reports done".
2. **One stale frame after a loop wrap.** On the update that wraps, the pose shows the last key; interpolation resumes on the next update. Keep for fidelity; it is invisible on animations whose last key equals the first.
3. **Transitions start from the interrupted animation's next key**, not its current interpolated pose, so a transition can pop slightly at its start. Keep.
4. **The frame right after a non-reset switch.** Until the next update, a tracker that was switched shows the previous entry's next key (u = 0 of the transition), even with I = 0. Whether the game's frame order makes this visible is Open question 2.
5. **An invalid weight set on any extra tracker freezes the whole model's pose** (no pose is computed). Our implementation should log and skip that tracker instead; nothing in the stock game relies on the freeze.
6. **Disabling a tracker via the `Null` weight set does nothing** because no model has that set. The game relies on this only in the sense that trackers stay on their last set; keep.
7. **`ModelTransitionMS` does nothing.** Keep the per-animation interpolation time as the only source.
8. **Rotations are not renormalised** after 16-bit decoding or slerp; the matrix formula divides by $|q|^2$. Use the same formula, or normalise before building the matrix (the result is the same rotation).

## Test cases

Run against the coordinator's copy of the install. Weapon cases (1–10) read unkeyed archives of Steam build 25595979. Character cases (11–14) were measured on the US reference build 1501.04 and need the V2 key on Steam; on Steam the counts and values may differ (the male child is a different size), so they are first a parser check and become exact checks once compared against the Steam file. Positions are in u, rotations as (x, y, z, w); compare with a tolerance of 1e-4.

1. **M92FS view model** `GUNS_M_PV_PISTOL\PV_PST_M92FS_SH.LTB` in `GUNS_M_PV_Pistol.rez`: 60,570 bytes, container byte 0 = 1, container version 9, model version 25. 51 nodes, no weight sets, child models C = 2 with child 1 = `Ani_PST_M92FS_SH.ltb`. The animation section starts at byte 58,852 and holds 1 animation: `CM_HND_SP_DF_Beretta_M92F_FS_CH`, compression 0, interpolation 200, 1 key at 0, every node channel holds one value. Sockets `flash` and `breach` on node 27. Slot-0 bindings: that one animation with box (2, 2, 2) and translation (0, 0, 0); slot 1: none. The file ends exactly after the bindings.
2. **M92FS animation file** `GUNS_M_PV_PISTOL\ANI_PST_M92FS_SH.LTB`: 191,067 bytes, version 25, 51 nodes with the same parent links as the view model (names differ on nodes 24–31), header counts: 92 keys, 13 animations, 41,192 bytes of channel data. Animation section at byte 143,066. All 13 animations: compression 3, interpolation 0, box (2, 2, 2); slot-0 bindings give every one root translation (−0.4, −3.6, −1.2).
3. **Animation list of the M92FS view model** after loading its child: 14 entries; index 0 is `CM_HND_SP_DF_Beretta_M92F_FS_CH`, then in this order (name: keys, length ms): `select` 19, 650; `idle_0` 3, 1566; `idle_1` 3, 1566; `prefire` 2, 0; `fire_0` 5, 180; `postfire` 2, 150; `reload` 29, 1500; `dryfire` 6, 300; `PreReload` 6, 300; `PostReload` 6, 300; `PreRush` 4, 250; `rush` 3, 1500; `PostRush` 4, 400. Lookup of `SELECT` returns 1, `dryFire` returns 8, `deselect` and `alt_fire_0` fail.
4. **Key times**: `select` keys at 0, 98, 157, 197, 216, 236, 275, 296, 335, 354, 393, 433, 453, 492, 532, 552, 591, 610, 650; `fire_0` at 0, 29, 52, 103, 180; `PostRush` at 0, 167, 267, 400.
5. **String events**: `select`: 296 `sound_key 3`. `fire_0`: 0 `FIRE_KEY`. `reload`: 226 `sound_key 2`, 453 `sound_key 1`, 962 `sound_key 3`. `dryfire`, `PreReload`, `PostReload`: 0 `dryfire_key`. All other keys of the file are empty.
6. **Channel counts** in `select`: node 0 `Scene Root` 1 position (0, 0, 0) and 1 rotation (0, 0, 0, 1); node 6 `Bip01 R UpperArm` 1 position, 19 rotations; node 7 `Bip01 R Forearm` 19 and 19; node 9 `Bip01 R Finger0` 19 and 1. Node 1 `Bip01`: 1 position (0, 0.3553, 0), 1 rotation (−0.013916, −0.510971, −0.023408, 0.859157).
7. **Decoded rotation and interpolation**: `select`, node 7 `Bip01 R Forearm`. Key 1 (98 ms): position (1.4803, 0, 0), rotation (0, 0.565600, 0, 0.824671). Key 2 (157 ms): position (1.4803, 0, 0), rotation (0, 0.594409, 0, 0.804132). At t = 120 ms: u = 22/59 = 0.372881, position (1.4803, 0, 0), rotation (0, 0.576431, 0, 0.817138). At t = 650: rotation (0, 0.685263, 0, 0.728263).
8. **Decoded position**: `reload`, node 29 `BOP06` (the magazine): 29 positions and 1 rotation (identity). Key 5 (311 ms) (0, −0.9404, −0.5573), key 6 (425 ms) (0, −1.0358, −0.5743). At t = 368 ms: u = 0.5, position (0, −0.9881, −0.5658). Keys 0–3 and 9–28 are (0, −0.0432, −0.2202). Node 0 at any time of `reload`, root translation included: (−0.4, −3.6, −1.2).
9. **Frame-stepped playback of `select`**, non-looping, rate 1, started at t = 0, Δ = 16 ms per update: `sound_key 3` fires on update 19 (t = 304); done is first reported after update 41 (t clamps to 651; update 40 at t = 640 is not done). With Δ = 10 ms: the sound fires at t = 300 (update 30), done after update 66 (t clamps to 651; update 65 at t = 650 is not done).
10. **Looping wrap**: `idle_0` set to looping, Δ = 1000: after update 1, t = 1000, a = 1, b = 2, u = 400/966; after update 2, t = 2000 mod 1566 = 434 and the pose equals key 2; after update 3, t = 1434, a = 1, b = 2, u = 834/966. Same with rate 0.5 and Δ = 3 ms: steps are 1, 2, 1, 2, ... ms (carry 0.5).
11. **Default character body** (attribute `Model0` "Normal", `CHARS_M_BODY\CM_VST_NM_DF_Nuts_JS.ltb`, US build): 429,442 bytes, version 25, command string `ShadowEnable`, 53 nodes, weight sets `upper`, `Lower`, `blend` (values in Layered playback), child `ST_M_child.ltb`. Own animation: `baseAnim`, compression 0, 21 keys, length 4800, box (15, 50, 15).
12. **Male child** `CHARS_M_BODY\ST_M_CHILD.LTB` (US build): 63,225,013 bytes, version 25, 53 nodes with the same names as the body, header counts 42,141 keys, 1,490 animations, 62,537,244 bytes of channel data. All animations compression 0. Combined list of the default character: 1,491 entries; `LST` at index 6 (8 keys, 4000 ms, box (15, 50, 15)), `LC` at 24 (6 keys, 4000 ms, box (15, 40, 15)), `LRF` at 42 (13 keys, 600 ms), `LWF` at 52 (18 keys, 1000 ms). Every child animation except `baseAnim` has root translation (0, −50, 0).
13. **Character string events** (US build): `LRF`: keys 0, 50, 75, 150, 175, 225, 300, 350, 375, 450, 475, 525, 600; strings 50 `FOOTSTEP_KEY 2`, 350 `FOOTSTEP_KEY 1`, others empty. `LWF`: 188 `FOOTPRINT_KEY 1`, 750 `FOOTPRINT_KEY 2`.
14. **Character node** (US build): `LRF`, node 2 `_zN_LowerBody`, 13 positions and 13 rotations. Key 1 (50 ms): (−0.4343, 43.4696, 0.1979), (−0.008482, −0.533826, −0.002053, 0.845550). Key 2 (75 ms): (−0.3676, 42.8186, 0.1979), (−0.008484, −0.535038, −0.002041, 0.844783). At t = 60: u = 0.4, position (−0.4076, 43.2092, 0.1979), rotation (−0.008483, −0.534311, −0.002048, 0.845243).
15. **Vertex animation** `FX\WEAPONS\M16MINE\MINE_INDI_SH.LTB` in `FX.rez`: 21,107 bytes, version 25, 4 nodes, one animation `Exp_FAK_SH2`, compression 0, interpolation 0, 121 keys from 0 to 4000 (0, 33, 67, 100, ...). Nodes 0 and 1 carry 121 positions and rotations; nodes 2 and 3 are vertex-animated with 4 vertices per key. Node 2, vertex 0: key 0 (−7.8755, 47.1020, −0.0143), key 1 (−7.8644, 47.0233, −0.4265). Header counts: 121 keys, 1 animation, 11,616 bytes of vertex data, 6,776 bytes of channel data.
16. **Compression 2 in the wild**: `MODELS\DEFAULT.LTB` in `MODELS.rez`: 1,720 bytes, version 23, 1 node `null`, animation section at byte 1,620, one animation `x`: compression 2, interpolation 200, 2 keys (0, 6566), box (3, 3, 3), 1 position (0, 0, 0) and 1 rotation (0, 0, 0, 1); header channel data 14 bytes.
17. **Install statistics** (unkeyed archives): the numbers in "What the install contains" (815 models, 2,622 animations, compression and interpolation histograms, counts 0/1/N only).

## Synthetic fixture

One node, two keys, compression 2. A unit test can embed this record in the minimal model of `models_ltb.md` (one node, the animation section `u32 1` followed by this record, slot-0 bindings `u32 1`, `test`, box (16, 32, 16), translation (0, 0, 0)), or feed the record alone to the animation reader with a node count of 1.

| Offset | Size | Type | Value | Meaning |
|---|---|---|---|---|
| 0 | 12 | Vector | (16.0, 32.0, 16.0) | Box |
| 12 | 2 | u16 | 4 | Name length |
| 14 | 4 | bytes | `test` | Name |
| 18 | 4 | u32 | 2 | Compression: i16 positions and rotations |
| 22 | 4 | u32 | 150 | Interpolation time, ms |
| 26 | 4 | u32 | 2 | Keys |
| 30 | 4 | u32 | 0 | Key 0 time |
| 34 | 2 | u16 | 8 | Key 0 string length |
| 36 | 8 | bytes | `FIRE_KEY` | Key 0 string |
| 44 | 4 | u32 | 100 | Key 1 time |
| 48 | 2 | u16 | 0 | Key 1 string (empty) |
| 50 | 4 | u32 | 2 | Positions stored |
| 54 | 6 | i16 × 3 | (16, 32, −48) | Key 0 position → (1, 2, −3) |
| 60 | 6 | i16 × 3 | (48, 32, 80) | Key 1 position → (3, 2, 5) |
| 66 | 4 | u32 | 2 | Rotations stored |
| 70 | 8 | i16 × 4 | (0, 0, 0, 32767) | Key 0 rotation → (0, 0, 0, 1) |
| 78 | 8 | i16 × 4 | (0, 23170, 0, 23170) | Key 1 rotation → (0, 0.707114, 0, 0.707114), about 90° about +Y |

Record length 86 bytes; channel data 28 bytes (header field 14); keys 2 (field 0).

Expected samples (root translation 0):

| t (ms) | u | Position | Rotation (x, y, z, w) | Angle about +Y |
|---|---|---|---|---|
| 0 | 0 | (1, 2, −3) | (0, 0, 0, 1) | 0° |
| 25 | 0.25 | (1.5, 2, −1) | (0, 0.195092, 0, 0.980786) | 22.5° |
| 50 | 0.5 | (2, 2, 1) | (0, 0.382686, 0, 0.923881) | 45.0° |
| 75 | 0.75 | (2.5, 2, 3) | (0, 0.555575, 0, 0.831474) | 67.5° |
| 100 | 1 | (3, 2, 5) | (0, 0.707114, 0, 0.707114) | 90° |

- At t = 50 the local matrix has $m_{00} = m_{22} = 0.707104$, $m_{02} = 0.707110$, $m_{20} = -0.707110$, translation (2, 2, 1): the node's +Z axis points to (0.707, 0, 0.707) in the parent frame.
- Variants with the same values: compression 3 stores the positions as f32 × 3 (12 bytes each: (1, 2, −3), (3, 2, 5)) and keeps the i16 rotations (record 98 bytes, channel data 40); compression 0 replaces offsets 50 to 85 by a 0 byte, two f32 positions, then two f32 quaternions (0, 0, 0, 1) and (0, 0.7071068, 0, 0.7071068) (record 107 bytes, channel data 56). Samples agree to 1e-5 (the f32 quaternion is exactly unit).
- Constant channels: storing one position (1, 2, −3) and one rotation gives that value at every t; storing zero gives (0, 0, 0) and identity.
- Playback, non-looping, rate 1, Δ = 30: `FIRE_KEY` fires on update 1 (t = 30); after update 3, t = 90; update 4 clamps t to 101, done. Looping, Δ = 30: update 4 gives t = 120 mod 100 = 20 with the pose of key 1, update 5 fires `FIRE_KEY` again (t = 50).
- Transition: from any animation into this one, Δ = 60: after update 1 the blend fraction is 0.4 and t = 0; after update 2, 0.8; update 3 ends the transition (elapsed 180 > 150), and the leftover 30 ms play: t = 30, `FIRE_KEY` fires.
- Two-tracker blend: main tracker on this animation at t = 50, a second tracker with weight 0.5 for the node on an animation whose node is constant at position (0, 0, 0) and rotation (0, 0, 0, 1): result position (1, 1, 0.5), rotation slerp((0, 0.382686, 0, 0.923881), (0, 0, 0, 1), 0.5) = (0, 0.195092, 0, 0.980786) (22.5° about +Y).

## Open questions

1. **Steam character animations.** The default character's child model in the Steam build differs from the US reference build. Re-measure cases 11–14 with the V2 key (listed in the combat-arms plan) and replace the US numbers.
2. **Frame order of weapon logic versus the model update.** If the weapon logic runs after the frame's model update and before drawing (as `inventory.md` says), Quirk 4 shows the old animation's next key for one frame after every non-reset weapon switch. Check with a frame capture right after drawing a pistol, or ignore it if invisible.
3. **Version 23 versus 25.** The engine version read for this spec reads both the same way in the animation data; whether version 25 differs anywhere else is `models_ltb.md`'s question.
4. **Missing child models.** The client loader is asked to fail on missing children by default, but its child callback treats "not found" as non-fatal in the code path read here. Check by renaming a weapon's `ANI_` file in a test copy: does the weapon load without animations or fail?
5. **Channel counts other than 0, 1 and N.** Never seen; we reject such files. If a later install has them, compare the game's behaviour (it would read past the channel).
6. **Additive weights (2.0)** appear only in the FX debris template's `twitch` set; the formula comes from the engine and is untested against a visible case. Check on a dead body if a character model with a `twitch` set turns up in the keyed archives.
7. **Rotation-only nodes** exist (26 in the readable archives) but not in the weapon and character models checked. The bind-offset rule comes from the engine; list which models use it once the keyed archives can be read.
8. **Hand bone matching** is by name; whether every stock hand model's bone names match its weapons' (otherwise those bones would use the hand model's own indices) needs the V2-keyed hand archive.
9. **Unused weapon animation names.** `fire_1` and the `alt_prefire`/`alt_fire_1`/`alt_postfire`/`alt_dryfire` family exist in some `ANI_` files, but the weapon code read here only looks up `fire_0` and `alt_fire_0`. The Steam client may be newer and use them (knives with two swings, dual pistols); check by watching a weapon whose file has `fire_1` fire repeatedly.
