#!/usr/bin/env python3
"""Start/stop a trace-enabled copy of the combat-golden lane stack on spare ports.

The running lane stack (29xxx) is never touched.  This copy reuses the lane's runtime
configs, game data and databases but writes to a temporary directory and listens on
293xx.  With RUST_LOG=info,aaemu_game::net::fields=trace the game server logs every
client packet's complete body and the decoded CSStartSkillPacket fields.

Usage:
    python tools/position-probe/stack.py up
    python tools/position-probe/stack.py down
    python tools/position-probe/stack.py status
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

LANE = Path(os.environ.get("PROBE_LANE", "D:/aaemu-wt/combat-golden"))
SRC = Path(os.environ.get("PROBE_LIVE_SRC", "D:/aaemu-live-combat-golden"))
RUN = Path(os.environ.get("PROBE_RUN", str(Path(os.environ["LOCALAPPDATA"]) / "Temp" / "opencode" / "position-probe")))
STACK = RUN / "live"
STATE = RUN / "stack.json"
LOGS = RUN / "logs"

PORTS = {
    "login": 29337,
    "internal": 29334,
    "game": 29339,
    "stream": 29350,
    "web": 29380,
}


def log(*a: object) -> None:
    print("stack:", *a, flush=True)


def port_open(port: int) -> bool:
    with socket.socket() as s:
        s.settimeout(0.3)
        return s.connect_ex(("127.0.0.1", port)) == 0


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def junction(link: Path, target: Path) -> None:
    if link.exists():
        return
    link.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cmd", "/c", "mklink", "/J", str(link), str(target)],
                   check=True, capture_output=True, text=True)


def prepare() -> None:
    LOGS.mkdir(parents=True, exist_ok=True)
    for name in ("rs-login", "rs-game"):
        (STACK / name).mkdir(parents=True, exist_ok=True)
        shutil.copy2(SRC / name / "Config.json", STACK / name / "Config.json")
    for name in ("Configurations", "Scripts", "Data"):
        junction(STACK / "rs-game" / name, SRC / "rs-game" / name)

    login = read_json(SRC / "rs-login" / "Config.Local.json")
    login["Network"]["Port"] = PORTS["login"]
    login["InternalNetwork"]["Port"] = PORTS["internal"]
    for server in login["GameServers"]:
        server["Port"] = PORTS["game"]
    write_json(STACK / "rs-login" / "Config.Local.json", login)

    game = read_json(SRC / "rs-game" / "Config.Local.json")
    game["Network"]["Port"] = PORTS["game"]
    game["StreamNetwork"]["Port"] = PORTS["stream"]
    game["WebApiNetwork"]["Port"] = PORTS["web"]
    game["LoginNetwork"]["Port"] = str(PORTS["internal"])
    write_json(STACK / "rs-game" / "Config.Local.json", game)


def start(name: str, exe: Path, rlog: str, extra: list[str] | None = None) -> subprocess.Popen:
    out = LOGS / f"{name}.log"
    env = dict(os.environ, RUST_LOG=rlog, NO_COLOR="1")
    cmd = [str(exe), *(extra or [])]
    if name == "rs-game":
        cmd.append(str(STACK / "rs-game"))
    handle = out.open("wb")
    proc = subprocess.Popen(cmd, cwd=str(STACK / name), env=env,
                            stdout=handle, stderr=subprocess.STDOUT,
                            creationflags=getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0))
    log(name, "pid", proc.pid, "->", out)
    return proc


def wait_registered(timeout: float = 120.0) -> None:
    path = LOGS / "rs-game.log"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            text = path.read_text(encoding="utf-8", errors="replace")
            if "Successfully registered on LoginServer" in text:
                return
            if "panicked" in text or "fatal" in text.lower():
                raise SystemExit(f"game server failed, see {path}")
        time.sleep(0.5)
    raise SystemExit(f"timed out waiting for the game server, see {path}")


def up() -> None:
    for port in PORTS.values():
        if port_open(port):
            raise SystemExit(f"port {port} is already in use; refusing to interfere")
    prepare()
    login = start("rs-login", SRC / "bin" / "aaemu-login.exe", "info")
    game = start("rs-game", SRC / "bin" / "aaemu-game.exe",
                 "info,aaemu_game::net::fields=trace",
                 extra=[])
    wait_registered()
    STATE.write_text(json.dumps({"login": login.pid, "game": game.pid,
                                 "ports": PORTS}, indent=2) + "\n", encoding="utf-8")
    log("up: login 127.0.0.1:%d, trace game %d, log %s" % (PORTS["login"], PORTS["game"], LOGS / "rs-game.log"))


def down() -> None:
    if not STATE.exists():
        log("no stack state")
        return
    state = read_json(STATE)
    for key in ("game", "login"):
        pid = state.get(key)
        if pid:
            subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True, timeout=30)
    STATE.unlink(missing_ok=True)
    log("down")


def status() -> None:
    state = read_json(STATE) if STATE.exists() else {}
    for name, port in PORTS.items():
        log(f"{name} port {port} open={port_open(port)}")
    if state:
        log("pids", state.get("login"), state.get("game"), "log", LOGS / "rs-game.log")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["up", "down", "status"])
    args = ap.parse_args()
    {"up": up, "down": down, "status": status}[args.command]()


if __name__ == "__main__":
    main()
