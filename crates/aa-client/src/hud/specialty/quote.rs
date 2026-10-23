//! Known-bundle Refund over actual received NPC/equipment and complete static data.
use super::*;
use crate::hud::data::{GameData, any_flag, any_int};
use aa_net::specialty::{KnownBundleQuote, quote_known_bundle};
use rusqlite::{Connection, OptionalExtension};
use std::time::{Duration, Instant};

/// `DAT_39ac9b7c`: the native minimum interval between route-rate requests (5 s).
const RATIO_REQUEST_COOLDOWN_MS: u64 = 5000;

/// A `specialties` row: the production zone group and a destination that has a vendor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SpecialtyRoute {
    row: u32,
    col: u32,
    vendor_exist: bool,
}

#[derive(Default)]
pub(in crate::hud) struct Tables {
    backpack_types: HashMap<u32, i32>,
    npc_coins: HashMap<u32, u32>,
    npc_bundles: HashMap<u32, u32>,
    profits: HashMap<(u32, u32), (i32, i32)>,
    grades: HashMap<u32, i32>,
    coin_divisor: Option<i32>,
    routes: Vec<SpecialtyRoute>,
}

impl Tables {
    pub(in crate::hud) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut tables = Self::default();
        let mut query = conn.prepare("SELECT item_id,backpack_type_id FROM item_backpacks")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tables.backpack_types.insert(any_int(row, 0)? as u32, any_int(row, 1)? as i32);
        }
        let mut query = conn.prepare("SELECT id,COALESCE(specialty_coin_id,0) FROM npcs")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tables.npc_coins.insert(any_int(row, 0)? as u32, any_int(row, 1)? as u32);
        }
        let mut query = conn.prepare("SELECT npc_id,specialty_bundle_id FROM specialty_npcs")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tables.npc_bundles.insert(any_int(row, 0)? as u32, any_int(row, 1)? as u32);
        }
        // Native vectors return the first matching item, not the last duplicate.
        let mut query = conn.prepare("SELECT id,item_id,profit,ratio,specialty_bundle_id FROM specialty_bundle_items")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let key = (any_int(row, 4)? as u32, any_int(row, 1)? as u32);
            tables
                .profits
                .entry(key)
                .or_insert((any_int(row, 2)? as i32, any_int(row, 3)? as i32));
        }
        let mut query = conn.prepare("SELECT id,refund_multiplier FROM item_grades")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tables.grades.insert(any_int(row, 0)? as u32, any_int(row, 1)? as i32);
        }
        //395720e0 indexes ID, not kind_id (whose22 entry has a different value).
        tables.coin_divisor = conn
            .query_row("SELECT value FROM content_configs WHERE id=22", [], |row| {
                Ok(any_int(row, 0)? as i32)
            })
            .optional()?;
        //394af270 /394aedd0 read the loaded `specialties` rows in id order.
        let mut query =
            conn.prepare("SELECT row_zone_group_id,col_zone_group_id,vendor_exist FROM specialties ORDER BY id")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tables.routes.push(SpecialtyRoute {
                row: any_int(row, 0)?.max(0) as u32,
                col: any_int(row, 1)?.max(0) as u32,
                vendor_exist: any_flag(row, 2)?,
            });
        }
        Ok(tables)
    }

    /// `FUN_394af270`: the distinct production zone groups (`specialties.row_zone_group_id`).
    pub(in crate::hud) fn production_zones(&self) -> Vec<u32> {
        let mut zones = Vec::new();
        for route in &self.routes {
            if !zones.contains(&route.row) {
                zones.push(route.row);
            }
        }
        zones
    }

    /// `FUN_394aedd0(from)`: the `vendor_exist` destinations of the production zone group `from`.
    pub(in crate::hud) fn sellable_zones(&self, from: u32) -> Vec<u32> {
        let mut zones = Vec::new();
        for route in self.routes.iter().filter(|r| r.row == from && r.vendor_exist) {
            if !zones.contains(&route.col) {
                zones.push(route.col);
            }
        }
        zones
    }

    /// Whether `template` is a trade pack (`item_backpacks.backpack_type_id = 3`).
    pub(in crate::hud) fn is_pack(&self, template: u32) -> bool {
        self.backpack_types.get(&template) == Some(&3)
    }

    /// The worn trade pack as `X2Equipment:GetBackPackGoodsInfo` shows it: the item tooltip table (its
    /// `lookType`, `itemGrade` and grade-scaled `refund`) when the equipped template is a type-3
    /// `item_backpacks` row with a nonzero item price.
    pub(in crate::hud) fn goods(&self, lua: &Lua, data: &crate::hud::api::HudData) -> mlua::Result<Option<Table>> {
        let Some(item) = crate::hud::enchant::equipment(data, aa_net::model::slot::BACKPACK as u8) else {
            return Ok(None);
        };
        if self.backpack_types.get(&item.template_id) != Some(&3) {
            return Ok(None);
        }
        let Some(game) = data.game.get() else { return Ok(None) };
        let Some(desc) = game.items.get(&item.template_id) else { return Ok(None) };
        if desc.price as i32 == 0 {
            return Ok(None);
        }
        crate::hud::tips::item_tip(
            lua,
            game,
            &data.tips,
            item.template_id,
            Some(i64::from(item.grade)),
            Some(i64::from(item.count)),
            crate::hud::tips::Instance {
                flags: Some(item.flags),
                ..Default::default()
            },
        )
    }

    fn quote(&self, game: &GameData, npc: u32, item: &aa_net::model::Item, ratio: i32) -> Result<Option<(i32, u32)>, i16> {
        let Some(&coin) = self.npc_coins.get(&npc) else { return Ok(None) };
        if self.backpack_types.get(&item.template_id) != Some(&3) {
            return Ok(None);
        }
        let Some(desc) = game.items.get(&item.template_id) else {
            return Ok(None);
        };
        if desc.price as i32 == 0 {
            return Ok(None);
        }
        let Some(&bundle) = self.npc_bundles.get(&npc) else {
            // Regional fallback/current-zone admission remains unprovided.
            return Ok(None);
        };
        let &(profit, profit_ratio) = self.profits.get(&(bundle, item.template_id)).ok_or(225i16)?;
        let Some(&refund_multiplier) = self.grades.get(&u32::from(item.grade)) else {
            return Ok(None);
        };
        let coin_divisor = if coin == 0 {
            None
        } else {
            let Some(divisor) = self.coin_divisor else { return Ok(None) };
            Some(divisor)
        };
        let quote = quote_known_bundle(KnownBundleQuote {
            refund: desc.refund as i32,
            refund_multiplier,
            profit,
            profit_ratio,
            received_ratio: ratio,
            // Captured original1.2 NPC factor map is empty; getter default100.
            // This is a qualified static input-copy, not a generic future default.
            npc_ratio: 100.0,
            coin_divisor,
        });
        let amount = if coin == 0 {
            quote.refund
        } else {
            let Some(amount) = quote.coin_amount else { return Ok(None) };
            amount
        };
        Ok(Some((amount, coin)))
    }
}

fn current(state: &Shared) -> Result<Option<(i32, u32)>, i16> {
    let data = state.borrow();
    // SC084 ->394270a0 writes this resolved local actor's Game+1aa0 target.
    // Native local setters also exist; this app subset needs a received target.
    let Some(actor) = data.specialty.target.obj_id else {
        return Ok(None);
    };
    let Some(&npc) = data.specialty.npc_templates.get(&actor) else {
        return Ok(None);
    };
    let Some(item) = crate::hud::enchant::equipment(&data, aa_net::model::slot::BACKPACK as u8) else {
        return Ok(None);
    };
    let Some(game) = data.game.get() else { return Ok(None) };
    game.specialties.quote(game, npc, item, data.specialty.ratio)
}

/// The NPC the sale packet names: the received target and the talked-to NPC must not disagree; either
/// one alone is the current interaction (the native sale reads the interaction manager, not the target).
fn sale_npc(data: &crate::hud::api::HudData) -> Option<u32> {
    let target = data.specialty.target.obj_id.filter(|id| *id != 0);
    let dialog = data.dialog.npc.map(|n| n.0).filter(|id| *id != 0);
    match (target, dialog) {
        (Some(target), Some(dialog)) if target == dialog => Some(target),
        (Some(target), None) => Some(target),
        (None, Some(dialog)) => Some(dialog),
        _ => None,
    }
}

/// A `{ id, name }` list as `FUN_394aec90` builds it; the window sorts by name itself.
fn zone_table(lua: &Lua, game: &GameData, zones: Vec<u32>) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    for (index, id) in zones.into_iter().enumerate() {
        let entry = lua.create_table()?;
        entry.set("id", id)?;
        let name = game.zone_groups.get(&id).map(|zone| {
            if zone.text.is_empty() {
                zone.key.as_str()
            } else {
                zone.text.as_str()
            }
        });
        entry.set("name", name.unwrap_or_default())?;
        out.set(index + 1, entry)?;
    }
    Ok(out)
}

/// `FUN_394af490`'s numeric adapter: the converted integer argument, `None` for anything else.
fn integer_arg(args: &MultiValue, index: usize) -> Option<i64> {
    match args.get(index) {
        Some(Value::Integer(n)) => Some(*n),
        Some(Value::Number(n)) if n.is_finite() => Some(*n as i64),
        _ => None,
    }
}

pub(super) fn install(lua: &Lua, store: &Table, state: &Shared) -> mlua::Result<()> {
    let equipment: Table = lua.globals().get("X2Equipment")?;
    let goods_state = state.clone();
    equipment.set(
        "GetBackPackGoodsInfo",
        lua.create_function(move |lua, _: MultiValue| {
            let data = goods_state.borrow();
            data.game.get().map_or(Ok(None), |game| game.specialties.goods(lua, &data))
        })?,
    )?;
    let production_state = state.clone();
    store.set(
        "GetProductionZoneGroups",
        lua.create_function(move |lua, _: MultiValue| {
            let data = production_state.borrow();
            let Some(game) = data.game.get() else { return lua.create_table() };
            zone_table(lua, game, game.specialties.production_zones())
        })?,
    )?;
    let sellable_state = state.clone();
    store.set(
        "GetSellableZoneGroups",
        lua.create_function(move |lua, args: MultiValue| {
            let from = integer_arg(&args, 1).unwrap_or(0).max(0) as u32;
            let data = sellable_state.borrow();
            let Some(game) = data.game.get() else { return lua.create_table() };
            zone_table(lua, game, game.specialties.sellable_zones(from))
        })?,
    )?;
    let ratio_state = state.clone();
    store.set(
        "GetSpecialtyRatioBetween",
        lua.create_function(move |_, args: MultiValue| {
            let (Some(from), Some(to)) = (integer_arg(&args, 1), integer_arg(&args, 2)) else {
                return Ok(0.0);
            };
            let mut data = ratio_state.borrow_mut();
            let now = Instant::now();
            if let Some(until) = data.specialty.ratio_request_until
                && now < until
            {
                //394fb6b0 returns the remaining milliseconds under an active cooldown.
                let remain = (until - now).as_millis().min(i32::MAX as u128) as i32;
                return Ok(f64::from(remain as f32));
            }
            data.specialty.ratio_request_until = Some(now + Duration::from_millis(RATIO_REQUEST_COOLDOWN_MS));
            data.requests.push(crate::hud::api::Request::SpecialtyRatioBetween {
                from: from as u16,
                to: to as u16,
            });
            // The request returns the full interval (`DAT_39ac9b7c`).
            Ok(f64::from(RATIO_REQUEST_COOLDOWN_MS as i32 as f32))
        })?,
    )?;
    let sale_state = state.clone();
    store.set(
        "SellBackPackGoods",
        lua.create_function(move |_, _: MultiValue| {
            let mut data = sale_state.borrow_mut();
            let Some(npc) = sale_npc(&data) else { return Ok(()) };
            let Some(item) = crate::hud::enchant::equipment(&data, aa_net::model::slot::BACKPACK as u8) else {
                return Ok(());
            };
            let sellable = data
                .game
                .get()
                .is_some_and(|game| game.specialties.is_pack(item.template_id) && game.items.get(&item.template_id).is_some_and(|d| d.price != 0));
            if !sellable {
                return Ok(());
            }
            //394fbadd resets the native ratio cache only after the sale sender.
            data.specialty.ratio = 100;
            data.requests.push(crate::hud::api::Request::SellBackpackGoods { npc_obj_id: npc });
            Ok(())
        })?,
    )?;
    let state = state.clone();
    store.set(
        "GetSpecialtyRatioRefund",
        lua.create_function(move |lua, args: MultiValue| {
            //394af190 requires numeric ScriptAny4 (including NaN/Inf). Its converted
            // f32/CVTT integer is ignored by394ae8c0; no supplied refund enters price math.
            if !matches!(args.get(1), Some(Value::Integer(_) | Value::Number(_))) {
                return Ok(MultiValue::new());
            }
            let quote = current(&state);
            match quote {
                Ok(Some((amount, coin))) => {
                    let mut values = vec![Value::Boolean(true), Value::Number(f64::from(amount as f32))];
                    if coin != 0 {
                        values.push(Value::Number(f64::from(coin as f32)));
                    }
                    Ok(MultiValue::from_vec(values))
                }
                Err(code) => {
                    super::error(lua, code)?;
                    Ok(MultiValue::from_vec(vec![Value::Boolean(false)]))
                }
                Ok(None) => Ok(MultiValue::from_vec(vec![Value::Boolean(false)])),
            }
        })?,
    )
}

#[cfg(test)]
mod tests;
