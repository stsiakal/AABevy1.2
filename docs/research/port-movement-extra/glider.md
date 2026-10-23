# Glider notes (1.2 x2game)

All addresses are VAs at base `0x38FF0000`; sizes from `funcs.csv`. Behaviour is read from
`x2game.dll.c`; DB values from `D:/aaemu-live/rs-game/Data/compact.sqlite3` (read-only).

## 1. Start path, line by line

### 1.1 `FUN_39357920` (0x39357920, 610 B) - build the glide block and start

Callers: `FUN_393220a0` (`X2::GameClient::ApplyChangedParts`, backpack part 26) at 0x649085;
`FUN_39389440` (`ClientBuffMan::ApplyAttrs`) at 0x736056.

```c
// param_1 = player/GameClient user object, param_2 = item or buff desc
// param_3 = 1 from ApplyChangedParts, 0 from ApplyAttrs
local_8 = FUN_390b3b40(param_1);              // player object / unit
if (!local_8 || !param_2) return;
FUN_390b3750()->vtable[0x98](0);              // stop current action
block[0x00] = FUN_39005b20(param_1+8, param_2+16000, 7, 0) * 0.001;  // time
block[0x04] = FUN_39005b20(..., 8, 0) * 0.001;
block[0x08] = FUN_39005b20(..., 9, 0) * 0.001;
block[0x0c] = *(float*)(param_2+0xb8) * 0x3999cf98;  // deg -> rad (1 deg)
block[0x10] = *(u32*)(param_2+0xc8);
block[0x14] = *(u32*)(param_2+0xc4);
block[0x18] = *(u32*)(param_2+0x90);
block[0x1c] = *(u32*)(param_2+0x8c);
block[0x20] = *(u32*)(param_2+0x88);
block[0x24] = *(u32*)(param_2+0xc0);
block[0x28] = *(u32*)(param_2+0x98);
block[0x2c] = *(u32*)(param_2+0xa0);
block[0x30] = *(u32*)(param_2+0xa8);
block[0x34] = *(u32*)(param_2+0x9c);
block[0x38] = *(u32*)(param_2+0xa4);
block[0x3c] = *(u32*)(param_2+0x94);
block[0x40] = *(u32*)(param_2+0xbc);
block[0x44] = 0; block[0x48] = 0; block[0x4c] = 1000;
FUN_3911fc80(1, block);                       // copy to actor+0x168, set stance
if (param_3) FUN_391219c0(1);                 // attach glider model in slot 0x1a
```

The 19 dwords are read in this exact order, but their **names** are mapped by the consumers in
`FUN_3912d860` / `FUN_3911fc80`, not by the column order. The resulting actor block is in
[`MAP.md`](MAP.md) §1.2.

### 1.2 `FUN_3911fc80` (0x3911fc80, 551 B) - set state / copy / landing probe

Called with `(start, block)`:

```c
if (start) {
    memcpy(actor+0x168, block, 0x50);
    actor+0x1b8 = *(u32*)(actor+0x17c);       // startup speed/sink
    actor+0x1bc = 0;
    actor+0x19c = *(float*)(actor+0x19c) / (float)*(int*)(actor+0x190);
                                             // lift_duration / lift_count
    stance = 4;                              // fly
} else {
    // stop / landing check
    velocity = entity->vtable[0x98]();       // current velocity
    if (timer(actor+0x1a8) > 0 && velocity.z <= 0) {
        ray = RayWorldIntersection(Game, actorPos, down, actor+0x1a4 + 0.2, ...);
        if (ray hit) stance = 5;             // land/slide
    } else stance = 4;
}
set stance (actor+0x1c1) via the usual stance helpers;
```

`FUN_3911fc80` calls `FUN_39113d40`, `FUN_39113db0`, `FUN_39113e70` (stance-change helpers),
`FUN_390ac0e0` (physics contact probe) and `FUN_39086370` (ray) **[R]**.

### 1.3 Ride/attach

* `FUN_3911d6c0(1)` (0x3911d6c0, 330 B): sets stance 4, stores `actor+0x1c2=1`, calls the
  stance helpers and `FUN_39113e70`, then asks the item model (`FUN_393548e0(actor+0x38)`) for
  its animation and plays `"loop"`; `param_2=0` plays `"endloop"` and resets stance to 0
  (`0x100` written to actor+0x1c1, which sets +0x1c2=1) **[R]**.
* `FUN_391219c0(1/0)` (0x391219c0, 308 B): resolves item slot `0x1a` (backpack/glider) through
  `FUN_39119140`/`FUN_3911d270`, calls `FUN_3931cd10`, `FUN_39360920`/`FUN_39360780` on the
  equipment and, when attaching, calls the item model's `+0x48(0)` (show). It is the visual
  attach/detach, not the flight state **[R]**.
* `FUN_39121c10` (0x39121c10, 870 B) is the actor animation-event dispatcher. Case `0x11d`
  (`CleanupGlider`) calls `FUN_3911d6c0(0)`, `FUN_391219c0(0)` and clears `actor+0x1a8`.
  Case `0x11e` (`CleanupBackpackAnim`), `0x11f` (`BindHorseStirrup`), `0x120` (`SwimDead`) are
  the neighbours; the 1.2 registration has no `StartGlider` id (see `MAP.md` §1.1) **[R]**.

### 1.4 Fold on buff removal

`FUN_39389d60` (0x39389d60, 1146 B): if the local unit exists and the buff's glider flag
(`piVar2[0x21]`) is set, it checks `actor+0x151`:
* `actor+0x151` set -> `FUN_3911d6c0(0)` + `FUN_391219c0(0)`;
* else -> `FUN_3911fc80(0, 0)`.
It also calls `FUN_3915b040`, `FUN_3907afc0(6)` and the item/sound cleanup **[R]**.

## 2. Movement mode functions

### 2.1 `FUN_3912dd40` (0x3912dd40, 895 B) - dispatch

The actor movement block pointer is `controller+8` = `actor+0x50`; therefore
`*(char*)(iVar3+0x180)` = `actor+0x1d0`, `+0x188` = `actor+0x1d8`, `+0x18a` = `actor+0x1da`,
`+0xa8` = `actor+0xf8`, `+0x171` = `actor+0x1c1`.

| condition | callee | note |
|---|---|---|
| `actor+0x1d0 != 0` | `FUN_3912af10` | fixed-speed flight, 0x3912af10 |
| `actor+0x1d8 != 0 || +0x1da != 0` | `FUN_3912b090` | input flight, 0x3912b090 |
| `actor+0xf8 != 0` | `FUN_3912c770` | mount/vehicle, 0x3912c770 |
| stance 1 | zero velocity | |
| stance 4 | `FUN_3912d860` | glide |
| stance 5 | `FUN_3912b280` | glide straight/land |
| swim | `FUN_3912ce00` | |
| else | `FUN_3912b330` | ground + jump |

After the branch, if `actor+0x1d9 != 0` and the controller velocity is non-zero, it raycasts
down and zeroes the velocity if the unit is within the ground tolerance (`0x3912dd40` tail,
`RayWorldIntersection(Game)`, `DAT_3999cf90=0.01`). This is the “snap to ground while gliding”
path **[R]**.

### 2.2 `FUN_3912b280` (0x3912b280, 166 B) - straight glide

```c
mode = 1;
v = quat(actor+0x2d0) * *(float*)(actor+0x16c);   // block+4, move_speed_normal
controller+0x64..0x6c = v;
```

Called for stance 5. It ignores gravity and input; it is the small “landing/slide” movement.

### 2.3 `FUN_3912d860` (0x3912d860, 1232 B) - glide controller

Inputs: `controller+8 = actor+0x50`, `controller+0x10` = vertical input, `controller+0x48` =
view quaternion (actor+0x2d0), `controller+0x40` = actor.

```c
block = actor+0x168;   // aliased through actor+0x50
block.elapsed += dt;   // actor+0x1ac
block.lift_left -= dt; // actor+0x1b0

if (block.elapsed <= block.startup_time) {
    hspeed = block.move_normal;
    vspeed = -block.startup_speed;
} else if (controller.vertical_input <= 0.1) {          // DAT_3999ccf0
    if (controller.vertical_input >= -0.1) {            // DAT_399a2f24
        hspeed = block.move_normal; vspeed = -block.fall_normal;
    } else {
        hspeed = block.move_slow;   vspeed = -block.fall_slow;
    }
} else {
    hspeed = block.move_fast;
    vspeed = -block.fall_fast;
}
play sound (per tier, unit vtable +0x130);
controller.velocity.xy = quat * hspeed;
controller.velocity.z  = vspeed;

if (block.lift_count > 0 && block.elapsed >= block.lift_valid_time &&
    controller.vertical_input <= -0.1 && block.lift_left <= 0) {
    if (RayWorldIntersection(actor->entity, down, block.lift_height) hit) {
        block.lift_left = block.lift_count * block.lift_duration;
        block.lift_count--;
    }
}
if (block.lift_left > 0) {
    controller.velocity.z = -block.lift_speed;
    controller.velocity.z *= 0.2;          // DAT_3999dd34
}
// vertical smooth: 1 + x + 0.48x^2 + 0.235x^3, x = 2*dt/smooth_time
actor+0x1b8/0x1bc = smooth(actor+0x1b8, controller.velocity.z);
```

The decompile uses `param_1+0x10 <= DAT_399a2f24` with `DAT_399a2f24 = -0.1`; the exact
meaning of `controller+0x10` is U-G2 (pitch/vertical axis). Constants: `0x399a2998=0.48`,
`0x399a2994=0.235`, `0x3999dd34=0.2`, `0x3999cf90=0.01`.

### 2.4 `FUN_3912af10` / `FUN_3912b090` - non-glider flight

* `FUN_3912af10` (370 B): mode 2, velocity = normalised actor forward * `actor+0x1d4 * 0.001`.
* `FUN_3912b090` (486 B): builds an input direction, then multiplies by
  `actor+0x430 * pl_flyingVelocityMultiplier` (`game_cfg+0x1dc`) when `actor+0x1d0 == 0`.
  These correspond to flying creatures/vehicles (the 10.x code has separate fly-mode paths);
  they are not needed for the glider itself but share the dispatcher **[R/I]**.

### 2.5 Sliding timer

`FUN_3911feb0` `CActor::Update` (0x3911feb0, 1098 B):

```c
if (actor+0x1c7 == 0) {
    FUN_3911eff0(dt);
    if (stance == -1) { set stance 0; stance helpers; }
    else if (stance == 5) {
        actor+0x1a8 -= dt;
        if (actor+0x1a8 < 0) { actor+0x1a8 = 0; FUN_39117530(0); }
    }
}
```

`FUN_39117530(0)` sets stance 0 and calls the stance/pose helpers; this is the end of the
sliding state **[R]**.

## 3. Glide block from DB - full examples

`buffs` rows (all `gliding='t'`):

| id | move s/n/f | fall s/n/f | rotate | start s/t | smooth | lift c/h/sp/dur/valid | land/slide |
|---|---|---|---|---|---|---|---|
| 1458 | 6/8/10 | 0.5/1.0/1.5 | 60 | 5.0/0.7 | 0.3 | 2/1.5/3.0/0.5/5.0 | 1.0/2.0 |
| 1460 | 4/5/6 | 0.8/1.2/1.6 | 60 | 2.0/0.7 | 1.0 | 3/1.5/2.0/1.0/5.0 | 1.0/1.0 |
| 1461 | 6/7/8 | 1.5/2.0/3.0 | 60 | 4.0/1.0 | 0.2 | 2/1.5/3.0/0.5/5.0 | 1.0/2.0 |
| 1463 | 2/3/4 | -1/0/1 | 30 | 5.0/2.0 | 0.5 | 0/1.5/5.0/1.0/2.0 | 1.0/0.0 |
| 2097 | 8/10/14 | -1/0/5 | 60 | 2.0/2.0 | 1.0 | 0/0/0/0/0 | 1.0/2.0 |
| 2098 | 7/9/11 | 0.3/0.5/1.5 | 60 | 3.0/1.5 | 0.5 | 3/1.5/5.0/0.5/5.0 | 1.0/1.5 |
| 3583 | 6/7/8 | 0.5/1.0/1.5 | 90 | 3.0/1.5 | 0.4 | 5/3.0/5.0/1.0/5.0 | 1.0/1.5 |
| 7206 | 0.1/9/11 | 3.5/0.5/1.2 | 90 | 3.0/2.0 | 0.2 | 3/1.5/3.0/0.5/5.0 | 1.0/2.0 |
| 1675 | 6/8/10 | 1.0/2.0/3.0 | 60 | 3.0/0.7 | 0.3 | 2/1.5/3.0/0.5/10.0 | 0.5/1.0 (`remove_on_land='f'`) |
| 7431 | 6/8/10 | 1.0/2.0/3.0 | 0 | 3.0/0.7 | 0.3 | 2/1.5/3.0/0.5/10.0 | 0.5/1.0 (`remove_on_land='f'`) |

These values feed the same 0x50 block; the tests should use 1458 and 2098 as the two extremes.

## 4. Camera

No glide branch in `FUN_39169860`; the camera follows the actor. The only bone-based query is
`FUN_39134440` (0x39134440, 870 B): when `DAT_3a5bb1d8+0x94 != 0` and the actor has an attached
model, it asks the attachment for bone id `0x2d` and transforms the actor position by that
bone's matrix. Treat it as the glider/camera attachment point until a live test says otherwise
(U-G4).

## 5. Test recipes

1. **Block copy**: call `FUN_39357920` on the live player with a synthetic desc whose
   `+0x88..+0xc8` are the 17 floats; hook `FUN_3911fc80` and dump its `block` argument
   (5 × 0x10 bytes). Compare with the Rust `GlideParams::from_skill_controller` byte for byte.
2. **Start/land**: on a live actor, set stance 0 and call `FUN_3911fc80(1, &block)`; read
   `actor+0x168..0x1bc`, stance `+0x1c1`. Then set a ground plane and call `FUN_3911fc80(0,0)`;
   compare stance 4 vs 5.
3. **Glide**: construct a `PlayerMovement` context:
   `[0]=0, [1]=actor+0x3ec, [2]=actor+0x50, [0x10]=actor, [0x11]=actor+0x2b0,
   [0x12]=actor+0x2d0, [0x40]=actor`, then call `FUN_3912d860` for the matrix. The context must
   be writable; use a 0x100-byte driver buffer and restore the actor block after each call.
4. **Lift**: set `lift_count=2, lift_valid_time=0, lift_duration=0.5, lift_speed=3.0` and a
   ground plane; after the first call `actor+0x1b0≈0.5`, count 1; after the second, count 0;
   a third call must not lift.
5. **Dispatcher**: set stance 4/5 and `actor+0xf8/0x1d0/0x1d8` individually; hook the five
   callees and assert the branch.
