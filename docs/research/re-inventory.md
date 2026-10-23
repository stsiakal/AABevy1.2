# Inventory and items (original 1.2 client)

Scope: bag window (tabs, sort, expand, slots), item move/split/stack/destroy, equipment window
(including backpack and costume), equip/unequip rules and errors, item tooltips and comparisons,
item use/cast/cooldown/targets, bank/warehouse, item locks, and the packets that carry them.
Evidence tags: `[R]` read in code/data, `[I]` inferred, `[?]` not found.

## 1. Sources

* 1.2 logic: `x2game_unpacked.dll` decompile `C:/AAEMU_DISK/aa-re/client12/out-x2game/`
  (function VAs are image base `0x38ff0000`; `_index.txt` was regenerated from the plain `.c`).
* 1.2 UI: `.alb` bytecode under `D:/aa-re/client12/ui/game/scriptsbin/x2ui/`; `albtool.py`
  (`D:/aa-re/client12/tools/`) disassembles it and preserves the original Lua line numbers.
* Data: `D:/aaemu-live-bevy-srv/rs-game/Data/compact.sqlite3` (read-only).
* Packet layouts: `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game` (the Rust port of the 1.2 server).
* Comparison: `D:/aaemu-client/AABevy1.2` (branch main), read-only.

No client or server was run for this pass; there are no live-oracle measurements.

## 2. Model and slot addressing

`SlotType` is one byte: 0 none, 1 equipment, 2 inventory, 3 bank, 4 trade, 5 mail, 6 auction,
`0xF2`/252 slave and mate equipment, 255 system (`models/container.rs:19-36`). The bag is
container 2 and the bank 3; coffer storage is a separate house container.

The client encodes a *virtual slot* as `(slotIndex << 24) | (bagId << 8)`; every `X2Bag`
method strips the low byte and passes the high byte as the physical index. Evidence:
`FUN_3944cd10` (lock) builds `CONCAT13(index-1, 0x200)` for the bag, `FUN_3944cdb0`
(`MoveToEmptyBankSlot`) uses `0x300` (`x2game.dll.c` FUN_3944cdb0), and `FUN_3961e850`
accepts a slot only when bits 8-15 equal the container's bag id and the high byte is below its
count (`FUN_3961e850`). Lua slot arguments are 1-based and decoded with `-1`
(`FUN_3944c9f0`).

The equipment window shows 21 slots: `ES_HEAD, NECK, CHEST, WAIST, LEGS, HANDS, FEET, ARMS,
BACK, EAR_1, EAR_2, FINGER_1, FINGER_2, UNDERSHIRT, UNDERPANTS, MAINHAND, OFFHAND, RANGED,
MUSICAL, BACKPACK, COSPLAY` (`equipped_item_view.alb:3-25`, `playerEquipSlots`). The server
container has 28: indices 0..27 add FACE, HAIR, GLASSES, RESERVED, TAIL, BODY, BEARD
(`models/container.rs:43-73`); the window deliberately hides body parts. `ES_BACKPACK` and
`ES_COSPLAY` are ordinary equipment slots 26/27, not bag tabs.

Item flags (`models/item.rs:39,653-659`): 0x01 soul-bound, 0x02 UCC, 0x04 secure/locked,
0x08 skinized, 0x10 unpacked, 0x20 auction-win. Bind kinds: 1 normal, 2 on pickup,
3 on equip, 4 on unpack, 5 on pickup-pack, 6 on auction win (`data/items.rs:24-31`).

Grades are `item_grades` 12 rows; the display order swaps raw 0/1 so raw 0 ("normal") is
shown as grade 1 and raw 1 ("low") as grade 0 (Bevy `hud/store.rs:107-112`,
`hud/tips.rs:165-171`). Colours come from `item_grades.color_argb`
(`FFBA976D, FF949293, FF77B064, FF558FD7, FFCB72D8, FFD78B06, FFE17853, FFF95252,
FFCF7D5D, FF8FA5CA, FFBF7900, FFC90B0B`). Refund multiplier and durability value are in the
same row (`item_grades.refund_multiplier`, `durability_value`).

## 3. The rules

### 3.1 Bag window

Window 470x600, 5 columns x 10 rows for the initial 50 slots, slot margin 13, spacing
`ICON_SIZE + 5` (`sort_inventory.alb:322-368`). Every cell is a cooldown button, registered
for left and right clicks and left drag (`sort_inventory.alb:108-118`). Capacity is
`X2Bag:Capacity()` = 50 + 10 per expansion step, hard maximum 150
(`FUN_3944d490` reads a byte from the bag object; `Expand` and `ExpansionMaxSlotCount` are
Lua properties on the same object, registration `FUN_394505b0`). Server rule: start 50,
`step=(slots-50)/10`, new size `50+10*(step+1)`, max 150 (`systems/economy/inventory.rs:1345-1440`).

Tabs: tab 1 is All; the database ships four default tabs and their groups
(`default_inventory_tabs`: Equipment, Consumables, Materials, Quest; groups
`{1,2,3,4}`, `{5}`, `{6,22,23,26}`, `{7}` from `default_inventory_tab_groups`). The UI caps
user tabs at `MAX_EXTEND_TAB = 10` and offers 18 icon coordinates
(`common.alb:1-25`, `MAX_TAB_ICON_COORDS`). Tabs are created/edited/removed through
`X2Bag:CreateTab(icon, text, groups)`, `EditTab`, `RemoveTab`, `SwitchTab`
(registration `FUN_394505b0`); each raises `BAG_TAB_CREATED/REMOVED/SWITCHED` and a sort raises
`BAG_TAB_SORTED` (event table `FUN_39214380`).

Sort has two operations. `ManualSort` is the sort button (`sort_inventory.alb:841-843`,
tooltip `inventory.sort`): it orders the All view by item-category order ascending, grade
descending, then name byte-wise, then slot (`sorting.lua:57-65` is the Bevy port of this
comparator; the native byte compare is `FUN_390105e0`). `Sort` only re-filters the current tab;
filtered tabs list their group's items (`sorting.lua:74-85`). A sorted view is a client-side
permutation, not item moves `[I]` (the Bevy port `sorting.lua:1-160` keeps a physical-slot
permutation; no move packet is produced by the sort path).

Slot state is updated in a fixed chain (`sort_inventory.alb:277-317`):
`UpdateInvalidItem` -> `UpdateEmptyItem` -> `UpdateIcon` -> `UpdateRequireLevelCheck` ->
`UpdateUsable` -> `UpdateDurability` -> `UpdateConfirmed` -> `UpdateStack` ->
`ChangeSlotBackground`.
* Invalid: virtual slot beyond `Capacity()` -> `InvalidState` background
  (`sort_inventory.alb:284-286`).
* Empty: icon and overlays cleared, stack 0, lock cleared, `InvalidState`
  (`sort_inventory.alb:193-203`).
* Icon: `SetItemIcon(lookType, itemGrade)`, dye, lock, pack flag
  (`sort_inventory.alb:217-229`).
* Level: `X2Bag:ItemRequireLevel(real)` -> `ProcessRequireLevelCheck(level)`; the widget paints
  the red 30%-alpha state when the player is below it (`sort_inventory.alb:231-234`,
  `sort_inventory.alb:96-102`).
* Usable/locked: `Enable(not X2Bag:IsLocked(real))` (`sort_inventory.alb:236-238`).
* Durability: `ItemDurability` -> `{current, max}` -> `ProcessDurabilityColor`
  (`sort_inventory.alb:240-254`).
* Confirmed: `IsConfirmedSlot(virtual)`; false -> `UnconfirmedState`, the yellow "new item"
  border (`sort_inventory.alb:256-261`). `BAG_ITEM_CONFIRMED` is the matching event.
* Stack: `ItemStackStr` prints `count` when the template is stackable and count > 1, else empty
  (`sort_bag.alb:106-122`).

The expand button calls `X2Bag:Expand()` (`sort_inventory.alb:905-907`); the confirmation is
`DLG_TASK_EXPAND_INVENTORY`, which the bag window deletes with the destroy/convert dialogs when
it closes (`sort_bag.alb:159-163`). `bag_expands` says every step costs one item 8000025 and
0 money for both bag and bank (`bag_expands` rows 1-18); the server sends
`SCInvenExpandedPacket(slotType, numSlots)` (`inventory.rs:1430-1439`).

### 3.2 Clicks, drags, split, destroy

A bag cell's left click goes through the active mode first (`PreClickSlot`), then
`X2Bag:HandleClick(real)`, then `PreUseSlot`/`X2Bag:HandleUse(real)`
(`sort_inventory.alb:121-140`). `HandleClick` picks the item up; if something is held it drops
it on the target (`drag.lua:79-126`). `HandleUse` is the use/equip entry, called on the left
path too; the native handler behind `X2Bag:HandleUse` (`FUN_3944c7f0`) equips a wearable
template and otherwise casts its use skill, and the right-button dispatch is native `[I]`
(corroborated by the Bevy route `ingame.lua:663-673`).

Shift+left click on a stack of more than one fires
`INVEN_SLOT_SPLIT` with `("bag", realSlot)`, which opens the bag's amount spinner
(`drag.lua:88-92`; original window seam `sort_bag.alb` `CreateSplitItemWindow`, spinner
`bagWindow.bagSpinner`). `PickupBagItemPartial(bagId, slot, amount)` then holds that amount
(`FUN_3944d210`, registration `FUN_394505b0`).

Left drag start calls `HandleClick` (`sort_inventory.alb:143-147`); dropping calls
`HandleClick` on the destination (`sort_inventory.alb:150-155`). The server move is
`CSSwapItemsPacket` (0x03a) with `fromItemId u64, toItemId u64, fromSlotType u8, fromSlot u8,
toSlotType u8, toSlot u8` (`item_handlers.rs:80-118`). Split is `CSSplitBagItemPacket`
(0x039) = the same block plus `count i32` (`item_handlers.rs:143-177`). Server action selection:
no target and source count > moved count -> split; no target and equal -> move-all; same
template and stackable -> merge (clamped to max stack); otherwise swap
(`inventory.rs:940-970`).

Dropping a held bag/equipment item on the world (`overUi` false) opens
`DLG_TASK_DESTROY_ITEM` with the item info and count; OK sends `CSDestroyItemPacket`
(0x038) = `itemId u64, slotType u8, slot u8, count i32` (`item_handlers.rs:180-264`;
Bevy port `drag.lua:262-275`). The server treats count <= 0 as "all", removes from the stack or
the container and answers `SCItemTaskSuccess(action=1 Destroy, tasks...)`
(`item_handlers.rs:198-260`).

### 3.3 Equip and unequip

`X2Bag:EquipBagItem(slot, isAuxEquip)` (`FUN_3944c9f0`) resolves the item's two candidate
equipment slots with `FUN_394e5550`: it classifies the template as accessory (slot pair at
`+0x1e`), armour (`+0x20`/`+0x74`) or weapon (`+0x30`) and returns normal/aux slot ids, -1 for
none. `isAuxEquip` chooses the second candidate; when that is -1 the client falls back to the
normal one (`FUN_3944c9f0`). The equipment window's three quick bag slots call exactly this
with an `isAux` flag (`equipped_item.alb:427-434`). An item with both ids -1 is not equippable.

Equipping is encoded as an item move, not a separate opcode: into an empty equipment slot the
client sends `CSSwapItemsPacket` with `fromItemId=0` and the empty slot in `fromSlotType/fromSlot`,
the bag item as target (`inventory.rs:830-845`, comment "the client sends FROM empty equipment
slot => TO item"). Unequip is the reverse move (equipment -> bag). The server then:
* rejects if the destination container refuses the template (`can_accept`, `inventory.rs:780-812`);
* rejects if item id/slot/count do not match the request (`inventory.rs:870-940`);
* splits/merges/swaps or equips into the empty slot (`inventory.rs:970-1060`);
* for two-handed weapons, requires one free bag cell and unequips the other hand
  mainhand/offhand (`inventory.rs:1010-1085`);
* answers with `SCItemTaskSuccess` tasks plus `SCUnitEquipmentsChanged` for observers.

Right click on an equipped slot: `IsInventoriable(slot)` true -> `UnequipItem(slot)`, else
`UseEquippedItem(slot)` (`equipped_item.alb:352-356`). Left click picks the worn item up
(`PickupEquippedItem`), left click with Shift while chat is inactive inserts the item link
(`equipped_item.alb:340-344`).

Equipment errors are server error ids rendered through `FUN_3956b860`. Item/equipment relevant
ids: 0x015 item_locked, 0x018 item_cannot_use, 0x019 bag_full, 0x01b not_enough_item,
0x0cb invalid_slot, 0x0d0 cannot_equip_slot_is_locked, 0x13c item_cannot_move_to_bag,
0x142 need_equip_before_use, 0x143 cant_change_equip, 0x146 must_equip_proper_item,
0x1af cannot_use_siege_item, 0x1b4 not_enough_space, 0x1b6 cannot_use_in_equipped_item,
0x1b8 item_cannot_use_here, 0x1d2 cannot_use_in_battle_field, 0x1d4 level_low_to_equip,
0x206 level_low_to_use, 0x213 level_high_to_equip, 0x23e cannot_change_equip_when_dead,
0x298 item_secure_condition, 0x2a1 no_match_gender_to_equip, 0x2af
not_enough_expand_item_and_money.

Costume/backpack specifics `[I]`: `ES_COSPLAY` is a normal equipment slot but only items whose
`item_categories.item_group_id = 4` (costume) can go there; `ES_BACKPACK` takes group 20
("backpack") items (`item_groups` 4/20; `item_backpacks` maps item -> backpack type).
`equipped_item_view.alb:98-103` fills the three quick bag slots with
`X2Bag:FindBagItemByEquipSlot`; `X2Equipment.GetBackPackGoodsInfo` is the separate backpack
window (`x2ui-calls.txt:179`, `store_backpack.alb:5`).
The server forbids moving lifespan or soul-bound items into a coffer (errors 0x219 and 0x1ea).

### 3.4 Tooltips

The item tooltip is assembled from the info table returned by `X2Bag:GetBagItemInfo` /
`X2Equipment:GetEquippedItemTooltipText` (`FUN_39423560` fills `stack, expireTime, locked,
soul_bound, repairCost, crafter, isMyWorld, craftedWorldName, convertibleItem, userMusicTitle,
userMusicRequireGrade, securityState, securityRemainDate, lookType, lookChanged, useAsSkin,
scalable, needsUnpack`). The UI pushes sections in this order (`tooltip.alb:3119-3184`):
itemTitles, securityState, userMusic, treasureInfos, itemTimeInfos, itemLookState, itemInfosEx,
itemInfos, itemSocketInfos, attr/options, descriptions, setItemsInfo, equipSetInfo, crafterInfo,
sellables; then `SetInfo`. Minimum width 250 (`tooltip.alb:3119-3124`). A bag cell calls
`GetBagItemInfo(realSlot)` on Enter and `ShowTooltip(info, 1, false)` on Enter, `HideTooltip`
on Leave (`sort_inventory.alb:158-168`); an equipment slot uses
`GetEquippedItemTooltipText("player", es)` and a delay (`equipped_item.alb:236-257`).

Lines and colours (all in `baselib/tooltip/tooltip.alb`):
1. Title: grade frame + icon, name in `data.gradeColor`, category left and usage ("equip"/"use")
   right (`:1326-1435`; grade colour from `item_grades.color_argb` through `X2Item:GradeColor`).
2. Look-changed title `%s%s [%s]`, ORANGE (`:1439-1456`).
3. Security state `%s%s`, SKYBLUE; states `ITEM_SECURITY_LOCKED`, `ITEM_SECURITY_UNLOCKING`
   (`:1460-1484`).
4. User music requirement `%s%s : %s (%s)`; GREEN when satisfied, RED otherwise, SOFT_BROWN for
   the grade (`:1488-1513`).
5. Treasure sextant coordinates (`:1517-1528`).
6. Level: `tooltip_player_item_usable_level` / `tooltip_pet_item_usable_level` then
   `%s%s: %d`, or `%s%s: %d~%d` for a range; lifespan/unpack/expiry follow with day/hour/minute
   formatting; expired is ORANGE (`:1532-1672`).
7. Enchantable line, ORANGE (`:1676-1717`).
8. Slot type and gender restriction (`onlyMale`/`onlyFemale`; SOFT_BROWN/RED); attack delay
   `%s%s%s %0.1f` and attack speed `%s%s %s%d/%d` for weapons; durability `%s%s %s%d/%d`, GRAY,
   ORANGE when enchantable (`:1721-1855`).
9. Socket line `%s%s`, GRAY (`:1910-1967`).
10. Weapon/armour info: weapon DPS `%s%s %s%0.1f (%s) %s`, min~max damage, magic/heal DPS;
    armour `%s%s %s%d %s` and magic resistance `%s%s %s%d`; labels GRAY, values SOFT_BROWN,
    with the diff appended (`:987-1093`).
11. Stat modifiers: one line per `modifier` entry via `GetAddModifierText`; percent entries use
    `type=1`, flat `type=0`; GRAY for labels (`:1971-1999`, `:929-981`).
12. Weight/length `%s%s` ORANGE, unit GRAY; caught date (`:2536-2770`).
13. Description; required condition (`requiredCondition`), enchant level/grade/slot conditions;
    use-skill description (`SIK_DESCRIPTION`); proc description; gem proc; recharge buff and
    remaining time; skin/stat marks `|c%s[%s]|r` with SKIN_ITEM / STAT_ITEM (`:2536-2770`).
14. Crafter `crafter`, GRAY (`:2774-2799`).
15. Sell price/refund; "cannot sell" in RED; repair status (`cantRepair`), repair cost
    (`:2803-2849`).
16. Set pieces `%s (%d/%d)` in SET_ORANGE, each piece name white when worn, GRAY otherwise
    (`:2853-2880`); set effects GREEN, satisfied pieces GRAY (`:2884-2912`).

Comparison: when the info carries `diffInfo`, every stat line becomes
`source (difference)`/`from -> to`. `MakeItemDiffText` uses `FONT_COLOR_HEX.RED` for a
decrease and `SOFT_BROWN` for an increase; percent differences print `-%`/`+%`
(`tooltip.alb:760-804`, `:626-748`). Armour/DPS comparison builds a signed diff table
(`out-dis-tooltip-32.txt:626-748`) and prints the value with its difference (`:987-1016`). The comparison table is supplied for the equip preview
(`GetRenewItemPreviewTooltipText(bagId, sourceSlot, targetSlot)`, registration
`FUN_394505b0`); the exact fill point in the bag hover path is `[?]`.

### 3.5 Item use, cast and cooldown

Use is the item template's `items.use_skill_id`, cast through the normal skill pipeline:
`CSStartSkillPacket` (0x052) = `skillId u32, casterType u8, caster, targetType u8, target,
flag u8, [skill object]`. For an item the caster is type Item:
`objId BC, itemId u64, itemTemplateId u32, type1 u8, type2 u32`
(`systems/skills/handlers.rs:25-52`, `types.rs:35-75`). The target is the skill's target type
(unit, position, position2, item, doodad, position3; `types.rs:230-270`); a potion is self, an
offensive item uses the selected target `[I]` for the examples. The server consumes the item
through the skill task list (`SKILL_REAGENTS` 40 / `SKILL_EFFECT_CONSUMPTION` 41,
`models/item_task.rs:52-55`); the client's "Used:" chat filter treats 13/40/41/45 as consumptions
(`items.rs:24`).

Per-slot cooldown (`X2Bag:GetBagCooldown(slot)`, `FUN_3944d9e0`):
* resolve item -> template -> use skill -> skill cooldown `(remaining, total)` in ms
  (`FUN_3989a3e0`);
* category id 8 (saplings/seedlings in this data, `item_categories` row 8) additionally gets a
  600 s cooldown measured from the item's timestamp at `+0x1e`: if the elapsed time is <= 600 s,
  total = 600000 ms and remaining = `(600 - elapsed) * 1000`; otherwise both 600000
  (`FUN_3944d9e0`; the double constant at VA `0x399daf58` is 600.0).
The slot paints this with `AddOnUpdateItemCooldown` / `SetItemSlotForCooldown`
(`sort_inventory.alb:177-191`, `:184-191`). Item use is refused when the item is locked
(`ITEM_LOCKED`, result 27) or the template's skill requirements fail; those results are the
normal skill failure list (`usable.rs:91-107`, Bevy port).

### 3.6 Bank / warehouse

The bank is container 3 (slot encoding `0x300`, `FUN_3944cdb0`) and its window is the same bag
view with `X2Bank` (`sort_bank.alb`). It has capacity 50 + 10 per step to 150
(`FUN_3944d490` on the bank object, same expansion code). `X2Bank:Deposit(moneyStr, aaPointStr)`
and `Withdraw(...)` parse both strings with `XlSafeAtoi`, using -1 on parse failure
(`FUN_394522b0`, `FUN_39452320`); the packets are `CSDepositMoneyPacket` (0x047) and
`CSWithdrawMoneyPacket` (0x048), each `amount i32, aaPoint i32`
(`item_handlers.rs:318-380`). Money moves are answered with `SCItemTaskSuccess` task types
83 (deposit) and 84 (withdraw) (`models/item_task.rs:83-84`).
`X2Bank:MoveToEmptyBagSlot(slot)` moves a bank item to the first free bag cell
(`FUN_39452480`, `0x300` source); `X2Bag:MoveToEmptyBankSlot(slot)` is the reverse
(`FUN_3944cdb0`). `PickupBankItemPartial`, `LockSlot`/`UnlockSlot`, `Sort`/`ManualSort`,
`Expand`, `GetLinkText` mirror the bag (`FUN_39454aa0` registration). The bank does not allow
selling or using items; `CanSaveMoney`/`CanManualSort` gate the controls (`sort_bank.alb`).

Coffer (house storage) uses `X2Coffer` (`FUN_3945ecb0` registration) and
`SCCofferContentsUpdatePacket` (0x096) = `objId BC, containerId u64, count u8,
(slot u8, item | u32 0)...` (`doodad/coffer.rs:121-170`); moves are
`CSSwapCofferItemsPacket` (0x040) and splits `CSSplitCofferItemPacket` (0x03f) with
`count i32, MoveArgs, dbId u64` (`item_handlers.rs:382-477`). Coffer permissions are the
house permission dialog (`coffer_permission.alb`).

### 3.7 Item locks

The bag lock button checks `CheckExclusiveUI(GROUP_BAG_SHARE, UI_KIND_ITEM_SECURITY_LOCK)`
and opens a four-button window with the texts Lock Item, Lock Equip, Unlock Item, Unlock Equip
(`sort_bag.alb:281-347`; texts `item_lock.alb` `LOCK_ITEM/LOCK_EQUIP/UNLOCK_ITEM/UNLOCK_EQUIP`).
The Lua switches `X2Item:EnterSecurityLockMode/LeaveSecurityLockMode` and
`EnterSecurityUnlockMode/LeaveSecurityUnlockMode` (`sort_bag.alb:290-308`) and, for the
equipment buttons, asks for confirmation (`lock_equip`/`unlock_equip`) and then calls
`X2Equipment:SecurityLock`/`SecurityUnlock` (`sort_bag.alb:311-341`). The button-variable names
(`itemLockBtn` calls `EnterSecurityUnlockMode`) do not line up with the labels, so the labels,
not the variable names, are authoritative `[R]`. `[I]`: SecurityLock/SecurityUnlock are the
`CSEquipmentsSecurePacket` (0x04c, empty) / `CSEquipmentsUnsecurePacket` (0x04d) senders
(`item_handlers.rs:531-558`), and a single item is sent with `CSItemSecurePacket` (0x04a):
the server reads `u8, slotType u8, u8, slot u8, itemId u64` (the two skipped bytes are unknown)
(`item_handlers.rs:479-509`); unlock is the same body (0x04b). Before an unlock the dialog shows the remaining delay:
`X2Item:GetSecurityUnlockDelayTime()` divided by 60, in red, then OK starts the unlock
(`sort_bag.alb:314-330`). The lock state is `[I]` item flag 0x04 and is shown as the padlock overlay
and the SKYBLUE "locked/unlocking" tooltip line (`item_lock.alb`, `tooltip.alb:1460-1484`).
The delay value itself is `[I]` 600 s, from the same 600.0 constant and the identical
minute display; its exact client constant was not read directly.

The lock button is feature-gated: `sort_inventory.alb:876` calls `CreateLockButton` only when
`X2Player:GetFeatureSet().itemSecure` is true. `FUN_39496b20` builds that table from the received
FeatureSet array at `+0x14`: siege 0, premium 4, ranking 36, ingamecashshop 38, customsaveload 40,
bm_mileage 43, itemSecure 45, secondpass 46, slave_customize 48, beautyshopBypass 52,
ingameshopSecondpass 55, backpackProfitShare 56, sensitiveOpeartion 58, taxItem 59, achievement 63,
dwarfWarborn 74, mailCoolTime 75, hudAuctionButton 81, auctionPostBuff 83; `levelLimit` is byte 1
and `mateLevelLimit` byte 8. With no FeatureSet the binding returns an empty table. This server
initializes `Feature::ItemSecure` (45) off (`features.rs`), so the original client shows no lock
button here `[R]`.

## 4. Numeric parameters

| Parameter | Value | Evidence |
|---|---|---|
| Bag/bank base slots | 50 | `inventory.rs:1345-1440` |
| Expansion step | +10 per step, max 150 | same |
| Expansion cost | item 8000025 x1, 0 money | `bag_expands` |
| Bag window | 470x600, 5x10 cells, margin 13, gap ICON_SIZE+5 | `sort_inventory.alb:322-368` |
| Max user tabs | 10 | `common.alb:1` |
| Tab icon coordinates | 18 | `common.alb` main |
| Tooltip min width | 250 | `tooltip.alb:3119-3124` |
| Sapling cooldown | 600 s | `FUN_3944d9e0`, double at `0x399daf58` |
| Equipment slots shown | 21 (0..20 in the list; server 28) | `playerEquipSlots` |
| Item grades | 12 rows, display swaps 0/1 | `item_grades` |
| Item flags | 0x01 soulbound, 0x02 UCC, 0x04 secure, 0x08 skin, 0x10 unpacked, 0x20 auction win | `models/item.rs:39,653-659` |
| Item detail (equip) | 55 bytes | `models/item.rs:1053-1105` |

Item wire layout (`models/item.rs:1053-1072`): `templateId u32`; stop if body part or 0; else
`itemId u64, grade u8, flags u8, count i32, detailType u8, details, createTime i64,
lifespanMins i32, madeUnitId u32, worldId u8, unsecureTime i64, unpackTime i64`.
Equip detail: `imageItemTemplateId u32, durability u8, i16 0, runeId u32, chargeStart i64,
dyeItemId u32, 7 gem u32, temperPhysical u16, temperMagical u16` (55 bytes).
`SCItemTaskSuccessPacket` (0x090): `action u8` (task type), `count u8`, tasks, `forceRemove
count u8`, u64 ids, `u32 0` (`item_packets.rs:13-37`; task type list
`models/item_task.rs:13-130`).

## 5. Packets

C->S: `CSSwapItemsPacket 0x03a` (move/swap/equip/unequip), `CSSplitBagItemPacket 0x039`,
`CSDestroyItemPacket 0x038`, `CSExpandSlotsPacket 0x041` (`slotType i32,
autoUseAAPoint bool`), `CSDepositMoneyPacket 0x047`, `CSWithdrawMoneyPacket 0x048`,
`CSSplitCofferItemPacket 0x03f`, `CSSwapCofferItemsPacket 0x040`, `CSItemSecurePacket 0x04a`,
`CSItemUnsecurePacket 0x04b`, `CSEquipmentsSecurePacket 0x04c`,
`CSEquipmentsUnsecurePacket 0x04d`, `CSThisTimeUnpackItemPacket 0x134`
(`opcodes.rs:58-77,160`, `item_handlers.rs`).
S->C: `SCCharacterInvenInitPacket 0x04b` (`numInvenSlots u32, numBankSlots u32`),
`SCCharacterInvenContentsPacket 0x04c` (`slotType u8, numChunks u8, startChunkIdx u8, items`),
`SCInvenExpandedPacket 0x04d` (`slotType u8, numSlots u8`), `SCItemTaskSuccessPacket 0x090`,
`SCItemTaskNotifyPacket 0x091` (layout not found), `SCItemDetailUpdatedPacket 0x093` (not
found), `SCUnitEquipmentsChangedPacket 0x094` (`objId BC, count u8, (slot u8, item | i32 0)...`),
`SCUnitEquipmentIdsPacket 0x095` (not found), `SCCofferContentsUpdatePacket 0x096`,
`SCItemAcquisitionPacket 0x097`, `SCSyncItemLifespanPacket 0x098`
(`isAdded bool, itemId u64, templateId u32, expireTime i64`), `SCSlaveEquipmentChangedPacket
0x068`, `SCMateEquipmentChangedPacket 0x126` (`opcodes.rs:618-698,826`,
`item_packets.rs:40-104`, `packets/world.rs:51-93`).

## 6. Inferred / unresolved

* `[I]` `X2Bag:HandleClick` picks up and drops, while `HandleUse` is the use/equip path; the
  exact C++ cursor vtable bodies (0x50-0x5c) are not in the plain export. Lua proves both are
  called on a left click (`sort_inventory.alb:121-140`).
* `[I]` comparison data is attached by `GetBagItemInfo` for the corresponding worn slot; the UI
  diff code is clear (`tooltip.alb:626-804`) but the fill point in `FUN_39423560` was not
  isolated.
* `[I]` security unlock delay is 600 s; only the dialog's `/60` and the 600.0 constant were read.
* `[?]` filtered-tab ordering source: the native `SlotByIdx` delegates to the cursor map
  (`FUN_3944ef90`), which is data-driven and absent from the export.
* `[?]` `SCItemTaskNotifyPacket`, `SCItemDetailUpdatedPacket`, `SCUnitEquipmentIdsPacket`
  layouts are not defined in the server port.

## 7. Gaps vs Bevy (ranked)

1. **Done (`644e473`, feature gating `03b8e6c`): item security locks.** Original: four-button lock window, `CSItemSecure` /
   `CSEquipmentsSecure`, flag 0x04, padlock overlay, SKYBLUE tooltip line and a 600 s unlock
   confirmation (`sort_bag.alb:281-347`, `item_handlers.rs:479-558`, `tooltip.alb:1460-1484`).
   Bevy now sends all four packets (`aa-net::secure`, byte-exact against the server reader),
   exposes the flag/deadline state as `securityState`/`securityRemainDate`, opens the original
   window when the received FeatureSet has `ItemSecure` (bit 45) on, and routes the item modes
   through `drag.lua`; the original padlock overlay and SKYBLUE line render from the item info
   table. `GetFeatureSet` now maps every native field from the received FeatureSet bytes (empty
   table before one arrives), so the lock button stays hidden on this server (ItemSecure off).
2. **Bag/bank item cooldown is never shown.** Original computes the use-skill cooldown plus the
   600 s sapling timer per slot (`FUN_3944d9e0`). Bevy's item slot hard-codes
   `cooldown: None` (`hud/panels.rs:144-160`) and `X2Bag.GetBagCooldown` returns nil
   (`aa-ui/examples/fake/fake_bag.lua:208`). Fix: fill `SlotSnap.cooldown` from the item's
   use skill and the category-8 timestamp, and override `GetBagCooldown`.
3. **Most tooltip sections are missing.** Original renders lifespan/expiry, locked, look
   changed, unpack, sockets, recharge, user music, crafter, set and sell lines
   (`tooltip.alb:1326-2912`, `FUN_39423560`). Bevy's `tips.rs:181-217` sets only name, grade,
   description, stack, level, price, refund, sellable and equipment stats. Fix: add the missing
   table fields from `items` row and item instance (`expireTime`, `needsUnpack`, `locked`,
   `securityState`, `itemSocketInfos`, `sellables`, `crafterInfo`).
4. **Level-requirement red state is dead.** Original calls `ItemRequireLevel` and
   `ProcessRequireLevelCheck` (`sort_inventory.alb:231-234`); Bevy's `X2Bag.ItemRequireLevel`
   always returns 0 (`fake_bag.lua:198`) so no item is ever marked too high level. Fix: return
   the real `items.level_requirement`.
5. **New-item "unconfirmed" state is missing.** Original marks freshly received items with
   `UnconfirmedState` and raises `BAG_ITEM_CONFIRMED` (`sort_inventory.alb:256-261`,
   `FUN_39214380`). Bevy's `IsConfirmedSlot` only knows an explicit test flag
   (`fake_bag.lua:204-207`); item-task intake never marks slots. Fix: mark added items and fire
   the event on hover/click.
6. **Aux equipment slots are ignored.** Original `EquipBagItem(slot, isAux)` picks the
   normal or aux candidate from `FUN_394e5550` (`FUN_3944c9f0`); Bevy's implementation ignores
   `isAux` and always takes the first free/first slot (`drag.lua:69-76`, `:142-146`). Fix: pass
   the auxiliary flag to `HUD.drag_equip`'s target selection.
7. **Using a worn item is a no-op.** Original right click uses `UseEquippedItem` for
   non-inventoriable worn items (`equipped_item.alb:352-356`); Bevy installs an empty
   `UseEquippedItem` (`fake_character.lua:53`) and never overrides it. Fix: cast the equipped
   item's use skill with an item caster.
8. **Bank ignores AA-point fields and locks.** Original Deposit/Withdraw parse money and AA
   point and the packets carry both (`FUN_394522b0`, `item_handlers.rs:318-380`); Bevy drops the
   AA argument (`bank.lua:69-70`) and `X2Bank.IsLocked` is always false (`bank.lua:47`).
   Fix: send `aa_point` and implement bank locks.
9. **Equipment stat getters still use the fake table.** Original `X2Equipment.ItemArmor/
   ItemDameage/ItemGemStats/GetEquippedItemInfo` read the worn item and
   `GetBackPackGoodsInfo` the backpack (`FUN_39471060` registration); Bevy only overrides `GetEquippedItemTooltipText/Info` and
   `ItemIdentifier` (`ingame.lua:566-596`), so paperdoll/backpack windows fall back to the fake
   starter set (`fake_character.lua:10-55`). Fix: implement the getters over `HUD.equipment`.
10. **Filtered-tab cell order may not match the original.** Bevy's filtered tab is sorted by
    item uid before virtual->physical mapping (`sorting.lua:66-85`), so moving an item changes a
    filtered tab's order. The original `SlotByIdx` delegates to the native cursor map
    (`FUN_3944ef90`), whose ordering was not recovered. Verify with a live bag after a move.
11. **Expand/destroy dialog details.** `DLG_TASK_DESTROY_ITEM` uses the held count
    (`drag.lua:262-275`) rather than a user-editable amount; expansion is otherwise ported
    (`expansion.rs:24-160`). Low.

## 8. Open questions

* What exactly do the two skipped bytes in `CSItemSecurePacket` mean, and when does the server
  answer with `ITEM_UNLOCK_EXCESS` (task 96)?
* Does the original filtered tab order follow physical slot order or item id? One live move on
  a filtered tab settles it.
* Where is `diffInfo` attached for a bag hover, and is only the equip-preview path compared?
* Exact `GetSecurityUnlockDelayTime` value and whether it is account/character configurable.
* Client-side item-use restrictions that are not in the skill rules (e.g. `item_cannot_use`
  versus `item_cannot_use_here`) and which error each surface uses.

## 9. Live oracle measurements

None. This pass was file-only (2026-10-22); no client, server or capture was started.
