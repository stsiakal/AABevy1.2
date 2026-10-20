use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use aa_ui::UiRuntime;

use super::{Host, State};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "aa-addons-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn addon(&self, id: &str, main: &str) {
        let dir = self.0.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.lua"), main).unwrap();
    }

    fn enabled(&self, ids: &str) {
        std::fs::write(self.0.join("addons.txt"), ids).unwrap();
    }

    fn settings(&self) -> PathBuf {
        self.0.join("settings")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn runtime() -> UiRuntime {
    UiRuntime::new("en_us", 960.0, 540.0).unwrap()
}

fn host(rt: &UiRuntime) -> Host {
    super::host(&rt.lua).expect("addon host")
}

fn load(rt: &UiRuntime, dir: &TempDir) {
    super::load_from(&rt.lua, Some(dir.path()), &dir.settings());
}

fn value<T: mlua::FromLua>(rt: &UiRuntime, code: &str) -> T {
    rt.lua.load(code).eval::<T>().unwrap()
}

#[test]
fn chat_command_names_are_recognised() {
    assert!(super::is_command("/addons"));
    assert!(super::is_command("  /ADDONS list "));
    assert!(super::is_command("/addons reload hello"));
    assert!(!super::is_command("/addonsx"));
    assert!(!super::is_command("/say /addons"));
}

#[test]
fn the_sample_addon_loads_updates_timers_and_persists_settings() {
    let dir = TempDir::new("sample");
    dir.enabled("hello\n");
    dir.addon("hello", include_str!("../../addons/hello/main.lua"));

    let rt = runtime();
    load(&rt, &dir);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    assert!(value::<bool>(
        &rt,
        "local s = assert(ADDON_STATE.addons.hello); \
         return type(s.env.ADDON_API) == 'table' and s.env.ONLOAD_RAN == true and s.env.UPDATE_COUNT == nil"
    ));

    let h = host(&rt);
    super::sandbox::tick(&rt.lua, &h, 16.0);
    super::sandbox::tick(&rt.lua, &h, 16.0);
    assert!(value::<bool>(
        &rt,
        "local s = ADDON_STATE.addons.hello; return s.env.UPDATE_COUNT == 2 and s.env.LAST_DT == 16"
    ));

    aa_ui::lua::ui(&rt.lua).time_ms = 1000.0;
    super::sandbox::tick(&rt.lua, &h, 16.0);
    assert!(value::<bool>(&rt, "return ADDON_STATE.addons.hello.env.TIMER_FIRED == true"));

    rt.fire_event(
        "CHAT_MESSAGE",
        (
            2i64,
            "1".to_string(),
            false,
            "Tester".to_string(),
            "hello".to_string(),
            true,
        ),
    );
    assert!(value::<bool>(&rt, "return ADDON_STATE.addons.hello.env.CHAT_LINES == 1"));

    assert_eq!(
        super::settings::load(&dir.settings().join("hello.json")).unwrap()["count"],
        1
    );

    super::unload(&rt.lua);
    load(&rt, &dir);
    assert_eq!(
        super::settings::load(&dir.settings().join("hello.json")).unwrap()["count"],
        2
    );

    let h = host(&rt);
    let core = h.0.borrow();
    assert_eq!(core.addons.len(), 1);
    assert_eq!(core.addons[0].state, State::Loaded);
    assert!(core.addons[0].last_error.is_none(), "{:?}", core.addons[0].last_error);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
}

#[test]
fn a_failing_addon_does_not_break_the_others_and_is_disabled() {
    let dir = TempDir::new("isolation");
    dir.enabled("good\nbad\n");
    dir.addon(
        "good",
        r#"
return {
  name = "Good", version = "1",
  OnLoad = function()
    ADDON_API.On("UPDATE", function(dt) GOOD_TICKS = (GOOD_TICKS or 0) + 1 end)
    ADDON_API.On("ADDON_LOCAL", function(value) EMIT_VALUE = value end)
    ADDON_API:Emit("ADDON_LOCAL", 7)
  end,
}
"#,
    );
    dir.addon(
        "bad",
        r#"
return {
  name = "Bad", version = "1",
  OnLoad = function()
    ADDON_API.On("UPDATE", function() error("boom") end)
  end,
}
"#,
    );

    let rt = runtime();
    load(&rt, &dir);
    let h = host(&rt);
    for _ in 0..6 {
        super::sandbox::tick(&rt.lua, &h, 16.0);
    }

    assert_eq!(value::<i64>(&rt, "return ADDON_STATE.addons.good.env.GOOD_TICKS"), 6);
    assert_eq!(value::<i64>(&rt, "return ADDON_STATE.addons.good.env.EMIT_VALUE"), 7);
    assert!(value::<bool>(&rt, "return ADDON_STATE.addons.bad == nil"));

    let core = h.0.borrow();
    let bad = core.addons.iter().find(|addon| addon.id == "bad").unwrap();
    let good = core.addons.iter().find(|addon| addon.id == "good").unwrap();
    assert_eq!(bad.state, State::Disabled);
    assert!(bad.last_error.as_deref().is_some_and(|error| error.contains("boom")));
    assert_eq!(good.state, State::Loaded);
    drop(core);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
}

#[test]
fn addon_require_is_local_and_the_environment_is_safe() {
    let dir = TempDir::new("sandbox");
    dir.enabled("sandbox\n");
    dir.addon(
        "sandbox",
        r#"
local util = require("lib/util")
assert(require("api") == ADDON_API)
assert(io == nil and os == nil and package == nil and debug == nil)
assert(load == nil and loadstring == nil and loadfile == nil and dofile == nil)
assert(getfenv == nil and setfenv == nil and newproxy == nil and module == nil)
assert(getmetatable == nil and setmetatable == nil)
local readonly = pcall(function() ADDON.Hacked = true end)
assert(not readonly)
local outside = pcall(function() require("../outside") end)
assert(not outside)
assert(type(print) == "function" and type(UIC) == "table" and type(UIParent) == "table")
require("lib/util")
return {
  name = "Sandbox", version = "1",
  OnLoad = function()
    SANDBOX_RAN = true
    REQUIRE_VALUE = util.value
  end,
}
"#,
    );
    let lib = dir.path().join("sandbox/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("util.lua"), "return { value = 42 }").unwrap();
    std::fs::write(dir.path().join("outside.lua"), "return { value = 0 }").unwrap();

    let rt = runtime();
    load(&rt, &dir);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    assert_eq!(
        value::<i64>(&rt, "return ADDON_STATE.addons.sandbox.env.REQUIRE_VALUE"),
        42
    );
    let h = host(&rt);
    let core = h.0.borrow();
    let addon = core.addons.iter().find(|addon| addon.id == "sandbox").unwrap();
    assert_eq!(addon.state, State::Loaded);
    assert_eq!(addon.modules, 1);
    assert!(addon.last_error.is_none());
}

#[test]
fn addon_chat_lines_use_the_system_channel() {
    let rt = runtime();
    assert_eq!(value::<i64>(&rt, "return CMF_SYSTEM"), 12);
    rt.lua
        .load(
            "local dispatch = X2Chat.DispatchChatMessage \
             X2Chat.DispatchChatMessage = function(self, filter, text) \
               CAPTURED_FILTER, CAPTURED_TEXT = filter, text \
             end",
        )
        .exec()
        .unwrap();
    super::sandbox::say(&rt.lua, "hello", false);
    assert_eq!(value::<i64>(&rt, "return CAPTURED_FILTER"), 12);
    assert_eq!(value::<String>(&rt, "return CAPTURED_TEXT"), "hello");
}

#[test]
fn the_sample_addon_window_is_visible_without_a_command() {
    let dir = TempDir::new("window");
    dir.enabled("hello\n");
    dir.addon("hello", include_str!("../../addons/hello/main.lua"));

    let rt = runtime();
    load(&rt, &dir);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);

    let (root_visible, window_visible, width, height, label) = {
        let u = aa_ui::lua::ui(&rt.lua);
        let root = u.tree.find("addon_hello_root").expect("addon root");
        let window = u.tree.find("hello_window").expect("hello window");
        let label = u.tree.find("hello_label").expect("hello label");
        (
            u.tree.visible(root),
            u.tree.visible(window),
            u.tree.get(window).width,
            u.tree.get(window).height,
            u.tree.get(label).text().map(|text| text.text.clone()),
        )
    };
    assert!(root_visible, "the addon root must be shown");
    assert!(window_visible, "the sample window must be visible in world");
    assert_eq!((width, height), (260.0, 96.0));
    assert_eq!(label.as_deref(), Some("Hello from an addon!"));
}
