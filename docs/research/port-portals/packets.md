# Portal / teleport / return-point / navigation packets (1.2 client)

Companion to [`MAP.md`](MAP.md). All packet-object offsets are byte offsets from the start of the
client's packet object (`param_2` in the handlers), because the network layer writes fields there
before the handler runs. `[R]` = read from the client reader/writer, `[I]` = inferred.

Opcode numbers below are the client's own class constants (`piVar4[1] = 0xNN` in the packet
factories at `0x391A9xxx` / `0x391CExxx` / `0x391B4xxx`). Directions overlap in ArcheAge: the same
number can appear in the C→S and S→C tables, so each row states the direction.

## 1. S→C handlers used by the portal system

| opcode | packet | handler | reader | packet fields (offset) |
|---|---|---|---|---|
| `0x05B` | `SCCharacterPortals` | `0x391D3760` | `0x397B3120` -> `0x397B1E30` | `+0xC` i32 count; `+0x10` N × `PortalWire` (0x9C each) |
| `0x05C` | `SCCharacterReturnDistricts` | `0x391D37E0` | `0x397B3170` -> `0x397B1E30` | `+0xC` count; `+0x10` rows; `+0x3D00` u32 `returnDistrictId` |
| `0x05D` | `SCPortalInfoSaved` | `0x391D3860` | `0x397B6220` -> `0x397B1E30` | `+0xC` one `PortalWire` |
| `0x05E` | `SCPortalDeleted` | `0x391D38E0` | `0x397B6240` | `+0xC` u8 `portalType` (1 district, 2 private); `+0x10` u32 id |
| `0x060` | `SCUnitPortalUsed` | `0x391D3970` | `0x397C6F70` | `+0xC` u32 `unitId` |
| `0x049` | `SCCharacterBound` | `0x391D4240` | `0x397C9FD0` | `+0xC` u32 `unitId`; `+0x10` u32 `returnDistrict`; `+0x14` u32 `resurrectionDistrict`; `+0x18` u8 `returnDistrictChanged`; `+0x1C` `PortalWire` |
| `0x041` | `SCNotifyResurrection` | `0x391D39F0` | `0x397C85B0` | `+0xC` u8 caster type; body per type (`+0x10`..): type 0/3 units, type 2 item/skill; handler uses `+0x18` (type 0/3) or `+0x24` (type 2) |
| `0x042` | `SCCharacterResurrected` | `0x391D3AD0` | `0x397C5C20` | `+0xC` u32 `unitId`; `+0x10` position (12 B); `+0x28` f32 `zRot` |
| `0x072` | `SCTeleportUnit` | `0x391D6ED0` | — | `+0xC` u32 `type`; `+0x10` u32 `objId`; `+0x18` position (12 B); `+0x30` u32 yaw (mapped in `port-movement-extra` §6.1) |
| `0x0E7` | `SCNaviTeleport` | `0x391DC8C0` | `0x397AEFA0` | `hasPing` u8; `pos` 12 B; `insId` u32 (reader `0x397AEFA0`: `hasPing`, `pos`, `insId`) |
| `0x197` | `SCLoadInstance` | `0x391DE970` | `0x397AFB40` | `+0xC` u32 `iid`; `+0x10` u32 `zoneId`; `+0x14` position (12 B); `+0x20` angles (12 B) |
| `0x1A2` | `SCAboxTeleport` | `0x391DC5A0` | `0x397AF670` | `+0xC` i32 x; `+0x10` i32 y (both relative, scaled by -0x400) |

`0x071 SCUnitDeath` (handler `0x391D51C0`) is reused from `port-combat`; only its `killReason 14`
(portal timeout) is portal-relevant.

### 1.1 `PortalWire` layout (proven)

`FUN_397B1E30` reads/writes:

```
+0x00  u32   id
+0x04  char  name[0x80]          // max 0x80 incl. terminator; padded to +0x88
+0x88  u32   zoneId
+0x8C  f32   x
+0x90  f32   y
+0x94  f32   z
+0x98  f32   zRot
------- 0x9C bytes
```

Evidence: the reader uses the field-name strings `id` (`0x3999D470`), `name` (`0x3999DA20`, max
`0x80`), `zoneId` (`0x39A12DC8`-family), `pos` (`0x399B1FE0`, 12 bytes), `zRot` (`0x399F9C58`);
`0x397B3120`/`0x397B3170` then copy the rows into the packet array at `+0x10`, and the handlers
copy the same 0x9C rows into the ClientCharacter vectors. This matches the server's `PortalWire`
(`portals.md` §2.2) field for field.

The string branch in `FUN_397B1E30`: `vtable+0x1C` selects a length-prefixed encoding
(`vtable+0xDC`, `XlStringSize`) or a fixed 0x80-byte buffer (`vtable+0xD8`). The 1.2 client
accepts both; the emulator writes the length-prefixed form. The exact selector
(`protocol/compression flag`) is U-A2 in `difftest.md`.

## 2. C→S builders / serializers

| opcode | request | builder (payload) | serializer | payload fields |
|---|---|---|---|---|
| `0x0DE` | `CSUsePortal` | `0x397AC7B0(id, flag)` at `0x3930D0B1` | `0x397C2A50` | `tid` u32 (+0xC), `type` u32 (+0x10), `role` u8 (+0x14) **[R]** |
| `0x0DF` | `CSDeletePortal` | `0x397AC7D0(type, id)` at `0x3938FC90` | `0x397AEF40` | `tid` u32 (+0xC) **[R]**; the builder also stores `+0x10 = id` |
| `0x0B3` | `CSUseTeleport` | `0x397B0FB0(returnPointId, money, npcUnitId)` at `0x394CB4B0` | class `0x391AC0A0` | `+0xC` u32 returnPointId; `+0x10` u32 money; `+0x14` u32 npcUnitId (server doc agrees) |
| `0x0E7` | `CSNaviTeleport` | `0x397AC8A0(doodadId)` at `0x393B12B0` | class `0x391ADAE0` | `+0xC` u32 doodadId |
| `0x0E8` | `CSNaviOpenPortal` | `0x397AC8B0(doodadId)` at `0x393B1250` | class `0x391ADBB0` | `+0xC` u32 doodadId |
| `0x0EA` | navi remove **[I]** | `0x397AC8C0(doodadId)` at `0x393B1140` | — | `+0xC` u32 doodadId |
| `0x04E` | `CSResurrectCharacter` | `0x39186E70(inPlace)` | — | `inPlace` bool (reused from `port-combat`) |
| `0x0E0` | `CSInstanceLoaded` | `0x397B47C0` at `0x3936F1C0` | class `0x391AD830` | empty/ack |
| `0x112` | `CSNotifySubZone` | `0x397ACB80(zoneId)` at `0x3936F1C0` | class `0x391AF820` | u32 sub-zone id |
| `0x0B4` | `CSTeleportEnded` | `0x391A4E00` (movement-extra) | `0x397AF370` | position + orientation |

**U-A2.** The `CSUsePortal` serializer writes three fields while the emulator documents two
(`objId`, `onlyMyPortal`). Either 1.2's `type` carries the safe-mode flag and `role` is a constant,
or the emulator's layout is short. Capture a real packet and compare with the server parse before
porting the wire; the native builder currently sets only `id` and a byte flag.

**U-A3.** The `CSDeletePortal` serializer writes only `tid`; the builder stores `type` at `+0xC` and
`id` at `+0x10` (`0x397AC7D0`), so the serializer's `tid` is the **type** at `+0xC` and the id is not
visible in this writer. Verify with a capture of both a private and a district delete.

## 3. `CSStartSkill` skill objects used by the book

`FUN_3938FAA0` (Open), `FUN_3938F7D0` (Save), `FUN_3938F950` (Rename) build a stack block and send
it through the normal `CSStartSkill` path. The block observed in all three [R]:

```
+0x00  u32   unknown = 0
+0x04  u32   object type: 1 = PortalInfo, 2 = SavePortalInfo
        type 1 body: u32 portalType (1/2), u32 portalId, then position fields (all zero in
                     FUN_3938FAA0); the remaining 0xD0 bytes are zeroed
        type 2 body: u32 id (0 for save, the portal id for rename), char name[0x81]
```

`OpenPortal` passes no position: `FUN_3938FAA0` only stores `portalType` and `portalId`
(U-A1). The emulator's `PortalInfo` is `kind u8, id i32, x i64, y i64, z f32`; the client's
in-memory layout is 4-byte aligned. Port the **client** layout, then confirm the wire through
`0x3AC1058C`.

## 4. Opcode/class anchors

The factory classes are the ones the client's packet table instantiates; the serializer at vtable
slot 2 is written to serve both directions (the `vtable+0x1C` branch inside it selects read/write),
so a class does not by itself prove that an opcode is C→S or S→C. The send sites in §2 are the
proof for the C→S payloads; the reader fields in §1 are the proof for S→C.

| class / factory | opcode | vtable | note |
|---|---|---|---|
| `0x391CE860` | 0x05B | `0x399B73A4` | alloc 0x1E88 |
| `0x391A9F30` | 0x05C | `0x399B5B80` | alloc 0x3D04 |
| `0x391AA020` | 0x05D | `0x399B5F28` | alloc 0xA8 |
| `0x391AA100` | 0x05E | `0x399B3B34` | alloc 0x14 |
| `0x391AA180` | 0x060 | `0x399B3B40` | alloc 0x10 |
| `0x391A9DD0` | 0x049 | `0x399B5BA4` | SCCharacterBound, alloc 0xB8 |
| `0x391A98D0` | 0x041 | `0x399B3210` | SCNotifyResurrection, alloc 0x30 |
| `0x391A9950` | 0x042 | `0x399B321C` | SCCharacterResurrected, alloc 0x30 |
| `0x391AD730` | 0x0DE | `0x399B360C` | CSUsePortal, alloc 0x18 |
| `0x391AD7B0` | 0x0DF | `0x399B3618` | CSDeletePortal, alloc 0x10 |
| `0x391AC0A0` | 0x0B3 | `0x399B399C` | CSUseTeleport, alloc 0x1C |
| `0x391ADAE0` | 0x0E7 | `0x399B5D48` | navi teleport class, alloc 0x20 |
| `0x391ADBB0` | 0x0E8 | `0x399B3660` | navi open-portal class, alloc 0x20 |
| `0x391B43D0` | 0x197 | `0x399B5FE8` | SCLoadInstance, alloc 0x2C |

## 5. Events / error messages

Event ids (registration order in `FUN_39214380`, 9391 B; `MAP.md` §1.1 has the derivation):

| id | name | fired by |
|---|---|---|
| `0x0E4` | `TOGGLE_PORTAL_DIALOG` | `FUN_394EAAC0` case 14 (item 4045) |
| `0x0E5` | `SAVE_PORTAL` | `FUN_3930FC80` when the row is new |
| `0x0E6` | `DELETE_PORTAL` | `FUN_3930F620` / `FUN_3930F6B0` |
| `0x0E7` | `RENAME_PORTAL` | `FUN_3930FC80` when the row exists |
| `0x0CE` | `TOGGLE_MAIL` | `FUN_393B1040` (NaviOpenMailbox), args `(1, doodadId)` |
| `0x127` | `NAVI_MARK_POS_TO_MAP` | `FUN_393B0C10`, args `(markerIndex, x, y, z)` |
| `0x128` | `NAVI_MARK_REMOVE` | map UI / not fired by the native paths mapped here |
| `0x129` | `OPEN_NAVI_DOODAD_NAMING_DIALOG` | `FUN_393B0FC0`, arg `doodadId` |
| `0x05C` | `PLAYER_RESURRECTION` | `0x391D39F0` (reused; `port-combat` uses id 92) |
| `0x05D` | `PLAYER_RESURRECTED` | `0x391D3AD0` (reused; id 93) |
| `0x045` | `STILL_LOADING` | `FUN_39187410` with progress |
| `0x04D` | `ENTERED_LOADING` | `FUN_3942F770` |
| `0x014` | `SYSMSG` | `0x391D4240` with text id 0x60 (`return_district_binding`) |

Error texts (`FUN_3956B860`, 5512 B) relevant to portals:

| id | text |
|---|---|
| `0x19B` | `delete_current_return_district` |
| `0x240` | `no_portal_book` |
| `0x319` | `cannot_reuse_portal_in_delay_time` |
| `0x165` | `cannot_use_portal_with_resurrection_buff` |
| `0x139` | `portal_count_exceeded` |
| `0x260` | `invalid_portal` |
| `0x261` | `not_my_portal` |
| `0x262` | `cannot_use_portal_with_backpack` |
| `0x263` | `cannot_use_portal_in_trial` |

The server-side refusals 609/610/611 (`SCErrorMsg`) arrive as normal error messages; the client
table above is the local set it can raise before a packet is sent.

## 6. `X2Warp` Lua signatures (native side)

Registration `0x394CB5E0`; the strings it passes to the Lua binder are [R]:

| name | arg string | impl |
|---|---|---|
| `IsTeleporterNpc` | — | `0x394CAEE0` |
| `UseTeleport` | `id, cost` | `0x394CB4B0` |
| `SavePortal` | `memo` | `0x394CACE0` |
| `RenamePortal` | `portalId, name` | `0x394CAD10` |
| `GetPortalListCount` | — | `0x394CAF70` |
| `GetPortalList` | — | `0x394CAFF0` |
| `GetReturnListCount` | — | `0x394CB0A0` |
| `GetReturnList` | — | `0x394CB120` |
| `OpenPortal` | `portalTypeStr, portalId, portalName` | `0x394CAD40` (name ignored) |
| `DeletePortal` | `portalType, portalId` | `0x394CADF0` |
| `GetBoundPortalId` | — | `0x394CB1D0` |
| `EndPortalInteraction` | — | `0x394CADD0` |

Row fields returned by `FUN_3930CB90`: `portal_index` (1-based), `portal_id`, `portal_type`
(string `"portal"`/`"return"`), `zone_id`, `zone_name`, `world_name`, `name`, `x`, `y`, `z`.
