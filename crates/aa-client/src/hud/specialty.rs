//! Original SC099 ratio receipt/getter. Request planning and price context are separate.
use aa_net::{Event, NetEvent};
use bevy::prelude::*;
use mlua::{Lua, MultiValue, ObjectLike, Table, Value};

use super::api::{HudBridge, Shared};
use std::collections::HashMap;
use std::time::Instant;

mod quote;
pub(super) use quote::Tables;

#[allow(dead_code)]
pub(super) mod fixture;

pub(super) struct View {
    ratio: i32,
    owner: Option<u32>,
    own_obj: u32,
    npc_templates: HashMap<u32, u32>,
    target: aa_net::state::Target,
    /// `DAT_3a5ba4c4`: when the next `CS 0x131` route-rate request is allowed.
    ratio_request_until: Option<Instant>,
}

impl Default for View {
    fn default() -> Self {
        //39991f50 loads global39ac9b74; both globals were read as100 in the
        // original StandMode client. The protected global initializer is not modeled.
        Self {
            ratio: 100,
            owner: None,
            own_obj: 0,
            npc_templates: HashMap::new(),
            target: Default::default(),
            ratio_request_until: None,
        }
    }
}

pub(super) fn install(lua: &Lua, state: &Shared) -> mlua::Result<()> {
    let store: Table = lua.globals().get("X2Store")?;
    let ratio_state = state.clone();
    store.set(
        "GetSpecialtyRatio",
        lua.create_function(move |_, _: MultiValue| {
            //394aeb80: no argument/context gate; signed CVTSI2SS before Lua number.
            Ok(f64::from(ratio_state.borrow().specialty.ratio as f32))
        })?,
    )?;
    quote::install(lua, &store, state)
}

fn error(lua: &Lua, code: i16) -> mlua::Result<()> {
    if let Some(key) = super::alerts::error_name(code) {
        let locale: Table = lua.globals().get("X2Locale")?;
        let group: Value = lua.globals().get("ERROR_MSG")?;
        let text: Value = locale.call_method("LocalizeUiText", (group, key))?;
        aa_ui::lua::fire_ui_event(lua, "SYSMSG", vec![text]);
    }
    Ok(())
}

fn observe(state: &Shared, event: &Event) {
    let mut data = state.borrow_mut();
    let view = &mut data.specialty;
    match event {
        Event::CharacterSelected(selected) => {
            view.npc_templates.clear();
            view.target = Default::default();
            view.ratio_request_until = None;
            view.owner = Some(selected.character.id);
            view.own_obj = 0;
        }
        Event::PlayerSpawn(spawn) => {
            if view.owner != Some(spawn.character().id) || (view.own_obj != 0 && view.own_obj != spawn.obj_id) {
                view.npc_templates.clear();
                view.target = Default::default();
                view.ratio_request_until = None;
            }
            view.owner = Some(spawn.character().id);
            view.own_obj = spawn.obj_id;
        }
        Event::UnitSpawned(unit) => match unit.kind {
            aa_net::model::UnitKind::Npc { template_id, .. } | aa_net::model::UnitKind::Mate { template_id, .. } => {
                view.npc_templates.insert(unit.obj_id, template_id);
            }
            _ => {
                view.npc_templates.remove(&unit.obj_id);
            }
        },
        Event::UnitsRemoved(ids) => {
            for id in ids {
                view.npc_templates.remove(id);
            }
        }
        Event::LoginSucceeded(_)
        | Event::ReadyToConnect { .. }
        | Event::LeftWorld { .. }
        | Event::Disconnected { .. }
        | Event::Kicked { .. }
        | Event::PhaseChanged(aa_net::Phase::Authenticating | aa_net::Phase::Connecting)
        | Event::GameDecodeFailed { .. }
        | Event::UnhandledGamePacket { level: 4, len: 1.., .. } => {
            // Explicit app certainty boundary; it never resets the native ratio.
            view.npc_templates.clear();
            view.owner = None;
            view.own_obj = 0;
            view.target = Default::default();
            view.ratio_request_until = None;
        }
        _ => {}
    }
    // Refund reads the actual current target (Game+1aa0), not the independently
    // retained NPC interaction manager. Apply receipts here, never post-batch state.
    view.target.apply(event);
}

/// `{ itemInfo, ratio }` rows for `specialty/info.alb`'s `SPECIALTY_RATIO_BETWEEN_INFO`.
fn rate_rows(lua: &Lua, state: &Shared, rates: &[(u32, u32)]) -> mlua::Result<Table> {
    let rows = lua.create_table()?;
    let data = state.borrow();
    let Some(game) = data.game.get() else { return Ok(rows) };
    for (index, (item, rate)) in rates.iter().enumerate() {
        let row = lua.create_table()?;
        let info = crate::hud::tips::item_tip(
            lua,
            game,
            &data.tips,
            *item,
            None,
            None,
            crate::hud::tips::Instance::default(),
        )?;
        match info {
            Some(info) => row.set("itemInfo", info)?,
            None => {
                // A row still lists the template the server named.
                let info = lua.create_table()?;
                info.set("itemType", *item)?;
                info.set("itemGrade", 0)?;
                info.set("name", "")?;
                row.set("itemInfo", info)?;
            }
        }
        row.set("ratio", *rate)?;
        rows.set(index + 1, row)?;
    }
    Ok(rows)
}

pub(super) fn receive(lua: Option<&Lua>, state: &Shared, event: &Event) {
    observe(state, event);
    match event {
        Event::SpecialtyRatio(ratio) => {
            if *ratio == 0 {
                // Original error225 is synchronous and precedes the reset to100.
                // Never hold the shared state borrow across either Lua call.
                if let Some(lua) = lua
                    && let Err(error) = error(lua, 225)
                {
                    warn!("hud: specialty error225: {error}");
                }
                state.borrow_mut().specialty.ratio = 100;
            } else {
                state.borrow_mut().specialty.ratio = *ratio;
                if let Some(lua) = lua {
                    aa_ui::lua::fire_ui_event(
                        lua,
                        "NPC_INTERACTION_START",
                        vec![Value::String(lua.create_string("specialty").expect("Lua string"))],
                    );
                }
            }
        }
        Event::SpecialtyCurrent { rates, .. } => {
            let Some(lua) = lua else { return };
            match rate_rows(lua, state, rates) {
                Ok(rows) => {
                    aa_ui::lua::fire_ui_event(lua, "SPECIALTY_RATIO_BETWEEN_INFO", vec![Value::Table(rows)]);
                }
                Err(error) => warn!("hud: specialty rate rows: {error}"),
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
