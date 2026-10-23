# x2game.dll 1.2 constant resolution (runtime dump with `.data`)

Source dump: `D:/aa-re/client12/x2game_unpacked_data.dll`, runtime image base `0x38ff0000`
(the PE header still says `ImageBase=0x1000`, so the helper rebases `VA - 0x38ff0000`).
Decompile: `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (same VAs).
Helper: `pe_read.py` in this folder (`read_va(pe, va, fmt)` maps RVA through the section table).

Legend: **static** = initialised bytes present in the dump (`.rdata`/`.data`) or in shipped Lua
bytecode; **runtime** = global was zero/current-state at dump time and is set by the running client.
Cvar entries are static *defaults*; the live value is mutable.

| Spec | Item | Function (VA) | Global (VA) | Dump value | Static/runtime | Bevy currently uses |
|---|---|---|---|---|---|---|
| re-placement §1 | plain wheel step coefficient | `FUN_39016fe0` `0x39016fe0` | `0x3999cf98` (+ `0x3999cf94` for right) | `+/-0.017453292` rad = **1 deg**, multiplied by action `0x5a` value and `X` | static coefficient; `X` runtime | `ROT_STEP_PLAIN = 10 deg` (`placement.rs:71`) |
| re-placement §1 | plain-step runtime multiplier `X` | `FUN_39016fe0` `0x39016fe0` | `DAT_39adac58` | `0` (u32); `*DAT->vtbl[3]()`, object not constructed in the dump | **runtime** | Bevy hard-codes 10 deg; **still unresolved**, effective step = 1 deg x action x X |
| re-placement §1 | Alt (small) step | `FUN_39016fe0` | `0x3999cf90` / `0x3999cf8c` | `+/-0.0099999998` rad = 0.573 deg | static | `ROT_STEP_SMALL = 0.01` (`placement.rs:66`) - matches |
| re-placement §1 | Shift (large) step | `FUN_39016fe0` | `0x3999cf80` = pi, `0x3999cf88` = 0.083333336 | `0.26179939` rad = 15 deg | static | `PI/12` (`placement.rs:64`) - matches |
| re-wagons §3/§7; re-transport §11 | unit/target interaction range (wagon, ship, house) | `FUN_393bf270` `0x393bf270` | `0x39ac79fc` = 9.0; `0x39ac7a00` = 12.25 | squared range -> **3.0 m** normally, **3.5 m** when the interacting unit's `+0x3c26` flag is set (mounted/attached state) | static | `PICK_RANGE = 4.0` (`vehicle.rs:248`); house uses skill 4/6 m (`hud/houses.rs:34`) |
| re-transport §11 gap 5; "4 m transfer/boarding" | bind precheck (ship/wagon) | `FUN_39346290` `0x39346290` | `0x39ac7408` | `16.0` squared -> **4.0 m** | static | `BOARD_RANGE = 10.0` (`vehicle.rs:283`) |
| re-mounts §1/§9 | `CSMountMate` send distance | `FUN_3909b070` `0x3909b070` | `0x39ac3878` | `9.0` squared -> **3.0 m** | static | `SEND_RANGE = 3.0` (`mount_anim.rs:274`) - matches |
| re-mounts §2 | immediate-dismount side offset / vertical / fallback | `FUN_3909b1a0` `0x3909b1a0` | `0x39ac3888` = 1.5, `0x39ac388c` = 1.0, `0x39ac3890` = 4.0 | **1.5 m** to the mate's side, z x1.0; 4.0 fallback | static | no numeric side placement: get-off clip root motion (`mount.rs:515`, `mount.rs:909`) |
| re-mounts §2 | approach arrival/slow/vertical tolerances | `FUN_390aa730` `0x390aa730` | `0x399a3240` = 0.64, `0x399a3244` = 2.25, `0x399a3238` = -1.5, `0x399a323c` = 2.5 | **0.8 m** arrive, **1.5 m** half-speed, z **-1.5..2.5 m** | static | `ARRIVE = 0.8`, `SLOW_RANGE = 1.5` (`mount_anim.rs:280-281`) - matches |
| re-mounts §2 | approach point side offset | `FUN_390a2c90` `0x390a2c90` -> `FUN_390aa730` | controller `+0x20` | normalised side vector, no static scalar; built from mate orientation and entry side | **runtime** | `approach_target` uses **1.0 m** at the side (`mount_anim.rs:323`) |
| re-mounts §7/§10 | F-key mount reach | no native F path found | - | original F key not in native code; generic interaction gate is 3.0 m (`0x39ac79fc`) | n/a | `KEY_MOUNT_REACH = 6.0` (`owned.rs:862`) - Bevy design value |
| re-social §2.7 | trade-start range | `FUN_394fd9a0` / `FUN_394fddf0` (`0x394fd9a0`, `0x394fddf0`) | `0x39ac9b84` | `25.0` **squared** -> **5.0 m** (the doc's "25 m" was the squared value) | static | no client range check (`hud/trade.rs:135`) |
| re-crafting §3.4 | workbench use distance | `FUN_390f7b40` registration | `0x399a7660` | **2.1 m** | static | `INTERACTION_DISTANCE = 2.1` (`hud/doodad.rs:58`) - matches |
| re-crafting §3.4 | dynamic action-bar workbench distance | `FUN_390f7b40` | `0x3999f85c` | **25.0 m** | static | `CRAFT_BAR_DISTANCE = 25.0` (`hud/doodad.rs:60`) - matches |
| re-crafting §3.3 | `CRAFT_MAX_COUNT` | Lua root chunk, `crafting_renewal_view.alb` | not a PE global | table: `PAGE_PER_HIGH_RANK_ITEM=5`, `MATERIAL_ITEM=12`, `VIEW_CRAFTABLE=999`; it is **not** a batch cap (batch max = `GetExecutableCraftCount`) | static Lua bytecode | original ALB runs unchanged in aa-lua |
| re-uitoolkit gap 6 | mouse-sensitivity slider min/max | `FUN_39492ce0` `0x39492ce0` | option objects `0x39afe224` / `0x39afe228`; defaults at `0x39ac4d4c` / `0x39ac4d60` | **10.0 / 30.0**; `cr_sensitivity` default **20.0** (`0x39ac4d38`), names `cr_sensitivityMin/Max` (`0x399b13f4`, `0x399b13e0`) | static defaults; live option objects runtime (0 in dump) | `return 0.1, 3` (`settings_options.lua:115`) |
| re-combat §3/§4 | tab-target cvars | `FUN_390f7b40` registration | `0x3999eda8` = 4, `0x3999ddc8` = 50, `0x3999d01c` = 60, `0x3999f8f0` = 20, `0x3999edbc` = 3, history max = 10 (immediate) | **4 m round / 50 m fan / 60 deg / 20 m z / 3 s / 10 entries** | static defaults | single `TAB_RANGE = 40.0`, nearest list (`hud/targeting.rs:15`); fan/history absent |
| re-inventory gap 2 | sapling/security item cooldown | `FUN_3944d9e0` `0x3944d9e0` | `0x399daf58` (f64) = 600.0; `600000` ms immediate | **600 s** (600000 ms) | static | `cooldown: None` (`hud/panels.rs:161`) |
| re-housing §3 | public-farm list request throttle | `FUN_39035560` `0x39035560` | `0x3999d848` (f64) | **2.0 s** | static | requests immediately on `SCShowCommonFarm` (`hud/farm.rs:195`) |
| re-housing §3 | house interaction range | `FUN_393befd0` `0x393befd0` | `0x39ac79fc` | `9.0` squared -> **3.0 m** | static | skill range 4 m / 6 m (`hud/houses.rs:34-35`) |
| re-movement §2/§4 | swim side multiplier | `FUN_390f7b40` registration | `0x399a24cc` = 0.60000002 | **0.6** (doc guessed 0.5) | static | forward and side both 3.0 m/s (`swim.rs:37`); side 0.6 missing |
| re-movement §2/§4 | other swim cvars | `FUN_390f7b40` registration | `0x399a1d94` = 0.8, `0x399a87e8` = 0.7, `0x3999c47c` = 0.5, `0x3999cffc` = 2.0, `0x3999f9d0` = 12, `0x3999dd34` = 0.2 | back 0.8 / up 0.7 / down 0.5 / buoy 2.0 / jump 12 / range+end depth 0.2 | static | `swim.rs:39-54` - all match |
| re-movement §3 | quest/NPC talk gate | generic interaction `FUN_39093860` users | `0x39ac79fc` | generic gate **3.0 m**; exact quest-talk call site not isolated | static range, partial | `TALK_RANGE = 6.0` (`hud/dialog.rs:52`) |
| re-transport §10.5 | wind globals | producers not traced | `0x39ae6c90` / `0x39ae6c94` | runtime weather values | **runtime** | sail billow not implemented (open) |

## Still unresolved (and why)

1. **Placement plain wheel step (`ROT_STEP_PLAIN`).** The static coefficient is exactly 1 deg, not
   10 deg, but the per-notch multiplier `X` is the virtual call on `DAT_39adac58`, whose storage is
   `0` in the dump. The object has no static initialiser (zero in the dump; presumably constructed
   later), and no direct writer to `0x39adac58` appears in the decompile. Therefore the effective step cannot be
   stated; 1 deg x action-value x X. Bevy's 10 deg remains an unverified placeholder.
2. **F-key mount reach.** No native F-key mount path exists; the owner capture only says F works
   next to the mate. The generic interaction gate is 3 m, but the 6 m in Bevy is a design choice,
   not an original constant.
3. **Mount approach offset.** The side point is recomputed each frame from the mate quaternion and
   entry side (controller `+0x20`), not from a static distance; only the 0.8/1.5/2.5 tolerances and
   the separate 1.5 m dismount placement are static.
4. **Quest talk range.** The generic interaction gate is 3 m, but the quest-specific talk call site
   was not isolated, so applying 3 m to quests is still an inference (Bevy uses 6 m).
5. **Tax-certificate divisor (re-housing, 10,000)** is not a static constant; it comes from the tax
   item's price/UI getter (`FUN_39478d80`), so it remains data-driven/unresolved.
6. **Wind** `DAT_39ae6c90/94` are runtime weather state, not constants.

Everything else listed above is now read from the dump and is static initialised data (or shipped
Lua bytecode), so it no longer needs an oracle measurement.
