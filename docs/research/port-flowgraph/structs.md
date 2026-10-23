# Structs: flow graph, sequence, camera and controller layouts

Only offsets that the 1.2 code actually reads/writes are listed. Everything is little-endian
x86; `f32` is IEEE-754. Offsets are relative to the object pointer in `ECX` (this) unless the
function signature says otherwise. Where the decompile does not settle a field's name it is
called `u32_xx` and the evidence (the function that touches it) is cited.

## 1. `CFlowSystem` (CryAction)

The object is the `IFlowSystem` returned by the game framework; x2game reaches it as
`DAT_3a5bb1d8[0x19]` (offset `0x64`) and the game framework itself as `DAT_307a36b4` inside
CryAction. Only fields observed in `CFlowSystem::Update` (`0x30556640`), `RegisterType`
(`0x30559170`) and `UpdateStats` (`0x30549cc0`) are listed.

| offset | type | meaning | evidence |
|---:|---|---|---|
| +0x24 | `void*` | type array begin | `RegisterType` indexes `+0x24 + id*8` |
| +0x28 | `void*` | type array end | max 4095 check `(end-begin)>>3` |
| +0x34 | `CFlowGraphBase**` | graph list begin | `Update` loop |
| +0x38 | `CFlowGraphBase**` | graph list end | `Update` loop |
| +0x54 | `u16` | short id for category 0 | `RegisterType` writes `+0x54 + cat*2` |
| +0x5c | `CFlowGraph*` | current graph? | `Update` early return when 0 |
| +0x70 | `u8` | debug flag (updates enabled) | `Update` |
| +0x44/0x48 | `void*` | per-frame "pending" list | `Update` loops |

The type entry is 8 bytes: `{u16 id; u16 pad; factory*}` (from `RegisterType` writing
`*(short*)puVar7 = id` and the factory at `+4`). The name→id map is a separate hash map; the
name table in `.data` is what `RegisterType` is called with at startup.

## 2. `CFlowGraphBase` / `CFlowGraph` (CryAction)

Ctor `0x30551a00` (1,030 bytes). `DoUpdate` `0x3054eb40` (984 bytes). This is the layout the
port needs.

| offset | type | meaning | evidence |
|---:|---|---|---|
| +0x00 | vtable | `IFlowGraph` (multiple interface vtables follow at +4…+0x1c) | ctor |
| +0x20 | `std::string` | graph name (`%s` in the max-loop log) | ctor, `DoUpdate` |
| +0x24..+0x2c | containers | port vector bookkeeping | ctor |
| +0x44/+0x48/+0x4c | containers | second port vector bookkeeping | ctor/`DoUpdate` |
| +0x54 | `u16` | activation queue head, event 0 | `DoUpdate` |
| +0x56 | `u16` | current activation port, event 0 | `DoUpdate` |
| +0x5c | `u16*` | queue link array, event 0 | `DoUpdate` |
| +0x6c | `u16` | activation queue head, event 1 | `DoUpdate` |
| +0x70 | `port[0x20]` array | port/edge slots | `DoUpdate`, `CreateNode` |
| +0x94 | container | node list | ctor |
| +0xa0 +0xa4 | `u16*` | edge free list | `FUN_30550c40` |
| +0xb0 | `u8` | enabled | ctor / savegame |
| +0xb2 | `u16` | flags, initial `0x101` | ctor |
| +0xb3 | `u8` | needs-initialize | ctor / savegame `needsInitialize` |
| +0xb4 | `void*` | owner / interface (param_2 of ctor) | ctor |
| +0xd8 | `u32` | special entity id 1 (`0xffffffff` port value) | `DoUpdate` |
| +0xdc | `u32` | special entity id 2 (`0xfffffffe`) | `DoUpdate` |
| +0xe4 | map | node name → node index (savegame resolve) | `FUN_30552a80` |
| +0x110/+0x114/+0x118 | containers | node list used by `Update` | `Update` |
| +0x124 | `IFlowSystem*` | entity/interface resolver | `DoUpdate` |
| +0x130 | `CFlowGraph*` | next graph in the update chain | `DoUpdate`, `UpdateStats` |
| +0x134 | `CFlowGraph*` | tail? | ctor |
| +0x13c | — | end of the fields seen | ctor |

### 2.1 Port / edge slot (0x20 bytes)

The graph keeps a flat array at `+0x70`; every port is one 0x20-byte slot. Observed fields:

| slot offset | type | meaning | evidence |
|---:|---|---|---|
| +0x00 | `u32` | first edge pointer (low bits flags) | `DoUpdate` `iVar8 = idx*0x20 + graph+0x70` |
| +0x08 | `u8*` | value array base; `+ (flagBit)*8` selects the value | `DoUpdate` (`local_1c`) |
| +0x10 | `CFlowNode*` | node object for this port | `DoUpdate` calls `node->vtable+0x18(portId,value)` |
| +0x18 | `u32` | entity id if `+0x1e & 0x1000` | `DoUpdate` |
| +0x1c | `u8` | edge count | `DoUpdate` clears `count` edge flags |
| +0x1e | `u16` | flags (`0x1000` entity, `0x2000` forwarding, `0x4000` resolved) | `DoUpdate`, `FUN_30546380` |
| +0x20 | — | next slot | — |

An edge is 8 bytes (`(u32*)(firstEdge) + i*2`): a target port id/flag dword at +0 and a second
dword; `DoUpdate` clears bit 1 (`& 0xfffffffd`) of the first dword after firing. **[R]** — the
exact high bits are not named in the decompile, mark them unknown.

### 2.2 Activation port id

The value passed to `ProcessEvent` is a packed 32-bit id. From x2game's helpers
(`0x3914a400`, `0x3914a450`, `0x3914a4a0`, `0x3914a4f0`, `0x3914a590`) and
`0x39095cd0`:

```
bits 0..15   node id (u16, as in the XML Id)
bits 16..23  port index (u8)
bits 24..31  edge index / activation flags (u8)
```

Examples: `CONCAT22(0x103, nodeId)` = output event on port 3; `CONCAT22(0x101, nodeId)` = port 1
with value 1; `CONCAT13(1, CONCAT12(port, nodeId))` = activate input `port`.

## 3. Node objects

Base interface (all node classes): vtable at +0, then the class fields. Observed base offsets:

| offset | meaning | evidence |
|---:|---|---|
| +0x00 | vtable; `+0x18` = `ProcessEvent(portId, SFlowValue*)`, `+0x1c` = `GetValue`, `+0x20` = `SetValue`, `+0x24` = `GetPort` | `DoUpdate`, `CFlowBaseNode` helpers |
| +0x08 | first port / value storage | x2game nodes |
| +0x0c | second field (often a pointer) | x2game nodes |
| +0x10 | third field (often an id) | x2game nodes |

`CFlowBaseNode::GetPortBool` (`0x3055b0a0`) and `GetPortString` (`0x3055bed0`) read the
`SFlowValue` variant. The variant is 8 bytes: low dword = tag (0 bool/int, 1 float, 2 vec3,
3 entity id, 6 string), high dword = payload/pointer. **[R]** from the tag comparisons in those
two functions and the x2game `FUN_39000180` family.

## 4. `Camera:OverrideFOV` node (x2game)

Node object is 0xc bytes (`FUN_3914acb0` allocates 0xc):

| offset | type | meaning |
|---:|---|---|
| +0x00 | vtable | `0x399ae30c` |
| +0x04 | `u32` | reserved / owner |
| +0x08 | `f32` | `m_storedFOV` (saved FOV, 0 = none) |

`GetConfiguration` `0x3914b3f0`; `Serialize` `0x3914ab90` (`m_storedFOV`, `curFOV`); `ProcessEvent`
`0x3914ad30` (port 1 = set: save the old FOV and apply the value, also emitting the current FOV on an
output edge; port 3 = reset to the saved FOV; all through the `cl_fov` cvar). The FOV write goes
through the console variable `cl_fov` (`0x3914ab90` gets the cvar object with
`DAT_3a5bb1d8+0x1c` vtable+0x60, then calls its `+0x18` setter). **[R]**

## 5. `CSequence` (CryMovie)

Ctor `FUN_340028a0` (369 bytes); the object is at least 0x74 bytes in the ctor. Fields observed:

| offset | type | meaning | evidence |
|---:|---|---|---|
| +0x00 | vtable | `0x3404c490` | ctor |
| +0x04 | `int` | refcount / state | ctor |
| +0x08 | `u32` | flags | ctor |
| +0x0c | `int` | current time? | `PlaySequence` |
| +0x10/+0x14 | `i64` | start time (100 ns) | `Update` compares with the timer |
| +0x18 | `f32` | start time seconds | ctor |
| +0x1c..+0x38 | container | node list / track list | ctor |
| +0x28 | `u8` | active/playing | ctor |
| +0x30/+0x34 | `void*` | sequence graph object | `0x34002620` fallback `<Objects>` |
| +0x40 | `std::string` | sequence name (`Name=`) | ctor |
| +0x44 | `std::string` | world name used by `Game/worlds/<world>/...` (inferred: empty in the ctor, filled from the current level when the caller passes no world) | `0x34002620` |
| +0x50/+0x54 | `f32` | `StartTime`/`EndTime` | ctor |
| +0x58 | `u32` | flags (`Flags=`) | ctor |
| +0x5c | `u32` | `TimeOfDayName` ptr | ctor |
| +0x60/+0x61 | `u8` | paused / break-on-stop | ctor |
| +0x6c | `IMovieSystem*` | owner | ctor |
| +0x70 | `u16` | mode | ctor |

A sequence node (the tracks' owner) is created from the XML `Node`; `FUN_34027680` resolves an
entity/unit by name (`Failed to find object(entity, unit, ...)!!! its name is '%s'`,
`0x3404dae8`). The XML `<Node Id>` is a 32-bit identity, not CRC32 of the name (checked:
`AnimObject_18` → `0xf6e69c62` ≠ `-1312470402`); the hash function is **unknown** (it is used
only for identity inside the file, so the port can keep it as an opaque u32).

## 6. `quest_cameras` descriptor (client DB)

Loaded by `FUN_39892880` (743 bytes) into 0x3c-byte records. Every field is copied from the SQL
columns in order:

| offset | type | column | notes |
|---:|---|---|---|
| +0x00 | `f32` | `id` | stored as float by the loader (`fVar6 = FUN_3953a390`) |
| +0x04 | `f32` | `camera_offset_x` | |
| +0x08 | `f32` | `camera_offset_y` | |
| +0x0c | `f32` | `camera_offset_z` | |
| +0x10 | `void*` | `comment` | interned string |
| +0x14 | `u8` | `dof` | 0/1 |
| +0x18 | `f32` | `fov` | degrees |
| +0x1c | `u8` | `interpolate` | |
| +0x1d | `u8` | `invisible` | applied to unit+0x3c5a |
| +0x20 | `f32` | `npc_offset_x` | |
| +0x24 | `f32` | `npc_offset_y` | |
| +0x28 | `f32` | `npc_offset_z` | |
| +0x2c | `f32` | `nv_bokeh_size` | |
| +0x30 | `u8` | `nv_dof` | |
| +0x34 | `f32` | `nv_intensity` | |
| +0x38 | `f32` | `nv_luminance` | |

In the camera actor update `FUN_3916b440` the offsets are copied as: desc+4..+0xc →
actor+0x1c8..+0x1d0; desc+0x20..+0x28 → actor+0x1d4..+0x1dc; desc+0x18 → actor+0x1e0;
desc+0x1d → `FUN_39354520` (unit+0x3c5a).

## 7. `IMovieSystem` (CryMovie) vtable

The object is `PTR_FUN_3404d1a8` from the ctor `FUN_3400f0c0`; the vtable is in
`crymovie.dll` .rdata at `0x3404d1a8`. Offsets the client uses:

| offset | address | operation |
|---:|---|---|
| +0x04 | `0x340207a0` | set camera actor / listener sink |
| +0x18 | `0x3400e4e0` | `FindSequence(name)` |
| +0x1c | `0x3400e580` | — |
| +0x20 | `0x3400ee40` | — |
| +0x24 | `0x3400cb50` | `FindSequence` / get by name |
| +0x2c | `0x3400c630` | `GetSequenceFlags` (bit 0x4000 = fade) |
| +0x34 | `0x3400f700` | `AddMovieListener(seq, listener)` |
| +0x4c | `0x3400e200` | `PlaySequence(seq, 0,0, color...)` |
| +0x54 | `0x3400f6a0` | — |
| +0x58 | `0x3400ce90` | — |
| +0x60 | `0x3400e160` | stop all / reset |
| +0x68 | `0x3400dba0` | — |
| +0x6c | `0x34020860` | — |
| +0x70 | `0x3400cef0` | `IsPlaying(seq)` |
| +0x74 | `0x3400bda0` | `IsPlayingAny` (used by `OnUnitState`) |
| +0x84 | `0x3400bdf0` | `IsPlayingSequence` |
| +0x94 | `0x3400bd20` | store a pointer |
| +0xac | `0x3400cdc0` | `IsPaused` / sequence flag |
| +0xb4 | `0x340207f0` | `StopSequence` thunk |
| +0x118 | (in the sequence vtable) | current time |

The `TryStartCinema` call sites use exactly these offsets, so this table is the contract for the
Bevy `SequencePlayer` and for the difftest driver.

## 8. Cinematic camera actor (x2game)

Allocated 0x368 bytes in `FUN_3916bbd0`; ctor `FUN_3916ad80`; vtable `0x399b0a90`. Fields seen in
`FUN_3916ad80`, `FUN_39164ff0`, `FUN_3916b440`, `FUN_39161540`:

| offset | type | meaning |
|---:|---|---|
| +0x00 | vtable | `0x399b0a90` |
| +0x04 | `int` | camera type (1..6; 3 = quest/fixed) |
| +0x08..+0x30 | floats | near/far/fov/height/offset parameters |
| +0x94 | container | target/history list |
| +0x138/+0x134 | `u32` | tracked entity ids (old/current) |
| +0x194 | `int` | previous camera type |
| +0x1a8 | `u8` | quest-camera enabled |
| +0x1ac | `u32` | quest-camera id (`0x3ff` = use model mapping) |
| +0x1b0 | `u32` | tracked unit id (set by `FUN_3915b160`) |
| +0x1b4 | `u32` | current unit id |
| +0x1b8..+0x1c4 | 4×`f32` | heading/rotation smoothing state |
| +0x1c8/+0x1cc/+0x1d0 | 3×`f32` | camera offset (from desc +4/+8/+0xc) |
| +0x1d4/+0x1d8/+0x1dc | 3×`f32` | npc offset (from desc +0x20/+0x24/+0x28) |
| +0x1e0 | `f32` | fov (from desc +0x18) |
| +0x1e4 | `f32` | target speed / glider speed |
| +0x1f8..+0x2ac | arrays | 11-entry blend state (index `+0x1a0`, entries of 0x20 bytes at +0x2c8) |
| +0x2a0/+0x2ac | `f32[11]` | blend duration/remaining |
| +0x35c | `u32` | cine-volume/mood parameter |
| +0x360 | `u8` | cine flag |

`FUN_3916b440` reads `param_3`/`param_4` (the sequence camera id) and returns immediately if
`param_3 != +0x1ac`; `DAT_39ac3f60` is the "no camera" sentinel.

## 9. Flow-graph skill controller (kind 8)

`skill_controllers` row is 24 dwords in SELECT order:

```
+0x00 id                +0x04 active_weapon_id   +0x08 end_anim_id
+0x0c end_skill_id      +0x10 kind_id            +0x14 start_anim_id
+0x18 str_value1        +0x1c transition_anim_1  +0x20 transition_anim_2
+0x24 value1            +0x28 value10            +0x2c value11
+0x30 value12           +0x34 value13            +0x38 value14
+0x3c value15           +0x40 value2             +0x44 value3
+0x48 value4            +0x4c value5             +0x50 value6
+0x54 value7            +0x58 value8             +0x5c value9
```

(`re-skills.md` §1.1, verified against `FUN_3971e060`'s SELECT order and `FUN_390ca700`'s
`param_2[4]` kind switch / `param_2[6]` string.)

Controller object (`FUN_390959f0`, 0x144 bytes):

| offset | type | meaning |
|---:|---|---|
| +0x00 | vtable | `0x399a2cc0` |
| +0x04 | `u32` | local time interface |
| +0x08 | `skill_controller_desc*` | set by `FUN_390ca700` after construction |
| +0x0c | `u32` | tag (source unit/plot) |
| +0x10 | `u32` | input flags |
| +0x14 | `u32` | (second local interface) |
| +0x18..+0x13c | 0x124 bytes | state block copied by the factory (0x49 dwords; includes the desc/parameters, exact field names not decoded) |
| +0x13c | `IFlowGraph*` | graph handle (`param_1[0x4f]`) |
| +0x140 | `u8` | started/ended |

The ctor calls, in order: `flowSystem->vtable+0x10` (create graph), `graph->vtable+0x24`
(owner/name), `graph->vtable+0x40(name, 1)` (init), `graph->vtable+0x48` (activate), then
`ActivatePort(nodeId | port<<16, &value)` for `X2:SkillController` nodes. The destructor
`FUN_39095cd0` sends port `0x101` value 1 and releases the graph.

## 10. Sequence XML node types

| `Type` | meaning | attributes |
|---:|---|---|
| 1 | entity/actor | `EntityGUID`, `DynamicType`, `GroupUnitText`, `Pos`, `Rotate`, `Scale` |
| 2 | scene | `Name="Scene"`; tracks for AVI/subtitles/camera/sounds |
| 3 | camera | `FOV`, `ShakeAEnabled/AmplitudeA/FrequencyA/...`, `ShakeB*`, `TimeOffsetA/B`, `EntityGUID` |

`DynamicType="0"` = fixed entity, `"1"` = unit (player/NPC), absent = scene/level entity. 128
nodes are `DynamicType="1"` in the 255 sequences. **[R]** — the flag names come from
`FUN_34027680`'s XML readers (`EntityGUID` `0x3404eb98`, `DynamicType` `0x3404eb60`,
`GroupUnitText` `0x3404eb2c`).
