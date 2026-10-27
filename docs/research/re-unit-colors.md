# Unit colours: name plates, target frame, ring (1.2 client)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

Question: why is the aggressive "Four Fingers Bladeslinger" orange, and are all NPCs yellow?

## Original's rules

The target frame is Lua (`x2ui/unitframe/target.alb`, `unitframe.alb`); the colour is a row of the bar strip in
`ui/common/hud.dds` (`x2ui/baselib/statusbar_style.alb`, `STATUSBAR_STYLE.S_HP_*`, x 301, 150 x 19 each):

| Style | y | Bar pixel (measured) | After-image colour | Chosen when (`unitframe.alb` proto 0.2 / 0.3) |
|---|---|---|---|---|
| `S_HP_FRIENDLY` | 0 | (64, 127, 38) green | (134, 207, 82) | relation string `"friendly"` |
| `S_HP_PARTY` | 20 | (21, 115, 155) blue | (86, 198, 239) | party member |
| `S_HP_HOSTILE` | 40 | (154, 39, 39) **red** | (223, 69, 69) | relation string `"hostile"` |
| `S_HP_NEUTRAL` | 60 | (156, 76, 23) **orange** | (230, 141, 36) | relation string `"neutral"` |
| `S_HP_PREEMTIVE_STRIKE` | 100 | (121, 58, 58) dark red | (202, 110, 105) | `X2Unit:IsFirstHitByMeOrMyTeam('target')` on a hostile |

There is no yellow row: the strings come from the native `X2Unit:GetCombatRelationshipStr` (x2game
`FUN_394c0d80`, via `FUN_39028240` -> `FUN_39838880`, the relation of unit A to unit B). Its first byte is the
state 1 hostile / 2 neutral / 3 friendly, from the faction tables (`system_faction_relations`, mother factions) and
then unit flags (vtable calls 0x18, 0x34, 0x40 ... not all identified). Orange is state 2.

Evidence that a hostile NPC that does not attack on sight is state 2: `reference/ingame/combat_melee` (real client)
shows the Angry Rat (faction 115, hostile to Nuian 101, **`npcs.aggression = 'f'`**) with name (238, 107, 35) and bar
(146, 66, 21), the orange row (156, 76, 23) at the plate's shading. Aggressive NPCs (`aggression = 't'`) are the
red row. This matches the owner's rule: orange = passive hostile, red = aggressive.

The name plate itself is native (no colour constants in x2game; the plate's relation-to-colour table was not found in
the Lua or the DLL) and was taken from captures: yellow bar and name for friendly villagers (Maude, Malson:
`reference/ingame/spawn/real.png`, own faction), green for players, orange for the rat. The target ring is red for
every hostile, gold (230, 186, 40) for a friendly NPC, green for a friendly player (`hud/ring.rs`).

## Owner's claim "all NPCs yellow"

Only partly supported: the captured friendly NPCs are yellow on the plate, and the ring is gold for friendly NPCs.
Hostile NPCs are red/orange, and the target frame's bar has no yellow at all (friendly NPC = green row). Plates for
non-own-faction friendly NPCs: **not verified** (the capture only has own-faction villagers); they stay as before
(green plate for friendly NPC of another faction, yellow for own faction). A retail check of one such NPC would settle it.

## Data

Four Fingers Bladeslinger = npcs 7701 / 7693 (also 3478, 7697): level 9, faction 115 (Monsters), `aggression = 't'`.
Hiero's killers (7701, 3479 Wizard, 7693) are all aggressive. The server sends nothing about aggression: the unit
packet carries the faction only (relations hold: 109/101 vs 115 = state 1). The Bevy decode was right; the bug was
Bevy's colour logic, which had one hostile colour (the rat's orange, from the capture) for every hostile and never read
`npcs.aggression`. Not a server issue.

## Fix (Bevy)

`NpcInfo.aggression` (compact `npcs.aggression`), `plates::colors(relation, passive)`: aggressive hostile red (bar
(154, 39, 39) from the row, name = bar * 1.63 = (251, 64, 64), the factor of the rat's orange pair, **assumed**),
passive hostile unchanged orange. `plates::combat_relation_of` / `TargetPanel::combat_relation` make
`GetCombatRelationshipStr` return `"neutral"` for a passive hostile, so the frame uses the orange row.
Tests in `hud/plates.rs`.
