# Placement (house and farm) in the original 1.2 client

Sources: Ghidra `proj-aa12u` `x2game_unpacked.dll` (read-only), 1.2 `.alb` Lua disassembly (`albtool`), `game/default_binding.g`
from `game_pak` (via `aa-pak cat`), `compact.sqlite3` (`housings`, `housing_build_steps`, `localized_texts`, `skills`, `icons`),
the three owner screenshots. Scratch: `tmp/re/placement/` (decompiles `r_bm.c`, `r_b17.c`, `d*.c`, tables `tbl*.txt`, `errcodes.txt`).
Confidence: **[R]** read in code/data, **[I]** inferred, **[?]** not found. No Ghidra lock file was left behind (none exists in `proj-aa12u`).

## 1. State machine (the "builder")

The builder manager is the global at `DAT_39aff98c`; its object (`+0x30`) is a class chosen in `FUN_39330ba0` (StartBuild) by
`FUN_3908c810` (= `1` when `FUN_39897070(housings.category_id)` is true: categories 1,7-12,15-18). The Thatched Farmhouse is
`housings.id 171`, `category_id 18` (steps 14575 x10, 14641 x5, 14574 x5, matching 0/10 0/5 0/5), so it is **kind 1**,
base class `FUN_39336140` (vtable `399cf920`). **[R]**

State field `+0xc`; the setters are 1-instruction vtable slots (`mov [ecx+0xc],n`: `393317e0/f0/…`): slot `+0x1c` -> 1, `+0x20` -> 2,
`+0x24` -> 3, `+0x28` -> 4. `FUN_39017410(_, state, error)` runs every frame and fires the UI event **`BUILDER_STEP`** (0x224)
with the string `{"stanby","position","rotation","elevation"}[state]` (table `39ac3088`); it also picks the cursor. **[R]**

| State | Meaning | Per frame (`FUN_39332030`) |
|---|---|---|
| 1 position | model follows the mouse | mouse -> terrain ray (`FUN_3921ac90`, `FUN_394298c0`) -> `SetPos` (slot `+0xc`, `FUN_3933a300`) |
| 2 rotation | spot locked at the click | `FUN_39016fe0()` delta -> `Rotate` (slot `+0x10`, `FUN_3933a380`: yaw += delta, then re-validate); position is NOT updated |
| 3 elevation | non-kind-1 classes only (axis 9) | |
| 4 | tax popup pending (kind 1) | `+0x18` revalidate only |

Left click, `FUN_39332200` (kind 1): state 1 -> **state 2** (slot `+0x20`); state 2 -> `FUN_39325f60` sends **`CSConstructHouseTax` (0x56:
design id + position)** and sets state 4. The Start Construction window opens when the answer arrives. Other kinds: 1 -> 2, 2 -> 3 (kinds
2,3) or send `CSCreateHouse` directly (`FUN_3932fe60`). **[R]** So the original needs **two clicks**: the first starts rotation (help
tooltip up), the second asks for the quote/popup. The owner's `farmhouse_placement.png` (tooltip, no popup) is exactly state 2.

Back/right click, `FUN_39331760` (kind 1): state 1 -> cancel builder (`FUN_39330120`, fires `BUILDER_END` 0x225 and clears
the cursor); state 2 -> slot `+0x1c` (**back to state 1, unlocked**); popup state -> fires event 0x100 (`HOUSE_BUILD_INFO`) and returns
to state 2. Escape: `FUN_39017340(0x10)` / `FUN_390171a0` cancel from any state (`FUN_39330120`). The Build window's buttons:
`NextStepFromBuildCheck` = `FUN_39477cb0` = `FUN_3932fe60` (CSCreateHouse); `PrevStepFromBuildCheck` = `FUN_39477ce0` =
`FUN_39017220(3)` (1000 ms guard `FUN_39016fb0`, steps the builder back; the hotkey mode is re-set from the state). **[R]; popup Cancel
returning to state 2 is [I]** from `FUN_39017220`.

**Wheel.** `default_binding.g` has a hotkey mode `builder_rotate_mode` (mode index 2 of `{normal_mode, mode_action_mode,
builder_rotate_mode, screen_shot_mode, quest_directing_mode}`, table `39ac9020`) with:
`wheelup builder_rotate_left_normal`, `SHIFT-wheelup ..left_large`, `ALT-wheelup ..left_small`, `CTRL-wheelup builder_zoom_in`, and the
`wheeldown` mirror (`right`/`zoom_out`). `FUN_393fe030(2, state==2)` switches that mode on only in state 2 (and off in
`FUN_390173b0` on end). Outside state 2 the wheel is the normal `zoom_in/zoom_out`. **[R]** This confirms the owner's rule: the wheel
rotates only after the spot is clicked, zooms before, and Ctrl+wheel keeps the camera zoom while rotating.

**Step sizes** (`FUN_39016fe0`, action ids 0x5a..0x5f = `builder_rotate_*`, yaw in radians): normal left/right = `value * (+/-0.0174533 rad = 1 deg) * X`
(X = `DAT_39adac58->vtbl[3]()`, a runtime global I could not resolve, probably a frame/time scale); **small (Alt) = +/-0.01 rad = 0.573 deg**
per pulse; **large (Shift) = +/-pi/12 = 15 deg** per pulse. `wheelup` = left = positive. **[R]** for small/large; the **normal step is [?]** (not 10
deg by any constant I can see; needs one live count in the retail client).

**Tooltip.** Lua `hud/build_guide.alb`: handler on `BUILDER_STEP(step)`: `step == "rotation"` -> `ShowBuildGuide(true)`, else hide;
`BUILDER_END` hides. Window `housing_build_guide`, anchored BOTTOM +0/-100, 356x172 background (`TEXTURE_PATH.TUTORIAL`, coords
356x172, colour alpha 0.7), four rows `locale.common.buildGuide1..4` (mouse icon + text; Alt/Shift/Ctrl key icons). English
(`ui_texts` 4529, 5669, 5670 + Ctrl line, as in the screenshot). It is shown in **state 2 only**. **[R]**

**Cursor** (`FUN_39017410` -> `FUN_397a7b40`): the OS cursor image by state: 1 `housing_position.dds`, 2 `housing_rotation.dds`, 3
`housing_elevation.dds`, **Ctrl held `housing_zoom.dds` (state value 4)**, **error != 0 `housing_invalid.dds` (5)**. **[R]**

**Initial yaw.** The first class constructor does not set it; I could not find the "faces the player" code (`FUN_393323c0` sets yaw from
the model default snapped to `step` (pi in `DAT_3a16828c`?)). **[?]** The owner's captures (camera heading) stand as the oracle.

## 2. Ghost model, tint, validation

* Ghost: temp housing object `"temp housing"` built from the design's main model (`FUN_3932fb20`), `FUN_393301a0` "chging builder house".
* **Ghost model = the whole building [R data, I original].** `housings.main_model_id` is a `PrefabModel` (Thatched Farmhouse: model 1082,
  prefab 530). Its `prefab_elements` list the pieces per state: state 1 (alive) = `housing_big.xml/nuia_housing_small3.nuia_small_wall01`
  + `..._roof01` + `..._floor01`; each prefab holds `Brush` objects (`housing_s_wall01` + `_in`, `housing_s_roof03`, `housing_s_floor03`,
  plus ivy / sweet potato / pumpkin decorations) at their own `Pos`/`Rotate`/`Scale`. The Bevy ghost used the first brush of the first
  element only (one wall: "3 parts", floating, no floor / roof). `Db::housing_parts` now returns every brush of the lowest state's
  elements and the ghost spawns them all (`placement::spawn_ghost_model`); finished houses and construction steps (`prepare_unit`) use the
  same list. Farm designs (scarecrow gardens, 89 / 267) are one-brush prefabs and unchanged. Render: `examples/ghost_farmhouse.rs`.
* **Ground.** The model origin is its ground contact (floor brush at z 0). `housings.auto_z_offset_z` is +1 m on almost every house
  design (+1.5 on the halls, +4 m y on the mansions); lifting the spot by it floated the ghost, so houses use no ground offset
  (`house_grounding`). **[?]** what the original does with the field (probably a ground-probe origin); the server stores the sent z.
* **Tint:** only the invalid state. `FUN_39334460`: if `error != 0` the ghost material is overridden with
  **`materials/special/housing_build_invalid`** (`FUN_39327b50(mat)`), else the override is cleared (0 = the normal model, untinted).
  A debug-flag text `Builder Error : <key>` (gate `FUN_394d0b90`) is printed only when the debug level is on. **[R]**
* Checks (`FUN_393363b0` runs these in order; the first error wins, `+0x10` = error code; keys from `FUN_3956b860`; text =
  `ui_texts` row `code+1` for codes <= ~0x78, verified 0x70-0x78): 
  - range: spot farther than `housings`' use range from the player -> `0x9a too_far_away` (`FUN_39333d40`, range `+0x40` + const);
  - `FUN_39333e20` area: category not allowed in this housing area (`FUN_39897280`) -> `0x244 house_cannot_loacate_invalid_category_area`;
    spot not inside a housing area (`+0xc0/+0xc4/+0xb4` queries) -> `0x70 house_cannot_locate_invalid_area` ("You can't construct a building here.");
  - `FUN_39334060` dominion: `0x154 house_cannot_locate_not_dominated_zone`, `0x155 ..near_others_guard_house`;
  - `FUN_39334380` water: `0x17f house_under_water_only` / `0x180 house_land_only` per `housings` flag `+0x154`;
  - class checks: `0x71 overlap_house` ("...overlap another structure."), `0x72 overlap_unit` ("...obstacles block the way."), `0x73/0x74 terrain_too_high/low`
    ("...on a steep slope."), `0x75 connector_missed`, `0x76 not_connected_neighbor`, `0x17c house_invalid_height` (`FUN_39334540`, band `DAT_399cf89c..a0` around terrain),
    plot terrain `0x74` (`FUN_39337fb0`, `FUN_39332180`: 3 m height spread `DAT_3999ecf4`);
  - on click: `0x2fe max_construct_count`, `0x267/0x268` houseless/existing-category, `0x163 other_build_is_on`, `0xce not_ready`, `0xde` mounted, `0x14e` gliding, `0x14c` dead (`FUN_39017620`).
  - `CSCreateHouse` is rate-limited 5000 ms (`DAT_3a167ea0`): message key `CANNOT_LOCATE_DOODAD_NOW` with the seconds left.

## 3. Plot border and neighbours

* The farmhouse design text says "Dimensions 24m x 24m". The kind-1 per-frame draw is vtable slot 1 = `FUN_39334ab0`
  (draws `+0x1fc..+0x200` points with `DrawPoint`-style calls, colour `0xAFFFFFFF` white alpha 0.69, and two outline boxes) over the points
  `FUN_39337fb0` gathers: a **terrain-grid sample of the plot rotated by the model yaw** (rotated box test, step `vtbl+0x20c` =
  terrain unit, per-point height from the terrain `+0x204`). Both are gated by `FUN_394d0b90` (`DAT_3a167e84`->vtbl[2] > 0). **[R]** code, **[?]** whether that gate is the
  "guide decal" option (`toggle_show_guide_decal`, Ctrl+Q) or a debug cvar; the owner's screenshot proves the dots appear in retail.
* So the retail dots **do follow the terrain and rotate with the model** in this code path (the plot box uses the model orientation). The Bevy comment
  "border stays axis-aligned" is contradicted by `FUN_39337fb0` (it builds the box from the yaw quaternion), **[I]** until a rotated
  screenshot is taken. Colour: white/grey translucent in the screenshot (not orange). Spacing in the screenshot ~1.5-2 m; code step = terrain unit **[?]**.
* Other kinds (`FUN_39337490`) draw 20 m grid edges (`FUN_39334740`) where the housing-area test changes; neighbours' fence posts are real
  `Housing::InitFences` objects (`FUN_3932ead0`), not part of the builder.

## 4. Packets and Start Construction window

* `CSConstructHouseTax` 0x56: `design_id u32, x i64, y i64, z f32` (`FUN_397abc50`). Reply `SCConstructHouseTax`.
* `CSCreateHouse` 0x57 (`FUN_397b09d0`, built in `FUN_3932fe60`): design id (`housings.id`), position (x,y i64 fixed + z f32),
  **z_rot = the builder yaw radians (vtbl `+0xc`)**, item id u64 (the design item, `+0x24` pointer), money (builder `+0x21c`),
  `ht` (`DAT_3a167ebc`), bool = client option `FUN_397ccd50` ("auto use AA point"). Matches the server `CSCreateHousePacket`. **[R]**
* Window (`build_window_view.alb`): title `locale.housing.buildWindow.title`; line 1 `[%s]|r %s` = `GetHousingModelName(type)`
  (`housings.name` localized) + "Begin construction."; `Deposit: %s` / `Base Tax: %s` via `GetTaxString`, `Buildings Owned: %d` (`houseCnt`);
  `Total Taxes:` and the gray `heavyTaxDesc` (or `heavyTaxExemption`) only when `heavy_tax`; stages from `GetHousingModelConstructionInfo(type)`
  (`housing_build_steps` grouped by item, icon + `consumeItemStack` + `needActions`, arrows between, "Each stage uses Labor" is in the Construction window);
  tax certificate icon with `CountTaxItemInBag/CountTaxItemForTax` as `have/need` (red when short, disables Build). The numbers are the SC packet
  (server formula `deposit = base * 2`, screenshot 30 = 2 x 15). Total `60` = tax cert count. **[R]** UI, **[I]** formula mapping from the screenshot.

## 5. Construction / info windows and F / G

* `housing_manager.alb`: `HOUSE_INTERACTION_START`: `IsHouseUnderConstruction` -> `ShowMaterialWindow` (title "Construction": `Ownership :`, `Tax Collector:` (zone),
  `Amount Due`, `Tax Status: [Paid in full]`, `Protected Until`, `Construction Status: a/b` via `GetHouseBaseProgress/RemainProgress`, per-step icons
  `GetHouseConstructionStepInfo`, note "Each stage uses Labor. Area ownership won't be recognized if not completed.", `Demolish` enabled only for `IsMyHouse`,
  `Confirm`); finished -> `ShowHousingMainTainWindow` + `UpdateMyHouseDefaultInfo` (own) / `UpdateHouseDefaultInfo` (other). **[R]**
* F/G are `do_interaction_1/2` (`f`,`g`) of the dynamic action bar (`dynamic_action_bar.alb`: `X2:GetDynamicActionCount`, `GetDynamicActionSkill(i)`,
  `ExecuteDynamicAction(i)`; the icon is `X2Interaction.GetInteractionSkillIconPath(skill)`). The list comes from the `InteractionIndicator`
  (`FUN_393bdad0` MakeCandidate -> `FUN_393bcb90` per-unit skill list, up to 256). **[R]**
* Under construction: F = **Construction Info**, skill 11001 (category 1, 4 m) / 15212 (category 3, 6 m, `Check Construction Info`), icon 1738 = `interaction/icon_interaction16.dds` (magnifier). G = the current
  step's `housing_build_steps.skill_id` (14575 stone / 14641 iron / 14574 wood, "Building Construction - ..."), icons `icon_interaction54/55` etc. The screenshot's G icon
  is a log bundle; with step 0 = stone (14575) the match is **[?]**. Finished building: F = info, G = craft only on one's own farm (owner's answer); the data source for the per-house list is
  not in a table I could decode (server-less client builds it from the unit's skill list) **[?]**.

## Gap list against Bevy (`D:/aaemu-client/AABevy1.2`)

1. **Two-click flow.** Original: click 1 locks the spot and enters rotation (state 2); click 2 sends `CSConstructHouseTax` and opens the popup. Bevy: click 1 already
   requests the quote and opens the window (`placement.rs:1594-1604`, `Session::locked`). Evidence `FUN_39332200`, `farmhouse_placement.png`. Fix: add a rotation state between; request the quote on the second click.
2. **Help tooltip missing.** `hud/build_guide.alb` / `BUILDER_STEP` are not wired (only `BUILDER_END`, `ingame/houses.rs:178`). Fire `BUILDER_STEP("position"|"rotation"|"elevation"|"stanby")` every state change; the stock Lua shows the 4-row guide only for `"rotation"`.
3. **Right-click.** Original: state 2 -> back to state 1 (unlock), state 1 -> cancel; Escape cancels from any state. Bevy cancels on right click everywhere (`placement.rs:1578-1583`). Popup Cancel returns to the rotation state in the original, not free placement.
4. **Step sizes.** Original Shift = 15 deg (matches), Alt = 0.01 rad = 0.573 deg (Bevy 1 deg, `wheel_turn`), plain = 1 deg (static `0x3999cf98 = 0.017453292`) times the runtime multiplier `X` (`DAT_39adac58->vtbl[3]()`, unresolved; Bevy keeps its 10 deg placeholder). Needs one live count; Alt fix is certain.
5. **Rotation mode keys.** Original rebinding is hotkey-based (`builder_rotate_mode` in `default_binding.g`; remappable). Bevy hard-codes the wheel. Optional parity: read the binding.
6. **Cursor images.** Original swaps `housing_rotation.dds` in state 2, `housing_zoom.dds` while Ctrl is held, `housing_invalid.dds` on any error; Bevy only uses position/invalid (`placement.rs:1342`).
7. **Ghost tint.** Original overrides the ghost with `materials/special/housing_build_invalid` when invalid; Bevy leaves the ghost untinted (red ring only, `docs` "plain, untinted").
8. **Plot dots rotate.** `FUN_39337fb0` builds the plot from the model orientation, so the dot region turns with the yaw and follows the terrain; Bevy forces yaw 0 (`plot_dot_quads(... yaw 0)`), colour orange (retail white/grey alpha ~0.69). Confirm with a rotated retail screenshot before changing.
9. **Error texts by code.** Retail prints `ui_texts[code+1]` for the 0x70-0x78 family ("You can't construct a building here.", "...overlap another structure.", "...obstacles block the way.", "...steep slope.", "You can't connect to this building.", "No building to connect to.", 0x9a "Too far away."), and the server codes through the same table; Bevy prints "Can't be installed here." / "Too close to other objects." (`error_message`). Use per-check codes (and the 5 s `CANNOT_LOCATE_DOODAD_NOW` countdown).
10. **Create rate limit.** 5000 ms between `CSCreateHouse` sends, with the countdown message; absent in Bevy.
11. **CSCreateHouse `auto_use_aa_point`.** Original sends the client option (`FUN_397ccd50`), Bevy always `false`; `money_amount` equals the builder total (Bevy sends the quote total, plausible).
12. **A refused `CSCreateHouse` must keep the builder.** The builder ends only on the server's `SCMyHouse`
    (`BUILDER_END`); a refusal (`SCErrorMsg`, e.g. 481 "Insufficient tax payment" when a 450000 quote
    needs 45 tax certificates and the bag has 20) keeps the ghost and the Start Construction window for
    a retry (the 5 s limit still runs). Bevy used to mark the session done and fire `BUILDER_END` on
    send, so `finish` dropped the ghost and there was nothing left to retry. Done in `8cb994a`:
    `finish` ends only when `created`, `ingame::houses::create_refused` clears the pending create;
    tests `the_builder_ends_only_after_the_server_creates_the_house` and
    `a_refused_create_keeps_the_build_window_and_the_placement`.

## Open questions

* Normal wheel step multiplier (value of `DAT_39adac58->vtbl[3]()` and the wheel pulse value); the static coefficient is resolved to 1 deg at `0x3999cf98`, the effective step still needs one live count.
* Initial yaw code ("faces the player") and the dot spacing / gate of `FUN_39334ab0` (guide decal option vs debug).
* G action on a house under construction (which step skill) and the per-house dynamic action source.
* The owner's "one click opens the popup" (3.5 retail) contradicts 1.2 code (two clicks); the screenshot with tooltip and no popup supports 1.2. Please re-check in 3.5 whether a second click is needed.

## Live original-client measurements (Scarecrow Garden, item 15596; screenshots `tmp/oracle/a-p-*.png`)

Measured **[M]** in the running original client:

* Click 1 sends nothing; click 2 sends `CSConstructHouseTax` and the Start Construction popup opens. Test:
  `a_house_needs_two_clicks_the_second_asks_for_the_quote` counts the packets through the real `input` system.
* After popup Cancel the ghost STAYS LOCKED (it ignores the cursor) and the rotation hint (the `housing_build_guide` window) stays; one right
  click returns to free placement (a-p-6, a-p-7). Test: `after_popup_cancel_the_ghost_stays_locked_until_a_right_click`.
* After click 1 the tutorial tip "Scroll to rotate / Alt precise / Shift 15 degrees / Ctrl view" is the stock `build_guide.alb` window
  (`BUILDER_STEP("rotation")`), not a separate tutorial path. Test (real Lua runtime):
  `the_original_build_guide_shows_for_rotation_and_hides_on_builder_end`.
* Plot dots: small plain WHITE points (alpha 0.69, `FUN_39334ab0` colour 0xAFFFFFFF), no glow, on straight plot edges that follow the terrain
  (a-p-8-move.png). Bevy now draws untextured white alpha-0.69 dots, 0.14 m, 1.0 m apart (spacing/size are estimates from the screenshot, the
  retail step is the terrain unit, still **[?]**). Whether they rotate with the model is still **unmeasured**: they stay axis-aligned.
  The orange `flare_08` look was the 3.5 capture's, not 1.2.
* Wheel (suspect: inverted on the old main build): the branch zooms before click 1 and rotates after it without zooming; Ctrl+wheel keeps the
  camera zoom while locked (`camera::orbit_input` now reads the raw wheel for it, because the normal mode has no CTRL-wheel binding). Test:
  `the_real_camera_zooms_before_the_lock_click_and_the_wheel_turns_the_model_after_it` (real `update_input`, `orbit_input`, `input`).
* Build button: disabled with 5 Tax Certificates in the bag, enabled with 35: the popup's own Lua decides it from `CountTaxItemInBag`;
  test `the_original_popup_disables_build_without_enough_tax_certificates`. The oracle's 450000 quote needs 45:
  20 certificates (10 normal + 10 bound) disable Build; the server's TaxItem feature charges certificates, not the
  money in the bag, and answers a scripted send with `SCErrorMsg` 481 (no `SCMyHouse`) - the refused builder stays in Bevy (`8cb994a`).

## Status in Bevy (fix/placement-re)

Gaps 1, 2, 3, 4 (Shift/Alt exact; plain step still a named placeholder `ROT_STEP_PLAIN`, 10 degrees), 6, 7, 9, 10, 11 and 12
(`8cb994a`) are implemented (see `docs/aa-client-hud.md`, "Builder state machine"). Gap 5 (rebinding through
`builder_rotate_mode`) is not done. Gap 8: the dots are white, small, on straight terrain-following edges (measured);
rotation with the model is still open until a rotated retail screenshot.
