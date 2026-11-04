# Source (HL2:DM, CS:S): VertexLitGeneric with `$phong` (the "skin" shading, DX9, LDR)

Source basis: Valve's public Source SDK 2013 (current GitHub head), DirectX 9 standard shader library only. I read the vertex-lit model shader's parameter list and its set-up code (the rule that sends a material to the Phong path, the parameter defaults it fills in, the DX level fallbacks), the Phong path's own set-up code (texture loading and colour spaces, the constants it derives from the material, the cheaper variant it picks for simple materials), its shader model 2.0b/3.0 vertex and pixel shaders, and the shared include files for per-pixel model lighting (ambient cube, diffuse term, specular and rim terms, fresnel), per-vertex light attenuation and final output. The engine, the material system and the D3D9 shader API are **not** in the SDK: what they set (how the 4 lights are picked and packed, the light scale constants, how a parameter missing from the VMT reads) is marked *derived* or listed under Open questions. Material data: every VMT in the user's HL2:DM install (`hl2mp` and the shared `hl2` VPKs) and CS:S install (`cstrike` and its shared `hl2` VPKs), parsed in a scratch folder (nothing committed).
Status: draft

## Summary

- A VertexLitGeneric material with `$phong 1` **and** a normal map (or a `$lightwarptexture`, or `$basemapalphaphongmask 1`) is drawn by a separate per-pixel shader. Everything is in linear light, like [shaders.md](shaders.md) section 1.
- Its diffuse is **always half-Lambert**, whatever `$halflambert` says: each local light gives colour × attenuation × (½·N·L + ½)², per pixel with the normal-mapped normal, plus the ambient cube at that normal. A `$lightwarptexture` replaces the square with a ramp lookup (×2).
- Its specular is **Phong, not Blinn-Phong**: per light, (R·L)^k × clamp(N·L) × colour × attenuation, with R the view vector reflected about the normal. The sum is multiplied by a mask (normal-map alpha by default), a three-point **fresnel ramp** of (1 − N·V)² (`$phongfresnelranges`), `$phongboost`, and a tint (`$phongtint`, or the albedo through `$phongalbedotint`).
- `$rimlight` adds a rim term: the same per-light lobe with its own exponent, masked by (1 − N·V)⁴, merged with the specular by a per-channel **max**, plus the ambient cube seen **along the view direction**, which only shows on upward-facing surfaces.
- Stock content: **54 materials** reach this path, the same 54 in both installs. They all live in the shared HL2 content (`materials/models/`). CS:S's own content uses none. HL2:DM uses them for its weapon view models (17 materials) and the Combine soldier player models (5), so this spec matters for HL2:DM, and for CS:S only if HL2 models are mounted.

## Units and conventions

- Same as [shaders.md](shaders.md) "Units and conventions": Source units, Z up, colours 0..1 (byte b = b/255), "gamma" = stored, "linear" = decoded, pure 2.2 power for the material-parameter conversion.
- Tangent space as in shaders.md: x = texture u, y = texture v (image-down), z = out of the surface; DirectX-style normal maps, green not flipped.
- All maths is **per pixel** at mat_hdr_level 0 on shader model 2.0b or 3.0 hardware (the reference), except the light attenuation (per vertex, interpolated).
- Vectors, all in world space and unit length unless stated otherwise:
  - P: the pixel's world position.
  - N: the shading normal (section 3).
  - V = normalize(eye − P): toward the eye.
  - L_i = normalize(p_i − P): toward light i, from its position p_i (section 4 for directional lights).
  - R = 2·N·(N·V) − V: V reflected about N.
- clamp(x) means clamp to [0, 1].

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| max_lights | 4 | lights | local lights evaluated per pixel |
| phong_exponent_map_scale | 149 | - | exponent = 1 + 149 × exponent-map red when no constant exponent is set |
| phong_exponent_no_map | 150 | - | that rule with the white stand-in map (red = 1) |
| phong_fresnel_ranges_default | (0, 0.5, 1) | (min, mid, max) | `$phongfresnelranges` when unset |
| phong_boost_default | 1 | - | `$phongboost` when unset |
| phong_tint_default | none (white, or the albedo tint) | - | `$phongtint` when unset (section 6) |
| rim_exponent_default | 4 | - | `$rimlightexponent` when unset |
| rim_exponent_min | 1 | - | a set `$rimlightexponent` below 1 becomes 1 |
| rim_boost_default | 1 | - | `$rimlightboost` when unset |
| lightwarp_scale | 2 | linear factor | the lightwarp texel is doubled |
| fresnel_power | 2 | - | the specular and envmap fresnel ramp's input is (1 − N·V)² |
| rim_fresnel_power | 4 | - | the rim mask is (1 − N·V)⁴ |
| envmap_scale | 1 in LDR (engine-set) | - | as shaders.md open question 3 |

## Behavior

### 1. Which materials get this shading

The material's shader must be VertexLitGeneric and `$phong` must be non-zero. Then it is drawn as specified here when **any** of these holds:

- `$lightwarptexture` names a texture;
- `$basemapalphaphongmask` is exactly 1;
- `$bumpmap` names a texture.

Otherwise (`$phong 1` with no normal map, no lightwarp and no base-alpha mask), the material is drawn as plain VertexLitGeneric (shaders.md section 4) and every `$phong*` and `$rim*` parameter is ignored. In stock content that is 3 materials: `models/combine_helicopter/combine_helicopter02` and its gib copy, and the HL2:DM frag grenade's `models/weapons/v_grenade/grenade body`. That one is a view model material, so the frag grenade's body has no highlight while its top, handle and rim do.

**DX9 fallbacks.** Plain VertexLitGeneric (shaders.md section 4) draws a material that would get this shading in any of these cases:

- the hardware lacks pixel shader 2.0b;
- Phong is turned off in the video config (the material system's "disable phong" setting);
- bump mapping is turned off (mat_bumpmap 0).

In the first two cases `$phong` is cleared at load. Section 4's per-pixel bump variant is used because these materials have a `$bumpmap`. Below DX level 90 the DX8/DX7/DX6 versions of the shader are used, and they have no Phong at all.

**The cheap variant.** A material that has none of the following:

- a normal map;
- `$phongexponenttexture`;
- `$phongwarptexture`;
- `$rimlight`;
- `$detail`;
- `$selfillum`;
- `$blendtintbybasealpha`;

and that has either `$basemapalphaphongmask` set or a base texture without alpha, is drawn with a reduced variant (section 9). No stock material qualifies: all 54 have a normal map.

### 2. Textures and their colour spaces

| Input | Decode | When absent |
|---|---|---|
| `$basetexture` | sRGB → linear | white (1, 1, 1, 1) |
| `$bumpmap` (normal in rgb, specular mask in alpha) | raw | a flat normal (0, 0, 1) is bound; its alpha is not known (Open questions) |
| `$phongexponenttexture` (r exponent, g albedo-tint amount, a rim mask) | raw | white (1, 1, 1, 1) |
| `$lightwarptexture` | raw | not used |
| `$phongwarptexture` | raw | not used |
| `$envmap` | sRGB → linear (LDR) | not used |
| `$detail` | raw for mode 0, sRGB for others (as shaders.md) | not used |
| `$selfillummask` | raw | base alpha is the mask |

Texture coordinates:

- The base, normal, exponent and self-illum mask textures share the base coordinates (`$basetexturetransform`). `$bumptransform` is set up but the shader reads all of them at the base coordinates (Open questions).
- The detail coordinates are `$detailtexturetransform` × `$detailscale` when the transform is set, otherwise the base transform × `$detailscale`.

### 3. Albedo, normal and masks

Albedo:

- base = sampled `$basetexture` (linear).
- Detail combine (modes 0–6 only) as shaders.md section 2/4. `$detailtint` is **not applied** by this shader.
- A = base.rgb × modulation, where modulation is the material colour converted gamma → linear exactly as shaders.md section 4 (the 1/255 table, components > 1 pass through).
- With `$blendtintbybasealpha` (and no `$selfillum`): A = lerp(base.rgb, lerp(base.rgb × modulation, modulation, `$blendtintcoloroverbase`), base.a).
- Vertex colour (`$vertexcolor`, `$vertexalpha`) is **ignored** by this shader.

Normal and specular mask:

- Default: n_t = 2·normal.rgb − 1 (not renormalised); mask = normal.a.
- `$basemapalphaphongmask 1`: n_t = (0, 0, 1) **even when a normal map is set**, and mask = base.a. The normal map is then not used at all: the surface shades with its interpolated vertex normal.
- World normal: N = normalize(T·n_t.x + B·n_t.y + N_v·n_t.z). T, B and N_v are the model's per-vertex tangent, binormal and normal after skinning. Each was normalised per vertex, then interpolated, and they are not renormalised per pixel before this sum.
- `$invertphongmask` does **not** invert the specular mask (see section 8 and Quirks).

### 4. Diffuse lighting (ambient cube and local lights)

```math
D = \text{ambient}(N) + \sum_{i<n} C_i \cdot a_i \cdot w(N \cdot L_i)
```

- **ambient(N):** the six-colour ambient cube of shaders.md section 4, evaluated **per pixel** with the normal-mapped N. If the engine says the model has no ambient light, all six colours are black.
- **n:** 0–4 lights, as the engine supplies them, with linear colours C_i.
- **a_i, the attenuation:** computed per vertex exactly as shaders.md section 4 (1/(c + l·d + q·d²), × the spot cone factor for spots, 1 for directional lights) and interpolated across the triangle.
- **L_i:** recomputed per pixel as normalize(p_i − P) for every light type. For a directional light the engine must pass a position far along the reverse of its direction (*derived*, Open questions).
- **w, the diffuse term:**
  - Normal (no lightwarp): w = (clamp(½·N·L + ½))². This is half-Lambert, **always**. `$halflambert` has no effect here (25 of the 54 stock materials set it anyway).
  - With `$lightwarptexture`: u = clamp(½·N·L + ½) (not squared); w = 2 × warp(u).rgb. That is the warp texture's raw colour at horizontal coordinate u on its first row, and the result is a colour, so it can tint. The cheat convar r_lightwarpidentity 1 replaces the warp with an identity ramp.
- The ambient is neither half-Lambert nor warped.
- No overbright factor applies, as in shaders.md section 4.

diffuse = A × D.

### 5. Specular exponent

The exponent k is picked in this order:

1. `$phongexponentfactor` ≠ 0: k = 1 + `$phongexponentfactor` × exp.r.
2. Otherwise, `$phongexponent` set and > 0: k = `$phongexponent`. The constant overrides the map's red channel even when `$phongexponenttexture` is set; stock gman materials set both.
3. Otherwise: k = 1 + 149 × exp.r. With no exponent texture (white), that gives k = 150.

exp is the `$phongexponenttexture` sample (raw).

### 6. Specular tint

The tint S_t multiplies the final specular and rim (section 7):

- `$phongtint` set to anything but (0, 0, 0): S_t = `$phongtint`, raw. It is **not** gamma-converted.
- Otherwise, with `$phongexponenttexture` set and `$phongalbedotint` non-zero: S_t = lerp((1, 1, 1), base.rgb, exp.g). base.rgb is the linear base colour after detail and before modulation.
- Otherwise: S_t = (1, 1, 1).
- `$phongalbedotint` without an exponent texture has no effect (stock: v_pistol_sheet, vshotgun_albedo).
- No stock material sets `$phongtint`. 14 of the 54 set `$phongalbedotint 1`; 12 of those have an exponent texture, so the tint takes effect.

### 7. Specular and rim

Fresnel ramp. Given `$phongfresnelranges` = (m₀, m₁, m₂) (min, mid, max):

```math
f = \text{clamp}(1 - N\cdot V)^2,\qquad
F = \begin{cases} m_0 + (m_1 - m_0)\cdot 2f & f < 0.5 \\ m_1 + (m_2 - m_1)\cdot(2f - 1) & f \ge 0.5 \end{cases}
```

- F = m₀ facing the eye, m₁ at f = ½ (N·V ≈ 0.2929), m₂ edge-on.
- The ramp is not clamped. Stock values reach F = 5 (`[.1 5 1]` on the HL2:DM pistol and shotgun view models: the mid is 5, so the highlight is brightest at mid angles, not at the edge).

Per-light lobes:

```math
s_i = \text{clamp}(R\cdot L_i)^{k}\cdot \text{clamp}(N\cdot L_i)\cdot C_i\, a_i
\qquad
r_i = \text{clamp}(R\cdot L_i)^{k_r}\cdot \text{clamp}(N\cdot L_i)\cdot C_i\, a_i
```

- The lobe is masked by plain clamped N·L, not half-Lambert.
- With `$phongwarptexture`: s_i is additionally × warp(u = s_i before the warp, v = F).rgb, raw.
- S = Σ s_i, Rm = Σ r_i (rim only with `$rimlight`).

Combine, in this order:

1. Without a phong warp, the mask takes the fresnel: mask' = mask × F. With a phong warp, mask' = mask, because F already went into the warp lookup.
2. S ×= mask' × `$phongboost`.
3. If rim lighting is on (`$rimlight` non-zero and convar r_rimlight 1, its default):
   - ρ = rim_mask × clamp(1 − N·V)⁴. rim_mask = exp.a when `$rimmask` is set and an exponent texture exists, otherwise 1.
   - Rm ×= ρ.
   - S = max(S, Rm), per channel.
   - S += ambient(V) × `$rimlightboost` × clamp(ρ × N_z). ambient(V) is the ambient cube evaluated at the **view vector V** (not N), and N_z is the world-space up component of N.
   - k_r = max(`$rimlightexponent`, 1), or 4 when unset.
4. specular = S × S_t (the tint applies to the rim too).

The rim's ambient part is zero on surfaces facing sideways or down (N_z ≤ 0). Most of the rim glow on a character's shoulders and head comes from it.

### 8. Environment map

- Only with `$envmap`. There is none in the flashlight pass.
- R as above (normal-mapped), cube = sampled `$envmap` (linear).
- Mask:
  - m_e = base.a by default. This is **not** inverted, unlike shaders.md's `$basealphaenvmapmask`, and the flag is not needed.
  - m_e = the specular mask (section 3) with `$normalmapalphaenvmapmask`.
  - m_e = `$invertphongmask`'s 0/1 value with `$selfillumfresnel`.
  - `$invertphongmask` then turns m_e into 1 − m_e.
- env = envmap_scale × lerp(1, F, `$envmapfresnel`) × m_e × cube × `$envmaptint`. F is section 7's phong fresnel ramp.
- `$envmaptint` is used **raw** (shaders.md's model path gamma-converts it).
- `$envmapcontrast`, `$envmapsaturation` and `$envmapmask` do nothing on this path.
- A base texture without alpha has base.a = 1, so the reflection is unmasked unless `$normalmapalphaenvmapmask` is set.
- 13 stock materials set `$envmap`; 11 of them use the normal alpha, and 8 set `$envmapfresnel 1`. antlionhigh_sheet misspells it as `$envmapfrensel`, which does nothing.

### 9. The cheap variant (no normal map)

For a material picked in section 1's last paragraph:

- N is the interpolated vertex normal; mask = base.a.
- k = max(c, 0), where c is `$phongexponentfactor` if non-zero, else `$phongexponent` if > 0, else −1. **No exponent set means k = 0**: the lobe is then 1 wherever R·L > 0 (Quirks).
- S_t = (1, 1, 1). `$phongtint` and `$phongalbedotint` are ignored.
- No rim.
- Fresnel, boost, diffuse (including a lightwarp), envmap and alpha as above.

### 10. Self-illumination, detail after lighting, alpha, output

- `$selfillum` (and not the flashlight pass): diffuse = max(0, lerp(diffuse, `$selfillumtint` × A, m_s)).
  - m_s = base.a, or the `$selfillummask` rgb when one is set.
  - `$selfillumtint` is raw.
  - A base texture without alpha clears `$selfillum` at load unless a mask texture or `$selfillumfresnel` is set.
  - 15 stock materials use it, for example the Combine soldier's eyes with tints up to [4 4 .1].
- `$selfillumfresnel`: the lerp target is `$selfillumtint` × A × max, and the lerp factor is base.a × clamp(scale·clamp(N_v·V)^exp + bias). N_v is the interpolated vertex normal. (min, max, exp) come from `$selfillumfresnelminmaxexp` (default (0, 1, 1)); bias = min/max (0 if max = 0) and scale = 1 − bias. No stock phong material uses it.
- Detail modes 5 and 6 then apply to diffuse as in shaders.md section 4.
- result = specular + env + diffuse.
- Alpha:
  - α = `$alpha`, times base.a unless `$selfillum`, `$blendtintbybasealpha` or `$basemapalphaphongmask` is set.
  - Alpha test (`$alphatestreference` > 0 sets the reference) and blending are as in shaders.md.
- Output: × linear light scale, fog, sRGB encode, as shaders.md section 1 and 7.

## Per-pixel order

1. Vertex stage: skin the position, normal, tangent and binormal and normalise them. Compute the world position, the vector to the eye, the base and detail coordinates, and the 4 light attenuations a_i.
2. Sample the base texture (sRGB), then apply the detail combine.
3. Sample the normal map (raw) and build n_t and the mask (section 3). Transform n_t to N and normalise.
4. V = normalize(eye − P); F (the ramp), the rim fresnel (1 − N·V)⁴, R.
5. D = ambient(N) + Σ lights with the half-Lambert or warp term.
6. Envmap term, if any.
7. Sample the exponent texture (white if none), then pick k and S_t.
8. Σ specular lobes (with the phong warp if set) and Σ rim lobes.
9. mask × F (unless warped), then S × mask × boost.
10. A = albedo × modulation (or blend-tint); diffuse = A × D; self-illum; post-lighting detail.
11. Rim: Rm × ρ, max with S, + ambient(V) × boost × clamp(ρ·N_z).
12. result = S × S_t + env + diffuse; alpha.
13. × linear light scale, fog, sRGB encode, 8-bit clamp, alpha test and blend.

## Edge cases

- **R·L = 0 with k = 0** (cheap variant, no exponent): GPUs compute 0⁰ via exp2(0 × log2 0), which is undefined (NaN on most hardware). Behind the light N·L ≤ 0 also makes the lobe 0 × NaN. Treat it as 0. No stock material reaches it.
- **F > 1 and boost** multiply freely. Highlights brighter than 1 clip per channel at the 8-bit write, as everywhere in shaders.md.
- **Unnormalised normal-map texels**: n_t is not renormalised, but N is normalised after the tangent-frame transform, so only the direction matters.
- **Light count 0** (a view model in a dark corridor): D = ambient(N); S = 0; the rim keeps its ambient(V) part. That is the only highlight a phong surface gets without local lights.
- **Flashlight** (HL2:DM's suit flashlight): an additive pass with its own diffuse and specular from the projected texture and no ambient, envmap, rim or self-illum. It is not specified here (Open questions).
- **Wrinkle maps** (`$compress`, `$stretch`, `$bumpcompress`, `$bumpstretch`): blend the base and normal textures by a flex weight. No stock phong material uses them; not specified.
- **mat_fastnobump 1** binds the flat normal instead of the normal map, and the mask then comes from that texture's alpha.
- **mat_fullbright 2** (lighting only): the base is grey and the specular tint and envmap are zeroed.

## Quirks

- **Half-Lambert is forced.** Every material on this path is half-Lambert lit; `$halflambert 0` cannot turn it off. Keep it: it is why HL2 characters never go fully black on the shadow side under a local light.
- **The specular is reflection-vector Phong** with an extra clamp(N·L) factor, not a Blinn half-vector. shaders.md section 4 called it Blinn-Phong; that was wrong.
- **`$basemapalphaphongmask 1` discards the normal map's shading**, not just its alpha.
- **`$invertphongmask` inverts only the envmap mask**, never the specular mask.
- **The envmap mask is base alpha, uninverted**, the opposite of the plain model shader's `$basealphaenvmapmask`.
- **`$envmaptint` and `$phongtint` are raw** while the plain model path gamma-converts tints.
- **The rim's ambient glow only lights upward-facing pixels** (it is scaled by N_z), and it is sampled along the view ray.
- **The rim merges with the specular by max**, so a strong highlight hides the rim rather than adding to it.
- **`$phongexponent` beats the exponent map**, and no exponent at all means 150 (not the parameter list's advertised 5), or 0 on the cheap variant.
- **The frag grenade's body** (`$phong 1`, no normal map) is plain VertexLitGeneric, so the view model mixes shaded and highlit parts.

## Stock content (counted 2026-11-03)

- **Phong materials.** 57 VMTs set `$phong 1`, all VertexLitGeneric. 54 reach this shader, and all 54 have a `$bumpmap`, so none takes the cheap variant. 3 fall back (section 1).
- **Where they come from.** HL2:DM's own `hl2mp` VPK adds none, and CS:S's own `cstrike` VPK adds none; both installs carry the same 54 in the shared HL2 VPKs.
- **Parameters among the 54:**
  - `$phongboost`: 54, values 1–11, most often 5, 2 and 1.
  - `$phongfresnelranges`: 54. Most common: `[.1 .5 1.0]` 6, `[.2 .6 1]` 4, `[.1 .8 1.2]` 4, `[.1 5 1]` 3.
  - `$phongexponent`: 38, values 5–135.
  - `$phongexponenttexture`: 17.
  - `$phongalbedotint`: 14.
  - `$halflambert`: 25 (no effect).
  - `$selfillum`: 15.
  - `$envmap`: 13; `$normalmapalphaenvmapmask` 11; `$envmapfresnel` 8.
  - `$rimlight`: 10. `$rimlightexponent` 10, values 2–75; `$rimlightboost` 6, values 0.5–3.
  - `$phongwarptexture`: 1 (`models/items/combinerifle_ammo`).
  - `$detail`, `$translucent`: 1 each; `$alphatest`: 2.
  - None set: `$phongtint`, `$lightwarptexture`, `$basemapalphaphongmask`, `$rimmask`, `$invertphongmask`, `$phongexponentfactor`.
- **HL2:DM's uses:**
  - View models: crowbar 2, crossbow 3, frag grenade 4 (+ the fallback body), hands 1, AR2 1, gravity gun 2, pistol 1, RPG 1, shotgun 1, SMG1 1.
  - The Combine soldier player models (combinesoldiersheet, its shotgun variant, the prison guard pair, combine_elite): all with an exponent texture, albedo tint, half-Lambert and self-illum eyes.
  - The AR2 ammo item (the one phong warp).
  - Rebels, metrocops, the 357, SLAM and stun stick are not phong.

## Test cases

Pure 2.2 colour model as in shaders.md; linear values to 4–5 significant digits; bytes ±1. Unless stated: N = (0, 0, 1), one light with C = 1 and a = 1, mask = 1, S_t = 1, no envmap.

| Setup | Input | After | Expected |
|---|---|---|---|
| Routing | `$phong 1` + `$bumpmap` | shader | this spec |
| Routing | `$phong 1`, no `$bumpmap`, no lightwarp, no base-alpha mask | shader | plain VertexLitGeneric; phong ignored |
| Routing | `$phong 1`, `$lightwarptexture`, no bump | shader | this spec, cheap variant if base has no alpha |
| Routing | `$phong 1` + `$bumpmap`, no ps2.0b | shader | plain VertexLitGeneric (bumped variant), `$phong` cleared |
| Fresnel ramp `[.1 .5 1]` | N·V = 1 / 0.5 / 0.29289 / 0.2 / 0 | F | 0.1 / 0.3 / 0.5 / 0.64 / 1.0 |
| Fresnel ramp `[.1 5 1]` (pistol) | N·V = 1 / 0.5 / 0.29289 / 0.2 / 0 | F | 0.1 / 2.55 / 5.0 / 3.88 / 1.0 |
| Fresnel ramp `[.2 .4 2]` (grenade) | N·V = 0.5 / 0.2 / 0 | F | 0.3 / 0.848 / 2.0 |
| Fresnel ramp default | unset, N·V = 0.5 | F | 0.25 |
| Exponent | `$phongexponent 15`, exponent texture r = 128 | k | 15 |
| Exponent | no constant, exponent texture r = 128 | k | 1 + 149 × 0.50196 = 75.792 |
| Exponent | no constant, no texture | k | 150 |
| Exponent | `$phongexponentfactor 100`, r = 128 | k | 51.196 |
| Exponent | `$phongexponent 0`, r = 255 | k | 150 (0 counts as unset) |
| Exponent, cheap variant | no constant | k | 0 |
| Half-Lambert | N·L = 1 / 0.5 / 0 / −0.5 / −1, `$halflambert` 0 or 1 | w | 1 / 0.5625 / 0.25 / 0.0625 / 0 |
| Lightwarp | N·L = 0, warp texel at u = 0.5 is 128 grey | w | 2 × 0.50196 = 1.00392 |
| Specular, mirror direction | V = normalize(1, 0, 1), L = normalize(−1, 0, 1), k = 15, `[.1 .5 1]`, boost 5 | R·L, N·L, F, S | 1, 0.70711, 0.16863, 0.59619 |
| Specular, 10° off the mirror | same V; L at 55° from N in the xz plane (L = (−0.81915, 0, 0.57358)) | R·L, lobe, S | 0.98481, 0.79483 × 0.57358, 0.38438 |
| Specular mask | as the mirror case, normal alpha byte 128 | S | 0.29927 |
| Specular light behind | N·L < 0 (any R·L) | s | 0 |
| Full pixel | base byte 128 grey, ambient 0.05 on all six faces, mirror case light, k 15, `[.1 .5 1]`, boost 5, mask 1 | diffuse, S, result, byte | A = 0.21952; D = 0.05 + 0.72855 = 0.77855; diffuse 0.17091; S 0.59619; result 0.76710 → 226 |
| Same, mask 128 | as above, normal alpha 128 | result, byte | 0.47017 → 181 |
| Albedo tint | base linear (0.5, 0.25, 0.1), exp.g = 0.5, `$phongalbedotint 1`, no `$phongtint` | S_t | (0.75, 0.625, 0.55) |
| Albedo tint without map | `$phongalbedotint 1`, no exponent texture | S_t | (1, 1, 1) |
| Tint raw | `$phongtint "[0.5 0.5 1]"` | S_t | (0.5, 0.5, 1) (no gamma conversion) |
| Rim ambient only | no lights; V = normalize(1, 0, 0.1) (N·V = 0.099504); ambient +X = 0.2, +Z = 0.1, others 0; `$rimlight 1`, boost unset | (1 − N·V)⁴, ambient(V), rim add | 0.65755, 0.19901, 0.13086 |
| Rim ambient, boost 3 | same, `$rimlightboost 3` | rim add | 0.39258 |
| Rim ambient on a wall | N = (1, 0, 0), any V | rim add | 0 (N_z = 0) |
| Rim vs specular, max | R·L = 0.9, N·L = 0.5, k = 50, k_r = 2, F = 0.81089, boost 1, ρ = 0.65755, no ambient | S before max, Rm, S after | 0.0020896, 0.26631, 0.26631 |
| Rim exponent | `$rimlightexponent 0.5` / unset | k_r | 1 / 4 |
| Envmap fresnel | cube 0.2, N·V = 0.5, `[.1 .6 1]`, `$envmapfresnel 1`, m_e = 1, tint 1 | env | 0.07 |
| Envmap mask default | base.a = 0.25, cube 0.2, no fresnel, no `$normalmapalphaenvmapmask` | env | 0.05 (not inverted) |
| `$invertphongmask` | base.a = 0.25, cube 0.2 | env; S | 0.15; unchanged |
| Base-alpha phong mask | `$basemapalphaphongmask 1`, normal map tilted (200, 90, 220), base.a = 0.5 | N, mask | the vertex normal (map ignored), 0.5 |
| Self-illum | A = 0.5, D = 0.2, base.a = 1, `$selfillumtint [2 1 1]` | diffuse | (1.0, 0.5, 0.5) |
| Alpha | `$alpha 0.5`, base.a = 0.5, no selfillum | α | 0.25 |
| Alpha, self-illum | same with `$selfillum 1` | α | 0.5 |

## Open questions

1. **Missing parameters.** The set-up code reads `$phongtint`, `$phongfresnelranges`, `$phongboost`, `$rimlightexponent` and `$rimlightboost` only "if defined", with the fallbacks given above. Its own comment says an all-zero tint means the VMT gave none. That suggests a missing vector parameter may read as defined and zero, in which case a missing `$phongfresnelranges` would give F = 0 and a missing `$phongboost` would give no highlight at all. The material system that decides is not in the SDK. All 54 stock materials set both, and 4 of the 10 rim materials omit `$rimlightboost`, which would then be 0 rather than 1. To check: a test VMT with `$rimlight 1` and no `$rimlightboost` on a model under a bright ambient, compared with `$rimlightboost 1`.
2. **Directional lights in the pixel stage.** The per-pixel light direction comes from a position. It is assumed the engine packs a directional light (light_environment) as a point far along its reverse direction, so L ≈ −direction. To check in a RenderDoc capture of a model outdoors.
3. **Which 4 lights and their scale**: same as shaders.md open question 11. HL2:DM's view models are lit with the player's ambient cube and lights at the eye.
4. **Flat normal stand-in's alpha** (only for a material without a normal map, or with mat_fastnobump 1): 1 would give full specular, 0 none. Unused by stock content.
5. **`$bumptransform`** is passed to the vertex stage but the normal map appears to be sampled at the base coordinates. Unused by stock phong materials.
6. **Lightwarp's second coordinate**: a 1D lookup on a 2D ramp texture. Assumed to read the first row. Unused by stock content (HL2:DM has no lightwarp materials).
7. **Flashlight pass** for HL2:DM's suit flashlight on phong models: diffuse and specular from the projected texture with the same exponent, mask, fresnel and boost rules. Needs its own spec from the shared flashlight include if the flashlight lights view models or players visibly.
8. **Revision skew**: as shaders.md open question 13. HL2:DM's 2024-era binaries should match this revision more closely than CS:S's.
9. **Measurement**: compare a Combine soldier under a single known light and ambient, and the SMG1 view model in the lit wash room on dm_lockdown, by refcmp once implemented.
