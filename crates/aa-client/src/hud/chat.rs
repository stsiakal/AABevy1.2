//! Chat: incoming lines and channel changes from aa-net become the UI events the chat scripts
//! listen for (`CHAT_MESSAGE`, `CHAT_JOINED_CHANNEL`, `CHAT_LEAVED_CHANNEL`), and `Enter` opens the
//! chat window's input (the scripts' own edit box, `chatwindow:GetChatEdit()`), whose text goes out
//! as `Command::Chat` (see `HUD.chat_send` in `ingame.lua`).
//!
//! The channel numbers aa-net reports are the client's own (`CHAT_SAY` 0, `CHAT_ZONE` 1 (shout),
//! `CHAT_TRADE` 2, `CHAT_FIND_PARTY` 3, `CHAT_PARTY` 4, `CHAT_RAID` 5, `CHAT_FACTION` 6,
//! `CHAT_EXPEDITION` 7, `CHAT_RACE` 14, whispers -3 / -4, notice -1, system -2), so they pass
//! through unchanged.

pub(super) mod channels;
pub(super) mod config;

use aa_net::state::{Chat, Player};
use aa_net::{Event, NetEvent};
use aa_ui::{UiKey, UiRuntime};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::api::HudBridge;
use super::data::{BlockedText, GameData};
use super::{HudBuilt, HudKeyboardFocus};

/// The chat edit's per-type limit: `chatLocale.maxTextLength` (`x2ui/chat/locale/ko.alb`, 160
/// for every type in the shipped locales).
const TEXT_LIMITS: [usize; 15] = [160; 15];

pub(super) fn text_limit(chat_type: i16) -> usize {
    usize::try_from(chat_type)
        .ok()
        .filter(|t| *t < TEXT_LIMITS.len())
        .map_or(160, |t| TEXT_LIMITS[t])
}

/// The C++ send buffer (`FUN_393c5aa0`) holds 1024 UTF-16 units.
pub(super) const TRANSPORT_LIMIT: usize = 1024;

/// Truncate at `limit` characters (the edit's own count).
pub(super) fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Truncate at `limit` UTF-16 units, as `FUN_393c5aa0` does after the conversion.
pub(super) fn truncate_utf16(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= limit
        })
        .collect()
}

/// `FUN_393c78e0`: replace every `check_chat` row's text with `*` (the same character count).
/// `partial` rows match anywhere, the others only as whole words. `None` when nothing matched.
pub(super) fn censor(text: &str, rows: &[BlockedText]) -> Option<String> {
    let mut chars: Vec<char> = text.chars().collect();
    let mut lower: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let mut changed = false;
    for row in rows.iter().filter(|r| r.check_chat && !r.text.is_empty()) {
        let needle: Vec<char> = row.text.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
        let n = needle.len();
        let mut i = 0;
        while i + n <= chars.len() {
            let hit = (0..n).all(|k| lower[i + k] == needle[k]);
            let whole = row.partial
                || ((i == 0 || !lower[i - 1].is_alphanumeric())
                    && (i + n == lower.len() || !lower[i + n].is_alphanumeric()));
            if hit && whole {
                for c in &mut chars[i..i + n] {
                    *c = '*';
                }
                for c in &mut lower[i..i + n] {
                    *c = '*';
                }
                changed = true;
                i += n;
            } else {
                i += 1;
            }
        }
    }
    changed.then(|| chars.into_iter().collect())
}

/// `FUN_39426f40`: fill a localized template's `%s` / `%u` / `%d` with the packet's two strings.
pub(super) fn format_params(template: &str, param: &str, param2: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut params = [param, param2].into_iter();
    let mut rest = template;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        if let Some(kind) = after.chars().nth(1) {
            if matches!(kind, 's' | 'u' | 'd') {
                if let Some(value) = params.next() {
                    out.push_str(value);
                }
                rest = &after[2..];
                continue;
            }
            if kind == '%' {
                out.push('%');
                rest = &after[2..];
                continue;
            }
        }
        out.push('%');
        rest = &after[1..];
    }
    out.push_str(rest);
    out
}

/// `CHAT_FAILED(localized, raw)` with the `ERROR_MSG` row of `code` (0x108-0x111 are the chat
/// refusals: yell too often, no whisper target, forbidden word, not in party/raid, channel errors).
pub(super) fn fire_failed(rt: &UiRuntime, code: i16, raw: &str) {
    let text = super::alerts::error_name(code)
        .and_then(|name| super::alerts::localized(rt, "ERROR_MSG", name))
        .unwrap_or_else(|| raw.to_string());
    rt.fire_event("CHAT_FAILED", (text, raw.to_string()));
}

/// The name the scripts show for a joined channel: the zone for the zone channels, the faction for
/// the faction ones, the user's name for user channels.
fn channel_name(game: Option<&GameData>, chat_type: i16, sub_type: i16, faction_id: u32, name: &str) -> String {
    if !name.is_empty() {
        return name.to_string();
    }
    let game = match game {
        Some(g) => g,
        None => return String::new(),
    };
    match chat_type {
        // shout / trade / party search: the zone group
        1..=3 => game.zone_groups.get(&(sub_type as u32)).map(|g| g.text.clone()).unwrap_or_default(),
        // faction (nation), alliance
        6 | 14 => game.factions.get(&faction_id).cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

fn join(rt: &UiRuntime, game: Option<&GameData>, chat_type: i16, sub_type: i16, faction_id: u32, name: &str) {
    rt.fire_event("CHAT_JOINED_CHANNEL", (chat_type as i64, channel_name(game, chat_type, sub_type, faction_id, name)));
}

/// `bound`: the speaker is a unit the client has (the player or one in range): the script draws the speech bubble of say, party,
/// raid and guild lines over it (`speakerInChatBound`, see `speech.rs`). Replayed lines are old news: no bubble.
fn message(rt: &UiRuntime, m: &aa_net::model::ChatMessage, friends: &aa_net::Friends, bound: bool) {
    if (m.sender_obj_id != 0 || m.sender_id != 0) && friends.is_blocked(&m.sender) { return; }
    // The original event passes 11 values; the scripts use type, numeric unit id, character id,
    // name, text and `speakerInChatBound`; the rest keep the channel and ability/language fields.
    rt.fire_event(
        "CHAT_MESSAGE",
        (
            m.chat_type as i64,
            i64::from(m.sender_obj_id),
            i64::from(m.sender_id),
            m.sender.clone(),
            m.message.clone(),
            bound,
            None::<String>,
            i64::from(m.sub_type),
            i64::from(m.faction_id),
            i64::from(m.ability),
            i64::from(m.language),
            i64::from(m.sender_race),
        ),
    );
}

/// `SCChatLocalizedMessage`: look the key up in `X2Locale` (the packet's category is the group)
/// and fill the template with its two string parameters.
fn localized(rt: &UiRuntime, m: &aa_net::model::ChatLocalizedMessage) {
    let locale = rt.lua.globals().get::<Option<mlua::Table>>("X2Locale").ok().flatten();
    let template = locale
        .as_ref()
        .and_then(|t| t.get::<Option<mlua::Function>>("LocalizeUiText").ok().flatten().map(|f| (t, f)))
        .and_then(|(t, f)| {
            f.call::<Option<String>>((t, i64::from(m.category), m.key.as_str()))
                .ok()
                .flatten()
        })
        .filter(|text| !text.is_empty() && *text != m.key)
        .unwrap_or_else(|| m.key.clone());
    let text = format_params(&template, &m.param, &m.param2);
    rt.fire_event(
        "CHAT_MESSAGE",
        (
            i64::from(m.chat_type()),
            i64::from(m.sender_obj_id),
            0i64,
            m.sender.clone(),
            text,
            false,
            None::<String>,
            i64::from(m.sub_type()),
            i64::from(m.channel_faction()),
            0i64,
            i64::from(m.language),
        ),
    );
}

/// The channels joined and the lines received before the HUD existed, in order.
pub fn replay(rt: &UiRuntime, chat: &Chat, game: Option<&GameData>, friends: &aa_net::Friends) {
    for c in &chat.channels {
        join(rt, game, c.chat_type, c.sub_type, c.faction_id, &c.name);
    }
    for m in &chat.log {
        message(rt, m, friends, false);
    }
}

#[cfg(test)]
mod blocked_tests {
    use super::*;

    fn line(name:&str,id:u32) -> aa_net::model::ChatMessage {
        aa_net::model::ChatMessage { chat_type:0,sub_type:0,channel_faction:0,sender_obj_id:id,
            sender_id:id,sender_race:1,faction_id:1,sender:name.into(),message:"hello".into(),ability:0,language:0 }
    }

    #[test]
    fn blocked_chat_is_filtered_for_live_and_replayed_lines_without_losing_raw_log() -> Result<(),Box<dyn std::error::Error>> {
        let rt=UiRuntime::new("en_us",1280.0,720.0)?;
        rt.lua.load("lines={}; UIParent:SetEventHandler('CHAT_MESSAGE',function(channel,id,flag,sender,text) lines[#lines+1]=sender end)").exec()?;
        let friends=aa_net::Friends {blocked:vec![aa_net::friend::BlockedUser {id:1,name:"Blocked".into()}],..default()};
        let mut chat=Chat::default();
        for m in [line("bLoCkEd",1),line("Visible",2),line("Blocked",0)] {
            chat.apply(&Event::Chat(m));
        }
        replay(&rt,&chat,None,&friends);
        assert_eq!(chat.log.len(),3);
        rt.lua.load("assert(#lines==2 and lines[1]=='Visible' and lines[2]=='Blocked'); lines={}").exec()?;
        let bridge=std::rc::Rc::new(std::cell::RefCell::new(super::super::api::HudData {friends,..default()}));
        let mut app=App::new(); app.add_message::<NetEvent>(); app.add_systems(Update,route_chat);
        app.insert_resource(HudBuilt {up:true,fresh:false}); app.insert_non_send(HudBridge(bridge)); app.insert_non_send(rt);
        app.world_mut().write_message(NetEvent(Event::Chat(line("blocked",1))));
        app.world_mut().write_message(NetEvent(Event::Chat(line("Visible",2))));
        app.update();
        app.world().non_send::<UiRuntime>().lua.load("assert(#lines==1 and lines[1]=='Visible')").exec()?;
        Ok(())
    }
}

/// Raises the chat events as aa-net reports them.
pub fn route_chat(
    mut events: MessageReader<NetEvent>,
    rt: Option<NonSend<UiRuntime>>,
    mut built: ResMut<HudBuilt>,
    bridge: NonSend<HudBridge>,
    units: Option<Res<aa_net::state::Units>>,
    player: Option<Res<Player>>,
) {
    // lines that arrived before (or while) the stage was built were replayed from the resource
    if !built.up || std::mem::take(&mut built.fresh) {
        events.clear();
        return;
    }
    let Some(rt) = rt else { return };
    let data = bridge.0.borrow();
    let game = data.game.get();
    for NetEvent(event) in events.read() {
        match event {
            Event::Chat(m) => {
                let own = player.as_ref().and_then(|p| p.spawn.as_ref()).is_some_and(|s| s.obj_id == m.sender_obj_id);
                let bound = m.sender_obj_id != 0 && (own || units.as_ref().is_some_and(|u| u.get(m.sender_obj_id).is_some()));
                message(&rt, m, &data.friends, bound)
            }
            Event::ChatChannelJoined { chat_type, sub_type, faction_id, name } => join(&rt, game, *chat_type, *sub_type, *faction_id, name),
            Event::ChatChannelLeft { chat_type, sub_type, faction_id } => {
                rt.fire_event("CHAT_LEAVED_CHANNEL", (*chat_type as i64, channel_name(game, *chat_type, *sub_type, *faction_id, "")));
            }
            Event::ChatNotice(n) => {
                rt.fire_event(
                    "NOTICE_MESSAGE",
                    (i64::from(n.kind), n.alpha_hex.clone(), i64::from(n.visible_time()), n.text.clone()),
                );
            }
            Event::ChatFailed { code, text } => fire_failed(&rt, *code as i16, text),
            Event::ChatLocalized(m) => localized(&rt, m),
            _ => {}
        }
    }
}

/// `--hud-key NAME T` / `--hud-type TEXT T`: scripted keyboard for unattended runs (`enter`,
/// `escape`, `backspace`, `tab`, `slot3`, `f` / `g` (loot) ...; typed text), at T seconds after the HUD is up.
#[derive(Resource, Default)]
pub struct ScriptedKeys {
    steps: Vec<(f32, String, bool)>,
    started: Option<f32>,
    /// A scripted `tab` is waiting for `targeting::tab_target`.
    tab: bool,
    escape: bool,
    /// A scripted `slotN` (action bar slot N, 1 based) waits for `panels::action_keys`.
    slot: Option<usize>,
    /// A scripted `f` (loot all, `Some(true)`) or `g` (open the loot) waits for `loot::loot_input`.
    loot: Option<bool>,
}

impl ScriptedKeys {
    /// A key of `--hud-key` (or typed text) now, for the script runner (`script`).
    pub fn push_now(&mut self, v: &str, is_text: bool) {
        self.steps.push((f32::MIN, v.to_string(), is_text));
    }

    pub fn take_tab(&mut self) -> bool {
        std::mem::take(&mut self.tab)
    }

    pub fn take_escape(&mut self) -> bool {
        std::mem::take(&mut self.escape)
    }

    pub fn take_slot(&mut self) -> Option<usize> {
        self.slot.take()
    }

    pub fn take_loot(&mut self) -> Option<bool> {
        self.loot.take()
    }

    pub fn from_args() -> ScriptedKeys {
        let args: Vec<String> = std::env::args().collect();
        let mut steps = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if (a == "--hud-key" || a == "--hud-type")
                && let (Some(v), Some(t)) = (args.get(i + 1), args.get(i + 2).and_then(|t| t.parse::<f32>().ok()))
            {
                steps.push((t, v.clone(), a == "--hud-type"));
            }
        }
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        steps.reverse(); // pop from the end
        ScriptedKeys { steps, ..ScriptedKeys::default() }
    }
}

fn ui_key(name: &str) -> Option<UiKey> {
    Some(match name {
        "enter" => UiKey::Enter,
        "escape" => UiKey::Escape,
        "backspace" => UiKey::Backspace,
        "delete" => UiKey::Delete,
        "tab" => UiKey::Tab,
        "left" => UiKey::Left,
        "right" => UiKey::Right,
        "home" => UiKey::Home,
        "end" => UiKey::End,
        _ => return None,
    })
}

/// `Enter` with no edit box focused opens the chat input (and a scripted `--hud-key enter` does the
/// same). While an edit box has the focus the toolkit already routes the keys to it. The input opens
/// one frame after the key: the toolkit forwards the same key press to the focused box in whatever
/// order its systems run, and the very press that opens the input must not also send it.
pub fn chat_keys(
    actions: Res<crate::settings::Actions>,
    mut typed: MessageReader<KeyboardInput>,
    focus: Res<HudKeyboardFocus>,
    rt: Option<NonSend<UiRuntime>>,
    time: Res<Time>,
    mut script: ResMut<ScriptedKeys>,
    mut pending: Local<Option<bool>>,
) {
    let Some(rt) = rt else {
        typed.clear();
        return;
    };
    // opening requested by the last frame's key
    if let Some(slash) = pending.take()
        && !focus.0
        && !rt.keyboard_captured()
        && let Err(e) = rt.exec("@chat_open", &format!("HUD.chat_open({})", if slash { "\"/\"" } else { "nil" }))
    {
        warn!("hud: chat input: {e}");
    }
    let mut open = actions.just_pressed("open_chat", 0);
    // "/" opens the input with a slash already typed, like the real client
    let slash = typed.read().any(|e| e.state.is_pressed() && matches!(&e.logical_key, Key::Character(c) if c.as_str() == "/"));
    // scripted keys, relative to the first frame they were looked at
    let now = time.elapsed_secs();
    let started = *script.started.get_or_insert(now);
    while script.steps.last().is_some_and(|s| now - started >= s.0) {
        let (_, v, is_text) = script.steps.pop().unwrap();
        if is_text {
            rt.type_text(&v);
        } else if let Some(n) = v.strip_prefix("slot").and_then(|n| n.parse::<usize>().ok()) {
            script.slot = Some(n.saturating_sub(1));
        } else if v == "f" || v == "g" {
            script.loot = Some(v == "f");
        } else if let Some(k) = ui_key(&v) {
            if focus.0 || rt.keyboard_captured() {
                rt.key(&k);
            } else if k == UiKey::Enter {
                open = true;
            } else if k == UiKey::Tab {
                script.tab = true;
            } else if k == UiKey::Escape {
                script.escape = true;
            }
        }
    }
    if focus.0 || rt.keyboard_captured() {
        return;
    }
    if open || slash {
        *pending = Some(slash && !open);
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    fn blocked(text: &str, check_chat: bool, partial: bool) -> BlockedText {
        BlockedText { text: text.into(), check_name: true, check_chat, partial }
    }

    #[test]
    fn the_type_limit_is_160_and_the_transport_cap_is_1024_utf16() {
        assert_eq!(text_limit(0), 160);
        assert_eq!(text_limit(-3), 160);
        assert_eq!(truncate_chars(&"a".repeat(200), text_limit(0)).chars().count(), 160);
        // Two UTF-16 units each: 513 emoji are 1026 units, so the last one is dropped.
        let emoji = "😀".repeat(513);
        assert_eq!(truncate_utf16(&emoji, TRANSPORT_LIMIT), "😀".repeat(512));
        assert_eq!(truncate_utf16("abc", TRANSPORT_LIMIT), "abc");
    }

    #[test]
    fn the_censor_keeps_the_character_count_and_honours_the_row_flags() {
        let rows = [
            blocked("fuck", true, false),
            blocked("nig", true, true),
            blocked("Admin", false, true),
        ];
        assert_eq!(censor("Fuck this", &rows).as_deref(), Some("**** this"));
        assert_eq!(censor("a fuckery", &rows), None, "whole word only");
        assert_eq!(censor("Niglet", &rows).as_deref(), Some("***let"));
        assert_eq!(censor("Admin", &rows), None, "check_chat is off");
        assert_eq!(censor("clean", &rows), None);
    }

    #[test]
    fn the_localized_template_fills_its_two_parameters() {
        assert_eq!(format_params("%s invited %s", "Ann", "Bob"), "Ann invited Bob");
        assert_eq!(format_params("100% fine", "Ann", "Bob"), "100% fine");
        assert_eq!(format_params("%d%%", "7", "Bob"), "7%");
    }

    #[test]
    fn the_chat_refusal_codes_map_to_the_locale_rows() {
        for (code, name) in [
            (0x108, "YELL_TOO_OFTEN"),
            (0x109, "WHISPER_NO_TARGET"),
            (0x10a, "SAID_FORBIDDEN_WORD"),
            (0x10b, "CHAT_NOT_IN_PARTY"),
            (0x10c, "CHAT_NOT_IN_RAID"),
            (0x10d, "CHAT_CHANNEL_ALREADY_EXISTS"),
            (0x10e, "CHAT_NO_CHANNEL"),
            (0x10f, "CHAT_ALREADY_JOINED_CHANNEL"),
            (0x110, "CHAT_PRIVATE_CHANNEL"),
            (0x111, "CHAT_WRONG_PASSWORD"),
        ] {
            assert_eq!(super::super::alerts::error_name(code), Some(name), "{code:#x}");
        }
    }

    #[test]
    fn chat_events_fire_with_the_native_fields() -> Result<(), Box<dyn std::error::Error>> {
        let rt = UiRuntime::new("en_us", 1280.0, 720.0)?;
        rt.lua
            .load(
                "X2Locale={LocalizeUiText=function(_,group,key) \
                   if key=='CHAT_ALREADY_JOINED_CHANNEL' then return 'Already joined' end \
                   if key=='party_invite' then return '%s joined %s' end \
                 end};\
                 lines={}; notice=nil; failed=nil;\
                 UIParent:SetEventHandler('CHAT_MESSAGE',function(a,b,c,d,e,f,g,h,i,j,k) lines[#lines+1]={a,b,c,d,e,f,g,h,i,j,k} end);\
                 UIParent:SetEventHandler('NOTICE_MESSAGE',function(a,b,c,d) notice={a,b,c,d} end);\
                 UIParent:SetEventHandler('CHAT_FAILED',function(a,b) failed={a,b} end)",
            )
            .exec()?;
        let bridge = std::rc::Rc::new(std::cell::RefCell::new(super::super::api::HudData::default()));
        let mut app = App::new();
        app.add_message::<NetEvent>();
        app.add_systems(Update, route_chat);
        app.insert_resource(HudBuilt { up: true, fresh: false });
        app.insert_non_send(HudBridge(bridge));
        app.insert_non_send(rt);
        let m = aa_net::model::ChatMessage {
            chat_type: 0,
            sub_type: 5,
            channel_faction: 0,
            sender_obj_id: 123,
            sender_id: 9,
            sender_race: 1,
            faction_id: 7,
            sender: "Ann".into(),
            message: "hi".into(),
            ability: 3,
            language: 1,
        };
        app.world_mut().write_message(NetEvent(Event::Chat(m)));
        app.world_mut().write_message(NetEvent(Event::ChatNotice(aa_net::model::ChatNotice {
            kind: 2,
            alpha_hex: "FF".into(),
            visible_ms: 0,
            text: "FF0000Maintenance".into(),
        })));
        app.world_mut().write_message(NetEvent(Event::ChatFailed { code: 0x10f, text: "Crew".into() }));
        app.world_mut().write_message(NetEvent(Event::ChatLocalized(aa_net::model::ChatLocalizedMessage {
            chat_id: 0x0000_0009_0000_000f,
            sender_obj_id: 124,
            language: 1,
            category: 7,
            key: "party_invite".into(),
            sender: "Bob".into(),
            num_param: 2,
            param: "Bob".into(),
            param2: "Squad".into(),
        })));
        app.update();
        let lua = &app.world().non_send::<UiRuntime>().lua;
        lua.load(
            "assert(lines[1][1]==0 and lines[1][2]==123 and lines[1][3]==9 and lines[1][4]=='Ann' and lines[1][5]=='hi');\
             assert(lines[1][8]==5 and lines[1][9]==7 and lines[1][10]==3 and lines[1][11]==1);\
             assert(lines[2][1]==15 and lines[2][2]==124 and lines[2][4]=='Bob' and lines[2][5]=='Bob joined Squad');\
             assert(lines[2][8]==0 and lines[2][9]==9 and lines[2][11]==1);\
             assert(notice[1]==2 and notice[2]=='FF' and notice[3]==1000+17*50 and notice[4]=='FF0000Maintenance');\
             assert(failed[1]=='Already joined' and failed[2]=='Crew')",
        )
        .exec()?;
        Ok(())
    }
}
