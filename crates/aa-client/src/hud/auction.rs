//! The original auction house windows over the lane's auction search/bid/post protocol.
//!
//! The scripts (`x2ui/auction`) keep every window, tab, filter and button. This module answers
//! `X2Auction` from the last received page, turns the scripts' calls into
//! `Command::AuctionForCharacter`, and raises the events the scripts already listen to:
//! `AUCTION_ITEM_PUT_UP`, `AUCTION_ITEM_SEARCH`, `AUCTION_ITEM_SEARCHED`, `AUCTION_BIDDED`,
//! `AUCTION_CANCELED`, `AUCTION_LOWEST_PRICE` and `AUCTION_ITEM_ATTACHMENT_STATE_CHANGED`.
//! Item/money settlement stays in aa-net's inventory and mail state.
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Deref;
use std::rc::Rc;

use aa_net::auction::{AuctionCommand, AuctionLot, AuctionSearch};
use aa_net::interaction::InteractEvent;
use aa_net::model::{Item, ItemDetail};
use aa_net::state::{Inventory, Player, Units};
use aa_net::{Auction, AuctioneerIds, Command, Event, NetCommand, NetEvent};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, MultiValue, ObjectLike, Table, Value};
use rusqlite::Connection;

use super::api::{HudBridge, Shared};
use super::data::{any_flag, any_int};
use super::tips::{self, Instance};

/// The server's auction option in `SCNpcInteractionSkillList` (`SkillsEnum.USE_AUCTIONEER`).
const USE_AUCTIONEER: u32 = 12083;
/// `GetSearchedItemTotalCount` is the native page bound, not a total from the lane.
const PAGE_BOUND: f64 = 50.0;
const SERVICE_RANGE: f32 = 5.0;

#[derive(Default)]
pub(super) struct Tables {
    auctioneers: HashSet<u32>,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut out = Self::default();
        let mut q = conn.prepare("SELECT id,auctioneer FROM npcs")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            if any_flag(r, 1)? {
                out.auctioneers.insert(any_int(r, 0)? as u32);
            }
        }
        Ok(out)
    }

    pub(super) fn has_service(&self, npc: u32) -> bool {
        self.auctioneers.contains(&npc)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Last {
    Search,
    MyLots,
    MyBids,
}

#[derive(Clone, Debug, Default)]
struct Condition {
    page: i32,
    min_level: u8,
    max_level: u8,
    grade: u8,
    category: i64,
    exact_match: bool,
    keyword: String,
    sort_kind: u8,
    sort_order: u8,
}

#[derive(Clone)]
struct Attached {
    /// Physical bag slot, zero based (`X2Auction` takes a one-based slot).
    slot: u8,
    item: Item,
}

struct Queued {
    owner: u32,
    auctioneer: AuctioneerIds,
    command: AuctionCommand,
}

#[derive(Default)]
struct View {
    initialized: bool,
    owner: u32,
    auctioneer: Option<(u32, u32)>,
    open: bool,
    tab: i64,
    condition: Condition,
    last: Option<Last>,
    last_page: i32,
    duration: u8,
    start_price: String,
    direct_price: String,
    attached: Option<Attached>,
    inventory: Inventory,
    received: Auction,
    requests: Vec<Queued>,
}

#[derive(Clone, Default)]
struct State(Rc<RefCell<View>>);

impl State {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(View::default())))
    }
}

fn amount_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.to_string_lossy().trim().to_owned()),
        Value::Integer(n) => Some(n.to_string()),
        Value::Number(n) if n.is_finite() => Some(format!("{}", *n as i64)),
        _ => None,
    }
}

fn parse_amount(value: &Value) -> Option<i32> {
    let text = amount_text(value)?;
    if text.contains('-') {
        return None;
    }
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() > 10 {
        return None;
    }
    digits.parse::<i64>().ok().and_then(|n| i32::try_from(n).ok())
}

fn number(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(n) => Some(*n),
        Value::Number(n) if n.is_finite() => Some(*n as i64),
        Value::String(s) => s.to_string_lossy().trim().parse().ok(),
        _ => None,
    }
}

fn effective_bid(lot: &AuctionLot) -> i32 {
    if lot.bid_price > 0 {
        lot.bid_price
    } else {
        lot.start_price
    }
}

fn grade(raw: u8) -> i64 {
    i64::from(super::store::ui_grade(raw))
}

fn wire_grade(ui: i64) -> u8 {
    // Original39449b20/394cec90: the UI grade is reduced by one; 0 is the lane's wildcard.
    ui.saturating_sub(1).clamp(0, 10) as u8
}

fn split_category(category: i64) -> (u8, u8, u8) {
    let category = category.clamp(0, 999_999);
    ((category / 10_000).min(255) as u8, ((category / 100) % 100) as u8, (category % 100) as u8)
}

fn instance(item: &Item) -> Instance {
    match &item.detail {
        ItemDetail::Equip(d) => Instance {
            durability: Some(d.durability),
            temper: (d.temper_physical, d.temper_magical),
            flags: Some(item.flags),
        },
        _ => Instance {
            flags: Some(item.flags),
            ..default()
        },
    }
}

fn item_name(bridge: &Shared, template: u32) -> Option<String> {
    bridge.borrow().game.get().and_then(|g| g.items.get(&template)).map(|i| i.named.name.clone()).filter(|n| !n.is_empty())
}

fn lot_at(view: &View, idx: i64) -> Option<&AuctionLot> {
    usize::try_from(idx).ok()?.checked_sub(1).and_then(|i| view.received.lots.get(i))
}

fn lot_by_id(view: &View, id: &str) -> Option<AuctionLot> {
    id.parse::<u64>().ok().and_then(|id| view.received.lot(id)).cloned()
}

fn service_ok(data: &super::api::HudData, units: &Units, npc: (u32, u32)) -> bool {
    data.game.get().is_some_and(|g| g.auction.has_service(npc.1))
        && units
            .get(npc.0)
            .is_some_and(|u| matches!(u.kind, aa_net::model::UnitKind::Npc { template_id, .. } if template_id == npc.1))
}

fn set_sysmsg(lua: &Lua, message: &str) {
    aa_ui::lua::fire_ui_event(lua, "SYSMSG", vec![Value::String(lua.create_string(message).unwrap())]);
}

fn set_error_code(lua: &Lua, code: i16) {
    let Some(key) = super::alerts::error_name(code) else { return };
    let Ok(locale) = lua.globals().get::<Table>("X2Locale") else { return };
    let Ok(group) = lua.globals().get::<Value>("ERROR_MSG") else { return };
    if let Ok(text) = locale.call_method::<Value>("LocalizeUiText", (group, key)) {
        aa_ui::lua::fire_ui_event(lua, "SYSMSG", vec![text]);
    }
}

fn search_command(view: &View, kind: Last, page: i32) -> Option<AuctionCommand> {
    let owner = view.owner;
    let (auctioneer, _) = view.auctioneer?;
    let context = AuctioneerIds { obj_id: auctioneer, obj_id2: auctioneer };
    Some(match kind {
        Last::Search => AuctionCommand::Search {
            auctioneer: context,
            query: AuctionSearch {
                keyword: view.condition.keyword.clone(),
                exact_match: view.condition.exact_match,
                grade: view.condition.grade,
                category_a: split_category(view.condition.category).0,
                category_b: split_category(view.condition.category).1,
                category_c: split_category(view.condition.category).2,
                page,
                client_id: 0,
                filter: 0,
                world_id: 0,
                min_item_level: view.condition.min_level,
                max_item_level: view.condition.max_level,
                sort_kind: view.condition.sort_kind,
                sort_order: view.condition.sort_order,
            },
        },
        Last::MyLots => AuctionCommand::Search {
            auctioneer: context,
            query: AuctionSearch {
                page,
                client_id: owner,
                min_item_level: view.condition.min_level,
                max_item_level: view.condition.max_level,
                grade: view.condition.grade,
                sort_kind: view.condition.sort_kind,
                sort_order: view.condition.sort_order,
                ..default()
            },
        },
        Last::MyBids => AuctionCommand::MyBids { auctioneer: context, page },
    })
}

fn queue(view: &mut View, command: AuctionCommand) -> bool {
    if view.owner == 0 {
        return false;
    }
    let Some((npc, _)) = view.auctioneer else { return false };
    if command.validate().is_err() {
        return false;
    }
    let auctioneer = AuctioneerIds { obj_id: npc, obj_id2: npc };
    let owner = view.owner;
    view.requests.push(Queued { owner, auctioneer, command });
    true
}

fn queue_search(view: &mut View, kind: Last, page: i32) -> bool {
    let Some(command) = search_command(view, kind, page) else { return false };
    if queue(view, command) {
        view.last = Some(kind);
        view.last_page = page;
        true
    } else {
        false
    }
}

fn unlock(lua: &Lua, slot: u8) {
    let Ok(bag) = lua.globals().get::<Table>("X2Bag") else { return };
    let _ = bag.call_method::<()>("UnlockSlot", usize::from(slot) + 1);
}

fn attachment_changed(lua: &Lua, attached: bool) {
    // x2ui/auction/putup.alb main.F0.F4 reads its one argument as a boolean: true reloads
    // GetAttachedItemInfo into the frame, false leaves it alone (the tab's Init clears it).
    aa_ui::lua::fire_ui_event(lua, "AUCTION_ITEM_ATTACHMENT_STATE_CHANGED", vec![Value::Boolean(attached)]);
}

fn attach(lua: &Lua, state: &State, slot: i64) -> bool {
    let Some(slot) = slot.checked_sub(1).and_then(|s| u8::try_from(s).ok()) else { return false };
    let (item, already) = {
        let view = state.0.borrow();
        (view.inventory.bag.get(usize::from(slot)).and_then(Option::as_ref).cloned(), view.attached.is_some())
    };
    let Some(item) = item else { return false };
    if already || item.flags & 1 != 0 {
        return false;
    }
    {
        let mut view = state.0.borrow_mut();
        if view.inventory.item_slot_pending(2, slot) {
            return false;
        }
        view.attached = Some(Attached { slot, item });
    }
    let locked = lua
        .globals()
        .get::<Table>("X2Bag")
        .and_then(|bag| bag.call_method::<bool>("IsLocked", usize::from(slot) + 1))
        .unwrap_or(false);
    if !locked
        && let Ok(bag) = lua.globals().get::<Table>("X2Bag")
    {
        let _ = bag.call_method::<()>("LockSlot", usize::from(slot) + 1);
    }
    attachment_changed(lua, true);
    true
}

fn detach(lua: &Lua, state: &State) -> bool {
    let slot = state.0.borrow_mut().attached.take().map(|a| a.slot);
    if let Some(slot) = slot {
        unlock(lua, slot);
        attachment_changed(lua, false);
        true
    } else {
        false
    }
}

fn search_item_info(lua: &Lua, bridge: &Shared, state: &State, idx: i64) -> mlua::Result<Option<Table>> {
    let view = state.0.borrow();
    let data = bridge.borrow();
    let Some(lot) = lot_at(&view, idx) else { return Ok(None) };
    let Some(item) = &lot.item else { return Ok(None) };
    let Some(game) = data.game.get() else { return Ok(None) };
    let Some(t) = tips::item_tip(lua, game, &data.tips, item.template_id, Some(grade(item.grade)), Some(i64::from(item.count)), instance(item))? else {
        return Ok(None);
    };
    t.set("idx", idx)?;
    t.set("articleId", lot.id.to_string())?;
    t.set("stackCount", i64::from(item.count))?;
    t.set("seller", lot.client_name.as_str())?;
    t.set("selWorldId", i64::from(lot.world_id))?;
    t.set("bidder", lot.bidder_name.as_str())?;
    t.set("bidWorldId", i64::from(lot.bid_world_id))?;
    let left = lot.end_time().map_or(0, |end| end.saturating_sub(view.received.server_time).max(0));
    t.set("leftTime", left)?;
    t.set("bidPriceStr", effective_bid(lot).to_string())?;
    t.set("depositStr", aa_net::auction::listing_fee(lot.buyout_price, lot.duration).unwrap_or(0).to_string())?;
    t.set("directPriceStr", lot.buyout_price.to_string())?;
    t.set("extra", lot.extra.to_string())?;
    Ok(Some(t))
}

fn attached_info(lua: &Lua, bridge: &Shared, state: &State) -> mlua::Result<Option<Table>> {
    let item = state.0.borrow().attached.as_ref().map(|a| a.item.clone());
    let Some(item) = item else { return Ok(None) };
    let data = bridge.borrow();
    let Some(game) = data.game.get() else { return Ok(None) };
    let Some(t) = tips::item_tip(lua, game, &data.tips, item.template_id, Some(grade(item.grade)), Some(i64::from(item.count)), instance(&item))? else {
        return Ok(None);
    };
    t.set("stack", i64::from(item.count))?;
    t.set("stackCount", i64::from(item.count))?;
    Ok(Some(t))
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let state = State::new();
    lua.set_app_data(state.clone());
    let api: Table = lua.globals().get("X2Auction")?;

    {
        let s = state.clone();
        let b = bridge.clone();
        api.set(
            "GetSearchedItemInfo",
            lua.create_function(move |lua, (_self, idx): (Value, i64)| search_item_info(lua, &b, &s, idx))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "DetachItem",
            lua.create_function(move |lua, _self: Value| {
                detach(lua, &s);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetSearchedItemPrice",
            lua.create_function(move |_, (_self, idx): (Value, i64)| {
                let view = s.0.borrow();
                Ok(match lot_at(&view, idx) {
                    Some(lot) => (Some(effective_bid(lot).to_string()), Some(lot.buyout_price.to_string())),
                    None => (None, None),
                })
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SetCurTab",
            lua.create_function(move |_, (_self, tab): (Value, i64)| {
                s.0.borrow_mut().tab = tab;
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetSearchedItemArticleId",
            lua.create_function(move |_, (_self, idx): (Value, i64)| Ok(lot_at(&s.0.borrow(), idx).map(|lot| lot.id.to_string())))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchMyBidList",
            lua.create_function(move |_, (_self, page): (Value, i64)| {
                let page = page.saturating_sub(1).clamp(0, i32::MAX as i64) as i32;
                queue_search(&mut s.0.borrow_mut(), Last::MyBids, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "ClearSearchCondition",
            lua.create_function(move |lua, _self: Value| {
                {
                    let mut view = s.0.borrow_mut();
                    view.condition = Condition::default();
                    view.last = Some(Last::Search);
                    view.last_page = 0;
                    // Native clears the attached-item price edits with the search condition.
                    view.start_price.clear();
                    view.direct_price.clear();
                }
                let _ = lua;
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetAttachedItemExist",
            lua.create_function(move |_, _self: Value| Ok(s.0.borrow().attached.is_some()))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchAuctionArticle",
            lua.create_function(move |lua, args: MultiValue| {
                let page = args.get(1).and_then(number).unwrap_or(1).saturating_sub(1).clamp(0, i32::MAX as i64) as i32;
                let min = args.get(2).and_then(number).unwrap_or(0).clamp(0, 255) as u8;
                let max = args.get(3).and_then(number).unwrap_or(0).clamp(0, 255) as u8;
                let grade = args.get(4).and_then(number).unwrap_or(1);
                let category = args.get(5).and_then(number).unwrap_or(0);
                let exact = match args.get(6) { Some(Value::Boolean(v)) => *v, Some(Value::Integer(v)) => *v != 0, Some(Value::Number(v)) => *v != 0.0, _ => false };
                let keyword = args.get(7).and_then(amount_text).unwrap_or_default();
                if (max != 0 && min > max) || keyword.len() > i16::MAX as usize {
                    set_sysmsg(lua, "The auction search condition is invalid.");
                    return Ok(());
                }
                let mut view = s.0.borrow_mut();
                view.condition = Condition {
                    page,
                    min_level: min,
                    max_level: max,
                    grade: wire_grade(grade),
                    category,
                    exact_match: exact,
                    keyword,
                    sort_kind: view.condition.sort_kind,
                    sort_order: view.condition.sort_order,
                };
                queue_search(&mut view, Last::Search, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchMyAuctionArticles",
            lua.create_function(move |_, (_self, page): (Value, i64)| {
                let page = page.saturating_sub(1).clamp(0, i32::MAX as i64) as i32;
                queue_search(&mut s.0.borrow_mut(), Last::MyLots, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchRefresh",
            lua.create_function(move |_, _self: Value| {
                let mut view = s.0.borrow_mut();
                let kind = view.last.unwrap_or(Last::Search);
                let page = view.last_page;
                queue_search(&mut view, kind, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SetOpen",
            lua.create_function(move |_, (_self, open): (Value, bool)| {
                s.0.borrow_mut().open = open;
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SetPrice",
            lua.create_function(move |_, (_self, bid, direct): (Value, Value, Value)| {
                let mut view = s.0.borrow_mut();
                view.start_price = amount_text(&bid).unwrap_or_default();
                view.direct_price = amount_text(&direct).unwrap_or_default();
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "AskAuctionArticle",
            lua.create_function(move |lua, _self: Value| {
                let command = {
                    let view = s.0.borrow();
                    let Some(attached) = &view.attached else {
                        set_sysmsg(lua, "Attach an item before posting it.");
                        return Ok(());
                    };
                    let current = view.inventory.bag.get(usize::from(attached.slot)).and_then(Option::as_ref);
                    if current.is_none_or(|item| item.id != attached.item.id) {
                        set_sysmsg(lua, "The attached item changed. Attach it again.");
                        return Ok(());
                    }
                    let start = view.start_price.parse::<i64>().ok().and_then(|n| i32::try_from(n).ok()).unwrap_or(-1);
                    let direct = view.direct_price.parse::<i64>().ok().and_then(|n| i32::try_from(n).ok()).unwrap_or(-1);
                    let command = AuctionCommand::Post {
                        auctioneer: AuctioneerIds::default(),
                        item_id: attached.item.id,
                        start_price: start,
                        buyout_price: direct,
                        duration: view.duration,
                    };
                    (command, attached.slot)
                };
                let (command, slot) = command;
                let queued = {
                    let mut view = s.0.borrow_mut();
                    let mut command = command;
                    if let AuctionCommand::Post { auctioneer, .. } = &mut command
                        && let Some((npc, _)) = view.auctioneer
                    {
                        *auctioneer = AuctioneerIds { obj_id: npc, obj_id2: npc };
                    }
                    let ok = queue(&mut view, command);
                    if ok {
                        view.attached = None;
                    }
                    ok
                };
                if queued {
                    unlock(lua, slot);
                    attachment_changed(lua, false);
                } else {
                    set_sysmsg(lua, "The auction posting condition is invalid.");
                }
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "AttachItemFromBag",
            lua.create_function(move |lua, (_self, slot): (Value, i64)| {
                attach(lua, &s, slot);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "AttachItemFromPick",
            lua.create_function(move |lua, _self: Value| {
                let slot = lua
                    .globals()
                    .get::<Table>("X2Cursor")
                    .and_then(|cursor| cursor.call_method::<i64>("GetCursorPickedBagItemIndex", ()))
                    .unwrap_or(0);
                if slot > 0 && attach(lua, &s, slot) {
                    aa_ui::lua::drag::clear(lua);
                }
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "BidAuctionArticle",
            lua.create_function(move |lua, (_self, id, money): (Value, String, Value)| {
                let command = {
                    let view = s.0.borrow();
                    let Some(lot) = lot_by_id(&view, &id) else { return Ok(()) };
                    let Some(amount) = parse_amount(&money) else {
                        set_sysmsg(lua, "Enter a bid amount.");
                        return Ok(());
                    };
                    AuctionCommand::Bid {
                        auctioneer: AuctioneerIds::default(),
                        bid: lot.bid(amount, 0, view.owner, ""),
                        lot,
                    }
                };
                if !queue(&mut s.0.borrow_mut(), command) {
                    set_sysmsg(lua, "The bid is not valid for this lot.");
                }
                Ok(())
            })?,
        )?;
    }
    {
        api.set(
            "CalcDeposit",
            lua.create_function(move |_, (_self, start, direct, duration): (Value, Value, Value, Value)| {
                let _ = parse_amount(&start);
                let direct = parse_amount(&direct).unwrap_or(0);
                let duration = number(&duration).unwrap_or(1).clamp(1, 4) as u8;
                let fee = aa_net::auction::listing_fee(direct, duration - 1).unwrap_or(0);
                Ok(fee)
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "CancelAuctionArticle",
            lua.create_function(move |lua, (_self, id): (Value, String)| {
                let command = {
                    let view = s.0.borrow();
                    let Some(lot) = lot_by_id(&view, &id) else { return Ok(()) };
                    if lot.client_id != view.owner {
                        set_sysmsg(lua, "Only the seller can cancel this lot.");
                        return Ok(());
                    }
                    AuctionCommand::Cancel { auctioneer: AuctioneerIds::default(), lot }
                };
                if !queue(&mut s.0.borrow_mut(), command) {
                    set_sysmsg(lua, "The lot cannot be canceled.");
                }
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "DirectPurchaseAuctionArticle",
            lua.create_function(move |lua, (_self, id): (Value, String)| {
                let command = {
                    let view = s.0.borrow();
                    let Some(lot) = lot_by_id(&view, &id) else { return Ok(()) };
                    if lot.buyout_price <= 0 {
                        set_sysmsg(lua, "This lot has no direct purchase price.");
                        return Ok(());
                    }
                    let bid = lot.bid(lot.buyout_price, 0, view.owner, "");
                    AuctionCommand::Bid { auctioneer: AuctioneerIds::default(), lot, bid }
                };
                if !queue(&mut s.0.borrow_mut(), command) {
                    set_sysmsg(lua, "The lot cannot be bought now.");
                }
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        let b = bridge.clone();
        api.set(
            "GetAttachedItemInfo",
            lua.create_function(move |lua, _self: Value| attached_info(lua, &b, &s))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetLinkText",
            lua.create_function(move |_, (_self, idx): (Value, i64)| {
                let view = s.0.borrow();
                let item = lot_at(&view, idx).and_then(|lot| lot.item.as_ref());
                Ok(item.and_then(super::itemlinks::link))
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetLowestPrice",
            lua.create_function(move |_, (_self, item_type, item_grade): (Value, u32, Option<i64>)| {
                let item_grade = item_grade.unwrap_or(0).clamp(0, 11) as u8;
                let mut view = s.0.borrow_mut();
                let cached = view.received.quote(item_type, item_grade).map(|q| q.amount.to_string());
                let pending = view
                    .received
                    .pending
                    .iter()
                    .any(|p| matches!(p, AuctionCommand::LowestPrice { item_template_id, grade, .. } if *item_template_id == item_type && *grade == item_grade));
                if cached.is_none()
                    && !pending
                    && let Some((npc, _)) = view.auctioneer
                {
                    let command = AuctionCommand::LowestPrice {
                        auctioneer: AuctioneerIds { obj_id: npc, obj_id2: npc },
                        item_template_id: item_type,
                        grade: item_grade,
                    };
                    let _ = queue(&mut view, command);
                }
                Ok(cached)
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetSearchedItemCount",
            lua.create_function(move |_, _self: Value| Ok(s.0.borrow().received.lots.len() as f64))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetSearchedItemPage",
            lua.create_function(move |_, _self: Value| Ok(f64::from(s.0.borrow().received.page + 1)))?,
        )?;
    }
    {
        api.set(
            "GetSearchedItemTotalCount",
            lua.create_function(move |_, _self: Value| Ok(PAGE_BOUND))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "GetSearchedSortInfo",
            lua.create_function(move |_, _self: Value| {
                let view = s.0.borrow();
                Ok((i64::from(view.condition.sort_kind), i64::from(view.condition.sort_order)))
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "HasPostAuthority",
            lua.create_function(move |_, _self: Value| Ok(s.0.borrow().auctioneer.is_some()))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "IsMyPutupArticle",
            lua.create_function(move |_, (_self, idx): (Value, i64)| {
                let view = s.0.borrow();
                Ok(lot_at(&view, idx).is_some_and(|lot| lot.client_id == view.owner))
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "RequirePostAuthority",
            lua.create_function(move |_, _self: Value| Ok(s.0.borrow().auctioneer.is_none()))?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchAuctionArticleByPage",
            lua.create_function(move |_, (_self, page): (Value, i64)| {
                let page = page.saturating_sub(1).clamp(0, i32::MAX as i64) as i32;
                let mut view = s.0.borrow_mut();
                let kind = view.last.unwrap_or(Last::Search);
                queue_search(&mut view, kind, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchDeclareSiege",
            lua.create_function(move |_, (_self, page): (Value, i64)| {
                let page = page.saturating_sub(1).clamp(0, i32::MAX as i64) as i32;
                queue_search(&mut s.0.borrow_mut(), Last::Search, page);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SearchedListSort",
            lua.create_function(move |_, (_self, sort): (Value, Value)| {
                let kind = match sort {
                    Value::String(s) => match s.to_string_lossy().as_str() {
                        "evaluated" => 1,
                        "item_name" => 2,
                        "item_level" => 3,
                        "expire_date" => 4,
                        _ => 0,
                    },
                    other => number(&other).unwrap_or(0).clamp(0, 6) as u8,
                };
                if kind == 0 {
                    return Ok(());
                }
                let mut view = s.0.borrow_mut();
                if view.condition.sort_kind == kind {
                    view.condition.sort_order ^= 1;
                } else {
                    view.condition.sort_kind = kind;
                    view.condition.sort_order = 0;
                }
                view.condition.page = 0;
                let last = view.last.unwrap_or(Last::Search);
                queue_search(&mut view, last, 0);
                Ok(())
            })?,
        )?;
    }
    {
        let s = state.clone();
        api.set(
            "SetDuration",
            lua.create_function(move |_, (_self, duration): (Value, i64)| {
                s.0.borrow_mut().duration = duration.saturating_sub(1).clamp(0, 3) as u8;
                Ok(())
            })?,
        )?;
    }
    for name in ["GetCurrencyForBid", "GetCurrencyForFee"] {
        // baselib.alb:593-598 names the bid and listing-fee currencies; this lane settles both
        // in copper (CURRENCY_GOLD).
        api.set(name, lua.create_function(move |_, _self: Value| Ok(0.0))?)?;
    }
    {
        let s = state.clone();
        let b = bridge.clone();
        api.set(
            "IsAuctionNpc",
            lua.create_function(move |lua, (_self, unit): (Value, Value)| {
                let _ = lua;
                let Some(id) = number(&unit) else { return Ok(false) };
                let Ok(id) = u32::try_from(id) else { return Ok(false) };
                let view = s.0.borrow();
                let data = b.borrow();
                Ok(view.auctioneer.is_some_and(|(npc, _)| npc == id) || data.game.get().is_some_and(|g| g.auction.has_service(id)))
            })?,
        )?;
    }
    Ok(())
}

/// The NPC service router opened the auction house for `npc` (its `NPC_INTERACTION_START("auction",
/// ...)`, the branch of the original `npc_interaction.alb`). The window itself is driven by that
/// event; this only sets the service context the `X2Auction` calls use.
pub(super) fn open_service(rt: &UiRuntime, npc: (u32, u32)) {
    if let Some(state) = rt.lua.app_data_ref::<State>().map(|s| s.clone()) {
        state.0.borrow_mut().auctioneer = Some(npc);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut events: MessageReader<NetEvent>,
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    auction: Option<Res<Auction>>,
    inventory: Option<Res<Inventory>>,
    player: Option<Res<Player>>,
    units: Option<Res<Units>>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(rt) = rt else {
        events.clear();
        return;
    };
    let Some(state) = rt.lua.app_data_ref::<State>().map(|s| s.clone()) else {
        events.clear();
        return;
    };
    let owner = player
        .as_deref()
        .and_then(|p| p.spawn.as_ref())
        .map_or(0, |spawn| spawn.character().id);
    let mut fires: Vec<(&'static str, Vec<Value>)> = Vec::new();
    let mut unlock_slots = Vec::new();
    let mut unlock_events = false;
    {
        let mut view = state.0.borrow_mut();
        if !view.initialized || view.owner != owner {
            let old_slot = view.attached.as_ref().map(|a| a.slot);
            *view = View { initialized: true, owner, ..default() };
            if let Some(slot) = old_slot {
                unlock_slots.push(slot);
            }
        }
        if let (Some(auction), true) = (auction.as_deref(), auction.as_ref().is_some_and(Res::is_changed)) {
            view.received = auction.clone();
        } else if view.received == Auction::default()
            && let Some(auction) = auction.as_deref()
        {
            view.received = auction.clone();
        }
        if let Some(inventory) = inventory.as_ref()
            && (inventory.is_changed() || view.inventory.bag.is_empty())
        {
            view.inventory = inventory.deref().clone();
        }
        // An item that left or changed in its physical slot is no longer attached.
        if let Some(attached) = view.attached.clone() {
            let current = view.inventory.bag.get(usize::from(attached.slot)).and_then(Option::as_ref);
            if current.is_none_or(|item| item.id != attached.item.id) {
                view.attached = None;
                unlock_slots.push(attached.slot);
                unlock_events = true;
            }
        }
    }
    for NetEvent(event) in events.read() {
        match event {
            Event::Interaction(InteractEvent::SkillList {
                npc_obj_id,
                skills,
                interactable,
                ..
            }) if *interactable && skills.contains(&USE_AUCTIONEER) => {
                // The NPC service router owns the interaction list and opens the auction house
                // through `open_service` (lone auctioneer or the chosen menu row); opening it
                // here too would pop the window over the greeting menu.
                if bridge.0.borrow().services.claimed(*npc_obj_id) {
                    continue;
                }
                let npc = bridge.0.borrow().dialog.npc.filter(|n| n.0 == *npc_obj_id);
                let Some(npc) = npc else { continue };
                let valid = units
                    .as_deref()
                    .is_some_and(|units| service_ok(&bridge.0.borrow(), units, npc));
                if valid {
                    let mut view = state.0.borrow_mut();
                    view.auctioneer = Some(npc);
                    fires.push(("NPC_INTERACTION_START", vec![
                        Value::String(rt.lua.create_string("auction").unwrap()),
                        Value::String(rt.lua.create_string("").unwrap()),
                        Value::String(rt.lua.create_string(npc.0.to_string()).unwrap()),
                    ]));
                }
            }
            Event::Auction(event) => match event {
                aa_net::AuctionEvent::Requested(command) => {
                    if matches!(command, AuctionCommand::Search { .. } | AuctionCommand::MyBids { .. }) {
                        fires.push(("AUCTION_ITEM_SEARCH", Vec::new()));
                    }
                }
                aa_net::AuctionEvent::Searched { error, .. } => {
                    fires.push(("AUCTION_ITEM_SEARCHED", Vec::new()));
                    if *error != 0 {
                        set_error_code(&rt.lua, *error);
                    }
                }
                aa_net::AuctionEvent::Posted(lot) => {
                    let name = lot.item.as_ref().and_then(|item| item_name(&bridge.0, item.template_id));
                    fires.push((
                        "AUCTION_ITEM_PUT_UP",
                        vec![Value::String(rt.lua.create_string(name.as_deref().unwrap_or("")).unwrap())],
                    ));
                }
                aa_net::AuctionEvent::Bid { bid, is_buyout, item_template_id } => {
                    if !is_buyout {
                        let name = item_name(&bridge.0, *item_template_id).unwrap_or_default();
                        fires.push((
                            "AUCTION_BIDDED",
                            vec![
                                Value::String(rt.lua.create_string(&name).unwrap()),
                                Value::String(rt.lua.create_string(bid.amount.to_string()).unwrap()),
                            ],
                        ));
                    }
                }
                aa_net::AuctionEvent::Canceled(lot) => {
                    let name = lot.item.as_ref().and_then(|item| item_name(&bridge.0, item.template_id));
                    fires.push((
                        "AUCTION_CANCELED",
                        vec![Value::String(rt.lua.create_string(name.as_deref().unwrap_or("")).unwrap())],
                    ));
                }
                aa_net::AuctionEvent::LowestPrice {
                    item_template_id,
                    grade,
                    amount,
                } => {
                    fires.push((
                        "AUCTION_LOWEST_PRICE",
                        vec![
                            Value::Number(f64::from(*item_template_id)),
                            Value::Number(f64::from(*grade)),
                            Value::String(rt.lua.create_string(amount.to_string()).unwrap()),
                        ],
                    ));
                }
                aa_net::AuctionEvent::RequestRejected { message, .. } => {
                    set_sysmsg(&rt.lua, message);
                }
            },
            Event::CharacterSelected(_) | Event::LeftWorld { .. } | Event::Disconnected { .. } => {
                let old = state.0.borrow_mut().attached.take().map(|a| a.slot);
                if let Some(slot) = old {
                    unlock_slots.push(slot);
                }
                state.0.borrow_mut().auctioneer = None;
                state.0.borrow_mut().open = false;
                fires.push(("NPC_INTERACTION_END", Vec::new()));
            }
            _ => {}
        }
    }
    // The service context ends when the talk does, the NPC leaves or the player walks away.
    let close = {
        let view = state.0.borrow();
        view.auctioneer.is_some_and(|npc| {
            let dialog = bridge.0.borrow().dialog.npc;
            let data = bridge.0.borrow();
            let near = units.as_deref().is_some_and(|units| {
                units.get(npc.0).is_some_and(|unit| data.pos.distance(Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z)) <= SERVICE_RANGE)
            });
            dialog != Some(npc) || !near || !service_ok(&data, units.as_deref().unwrap_or(&Units::default()), npc)
        })
    };
    if close {
        let mut view = state.0.borrow_mut();
        view.auctioneer = None;
        view.open = false;
        if let Some(attached) = view.attached.take() {
            unlock_slots.push(attached.slot);
            unlock_events = true;
        }
        drop(view);
        fires.push(("NPC_INTERACTION_END", Vec::new()));
    }
    for (event, args) in fires {
        aa_ui::lua::fire_ui_event(&rt.lua, event, args);
    }
    for slot in unlock_slots {
        unlock(&rt.lua, slot);
    }
    if unlock_events {
        attachment_changed(&rt.lua, false);
    }
    // Requests queued by this frame's callbacks (including the ones the events above raised).
    let requests = std::mem::take(&mut state.0.borrow_mut().requests);
    {
        let view = state.0.borrow();
        for queued in requests {
            if queued.owner == 0
                || queued.owner != view.owner
                || view.auctioneer.is_none()
                || queued.auctioneer.obj_id == 0
            {
                continue;
            }
            net.write(NetCommand(Command::AuctionForCharacter {
                character_id: queued.owner,
                command: queued.command,
            }));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        update
            .after(super::sync_hud)
            .before(aa_ui::UiFrame)
            .run_if(super::hud_up),
    )
    .add_systems(OnExit(crate::Screen::InGame), |rt: Option<NonSend<UiRuntime>>| {
        if let Some(rt) = rt.as_deref()
            && let Some(state) = rt.lua.app_data_ref::<State>().map(|s| s.clone())
        {
            let slots: Vec<u8> = state.0.borrow_mut().attached.take().into_iter().map(|a| a.slot).collect();
            for slot in slots {
                unlock(&rt.lua, slot);
            }
            *state.0.borrow_mut() = View::default();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::auction::AuctionQuery;

    fn setup() -> (UiRuntime, Shared, State) {
        let mut rt = UiRuntime::new("en_us", 1280.0, 720.0).unwrap();
        let bridge = Rc::new(RefCell::new(super::super::api::HudData::default()));
        let mut game = super::super::data::GameData::default();
        let info = super::super::data::ItemInfo {
            named: super::super::data::Named { name: "Test Helm".into(), icon: "icon.dds".into() },
            max_stack: 1,
            ..default()
        };
        game.items.insert(100, info);
        game.auction.auctioneers.insert(42);
        assert!(bridge.borrow().game.0.set(game).is_ok());
        super::super::api::install(&mut rt, &bridge).unwrap();
        let state = rt.lua.app_data_ref::<State>().unwrap().clone();
        (rt, bridge, state)
    }

    fn lot(id: u64) -> AuctionLot {
        lot_for(id, 100, 3)
    }

    fn lot_for(id: u64, template: u32, count: i32) -> AuctionLot {
        let mut w = aa_net::wire::Writer::new();
        w.u64(id)
            .u8(2)
            .u32(template)
            .u64(900)
            .u8(0)
            .u8(0)
            .i32(count)
            .u8(0)
            .i64(0)
            .i32(0)
            .u32(0)
            .u8(0)
            .i64(0)
            .i64(0)
            .u8(1)
            .u32(7)
            .string("Seller")
            .i32(10)
            .i32(100)
            .i64(1_700_000_000)
            .u8(0)
            .u32(0)
            .string("")
            .i32(0)
            .i32(0);
        AuctionLot::from_wire(w.as_bytes()).unwrap()
    }

    fn ready(rt: &UiRuntime, state: &State) {
        rt.lua
            .load("X2Bag={IsLocked=function() return false end,LockSlot=function() end,UnlockSlot=function() end}")
            .exec()
            .unwrap();
        let mut view = state.0.borrow_mut();
        view.initialized = true;
        view.owner = 7;
        view.auctioneer = Some((600, 42));
        view.open = true;
        view.inventory = Inventory::default();
        view.inventory.bag_slots = 8;
        view.inventory.bag.resize(8, None);
        view.received = Auction {
            page: 1,
            reported_count: 1,
            lots: vec![lot(900)],
            loaded: true,
            server_time: 1_700_003_600,
            query: Some(AuctionQuery::Search(AuctionSearch { page: 1, ..default() })),
            ..default()
        };
    }

    #[test]
    fn auction_queries_use_received_page_and_send_native_commands() -> mlua::Result<()> {
        let (rt, _, state) = setup();
        ready(&rt, &state);
        rt.lua
            .load(
                r#"
                local i=X2Auction:GetSearchedItemInfo(1)
                assert(i.articleId=='900' and i.seller=='Seller' and i.stackCount==3 and i.bidPriceStr=='10' and i.directPriceStr=='100')
                local b,d=X2Auction:GetSearchedItemPrice(1); assert(b=='10' and d=='100')
                assert(X2Auction:GetSearchedItemArticleId(1)=='900')
                assert(X2Auction:GetSearchedItemCount()==1 and X2Auction:GetSearchedItemPage()==2 and X2Auction:GetSearchedItemTotalCount()==50)
                assert(X2Auction:IsMyPutupArticle(1))
                X2Auction:SearchAuctionArticle(2,3,40,2,300,true,'helm')
                X2Auction:SearchMyAuctionArticles(1)
                X2Auction:SearchAuctionArticleByPage(3)
                X2Auction:SearchedListSort('item_level')
                X2Auction:SearchMyBidList(2)
                X2Auction:BidAuctionArticle('900','25')
                X2Auction:CancelAuctionArticle('900')
                X2Auction:SetPrice('10','100'); X2Auction:SetDuration(2)
                assert(X2Auction:CalcDeposit('10','100',2)==2)
                assert(X2Auction:HasPostAuthority() and not X2Auction:RequirePostAuthority())
                "#,
            )
            .exec()?;
        let view = state.0.borrow();
        assert!(matches!(&view.requests[0].command, AuctionCommand::Search { query, .. }
            if query.page == 1 && query.min_item_level == 3 && query.max_item_level == 40
                && query.grade == 1 && query.category_b == 3 && query.exact_match && query.keyword == "helm"));
        assert!(matches!(&view.requests[1].command, AuctionCommand::Search { query, .. } if query.client_id == 7 && query.page == 0));
        assert!(matches!(&view.requests[2].command, AuctionCommand::Search { query, .. } if query.client_id == 7 && query.page == 2));
        assert!(matches!(&view.requests[3].command, AuctionCommand::Search { query, .. } if query.sort_kind == 3 && query.sort_order == 0 && query.page == 0));
        assert!(matches!(&view.requests[4].command, AuctionCommand::MyBids { page: 1, .. }));
        assert!(matches!(&view.requests[5].command, AuctionCommand::Bid { bid, .. } if bid.amount == 25 && bid.bidder_id == 7));
        assert!(matches!(&view.requests[6].command, AuctionCommand::Cancel { lot, .. } if lot.id == 900));
        assert_eq!(view.duration, 1);
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        Ok(())
    }

    #[test]
    fn attachment_uses_physical_whole_stack_and_posting_keeps_the_item_until_sent() -> mlua::Result<()> {
        let (rt, _, state) = setup();
        ready(&rt, &state);
        let item = Item {
            id: 9_007_199_254_740_993,
            template_id: 100,
            count: 4,
            ..default()
        };
        state.0.borrow_mut().inventory.bag[4] = Some(item.clone());
        rt.lua
            .load(
                r#"
                X2Auction:AttachItemFromBag(5)
                local i=X2Auction:GetAttachedItemInfo(); assert(i and i.itemType==100 and i.stack==4)
                assert(X2Auction:GetAttachedItemExist())
                X2Auction:SetPrice('10','100'); X2Auction:SetDuration(3)
                X2Auction:AskAuctionArticle()
                "#,
            )
            .exec()?;
        let view = state.0.borrow();
        assert!(!view.attached.is_some());
        assert!(matches!(&view.requests[0].command, AuctionCommand::Post { item_id, start_price: 10, buyout_price: 100, duration: 2, .. } if *item_id == item.id));
        assert_eq!(view.inventory.bag[4].as_ref().unwrap().count, 4);
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        Ok(())
    }

    #[test]
    fn received_auction_events_raise_the_original_ui_events_after_the_auctioneer_service() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, bridge, state) = setup();
        {
            let mut data = bridge.borrow_mut();
            data.dialog.npc = Some((600, 42));
            data.pos = Vec3::ZERO;
        }
        let mut units = Units::default();
        units.by_id.insert(
            600,
            aa_net::model::Unit {
                obj_id: 600,
                name: "Auctioneer".into(),
                kind: aa_net::model::UnitKind::Npc { template_id: 42, owner_id: 0 },
                owner_name: String::new(),
                pos: aa_net::wire::Pos::default(),
                rotation: [0.0; 3],
                scale: 1.0,
                level: 1,
                model_id: 0,
                equipment: default(),
                model_params: default(),
                hp: 1,
                mp: 0,
                max_hp: 1,
                max_mp: 0,
                dead: false,
                race_gender: 0,
                movement: None,
                faction_id: 0,
                expedition_id: 0,
                appellation: 0,
                flags: 0,
                gm_modes: [0; 4],
                bounty_money: None,
                pvp_honor: 0,
                pvp_kills: 0,
                skills: Vec::new(),
                passive_buffs: Vec::new(),
                ability_exp: Vec::new(),
                active_abilities: Vec::new(),
                buffs: Vec::new(),
                anim_action: 0,
                house_posture: None,
                state_attachments: None,
                attached_to: None,
            },
        );
        rt.lua
            .load(
                r#"
                seen={}
                UIParent:SetEventHandler('NPC_INTERACTION_START',function(kind,extra,id) seen[#seen+1]=kind..':'..id end)
                UIParent:SetEventHandler('AUCTION_ITEM_SEARCHED',function() seen[#seen+1]='searched' end)
                UIParent:SetEventHandler('AUCTION_ITEM_PUT_UP',function(name) seen[#seen+1]='put:'..name end)
                UIParent:SetEventHandler('AUCTION_BIDDED',function(name,money) seen[#seen+1]='bid:'..name..':'..money end)
                UIParent:SetEventHandler('AUCTION_CANCELED',function(name) seen[#seen+1]='cancel:'..name end)
                UIParent:SetEventHandler('AUCTION_LOWEST_PRICE',function(id,grade,price) seen[#seen+1]='low:'..price end)
                UIParent:SetEventHandler('SYSMSG',function(text) seen[#seen+1]='msg:'..text end)
                "#,
            )
            .exec()?;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_non_send(rt)
            .insert_non_send(HudBridge(bridge.clone()))
            .insert_resource(units)
            .insert_resource(Inventory::default())
            .insert_resource(Auction::default())
            .add_message::<NetEvent>()
            .add_message::<NetCommand>()
            .add_systems(Update, update);
        let event = |event: Event| NetEvent(event);
        app.world_mut().write_message(event(Event::Interaction(InteractEvent::SkillList {
            npc_obj_id: 600,
            obj_id: 100,
            extra_info: 0,
            pick_id: 0,
            mouse_button: 2,
            skills: vec![USE_AUCTIONEER],
            interactable: true,
            modifier_keys: 0,
        })));
        app.update();
        assert_eq!(state.0.borrow().auctioneer, Some((600, 42)));
        app.world_mut().write_message(event(Event::Auction(aa_net::AuctionEvent::Searched {
            page: 0,
            count: 1,
            lots: vec![lot(900)],
            error: 0,
            server_time: 1_700_000_000,
        })));
        app.world_mut().write_message(event(Event::Auction(aa_net::AuctionEvent::Posted(lot(901)))));
        app.world_mut().write_message(event(Event::Auction(aa_net::AuctionEvent::Bid {
            bid: lot(900).bid(12, 0, 7, "Test"),
            is_buyout: false,
            item_template_id: 100,
        })));
        app.world_mut().write_message(event(Event::Auction(aa_net::AuctionEvent::Canceled(lot(900)))));
        app.world_mut().write_message(event(Event::Auction(aa_net::AuctionEvent::LowestPrice {
            item_template_id: 100,
            grade: 0,
            amount: 55,
        })));
        app.update();
        let seen: String = app
            .world()
            .non_send::<UiRuntime>()
            .lua
            .load("return table.concat(seen, '|')")
            .eval()?;
        for expected in [
            "auction:600",
            "searched",
            "put:Test Helm",
            "bid:Test Helm:12",
            "cancel:Test Helm",
            "low:55",
        ] {
            assert!(seen.contains(expected), "{expected} missing from {seen}");
        }
        Ok(())
    }

    #[test]
    #[ignore = "loads the original world-stage assets; run in an isolated fixture process"]
    fn original_auction_search_bid_and_putup_controls() -> Result<(), Box<dyn std::error::Error>> {
        let mut rt = UiRuntime::new("en_us", 1280.0, 720.0)?;
        rt.load_ui_texts(std::path::Path::new(aa_anim::db::DEFAULT_DB))?;
        let bridge = Rc::new(RefCell::new(super::super::api::HudData::default()));
        let mut game = super::super::data::GameData::load(aa_anim::db::DEFAULT_DB)?;
        game.auction.auctioneers.insert(42);
        let (template, name) = game
            .items
            .iter()
            .find(|(_, item)| !item.named.name.is_empty() && item.max_stack > 1)
            .map(|(id, item)| (*id, item.named.name.clone()))
            .ok_or("missing auction item fixture")?;
        {
            let mut d = bridge.borrow_mut();
            d.snapshot.unit_id = "100".into();
            d.snapshot.name = "Auction fixture".into();
            d.snapshot.level = 20;
            d.snapshot.hp = 100;
            d.snapshot.max_hp = 100;
            d.dialog.npc = Some((600, 42));
            assert!(d.game.0.set(game).is_ok());
        }
        super::super::api::install(&mut rt, &bridge)?;
        rt.run_stage(aa_ui::Stage::World);
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        super::super::api::enter_world(&rt, &bridge, &aa_net::state::Chat::default());
        let state = rt.lua.app_data_ref::<State>().unwrap().clone();
        let item = Item {
            id: 9_007_199_254_740_993,
            template_id: template,
            count: 3,
            ..default()
        };
        {
            let mut view = state.0.borrow_mut();
            view.initialized = true;
            view.owner = 7;
            view.auctioneer = Some((600, 42));
            view.open = true;
            view.inventory.bag_slots = 8;
            view.inventory.bag.resize(8, None);
            view.inventory.bag[4] = Some(item.clone());
            view.received = Auction {
                page: 0,
                lots: vec![lot_for(900, template, 3), lot_for(901, template, 3), lot_for(902, template, 3)],
                reported_count: 3,
                loaded: true,
                server_time: 1_700_000_000,
                ..default()
            };
        }
        rt.fire_event("NPC_INTERACTION_START", ("auction", "", "600"));
        rt.lua
            .load(
                r#"
                assert(auctionWindow ~= nil and auctionWindow:IsVisible())
                assert(auctionWindow.tab:GetSelectedTab()==1)
                auctionWindow.tab.window[1].searchFrame.nameEditbox:SetText('')
                "#,
            )
            .exec()?;
        super::super::expansion::fixture::click(&rt, "auctionWindow.tab.window[1].searchFrame.searchButton")?;
        assert!(
            matches!(state.0.borrow().requests.first().map(|q| &q.command), Some(AuctionCommand::Search { .. })),
            "the original search button queues an auction search"
        );
        rt.fire_event("AUCTION_ITEM_SEARCHED", ());
        rt.lua
            .load(format!(
                r#"
                local f=auctionWindow.tab.window[1]
                assert(f.listCtrl.items[1].subItems[1].itemNameLabel:GetText()==[[{name}]])
                assert(f.listCtrl.items[2].subItems[1].itemNameLabel:GetText()==[[{name}]])
                assert(f.listCtrl.items[3].subItems[1].itemNameLabel:GetText()==[[{name}]])
                "#,
            ))
            .exec()?;
        let list: Value = rt.lua.load("return auctionWindow.tab.window[1].listCtrl").eval()?;
        let list_id = aa_ui::lua::value_id(&rt.lua, &list).ok_or("auction list missing")?;
        aa_ui::lua::widget_set_selected(&rt.lua, list_id, 0, true, false);
        aa_ui::lua::flush(&rt.lua);
        rt.lua
            .load(format!(
                r#"
                local f=auctionWindow.tab.window[1]
                assert(f.moneyEditsWindow:GetAmountStr()~='0')
                AttachItemToAuction(5)
                local p=auctionWindow.tab.window[2]
                assert(p:IsVisible() and p.putupItemFrame.itemNameLabel:GetText()==[[{name}]])
                assert(X2Auction:GetAttachedItemExist())
                p.putupItemFrame.startPriceEditsWindow:SetAmountStr('10')
                p.putupItemFrame.directPriceEditsWindow:SetAmountStr('100')
                "#,
            ))
            .exec()?;
        super::super::expansion::fixture::click(&rt, "auctionWindow.tab.window[2].putupItemFrame.putupButton")?;
        // putup.alb main.F0.F7 multiplies the per-item direct price by the attached stack into
        // directBundlePrice; main.F0.F21 passes that whole-stack total to SetPrice.
        let posted = state
            .0
            .borrow()
            .requests
            .iter()
            .any(|q| matches!(q.command, AuctionCommand::Post { item_id, start_price: 10, buyout_price: 300, duration: 0, .. } if item_id == item.id));
        assert!(posted, "the original put-up button queues the exact attached item");
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        Ok(())
    }
}
