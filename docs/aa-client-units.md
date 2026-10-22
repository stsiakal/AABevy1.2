# aa-client: other units (NPCs, mobs, players, mounts)

How the units the server announces move and animate in game. Code: `crates/aa-client/src/ingame/units.rs` (spawn, per-frame
movement), `remote.rs` (the pure interpolation, unit-tested), `creature.rs` (creature rigs and clips), `idle.rs` (combat stance
and NPC poses), `mount.rs` (riders). The wire side (what each packet carries) is in `docs/aa-net.md`, "Other units' motion".

## What the server sends

* Every moving NPC or mob sends a `SCOneUnitMovement` every 100 ms. Its `velocity` is a fixed 4000 (3.9 m/s) whatever the pace,
  so the **speed is measured from positions**, not read from the packet. `alertness` 2 or the IN_COMBAT flag means the unit
  fights. `actor_flags` 4 = running on the ground, 5 = walking.
* Players send their real velocity and the pressed direction (`delta_movement`); a jump or fall has no solid-ground flag.
* A rider gets no movement of its own: the server moves it with the mount (`SCUnitAttached`).
* `SCUnitMovements` is flattened by `aa-net` into one `Event::UnitMoved` per entry, so everything below sees a single stream.

## Movement (`units::receive_moves`, `move_units`)

`receive_moves` turns each `UnitMoved` into a `remote::Report` (position, yaw, the sender's `server_ms`, airborne, combat) and
pushes it into the unit's `remote::Track`. `move_units` samples the track every frame:

Death freezes the displayed track at the corpse; resurrection clears it and uses the packet's position and facing
immediately, even when no movement follows. Ship steering requests carry no position and never enter interpolation.

* **Snapshot interpolation.** The track keeps the last 8 reports and plays them back `delay` seconds behind the newest
  one: `delay = clamp(1.25 * report gap, 0.08, 0.35)`, the gap measured with the sender's `server_ms`, so the arrival jitter of
  the network never shows. A playback clock runs at `clamp(1 + error, 0.5, 1.5)` times real time to stay near that delay, and
  jumps when it is off by more than 0.6 s.
* **Extrapolation** carries on for 0.3 s when a report is late, then holds still.
* A silence longer than 0.5 s inserts a hold snapshot (the unit stood; it does not glide to the next position). A move of more
  than 12 m clears the track (teleport, respawn).
* **Heading.** The unit turns toward its direction of travel at 8 rad/s (`TURN_RATE`) when it moves, and takes the server's yaw
  when it stands. Players strafe: `remote::local_dir(heading, yaw)` is the direction of travel in the unit's own frame, which
  picks the forward, backward and strafe clips of `drive_locomotion`; NPCs always run forward.
* **Ground.** The server's z is replaced by `TerrainQuery::height_at` when it is within 1 m of it and the unit is not airborne.
  A jumping or falling player keeps the server's z.
* `Motion { speed, local_dir, airborne }` is written for `anim::drive_locomotion`, which owns the walk, run, jump and fall clips.
  The transform is touched only when the pose changed (a standing unit costs nothing).

## Clips

Creatures (`creature.rs`) use the `.cal` of their model; the sets are the `fist_*` ones (`fist_ba_relaxed_idle`,
`fist_mo_normal_walk_f`, `fist_mo_normal_run_f`, strafes through `global_monster_strafe`). Humanoid NPCs and players use the
character `.cal` (`ba_relaxed_idle`, `mo_normal_*`, jump and fall clips for players).

* **Combat stance** (`idle::IdleClips::combat`): a creature that stands while the server reports it in combat plays
  `fist_ba_combat_idle`; moving, it keeps the walk or run clip of locomotion.
* **NPC poses** (`IdleClips::pose`): `Unit::anim_action` (from `SCUnitState` or `SCUnitModelPostureChanged`) names an
  `anim_actions` row; its `anim_name` is the idle the NPC stands in (`fist_pos_stn_armor_dealer_idle`,
  `fist_pos_sit_chair_rest_idle`, ...). `idle::posture_changes` rebinds the clip when the server changes it.
* No turn-in-place clips exist for humanoids and most creatures in the 1.2 data, so a unit turns without one.

## Mounts (`mount.rs`)

A unit with `Unit::attached_to` set (`SCUnitAttached`) becomes a `Rider`:

* the mount is another unit, drawn like any creature (model, gait clips by its own speed);
* the rider sits on the mount's `bone_spine_driver` (attach point 1) or `bone_spine_passenger`, faces the mount's facing, and
  plays the rider clips of the mount's kind: `models.mount_pose_id` 1 horse, 2 lion, 3 bear, 4 wildboar, 10 pangolin give
  `horse_ba_relaxed_idle`, `horse_mo_normal_walk_f`, `horse_mo_normal_run_f` (and the other prefixes). The gait is the mount's
  PseudoSpeed, copied from the mount every frame as the original does (`FUN_390dcc80`): PseudoSpeed is the discrete
  requested urgency of the movement request (confirmed, `FUN_39404d50` / `FUN_3906a7f0`): 0 idle or under 0.5 m/s, 1.0 moving, 0.4
  walking, 1.5 under auto-move (`mount_anim::pseudo_speed`; the walk modifier of a remote mate is told from its speed), rider idle up to 0.1, walk 0.15 to 0.6,
  run from 0.7 (the gaps hold the band), sprint tier (`horse_mo_normal_sprint_f`) from 1.25, reachable only through the 1.5 of auto-move (how else a mount gets a value above 1 is open), `..._run_f_lturn/rturn`
  while the mount turns faster than 34.4 deg/s (0.6 rad/s, inferred; `RequestedTurnSpeedLZ` is in degrees/s), `horse_mo_relaxed_idletorun_f` / `runtoidle_f` between idle and run,
  `horse_mo_jump_f_start/loop/land` while the mount is airborne, and the idle while the mount swims. The tempo is copied from
  the mount's animator. The bear set has no mounted walk/run, so it keeps its idle (`mount::RideAnim`, `mount_anim::rider_band`);
* **Get on**: the side comes from the `reason` byte of `SCUnitAttached` (`mount::AttachReasons`, 1 left, 2 right, 3 back, 4
  front, anything else left). The rider is bound to the seat bone as soon as the mount is confirmed and plays
  `fist_ac_<kind>_<l|r|b|f>_geton` (1.0 s); the clip is entity-controlled in the original (no root motion reaches the entity), so
  the pose starting 1 to 2 m beside the horse is the clip's own skeletal motion. A rider already seated when it came into view has no reason and plays no clip. Other
  clients play the same side clip from the same byte.
* **Get off** (`SCUnitDetached`): `fist_ac_<kind>_l_getoff_start`, `_loop`, `_end` as a hop (`mount_anim::Hop`): the rider stays bound
  to the saddle while the clips play their skeletal motion, and the unit stands where the last clip left the body (about 1 m to
  the mount's left, on the ground); the on-foot `Locomotion` is held in a `Dismounting` component until then. The local player
  uses the same clips through `LocalRider` (`mount.rs`), seated at the same bones;
* a mate's own clips follow the PseudoSpeed bands of `horse_full_body.xml` (`mount_anim::MateGait`, `anim::drive_locomotion`):
  idle up to 0.1, walk 0.11 to 0.7, run from 0.8, the sprint tier from 1.25, turn in place (`fist_mo_relaxed_turn_l/r`) and the run
  turn blend (`fist_mo_normal_run_f_lturn/rturn`) from the turn rate, `fist_mo_relaxed_idletorun_f` / `runtoidle_f` transitions,
  `fist_mo_jump_f_start/loop/land` for jumps;
* the local player's seat comes from `aa_net::state::Player::apply` keeping the server's `UnitAttached`/`UnitDetached` for its
  own object on `PlayerSpawn.unit.attached_to` (the local player is not in `Units`). `control_player` freezes the on-foot
  controller while attached and during the get-off clip.

The server has no command that summons a mount, so the riders are checked with `AA_FAKE_MOUNT=1`: a horse (model 139) and a
rider that looks like the local player appear next to the player and run a circle at 5 m/s.
The offset between the seat bone and the rider's origin is still a first guess, not checked against the real client. A capture
that would settle it: the original 1.2 client mounted on a known horse from a fixed side and rear camera, with the mount and
rider world transforms, so the rider root's offset from `bone_spine_driver` can be read off the same frame.

Mounted locomotion (`mount::drive_mount`): while the local player is the driver (attach point 1) the on-foot controller is
frozen and the same keys drive the mate. `CSMoveUnit` names the mate object id (`aa_net::Command::MoveUnit`; the server's
`apply_unit_move` accepts an owned mate and runs `on_mate_unit_move`), at 0.1 s intervals, with the ordinary unit body
(position, yaw, velocity, `delta_movement`, ground/jump/fall and `stance::SWIM`). Walk/run come from the mate model's first
`game_stances` row (`ai_move_speed_walk` / `ai_move_speed_run`), the body radius/height, jump height, gravity and turn rate
from `actor_models` (`Db::mount_move`); `.`/Shift walk, NumLock auto-run, A/D turn the camera and the mount, both mouse
buttons run, Space jumps, terrain/brush collision slides and falls use the same `aa-world` queries as the player. Deep water
floats the mount at the surface at 45 % of its run speed. The camera follows the seated rider, which `pose_local_riders` keeps
on the seat; `Motion` drives the mount's own walk/run/jump clips and the rider's gait. A dismount removes `DrivenMount`, reseeds
the mount's received track and hands control back to the character at the seat position. Mounts do not dive; there is no
native seat-bone offset oracle yet (see the capture note below). Tests: `mount::tests` check the horse walk/run/swim
speeds, a forward input packet naming the mate object id with the expected velocity, and the driver-seat gate that restores
on-foot control after dismount; `aa-net` covers the `MoveUnit` body. Full client suite 418 passed / 39 ignored, `aa-net`
library 632 passed (including the mate-cast position target).

Mounting is not immediate (`mount::MountFlow`, the original `MountHorse Controller`): the pet bar's mount command checks 25 m, the
shared two-second cooldown and a line of sight (blocked: the text of system message 0x6b, no walk), then the controller walks the
player to the mate's side (any key cancels it, half speed inside 1.5 m, 10 s lifetime) and sends `CSMountMate` once the player is
within 3 m, with the reason byte computed at that moment, then waits 2 s for the attach. While riding the own mate the dynamic action
bar (F) holds `command_unmount` (`icon_skill_vehicle03`, `CSUnMountMate` reason 9). The driven mount's speed follows `game_inertia`
(slowing down) and `game_inertia_accel` (speeding up) of its actor model, 4 and 11 for the horse; which of the two applies when is
inferred. Not read from the original: the approach point of the auto-walk (the controller heads for 1 m beside the saddle on the player's
side), the line-of-sight mask (terrain and structures) and the exit point of the hop.

The owned mount/dismiss flow is in `hud/pets/owned.rs`: `X2Mate:CanMount`/`CanDismiss` follow the original controller gates
(25 m, shared two-second cooldown, casting/channel and pending action), `MountMate`/`UnMountMate`/`DismissPet` send the proven
`CSMountMate`/`CSUnMountMate`/`CSRemoveMate`, and a mounted dismiss first unmounts with reason 10 then removes after the
server's detach (native `39362070`). The mount reason byte follows the decompiled native `FUN_3909ae90`: the player→mate
direction against the **mate's** forward gives back 3 (with the passenger seat taken: 1) / front 4 (driver seat taken: 1), the sides
1 or 2 by the cross sign with no occupancy override (`mount_anim::entry_reason`). `ISLOT_PET_ACTION` clicks reach `use_pet_action`, which sends a mate-cast `CSStartSkill` with the mate as the
self target and the player's selection for unit-target skills; position-targeted mount skills send the mate-caster position
target (`UseMateSkillAt`) at the selected target's spot, or the mate's own spot for a source-position row. `X2Mate:GetSpeedInfo`
returns the last received movement report's horizontal speed in m/s, or
no values without a pet.

## What is drawn (the census)

Units are spawned within `UNIT_RADIUS` (140 m) of the player, in a time budget per frame (models are prepared on the task pool,
`prepare.rs`). The `in game:` log line ends with
`not drawn: N beyond 140 m, M without a model, K waiting`, and a unit whose model cannot be built logs
`warn: unit <id> (<name>) is not drawn: <reason>` once. At the Nuian spawn the "13 of 18" was all of the 5 units beyond 140 m;
no model failed. The set of units grows as the player walks, as the server announces units around it.

## The second figure in `shots/gallery/25_spawn_night.png`

It is a real other player (another agent session logged in on the same lane server standing at the spawn), announced with its
own object id; it is not a duplicate of the local player and not a ghost. The reproduction with a second account showed the
same picture. `aa-net` also drops an `SCUnitState` that carries the local player's own object id, so that one could never draw a
second figure.

## Land vehicles (`vehicle.rs`)

Owned carts, cars and tractors are slaves (`SCSlaveCreated` 061 -> `Slaves::own`, already decoded in
`aa-net/src/slave.rs`). The vehicle item is used like any item: its `items.use_skill_id` is sent as a
`CSStartSkill` item caster (`SpawnSlave` special effect 60 on the server) and the existing cast bar /
`SCErrorMsg` path applies. When the owner's creation record and visible `UnitKind::Slave` correlate,
`vehicle_seat` sends `CSBindSlave` once (`SlaveCommand::Bind`); leaving uses `CSDiscardSlave`
(`SlaveCommand::Unbind`, bound to the primary interaction key while seated). `SlaveCommand` already
had the exact u16 TL bodies; no new server-side request was invented.

With the player in the driver seat (`UnitKind::Slave` attachment point 1, kept on
`PlayerSpawn.unit.attached_to` by aa-net), `drive_vehicle` drives the slave: W/S throttle, A/D steer,
mouse heading; speed and turn come from `vehicle_models.velocity` / `angVel` (`Db::vehicle_body`),
with acceleration/braking, terrain-following `floor_at`, and world-position collision sliding. Every
0.1 s it sends `Command::MoveVehicle` -> `CSMoveUnit` type 2 for the **slave object id** with the
native `VehicleMove` body (position, velocity, quaternion shadows, angular velocity, steering, wheel
speeds); aa-net's `Movement` now has a generic vehicle writer and reader. The server's
`apply_unit_move`/`vehicle_move` accepts the owned slave, moves it and broadcasts to observers; the
seated player is positioned by `pose_local_riders` at the vehicle root + 1.1 m (the authored seat
bone is not mapped yet), so the camera follows the vehicle. Jump/fall are not sent because the
native vehicle body has no such fields; ships use the separate type-3/4 bodies (below).

The original mode action bar (`hud/mode_action_bar/mode_action.alb`) is bounded by
`X2Unit:GetModeActionsCount()` and establishes `ISLOT_MODE_ACTION` slots. `hud/vehicles.rs` loads
`slave_mount_skills` in table order, resolves each to its `mount_skills` combat row, filters by the
driver seat (attach point 1), and sends the mate-caster `UseMateSkill`/`UseMateSkillAt` for the slave
object id — the same mount-caster contract the server's `mount_authority` accepts for owned slaves.
The slave bar is the original mode bar; no invented window. Validation: `ingame::vehicle` checks the
mate-object vehicle packet/velocity and the one-shot visible slave bind; `hud::vehicles` checks seat
filtering and self/hostile/position mode skills; aa-net checks the exact type-2 body and session
send. Full client suite 427 passed / 39 ignored, aa-net library 636 passed.

## Ships (`ship.rs`)

The four 1.2 ship families (rowboat, clipper, merchant ship, fishing boat) use the same slave
lifecycle as vehicles: `item_summon_slaves` maps the summon item's `use_skill_id` (15802,
`target_type_id` 12 `SummonPos`) to a slave whose `models.sub_type` is `ShipModel`. The HUD marks
those item templates (`GameData::is_ship_summon`), so `ground_target` turns the cursor into the
water-surface reticle only for them; `WaterQuery` supplies the surface height over the sea bed the
ray hits, and a dry or out-of-range spot is not sent. The aim is the view ray's crossing of the water
plane (`surface_along_ray`, 1 m steps over `WaterQuery`, bisected), so open sea (no sea bed within the
400 m reach) gets a spot too; a shore or dock hit before the crossing falls back to the surface over
that ground. The server currently places a summoned slave
in front of its owner (`slaves::create_from_skill` drops the cast position), so the chosen shoreline
point is validated by the client but not used by this lane.

A ship is drawn from the brushes of its `ship_models.normal` prefab (`Db::ship_parts`, `UnitBuild::Ship`):
hull (`galleon_body.cgf`), rudder, cannons, cargo and shields, each at the prefab's `Pos`/`Rotate`
(`w,x,y,z`)/`Scale` under the ship root, so every `ShipModel` slave resolves to a body (the prefab name
is matched case-insensitively; 512 `galleon02`, 1386 and 1402, the sunken wrecks, name prefabs the
1.2 pak does not contain and stay undrawn). `actor_model_file` returns the hull brush for them. The
sails of the larger ships are separate slave doodads the server binds, not part of the prefab. A
ship has no skeleton, so it never plays a swim or idle clip. The brush transform is the world's: `Rotate` is
`w,x,y,z`, the position goes through `cry_to_bevy` and the rotation through `cry_rot_to_bevy(q) * CRY_TO_BEVY`: the loaded
model's root carries `CRY_TO_BEVY` (its mesh is in cry space) and the brush transform replaces it, so it must contain it (a
brush transform without it drew every hull standing on its bow); the scale stays on the mesh's cry axes
(`ship_part_transform`, tested; `cargo run -p aa-client --example ship_hull` with `--offscreen-shot` renders a ship's brushes,
`SHIP_MODEL`, `SHIP_VIEW`, `SHIP_OLD=1` for the old transform). The hull body spans z -5.8 .. 41.8 and y -17 .. 28.5 around the root (the galleon's main
deck is at z 2.5, the poop and helm chair at 6.3), and the server puts the summoned hull's root 7 m in front of its
owner, so a swimmer at the summon spot is inside the hull volume (seeing the deck from below). Creatures, mounts and
remote player characters that are not driven
swim in water deeper than 1.3 m (and not above the surface: a unit on a deck never swims) with `fist_ba_idle_swim` / `fist_mo_swim_f` when their `.cal` has them
(`Motion::swimming`); a driven mount does the same while its `DrivenMount` swims.

`vehicle_seat` binds an owned visible ship with the interaction key (no land-vehicle auto-board:
that would pull a character who only stands on deck into the helm); `CSDiscardSlave` leaves and
`CSDestroySlave` (`SlaveCommand::Dismiss`, secondary interaction key) is the explicit unsummon.
Reusing the summon item is the recall (the server saves and removes the active slave, then creates
it anew). Driver input is the native `ShipRequestMoveType`: `Command::SteerShip` writes
`CSMoveUnit` type 4 with throttle/steering in the server's signed 127 range
(`ship_physics.rs`), and the server's physics owns the hull pose and its type-3 broadcasts, which
`Movement::read_typed` now decodes (`Movement::ship` also has a type-3 writer for completeness).
Space stops/anchors the helm while seated; there is no separate 1.2 anchor skill in
`slave_mount_skills`.

`move_units` interpolates the hull; `seat`s come from the server's `slave_attach_points.json`
(rowboat point 1 has yaw 180; merchant/fishing helm points are at 1.56 m / 6.15 m), so local and
remote riders sit on the authored seat (`mount::seat_world`); the seat is a point of the hull's model
space (the Eznan Cutter's driver seat, z 6.2, is the deck beside the prefab's helm chair at z 6.3, tested),
so a seated player stands at the hull root plus that offset; a root far above the water (the cutter
summoned in a pond was at about z 145 over water at 127) is the server hull's position, not a seat error. A character who is not attached but
stands on the hull is `DeckRide`: `board_deck` takes the floor under its feet from `Db::ship_floor` (the upward-facing
triangles of the hull body brush in the ship's frame, 1 m cell buckets; `ShipFloor::height(x, y, max_z)` is the highest
surface not above `max_z`) and `deck_control` walks it in the ship frame: a step up to 0.6 m or down to 4 m is walkable,
a rail or the edge stops the walk, so the main deck and the poop are at their own heights and the player stays inside
the hull; a model without a readable hull keeps the old box (`ship_models.mass_box_size_x/y`, the helm's height). It
follows the interpolated hull and reports the resulting world position 10 times a second. The cargo, cannons and
rigging brushes are not floors. The server's `bind_slave_by_tl` always requests the driver point, so a
second attached passenger seat is not reachable through this lane; deck standing is the passenger
path. Slave equipment/cannon items exist in the database (`item_slave_equipments`) but the server's
`CSChangeSlaveEquipment` handler is a log-only stub, so no cannon spawning/firing path is sent.

A cannon (child slave, e.g. slave 10 on ship 22) is announced in its parent ship's frame (`-4.3, 2.2, 4.4`): `Units` keeps
`slave_local` for a slave whose unit state names another slave as its owner object and `resolve_children` puts it at the
parent's position plus the parent's heading applied to the local position (`slave_child_pose`); `follow_slave_parents`
repeats that every frame from the interpolated hull, so the cannon sails with the ship and the cannon bind range
(`CANNON_REACH` 4.5 m) uses its real place. The attach point's own offset is not applied. A Trade Pack Storage chest of a
ship (`doodad::TRADE_PACK_STORAGE_SKILL`) is only the F key's / the bar's use for a worn trade pack or a right clicked
chest, so F near an owned ship boards it; the dynamic action bar ignores its buttons while a ground skill is aimed
(`Aiming::active`).

## Env switches (dev)

* `AA_UNIT_SMOOTHNESS=1`: logs how the displayed speed of remote units compares with their path speed (mean and p5/p95 per
  group, npcs and players).
* `AA_FAKE_MOUNT=1`: the mount check above.
* `AA_UNIT_SHADOW_RADIUS`, `AA_ANIM_LOD`: shadow radius and animation levels of detail of units (performance pass).

## Gaps

* `SCBuffUpdated` is decoded (`crates/aa-net/src/game.rs`, layout in `docs/aa-net.md`), but the AAEmu server never sends it, so live stack/charge updates do not arrive; a buff refresh comes as `SCBuffCreated` with the same index.
* Combat idle is for creatures only; humanoid NPCs fight with the combat agent's clips.
* Mount and rider placement (the seat-bone offset) is unverified against the real client; the exact native seat/get-on
  reason for every mount kind and underwater mount motion remain open. Mounted locomotion, mount skills and the get-off clip
  now exist (see Mounts).
* Land vehicles: full suspension roll/pitch are not mapped (the client keeps a level yaw-only quaternion).
  Ships: the server drops the `SummonPos` target and always puts a summoned ship in front of its owner; its
  `bind_slave_by_tl` always requests the driver seat (no attached passenger path), and cannon equipment is a
  log-only stub. Ship deck walking is client-side frame following, not server support movement; the deck box
  comes from `ship_models`, not a walkable hull collision mesh. `SCUnitMovements` entries are all read as unit
  movements (the real client takes the type from the object).
* Units beyond 140 m are not drawn.

## Authored NPC facial playback (2026-10-18)

`creature::spawn` now prepares a supported CDF head before spawning its skin subsets and
puts the returned facial state and morph weights on the same root as the body Animator.
The head's own CAL must declare a facial library; this inexpensive shared-CAL check avoids
parsing every body/clothing part in a crowd as expression geometry. Unsupported or missing
head data keeps the existing ordinary skin path. Cached models remain unchanged, and each
prepared instance receives private mesh handles so its weights cannot affect another NPC.
The existing FacialPlugin follows the body's authored CAL-to-FSQ mapping.

`cargo build -p aa-client --example facial_creature` passed. The unattended example exercises
the actual client CDF loader with `npc_chef01_base.cdf` (bound clothing/face/hair), not the
blank customization slots in `npc_chef01.cdf`. Three independent instances use the original
`fist_ac_talk_11` CAF at 0, 1.7218 and 3 seconds. Runtime assertions verify 23 targets, the
body clock, private mesh handles, correct owner references, and one facial versus three
ordinary skin subsets per instance. Speech peaks at 0.6935022; the authored return retains
its small signed values, with positive peak 0.040279. The inspected capture is
`shots/aa-client/facial-creature/cdf-body-clock.png`, with 266–277 FPS after startup and no
GPU errors; the screenshot process exited normally.

This adds playback to supported bound NPC heads. The following milestone also handles rigid
bone attachments and explicit received body items. PC-shaped remote units and voice-driven
sequence selection remain separate work. No procedural mouth movement or voice lip sync is claimed.

## Received NPC body slots and rigid attachments (2026-10-18)

The creature preparation path applies explicit received Equipment slots19..25 to the original
actor CDF's matching declared skin slots. Each item must resolve to the same model and its
matching native body slot23..29. Empty or incompatible items preserve the authored definition.
The worker's model list and main-thread spawn share this effective CDF; neither substitutes
an `_base.cdf` or infers an outfit. This fixes blank actor CDF slots previously ignoring received
face/hair/body items. Initial spawn is covered; later appearance-change rebuilding is still open.

Declared visible CA_BONE bindings now load alongside skin parts, including material overrides
and CDF wrappers, and parent to the existing animated joints using aa-anim's source-proven
alignment helper. Bone-aligned attachments ignore XML absolute offsets; model-aligned ones
use inverse joint bind transforms. Skin-aligned projection remains unsupported. Missing joints
or unsupported bindings skip that attachment without breaking the body. The loader preserves
authored eye-path choices, including a duplicate left-eye path in the chef's actor CDF.

`facial_creature --packet` exercises the actual blank chef actor CDF with explicit original
face14478/hair14475/body14479 items, the same configuration helper and spawn path as production.
The inspected `shots/aa-client/facial-creature/packet-cdf-eyes.png` shows seated eyes, clothing,
hair and authored neutral/A/return poses. Each figure has one independent facial subset and
five ordinary subsets; body-clock and owner assertions pass. It measured223–256 FPS while the
HUD fixture also used the shared GPU, with no GPU errors and normal screenshot exit.
The client suite passes150 tests /24 ignored, including model/slot incompatibility and worker
model-list checks. This is offline visual/source verification; live lane game connections remain stalled.

## Selected remote character faces (2026-10-18)

The server-confirmed Target now opts one visible, awake remote Character into original facial
playback after normal unit culling. A target change, hidden/out-of-view target, parked Animator,
or removed entity detaches the previous private face before enabling the next. The queued mutation
rechecks visibility/awake identity. Unsupported heads are attempted only when eligibility changes,
not every frame. The local player's existing spawn-time face is independent. This is a bounded
client memory policy, not a claim of identical native crowd facial LOD.

The aa-anim in-place API preserves the actual customized face, material/skin components, bones,
movement/entity identity and cached ordinary mesh handles. It handles selection AFTER GPU upload
has released ordinary CPU vertices by rebuilding only the selected head with the same source model,
face_morph and aa-cry subset converter. Disabling restores ordinary handles and underlying body bone
poses, including a parked body; private meshes are released through normal asset lifetime.

The supporting aa-anim regression/capture enables at3s, disables at5s after private GPU upload and
captures at8s. It passed73 tests /1 ignored; facial_attach_toggle_final_20261018.png was inspected
by the owner at252-339 FPS, with no window left open. Root's main client check passes, and a
read-only integration review found no lifecycle blocker. Natural full-world target switching is
still awaiting the lane game-connection unblock. Same-entity appearance replacement will need an
explicit re-enable trigger once that pipeline is implemented.

Measured Nuian male morph payload is4,049,472 bytes per enabled head (about386 MiB for100), with
zero extra image/material/bindpose assets. The crowd audit therefore did not add a CPU cache that
would leave per-mesh GPU duplication intact. Its expression-update microbench excludes full ECS,
body and facial-bone costs; original32-head captures check independent poses and real GPU use.

## CDF particle and object attachments (2026-10-18)

The client now distinguishes object paths, named particle effects and entity bindings before
prewarming or spawning. Original CA_BONE objects and effects use the authored bone/local
transform. Creature CDFs and their skin-part CDFs retain these bindings; assembled Characters
use the attachments retained by aa-anim on each resolved costume/hair part. Named effects are
never mistaken for mesh filenames. The three source entity bindings remain unsupported.

Effect anchors follow the existing actor bones while retaining their actual source part as
owner. A hidden actor or independently hidden part releases the emitter; unhide lazily creates
it again. Removing a part releases its bone-parented attachment, and actor unload removes all
descendants. Rigid object attachments also follow source-part visibility and lifetime.

Native constructor31515900 uses scale1; lazy spawn31517A40 and update31517CC0 retain the
emitter pointer until hidden/destructed. The integration therefore uses scale1, no priming,
and no forced repeat. Native flag0x20000 passed through315170B0 remains unclassified and is
not represented as an invented priming/loop option. This does not claim full native particle
rendering/material parity.

The unattended cdf_attachments example uses the actual horse_lord.cdf four hoof effects and
the Elf female white-day costume head ring. Inspected native-bindings.png shows all five;
native-unload.png and its assertions confirm zero Effect/FxMesh entities after despawn.
The baseline measured218-298 FPS, and the hide/show/unload run279-412 FPS on the shared GPU,
with no render errors. Captures/logs are in shots/aa-client/cdf-attachments/. These are original
asset/lifetime checks in a controlled scene; same-scene real-client lighting comparison and
live full-world attachment validation remain open.

The fresh client suite passes156 tests /26 ignored (4.34s), including part/actor visibility,
emitter recreation, source-part destruction and named-effect exclusion from mesh prewarm.
