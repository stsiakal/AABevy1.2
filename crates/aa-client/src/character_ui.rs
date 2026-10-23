//! Character option/key-binding UI data over the network: `CSRequestUIData`/`CSSaveUIData` for native types 1 and 2.
//!
//! The original asks for both types when the character list arrives (`0x391eaf20` -> `0x392175f0`), copies the answers into the
//! per-character buffers (`0x391e4320`) and flushes the modified trees on a screen transition or after 500 ms (`0x392f3290`).
use std::time::Duration;

use aa_net::{Command, Event, NetCommand, NetEvent, Phase, UiDataEvent};
use bevy::prelude::*;

use crate::settings;

/// `0x392f3290`: one save per type per coalescing window.
const COALESCE: Duration = Duration::from_millis(500);

pub struct CharacterUiPlugin;

impl Plugin for CharacterUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (track, flush).chain());
    }
}

fn track(mut events: MessageReader<NetEvent>, mut net: MessageWriter<NetCommand>) {
    for NetEvent(event) in events.read() {
        match event {
            Event::CharacterList(list) => {
                for character in list {
                    for command in settings::character_ui_requests(character.id) {
                        net.write(NetCommand(Command::UiData(command)));
                    }
                }
            }
            Event::UiData(UiDataEvent::Response(response)) => {
                settings::character_ui_receive(response);
            }
            Event::CharacterSelected(state) => switch(state.character.id, &mut net),
            Event::PlayerSpawn(spawn) => switch(spawn.character().id, &mut net),
            Event::LoginSucceeded(_)
            | Event::Disconnected { .. }
            | Event::Kicked { .. }
            | Event::LeftWorld { .. }
            | Event::PhaseChanged(Phase::Authenticating | Phase::Connecting) => switch(0, &mut net),
            _ => {}
        }
    }
}

/// A screen transition flushes the modified trees before the next character's buckets apply.
fn switch(owner: u32, net: &mut MessageWriter<NetCommand>) {
    let previous = settings::character_id();
    if previous == owner {
        return;
    }
    for command in settings::character_ui_take_saves(previous) {
        net.write(NetCommand(Command::UiData(command)));
    }
    settings::set_character(owner);
}

fn flush(mut net: MessageWriter<NetCommand>) {
    let owner = settings::character_id();
    if !settings::character_ui_due(owner, COALESCE) {
        return;
    }
    for command in settings::character_ui_take_saves(owner) {
        net.write(NetCommand(Command::UiData(command)));
    }
}
