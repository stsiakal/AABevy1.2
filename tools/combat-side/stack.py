#!/usr/bin/env python3
"""Start/stop the trace-enabled copy of the combat-golden lane stack (ports 293xx).

The lane's own stack on 29xxx is never touched: this copy reuses its runtime
configs, game data and databases but writes to the probe run directory and
listens on 293xx.  With the extra tracing the game server logs

* every client packet body (`aaemu_game::net::fields=trace`), and
* every server packet (`aaemu_game::net::connection=trace`),

so the CSStartSkill bytes and the SC replies of each cast are in one log.

Usage:
    python tools/combat-side/stack.py up [--run-dir DIR]
    python tools/combat-side/stack.py down [--run-dir DIR]
    python tools/combat-side/stack.py status [--run-dir DIR]
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import time
from pathlib import Path

LANE = Path(os.environ.get("COMBAT_SIDE_LANE", "D:/aaemu-wt/combat-golden"))
SRC = Path(os.environ.get("COMBAT_SIDE_LIVE_SRC", "D:/aaemu-live-combat-golden"))
DEFAULT_RUN = Path(os.environ.get("COMBAT_SIDE_RUN", "shots/combat-side"))

PORTS = {
    "login": 29337,
    "internal": 29334,
    "game": 29339,
    "stream": 29350,
    "web": 29380,
}
RUST_LOG = "info,aaemu_game::net::fields=trace,aaemu_game::net::connection=trace"


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


def prepare(run: Path) -> None:
    stack = run / "stack"
    logs = run / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    for name in ("rs-login", "rs-game"):
        (stack / name).mkdir(parents=True, exist_ok=True)
        shutil.copy2(SRC / name / "Config.json", stack / name / "Config.json")
    for name in ("Configurations", "Scripts", "Data"):
        junction(stack / "rs-game" / name, SRC / "rs-game" / name)

    login = read_json(SRC / "rs-login" / "Config.Local.json")
    login["Network"]["Port"] = PORTS["login"]
    login["InternalNetwork"]["Port"] = PORTS["internal"]
    for server in login["GameServers"]:
        server["Port"] = PORTS["game"]
    write_json(stack / "rs-login" / "Config.Local.json", login)

    game = read_json(SRC / "rs-game" / "Config.Local.json")
    game["Network"]["Port"] = PORTS["game"]
    game["StreamNetwork"]["Port"] = PORTS["stream"]
    game["WebApiNetwork"]["Port"] = PORTS["web"]
    game["LoginNetwork"]["Port"] = str(PORTS["internal"])
    write_json(stack / "rs-game" / "Config.Local.json", game)


def start(name: str, exe: Path, run: Path) -> subprocess.Popen:
    out = run / "logs" / f"{name}.log"
    env = dict(os.environ, RUST_LOG=RUST_LOG, NO_COLOR="1")
    cmd = [str(exe)]
    if name == "rs-game":
        cmd.append(str(run / "stack" / "rs-game"))
    handle = out.open("wb")
    proc = subprocess.Popen(cmd, cwd=str(run / "stack" / name), env=env,
                            stdout=handle, stderr=subprocess.STDOUT,
                            creationflags=getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0))
    log(name, "pid", proc.pid, "->", out)
    return proc


def wait_registered(run: Path, timeout: float = 180.0) -> None:
    path = run / "logs" / "rs-game.log"
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


def up(run: Path) -> None:
    for port in PORTS.values():
        if port_open(port):
            raise SystemExit(f"port {port} is already in use; refusing to interfere")
    prepare(run)
    login = start("rs-login", SRC / "bin" / "aaemu-login.exe", run)
    game = start("rs-game", SRC / "bin" / "aaemu-game.exe", run)
    wait_registered(run)
    write_json(run / "stack.json", {"login": login.pid, "game": game.pid, "ports": PORTS})
    log("up: login 127.0.0.1:%d, trace game %d, log %s"
        % (PORTS["login"], PORTS["game"], run / "logs" / "rs-game.log"))


def down(run: Path) -> None:
    state = run / "stack.json"
    if not state.exists():
        log("no stack state")
        return
    data = read_json(state)
    for key in ("game", "login"):
        pid = data.get(key)
        if pid:
            subprocess.run(["taskkill", "/PID", str(pid), "/F"],
                           capture_output=True, timeout=30)
    state.unlink(missing_ok=True)
    log("down")


def status(run: Path) -> None:
    state = read_json(run / "stack.json") if (run / "stack.json").exists() else {}
    for name, port in PORTS.items():
        log(f"{name} port {port} open={port_open(port)}")
    if state:
        log("pids", state.get("login"), state.get("game"),
            "log", run / "logs" / "rs-game.log")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["up", "down", "status"])
    parser.add_argument("--run-dir", type=Path, default=DEFAULT_RUN)
    args = parser.parse_args()
    args.run_dir.mkdir(parents=True, exist_ok=True)
    {"up": up, "down": down, "status": status}[args.command](args.run_dir)


if __name__ == "__main__":
    main()
