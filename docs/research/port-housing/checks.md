# Placement validation, error codes and cursors

Sources: decompile `x2game.dll.c` (VAs), `funcs.csv`, `.data`; error key table
`FUN_3956b860` (`0x3956b860`, 5512 B) maps an error code to the key printed by the UI; the key→text
rows are in `ui_texts` (server DB). A claim is `[R]` when the code path/constant was read.

## 1. Error key function (port first)

`FUN_3956b860(int code) -> const char*` is a pure 783-case switch: `0 -> "no_error_message"`, and
the housing/doodad/deco subset is:

| code | key | where raised in 1.2 |
|---|---|---|
| 0x70 | `house_cannot_locate_invalid_area` | `FUN_39333e20` `0x39333e..` (outside any housing area), `FUN_39332030` `0x3933203a` (cursor not over world), `FUN_39332200` special branch |
| 0x71 | `house_cannot_locate_overlap_house` | `FUN_39335860` `0x39335860+` (OBB vs house) |
| 0x72 | `house_cannot_locate_overlap_unit` | `FUN_39335860` (OBB vs unit) |
| 0x73 | `house_cannot_locate_terrain_too_high` | key defined; raising site not isolated [?] (see FLAG-list) |
| 0x74 | `house_cannot_locate_terrain_too_low` | `FUN_39332180` `0x3933218a` and `FUN_39337fb0` (height spread > 6.0 m) |
| 0x75 | `house_cannot_locate_connector_missed` | `FUN_39338d40` (connector snap, 0x3933..), `FUN_39336600` |
| 0x76 | `house_cannot_locate_not_connected_neighbor` | `FUN_39338d40` |
| 0x77 | `house_cannot_create` | key defined; server-side refusal path [?] |
| 0x79 | `house_cannot_spawn` | key defined; server-side path [?] |
| 0x7a | `house_cannot_decorate` | key defined; deco start generic failure [?] |
| 0x7b | `house_not_decoratable_state` | key defined; unfinished/demolishing house [?] |
| 0x7c | `house_too_many_decorations` | `FUN_39327c50` `0x39327c50+` (count/limit pre-check) |
| 0x7d | `house_cannot_decorate_overlap` | `FUN_3903c5a0` (final primitive overlap test) |
| 0x7e | `house_cannot_decorate_out_of_house` | `FUN_3903c5a0` (item must be in a house) |
| 0x7f | `house_cannot_decorate_surface` | `FUN_3903c5a0` (surface flags), `FUN_3903e020` (cursor off world) |
| 0x9a | `too_far_away` | `FUN_39333d40` (house), `FUN_393a8fa0` (doodad, 30 m²) |
| 0xc7 | `too_far_to_decorate_house` | `FUN_394eaac0` case 6 (`FUN_39328630` found no house, pushes 199) |
| 0xc8 | `no_more_space_to_decorate` | `FUN_39327c50` (pushes 200) |
| 0xce | `not_ready` | `FUN_39017620` (globals missing) |
| 0xde | other-build/state sentinel | `FUN_39017620` (`player[0xebc] != DAT_39adac64`) |
| 0xcf | `failed_to_use_item` | deco/deco-packet cleanup (`FUN_39327de0`, `FUN_393a9340`) |
| 0xe5 | `no_housing_area` | `FUN_39330ba0` (`FUN_393304b0` failed) |
| 0x114 | `doodad_sim_area` | seed placement: `FUN_393a7170`/`FUN_393a8fa0` |
| 0x115 | `doodad_sim_too_close_to_other_doodad` | `FUN_393a8fa0` |
| 0x116 | `doodad_sim_too_close_to_other_unit` | `FUN_393a8fa0` |
| 0x117 | `doodad_sim_low_terrain` | `FUN_393a7720` (height difference) |
| 0x118 | `doodad_sim_no_error` | success sentinel of the doodad builder |
| 0x121 | `house_cannot_decorate_out_of_garden` | `FUN_3903c5a0` (garden/vertical constraint) |
| 0x122 | `house_cannot_decorate_in_house` | `FUN_3903c5a0` (wall/ceiling item outside) |
| 0x145 | `doodad_not_on_ground` | `FUN_393a9690` (mouse ray did not hit ground) |
| 0x14c | `dead`/dead-player check | `FUN_39017620` |
| 0x14e | gliding/mounted check | `FUN_39017620` |
| 0x154 | `house_cannot_locate_not_dominated_zone` | `FUN_39334060` |
| 0x155 | `house_cannot_locate_near_others_guard_house` | `FUN_39334060` |
| 0x163 | `other_build_is_on` | `FUN_39017620`, `FUN_393a9540` (doodad builder busy) |
| 0x17c | `house_invalid_height` | house height band `FUN_39334540` |
| 0x17f | `house_under_water_only` | `FUN_39334380` |
| 0x180 | `house_land_only` | `FUN_39334380` |
| 0x215 | `castle_cannot_locate_invalid_area` | `FUN_39332200` (castle/keep branch) |
| 0x235 | `cannot_locate_doodad_now` | shown by `FUN_3932ef10` / create-rate-limit text |
| 0x244 | `house_cannot_loacate_invalid_category_area` | `FUN_39333e20` (category vs housing area) |
| 0x267 | `house_cannot_own_more_houseless_condition` | create-time server/client check |
| 0x268 | `house_cannot_own_more_existing_category_condition` | create-time check |
| 0x274 | `housing_actability_deco_limited` | deco actability/limit (`FUN_39327bc0` / UI) |
| 0x275 | `acatbility_deco_arrang_fail` | `FUN_39327c50` |
| 0x294 | `cannot_decorate_house_during_war` | `FUN_39327c50` |
| 0x2ec | `only_nation_owner` | seed/farm permission `FUN_394eaac0` case 9 |
| 0x2ed | `locate_invalid_dominion_area` | seed/farm permission `FUN_394eaac0`, `FUN_393a8fa0` |
| 0x2fe | `house_cannot_construct_in_area_by_max_construct_count` | click/create max count |
| 0xb5 | `bag_invalid_item` | `FUN_39327c50` |

The two error slots on a builder are `+0x10` (reported, `FUN_393363b0` keeps the first) and `+0x14`
(secondary, e.g. terrain spread from `FUN_39332180`/`FUN_39337fb0`); `FUN_393363b0` copies `+0x14`
into `+0x10` if `+0x10` is still 0 (0x393363b0 body after `FUN_393354c0`).

## 2. House builder validation order (area A)

`FUN_393363b0` (`0x393363b0`, 280 B) is called from every SetPos/Rotate/Revalidate path
(`FUN_3933a300`, `FUN_3933a380`, `FUN_393364e0`, kind-specific variants; `calls.tsv` 14 callers).
It runs, in this order:

1. `FUN_39333cb0` (`0x39333cb0`, 134 B) — copy player position to builder `+0x8` (x/y) and `+0x10`
   (z?) and store `FUN_39325190()` (1/2 = garden radius class) in `+0x14`; snapshot the current
   target unit/model. `FUN_39325190` returns 2 when the housing desc `+0x158` (garden radius) > 0.
2. `FUN_39333d40` (`0x39333d40`, 214 B) — range: distance from the player position to the builder
   spot vs `unit+0x40 + 30.0`; error **0x9a** (`too_far_away`).
3. `FUN_39333e20` (`0x39333e20`, 573 B) — housing area/category:
   * for house-like categories (`FUN_39897070`) calls the housing-area service
     `[0x3a5bb1d8+0x44]->vtable+0xc4` and `FUN_39897280(area, category, out)`; mismatch -> **0x244**;
   * `vtable+0xc0`/`+0xb4` area membership -> **0x70**;
   * the debug path prints `housing area : ...` and the dot/box gates.
4. `FUN_39334060` (`0x39334060`, 799 B) — dominion/guard-house: **0x154**, **0x155**, **0x70**;
   also the 2 m neighbour pylon/proximity test.
5. `FUN_39334380` (`0x39334380`, 218 B) — water: `water_level == -1000000.0` (`0x399a3520`) or
   below the spot; `housings` byte `+0x154` (land/water only) -> **0x17f**/**0x180**.
6. `FUN_39335860` (`0x39335860`, 2234 B) + `FUN_393354c0` — OBB overlap against every other house
   and unit: house -> **0x71**, unit -> **0x72**; the "too many neighbours" list is capped at 32.
7. `FUN_39334540` (`0x39334540`, 604 B, called from the kind-specific `SetPos` variants) — height
   band around terrain (`DAT_399cf89c..a0`, re-placement §2) -> **0x17c**.
8. `FUN_39334460` (`0x39334460`, 216 B) — if an error is set, print `Builder Error : <key>`
   (`FUN_3900c0f0` key→text, `XlMultiToAcp`) only when the debug gate `DAT_3a167e84` is on, and
   override the ghost material with `materials/special/housing_build_invalid`; otherwise clear the
   override (`FUN_39327b50`).

Plot style: `FUN_3933a300` calls `FUN_3908c850()` (housing desc byte `+0xd`); false -> simple
6 m spread check `FUN_39332180` (error **0x74**), true -> the terrain-grid plot
`FUN_39337fb0` (also error **0x74** if the sampled heights spread > 6.0 m).

## 3. Decoration validation (area C)

Entry pre-checks (`FUN_3932c120`, `0x3932c120`, 1305 B):

1. `FUN_39017620` — global "can build" (`0xce` not ready, `0x14e` mounted, `0x14c` dead, `0xde`
   other state, `0x163` other build).
2. Cooldown `DAT_39adcc24` (1000 ms).
3. `FUN_3932ac80` item→deco design; house lookup through `FUN_39328630` in `FUN_394eaac0` case 6
   (no house -> **0xc7**).
4. `FUN_39327c50` (`0x39327c50`, 388 B): war -> **0x294**; design missing -> **0x12**; limit
   `desc+4` vs player -> **0x7c**; item missing -> **0xb5**; actability/`FUN_39327bc0` ->
   **0x275**; per-house count vs `desc+0x28` -> **0xc8**.
5. `FUN_393b03b0` builds the doodad ghost (name from 10.x `LoadDoodadModel`), `FUN_3903bf60`
   creates the 0x54-byte cursor.

Cursor state machine (`FUN_3903e020`, `0x3903e020`, 90 B):

* state `+0x44 == 1` (position): `FUN_3921ac90` mouse position -> `FUN_3903da00` -> ray to terrain
  (inside `FUN_3903da00`), sets the transform `+0xc..+0x3c`, `+0x49 = 1`, then calls
  `FUN_3903c5a0` and `FUN_3903d6a0`.
* state `+0x44 == 2` (rotation): `FUN_39016fe0` wheel delta added to yaw `+0x3c`, then
  `FUN_3903c5a0`/`FUN_3903d6a0`.
* `FUN_3903e020` when the cursor is not over the world sets error `+0x40 = 0x7f` and stops.
* Confirm: `FUN_3903b9d0` (`0x3903b9d0`, 33 B) — state 1 with `+0x49 != 0` -> state 2; state 2 ->
  `FUN_39327de0` (sends `CSDecorateHouse`). Right click `FUN_3903ba00` (`0x3903ba00`, 25 B) —
  state 2 -> state 1, otherwise cancel (`FUN_39325210`).

`FUN_3903c5a0` (`0x3903c5a0`, 4339 B) is the deco validity test; its error slots are
`cursor+0x40` and the desc returned by `FUN_3963b3d0(cursor+4)`:

| order | test | error |
|---|---|---|
| 1 | no doodad desc, or surface flag `cursor+0x48 == 0` | **0x7f** |
| 2 | garden constraint: parent house `+0x158` (garden radius) > 0.0 (live `[0x3999c484] = 0.0`) and point not in garden (`FUN_39326b40`) | **0x121** |
| 3 | 8-corner ray test vs the house; `allow_on_*` mask `desc+8/9/a/b/c`; outside when the item must be inside | **0x7e** |
| 4 | inside when the item must be outside (wall/ceiling mask) | **0x122** |
| 5 | `allow_on_wall` (`desc+8`) corners not inside the wall box | **0x121** |
| 6 | `PrimitiveWorldIntersection` against doodads/units, mask `0x1f` | **0x7d** |
| 7 | success | **0** |

Surface flag `cursor+0x48` comes from `FUN_3903da00` tail: state 1 -> any of desc `+0xa/+0xc/+8`;
state 2 -> desc `+0xb`; state 3 -> desc `+0x9`; otherwise 0.

Furniture move/rotate of an already placed item: no separate 1.2 client packet was found. The deco
cursor itself provides position + wheel rotation; re-using/picking up a placed furniture item and
using it again re-enters this same path. This is a **[?]** (see `MAP.md` U7).

## 4. Seed/sapling placement (area D)

`FUN_393a8fa0` (`0x393a8fa0`, 927 B) is the seed/doodad validity test used every frame by
`FUN_393a9690` (`0x393a9690`, 275 B):

1. `FUN_393a7170` (`0x393a7170`, 188 B): target doodad/farm area check; if no farm/area -> **0x114**.
2. Build the candidate doodad group; overlap with existing doodads -> **0x114**.
3. Terrain/water query at the spot (`[0x3a5bb1d8+0x2c]+0xf4`); invalid -> **0x114**.
4. Distance from the player: `> 900.0` (30 m²) -> **0x9a**.
5. Farm permission when the target is a farm (`FUN_39053360`) -> **0x2ed**.
6. Doodad overlap -> **0x115**; unit overlap -> **0x116**.
7. Success sentinel **0x118**.

`FUN_393a7720` (`0x393a7720`, 184 B) additionally rejects a height difference > 6.0 m
(`0x3999ecf4`) with **0x117**. `FUN_393a9690` sets **0x145** when the mouse ray misses the ground.
`FUN_393a9340` refuses to send unless `builder+0x10 == 0x118`.

Leaf markers (`FUN_393a4cc0`, `0x393a4cc0`, 1023 B) place `objects/ui/grow_mark/guide_leaf_y.cgf`
(neutral), `guide_leaf_g.cgf` (valid) and `guide_leaf_r.cgf` (invalid) on a grid around the spot;
the strings are in `strings.txt` and the count is derived from the target farm area
(`param_1+0x900`, `+0x38c..0x390` vectors).

## 5. Cursor tables and states

`FUN_397a7b40(mode, state)` (`0x397a7b40`, 16 B) indexes two tables:

* `0x39acb470` (modes 0 and 2): `0=empty`, `1=housing_position.dds`, `2=housing_rotation.dds`,
  `3=housing_elevation.dds`, `4=housing_zoom.dds`, `5=housing_invalid.dds`.
* `0x39acb458` (mode 1 = decorations): `1=housing_deco_position.dds`, `2=housing_deco_rotation.dds`,
  `3=housing_elevation.dds`, `4=housing_deco_zoom.dds`, `5=housing_deco_invalid.dds`.

`FUN_39017410(manager_kind, state, error)` (`0x39017410`, 396 B) is the one that selects the image:
state 1..3 map to position/rotation/elevation; Ctrl held (`FUN_394245a0`) forces 4 (zoom);
`error != 0 && error != 0x118` forces 5 (invalid). It also resets `DAT_39adac5c`/`DAT_39adac60`
and fires **`BUILDER_STEP` 0x224** with the string from `0x39ac3088` when the state changes. The
debug overlay string is `Builder Cursor : %s` (`FUN_39017730`).

Housing-manager cursor mode: `FUN_393fe030(2, on)` is switched on only in builder state 2
(`FUN_3932a3b0`/`FUN_390173b0`), so plain wheel rotates only after the first click and Ctrl+wheel
stays with the camera zoom.

## 6. Phase D live corrections (2026-10-25)

`goldens/phase_d.json` (capture: `rust/tools/capture_housing_phase_d.py`,
lane `house-golden`) records 24 cursor-constructor, 27 pre-check, 50
validation, 20 decoration-confirm, 20 seed-start, 38 seed-check, 20
seed-height, 20 farm/area and 20 seed-confirm cases.

The live decoration descriptor byte layout is `+8 allow_mesh_on_garden`,
`+9 allow_on_ceiling`, `+a allow_on_floor`, `+b allow_on_wall`,
`+c allow_pivot_on_garden`.  `FUN_3903c5a0` therefore treats an item as
**inside-capable** when ceiling/floor/wall is set, as **outside-capable** when
mesh/wall/pivot is set, and runs the wall-box test only for the `+8` (mesh)
flag.  The constructor always sets `cursor+0x48 = 1`; the surface byte is not
cleared by `FUN_3903bf60`.  Its surface gate tests `+8|+a|+c`
(mesh/floor/pivot), so a wall-only design (`+b`) skips the ghost-basis yaw
computation and keeps yaw 0 while still creating the cursor.

`FUN_393a7720` compares the doodad box maximum and minimum heights:
`max - min > 6.0` writes 0x117.  It is not terminal: `FUN_393a8fa0` continues
with range/house/doodad/unit checks, and a later failure replaces 0x117.  A
spread <= 6 m instead snaps the ghost to the box minimum (`FUN_393a1050`),
which the later distance/overlap checks observe.  The live tree ghost box is
under 6 m, so the 0x117 branch was exercised with a controlled height leaf.

`FUN_39327de0` checks `cursor+0x40` before the manager item pair; a non-zero
cursor error is reported and the cursor is kept.  With error 0 and no item
pair it reports 0xcf and keeps the cursor; a valid pair sends 0x58 and frees
it.  For seeds, a second item use while a builder exists is refused by the
dispatcher's global check with **0xce** before `DAT_3a17a950` (the 2 s guard
inside `FUN_393a9540`), so 0x163 was not observed through live item use.
