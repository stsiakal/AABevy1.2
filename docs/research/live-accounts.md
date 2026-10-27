# Live-test accounts and slots

Per-worker accounts on the shared golden lanes. Two workers never share an account: every slot
owns an `a` (hidden original) and a `b` (Bevy/second client) account **per lane**. The rows live in
the lanes' throwaway databases (`lane_move_golden_*` on 28xxx, `lane_combat_golden_*` on 29xxx,
private MySQL `127.0.0.1:3317`); the static reference characters are only clone sources.

## Slot table

Character id = `1001 + slot_index * 2 + role_index` on both lanes. `a` is the hidden original,
`b` is Bevy. Accounts are `<slot>a` / `<slot>b`; the password is the one in each lane's
`cap-pw.txt` (the same file the tools already pass to `aaemu-launcher`; never committed).

| Slot | Original (`a`) char | Second (`b`) char | move-golden spot (x, y, z), zone 182 | combat-golden spot (x, y, z), zone 179 |
|---|---|---|---|---|
| wp1 | MgWp1a / 1001 | MgWp1b / 1002 | 10512.0, 15585.0, 229.437 | 15528.8, 15335.7, 132.658 |
| wp2 | MgWp2a / 1003 | MgWp2b / 1004 | 10515.0, 15585.0, 229.437 | 15531.8, 15335.7, 132.658 |
| wp3 | MgWp3a / 1005 | MgWp3b / 1006 | 10518.0, 15585.0, 229.437 | 15534.8, 15335.7, 132.658 |
| wp4 | MgWp4a / 1007 | MgWp4b / 1008 | 10521.0, 15585.0, 229.437 | 15537.8, 15335.7, 132.658 |
| wp5 | MgWp5a / 1009 | MgWp5b / 1010 | 10524.0, 15585.0, 229.437 | 15540.8, 15335.7, 132.658 |
| wp6 | MgWp6a / 1011 | MgWp6b / 1012 | 10527.0, 15585.0, 229.437 | 15543.8, 15335.7, 132.658 |
| f7 | MgF7a / 1013 | MgF7b / 1014 | 10530.0, 15585.0, 229.437 | 15546.8, 15335.7, 132.658 |
| interaction | MgInteractiona / 1015 | MgInteractionb / 1016 | 10533.0, 15585.0, 229.437 | 15549.8, 15335.7, 132.658 |
| combat | MgCombata / 1017 | MgCombatb / 1018 | 10536.0, 15585.0, 229.437 | 15552.8, 15335.7, 132.658 |
| spare1 | MgSpare1a / 1019 | MgSpare1b / 1020 | 10539.0, 15585.0, 229.437 | 15555.8, 15335.7, 132.658 |
| spare2 | MgSpare2a / 1021 | MgSpare2b / 1022 | 10542.0, 15585.0, 229.437 | 15558.8, 15335.7, 132.658 |
| spare3 | MgSpare3a / 1023 | MgSpare3b / 1024 | 10545.0, 15585.0, 229.437 | 15561.8, 15335.7, 132.658 |

The combat-lane characters are `CgWp1a` … `CgSpare3b` with the same ids, level 30 (the move-lane
characters are level 1, like `Movegoldc`). Both roles of a slot stand 2 m apart; slots are 3 m
apart. Each character has the reference character's race (1), gender, three ability trees (1/11/11),
learned skills, action bar (the cloned `characters.slots` blob), equipment and bag items, plus one
live Brown Lilyut Horse.

Tools pick their accounts with `--slot` (or `FEEL_SLOT` / `PROBE_SLOT` / `COMBAT_SIDE_SLOT`):

```bash
python tools/feel-recorder/run.py --slot wp1 --scenario mounted
python tools/position-probe/probe.py --slot wp1 --lane move-golden lua <pid> "return 1"
python tools/combat-side/record_original.py --slot wp1 ...
```

`tools/live_slots.py` is the source of truth for the mapping; `tools/live-slots/provision.py`
provisions it (idempotent, replaces only ids 1001–1024, never the reference characters):

```bash
python tools/live-slots/provision.py --dry-run
python tools/live-slots/provision.py
```

## What the provisioning SQL does

`tools/live-slots/provision.py` writes and executes the following per lane and character (with the
lane's DB selected; the account password is the SHA-256 of the lane `cap-pw.txt`, stored in the
legacy Base64 format the login server verifies). It is the recorded SQL that was run on 2026-10-27
against `lane_move_golden_game` and `lane_combat_golden_game`:

```sql
-- account (once per user; id is auto-increment)
INSERT INTO users(username,password,email,last_ip,last_login,created_at,updated_at)
VALUES ('<slot><role>', '<legacy-base64 sha256 of cap-pw.txt>', '', '', NOW(), NOW(), NOW());

-- character: every column cloned from the lane reference character 3
INSERT INTO characters(<all columns>) SELECT <reference columns with id/account_id/name/level/
  hp/mp/x/y/z replaced> FROM characters WHERE id = 3;
-- move-golden: level 1, hp/mp 9999 (the load clamps to max), spot x+3*slot, y+2*role
-- combat-golden: level 30, same gear/skills/action bar, spot Goldtest + 3*slot, +2*role

-- item containers: the reference equipment/bag/bank containers, new ids
INSERT INTO item_containers(container_id,container_type,slot_type,container_size,owner_id,mate_id)
VALUES (<700000 + character_id*20 + n>, '<source type>', <slot_type>, <size>, <character_id>, 0);

-- equipment and bag items (except SummonMate), new ids
INSERT INTO items(<all columns>) VALUES (<20000000 + character_id*1000 + n>, ...);

-- exactly one live horse per character
INSERT INTO items(id,type,template_id,container_id,slot_type,slot,count,details,...)
VALUES (<next id>,'AAEmu.Game.Models.Game.Items.SummonMate',8159,<bag container>,2,10,1,
        UNHEX('DC0500000005'),...);   -- exp 1500, mate_dead 0, mate_level 5
INSERT INTO mates(id,item_id,name,xp,level,mileage,hp,mp,owner,updated_at,created_at)
VALUES (<item id>,<item id>,'Brown Lilyut Horse',1500,5,0,9999,9999,<character_id>,NOW(),NOW());

-- skills / abilities cloned from reference character 3
INSERT INTO skills(id,level,type,owner) SELECT ..., <character_id> FROM skills WHERE owner = 3;
INSERT INTO abilities(id,exp,owner) SELECT ..., <character_id> FROM abilities WHERE owner = 3;
```

## Registered fix: dead-flagged horses (2026-10-27)

The hidden original refuses to send `CSStartSkill` for a summon item whose `details` byte 4 is
`mate_dead = 1`; the server therefore never sees the cast and no pet exists. Evidence on
move-golden: `Movegolda` (char 1) item `16777258` and `Farmbevy` (char 8) item `16777345` had
`details = DC0500000105`; `Movegoldc` (char 3) had live `DC0500000005` and its cast produced
`StartSkill: Id 10602` in `rs-game.log`. Characters 1 and 8 also had `characters.hp = 0` and their
`mates` rows had `hp = 0`. This contradicts the comment at `mates.rs:836` ("the client never checks
the flag on its own").

Applied to `lane_move_golden_game` with the stack stopped, then every reference/slot horse was
(`tools/live-slots/provision.py` repeats it as `heal_existing`):

```sql
UPDATE items SET details = CONCAT(SUBSTRING(details,1,4), CHAR(0), SUBSTRING(details,6))
 WHERE type = 'AAEmu.Game.Models.Game.Items.SummonMate'
   AND template_id = 8159 AND ORD(SUBSTRING(details,5,1)) <> 0;
UPDATE mates SET hp = 9999, mp = 9999, updated_at = NOW() WHERE hp <= 0 OR mp <= 0;
UPDATE characters SET hp = 9999, mp = 9999 WHERE hp <= 0 OR mp <= 0;
```

`mate_dead = 0` + full mate pools is the state the original client accepts (`movegold3` worked
with a live item); the server's own corpse refusal at `mates.rs:836-853` is not reached for the
fixed rows. The new slot characters never need the repair: their horse is inserted live.

## Proof runs (2026-10-27, move-golden)

After provisioning, with the lane stack on 28xxx and every client hidden:

* `wp1a` (`MgWp1a`, character 1001) launched through the rebuilt lane driver, used the live horse in
  bag slot 11, mounted (`X2Mate:IsPlayerMounting() == true`, `GetSpeedInfo() == 9`) and galloped for
  3 s. `rs-game.log` shows `StartSkill: Id 10602` at 02:08:27 and the character moved on the client
  (server position changed as well). This is the first live proof of the dead-flag fix: the same
  account pair previously had `details = DC0500000105`.
* Two feel recorders ran at the same time on `wp2` (`MgWp2a`, pid 23436) and `wp3` (`MgWp3a`, pid
  45528), `--scenario mounted --only original --reuse-stack`, each with its own 8 m slot offset.
  Both logged in (server: `NotifyInGame: MgWp2a (1003)` / `MgWp3a (1005)`), both sent
  `StartSkill: Id 10602` (02:12:56 / 02:12:59), both mounted and galloped:
  `gallop_mean_mps = 10.22` (90 rows, `stride_period_ms = 540`) and `9.76` (102 rows, 551 ms).
  Output: `shots/feel-recorder/slot-wp2/`, `shots/feel-recorder/slot-wp3/`.
* The two accounts named in the bug report were then re-checked on the repaired rows:
  `clienttest` / `Movegolda` (char 1) sent `StartSkill: Id 10602` at 02:16:47 and galloped at
  `9.46 m/s` (181 rows, stride 560 ms); `movegold1` / `Farmbevy` (char 8) sent it at 02:20:06 and
  galloped at `9.96 m/s` (182 rows, stride 562 ms). Output:
  `shots/feel-recorder/try-movegolda/`, `shots/feel-recorder/try-farmbevy/`.
* Every client was stopped by pid after its check; no `archeage.exe` or `aa-client.exe` was left
  running.

Both lanes also carry a `cap-pw.txt` beside the runtime dirs (`D:/aaemu-live-move-golden/`,
`D:/aaemu-live-combat-golden/`); it is outside git and is the only password source the slot
scripts use.

## Driver build

The `D:/aaemu-target/move-golden/debug/aaemu-clientdriver.exe` found by the tools was built on
23 Oct, before the lane's current `aaemu-clientdriver` source (`a425ba2ef`, 25 Oct) and the 24 Oct
`aadriver.dll`; the working copies from `D:/aaemu-target/main/debug` were newer. The tools now use
the lane-built driver, so rebuild it from the lane whenever the lane source moves:

```bash
cd D:/aaemu-wt/move-golden/rust && cargo build -p aaemu-clientdriver   # debug for the tools
# or: Scripts/live-stack.sh build                                       # live profile
```

`Scripts/live-stack.sh start` copies the lane's live binaries into the runtime `bin`; the
`aadriver.dll` stays at `D:/aaemu-target/<lane>/aadriver/aadriver.dll` and is passed explicitly by
the tools (`--dll`). Do not mix a lane driver with another lane's DLL.

All four golden lanes were rebuilt this way on 2026-10-27 (`cargo clean -p aaemu-clientdriver`
first, because the checked-out source mtimes were older than the stale 23 Oct exe, so a plain
`cargo build` said "Finished" and kept the old binary):

| Lane | debug driver (tools) | live driver |
|---|---|---|
| move-golden | 27 Oct 01:56 | 27 Oct 01:40 |
| combat-golden | 27 Oct 01:40 | 27 Oct 01:40 |
| veh-golden | — | 27 Oct 01:40 |
| house-golden | — | 27 Oct 01:40 |

The rebuilt move-golden driver launched and injected `wp1a` successfully (the old `main/debug`
workaround is no longer needed).
