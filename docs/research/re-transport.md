# Transport beyond mounts and the Farm Wagon (AA 1.2 client)

Area: ships, gliders, other land vehicles (cars/tractors), carriages/airships (transfers), siege weapons,
the water interaction of vehicles and their packets. Re-mounts and the Farm Wagon are in `re-mounts*.md`
and `re-wagons.md`; this note does not repeat their common bind/drive code.

## 0. Scope, sources, method

Sources: 1.2 decompile `C:/AAEMU_DISK/aa-re/client12/out-x2game` (VAs base 0x38ff0000),
`x2game_unpacked.dll` for `.data`, the 1.2 pak (`D:/AA1.2/game_pak`, read with a local AES-CBC reader;
extracted XML/CAL in `tmp/re/areas/transport/pakout*`), UI `.alb` (`D:/aa-re/client12/tools/albtool.py`),
DB `D:/aaemu-live-bevy-srv/rs-game/Data/compact.sqlite3`, Bevy `D:/aaemu-client/AABevy1.2` (read-only).
`[R]` code/data, `[I]` inferred, `[?]` not found; wagon findings are cited, not re-proved.

`UpdateSail` etc. were found by the `"X2::..."` prologue literal (strings do not xref them). Event
ids use `FUN_39214380`'s order: `SIEGEWEAPON_BOUND` = 97 = 0x61 (matches `FUN_39345d00`), `SLAVE_SPAWN`
= 102 = 0x66 (matches `FUN_3934a6c0`); hence `TRANSFER_BOARDING` 0x64, `TRANSFER_UNBOARDING` 0x65,
`SLAVE_SHIP_BOARDING` 0x67, `SLAVE_SHIP_UNBOARDING` 0x68, `SLAVE_UNIT_EQUIPMENT_CHANGED` 0x69. [R]

## 1. Ship identity and data

Ships are slaves (`slaves.slave_kind_id` 1/2/3/4/9/10) whose `models.sub_type='ShipModel'`;
`models.sub_id` names a `ship_models` row. Player-summoned hulls [R]:

| slave | name | item | model | ship_models | velocity | reverse | steer_vel | mass | box |
|---|---|---|---|---|---|---|---|---|---|
| 15 | rowboat (나룻배) | 15817 | 129 | 4 | 6.4 | 3.5 | 6.0 | 800 | 1×3 |
| 14/52/76 | clipper/speedboat | 15816/17626/23618 | 128 | 3 | 13.0 | 5.0 | 5.0 | 5000 | 4×8 |
| 21/92 | small sailboat | 5223/25739 | 393/1249 | 6/15 | 12.6 | 5.0 | 1.0 | 45000 | 6.5×30 |
| 24 | Growling sailboat | 13711 | 523 | 9 | **19.6** | 5.0 | 1.0 | 45000 | 6.5×30 |
| 75 | merchant ship | 23398 | 1205 | 14 | 13.0 | 5.0 | 1.0 | 45000 | 6.5×30 |
| 110/113/114 | fishing boat | 27135/27701/27702 | 1360/1383/1384 | 19/20/21 | 13.0 | 5.0 | 0.8 | 12000 | 5×11 |

Duck boat 102 (8.0), solo boat 103 (10.0), cruise 63 (12.6), galleon 9/11/13 (6.5–12.6) and middle galleon
139 (12.6) use related rows. [R] `ship_models` also carries `damaged25/50/75`, `dead`, `mass_center_*`,
`tube_*`, `keel_*`, `water_*`, `accel`, `reverse_accel`, `turn_accel` (`SELECT ... FROM ship_models`
39a06618). Simulation is the **server's**; `ShipUnitModel::Update` (`FUN_390ebc50`) is visual/audio only.
[R]

All ship scrolls share `items.use_skill_id = 15802` (5 s cast, 10 s cd, SummonPos type 12, selection 4,
`check_obstacle t`, `SpecialEffect` 10822). Placement uses `SlaveLocator::Update` (`FUN_3934a3e0`; cursor
ray `FUN_39349ad0`, wheel `FUN_39349b70`, red `housing_build_invalid` `FUN_39346580`). For boats
(`kind 4`) `FUN_39348bf0` requires distance squared `<= 6400.0` (80 m; `0x399cfcfc`) and a
`PrimitiveWorldIntersection` of the OBB (bbox×0.5, flags 0x10d) returning 0 hits. [R] No water lookup is
there; SummonPos/server enforce water. [R/I] Too-small space: `SLAVE_SPAWN_SHIP_NEED_MORE_SPACE`
(ui_texts 101). [R]

## 2. Ship boarding, helm, and the visual chain

**Boarding.** The unit target interaction ends in `CSBindSlave` 0x31 (`FUN_393464a0`, log
"requesting bind slave(slaveId:%u)") after `FUN_39346290`: on foot (state 2), not busy (+0x3a70..0x3a7b),
not dead, not already bound; range is squared `DAT_39ac7408 = 16.0` → **4 m** (`FUN_39093860`).
Refusals: 0x144 dead, 0x13 busy/bound, 0x9a range, 0x66 already has a master. Target interaction itself
reaches 3 m (`DAT_39ac79fc = 9.0`; `FUN_393bf270`). [R] `CSBindSlave` sends only the **tl id**; the server
picks the seat and answers `SCUnitAttached` (child, point, reason, parent) plus `SCSlaveBound` for point 1.
`OnUnitAttached` (`FUN_391d49d0`) parents; `OnSlaveBound` (`FUN_391d4760`) calls `FUN_39345d00`, which
fires `SIEGEWEAPON_BOUND` (0x61). [R] Ship UI events 0x67/0x68 exist but no fire site or 1.2 UI listener
was found. [R id, ? use]

**Helm.** The driver sends `CSMoveUnit` type 4 (`ShipRequestMoveType`, `FUN_397c0470` case 4): signed
`throttle`, then `steering`. The server owns the hull and broadcasts type 3 (`ShipMoveType`): position,
i16 velocity, i16 quaternion (x,z,y; 1/32767), three f32 angular velocities, i8 steering/throttle, u16
zone, bool `stuck`. [R] The local client only turns parts:

* rudder: `UpdateRudder` (`FUN_390c7410`) rotates the child "rudder" by steering (`+0x35c`); missing it
  logs "ship unit model(%u) has no rudder" (`FUN_390c7330`).
* wheel: `UpdateSteeringWheel` (`FUN_390d2990`) uses attach 35 (`$helms`), skips smallboat/soloship/
  coupleduckship, and eases toward a **45°-quantised** target (`DAT_3999f5bc=45.0`; π/180) with sound.
* sails: `UpdateSail` (`FUN_390d1c60`) runs on attach 21/22 (`FUN_390e8e40` accepts only 0x15/0x16),
  mixing wind (`DAT_39ae6c90/94`), speed/model speed and the 0.48/0.235 filter into a billow; "flag"
  bones get `bone_yaw`, limited to 0.628 rad/s (`0x399a44d0`).
* `UpdateThrottleAnim` (`FUN_390db700`) plays idle/run_f/run_b from signed speed (`+0x358`), rate=speed,
  flags 0x5082/0x508a. `UpdateExhaustEffect` (`FUN_390c78d0`) drives "exhaust"; `UpdateWaterRipple`
  (`FUN_390c7640`) emits `splash_l/r`; `UpdateSound` (`FUN_390c7bf0`) runs the water loop, stopping at
  state 5/6. `Init` (`FUN_390e8b30`) binds all of them via `FUN_390eaf40`. [R]

Sail/anchor **skills** exist in data (11545/11546 sail fold/unfold, 11647/11648 anchor, 1 s cast, 4.7 s
cooldown, max range 1–4 m) but **no `slave_mount_skills` row references them** and no ship UI uses them;
1.2 ship sails therefore have no player control: they are permanently rigged and animated from
throttle/speed. `[R absence]` The anchor is a static equipment doodad (item 29727 / doodad 1195, attach
23) with no client animation (`slave_doodad_bindings` 21/92 attach 23); Bevy's Space "anchor" is an
invention, not a 1.2 rule. [R]

**Mode bar / Summon Info.** `FUN_39204500` fills the bar from the slave's mount skills, then adds
"unbind" (`icon_skill_vehicle03.dds`) and, at seat 1, "showSlaveInfo" (`icon_skill_info_view.dds`);
`FUN_392045f0` adds "command_unmount" and fires `MODE_ACTIONS_UPDATE` (0x5f). Summon Info still opens
only by `SHOW_SLAVE_INFO` (0x60); boarding never opens it. `mode_action.alb` hides `slaveInfoFrame` at
`GetModeActionsCount()==0`. [R]

## 3. Ship equipment and cannons

`slave_equip_slots` (slave template → `equip_slot_id`, `attach_point_id`) and
`slave_customizings` / `slave_customizing_equip_slots` name the visible parts. Slot ids [R]:

| slot | part (attach points) |
|---|---|
| 0 | figurehead (57, 9–16) |
| 1 | signal/bell (81) |
| 2 | helm (35) |
| 3–8, 25, 26 | cannon (9–16, 15/16) |
| 9, 10 | sail (21, 22) |
| 11, 12 | mast (19, 20) |
| 13 | light (28, 29) |
| 15 | plank (26, 27) |
| 17–23 | other device (48–53, 47, 11, 12) |
| 24 | spyglass (80) |

Player hulls declaring slots: speedboat 14/76 (8), merchant 75 (8), small-sailboat test hulls 21/22/132
(9/27/27), fishing 110 (5); rowboat 15, Bipa 92 and most NPC hulls have none. Items map through
`item_slave_equipments.item_id → (slave_equip_pack_id, slot_pack_id)`, gated by
`allow_to_equip_slaves`/`allow_to_equip_slots` (e.g. steel cannon 28852, helm 29726, sails 29692/3, masts
29690/1, spyglasses 29728/36, oxygen 29727, planks 29724, bell 29725, lamps 29723). [R]

UI: `x2ui/slave/slave_info.alb` + `slave_equipment.alb` call `X2SiegeWeapon.GetSlaveCustomizingType`,
`GetSlaveEquipSlotInfo`, `PickEquipSlotOfSlave` (left click/drag), `UninstallEquipSlotOfSlave` (right
click), `CanSlaveEquipPickedItem`; bindings in `FUN_394ac320`, slot info in `FUN_394abd90`, fit test in
`FUN_394ac1e0` (dragged type 0x1c). Slots are 0-based, 0..26. [R] The part is a **doodad** from
`item_slave_equipments.doodad_id` at the slot's point (defaults in `slave_doodad_bindings`: masts
1130/1, sails 1135/6/5732/3/5780/1, ladder 1187, lamps 1188/5739, planks 1191, anchor 1195/5735, helm
2374/5738, bell 2611/5736, rear ladders 3299/3300, oxygen 3445, spyglasses 4726/4896/7/4906/5734/6563,
crane 6560, fishing ladders 6561/2, tanks 6655, bait 7911). `SCUnitEquipmentsChanged` 0x94 swaps the part; `SCSlaveEquipmentChanged` 0x68 is the transaction
result; events 0x69/0x262 refresh the window. [R]

**Cannons** are separate child slaves (`slave_kind_id` 8, slave 10 model 117 = `vehicle_models` 10,
`galleon.cannon`; slave 133 model 1535) bound at the hull's cannon points (`slave_bindings`: hull 21/92
→ slave 10 at 9..16). The gunner is bound by sending `CSBindSlave` for the **cannon's** tl id (the server
`is_manned_cannon` path). Firing is a normal slave/mount skill; `vehicle_models` 10 has
`turret_yaw_angle_max 22°`, `turret_pitch_angle_max 35°`, no installed turret. [R] The turret bones are
driven by `UpdateGunTurret` (`FUN_390d14b0`): it reads `bone_yaw` and `bone_pitch`, derives target angles
from the unit's aim (`+0x2cc`/`+0x2d0`), limits/smooths with `turret_yaw_angvel`/`turret_pitch_angvel`
(`vehicle_models`, deg/s) and writes the bone matrices every frame. [R]

The "stuck" pair on the ship info panel is live: `IsMountingShipStucked` (`FUN_394aa840`) compares the
target with the current transfer/mount, and `DestroyStuckedShip` (`FUN_394aaad0`) sends a
target/skill request when the ship is stuck (flag `+0xde`); `ShipMoveType.stuck` is the source. [R]

## 4. Damage, sinking, and the water

The model state is a pure function of HP (`FUN_3989f8b0`):
HP% > 75 → 1 (`normal`); 50 < HP% ≤ 75 → 2 (`damaged25`); 25 < HP% ≤ 50 → 3 (`damaged50`); HP% ≤ 25
→ 4 (`damaged75`); HP == 0 → 6 (`dead`). State 5 (`dying`) exists only for prefab models with a `dying`
entry; ships have no `ship_models.dying`, so a ship goes straight to `dead`. [R] `FUN_390eb610` compares
the computed state with the current one and calls `FUN_390e7b20`, which resolves the model variant with
`FUN_397a7980` and swaps the prefab brush. [R] The galleon `dead` prefab (`game/prefabs/galleon.xml`,
`<Prefab Name="galleon.dead">`) is the `galleon_dead.cgf` hull plus one `ParticleEffect`
`X2_EFFECTS.e_effect.caraval_water_destroy` at z 2.83; the damaged prefabs carry `_spawn_effect_dmg_*`
particles, water `exhaust` and `splash_l/r`. The "sinking" is therefore a model swap plus that particle,
not a scripted descent. [R]

Water interaction of the ship is visual: `floating_height`/`floating_wave_height`/
`floating_wave_period_ratio` exist on `vehicle_models` (floatship 15: 1.0/0.3; ferry 9; carriage ghost 21
−1.0) and are consumed by the physics/transfer layer, not by `ShipUnitModel`; the client draws the
server-sent pose. [R data, ? exact consumer] `ship_models.water_*` parameters are likewise physics data.
The only water feedback in the client is the ripple/exhaust/sound chain above. [R]

## 5. Gliders

A glider is a **back-slot item**: `item_backpacks.backpack_type_id = 2`, `asset_id` = glider `.chr`
(`objects/env/06_unit/06_glider/glider_b_lv_*.chr`, `glider_sloth.cdf`, …), `asset2_id` = stowed back
`.cgf` (`*_back.cgf`). `ChangeBackpackVisual` (`FUN_39360bb0`) swaps the back model, applies the asset's
offset/rotation and plays `start` when opening and `endloop` when folding; the item id 0x1a
(backpack slot) alone plays `default`. [R] Every real glider item (e.g. 14677, 23381–3, 28887, 30621,
34579, 8000003/4/9) uses a skill with `skill_controller_id = 2687`; the skill's effects are not the
gliding buff (23381 → BuffEffect 6466), so the controller is the trigger. [R controller id, I mechanism]

The flight state is a **buff** with `buffs.gliding='t'` (55 rows) and parameters
`gliding_{fall,move}_speed_{slow,normal,fast}`, `rotate_speed` (deg/s), `land_height`, `lift_{count,
duration,height,speed,valid_time}`, `sliding_time`, `smooth_time`, `startup_{speed,time}` plus
`duration`; `LoadBuffDescs` is `FUN_39733a90`, and 53/55 rows have `remove_on_land='t'` (1675, 7431 are
not). [R] The 1.2 client function that converts these columns into a flight was **not located**; the only
strings are the cvars `gliding_mouse_ad/ws` (registered in `FUN_390f7b40`, default 8.0) and the item
visual code. [R absence] The 1.2-compatible rules in Bevy (`ingame/glide.rs`) are therefore an
explicitly 10.x-derived reconstruction: opening at `startup_speed` for `startup_time`; W/S select the
fast/slow move+fall sets; A/D turn at `rotate_speed`; Space lifts `lift_count` times within
`lift_valid_time`; within `land_height` the glider slides for `sliding_time`; landing removes the buff.
[I/Bevy]

Rows [R]: 1458 (move 6/8/10, fall 0.5/1/1.5, 60°, 2 lifts); 1460 (4/5/6, 3 lifts); 1461 (6/7/8,
fall 1.5/2/3); 1463 (fall −1/0/1, no lifts); 2097 (8/10/14, fall −1/0/5); 2098 (7/9/11, 0.3/0.5/1.5,
3×1.5 m); 3583 (6/7/8, 5×3 m, 90°); 7206 (move 0.1/9/11, slow fall 3.5, 90°).

`buff_skills` gives the in-flight mode actions [R]: fold 17657 (all); boost 13435 (30 s cd) or strong
boost 23040; descend 21297 (6 s); barrage 13440 (3 s); flame 14236 (12 s); smoke 15266; bomb 13430;
shadow flight 17624 / invincible 17663 / self-destruct 17666; aerobatics 19331/19167 (6 s); sloth
24332/24326; dragon tears 23270. Only gliders with the buff get these. `[R rows]`

Deployment option: config variable `glider_start_with_double_jump` is registered with **default 0**
(`FUN_390f7b40`, `param+0x2ac`), while the option text is `option_item_double_jump` ("점프키 두번입력으로
날틀 펴기") and tooltip "날틀을 착용하고 있을 경우 점프키 두번으로 날틀을 작동합니다." (ui_texts 2982/3565).
The original UI therefore lets a second jump with a glider equipped use it; whether a minimum height or
freefall state is also required was not found in code. [R config/text, ? height gate] The item/stowed
visual pair is `OPTION_ITEM_SHOW_GLIDER` / `option_item_hide_glider` ("날틀 감추기") and
`OPTION_ITEM_USE_GLIDE_WITH_DOUBLE_JUMP`. [R]

## 6. Other land vehicles (cars, tractors)

These are `models.sub_type='VehicleModel'`, driven exactly like the Farm Wagon (`CSMoveUnit` type 2,
`VehicleMoveType`; `vehicle.rs` in Bevy). `vehicle_models` rows [R]:

* tractors 38/51/58/65/8000001: `velocity 4–5`, `angVel 0.4`, `use_center_spindle t`,
  `max_climb_ang 45`, `lin/rot inertia 0.4/0.2`;
* speedcars 45–48/50/52/54–56: `velocity 15` (17 for UCC), `angVel 1.0`, **`use_wheeled_vehicle_simulation
  t`**, `wheeled_vehicle_power 50–60`, `max_gear 2`, `can_fly f`, `use_center_spindle f`.
  `use_wheeled_vehicle_simulation` selects the CryPhysics wheeled branch in the local controller
  (`FUN_390acee0` → `FUN_3909c660`); the wagon/tractor use the generic kinematic branch. [R]
* `can_fly t` is also set on floating transfer models (9/15/16/25/26/27), selecting the floating path.
[R]

`VehicleMoveType` (reader `FUN_397c0470` case 2): position, i16 velocity×3, i16 quaternion×3, f32
angular velocity×3, f32 steering (−1..1), u8 wheel count, f32 wheel speeds[n]; Bevy's `VehicleMove`
matches. Wheels are bone-driven (`FUN_390d6b10`): `wheelN` uses `vehicle_models.wheel`, `wheel_type2N`
uses `wheel2`, radius = ½ bbox. [R] Helm IK/`Bone_spindle` are the wagon code (`re-wagons.md` 2);
summoning is the land `SlaveLocator` branch + skill 15802. [R/I]

## 7. Transfers (carriages, ferries, airships, lifts)

`transfers` rows are world-placed vehicles: `model_id` (a `models` row with `sub_type='VehicleModel'`),
`wait_time` (s), `cyclic`, `path_smoothing`. `transfer_paths` names spline paths (`path_name`) per
`transfer` and carries `wait_time_start` / `wait_time_end`. `transfer_bindings` attach child transfers at points (mostly 30); `transfer_binding_doodads`
attach doodads at points 2/3 (e.g. 5890/5921). [R] Examples: 3 Marianople tram (606, 8 m/s, 15 s), 6 carriage (654, 8 m/s,
20 s), 23/24 Andelph train/carriage (8 m/s, 10 s), 51/52/65 airships (657, 9.6 m/s, 90 s, smoothing 15),
57–64 ferry (116, 4 m/s, 20 s), 82 flying carpet (896, 8 m/s). [R]

Path geometry is client data: `game/worlds/<world>/level_design/zone/<zone>/client/transfer_path.xml`,
loaded by `TransferManager::LoadPaths` (`FUN_393539e0`; the file-name pattern is the literal at
0x399cfeb4). Format: `<Objects><Transfer Name="mari_tram_01" Type="3" cellX="0" cellY="1"><Points>
<Point Pos="x,y,z"/>…`. `Type` is the `transfers.id` (mari_tram_01 → 3, lily_solzreed_in1 → 6), `Name` is
`transfer_paths.path_name`; 111 zone files, 121 `<Transfer>` blocks, 114 names resolve in the DB. [R]
Example: `mari_tram_01` has 25 points, 344.0 m; at 8 m/s that is 43 s per leg. `lily_solzreed_in1` has 19
points, 143.7 m. [R computed]

Boarding is the 0x67 `CSBoardingTransferPacket` (`{tl: u16, ap: u8}`). `FUN_393565c0` sends it for a
target transfer; `FUN_393546b0` returns the first **free attach point 2..7** and the packet carries that
`ap`. Leaving is `X2.UnBoardingTransfer`
(`FUN_39440d30`), which sends the same opcode with the transfer's object id and `ap = 0`; the UI's
`transfer.alb` shows a one-button bar on `TRANSFER_BOARDING` (0x64) and hides it on
`TRANSFER_UNBOARDING` (0x65); the button sends `UnBoardingTransfer`. [R] The transfer positions arrive
as `CSMoveUnit`/`SCUnitMovement` move type 5 (`TransferMoveType`): position, i16 velocity×50, quaternion,
three f32 angular velocity, i32 steering, i32 `pathPointIndex`, f32 speed, bool reverse; the client does
not integrate it (`FUN_393540a0` refreshes transfer path data around the player; no integrator found). [R]

Speeds/stops are data: `transfers.wait_time` and per-leg `wait_time_start/end` (mari legs 15 s,
Solzreed 20 s, airships 90 s). [R]

## 8. Siege weapons

Siege weapons are `slave_kind_id` 7/8 slaves on `VehicleModel`s. Representative `vehicle_models`: catapult
1 (2.0 m/s, 60° climb), trebuchet 2/36/59/60 (2.5–6.0, **installed turret**, yaw ±180°, pitch −10..70°,
yaw 0.1–30°/s), ram 4/28 (2–10), tower 5 (3.0, 75°), ballista 6 (3.5, 0.8 rad/s, yaw rate 360°/s),
siege tanks 30/33/34 (3.0–5.0, turret ±180°, 30–60°/s), cannon 43 (0.0, ±180°, pitch −20..30°), Hwang
tank 35 (0.0, ±360°). `installed_turret` marks the prefab turret assembly. [R] Driving is generic type 2,
seat point 1, and `UpdateGunTurret` aims the turret bones from the player's aim with the
`vehicle_models.turret_*` limits/rates. [R]

The Summon Info numbers are **live**, not the DB velocity: `GetSiegeWeaponSpeed` (`FUN_394aabd0`)
returns the signed forward speed (dot of velocity with the model forward, rounded by `FUN_390699b0`),
and `GetSiegeWeaponTurnSpeed` (`FUN_394aae90`) returns body angular velocity z × 57.2958 (deg/s). Both return 0 unless the local player is in the mounted state (player state 2).
`GetVehicleInfo` (`FUN_394ab270`) returns `gear/handbrake/tractionCtrl` only for a wheeled-simulation
vehicle; a ship or a kinematic wagon gets nil and the script hides the gear block. [R] `IsDriver`
(`FUN_394aafd0`) compares the player's attach point with 1. [R]

## 9. Packets (name, opcode, fields, when)

Opcodes are from the 1.2-mirroring server (`aaemu-game/src/opcodes.rs`), field bodies from the server
writers and the client readers. [R]

| packet | op | fields / when |
|---|---|---|
| `CSStartSkill` | 0x52 | skill 15802 + position target; summon scroll, 5 s cast/10 s cd |
| `CSBindSlave` | 0x31 | tl u16; target interaction within 4 m (ship/cannon/passenger) |
| `CSDiscardSlave` | 0x32 | –; leave a seat, refused above 5 m/s (`DAT_3999f85c=25`) |
| `CSDestroySlave` / `CSDespawnSlave` | 0x30 / 0x2f | tl u16 / slaveObjId bc; dismiss / recall |
| `CSBoardingTransfer` | 0x67 | tl u16, ap u8 (2..7 board, 0 leave) |
| `CSMoveUnit` type 2 | – | pos, i16 vel×3, i16 rot×3, f32 angVel×3, f32 steering, u8 n, f32 wheelAngVel[n]; land driving |
| `CSMoveUnit` type 3 | – | same pose, i8 steering/throttle, u16 zone, bool stuck; ship pose broadcast |
| `CSMoveUnit` type 4 | – | i8 throttle, i8 steering; helm input, driver only |
| `CSMoveUnit` type 5 | – | pos, i16 vel×3 (×50), quat, f32 angVel×3, i32 steering/pathPointIndex, f32 speed, bool reverse; transfer pose |
| `SCSlaveCreated` | 0x61 | owner bc, tl u16, slave bc, hideFx bool, i64 unk, creator string; after summon |
| `SCMySlave` | 0x66 | objId, tl, name, templateId, hp, maxHp, x, y, z; own slave map |
| `SCSlaveBound` | 0x64 | masterId u32, slaveObjId bc; point-1 bind |
| `SCUnitAttached` / `SCUnitDetached` | 0x76 / 0x77 | child bc, point u8, parent bc, reason u8 / child bc, reason u8 |
| `SCUnitEquipmentsChanged` | 0x94 | slot u8 + item; ship part changed |
| `SCSlaveEquipmentChanged` | 0x68 | characterId, tl, dbSlaveId, bts, two items/slots, success; part transaction |
| `SCUnitFlyingStateChanged` | 0x7a | objId bc + bool; flying creature (not glider) |

## 10. Inferred / not found

1. **[I]** Ship physics is server-authoritative (type-3 pose / type-4 helm); the original world server
   was not read.
2. **[?]** Water test for the ship ghost: `FUN_39348bf0` checks distance and OBB only; a water lookup may
   sit in the skill/SummonPos layer or `FUN_391040b0`.
3. **[?]** The 1.2 glider per-frame integrator: not located. Controller 2687, the buff columns and the
   10.x-shaped Bevy rules are documented, but smoothing/lift/slide/startup were not read in 1.2 code.
4. **[?]** Fire sites of `SLAVE_SHIP_BOARDING/UNBOARDING` (0x67/0x68) and
   `TRANSFER_BOARDING/UNBOARDING` (0x64/0x65): ids certain, firing code not found.
5. **[?]** `DAT_39ae6c90/94` (wind) are runtime weather globals; producers not traced, so the sail billow
   is only qualitative.
6. **[?]** Deck walking and the ship's deck collision were not read (seats only cover `SCUnitAttached`).
7. **[?]** The eight slot-0 rows at points 9–16 on slave 21 look like cannons, not figureheads; the data
   does not explain them.
8. **[I]** `use_wheeled_vehicle_simulation` selects the CryPhysics wheeled vehicle; acceleration/gear
   formulas live in CryPhysics and were not read.

## 11. Gaps vs Bevy (ranked)

1. **Transfers are absent.** Original: `CSBoardingTransfer` 0x67 (board at first free point 2..7, leave
   with 0), bar on 0x64/0x65, server type-5 pose, zone `transfer_path.xml`, `transfers.wait_time` and
   per-leg waits. Bevy has no boarding, entity or path loader; only `aa-ui/src/lua/stubs.rs:1943`. Fix:
   add the packet/bar/attach flow and a route layer; read XML `Type` as `transfers.id`.
2. **Ship parts are not equipment-driven.** Original: hull brushes + slot doodads
   (`slave_doodad_bindings`, `item_slave_equipments.doodad_id`) swapped by `SCUnitEquipmentsChanged`.
   Bevy draws every prefab brush unconditionally (`ingame/units.rs:651-659`, `ingame/data.rs:1049-1059`)
   and binds no slot doodads, so equipment changes only the UI (`hud/slave_equipment.rs`). Fix: build
   parts from installed equipment.
3. **No ship visual chain.** Original: rudder (`FUN_390c7410`), 45° wheel (`FUN_390d2990`), sail billow
   (`FUN_390d1c60`), throttle anim (`FUN_390db700`), exhaust/ripple/sound (`FUN_390c78d0/7640/7bf0`).
   Bevy spawns static brushes (`ingame/units.rs:651-659`). Fix: bind named children and drive them from
   `ShipMove`.
4. **No damage/dead model/sinking.** Original HP→state 1/2/3/4/6 (`FUN_3989f8b0`), model swap
   (`FUN_390eb610`/`FUN_390e7b20`), dead prefab + `caraval_water_destroy`. Bevy always draws `normal`
   (`data.rs:1049-1059`). Fix: state→`ship_models` variant and dead particle.
5. **Boarding ranges are wrong.** Original bind precheck squared 16.0 → **4 m** (`DAT_39ac7408`,
   `FUN_39346290`), target interaction 3 m (`DAT_39ac79fc=9.0`). Bevy: `BOARD_RANGE 10.0`
   (`ingame/vehicle.rs:283`), **60 m** own-hull test (`vehicle.rs:227`), `CANNON_REACH 4.5`
   (`vehicle.rs:74`). Fix: 4 m bind / 3 m interaction; treat ships as normal targets.
6. **Glider mode skills absent.** Original `buff_skills`: fold 17657, boost 13435/23040, descend 21297,
   barrage 13440, flame 14236, smoke 15266, bomb 13430, shadow/invincible/self-destruct, aerobatics
   19331/19167. Bevy's `ingame/glide.rs` has flight only. Fix: load `buff_skills` for the active buff.
7. **Glider deploy default/gate differ.** Original `glider_start_with_double_jump` defaults to **0**
   (`FUN_390f7b40`, `param+0x2ac`); Bevy treats it as **on** (`glide.rs:456-458`) and requires freefall or
   ≥3 m (`glide.rs:462-463`), neither found in 1.2. Fix: default off; verify the height gate.
8. **Summon Info is static.** Original live signed speed (`FUN_394aabd0`) and angVel z×57.2958
   (`FUN_394aae90`); `GetVehicleInfo` only for wheeled sims (`FUN_394ab270`). Bevy returns authored values
   and always `gear 0` (`hud/vehicles.rs:298-304`, `:340-355`). Fix: live values; nil for ships/kinematic.
9. **No turret aiming.** Original `UpdateGunTurret` (`FUN_390d14b0`) writes `bone_yaw`/`bone_pitch` with
   `turret_*` limits/rates; Bevy has no turret code. Fix: bone aim from the DB limits.
10. **No water feedback.** No splash/ripple, exhaust/wake, moving-water sound or dead-water particle in
    Bevy (`FUN_390c7640/78d0/7bf0` + prefabs in the original).
11. **Bevy adds an anchor.** `Space` zeroes throttle (`ingame/ship.rs:70-79`); 1.2 has no anchor control
    (no mount skill 11647/11648; anchor doodad static). Remove or mark as a deviation.
12. **Wheeled cars/tractors are arcade.** Bevy accel 8/brake 14, level quaternion
    (`ingame/vehicle.rs:20-26`), ignores `use_wheeled_vehicle_simulation`, `max_climb_ang`, suspension and
    the gear panel. Gate a physics path or document.
13. **Stuck-ship actions missing.** Original `IsMountingShipStucked`/`DestroyStuckedShip` plus
    `ShipMoveType.stuck` (`FUN_394aa840`/`FUN_394aaad0`); Bevy decodes `stuck` with no action.

## 12. Live oracle measurements

None: read-only code/data work; no client, server or capture was started. The only reused
measurements are the Farm Wagon rows in `re-wagons.md` §7.

## 13. Open questions

* What exactly fires `TRANSFER_BOARDING`/`UNBOARDING` and `SLAVE_SHIP_BOARDING/UNBOARDING`, and do they
  drive only sound/camera?
* 1.2 glider per-frame integrator: find the consumer of the `buffs.gliding_*` fields and confirm the
  Bevy smoothing/lift/slide formulas or replace them.
* Is a water lookup part of the ship placement ghost, or only the skill/server?
* Ship deck walking and the collision surface the original uses for passengers.
* Exact `wait_time_start`/`wait_time_end` precedence vs `transfers.wait_time` (per-leg vs whole route).
* Whether `installed_turret` gates the turret assembly or only the prefab construction; and the
  cannon/ballista fire controllers.
