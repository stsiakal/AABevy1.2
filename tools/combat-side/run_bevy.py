#!/usr/bin/env python3
"""Run the Bevy client for the combat side-by-side acceptance and keep its logs.

The Bevy client uses its script runner (``--script``): spawn the same fox_test
template, target it, cast slot 1 (Triple Slash), cast slot 2 (Freezing Arrow),
then repeat slot 3 (Melee Attack) every 1.5 s until the target dies; wait ten
seconds for the stance flip and end the feel recording.

``AA_COMBAT_LOG=1`` / ``AA_COMBAT_ANIM_LOG=1`` write the per-frame scheduler and
event lines; ``--feel-csv`` writes the local player's state and clip time.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_slots  # noqa: E402

DEFAULT_EXE = Path(r"D:/aaemu-target/bevy-farm-visual/debug/aa-client.exe")
DEFAULT_DB = Path(r"D:/aaemu-live-combat-golden/rs-game/Data/compact.sqlite3")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--login", default="127.0.0.1:29237")
    parser.add_argument("--slot", default=os.environ.get("COMBAT_SIDE_SLOT"),
                        help="worker slot; sets --user/--character to the slot's Bevy account")
    parser.add_argument("--user", default="combatgold")
    parser.add_argument("--character", default="Goldone")
    parser.add_argument("--password-file", type=Path, required=True)
    parser.add_argument("--server-log", type=Path, required=True)
    parser.add_argument("--exe", type=Path, default=DEFAULT_EXE)
    parser.add_argument("--game-db", type=Path, default=DEFAULT_DB)
    parser.add_argument("--npc", type=int, default=3968)
    parser.add_argument("--pos", default="15598.5,15264.3,137.133")
    parser.add_argument("--timeout", type=float, default=600.0)
    args = parser.parse_args()
    if args.slot:
        account = live_slots.resolve(args.slot, "combat-golden", "b")
        args.user = account.user
        args.character = account.character
        offset = live_slots.slot_index(args.slot) * 8.0
        x, y, z = (float(v) for v in args.pos.split(","))
        args.pos = f"{x + offset:.1f},{y + offset * 0.5:.1f},{z:.1f}"
    run: Path = args.run_dir
    run.mkdir(parents=True, exist_ok=True)

    script = run / "bevy-script.txt"
    script.write_text(
        "\n".join([
            "feel SETUP",
            "lua if tutorial and tutorial.window then tutorial.window:Show(false) end",
            f"gm despawn npc {args.npc} 30",
            "wait 1",
            f"gm spawn npc {args.npc}",
            "wait 2",
            f"target template:{args.npc}",
            "walk to current",
            "wait 1",
            "feel START",
            "wait 1",
            "cast slot 1",
            "wait 2.5",
            "cast slot 2",
            "wait 0.2",
            "fight slot 3 current 20",
            "wait 15",
            "feel END",
            "wait 0.3",
            "quit now",
        ]) + "\n", encoding="utf-8")

    csv = run / "feel.csv"
    log = run / "bevy.log"
    server_start = args.server_log.stat().st_size if args.server_log.exists() else 0
    cmd = [
        str(args.exe), "--headless", "--mute",
        "--account", args.user, "--login", args.login,
        "--password-file", str(args.password_file), "--character", args.character,
        "--pos", args.pos,
        "--script", str(script),
        "--feel-csv", str(csv), "--feel-fps", "0",
        "--bar-skill", "1", "18132",
        "--bar-skill", "2", "10955",
        "--bar-skill", "3", "2",
        "--size", "1280x720", "--headless-fps", "120",
    ]
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = r"D:/aaemu-target/bevy-farm-visual"
    env["AA_DB"] = str(args.game_db)
    env["AA_COMBAT_LOG"] = "1"
    env["AA_COMBAT_ANIM_LOG"] = "1"
    print("bevy:", " ".join(cmd), flush=True)
    started = time.time()
    with log.open("w", encoding="utf-8") as handle:
        proc = subprocess.run(cmd, cwd=str(Path(__file__).resolve().parents[2]),
                              env=env, stdout=handle, stderr=subprocess.STDOUT,
                              timeout=args.timeout)
    elapsed = time.time() - started
    server_end = args.server_log.stat().st_size if args.server_log.exists() else 0
    if server_end > server_start:
        with args.server_log.open("rb") as f:
            f.seek(server_start)
            (run / "bevy_server_slice.log").write_bytes(f.read(server_end - server_start))
    (run / "bevy_meta.json").write_text(json.dumps({
        "exe": str(args.exe), "returncode": proc.returncode, "seconds": elapsed,
        "account": args.user, "character": args.character, "npc": args.npc,
        "script": script.name,
    }, indent=1) + "\n", encoding="utf-8")
    print("bevy: exit", proc.returncode, "in %.1f s -> %s" % (elapsed, log), flush=True)
    if proc.returncode != 0:
        tail = log.read_text(encoding="utf-8", errors="replace").splitlines()[-20:]
        raise SystemExit("bevy failed:\n" + "\n".join(tail))


if __name__ == "__main__":
    main()
