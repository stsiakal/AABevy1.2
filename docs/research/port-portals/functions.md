# Portal / teleport / return-point / navigation function inventory (1.2 x2game)

Companion to [`MAP.md`](MAP.md). Addresses are x2game VAs at base `0x38FF0000`; sizes are from
`funcs.csv` in bytes. `C:` = relevant callers, `E:` = relevant callees (from `calls.tsv`,
de-noised). Purity: **pure** = output is a function of the listed inputs/fields only; **state** =
reads/writes live game state or sends/creates engine objects. Evidence markers in the role column:
**[R]** read from the decompile/DB, **[I]** inference, **[?]** open.

`ClientCharacter` = `GameClient+0x40`; `GameClient` global = `DAT_39b02294` (VA `0x39B02294`).
`PortalEntry` = 0x9C bytes ([`structs.md`](structs.md) §2).

## A. Packet handlers (S→C)

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391D3760` | 118 | `OnCharacterPortals` (0x05B): append `count` rows to the private list **[R]** | descriptor `0x39B5C998` | 3930FA50 | pkt `+0xC` count, `+0x10` rows; `cc+0xB00` | state |
| `0x391D37E0` | 125 | `OnCharacterReturnDistricts` (0x05C): replace district list + bound id **[R]** | descriptor `0x39B5DA68` | 3930FD40 | pkt `+0xC`, `+0x10`, `+0x3D00`; `cc+0xB10/+0x608` | state |
| `0x391D3860` | 114 | `OnPortalSaved` (0x05D): add/update one private row **[R]** | descriptor `0x39B5EB38` | 3930FC80 | pkt `+0xC` row; `cc+0xB00` | state |
| `0x391D38E0` | 144 | `OnPortalDeleted` (0x05E): type 2 private / type 1 district **[R]** | descriptor `0x39B5FC08` | 3930F620, 3930F6B0 | pkt `+0xC` type, `+0x10` id | state |
| `0x391D3970` | 122 | `OnUnitPortalUsed` (0x060): effect 57 for 5 s on the unit **[R]** | descriptor `0x39B60CE0` | 39360020, 393558E0 | pkt `+0xC` unitId | state |
| `0x391D4240` | 369 | `OnCharacterBound` (0x049): new return district, bound id, SYSMSG 0x60 **[R]** | descriptor `0x39B5A778`, factory 0x391A9DD0 | 3930FD80, 39355820, 392E3420, 39360020 | pkt `+0xC` unit, `+0x10` returnDistrict, `+0x14` resDistrict, `+0x18` changed, `+0x1C` PortalWire; `cc+0x608/+0x60C` | state |

## B. Packet factories / readers

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391CE860` | 125 | 0x05B factory: 0x1E88-byte packet object **[R]** | SC table | 397B3120 | opcode `[1]=0x5B` | state (alloc) |
| `0x391A9F30` | 125 | 0x05C factory: 0x3D04-byte object **[R]** | SC table | 397B3170 | `[1]=0x5C` | state (alloc) |
| `0x391AA020` | 125 | 0x05D factory: 0xA8-byte object **[R]** | SC table | 397B6220 | `[1]=0x5D` | state (alloc) |
| `0x391AA100` | 122 | 0x05E factory: 0x14-byte object **[R]** | SC table | 397B6240 | `[1]=0x5E` | state (alloc) |
| `0x391AA180` | 122 | 0x060 factory: 0x10-byte object **[R]** | SC table | 397C6F70 | `[1]=0x60` | state (alloc) |
| `0x397B1E30` | 180 | **PortalWire read/write**: `id`, `name` (0x80 buffer), `zoneId`, `pos`, `zRot` **[R]** | 397B3120/3170/6220, 397C9FD0 | — | entry `+0/+4/+0x88/+0x8C/+0x98` | state (serializer) |
| `0x397B3120` | 67 | 0x05B reader: `count`, then N `PortalWire` **[R]** | 391CE860 | 397B1E30 | pkt `+0xC`, rows `+0x10` | state |
| `0x397B3170` | 106 | 0x05C reader: `count`, N `PortalWire`, `returnDistrictId` **[R]** | 391A9F30 | 397B1E30 | pkt `+0xC`, `+0x10`, `+0x3D00` | state |
| `0x397B6220` | 19 | 0x05D reader: one `PortalWire` **[R]** | 391AA020 | 397B1E30 | pkt `+0xC` | state |
| `0x397B6240` | 101 | 0x05E reader: `portalType` (u8) + `id` (u32) **[R]** | 391AA100 | — | pkt `+0xC/+0x10` | state |
| `0x397C6F70` | 26 | 0x060 reader: `unitId` **[R]** | 391AA180 | 397B27D0 | pkt `+0xC` | state |
| `0x397C9FB0` | 19 | 0x041 reader wrapper -> `FUN_397C85B0` (SkillCaster) **[R]** | 391A98D0 | 397C85B0 | pkt `+0xC` caster | state |
| `0x397C9FD0` | 169 | 0x049 reader: `unitId`, `returnDistrict`, `resurrectionDistrict`, `returnDistrictChanged`, `PortalWire` **[R]** | 391A9DD0/9E50 | 397B1E30, 397B27D0 | pkt `+0xC/+0x10/+0x14/+0x18/+0x1C` | state |

## C. Portal book state (ClientCharacter)

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x3930FA50` | 47 | append `count` rows to private vector **[R]** | 391D3760 | 3930F450 | `cc+0xB00/+0xB04/+0xB08` | state |
| `0x3930FD40` | 51 | replace district vector + set bound id **[R]** | 391D37E0 | 3930F880 | `cc+0xB10..+0xB18`, `+0x608` | state |
| `0x3930FC80` | 186 | add-or-update private row; event 0xE5 (add) / 0xE7 (rename) **[R]** | 391D3860 | 3930F7F0, 39424220 | `cc+0xB00`; entries by `+0` id | state |
| `0x3930FD80` | 160 | add-or-update a district row (SCCharacterBound) **[R]** | 391D4240 | 3930F7F0 | `cc+0xB10` | state |
| `0x3930F620` | 51 | delete private row by id; event 0xE6 **[R]** | 391D38E0, 3938FC90 | — | `cc+0xB00`, row `+0` | state |
| `0x3930F6B0` | 193 | delete district row; refuses the bound id (error 0x19B), event 0xE6 **[R]** | 391D38E0, 3938FC90 | 39428370, 39424220 | `cc+0xB10`, `+0x608` | state |
| `0x3930F880` | 93 | clear + insert a range into the district vector **[R]** | 3930FD40 | 3930F450 | vector | state |
| `0x3930CFB0` | 31 | `GetPortalList` body: row table from private list **[R]** | 3938FC00 | 3930CB90 | `cc+0xB00`, type `"portal"` | state |
| `0x3930CFD0` | 31 | `GetReturnList` body: row table from district list **[R]** | 3938FC60 | 3930CB90 | `cc+0xB10`, type `"return"` | state |
| `0x3930CB90` | 1047 | **Lua row builder**: index, id, type string, zone/world names, name, x/y/z **[R]** | 3930CFB0/CFD0 | 3921A9F0, 395779F0, 397F2180, 397F23D0 | entry `+0/+4/+0x88/+0x8C/+0x90/+0x94`; Lua table | state |
| `0x3930B410` | 34 | private count `(end-begin)/0x9C` **[R]** | 3938FBD0 | — | `cc+0xB00/+0xB04` | **pure** |
| `0x3930B480` | 34 | district count `(end-begin)/0x9C` **[R]** | 3938FC30 | — | `cc+0xB10/+0xB14` | **pure** |

## D. `X2Warp` Lua binding and request builders

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x394CB5E0` | 1234 | registers 12 functions on the `X2Warp` object (name table in `MAP.md` §1.1) **[R]** | Lua init | — | strings `GetPortalList`, `OpenPortal`, … | state |
| `0x394CACE0` | 47 | `SavePortal(memo)`: `XlIsValidName(memo,5)` then request **[R]** | X2Warp | 3938F7D0 | string | state |
| `0x394CAD10` | 33 | `RenamePortal(portalId,name)` **[R]** | X2Warp | 3938F950 | int, string | state |
| `0x394CAD40` | 143 | `OpenPortal(portalTypeStr,portalId)`: `"portal"`→2, `"return"`→1, else log **[R]** | X2Warp | 3938FAA0 | strings at `0x399E52A4/…` | state |
| `0x394CADF0` | 85 | `DeletePortal(portalType,portalId)`: same mapping **[R]** | X2Warp | 3938FC90 | int | state |
| `0x394CADD0` | 27 | `EndPortalInteraction` -> `FUN_393B7B50(6)` **[R]** | X2Warp | 393B7B50 | interaction type 6 | state |
| `0x394CAEE0` | 131 | `IsTeleporterNpc`: current target NPC desc `+0xE5` `teleporter` **[R]** | X2Warp | 394FB310 | target unit `+0x410/+0x41C` | state |
| `0x394CAF70` | 121 | `GetPortalListCount` **[R]** | X2Warp | 3938FBD0 | — | state |
| `0x394CAFF0` | 167 | `GetPortalList` returns the Lua table **[R]** | X2Warp | 3938FC00, 39058B50 | — | state |
| `0x394CB0A0` | 121 | `GetReturnListCount` **[R]** | X2Warp | 3938FC30 | — | state |
| `0x394CB120` | 171 | `GetReturnList` returns the Lua table **[R]** | X2Warp | 3938FC60, 39058B50 | — | state |
| `0x394CB1D0` | 121 | `GetBoundPortalId` **[R]** | X2Warp | 3938FD20 | `cc+0x608` | state |
| `0x394CB4B0` | 299 | `UseTeleport(id,cost)`: target NPC teleporter, money check, CS 0x0B3 **[R]** | X2Warp | 394FB310, 39360020, 396962C0, 397B0FB0 | target `+0x410/+0x41C/+8`; desc `+0xE5`; `DAT_3A5AB340/33C` | state |
| `0x3938F7D0` | 370 | **SavePortal request**: item 4045 lookup, `CSStartSkill` object type 2 `{id=0,name}` **[R]** | 394CACE0 | 394E0960, 397CD0A0, 39428370 | inv `cc+0x4B8`; name len 0x81; error 0x240 | state |
| `0x3938F950` | 325 | **RenamePortal request**: type 2 `{id,name}`; same item requirement **[R]** | 394CAD10 | 394E0960, 397CD0A0 | `cc+0x4B8` | state |
| `0x3938FAA0` | 295 | **OpenPortal request**: type 1 `{portalType,id, position zeroed}` **[R]** | 394CAD40 | 394E0960, 397CD0A0 | `cc+0x4B8`; skill object 0xD0 | state |
| `0x3938FC90` | 132 | **DeletePortal request**: local remove then CS 0x0DF `{type,id}` **[R]** | 394CADF0 | 3930F620/F6B0, 397AC7D0, 39186810 | `cc+0x608` guard | state |
| `0x3938FBD0` | 34 | private count wrapper (checks GameClient) **[R]** | 394CAF70 | 3930B410 | — | state |
| `0x3938FC00` | 42 | private row-table wrapper **[R]** | 394CAFF0 | 3930CFB0 | — | state |
| `0x3938FC30` | 34 | district count wrapper **[R]** | 394CB0A0 | 3930B480 | — | state |
| `0x3938FC60` | 42 | district row-table wrapper **[R]** | 394CB120 | 3930CFD0 | — | state |
| `0x3938FD20` | 36 | bound id getter (`cc+0x608`) **[R]** | 394CB1D0 | — | — | **pure** (read) |

## E. Book trigger and name policy

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x394EAAC0` | 1882 | **item-use dispatcher** *(inferred name)* on `itemDesc+0x48` (item `impl_id` **[I]**); case 14 = Teleport Book -> event 0xE4 **[R]** | 394EB270 | 3902DB50, 3904C760, 39053360/3F0, 39695DC0, 39424220 | item desc `+0x48`; local player | state |
| `0x394EB270` | 728 | UseItem core: resolves item/skill, calls the dispatcher **[R]** | many item paths | 394EAAC0, 39695DC0, 39428370 | item/unit/inventory | state |
| `0x394C9B40` | 4509 | X2Edit name-policy registration incl. `VNT_PORTAL` **[R]** | Lua init | — | `VNT_PORTAL` string `0x399E4DE0` | state |

## F. World portals, auto-use and presentation

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x3930E7E0` | 636 | `X2::GameClient::ClientCharacter::Update`; per-frame, ends in auto-use **[R]** | engine tick | 3930CFF0, 39360020, 39035110 | profiler string at entry | state |
| `0x3930CFF0` | 497 | **auto-use nearby portal gate** *(inferred name)*: 1 s scan, range, buff/10 s gates, CS 0x0DE **[R]** | 3930E7E0 | 3930B5B0, 391827F0, 397AC7B0, 39354210, 3989AC80 | `cc+0xB20`; `GameClient+0x109A/+0x109C`; unit `+0x410/+0x41C/+0x418`; buffs 1128/1910 | state |
| `0x3930B5B0` | 77 | auto-use precondition: per-unit var 6 or speed > 0.2 **[R/I]** | 3930CFF0 | 39424580, 390B3750 | mover `+0x54` | state |
| `0x393558E0` | 239 | **SCUnitPortalUsed effect** 57, 5.0 s, at the unit transform **[R]** | 391D3970 | 393F1160, 393EEE10/80, 393EEFA0 | `DAT_39AC7460`=57, `0x3999CCF4`=5.0 | state |
| `0x394C1CA0` | 527 | unit tooltip: `is_portal`, `portal_owner` for 3891/5476/6629 **[R]** | 394C42B0 | 396962C0, 3976AD60, 39570670/740 | unit `+0x410/+0x41C/+0x428`; ids `0x39AC9A78/7C/80` | state |
| `0x394C0680` | 307 | unit type string: `"portal"` for the same ids **[R]** | 394BBBF0-family | 3956DB10, 39265D60 | unit `+0x410/+0x41C` | state |

## G. Teleport transition / arrival (portal part)

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391DE970` | 123 | `OnLoadInstance` (0x197): `iid, zoneId, pos, ang` -> core **[R]** | descriptor | 3936F1C0 | pkt `+0xC/+0x10/+0x14/+0x20` | state |
| `0x3936F1C0` | 331 | instance/zone transition core *(inferred name)*: zone data, clear units, CSInstanceLoaded 0x0E0, CSNotifySubZone 0x112 **[R]** | 391DE970 | 39367220, 3901B9E0, 391EF490, 39186810 | `GameClient+0x110`; `DAT_39AEFB98+0xF4/F8` | state |
| `0x39367220` | 160 | clear/destroy units not in the current zone map **[R]** | 39171800, 3936F1C0 | 39366480 | unit map `DAT_3A16999C` | state |
| `0x39187410` | 196 | loading-screen manager *(inferred name)*: mode 0 + progress<1 → art; event 0x45 STILL_LOADING **[R]** | world-manager vtable [?] | 3942F770, 3921A7E0, 39370E60 | mode, progress | state |
| `0x3942F770` | 987 | **loading art**: `worlds/<world>/loading.dds` else `ui/loading.dds`; fires ENTERED_LOADING 0x4D **[R]** | 39187410, 390F5800, 3942FE60 | 3920FA60/FCF0, 39415D60 | world name, `loading.dds` | state |
| `0x397AC7B0` | 19 | `CSUsePortal` body: `+0xC`=id, `+0x10`=byte flag **[R]** | 3930CFF0 | — | — | **pure** (write) |
| `0x397AC7D0` | 19 | `CSDeletePortal` body: `+0xC`=type, `+0x10`=id **[R]** | 3938FC90 | — | — | **pure** (write) |
| `0x397AC8A0` | 13 | `CSNaviTeleport` body: `+0xC`=doodadId **[R]** | 393B12B0 | — | — | **pure** (write) |
| `0x397AC8B0` | 13 | `CSNaviOpenPortal` body: `+0xC`=doodadId **[R]** | 393B1250 | — | — | **pure** (write) |
| `0x397AC8C0` | 13 | `CS 0x0EA` body: `+0xC`=doodadId **[R]** | 393B1140 | — | — | **pure** (write) |
| `0x397B0FB0` | 43 | `CSUseTeleport` body: `+0xC`=returnPointId, `+0x10`=money, `+0x14`=npcUnitId **[R]** | 394CB4B0 | — | — | **pure** (write) |
| `0x397C2A50` | 133 | `CSUsePortal` serializer: `tid`(u32), `type`(u32), `role`(u8) **[R]** | 0x0DE class | — | `+0xC/+0x10/+0x14` | state |
| `0x397AEF40` | 30 | `CSDeletePortal` serializer: `tid`(u32) **[R]** | 0x0DF class | — | `+0xC` | state |
| `0x397AFB40` | 95 | `SCLoadInstance` reader: `iid`, `zoneId`, `pos`, `ang` **[R]** | 0x197 factory | — | `+0xC/+0x10/+0x14/+0x20` | state |

## H. Navigation: navi doodads and map pins

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391DC8C0` | 138 | `OnNaviTeleport` (0x0E7): `FUN_3930C9E0(0,0,pos,0)` **[R]** | descriptor | 3930C9E0 | pkt `+0x10` pos | state |
| `0x391DC5A0` | 246 | `OnAboxTeleport`: corner/box teleport, `pos = camera + axis*-1024`, `z = water/ground + 10.0` **[R]** | descriptor | client vtable `+0x44` | `+0xC/+0x10` ints; `0x3999CCE8`=10.0 | state |
| `0x393B1250` | 95 | send `CSNaviOpenPortal` 0x0E8 if the player is at the saved navi spot **[R]** | interaction | 397AC8B0, 39186810 | player `+0x3AC4/+0x3AC8`; `DAT_3A17D7A8/A4`; doodad `+0x7C` | state |
| `0x393B12B0` | 95 | send `CSNaviTeleport` 0x0E7 under the same gate **[R]** | interaction | 397AC8A0, 39186810 | same | state |
| `0x393B0C10` | 941 | **NaviMarkPosToMap**: func type `0x2D`, fire event 0x127 `{index,x,y,z}` **[R]** | 393B1100, 393A1330, 3902E1F0 | 3963B250, 394242A0, 3961DE30 | doodad func row `+8/+0xC`; `DAT_39AC7770` marker index | state |
| `0x393B0FC0` | 126 | fire `OPEN_NAVI_DOODAD_NAMING_DIALOG` 0x129 `{doodadId}` **[R]** | interaction | 39424240 | doodad `+0x7C` | state |
| `0x393B1040` | 190 | **NaviOpenMailbox**: fire `TOGGLE_MAIL` 0xCE `{1,doodadId}`, end interaction **[R]** | interaction | 393B7B10, 39424260 | doodad `+0x7C` | state |
| `0x393B1100` | 50 | adapter: current func + quest row -> `FUN_393B0C10` **[R]** | interaction | 393B0C10 | doodad `+0x7E/+0x7F` | state |
| `0x393B1140` | 56 | send `CS 0x0EA {doodadId}` (NaviRemove) **[I]** | interaction | 397AC8C0, 39186810 | doodad `+0x7C` | state |
| `0x39673B90` | 4501 | doodad-func descriptor loader (`doodad_funcs`, all polymorphic types incl. Navi*) **[R]** | DB loader | 3955B3D0, 3962Bxxx | DB tables; desc type at `+8` | state |
| `0x39674D30` | 2120 | doodad-func finals/loaders (incl. NaviRemoveTimer, RatioRespawn) **[R]** | DB loader | 3955B3D0 | DB tables | state |
| `0x397A5780` | 427 | `doodad_func_navi_mark_pos_to_maps` loader (`id,x,y,z`) **[R]** | 3960B820 | 3978D7B0, 3979ACD0 | desc `+0..+0xC` | state |
| `0x397A5D90` | 348 | `doodad_func_navi_open_portals` loader (`id`) **[R]** | 3960B820 | 3978DD60, 3979B340 | — | state |
| `0x397A5EF0` | 348 | `doodad_func_navi_teleports` loader (`id`) **[R]** | 3960B820 | 3978DEC0, 3979B4D0 | — | state |
| `0x397A54A0` | 348 | `doodad_func_navi_namings` loader (`id`) **[R]** | 3960B820 | 3978D4E0, 3979A9A0 | — | state |
| `0x397A5600` | 380 | `doodad_func_navi_open_mailboxes` loader (`id,duration`) **[R]** | 3960B820 | 3978D640, 3979AB30 | — | state |
| `0x397A5A90` | 380 | `doodad_func_navi_remove_timers` loader (`id,after`) **[R]** | 3960B820 | 3978DA80, 3979B000 | — | state |
| `0x397A5C10` | 380 | `doodad_func_navi_removes` loader (`id,req_lp`) **[R]** | 3960B820 | 3978DBF0, 3979B1A0 | — | state |
| `0x397A5930` | 348 | `doodad_func_navi_donations` loader (`id`) **[R]** | 3960B820 | 3978D920, 3979AE70 | — | state |
| `0x397A6050` | 348 | `doodad_func_navi_open_bounties` loader (`id`) **[R]** | 3960B820 | 3978E020, 3979B660 | — | state |
| `0x3904CE90` | 6834 | minimap marker table registration; index `0x45` = `M_PORTAL` type 0x0B **[R]** | 39051DE0 | 3904AA80, 3904C690 | table `0x39AC…` | state |

## I. Navigation: quest marks / arrows

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x39359C60` | 547 | `ClientUnit::UpdateQuestMark`: pick prefab mark, bind to unit **[R]** | 39359E90, 3936A0B0 | 393829E0, 393B4780/4930, 390B3AD0 | unit `+0x5FA4/+0x5FA8/+0x5FB0/+0x3C6C/+0x3D7E` | state |
| `0x39383E20` | 281 | `UpdateQuestContextMarks`: refresh the mark set **[R]** | 391739E0, 391D9F30, 391DEA60 | 393822F0, 393838A0, 39383D80 | quest context marks | state |
| `0x393A2C10` | 488 | `ClientDoodad::UpdateQuestMark` **[R]** | 393A4810 | 393829E0, 393B45C0, 393ED520 | doodad mark fields | state |
| `0x39382790` | 587 | prefab-mark resource loader (`prefab_model_marks_%s`) **[R]** | 393829E0/2A70 | 390FE170, 390FF600/9F0/A60, 39108DE0 | prefab path strings `0x399D13C8…` | state |
| `0x3901EB30` | 793 | prologue quest arrow model (`objects/ui/quest_mark/prologue_arrow`) **[R]** | 3901EE50 | engine render | arrow transform | state |

## J. Reused (do not re-map)

These are already covered by `port-movement-extra` or `port-combat`; listed for the packet flow only.

| addr | size | where |
|---|---|---|
| `0x3930C9E0` | 378 | movement/camera reset — `port-movement-extra/click-move-jump-teleport.md` §6.2 |
| `0x391D6ED0` | 370 | `SCTeleportUnit` — same §6.1 |
| `0x391D39F0` | 215 | `SCNotifyResurrection` — `port-combat` / `re-combat.md` §2.9 |
| `0x391D3AD0` | 399 | `SCCharacterResurrected` — same |
| `0x391D51C0` | 165 | `OnUnitDeath` — same |
| `0x39369C10` | 1183 | local death state — same |
| `0x39395AE0` | 106 | death packet gate — same |
| `0x39499170` | 4875 | `X2Player` registration — same |
| `0x39495AA0` | 117 | `IsPlayerAimming` — same |
| `0x39495B20` | 222 | `IsInIndunWithGraveyard` — same |
| `0x39495C00` | 259 | `GetResurrectionCountdown` — same |
| `0x394952B0` | 50 | `Resurrect(inPlace)` — same |
| `0x39186E70` | 198 | `CSResurrectCharacter` 0x04E — same |
| `0x39495680` | 214 | three-value Lua return — same |
| `0x397C85B0` | 431 | `SCNotifyResurrection` SkillCaster reader — same |
| `0x397C5C20` | 63 | `SCCharacterResurrected` reader — same |

**Counts.** 102 new entries / 41,238 B; 16 reused entries / 9,245 B; total 118 / 50,483 B.
