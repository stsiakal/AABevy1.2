//! The transfer bar (`x2ui/transfer.alb`): it shows on `TRANSFER_BOARDING` (event id 0x64) and
//! hides on `TRANSFER_UNBOARDING` (0x65); its button calls `X2.UnBoardingTransfer`, which sends
//! `CSBoardingTransfer` with the boarded transfer's timeline id and `ap = 0`.
//!
//! The events follow the local player's seat: `SCUnitAttached` to a transfer (a carriage, ferry or
//! airship) shows the bar, `SCUnitDetached` hides it. The script's handler takes no arguments.

use aa_net::model::UnitKind;
use aa_net::state::{Player, Units};
use aa_net::transfer::TransferCommand;
use aa_net::{Command, NetCommand};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, MultiValue, Table};

use super::api::{HudBridge, Shared};

#[derive(Default)]
pub(super) struct View {
    /// The transfer the local player sits on (`None` while on foot or on another seat).
    attached: Option<u32>,
    /// `X2.UnBoardingTransfer` was called since the last update.
    unboard: bool,
}

/// The local player's transfer seat: `(object id, timeline id)`.
fn attached_transfer(player: &Player, units: &Units) -> Option<(u32, u16)> {
    let attached = player.spawn.as_ref()?.unit.attached_to?;
    let unit = units.get(attached.obj_id)?;
    let UnitKind::Transfer { tl_id, .. } = unit.kind else { return None };
    Some((unit.obj_id, tl_id))
}

/// One update: the bar event and the leave request the seat change and the button ask for.
fn step(view: &mut View, current: Option<(u32, u16)>, unboard: bool) -> (Option<&'static str>, Option<TransferCommand>) {
    let previous = view.attached;
    view.unboard = false;
    let event = match (previous, current.map(|c| c.0)) {
        (None, Some(_)) => Some("TRANSFER_BOARDING"),
        (Some(_), None) => Some("TRANSFER_UNBOARDING"),
        (Some(before), Some(now)) if before != now => Some("TRANSFER_BOARDING"),
        _ => None,
    };
    view.attached = current.map(|c| c.0);
    let leave = unboard.then_some(current).flatten().map(|(_, tl_id)| TransferCommand::Leave { tl_id });
    (event, leave)
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let x2: Table = lua.globals().get("X2")?;
    let state = bridge.clone();
    x2.set(
        "UnBoardingTransfer",
        lua.create_function(move |_, _: MultiValue| {
            state.borrow_mut().transfer.unboard = true;
            Ok(0)
        })?,
    )
}

pub(super) fn update(
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    player: Res<Player>,
    units: Res<Units>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(rt) = rt else { return };
    let current = attached_transfer(&player, &units);
    let (event, leave) = {
        let mut data = bridge.0.borrow_mut();
        let unboard = std::mem::take(&mut data.transfer.unboard);
        step(&mut data.transfer, current, unboard)
    };
    if let Some(command) = leave {
        net.write(NetCommand(Command::Transfer(command)));
    }
    if let Some(event) = event {
        rt.fire_event(event, ());
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        update.after(super::sync_hud).before(aa_ui::UiFrame).run_if(super::hud_up),
    )
    .add_systems(OnExit(crate::Screen::InGame), |bridge: NonSend<HudBridge>| {
        bridge.0.borrow_mut().transfer = default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::model::{Attachment, Equipment, ModelParams, Unit};
    use aa_net::PlayerSpawn;
    use aa_net::Pos;

    fn unit(kind: UnitKind) -> Unit {
        Unit {
            obj_id: 500,
            name: "Carriage".into(),
            kind,
            owner_name: String::new(),
            pos: Pos::new(0.0, 0.0, 0.0),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id: 654,
            equipment: Equipment::default(),
            model_params: ModelParams::default(),
            hp: 100,
            mp: 0,
            max_hp: 100,
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
        }
    }

    fn seated(point: u8) -> Player {
        let mut state = aa_net::model::CharacterState::default();
        state.character.id = 47;
        let mut me = unit(UnitKind::Character { char_id: 47 });
        me.obj_id = 100;
        me.attached_to = Some(Attachment { obj_id: 500, point });
        Player { spawn: Some(PlayerSpawn { obj_id: 100, state, unit: me }) }
    }

    #[test]
    fn the_bar_follows_the_players_transfer_seat() {
        let mut units = Units::default();
        units.by_id.insert(500, unit(UnitKind::Transfer { tl_id: 7, template_id: 6 }));
        let mut view = View::default();
        assert_eq!(step(&mut view, None, false), (None, None));
        assert_eq!(step(&mut view, Some((500, 7)), false), (Some("TRANSFER_BOARDING"), None));
        assert_eq!(step(&mut view, Some((500, 7)), false), (None, None), "the bar stays up");
        assert_eq!(step(&mut view, None, false), (Some("TRANSFER_UNBOARDING"), None));
        // another transfer while seated is still a boarding event for the bar
        let _ = step(&mut view, Some((500, 7)), false);
        assert_eq!(step(&mut view, Some((501, 8)), false), (Some("TRANSFER_BOARDING"), None));
        // the local seat helper only accepts transfers, not ships or mounts on the same object id
        assert_eq!(attached_transfer(&seated(2), &units), Some((500, 7)));
        let mut rider = unit(UnitKind::Slave { id: 9, tl_id: 7, template_id: 60, summoner_id: 47 });
        rider.obj_id = 500;
        units.by_id.insert(500, rider);
        assert_eq!(attached_transfer(&seated(2), &units), None);
    }

    #[test]
    fn unboarding_sends_the_seated_transfers_timeline_id_with_point_zero() {
        let current = Some((500, 7));
        let mut view = View::default();
        let _ = step(&mut view, current, false);
        assert_eq!(
            step(&mut view, current, true),
            (None, Some(TransferCommand::Leave { tl_id: 7 }))
        );
        assert!(!view.unboard, "the request is taken once");
        // the button off a transfer does nothing
        let mut view = View::default();
        assert_eq!(step(&mut view, None, true), (None, None));
    }

    #[test]
    fn the_original_button_function_queues_the_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut rt = UiRuntime::new("en_us", 1280., 720.)?;
        let bridge = Shared::default();
        super::super::api::install(&mut rt, &bridge)?;
        rt.exec("@transfer", "X2.UnBoardingTransfer()")?;
        assert!(bridge.borrow().transfer.unboard);
        Ok(())
    }
}
