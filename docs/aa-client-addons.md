# aa-client addons

ArcheAge-Classic-compatible addons: folders of local Lua files loaded into the in-game UI after the
original world-stage scripts, before the entering-world events. Phase 1 implements the loader, the
sandbox and the `ADDON_API` subset listed below.

Code: `crates/aa-client/src/addons/` (`manifest.rs`, `sandbox.rs`, `settings.rs`, `api.lua`).
Example addon: `crates/aa-client/addons/hello/`.

## Installing

The addons directory is the first existing one of:

1. `AA_ADDONS_DIR` (an environment variable), for tests and portable installs;
2. `%LOCALAPPDATA%/AABevy1.2/addons`;
3. `<exe>/addons` (the directory of the running `aa-client.exe`).

Its `addons.txt` lists the enabled folder names in load order, one per line; `#` starts a comment.
Missing folders are logged and shown as `failed`; they do not stop the other addons.

```text
%LOCALAPPDATA%/AABevy1.2/addons/
  addons.txt          # hello
  hello/
    main.lua
    lib/util.lua      # optional modules for require
```

`main.lua` runs in the addon's environment and returns its manifest:

```lua
return {
  name = "Hello",
  author = "AABevy1.2",
  desc = "Example addon.",
  version = "1.0.0",
  OnLoad = function() end,
  OnUnload = function() end,
}
```

`name`, `author`, `desc` and `version` are optional; `OnLoad` and `OnUnload` are called under
`pcall`. Addons are loaded when the in-game HUD is built and unloaded (with a settings flush) when
the world is left.

## ADDON_API (phase 1)

| member | meaning |
|---|---|
| `ADDON_API.Log:Info(message)` / `Log:Err(message)` | a yellow / red line in chat |
| `ADDON_API.GetSettings(addonId)` | this addon's settings table (the id must be its own folder) |
| `ADDON_API.SaveSettings()` | write the settings table now |
| `ADDON_API.On(event, fn)` | register `UPDATE`, `CHAT_MESSAGE`, `TEAM_MEMBERS_CHANGED` or an addon-local event |
| `ADDON_API:DoIn(msec, fn, ...)` | one-shot timer on the UI clock (`X2Time:GetUiMsec`), arguments passed to `fn` |
| `ADDON_API:Emit(event, ...)` | call this addon's own handlers for `event` |
| `ADDON_API.Interface:CreateWindow(id, title, x, y)` | a standard window (680 x 615; `tabs` is accepted but not built yet) |
| `ADDON_API.Interface:CreateWidget(type, id, parent)` | `UIParent:CreateWidget` with `rootWindow` as the default parent |
| `ADDON_API.Interface:CreateEmptyWindow(id)` | a window without a title |
| `ADDON_API.Interface:Free(wnd)` | hide and remove a window (`EnableHidingIsRemove`) |
| `ADDON_API.rootWindow` | the addon's root widget |
| `ADDON_API.baseDir` | the addons directory |
| `print(...)` | `ADDON_API.Log:Info` |

`UPDATE` handlers receive the frame time in milliseconds. `CHAT_MESSAGE` and
`TEAM_MEMBERS_CHANGED` receive the same arguments as the original UI scripts' handlers.
`Emit` is addon-local in phase 1.

`UIC` is a table of the client's `UIC_*` constants with the prefix removed (`UIC.BAG`,
`UIC.SKILL`, ...; the `UIC_BAG` spelling is also present). `ADDON`, `UIParent` and the `X2*` API
objects are read-only views: methods work, but assigning a field to one of those tables raises an
error.

`require(name)` loads `name.lua` (or `name`) from the addon's own folder, for example
`require("lib/util")`. Absolute paths, drive letters, `..`, backslashes and files outside the
folder are rejected. Modules are cached; nesting is limited to 8 and 64 files / 4 MB per addon.
`require("api")` returns `ADDON_API`.

### Settings

Settings are one JSON file per addon at
`%LOCALAPPDATA%/AABevy1.2/addons/settings/<folder>.json`, written atomically (temporary file, then
rename). Tables become arrays when their keys are exactly 1..n, otherwise objects; string keys
beginning with `#` and integer keys are escaped so they round-trip. Limits: 16 levels, 4096 keys,
64 KB strings, 4 MB file. The table is flushed when the addon is unloaded, even if `SaveSettings`
was not called.

## Chat commands

Typed into the chat input, not sent to the server:

```text
/addons                      list the installed addons (id, name, version, state, last error)
/addons list
/addons enable [folder]      load a disabled/failed addon (all when omitted)
/addons disable [folder]     unload and disable at runtime (all when omitted)
/addons reload [folder]      unload and load again (re-reads addons.txt when no folder is given)
/addons log <folder>         the addon's recent loader messages and errors
```

Runtime enable/disable is not written back to `addons.txt`. A missing folder listed there is shown
as `failed` with its error; fixing it and `/addons enable <folder>` retries without a restart.

## Errors and isolation

Every load (`main.lua`, `OnLoad`, required modules) and every callback runs protected. A failure is
logged as `addon <id>: ...`, kept as the addon's last error and counted; five errors within a
minute disable the addon (its `OnUnload` is attempted, its handlers and timers are dropped). Other
addons and the UI keep running.

The sandbox provides `assert`, `error`, `ipairs`, `next`, `pairs`, `pcall`, `select`, `tonumber`,
`tostring`, `type`, `unpack`, `xpcall`, `rawequal`, `rawget`, `rawset`, and copies of the
`string`, `table`, `math` and `coroutine` libraries. It does **not** provide `io`, `os`, `package`,
`debug`, `load`, `loadstring`, `loadfile`, `dofile`, `getfenv`, `setfenv`, `getmetatable`,
`setmetatable`, `newproxy` or `module`.

This isolates faults, not hostile code. An addon cannot reach the network, start processes, read
arbitrary files or assign to the game's tables, but a local addon the user installs is still code
the user chose to run; the loader is not a security boundary against a local attacker.

## Example

Copy `crates/aa-client/addons/hello` into the addons directory and put `hello` in `addons.txt`.
It logs to chat, creates a small window, counts `UPDATE` calls, fires a `DoIn` timer, counts
`CHAT_MESSAGE` lines and saves a run counter in its settings.

Tests (`cargo test -p aa-client addons`) cover folder/id sanitising, `require` traversal, JSON
round-trips and limits, callback isolation and the five-errors-per-minute disable, the read-only
views, the chat-event bridge and the sample addon's load/update/timer/settings cycle.

## Not in phase 1

`File:Read/Write`, the rest of the `ADDON_API` domains (`Unit`, `Team`, `Bag`, `Ability`, `Item`,
`Skill`, `Store`, `Bank`, ...), content registration/show (`ADDON:GetContent`,
`RegisterContentWidget`, `ShowContent`), `UI_RELOADED` / `UPDATE_PING_INFO`, the status bar, combo
box and page-control helpers, a character-select addons window and persistent enable/disable.
