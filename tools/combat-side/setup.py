#!/usr/bin/env python3
"""Snapshot, apply and restore the small lane-DB setup for the combat side-by-side probe.

The probe logs in two existing GM characters of the lane ``combat-golden``:

* ``clienttest`` / ``Goldtest`` (account 1) - the hidden original client
* ``combatgold`` / ``Goldone``  (account 2) - the Bevy client

Both get the same learned-skill set and the same action bar (1 = Triple Slash,
2 = Freezing Arrow, 3 = Melee Attack), and both account passwords are set to the
probe password so the launcher can log in without touching the real ones.

Everything changed here is written to a JSON snapshot first; ``restore`` puts it
back.  Only ``characters.slots``/position/hp/mp/level/experience, the two
characters' ``skills`` rows and the two login ``users`` rows are touched.

Usage (from the repo root):
    python tools/combat-side/setup.py snapshot shots/combat-side/setup.json
    python tools/combat-side/setup.py apply    shots/combat-side/setup.json
    python tools/combat-side/setup.py restore  shots/combat-side/setup.json
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_slots  # noqa: E402

LANE_ENV = Path(os.environ.get("COMBAT_SIDE_LANE_ENV", "D:/aaemu-lanes/combat-golden.env"))
MYSQL = Path(os.environ.get("COMBAT_SIDE_MYSQL", r"C:\Program Files\MySQL\MySQL Server 8.4\bin\mysql.exe"))
MYSQL_PORT = os.environ.get("COMBAT_SIDE_MYSQL_PORT", "3317")
PASSWORD = os.environ.get("COMBAT_SIDE_PASSWORD", "combatside")
CHARACTERS = [1, 2]
USERS = [1, 2]
SKILLS = [(2, 1, "Skill", 0), (10955, 1, "Skill", 0), (18132, 1, "Skill", 0)]
ACTION_BAR = {1: 18132, 2: 10955, 3: 2}
MAX_ACTION_SLOTS = 85


def lane() -> dict[str, str]:
    values: dict[str, str] = {}
    for line in LANE_ENV.read_text(encoding="utf-8-sig").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def mysql(sql: str, database: str | None = None) -> list[list[str]]:
    command = [str(MYSQL), "--host=127.0.0.1", f"--port={MYSQL_PORT}", "--user=root",
               "--protocol=TCP", "--default-character-set=utf8mb4", "--batch", "--raw",
               "--skip-column-names"]
    if database:
        command.append(database)
    command += ["-e", sql]
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=60)
    if result.returncode != 0:
        raise SystemExit(f"mysql failed: {result.stderr.strip()}")
    rows = []
    for line in result.stdout.splitlines():
        if line:
            rows.append(line.split("\t"))
    return rows


def parse_slots(hex_text: str) -> list[tuple[int, int]]:
    data = bytes.fromhex("" if hex_text == "NULL" else hex_text)
    slots: list[tuple[int, int]] = []
    index = 0
    for _ in range(MAX_ACTION_SLOTS):
        kind = data[index] if index < len(data) else 0
        index += 1
        value = 0
        if kind in (1, 2, 5):
            value = int.from_bytes(data[index:index + 4].ljust(4, b"\0"), "little")
            index += 4
        elif kind == 4:
            value = int.from_bytes(data[index:index + 8].ljust(8, b"\0"), "little")
            index += 8
        slots.append((kind, value))
    return slots


def build_slots(bar: dict[int, int]) -> str:
    out = bytearray()
    for slot in range(MAX_ACTION_SLOTS):
        action = bar.get(slot)
        if action is None:
            out.append(0)
        else:
            out.append(2)  # ActionSlotType::Spell
            out += int(action).to_bytes(4, "little")
    return out.hex().upper()


def char_list() -> str:
    return ",".join(str(v) for v in CHARACTERS)


def user_list() -> str:
    return ",".join(str(v) for v in USERS)


def snapshot(path: Path) -> None:
    values = lane()
    game = values["DB_GAME"]
    login = values["DB_LOGIN"]
    character_rows = mysql(
        "SELECT id,account_id,name,x,y,z,yaw,world_id,zone_id,hp,mp,level,experience,"
        f"recoverable_exp,HEX(slots) FROM characters WHERE id IN ({char_list()}) ORDER BY id", game)
    skill_rows = mysql(
        f"SELECT id,level,type,owner FROM skills WHERE owner IN ({char_list()}) ORDER BY owner,id", game)
    user_rows = mysql(
        "SELECT id,username,HEX(password),last_login,last_ip,created_at,updated_at,banned,ban_reason "
        f"FROM users WHERE id IN ({user_list()}) ORDER BY id", login)
    data = {
        "lane": values.get("LANE"),
        "game_db": game,
        "login_db": login,
        "characters": character_rows,
        "skills": skill_rows,
        "users": user_rows,
        "applied_password_sha256": hashlib.sha256(PASSWORD.encode()).hexdigest(),
        "applied_password_base64": base64.b64encode(hashlib.sha256(PASSWORD.encode()).digest()).decode(),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=1) + "\n", encoding="utf-8")
    print(f"snapshot: {len(character_rows)} characters, {len(skill_rows)} skills, "
          f"{len(user_rows)} users -> {path}")


def load(path: Path) -> dict:
    if not path.exists():
        raise SystemExit(f"missing snapshot {path}; run snapshot first")
    return json.loads(path.read_text(encoding="utf-8"))


def apply(path: Path) -> None:
    data = load(path)
    game, login = data["game_db"], data["login_db"]
    password_b64 = data.get("applied_password_base64") or base64.b64encode(
        hashlib.sha256(PASSWORD.encode()).digest()).decode()
    mysql(f"UPDATE users SET password = '{password_b64}' WHERE id IN ({user_list()})", login)
    mysql(f"DELETE FROM skills WHERE owner IN ({char_list()})", game)
    rows = []
    for owner in CHARACTERS:
        for skill_id, level, kind, _ in SKILLS:
            rows.append(f"({skill_id},{level},'{kind}',{owner})")
    if rows:
        mysql("INSERT INTO skills (id,level,type,owner) VALUES " + ",".join(rows), game)
    blob = build_slots(ACTION_BAR)
    mysql(
        "UPDATE characters SET "
        f"level=30,hp=5000,mp=4000,experience=0,recoverable_exp=0,"
        f"ability1=1,ability2=10,ability3=11,"
        f"slots=UNHEX('{blob}') WHERE id IN ({char_list()})", game)
    print(f"applied: skills {SKILLS} bar {ACTION_BAR}, level 30 both, "
          f"password set for users {USERS}")


def sql_string(value: str) -> str:
    if value == "NULL":
        return "NULL"
    return "'" + value.replace("\\", "\\\\").replace("'", "''") + "'"


def restore(path: Path) -> None:
    data = load(path)
    game, login = data["game_db"], data["login_db"]
    for row in data["characters"]:
        (cid, account_id, name, x, y, z, yaw, world_id, zone_id, hp, mp, level, experience,
         recoverable_exp, slots_hex) = row
        mysql(
            "UPDATE characters SET "
            f"x={x},y={y},z={z},yaw={yaw},world_id={world_id},zone_id={zone_id},"
            f"hp={hp},mp={mp},level={level},experience={experience},"
            f"recoverable_exp={recoverable_exp},slots=UNHEX('{slots_hex}') "
            f"WHERE id={cid} AND account_id={account_id}", game)
    mysql(f"DELETE FROM skills WHERE owner IN ({char_list()})", game)
    if data["skills"]:
        values = ",".join(f"({sid},{level},'{kind}',{owner})" for sid, level, kind, owner in data["skills"])
        mysql("INSERT INTO skills (id,level,type,owner) VALUES " + values, game)
    for row in data["users"]:
        (uid, username, password_hex, last_login, last_ip, created_at, updated_at, banned,
         ban_reason) = row
        mysql(
            "UPDATE users SET "
            f"password=UNHEX('{password_hex}'),last_login={last_login},"
            f"last_ip={sql_string(last_ip)},created_at={created_at},updated_at={updated_at},"
            f"banned={banned},ban_reason={ban_reason} WHERE id={uid}", login)
    print(f"restored: {len(data['characters'])} characters, {len(data['skills'])} skills, "
          f"{len(data['users'])} users")


def apply_slot(slot: str, values: dict[str, str]) -> None:
    global CHARACTERS, USERS, PASSWORD
    first = live_slots.resolve(slot, "combat-golden", "a")
    second = live_slots.resolve(slot, "combat-golden", "b")
    CHARACTERS = [first.character_id, second.character_id]
    rows = mysql(
        "SELECT id FROM users WHERE username IN "
        f"({sql_string(first.user)},{sql_string(second.user)}) ORDER BY id",
        values["DB_LOGIN"])
    USERS = [int(row[0]) for row in rows]
    if len(USERS) != 2:
        raise SystemExit(f"slot {slot}: expected two login users, found {USERS}")
    password_file = Path(os.environ.get(
        "COMBAT_SIDE_PASSWORD_FILE",
        str(Path(values.get("LIVE", "D:/aaemu-live-combat-golden")) / "cap-pw.txt")))
    PASSWORD = password_file.read_text(encoding="utf-8-sig").strip()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["snapshot", "apply", "restore"])
    parser.add_argument("path", type=Path)
    parser.add_argument("--slot", default=os.environ.get("COMBAT_SIDE_SLOT"),
                        help="worker slot; targets the slot's two characters/accounts")
    args = parser.parse_args()
    if args.slot:
        apply_slot(args.slot, lane())
    {"snapshot": snapshot, "apply": apply, "restore": restore}[args.command](args.path)


if __name__ == "__main__":
    main()
