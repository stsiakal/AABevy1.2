# Port-engine survey: core runtime modules (cryinput, crysystem, cryscriptsystem, crygame, cryentitysystem, cryaction)

Roadmap v2 **engine survey**, branch `map/engine-survey-a`. This is a breadth-first classification of
the 1.2 engine modules owned by this task, so the owner can see what remains to map/port for the
Bevy parity goal. It is a survey, not a map: areas are grouped by class/subsystem from the binaries'
own strings, exports/imports, function clusters and x2game call sites; only the top areas carry
function-level leads.

Derived **only** from the shipped 1.2 binaries and their decompiles. The published CRYENGINE source
was not used, quoted or consulted. Already-mapped work is linked, not repeated:

* `docs/research/port-engine-anim/MAP.md` (E2, CryAnimation playback)
* `docs/research/port-engine-char/MAP.md` (E3, CAnimatedCharacter / actor / entity glue)
* `docs/research/port-engine-loop/MAP.md` (E1, frame loop, timer, update order)
* `docs/research/port-*/MAP.md` (x2game maps: movement, mounts, vehicles, flowgraph, combat, housing, …)
* `docs/aa-ui.md` (CryScriptSystem argument conversion / name binding), `docs/aa-world.md`
  (CryEntitySystem parsers)
* CryPhysics is already ported (`aaemu-physics`).

Sources (image bases in parentheses):

| module | source | base |
|---|---|---|
| cryinput | `D:/aa-re/client12/engine2/out/cryinput.dll.{c,funcs.csv,strings.txt,exports.txt,imports.txt}` | `0x33500000` |
| crysystem | `D:/aa-re/client12/engine3/out/crysystem.dll.*` | `0x36500000` |
| cryscriptsystem | `D:/aa-re/client12/engine3/out/cryscriptsystem.dll.*` | `0x35500000` |
| crygame | `D:/aa-re/client12/engine/out/crygame.dll.*` | `0x39000000` |
| cryentitysystem | `D:/aa-re/client12/engine/out/cryentitysystem.dll.*` | `0x32000000` |
| cryaction | `D:/aa-re/client12/engine/out/cryaction.dll.*` | `0x304f0000` |
| x2game hand-over sites | `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.{c,funcs.csv,strings.txt,strings-xrefs.tsv,calls.tsv}` | `0x38ff0000` |

**Method and confidence.** `funcs.csv` gives function extents and sizes; `strings.txt` gives the
binary's own class/profiler/assert/cvar/file names; `.c` was only grepped/parsed, never read whole.
Functions were assigned to an area by their own strings and by their address/call cluster, then the
residual was split by the module's observed layout. Counts are therefore **approximate**: an
unlabelled STL helper is counted with the subsystem that owns the cluster, not individually. Sizes
are code bytes (`funcs.csv` `size`), KB = 1024 B. Tags: **[R]** read from the binary/decompile,
**[I]** inferred from data or an unread branch, **[?]** open.

Parity priorities: **P1** movement/riding, **P2** combat, **P3** interaction/housing,
**P4** vehicles, **P5** world flow/cutscenes/UI/audio.

### Module totals (approximate; see each section for the area breakdown)

| module | analysed fns | KB | to port | already mapped/covered | out of scope | skip |
|---|---:|---:|---:|---:|---:|---:|
| cryinput | 476 | 39.6 | 0 (0.5 optional mouse smoothing) | 10.4 / 95 | 0 | 29.2 / 381 |
| crysystem | 6115 | 972.7 | 0 | 368.3 / 1273 | 65.9 / 134 | 538.5 / 4708 |
| cryscriptsystem | 3091 | 404.4 | 3.9 / 21 (`Movie` bind) | 339.4 / 1737 | 8.3 / 19 | 52.8 / 1314 |
| crygame | 5371 | 1662.4 | 0 | 0 | 0 | 1662.4 / 5371 (unused sample game) |
| cryentitysystem | 4254 | 712.6 | 11.6 / 1 (`CBreakableManager`) | ~531.3 / ~2198 | ~56.7 / ~213 | ~112.8 / ~1842 |
| cryaction | 15075 | 1974.0 | 14.7 / ~33 (`CExactPositioning` + breakable callback) | ~1545 / ~8090 | 0 | ~414 / ~6950 |
| **total** | **34382** | **~5766** | **~30.2 KB / ~55 fns** | **~2794 KB / ~13393 fns** | **~131 KB / ~366 fns** | **~2810 KB / ~20566 fns** |

Only three concrete line-by-line gaps remain in these modules: `CExactPositioning` (7.2 KB),
`CActionGame::OnCollisionLogged_Breakable` (7.5 KB) and `CBreakableManager::BreakIntoPieces`
(11.6 KB), plus the `Movie` Lua bind (3.9 KB) once a cutscene backend exists. Open/unverified
items are called out in the module sections: binary XML, the XML-Lua table loader, the engine
dialog/AI paths (x2game has its own AI) and the remaining `CGameObjectSystem` network binding.

---

## 1. cryinput.dll — 492 functions, 39.6 KB

The 1.2 client does use it: crysystem's `CSystem::InitInput` loads `CreateInput` (cryinput export) and
stores `IInput` at `CSystem+0x38` / `gEnv+0x38` [R, E1 §1.4]. x2game wraps it in its own
`X2Input` / `@@InputManager` (`X2::GameClient::InputManager::Update` `0x39406540`) and the movement
port samples it through the action map [R: x2game strings/xrefs]. All keys/mouse reach the game
through this module; there is no XInput implementation in the shipped DLL (the `i_xinput` cvars are
registered but there are no XInput/DirectInput imports or device bodies) [R].

| area | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| CBaseInput core: event queue, `Update`, `PostInputEvent`, `ClearKeyState`, pressed/released state, action map, `InputDebug` cvars | 66 | 4.0 | Collects OS events into key/action states; exclusive mode, buffered keys, event posting; action→key bindings | yes — every frame via `IInput` | P1 (and P5 UI) | **covered**: Bevy/winit input + `aa-client/src/port/movement/input.rs`; keep action-map semantics if Lua key bindings need them (`aa-ui` keyboard) |
| STL containers/allocator (map/set/list/vector) | 45 | 4.5 | Static MSVC STL instantiations | infrastructure | none | skip (Rust std) |
| Keyboard scan-code → name table (`escape`, `np_divide`, … one 3732 B builder) | 1 | 3.6 | Builds the DIK/scan-code→key-name map used by bindings/config | yes, at input init | P1/P5 | **covered** by `aa-ui/src/lua/keyboard.rs`; port the exact name table only if a Lua script expects a missing name |
| Windows message input (`WM_KEY*`, `WM_CHAR`, IME/`ToUnicodeEx`, `MapVirtualKeyExA`) | 17 | 0.8 | Translates window messages to key/char events, keyboard layout handling | yes (Windows client) | P5 UI text | **covered** by winit/`aa-ui` keyboard + IME path |
| Mouse input: axes, wheel, cursor warp/clip, `i_mouse_*` smoothing/acceleration/inertia | 11 | 2.0 | Mouse delta sampling, buffered mouse, smoothing/accel, cursor recentering for camera | yes — camera and UI | P1 (camera feel) | **covered** by winit; port the smoothing/accel formula (0x33504bca, 536 B) only if camera feel is calibrated against the real client |
| XL string/value helpers (format `%.*f`, `%.16g`, type names) | 41 | 3.6 | Statically linked value/string conversion used by cvars | only via cvar plumbing | none | skip (Rust formatting) |
| `CreateInput` / `CBaseInput` interface glue | 9 | 1.2 | Exported factory, vtable setup, memory-info export | once at init | none | **covered**: no engine interface layer in Bevy |
| CVar typed-value/property system (`float`, `int64`, `Vec3_tpl<>`, `CTimeValue`, parse/format) | 202 | 15.9 | Generic typed property/cvar value storage, parsing and serialization | cvar reads by x2game/console | none | skip: cvars become typed Bevy resources/config; do not port the property engine |
| CVar registration (`i_debug`, `i_mouse_*`, `i_bufferedkeys`, `i_forcefeedback`, `i_xinput*`) + console callbacks | 20 | 1.7 | Registers input cvars and their help text | init | none (values only) | skip: re-declare the handful of values the game reads |
| CRT/startup/SEH/unwind | 64 | 2.3 | MSVC runtime | — | none | skip |

**Totals cryinput:** 39.6 KB / 476 analysed bodies (492 `funcs.csv` rows; 16 are thunks not in the
decompile). **To port line by line: 0 KB** (only optional mouse smoothing, 0.5 KB, if camera feel
needs it). **Already covered** (Bevy/winit + `aa-client` movement + `aa-ui` keyboard): 10.4 KB /
95 fns. **Rendering/out of scope: 0**. **Skip** (STL/CRT/property/cvar plumbing/interface glue):
29.2 KB / 381 fns.

---

## 2. crysystem.dll — 6166 functions, 972.7 KB

The engine host: it creates `gEnv`, loads every other engine DLL, owns the console/log/timer/stream
engine/XML and the frame update that calls back into the game. The 1.2 client uses it from the first
instruction: x2game's `CreateGame`/`CGame::Init` receives the `ISystem` created by
`CreateSystemInterface`, and `CCryAction::PreUpdate` calls `ISystem vtable+0x1c` every frame
(`CSystem::Update`, mapped in E1 §1.4) [R]. Subsystem objects are stored in `CSystem` and reached
through `gEnv` slots (renderer `+0x34`, input `+0x38`, console `+0x44`, script systems `+0x48/4c/50`,
3D engine `+0x54`, sound `+0x58`, music `+0x5c`, physics `+0x60`, movie `+0x64`, AI `+0x68`, entity
system `+0x6c`, log `+0x78`, character manager `+0x7c`, stream engine `+0x8ec`, timer `+0x118`) [R,
E1 §1.4].

The binary's own layout is unusually clear (zlib → pak → pack → PDCurses → hardware detect →
console → log → CSystem → console → streaming → XML → profiler → stats → Seamless/World), so areas
below are address ranges confirmed by the profiler/log strings listed in each row. The three big
non-obvious blocks are the 24.7 KB software 3D stat-graph renderer at `0x3653a830`, the 13.5 KB
DirectDraw/GPU capability probe at `0x3655baf0`, and the XML parser/serializer at
`0x3658a0c0`–`0x365b5a10`.

| area | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| zlib inflate/deflate | 93 | 32.8 | Static zlib used by pak and CGF streams | yes (pak reads) | P5 asset IO | **covered** by `aa-pak`/`flate2` |
| CryPak mount/search (`_pak`, `/game`, `/USER`, `/addon`, mount pak/dir) | 179 | 33.1 | Virtual file system, mount points, file lookup | yes | P5 asset IO | **covered** by `aa-pak` |
| Pack/zip reader (`Pack::Open`, `Pack::GetFP`, signatures) | 115 | 28.5 | game_pak/zip directory and file access | yes | P5 asset IO | **covered** by `aa-pak` |
| PDCurses console library (`initscr`, `pdcurses`) | 51 | 8.8 | Curses UI for dedicated/editor console | no — Windows client uses `CXConsole` | none | skip (unused client path) |
| GPU/spec autodetect (`config/gpu/*.txt`, `D3D11CreateDevice`, `MaxConfigSpec`) | 54 | 12.4 | Probes GPU/memory and picks sys_spec | init only | none | skip (Bevy/wgpu chooses adapter; no spec tiers) |
| AVI/movie errors + streaming budget (`AVIERR_*`, `Budget`) | 30 | 7.3 | Movie-file error strings and disk/stream budget bookkeeping | movie system init; budgets via streaming | P5 cutscenes | skip: playback belongs to crymovie (not implemented yet); budget covered by `aa-world` streaming |
| Console batch/config + CPU detection (`executes a batch file`, `game/config/`, AMD/Intel CPUID) | 59 | 15.3 | `exec` of .cfg files at startup, CPU feature detection | startup | none | skip (config becomes Bevy resources; CPUID irrelevant) |
| Memory statistics (`Memory Statistics`, heap walkers) | 75 | 12.6 | Memory accounting/reporting | debug/profiler | none | skip (profiler) |
| Typed value/property system (`float`, `int64`, `Vec3_tpl<>`, `CTimeValue`) | 252 | 28.0 | Generic typed cvar/Data value parse/format/serialize | via cvars and XML Data | none | skip: typed config/resources replace it |
| Registry RootPath / SettingsMgr dialog | 20 | 7.8 | Windows registry root path, editor settings launcher | editor/launcher only | none | skip |
| Excel XML export + DxDiag probe | 70 | 14.9 | Stats workbook writer and DirectX diagnostics | tools/stats | none | skip |
| CLog (`CLog::LogV/Update/LogStringToFile`, log cvars) | 52 | 13.8 | Engine log, console mirror, log rotation | yes, but no gameplay effect | none | skip (`tracing`/Rust logging) |
| STL helpers around the graph renderer | 24 | 2.5 | MSVC STL | infrastructure | none | skip |
| Software 3D stat-graph renderer (`0x3653a830`, 24.7 KB, sin/cos + renderer slot) | 20 | 30.7 | Draws the 3D frame-time/stat graph in the world | only with stats/profiler enabled | none | **out of scope**: rendering/debug overlay (Nick) |
| Screenshot save (.dds/.tif), editor/rc.exe launch, server throttle | 43 | 7.7 | Screenshot writer, material-editor/rc tool launcher, dedicated-server FPS throttle | screenshots/tools; throttle server-only | none | skip (tooling/server) |
| CSystem misc: `SleepIfInactive`, physics-thread setup, `SystemCameraProxy`, `LoadDLL`, devmode | 19 | 4.4 | Startup/shutdown helpers and DLL loading | yes (startup) | P5 | **covered** by `aa-client` startup (no DLL layer) |
| `CSystem::Update` | 1 | 4.4 | Engine frame: input, console, sound, entity pre/post, physics, network, stream | yes, every frame | P1/P5 | **already mapped** — E1 §1.4 |
| TimeProfiler / memory-fragmentation profiler / missing-files report | 150 | 27.6 | Profiling and diagnostics | debug | none | skip (profiler) |
| CSystem init/config/spec/submodule init (`InitNetwork`, `InitInput`, `InitEntitySystem`, `InitSound`, `InitPhysics`, `InitMovieSystem`, `InitAISystem`, `InitStreamEngine`, `Init3DEngine`, `InitAnimationSystem`, `InitRenderer`, `InitScriptSystem`, `InitFont`, sys_spec) | 169 | 51.2 | Loads all engine DLLs, builds `gEnv`, reads config | yes (startup) | P5 | **covered** by `aa-client`/Bevy plugin startup; init order already known from E1 |
| DirectDraw/GPU capability probe (13.5 KB `ddraw.dll` block) | 2 | 13.5 | Legacy DX9/DirectDraw adapter enumeration | init only | none | skip (obsolete API) |
| CSystem render/stats/mem/debug/crash (`RenderBegin/Render/RenderEnd/RenderStatistics`, resolution modes, callstack, affinity, crash dump, `CollectMemStats`) | 114 | 35.2 | Frame render hooks, stat overlays, resolution switching, crash/mem diagnostics | render/stats; no gameplay | none | **out of scope** (rendering/debug; Nick); crash handler skip |
| CThreadTaskManager / IOCP worker pools | 89 | 20.9 | Thread pools for streaming/physics/IO tasks | yes (async work) | P5 | **covered** by Bevy tasks + `aa-world` streaming |
| `CTimer` | 43 | 6.2 | Frame time, async time, smoothing, pause/serialize | yes, every frame | P1 | **already mapped** — E1 §1.3 |
| CUNIXConsole | 117 | 23.9 | Curses console for dedicated server | no (client) | none | skip (unused client path) |
| CXConsole / help / keybindings / CVarGroups (`sys_spec`, `con_*`, `~`) | 165 | 48.0 | In-game console UI, command help, key bindings, spec groups | debug console; settings via CVarGroups | P5 (debug only) | skip: dev console not parity-critical; graphics settings become Bevy config |
| CMTSafeHeap + CRefStreamEngine/Proxy | 133 | 26.4 | Thread-safe heap and async read/job streaming | yes (streaming) | P5 | **covered** by `aa-world`/`aa-pak` streaming |
| XML/Data/savegame serializer + text/binary XML parser (`XML Error`, `Binary XML`, `XMLBinary`) | 503 | 79.8 | Parses level/entity/prefab XML, serializes savegames | yes (level/entity/prefab load) | P5 | **covered** by `aa-world`/`aa-cry` (`roxmltree`) for text XML; binary-XML path unused/unverified — skip until evidence |
| XML DTD parser (`DOCTYPE`, `ELEMENT`, `ATTLIST`, `PCDATA`) | 200 | 48.6 | Validating XML DTD support | no game-data evidence | none | skip (engine feature, not used by shipped data) |
| XML-Lua table loader (`XML-Lua data file`, `Property has no name`, `Array has no name`) | 84 | 25.6 | Builds Lua tables from XML definition/data pairs | no x2game evidence; engine data tooling [?] | P5 UI (if used) | skip for now; map only if a shipped UI/data file is found using it |
| CFrameProfileSystem / sampling / loading profiler (`frameprofile%02d.dat`) | 238 | 64.4 | Frame profiler, callstack sampling, loading profiler | debug | none | skip (profiler) |
| Localization (`@ui_day`, `Localized string`, Excel worksheet import) | 69 | 18.3 | `@`-token localization and Excel/localization-file import | yes (UI text) | P5 UI | **covered** by `aa-ui`/`aa-client` DB localization (`ui_texts`/`localized_texts`) |
| Stats/Excel/CEngineStats collectors | 73 | 59.5 | Engine asset/memory statistics export | tools | none | skip |
| Stats residual + STL | 353 | 70.7 | Remaining stat templates/containers | debug | none | skip |
| SeamlessSystem + WorldSystem (`UpdateSeamless*`, `LoadZone`, `LoadWorldList`, `Load`) | 240 | 62.3 | Cell/zone streaming, origin changes, time-of-day/env update | yes (world flow) | P5 | **covered** by `aa-world` (streaming, world.xml, ToD) |
| STL/CRT/startup | 2216 | 25.0 | MSVC STL/runtime | — | none | skip |

**Totals crysystem:** 972.7 KB / 6115 analysed bodies (6166 `funcs.csv` rows incl. thunks).
**To port line by line: 0 KB** — the runtime-critical parts are either already mapped (E1) or owned by
an existing crate. **Already covered/mapped:** 368.3 KB / 1273 fns — `aa-pak` 94.4, `aa-client`
startup 55.6, E1 (Update+Timer) 10.6, Bevy tasks 20.9, `aa-world` streaming/Seamless/XML 168.5,
`aa-ui` localization 18.3. **Out of scope (rendering/debug):** 65.9 KB / 134 fns. **Skip:** 538.5 KB
/ 4708 fns (profiler/stats/console/PDCurses/registry/DTD/XML-Lua/tooling/STL).

---

## 3. cryscriptsystem.dll — 3119 functions, 404.4 KB

The Lua 5.1 runtime plus the CryEngine script-bind tables. The 1.2 client uses it for the whole UI
and much of the game glue: `CSystem::InitScriptSystem` creates **three** script systems
(`CSystem+0x48/0x4c/0x50`, `gEnv` script slots) and x2game loads the shipped
`scriptsbin/*.alb`/`toc.g` through them [R: E1 §1.4, x2game `CreateScriptSystem`/`scriptsbin`
strings]. `aa-lua` already provides the 5.1 state and `Script`/`Console`/`Movie` globals; the
argument-conversion contract is documented in `docs/aa-ui.md` ("CryScriptSystem"), and
`aa-fx` already parses/simulates `ParticleParams`.

| area | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| Typed value/property conversion (`float`, `int64`, `TFixed<>`, vectors) | 279 | 33.0 | Shared cvar/ScriptAny value parse/format/serialize | indirectly (script args, cvars) | none | skip: Rust/`mlua` typed conversion replaces it |
| ScriptAny + parameter conversion (`Any`, `Null`, `Boolean`, `Pointer`, "Wrong parameter type") | 5 | 0.4 | The exact Lua→C++ argument admission rules | yes | P5 UI | **already documented/ported** — `docs/aa-ui.md` CryScriptSystem section; `aa-lua` |
| Error reporting / callstack / `BeginCall` (`[Lua Error]`, `[CScriptSystem::BeginCall]`) | 59 | 6.9 | Lua error capture, stack dump, table/function lookup | yes (every script call) | P5 UI | **covered** by `aa-lua` |
| `CScriptSystem` core: `Update`, `ForceGarbageCollection`, `_time/_frametime/_aitick`, script loading, timers | 24 | 5.0 | Per-frame Lua tick, GC, script file load, time globals | yes, every frame | P5 UI | **covered** by `aa-lua` |
| Script dump / callstack / profiler (`lua_dump_state`, `lua_count`, TimeProfiler) | 55 | 9.6 | Debug dumps of Lua tables/stacks and script profiling | debug only | none | skip (debug tooling) |
| Script-table references / deleted-object guards + `ScriptTimers` | 96 | 14.8 | Ref-counted script objects, `__index` guards, Lua timers | yes (UI/game scripts) | P5 UI | **covered** by `aa-lua`/`aa-ui` stubs |
| `Movie` bind (`PlaySequence`, `StopSequence`) | 21 | 3.9 | Lua control of movie/cutscene sequences | login/cutscene path; movie system not implemented yet | P5 | **to port** once a movie/cutscene backend exists; `aa-lua` currently stubs `Movie` |
| `Particle` bind + `ParticleParams` (`CreateEffect`, `SpawnEffect`, decal params) | 370 | 81.6 | Lua particle spawn/params and the full particle parameter schema | yes (effects, UI) | P5 | **covered** by `aa-fx` (params/sim) + `aa-lua` `Particle` table; do not port the parser |
| `Physics` bind (`RayTraceCheck`, `#ForceBreakable`) | 10 | 7.3 | Lua raycast and breakable registration | yes (scripts) | P1/P2 | **covered** by `aaemu-physics` + `aa-lua` binding |
| `Script` bind (`ReloadScripts`, load/unload) | 30 | 3.7 | Script lifecycle from Lua | yes | P5 UI | **covered** by `aa-lua` |
| `Sound` bind (music/sound volume, loops, EAX, music moods) | 110 | 31.3 | Full Lua audio API | yes (UI/game audio) | P5 audio | **covered** by `aa-audio` + `aa-lua` |
| `System` bind: time, cvars, console, raycast, display, terrain/fog/wind, log, font, draw | 131 | 51.1 | The `System.*` engine API surface | yes (UI and scripts) | P5 UI | **mostly covered** by `aa-lua`/`aa-client`/`aa-world`/`aa-cry`/`aa-ui`; see draw subset below |
| — of which draw/postprocess/font/light (`DrawText`, `DrawLine`, `DrawImage*`, `SetScreenShader`, `SetPostProcessFxParam`, `LoadFont`, `ActivateLight`, `SetScissor`) | 19 | 8.3 | Immediate-mode 2D/3D drawing and post-process from Lua | UI-era draw calls | P5 (rendering) | **out of scope** — rendering (Nick); `aa-ui` draws widgets natively |
| `System` table init / `SCANDIR` / remaining bind registration | 43 | 13.0 | Builds the `System` Lua table, directory enumeration | yes (startup) | P5 UI | **covered** by `aa-lua`/`aa-client` |
| `SurfaceType` bind (`Materials`, footstep/impact params) | 29 | 5.1 | Surface-type script definitions used by physics/audio | yes (data load) | P3/P5 | **covered** by `aa-world`/`aa-cry` surface types |
| Lua 5.1 VM / compiler / auxlib / stdlib (`Lua 5.1`, `luaopen_%s`, `LUA_PATH`) | 849 | 127.5 | Stock Lua 5.1 interpreter and libraries | yes | P5 UI | **covered** by `aa-lua` (`mlua` Lua 5.1) |
| STL/CRT | 980 | 10.2 | MSVC STL/runtime | — | none | skip |

**Totals cryscriptsystem:** 404.4 KB / 3091 analysed bodies (3119 `funcs.csv` rows).
**To port line by line:** 3.9 KB / 21 fns (`Movie` bind, only after a cutscene backend exists).
**Already covered/mapped:** 339.4 KB / 1737 fns — `aa-lua` VM/error/core/tables 127.5+5.0+6.9+14.8+
0.4, `aa-fx` particles 81.6, `aa-audio` sound 31.3, `aaemu-physics` 7.3, `aa-world`/`aa-cry`
surface types 5.1, `aa-client`/`aa-lua` `System`/script/table init 59.5. **Out of scope
(rendering):** 8.3 KB / 19 fns. **Skip:** 52.8 KB / 1314 fns (typed-value plumbing, script
dump/profiler, STL/CRT).

---

## 4. crygame.dll — 5390 functions, 1662.4 KB

**Not used by the 1.2 client.** `crygame.dll` is the CryEngine 3 *sample game* (Crysis-derived)
shipped alongside the engine, not ArcheAge's game DLL. Evidence:

* The game DLL is selected by `sys_dll_game`, whose shipped default is `x2game-dev.dll`; the retail
  client loads `x2game.dll`. `crysystem`'s `CSystem::Init`/`LoadDLL` loads that name, and x2game
  itself exports the game-DLL contract `CreateGame`/`CreateGameStartup` [R: crysystem strings
  `sys_dll_game`/`x2game-dev.dll`, x2game exports].
* No x2game or cryaction string/import references `crygame.dll`; x2game loads `cryaction.dll` and
  registers `"Game"` through it (E1 §1.1) [R].
* The binary's own content is Crysis single-player: `nanosuit_*` materials, `parachute_opening.c`,
  `alien_special.h`, `diff_easy.c`/`diff_hard.c`/`diff_bauer.c`, `act1.xml`, `mp_objectives.xml`,
  `RichSaveGames`, and MSVCR80 (the 1.2 client modules use MSVCR100) [R: strings/imports].
* `CreateEditorGame` is the editor/sample entry point; no client call site exists [R].

It is therefore classified for completeness only. The gameplay features it implements (player,
items, weapons, vehicles, HUD, game rules, tutorial, flow nodes) are **not** the ones the 1.2 client
runs — those live in x2game (`port-*` maps). **Do not port anything from this DLL.**

| area (Crysis sample game) | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| Framework/startup/serialization/savegames/XML/zip (`CGameStartup`, `CGame`, `CLMSerializationManager`, `XMLBinary`, `ZipFile`, `RichSaveGames`, `C3DSamplerOctree`) | 657 | 200.6 | Sample-game bootstrap, LM save serialization, binary XML, save-game containers | no | none | skip (unused sample) |
| Player/input/actor/inventory (`CPlayer`, `CPlayerInput`, `CItem`, `SharedString`) | 316 | 214.6 | Crysis player, nanosuit input, item/inventory model | no | none | skip |
| Tutorial / script bind game / flow (`CMPTutorial`, `CScriptBind_Game`, flow base nodes) | 416 | 106.3 | Sample tutorial logic, `Game.*` Lua bind, flow nodes | no | none | skip |
| Vehicle movement (`CVehicleMovementStdWheeled`, `StdBoat`, `Tank`, `Helicopter`, `VTOL`, `Hovercraft`, damage behaviors) | 343 | 161.1 | Crysis vehicle physics/controllers | no (ArcheAge vehicles are x2game, `port-vehicles`) | none | skip |
| Items/weapons/projectiles (`CWeapon`, `CProjectile`, `CBullet`, `CHomingMissile`, `CRocketLauncher`, `CLam`, `CGunTurret`, `COffHand`) | 918 | 261.3 | Sample weapons, projectiles, offhand, turrets | no | none | skip |
| More weapons/projectiles/offhand/turrets (`CWeapon`, `CScriptBind_Weapon`, `CGunTurret`, `COffHand`, `CHeal`, `CPlant`, `CRapid`) | 580 | 216.9 | Continuation of the same sample combat code | no | none | skip |
| HUD/radar (`CHUD`, `CHUDRadar`, `CHUDTweakMenu`, `CFlashMenuObject`) | 691 | 229.4 | Crysis HUD, radar, tweak menu, Flash menu | no | none | skip |
| Game rules (`CGameRules`, `CSingle`, `CWorkOnTarget`) | 411 | 84.1 | Single-player rules, near-miss/work-on-target logic | no | none | skip |
| Flash menu/UI + residual/STL | 1039 | 187.8 | Scaleform/Flash menus and template/container residue | no | none | skip |

**Totals crygame:** 1662.4 KB / 5371 analysed bodies (5390 `funcs.csv` rows).
**To port: 0.** **Already covered: 0.** **Out of scope: 0.** **Skip (unused sample game): 1662.4 KB /
5371 fns.**

---

## 5. cryentitysystem.dll — 4282 functions, 712.6 KB

The entity/transform/streaming-object layer. Used every frame: `CSystem::InitEntitySystem` stores
`IEntitySystem` at `CSystem+0x6c`/`gEnv`, `CSystem::Update` calls its `PrePhysicsUpdate` and `Update`
slots, and x2game's player/actor/doodad proxies are registered on `CEntity` objects (E1 §1.4,
E3 §1.1) [R]. Do **not** re-map the transform graph, event dispatch or proxy plumbing here: E3
covers `CEntity::PrePhysicsUpdate`/`Update`, `CEntitySystem` iteration and the
`CPhysicalProxy`/`CRenderProxy` hand-over, and `docs/aa-world.md` covers the entity/cell parsers and
the fog-volume/visarea formats. This table classifies what E3/`aa-world` did **not** cover.

| area | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| `CEntity` core: transform (`SetPosRotScale`, `InvalidateTM`), slots, flags, event dispatch | 368 | 54.4 | Entity transform/attachment graph and per-entity events | yes, every frame | P1/P3 | **already mapped/covered** — E3 §1–§2, `aa-world` entity hierarchy |
| `CEntitySystem` core: `LoadEntities`/`LoadEntity`, iteration, pre/post-physics, timers | 248 | 58.9 | Entity registry, cell/level loading, frame dispatch | yes, every frame | P1/P5 | **already mapped/covered** — E1 §1.4, E3 §1.1, `aa-world` cell loading |
| `CScriptBind_Entity` + entity script proxies (`SetName`, `GetAIName`, `GetPos/GetAngles`, `IsValid`, spawn/link) | 531 | 178.5 | The Lua `Entity` table and script proxy callbacks | yes (level/UI scripts) | P3/P5 | **covered/stubbed** by `aa-lua` + `aa-client` for the methods ArcheAge UI uses (`GetEntityByName`, `GetPos`, `GetAngles`, `IsValid`); map only further call sites, do not port the whole table |
| Entity transform/physics/procedural helpers (unlabelled math cluster) | 393 | 58.3 | Unnamed entity math, proxy bookkeeping, containers | indirectly | P1/P3 | **covered** by E3/`aa-world`; residual STL skip |
| `CScriptProxy` / `CPhysicalProxy` | 257 | 61.5 | Script callbacks and physics proxy (entity ↔ physical entity) | yes | P1/P3 | **covered** — E3 §2.2 (`CPhysicalProxy`), `aa-lua` script proxy |
| `CRenderProxy` / `CEntityObject` / `CEntityLoader` | 413 | 113.7 | Render proxy, static object instances, prefab/entity loader | yes (static props/prefabs) | P3/P5 | **mixed**: `CEntityObject`/`CEntityLoader` (~57 KB) covered by `aa-world` static props/prefabs; `CRenderProxy` (~57 KB) is **out of scope** (rendering, Nick) |
| `CAreaManager` / `CAreaGrid` / `GridCompileThread` | 199 | 60.9 | Visarea/portal/occlusion grid and async compile | yes at level load | P5 | **covered** by `aa-world` visarea data (`visareas.dat`); portal/occlusion *traversal* remains rendering (Nick) |
| `CFlock` / boids (`BirdBoid`, `FishBoid`, `e_flocks`) | 168 | 62.8 | Ambient bird/fish flock simulation | no x2game evidence | none | skip (engine ambient feature; no client call site) |
| `CBreakableManager` (11.6 KB) + `CPartitionGrid`/`CProximityTriggerSystem` (1.8 KB) + STL instantiations (44.0 KB) | 1114 | 57.4 | Breakable-object pieces, spatial partition, proximity triggers, mostly STL templates | breakable events exist in x2game (`OnBreakableCreatePartEvent`), partition used by streaming | P2/P3 | `CBreakableManager` **to port** if breakable doodads are in scope (11.6 KB, 1 body); partition/proximity **covered** by `aa-world`; STL skip |
| STL/CRT | 563 | 6.0 | MSVC STL/runtime | — | none | skip |

**Totals cryentitysystem:** 712.6 KB / 4254 analysed bodies (4282 `funcs.csv` rows).
**To port line by line:** 11.6 KB / 1 body (`CBreakableManager::BreakIntoPieces` path), only if
breakable doodads are implemented. **Already covered/mapped:** ~531.3 KB / ~2198 fns — E3 transform/
proxy/event core, `aa-world` entity/cell/prefab/visarea/partition data, `aa-lua`/`aa-client` entity
script methods. **Out of scope (rendering):** ~56.7 KB / ~213 fns (`CRenderProxy`). **Skip:** ~112.8 KB
/ ~1842 fns (CFlock/boids, STL/CRT and template instantiations).

---

## 6. cryaction.dll — 15108 functions, 1974.0 KB

The game-framework/action layer between x2game and the engine. x2game loads it with
`LoadLibraryA("cryaction.dll")`, calls `CreateGameFramework`, then `framework->Init` and the
per-frame `PreUpdate`/`PostUpdate` slots; it registers the game as `"Game"` through the framework
(E1 §1.1) [R]. Most of its runtime-critical code is **already mapped**:

* `CCryAction` frame/init (`Init`, `CompleteInit`, `PreUpdate`, `PostUpdate`, `InitScriptBinds`,
  `CheckEndLevelSchedule`) — E1 §1.1–§1.2.
* `CAnimatedCharacter` and the `CGameObject` extension/event path — E3 §1.2, §3.
* `CAnimationGraph`/`CAnimationGraphState`/`CAnimationGraphManager` — E2 and
  `port-mount-anim/MAP.md` (graph inputs, guards, transitions, command buffer).
* Flow graph runtime and all node classes — `port-flowgraph/MAP.md` (1,529 graphs / 10,295 nodes;
  engine + X2/AA node inventory).

This table therefore only classifies what those maps do not own. The 0x3054–0x305e flow-graph
cluster and the 0x305f–0x3065 animation-graph cluster are counted as **already mapped** even though
they are the largest blocks.

| area | fns | KB | what it does | used at runtime | parity | approach |
|---|---:|---:|---|---|---|---|
| Startup/thunks | 1 | 0.1 | CRT/import glue | — | none | skip |
| `CLevelSystem` / `CActorSystem` / `CAIHandler` / `CAIProxy` / readability | 594 | 115.3 | Level/actor registries, AI handler and proxy, readability manager | actor/level used; **AI handler not used by x2game** (x2game has its own `<CAIHandler>`/`<CScriptBind_AI>` implementations) [R: x2game strings/xrefs] | P3 (actor/level) | **covered/mapped** by E3 actor glue and `aa-client`; AI part skip (server-authoritative; cryaisystem survey owns AI) |
| `CScriptBind_AI` (146 KB of `AI.*` Lua methods) | 281 | 146.2 | Engine AI Lua API | no cryaction call site in x2game (own AI bind) | none | skip |
| FlowGraph runtime + nodes (`CFlowSystem`, `CFlowGraphBase`, `CFlowNode_*`, `CFlowDelayNode`, `CFlowNode_AIBase`, X2/AA nodes) | 3582 | 575.8 | Graph evaluation, node activation, quest/cutscene/doodad graphs | yes (quests, cutscenes, doodads) | P5/P3 | **already mapped** — `port-flowgraph/MAP.md` (all node classes/counts) |
| `CGameObject` / `CGameObjectSystem` / `CGameObjectDispatch` / `CInteractor` | 299 | 59.0 | Game-object extensions, event forwarding, network binding, scheduling | yes (entity/gameobject glue) | P3/P5 | **covered/mapped** — E3 §1.2 event forwarding; remaining binding follows `aa-net`/`aa-client` gameobject layer |
| `CAnimationGraph` / `CAnimationGraphManager` | 1171 | 238.3 | Animation graph construction, state cache, LMG/command buffer | yes (all characters) | P1/P2 | **already mapped** — E2 + `port-mount-anim/MAP.md` |
| `CAnimationGraphState` / `CAnimationGraphStates` + `CExactPositioning` | 411 | 125.1 | Graph state queries and exact-positioning alignment (`CalculateTarget`, `RecalculateTriggering`) | graph state yes; exact positioning for sit/use animations | P1/P3 | **117.9 KB mapped** (E2/port-mount-anim); **CExactPositioning 7.2 KB / 3 bodies to map+port** (P3 chairs/doodad use) |
| Animation state + `CAnimatedCharacter` (graph state input, PostInit, PrePhysics/Update) | 349 | 119.4 | Character animation state and animated-character frame | yes, every character | P1 | **already mapped** — E2, E3 §3 |
| `CAnimatedCharacter` + `CMaterialEffects`/`CMFX*` (impact/footstep/decal/particle/flowgraph/sound effects) | 621 | 120.3 | Collision-driven material effects and MFX effect classes | yes (combat/footsteps) | P2/P5 | **covered/mapped** — E3 for the character; `aa-client` combat (footsteps/impacts) + `aa-fx` for the effects |
| `GameTokenSystemNew` / `CEffectSystem` / `CPersistantDebug` / `CDialogSystem`/`CDialogSession`/`CDialogScript` | 684 | 119.7 | Game tokens, ground/effect system, persistent debug, engine dialog sessions | tokens/effects used by flow graphs; **engine dialog unused by AA UI** | P3/P5 | **partially covered**: tokens/effects via `aa-ui`/`aa-client` where a script uses them; engine dialog skip (AA dialogs are Lua UI + `aa-net`); debug skip |
| `CGameContext` / `CGameQueryListener` / `CGameStats` / `MetadataRecorder` / `CDebrisMgr` | 577 | 121.6 | Server browser/query, game context/serialization, player stats, debris manager | query/server list and context used | P5/P2 | **covered/mapped** — `aa-net`/`aa-client` (server select/session), `aa-fx` for debris where used |
| `CActionGame` / `CCryAction` / `CMusicLogic` / `CCallbackTimer` / `CDevMode` | 1202 | 178.1 | Action-game update, collision callbacks (material FX, breakable, AI notify), framework init/loop, procedural music, timers, dev camera | framework loop yes; collision FX yes | P1/P2/P5 | **mixed**: E1 maps `CCryAction` loop/init (~26 KB); `CActionGame` material-FX collision path (~32 KB) covered by `aa-client` combat/`aa-fx`; **`OnCollisionLogged_Breakable` 7.5 KB to port if breakable doodads are in scope**; `CMusicLogic` 2.1 KB skip (AA uses `x2music` packs); remaining ActionGame serialization/STL skip (~111 KB) |
| Thunks/obfuscated tail (0x30700000–0x30720000) | 5265 | 54.2 | Import/exception thunks and tiny stubs | — | none | skip |
| Thunks/CRT tail | 38 | 0.8 | CRT | — | none | skip |

**Totals cryaction:** 1974.0 KB / 15075 analysed bodies (15108 `funcs.csv` rows).
**To port line by line:** 14.7 KB / ~33 fns — `CExactPositioning` 7.2 KB (P3 sit/use alignment) and
`CActionGame::OnCollisionLogged_Breakable` 7.5 KB (P3 breakable doodads), both only if those
features are in scope. **Already mapped/covered:** ~1545 KB / ~8090 fns — E1 (`CCryAction`),
E2/`port-mount-anim` (animation graph/state), E3 (animated character/gameobject/actor),
`port-flowgraph` (runtime/nodes), `aa-client` combat/material FX + `aa-fx`, `aa-net`/`aa-client`
game context/server list. **Out of scope (rendering): 0.** **Skip:** ~414 KB / ~6950 fns —
`CScriptBind_AI`/`CAIHandler` (x2game has its own AI), engine dialog, `CMusicLogic`, debug/tokens
residual, ActionGame serialization/STL and thunks.
