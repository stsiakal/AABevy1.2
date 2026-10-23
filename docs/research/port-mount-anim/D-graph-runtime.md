# D. Animation-graph runtime (cryaction + cryanimation)

Scope: how a graph XML is loaded and evaluated each frame: input values -> guard evaluation ->
state selection -> transitions -> animation desc -> clip/LMG blend -> character command buffer,
including signals/Action, upper-body layers and the compiled graph data. cryaction VA base
`0x304f0000`, cryanimation base `0x31500000`, x2game base `0x38ff0000`. Sizes from `funcs.csv`.
Strings quoted below are the binaries' own profiler/log strings.

The graph files used by the mount/rider: `game/animations/graphs/horse_full_body.xml` (140,337 B,
151 states) and `player_full_body.xml` (1,736,415 B), both also shipped as `.ag`. The client
loader `FUN_3061bd50` parses XML (`IXmlNode` attributes `States`, `State`, `SelectWhen`,
`Template`, `Parameter`, `Override`, `Mapping`, `Views`, `ExcludeFromGraph`), so the XML is the
runtime source; the `.ag` is the compiled side copy described in `re-mounts-full.md` §1.

---

## 1. Objects and layouts

### 1.1 `CAnimationGraph` (the immutable graph)

Observed from `LoadInputs/LoadStates/LoadTransitions/CachePass`, `Init`, `ConstructAnimationDesc`,
`PerformQuery` and `GetStateAnimationDesc`:

| offset | content |
|---:|---|
| `+0x00` | vtable/base |
| `+0x08` | graph name (CString*) |
| `+0x0c` | graph file/entity string |
| `+0x10/+0x14` | input object vector (pointers to `C*InputValue`; one per input) |
| `+0x30..+0x50` | timing/statistics |
| `+0x58/+0x5c` | state array start/end; each state is `0xa0` bytes |
| `+0x68/+0x6c` | transition array start/end; each entry `0xc` bytes `{u16 from, u16 to, u16 weight}` |
| `+0x180` | default state id (u16, `-1` = none) |
| `+0x182` | animation descs constructed flag |
| `+0x184` | state->factory index byte array |
| `+0x194` | factory pointer array |
| `+0x206` | state count/version (`0xfffe` initial) |
| `+0x21c/+0x220` | weighted random table (pairs `{count, state}`) |

### 1.2 `CAnimationGraphState` (one instance per character graph/layer)

Observed from `SetInput FUN_30632810`, `Update FUN_30639000`, the update variants,
`PerformQuery FUN_306359e0`, `EnterState FUN_306352a0`:

| offset | content |
|---:|---|
| `+0x00` | vtable |
| `+0x0c` | "remote/serialized" flag (`0` = local; disables logging and enables rollback) |
| `+0x10` | input id `ActualMoveDir4LH` (cached in the constructor at `0x30639f80`) |
| `+0x14` | input id `RequestedMoveDir4LH` |
| `+0x18` | input id `PseudoSpeed` |
| `+0x20` | `CAnimationGraph*` |
| `+0x24` | next state (u16) |
| `+0x26` | current state (u16, `-1` = none) |
| `+0x28` | queued/target state |
| `+0x2a` | pending transition target |
| `+0x2c` | force/teleport flags (u16) |
| `+0x2d` | current encoded-input bank (0/1) |
| `+0x2e` | encoded input values `[2][0x20]` u16 (bank stride `0x40` bytes) |
| `+0xb0` | raw input values `[0x20]` u32 (float bits, int, or key index) |
| `+0x138` | changed-input bitmask (u32) |
| `+0x13c` | change sequence counter |
| `+0x140` | change id per input |
| `+0x1c0` | change id per input for the second consumer |
| `+0x240/+0x244` | candidate state vector (u16) |
| `+0x250/+0x254`, `+0x25c`, `+0x260/+0x264` | time accumulators (u64) |
| `+0x26c` | event buffer passed to factory/event virtuals |
| `+0x274` | `ICharacterInstance` (animation system) |
| `+0x570` | paused flag |
| `+0x571..+0x57c` | state bits/values: transition/steady flags, snapped state data |
| `+0x5a8/+0x5ac` | animation factory object vector (state nodes) |
| `+0x5b8` | current `SAnimationDesc` |
| `+0x5e8/+0x5ec` | child graph states |
| `+0x620/+0x624` | queued events/list |
| `+0x63c..+0x63e` | state-change bookkeeping |
| `+0x640/+0x654` | event queue (`FUN_30631f80` vector) |
| `+0x670/+0x674` | state-change counter/last state |
| `+0x6a8/+0x6aa` | initial/sent state |
| `+0x708` | current update function pointer |
| `+0x70c/+0x70d/+0x70e[24]` | pending-state ring (count, index, 24 u16) |
| `+0x73e/+0x73f/+0x740` | transition/ephemeral flags |
| `+0x764` | query result struct |
| `+0x7c8` | serialized-state object (rollback/cutscene), `+0x7cd` dirty flag |

### 1.3 `CStateIndex` (query accelerator)

`CStateIndex::Query FUN_3065e050` (502 B) uses:

| offset | content |
|---:|---|
| `+0x90/+0x94` | 20-byte entries (state id + input pair set) |
| `+0xb0/+0xb4` | input-pair array (u16 pairs, `input id`, `value`) |
| `+0xc0` | input count (u16) |
| `+0xc2` | state count (u16) |
| `+0x200/+0xcc` | result entries (20 bytes: state id + count, values) |
| `+0x21c/+0x220` | weighted random table |

`FUN_3065ab70`/`FUN_3065b9a0` clear the result sets; `FUN_30657e50` binary-searches the sorted
input-pair array; `FUN_3065ba50` accumulates matches; `FUN_3065da00` walks the result table.
`CachePass FUN_305f9cc0` (640 B) marks each state: bit 1 in `state+6` = has outgoing
transitions, bit 2 = has a nonzero-weight transition, bit 0x40 = target unreachable, then builds
the weighted random table and the per-state transition counts.

### 1.4 Input value objects

`LoadInputs FUN_30611d30` creates one object per `<Input>` (type strings `KeyState`, `Float`,
`Int`, ...), stores default/min/max/mixinFilter, and `FUN_305f7c70` resolves the type. The
object's vtable is what `SetInput` uses:

- `+0x04` key setter (id, key string, change id)
- `+0x08` int setter
- `+0x0c` float setter
- `+0x10` int setter (second form)
- `+0x18` encode/format for debug
- `+0xc4` "is key" / interpolation flag

The graph-state public accessors used by x2game are `+0x60` `GetInputId(name)`, `+0x5c`
`GetInputFloat(id)`, `+0x24` `GetInputString(id, out)`, `+0x04/0x08/0x0c` `SetInput`.

`CIntegerInputValue::EncodeInput FUN_30604b10` (305 B) parses the string as an int and calls the
object's encode virtual (`+0x08`). `CFloatInputValue::EncodeInput FUN_30605180` (327 B) parses a
float and calls `+0x0c`. `CKeyedInputValue::EncodeInputs FUN_30611470` (345 B) binary-searches
the object's sorted key list (`+0x14` map, `+0x1c` end) and returns -1 (0xffff) for an unknown
key. The exact float quantization formula is in the per-object virtual (not exported) [I]: the
value is stored as a byte in the guard, so it is a min/max linear map to 0..255.

`SetInput FUN_30632810` (345 B) writes:

```
state+0x2e[bank][id] = encoded u16 (0xffff = none)
state+0xb0[id]       = raw value
state+0x140[id]      = change id
state+0x138         &= ~(1 << id)
if value changed: state+0x138 bit, and queued change objects are updated/removed
```

The manager wrappers `CAnimationGraphStates::SetInput[key] FUN_3063e560` (235 B),
`[float] FUN_3063e370` (241 B), `[int] FUN_3063e470` (235 B) and
`SetInputOptional[key] FUN_3063d6e0` (564 B) iterate all state objects of a character and call
the input object's setter with a change id from `FUN_3063d460` (103 B); failure rolls back via
`FUN_3063cbd0` (151 B). `SetInput[key]` and `[float]` are implemented by `FUN_3063e0f0`
(342 B) and `FUN_3063de20` (356 B), which are the ones a port should mirror.

---

## 2. Load pipeline

`CAnimationGraph::Init FUN_3061f210` (726 B) in order:

| # | function | size | role |
|---:|---|---:|---|
| 1 | `FUN_3061bd50` | 13,453 | parse the graph XML: `States`, `State`, `SelectWhen`, `Template`, `Parameter`, `Override`, `Mapping`, `Views`, `ExcludeFromGraph`; returns the DOM/template tree |
| 2 | `FUN_3060f730` `PreprocessTemplates` | 4,647 | load `Libs/AnimationGraphTemplates`, resolve template names, inheritance, `failed to load ... state ordering` check |
| 3 | `FUN_30655570` `PreprocessLinksAndStates` | 503 | resolve links/state names before inputs |
| 4 | `FUN_3060aed0` | 2,347 | state-order/topological resolution (`"States"`, `%s failed to load due to state ordering`) |
| 5 | `FUN_30611d30` `LoadInputs` | 6,127 | input objects, default values, ranges, mixin filters |
| 6 | `FUN_30616cc0` `LoadStates` | 6,997 | state nodes, guards, templates, parameterization, factory ranges |
| 7 | `FUN_3061a5f0` `LoadTransitions` | 2,393 | transition entries and state ordering |
| 8 | `FUN_305f9cc0` `CachePass` | 640 | transition flags, weighted table, reachability |
| 9 | `FUN_30609740` `LoadHubStates` | 1,399 | `Main Hub` lookup (`Main Hub Not Found (%s)`) |
| 10 | `FUN_3060b800` | 204 | state-id -> transition matrix |
| 11 | `FUN_3060c790` | 193 | finalize |

`PreprocessParameterization` is a profiler string at `0x30741f2c` but its body was not exported
by the decompile [?]. The XML `<Parameterization><Parameter id="Stance"><Value .../></...>`
and the `[stance]`/`[dir]`/`[num]` placeholders in state ids are resolved during 6/8; the exact
expansion is [?] (likely one state per parameter combination, with the parameter value encoded as
an equality guard).

Per-state node factories: `FUN_30631520` (483 B, `BuildStateFactories`) walks the state's factory
range (`state+0xc`, byte `state+0xa`) into the graph arrays `+0x184/+0x194`, recursively adding
the parent's factories (`state+4` parent id). `FUN_30631d20` (129 B) clears/rebuilds the factory
vector; `FUN_306352a0` (`EnterState`, 746 B) resolves the current state's factory list, sorts it,
and calls each factory's `+0xc` enter / `+0x4` leave virtual with the event buffer; it also
publishes the state's enter flags (`+0x575/576/578/57c/57a`).

---

## 3. Per-frame update and state selection

### 3.1 `CAnimationGraphState::Update FUN_30639000` (1,718 B)

1. Profiler scope `CAnimationGraphState::Update`.
2. `param_1+0x7cd = 0`; if a serialized state `+0x7c8` exists, `FUN_3064ae10`/`FUN_30643f70`/
   `FUN_30643f90`/`FUN_306ea960` restore or drop it.
3. Verify the character instance is valid (`graph+4` vs state+4) and not paused/hidden
   (`+0x274` virtual `+200`/`+0x18` flags 0x200000).
4. Compute `local_11` (telemetry) and `local_12` (input change since the last query) by comparing
   the encoded banks (`+0x2e` with bank `+0x2d` and `bank^1`), then choose `iVar6 = 1` or `2`
   update passes.
5. Loop:
   - if `+0x570` (paused) or a child state update is needed, call `FUN_30631f80` and
     `FUN_30636a00`/child `+0x1c` updates;
   - `FUN_306359e0` (`PerformQuery`) returns a query result;
   - `FUN_3062e960` (animate/teleport bookkeeping), `FUN_30632e20`, `FUN_3064b030`
     (serialization);
   - call the state function pointer `+0x708` (initial/steady/transition/ephemeral);
   - repeat while the state changed (`pcVar11 != +0x708` or `+0x570`), max 7 iterations.
6. `FUN_306388a0(result)` finalizes the character update.

### 3.2 `PerformQuery FUN_306359e0` (2,257 B)

- `BasicQuery FUN_30630c20` (345 B) -> `CStateIndex::Query FUN_3065e050` over the encoded input
  bank; the result is the candidate state list (`+0x240`, count `+0x244`) and a pass/fail bit.
- If no candidate, the graph default `+0x180` is used; if there is no default the query fails.
- For the current state, `state+0x34` indexes the transition list; for each entry whose
  `from == current`, `EvaluateGuards` is called on the state's guard array (the transition's
  target state's guards) with the encoded input bank. Passing entries are accumulated with their
  weights; if more than one, the choice is `time % totalWeight` (the low bits of the query clock),
  otherwise the first. This is the `[ag] ...` debug path.
- The "current state still satisfies its guards" case is handled by `MaybeBeginTransition`
  `FUN_30634240` (708 B) and `FUN_30634150` (228 B): a state can be left only when its guards no
  longer pass (or a force flag/teleport is set).
- `ForceTeleportToQueriedState FUN_30636480` (440 B) clears the pending queue, pushes the target
  state, calls `FUN_30631d20`/`FUN_306337f0`, sets `+0x5a4`, and calls `FUN_30635180` to leave
  the current state.
- `PathFindBetweenStates` (profiler string `0x30741284`) + `FUN_306337f0` (2,124 B) build the
  ephemeral path queue when a transition requires intermediate states.

### 3.3 The four update functions

| function | size | role |
|---|---:|---|
| `Update_InitialState FUN_30636640` | 537 | `FUN_306359e0`; if a valid state is found, set current/next, reset timers, build factories/descs, set update fn to steady, enter state (`FUN_306352a0`), fire `0x6a8` |
| `Update_SteadyState FUN_306357f0` | 483 | `FUN_306359e0(1)` (query with `force`); if the query returns a target, check `FUN_30634240` (leave allowed) and `FUN_30635590` (begin the transition); optionally switch to ephemeral |
| `Update_TransitionState FUN_30634d50` | 1,053 | drains the factory leave/enter list; when the target state's guards pass and the entity is ready, `FUN_30633690` (state-enter bookkeeping), `FUN_306352a0` enter, clears the transition, sets steady/ephemeral; sends transition events through `FUN_30631f80` |
| `Update_EphemeralState FUN_30634b70` | 480 | walks the pending state path, calls `FUN_30634240`/`FUN_30635590`/`FUN_30634150`, switches back to steady when done |
| `BeginNextTransition FUN_30635590` | 604 | pops the 24-entry ring at `+0x70e`, sends leave events (`FUN_30635180`), decrements the count/index |

### 3.4 Guards

`CAnimationGraph::SStateInfo::EvaluateGuards FUN_305f7a80` (356 B):

```
count = stateInfo+0xb
for i in 0..count-1:
    g = stateInfo+0x10 + i*2          # u16
    input = g & 0x1f
    value = (g >> 5) & 0xff
    op    = (g >> 13) & 3
    cur   = encodedInputs[input]      # u16, 0xffff = none
    if op == 0 and value <  cur: return 0
    if op == 1 and cur   <  value: return 0
    if op == 2 and cur   == value: return 0
return 1
```

So op 0 passes `cur <= value` (max), op 1 passes `cur >= value` (min), op 2 passes
`cur != value`. `min`/`max` in the XML map to 0/1 [R for the runtime, I for the loader mapping].
A state with no guards always passes. The encoded input value is a byte; key inputs use the key
index, floats/ints use their quantized value.

### 3.5 Action, Signal, events

- `Action` is a keyed input; one-shot states select on `<Action value="jumpMP01"/>`,
  `<Action value="all_re_combat_hit"/>`, etc. The x2game side writes it with
  `FUN_39153ff0` (jump) and the skill/cast state (match animation).
- `Signal` is a keyed input used by event states (`<Signal value="idle_rand"/>`,
  `all_re_combat_hit`, `fall`, `em_jump_in_place`). Template `SetInput time=N -> Signal name`
  schedules a delayed signal; the graph state processes the input change at the next query.
- The event path is `state+0x620`/`+0x640`; `FUN_30631f80` copies/merges event lists;
  `Update_TransitionState` calls the event virtuals and `FUN_30635180` sends the state-leave
  event.
- The state factory (node) order matters; `FUN_306352a0` sorts factories and calls `+0xc`
  enter and `+0x4` leave. This is where upper-body/layer nodes and gameplay event nodes attach.

---

## 4. Cryanimation side: desc, LMG and command buffer

### 4.1 Animation desc

`CAnimationSet::GetAnimationDesc FUN_31558f80` (304 B) resolves the clip/locomotion asset
(`FUN_3160cc50`, 329 B) and fills a desc. If the asset type (`+0xc`) is 2 (LMG), it fills the
capabilities block with `GetLMGCapabilities FUN_315714b0`; it then calls
`FUN_31558550` (1,187 B) for each of 13 animation layers. The desc layout used by the command
writer (see 4.3): `+0x00` flags/mode, `+0x02` example count, `+0x04` u16 animation ids,
`+0x0c` playback scale, `+0x10/+0x14` speed spans, `+0x1c` root-motion scale, `+0x28` path
string, `+0xa4` float weights[40]. (Offsets observed in `FUN_31556e90`/`FUN_31558550`.)

`FUN_31558550` (`UpdateAnimationLayer`) computes the layer's desired animation parameters from
the character movement state: it switches on the layer index 0..0xc and reads the character's
velocity/quaternion block (`param_3`), setting the desc's blend-in/out times, playback speeds and
the root-motion scale. It sets `param_2+7` to `1` when the layer is inactive. This is the
per-layer on-foot/rider policy shared by E.

### 4.2 LMG asset and capabilities

`CAnimationSet::LoadFileLMG FUN_315765c0` (2,517 B) parses `<LocomotionGroup>`:

- `<BLENDTYPE type="XL14"/>`: the four `type` bytes are stored as an integer at asset `+0x40`
  (a FOURCC, not a string enum).
- `<CAPS code="RUN_"/>`: `+0x44`.
- `<THRESHOLD value=...>`: `+0x3c`; `<LMGPARAMS LookAheadTime=...>`: `+0x28`.
- `<ExampleList><Example Position="x,y,z" AName="clip" [MoveDir=...]/>`: each example is
  44 bytes in the vector at `+0x20` (position + animation id + move dir).
- the animation name is lowercased and CRC32'd (`Crc32Gen::GetCRC32`) to resolve the CAL entry
  (`FUN_31516b20`, `param_1+0x90`), with the errors
  `locomotion group '%s' is invalid! The animation '%s' is not in the CAL-file.`
  (`0x3167864c`...).

`FUN_31568fc0` (97 B) maps the FOURCC through a 59-entry sorted table at cryanimation
`0x316b5890` (`{fourcc, u16 code}` pairs) to the switch case 0..0x39 of
`GetLMGCapabilities FUN_315714b0` (2,004 B). The table includes `S__1`, `M__1`, `S_H2`,
`ST_2`, `_TH2`, `STH2`, `STHX`, `STF1`, `STF2`, `SUD2`, `T_WS`, `S8_1`, `S8SW`, `S8R3`,
`XLV1..XLV9`, `XL10..XL15`, `MI01`, `MI02`, `M2I8`, `TSTP`, `ISTP`, ... (the fourcc compare
functions `FUN_31566430` 272 B / `FUN_31566540` 575 B list the same families).
For every family the dispatcher calls a per-type weight function (cases 0..0x39, addresses
`FUN_3156bd00` through `FUN_31571080`, 63-1,245 B each). These functions are the line-by-line
port target for the blend spaces; their bodies are identified but not yet decoded [see §6].

### 4.3 Command buffer writer `FUN_31556e90`

Inputs: `this` = `CAnimationSet`-bearing character, `param_1` = animation desc pointer,
`param_3` = command buffer (`+0x800` write cursor, `+0x804` command count), `param_2` = time.
The function:

1. resolves the animation set and LMG (`FUN_3160cc50`), checks `FUN_31561820` (594 B) that the
   weights sum to 1 (warnings `CryAnimation: LMG weights don't sum up to 1`,
   `Sum of weights is wrong. LMG blend code is '%s'`),
2. groups equal animation ids and sums their weights,
3. emits commands into the buffer (the `*puVar10 = cmd` bytes; sizes in parentheses):
   - `0` (4): reset/clear,
   - `2` (0x1c): play clip: `+1` layer/flags, `+2` u16 anim id, `+4/+8` playback range,
     `+0xc` weight, `+0x10` ?, `+0x14` blend time, `+0x18` flag,
   - `3` (0x18): secondary/track setup (id, time, weight),
   - `7` (8): finalize layer (u16 0x300 + playback),
   - `9` (8): set LMG blend/playback,
   - `10` (8): set animation set (id),
4. accumulates root motion (`this+0x14/+0x18/+0x1c`) from the weighted example positions,
5. sets the dirty bit `this+0x458 |= 1`.
Command buffer overflow is the `CryAnimation: CommandBuffer overflow!` error; the buffer is
0x800 bytes and the writer checks the remaining space before each command.

The static validators `FUN_31561820` and the error strings `0x31677ee4`, `0x316780ec`,
`0x31678194`, `0x31678238` are useful differential oracles: the original reports bad weights,
bad LMG assets and wrong sums; a port must reproduce the same command bytes for valid data and
the same diagnostics for invalid data.

### 4.4 Which blend type is used where

From the extracted LMGs (`tmp` extraction; same files in the pak) and `re-mounts-full.md` §3-4:

| asset | BLENDTYPE | used by |
|---|---|---|
| `animals/horse/fist_mo_normal_run.lmg` | `XL14` | horse run (16 rows: 8 dirs, lturn/rturn, uphill/downhill, sprint tier) |
| `animals/horse/fist_mo_normal_walk.lmg` | `S8_1` | horse walk |
| `animals/horse/fist_mo_relaxed_turn.lmg` | `TSTP` | horse turn in place |
| `animals/horse/fist_mo_relaxed_step.lmg` | `ISTP` | horse idle steps |
| `animals/horse/fist_mo_relaxed_idletorun.lmg` / `runtoidle` | `M2I8` | horse start/stop |
| `nuian/male/fist_mo_rider_horse_run.lmg` | `XL14` | rider run while mounted |
| `nuian/male/fist_mo_rider_horse_idletorun.lmg` | `M2I8` | rider start/stop |
| `nuian/male/fist_mo_normal_run.lmg` | `XL15` | player run |
| `nuian/male/fist_mo_normal_walk.lmg` | `S8_1` | player walk |
| `nuian/male/fist_mo_relaxed_runtoidle.lmg` | `MI02` | player run->idle |
| `nuian/male/fist_mo_relaxed_walktoidle.lmg` | `MI01` | player walk->idle |
| `nuian/male/fist_mo_swim_run.lmg` / `swim_deep_run` | `S8SW` | swim |

The `BLENDTYPE type=` value is the fourcc read by `LoadFileLMG`; the rest of the LMG file
(`Example Position`, `AName`, `MoveDir`, `TimeList`) is parsed by `FUN_315765c0` and the
`FUN_3156*` family.

---

## 5. Upper-body layers and the animated character

`CAnimatedCharacter::Init` (body `0x30662b40..0x30662f70`, 1,072 B; no `funcs.csv` entry)
loads the character's animation graphs:

- reads the model's animation-layer properties and loops over 8 layers
  (`AnimationLayer1..8`, string `0x30744540`; `this+0x10 + layer*4` graph pointers);
- the graph name comes from the layer property; the fallbacks are `player_full_body`
  (`0x30746a9c`) and `player_upper_body` (`0x30746a88`);
- for each graph it calls `FUN_30626530` (graph load/create, inferred) and
  `FUN_3063cf10` (144 B), which adds the graph to the character's `CAnimationGraphStates`
  wrapper at `+0x24` and registers the `"wrapper"`;
- a missing property logs `No AnimationGraph layer property defined for entity`
  (`0x307469f0`) and fails the init; the graph must be present in the entity/model
  (`ERROR: AnimatedCharacter::Init() - Can't load AnimationGraph '%s'...`, `0x30746a28`);
- `this+0x204` gets the model scale, `this+0x24` the graph-state wrapper.

The wrapper `CAnimationGraphStates::Update FUN_3063b090` (372 B) updates every registered graph
state (and thus every layer) each frame. The per-layer output is blended on the cryanimation side:
`FUN_31558550` handles 13 layer slots (`for iVar1<0xd`), and the command writer emits the layer
commands (§4.3). The upper-body graph is just a second `CAnimationGraphState`; the runtime does
not special-case it in x2game. The exact masking of lower-body bones is in cryanimation's
layer/command consumer, not in the graph state [R for the load/wrapper, ? for the bone mask
internals].

`CAnimatedCharacter::Update FUN_306637e0` (487 B) calls `FUN_306666b0`, the world/entity time
(`DAT_307a36b4+0xa8`), `FUN_3066b2b0`, `FUN_3066ea40` (current motion block), the entity
pose update (`FUN_306617f0`, `FUN_30671e70`, `FUN_30671cc0`), and drives the graph update through
the entity/character interfaces. It is the frame entry that a port must call after setting the
graph inputs.

---

## 6. Function table

Sizes from `funcs.csv`; "stateful" means it needs a live graph/character/actor object.

| addr | size | role / callers | key offsets | kind | test |
|---|---:|---|---|---|---|
| `3061bd50` | 13453 | XML parser (`Init` #1) | DOM, `States/State/SelectWhen/Template/Parameter/Override/Mapping/Views/ExcludeFromGraph` | stateful | call with a scratch graph + a small XML file; compare the DOM |
| `3060f730` | 4647 | `PreprocessTemplates` (`Init` #2) | template map, `Libs/AnimationGraphTemplates` | stateful | call with a parsed graph; compare templates |
| `30655570` | 503 | `PreprocessLinksAndStates` (`Init` #3) | link/state maps | stateful | call after the parse |
| `3060aed0` | 2347 | state-order resolution (`Init` #4) | `"States"` | stateful | call after the parse; compare ordering |
| `30611d30` | 6127 | `LoadInputs` (#5) | inputs vector `+0x10/+0x14`, input objects | stateful | call with a parsed graph; dump input objects |
| `30616cc0` | 6997 | `LoadStates` (#6) | states `+0x58`, guards `state+0x10`, `state+0xb`, factories | stateful | call with a parsed graph; dump states |
| `3061a5f0` | 2393 | `LoadTransitions` (#7) | transitions `+0x68`, 0xc bytes/entry | stateful | call; dump transition array |
| `305f9cc0` | 640 | `CachePass` (#8) | state bits `+6/+7`, `+0x21c/+0x220` | stateful | call; dump flags |
| `30609740` | 1399 | `LoadHubStates` (#9) | `Main Hub` | stateful | call; compare hubs |
| `3060b800` | 204 | transition matrix (#10) | graph `+0x?` | stateful | call; compare matrix |
| `3060c790` | 193 | finalize (#11) | — | stateful | call; compare graph |
| `3061f210` | 726 | `CAnimationGraph::Init` | calls #1..#11 | stateful | call with a parsed graph; compare the whole graph |
| `305f7a80` | 356 | `SStateInfo::EvaluateGuards` | guard `+0xb/+0x10`; `input/value/op` packing | pure | allocate guard array + encoded inputs; call `thiscall` this=stateInfo; EAX 0/1 |
| `30632810` | 345 | `CAnimationGraphState::SetInput` | `+0x2e`, `+0xb0`, `+0x138`, `+0x140` | stateful | call with a scratch state object; dump the banks |
| `3063e0f0` | 342 | `SetInput[key]` impl | input objects | stateful | call with a live character; read the graph inputs |
| `3063de20` | 356 | `SetInput[float]` impl | input objects | stateful | same |
| `3063d6e0` | 564 | `SetInputOptional[key]` | change ids | stateful | same |
| `30639000` | 1718 | `CAnimationGraphState::Update` | `+0x20/+0x24/+0x26/+0x28/+0x2a/+0x2e/+0x240/+0x570/+0x708/+0x70c` | stateful | call per frame with a live character graph; dump current state |
| `306359e0` | 2257 | `PerformQuery` | candidate vector, transitions, default `+0x180` | stateful | call with a live graph state; dump the result struct |
| `30630c20` | 345 | `BasicQuery` | `+0x764` query result | stateful | call with a live graph state; dump `+0x764` |
| `3065e050` | 502 | `CStateIndex::Query` | `+0x90/+0xb0/+0xc0/+0xc2/+0x200` | stateful | call with a scratch graph + encoded inputs; dump candidates |
| `30636640` | 537 | `Update_InitialState` | state/queue | stateful | call on a fresh graph state |
| `306357f0` | 483 | `Update_SteadyState` | state/queue | stateful | call after init; compare transitions |
| `30634d50` | 1053 | `Update_TransitionState` | factories, events | stateful | call during a transition; dump events |
| `30634b70` | 480 | `Update_EphemeralState` | path queue | stateful | call on a multi-hop path |
| `30635590` | 604 | `BeginNextTransition` | `+0x70c/+0x70d/+0x70e` | stateful | call with a queued path; dump the queue |
| `30634240` | 708 | `MaybeBeginTransition` | guard/query | stateful | call in steady state; compare the target |
| `306352a0` | 746 | `EnterState` | factories, `+0x575..+0x57c` | stateful | call on a state change; dump the factory calls |
| `30631520` | 483 | `BuildStateFactories` | `+0x184/+0x194`, `state+0xc/+0xa` | stateful | call with a parsed graph/state |
| `30631d20` | 129 | clear/rebuild factories | `+0x5b8` | stateful | call with a live state |
| `30636480` | 440 | `ForceTeleportToQueriedState` | queue/target | stateful | call with a live state; compare forced state |
| `30600220` | 224 | `GetStateAnimationDesc` | `state+0x40/+0x8c` | stateful | call with a live state; dump the desc |
| `305fdc40` | 413 | `ConstructAnimationDescForAllStates` | graph `+0x182` | stateful | call after load |
| `305fa110` | 407 | `ConstructAnimationDesc` | desc | stateful | call with a state |
| `30604b10` | 305 | `CIntegerInputValue::EncodeInput` | parse int | pure-ish | call with a scratch input object; compare the encoded byte |
| `30605180` | 327 | `CFloatInputValue::EncodeInput` | parse float | pure-ish | same |
| `30611470` | 345 | `CKeyedInputValue::EncodeInputs` | key map `+0x14/+0x1c`, index `+0x28` | pure-ish | same (unknown key -> 0xffff) |
| `3063b090` | 372 | `CAnimationGraphStates::Update` | wrapper `+0x10/+0x14` | stateful | call per frame; all layer graphs tick |
| `3063cf10` | 144 | register graph in wrapper | wrapper `+0x80/+0x84` | stateful | call with a loaded graph |
| `30662b40` (no entry) | 1072 | `CAnimatedCharacter::Init` (layers) | `+0x10..+0x30`, `+0x24`, `+0x204` | stateful | call with a model/entity; compare layer graph names |
| `306637e0` | 487 | `CAnimatedCharacter::Update` | `+0x204/+0x738/+0x744` | stateful | call per frame |
| `30665830` | 9646 | `SetAnimGraphSpeedInputs` | `+0x24` graph, `+0x6d0..+0x6e0` ids, `+0x659/+0x65a` | stateful | call with a scratch motion block; read the graph inputs |
| `315765c0` | 2517 | `LoadFileLMG` | asset `+0x20/+0x28/+0x3c/+0x40/+0x44` | stateful | call with a parsed LMG XML; dump examples/caps |
| `315714b0` | 2004 | `GetLMGCapabilities` (dispatch) | `+0x40` fourcc -> case | stateful | call with an LMG asset; compare the caps struct |
| `31568fc0` | 97 | fourcc -> case table | `cryanimation+0x1b5890` (59 pairs) | pure | call `cdecl` with a fourcc; compare the code |
| `31558f80` | 304 | `GetAnimationDesc` | desc layers 0..0xc | stateful | call with a live animation set; dump the desc |
| `31558550` | 1187 | per-layer desc data | desc `+0x00..+0xa4` | stateful | call for each layer; compare the desc |
| `31556e90` | 2488 | command buffer writer | buffer `+0x800/+0x804`; desc `+0x04/+0xa4` | stateful | call with a fixed desc; compare the command bytes |
| `31561820` | 594 | LMG weight validator | weights `+0xa4` | pure-ish | call with a scratch desc; compare the warning/no-warning result |
| `31566430` | 272 | blend-type family test | fourcc | pure | call `cdecl` with each fourcc; 0/1 |
| `31566540` | 575 | blend-type family test | fourcc | pure | same |

The fourcc table and the `GetLMGCapabilities` switch are the fastest way to validate the
loader: read the live table at `cryanimation+0x1b5890` (59 entries) and compare the case mapping
for `XL14`, `S8_1`, `M2I8`, `TSTP`, `ISTP`, `S8SW`, `XL15`, `XLV5`, `MI01`, `MI02`.

---

## 7. Unknowns specific to D

1. **Per-blend-space weight formulas** (all cases in `GetLMGCapabilities`, especially XL14,
   S8_1, M2I8, TSTP, ISTP, S8SW). The dispatcher/caps/command writer are mapped; the case bodies
   are the remaining line-by-line work. [highest priority]
2. **Parameterization expansion** (`PreprocessParameterization`, `[stance]`/`[dir]`/`[num]`).
3. **Guard op 2 / min-max packing** at load time (runtime semantics are proven).
4. **Float encode quantization** (the per-object virtual; likely linear min/max -> 0..255).
5. **`Update_SteadyState` / `Update_EphemeralState` branch details** (the high-level flow is
   mapped; the exact leave/enter order for equal-cost links and `forceFollowChance` is [?]).
6. **Upper-body bone masking** (cryanimation consumer, not the graph state).
