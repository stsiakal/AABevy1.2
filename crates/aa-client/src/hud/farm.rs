//! The public farm data the original `commonfarm` windows read (`X2.GetFarm*`).
//!
//! The scripts the world stage loads (`commonfarm/info.alb`, `my_farm_info.alb`) ask:
//!
//! | call | returns (the original bytecode's order) |
//! |---|---|
//! | `X2.GetCommonFarmInfo(farmType)` | `commonFarmType, name, guardTime` (ms) |
//! | `X2.GetFarmGorupInfo(farmType)` | `farmGroupType, maxCount, farmGroupName` |
//! | `X2.GetFarmGroupDoodadList(farmType)` | the names of the items the group allows |
//! | `X2.GetFarmGroups()` | `{ { name, type }, ... }` for the tabs |
//! | `X2.GetRuntimeCommonFarmDoodadsInfo(farmType)` | `{ { itemType, name, leftGrowthTime, leftProtectTime, growUp, location, farmType, vPosX, vPosY }, ... }` |
//! | `X2.GetRuntimeCommonFarmDoodadCount(farmType)` | the planted count |
//! | `X2.RequestRuntimeCommonFarmDoodadInfo()` | asks the server for `SCResponseCommonFarmList` |
//!
//! `SCShowCommonFarm` raises `OPEN_COMMON_FARM_INFO` (the farm sign's window) with its farm type and asks for the
//! list; `SCResponseCommonFarmList` fills the rows and raises `COMMON_FARM_UPDATED`, which is what `my_farm_info`
//! listens for. `itemType` is the item template from `farm_group_doodads` (the packet carries the doodad template),
//! `leftGrowthTime` / `leftProtectTime` are seconds, and `growUp` says the growth is over (`time_left == 0`).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aa_net::{Command, CommonFarmList, Farms, NetCommand, ShowCommonFarm};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table, Value};

use super::api::{HudBridge, Request};
use super::data::GameData;

/// One tab of the farm window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    pub kind: u32,
    pub name: String,
    pub max: u32,
}

/// The public farm the player is standing in (`SCShowCommonFarm`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Common {
    pub kind: u32,
    pub name: String,
    pub guard_ms: u32,
}

/// One planted crop.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub item_type: u32,
    pub name: String,
    pub left_growth_time: i64,
    pub left_protect_time: i64,
    pub grow_up: bool,
    pub location: String,
    pub farm_type: u32,
    pub v_pos_x: f32,
    pub v_pos_y: f32,
}

/// What the getters and the event tracker see.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub groups: Vec<Group>,
    pub common: Option<Common>,
    pub rows: Vec<Row>,
    /// The last show / list the scripts were told about.
    pub last_show: Option<ShowCommonFarm>,
    pub last_list: Option<CommonFarmList>,
    /// When the client last asked for the list: `FUN_39035560` waits `_DAT_3999d848` (2.0 s) between sends.
    pub last_list_request: Option<Instant>,
    /// Events waiting for the HUD to be up (`None` when there is no HUD yet).
    pub pending_open: Option<i32>,
    pub pending_update: bool,
    /// The zone the rows were built for (`world_info.zone_key`).
    pub zone_key: u32,
}

/// `farm_group_doodads` maps the packet's doodad templates to the item templates the window shows; `farm_rows` uses
/// the mappings the caller supplies so it can be tested without the database.
pub fn farm_rows(
    list: &CommonFarmList,
    mut item_of: impl FnMut(u32) -> Option<u32>,
    mut name_of: impl FnMut(u32) -> String,
    mut guard_ms: impl FnMut(u32) -> u32,
    now: i64,
) -> Vec<Row> {
    list.entries
        .iter()
        .map(|e| {
            let item_type = item_of(e.template_id).unwrap_or(0);
            let left_growth = i64::from(e.time_left) / 1000;
            let left_protect = (i64::from(guard_ms(e.farm_type)) / 1000 - (now - e.plant_time)).max(0);
            Row {
                item_type,
                name: if item_type != 0 { name_of(item_type) } else { format!("Doodad {}", e.template_id) },
                left_growth_time: left_growth,
                left_protect_time: left_protect,
                grow_up: e.time_left == 0,
                location: format!("{:.0}, {:.0}", e.pos.x, e.pos.y),
                farm_type: e.farm_type,
                v_pos_x: e.pos.x,
                v_pos_y: e.pos.y,
            }
        })
        .collect()
}

/// Seconds since the unix epoch (the server's `plant_time`).
pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// `_DAT_3999d848`: the public-farm list request interval (`FUN_39035560`).
const FARM_LIST_INTERVAL: Duration = Duration::from_secs(2);

/// Whether `CSRequestCommonFarmList` may go out now, recording the send (`FUN_39035560` refuses while the
/// elapsed constant has not passed).
pub(super) fn may_request(view: &mut View, now: Instant) -> bool {
    if view.last_list_request.is_some_and(|last| now.duration_since(last) < FARM_LIST_INTERVAL) {
        return false;
    }
    view.last_list_request = Some(now);
    true
}

fn groups(game: &GameData) -> Vec<Group> {
    let mut out: Vec<Group> = game
        .housing
        .farm_groups
        .values()
        .map(|g| Group { kind: g.id, name: g.name.clone(), max: g.count })
        .collect();
    out.sort_by_key(|g| g.kind);
    out
}

fn common_for(game: &GameData, kind: u32, zone_key: u32) -> Common {
    let name = game
        .housing
        .common_farms
        .iter()
        .find(|f| f.farm_group_id == kind && f.comments.contains(&zone_key.to_string()))
        .or_else(|| game.housing.common_farms.iter().find(|f| f.farm_group_id == kind))
        .map_or_else(|| game.housing.farm_groups.get(&kind).map_or_else(|| format!("Farm {kind}"), |g| g.name.clone()), |f| f.name.clone());
    Common { kind, name, guard_ms: game.housing.farm_guard_time(kind, zone_key) }
}

fn item_name(game: &GameData, item: u32) -> String {
    game.items.get(&item).map(|i| i.named.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Item {item}"))
}

/// Rebuilds the static groups and the current farm from the game data.
fn refresh_static(view: &mut View, game: &GameData, farms: &Farms, zone_key: u32) {
    view.groups = groups(game);
    view.zone_key = zone_key;
    let kind = farms.show.map(|s| s.farm_type).or(farms.unit_farm_type);
    view.common = kind.and_then(|kind| u32::try_from(kind).ok()).map(|kind| common_for(game, kind, zone_key));
}

/// The rows of the list, with the database mappings.
fn refresh_rows(view: &mut View, game: &GameData, farms: &Farms, now: i64) {
    let Some(list) = &farms.list else {
        view.rows.clear();
        return;
    };
    let zone = view.zone_key;
    view.rows = farm_rows(
        list,
        |doodad| game.housing.farm_group_doodads.iter().find(|d| d.doodad_id == doodad).map(|d| d.item_id),
        |item| item_name(game, item),
        |kind| game.housing.farm_guard_time(kind, zone),
        now,
    );
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, track.run_if(crate::ingame::world_screens));
}

/// Keeps [`View`] in step with aa-net's [`Farms`] and raises the original UI's events when the HUD is up.
fn track(
    farms: Res<Farms>,
    info: Option<Res<aa_net::state::WorldInfo>>,
    bridge: NonSend<HudBridge>,
    built: Res<super::HudBuilt>,
    rt: Option<NonSend<UiRuntime>>,
    mut net: MessageWriter<NetCommand>,
) {
    let zone_key = info.as_ref().map_or(0, |w| w.zone_key);
    let game = bridge.0.borrow().game.clone();
    let Some(game) = game.get() else { return };
    let mut data = bridge.0.borrow_mut();
    let view = &mut data.farm;
    let static_changed = view.groups.is_empty() || view.zone_key != zone_key;
    if static_changed || farms.is_changed() {
        refresh_static(view, game, &farms, zone_key);
        refresh_rows(view, game, &farms, now_unix());
    }
    // the world may have changed without a farm packet (a zone change): keep the guard time of the new zone
    if view.common.as_ref().is_some_and(|c| view.zone_key == zone_key && game.housing.farm_guard_time(c.kind, zone_key) != c.guard_ms) {
        if let Some(kind) = view.common.as_ref().map(|c| c.kind) {
            view.common = Some(common_for(game, kind, zone_key));
        }
        refresh_rows(view, game, &farms, now_unix());
    }
    if farms.show != view.last_show {
        view.last_show = farms.show;
        if let Some(show) = farms.show {
            view.pending_open = Some(show.farm_type);
            if may_request(view, Instant::now()) {
                net.write(NetCommand(Command::RequestCommonFarmList));
            }
        }
    }
    if farms.list != view.last_list {
        view.last_list = farms.list.clone();
        view.pending_update = true;
    }
    if !built.up {
        return;
    }
    drop(data);
    let Some(rt) = rt.as_deref() else { return };
    let mut data = bridge.0.borrow_mut();
    if let Some(kind) = data.farm.pending_open.take() {
        rt.fire_event("OPEN_COMMON_FARM_INFO", i64::from(kind));
    }
    if data.farm.pending_update {
        data.farm.pending_update = false;
        rt.fire_event("COMMON_FARM_UPDATED", ());
    }
}

/// Installs the `X2.GetFarm*` functions the original `commonfarm` scripts call.
pub fn install(lua: &Lua, bridge: &super::api::Shared) -> mlua::Result<()> {
    let x2: Table = lua.globals().get("X2")?;
    // before the database is loaded (or in a run without one) the reference capture's fake farm data stays
    let prior_groups: Option<mlua::Function> = x2.get("GetFarmGroups").ok();
    let prior_group: Option<mlua::Function> = x2.get("GetFarmGorupInfo").ok();
    let prior_doodads: Option<mlua::Function> = x2.get("GetFarmGroupDoodadList").ok();

    let state = bridge.clone();
    x2.set(
        "GetCommonFarmInfo",
        lua.create_function(move |lua, _: Value| {
            let d = state.borrow();
            let Some(c) = d.farm.common.as_ref() else { return Ok(mlua::MultiValue::new()) };
            let mut out = mlua::MultiValue::new();
            out.push_back(Value::Integer(i64::from(c.kind)));
            out.push_back(Value::String(lua.create_string(&c.name)?));
            out.push_back(Value::Integer(i64::from(c.guard_ms)));
            Ok(out)
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "GetFarmGorupInfo",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let kind = args
                .get(1)
                .and_then(|v| v.as_integer())
                .map(|i| i.max(0) as u32)
                .or_else(|| state.borrow().farm.common.as_ref().map(|c| c.kind));
            let group = {
                let d = state.borrow();
                kind.and_then(|k| d.farm.groups.iter().find(|g| g.kind == k).cloned())
            };
            let Some(g) = group else {
                if let Some(f) = &prior_group {
                    return f.call(args);
                }
                return Ok(mlua::MultiValue::new());
            };
            let mut out = mlua::MultiValue::new();
            out.push_back(Value::Integer(i64::from(g.kind)));
            out.push_back(Value::Integer(i64::from(g.max)));
            out.push_back(Value::String(lua.create_string(&g.name)?));
            Ok(out)
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "GetFarmGroupDoodadList",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let kind = args
                .get(1)
                .and_then(|v| v.as_integer())
                .map(|i| i.max(0) as u32)
                .or_else(|| state.borrow().farm.common.as_ref().map(|c| c.kind));
            let d = state.borrow();
            let Some(game) = d.game.get() else {
                drop(d);
                if let Some(f) = &prior_doodads {
                    return f.call(args);
                }
                return lua.create_table();
            };
            let mut names: Vec<String> = Vec::new();
            if let Some(kind) = kind {
                for row in game.housing.farm_group_doodads.iter().filter(|r| r.farm_group_id == kind) {
                    let name = item_name(game, row.item_id);
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            drop(d);
            let t = lua.create_table()?;
            for (i, name) in names.iter().enumerate() {
                t.set(i + 1, name.as_str())?;
            }
            Ok(t)
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "GetFarmGroups",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let empty = state.borrow().farm.groups.is_empty();
            if empty
                && let Some(f) = &prior_groups
            {
                return f.call(args);
            }
            let d = state.borrow();
            let t = lua.create_table()?;
            for (i, g) in d.farm.groups.iter().enumerate() {
                let row = lua.create_table()?;
                row.set("name", g.name.as_str())?;
                row.set("type", i64::from(g.kind))?;
                t.set(i + 1, row)?;
            }
            Ok(t)
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "GetRuntimeCommonFarmDoodadsInfo",
        lua.create_function(move |lua, (_, kind): (Value, Option<u32>)| {
            let d = state.borrow();
            let t = lua.create_table()?;
            let mut n = 0;
            for row in d.farm.rows.iter().filter(|r| kind.is_none_or(|k| r.farm_type == k)) {
                n += 1;
                let r = lua.create_table()?;
                r.set("itemType", i64::from(row.item_type))?;
                r.set("name", row.name.as_str())?;
                r.set("leftGrowthTime", row.left_growth_time)?;
                r.set("leftProtectTime", row.left_protect_time)?;
                r.set("growUp", row.grow_up)?;
                r.set("location", row.location.as_str())?;
                r.set("farmType", i64::from(row.farm_type))?;
                r.set("vPosX", row.v_pos_x)?;
                r.set("vPosY", row.v_pos_y)?;
                t.set(n, r)?;
            }
            Ok(t)
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "GetRuntimeCommonFarmDoodadCount",
        lua.create_function(move |_, (_, kind): (Value, Option<u32>)| {
            let d = state.borrow();
            Ok(d.farm.rows.iter().filter(|r| kind.is_none_or(|k| r.farm_type == k)).count())
        })?,
    )?;
    let state = bridge.clone();
    x2.set(
        "RequestRuntimeCommonFarmDoodadInfo",
        lua.create_function(move |_, _: Value| {
            state.borrow_mut().requests.push(Request::FarmList);
            Ok(())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::farm::CommonFarmEntry;
    use aa_net::Pos;

    fn entry(farm_type: u32, doodad: u32, time_left: u32, plant_time: i64) -> CommonFarmEntry {
        CommonFarmEntry { farm_type, template_id: doodad, time_left, func_group_id: 0, pos: Pos::new(100.0, 200.0, 10.0), plant_time }
    }

    #[test]
    fn rows_map_doodads_to_items_and_count_the_guard_time_from_the_plant_time() {
        let list = CommonFarmList { max_count: 10, entries: vec![entry(1, 2246, 5000, 1_000), entry(1, 367, 0, 1_000)] };
        let now = 1_000 + 3_600;
        let rows = farm_rows(
            &list,
            |doodad| match doodad {
                2246 => Some(15646),
                367 => Some(14899),
                _ => None,
            },
            |item| format!("item {item}"),
            |_| 7_200_000,
            now,
        );
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].item_type, rows[0].name.as_str()), (15646, "item 15646"));
        assert_eq!((rows[0].left_growth_time, rows[0].left_protect_time, rows[0].grow_up), (5, 3_600, false));
        assert_eq!((rows[1].left_growth_time, rows[1].left_protect_time, rows[1].grow_up), (0, 3_600, true));
        assert_eq!((rows[0].v_pos_x, rows[0].v_pos_y), (100.0, 200.0));
    }

    #[test]
    fn list_requests_wait_the_two_second_interval() {
        let mut view = View::default();
        let start = Instant::now();
        assert!(may_request(&mut view, start));
        assert!(!may_request(&mut view, start + Duration::from_millis(1_999)));
        assert!(may_request(&mut view, start + FARM_LIST_INTERVAL));
        assert!(!may_request(&mut view, start + FARM_LIST_INTERVAL + Duration::from_millis(1)));
    }

    #[test]
    fn an_unknown_doodad_still_shows_a_row() {
        let list = CommonFarmList { max_count: 1, entries: vec![entry(2, 999, 1_000, 0)] };
        let rows = farm_rows(&list, |_| None, |_| String::new(), |_| 0, 0);
        assert_eq!(rows[0].item_type, 0);
        assert_eq!(rows[0].name, "Doodad 999");
        assert_eq!(rows[0].left_growth_time, 1);
    }
}
