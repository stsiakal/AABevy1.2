//! The original social slash functions: `slash_commands` rows of type `SlashFunction`, matched and
//! dispatched as `FUN_393c3bc0`/`FUN_393c1ec0` do with the `FUN_393c8bd0` handler table.
//!
//! A line matches when it starts with `/`, its first word is one of the row's aliases (case
//! insensitive) and the alias is followed by a space or the end of the line. The rest of the line
//! is the command's argument string. Party, expedition, family and friend actions go through the
//! same native checks and request queues as the script APIs; dice and the user chat channels send
//! the packets the original handlers send (`0x0bd`, `0x061`, `0x062`).

use std::collections::HashMap;

use aa_net::model::chat_type;
use aa_net::party::PartyCommand;
use aa_net::state::ChatChannel;
use aa_net::{Command, FriendCommand};
use bevy::prelude::*;
use mlua::{Function, Lua, Table, Value};

use super::api::HudData;

/// `slash_functions` plus the command aliases of `slash_commands`.
#[derive(Clone, Debug, Default)]
pub(super) struct Tables {
    pub rows: Vec<Row>,
    /// `slash_function_id` -> its original comment. A row without a function does not dispatch.
    pub functions: HashMap<i32, String>,
}

#[derive(Clone, Debug)]
pub(super) struct Row {
    pub action_id: i32,
    pub action_type: String,
    pub aliases: Vec<String>,
}

impl Tables {
    pub(super) fn load(conn: &rusqlite::Connection) -> rusqlite::Result<Tables> {
        let mut functions = HashMap::new();
        let mut q = conn.prepare("SELECT slash_func_id, comments FROM slash_functions")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            functions.insert(r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default());
        }
        let mut out = Vec::new();
        let mut q = conn.prepare(
            "SELECT action_id, action_type, command_list FROM slash_commands ORDER BY id",
        )?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            let list: Option<String> = r.get(2)?;
            out.push(Row {
                action_id: r.get(0)?,
                action_type: r.get(1)?,
                aliases: list
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
            });
        }
        Ok(Tables {
            rows: out,
            functions,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Resolved<'a> {
    pub action: i32,
    pub args: &'a str,
}

/// The slash function `line` names, with the argument string the handler receives.
pub(super) fn resolve<'a>(tables: &Tables, line: &'a str) -> Option<Resolved<'a>> {
    let rest = line.strip_prefix('/')?;
    let (word, args) = match rest.split_once(' ') {
        Some((word, args)) => (word, args.trim_start_matches(' ')),
        None => (rest, ""),
    };
    if word.is_empty() {
        return None;
    }
    tables.rows.iter().find_map(|row| {
        (row.action_type == "SlashFunction"
            && tables.functions.contains_key(&row.action_id)
            && row.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(word)))
        .then_some(Resolved {
            action: row.action_id,
            args,
        })
    })
}

/// The unit the player has selected, when it is in view.
#[derive(Clone, Copy, Debug)]
pub(super) struct Target<'a> {
    pub name: &'a str,
    pub is_player: bool,
}

/// A client-side refusal, shown the way the original shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Notice {
    /// An `ErrorMessageType` (the original's `FUN_39425f50`/`FUN_39428370`).
    Error(i16),
    /// A `COMMUNITY_TEXT` key (the original's `FUN_393032f0`), with its `$1`.
    Community {
        key: &'static str,
        arg: Option<String>,
    },
    /// `/block` without a name: `FUN_39303c50` opens the blocked-user list.
    OpenBlockList,
}

#[derive(Default)]
pub(super) struct Outcome {
    pub notice: Option<Notice>,
    pub commands: Vec<Command>,
}

impl Outcome {
    fn error(code: i16) -> Outcome {
        Outcome {
            notice: Some(Notice::Error(code)),
            commands: Vec::new(),
        }
    }

    fn community(key: &'static str, arg: Option<String>) -> Outcome {
        Outcome {
            notice: Some(Notice::Community { key, arg }),
            commands: Vec::new(),
        }
    }
}

const TEAM_NO_RIGHTS: i16 = 0x48;
const TEAM_FULL: i16 = 0x49;
const TEAM_YOURSELF: i16 = 0x54;
const CHAT_NOT_IN_PARTY: i16 = 0x10b;
const CHAT_NOT_IN_RAID: i16 = 0x10c;
const CHAT_ALREADY_JOINED: i16 = 0x10f;
const EXPEDITION_NO_TARGET: i16 = 0x1b3;
const CANNOT_ADD_FRIEND_SELF: i16 = 0x1f5;
const CANNOT_ADD_EXISTING_MEMBER: i16 = 0x229;

fn not_in_team(is_party: bool) -> i16 {
    if is_party {
        CHAT_NOT_IN_PARTY
    } else {
        CHAT_NOT_IN_RAID
    }
}

fn selected<'a>(args: &'a str, target: Option<Target<'a>>) -> (&'a str, bool) {
    if args.is_empty() {
        (target.map_or("", |t| t.name), true)
    } else {
        (args, false)
    }
}

fn party_invite(data: &HudData, name: &str, is_party: bool) -> Result<PartyCommand, i16> {
    let party = &data.party;
    if name.is_empty() {
        return Err(EXPEDITION_NO_TARGET);
    }
    if data.character_id != 0 && name.eq_ignore_ascii_case(&data.snapshot.name) {
        return Err(TEAM_YOURSELF);
    }
    if party.id != 0 {
        if party.is_party != is_party {
            return Err(not_in_team(is_party));
        }
        if party.owner_id != data.character_id {
            return Err(TEAM_NO_RIGHTS);
        }
        let capacity = if is_party { 5 } else { 50 };
        if party.members.len() >= capacity {
            return Err(TEAM_FULL);
        }
    }
    Ok(PartyCommand::Invite {
        team_id: party.id,
        is_party,
        target_name: name.to_string(),
    })
}

fn party_leave(data: &HudData, is_party: bool) -> Result<PartyCommand, i16> {
    let party = &data.party;
    if party.id == 0 || party.is_party != is_party {
        return Err(not_in_team(is_party));
    }
    Ok(PartyCommand::Leave {
        team_id: party.id,
        character_id: data.character_id,
    })
}

fn party_kick(data: &HudData, name: &str, is_party: bool) -> Result<Option<PartyCommand>, i16> {
    let party = &data.party;
    if name.is_empty() {
        return Err(EXPEDITION_NO_TARGET);
    }
    if party.id == 0 {
        // `FUN_39306190` finds no member without a team: the native kick is silent.
        return Ok(None);
    }
    if party.is_party != is_party {
        return Err(not_in_team(is_party));
    }
    let Some(member) = party
        .members
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(name))
    else {
        return Ok(None);
    };
    if member.id == data.character_id {
        return Ok(None);
    }
    if party.owner_id != data.character_id {
        return Err(TEAM_NO_RIGHTS);
    }
    Ok(Some(PartyCommand::Kick {
        team_id: party.id,
        member_id: member.id,
    }))
}

fn friend_add(data: &HudData, name: &str) -> Result<FriendCommand, Notice> {
    if data.character_id != 0 && name.eq_ignore_ascii_case(&data.snapshot.name) {
        return Err(Notice::Error(CANNOT_ADD_FRIEND_SELF));
    }
    if data
        .friends
        .friends
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(name))
    {
        return Err(Notice::Error(CANNOT_ADD_EXISTING_MEMBER));
    }
    Ok(FriendCommand::Add {
        name: name.to_string(),
    })
}

fn friend_block(data: &HudData, name: &str) -> Result<Option<FriendCommand>, Notice> {
    if name.is_empty() {
        // `FUN_39303c50` opens the block list.
        return Ok(None);
    }
    if data.friends.is_blocked(name) {
        return Err(Notice::Error(CANNOT_ADD_EXISTING_MEMBER));
    }
    if data.character_id != 0 && name.eq_ignore_ascii_case(&data.snapshot.name) {
        return Err(Notice::Community {
            key: "cannot_block_self",
            arg: None,
        });
    }
    Ok(Some(FriendCommand::Block {
        name: name.to_string(),
    }))
}

fn friend_unblock(data: &HudData, name: &str) -> Result<FriendCommand, Notice> {
    if !data.friends.is_blocked(name) {
        return Err(Notice::Community {
            key: "user_not_in_list",
            arg: Some(name.to_string()),
        });
    }
    Ok(FriendCommand::Unblock {
        name: name.to_string(),
    })
}

/// `/dice [max]`: no or invalid argument means 100; values outside `2..=1000000` are ignored.
fn dice(args: &str) -> Option<i32> {
    if args.is_empty() {
        return Some(100);
    }
    match args.parse::<i32>() {
        Ok(max) => (2..=1_000_000).contains(&max).then_some(max),
        Err(_) => Some(100),
    }
}

fn channel_name(name: &str) -> String {
    let mut end = name.len().min(48);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

fn channel_identity(channel: &ChatChannel) -> i64 {
    ((u64::from(channel.faction_id) << 32)
        | (u64::from(channel.sub_type as u16) << 16)
        | u64::from(channel.chat_type as u16)) as i64
}

/// Carry out `line` when it is a slash function. `None` means the line is ordinary chat.
pub(super) fn run(
    data: &mut HudData,
    line: &str,
    target: Option<Target<'_>>,
    channels: &[ChatChannel],
) -> Option<Outcome> {
    let resolved = {
        let tables = &data.game.get()?.slash;
        resolve(tables, line)?
    };
    let args = resolved.args;
    let target_is_player = target.is_some_and(|t| t.is_player);
    let current_party = data.party.id == 0 || data.party.is_party;
    match resolved.action {
        // `/invite` (`FUN_393c2a10`): the current team type, the target when no name is typed.
        1 => {
            let (name, from_target) = selected(args, target);
            let invalid_target = from_target && !target_is_player;
            if invalid_target {
                return Some(Outcome::error(EXPEDITION_NO_TARGET));
            }
            match party_invite(data, name, current_party) {
                Ok(command) => Some(Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                }),
                Err(code) => Some(Outcome::error(code)),
            }
        }
        // `/leave` (`FUN_393c1d90`): the current team type.
        2 => Some(
            match party_leave(data, current_party) {
                Ok(command) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Err(code) => Outcome::error(code),
            },
        ),
        // `/kick` (`FUN_393c2a30`): the current team type.
        3 => {
            let (name, _) = selected(args, target);
            Some(match party_kick(data, name, current_party) {
                Ok(Some(command)) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Ok(None) => Outcome::default(),
                Err(code) => Outcome::error(code),
            })
        }
        // `/create_channel` (`FUN_393c3810(..., 1)`).
        4 => Some(join_channel(args, channels, true)),
        // `/enter_channel` (`FUN_393c3810(..., 0)`).
        5 => Some(join_channel(args, channels, false)),
        // `/leave_channel` (`FUN_393c38b0`).
        6 => Some(leave_channel(args, channels)),
        // `/dice` (`FUN_393c34d0`).
        7 => Some(match dice(args) {
            Some(max) => Outcome {
                notice: None,
                commands: vec![Command::RollDice { max }],
            },
            None => Outcome::default(),
        }),
        // `/expeditioninvite` (`FUN_393c2e10`).
        12 => {
            let (name, from_target) = selected(args, target);
            if name.is_empty() || (from_target && !target_is_player) {
                Some(Outcome::error(EXPEDITION_NO_TARGET))
            } else {
                Some(super::expedition::slash_request(
                    &mut data.expedition,
                    0,
                    name.as_bytes(),
                )
                .map_or_else(Outcome::error, |()| Outcome::default()))
            }
        }
        // `/expeditionleave` (`FUN_393c2e30`): the native leave/dismiss task.
        13 => Some(
            super::expedition::slash_request(&mut data.expedition, 1, &[])
                .map_or_else(Outcome::error, |()| Outcome::default()),
        ),
        // `/block` (`FUN_39303c50`).
        15 => Some(match friend_block(data, args) {
            Ok(Some(command)) => Outcome {
                notice: None,
                commands: vec![Command::Friend(command)],
            },
            Ok(None) => Outcome {
                notice: Some(Notice::OpenBlockList),
                commands: Vec::new(),
            },
            Err(notice) => Outcome {
                notice: Some(notice),
                commands: Vec::new(),
            },
        }),
        // `/unblock` (`FUN_39303d20`).
        16 => Some(match friend_unblock(data, args) {
            Ok(command) => Outcome {
                notice: None,
                commands: vec![Command::Friend(command)],
            },
            Err(notice) => Outcome {
                notice: Some(notice),
                commands: Vec::new(),
            },
        }),
        // `/family_invite` (`FUN_393c86c0`): name and title are two tokens.
        18 => Some(family_action(data, 0, args, "invite_syntax")),
        // `/family_leave` (`FUN_393c2da0`).
        19 => Some(
            super::family::slash_request(&mut data.family, 1, &[], &[])
                .map_or_else(Outcome::error, |()| Outcome::default()),
        ),
        // `/family_kick` (`FUN_393c2dc0`): one name.
        21 => Some(
            super::family::slash_request(&mut data.family, 2, args.as_bytes(), &[])
                .map_or_else(Outcome::error, |()| Outcome::default()),
        ),
        // `/family_title` (`FUN_393c8830`): name and title are two tokens.
        22 => Some(family_action(data, 3, args, "title_syntax")),
        // `/raidinvite` (`FUN_393c2a50`).
        23 => {
            let (name, from_target) = selected(args, target);
            if from_target && !target_is_player {
                return Some(Outcome::error(EXPEDITION_NO_TARGET));
            }
            match party_invite(data, name, false) {
                Ok(command) => Some(Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                }),
                Err(code) => Some(Outcome::error(code)),
            }
        }
        // `/raidleave` (`FUN_393c1dd0`).
        24 => Some(
            match party_leave(data, false) {
                Ok(command) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Err(code) => Outcome::error(code),
            },
        ),
        // `/raidkick` (`FUN_393c2a70`).
        25 => {
            let (name, _) = selected(args, target);
            Some(match party_kick(data, name, false) {
                Ok(Some(command)) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Ok(None) => Outcome::default(),
                Err(code) => Outcome::error(code),
            })
        }
        // `/partyinvite` (`FUN_393c2a90`).
        26 => {
            let (name, from_target) = selected(args, target);
            if from_target && !target_is_player {
                return Some(Outcome::error(EXPEDITION_NO_TARGET));
            }
            match party_invite(data, name, true) {
                Ok(command) => Some(Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                }),
                Err(code) => Some(Outcome::error(code)),
            }
        }
        // `/partyleave` (`FUN_393c1de0`).
        27 => Some(
            match party_leave(data, true) {
                Ok(command) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Err(code) => Outcome::error(code),
            },
        ),
        // `/partykick` (`FUN_393c2ab0`).
        28 => {
            let (name, _) = selected(args, target);
            Some(match party_kick(data, name, true) {
                Ok(Some(command)) => Outcome {
                    notice: None,
                    commands: vec![Command::Party(command)],
                },
                Ok(None) => Outcome::default(),
                Err(code) => Outcome::error(code),
            })
        }
        // `/expedition_kick` (`FUN_392f91b0`).
        41 => Some(
            super::expedition::slash_request(&mut data.expedition, 2, args.as_bytes())
                .map_or_else(Outcome::error, |()| Outcome::default()),
        ),
        // `/친구추가` (`FUN_39302370`).
        42 => Some(match friend_add(data, args) {
            Ok(command) => Outcome {
                notice: None,
                commands: vec![Command::Friend(command)],
            },
            Err(notice) => Outcome {
                notice: Some(notice),
                commands: Vec::new(),
            },
        }),
        // `/친구삭제` (`FUN_39300e60`).
        43 => Some(Outcome {
            notice: None,
            commands: vec![Command::Friend(FriendCommand::Remove {
                name: args.to_string(),
            })],
        }),
        // Matched functions without a social destination (languages, jury, watch/target, music,
        // the nation commands) are still consumed by the native input.
        _ => Some(Outcome::default()),
    }
}

fn family_action(data: &mut HudData, kind: u8, args: &str, syntax: &'static str) -> Outcome {
    let words: Vec<&str> = args.split_whitespace().collect();
    let (Some(name), Some(title)) = (words.first(), words.get(1)) else {
        return Outcome::community(syntax, None);
    };
    super::family::slash_request(&mut data.family, kind, name.as_bytes(), title.as_bytes())
        .map_or_else(Outcome::error, |()| Outcome::default())
}

fn join_channel(args: &str, channels: &[ChatChannel], create: bool) -> Outcome {
    let mut words = args.split_whitespace();
    let name = channel_name(words.next().unwrap_or(""));
    let password = words.next().unwrap_or("");
    if channels.iter().any(|c| c.name == name) {
        return Outcome::error(CHAT_ALREADY_JOINED);
    }
    Outcome {
        notice: None,
        commands: vec![Command::JoinUserChatChannel {
            name,
            password: password.to_string(),
            create,
        }],
    }
}

fn leave_channel(args: &str, channels: &[ChatChannel]) -> Outcome {
    let name = args.split_whitespace().next().unwrap_or("");
    let Some(channel) = channels
        .iter()
        .find(|c| c.chat_type == chat_type::USER && c.name == name)
    else {
        return Outcome::default();
    };
    Outcome {
        notice: None,
        commands: vec![Command::LeaveChatChannel {
            chat_id: channel_identity(channel),
        }],
    }
}

fn localize(
    lua: &Lua,
    group: &str,
    key: &str,
    arg: Option<&str>,
) -> mlua::Result<Value> {
    let locale: Table = lua.globals().get("X2Locale")?;
    let text: Function = locale.get("LocalizeUiText")?;
    let group: i64 = lua.globals().get(group)?;
    match arg {
        Some(arg) => text.call::<Value>((locale, group, key, arg)),
        None => text.call::<Value>((locale, group, key)),
    }
}

fn report(lua: &Lua, notice: Notice) {
    if notice == Notice::OpenBlockList {
        if let Err(error) = lua
            .load("ToggleRelationshipUI(true);relationshipFrame.tab:SelectTab(2)")
            .exec()
        {
            bevy::log::warn!("hud: open block list: {error}");
        }
        return;
    }
    let value = match notice {
        Notice::Error(code) => match super::alerts::error_name(code) {
            Some(key) => localize(lua, "ERROR_MSG", key, None),
            None => return,
        },
        Notice::Community { key, arg } => localize(lua, "COMMUNITY_TEXT", key, arg.as_deref()),
        Notice::OpenBlockList => unreachable!(),
    };
    match value {
        Ok(message) => aa_ui::lua::fire_ui_event(lua, "SYSMSG", vec![message]),
        Err(error) => bevy::log::warn!("hud: slash command notice: {error}"),
    }
}

/// Show the refusal and send the command(s) the line asked for.
pub(super) fn apply(outcome: Outcome, lua: Option<&Lua>, net: &mut MessageWriter<aa_net::NetCommand>) {
    if let (Some(notice), Some(lua)) = (outcome.notice, lua) {
        report(lua, notice);
    }
    for command in outcome.commands {
        net.write(aa_net::NetCommand(command));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::{friend::FriendInfo, state::ChatChannel};

    fn tables(rows: &[(i32, &str)]) -> Tables {
        let mut functions = HashMap::new();
        for (id, _) in rows {
            functions.insert(*id, String::new());
        }
        Tables {
            rows: rows
                .iter()
                .map(|(id, aliases)| Row {
                    action_id: *id,
                    action_type: "SlashFunction".into(),
                    aliases: aliases.split(' ').map(str::to_string).collect(),
                })
                .collect(),
            functions,
        }
    }

    fn data(rows: &[(i32, &str)]) -> HudData {
        let mut data = HudData {
            character_id: 47,
            ..Default::default()
        };
        data.snapshot.name = "Self".into();
        let game = super::super::data::GameData {
            slash: tables(rows),
            ..Default::default()
        };
        let _ = data.game.0.set(game);
        data
    }

    #[test]
    fn aliases_match_case_insensitively_and_only_as_whole_words() {
        let tables = tables(&[(1, "invite 초대"), (7, "굴림 dice 주사위 roll")]);
        assert_eq!(
            resolve(&tables, "/INVITE Bob"),
            Some(Resolved { action: 1, args: "Bob" })
        );
        assert_eq!(
            resolve(&tables, "/초대  Bob"),
            Some(Resolved { action: 1, args: "Bob" })
        );
        assert_eq!(resolve(&tables, "/invite"), Some(Resolved { action: 1, args: "" }));
        assert_eq!(resolve(&tables, "/invitex Bob"), None);
        assert_eq!(resolve(&tables, "/invite\tBob"), None);
        assert_eq!(resolve(&tables, "/roll 20"), Some(Resolved { action: 7, args: "20" }));
        assert_eq!(resolve(&tables, "/"), None);
    }

    #[test]
    fn party_commands_use_the_target_and_the_original_refusals() {
        let mut d = data(&[(1, "invite"), (2, "leave"), (3, "kick")]);
        let no_target = run(&mut d, "/invite", None, &[]).unwrap();
        assert_eq!(no_target.notice, Some(Notice::Error(0x1b3)));
        let target = Target { name: "Friend", is_player: true };
        let invite = run(&mut d, "/invite", Some(target), &[]).unwrap();
        assert!(matches!(
            &invite.commands[..],
            [Command::Party(PartyCommand::Invite { team_id: 0, is_party: true, target_name })] if target_name == "Friend"
        ));
        let own = run(&mut d, "/invite Self", None, &[]).unwrap();
        assert_eq!(own.notice, Some(Notice::Error(TEAM_YOURSELF)));
        let leave = run(&mut d, "/leave", None, &[]).unwrap();
        assert_eq!(leave.notice, Some(Notice::Error(CHAT_NOT_IN_PARTY)));
        d.party.id = 9;
        d.party.is_party = true;
        d.party.owner_id = 48;
        d.party.members = vec![aa_net::party::PartyMember {
            id: 47,
            name: "Self".into(),
            ..Default::default()
        }];
        let invite = run(&mut d, "/invite Friend", None, &[]).unwrap();
        assert_eq!(invite.notice, Some(Notice::Error(TEAM_NO_RIGHTS)));
        let kick = run(&mut d, "/kick Self", None, &[]).unwrap();
        assert!(kick.commands.is_empty() && kick.notice.is_none());
        d.party.owner_id = 47;
        d.party.members.push(aa_net::party::PartyMember {
            id: 48,
            name: "Friend".into(),
            ..Default::default()
        });
        let kick = run(&mut d, "/kick Friend", None, &[]).unwrap();
        assert!(matches!(
            &kick.commands[..],
            [Command::Party(PartyCommand::Kick { team_id: 9, member_id: 48 })]
        ));
    }

    #[test]
    fn friend_commands_match_the_native_checks() {
        let mut d = data(&[(42, "friendadd"), (43, "frienddel"), (15, "block"), (16, "unblock")]);
        assert_eq!(
            run(&mut d, "/friendadd Self", None, &[]).unwrap().notice,
            Some(Notice::Error(CANNOT_ADD_FRIEND_SELF))
        );
        d.friends.friends.push(FriendInfo {
            name: "Friend".into(),
            ..Default::default()
        });
        assert_eq!(
            run(&mut d, "/friendadd friend", None, &[]).unwrap().notice,
            Some(Notice::Error(CANNOT_ADD_EXISTING_MEMBER))
        );
        assert!(matches!(
            &run(&mut d, "/frienddel Friend", None, &[]).unwrap().commands[..],
            [Command::Friend(FriendCommand::Remove { name })] if name == "Friend"
        ));
        assert_eq!(
            run(&mut d, "/block Self", None, &[]).unwrap().notice,
            Some(Notice::Community { key: "cannot_block_self", arg: None })
        );
        assert_eq!(
            run(&mut d, "/unblock Ghost", None, &[]).unwrap().notice,
            Some(Notice::Community {
                key: "user_not_in_list",
                arg: Some("Ghost".into())
            })
        );
        assert!(matches!(
            &run(&mut d, "/block Friend", None, &[]).unwrap().commands[..],
            [Command::Friend(FriendCommand::Block { name })] if name == "Friend"
        ));
    }

    #[test]
    fn dice_defaults_and_bounds_follow_the_native_parser() {
        assert_eq!(dice(""), Some(100));
        assert_eq!(dice("20"), Some(20));
        assert_eq!(dice("1"), None);
        assert_eq!(dice("1000001"), None);
        assert_eq!(dice("20x"), Some(100));
        let mut d = data(&[(7, "dice")]);
        assert!(matches!(
            &run(&mut d, "/dice 5", None, &[]).unwrap().commands[..],
            [Command::RollDice { max: 5 }]
        ));
    }

    #[test]
    fn family_syntax_matches_the_native_two_token_rules() {
        let mut d = data(&[(18, "family_invite"), (22, "family_title")]);
        assert_eq!(
            run(&mut d, "/family_invite Bob", None, &[]).unwrap().notice,
            Some(Notice::Community { key: "invite_syntax", arg: None })
        );
        assert_eq!(
            run(&mut d, "/family_title Bob", None, &[]).unwrap().notice,
            Some(Notice::Community { key: "title_syntax", arg: None })
        );
    }

    #[test]
    fn user_channel_commands_send_the_native_packets() {
        let mut d = data(&[(4, "create_channel"), (5, "enter_channel"), (6, "leave_channel")]);
        let join = run(&mut d, "/create_channel Trade secret", None, &[]).unwrap();
        assert!(matches!(
            &join.commands[..],
            [Command::JoinUserChatChannel { name, password, create: true }]
                if name == "Trade" && password == "secret"
        ));
        let long = "x".repeat(60);
        let truncated = run(&mut d, &format!("/enter_channel {long}"), None, &[]).unwrap();
        assert!(matches!(
            &truncated.commands[..],
            [Command::JoinUserChatChannel { name, create: false, .. }] if name.len() == 48
        ));
        let channels = [ChatChannel {
            chat_type: chat_type::USER,
            sub_type: 0,
            faction_id: 9,
            name: "Trade".into(),
        }];
        assert_eq!(
            run(&mut d, "/enter_channel Trade", None, &channels).unwrap().notice,
            Some(Notice::Error(CHAT_ALREADY_JOINED))
        );
        let leave = run(&mut d, "/leave_channel Trade", None, &channels).unwrap();
        assert!(matches!(
            &leave.commands[..],
            [Command::LeaveChatChannel { chat_id }] if *chat_id == 0x0000_0009_0000_000f
        ));
        let absent = run(&mut d, "/leave_channel Missing", None, &channels).unwrap();
        assert!(absent.commands.is_empty() && absent.notice.is_none());
    }

    #[test]
    fn live_tables_load_the_original_social_aliases() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        let conn = rusqlite::Connection::open(path)?;
        let tables = Tables::load(&conn)?;
        assert_eq!(tables.functions.len(), 47);
        for (line, action) in [
            ("/invite", 1),
            ("/leave", 2),
            ("/kick", 3),
            ("/create_channel", 4),
            ("/enter_channel", 5),
            ("/leave_channel", 6),
            ("/dice", 7),
            ("/expeditioninvite", 12),
            ("/expeditionleave", 13),
            ("/block", 15),
            ("/unblock", 16),
            ("/family_invite", 18),
            ("/family_leave", 19),
            ("/family_kick", 21),
            ("/family_title", 22),
            ("/raidinvite", 23),
            ("/raidleave", 24),
            ("/raidkick", 25),
            ("/partyinvite", 26),
            ("/partyleave", 27),
            ("/partykick", 28),
            ("/expedition_kick", 41),
            ("/친구추가", 42),
            ("/친구삭제", 43),
        ] {
            assert_eq!(resolve(&tables, line).map(|r| r.action), Some(action), "{line}");
        }
        Ok(())
    }

    #[test]
    fn unrelated_lines_are_not_consumed() {
        let mut d = data(&[(1, "invite")]);
        assert!(run(&mut d, "/say hello", None, &[]).is_none());
    }
}
