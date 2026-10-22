# UI toolkit and Options (`uitoolkit`)

## 1. Scope and sources

The original client has **two UI widgets stacks**.

* **`x2ui.dll`** (image base `0x10000000`; `D:/aa-re/client12/engine2/out/x2ui.dll.c`) is the CryEngine `IUI` module the game loads from `FUN_3921db60 @ 0x3921db60`: `InitDesktop` (`0x10001990`) creates a `DesktopListener`, registers it as input and hardware-mouse listener, registers cvar `ui_draw_level` (default 1), loads the top-level widget `ui/world.ui` (`CreateTopLevelWidget`, `0x10002aa0`), and runs `scripts/x2ui/x2ui.lua` (`0x10001990` area, string `1005ac40`; call site `0x3921db60`). Its widgets are built from `.g` `Data` files under `Game/` and take types 0..6 = window, label, button, editbox, scrollbar, listbox, world (`CreateWidget`, `0x100025e0`).
* **`x2game.dll`** (image base `0x38FF0000`) implements the in-game X2 toolkit that the `scriptsbin/x2ui/*.alb` Lua drives: `UIParent`, `CreateWindow`, `CreateWidget`, handlers, animations, scroll and cursor. This is the stack Bevy re-implements. `x2ui.dll` has no tooltip and no `WidgetManager::Pick`; those are `x2game` (`WidgetManager::Pick @ 0x392a8ac0`).

Sources used: `x2game.dll.c`/`funcs.csv`/`strings.txt` (addresses are the VAs printed in the dump headers), `x2ui.dll.c`/`.strings.txt`, the `.alb` bytecode under `D:/aa-re/client12/ui/game/scriptsbin/`, `D:/AA1.2/hotkey.g`, server packet definitions in `C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/src`, and the Bevy tree `D:/aaemu-client/AABevy1.2` (branch `main`). Lua line numbers are the bytecode line table.

## 2. The rules

### 2.1 Widget creation, identity, layers

`ScriptBindWidget`'s constructor (`0x39268c50`) registers the whole Lua vocabulary: `CreateChildWidget(typeStr,name,index)`, `CreateDrawable`, `Show`, `IsVisible`, `Enable`, `Clickable`, `EnableKeyboard/Focus`, `SetHandler/RegisterEvent`, `Raise/Lower`, `SetUILayer`, `StartMoving/StartSizing/StopMovingOrSizing`, `UseResizing`, `SetResizingBorderSize`, `EnableScroll`, `GetMinMaxValues`, `SetText/GetText/LText`, animation setters, `SetAlpha`, `ApplyUIScale`, `SetScale`, and `CreateChildWidgetByType`. Child creation takes a type name (`"window"`, `"button"`, `"label"`, `"textbox"`, `"editbox"`, `"listbox"`, …), a unique name and a 1-based index; the parent is reflected back into a Lua table.

`SetUILayer(name)` is implemented at `0x3926ad30`: it resolves the name through a fixed 8-entry table (`0x3922c0e0`), returns 8 and logs `"invalid layer 0X%llX %s."` for an unknown name, and otherwise reparents the widget into that layer list. Observed names in the scripts are `system`, `game`, `ADDON`, `tooltip`, and `DEFAULT` (e.g. `baselib/tooltip/tooltip_view.alb:1326` sets `SetUILayer("tooltip")`, `unitframe/unitframe_view.alb:5` sets `"game"`, `dynamic_action_bar_view.alb:0` sets `"DEFAULT"`).

The manager owns **eight layers**. `WidgetManager::Pick` (`0x392a8ac0`) walks layer slots 0..7 and, inside each, the widget list in head-to-tail order, calling the widget hit test; the first hit wins. `DrawVisibleWidgets` (`0x392a8830`) walks from the requested layer **down to 0**, so draw index 0 is the front-most layer. A shown window therefore occludes and is picked before lower layers. `Widget::Show` (`0x39266130` → `0x3927e420`) changes the visible flag, raises `OnShow`/`OnHide`, and re-applies `ui_scale` when the shown widget is a scale-opted-in root; stacking is changed separately with `Raise`/`Lower` (`0x39265260`/`0x39265220`). The `x2ui` desktop `Show(bool,z)` (`0x10008970`) additionally clears focus/hover/capture when the hidden widget or a descendant owned them (`FUN_100022c0`).

`Widget::Update` (`0x3927de60`) is the per-frame rule: it fires the two update script slots (9 and 10), steps alpha animation (`SetAlphaAnimation`: initial/final alpha, velocity, acceleration), steps scale animation (`SetScaleAnimation`), recursively updates only visible children, then calls the native `OnUpdate` vtable. Time is converted with the constant at `0x3999cf30`; this is the ms→s factor, so animation and tooltip timers below are in **milliseconds** `[I]`.

### 2.2 Windows: open, close, drag, resize

`baselib/window_view.alb:195` (`CreateWindow`) builds a `window` child of `UIParent`, shows it, adds a title bar and close button; `baselib/window.alb:8` defines opening/closing:

* **Open** is `ShowProc`: if `UIParent:EnableUIAnimation()` is true it plays `SetStartAnimation` (scale 0.95→1 with alpha 0→1; `baselib/window_view.alb:1`, `baselib/window.alb:58`); otherwise it calls `Show` immediately.
* **Close** (right-click on the frame, close button click, or `OnClose`) calls the optional `OnClose` handler, then `Show(false)`, then `X2Cursor:ClearCursor()` (`baselib/window.alb:8-23`). The close button is bound to `OnClick` (`window.alb:6`).
* **Move** is explicit Lua, not implicit title-bar dragging: on `LeftButton` mouse-down the title bar calls `X2Cursor:ClearCursor()`, `X2Cursor:SetCursorImage(CURSOR_PATH.MOVE,0,0)`, `StartMoving` and returns `true`; on mouse-up it clears the cursor and calls `StopMovingOrSizing` (`baselib/window.alb:28-44`). `RegisterForDrag(button)` is the alternative: it iterates the widget's children and sets per-button drag flags (`0x392667b0` → `0x39278c40`: Left/Right/Middle/Wheel); the native drag start then raises `OnDragStart` (`Widget::OnDragStart 0x3927cf30`, `ButtonWidget::OnDragStart 0x392820e0` maps button 1 to anim 2, 3 to 7, else 0xc).
* **Resize** is opt-in per widget: `UseResizing(true)` enables the edge strips, `SetResizingBorderSize(left,top,right,bottom)` sets their widths, `SetMin/MaxResizingExtent` clamps the extent, and `StartSizing(anchorPoint)` begins an edge drag (`0x392652a0`, `0x392659b0`, `0x39265450`, `0x39265130`). In 1.2 only the quest notifier uses it (`quest_context_notifier.alb` 1507/1516); borders are native-pixel strict strips.
* **ESC** is native: `SetCloseOnEscape(v)` stores two bytes on the window (`0x31c` escape-enabled, `0x31d` escape-chain) (`0x3926b230`). The Escape action calls `0x3921b170`, which calls `0x392a8c20`: scan layers 0→7 and the intra-layer list, and close the first **visible** window with `0x31c != 0` through `0x392d4500` (dispatch Lua handler slot `0x29` `OnCloseByEsc`, then hide). If nothing closes, the target is cleared and sound id `0x6a` plays. The Lua-side `X2.WidgetClosedByEsc(windowId)` (`0x39440a90`) additionally closes the loot window when the id is `"lootWindow"`. So ESC order is: front layer first, list order within the layer, first escape-enabled visible window only.

### 2.3 Buttons, edit boxes, lists and scrollbars

*Buttons* are ordinary widgets with `Clickable`, four drawable states (`UI_BUTTON_NORMAL/PUSHED/HIGHLIGHTED/DISABLED`) and handlers `OnClick`, `OnEnter/OnLeave`, `OnMouseDown/Up`, `OnDragStart/Stop`. Their text is a child label or an image; `ApplyButtonSkin` picks the button style. `SetButtonTooltip`/`SetTooltip` attach tooltip content (not a timer).

*Edit boxes* are `editbox`/`editboxmultiline` children; `SetMaxTextLength`, `ApplyEditSkin`, `SetInset`, `SetLineSpace`, `SetCursorColor`, `SetAlign`; `EnableKeyboard/Focus`. Text is clipped to the box; `SetEllipsis` is a native text-drawable style flag (bit 4, `0x39274900`) and `SetAutoResize` a widget flag. Keyboard capture is done by the UI runtime, and the Options key-binding page uses `RequestInput(widget)` to capture the next key.

*Lists and scroll*: `EnableScroll(true)` plus `GetMinMaxValues("horz"|anything)` returns the current scroll range/offset; `ChangeChildAnchorByScrollValue("horz", value)` moves the content by a pixel offset; `FindPrev/NextWidgetOffset` return the offset of the previous/next child for keyboard paging (`0x39266cd0`, `0x39265860`, `0x39266d60`, `0x39266e30`). `UpdateRange`/`UpdateThumbPos` recompute the scrollbar thumb (`0x39265410`, `0x392653c0`). `components/scrollwindow.alb`, `scrolllistbox.alb` and `scrolllistctrl.alb` wrap this. The separate `x2ui.dll` listbox is data-driven: `AppendItem(text,value,color)` (`0x1000ba30`), `GetItem(index)` (`0x1000bad0`), `GetSelectedValue()` (`0x1000baf0`), with descriptor keys `listbox`, `scrollbar`, `scroll_on_append`, `up_button`, `down_button`, `horizontal`, `thickness`.

*Cursor*: `X2Cursor` (`ScriptBindCursor 0x39464fa0`) is the single cursor owner. `SetCursorImage(path,xOffset,yOffset)` pins a custom image cursor with a custom hotspot (`0x394648f0`); `ClearCursor` restores the system cursor; `ShowCursor(0)` hides the pointer and any non-zero value shows it (`0x39464880`, which calls `HideCursor` for 0); `GetCursorInfo`/`GetCursorPickedBagItemIndex/Amount` describe a held item/spell/macro. When no custom image is set, `UpdateSystemCursorIcon` (`0x39434ce0`) chooses the resize arrows from the hovered widget border and the normal/HUD cursor otherwise. Window dragging sets the MOVE image with offset (0,0); dropping clears it.

### 2.4 Tooltips

The toolkit has eight pooled tooltip windows (`tooltip.num = 8`, `tooltip.tooltipWindows`, `tooltip_view.alb:279`) plus specialised ones. They hang off `tooltip.rootWnd` on layer `ADDON`; the pooled windows set `UILayer("tooltip")`, are children of `UIParent`, `EnablePick(false)`, with initial extent 260×100 and minimum width 250. Each window has `delayTime` and `fadeTime`; `SetViewOfWindowToolTip` sets `delayTime` from its argument and `fadeTime = 50` ms (`tooltip_view.alb:95-96`). `SetTooltipWindow`/`SetTooltipDelayTime` (same file, line 408 and `tooltip.alb:3902`) set the delay for the next tooltip; `ShowTooltip` writes `delayDurationTime` and `fadeDurationTime` and hides until the delay elapses. `OnUpdate` (`tooltip_view.alb:539`) subtracts the frame time from `delayDurationTime` (clamped at 0), then from `fadeDurationTime`, and sets `alpha = fadeDurationTime / fadeTime`; a tooltip is therefore fully opaque after the delay and fades out over 50 ms. Content lines are pushed with `AddLine(text,fontPath,fontSize,column,align,indent)` and `AddAnotherSideLine`.

Placement is cursor-relative and screen-clamped (`tooltip_view.alb:408-537`, `tooltip.alb:37-260`). It reads `UIParent:GetCursorPosition/GetScreenWidth/GetScreenHeight`, chooses above/below from the cursor's half of the screen, flips left/right when the text would cross the edge, offsets by ~2–10 px, clamps with a 30 px screen limit (`SetTooltipOffsetScreenLimit`), and calls `CalcDontApplyUIScale` so tooltip positions are in screen pixels.

### 2.5 Text, fonts, outlines and UI scale

Text is a drawable (`CreateTextDrawable(path,size,layer)`, `ScriptBindTextDrawable 0x3925b900`). Its native flags are bits of the text-drawable style word at `+0x34`: shadow = bit 0 (`0x392748a0`), **outline = bit 2** (`0x392748e0`), ellipsis = bit 4 (`0x39274900`); `SetFont/SetFontSize/SetAlign/SetLineSpace/SetAutoWordwrap/SetAutoResize` complete the style. The layout constants shipped in Lua are `FONT_SIZE = {SMALL=11, MIDDLE=13, LARGE=15, XLARGE=18, XXLARGE=22}` and `FONT_PATH = {DEFAULT="ui/font/yd_ygo540.ttf", SNAIL="ui/font/yoon_snail_b.ttf"}` (`baselib/layout_variable.alb:37-48`); `TEXTBOX_LINE_SPACE = {SMALL=3, TOOLTIP=4, MIDDLE=5}` (same file 26-31). `SetAutoWordwrap(true)` wraps at the widget width minus insets/indent; `SetAutoResize(true)` grows the widget to the text; `SetEllipsis(true)` truncates with `...` when a non-wrapping line exceeds the width. Fonts are rasterised through the FreeType-based `cryfont.dll`; layout is in the game's text drawable.

UI scale is the cvar `ui_scale`. `UIParent:GetUIScale()` reads it; `UIParent:SetUIScale(value, apply)` (`0x3925cd40`) writes the cvar and, when `apply` is true, calls `0x39426990`, which stores the value globally and re-applies it to every widget whose **ApplyUIScale** flag (`+0x2b6`) is set (`0x392a7170`). The widget constructor sets that flag to 1 by default (line `506186` of `x2game.dll.c`), and `ApplyUIScale(false)` clears it. During scale animation the root baseline is `ui_scale` and the animated scale is divided/multiplied by it (`Widget::Update`). At screen start (`0x39437e90`) the client checks whether `ui_scale` exists; if not it runs `SetDefaultUIScaleByResolution`, i.e. a resolution-derived default, and then applies it.

### 2.6 Options window

`ScriptBindOption` (`0x39493230`) exposes `Save`, `Reset`, `CreateOptionItemFloat/String`, `GetOptionItemValue(ByName)`, `SetItemFloat/StringValue(ByName)`, `SetItemFloatValueWithoutModify`, `SetItem*Default*`, `GetResolutionCount/GetResolution`, `EnumAAFormats`, `GetModifiedRestartOption`, `RemoveModifiedOption`, `GetNextSysSpecFullValue`, `IsRestartOption`, `GetMinxMaxOfMouseSensitivity`. Save levels are `OL_CHARACTER=0` and `OL_SYSTEM=1` (same function). The option constants are Lua globals resolved here; the tail is registered with explicit values: `0x3d USE_ONLY_MY_PORTAL`, `0x3e SMART_GROUND_TARGETING`, `0x3f COMBAT_MSG_LEVEL`, `0x40 COMBAT_MSG_VISIBILITY`, `0x41 CAMERA_FOV_SET`, `0x42 CAMERA_USE_SHAKE`, `0x43 MOUSE_SENSITIVITY`, `0x44..0x46 skill/item tooltip switches`, `0x47..0x4f ignore-party/raid/expedition/family/jury/trade/whisper/duel/faction invitations`, `0x50 IGNORE_CHAT_FILTER`, `0x51 ACTION_BAR_LOCK`, `0x52 SLOT_COOLDOWN_VISIBLE`, `0x53 HIDE_TUTORIAL`, `0x54 SHOW_LOOT_WINDOW`, `0x55 CUSTOM_CLONE_MODE`, `0x56 CUSTOM_MAX_CLONE_MODEL`, `0x57 CUSTOM_MAX_MODEL`. The earlier ids (`FULLSCREEN`, `SCREEN_RESOLUTION_WIDTH/HEIGHT`, `GAMMA`, `VSYNC`, `RENDER_THREAD`, `DIRECTX`, `GRAPHIC_QUALITY`, `SOUND_QUALITY`, `BG_QUALITY`, `CHAR_QUALITY`, `VIEW_DST`, terrain/shadow/shader/effect/water knobs, `MASTER/BGM/SFX/USE_COMBAT_SOUND/CINEMA/USER_MUSIC` volumes and the name-tag, movement and targeting switches) are the same registry names read from Lua by the same constructor.

`option.alb` lays out the window 680×615, close-on-escape, a category menu and the pages: screen (`ui_scale` default 1.0, system level, plus resolution, screen mode, render thread, DX, gamma, camera FOV and VSync), advanced screen (`MasterGrahicQuality` default 4.0, system, plus the quality sliders), sound (sound quality + master/BGM/SFX/combat/cinema/user-music volumes, 0..10 steps, user-music-disable-others), interface/name-tag/game-info, action bar (`ShowActionBar_1..4`, defaults from `localeView`, character level, plus `OPTION_ITEM_SLOT_COOLDOWN_VISIBLE`), key bindings.

Named options with defaults read from the registration tables (`option/interface_option.alb:59`, `option/interface_actionbar_option.alb:7`, `option/screen_option.alb:28`, `option/screen_advanced_option.alb:51`):

| id | default | level |
|---|---|---|
| `ui_scale` | 1.0 | system |
| `MasterGrahicQuality` | 4.0 | system |
| `OptionBackCarryingOrder1st/2nd/3rd/4th` | 2/3/4/1 | character |
| `ShowHeatlthNumber`, `ShowMagicPointNumber` | 0, 0 | character |
| `ShowBuffDuration` | 1 | character |
| `ShowTargetCastingBar` | 1 | character |
| `ShowTargetToTargetCastingBar` | 0 | character |
| `ShowPlayerFrameLifeAlertEffect` | 1 | character |
| `UseQuestDirectingCloseUpCamera` | 1 | character |
| `FixedTooltipPosition` | 1 | character |
| `ShowEmptyBagSlotCounter` | 1 | character |
| `ShowPlayerHelmet`, `ShowMyBackHoldable`, `ShowMyCosplay` | 1 | character |
| `ShowMyBackPackWithCosplay` | 0 | character |
| `ShowChatBubble` | 1 | character |
| `ShowFps` | 0 | character |
| `ShowActionBar_1..4` | `localeView` value | character |

The control types (`option/common.alb`) are `checkbox`, `combobox`, `sliderbar` (integer steps, marks, 0..10 or per-option min/max), `radiobuttonH/V`, `keybindingcontrol`, plus labels/partitions. Each control's `Init` snapshots `originalValue` and shows it; `Save` writes the current value (`checkbox` 1/0, `combobox` selected index, `sliderbar` `floor(value)`, `radio` checked index) and then the frame calls the item's `funcOnChanged`; `Cancel` is just `Init`, i.e. it restores the snapshot, not the default (`option/common.alb:415-531`). Some controls deliberately apply live in their `OnChanged` (combat-message level/distance, mouse sensitivity) rather than only on Save.

The frame flow (`option/option.alb`, protos at lines 355, 376, 394):
* **Init / open**: `X2Hotkey:BindingToOption()`, reset the list, title `<category> - <page>`, call every sub-frame `Init` except `buttons`, `ResetScroll`, then `X2Option:RemoveModifiedOption()` — this records the opening values so Cancel can undo a Reset.
* **Save**: call every sub-frame `Save` except `SaveHotKey`, then `X2Hotkey:SaveHotKey()`, then if `IsExistWarningAllSubOptions` show the warning, then `X2Option:Save()`, close.
* **Cancel / OnCloseByEsc / OnHide**: call every sub-frame `Cancel` (restore snapshots).
* **Reset**: run the page's `resetProc` (re-apply defaults and re-init the controls) and show the `restartTip` label when a changed option is marked restart. `IsRestartOption`/`GetModifiedRestartOption` are native; the page marks some options as requiring a client restart.
* **Apply**: a native option save writes the value into the cvar/option store; `funcOnChanged` runs after the page save; `X2Option:Save` persists (see 2.8). `SetItemFloatValueWithoutModify` intentionally does not count as a user change (used by `AdjustUserSetGraphicQuality`).

### 2.7 Key bindings

`InitOptionHotKey` loads the maps; `BindingToOption` refreshes the page before display; `SaveHotKey` saves them. The page (`option/key_binding_option.alb`) has seven groups: character control (8), mount control (9), game control (10), camera (11), interface popup (12), shortcut (13), additional options (14).

Capture: clicking a key button sets it as `lastClickedWidget`, highlights it, and calls `RequestInput` plus `EnableFocus` (`0x...` registration in `0x39477260`; Lua `key_binding_option.alb:199`/`213`). On the next input the Lua builds the key string in the fixed order `SHIFT-`, `CTRL-`, `ALT-` (using `X2Input.IsShiftKeyDown/IsControlKeyDown/IsAltKeyDown`), appends the key **uppercased**, and calls `X2Hotkey:SetOptionBindingWithIndex(action,key,index,arg)`; an invalid action name shows `locale.systemHotKeyErrorMessage` (`key_binding_option.alb:183-224`). Right-click during capture cancels/confirms, and the Release button removes a binding (`0x39476460` → `0x393ffc20`: index is 1-based, `arg<=0` → slot 0, `arg>0` → slot 5). Rebinding the same action/key is a no-op; binding a key already used by another action **removes the other action's binding** (same function); fixed keys (`FIXED_KEY_BINDING`, `SHIFT-OVER`, the F-key shortcuts) are protected. `EnableHotkey(false)` disables dispatch. Bindings are per character (ui data type 2) and also written to `hotkey.g`.

### 2.8 Saved files and packets

* **System option store**: `option.g` in the per-account path (`%s/%s/%s`, `0x392f27e0`/`0x392f2940`). The loader (`0x392081d0`) sees the in-memory `Data` has `version < 1`, builds that path and reads `option.g`, then applies the values with `0x39207ab0`. The same `Data` is then cached in LevelDB / `uiData_<name>.g` like the other types; no code in this build writes `option.g` itself.
* **Character options and key bindings**: cached locally in LevelDB (`LevelDbSave` in `0x39217760`), also written as seven `uiData_<name>.g` files by `0x39217540`, and (for the server-backed types) sent to the server. The seven ui-data names/types (`0x399cc858`..`0x399cc918`) are `0 system_option`, `1 character_option`, `2 key_binding`, `3 quest_notifier_list`, `4 quest_context_state_values`, `5 common_server_data_1`, `6 common_local_data_1`; the static flag array at `0x39acb8e0` decides local vs server per type (`0x392178d0`). The client flushes all seven on a screen transition (`0x390f4420` calls `0x392178d0`) and when Lua calls `SaveCharacterUiData` (`0x39440a30` → the ui-data manager); `0x392f3350` serialises a named node into an 8192-byte buffer and sends it with a **500 ms per-type coalescing window** (`0x392f3290`).
* **Hotkeys**: defaults are read from `Game/default_binding.g` (`0x39401a50`); the user file is `hotkey.g` (`0x39400e80`; `version<1` → `"cannot load user hotkey binding file"` and defaults). `SaveHotKey` builds `primary`/`second`/`removed` trees of `mode → builtin|builtin_multi → action → key` and writes `hotkey.g` (`0x393ff1f0`). The live file at `D:/AA1.2/hotkey.g` shows this shape (`primary/normal_mode/builtin/escape open_config`, `builtin_multi/action_bar_button/1 1`, `removed/normal_mode/builtin/turnright 0 0`). The server persists the character blobs in its `options` table keyed by `owner` (character id) and `key` (ui-data type), loaded/saved by `db_character.rs:253-264` and `:957-959`.

Packets (server definitions, client builders):

| packet | opcode | fields | when |
|---|---|---|---|
| `CSRequestUIDataPacket` | `0x117` | `u16 uiDataType`, `u32 characterId` | on the character-list response, once per character for types 1 and 2 (`0x391eaf20` → `0x392175f0`); server reader `character_packets.rs:631-641` |
| `CSSaveUIDataPacket` | `0x118` | `u16 uiDataType`, `u32 characterId`, `string uiData` | on the UI-data flush (screen transition `0x390f4420` → `0x392178d0`, `SaveCharacterUiData` `0x39440a30`) and 500 ms-coalesced saves (`0x392f3350` → `0x392f3290`); server `character_packets.rs:687-704` |
| `SCResponseUIDataPacket` | `0x1ca` | `u32 characterId`, `u16 uiDataType`, `string uiData` (u16 length), `i32 byteCount+1` | answer to a request; server `world.rs:509-528`, client handler `OnResponseUIData 0x391e57e0` (types 1/2 go to the per-character option/keybinding buffers at `+0x2c`/`+0x2030`, others to `0x39217910`) |

## 3. Numeric parameters

| value | number | evidence |
|---|---|---|
| Font sizes SMALL/MIDDLE/LARGE/XLARGE/XXLARGE | 11/13/15/18/22 px | `baselib/layout_variable.alb:37-43` |
| Text box line space SMALL/TOOLTIP/MIDDLE | 3/4/5 px | same, lines 26-31 |
| Default/Snail fonts | `ui/font/yd_ygo540.ttf`, `ui/font/yoon_snail_b.ttf` | same, lines 45-48 |
| Tooltip fade | 50 ms | `tooltip_view.alb:96` |
| Tooltip windows / min width / initial extent | 8 / 250 / 260×100 | `tooltip_view.alb:279`, `:74-90` |
| Tooltip screen margin | 30 px | `tooltip_view.alb:102-127` (`SetTooltipOffsetScreenLimit`) |
| Options window | 680×615 | `option/option.alb:146` |
| UI layer count | 8 | `0x392a8ac0`, `0x392a8830`, `0x3922c0e0` |
| UI-data buffer | 8192 bytes (0x2000), text buffer 8191 | `0x39217640`, `aa-net/src/ui_data.rs:10` |
| UI-data send coalescing | 500 ms per type | `0x392f3290` |
| Widget default UI-scale flag | 1 (on) | `x2game.dll.c:506186` |

`x2ui.dll` descriptor keys (all read by its `Data` parser, strings `1005b4xx`): `image`/`three_part`/`nine_part` plus corner coords, `outline`, `font_path`, `font_size`, `align`, `texture`, `coords`, `inset`, `padding`, `width`, `height`, `bounds`, `auto_resize`, `background*`, `extent`, `title*`, `max_text_len`, `clear_on_action`, `password`, `movable`, `resizable`, `focus_window`, `dont_pick`, `overlays`, `widgets`, `auto_hide`, `horizontal`, `thickness`, `up_button`, `down_button`, `scroll_on_append`, `scrollbar`, `editbox`, `listbox`, `world`.

## 4. What is inferred

* `[R]` Everything in §2 except the rows below is read from code/data.
* `[I]` Timers are milliseconds: `Widget::Update` scales the frame delta with `0x3999cf30` before animation math, and the 50-unit tooltip fade would be 50 s otherwise.
* `[I]` The eight layer **names** are not dumped: the table at `0x39ac6a80` is in `.data`; the names listed are those the Lua passes to `SetUILayer`. Index order (0 = front) is inferred from Pick 0→7 vs Draw N→0.
* `[I]` Defaults for the native `OPTION_ITEM_*` ids are not in the scripts; they come from the cvar/option registration. Bevy's `native_default` (`aa-client/src/settings.rs:96-103`, rebuilt from `game/config64/cvargroups/option_*.cfg`) is the current reconstruction, not proof for 1.2.
* `[I]` Ui-data types 3..6 are server-backed/local according to the static `0x39acb8e0` flags; only 1 and 2 are proven by `OnResponseUIData`.
* `[?]` Exact tooltip delay values passed by individual callers (the per-call argument is clear; the common value was not extracted).
* `[?]` Exact word-wrap break rule (word vs character) and ellipsis string in the native text drawable.

## 5. Gaps vs Bevy (ranked)

1. **Character options and key bindings are not synchronised through UI data.** Original: options and keybindings are per-character server blobs (types 1/2) requested with `CSRequestUIData` (`0x392175f0`) and saved with `CSSaveUIData` (`0x39217640`), restored by `OnResponseUIData` (`0x391e57e0`). Bevy only persists them in its local JSON: `aa-client/src/settings.rs:69`, `:155-172`; `settings/bindings.rs:270`; the UI-data lane is used only for type 5 (achievements) (`hud/achievement/persistence.rs:118-132`). Fix: request/save types 1 and 2 around character select/world entry and merge the server blob into the option/binding store.
2. **No UI-scale persistence or resolution default.** Original `ui_scale` is a cvar restored at startup; if absent, `SetDefaultUIScaleByResolution` derives it (`0x39437e90`) and `0x39426990` applies it. Bevy clamps `SetUIScale` to 0.5..2.0 and only changes the live value (`aa-ui/src/lua/globals.rs:67-85`); startup is always 1.0 (`lua/animation/widgets.rs:63`) and the saved `ui_scale` option is never consumed. Fix: apply the saved option at boot, remove the clamp or match the original range, add the resolution default.
3. **Native option defaults are incomplete.** Bevy's `native_default` lists only ids 3, 7..33, 55 and 65 (`settings.rs:96-103`); original registers a default for every `OPTION_ITEM_*` in `0x39493230`. Ids 34..54 and 56..57 (sound volumes, name tags, click-to-move, glide, action-bar lock, cooldown visibility, loot/tutorial switches) fall back to 0 in Bevy, so a never-saved profile shows every enumeration at its lowest step/unchecked. Fix: extract the cvargroups' `[default]` values and extend the table.
4. **Option/keybinding storage format is not the original.** Original reads `option.g` (`0x392081d0`) and writes LevelDB/`uiData_<name>.g` (`0x39217760`, `0x39217540`) plus `hotkey.g` (`0x393ff1f0`). Bevy writes one `settings.json` (`settings.rs:69-90`) and a JSON `version:2` binding list (`settings/bindings.rs:270`): no 1.2 profile import and no `hotkey.g` interop, and the server path stays unused until gap 1 is fixed.
5. **Restart options are never reported.** Original `IsRestartOption`/`GetModifiedRestartOption` consult the option registry and the Options window shows `restartTip`; Bevy hardcodes `return false` (`aa-client/src/settings_options.lua`, `X2Option.IsRestartOption`) and never shows the restart warning. Fix: mark the ids flagged restart in the registry and show the tip when one changed.
6. **Mouse-sensitivity range is hardcoded.** Original reads two cvars and returns their min/max (`0x39492ce0`); Bevy returns `0.1, 3` (`settings_options.lua`, `X2Option.GetMinxMaxOfMouseSensitivity`). This changes the slider's endpoints and the saved value's meaning.
7. **Options that have no consumer.** Bevy's `graphics.rs` applies resolution/mode/vsync/gamma/draw distance/object/vegetation/shadows/clouds/effects/HDR/anti-aliasing/camera, but texture quality, terrain detail, character/water quality and DoF options are saved and never applied (`docs/aa-client-parity.md`, Options row). Original applies each through its cvar/engine knob (`SetItemFloatValue` writes the cvar); with the renderer exception these should at least be stored and exposed where a knob exists.
8. **`X2Option.Reset` semantics are approximated.** Original `Reset` (`0x39491ed0` → `0x392053e0`) resets every modified option to its registered default and clears its modified flag in the native registry. Bevy re-writes its own defaults into the store but has no registry/modified flags, so `GetModifiedRestartOption` and the Cancel-after-Reset baseline are reconstructed in Lua (`settings_options.lua`, `AA_CLIENT_OPTION_REMOVE_MODIFIED`).

Non-gaps worth stating: the Options window itself, its pages, tooltip content and delay logic, key capture and the list/scroll scripts run from the **original Lua bytecode** in Bevy (`aa-lua/src/stage.rs`, `aa-lua/src/toc.rs`), so their layout, labels, capture rules and tooltip timing are shared; native layout/wrapping/drawing and the persistence above are where differences remain. Bevy's ESC (`aa-ui/src/content.rs:120-146`) already dispatches `OnCloseByEsc` before hiding, matching `0x392d4500`.

## 6. Open questions

1. The exact `[default]` values of every `OPTION_ITEM_*` cvar in 1.2 (`game/config64/cvargroups/option_*.cfg`); this settles gap 3 and the sound/quality sliders.
2. The static ui-data flag array `0x39acb8e0` (which of types 0..6 is local vs server) and whether type 5 in the server comment is the same as `common_server_data_1`.
3. Whether `SetCloseOnEscape`'s second byte (`0x31d`) means "close chained child windows"; a live capture of a nested window would settle it.
4. Exact tooltip delay arguments used by `SetTooltip` callers and the wrap/ellipsis algorithm in the native text drawable.
5. The `option.g` schema version/migration and whether a `version<1` file is discarded or merged (the code fails the load and keeps defaults).

## 7. Live oracle measurements

None in this pass: all findings are from the static decompiles, the shipped Lua bytecode, `hotkey.g`, the server packet definitions and Bevy sources. No client was started and no packet capture was made. The next oracle session should capture `CSRequestUIData`/`CSSaveUIData` around the first Options Save and a key rebind, plus the `option.g`/`uiData_*.g` bytes.


