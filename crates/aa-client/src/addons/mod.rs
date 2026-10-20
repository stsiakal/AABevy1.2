//! ArcheAge-Classic-compatible addons: local Lua folders loaded after the world-stage UI scripts.
//!
//! The addons directory is the first existing one of `AA_ADDONS_DIR`,
//! `%LOCALAPPDATA%/AABevy1.2/addons` and `<exe>/addons`. Its `addons.txt` lists the enabled folder
//! names; each folder has a `main.lua` returning `{ name, author, desc, version, OnLoad, OnUnload }`.
//!
//! Every addon runs in its own environment ([`sandbox`]): safe base/string/table/math/coroutine
//! functions, read-only views of `X2*`, `ADDON` and `UIParent`, `UIC`, `ADDON_API`, and a
//! `require` that only reads `.lua` files inside the addon's folder. Loads and callbacks are
//! protected; an addon is disabled after five errors in a minute. This isolates faults, not
//! hostile code: the loader never gives addons the network, processes, arbitrary files or the
//! real Lua globals, but it is not a security boundary against a local attacker.

mod manifest;
mod sandbox;
mod settings;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use mlua::Lua;

use manifest::Manifest;

const ERROR_LIMIT: usize = 5;
const ERROR_WINDOW: Duration = Duration::from_secs(60);
const LOG_LINES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    Loaded,
    Disabled,
    Failed,
}

pub(super) struct AddonEntry {
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Option<Manifest>,
    pub state: State,
    pub last_error: Option<String>,
    pub errors: VecDeque<Instant>,
    pub log: VecDeque<String>,
    pub disabled_reason: Option<String>,
    pub require_depth: u32,
    pub modules: u32,
    pub module_bytes: u64,
    pub settings_path: PathBuf,
}

pub(super) struct HostCore {
    pub root: PathBuf,
    pub settings_dir: PathBuf,
    pub addons: Vec<AddonEntry>,
    pub active: bool,
}

#[derive(Clone)]
pub(super) struct Host(pub Rc<RefCell<HostCore>>);

pub(super) fn host(lua: &Lua) -> Option<Host> {
    lua.app_data_ref::<Host>().map(|host| Host(host.0.clone()))
}

fn head(text: &str, max: usize) -> &str {
    let mut end = max.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn clean_error(error: &str) -> String {
    let line = error.lines().next().unwrap_or("").trim();
    if line.len() > 300 {
        format!("{}...", head(line, 300))
    } else {
        line.to_string()
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    format!("{}...", head(text, max))
}

pub(super) fn push_log(host: &Host, id: &str, line: &str) {
    let mut core = host.0.borrow_mut();
    if let Some(addon) = core.addons.iter_mut().find(|addon| addon.id == id) {
        if addon.log.len() >= LOG_LINES {
            addon.log.pop_front();
        }
        addon.log.push_back(truncate(line, 512));
    }
}

fn status(addon: &AddonEntry) -> &'static str {
    match addon.state {
        State::Loaded => "loaded",
        State::Disabled => "disabled",
        State::Failed => "failed",
    }
}

fn find(host: &Host, id: &str) -> Option<usize> {
    host.0.borrow().addons.iter().position(|addon| addon.id == id)
}

fn retry(lua: &Lua, host: &Host, index: usize) {
    let id = match host.0.borrow().addons.get(index) {
        Some(addon) => addon.id.clone(),
        None => return,
    };
    match sandbox::load_addon(lua, host, index) {
        Ok(manifest) => {
            let version = manifest.version.clone();
            let mut core = host.0.borrow_mut();
            if let Some(addon) = core.addons.get_mut(index) {
                addon.manifest = Some(manifest);
                addon.state = State::Loaded;
                addon.errors.clear();
                addon.disabled_reason = None;
                addon.last_error = None;
            }
            drop(core);
            push_log(host, &id, &format!("loaded (version {version})"));
            bevy::log::info!("addons: {id}: loaded (version {version})");
        }
        Err(error) => {
            let error = clean_error(&error);
            let mut core = host.0.borrow_mut();
            if let Some(addon) = core.addons.get_mut(index) {
                addon.state = State::Failed;
                addon.last_error = Some(error.clone());
            }
            drop(core);
            push_log(host, &id, &format!("error: {error}"));
            bevy::log::warn!("addon {id}: {error}");
        }
    }
}

pub(super) fn record_error(lua: &Lua, host: &Host, id: &str, error: &str) {
    let error = clean_error(error);
    let over = {
        let mut core = host.0.borrow_mut();
        let Some(addon) = core.addons.iter_mut().find(|addon| addon.id == id) else {
            return;
        };
        addon.last_error = Some(error.clone());
        if addon.state != State::Loaded {
            false
        } else {
            addon.errors.push_back(Instant::now());
            while addon
                .errors
                .front()
                .is_some_and(|seen| seen.elapsed() > ERROR_WINDOW)
            {
                addon.errors.pop_front();
            }
            addon.errors.len() >= ERROR_LIMIT
        }
    };
    bevy::log::warn!("addon {id}: {error}");
    push_log(host, id, &format!("error: {error}"));
    if over {
        disable(lua, host, id, "too many errors");
    }
}

fn disable(lua: &Lua, host: &Host, id: &str, reason: &str) {
    let Some(index) = find(host, id) else { return };
    sandbox::unload_addon(lua, host, index);
    {
        let mut core = host.0.borrow_mut();
        if let Some(addon) = core.addons.get_mut(index) {
            addon.state = State::Disabled;
            addon.disabled_reason = Some(reason.to_string());
        }
    }
    push_log(host, id, &format!("disabled: {reason}"));
    bevy::log::warn!("addon {id}: disabled ({reason})");
    sandbox::say(lua, &format!("addon {id} disabled: {reason}"), true);
}

fn add_entry(host: &Host, id: &str) -> Option<usize> {
    let mut core = host.0.borrow_mut();
    if core.root.as_os_str().is_empty() {
        return None;
    }
    if let Some(index) = core.addons.iter().position(|addon| addon.id == id) {
        return Some(index);
    }
    let dir = core.root.join(id);
    let settings_path = core.settings_dir.join(format!("{id}.json"));
    core.addons.push(AddonEntry {
        id: id.to_string(),
        dir,
        manifest: None,
        state: State::Disabled,
        last_error: None,
        errors: VecDeque::new(),
        log: VecDeque::new(),
        disabled_reason: None,
        require_depth: 0,
        modules: 0,
        module_bytes: 0,
        settings_path,
    });
    Some(core.addons.len() - 1)
}

fn populate(lua: &Lua, host: &Host, root: Option<&Path>) {
    let root_text = root.map(|root| root.to_string_lossy().into_owned()).unwrap_or_default();
    if let Err(error) = sandbox::reset_state(lua, &root_text) {
        bevy::log::warn!("addons: state: {error}");
        return;
    }
    let Some(root) = root.filter(|root| !root.as_os_str().is_empty()) else {
        return;
    };
    let enabled = match manifest::read_enabled(root) {
        Ok(enabled) => enabled,
        Err(error) => {
            bevy::log::warn!("addons: {error}");
            Vec::new()
        }
    };
    host.0.borrow_mut().addons.clear();
    for id in enabled {
        if !manifest::valid_folder(&id) {
            continue;
        }
        let Some(index) = add_entry(host, &id) else { continue };
        retry(lua, host, index);
    }
}

fn load_from(lua: &Lua, root: Option<&Path>, settings_dir: &Path) {
    if host(lua).is_some_and(|host| host.0.borrow().active) {
        unload(lua);
    }
    let core = HostCore {
        root: root.map(Path::to_path_buf).unwrap_or_default(),
        settings_dir: settings_dir.to_path_buf(),
        addons: Vec::new(),
        active: false,
    };
    let host = Host(Rc::new(RefCell::new(core)));
    lua.set_app_data(host.clone());
    populate(lua, &host, root);
    if let Err(error) = sandbox::install_bridge(lua) {
        bevy::log::warn!("addons: event bridge: {error}");
    }
    host.0.borrow_mut().active = true;
    let (loaded, total) = {
        let core = host.0.borrow();
        (
            core.addons.iter().filter(|addon| addon.state == State::Loaded).count(),
            core.addons.len(),
        )
    };
    match root {
        Some(root) => bevy::log::info!(
            "addons: {loaded} of {total} loaded from {}",
            root.display()
        ),
        None => bevy::log::info!(
            "addons: no addons directory (AA_ADDONS_DIR, %LOCALAPPDATA%/AABevy1.2/addons, <exe>/addons)"
        ),
    }
}

pub(crate) fn load(lua: &Lua) {
    load_from(lua, manifest::find_root().as_deref(), &settings::settings_dir());
}

pub(crate) fn unload(lua: &Lua) {
    let Some(host) = host(lua) else { return };
    if !host.0.borrow().active {
        return;
    }
    let count = host.0.borrow().addons.len();
    for index in (0..count).rev() {
        if host
            .0
            .borrow()
            .addons
            .get(index)
            .is_some_and(|addon| addon.state == State::Loaded)
        {
            sandbox::unload_addon(lua, &host, index);
        }
    }
    host.0.borrow_mut().active = false;
    sandbox::clear_state(lua);
    lua.remove_app_data::<Host>();
    bevy::log::info!("addons: unloaded");
}

pub(crate) fn unload_world(world: &mut World) {
    if let Some(rt) = world.get_non_send::<aa_ui::UiRuntime>() {
        unload(&rt.lua);
    }
}

pub(crate) fn tick(rt: Option<NonSend<aa_ui::UiRuntime>>, time: Res<Time>) {
    let Some(rt) = rt else { return };
    let Some(host) = host(&rt.lua) else { return };
    if !host.0.borrow().active {
        return;
    }
    sandbox::tick(&rt.lua, &host, f64::from(time.delta_secs()) * 1000.0);
}

pub(crate) fn is_command(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("/addons"))
}

pub(crate) fn handle_commands(
    rt: Option<NonSend<aa_ui::UiRuntime>>,
    bridge: NonSend<crate::hud::api::HudBridge>,
) {
    let commands = {
        let mut data = bridge.0.borrow_mut();
        if data.addon_commands.is_empty() {
            return;
        }
        std::mem::take(&mut data.addon_commands)
    };
    let Some(rt) = rt else { return };
    for command in commands {
        run_command(&rt.lua, &command);
    }
}

fn run_command(lua: &Lua, line: &str) {
    let Some(host) = host(lua) else {
        sandbox::say(lua, "addons: not loaded", true);
        return;
    };
    let mut words = line.split_whitespace();
    words.next();
    let command = words.next().unwrap_or("list").to_ascii_lowercase();
    let argument = words.next();
    match command.as_str() {
        "list" => list(lua, &host),
        "enable" => enable(lua, &host, argument),
        "disable" => disable_command(lua, &host, argument),
        "reload" => reload(lua, &host, argument),
        "log" => show_log(lua, &host, argument),
        other => sandbox::say(
            lua,
            &format!("addons: unknown command {other:?}; use list, enable, disable, reload or log"),
            true,
        ),
    }
}

fn list(lua: &Lua, host: &Host) {
    let (root, lines) = {
        let core = host.0.borrow();
        let mut lines = Vec::new();
        for addon in &core.addons {
            let (name, version) = addon
                .manifest
                .as_ref()
                .map_or(("-", "-".to_string()), |manifest| (manifest.name.as_str(), manifest.version.clone()));
            let mut line = format!("{} {name} {version} {}", addon.id, status(addon));
            if let Some(error) = addon.last_error.as_deref() {
                line.push_str(&format!(" ({error})"));
            }
            lines.push(line);
        }
        (core.root.clone(), lines)
    };
    if lines.is_empty() {
        sandbox::say(
            lua,
            &format!("addons: none installed in {}", root.display()),
            false,
        );
        return;
    }
    sandbox::say(lua, &format!("addons: {} installed", lines.len()), false);
    for line in lines {
        sandbox::say(lua, &line, false);
    }
}

fn enable(lua: &Lua, host: &Host, id: Option<&str>) {
    let indices: Vec<usize> = match id {
        Some(id) => match find(host, id) {
            Some(index) => vec![index],
            None => {
                if !manifest::valid_folder(id) {
                    sandbox::say(lua, &format!("addons: invalid folder {id:?}"), true);
                    return;
                }
                match add_entry(host, id) {
                    Some(index) => vec![index],
                    None => {
                        sandbox::say(lua, "addons: no addons directory", true);
                        return;
                    }
                }
            }
        },
        None => (0..host.0.borrow().addons.len()).collect(),
    };
    for index in indices {
        let loaded = host
            .0
            .borrow()
            .addons
            .get(index)
            .is_some_and(|addon| addon.state == State::Loaded);
        if !loaded {
            retry(lua, host, index);
        }
    }
}

fn disable_command(lua: &Lua, host: &Host, id: Option<&str>) {
    let indices: Vec<usize> = match id {
        Some(id) => match find(host, id) {
            Some(index) => vec![index],
            None => {
                sandbox::say(lua, &format!("addons: no addon {id:?}"), true);
                return;
            }
        },
        None => (0..host.0.borrow().addons.len()).rev().collect(),
    };
    for index in indices {
        let Some(id) = host.0.borrow().addons.get(index).map(|addon| addon.id.clone()) else {
            continue;
        };
        if host.0.borrow().addons[index].state != State::Loaded {
            continue;
        }
        sandbox::unload_addon(lua, host, index);
        {
            let mut core = host.0.borrow_mut();
            if let Some(addon) = core.addons.get_mut(index) {
                addon.state = State::Disabled;
                addon.disabled_reason = Some("disabled from chat".into());
            }
        }
        push_log(host, &id, "disabled from chat");
        sandbox::say(lua, &format!("addon {id}: disabled"), false);
    }
}

fn reload(lua: &Lua, host: &Host, id: Option<&str>) {
    if let Some(id) = id {
        let index = match find(host, id) {
            Some(index) => Some(index),
            None => {
                if manifest::valid_folder(id) {
                    add_entry(host, id)
                } else {
                    sandbox::say(lua, &format!("addons: invalid folder {id:?}"), true);
                    None
                }
            }
        };
        let Some(index) = index else {
            sandbox::say(lua, "addons: no addons directory", true);
            return;
        };
        if host
            .0
            .borrow()
            .addons
            .get(index)
            .is_some_and(|addon| addon.state == State::Loaded)
        {
            sandbox::unload_addon(lua, host, index);
        }
        retry(lua, host, index);
        return;
    }
    let root = host.0.borrow().root.clone();
    let count = host.0.borrow().addons.len();
    for index in (0..count).rev() {
        if host
            .0
            .borrow()
            .addons
            .get(index)
            .is_some_and(|addon| addon.state == State::Loaded)
        {
            sandbox::unload_addon(lua, host, index);
        }
    }
    let root = (!root.as_os_str().is_empty()).then_some(root.as_path());
    populate(lua, host, root);
    let (loaded, total) = {
        let core = host.0.borrow();
        (
            core.addons.iter().filter(|addon| addon.state == State::Loaded).count(),
            core.addons.len(),
        )
    };
    sandbox::say(lua, &format!("addons: reloaded {loaded} of {total}"), false);
}

fn show_log(lua: &Lua, host: &Host, id: Option<&str>) {
    let Some(id) = id else {
        sandbox::say(lua, "addons: usage: /addons log <folder>", true);
        return;
    };
    let Some(index) = find(host, id) else {
        sandbox::say(lua, &format!("addons: no addon {id:?}"), true);
        return;
    };
    let lines: Vec<String> = {
        let core = host.0.borrow();
        let addon = &core.addons[index];
        addon.log.iter().rev().take(12).rev().cloned().collect()
    };
    if lines.is_empty() {
        sandbox::say(lua, &format!("addon {id}: no log"), false);
        return;
    }
    for line in lines {
        sandbox::say(lua, &format!("{id}: {line}"), false);
    }
}

