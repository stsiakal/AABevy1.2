# Struct layouts for the portal / teleport / return-point port

Companion to [`MAP.md`](MAP.md). Offsets are from the 1.2 x2game decompile and `.data` dump.
`[R]` = read from the binary, `[I]` = inferred. Only the fields the portal system touches are
listed; the full unit/actor layout lives in the other port maps.

## 1. `ClientCharacter` (the per-character client object)

Pointer: `GameClient+0x40` (`DAT_39b02294` + 0x40). Every portal handler reads it that way [R].
`GameClient+0x10C` is the local unit pointer [R].

| offset | type | field | evidence |
|---|---|---|---|
| `+0xB00` | `PortalEntry*` | private ("Recorded Site") begin | `FUN_3930F620/F6B0/FC80` scan `+0xB00` |
| `+0xB04` | `PortalEntry*` | private end | same |
| `+0xB08` | `PortalEntry*` | private capacity | `FUN_3930F7F0` / `FUN_3930FC80` |
| `+0xB10` | `PortalEntry*` | district ("Default Site") begin | `FUN_3930F6B0/FD40/FD80` |
| `+0xB14` | `PortalEntry*` | district end | same |
| `+0xB18` | `PortalEntry*` | district capacity | `FUN_3930FD80` |
| `+0x608` | `u32` | bound return-point id | `FUN_3938FD20`, `FUN_3930F6B0`, `FUN_3930FD40`, `0x391D4240` |
| `+0x60C` | `u32` | resurrection district id | `0x391D4240` (`*(pkt+0x14)`) |
| `+0xB20` | `u32` | last auto-portal-use time (ms) | `FUN_3930CFF0` |
| `+0xB24` | `u32` | auxiliary timer (skill 0x5B cooldown) | `FUN_3930B600` |
| `+0xB28` | `u32` | auxiliary timer total | same |
| `+0xAF4` | `u32` | death time (ms) | `FUN_39495C00`, `FUN_39186E70` |
| `+0xAF8` | `u32` | server `resurrect_wait_ms` | same |
| `+0xAFC` | `u8` | resurrection offered / can-resurrect | `0x391D39F0`, `0x391D3AD0`, `FUN_39495C00` |
| `+0x4B8` | object | inventory manager (item 4045 lookup) | `FUN_3938F7D0/F950/FAA0` |

The two vectors are `std::vector<PortalEntry>`; the code uses begin/end/cap and `0x9C` stride
(`(end-begin)/0x9C` for the count, `FUN_3930B410/B480`) [R].

## 2. `PortalEntry` (0x9C bytes)

Proven by the `PortalWire` reader `FUN_397B1E30` and the row builder `FUN_3930CB90` [R]:

```c
struct PortalEntry {          // sizeof == 0x9C
    u32  id;                  // +0x00  portal_book_coords.id / return point id
    char name[0x84];          // +0x04  UTF-8, max 0x80 copied, NUL-terminated
    u32  zone_id;             // +0x88
    f32  x;                   // +0x8C
    f32  y;                   // +0x90
    f32  z;                   // +0x94
    f32  z_rot;               // +0x98  degrees (reader: zRot)
};                            // +0x9C
```

`name` is 0x84 bytes only because the next field is at +0x88; the reader caps the string at 0x80
and writes the terminator, so bytes +0x84..+0x87 are padding. Rust:

```rust
#[derive(Clone, Default, PartialEq)]
pub struct PortalEntry {
    pub id: u32,
    pub name: String,        // <= 128 bytes on the wire
    pub zone_id: u32,
    pub pos: [f32; 3],
    pub z_rot: f32,
}
```

## 3. Packet object layouts

Packet classes are `{vtable, opcode, payload}`; the payload begins at `+0xC` [R, same convention as
`port-combat/packets.md`]. The sizes below are the `FUN_3950A1B0` allocation sizes in the factories.

| opcode | object | payload layout | max rows |
|---|---|---|---|
| 0x05B | 0x1E88 | `+0xC` i32 count; `+0x10` inline `PortalEntry[]` | 50 (`(0x1E88-0x10)/0x9C`) |
| 0x05C | 0x3D04 | `+0xC` count; `+0x10` rows; `+0x3D00` u32 `returnDistrictId` | 100 (`(0x3D00-0x10)/0x9C` = 100) |
| 0x05D | 0xA8 | `+0xC` one `PortalEntry` | 1 |
| 0x05E | 0x14 | `+0xC` u8 `portalType`; `+0x10` u32 id | — |
| 0x060 | 0x10 | `+0xC` u32 unitId | — |
| 0x049 | 0xB8 | `+0xC` unitId; `+0x10` returnDistrict; `+0x14` resurrectionDistrict; `+0x18` u8 changed; `+0x1C` `PortalEntry` | 1 |
| 0x197 | 0x2C | `+0xC` iid; `+0x10` zoneId; `+0x14` pos[3]; `+0x20` ang[3] | — |

The 0x049 size is the strongest confirmation of the layout: 0xB8 − 0x1C = 0x9C.

### 3.1 Client→server bodies (built on the stack by the send site)

The send sites build a `{vtable, opcode, payload}` object on the stack (then `0x3AC1058C` sends it).
Payloads created by the mapped builders [R]:

| request | payload |
|---|---|
| `CSUsePortal` | `+0xC` u32 id (`unit+0x418`), `+0x10` byte flag; the serializer `0x397C2A50` additionally names `type` (u32) and `role` (u8) |
| `CSDeletePortal` | `+0xC` u32 type (1/2), `+0x10` u32 id |
| `CSUseTeleport` | `+0xC` u32 returnPointId, `+0x10` u32 money, `+0x14` u32 npcUnitId |
| `CSNaviTeleport` / `CSNaviOpenPortal` / 0x0EA | `+0xC` u32 doodadId |
| `CSStartSkill` (book) | see §4 |

Do not assume the server→client reader class sizes apply to the C→S stack objects: the 0x0DE
serializer's three fields and the 0x0DF builder's two fields are exactly the open questions
U-A2/U-A3 in [`packets.md`](packets.md).

## 4. `CSStartSkill` skill object (portal requests)

Stack block observed in `FUN_3938FAA0` (open), `FUN_3938F7D0` (save), `FUN_3938F950` (rename) [R]:

```c
struct PortalSkillObject {
    u32 unknown;          // +0x00  always 0 in these three paths
    u32 type;             // +0x04  1 = PortalInfo, 2 = SavePortalInfo
    union {
        struct {          // type 1
            u32 portal_type;  // +0x08  1 district / 2 private
            u32 portal_id;    // +0x0C
            /* +0x10 .. +0xE0 zero-filled in FUN_3938FAA0 */
        } open;
        struct {          // type 2
            u32 id;           // +0x08  0 = save, existing id = rename
            char name[0x81];  // +0x0C
        } save;
    };
};                        // 0xE4 stack block, 0xD0 zeroed by the save/open paths
```

The client uses 4-byte fields; the emulator's `SkillObject` uses `u8 type, u8 kind, i32 id, i64 x,
i64 y, f32 z` (`portals.md` §2.3). Port the client layout and settle the wire in the capture
(U-A1). `OpenPortal` leaves the position zeroed [R].

## 5. Unit fields used by the portal paths

`unit` is the `ClientUnit` returned by `FUN_39360020(id)` (`GameClient+0x10C` is the local unit).

| offset | type | field | evidence |
|---|---|---|---|
| `+0x08` | `u32` | unit object id | `FUN_394CB4B0` passes it as the teleporter NPC id; `InitFromNetUnit` logs `*param_2` |
| `+0x410` | `i32` | unit kind: 1 = NPC, 5 = mate/vehicle etc. | tooltip, interaction, auto-use |
| `+0x418` | `u32` | generic per-kind id; passed to `CSUsePortal` as `tid` | `FUN_3930CFF0`; semantics U-B1 |
| `+0x41C` | `u32` | NPC template id (3891/5476/6629) | `FUN_394C1CA0`, `FUN_394C0680`, `FUN_3930CFF0` |
| `+0x428` | char[] | portal owner name | `FUN_394C1CA0` (tooltip `portal_owner`) |
| `+0x3AC4` | `u32` | navi saved position x (or packed) | `FUN_393B1250/12B0` |
| `+0x3AC8` | `u32` | navi saved position y | same |
| `+0x3C26` | `u8` | dead / cannot-resurrect flag | `FUN_39186E70` |
| `+0x1AA0` | `u32` | current target id | `FUN_394CAEE0`, `FUN_394CB4B0` |
| `+0x3BE8` | object* | movement controller (speed at `+0x54`) | `FUN_3930B5B0` |

Portal NPC templates: **3891** entrance, **5476** guide-gate entrance, **6629** exit; the server
emulator also uses **6949** (exit), which the client does not include in its `is_portal` set [R:
`0x39AC9A78/7C/80` vs `compact.sqlite3.npcs`]. The auto-use range source (`FUN_3989AC80`) is still open (U-B2);
`DAT_39AC7304` = 9.0f is used by the tracker distance in `ClientCharacter::Update`, not by the
portal pass [R].

## 6. Global / GameClient fields

| global / offset | type | meaning | evidence |
|---|---|---|---|
| `DAT_39B02294` (`0x39B02294`) | `GameClient*` | client singleton | everywhere |
| `GameClient+0x40` | `ClientCharacter*` | character data | handlers |
| `GameClient+0x10C` | `ClientUnit*` | local unit | handlers |
| `GameClient+0x109A` | `u8` | teleport in progress (set by `SCTeleportUnit`) | `0x391D6ED0`, `FUN_3930CFF0` |
| `GameClient+0x109C` | `u32` | teleport/reuse timestamp | `0x391D6ED0`, `FUN_391827F0` |
| `GameClient+0x10A0` | `u8` | position-set disabled until `CSTeleportEnded` | `0x391D6ED0`, `FUN_391A4E00` |
| `GameClient+0x10A4` | `u32` | last teleport reason (0xD default) | `0x391D6ED0` |
| `GameClient+0x110` | `u32` | current zone id (instance load) | `FUN_3936F1C0` |
| `GameClient+0x118` | struct | local position/orientation used by `FUN_3961DE30` | `0x391D3AD0` [?] |
| `DAT_3A169998` (`0x3A169998`) | list | nearby-unit list used by auto-use | `FUN_39354210`, `FUN_3930CFF0` |
| `DAT_3A16999C` (`0x3A16999C`) | map | unit map by id/zone | `FUN_39367220` |
| `DAT_3A17D7A8/A4` | `u32` | saved navi position comparison values | `FUN_393B1250/12B0` |
| `DAT_39AC7770` | `u32` | next map-marker index | `FUN_393B0C10` |
| `DAT_3A17E6A0` | `i32` | current interaction type (6 = portal) | `FUN_393B7B50` |

## 7. Death / resurrection (reused, for reference)

The full layout and logic are in `re-combat.md` §2.9 and `port-combat/MAP.md`. The portal/return
system only reads:

```c
// ClientCharacter
u32 death_time_ms;       // +0xAF4
u32 resurrect_wait_ms;   // +0xAF8
u8  resurrection_offered;// +0xAFC
```

`GetResurrectionCountdown` (`FUN_39495C00`) returns three Lua values: the `+0xAFC` flag, the
remaining time to a fixed 300,000 ms deadline, and the remaining `resurrect_wait_ms`; both
remaining values are floats and milliseconds (`re-combat.md` §2.9). `CSResurrectCharacter` is the
only client choice (`inPlace`).

## 8. Rust module shape (suggested)

```rust
pub struct PortalBook {
    pub private: Vec<PortalEntry>,   // +0xB00
    pub districts: Vec<PortalEntry>, // +0xB10
    pub bound_return_point: u32,     // +0x608
    pub resurrection_district: u32,  // +0x60C
    pub last_auto_use_ms: u32,       // +0xB20
}

pub enum PortalType { District, Private } // Lua strings "return" / "portal"

pub enum PortalRequest {
    Save { name: String },
    Rename { id: u32, name: String },
    Open { ty: PortalType, id: u32 },
    Delete { ty: PortalType, id: u32 },
}
```

Keep `PortalEntry` plain data (no Lua values); the row builder is a pure function
`entry -> PortalRow` plus the zone/world name lookup, which makes it directly difftestable.
