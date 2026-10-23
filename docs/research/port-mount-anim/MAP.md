# Mount summon / riding / animation-graph port map (ArcheAge 1.2 client)

Status: research map for a line-by-line Rust port and for differential tests that call the
original functions inside the running 1.2 client. Branch `port/mount-anim-map`.

Scope: the local summon -> mount -> ride -> dismount -> dismiss path of a mate (horse), the
rider/mount animation-graph coupling, and the cryaction/cryanimation animation-graph runtime that
turns graph inputs into state changes and animation commands. Movement-request construction
(`FUN_39404d50`, turn/auto-run, camera modes 1-6) is mapped by the sibling document
`docs/research/port-movement/MAP.md` (other branch); this map only covers what the mount and
animation layer add on top of it.

All addresses are virtual addresses at the **preferred image base**:

| module | base | decompile |
|---|---|---|
| `x2game.dll` | `0x38ff0000` | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c`, `.funcs.csv`, `.calls.tsv`, `.strings-xrefs.tsv` |
| `cryaction.dll` | `0x304f0000` | `D:/aa-re/client12/engine/out/cryaction.dll.c`, `.funcs.csv` |
| `cryanimation.dll` | `0x31500000` | `D:/aa-re/client12/engine/out/cryanimation.dll.c`, `.funcs.csv` |

At runtime the module may be rebased; call `module_name+rva` (rva = VA - preferred base), which is
what `aaemu-clientdriver call/read` expects. Sizes are the `funcs.csv` `size` field unless stated.

**Licensing.** Everything here was derived from the shipped binaries, their decompiles, their
`.data`, the shipped Lua bytecode and the shipped game data. The published CRYENGINE source was
not used, quoted or consulted. Names in quotes or `CClass` form come from the binaries' own
profiler/log/assert strings; names marked *(inferred)* describe observed behaviour only. This is
the same rule as `C:/Users/stsiakal/AAEmu/rust/docs/physics/CRYPHYSICS-MAP.md` ("Licensing").

Tags: **[R]** read directly in the decompile/bytes; **[I]** inferred from data or from an
unread branch; **[?]** unresolved.

---

## 1. Per-area summary

### A. Summon / dismiss

**Item -> cast.** The Brown Lilyut Horse is item `8159` -> `item_summon_mates` -> npc `5431`,
`use_skill_id = 10602` (`skills`: casting time `0`, `fire_anim_id 393`, `fx_group_id 929`;
`fx_groups 929` -> `fx_items 1451` = `x2_sounds.particles.whistle`, a sound/particle whistle, no
spawn visual). The item use path is the generic item/skill cast; the summon is server-authoritative.

**Packets -> model.** `SCMateSpawned` (0x125) -> `FUN_391db180` -> `FUN_3933fd00` creates the
0xc0-byte Mate record (vtable `PTR_FUN_399cfb18`). The visual is **not** created here: the mate's
unit arrives through the ordinary `SCUnitState` path. `FUN_391d4c30` (`OnUnitState`) ->
`FUN_391b90a0` -> `FUN_3936b580` allocates a 0x6008 `ClientUnit`, constructs it and calls
`FUN_3936b000` (`ClientUnit::InitFromNetUnit`), which calls `FUN_390b95e0` (`X2::Unit::InitModel`)
with the model id and the **server-provided** transform. For an own mate, the same unit-state
dispatch calls `FUN_3933f0e0` -> `FUN_3933e3a0` (binds the unit id into the Mate record and fires
event `0x110` `SPAWN_PET`).

**Spawn timing / fade / pose.** There is no mate-specific spawn animation, fade or appear state.
The actor is created synchronously from the net unit state; the visible model arrives when the
model stream completes (`FUN_3935fcb0`, `ClientUnit::OnModelStreamLoadingComplete`), which swaps
the model with `FUN_390ba9c0` (`Unit::ChangeModel`) if the loaded id differs. The horse graph has no
`_spawn_effect` state and the generic spawn-effect scanner (`FUN_39109120`) is not called on this
path. Initial pose is the graph's default idle (`Main Hub` -> `Idle Start` ->
`fist_ba_relaxed_idle_anim`), because the attach/summon path writes no graph input. Placement is
the received net-unit transform; the client adds no offset.

**Dismiss.** `DismissPet` (`FUN_39362070`): mounted on the same mate -> `CSUnMountMate` reason 10;
otherwise after the 2 s vehicle cooldown it starts the `UnMountHorseController` with reason 10
(`FUN_390ca560` -> `FUN_390a6110`), sets the Mate record flag `+0xbc`, fires `DISMISS_PET`
(`0x111`); when the controller ends the client removes the mate (`CSRemoveMate` 0xa4 writer
`FUN_3933f040`). Auto-dismiss when too far/stuck is `FUN_3933f1b0` (5 s too-far timer, 60 s stuck
timer; thresholds squared 3600 / 49 in `.data` `0x39ac7404` / `0x39ac7400`).

### B. Mount / dismount

**Entry.** Lua `X2Mate:MountMate()` = `FUN_390697f0`; the pet bar command button 2
(`PET_CMD_MOUNT`, 52x52 icon `ui/icon/pet_command/pet_mount.dds` from `pet_view.alb`, hotkey name
`PetCommandButton2`, unbound by default) uses the same path. `FUN_393642f0(target)` resolves the
unit, checks eligibility (`FUN_3935aa10`: unit kind 5, not dead/attached, local player not
mounted, not stunned-like, distance squared <= 625 = 25 m, 2000 ms cooldown, casting/pending gate)
and starts the mount controller for the own/passenger seat (attach point 1 or 2).

**Approach.** `FUN_390a5f00` constructs a 0x38-byte `MountHorse Controller` (vtable
`PTR_FUN_399a30b8`, type 8, 10 s lifetime). `FUN_390a2c90` computes the side approach point from
the mate orientation and the entry side; `FUN_390aaf30` each frame cancels on movement input,
calls `FUN_390aa730` (auto-walk request: arrive 0.8 m, half-speed inside 1.5 m, z -1.5..2.5 m) and
`FUN_3909b070`, which sends `CSMountMate` 0xa7 `{tl_id u16, attach point u8, reason u8}` only when
within 3 m (distance squared < 9.0, `.data 0x39ac3878`) and then waits 2 s
(`0x3999cffc`). The reason byte is `FUN_3909ae90`: dot of normalized(mate - player) with the
**mate's** forward; >= 0.96 back (3), < -0.96 front (4), otherwise cross >= 0 -> 2 else 1.
Occupancy of the mount's seats is queried with `FUN_390c9f40` (seat keys 1/2). The exact
occupancy override is [I]: back/front use seat 2 occupancy, the side band uses seat 1.

**Attach.** `SCUnitAttached` 0x76 -> `FUN_391d49d0` -> `FUN_39362770` (retries via
`FUN_39364d40` until the parent unit exists) -> `Unit::AttachTo FUN_390b3f10` (writes child
`+0x3af0` parent id, `+0x3af4` seat) -> `FUN_390e16b0` -> `FUN_390eb520` (`On Attaching`):
`FUN_390e96a0` binds the rider actor to the mount skeleton/seat (attach point 1/2, bones
`bone_spine_driver` / `bone_spine_passenger`, from `model_attach_point_strings`), then
`FUN_39153b60` sets graph key `Vehicle` = pose name (`FUN_39571f10`: 1 horse, 2 lion, 3 bear,
4 wildboar, 5 submarine, 6 smallboat, 7 coupleduckship, 8 speedcar, 9 elephant, 10 pangolin) and
int `VehicleSeat` = reason 2/3/4, else 1. The rider graph enters `Horse Start`, whose four
`Horse_Enter_*` states carry the `vehicle_transition` template `seat=` 1/2/3/4 (l/r/b/f), matched
against `VehicleSeat`; the clips are `fist_ac_horse_{l,r,b,f}_geton` (1.00 s); see
`player_full_body.xml:1509-1521`, `re-mounts-full.md` §4. The mount itself is unchanged.

**Dismount.** `CSUnMountMate` 0xa8 (Lua `FUN_390697b0` -> `FUN_39361e80(9)`; refused by buffs
0x2b05/0x2b15 and the 2 s cooldown) -> `SCUnitDetached` -> `FUN_391d4a70` -> `FUN_39355bd0` ->
`Unit::DetachFrom FUN_390b4000`. `On Detaching FUN_390cea80` clears the stirrup bindings, then
starts `UnMountHorseController` (`FUN_390a6110`, vtable `PTR_FUN_399a3108`, type 9; 500 ms for
reason 9, 1000 ms for 10). Placement: `FUN_3909b1a0` tries 1.5 m to the mate's left then right
(side vector from the mate's basis, z x 1.0; `.data 0x39ac3888/0x39ac388c`), collision-checked;
if both fail it falls back to the saddle transform. `FUN_3909b4a0` (reasons 9/10/b/d) applies that
placement immediately; `FUN_390a2e90` handles reasons 3/8/c with a ground/water offset and states
5/6 for the rest. The rider graph gets `Action=jump_horse` -> `jump_horse_start`
(`fist_ac_horse_l_getoff_start`, 0.67 s) -> `_loop` -> `_end` (0.83 s), always **left**, played as
JumpMain/JumpLoop/JumpEnd (`player_full_body.xml:11733-11747`).

**Camera.** No camera code exists in the summon, eligibility, controller, attach, detach or
placement functions. The camera manager remains in the unit third-person mode
(`FUN_3916aab0` / `FUN_39169860`); because the local player is bound to the seat bone the camera
pivot follows the seat. The `models(450)` `camera_distance 15 / 12` values are not consumed by any
mounted branch found [R for the values, ? for use]. There is no blend or mode switch during the
get-on/get-off clips; the clip root motion is skeletal only (`re-mounts-full.md` §2).

**F/interaction prompt (the answer).** F is `do_interaction_1`
(`game/default_binding.g:29`, extracted copy `.../pak/game/default_binding.g`). The on-screen
prompt is the **dynamic action bar** (`x2ui/dynamic_action_bar.alb` + `_view.alb`): 48x48 buttons
in the lower middle at `2.75/4` screen height, key label from
`ConvertAbbreviatedBindingText(X2Hotkey:GetBinding("do_interaction_"..index))` (so F/G/H/J), icon
from `X2Interaction:GetInteractionSkillIconPath(skill)` = `Game\ui\icon\` + the skill icon asset
(`FUN_394e6c10`), with `_o/_c/_d.dds` variants derived by the script. The dynamic bar itself has
no own-mount entry. The only native bar entry that is specifically a mount command is `command_unmount`
(icon `ui/icon/icon_skill_vehicle03.dds`), and it belongs to the **mode action bar**, not the
F bar: `FUN_392045f0` (263 B) resets the 15 mode slots (`+0x14c`, stride 0x54), adds the entry
only when the ridden mount is **not** one's own mate (`FUN_3933b1b0` false), and fires
`MODE_ACTIONS_UPDATE` `0x5f`; `mode_action.alb` consumes it with the `mode_action_bar_button`
bindings (R/T/Y/U by default). While riding one's own mate no such entry is added (owner capture
`re-mounts.md` §10). **No native F entry for mounting an own mate was found**; the mount command
is the pet-bar `PetCommandButton2` (`pet.alb` disassembly), whose icon is
`ui/icon/pet_command/pet_mount.dds` and whose default binding is empty. If F mounts a mate in the
owner's client, that is a script/behaviour not present in the decompiled native path.

### C. Rider and mount reactions while riding

`FUN_390dcc80` runs from `X2::ActorUnitModel::Update` (`FUN_390dd080`) each frame. For a rider
attached to a unit parent it reads the mount graph's `PseudoSpeed`, `Stance`,
`RequestedTurnSpeedLZ` (`GetInputId` at graph vtable `+0x60`, `GetInputFloat` at `+0x5c`), forces
`PseudoSpeed = 0` when `Stance == "swim"`, and writes both floats into the rider graph. For a
non-unit parent it computes `Desired8Dir` (keys `s,f,fl,fr,b,bl,br`) from the parent's velocity
(thresholds 0.1 / 0.5, `.data 0x3999dda8/0x3999ccf0/0x399a2f24`; key table `0x39ac3a2c`).

Mount graph inputs come from the shared `CAnimatedCharacter::SetAnimGraphSpeedInputs`
`FUN_30665830` (profile 0, and profile 9 for the second graph input bank) and the mount's own
movement request. Jump is the player's input: `FUN_39153ff0` sets `Action=jumpMP01` (or
`jumpMP02` with the walk flag 0x1000), or `Signal=em_jump_in_place`; the rider graph selects
`Horse_Driver_Jump_Start/Land` from its own `Action`, while the mount selects its own jump states.
Hit reactions are graph keys/signals: the skill's `match_animation` (e.g.
`all_re_combat_hit`) is written as `Action`, the graph transitions `Skill Start ->
all_re_combat_hit -> all_re_combat_hit_anim -> Skill End` (`horse_full_body.xml:521-528,1559,1653`).
Death: `FUN_390c47f0` clears controllers and forces `death_animation` /
`death_animation_last_frame` (or swim variants) on the actor's graph; the server detaches a rider
on death, the client does not locally detach.
Idle randoms are graph-side timers (`SetInput time=5 -> Signal idle_rand`, additive idle-rand
animations in `horse_full_body.xml:585-592`); the run-time for these is the template/signal node in
the graph runtime (section D). Turn is `RequestedTurnSpeedLZ` from
`SetAnimGraphSpeedInputs`/the mount copy; `FIST_MO_[STANCE]_TURN` is the TSTP state.

### D. Animation-graph runtime (cryaction + cryanimation)

**Load.** `CAnimationGraph::Init FUN_3061f210` calls, in order: graph XML/`.ag` parse
`FUN_3061bd50` (13,453 B), `PreprocessTemplates FUN_3060f730`, `PreprocessLinksAndStates FUN_30655570`,
state-order resolution `FUN_3060aed0`, `LoadInputs FUN_30611d30`,
`LoadStates FUN_30616cc0`, `LoadTransitions FUN_3061a5f0`, `CachePass FUN_305f9cc0`,
`LoadHubStates FUN_30609740`, transition matrix `FUN_3060b800`, finalize `FUN_3060c790`.
`PreprocessParameterization` is present as a profiler string (`0x30741f2c`) but its body is not in
the exported decompile [?].

**Inputs.** Each input is a `CIntegerInputValue` / `CFloatInputValue` / `CKeyedInputValue`.
`SetInput` (`FUN_30632810`) stores a raw value (`+0xb0 + id*4`), an encoded u16
(`+0x2e + bank*0x40 + id*2`), a change id (`+0x140 + id*4`) and a changed bit (`+0x138`).
Floats/ints are encoded to a byte by the input object (`EncodeInput`, `FUN_30604b10` int,
`FUN_30605180` float, `FUN_30611470` keys); keys map the key name to an index (0xffff = none).

**Guards and state selection.** A state stores a guard count (`+0xb`) and u16 guards (`+0x10`).
`CAnimationGraph::SStateInfo::EvaluateGuards FUN_305f7a80` unpacks a guard as
`input = g & 0x1f`, `value = (g>>5) & 0xff`, `op = (g>>13) & 3` and returns false iff
`op 0: value < current`, `op 1: current < value`, `op 2: current == value`; i.e. pass means
`current <= value`, `current >= value`, `current != value` for 0/1/2 [R]. XML `min`/`max`/`value`
map onto these ops; the exact loader packing and the meaning of op 2 are [?].

`CStateIndex::Query FUN_3065e050` intersects precomputed input->state tables and returns the
matching state set (or all states on no results). `PerformQuery FUN_306359e0` (`BasicQuery`
`FUN_30630c20`) picks the next state: it evaluates the current state's outgoing transition list
(graph `+0x68`, 0xc-byte entries `{from u16, to u16, weight u16}`), accepts entries whose guards
pass, picks weighted-random by `time % total` for back/front/straight ambiguity, falls back to the
graph's default state (`+0x180`), supports forced/queued states (`+0x28`, `+0x70c` ring of 24),
and uses `PathFindBetweenStates` for ephemeral paths.

**State machine update.** `CAnimationGraphState::Update FUN_30639000` (1,718 B) calls one of
`Update_InitialState FUN_30636640` (first state), `Update_SteadyState FUN_306357f0`,
`Update_TransitionState FUN_30634d50`, `Update_EphemeralState FUN_30634b70`, via the function
pointer at `+0x708`. Transitions enter/leave states through `FUN_306352a0` (`EnterState`, collects
the state's node factories, sends enter/leave events) and `FUN_30635590`
(`BeginNextTransition`). `FUN_30636480` forces a state from the query
(`ForceTeleportToQueriedState`). Per-state node factories are built by `FUN_30631520` from the
graph's factory index/pointer arrays (`+0x184`, `+0x194`) and the state's range (`+0xc` / byte
`+0xa`). The result is a list of animation-desc/factory objects (`+0x5a8`), not clips played
directly by cryaction.

**Clip / LMG blending (cryanimation).** `CAnimationSet::GetAnimationDesc FUN_31558f80` resolves
the animation/locomotion asset and, for an LMG (asset type 2), fills the 0x58-byte capabilities
struct with `GetLMGCapabilities FUN_315714b0`; the per-animation layer data is
`FUN_31558550` (1,187 B). `FUN_31556e90` converts an animation desc into the character command
buffer (0x800 cursor, 0x804 count): resolves the animation set (`FUN_3160cc50`), validates the
weights (sum = 1, same-animation rows summed, error strings at `0x31677e78`/`0x316780ec`), and
emits commands 0/2/3/7/9/10 (set layer, play clip with weight/speed, set animation set, finalize).
The blend-space type is a FOURCC in the LMG asset (`+0x40`): `XL14`, `S8_1`, `M2I8`, `TSTP`,
`ISTP`, `S8SW`, `XL15`, `XLV5`, `MI01`, `MI02`, ...; `FUN_31568fc0` maps the FOURCC through a
59-entry table at `cryanimation+0x1b5890` to the switch case 0..0x39 in `GetLMGCapabilities`
(per-case weight functions `FUN_3156bd00..31571080`). The exact weight formula for each type is
the main remaining unknown (section 6).

**Action / Signal / timers.** Key inputs can be driven by state templates: `SetInput time=N ->
Signal name` (idle random), `Additive` animations, `Event onExit=...`. The state machine exposes
`Signal`/`Action` as keyed inputs; `Action` selects one-shot states (e.g. `jumpMP01`,
`all_re_combat_hit`, `ac_cooking`), `Signal` selects event states (`idle_rand`, `fall`,
`em_jump_in_place`). Signals are queued as state events (`+0x620`, `+0x640`) and delivered by
`Update_TransitionState`/`EnterState`.

**Upper-body layers.** `CAnimatedCharacter::Init` (body at `0x30662b40`, no `funcs.csv` entry,
1,072 B disassembly) reads the model's animation-layer properties, 8 layers (`AnimationLayer1..8`,
`0x30744540`), and for each loads the graph named by the layer property. Defaults are
`player_full_body` (`0x30746a9c`) and `player_upper_body` (`0x30746a88`); a missing property logs
`No AnimationGraph layer property defined for entity` (`0x307469f0`). Each graph is registered
with the character's `CAnimationGraphStates` wrapper (`+0x24`) via `FUN_3063cf10`; the wrapper's
`Update FUN_3063b090` ticks all layers. cryanimation blends the resulting per-layer commands in
its command buffer (13 layer slots in `FUN_31558550`).

### E. On-foot equivalents that share the machinery

The player graph uses exactly the same `CAnimationGraphState`, `CStateIndex`, input encoding,
transition/guard evaluation, `CAnimatedCharacter::SetAnimGraphSpeedInputs FUN_30665830`, jump
input `FUN_39153ff0`, death force `FUN_390c47f0`, and LMG command builder. On foot the differences
are data only: `player_full_body.xml` states (`Run`, `Walk`, `Idle`, `Start/Stop`, `Turn`), the
same `PseudoSpeed`/`RequestedTurnSpeedLZ`/`Desired8Dir`/`Action`/`Signal` inputs, and the same
get-on/off states while `Vehicle` is set. This is why the port order below starts with the graph
runtime and speed inputs, not with mount code.

---

## 2. Size and function inventory

Counted from the three decompiles; functions that Ghidra split or did not export are listed with
their observed body size and marked `(no entry)`.

| area | function rows | bytes | files |
|---|---:|---:|---|
| A summon/dismiss | 22 | 12,718 | `A-summon-dismiss.md` |
| B mount/dismount + prompt | 43 | 23,872 | `B-mount-dismount.md` |
| C rider/mount reactions | 8 | 13,737 | `C-riding-reactions.md` |
| D graph runtime | 51 | 75,857 | `D-graph-runtime.md` |
| **rows** | **124** | **126,184** | |
| **unique** | **123** | **116,538** | `SetAnimGraphSpeedInputs 30665830` is shared by C and D |

The table includes the no-entry/split bodies with their observed sizes:
`CAnimatedCharacter::Init` `0x30662b40` (1,072 B, D), the `PseudoSpeed` writer at `0x390a51f0`
(464 B, C), and the `OnUnitState` entry `0x391b90a0` (8 B, A; its split body is not separately
countable). The per-blend-type LMG weight functions are not counted (only the dispatcher,
fourcc map and validators); they are the main remaining D work.

The per-area tables give, for every function: address, size, callers, callees, object offsets,
globals/cvars/tables, pure vs stateful, and the in-process call recipe.

---

## 3. Differential test harness (existing, use it)

The `driver-call` build of `aaemu-clientdriver` exposes the needed primitive operations
(`D:/aaemu-target/driver-call/live/aaemu-clientdriver.exe` usage text; the same strings are in
`D:/aaemu-target/driver-call/aadriver/aadriver.dll`):

```text
call <module+rva> <cdecl|stdcall|thiscall|fastcall> [this=addr] [dump=addr:len]...
     [unsafe] [args...]
     args: 123 / 0x7b (u32), f:1.5 (f32), p:addr (scratch pointer)
     prints EAX, ST0 (raw x87) and each dumped buffer
read <module+rva|addr> <len>        alloc <len>        free <addr>
write <target> <hex> [--unsafe]     peek-global ...   mod <module>
```

Rules:
- The call runs on the client main thread and is SEH-wrapped ("call raised exception ...").
- Allocate argument/out buffers with `alloc`, fill with `write` (only tracked scratch memory by
  default), pass them with `p:<addr>`, read results with `dump=addr:len` or `read`.
- Use `mod <module>` to get the actual base, then pass `module+rva` (the tool relocates by the
  actual base).
- x86 conventions: most engine methods are `__thiscall` (ECX = this, args on stack, callee pops);
  free helpers are `__cdecl`/`__stdcall`; x2game's small virtuals are `__thiscall`. The tool
  supports all four and `this=`.
- A test must restore anything it mutates. Prefer pure functions and scratch copies; for stateful
  functions (graph state, actor) clone the object into scratch memory or use the live local
  player and accept read-only observations.
- Do not inject/upgrade a driver into a client owned by another session. The current live client
  may have an older driver (it rejects `read`); the `driver-call` build is the one to use for a
  dedicated oracle run.

Typical calls:

```text
# pure-ish: EvaluateGuards(stateInfo*, u16 encodedInputs[32]) -> 0/1
alloc 32                      -> A
write A <64 hex chars>        # encoded inputs
alloc 0x30                    -> G
write G <guard count + guards>
call cryaction.dll+0x5f7a80 thiscall this=G p:A
# returns EAX = 1 (pass) / 0 (fail)

# float input encode: CFloatInputValue::EncodeInput
call cryaction.dll+0x605180 thiscall this=<input object> f:0.4

# graph state input + query, against a live actor
# local player: game client global 0x39b02294 + 0x10c; actor at +0x3be8;
# graph state at actor+0x24 (CAnimationGraphStates) and actor+0x1ec (state interface)
call x2game.dll+0xdcc80 cdecl # no args? it is thiscall: this=rider actor
```

For each per-area table the "test" column gives the exact convention, `this` object and
argument/out buffer layout. Tests that need a live actor should use the local player and the
mount from a fixture character; the driver runs Lua on the same main thread, so no cross-thread
state race is introduced.

---

## 4. Proposed port order (testable leaves first)

1. **Harness + pure leaves.** Wire `call/read/alloc/write/dump`; diff `EvaluateGuards`,
   `CIntegerInputValue::EncodeInput`, `CFloatInputValue::EncodeInput`, key encoding, the
   FOURCC->case map `FUN_31568fc0`, entry-side `FUN_3909ae90`, approach-side `FUN_390a2c90`,
   side placement `FUN_3909b1a0` (pure geometry). These need no actor; high confidence.
2. **Graph load + static dump.** Implement the graph XML/`.ag` loader (inputs, states, guards,
   transitions, templates, hubs) and dump the original graph's tables through the harness
   (`read` on the live `CAnimationGraph` object, or a debug dump via a temporary call). Compare
   state ids, guard arrays, transition lists, default state and factory lists. This validates the
   data layer without animation.
3. **Graph state machine.** `SetInput` (`FUN_30632810`), `PerformQuery`/`BasicQuery`,
   initial/steady/transition/ephemeral updates, queue/forced state, events. Test with a synthetic
   graph (deterministic transitions) and then with `horse_full_body`/`player_full_body`.
4. **Cryanimation desc + command buffer.** `GetAnimationDesc`, LMG caps, per-layer data,
   `FUN_31556e90` command bytes. Diff the command buffer for fixed descs; then fill in the
   per-blend-space weight functions (the main unknown).
5. **CAnimatedCharacter speed inputs + rider copy.** `FUN_30665830` (profiles 0/9),
   `FUN_390dcc80`, `FUN_390dd080`; compare graph input values frame by frame for walking, turning,
   jumping, swimming, and riding.
6. **Mount flow.** Controllers (`FUN_390a5f00/390a6110`), approach/send, attach/detach, seat
   binding, side clips, camera observation. Test with packet capture plus actor transforms; the
   controller functions are stateful but their inputs/outputs are explicit.
7. **Summon / dismiss / UI.** Packet handlers, ClientUnit creation, streaming, `SPAWN_PET`/
   `DISMISS_PET`/`MOUNT_PET`/`UNMOUNT_PET` events, pet-bar mount button and the mode-bar
   `command_unmount`. This is the integration layer; keep it after the graph and controllers.
8. **On-foot shared + camera.** Regression-test the on-foot states against the same graph runtime;
   confirm the camera has no mounted branch with a live capture.

Each step has a hard gate: byte-identical state/desc/command output for the ported functions on
the captured inputs, or an accepted documented difference in the unknowns list.

---

## 5. What is proven vs inferred

Proven (decompile/bytes/data):
- packet -> Mate record, unit state -> ClientUnit -> InitModel, spawn event `0x110`;
- no mate fade/spawn state; horse graph has no `_spawn_effect`; spell FX is a whistle;
- controller vtables/types/lifetimes, CS packet opcodes/fields, approach tolerances, send range,
  reason formula, attach bone/seat inputs, side clips, get-off always left;
- F key = `do_interaction_1`, dynamic-bar widget/icon path/key label, pet-bar mount icon;
- graph load order, guard packing/evaluation, state/transition structures, CStateIndex query,
  command buffer writer, FOURCC table and caps dispatch;
- `SetAnimGraphSpeedInputs` inputs and offsets, rider copy semantics, swim zeroing, Desired8Dir;
- jump Action values, death force states, event ids.

Inferred / unresolved: see section 6.

---

## 6. Unknowns (ranked)

1. **LMG weight formulas per blend type** — the dispatcher `FUN_315714b0` and the per-type
   functions are identified, but the exact weights for `XL14` (horse/rider run), `S8_1` (walk),
   `M2I8` (idle<->run), `TSTP` (turn), `ISTP` (idle steps), `S8SW` (swim) are not decoded. This is
   the largest remaining line-by-line gap for D.
2. **`CAnimationGraph::PreprocessParameterization`** — profiler string `0x30741f2c`, no exported
   body; how `[stance]`/`[dir]`/`[num]` state variants are expanded and encoded is [?].
3. **Guard op 2 / XML min/max packing** — runtime semantics are read; the loader's exact choice of
   op 0/1/2 for `min`, `max`, `value`, and parameterized equality is [?] (likely min=max equality
   for keys).
4. **PseudoSpeed producer** — `re-mounts.md` cites `FUN_3906a7f0`; the address falls in a split
   body, and the visible writer at `0x390a51f0` only sets 0/1. The 0 / 0.4 / 1.0 / 1.5 values
   used by the graph need a live input trace or a completed split-body read.
5. **Seat occupancy** in `FUN_3909ae90` — `FUN_390c9f40` returns a seat-list value; the exact
   "occupied" polarity and the override in the side band are [I].
6. **Dismiss -> `CSRemoveMate` continuation** — the writer is `FUN_3933f040`; its callers are
   `Mate::Update` and `FUN_3936f540`, but the exact site that fires after the reason-10 controller
   end is [?]. `re-mounts.md` §9 states the observed sequence.
7. **Model streaming state machine** — `+0x3c54/+0x3c58/+0x3c40` and the retry callback are read,
   but the exact "appears now vs waits" condition for a not-yet-streamed model is [?].
8. **Upper-body layer policy** — the 8 layer properties and `player_upper_body` load are read;
   which states go to the upper-body graph and how the runtime masks lower-body bones is in
   cryanimation's command/layer code (not fully read).
9. **Camera during mount** — no mounted branch found; the `models(450)` distances and `camera`
   bone are not consumed by any located code [?]. Needs a live capture.
10. **Hit reaction source while mounted** — the graph side is proven; whether the rider gets the
    `Action` from its own cast/skill state or also from the mount is [I] (the player's own input
    state is the likely source).

---

## 7. Files

- [`A-summon-dismiss.md`](A-summon-dismiss.md)
- [`B-mount-dismount.md`](B-mount-dismount.md)
- [`C-riding-reactions.md`](C-riding-reactions.md)
- [`D-graph-runtime.md`](D-graph-runtime.md)
- [`E-onfoot-shared.md`](E-onfoot-shared.md)

Sibling: `docs/research/port-movement/MAP.md` (movement request, turn, camera modes; not duplicated
here). Prior notes reused with their own citations: `docs/research/re-mounts.md`,
`re-mounts-full.md`, `re-movement.md`, `re-constants.md`.
