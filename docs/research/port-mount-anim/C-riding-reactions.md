# C. Rider and mount reactions while riding

Scope: how the rider's animation graph inputs are produced from the mount, idle randoms, turn,
jump, hit reactions, death while mounted, and the mount's own graph inputs. x2game VAs at base
`0x38ff0000`, cryaction at `0x304f0000`; sizes from `funcs.csv`; constants from
`D:/aa-re/client12/x2game_unpacked_data.dll` (VA - base).

Relevant data: the horse graph `horse_full_body.xml` (151 states; `PseudoSpeed` 0..30,
`RequestedTurnSpeedLZ` -50..50, `Stance`, `Action`, `Signal`, `RequestedMoveDir4LH`), the rider
graph `player_full_body.xml`, `global_horse.cal` / `global_player.cal`, and the `models(450)` /
`actor_models(206)` rows (`re-mounts-full.md` §1-§5).

---

## 1. The rider copy `FUN_390dcc80` (1,011 B)

Called every frame from `X2::ActorUnitModel::Update FUN_390dd080` (632 B). `FUN_390dd080`'s call
order (verified in the decompile) is: `FUN_390c3d00`, `FUN_390c4da0`, `FUN_390cd0f0`,
`FUN_390c5230`, `FUN_390cd890` (stirrup IK), `FUN_390dcc80` (rider copy), then the optional
distance/fade helper using `DAT_39ae6c64/68`.

`FUN_390dcc80(this = rider actor)`:

```
parent = this.vt[0x34]()                    # attach-to getter
if !parent: return
if parent.vt[0x04]() == 1:                  # parent is a unit (mate)
    speed = 0; turn = 0
    g = parent.vt[0x1ec]()                  # parent graph state interface
    if g:
        speed = g.vt[0x5c](g.vt[0x60]("PseudoSpeed"))
        stance = g.vt[0x24](g.vt[0x60]("Stance"))     # string
        if _stricmp(stance, "swim") == 0: speed = 0
        turn = g.vt[0x5c](g.vt[0x60]("RequestedTurnSpeedLZ"))
    r = this.vt[0x1ec]()
    if r:
        r.vt[0x0c](id("PseudoSpeed"), speed, 0)
        r.vt[0x0c](id("RequestedTurnSpeedLZ"), turn, 0)
        FUN_390a8dc0(); FUN_390a5d00();          # movement-request reset/submit
        this.vt[0x3c](request with speed=speed)  # keep the rider's animation character in sync
        return
else if this[0x4b] == 1:                    # non-unit parent (vehicle/slave) / passenger?
    # parent movement vector -> Desired8Dir key
    ...
```

The graph interface slots are the ones x2game uses throughout:
`+0x60` `GetInputId(name)`, `+0x5c` `GetInputFloat(id)`, `+0x24` `GetInputString(id, out)`,
`+0x04` `SetInput(key, value)`, `+0x08` `SetInput(int)`, `+0x0c` `SetInput(float)`.
This is exactly the copy documented in `re-mounts.md` §C and `re-mounts-full.md` §4.

For the non-unit parent branch:

- reads the parent's transform (`vt[0x228]`), local forward/velocity (`vt[0xa4]`, `vt[0xa8]`),
- rotates the velocity into the parent's basis,
- quantizes it to `Desired8Dir` with thresholds `0.5` (`0x3999dda8 = -0.5`),
  `0.1` (`0x3999ccf0 = 0.1`, `0x399a2f24 = -0.1`):
  forward `f`, forward-left `fl`, forward-right `fr`, back `b`, back-left `bl`, back-right `br`,
  still `s`; the key table is at `0x39ac3a2c` (string pointers `s,f,fl,fr,b,bl,br`).
- `r.vt[0x04](id("Desired8Dir"), key, 0)`.

`this[0x4b]` is the attachment point/type used by `FUN_390e96a0` (`param_2[0x4b]`); the branch is
for a non-unit parent, so the rider derives direction instead of copying speeds. [R for the code,
I for the "passenger/vehicle" naming]

The rider graph's `Horse_Driver*` states then select from `PseudoSpeed`; the seat key
`VehicleSeat` and `Vehicle` were set at attach time (B). The rider has no independent movement
input while mounted.

---

## 2. The mount's own graph inputs

The mount is an ordinary actor and uses the same producers as the player:

- `CAnimatedCharacter::SetAnimGraphSpeedInputs FUN_30665830` (9,646 B; caller
  `CAnimatedCharacter::Update` / the animation update) computes the local movement vector and
  writes the graph input ids cached in the character:
  - `+0x6d6 + profile*2` key: movement direction (`PseudoSpeed`/move-dir family),
  - `+0x6d0`, `+0x6d2`, `+0x6d4`, `+0x6d8`, `+0x6da`, `+0x6dc`, `+0x6de` floats,
  - `+0x6e0 + profile*2` float: `RequestedTurnSpeedLZ`, computed from the quaternion delta / dt
    and converted rad->deg (constant `DAT_3072458c`).
  `profile` is 0 or 9 (`param_5`); the two profiles have separate input-id banks
  (`+0x659/+0x65a` and `+0x6fc/+0x6fd` flags), used by the two graph instances (full body and a
  second layer/graph). The input names for the `+0x6d0..` slots are not stored in this function
  [I]; the slot at `+0x6e0` is the turn-rate input identified by `re-mounts.md` §C.
- The actual `PseudoSpeed` producer (0 / 0.4 / 1.0 / 1.5) is still unresolved at the address
  cited in `re-mounts.md` (`FUN_3906a7f0` falls in a split body). A visible split writer at
  `0x390a51f0` (464 B, no `funcs.csv` entry) reads the actor's movement request
  (`vt[0xa0]`), rotates it by the actor basis (`this+0xa4..+0xac`), tests whether the vector is
  nonzero and writes `PseudoSpeed = 0.0` or `1.0`; the 0.4/1.5 bands are produced elsewhere
  (likely the request builder). Treat this as [?].
- The mount graph gets `Action`/`Signal` from the same input system; `jumpMP01` is written by
  `FUN_39153ff0` (see §4), and skill `match_animation` values (e.g. `all_re_combat_hit`) are
  written by the skill/cast state.

The mount's `Stance` is set by the actor state (relaxed/combat/swim); the rider copy reads it and
zeroes the rider's PseudoSpeed on `swim` (`FUN_390dcc80`).

---

## 3. Idle randoms and turn-in-place

These are graph data + the generic graph runtime (D), not x2game logic:

- Horse idle state `fist_ba_[stance]_idle_anim` (`horse_full_body.xml:585`) has
  `SetInput time=5 -> Signal idle_rand` and additive `fist_ba_relaxed_idle_rand_1..7` at 10%/68%
  with random delay (`horse_full_body.xml:592`). The timer/signal is implemented by the state
  template `locomotion_idle_entity` / the graph node factory; the signal arrives as a keyed
  `Signal` input (`idle_rand`), and the state machine selects the one-shot rand states.
- Rider idle `Horse_DriverIdle` (`player_full_body.xml:1464`) uses
  `horse_ba_relaxed_idle` and `SetInput time=3 -> Signal horse_ba_relaxed_idle_rand`, followed
  by `horse_ba_relaxed_idle_rand_1..5` one-shots.
- Turn: `FIST_MO_[STANCE]_TURN` (horse `:267`) is selected when `Action=idle`,
  `PseudoSpeed 0..1.5`, `Stance`; the TSTP blend uses `fist_mo_relaxed_turn_r`,
  `fist_ba_relaxed_idle_stop`, `fist_mo_relaxed_turn_l` weighted by `RequestedTurnSpeedLZ`
  (produced by §2 / copied by §1). The horse run state also has lturn/rturn examples in the
  XL14 blend.
- Idle-to-run / run-to-idle: `FIST_MO_IDLETORUN` and `FIST_MO_NORMAL_RUNTOIDLE` states with
  `leave_at_stop` / `leave_actual_changed` / `leave_request_changed` flags; the runtime uses the
  `PseudoSpeed` bands 0.11-0.7 (walk), 0.8-3.0 (run), and the transition times 0.2-0.3 s.

The exact signal timer implementation is in the graph runtime (D §"Action/Signal/timers"); the
weight formulas are D's main unknown.

---

## 4. Jump

`FUN_39153ff0` (99 B, `this` = local actor, `param_2` = jump kind):

```
if param_2 == 0:
    graph.vt[0x04](Action, "em_jump_in_place", 0)
else:
    if (this+0x04 flags & 0x1000) != 0:            # walk toggle
        graph.vt[0x04](Action, "jumpMP02", 0)
    else:
        graph.vt[0x04](Action, "jumpMP01", 0)
```

The actor's `Action` input id is at `this+0x10`; the graph state interface is `this+0x0c`.
While mounted the player still owns the input, so the rider graph gets `Action=jumpMP01/02`
directly and selects `Horse_Driver_Jump_Start`/`_Land`
(`player_full_body.xml:1498-1502`); the mount gets its own jump action from its own input state
and plays `fist_mo_jump_*`. No extra packet is sent (`re-mounts.md` §9). The freefall
`Action=freefall` is set by the fall/physics state, not by this function.

The on-foot jump uses the same function; `game_jump_height 1.3` and gravity 20 (horse) / 25
(player) are physics data (`re-mounts-full.md` §5, `re-movement.md` §2.2).

---

## 5. Hit reactions

The graph side is data:

- `all_re_combat_hit` (horse `:521`) selects when `Signal=all_re_combat_hit`;
  `all_re_combat_hit_anim` (`:527`) plays the `all_re_combat_hit` animation with
  `IdleOneShotNoLookIk`; links `Skill Start -> all_re_combat_hit -> all_re_combat_hit_anim ->
  Skill End` (`:1559`, `:1653-1654`).
- The key comes from the skill data (`skills.match_animation`; for hit skills it is
  `all_re_combat_hit`). The generic skill-cast state writes the skill's match animation as
  `Action`; no mount-specific handler was found.
- The rider's hit reaction while mounted uses the same `Action` path; because the rider is
  attached, the server sends the damage/skill events for the pair; the client plays the rider's
  `all_re_combat_hit` state. [R for the graph and the generic input, I for the exact mounted
  skill dispatch]

---

## 6. Death while mounted

`FUN_390c47f0` (557 B, `this` = actor, `param_2` = last-frame flag):

- clears the actor's controller/state (`vt[0x108](0,0,0,0,0)`, `vt[0x44]`,
  `FUN_39113f80(0)`);
- if the actor is in water / has a swim height, chooses `swim_death_animation` /
  `swim_death_animation_last_frame`, else `death_animation` / `death_animation_last_frame`;
- forces the state on the graph (`graph.vt[0x9c]` returns the state object,
  `state.vt[0x50](name, 0)`), see D's `ForceTeleportToQueriedState` path;
- calls `this.vt[0x98](0)`.

The death state is a one-shot `death_one_shot` template (`horse_full_body.xml:545-564`,
`re-mounts-full.md` §3). The client does **not** locally detach a mounted rider on death; the
server sends `SCUnitDetached` (B §4), which runs the normal detach/unmount controller. The
mounted death is therefore: server detach -> `On Detaching` -> `UnMountHorseController` -> the
rider's death animation/placement. [R for the client functions, I for the exact server order]

---

## 7. Function table

| addr | size | callers | callees | offsets / globals | kind | test |
|---|---:|---|---|---|---|---|
| `390dcc80` | 1011 | `390dd080` | `_stricmp`, `390a8dc0`, `390a5d00`, `39695a70` | rider `+0x34/+0x1ec/+0x3c/+0x4b`; parent graph; `0x39ac3a2c` key table; `0x3999dda8=0.5`, `0x3999ccf0=0.1`, `0x399a2f24=-0.1` | stateful (per-frame) | call `fastcall` with ECX=rider actor on a live mount; read the rider graph inputs before/after |
| `390dd080` | 632 | actor update | `390c3d00`, `390c4da0`, `390cd0f0`, `390c5230`, `390cd890`, `390dcc80`, `39007da0` | `DAT_39ae6c64/68`, `param_1[0x7f]` | stateful | integration; call per frame and compare the call order |
| `30665830` | 9646 | animation update | `FUN_306fd3ba`, `FUN_306fd4*` | animated char `+0x24` graph, `+0x659/+0x65a` flags, `+0x6d0..+0x6e0` input ids, `+0x6fc/+0x6fd` flags | stateful | call `thiscall` this=character with scratch `param_2` (7 floats), `param_3` (4 floats), `f:dt`, `param_5=0`; read the graph inputs via `GetInputFloat` |
| `39153ff0` | 99 | `390ccab0` | — | actor `+0x10` Action id, `+0x0c` graph, `+0x04` flags | stateful | call with a live actor; read `Action` |
| `39132be0` | 1044 | `39134930`, `39135570` | `3916c1f0`, `3916c280`, `3916c240`, `3916c2e0`, `XlCompareString` | `Vehicle`, `Action` (jump/climb), `climbLadderUp`, ... | stateful | call with a live actor in various special moves; read the graph keys |
| `390c47f0` | 557 | death path | `39007da0`, `39113f80`, `39695a70`, `390c2fd0` | death/swim state names | stateful | call with a live actor; read the forced state |
| `390a51f0` (no entry) | 464 | split body | movement request `vt[0xa0]` | actor `+0xa4..+0xac`, graph `+0x0c`; PseudoSpeed id `0x399a30a8` | stateful | call the split entry at `0x390a51f0`; it uses ECX and one stack argument (`[ebx+8]`), both object pointers; identify which is the actor by reading `[arg+0xc]` before the call and dump PseudoSpeed |
| `3906a640` | 284 | Lua `IsPlayerMounting` | `3933f5e0`, `39360020`, `390b3750`, `390c9f40`, `39058bd0` | mount/seat query | stateful | call with a live actor; returns bool |

Constants: `0x3999c488 = 1.0`, `0x3999c47c = 0.5`, `0x3999c480 = FLT_EPS`,
`0x3999c5f0 = 0x80000000` (sign XOR), `0x3999d500 = 0x7fffffff` (abs mask),
`0x39ac3a2c` = direction key table (`s,f,fl,fr,b,bl,br`), `0x399a2d28 = 0.96`,
`0x399a2d24 = -0.96`.
