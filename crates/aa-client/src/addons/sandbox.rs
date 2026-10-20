//! One Lua environment per addon: safe standard functions, read-only views of the game API
//! tables, `ADDON_API`, `UIC` and a `require` that only reads plain Lua files inside the addon's
//! own folder.

use std::path::{Path, PathBuf};

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, MultiValue, Table, Value, Variadic};

use super::manifest::{self, Manifest};
use super::{AddonEntry, Host, State};
use super::settings;

const API_LUA: &str = include_str!("api.lua");
const MAX_MODULE_BYTES: u64 = 512 * 1024;
const MAX_TOTAL_MODULE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MODULES: u32 = 64;
const MAX_REQUIRE_DEPTH: u32 = 8;
const MAX_EVENT_NAME: usize = 64;
const MAX_LOG_BYTES: usize = 1024;
/// `CMF_SYSTEM`: the client's system-text channel. `chat_msg_event.lua` maps
/// `CHAT_SYSTEM`/`CHAT_SYSTEM_TEXT` and `CHAT_DAILY_MSG` to `CMF_SYSTEM` (12), and the default
/// "All" tab takes it (`hud/chat/config/native_defaults.json`, `filters.general`).
const SYSTEM_CHAT_FILTER: i64 = 12;

const SAFE_BASE: [&str; 15] = [
    "assert", "error", "ipairs", "next", "pairs", "pcall", "rawequal", "rawget", "rawset", "select",
    "tonumber", "tostring", "type", "unpack", "xpcall",
];

const BRIDGE_LUA: &str = r#"
if not ADDON_EVENTS_BRIDGED then
  ADDON_EVENTS_BRIDGED = true
  local function forward(name)
    return function(...) ADDON_HOST_EVENT(name, ...) end
  end
  local ok = pcall(function()
    UIParent:SetEventHandler("CHAT_MESSAGE", forward("CHAT_MESSAGE"))
    UIParent:SetEventHandler("TEAM_MEMBERS_CHANGED", forward("TEAM_MEMBERS_CHANGED"))
  end)
  if not ok then ADDON_EVENTS_BRIDGED = nil end
end
"#;

fn error(message: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(message.into())
}

fn addons_state(lua: &Lua) -> mlua::Result<Table> {
    let globals = lua.globals();
    let state = match globals.raw_get::<Option<Table>>("ADDON_STATE")? {
        Some(state) => state,
        None => {
            let state = lua.create_table()?;
            globals.raw_set("ADDON_STATE", &state)?;
            state
        }
    };
    match state.raw_get::<Option<Table>>("addons")? {
        Some(addons) => Ok(addons),
        None => {
            let addons = lua.create_table()?;
            state.raw_set("addons", &addons)?;
            Ok(addons)
        }
    }
}

fn state_entry(lua: &Lua, id: &str) -> mlua::Result<Option<Table>> {
    addons_state(lua)?.raw_get::<Option<Table>>(id)
}

fn remove_state(lua: &Lua, id: &str) -> mlua::Result<()> {
    addons_state(lua)?.raw_set(id, Value::Nil)
}

fn loaded_ids(host: &Host) -> Vec<String> {
    host.0
        .borrow()
        .addons
        .iter()
        .filter(|addon| addon.state == State::Loaded)
        .map(|addon| addon.id.clone())
        .collect()
}

pub(super) fn is_loaded(host: &Host, id: &str) -> bool {
    host.0
        .borrow()
        .addons
        .iter()
        .any(|addon| addon.id == id && addon.state == State::Loaded)
}

fn text_field(main: &Table, key: &str, max: usize, default: String) -> mlua::Result<String> {
    match main.raw_get::<Value>(key)? {
        Value::Nil => Ok(default),
        Value::String(text) => {
            let text = text.to_string_lossy();
            Ok(text.chars().take(max).collect())
        }
        _ => Err(error(format!("{key} must be a string"))),
    }
}

fn version_field(main: &Table) -> mlua::Result<String> {
    Ok(match main.raw_get::<Value>("version")? {
        Value::Nil => "0".into(),
        Value::String(text) => text.to_string_lossy().chars().take(64).collect(),
        Value::Integer(number) => number.to_string(),
        Value::Number(number) => {
            if number.fract() == 0.0 {
                format!("{number:.0}")
            } else {
                number.to_string()
            }
        }
        _ => return Err(error("version must be a string or a number")),
    })
}

fn manifest_of(main: &Table, id: &str) -> mlua::Result<Manifest> {
    Ok(Manifest {
        name: text_field(main, "name", 128, id.to_string())?,
        author: text_field(main, "author", 256, String::new())?,
        desc: text_field(main, "desc", 2048, String::new())?,
        version: version_field(main)?,
    })
}

fn proxy(lua: &Lua, original: Table, what: &str, id: &str) -> mlua::Result<Table> {
    let view = lua.create_table()?;
    let mt = lua.create_table()?;
    let source = original.clone();
    let target = view.clone();
    mt.raw_set(
        "__index",
        lua.create_function(move |lua, (_view, key): (Table, Value)| {
            let value = source.get::<Value>(key)?;
            let Value::Function(function) = value else {
                return Ok(value);
            };
            let source = source.clone();
            let target = target.clone();
            Ok(Value::Function(lua.create_function(move |_, args: MultiValue| {
                let mut args = args.into_iter();
                let first = args.next();
                let is_method = first
                    .as_ref()
                    .is_some_and(|value| matches!(value, Value::Table(table) if table.to_pointer() == target.to_pointer()));
                let mut call = MultiValue::new();
                call.push_back(Value::Table(source.clone()));
                if !is_method && let Some(value) = first {
                    call.push_back(value);
                }
                for value in args {
                    call.push_back(value);
                }
                function.call::<MultiValue>(call)
            })?))
        })?,
    )?;
    let (name, addon) = (what.to_string(), id.to_string());
    mt.raw_set(
        "__newindex",
        lua.create_function(move |_, (_view, _key, _value): (Table, Value, Value)| -> mlua::Result<()> {
            Err(error(format!("addon {addon}: {name} is read-only")))
        })?,
    )?;
    view.set_metatable(Some(mt))?;
    Ok(view)
}

fn library(lua: &Lua, name: &str) -> mlua::Result<Table> {
    let source: Table = lua.globals().raw_get(name)?;
    let entries: Vec<(Value, Value)> = source.pairs::<Value, Value>().collect::<mlua::Result<_>>()?;
    let copy = lua.create_table()?;
    for (key, value) in entries {
        copy.raw_set(key, value)?;
    }
    Ok(copy)
}

fn uic_table(lua: &Lua) -> mlua::Result<Table> {
    let globals = lua.globals();
    let constants: Vec<(String, Value)> = globals
        .pairs::<Value, Value>()
        .filter_map(|pair| {
            let (key, value) = pair.ok()?;
            let Value::String(key) = key else { return None };
            let key = key.to_str().ok()?;
            let name = key.strip_prefix("UIC_")?;
            (!name.is_empty()).then(|| (name.to_string(), value))
        })
        .collect();
    let uic = lua.create_table()?;
    for (name, value) in constants {
        uic.raw_set(format!("UIC_{name}"), value.clone())?;
        uic.raw_set(name, value)?;
    }
    Ok(uic)
}

fn create_root(lua: &Lua, id: &str) -> mlua::Result<Table> {
    let ui: Table = lua
        .globals()
        .raw_get::<Option<Table>>("UIParent")?
        .ok_or_else(|| error("UIParent is not installed"))?;
    let create: Function = ui.get("CreateWidget")?;
    let name: String = format!("addon_{}_root", id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let root = create.call::<Table>((ui, "emptywidget", name, "UIParent"))?;
    // Top-level widgets start hidden (aa-ui `Tree::create_widget`); the addon root is the default
    // parent of every addon widget, so it must be shown or the whole addon stays invisible.
    let show: Function = root.get("Show")?;
    show.call::<()>((root.clone(), true))?;
    Ok(root)
}

fn free_widget(widget: &Table) {
    if let Ok(hide) = widget.get::<Function>("EnableHidingIsRemove") {
        let _ = hide.call::<()>((widget.clone(), true));
    }
    if let Ok(show) = widget.get::<Function>("Show") {
        let _ = show.call::<()>((widget.clone(), false));
    }
}

pub(super) fn now_ms(lua: &Lua) -> f64 {
    if let Ok(Some(time)) = lua.globals().raw_get::<Option<Table>>("X2Time")
        && let Ok(Some(get)) = time.raw_get::<Option<Function>>("GetUiMsec")
        && let Ok(value) = get.call::<f64>(time)
    {
        return value;
    }
    lua.try_app_data_ref::<aa_ui::lua::Ui>()
        .ok()
        .flatten()
        .map_or(0.0, |ui| ui.time_ms)
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

fn dispatch_chat(lua: &Lua, text: &str) -> bool {
    let result = (|| -> mlua::Result<()> {
        let Some(chat) = lua.globals().raw_get::<Option<Table>>("X2Chat")? else {
            return Ok(());
        };
        let Some(dispatch) = chat.raw_get::<Option<Function>>("DispatchChatMessage")? else {
            return Ok(());
        };
        dispatch.call::<()>((chat, SYSTEM_CHAT_FILTER, text))?;
        Ok(())
    })();
    result.is_ok()
}

pub(super) fn say(lua: &Lua, text: &str, error: bool) {
    let text = truncate(text, MAX_LOG_BYTES);
    let line = if error {
        format!("|cFFFF6060{text}|r")
    } else {
        text.clone()
    };
    if !dispatch_chat(lua, &line) {
        if error {
            bevy::log::warn!("addons: {text}");
        } else {
            bevy::log::info!("addons: {text}");
        }
    }
}

fn addon_log(lua: &Lua, host: &Host, id: &str, text: &str, error: bool) {
    let text = truncate(text.trim(), MAX_LOG_BYTES);
    super::push_log(host, id, &text);
    if error {
        bevy::log::warn!("addon {id}: {text}");
    } else {
        bevy::log::debug!("addon {id}: {text}");
    }
    say(lua, &format!("[{id}] {text}"), error);
}

fn settings_table(lua: &Lua, host: &Host, id: &str) -> mlua::Result<Table> {
    let Some(entry) = state_entry(lua, id)? else {
        return Err(error(format!("addon {id}: not loaded")));
    };
    if let Some(settings) = entry.raw_get::<Option<Table>>("settings")? {
        return Ok(settings);
    }
    let path = host
        .0
        .borrow()
        .addons
        .iter()
        .find(|addon| addon.id == id)
        .map(|addon| addon.settings_path.clone())
        .ok_or_else(|| error(format!("addon {id}: not loaded")))?;
    let json = match settings::load(&path) {
        Ok(json) => json,
        Err(e) => {
            bevy::log::warn!("addon {id}: settings: {e}");
            serde_json::Value::Object(Default::default())
        }
    };
    let value = match settings::to_lua(lua, &json) {
        Ok(Value::Table(table)) => table,
        Ok(_) => lua.create_table()?,
        Err(e) => {
            bevy::log::warn!("addon {id}: settings: {e}");
            lua.create_table()?
        }
    };
    entry.raw_set("settings", &value)?;
    Ok(value)
}

fn save_settings(lua: &Lua, host: &Host, id: &str) {
    let Some(table) = state_entry(lua, id).ok().flatten().and_then(|entry| entry.raw_get::<Option<Table>>("settings").ok().flatten()) else {
        return;
    };
    let path = match host
        .0
        .borrow()
        .addons
        .iter()
        .find(|addon| addon.id == id)
        .map(|addon| addon.settings_path.clone())
    {
        Some(path) => path,
        None => return,
    };
    match settings::to_json(&Value::Table(table)).and_then(|json| settings::save(&path, &json)) {
        Ok(()) => super::push_log(host, id, &format!("settings saved to {}", path.display())),
        Err(e) => {
            bevy::log::warn!("addon {id}: settings: {e}");
            super::push_log(host, id, &format!("settings error: {e}"));
        }
    }
}

fn valid_event(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_EVENT_NAME
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn handler_list(lua: &Lua, id: &str, event: &str) -> Vec<Function> {
    let Ok(Some(entry)) = state_entry(lua, id) else {
        return Vec::new();
    };
    let Ok(Some(handlers)) = entry.raw_get::<Option<Table>>("handlers") else {
        return Vec::new();
    };
    let Ok(Some(list)) = handlers.raw_get::<Option<Table>>(event) else {
        return Vec::new();
    };
    (1..=list.raw_len())
        .filter_map(|index| match list.raw_get::<Value>(index) {
            Ok(Value::Function(function)) => Some(function),
            _ => None,
        })
        .collect()
}

fn dispatch(lua: &Lua, host: &Host, only: Option<&str>, event: &str, args: &[Value]) {
    let all = only.is_none();
    let ids = match only {
        Some(id) => vec![id.to_string()],
        None => loaded_ids(host),
    };
    for id in ids {
        for function in handler_list(lua, &id, event) {
            if all && !is_loaded(host, &id) {
                break;
            }
            if let Err(e) = function.call::<()>(MultiValue::from_iter(args.iter().cloned())) {
                super::record_error(lua, host, &id, &e.to_string());
            }
        }
    }
}

pub(super) fn fire_event(lua: &Lua, host: &Host, event: &str, args: Vec<Value>) {
    dispatch(lua, host, None, event, &args);
}

fn take_due_timers(lua: &Lua, id: &str, now: f64) -> Vec<(Function, Vec<Value>)> {
    let Ok(Some(entry)) = state_entry(lua, id) else {
        return Vec::new();
    };
    let Ok(Some(timers)) = entry.raw_get::<Option<Table>>("timers") else {
        return Vec::new();
    };
    let mut due = Vec::new();
    let mut keep = Vec::new();
    for index in 1..=timers.raw_len() {
        let Ok(Value::Table(timer)) = timers.raw_get::<Value>(index) else {
            continue;
        };
        let at = timer.raw_get::<f64>("due").unwrap_or(0.0);
        if at <= now {
            let Ok(Value::Function(function)) = timer.raw_get::<Value>("fn") else {
                continue;
            };
            let args = timer
                .raw_get::<Option<Table>>("args")
                .ok()
                .flatten()
                .map(|args| {
                    (1..=args.raw_len())
                        .filter_map(|index| args.raw_get::<Value>(index).ok())
                        .collect()
                })
                .unwrap_or_default();
            due.push((function, args));
        } else {
            keep.push(timer);
        }
    }
    if !due.is_empty()
        && let Ok(fresh) = lua.create_table()
    {
        for (index, timer) in keep.into_iter().enumerate() {
            let _ = fresh.raw_set(index + 1, timer);
        }
        let _ = entry.raw_set("timers", fresh);
    }
    due
}

pub(super) fn tick(lua: &Lua, host: &Host, dt_ms: f64) {
    if dt_ms < 0.0 || !dt_ms.is_finite() {
        return;
    }
    for id in loaded_ids(host) {
        for function in handler_list(lua, &id, "UPDATE") {
            if !is_loaded(host, &id) {
                break;
            }
            if let Err(e) = function.call::<()>(dt_ms) {
                super::record_error(lua, host, &id, &e.to_string());
            }
        }
        let now = now_ms(lua);
        for (function, args) in take_due_timers(lua, &id, now) {
            if !is_loaded(host, &id) {
                break;
            }
            if let Err(e) = function.call::<()>(MultiValue::from_iter(args)) {
                super::record_error(lua, host, &id, &e.to_string());
            }
        }
    }
}

fn resolve_module(root: &Path, name: &str) -> Result<(PathBuf, u64), String> {
    if name.is_empty() || name.len() > 128 || name.contains('\\') || name.contains(':') || name.contains('\0') {
        return Err(format!("invalid module name {name:?}"));
    }
    if name.starts_with('/') {
        return Err(format!("invalid module name {name:?}"));
    }
    let mut relative = if name.ends_with(".lua") { name.to_string() } else { format!("{name}.lua") };
    relative = relative.replace('\\', "/");
    if relative.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(format!("invalid module name {name:?}"));
    }
    let path: PathBuf = relative.split('/').collect();
    if path.is_absolute() {
        return Err(format!("invalid module name {name:?}"));
    }
    let root = std::fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let path = std::fs::canonicalize(root.join(&path)).map_err(|e| format!("{name}: {e}"))?;
    if !path.starts_with(&root) {
        return Err(format!("{name} is outside the addon folder"));
    }
    let meta = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    if meta.len() > MAX_MODULE_BYTES {
        return Err(format!("{} is too large", path.display()));
    }
    Ok((path, meta.len()))
}

fn require_module(lua: &Lua, host: &Host, id: &str, dir: &Path, env: &Table, name: &str) -> mlua::Result<Value> {
    if name == "api" {
        return Ok(state_entry(lua, id)?
            .and_then(|entry| entry.raw_get::<Option<Table>>("api").ok().flatten())
            .map_or(Value::Nil, Value::Table));
    }
    let entry = state_entry(lua, id)?.ok_or_else(|| error(format!("addon {id}: not loaded")))?;
    let cache: Table = match entry.raw_get::<Option<Table>>("require_cache")? {
        Some(cache) => cache,
        None => {
            let cache = lua.create_table()?;
            entry.raw_set("require_cache", &cache)?;
            cache
        }
    };
    let cached = cache.raw_get::<Value>(name)?;
    if !cached.is_nil() {
        return Ok(cached);
    }
    let (path, bytes) = resolve_module(dir, name).map_err(error)?;
    {
        let mut core = host.0.borrow_mut();
        let addon = core
            .addons
            .iter_mut()
            .find(|addon| addon.id == id)
            .ok_or_else(|| error(format!("addon {id}: not loaded")))?;
        if addon.require_depth >= MAX_REQUIRE_DEPTH {
            return Err(error(format!("addon {id}: require() nested deeper than {MAX_REQUIRE_DEPTH}")));
        }
        if addon.modules >= MAX_MODULES {
            return Err(error(format!("addon {id}: require() loaded more than {MAX_MODULES} modules")));
        }
        if addon.module_bytes.saturating_add(bytes) > MAX_TOTAL_MODULE_BYTES {
            return Err(error(format!("addon {id}: require() modules are too large")));
        }
        addon.require_depth += 1;
    }
    let source = std::fs::read_to_string(&path).map_err(|e| error(format!("{}: {e}", path.display())))?;
    let result = lua
        .load(&source)
        .set_name(format!("@{}", path.display()))
        .set_environment(env.clone())
        .set_mode(ChunkMode::Text)
        .eval::<Value>();
    {
        let mut core = host.0.borrow_mut();
        if let Some(addon) = core.addons.iter_mut().find(|addon| addon.id == id) {
            addon.require_depth = addon.require_depth.saturating_sub(1);
            if result.is_ok() {
                addon.modules += 1;
                addon.module_bytes = addon.module_bytes.saturating_add(bytes);
            }
        }
    }
    let value = result.map_err(|e| error(format!("{}: {e}", path.display())))?;
    let value = if value.is_nil() { Value::Boolean(true) } else { value };
    cache.raw_set(name, &value)?;
    Ok(value)
}

fn host_api(lua: &Lua, host: &Host, id: &str) -> mlua::Result<Table> {
    let api = lua.create_table()?;

    let (h, addon) = (host.clone(), id.to_string());
    api.raw_set(
        "log_info",
        lua.create_function(move |lua, message: String| {
            addon_log(lua, &h, &addon, &message, false);
            Ok(())
        })?,
    )?;

    let (h, addon) = (host.clone(), id.to_string());
    api.raw_set(
        "log_err",
        lua.create_function(move |lua, message: String| {
            addon_log(lua, &h, &addon, &message, true);
            Ok(())
        })?,
    )?;

    let (h, addon) = (host.clone(), id.to_string());
    api.raw_set(
        "get_settings",
        lua.create_function(move |lua, requested: Option<String>| {
            if let Some(other) = requested
                && other != addon
            {
                return Err(error(format!("addon {addon}: cannot read the settings of {other}")));
            }
            settings_table(lua, &h, &addon)
        })?,
    )?;

    let (h, addon) = (host.clone(), id.to_string());
    api.raw_set(
        "save_settings",
        lua.create_function(move |lua, ()| {
            save_settings(lua, &h, &addon);
            Ok(())
        })?,
    )?;

    let addon = id.to_string();
    api.raw_set(
        "on",
        lua.create_function(move |lua, (event, callback): (String, Function)| {
            if !valid_event(&event) {
                return Err(error(format!("addon {addon}: invalid event name {event:?}")));
            }
            let entry = state_entry(lua, &addon)?.ok_or_else(|| error(format!("addon {addon}: not loaded")))?;
            let handlers: Table = match entry.raw_get::<Option<Table>>("handlers")? {
                Some(handlers) => handlers,
                None => {
                    let handlers = lua.create_table()?;
                    entry.raw_set("handlers", &handlers)?;
                    handlers
                }
            };
            let list: Table = match handlers.raw_get::<Option<Table>>(event.as_str())? {
                Some(list) => list,
                None => {
                    let list = lua.create_table()?;
                    handlers.raw_set(event.as_str(), &list)?;
                    list
                }
            };
            list.raw_set(list.raw_len() + 1, callback)?;
            Ok(())
        })?,
    )?;

    let addon = id.to_string();
    api.raw_set(
        "do_in",
        lua.create_function(move |lua, (msec, callback, args): (f64, Function, Option<Table>)| {
            if !msec.is_finite() {
                return Err(error(format!("addon {addon}: DoIn needs a finite duration")));
            }
            let entry = state_entry(lua, &addon)?.ok_or_else(|| error(format!("addon {addon}: not loaded")))?;
            let timers: Table = match entry.raw_get::<Option<Table>>("timers")? {
                Some(timers) => timers,
                None => {
                    let timers = lua.create_table()?;
                    entry.raw_set("timers", &timers)?;
                    timers
                }
            };
            let timer = lua.create_table()?;
            timer.raw_set("due", now_ms(lua) + msec.max(0.0))?;
            timer.raw_set("fn", callback)?;
            timer.raw_set("args", args.unwrap_or(lua.create_table()?))?;
            timers.raw_set(timers.raw_len() + 1, timer)?;
            Ok(())
        })?,
    )?;

    let (h, addon) = (host.clone(), id.to_string());
    api.raw_set(
        "emit",
        lua.create_function(move |lua, (event, args): (String, Variadic<Value>)| {
            if !valid_event(&event) {
                return Err(error(format!("addon {addon}: invalid event name {event:?}")));
            }
            let args: Vec<Value> = args.into();
            dispatch(lua, &h, Some(&addon), &event, &args);
            Ok(())
        })?,
    )?;

    Ok(api)
}

fn create_env(lua: &Lua, host: &Host, id: &str, dir: &Path, addons_root: &Path) -> mlua::Result<(Table, Table)> {
    let env = lua.create_table()?;
    env.raw_set("_G", &env)?;
    env.raw_set("_VERSION", "Lua 5.1")?;
    let globals = lua.globals();
    for name in SAFE_BASE {
        if let Some(value) = globals.raw_get::<Option<Value>>(name)? {
            env.raw_set(name, value)?;
        }
    }
    for name in ["string", "table", "math", "coroutine"] {
        env.raw_set(name, library(lua, name)?)?;
    }

    env.raw_set("UIC", uic_table(lua)?)?;
    let ui_parent: Table = globals
        .raw_get::<Option<Table>>("UIParent")?
        .ok_or_else(|| error("UIParent is not installed"))?;
    env.raw_set("UIParent", proxy(lua, ui_parent, "UIParent", id)?)?;
    if let Some(addon) = globals.raw_get::<Option<Table>>("ADDON")? {
        env.raw_set("ADDON", proxy(lua, addon, "ADDON", id)?)?;
    }
    let game: Vec<(String, Table)> = globals
        .pairs::<Value, Value>()
        .filter_map(|pair| {
            let (key, value) = pair.ok()?;
            let Value::String(key) = key else { return None };
            let name = key.to_str().ok()?;
            let Value::Table(table) = value else { return None };
            name.starts_with("X2").then(|| (name.to_string(), table))
        })
        .collect();
    for (name, table) in game {
        env.raw_set(name.as_str(), proxy(lua, table, &name, id)?)?;
    }

    env.raw_set("__ADDON_HOST", host_api(lua, host, id)?)?;
    let (h, addon, folder) = (host.clone(), id.to_string(), dir.to_path_buf());
    let environment = env.clone();
    env.raw_set(
        "require",
        lua.create_function(move |lua, name: String| require_module(lua, &h, &addon, &folder, &environment, &name))?,
    )?;

    lua.load(API_LUA)
        .set_name("@addons/api.lua")
        .set_environment(env.clone())
        .set_mode(ChunkMode::Text)
        .exec()?;
    let api: Table = env
        .raw_get::<Option<Table>>("ADDON_API")?
        .ok_or_else(|| error("api.lua did not define ADDON_API"))?;
    api.raw_set("baseDir", addons_root.to_string_lossy().as_ref())?;
    Ok((env, api))
}

fn cleanup(lua: &Lua, id: &str) {
    if let Ok(Some(entry)) = state_entry(lua, id)
        && let Ok(Some(root)) = entry.raw_get::<Option<Table>>("root")
    {
        free_widget(&root);
    }
    let _ = remove_state(lua, id);
}

pub(super) fn load_addon(lua: &Lua, host: &Host, index: usize) -> Result<Manifest, String> {
    let (id, dir, addons_root) = {
        let core = host.0.borrow();
        let entry: &AddonEntry = core.addons.get(index).ok_or("no such addon")?;
        (entry.id.clone(), entry.dir.clone(), core.root.clone())
    };
    let result = load_addon_inner(lua, host, &id, &dir, &addons_root);
    if result.is_err() {
        cleanup(lua, &id);
    }
    result
}

fn load_addon_inner(lua: &Lua, host: &Host, id: &str, dir: &Path, addons_root: &Path) -> Result<Manifest, String> {
    let (env, api) = create_env(lua, host, id, dir, addons_root).map_err(|e| e.to_string())?;
    let root = create_root(lua, id).map_err(|e| e.to_string())?;
    api.raw_set("rootWindow", &root).map_err(|e| e.to_string())?;

    let entry = lua.create_table().map_err(|e| e.to_string())?;
    entry.raw_set("env", &env).map_err(|e| e.to_string())?;
    entry.raw_set("api", &api).map_err(|e| e.to_string())?;
    entry.raw_set("root", &root).map_err(|e| e.to_string())?;
    entry.raw_set("handlers", lua.create_table().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    entry.raw_set("timers", lua.create_table().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    entry
        .raw_set("require_cache", lua.create_table().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    addons_state(lua)
        .and_then(|addons| addons.raw_set(id, &entry))
        .map_err(|e| e.to_string())?;

    let source = manifest::read_main(dir)?;
    let main = lua
        .load(&source)
        .set_name(format!("@{}", dir.join(manifest::MAIN_LUA).display()))
        .set_environment(env)
        .set_mode(ChunkMode::Text)
        .eval::<Value>()
        .map_err(|e| format!("main.lua: {e}"))?;
    let Value::Table(main) = main else {
        return Err("main.lua must return a table".into());
    };
    let manifest = manifest_of(&main, id).map_err(|e| e.to_string())?;
    entry.raw_set("main", &main).map_err(|e| e.to_string())?;
    let on_load = main.raw_get::<Option<Function>>("OnLoad").map_err(|e| e.to_string())?;
    if let Some(function) = on_load {
        function.call::<()>(()).map_err(|e| format!("OnLoad: {e}"))?;
    }
    Ok(manifest)
}

pub(super) fn unload_addon(lua: &Lua, host: &Host, index: usize) {
    let id = match host.0.borrow().addons.get(index) {
        Some(entry) => entry.id.clone(),
        None => return,
    };
    let Some(entry) = state_entry(lua, &id).ok().flatten() else {
        return;
    };
    if let Some(main) = entry.raw_get::<Option<Table>>("main").ok().flatten()
        && let Some(on_unload) = main.raw_get::<Option<Function>>("OnUnload").ok().flatten()
        && let Err(e) = on_unload.call::<()>(())
    {
        bevy::log::warn!("addon {id}: OnUnload: {e}");
        super::push_log(host, &id, &format!("OnUnload error: {e}"));
    }
    save_settings(lua, host, &id);
    cleanup(lua, &id);
}

pub(super) fn install_bridge(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    if globals.raw_get::<Value>("ADDON_HOST_EVENT")?.is_nil() {
        globals.raw_set(
            "ADDON_HOST_EVENT",
            lua.create_function(|lua, (event, args): (String, Variadic<Value>)| {
                if let Some(host) = super::host(lua) {
                    fire_event(lua, &host, &event, args.into());
                }
                Ok(())
            })?,
        )?;
    }
    lua.load(BRIDGE_LUA).set_name("@addons/bridge.lua").exec()
}

pub(super) fn reset_state(lua: &Lua, root: &str) -> mlua::Result<()> {
    let state = lua.create_table()?;
    state.raw_set("addons", lua.create_table()?)?;
    state.raw_set("root", root)?;
    lua.globals().raw_set("ADDON_STATE", state)
}

pub(super) fn clear_state(lua: &Lua) {
    let _ = lua.globals().raw_set("ADDON_STATE", Value::Nil);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn module_paths_cannot_escape_the_addon() {
        let base = std::env::temp_dir().join(format!("aa-addons-require-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("lib")).unwrap();
        std::fs::write(base.join("util.lua"), "return 1").unwrap();
        std::fs::write(base.join("lib/thing.lua"), "return 2").unwrap();
        std::fs::write(base.join("text.txt"), "no").unwrap();
        assert!(resolve_module(&base, "util").unwrap().0.ends_with("util.lua"));
        assert!(resolve_module(&base, "util.lua").is_ok());
        assert!(resolve_module(&base, "lib/thing").is_ok());
        for bad in [
            "../x", "a/../../x", "/etc/passwd", "C:/x", "C:x", "a\\b", "", ".", "..", "a//b", "lib/../util",
        ] {
            assert!(resolve_module(&base, bad).is_err(), "{bad:?}");
        }
        assert!(resolve_module(&base, "text").is_err());
        let _ = std::fs::remove_dir_all(PathBuf::from(&base));
    }
}

