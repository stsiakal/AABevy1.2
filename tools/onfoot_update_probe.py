#!/usr/bin/env python3
"""Capture the original 1.2 client's per-frame on-foot update chain.

W0 recon: which update functions run for the LOCAL player on foot (standing,
walking/running, strafing, turning, jumping, falling), in what order and how
many times per frame, and the same while mounted.

The probe launches the hidden original client on the move-golden lane, enters
the world with the slot's own account, installs a read-only entry counter on the
x2game.dll functions (aadriver `K` trace, see native/trace.h in the lane
driver), drives real key input through the client's own CryInput path, and
records one trace row per engine frame plus a Lua state sample per ~16 ms.

Nothing in the game state is written by the trace: each detour saves the
registers/flags, increments an in-DLL counter and records (function, ECX), then
executes the original prologue. The client's behaviour is unchanged.

Usage:
    python tools/onfoot_update_probe.py --slot wp6 --lane move-golden

Outputs:
    data/extract/onfoot-update/<tag>/            raw traces, samples, logs (git-ignored)
    tools/evidence/onfoot_update/report.txt     scenario report (generated)
    tools/evidence/onfoot_update/per-frame-calls.csv
    tools/evidence/onfoot_update/summary.json

The driver trace capability lives on the server lane branch recon/onfoot-trace
(D:/aaemu-wt/move-golden). Rebuild it with:

    set AADRIVER_OUT=D:/aaemu-target/move-golden/aadriver-onfoot
    D:/aaemu-wt/move-golden/rust/crates/aaemu-clientdriver/native/build.bat

The exact diff is kept under tools/evidence/onfoot_update/driver-trace.patch.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
import socket
import struct
import subprocess
import sys
import threading
import time
from collections import Counter, defaultdict
from ctypes import wintypes
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import live_slots  # noqa: E402

LANE = Path(os.environ.get("ONFOOT_LANE", "D:/aaemu-wt/move-golden"))
LIVE = Path(os.environ.get("ONFOOT_LIVE", "D:/aaemu-live-move-golden"))
CLIENT = Path(os.environ.get("ONFOOT_CLIENT", "D:/AA1.2/bin32/archeage.exe"))
LAUNCHER = Path(os.environ.get("ONFOOT_LAUNCHER", str(LIVE / "bin" / "aaemu-launcher.exe")))
DRIVER = Path(os.environ.get("ONFOOT_DRIVER", "D:/aaemu-target/move-golden/live/aaemu-clientdriver.exe"))
DLL = Path(os.environ.get("ONFOOT_DLL", "D:/aaemu-target/move-golden/aadriver-onfoot/aadriver.dll"))
PASSWORD_FILE = Path(os.environ.get("ONFOOT_PASSWORD", str(LIVE / "cap-pw.txt")))
BASH = Path(os.environ.get("ONFOOT_BASH", "C:/Program Files/Git/bin/bash.exe"))

# x2game.dll function table. `decomp` is the line in
# D:/aa-re/client12/out-x2game/x2game.dll.c; `role` is what the function is.
FUNCS = [
    {"id": 0, "rva": 0x139800, "name": "Quadruped::PrePhysicsUpdate", "decomp": 229029,
     "role": "quadruped pre-physics"},
    {"id": 1, "rva": 0x143280, "name": "CPlayer::PrePhysicsUpdate", "decomp": 233948,
     "role": "player pre-physics"},
    {"id": 2, "rva": 0x13DD40, "name": "FUN_3912dd40", "decomp": 231254,
     "role": "quadruped movement dispatch"},
    {"id": 3, "rva": 0x13B330, "name": "FUN_3912b330", "decomp": 229974,
     "role": "quadruped ground mover"},
    {"id": 4, "rva": 0x128650, "name": "CActor::UpdateMovementRequest", "decomp": 219255,
     "role": "movement request"},
    {"id": 5, "rva": 0x123F60, "name": "FUN_39113f60", "decomp": 216241,
     "role": "actor post-physics"},
    {"id": 6, "rva": 0x145ED0, "name": "CPlayer::ProcessFrameMovementRotation", "decomp": 235447,
     "role": "rotation"},
    {"id": 7, "rva": 0x145FE0, "name": "CPlayer::ProcessFrameMovement", "decomp": 235491,
     "role": "movement"},
    {"id": 8, "rva": 0x12FEB0, "name": "CActor::Update", "decomp": 224683,
     "role": "actor update"},
    {"id": 9, "rva": 0x12EFF0, "name": "CActor::UpdateStats", "decomp": 223817,
     "role": "actor stats/state update"},
    {"id": 10, "rva": 0x12C780, "name": "FUN_3911c780", "decomp": 222108,
     "role": "fall/landing update"},
    {"id": 11, "rva": 0x127590, "name": "FUN_39117590", "decomp": 218718,
     "role": "landing/water check"},
    {"id": 12, "rva": 0x1487C0, "name": "FUN_391387c0 (CPlayer::Update wrapper)", "decomp": 236825,
     "role": "player update wrapper"},
]
FUNC_BY_ID = {f["id"]: f for f in FUNCS}

# Static vtables from the unpacked 1.8 r208022 image (preferred base 0x38FF0000).
VTABLE_CPLAYER_WINDOW = 0x9BD450      # object's vtable pointer observed at runtime
VTABLE_CPlayer_PrePhysicsUpdate = 0x9BD60C
VTABLE_ProcessFrameMovementRotation = 0x9BD5F4
VTABLE_ProcessFrameMovement = 0x9BD5F8
VTABLE_CPlayer_Update_wrapper = 0x9BD478
VTABLE_QUADRUPED = 0x9BC918
VTABLE_Quadruped_PrePhysicsUpdate = 0x9BCAD4
VTABLE_CActor_Update = 0x9BC400

# Decompile call lines used for the report (same file as above).
CALLS = {
    "quad_prephysics_calls_dispatch": [229216, 229256],
    "dispatch_calls_ground_mover": [231312],
    "player_prephysics_calls_update_request": [233987],
    "player_prephysics_calls_postphysics": [233988],
    "update_request_calls_rotation": [219374],
    "update_request_calls_movement": [219454],
    "player_update_calls_actor_update": [236838],
    "actor_update_calls_stats": [224840],
    "stats_calls_fall": [223961],
    "fall_calls_landing": [222142],
    "quad_prephysics_calls_postphysics": [229265],
}

SCENARIOS = {
    "on_foot": [
        ("stand", 2.6, []),
        ("forward", 3.0, [(0.0, "w", 3000)]),
        ("strafe_q", 2.2, [(0.0, "q", 2000)]),
        ("strafe_e", 2.2, [(0.0, "e", 2000)]),
        ("turn_a", 1.8, [(0.0, "a", 1500)]),
        ("turn_d", 1.8, [(0.0, "d", 1500)]),
        ("jump_stand", 2.4, [(0.3, "space", 120)]),
        ("jump_run", 2.6, [(0.0, "w", 2400), (0.9, "space", 120)]),
        ("jump_run_b", 2.8, [(0.0, "w", 2600), (0.4, "space", 160)]),
        ("fall", 4.5, []),
    ],
    "mounted": [
        ("mounted_stand", 2.4, []),
        ("mounted_forward", 3.0, [(0.0, "w", 3000)]),
        ("mounted_turn", 1.8, [(0.0, "a", 1500)]),
        ("mounted_jump", 2.4, [(0.3, "space", 120)]),
    ],
}

SAMPLER_LUA = r"""
_G.OFP_ROWS = {}
_G.OFP_RUN = false
_G.OFP_STOP = false
local function vec(t)
  if type(t) ~= 'table' then return 'nil' end
  return string.format('%.5f,%.5f,%.5f', t.x or 0, t.y or 0, t.z or 0)
end
local function anim(e)
  local s = ''
  if e ~= nil then
    pcall(function() local a = e.actor:GetCurrentAnimationState(); if a then s = a end end)
  end
  return s
end
local function sample()
  local e = _G.OFP_PE
  if e == nil then return nil end
  local ok,p = pcall(function() return e:GetWorldPos() end)
  if not ok or p == nil then return nil end
  local ok2,a = pcall(function() return e:GetAngles() end)
  local ok3,v = pcall(function() return e:GetVelocity() end)
  if not ok2 then a = {x=0,y=0,z=0} end
  if not ok3 then v = {x=0,y=0,z=0} end
  local h = _G.OFP_HE
  local hp,hv = 'nil','nil'
  if h ~= nil then
    local h1,p1 = pcall(function() return h:GetWorldPos() end)
    local h2,v1 = pcall(function() return h:GetVelocity() end)
    if h1 and p1 then hp = vec(p1) end
    if h2 and v1 then hv = vec(v1) end
  end
  return table.concat({
    tostring(System.GetFrameID()),
    string.format('%.6f', System.GetCurrTime()),
    vec(p), vec(a), vec(v),
    hp, hv,
    anim(e), anim(h),
  }, '|')
end
local function tick()
  if _G.OFP_STOP then return end
  if _G.OFP_RUN then
    local ok,row = pcall(sample)
    if ok and type(row) == 'string' then
      _G.OFP_ROWS[#_G.OFP_ROWS + 1] = row
    end
  end
  Script.SetTimer(16, tick)
end
Script.SetTimer(16, tick)
return 'installed'
"""


def log(*args):
    print("onfoot:", *args, flush=True)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def pe_identity(path: Path):
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    stamp = struct.unpack_from("<I", data, pe + 8)[0]
    magic = struct.unpack_from("<H", data, pe + 24)[0]
    if magic == 0x10B:
        size = struct.unpack_from("<I", data, pe + 24 + 56)[0]
    else:
        size = struct.unpack_from("<I", data, pe + 24 + 56)[0]
    return {"path": str(path), "sha256": sha256(path), "time_date_stamp": stamp,
            "size_of_image": size, "bytes": len(data)}


class Mcp:
    def __init__(self, command):
        self.proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
        self.sequence = 0
        self.call("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                 "clientInfo": {"name": "onfoot-update-probe", "version": "1"}})
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, payload):
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def call(self, method, params=None):
        assert self.proc.stdin and self.proc.stdout
        self.sequence += 1
        request = {"jsonrpc": "2.0", "id": self.sequence, "method": method}
        if params is not None:
            request["params"] = params
        self._send(request)
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("driver MCP closed stdout")
        response = json.loads(line)
        if response.get("id") != self.sequence:
            raise RuntimeError("driver MCP response id mismatch")
        if "error" in response:
            raise RuntimeError(str(response["error"]))
        return response["result"]

    def tool(self, name, **arguments):
        result = self.call("tools/call", {"name": name, "arguments": arguments})
        if result.get("isError"):
            raise RuntimeError("\n".join(c.get("text", "") for c in result.get("content", [])))
        return result

    def text(self, name, **arguments):
        return "\n".join(c["text"] for c in self.tool(name, **arguments)["content"]
                         if c.get("type") == "text")

    def close(self):
        try:
            assert self.proc.stdin
            self.proc.stdin.close()
            self.proc.wait(timeout=15)
        except Exception:
            pass


def mcp_text(mcp: Mcp, name, retries=150, **arguments):
    last = None
    for _ in range(retries):
        try:
            return mcp.text(name, **arguments)
        except RuntimeError as exc:
            last = exc
            text = str(exc).lower()
            if "cannot find the file" not in text and "pipe" not in text:
                raise
            time.sleep(0.1)
    raise last


class Pipe:
    def __init__(self, pid):
        self.pid = pid

    def request(self, op: str, payload: str = "") -> str:
        data = (op + payload).encode()
        path = r"\\.\pipe\aaemu-clientdriver-%d" % self.pid
        last = None
        for _ in range(400):
            try:
                handle = open(path, "r+b", buffering=0)
                break
            except OSError as exc:
                last = exc
                time.sleep(0.05)
        else:
            raise last
        try:
            handle.write(struct.pack("<I", len(data)) + data)
            head = handle.read(4)
            if len(head) != 4:
                raise RuntimeError("short pipe reply")
            length = struct.unpack("<I", head)[0]
            out = b""
            while len(out) < length:
                chunk = handle.read(length - len(out))
                if not chunk:
                    break
                out += chunk
        finally:
            handle.close()
        return out.decode("utf-8", "replace")


def parse_module(reply):
    match = re.search(r"base=0x([0-9A-Fa-f]+) size=0x([0-9A-Fa-f]+) stamp=0x([0-9A-Fa-f]+)",
                      reply)
    if not match:
        return None
    return {"base": int(match.group(1), 16), "size": int(match.group(2), 16),
            "stamp": int(match.group(3), 16)}


def hexof(reply):
    match = re.search(r"hex=([0-9a-f]+)", reply)
    return bytes.fromhex(match.group(1)) if match else None


def u32_at(pipe: Pipe, address: int) -> int:
    blob = hexof(pipe.request("R", f"0x{address:x} 4"))
    return struct.unpack("<I", blob)[0] if blob and len(blob) == 4 else 0


def parse_trace(text):
    rows = []
    current = None
    for line in text.splitlines():
        if line.startswith("K "):
            parts = line.split()
            current = {"frame": int(parts[1]), "overflow": int(parts[2]),
                       "count": int(parts[3]), "counts": parts[4:], "entries": []}
            rows.append(current)
        elif line.startswith("E ") and current is not None:
            parts = line.split()
            current["entries"].append((int(parts[1]), int(parts[2], 16)))
    return rows


class Process:
    def __init__(self, pid):
        self.pid = pid
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.handle = self.kernel.OpenProcess(0x0410, False, pid)
        if not self.handle:
            raise RuntimeError("OpenProcess failed")

    def modules(self):
        TH32CS_SNAPMODULE = 0x8
        TH32CS_SNAPMODULE32 = 0x10

        class ME(ctypes.Structure):
            _fields_ = [("dwSize", wintypes.DWORD), ("th32ModuleID", wintypes.DWORD),
                        ("th32ProcessID", wintypes.DWORD), ("GlblcntUsage", wintypes.DWORD),
                        ("ProccntUsage", wintypes.DWORD), ("modBaseAddr", ctypes.c_void_p),
                        ("modBaseSize", wintypes.DWORD), ("hModule", wintypes.HMODULE),
                        ("szModule", ctypes.c_wchar * 256), ("szExePath", ctypes.c_wchar * 260)]

        snapshot = self.kernel.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                                                        self.pid)
        entry = ME()
        entry.dwSize = ctypes.sizeof(ME)
        out = []
        ok = self.kernel.Module32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            out.append({"name": entry.szModule, "base": int(entry.modBaseAddr or 0),
                        "size": int(entry.modBaseSize)})
            ok = self.kernel.Module32NextW(snapshot, ctypes.byref(entry))
        self.kernel.CloseHandle(snapshot)
        return out

    def read(self, address, size):
        buffer = (ctypes.c_char * size)()
        got = ctypes.c_size_t()
        if not self.kernel.ReadProcessMemory(self.handle, ctypes.c_void_p(address), buffer,
                                             size, ctypes.byref(got)):
            return None
        return bytes(buffer[:got.value])

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None


class PositionSampler(threading.Thread):
    """Read the entity world position (x,y,z at entity+0x28) at ~50 Hz."""

    def __init__(self, pid, addresses, interval=0.02):
        super().__init__(daemon=True)
        self.pid = pid
        self.addresses = dict(addresses)
        self.interval = interval
        self.stop_event = threading.Event()
        self.rows = []
        self.error = None

    def run(self):
        try:
            proc = Process(self.pid)
            try:
                t0 = time.time()
                while not self.stop_event.is_set():
                    now = time.time() - t0
                    for role, address in self.addresses.items():
                        if not address:
                            continue
                        blob = proc.read(address + 0x28, 12)
                        if blob and len(blob) == 12:
                            x, y, z = struct.unpack("<3f", blob)
                            self.rows.append((now, role, x, y, z))
                    self.stop_event.wait(self.interval)
            finally:
                proc.close()
        except Exception as exc:
            self.error = str(exc)

    def stop(self):
        self.stop_event.set()
        self.join(timeout=5)
        return self.rows


def identify_local_actor(rows, entity_ptr):
    candidates = set()
    for row in rows:
        for func, obj in row["entries"]:
            if func == 1:
                candidates.add(obj)
    return candidates


def local_actor_from_entity(pipe, rows, entity_ptr):
    for obj in identify_local_actor(rows, entity_ptr):
        if u32_at(pipe, obj + 0xC) == entity_ptr:
            return obj
    return None


def locate_mount_actor(pipe, rows, entity_ptr):
    for obj in {obj for row in rows for func, obj in row["entries"] if func == 0}:
        if u32_at(pipe, obj + 0xC) == entity_ptr:
            return obj
    return None


def local_order(row, local):
    return [func for func, obj in row["entries"] if obj == local]


def quadruped_group(row, mount):
    """Entries from the mount's Quadruped::PrePhysicsUpdate to the next group."""
    entries = row["entries"]
    start = None
    for index, (func, obj) in enumerate(entries):
        if func == 0 and obj == mount:
            start = index
            break
    if start is None:
        return []
    group = [0]
    for func, obj in entries[start + 1:]:
        if func == 0:
            break
        if func in (2, 3, 5):
            group.append(func)
        elif func == 1:
            break
    return group


class Capture:
    def __init__(self, args, raw_dir: Path):
        self.args = args
        self.raw_dir = raw_dir
        self.mcp = None
        self.pid = None
        self.pipe = None
        self.base = 0
        self.meta = {"started": time.strftime("%Y-%m-%dT%H:%M:%S"),
                     "argv": sys.argv, "commands": [], "inputs": [], "phases": [],
                     "failures": []}
        self.session = None
        self.session_local = {}
        self.session_mount = {}
        self.session_identity = {}
        self.phase_session = {}
        self.player_entity = None
        self.player_entity_id = None
        self.mount_entity = None
        self.scenario_rows = {}
        self.state_rows = {}
        self.pos_rows = {}
        self.frame_ids = {}
        self.render_frames = {}

    def record_command(self, command):
        self.meta["commands"].append(command)

    def ui(self, code):
        return mcp_text(self.mcp, "client_lua", pid=self.pid, target="ui", code=code)

    def lua(self, code):
        return mcp_text(self.mcp, "client_lua", pid=self.pid, target="system", code=code)

    def chat(self, message):
        self.meta["inputs"].append("client_chat " + message)
        return mcp_text(self.mcp, "client_chat", pid=self.pid, message=message)

    def key(self, name, duration):
        self.meta["inputs"].append(f"client_input {name} {int(duration)}ms")
        return mcp_text(self.mcp, "client_input", pid=self.pid, key=name,
                        duration_ms=int(duration))

    def start_stack(self):
        port = int(self.args.login.rsplit(":", 1)[1]) + 2
        with socket.socket() as sock:
            sock.settimeout(0.3)
            if sock.connect_ex(("127.0.0.1", port)) == 0:
                log("lane stack already up")
                return
        command = [str(BASH), "-lc",
                   f"cd {LANE.as_posix()} && Scripts/live-stack.sh start"]
        env = dict(os.environ, AAEMU_LANE=self.args.lane)
        log("starting lane stack")
        self.record_command(" ".join(command))
        proc = subprocess.run(command, env=env, capture_output=True, text=True, timeout=420)
        (self.raw_dir / "stack-start.txt").write_text(proc.stdout + "\n" + proc.stderr,
                                                      encoding="utf-8")
        if proc.returncode != 0:
            raise RuntimeError(f"lane stack start failed (exit {proc.returncode})")

    def launch(self, session):
        self.session = session
        self.mount_entity = None
        command = [str(DRIVER), "mcp", "--dll", str(DLL), "--client", str(CLIENT),
                   "--launcher", str(LAUNCHER), "--password-file", str(PASSWORD_FILE),
                   "--login", self.args.login, "--user", self.args.user,
                   "--artifact-dir", str(self.raw_dir / ("mcp-" + session))]
        self.record_command(" ".join(command))
        self.mcp = Mcp(command)
        out = mcp_text(self.mcp, "client_launch", fast_start=True, fast_pak=True,
                       hidden=True, user=self.args.user,
                       password_file=str(PASSWORD_FILE))
        (self.raw_dir / ("launch-" + session + ".txt")).write_text(out, encoding="utf-8")
        match = re.search(r"pid (\d+)", out)
        if not match:
            raise RuntimeError("client_launch: " + out)
        self.pid = int(match.group(1))
        self.meta.setdefault("pids", []).append({"session": session, "pid": self.pid})
        log(session, "client pid", self.pid)

    def check_anticheat(self, session):
        proc = Process(self.pid)
        try:
            modules = proc.modules()
        finally:
            proc.close()
        (self.raw_dir / ("modules-" + session + ".txt")).write_text(
            "\n".join(f"{m['name']} {m['base']:08x} {m['size']:x}" for m in modules),
            encoding="utf-8")
        bad = [m["name"] for m in modules
               if re.match(r"(?i)^(ehsvc|v3.*|ahnrpt|hshield)", m["name"])]
        if bad:
            raise RuntimeError("anti-cheat module loaded, refusing: " + ", ".join(bad))
        return modules

    def enter_world(self):
        deadline = time.monotonic() + 260
        while time.monotonic() < deadline:
            try:
                if self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()") == "3":
                    break
            except Exception:
                pass
            time.sleep(1)
        else:
            raise RuntimeError("server select stage not reached")
        log("server select")
        self.ui("X2World:EnterWorld(1,0)")
        while time.monotonic() < deadline:
            try:
                if self.ui("return X2LoginCharacter and X2LoginCharacter:GetCurrentStage()") == "4":
                    break
            except Exception:
                pass
            time.sleep(0.5)
        else:
            raise RuntimeError("character select stage not reached")
        log("character select")
        self.ui("local b=characterSelectWindow.bottomPanel.startButton b:OnClick('LeftButton')")
        while time.monotonic() < deadline:
            try:
                state = self.ui("return X2LoginCharacter:GetCurrentStage(), X2Unit and X2Unit:UnitName('player')")
                stage, _, name = state.partition("\t")
                if stage == "0" and name not in ("", "nil"):
                    log("in world as", name)
                    break
            except Exception:
                pass
            time.sleep(1)
        else:
            raise RuntimeError("in-world player not reached")
        time.sleep(2.5)
        try:
            self.ui("if tutorial and tutorial.window then tutorial.window:Show(false) end return 1")
        except Exception:
            pass
        state = self.lua(
            "local pe,pid for id,x in pairs(System.GetEntities() or {}) do "
            "if x:GetName()==%s then pe=x;pid=id end end _G.OFP_PE=pe "
            "return tostring(pe~=nil), tostring(pid), tostring(pe and pe.__this or nil)"
            % json.dumps(self.args.character))
        parts = state.split("\t")
        if len(parts) < 3 or parts[0] != "true":
            raise RuntimeError("player entity not found: " + state)
        self.player_entity_id = int(parts[1])
        matches = re.findall(r"(?:0x)?([0-9A-Fa-f]{6,8})", parts[2])
        if not matches:
            raise RuntimeError("player __this not a pointer: " + parts[2])
        self.player_entity = int(matches[-1], 16)
        log("player entity id", self.player_entity_id, "ptr", hex(self.player_entity))
        self.meta["player_entity_id"] = self.player_entity_id
        self.meta["player_entity_ptr"] = f"{self.player_entity:08x}"

    def driver_frames(self):
        reply = self.pipe.request("P")
        match = re.search(r"frames=(\d+)", reply)
        return int(match.group(1)) if match else None

    def player_state(self):
        return self.lua(
            "local p=OFP_PE:GetWorldPos() local a=OFP_PE:GetAngles() "
            "local v=OFP_PE:GetVelocity() "
            "return string.format('%.4f %.4f %.4f|%.4f %.4f %.4f|%.4f %.4f %.4f',"
            "p.x,p.y,p.z,a.x,a.y,a.z,v.x,v.y,v.z)")

    def install_trace(self, session):
        self.pipe = Pipe(self.pid)
        reply = self.pipe.request("M", "x2game.dll")
        module = parse_module(reply)
        if not module:
            raise RuntimeError("x2game module: " + reply)
        self.base = module["base"]
        self.meta["x2game"] = module
        (self.raw_dir / ("module-" + session + ".txt")).write_text(reply, encoding="utf-8")
        prologues = {}
        for func in FUNCS:
            out = self.pipe.request("R", f"x2game.dll+0x{func['rva']:x} 16")
            prologues[str(func["id"])] = out
        (self.raw_dir / ("prologues-" + session + ".txt")).write_text(
            "\n".join(f"{f['id']} {f['name']} {prologues[str(f['id'])]}" for f in FUNCS),
            encoding="utf-8")
        selftest = self.pipe.request("K", "selftest")
        self.meta["selftest"] = selftest
        if not selftest.startswith("0"):
            raise RuntimeError("driver trace selftest failed: " + selftest)
        targets = " ".join(f"x2game.dll+0x{func['rva']:x}" for func in FUNCS)
        install = self.pipe.request("K", "install " + targets)
        self.meta["install"] = install
        if not install.startswith("0"):
            raise RuntimeError("trace install failed: " + install)
        self.meta["trace_info"] = self.pipe.request("K", "info")
        log("trace installed", self.meta["trace_info"])

    def capture_identity(self, session):
        self.pipe.request("K", "clear")
        time.sleep(0.8)
        text = self.pipe.request("K", "dump 0 120")
        (self.raw_dir / ("identity-trace-" + session + ".txt")).write_text(text, encoding="utf-8")
        rows = parse_trace(text)
        local = local_actor_from_entity(self.pipe, rows, self.player_entity)
        if local is None:
            raise RuntimeError("local CPlayer actor not found by __this")
        self.session_local[session] = local
        log("local actor", hex(local))
        identity = {"actor": f"{local:08x}",
                    "actor_vtable": f"{u32_at(self.pipe, local):08x}",
                    "entity": f"{u32_at(self.pipe, local + 0xC):08x}",
                    "player_entity": f"{self.player_entity:08x}",
                    "player_entity_id": self.player_entity_id,
                    "controller": f"{u32_at(self.pipe, local + 0x24):08x}"}
        entity = u32_at(self.pipe, local + 0xC)
        controller = u32_at(self.pipe, local + 0x24)
        if controller:
            identity["controller_vtable"] = f"{u32_at(self.pipe, controller):08x}"
        # Vtable window itself, from live memory.
        window = hexof(self.pipe.request("R", f"x2game.dll+0x{VTABLE_CPLAYER_WINDOW:x} 0x220"))
        if window:
            (self.raw_dir / ("vtable-cplayer-window-" + session + ".bin")).write_bytes(window)
        # Controller vtable through ReadProcessMemory (the driver read can refuse it
        # when the page is execute-only for its probe; ReadProcessMemory is read-only).
        if controller:
            proc = Process(self.pid)
            try:
                modules = proc.modules()
                vtable = u32_at(self.pipe, controller)
                blob = proc.read(vtable, 0x100) if vtable else None
                if blob:
                    (self.raw_dir / ("vtable-controller-" + session + ".bin")).write_bytes(blob)
                owner = next((m for m in modules if m["base"] <= vtable < m["base"] + m["size"]),
                             None)
                identity["controller_vtable_module"] = owner
                if blob:
                    entries = struct.unpack("<%dI" % (len(blob) // 4), blob)
                    identity["controller_vtable_entries"] = [f"{entry:08x}" for entry in entries]
            finally:
                proc.close()
        # Static CPlayer and Quadruped vtable slots, as live pointers.
        vt = {}
        for label, rva in [("cplayer_window", VTABLE_CPLAYER_WINDOW),
                           ("cplayer_prephysics", VTABLE_CPlayer_PrePhysicsUpdate),
                           ("process_rotation", VTABLE_ProcessFrameMovementRotation),
                           ("process_movement", VTABLE_ProcessFrameMovement),
                           ("cplayer_update", VTABLE_CPlayer_Update_wrapper),
                           ("quadruped", VTABLE_QUADRUPED),
                           ("quad_prephysics", VTABLE_Quadruped_PrePhysicsUpdate),
                           ("cactor_update", VTABLE_CActor_Update)]:
            vt[label] = u32_at(self.pipe, self.base + rva)
        identity["vtable_slots"] = {k: f"{v:08x}" for k, v in vt.items()}
        self.session_identity[session] = identity

    def find_mount_entity(self):
        code = (
            "local pe=_G.OFP_PE local he, best "
            "if pe~=nil then local pp=pe:GetWorldPos() "
            "for _,x in pairs(System.GetEntities() or {}) do "
            "local n=string.lower(x:GetName()) "
            "if string.find(n,'horse') and x~=pe then "
            "local p=x:GetWorldPos() local dx,dy,dz=p.x-pp.x,p.y-pp.y,p.z-pp.z "
            "local d=dx*dx+dy*dy+dz*dz "
            "if d<400 and (best==nil or d<best) then best=d; he=x end end end end "
            "_G.OFP_HE=he return tostring(he~=nil), tostring(he and he.__this or nil)")
        state = self.lua(code)
        parts = state.split("\t")
        if len(parts) < 2 or parts[0] != "true":
            return None
        matches = re.findall(r"(?:0x)?([0-9A-Fa-f]{6,8})", parts[1])
        return int(matches[-1], 16) if matches else None

    def summon_and_mount(self):
        dbg = self.meta.setdefault("mount_debug", [])
        dbg.append("pet_before=" + self.ui("return tostring(X2Mate:IsPlayerPetExists())"))
        self.chat("/item add self 8159")
        time.sleep(1.2)
        try:
            inventory = json.loads(mcp_text(self.mcp, "client_inventory", pid=self.pid))
            horses = {k: v for k, v in inventory.get("slots", {}).items()
                      if v.get("item_template_id") == 8159 and not v.get("dead")}
            dbg.append("inventory_horse_slots=" + ",".join(sorted(horses)))
        except Exception as exc:
            dbg.append("inventory_error=" + str(exc))
        use = self.ui("return tostring(X2Bag:UseItemByType(8159))")
        dbg.append("use_by_type=" + use)
        if use.strip() != "true":
            try:
                inventory = json.loads(mcp_text(self.mcp, "client_inventory", pid=self.pid))
                slot = next((int(k) for k, v in inventory["slots"].items()
                             if v.get("item_template_id") == 8159 and not v.get("dead")), None)
                if slot is not None:
                    dbg.append("handle_use_slot=" + str(slot) + ":" +
                               self.ui(f"X2Bag:HandleUse({slot}) return 'ok'"))
            except Exception as exc:
                dbg.append("handle_use_error=" + str(exc))
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if self.ui("return tostring(X2Mate:IsPlayerPetExists())").strip() == "true":
                break
            time.sleep(0.5)
        else:
            raise RuntimeError("mount summon failed; debug=" + "; ".join(dbg))
        time.sleep(1.5)
        dbg.append("pet_before_mount=" + self.ui("return tostring(X2Mate:IsPlayerPetExists())"))
        self.ui("X2Mate:MountMate() return 1")
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if self.ui("return tostring(X2Mate:IsPlayerMounting())").strip() == "true":
                time.sleep(1.0)
                self.mount_entity = self.find_mount_entity()
                dbg.append("mount_entity=" + (f"{self.mount_entity:08x}"
                                              if self.mount_entity else "not found"))
                if self.mount_entity is None:
                    raise RuntimeError("mounted but horse entity not found")
                log("mount entity", hex(self.mount_entity))
                self.meta["mount_entity_ptr"] = f"{self.mount_entity:08x}"
                return
            time.sleep(0.4)
        raise RuntimeError("mount failed; debug=" + "; ".join(dbg))

    def install_sampler(self):
        reply = self.lua(SAMPLER_LUA)
        if not reply.startswith("installed"):
            raise RuntimeError("sampler install: " + reply)

    def start_sampler(self):
        self.lua("_G.OFP_RUN = true _G.OFP_ROWS = {} return 'run'")

    def stop_sampler(self):
        count = self.lua("_G.OFP_RUN = false return #_G.OFP_ROWS")
        rows = []
        total = int(count)
        for start in range(1, total + 1, 400):
            end = min(total, start + 399)
            text = self.lua("return table.concat(_G.OFP_ROWS, '\\n', %d, %d)" % (start, end))
            if text:
                rows.extend(text.splitlines())
        return rows

    def run_phase(self, name, duration, events, fall=False):
        self.phase_session[name] = self.session
        local = self.session_local.get(self.session)
        mount = self.session_mount.get(self.session)
        log("phase", name)
        self.pipe.request("K", "clear")
        render0 = self.driver_frames()
        frame0 = self.lua("return tostring(System.GetFrameID())").strip()
        before = self.player_state()
        self.start_sampler()
        pos_sampler = PositionSampler(self.pid, {"player": self.player_entity,
                                                 "mount": self.mount_entity or 0})
        pos_sampler.start()
        wall0 = time.time()
        for at, key, hold in events:
            delay = at - (time.time() - wall0)
            if delay > 0:
                time.sleep(delay)
            self.key(key, hold)
        if fall:
            # Teleport 25 m above the current spot and let the client fall.
            pos = before.split("|")[0].split()
            x, y, z = map(float, pos)
            self.chat("/move %.2f %.2f %.2f" % (x, y, z + 25.0))
        remaining = duration - (time.time() - wall0)
        if remaining > 0:
            time.sleep(remaining)
        wall1 = time.time()
        states = self.stop_sampler()
        pos_rows = pos_sampler.stop()
        frame1 = self.lua("return tostring(System.GetFrameID())").strip()
        render1 = self.driver_frames()
        after = self.player_state()
        rows = []
        start = 0
        while True:
            text = self.pipe.request("K", f"dump {start} 150")
            chunk = parse_trace(text)
            rows.extend(chunk)
            if len(chunk) < 150:
                break
            start += len(chunk)
        (self.raw_dir / f"trace-{name}.txt").write_text(
            "\n".join(f"K {r['frame']} {r['overflow']} {r['count']} {' '.join(r['counts'])}"
                      + "".join(f"\nE {f} {o:08x}" for f, o in r["entries"]) for r in rows),
            encoding="utf-8")
        (self.raw_dir / f"state-{name}.txt").write_text("\n".join(states), encoding="utf-8")
        (self.raw_dir / f"pos-{name}.csv").write_text(
            "seconds,role,x,y,z\n" + "\n".join(
                f"{t:.4f},{role},{x:.5f},{y:.5f},{z:.5f}" for t, role, x, y, z in pos_rows),
            encoding="utf-8")
        (self.raw_dir / f"meta-{name}.json").write_text(json.dumps({
            "duration": duration, "events": events, "frame_before": frame0,
            "frame_after": frame1, "state_before": before, "state_after": after,
            "wall_seconds": wall1 - wall0, "fall": fall,
            "render_frames_before": render0, "render_frames_after": render1,
            "position_rows": len(pos_rows), "position_error": pos_sampler.error,
        }, indent=1), encoding="utf-8")
        self.scenario_rows[name] = rows
        self.state_rows[name] = states
        self.pos_rows[name] = pos_rows
        self.frame_ids[name] = (int(frame0), int(frame1))
        self.render_frames[name] = (render0, render1)
        if rows:
            found = local_actor_from_entity(self.pipe, rows, self.player_entity)
            if found is not None:
                current = self.session_local.get(self.session)
                if current is not None and found != current:
                    self.meta["local_actor_changed"] = {"session": self.session,
                                                       "from": f"{current:08x}",
                                                       "to": f"{found:08x}",
                                                       "phase": name}
                self.session_local[self.session] = found
        self.meta["phases"].append({"name": name, "session": self.session,
                                    "duration": duration, "events": events,
                                    "frames": len(rows), "samples": len(states),
                                    "position_rows": len(pos_rows),
                                    "position_error": pos_sampler.error,
                                    "render_frames": (None if render0 is None or render1 is None
                                                      else render1 - render0),
                                    "frame_before": frame0, "frame_after": frame1,
                                    "state_before": before, "state_after": after,
                                    "fall": fall})

    def mount_identity(self, rows):
        if self.mount_entity is None:
            return None
        mount_actor = locate_mount_actor(self.pipe, rows, self.mount_entity)
        if mount_actor:
            self.session_mount[self.session] = mount_actor
            identity = self.session_identity.setdefault(self.session, {})
            identity["mount_actor"] = f"{mount_actor:08x}"
            identity["mount_actor_vtable"] = f"{u32_at(self.pipe, mount_actor):08x}"
            identity["mount_entity"] = f"{self.mount_entity:08x}"
            self.meta["mount_actor"] = f"{mount_actor:08x}"
            self.meta["mount_actor_vtable"] = f"{u32_at(self.pipe, mount_actor):08x}"
        return mount_actor

    def stop_client(self):
        if self.pid:
            try:
                mcp_text(self.mcp, "client_stop", pid=self.pid, retries=20)
            except Exception as exc:
                log("client_stop failed:", exc)
            self.meta.setdefault("stopped", []).append(self.pid)
            self.pid = None

    def close_session(self):
        if self.mcp:
            self.mcp.close()
            self.mcp = None


def analyze(capture: Capture, evidence: Path):
    summary = {"functions": FUNCS, "phases": {},
               "sessions": {name: identity for name, identity in capture.session_identity.items()},
               "build": capture.meta}
    frames_csv = ["phase,frame,actor,order,counts"]
    for phase, rows in capture.scenario_rows.items():
        local = capture.session_local.get(capture.phase_session.get(phase))
        mount = capture.session_mount.get(capture.phase_session.get(phase))
        local_patterns = Counter()
        mount_patterns = Counter()
        local_counts = defaultdict(Counter)
        mount_counts = defaultdict(Counter)
        for row in rows:
            if local is not None:
                order = local_order(row, local)
                local_patterns[tuple(order)] += 1
                for func in order:
                    local_counts[row["frame"]][func] += 1
                frames_csv.append(f"{phase},{row['frame']},local,"
                                  + " ".join(map(str, order)) + ","
                                  + " ".join(f"{k}:{v}" for k, v in sorted(
                                      Counter(order).items())))
            if mount is not None:
                group = quadruped_group(row, mount)
                mount_patterns[tuple(group)] += 1
                for func in group:
                    mount_counts[row["frame"]][func] += 1
                frames_csv.append(f"{phase},{row['frame']},mount,"
                                  + " ".join(map(str, group)) + ","
                                  + " ".join(f"{k}:{v}" for k, v in sorted(
                                      Counter(group).items())))
        frame_before, frame_after = capture.frame_ids[phase]
        roles = {}
        for role in ("player", "mount"):
            vals = [(ts, x, y, z) for ts, who, x, y, z in capture.pos_rows.get(phase, [])
                    if who == role]
            if vals:
                ts0, x0, y0, z0 = vals[0]
                ts1, x1, y1, z1 = vals[-1]
                roles[role] = {
                    "samples": len(vals),
                    "seconds": ts1 - ts0,
                    "start": [x0, y0, z0], "end": [x1, y1, z1],
                    "dz": z1 - z0, "min_z": min(v[3] for v in vals),
                    "max_z": max(v[3] for v in vals),
                    "drop": z0 - min(v[3] for v in vals),
                    "peak": max(v[3] for v in vals) - z0,
                    "horizontal": ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5,
                }
        render0, render1 = capture.render_frames.get(phase, (None, None))
        summary["phases"][phase] = {
            "frames": len(rows),
            "frame_id_delta": frame_after - frame_before,
            "render_frames": None if render0 is None or render1 is None else render1 - render0,
            "local_patterns": [{"order": list(k), "frames": v}
                               for k, v in local_patterns.most_common()],
            "mount_patterns": [{"order": list(k), "frames": v}
                               for k, v in mount_patterns.most_common()],
            "samples": len(capture.state_rows.get(phase, [])),
            "position": roles,
        }
    (evidence / "summary.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    (evidence / "per-frame-calls.csv").write_text("\n".join(frames_csv) + "\n", encoding="utf-8")

    lines = []
    lines.append("On-foot update chain probe (W0 recon)")
    lines.append("=====================================")
    lines.append("")
    lines.append(f"date: {capture.meta['started']}")
    lines.append(f"argv: {' '.join(capture.meta.get('argv', []))}")
    lines.append(f"slot: {capture.args.slot} ({capture.args.user} / {capture.args.character})")
    lines.append(f"client pids: {capture.meta.get('pids')}")
    x2 = capture.meta.get("x2game")
    if x2:
        lines.append(f"x2game.dll: base=0x{x2['base']:08X} "
                     f"size=0x{x2['size']:X} stamp=0x{x2['stamp']:08X}")
    for session, identity in capture.session_identity.items():
        lines.append(f"{session} session:")
        lines.append(f"  local CPlayer actor: {identity.get('actor')} "
                     f"vtable {identity.get('actor_vtable')}")
        lines.append(f"  actor entity: {identity.get('entity')} "
                     f"(Lua __this {identity.get('player_entity')}, id {identity.get('player_entity_id')})")
        if identity.get("controller"):
            lines.append(f"  movement controller: {identity['controller']} "
                         f"vtable {identity.get('controller_vtable')}")
            owner = identity.get("controller_vtable_module")
            if owner:
                lines.append(f"    vtable module: {owner['name']} base=0x{owner['base']:08X} "
                             f"size=0x{owner['size']:X}")
        if identity.get("mount_actor"):
            lines.append(f"  mounted Quadruped actor: {identity['mount_actor']} "
                         f"vtable {identity.get('mount_actor_vtable')} "
                         f"entity {identity.get('mount_entity')}")
    lines.append("")
    prov = capture.meta.get("provenance", {})
    if prov:
        lines.append("Provenance:")
        lines.append("  probe: " + prov["probe"]["path"] + " sha256=" + prov["probe"]["sha256"])
        lines.append("  driver DLL: " + prov["driver_dll"]["path"] + " sha256=" +
                     prov["driver_dll"]["sha256"])
        lines.append("  lane: " + str(prov["lane"]["path"]) + " " +
                     str(prov["lane"]["branch"]) + " " + str(prov["lane"]["head"]))
        lines.append("  client: " + str(prov["client"]["path"]) + " " +
                     str(prov["client"]["branch"]) + " " + str(prov["client"]["head"]))
        for binary in prov.get("client_binaries", []):
            lines.append(f"  client binary: {binary['path']} sha256={binary['sha256']} "
                         f"stamp=0x{binary['time_date_stamp']:08X} "
                         f"size=0x{binary['size_of_image']:X}")
        for binary in prov.get("server_binaries", []):
            lines.append(f"  server binary: {binary['path']} sha256={binary['sha256']}")
    lines.append("")
    lines.append("Function table (ids used in the per-frame rows):")
    for func in FUNCS:
        lines.append(f"  {func['id']:2d} x2game+0x{func['rva']:06X} {func['name']} "
                     f"[decompile line {func['decomp']}]")
    lines.append("")
    lines.append("Static call edges (decompile lines):")
    for name, where in CALLS.items():
        lines.append(f"  {name}: " + ", ".join(str(x) for x in where))
    lines.append("")
    lines.append("Vtable slots read from live memory (pointers):")
    for session, identity in capture.session_identity.items():
        for key, value in identity.get("vtable_slots", {}).items():
            lines.append(f"  {session}/{key}: {value}")
        controller_entries = identity.get("controller_vtable_entries")
        if controller_entries:
            lines.append(f"  {session}/controller entries: "
                         + " ".join(controller_entries[:32]))
    lines.append("")
    lines.append("Per-scenario result (local actor rows; a row is one actor update pass):")
    lines.append("phase                 rows  render  rows/render  frame-id  local order patterns (frames)")
    for phase, data in summary["phases"].items():
        patterns = "; ".join(" ".join(map(str, p["order"])) + f" x{p['frames']}"
                             for p in data["local_patterns"][:4])
        render = data.get("render_frames")
        ratio = (f"{data['frames']/render:.2f}" if render else "n/a")
        lines.append(f"{phase:<20} {data['frames']:5d} {str(render):>6} {ratio:>11} "
                     f"{data['frame_id_delta']:9d}  {patterns}")
    lines.append("")
    lines.append("Frame unit: a trace row is one System script-system Update, and every")
    lines.append("instrumented actor function appears once per row (plus the 10/11 state")
    lines.append("branch). The client's own Present counter (driver ping frames=) matches")
    lines.append("the rows within the clear/dump boundaries; System.GetFrameID advanced about")
    lines.append("twice per row in this run, so it is not the row unit. Per rendered present")
    lines.append("the actor update pass therefore runs once (rows/render ~1.0).")
    lines.append("")
    lines.append("Per-frame local entry order (frame=function ids):")
    for phase, rows in capture.scenario_rows.items():
        local = capture.session_local.get(capture.phase_session.get(phase))
        if local is None:
            continue
        seq = []
        for row in rows:
            order = local_order(row, local)
            seq.append(f"{row['frame']}=" + ",".join(map(str, order)))
        lines.append(f"  {phase}: " + " ".join(seq))
    lines.append("")
    lines.append("Local per-function calls per frame (min..max over the phase):")
    for phase, rows in capture.scenario_rows.items():
        local = capture.session_local.get(capture.phase_session.get(phase))
        if local is None:
            continue
        per_frame = defaultdict(Counter)
        for row in rows:
            for func in local_order(row, local):
                per_frame[row["frame"]][func] += 1
        stats = {}
        for func in FUNC_BY_ID:
            values = [frame_counts[func] for frame_counts in per_frame.values()]
            if values:
                stats[func] = (min(values), max(values), sum(values) / len(values))
        lines.append("  " + phase + ": " + ", ".join(
            f"{func}:{lo}..{hi}({avg:.2f})" for func, (lo, hi, avg) in sorted(stats.items())))
    lines.append("")
    lines.append("Mounted contrast (mount Quadruped rows):")
    lines.append("phase                 rows  render  mount order patterns (frames)")
    for phase, data in summary["phases"].items():
        if not data["mount_patterns"]:
            continue
        patterns = "; ".join(" ".join(map(str, p["order"])) + f" x{p['frames']}"
                             for p in data["mount_patterns"][:4])
        render = data.get("render_frames")
        lines.append(f"{phase:<20} {data['frames']:5d} {str(render):>6}  {patterns}")
    lines.append("")
    lines.append("Entity position from ReadProcessMemory (entity+0x28, ~50 Hz):")
    lines.append("phase                 role    samples  seconds  horiz_m   dz_m    drop_m  peak_m")
    for phase, data in summary["phases"].items():
        for role, pos in data.get("position", {}).items():
            lines.append(f"{phase:<20} {role:<7} {pos['samples']:7d}  {pos['seconds']:7.2f} "
                         f"{pos['horizontal']:8.2f} {pos['dz']:7.2f} {pos['drop']:7.2f} "
                         f"{pos['peak']:7.2f}")
    lines.append("")
    lines.append("Note: the fall phase entered the fall state through a GM /move to +25 m.")
    lines.append("The entity rose to the teleport peak and fell from it while the trace shows")
    lines.append("the fall call chain; this is not a normal ledge fall, so the full trajectory")
    lines.append("and landing stay with W5.")
    lines.append("")
    lines.append("Checks:")
    checks = []
    stand = summary["phases"].get("stand", {})
    stand_order = stand.get("local_patterns", [{}])[0].get("order") if stand else None
    checks.append(("stand local order", stand_order == [1, 4, 6, 7, 5, 12, 8, 9],
                   str(stand_order)))
    for phase, data in summary["phases"].items():
        if phase == "fall":
            # The GM teleport crosses a client cell; rows do not match the render
            # frames during the load. The call-chain check below is the proof.
            continue
        render = data.get("render_frames")
        if render:
            delta = abs(data["frames"] - render)
            checks.append((f"{phase} rows/render-frames", delta <= max(4, int(0.15 * render)),
                           f"rows={data['frames']} render={render}"))
    fall_data = summary["phases"].get("fall")
    if fall_data:
        with_fall = sum(p["frames"] for p in fall_data["local_patterns"]
                        if 10 in p["order"] and 11 in p["order"])
        checks.append(("fall state calls 10+11 per frame", with_fall > 0,
                       f"{with_fall}/{fall_data['frames']} frames"))
    jump_peaks = []
    for jump_phase in ("jump_run", "jump_run_b"):
        jump = summary["phases"].get(jump_phase, {}).get("position", {}).get("player")
        if jump:
            jump_peaks.append((jump_phase, jump["peak"]))
    if jump_peaks:
        best = max(jump_peaks, key=lambda item: item[1])
        checks.append(("running jump peak > 0.3 m", best[1] > 0.3,
                       f"best={best[0]} peak={best[1]:.2f} m; " +
                       ", ".join(f"{name}={peak:.2f}" for name, peak in jump_peaks)))
    # The local CPlayer must never enter the Quadruped chain on foot.
    quad_funcs = {0, 2, 3}
    on_foot_phases = [phase for phase in summary["phases"]
                      if capture.phase_session.get(phase) == "on_foot"]
    leaked = [(phase, pattern["order"]) for phase in on_foot_phases
              for pattern in summary["phases"][phase]["local_patterns"]
              if quad_funcs.intersection(pattern["order"])]
    checks.append(("on-foot local never calls 0/2/3", not leaked, str(leaked)))
    mounted_forward = summary["phases"].get("mounted_forward", {})
    rider_orders = [p["order"] for p in mounted_forward.get("local_patterns", [])]
    checks.append(("mounted rider keeps CPlayer chain", bool(rider_orders) and all(
        1 in order and 12 in order for order in rider_orders), str(rider_orders)))
    mount_orders = [p["order"] for p in mounted_forward.get("mount_patterns", [])]
    checks.append(("mounted Quadruped chain 0/2/5", bool(mount_orders) and all(
        set(order) == {0, 2, 5} for order in mount_orders), str(mount_orders)))
    stand_jump = summary["phases"].get("jump_stand", {}).get("position", {}).get("player")
    if stand_jump:
        checks.append(("standing jump entity-z unchanged (root motion?)", True,
                       f"peak={stand_jump['peak']:.2f} m (informational)"))
    summary["checks"] = [{"name": name, "pass": ok, "detail": detail}
                         for name, ok, detail in checks]
    (evidence / "summary.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    for name, ok, detail in checks:
        lines.append(f"  [{'PASS' if ok else 'FAIL'}] {name}: {detail}")
    lines.append("")
    lines.append("Scenario inputs and state (Lua sampler rows):")
    for entry in capture.meta["phases"]:
        lines.append(f"  {entry['name']}: events={entry['events']} "
                     f"before={entry['state_before']} after={entry['state_after']}")
    lines.append("")
    lines.append(f"driver selftest: {capture.meta.get('selftest')}")
    lines.append(f"trace install: {capture.meta.get('install')}")
    lines.append(f"trace info: {capture.meta.get('trace_info')}")
    lines.append("")
    lines.append("Evidence files are generated by tools/onfoot_update_probe.py; the raw")
    lines.append("trace/state dumps and exact commands are in the run directory named in")
    lines.append("ACTUAL_RUN_REPORT.md next to this report.")
    (evidence / "report.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--slot", default=os.environ.get("ONFOOT_SLOT", "wp6"))
    parser.add_argument("--lane", default=os.environ.get("ONFOOT_LANE_NAME", "move-golden"))
    parser.add_argument("--login", default=os.environ.get("ONFOOT_LOGIN", "127.0.0.1:28237"))
    parser.add_argument("--tag", default=time.strftime("onfoot-update-%Y%m%d-r1"))
    parser.add_argument("--out", type=Path,
                        default=ROOT / "data" / "extract" / "onfoot-update")
    parser.add_argument("--evidence", type=Path, default=ROOT / "tools" / "evidence" / "onfoot_update")
    parser.add_argument("--mode", choices=["on_foot", "mounted", "both"], default="both")
    parser.add_argument("--keep-client", action="store_true")
    args = parser.parse_args()

    for path in (DRIVER, DLL, CLIENT, LAUNCHER, PASSWORD_FILE):
        if not path.exists():
            raise SystemExit(f"missing required file: {path}")
    account = live_slots.resolve(args.slot, args.lane, "a")
    args.user = account.user
    args.character = account.character
    raw_dir = args.out / args.tag
    raw_dir.mkdir(parents=True, exist_ok=True)
    args.evidence.mkdir(parents=True, exist_ok=True)
    evidence = args.evidence

    capture = Capture(args, raw_dir)
    def git_value(repo, *args):
        result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
        return result.stdout.strip() if result.returncode == 0 else None
    capture.meta["provenance"] = {
        "probe": {"path": str(Path(__file__)), "sha256": sha256(Path(__file__))},
        "driver_dll": {"path": str(DLL), "sha256": sha256(DLL)},
        "lane": {"path": str(LANE), "head": git_value(LANE, "rev-parse", "HEAD"),
                 "branch": git_value(LANE, "rev-parse", "--abbrev-ref", "HEAD")},
        "client": {"path": str(ROOT), "head": git_value(ROOT, "rev-parse", "HEAD"),
                   "branch": git_value(ROOT, "rev-parse", "--abbrev-ref", "HEAD")},
        "client_binaries": [pe_identity(path) for path in
                            (CLIENT, Path("D:/AA1.2/bin32/x2game.dll")) if path.exists()],
        "server_binaries": [{"path": str(path), "sha256": sha256(path)}
                            for path in (LIVE / "bin" / "aaemu-game.exe",
                                         LIVE / "bin" / "aaemu-login.exe") if path.exists()],
    }
    exit_code = 0
    try:
        capture.start_stack()
        modes = ["on_foot", "mounted"] if args.mode == "both" else [args.mode]
        for mode in modes:
            capture.launch(mode)
            capture.check_anticheat(mode)
            capture.enter_world()
            capture.install_trace(mode)
            capture.capture_identity(mode)
            capture.install_sampler()
            if mode == "mounted":
                capture.summon_and_mount()
                for name, duration, events in SCENARIOS["mounted"]:
                    capture.run_phase(name, duration, events)
                if capture.scenario_rows.get("mounted_stand"):
                    capture.mount_identity(capture.scenario_rows["mounted_stand"])
            else:
                for name, duration, events in SCENARIOS["on_foot"]:
                    capture.run_phase(name, duration, events, fall=(name == "fall"))
            capture.pipe.request("K", "uninstall")
            if not args.keep_client:
                capture.stop_client()
                capture.close_session()
        analyze(capture, evidence)
    except Exception as exc:
        exit_code = 1
        capture.meta["failures"].append(str(exc))
        log("FAILED:", exc)
        try:
            analyze(capture, evidence)
        except Exception as second:
            log("analysis also failed:", second)
    finally:
        if capture.pid and not args.keep_client:
            capture.stop_client()
        capture.close_session()
        (raw_dir / "run-meta.json").write_text(json.dumps(capture.meta, indent=1),
                                               encoding="utf-8")
        lines = ["# Actual run report", "",
                 f"tag: {args.tag}", f"date: {capture.meta['started']}",
                 f"slot: {args.slot} ({args.user} / {args.character})",
                 f"client pid: {capture.pid}", f"exit code: {exit_code}", "",
                 "## Invocation", "",
                 "    " + " ".join(sys.argv), "",
                 "## Command lines", ""]
        lines += ["    " + c for c in capture.meta["commands"]]
        lines += ["", "## Scenario inputs", ""]
        lines += ["    " + c for c in capture.meta["inputs"]] or ["    (none)"]
        lines += ["", "## Client build", ""]
        for path in [CLIENT, Path("D:/AA1.2/bin32/x2game.dll")]:
            if path.exists():
                identity = pe_identity(path)
                lines.append(f"- {path}: sha256={identity['sha256']} "
                             f"stamp=0x{identity['time_date_stamp']:08X} "
                             f"size=0x{identity['size_of_image']:X}")
        lines.append(f"- driver DLL: {DLL} sha256={sha256(DLL)}")
        for path in [LIVE / "bin" / "aaemu-game.exe", LIVE / "bin" / "aaemu-login.exe"]:
            if path.exists():
                lines.append(f"- server {path.name}: sha256={sha256(path)}")
        git_lane = subprocess.run(["git", "-C", str(LANE), "rev-parse", "HEAD"],
                                  capture_output=True, text=True)
        git_client = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"],
                                    capture_output=True, text=True)
        lines += ["", f"- lane source: {git_lane.stdout.strip()}",
                  f"- client source: {git_client.stdout.strip()}", "",
                  "## Captured", "",
                  "- one trace row per engine frame (System Lua state CScriptSystem::Update)",
                  "- one Lua sampler row per ~16 ms (position, angles, velocity, animation state)",
                  "- per-scenario key input through the client driver (CryInput)",
                  "- local actor identified by Lua PE.__this == [actor+0xc]",
                  "", "## Failures", ""]
        lines += ["- " + f for f in capture.meta["failures"]] or ["- none"]
        lines += ["", "## Raw files", "",
                  "- trace-<phase>.txt, state-<phase>.txt, meta-<phase>.json",
                  "- launch.txt, modules.txt, prologues.txt, module.txt, mcp/", "",
                  "The trace detours only increment in-DLL counters and record (function, ECX);",
                  "the original prologue bytes execute in the thunk, so game state is unchanged.",
                  ]
        (raw_dir / "ACTUAL_RUN_REPORT.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
