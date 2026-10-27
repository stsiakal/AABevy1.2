# Social systems (ArcheAge 1.2 client)

> **Restored 2026-10-27 from `archive/fork-main-2026-10-27`, UNVERIFIED.** The client was restarted from NickMesser's `main`; nothing described here is wired into the game. Addresses, call chains and offsets must be re-checked against the decompile ([R]/[I]/[?]) before use. A 2026-10-27 audit found 151 of the 269 EXACT/TOLERANCE labels in `proof-levels.*` weak or wrong (report: `D:\aaemu-client\handoff\2026-10-27\audit\report-proofs.md`).

## 1. Scope and sources

Chat (channels, commands, links, filters, whispers, bubbles); party and raid (invite, roles, frames,
loot, marks, pings); guild/expedition (roster, ranks, policy, invite); friends/blocks; family; mail;
player trade; their packets and UI.

Sources: `C:/AAEMU_DISK/aa-re/client12/out-x2game/x2game.dll.c` (VA in `FUN_<addr>`, image base
`0x38FF0000`); 1.2 UI bytecode `D:/aa-re/client12/ui/game/scriptsbin/x2ui/...` (`albtool.py`
`dis`/`strings`; `proto` retained Lua line numbers); `compact.sqlite3`; server layouts under
`C:/Users/stsiakal/AAEmu/rust/crates/aaemu-game/src`; Bevy under `D:/aaemu-client/AABevy1.2`.
`[R]` = read, `[I]` = inferred, `[?]` = not found.

## 2. The rules

### 2.1 Chat

**Pipeline.** Incoming `SCChatMessage` (`0x0ce`) is decoded by `FUN_391d8b00` (x2game.dll.c:366842):
`i16 chatType, i16 subType, u32 0, bc objId, u32 charId, u8 language, u8 race, u32 faction,
string name, string message, i32 ability, i32 option` (`packets/world.rs:696-731`). It fires
`OnChatMessage`; chat types are `-4 whispered, -3 whisper, -2 daily/system, -1 notice, 0 say, 1
zone/shout, 2 trade, 3 party-finder, 4 party, 5 raid, 6 faction/nation, 7 expedition, 8 system, 9
family, 10 raid-command, 11 trial, 13 performance, 14 race/coalition, 15 user`
(`x2ui/baselib/baselib.alb:116-150` [R]).

`SCNpcChatMessage 0x0cf` -> `FUN_391d8bb0`; `SCChatLocalizedMessage 0x0d2` -> `FUN_391d8c40`
(formats a template then dispatches like a chat line); `SCChatFailed 0x0d1` -> `FUN_391d8de0`
(`i32 code`, `string text`) which looks the code up in `FUN_3956b860`: `0x108 yell_too_often, 0x109
whisper_no_target, 0x10a said_forbidden_word, 0x10b chat_not_in_party, 0x10c chat_not_in_raid, 0x10d
chat_channel_already_exists, 0x10e chat_no_channel, 0x10f chat_already_joined_channel, 0x110
chat_private_channel, 0x111 chat_wrong_password` (x2game.dll.c:1136287-1136310). `SCNoticeMessage
0x0d0` -> `FUN_391d8d60` (`u8 kind, string alphaHex, i32 visTime, string RRGGBB+text`); a non-positive
visTime means `1000 + 50 ms/character` (`systems/services/shutdown.rs:20-62`). `SCChatSpamDelay 0x0d3`
-> `FUN_391d8cf0` copies a 0x68-byte config: 15 group bytes + 15 float delays; `FUN_393c2540` uses
`group[chatType]` and the matching delay, refusing with 0x108.

**User channels.** `X2Chat:CreateUserChatChannel(channel,password)`,
`JoinUserChatChannel(channel,password)`, `LeaveUserChatChannel(channel)` are bound in
`FUN_3945aa00` (x2game.dll.c:908837-908873). Join/create send `CSJoinUserChatChannelPacket 0x061`
(`string name, string pwd, bool create`, `systems/social/packets/chat.rs:16-27`); leave sends
`CSLeaveChatChannelPacket 0x062` (`i64 chat id`, :29-40). Names are capped at 0x31 bytes
(`FUN_393c63e0`:786853). Joined channels are tracked by `FUN_393c65c0`: type 1 registers say/zone/
trade, 6 faction, 11 trial, 14 family, 15 user (x2game.dll.c:786942-786990).

**Sending.** `X2Chat:SendChatMessage(channel,message,target)` -> `FUN_39458150` -> `FUN_393c5aa0`
(x2game.dll.c:786319). It drops empty text; checks the spam timer (`FUN_393c2540`, failure 0x108) and
the local filter (`FUN_393c22f0`, silent); converts UTF-8 to UTF-16, truncates at `DAT_39ac7ef8`
(<0x400), converts back and sends. Channel switch: 0 say, 4/5 party/raid, 6 faction, 7 expedition,
9 family, 10 raid-command, 11 trial, 13 performance (local music broadcast `FUN_39083960`), 14
coalition, -3 whisper (`FUN_393c58f0`). The wire is `CSSendChatMessage 0x063`: `i16 type, i16 0, i32
0, string target, string message, u8 language, i32 ability` (`packets/c2g_world.rs:250-272`). The chat
edit's per-type limits are `chatLocale.maxTextLength[type]`, default 160 characters
(`x2ui/chat/locale/ko.alb:20,50`); the C++ buffer cap is 1024 UTF-16 units.

**Filtering/censor.** `FUN_393c7bb2` converts the text, checks the local list state, and censors
using `blocked_texts` (`utf8str, check_name, check_chat, partial_match`, 24 rows) via
`FUN_393c78e0/FUN_393c7a10`; `replace_chats`/`replace_chat_texts` (17/67 rows) provide the
shortcut-to-phrase expansion reached from command matching (`FUN_393c5eb0`) [I]. The chat menu comes from `chat_commands` (15 rows: Say 0, Shout 1, Trade 2,
Party-finder 3, Party 4, Raid 5, Nation 6, Expedition 7, Family 9, Command 10, Trial 11, Performance
13, Coalition 14, Whisper -3, Whisper-reply -5). `/` functions come from `slash_commands` (171 rows;
15 ChatCommand, 47 SlashFunction, 109 ExpressText) and `slash_functions` (47). Social ones include
party invite/leave/kick (1/2/3), create/join/leave channel (4/5/6), dice (7), expedition invite/leave
(12/13), block/unblock (15/16), family list/invite/leave/kick/title (17/18/19/21/22), raid
invite/leave/kick (23/24/25), party (26/27/28), guild kick (41), friend add/delete (42/43).

**Bubbles.** `SCAreaChatBubble 0x153` -> `FUN_391dc340` with two ids (`+0x10,+0x14`) and requires the
same faction plus a distance check (`FUN_39370230`); `SCChatBubble 0x154` -> `FUN_391dc3c0` with kind
`+0x14`, text or NPC id at `+0x18`. Chatbubble text default life is 5000 ms
(`x2ui/chat/chatbubble.alb` proto 0.2).

**UI.** `x2ui/chat/chat_window.alb` allows up to 5 windows (`chatLocale.chatWindow`, ko.alb:15-16), each
430x350 at `BOTTOMLEFT(0,-145)`, slide 150 ms, resize 345x160..650x620, tab min width 42, tab area 30,
tab width 65, notify 3000 ms/blink 400 ms (proto 3/11); the message box holds 300 lines and hides
after 120 s (proto 8, lines 261/268). Tabs/filters/colors/font (12-17 pt, default 15), opacity, lock
and bounds persist per character; Reset deletes the four owner keys and re-runs the defaults
(`chat_window.alb` proto 9.1; `config/reset.rs:10-23`). Item and quest links are inserted through
`AddItemLinkToActiveChatInput`/`AddQuestLinkToActiveChatInput` (`FUN_3945aa00`); the item form is the
`|i...;` reference consumed by `X2Item:InfoFromLink`.

### 2.2 Party and raid

`SCAskToJoinTeam 0x0d4`/`Area 0x0d5` -> `FUN_391d7fe0`/`FUN_391d8060`; `SCJoinedTeam 0x0d6` ->
`FUN_391d80e0`; `SCRejectedTeam 0x0d7` -> `FUN_391d8160`; `SCLeavedTeam 0x0d8` -> `FUN_391d81e0`;
`SCTeamDismissed 0x0d9` -> `FUN_391d8280`; member joined/left/disconnected `0x0da-0x0dc` ->
`FUN_391d8300-8400`; owner/role `0x0dd/0x0de` -> `FUN_391d8480/8500`; raid convert `0x0df`;
move `0x0e0`; loot rule `0x0e2`; refresh `0x0e3`; remote members `0x0e4`; area invite `0x0e5`;
marker `0x0e6`; ping `0x0e7`; risky ack `0x0e8` (handlers `FUN_391d8580`..`FUN_391d8990`).
The ask handler reads `teamId, isParty, senderId, name`; member-joined carries team + `TeamMember` +
party (zero-based on the wire, one-based in Lua); move carries ids and zero-based `from/to`; loot
carries a 16-byte rule + change flags
(bit 0 method, 1 grade, 2 master, 3 BoP); ping carries `hasPing, x, y, z, instanceId`.

Party is 5, raid 50 in 10 parties x 5 (`FUN_39307320` rejects `>=0x32`; `FUN_393093e0` uses `/5,*5`);
roles `0 none, 1 tank, 2 healer, 3 dealer` and loot `0 free-for-all, 1 round-robin, 2 master`
(`x2game.dll.c:978520-978601`). `FUN_393099f0` refuses method `>=3`, grade outside the global range
or equal to the "unset" sentinel (x2game.dll.c:628187-628189). Area invite is 100 m
(`FUN_39309b10` string `399cd6fc`). Marks are 12 (`M_OVER_HEAD_MARK1..12`); index `-2` clears all.

Leave/kick/dismiss are two-stage when a risk prompt is required: `FUN_39308f60/39309030/39308c10`
send `CSLeaveTeam 0x07d`/`CSKickTeamMember 0x07e`/`CSDismissTeam 0x084` when no risk flag is set;
when `FUN_39309880` reports `w&1`, they instead send `CSAskRiskyTeamAction 0x088` with action 0/1/2
and wait (`FUN_393095c0`), then send the real opcode from the ack. Invite/loot/move/role/marker/ping
packets: `0x07a, 0x083, 0x081, 0x085, 0x086, 0x087`.

Group loot uses `SCLootDice 0x0fb` -> `FUN_391d9990`, `SCLootDiceNotify 0x0fc` ->
`FUN_391d9a10` (item link text at `+0x90`, dice count at `+0x150`; a bad link logs "Invalid item link
text for dice notification!") and `SCLootDiceSummary 0x0fd` -> `FUN_391d9b50`; the roll result is a
`CHAT_DICE_VALUE`/`msg_dice_value` line.

UI: `raidframe/raidframe_view.alb` always builds 10 party rows x 5, hiding empty parties
(`party.alb:10-24`); member bars are role-coloured, leader-marked, distance-faded and grey when
offline (`member.alb:133-282`); right-click opens the team popup (`popup_menu_proc.alb:345-424`:
loot method/grade/BoP, 12 marks, kick/by-name, owner); role window offers tank/healer/dealer/none;
manager offers convert/area invite/options/dismiss and drag-move (`raidteammanager/*`). Hotkeys
`team_target_1..4` skip self and dead targets (`FUN_3942a2a0`).

### 2.3 Guild ("expedition")

Incoming: `SCExpeditionRolePolicyList 0x00b` -> `FUN_391d2110` (`u8 count`, 148-byte policies);
`PolicyChanged 0x00c` -> `FUN_391d2190`; `RoleChanged 0x00d` -> `FUN_391d2210` (`u32 id, u8 role,
string name`); `OwnerChanged 0x00e` -> `FUN_391d2290`; `ShowRenameUI 0x00f` -> `FUN_391d2310`;
`MemberList 0x016` -> `FUN_391d2470` (`u32 total, u8 count, u32 id, members`); `MemberStatus 0x017`
-> `FUN_391d24f0`; `Invitation 0x01a` -> `FUN_391d2820`; `UnitExpeditionChanged 0x01c` ->
`FUN_391d2940`; `FactionOwnerChanged 0x018` -> `FUN_391d2570`; `FactionRelation 0x019` ->
`FUN_391d2790`. `ExpeditionMember` wire order is `expeditionId, charId, inParty, online,
lastWorldLeave(i64), name, level, zoneId, u32 id3, abilities[3], role, x(i64), y(i64), z(f32), memo,
transferRequestedTime(i64)` (`systems/social/expedition.rs:32-62`). A policy is `expeditionId, role,
name, 9 bools: dominionDeclare, invite, expel, promote, dismiss, chat, managerChat, siegeMaster,
joinSiege` (:64-92); the client buffers 148 bytes and reads success at `+0xa0`.

Outgoing: `CSCreateExpedition 0x004` (`string name, u32 sponsorId`; `FUN_392fa240`, client validates
name with `XlIsValidName(name,2)`, error 0x22), `CSChangeRolePolicy 0x006` (policy, `FUN_392f8bb0`),
`CSChangeMemberRole 0x007` (`u8 role, u32 memberId`), `CSChangeOwner 0x008` (`u32 memberId`),
`CSRename 0x009` (`u32 id, string name, u8 1`), `CSDismiss 0x00b`, `CSInvite 0x00c` (`string name`),
`CSReply 0x00d` (`u32 expeditionId, u32 inviterId, u8 join`), `CSLeave 0x00e`, `CSKick 0x00f`
(`u32 memberId`). Create/leave/dismiss open confirmation dialogs (`FUN_392fa2f0/392fa4b0`,
`FUN_392fa3a0/392fa5a0`). Client-side create only requires a sponsor tab and valid name
(`create_expedition.alb` proto 2, lines 21-96); level 5/party-of-5/1 gold are server rules
(`config.rs:42-55`), not client checks. Invite prompt is `ask_expedition_invite`; ignore option
`g_ignore_expedition_invite`.

UI: `expedition_management.alb` has three tabs (members, role policy, siege) plus two web-browser
children fed by `RequestExpeditionHome`/`RequestExpeditionBBS` (probe strings only; the C++ xref is the
generic browser `FUN_39263fa0`). Member list columns NAME, LV, CLASS, ROLE, POS, MEMO, with ONLINE,
PARTY, CHK, offline toggle and name/level sort (`expedition_management_memberlist_tab_view.alb`).
Role policy tab has 5 role buttons but only four permission checkboxes: inviteMember, expel, promote,
joinSiege (`..._role_policy_tab_view.alb`; `..._role_policy_tab.alb:99-112`); the other five bits are
sent unchanged from the loaded policy.

### 2.4 Friends and blocks

`SCFriends 0x04e`, `SCAddFriend 0x050`, `SCDeleteFriend 0x051`, `SCFriendStatusChanged 0x052`,
`SCBlockedUsers 0x054`, `SCAddBlockedUser 0x055`, `SCDeleteBlockedUser 0x056` -> handlers
`FUN_391df170`, `FUN_391df270`, `FUN_391df2f0`, `FUN_391df410`, `FUN_391df4a0`, `FUN_391df520`,
`FUN_391df5a0`. Friend entry is 0xD0 bytes: `id, name[0x81], race, level, hp, abilities[3], x/y/z,
zoneId, type, inParty, online, lastWorldLeave(i64)`; blocked entry is `u32 id, name[0x81]`. List is
`i32 total, i32 count, entries`. Add/delete/block/unblock carry a single `string name` (`0x104/0x105/
0x107/0x108`), 0x81 max. Add rejects self (0x1F5), duplicate (0x229) and normalises the first letter
to upper case; unblock of an absent user errors `user_not_in_list`. The client only pages locally
(10 rows/page, `friend_tab_view.alb`), and uses `IsBlockedUser` only to swap the popup block/unblock
entry; there is no local suppression of blocked whispers/mail/trade found in the client [?].

### 2.5 Family

Incoming: `Invitation 0x02d` (`u32 familyId, string name, string title`), `Created 0x02e`,
`Removed 0x02f`, `Desc 0x030` (roster `u32 id, i32 count, {u32 id, name, u8 role, u8 online,
title[0x69]}`), `MemberAdded 0x031` (+ `addedIndex` at `+0x794`), `MemberRemoved 0x032`
(`id, charId, kicked`), `OwnerChanged 0x033` (`id, charId`), `TitleChanged 0x034` (`id, charId,
title`), `MemberNameChanged 0x035`, `Title 0x036` (`id, title, owner`), `MemberOnline 0x037`
(`id, charId, online`) -> `FUN_391d29e0..FUN_391d2f20`. A member is 0xF0 bytes (`id, name[0x81],
role, online, title[0x69]`); the roster capacity is therefore 8 (`0xC+8+8*0xF0=0x794`).

Outgoing: `CSFamilyInviteMember 0x01a` (`string name, string title`; buffers 0x81/0x69),
`CSFamilyReplyInvitation 0x01b` (`u32 inviterId, u8 join, string title`), `Leave 0x01c` (empty),
`Kick 0x01d` (`u32 id`), `ChangeTitle 0x01e` (`u32 id, string title`), `ChangeOwner 0x01f`
(`u32 id`). Slash functions 17/18/19/21/22 map to list/invite/leave/kick/title. UI:
`relationship/relationship_view.alb:33-40` has Friends, Blocked, Family tabs (600x485, tab width 130);
family columns NAME, TITLE, ROLE, ONLINE, 10 rows/page (`family_tab_view.alb`). Titles are free text
validated by name policy; no cost or family level/exp is present in 1.2 [?].

### 2.6 Mail

Incoming opcodes `0x115-0x123` (failed, unread count, sent, got-mail, list, list-end, body,
receiver-opened, attachment-taken, charge-paid, deleted, returned, status-updated, removed). MailHeader
=`i64 id, u8 type, u8 status, string title(400), string sender(128), u8 attachments, string
receiver(128), i64 openDate, bool returned, i64 extra` (`FUN_398a5a00`:1941080-1941121). MailBody =
`i64 id, u8 type, string receiver(128), string title(400), string text(1600), i32 money0/1/2,
i64 sendDate/recvDate/openDate, 10 item slots` (`FUN_398a5e60`:1941248-1941298). Client mail types
0..25: NORMAL 1, EXPRESS 2, REGISTERED 3, SPAM 4, ADMIN 5, BILLING 6, BALANCE_RECEIPT 7,
TAXRATE_CHANGED 8, AUCTION_* 13-17, SYS_SELL_BACKPACK 19, HOUSING_SALE 21, NATION_* 23-25
(`mailbox/mail/common.alb`, K6-K44); page size 14, max 10 attachments.

`CSSendMail 0x098`: `u8 type, string receiver, u32 unk, string title, string text, u8 attachments,
i32 money0/1/2, i64 extra, 10x(u8 slotType,u8 slot), bc doodadObjId`
(`systems/economy/mail_packets.rs:342-375`). Limits: receiver 129 B, title 401 B, body 1601 B
(`FUN_3948b9c0`). Compose only offers `mail_normal`/`mail_express` and sends gold/silver/copper as one
i32 money (`write_mail.alb` lines 94-119). `withReceiver` is a client flag (`MAIL_WITHRECEIVER`) but
its wire slot was not resolved [?]. `CSListMail 0x09a` empty; `CSListMailContinue 0x09b` empty;
`CSReadMail 0x09c` `(bool isSent, i64 id)`; `CSDeleteMail 0x0a1` `(i64 id, bool isSent)`; return has
no 1.2 opcode (0xfff, server comment). Mailbox requires a `DoodadFuncNaviOpenMailbox` within 5 m
(`mail_packets.rs`). Server fees 50/100 cost units +30/80 per extra attachment, normal delivery
30 min, express instant (`mail.rs:148-155`); the client UI does not compute or show them.

UI: `mailbox/toc.g` has received/sent tabs only; columns TITLE, NAME, ISREAD, HASITEM, MAILID, CHK;
read window has attachment/money receive, reply, return, delete, pay tax; commercial mailbox is a
separate UI. `MAX_ATTACHMENT_COUNT=10`, `PAGE_PER_MAIL_MAX_COUNT=14` (`mail/common.alb`).

### 2.7 Player trade

Incoming `0x15e-0x16a`: can/cannot start, started, canceled, item putup/tookdown, other item
putup/tookdown, money putups, ok/lock updates, made. `SCTradeItemPutup` = `u8 slotType, u8 slot, i32
amount`; `SCTradeMade` = `u8 action, u8 count + tasks, u8 forceRemoveCount + i64 ids, u32 type`, cap
30. Outgoing: `CSStartTrade 0x0ec` objId; `CSCanStartTrade 0x0ed` objId; `CSCannotStartTrade 0x0ee`
`objId, reason`; `CSCancelTrade 0x0ef` reason; `CSPutupTradeItem 0x0f0` `(u8 slotType, u8 slot) +
i32 amount`; `CSPutupTradeMoney 0x0f1` i32; `CSTakedownTradeItem 0x0f2` `(slotType,slot)`;
`CSTradeLock 0x0f3` bool; `CSTradeOk 0x0f4` empty (only after lock). UI drags use the cursor-picked
amount (`trade.alb` proto 21.1 lines 328-338 -> `PutupTradeItem(index,amount)`), i.e. partial stacks
are supported. Lock click locks both item and money panes; OK only unlocks after both sides lock
(`trade.alb:437-446`). The client starts trades only within the **5 m surface gap** (`sqrt(DAT_39ac9b84 = 25.0)`
via `FUN_3989bf90`; `FUN_394fd9a0` / `FUN_394fddf0`; the doc's earlier "25 m" was the squared value) [R].
The server has no range check (`economy/trade.rs`).

## 3. Numeric parameters

| Thing | Value | Source |
|---|---|---|
| Chat type values | -4..15 as listed | baselib.alb:116-150 |
| Chat message cap | default 160 chars/type; 1024 UTF-16 buffer | ko.alb:20,50; FUN_393c5aa0 |
| Chat history/fade | 300 lines, 120 s | chat_window.alb proto 8 |
| Chat windows/tabs | up to 5 windows; tab 65 px, min 42, height 30 | ko.alb:15-16; chat_window.alb proto 3 |
| Window default | 430x350 at (0,-145); 345x160..650x620 | chat_window.alb proto 11/3 |
| Notify | 3000 ms, blink 400 ms | chat_window.alb proto 3 |
| Channel name | 49 bytes | FUN_393c63e0 |
| Spam config | 15 groups + 15 float delays | FUN_391d8cf0/FUN_393c2540 |
| Bubbles | 5000 ms default; area needs same faction | chatbubble.alb; FUN_39370230 |
| Party/raid | 5 / 50; 10x5 | FUN_39307320; FUN_393093e0 |
| Roles/loot | 0-3; 0-2 | x2game.dll.c:978535-978601 |
| Marks/area invite | 12; 100 m | map/common.alb; FUN_39309b10 |
| Guild policies | 5 roles, 9 permission bits, 148 B; 4 editable | expedition.rs:64-92; role tab |
| Family roster | 8 x 0xF0 B; page 10 | addedIndex 0x794; family_tab_view.alb |
| Friends/block row | 0xD0 / 0x88 B; page 10 | FUN_39304550/393035d0 |
| Mail | 10 attachments; page 14; 128/400/1600 B; fee 50/100+30/80; delay 30 min | mail/common.alb; mail.rs:148-155 |
| Trade | 30 tasks; 5 m start surface gap | packets/economy/trade.rs; DAT_39ac9b84 = 25.0 |

## 4. What is inferred

* `[I]` Chat type 6 is both faction and nation; 14 is race/coalition; the UI uses one channel row.
* `[I]` `withReceiver` is sent in the unread `u32 unk` or `i64 extra` slot; not proven.
* `[I]` Client-side blocked-list enforcement of whisper/mail/trade is absent; server enforces.
* `[I]` Friend-entry labels beyond id/name/race/level/zone/inParty/online/lastWorldLeave (hp,
  abilities, type) are inferred from struct offsets; blocked-entry is `u32 id + name[0x81]`.
* `[?]` Max friend/block list size; max chat tabs; exact spam thresholds (the table is a graph);
  default tab/filter colours; `DAT_39ac7ef8` and bubble-range constants (`.data` absent);
  `CSReturnMail` opcode; `PlayChatBubbling` xref; mail postage display.
* `[?]` Exact Lua parameter mapping of `OnChatMessage` (the handler passes 11 values; the shipped
  `chat_msg_event.alb` handler has 7 parameters and uses a numeric obj id).

## 5. Live oracle measurements

None for this area. No packet log, capture or screenshot was taken during this pass; all findings are
code/data reads. A live session should measure chat rate limiting, the invite field order accepted by
the original client, mail compose slots and family/guild window geometry. The trade range is now read
(`DAT_39ac9b84 = 25.0` squared, 5 m).

## 6. Open questions

1. Does the server send the team packets in the original field order, or is AAEmu's order a port bug?
   The original client reads `team,isParty,sender,name`; the server writes
   `team,sender,name,isParty` (`packets/team.rs:107-113`).
2. `withReceiver`, mail `extra`, and the three money fields: which is postage, COD and gold?
3. What are the real client-side spam thresholds and channel-join limits?
4. Is the guild member list sorted client-side or server-side, and what is its page size?
5. `SCChatFailed` vs `SCErrorMsg`: which codes are chat-only in 1.2?

## 7. Gaps vs Bevy (ranked)

1. **Social slash commands - done in `5401aaa`, `4d49692`.** The original dispatches 47 slash
   functions from `slash_commands`/`slash_functions`: party/raid/guild invite, leave and kick,
   channel create/join/leave, `/dice`, block/unblock, friend add/delete, family list/invite/leave/
   kick/title (`slash_functions` rows 1-28,42,43; `FUN_393c5eb0`). Bevy only resolved channel names
   and emote names. `crates/aa-client/src/hud/slash.rs` loads both tables at start-up, matches an
   alias as `FUN_393c3bc0` does (case-insensitive whole word), and routes the social rows to the
   native party/expedition/family/friend checks and queues with the original refusals
   (`0x1b3`, `0x54`, `0x48`, `0x49`, `0x10b`/`0x10c`, `0x1f5`, `0x229`, family
   `0x172`/`0x178`/`0x16f`). `/dice` and the channel functions send `CSRollDice` 0x0bd,
   `CSJoinUserChatChannel` 0x061 and `CSLeaveChatChannel` 0x062 with the native name 0x31 and
   password 7 byte caps (`FUN_397ab6c0`) and the duplicate-channel 0x10f text with its name.
   `slash_functions` 17 (family list) has no `slash_commands` row, so the relationship window stays
   its only entry. The remaining rows (cast/journal, languages, jury, watch/target, music, nation)
   are matched and consumed without an action.
2. **User chat channels - done in `68dc736`, `770eda1`, `cb0dbce`.** Original
   `X2Chat:Create/Join/LeaveUserChatChannel` sends `CSJoinUserChatChannel 0x061`/`CSLeaveChatChannel
   0x062` (x2game.dll.c:908837-908873; server reader `systems/social/packets/chat.rs:16-40`). The
   packets landed in `68dc736`; `ChatChannel` now carries the native i64 identity, `Chat` keeps the
   joined list and the three X2Chat bindings queue the native requests with the 0x31-byte name and
   7-byte password caps. A duplicate create/join refuses with 0x10f, and leave uses the joined
   channel's identity.
3. **Notices and chat failures - done in `770eda1`, `cb0dbce`.** Original handles
   `SCNoticeMessage 0x0d0`, `SCChatFailed 0x0d1`, `SCChatLocalizedMessage 0x0d2`,
   `SCChatSpamDelay 0x0d3` (`FUN_391d8d60/8de0/8c40/8cf0`). All four are decoded: the notice
   raises `NOTICE_MESSAGE` (non-positive visible time = `1000 + 50 ms` per character), the failure
   maps 0x108-0x111 through `ERROR_MSG` and raises `CHAT_FAILED`, the localized line fills its
   `X2Locale` template and dispatches as `CHAT_MESSAGE`, and the spam config is stored on `Chat`.
   Both the 1.2 server's short spam layout and the client's full 0x68 config are accepted.
4. **Chat send and render policy - done in `cb0dbce`.** Original enforces per-type 160-char limits, a
   1024 UTF-16 buffer, per-type spam delays and `blocked_texts` censoring (FUN_393c5aa0:786355;
   FUN_393c2540; FUN_393c7bb2). The send path now drops empty text, checks the type's spam group
   (refusing with 0x108), truncates at the type table's 160 characters, censors every `check_chat`
   row (whole-word or partial, refusing with 0x10a while sending the censored text) and caps the
   transport string at 1024 UTF-16 units. `CHAT_MESSAGE` carries the numeric unit id, character id,
   sender, text, `speakerInChatBound`, sub-type, faction, ability, language and race, so
   say-whisper distinctions, hostile colouring and ability/language styling work again.
5. **Party protocol diverges from the original.** Bevy sends only `CSAskRiskyTeamAction 0x088` for
   leave/kick/dismiss and never the real `0x07d/0x07e/0x084` after the ack
   (`crates/aa-net/src/party.rs:497-521`); the original sends `0x088` only when the ack is required,
   then the real packet (`FUN_39308f60/39309030/39308c10`, `FUN_393095c0`). Bevy writes invite as
   `team,bool,name` (`party.rs:499-501`); the original builder stores `team,name,bool`
   (`FUN_397abd00`) and its reader consumes `team,isParty,sender,name` (`FUN_391d7fe0`), while the
   server writes `team,sender,name,isParty` (`packets/team.rs:107-113`). The original wire order for
   CSInvite still needs one marshaller check [I], but the SCAsk mismatch is real. Fix: keep the
   two-stage risky flow, verify both directions against the original client, and add a compatibility
   test.
6. **Loot method 255 is accepted by Bevy.** Original `FUN_393099f0` accepts only method `<3` and
   rejects the sentinel (`x2game.dll.c:628187`); Bevy allows `0..=2 | 255`
   (`crates/aa-net/src/party.rs:451`). Fix: reject 255 and clamp grade to the original range.
7. **Guild/expedition coverage is incomplete.** Original handles `SCExpeditionSponsorChanged 0x014`,
   `SCFactionOwnerChanged 0x018`, `SCFactionSetRelationState 0x019` and shows the invitation; Bevy's
   handle list stops at `0x008|0x00b..0x00f|0x011|0x013|0x015..0x017|0x01a..0x01c`
   (`crates/aa-net/src/expedition.rs:140-142`), and `ExpeditionEvent::Invitation` is a cache no-op
   (:359). Bevy also normalises the undefined policy dismiss byte instead of sending it raw
   (`hud/expedition/actions.rs:156-163`) and its roster is level-desc only, 50/page
   (`hud/expedition/members.rs:8,63-69`), while the original member tab exposes NAME/LV/CLASS/ROLE/
   POS/MEMO, ONLINE/PARTY/CHK, offline toggle and name or level sort
   (`expedition_management_memberlist_tab_view.alb`). Fix: decode 0x014/0x018/0x019, surface the
   invitation, send the raw policy and add the missing sorts/columns.
8. **Mail and trade tails differ.** Bevy never sends `CSListMailContinue` ("lane currently only logs
   it", `crates/aa-net/src/mail.rs:137`), drops the compose `withReceiver` flag (`hud/mail.rs:51` vs
   `SendMail` has no field), reports `GetCurrencyForFee = 0` (`hud/mail.rs:392-393`) though the server
   charges 50/100+30/80, and disables return (`hud/mail.rs:395-405`). Trade refuses partial stacks
   ("Split the stack before trading fewer items", `hud/trade.rs:257-263`), while the original sends
   the cursor amount (`trade.alb` proto 21.1). Fix: send ListContinue, carry `withReceiver`, call the
   postage helper, and pass the picked amount in `CSPutupTradeItem`. The 5 m start range is done in
   `b7f1abe` (`DAT_39ac9b84 = 25.0` squared).
9. **Friends/family polish.** Bevy's friend rows omit the offline elapsed duration
   (`hud/friends.rs:196-197`) and expose no block effect outside chat; family is capped at 8 in the
   UI only (`hud/family/state.rs:69`) and has no invitation timer. These are lower risk: the original
   client also has no family timer and no block enforcement found, so only the offline duration and
   the missing `/family_*` slash commands are true parity items.
