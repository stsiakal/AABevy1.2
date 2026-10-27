#!/usr/bin/env python3
"""Drive the hidden 1.2 client (aaemu-clientdriver MCP) and collect packet evidence.

Attaches to an already running client by pid.  This is the small tool used to place a
ground skill on the action bar, press its key, click a ground point and save a screenshot.

Examples:
    python tools/position-probe/probe.py lua 9000 "return X2Unit:UnitName('player')"
    python tools/position-probe/probe.py key 9000 4
    python tools/position-probe/probe.py move 9000 900 700
    python tools/position-probe/probe.py click 9000 900 700
    python tools/position-probe/probe.py shot 9000 shot.png
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import live_slots  # noqa: E402

RUN = Path(os.environ.get("PROBE_RUN", str(Path(os.environ["LOCALAPPDATA"]) / "Temp" / "opencode" / "position-probe")))
DRIVER = Path(os.environ.get("PROBE_DRIVER", "D:/aaemu-target/combat-golden/live/aaemu-clientdriver.exe"))
DLL = Path(os.environ.get("PROBE_DLL", "D:/aaemu-target/combat-golden/aadriver/aadriver.dll"))


class Mcp:
    def __init__(self, driver: Path = DRIVER, dll: Path = DLL) -> None:
        self.process = subprocess.Popen(
            [str(driver), "mcp", "--dll", str(dll), "--artifact-dir", str(RUN / "mcp")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, encoding="utf-8",
        )
        self.sequence = 0
        self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                 "clientInfo": {"name": "position-probe", "version": "1"}})
        assert self.process.stdin and self.process.stdout
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.process.stdin.flush()

    def call(self, method: str, params: object = None) -> object:
        assert self.process.stdin and self.process.stdout
        self.sequence += 1
        request: dict[str, object] = {"jsonrpc": "2.0", "id": self.sequence, "method": method}
        if params is not None:
            request["params"] = params
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError("MCP closed stdout")
        response = json.loads(line)
        if response.get("id") != self.sequence:
            raise RuntimeError(f"MCP id mismatch: {response}")
        if "error" in response:
            raise RuntimeError(str(response["error"]))
        return response["result"]

    def tool(self, name: str, **arguments: object) -> dict:
        result = self.call("tools/call", {"name": name, "arguments": arguments})
        if result.get("isError"):
            raise RuntimeError("\n".join(c.get("text", "") for c in result.get("content", [])))
        return result

    def text(self, name: str, **arguments: object) -> str:
        return "\n".join(c["text"] for c in self.tool(name, **arguments)["content"] if c["type"] == "text")

    def close(self) -> None:
        try:
            assert self.process.stdin
            self.process.stdin.close()
            self.process.wait(timeout=10)
        except Exception:
            self.process.kill()


def lua(mcp: Mcp, pid: int, code: str, target: str = "ui") -> str:
    return mcp.text("client_lua", pid=pid, target=target, code=code)


def key(mcp: Mcp, pid: int, name: str, duration_ms: int = 80) -> str:
    return mcp.text("client_input", pid=pid, key=name, duration_ms=duration_ms)


def pointer(mcp: Mcp, pid: int, action: str, x: int, y: int) -> str:
    return mcp.text("client_pointer", pid=pid, action=action, x=x, y=y)


def screenshot(mcp: Mcp, pid: int, path: Path) -> Path:
    content = mcp.tool("client_screenshot", pid=pid)["content"]
    image = next(c for c in content if c["type"] == "image")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(base64.b64decode(image["data"], validate=True))
    return path


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["lua", "key", "move", "click", "right_click", "shot", "chat"])
    ap.add_argument("pid", type=int)
    ap.add_argument("args", nargs="*")
    ap.add_argument("--slot", default=os.environ.get("PROBE_SLOT"),
                    help="worker slot (wp1..wp6, f7, interaction, combat, spare1..3)")
    ap.add_argument("--lane", choices=sorted(live_slots.LANES),
                    help="lane of the running client (default: from PROBE_DRIVER)")
    ns = ap.parse_args()
    driver, dll = DRIVER, DLL
    if ns.slot:
        lane = ns.lane or live_slots.lane_for_path(str(DRIVER)) or "combat-golden"
        account = live_slots.resolve(ns.slot, lane, "a")
        print(f"slot {account.slot}: {account.lane} {account.user} / {account.character} "
              f"(pid {ns.pid})", file=sys.stderr)
        driver = Path(f"D:/aaemu-target/{lane}/live/aaemu-clientdriver.exe")
        dll = Path(f"D:/aaemu-target/{lane}/aadriver/aadriver.dll")
    mcp = Mcp(driver, dll)
    try:
        if ns.command == "lua":
            print(lua(mcp, ns.pid, " ".join(ns.args)))
        elif ns.command == "key":
            print(key(mcp, ns.pid, ns.args[0], int(ns.args[1]) if len(ns.args) > 1 else 80))
        elif ns.command in ("move", "click", "right_click"):
            print(pointer(mcp, ns.pid, ns.command, int(ns.args[0]), int(ns.args[1])))
        elif ns.command == "shot":
            print(screenshot(mcp, ns.pid, Path(ns.args[0])))
        elif ns.command == "chat":
            print(mcp.text("client_chat", pid=ns.pid, message=" ".join(ns.args)))
    finally:
        mcp.close()


if __name__ == "__main__":
    main()
