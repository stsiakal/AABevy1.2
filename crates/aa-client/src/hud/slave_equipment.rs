//! The equipment of the owned ship / vehicle: the grid of the original Summon Info window
//! (`x2ui/slave/slave_info.alb` embeds `slave_equipment.alb`, laid out by `slave_customizing_style.alb`).
//!
//! The scripts ask `X2SiegeWeapon` for the slave's customizing type (`slaves.slave_customizing_id`, only
//! for hulls that declare `slave_equip_slots`), for each slot (`GetSlaveEquipSlotInfo`), and move parts with
//! `PickEquipSlotOfSlave` (left click / drag: pick up or drop what the pointer holds),
//! `UninstallEquipSlotOfSlave` (right click) and `CanSlaveEquipPickedItem` (dims slots that do not fit the
//! picked bag item). Slot numbers are 0 based equip slot ids (`ES_* - 1`, 0..26).
//!
//! The parts themselves are the slave unit's equipment (`SCUnitEquipmentsChanged`); a drop becomes a
//! `CSChangeSlaveEquipment` (see `aa_net::slave_control`), validated like the server does with the database
//! packs (`item_slave_equipments`, `allow_to_equip_slaves`, `allow_to_equip_slots`).

use std::collections::{BTreeSet, HashMap, HashSet};

use aa_net::model::{Item, ItemDetail};
use aa_net::slave_control::{
    BAG_CONTAINER, SLAVE_EQUIPMENT_CONTAINER, SLAVE_EQUIPMENT_SLOTS, SlaveEquipmentRequest, SlaveItemLocation,
};
use aa_net::state::Inventory;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;

use super::api::{HudData, Shared};
use super::data::{GameData, any_int};
use super::vehicles::Current;

/// Rules from the database, shared by the grid and the request check.
#[derive(Default)]
pub(super) struct Tables {
    /// `slave_equip_slots`: slave template -> declared equip slot ids.
    pub(super) slots: HashMap<u32, BTreeSet<u8>>,
    /// `slaves.slave_customizing_id` of hulls that also declare slots (the window layout `1..4`).
    pub(super) customizing: HashMap<u32, u32>,
    /// `item_slave_equipments` rows that name a part: item -> (hull pack, slot pack).
    pub(super) parts: HashMap<u32, (u32, u32)>,
    /// `allow_to_equip_slaves`: hull pack -> slave templates.
    pub(super) hull_packs: HashMap<u32, HashSet<u32>>,
    /// `allow_to_equip_slots`: slot pack -> equip slot ids.
    pub(super) slot_packs: HashMap<u32, HashSet<u8>>,
    /// The localized name of a slave template (the "Classification" row).
    pub(super) names: HashMap<u32, String>,
    /// `SLAVE_KIND` text key of a slave template (the "Type" row).
    pub(super) kinds: HashMap<u32, &'static str>,
}

/// The `ui_texts` key (category `SLAVE_KIND`) of `slaves.slave_kind_id`.
pub(super) fn kind_key(kind: u32) -> &'static str {
    match kind {
        1 => "big_sailing_ship",
        2 => "small_sailing_ship",
        3 => "speedboat",
        4 => "boat",
        5 => "tank",
        6 => "machine",
        7 | 8 => "siege_weapon",
        9 => "small_sailing_ship",
        _ => "machine",
    }
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut out = Self::default();
        let mut query = conn.prepare("SELECT slave_id,equip_slot_id FROM slave_equip_slots")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let (slave, slot) = (any_int(row, 0)? as u32, any_int(row, 1)?);
            if let Ok(slot) = u8::try_from(slot) {
                out.slots.entry(slave).or_default().insert(slot);
            }
        }
        let mut query = conn.prepare(
            "SELECT id,slave_customizing_id,slave_kind_id,coalesce(nullif((SELECT l.en_us FROM localized_texts l \
             WHERE l.tbl_name='slaves' AND l.tbl_column_name='name' AND l.idx=slaves.id),''),name,'') FROM slaves",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let id = any_int(row, 0)? as u32;
            let customizing = any_int(row, 1)? as u32;
            if customizing != 0 && out.slots.contains_key(&id) {
                out.customizing.insert(id, customizing);
            }
            out.kinds.insert(id, kind_key(any_int(row, 2)? as u32));
            out.names.insert(id, row.get::<_, String>(3)?);
        }
        let mut query = conn.prepare(
            "SELECT item_id,coalesce(slave_equip_pack_id,0),coalesce(slot_pack_id,0) FROM item_slave_equipments \
             ORDER BY id",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.parts
                .entry(any_int(row, 0)? as u32)
                .or_insert((any_int(row, 1)? as u32, any_int(row, 2)? as u32));
        }
        let mut query = conn.prepare("SELECT slave_equip_pack_id,slave_id FROM allow_to_equip_slaves")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            out.hull_packs.entry(any_int(row, 0)? as u32).or_default().insert(any_int(row, 1)? as u32);
        }
        let mut query =
            conn.prepare("SELECT slave_equipment_equip_slot_pack_id,equip_slot_id FROM allow_to_equip_slots")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            if let Ok(slot) = u8::try_from(any_int(row, 1)?) {
                out.slot_packs.entry(any_int(row, 0)? as u32).or_default().insert(slot);
            }
        }
        Ok(out)
    }

    /// The server's `can_equip_item_on_slave`: the item is a ship part, the hull declares the slot and the
    /// item's hull / slot packs (when set) allow it.
    pub(super) fn can_equip(&self, item: u32, slave: u32, slot: u8) -> bool {
        let Some(&(hull_pack, slot_pack)) = self.parts.get(&item) else { return false };
        self.slots.get(&slave).is_some_and(|slots| slots.contains(&slot))
            && (hull_pack == 0 || self.hull_packs.get(&hull_pack).is_some_and(|set| set.contains(&slave)))
            && (slot_pack == 0 || self.slot_packs.get(&slot_pack).is_some_and(|set| set.contains(&slot)))
    }
}

/// What a script asked for; slots are 0 based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Request {
    /// Mount the bag item in `bag` on the equip slot `slot` (swapping the part there into its place).
    Equip { bag: usize, slot: u8 },
    /// Take the part in `slot` off into the bag slot `bag`, or the first free one.
    Unequip { slot: u8, bag: Option<usize> },
}

/// The wire request for `request`, or why it is refused. Items are echoed as received.
pub(super) fn build(
    tables: &Tables,
    current: &Current,
    character_id: u32,
    inventory: &Inventory,
    request: Request,
) -> Result<SlaveEquipmentRequest, &'static str> {
    let worn = |slot: u8| current.equipment.get(usize::from(slot)).and_then(Option::clone);
    let (first_item, second_item, first_slot, slot) = match request {
        Request::Equip { bag, slot } => {
            let item = inventory.item_at(BAG_CONTAINER, bag).cloned().ok_or("no item in that bag slot")?;
            if !tables.can_equip(item.template_id, current.template, slot) {
                return Err("that part does not fit this slot");
            }
            (Some(item), worn(slot), bag, slot)
        }
        Request::Unequip { slot, bag } => {
            let item = worn(slot).ok_or("nothing is mounted there")?;
            let bag = match bag.filter(|b| inventory.item_at(BAG_CONTAINER, *b).is_none()) {
                Some(b) => b,
                None => inventory.first_free_bag_slot().ok_or("the bag is full")?,
            };
            (None, Some(item), bag, slot)
        }
    };
    let request = SlaveEquipmentRequest {
        character_id,
        slave_tl_id: current.tl_id,
        db_slave_id: current.db_id,
        bts: false,
        first_item,
        second_item,
        first_location: SlaveItemLocation { container: BAG_CONTAINER, slot: u8::try_from(first_slot).map_err(|_| "bag slot out of range")? },
        second_location: SlaveItemLocation { container: SLAVE_EQUIPMENT_CONTAINER, slot },
    };
    request.validate()?;
    Ok(request)
}

fn tip(lua: &Lua, data: &HudData, item: &Item) -> mlua::Result<Table> {
    let detail = match &item.detail {
        ItemDetail::Equip(detail) => Some(detail),
        _ => None,
    };
    let grade = i64::from(super::store::ui_grade(item.grade));
    let result = match data.game.get() {
        Some(game) => super::tips::item_tip(
            lua,
            game,
            &data.tips,
            item.template_id,
            Some(grade),
            Some(i64::from(item.count)),
            super::tips::Instance {
                durability: detail.map(|d| d.durability),
                temper: detail.map_or((0, 0), |d| (d.temper_physical, d.temper_magical)),
                flags: Some(item.flags),
            },
        )?,
        None => None,
    };
    match result {
        Some(out) => Ok(out),
        None => {
            let out = lua.create_table()?;
            out.set("itemType", item.template_id)?;
            out.set("lookType", item.template_id)?;
            out.set("itemGrade", grade)?;
            Ok(out)
        }
    }
}

fn icon(game: Option<&GameData>, template: u32) -> String {
    game.and_then(|g| g.items.get(&template)).map(|i| i.named.icon.clone()).unwrap_or_default()
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let weapon: Table = lua.globals().get("X2SiegeWeapon")?;
    let state = bridge.clone();
    weapon.set(
        "GetSlaveCustomizingType",
        lua.create_function(move |_, _: Value| {
            let data = state.borrow();
            let (Some(current), Some(game)) = (&data.vehicles.current, data.game.get()) else { return Ok(None) };
            Ok(game.vehicles.equip.customizing.get(&current.template).copied())
        })?,
    )?;
    let state = bridge.clone();
    weapon.set(
        "GetSlaveEquipSlotInfo",
        lua.create_function(move |lua, (_, slot): (Value, i64)| {
            let data = state.borrow();
            let (Some(current), Some(game)) = (&data.vehicles.current, data.game.get()) else { return Ok(None) };
            let Ok(slot) = u8::try_from(slot) else { return Ok(None) };
            let out = lua.create_table()?;
            let has = game.vehicles.equip.slots.get(&current.template).is_some_and(|s| s.contains(&slot));
            out.set("has", has)?;
            if let Some(item) = current.equipment.get(usize::from(slot)).and_then(Option::as_ref).filter(|_| has) {
                out.set("itemInfo", tip(lua, &data, item)?)?;
            }
            Ok(Some(out))
        })?,
    )?;

    let hud: Table = lua.globals().get("HUD")?;
    let state = bridge.clone();
    // The part in `slot` as the pointer holds it (`drag.lua` `PickEquipSlotOfSlave`).
    hud.set(
        "slave_part",
        lua.create_function(move |lua, slot: i64| {
            let data = state.borrow();
            let Some(item) = data
                .vehicles
                .current
                .as_ref()
                .and_then(|c| usize::try_from(slot).ok().and_then(|s| c.equipment.get(s)).and_then(Option::as_ref))
            else {
                return Ok(None);
            };
            let out = lua.create_table()?;
            out.set("id", item.template_id)?;
            out.set("grade", super::store::ui_grade(item.grade))?;
            out.set("icon", icon(data.game.get(), item.template_id))?;
            Ok(Some(out))
        })?,
    )?;
    let state = bridge.clone();
    hud.set(
        "slave_can_equip",
        lua.create_function(move |_, (item, slot): (u32, i64)| {
            let data = state.borrow();
            let (Some(current), Some(game)) = (&data.vehicles.current, data.game.get()) else { return Ok(false) };
            Ok(u8::try_from(slot).is_ok_and(|slot| game.vehicles.equip.can_equip(item, current.template, slot)))
        })?,
    )?;
    let state = bridge.clone();
    // `bag` is the real bag slot, 1 based.
    hud.set(
        "slave_equip",
        lua.create_function(move |_, (bag, slot): (usize, u8)| {
            if let Some(bag) = bag.checked_sub(1) {
                state.borrow_mut().vehicles.equip_requests.push(Request::Equip { bag, slot });
            }
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    hud.set(
        "slave_unequip",
        lua.create_function(move |_, (slot, bag): (u8, Option<usize>)| {
            state
                .borrow_mut()
                .vehicles
                .equip_requests
                .push(Request::Unequip { slot, bag: bag.and_then(|b| b.checked_sub(1)) });
            Ok(())
        })?,
    )?;
    Ok(())
}

/// The slots (0..26) whose part differs between two snapshots of the slave's equipment.
pub(super) fn changed(old: &[Option<Item>], new: &[Option<Item>]) -> Vec<u8> {
    (0..SLAVE_EQUIPMENT_SLOTS).filter(|slot| old.get(usize::from(*slot)) != new.get(usize::from(*slot))).collect()
}

#[cfg(test)]
mod tests;
