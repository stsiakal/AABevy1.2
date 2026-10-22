//! The original 1.2 house maintain window (`x2ui/housing/maintain_window.alb`) for a finished house.
//!
//! `housing_manager.alb` opens it on `HOUSE_INTERACTION_START("housing")` once `X2House:IsHouseUnderConstruction()`
//! is false (`ShowHousingMainTainWindow(true)` plus `UpdateMyHouseDefaultInfo` / `UpdateHouseDefaultInfo`). The
//! window asks `X2House` for the name, owner, permission, sale and tax state and calls it back to change them. This
//! module answers those calls from the received house state and turns the changes into the `CS*House*` packets;
//! the server's answers (`SCHouseTaxInfo`, `SCHousePermissionChanged`, `SCHouseSetForSale` ...) come back as the
//! events the manager script listens for ([`route_events`]).
//!
//! Permissions: the wire value is 0-based (private 0, guild 1, public 2, family 3) while every script table is
//! 1-based, so `GetHousePermission` answers `wire + 1` and `SetHousePermission` takes it back.

use aa_net::housing::{HouseCommand, HousingEvent};
use aa_net::model::UnitKind;
use aa_net::state::Units;
use aa_net::{Event, NetEvent};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, ObjectLike, Table, Value};

use super::api::{HudBridge, HudData, Request, Shared};
use super::houses::{HouseState, TAX_ITEM, amount, bag_count};

/// `WI_DEMOLISH` (94) owner skill: `skills.category_id = 3`, range 20 m, a 20 s cast; the server's
/// `InteractionEffect` runs `housing::demolish_by_player` on the target house.
pub const DEMOLISH_SKILL: u32 = 11404;
/// The appraisal certificate the server takes when a house is listed (`APPRAISAL_CERTIFICATE`).
pub const APPRAISAL_ITEM: u32 = 28085;
/// Copper per appraisal certificate (the server's `COPPER_PER_CERTIFICATE`: one per 100 gold).
const COPPER_PER_APPRAISAL: i64 = 1_000_000;
/// The window closes when the player walks this far from the house, metres.
const REACH: f32 = 30.0;

/// The wire permission a script index (1-based) stands for.
pub(super) fn wire_permission(index: i64) -> Option<u8> {
    u8::try_from(index.checked_sub(1)?).ok().filter(|p| *p <= 3)
}

/// Opens the maintain window of a finished house: select it, ask for its tax (the window fills the rest from the
/// snapshot) and raise the original interaction event.
pub(super) fn open(
    unit: &aa_net::Unit,
    house: &aa_net::housing::House,
    bridge: &Shared,
    rt: Option<&UiRuntime>,
    out: &mut Vec<aa_net::Command>,
) {
    {
        let mut d = bridge.borrow_mut();
        d.house.selected = Some(HouseState::of_house(house));
        d.house.maintain_house = Some(unit.obj_id);
    }
    out.push(aa_net::Command::Target { obj_id: unit.obj_id });
    out.push(aa_net::Command::House(HouseCommand::RequestTax { tl: house.snapshot.tl_id }));
    if let Some(rt) = rt {
        rt.fire_event("HOUSE_INTERACTION_START", "housing");
        // like the material window, `ShowHousingMainTainWindow(true)` has to leave the window visible
        if let Err(e) = rt.exec(
            "@house_maintain",
            "if housing.maintainWindow and not housing.maintainWindow:IsVisible() then housing.maintainWindow:Show(true) end",
        ) {
            warn!("housing: cannot show the maintain window: {e}");
        }
    }
}

fn with_selected<R>(bridge: &Shared, f: impl FnOnce(&HudData, &HouseState) -> R) -> Option<R> {
    let d = bridge.borrow();
    d.house.selected.as_ref().map(|h| f(&d, h))
}

/// `gold`, `silver`, `copper` of a price in copper.
fn split_price(price: i64) -> (i64, i64, i64) {
    (price / 10_000, price % 10_000 / 100, price % 100)
}

fn price_of(gold: &Value, silver: &Value, copper: &Value) -> i64 {
    amount(gold).max(0) * 10_000 + amount(silver).max(0) * 100 + amount(copper).max(0)
}

/// The appraisal certificates a listing at `price` copper costs (`calculate_sale_certificates`).
pub(super) fn certificates_for(price: i64) -> i64 {
    if price <= 0 {
        return 0;
    }
    ((price + COPPER_PER_APPRAISAL - 1) / COPPER_PER_APPRAISAL).max(1)
}

/// The `X2House` methods only the maintain window calls.
pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let x2: Table = lua.globals().get("X2House")?;

    let b = bridge.clone();
    x2.set(
        "SetHousePermission",
        lua.create_function(move |_, (_this, index): (Table, i64)| {
            let Some(permission) = wire_permission(index) else { return Ok(()) };
            let mut d = b.borrow_mut();
            let Some(h) = d.house.selected.as_ref().filter(|h| h.permission != permission) else { return Ok(()) };
            let tl = h.tl_id;
            info!("housing: permission of house {tl} -> {permission}");
            d.requests.push(Request::House(HouseCommand::ChangePermission { tl, permission }));
            Ok(())
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "GetHouseAllowRecover",
        lua.create_function(move |_, _this: Table| Ok(with_selected(&b, |_, h| h.allow_recover).unwrap_or(true)))?,
    )?;
    let b = bridge.clone();
    x2.set(
        "SetHouseAllowRecover",
        lua.create_function(move |_, (_this, allow): (Table, bool)| {
            let mut d = b.borrow_mut();
            // the server toggles; ask only for a change
            let Some(h) = d.house.selected.as_ref().filter(|h| h.allow_recover != allow) else { return Ok(()) };
            let tl = h.tl_id;
            info!("housing: allow recover of house {tl} -> {allow}");
            d.requests.push(Request::House(HouseCommand::AllowRecover { tl }));
            Ok(())
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "GetHouseSaleInfo",
        lua.create_function(move |lua, _this: Table| {
            let d = b.borrow();
            let out = lua.create_table()?;
            let Some(h) = d.house.selected.as_ref() else { return Ok(out) };
            let sellable = d.game.get().and_then(|g| g.housing.housings.get(&h.template_id)).is_none_or(|t| t.is_sellable);
            let price = i64::from(h.sell_price).max(0);
            let (gold, silver, copper) = split_price(price);
            out.set("canSell", sellable && !h.under_construction())?;
            out.set("onSale", price > 0)?;
            out.set("targetName", h.sell_to_name.clone())?;
            out.set("price", price)?;
            out.set("gold", gold)?;
            out.set("silver", silver)?;
            out.set("copper", copper)?;
            Ok(out)
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "GetHouseSaleItemInfo",
        lua.create_function(move |lua, (_this, gold, silver, copper): (Table, Value, Value, Value)| {
            let d = b.borrow();
            let out = lua.create_table()?;
            out.set("itemType", i64::from(APPRAISAL_ITEM))?;
            out.set("hasCount", bag_count(&d.panels.bag, APPRAISAL_ITEM))?;
            out.set("needCount", certificates_for(price_of(&gold, &silver, &copper)))?;
            Ok(out)
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "SetHouseForSale",
        lua.create_function(move |_, (_this, gold, silver, copper, target): (Table, Value, Value, Value, Option<String>)| {
            let price = price_of(&gold, &silver, &copper);
            let mut d = b.borrow_mut();
            let Some(tl) = d.house.selected.as_ref().map(|h| h.tl_id) else { return Ok(()) };
            let Some(money_amount) = u32::try_from(price).ok().filter(|p| *p > 0) else { return Ok(()) };
            info!("housing: list house {tl} for {money_amount} copper to {target:?}");
            d.requests.push(Request::House(HouseCommand::Sell { tl, money_amount, sell_to: target.unwrap_or_default() }));
            Ok(())
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "CancelHouseForSale",
        lua.create_function(move |_, _this: Table| {
            let mut d = b.borrow_mut();
            let Some(tl) = d.house.selected.as_ref().map(|h| h.tl_id) else { return Ok(()) };
            info!("housing: cancel the sale of house {tl}");
            d.requests.push(Request::House(HouseCommand::CancelSell { tl }));
            Ok(())
        })?,
    )?;
    let b = bridge.clone();
    x2.set(
        "BuyHouse",
        lua.create_function(move |_, _this: Table| {
            let mut d = b.borrow_mut();
            let Some((tl, price)) = d.house.selected.as_ref().map(|h| (h.tl_id, h.sell_price)) else { return Ok(()) };
            let Ok(money_amount) = u32::try_from(price) else { return Ok(()) };
            info!("housing: buy house {tl} for {money_amount} copper");
            d.requests.push(Request::House(HouseCommand::Buy { tl, money_amount }));
            Ok(())
        })?,
    )?;

    let b = bridge.clone();
    x2.set(
        "Demolish",
        lua.create_function(move |_, _this: Table| {
            let mut d = b.borrow_mut();
            let Some(obj_id) = d.house.selected.as_ref().map(|h| h.obj_id).filter(|id| *id != 0) else { return Ok(()) };
            info!("housing: demolish house unit {obj_id} (skill {DEMOLISH_SKILL})");
            d.requests.push(Request::Demolish { obj_id });
            Ok(())
        })?,
    )?;

    x2.set("IsSiegePeriod", lua.create_function(move |_, _this: Table| Ok(false))?)?;
    // The decoration counters: the server sends no counts, so the window hides its furniture line (the answers
    // are nil, as for a house without a decoration limit).
    x2.set("GetHouseDecoCount", lua.create_function(move |_, _this: Table| Ok(Value::Nil))?)?;
    x2.set("GetActabilityDecoCount", lua.create_function(move |_, _this: Table| Ok(Value::Nil))?)?;
    x2.set("GetDecoActabilityLimitGroup", lua.create_function(move |_, _this: Table| Ok(Value::Nil))?)?;
    Ok(())
}

/// The eight `HOUSE_TAX_INFO` arguments `housing_manager.alb` hands to `UpdateMyHouseTaxInfo`: the dominion tax rate,
/// the tax, the due date (`os.date("*t")` table), the weeks without payment, paid, heavy tax, the tax in copper
/// (counted into certificates) and the certificate item.
fn tax_args(lua: &Lua, info: &aa_net::housing::HouseTaxInfo) -> mlua::Result<mlua::MultiValue> {
    use mlua::IntoLuaMulti;
    let os: Table = lua.globals().get("os")?;
    let due: Value = match os.call_function::<Option<Table>>("date", ("*t", info.due.clamp(0, 32_535_244_799))) {
        Ok(Some(date)) => {
            // the scripts read `minute` / `second` (and `min` / `sec` from `os.date`)
            let _ = date.set("minute", date.get::<Value>("min")?);
            let _ = date.set("second", date.get::<Value>("sec")?);
            Value::Table(date)
        }
        _ => Value::Nil,
    };
    (
        i64::from(info.dominion_tax_rate),
        i64::from(info.money_amount),
        due,
        i64::from(info.weeks_without_pay),
        info.is_already_paid,
        info.is_heavy_tax_house,
        i64::from(info.money_amount),
        i64::from(TAX_ITEM),
    )
        .into_lua_multi(lua)
}

/// The server's house answers become the events the manager script listens for.
pub fn route_events(
    mut events: MessageReader<NetEvent>,
    bridge: NonSend<HudBridge>,
    houses: Option<Res<aa_net::Houses>>,
    rt: Option<NonSend<UiRuntime>>,
) {
    let Some(rt) = rt else { return };
    let Some(houses) = houses else { return };
    for NetEvent(event) in events.read() {
        let mut d = bridge.0.borrow_mut();
        let Some(selected) = d.house.selected.clone() else { continue };
        let me = d.character_id;
        let open = d.house.maintain_house.is_some();
        let tl_of = |event: &HousingEvent| match event {
            HousingEvent::TaxInfo(i) => Some(i.tl_id),
            HousingEvent::PermissionChanged { tl_id, .. }
            | HousingEvent::SetForSale(aa_net::housing::HouseSale { tl_id, .. })
            | HousingEvent::ResetForSale { tl_id, .. }
            | HousingEvent::Sold { tl_id, .. }
            | HousingEvent::OwnerNameChanged { tl_id, .. }
            | HousingEvent::Demolished { tl_id }
            | HousingEvent::MyHouseRemoved { tl_id }
            | HousingEvent::RecoverToggle { tl_id, .. } => Some(*tl_id),
            HousingEvent::State(s) | HousingEvent::MyHouse(s) => Some(s.tl_id),
            _ => None,
        };
        match event {
            Event::Housing(h) if tl_of(h) == Some(selected.tl_id) => {
                // the getters read the state the server just changed
                if let Some(house) = houses.by_tl_id.get(&selected.tl_id).filter(|h| !h.demolished) {
                    d.house.selected = Some(HouseState::of_house(house));
                }
                if !open {
                    continue;
                }
                let obj_id = selected.obj_id;
                drop(d);
                match h {
                    HousingEvent::TaxInfo(info) => match tax_args(&rt.lua, info) {
                        Ok(args) => {
                            info!("housing: tax of house {}: {} due {}", info.tl_id, info.money_amount, info.due);
                            rt.fire_event("HOUSE_TAX_INFO", args);
                        }
                        Err(e) => warn!("housing: tax info: {e}"),
                    },
                    HousingEvent::PermissionChanged { .. } => rt.fire_event("HOUSE_PERMISSION_UPDATED", ()),
                    HousingEvent::SetForSale(_) => rt.fire_event("HOUSE_SET_SELL_SUCCESS", ()),
                    HousingEvent::ResetForSale { .. } => rt.fire_event("HOUSE_CANCEL_SELL_SUCCESS", ()),
                    HousingEvent::Sold { new_owner_id, previous_owner_id, house_name, .. } => {
                        if me != 0 && *new_owner_id == me {
                            rt.fire_event("HOUSE_BUY_SUCCESS", ());
                        } else if me != 0 && *previous_owner_id == me {
                            rt.fire_event("HOUSE_SALE_SUCCESS", house_name.clone());
                        } else {
                            rt.fire_event("HOUSE_INFO_UPDATED", ());
                        }
                    }
                    HousingEvent::OwnerNameChanged { .. } => rt.fire_event("UNIT_NAME_CHANGED", obj_id.to_string()),
                    HousingEvent::RecoverToggle { .. } | HousingEvent::State(_) | HousingEvent::MyHouse(_) => {
                        rt.fire_event("HOUSE_INFO_UPDATED", ());
                    }
                    HousingEvent::Demolished { .. } | HousingEvent::MyHouseRemoved { .. } => {
                        info!("housing: house {} is gone; the maintain window closes", selected.tl_id);
                        {
                            let mut d = bridge.0.borrow_mut();
                            d.house.maintain_house = None;
                            d.house.selected = None;
                        }
                        rt.fire_event("HOUSE_INTERACTION_END", ());
                    }
                    _ => {}
                }
            }
            // a refused listing: the sell window enables its inputs again
            Event::ServerError { code } if open && super::alerts::error_name(*code).is_some_and(|n| n.starts_with("HOUSE_CANNOT_SELL")) => {
                drop(d);
                rt.fire_event("HOUSE_SET_SELL_FAIL", ());
            }
            _ => {}
        }
    }
}

/// Closes the maintain window's bookkeeping when the player closed it, and the window itself when the player walked
/// away from the house (the original ends the interaction by distance).
pub fn watch(bridge: NonSend<HudBridge>, units: Res<Units>, rt: Option<NonSend<UiRuntime>>) {
    let Some(rt) = rt else { return };
    let Some(obj_id) = bridge.0.borrow().house.maintain_house else { return };
    let visible = rt
        .lua
        .load("return housing ~= nil and housing.maintainWindow ~= nil and housing.maintainWindow:IsVisible()")
        .eval::<bool>()
        .unwrap_or(false);
    let near = {
        let d = bridge.0.borrow();
        units
            .by_id
            .get(&obj_id)
            .filter(|u| matches!(u.kind, UnitKind::Housing { .. }))
            .is_some_and(|u| d.pos.distance(Vec3::new(u.pos.x, u.pos.y, u.pos.z)) <= REACH)
    };
    if visible && near {
        return;
    }
    bridge.0.borrow_mut().house.maintain_house = None;
    if visible {
        info!("housing: walked away from house unit {obj_id}; the maintain window closes");
        rt.fire_event("HOUSE_INTERACTION_END", ());
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (route_events, watch)
            .chain()
            .after(super::sync_hud)
            .before(super::api::handle_requests)
            .run_if(super::hud_up)
            .run_if(in_state(crate::Screen::InGame)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use aa_net::housing::{House, HouseSnapshot, HouseTaxInfo};

    fn snapshot() -> HouseSnapshot {
        HouseSnapshot {
            tl_id: 13,
            db_id: 1,
            obj_id: 16211,
            template_id: 267,
            model_id: 989,
            co_owner_id: 0,
            owner_id: 9,
            owner_name: "Farmax".into(),
            account_id: 1,
            permission: 0,
            all_actions: 1,
            current_actions: 1,
            tax: 0,
            x: 0,
            y: 0,
            z: 0.0,
            name: "Scarecrow Garden".into(),
            allow_recover: true,
            sell_price: 0,
            sell_to_player_id: 0,
            sell_to_name: String::new(),
        }
    }

    fn house() -> House {
        House { snapshot: snapshot(), demolished: false, build_pay: None, tax: None, sale: None }
    }

    fn lua_with(state: HouseState) -> (Lua, Shared) {
        let lua = Lua::new();
        lua.globals().set("X2House", lua.create_table().unwrap()).unwrap();
        let bridge: Shared = Rc::new(RefCell::new(HudData::default()));
        {
            let mut d = bridge.borrow_mut();
            d.house.selected = Some(state);
            d.character_id = 9;
        }
        install(&lua, &bridge).unwrap();
        (lua, bridge)
    }

    fn finished() -> HouseState {
        HouseState::of_house(&house())
    }

    #[test]
    fn script_permissions_are_the_wire_value_plus_one() {
        assert_eq!((0..=3).map(super::super::doodad::lua_permission).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        assert_eq!(wire_permission(1), Some(0));
        assert_eq!(wire_permission(4), Some(3));
        assert_eq!(wire_permission(0), None);
        assert_eq!(wire_permission(5), None);
    }

    #[test]
    fn a_changed_permission_or_recover_flag_becomes_one_packet_and_an_unchanged_one_none() {
        let (lua, bridge) = lua_with(finished());
        // to_permission = {1, 4, 2, 3}: the family check box (index 2) is script index 4 = wire 3
        lua.load("X2House:SetHousePermission(1); X2House:SetHouseAllowRecover(true)").exec().unwrap();
        assert!(bridge.borrow().requests.is_empty(), "nothing changed");
        lua.load("X2House:SetHousePermission(4); X2House:SetHouseAllowRecover(false)").exec().unwrap();
        assert_eq!(
            bridge.borrow().requests,
            vec![
                Request::House(HouseCommand::ChangePermission { tl: 13, permission: 3 }),
                Request::House(HouseCommand::AllowRecover { tl: 13 }),
            ]
        );
    }

    #[test]
    fn selling_buying_and_demolishing_use_the_selected_house() {
        let (lua, bridge) = lua_with(finished());
        lua.load("X2House:SetHouseForSale('2', '50', '0', 'Bob'); X2House:CancelHouseForSale(); X2House:Demolish()").exec().unwrap();
        bridge.borrow_mut().house.selected.as_mut().unwrap().sell_price = 5_000_000;
        lua.load("X2House:BuyHouse()").exec().unwrap();
        assert_eq!(
            bridge.borrow().requests,
            vec![
                Request::House(HouseCommand::Sell { tl: 13, money_amount: 25_000, sell_to: "Bob".into() }),
                Request::House(HouseCommand::CancelSell { tl: 13 }),
                Request::Demolish { obj_id: 16211 },
                Request::House(HouseCommand::Buy { tl: 13, money_amount: 5_000_000 }),
            ]
        );
    }

    #[test]
    fn the_sale_info_splits_the_price_and_counts_appraisal_certificates() {
        let (lua, bridge) = lua_with(HouseState { sell_price: 123_456, sell_to_name: "Bob".into(), ..finished() });
        bridge.borrow_mut().panels.bag = vec![Some(super::super::panels::SlotSnap { id: APPRAISAL_ITEM, count: 3, ..Default::default() })];
        let (on_sale, price, gold, silver, copper, target): (bool, i64, i64, i64, i64, String) = lua
            .load("local i = X2House:GetHouseSaleInfo() return i.onSale, i.price, i.gold, i.silver, i.copper, i.targetName")
            .eval()
            .unwrap();
        assert_eq!((on_sale, price, gold, silver, copper, target.as_str()), (true, 123_456, 12, 34, 56, "Bob"));
        // 250 gold = 2,500,000 copper = 3 certificates
        let (has, need, kind): (i64, i64, i64) = lua
            .load("local i = X2House:GetHouseSaleItemInfo('250', '0', '0') return i.hasCount, i.needCount, i.itemType")
            .eval()
            .unwrap();
        assert_eq!((has, need, kind), (3, 3, i64::from(APPRAISAL_ITEM)));
        assert_eq!(certificates_for(0), 0);
        assert_eq!(certificates_for(1), 1);
        assert_eq!(certificates_for(1_000_000), 1);
        assert_eq!(certificates_for(1_000_001), 2);
    }

    #[test]
    fn the_remaining_maintain_window_getters_are_answered() {
        let (lua, _bridge) = lua_with(finished());
        let (siege, deco): (bool, Value) = lua.load("return X2House:IsSiegePeriod(), X2House:GetHouseDecoCount()").eval().unwrap();
        assert!(!siege);
        assert!(matches!(deco, Value::Nil));
    }

    #[test]
    fn the_tax_answer_becomes_the_eight_arguments_of_the_manager_event() {
        let lua = Lua::new();
        let info = HouseTaxInfo {
            tl_id: 13,
            dominion_tax_rate: 5,
            money_amount: 150_000,
            money_amount2: 0,
            due: 1_800_000_000,
            is_already_paid: true,
            weeks_without_pay: 0,
            is_heavy_tax_house: false,
        };
        let args = tax_args(&lua, &info).unwrap();
        assert_eq!(args.len(), 8);
        let values: Vec<Value> = args.into_iter().collect();
        assert!(matches!(values[0], Value::Integer(5)));
        assert!(matches!(values[1], Value::Integer(150_000)));
        let Value::Table(due) = &values[2] else { panic!("the due date is a table") };
        assert_eq!(due.get::<i64>("year").unwrap(), 2027);
        assert!(due.get::<Value>("minute").is_ok());
        assert!(matches!(values[4], Value::Boolean(true)));
        assert!(matches!(values[7], Value::Integer(i) if i == i64::from(TAX_ITEM)));
    }

    /// Wraps the manager's and the maintain window's global update functions so a Lua error in one is recorded
    /// (`call_handler` only logs it).
    fn record_errors(rt: &UiRuntime) {
        rt.lua
            .load(
                r#"
                MAINTAIN_ERRS = {}
                for _, name in ipairs({"ShowHousingMainTainWindow", "UpdateMyHouseDefaultInfo", "UpdateHouseDefaultInfo",
                    "UpdateMyHouseTaxInfo", "UpdateHouseTaxInfo", "UpdateMaintainWindowHeight", "UpdateDecoImage",
                    "UpdateSellInfo", "UpdateFurnitureCount"}) do
                  local orig = _G[name]
                  if orig then
                    _G[name] = function(...)
                      local r = {pcall(orig, ...)}
                      if not r[1] then MAINTAIN_ERRS[#MAINTAIN_ERRS + 1] = name .. ": " .. tostring(r[2]) end
                      return unpack(r, 2)
                    end
                  else
                    MAINTAIN_ERRS[#MAINTAIN_ERRS + 1] = name .. " is not defined"
                  end
                end
                "#,
            )
            .exec()
            .unwrap();
    }

    fn errors(rt: &UiRuntime) -> String {
        rt.lua.load("return table.concat(MAINTAIN_ERRS, '; ')").eval().unwrap()
    }

    fn visible(rt: &UiRuntime) -> bool {
        rt.lua.load("return housing.maintainWindow ~= nil and housing.maintainWindow:IsVisible()").eval().unwrap()
    }

    fn tax_info() -> HouseTaxInfo {
        HouseTaxInfo {
            tl_id: 13,
            dominion_tax_rate: 5,
            money_amount: 150_000,
            money_amount2: 0,
            due: 1_800_000_000,
            is_already_paid: false,
            weeks_without_pay: 0,
            is_heavy_tax_house: false,
        }
    }

    #[test]
    fn a_finished_house_opens_the_original_maintain_window_and_asks_for_its_tax() {
        let Ok((rt, bridge, path)) = super::super::breath::fixture::runtime(1280.0, 720.0) else {
            return;
        };
        record_errors(&rt);
        bridge.borrow_mut().character_id = 9;
        let (unit, house) = (
            super::super::houses::tests::unit(),
            House { snapshot: HouseSnapshot { current_actions: 1, owner_id: 9, ..snapshot() }, ..house() },
        );
        let mut commands = Vec::new();
        open(&unit, &house, &bridge, Some(&rt), &mut commands);
        assert!(
            matches!(
                commands.as_slice(),
                [aa_net::Command::Target { obj_id: 16211 }, aa_net::Command::House(HouseCommand::RequestTax { tl: 13 })]
            ),
            "{commands:?}"
        );
        assert_eq!(errors(&rt), "");
        assert!(visible(&rt), "the maintain window is up");
        assert_eq!(bridge.borrow().house.maintain_house, Some(16211));
        // the tax answer fills the window
        rt.fire_event("HOUSE_TAX_INFO", tax_args(&rt.lua, &tax_info()).unwrap());
        assert_eq!(errors(&rt), "");
        // a house of somebody else gets the other-house variant
        bridge.borrow_mut().character_id = 4;
        rt.fire_event("HOUSE_TAX_INFO", tax_args(&rt.lua, &tax_info()).unwrap());
        rt.fire_event("HOUSE_PERMISSION_UPDATED", ());
        rt.fire_event("HOUSE_INFO_UPDATED", ());
        assert_eq!(errors(&rt), "");
        rt.fire_event("HOUSE_INTERACTION_END", ());
        assert!(!visible(&rt), "the interaction end closes the window");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_server_answers_reach_the_manager_script_as_its_events() {
        let Ok((rt, bridge, path)) = super::super::breath::fixture::runtime(1280.0, 720.0) else {
            return;
        };
        record_errors(&rt);
        rt.lua
            .load(
                r#"
                SEEN = {}
                for _, name in ipairs({"HOUSE_TAX_INFO", "HOUSE_PERMISSION_UPDATED", "HOUSE_SET_SELL_SUCCESS",
                    "HOUSE_CANCEL_SELL_SUCCESS", "HOUSE_BUY_SUCCESS", "HOUSE_SALE_SUCCESS", "HOUSE_INFO_UPDATED",
                    "UNIT_NAME_CHANGED", "HOUSE_INTERACTION_END", "HOUSE_SET_SELL_FAIL"}) do
                  local w = UIParent:CreateWidget("emptywidget", "seen_" .. name, "UIParent")
                  w:SetHandler("OnEvent", function(this, event, ...) SEEN[#SEEN + 1] = event end)
                  w:RegisterEvent(name)
                end
                "#,
            )
            .exec()
            .unwrap();
        let mut houses = aa_net::Houses::default();
        houses.apply(&Event::Housing(HousingEvent::State(HouseSnapshot { current_actions: 1, owner_id: 9, ..snapshot() })));
        {
            let mut d = bridge.borrow_mut();
            d.character_id = 9;
            d.house.selected = Some(HouseState::of_house(&houses.by_tl_id[&13]));
            d.house.maintain_house = Some(16211);
        }
        let mut app = App::new();
        app.add_message::<NetEvent>()
            .insert_resource(houses)
            .insert_non_send(rt)
            .insert_non_send(HudBridge(bridge.clone()))
            .add_systems(Update, route_events);
        let send = |app: &mut App, event: HousingEvent| {
            let event = Event::Housing(event);
            app.world_mut().resource_mut::<aa_net::Houses>().apply(&event);
            app.world_mut().write_message(NetEvent(event));
            app.update();
        };
        send(&mut app, HousingEvent::PermissionChanged { tl_id: 13, permission: 2 });
        assert_eq!(bridge.borrow().house.selected.as_ref().map(|h| h.permission), Some(2), "the getters read the new state");
        send(&mut app, HousingEvent::TaxInfo(tax_info()));
        assert!(bridge.borrow().house.selected.as_ref().unwrap().tax.is_some());
        send(
            &mut app,
            HousingEvent::SetForSale(aa_net::housing::HouseSale {
                tl_id: 13,
                money_amount: 5_000_000,
                sell_to_player_id: 0,
                sell_to_name: String::new(),
                house_name: "Scarecrow Garden".into(),
            }),
        );
        assert_eq!(bridge.borrow().house.selected.as_ref().map(|h| h.sell_price), Some(5_000_000));
        send(&mut app, HousingEvent::ResetForSale { tl_id: 13, house_name: "Scarecrow Garden".into() });
        send(&mut app, HousingEvent::OwnerNameChanged { tl_id: 13, name: "Farm".into() });
        send(
            &mut app,
            HousingEvent::Sold {
                tl_id: 13,
                previous_owner_id: 9,
                new_owner_id: 4,
                new_owner_account_id: 2,
                owner_name: "Bob".into(),
                house_name: "Scarecrow Garden".into(),
            },
        );
        send(&mut app, HousingEvent::Demolished { tl_id: 13 });
        let rt = app.world().non_send::<UiRuntime>();
        let seen: String = rt.lua.load("return table.concat(SEEN, ',')").eval().unwrap();
        assert_eq!(
            seen,
            "HOUSE_PERMISSION_UPDATED,HOUSE_TAX_INFO,HOUSE_SET_SELL_SUCCESS,HOUSE_CANCEL_SELL_SUCCESS,UNIT_NAME_CHANGED,HOUSE_SALE_SUCCESS,HOUSE_INTERACTION_END"
        );
        assert_eq!(errors(rt), "");
        assert_eq!(bridge.borrow().house.maintain_house, None);
        let _ = std::fs::remove_file(path);
    }
}
