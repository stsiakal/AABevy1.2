# Click-to-move, jump and teleport notes (1.2 x2game)

Addresses are VAs at base `0x38FF0000`; sizes from `funcs.csv`.

## 1. Click-to-move and the path controller

### 1.1 `MouseClickControl::Normal` `FUN_390f4960` (0x390f4960, 715 B)

Profile string `MouseClickControl::Normal` (`0x399a66f0`). It is called from
`ControlMouseClick` `FUN_390f50a0` (0x390f50a0) when `DAT_3a5a4ea0 == 0`. It filters the click
type (`param_1 == 1` / `3`), the current target/cursor ids and the “pathfinder” flag, then calls
`FUN_390a6af0(param_1, DAT_39aefb9c, DAT_39aefc08, DAT_39aefc0c)` **[R]**.

### 1.2 `FUN_390a6af0` (0x390a6af0, 1848 B) - click-to-move

Callers: `FUN_390f4960`, `FUN_393b3ec0` (interaction approach), `FUN_393b7440`, `FUN_393bf270`.

```c
player = GameClient->controlled_unit;          // FUN_39360020(*(GameClient+0x3af0))
if (player dead / mounted / using vehicle / ... ) return 0;
cameraPos = FUN_390b3850();
cameraQuat = FUN_390b3870();
ray = RayWorldIntersection(Game, cameraPos, cameraDir, 0x117, 0x2048f, ...);
if (ray) {
    target = hit unit;                       // FUN_39035110 / FUN_39360020
    // reject model types 4 and 2 (vehicle), dead, etc.
    if (target unit) {
        FUN_39095e70(targetId, target+0x1f, param4);   // set movement target
        return 1;
    }
    path = FUN_390ca3d0(localPlayer, hitPoint);         // create path controller
    if (cvar("x2_effects.ui.click_to_move") != 0) {
        if (!DAT_3a5a4de0) DAT_3a5a4de0 = FUN_390a58b0(0, &point, 0x400000);
        else call the queue object;
    }
}
```

The cvar is looked up by name `"x2_effects.ui.click_to_move"` (`0x399a3180`) through the config
object `FUN_... +0x148`; the default is 0 (option 54 off). The ground point is the ray hit; the
click move is the ordinary path controller, not a new movement mode.

### 1.3 Target setter `FUN_39095e70` (0x39095e70, 90 B)

```c
this[0x2c] = targetId;        // +0x2c
this[0x30] = arg2;            // +0x30
this[0x34] = arg3;            // +0x34
unit = FUN_390b3750()->vtable[0x208]();
if (!this[0x18c] && unit) {
    this[0x18c] = 1;
    this[0x190] = unit[0x468];   // inertia/speed
    FUN_391130c0(0);
}
```

This is the “set movement target and start” primitive used by both click-to-move and
`FUN_393b3ec0`.

### 1.4 Path controller

* `FUN_390ca3d0` (0x390ca3d0, 242 B): if the unit is a player (`vtable+4 == 1`), reuses
  `unit[0x16]` when it is the path controller (type 7) or allocates a new 0x1a0-byte controller
  `FUN_390b2af0`; calls `FUN_390ca140` and stores it at `unit[0x16]`.
* `FUN_390b2af0` (0x390b2af0, 504 B): constructor; fields `+0x06 target`, `+0x0c/+0x10`
  ids, `+0x20..` callback object, `+0x38` start point, `+0x58` speed, `+0x188` active flag,
  `+0x4bd` reset flag, `+0x5c` timer. It calls `FUN_3902f390` and `memset`.
* `FUN_390a58b0` (0x390a58b0, 1094 B): queue element used by `FUN_390a6af0`; it stores the
  point, target and flags (`0x400000`) and is fed to the path controller.

Keyboard cancellation is not in `FUN_390a6af0`; the input layer/manager clears the target when
the player starts moving (U-C1).

## 2. Move-to-point used by interactions

`FUN_393ba480` (0x393ba480, 1340 B), caller `FUN_393b31e0` (0x393b31e0, 30 B; itself called by
`FUN_393b3ec0` and `FUN_393bfc70`):

1. Get the controlled unit and its movement object (`vtable+0xb0`), initialise a request with
   `FUN_390a8dc0`.
2. `dir = point - actorPos`; normalise. Build the camera-space axes from the quaternion
   (`FUN_390b3750()->vtable+0x228`); `dot = dir · forward`, `side = dir · right`.
3. `speed = DAT_3999c478` = **1.5**; compute the forward/side request components.
4. Set request flags `0x800` (`local_f0[0] |= 0x800`), `local_f0[0] |= 0x40`, call
   `FUN_39097bd0`, `FUN_39098380`, submit via the unit movement vtable, `FUN_390a5d00`.

`FUN_393b3ec0` (0x393b3ec0, 787 B) is the interaction approach:

* if the target is out of interaction range (`FUN_39093800` with `game_cfg+0x2a0`), it calls
  `FUN_390a6af0(3, ...)` (click-to-move);
* if in range, it calls `FUN_393b31e0(player, target+0x18)` -> `FUN_393ba480` to step to the
  interaction point, then `FUN_39429e40` and the interaction/use path.

## 3. Auto-run

The auto-run latch and packet pump are in the core input/send code:

* `FUN_39404d50` (0x39404d50, 5971 B): reads the forward/back axes, both mouse buttons and the
  auto-run action; the latch is `input_state+0x0c` / `+0x31` (see `port-movement/MAP.md` §2.5).
* `FUN_39406650` (0x39406650, 228 B): consumes bits `0x1`/`0x2000` of `state+0x84` and pushes the
  remaining flags through the game vtable `+0x58`.
* `FUN_39406740` (0x39406740, 307 B): calls `FUN_39406650` for the player and the five extra
  slots, accumulates `dt` at `state+4`, and calls `FUN_394064b0` to choose the packet writer.

No separate auto-run movement function exists; reuse the core port.

## 4. Follow

There is **no local-player follow movement controller** in x2game:

* `followUnit` / `GO_TO_FOLLOW_UNIT` are the AI controller `FUN_3933c8f0` (0x3933c8f0, 624 B;
  strings `0x399cfaa8`/`0x399cfa84`). It sets the AI state, stores the followed unit id at
  AI `+0x34`, calls `FUN_390242c0("followUnit", …)` and `FUN_3933b070(0,"GO_TO_FOLLOW_UNIT",0)`.
  It is used by NPCs/pets, not by the client player.
* The client exposes `TOGGLE_FOLLOW` (`0x399be05c`), `toggle_follow_target` (`0x399d5784`) and
  the errors 495–500 (`cannot_follow_in_combat`, `cannot_follow_non_party`,
  `cannot_follow_too_far`, `msg_following_begin/end`) in the string table
  (`FUN_3956b860`), but no player movement function consumes them.

Treat player follow as server-authoritative (U-C2). The test can send the follow command and
compare the resulting server movement, not a client function.

## 5. Jump

### 5.1 Input

Jump is action index 6 in the engine action table (`0x39ac8e68`, core map §2.1), bound to Space
in `game/default_binding.g`. `FUN_39424580(6)` is read by `FUN_3930b5b0` (swim-jump gate) and by
the click code; `FUN_3912b330` consumes the “jump requested” state from the movement request.

### 5.2 `FUN_3912b330` (0x3912b330, 3199 B) - ground/jump/air

The jump branch is inside the `controller+0x31` (jump requested) && `controller+0x98 == 0`
(airborne) guard:

```c
if (horizontal input != 0) {
    if (actor+0x5c > 0) {                          // jump available
        controller+0x60 = 7;                       // mode 7 = jump
        jumpDir = actor+0x9c..0xa4;                // direction
        ext = actor+0xd4..0xdc;                    // external impulse
        mag = |jumpDir|;
        v = sqrt(2 * mag * actor+0x470) / mag - (jumpDir != 0 ? 0.5 : 0);
        // build jumpVec = quat * jumpDir * v, add ext
        controller+0x98 = 1;                       // airborne
        controller+0x99 = 1;                       // fired "jumped"
    }
} else {
    if (!animation contains "jump_in_place") {
        controller+0x98 = 0x101;                   // jump in place
        unit vtable +0x2a8(0);
    }
}
```

`actor+0x470` is `game_jump_height` (actor_models model 1: 1.3), copied by `FUN_3911b950`
(0x3911b950, 624 B) at `+0x470` from desc `+0x44`. The formula uses the direction magnitude and
the height; the literal gravity `25` is not read in this function (the physics model owns it)
**[R/I]**.

### 5.3 Applying / events

* `FUN_3912ac50` (0x3912ac50, 703 B) copies the request into the actor movement/graph state and
  fires `"jumped"` (`actor vtable +0x184`) once when `controller+0x99` is set; it also writes the
  final velocity/quaternion and clears the flag.
* `FUN_3912bfb0` (0x3912bfb0, 1670 B) is the ground-align/final velocity pass; it is called
  after the mode function by `FUN_3912dd40` and zeroes horizontal velocity when the unit is not
  allowed to move.
* `FUN_3914ae50` (0x3914ae50, 856 B) registers the special-move trigger data: string
  `Triggered On SpecialMove. 0=Jump,1=SpeedSprint,2=StrengthJump` (`0x399ae3c4`).

### 5.4 Air control / double / mount jump

* Horizontal momentum is carried by the request velocity; the physics parameters
  `physics_living_k_air_control=0.9`, `air_resistance=0.29`, `max_vel_ground=30`
  (`actor_models` row 1) are consumed by cryphysics, not by x2game. The Rust client must feed
  the same velocity and let its physics step integrate.
* Double jump is not a second impulse in `FUN_3912b330`; with a glider equipped the second jump
  is the glider skill (`glider_start_with_double_jump`, default 0 at `game_cfg+0x2ac`) and the
  item/buff path in `glider.md`.
* Mount jump uses the same action and the mate’s movement controller; the graph action is
  `jumpMP01` (`re-mounts-full.md` §3), and the packet remains `CSMoveUnit`.

## 6. Teleport / portal movement and camera

### 6.1 Handlers

| handler | addr | size | body |
|---|---|---|---|
| `OnTeleportUnit` | 0x391d6ed0 | 370 | `(type, objId, pos+yaw)` -> `FUN_3930c9e0`; type 2 sends opcode 0x6e; sets `GameClient+0x109a/0x10a0/0x10a4` |
| `OnAboxTeleport` | 0x391dc5a0 | 246 | `pos = currentPos + axis * -0x400`; `z = water/ground + 10.0`; relocate local unit via `client vtable+0x44` |
| `OnNaviTeleport` | 0x391dc8c0 | 138 | `FUN_3930c9e0(0, 0, pos, 0)` |
| `OnUnitPortalUsed` | 0x391d3970 | 122 | `FUN_393558e0` portal effect |
| `OnCharacterPortals` | 0x391d3760 | 118 | UI list |
| `OnPortalSaved` | 0x391d3860 | 114 | UI list |
| `OnPortalDeleted` | 0x391d38e0 | 144 | UI list |

### 6.2 `FUN_3930c9e0` (0x3930c9e0, 378 B) - teleport reset

```c
void FUN_3930c9e0(int type, int objId, float* pos, u32 yaw) {
    if (type != 0) { FUN_39428370(type); return; }   // error/UI path
    if (!GameClient || !localUnit) { FUN_391a4e00(); return; }
    FUN_39202fb0(0x13,0); FUN_39202db0(0x13,0);
    // write the new position/quaternion to the unit (vtable+0x44)
    // camera yaw = cos/sin(yaw * 0.5) or the supplied quaternion
    FUN_3915e930(0);        // camera+0x84 = 0, FUN_3915ccf0
    FUN_3915af10();         // camera+0x1f8/0x1fc = 0 (FOV blend)
    DAT_39aff9d0[0x0c] = 1; DAT_39aff9d0[0x0d] = 1;
    FUN_391a4e00();         // send position notify (opcode 0xb4)
    FUN_39171800(3);        // fade/loading state
}
```

The exact argument typing is from the decompile (`undefined4*`/`float`), but `OnTeleportUnit`
calls it with `(type, objId, pos, yaw)`; treat `type`/`objId` order as U-T1 until a live call.

`FUN_391a4e00` (0x391a4e00, 270 B): builds a position+quaternion from
`FUN_390b3850`/`FUN_390b3870`, calls `FUN_397ae690` to serialize and `FUN_39186810` to send
opcode `0xb4`, clears `GameClient+0x10a0`.

### 6.3 Safe-zone teleport

`FUN_39181cc0` (0x39181cc0, 285 B) is `PhysicsManager::UpdateSafeZoneTeleporting`: while the
queue at `+0x20`/`+0x21` has entries whose timestamp is due (2 ms grace), it calls
`FUN_39181a20(this+0x24+index*0x14)` and advances the ring. `FUN_39181a20` (0x39181a20, 660 B)
is `ProcessSafeZoneTeleporting`:

* distance from the pending target to the actor must be <= `DAT_399b2cf0`;
* resolve the unit id at `+0x04`, test the world `"safe_zone"` position with `FUN_39181660`;
* if valid, write the position through the local client vtable `+0x44`; also move the
  follow/slave unit if present.

`FUN_39181de0` (`PhysicsManager::Update`) calls it after `FUN_39181cc0` every frame.

### 6.4 Camera side

`FUN_3915e930` (0x3915e930, 30 B): `camera+0x84 = 0` then `FUN_3915ccf0` (yaw settle).
`FUN_3915af10` (0x3915af10, 29 B): `camera+0x1f8 = 0`, `camera+0x1fc = 0` (FOV blend reset).
`FUN_39171800` (0x39171800, 425 B): screen fade/loading state (calls the render/UI transitions).
These are the only camera operations in the teleport path; the normal unit camera
`FUN_39169860` resumes on the next frame.

## 7. Test recipes

### 7.1 Click-to-move

1. Stand the live player in the open, aim the camera at a ground point; call
   `FUN_390a6af0(1, DAT_39aefb9c, DAT_39aefc08, DAT_39aefc0c)` (or synthesize a click through
   the normal UI path). Hook `FUN_390a58b0`, `FUN_390ca3d0`, `FUN_39095e70`; compare the queue
   point/target and the resulting path.
2. Repeat with `x2_effects.ui.click_to_move=0` and assert no queue.
3. Press W during the move and observe the target clear (U-C1).

### 7.2 Move-to-point

Call `FUN_393ba480(unit, &point)` with a live unit at a known point; hook `FUN_390a5d00` to
capture the request; compare direction, the `1.5` scale and flags; advance a few frames and
compare the actor position.

### 7.3 Jump

Build the movement context as in `glider.md`; call `FUN_3912b330` with:

* no input + jump -> `+0x60`/`+0x98` 0x101 and no velocity;
* forward + jump -> mode 7, `jumpVec`, `+0x98=1`;
* `actor+0x470` = 0.5/1.3/2.0;
* an animation whose name contains `jump_in_place`;
* then `FUN_3912ac50` -> exactly one `"jumped"` event.

### 7.4 Teleport

Call `FUN_3930c9e0(0, 0, pos, yaw)` on the live player (type 0, not 2 so no packet). Compare:

* actor position/quaternion;
* camera manager `+0x84`, `+0x1f8`, `+0x1fc`;
* `DAT_39aff9d0+0xc/+0xd`;
* the `0xb4` notify captured at `FUN_39186810`.

For `OnAboxTeleport`/`OnNaviTeleport`, feed a synthetic packet body and compare the same fields.
For safe-zone, push a synthetic entry into `FUN_39181cc0` with a valid target and compare the
position write.
