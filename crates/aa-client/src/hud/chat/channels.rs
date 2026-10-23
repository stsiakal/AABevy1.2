//! The three original user-channel bindings of `X2Chat` (`FUN_3945aa00`):
//! `CreateUserChatChannel(channel, password)`, `JoinUserChatChannel(channel, password)` and
//! `LeaveUserChatChannel(channel)`. They queue a request; the app sends the native packet.

use mlua::{Lua, MultiValue, Table, Value};

use super::super::api::{Request, Shared};

/// `FUN_393c63e0`: the channel name is capped at 0x31 bytes; the password at 7 bytes
/// (`FUN_397ab6c0`).
const NAME_BYTES: usize = 0x31;
const PASSWORD_BYTES: usize = 7;

fn text(args: &MultiValue, i: usize) -> String {
    match args.iter().nth(i) {
        Some(Value::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

/// Cut at a UTF-8 boundary, as the native `XlCopyString` size does.
fn truncate_bytes(text: &str, max: usize) -> String {
    let mut end = 0;
    for (i, c) in text.char_indices() {
        let next = i + c.len_utf8();
        if next > max {
            break;
        }
        end = next;
    }
    text[..end].to_string()
}

fn channel_args(args: &MultiValue) -> Option<(String, String)> {
    // Called as `X2Chat:...`: the first value is the table itself.
    let name = truncate_bytes(&text(args, 1), NAME_BYTES);
    if name.is_empty() {
        return None;
    }
    Some((name, truncate_bytes(&text(args, 2), PASSWORD_BYTES)))
}

pub(in crate::hud) fn install(lua: &Lua, state: &Shared) -> mlua::Result<()> {
    let api: Table = lua.globals().get("X2Chat")?;
    let s = state.clone();
    api.raw_set(
        "CreateUserChatChannel",
        lua.create_function(move |_, args: MultiValue| {
            if let Some((name, password)) = channel_args(&args) {
                s.borrow_mut().requests.push(Request::ChatChannel { name, password, create: true });
            }
            Ok(())
        })?,
    )?;
    let s = state.clone();
    api.raw_set(
        "JoinUserChatChannel",
        lua.create_function(move |_, args: MultiValue| {
            if let Some((name, password)) = channel_args(&args) {
                s.borrow_mut().requests.push(Request::ChatChannel { name, password, create: false });
            }
            Ok(())
        })?,
    )?;
    let s = state.clone();
    api.raw_set(
        "LeaveUserChatChannel",
        lua.create_function(move |_, args: MultiValue| {
            let name = truncate_bytes(&text(&args, 1), NAME_BYTES);
            if !name.is_empty() {
                s.borrow_mut().requests.push(Request::LeaveChatChannel { name });
            }
            Ok(())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use aa_ui::UiRuntime;

    use super::*;
    use crate::hud::api::HudData;

    #[test]
    fn the_three_bindings_queue_the_native_requests() -> Result<(), Box<dyn std::error::Error>> {
        let rt = UiRuntime::new("en_us", 1280.0, 720.0)?;
        let state = Rc::new(RefCell::new(HudData::default()));
        install(&rt.lua, &state)?;
        rt.lua
            .load(
                "X2Chat:CreateUserChatChannel('Crew','secret');\
                 X2Chat:JoinUserChatChannel('Guild','x');\
                 X2Chat:LeaveUserChatChannel('Crew');\
                 X2Chat:CreateUserChatChannel('','ignored')",
            )
            .exec()?;
        let requests = &state.borrow().requests;
        assert_eq!(
            &requests[..],
            [
                Request::ChatChannel { name: "Crew".into(), password: "secret".into(), create: true },
                Request::ChatChannel { name: "Guild".into(), password: "x".into(), create: false },
                Request::LeaveChatChannel { name: "Crew".into() },
            ]
        );
        Ok(())
    }

    #[test]
    fn the_name_and_password_keep_the_native_byte_caps() -> Result<(), Box<dyn std::error::Error>> {
        let rt = UiRuntime::new("en_us", 1280.0, 720.0)?;
        let state = Rc::new(RefCell::new(HudData::default()));
        install(&rt.lua, &state)?;
        rt.lua
            .load("X2Chat:JoinUserChatChannel(string.rep('a', 60), '12345678')")
            .exec()?;
        let requests = &state.borrow().requests;
        assert!(matches!(&requests[..], [Request::ChatChannel { name, password, create: false }]
            if name.len() == 49 && password == "1234567"));
        Ok(())
    }
}
