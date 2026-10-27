#!/usr/bin/env python3
"""Provision the per-worker live-test slots on the golden lanes.

For every slot (``wp1``..``wp6``, ``f7``, ``interaction``, ``combat``,
``spare1``..``spare3``) and both roles (``a`` = hidden original, ``b`` = Bevy)
this creates, in each lane's own databases:

* a login user (password = the lane's ``cap-pw.txt``),
* a character cloned from the lane's reference character (race/class/gear/
  skills/action bar) at the usual test spot, level 1 on move-golden and 30 on
  combat-golden,
* one live Brown Lilyut Horse (item 8159, ``mate_dead=0``) plus its ``mates``
  row at full HP/MP.

It is idempotent: existing slot rows are replaced, reference characters are
never touched. All lane DBs are the throwaway ``lane_*`` databases on the
private MySQL at 127.0.0.1:3317.

Usage:
    python tools/live-slots/provision.py --dry-run
    python tools/live-slots/provision.py
    python tools/live-slots/provision.py --lane move-golden --slot wp1
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import live_slots  # noqa: E402

MYSQL = Path(os.environ.get("LIVE_SLOTS_MYSQL", r"C:\Program Files\MySQL\MySQL Server 8.4\bin\mysql.exe"))
MYSQL_PORT = os.environ.get("LIVE_SLOTS_MYSQL_PORT", "3317")
MYSQL_USER = os.environ.get("LIVE_SLOTS_MYSQL_USER", "aaemu_local")
DB_PASSWORD_FILE = Path(os.environ.get("LIVE_SLOTS_DB_PASSWORD", r"D:\aaemu-live\cfg\db_password.txt"))
LANES_DIR = Path(os.environ.get("LIVE_SLOTS_LANES", r"D:\aaemu-lanes"))
PASSWORD_FILE = Path(os.environ.get("LIVE_SLOTS_PASSWORD_FILE", r"D:\aaemu-live-move-golden\cap-pw.txt"))
REFERENCE_CHARACTER = 3
HORSE_TEMPLATE = 8159
HORSE_DETAILS = "DC0500000005"  # exp 1500, mate_dead=0, mate_level=5
ITEM_ID_BASE = 20_000_000
CONTAINER_ID_BASE = 700_000


def lane_settings(lane: str) -> dict[str, str]:
    path = LANES_DIR / f"{lane}.env"
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            values[key.strip()] = value.strip()
    return values


def mysql(sql: str, database: str | None = None, dry_run: bool = False) -> str:
    if dry_run:
        print(sql)
        return ""
    env = dict(os.environ)
    env["MYSQL_PWD"] = DB_PASSWORD_FILE.read_text(encoding="utf-8-sig").strip()
    command = [str(MYSQL), "--host=127.0.0.1", f"--port={MYSQL_PORT}", f"--user={MYSQL_USER}",
               "--protocol=TCP", "--default-character-set=utf8mb4", "--batch", "--raw",
               "--skip-column-names"]
    if database:
        command.append(database)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".sql", delete=False) as fh:
        fh.write(sql)
        sql_path = fh.name
    try:
        with open(sql_path, "rb") as stdin:
            result = subprocess.run(command, stdin=stdin, capture_output=True, timeout=180,
                                    env=env)
    finally:
        os.unlink(sql_path)
    if result.returncode != 0:
        raise SystemExit(f"mysql failed: {result.stderr.decode('utf-8', 'replace').strip()}")
    return result.stdout.decode("utf-8", "replace")


def q(value: str) -> str:
    return "'" + value.replace("\\", "\\\\").replace("'", "''") + "'"


def columns(database: str, table: str) -> list[str]:
    text = mysql(
        "SELECT column_name FROM information_schema.columns "
        f"WHERE table_schema={q(database)} AND table_name={q(table)} ORDER BY ordinal_position")
    return [line.strip() for line in text.splitlines() if line.strip()]


def account_hash() -> str:
    data = hashlib.sha256(PASSWORD_FILE.read_text(encoding="utf-8-sig").strip().encode()).digest()
    return base64.b64encode(data).decode()


def delete_character(game_db: str, character_id: int) -> str:
    """Remove every row that belongs to a slot character before re-inserting it."""
    return f"""
DELETE FROM item_containers WHERE owner_id = {character_id};
DELETE FROM items WHERE owner = {character_id};
DELETE FROM mates WHERE owner = {character_id};
DELETE FROM skills WHERE owner = {character_id};
DELETE FROM abilities WHERE owner = {character_id};
DELETE FROM character_active_buffs WHERE character_id = {character_id};
DELETE FROM characters WHERE id = {character_id};
"""


def provision_character(lane: str, settings: dict[str, str], game_db: str,
                        slot: str, role: str, password_b64: str,
                        dry_run: bool) -> dict:
    info = live_slots.resolve(slot, lane, role)
    index = live_slots.slot_index(slot)
    role_index = live_slots.ROLES.index(role)
    pos = live_slots.LANES[lane]["position"]
    level = live_slots.LANES[lane]["level"]
    x = pos[0] + index * 3.0
    y = pos[1] + role_index * 2.0
    z = pos[2]

    users = mysql(f"SELECT id FROM users WHERE username = {q(info.user)}", settings["DB_LOGIN"],
                  dry_run=dry_run)
    if not dry_run:
        if users.strip():
            account_id = int(users.split()[0])
            mysql(f"UPDATE users SET password = {q(password_b64)} WHERE id = {account_id};",
                  settings["DB_LOGIN"])
        else:
            mysql(
                "INSERT INTO users(username,password,email,last_ip,last_login,created_at,updated_at) "
                f"VALUES ({q(info.user)}, {q(password_b64)}, '', '', NOW(), NOW(), NOW());",
                settings["DB_LOGIN"])
            account_id = int(mysql(f"SELECT id FROM users WHERE username = {q(info.user)}",
                                   settings["DB_LOGIN"]).split()[0])
    else:
        account_id = 0

    char_cols = columns(game_db, "characters")
    overrides = {"id", "account_id", "name", "level", "hp", "mp", "x", "y", "z", "yaw",
                 "pitch", "roll", "updated_at"}
    select = []
    for col in char_cols:
        if col == "id":
            select.append(str(info.character_id))
        elif col == "account_id":
            select.append(str(account_id))
        elif col == "name":
            select.append(q(info.character))
        elif col == "level":
            select.append(str(level))
        elif col in ("hp", "mp"):
            select.append("9999")
        elif col == "x":
            select.append(f"{x:.3f}")
        elif col == "y":
            select.append(f"{y:.3f}")
        elif col == "z":
            select.append(f"{z:.3f}")
        elif col in ("yaw", "pitch", "roll"):
            select.append("0")
        elif col == "updated_at":
            select.append("NOW()")
        else:
            select.append(f"`{col}`")

    containers = mysql(
        f"SELECT container_id, container_type, slot_type, container_size FROM item_containers "
        f"WHERE owner_id = {REFERENCE_CHARACTER} AND container_type <> 'MateEquipmentContainer' "
        f"ORDER BY container_id", game_db)
    container_map: dict[int, int] = {}
    container_sql = []
    for seq, line in enumerate(containers.splitlines()):
        if not line.strip():
            continue
        old_id, ctype, slot_type, size = line.split("\t")
        new_id = CONTAINER_ID_BASE + info.character_id * 20 + seq
        container_map[int(old_id)] = new_id
        container_sql.append(
            f"INSERT INTO item_containers(container_id,container_type,slot_type,container_size,"
            f"owner_id,mate_id) VALUES ({new_id},{q(ctype)},{slot_type},{size},{info.character_id},0);")

    items = mysql(
        "SELECT id,type,template_id,container_id,slot_type,slot,count,IFNULL(HEX(details),''),"
        "lifespan_mins,made_unit_id,unsecure_time,unpack_time,grade,flags,created_at,ucc,"
        "IFNULL(expire_time,'0001-01-01 00:00:00'),expire_online_minutes,"
        "IFNULL(charge_time,'0001-01-01 00:00:00'),charge_count "
        f"FROM items WHERE owner = {REFERENCE_CHARACTER} "
        "AND type <> 'AAEmu.Game.Models.Game.Items.SummonMate' ORDER BY slot_type,slot", game_db)
    item_sql = []
    seq = 0
    for line in items.splitlines():
        if not line.strip():
            continue
        (old_item_id, itype, template_id, old_container, slot_type, slot, count, details,
         lifespan, made_unit, unsecure, unpack, grade, flags, created, ucc, expire,
         expire_online, charge_time, charge_count) = line.split("\t")
        new_item_id = ITEM_ID_BASE + info.character_id * 1000 + seq
        seq += 1
        new_container = container_map.get(int(old_container), 0)
        details_sql = f"UNHEX('{details}')" if details else "NULL"
        item_sql.append(
            "INSERT INTO items(id,type,template_id,container_id,slot_type,slot,count,details,"
            "lifespan_mins,made_unit_id,unsecure_time,unpack_time,owner,grade,flags,created_at,"
            "ucc,expire_time,expire_online_minutes,charge_time,charge_count) VALUES "
            f"({new_item_id},{q(itype)},{template_id},{new_container},{slot_type},{slot},{count},"
            f"{details_sql},{lifespan},{made_unit},{q(unsecure)},{q(unpack)},{info.character_id},"
            f"{grade},{flags},{q(created)},{ucc},{q(expire)},{expire_online},{q(charge_time)},"
            f"{charge_count});")

    horse_id = ITEM_ID_BASE + info.character_id * 1000 + seq
    inventory_container = next(
        (new for old, new in container_map.items()
         if old == 65553 or old == 65537), None)
    if inventory_container is None:
        # fall back to the slot_type 2 container of the reference character
        for old, new in container_map.items():
            container = next((l for l in containers.splitlines() if l.startswith(f"{old}\t")), "")
            if container.split("\t")[2] == "2":
                inventory_container = new
                break
    if inventory_container is None:
        raise SystemExit(f"{lane}: no inventory container for character {REFERENCE_CHARACTER}")
    item_sql.append(
        "INSERT INTO items(id,type,template_id,container_id,slot_type,slot,count,details,"
        "lifespan_mins,made_unit_id,unsecure_time,unpack_time,owner,grade,flags,created_at,"
        "ucc,expire_time,expire_online_minutes,charge_time,charge_count) VALUES "
        f"({horse_id},'AAEmu.Game.Models.Game.Items.SummonMate',{HORSE_TEMPLATE},"
        f"{inventory_container},2,10,1,UNHEX('{HORSE_DETAILS}'),0,0,'0001-01-01 00:00:00',"
        f"'0001-01-01 00:00:00',{info.character_id},0,1,NOW(),0,'0001-01-01 00:00:00',0,"
        "'0001-01-01 00:00:00',0);")
    mate_sql = (
        f"INSERT INTO mates(id,item_id,name,xp,level,mileage,hp,mp,owner,updated_at,created_at) "
        f"VALUES ({horse_id},{horse_id},'Brown Lilyut Horse',1500,5,0,9999,9999,"
        f"{info.character_id},NOW(),NOW());")

    skills = mysql(f"SELECT id,level,type FROM skills WHERE owner = {REFERENCE_CHARACTER}",
                   game_db)
    skill_sql = [f"INSERT INTO skills(id,level,type,owner) VALUES ({id_},{lvl},{q(typ)},"
                 f"{info.character_id});"
                 for id_, lvl, typ in (line.split("\t") for line in skills.splitlines() if line.strip())]
    abilities = mysql(f"SELECT id,exp FROM abilities WHERE owner = {REFERENCE_CHARACTER}",
                      game_db)
    ability_sql = [f"INSERT INTO abilities(id,exp,owner) VALUES ({id_},{exp},{info.character_id});"
                   for id_, exp in (line.split("\t") for line in abilities.splitlines() if line.strip())]

    sql = delete_character(game_db, info.character_id)
    sql += ("INSERT INTO characters(" + ",".join(f"`{c}`" for c in char_cols) + ") SELECT "
            + ",".join(select) + f" FROM characters WHERE id = {REFERENCE_CHARACTER};\n")
    sql += "\n".join(container_sql + item_sql + [mate_sql] + skill_sql + ability_sql) + "\n"
    mysql(sql, game_db, dry_run=dry_run)
    return {"slot": info.slot, "role": role, "lane": lane, "user": info.user,
            "character": info.character, "character_id": info.character_id,
            "level": level, "position": [round(x, 3), round(y, 3), round(z, 3)],
            "zone_id": live_slots.LANES[lane]["zone_id"], "account_id": account_id,
            "horse_item_id": horse_id}


def heal_existing(game_db: str, dry_run: bool) -> None:
    """Clear the dead-mate flags and pools on the hand-made reference characters."""
    mysql(f"""
UPDATE items SET details = CONCAT(SUBSTRING(details,1,4), CHAR(0), SUBSTRING(details,6))
 WHERE type = 'AAEmu.Game.Models.Game.Items.SummonMate'
   AND template_id = {HORSE_TEMPLATE} AND ORD(SUBSTRING(details,5,1)) <> 0;
UPDATE mates SET hp = 9999, mp = 9999, updated_at = NOW() WHERE hp <= 0 OR mp <= 0;
UPDATE characters SET hp = 9999, mp = 9999 WHERE hp <= 0 OR mp <= 0;
""", game_db, dry_run=dry_run)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lane", action="append", choices=sorted(live_slots.LANES))
    ap.add_argument("--slot", action="append", choices=live_slots.SLOT_NAMES)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    lanes = args.lane or sorted(live_slots.LANES)
    slots = args.slot or live_slots.SLOT_NAMES
    password_b64 = account_hash()
    summary = []
    for lane in lanes:
        settings = lane_settings(lane)
        for slot in slots:
            for role in live_slots.ROLES:
                summary.append(provision_character(lane, settings, settings["DB_GAME"], slot,
                                                   role, password_b64, args.dry_run))
        heal_existing(settings["DB_GAME"], args.dry_run)
    summary_path = ROOT / "data" / "live-slots.json"
    if not args.dry_run:
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"provisioned {len(summary)} characters on {', '.join(lanes)}")
    for row in summary:
        print(f"  {row['lane']:<13} {row['slot']:<12} {row['role']} "
              f"{row['user']:<14} {row['character']:<14} id={row['character_id']} "
              f"lvl={row['level']} at {row['position']}")


if __name__ == "__main__":
    main()
