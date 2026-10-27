# Packets and events built by the 1.2 housing placement code

Addresses are x2game VAs. The serializers are not visible as functions in the decompile: each
packet is a small stack object `{ vtable, opcode }` followed by fields; the client finalizer is
`thunk_FUN_3ac1058c` / `FUN_39186810` (`0x39186810`, jmp to `0x3ac1058c`). Field layouts below were
read from the stack constructors and cross-checked against `crates/aa-net/src/housing.rs` /
`farm.rs` (server writers). The wire order is the server reader's order (aa-net is byte-exact for
1.2).

## 1. `CSConstructHouseTax` - opcode 0x56

Built by `FUN_39325f60` (`0x39325f60`, 176 B) from the active builder:

```
state 2 click on a kind-1 design -> FUN_39325f60
  requires manager+0x30 (builder), builder+0x10 == 0, design id != 0x3a167ee8
  local_50 = vtable 0x399cf5c0, local_4c = 0x56
  design = *(*(builder+8)+8)                  // housings.id
  pos    = FUN_3961dd70(GameClient+0x118, &out, FUN_3908c870(builder+8))  // temp housing view position
  FUN_397abc50(this, design, &pos)            // ctor, 45 B
  send
```

Ctor `FUN_397abc50` (`0x397abc50`): `+0xc = design u32`, `+0x10 = x i64`, `+0x18 = y i64`,
`+0x20 = z f32` (+4 padding), exactly the server's `CSConstructHouseTaxPacket`:
`design u32, x i64, y i64, z f32`.

The reply `SCConstructHouseTax` (0xC5) is handled by `FUN_3932e530` (`0x3932e530`, 722 B):
stores `total` in `builder+0x21c`, writes the numbers into the Lua UI and fires **`HOUSE_BUILD_INFO`
0x100** (`FUN_39424280`/`FUN_390155a0` calls). The stock Lua then opens the Start Construction
window.

## 2. `CSCreateHouse` - opcode 0x57

Built by `FUN_3932fe60` (`0x3932fe60`, 701 B), called on the last click
(`FUN_39332200` kind 1 state 2, or the non-house state 3/2 click) and by the Build window's
`NextStepFromBuildCheck` (`FUN_39477cb0` -> `FUN_3932fe60`).

```
fires BUILDER_END 0x225 first (FUN_39424220)
requires builder state error +0x10 == 0 and the design item (builder+0x24) non-null
local_b8 = vtable 0x399cdb48, local_b4 = 0x57
design  = *(*(builder+8)+8)
z_rot   = builder model yaw (model vtable+0xc)
money   = builder+0x21c (total from SCConstructHouseTax)
ht      = DAT_3a167ebc
auto    = FUN_397ccd50() (client option "auto use AA point")
item    = u64 at (builder+0x24)[0..8]
pos     = FUN_3961dd70(GameClient+0x118, ...) of the model position
FUN_397b09d0(design, &pos, z_rot, item_lo, item_hi, money, ht, auto)
send
```

Ctor `FUN_397b09d0` (`0x397b09d0`, 94 B): `+0xc design`, `+0x10/+0x18 x/y i64`, `+0x20 z f32`,
`+0x28 z_rot f32`, `+0x30/+0x34 item u64`, `+0x38 money i32`, `+0x3c ht i32`, `+0x40 auto bool`.
The live constructor capture (2026-10-24) corrected the earlier order: `+0x38` is **money** and
`+0x3c` is **ht**, matching aa-net `HouseCommand::CreateHouse` (`housing.rs:300-323`). The object
also has untouched padding dwords at `+0x24` and `+0x2c`; the wire body omits them.

**Rate limit.** If `now < DAT_3a167ea0 + 5000` the client does not send: it formats the remaining
seconds into the `CANNOT_LOCATE_DOODAD_NOW` message and fires event 0x14 with the text
(`0x3932ffb7..0x393300cb`). Otherwise `DAT_3a167ea0 = now` and sends (`0x393300e8`).
The builder is destroyed and `BUILDER_END` fired by `FUN_3932f960(1)` after a successful send; a
server refusal (`SCErrorMsg`) leaves the ghost in place (`re-placement.md` gap 12).

## 3. `CSDecorateHouse` - opcode 0x58

Receiver of the decoration confirm: `FUN_39327de0` (`0x39327de0`, 2119 B, not listed in
`funcs.csv`; reached from `FUN_3903b9d0` by `jmp 0x39327de0` with ECX=`DAT_39aff98c`).

```
fires BUILDER_END 0x225
cursor = manager+0x34; if cursor->error (+0x40) != 0 -> FUN_39428370(error), abort
local_f0 = vtable 0x399cf674, local_ec = 0x58
house_tl = (cursor->interface + 4)                         // interface = *(cursor+0)
design   = ghost->+4
pos      = cursor transform (x,y,z floats)
quat     = cursor orientation (4 floats)
parent   = ghost->+0x4c                                   // bonded house obj id
item     = manager+0x28 object's first u64 (design item)
FUN_397ae590(house_tl, design, &pos, &quat, parent, item_lo, item_hi)
send
then: free manager+0x28 design item, destroy cursor (FUN_3903b8a0) and manager+0x34,
      FUN_390173b0(0) (cursor mode off)
```

Ctor `FUN_397ae590` (`0x397ae590`, 81 B): `+0xc house_tl`, `+0x10 design`, `+0x14 x f32`,
`+0x18 y f32`, `+0x1c z f32`, `+0x20..0x2c quat[4]`, `+0x30/+0x34 item u64`, `+0x38 parent u32`.
Matches aa-net `HouseCommand::DecorateHouse` (`housing.rs:311-319, 379-391`).
The design/house-tl/parent sources are read from the object graph above; their semantics are **[R]**
for the loads and **[I]** for the field names (confirmed by the server reader and the 10.x RTTI type
`CSDecorateHousePacket`, `0x3A5724D8` **[R10]**).

The server spawns a doodad bonded to the house and answers `SCDoodadCreated`; the housing object
then fires **`HOUSE_DECO_UPDATED` 0x10E** in `FUN_39330270` (`0x39330270`, 158 B) when the changed
house is the selected one.

## 4. `CSCreateDoodad` - opcode 0xE6 (seeds/saplings/farm plots)

Built by `FUN_393a9340` (`0x393a9340`, 392 B) when the doodad builder `DAT_3a17a948` is valid:

```
FUN_39424220(0x1a9); FUN_39424220(0x225)
if FUN_3915b270() == 6 -> FUN_39428370(0xcf), cancel
if builder+0x10 != 0x118 -> show that error, keep
id     = ghost->+0x80 (doodad template/design)
pos    = FUN_3961dd70(player origin, ghost->+0x18)
z_rot  = ghost->+0x24
scale  = ghost->+0x210; if the template asks (FUN_3983b2f0/340) -> CryRandomFloat()*2*pi
item   = u64 at builder+0x14
local_58 = vtable 0x399d2468, local_54 = 0xe6
FUN_397ac850(id, &pos, z_rot, scale, item_lo, item_hi)
send; builder+0x2c = 1; FUN_393a8f60()
```

Ctor `FUN_397ac850` (`0x397ac850`, 77 B): `+0xc id`, `+0x10/+0x18 x/y i64`, `+0x20 z f32`,
`+0x28 z_rot`, `+0x2c scale`, `+0x30/+0x34 item u64` - aa-net `CSCreateDoodadPacket`
(`farm.rs:120-140`, server `doodad/packets.rs`). The position `movq` leaves an untouched padding
dword at `+0x24`; the wire body omits it.

## 5. House interaction: `CSRequestHouseTax` - opcode 0x5C

`FUN_393befd0` (`0x393befd0`, 442 B) is the house interaction path (any house, not only placement):

```
range check (3 m surface gap, FUN_39093860/FUN_39093800, DAT_39ac79fc = 9.0)
FUN_3932b550(unit) -> house
local_20 = vtable 0x399cf5e4, local_1c = 0x5c
FUN_397abcd0(tl)
send
manager+0x78/+0x7c = selected/interacting unit id
fires HOUSE_INTERACTION_START 0x6d with (unit type, category string, 1)
category string: seafarm (7), farm (8,16,17), house (9,10,11,12,15,18), else nil
```

The stock `housing_manager.alb` then opens the material window (under construction) or the
maintain window; the client never casts a build skill on interaction.

## 6. Construction progress and UI events

* `SCHouseBuildProgress` (0xBD) -> `FUN_39327990` (`0x39327990`, 327 B): stores `all` in
  `house+0x140`, `current` in `house+0x144`; if `current` changed it updates the unit animation
  (`FUN_390b4ce0`, `FUN_390ba9c0`), plays `start_effect` (`FUN_39326770`), and fires
  **`HOUSE_STEP_INFO_UPDATED` 0x103** (event call at `0x39327a..`).
* `GetHouseBaseProgress` = `house+0x140` (`FUN_394785d0`, 211 B); `GetHouseRemainProgress` =
  `0x140 - 0x144` (`FUN_394784f0`, 217 B). Finished houses send 0/0, so "under construction" is
  `0x144 < 0x140` (and 0/0 means finished).
* `GetHouseConstructionStepInfo` (`FUN_3947b3b0`, 1065 B) builds one row per
  `housing_build_steps`: item/stack (`FUN_39479460` adds `isLaborPower`, count, `needActions`),
  icon/grade/step (`FUN_39479460` / `FUN_3941a130`), same rows as the Start Construction window.
* `GetHouseDecoCount` (`FUN_394796b0`, 102 B) returns `(placed - builtins, deco_limit - builtins)`;
  `GetActabilityDecoCount`/`GetDecoActabilityByType`/`Point`/`LimitGroup` are
  `FUN_394797c0/79890/7a040/7a310` (194/295/708/446 B) and read `housing_deco_limit_elems` /
  `deco_actability_groups`. `HOUSE_DECO_UPDATED` 0x10E is fired by `FUN_39330270`.
* F/G dynamic actions: the list is the controlled unit's received skill vector plus the doodad/slave
  interaction skills (`FUN_393bcb90` 134 B -> `FUN_3910a950` 204 B); `ExecuteDynamicAction` is
  `FUN_393b1de0` (202 B, casts through the normal skill path, refusing the caster's own slave).
  No client-side `IsMyHouse` filter exists in that list.

## 7. Events

| event | id | fired by |
|---|---|---|
| `HOUSE_INTERACTION_START` | 0x6d | `FUN_393befd0` |
| `HOUSE_TAX_INFO` | 0xff | `FUN_3932e210` (tax reply) |
| `HOUSE_BUILD_INFO` | 0x100 | `FUN_3932e530` (quote reply) |
| `HOUSE_STEP_INFO_UPDATED` | 0x103 | `FUN_39327990` |
| `BUILDER_STEP` | 0x224 | `FUN_39017410` (state string from `0x39ac3088`) |
| `BUILDER_END` | 0x225 | `FUN_39330120`, `FUN_3932fe60` (before send), `FUN_39327de0`, `FUN_393a9340` |
| `HOUSE_DECO_UPDATED` | 0x10e | `FUN_39330270` |
| `WAIT_REPLY_FROM_SERVER` | 0x234 | same decorator path (re-housing §1.6) |

Event ids are the registration order in `FUN_39214380`; the anchors 0x224/0x225/0x103/0x100/0x10E
are read directly in the functions above.
