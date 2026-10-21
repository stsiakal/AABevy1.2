//! The original item-look converter: pick a base item and the item whose appearance it
//! should copy, then send `CSConvertItemLook` after confirmation.
//!
//! `changeitemlook.alb` F1 (lines 27-286) drives `changeItemLookWindow` through
//! `X2ItemLookConverter`; `sort_bag.alb` routes bag clicks through
//! `AttachItemToItemConverter`. The server's `economy/store.rs::convert_item_look` consumes
//! `item_look_convert_required_items` and applies the image template.
use std::collections::HashMap;

use aa_net::interaction::Interact;
use aa_net::state::Player;
use aa_net::{Command, Event, NetCommand, NetEvent};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;

use super::api::{HudBridge, HudData, Shared};
use super::data::any_int;
use super::tips::{self, Instance};
use crate::Screen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Convert {
    pub required_item_id: u32,
    pub required_item_count: i32,
}

#[derive(Default)]
pub(super) struct Tables {
    converts: HashMap<u32, Convert>,
    holdables: HashMap<u32, u32>,
    wearables: HashMap<u32, u32>,
    weapon_holdable: HashMap<u32, u32>,
    armor_slot: HashMap<u32, u32>,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut t = Tables::default();
        let mut q = conn.prepare("SELECT item_look_convert_id,item_id,item_count FROM item_look_convert_required_items ORDER BY id")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.converts.entry(any_int(r, 0)?.max(0) as u32).or_insert(Convert {
                required_item_id: any_int(r, 1)?.max(0) as u32,
                required_item_count: any_int(r, 2)?.max(0) as i32,
            });
        }
        let mut q = conn.prepare("SELECT holdable_id,item_look_convert_id FROM item_look_convert_holdables ORDER BY id")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.holdables.entry(any_int(r, 0)?.max(0) as u32).or_insert(any_int(r, 1)?.max(0) as u32);
        }
        let mut q = conn.prepare("SELECT wearable_slot_id,item_look_convert_id FROM item_look_convert_wearables ORDER BY id")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.wearables.entry(any_int(r, 0)?.max(0) as u32).or_insert(any_int(r, 1)?.max(0) as u32);
        }
        let mut q = conn.prepare("SELECT item_id,holdable_id FROM item_weapons")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.weapon_holdable.insert(any_int(r, 0)?.max(0) as u32, any_int(r, 1)?.max(0) as u32);
        }
        let mut q = conn.prepare("SELECT item_id,slot_type_id FROM item_armors")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            t.armor_slot.insert(any_int(r, 0)?.max(0) as u32, any_int(r, 1)?.max(0) as u32);
        }
        Ok(t)
    }

    /// `EquipItemTemplate.ItemLookConvert` for an armor or weapon template.
    pub(super) fn convert_for(&self, template: u32) -> Option<&Convert> {
        let convert = if let Some(slot) = self.armor_slot.get(&template) {
            self.wearables.get(slot)
        } else if let Some(holdable) = self.weapon_holdable.get(&template) {
            self.holdables.get(holdable)
        } else {
            None
        }?;
        self.converts.get(convert)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Picked {
    slot: u8,
    uid: u64,
}

#[derive(Default)]
pub(super) struct View {
    owner: u32,
    mode: bool,
    base: Option<Picked>,
    look: Option<Picked>,
    requests: Vec<(u64, u64)>,
}

fn slot(data: &HudData, picked: Picked) -> Option<&super::panels::SlotSnap> {
    data.panels
        .bag
        .get(usize::from(picked.slot))
        .and_then(Option::as_ref)
        .filter(|item| item.uid == picked.uid && item.kind == "item")
}

fn info(lua: &Lua, data: &HudData, picked: Picked) -> mlua::Result<Option<Table>> {
    let (Some(game), Some(item)) = (data.game.get(), slot(data, picked)) else { return Ok(None) };
    tips::item_tip(
        lua,
        game,
        &data.tips,
        item.id,
        Some(i64::from(super::store::ui_grade(item.grade))),
        Some(item.count),
        Instance {
            durability: item.dur,
            flags: item.flags,
            ..default()
        },
    )
}

fn pick(data: &HudData, index: i64) -> Option<Picked> {
    let index = index.checked_sub(1).and_then(|i| u8::try_from(i).ok())?;
    let item = data.panels.bag.get(usize::from(index))?.as_ref()?;
    (item.kind == "item" && item.uid != 0 && item.id != 0).then_some(Picked { slot: index, uid: item.uid })
}

fn convertible(data: &HudData, index: i64) -> Option<Picked> {
    let picked = pick(data, index)?;
    let game = data.game.get()?;
    let item = slot(data, picked)?;
    game.look.convert_for(item.id).map(|_| picked)
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let converter: Table = lua.globals().get("X2ItemLookConverter")?;
    let state = bridge.clone();
    converter.set(
        "EnterItemConvertMode",
        lua.create_function(move |_, _: Value| {
            state.borrow_mut().look.mode = true;
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "LeaveItemConvertMode",
        lua.create_function(move |_, _: Value| {
            let mut d = state.borrow_mut();
            d.look.mode = false;
            d.look.base = None;
            d.look.look = None;
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "ClearConvertBaseItemSlot",
        lua.create_function(move |_, _: Value| {
            state.borrow_mut().look.base = None;
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "ClearConvertLookItemSlot",
        lua.create_function(move |_, _: Value| {
            state.borrow_mut().look.look = None;
            Ok(())
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "SetConvertBaseItemSlotFromBag",
        lua.create_function(move |_, (_this, index): (Value, i64)| {
            let mut d = state.borrow_mut();
            let Some(picked) = convertible(&d, index) else { return Ok(false) };
            d.look.base = Some(picked);
            Ok(true)
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "SetConvertLookItemSlotFromBag",
        lua.create_function(move |_, (_this, index): (Value, i64)| {
            let mut d = state.borrow_mut();
            let Some(picked) = convertible(&d, index) else { return Ok(false) };
            if d.look.base.is_some_and(|base| base.slot == picked.slot) {
                return Ok(false);
            }
            d.look.look = Some(picked);
            Ok(true)
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "GetConvertBaseItemInfo",
        lua.create_function(move |lua, _: Value| {
            let d = state.borrow();
            let Some(picked) = d.look.base else { return Ok(None) };
            info(lua, &d, picked)
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "GetConvertLookItemInfo",
        lua.create_function(move |lua, _: Value| {
            let d = state.borrow();
            let Some(picked) = d.look.look else { return Ok(None) };
            info(lua, &d, picked)
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "GetConvertResultItemInfo",
        lua.create_function(move |lua, _: Value| {
            let d = state.borrow();
            let Some(picked) = d.look.look else { return Ok(None) };
            info(lua, &d, picked)
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "GetItemLookConvertInfo",
        lua.create_function(move |lua, _: Value| {
            let d = state.borrow();
            let Some(base) = d.look.base else { return Ok(None) };
            let (Some(game), Some(item)) = (d.game.get(), slot(&d, base)) else { return Ok(None) };
            let Some(convert) = game.look.convert_for(item.id) else { return Ok(None) };
            let has = d
                .panels
                .bag
                .iter()
                .flatten()
                .filter(|i| i.id == convert.required_item_id)
                .map(|i| i.count)
                .sum::<i64>();
            let need = i64::from(convert.required_item_count);
            let out = lua.create_table()?;
            out.set("money", "0")?;
            out.set("enoughMoney", true)?;
            let items = lua.create_table()?;
            let row = lua.create_table()?;
            row.set("itemType", convert.required_item_id)?;
            row.set("need", need)?;
            row.set("has", has)?;
            items.raw_push(row)?;
            out.set("items", items)?;
            out.set("enoughItems", has >= need)?;
            Ok(Some(out))
        })?,
    )?;
    let state = bridge.clone();
    converter.set(
        "ConvertItemLook",
        lua.create_function(move |_, _: Value| {
            let mut d = state.borrow_mut();
            let (Some(base), Some(look)) = (d.look.base, d.look.look) else { return Ok(()) };
            if slot(&d, base).is_none() || slot(&d, look).is_none() {
                return Ok(());
            }
            d.look.requests.push((base.uid, look.uid));
            Ok(())
        })?,
    )?;

    let item: Table = lua.globals().get("X2Item")?;
    let state = bridge.clone();
    item.set(
        "IsInConvertMode",
        lua.create_function(move |_, _: Value| Ok(state.borrow().look.mode))?,
    )?;
    let state = bridge.clone();
    item.set(
        "IsConvertible",
        lua.create_function(move |_, (_this, index): (Value, i64)| {
            let d = state.borrow();
            Ok(convertible(&d, index).is_some())
        })?,
    )?;
    lua.load(include_str!("look.lua")).set_name("@look.lua").exec()
}

pub(super) fn update(
    mut events: MessageReader<NetEvent>,
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    player: Res<Player>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(rt) = rt else {
        events.clear();
        return;
    };
    let owner = player.spawn.as_ref().map_or(0, |s| s.character().id);
    let mut done = false;
    {
        let mut d = bridge.0.borrow_mut();
        if d.look.owner != owner {
            d.look = View { owner, ..default() };
        }
        for NetEvent(event) in events.read() {
            if let Event::ItemsChanged(tasks) = event
                && tasks.reason == 111
            {
                done = true;
            }
        }
        if done {
            d.look.mode = false;
            d.look.base = None;
            d.look.look = None;
        }
    }
    if done {
        rt.fire_event("ITEM_LOOK_CONVERTED", ());
        rt.fire_event("LEAVE_ITEM_LOOK_CONVERT_MODE", ());
    }
    let requests = std::mem::take(&mut bridge.0.borrow_mut().look.requests);
    for (base_id, look_id) in requests {
        net.write(NetCommand(Command::Interact(Interact::ConvertItemLook { base_id, look_id })));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        update
            .after(super::sync_hud)
            .after(super::dialog::handle_dialog)
            .before(aa_ui::UiFrame)
            .run_if(super::hud_up)
            .run_if(in_state(Screen::InGame)),
    )
    .add_systems(OnExit(Screen::InGame), |bridge: NonSend<HudBridge>| {
        bridge.0.borrow_mut().look = default()
    });
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_database_has_item_look_converts() {
        let Ok(conn) = Connection::open_with_flags(aa_anim::db::DEFAULT_DB, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else { return };
        let t = Tables::load(&conn).unwrap();
        assert!(!t.converts.is_empty());
        assert!(t.converts.values().any(|c| c.required_item_id != 0 && c.required_item_count > 0));
    }

    #[test]
    #[ignore = "loads original world-stage assets; run in an isolated fixture process"]
    fn original_look_converter_window_and_packet() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, bridge, path, mut inventory, units, _npc) = super::super::pets::fixture::runtime(1920.0, 1080.0)?;
        let (base_tpl, look_tpl) = {
            let d = bridge.borrow();
            let game = d.game.get().ok_or("fixture game data missing")?;
            let mut converts = game
                .items
                .keys()
                .filter(|id| game.look.convert_for(**id).is_some())
                .copied()
                .collect::<Vec<_>>();
            converts.sort_unstable();
            let mut it = converts.into_iter();
            (it.next().ok_or("no base convert item")?, it.next().ok_or("no look convert item")?)
        };
        inventory.bag[0] = Some(aa_net::model::Item { id: 7001, template_id: base_tpl, count: 1, ..default() });
        inventory.bag[1] = Some(aa_net::model::Item { id: 7002, template_id: look_tpl, count: 1, ..default() });
        {
            let mut d = bridge.borrow_mut();
            let mut world = World::new();
            world.insert_resource(inventory.clone());
            d.panels = super::super::panels::refresh(&world, d.game.get(), 0, 100, d.start, &d.snapshot, Vec3::ZERO).0;
            d.panels.actions_known = true;
        }
        super::super::api::push_changes(&rt, &bridge);
        bridge.borrow_mut().look.owner = 47;
        rt.fire_event("NPC_INTERACTION_START", ("look_converter", "", "200"));
        rt.lua.load("assert(changeItemLookWindow and changeItemLookWindow:IsVisible())").exec()?;
        rt.lua
            .load(format!(
                r#"
                assert(X2Item:IsInConvertMode())
                assert(X2ItemLookConverter:SetConvertBaseItemSlotFromBag(1))
                assert(X2ItemLookConverter:SetConvertLookItemSlotFromBag(2))
                assert(X2ItemLookConverter:GetConvertBaseItemInfo().itemType=={base_tpl})
                local info=X2ItemLookConverter:GetItemLookConvertInfo()
                assert(info and info.items[1].need>0)
                X2ItemLookConverter:ConvertItemLook()
                "#
            ))
            .exec()?;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_non_send(rt)
            .insert_non_send(HudBridge(bridge.clone()))
            .insert_resource(inventory)
            .insert_resource(units)
            .insert_resource(Player { spawn: Some(super::super::expansion::fixture::spawn()) })
            .add_message::<NetEvent>()
            .add_message::<NetCommand>()
            .add_systems(Update, update);
        app.update();
        let commands = app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect::<Vec<_>>();
        assert!(matches!(
            commands.as_slice(),
            [Command::Interact(Interact::ConvertItemLook { base_id: 7001, look_id: 7002 })]
        ), "{commands:?}");
        drop(app);
        drop(bridge);
        std::fs::remove_file(path)?;
        Ok(())
    }
}
