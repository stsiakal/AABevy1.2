#!/usr/bin/env python3
"""Machine-checkable area/status table for the r3-394bec40 survey.

Splits every function in the range into exactly one area. DB intervals are
split into SQL loaders (calls a sqlite prepare/step/column helper or has an
SQL/Load* literal) and the remaining generated descriptor/STL model code.

Prints a markdown table + status totals; asserts the totals equal funcs.csv.
"""
import collections
import csv

LO, HI = 0x394BEC40, 0x397135D0
CSV = r"D:/aa-re/client12/out-x2game/funcs.csv"
CALLS = r"D:/aa-re/client12/out-x2game/calls.tsv"
XREFS = r"D:/aa-re/client12/out-x2game/strings-xrefs.tsv"

SQLHELP = {
    0x3955B3D0,  # sqlite3_prepare_v2 wrapper
    0x3955FB10,  # sqlite3_step wrapper
    0x3953A390,  # sqlite3_column_* wrapper
    0x395519B0,  # sqlite3_finalize wrapper
    0x395338F0,  # sqlite3_errmsg wrapper
    0x3953A480,  # sqlite3_column_text wrapper
}

# name, lo, hi, status, runtime, what, evidence
ROWS = [
    ("UnitModifierInfo stat table", 0x394BEC40, 0x394C0230, "MAPPED", "yes",
     "X2Unit:UnitModifierInfo delta/colour table over a unit's stat cache",
     "port-formulas/MAP.md cites 0x394bec40; hud/stats.rs"),
    ("X2Unit native getters", 0x394C0230, 0x394C43D0, "TO-PORT", "yes",
     "unit queries used by the Lua UI (health/mana/class/buff/portrait/screen position...)",
     "hud/api.rs + docs/ui-api.md list 84 X2Unit methods; 24 names absent from aa-client/aa-ui/aa-lua"),
    ("X2Unit Lua registration", 0x394C43D0, 0x394C6583, "COVERED", "yes",
     "registers the 84 X2Unit methods as a Lua table",
     "docs/ui-api.md -7 register 0x394c43d0; aa-client hud tables; not proven equal"),
    ("X2Unit helpers", 0x394C6583, 0x394C6B20, "COVERED", "yes",
     "small native helpers behind the X2Unit getters",
     "aa-client hud/api.rs; not proven equal"),
    ("X2Util money/number/time helpers", 0x394C6B20, 0x394C8CA0, "TO-PORT", "yes",
     "money/AAPoint/number string arithmetic, date/time formatting and validation",
     "docs/ui-api.md X2Util 37 methods; 7 names (Random2, DiffTimeStr, ConvertWorldToScreen, GetRGBAByCursor, GetRevisionStr, GetFocusedWidgetId, GetSeamlessOrigin) absent from crates"),
    ("Name policy", 0x394C8CA0, 0x394C9B40, "MAPPED", "yes",
     "character/portal/summon/chat-tab name policy tables + XlGetNamePolicyInfo",
     "port-engine-core-rest/MAP.md cites 0x394c8ca0; aa-client login/name_policy.rs"),
    ("X2Util Lua registration", 0x394C9B40, 0x394CAD40, "COVERED", "yes",
     "registers the 37 X2Util methods",
     "docs/ui-api.md -7 register 0x394c9b40; aa-client hud/ingame.lua X2Util subset"),
    ("X2Warp/portal helpers", 0x394CAD40, 0x394CB5E0, "MAPPED", "yes",
     "portal list/return-list/open/save/rename/delete helpers",
     "port-portals/MAP.md, functions.md; aa-client hud/portal.rs"),
    ("X2Warp Lua registration", 0x394CB5E0, 0x394CBB10, "MAPPED", "yes",
     "registers the 12 X2Warp methods",
     "port-portals/MAP.md cites 0x394cb5e0; docs/ui-api.md -7"),
    ("X2World world/zone list", 0x394CBB10, 0x394CD3F0, "TO-PORT", "yes",
     "world list, zone names/factions, race congestion, level table",
     "docs/ui-api.md X2World 27 methods; GetCurrentWorldInfo, LevelCount, LevelName, GetZoneName, GetWorldCharactersInfo absent from crates"),
    ("X2World registration/character select", 0x394CD3F0, 0x394CFFF0, "COVERED", "yes",
     "registers X2World and drives world/char list selection and exit",
     "docs/ui-api.md -7 register 0x394cd3f0; aa-client login/world_select.rs, character_select.rs"),
    ("Item tooltip/effect helpers", 0x394CFFF0, 0x394D24D0, "MAPPED", "yes",
     "item tooltip rows, effect/date strings, world intersections",
     "port-interaction/MAP.md cites 0x394d0730/0810/0820; port/interaction/doodad.rs"),
    ("Craft manager", 0x394D24D0, 0x394DBE90, "COVERED", "yes",
     "recipe/actability search, craft list, craft manager update",
     "aa-client hud/crafting/*; docs/research/re-crafting.md; not proven equal"),
    ("Craft init/tooltip keys", 0x394DBE90, 0x394DCEF0, "COVERED", "yes",
     "craft list init and item tooltip warning keys",
     "aa-client hud/crafting/*, hud/tips.rs; not proven equal"),
    ("Auction/trade event messages", 0x394DCEF0, 0x394E06A0, "COVERED", "yes",
     "auction/trade UI event text and item/money offer checks",
     "aa-client hud/auction.rs, hud/trade.rs; 0x394df370 cited by port-housing/MAP.md"),
    ("Item use/equip checks", 0x394E06A0, 0x394E4E80, "MAPPED", "yes",
     "use/consume/equip/unequip validation and request builders",
     "port-combat/MAP.md cites 0x394e2650; port-portals/MAP.md cites 0x394e0960"),
    ("Attachment/icons/slave equipment", 0x394E4E80, 0x394E79C0, "MAPPED", "yes",
     "item bone attachment, icon paths, slave-equip errors",
     "port-interaction/MAP.md cites 0x394e6c10; docs/research/equipment-attach.md"),
    ("Slave/housing item misc", 0x394E79C0, 0x394F0000, "PORTED", "yes",
     "slave/housing item helpers and placement/routing checks",
     "crates/aa-client/src/port/housing/{decor,lifecycle,routing}.rs; port/interaction/click.rs"),
    ("Item getters/duration monitor", 0x394F0000, 0x394F61F0, "COVERED", "yes",
     "item accessor helpers and item-duration monitor",
     "aa-client hud/items.rs, tips.rs; port-housing/inventory.md; not proven equal"),
    ("Loot/interaction helpers", 0x394F61F0, 0x394FDCD0, "PORTED", "yes",
     "loot piles, world-interaction candidates, markers and build helpers",
     "crates/aa-client/src/port/interaction/{loot,builders,npc}.rs; port-interaction/MAP.md"),
    ("Trade UI/rules", 0x394FDCD0, 0x394FF650, "COVERED", "yes",
     "player trade offers, locks, cancellation and level restrictions",
     "aa-client hud/trade.rs; re-social.md; not proven equal"),
    ("Web event telemetry", 0x394FF650, 0x39500000, "OUT", "yes",
     "posts gameplay/web events to http://%s/events/game?sessionKey=...",
     "R: cl_web_session_key + URL in FUN_394ff650; telemetry only"),
    ("Chat data/tabs", 0x39500000, 0x395072C0, "COVERED", "yes",
     "chat tabs, options, window/tab persistence and message dispatch",
     "aa-client hud/chat.rs, hud/chat/*; docs/ui-api.md X2Chat 29 methods (2 names missing)"),
    ("Chat widgets/defaults", 0x395072C0, 0x39508C90, "COVERED", "yes",
     "chat window defaults, reset and widget wiring",
     "aa-client hud/chat.rs; aa-ui native chat examples; not proven equal"),
    ("Publisher fake configs", 0x39508C90, 0x39509840, "OUT", "only-tools",
     "fake_gameon/fake_mailru auth configs and command-line overrides",
     "R: strings fake_*.cfg; test/publisher tooling, not AAEmu login"),
    ("Startup/framework/window", 0x39509840, 0x39509CF0, "MAPPED", "yes",
     "CreateGame wrapper, X2Startup vtable, window class, cryaction load",
     "port-engine-loop/MAP.md cites 0x39509870/9910/9940/99c0/9be0"),
    ("Cry alloc/free wrappers", 0x39509CF0, 0x3950A230, "OUT", "yes",
     "CryMalloc/CryFree + memory statistics wrappers used by all containers",
     "R: calls CryMalloc/CryFree; Rust allocator replaces it"),
    ("TenProxy", 0x3950A230, 0x3950A2B0, "OUT", "yes",
     "Tencent TenProxy.dll Invoke(0x566) probe",
     "R: TenProxy.dll/Invoke strings; publisher anti-cheat/tooling"),
    ("Fake tcls/args", 0x3950A2B0, 0x3950A870, "OUT", "only-tools",
     "fake_tcls.cfg auth/world arguments and r_Fullscreen setup",
     "R: fake_tcls.cfg, r_Fullscreen; publisher/test tooling"),
    ("Window proc/UI init", 0x3950A870, 0x3950C4F0, "COVERED", "yes",
     "WndProc for mouse/key/IME/focus plus window class registration",
     "R: 0x3950a8e0 WndProc; covered by winit/aa-ui; port-engine-loop/MAP.md for 0x3950c3c0"),
    ("X2Startup::Run loop", 0x3950C4F0, 0x3950C6C0, "MAPPED", "yes",
     "message pump + CGame update loop",
     "port-engine-loop/MAP.md cites 0x3950c4f0"),
    ("Login RC4/crypto", 0x3950C6C0, 0x3950CB10, "COVERED", "yes",
     "RC4 key setup/crypt and performance-counter struct used by login",
     "aa-net login/game.rs cipher; not proven equal"),
    ("GameSpy/publisher/anti-cheat", 0x3950CB10, 0x3950D3D0, "OUT", "yes",
     "GameSpy interface, encrypted publisher config, process-memory scan",
     "R: GameSpy, RF T block, CreateToolhelp32Snapshot/OpenProcess; anti-cheat/telemetry"),
    ("Game DB init/DDCMS bootstrap", 0x3950D3D0, 0x3950E000, "MAPPED", "yes",
     "loads game_pak DB, local fallback and DDCMS localized texts",
     "port-engine-loop/MAP.md (CGame::Init 0x3950d5e0); aa-client hud/data.rs"),
    ("Chat UI native", 0x3950E000, 0x39514760, "COVERED", "yes",
     "chat window/tab/edit/IME widgets and X2Chat data plumbing",
     "aa-client hud/chat.rs; docs/ui-api.md registers 0x3950e200/0x3950e850; not proven equal"),
    ("SQLite 3.5.6", 0x39514760, 0x39560C36, "COVERED", "yes",
     "embedded SQLite engine (btree, pager, VDBE, parser, functions)",
     "R: 3.5.6, SQLite format 3, sqlite_master; rusqlite 0.40 bundled; not proven equal"),
    ("HShield anti-cheat", 0x39560C36, 0x39565338, "OUT", "yes",
     "HackShield update/PE export/config checks (HSUpChk.log, hsupdate.env)",
     "R: HSUpChk.log, HShield, hsupdate.env; anti-cheat"),
    ("Intel ITT telemetry", 0x39565338, 0x39566BF0, "OUT", "yes",
     "libittnotify instrumentation wrappers",
     "R: libittnotify.dll, __itt_* imports; profiler telemetry"),
    ("Formula expression engine", 0x39566BF0, 0x3956B0F0, "PORTED", "yes",
     "dynamic f32 expression parser/evaluator/verifier and DB formula loading",
     "crates/aa-client/src/port/formulas/eval.rs cites 0x39566d00..0x39569540; formulas.md"),
    ("Game data enum/name tables", 0x3956B0F0, 0x39571FB0, "COVERED", "yes",
     "item/npc/skill/grade/slot/quest enum -> string tables used by UI and loaders",
     "aa-client hud/data.rs, tips.rs (0x39570b90), item_effects.rs; equipment-attach.md; not proven equal"),
]

# DB intervals: loaders are COVERED (rusqlite readers), remaining generated
# descriptor/STL model code is OUT (no behavioural parity value).
DB = [
    ("DB boot/wearables/config", 0x39571FB0, 0x3957D870,
     "content_configs, wearables BP, allowed_name_chars, unit_attribute_limits, item_configs"),
    ("DB item configs/model", 0x3957D870, 0x395AC210,
     "item_configs row plus the item/wearable descriptor model and containers"),
    ("DB loot/buff relations", 0x395AC210, 0x395B9000,
     "loot groups, buff/skill relations, item grades/sockets, expert limits, merchant ratios"),
    ("DB siege/skill model", 0x395B9000, 0x395D0000,
     "siege_plans + skill/buff descriptor model code"),
    ("DB skill tags", 0x395D0000, 0x395F0000,
     "skill_req/tagged skill/buff/item/npc relations and zone banned tags"),
    ("DB loot/merchant/localized", 0x395F0000, 0x39610000,
     "loot packs, merchants, item weapons/armors, achievements, localized_texts + DDCMS (0x3960b820)"),
    ("DB levels/character/account", 0x39610000, 0x39630000,
     "levels/resurrection/aoe-diminishing loaders + character/account/social row model"),
    ("DB templates/table counts", 0x39630000, 0x39660000,
     "per-table SELECT COUNT probes, vector/map instantiations and descriptor templates"),
    ("DB doodad/housing/sphere", 0x39660000, 0x39680000,
     "doodad func groups, housing, spheres, quests, mount skills"),
    ("DB model templates", 0x39680000, 0x396A0000,
     "generated descriptor/container templates"),
    ("DB item descriptions", 0x396A0000, 0x396C0000,
     "item armor/accessory/weapon/tool/housing/backpack/body/dyeing/summon descs"),
    ("DB item/npc/character", 0x396C0000, 0x396D0000,
     "item grade/category/set/model/holdable, npc AI, slave/character customization"),
    ("DB slave/transfer/bindings", 0x396D0000, 0x396E0000,
     "slaves, npc groups/spawners, transfers, character packs, model bindings"),
    ("DB model/counts", 0x396E0000, 0x397135D0,
     "generated descriptor/container templates + per-table count probes"),
]

DB_RUSQLITE = "note A"
DB_MODEL = "note B"


def main():
    sizes = {}
    for r in csv.DictReader(open(CSV)):
        s = int(r["start"], 16)
        if LO <= s < HI:
            sizes[s] = int(r["size"])
    xr = collections.defaultdict(list)
    for r in csv.DictReader(open(XREFS, encoding="utf-8", errors="replace"), delimiter="\t"):
        fa = r["function_addr"]
        if fa != "-":
            a = int(fa, 16)
            if a in sizes:
                xr[a].append(r["string"])
    callees = collections.defaultdict(set)
    for r in csv.DictReader(open(CALLS), delimiter="\t"):
        a = int(r["caller_addr"], 16)
        try:
            b = int(r["callee_addr"], 16)
        except ValueError:
            continue
        if a in sizes:
            callees[a].add(b)

    def is_loader(a):
        if callees[a] & SQLHELP:
            return True
        return any(
            ("SELECT " in s or "sqlite3_prepare" in s or "failed: sqlite3" in s or "function=Load" in s)
            for s in xr.get(a, [])
        )

    rows = []  # (name, status, fns, bytes, runtime, what, evidence)
    for name, lo, hi, status, runtime, what, ev in ROWS:
        fs = [a for a in sizes if lo <= a < hi]
        rows.append((name, status, len(fs), sum(sizes[a] for a in fs), runtime, what, ev))
    for name, lo, hi, what in DB:
        fs = [a for a in sizes if lo <= a < hi]
        loaders = [a for a in fs if is_loader(a)]
        model = [a for a in fs if a not in set(loaders)]
        if loaders:
            rows.append((name + " loaders", "COVERED", len(loaders),
                         sum(sizes[a] for a in loaders), "yes", what, DB_RUSQLITE))
        if model:
            rows.append((name + " model", "OUT", len(model),
                         sum(sizes[a] for a in model), "yes",
                         "generated descriptor/container model code around the same tables", DB_MODEL))

    total_fns = sum(r[2] for r in rows)
    total_b = sum(r[3] for r in rows)
    assert total_fns == len(sizes), (total_fns, len(sizes))
    assert total_b == sum(sizes.values()), (total_b, sum(sizes.values()))

    print("| area | fns | KB | runtime | status | what | evidence |")
    print("|---|---:|---:|---|---|---|---|")
    for name, status, n, b, runtime, what, ev in rows:
        print("| %s | %d | %.1f | %s | %s | %s | %s |" % (name, n, b / 1024, runtime, status, what, ev))
    print()
    print("| status | fns | KB |")
    print("|---|---:|---:|")
    agg = collections.defaultdict(lambda: [0, 0])
    for _name, status, n, b, *_ in rows:
        agg[status][0] += n
        agg[status][1] += b
    for status in ("MAPPED", "PORTED", "COVERED", "TO-PORT", "OUT", "UNKNOWN"):
        n, b = agg.get(status, [0, 0])
        print("| %s | %d | %.1f |" % (status, n, b / 1024))
    print("| **total** | **%d** | **%.1f** |" % (total_fns, total_b / 1024))


if __name__ == "__main__":
    main()
