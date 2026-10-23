# What is left: ordered work queue for AABevy1.2

Date: 2026-10-23. Restructured from the 2026-10-18 stocktaking into the order the parity loop needs; no code changed.
Selected rows were corrected on 2026-10-23 after source and runtime checks; other rows remain historical and need a
current-code audit before assigning work. Sources: every `docs/*.md` gap section, `docs/research/*`, `docs/aa-client-parity.md` (the completion ledger), `git log`,
`git status`, `data/extract/game/scriptsbin/x2ui`, `crates/aa-client/src/hud`, `crates/aa-ui/examples/fake`, the AAEmu lane
source (`D:\aaemu-wt\bevy`, its `rust/tools/opcode_coverage/sc_sent.json`), `reference/`.

How sure each claim is: "doc" = a doc says so and I did not re-check the code. "code" = I grepped the tree. "run" = I ran it
on 2026-10-18. Two read-only sub-agent sweeps (server packets vs `aa-net`, x2ui windows vs HUD) produced the system lists; I
spot-checked the headline claims (auction, housing, duel, slave, tower defense, portal, cash shop have no `aa-client` consumer;
`SCBuffUpdated` is already decoded) but not every row. Sizes: S = under a day for one agent, M = a few days, L = a week or more.
Items that the parity ledger or git log has since closed are marked stale instead of being dropped.

## 1. Status summary

From a typed login (new `Screen::Login`, committed 18:27 today) through World Select (live lane list), Select Character (3D stage,
authored facial overlays, all eight playable heads), Create Character (all four 1.2 races, both genders, the three steps, server
create, name policy), Loading (world and HUD are built behind it) to the world, the client works end to end in the original Lua
UI, with the game on real data: streamed terrain, brushes, vegetation, roads, decals, water, sky, stars, moons, time of day, fog,
collision, walking / swimming / diving, remote players, NPCs, mounts as remote units, animated and equipped characters, combat
visuals, ambient audio and music, and a very large set of original HUD windows (bag, bank, equipment, merchants, mail, trade, party /
raid, friends, guilds / families, quests, map, chat, skills, buffs, crafting folio, achievements, ranking, trials, battlefield, pets,
enchant, repair, specialty quotes). Typical in-game speed is 70 to 95 fps at 1920x1080 on the shared machine, 38 fps in a big town
before the latest aa-fx fix. **The 2026-10-18 live block is gone:** on 2026-10-20 the client logged in, entered the world and
passed the headless smoke tests (titles, tooltip, Character Info, emotes, guild window) against the AAEmu server. Auction
settlement tail/reconnect and the final social unblock/offline/restart live checks remain unverified. By the parity ledger's
own words, "overall 1.2 parity is unfinished".

The parity loop and its three status fields are in `docs/aa-client-parity.md`. The area specs now cover: `re-placement.md`,
`re-mounts.md`, `re-mounts-full.md`, `re-wagons.md`, `re-unit-colors.md`, `re-movement.md`, `re-housing.md`, `re-combat.md`,
`re-inventory.md`, `re-quests.md`, `re-transport.md`, `re-crafting.md`, `re-social.md` and `re-uitoolkit.md`;
`re-equipment.md` and `re-skills.md` stay narrow notes. World, character, loading and the remaining rows still have no area
spec, so those rows cannot be done yet even where the feature mostly works.

Spec progress in the 2026-10-22/23 merges: placement gaps 1-7 and 9-12 (refused-build retry `8cb994a`, design match
`b9ad5b1`), mounts gaps 1-6 and 8, wagon gaps 1-3, 5 and 7-8 along with the measured packet flow (gap 4 is partly: helm and
wheels in, grip IK open), movement gaps 1 and 3-5 (ladders
`11d6122`, camera modes `5ea2227`, stance capsule `0fb4ba6`), housing gaps 1-5 (`231e9df` decoration, `5e52514`
maintain/sale/tax/rename), combat gaps 1-5 (`c86a280`, `2a5919e`, `6d646af`), inventory gap 1 (`644e473`, `03b8e6c`),
quest gaps 1-4 (`3037053`, `f79ebe4`, `a41e134`, `0e42647`), transport gaps 1, 2 and 4 (`fe37759`/`c0e7eec`, `654d092`),
crafting gaps 1-5 (`6d00d0d`, `075260d`/`38a2f5c`, `8f1255a`/`6405a87`), social gap 1 (`5401aaa`/`4d49692`) and UI-toolkit
gaps 1-3 (`ad80b3a`, `22a19b7`, `593c2c5`). The still-open gaps are listed per area in section 3 and under 4.14/4.18.

Only mounts, placement and the wagon flows, plus the action-bar pages and the quest tracker, have been compared live against
the original. Oracle batch 2 (2026-10-22/23): action-bar pages SAME and the 1-based slot numbering fixed in `c86a280`; quest
tracker SAME; a refused `CSCreateHouse` keeps the Start Construction builder (`8cb994a`). Quest talk/timer and ladders are
implemented but not drivable in the live oracle yet.

## 2. Engine systems first

These are shared foundations. Most partial rows wait on one of them, so they run before the area queue or in parallel with the
first area that needs them.

### 2.1 Animation graph player (CryEngine `.xml` / `.ag`)

**Why.** The original selects locomotion and actions from the compiled graph per actor model: inputs `PseudoSpeed`,
`RequestedTurnSpeedLZ`, `Stance`, `Action`, `Signal`, `Vehicle` and `VehicleSeat`, with per-state templates/transitions; the mount
feeds the rider each frame (`FUN_390dcc80`). Ad-hoc clip selection cannot reproduce gait bands, turn-in-place, idle/run
transitions, jump/swim states or mounted rider poses.

**Rows that depend on it.** Ground movement and camera; Water and mounted movement; Character and animation rendering; Combat and
HUD; Skill progression; Pets, slaves and transport; Duels.

**Next step.** Load `game/animations/graphs/*.xml` plus the matching `.ag`, compile states/templates/transitions and drive the
inputs; start with `horse_full_body` and `player_full_body` (state tables in `re-mounts-full.md` §2-4), then upper-body and
creature graphs.

### 2.2 Original tick rate for movement and animation

**Why.** Movement samples, animation phase, jump/fall and packet cadence all follow the original tick. The original clamps a
remote movement sample to `min(dt, 0.033 s)` (`FUN_390a30e0`, `re-mounts-full.md` §6); a different client step or send rate
changes speeds, gaits and interpolation.

**Rows that depend on it.** Ground movement and camera; Water and mounted movement; Combat and HUD; Skill progression; Pets,
slaves and transport; `N4`.

**Next step.** Measure the original tick and movement send rate (client log or oracle capture), pin the client fixed step,
animation sampling and `CSMoveUnit`/`VehicleMove` cadence to it, then re-run the mount gait trace and a wagon drive.

### 2.3 CryPhysics port (`aaemu-physics`) in the client

**Why.** 1.2 movement is CryPhysics-driven: wagon/Cart driving, roll/pitch, linear/rotational inertia, `max_climb_ang`, mount
jump/fall and collision. Bevy's vehicle is arcade with roll/pitch always 0 (`re-wagons.md` §6.9), and mount inertia semantics are
inferred (`re-mounts-full.md` §6-7).

**Rows that depend on it.** Ground movement and camera; Water and mounted movement; Pets, slaves and transport; `G2a`, `G4`,
`CH2`, `CB2`, `N4`.

**Next step.** Wire the bit-exact port (`C:/Users/stsiakal/AAEmu/rust/crates/aaemu-physics`, with `physoracle.exe` scenarios and
goldens) into the client as the living/vehicle mover and query service; start with the Farm Wagon from `re-wagons.md` §2/§4 and
match the oracle before replacing the arcade path.

### 2.4 Flow graphs and cutscenes

**Why.** Quest cameras/cinematics, animated entities/controllers and skill controller kind 8 are FlowGraph assets, and the client
has no flowgraph player. Hidden cinematic actors are still "aa-client's screen-flow responsibility" (`aa-world.md` §6), `WR6`
animated entities are open and `G6` is untouched.

**Rows that depend on it.** World geometry; Quests and map; Combat and HUD (flowgraph skill controller); Character and animation
rendering (animated props).

**Next step.** Load FlowGraph assets from the pak and implement module/entity nodes; wire the quest camera/cinematic actors and
skill controller kind 8; verify on one quest cutscene and one animated prop before enabling more.

## 3. Area queue

Queue order is priority. Independent areas may still run in parallel by crate ownership (`AGENTS.md`). Each entry gives the RE
spec status, the next step and every old item moved under it.

### 3.1 Movement and mounts

**RE spec.** `re-mounts.md` (summon/mount/dismount, attach/seat rules, measured packets), `re-mounts-full.md` (graphs, speeds,
movement code; §7 lists what stays inferred) and `re-movement.md` (walk/run/strafe, ladders, camera, collision). Mount gaps 1-6
and 8 are implemented; movement gaps 1, 3, 4 and 5 are implemented. Open: mount gap 7 (rider skills) and gap 9 (stirrup IK,
`CSRemoveMate` auto-dismiss), humanoid combat idle, and the `re-mounts-full.md` §7 inferences (PseudoSpeed producer, inertia
semantics, jump impulse, turn 360, camera, XL14 tier, rider idle quirk, transition choice, `normal_speed`); movement gap 2
(click-to-move), gap 6 (air momentum), gap 7 (stance/alertness/combat flags), gap 8 (swim multipliers and graph controls),
gap 9 (camera collision/fade) and gap 10 (follow). Mounts were matched live once; ladders are implemented but not drivable in
the live oracle yet.

**Next step.** Close the remaining rider/mount items (`CH2`): rider skills (`ride_ub`, `<weapon>_co_horse_attack_*`,
`horse_ac_spur_f`), stirrup IK, `CSRemoveMate` auto-dismiss and humanoid combat idle; then movement gaps 2 and 6-10; re-verify
placement/gaits with the animation graph player and tick rate, and do the live owner test (jump, strafe, mount/dismount,
ladder).

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| CH2 | units | Mount / rider placement is implemented and matched live once (get-on/get-off clips, PseudoSpeed gaits, inertia), but rider/mount skills (`ride_ub`, `<weapon>_co_horse_attack_*`, `horse_ac_spur_f`, jousting), stirrup IK, `CSRemoveMate` auto-dismiss and humanoid combat idle remain; `re-mounts-full.md` §7 inferences (PseudoSpeed producer, inertia semantics, jump impulse, turn 360, camera, XL14 tier, rider idle quirk, transition choice, `normal_speed`); `SCUnitMovements` types read from the packet not the object | `aa-client-units.md` Gaps, `re-mounts.md` gaps 7/9 | med | M | G2 |
| G4 | gameplay | Click-to-move, air momentum, stance/alertness/combat flags, swim multipliers and graph controls, camera collision/fade, follow, camera preset actions, hanging / moving supports, less common locomotion (`re-movement.md` gaps 2, 6-10) | parity row "Ground movement" | med | M | none |

### 3.2 Housing and farming

**RE spec.** `re-placement.md` (builder state machine, ghost model/tint, checks/error codes, plot border, packets, Start
Construction/Construction windows, F/G actions) and `re-housing.md` (decoration builder, maintain/sale/tax/rename, farm
windows). Placement gaps 1-7 and 9-12 are done; gap 8 (rotating plot dots) and the open questions remain. Housing gaps 1-5 are
done (`231e9df`, `5e52514`); open are gap 6 (right-click on an unfinished house casts the build skill), gap 7 (silent
buy/cancel-sale failures), gap 8 (unused `SCHouseFarm`), gap 9 (public-farm trigger), gap 10 (synthesized F/G), gap 11
(hardcoded `IsCastle`/`IsSiegePeriod`) and gap 12 (smaller parity items). Farming/crops still have no RE spec.

**Next step.** Finish `H2` (plain wheel step, rotating plot dots, right-click build casting, public-farm trigger, castle/siege
flags, live maintain-window and retail entry checks) and `G3` (gathering, farming, coffers, switches); crops need their own
`doodad`/`DoodadFunc*` RE pass before implementation.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H2 | HUD | Housing: placement, construction, tooltip, build precheck, the finished-house maintain window (tax, permission, recover, sale, buy, demolish), decoration placement (`231e9df`), counters/actability groups and sale/tax/rename (`5e52514`) exist (`hud/houses.rs`, `hud/house_maintain.rs`); still open: plain placement wheel step (unresolved, 10 deg placeholder `ROT_STEP_PLAIN`; one live retail count), plot dots rotating with the model / following the terrain (rotated retail screenshot first), right-click build casting, public-farm trigger, hardcoded castle/siege flags, silent buy/cancel failures, live maintain/deco checks, retail entry point of the finished-house window; server `systems/housing` ~14k lines | HUD sweep; opcodes 0x0c1 to 0x0ca, 0x056 to 0x060 | high | L | aa-net + aa-client + aa-world placement |
| G3 | gameplay | Doodad interaction: gathering, farming, coffers, switches (`ChangeDoodadPhase`, `SelectInteractionEx`, `SCCofferContentsUpdate`, `SCVegetationCutdowning` unhandled) | opcodes 0x096, 0x081 | high | M | none |

### 3.3 Combat and HUD

**RE spec.** `re-combat.md` is the area spec; gaps 1-5 are done (`c86a280` action-bar slot numbering, `2a5919e` target combat
state and `SCCombatText`, `6d646af` resurrection/`PLAYER_RESURRECTION`); gaps 6-9 are open (tab targeting, target-of-target,
GCD attribute/per-slot query, first-hit/preemptive state). `re-skills.md` stays a narrow note for skill controllers,
knockdown and effect lookup/beams.

**Next step.** Close `re-combat.md` gaps 6-9, then `CB1` effect decals/lights/forces, `CB2` casters/projectiles/poses, `CB3`
cooldown/GCD producers and TL lifetimes, `CB4` force-mode plus more reference captures; the skill-caster flowgraph (kind 8)
waits on 2.4 and mounted/flying casting on 3.1.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| CB1 | combat | Sun lighting/live HDR/pool cleanup and bounded adaptive ground drapes are committed and checked; client FxMaterial hit flashes are integrated. Bounded direct constant-clock Target ancestors are implemented (d34e9e4;65 tests). Native GeometryInPieces is implemented for the four authored flat compound root-node models (0cb85b4;72 tests), with ordered safety-budget truncation. Native receiver/depth projection, nested/skinned/per-piece-material compounds, wind/gravity force areas, indirect Target clocks and integrated per-skill comparisons remain open. Stretched beams do not establish ForceGeneration parity | 2026-10-23 d0b98da/cc9fc34; `aa-fx.md` rounds17/18, 59 tests plus all-target check and inspected captures; curved geometry has measured cost | med | M | native particle gaps and integrated comparisons |
| CB2 | combat | Mounted / swimming / flying casters, doodad and ship cannon projectiles; cast cancel on mount-jump; poses for 3 channeling skills; two-hand grip, instruments' placement and sounds; remote camera shakes | `aa-client-combat.md` Not done | med | M | G2 |
| CB3 | combat | Cooldown / GCD: native Fired producers, custom GCD, full timeline lifetimes, re-combat gap 8 (attribute multiplier and per-slot four-value query), lane cooldown emitter re-entry check | parity row "Skill progression", `re-combat.md` gap 8 | med | M | N1 for live check |
| CB4 | combat | Force-mode (protected / team) eligibility; tab targeting and target-of-target (re-combat gaps 6-7); first-hit/preemptive state (gap 9); skill-by-skill reference verification (only 6 combat captures exist) | parity "Combat and HUD", `re-combat.md` gaps 6-7/9, `reference/ingame/combat_*` | med | L | more captures |

### 3.4 Inventory and items

**RE spec.** `re-inventory.md` is the area spec; gap 1 (item security locks, `644e473`/`03b8e6c`) is done, gaps 2-11 are open
(bag/bank cooldowns, missing tooltip sections, level-requirement red state, unconfirmed state, aux equipment slots, worn-item
use, bank AA-point/locks, equipment stat getters, filtered-tab order, expand/destroy dialog). `re-crafting.md` covers the
crafting/trade-pack side; `re-equipment.md` stays a narrow note.

**Next step.** `H11` item workflows (enchant protected planning/shared pending/continuation, dye, look converter, temper),
`H10` remaining tooltip fields, `H7` crafting/gathering/trade-pack follow-ups; close `re-inventory.md` gaps 2-11; live
repair/expansion routing stays with the NPC entry (3.5).

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H7 | HUD | Crafting follow-ups: heavy-pack penalties, stale proficiency after a craft, hardcoded max labor and premium getters, gathering interactions and processing flows, the missing optional natives and ignored `SCWorldInteractionSkillList` (`re-crafting.md` gaps 6-10); live workbench input/permission validation | parity rows "Crafting" and "NPC interactions", `aa-client-specialty.md`, `re-crafting.md` | high | M to L | G4 |
| H10 | HUD | Tooltips: damage breakdown option, `ea_*` / `return_point`, buff caster stats on others, gems / tempers / enchants, repair cost; missing `re-inventory.md` gap 3 sections | `aa-client-hud.md` "Tooltip numbers", `re-inventory.md` gap 3 | med | S to M | none |
| H11 | HUD | Item workflows: enchant protected planning / shared pending / continuation, dye, look converter (`X2ItemLookConverter`), temper, plus `re-inventory.md` gaps 2, 4-11 (bag/bank cooldowns, level red state, unconfirmed state, aux slots, worn-item use, bank AA/locks, equipment getters, filtered-tab order, expand/destroy dialog) | `aa-client-enchant.md`, parity row "Inventory", `re-inventory.md` | med | M | none |

### 3.5 Quests and NPCs

**RE spec.** `re-quests.md` is the area spec; gaps 1-4 are done (`3037053` timer, `f79ebe4` marks, `a41e134` every talk
objective, `0e42647` the 16 NPC windows), gaps 5-10 are open (requirement kinds mostly default true, journal data stubs,
quest camera/decline no-ops, shop AA-point currency/catalog order, undecoded quest packets, `TryStartQuestContext` arity).
Oracle: the quest tracker is SAME in batch 2; quest talk/timer are implemented but not drivable live yet.

**Next step.** Close `re-quests.md` gaps 5-10, then `H8` NPC leftovers (coin/doodad shops, teleporter fare, live multi-service
verification), `H9` map (other-zone icons, climate/conflict overlays, live housing/public-farm icons, native radar
relations/area fog), `H5` portals/teleport/navi/return points, and `G6` quest camera (with 2.4).

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H5 | HUD | Portals / teleport / navi / return points: nothing wired, `SCCharacterPortals` etc. undecoded | opcodes 0x05b to 0x05d, 0x0b3, 0x115 | med | M | none |
| H8 | HUD | NPC services: the 16 original service windows, multi-service menu, repair routing, ability services and priest buffs are done (`0e42647`); still open: coin / doodad-shop NPCs, the server-side teleporter fare table, live multi-service verification | parity row "NPC interactions", `re-quests.md` gap 4 | med | M | none |
| H9 | HUD | Map: quest objective areas, notifier icons and ship/transport/fish radar are done; still open: other-zone icons, climate / conflict overlays, live housing / public farm icons, native radar relations/area fog | `aa-client-hud.md` Not done | med | M | H2, G2 |
| G6 | gameplay | Cinema / quest camera: hidden cinematic actors are "aa-client's screen-flow responsibility"; `X2Quest.UseQuestCamera` false | `aa-world.md` §6 | low | M | none |

### 3.6 Transport (ships, gliders, carts)

**RE spec.** `re-wagons.md` for the Farm Wagon (identity, model/wheels/helm, seats/boarding, driving, cargo/mode bar,
despawn; packet flow measured live) and `re-transport.md` for ships, gliders, transfers, siege weapons and the packet table.
Wagon gaps 1-3, 5 and 7-8 are done and gap 4 is partly done (helm and wheels; `$IK_Grip` IK open); open are gap 6 (SlaveLocator
ghost), gap 9 (CryPhysics driving) and gap 10 (name-tag offset unverified), plus the open questions (interaction/range,
rotation, wheel spin/remote smoothing, mate-wagon link). Transport gaps 1, 2 and 4 are done
(`fe37759`/`c0e7eec`, `654d092`); open are gap 3 (ship visual chain: rudder, wheel, sail, throttle, sounds), gap 5 (boarding
ranges), gap 6 (glider mode skills), gap 7 (deploy default/gate), gap 8 (static Summon Info), gap 9 (turret aiming), gap 10
(water feedback), gap 11 (extra anchor), gap 12 (arcade cars/tractors) and gap 13 (stuck-ship actions). `re-mounts.md` covers
the mount core.

**Next step.** `G2a` land-vehicle leftovers (SlaveLocator ghost, CryPhysics driving, `$IK_Grip`/helm, boarding range constant,
cargo models per pack, name-tag offset), `G2` ships/gliders/hanging/flying and ship cannons (transport gaps 3, 5-13), `N4`
outbound land/ship
movement plus type-3 bodies. CryPhysics driving waits on 2.3; ship cannons remain blocked by the server's log-only
slave-equipment stub. The wagon packet flow was measured live; transfer boarding/routes have not been captured live.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| G2a | gameplay | Land vehicles (Farm Wagon), left after the `fix/wagon-retail`/`fix/wagon-mount-packets` passes (`re-wagons.md` 6): gap 6 summon locator ghost (`SlaveLocator`: cursor ray, wheel rotation, OBB validity, red invalid material) and gap 9 CryPhysics driving (inertia, roll/pitch, `max_climb_ang`); `$IK_Grip_L/R` hand IK on the helm and the helm's exact orientation (no `$helms` helper in the wagon skeleton: it sits between the grips); the boarding range constant `DAT_39ac79fc` (10 m stand-in; `re-transport.md` gap 5 wants 4 m bind / 3 m interaction); cargo crate models per loaded pack, the name-tag offset (`name_tag_offset` 3.0) and `VehicleMove` remote steering from the packet instead of the heading rate are unverified live | `re-wagons.md`, `re-transport.md` gap 5 | med | M | G2 |
| G2 | gameplay | Transport: mounts, owned land vehicles and transfers (boarding/bar/routes, `fe37759`/`c0e7eec`), plus ship parts and damage states (`654d092`) are in; remaining: ship visual chain, boarding ranges, glider mode skills/deploy gate, live Summon Info, turret aiming, water feedback, extra anchor, arcade cars/tractors and stuck-ship actions (`re-transport.md` gaps 3, 5-13); hanging/flying state (`SCSkillControllerState`, `SCUnitFlyingStateChanged`, `SCShipyardState`, `SCHung`, `SCBondDoodad` undecoded); ship cannons blocked by the server's log-only slave-equipment stub; server drops the ship summon position and always binds the driver seat | `aa-client-units.md`, parity "Pets, slaves", `re-transport.md`, opcodes 0x06d, 0x07a, 0x137 to 0x13a, 0x1aa | med | L | N4 |
| N4 | network | Outbound land-vehicle (`MoveVehicle`/type 2) and ship helm (`SteerShip`/type 4) movement are written; type-3 ship bodies and hanging/support blocks remain read-only | `aa-net.md` Known gaps | med | M | G2 |

## 4. Remaining ledger rows

Rows not covered by 3.1-3.6, in ledger order. Each entry gives the RE spec status, the next step and the old items moved under
it.

### 4.1 World selection and login

**RE spec.** None. **Next step.** `N2` reconnect policy, `N5` houses in the char list/premium lobby/regional logins, `L1` login
fields and text; `W1` has nothing open besides these.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| L1 | login | Fields / buttons are drawn (no original `field.png` / `button*.png`); no text selection, IME, Ctrl+C; generic kick text off World Select | `aa-client-login.md` Not done | low | S | none |
| N2 | network | Reconnect after a dropped game connection: session returns to idle | `aa-net.md` Known gaps; parity "reconnect policy" | med | S | none |
| N5 | network | `SCLoginCharInfoHouse` (houses in char list), premium grade in lobby, Korean / Mail.Ru / Tencent logins | `aa-net.md`, parity row 1 | low | S | none |
| W1 | world select | Nothing open besides N5 / N2 (matches captures, 53 of 53 widgets) | `aa-ui.md` table | low | S | none |

### 4.2 Character selection and creation

**RE spec.** None. **Next step.** `C1` skin/fur/hair lighting, `C2` hat/beard/lip sync/appearance rebuild, `C3` grid order,
`CR1` wind/dummy/elf fog, `CR2` step-3 GPU cost, `CR3` beauty salon.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| C1 | char select | Skin: one calibrated colour per model instead of the HumanSkin SSS shader; fur drawn flat (ferre); hair spec is a baked strand map; flatter lighting than captures ("Char BackLight" lights missing) | `aa-anim.md` Gaps, `aa-cry.md` Known gaps, `aa-world.md` §6 | med | M | character-lighting calibration |
| C2 | char select / create | Hat / beard variants: beard meshes absent in the pak for ferre and orc (only dwarf has one), no automatic voice lip sync, appearance-change rebuilding of remote / NPC units | `aa-anim.md` Gaps, `aa-client-units.md` | low | S to M | none |
| C3 | char select | Order of the hair / hair-colour grids and preset order not verified against the screen | `aa-anim.md` Gaps | low | S | a reference capture |
| CR1 | create | Step 3 field: no wind, no dummy animation; `elf_fog_01` is in no particle library of the pak (WARN logged each time) | `aa-client-create.md`, `aa-fx.md:362`, log-sweep #4 | low | S | substitute effect |
| CR2 | create / perf | Step 3 runs 34 to 76 fps; GPU bound, the stage camera re-filters the environment cube map every frame (`GeneratedEnvironmentMapLight`) | `research/perf.md` "Character create, step 3" | med | S | aa-cry `stage::environment_light` |
| CR3 | create | In-game re-customize / beauty salon (`EnterBeautyShop` etc. are stubs; server has `beauty_salon.rs`) | HUD sweep; opcodes 0x12c to 0x12e, 0x1f1, 0x1f3 | low | M | G* wiring |

### 4.3 Loading and screen lifecycle

**RE spec.** None. **Next step.** `LD1`: persistent pipeline cache, loading screen for `--enter-world`, interruption/race coverage.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| LD1 | loading | No persistent pipeline cache: frame 2 of a cold entry takes 0.1 to 2.8 s; `--enter-world` (test path) has no loading screen or prewarm; interruption / race coverage thin | parity row "Loading", `research/perf.md`, log-sweep #2 | med | S to M | none |

### 4.4 World geometry

**RE spec.** None. **Next step.** `WR1` HLOD/impostors, `WR2` vertex colours, `WR5` terrain/decal material gaps, `WR6`
vegetation wind and FlowGraph entities, `WR8` live sub-zone registration.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| WR1 | world | Far-plane dissolve, mesh/sprite handoff and shared quads recovered. Olive patches fixed by retaining unsupported normal-mapped leaf-only meshes; 23 models retain sprites. Native brush mesh-pair distance dissolve/reversal/FOV guards recovered; the 0.3-second cvar has registration-only references. Sprite-lighting parity, broader profiling and unit distance policy remain open. | b2f2499/2bf76a0/54309c7; 272 tests, 348160 GPU pixels with zero coverage/mask mismatches; `aa-world.md` | med | L | native sprite atlas and perf (P1) |
| WR2 | world / cry | Illum preserves source vertex RGB and squares it after interpolation/composition. Forward punctual highlights and exact authored light specular ratios pass bindless/fallback/churn controls. Terrain forward per-layer gloss/normal/Phong is recovered, adding about 66 MB of arrays. Ordinary VERTALPHA/ALPHAGLOW main and masked prepasses are recovered for 169 submaterials (15 masks); 277 permutations are explicitly declined. Open: excluded alpha permutations, native opaque deferred/tiled shading, native shadows, punctual terrain specular and integrated profiling. | 56f179a/ab918ae/b690462/3498eeb/328d6a4; terrain 275 tests and 48 GPU controls; Illum punctual 94 checks and 6240 GPU samples max0.575 sRGB step; alpha 108 checks, 144 native cases, 3360 GPU samples and client nested-prepass controls; `aa-cry.md`, `aa-world.md` | med | M | native pass follow-up |
| WR5 | world | Terrain forward per-layer specular is recovered; native deferred/punctual terrain lighting, 4 missing diffuse maps, decal MRT normal / specular, voxel specular / atlases, portal and area lighting, procedural grass blades | `aa-world.md` §6, parity "World geometry" | low to med | M to L | none |
| WR6 | world | Vegetation bending / wind not applied; animated entities / FlowGraph / unsupported CGA clips; far clouds animation and calibration | `aa-cry.md`, parity | low | M | none |
| WR8 | world | Sub-zone queries pass native tests but are not auto-registered live | parity "World geometry / Environment" | low | S | none |

### 4.5 Environment

**RE spec.** None. **Next step.** `WR3` post effects, `WR4` water fidelity, `WR7` time-of-day overrides and the unchecked noon
plateau.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| WR3 | world | Post: sun shafts, glow buffer, eye adaptation, photofilter / blueshift, SSDO bounce (SSAO opt-in), cooler grade vs real | `aa-world.md` §6, `research/time-of-day.md` | med | M | none |
| WR4 | world | Waterfall type-2 UV motion and anchored opacity recovered (53910e9; 98 checks, 36 native matrices, 728 GPU samples). Authored WaterSurface underside and plain volume Outof recovered (227d2f3; 136 pixel controls, 5 crossing states). Source-qualified ocean underside refraction/Fresnel and cone-free post are also recovered (896a287;179 GPU controls and11 image identities). Open: waterfall DualMaterial/Custom/VertexDeform, exact sea colour/scatter/reflection and accepted-receiver foam depth, native near-clip/fog ordering, pond calibration/river darkness and FFT sea displacement. Qualified sea foam scalar lighting and camera fade pass native/GPU controls on both sides (b64f71e). Native water tessellation/crest support and lake/river caustics still require audit; the renderer DLL name does not establish capability | `aa-world.md` §6 | med | M | reference captures of lakes / sea |
| WR7 | world | Time of day: area / relic overrides, subject-qualification producer, 12:00 to 19:05 "noon plateau" never checked against the real client | `research/time-of-day.md`, `aa-world.md` §6 | med | S | real-client captures at several hours (`time set hour=H`) |

### 4.6 Character and animation rendering

**RE spec.** None. **Next step.** `CH1` shading/clipping/dye, `CH3` remote facial/attachment QA, CDF bindings and skinned render
slots; the graph player (2.1) is a prerequisite for the remaining clips.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| CH1 | characters | Costume specular / silver armour duller than captures; clothing clipping (the `Cut` hulls leave straddling triangles); dye only for the bake path (weapons, bows, non-default armour dye, dynamic UCC) | `aa-anim.md` Gaps, `aa-cry.md`, parity | med | M | character-lighting calibration |
| CH3 | units | Remote units: full-world facial / attachment QA, CDF entity bindings, skinned render slots | parity "Character and animation rendering" | low | M | none |

### 4.7 Skill progression

**RE spec.** None. **Next step.** `H18` ability change/reset and `X2Player` pay/cash/schedule getters; cooldown work is `CB3` in
3.3.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H18 | HUD | Ability change / reset costs fake (`X2Ability`), `X2Player` pay / cash / schedule getters fake | `fake_ingame.lua:790-806` | low | S | none |

### 4.8 Chat

**RE spec.** `re-social.md` (gaps 2-4; gap 1 is done, `5401aaa`/`4d49692`). **Next step.** `H13`: user chat channels,
notices/chat failures, send/render policy, send-size overflow, window position persistence, other-tab splitting, full command
set and moderation.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H13 | HUD | Native move/anchor/bound callbacks, covered resize borders, secondary-tab split/whole-window transfer and character-scoped metadata persistence recovered. Caret/slide preview, drag draw preparation and release-time reorder now pass 271 toolkit checks; hover does not mutate native rows. Actual drag/history/filter/reload and allocation-write failure checks pass. Open: broader class/geometry/hover/held-item/link paths and remaining send-size/commands/moderation audit. SetTabButtonAlpha raw storage/inactive native draw behavior is recovered (4b68eb9;285 checks). Emotes/bubbles remain implemented. | f51b3fb/389b6d1/9b3ae0c/f6f0929/f93a96c/6fed35a/9a618ee; `aa-ui.md`, `aa-client-hud.md` | med | S to M | native UI follow-up |

### 4.9 Party and raid

**RE spec.** `re-social.md` (gaps 5-6). **Next step.** `H14` overhead 3D glyphs and party indicators, the risky two-stage flow,
invite field order and loot-method 255; paired live loot settlement waits on `N1`.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H14 | HUD | Overhead 3D glyphs: raid markers, family / guild titles; party buff / combat indicators | `aa-client-hud.md`, `aa-ui.md` "Native 3D overhead marker audit" | med | S to M | aa-ui |

### 4.10 Social/community

**RE spec.** `re-social.md` (gaps 3, 7-9; gap 1 is done). **Next step.** `H17` live settlement/unblock/restart, 3D titles,
notices, expedition coverage and mail/trade/friends tails; guild web/siege controls are in `H3` (4.12), nation in 4.12.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H17 | HUD | Social live settlement (paired guild, family, mail, trade, raid loot, friends unblock / restart) never re-run after the lane stalled; 3D titles; nation | parity rows "Mail", "Social", "Party" | med | M | N1 |

### 4.11 Mail, trade and auctions

**RE spec.** `re-social.md` (gap 8). **Next step.** `H1`: live search/bid/buyout/put-up/cancel/settlement and escrow/restart,
`CSListMailContinue`, `withReceiver`, postage/return and partial-stack trade; paired mail/trade checks wait on `N1`.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H1 | HUD | Auction house. Stale first line: the sweep said "no HUD module, `X2Auction` is a stub", but the upstream auction UI merged in d627eec (parity row "Mail, trade and auctions"). Remaining: live search/bid/buyout/put-up/cancel/settlement, escrow/restart and the original chat notices | `aa-net.md`, parity row "Mail, trade and auctions" | high | M | N1 to validate |

### 4.12 Territory, siege and dominion

**RE spec.** None. **Next step.** `H3`: territory/siege/dominion/nation plus guild web and siege controls.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H3 | HUD | Territory / siege / dominion / nation: `X2Dominion` has 2 constant getters, `X2Nation` stub; guild web and siege controls | HUD sweep; `dominion.rs` unwired | med | L | none |

### 4.13 Battlefields

**RE spec.** None. **Next step.** `G5` tower defense/siege alerts/crime arrest/appearance gender, plus live matchmaking/QA, the
lane Pong/start timer-origin mismatch and protected native ordinary skill eligibility/casting (`CB4`). Queue errors and remaining
scheduled-event presentation are open.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| G5 | gameplay | Tower defense (0x16b to 0x16e), siege alerts (0x0ec to 0x0ef), crime arrest (0x170), appearance gender change (0x1f3) | opcodes | low | M | H3 |

### 4.14 Unit colours

**RE spec.** `re-unit-colors.md` (rules, `hud.dds` rows, passive-hostile evidence, Bevy fix; implemented in `a54eaa0`, no open
spec gaps). **Next step.** Capture the aggressive name colour (the red row scaled is assumed), check a friendly NPC of another
faction, and run one live side-by-side. No item ID.

### 4.15 Duels

**RE spec.** None. **Next step.** Retail placement of the countdown text (currently a system message) and the
`SCAreaChatBubble` 0x153 (type 543, ignored). `G1` is closed since the sweep: challenge/prompt/accept/refuse, `STARTED_DUEL`/`ENDED_DUEL`
and the countdown are in the parity row; the server now sends `SCDuelStartCountdownPacket` at acceptance (NickMesser/AAEmu#15).

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| G1 | gameplay | Duels: closed as an open item (server real, client now has the packets and UI). Historical text was "server real (`duel.rs`); client has no packets or UI (`SCDuel*` 0x08b to 0x08f, `ChallengeDuel` 0x050)" | server grep; aa-net grep; parity row "Duels" | med | closed | none |

### 4.16 Achievements, ranking and trials

**RE spec.** None. **Next step.** English summary/description data, live achievement save/re-entry, ranking reward-mail
formatter/live settlement, the original nonempty bounty asset defect/payment lane gaps, trial spatial strings and mixed-subsystem
ordering, crime reporting/prison and racing flows, and paired live crime/jury settlement. No item IDs.

### 4.17 Music and other special systems

**RE spec.** None. **Next step.** `A1`/`A2` audio gaps, `H15` bard/composition, `H4` in-game shop/premium, `H16` UCC/books/
common farm/racing/second password/web/tutorial.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| A1 | audio | Voice-over gaps: `x2voice_prologue` has no FEV (client-driven indun voices), 59 absent events and 41 absent FEVs play nothing, as in the original client (same pak; nothing to substitute); one CELT subsound (left silent) | `research/audio.md` §5.4, `aa-audio.md` | low | S (CELT L) | none |
| A2 | audio | Modulation, doppler, spawning/playlists, queries and long-loop streaming recovered; native envelope flags/failed-voice cleanup fixed. Quietest policies 3/5 now use native audibility/admission/pool ordering; 20ms steal ramp and documented gain bridges remain approximations. Authored 3D pan and five proven log-mapped high-pass roots are recovered (7d0281b/8d97fe4; 93 device-inclusive tests and native/captured-mixer controls). Open: unsupported high-pass topologies, reverb/time curves, auto-pitch, seek/sustain, mood pitch/EQ/occlusion, remaining client triggers/live zone crossfades. One authoring-test oscillator exists. | fe13c6c/88afe6c/9ec84a2/fa43393; 73 device-inclusive checks, 13 native selection cases on both paths plus 6 slot cases; `aa-audio.md`, `aa-client-sound.md` | low to med | M | aa-audio/client follow-up |
| H4 | HUD | In-game shop, premium service, cash points: `X2InGameShop` stub, `X2PremiumService` fixed getters; server has `cash_shop.rs` (AA-point wallet is a server gap) | HUD sweep; opcodes 0x1cf to 0x1d8 | med | M | server wallet |
| H15 | HUD | Native composition, world/low32 persistent score cache, query-time bag/action/item-link tips, receipt-ordered Started93/94 and Fired97, tracked own/foreign PCM and own natural preview/performance End are implemented.25 focused checks include real device/DLS and original UI. Native piano/drum overrides use receipt-local buff lists with startup/decode-gap handling. Open: placed-instrument interaction, exact RNG/FMOD/rolloff/master output, channel limits and live settlement. Server PR44 fixes native uncached note replies; not deployed to existing bevy. | f8ce9b0/e22a0c0/51965d5 and root integration; `aa-client-music.md` | low | M | own-lane live QA and remaining native gates |
| H16 | HUD | UCC / crest / books (`X2Ucc`, `X2Book`), common / public farm, racing, second password, web browser, tutorial (one canned page) | HUD sweep | low | M | server (racing, second password have no server types) |

### 4.18 Options and UI toolkit

**RE spec.** `re-uitoolkit.md`: gaps 1-3 are done (`ad80b3a`, `22a19b7`, `593c2c5`); gaps 4-8 are open (option/keybinding
storage format and profile import, restart options, mouse-sensitivity range, options with no consumer, `Reset` semantics).
**Next step.** `H12`: remaining quality knobs, live draw-distance change, key-binding blob/character sync, remaining action
consumers and layout persistence, plus `re-uitoolkit.md` gaps 4-8.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| H12 | HUD | Options: texture / terrain detail / character / water quality and DoF have no renderer knob, loaded object-distance refresh is implemented (d26c359; 278 checks), native global far clipping remains open, key-binding blob and character sync, remaining action consumers, layout persistence | parity row "Options" | med | M | aa-ui |

## 5. Other open items

### 5.1 Live validation and network

`N1` gates every live check in the area entries (auction settlement tail/reconnect, mail/trade, guild/family, battlefield,
cooldowns). Once the lane is confirmed, run the offline/fixture items live, then territory/siege/tower defense. `N3` is
cross-cutting per-system decode work; `N6` is the stale network doc.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| N1 | network | Live-QA tail after the 2026-10-18 lane-server stall, which no longer reproduces: auction settlement tail/reconnect and the final social unblock/offline/restart checks are still unverified | 2026-10-20 headless smoke run (login, enter world, titles, tooltip, Character Info, emotes, guild window); `aa-client-parity.md` "Verification baseline" | med | S | none |
| N3 | network | About 90 server packets still `ServerMsg::Unhandled`; one that fails inside a `DD04` batch ends the whole batch (risk of losing later packets). Per-system lists live in the area entries above | `aa-net/src/game.rs` `decode_game`; sc_sent.json diff | med | M (spread) | per system |
| N6 | tooling | `aa-net.md` "Known gaps" is stale (says `SCBuffUpdated`, slave and mate packets are not decoded; they are) | `aa-net/src/game.rs` grep, commits 52bc34b, f14693e | low | S | none |

### 5.2 Tooling, docs and portability

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| L2 | tooling / login | Client runs only on this box: game DB read from `D:/aaemu-live/rs-game/Data/compact.sqlite3` (the client's own copy is encrypted), pak at `D:\AA1.2`; log shows "6325 UI texts from D:/aaemu-live/..." | `aa-anim/src/db.rs:29`, `aa-ui/examples/fake/mod.rs:192`, log-sweep #14, run | med (for anyone else) | M | decrypt the client DB, or ship an export |
| H19 | tooling | No run-time report of which stubs / fakes the scripts hit (`ui.stub_calls()` only printed by the fake example) | HUD sweep: `aa-ui/src/lib.rs:192` | low | S | none |
| T1 | tooling | Reference gaps: no `ingame/worldmap_*` check in docs, `combat_kick`, `combat_death`, tooltips, Palace each mentioned once; no captures at all for skill book, quest log, mail, party, crafting, friends, mounts, ships, housing, weather, night. Real client (`archeage.exe` pid 48924) is up | `reference/`, grep of docs | med | S per set | N1 (lane) |
| T2 | tooling | Docs are 2.6 MB (`aa-net.md` 714 KB, `aa-ui.md` 464 KB, `aa-world.md` 420 KB) and gap lists are stale in places; no single status page besides the parity ledger | `wc -c docs` | low | S | none |

### 5.3 Performance

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| P1 | perf | Town fps: Marianople 38 fps (40k entities, 19k mesh entities, 836 mesh assets rewritten per frame by aa-fx, `allocate_and_free_meshes` 8 ms). One fix landed (77d3f01, invisible emitters skip rewrites); no re-measurement. Frame is render-thread bound, GPU 3 to 4 ms idle | `research/perf.md` (draft), commit 77d3f01 | high | M | aa-fx, aa-world |
| P2 | perf | Entity / draw-call count: merge static part entities, cull earlier; `CryAssets::load_model` is main-thread only (prepare / finish split requested) | `research/perf.md`, `aa-world.md` §7 | med | L | aa-cry |
| P3 | perf | 3 to 10 fps stalls when other processes load the CPU (cause found: scheduler hand-offs; above-normal priority class added in `priority.rs`). Not re-verified on a quiet machine; no memory / VRAM budget measured anywhere | `research/perf.md` | med | S | none |

### 5.4 Misc gameplay

`G7` spans systems instead of belonging to one row. `H6` (appellations/titles) has no natural ledger entry.

| ID | Area | Item | Evidence | Vis | Size | Depends on |
|---|---|---|---|---|---|---|
| G7 | gameplay | Misc small: dice (0x134), impulse / knockback visuals (0x079), idle status, visual options, PvP point change (0x204), `SCPlaySound` (0x1de), item look convert, lobby refresh (0x1cd) | opcodes | low | S each | none |
| H6 | HUD | Appellations (titles): `GetAppellationCount` fake 0 in the character window; `SCAppellations` etc. undecoded and `ChangeAppellation` unsent | `fake_ingame.lua`; opcodes 0x1a4 to 0x1a6 | med | S to M | none |

### 5.5 Uncommitted or abandoned work (U1-U8)

The 2026-10-18 table is closed in this worktree: `git status` shows only the untracked `docs/research/re-mounts-full.md`
(committed with this docs change). Historical entries:

| ID | Former item | State now |
|---|---|---|
| U1 | Specialty-quote tail (`hud/data.rs` specialties, `hud/enchant.rs` equipment) | committed (d612b06 and later specialty work) |
| U2 | Character-lighting calibration (`aa-anim/src/character.rs`, `aa-cry/src/stage.rs`, `shade.rs`) | landed in the stage/skin commits |
| U3 | World post/sky/water tuning (`post.wesl`, `sky.rs`, `water_material.rs`, `examples/fly.rs`) | NaN guard/water knobs landed (76af157 and later) |
| U4 | `aa-audio/src/reverb.rs` ignored cost test | committed (5883e6f) |
| U5 | Frame recorder (`aa-dev/record.rs`) and combat video tooling | closed in this worktree |
| U6 | Labor delta (`aa-net/src/state/stats.rs`) | committed (4903cea, a9d96df) |
| U7 | `docs/research/perf.md` placeholder draft | committed without placeholders (25f62dc) |
| U8 | `target/agents/*` and `target/debug/aa-client-2026*.exe` build pile | **still open**: prune by the owners (not in git) |

Not abandoned but worth a look: the `.bashrc` of this machine begins with a UTF-16 BOM and prints an error on every Bash call
(`$'\377\376export': command not found`); harmless but noisy for every agent.
