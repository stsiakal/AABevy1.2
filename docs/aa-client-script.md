# aa-client: headless runs and the script runner

Automated client tests against a server: `--headless` runs the client without a window, `--script file.txt` drives it
from a line-based script. Code: `crates/aa-dev/src/{headless,capture}.rs`, `crates/aa-client/src/script/`,
`crates/aa-client/src/hud/script_api.rs`.

```text
aa-client.exe --headless --account NAME --login 127.0.0.1:32237 --script run.txt [--password-file P] [--character C]
echo $?          # 0 = PASS, 1 = a command, expect or problem failed, 2 = the script does not parse
```

`--script` implies `--enter-world` and skips the login screen: the client logs in with `--account` / `--login` /
`--password-file`, enters the first character (or `--character`), and the script starts two seconds after the HUD is up.

## Headless (`aa-dev`)

| Flag | What it does |
|---|---|
| `--headless` | No OS window (winit is off), no GPU (`RenderPlugin` with `backends: None`), a timed schedule loop. The primary `Window` stays a component, because the UI takes its size from it. aa-net, game state, the Lua UI, the HUD systems and the movement run as in a windowed client. |
| `--headless-fps N` | Loop rate (default 30). |
| `--offscreen-shot out.png` | Headless **with** the GPU: every camera that would draw into the window draws into an `Image` of the window size; the image is saved after `--shot-after` s (default 5). Without `--script` the process then exits; with a script, `shot <file>` takes more. |
| `--allow-lua-error TEXT` | Repeatable. A Lua error whose message contains TEXT is not a failure (known harmless ones, see below). |

`--shot` (a window screenshot) is not available with `--headless`.

## Failures (exit code 1)

The run fails on: a failing command or a timeout, a failing `expect` / `waitfor`, a Lua runtime error logged by the UI
scripts (a WARN/ERROR record that holds `runtime error`, `stack traceback`, `scriptsbin/` or `attempt to`; see
`aa_dev::capture`), `Event::GameDecodeFailed`, an `Event::UnhandledGamePacket` of level 4 (a compressed DD04 batch
lost its tail), a lost connection, a panic (`SCRIPT RESULT: FAIL panic: ...`, the process also exits non-zero), no
world within `--script-enter-timeout` (180 s) or the whole script over `--script-timeout` (900 s). The first failing
command stops the script. The last lines are a summary:

```text
SCRIPT SUMMARY run.txt
  commands:   24 of 24 done (95 s wall)
  events:     5321 received, 0 server errors {}
  log:        12 warnings, 0 errors, 1 allowed Lua errors
  player:     (15601.2, 15380.0, 126.3), moved 51.4 m since the start
SCRIPT RESULT: PASS
```

## Commands

One per line. A `#` at the start of a line, or one standing alone after a space, starts a comment (`#t`, Lua's length operator, is kept). The names follow `aaemu-testclient`'s `scenario/script.rs`
(`target`, `cast`, `interact`, `loot`, `await`, `tryawait`, `gm`, `tp`, `chat`, `bag`, `units`, `me`). An unknown
command or a bad argument fails when the file is read, with the line number, before anything runs.

A *unit spec* names a unit: `nearest` (alive, not you), `hostile`, `friendly` (alive NPC that is not hostile),
`corpse`, `self`, `current` (the target), `id:N`, `template:N` or a bare number (an NPC template), `name:TEXT`.
Distance is from the player; `hostile` / `friendly` use the nameplate relation.

| Command | Does |
|---|---|
| `wait S` (`pump`) | wait S seconds |
| `walk M` | walk M metres the way the character faces |
| `walk X Y` | walk to a game position (W key held, camera turned to the bearing; fails when stuck 8 s or late) |
| `walk to SPEC` | walk to 3 m from a unit |
| `face DEG` | heading in degrees (0 north, counter-clockwise) |
| `orbit DEG` | turn the camera alone around the player, DEG from the character's heading (90 or -90: a side view), for a shot |
| `target SPEC` | `CSChangeTarget`, waits for the server's `TargetChanged` |
| `click SPEC` | the left click: the scripted pick goes through `hud::targeting::click_target` |
| `cast SKILL [SPEC]` | `CSStartSkill` on the spec (default: current target, else self); waits for `SkillStarted` (a skill with a cast time), `SkillFired` (an instant one) or the first own `PlotEvent` (a plot-only one: Triple Slash, Charge), fails when refused; an `OUTOF_ANGLE` refusal waits once more while the combat code turns the player and asks again; `cast slot N` presses action bar slot N |
| `interact [SPEC]` | the right click on an NPC (default `friendly`): the scripted pick goes through `hud::dialog::talk_input` (reach 6 m, hostile or dead NPCs are refused as for the mouse); waits until the talk began |
| `interact doodad SPEC SKILL` | `CSStartSkill` with a doodad target (the client has no doodad click yet) |
| `plant ITEM DX DY [YAW]` | plant an `item_spawn_doodads` item `DX`/`DY` metres from the player (same use range, ground and footprint checks as the ghost; `YAW` in radians, default the player's facing), then `CSCreateDoodad` and waits for `DoodadsCreated` (alias of aa-net `DoodadsSpawned`) |
| `house_place ITEM DX DY [YAW]` | aim an `item_housings` design item's placement `DX`/`DY` metres from the player (same housing-area / overlap checks as the ghost); the ghost and boundary stay, the first `click left` locks the spot (rotation, `wheel` turns the model), the second asks for the tax quote and opens the original "Start Construction" window |
| `house_build` | confirm the open original build window (the same path as its Build button): `CSCreateHouse` with the quote's total, then waits for `Housing.MyHouse` (`SCMyHouse`) |
| `accept [QUEST]` / `complete [QUEST [REWARD]]` | the dialog's Accept / Complete buttons (default: the first offered / hand-in quest); waits for the server |
| `endtalk` | close the NPC talk |
| `loot [SPEC]` | waits until a corpse with loot is in reach, presses F (loot all), waits for the server's answer |
| `chat TEXT`, `gm CMD`, `tp X Y Z` | say-chat; a GM command (`/CMD`); `/move X Y Z` |
| `open ACTION` | a key binding action (`toggle_bag`, `toggle_character`, ...) |
| `key NAME` (`tap`), `close` | press and release a real key through the original `hotkey.g` bindings (`enter`, `escape`, `tab`, `slotN`, `f`, `g`, `number1`, ...); `close` is `key escape` |
| `keydown NAME` / `keyup NAME` (`press` / `release`) | hold and release a key through the same bindings, for keys the script keeps down across commands |
| `hold NAME S` | press the key and release it after S seconds (`hold w 3` walks forward for 3 s) |
| `type TEXT` | typed text (the chat input), not key bindings |
| `cursor X Y` | the virtual pointer at window pixels, or a fraction of the window when both values are 0..=1 (`cursor 0.5 0.6`); the client's window cursor moves there too, so HUD clicks, placement and the camera ray use it, including `--offscreen-shot` |
| `click left` / `click right` | a real left/right mouse button press and release at the virtual cursor (placement confirm / cancel, HUD, right-click on a house); the OS mouse is untouched |
| `wheel STEPS [alt\|shift\|ctrl]` | a real wheel event with those modifiers held for its frame (placement rotation, camera zoom, chat scroll; `ctrl` leaves it to the camera) |
| `uiclick X Y`, `uirclick X Y`, `uihover X Y` | `--hud-click` at a physical pixel |
| `lua CODE` | run Lua in the UI state (an error fails) |
| `await EVENT [S]`, `tryawait EVENT [S]` | wait for a client event (default 60 s / 10 s); `await` fails on timeout. EVENT is the `aa_net::Event` variant name; nested ones as `Interaction.SkillList`; matched by prefix, case-insensitive. Events that arrive before the `await` stay queued |
| `waitfor [S] COND` / `expect COND` | poll a condition until true (30 s default) / check it now and fail if false; `!COND` negates |
| `shot FILE` | window screenshot, or the offscreen image with `--offscreen-shot`; skipped (noted) in plain `--headless` |
| `mark` | the start point of `moved` |
| `use TEMPLATE` | use the bag item of this template, as the bag window's right click (`Request::UseBag`); fails when the bag has none |
| `units [TEMPLATE...]`, `me`, `bag`, `quests`, `echo TEXT` | print state |
| `quit` / `quit now` | log out and wait for the server to close the connection (45 s) / drop the connection; ends the run |

Conditions: `in_world`, `target`, `target_name TEXT`, `target_template N`, `target_alive`, `target_dead`, `target_hurt`
(health below the first sighting), `alive`, `dead`, `dialog` (the last `interact` started a talk), `quest N` (active),
`quest_done N`, `moved M` (metres since `mark`), `near X Y R`, `item TEMPLATE [COUNT]`, `lootable`, `chat TEXT`,
`event NAME`, `no_errors`, `lua EXPR`, `unit SPEC`, `unit_dead SPEC`, `hp_below PERCENT`, `duel` (a duel runs),
`duel_challenge` (a challenge waits for an answer), `riding` (the player sits on a mount), `duel_result draw|winner|loser|runaway|remained` (how the last duel ended for the
player). `fight` also ends when a duel ends (a duel never kills); `lua HUD.duel_press(true|false)` answers the open challenge
prompt, `lua X2Unit:ChallengeTargetToDuel()` challenges the target.

## How the scripted click reaches the game

`ScriptPick` (in `script/mod.rs`) is a resource the pick systems read before they ray-cast the cursor:
`click SPEC` takes `take_left()` and sends `CSChangeTarget`; `interact SPEC` takes `take_right()` and goes on exactly
as after a ray hit (range, hostile, dead checks, `CSInteractNPC`, the quest dialog). `talk_input` reports the result
(`ScriptPick::talked`) so `interact` knows the talk began. Those are semantic picks, not mouse events.

## Scripted keys, clicks and the wheel are real input

`key`, `keydown`, `keyup`, `hold`, `click left|right`, `cursor` and `wheel` write the `VirtualInput` resource
(`crates/aa-client/src/ingame/virtual_input.rs`). Its `inject` system runs in `PreUpdate` **before**
`bevy::input::InputSystems`: it writes the same `KeyboardInput` / `MouseButtonInput` / `MouseWheel` messages winit
would and sets the primary window's cursor. Bevy updates `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>` and
`AccumulatedMouseScroll` from them in the same frame, `settings::update_input` resolves the original `hotkey.g`
bindings into `Actions`, and everything downstream sees the input exactly like a physical device:

* the on-foot controller (`moveforward`, `jump`, Shift, the mouse buttons) and the camera;
* the mount (`drive_mount`) and vehicle drivers, which read the same `Actions` and `ButtonInput` state;
* the dynamic action bar (`do_interaction_1..4`, F/G/H/J) and the action-bar slots (`1` ... `0`);
* Escape (`targeting::escape_key`, the placement), `Tab`, the window hotkeys and the HUD's own keyboard/mouse input.

`key f` is a tap: the press and the release are in the same injected frame, so `just_pressed` fires once.
`hold w 3` presses now and releases three seconds later; `keydown` / `keyup` leave the timing to the script. A
scripted `walk` is a thin wrapper over this: it holds `W` (`VirtualInput::key_down`) while it turns the camera
(`OrbitCamera::yaw`), so the real movement, collision and movement packets run, and releases it when it arrives or
fails. `clear_edges` (a new placement) releases any keys the script still held.

## Known limits

* No renderer in `--headless`: nothing that needs a camera ray or a viewport works with the mouse; use the script
  commands (`click`, `interact`) instead.
* Doodads cannot be clicked (the client has no doodad interaction yet): `interact doodad` sends the skill directly.
* The original UI scripts log one Lua error per world entry (`tab_language.alb:178: attempt to index global 'self'`);
  pass `--allow-lua-error tab_language` to ignore it.

## Resources (one client, standing in Desiree Peak, 1920x1080, Ryzen 9800X3D / RTX 5070 Ti, dev profile)

| Mode | Peak working set | CPU in steady state |
|---|---|---|
| windowed (`--script`) | 1.31 GB (+ VRAM) | about 86 % of one core (uncapped frame rate) |
| `--headless` (30 Hz, no GPU) | 1.15 GB | about 34 % of one core |
| `--headless --offscreen-shot` (GPU, no window) | about 2.0 GB | not measured |

Start to in-world takes about 2 s on a warm cache.
