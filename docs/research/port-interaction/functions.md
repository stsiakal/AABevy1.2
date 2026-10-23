# Interaction function inventory (1.2 x2game)

Companion to [`MAP.md`](MAP.md). All addresses are x2game VAs at base `0x38FF0000`; sizes are from
`funcs.csv` in bytes, `tail` marks code Ghidra sizes only as a cold fragment, `hand` marks the four
`do_interaction_*` stubs which `funcs.csv` folds into `FUN_393f81a0` (8 bytes each). `C:` = relevant
callers, `E:` = relevant callees (from `calls.tsv`, de-noised). Purity: **pure** = output is a
function of the listed inputs/fields only; **state** = reads/writes live game state and needs a live
object or a captured snapshot. Evidence markers in the role column: **[R]** read from the
decompile/data, **[I]** inference, **[?]** open.

## A. Input, action dispatch, the dynamic action bar

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x393f8290` | 8 | `do_interaction_1` (action index 31) → `FUN_393b2b70(0)` **[R]** | engine action map | 393b2b70 | — | **pure** |
| `0x393f82a0` | 8 | `do_interaction_2` (32) → `(1)` **[R]** | engine action map | 393b2b70 | — | **pure** |
| `0x393f82b0` | 8 | `do_interaction_3` (33) → `(2)` **[R]** | engine action map | 393b2b70 | — | **pure** |
| `0x393f82c0` | 8 | `do_interaction_4` (34) → `(3)` **[R]** | engine action map | 393b2b70 | — | **pure** |
| `0x393f8d20` | 70 | action name → index in `0x39ac8e68` (0x68 entries; `0x68`=not found) **[R]** | 393f8d70/8db0/8df0, 393ffc20 | `XlCompareString` | table `0x39ac8e68` | **pure** |
| `0x393f8d70` | 52 | run the per-action callback at `0x3a59f1c0+0x13c+4*index` **[R]** | `X2Hotkey:ExcuteActionHandler` | 393f8d20 | `0x3a59f1c0` callback table | state |
| `0x393b2b70` | 20 | `ExecuteDynamicAction` shim → `FUN_393b1de0` **[R]** | 0x393f8290…c0 | 393b1de0 | — | state |
| `0x393b1de0` | 202 | **bar execute**: bounds, loot gates, doodad→`FUN_393b3ec0`, unit→`FUN_393be280` **[R]** | 393b2b70, 39440850 | 39035110, 39360020, 393b3ec0, 393bc7e0, 393bc8c0, 393be280 | bar `+0x08/+0x0c/+0x20/+0x24`; sentinels `DAT_3a17d7d0/d7d4` | state |
| `0x39442370` | 131 | `X2:GetDynamicActionCount()` = `(bar+0x24-bar+0x20)/4` **[R]** | script | — | `DAT_39affa0c+0x20/+0x24` | state/read |
| `0x39442400` | 180 | `X2:GetDynamicActionSkill(idx)` (1-based) **[R]** | script | 393b1da0 | bar shown vector | state/read |
| `0x394424c0` | 298 | `X2:IsDynamicActionSkillToggled(idx)`: desc/active check **[R]** | script | 393b1da0, 396f6390, 39387610, 39058bd0 | bar vector | state/read |
| `0x39440850` | 33 | `X2:ExecuteDynamicAction(idx)` → `FUN_393b1de0(idx-1)` **[R]** | script | 393b1de0 | `DAT_39affa0c` | state |
| `0x3947e060` | 130 | `X2Interaction:IsDynamicActionBarVisible()` = `bar+3 && !bar+4` **[R]** | script | — | `DAT_39affa0c+3/+4` | state/read |
| `0x393b23d0` | 46 | bar ctor: zero `+0..+0x28`, ids = empty sentinels **[R]** | 39174520 | — | `DAT_3a17d7cc/d7d4` | state |
| `0x393b2530` | 108 | bar reset: clear raw/shown vectors, ids, state **[R]** | 393bc660, 393bd050, 393bd850, 393bdad0 | 393b1eb0 | bar `+0x08/+0x0c/+0x10/+0x14/+0x20/+0x24/+0x02..04` | state |
| `0x393b2490` | 70 | set current **unit** id + clear raw vector; show pending **[R]** | 393bd050 | — | bar `+0x08 = objId`, `bar+0x0c = empty`, `+0x02 = 1` | state |
| `0x393b24e0` | 69 | set current **doodad** id + clear raw vector; show pending **[R]** | 393bd850 | — | bar `+0x0c = objId`, `bar+0x08 = empty`, `+0x02 = 1` | state |
| `0x393b25a0` | 440 | `DynamicActionBar::RefreshActions`: rebuild raw list, compare/copy to shown, fire 0x19E if empty **[R]** | 393b2760 | 393b2230, 393bc620, 393bd1b0 | bar `+0x10/+0x14/+0x20/+0x24` | state |
| `0x393b2760` | 271 | `DynamicActionBar::Update`: RefreshActions + Show(true) **[R]** | 393bdf10 | 393b25a0, 393b1eb0 | bar `+0x02` | state |
| `0x393b1eb0` | 611 | `DynamicActionBar::Show(show)`: 0x19D with `("npc"/"doodad", id)` or 0x19E **[R]** | 393b2530, 393b2760 | 39424260, 39424220, 391f0120, 39022340/39022400 | `0x3999d144` "npc", `0x3999d134` "doodad" | state |
| `0x393bc620` | 6 | returns `DAT_3a17e6f4` (current loot/interaction source?) **[R]** | many | — | `DAT_3a17e6f4` | state/read |
| `0x393bdad0` | 799 | `InteractionIndicator::MakeCandidate`: UI/drag/state guards, pick query, resolve, set the bar **[R]** | 393bddf0 | 393601a0, 3902f010, 393bd650, 393bd050, 393bd850, 393bc980, 390b3870, 393b2530 | camera/player pose, `bar` | state |
| `0x393bddf0` | 280 | `InteractionIndicator::Update`: gate + `MakeCandidate` **[R]** | 393bdf10 | 393bc630, 393bdad0 | — | state |
| `0x393bdf10` | 283 | `UpdateInteractionIndicator`: indicator update + bar update **[R]** | 391739e0 | 393bddf0, 393b2760 | `DAT_39affa0c` | state |
| `0x393bc630` | 47 | indicator enable predicate (player/client/state) **[R]** | 393bddf0 | — | — | state/read |
| `0x393bd650` | 420 | resolve a pick result: node type 1 = unit (`node[6]`), 2 = doodad (`node[7]`); returns `node+3` (type at +8, unit +0xc, doodad +0x10) **[R]** | 393bdad0 | 393bcf00, 393bd290, 393b66c0, 393b73a0 | pick block `+8/+0xc/+0x10` | state |
| `0x393bd050` | 196 | **bar set for a unit**: collect player/pet/target skills, per-kind build, `bar+0x08` **[R]** | 393bdad0 | 390b3ae0, 393bcc20, 393bc980, 393bcf00, 393b2490, 393bcff0, 393b2530 | bar; unit `+8` | state |
| `0x393bd850` | 336 | **bar set for a doodad**: unit skill list + current-phase candidates, `bar+0xc` **[R]** | 393bdad0 | 390b3ae0, 393bcb90, 393bcd60, 393bd290, 393b24e0, 393bc980, 393bcff0 | bar; doodad `+0x7c` | state |
| `0x393bc6e0` | 201 | candidate cooldown/permission gate (phase/perm timers) **[R]** | 393bd290 | 397f2750, 3983b5f0, XlGetCurrentFileTime | doodad `+0xac/+0x360/+0x218/+0x358/+0x35c` | state |
| `0x393bc7e0` | 224 | bar loot gate: `bar[i]` 14047/12079 → open the current bag (`FUN_394fb010`, loot_all for 14047); if already open, 14047 → all rows, 12079 → slot 0 **[R]** | 393b1de0 | 393b1da0, 394f9360, 394f9790, 394f9ec0, 394fb010 | bar shown vector; `DAT_3a17e6f4+4` | state |
| `0x393bc8c0` | 180 | loot-source gate: `DAT_3a17e6f4` lootable → open with `FUN_394fb160(source, i==0)`; if already open, i==0 → all rows else slot 0 **[R]** | 393b1de0, 393b3ec0 | 394f9360, 394f9790, 394f9f20, 394fb160 | `DAT_3a17e6f4` | state |
| `0x393bcff0` | 86 | clear a candidate list (erase to begin) + reset ids **[R]** | 393bd050, 393bd850 | 393b5cc0 | candidate vector `+0x18/+0x1c` | **pure** (container) |
| `0x393b7320` | 114 | init a 0x24 candidate list object (set +0x10/+0x18/+0x1c/+0x20) **[R]** | 393b79a0 | `operator new` | — | **pure** (alloc) |
| `0x393b73a0` | 126 | destroy/clear the candidate list object **[R]** | 393b79a0, 393bd650 | 393b66c0 | — | **pure** |
| `0x393b5b30` | 98 | pick the first candidate and its extra **[R]** | 393b79a0 | — | — | **pure** (read) |
| `0x393b7eb0` | 256 | quest-act condition check for a doodad func row **[R]** | 393bd290 | — | row `+0xc`, player | state |
| `0x393b2af0` | 19 | current interaction object → vtable+4 (target id) **[R]** | 3947dac0 | — | `manager+0x48` | state |
| `0x393b30a0` | 56 | `SCWorldInteractionCanceled` for a non-current source **[R]** | 391d9100 | 39424220 | manager | state |
| `0x393b30e0` | 56 | `SCWorldInteractionCanceled` for the current source **[R]** | 391d9100 | 39424220 | manager | state |
| `0x393b7e40` | 111 | 1 s watchdog: drop the selected doodad if out of range **[R]** | 393b5f60 | 39035110, 39093800, 393b7aa0 | `DAT_3a17e6a0/6a4/684` | state |
| `0x393bc660` | 41 | reset the bar + set the current source `+0x28 = now + 350 ms` (`0x15e`) **[R]** | 393b3ec0, 393be280 | 393b2530 | `DAT_3a17e6f4+0x28` | state |

## B. Candidate collection, ordering and permission

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x393bd290` | 928 | **doodad candidate builder**: current phase funcs → filtered skill set **[R]** | 393b79a0, 393bc100, 393bd650, 393bd850, 393bd9e0 | 3963b250, 393b3300, 3983b4e0, 393b2c40, 393b9a80, 393b7eb0, 393bd240, … | doodad `+0x218` phase object, `+0x7c` objId, `+0xd9/+0xdc`; row `+0/+8/+0x20/+0x24` | state |
| `0x393bcf00` | 200 | per-unit-kind bar builder (switch 1..6) **[R]** | 393bd050, 393bd650, 393bd9e0 | 393bb9d0, 393bbb80, 393bbc90, 393bbd70, 393bbf10 | unit `+0x410`, `+0x3c30` | state |
| `0x393bb9d0` | 417 | kind 1 (NPC) candidates: loot/loot-all when mounted, gate 393c01d0, quest/talk, services **[R]** | 393bcf00 | 39340280, 393c01d0, 393c17e0, 393bed90, 393bb420, 393bb910 | NPC `+0x5fd4`, `+0x3c26`; skills 14047/12079/13877/13356 | state |
| `0x393bb420` | 1258 | NPC flag → service skill table (see MAP §2.2) **[R]** | 393bb910, 393bb9d0 | 393bab40, 393ba1b0, 393bb370, 39696280 | NPC desc slots `+0x04,+0x24,+0x25,+0x34,+0x58,+0x6c,+0x78,+0xac,+0xb0,+0xb4,+0xdc,+0xe4,+0xe5,+0xed` (non-zero test), list `+0x90` | state |
| `0x393bbb80` | 258 | kind 2 candidates (loot bag / corpse?) **[I]** | 393bcf00 | 390b3750, 397a6c60 | client `+0xde/+0xdf` | state |
| `0x393bbf10` | 450 | kind 3 (vehicle/slave) candidates from `FUN_3932d3a0` list **[R]** | 393bcf00 | 3932b550, 3932d3a0, 393ba1b0, 393bab40 | slave list | state |
| `0x393bbc90` | 220 | kind 5 (mate) candidates: mount/seat/cancel by relation and mount state **[R]** | 393bcf00 | 39028240, 3933b1b0, 393544d0, 39387610 | mate `+0x410` | state |
| `0x393bbd70` | 392 | kind 6 (house) candidates from `FUN_394d7a40` list **[R]** | 393bcf00 | 394d63e0, 394d7a40, 393ba1b0, 393bab40 | house object | state |
| `0x393bb320` | 72 | add one skill id to a candidate set **[R]** | 393bb9d0, 393bbb80, 393bbc90 | 393bab40, 393ba1b0 | — | state |
| `0x393bab40` | 110 | allocate a 0x18 set node `{key,value}` from an 8-byte stack struct **[R]** | 25 sites | `operator new` | key at `+0xc`, value `+0x10/+0x14` | **pure** (alloc) |
| `0x393ba1b0` | 188 | ordered insert (ascending key, dedup) **[R]** | 25 sites | 393b9620, 393b7d20 | node `+0xc` key | **pure** (container) |
| `0x393b6e90` | 101 | ordered insert variant used by the interaction-effect set **[R]** | 393b7140 | 393b68a0, 393b4320 | node `+0xc` | **pure** |
| `0x393ba370` | 98 | set lookup (by skill id) **[R]** | 393ba9c0 | — | — | **pure** |
| `0x393bd240` | 69 | add `{skillId, extra}` candidate **[R]** | 393bd290 | 393bab40, 393ba1b0 | — | state |
| `0x393b3300` | 389 | **permission switch** on `doodad_funcs.perm_id` 0..8 **[R]** | 393bc260, 393bd290, 393b9cb0 | 39360020, 39302aa0, 392f8af0, 392f80e0, 393b3280, 39308dd0, 393b32c0, 39053360, 3932aaf0 | unit/owner ids | state/read |
| `0x393b2c40` | 40 | func-type gate for loot funcs (types 6, 0x35) **[R]** | 393bd290 | 394f9f20 | player id | state |
| `0x3983b4e0` | 27 | `(playerId != currentTarget) || row.type != 0x34` gate **[R]** | 393bd290 | — | — | **pure** |


## C. Picking, click and default interaction

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x3942c640` | 976 | **UpdatePicking** per frame: UI/drag guards, cursor ray, nearest unit, doodad tooltip **[R]** | 391739e0 | 39428d80, 39354210, 39358c00/70, 393b51a0, 393b2c10, 39360020, 39035110 | `DAT_3a5a4dbc/dc0/dc4/dd0` | state |
| `0x39428d80` | 1392 | cursor → world ray → 2× `RayWorldIntersection(Game)`; fills pick block **[R]** | 3942c640 | 39086370, 39428ba0, 39426b50 | masks `0x8002048f`/`0x200048f`, rwi `0x117`/`0x11f`, max 2000 (`0x399a1ec0`) | state |
| `0x3942c540` | 189 | vehicle/boarding click handler (mode 1) **[R]** | 3942c620 | 3933b960, 3933df40, 3921aa80 | — | state |
| `0x390f50a0` | 384 | `ControlMouseClick`: dispatch on `DAT_3a5a4ea0` (0 normal, 2/3/8 skill, 4 item, 6 vehicle, 7/9) **[R]** | input | 390f4960, 390f4c30, 390f38f0, 390f3b20, 39016fd0, 39017220, 3942fb50, 39348800/70 | `DAT_3a5a4ea0`, `DAT_3a5a4da8==6` | state |
| `0x390f4960` | 715 | `MouseClickControl::Normal`: set/clear target, auto-attack gate, default interaction **[R]** | 390f50a0 | 39429e40, 393be4e0, 390f3780, 390f37f0, 394ea9c0, 390a6af0 | `setup+0x2d4`, `DAT_3a5a4dbc/dc0/dcc` | state |
| `0x390f3780` | 109 | default interaction: unit→0x68, doodad→doodad use, gimmick→39429d00 **[R]** | 390f4960 | 393be030, 393b79a0, 39429d00, 3915b270 | `DAT_3a5a4dbc/dc0/dc4` | state |
| `0x39429e40` | 567 | set target (`unit+0x1aa0`) + clicked object (`39361bf0`→`393576a0` writes `player+0x5f90`), UI frames, cancel NPC **[R]** | 12 sites | 39360020, 3920fa60, 3920ff80, 393be180, 39361bf0 | `unit+0x1aa0`, `player+0x5f90` | state |
| `0x393be4e0` | 645 | `CanAutoAttackTargetUnit` gate (combat) **[R]** | 390f4960, 393989e0 | 393578e0, 39360020, 39898e60 | relation bytes | state |
| `0x393b79a0` | 247 | doodad click: build candidates (`393bd290`), take first, `393b3ec0` **[R]** | 390f3780 | 39035110, 393b7320, 393b73a0, 393bd290, 393b5b30, 393b3ec0 | doodad `+0x7c` | state |
| `0x39429d00` | 320 | `StartGimmickInteraction` (gimmick kind 3) **[R]** | 390f3780 | 39177e00/60 | — | state |
| `0x393b51a0` | 158 | doodad sign/tooltip (`0x1A7` `DRAW_DOODAD_SIGN_TAG`) + `0x1A8` **[R]** | 3942c640 | 3963b290, 3963b250, 393b4da0/4bf0 | `DAT_3a17e2dc/e0` | state |
| `0x393b2c10` | 40 | clear the sign/tooltip (0x1A7) **[R]** | 3942c640 | 39424220 | `DAT_3a17e2dc` | state |
| `0x393601a0` | 641 | candidate query over the actor list (distance ≤ limit, mates +0.8) **[R]** | 393bdad0 | 39093860, 390b3850, 3935f560 | `DAT_39aff9cc` list, `0x3999d7e4` | state |
| `0x3902f010` | 685 | candidate query over the doodad list (takes `+0x2a0` 2.1 m when limit ≤ 0) **[R]** | 393bdad0 | 39093800, 392f4090, 39360020 | `DAT_39aff9cc+8` | state |

## D. Doodad use

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x393b3ec0` | 787 | **StartDoodadInteraction**: prechecks, 2.1 m gate + walk, gates, direct `CSStartSkill` **[R]** | 3907bcc0, 393b1de0, 393b4550, 393b79a0, 3947da50/dac0/db20 | 396f6390, 393b3d50, 39093800, 390a6af0, 393b3c50(doodad,desc), 393bc480, 393bc260, 393ba9c0, 393b9a80, 393b7c40, 39397e50, 393b3000, 393b2ab0 | `manager+4/+0x10/+0x38/+0x44/+0x5c`; `DAT_3a17e280`; sentinels | state |
| `0x393b3c50` | 256 | ship/vehicle interaction gate: doodad `+0x90` count, ship owner/name, shop state; true = use the vehicle path **[R]** | 393b3ec0, 393b7440 | 393a1e70, 390b3a10, 392ff2d0/370, 391826e0, 395779f0, 397f2180 | doodad `+0x90`, `+0x360`; player `+0x4ac` | state |
| `0x393b3d50` | 368 | prechecks: mounted, climbing, target/pet state, distance **[R]** | 393b3ec0, 393b7440 | 39093800, 390b4d40, 39360020, 39345aa0, 39361e80 | skill desc `+0x14b/+0x14c`; player `+0x3af0`; doodad `+0x80` | state |
| `0x393bc480` | 85 | resolve the current phase func (`FUN_3963b1d0`) and return `row+0x24` (usable flag) **[R]** | 393b3ec0, 393b7440 | 393bc100, 393bc620, 393bcb50, 3963b1d0 | row `+0x24` | state/read |
| `0x393bc260` | 567 | **func-type switch** (0x34,0x3d,0x3f,0x40,0x43,0x46,0x47,0x48,0x4a,0x4f,0x50,0x51,0x53..0x57) **[R]** | 393b3ec0, 393b7440 | 27 handlers (see MAP §3.4) | row `+8` type, `+0xc` func id | state |
| `0x393bc200` | 87 | resolve func descriptor from `(skill,type,doodad)` **[R]** | 393bc260 | 393bc100, 393bc620, 393bcb50, 3963b1d0 | — | state/read |
| `0x393bc100` | 255 | resolve the "current skill" func descriptor **[R]** | 393bc200, 393bc480 | — | — | state/read |
| `0x393ba9c0` | 60 | look up an active interaction object by skill and call its vtable **[R]** | 393b3ec0, 393b7440 | 393ba370 | `DAT_3a17e6d8` sentinel | state |
| `0x393b9a80` | 494 | quest/requirement gate for a doodad func row **[R]** | 393b3ec0, 393b7440, 393bd290 | 3963b250, 397d5200/5700/6140/71b0/6210, 39856420 | row `+0x34` list, `+0x24` list | state |
| `0x393b7c40` | 46 | skill desc usable? (`FUN_3930c020(desc+0xc, desc+0x78)`) **[R]** | 393b3ec0, 393b7440 | 396f6390, 3930c020 | — | state/read |
| `0x393b3000` | 154 | queue a deferred re-entry `{skill, doodadId, mode}` (delay 30) **[R]** | 393b3ec0, 393b7440 | 3902db50, 3902d670, 393b7b10 | action vtable `0x399d2dd0` | state |
| `0x3a6aa27f` | 20 | thunk (also `0x39397e50`) → `0x3ab94847` → `0x3ab73cfb`: **use skill with target** request **[R]** | 393b3ec0, 393b7440, 393c0ca0 | tail | caster/target/object blocks | state |
| `0x393b2ab0` | 33 | clear the current interaction object + list **[R]** | 393b3ec0, 393b7440, 393b7aa0 | — | `manager+0x38/+0x40/+0x48` | state |
| `0x393b7440` | 876 | `SCWorldInteractionSkillList` 0x07E: store list, 2.1 m gate/walk, auto-use first skill **[R]** | 391d9080 | 39035110, 39093800, 390a6af0, 393b3c50, 393bc480, 393b3d50, 393ba9c0, 393bc260, 393b9a80, 393b7c40, 39397e50 | manager `+0..+0x38`, `+4` doodad id | state |
| `0x393b3120` | 157 | **send CSSelectInteractionEx 0x06B** `{targetId, interaction, var1}` **[R]** | 3947dac0 | 397ac0d0, 3ac1058c, 3a6779eb | `unit+0x1aa0` target, `manager+0x38` count | state |
| `0x393b2af0` | 19 | current interaction object → vtable+4 (target id) **[R]** | 3947dac0 | — | `manager+0x48` | state |

## E. Interaction window state and per-func handlers

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x393b7b10` | 59 | switch interaction state (close old, set `+6a0/+6a4`) **[R]** | 3902e190, 393b1040, 393b3000, 393b8870, 393b8a30, 393bc260, 394dc360 | 393b7aa0 | `DAT_3a17e6a0/6a4` | state |
| `0x393b7b50` | 43 | cancel a state (craft cancel) **[R]** | 391e5070, 39440b40, 394411a0, 394cadd0, 394ce540, 394d0ae0 | 393b7aa0 | `DAT_3a17e6a0` | state |
| `0x393b7aa0` | 104 | **state cleanup**: per-state close event + `INTERACTION_END` 0xD2 + clear manager **[R]** | 12 sites | 39424220, 3902db50/cef0, 394d0710/810/8c0, 394419c0, 393b2ab0 | state 5/0x0e/0x0f/0x10/0x11/0x12 | state |
| `0x393b8240` | 86 | phase switch: `FUN_393703b0` + **CSChangeDoodadPhase 0x0E9** (writer `397ac8d0`) **[R]** | 393bc260 | 393703b0, 397ac8d0, 3ac1058c | row `+0x1c`, `*row` | state |
| `0x393b82a0` | 171 | UCC/emblem: event `0x1A4` with `"%u"` doodad id **[R]** | 393bc260 | 39424240 | `DAT_3a17e6a0 = 7` | state |
| `0x393b8450` | 446 | open paper/book: event `0x201` with `"book"/"page"` and a func id; may send packet 0x0E9 **[R]** | 393bc260 | 39424260, 397ac8d0, 39186810 | func `+0xc/+0x10` | state |
| `0x393b8610` | 227 | item renewal: event `0x8A RENEW_ITEM_WND_SHOW` (two ints) **[R]** | 393bc260 | 39424260 | doodad id, func id | state |
| `0x393b8700` | 169 | **open common farm info**: event `0x102` with farm type `*(row+0xc)` **[R]** | 393bc260 | 39424240 | `DAT_3a17e6a0 = 10` | state |
| `0x393b8870` | 256 | craft/act gate `FUN_397cf5a0` + event/delay **[I]** | 393bc260 | 397cf5a0, 39428370, 3902db50/cef0 | player `+0x1838..+0x1850` | state |
| `0x393b8a30` | 252 | second craft/act gate `FUN_397cf660` **[I]** | 393bc260 | 397cf660, 39428370, 3902db50/cef0 | player `+0x1838` | state |
| `0x393b8b80` | 146 | auction: event `0x6B` with `"auction"` **[R]** | 393bc260 | 39424240 | `DAT_3a17e6a0 = 0x0e` | state |
| `0x393b8c20` | 146 | bank: event `0x6B` with `"bank"` **[R]** | 393bc260 | 39424240 | `0x0f` | state |
| `0x393b8cc0` | 146 | store: event `0x6B` with `"store"` **[R]** | 393bc260 | 39424240 | `0x11` | state |
| `0x393b8f40` | 261 | hang/climb: 2 s throttle `0x2f4`, **CSHang 0x0CB** (writer `397ac600`) **[R]** | (jump table) | 393630e0, 39363410, 397ac600, 3935a480 | player `+0x3ac4/+0x3ac8/+0x3af0/+0x3b00` | state |
| `0x393b80d0` | 357 | deferred book/page action (two queued 0x1C/0x20 objects) **[R]** | 393bc260 | 3902db50/cef0/d670 | func `+0xc/+0x10` | state |
| `0x393b8400` | 67 | interact with the target unit (`FUN_393befd0`) **[R]** | 393bc260 | 39360020, 393befd0 | doodad `+0x88`/owner | state |
| `0x393b7c00` | 21 | open coffer: `FUN_3902e190(doodad)` (CS 0x6C) **[R]** | jump table | 3902e190 | — | state |
| `0x393b7c20` | 27 | `FUN_393703b0(doodad, DAT_3a17e6d0)` **[R]** | jump table | 393703b0 | — | state |
| `0x393b7ba0` | 21 | `FUN_394dc360(doodad)` (mailbox open) **[R]** | jump table | 394dc360 | — | state |
| `0x393b7bc0` | 25 | `FUN_393b1a90` (doodad vtable action) **[R]** | jump table | 393b1a90 | — | state |
| `0x393b7be0` | 25 | `FUN_393b1d60` (packet 0x0F7) **[R]** | jump table | 393b1d60 | — | state |
| `0x393b1910` | 98 | look up a doodad interaction object and call it **[R]** | 393b1a90 | — | `DAT_3a17d7b0` | state |
| `0x393b1a90` | 70 | doodad vtable action by objId **[R]** | 393b7bc0 | 39035110, 393b1910 | — | state |
| `0x393b1d60` | 62 | send packet **0x0F7** if skill == 12079 **[R]** | 393b7be0 | 397ac9d0, 3ac1058c | `DAT_39ac77a4` = 12079 | state |

## F. Phases, coffers, vegetation, interaction effects

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x391da5c0` | 130 | `OnDoodadPhaseChanged` (0x10F) → `FUN_39035280` **[R]** | packet | 390115a0/6d0, 39035280 | packet `+0xc,+0x10,+0x14,+0x18,+0x1c` | state |
| `0x39035280` | 152 | `DoodadManager::HandleDoodadPhaseChanged`: find doodad, apply if changed, refresh marks **[R]** | 391da5c0 | 39034a50, 393a6c60, 39383f40 | manager `+8/+0x14` | state |
| `0x393a6c60` | 224 | **ChangePhase**: set func group/time/puzzle, resolve desc, refresh model/anim/quest marks **[R]** | 39035280 | 3963b250, 393a1c90, 393a2e10, 393a3070, 393a4360, 393a6a70, 39411650 | doodad `+0x220/+0x224/+0x28c/+0x2a8/+0x354` | state |
| `0x391dc720` | 115 | `OnDoodadPhaseMsg` (0x157) → `FUN_3902e0f0` **[R]** | packet | 3902e0f0 | packet `+0xc,+0x10` | state |
| `0x3902e0f0` | 155 | fire `DOODAD_PHASE_MSG` 0xFD with the phase text **[R]** | 391dc720 | 3963b250, 39035110, 39424240 | desc `+0x18`, `+0xc` | state |
| `0x3902e190` | 86 | send **CSCofferInteraction 0x06C** for a doodad **[R]** | 393b7c00 | 39035110, 393b7b10, 397ac0f0, 3ac1058c | — | state |
| `0x391d5590` | 292 | `OnCofferContents` (0x096): 2.1 m gate, fill coffer rows, events **[R]** | packet | 39035110, 39093800, 394d0810/820/730, 39202fb0/db0 | packet `+0xc,+0x10,+0x14,+0x18,+0x19,+0x38` | state |
| `0x391d56c0` | 153 | `OnSplitCofferItemResult`: error toast or refresh stacks **[R]** | packet | 39428370, 394df370, 397d2200 | packet `+0xc,+0x10,+0x14` | state |
| `0x391da940` | 155 | `OnVegetationCutdowning` (0x081): `FUN_393b7800(unit, doodad, 100)` **[R]** | packet | 39360020, 39035110, 393b7800 | packet `+0xc,+0x10` | state |
| `0x393b7800` | 162 | append an interaction effect to the manager list **[R]** | 391da940 | 393b7140, 393b5700 | manager `+0x4c/+0x50` | state |
| `0x393b7140` | 234 | interaction-effect ctor (0x54 B): source/target/duration/time **[R]** | 393b7800 | 393b6e90 | `+0xc duration, +0x10 source, +0x14 target, +0x08 now` | state |
| `0x393b5f60` | 458 | `InteractionManager::Update`: tick/remove effects, 1 s watchdog `FUN_393b7e40` **[R]** | 391739e0 | 393b7e40, 3950a1f0 | manager `+0x4c/+0x50/+0x58` | state |
| `0x393b6220` | 148 | `OnInteractionEffectSkillCanceled`: remove effect by `(source,target,interaction)` **[R]** | 39395b50 | 3950a1f0 | effect vtable `+0x10/+0x14/+0x1c` | state |
| `0x393b7e40` | 111 | watchdog: drop the selected doodad if out of range **[R]** | 393b5f60 | 39035110, 39093800, 393b7aa0 | `DAT_3a17e6a0/6a4/684` | state |
| `0x3947dca0` | 127 | `X2Interaction:GetInteractionCount()` → `FUN_393b2b10` **[R]** | script | 393b2b10 | manager `+0x38` | state/read |
| `0x3947dd20` | 127 | `X2Interaction:GetInteractionSkillCount()` (same value) **[R]** | script | 393b2b10 | manager `+0x38` | state/read |
| `0x3947dda0` | 139 | `GetInteractionSkillId(index)`: 1-based, `FUN_393b31c0` **[R]** | script | 393b31c0 | manager `+0x10..` | state/read |
| `0x3947de30` | 123 | `GetInteractionSkillName(index)`: 1-based, `FUN_393b2b20` **[R]** | script | 393b2b20 | desc `+0x104` | state/read |
| `0x3947e130` | 186 | `GetInteractionSkillIconPath(item)`: `Game\ui\icon\` + `FUN_394e6c10(desc+0xd8)` **[R]** | script | 396f6390, 394e6c10 | `0x39ac3044` prefix | state/read |
| `0x393b2b10` | 4 | `manager+0x38` (skill count) **[R]** | 393b2b20, 3947dca0/dd20 | — | — | **pure** |
| `0x393b2b20` | 48 | skill id → desc `+0x104` name **[R]** | 3947de30 | 396f6390 | — | state/read |
| `0x393b31c0` | 32 | skill id at `manager+0x10` with bounds **[R]** | 3947dda0 | — | out-of-range → `DAT_39ac7840`=10995 | **pure** |

## G. NPC interaction

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x393be280` | 606 | **StartNpcInteraction**: gates, range/walk, service/quest routing, CS 0x65/0x68 **[R]** | 393b1de0, 393be850, 3947db70 | 393be0e0, 393b2c70, 393c01d0, 393bf270, 393bf310, 393bf500, 393c1280, 397ab740, 393bfc70, 39186810 | `manager+4/+8/+0xc/+0x10`; target `+0x410` | state |
| `0x393be180` | 203 | cancel NPC interaction; send **CSInteractNPCEnd 0x066** if target kind 1; clear camera **[R]** | 39046600, 391d9000, 3930e7e0, 393be280, 39429e40, 3947da90 | 39360020, 397ab760, 393bec00, 393c1310, 394df2c0 | `manager+4/+8/+0xc` | state |
| `0x393be030` | 176 | **send CSStartInteraction 0x068** after `FUN_393c01d0` gate **[R]** | 390f3780 | 39186810, 39360020, 393c01d0, 397ac0a0, 3ac1058c | `DAT_3a5a4dbc/dcc/dd0` | state |
| `0x393be0e0` | 149 | skill special-case gate: 12083/13238 need money/bank access **[R]** | 393be280 | 398a7a30, 39428370 | account/money | state/read |
| `0x393b2c70` | 23 | 500 ms rate limit predicate **[R]** | 393be280 | — | — | **pure** |
| `0x393c01d0` | 329 | **target gate**: cast busy 3, kind 2 → 0x66, `+0x3b00` → 0xdf, house 1000, hostile → 0x166 **[R]** | 393bb9d0, 393be030, 393be280 | 39360020, 394d63e0, 39027f40, 39839670, 398392e0, 3901be50/c5f0 | player `+0x3ac4/+0x3ac8/+0x3af0/+0x3b00`; target `+0x410/+0x420/+0x3c26` | state |
| `0x393bf270` | 151 | **3.0/3.5 m gate + auto-walk** **[R]** | 393be280 | 39093860, 390a6af0, 3915b0a0 | `0x39ac79fc=9.0`, `0x39ac7a00=12.25` | state |
| `0x393bf310` | 485 | service routing: occupy/seat/destroy, mount/resurrect, board, loot, recruiter **[R]** | 393be280 | 39362f50, 39365230, 393565c0, 393beb30, 393beb70, 3936d5f0, 398a99c0, 39424240 | skill ids `0x39ac7974…` | state |
| `0x393bf500` | 576 | **quest routing**: 13877 → 0x6B `("quest", "start"/"talk"/"complete", npcId)` **[R]** | 393be280 | 39340280, 39424820, 39424280 | `0x3999d2f4`, `0x399cb45c`, `0x3999dcd4` | state |
| `0x393bfc70` | 272 | face the NPC, stop auto-attack, play `fist_ac_talk_12`, camera **[R]** | 39379c70, 393be280 | 393b31e0, 39360920, 390b3750, 396962c0, 39007860 | `DAT_399abd40` | state |
| `0x393be850` | 540 | `SCNpcInteractionSkillList` 0x07B: store/filter list, inject quest/talk, start first **[R]** | 391d8f80 | 390b39c0, 39340280, 393bed90, 393c17e0, 393be280 | manager `+0x14..+0x44`; npc `+0x3c26/+0x410` | state |
| `0x393be250` | 41 | `SCNpcInteractionStatusChanged` 0x080: set `unit+0x5fd4` **[R]** | 391d91a0 | 39360020 | `+0x5fd4` | state |
| `0x393bea70` | 154 | collect a unit's interaction list (`+0x14` type, `+0x1c` ids, `+0x44` count) **[R]** | 39454920, 3947e3e0 | — | unit interaction fields | state/read |
| `0x393bed90` | 243 | **NPC greeting**: find `OnClientGreeting`, check requirements, pick a random line **[R]** | 393bb9d0, 393be850, 393bee90 | 39340210, 396962c0, 3936cda0, `CryRandom` | npc desc `+0xf4/+0xf8/+0xc/+0x1c/+0x2c` | state |
| `0x393bee90` | 117 | play the greeting chat bubble (`thunk_FUN_3a98400a` + `393f4ee0`) **[R]** | 393bf8d0 | 393bed90, 39696200, 393f4ee0 | — | state |
| `0x393bec00` | 22 | close an NPC handler (`FUN_393f1bd0(obj,1)`) **[R]** | 393be180, 393be280 | 393f1bd0 | — | state |
| `0x393c1310` | 146 | skill → handler **key name** (`FUN_393c11c0`, special "recruiter") **[R]** | 393be180, 393be280 | 393c11c0, 393f4f60 | `0x3a17f58c` map | state |
| `0x393c1280` | 134 | invoke the service handler by skill (special 23469/22780) or `FUN_393c0ca0` **[R]** | 393be280 | 393c11c0, 393c0ca0, 393bfa00, 393bfbd0 | `0x3a17f58c` | state |
| `0x393c11c0` | 98 | map lookup by skill id (std::map node) **[R]** | 393c1280, 393c1310 | — | map at `0x3a17f58c` | **pure** (read) |
| `0x393c14c0` | 792 | register the 17 service handlers into `0x3a17f58c`; handler objects `0x39ac7a30…` hold the vtable and the inline tag (`skill_trainer`, `auction`, …) **[R]** | init | 393c1450, 393c0ff0 | vtables `0x399d33c4…0x399d3500`, tags at `0x39ac7a34…` | state (init) |
| `0x393c1450` | 110 | allocate a map node `{key at +0xc, vtable at +0x10}` **[R]** | 393c14c0 | `operator new` | — | **pure** (alloc) |
| `0x393c0ff0` | 188 | ordered map insert **[R]** | 393c14c0 | 393c09c0, 393becb0 | key `+0xc` | **pure** |
| `0x393c0ca0` | 168 | fallback: cast the skill at the NPC from the NPC extra-skill list **[R]** | 393c1280 | 39340210, 39696280/2c0, 39397e50 | npc `+0x90` list | state |
| `0x393bf750` | 104 | service 12081 (learn skill): 0x6B **[R]** | registry | 39424240 | — | state |
| `0x393bf7c0` | 115 | service 17063 (pray): 0x6B **[R]** | registry | 39424240 | — | state |
| `0x393bf840` | 137 | service 17135 (buy specialty): zone check + packet 0x44 **[R]** | registry | 397cee70, 397aba80, 39428370 | player `+0x1838` | state |
| `0x393bf8d0` | 19 | service 13356 (talk) → greeting **[R]** | registry | 393bee90 | — | state |
| `0x393bf8f0` | 30 | service 16912 (territory) → `FUN_393bf1b0` **[R]** | registry | 393bf1b0 | — | state |
| `0x393bf1b0` | 178 | `DOMINION_NPC` event 0x6B with `"dominion_npc"` **[R]** | 393bf8f0 | 390b3850, 392f34d0, 39424260 | `0x399d33a8` | state |
| `0x393c0500` | 231 | service 14866 (sell backpack): house/zone checks + 0x6B **[R]** | registry | 396a6e40, 39695dc0, 39424240 | player `+0x1840` | state |
| `0x393c05f0` | 340 | service 16376 (sell specialty): trade pack **[R]** | registry | 396a6e40, 398a9b00, 397aba70, 39428370 | player `+0x1840`, item `+0x8e/+0x84` | state |
| `0x393c10d0` | 169 | generic service handler: 0x6B with the handler tag (`+4`) + text group **[R]** | 393c1180 | 39424240, 39202fb0/db0, 393c0c00 | handler `+4` tag | state |
| `0x393c1180` | 51 | service 23478: reset trade then generic handler **[R]** | registry | 394fd440/570, 393c10d0 | — | state |
| `0x393bfa00` | 150 | special 23469 (gender swap) **[R]** | 393c1280 | 3902db50/cef0/d670 | — | state |
| `0x393bfbd0` | 150 | special 22780 (beauty salon) **[R]** | 393c1280 | 3902db50/cef0/d670 | — | state |

## H. X2Interaction script bindings

`FUN_3947e730` (2550 B) registers all of these with the script system. The binding functions are
thin wrappers around the manager getters/setters above.

| addr | size | Lua name | native | purity |
|---|---|---|---|---|
| `0x3947d9d0` | 17 | `GetNPCInteractionMainMsg` | nil | **pure** |
| `0x3947dc30` | 112 | `GetNPCInteractionCount` | 0 | **pure** |
| `0x3947d9f0` | 17 | `GetNPCInteractionMsg` | nil | **pure** |
| `0x3947da10` | 17 | `ConfirmNPCInteraction` | no-op | **pure** |
| `0x3947da90` | 41 | `CancelNPCInteraction` | `FUN_393be180(1)` | state |
| `0x3947da30` | 17 | `SelectNpcInteraction` | no-op | **pure** |
| `0x3947dac0` | 94 | `SelectInteraction` | current object→0x6B or `FUN_393b3ec0` | state |
| `0x3947da50` | 49 | `CancelInteraction` | `FUN_393b3ec0(10994, 0, …)` | state |
| `0x3947dca0` | 127 | `GetInteractionCount` | `FUN_393b2b10` | state/read |
| `0x3947dd20` | 127 | `GetInteractionSkillCount` | `FUN_393b2b10` | state/read |
| `0x3947dda0` | 139 | `GetInteractionSkillId` | `FUN_393b31c0(i-1)` | state/read |
| `0x3947de30` | 123 | `GetInteractionSkillName` | `FUN_393b2b20(i-1)` | state/read |
| `0x3947deb0` | 203 | `GetNpcInteractionText` | current target NPC text | state/read |
| `0x3947e3e0` | 415 | `GetNpcInteractionList` | `FUN_393bea70` → Lua table | state/read |
| `0x3947e590` | 398 | `GetDoodadInteractionList` | `FUN_393b7260(player+0x5f90)` | state/read |
| `0x3947e130` | 186 | `GetInteractionSkillIconPath` | `FUN_394e6c10(desc+0xd8)` | state/read |
| `0x3947e730` | 2550 | registration of the above + `IsValidIconPath`, `IsComboPop` | — | state (init) |

## I. Loot

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x394fa610` | 164 | loot manager ctor (0x30 B): zero, row vector, lootable tree **[R]** | 39174520 | `operator new`, 397cc8f0 | `+0x0c/+0x10/+0x20/+0x24` | state |
| `0x394fa110` | 328 | `SCLootableState` true: insert lootable, mark unit `+0x208`, glow/event **[R]** | 39034510, 390382d0, 391d9780 | 39035110, 394f9620, 394f9bc0, 394f9cb0, 394f9f80, 393a17a0, 393eacf0, 393ed520 | tree key `(type,id)`, unit `+0x208/+0x20c` | state |
| `0x394faf40` | 208 | `SCLootableState` false: remove lootable, clear `+0x208`, close if current **[R]** | 39034510, 39038a30, 391d9780, 39366480 | 39035110, 394f9620, 394fab30, 393a1790, 393ed520, 391725b0 | — | state |
| `0x394fa9a0` | 386 | `SCLootBagData` 0x0F8: not mounted, stance, build rows, event 0x7E **[R]** | 391d9890 | 3901c0a0/cb10, 39360920, 393546f0, 397cc920, 394fa880 | 0xC0-byte item rows, count 0xFF | state |
| `0x394fa6c0` | 171 | `SCLootItemTook` 0x0F9: find row, clear in-flight, decrement/remove, event 0x7E **[R]** | 391d9910 | 394f9b10, 39173610 | row `+8/+0xc/+0x10/+0x18/+0xc8` | state |
| `0x394f9c20` | 144 | `SCLootItemFailed` 0x0FA: clear in-flight, event 0x7E **[R]** | 391e56d0 | 3901cba0, 39424220 | row `+0xc8` | state |
| `0x394f9790` | 157 | **LootItem(slot)**: validate row, set `+0xc8`, send **CSLootItem 0x08F** **[R]** | 393bc7e0, 393bc8c0, 3948a570 | 397abf10, 39186810 | row, `+0xc8` | state |
| `0x394fab30` | 209 | **CloseLoot**: send **CSLootCloseBag 0x090**, reset, event 0x7E **[R]** | 12 sites | 397abf40, 39186810, 394f9ec0, 394f9b10, 39424220, 39429e40 | `+0/+2/+4` | state |
| `0x394fb010` | 323 | **Open bag (unit)**: mounted gate, already-open check, **CSLootOpenBag 0x08E** **[R]** | 393bc7e0, 393beb30, 393c0040, 393c0320 | 397abed0, 394f9620, 394fab30 | player `+0x3c26`; `+0/+2/+4` | state |
| `0x394fb160` | 240 | **Open bag (npc/corpse variant)** **[R]** | 393bc8c0 | 397abef0, 394f9620, 394fab30 | — | state |
| `0x394f9360` | 24 | row count `(end-begin)/0xd0` **[R]** | 393bc7e0, 393bc8c0, 3948a6e0 | — | — | **pure** |
| `0x394f9620` | 65 | find lootable/tree node by `(type,id)` **[R]** | 394fa110, 394faf40, 394fb010/fb160 | — | tree | **pure** (read) |
| `0x394f9ec0` | 88 | "bag open for this owner?" (distance + tree) **[R]** | 393b8010, 393bb9d0, 393bc7e0, 393beb30, 393c0320, 394fab30 | 39093860, 39360020, 394f9dc0 | — | state/read |
| `0x394f9f20` | 91 | "lootable/current target?" (distance + tree) **[R]** | 393b2c40, 393bc8c0 | 39035110, 39093800, 394f9e40 | — | state/read |
| `0x394f9bc0` | 96 | mark the lootable object (quest/perm check, desc) **[R]** | 394fa110 | 393b3300, 3963b250 | — | state |
| `0x394f9cb0` | 247 | build/show the lootable marker (event 0x7E path) **[R]** | 394fa110 | 394f9300, 394f9850 | — | state |
| `0x391d9780` | 131 | `OnLootableState` 0x0F6 **[R]** | packet | 394fa110, 394faf40 | packet `+0x10,+0x14,+0x18` | state |
| `0x391d9810` | 128 | `OnUnitLootingState` 0x0F7 **[R]** | packet | 39360020, 393546f0 | packet `+0xc,+0x10` | state |
| `0x391d9890` | 122 | `OnLootingBag` 0x0F8 **[R]** | packet | 394fa9a0 | packet `+0x10`, `+0xc051` loot-all | state |
| `0x391d9910` | 126 | `OnLootItem` 0x0F9 **[R]** | packet | 394fa6c0 | packet `+0xc..+0x18` | state |
| `0x391e56d0` | 141 | `OnLootItemFailed` 0x0FA **[R]** | packet | 394f9c20, 391e5070 | packet `+0x10,+0x14,+0x18` | state |
| `0x391d9990` | 115 | `OnLootDice` 0x0FB **[R]** | packet | 394fae70, 394fb250 | — | state |
| `0x391d9a10` | 311 | `OnLootDiceNotify` 0x0FC **[R]** | packet | 39424280, 394e4ef0 | — | state |
| `0x391d9b50` | 118 | `OnLootDiceSummary` 0x0FD **[R]** | packet | 394f9410 | — | state |
| `0x3948add0` | 906 | `X2Loot` registration (`GetNumLootItems`, `GetLootingBagItemInfo`, `LootItem`, `CloseLoot`, dice) **[R]** | init | 3948a6e0, 3948a9e0, 3948a760, 3948a570, 3948a5a0, 3948a8a0, 3948a5c0/5f0/610 | — | state (init) |
| `0x3948a6e0` | 127 | `GetNumLootItems` → `FUN_394f9360` **[R]** | script | 394f9360 | — | state/read |
| `0x3948a570` | 33 | `LootItem(slot)` → `FUN_394f9790(slot-1)` **[R]** | script | 394f9790 | — | state |
| `0x3948a5a0` | 28 | `CloseLoot` → `FUN_394fab30` **[R]** | script | 394fab30 | — | state |
| `0x3948a760` | 315 | `GetLootingBagItemTooltipText(slot)` **[R]** | script | 394f9380, 39009e40, 39423560 | — | state/read |
| `0x3948a8a0` | 248 | `GetLootItemSoundType(slot)` **[R]** | script | 394f9380, 39695dc0, 39079890 | — | state/read |

## J. Shared leaves used by this subsystem

| addr | size | role | C | E | fields / globals | purity |
|---|---|---|---|---|---|---|
| `0x39093800` | 82 | `(actorA, actorB, limit, outDist)` → `dist < limit` **[R]** | 17 sites | 390b3ae0, 3989bf90 | object vtable `+0x18` | state/read |
| `0x39093860` | 90 | same with a **squared** limit **[R]** | 19 sites | 390b3ae0, 3989bf90 | — | state/read |
| `0x3989bf90` | 101 | minimum collider-pair distance `(colliderList, out)` **[R]** | 7 sites | 398b1ab0 | `0x3999d608` init | **pure** (given colliders) |
| `0x39035110` | 61 | objId → **doodad** object (`FUN_39034a50`) **[R]** | 114 sites | 39034a50 | — | state/read |
| `0x39360020` | 57 | objId → **unit/actor** object (`FUN_3935f560`) **[R]** | 641 sites | 3935f560 | — | state/read |
| `0x39340210` | 112 | objId → NPC/quest object (`FUN_3936ba70`) **[R]** | 19 sites | 3936ba70 | — | state/read |
| `0x396962c0` | 62 | object → name/value (`FUN_39696280` variant) **[R]** | 109 sites | — | — | state/read |
| `0x39428370` | 8 | show a localized interaction error text id **[R]** | 421 sites | `0x3a6a126a` | — | state (UI) |
| `0x390115a0` | 297 | profiler/event scope enter (`name, 2, rate, 1`) **[R]** | 543 sites | 39001f80 | `DAT_3a5bb1d8` | state |
| `0x390116d0` | 237 | profiler/event scope leave **[R]** | 1153 sites | — | — | state |
| `0x39214380` | 9391 | **UI event name table**: 626 `FUN_39002bf0("NAME")` calls; id = `(ECX-0x39d22ce8)/4`, not the call order **[R]** | init | 39002bf0 | `DAT_3a5bb1d8` | state (init) |

## Totals

The tables list **220 distinct function entries**: 216 sized by `funcs.csv`
(**57,346 bytes**) plus the four 8-byte `do_interaction_*` stubs (**32 bytes**), i.e.
**~57,378 bytes**. Rows that repeat an address appear in two groups. The largest single functions are the UI event
registry (`0x39214380`, 9,391 B), the cursor ray (`0x39428d80`, 1,392 B) and the NPC skill table
(`0x393bb420`, 1,258 B).
